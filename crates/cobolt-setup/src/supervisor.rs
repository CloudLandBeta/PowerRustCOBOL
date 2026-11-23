// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! The custom action that runs one long step of the setup and stays answerable
//! while it does (spec 112).
//!
//! Windows Installer documents exactly one way for a custom action to learn that
//! the person at the keyboard pressed Cancel: it calls `MsiProcessMessage` and
//! reads `IDCANCEL` from the answer. The same call is the only way to put text
//! on the progress page. A quiet-exec action does neither, so this DLL starts the
//! step itself, reads its output as it comes, turns each line into progress text
//! and asks the installer about Cancel every half second.
//!
//! The step is put in a **job object** that kills everything in it when the job
//! is closed, so a Cancel, a watchdog or a crash of the installer cannot leave a
//! `cargo` or a `rustc` running behind.
//!
//! Only `IDCANCEL` makes the action fail. A step that cannot start, a child that
//! exits with an error and a watchdog that gives up are *results* the setup
//! reports on its finish page; none of them is allowed to fail the install
//! (R12).

use crate::data::{RunData, Spawn};
use crate::ident;
use crate::lines::LineBuffer;
use crate::progress::Part;
use crate::result;
use crate::ticks::Tracker;
use crate::watch::{outcome_for, End, Watchdog};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_INSTALL_USEREXIT, ERROR_MORE_DATA, ERROR_SUCCESS,
    GENERIC_READ, HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::{
    DuplicateTokenEx, RevertToSelf, SecurityImpersonation, TokenPrimary, SECURITY_ATTRIBUTES,
    TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
    MsiCloseHandle, MsiCreateRecord, MsiGetPropertyW, MsiProcessMessage, MsiRecordSetInteger,
    MsiRecordSetStringW, INSTALLMESSAGE_ACTIONDATA, INSTALLMESSAGE_INFO, INSTALLMESSAGE_PROGRESS,
    MSIHANDLE,
};
use windows_sys::Win32::System::Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, CreateProcessW, GetCurrentProcess, GetCurrentThread, GetExitCodeProcess,
    OpenProcessToken, OpenThreadToken, ResumeThread, SetThreadToken, WaitForSingleObject,
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, NORMAL_PRIORITY_CLASS,
    PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTF_USESTDHANDLES, STARTUPINFOW,
};

/// `IDCANCEL`, which `MsiProcessMessage` answers when the developer has cancelled.
pub(crate) const IDCANCEL: i32 = 2;
/// The kinds of `INSTALLMESSAGE_PROGRESS` (field 1 of its record): how this action's text moves the bar, a report
/// of ticks done, and an addition to the bar's total.
const PROGRESS_ACTION_INFO: i32 = 1;
const PROGRESS_REPORT: i32 = 2;
const PROGRESS_ADDITION: i32 = 3;
/// `MAXIMUM_ALLOWED`: duplicate the token with every right its security descriptor grants (declared here so the
/// crate does not pull in a whole feature of `windows-sys` for one number).
const MAXIMUM_ALLOWED: u32 = 0x0200_0000;
/// How often the installer is asked about Cancel.
const CANCEL_POLL: Duration = Duration::from_millis(500);
/// How long a step that was asked to stop (its cancel file) is given to stop its own children before the whole job
/// is ended.
const CANCEL_GRACE: Duration = Duration::from_secs(5);
/// The most often a line becomes progress text; a child that prints a thousand lines a second must not flood the page.
const TEXT_EVERY: Duration = Duration::from_millis(250);
/// How many of a step's lines are copied into the install log one by one.
const LOGGED_LINES: usize = 400;

// ── the installer session ─────────────────────────────────────────────────

/// The session handle of the running custom action and the three things this DLL does with it.
pub(crate) struct Session(pub(crate) MSIHANDLE);

impl Session {
    /// A property of the install. A deferred action can read only `CustomActionData`, `ProductCode` and `UserSID`.
    pub(crate) fn property(&self, name: &str) -> Option<String> {
        let name = wide(name);
        let mut len = 0u32;
        let mut probe = [0u16; 1];
        // SAFETY: `name` is NUL-terminated; `probe` and `len` are valid. A zero length asks for the size.
        let rc = unsafe { MsiGetPropertyW(self.0, name.as_ptr(), probe.as_mut_ptr(), &mut len) };
        if rc != ERROR_MORE_DATA && rc != ERROR_SUCCESS {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let mut cap = buf.len() as u32;
        // SAFETY: `buf` holds `cap` characters.
        let rc = unsafe { MsiGetPropertyW(self.0, name.as_ptr(), buf.as_mut_ptr(), &mut cap) };
        (rc == ERROR_SUCCESS).then(|| String::from_utf16_lossy(&buf[..cap as usize]))
    }

    /// One line into the install log (`/l*v`). Never shown to the developer.
    pub(crate) fn info(&self, text: &str) {
        let _ = self.send_text(INSTALLMESSAGE_INFO, text);
    }

    /// One line on the progress page. `true` when the developer has cancelled.
    pub(crate) fn action_data(&self, text: &str) -> bool {
        self.send_text(INSTALLMESSAGE_ACTIONDATA, text) == IDCANCEL
    }

    /// Move the progress bar by `ticks`; `0` only asks whether the developer has cancelled. `true` when they have.
    pub(crate) fn progress(&self, ticks: u32) -> bool {
        self.progress_message(PROGRESS_REPORT, ticks) == IDCANCEL
    }

    /// Tell the installer that this action's text messages move the bar by nothing: the ticks are reported by
    /// [`Session::progress`], from the step's lines.
    pub(crate) fn action_info(&self) {
        let _ = self.progress_message(PROGRESS_ACTION_INFO, 0);
    }

    /// Add `ticks` to the bar's total. Sent by an immediate action while the install's script is being made.
    /// Answers what the installer answered (`IDCANCEL` when the developer has cancelled).
    pub(crate) fn add_total(&self, ticks: u32) -> i32 {
        self.progress_message(PROGRESS_ADDITION, ticks)
    }

    /// One `INSTALLMESSAGE_PROGRESS`: field 1 is what kind, field 2 the ticks, field 3 stays 0.
    fn progress_message(&self, kind: i32, ticks: u32) -> i32 {
        // SAFETY: the record is closed before returning.
        unsafe {
            let rec = MsiCreateRecord(3);
            if rec == 0 {
                return 0;
            }
            MsiRecordSetInteger(rec, 1, kind);
            MsiRecordSetInteger(rec, 2, i32::try_from(ticks).unwrap_or(i32::MAX));
            MsiRecordSetInteger(rec, 3, 0);
            let answer = MsiProcessMessage(self.0, INSTALLMESSAGE_PROGRESS, rec);
            MsiCloseHandle(rec);
            answer
        }
    }

    fn send_text(&self, kind: i32, text: &str) -> i32 {
        let text = wide(text);
        let template = wide("[1]");
        // SAFETY: the record is closed before returning; both strings are NUL-terminated.
        unsafe {
            let rec = MsiCreateRecord(1);
            if rec == 0 {
                return 0;
            }
            MsiRecordSetStringW(rec, 0, template.as_ptr());
            MsiRecordSetStringW(rec, 1, text.as_ptr());
            let answer = MsiProcessMessage(self.0, kind, rec);
            MsiCloseHandle(rec);
            answer
        }
    }
}

pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

// ── handles ───────────────────────────────────────────────────────────────

/// A Windows handle that is closed when it goes out of scope.
struct Owned(HANDLE);

impl Owned {
    fn is_valid(&self) -> bool {
        !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        if self.is_valid() {
            // SAFETY: the handle is ours and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }
}

fn last_error(what: &str) -> String {
    // SAFETY: reads the calling thread's last error.
    format!("{what} failed (Windows error {})", unsafe {
        GetLastError()
    })
}

// ── starting the step ─────────────────────────────────────────────────────

/// A running step: the process, the job that holds it and the read end of its output.
struct Step {
    process: Owned,
    job: Owned,
    output: Owned,
}

/// The primary token the child runs with, and where it came from.
///
/// An impersonated action has the user's token on the *thread*; an action that
/// runs in a server process started with the user's token has it on the
/// *process*. The spike of spec 112 found out which one a deferred action gets,
/// and the line this function returns is how every real run keeps saying so.
fn user_token() -> Result<UserToken, String> {
    let mut source: HANDLE = null_mut();
    let wanted = TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY;
    // SAFETY: both calls fill `source`, which `Owned` closes.
    let from = unsafe {
        if OpenThreadToken(GetCurrentThread(), wanted, 1, &mut source) != 0 {
            "thread token"
        } else if OpenProcessToken(GetCurrentProcess(), wanted, &mut source) != 0 {
            "process token"
        } else {
            return Err(last_error("opening the user's token"));
        }
    };
    let source = Owned(source);
    let mut primary: HANDLE = null_mut();
    // SAFETY: `source` is a valid token handle; `primary` receives a new handle.
    let ok = unsafe {
        DuplicateTokenEx(
            source.0,
            MAXIMUM_ALLOWED,
            null(),
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    };
    if ok == 0 {
        return Err(last_error("duplicating the user's token"));
    }
    Ok(UserToken {
        primary: Owned(primary),
        // Kept only when the user's identity is on this thread: it is what the thread goes back to after the
        // child has been started (see `start`).
        impersonation: (from == "thread token").then_some(source),
        from,
    })
}

/// What [`user_token`] found.
struct UserToken {
    /// The primary token the child runs with.
    primary: Owned,
    /// The thread's own impersonation token, when the thread is impersonating.
    impersonation: Option<Owned>,
    /// Which token it was: `thread token` or `process token`.
    from: &'static str,
}

/// A handle to `NUL`, inheritable, for the child's standard input.
fn nul_input() -> Owned {
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let name = wide("NUL");
    // SAFETY: `name` is NUL-terminated and `sa` outlives the call.
    Owned(unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &sa,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    })
}

/// Start the step suspended, put it in a kill-on-close job, then let it run.
fn start(session: &Session, data: &RunData) -> Result<Step, String> {
    // The pipe the child writes to. Only the write end is inherited.
    let sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write): (HANDLE, HANDLE) = (null_mut(), null_mut());
    // SAFETY: out-parameters are valid; both handles are owned right after.
    if unsafe { CreatePipe(&mut read, &mut write, &sa, 0) } == 0 {
        return Err(last_error("creating the output pipe"));
    }
    let (read, write) = (Owned(read), Owned(write));
    // SAFETY: `read` is a valid handle; the read end must not be inherited.
    unsafe { windows_sys::Win32::Foundation::SetHandleInformation(read.0, HANDLE_FLAG_INHERIT, 0) };
    let input = nul_input();

    // SAFETY: STARTUPINFOW is plain data; zero is a valid starting state.
    let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
    startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    startup.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
    startup.wShowWindow = 0; // SW_HIDE
    startup.hStdInput = input.0;
    startup.hStdOutput = write.0;
    startup.hStdError = write.0;

    let mut command = wide(&data.command);
    // SAFETY: PROCESS_INFORMATION is plain data.
    let mut info: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let flags =
        CREATE_SUSPENDED | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT | NORMAL_PRIORITY_CLASS;

    let created = match data.spawn {
        Spawn::Plain => {
            session.info("PRC: starting the step with the action's own identity");
            // SAFETY: every pointer is valid for the call; the command buffer is mutable as the API requires.
            unsafe {
                CreateProcessW(
                    null(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    flags,
                    null(),
                    null(),
                    &startup,
                    &mut info,
                )
            }
        }
        Spawn::AsUser => {
            let user = user_token()?;
            let token = &user.primary;
            session.info(&format!(
                "PRC: starting the step with the user's token (from the {})",
                user.from
            ));
            let mut env: *mut c_void = null_mut();
            // SAFETY: `token` is a primary token; `env` receives a block that is destroyed below.
            let have_env = unsafe { CreateEnvironmentBlock(&mut env, token.0, 0) } != 0;
            if !have_env {
                session.info(&format!(
                    "PRC: {}; the step inherits the action's environment",
                    last_error("building the user's environment")
                ));
                env = null_mut();
            }
            // CreateProcessAsUser checks the *caller's* privileges (assign a primary token, raise a quota). A
            // thread that is impersonating an ordinary user does not have them, so it steps out of the
            // impersonation for the one call and goes back in afterwards.
            // SAFETY: `RevertToSelf` and `SetThreadToken` act on the calling thread only.
            let stepped_out = user.impersonation.is_some() && unsafe { RevertToSelf() } != 0;
            // SAFETY: as above; `env` is either null or the block just built.
            let ok = unsafe {
                CreateProcessAsUserW(
                    token.0,
                    null(),
                    command.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    flags,
                    env,
                    null(),
                    &startup,
                    &mut info,
                )
            };
            // Read the error before anything else can overwrite it.
            let failure = (ok == 0).then(|| {
                format!(
                    "{}{}",
                    last_error("CreateProcessAsUser"),
                    if stepped_out {
                        ", called after leaving the impersonation"
                    } else {
                        ""
                    }
                )
            });
            if stepped_out {
                if let Some(back) = &user.impersonation {
                    // SAFETY: the token is ours; a null thread means the calling thread.
                    unsafe { SetThreadToken(null(), back.0) };
                }
            }
            if have_env {
                // SAFETY: the block is ours and is destroyed once.
                unsafe { DestroyEnvironmentBlock(env) };
            }
            if let Some(why) = failure {
                return Err(why);
            }
            ok
        }
    };
    if created == 0 {
        return Err(last_error("starting the step"));
    }
    let (process, thread) = (Owned(info.hProcess), Owned(info.hThread));
    // The parent's copy of the write end must go, or the pipe never reports that the child has finished.
    drop(write);
    drop(input);

    // SAFETY: the job handle is owned right after; the limit structure is plain data.
    let job = Owned(unsafe { CreateJobObjectW(null(), null()) });
    if job.is_valid() {
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is a valid structure of the size passed.
        let set = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        // SAFETY: both handles are valid.
        let assigned = unsafe { AssignProcessToJobObject(job.0, process.0) };
        if set == 0 || assigned == 0 {
            session.info(&format!(
                "PRC: the step is NOT in a kill-on-close job ({})",
                last_error("setting up the job")
            ));
        }
    } else {
        session.info(&format!(
            "PRC: no job object ({})",
            last_error("creating it")
        ));
    }
    // SAFETY: the thread handle is valid and suspended.
    unsafe { ResumeThread(thread.0) };
    Ok(Step {
        process,
        job,
        output: read,
    })
}

/// Read what the child has written, without waiting for more. `true` in the second place when the pipe has closed.
fn drain(output: &Owned, buffer: &mut LineBuffer) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    for _ in 0..64 {
        let mut available = 0u32;
        // SAFETY: `available` is a valid out-parameter.
        let peeked = unsafe {
            PeekNamedPipe(
                output.0,
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            )
        };
        if peeked == 0 {
            return (lines, true);
        }
        if available == 0 {
            return (lines, false);
        }
        let mut chunk = [0u8; 4096];
        let mut read = 0u32;
        let want = (available as usize).min(chunk.len()) as u32;
        // SAFETY: `chunk` holds `want` bytes.
        let ok = unsafe { ReadFile(output.0, chunk.as_mut_ptr(), want, &mut read, null_mut()) };
        if ok == 0 || read == 0 {
            return (lines, true);
        }
        lines.extend(buffer.push(&chunk[..read as usize]));
    }
    (lines, false)
}

// ── the run ───────────────────────────────────────────────────────────────

fn exited(step: &Step) -> bool {
    // SAFETY: the process handle is valid.
    unsafe { WaitForSingleObject(step.process.0, 0) == WAIT_OBJECT_0 }
}

fn exit_code(step: &Step) -> Option<u32> {
    let mut code = 0u32;
    // SAFETY: the process handle is valid.
    (unsafe { GetExitCodeProcess(step.process.0, &mut code) } != 0).then_some(code)
}

fn kill(step: &Step) {
    // SAFETY: both handles are valid. The job is the reliable way; the process handle is the fallback.
    unsafe {
        if !step.job.is_valid() || TerminateJobObject(step.job.0, ERROR_INSTALL_USEREXIT) == 0 {
            windows_sys::Win32::System::Threading::TerminateProcess(
                step.process.0,
                ERROR_INSTALL_USEREXIT,
            );
        }
    }
}

/// What the lines of a step do to the install log, the progress page and the bar, and how long the step has been
/// quiet.
struct Feed {
    tracker: Tracker,
    watchdog: Watchdog,
    last_text: Option<Instant>,
    logged: usize,
}

impl Feed {
    /// Take a batch of lines. `true` when the installer answered Cancel to one of the messages this caused.
    fn take(&mut self, session: &Session, lines: &[String]) -> bool {
        if lines.is_empty() {
            return false;
        }
        // Any line at all, the child's own included, is a step that is alive.
        self.watchdog.heard(Instant::now());
        for line in lines {
            if self.logged < LOGGED_LINES {
                session.info(&format!("PRC: > {line}"));
                self.logged += 1;
                if self.logged == LOGGED_LINES {
                    session
                        .info("PRC: (the rest of the step's lines are not copied into this log)");
                }
            }
        }
        let seen = self.tracker.see(lines);
        let mut cancel = false;
        if seen.ticks > 0 {
            cancel |= session.progress(seen.ticks);
        }
        if let Some(text) = seen.text {
            if self.last_text.is_none_or(|t| t.elapsed() >= TEXT_EVERY) {
                self.last_text = Some(Instant::now());
                cancel |= session.action_data(&text);
            }
        }
        cancel
    }
}

/// Stop the step on a Cancel: ask it to stop (its cancel file), give it a few seconds to do so and to stop its own
/// `cargo`, then end the whole job whatever is left.
fn stop(session: &Session, data: &RunData, step: &Step, buffer: &mut LineBuffer, begun: Instant) {
    let mut asked = false;
    if let Some(file) = &data.cancel_file {
        match std::fs::write(file, b"cancel") {
            Ok(()) => asked = true,
            Err(e) => session.info(&format!(
                "PRC: the cancel file {file} could not be written ({e}); the step is stopped without being asked"
            )),
        }
    }
    if asked {
        let until = Instant::now() + CANCEL_GRACE;
        while Instant::now() < until && !exited(step) {
            // Keep the pipe empty, or a step that is writing its last lines cannot end.
            let _ = drain(&step.output, buffer);
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let by_itself = exited(step);
    if !by_itself {
        kill(step);
    }
    session.info(&format!(
        "PRC: Cancel after {:.1}s: the step {}",
        begun.elapsed().as_secs_f64(),
        if by_itself {
            "stopped when asked"
        } else {
            "and everything it started were stopped"
        }
    ));
    // SAFETY: the process handle is valid; the wait is bounded.
    unsafe { WaitForSingleObject(step.process.0, 10_000) };
}

/// A step that ended without saying how is a result for the finish page, never a failed install (R12): write it
/// into the result file the step would have written to.
fn report(session: &Session, data: &RunData, end: &End, part: Option<Part>) {
    let part = part.or(data.part).unwrap_or(Part::Check);
    let Some(outcome) = outcome_for(end, part) else {
        return;
    };
    session.info(&format!("PRC: result: {}", result::format(&outcome)));
    match &data.result_file {
        Some(file) => {
            if let Err(e) = result::append(Path::new(file), &outcome) {
                session.info(&format!(
                    "PRC: the result file {file} could not be written ({e})"
                ));
            }
        }
        None => session.info("PRC: no result file was named, so the finish page will not say so"),
    }
}

/// Run one step to its end, a Cancel or the watchdog. Returns the action's result for the installer.
fn run(session: &Session, data: &RunData) -> u32 {
    session.info(&format!("PRC: supervisor: {}", ident::describe()));
    session.info(&format!("PRC: command: {}", data.command));
    // A cancel file left by an earlier run would stop this step at once.
    if let Some(file) = &data.cancel_file {
        let _ = std::fs::remove_file(file);
    }
    // The text of this action moves the bar by nothing: the ticks are reported by the supervisor, from the lines.
    session.action_info();

    let step = match start(session, data) {
        Ok(step) => step,
        Err(why) => {
            // A step that cannot start is a result for the finish page, not a failed install (R12).
            session.info(&format!("PRC: the step could not start: {why}"));
            report(session, data, &End::CouldNotStart(why), None);
            return ERROR_SUCCESS;
        }
    };

    let begun = Instant::now();
    let mut feed = Feed {
        tracker: Tracker::new(data.part, data.ticks),
        watchdog: Watchdog::new(data.watchdog_secs, begun),
        last_text: None,
        logged: 0,
    };
    let mut buffer = LineBuffer::default();
    let mut last_poll = begun;
    let mut cancelled = false;
    let mut gave_up = false;

    loop {
        let (lines, closed) = drain(&step.output, &mut buffer);
        let mut asked_to_cancel = feed.take(session, &lines);

        if !cancelled && !asked_to_cancel && last_poll.elapsed() >= CANCEL_POLL {
            last_poll = Instant::now();
            asked_to_cancel = session.progress(0);
        }
        if asked_to_cancel && !cancelled {
            cancelled = true;
            if data.kill_on_cancel {
                session.action_data("Cancelling: stopping the step that is running");
                stop(session, data, &step, &mut buffer, begun);
                break;
            }
            session.info(&format!(
                "PRC: Cancel after {:.1}s: the step is NOT stopped; waiting for it to finish",
                begun.elapsed().as_secs_f64()
            ));
            session.action_data("Cancelling: waiting for the step that is running to finish");
        }

        if closed || exited(&step) {
            let (rest, _) = drain(&step.output, &mut buffer);
            let tail: Vec<String> = rest.into_iter().chain(buffer.finish()).collect();
            feed.take(session, &tail);
            break;
        }
        if feed.watchdog.expired(Instant::now()) {
            session.info(&format!(
                "PRC: no output for {} s: the step is given up and stopped",
                data.watchdog_secs
            ));
            kill(&step);
            gave_up = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let code = exit_code(&step);
    session.info(&format!(
        "PRC: the step ended after {:.1}s with exit code {}{}{}",
        begun.elapsed().as_secs_f64(),
        code.map_or_else(|| "unknown".to_owned(), |c| c.to_string()),
        if cancelled { " (cancelled)" } else { "" },
        if gave_up { " (watchdog)" } else { "" },
    ));
    if let Some(file) = &data.cancel_file {
        let _ = std::fs::remove_file(file);
    }
    if !cancelled {
        // Whatever happened, what is left of the step's share of the bar is reported, so the bar is where the
        // install expects it. A Cancel that arrives with it is still a Cancel.
        cancelled = session.progress(feed.tracker.finish());
    }

    let end = if cancelled {
        End::Cancelled
    } else if gave_up {
        End::GaveUp {
            secs: data.watchdog_secs,
        }
    } else {
        End::Exited(code)
    };
    report(session, data, &end, feed.tracker.part());
    if cancelled {
        ERROR_INSTALL_USEREXIT
    } else {
        ERROR_SUCCESS
    }
}

/// Run `body` as one custom action: nothing it does, not even a panic, may fail the install by accident.
pub(crate) fn guarded(handle: MSIHANDLE, body: impl FnOnce(&Session) -> u32) -> u32 {
    let session = Session(handle);
    match catch_unwind(AssertUnwindSafe(|| body(&session))) {
        Ok(result) => result,
        Err(_) => {
            session.info("PRC: the supervisor stopped on an internal error; the install goes on");
            ERROR_SUCCESS
        }
    }
}

/// The entry point the `.msi` names (`DllEntry="PrcRun"`): run the step described by `CustomActionData`.
#[no_mangle]
pub extern "system" fn PrcRun(handle: MSIHANDLE) -> u32 {
    guarded(handle, |session| {
        let Some(text) = session.property("CustomActionData") else {
            session.info("PRC: the action was given no CustomActionData; nothing was run");
            return ERROR_SUCCESS;
        };
        match RunData::parse(&text) {
            Ok(data) => run(session, &data),
            Err(why) => {
                session.info(&format!(
                    "PRC: the action data could not be read ({why}); nothing was run"
                ));
                ERROR_SUCCESS
            }
        }
    })
}

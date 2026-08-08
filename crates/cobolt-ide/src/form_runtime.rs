// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Form Runtime Engine — Phase 6.
//!
//! `FormRuntime` owns the background interpreter thread that executes a form's
//! generated COBOL, plus the three channels that connect it to the UI thread:
//!
//! ```text
//! UI thread (egui)                    Interpreter thread
//! ─────────────────────────────       ──────────────────────────────────────
//! FormRuntime.send_event()  ────────► COBOL-WAIT-EVENT (blocks on recv)
//! FormRuntime.drain_state() ◄───────  COBOL-SET-PROPERTY (sends StateUpdate)
//! FormRuntime.drain_display()◄──────  DISPLAY statement (sends String)
//! ```
//!
//! The interpreter thread terminates when:
//!  - COBOL executes `STOP RUN`
//!  - The UI sends `FormEvent::quit()` (closing the form window)
//!  - The channel is dropped (UI drops `FormRuntime`)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};
use std::thread;

use cobolt_forms::Form;

// Host C — the in-process `FormRuntime` (its own `CtrlState`/`CtrlMeta`, the
// `run-form-ipc` child pump, and the viewport host in app.rs) — was retired by
// spec 042 R4. Run Form executes in the external `rcrun run-form` process
// hosting the shared `cobolt-form-host` window; the control-state type it uses
// is the ONE shared `cobolt_form_host::state::CtrlState` (042 R2).

// ── Helpers ───────────────────────────────────────────────────────────────────
// (The `_Binding*` seed builders that lived here moved to
// `cobolt_form_host::seeding` with the rest of object seeding — 042 R20.)

/// Flatten nested control tree into a flat Vec (pre-order).
fn collect_controls(controls: &[cobolt_forms::Control]) -> Vec<&cobolt_forms::Control> {
    let mut out = Vec::new();
    for c in controls {
        collect_rec(c, &mut out);
    }
    out
}

fn collect_rec<'a>(ctrl: &'a cobolt_forms::Control, out: &mut Vec<&'a cobolt_forms::Control>) {
    out.push(ctrl);
    for child in &ctrl.children {
        collect_rec(child, out);
    }
}

// ── ExternalFormRun ───────────────────────────────────────────────────────────

/// A form running as a fully external `rcrun run-form` process: its own window,
/// its own event loop, its own interpreter. The IDE only spawns it, pipes its
/// stdout into the Output pane, and can kill it — the IDE render loop stays
/// idle while the form runs (and what runs is exactly what `rcrun build`
/// ships: same renderer, same interpreter, separate process).
pub struct ExternalFormRun {
    /// Path to the `.cfrm` file (identifies which form is running).
    pub form_path: PathBuf,
    /// The form's id/name — for Output pane labels.
    pub form_name: String,
    /// True when spawned with `--debug`: the IDE debugger controls the
    /// process over stdin (`@DBG` JSON commands) / stdout (`@DBG` events).
    pub debug: bool,
    child: Child,
    /// Kept only in debug mode, for sending `@DBG` command lines.
    child_stdin: Option<Mutex<ChildStdin>>,
    stdout_rx: Receiver<String>,
    stderr_buf: Arc<Mutex<Vec<String>>>,
    exit_status: Option<std::process::ExitStatus>,
}

/// Diagnostics forwarded to a `rcrun run-form` child, derived from the IDE's
/// debug settings at launch. `env` comes from
/// [`DebugSettings::child_env`](crate::debug_settings::DebugSettings::child_env)
/// — one pair per switch the child understands; `dump_project` is `Some(name)`
/// whenever ANY switch is on, which triggers the per-control diagnostics dump at
/// `/tmp/<name>_diagnostics_dump.log`.
#[derive(Clone, Default)]
pub struct RunDiagnostics {
    pub env: Vec<(&'static str, String)>,
    pub dump_project: Option<String>,
}

/// The bundled `rcrun` sitting next to the running executable.
///
/// `EXE_SUFFIX` is empty on Unix and `".exe"` on Windows, where the sibling is
/// `rcrun.exe`: spelling the name without it missed the bundled binary entirely,
/// so Run Form fell back to a bare PATH lookup and only worked if the developer
/// happened to have rcrun on PATH. The rest of the tree already handles both
/// names (see `project_model::…` and the compiler's `rcrun_name`).
fn sibling_rcrun(exe: &std::path::Path) -> PathBuf {
    exe.with_file_name(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

/// Window-effect spawn args (spec 038, plan D2): the IDE resolves project
/// settings × the form's opt-out × the kill-switch BEFORE spawning, so the
/// child stays project-file-agnostic. `entrance`/`exit` are
/// `FxSpec::format()` triples (`id:ms:easing`).
pub struct FormFxArgs {
    pub entrance: String,
    pub exit: String,
    pub restore: bool,
}

/// Resolve the effect spawn args from project settings × the form's opt-out
/// × the machine kill-switch (038 R3/R14, plan D2). `None` when nothing
/// would play — the child then receives no `--fx-*` args at all.
pub fn resolve_fx_args(
    project: Option<&crate::project_model::CoboltProject>,
    form_effects_on: bool,
    kill_switch: bool,
) -> Option<FormFxArgs> {
    let p = project?;
    if !form_effects_on || kill_switch {
        return None;
    }
    let entrance = p.entrance_fx();
    let exit = p.exit_fx();
    if !entrance.is_active() && !exit.is_active() {
        return None;
    }
    Some(FormFxArgs {
        entrance: entrance.format(),
        exit: exit.format(),
        restore: p.forms.entrance_on_restore,
    })
}

/// `rcrun run-form`'s env var name for a resolved Google Maps API key (spec
/// 039 T12/R23) — kept in sync with `cobolt-cli/src/form_gui.rs`'s constant
/// of the same name, since the two crates don't share this module.
pub const GOOGLE_MAPS_API_KEY_ENV: &str = "COBOLT_GOOGLE_MAPS_API_KEY";

/// `Some((env var, key))` when `form` has at least one Maps control and the
/// project has a Google Maps API key configured — the secret the IDE passes
/// to the `rcrun run-form` child's environment, never to disk.
pub fn resolve_maps_api_key_secret(
    form: &Form,
    llm: &crate::llm::LlmConfig,
) -> Option<(&'static str, String)> {
    let key = llm.api_keys.get(crate::llm::GOOGLE_MAPS_API_KEY_SLOT)?;
    if key.trim().is_empty() {
        return None;
    }
    let has_maps = collect_controls(&form.controls)
        .iter()
        .any(|c| c.control_type == cobolt_forms::ControlType::Maps);
    has_maps.then(|| (GOOGLE_MAPS_API_KEY_ENV, key.clone()))
}

/// `rcrun run-form`'s env var name for a resolved Google Custom Search API
/// key (spec 039 T15/R30) — kept in sync with `cobolt-cli/src/form_gui.rs`'s
/// constant of the same name.
pub const GOOGLE_SEARCH_API_KEY_ENV: &str = "COBOLT_GOOGLE_SEARCH_API_KEY";

/// `Some((env var, key))` when `form` has at least one WebSearch control and
/// the project has a Google Custom Search API key configured. Mirrors
/// `resolve_maps_api_key_secret` above.
pub fn resolve_search_api_key_secret(
    form: &Form,
    llm: &crate::llm::LlmConfig,
) -> Option<(&'static str, String)> {
    let key = llm
        .api_keys
        .get(crate::llm::GOOGLE_CUSTOM_SEARCH_API_KEY_SLOT)?;
    if key.trim().is_empty() {
        return None;
    }
    let has_web_search = collect_controls(&form.controls)
        .iter()
        .any(|c| c.control_type == cobolt_forms::ControlType::WebSearch);
    has_web_search.then(|| (GOOGLE_SEARCH_API_KEY_ENV, key.clone()))
}

impl ExternalFormRun {
    /// Spawn `rcrun run-form <cfrm> <cbl>`. Looks for `rcrun` next to the
    /// current executable first (bundle + target/debug layouts), then in PATH.
    pub fn spawn(
        form_path: PathBuf,
        form_name: String,
        cbl_path: &std::path::Path,
        theme_default: Option<&str>,
        project_icon: Option<&std::path::Path>,
        debug: bool,
        diagnostics: &RunDiagnostics,
        fx: Option<&FormFxArgs>,
        secrets: &[(&'static str, String)],
    ) -> Result<Self, String> {
        let exe = std::env::current_exe().map_err(|e| format!("failed to get current exe: {e}"))?;
        let rcrun_path = sibling_rcrun(&exe);

        let spawn_with = |program: &std::path::Path| {
            let mut cmd = Command::new(program);
            cmd.arg("run-form").arg(&form_path).arg(cbl_path);
            if let Some(id) = theme_default {
                cmd.arg("--theme-default").arg(id);
            }
            if let Some(icon) = project_icon {
                cmd.arg("--icon").arg(icon);
            }
            // 038 — window effects, already resolved by the IDE.
            if let Some(fx) = fx {
                cmd.arg("--fx-entrance").arg(&fx.entrance);
                cmd.arg("--fx-exit").arg(&fx.exit);
                if fx.restore {
                    cmd.arg("--fx-restore");
                }
            }
            if debug {
                cmd.arg("--debug");
            }
            // Drive the child's diagnostics from the IDE's debug settings. The
            // booleans are set explicitly (including "0") so the setting is
            // authoritative over any value inherited from the IDE's own env.
            for (name, value) in &diagnostics.env {
                cmd.env(name, value);
            }
            // Credentials resolved IDE-side (e.g. the Maps API key, spec 039
            // T12/R23) — never written to the `.cfrm`/`.cbl`/project file, so
            // they only ever exist in this child's environment.
            for (name, value) in secrets {
                cmd.env(name, value);
            }
            // When ANY diagnostic is on, ask the child to write the per-control
            // diagnostics dump; pass the project name so it lands at
            // /tmp/<project>_diagnostics_dump.log.
            if let Some(project) = diagnostics.dump_project.as_deref() {
                cmd.arg("--diagnostics-dump").arg(project);
            }
            // stdin carries `@DBG` command lines in debug mode only.
            cmd.stdin(if debug { Stdio::piped() } else { Stdio::null() })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        };
        let mut child = spawn_with(&rcrun_path)
            .or_else(|_| spawn_with(std::path::Path::new("rcrun")))
            .map_err(|e| {
                format!(
                    "failed to spawn rcrun run-form: {e}. Make sure rcrun is next to the \
                     IDE executable or in PATH."
                )
            })?;
        let child_stdin = child.stdin.take().map(Mutex::new);

        // stdout → line channel (drained into the Output pane each frame).
        let (out_tx, stdout_rx) = mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if out_tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }

        // stderr → buffered (surfaced in a modal if the process exits non-zero).
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        if let Some(stderr) = child.stderr.take() {
            let buf = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                use std::io::BufRead;
                let reader = std::io::BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut b) = buf.lock() {
                        b.push(line);
                    }
                }
            });
        }

        Ok(Self {
            form_path,
            form_name,
            debug,
            child,
            child_stdin,
            stdout_rx,
            stderr_buf,
            exit_status: None,
        })
    }

    /// Send a debug command to the process as an `@DBG <json>` stdin line
    /// (debug mode only; silently ignored otherwise).
    pub fn send_debug(&self, cmd: &cobolt_runtime::RemoteDebugCmd) {
        let Some(stdin) = &self.child_stdin else {
            return;
        };
        let Ok(json) = serde_json::to_string(cmd) else {
            return;
        };
        if let Ok(mut guard) = stdin.lock() {
            let _ = writeln!(guard, "@DBG {json}");
            let _ = guard.flush();
        }
    }

    /// Drain DISPLAY / stdout lines produced since the last call.
    pub fn drain_output(&self) -> Vec<String> {
        self.stdout_rx.try_iter().collect()
    }

    /// OS process id of the `rcrun run-form` child — for the Run-Form
    /// Inspector's per-form CPU chart.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Poll the child: `true` while the process is alive. Caches the exit
    /// status once the process ends.
    pub fn is_running(&mut self) -> bool {
        if self.exit_status.is_some() {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exit_status = Some(status);
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// If the process exited with a failure, return its stderr as one message.
    pub fn take_exit_error(&mut self) -> Option<String> {
        let failed = self.exit_status.map(|s| !s.success()).unwrap_or(false);
        if !failed {
            return None;
        }
        let lines = self
            .stderr_buf
            .lock()
            .map(|b| b.clone())
            .unwrap_or_default();
        // Warnings alone don't constitute the error message; but on a failed
        // exit everything stderr said is the best diagnostic we have.
        Some(if lines.is_empty() {
            format!("Form '{}' exited with an error.", self.form_name)
        } else {
            lines.join("\n")
        })
    }

    /// Kill the external process (Stop button / window replaced by a re-run).
    pub fn stop(&mut self) {
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            if let Ok(status) = self.child.wait() {
                self.exit_status = Some(status);
            }
        }
    }
}

impl Drop for ExternalFormRun {
    fn drop(&mut self) {
        // Never leave an orphaned form window running after the IDE exits.
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// ── A launched BUILT binary (spec 041 T13 run path) ──────────────────────────

/// A compiled application started by Run, tracked for its whole life.
///
/// This exists because `launch_built_binary` used to `spawn()` and drop the
/// `Child` on the floor: no stderr, no exit detection, stdio inherited into the
/// IDE where nobody can see it. A built program that died at startup left the
/// IDE saying "starting the built program" with a green semaphore — the exact
/// "nothing anywhere" the operator reported, undiagnosable because the evidence
/// was discarded at the moment it was produced. Same shape as
/// [`ExternalFormRun`], minus the debug stdin.
pub struct BuiltAppRun {
    /// The form whose Run started this (tree semaphore + replace-on-re-Run).
    pub form_path: PathBuf,
    /// Display name for Output-panel lines.
    pub name: String,
    child: Child,
    /// Live stdout+stderr lines, merged (stderr prefixed) — drained per frame.
    output_rx: mpsc::Receiver<String>,
    /// stderr kept separately too, for the exit report.
    stderr_buf: Arc<Mutex<Vec<String>>>,
    exit_status: Option<std::process::ExitStatus>,
}

impl BuiltAppRun {
    /// Spawn `binary` with both pipes captured and reader threads attached.
    pub fn spawn(
        binary: &Path,
        form_path: PathBuf,
        envs: Vec<(&'static str, String)>,
    ) -> std::io::Result<Self> {
        let mut child = Command::new(binary)
            .current_dir(binary.parent().unwrap_or(Path::new(".")))
            .envs(envs)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let (tx, output_rx) = mpsc::channel::<String>();
        if let Some(stdout) = child.stdout.take() {
            let tx = tx.clone();
            thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        let stderr_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        if let Some(stderr) = child.stderr.take() {
            let buf = Arc::clone(&stderr_buf);
            thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Ok(mut b) = buf.lock() {
                        b.push(line.clone());
                    }
                    if tx.send(format!("⚠ {line}")).is_err() {
                        break;
                    }
                }
            });
        }

        let name = binary
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "built program".to_owned());
        Ok(Self {
            form_path,
            name,
            child,
            output_rx,
            stderr_buf,
            exit_status: None,
        })
    }

    /// OS process id, reported so the operator can SEE something started.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Everything the program printed since the last call.
    pub fn drain_output(&self) -> Vec<String> {
        self.output_rx.try_iter().collect()
    }

    /// `true` while the process is alive; caches the exit status once it ends.
    pub fn is_running(&mut self) -> bool {
        if self.exit_status.is_some() {
            return false;
        }
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.exit_status = Some(status);
                false
            }
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// After exit: `None` for success, `Some(report)` for a failure — the exit
    /// code plus whatever stderr said, which is the best diagnostic there is.
    pub fn exit_error(&self) -> Option<String> {
        let status = self.exit_status?;
        if status.success() {
            return None;
        }
        let code = match status.code() {
            Some(c) => c.to_string(),
            None => {
                // Name the signal — "killed by signal" alone cost a diagnostic
                // round-trip when the answer (SIGKILL: macOS code-signing kill
                // after an in-place binary overwrite) was in the number.
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    match status.signal() {
                        Some(9) => "killed by SIGKILL (9) — if this happened right \
                                    after a build, the binary was overwritten in \
                                    place (fixed in 1.60.19: rebuild)"
                            .to_owned(),
                        Some(n) => format!("killed by signal {n}"),
                        None => "killed by signal".to_owned(),
                    }
                }
                #[cfg(not(unix))]
                {
                    "killed by signal".to_owned()
                }
            }
        };
        let lines = self.stderr_buf.lock().map(|b| b.clone()).unwrap_or_default();
        Some(if lines.is_empty() {
            format!("exit {code} — the program printed nothing on stderr")
        } else {
            format!("exit {code}\n{}", lines.join("\n"))
        })
    }

    /// Kill the process (a re-Run replaces the previous instance).
    pub fn stop(&mut self) {
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            if let Ok(status) = self.child.wait() {
                self.exit_status = Some(status);
            }
        }
    }
}

impl Drop for BuiltAppRun {
    fn drop(&mut self) {
        // Never leave an orphaned built application running after the IDE exits.
        if self.exit_status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod built_app_run_tests {
    use super::BuiltAppRun;
    use std::path::PathBuf;

    fn wait_exit(run: &mut BuiltAppRun) {
        for _ in 0..200 {
            if !run.is_running() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("the child never exited");
    }

    /// The whole reason the struct exists: a child that fails at startup must
    /// hand back its exit code AND its stderr, not vanish.
    #[test]
    fn a_failing_child_reports_exit_code_and_stderr() {
        let mut run = BuiltAppRun::spawn(
            std::path::Path::new("/bin/sh"),
            PathBuf::from("form.cfrm"),
            vec![],
        )
        .expect("spawn sh");
        // No args — sh on stdin=null exits 0 immediately; so use a scripted one.
        run.stop();

        let dir = std::env::temp_dir().join(format!("prc-builtrun-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fails.sh");
        std::fs::write(&script, "#!/bin/sh\necho out-line\necho err-line 1>&2\nexit 3\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let mut run =
            BuiltAppRun::spawn(&script, PathBuf::from("form.cfrm"), vec![]).expect("spawn script");
        assert!(run.pid() > 0);
        wait_exit(&mut run);

        let err = run.exit_error().expect("exit 3 must be reported");
        assert!(err.contains("exit 3"), "code missing: {err}");
        assert!(err.contains("err-line"), "stderr missing: {err}");

        // Output lines were streamed too — both streams, stderr marked.
        let all: Vec<String> = run.drain_output();
        assert!(all.iter().any(|l| l == "out-line"), "{all:?}");
        assert!(all.iter().any(|l| l.contains("err-line")), "{all:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A clean exit reports nothing — silence is only correct when true.
    #[test]
    fn a_clean_exit_reports_no_error() {
        let mut run = BuiltAppRun::spawn(
            std::path::Path::new("/usr/bin/true"),
            PathBuf::from("form.cfrm"),
            vec![],
        )
        .expect("spawn true");
        wait_exit(&mut run);
        assert!(run.exit_error().is_none());
    }
}

#[cfg(test)]
mod form_codegen_roundtrip_tests {
    // The ONE shared control-state type (spec 042 R2) — this test pins the
    // designed-vs-runtime caption merge on the type every host actually uses.
    use cobolt_form_host::state::CtrlState;
    use cobolt_forms::render::merge_props;
    use cobolt_forms::PropValue;
    use cobolt_forms::{Control, ControlType, EventBinding, Form};
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::{parse, Severity as PSev};
    use cobolt_runtime::{FormEvent, Interpreter};
    // Spec 044 R20 — the service wrapper allows registered External Crates.
    use crate::external_crates_service::analyze_project as analyze;
    use std::sync::mpsc;
    use std::thread;

    /// 038 T5 — fx spawn args resolve from project × form opt-out ×
    /// kill-switch, and are omitted entirely when nothing would play.
    #[test]
    fn fx_spawn_args_resolution_matrix() {
        let project = crate::project_model::CoboltProject::new("Fx", "src/main.cbl");
        // New project ⇒ matrix-rain entrance ⇒ args present.
        let args = super::resolve_fx_args(Some(&project), true, false)
            .expect("effects on, kill-switch off ⇒ Some");
        assert!(args.entrance.starts_with("matrix-rain:2000:"), "{}", args.entrance);
        assert!(args.exit.starts_with("none:"), "{}", args.exit);
        assert!(!args.restore);
        println!("fx args: entrance={} exit={} restore={}", args.entrance, args.exit, args.restore);

        // Form opted out ⇒ None; kill-switch on ⇒ None; no project ⇒ None.
        assert!(super::resolve_fx_args(Some(&project), false, false).is_none());
        assert!(super::resolve_fx_args(Some(&project), true, true).is_none());
        assert!(super::resolve_fx_args(None, true, false).is_none());

        // Both effects inactive ⇒ None even with everything enabled.
        let mut quiet = crate::project_model::CoboltProject::new("Q", "src/main.cbl");
        quiet.forms.entrance_effect.clear();
        quiet.forms.exit_effect.clear();
        assert!(super::resolve_fx_args(Some(&quiet), true, false).is_none());
        println!("fx args suppressed: opt-out, kill-switch, no-project, all-none");
    }

    #[test]
    fn generated_form_with_handler_parses_and_dispatches() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\
       LINKAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           MOVE 1 TO COBOL-QUIT."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);

        assert!(
            src.contains("CALL \"BUTTON-1--ONCLICK\""),
            "missing dispatch:\n{src}"
        );
        // Spec 009 R4: handlers are generated `IS COMMON PROGRAM`.
        assert!(
            src.contains("PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM."),
            "missing handler program"
        );
        assert!(src.contains("MOVE 1 TO COBOL-QUIT"), "missing handler body");

        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(
            perrs.is_empty(),
            "parse errors in generated form:\n{perrs:#?}\n--- src ---\n{src}"
        );

        let program = pr.program.expect("no program recovered");
        let sem = analyze(&program);
        let serrs: Vec<_> = sem.errors().collect();
        assert!(
            serrs.is_empty(),
            "semantic errors in generated form:\n{serrs:#?}"
        );
    }

    /// The generated IndexedFile helpers live at OUTER-program scope, so a
    /// handler — a nested program — cannot `PERFORM` them. This is the shape
    /// the Knowledge Base used to document, and it stopped compiling in 1.55.6
    /// when the analyzer began recursing into contained programs. Locked in as
    /// a test so the constraint is visible: whoever makes IndexedFile reachable
    /// from a handler (nested COMMON programs, or `::` methods) will see this
    /// fail and can retire it deliberately.
    #[test]
    fn indexedfile_paragraphs_are_not_reachable_from_a_handler() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut ixf = Control::new("IXF-1", ControlType::IndexedFile, 0, 0);
        ixf.set_prop("IndexedFile", PropValue::String("CUSTOMERS".into()));
        form.controls.push(ixf);

        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           PERFORM IXF-1-OPEN."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let program = pr.program.expect("no program");
        let sem = analyze(&program);
        let serrs: Vec<String> = sem.errors().map(|d| d.message.clone()).collect();
        assert!(
            serrs.iter().any(|m| m.contains("IXF-1-OPEN")
                && m.contains("not a paragraph or section of this program")),
            "expected the cross-program PERFORM to be rejected, got {serrs:#?}"
        );
    }

    #[test]
    fn runtime_uppercase_property_update_overwrites_designed_caption() {
        let mut label = Control::new("label-5", ControlType::Label, 10, 10);
        label.set_prop("Caption", PropValue::String("old".into()));

        let mut state = CtrlState::from_control(&label);
        state.set("CAPTION", "42".into());

        let merged = merge_props(&label, state.props.iter());
        assert_eq!(merged.get_prop("Caption").unwrap().as_str(), "42");
    }

    #[test]
    fn generated_form_click_event_runs_handler_in_live_runtime() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\
       LINKAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           DISPLAY \"CLICKED\"."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(
            perrs.is_empty(),
            "parse errors in generated form:\n{perrs:#?}\n--- src ---\n{src}"
        );

        let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let program = pr.program.expect("no program recovered");
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            let _ = interp.run();
        });

        event_tx.send(FormEvent::click("Button-1")).unwrap();
        event_tx.send(FormEvent::quit()).unwrap();
        handle.join().expect("interpreter thread panicked");

        let display: Vec<String> = display_rx.try_iter().collect();
        assert!(
            display.iter().any(|line| line.trim() == "CLICKED"),
            "onClick handler did not run; DISPLAY output was {display:?}"
        );
    }

    /// The handler pattern the Knowledge Base recommends, end to end: declare a
    /// paragraph, `PERFORM` it, and end the main flow with `GOBACK.` before it.
    ///
    /// Both halves of this were broken until 1.55.7. `PERFORM` resolved against
    /// the OUTER program, so a handler could not reach a paragraph it declared
    /// itself; and `GOBACK` was not a keyword, so the terminator parsed as yet
    /// another paragraph and control fell through and ran STEP again.
    #[test]
    fn handler_performs_its_own_paragraph_and_goback_stops_the_fall_through() {
        let mut form = Form::new("MAIN-FORM", "Demo", 640, 480);
        let mut btn = Control::new("Button-1", ControlType::Button, 10, 10);
        let mut ev = EventBinding::for_control("Button-1", "onClick");
        ev.code = "\
       ENVIRONMENT DIVISION.\n\
       DATA DIVISION.\n\
       WORKING-STORAGE SECTION.\n\n\
       PROCEDURE DIVISION.\n\
           PERFORM SHOW-STEP.\n\
           GOBACK.\n\
       SHOW-STEP.\n\
           DISPLAY \"STEP\"."
            .into();
        btn.events.push(ev);
        form.controls.push(btn);

        let src = cobolt_codegen::generate(&form);
        let pr = parse(tokenize(&src, SourceFormat::Free));
        let perrs: Vec<_> = pr
            .diagnostics
            .iter()
            .filter(|d| d.severity == PSev::Error)
            .collect();
        assert!(perrs.is_empty(), "parse errors:\n{perrs:#?}\n--- src ---\n{src}");

        let program = pr.program.expect("no program recovered");
        let sem = analyze(&program);
        let serrs: Vec<_> = sem.errors().collect();
        assert!(
            serrs.is_empty(),
            "a handler PERFORMing its own paragraph must analyze cleanly:\n{serrs:#?}"
        );

        let (event_tx, event_rx) = mpsc::channel::<FormEvent>();
        let (state_tx, _state_rx) = mpsc::channel();
        let (display_tx, display_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let mut interp =
                Interpreter::new_with_channels(program, event_rx, state_tx, display_tx);
            let _ = interp.run();
        });

        event_tx.send(FormEvent::click("Button-1")).unwrap();
        event_tx.send(FormEvent::quit()).unwrap();
        handle.join().expect("interpreter thread panicked");

        let steps = display_rx
            .try_iter()
            .filter(|line| line.trim() == "STEP")
            .count();
        assert_eq!(
            steps, 1,
            "SHOW-STEP must run exactly once — 0 means PERFORM could not reach \
             the handler's own paragraph, 2 means GOBACK failed to stop the \
             fall-through into it"
        );
    }
}

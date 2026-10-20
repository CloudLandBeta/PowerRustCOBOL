// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built-ins of the backend contract (WASM-014), through the interpreter:
//! on the backend, `WS-FAIL`, `WS-PROGRESS`, `WS-CANCELLED`, `WS-PAUSE`,
//! `WS-EMIT`, `WS-FILE-PATH` and `WS-FILE-PUBLISH`; on the frontend, the
//! idempotency key a repeat reuses, the observer that hears progress and
//! warnings, the cancel handle, `WS-WARNING`, `WS-UPLOAD` and `WS-DOWNLOAD`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::{parse, Severity};
use cobolt_runtime::interpreter::remote::{BackendHooks, RemoteEvent};
use cobolt_runtime::Interpreter;
use cobolt_wire::call::{BackendFiles, CallObserver, CallReply, CallRequest, DownloadRequest, DownloadResult, Transport, UploadRequest, UploadResult};
use cobolt_wire::error::{codes, CallError, ErrorClass};

fn interp(src: &str) -> (Interpreter, mpsc::Receiver<String>) {
    let result = parse(tokenize(src, SourceFormat::Free));
    assert!(result.diagnostics.iter().all(|d| d.severity != Severity::Error), "parse errors: {:?}", result.diagnostics);
    let (_e, event_rx) = mpsc::channel();
    let (state_tx, _s) = mpsc::channel();
    let (display_tx, display_rx) = mpsc::channel();
    (Interpreter::new_with_channels(result.program.expect("program"), event_rx, state_tx, display_tx), display_rx)
}

fn hooks(cancel: Arc<AtomicBool>) -> BackendHooks {
    BackendHooks { progress: None, emit: None, files: None, cancel_requested: cancel }
}

fn shown(rx: &mpsc::Receiver<String>) -> Vec<String> {
    rx.try_iter().map(|s| s.trim_end().to_string()).collect()
}

fn program(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. EXPORTED.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-FLAG   PIC X.\n       01 WS-PATH   PIC X(200).\n       01 WS-ST     PIC X(10).\n       01 WS-ID     PIC X(32).\n       01 TICK.\n          05 N      PIC 9(4).\n          05 WHO    PIC X(6).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n{body}\n           STOP RUN.\n"
    )
}

// ── the backend's side ───────────────────────────────────────────────────────

#[test]
fn ws_fail_ends_the_call_with_the_programs_own_failure_and_nothing_after_it_runs() {
    let (mut i, rx) = interp(&program("           DISPLAY \"before\"\n           COBOL::\"WS-FAIL\" ( 4201 \"card declined\" '{\"retry\":false}' )\n           DISPLAY \"after\""));
    i.set_backend_hooks(hooks(Arc::new(AtomicBool::new(false))));
    assert!(i.run().is_ok(), "the run ends cleanly; the failure is read off the interpreter");
    let f = i.take_app_failure().expect("a failure was reported");
    assert_eq!((f.code, f.message.as_str(), f.detail.as_deref()), (4201, "card declined", Some("{\"retry\":false}")));
    assert_eq!(shown(&rx), vec!["before"], "nothing after WS-FAIL ran");
    assert!(i.take_app_failure().is_none(), "it is read once");
}

#[test]
fn ws_fail_refuses_a_platform_code_a_zero_and_a_run_that_is_not_serving_a_call() {
    for code in ["0", "8000", "9001", "70000"] {
        let (mut i, _rx) = interp(&program(&format!("           COBOL::\"WS-FAIL\" ( {code} \"x\" )")));
        i.set_backend_hooks(hooks(Arc::new(AtomicBool::new(false))));
        let e = i.run().expect_err(code).to_string();
        assert!(e.contains("belong to the platform") || e.contains("must be 1 to 7999"), "{code}: {e}");
    }
    let (mut i, _rx) = interp(&program("           COBOL::\"WS-FAIL\" ( 5 \"x\" )"));
    let e = i.run().expect_err("not serving a call").to_string();
    assert!(e.contains("belongs to a program the backend exports"), "{e}");
}

#[test]
fn ws_progress_reports_clamped_values_and_is_the_safe_point_where_a_cancel_ends_the_call() {
    let program_text = program(
        "           COBOL::\"WS-CANCELLED\" ( WS-FLAG )\n           DISPLAY \"cancelled=\" WS-FLAG\n           COBOL::\"WS-PROGRESS\" ( 10 \"reading\" )\n           COBOL::\"WS-PROGRESS\" ( 250 \"clamped\" )\n           COBOL::\"WS-CANCELLED\" ( WS-FLAG )\n           DISPLAY \"cancelled=\" WS-FLAG\n           COBOL::\"WS-PROGRESS\" ( 50 \"after the cancel\" )\n           DISPLAY \"not reached\"",
    );
    // Nobody asks to cancel: every report goes out, values clamped to 0..100, the program runs on.
    let seen: Arc<Mutex<Vec<(u8, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let (mut i, rx) = interp(&program_text);
    let s2 = seen.clone();
    let mut h = hooks(Arc::new(AtomicBool::new(false)));
    h.progress = Some(Box::new(move |p, m| s2.lock().unwrap().push((p, m.to_string()))));
    i.set_backend_hooks(h);
    i.run().expect("runs to its end");
    assert!(!i.remote_cancelled());
    assert_eq!(*seen.lock().unwrap(), vec![(10, "reading".to_string()), (100, "clamped".to_string()), (50, "after the cancel".to_string())]);
    assert_eq!(shown(&rx), vec!["cancelled=N", "cancelled=N", "not reached"]);

    // The caller has asked to cancel: the program can see it, and the first report is the safe point where the call ends.
    let seen: Arc<Mutex<Vec<(u8, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let (mut i, rx) = interp(&program_text);
    let s2 = seen.clone();
    let mut h = hooks(Arc::new(AtomicBool::new(true)));
    h.progress = Some(Box::new(move |p, m| s2.lock().unwrap().push((p, m.to_string()))));
    i.set_backend_hooks(h);
    i.run().expect("a cancelled call ends cleanly");
    assert!(i.remote_cancelled());
    assert_eq!(*seen.lock().unwrap(), vec![(10, "reading".to_string())], "the report goes out, then the call ends");
    assert_eq!(shown(&rx), vec!["cancelled=Y"]);
}

#[test]
fn ws_pause_waits_and_ends_early_on_a_cancel_and_at_once_when_the_call_is_cut_off() {
    let (mut i, _rx) = interp(&program("           COBOL::\"WS-PAUSE\" ( 120 )"));
    i.set_backend_hooks(hooks(Arc::new(AtomicBool::new(false))));
    let t = Instant::now();
    i.run().unwrap();
    assert!(t.elapsed() >= Duration::from_millis(110), "{:?}", t.elapsed());

    let asked = Arc::new(AtomicBool::new(true));
    let (mut i, _rx) = interp(&program("           COBOL::\"WS-PAUSE\" ( 5000 )"));
    i.set_backend_hooks(hooks(asked));
    let t = Instant::now();
    i.run().unwrap();
    assert!(t.elapsed() < Duration::from_millis(500), "a pause ends early once the caller has asked to cancel: {:?}", t.elapsed());

    let (mut i, _rx) = interp(&program("           COBOL::\"WS-PAUSE\" ( 5000 )"));
    let hard = Arc::new(AtomicBool::new(true));
    i.set_cancel_flag(hard);
    let t = Instant::now();
    let _ = i.run();
    assert!(t.elapsed() < Duration::from_millis(500), "a call that is cut off stops at once: {:?}", t.elapsed());
}

#[test]
fn ws_emit_sends_the_group_and_ends_the_program_when_nobody_listens() {
    let items: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let (mut i, rx) = interp(&program(
        "           MOVE \"clock\" TO WHO\n           PERFORM VARYING N FROM 1 BY 1 UNTIL N > 9\n               COBOL::\"WS-EMIT\" ( TICK )\n           END-PERFORM\n           DISPLAY \"finished\"",
    ));
    let sink = items.clone();
    let mut h = hooks(Arc::new(AtomicBool::new(false)));
    h.emit = Some(Box::new(move |json| {
        let mut v = sink.lock().unwrap();
        v.push(json.to_string());
        v.len() < 3 // the third item is the last anyone wants
    }));
    i.set_backend_hooks(h);
    i.run().unwrap();
    assert_eq!(*items.lock().unwrap(), vec!["{\"N\":1,\"WHO\":\"clock\"}", "{\"N\":2,\"WHO\":\"clock\"}", "{\"N\":3,\"WHO\":\"clock\"}"]);
    assert!(i.remote_cancelled());
    assert!(shown(&rx).is_empty(), "the program did not run to its end");

    let (mut i, _rx) = interp(&program("           COBOL::\"WS-EMIT\" ( TICK )"));
    i.set_backend_hooks(hooks(Arc::new(AtomicBool::new(false))));
    assert!(i.run().unwrap_err().to_string().contains("exported as a stream"));
}

struct Files;

impl BackendFiles for Files {
    fn resolve(&self, id: &str) -> Option<std::path::PathBuf> {
        (id == "0123456789abcdef0123456789abcdef").then(|| std::path::PathBuf::from("/data/files/0123456789abcdef0123456789abcdef"))
    }
    fn publish(&self, path: &std::path::Path) -> Result<String, String> {
        if path.to_string_lossy().ends_with("report.txt") {
            Ok("fedcba9876543210fedcba9876543210".into())
        } else {
            Err("no such file".into())
        }
    }
}

#[test]
fn the_file_built_ins_answer_with_a_status_word_and_never_reveal_a_file_that_is_not_the_callers() {
    let src = program(
        "           MOVE \"0123456789abcdef0123456789abcdef\" TO WS-ID\n           COBOL::\"WS-FILE-PATH\" ( WS-ID WS-PATH WS-ST )\n           DISPLAY WS-ST \"|\" WS-PATH\n           MOVE \"ffffffffffffffffffffffffffffffff\" TO WS-ID\n           COBOL::\"WS-FILE-PATH\" ( WS-ID WS-PATH WS-ST )\n           DISPLAY WS-ST \"|\" WS-PATH\n           COBOL::\"WS-FILE-PUBLISH\" ( \"/data/out/report.txt\" WS-ID WS-ST )\n           DISPLAY WS-ST \"|\" WS-ID\n           COBOL::\"WS-FILE-PUBLISH\" ( \"/data/out/none.txt\" WS-ID WS-ST )\n           DISPLAY WS-ST \"|\" WS-ID",
    );
    let (mut i, rx) = interp(&src);
    let mut h = hooks(Arc::new(AtomicBool::new(false)));
    h.files = Some(Arc::new(Files));
    i.set_backend_hooks(h);
    i.run().unwrap();
    let out = shown(&rx);
    assert_eq!(out[0], "OK        |/data/files/0123456789abcdef0123456789abcdef");
    assert_eq!(out[1].trim_end(), "NOFILE    |");
    assert_eq!(out[2], "OK        |fedcba9876543210fedcba9876543210");
    assert_eq!(out[3].trim_end(), "NOFILE    |");

    let (mut i, rx) = interp(&src);
    i.set_backend_hooks(hooks(Arc::new(AtomicBool::new(false))));
    i.run().unwrap();
    assert!(shown(&rx).iter().all(|l| l.starts_with("NOBACKEND")), "no file store: NOBACKEND");
}

// ── the frontend's side ──────────────────────────────────────────────────────

type Script = Box<dyn FnMut(&CallRequest, &mut dyn CallObserver) -> Result<CallReply, CallError> + Send>;

struct Scripted {
    script: Script,
    keys: Arc<Mutex<Vec<Option<String>>>>,
    uploads: Arc<Mutex<Vec<UploadRequest>>>,
    downloads: Arc<Mutex<Vec<DownloadRequest>>>,
    upload_result: Arc<Mutex<Result<UploadResult, CallError>>>,
    download_result: Arc<Mutex<Result<DownloadResult, CallError>>>,
}

impl Transport for Scripted {
    fn call(&mut self, r: &CallRequest) -> Result<CallReply, CallError> {
        self.call_observed(r, &mut cobolt_wire::call::NoObserver)
    }
    fn call_observed(&mut self, r: &CallRequest, o: &mut dyn CallObserver) -> Result<CallReply, CallError> {
        self.keys.lock().unwrap().push(r.idempotency_key.clone());
        (self.script)(r, o)
    }
    fn upload(&mut self, r: &UploadRequest, _o: &mut dyn CallObserver) -> Result<UploadResult, CallError> {
        self.uploads.lock().unwrap().push(r.clone());
        self.upload_result.lock().unwrap().clone()
    }
    fn download(&mut self, r: &DownloadRequest, _o: &mut dyn CallObserver) -> Result<DownloadResult, CallError> {
        self.downloads.lock().unwrap().push(r.clone());
        self.download_result.lock().unwrap().clone()
    }
}

struct Rig {
    keys: Arc<Mutex<Vec<Option<String>>>>,
    uploads: Arc<Mutex<Vec<UploadRequest>>>,
    downloads: Arc<Mutex<Vec<DownloadRequest>>>,
    upload_result: Arc<Mutex<Result<UploadResult, CallError>>>,
    download_result: Arc<Mutex<Result<DownloadResult, CallError>>>,
}

fn scripted(i: &mut Interpreter, script: Script) -> Rig {
    let r = Rig {
        keys: Arc::new(Mutex::new(Vec::new())),
        uploads: Arc::new(Mutex::new(Vec::new())),
        downloads: Arc::new(Mutex::new(Vec::new())),
        upload_result: Arc::new(Mutex::new(Ok(UploadResult { file: "0123456789abcdef0123456789abcdef".into(), size: 5 }))),
        download_result: Arc::new(Mutex::new(Ok(DownloadResult { size: 5 }))),
    };
    i.set_remote_transport(Box::new(Scripted {
        script,
        keys: r.keys.clone(),
        uploads: r.uploads.clone(),
        downloads: r.downloads.clone(),
        upload_result: r.upload_result.clone(),
        download_result: r.download_result.clone(),
    }));
    r
}

fn caller(body: &str) -> String {
    format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CALLER.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 ORDER-REC.\n          05 AMOUNT PIC 9(4).\n       01 OTHER-REC.\n          05 AMOUNT2 PIC 9(4).\n       01 WS-W   PIC X(80).\n       01 WS-ID  PIC X(32).\n       01 WS-ST  PIC X(10).\n       PROCEDURE DIVISION.\n       MAIN-PARA.\n           MOVE 5 TO AMOUNT\n           MOVE 9 TO AMOUNT2\n{body}\n           STOP RUN.\n"
    )
}

const PAY: &str = "           CALL \"CHARGE\" WITH WEBSOCKET LINKAGE USING BY REFERENCE ORDER-REC ON EXCEPTION DISPLAY \"failed\" NOT ON EXCEPTION DISPLAY \"ok\" END-CALL";
const PAY_OTHER: &str = "           CALL \"CHARGE\" WITH WEBSOCKET LINKAGE USING BY REFERENCE OTHER-REC ON EXCEPTION DISPLAY \"failed\" NOT ON EXCEPTION DISPLAY \"ok\" END-CALL";

#[test]
fn a_call_made_again_right_after_one_the_backend_may_have_run_reuses_its_key() {
    let (mut i, rx) = interp(&caller(&format!("{PAY}\n{PAY}\n{PAY_OTHER}\n{PAY}")));
    let step = Arc::new(Mutex::new(0));
    let rig = scripted(
        &mut i,
        Box::new(move |_r, _o| {
            let mut n = step.lock().unwrap();
            *n += 1;
            match *n {
                // the first: sent, then the line was lost - it may have run
                1 => Err(CallError::transport(codes::CLOSED, "lost").may_have_run(true)),
                // the repeat gets through
                2 => Ok(CallReply { args: vec![Some("{\"AMOUNT\":5}".into())], ..CallReply::default() }),
                // a different call, and a later one (nothing to write back)
                _ => Ok(CallReply { args: vec![None], ..CallReply::default() }),
            }
        }),
    );
    i.run().unwrap();
    assert_eq!(shown(&rx), vec!["failed", "ok", "ok", "ok"]);
    let keys: Vec<String> = rig.keys.lock().unwrap().iter().map(|k| k.clone().expect("every call carries a key")).collect();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0], keys[1], "the repeat of a call that may have run carries the same key");
    assert_ne!(keys[1], keys[2], "a different call gets its own");
    assert_ne!(keys[1], keys[3], "and so does a later call that succeeded before: the key was spent");
    assert!(keys.iter().all(|k| k.len() == 32 && k.bytes().all(|b| b.is_ascii_hexdigit())), "{keys:?}");
}

#[test]
fn a_failure_that_certainly_did_not_run_is_not_remembered_so_the_next_call_is_a_new_one() {
    let (mut i, _rx) = interp(&caller(&format!("{PAY}\n{PAY}")));
    let rig = scripted(&mut i, Box::new(|_r, _o| Err(CallError::transport(codes::NO_CONNECTION, "no route").may_have_run(false))));
    i.run().unwrap();
    let keys = rig.keys.lock().unwrap().clone();
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn progress_and_warnings_reach_the_host_and_ws_warning_reads_the_last_one() {
    let events: Arc<Mutex<Vec<RemoteEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let (mut i, rx) = interp(&caller(&format!("{PAY}\n           COBOL::\"WS-WARNING\" ( WS-W )\n           DISPLAY \"warning=[\" WS-W \"]\"")));
    let e2 = events.clone();
    i.set_remote_events(Box::new(move |e| e2.lock().unwrap().push(e)));
    scripted(
        &mut i,
        Box::new(|_r, o| {
            o.progress(30, "reading");
            o.progress(100, "done");
            o.warning("CHARGE is deprecated: use CHARGE-V2");
            Ok(CallReply { args: vec![Some("{\"AMOUNT\":5}".into())], warnings: vec!["CHARGE is deprecated: use CHARGE-V2".into()], ..CallReply::default() })
        }),
    );
    i.run().unwrap();
    let ev = events.lock().unwrap().clone();
    assert_eq!(ev[0], RemoteEvent::Progress { percent: 30, message: "reading".into() });
    assert_eq!(ev[1], RemoteEvent::Progress { percent: 100, message: "done".into() });
    assert_eq!(ev[2], RemoteEvent::Warning("CHARGE is deprecated: use CHARGE-V2".into()));
    assert_eq!(ev.len(), 3, "a warning is reported once however it arrives");
    assert!(shown(&rx)[1].starts_with("warning=[CHARGE is deprecated: use CHARGE-V2 "), "the field is padded as a PIC X(80) is");
}

#[test]
fn a_host_cancels_the_pending_call_through_the_handle() {
    let (mut i, rx) = interp(&caller(PAY));
    let handle = i.remote_cancel_handle();
    scripted(
        &mut i,
        Box::new(move |_r, o| {
            assert!(!o.cancel_requested(), "the flag starts clear for each call");
            handle.store(true, Ordering::SeqCst); // the host's button
            if o.cancel_requested() {
                return Err(CallError::new(ErrorClass::Application, codes::CANCELLED, "cancelled"));
            }
            Ok(CallReply::default())
        }),
    );
    i.run().unwrap();
    assert_eq!(shown(&rx), vec!["failed"]);
    assert_eq!(i.remote_last_error().map(|e| e.code), Some(codes::CANCELLED));
}

#[test]
fn ws_upload_and_ws_download_answer_with_status_words_and_record_why_in_ws_last_error() {
    let dir = std::env::temp_dir().join(format!("prc-hooks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let local = dir.join("photo.png");
    std::fs::write(&local, b"hello").unwrap();
    let body = format!(
        "           COBOL::\"WS-UPLOAD\" ( \"docs\" \"{p}\" \"image/png\" WS-ID WS-ST )\n           DISPLAY \"up=\" WS-ST \"|\" WS-ID\n           COBOL::\"WS-UPLOAD\" ( \"docs\" \"{p}.missing\" \"image/png\" WS-ID WS-ST )\n           DISPLAY \"up=\" WS-ST\n           COBOL::\"WS-DOWNLOAD\" ( WS-ID \"{d}/back.png\" WS-ST )\n           DISPLAY \"down=\" WS-ST",
        p = local.display(),
        d = dir.display()
    );
    let (mut i, rx) = interp(&caller(&body));
    let rig = scripted(&mut i, Box::new(|_r, _o| Ok(CallReply::default())));
    i.run().unwrap();
    let out = shown(&rx);
    assert_eq!(out[0], "up=OK        |0123456789abcdef0123456789abcdef");
    assert_eq!(out[1].trim_end(), "up=NOFILE");
    assert_eq!(out[2].trim_end(), "down=OK");
    let up = rig.uploads.lock().unwrap();
    assert_eq!(up.len(), 1, "a file that is not there is never offered to the transport");
    assert_eq!((up[0].slot.as_str(), up[0].name.as_str(), up[0].mime.as_str()), ("docs", "photo.png", "image/png"));

    // A refusal and a lost line are different words; why is in WS-LAST-ERROR.
    let (mut i, rx) = interp(&caller(&format!(
        "           COBOL::\"WS-UPLOAD\" ( \"docs\" \"{p}\" \"image/png\" WS-ID WS-ST )\n           DISPLAY \"up=\" WS-ST\n           COBOL::\"WS-DOWNLOAD\" ( WS-ID \"{d}/x.png\" WS-ST )\n           DISPLAY \"down=\" WS-ST",
        p = local.display(),
        d = dir.display()
    )));
    let rig = scripted(&mut i, Box::new(|_r, _o| Ok(CallReply::default())));
    *rig.upload_result.lock().unwrap() = Err(CallError::new(ErrorClass::Validation, codes::INVALID_PAYLOAD, "the type is not allowed in this slot"));
    *rig.download_result.lock().unwrap() = Err(CallError::transport(codes::CLOSED, "cut off"));
    i.run().unwrap();
    let out = shown(&rx);
    assert_eq!(out[0].trim_end(), "up=REFUSED");
    assert_eq!(out[1].trim_end(), "down=FAILED");

    // No transport at all.
    let (mut i, rx) = interp(&caller(&format!("           COBOL::\"WS-UPLOAD\" ( \"docs\" \"{}\" \"image/png\" WS-ID WS-ST )\n           DISPLAY \"up=\" WS-ST", local.display())));
    i.run().unwrap();
    assert_eq!(shown(&rx)[0].trim_end(), "up=NOBACKEND");
    let _ = std::fs::remove_dir_all(&dir);
}

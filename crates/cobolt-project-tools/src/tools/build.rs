// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `build` — regenerate, gate on `check`, build; never hold a call longer than
//! [`DEFAULT_BUILD_WAIT`].
//!
//! `cobolt_compiler::build_project` compiles whatever generated code is on
//! disk; it does not regenerate (plan F9). The IDE regenerates before Build,
//! and so does this tool. A build takes minutes and an MCP client gives a
//! tool call about a minute (plan §8 A2), so the build runs on a worker thread
//! and a call waits at most 40 s: still running, it answers `running`, and the
//! next `build` call waits on the **same** build instead of starting another.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::root::ProjectRoot;
use crate::tools::{check, regenerate, Shared};

/// The longest one `build` call waits.
pub const DEFAULT_BUILD_WAIT: Duration = Duration::from_secs(40);

/// What a finished build produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOutcome {
    pub binary: PathBuf,
}

/// Runs one build: the compiler, or a stub in tests.
pub type Builder =
    Arc<dyn Fn(&Path, &cobolt_compiler::BuildOptions) -> Result<BuildOutcome, String> + Send + Sync>;

/// The real builder.
pub fn compiler_builder() -> Builder {
    Arc::new(|manifest, opts| {
        cobolt_compiler::build_project(manifest, opts)
            .map(|r| BuildOutcome { binary: r.binary_path })
            .map_err(|e| e.to_string())
    })
}

#[derive(Default)]
enum SlotState {
    #[default]
    Idle,
    Running {
        started: Instant,
    },
    Done {
        result: Result<BuildOutcome, String>,
        elapsed: Duration,
    },
}

/// The build in flight, shared by every handler of one server.
#[derive(Default)]
pub struct BuildSlot {
    state: Mutex<SlotState>,
    done: Condvar,
}

pub fn run<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    shared: &Arc<Shared>,
    full: bool,
) -> Result<Value, String> {
    let idle = {
        let st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
        matches!(*st, SlotState::Idle)
    };
    if idle {
        if let Some(why) = host.build_blocked() {
            return Err(why);
        }
        // Regenerate everything, then the IDE's gate: no build over an error.
        {
            let _w = shared.write_lock.lock().unwrap_or_else(|p| p.into_inner());
            regenerate::run(host, root, None)?;
        }
        let report = check::check_project(host, root)?;
        if report.errors() > 0 {
            return Ok(json!({
                "status": "refused",
                "reason": "the project has errors; fix them (see diagnostics) and build again",
                "check": report.to_json(),
            }));
        }
        start(shared, root.manifest().to_path_buf(), host.workspace_root(), full);
    }
    Ok(wait(shared, root))
}

/// Start the build on a worker unless one is already running.
fn start(shared: &Arc<Shared>, manifest: PathBuf, workspace_root: Option<PathBuf>, full: bool) {
    {
        let mut st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
        if !matches!(*st, SlotState::Idle) {
            return;
        }
        *st = SlotState::Running { started: Instant::now() };
    }
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        let opts = cobolt_compiler::BuildOptions {
            // Nothing on stdout: on `rcrun mcp` stdout is the protocol stream.
            verbose: false,
            workspace_root,
            progress: None,
            target: None,
            full,
            // Built from disk, like `rcrun build`: the files are what the
            // agent saved (a host with unsaved edits refuses before this), so
            // the forms' COBOL is regenerated from them first.
            regenerate_forms: true,
        };
        let started = Instant::now();
        let result = (shared.builder)(&manifest, &opts);
        let mut st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
        *st = SlotState::Done {
            result,
            elapsed: started.elapsed(),
        };
        shared.build.done.notify_all();
    });
}

/// Wait at most `shared.build_wait` for the build in flight, and say where it
/// stands. A finished build's result is handed out once, and the slot is free
/// again.
fn wait(shared: &Arc<Shared>, root: &ProjectRoot) -> Value {
    let deadline = Instant::now() + shared.build_wait;
    let mut st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
    loop {
        match &*st {
            SlotState::Done { .. } => {
                let SlotState::Done { result, elapsed } = std::mem::take(&mut *st) else {
                    unreachable!()
                };
                return match result {
                    Ok(out) => {
                        let size = std::fs::metadata(&out.binary).map(|m| m.len()).ok();
                        json!({
                            "status": "built",
                            "binary": root.relative(&out.binary).unwrap_or_else(|| {
                                out.binary.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                            }),
                            "bytes": size,
                            "elapsed_s": elapsed.as_secs(),
                        })
                    }
                    Err(e) => json!({ "status": "failed", "errors": e, "elapsed_s": elapsed.as_secs() }),
                };
            }
            SlotState::Running { started } => {
                let now = Instant::now();
                if now >= deadline {
                    return json!({
                        "status": "running",
                        "elapsed_s": started.elapsed().as_secs(),
                        "next": "call build again to keep waiting on this same build",
                    });
                }
                st = shared
                    .build
                    .done
                    .wait_timeout(st, deadline - now)
                    .unwrap_or_else(|p| p.into_inner())
                    .0;
            }
            SlotState::Idle => return json!({ "status": "idle" }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{FileList, NoProject};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixed(ProjectRoot);
    impl ProjectHost for Fixed {
        fn project(&self) -> Result<ProjectRoot, NoProject> {
            Ok(self.0.clone())
        }
        fn record(&mut self, rel: &str, list: FileList) -> Result<(), String> {
            crate::host::record_in_manifest(self.0.manifest(), rel, list)
        }
        fn external_crates(&self) -> Vec<String> {
            Vec::new()
        }
        fn version(&self) -> String {
            "test".into()
        }
    }

    fn fixture(name: &str, handler: &str) -> ProjectRoot {
        let dir = std::env::temp_dir().join(format!("prc-080-build-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("forms")).unwrap();
        std::fs::write(
            dir.join("B.project.toml"),
            "[project]\nname = \"B\"\nstructure = 1\nversion = \"1.0.0\"\nmain = \"\"\n[files]\nforms = [\"forms/MAIN.cfrm\"]\n",
        )
        .unwrap();
        let mut f = cobolt_forms::Form::new("MAIN", "MAIN", 300, 200);
        f.main_form = true;
        f.form_events
            .iter_mut()
            .find(|e| e.event == "onLoad")
            .expect("a new form has an onLoad stub")
            .code = handler.into();
        cobolt_forms::save_form(&f, &dir.join("forms/MAIN.cfrm")).unwrap();
        ProjectRoot::open(&dir).unwrap()
    }

    const CLEAN: &str = "       PROCEDURE DIVISION.\n           CONTINUE.\n";
    const BROKEN: &str = "       PROCEDURE DIVISION.\n           MOVE 1 TO WS-NOWHERE.\n";

    #[test]
    fn build_refuses_on_a_check_error_and_never_calls_the_builder() {
        let root = fixture("refuse", BROKEN);
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let shared = Arc::new(Shared::with_builder(
            Arc::new(move |_, _| {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(BuildOutcome { binary: PathBuf::from("x") })
            }),
            Duration::from_secs(5),
        ));
        let mut host = Fixed(root.clone());
        let v = run(&mut host, &root, &shared, false).unwrap();
        assert_eq!(v["status"], "refused", "{v}");
        assert!(v["check"]["errors"].as_u64().unwrap() >= 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        println!(
            "build: refused with {} check error(s), builder called 0 times",
            v["check"]["errors"]
        );
    }

    #[test]
    fn build_joins_the_running_build_instead_of_starting_another() {
        let root = fixture("join", CLEAN);
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let bin = root.dir().join("dist").join("B");
        let bin2 = bin.clone();
        let shared = Arc::new(Shared::with_builder(
            Arc::new(move |_, _| {
                c.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(900));
                std::fs::create_dir_all(bin2.parent().unwrap()).unwrap();
                std::fs::write(&bin2, b"binary").unwrap();
                Ok(BuildOutcome { binary: bin2.clone() })
            }),
            Duration::from_millis(300),
        ));
        let mut host = Fixed(root.clone());
        let first = run(&mut host, &root, &shared, false).unwrap();
        assert_eq!(first["status"], "running", "{first}");
        let mut polls = 1;
        let last = loop {
            let v = run(&mut host, &root, &shared, false).unwrap();
            polls += 1;
            if v["status"] != "running" {
                break v;
            }
            assert!(polls < 20, "the stub build never finished");
        };
        assert_eq!(last["status"], "built", "{last}");
        assert_eq!(last["binary"], "dist/B");
        assert_eq!(last["bytes"], 6);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one build, however many calls");
        // The generated program was written and recorded before the build.
        let view = cobolt_compiler::project_manifest_view(root.manifest()).unwrap();
        assert_eq!(view.generated, ["generated/MAIN.cbl"]);
        println!("build: {polls} calls, 1 build started, answered running then built (dist/B, 6 bytes)");
    }
}

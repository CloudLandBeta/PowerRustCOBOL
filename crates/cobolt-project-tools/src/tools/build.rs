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
#[derive(Debug, Clone, PartialEq)]
pub struct BuildOutcome {
    pub binary: PathBuf,
    /// A Desktop Build's report (DESKTOP-WASM-001 R13, R18) — the path it took, why it took the
    /// Cargo path, the time of each phase and the peak memory — as [`report_json`] writes it;
    /// `None` for a Mobile package.
    pub report: Option<Value>,
}

/// Why a build did not finish: the compiler's message and, when a Desktop Build stopped on the
/// Cargo path for want of a Rust toolchain (DESKTOP-WASM-001 R25), what [`failure_json`] says
/// about it.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildFailure {
    pub errors: String,
    pub detail: Option<Value>,
}

impl From<String> for BuildFailure {
    fn from(errors: String) -> Self {
        Self { errors, detail: None }
    }
}

/// Runs one build: the compiler, or a stub in tests.
pub type Builder =
    Arc<dyn Fn(&Path, &cobolt_compiler::BuildOptions) -> Result<BuildOutcome, BuildFailure> + Send + Sync>;

/// The real builder.
pub fn compiler_builder() -> Builder {
    Arc::new(|manifest, opts| {
        cobolt_compiler::build_project(manifest, opts)
            .map(|r| BuildOutcome { report: Some(report_json(&r)), binary: r.binary_path })
            .map_err(|e| BuildFailure { errors: e.to_string(), detail: failure_json(&e) })
    })
}

/// Why a Build took the Cargo path, for the agent (DESKTOP-WASM-001 R13): `kind`, the sentence the
/// IDE and `rcrun` show (`text`), and for an `EXEC RUST` block or a construct the WebAssembly path
/// does not take yet the `file` (relative to the project) and its `line` (absent when the
/// construct is a whole file); a refused construct also names itself (`construct`) and `why`. A
/// form holding a control the runner does not serve yet names the `form`, its `file`, the
/// `control`, its `control_type` and `why` (T33).
pub fn reason_json(reason: &cobolt_compiler::CargoReason) -> Value {
    use cobolt_compiler::CargoReason as R;
    let kind = match reason {
        R::DebugBuild => "debug_build",
        R::Chosen => "chosen",
        R::ExternalCrate => "external_crate",
        R::ExecRust { .. } => "exec_rust",
        R::Refused { .. } => "refused",
        R::DesktopControl { .. } => "desktop_control",
        R::RunnerUnavailable => "runner_unavailable",
    };
    let mut v = json!({ "kind": kind, "text": reason.to_string() });
    let at = match reason {
        R::ExecRust { file, line } => Some((file, *line)),
        R::Refused { construct, reason, file, line } => {
            v["construct"] = json!(construct);
            v["why"] = json!(reason);
            Some((file, *line))
        }
        R::DesktopControl { form, file, control, control_type, reason } => {
            v["form"] = json!(form);
            v["control"] = json!(control);
            v["control_type"] = json!(control_type);
            v["why"] = json!(reason);
            Some((file, 0))
        }
        _ => None,
    };
    if let Some((file, line)) = at {
        v["file"] = json!(file);
        if line > 0 {
            v["line"] = json!(line);
        }
    }
    v
}

/// A Desktop Build's report (DESKTOP-WASM-001 R13, R18): `path` ("WebAssembly" or "Cargo"),
/// `reason` on the Cargo path, `phases` in the order they ran with each one's time in ms (to a
/// tenth), and `peak_bytes`, the Build's highest resident memory (null where the system did not
/// tell). The Cargo path also says how many crates `cargo` compiled.
pub fn report_json(r: &cobolt_compiler::BuildResult) -> Value {
    let phases: Vec<Value> = r
        .phases
        .iter()
        .map(|(phase, took)| json!({ "phase": phase.name(), "ms": (took.as_secs_f64() * 10_000.0).round() / 10.0 }))
        .collect();
    let mut v = json!({ "path": r.path.name(), "phases": phases, "peak_bytes": r.peak_rss });
    if let Some(cargo) = &r.cargo {
        v["reason"] = reason_json(&cargo.reason);
        v["crates_compiled"] = json!(cargo.crates_compiled);
    }
    v
}

/// What a failed Desktop Build adds to its message (DESKTOP-WASM-001 R25): a Build that stopped
/// because its program takes the Cargo path and no usable Rust toolchain was found answers
/// `needs_rust`, the missing `tool`, the path and the `reason` the program needs Cargo — so the
/// agent can tell the developer why, and what else would let it build. `None` for any other failure.
pub fn failure_json(e: &cobolt_compiler::CompilerError) -> Option<Value> {
    use cobolt_compiler::CompilerError as E;
    match e {
        E::CargoNeedsToolchain { reason, tool, .. } => Some(json!({
            "needs_rust": true,
            "tool": tool,
            "path": cobolt_compiler::BuildPath::Cargo.name(),
            "reason": reason_json(reason),
        })),
        E::Toolchain { tool, .. } => Some(json!({ "needs_rust": true, "tool": tool })),
        _ => None,
    }
}

/// `extra`'s fields, copied into the answer `into`.
fn merge(into: &mut Value, extra: Option<&Value>) {
    if let (Some(into), Some(Value::Object(extra))) = (into.as_object_mut(), extra) {
        for (k, v) in extra {
            into.insert(k.clone(), v.clone());
        }
    }
}

#[derive(Default)]
enum SlotState {
    #[default]
    Idle,
    Running {
        started: Instant,
    },
    Done {
        result: Result<BuildOutcome, BuildFailure>,
        elapsed: Duration,
    },
}

/// The build in flight, shared by every handler of one server.
#[derive(Default)]
pub struct BuildSlot {
    state: Mutex<SlotState>,
    done: Condvar,
}

/// A Web or Mobile project's build (WASM-015 R10, R12): the same gate as the
/// desktop build — regenerate, then `check`, and no build over an error — and
/// then the web backend builds the bundle. The answer carries the size by part,
/// the start-up time and the warnings; a Mobile project says plainly which
/// package step this computer cannot perform and why.
fn run_web<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    shared: &Arc<Shared>,
    view: &cobolt_compiler::ManifestView,
    release: bool,
) -> Result<Value, String> {
    use crate::web::{self, WebBuildError, WebBuildRequest};
    use cobolt_compiler::project_kind::MobilePlatform;
    let platform = view.mobile.as_ref().filter(|_| view.kind == cobolt_compiler::ProjectKind::Mobile).map(|m| m.platform);
    // A package still building: keep waiting on it (the bundle it wraps was built by the call that
    // started it, and is not rebuilt under it).
    if let (Some(platform), false) = (platform, slot_idle(shared)) {
        return Ok(json!({
            "status": "built",
            "kind": view.kind.as_str(),
            "bundle": web::BUNDLE_SUBDIR,
            "package": package_answer(wait(shared, root), platform),
        }));
    }
    if let Some(why) = host.build_blocked() {
        return Err(why);
    }
    {
        let _w = shared.write_lock.lock().unwrap_or_else(|p| p.into_inner());
        regenerate::run(host, root, None)?;
    }
    let report = check::check_project(host, root)?;
    if report.errors() > 0 {
        return Ok(json!({
            "status": "refused",
            "kind": view.kind.as_str(),
            "reason": "the project has errors; fix them (see diagnostics) and build again",
            "check": report.to_json(),
        }));
    }
    let mut request = WebBuildRequest::from_manifest(root.manifest())?;
    // A release package wraps a release bundle (WASM-013 R11).
    if release {
        request.profile = web::BuildProfile::Release;
    }
    let backend = web::backend();
    let started = Instant::now();
    let mut answer = match backend.build(&request) {
        Ok(built) => {
            let mut a = json!({
                "status": "built",
                "kind": view.kind.as_str(),
                "engine": backend.name(),
                "bundle": root.relative(&built.bundle_dir).unwrap_or_else(|| web::BUNDLE_SUBDIR.to_owned()),
            });
            a["report"] = built.to_json();
            a["variants"] = json!({ "multi_memory": request.variants.multi_memory, "single_memory": request.variants.single_memory });
            a
        }
        Err(WebBuildError::NotYetLinked) => json!({
            "status": "not_linked",
            "kind": view.kind.as_str(),
            "reason": web::NOT_YET_LINKED_MESSAGE,
        }),
        Err(WebBuildError::Refused(lines)) => json!({
            "status": "refused",
            "kind": view.kind.as_str(),
            "reason": "the project uses what a browser cannot run",
            "portable_subset": lines.iter().map(|d| d.to_string()).collect::<Vec<_>>(),
        }),
        Err(WebBuildError::Failed(why)) => json!({ "status": "failed", "kind": view.kind.as_str(), "errors": why }),
        Err(WebBuildError::PlayerPin(e)) => json!({ "status": "failed", "kind": view.kind.as_str(), "errors": e.to_string() }),
    };
    answer["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
    if view.kind == cobolt_compiler::ProjectKind::Mobile && answer["status"] == "built" {
        // A Mobile project whose `[mobile]` table cannot be read is answered by the iOS step, which
        // says so (`PackageRequest::from_manifest`).
        answer["package"] = start_package(shared, root, release, platform.unwrap_or(MobilePlatform::Ios));
    }
    Ok(answer)
}

fn slot_idle(shared: &Arc<Shared>) -> bool {
    matches!(*shared.build.state.lock().unwrap_or_else(|p| p.into_inner()), SlotState::Idle)
}

/// The tools of a package build, found on this computer.
enum PackageTools {
    Android(cobolt_compiler::mobile_build::Toolchain),
    Ios(cobolt_compiler::mobile_build::IosToolchain),
}

/// The package step of a Mobile project (WASM-012 T18; the iOS one is the simulator application):
/// the tools found, the project checked, and the package built on the build worker — the slot a
/// desktop build uses, so one build runs at a time and a call waits at most
/// [`DEFAULT_BUILD_WAIT`] on it. The tools' output goes to `package.log` in the staging folder
/// (stdout is the protocol stream of `rcrun mcp`).
fn start_package(shared: &Arc<Shared>, root: &ProjectRoot, release: bool, platform: cobolt_compiler::project_kind::MobilePlatform) -> Value {
    use cobolt_compiler::mobile_build as mb;
    use cobolt_compiler::project_kind::MobilePlatform;
    let abi = mb::AndroidAbi::Aarch64;
    let (tools, label) = match platform {
        MobilePlatform::Android => (mb::Toolchain::detect(abi).map(PackageTools::Android), "Android"),
        MobilePlatform::Ios => (mb::IosToolchain::detect().map(PackageTools::Ios), "iOS"),
    };
    let tools = match tools {
        Ok(t) => t,
        Err(missing) => {
            let how: Vec<&str> = missing.iter().map(|m| m.how.as_str()).collect();
            return json!({ "built": false, "reason": format!("the {label} tools this build needs are missing"), "missing": how });
        }
    };
    let profile = if release { mb::Profile::Release } else { mb::Profile::Debug };
    let name = cobolt_compiler::project_manifest_view(root.manifest()).map(|v| mb::application_name(&v.name)).unwrap_or_default();
    let staging = cobolt_compiler::build_staging_dir(None, &format!("{}-{}", mb::crate_name(&name), platform.as_str()));
    let req = match mb::PackageRequest::from_manifest(root.manifest(), staging, profile, abi) {
        Ok(r) => r,
        Err(e) => return json!({ "built": false, "reason": e }),
    };
    let secrets = release.then(signing_secrets_from_env).flatten();
    if let Err(problems) = mb::check(&req, secrets.as_ref()) {
        return json!({ "built": false, "reason": "the package cannot be built yet", "refused": problems });
    }
    let log_path = req.staging_dir.join("package.log");
    let log_shown = root.relative(&log_path).unwrap_or_else(|| log_path.to_string_lossy().into_owned());
    start_job(shared, move || {
        let _ = std::fs::create_dir_all(&req.staging_dir);
        let mut file = std::fs::File::create(&log_path).ok();
        let mut log = |line: &str| {
            if let Some(f) = file.as_mut() {
                use std::io::Write;
                let _ = writeln!(f, "{line}");
            }
        };
        let package = match &tools {
            PackageTools::Android(t) => mb::build_android(&req, t, secrets.as_ref(), &mut log),
            PackageTools::Ios(t) => mb::build_ios(&req, t, &mut log),
        };
        package.map(|p| BuildOutcome { binary: p.path, report: None }).map_err(BuildFailure::from)
    });
    let mut answer = package_answer(wait(shared, root), platform);
    answer["log"] = json!(log_shown);
    answer
}

/// The release passwords from the build's environment (`PRC_ANDROID_STORE_PASSWORD`,
/// `PRC_ANDROID_KEY_PASSWORD`); the IDE reads them from the system's secret store instead.
fn signing_secrets_from_env() -> Option<cobolt_compiler::mobile_build::SigningSecrets> {
    use cobolt_compiler::mobile_build as mb;
    let store_password = std::env::var(mb::ENV_STORE_PASSWORD).ok()?;
    let key_password = std::env::var(mb::ENV_KEY_PASSWORD).ok()?;
    Some(mb::SigningSecrets { store_password, key_password })
}

/// A package's answer from the slot's: `built` true or false, the package's path as `apk` (Android)
/// or `app` (the iOS simulator's application).
fn package_answer(mut slot: Value, platform: cobolt_compiler::project_kind::MobilePlatform) -> Value {
    let built = slot["status"] == "built";
    let key = match platform {
        cobolt_compiler::project_kind::MobilePlatform::Android => "apk",
        cobolt_compiler::project_kind::MobilePlatform::Ios => "app",
    };
    if let Some(path) = slot.as_object_mut().and_then(|o| o.remove("binary")) {
        slot[key] = path;
    }
    slot["built"] = json!(built);
    slot
}

pub fn run<H: ProjectHost>(
    host: &mut H,
    root: &ProjectRoot,
    shared: &Arc<Shared>,
    full: bool,
    release: bool,
) -> Result<Value, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    if !view.kind.is_desktop() {
        return run_web(host, root, shared, &view, release);
    }
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

/// Run `job` on the build worker unless a build is already running.
fn start_job(shared: &Arc<Shared>, job: impl FnOnce() -> Result<BuildOutcome, BuildFailure> + Send + 'static) {
    {
        let mut st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
        if !matches!(*st, SlotState::Idle) {
            return;
        }
        *st = SlotState::Running { started: Instant::now() };
    }
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        let started = Instant::now();
        let result = job();
        let mut st = shared.build.state.lock().unwrap_or_else(|p| p.into_inner());
        *st = SlotState::Done { result, elapsed: started.elapsed() };
        shared.build.done.notify_all();
    });
}

/// Start the build on a worker unless one is already running.
fn start(shared: &Arc<Shared>, manifest: PathBuf, workspace_root: Option<PathBuf>, full: bool) {
    let builder = Arc::clone(&shared.builder);
    start_job(shared, move || {
        let opts = cobolt_compiler::BuildOptions {
            // Nothing on stdout: on `rcrun mcp` stdout is the protocol stream.
            verbose: false,
            workspace_root,
            progress: None,
            target: None,
            full,
            // The folder is resolved by the compiler: the IDE's setting when this runs inside the
            // IDE, else the POWERRUSTCOBOL_BUILD_DIR variable, else the default (spec 100).
            build_dir: None,
            // Built from disk, like `rcrun build`: the files are what the
            // agent saved (a host with unsaved edits refuses before this), so
            // the forms' COBOL is regenerated from them first.
            regenerate_forms: true,
            // The agent's build is the one a developer ships: optimised.
            debug: false,
            // The path the project chooses: the WebAssembly one by default, with the runner found
            // beside this program, Cargo when the project or its contents ask for it (DESKTOP-WASM-001).
            runner: None,
            force_cargo: false,
            no_machine_code: false,
            wasm_only: false,
            ask_before_platform_compile: false,
        };
        builder(&manifest, &opts)
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
                        // An iOS package is a folder (the application): the size of what is in it.
                        let size = std::fs::metadata(&out.binary)
                            .map(|m| if m.is_dir() { cobolt_compiler::mobile_build::dir_size(&out.binary) } else { m.len() })
                            .ok();
                        let mut answer = json!({
                            "status": "built",
                            "binary": root.relative(&out.binary).unwrap_or_else(|| {
                                out.binary.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                            }),
                            "bytes": size,
                            "elapsed_s": elapsed.as_secs(),
                        });
                        merge(&mut answer, out.report.as_ref());
                        answer
                    }
                    Err(e) => {
                        let mut answer = json!({ "status": "failed", "errors": e.errors, "elapsed_s": elapsed.as_secs() });
                        merge(&mut answer, e.detail.as_ref());
                        answer
                    }
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
                Ok(BuildOutcome { binary: PathBuf::from("x"), report: None })
            }),
            Duration::from_secs(5),
        ));
        let mut host = Fixed(root.clone());
        let v = run(&mut host, &root, &shared, false, false).unwrap();
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
                Ok(BuildOutcome { binary: bin2.clone(), report: None })
            }),
            Duration::from_millis(300),
        ));
        let mut host = Fixed(root.clone());
        let first = run(&mut host, &root, &shared, false, false).unwrap();
        assert_eq!(first["status"], "running", "{first}");
        let mut polls = 1;
        let last = loop {
            let v = run(&mut host, &root, &shared, false, false).unwrap();
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

    /// A hand-made Build result, as `build_project` returns it.
    fn result(path: cobolt_compiler::BuildPath, reason: Option<cobolt_compiler::CargoReason>) -> cobolt_compiler::BuildResult {
        use cobolt_compiler::BuildPhase as P;
        let ms = |n: u64| Duration::from_micros(n * 1000 + 250);
        let phases = match path {
            cobolt_compiler::BuildPath::Wasm => vec![
                (P::Regenerate, ms(3)),
                (P::FrontEnd, ms(41)),
                (P::Classify, ms(1)),
                (P::Generate, ms(17)),
                (P::MachineCode, ms(820)),
                (P::Assemble, ms(9)),
                (P::Install, ms(4)),
            ],
            cobolt_compiler::BuildPath::Cargo => vec![(P::Regenerate, ms(3)), (P::FrontEnd, ms(41)), (P::Cargo, ms(61_000)), (P::Install, ms(4))],
        };
        cobolt_compiler::BuildResult {
            binary_path: PathBuf::from("bin/B"),
            source_count: 2,
            form_count: 0,
            ast_bytes: 0,
            build_dir: PathBuf::from("staging"),
            path,
            cargo: reason.map(|reason| cobolt_compiler::CargoBuild {
                reason,
                crates_compiled: 3,
                target_dir: PathBuf::from("staging/target"),
                forced_full: Vec::new(),
            }),
            phases,
            peak_rss: Some(181_403_648),
        }
    }

    /// **DESKTOP-WASM-001 T19 (R13, R18, R25)** — the `build` tool's answer names the path the Build
    /// took, why it took the Cargo path (with the file and line of an `EXEC RUST` block or of a
    /// construct the WebAssembly path does not take yet), each phase in ms and the peak; a Build
    /// stopped for want of Rust says so, with the reason. Hand-made results, then one through the
    /// build slot as an agent sees it.
    #[test]
    fn build_answer_names_the_path_the_reason_the_phases_and_the_peak() {
        use cobolt_compiler::{BuildPath, CargoReason, CompilerError};
        let t = Instant::now();
        let mut checks: Vec<(String, bool)> = Vec::new();
        let mut check = |what: &str, ok: bool| checks.push((what.to_owned(), ok));

        // ── The WebAssembly path: no reason, seven phases, the peak ──
        let wasm = report_json(&result(BuildPath::Wasm, None));
        check("wasm: path WebAssembly", wasm["path"] == "WebAssembly");
        check("wasm: no reason", wasm.get("reason").is_none());
        let names: Vec<&str> = wasm["phases"].as_array().unwrap().iter().map(|p| p["phase"].as_str().unwrap()).collect();
        check(
            "wasm: seven phases in order",
            names == ["regenerate", "front end", "classify", "generate", "machine code", "assemble", "install"],
        );
        check("wasm: machine code 820.3 ms", wasm["phases"][4]["ms"] == 820.3);
        check("wasm: peak bytes", wasm["peak_bytes"] == 181_403_648u64);

        // ── The Cargo path for an EXEC RUST block: its file and line ──
        let exec = report_json(&result(BuildPath::Cargo, Some(CargoReason::ExecRust { file: "src/main.cbl".into(), line: 12 })));
        check("exec rust: path Cargo", exec["path"] == "Cargo");
        check("exec rust: kind", exec["reason"]["kind"] == "exec_rust");
        check("exec rust: file and line", exec["reason"]["file"] == "src/main.cbl" && exec["reason"]["line"] == 12);
        check("exec rust: text names the block", exec["reason"]["text"].as_str().unwrap().contains("src/main.cbl line 12 holds an EXEC RUST"));
        check("exec rust: cargo phase 61000.3 ms", exec["phases"][2]["phase"] == "cargo" && exec["phases"][2]["ms"] == 61_000.3);
        check("exec rust: crates compiled", exec["crates_compiled"] == 3);

        // ── A refused construct: its name, why, file and line; a whole file has no line ──
        let refused = CargoReason::Refused {
            construct: "a disk file".into(),
            reason: "files come at M3".into(),
            file: "src/common.cbl".into(),
            line: 7,
        };
        let r = report_json(&result(BuildPath::Cargo, Some(refused)));
        check(
            "refused: kind, construct, why",
            r["reason"]["kind"] == "refused" && r["reason"]["construct"] == "a disk file" && r["reason"]["why"] == "files come at M3",
        );
        check("refused: file and line", r["reason"]["file"] == "src/common.cbl" && r["reason"]["line"] == 7);
        let whole = reason_json(&CargoReason::Refused {
            construct: "a form".into(),
            reason: "forms come at M2".into(),
            file: "forms/MAIN.cfrm".into(),
            line: 0,
        });
        check("refused whole file: file, no line", whole["file"] == "forms/MAIN.cfrm" && whole.get("line").is_none());

        // ── A control the runner does not serve yet: the form, its file, the control (T33) ──
        let control = reason_json(&CargoReason::DesktopControl {
            form: "MAIN".into(),
            file: "forms/main.cfrm".into(),
            control: "MAP-CITY".into(),
            control_type: "Maps".into(),
            reason: "Maps come at M6".into(),
        });
        check(
            "desktop control: kind, form, file, control, type, why, no line",
            control["kind"] == "desktop_control"
                && control["form"] == "MAIN"
                && control["file"] == "forms/main.cfrm"
                && control["control"] == "MAP-CITY"
                && control["control_type"] == "Maps"
                && control["why"] == "Maps come at M6"
                && control.get("line").is_none(),
        );
        check("desktop control: text names the control", control["text"].as_str().unwrap().contains("MAP-CITY (Maps)"));

        // ── Every reason has its kind ──
        let kinds: Vec<Value> = [
            CargoReason::DebugBuild,
            CargoReason::Chosen,
            CargoReason::ExternalCrate,
            CargoReason::RunnerUnavailable,
        ]
        .iter()
        .map(|r| reason_json(r)["kind"].clone())
        .collect();
        check(
            "the other four kinds",
            kinds == [json!("debug_build"), json!("chosen"), json!("external_crate"), json!("runner_unavailable")],
        );

        // ── No Rust: the failure says so, with the reason ──
        let no_rust = CompilerError::CargoNeedsToolchain {
            reason: CargoReason::ExecRust { file: "src/main.cbl".into(), line: 12 },
            tool: "rustc".into(),
            detail: "not found".into(),
        };
        let f = failure_json(&no_rust).expect("a typed toolchain failure has detail");
        check("no rust: needs_rust, tool, path", f["needs_rust"] == true && f["tool"] == "rustc" && f["path"] == "Cargo");
        check("no rust: reason with file and line", f["reason"]["file"] == "src/main.cbl" && f["reason"]["line"] == 12);
        check("any other failure: no detail", failure_json(&CompilerError::NoMain).is_none());

        // ── Through the slot, as an agent sees it ──
        let root = fixture("report", CLEAN);
        let shared = Arc::new(Shared::with_builder(
            Arc::new(|_, _| {
                let r = result(BuildPath::Cargo, Some(CargoReason::ExecRust { file: "src/main.cbl".into(), line: 12 }));
                Ok(BuildOutcome { report: Some(report_json(&r)), binary: PathBuf::from("bin/B") })
            }),
            Duration::from_secs(5),
        ));
        let mut host = Fixed(root.clone());
        let built = run(&mut host, &root, &shared, false, false).unwrap();
        check("slot: built", built["status"] == "built");
        check(
            "slot: path, reason, phases, peak",
            built["path"] == "Cargo" && built["reason"]["line"] == 12 && built["phases"].as_array().map_or(0, Vec::len) == 4 && built["peak_bytes"] == 181_403_648u64,
        );
        let failing = Arc::new(Shared::with_builder(
            Arc::new(move |_, _| Err(BuildFailure { errors: no_rust.to_string(), detail: failure_json(&no_rust) })),
            Duration::from_secs(5),
        ));
        let failed = run(&mut host, &root, &failing, false, false).unwrap();
        check("slot: failed", failed["status"] == "failed");
        check(
            "slot: the failure names the need and the reason",
            failed["needs_rust"] == true
                && failed["reason"]["kind"] == "exec_rust"
                && failed["errors"].as_str().unwrap().contains("Rust toolchain is required"),
        );

        let failures: Vec<&(String, bool)> = checks.iter().filter(|(_, ok)| !ok).collect();
        println!("== build tool answer (DESKTOP-WASM-001 T19: R13, R18, R25) ==");
        println!("reports: WebAssembly 1, Cargo 2 (EXEC RUST, refused construct); reasons 6 kinds; failures 2; slot answers 2");
        for (what, ok) in &checks {
            println!("  {} {what}", if *ok { "ok  " } else { "FAIL" });
        }
        println!("{} of {} checks passed in {:.1} ms", checks.len() - failures.len(), checks.len(), t.elapsed().as_secs_f64() * 1000.0);
        assert!(failures.is_empty(), "failed: {failures:?}");
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Building a project's backend into a headless server binary (WASM-003 R2, R4; spec 114).
//!
//! The backend is the `backend/` folder of a Web or Mobile project (WASM-001
//! D13). A build here:
//!
//! 1. reads and checks it (`cobolt_server::bundle`): every program parsed and
//!    analysed, `exports.toml` read, every exported program's parameters turned
//!    into layouts;
//! 2. writes the **interface description** to `backend/contract/interface.json`
//!    — what the frontend project's Check imports (R4);
//! 3. compares it with the last released one and **fails a release build** on a
//!    breaking change unless the contract's major version was raised on purpose
//!    (WASM-014 R5);
//! 4. chooses a path ([`crate::backend_module::classify_backend`]):
//!    - the **module path** (the default when the module serves the whole Backend): the programs become one
//!      WebAssembly module, which goes with its machine code into a copy of the prebuilt headless runner. No Rust
//!      toolchain is used (spec 114 R14);
//!    - the **Cargo path**: a small Cargo project — **no forms, no `eframe`, no `egui`**: the `cobolt-server` crate
//!      and the runtime features the programs reach — is generated and built;
//! 5. installs `bin/<name>-backend` and `bin/sbom.cdx.json`.
//!
//! The deliverable is one self-contained executable per host platform; the host
//! only, as everywhere else in this compiler (spec 041). Both paths give a file with the same name, the same arguments
//! and the same behaviour.

use std::path::{Path, PathBuf};

use std::time::Instant;

use crate::backend_module::{self, BackendCargoReason, BackendEngine, BackendPath, BackendPhase, ClassifyInput, ModuleBuild};
use crate::{
    build_staging_dir, host_triple, install_executable, resolve_workspace_root, runtime_features, sanitize_package_name, toolchain_error,
    CompilerError, IoContext,
};

/// A Backend built for another system than this machine's (spec 115 R3, R5, R10): the application's payload joined to that
/// system's runner, installed in a folder of its own so the project's `bin/` (the Backend Run and Preview start) is not touched.
#[derive(Debug, Clone)]
pub struct BackendTarget {
    pub system: crate::backend_runners::System,
    /// The runner of that system, already verified ([`crate::backend_runners::runner_for`]).
    pub runner: backend_module::RunnerFiles,
    /// Where the executable and its bill of materials are installed.
    pub bin_dir: PathBuf,
}

/// What a backend build is told.
#[derive(Debug, Clone, Default)]
pub struct BackendBuildOptions {
    /// A Backend for another system, which only the module path can make: a Cargo reason is refused
    /// ([`CompilerError::BackendForeignRefused`]), no machine code is made, and `bin/` of the project is left as it was.
    /// `None` is the Build of this machine's own system, as it has always been.
    pub target: Option<BackendTarget>,
    /// A debug build (the default is release).
    pub debug: bool,
    /// Print what it is doing.
    pub verbose: bool,
    /// Where the platform's crates are, when not found beside the executable.
    pub workspace_root: Option<PathBuf>,
    /// Let a breaking change to the contract through (the major version was
    /// raised on purpose, or this is a development build).
    pub accept_breaking: bool,
    /// `rcrun backend build --cargo` or `--wasm`; `None` follows the project's `[backend] build_path` (spec 114 R13).
    pub path: Option<BackendPath>,
    /// The Backend runner a module is put into; `None` looks in [`backend_module::BACKEND_RUNNER_ENV`], then beside the
    /// running program.
    pub runner: Option<PathBuf>,
}

/// What a backend build produced.
#[derive(Debug, Clone)]
pub struct BackendBuildResult {
    pub binary: PathBuf,
    pub interface: PathBuf,
    pub contract: String,
    pub programs: usize,
    /// Warnings about the contract (a compatible-but-noteworthy change).
    pub notes: Vec<String>,
    /// The software bill of materials written beside the binary (`sbom.cdx.json`, WASM-013 R10).
    pub sbom: PathBuf,
    /// The way the binary was made (spec 114 R13).
    pub engine: BackendEngine,
    /// Why the Build took the Cargo path, each reason with its construct, file and line; empty on the module path.
    pub reasons: Vec<BackendCargoReason>,
    /// The time of each phase of this Build, in order (R16).
    pub phases: Vec<BackendPhase>,
    /// The module's SHA-256 (lower-case hex), size and machine-code size; `None` on the Cargo path.
    pub module_sha256: Option<String>,
    pub module_bytes: Option<usize>,
    pub machine_code_bytes: Option<usize>,
}

/// The CycloneDX document of a backend build (WASM-013 T17): the binary with its SHA-256, then every
/// package of the generated project's resolved graph with its version and licence, from the `cargo
/// metadata` JSON. The generated package itself is not listed as a library; it is the application.
pub fn backend_sbom(application: &str, binary_name: &str, binary_bytes: &[u8], metadata_json: &str) -> Result<serde_json::Value, String> {
    use sha2::{Digest, Sha256};
    let meta: serde_json::Value = serde_json::from_str(metadata_json).map_err(|e| format!("cargo metadata answered something unreadable: {e}"))?;
    let packages = meta["packages"].as_array().ok_or("cargo metadata listed no packages")?;
    let digest = Sha256::digest(binary_bytes);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let mut components = vec![cobolt_wire::sbom::file_component(binary_name, &hex)];
    let mut libs: Vec<(String, String, String)> = packages
        .iter()
        .filter(|p| p["name"].as_str() != Some(application))
        .map(|p| {
            (
                p["name"].as_str().unwrap_or("?").to_owned(),
                p["version"].as_str().unwrap_or("?").to_owned(),
                p["license"].as_str().filter(|l| !l.is_empty()).unwrap_or("NOASSERTION").to_owned(),
            )
        })
        .collect();
    libs.sort();
    for (n, v, l) in &libs {
        components.push(cobolt_wire::sbom::library_component(n, v, l));
    }
    Ok(cobolt_wire::sbom::cyclonedx(application, components, None))
}

fn main_rs() -> &'static str {
    "// Generated by PowerRustCOBOL AI — the project's backend. Do not edit.\n\
     static BUNDLE: &[u8] = include_bytes!(\"../assets/backend.bin\");\n\
     fn main() {\n    std::process::exit(cobolt_server::standalone::main(BUNDLE, std::env::args().skip(1).collect()));\n}\n"
}

/// What the Backend's own staging folder tells Cargo on Windows (spec 113 R13): the Backend carries its C runtime, so it
/// starts on a machine that has not installed the Visual C++ Redistributable. Only the MSVC hosts need it (the other
/// systems link what they have), and only this staging folder says so: no other build of the product is touched.
fn static_crt_config(host: &str) -> Option<String> {
    if !host.ends_with("-pc-windows-msvc") {
        return None;
    }
    Some(format!(
        "# Written by PowerRustCOBOL AI for this Backend only: the C runtime is linked in, not required on the machine that runs it.\n[target.{host}]\nrustflags = [\"-C\", \"target-feature=+crt-static\"]\n"
    ))
}

fn cargo_toml(bin_name: &str, version: &str, crates_path: &Path, features: runtime_features::RuntimeFeatures) -> String {
    let cp = crate::external_crates::toml_path(crates_path);
    let feats = features.as_toml_features();
    format!(
        r#"[package]
name    = "{bin_name}"
version = "{version}"
edition = "2021"

[workspace]

[[bin]]
name = "{bin_name}"
path = "src/main.rs"

[dependencies]
cobolt-server  = {{ path = "{cp}/cobolt-server", default-features = false, features = ["server", "interpreter"] }}
cobolt-runtime = {{ path = "{cp}/cobolt-runtime", default-features = false, features = [{feats}] }}

[profile.release]
opt-level = 3
"#
    )
}

/// The project's `[backend] state` (WASM-003 Q2): `per-call` when the folder
/// has no manifest, the manifest has no `[backend]` or it says nothing; a
/// value that is neither spelling stops the build.
pub fn backend_state(project_dir: &Path) -> Result<cobolt_server::backend::StateMode, CompilerError> {
    let Some(manifest) = crate::find_project_manifest(project_dir).filter(|m| m.parent().and_then(|p| p.canonicalize().ok()) == project_dir.canonicalize().ok()) else {
        return Ok(cobolt_server::backend::StateMode::default());
    };
    let view = crate::project_manifest_view(&manifest).map_err(CompilerError::Workspace)?;
    view.backend_state_mode().map_err(|message| CompilerError::Semantic { file: manifest.display().to_string().into(), message })
}

/// Does `backend/backend.toml` have an `[identity]` section (WASM-009 R7)? Only its presence is read
/// here; its settings are checked when the backend starts.
pub fn backend_has_identity(backend_dir: &Path) -> bool {
    std::fs::read_to_string(backend_dir.join("backend.toml"))
        .ok()
        .and_then(|t| t.parse::<toml::Value>().ok())
        .is_some_and(|v| v.get("identity").is_some())
}

/// Bring `backend/generated/identity/` in line with `backend.toml` (WASM-009 D4): the identity
/// programs are written while the section is there and removed once it is gone. Every path that runs
/// or builds a backend calls this first — the backend build, `rcrun backend run|check|describe|release`,
/// the Web Build — so the programs never lag behind the section. Only that folder is written.
pub fn regenerate_identity(backend_dir: &Path) -> Result<(), CompilerError> {
    cobolt_codegen::identity::sync(backend_dir, backend_has_identity(backend_dir)).map(|_| ()).map_err(CompilerError::Workspace)
}

/// What the Web Build takes from `backend/backend.toml`'s `[identity]` (WASM-009 D9, D14): the
/// public reCAPTCHA site key for the page, and the secrets as they resolve on this machine, which
/// no form, copybook or bundle file may hold. No file or no section gives the default.
pub fn identity_for_build(backend_dir: &Path) -> cobolt_server::identity_settings::BuildIdentity {
    let text = std::fs::read_to_string(backend_dir.join("backend.toml")).unwrap_or_default();
    cobolt_server::identity_settings::build_identity(&text, backend_dir)
}

/// What the contract check returns: the report and the structured breaks in it (WASM-014 T16).
pub use cobolt_wire::evolution::{Break as ContractBreak, Report as ContractReport};

/// What comparing a backend's interface with the contracts it released found (WASM-014 R5): the one place Build, `rcrun` and the IDE
/// get it from.
fn contract_report(current: &cobolt_wire::contract::InterfaceDescription, history: &[String]) -> cobolt_wire::evolution::Report {
    let released: Vec<cobolt_wire::contract::InterfaceDescription> = history.iter().filter_map(|h| cobolt_wire::contract::InterfaceDescription::from_json(h).ok()).collect();
    cobolt_wire::evolution::check_against_released(current, &released)
}

/// Read the backend of the project in `project_dir` and compare its contract with the last released one, without building anything
/// (WASM-014 T16): no cargo, no staging folder, nothing written but the identity programs that every path regenerates first. The IDE
/// calls this to write the report to the Output panel before a Build; each break names its program, parameter, item and rule.
pub fn check_contract(project_dir: &Path) -> Result<cobolt_wire::evolution::Report, CompilerError> {
    let backend_dir = project_dir.join("backend");
    if !backend_dir.join("exports.toml").exists() {
        return Err(CompilerError::Workspace(format!("{} has no exports.toml: a backend declares the programs it exports there (see the Developer's Guide, Backend)", backend_dir.display())));
    }
    let state = backend_state(project_dir)?;
    regenerate_identity(&backend_dir)?;
    let mut bundle = cobolt_server::bundle::Bundle::from_dir(&backend_dir).map_err(|errs| CompilerError::Parse { file: "backend".into(), message: errs.join("\n") })?;
    bundle.state = state;
    let history = bundle.history.clone();
    let backend = bundle.into_backend().map_err(|errs| CompilerError::Semantic { file: "backend".into(), message: errs.join("\n") })?;
    Ok(contract_report(&backend.interface, &history))
}

/// The `[backend]` settings of the project in `project_dir` that choose a path: what the manifest says in `build_path`, and whether it
/// registers External Crates. A folder with no manifest of its own reads `auto` and no crates.
fn path_settings(project_dir: &Path) -> Result<(BackendPath, bool), CompilerError> {
    let Some(manifest) = crate::find_project_manifest(project_dir).filter(|m| m.parent().and_then(|p| p.canonicalize().ok()) == project_dir.canonicalize().ok()) else {
        return Ok((BackendPath::Auto, false));
    };
    let view = crate::project_manifest_view(&manifest).map_err(CompilerError::Workspace)?;
    let path = view.backend_build_path_choice().map_err(|message| CompilerError::Semantic { file: manifest.display().to_string(), message })?;
    Ok((path, !view.crates.is_empty()))
}

/// One line per reason, as a refusal and a toolchain error print them.
fn reason_lines(reasons: &[BackendCargoReason]) -> String {
    reasons.iter().map(|r| format!("  - {r}")).collect::<Vec<_>>().join("\n")
}

/// The first `EXEC RUST` line of each program that has one: its file relative to the project (`files` gives it by PROGRAM-ID),
/// and the line. What `classify_backend` takes as `exec_rust`.
fn exec_rust_of(programs: &[cobolt_ast::program::Program], files: &[(String, String)]) -> Vec<(String, u32)> {
    programs
        .iter()
        .filter_map(|p| {
            crate::first_exec_rust_line(p).map(|line| {
                let id = p.identification.program_id.to_ascii_uppercase();
                (files.iter().find(|(f, _)| *f == id).map_or("backend".to_string(), |(_, rel)| rel.clone()), line)
            })
        })
        .collect()
}

/// The exported programs that seed a control, by PROGRAM-ID, in order. What `classify_backend` takes as `control_programs`.
fn control_programs_of(backend: &cobolt_server::backend::Backend) -> Vec<String> {
    let mut names: Vec<String> = backend.programs().filter(|p| p.control.is_some()).map(|p| p.sig.name.clone()).collect();
    names.sort();
    names
}

/// A Backend and its module, made in memory.
pub struct DevModule {
    pub backend: cobolt_server::backend::Backend,
    pub compiled: cobolt_wasm::Compiled,
}

/// The module of the Backend in `project_dir`, made in memory for `rcrun backend run --engine wasm` (spec 114 R18): the same
/// reading, the same reasons and the same generator as the Build's module path, with no runner, nothing written and no
/// toolchain, so what will ship can be tried before it ships. `Err` says, line by line, why the module does not serve this
/// Backend (the Build would take the Cargo path for it).
pub fn dev_module(project_dir: &Path) -> Result<DevModule, Vec<String>> {
    let backend_dir = project_dir.join("backend");
    let one = |e: CompilerError| vec![e.to_string()];
    let state = backend_state(project_dir).map_err(one)?;
    let (_, external_crates) = path_settings(project_dir).map_err(one)?;
    regenerate_identity(&backend_dir).map_err(one)?;
    let mut bundle = cobolt_server::bundle::Bundle::from_dir(&backend_dir)?;
    bundle.state = state;
    let files = backend_module::program_files(project_dir);
    let exec_rust = exec_rust_of(&bundle.programs, &files);
    let backend = bundle.into_backend()?;
    let control_programs = control_programs_of(&backend);
    let reasons = backend_module::classify_backend(&ClassifyInput { path: BackendPath::Auto, external_crates, exec_rust, identity: backend_has_identity(&backend_dir), control_programs });
    if !reasons.is_empty() {
        return Err(reasons.iter().map(ToString::to_string).collect());
    }
    let compiled = backend_module::generate(&backend, &files).map_err(|refused| refused.iter().map(ToString::to_string).collect::<Vec<_>>())?;
    Ok(DevModule { backend, compiled })
}

/// Build the backend of the project in `project_dir`.
///
/// The Build reads and checks the Backend, writes the interface and compares the contract, then chooses a path (spec 114
/// R13): the module path when the module serves the whole Backend and a runner is installed, the Cargo path otherwise, each
/// reason named. `opts.path` (`--cargo`, `--wasm`) overrides the project's `[backend] build_path`.
pub fn build_backend(project_dir: &Path, opts: &BackendBuildOptions) -> Result<BackendBuildResult, CompilerError> {
    let log = |m: &str| {
        if opts.verbose {
            eprintln!("{m}");
        }
    };
    let backend_dir = project_dir.join("backend");
    if !backend_dir.join("exports.toml").exists() {
        return Err(CompilerError::Workspace(format!(
            "{} has no exports.toml: a backend declares the programs it exports there (see the Developer's Guide, Backend)",
            backend_dir.display()
        )));
    }
    let mut phases: Vec<BackendPhase> = Vec::new();
    let began = Instant::now();

    // 1. Read and check. Nothing here touches Rust: the module path needs no toolchain.
    let state = backend_state(project_dir)?;
    let (project_path, external_crates) = path_settings(project_dir)?;
    let path = opts.path.unwrap_or(project_path);
    log(&format!("Building the backend of {}…", project_dir.display()));
    regenerate_identity(&backend_dir)?;
    let mut bundle = cobolt_server::bundle::Bundle::from_dir(&backend_dir).map_err(|errs| CompilerError::Parse { file: "backend".into(), message: errs.join("\n") })?;
    bundle.state = state;
    log(&format!("   state: {}", state.as_str()));
    let files = backend_module::program_files(project_dir);
    let exec_rust = exec_rust_of(&bundle.programs, &files);
    let bytes = bundle.to_bytes().map_err(CompilerError::Serialize)?;
    let mut features = runtime_features::RuntimeFeatures::default();
    for p in &bundle.programs {
        features = features.union(runtime_features::scan_program(p));
    }
    let programs_in_bundle = bundle.programs.len();
    let history = bundle.history.clone();
    let backend = bundle.into_backend().map_err(|errs| CompilerError::Semantic { file: "backend".into(), message: errs.join("\n") })?;
    // A generated program's declared control (WASM-005 T11) needs what its class reaches.
    for p in backend.programs() {
        if let Some(c) = &p.control {
            features = features.union(runtime_features::scan_control_class(&c.class));
        }
    }
    let contract = backend.interface.contract.clone();
    log(&format!("   {} program(s), {} exported, contract {contract}", programs_in_bundle, backend.interface.programs.len()));

    // 2. The interface description.
    let contract_dir = backend_dir.join("contract");
    std::fs::create_dir_all(&contract_dir).ctx(|| format!("create '{}'", contract_dir.display()))?;
    let interface_path = contract_dir.join("interface.json");
    std::fs::write(&interface_path, backend.interface.to_json()).ctx(|| format!("write '{}'", interface_path.display()))?;

    // 3. Compatibility with the last released contract (WASM-014 R5).
    let mut notes = Vec::new();
    let report = contract_report(&backend.interface, &history);
    // A connection that keeps its state runs its calls in turn (WASM-003 Q2).
    if state == cobolt_server::backend::StateMode::PerConnection {
        for p in backend.interface.programs.iter().filter(|p| p.independent) {
            let n = format!("{} is declared independent, which per-connection state ignores: a connection's calls run in turn", p.name);
            log(&format!("   note: {n}"));
            notes.push(n);
        }
    }
    for n in &report.notes {
        notes.push(n.clone());
        log(&format!("   note: {n}"));
    }
    if !report.breaking.is_empty() && !opts.accept_breaking && !opts.debug {
        return Err(CompilerError::Semantic {
            file: "backend/contract".into(),
            message: format!(
                "the contract {contract} breaks the last released contract {} — raise the major version in exports.toml deliberately, or change the program back:\n  {}",
                report.against,
                report.breaking.join("\n  ")
            ),
        });
    }
    phases.push(BackendPhase { name: "check", millis: began.elapsed().as_millis() });

    // 4. Which path (spec 114 R13, plan D8): the reasons that stop the module, in order, each named.
    let control_programs = control_programs_of(&backend);
    let mut reasons = backend_module::classify_backend(&ClassifyInput { path, external_crates, exec_rust, identity: backend_has_identity(&backend_dir), control_programs });
    let mut module = None;
    if reasons.is_empty() {
        let began = Instant::now();
        match backend_module::generate(&backend, &files) {
            Ok(m) => module = Some(m),
            Err(refused) => reasons.extend(refused),
        }
        phases.push(BackendPhase { name: "generate", millis: began.elapsed().as_millis() });
    }
    let mut runner = None;
    if let Some(m) = &module {
        // For another system the runner is the one that system's package is made with (verified by the caller): the module's
        // capabilities are checked against the same list, and its bill of materials is the file beside it.
        let given = opts.target.as_ref().map(|t| t.runner.runner.as_path()).or(opts.runner.as_deref());
        match backend_module::find_runner(given, m) {
            Ok(found) => runner = Some(found),
            Err(missing) => reasons.extend(missing),
        }
    }
    // Only the module path is portable (spec 115 R5): a Backend that needs Cargo is built on the system it will run on.
    if let (Some(target), false) = (&opts.target, reasons.is_empty()) {
        return Err(CompilerError::BackendForeignRefused { system: target.system.to_string(), reasons: reason_lines(&reasons) });
    }
    if path == BackendPath::Wasm && !reasons.is_empty() {
        return Err(CompilerError::BackendWasmRefused { reasons: reason_lines(&reasons) });
    }
    let name = sanitize_package_name(&project_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app".into()));
    let bin_name = format!("{name}-backend");
    let exe = match &opts.target {
        Some(target) => format!("{bin_name}{}", target.system.exe_suffix()),
        None if cfg!(windows) => format!("{bin_name}.exe"),
        None => bin_name.clone(),
    };

    if let (Some(module), Some(runner), true) = (&module, &runner, reasons.is_empty()) {
        log(&format!("🧩 WebAssembly module path: {} bytes of module, the Backend runner {}", module.wasm.len(), runner.runner.display()));
        let (bin_dir, staging) = match &opts.target {
            Some(target) => (target.bin_dir.clone(), build_staging_dir(None, &bin_name).join(format!("module-{}", target.system))),
            None => (project_dir.join("bin"), build_staging_dir(None, &bin_name).join("module")),
        };
        let made = backend_module::build_module(
            &ModuleBuild { backend: &backend, module, state, application: &name, runner, exe_name: &exe, bin_dir: &bin_dir, staging: &staging, machine_code: opts.target.is_none() },
            &log,
        )?;
        phases.extend(made.phases);
        notes.extend(made.notes);
        if let Some(target) = &opts.target {
            let n = format!("no machine code is made for {}: the Backend compiles its module when it first starts", target.system);
            log(&format!("   note: {n}"));
            notes.push(n);
        }
        return Ok(BackendBuildResult {
            binary: made.binary,
            interface: interface_path,
            contract,
            programs: backend.interface.programs.len(),
            notes,
            sbom: made.sbom,
            engine: BackendEngine::Module,
            reasons: Vec::new(),
            phases,
            module_sha256: Some(made.module_sha256),
            module_bytes: Some(made.module_bytes),
            machine_code_bytes: made.machine_code_bytes,
        });
    }

    // The Cargo path. A Backend the developer sent to Cargo is not told why; one the module does not serve is.
    for r in &reasons {
        log(&format!("   Cargo path: {r}"));
    }
    let chosen = reasons == [BackendCargoReason::Chosen];
    let wrap = |e: CompilerError| match e {
        CompilerError::Toolchain { tool, detail } if !chosen => CompilerError::BackendCargoNeedsToolchain { reasons: reason_lines(&reasons), tool, detail },
        other => other,
    };
    let began = Instant::now();
    let built = cargo_build(
        project_dir,
        opts,
        &CargoInputs { bytes: &bytes, features, contract: &contract, bin_name: &bin_name, exe: &exe },
        &wrap,
        &log,
    )?;
    phases.push(BackendPhase { name: "cargo", millis: began.elapsed().as_millis() });
    Ok(BackendBuildResult {
        binary: built.binary,
        interface: interface_path,
        contract,
        programs: backend.interface.programs.len(),
        notes,
        sbom: built.sbom,
        engine: BackendEngine::Cargo,
        reasons,
        phases,
        module_sha256: None,
        module_bytes: None,
        machine_code_bytes: None,
    })
}

/// What the Cargo path needs from the check: the packed programs, the runtime features they reach, the contract, the names.
struct CargoInputs<'a> {
    bytes: &'a [u8],
    features: runtime_features::RuntimeFeatures,
    contract: &'a str,
    bin_name: &'a str,
    exe: &'a str,
}

/// What the Cargo path installed.
struct CargoBuilt {
    binary: PathBuf,
    sbom: PathBuf,
}

/// The Cargo path: a generated Cargo project around the interpreter, built, installed, with its bill of materials.
fn cargo_build(
    project_dir: &Path,
    opts: &BackendBuildOptions,
    input: &CargoInputs<'_>,
    wrap: &dyn Fn(CompilerError) -> CompilerError,
    log: &impl Fn(&str),
) -> Result<CargoBuilt, CompilerError> {
    let CargoInputs { bytes, features, contract, bin_name, exe } = input;
    let (bin_name, exe) = (bin_name.to_string(), exe.to_string());
    let host = host_triple().map_err(wrap)?;
    log(&format!("Building the backend with Cargo for {host}…"));

    // 4. The generated crate.
    let root = resolve_workspace_root(opts.workspace_root.clone())
        .ok_or_else(|| CompilerError::Workspace("the PowerRustCOBOL platform crates were not found (Cargo.toml + crates/cobolt-server)".into()))?;
    let crates_path = root.join("crates");
    if !crates_path.join("cobolt-server").is_dir() {
        return Err(CompilerError::Workspace(format!("{} has no cobolt-server crate", crates_path.display())));
    }
    let build_dir = build_staging_dir(None, &bin_name);
    let src_dir = build_dir.join("src");
    let assets_dir = build_dir.join("assets");
    for d in [&src_dir, &assets_dir] {
        std::fs::create_dir_all(d).ctx(|| format!("create '{}'", d.display()))?;
    }
    std::fs::write(build_dir.join("Cargo.toml"), cargo_toml(&bin_name, contract, &crates_path, *features)).ctx(|| "write the backend's Cargo.toml".to_string())?;
    std::fs::write(src_dir.join("main.rs"), main_rs()).ctx(|| "write the backend's main.rs".to_string())?;
    std::fs::write(assets_dir.join("backend.bin"), bytes).ctx(|| "write the embedded programs".to_string())?;
    if let Some(config) = static_crt_config(&host) {
        let cargo_dir = build_dir.join(".cargo");
        std::fs::create_dir_all(&cargo_dir).ctx(|| format!("create '{}'", cargo_dir.display()))?;
        std::fs::write(cargo_dir.join("config.toml"), config).ctx(|| "write the backend's .cargo/config.toml".to_string())?;
        log("   linking the C runtime into the backend, so it starts on a Windows without the Visual C++ Redistributable (the first build compiles the platform for this setting and takes longer)");
    }
    log(&format!("   generated {}", build_dir.display()));

    // 5. cargo. The backend starts from the SDK's `Cargo.lock`, so it links the
    // very versions of the third-party crates the platform was tested with
    // (spec 101 R4); if this machine cannot supply a pinned version the seed is
    // dropped and cargo resolves as it would without it. The staging folder and
    // its `target/` are per project and kept between builds, so a second build
    // recompiles only what changed.
    let mut seeded = crate::build_cache::seed_lock(&root, &build_dir);
    if seeded {
        log("   starting Cargo.lock from the SDK's, so the backend uses the tested crate versions");
    }
    let target_dir = build_dir.join("target");
    let mut attempt_offline = true;
    loop {
        let mut cmd = crate::build_cache::cargo();
        cmd.arg("build");
        if !opts.debug {
            cmd.arg("--release");
        }
        if attempt_offline {
            cmd.arg("--offline");
        }
        let out = cmd
            .current_dir(&build_dir)
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .map_err(|e| wrap(toolchain_error("cargo", &e)))?;
        if out.status.success() {
            break;
        }
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        // The SDK's lock names a version this machine cannot supply: resolve afresh.
        if seeded && !stderr.contains("Compiling") && !stderr.contains("error[E") {
            let _ = std::fs::remove_file(build_dir.join("Cargo.lock"));
            seeded = false;
            continue;
        }
        // A cold cache fails resolution and says so; fetch and try again once.
        if attempt_offline && stderr.contains("--offline") {
            attempt_offline = false;
            continue;
        }
        if let Some(linker) = crate::missing_linker(&stderr) {
            return Err(CompilerError::LinkerMissing { linker });
        }
        let tail: String = stderr.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        return Err(CompilerError::CargoBuild { code: out.status.code().unwrap_or(-1), stderr: tail });
    }

    // 6. Install.
    let src_bin = target_dir.join(if opts.debug { "debug" } else { "release" }).join(&exe);
    let bin_dir = project_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).ctx(|| format!("create '{}'", bin_dir.display()))?;
    let dst = bin_dir.join(&exe);
    install_executable(&src_bin, &dst).ctx(|| format!("install the backend at '{}'", dst.display()))?;
    log(&format!("✅ Backend → {}", dst.display()));

    // 7. The SBOM beside the binary (WASM-013 R10): the packages cargo resolved for this machine's platform, and the
    // binary's hash. Asked for the platform the binary is built for: without one, `cargo metadata` lists the crates of
    // EVERY platform (an Android-only one included) and, offline, fails on a machine whose cache holds only what this
    // build needed (a fresh install, a CI runner): "failed to download `android_system_properties`". Offline first; online
    // once, as the build above does, if even the crates of this platform are not all there.
    let mut metadata_offline = true;
    let meta = loop {
        let out = crate::build_cache::cargo()
            .args(metadata_args(Some(&host), metadata_offline))
            .current_dir(&build_dir)
            .output()
            .map_err(|e| wrap(toolchain_error("cargo", &e)))?;
        if out.status.success() {
            break out;
        }
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if metadata_offline && stderr.contains("--offline") {
            metadata_offline = false;
            continue;
        }
        return Err(CompilerError::CargoBuild { code: out.status.code().unwrap_or(-1), stderr });
    };
    let binary_bytes = std::fs::read(&dst).ctx(|| format!("read '{}'", dst.display()))?;
    let bom = backend_sbom(&bin_name, &exe, &binary_bytes, &String::from_utf8_lossy(&meta.stdout)).map_err(|m| CompilerError::Semantic { file: "sbom.cdx.json".into(), message: m })?;
    let sbom_path = bin_dir.join("sbom.cdx.json");
    std::fs::write(&sbom_path, serde_json::to_string_pretty(&bom).expect("sbom")).ctx(|| format!("write '{}'", sbom_path.display()))?;
    log(&format!("   SBOM → {}", sbom_path.display()));
    Ok(CargoBuilt { binary: dst, sbom: sbom_path })
}

/// The arguments of the `cargo metadata` that lists the packages the SBOM names: for `host` when it is known, offline
/// when asked.
fn metadata_args(host: Option<&str>, offline: bool) -> Vec<String> {
    let mut args: Vec<String> = ["metadata", "--format-version", "1"].iter().map(|a| (*a).to_owned()).collect();
    if let Some(host) = host {
        args.push("--filter-platform".into());
        args.push(host.into());
    }
    if offline {
        args.push("--offline".into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_windows_msvc_backend_carries_its_c_runtime() {
        let on = static_crt_config("x86_64-pc-windows-msvc").expect("a Windows MSVC host links the runtime in");
        assert!(on.contains("[target.x86_64-pc-windows-msvc]") && on.contains("rustflags = [\"-C\", \"target-feature=+crt-static\"]"));
        assert!(static_crt_config("aarch64-pc-windows-msvc").is_some());
        for host in ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-gnu"] {
            assert!(static_crt_config(host).is_none(), "{host} must be left as it was");
        }
        // The text is valid TOML, so Cargo reads it.
        let parsed: toml::Value = on.parse().expect("valid TOML");
        assert_eq!(parsed["target"]["x86_64-pc-windows-msvc"]["rustflags"][1].as_str(), Some("target-feature=+crt-static"));
        println!("static CRT config: 7 hosts checked, written for 2 (Windows MSVC), absent for 5; TOML parses");
    }

    #[test]
    fn the_sbom_lists_the_packages_of_the_platform_the_binary_is_built_for() {
        // Offline first, for the platform: the crates of every other platform are not on a machine that did not need them.
        assert_eq!(metadata_args(Some("x86_64-unknown-linux-gnu"), true), ["metadata", "--format-version", "1", "--filter-platform", "x86_64-unknown-linux-gnu", "--offline"]);
        // The retry may use the network, still for the platform.
        assert_eq!(metadata_args(Some("x86_64-unknown-linux-gnu"), false), ["metadata", "--format-version", "1", "--filter-platform", "x86_64-unknown-linux-gnu"]);
        // `rustc` could not be asked: what the build did before.
        assert_eq!(metadata_args(None, true), ["metadata", "--format-version", "1", "--offline"]);
    }

    /// The failure that CI showed: a machine whose cargo home is cold builds online (only the crates this platform needs
    /// are downloaded) and `cargo metadata --offline` then asks for every platform's crates. A tiny crate with a dependency
    /// that is Android-only is built with a fresh cargo home, and the SBOM's own arguments must still answer. Needs the
    /// network once (crates.io); with none, the test says it was skipped.
    #[test]
    fn the_sbom_is_listed_on_a_machine_whose_cache_holds_only_what_the_build_needed() {
        let Ok(host) = host_triple() else {
            println!("skipped: rustc cannot be asked for its host");
            return;
        };
        let dir = std::env::temp_dir().join(format!("prc-sbom-cold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"cold\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[target.'cfg(target_os = \"android\")'.dependencies]\nandroid_system_properties = \"0.1\"\n").unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}\n").unwrap();
        let home = dir.join("cargo-home");
        std::fs::create_dir_all(&home).unwrap();
        let cargo = |args: &[String]| std::process::Command::new("cargo").args(args).env("CARGO_HOME", &home).current_dir(&dir).output().expect("cargo runs");
        let build = cargo(&["build".to_owned()]);
        if !build.status.success() {
            println!("skipped: the crates could not be fetched ({})", String::from_utf8_lossy(&build.stderr).lines().last().unwrap_or(""));
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let before = cargo(&metadata_args(None, true));
        assert!(!before.status.success() && String::from_utf8_lossy(&before.stderr).contains("--offline"), "without a platform the offline listing wants every platform's crates: {}", String::from_utf8_lossy(&before.stderr));
        let after = cargo(&metadata_args(Some(&host), true));
        assert!(after.status.success(), "for the host platform the offline listing needs only what the build downloaded: {}", String::from_utf8_lossy(&after.stderr));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

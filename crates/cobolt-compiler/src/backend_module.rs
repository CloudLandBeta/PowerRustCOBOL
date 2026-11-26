// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The module path of the Backend Build (spec 114 M4, R7, R13, R14, R17, R22).
//!
//! A Backend is built one of two ways. The **module path** turns the programs it exports (and the programs they `CALL`)
//! into one WebAssembly module, makes the module's machine code for this machine, and puts both into a copy of the
//! prebuilt headless runner (`prc-backend-runner`): no Rust toolchain is used and the Build takes seconds. The **Cargo
//! path** generates a small Cargo project around the interpreter and builds it, as every Backend was built before.
//!
//! [`classify_backend`] says which path a Backend takes and, for the Cargo path, **every** reason, so that a Backend that
//! falls back is never a surprise (R13). The reasons that stop the module path are checked in a fixed order (plan D8): the
//! developer chose Cargo; the project registers External Crates; an `EXEC RUST` block; `[identity]`; `per-connection`
//! state (until the runner serves it); a construct the generator refuses; then the runner: its file, its capabilities
//! and its software bill of materials. A Backend the module does not serve in full is never built as a module.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cobolt_server::backend::{Backend, StateMode};
use cobolt_wasm::{compile_backend, BackendProgram, Compiled, Options};

use crate::{CompilerError, IoContext};

/// The `[backend] build_path` value that lets the Build choose.
pub const BUILD_PATH_AUTO: &str = "auto";
/// The `[backend] build_path` value (and `--wasm`) that asks for the module path alone.
pub const BUILD_PATH_WASM: &str = "wasm";
/// The `[backend] build_path` value (and `--cargo`) that chooses Cargo.
pub const BUILD_PATH_CARGO: &str = "cargo";

/// The runner's file name, beside the IDE and `rcrun`.
pub const BACKEND_RUNNER_NAME: &str = if cfg!(windows) { "prc-backend-runner.exe" } else { "prc-backend-runner" };
/// The environment variable that names the Backend runner a Build uses (the file, as [`crate::RUNNER_ENV`] names the
/// Desktop runner's).
pub const BACKEND_RUNNER_ENV: &str = "PRC_BACKEND_RUNNER";
/// The runner's bill of materials, beside it: the release step writes it and the Build adds the module to it.
pub const BACKEND_RUNNER_SBOM: &str = "prc-backend-runner.sbom.json";

/// What a Backend Build is asked to do about its path: the developer's `[backend] build_path`, or `--cargo` / `--wasm`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackendPath {
    /// The module path when the module serves the Backend in full, Cargo otherwise.
    #[default]
    Auto,
    /// The module path alone: a Backend it does not serve is refused, not built another way.
    Wasm,
    /// The Cargo path, as every Backend was built before.
    Cargo,
}

impl BackendPath {
    /// The setting's value, read. An empty one is `auto`; any other word is refused naming the three.
    pub fn parse(text: &str) -> Result<BackendPath, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "" | BUILD_PATH_AUTO => Ok(BackendPath::Auto),
            BUILD_PATH_WASM => Ok(BackendPath::Wasm),
            BUILD_PATH_CARGO => Ok(BackendPath::Cargo),
            other => Err(format!("`[backend] build_path = \"{other}\"` is not a path: it is \"{BUILD_PATH_AUTO}\", \"{BUILD_PATH_WASM}\" or \"{BUILD_PATH_CARGO}\"")),
        }
    }

    /// The setting's word.
    pub fn as_str(self) -> &'static str {
        match self {
            BackendPath::Auto => BUILD_PATH_AUTO,
            BackendPath::Wasm => BUILD_PATH_WASM,
            BackendPath::Cargo => BUILD_PATH_CARGO,
        }
    }
}

/// The way a Backend Build made its executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendEngine {
    /// A WebAssembly module inside a copy of the prebuilt runner: no Rust toolchain was used.
    Module,
    /// A Cargo project around the interpreter.
    Cargo,
}

impl BackendEngine {
    /// The engine's name in a build log and a result block.
    pub fn name(self) -> &'static str {
        match self {
            BackendEngine::Module => "WebAssembly module",
            BackendEngine::Cargo => "Cargo",
        }
    }
}

/// Why a Backend takes the Cargo path (spec 114 R13), in the order the Build checks them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendCargoReason {
    /// The project chose Cargo (`[backend] build_path = "cargo"`) or the Build was asked for it (`--cargo`).
    Chosen,
    /// The project registers External Crates (`[[crates]]`), which only Cargo links.
    ExternalCrate,
    /// An `EXEC RUST` block: Rust compiled into the program. `file` is relative to the project.
    ExecRust { file: String, line: u32 },
    /// `backend.toml` has an `[identity]` section: its sign-in programs are not served by the module in this release.
    Identity,
    /// `[backend] state = "per-connection"`: the runner serves per-call Backends in this release.
    PerConnection,
    /// A construct the generator does not take, and why (its reason as the Desktop's Build words it). `line` is 0 when
    /// the construct is a whole file.
    Refused { construct: String, reason: String, file: String, line: u32 },
    /// The module needs a capability the runner does not provide.
    RunnerCapability { capability: String },
    /// No runner was found to put the module into.
    RunnerUnavailable { searched: String },
    /// The runner has no bill of materials beside it, and the Build writes one for every Backend (R17).
    RunnerSbomMissing { sbom: String },
}

impl std::fmt::Display for BackendCargoReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at = |file: &str, line: u32| if line > 0 { format!("{file} line {line}") } else { file.to_string() };
        match self {
            BackendCargoReason::Chosen => f.write_str("the Cargo path is chosen (the project's `[backend] build_path = \"cargo\"`, or `rcrun backend build --cargo`)"),
            BackendCargoReason::ExternalCrate => f.write_str("the project uses External Crates, which only Cargo links"),
            BackendCargoReason::ExecRust { file, line } => write!(f, "{} holds an EXEC RUST block, Rust compiled into the program", at(file, *line)),
            BackendCargoReason::Identity => f.write_str("backend.toml has an [identity] section, whose sign-in programs the module path does not serve yet"),
            BackendCargoReason::PerConnection => {
                f.write_str("the project's `[backend] state = \"per-connection\"` keeps WORKING-STORAGE for the life of a connection, which the module path does not serve yet")
            }
            BackendCargoReason::Refused { construct, reason, file, line } => write!(f, "{}: {construct}: {reason}", at(file, *line)),
            BackendCargoReason::RunnerCapability { capability } => {
                write!(f, "the program needs the capability `{capability}`, which the Backend runner ({BACKEND_RUNNER_NAME}) does not provide")
            }
            BackendCargoReason::RunnerUnavailable { searched } => {
                write!(f, "the Backend runner ({BACKEND_RUNNER_NAME}) was not found {searched}")
            }
            BackendCargoReason::RunnerSbomMissing { sbom } => write!(f, "the Backend runner has no bill of materials beside it ({sbom})"),
        }
    }
}

/// The time one phase of a Backend Build took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendPhase {
    pub name: &'static str,
    pub millis: u128,
}

/// The programs of a Backend and what the Build must know about them to choose a path.
pub struct ClassifyInput {
    /// What the developer or the command line asked for.
    pub path: BackendPath,
    /// The project registers External Crates.
    pub external_crates: bool,
    /// The first `EXEC RUST` line of each program that has one: its file relative to the project, and the line.
    pub exec_rust: Vec<(String, u32)>,
    /// `backend.toml` has an `[identity]` section.
    pub identity: bool,
    /// `[backend] state`.
    pub state: StateMode,
}

/// The reasons, before the generator is asked, that a Backend takes the Cargo path (plan D8, first half). Empty means the
/// generator is asked next. A Backend the developer sent to Cargo has that one reason and no other.
pub fn classify_backend(input: &ClassifyInput) -> Vec<BackendCargoReason> {
    if input.path == BackendPath::Cargo {
        return vec![BackendCargoReason::Chosen];
    }
    let mut reasons = Vec::new();
    if input.external_crates {
        reasons.push(BackendCargoReason::ExternalCrate);
    }
    for (file, line) in &input.exec_rust {
        reasons.push(BackendCargoReason::ExecRust { file: file.clone(), line: *line });
    }
    if input.identity {
        reasons.push(BackendCargoReason::Identity);
    }
    if input.state == StateMode::PerConnection {
        reasons.push(BackendCargoReason::PerConnection);
    }
    reasons
}

/// Where the programs of a Backend are, as `(PROGRAM-ID in capitals, file relative to the project)`, so that a refusal
/// can name the file of the statement. Read from the source text; a file whose PROGRAM-ID cannot be read is left out.
pub fn program_files(project_dir: &Path) -> Vec<(String, String)> {
    let backend = project_dir.join("backend");
    let mut out = Vec::new();
    for sub in ["src", "generated", "generated/identity"] {
        let dir = backend.join(sub);
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = read
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("cbl") || x.eq_ignore_ascii_case("cob")))
            .collect();
        files.sort();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            let rel = file.strip_prefix(project_dir).unwrap_or(&file).to_string_lossy().replace('\\', "/");
            for id in program_ids(&text) {
                out.push((id, rel.clone()));
            }
        }
    }
    out
}

/// Every `PROGRAM-ID` of a source text, in capitals.
fn program_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in text.lines() {
        let upper = line.to_ascii_uppercase();
        let Some(at) = upper.find("PROGRAM-ID") else { continue };
        if upper[..at].contains('*') {
            continue;
        }
        let rest = upper[at + "PROGRAM-ID".len()..].trim_start_matches(['.', ' ', '\t']);
        let id: String = rest.chars().take_while(|c| !c.is_whitespace() && *c != '.').collect();
        if !id.is_empty() {
            ids.push(id);
        }
    }
    ids
}

/// The module of a Backend, or every reason the generator gives for not making one (plan D8, second half): each construct
/// it refuses, with the file and line it is at.
pub fn generate(backend: &Backend, files: &[(String, String)]) -> Result<Compiled, Vec<BackendCargoReason>> {
    let mut programs: Vec<_> = backend.programs().collect();
    programs.sort_by(|a, b| a.sig.name.cmp(&b.sig.name));
    let mut list = Vec::new();
    for p in &programs {
        match p.program.as_ref() {
            Some(program) => list.push(BackendProgram { name: p.sig.name.clone(), program }),
            None => {
                return Err(vec![BackendCargoReason::Refused {
                    construct: "the program's source".into(),
                    reason: format!("{} has no source to compile", p.sig.name),
                    file: "backend".into(),
                    line: 0,
                }])
            }
        }
    }
    compile_backend(&list, &Options { native: true, ..Options::default() }).map_err(|e| {
        e.all_kinded()
            .into_iter()
            .map(|e| {
                let file = files.iter().find(|(id, _)| *id == e.program).map_or("backend", |(_, rel)| rel.as_str());
                let construct = if e.subject.is_empty() { "a statement".to_string() } else { e.subject.clone() };
                let reason = e.message.strip_prefix(&format!("{}: ", e.subject)).unwrap_or(&e.message).to_string();
                BackendCargoReason::Refused { construct, reason, file: file.to_string(), line: e.line }
            })
            .collect()
    })
}

/// A runner found for a Build: the file, and its bill of materials.
#[derive(Clone, Debug)]
pub struct RunnerFiles {
    pub runner: PathBuf,
    pub sbom: PathBuf,
}

/// Where the Backend runner is, if it is installed: the file a Build is given, else [`BACKEND_RUNNER_ENV`], else beside
/// the running executable (the IDE, `rcrun` and the runner ship together), as [`backend_runner_near`] finds it.
pub fn backend_runner_path(given: Option<&Path>) -> Result<PathBuf, BackendCargoReason> {
    if let Some(p) = given {
        return if p.is_file() { Ok(p.to_path_buf()) } else { Err(BackendCargoReason::RunnerUnavailable { searched: format!("at {}", p.display()) }) };
    }
    if let Some(p) = std::env::var_os(BACKEND_RUNNER_ENV).map(PathBuf::from) {
        return if p.is_file() { Ok(p) } else { Err(BackendCargoReason::RunnerUnavailable { searched: format!("at {} ({BACKEND_RUNNER_ENV})", p.display()) }) };
    }
    let exe = std::env::current_exe().ok();
    exe.as_deref().and_then(backend_runner_near).ok_or_else(|| BackendCargoReason::RunnerUnavailable {
        searched: match exe.as_deref().and_then(Path::parent) {
            Some(dir) => format!("in {}, or in {BACKEND_RUNNER_ENV}", dir.display()),
            None => format!("beside the program that builds, or in {BACKEND_RUNNER_ENV}"),
        },
    })
}

/// The Backend runner that ships with the program at `exe`: [`BACKEND_RUNNER_NAME`] in the same folder, or in the folder of
/// the file `exe` links to. A test or an example of a checkout finds none: it names its runner.
pub fn backend_runner_near(exe: &Path) -> Option<PathBuf> {
    let linked = exe.canonicalize().ok();
    let found = [Some(exe), linked.as_deref()].into_iter().flatten().filter_map(Path::parent).map(|dir| dir.join(BACKEND_RUNNER_NAME)).find(|p| p.is_file());
    found
}

/// The runner, with its bill of materials, and the capabilities the module needs that it lacks (plan D8, last): the
/// reasons when it cannot take the module.
pub fn find_runner(given: Option<&Path>, module: &Compiled) -> Result<RunnerFiles, Vec<BackendCargoReason>> {
    let runner = backend_runner_path(given).map_err(|r| vec![r])?;
    let sbom = runner.with_file_name(BACKEND_RUNNER_SBOM);
    let mut reasons = Vec::new();
    let needs: Vec<&str> = module.manifest["capabilities"].as_array().map(|a| a.iter().filter_map(|c| c.as_str()).collect()).unwrap_or_default();
    for capability in needs {
        if !cobolt_wasm_abi::codes::cap::BACKEND_RUNNER.contains(&capability) {
            reasons.push(BackendCargoReason::RunnerCapability { capability: capability.to_string() });
        }
    }
    if !sbom.is_file() {
        reasons.push(BackendCargoReason::RunnerSbomMissing { sbom: sbom.display().to_string() });
    }
    if reasons.is_empty() {
        Ok(RunnerFiles { runner, sbom })
    } else {
        Err(reasons)
    }
}

/// The lower-case hex of a SHA-256.
fn hex(bytes: &[u8]) -> String {
    cobolt_runner_engine::sha256(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The bill of materials of a Backend built as a module (R17): the executable and the module with their hashes, then the
/// libraries the runner links, taken from the runner's own bill of materials.
pub fn module_sbom(application: &str, exe_name: &str, exe: &[u8], module: &[u8], runner_sbom: &str) -> Result<serde_json::Value, String> {
    let runner: serde_json::Value = serde_json::from_str(runner_sbom).map_err(|e| format!("the runner's bill of materials is unreadable: {e}"))?;
    let components = runner["components"].as_array().ok_or("the runner's bill of materials lists no components")?;
    let mut out = vec![cobolt_wire::sbom::file_component(exe_name, &hex(exe)), cobolt_wire::sbom::file_component(&format!("{application}.module.wasm"), &hex(module))];
    out.extend(components.iter().filter(|c| c["type"].as_str() == Some("library")).cloned());
    Ok(cobolt_wire::sbom::cyclonedx(application, out, None))
}

/// What a Backend module build needs besides the module.
pub struct ModuleBuild<'a> {
    pub backend: &'a Backend,
    pub module: &'a Compiled,
    pub state: StateMode,
    /// The application's name: what the SQL connection overrides (`<NAME>_SQL_<CONNECTION>_URL`) are named after.
    pub application: &'a str,
    pub runner: &'a RunnerFiles,
    /// The file's name in `bin/`.
    pub exe_name: &'a str,
    pub bin_dir: &'a Path,
    /// Where the file is made before it is installed.
    pub staging: &'a Path,
}

/// What a module build made.
pub struct ModuleBuilt {
    pub binary: PathBuf,
    pub sbom: PathBuf,
    /// The module's SHA-256, lower-case hex.
    pub module_sha256: String,
    pub module_bytes: usize,
    /// The machine code's size, when it was made.
    pub machine_code_bytes: Option<usize>,
    pub phases: Vec<BackendPhase>,
    pub notes: Vec<String>,
}

/// Put the module into a copy of the runner and install it as `bin/<name>-backend`, with its bill of materials (spec 114
/// T28, T29): machine code for the baseline CPU of this machine when it can be made (the module is compiled when the file
/// first starts when it cannot), then the container, the payload, the copy, and the file installed by renaming it into
/// place.
pub fn build_module(b: &ModuleBuild<'_>, log: &impl Fn(&str)) -> Result<ModuleBuilt, CompilerError> {
    use cobolt_program_package::backend::BackendContainer;
    use cobolt_program_package::Kind;
    let mut phases = Vec::new();
    let mut notes = Vec::new();
    let wasm = b.module.wasm.as_slice();
    for w in &b.module.warnings {
        let n = format!("{w}");
        log(&format!("   note: {n}"));
        notes.push(n);
    }

    let began = Instant::now();
    let host = cobolt_runner_engine::Target::host().triple();
    let code = match cobolt_runner_engine::Target::baseline(&host).and_then(|t| cobolt_runner_engine::precompile_backend(wasm, &t)) {
        Ok(code) => {
            log(&format!("   machine code for {host}: {} bytes", code.bytes.len()));
            Some(code)
        }
        Err(e) => {
            let n = format!("no machine code was made ({e}): the Backend compiles its module when it first starts");
            log(&format!("   note: {n}"));
            notes.push(n);
            None
        }
    };
    phases.push(BackendPhase { name: "machine code", millis: began.elapsed().as_millis() });

    let began = Instant::now();
    let runner_failure = |detail: String| CompilerError::Runner { path: b.runner.runner.display().to_string(), detail };
    let container = BackendContainer {
        state: b.state.as_str().to_string(),
        interface: b.backend.interface.to_json().into_bytes(),
        exports: serde_json::to_vec(&b.backend.export_entries()).map_err(|e| CompilerError::Serialize(e.to_string()))?,
        history: serde_json::to_vec(&b.backend.history).map_err(|e| CompilerError::Serialize(e.to_string()))?,
        slots: serde_json::to_vec(&b.backend.slots).map_err(|e| CompilerError::Serialize(e.to_string()))?,
        module: wasm.to_vec(),
        name: b.application.to_string(),
    };
    let container_bytes = container.encode();
    let mut parts: Vec<(Kind, &[u8])> = vec![(Kind::Backend, container_bytes.as_slice())];
    if let Some(code) = &code {
        parts.push((Kind::MachineCode, code.bytes.as_slice()));
        parts.push((Kind::MachineCodeEngine, &code.compat[..]));
    }
    let payload = cobolt_program_package::encode(&parts);
    let runner_bytes = std::fs::read(&b.runner.runner).map_err(|e| runner_failure(e.to_string()))?;
    let exe = cobolt_program_package::embed(&runner_bytes, &payload, b.exe_name).map_err(|e| runner_failure(e.to_string()))?;
    std::fs::create_dir_all(b.staging).ctx(|| format!("create '{}'", b.staging.display()))?;
    let staged = b.staging.join(b.exe_name);
    cobolt_program_package::write_executable(&staged, &exe).ctx(|| format!("write the Backend '{}'", staged.display()))?;
    phases.push(BackendPhase { name: "assemble", millis: began.elapsed().as_millis() });

    let began = Instant::now();
    std::fs::create_dir_all(b.bin_dir).ctx(|| format!("create '{}'", b.bin_dir.display()))?;
    let dst = b.bin_dir.join(b.exe_name);
    crate::install_executable(&staged, &dst).ctx(|| format!("install the Backend at '{}' (made at '{}')", dst.display(), staged.display()))?;
    log(&format!("✅ Backend → {} ({} bytes)", dst.display(), exe.len()));

    let runner_sbom = std::fs::read_to_string(&b.runner.sbom).ctx(|| format!("read '{}'", b.runner.sbom.display()))?;
    let bom = module_sbom(b.application, b.exe_name, &exe, wasm, &runner_sbom).map_err(|m| CompilerError::Semantic { file: "sbom.cdx.json".into(), message: m })?;
    let sbom = b.bin_dir.join("sbom.cdx.json");
    std::fs::write(&sbom, serde_json::to_string_pretty(&bom).expect("sbom")).ctx(|| format!("write '{}'", sbom.display()))?;
    log(&format!("   SBOM → {}", sbom.display()));
    phases.push(BackendPhase { name: "install", millis: began.elapsed().as_millis() });

    Ok(ModuleBuilt { binary: dst, sbom, module_sha256: hex(wasm), module_bytes: wasm.len(), machine_code_bytes: code.as_ref().map(|c| c.bytes.len()), phases, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(path: BackendPath) -> ClassifyInput {
        ClassifyInput { path, external_crates: false, exec_rust: Vec::new(), identity: false, state: StateMode::PerCall }
    }

    #[test]
    fn the_setting_is_read_and_an_unknown_word_names_the_three() {
        let mut tried = 0;
        for (text, want) in [("", BackendPath::Auto), ("auto", BackendPath::Auto), ("AUTO", BackendPath::Auto), (" wasm ", BackendPath::Wasm), ("cargo", BackendPath::Cargo)] {
            assert_eq!(BackendPath::parse(text), Ok(want), "{text:?}");
            tried += 1;
        }
        let err = BackendPath::parse("rust").expect_err("not a path");
        assert!(err.contains("\"auto\"") && err.contains("\"wasm\"") && err.contains("\"cargo\"") && err.contains("rust"), "{err}");
        assert_eq!(BackendPath::default(), BackendPath::Auto);
        for p in [BackendPath::Auto, BackendPath::Wasm, BackendPath::Cargo] {
            assert_eq!(BackendPath::parse(p.as_str()), Ok(p));
        }
        println!("build_path: {tried} spellings read, 1 unknown word refused naming the three, the default is auto; PASS 009 FAIL 000");
    }

    #[test]
    fn a_backend_with_nothing_the_module_refuses_goes_on_to_the_generator() {
        assert_eq!(classify_backend(&input(BackendPath::Auto)), Vec::<BackendCargoReason>::new());
        assert_eq!(classify_backend(&input(BackendPath::Wasm)), Vec::<BackendCargoReason>::new(), "--wasm asks for the module and refuses nothing here");
    }

    #[test]
    fn each_reason_alone_sends_the_backend_to_cargo_and_names_itself() {
        let mut one = input(BackendPath::Auto);
        one.external_crates = true;
        assert_eq!(classify_backend(&one), vec![BackendCargoReason::ExternalCrate]);

        let mut one = input(BackendPath::Auto);
        one.exec_rust = vec![("backend/src/a.cbl".into(), 12)];
        assert_eq!(classify_backend(&one), vec![BackendCargoReason::ExecRust { file: "backend/src/a.cbl".into(), line: 12 }]);

        let mut one = input(BackendPath::Auto);
        one.identity = true;
        assert_eq!(classify_backend(&one), vec![BackendCargoReason::Identity]);

        let mut one = input(BackendPath::Auto);
        one.state = StateMode::PerConnection;
        assert_eq!(classify_backend(&one), vec![BackendCargoReason::PerConnection]);

        // The developer's choice is the only reason, whatever else holds.
        let mut chosen = input(BackendPath::Cargo);
        chosen.external_crates = true;
        chosen.identity = true;
        assert_eq!(classify_backend(&chosen), vec![BackendCargoReason::Chosen]);
        println!("classify_backend: 5 rules, each alone names itself, and Chosen stands alone; PASS 005 FAIL 000");
    }

    #[test]
    fn several_reasons_are_all_listed_in_the_order_of_the_plan() {
        let mut many = input(BackendPath::Auto);
        many.external_crates = true;
        many.exec_rust = vec![("backend/src/a.cbl".into(), 3), ("backend/src/b.cbl".into(), 9)];
        many.identity = true;
        many.state = StateMode::PerConnection;
        let got = classify_backend(&many);
        assert_eq!(
            got,
            vec![
                BackendCargoReason::ExternalCrate,
                BackendCargoReason::ExecRust { file: "backend/src/a.cbl".into(), line: 3 },
                BackendCargoReason::ExecRust { file: "backend/src/b.cbl".into(), line: 9 },
                BackendCargoReason::Identity,
                BackendCargoReason::PerConnection,
            ]
        );
        for r in &got {
            assert!(!r.to_string().is_empty());
            assert!(!r.to_string().to_ascii_lowercase().contains("cobolt"), "no user-facing text says cobolt: {r}");
        }
        println!("classify_backend: 5 reasons from 4 rules listed in order, each worded; PASS 007 FAIL 000");
    }

    #[test]
    fn the_program_ids_of_a_source_are_read_in_either_format() {
        assert_eq!(program_ids("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. Hello.\n"), vec!["HELLO"]);
        assert_eq!(program_ids("IDENTIFICATION DIVISION.\nPROGRAM-ID. ADDER.\n...\nPROGRAM-ID. INNER.\n"), vec!["ADDER", "INNER"]);
        assert_eq!(program_ids("      * PROGRAM-ID. NOT-ONE.\n       PROGRAM-ID. REAL.\n"), vec!["REAL"]);
        assert_eq!(program_ids("PROGRAM-ID.   spaced   .\n"), vec!["SPACED"]);
        assert!(program_ids("DISPLAY 'x'.\n").is_empty());
    }

    #[test]
    fn the_modules_bill_of_materials_lists_the_files_and_the_runners_libraries() {
        let runner = serde_json::json!({
            "components": [
                cobolt_wire::sbom::file_component("prc-backend-runner", "00"),
                cobolt_wire::sbom::library_component("wasmtime", "44.0.3", "Apache-2.0"),
                cobolt_wire::sbom::library_component("tokio", "1.40.0", "MIT"),
            ]
        })
        .to_string();
        let bom = module_sbom("oficina", "oficina-backend", b"the exe", b"the module", &runner).expect("a bill");
        let comps = bom["components"].as_array().unwrap();
        let names: Vec<&str> = comps.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["oficina-backend", "oficina.module.wasm", "wasmtime", "tokio"], "the runner's own file is replaced by the Backend's");
        assert_eq!(comps[1]["hashes"][0]["content"].as_str().unwrap().len(), 64, "the module's SHA-256 is recorded");
        assert!(module_sbom("a", "a", b"", b"", "not json").is_err());
        assert!(module_sbom("a", "a", b"", b"", "{}").is_err());
        println!("module bill of materials: 2 files + 2 libraries listed, 2 unreadable inputs refused; PASS 005 FAIL 000");
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The backend project: its programs, the ones it exports, and the interface
//! description they make (WASM-003 R1, R4).
//!
//! A backend is the `backend/` folder of a Web or Mobile project:
//!
//! ```text
//! backend/
//!   exports.toml     which programs are exported, to whom, and how they may be repeated
//!   backend.toml     the deployment: address, TLS, issuer, limits (config.rs)
//!   src/*.cbl        the programs; those named in exports.toml are reachable, the rest are
//!                    ordinary procedures the exported ones CALL
//!   COPYBOOKS/       shared layouts (COPY), the same files the frontend uses
//!   contract/interface.json            written by Build, imported by the frontend's Check
//!   contract/released/<version>.json   the interface of each released contract
//! ```
//!
//! **Only an exported program can be called from the channel**, whatever its
//! name: the table below is the whole of what the server will run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cobolt_ast::program::{DataSection, Program};
use serde::{Deserialize, Serialize};

use cobolt_wire::contract::{Deprecation, Direction, InterfaceDescription, Param, ProgramSig};
use cobolt_wire::layout::from_decl;

/// Whether a call's `WORKING-STORAGE` outlives it — the project's `[backend]
/// state` (WASM-003 Q2, decided by the operator 2026-10-22: a project property,
/// fresh per call by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StateMode {
    /// Every call starts with fresh `WORKING-STORAGE`.
    #[default]
    PerCall,
    /// A connection keeps one run unit per exported program it has called;
    /// its calls take turns, and it ends with the connection.
    PerConnection,
}

impl StateMode {
    pub const PER_CALL: &'static str = "per-call";
    pub const PER_CONNECTION: &'static str = "per-connection";

    /// The manifest's spelling; an empty value is the default.
    pub fn parse(text: &str) -> Result<StateMode, String> {
        match text.trim() {
            "" | Self::PER_CALL => Ok(StateMode::PerCall),
            Self::PER_CONNECTION => Ok(StateMode::PerConnection),
            other => Err(format!(
                "[backend] state \"{other}\" is not one of \"{}\" (fresh WORKING-STORAGE at every call, the default) or \"{}\" (kept for the connection)",
                Self::PER_CALL,
                Self::PER_CONNECTION
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            StateMode::PerCall => Self::PER_CALL,
            StateMode::PerConnection => Self::PER_CONNECTION,
        }
    }
}

/// The names `backend_dir` exports, in file order — the developer's
/// `exports.toml`, then the Build's `generated/exports.toml`; empty when they
/// cannot be read.
pub fn exported_names(backend_dir: &Path) -> Vec<String> {
    read_exports(backend_dir).map(|(_, f)| f.exports.into_iter().map(|e| e.program).collect()).unwrap_or_default()
}

/// The folder, under `backend/`, where the Build writes the programs of a web
/// form's data controls and their `exports.toml` (WASM-005 R16): the Build's,
/// never the developer's, which is why it is read beside `exports.toml` and
/// `src/` rather than merged into them.
pub const GENERATED_DIR: &str = "generated";

/// `generated/exports.toml`: `[[export]]` entries only — the contract version
/// is the developer's.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedExports {
    #[serde(default, rename = "export")]
    exports: Vec<ExportDecl>,
}

/// The exports of `dir` as one declaration, and its text: `exports.toml`, with
/// the entries of `generated/exports.toml` after it when that file exists. A
/// program both files export is an error naming it. The text is what a packed
/// backend carries, so a built backend reads exactly what `load` read.
pub fn read_exports(dir: &Path) -> Result<(String, ExportsFile), Vec<String>> {
    let path = dir.join("exports.toml");
    let mut text = std::fs::read_to_string(&path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
    let mine = ExportsFile::from_toml(&text).map_err(|e| vec![e])?;
    let gen_path = dir.join(GENERATED_DIR).join("exports.toml");
    let Ok(gen_text) = std::fs::read_to_string(&gen_path) else {
        return Ok((text, mine));
    };
    let generated: GeneratedExports =
        toml::from_str(&gen_text).map_err(|e| vec![format!("{GENERATED_DIR}/exports.toml: {e}")])?;
    let clashes: Vec<String> = generated
        .exports
        .iter()
        .filter(|g| mine.exports.iter().any(|m| m.program.eq_ignore_ascii_case(&g.program)))
        .map(|g| {
            format!(
                "exports.toml: {} is exported by exports.toml and by the Build's {GENERATED_DIR}/exports.toml; rename the program in backend/src",
                g.program.to_ascii_uppercase()
            )
        })
        .collect();
    if !clashes.is_empty() {
        return Err(clashes);
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(&gen_text);
    let merged = ExportsFile::from_toml(&text).map_err(|e| vec![e])?;
    Ok((text, merged))
}

/// The programs of `dir`: `src/*.cbl`, then the Build's `generated/*.cbl`
/// (free format, as every generated program). A generated program whose
/// PROGRAM-ID a program of `src` also has is an error naming both.
pub fn load_all_programs(dir: &Path, errs: &mut Vec<String>) -> Vec<Program> {
    let mut programs = load_programs(&dir.join("src"), errs);
    let generated_dir = dir.join(GENERATED_DIR);
    if generated_dir.is_dir() {
        for p in load_programs_as(&generated_dir, Some(cobolt_lexer::SourceFormat::Free), errs) {
            let id = &p.identification.program_id;
            if programs.iter().any(|q| q.identification.program_id.eq_ignore_ascii_case(id)) {
                errs.push(format!(
                    "backend/src has a program with the PROGRAM-ID {} of a program the Build generated in {GENERATED_DIR}/; rename it",
                    id.to_ascii_uppercase()
                ));
                continue;
            }
            programs.push(p);
        }
    }
    programs
}

/// The `[[export]]` entries of `exports.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportDecl {
    /// The PROGRAM-ID of the exported program.
    pub program: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub idempotent: bool,
    #[serde(default)]
    pub independent: bool,
    #[serde(default)]
    pub long_running: bool,
    #[serde(default)]
    pub stream: bool,
    /// One of `in`, `out`, `inout` per parameter; default `inout`.
    #[serde(default)]
    pub directions: Vec<String>,
    #[serde(default)]
    pub deprecated: Option<DeprecatedDecl>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeprecatedDecl {
    pub message: String,
    pub removal: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDecl {
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportsFile {
    pub contract: ContractDecl,
    #[serde(default, rename = "export")]
    pub exports: Vec<ExportDecl>,
}

impl ExportsFile {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let f: ExportsFile = toml::from_str(text).map_err(|e| format!("exports.toml: {e}"))?;
        if !is_semver(&f.contract.version) {
            return Err(format!("exports.toml: contract.version \"{}\" is not major.minor.patch", f.contract.version));
        }
        for e in &f.exports {
            for d in &e.directions {
                if !matches!(d.as_str(), "in" | "out" | "inout") {
                    return Err(format!("exports.toml: export {}: direction \"{d}\" is not in, out or inout", e.program));
                }
            }
        }
        Ok(f)
    }
}

pub fn is_semver(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3 && p.iter().all(|x| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()))
}

/// `major.minor.patch` as numbers.
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let p: Vec<u64> = v.split('.').map(|x| x.parse().ok()).collect::<Option<Vec<_>>>()?;
    (p.len() == 3).then(|| (p[0], p[1], p[2]))
}

/// One exported program, ready to run.
pub struct ExportedProgram {
    pub sig: ProgramSig,
    /// The top-level program, with every other backend program nested in it.
    pub program: Program,
    /// The names of the `USING` parameters, in order.
    pub params: Vec<String>,
    pub returning: Option<String>,
}

/// Everything the server runs.
pub struct Backend {
    pub interface: InterfaceDescription,
    /// Released contracts, oldest first (for answering older clients, WASM-014 R4).
    pub history: Vec<InterfaceDescription>,
    exports: HashMap<String, Arc<ExportedProgram>>,
    /// Whether `WORKING-STORAGE` outlives a call (the project's `[backend] state`).
    pub state: StateMode,
}

impl Backend {
    pub fn program(&self, name: &str) -> Option<&Arc<ExportedProgram>> {
        self.exports.get(&name.trim().to_ascii_uppercase())
    }

    pub fn programs(&self) -> impl Iterator<Item = &Arc<ExportedProgram>> {
        self.exports.values()
    }

    /// The interface the contract version `v` described, for projecting an
    /// answer to the layouts an older client was built against: the newest
    /// released contract of `v`'s major that is not newer than `v`.
    pub fn interface_for(&self, v: &str) -> Option<&InterfaceDescription> {
        let want = parse_semver(v)?;
        if parse_semver(&self.interface.contract) == Some(want) {
            return Some(&self.interface);
        }
        self.history
            .iter()
            .filter_map(|h| parse_semver(&h.contract).map(|s| (s, h)))
            .filter(|(s, _)| s.0 == want.0 && *s <= want)
            .max_by_key(|(s, _)| *s)
            .map(|(_, h)| h)
    }

    /// Build from parsed programs and the exports declaration.
    pub fn build(programs: Vec<Program>, exports: &ExportsFile, history: Vec<InterfaceDescription>) -> Result<Backend, Vec<String>> {
        let mut errs = Vec::new();
        let mut map: HashMap<String, Arc<ExportedProgram>> = HashMap::new();
        let mut interface = InterfaceDescription::new(&exports.contract.version);

        let exported_names: Vec<String> = exports.exports.iter().map(|e| e.program.to_ascii_uppercase()).collect();
        for decl in &exports.exports {
            let name = decl.program.to_ascii_uppercase();
            if map.contains_key(&name) {
                errs.push(format!("exports.toml: {name} is exported twice"));
                continue;
            }
            let Some(prog) = programs.iter().find(|p| p.identification.program_id.eq_ignore_ascii_case(&name)) else {
                errs.push(format!("exports.toml: no program has PROGRAM-ID {name} in backend/src"));
                continue;
            };
            let mut sig = ProgramSig::new(&name);
            sig.roles = decl.roles.clone();
            sig.public = decl.public;
            sig.idempotent = decl.idempotent;
            sig.independent = decl.independent;
            sig.long_running = decl.long_running;
            sig.stream = decl.stream;
            sig.deprecated = decl.deprecated.as_ref().map(|d| Deprecation { message: d.message.clone(), removal: d.removal.clone() });

            let linkage: Vec<&cobolt_ast::data::DataDecl> = prog
                .data
                .iter()
                .flat_map(|d| d.sections.iter())
                .filter_map(|s| if let DataSection::Linkage(items) = s { Some(items.iter()) } else { None })
                .flatten()
                .collect();
            let layout_of = |pname: &str, direction: Direction, errs: &mut Vec<String>| -> Option<Param> {
                let Some(d) = linkage.iter().find(|d| d.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(pname))) else {
                    errs.push(format!("{name}: parameter {pname} is not declared in its LINKAGE SECTION"));
                    return None;
                };
                match from_decl(d) {
                    Ok(layout) => Some(Param { name: pname.to_string(), direction, layout }),
                    Err(e) => {
                        errs.push(format!("{name}: parameter {pname} cannot travel — {e}"));
                        None
                    }
                }
            };
            let params_names = prog.procedure.using.clone();
            if !decl.directions.is_empty() && decl.directions.len() != params_names.len() {
                errs.push(format!("exports.toml: export {name} lists {} directions for {} parameters", decl.directions.len(), params_names.len()));
            }
            for (i, pn) in params_names.iter().enumerate() {
                let dir = match decl.directions.get(i).map(String::as_str) {
                    Some("in") => Direction::In,
                    Some("out") => Direction::Out,
                    _ => Direction::InOut,
                };
                if let Some(p) = layout_of(pn, dir, &mut errs) {
                    sig.params.push(p);
                }
            }
            if let Some(r) = &prog.procedure.returning {
                if let Some(p) = layout_of(r, Direction::Out, &mut errs) {
                    sig.returning = Some(p);
                }
            }

            // Every other program rides inside this one, as Common Code does.
            let mut top = prog.clone();
            for other in &programs {
                if !other.identification.program_id.eq_ignore_ascii_case(&name) {
                    // Another exported program is a procedure here too: it may be CALLed locally.
                    top.nested_programs.push(other.clone());
                }
            }
            interface.programs.push(sig.clone());
            map.insert(
                name.clone(),
                Arc::new(ExportedProgram { sig, program: top, params: params_names, returning: prog.procedure.returning.clone() }),
            );
        }
        let _ = exported_names;
        if !errs.is_empty() {
            return Err(errs);
        }
        interface.programs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Backend { interface, history, exports: map, state: StateMode::default() })
    }

    /// Read a backend folder: `exports.toml`, `src/*.cbl`, the Build's
    /// `generated/` programs and exports when there are any, the released
    /// interfaces. Every program is parsed and analysed; the diagnostics of all
    /// of them are reported together.
    pub fn load(dir: &Path) -> Result<Backend, Vec<String>> {
        let (_, exports) = read_exports(dir)?;
        let mut errs = Vec::new();
        let programs = load_all_programs(dir, &mut errs);
        let history = load_history(&dir.join("contract").join("released"), &mut errs);
        if !errs.is_empty() {
            return Err(errs);
        }
        Backend::build(programs, &exports, history)
    }
}

/// The source format of a hand-written file: fixed when a line carries a
/// sequence area followed by an indicator, free otherwise.
pub fn source_format(source: &str) -> cobolt_lexer::SourceFormat {
    if source.lines().any(|l| {
        let b = l.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    }) {
        cobolt_lexer::SourceFormat::Fixed
    } else {
        cobolt_lexer::SourceFormat::Free
    }
}

/// Parse and analyse every `*.cbl` / `*.cob` under `src`.
pub fn load_programs(src: &Path, errs: &mut Vec<String>) -> Vec<Program> {
    load_programs_as(src, None, errs)
}

/// [`load_programs`], in `format` when given, else each file's own
/// ([`source_format`]).
fn load_programs_as(src: &Path, format: Option<cobolt_lexer::SourceFormat>, errs: &mut Vec<String>) -> Vec<Program> {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(src) {
        Ok(rd) => rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case("cbl") || x.eq_ignore_ascii_case("cob")))
            .collect(),
        Err(e) => {
            errs.push(format!("{}: {e}", src.display()));
            return Vec::new();
        }
    };
    files.sort();
    let mut programs = Vec::new();
    for f in files {
        let text = match std::fs::read_to_string(&f) {
            Ok(t) => t,
            Err(e) => {
                errs.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        match parse_program_as(&text, &f, format.unwrap_or_else(|| source_format(&text))) {
            Ok(p) => programs.push(p),
            Err(mut e) => errs.append(&mut e),
        }
    }
    programs
}

/// Lex (with COPY expanded), parse and analyse one source.
pub fn parse_program(text: &str, path: &Path) -> Result<Program, Vec<String>> {
    parse_program_as(text, path, source_format(text))
}

/// [`parse_program`] in a given source format.
pub fn parse_program_as(text: &str, path: &Path, format: cobolt_lexer::SourceFormat) -> Result<Program, Vec<String>> {
    let tokens = match cobolt_lexer::preprocess_program(text, path, format) {
        Some(exp) => {
            if !exp.errors.is_empty() {
                return Err(exp.errors.iter().map(|e| format!("{}: copybook: {e}", path.display())).collect());
            }
            cobolt_lexer::tokenize_expansion(&exp)
        }
        None => cobolt_lexer::tokenize(text, format),
    };
    let parsed = cobolt_parser::parse(tokens);
    let mut errs: Vec<String> = parsed
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .map(|d| format!("{}:{}: {}", path.display(), d.span.line, d.message))
        .collect();
    let Some(program) = parsed.program else {
        errs.push(format!("{}: no program", path.display()));
        return Err(errs);
    };
    let sem = cobolt_semantic::analyze(&program);
    errs.extend(sem.errors().map(|d| format!("{}:{}: {}", path.display(), d.span.line, d.message)));
    if errs.is_empty() {
        Ok(program)
    } else {
        Err(errs)
    }
}

/// The interfaces of released contracts, oldest first.
pub fn load_history(dir: &Path, errs: &mut Vec<String>) -> Vec<InterfaceDescription> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<InterfaceDescription> = Vec::new();
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| InterfaceDescription::from_json(&t)) {
            Ok(d) => out.push(d),
            Err(e) => errs.push(format!("{}: {e}", p.display())),
        }
    }
    out.sort_by_key(|d| parse_semver(&d.contract));
    out
}

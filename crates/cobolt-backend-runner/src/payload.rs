// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Backend inside the runner's executable (spec 114 T20, R2, R3, R7).
//!
//! A Build copies the runner, puts the payload inside the copy and names the copy after the project. The payload's
//! `Kind::Backend` part holds the interface description, the table of exports, the released interfaces, the upload slots,
//! the state mode and the WebAssembly module; its machine code, when the Build made it, is in the parts the Desktop's
//! payload already has. This module reads them and makes the `Backend` the server serves, with no source: every program
//! the interface lists has an entry in the table and the module runs it.
//!
//! A module that declares a capability this runner does not provide is refused here, **by name, before the server
//! starts** (R2): a Backend that needs a database must not start on a runner that has none and fail at its first call.

use std::sync::Arc;

use cobolt_program_package::backend::BackendContainer;
use cobolt_program_package::{decode, locate, Kind, PackageError};
use cobolt_runner_engine::MachineCode;
use cobolt_server::backend::{Backend, ExportEntry, SlotDecl, StateMode};
use cobolt_server::standalone::Loaded;
use cobolt_wire::contract::InterfaceDescription;

use crate::engine::WasmEngine;

/// What a payload carries, read: the container and the machine code the Build made for the module, when it did.
pub struct Package {
    pub container: BackendContainer,
    pub code: Option<MachineCode>,
}

/// The payload of the executable `exe` (its bytes), read. `Err` says, in words, what is wrong with the file.
pub fn read(exe: &[u8]) -> Result<Package, String> {
    let bytes = match locate(exe) {
        Ok(p) => p,
        Err(PackageError::NoPayload) => {
            return Err("this is a bare Backend runner: it carries no program. Build a Backend with `rcrun backend build` and run the file that makes".into())
        }
        Err(e) => return Err(format!("the Backend this file carries cannot be read: {e}")),
    };
    let payload = decode(bytes).map_err(|e| format!("the Backend this file carries cannot be read: {e}"))?;
    let container = payload.backend().map_err(|e| match e {
        // A Desktop program's payload has no Backend part; a newer Build's container is named as newer.
        PackageError::Damaged(m) if m.contains("no BACKEND part") => "this file carries a Desktop program, not a Backend".to_string(),
        other => format!("the Backend this file carries cannot be read: {other}"),
    })?;
    let code = payload.part(Kind::MachineCode).zip(payload.part(Kind::MachineCodeEngine)).and_then(|(code, engine)| {
        let compat: [u8; 32] = engine.bytes.try_into().ok()?;
        Some(MachineCode { bytes: code.bytes.to_vec(), sha256: code.sha256, compat })
    });
    Ok(Package { container, code })
}

/// The `Backend` the container describes and the engine that runs its programs, started: the module is checked against
/// `provided` (the capabilities this runner has), loaded on the Backend engine and linked. The lines of what is wrong
/// when it cannot.
pub fn load(package: Package, provided: &[&str]) -> Result<Loaded, Vec<String>> {
    let c = &package.container;
    let json = |what: &str, bytes: &[u8]| -> Result<String, Vec<String>> {
        String::from_utf8(bytes.to_vec()).map_err(|_| vec![format!("the Backend's {what} is not text: the file is damaged")])
    };
    let interface =
        InterfaceDescription::from_json(&json("interface", &c.interface)?).map_err(|e| vec![format!("the Backend's interface cannot be read: {e}")])?;
    let entries: Vec<ExportEntry> = serde_json::from_slice(&c.exports).map_err(|e| vec![format!("the Backend's table of exports cannot be read: {e}")])?;
    let history: Vec<InterfaceDescription> = if c.history.is_empty() {
        Vec::new()
    } else {
        let list: Vec<serde_json::Value> =
            serde_json::from_slice(&c.history).map_err(|e| vec![format!("the Backend's released interfaces cannot be read: {e}")])?;
        list.iter()
            .map(|v| InterfaceDescription::from_json(&v.to_string()))
            .collect::<Result<_, _>>()
            .map_err(|e| vec![format!("a released interface of the Backend cannot be read: {e}")])?
    };
    let slots: Vec<SlotDecl> = if c.slots.is_empty() {
        Vec::new()
    } else {
        serde_json::from_slice(&c.slots).map_err(|e| vec![format!("the Backend's upload slots cannot be read: {e}")])?
    };
    let state = StateMode::parse(&c.state).map_err(|e| vec![e])?;
    if state == StateMode::PerConnection {
        // A run unit kept for a connection is not served by this runner yet (spec 114 T37): refused here, in words, rather
        // than answered as if the state were fresh.
        return Err(vec![
            "this Backend keeps its WORKING-STORAGE for the connection ([backend] state = \"per-connection\"): the Backend runner serves per-call Backends in this release. Build it with `rcrun backend build --cargo`".into(),
        ]);
    }
    let backend = Backend::from_parts(interface, history, entries, state, slots)?;
    let began = std::time::Instant::now();
    let engine = WasmEngine::start(&c.module, package.code.as_ref(), provided).map_err(|e| vec![e])?;
    // For the operator only, in the log and on standard error: which engine runs this Backend (R12).
    let notice = format!(
        "engine: WebAssembly module, sha256 {}, {} exported program(s), loaded by {} in {:.1} ms",
        engine.module_sha256,
        backend.interface.programs.len(),
        engine.module().load_path,
        began.elapsed().as_secs_f64() * 1000.0
    );
    Ok(Loaded { backend, engine: Some(Arc::new(engine)), notice: Some(notice) })
}

/// The loader a runner's executable gives the server: its own file, read.
pub fn load_self(provided: &[&str]) -> Result<Loaded, Vec<String>> {
    let exe = std::env::current_exe().and_then(std::fs::read).map_err(|e| vec![format!("the runner cannot read its own file: {e}")])?;
    let package = read(&exe).map_err(|e| vec![e])?;
    // The SQL connections the Build wrote beside the file, named after the application, as a Cargo-built Backend reads them.
    let app = if package.container.name.is_empty() {
        std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned())).unwrap_or_default()
    } else {
        package.container.name.clone()
    };
    cobolt_runner::install_sql_connections(&app);
    load(package, provided)
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The module engine, for the suites that run on both engines (spec 114 T38).
//!
//! `loopback`, `client_contract` and `fuzz_dispatch` start their servers through the test kit's rigs, and the kit runs the
//! engine `PRC_TEST_ENGINE` names. The kit does not know the module engine (`cobolt-server` does not depend on the runner),
//! so a suite that wants it takes its rigs from here: each one registers the engine once and hands over to the kit's own.
//!
//! ```text
//! cargo test -p cobolt-server --test loopback                          # the interpreter
//! PRC_TEST_ENGINE=wasm cargo test -p cobolt-server --test loopback     # the Backend as a module, as a Build makes it
//! ```
//!
//! The engine is made from the Backend the rig serves exactly as a Build makes it: the programs are compiled by the
//! generator (the state mode the rig gives decides whether the module keeps a connection's state) and the runner's engine
//! loads the module against the capabilities the runner provides. A Backend the generator refuses stops the test, naming
//! why: the Build would have taken the Cargo path for it, and a suite must not pass by quietly running something else.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};

use cobolt_server::backend::{Backend, StateMode};
use cobolt_server::engine::Engine;
use cobolt_server::testkit::{self, Rig};

static REGISTERED: Once = Once::new();

fn registered() {
    REGISTERED.call_once(|| testkit::register_engine("wasm", module_engine));
}

/// The module engine for the Backend in `dir`, built as a Build builds it.
fn module_engine(dir: &Path, state: StateMode) -> Arc<dyn Engine> {
    let mut backend = Backend::load(dir).unwrap_or_else(|e| panic!("the Backend does not load: {e:#?}"));
    backend.state = state;
    let compiled = cobolt_compiler::backend_module::generate(&backend, &[]).unwrap_or_else(|reasons| panic!("the generator refuses this Backend, so a Build would take the Cargo path: {reasons:#?}"));
    let engine = cobolt_backend_runner::engine::WasmEngine::start(&compiled.wasm, None, &cobolt_backend_runner::provided()).unwrap_or_else(|e| panic!("the runner refuses the module: {e}"));
    Arc::new(engine)
}

/// [`testkit::rig`] on the engine `PRC_TEST_ENGINE` names.
pub fn rig(dir: PathBuf, extra_toml: &str) -> Rig {
    registered();
    testkit::rig(dir, extra_toml)
}

/// [`testkit::rig_with_state`] on the engine `PRC_TEST_ENGINE` names.
pub fn rig_with_state(dir: PathBuf, extra_toml: &str, state: StateMode) -> Rig {
    registered();
    testkit::rig_with_state(dir, extra_toml, state)
}

/// [`testkit::rig_with_identity`] on the engine `PRC_TEST_ENGINE` names.
pub fn rig_with_identity(dir: PathBuf, extra_toml: &str, http: Arc<dyn cobolt_server::identity::client::Http>) -> Rig {
    registered();
    testkit::rig_with_identity(dir, extra_toml, http)
}

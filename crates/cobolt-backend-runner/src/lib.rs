// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `prc-backend-runner` (spec 114): the prebuilt runner of a Backend built as a WebAssembly module.
//!
//! A Build copies this runner, puts the Backend inside the copy (its interface, its table of exports, its module and,
//! when the Build made it, the module's machine code) and names the copy after the project. What then starts is the
//! server the Cargo-built Backend is, with the same arguments, the same configuration, the same output and the same exit
//! codes (`cobolt_server::standalone`), and one difference the caller never sees: the programs are not interpreted. Each
//! call is one run of the module on the Backend engine, with the data capabilities of the Desktop runner as its imports.
//!
//! * [`payload`] reads the Backend out of the executable and refuses, by name, a module that needs a capability this
//!   runner does not provide.
//! * [`engine`] is the server's engine for a module: one call, one instance, the answer the interpreter would give.
//! * [`epoch`] is the clock that ends a call nothing else ends.

pub mod engine;
pub mod epoch;
pub mod payload;

/// The capabilities this runner provides: what a program with no window can use. A Backend whose module declares another
/// is refused when it starts, naming it. The groups of capabilities a variant of the runner leaves out are decided at
/// spec 114's gate G3, with each variant's measured size.
pub fn provided() -> Vec<&'static str> {
    cobolt_runner::headless_capabilities()
}

/// Run the Backend this executable carries with the process arguments (without the program's name): the exit code.
/// `0` a clean stop, `1` could not load or start, `2` refused configuration: what `standalone::main` of a Cargo-built file
/// answers.
pub fn run_self(args: Vec<String>) -> i32 {
    let provided = provided();
    let load = move || payload::load_self(&provided);
    cobolt_server::standalone::main_with(&load, args)
}

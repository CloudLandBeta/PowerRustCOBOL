// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt COBOL runtime — tree-walking interpreter.
//!
//! # Architecture
//!
//! ```text
//! Program (AST)
//!   └─► Interpreter::run()
//!         ├─► CobolEnvironment  (data store: COBOL name → CobolValue)
//!         ├─► exec_stmt()       (statement dispatcher)
//!         │     ├─► exec_move / exec_add / exec_if / …
//!         │     └─► exec_exec_rust()  ← EXEC RUST block executor
//!         └─► ObjectRegistry    (form/control state)
//! ```
//!
//! # Entry point
//!
//! ```rust,no_run
//! use cobolt_ast::program::Program;
//! use cobolt_runtime::Interpreter;
//!
//! # let program: Program = unimplemented!();
//! let mut interp = Interpreter::new(program);
//! match interp.run() {
//!     Ok(()) => println!("Program completed normally."),
//!     Err(e) => eprintln!("Runtime error: {e}"),
//! }
//! ```

pub mod channels;
pub mod db_runtime;
pub mod debugger;
pub mod environment;
pub mod error;
pub mod exec_rust;
pub mod http_runtime;
pub mod files;
pub mod indexed;
pub mod interpreter;
pub mod numedit;
pub mod objects;
pub mod value;

pub use channels::{FormEvent, StateUpdate};
pub use db_runtime::DbRegistry;
pub use debugger::{Breakpoints, DebugCmd, DebugEvent, VarSnapshot, new_breakpoints};
pub use error::RuntimeError;
pub use http_runtime::HttpClient;
pub use indexed::IndexedEngine;
pub use interpreter::Interpreter;
pub use environment::CobolEnvironment;
pub use objects::ObjectRegistry;
pub use value::CobolValue;

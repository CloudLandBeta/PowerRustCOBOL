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

pub mod async_op;
pub mod channels;
pub mod compress;
pub mod db_runtime;
pub mod debugger;
pub mod diag_path;
pub mod environment;
pub mod error;
pub mod exec_rust;
pub mod files;
pub mod form_host;
pub mod http_runtime;
pub mod indexed;
pub mod indexed_disk;
pub mod indexed_ide;
pub mod indexed_import;
pub mod indexed_log;
pub mod indexed_redb;
pub mod interpreter;
pub mod maps_bridge;
pub mod ors_bridge;
pub mod numedit;
pub mod objects;
pub mod rust_bridge;
pub mod value;

pub use channels::{FormEvent, FormIpcMessage, StateUpdate};
pub use db_runtime::DbRegistry;
pub use debugger::{
    new_breakpoints, Breakpoints, DebugCmd, DebugEvent, RemoteDebugCmd, VarSnapshot,
};
pub use environment::{new_external_store, CobolEnvironment, ExternalStore};
pub use error::RuntimeError;
pub use http_runtime::HttpClient;
pub use indexed::{
    IndexedEngine, IndexedFileInfo, KeyDescriptor, KeyEncoding, KeyList, KeyOrdering, KeyPart,
    RecordFormat,
};
pub use indexed_ide::{
    compare_schema, create_empty_from_definition, key_specs_from_def, GridSession, SchemaDrift,
};
pub use indexed_import::{definition_from_inspect, inspect_any_path};
pub use interpreter::Interpreter;
pub use objects::ObjectRegistry;
pub use rust_bridge::{BridgeError, BridgeValue, RustBridge};
pub use value::CobolValue;

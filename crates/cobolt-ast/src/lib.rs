// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Cobolt AST — Abstract Syntax Tree node types shared across all compiler
//! phases (parser, semantic analyzer, runtime).
//!
//! This crate is **pure data** — no parsing logic, no evaluation.  Every
//! type derives `Debug`, `Clone`, and `PartialEq`.
//!
//! # Module layout
//!
//! | Module    | Contents |
//! |-----------|----------|
//! | `program` | Top-level program structure and all divisions |
//! | `data`    | DATA DIVISION: `DataDecl`, `PicClause`, `Usage`, `OccursClause` |
//! | `expr`    | `Expr`, `Literal`, `FigurativeConstant`, `Condition`, operators |
//! | `stmt`    | `Stmt` and all supporting clause types |
//!
//! # Span
//!
//! All node types that carry source location information embed a
//! [`Span`] from [`cobolt_lexer`].  This keeps diagnostics self-contained
//! inside the AST without requiring callers to carry a separate source map.

pub mod data;
pub mod expr;
pub mod program;
pub mod stmt;

// Re-export the span types so downstream crates only need one import.
pub use cobolt_lexer::{Span, SpannedToken};

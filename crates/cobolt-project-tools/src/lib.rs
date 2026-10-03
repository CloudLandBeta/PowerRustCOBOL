// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The coding-agent tools** (spec 080).
//!
//! One tool set over one PowerRustCOBOL project, implemented once and served
//! through two front doors: `rcrun mcp` (stdio, headless) and the IDE (HTTP on
//! `127.0.0.1`). Neither is a special case of the other; the only thing that
//! differs between them is the [`host::ProjectHost`] — who owns the project
//! manifest while the tools run.
//!
//! ```text
//!   rcrun mcp ──stdio──▶ cobolt_mcp::serve ─┐
//!                                           ├─▶ ProjectTools<H: ProjectHost>
//!   IDE ───HTTP──▶ http::serve_http ─▶ dispatch ─┘      list_files · check · regenerate
//!                                                       add_to_project · build
//!                                                       validate · kb_lookup
//! ```
//!
//! The tools reuse the IDE's own code paths, moved down rather than copied:
//! [`gen_paths`] (where generated COBOL goes), [`validate_source`] (form and
//! source validation) and [`binding_guardian`] (the data-binding gate).

pub mod binding_guardian;
/// What a coding agent is told, independent of which agent (spec 080 R20).
pub mod content;
/// A new project, exactly as the IDE's New Project makes it (spec 084 R15).
pub mod create;
pub mod gen_paths;
pub mod host;
pub mod http;
/// Opening a project in PowerRustCOBOL AI from `rcrun mcp` (spec 084 R20a).
pub mod ide_launch;
/// The reference documents a coding agent reads (spec 080 R7, spec 084 R13).
pub mod reference;
pub mod root;
pub mod tools;
pub mod validate_source;

pub use host::{FileList, HeadlessHost, NoProject, ProjectHost};
pub use root::ProjectRoot;
pub use tools::ProjectTools;

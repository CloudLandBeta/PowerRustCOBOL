// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! No database driver was linked: every connection says why, as `08001`.

#![cfg_attr(feature = "sql", allow(dead_code))]

use super::super::state::{code, SqlError};
use super::Backend;

/// Why a program without drivers cannot connect.
pub const NOT_LINKED: &str = "the SQL drivers are not linked into this program: it was built \
    without SQLite, PostgreSQL and MySQL because the build found no EXEC SQL block and no \
    COBOL::\"OPEN-DB\" in it";

pub fn open(conn: &str) -> Result<Box<dyn Backend>, SqlError> {
    Err(SqlError::new(
        code::CANNOT_CONNECT,
        format!("{NOT_LINKED} (asked for {})", super::BackendKind::of(conn).name()),
    ))
}

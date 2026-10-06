// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Turning a parsed statement into the text a database receives: host
//! variables become placeholders in the database's own style, and
//! `CURRENT OF cursor` becomes the condition that names the row (R26).

use cobolt_ast::sql::SqlPart;

use super::backend::BackendKind;
use super::state::SqlError;

/// The placeholder for the n-th parameter (0-based): `?` for SQLite and
/// MySQL, `$n` for PostgreSQL.
pub fn placeholder(kind: BackendKind, n: usize) -> String {
    match kind {
        BackendKind::Postgres => format!("${}", n + 1),
        _ => "?".to_string(),
    }
}

/// The text of a statement for `kind`. `current_of` resolves a cursor named
/// by `WHERE CURRENT OF` to the condition text that selects its current row,
/// given the number of parameters already placed (it may add one).
pub fn render(
    parts: &[SqlPart],
    kind: BackendKind,
    mut current_of: impl FnMut(&str, usize) -> Result<String, SqlError>,
) -> Result<String, SqlError> {
    let mut out = String::new();
    let mut placed = 0usize;
    for p in parts {
        match p {
            SqlPart::Text(t) => out.push_str(t),
            SqlPart::Input(_) => {
                out.push_str(&placeholder(kind, placed));
                placed += 1;
            }
            SqlPart::CurrentOf(c) => {
                let cond = current_of(c, placed)?;
                placed += cond.matches(['?', '$']).count().min(1);
                out.push_str(&cond);
            }
        }
    }
    Ok(out)
}

/// Rewrite the `?` markers of dynamic SQL (R42) into the database's style —
/// a no-op except on PostgreSQL. Markers inside strings, quoted names and
/// comments are left alone.
pub fn question_marks(text: &str, kind: BackendKind) -> String {
    if kind != BackendKind::Postgres {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut n = 0;
    for p in cobolt_lexer::sql::scan(text) {
        if p.kind == cobolt_lexer::sql::SqlKind::Param {
            out.push_str(&placeholder(kind, n));
            n += 1;
        } else {
            out.push_str(&text[p.start..p.end]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_follow_the_database() {
        let parts = vec![
            SqlPart::Text("SELECT A FROM T WHERE B = ".into()),
            SqlPart::Input(0),
            SqlPart::Text(" AND C = ".into()),
            SqlPart::Input(1),
        ];
        let none = |_: &str, _: usize| -> Result<String, SqlError> { unreachable!() };
        assert_eq!(render(&parts, BackendKind::Sqlite, none).unwrap(), "SELECT A FROM T WHERE B = ? AND C = ?");
        assert_eq!(render(&parts, BackendKind::Postgres, none).unwrap(), "SELECT A FROM T WHERE B = $1 AND C = $2");
        assert_eq!(question_marks("X = ? AND Y = '?'", BackendKind::Postgres), "X = $1 AND Y = '?'");
        assert_eq!(question_marks("X = ?", BackendKind::MySql), "X = ?");
    }
}

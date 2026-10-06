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

/// The words of `sql` that are SQL code (not inside a string, quoted name or
/// comment), upper-cased, with their byte range and parenthesis depth.
fn code_words(sql: &str) -> Vec<(String, usize, usize, i32)> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    for p in cobolt_lexer::sql::scan(sql) {
        if p.kind != cobolt_lexer::sql::SqlKind::Code {
            continue;
        }
        let mut i = p.start;
        while i < p.end {
            let c = b[i];
            if c == b'(' {
                depth += 1;
                i += 1;
            } else if c == b')' {
                depth -= 1;
                i += 1;
            } else if c.is_ascii_alphanumeric() || c == b'_' {
                let s = i;
                while i < p.end && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                out.push((sql[s..i].to_ascii_uppercase(), s, i, depth));
            } else {
                if c == b',' {
                    out.push((",".into(), i, i + 1, depth));
                }
                i += 1;
            }
        }
    }
    out
}

/// `sql` without a top-level trailing `FOR UPDATE [OF …]` — SQLite neither
/// accepts nor needs it (it locks the whole database on a write).
pub fn strip_for_update(sql: &str) -> String {
    let w = code_words(sql);
    for i in 0..w.len().saturating_sub(1) {
        if w[i].3 == 0 && w[i].0 == "FOR" && w[i + 1].0 == "UPDATE" {
            return sql[..w[i].1].trim_end().to_string();
        }
    }
    sql.to_string()
}

/// The one table a positioned cursor's query reads (R26): where its select
/// list starts, and the table as written. Only a query over ONE table has a
/// row to name: a join, a list of tables, `DISTINCT` or `GROUP BY` gives
/// `0A000`.
pub fn single_table(sql: &str) -> Result<(usize, String), SqlError> {
    let not_supported = |why: &str| {
        Err(SqlError::new(
            super::state::code::NOT_SUPPORTED,
            format!("WHERE CURRENT OF needs a cursor over one table; this one {why}"),
        ))
    };
    let w = code_words(sql);
    let Some(sel) = w.iter().position(|x| x.3 == 0 && x.0 == "SELECT") else {
        return not_supported("is not a SELECT");
    };
    if w.get(sel + 1).is_some_and(|x| x.0 == "DISTINCT") {
        return not_supported("selects DISTINCT rows");
    }
    let from = w.iter().position(|x| x.3 == 0 && x.0 == "FROM");
    let Some(from) = from else { return not_supported("reads no table") };
    const ENDS: [&str; 8] = ["WHERE", "GROUP", "ORDER", "LIMIT", "HAVING", "UNION", "FOR", "WINDOW"];
    for x in &w[from + 1..] {
        if x.3 != 0 {
            continue;
        }
        if ENDS.contains(&x.0.as_str()) {
            if x.0 == "GROUP" {
                return not_supported("groups its rows");
            }
            break;
        }
        if x.0 == "," || x.0 == "JOIN" {
            return not_supported("reads more than one table");
        }
    }
    // The table: what follows FROM, up to white space, a comma or a bracket.
    let after = sql[w[from].2..].trim_start();
    let table: String = after.chars().take_while(|c| !c.is_whitespace() && !matches!(c, ',' | '(' | ')' | ';')).collect();
    Ok((w[sel].2, table))
}

/// A cursor's query with `keys` selected first, so `WHERE CURRENT OF` can
/// name the row it fetched.
pub fn keyed_select(sql: &str, at: usize, keys: &[String]) -> String {
    format!("{} {},{}", &sql[..at], keys.join(", "), &sql[at..])
}

/// SQLite's form of [`keyed_select`]: `rowid` first.
pub fn sqlite_keyed_select(sql: &str) -> Result<String, SqlError> {
    let (at, _) = single_table(sql)?;
    Ok(keyed_select(sql, at, &["rowid".to_string()]))
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

    #[test]
    fn positioned_cursors_are_keyed_on_one_table_only() {
        assert_eq!(
            sqlite_keyed_select("SELECT ID, NAME FROM T WHERE A = ? ORDER BY ID").unwrap(),
            "SELECT rowid, ID, NAME FROM T WHERE A = ? ORDER BY ID"
        );
        assert_eq!(
            sqlite_keyed_select("select x from t where y = 'a, b FROM c, d'").unwrap(),
            "select rowid, x from t where y = 'a, b FROM c, d'"
        );
        for bad in ["SELECT A FROM T, U", "SELECT A FROM T JOIN U ON 1", "SELECT DISTINCT A FROM T", "SELECT A FROM T GROUP BY A"] {
            assert_eq!(sqlite_keyed_select(bad).unwrap_err().sqlstate, "0A000", "{bad}");
        }
        assert!(sqlite_keyed_select("SELECT A FROM T WHERE B IN (SELECT C FROM U, V)").is_ok(), "a subquery is not the cursor's table list");
        assert_eq!(single_table("SELECT A FROM shop.ORDERS WHERE B = 1").unwrap().1, "shop.ORDERS");
        assert_eq!(single_table("select a from `T`;").unwrap().1, "`T`");
        assert_eq!(keyed_select("SELECT A FROM T", 6, &["`ID`".into(), "`LINE`".into()]), "SELECT `ID`, `LINE`, A FROM T");
        assert_eq!(strip_for_update("SELECT A FROM T FOR UPDATE OF A"), "SELECT A FROM T");
        assert_eq!(strip_for_update("SELECT 'FOR UPDATE' FROM T"), "SELECT 'FOR UPDATE' FROM T");
    }
}

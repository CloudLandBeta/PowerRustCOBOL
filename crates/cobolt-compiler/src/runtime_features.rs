// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Which parts of the runtime an application actually needs linked.
//!
//! # Why a build asks this at all
//!
//! `cobolt-runtime` pins `rusqlite` with its `bundled` feature, which compiles
//! the SQLite **C amalgamation**. That made a working **C toolchain**
//! (`link.exe` on Windows, `cc` elsewhere) a requirement for building *any*
//! PowerRustCOBOL application — a console program that only ever DISPLAYs paid
//! the same price as one that opens a database, and the failure it produced
//! ("linker `cc` not found") names nothing a COBOL developer put in their
//! source.
//!
//! So the build reads the program first. A program it can prove never reaches a
//! SQL verb is compiled against a runtime with no SQL drivers at all, and needs
//! Rust alone.
//!
//! # The reading is deliberately timid
//!
//! Being wrong here is expensive and confusing in a specific way: the IDE
//! *interprets* with the full runtime, so a program that works under Run Form
//! would fail only once built. Every doubt therefore resolves towards linking:
//!
//! - a `CALL` naming a SQL verb as a **literal** — the ordinary case;
//! - a `CALL` whose target is **not** a literal (`CALL WS-VERB`), because the
//!   name is only known at run time and no reading of the source can settle it;
//! - an `EXEC RUST` block whose source mentions the SQL modules by name, since
//!   a block reaches Rust APIs directly and would fail to compile without them.
//!
//! Only a program where none of those appear loses SQL. That is a strong
//! condition, and it is what makes the trade safe rather than clever.

use cobolt_ast::{
    expr::{Expr, Literal},
    program::{ProcedureBody, Program},
    stmt::Stmt,
};

/// The SQL bridge's CALL surface (spec Phase 8). A program naming any of these
/// needs the drivers.
const SQL_CALLS: &[&str] = &[
    "COBOL-OPEN-DB",
    "COBOL-EXEC-SQL",
    "COBOL-FETCH-ROW",
    "COBOL-NEXT-ROW",
    "COBOL-ROW-COUNT",
    "COBOL-CLOSE-DB",
];

/// Rust paths a block would name to reach the SQL runtime directly.
const SQL_RUST_PATHS: &[&str] = &["db_runtime", "DbRegistry", "rusqlite", "postgres", "mysql"];

/// What a program needs linked, unioned across every program in the build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeFeatures {
    /// SQLite / PostgreSQL / MySQL. The only one that costs a C toolchain.
    pub sql: bool,
}

impl RuntimeFeatures {
    /// Everything on — the answer whenever the build cannot read the program.
    pub fn all() -> Self {
        Self { sql: true }
    }

    /// Fold in another program's needs.
    pub fn union(self, other: Self) -> Self {
        Self {
            sql: self.sql || other.sql,
        }
    }

    /// The cargo feature list for `cobolt-runtime`, as TOML array items.
    ///
    /// Deliberately explicit even when empty: the generated manifest pairs it
    /// with `default-features = false`, so an empty list is a real statement —
    /// "no optional runtime" — not an oversight.
    pub fn as_toml_features(self) -> String {
        let mut names: Vec<&str> = Vec::new();
        if self.sql {
            names.push("\"sql\"");
        }
        names.join(", ")
    }
}

/// Read one program — and its nested programs — for what it needs.
pub fn scan_program(program: &Program) -> RuntimeFeatures {
    let mut found = RuntimeFeatures::default();
    for_each_stmt(program, &mut |stmt| {
        match stmt {
            Stmt::Call { program: target, .. } => match literal_text(target) {
                Some(name) => {
                    let upper = name.trim().to_ascii_uppercase();
                    if SQL_CALLS.contains(&upper.as_str()) {
                        found.sql = true;
                    }
                }
                // The verb is computed at run time. Nothing in the source says
                // what it will be, so the build cannot rule SQL out.
                None => found.sql = true,
            },
            Stmt::ExecRust { source, .. } => {
                if SQL_RUST_PATHS.iter().any(|p| source.contains(p)) {
                    found.sql = true;
                }
            }
            _ => {}
        }
    });
    // An item-level block is Rust at module scope; it can name the runtime too.
    for item in &program.rust_items {
        if SQL_RUST_PATHS.iter().any(|p| item.source.contains(p)) {
            found.sql = true;
        }
    }
    for nested in &program.nested_programs {
        found = found.union(scan_program(nested));
    }
    found
}

/// The literal text of a `CALL` target, or `None` when it is computed.
///
/// A non-string literal is `None` too: it names no verb this build knows, and
/// treating "not a name I recognise" as "definitely not SQL" is exactly the
/// optimism this reading avoids.
fn literal_text(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Literal(Literal::String(s), _) => Some(s.as_str()),
        _ => None,
    }
}

/// Every statement in this program's PROCEDURE DIVISION, nested ones included.
fn for_each_stmt(program: &Program, f: &mut impl FnMut(&Stmt)) {
    match &program.procedure.body {
        ProcedureBody::Paragraphs(paras) => {
            for para in paras {
                for stmt in &para.stmts {
                    stmt.walk(f);
                }
            }
        }
        ProcedureBody::Sections(secs) => {
            for sec in secs {
                for para in &sec.paragraphs {
                    for stmt in &para.stmts {
                        stmt.walk(f);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_lexer::{tokenize, SourceFormat};
    use cobolt_parser::parse;

    fn scan(src: &str) -> RuntimeFeatures {
        let parsed = parse(tokenize(src, SourceFormat::Free));
        scan_program(&parsed.program.expect("the fixture parses"))
    }

    fn prog(body: &str) -> String {
        format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
             DATA DIVISION.\nWORKING-STORAGE SECTION.\n\
             01 WS-VERB PIC X(20).\n01 WS-H PIC 9(9).\n01 WS-S PIC X(2).\n\
             PROCEDURE DIVISION.\nMAIN.\n{body}\n"
        )
    }

    /// The whole point: a program that never touches SQL is built without it,
    /// and therefore without a C toolchain.
    #[test]
    fn a_plain_program_needs_no_sql() {
        let f = scan(&prog("    DISPLAY \"hello\".\n    STOP RUN."));
        assert!(!f.sql, "nothing here reaches a database");
        assert_eq!(f.as_toml_features(), "");
    }

    #[test]
    fn a_literal_sql_call_links_the_drivers() {
        let f = scan(&prog(
            "    CALL \"COBOL-OPEN-DB\" USING WS-VERB, WS-H, WS-S.\n    STOP RUN.",
        ));
        assert!(f.sql);
        assert_eq!(f.as_toml_features(), "\"sql\"");
    }

    /// Every verb of the surface counts, not just the one that opens.
    #[test]
    fn any_verb_of_the_surface_counts() {
        for verb in SQL_CALLS {
            let f = scan(&prog(&format!(
                "    CALL \"{verb}\" USING WS-H.\n    STOP RUN."
            )));
            assert!(f.sql, "{verb} should link the drivers");
        }
    }

    /// A verb assembled at run time cannot be read, so the build must not
    /// guess that it is harmless — this is the case that would otherwise fail
    /// only once built, having worked all along under Run Form.
    #[test]
    fn a_computed_call_target_keeps_sql() {
        let f = scan(&prog(
            "    MOVE \"COBOL-OPEN-DB\" TO WS-VERB.\n\
             \x20   CALL WS-VERB USING WS-VERB, WS-H, WS-S.\n    STOP RUN.",
        ));
        assert!(
            f.sql,
            "a CALL through a data item could be any verb — link it"
        );
    }

    /// A block that reaches the runtime's SQL API directly would not compile
    /// without the drivers, and it names no COBOL verb to find.
    #[test]
    fn a_block_naming_the_runtime_keeps_sql() {
        let f = scan(&prog(
            "    EXEC RUST\n\
             \x20   let _ = cobolt_runtime::db_runtime::DbRegistry::new();\n\
             \x20   END-EXEC.\n    STOP RUN.",
        ));
        assert!(f.sql, "the block names db_runtime");
    }

    /// A block that has nothing to do with databases does not drag them in.
    #[test]
    fn an_unrelated_block_does_not_link_sql() {
        let f = scan(&prog(
            "    EXEC RUST\n    println!(\"hi\");\n    END-EXEC.\n    STOP RUN.",
        ));
        assert!(!f.sql);
    }

    /// A form's event handlers are nested programs; a CALL in one counts.
    #[test]
    fn a_nested_program_is_read_too() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\n\
                   DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-H PIC 9(9).\n\
                   PROCEDURE DIVISION.\nMAIN.\n    DISPLAY \"outer\".\n\
                   IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\n\
                   DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-K PIC 9(9).\n\
                   PROCEDURE DIVISION.\nCLICK.\n\
                   \x20   CALL \"COBOL-CLOSE-DB\" USING WS-K.\n\
                   END PROGRAM INNER.\nEND PROGRAM OUTER.\n";
        assert!(
            scan(src).sql,
            "the handler that closes a database is inside a nested program"
        );
    }

    #[test]
    fn union_and_all_behave() {
        let none = RuntimeFeatures::default();
        let sql = RuntimeFeatures { sql: true };
        assert!(!none.union(none).sql);
        assert!(none.union(sql).sql);
        assert!(RuntimeFeatures::all().sql);
    }

    /// The manifest must both cut the defaults and state what it wants. Naming
    /// features while leaving `default-features` alone would union `sql` back
    /// in and change nothing — the failure would be invisible, because the
    /// build would keep working and keep needing a C toolchain.
    #[test]
    fn the_manifest_cuts_defaults_and_names_what_it_wants() {
        let dir = std::path::Path::new("/ws/crates");

        let lean = crate::base_dependency_block(dir, false, RuntimeFeatures::default());
        assert!(
            lean.contains("cobolt-runtime")
                && lean.contains("default-features = false")
                && lean.contains("features = []"),
            "a program with no SQL asks for none, explicitly:\n{lean}"
        );
        assert!(
            !lean.contains("\"sql\""),
            "…and must not name it anyway:\n{lean}"
        );

        let full = crate::base_dependency_block(dir, false, RuntimeFeatures::all());
        assert!(
            full.contains("features = [\"sql\"]"),
            "a program that uses SQL asks for it:\n{full}"
        );

        // A form application reaches the runtime through the form host too, and
        // cargo unions features across the graph — so the host has to be cut
        // the same way or the drivers come straight back.
        let forms = crate::base_dependency_block(dir, true, RuntimeFeatures::default());
        let host_line = forms
            .lines()
            .find(|l| l.starts_with("cobolt-form-host"))
            .expect("a form application links the host");
        assert!(
            host_line.contains("default-features = false") && host_line.contains("features = []"),
            "the host must not re-enable what the runtime line just turned off: {host_line}"
        );
    }
}

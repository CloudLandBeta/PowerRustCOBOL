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

/// The REST client's CALL surface (spec Phase 10).
const HTTP_CALLS: &[&str] = &[
    "COBOL-HTTP-GET",
    "COBOL-HTTP-POST",
    "COBOL-HTTP-PUT",
    "COBOL-HTTP-DELETE",
    "COBOL-HTTP-SET-HEADER",
    "COBOL-HTTP-CLEAR-HEADERS",
];

/// Rust paths a block would name to reach the HTTP runtime directly.
const HTTP_RUST_PATHS: &[&str] = &["http_runtime", "HttpClient", "ureq", "native_tls"];

/// Rust paths a block would name to reach the Maps runtime directly.
const MAPS_RUST_PATHS: &[&str] = &["maps_bridge", "google_maps"];

/// What a program needs linked, unioned across every program in the build.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeFeatures {
    /// SQLite / PostgreSQL / MySQL. Costs a C toolchain everywhere.
    pub sql: bool,
    /// The REST client. Costs OpenSSL development files on Linux.
    pub http: bool,
    /// The Google Maps data verbs. Pure Rust, but the largest dependency the
    /// runtime carries.
    pub maps: bool,
}

impl RuntimeFeatures {
    /// Everything on — the answer whenever the build cannot read the program.
    pub fn all() -> Self {
        Self {
            sql: true,
            http: true,
            maps: true,
        }
    }

    /// Fold in another program's needs.
    pub fn union(self, other: Self) -> Self {
        Self {
            sql: self.sql || other.sql,
            http: self.http || other.http,
            maps: self.maps || other.maps,
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
        if self.http {
            names.push("\"http\"");
        }
        if self.maps {
            names.push("\"maps\"");
        }
        names.join(", ")
    }
}

/// What the project's **forms** need, which no COBOL statement reveals.
///
/// The REST, Maps and WebSearch controls are reached by *method invocation* on
/// a control id (`MAPS-1::Geocode`), and the AST cannot tell one method call
/// from another — `GET` on a RestClient looks exactly like `GET` on anything
/// else. The designed form says what a control actually is, so that is what
/// gets read.
///
/// **HTTP is linked for any form application at all**, deliberately, and not
/// because a RestClient might be hiding: `cobolt-forms`'s `render` feature
/// fetches OSM basemap tiles with its own `ureq`, so a form application links
/// the platform TLS stack whatever this says. There is nothing to win by being
/// clever and a working program to lose. Console programs — the ones that can
/// actually shed OpenSSL — own no controls, and their HTTP shows up as a `CALL`.
pub fn scan_forms<'a>(forms: impl IntoIterator<Item = &'a cobolt_forms::Form>) -> RuntimeFeatures {
    let mut found = RuntimeFeatures::default();
    for form in forms {
        // Any form → HTTP. See above.
        found.http = true;
        for ctrl in flatten(&form.controls) {
            if matches!(
                ctrl.control_type,
                cobolt_forms::ControlType::Maps | cobolt_forms::ControlType::WebSearch
            ) {
                found.maps = true;
            }
        }
    }
    found
}

/// Every control in the tree, containers included.
fn flatten(controls: &[cobolt_forms::Control]) -> Vec<&cobolt_forms::Control> {
    let mut out = Vec::new();
    fn rec<'a>(list: &'a [cobolt_forms::Control], out: &mut Vec<&'a cobolt_forms::Control>) {
        for c in list {
            out.push(c);
            rec(&c.children, out);
        }
    }
    rec(controls, &mut out);
    out
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
                    if HTTP_CALLS.contains(&upper.as_str()) {
                        found.http = true;
                    }
                }
                // The verb is computed at run time. Nothing in the source says
                // what it will be, so the build can rule nothing out.
                None => found = found.union(RuntimeFeatures::all()),
            },
            Stmt::ExecRust { source, .. } => found = found.union(scan_rust(source)),
            _ => {}
        }
    });
    // An item-level block is Rust at module scope; it can name the runtime too.
    for item in &program.rust_items {
        found = found.union(scan_rust(&item.source));
    }
    for nested in &program.nested_programs {
        found = found.union(scan_program(nested));
    }
    found
}

/// What a block's Rust source reaches into, by the module names it mentions.
///
/// A block calls Rust APIs directly, so a bridge it names has to be linked or
/// the generated crate will not compile — a build failure against the
/// developer's own line, for a decision they never made.
fn scan_rust(source: &str) -> RuntimeFeatures {
    RuntimeFeatures {
        sql: SQL_RUST_PATHS.iter().any(|p| source.contains(p)),
        http: HTTP_RUST_PATHS.iter().any(|p| source.contains(p)),
        maps: MAPS_RUST_PATHS.iter().any(|p| source.contains(p)),
    }
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

    /// The whole point: a console program that touches none of the bridges is
    /// built without any of them — no C toolchain for SQLite, no platform TLS.
    #[test]
    fn a_plain_program_needs_nothing() {
        let f = scan(&prog("    DISPLAY \"hello\".\n    STOP RUN."));
        assert!(!f.sql, "nothing here reaches a database");
        assert!(!f.http, "…nor the network");
        assert!(!f.maps, "…nor Maps");
        assert_eq!(f.as_toml_features(), "");
    }

    #[test]
    fn a_literal_http_call_links_the_client() {
        for verb in HTTP_CALLS {
            let f = scan(&prog(&format!(
                "    CALL \"{verb}\" USING WS-VERB.\n    STOP RUN."
            )));
            assert!(f.http, "{verb} should link the REST client");
            assert!(!f.sql, "{verb} has nothing to do with databases");
        }
    }

    /// Each bridge is asked for by name, so a program using two links two.
    #[test]
    fn the_feature_list_names_exactly_what_is_reached() {
        let f = scan(&prog(
            "    CALL \"COBOL-OPEN-DB\" USING WS-VERB, WS-H, WS-S.\n\
             \x20   CALL \"COBOL-HTTP-GET\" USING WS-VERB, WS-VERB, WS-H.\n    STOP RUN.",
        ));
        assert_eq!(
            f.as_toml_features(),
            "\"sql\", \"http\"",
            "Maps was never mentioned and must not be linked"
        );
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
    ///
    /// It could be *any* verb, so it links **everything**, not just SQL.
    #[test]
    fn a_computed_call_target_links_everything() {
        let f = scan(&prog(
            "    MOVE \"COBOL-OPEN-DB\" TO WS-VERB.\n\
             \x20   CALL WS-VERB USING WS-VERB, WS-H, WS-S.\n    STOP RUN.",
        ));
        assert_eq!(
            f,
            RuntimeFeatures::all(),
            "a CALL through a data item could be any verb — link all of them"
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
        let sql = RuntimeFeatures {
            sql: true,
            ..Default::default()
        };
        assert!(!none.union(none).sql);
        assert!(none.union(sql).sql);
        assert!(!none.union(sql).http, "union must not invent a feature");
        let all = RuntimeFeatures::all();
        assert!(all.sql && all.http && all.maps);
        assert_eq!(all.as_toml_features(), "\"sql\", \"http\", \"maps\"");
    }

    /// A Maps control is reached by method call on a control id, which the AST
    /// cannot distinguish from any other method call — so the DESIGN is read.
    #[test]
    fn a_maps_control_links_the_maps_client() {
        let with_maps = form_with(cobolt_forms::ControlType::Maps);
        let f = scan_forms([&with_maps]);
        assert!(f.maps, "the form owns a Maps control");
        assert!(f.http, "any form links TLS through the basemap fetcher anyway");
        assert!(!f.sql, "a Maps control says nothing about databases");
    }

    /// A form application WITHOUT a Maps control sheds the Google Maps client —
    /// this is the case that saves the most, since `google_maps` is `reqwest`
    /// plus `tokio`.
    #[test]
    fn an_ordinary_form_does_not_link_maps() {
        let plain = form_with(cobolt_forms::ControlType::Button);
        let f = scan_forms([&plain]);
        assert!(!f.maps, "no Maps control, no Maps client");
        assert!(f.http, "…but TLS is linked regardless, by cobolt-forms");
    }

    /// A WebSearch control rides the same client.
    #[test]
    fn a_websearch_control_links_maps_too() {
        let ws = form_with(cobolt_forms::ControlType::WebSearch);
        assert!(scan_forms([&ws]).maps);
    }

    /// No forms at all — the console case, and the only one that can shed TLS.
    #[test]
    fn no_forms_asks_for_nothing() {
        assert_eq!(scan_forms(std::iter::empty()), RuntimeFeatures::default());
    }

    /// A Maps control nested inside a container is still a Maps control.
    #[test]
    fn a_control_inside_a_container_is_found() {
        let mut panel = control("PANEL-1", cobolt_forms::ControlType::GroupBox);
        panel
            .children
            .push(control("MAPS-1", cobolt_forms::ControlType::Maps));
        let mut form = cobolt_forms::Form::new("F", "F", 800, 600);
        form.controls = vec![panel];
        assert!(
            scan_forms([&form]).maps,
            "the tree is walked, not just its top level"
        );
    }

    fn control(id: &str, kind: cobolt_forms::ControlType) -> cobolt_forms::Control {
        cobolt_forms::Control::new(id, kind, 0, 0)
    }

    fn form_with(kind: cobolt_forms::ControlType) -> cobolt_forms::Form {
        let mut form = cobolt_forms::Form::new("F", "F", 800, 600);
        form.controls = vec![control("C-1", kind)];
        form
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
            full.contains("features = [\"sql\", \"http\", \"maps\"]"),
            "a program that reaches everything asks for everything:\n{full}"
        );

        // One bridge on, the others off — the list is not all-or-nothing.
        let sql_only = crate::base_dependency_block(
            dir,
            false,
            RuntimeFeatures {
                sql: true,
                ..Default::default()
            },
        );
        assert!(
            sql_only.contains("features = [\"sql\"]"),
            "SQL alone asks for SQL alone:\n{sql_only}"
        );
        assert!(
            !sql_only.contains("\"http\"") && !sql_only.contains("\"maps\""),
            "…and drags nothing else in:\n{sql_only}"
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

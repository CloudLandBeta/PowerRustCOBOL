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
    // Spec 092 R12 — a binary download rides the same client.
    "COBOL-HTTP-DOWNLOAD",
];

/// Rust paths a block would name to reach the HTTP runtime directly.
const HTTP_RUST_PATHS: &[&str] = &["http_runtime", "HttpClient", "ureq", "native_tls"];

/// Spec 092 R10/R11 — the picture built-ins. A program naming either needs the
/// image decoders.
const IMAGE_CALLS: &[&str] = &["COBOL-IMAGE-FIT", "COBOL-IMAGE-INFO"];

/// Rust paths a block would name to reach the Maps runtime directly.
const MAPS_RUST_PATHS: &[&str] = &["maps_bridge", "google_maps"];

/// Spec 092 — the sound built-ins. A program naming any of these needs the
/// audio runtime, and on Linux the ALSA development files to build it.
const AUDIO_CALLS: &[&str] = &[
    "COBOL-AUDIO-PLAY",
    "COBOL-AUDIO-PAUSE",
    "COBOL-AUDIO-RESUME",
    "COBOL-AUDIO-STOP",
    "COBOL-AUDIO-STATE",
];

/// WASM-014 — the built-ins that talk to the backend without a `CALL … WITH
/// WEBSOCKET LINKAGE`. A program naming either needs the client.
const REMOTE_CALLS: &[&str] = &["COBOL-WS-UPLOAD", "COBOL-WS-DOWNLOAD"];

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
    /// Spec 068 — the application Knowledge Base (a `KnowledgeBase` control).
    /// Pure Rust.
    pub kb: bool,
    /// Spec 068 — the built-in semantic model, only when the project asks for
    /// it (`[rag] embedder = "builtin"`): it links candle and tokenizers, and
    /// tokenizers' `onig` compiles C. Never inferred, never in [`Self::all`].
    pub kb_semantic: bool,
    /// Spec 075 — `smb://` addresses for files an `AgentObject` registers by
    /// path. Pure Rust (`smb2`); on for any form with an `AgentObject`.
    pub smb: bool,
    /// The Viewer's `SaveAsPdf` — what the Viewer paints, written as a PDF.
    /// Pure Rust; on for any form with a Viewer (which links the painter
    /// already).
    pub pdf: bool,
    /// The Viewer opening Word, PowerPoint, Excel and OpenDocument files as
    /// their text (`cobolt-docs`). Pure Rust; on for any form with a Viewer.
    pub office: bool,
    /// Spec 078 and spec 103 — the AWS controls: the pool of AWS Labs server
    /// processes, and the native HTTPS transport to the hosted AWS MCP Server
    /// (the AWS credential crates, `ureq` with the operating system's TLS, and
    /// `tokio`). Pure Rust, no rustls and no C build; it implies `http`. On for
    /// any form with an `Aws*` control (R29: an application without one links
    /// none of it).
    pub aws: bool,
    /// Spec 092 — `IMAGE-FIT` / `IMAGE-INFO`: the `image` decoders. Pure
    /// Rust; on for a program that calls either. A form application links
    /// `image` anyway (through `cobolt-forms`' renderer), so this is what a
    /// console program sheds.
    pub images: bool,
    /// Spec 092 — the sound built-ins (`rodio`). Pure Rust on macOS and
    /// Windows; on Linux it needs ALSA to build and `libasound2` to run, so
    /// only a program that calls an `AUDIO-…` verb links it. No control does.
    pub audio: bool,
    /// WASM-005/014 - `CALL … WITH WEBSOCKET LINKAGE` and the file built-ins
    /// `WS-UPLOAD` / `WS-DOWNLOAD`: the application links the backend client
    /// (`cobolt-server`'s `client` feature) and starts it. Not a feature of the
    /// runtime, so it is not in [`Self::as_toml_features`]; a program without a
    /// remote call links no socket code for it.
    pub remote: bool,
}

impl RuntimeFeatures {
    /// Everything on — the answer whenever the build cannot read the program.
    pub fn all() -> Self {
        Self {
            sql: true,
            http: true,
            maps: true,
            kb: true,
            kb_semantic: false,
            smb: true,
            pdf: true,
            office: true,
            aws: true,
            images: true,
            audio: true,
            remote: true,
        }
    }

    /// Fold in another program's needs.
    pub fn union(self, other: Self) -> Self {
        Self {
            sql: self.sql || other.sql,
            http: self.http || other.http,
            maps: self.maps || other.maps,
            kb: self.kb || other.kb,
            kb_semantic: self.kb_semantic || other.kb_semantic,
            smb: self.smb || other.smb,
            pdf: self.pdf || other.pdf,
            office: self.office || other.office,
            aws: self.aws || other.aws,
            images: self.images || other.images,
            audio: self.audio || other.audio,
            remote: self.remote || other.remote,
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
        if self.kb_semantic {
            names.push("\"kb-semantic\"");
        } else if self.kb {
            names.push("\"kb\"");
        }
        if self.smb {
            names.push("\"smb\"");
        }
        if self.pdf {
            names.push("\"pdf\"");
        }
        if self.office {
            names.push("\"office\"");
        }
        if self.aws {
            names.push("\"aws\"");
        }
        if self.images {
            names.push("\"images\"");
        }
        if self.audio {
            names.push("\"audio\"");
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
            if ctrl.control_type == cobolt_forms::ControlType::KnowledgeBase {
                found.kb = true;
            }
            if ctrl.control_type == cobolt_forms::ControlType::AgentObject {
                found.smb = true;
            }
            if ctrl.control_type == cobolt_forms::ControlType::Viewer {
                found.pdf = true;
                found.office = true;
            }
            if ctrl.control_type.as_str().starts_with("Aws") {
                found.aws = true;
            }
        }
    }
    found
}

/// What a backend program's declared control needs (WASM-005 T11): the
/// backend seeds a typed control for a web form's generated program, and its
/// methods are member calls no statement scan can tell apart — so the class
/// says, as the form does in [`scan_forms`]. The backend has no renderer, so
/// HTTP is linked only for a control that reaches the network.
pub fn scan_control_class(class: &str) -> RuntimeFeatures {
    let mut found = RuntimeFeatures::default();
    match class {
        "SqlDatabase" => found.sql = true,
        "RestClient" => found.http = true,
        "Maps" | "WebSearch" => {
            found.http = true;
            found.maps = true;
        }
        "KnowledgeBase" => {
            found.http = true;
            found.kb = true;
        }
        "AgentObject" => {
            found.http = true;
            found.smb = true;
        }
        c if c.starts_with("Aws") => {
            found.http = true;
            found.aws = true;
        }
        _ => {}
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
            // WASM-005: the target of a WEBSOCKET call is a program of the
            // backend, never a built-in of this runtime — its name says nothing
            // about what the application links, except that it needs the client.
            Stmt::Call { convention: cobolt_ast::stmt::CallConvention::WebSocket, .. } => found.remote = true,
            Stmt::Call { program: target, .. } => match literal_text(target) {
                Some(name) => {
                    let upper = name.trim().to_ascii_uppercase();
                    if SQL_CALLS.contains(&upper.as_str()) {
                        found.sql = true;
                    }
                    if HTTP_CALLS.contains(&upper.as_str()) {
                        found.http = true;
                    }
                    if IMAGE_CALLS.contains(&upper.as_str()) {
                        found.images = true;
                    }
                    if AUDIO_CALLS.contains(&upper.as_str()) {
                        found.audio = true;
                    }
                    // Files in and out of the backend ride the client.
                    if REMOTE_CALLS.contains(&upper.as_str()) {
                        found.remote = true;
                    }
                }
                // The verb is computed at run time. Nothing in the source says
                // what it will be, so the build can rule nothing out.
                None => found = found.union(RuntimeFeatures::all()),
            },
            Stmt::ExecRust { source, .. } => found = found.union(scan_rust(source)),
            // Embedded SQL (spec 087) reaches its database through the same
            // drivers as the built-ins.
            Stmt::ExecSql(_) => found.sql = true,
            _ => {}
        }
    });
    // A cursor declared in WORKING-STORAGE is SQL even before a statement
    // opens it.
    if !program.sql_cursors.is_empty() {
        found.sql = true;
    }
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
        kb: source.contains("cobolt_kb"),
        kb_semantic: false,
        smb: source.contains("smb_source") || source.contains("registered_file"),
        pdf: source.contains("viewer_pdf"),
        office: source.contains("cobolt_docs"),
        aws: source.contains("cobolt_runtime::aws"),
        images: source.contains("image_runtime"),
        audio: source.contains("audio_runtime"),
        remote: source.contains("cobolt_server"),
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
        assert!(!f.images, "…nor a picture");
        assert_eq!(f.as_toml_features(), "");
    }

    /// WASM-014 - an application links the backend client only when it talks to
    /// its backend, and for nothing else: not a SQL driver, not the REST client.
    #[test]
    fn a_remote_call_or_a_file_transfer_links_the_backend_client_and_nothing_else_does() {
        for body in [
            "    CALL \"GET-CUSTOMER\" WITH WEBSOCKET LINKAGE USING BY REFERENCE WS-VERB RETURNING WS-S.\n    STOP RUN.",
            "    COBOL::\"WS-UPLOAD\" ( \"docs\" WS-VERB \"image/png\" WS-VERB WS-S )\n    STOP RUN.",
            "    CALL \"COBOL-WS-DOWNLOAD\" USING WS-VERB, WS-VERB, WS-S\n    STOP RUN.",
        ] {
            let f = scan(&prog(body));
            assert!(f.remote, "{body} needs the client");
            assert!(!f.sql && !f.http && !f.maps && !f.images && !f.audio, "…and nothing else: {f:?}");
            assert_eq!(f.as_toml_features(), "", "the client is not a feature of the runtime");
        }
        assert!(!scan(&prog("    DISPLAY \"hello\".\n    STOP RUN.")).remote);
        // The backend's own built-ins run in the backend, which has the server, not the client.
        assert!(!scan(&prog("    COBOL::\"WS-CLAIM\" ( \"sub\" WS-VERB )\n    STOP RUN.")).remote);
        assert!(RuntimeFeatures::all().remote && RuntimeFeatures::default().union(RuntimeFeatures { remote: true, ..Default::default() }).remote);
    }

    /// Spec 092 — `IMAGE-FIT` / `IMAGE-INFO` link the decoders, written either
    /// way, and nothing else; a console program that never calls them sheds
    /// them (`a_plain_program_needs_nothing`).
    #[test]
    fn a_picture_built_in_links_the_image_decoders() {
        for call in [
            "CALL \"COBOL-IMAGE-FIT\" USING WS-VERB, WS-VERB, WS-H, WS-H, WS-S",
            "COBOL::\"IMAGE-FIT\" ( WS-VERB WS-VERB 400 400 WS-S )",
            "CALL \"COBOL-IMAGE-INFO\" USING WS-VERB, WS-H, WS-H, WS-VERB, WS-S",
            "COBOL::\"IMAGE-INFO\" ( WS-VERB WS-H WS-H WS-VERB WS-S )",
        ] {
            let f = scan(&prog(&format!("    {call}\n    STOP RUN.")));
            assert!(f.images, "{call} should link the decoders");
            assert_eq!(f.as_toml_features(), "\"images\"", "{call} links nothing else");
        }
        assert!(scan_rust("let _ = cobolt_runtime::image_runtime::info(p);").images);
    }

    /// Spec 092 — `HTTP-DOWNLOAD` is a REST verb: it links the client (and
    /// with it the platform TLS), not the decoders.
    #[test]
    fn http_download_links_the_rest_client() {
        let f = scan(&prog(
            "    COBOL::\"HTTP-DOWNLOAD\" ( WS-VERB WS-VERB WS-H WS-S )\n    STOP RUN.",
        ));
        assert!(f.http && !f.images && !f.sql, "{f:?}");
        assert_eq!(f.as_toml_features(), "\"http\"");
    }

    /// A form's handler is a nested program: a fit inside one links the
    /// decoders for the built binary too (interpreter-binary parity).
    #[test]
    fn a_picture_built_in_in_a_handler_links_the_decoders() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. OUTER.\n\
                   DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-H PIC 9(9).\n\
                   PROCEDURE DIVISION.\nMAIN.\n    DISPLAY \"outer\".\n\
                   IDENTIFICATION DIVISION.\nPROGRAM-ID. INNER.\n\
                   DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 WS-S PIC X(12).\n\
                   PROCEDURE DIVISION.\nCLICK.\n\
                   \x20   COBOL::\"IMAGE-FIT\" ( \"a.jpg\" \"b.png\" 40 40 WS-S )\n\
                   END PROGRAM INNER.\nEND PROGRAM OUTER.\n";
        assert!(scan(src).images, "the handler that fits a picture is a nested program");
        // A designed form says nothing about pictures by itself: its `image`
        // comes from the renderer, and the feature only from a call.
        assert!(!scan_forms([&form_with(cobolt_forms::ControlType::PictureBox)]).images);
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
    fn scan_marks_exec_sql() {
        let statement = prog("EXEC SQL DELETE FROM ORDERS END-EXEC");
        assert!(scan(&statement).sql, "an EXEC SQL statement links the drivers");
        let cursor = "IDENTIFICATION DIVISION.\nPROGRAM-ID. CURSOR-ONLY.\nDATA DIVISION.\n\
                      WORKING-STORAGE SECTION.\n    EXEC SQL DECLARE C1 CURSOR FOR SELECT ID FROM ORDERS END-EXEC.\n\
                      PROCEDURE DIVISION.\nMAIN-PARA.\n    STOP RUN.\n";
        assert!(scan(cursor).sql, "a declared cursor links the drivers");
        assert!(!scan(&prog("DISPLAY \"EXEC SQL\"")).sql, "the words in a literal do not");
    }

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
        assert!(all.sql && all.http && all.maps && all.kb);
        assert!(!all.kb_semantic, "the built-in model is never part of \"everything\"");
        assert_eq!(
            all.as_toml_features(),
            "\"sql\", \"http\", \"maps\", \"kb\", \"smb\", \"pdf\", \"office\", \"aws\", \"images\", \"audio\""
        );
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

    /// Spec 075 — an AgentObject can register files by `smb://` address, so
    /// its form links the SMB client; a form without one does not.
    #[test]
    fn an_agent_object_links_smb() {
        let agent = form_with(cobolt_forms::ControlType::AgentObject);
        let f = scan_forms([&agent]);
        assert!(f.smb);
        assert!(f.as_toml_features().contains("\"smb\""));
        let plain = form_with(cobolt_forms::ControlType::Button);
        assert!(!scan_forms([&plain]).smb, "no AgentObject, no SMB client");
    }

    /// A Viewer can save what it shows as a PDF, so its form links the PDF
    /// writer; a form without one does not (operator, 2026-09-27).
    #[test]
    fn a_viewer_links_the_pdf_writer() {
        let viewer = form_with(cobolt_forms::ControlType::Viewer);
        let f = scan_forms([&viewer]);
        assert!(f.pdf);
        assert!(f.as_toml_features().contains("\"pdf\""));
        assert!(f.office, "…and opens Office files as their text");
        let plain = form_with(cobolt_forms::ControlType::Button);
        assert!(!scan_forms([&plain]).pdf, "no Viewer, no PDF writer");
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

    /// Spec 078 T-A15 / AC17 — a form without an AWS control builds without
    /// AWS support (R29)…
    #[test]
    fn a_form_without_aws_controls_builds_without_aws() {
        let f = scan_forms([&form_with(cobolt_forms::ControlType::Button)]);
        assert!(!f.aws, "{f:?}");
        assert!(!f.as_toml_features().contains("\"aws\""));
    }

    /// …and either AWS control, top level or inside a container, turns it on.
    #[test]
    fn an_aws_control_turns_the_feature_on() {
        for kind in cobolt_forms::ControlType::ALL.iter().filter(|t| t.is_aws()) {
            assert!(scan_forms([&form_with(kind.clone())]).aws, "{kind:?}");
            let mut panel = control("PANEL-1", cobolt_forms::ControlType::Panel);
            panel.children.push(control("AWS-1", kind.clone()));
            let mut form = cobolt_forms::Form::new("F", "F", 800, 600);
            form.controls = vec![panel];
            let f = scan_forms([&form]);
            assert!(f.aws && f.as_toml_features().contains("\"aws\""), "{kind:?} in a container");
        }
        assert!(scan_rust("let c = cobolt_runtime::aws::connections::find(\"x\");").aws);
    }

    /// Spec 092 R17 — every sound verb, by its literal name, links the audio
    /// runtime, and nothing else does: a program that never plays a sound
    /// needs no ALSA to build on Linux.
    #[test]
    fn a_literal_audio_call_links_sound_and_nothing_else_does() {
        for verb in AUDIO_CALLS {
            let f = scan(&prog(&format!("    CALL \"{verb}\" USING WS-VERB.\n    STOP RUN.")));
            assert!(f.audio, "{verb} should link the audio runtime");
            assert_eq!(f.as_toml_features(), "\"audio\"", "{verb} links sound alone");
        }
        // The inline spelling is the same CALL.
        let inline = scan(&prog("    COBOL::\"AUDIO-PLAY\" ( \"a.mp3\" WS-S ).\n    STOP RUN."));
        assert!(inline.audio, "COBOL::\"AUDIO-PLAY\" is CALL \"COBOL-AUDIO-PLAY\"");
        assert!(!scan(&prog("    DISPLAY \"COBOL-AUDIO-PLAY\".\n    STOP RUN.")).audio, "a literal is not a call");
        assert!(!scan(&prog("    CALL \"COBOL-HTTP-GET\" USING WS-VERB WS-VERB WS-H.\n    STOP RUN.")).audio);
        assert!(!scan_forms([&form_with(cobolt_forms::ControlType::Button)]).audio, "no control plays sound");
        assert!(scan_rust("cobolt_runtime::audio_runtime::AudioHandle::new();").audio);
        assert!(!scan_rust("println!(\"audio\");").audio);
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
            full.contains("features = [\"sql\", \"http\", \"maps\", \"kb\", \"smb\", \"pdf\", \"office\", \"aws\", \"images\", \"audio\"]"),
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

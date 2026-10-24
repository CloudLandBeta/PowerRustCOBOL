// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T9: the backend programs of a web form's bound DataGrids.
//!
//! Golden files pin every generated program and the generated `exports.toml`
//! (`tests/golden/backend/`); regenerate with `UPDATE_GOLDEN=1` and review the
//! diff. Every program must parse and pass the analyser with no error — the
//! backend refuses to load one that does not.

use cobolt_codegen::remote_controls::{fixtures, generate_backend, generate_control_programs, is_generated, GeneratedBackend};
use std::path::PathBuf;

fn generated() -> GeneratedBackend {
    let load = |path: &str| {
        assert_eq!(path, fixtures::CIDX_PATH);
        Ok(fixtures::customers_cidx("data/customers.idx"))
    };
    generate_backend(&fixtures::grid_forms(), &load).unwrap_or_else(|e| panic!("generation failed: {e:#?}"))
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join("backend")
}

#[test]
fn one_read_per_bound_grid_and_a_save_only_when_writable() {
    let g = generated();
    let ids: Vec<&str> = g.programs.iter().map(|p| p.program_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "RC-6-ORDERS-9-GRID-CUST-READ",
            "RC-6-ORDERS-9-GRID-CUST-SAVE",
            "RC-6-ORDERS-10-GRID-ITEMS-READ",
            "RC-6-ORDERS-10-GRID-ITEMS-SAVE",
            "RC-6-REPORT-11-GRID-TOTALS-READ",
        ],
        "the read-only REPORT grid has no SAVE"
    );
    for p in &g.programs {
        assert!(is_generated(&p.source), "{}: no banner", p.program_id);
        assert_eq!(p.file_name, format!("{}.cbl", p.program_id));
        assert_eq!(p.idempotent, p.program_id.ends_with("-READ"));
    }
    // The grid's BackendRoles; an unset one is any authenticated caller.
    assert_eq!(g.programs[0].roles, vec!["clerk".to_string(), "manager".to_string()]);
    assert!(g.programs[2].roles.is_empty());
    let exports = g.exports_toml();
    assert!(is_generated(&exports));
    assert_eq!(exports.matches("[[export]]").count(), 5);
    assert_eq!(exports.matches("public = false").count(), 5, "a generated endpoint is never public");
    assert!(!exports.contains("public = true"));
    println!("{} programs, exports.toml {} bytes", g.programs.len(), exports.len());
}

#[test]
fn every_generated_program_parses_and_analyses_clean() {
    let g = generated();
    for p in &g.programs {
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&p.source, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<String> =
            parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
        assert!(errors.is_empty(), "{} does not parse:\n{}\n{}", p.program_id, errors.join("\n"), p.source);
        let program = parsed.program.unwrap_or_else(|| panic!("{}: no program", p.program_id));
        assert!(program.identification.program_id.eq_ignore_ascii_case(&p.program_id));
        assert_eq!(program.procedure.using, vec!["RC-PARAMS".to_string()]);
        assert_eq!(program.procedure.returning.as_deref(), Some("RC-RESULT"));
        let sem: Vec<String> = cobolt_semantic::analyze(&program)
            .errors()
            .map(|d| format!("line {}: {}", d.span.line, d.message))
            .collect();
        assert!(sem.is_empty(), "{} fails the analyser:\n{}\n{}", p.program_id, sem.join("\n"), p.source);
        println!("  {:<34} {:>5} lines  parse 0 errors, analyser 0 errors", p.program_id, p.source.lines().count());
    }
}

#[test]
fn no_value_is_spliced_into_sql_text() {
    // R17 and the save's host variables: every value reaches a statement as a
    // host variable; the only SQL text is the design-time query and table.
    let g = generated();
    let save = g.programs.iter().find(|p| p.program_id == "RC-6-ORDERS-10-GRID-ITEMS-SAVE").unwrap();
    assert!(save.source.contains("SET NAME = :RC-V2:RC-I2"), "{}", save.source);
    assert!(save.source.contains("WHERE ID = :RC-K1"));
    assert!(save.source.contains("VALUES (:RC-V1:RC-I1, :RC-V2:RC-I2, :RC-V3:RC-I3, :RC-V4:RC-I4)"));
    assert!(!save.source.contains("STRING "), "no statement is built from text");
    let read = g.programs.iter().find(|p| p.program_id == "RC-6-ORDERS-10-GRID-ITEMS-READ").unwrap();
    assert!(read.source.contains("CONNECT TO 'SALES' AS RC-CONN"), "the BackendConfig name, never a connection string");
    assert!(read.source.contains("ORDER BY ID\n"), "the query's trailing ';' is dropped");
    assert!(!read.source.contains("ORDER BY ID;"));
}

#[test]
fn a_grid_the_backend_cannot_serve_is_refused_naming_it() {
    let mut forms = fixtures::grid_forms();
    // No connection name anywhere: refused, never a guess.
    forms[1].controls[1].properties.shift_remove("SqlConnection");
    // A writable indexed grid that does not show the key.
    if let cobolt_forms::BindingSourceDescriptor::IndexedFile { fields, .. } = &mut forms[0].data_bindings[0].source {
        fields.remove(0);
    }
    let load = |_: &str| Ok(fixtures::customers_cidx("data/customers.idx"));
    let errs = generate_backend(&forms, &load).unwrap_err();
    assert_eq!(errs.len(), 2, "{errs:#?}");
    assert!(errs[0].starts_with("ORDERS: DataGrid GRID-CUST: the key field CUST-ID"), "{}", errs[0]);
    assert!(errs[1].starts_with("REPORT: DataGrid GRID-TOTALS: DB-SALES names no backend connection"), "{}", errs[1]);
}

#[test]
fn generated_backend_programs_match_their_goldens() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let dir = golden_dir();
    let g = generated();
    let mut files: Vec<(String, String)> = g.programs.iter().map(|p| (p.file_name.clone(), p.source.clone())).collect();
    files.push(("exports.toml".into(), g.exports_toml()));
    let mut compared = 0;
    for (name, text) in &files {
        let path = dir.join(name);
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, text).unwrap();
            println!("  {name:<38} WRITTEN ({} bytes)", text.len());
            continue;
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing golden {path:?} ({e}); run once with UPDATE_GOLDEN=1 and review"));
        if *text != golden {
            let line = text.lines().zip(golden.lines()).position(|(a, b)| a != b).map(|i| i + 1);
            panic!("{name} moved; first differing line {line:?}. If deliberate, regenerate with UPDATE_GOLDEN=1 and review.");
        }
        compared += 1;
        println!("  {name:<38} {:>6} bytes identical", text.len());
    }
    if !update {
        assert_eq!(compared, 6, "every generated file is compared");
    }
}

// ── T11: the programs of a class C control's method calls ────────────────────

fn control_programs() -> GeneratedBackend {
    let (form, sites) = fixtures::control_form();
    let programs = generate_control_programs(&form, &sites).unwrap_or_else(|e| panic!("generation failed: {e:#?}"));
    GeneratedBackend { programs, ..Default::default() }
}

#[test]
fn one_program_per_call_site_numbered_in_source_order() {
    let g = control_programs();
    let ids: Vec<&str> = g.programs.iter().map(|p| p.program_id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "RC-4-SHOP-6-REST-1-GET-1",
            "RC-4-SHOP-6-REST-1-POST-1",
            "RC-4-SHOP-6-REST-1-GET-2",
            "RC-4-SHOP-7-AGENT-1-ASK-1",
            "RC-4-SHOP-4-DB-1-QUERY-1",
            "RC-4-SHOP-12-ORDERS-TABLE-GETITEM-1",
        ],
        "a property read (REST-1::Caption) makes no program"
    );
    for p in &g.programs {
        assert!(is_generated(&p.source), "{}: no banner", p.program_id);
        assert!(!p.idempotent, "{}: a method may change the world", p.program_id);
        assert!(p.control.is_some(), "{}: the backend must know what to seed", p.program_id);
    }
    assert_eq!(g.programs[0].roles, vec!["clerk".to_string()]);
    // The resource is the handler's literal, compiled in; nothing else is.
    assert!(g.programs[0].source.contains("MOVE REST-1::Get( \"customers\" ) TO RESULT-VALUE"), "{}", g.programs[0].source);
    assert!(g.programs[0].source.contains("IF ARG-COUNT NOT = 0"));
    assert!(g.programs[1].source.contains("REST-1::Post( \"orders\" ARG-VALUE ( 1 ) )"));
    assert!(g.programs[1].source.contains("IF ARG-COUNT NOT = 1"));
    assert!(g.programs[4].source.contains("DB-1::Query( \"SELECT NAME, PRICE FROM ITEMS ORDER BY NAME\" )"));
    assert!(g.programs[4].source.contains("DB-1::Open()"), "no connection outlives a call");
    assert!(g.programs[4].source.contains("RC-TAKE-ROWS"), "a query's rows travel with its answer");
}

#[test]
fn the_declaration_carries_the_configuration_name_and_never_a_secret() {
    let g = control_programs();
    let rest = g.programs[0].control.as_ref().unwrap();
    assert_eq!((rest.id.as_str(), rest.class.as_str(), rest.configuration.as_str()), ("REST-1", "RestClient", "CRM"));
    let get = |k: &str| rest.properties.iter().find(|(n, _)| n.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str());
    assert_eq!(get("BaseURL"), Some("https://api.example.test/v1"));
    assert_eq!(get("AuthToken"), None, "a secret typed on the form never reaches the backend's files (R14)");
    assert_eq!(get("BackendConfig"), None);
    assert_eq!(get("BackendRoles"), None);
    let db = g.programs[4].control.as_ref().unwrap();
    assert_eq!(db.configuration, "", "SQL keeps its catalogue");
    assert!(db.properties.iter().any(|(k, v)| k == "SqlConnection" && v == "SALES"));
    let exports = g.exports_toml();
    assert!(!exports.contains("typed-on-the-form"), "{exports}");
    assert_eq!(exports.matches("[export.control]").count(), 6);
    assert!(exports.contains("configuration = \"CRM\""));
    println!("exports.toml with declarations: {} bytes", exports.len());
}

#[test]
fn every_method_program_parses_and_analyses_clean() {
    let g = control_programs();
    for p in &g.programs {
        let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(&p.source, cobolt_lexer::SourceFormat::Free));
        let errors: Vec<String> =
            parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
        assert!(errors.is_empty(), "{} does not parse:\n{}\n{}", p.program_id, errors.join("\n"), p.source);
        let program = parsed.program.unwrap_or_else(|| panic!("{}: no program", p.program_id));
        assert_eq!(program.procedure.using, vec!["RC-PARAMS".to_string()]);
        assert_eq!(program.procedure.returning.as_deref(), Some("RC-RESULT"));
        let sem: Vec<String> =
            cobolt_semantic::analyze(&program).errors().map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
        assert!(sem.is_empty(), "{} fails the analyser:\n{}\n{}", p.program_id, sem.join("\n"), p.source);
        println!("  {:<38} {:>4} lines  parse 0 errors, analyser 0 errors", p.program_id, p.source.lines().count());
    }
}

#[test]
fn a_call_the_backend_cannot_serve_is_refused_at_its_line() {
    use cobolt_codegen::remote_controls::{CallSite, SiteArg};
    let (form, _) = fixtures::control_form();
    let site = |control: &str, method: &str, line: u32, args: Vec<SiteArg>| CallSite { control: control.into(), method: method.into(), line, column: 1, args };
    let sites = vec![
        // The resource is a value only the handler knows: the browser would choose it.
        site("REST-1", "Get", 11, vec![SiteArg::Expression]),
        // A connection string in the handler: never on the web.
        site("DB-1", "Open", 12, vec![SiteArg::Literal("Server=x;Password=y".into())]),
        // A local file: through a slot, not yet.
        site("ORDERS-TABLE", "Query", 13, vec![SiteArg::Expression]),
    ];
    let errs = generate_control_programs(&form, &sites).unwrap_err();
    let lines: Vec<u32> = errs.iter().map(|e| e.line).collect();
    assert_eq!(lines, vec![11, 12, 13], "{errs:#?}");
    assert!(errs[0].message.starts_with("SHOP: REST-1::Get — argument 1 (what is reached) must be a literal"), "{}", errs[0].message);
    assert!(errs[1].message.contains("connection string"), "{}", errs[1].message);
    assert!(!errs[1].message.contains("Password=y"), "a refusal never repeats the value");
    assert!(errs[2].message.contains("argument 1 (the statement) must be a literal"), "{}", errs[2].message);
    // A SqlDatabase with no connection name at all.
    let mut bare = form.clone();
    bare.controls[2].properties.shift_remove(cobolt_forms::web_class::BACKEND_CONFIG);
    let errs = generate_control_programs(&bare, &[site("DB-1", "Execute", 9, vec![SiteArg::Literal("DELETE FROM T".into())])]).unwrap_err();
    assert!(errs[0].message.contains("names no backend connection"), "{}", errs[0].message);
}

#[test]
fn generated_method_programs_match_their_goldens() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    let dir = golden_dir().join("controls");
    let g = control_programs();
    let mut files: Vec<(String, String)> = g.programs.iter().map(|p| (p.file_name.clone(), p.source.clone())).collect();
    files.push(("exports.toml".into(), g.exports_toml()));
    let mut compared = 0;
    for (name, text) in &files {
        let path = dir.join(name);
        if update {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, text).unwrap();
            println!("  {name:<42} WRITTEN ({} bytes)", text.len());
            continue;
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing golden {path:?} ({e}); run once with UPDATE_GOLDEN=1 and review"));
        if *text != golden {
            let line = text.lines().zip(golden.lines()).position(|(a, b)| a != b).map(|i| i + 1);
            panic!("{name} moved; first differing line {line:?}. If deliberate, regenerate with UPDATE_GOLDEN=1 and review.");
        }
        compared += 1;
        println!("  {name:<42} {:>6} bytes identical", text.len());
    }
    if !update {
        assert_eq!(compared, 7, "every generated file is compared");
    }
}

// ── T12: a FileDropZone's upload slot ────────────────────────────────────────

#[test]
fn a_file_drop_zone_makes_an_upload_slot_with_its_own_limits() {
    use cobolt_codegen::remote_controls::generate_slots;
    let (form, sites) = fixtures::control_form();
    let slots = generate_slots(&form).unwrap();
    assert_eq!(slots.len(), 1);
    let s = &slots[0];
    assert_eq!(s.name, "RC-4-SHOP-6-DROP-1-COMMITFILES");
    assert_eq!(s.extensions, vec!["csv".to_string(), "xlsx".to_string()], "the zone's own reading of AllowedExtensions");
    assert_eq!(s.max_bytes, Some(64 * 1024));
    assert_eq!(s.roles, vec!["clerk".to_string()]);
    // No program for the zone, and a zone that sets no limit has none.
    assert!(generate_control_programs(&form, &sites).unwrap().iter().all(|p| !p.program_id.contains("DROP-1")));
    let mut open = form.clone();
    let z = open.controls.iter_mut().find(|c| c.id == "DROP-1").unwrap();
    z.set_prop("AllowedExtensions", cobolt_forms::PropValue::String(String::new()));
    z.set_prop("MaximumFileSizeKB", cobolt_forms::PropValue::Int(0));
    let open = generate_slots(&open).unwrap();
    assert!(open[0].extensions.is_empty() && open[0].max_bytes.is_none());
    let exports = GeneratedBackend { slots, ..Default::default() }.exports_toml();
    assert!(exports.contains("[[slot]]\nname = \"RC-4-SHOP-6-DROP-1-COMMITFILES\"\nextensions = [\"csv\", \"xlsx\"]\nmax_bytes = 65536\nroles = [\"clerk\"]\n"), "{exports}");
}

// ── T13: a secret on a web form, per class C control ────────────────────────

#[test]
fn a_connection_string_or_key_on_a_control_is_found_and_the_value_is_never_kept() {
    use cobolt_codegen::remote_controls::{secrets_on_form, SecretOnForm};
    use cobolt_forms::{web_class, Control, ControlType, Form, PropValue};
    let mut form = Form::new("SHOP", "Shop", 800, 600);
    // Seeded as the model seeds them: no finding (`sqlite::memory:` names no server).
    form.controls.push(Control::new("DB-0", ControlType::SqlDatabase, 0, 0));
    form.controls.push(Control::new("AGENT-0", ControlType::AgentObject, 0, 0));
    assert!(secrets_on_form(&form).is_empty());
    // A connection string with no password — the shape scan passes it — and a key.
    let mut db = Control::new("DB-1", ControlType::SqlDatabase, 0, 0);
    db.set_prop("ConnectionString", PropValue::String("postgres://db.example.com/sales".into()));
    let mut agent = Control::new("AGENT-1", ControlType::AgentObject, 0, 0);
    agent.set_prop("AgentAPIKey", PropValue::String("not-a-known-shape-0042".into()));
    // Nested in a panel: found all the same.
    let mut panel = Control::new("PNL-1", ControlType::Panel, 0, 0);
    panel.children.push(agent);
    form.controls.push(db);
    form.controls.push(panel);
    // A name in BackendConfig is what a web form should hold: not a finding.
    let mut rest = Control::new("REST-1", ControlType::RestClient, 0, 0);
    rest.set_prop(web_class::BACKEND_CONFIG, PropValue::String("CRM".into()));
    form.controls.push(rest);
    let found = secrets_on_form(&form);
    assert_eq!(
        found,
        vec![
            SecretOnForm { control: "DB-1".into(), property: "ConnectionString".into() },
            SecretOnForm { control: "AGENT-1".into(), property: "AgentAPIKey".into() },
        ]
    );
    assert!(!format!("{found:?}").contains("example.com") && !format!("{found:?}").contains("0042"), "the value is never kept");
}

// ── T12: a Viewer's upload slot (operator, 2026-10-24) ──────────────────────

#[test]
fn a_viewer_makes_an_open_slot_of_the_types_it_renders_or_its_own() {
    use cobolt_codegen::remote_controls::generate_slots;
    use cobolt_forms::{web_class, Control, ControlType, Form, PropValue};
    let mut form = Form::new("DOCS", "Docs", 800, 600);
    form.controls.push(Control::new("VWR-1", ControlType::Viewer, 0, 0));
    let slots = generate_slots(&form).unwrap();
    assert_eq!(slots.len(), 1);
    let s = &slots[0];
    assert_eq!(s.name, "RC-4-DOCS-5-VWR-1-OPEN");
    assert_eq!(s.extensions.len(), 17, "unset: every type the Viewer renders");
    assert!(s.extensions.iter().any(|e| e == "pdf") && !s.extensions.iter().any(|e| e == "exe"));
    assert_eq!(s.max_bytes, Some(10_240 * 1024), "unset: 10 MB");
    assert!(s.roles.is_empty());
    // Set on the web form: its own limits and roles.
    let v = form.controls.iter_mut().find(|c| c.id == "VWR-1").unwrap();
    v.set_prop(web_class::ALLOWED_EXTENSIONS, PropValue::String("pdf, .MD".into()));
    v.set_prop(web_class::MAXIMUM_FILE_SIZE_KB, PropValue::Int(512));
    v.set_prop(web_class::BACKEND_ROLES, PropValue::String("reader".into()));
    let s = &generate_slots(&form).unwrap()[0];
    assert_eq!(s.extensions, vec!["pdf".to_string(), "md".to_string()]);
    assert_eq!(s.max_bytes, Some(512 * 1024));
    assert_eq!(s.roles, vec!["reader".to_string()]);
    // Neither Save As nor any other Viewer method makes a backend program.
    assert!(generate_control_programs(&form, &[]).unwrap().is_empty());
}

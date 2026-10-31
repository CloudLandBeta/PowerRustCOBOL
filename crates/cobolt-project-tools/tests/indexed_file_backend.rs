// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 111 on a real Web project: the Build generates an IndexedFile control's
//! backend programs under `backend/generated/` with their `[[export]]` entries,
//! regenerates them when the control changes (hand edits are lost), keeps the
//! banner convention (it refuses to overwrite a file without it), and stops
//! Check and Build, naming the form and the control, when the file cannot be
//! served. The handler's own COBOL is unchanged: its file verbs build.

use cobolt_codegen::remote_controls::fixtures;
use cobolt_forms::model::{Control, ControlType, EventBinding, Form, PropValue};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// The handler: its own `SELECT`/`FD` on the control's file and the verbs, as on the desktop.
const HANDLER: &str = "       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT PARTS ASSIGN TO \"parts\"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PART-ID
               STORAGE IS MEMORY
               FILE STATUS IS PARTS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  PARTS.
       01  PART-REC.
           05 PART-ID   PIC 9(5).
           05 PART-NAME PIC X(20).
           05 PART-QTY  PIC 9(4).
       WORKING-STORAGE SECTION.
       01 PARTS-FS PIC XX.
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN I-O PARTS.
           MOVE 1 TO PART-ID.
           MOVE \"bolt\" TO PART-NAME.
           MOVE 5 TO PART-QTY.
           WRITE PART-REC INVALID KEY MOVE \"dup\" TO LBL-OUT::Caption END-WRITE.
           MOVE 1 TO PART-ID.
           START PARTS KEY IS >= PART-ID INVALID KEY CONTINUE END-START.
           READ PARTS NEXT AT END CONTINUE NOT AT END MOVE PART-NAME TO LBL-OUT::Caption END-READ.
           CLOSE PARTS.
";

const OPS: [&str; 9] = ["OPEN", "CLOSE", "START", "READ-NEXT", "READ-PREVIOUS", "READ-INVALID", "WRITE", "REWRITE", "DELETE"];

fn project(tag: &str, roles: &str, cidx: Option<cobolt_indexed::IndexedDefinition>) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("prc-spec111-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(
        dir.join("Shop.project.toml"),
        "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n[files]\nforms = [\"webforms/main-form.cfrm\"]\n\n[web]\nbackend_url = \"wss://api.example.com/ws\"\n",
    )
    .unwrap();
    let def = cidx.unwrap_or_else(|| fixtures::parts_cidx("data/parts.idx"));
    cobolt_indexed::save_indexed(&dir.join("data/parts.cidx"), &def).unwrap();
    let form = form(roles, HANDLER);
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.cfrm")).unwrap();
    (dir.join("Shop.project.toml"), dir)
}

fn form(roles: &str, handler: &str) -> Form {
    let mut form = Form::new("main-form", "Shop", 400, 300);
    let mut btn = Control::new("BTN-OK", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-OK", "onClick");
    ev.code = handler.to_owned();
    btn.events.push(ev);
    form.controls.push(btn);
    form.controls.push(Control::new("LBL-OUT", ControlType::Label, 10, 60));
    let mut ix = Control::new("IXF-1", ControlType::IndexedFile, 10, 100);
    ix.set_prop("IndexedFile", PropValue::String("data/parts.cidx".into()));
    if !roles.is_empty() {
        ix.set_prop(cobolt_forms::web_class::BACKEND_ROLES, PropValue::String(roles.into()));
    }
    form.controls.push(ix);
    form
}

fn generated(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("backend/generated"))
        .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn the_build_generates_regenerates_and_guards_an_indexed_files_backend_programs() {
    let t0 = Instant::now();
    let (manifest, dir) = project("build", "", None);
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();

    // Check builds nothing and refuses nothing; Build writes the nine programs and their exports.
    GeneratorBackend.check(&req).unwrap_or_else(|e| panic!("Check: {e}"));
    assert!(generated(&dir).is_empty(), "Check writes no backend program");
    let built = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("Build: {e}"));
    let names = generated(&dir);
    let mut want: Vec<String> = OPS.iter().map(|o| format!("RC-9-MAIN-FORM-5-IXF-1-{o}.cbl")).collect();
    want.push("exports.toml".into());
    want.sort();
    assert_eq!(names, want);
    let exports = std::fs::read_to_string(dir.join("backend/generated/exports.toml")).unwrap();
    for o in OPS {
        assert!(exports.contains(&format!("program = \"RC-9-MAIN-FORM-5-IXF-1-{o}\"")), "{o} is exported");
    }
    assert_eq!(exports.matches("public = false").count(), 9, "never public");
    assert!(!exports.contains("roles"), "any authenticated caller by default");
    let read = std::fs::read_to_string(dir.join("backend/generated/RC-9-MAIN-FORM-5-IXF-1-READ-NEXT.cbl")).unwrap();
    assert!(read.contains("This code was generated automatically by PowerRustCOBOL RAD."));
    assert!(read.contains("data/parts.idx"), "the file is fixed at design time");
    let first = t0.elapsed();

    // A change to the control (its roles) regenerates; a hand edit is lost.
    let edited = dir.join("backend/generated/RC-9-MAIN-FORM-5-IXF-1-WRITE.cbl");
    let original = std::fs::read_to_string(&edited).unwrap();
    std::fs::write(&edited, format!("{original}      *> a developer's edit\n")).unwrap();
    cobolt_forms::xml::save_form(&form("clerk, manager", HANDLER), &dir.join("webforms/main-form.cfrm")).unwrap();
    GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("Build: {e}"));
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), original, "the Build rewrote the file: the edit is lost");
    let exports = std::fs::read_to_string(dir.join("backend/generated/exports.toml")).unwrap();
    assert_eq!(exports.matches("roles = [\"clerk\", \"manager\"]").count(), 9, "the control's BackendRoles reach every export");

    // A file without the banner is the developer's: the Build refuses to overwrite it.
    std::fs::write(&edited, "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MINE.\n").unwrap();
    let err = GeneratorBackend.build(&req).expect_err("a file that is not the Build's stays");
    assert!(err.to_string().contains("not a generated file"), "{err}");
    std::fs::remove_file(&edited).unwrap();

    // Without the control the Build removes what it generated.
    let mut plain = form("", HANDLER);
    plain.controls.retain(|c| c.id != "IXF-1");
    cobolt_forms::xml::save_form(&plain, &dir.join("webforms/main-form.cfrm")).unwrap();
    GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("Build: {e}"));
    assert!(generated(&dir).is_empty(), "the control is gone, so are its programs: {:?}", generated(&dir));
    println!(
        "spec 111 Build: IndexedFile -> 9 programs + exports.toml in {} ms (module {} bytes); roles change regenerates 9 exports; hand edit lost; non-banner file refused; control removed -> programs removed; 6/6 checks passed",
        first.as_millis(),
        built.size.total_bytes
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_indexed_file_the_backend_cannot_serve_stops_check_and_build_naming_the_form_and_control() {
    let mut def = fixtures::parts_cidx("data/parts.idx");
    def.fields[0].children[2].usage = cobolt_indexed::FieldUsage::Comp3;
    let (manifest, dir) = project("refused", "", Some(def));
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    for (what, result) in [("Check", GeneratorBackend.check(&req).map(|_| ())), ("Build", GeneratorBackend.build(&req).map(|_| ()))] {
        let ds = match result {
            Err(WebBuildError::Refused(ds)) => ds,
            other => panic!("{what}: expected a refusal, got {other:?}"),
        };
        let text = ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("main-form") && text.contains("IXF-1") && text.contains("PART-QTY"), "{what}: {text}");
        println!("{what} → {text}");
    }
    assert!(generated(&dir).is_empty(), "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_controls_generated_paragraphs_build_too_when_the_form_declares_the_file() {
    // The form's own SELECT and FD (the Indexed File Editor's), so the control's
    // `<id>-OPEN … -CLOSE` paragraphs exist; their verbs go to the backend as well.
    let (manifest, dir) = project("facade", "", None);
    let path = dir.join("webforms/main-form.cfrm");
    let mut f = form("", "       PROCEDURE DIVISION.\n           MOVE \"Hi\" TO LBL-OUT::Caption.\n");
    let def = fixtures::parts_cidx("data/parts.idx");
    f.cobol_structure.file_control = cobolt_codegen::generate_indexed_select(&def);
    f.cobol_structure.file_section = cobolt_codegen::generate_indexed_fd(&def).replacen("FD  PARTS", "FD  PARTS IS GLOBAL", 1);
    for c in &mut f.controls {
        if c.id == "IXF-1" {
            c.set_prop("RecordName", PropValue::String("PART-REC".into()));
            c.set_prop("KeyName", PropValue::String("PART-ID".into()));
        }
    }
    cobolt_forms::xml::save_form(&f, &path).unwrap();
    let req = WebBuildRequest::from_manifest(&manifest).unwrap();
    GeneratorBackend.check(&req).unwrap_or_else(|e| panic!("Check: {e}"));
    GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("Build: {e}"));
    assert_eq!(generated(&dir).len(), 10, "nine programs and exports.toml: {:?}", generated(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

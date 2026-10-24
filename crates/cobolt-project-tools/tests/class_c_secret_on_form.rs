// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T13 (R14, AC9) on a real Web project: a class C control holding a
//! connection string or a key on a web form stops Check and Build, naming the
//! control and the property and never the value — a connection string with no
//! password included, which the bundle's shape scan lets through. The same
//! values in `backend/backend.toml`, the form naming them in `BackendConfig`,
//! build.

use cobolt_forms::model::{Control, ControlType, Form, PropValue};
use cobolt_forms::web_class::BACKEND_CONFIG;
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest, WebDiagnosticKind};
use std::path::PathBuf;

const CONNECTION: &str = "postgres://db.example.com/sales";
const KEY: &str = "plain-words-key-0042";

/// A Web project with one form holding an AgentObject and, `with_db`, a
/// SqlDatabase. With `on_form`, the values are in the form; otherwise the
/// controls name backend configurations and `backend/backend.toml` holds them.
fn project(tag: &str, on_form: bool, with_db: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-t13-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("webforms")).unwrap();
    std::fs::create_dir_all(dir.join("backend/src")).unwrap();
    std::fs::write(
        dir.join("Shop.project.toml"),
        "[project]\nname = \"Shop.project\"\nversion = \"1.0.0\"\nmain = \"\"\nkind = \"web\"\n\n\
         [files]\nforms = [\"webforms/main-form.wcfrm\"]\nbackend = [\"backend/src/health.cbl\"]\n\n\
         [web]\nbackend_url = \"wss://api.example.com/ws\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("backend/src/health.cbl"), cobolt_project_tools::create::starter_backend("Shop")).unwrap();
    std::fs::write(dir.join("backend/exports.toml"), cobolt_project_tools::create::starter_backend_exports("Shop")).unwrap();
    let mut form = Form::new("main-form", "Shop", 400, 300);
    let mut db = Control::new("DB-1", ControlType::SqlDatabase, 10, 10);
    let mut agent = Control::new("AGENT-1", ControlType::AgentObject, 10, 40);
    if on_form {
        // The key is written into the file below: the designer never saves a
        // credential (`connections::CREDENTIAL_PROPS`), so a key reaches a
        // `.wcfrm` only by a hand edit — which is what the Build must catch.
        db.set_prop("ConnectionString", PropValue::String(CONNECTION.into()));
    } else {
        db.set_prop("ConnectionString", PropValue::String(String::new()));
        db.set_prop(BACKEND_CONFIG, PropValue::String("SALES".into()));
        agent.set_prop(BACKEND_CONFIG, PropValue::String("ASSISTANT".into()));
        std::fs::write(
            dir.join("backend/backend.toml"),
            format!(
                "[server]\nlisten = \"127.0.0.1:8443\"\n\n[[configuration]]\nname = \"ASSISTANT\"\nAgentAPIKey = \"{KEY}\"\n"
            ),
        )
        .unwrap();
    }
    if with_db {
        form.controls.push(db);
    }
    form.controls.push(agent);
    let path = dir.join("webforms/main-form.wcfrm");
    cobolt_forms::xml::save_form(&form, &path).unwrap();
    if on_form {
        let text = std::fs::read_to_string(&path).unwrap();
        let planted = format!("<Property name=\"AgentAPIKey\">{KEY}</Property>");
        let with_key = text
            .replace("<Property name=\"AgentAPIKey\"></Property>", &planted)
            .replace("<Property name=\"AgentAPIKey\"/>", &planted)
            .replace("<Property name=\"AgentAPIKey\" />", &planted);
        assert!(with_key.contains(&planted), "the saved form names an empty AgentAPIKey to fill");
        std::fs::write(&path, with_key).unwrap();
    }
    dir
}

fn secret_subjects(result: Result<(), WebBuildError>) -> Vec<String> {
    match result {
        Ok(()) => Vec::new(),
        Err(WebBuildError::Refused(ds)) => {
            ds.iter().filter(|d| d.kind == WebDiagnosticKind::SecretOnForm).map(|d| d.subject.clone()).collect()
        }
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn a_connection_string_or_key_on_a_web_form_stops_check_and_build_and_on_the_backend_it_builds() {
    // 1. In the form: Check and Build stop, naming control and property.
    let dir = project("form", true, true);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    for (what, result) in [("Check", GeneratorBackend.check(&req).map(|_| ())), ("Build", GeneratorBackend.build(&req).map(|_| ()))] {
        let ds = match result {
            Err(WebBuildError::Refused(ds)) => ds,
            other => panic!("{what}: expected a refusal, got {other:?}"),
        };
        let secrets: Vec<_> = ds.iter().filter(|d| d.kind == WebDiagnosticKind::SecretOnForm).collect();
        let subjects: Vec<&str> = secrets.iter().map(|d| d.subject.as_str()).collect();
        assert_eq!(subjects, vec!["DB-1.ConnectionString", "AGENT-1.AgentAPIKey"], "{what}: {ds:?}");
        for d in &secrets {
            assert!(d.location.contains("main-form"), "{what}: the form is named: {d}");
            let text = d.to_string();
            assert!(!text.contains(CONNECTION) && !text.contains("example.com") && !text.contains(KEY), "{what}: the value is never shown: {text}");
            println!("{what} → {text}");
        }
    }
    assert!(!req.out_dir.join("index.html").exists(), "nothing was written");
    let _ = std::fs::remove_dir_all(&dir);

    // 2. The key in backend/backend.toml, the form naming it: the Build goes on.
    let dir = project("backend", false, false);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    GeneratorBackend.check(&req).unwrap_or_else(|e| panic!("Check: the key on the backend must not stop it: {e}"));
    let report = GeneratorBackend.build(&req).unwrap_or_else(|e| panic!("Build: the key on the backend must not stop it: {e}"));
    println!("key on the backend → built: {} bytes", report.size.total_bytes);
    let _ = std::fs::remove_dir_all(&dir);

    // 3. The SqlDatabase named by BackendConfig: no secret refusal. (A web form's
    //    SqlDatabase is still refused by the generator for the calls its form
    //    code makes until the remote-lowering pass, T14, turns them into calls
    //    to the backend; that refusal is not this check's.)
    let dir = project("backend-db", false, true);
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    assert!(secret_subjects(GeneratorBackend.check(&req)).is_empty(), "nothing on the form is a secret");
    let _ = std::fs::remove_dir_all(&dir);
}

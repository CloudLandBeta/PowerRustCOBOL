// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-005 T15 (operator, 2026-10-30): a DataGrid bound to a `RestApi` or an
//! `AgentAi` source waits for a spec of its own, so on a real Web project
//! Check and Build both refuse it, naming the form and the grid — the form
//! is read back from its `.wcfrm`, as the IDE saves it.

use cobolt_forms::model::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor, Control,
    ControlType, DataBindingDef, Form,
};
use cobolt_project_tools::web::generator::GeneratorBackend;
use cobolt_project_tools::web::{WebBuildBackend, WebBuildError, WebBuildRequest};
use std::path::PathBuf;

fn project() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("prc-t15-rest-agent-{}", std::process::id()));
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
    std::fs::write(
        dir.join("backend/src/health.cbl"),
        cobolt_project_tools::create::starter_backend("Shop"),
    )
    .unwrap();
    std::fs::write(
        dir.join("backend/exports.toml"),
        cobolt_project_tools::create::starter_backend_exports("Shop"),
    )
    .unwrap();
    let mut form = Form::new("main-form", "Shop", 400, 300);
    form.controls
        .push(Control::new("GRID-REST", ControlType::DataGrid, 10, 10));
    form.controls
        .push(Control::new("GRID-AGENT", ControlType::DataGrid, 10, 150));
    let fields = vec![
        BindingField::new("ID", BindingDataType::Integer),
        BindingField::new("NAME", BindingDataType::Text),
    ];
    form.data_bindings.push(DataBindingDef::new(
        "rest",
        "Rest",
        BindingSourceDescriptor::RestApi {
            source_control_id: "REST-1".into(),
            endpoint_name: "GET-CUSTOMERS".into(),
            response_data_item: "REST-RESPONSE".into(),
            fields: fields.clone(),
            update: None,
        },
        BindingTargetDescriptor::DataGrid {
            control_id: "GRID-REST".into(),
        },
    ));
    form.data_bindings.push(DataBindingDef::new(
        "agent",
        "Agent",
        BindingSourceDescriptor::AgentAi {
            source_control_id: "AGENT-1".into(),
            output_name: "AGENT-1-OUTPUT".into(),
            fields,
            update: None,
        },
        BindingTargetDescriptor::DataGrid {
            control_id: "GRID-AGENT".into(),
        },
    ));
    cobolt_forms::xml::save_form(&form, &dir.join("webforms/main-form.wcfrm")).unwrap();
    dir
}

#[test]
fn a_web_forms_grid_bound_to_rest_or_agent_stops_check_and_build_naming_form_and_grid() {
    let dir = project();
    let req = WebBuildRequest::from_manifest(&dir.join("Shop.project.toml")).unwrap();
    for (what, result) in [
        ("Check", GeneratorBackend.check(&req).map(|_| ())),
        ("Build", GeneratorBackend.build(&req).map(|_| ())),
    ] {
        let ds = match result {
            Err(WebBuildError::Refused(ds)) => ds,
            other => panic!("{what}: expected a refusal, got {other:?}"),
        };
        let texts: Vec<String> = ds.iter().map(|d| d.to_string()).collect();
        for (grid, kind) in [("GRID-REST", "RestApi"), ("GRID-AGENT", "AgentAi")] {
            let named = format!("main-form: DataGrid {grid}: binding");
            assert!(
                texts.iter().any(|t| t.contains("main-form")
                    && t.contains(&named)
                    && t.contains(&format!("({kind}) cannot run on a web or mobile form yet"))),
                "{what}: {grid} refused: {texts:#?}"
            );
        }
        for t in &texts {
            println!("{what} → {t}");
        }
    }
    assert!(
        !req.out_dir.join("index.html").exists(),
        "nothing was written"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

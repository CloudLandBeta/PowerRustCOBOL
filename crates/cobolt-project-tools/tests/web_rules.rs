// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! WASM-015 T18 (AC10): what the plugin teaches about the web kinds is held to the class
//! tables, so the agent is never told one thing while the product refuses another.

use cobolt_forms::model::ControlType;
use cobolt_forms::web_class::{class_of, exceptions_of, Class};
use cobolt_project_tools::content::{self, BuildInput, KitContent, RcrunLocation, ToolInfo, APPLICATION_RULES};

fn rule(id: &str) -> &'static str {
    APPLICATION_RULES.iter().find(|(i, _)| *i == id).unwrap_or_else(|| panic!("a `{id}` rule")).1
}

fn kit() -> KitContent {
    content::build(BuildInput {
        ide_version: "1.test".into(),
        project_name: "Demo".into(),
        ide_url: "http://127.0.0.1:5720/mcp".into(),
        rcrun: RcrunLocation::OnPath,
        tools: ["list_files", "check", "regenerate", "add_to_project", "build", "validate", "kb_lookup", "render_form"]
            .into_iter()
            .map(|n| ToolInfo { name: n.into(), description: String::new() })
            .collect(),
        reference: Vec::new(),
    })
}

#[test]
fn web_kinds_names_every_class_d_thing() {
    let text = rule("web-kinds");
    let mut named = Vec::new();
    // Whole controls of class D are plug-in controls.
    let d_controls: Vec<_> = ControlType::ALL.iter().filter(|t| class_of(t) == Class::D).collect();
    if !d_controls.is_empty() {
        assert!(text.contains("plug-in"), "class D controls are plug-in controls: {text}");
        named.push("plug-in".to_string());
    }
    // Operations of other controls.
    for t in ControlType::ALL {
        for e in exceptions_of(t).iter().filter(|e| e.class == Class::D) {
            let n = e.operation.name();
            assert!(text.contains(n), "class D `{n}` of {t:?} is named in web-kinds");
            named.push(n.to_string());
        }
    }
    println!("web-kinds names {} class D things: {}", named.len(), named.join(", "));
}

#[test]
fn web_kinds_class_c_goes_through_the_backend_and_the_subset_is_stated() {
    let text = rule("web-kinds");
    for needle in ["Class C", "Backend", "backend configuration", "in-memory INDEXED", "web view", "secret scan"] {
        assert!(text.contains(needle), "web-kinds says `{needle}`");
    }
    assert!(!text.contains("because `check` does not"), "check does run the web gates now");
}

#[test]
fn web_layers_and_backend_secrets_are_rules_and_fifteen_in_all() {
    assert_eq!(APPLICATION_RULES.len(), 15);
    let layers = rule("web-layers");
    assert!(layers.contains("LAYER-NAME::Property") && layers.contains("never a control") && layers.contains("Non-Visuals"));
    assert!(rule("backend-secrets").contains("connection string"));
}

#[test]
fn the_skills_teach_layers_and_the_gates_and_the_reviewer_rejects_class_d() {
    let kit = kit();
    let web = kit.skills.iter().find(|s| s.name == "powerrustcobol-web-application").expect("web skill").steps.join("\n");
    assert!(web.contains("LAYER-NAME::Property") && web.contains("also runs the gates"), "{web}");
    let be = kit.skills.iter().find(|s| s.name == "powerrustcobol-backend").expect("backend skill").steps.join("\n");
    assert!(be.contains("secret_scan") && be.contains("backend configuration"), "{be}");
    let checks = kit.reviewer.checks.join("\n");
    assert!(checks.contains("class D") && checks.contains("class C"), "{checks}");
    assert!(checks.contains("SqlDatabase") && checks.contains("a class C control in a project with no Backend"), "{checks}");
}

/// WASM-015 AC11: the plugin teaches the program-level database. The Backend skill and the SQL
/// skill name `COBOL::SqlDatabase` and its verbs, the reviewer rejects a SqlDatabase control and
/// any use of it in a form, and the overview lists it — so an agent never reaches for `EXEC SQL`
/// only, nor puts the control on a web form.
#[test]
fn the_plugin_teaches_the_program_level_database_and_the_reviewer_rejects_it_in_a_form() {
    let kit = kit();
    let skill = |name: &str| kit.skills.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("a `{name}` skill")).steps.join("\n");
    let be = skill("powerrustcobol-backend");
    assert!(be.contains("COBOL::SqlDatabase::Open") && be.contains("::Fetch()") && be.contains("::LastError"), "{be}");
    assert!(be.contains("no SqlDatabase control") || be.contains("it has no SqlDatabase control"), "{be}");
    let sql = skill("powerrustcobol-use-sql");
    assert!(sql.contains("COBOL::SqlDatabase::Open") && sql.contains("Never add a SqlDatabase control to a Web or Mobile form"), "{sql}");
    assert!(sql.contains("statement TEXT only") && sql.contains("EXEC SQL"), "it says what the object cannot do: {sql}");
    let checks = kit.reviewer.checks.join("\n");
    assert!(checks.contains("any use of `COBOL::SqlDatabase` in a form's handler"), "{checks}");
    assert!(content::server_instructions("1.test").contains("COBOL::SqlDatabase"), "the overview lists the object");
}

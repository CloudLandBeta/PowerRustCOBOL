// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 077 AC10 (R15): a form moves a TextBox's `Text` into a `PIC N(30)`
//! item and from it into a Label's `Caption`; the Caption reads back the
//! same characters. Driven through the real Run Form host, off screen.

use std::path::Path;
use std::sync::Arc;

use cobolt_mcp::McpHandler;
use cobolt_project_tools::tools::{run, Shared};
use cobolt_project_tools::{HeadlessHost, ProjectTools};
use serde_json::{json, Value};

const FORM: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="NAT-FORM" title="National" width="420" height="200" main-form="true">
  <Control id="TXT-IN" type="TextBox" x="20" y="20" w="300" h="28" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Text"></Property>
  </Control>
  <Control id="BTN-GO" type="Button" x="330" y="20" w="70" h="28" tab-order="1" z-order="1" visible="true" enabled="true">
    <Property name="Caption">Go</Property>
    <Event name="onClick" paragraph="BTN-GO--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N   PIC N(30).
       01 WS-LEN PIC 9(4).
       PROCEDURE DIVISION.
           MOVE TXT-IN::Text TO WS-N.
           MOVE WS-N TO LBL-OUT::Caption.
           MOVE FUNCTION BYTE-LENGTH(WS-N) TO WS-LEN.
           MOVE WS-LEN TO LBL-LEN::Caption.
]]></Event>
  </Control>
  <Control id="LBL-OUT" type="Label" x="20" y="70" w="380" h="28" tab-order="2" z-order="2" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
  <Control id="LBL-LEN" type="Label" x="20" y="110" w="380" h="28" tab-order="3" z-order="3" visible="true" enabled="true">
    <Property name="Caption">-</Property>
  </Control>
</Form>
"#;

fn call(tools: &mut ProjectTools<HeadlessHost>, name: &str, args: Value) -> (Value, bool) {
    let r = tools.call_tool(name, &args);
    let mut meta = Value::Null;
    for c in &r.content {
        if let Some(t) = c.as_text() {
            meta = serde_json::from_str(t).unwrap_or(Value::String(t.to_owned()));
        }
    }
    (meta, r.is_error == Some(true))
}

#[test]
fn ac10_a_form_round_trips_text_through_a_national_item() {
    let base = std::env::temp_dir().join(format!("prc-077-form-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let start = base.join("start");
    std::fs::create_dir_all(&start).unwrap();
    let runner = run::rcrun_runner(Path::new(env!("CARGO_BIN_EXE_rcrun")).to_path_buf());
    let shared = Arc::new(Shared::new().with_runner(runner));
    let mut tools = ProjectTools::with_shared(HeadlessHost::new(start.join("none.project.toml"), "test"), shared);

    let project = base.join("National");
    let (made, err) = call(&mut tools, "create_project", json!({"folder": project.to_string_lossy(), "name": "National"}));
    assert!(!err, "{made}");
    std::fs::write(project.join("forms/NAT-FORM.cfrm"), FORM).unwrap();
    let (added, err) = call(&mut tools, "add_to_project", json!({"path": "forms/NAT-FORM.cfrm"}));
    assert!(!err, "{added}");

    let text = "Configuração – ok";
    let (out, err) = call(
        &mut tools,
        "run_form",
        json!({"path": "forms/NAT-FORM.cfrm", "steps": [
            {"set": {"control": "TXT-IN", "property": "Text", "value": text}},
            {"event": {"control": "BTN-GO", "name": "onClick"}},
            {"read": {"control": "LBL-OUT", "property": "Caption"}},
            {"read": {"control": "LBL-LEN", "property": "Caption"}}
        ]}),
    );
    assert!(!err, "{out}");
    // The item's 13 padding spaces are not part of the text a Caption shows,
    // exactly as for `PIC X`; a number reads back without its leading zeros.
    assert_eq!(out["reads"]["LBL-OUT::Caption"], text, "{out}");
    assert_eq!(out["reads"]["LBL-LEN::Caption"], "60", "{out}");
    let _ = std::fs::remove_dir_all(&base);
    println!("077 AC10: TextBox \"{text}\" -> PIC N(30) -> Label, same characters; BYTE-LENGTH 60");
}

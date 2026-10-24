// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The generated module the player's ABI test runs (WASM-010 T21, D3): a form
//! built the way the Build builds it — `cobolt-codegen` writes its program,
//! `compile_project` compiles it — committed under
//! `crates/cobolt-web-player/tests/fixtures/generated/`, because the player is
//! a workspace of its own and does not build the generator. This test fails
//! when the committed module is not what the generator writes today;
//! `UPDATE_PLAYER_FIXTURE=1` rewrites it.

use cobolt_forms::model::{Control, ControlType, EventBinding, Form};
use cobolt_wasm::module::{compile_project, FormSpec};
use cobolt_wasm::Options;
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cobolt-web-player/tests/fixtures/generated")
}

/// `main-form`: BTN-GO sets LBL-OUT's caption, counts its clicks and says so.
fn form() -> Form {
    let mut form = Form::new("main-form", "Generated", 400, 300);
    let mut btn = Control::new("BTN-GO", ControlType::Button, 10, 10);
    let mut ev = EventBinding::for_control("BTN-GO", "onClick");
    ev.code = "       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 WS-CLICKS PIC 9(3) VALUE 0.\n       PROCEDURE DIVISION.\n           ADD 1 TO WS-CLICKS\n           MOVE \"from the module\" TO LBL-OUT::Caption\n           DISPLAY \"clicked \" WS-CLICKS.".to_string();
    btn.events.push(ev);
    form.controls.push(btn);
    form.controls
        .push(Control::new("LBL-OUT", ControlType::Label, 10, 60));
    form
}

fn build(multi: bool) -> Vec<u8> {
    let form = form();
    let (src, _) = cobolt_codegen::generate_with_map(&form);
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(
        &src,
        cobolt_lexer::SourceFormat::Free,
    ));
    let program = parsed.program.expect("the generated program parses");
    let spec = FormSpec {
        name: form.name.clone(),
        main: true,
        instances: 1,
        program: &program,
        controls: form
            .controls
            .iter()
            .map(|c| (c.id.clone(), format!("{:?}", c.control_type)))
            .collect(),
        bindings: vec![("BTN-GO".into(), "onClick".into())],
        layers: vec![],
        lines: vec![],
    };
    let opts = Options {
        multi_memory: multi,
        name: "generated".into(),
        when_compiled: Some("2026102300000000+0000".into()),
        ..Options::default()
    };
    compile_project(std::slice::from_ref(&spec), &opts)
        .unwrap_or_else(|e| panic!("{e}"))
        .wasm
}

#[test]
fn the_players_generated_fixture_is_what_the_generator_writes() {
    let cfrm = cobolt_forms::xml::form_to_string(&form()).unwrap();
    let files = [
        ("program.wasm", build(true)),
        ("program.single.wasm", build(false)),
        ("main-form.cfrm", cfrm.into_bytes()),
    ];
    assert_eq!(
        build(true),
        files[0].1,
        "the generator writes the same module twice"
    );
    if std::env::var("UPDATE_PLAYER_FIXTURE").as_deref() == Ok("1") {
        std::fs::create_dir_all(dir()).unwrap();
        for (name, bytes) in &files {
            std::fs::write(dir().join(name), bytes).unwrap();
        }
    }
    for (name, bytes) in &files {
        let committed = std::fs::read(dir().join(name)).unwrap_or_default();
        assert!(committed == *bytes, "{name} is stale: run UPDATE_PLAYER_FIXTURE=1 cargo test -p cobolt-wasm --test player_fixture");
    }
    eprintln!(
        "player fixture: {} files fresh ({} + {} bytes of module)",
        files.len(),
        files[0].1.len(),
        files[1].1.len()
    );
}

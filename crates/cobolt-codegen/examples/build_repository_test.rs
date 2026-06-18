// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! One-off builder for the `examples/repository-test` project: a form with a
//! single "Run tests" button and a results ListBox. The button handler exercises
//! the curated Rust-FFI bridge (spec 005) by `INVOKE`-ing methods on
//! `OBJECT REFERENCE` items and appending each result as a new ListBox line.
//!
//! Run with:  cargo run -p cobolt-codegen --example build_repository_test

use cobolt_forms::model::{PropValue, Rect};
use cobolt_forms::{save_form, Control, ControlType, EventBinding, Form};
use std::fs;
use std::path::Path;

// OBJECT REFERENCE items live in the FORM's WORKING-STORAGE (the outer program),
// so the interpreter constructs them at start-up from their VALUE; the nested
// button handler then sees them. REPOSITORY (the curated Rust types) is seeded
// into every new form automatically.
const FORM_WS: &str = "
       01 RSTR USAGE OBJECT REFERENCE RUST-STRING VALUE \"Hello\".
       01 RINT USAGE OBJECT REFERENCE RUST-I64 VALUE \"10\".
       01 RFLT USAGE OBJECT REFERENCE RUST-F64 VALUE \"16\".
       01 RBOO USAGE OBJECT REFERENCE RUST-BOOL VALUE \"1\".
       01 RVEC USAGE OBJECT REFERENCE RUST-VEC.";

// The button's onClick handler: a battery of INVOKE tests, each appending a line.
const HANDLER: &str = "
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N    PIC 9(4).
       01 WS-T    PIC X(20).
       01 WS-LINE PIC X(50).

       PROCEDURE DIVISION.
           INVOKE RESULTS \"Clear\"
           INVOKE RESULTS \"AddItem\" USING \"== Rust-FFI bridge tests ==\"

           INVOKE RSTR \"len\" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"String.len()       = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RSTR \"to_uppercase\" RETURNING WS-T
           MOVE SPACES TO WS-LINE
           STRING \"String.to_uppercase = \" DELIMITED BY SIZE
                  WS-T DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RSTR \"contains\" USING \"ell\" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"String.contains ell = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RSTR \"push_str\" USING \"!!\"
           INVOKE RSTR \"len\" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"String.len after +!! = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RINT \"add\" USING 5 RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"i64 10 add 5        = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RINT \"pow\" USING 2 RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"i64 (now 15) pow 2  = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RFLT \"sqrt\" RETURNING WS-T
           MOVE SPACES TO WS-LINE
           STRING \"f64 16 sqrt         = \" DELIMITED BY SIZE
                  WS-T DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RBOO \"not\" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"bool true not       = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RVEC \"push\" USING 7
           INVOKE RVEC \"push\" USING 8
           INVOKE RVEC \"len\" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING \"Vec push x2, len    = \" DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS \"AddItem\" USING WS-LINE

           INVOKE RESULTS \"AddItem\" USING \"== done ==\".";

fn main() {
    let proj_dir = Path::new("examples/repository-test");
    fs::create_dir_all(proj_dir.join("forms")).unwrap();
    fs::create_dir_all(proj_dir.join("generated")).unwrap();

    let mut form = Form::new("REPO-TEST", "Repository / Rust-FFI Test", 520, 420);
    form.user_ws_source = FORM_WS.to_owned();

    let mut btn = Control::new("RUN-TESTS", ControlType::Button, 20, 16);
    btn.rect = Rect::new(20, 16, 240, 34);
    btn.set_prop("Caption", PropValue::String("Run REPOSITORY tests".into()));
    let mut ev = EventBinding::for_control("RUN-TESTS", "onClick");
    ev.code = HANDLER.to_owned();
    btn.events.push(ev);
    form.controls.push(btn);

    let mut list = Control::new("RESULTS", ControlType::ListBox, 20, 60);
    list.rect = Rect::new(20, 60, 480, 340);
    form.controls.push(list);

    let cfrm_rel = "forms/repository-test.cfrm";
    let gen_rel = "generated/repository-test.cbl";
    save_form(&form, &proj_dir.join(cfrm_rel)).expect("save_form");
    let cbl = cobolt_codegen::generate(&form);
    fs::write(proj_dir.join(gen_rel), cbl).unwrap();

    let toml = format!(
        "[project]\nname = \"Repository FFI Test\"\nversion = \"1.0.0\"\nmain = \"{gen_rel}\"\n\n\
         [files]\nforms = [\"{cfrm_rel}\"]\ngenerated = [\"{gen_rel}\"]\n"
    );
    fs::write(proj_dir.join("cobolt.toml"), toml).unwrap();

    let readme = "# Repository / Rust-FFI test\n\n\
        A single **Run REPOSITORY tests** button drives a battery of `INVOKE` calls\n\
        against the curated Rust-FFI bridge (spec 005). Each result is appended as a\n\
        new line in the **results** ListBox. Open `cobolt.toml` in the IDE and press\n\
        **Run**, then click the button.\n";
    fs::write(proj_dir.join("README.md"), readme).unwrap();

    println!("Wrote {}", proj_dir.display());
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Throwaway helper: generate a small valid `.cfrm` for testing the compiler.
//! Usage: cargo run -p cobolt-forms --example gen_sample -- <out_path>

use cobolt_forms::{Control, ControlType, Form, save_form};
use cobolt_forms::model::PropValue;
use std::path::Path;

fn main() {
    let mut form = Form::new("MAIN-FORM", "Form Demo", 420, 200);

    let mut lbl = Control::new("LBL-HELLO", ControlType::Label, 20, 20);
    lbl.set_prop("Caption", PropValue::String("Hello from a compiled RustCOBOL form".into()));
    form.controls.push(lbl);

    let mut btn = Control::new("BTN-OK", ControlType::Button, 20, 70);
    btn.set_prop("Caption", PropValue::String("OK".into()));
    form.controls.push(btn);

    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/main.cfrm".to_owned());
    save_form(&form, Path::new(&out)).expect("save_form");
    println!("wrote {out}");
}

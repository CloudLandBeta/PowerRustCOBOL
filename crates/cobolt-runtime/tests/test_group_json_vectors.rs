// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The golden vectors of the group <-> JSON mapping (WASM-005 R5b), run through
//! the **interpreter's adapter**: the group lives in the environment, is read
//! out through its declaration, encoded, and the answers are decoded and
//! written back into it. The reference implementation runs the same file in
//! `cobolt-wire`; the browser module's generator runs it a third time. All three
//! must print the same tally.

use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_runtime::Interpreter;
use cobolt_wire::node::dump;
use serde_json::Value;

const VECTORS: &str = include_str!("../../../specs/WASM-005-websocket-call-and-remote-data/vectors/group-json.v1.json");

/// An interpreter whose WORKING-STORAGE is `layout`; returns it and the name of
/// the first item.
fn interp_for(layout: &str) -> Result<(Interpreter, String), String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. VECTOR.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{layout}\n       PROCEDURE DIVISION.\n           STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Fixed));
    if !result.diagnostics.is_empty() {
        return Err(format!("the layout does not parse: {:?}", result.diagnostics));
    }
    let program = result.program.ok_or("no program")?;
    let root = program
        .data
        .as_ref()
        .and_then(|d| {
            d.sections.iter().find_map(|s| match s {
                cobolt_ast::program::DataSection::WorkingStorage(items) => items.first().and_then(|i| i.name.clone()),
                _ => None,
            })
        })
        .ok_or("no item")?;
    Ok((Interpreter::new(program), root))
}

fn pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|a| a.iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect())
        .unwrap_or_default()
}

fn trimmed(d: Vec<(String, String)>) -> Vec<(String, String)> {
    d.into_iter().map(|(p, t)| (p, t.trim_end_matches(' ').to_string())).collect()
}

fn state(interp: &mut Interpreter, root: &str) -> Vec<(String, String)> {
    let (item, node) = interp.read_group(root).expect("read the group");
    trimmed(dump(&item, &node))
}

/// A fresh interpreter with `leaves` written into the group.
fn with_leaves(layout: &str, leaves: &[(String, String)]) -> Result<(Interpreter, String), String> {
    let (mut interp, root) = interp_for(layout)?;
    for (path, text) in leaves {
        interp.set_group_leaf(&root, path, text).map_err(|e| format!("initial {path}: {e}"))?;
    }
    Ok((interp, root))
}

#[test]
fn the_interpreters_adapter_passes_every_golden_vector() {
    let doc: Value = serde_json::from_str(VECTORS).expect("the vector file is JSON");
    let (mut cases, mut encodes, mut round_trips, mut ok_reads, mut bad_reads, mut layouts) = (0, 0, 0, 0, 0, 0);
    let mut failures: Vec<String> = Vec::new();

    for case in doc["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap().to_string();
        cases += 1;
        let layout = case["layout"].as_str().unwrap();

        if let Some(expect) = case.get("layout_error") {
            let expect = expect.as_str().unwrap();
            match interp_for(layout) {
                Ok((mut i, root)) => match i.read_group(&root) {
                    Err(e) if e.message.contains(expect) => layouts += 1,
                    other => failures.push(format!("{id}: expected a layout error containing {expect:?}, got {other:?}")),
                },
                Err(e) => failures.push(format!("{id}: {e}")),
            }
            continue;
        }

        let initial = pairs(&case["initial"]);
        let (mut interp, root) = match with_leaves(layout, &initial) {
            Ok(x) => x,
            Err(e) => {
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        let before = state(&mut interp, &root);

        if let Some(expected) = case.get("json").and_then(|j| j.as_str()) {
            encodes += 1;
            match interp.group_to_json(&root) {
                Ok(got) if got == expected => {}
                other => failures.push(format!("{id}: encode\n   expected {expected}\n   got      {other:?}")),
            }
            // Round trip into a blank group (or the case's own `blank`).
            let blank = case.get("blank").map(pairs).unwrap_or_default();
            match with_leaves(layout, &blank) {
                Ok((mut b, broot)) => match b.group_from_json(&broot, expected) {
                    Ok(()) => {
                        round_trips += 1;
                        let got = state(&mut b, &broot);
                        if got != before {
                            failures.push(format!("{id}: round trip differs\n  {got:?}\n  {before:?}"));
                        }
                    }
                    Err(e) => failures.push(format!("{id}: round trip failed: {e}")),
                },
                Err(e) => failures.push(format!("{id}: {e}")),
            }
        }
        if let Some(code) = case.get("encode_error").and_then(|c| c.as_str()) {
            encodes += 1;
            match interp.group_to_json(&root) {
                Err(e) if e.message.starts_with(&format!("{code} at ")) => {
                    if let Some(p) = case.get("encode_error_path").and_then(|p| p.as_str()) {
                        if e.detail.as_deref() != Some(p) {
                            failures.push(format!("{id}: encode error path {:?} != {p}", e.detail));
                        }
                    }
                }
                other => failures.push(format!("{id}: expected encode error {code}, got {other:?}")),
            }
        }

        for read in case.get("reads").and_then(|r| r.as_array()).cloned().unwrap_or_default() {
            let text = read["json"].as_str().unwrap();
            let (mut r, rroot) = with_leaves(layout, &initial).expect("initial applied once already");
            let outcome = r.group_from_json(&rroot, text);
            match (outcome, read.get("error")) {
                (Ok(()), None) => {
                    ok_reads += 1;
                    let mut want = before.clone();
                    for (p, v) in pairs(&read["changes"]) {
                        match want.iter_mut().find(|(wp, _)| *wp == p) {
                            Some(slot) => slot.1 = v,
                            None => failures.push(format!("{id}: read {text}: change names unknown leaf {p}")),
                        }
                    }
                    let got = state(&mut r, &rroot);
                    if got != want {
                        let diff: Vec<String> = got
                            .iter()
                            .zip(want.iter())
                            .filter(|(g, w)| g != w)
                            .map(|(g, w)| format!("{}: got {:?} want {:?}", g.0, g.1, w.1))
                            .collect();
                        failures.push(format!("{id}: read {text}\n   {}", diff.join("\n   ")));
                    }
                }
                (Err(e), Some(code)) => {
                    bad_reads += 1;
                    let code = code.as_str().unwrap();
                    if !e.message.starts_with(&format!("{code} at ")) {
                        failures.push(format!("{id}: read {text}: expected {code}, got {e}"));
                    }
                    if let Some(p) = read.get("path").and_then(|p| p.as_str()) {
                        if e.detail.as_deref() != Some(p) {
                            failures.push(format!("{id}: read {text}: error path {:?} != {p}", e.detail));
                        }
                    }
                    if state(&mut r, &rroot) != before {
                        failures.push(format!("{id}: read {text}: the group changed although the read was refused"));
                    }
                }
                (Ok(()), Some(code)) => failures.push(format!("{id}: read {text}: expected {code}, but it was accepted")),
                (Err(e), None) => failures.push(format!("{id}: read {text}: unexpected error {e}")),
            }
        }
    }

    println!(
        "\n  group <-> JSON golden vectors (interpreter adapter): {cases} cases, {encodes} encodings, {round_trips} round trips, {ok_reads} reads accepted, {bad_reads} reads refused, {layouts} layouts refused"
    );
    assert!(failures.is_empty(), "{} vector failures:\n{}", failures.len(), failures.join("\n"));
}

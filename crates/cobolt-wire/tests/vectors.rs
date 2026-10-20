// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The golden vectors of the group <-> JSON mapping (WASM-005 R5b), run against
//! the reference implementation.
//!
//! The vectors live in `specs/WASM-005-websocket-call-and-remote-data/vectors/`
//! in a language-neutral form (see `vectors/README.md` there) and are run
//! unchanged by the interpreter's adapter (`cobolt-runtime`) and by the browser
//! module's generator. This runner is the reference: a vector this crate fails
//! is a mistake in the vector or in the rules, never "an implementation detail".

use cobolt_ast::program::DataSection;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_wire::node::{dump, set_by_path};
use cobolt_wire::{decode, encode, from_decl, Item, Node};
use serde_json::Value;

const VECTORS: &str = include_str!("../../../specs/WASM-005-websocket-call-and-remote-data/vectors/group-json.v1.json");

/// The layout of the first 01/77 item of `layout` (a DATA DIVISION fragment).
fn layout_of(layout: &str) -> Result<Item, String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. VECTOR.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{layout}\n       PROCEDURE DIVISION.\n           STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Fixed));
    if !result.diagnostics.is_empty() {
        return Err(format!("the layout does not parse: {:?}", result.diagnostics));
    }
    let program = result.program.ok_or("no program")?;
    for section in &program.data.as_ref().ok_or("no data division")?.sections {
        if let DataSection::WorkingStorage(items) = section {
            let decl = items.first().ok_or("no item")?;
            return from_decl(decl).map_err(|e| e.to_string());
        }
    }
    Err("no WORKING-STORAGE".into())
}

fn trimmed(d: Vec<(String, String)>) -> Vec<(String, String)> {
    d.into_iter().map(|(p, t)| (p, t.trim_end_matches(' ').to_string())).collect()
}

fn pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|a| a.iter().map(|p| (p[0].as_str().unwrap().to_string(), p[1].as_str().unwrap().to_string())).collect())
        .unwrap_or_default()
}

struct Tally {
    cases: usize,
    encodes: usize,
    reads_ok: usize,
    reads_err: usize,
    layout_errors: usize,
    round_trips: usize,
}

#[test]
fn the_reference_implementation_passes_every_golden_vector() {
    let doc: Value = serde_json::from_str(VECTORS).expect("the vector file is JSON");
    assert_eq!(doc["format"], "prc-group-json-vectors");
    let mut t = Tally { cases: 0, encodes: 0, reads_ok: 0, reads_err: 0, layout_errors: 0, round_trips: 0 };
    let mut failures: Vec<String> = Vec::new();

    for case in doc["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("id").to_string();
        t.cases += 1;
        let layout_src = case["layout"].as_str().expect("layout");

        // A layout the wire refuses is itself a vector.
        if let Some(expect) = case.get("layout_error") {
            match layout_of(layout_src) {
                Err(msg) if msg.contains(expect.as_str().unwrap()) => t.layout_errors += 1,
                other => failures.push(format!("{id}: expected a layout error containing {expect}, got {other:?}")),
            }
            continue;
        }

        let item = match layout_of(layout_src) {
            Ok(i) => i,
            Err(e) => {
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        let mut initial = Node::initial(&item);
        for (path, text) in pairs(&case["initial"]) {
            if let Err(e) = set_by_path(&item, &mut initial, &path, &text) {
                failures.push(format!("{id}: initial {path}: {e}"));
            }
        }

        // Encoding.
        if let Some(expected) = case.get("json").and_then(|j| j.as_str()) {
            t.encodes += 1;
            match encode(&item, &initial) {
                Ok(got) if got == expected => {}
                other => failures.push(format!("{id}: encode\n   expected {expected}\n   got      {other:?}")),
            }
            // Round trip: the text read into a blank group reproduces the original.
            let mut blank = Node::initial(&item);
            if let Some(blank_paths) = case.get("blank") {
                for (path, text) in pairs(blank_paths) {
                    let _ = set_by_path(&item, &mut blank, &path, &text);
                }
            }
            match decode(&item, &blank, expected, 1 << 20) {
                Ok(back) => {
                    t.round_trips += 1;
                    if trimmed(dump(&item, &back)) != trimmed(dump(&item, &initial)) {
                        failures.push(format!("{id}: round trip differs\n  {:?}\n  {:?}", dump(&item, &back), dump(&item, &initial)));
                    }
                }
                Err(e) => failures.push(format!("{id}: round trip failed: {e}")),
            }
        }
        if let Some(code) = case.get("encode_error").and_then(|c| c.as_str()) {
            t.encodes += 1;
            match encode(&item, &initial) {
                Err(e) if e.kind.code() == code => {
                    if let Some(p) = case.get("encode_error_path").and_then(|p| p.as_str()) {
                        if e.path != p {
                            failures.push(format!("{id}: encode error path {} != {p}", e.path));
                        }
                    }
                }
                other => failures.push(format!("{id}: expected encode error {code}, got {other:?}")),
            }
        }

        // Reads.
        for read in case.get("reads").and_then(|r| r.as_array()).cloned().unwrap_or_default() {
            let text = read["json"].as_str().expect("read json");
            let before = trimmed(dump(&item, &initial));
            match (decode(&item, &initial, text, 1 << 20), read.get("error")) {
                (Ok(after), None) => {
                    t.reads_ok += 1;
                    let mut want = before.clone();
                    for (p, v) in pairs(&read["changes"]) {
                        match want.iter_mut().find(|(wp, _)| *wp == p) {
                            Some(slot) => slot.1 = v,
                            None => failures.push(format!("{id}: read {text}: change names unknown leaf {p}")),
                        }
                    }
                    let got = trimmed(dump(&item, &after));
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
                    t.reads_err += 1;
                    if e.kind.code() != code.as_str().unwrap() {
                        failures.push(format!("{id}: read {text}: expected {code}, got {e}"));
                    }
                    if let Some(p) = read.get("path").and_then(|p| p.as_str()) {
                        if e.path != p {
                            failures.push(format!("{id}: read {text}: error path {} != {p}", e.path));
                        }
                    }
                    // The group is untouched: decode returns a new tree and `initial` is borrowed.
                    if trimmed(dump(&item, &initial)) != before {
                        failures.push(format!("{id}: read {text}: the original changed"));
                    }
                }
                (Ok(_), Some(code)) => failures.push(format!("{id}: read {text}: expected error {code}, but it was accepted")),
                (Err(e), None) => failures.push(format!("{id}: read {text}: unexpected error {e}")),
            }
        }
    }

    println!(
        "\n  group <-> JSON golden vectors (reference implementation): {} cases, {} encodings, {} round trips, {} reads accepted, {} reads refused, {} layouts refused",
        t.cases, t.encodes, t.round_trips, t.reads_ok, t.reads_err, t.layout_errors
    );
    assert!(failures.is_empty(), "{} vector failures:\n{}", failures.len(), failures.join("\n"));
}

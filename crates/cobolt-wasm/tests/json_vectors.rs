// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The golden vectors of the group ⇄ JSON mapping (WASM-005 R5b; WASM-010 T15)
//! through the module's own implementation: the routines of
//! `cobolt_wasm_rt::json`, on the descriptor `cobolt_wasm::json_layout` builds
//! and the region image the generator initialises, in the routines' native
//! build (the code linked into every module, run against a test memory — the
//! routines' own testing rule). Every answer is compared with the vector and
//! with `cobolt-wire`'s, the reference.

use cobolt_ast::program::DataSection;
use cobolt_decimal::CobolNumeric;
use cobolt_lexer::{tokenize, SourceFormat};
use cobolt_parser::parse;
use cobolt_wasm::json_layout::{self, Leaf};
use cobolt_wasm::layout::Layout;
use cobolt_wasm_rt::codec;
use cobolt_wasm_rt::json;
use cobolt_wasm_rt::mem;
use cobolt_wasm_rt::opd::Loc;
use cobolt_wasm_rt::testkit::Kit;
use cobolt_wasm_rt::val::Val;
use cobolt_wire::layout::Kind;
use cobolt_wire::node::{dump, set_by_path};
use cobolt_wire::{decode, encode, from_decl, Node};
use serde_json::Value;

const VECTORS: &str = include_str!(
    "../../../specs/WASM-005-websocket-call-and-remote-data/vectors/group-json.v1.json"
);

fn pairs(v: &Value) -> Vec<(String, String)> {
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|p| {
                    (
                        p[0].as_str().unwrap().to_string(),
                        p[1].as_str().unwrap().to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn trimmed(d: Vec<(String, String)>) -> Vec<(String, String)> {
    d.into_iter()
        .map(|(p, t)| (p, t.trim_end_matches(' ').to_string()))
        .collect()
}

/// A group of the vectors, laid out by the generator in a test memory.
struct Group {
    kit: Kit,
    image: Vec<u8>,
    addr: u32,
    desc: u32,
    leaves: Vec<Leaf>,
    out: u32,
    text: u32,
}

const OUT_CAP: u32 = 16 * 1024;

impl Group {
    /// Back to the generator's initial image (VALUE clauses, zeros, spaces).
    fn reset(&mut self) {
        let base = self.kit.base;
        self.kit.mem.put(base, &self.image);
    }
    fn set(&mut self, path: &str, text: &str) -> Result<(), String> {
        let leaf = self
            .leaves
            .iter()
            .find(|l| l.path == path)
            .ok_or_else(|| format!("no leaf {path}"))?
            .clone();
        let at = self.addr + leaf.off;
        let cap = leaf.fd.len;
        match &leaf.kind {
            Kind::Numeric {
                digits,
                scale,
                signed,
            } => {
                let m =
                    json::number_at_scale(text.as_bytes(), *digits as u32, *scale as u32, *signed)
                        .map_err(|e| format!("{path}: {e:?}"))?;
                codec::store_value(
                    &Loc {
                        addr: at,
                        len: cap,
                        fd: leaf.fd,
                    },
                    Val::Dec(CobolNumeric::new(m, *scale as u8)),
                    false,
                    false,
                );
            }
            Kind::Alphanumeric {
                justified: true, ..
            } => {
                let b = text.as_bytes();
                let take = b.len().min(cap as usize);
                let mut v = vec![b' '; cap as usize - take];
                v.extend_from_slice(&b[b.len() - take..]);
                self.kit.mem.put(at, &v);
            }
            _ => {
                let mut v = text.as_bytes().to_vec();
                v.truncate(cap as usize);
                v.resize(cap as usize, b' ');
                self.kit.mem.put(at, &v);
            }
        }
        Ok(())
    }
    fn dump(&self) -> Vec<(String, String)> {
        self.leaves
            .iter()
            .map(|l| {
                let at = self.addr + l.off;
                let text = match &l.kind {
                    Kind::Numeric { scale, .. } => match codec::load_value(&Loc {
                        addr: at,
                        len: l.fd.len,
                        fd: l.fd,
                    }) {
                        Val::Dec(n) => {
                            let m = n.mantissa
                                * 10i128.pow((*scale as u32).saturating_sub(n.decimals as u32));
                            cobolt_wire::number::from_mantissa(m, *scale)
                        }
                        Val::F(f) => format!("float {f}"),
                    },
                    _ => String::from_utf8_lossy(self.kit.mem.get(at, l.fd.len)).into_owned(),
                };
                (l.path.clone(), text.trim_end_matches(' ').to_string())
            })
            .collect()
    }
    fn encode(&self) -> Result<String, (String, String)> {
        let n = json::encode(self.desc, self.addr, self.out, OUT_CAP);
        if n < 0 {
            return Err((
                json::code_name(-n).to_string(),
                String::from_utf8_lossy(json::error_path()).into_owned(),
            ));
        }
        Ok(String::from_utf8(self.kit.mem.get(self.out, n as u32).to_vec()).unwrap())
    }
    fn decode(&mut self, text: &str) -> Result<(), (String, String)> {
        let t = self.text;
        self.kit.mem.put(t, text.as_bytes());
        let r = json::decode(self.desc, self.addr, t, text.len() as u32);
        if r < 0 {
            return Err((
                json::code_name(-r).to_string(),
                String::from_utf8_lossy(json::error_path()).into_owned(),
            ));
        }
        Ok(())
    }
}

/// The first WORKING-STORAGE item of `layout` (fixed format), as the wire and
/// the generator each see it.
fn group_of(layout_src: &str) -> Result<(cobolt_wire::Item, Group), String> {
    let src = format!(
        "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. VECTOR.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n{layout_src}\n       PROCEDURE DIVISION.\n           STOP RUN.\n"
    );
    let result = parse(tokenize(&src, SourceFormat::Fixed));
    if !result.diagnostics.is_empty() {
        return Err(format!(
            "the layout does not parse: {:?}",
            result.diagnostics
        ));
    }
    let program = result.program.ok_or("no program")?;
    let decls = program
        .data
        .as_ref()
        .and_then(|d| {
            d.sections.iter().find_map(|s| {
                if let DataSection::WorkingStorage(items) = s {
                    Some(items.clone())
                } else {
                    None
                }
            })
        })
        .ok_or("no WORKING-STORAGE")?;
    let item = from_decl(decls.first().ok_or("no item")?).map_err(|e| e.to_string())?;
    let mut layout = Layout::new();
    layout.begin_unit(0, vec![0]);
    layout.add_section(&decls).map_err(|e| e.to_string())?;
    layout.end_unit();
    layout.finish();
    let root = layout.roots[0];
    let blob = json_layout::build(&layout, root, &item)?;
    let leaves = json_layout::leaves(&layout, root, &item)?;
    let image = cobolt_wasm::image::build(&layout, false, '$')
        .map_err(|e| e.to_string())?
        .bytes;
    let mut kit = Kit::new();
    assert!(image.len() <= 16 * 1024, "the test region holds 16 KiB");
    let desc = kit.put(&blob);
    let out = kit.alloc(OUT_CAP);
    let text = kit.alloc(64 * 1024);
    let addr = kit.base + layout.items[root].off;
    let mut g = Group {
        kit,
        image,
        addr,
        desc,
        leaves,
        out,
        text,
    };
    g.reset();
    Ok((item, g))
}

#[derive(Default)]
struct Tally {
    cases: usize,
    encodes: usize,
    round_trips: usize,
    reads_ok: usize,
    reads_err: usize,
    layout_errors: usize,
    equal_to_reference: usize,
}

#[test]
fn every_golden_vector_through_the_module_routines() {
    let doc: Value = serde_json::from_str(VECTORS).expect("the vector file is JSON");
    let mut t = Tally::default();
    let mut failures: Vec<String> = Vec::new();
    for case in doc["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().unwrap().to_string();
        t.cases += 1;
        let layout_src = case["layout"].as_str().unwrap();
        if let Some(expect) = case.get("layout_error").and_then(|e| e.as_str()) {
            match group_of(layout_src) {
                Err(msg) if msg.contains(expect) => t.layout_errors += 1,
                Err(msg) => failures.push(format!("{id}: refused, but not with {expect:?}: {msg}")),
                Ok(_) => failures.push(format!("{id}: the layout should be refused ({expect})")),
            }
            continue;
        }
        let (item, mut g) = match group_of(layout_src) {
            Ok(x) => x,
            Err(e) => {
                failures.push(format!("{id}: {e}"));
                continue;
            }
        };
        let initial = pairs(&case["initial"]);
        let mut reference = Node::initial(&item);
        for (p, v) in &initial {
            set_by_path(&item, &mut reference, p, v).unwrap();
        }
        let load_initial = |g: &mut Group, failures: &mut Vec<String>| {
            g.reset();
            for (p, v) in &initial {
                if let Err(e) = g.set(p, v) {
                    failures.push(format!("{id}: initial {e}"));
                }
            }
        };
        load_initial(&mut g, &mut failures);
        let before = g.dump();
        if before != trimmed(dump(&item, &reference)) {
            failures.push(format!(
                "{id}: the initial state differs from the reference's\n  {before:?}\n  {:?}",
                trimmed(dump(&item, &reference))
            ));
            continue;
        }

        if let Some(expected) = case.get("json").and_then(|j| j.as_str()) {
            t.encodes += 1;
            let got = g.encode();
            let reference_text = encode(&item, &reference).ok();
            match &got {
                Ok(s) if s == expected => {
                    if reference_text.as_deref() == Some(s.as_str()) {
                        t.equal_to_reference += 1;
                    }
                }
                other => failures.push(format!(
                    "{id}: encode\n   expected {expected}\n   got      {other:?}"
                )),
            }
            // The round trip: the text read into a fresh group reproduces it.
            g.reset();
            for (p, v) in pairs(case.get("blank").unwrap_or(&Value::Null)) {
                let _ = g.set(&p, &v);
            }
            match g.decode(expected) {
                Ok(()) => {
                    t.round_trips += 1;
                    if g.dump() != before {
                        failures.push(format!(
                            "{id}: round trip differs\n  {:?}\n  {before:?}",
                            g.dump()
                        ));
                    }
                }
                Err(e) => failures.push(format!("{id}: round trip failed: {e:?}")),
            }
            load_initial(&mut g, &mut failures);
        }
        if let Some(code) = case.get("encode_error").and_then(|c| c.as_str()) {
            t.encodes += 1;
            let reference_err = encode(&item, &reference)
                .err()
                .map(|e| (e.kind.code().to_string(), e.path));
            match g.encode() {
                Err((c, p)) if c == code => {
                    if let Some(want) = case.get("encode_error_path").and_then(|p| p.as_str()) {
                        if p != want {
                            failures.push(format!("{id}: encode error path {p} != {want}"));
                        }
                    }
                    if reference_err == Some((c, p)) {
                        t.equal_to_reference += 1;
                    }
                }
                other => {
                    failures.push(format!("{id}: expected encode error {code}, got {other:?}"))
                }
            }
        }

        for read in case
            .get("reads")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default()
        {
            let text = read["json"].as_str().unwrap();
            load_initial(&mut g, &mut failures);
            let reference_result = decode(&item, &reference, text, 1 << 20);
            let got = g.decode(text);
            match (&got, read.get("error").and_then(|e| e.as_str())) {
                (Ok(()), None) => {
                    t.reads_ok += 1;
                    let mut want = before.clone();
                    for (p, v) in pairs(&read["changes"]) {
                        match want.iter_mut().find(|(wp, _)| *wp == p) {
                            Some(slot) => slot.1 = v,
                            None => failures
                                .push(format!("{id}: read {text}: change names unknown leaf {p}")),
                        }
                    }
                    let state = g.dump();
                    if state != want {
                        let diff: Vec<String> = state
                            .iter()
                            .zip(want.iter())
                            .filter(|(a, b)| a != b)
                            .map(|(a, b)| format!("{}: got {:?} want {:?}", a.0, a.1, b.1))
                            .collect();
                        failures.push(format!("{id}: read {text}\n   {}", diff.join("\n   ")));
                    } else if reference_result
                        .as_ref()
                        .is_ok_and(|n| trimmed(dump(&item, n)) == state)
                    {
                        t.equal_to_reference += 1;
                    }
                }
                (Err((c, p)), Some(code)) => {
                    t.reads_err += 1;
                    if c != code {
                        failures.push(format!(
                            "{id}: read {text}: expected {code}, got {c} at {p}"
                        ));
                    }
                    if let Some(want) = read.get("path").and_then(|p| p.as_str()) {
                        if p != want {
                            failures.push(format!("{id}: read {text}: error path {p} != {want}"));
                        }
                    }
                    if g.dump() != before {
                        failures.push(format!(
                            "{id}: read {text}: a refused read changed the group"
                        ));
                    }
                    if reference_result
                        .as_ref()
                        .err()
                        .is_some_and(|e| e.kind.code() == c && e.path == *p)
                    {
                        t.equal_to_reference += 1;
                    }
                }
                (Ok(()), Some(code)) => failures.push(format!(
                    "{id}: read {text}: expected error {code}, but it was accepted"
                )),
                (Err(e), None) => {
                    failures.push(format!("{id}: read {text}: unexpected error {e:?}"))
                }
            }
        }
    }
    let answers = t.encodes + t.reads_ok + t.reads_err;
    println!(
        "\n  group <-> JSON golden vectors (module routines): {} cases, {} encodings, {} round trips, {} reads accepted, {} reads refused, {} layouts refused; {} of {answers} answers equal to cobolt-wire's",
        t.cases, t.encodes, t.round_trips, t.reads_ok, t.reads_err, t.layout_errors, t.equal_to_reference
    );
    let _ = mem::rd_u8;
    assert!(
        failures.is_empty(),
        "{} vector failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(
        t.equal_to_reference, answers,
        "every answer equals the reference's"
    );
}

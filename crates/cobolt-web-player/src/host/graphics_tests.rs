// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The host half of Canvas2D in the player (WASM-018 T25, ABI A23): a `DRAW` hands the display list of the graphics
//! region to the page in order with the control's other commands, and a read-back is answered by the Canvas part
//! through the page's hook, or refused with the reason while the part has not loaded.

use super::*;
use cobolt_forms::description::{describe, to_json};
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::batch::{BatchWriter, PathSpec};
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, Limits, VariantInfo};
use std::cell::RefCell;

const FORM: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Canvas" width="640" height="480">
  <Control id="CNV-A" type="Canvas2D" x="10" y="10" w="200" h="100"></Control>
  <Control id="PLT-A" type="Plot" x="10" y="120" w="200" h="100"></Control>
</Form>"##;

fn manifest() -> Manifest {
    let limits = Limits { gfx_size: 4096, ..Limits::default() };
    Manifest {
        abi: 1,
        generator: "test".into(),
        project: "canvas".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits,
        names: vec!["FillRect".into(), "GetPixel".into(), "SetFillColor".into()],
        events: vec!["onLoad".into()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![ControlEntry { id: 3, name: "CNV-A".into(), kind: "Canvas2D".into() }, ControlEntry { id: 4, name: "PLT-A".into(), kind: "Plot".into() }],
            handlers: vec![],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    }
}

fn host() -> Host {
    let mut h = Host::new(manifest(), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(FORM).unwrap()))).unwrap();
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    h
}

/// The commands the page would run for the canvas, as (name, first argument).
fn commands(h: &mut Host) -> Vec<(String, String)> {
    h.trees.get_mut(&0).unwrap().take_changes().into_iter().filter_map(|c| match c {
        crate::tree::Change::Command { name, args, .. } => Some((name, args.first().cloned().unwrap_or_default())),
        _ => None,
    }).collect()
}

#[test]
fn a_draw_gives_the_page_the_list_of_the_graphics_region_in_order_with_the_calls() {
    let mut h = host();
    let _ = commands(&mut h);
    let list: Vec<u8> = (0..300u32).map(|i| (i % 251) as u8).collect();
    let mut w = BatchWriter::new();
    w.call(&PathSpec::call(0, 3, 2, &["#FF0000"]));
    w.draw(0, 3, 16, 200);
    w.call(&PathSpec::call(0, 3, 0, &["0", "0", "5", "5"]));
    h.set_gfx(list.clone());
    h.flush(w.as_bytes());
    let cmds = commands(&mut h);
    let names: Vec<&str> = cmds.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(names, ["canvas-call", "canvas-draw", "canvas-call"], "the list is drawn between the two calls");
    let seq: u64 = cmds[1].1.parse().unwrap();
    let got = h.trees.get_mut(&0).unwrap().take_gfx(seq).expect("the list waits for the page");
    assert_eq!(got, &list[16..216], "the bytes the DRAW names, no more");
    assert!(h.trees.get_mut(&0).unwrap().take_gfx(seq).is_none(), "taken once");
    assert_eq!(h.stats.draws, 1);
    println!("DRAW 200 bytes at 16 of a 300-byte region: canvas-call, canvas-draw, canvas-call; bytes equal; taken once");
}

#[test]
fn a_draw_that_names_bytes_the_flush_did_not_bring_is_reported_and_changes_nothing() {
    let mut h = host();
    let _ = commands(&mut h);
    let mut w = BatchWriter::new();
    w.draw(0, 3, 0, 64);
    h.flush(w.as_bytes());
    assert!(commands(&mut h).is_empty(), "no list, nothing for the page");
    assert_eq!(h.stats.draws, 0);
    // A list that is shorter than the DRAW says is refused the same way.
    h.set_gfx(vec![0; 10]);
    let mut w = BatchWriter::new();
    w.draw(0, 3, 0, 64);
    h.flush(w.as_bytes());
    assert!(commands(&mut h).is_empty());
    assert_eq!(h.stats.draws, 0);
}

#[test]
fn a_read_back_is_answered_by_the_canvas_part_and_refused_with_the_reason_before_it_has_loaded() {
    let mut h = host();
    let seen: Rc<RefCell<Vec<(String, Vec<String>)>>> = Rc::default();
    let log = seen.clone();
    let loaded = Rc::new(std::cell::Cell::new(false));
    let flag = loaded.clone();
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, _kind, _size, method, args| {
        log.borrow_mut().push((method.to_owned(), args.to_vec()));
        if flag.get() { GfxAnswer::Answer("4278190335".into()) } else { GfxAnswer::NotLoaded }
    }));
    let before = h.invoke(0, "CNV-A", "GetPixel", &["2", "2"]);
    let why = before.expect_err("the part has not loaded");
    assert!(why.contains("GetPixel") && why.contains("needs the graphics part"), "{why}");
    loaded.set(true);
    assert_eq!(h.invoke(0, "CNV-A", "GetPixel", &["2", "2"]), Ok(Some("4278190335".into())));
    assert_eq!(seen.borrow().last().unwrap(), &("GetPixel".to_owned(), vec!["2".to_owned(), "2".to_owned()]));
    // The three answers the control gives from its own properties never reach the part.
    let n = seen.borrow().len();
    assert_eq!(h.invoke(0, "CNV-A", "GetWidth", &[]), Ok(Some("200".into())));
    assert_eq!(seen.borrow().len(), n);
    println!("GetPixel before the part loads: refused with the reason; after: answered 4278190335; GetWidth never asks the part");
}

#[test]
fn every_method_of_the_other_graphics_objects_goes_to_the_part_and_a_refusal_is_the_controls_last_error() {
    let mut h = host();
    let seen: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let log = seen.clone();
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, kind, _size, method, _args| {
        log.borrow_mut().push((kind.to_owned(), method.to_owned()));
        if method == "AddSeries" { GfxAnswer::Answer("7".into()) } else { GfxAnswer::Refused("no such thing".into()) }
    }));
    assert_eq!(h.invoke(0, "PLT-A", "AddSeries", &["sales"]), Ok(Some("7".into())), "a handle comes back as text");
    assert_eq!(seen.borrow()[0], ("Plot".to_owned(), "AddSeries".to_owned()), "the part is told the kind");
    assert_eq!(h.invoke(0, "PLT-A", "Frobnicate", &[]), Ok(Some(String::new())), "a refusal answers nothing, as on the desktop");
    let last = h.trees[&0].get(2, "LastError");
    assert!(last.contains("Frobnicate refused") && last.contains("no such thing"), "{last}");
    assert_eq!(h.invoke(0, "PLT-A", "AddSeries", &["x"]), Ok(Some("7".into())));
    assert!(h.trees[&0].get(2, "LastError").is_empty(), "an answer clears it");
    let n = seen.borrow().len();
    h.invoke(0, "PLT-A", "Hide", &[]).expect("a universal verb");
    assert_eq!(seen.borrow().len(), n, "Hide never reaches the part");
    println!("Plot: AddSeries answered 7; a refusal wrote LastError and answered nothing; an answer cleared it; Hide stayed in the player");
}

#[test]
fn the_part_a_form_needs_comes_from_the_kinds_its_controls_use() {
    let h = host();
    assert_eq!(h.graphics_part(0), Some("graphics"), "a Canvas2D with a Plot needs the part that has plots");
    assert_eq!(h.graphics_part(9), None, "no such form");
}

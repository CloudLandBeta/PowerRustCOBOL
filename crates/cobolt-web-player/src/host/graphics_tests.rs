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
        names: vec!["FillRect".into(), "GetPixel".into(), "SetFillColor".into(), "AddSeries".into()],
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
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, _part, _kind, _size, method, args, _q| {
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
fn a_read_back_is_handed_the_drawing_queued_before_it_in_order() {
    let mut h = host();
    let seen: Rc<RefCell<Vec<Vec<String>>>> = Rc::default();
    let log = seen.clone();
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, _part, _kind, _size, _method, _args, q| {
        for (_c, item) in q {
            if let crate::tree::GfxItem::Call(a) = item {
                log.borrow_mut().push(a);
            }
        }
        GfxAnswer::Answer("0".into())
    }));
    h.invoke(0, "CNV-A", "SetFillColor", &["#FF0000"]).unwrap();
    h.invoke(0, "CNV-A", "FillRect", &["0", "0", "10", "10"]).unwrap();
    h.invoke(0, "CNV-A", "GetPixel", &["2", "2"]).unwrap();
    let first: Vec<String> = seen.borrow().iter().map(|a| a[0].clone()).collect();
    assert_eq!(first, ["SetFillColor", "FillRect"], "the read gets what was drawn before it, in order");
    h.invoke(0, "CNV-A", "GetPixel", &["3", "3"]).unwrap();
    assert_eq!(seen.borrow().len(), 2, "and nothing twice");
    println!("GetPixel after SetFillColor + FillRect: the part was handed [SetFillColor, FillRect] first, once");
}

#[test]
fn every_method_of_the_other_graphics_objects_goes_to_the_part_and_a_refusal_is_the_controls_last_error() {
    let mut h = host();
    let seen: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let log = seen.clone();
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, _part, kind, _size, method, _args, _q| {
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

/// WASM-018 T28 (R13, AC12 gamepad), the browser's side: the page reads the pads, `gamepad::diff` makes the events and the host
/// raises each on every canvas with a handler bound to it, with the argument text the desktop raises; a canvas with no handler
/// gets nothing, and the page polls the pads only while some canvas has one.
#[test]
fn a_gamepad_event_reaches_a_canvas_with_a_bound_handler_and_no_other() {
    use cobolt_forms::gamepad::{diff, Pad};
    use cobolt_wasm_abi::manifest::HandlerEntry;
    let mut m = manifest();
    m.events = vec!["onLoad".into(), "onGamepadButton".into(), "onGamepadAxis".into()];
    let started = std::time::Instant::now();

    // No handler bound: an event is queued nowhere. (The page looks at the pads whenever a canvas is open, for `GamepadAxis` and
    // `GamepadButton`, which are readable at any time.)
    let mut h = Host::new(m.clone(), HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(FORM).unwrap()))).unwrap();
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    let pads = |axes: &[f32], buttons: &[bool]| vec![Pad { index: 0, axes: axes.to_vec(), buttons: buttons.to_vec() }];
    let events = diff(&[], &pads(&[0.5], &[true, false]));
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(h.wants_gamepad(), "a canvas is open: the pads are read for the polled methods");
    assert_eq!(h.gamepad_events(&events), 0, "no handler bound, nothing raised");

    // The canvas CNV-A (control 3) binds both: it is polled, and each event is queued with its argument text.
    m.forms[0].handlers = vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }, HandlerEntry { ctrl: 3, event: 2, entry: 0 }];
    let mut h = Host::new(m, HostOptions::default());
    h.add_description(&to_json(&describe(&load_form_from_str(FORM).unwrap()))).unwrap();
    h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
    assert!(h.wants_gamepad(), "a canvas with a bound gamepad handler asks the page to look");
    let before = h.work.len();
    assert_eq!(h.gamepad_events(&events), 2);
    let queued: Vec<(u32, u32, Vec<String>)> = h
        .work
        .iter()
        .skip(before)
        .filter_map(|w| match w {
            Work::Event(e) => Some((e.ctrl, e.event, e.args.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(queued, [(3, 1, vec!["0 0 1".to_owned()]), (3, 2, vec!["0 0 0.500".to_owned()])], "the Plot PLT-A is not a canvas and gets none");
    println!("T28 browser: a pad with one button down and one axis at 0.5 queued onGamepadButton(0 0 1) and onGamepadAxis(0 0 0.500) on the bound canvas only ({} us)", started.elapsed().as_micros());
}

/// WASM-018 T30 (R14, AC12): a canvas with `TargetFps` and an `onFrame` handler is paced by the desktop's own clock: at the target
/// rate within a tolerance, in whole fixed steps when `FixedStep` is on, and not at all while the page is hidden; a canvas with no
/// handler (or no `TargetFps`) asks for no frames.
#[test]
fn on_frame_is_paced_by_the_desktops_clock_and_a_hidden_page_raises_none() {
    use cobolt_wasm_abi::manifest::HandlerEntry;
    let started = std::time::Instant::now();
    let form = |fps: &str, fixed: &str| FORM.replace(r#"h="100"></Control>
  <Control id="PLT-A"#, &format!(r#"h="100"><Property name="TargetFps">{fps}</Property><Property name="FixedStep">{fixed}</Property></Control>
  <Control id="PLT-A"#));
    let host = |fps: &str, fixed: &str, bound: bool| {
        let mut m = manifest();
        m.events = vec!["onLoad".into(), "onFrame".into()];
        if bound {
            m.forms[0].handlers = vec![HandlerEntry { ctrl: 3, event: 1, entry: 0 }];
        }
        let mut h = Host::new(m, HostOptions::default());
        h.add_description(&to_json(&describe(&load_form_from_str(&form(fps, fixed)).unwrap()))).unwrap();
        h.create_tree_as(0, 0, None, Vec::new(), OpenMode::Root).unwrap();
        h
    };
    let frames = |h: &Host, from: usize| -> Vec<String> {
        h.work.iter().skip(from).filter_map(|w| if let Work::Event(e) = w { e.args.first().cloned() } else { None }).collect()
    };

    // No handler: no frames asked for, none raised.
    let mut h = host("30", "0", false);
    assert!(!h.wants_frames());
    assert_eq!(h.frame_tick(0.0, true) + h.frame_tick(50.0, true), 0);

    // Variable rate, a 144 Hz display for 10 s: the target rate within a tolerance.
    let mut h = host("30", "0", true);
    assert!(h.wants_frames());
    let (mut t, mut raised) = (0.0, 0);
    while t < 10_000.0 {
        raised += h.frame_tick(t, true);
        t += 1000.0 / 144.0;
    }
    let rate = raised as f64 / 10.0;
    assert!((rate - 30.0).abs() <= 3.0, "{rate} frames a second");

    // Fixed step: every argument is exactly 1000 / fps.
    let mut h = host("60", "1", true);
    let before = h.work.len();
    let mut t = 0.0;
    for gap in [7.0, 31.0, 5.0, 60.0, 16.0, 16.0, 2.0, 40.0].iter().cycle().take(80) {
        h.frame_tick(t, true);
        t += gap;
    }
    let steps = frames(&h, before);
    assert!(!steps.is_empty() && steps.iter().all(|s| s == "16.667"), "{steps:?}");

    // Hidden: no frame, and showing the page again does not repay the hidden time.
    let mut h = host("60", "0", true);
    h.frame_tick(0.0, true);
    assert!(h.frame_tick(20.0, true) >= 1);
    let before = h.work.len();
    for i in 0..50 {
        assert_eq!(h.frame_tick(1000.0 + i as f64 * 16.0, false), 0, "a hidden page raises no frame");
    }
    assert_eq!(h.frame_tick(60_000.0, true), 0, "the first visible frame only sets the mark");
    assert_eq!(h.work.len(), before);
    println!(
        "T30 AC12 browser: onFrame at TargetFps 30 on a 144 Hz display, 10 s: {raised} frames = {rate:.2}/s; fixed step 60: {} steps all 16.667 ms; hidden 50 frames: 0 raised, no repayment ({} us)",
        steps.len(),
        started.elapsed().as_micros()
    );
}

/// WASM-018 T30: a call the page sends to a worker parks its batch; what the module sends meanwhile waits behind it, and when the
/// worker answers everything runs in the order it was sent, the parked query answered last.
#[test]
fn a_call_that_goes_to_a_worker_parks_its_batch_and_everything_after_it_runs_in_order_when_it_is_answered() {
    let mut h = host();
    let away = Rc::new(std::cell::Cell::new(0u32));
    let count = away.clone();
    h.gfx_sync = Some(Rc::new(move |_inst, _ctl, _part, _kind, _size, method, _args, _q| {
        if method == "AddSeries" {
            count.set(count.get() + 1);
            GfxAnswer::Deferred
        } else {
            GfxAnswer::Answer("5".into())
        }
    }));
    let plot = PathSpec::call(0, 4, 3, &["sales"]);
    let shown = |h: &Host| h.effects.iter().filter_map(|e| if let Effect::Display(t) = e { Some(t.clone()) } else { None }).collect::<Vec<_>>();

    // a flush: Display a, the call that goes away, Display b (which must wait)
    let mut w = BatchWriter::new();
    w.display("a").call(&plot).display("b");
    h.flush(w.as_bytes());
    assert_eq!(shown(&h), ["a"], "what follows the call waits for its answer");
    assert!(!h.query_waiting());
    // another flush while it is away: held
    let mut w = BatchWriter::new();
    w.display("c");
    h.flush(w.as_bytes());
    assert_eq!(shown(&h), ["a"]);
    // a query while it is away: parked too
    let mut w = BatchWriter::new();
    w.display("d").call_result(&PathSpec::call(0, 4, 1, &["2", "2"]));
    assert!(h.query(w.as_bytes()).is_err());
    assert!(h.query_waiting(), "the module is told to wait");
    assert_eq!(shown(&h), ["a"]);
    assert_eq!(away.get(), 1);

    h.finish_deferred(GfxAnswer::Answer("7".into()));
    assert_eq!(shown(&h), ["a", "b", "c", "d"], "the rest of the batch, then the batches that waited, in order");
    assert_eq!(h.take_final_answer(), Some(Ok(b"5".to_vec())), "the parked query is answered after all of them");
    assert!(!h.query_waiting());

    // a query that is itself the call that goes away: its answer is the worker's
    let mut w = BatchWriter::new();
    w.call_result(&plot);
    assert!(h.query(w.as_bytes()).is_err());
    assert!(h.query_waiting());
    h.finish_deferred(GfxAnswer::Answer("9".into()));
    assert_eq!(h.take_final_answer(), Some(Ok(b"9".to_vec())));
    // and a refusal is the control's LastError, answering nothing
    let mut w = BatchWriter::new();
    w.call_result(&plot);
    let _ = h.query(w.as_bytes());
    h.finish_deferred(GfxAnswer::Refused("too big".into()));
    assert_eq!(h.take_final_answer(), Some(Ok(Vec::new())));
    assert!(h.trees[&0].get(2, "LastError").contains("AddSeries refused: too big"));
    println!("worker call: batch parked after the call (a), two later batches held (c, d); answered -> a b c d in order, parked query answered 5; a query that is the away call answered 9; a refusal wrote LastError");
}

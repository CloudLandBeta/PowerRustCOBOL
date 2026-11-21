// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T49 (ABI A57): a form's animation sequences and layout scripts in the desktop host,
//! by the interpreter's rules, on a clock the test moves.

use super::*;
use crate::tree::Change;
use cobolt_forms::xml::load_form_from_str;
use cobolt_wasm_abi::manifest::{ControlEntry, FormEntry, Limits, VariantInfo};
use std::sync::atomic::{AtomicU64, Ordering};

static NOW: AtomicU64 = AtomicU64::new(0);

fn test_clock() -> u64 {
    NOW.load(Ordering::SeqCst)
}

/// A desktop host with two labels and a panel holding a third, on the test's clock.
fn host() -> Host {
    let form = load_form_from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="main-form" title="Motion" width="640" height="480" index-base="1">
  <Control id="Lbl-A" type="Label" x="8" y="8" w="120" h="24"></Control>
  <Control id="LBL-B" type="Label" x="8" y="40" w="120" h="24"></Control>
  <Control id="PNL-1" type="Panel" x="8" y="80" w="300" h="200"></Control>
  <Control id="LBL-IN" type="Label" x="8" y="8" w="120" h="24" parent="PNL-1"></Control>
</Form>"#,
    )
    .expect("the form loads");
    let manifest = Manifest {
        contract: None,
        remote: Vec::new(),
        capabilities: vec!["sequences".into(), "layouts".into()],
        abi: 1,
        generator: "test".into(),
        project: "motion".into(),
        variant: VariantInfo { multi_memory: true, conformance: false },
        limits: Limits::default(),
        names: Vec::new(),
        events: vec!["onClick".into()],
        forms: vec![FormEntry {
            id: 0,
            name: "main-form".into(),
            main: true,
            instances: 1,
            region_size: 64,
            controls: vec![
                ControlEntry { id: 1, name: "Lbl-A".into(), kind: "Label".into() },
                ControlEntry { id: 2, name: "LBL-B".into(), kind: "Label".into() },
                ControlEntry { id: 3, name: "PNL-1".into(), kind: "Panel".into() },
                ControlEntry { id: 4, name: "LBL-IN".into(), kind: "Label".into() },
            ],
            handlers: vec![],
            layers: vec![],
        }],
        builtins: vec![],
        imports: vec![],
        sizes: Default::default(),
    };
    let mut h = Host::new(manifest, HostOptions { desktop: true, ..HostOptions::default() });
    h.set_clock(test_clock);
    h.add_form(form).unwrap();
    h.create_tree(0, 0, None, Vec::new()).expect("the tree builds");
    h
}

fn seq(h: &mut Host, args: &[&str]) -> Result<Option<String>, String> {
    h.verb(0, ctrl::APP, "AnimationSequence", args).map_err(|e| match e {
        CmdError::Unknown(m) | CmdError::Refused(m) | CmdError::Bad(m) => m,
        CmdError::Deferred => "deferred".into(),
    })
}

fn layout(h: &mut Host, args: &[&str]) -> Result<Option<String>, String> {
    h.verb(0, ctrl::APP, "LAYOUT", args).map_err(|e| match e {
        CmdError::Unknown(m) | CmdError::Refused(m) | CmdError::Bad(m) => m,
        CmdError::Deferred => "deferred".into(),
    })
}

fn visible(h: &Host, name: &str) -> String {
    let t = &h.trees[&0];
    t.get(t.by_name(name).unwrap(), "Visible")
}

/// The animation commands since the last look, as `control:name(args)`.
fn commands(h: &mut Host) -> Vec<String> {
    let t = h.trees.get_mut(&0).unwrap();
    let names: Vec<String> = t.ctls.iter().map(|c| c.name.clone()).collect();
    t.take_changes()
        .into_iter()
        .filter_map(|c| match c {
            Change::Command { ctl, name, args } => Some(format!("{}:{name}({})", names[ctl], args.join(","))),
            _ => None,
        })
        .collect()
}

#[test]
fn a_sequence_fires_its_entries_on_the_clock_and_answers_its_state() {
    let t = std::time::Instant::now();
    let mut h = host();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    NOW.store(1000, Ordering::SeqCst);
    let script = "SHOW lbl-a AFTER 0; HIDE LBL-B AFTER 100\nPLAY LBL-A \"pulse\" AFTER 250";
    check(seq(&mut h, &["Create", "INTRO", script]).is_ok(), "Create reads the script (a name spelled in lower case)");
    check(h.app_get("BuiltinAnswer3") == script, "an action other than Status answers the third argument as it was sent");
    check(h.sequences_due_in().is_none() && !h.sequences_tick(), "a created sequence does not play");
    check(seq(&mut h, &["start", " intro "]).is_ok(), "Start, the action and the name in any case and with spaces");
    check(h.sequences_due_in() == Some(0), "the first entry is due at once");
    let _ = seq(&mut h, &["Status", "INTRO", "        "]);
    check(h.app_get("BuiltinAnswer3") == "Playing", "Status while entries remain: Playing");
    check(h.sequences_tick() && visible(&h, "Lbl-A") == "true", "SHOW writes Visible on");
    check(h.sequences_due_in() == Some(100), "the next entry is 100 ms away");
    NOW.store(1099, Ordering::SeqCst);
    check(!h.sequences_tick() && visible(&h, "LBL-B") == "true", "nothing fires before its time");
    NOW.store(1100, Ordering::SeqCst);
    check(h.sequences_tick() && visible(&h, "LBL-B") == "false", "HIDE writes Visible off at its time");
    let _ = commands(&mut h);
    NOW.store(1400, Ordering::SeqCst);
    check(h.sequences_tick() && commands(&mut h) == ["Lbl-A:playanimation(pulse)"], "PLAY starts the named animation");
    check(h.sequences_due_in().is_none(), "a sequence whose entries all fired stops");
    let _ = seq(&mut h, &["STATUS", "INTRO", "x"]);
    check(h.app_get("BuiltinAnswer3") == "Stopped", "Status after the last entry: Stopped");

    // A control inside a panel is the form's too, as the interpreter is told every control of the design.
    check(seq(&mut h, &["Create", "INNER", "HIDE lbl-in AFTER 0"]).is_ok() && seq(&mut h, &["Start", "INNER"]).is_ok(), "a nested control in a script");
    check(h.sequences_tick() && visible(&h, "LBL-IN") == "false", "HIDE reaches a control inside a panel");

    // Restart and Stop; Delete forgets it.
    let _ = seq(&mut h, &["Restart", "INTRO"]);
    check(h.sequences_due_in() == Some(0), "Restart runs it again from the beginning");
    let _ = seq(&mut h, &["Stop", "INTRO"]);
    check(h.sequences_due_in().is_none() && !h.sequences_tick(), "Stop drops the entries not fired");
    let _ = seq(&mut h, &["Delete", "INTRO"]);
    let refusals = [
        (seq(&mut h, &["Start", "INTRO"]), "COBOL::AnimationSequence: there is no sequence named \"INTRO\" (Create it first)"),
        (seq(&mut h, &["Spin", "X"]), "COBOL::AnimationSequence: unknown action \"SPIN\" (Create, Start, Stop, Restart, Status or Delete)"),
        (seq(&mut h, &["Create", "X"]), "COBOL::AnimationSequence: Create needs the script as its third argument"),
        (seq(&mut h, &["Create", "X", "SHOW LBL-A\nFLY LBL-B AFTER 1"]), "COBOL::AnimationSequence: script of \"X\", line 2"),
        (seq(&mut h, &["Create", "X", "SHOW LBL-IN\nSHOW LBL-ZZ"]), "COBOL::AnimationSequence: script of \"X\", line 2: this form has no control named \"LBL-ZZ\""),
        (seq(&mut h, &["Create", "  "]), "COBOL::AnimationSequence: the sequence name is empty"),
        (seq(&mut h, &["Status"]), "COBOL::AnimationSequence: takes an action and a sequence name"),
    ];
    for (got, want) in &refusals {
        check(got.as_ref().err().is_some_and(|m| m.starts_with(want)), &format!("refused as the interpreter refuses: {want} (got {got:?})"));
    }
    let _ = seq(&mut h, &["Create", "Y", "SHOW LBL-B"]);
    check(seq(&mut h, &["Status", "Y"]).err().as_deref() == Some("COBOL::AnimationSequence: Status answers into an item: COBOL::AnimationSequence(\"Status\", name, item)"), "Status with no item");

    // A sequence playing in an instance that closes goes with it.
    let _ = seq(&mut h, &["Start", "Y"]);
    h.destroy(0);
    check(h.sequences_due_in().is_none(), "a form that closes takes its sequences with it");
    println!(
        "── a_sequence_fires_its_entries_on_the_clock_and_answers_its_state (DESKTOP-WASM-001 T49, A57) ──\n  Create, Start, Status, SHOW, HIDE, PLAY on the clock, Restart, Stop, Delete, {} refusals, a closed form: {checks} of {checks} checks in {:.2} ms",
        refusals.len() + 1,
        t.elapsed().as_secs_f64() * 1000.0
    );
}

#[test]
fn a_layout_script_is_written_to_the_form_for_the_form_host() {
    let t = std::time::Instant::now();
    let mut h = host();
    let mut checks = 0;
    let mut check = |ok: bool, what: &str| {
        assert!(ok, "{what}");
        checks += 1;
    };
    let form = |h: &Host| h.trees[&0].get(0, "_LayoutScript");
    check(layout(&mut h, &["Apply", "pnl-1", "MODE FLEX ROW\nGAP 14\nWHEN Compact\n    MODE FLEX COLUMN"]).is_ok(), "Apply reads the script");
    let sent = form(&h);
    let (owner, lines) = sent.split_once('\n').unwrap_or_default();
    check(owner == "PNL-1", "the container as the form spells it");
    check(lines.contains("*\tPNL-1\tGap\t14") && lines.contains("Compact\tPNL-1\tFlexDirection\tColumn"), "one line per override, by breakpoint");
    check(layout(&mut h, &["Apply", "PNL-1", "ORDER lbl-in"]).is_ok(), "a control inside the panel may be named");
    check(layout(&mut h, &["Clear", "PNL-1"]).is_ok() && form(&h) == "PNL-1\n", "Clear writes the name alone");
    let refusals = [
        (layout(&mut h, &["Apply", "PNL-1"]), "COBOL::Layout: Apply needs the script as its third argument"),
        (layout(&mut h, &["Spin", "PNL-1"]), "COBOL::Layout: unknown action \"SPIN\" (Apply or Clear)"),
        (layout(&mut h, &["Apply", "NOPE", "GAP 4"]), "COBOL::Layout: this form has no control named \"NOPE\""),
        (layout(&mut h, &["Apply", "PNL-1", "ORDER LBL-IN LBL-ZZ"]), "COBOL::Layout: this form has no control named \"LBL-ZZ\""),
        (layout(&mut h, &["Apply", " "]), "COBOL::Layout: the container's name is empty"),
        (layout(&mut h, &["Clear"]), "COBOL::Layout: takes an action and the container's name"),
    ];
    for (got, want) in &refusals {
        check(got.as_ref().err().is_some_and(|m| m.starts_with(want)), &format!("refused as the interpreter refuses: {want} (got {got:?})"));
    }
    check(form(&h) == "PNL-1\n", "a refused call writes nothing");
    println!(
        "── a_layout_script_is_written_to_the_form_for_the_form_host (DESKTOP-WASM-001 T49, A57) ──\n  Apply, Clear, {} refusals: {checks} of {checks} checks in {:.2} ms",
        refusals.len(),
        t.elapsed().as_secs_f64() * 1000.0
    );
}

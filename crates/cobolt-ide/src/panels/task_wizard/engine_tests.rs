// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Every route through the engine (spec 104 tasks.md T10), on a task that exists only here —
//! which is also the proof that a task is a definition, not engine code (R37, AC26).

use eframe::egui::{pos2, vec2, Rect, ViewportId};

use super::anchors::{AnchorId as A, Mark, Marks};
use super::conditions as c;
use super::facts::{ControlKind, DesignerFacts, Facts, Selected};
use super::tasks::{Action, KeyPolicy, Mechanical, StepDef, TaskDef, TASKS};
use super::{Card, Effect, Engine, FrameInput, FrameOutput, Nav};
use crate::i18n::{Language, Tr};

fn tr() -> Tr {
    Language::English.tr()
}

const fn st(key: &'static str, ring: A) -> StepDef {
    StepDef {
        key,
        ring,
        live: None,
        via: &[],
        title: |t| t.task_wizard_next,
        body: |t| t.task_wizard_back,
        done: None,
        applies: c::always,
        mechanical: None,
        keys: KeyPolicy::LiveRegion,
        captures: false,
    }
}

static FIXTURE: &[TaskDef] = &[TaskDef {
    key: "fixture",
    title: |t| t.task_wizard_models_title,
    blurb: |t| t.task_wizard_models_blurb,
    result: |t| t.task_wizard_models_result,
    offer_for: None,
    steps: &[
        StepDef { done: Some(c::models_open), ..st("open", A::SettingsModelProviders) },
        StepDef {
            done: Some(c::endpoint_filled),
            live: Some(A::ModelsWindow),
            via: &[A::SettingsModelProviders],
            mechanical: Some(Mechanical {
                action: Action::ResetEndpoint,
                available: c::endpoint_empty,
                said: |t| t.task_wizard_did_endpoint,
            }),
            ..st("endpoint", A::ModelsEndpoint)
        },
        StepDef {
            done: Some(c::test_passed),
            applies: c::provider_needs_key,
            ..st("key", A::ModelsKeyAndTest)
        },
        st("read", A::ModelsSave),
    ],
}];

static DESIGNER_FIXTURE: &[TaskDef] = &[TaskDef {
    key: "grid",
    title: |t| t.task_wizard_grid_title,
    blurb: |t| t.task_wizard_grid_blurb,
    result: |t| t.task_wizard_grid_result,
    offer_for: Some(ControlKind::DataGrid),
    steps: &[
        StepDef {
            done: Some(c::selected_datagrid),
            captures: true,
            ..st("select", A::Canvas)
        },
        StepDef { applies: c::has_subject, ..st("settings", A::GridSettingsButton) },
    ],
}];

fn project() -> Facts {
    Facts {
        project: Some("/p/demo".into()),
        ..Default::default()
    }
}

/// Every anchor of `ids` painted on screen in the IDE window.
fn shown(ids: &[A]) -> Marks {
    let mut m = Marks::default();
    for (i, &id) in ids.iter().enumerate() {
        m.push(Mark {
            id,
            rect: Rect::from_min_size(pos2(10.0, 30.0 * i as f32 + 10.0), vec2(80.0, 20.0)),
            clip: Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0)),
            viewport: ViewportId::ROOT,
            layer: None,
        });
    }
    m
}

fn frame(e: &mut Engine, facts: &Facts, marks: &Marks, nav: Option<Nav>) -> FrameOutput {
    let tr = tr();
    e.frame(FrameInput {
        facts,
        marks,
        nav,
        tr: &tr,
    })
}

fn step_key(e: &Engine) -> Option<&'static str> {
    match &e.state {
        super::State::Running(r) => Some(e.table[r.task].steps[r.step].key),
        _ => None,
    }
}

fn card(out: &FrameOutput) -> super::StepCard {
    match &out.plan.as_ref().expect("a plan").card {
        Card::Step(s) => s.clone(),
        Card::Summary(_) => panic!("expected a step"),
    }
}

#[test]
fn starting_shows_the_first_step_and_remembers_it() {
    let mut e = Engine::new(FIXTURE);
    let f = project();
    let fx = e.start("fixture", None, &f, &tr());
    assert!(fx.contains(&Effect::Progress {
        project: "/p/demo".into(),
        task: "fixture",
        step: Some("open"),
    }));
    let out = frame(&mut e, &f, &shown(&[A::SettingsModelProviders]), None);
    let plan = out.plan.unwrap();
    assert_eq!(plan.ring, Some(A::SettingsModelProviders));
    assert!(!plan.dim_only);
}

#[test]
fn a_step_advances_only_on_the_rising_edge_of_its_condition() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    e.start("fixture", None, &f, &tr());
    let m = shown(&[A::SettingsModelProviders, A::ModelsEndpoint, A::ModelsWindow]);
    frame(&mut e, &f, &m, None);
    assert_eq!(step_key(&e), Some("open"));
    f.models_open = true;
    frame(&mut e, &f, &m, None);
    assert_eq!(step_key(&e), Some("endpoint"), "false → true while showing moves on (R17)");
}

#[test]
fn a_step_already_complete_says_so_and_never_bounces() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    f.models_open = true;
    e.start("fixture", None, &f, &tr());
    let m = shown(&[A::SettingsModelProviders, A::ModelsEndpoint, A::ModelsWindow]);
    for _ in 0..5 {
        let out = frame(&mut e, &f, &m, None);
        assert_eq!(step_key(&e), Some("open"), "a condition already true at entry never advances");
        assert!(card(&out).already);
    }
    // Next, then Back: the finished step is shown, and stays.
    frame(&mut e, &f, &m, Some(Nav::Next));
    assert_eq!(step_key(&e), Some("endpoint"));
    frame(&mut e, &f, &m, Some(Nav::Back));
    for _ in 0..5 {
        frame(&mut e, &f, &m, None);
        assert_eq!(step_key(&e), Some("open"), "Back to a finished step never bounces (R17)");
    }
}

#[test]
fn a_step_that_does_not_apply_is_skipped_and_not_counted() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    e.start("fixture", None, &f, &tr());
    let m = shown(&[A::SettingsModelProviders, A::ModelsEndpoint, A::ModelsWindow, A::ModelsSave]);
    let out = frame(&mut e, &f, &m, None);
    assert_eq!((card(&out).n, card(&out).m), (1, 3), "the key step does not apply: 3, not 4");
    f.models_provider_needs_key = true;
    let out = frame(&mut e, &f, &m, None);
    assert_eq!((card(&out).n, card(&out).m), (1, 4));
    f.models_provider_needs_key = false;
    frame(&mut e, &f, &m, Some(Nav::Next));
    frame(&mut e, &f, &m, Some(Nav::Next));
    assert_eq!(step_key(&e), Some("read"), "Next from endpoint skips the key step (R19)");
}

#[test]
fn leaving_unmet_is_skipped_and_the_summary_says_so() {
    let mut e = Engine::new(FIXTURE);
    let f = project();
    e.start("fixture", None, &f, &tr());
    let m = shown(&[A::SettingsModelProviders, A::ModelsEndpoint, A::ModelsWindow, A::ModelsSave]);
    frame(&mut e, &f, &m, Some(Nav::Next));
    frame(&mut e, &f, &m, Some(Nav::Next));
    let out = frame(&mut e, &f, &m, Some(Nav::Next));
    assert!(out.effects.contains(&Effect::Done("fixture")), "finishing marks it done (R29)");
    assert!(out.effects.contains(&Effect::Progress {
        project: "/p/demo".into(),
        task: "fixture",
        step: None,
    }));
    let Card::Summary(s) = &out.plan.as_ref().unwrap().card else {
        panic!("the summary shows the frame the task ends")
    };
    assert_eq!(s.skipped.len(), 2, "open and endpoint were left unmet (R18)");
    assert!(s.done.is_empty());
    let out = frame(&mut e, &f, &m, Some(Nav::Close));
    assert!(out.plan.is_none());
    assert!(!e.is_active());
}

#[test]
fn do_this_for_me_is_offered_only_while_empty_and_said_on_the_next_step() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    f.models_open = true;
    f.models_endpoint_empty = true;
    e.start("fixture", Some("endpoint"), &f, &tr());
    let m = shown(&[A::SettingsModelProviders, A::ModelsEndpoint, A::ModelsWindow, A::ModelsSave]);
    let out = frame(&mut e, &f, &m, None);
    assert!(card(&out).do_for_me);
    let out = frame(&mut e, &f, &m, Some(Nav::DoForMe));
    assert!(out.effects.contains(&Effect::Do(Action::ResetEndpoint)));
    // The IDE did it: the field filled, the step completes on the edge.
    f.models_endpoint_empty = false;
    let out = frame(&mut e, &f, &m, None);
    assert_eq!(step_key(&e), Some("read"));
    assert_eq!(card(&out).did, Some(tr().task_wizard_did_endpoint), "it says what it did (R25)");
    // With the field filled, the button is not offered (R26).
    e.start("fixture", Some("endpoint"), &f, &tr());
    let out = frame(&mut e, &f, &m, Some(Nav::DoForMe));
    assert!(!card(&out).do_for_me);
    assert!(!out.effects.iter().any(|x| matches!(x, Effect::Do(_))));
}

#[test]
fn a_closed_container_lights_its_via_then_falls_back() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    f.models_open = true;
    e.start("fixture", Some("endpoint"), &f, &tr());
    // The manager closed: the endpoint is gone, the button that opens it is on screen.
    let m = shown(&[A::SettingsModelProviders]);
    let mut last = None;
    for _ in 0..6 {
        last = Some(frame(&mut e, &f, &m, None));
    }
    let out = last.unwrap();
    let plan = out.plan.as_ref().unwrap();
    assert_eq!(plan.ring, Some(A::SettingsModelProviders), "the via is lit (R20)");
    assert!(card(&out).reopen);
    // Nothing of it on screen: back to the nearest earlier step that is.
    let m = shown(&[A::SettingsModelProviders]);
    let mut e = Engine::new(FIXTURE);
    e.start("fixture", Some("read"), &f, &tr());
    for _ in 0..6 {
        frame(&mut e, &f, &m, None);
    }
    assert_eq!(step_key(&e), Some("open"), "never a balloon pointing at nothing (R20)");
}

#[test]
fn nothing_anywhere_is_a_balloon_with_no_target_never_a_dead_dim() {
    let mut e = Engine::new(FIXTURE);
    let f = project();
    e.start("fixture", None, &f, &tr());
    let mut out = FrameOutput::default();
    for _ in 0..6 {
        out = frame(&mut e, &f, &Marks::default(), None);
    }
    let plan = out.plan.unwrap();
    assert_eq!(plan.ring, None);
    assert!(!plan.dim_only, "the balloon shows, so Back / Next / Exit are reachable");
}

#[test]
fn a_target_scrolled_out_is_revealed_within_a_budget() {
    let mut e = Engine::new(FIXTURE);
    let f = project();
    e.start("fixture", None, &f, &tr());
    let mut m = Marks::default();
    m.push(Mark {
        id: A::SettingsModelProviders,
        rect: Rect::from_min_size(pos2(10.0, 900.0), vec2(80.0, 20.0)),
        clip: Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0)),
        viewport: ViewportId::ROOT,
        layer: None,
    });
    let out = frame(&mut e, &f, &m, None);
    assert!(out.effects.contains(&Effect::Reveal(A::SettingsModelProviders)));
    assert!(out.plan.unwrap().dim_only);
    let mut reveals = 1;
    for _ in 0..20 {
        let out = frame(&mut e, &f, &m, None);
        reveals += out.effects.iter().filter(|x| matches!(x, Effect::Reveal(_))).count();
    }
    assert!(reveals <= 8, "a target that never scrolls in cannot wedge the step");
}

#[test]
fn exit_and_a_project_change_keep_the_place() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    e.start("fixture", Some("endpoint"), &f, &tr());
    let out = frame(&mut e, &f, &Marks::default(), Some(Nav::Exit));
    assert!(out.effects.contains(&Effect::Progress {
        project: "/p/demo".into(),
        task: "fixture",
        step: Some("endpoint"),
    }));
    assert!(!e.is_active());

    e.start("fixture", Some("endpoint"), &f, &tr());
    f.project = Some("/p/other".into());
    let out = frame(&mut e, &f, &Marks::default(), None);
    assert!(!e.is_active(), "closing or switching the project stops the task (R22)");
    assert!(out.effects.contains(&Effect::Progress {
        project: "/p/demo".into(),
        task: "fixture",
        step: Some("endpoint"),
    }), "…and keeps its place for the project it ran in");
}

#[test]
fn resume_is_by_key_and_an_unknown_key_starts_over() {
    let mut e = Engine::new(FIXTURE);
    let f = project();
    e.start("fixture", Some("read"), &f, &tr());
    assert_eq!(step_key(&e), Some("read"));
    e.start("fixture", Some("a-step-this-release-removed"), &f, &tr());
    assert_eq!(step_key(&e), Some("open"), "a key the table no longer has restarts (R29)");
}

#[test]
fn a_pause_draws_nothing_and_changes_nothing() {
    let mut e = Engine::new(FIXTURE);
    let mut f = project();
    e.start("fixture", None, &f, &tr());
    let m = shown(&[A::SettingsModelProviders]);
    f.paused = true;
    let out = frame(&mut e, &f, &m, Some(Nav::Next));
    assert!(out.plan.is_none(), "paused: nothing drawn (R24)");
    f.paused = false;
    frame(&mut e, &f, &m, None);
    assert!(e.is_running());
}

#[test]
fn the_subject_is_captured_on_leaving_and_recovered_on_resume() {
    let vp = ViewportId::from_hash_of("designer");
    let grid = Selected {
        form: "Main".into(),
        control: "Grid1".into(),
        kind: ControlKind::DataGrid,
    };
    let mut f = project();
    f.designers.push(DesignerFacts {
        viewport: vp,
        form: "Main".into(),
        focused: true,
        selected: Some(grid.clone()),
    });
    f.subject.exists = true;
    let mut e = Engine::new(DESIGNER_FIXTURE);
    e.start("grid", None, &f, &tr());
    frame(&mut e, &f, &Marks::default(), Some(Nav::Next));
    assert_eq!(e.subject(), Some(&grid));
    assert_eq!(step_key(&e), Some("settings"));

    // Resumed past the selection with the grid still selected: the subject comes back.
    let mut e = Engine::new(DESIGNER_FIXTURE);
    e.start("grid", Some("settings"), &f, &tr());
    assert_eq!(step_key(&e), Some("settings"));
    assert_eq!(e.subject(), Some(&grid));

    // …and with something else selected, the task resumes at the selection.
    f.designers[0].selected = None;
    let mut e = Engine::new(DESIGNER_FIXTURE);
    e.start("grid", Some("settings"), &f, &tr());
    assert_eq!(step_key(&e), Some("select"));
}

/// The operator's report (2026-10-17): start *Set up a DataGrid* (or the SideMenu or MenuBar
/// task) with the control already selected, and the selection step said "already done",
/// counted "step 1 of 1", offered **Finish**, and Finish ended the task. The app fills
/// `facts.subject` from `engine.subject()` at the START of each frame, so a subject taken only
/// when leaving the step was one frame too late for every step after it (`has_subject`).
/// Here the facts are built the app's way, from what the engine knew the frame before.
fn facts_as_the_app_builds_them(base: &Facts, e: &Engine) -> Facts {
    let mut f = base.clone();
    f.subject.exists = e.subject().is_some();
    f.subject.selected = e.subject().is_some();
    f
}

#[test]
fn a_control_already_selected_is_the_subject_from_the_first_frame() {
    let vp = ViewportId::from_hash_of("designer");
    let grid = Selected {
        form: "Main".into(),
        control: "Grid1".into(),
        kind: ControlKind::DataGrid,
    };
    let mut base = project();
    base.designers.push(DesignerFacts {
        viewport: vp,
        form: "Main".into(),
        focused: true,
        selected: Some(grid.clone()),
    });
    let mut e = Engine::new(DESIGNER_FIXTURE);
    let f = facts_as_the_app_builds_them(&base, &e);
    e.start("grid", None, &f, &tr());
    assert_eq!(e.subject(), Some(&grid), "taken as the step opens already done");
    let f = facts_as_the_app_builds_them(&base, &e);
    let out = frame(&mut e, &f, &Marks::default(), None);
    let Some(Card::Step(card)) = out.plan.map(|p| p.card) else { panic!("a step card") };
    assert!(card.already, "the grid was selected before the task began");
    assert_eq!((card.n, card.m), (1, 2), "the settings step counts: not '1 of 1'");
    assert!(!card.last, "Next, not Finish");
    let f = facts_as_the_app_builds_them(&base, &e);
    frame(&mut e, &f, &Marks::default(), Some(Nav::Next));
    assert_eq!(step_key(&e), Some("settings"), "Next goes on; it does not end the task");
}

/// The same, on the real table: each designer task started with its control selected shows
/// more than its selection step, and Next goes on.
#[test]
fn every_select_first_task_goes_on_past_an_already_selected_control() {
    let vp = ViewportId::from_hash_of("designer");
    for (task, kind) in [
        ("datagrid", ControlKind::DataGrid),
        ("sidemenu", ControlKind::SideMenu),
        ("menubar", ControlKind::MenuBar),
        ("treeview", ControlKind::TreeView),
        ("maps", ControlKind::Maps),
        ("animation", ControlKind::Other { bindable: false }),
        ("binding", ControlKind::Other { bindable: true }),
    ] {
        let selected = Selected { form: "Main".into(), control: "C1".into(), kind };
        let mut base = project();
        base.designers.push(DesignerFacts {
            viewport: vp,
            form: "Main".into(),
            focused: true,
            selected: Some(selected),
        });
        let mut e = Engine::default();
        let f = facts_as_the_app_builds_them(&base, &e);
        e.start(task, None, &f, &tr());
        let f = facts_as_the_app_builds_them(&base, &e);
        let out = frame(&mut e, &f, &Marks::default(), None);
        let Some(Card::Step(card)) = out.plan.map(|p| p.card) else { panic!("{task}: a step card") };
        assert!(card.m > card.n, "{task}: step {} of {} — the steps after the selection must count", card.n, card.m);
        assert!(!card.last, "{task}: Next, not Finish");
        let before = step_key(&e);
        let f = facts_as_the_app_builds_them(&base, &e);
        frame(&mut e, &f, &Marks::default(), Some(Nav::Next));
        assert!(e.is_running() && step_key(&e) != before, "{task}: Next went on to a later step, not to the summary");
        println!("{task}: step {} of {} on start; Next -> {:?}", card.n, card.m, step_key(&e));
    }
}

// ── The real table ──────────────────────────────────────────────────────────────────────

#[test]
fn every_task_and_step_key_is_unique() {
    let mut tasks = std::collections::HashSet::new();
    for t in TASKS {
        assert!(tasks.insert(t.key), "duplicate task key {}", t.key);
        assert!(!t.steps.is_empty());
        let mut steps = std::collections::HashSet::new();
        for s in t.steps {
            assert!(steps.insert(s.key), "{}: duplicate step key {}", t.key, s.key);
        }
    }
    assert_eq!(TASKS.len(), 9, "the eight tasks of §3, and the map (2026-10-17)");
}

/// AC19 — no *Do this for me* on a step whose action would call the network, save with no
/// undo, or write the developer's own content (R27).
#[test]
fn no_mechanical_step_touches_the_network_or_the_developers_content() {
    let forbidden = [
        A::SettingsModelProviders,
        A::ModelsProviderList,
        A::ModelsKeyAndTest,
        A::SettingsManageAgents,
        A::AgentsModelTab,
        A::AgentsScope,
        A::AgentsGrid,
        A::AgentsSave,
        A::TreeItems,
    ];
    for t in TASKS {
        for s in t.steps {
            if s.mechanical.is_some() {
                assert!(
                    !forbidden.contains(&s.ring),
                    "{}/{}: Do this for me on {:?}",
                    t.key,
                    s.key,
                    s.ring
                );
            }
        }
    }
}

#[test]
fn every_task_runs_to_its_summary_by_next_alone() {
    let f = project();
    for t in TASKS {
        let mut e = Engine::default();
        e.start(t.key, None, &f, &tr());
        let mut finished = false;
        for _ in 0..40 {
            let out = frame(&mut e, &f, &Marks::default(), Some(Nav::Next));
            if out.effects.contains(&Effect::Done(t.key)) {
                finished = true;
                break;
            }
        }
        assert!(finished, "{}: Next must always lead to the end (R18)", t.key);
    }
}

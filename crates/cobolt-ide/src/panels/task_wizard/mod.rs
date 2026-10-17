// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Task Wizard — spec 104.
//!
//! A wizard of the Walkthrough's kind (spec 059): each step dims the IDE, lights one live
//! region, rings the widget to use and points a speech balloon at it. Unlike the tour, the
//! lit region is **live**: the developer does the step there, and the wizard moves on when
//! the IDE's own state says it is done.
//!
//! # Shape
//!
//! - [`tasks`] — the tasks, as data.
//! - [`conditions`] — what the table's conditions are written in.
//! - [`facts`] — the read-only snapshot of the IDE the engine reads (key-free, R28).
//! - [`anchors`] — where the frame painted each target.
//! - [`Engine`] (here) — the pure state machine: given the facts and the marks, which step
//!   shows, what to draw and what the IDE must do. It never touches `CoboltApp` or a file
//!   (R36), which is what lets every route through it be tested.
//! - [`overlay`], [`input`] — the egui side: the dim with its holes, the balloon, and the
//!   keyboard policy, per OS window.
//! - [`catalogue`], [`offers`] — the popup under the toggle and the chips beside a selection.

pub mod anchors;
pub mod catalogue;
pub mod conditions;
#[cfg(test)]
mod engine_tests;
pub mod facts;
pub mod input;
pub mod offers;
pub mod overlay;
#[cfg(test)]
mod overlay_tests;
pub mod tasks;

use eframe::egui::ViewportId;

pub use anchors::{AnchorId, Marks};
pub use facts::{Facts, Selected};
pub use tasks::{Action, KeyPolicy, TaskDef, TASKS};

use tasks::Ctx;

use crate::i18n::Tr;

/// Everything the IDE keeps for the wizard: the switch, what is remembered (R5, R29, R30),
/// the engine and the overlay, and the few counters the facts need.
#[derive(Debug, Default)]
pub struct Wizard {
    /// The switch (R5), as read at start-up and changed by either toggle.
    pub on: bool,
    /// Tasks finished on this machine (R29).
    pub done: std::collections::BTreeSet<String>,
    /// Project key → task key → step key (R29).
    pub progress: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// Dismissed offers, `project|form|control|task` (R30).
    pub dismissed: std::collections::BTreeSet<String>,
    pub engine: Engine,
    pub overlay: overlay::Overlay,
    /// What the balloon or `Esc` asked for, taken by the next engine frame.
    pub pending: Option<Nav>,
    /// The anchor the engine asked to have revealed, handed to next frame's sink.
    pub reveal: Option<AnchorId>,
    /// Saves of the Model Providers Manager this session (a fact for task 1).
    pub models_saves: u32,
    /// Animation previews started, per control (a fact for task 8).
    pub anim_previews: std::collections::HashMap<String, u32>,
    /// The Form Designer window that last had the OS focus.
    pub focused_designer: Option<ViewportId>,
}

impl Wizard {
    /// Read what `ui.toml` remembers (R5).
    pub fn load() -> Self {
        let p = crate::ui_prefs::UiPrefs::load();
        Wizard {
            on: p.task_wizard_on,
            done: p.task_wizard_done,
            progress: p.task_wizard_progress,
            dismissed: p.task_wizard_dismissed,
            ..Default::default()
        }
    }

    /// Write what is remembered (load-then-save, so the other preferences survive).
    pub fn save(&self) {
        let mut p = crate::ui_prefs::UiPrefs::load();
        p.task_wizard_on = self.on;
        p.task_wizard_done = self.done.clone();
        p.task_wizard_progress = self.progress.clone();
        p.task_wizard_dismissed = self.dismissed.clone();
        p.save();
    }

    /// Where `task` stands in `project`, if it is in progress there.
    pub fn progress_of(&self, project: &str, task: &str) -> Option<&str> {
        self.progress.get(project)?.get(task).map(String::as_str)
    }

    /// Apply one engine effect to what is remembered. Returns whether it changed anything
    /// that must be saved.
    pub fn remember(&mut self, effect: &Effect) -> bool {
        match effect {
            Effect::Progress { project, task, step } => {
                let before = self.progress_of(project, task).map(str::to_owned);
                match step {
                    Some(s) => {
                        self.progress
                            .entry(project.clone())
                            .or_default()
                            .insert((*task).to_owned(), (*s).to_owned());
                    }
                    None => {
                        if let Some(m) = self.progress.get_mut(project) {
                            m.remove(*task);
                            if m.is_empty() {
                                self.progress.remove(project);
                            }
                        }
                    }
                }
                before.as_deref() != step.as_deref()
            }
            Effect::Done(task) => self.done.insert((*task).to_owned()),
            Effect::Reveal(_) | Effect::Do(_) => false,
        }
    }
}

/// What the developer pressed in the balloon, or the keyboard said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Next,
    Back,
    /// Leave the task where it is (R22).
    Exit,
    DoForMe,
    /// Close the closing summary.
    Close,
}

/// What the IDE must do after a frame of the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Remember that `task` stands at `step` in `project`; `None` forgets it (R29).
    Progress {
        project: String,
        task: &'static str,
        step: Option<&'static str>,
    },
    /// `task` was finished on this machine (R29).
    Done(&'static str),
    /// Bring this anchor into view next frame (R20).
    Reveal(AnchorId),
    /// *Do this for me* (R25).
    Do(Action),
}

/// What one step's balloon says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepCard {
    pub task: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    /// "step *n* of *m*", counting only the steps that apply (R19).
    pub n: usize,
    pub m: usize,
    /// The condition already held when the step began: say so, make Next primary (R17).
    pub already: bool,
    /// What *Do this for me* did on the step before, to say it (R25).
    pub did: Option<&'static str>,
    pub back: bool,
    pub do_for_me: bool,
    /// The step's own target is closed: the ring is on its *via*, "Reopen…" (R20).
    pub reopen: bool,
    /// The last step: Next reads *Finish*.
    pub last: bool,
}

/// What the closing summary says (R21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryCard {
    pub task: &'static str,
    pub done: Vec<&'static str>,
    pub skipped: Vec<&'static str>,
    pub for_you: Vec<&'static str>,
    pub result: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Card {
    Step(StepCard),
    Summary(SummaryCard),
}

/// What to draw this frame. Symbolic: anchors, not rects — each window's overlay resolves
/// them against the rects it painted itself, so a Form Designer, which paints before the
/// engine runs, never draws last frame's geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// Changes whenever the step changes; the overlay keys its own per-step memory on it.
    pub serial: u64,
    /// The window the step is in.
    pub viewport: ViewportId,
    /// What to ring. `None`: a balloon with no target (the summary, or nothing on screen).
    pub ring: Option<AnchorId>,
    /// What stays live.
    pub live: Option<AnchorId>,
    /// Waiting for the target to scroll in: dim only, no balloon.
    pub dim_only: bool,
    pub keys: KeyPolicy,
    pub card: Card,
}

/// One frame's input.
pub struct FrameInput<'a> {
    pub facts: &'a Facts,
    pub marks: &'a Marks,
    pub nav: Option<Nav>,
    pub tr: &'a Tr,
}

#[derive(Debug, Default)]
pub struct FrameOutput {
    pub plan: Option<Plan>,
    pub effects: Vec<Effect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Completed,
    Skipped,
    ForYou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Forward,
    Back,
}

#[derive(Debug, Clone)]
struct Run {
    task: usize,
    step: usize,
    dir: Dir,
    project: String,
    /// The IDE when the step began.
    entry: Facts,
    /// Last frame's value of the step's condition — completion is its rising edge (R17).
    prev: bool,
    already: bool,
    subject: Option<Selected>,
    outcomes: Vec<Option<Outcome>>,
    did: Option<&'static str>,
    /// *Do this for me* was pressed on this step.
    did_here: bool,
    reveal_budget: u8,
    missing: u8,
}

#[derive(Debug, Clone)]
enum State {
    Idle,
    Running(Box<Run>),
    Summary { viewport: ViewportId, card: SummaryCard },
}

/// Frames a step may spend asking for its target to be scrolled into view.
const REVEAL_BUDGET: u8 = 8;
/// Frames a target may be missing before the wizard looks elsewhere: a window that opens
/// on a click is painted the frame after.
const MISSING_GRACE: u8 = 3;

/// The wizard's state machine.
pub struct Engine {
    table: &'static [TaskDef],
    state: State,
    serial: u64,
    last_plan: Option<Plan>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("state", &self.state)
            .field("serial", &self.serial)
            .field("last_plan", &self.last_plan)
            .finish_non_exhaustive()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(TASKS)
    }
}

impl Engine {
    pub fn new(table: &'static [TaskDef]) -> Self {
        Engine {
            table,
            state: State::Idle,
            serial: 0,
            last_plan: None,
        }
    }

    pub fn table(&self) -> &'static [TaskDef] {
        self.table
    }

    /// A task or its summary is on screen.
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    /// A task is running (not its summary).
    #[cfg(test)]
    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// The control the running task is about, once captured — the IDE reads its facts.
    pub fn subject(&self) -> Option<&Selected> {
        match &self.state {
            State::Running(r) => r.subject.as_ref(),
            _ => None,
        }
    }

    /// Last frame's plan, for a Form Designer's overlay, which paints before the engine runs.
    pub fn plan(&self) -> Option<&Plan> {
        self.last_plan.as_ref()
    }

    /// Start task `key` in the open project, at step `resume` when given and still in the
    /// table, else at its first step (R9, R29). Returns the effects of starting.
    pub fn start(&mut self, key: &str, resume: Option<&str>, facts: &Facts, tr: &Tr) -> Vec<Effect> {
        let mut effects = self.stop();
        let Some(task) = self.table.iter().position(|t| t.key == key) else {
            return effects;
        };
        let Some(project) = facts.project.clone() else {
            return effects;
        };
        let def = &self.table[task];
        let mut subject = None;
        let mut at = resume.and_then(|k| def.step_index(k)).unwrap_or(0);
        // A designer task resumed past its selection step needs its subject again: take it
        // from the selection if that is still the right kind of control, else resume at the
        // selection step itself.
        if let Some(cap) = (0..at).rev().find(|&i| def.steps[i].captures) {
            let ok = def.steps[cap].done.is_some_and(|d| {
                d(&Ctx {
                    now: facts,
                    entry: facts,
                    subject: None,
                })
            });
            if ok {
                subject = facts.selected().cloned();
            } else {
                at = cap;
            }
        }
        let mut run = Box::new(Run {
            task,
            step: at,
            dir: Dir::Forward,
            project,
            entry: facts.clone(),
            prev: false,
            already: false,
            subject,
            outcomes: vec![None; def.steps.len()],
            did: None,
            did_here: false,
            reveal_budget: REVEAL_BUDGET,
            missing: 0,
        });
        // The first step that applies, from `at` on.
        match next_applicable(def, &run, facts, at) {
            Some(i) => {
                enter(&mut run, def, i, Dir::Forward, facts);
                effects.push(progress(&run, def));
                self.state = State::Running(run);
                self.serial += 1;
            }
            None => {
                // Nothing applies (everything is already in place): straight to the summary.
                self.serial += 1;
                effects.extend(self.finish(run, ViewportId::ROOT, tr));
            }
        }
        effects
    }

    /// Stop whatever runs, keeping a task's progress (R4, R22).
    pub fn stop(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let State::Running(run) = &self.state {
            effects.push(progress(run, &self.table[run.task]));
        }
        self.state = State::Idle;
        self.last_plan = None;
        effects
    }

    /// One frame.
    pub fn frame(&mut self, input: FrameInput) -> FrameOutput {
        let mut out = FrameOutput::default();
        let facts = input.facts;
        let tr = input.tr;

        // The summary: one balloon, closed by Close (or Exit / Esc).
        if let State::Summary { viewport, card } = &self.state {
            if matches!(input.nav, Some(Nav::Close | Nav::Exit | Nav::Next)) {
                self.state = State::Idle;
                self.last_plan = None;
                return out;
            }
            if facts.paused {
                self.last_plan = None;
                return out;
            }
            let plan = Plan {
                serial: self.serial,
                viewport: *viewport,
                ring: None,
                live: None,
                dim_only: false,
                keys: KeyPolicy::LiveRegion,
                card: Card::Summary(card.clone()),
            };
            self.last_plan = Some(plan.clone());
            out.plan = Some(plan);
            return out;
        }

        let table: &'static [TaskDef] = self.table;
        let State::Running(run) = &mut self.state else {
            self.last_plan = None;
            return out;
        };
        let def = &table[run.task];

        // The project went away or changed: leave the task where it was, for its project.
        if facts.project.as_deref() != Some(run.project.as_str()) {
            out.effects = self.stop();
            return out;
        }
        // Something else owns the screen (R24): draw nothing, take nothing, change nothing.
        if facts.paused {
            self.last_plan = None;
            return out;
        }

        // ── Navigation the developer asked for ──────────────────────────────────────────
        let mut nav = input.nav;
        let step = def.steps[run.step];
        if nav == Some(Nav::DoForMe) {
            nav = None;
            if let Some(m) = step.mechanical {
                if (m.available)(&ctx(run, facts)) {
                    out.effects.push(Effect::Do(m.action));
                    run.did_here = true;
                }
            }
        }
        // The condition, read now; completion is its rising edge while the step shows.
        let now = step.done.is_some_and(|d| d(&ctx(run, facts)));
        let rose = now && !run.prev;
        run.prev = now;
        if rose && nav.is_none() {
            nav = Some(Nav::Next);
        }

        match nav {
            Some(Nav::Exit) | Some(Nav::Close) => {
                out.effects.extend(self.stop());
                return out;
            }
            Some(Nav::Back) => {
                if let Some(i) = prev_applicable(def, run, facts) {
                    enter(run, def, i, Dir::Back, facts);
                    self.serial += 1;
                    out.effects.push(progress(run, def));
                }
            }
            Some(Nav::Next) => {
                leave_forward(run, def, facts, now);
                match next_applicable(def, run, facts, run.step + 1) {
                    Some(i) => {
                        let did = if run.did_here {
                            step.mechanical.map(|m| (m.said)(tr))
                        } else {
                            None
                        };
                        enter(run, def, i, Dir::Forward, facts);
                        run.did = did;
                        self.serial += 1;
                        out.effects.push(progress(run, def));
                    }
                    None => {
                        let State::Running(run) = std::mem::replace(&mut self.state, State::Idle)
                        else {
                            unreachable!()
                        };
                        let vp = self.last_plan.as_ref().map_or(ViewportId::ROOT, |p| p.viewport);
                        self.serial += 1;
                        out.effects.extend(self.finish(run, vp, tr));
                        // Draw the summary this same frame.
                        let rest = self.frame(FrameInput {
                            facts,
                            marks: input.marks,
                            nav: None,
                            tr,
                        });
                        out.plan = rest.plan;
                        return out;
                    }
                }
            }
            Some(Nav::DoForMe) | None => {}
        }

        // ── Where is the target? (R20) ──────────────────────────────────────────────────
        let State::Running(run) = &mut self.state else {
            unreachable!()
        };
        let def = &table[run.task];
        let step = def.steps[run.step];
        let prefer = preferred_viewport(run, facts, step.ring);
        let mut ring = step.ring;
        let mut reopen = false;
        let mut dim_only = false;
        let mut viewport = prefer.unwrap_or(ViewportId::ROOT);
        let mut no_target = false;

        match input.marks.find(step.ring, prefer) {
            Some(m) if m.on_screen() => {
                run.missing = 0;
                viewport = m.viewport;
            }
            Some(m) => {
                run.missing = 0;
                viewport = m.viewport;
                if run.reveal_budget > 0 {
                    run.reveal_budget -= 1;
                    out.effects.push(Effect::Reveal(step.ring));
                    dim_only = true;
                }
                // Out of budget: point at what is visible of it.
            }
            None => {
                run.missing = run.missing.saturating_add(1);
                if run.missing <= MISSING_GRACE {
                    dim_only = true;
                } else if let Some(m) = step.via.iter().find_map(|&v| {
                    input
                        .marks
                        .find(v, preferred_viewport(run, facts, v))
                        .filter(|m| m.on_screen())
                }) {
                    ring = m.id;
                    reopen = true;
                    viewport = m.viewport;
                } else if let Some(i) = (0..run.step).rev().find(|&i| {
                    (def.steps[i].applies)(&ctx(run, facts))
                        && input
                            .marks
                            .find(def.steps[i].ring, preferred_viewport(run, facts, def.steps[i].ring))
                            .is_some_and(|m| m.on_screen())
                }) {
                    // Back to the nearest earlier step that can be shown.
                    enter(run, def, i, Dir::Back, facts);
                    self.serial += 1;
                    out.effects.push(progress(run, def));
                    // Draw it next frame, from the marks of the frame that showed it.
                    dim_only = true;
                } else {
                    // Nothing to point at anywhere: a balloon of its own, so the developer
                    // can still go Back, Next or Exit — never stuck behind a dim.
                    no_target = true;
                    viewport = ViewportId::ROOT;
                }
            }
        }

        let State::Running(run) = &self.state else {
            unreachable!()
        };
        let def = &table[run.task];
        let step = def.steps[run.step];
        let x = ctx(run, facts);
        let (n, m) = count(def, run, facts);
        let card = StepCard {
            task: (def.title)(tr),
            title: (step.title)(tr),
            body: if reopen { tr.task_wizard_reopen } else { (step.body)(tr) },
            n,
            m,
            already: run.already,
            did: run.did,
            back: prev_applicable(def, run, facts).is_some(),
            do_for_me: !reopen
                && step
                    .mechanical
                    .is_some_and(|mc| (mc.available)(&x)),
            reopen,
            last: next_applicable(def, run, facts, run.step + 1).is_none(),
        };
        let plan = Plan {
            serial: self.serial,
            viewport,
            ring: if no_target { None } else { Some(ring) },
            live: if no_target {
                None
            } else if reopen {
                Some(ring)
            } else {
                Some(step.live.unwrap_or(step.ring))
            },
            dim_only,
            keys: step.keys,
            card: Card::Step(card),
        };
        self.last_plan = Some(plan.clone());
        out.plan = Some(plan);
        out
    }

    /// End a run: the summary, *done*, and the progress forgotten (R21, R29).
    fn finish(&mut self, run: Box<Run>, viewport: ViewportId, tr: &Tr) -> Vec<Effect> {
        let def = &self.table[run.task];
        let mut card = SummaryCard {
            task: "",
            done: Vec::new(),
            skipped: Vec::new(),
            for_you: Vec::new(),
            result: "",
        };
        card.task = (def.title)(tr);
        card.result = (def.result)(tr);
        for (i, o) in run.outcomes.iter().enumerate() {
            let title = (def.steps[i].title)(tr);
            match o {
                Some(Outcome::Completed) => card.done.push(title),
                Some(Outcome::ForYou) => card.for_you.push(title),
                Some(Outcome::Skipped) => card.skipped.push(title),
                None => {}
            }
        }
        self.state = State::Summary { viewport, card };
        vec![
            Effect::Done(def.key),
            Effect::Progress {
                project: run.project.clone(),
                task: def.key,
                step: None,
            },
        ]
    }
}

fn ctx<'a>(run: &'a Run, facts: &'a Facts) -> Ctx<'a> {
    Ctx {
        now: facts,
        entry: &run.entry,
        subject: run.subject.as_ref(),
    }
}

fn progress(run: &Run, def: &TaskDef) -> Effect {
    Effect::Progress {
        project: run.project.clone(),
        task: def.key,
        step: Some(def.steps[run.step].key),
    }
}

/// Begin step `i`: its facts at entry, and whether it is already complete (R17).
fn enter(run: &mut Run, def: &TaskDef, i: usize, dir: Dir, facts: &Facts) {
    run.step = i;
    run.dir = dir;
    run.entry = facts.clone();
    run.did = None;
    run.did_here = false;
    run.reveal_budget = REVEAL_BUDGET;
    run.missing = 0;
    let held = def.steps[i].done.is_some_and(|d| d(&ctx(run, facts)));
    run.prev = held;
    run.already = held;
}

/// Record how the step was left going forward, and take the subject if it captures one.
fn leave_forward(run: &mut Run, def: &TaskDef, facts: &Facts, now: bool) {
    let step = def.steps[run.step];
    run.outcomes[run.step] = match step.done {
        None => None,
        Some(_) if run.did_here && now => Some(Outcome::ForYou),
        Some(_) if now => Some(Outcome::Completed),
        Some(_) => Some(Outcome::Skipped),
    };
    if step.captures && now {
        run.subject = facts.selected().cloned();
    }
}

fn next_applicable(def: &TaskDef, run: &Run, facts: &Facts, from: usize) -> Option<usize> {
    (from..def.steps.len()).find(|&i| (def.steps[i].applies)(&ctx(run, facts)))
}

fn prev_applicable(def: &TaskDef, run: &Run, facts: &Facts) -> Option<usize> {
    (0..run.step)
        .rev()
        .find(|&i| (def.steps[i].applies)(&ctx(run, facts)))
}

/// "*n* of *m*": the steps that apply, as far as the IDE can tell now (R19).
fn count(def: &TaskDef, run: &Run, facts: &Facts) -> (usize, usize) {
    let x = ctx(run, facts);
    let before = (0..run.step)
        .filter(|&i| run.outcomes[i].is_some() || (def.steps[i].applies)(&x))
        .count();
    let after = (run.step + 1..def.steps.len())
        .filter(|&i| (def.steps[i].applies)(&x))
        .count();
    (before + 1, before + 1 + after)
}

/// The window a step's target should be looked for in: the subject's designer; else, for a
/// designer anchor, the designer the developer is working in; else the IDE window.
fn preferred_viewport(run: &Run, facts: &Facts, anchor: AnchorId) -> Option<ViewportId> {
    if anchor.in_designer() {
        if let Some(s) = &run.subject {
            if let Some(d) = facts.designer_for(&s.form) {
                return Some(d.viewport);
            }
        }
        return facts.active_designer().map(|d| d.viewport);
    }
    if anchor == AnchorId::Toggle {
        return None;
    }
    Some(ViewportId::ROOT)
}

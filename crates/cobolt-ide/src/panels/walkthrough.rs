// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The IDE Walkthrough — spec 059.
//!
//! A short guided tour of the IDE's six main components. Each step dims the
//! whole window, leaves one component lit, and points a comic speech balloon at
//! it. It runs once per machine, the first time a project is opened, and a
//! checkable item in the Help menu replays it.
//!
//! # What this module deliberately does not know
//!
//! It never mentions `CoboltApp` or `ProjectPanel`. It is handed an [`Anchors`]
//! — the rects the frame actually painted — and returns an [`Outcome`] saying
//! whether the tour ended and whether it needs a component revealed. That is
//! what makes it testable: `CoboltApp` cannot be constructed in a test (it needs
//! an `eframe::CreationContext`), so a tour that reached into it could only ever
//! be tested by driving the application, which this project does not do.
//!
//! The logic worth testing is therefore in free functions —
//! [`should_auto_start`], [`on_menu_toggle`], [`scrim_bands`], [`place_balloon`],
//! [`balloon_palette`] — none of which need an `egui::Ui` at all.
//!
//! # Why the dim is four rectangles
//!
//! `egui::Modal` paints its backdrop as a single `rect_filled` over the whole
//! content rect and offers no way to cut a hole in it. A spotlight needs the
//! hole, so the dim is painted as the four bands *around* the target instead,
//! and the component simply keeps its own pixels.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, Vec2};

use crate::i18n::Tr;
use crate::project_model::Category;
use crate::theme::Theme;

/// The six components, in the order the operator set (spec 059 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    ProjectSettings,
    Forms,
    IndexedFiles,
    Assets,
    KnowledgeBase,
    Output,
}

impl Step {
    /// The tour, in order. One list, so nothing can disagree about the sequence.
    pub const ORDER: [Step; 6] = [
        Step::ProjectSettings,
        Step::Forms,
        Step::IndexedFiles,
        Step::Assets,
        Step::KnowledgeBase,
        Step::Output,
    ];

    /// What this step points at.
    ///
    /// This is the one place that encodes **Knowledge Base =
    /// `Category::Documentation`** — the tree's own equivalence
    /// (`panels/project.rs`, `is_knowledge_base`), which is not guessable from
    /// the variant name.
    pub fn anchor(self) -> Anchor {
        match self {
            Step::ProjectSettings => Anchor::ProjectRoot,
            Step::Forms => Anchor::Category(Category::Forms),
            Step::IndexedFiles => Anchor::Category(Category::IndexedFiles),
            Step::Assets => Anchor::Category(Category::Assets),
            Step::KnowledgeBase => Anchor::Category(Category::Documentation),
            Step::Output => Anchor::OutputPanel,
        }
    }

    /// The component's name.
    ///
    /// Steps 2-6 borrow the tree's own labels, so a balloon calls each component
    /// exactly what the developer can see it called two inches to the left.
    pub fn title(self, tr: &Tr) -> &'static str {
        match self {
            Step::ProjectSettings => tr.walkthrough_title_settings,
            Step::Forms => tr.panel_forms,
            Step::IndexedFiles => tr.cat_indexed_files,
            Step::Assets => tr.panel_assets,
            Step::KnowledgeBase => tr.cat_documentation,
            Step::Output => tr.panel_output,
        }
    }

    /// What the balloon says.
    pub fn body(self, tr: &Tr) -> &'static str {
        match self {
            Step::ProjectSettings => tr.walkthrough_step_settings,
            Step::Forms => tr.walkthrough_step_forms,
            Step::IndexedFiles => tr.walkthrough_step_indexed,
            Step::Assets => tr.walkthrough_step_assets,
            Step::KnowledgeBase => tr.walkthrough_step_knowledge,
            Step::Output => tr.walkthrough_step_output,
        }
    }
}

/// A thing a step can point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The project tree's root node — the project-settings node.
    ProjectRoot,
    /// One top-level category header in the project tree.
    Category(Category),
    /// The output pane.
    OutputPanel,
}

/// Where the tour's targets were on screen, this frame.
///
/// Keyed on [`Category`], a `Copy` enum, and never on a label: the tree's own
/// header id is `make_persistent_id(("project_cat", label))` with the
/// **localized** label in it, which is precisely why the caller cannot look
/// these up itself.
#[derive(Debug, Default, Clone)]
pub struct Anchors {
    /// The root node's header row.
    pub root: Option<Rect>,
    /// Each top-level category header that was drawn.
    pub categories: Vec<(Category, Rect)>,
    /// The tree's scroll viewport — what "on screen" means for a tree anchor.
    pub viewport: Option<Rect>,
    /// The output pane's outer rect.
    pub output: Option<Rect>,
}

impl Anchors {
    /// Forget last frame's rects. Called where the panel clears its other
    /// per-frame state, so a rect can never outlive the frame that painted it.
    pub fn clear(&mut self) {
        self.root = None;
        self.categories.clear();
        self.viewport = None;
        // `output` is filled by the caller from the panel's stored state, not by
        // the tree, so it is not cleared here.
    }

    fn rect_for(&self, anchor: Anchor) -> Option<Rect> {
        match anchor {
            Anchor::ProjectRoot => self.root,
            Anchor::Category(c) => self
                .categories
                .iter()
                .find(|(k, _)| *k == c)
                .map(|(_, r)| *r),
            Anchor::OutputPanel => self.output,
        }
    }

    /// Is this anchor's rect both known and actually visible?
    ///
    /// A tree anchor also has to be inside the scroll viewport: a header
    /// scrolled under the panel edge still has a rect, and lighting it would put
    /// the spotlight outside the panel it belongs to.
    fn visible(&self, anchor: Anchor) -> Option<Rect> {
        let r = self.rect_for(anchor)?;
        if r.width() <= 0.0 || r.height() <= 0.0 {
            return None;
        }
        match anchor {
            Anchor::OutputPanel => Some(r),
            _ => match self.viewport {
                Some(v) if v.contains_rect(r) => Some(r),
                Some(_) => None,
                // No viewport reported — trust the rect rather than stall.
                None => Some(r),
            },
        }
    }
}

/// Which way the developer is moving, so a skipped step skips the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Running {
        idx: usize,
        dir: Dir,
        /// Frames left to wait for a reveal before giving up on this step.
        reveal_budget: u8,
    },
}

/// How many frames a step may spend waiting to be scrolled into view.
///
/// A reveal normally takes two — one to ask, one for the tree to publish the new
/// rect — so this is generous. It exists only so a component that can never be
/// revealed cannot wedge the tour.
const REVEAL_BUDGET: u8 = 8;

/// What the caller must do after a frame of the tour.
#[derive(Debug, Default, Clone, Copy)]
pub struct Outcome {
    /// The tour finished, was skipped, or was escaped. Set the machine flag.
    pub ended: bool,
    /// This anchor needs bringing into view before the balloon can point at it.
    pub reveal: Option<Anchor>,
}

/// What unchecking or checking the Help-menu item should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Start the tour now.
    Start,
    /// A project is needed first; say so and start nothing.
    NeedsProject,
    /// Just remember the flag.
    Remember,
}

/// Should the tour start by itself?
///
/// `project_just_opened` is an **edge**, not a state: the tour starts on the
/// frame a project appears, not on every frame one is open.
pub fn should_auto_start(shown: bool, project_just_opened: bool, blocked: bool) -> bool {
    !shown && project_just_opened && !blocked
}

/// What the Help-menu checkbox does when it changes.
///
/// Unchecking means "I have not seen it", which is the developer asking for the
/// tour. Checking means "I have", which is only a note to self.
pub fn on_menu_toggle(now_checked: bool, has_project: bool) -> MenuAction {
    match (now_checked, has_project) {
        (false, true) => MenuAction::Start,
        (false, false) => MenuAction::NeedsProject,
        (true, _) => MenuAction::Remember,
    }
}

/// The four rectangles that dim everything except `hole`.
///
/// Returned rather than painted so the geometry can be asserted without a
/// renderer. Together they cover `screen` minus `hole` exactly, and none of them
/// overlaps `hole` — which is the whole claim a spotlight makes.
pub fn scrim_bands(screen: Rect, hole: Rect) -> [Rect; 4] {
    let hole = hole.intersect(screen);
    [
        // Above.
        Rect::from_min_max(screen.min, Pos2::new(screen.max.x, hole.min.y)),
        // Below.
        Rect::from_min_max(Pos2::new(screen.min.x, hole.max.y), screen.max),
        // Left of the hole, between its top and bottom.
        Rect::from_min_max(
            Pos2::new(screen.min.x, hole.min.y),
            Pos2::new(hole.min.x, hole.max.y),
        ),
        // Right of it.
        Rect::from_min_max(
            Pos2::new(hole.max.x, hole.min.y),
            Pos2::new(screen.max.x, hole.max.y),
        ),
    ]
}

/// Which edge of the balloon the tail leaves from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
    Below,
    Above,
}

/// A placed balloon: its body, and the three points of its tail.
#[derive(Debug, Clone, Copy)]
pub struct Balloon {
    pub body: Rect,
    pub tail: [Pos2; 3],
    pub side: Side,
}

/// Gap between the target and the balloon body.
const GAP: f32 = 18.0;
/// How far the balloon must stay from the window edge.
const MARGIN: f32 = 12.0;
/// Half-width of the tail where it meets the body.
const TAIL_HALF: f32 = 9.0;
/// How far the tail tip reaches *inside* the target, so it demonstrably touches.
const TAIL_BITE: f32 = 6.0;

/// Place a balloon of `size` so it points at `target` and stays on `screen`.
///
/// Sides are tried right, left, below, above. Right first is not arbitrary: five
/// of the six targets live in the left-hand project tree, so the balloon lands
/// over the central pane with its tail pointing back at the tree — the way the
/// eye already travels. The sixth, the output pane, spans the full width, so
/// right and left both fail on space and `Above` wins, which is also correct.
pub fn place_balloon(screen: Rect, target: Rect, size: Vec2) -> Balloon {
    let field = screen.shrink(MARGIN);
    let need = |space: f32| space >= size.x + GAP;
    let need_v = |space: f32| space >= size.y + GAP;

    let side = if need(field.max.x - target.max.x) {
        Side::Right
    } else if need(target.min.x - field.min.x) {
        Side::Left
    } else if need_v(field.max.y - target.max.y) {
        Side::Below
    } else if need_v(target.min.y - field.min.y) {
        Side::Above
    } else {
        // Nowhere fits. Centre it and keep aiming — a balloon that overlaps its
        // target still points at the right thing; a tailless one does not.
        Side::Right
    };

    let c = target.center();
    let mut body = match side {
        Side::Right => Rect::from_min_size(Pos2::new(target.max.x + GAP, c.y - size.y / 2.0), size),
        Side::Left => Rect::from_min_size(
            Pos2::new(target.min.x - GAP - size.x, c.y - size.y / 2.0),
            size,
        ),
        Side::Below => Rect::from_min_size(Pos2::new(c.x - size.x / 2.0, target.max.y + GAP), size),
        Side::Above => Rect::from_min_size(
            Pos2::new(c.x - size.x / 2.0, target.min.y - GAP - size.y),
            size,
        ),
    };

    // Slide it back inside the window. This is what keeps a balloon on screen
    // when its target is hard against an edge.
    let dx = (field.min.x - body.min.x).max(0.0) - (body.max.x - field.max.x).max(0.0);
    let dy = (field.min.y - body.min.y).max(0.0) - (body.max.y - field.max.y).max(0.0);
    body = body.translate(Vec2::new(dx, dy));

    // The tip sits *inside* the target, so "the tail touches it" is true by
    // construction rather than by eye.
    let tip = match side {
        Side::Right => Pos2::new(target.max.x - TAIL_BITE, c.y.clamp(target.min.y, target.max.y)),
        Side::Left => Pos2::new(target.min.x + TAIL_BITE, c.y.clamp(target.min.y, target.max.y)),
        Side::Below => Pos2::new(c.x.clamp(target.min.x, target.max.x), target.max.y - TAIL_BITE),
        Side::Above => Pos2::new(c.x.clamp(target.min.x, target.max.x), target.min.y + TAIL_BITE),
    };

    // The base sits on the body edge facing the target, centred on the tip and
    // clamped clear of the rounded corners.
    let r = 14.0;
    let tail = match side {
        Side::Right | Side::Left => {
            let x = if side == Side::Right {
                body.min.x
            } else {
                body.max.x
            };
            let y = tip.y.clamp(body.min.y + r + TAIL_HALF, body.max.y - r - TAIL_HALF);
            [
                Pos2::new(x, y - TAIL_HALF),
                tip,
                Pos2::new(x, y + TAIL_HALF),
            ]
        }
        Side::Below | Side::Above => {
            let y = if side == Side::Below {
                body.min.y
            } else {
                body.max.y
            };
            let x = tip.x.clamp(body.min.x + r + TAIL_HALF, body.max.x - r - TAIL_HALF);
            [
                Pos2::new(x - TAIL_HALF, y),
                tip,
                Pos2::new(x + TAIL_HALF, y),
            ]
        }
    };

    Balloon { body, tail, side }
}

/// The balloon's colours.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fill: Color32,
    pub text: Color32,
    pub dim_text: Color32,
    pub ring: Color32,
    pub border: Color32,
    pub button_fill: Color32,
    pub button_text: Color32,
}

/// The dim laid over everything but the target — the same value the Building
/// modal and the KB-reindex scrim use, so the tour looks like the IDE's other
/// modals on every theme.
const DIM: Color32 = Color32::from_black_alpha(150);

/// Colours for the balloon on `theme`.
///
/// **The fill and the text are a fixed pair, deliberately.** The balloon always
/// sits on the same `from_black_alpha(150)` field on all 32 themes, so there is
/// nothing theme-dependent for its legibility to adapt to — and deriving ink
/// from `ui.visuals()` is what renders dark-on-dark under the glass themes
/// (CLAUDE.md). What *does* follow the theme is the decoration: the ring around
/// the lit component and the balloon's border.
pub fn balloon_palette(theme: &Theme) -> Palette {
    let accent = theme.accent;
    // The Next button borrows the accent only when white reads on it; otherwise
    // the accent stays an outline and the button keeps the charcoal.
    let accent_ok = crate::contrast::contrast_ratio(accent, Color32::WHITE) >= 4.5;
    Palette {
        fill: Color32::from_rgb(0x1E, 0x22, 0x2A),
        text: Color32::WHITE,
        dim_text: Color32::from_rgb(0xC2, 0xC9, 0xD4),
        ring: accent,
        border: accent,
        button_fill: if accent_ok {
            accent
        } else {
            Color32::from_rgb(0x2C, 0x33, 0x3E)
        },
        button_text: Color32::WHITE,
    }
}

/// The tour.
#[derive(Debug, Default)]
pub struct Walkthrough {
    state: State,
    /// A navigation the key handler took before the frame drew.
    pending: Option<Nav>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nav {
    Next,
    Back,
    Skip,
}

impl Default for State {
    fn default() -> Self {
        State::Idle
    }
}

impl Walkthrough {
    /// Is the tour on screen? Callers gate their own input on this.
    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running { .. })
    }

    /// Start at step one.
    pub fn start(&mut self, ctx: &egui::Context) {
        self.state = State::Running {
            idx: 0,
            dir: Dir::Forward,
            reveal_budget: REVEAL_BUDGET,
        };
        self.pending = None;
        ctx.request_repaint();
    }

    /// The step now showing, for tests and for a caller that wants to log it.
    pub fn step(&self) -> Option<Step> {
        match self.state {
            State::Running { idx, .. } => Step::ORDER.get(idx).copied(),
            State::Idle => None,
        }
    }

    /// Take the keyboard for the frame (R4, R5).
    ///
    /// Must be the **first** thing the frame does. Every key reader in this
    /// crate bottoms out in `InputState::events` — `key_pressed` walks it,
    /// `consume_shortcut` retains over it — so emptying it here is what stops
    /// all ~25 of them at once, `TextEdit` included. Gating them one by one
    /// would be a list that rots.
    pub fn take_keys(&mut self, ctx: &egui::Context) {
        if !self.is_running() {
            return;
        }
        ctx.input_mut(|i| {
            // Esc ends the tour like Skip (R5) — taken before the strip, or it
            // would be thrown away with everything else.
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                self.pending = Some(Nav::Skip);
            }
            i.events.retain(|e| {
                !matches!(
                    e,
                    egui::Event::Key { .. } | egui::Event::Text(_) | egui::Event::Ime(_)
                )
            });
            i.keys_down.clear();
        });
    }

    fn navigate(&mut self, nav: Nav) -> bool {
        let State::Running { idx, .. } = self.state else {
            return false;
        };
        match nav {
            Nav::Skip => {
                self.state = State::Idle;
                true
            }
            Nav::Next => {
                if idx + 1 < Step::ORDER.len() {
                    self.state = State::Running {
                        idx: idx + 1,
                        dir: Dir::Forward,
                        reveal_budget: REVEAL_BUDGET,
                    };
                    false
                } else {
                    self.state = State::Idle;
                    true
                }
            }
            Nav::Back => {
                if idx > 0 {
                    self.state = State::Running {
                        idx: idx - 1,
                        dir: Dir::Back,
                        reveal_budget: REVEAL_BUDGET,
                    };
                }
                false
            }
        }
    }

    /// Advance past a step whose component cannot be shown, the way the
    /// developer was already going.
    fn give_up_on_step(&mut self) -> bool {
        let State::Running { idx, dir, .. } = self.state else {
            return false;
        };
        match dir {
            Dir::Forward => self.navigate(Nav::Next),
            Dir::Back => {
                if idx == 0 {
                    self.state = State::Idle;
                    true
                } else {
                    self.navigate(Nav::Back)
                }
            }
        }
    }

    /// Draw the tour for one frame.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        anchors: &Anchors,
        tr: &Tr,
        theme: &Theme,
    ) -> Outcome {
        let mut out = Outcome::default();
        if let Some(nav) = self.pending.take() {
            out.ended = self.navigate(nav);
            if out.ended || !self.is_running() {
                return out;
            }
        }
        let State::Running { idx, .. } = self.state else {
            return out;
        };
        let Some(step) = Step::ORDER.get(idx).copied() else {
            self.state = State::Idle;
            out.ended = true;
            return out;
        };

        let screen = ctx.content_rect();
        let anchor = step.anchor();

        // Where is it? If it is not visible, ask for it and paint only the dim —
        // never a balloon, because a balloon with no target is a tail pointing
        // at nothing, which R16 forbids.
        let Some(target) = anchors.visible(anchor) else {
            ctx.request_repaint();
            if let State::Running { reveal_budget, .. } = &mut self.state {
                if *reveal_budget == 0 {
                    out.ended = self.give_up_on_step();
                    return out;
                }
                *reveal_budget -= 1;
            }
            out.reveal = Some(anchor);
            self.paint_dim_only(ctx, screen);
            return out;
        };

        let palette = balloon_palette(theme);
        let nav = self.paint(ctx, screen, target, step, tr, &palette);
        if let Some(nav) = nav {
            out.ended = self.navigate(nav);
        }
        out
    }

    /// Everything dimmed, nothing lit — one frame while a component is being
    /// scrolled into view.
    fn paint_dim_only(&self, ctx: &egui::Context, screen: Rect) {
        let id = egui::Id::new("walkthrough_overlay");
        ctx.memory_mut(|m| m.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, id)));
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.painter().rect_filled(screen, 0.0, DIM);
                ui.allocate_rect(screen, egui::Sense::click_and_drag());
            });
    }

    /// The spotlight, the ring, the balloon and its buttons.
    fn paint(
        &self,
        ctx: &egui::Context,
        screen: Rect,
        target: Rect,
        step: Step,
        tr: &Tr,
        p: &Palette,
    ) -> Option<Nav> {
        let idx = match self.state {
            State::Running { idx, .. } => idx,
            State::Idle => return None,
        };

        let id = egui::Id::new("walkthrough_overlay");
        ctx.memory_mut(|m| m.set_modal_layer(egui::LayerId::new(egui::Order::Foreground, id)));
        crate::app::raise_modal_layer(ctx, id);

        let mut nav = None;
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                // Swallow everything underneath. The bands are painted after, so
                // this allocation never covers the balloon's own controls.
                ui.allocate_rect(screen, egui::Sense::click_and_drag());
                let painter = ui.painter().clone();

                // Size the balloon from the text it holds, never from available
                // space: a window may never resize itself, and a balloon
                // measured against the window is how that starts.
                let inner = BALLOON_W - 2.0 * PAD;
                let title = painter.layout(
                    step.title(tr).to_owned(),
                    FontId::proportional(17.0),
                    p.text,
                    inner,
                );
                let body = painter.layout(
                    step.body(tr).to_owned(),
                    FontId::proportional(13.5),
                    p.dim_text,
                    inner,
                );
                let height =
                    PAD + title.size().y + 8.0 + body.size().y + 14.0 + BTN_H + PAD;
                let placed = place_balloon(screen, target, Vec2::new(BALLOON_W, height));

                for band in scrim_bands(screen, target) {
                    if band.width() > 0.0 && band.height() > 0.0 {
                        painter.rect_filled(band, 0.0, DIM);
                    }
                }
                painter.rect_stroke(
                    target.expand(2.0),
                    CornerRadius::same(6),
                    Stroke::new(2.0, p.ring),
                    egui::StrokeKind::Outside,
                );

                // Body, then tail, then the outline over both, so the join
                // between them is never drawn as a seam.
                painter.rect_filled(placed.body, CornerRadius::same(14), p.fill);
                painter.add(egui::Shape::convex_polygon(
                    placed.tail.to_vec(),
                    p.fill,
                    Stroke::NONE,
                ));
                painter.rect_stroke(
                    placed.body,
                    CornerRadius::same(14),
                    Stroke::new(1.0, p.border),
                    egui::StrokeKind::Inside,
                );
                painter.line_segment([placed.tail[0], placed.tail[1]], Stroke::new(1.0, p.border));
                painter.line_segment([placed.tail[1], placed.tail[2]], Stroke::new(1.0, p.border));

                let x = placed.body.min.x + PAD;
                let y = placed.body.min.y + PAD;
                let title_h = title.size().y;
                painter.galley(Pos2::new(x, y), title, p.text);
                painter.galley(Pos2::new(x, y + title_h + 8.0), body, p.dim_text);

                // Buttons along the bottom edge.
                let row = Rect::from_min_max(
                    Pos2::new(x, placed.body.max.y - PAD - BTN_H),
                    Pos2::new(placed.body.max.x - PAD, placed.body.max.y - PAD),
                );
                painter.text(
                    Pos2::new(row.min.x, row.center().y),
                    Align2::LEFT_CENTER,
                    tr.walkthrough_progress
                        .replace("{n}", &(idx + 1).to_string())
                        .replace("{total}", &Step::ORDER.len().to_string()),
                    FontId::proportional(11.5),
                    p.dim_text,
                );

                let mut right = row.max.x;
                for (label, action, primary) in [
                    (tr.walkthrough_next, Nav::Next, true),
                    (tr.walkthrough_back, Nav::Back, false),
                    (tr.walkthrough_skip, Nav::Skip, false),
                ] {
                    let w = painter
                        .layout_no_wrap(
                            label.to_owned(),
                            FontId::proportional(12.5),
                            Color32::WHITE,
                        )
                        .size()
                        .x
                        + 20.0;
                    let r = Rect::from_min_max(
                        Pos2::new(right - w, row.min.y),
                        Pos2::new(right, row.max.y),
                    );
                    right = r.min.x - 8.0;
                    let enabled = !(action == Nav::Back && idx == 0);
                    let resp = ui.interact(
                        r,
                        egui::Id::new(("walkthrough_btn", label)),
                        egui::Sense::click(),
                    );
                    let fill = if !enabled {
                        Color32::from_rgb(0x2A, 0x2F, 0x38)
                    } else if primary {
                        p.button_fill
                    } else {
                        Color32::from_rgb(0x33, 0x3A, 0x46)
                    };
                    painter.rect_filled(r, CornerRadius::same(6), fill);
                    painter.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(12.5),
                        if enabled {
                            p.button_text
                        } else {
                            Color32::from_rgb(0x77, 0x7F, 0x8B)
                        },
                    );
                    if enabled && resp.clicked() {
                        nav = Some(action);
                    }
                }

                #[cfg(test)]
                ui.data_mut(|d| {
                    d.insert_temp(
                        egui::Id::new("walkthrough_probe"),
                        Probe {
                            step: idx,
                            hole: target,
                            body: placed.body,
                            tail_tip: placed.tail[1],
                            buttons: row,
                        },
                    )
                });
            });
        nav
    }
}

/// Balloon width. A constant, never `available_width()`: the tour must not be
/// able to change any layout, and a balloon measured against the window is how
/// a window starts measuring itself back.
const BALLOON_W: f32 = 340.0;
const PAD: f32 = 16.0;
const BTN_H: f32 = 26.0;

/// What a test can read back about the frame that was just painted.
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Probe {
    pub step: usize,
    pub hole: Rect,
    pub body: Rect,
    pub tail_tip: Pos2,
    pub buttons: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))
    }

    // ── the pure predicates ─────────────────────────────────────────────────

    #[test]
    fn it_starts_only_on_the_first_project_open_of_a_fresh_machine() {
        assert!(should_auto_start(false, true, false));
        // Already shown — never again.
        assert!(!should_auto_start(true, true, false));
        // A project is open but did not just open: not an edge.
        assert!(!should_auto_start(false, false, false));
        // Something else owns the screen; defer rather than fight it.
        assert!(!should_auto_start(false, true, true));
    }

    #[test]
    fn unchecking_the_menu_item_is_what_replays_the_tour() {
        assert_eq!(on_menu_toggle(false, true), MenuAction::Start);
        assert_eq!(on_menu_toggle(false, false), MenuAction::NeedsProject);
        assert_eq!(on_menu_toggle(true, true), MenuAction::Remember);
        assert_eq!(on_menu_toggle(true, false), MenuAction::Remember);
    }

    // ── geometry ────────────────────────────────────────────────────────────

    #[test]
    fn the_bands_cover_everything_except_the_hole() {
        let s = screen();
        let hole = Rect::from_min_size(Pos2::new(300.0, 200.0), Vec2::new(220.0, 40.0));
        let bands = scrim_bands(s, hole);
        for b in bands {
            assert!(
                b.intersect(hole).area() <= 0.01,
                "a band overlaps the lit component: {b:?} vs {hole:?}"
            );
        }
        let covered: f32 = bands.iter().map(|b| b.area().max(0.0)).sum();
        assert!(
            (covered - (s.area() - hole.area())).abs() < 1.0,
            "bands cover {covered}, expected {}",
            s.area() - hole.area()
        );
    }

    #[test]
    fn the_tail_always_ends_inside_the_component_it_points_at() {
        let s = screen();
        let size = Vec2::new(BALLOON_W, 160.0);
        // A target hard against each edge in turn, plus the middle.
        for target in [
            Rect::from_min_size(Pos2::new(0.0, 400.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(1100.0, 400.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(500.0, 0.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(500.0, 870.0), Vec2::new(300.0, 30.0)),
            Rect::from_min_size(Pos2::new(0.0, 860.0), Vec2::new(1400.0, 40.0)),
            Rect::from_min_size(Pos2::new(600.0, 430.0), Vec2::new(200.0, 30.0)),
        ] {
            let b = place_balloon(s, target, size);
            assert!(
                target.expand(0.5).contains(b.tail[1]),
                "tail tip {:?} is outside its target {target:?} (side {:?})",
                b.tail[1],
                b.side
            );
            assert!(
                s.contains_rect(b.body),
                "balloon {:?} left the window for target {target:?}",
                b.body
            );
        }
    }

    #[test]
    fn a_balloon_never_grows_with_the_window() {
        let small = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0));
        let big = Rect::from_min_size(Pos2::ZERO, Vec2::new(2560.0, 1400.0));
        let target = Rect::from_min_size(Pos2::new(40.0, 300.0), Vec2::new(200.0, 30.0));
        let size = Vec2::new(BALLOON_W, 160.0);
        let a = place_balloon(small, target, size);
        let b = place_balloon(big, target, size);
        assert_eq!(a.body.size(), b.body.size(), "the balloon read the window");
    }

    // ── the state machine ───────────────────────────────────────────────────

    #[test]
    fn every_route_out_ends_the_tour() {
        let ctx = egui::Context::default();
        for nav in [Nav::Skip, Nav::Next] {
            let mut w = Walkthrough::default();
            w.start(&ctx);
            if nav == Nav::Next {
                // Walk to the last step; none of these should end it.
                for _ in 0..Step::ORDER.len() - 1 {
                    assert!(!w.navigate(Nav::Next), "ended early");
                }
            }
            assert!(w.navigate(nav), "{nav:?} must end the tour");
            assert!(!w.is_running());
        }
    }

    #[test]
    fn back_stops_at_the_first_step_rather_than_ending() {
        let ctx = egui::Context::default();
        let mut w = Walkthrough::default();
        w.start(&ctx);
        assert!(!w.navigate(Nav::Back));
        assert_eq!(w.step(), Some(Step::ProjectSettings));
    }

    #[test]
    fn the_six_steps_point_at_six_different_components() {
        let anchors: Vec<Anchor> = Step::ORDER.iter().map(|s| s.anchor()).collect();
        for (i, a) in anchors.iter().enumerate() {
            assert!(
                !anchors[..i].contains(a),
                "{a:?} is pointed at twice — step {i} duplicates an earlier one"
            );
        }
        // The one equivalence that is not guessable from the name.
        assert_eq!(
            Step::KnowledgeBase.anchor(),
            Anchor::Category(Category::Documentation),
            "the Knowledge Base node IS Category::Documentation in the tree"
        );
    }

    #[test]
    fn an_anchor_outside_the_viewport_is_not_visible() {
        let mut a = Anchors {
            viewport: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 500.0))),
            ..Default::default()
        };
        a.categories
            .push((Category::Assets, Rect::from_min_size(Pos2::new(10.0, 700.0), Vec2::new(300.0, 30.0))));
        assert!(a.visible(Anchor::Category(Category::Assets)).is_none());

        a.categories[0].1 = Rect::from_min_size(Pos2::new(10.0, 100.0), Vec2::new(300.0, 30.0));
        assert!(a.visible(Anchor::Category(Category::Assets)).is_some());
    }

    // ── legibility, on every theme ──────────────────────────────────────────

    #[test]
    fn the_balloon_reads_on_all_32_themes() {
        use crate::contrast::contrast_ratio;
        for t in crate::theme::THEMES {
            let p = balloon_palette(t);
            let body = contrast_ratio(p.text, p.fill);
            assert!(
                body >= 4.5,
                "{}: balloon text {body:.2}:1 against its own fill",
                t.id
            );
            let dim = contrast_ratio(p.dim_text, p.fill);
            assert!(dim >= 4.5, "{}: balloon body text {dim:.2}:1", t.id);
            let btn = contrast_ratio(p.button_text, p.button_fill);
            assert!(
                btn >= 4.5,
                "{}: Next button {btn:.2}:1 — the accent was taken when it should not have been",
                t.id
            );
        }
    }
}

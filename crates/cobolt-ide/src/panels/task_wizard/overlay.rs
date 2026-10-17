// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Task Wizard on screen: the dim with its holes, the ring and the balloon, in one OS
//! window (spec 104 R12–R16, R33, R34).
//!
//! # Why the dim is many small Areas
//!
//! The Walkthrough lays one interactable Area over the whole screen, which makes the tour
//! read-only — exactly what it wants. The wizard's lit region must stay **live**, and in
//! egui an interactable Area takes the pointer over its whole rect, hover and wheel
//! included. So the dim is cut ([`spotlight::blockers`]) into the rectangles that cover the
//! screen *minus* the holes, and each is its own interactable Area at [`Order::Middle`],
//! raised every frame: above every window and panel, never over a hole.
//!
//! Everything a lit widget opens — a combo box's list, a menu, a colour picker — is a
//! popup at [`Order::Foreground`], above the blockers, so it stays usable where it hangs
//! over the dim (R14). The balloon is a Foreground Area too, but it is **not** raised every
//! frame, so a popup opened after it lands on top of it.
//!
//! No `set_modal_layer`: a modal layer would deny focus and scrolling to the lit window
//! itself.

use std::collections::{HashMap, HashSet};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, LayerId, Order, Pos2, Rect, Sense, Stroke,
    Vec2, ViewportCommand, ViewportId,
};

use super::anchors::{AnchorId, Marks};
use super::{Card, Nav, Plan, StepCard, SummaryCard};
use crate::i18n::Tr;
use crate::panels::spotlight::{
    blockers, paint_balloon_frame, paint_ring, place_balloon_avoiding, Palette, BODY_RADIUS, DIM,
    MAX_BLOCKERS, MAX_HOLES,
};

/// Balloon width: a constant, never the space available (R34).
const BALLOON_W: f32 = 360.0;
const PAD: f32 = 16.0;
const BTN_H: f32 = 26.0;
const ROW_GAP: f32 = 8.0;

/// What one window remembers between frames.
#[derive(Debug, Default)]
struct Memory {
    /// The step these sets belong to.
    serial: u64,
    /// The windows already open when the step began; any other is a hole (R16).
    windows_at_start: HashSet<LayerId>,
    /// Where the live region was last painted — the keyboard policy reads it (R23).
    live: Option<Rect>,
    /// The text field that held the keyboard at the end of the last frame (R22's first `Esc`).
    field: Option<Id>,
    /// The lit window already brought forward for this step.
    raised: Option<LayerId>,
}

/// The overlay's memory, per OS window.
#[derive(Debug, Default)]
pub struct Overlay {
    windows: HashMap<ViewportId, Memory>,
}

/// What a test can read back about a painted frame.
#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct Probe {
    pub holes: Vec<Rect>,
    pub blockers: Vec<Rect>,
    pub ring: Option<Rect>,
    pub balloon: Option<Rect>,
    pub buttons: Vec<(&'static str, Rect)>,
}

#[cfg(test)]
pub(crate) const PROBE_ID: &str = "task_wizard_probe";

impl Overlay {
    /// Where the live region was last painted in `viewport`.
    pub fn live_rect(&self, viewport: ViewportId) -> Option<Rect> {
        self.windows.get(&viewport).and_then(|m| m.live)
    }

    /// The text field focused in `viewport` when the last frame ended.
    pub fn field_focus(&self, viewport: ViewportId) -> Option<Id> {
        self.windows.get(&viewport).and_then(|m| m.field)
    }

    /// Forget everything — the wizard stopped.
    pub fn clear(&mut self) {
        self.windows.clear();
    }

    /// Paint the wizard in the window `ctx` is drawing, at the end of its pass, against the
    /// rects that pass painted. `window_name` names another window for the chip that sends
    /// the developer there (R15). Returns what the developer pressed.
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        plan: Option<&Plan>,
        marks: &Marks,
        tr: &Tr,
        palette: &Palette,
        window_name: &dyn Fn(ViewportId) -> String,
    ) -> Option<Nav> {
        let vp = ctx.viewport_id();
        let Some(plan) = plan else {
            self.windows.remove(&vp);
            return None;
        };
        let screen = ctx.content_rect();
        let mem = self.windows.entry(vp).or_default();
        mem.field = if ctx.text_edit_focused() {
            ctx.memory(|m| m.focused())
        } else {
            None
        };
        if mem.serial != plan.serial || mem.windows_at_start.is_empty() {
            mem.serial = plan.serial;
            mem.windows_at_start = middle_windows(ctx);
            mem.raised = None;
        }

        // The toggles are always holes, so the wizard can always be turned off (R16).
        let toggles: Vec<Rect> = marks
            .all_in(AnchorId::Toggle, vp)
            .map(|m| m.rect.expand(2.0))
            .collect();

        if plan.viewport != vp {
            mem.live = None;
            paint_blockers(ctx, screen, &toggles);
            return paint_chip(ctx, screen, plan.viewport, window_name, tr, palette);
        }

        let find = |a: AnchorId| marks.find(a, Some(vp)).filter(|m| m.viewport == vp);
        let ring = plan
            .ring
            .and_then(find)
            .map(|m| m.rect.intersect(m.clip))
            .filter(|r| r.is_positive());
        let live_mark = plan.live.and_then(find);
        let live = live_mark.map(|m| m.rect).or(ring);
        mem.live = if plan.dim_only { None } else { live };

        // The lit window is brought above the other windows (and stays below the blockers,
        // which never cover it) — ONCE per step. Raising it every frame would bury, for good,
        // any window it opens itself: egui keeps the order of layers that all ask to be on
        // top, so a child such as the binding editor's Dropdown configuration would sink
        // under its parent a frame after it appeared.
        if let Some(layer) = live_mark.and_then(|m| m.layer) {
            if mem.raised != Some(layer) {
                ctx.move_to_top(layer);
                mem.raised = Some(layer);
            }
        }

        let mut holes: Vec<Rect> = Vec::with_capacity(MAX_HOLES + 2);
        if !plan.dim_only {
            holes.extend(live);
        }
        holes.extend(toggles.iter().copied());
        if !plan.dim_only {
            let skip: HashSet<LayerId> = live_mark.and_then(|m| m.layer).into_iter().collect();
            for layer in middle_windows(ctx) {
                if mem.windows_at_start.contains(&layer) || skip.contains(&layer) {
                    continue;
                }
                if let Some(r) = ctx.memory(|m| m.area_rect(layer.id)) {
                    holes.push(r);
                }
            }
        }
        let pieces = paint_blockers(ctx, screen, &holes);

        #[cfg(test)]
        let mut probe = Probe {
            holes: holes.clone(),
            blockers: pieces.clone(),
            ring,
            ..Default::default()
        };
        #[cfg(not(test))]
        let _ = pieces;

        if plan.dim_only {
            #[cfg(test)]
            ctx.data_mut(|d| d.insert_temp(Id::new(PROBE_ID), probe));
            return None;
        }

        let nav = match &plan.card {
            Card::Step(card) => {
                let aim = ring.unwrap_or_else(|| Rect::from_center_size(screen.center(), Vec2::ZERO));
                let avoid = live.unwrap_or(aim);
                paint_step(
                    ctx,
                    screen,
                    aim,
                    avoid,
                    ring,
                    card,
                    tr,
                    palette,
                    #[cfg(test)]
                    &mut probe,
                )
            }
            Card::Summary(card) => paint_summary(
                ctx,
                screen,
                card,
                tr,
                palette,
                #[cfg(test)]
                &mut probe,
            ),
        };
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(Id::new(PROBE_ID), probe));
        nav
    }
}

/// The ids of our own blocker Areas, which are never "a window that opened".
fn blocker_id(i: usize) -> Id {
    Id::new(("task_wizard_block", i))
}

/// The top-level windows visible in this OS window, other than the wizard's own.
fn middle_windows(ctx: &egui::Context) -> HashSet<LayerId> {
    let ours: HashSet<Id> = (0..MAX_BLOCKERS).map(blocker_id).collect();
    ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter(|l| l.order == Order::Middle && !ours.contains(&l.id))
            .collect()
    })
}

/// The dim, as interactable Areas over the screen minus `holes`. Returns the pieces.
fn paint_blockers(ctx: &egui::Context, screen: Rect, holes: &[Rect]) -> Vec<Rect> {
    let cut = blockers(screen, holes);
    let pieces = cut.as_slice().to_vec();
    for (i, r) in pieces.iter().enumerate() {
        let id = blocker_id(i);
        egui::Area::new(id)
            .order(Order::Middle)
            .fixed_pos(r.min)
            .constrain(false)
            .interactable(true)
            .show(ctx, |ui| {
                ui.painter().rect_filled(*r, 0.0, DIM);
                // Swallow the pointer here: a stray click must not wander off the task.
                ui.interact(*r, id.with("hit"), Sense::click_and_drag());
                ui.allocate_rect(*r, Sense::hover());
            });
        ctx.move_to_top(LayerId::new(Order::Middle, id));
    }
    pieces
}

/// A row of buttons, laid out right to left from `right`. Returns the clicked one.
struct Button {
    label: &'static str,
    nav: Nav,
    primary: bool,
    enabled: bool,
}

fn button_width(painter: &egui::Painter, label: &str) -> f32 {
    painter
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.5), Color32::WHITE)
        .size()
        .x
        + 20.0
}

fn draw_button(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    r: Rect,
    b: &Button,
    p: &Palette,
) -> bool {
    let resp = ui.interact(r, Id::new(("task_wizard_btn", b.nav as u8, b.label)), Sense::click());
    let fill = if !b.enabled {
        Color32::from_rgb(0x2A, 0x2F, 0x38)
    } else if b.primary {
        p.button_fill
    } else if resp.hovered() {
        Color32::from_rgb(0x3D, 0x45, 0x52)
    } else {
        Color32::from_rgb(0x33, 0x3A, 0x46)
    };
    painter.rect_filled(r, CornerRadius::same(6), fill);
    if b.primary && b.enabled && resp.hovered() {
        painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
    }
    painter.text(
        r.center(),
        Align2::CENTER_CENTER,
        b.label,
        FontId::proportional(12.5),
        if b.enabled {
            p.button_text
        } else {
            Color32::from_rgb(0x77, 0x7F, 0x8B)
        },
    );
    b.enabled && resp.clicked()
}

#[allow(clippy::too_many_arguments)]
fn paint_step(
    ctx: &egui::Context,
    screen: Rect,
    aim: Rect,
    avoid: Rect,
    ring: Option<Rect>,
    card: &StepCard,
    tr: &Tr,
    p: &Palette,
    #[cfg(test)] probe: &mut Probe,
) -> Option<Nav> {
    let id = Id::new("task_wizard_balloon");
    let mut nav = None;
    // Lay out first, with a painter of the screen; then place, then paint inside the Area.
    let measure = ctx.layer_painter(LayerId::new(Order::Foreground, id));
    let inner = BALLOON_W - 2.0 * PAD;
    let head = format!(
        "{}  ·  {}",
        card.task,
        tr.task_wizard_progress
            .replace("{n}", &card.n.to_string())
            .replace("{m}", &card.m.to_string())
    );
    let head_g = measure.layout(head, FontId::proportional(11.5), p.dim_text, inner);
    let title_g = measure.layout(card.title.to_owned(), FontId::proportional(17.0), p.text, inner);
    let body_g = measure.layout(card.body.to_owned(), FontId::proportional(13.5), p.dim_text, inner);
    let note = match (card.did, card.already) {
        (Some(did), _) => Some(format!("✓ {did}")),
        (None, true) => Some(tr.task_wizard_already.to_owned()),
        _ => None,
    };
    let note_g = note.map(|n| {
        measure.layout(n, FontId::proportional(12.5), Color32::from_rgb(0x8F, 0xE3, 0xA8), inner)
    });

    let mut height = PAD + head_g.size().y + 6.0 + title_g.size().y + 8.0 + body_g.size().y;
    if let Some(g) = &note_g {
        height += ROW_GAP + g.size().y;
    }
    if card.do_for_me {
        height += 14.0 + BTN_H;
    }
    height += 14.0 + BTN_H + PAD;

    let placed = if ring.is_some() {
        place_balloon_avoiding(screen, avoid, aim, Vec2::new(BALLOON_W, height))
    } else {
        let body = Rect::from_center_size(screen.center(), Vec2::new(BALLOON_W, height));
        crate::panels::spotlight::Balloon {
            body,
            tail: [body.center(); 3],
            side: crate::panels::spotlight::Side::Below,
        }
    };

    egui::Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(placed.body.min)
        .constrain(false)
        .interactable(true)
        .show(ctx, |ui| {
            let mut painter = ui.painter().clone();
            painter.set_clip_rect(screen);
            ui.allocate_rect(placed.body, Sense::click());
            if let Some(r) = ring {
                paint_ring(&painter, r, p);
            }
            if ring.is_some() {
                paint_balloon_frame(&painter, &placed, p);
            } else {
                paint_body(&painter, placed.body, p);
            }

            let x = placed.body.min.x + PAD;
            let mut y = placed.body.min.y + PAD;
            let h = head_g.size().y;
            painter.galley(Pos2::new(x, y), head_g, p.dim_text);
            y += h + 6.0;
            let h = title_g.size().y;
            painter.galley(Pos2::new(x, y), title_g, p.text);
            y += h + 8.0;
            let h = body_g.size().y;
            painter.galley(Pos2::new(x, y), body_g, p.dim_text);
            y += h;
            if let Some(g) = note_g {
                y += ROW_GAP;
                let h = g.size().y;
                painter.galley(Pos2::new(x, y), g, Color32::from_rgb(0x8F, 0xE3, 0xA8));
                y += h;
            }
            let right = placed.body.max.x - PAD;
            if card.do_for_me {
                y += 14.0;
                let r = Rect::from_min_max(Pos2::new(x, y), Pos2::new(right, y + BTN_H));
                let b = Button {
                    label: tr.task_wizard_do_for_me,
                    nav: Nav::DoForMe,
                    primary: false,
                    enabled: true,
                };
                if draw_button(ui, &painter, r, &b, p) {
                    nav = Some(Nav::DoForMe);
                }
                #[cfg(test)]
                probe.buttons.push((b.label, r));
                y += BTN_H;
            }
            y += 14.0;
            let row = Rect::from_min_max(Pos2::new(x, y), Pos2::new(right, y + BTN_H));
            // Exit on the left, Back and Next on the right.
            let exit = Button {
                label: tr.task_wizard_exit,
                nav: Nav::Exit,
                primary: false,
                enabled: true,
            };
            let w = button_width(&painter, exit.label);
            let r = Rect::from_min_size(row.min, Vec2::new(w, BTN_H));
            if draw_button(ui, &painter, r, &exit, p) {
                nav = Some(Nav::Exit);
            }
            #[cfg(test)]
            probe.buttons.push((exit.label, r));
            let mut at = row.max.x;
            for b in [
                Button {
                    label: if card.last { tr.task_wizard_finish } else { tr.task_wizard_next },
                    nav: Nav::Next,
                    // Next is primary unless the step still wants doing (R17): then the
                    // developer's own action in the lit region is what moves on.
                    primary: true,
                    enabled: true,
                },
                Button {
                    label: tr.task_wizard_back,
                    nav: Nav::Back,
                    primary: false,
                    enabled: card.back,
                },
            ] {
                let w = button_width(&painter, b.label);
                let r = Rect::from_min_max(Pos2::new(at - w, row.min.y), Pos2::new(at, row.max.y));
                at = r.min.x - 8.0;
                if draw_button(ui, &painter, r, &b, p) {
                    nav = Some(b.nav);
                }
                #[cfg(test)]
                probe.buttons.push((b.label, r));
            }
            #[cfg(test)]
            {
                probe.balloon = Some(placed.body);
            }
        });
    nav
}

fn paint_body(painter: &egui::Painter, body: Rect, p: &Palette) {
    painter.rect_filled(body, CornerRadius::same(BODY_RADIUS as u8), p.fill);
    painter.rect_stroke(
        body,
        CornerRadius::same(BODY_RADIUS as u8),
        Stroke::new(1.0, p.border),
        egui::StrokeKind::Inside,
    );
}

fn paint_summary(
    ctx: &egui::Context,
    screen: Rect,
    card: &SummaryCard,
    tr: &Tr,
    p: &Palette,
    #[cfg(test)] probe: &mut Probe,
) -> Option<Nav> {
    let id = Id::new("task_wizard_balloon");
    let measure = ctx.layer_painter(LayerId::new(Order::Foreground, id));
    let inner = BALLOON_W - 2.0 * PAD;
    let title = tr.task_wizard_summary_title.replace("{task}", card.task);
    let mut text = String::new();
    for (head, list) in [
        (tr.task_wizard_summary_done, &card.done),
        (tr.task_wizard_summary_for_you, &card.for_you),
        (tr.task_wizard_summary_skipped, &card.skipped),
    ] {
        if list.is_empty() {
            continue;
        }
        text.push_str(head);
        text.push('\n');
        for item in list {
            text.push_str("  • ");
            text.push_str(item);
            text.push('\n');
        }
        text.push('\n');
    }
    text.push_str(card.result);
    let title_g = measure.layout(title, FontId::proportional(17.0), p.text, inner);
    let body_g = measure.layout(text, FontId::proportional(13.0), p.dim_text, inner);
    let height = PAD + title_g.size().y + 8.0 + body_g.size().y + 14.0 + BTN_H + PAD;
    let body = Rect::from_center_size(screen.center(), Vec2::new(BALLOON_W, height));
    let mut nav = None;
    egui::Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(body.min)
        .constrain(false)
        .interactable(true)
        .show(ctx, |ui| {
            let mut painter = ui.painter().clone();
            painter.set_clip_rect(screen);
            ui.allocate_rect(body, Sense::click());
            paint_body(&painter, body, p);
            let x = body.min.x + PAD;
            let y = body.min.y + PAD;
            let th = title_g.size().y;
            painter.galley(Pos2::new(x, y), title_g, p.text);
            painter.galley(Pos2::new(x, y + th + 8.0), body_g, p.dim_text);
            let b = Button {
                label: tr.task_wizard_close,
                nav: Nav::Close,
                primary: true,
                enabled: true,
            };
            let w = button_width(&painter, b.label);
            let r = Rect::from_min_max(
                Pos2::new(body.max.x - PAD - w, body.max.y - PAD - BTN_H),
                Pos2::new(body.max.x - PAD, body.max.y - PAD),
            );
            if draw_button(ui, &painter, r, &b, p) {
                nav = Some(Nav::Close);
            }
            #[cfg(test)]
            {
                probe.buttons.push((b.label, r));
                probe.balloon = Some(body);
            }
        });
    nav
}

/// In a window the step is not in: a chip that names where it continues and brings that
/// window forward (R15).
fn paint_chip(
    ctx: &egui::Context,
    screen: Rect,
    target: ViewportId,
    window_name: &dyn Fn(ViewportId) -> String,
    tr: &Tr,
    p: &Palette,
) -> Option<Nav> {
    let id = Id::new("task_wizard_chip");
    let text = tr.task_wizard_continue_in.replace("{window}", &window_name(target));
    let measure = ctx.layer_painter(LayerId::new(Order::Foreground, id));
    let g = measure.layout(text, FontId::proportional(13.0), p.text, BALLOON_W - 2.0 * PAD);
    let bw = button_width(&measure, tr.task_wizard_show_window);
    let size = Vec2::new(BALLOON_W, PAD + g.size().y + 12.0 + BTN_H + PAD);
    let body = Rect::from_center_size(screen.center(), size);
    egui::Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(body.min)
        .constrain(false)
        .interactable(true)
        .show(ctx, |ui| {
            let mut painter = ui.painter().clone();
            painter.set_clip_rect(screen);
            ui.allocate_rect(body, Sense::click());
            paint_body(&painter, body, p);
            let gh = g.size().y;
            painter.galley(body.min + Vec2::splat(PAD), g, p.text);
            let r = Rect::from_min_size(
                Pos2::new(body.max.x - PAD - bw, body.min.y + PAD + gh + 12.0),
                Vec2::new(bw, BTN_H),
            );
            let b = Button {
                label: tr.task_wizard_show_window,
                nav: Nav::Next,
                primary: true,
                enabled: true,
            };
            if draw_button(ui, &painter, r, &b, p) {
                ctx.send_viewport_cmd_to(target, ViewportCommand::Focus);
            }
        });
    None
}

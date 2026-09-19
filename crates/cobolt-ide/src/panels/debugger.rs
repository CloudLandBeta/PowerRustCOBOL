// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Debugger floating window — Phase 7+.
//!
//! Opened automatically when a debug session starts. Provides:
//!   • Debug toolbar: Stop / Continue (F5) / Step Over (F10) / Pause
//!   • Source viewer: line numbers, breakpoint gutter (●), current-line arrow (►),
//!     simple COBOL syntax colouring
//!   • Tabbed data panel: Variables (filterable), Call Stack, Breakpoints

use egui::{Color32, Context, Key, RichText, ScrollArea, TextEdit, Vec2};
use egui_extras::{Column, TableBuilder};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::i18n::Tr;
use crate::panels::empty_blocks::{folds, marker_text, FoldKind, HiddenRun};
use crate::runner::{DebugRunner, RunMsg};
use cobolt_runtime::{
    DebugAnswer, DebugEvent, DebugFrame, DebugQuery, ScopeInfo, SpecialValue, StopReason, VarInfo,
    VarSnapshot,
};

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy, Default)]
enum Tab {
    #[default]
    Variables,
    Watches,
    CallStack,
    Breakpoints,
}

/// The investigation dock along the bottom.
///
/// Separate tabs rather than one console, so "what did this program do to my
/// files" is a place to look rather than a grep through unrelated chatter.
#[derive(PartialEq, Clone, Copy, Default)]
enum DockTab {
    #[default]
    Console,
    Events,
    FileIo,
    Problems,
    Timeline,
}

/// One line in the investigation dock.
pub struct DockLine {
    pub channel: cobolt_runtime::OutputChannel,
    pub text: String,
    /// Milliseconds since the session started — the Timeline's ordering.
    pub at_ms: u64,
}

/// A persisted watch expression and its last answer.
pub struct Watch {
    pub expression: String,
    /// `None` until the first evaluation of this stop.
    pub value: Option<String>,
    pub error: Option<String>,
}

// ── DebugAction ───────────────────────────────────────────────────────────────

/// Action requested by the debug window in a single frame.
pub enum DebugAction {
    Stop,
    /// Ask the stopped debuggee a question. The answer arrives as
    /// `DebugEvent::Answer` and is folded back in by `apply_event`.
    Query(u64, DebugQuery),
    Continue,
    StepOver,
    StepIn,
    /// Run until the current PERFORM or CALL returns, then pause in the caller.
    StepOut,
    /// Run to a 1-based source line, then pause. Gives up if the frame it was
    /// issued from returns first.
    RunToCursor(u32),
    Pause,
    /// The developer clicked the gutter beside a line: add or remove a
    /// breakpoint there. Carries the 1-based line in the displayed source.
    ToggleBreakpoint(u32),
}

/// A blank source line — a thin spacer between statements.
const CODE_BLANK_H: f32 = 1.0;

/// The stopped line's band.
///
/// A TINT, not a slab. It used to be fluorescent lime with near-black text —
/// legible, and the single thing that made the window read as a terminal from
/// 1975 (operator, 2026-09-17: "too mainframeish"). Every modern debugger marks
/// the stopped line the same way: warm the row, put an accent bar down its
/// edge, and leave the code alone. The syntax colours stay, so the current line
/// still reads as COBOL instead of turning into a monochrome ribbon.
const CURRENT_BG: Color32 = Color32::from_rgb(31, 58, 92);

/// The accent bar down the left edge of the stopped row, and the arrow and line
/// number that go with it. Amber, because it is the one hue the syntax palette
/// does not already use — so "you are here" cannot be mistaken for a keyword.
const CURRENT_ACCENT: Color32 = Color32::from_rgb(255, 191, 71);

/// The stopped line's own annotations — the arrow, its line number and the
/// inline values. Warm and light: they belong to the accent, not to the code.
const CURRENT_INK: Color32 = Color32::from_rgb(255, 214, 150);

/// Width of the accent bar.
const CURRENT_STRIPE_W: f32 = 3.0;

/// Where one frame ends and the next begins. The panes used to meet with
/// nothing drawn between them.
const FRAME_LINE: Color32 = Color32::from_gray(110);

/// Which material the debugger window is made of.
///
/// The debugger carries its OWN look, separate from the IDE's theme: it is a
/// tool you stare at while reading one program, and the ground that suits a
/// project tree is not the ground that suits a listing. Both were mocked up and
/// the operator liked both (2026-09-17), so both ship and a pair of toolbar
/// buttons chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugSkin {
    /// Smoked: white at a few percent over a deep slate ground. The low-light
    /// feel, without the flat black slab — surfaces separate by translucency
    /// and a hairline rather than by a step in darkness.
    #[default]
    Smoked,
    /// Frosted: white at 60–70% over a pale ground. The listing sits on paper
    /// instead of in a hole, for anyone the dark ground tires.
    Frosted,
}

/// Every colour the debugger paints with, for one skin.
///
/// One struct rather than scattered constants because a second skin is exactly
/// what scattered constants cannot have: each one would need its own `if`, and
/// the one someone forgot would be the one that stayed dark on a light ground.
#[derive(Debug, Clone, Copy)]
struct Skin {
    /// The listing's own surface — the LEFT stop of its horizontal gradient.
    /// Every panel (listing, inspector, dock) now shares this gradient
    /// (operator: "all panels must use the same background as the code panel").
    code_bg: Color32,
    /// The RIGHT stop of the editor's horizontal gradient (spec §9: azul
    /// acinzentado → grafite across the listing).
    code_bg2: Color32,
    /// Dividers and frame rules.
    line: Color32,
    /// Ordinary code.
    ink: Color32,
    /// Line numbers and other quiet chrome.
    gutter: Color32,
    /// Selection band and the ink redrawn over it.
    sel_bg: Color32,
    sel_ink: Color32,
    /// The stopped row: tint, accent bar, and the annotations that ride with it.
    cur_bg: Color32,
    cur_accent: Color32,
    cur_ink: Color32,
    /// Syntax.
    kw: Color32,
    lit: Color32,
    cmt: Color32,
    num: Color32,

    // ── Chrome ────────────────────────────────────────────────────────────
    // Everything that is not the listing. Without these the skin reached the
    // code pane and stopped: Frosted lit the listing and left the toolbar, the
    // inspector and the window itself dark around it (operator, 2026-09-17).
    /// The window behind every pane.
    window_bg: Color32,
    /// The inspector and other panels that sit on the window.
    panel_bg: Color32,
    /// The inspector's expression/value cards (spec §9: violeta-acinzentado).
    row_bg: Color32,
    /// Primary and secondary UI text.
    chrome: Color32,
    chrome_dim: Color32,
    /// A pressed toolbar button, a selected tab, and the ink on them.
    accent: Color32,
    accent_ink: Color32,
    /// A text field's own ground, and a widget under the pointer.
    field_bg: Color32,
    hover_bg: Color32,
    /// The window's ground gradient, spec §3: a 2×3 mesh — the TOP row of three
    /// stops (left/centre/right of the header band)…
    grad_top: [Color32; 3],
    /// …and the BOTTOM row (left/centre/right of the console band). The mesh
    /// runs top→bottom between them. The big decorative blooms are gone (spec
    /// §1: "Remova as ondas grandes e chamativas").
    grad_bot: [Color32; 3],
    /// The one remaining glow: a soft petrol wash low and centre (spec §1/§3).
    petrol: Color32,
}

/// The window's ground: the flat fill plus three soft blooms.
///
/// egui has no blur, so a bloom is a stack of concentric circles whose alpha
/// falls off — cheap, and at this size indistinguishable from a blurred blob.
/// They are what the panes are translucent OVER; without them "glass" is a
/// solid colour with a lighter edge.
fn paint_backdrop(painter: &egui::Painter, rect: egui::Rect, sk: Skin) {
    // Spec §3: the shell ground is a 2×3 gradient mesh — a top row (the header
    // band, three stops across) and a bottom row (the console band), the mesh
    // running top→bottom between them. No rasterised texture, and none of the
    // big decorative blooms the old ground had (spec §1: "Remova as ondas
    // grandes e chamativas").
    let cx = rect.center().x;
    let mut mesh = egui::epaint::Mesh::default();
    let v = |p: egui::Pos2, c: Color32| egui::epaint::Vertex {
        pos: p,
        uv: egui::epaint::WHITE_UV,
        color: c,
    };
    let i = mesh.vertices.len() as u32;
    mesh.vertices.push(v(rect.left_top(), sk.grad_top[0]));
    mesh.vertices.push(v(egui::pos2(cx, rect.top()), sk.grad_top[1]));
    mesh.vertices.push(v(rect.right_top(), sk.grad_top[2]));
    mesh.vertices.push(v(rect.left_bottom(), sk.grad_bot[0]));
    mesh.vertices.push(v(egui::pos2(cx, rect.bottom()), sk.grad_bot[1]));
    mesh.vertices.push(v(rect.right_bottom(), sk.grad_bot[2]));
    mesh.indices.extend_from_slice(&[
        i, i + 1, i + 4, i, i + 4, i + 3, // left cell
        i + 1, i + 2, i + 5, i + 1, i + 5, i + 4, // right cell
    ]);
    painter.add(egui::Shape::mesh(mesh));

    // A subtle texture over the ground (operator): faint diagonal grain, a
    // near-invisible lightening every 18 px, so the flat gradient reads as a
    // surface rather than a fill.
    let grain = Color32::from_rgba_unmultiplied(255, 255, 255, 4);
    let step = 18.0;
    let mut off = -rect.height();
    while off < rect.width() {
        let x = rect.left() + off;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x + rect.height(), rect.bottom())],
            egui::Stroke::new(1.0, grain),
        );
        off += step;
    }

    // Spec §1/§3: the one remaining glow — a soft petrol wash, low and centre.
    // egui has no blur, so it is a short stack of concentric circles whose alpha
    // falls off towards the edge.
    let centre = egui::pos2(cx, rect.bottom() - rect.height() * 0.05);
    let full = rect.width().max(rect.height()) * 0.34;
    const RINGS: usize = 14;
    for ring in (0..RINGS).rev() {
        let t = (ring + 1) as f32 / RINGS as f32;
        let a = (1.0 - t).powf(1.7) * 20.0;
        painter.circle_filled(
            centre,
            full * t,
            Color32::from_rgba_unmultiplied(sk.petrol.r(), sk.petrol.g(), sk.petrol.b(), a as u8),
        );
    }
}

/// The shared pane surface (operator: every panel uses the code panel's
/// background): a soft horizontal gradient, left→right, rounded at a small
/// radius, and NO border. egui cannot clip a mesh to a rounded rect, so the
/// rounded base is filled first and the gradient inset by the radius —
/// invisible at 5 px, and the corners stay cut.
fn paint_grad_pane_h(painter: &egui::Painter, rect: egui::Rect, left: Color32, right: Color32) {
    // The panel darkening is now baked per skin (Smoked's surfaces carry it;
    // Frosted inverts panel and ground instead), so this paints the colours as
    // given.
    // 10 px on every panel (operator).
    let r = egui::CornerRadius::same(10);
    painter.rect_filled(rect, r, left);
    let inner = rect.shrink(10.0);
    let mut mesh = egui::epaint::Mesh::default();
    let v = |p: egui::Pos2, c: Color32| egui::epaint::Vertex {
        pos: p,
        uv: egui::epaint::WHITE_UV,
        color: c,
    };
    let i = mesh.vertices.len() as u32;
    mesh.vertices.push(v(inner.left_top(), left));
    mesh.vertices.push(v(inner.right_top(), right));
    mesh.vertices.push(v(inner.right_bottom(), right));
    mesh.vertices.push(v(inner.left_bottom(), left));
    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// Dress the whole egui subtree in this skin.
///
/// Set once at the debugger's root rather than threaded through every widget:
/// buttons, tabs, text fields and labels all read `Ui::visuals`, so one
/// assignment is what makes the skin reach them. Painting the panes by hand and
/// leaving the widgets on the IDE's visuals is exactly how Frosted ended up as
/// a light listing inside a dark window.
fn apply_skin_visuals(sk: Skin, ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.panel_fill = sk.window_bg;
    v.window_fill = sk.window_bg;
    v.extreme_bg_color = sk.field_bg;
    // Striped rows (the inspector's variable table) take the violet-grey card
    // colour the spec asks for (§9).
    v.faint_bg_color = sk.row_bg;
    v.override_text_color = Some(sk.chrome);
    v.hyperlink_color = sk.accent;
    v.selection.bg_fill = sk.accent;
    v.selection.stroke = egui::Stroke::new(1.0, sk.accent_ink);
    v.widgets.noninteractive.bg_fill = sk.panel_bg;
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, sk.chrome_dim);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, sk.line);
    v.widgets.inactive.bg_fill = sk.panel_bg;
    v.widgets.inactive.weak_bg_fill = sk.panel_bg;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, sk.chrome);
    v.widgets.hovered.bg_fill = sk.hover_bg;
    v.widgets.hovered.weak_bg_fill = sk.hover_bg;
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, sk.chrome);
    v.widgets.active.bg_fill = sk.accent;
    v.widgets.active.weak_bg_fill = sk.accent;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, sk.accent_ink);
    v.widgets.open.bg_fill = sk.panel_bg;
    v.widgets.open.fg_stroke = egui::Stroke::new(1.0, sk.chrome);
}

impl DebugSkin {
    fn palette(self) -> Skin {
        match self {
            // The spec's token table (§3), verbatim. Desaturated navy, subtle
            // violet, controlled cyan, amber only for the paused/current-line
            // state. The editor is a horizontal navy→graphite gradient, the
            // inspector a violet panel with violet-grey cards.
            // Heavy neutral gray (operator: gray, not violet), with the panels
            // a clear step ABOVE a deep gray window so the gaps between them
            // read as gutters, not seams. The code surface is a solid mid-gray,
            // not the pale blue that came out milky.
            DebugSkin::Smoked => Skin {
                // Panels 20% darker than the ground (operator), baked in.
                code_bg: Color32::from_rgb(45, 48, 54),
                code_bg2: Color32::from_rgb(42, 45, 50),
                line: Color32::from_rgb(92, 99, 110),
                ink: Color32::from_rgb(216, 222, 230),
                gutter: Color32::from_rgb(122, 130, 142),
                sel_bg: Color32::from_rgb(58, 108, 176),
                sel_ink: Color32::WHITE,
                cur_bg: Color32::from_rgb(62, 84, 132),
                cur_accent: Color32::from_rgb(226, 166, 64), // amber #E2A640
                cur_ink: Color32::from_rgb(233, 180, 92),
                kw: Color32::from_rgb(85, 183, 255),        // blue #55B7FF
                lit: Color32::from_rgb(226, 166, 64),       // amber #E2A640
                cmt: Color32::from_rgb(111, 206, 143),      // green #6FCE8F
                num: Color32::from_rgb(150, 172, 214),      // blue-grey, not violet
                window_bg: Color32::from_rgb(35, 37, 42),   // heavy gray ground
                panel_bg: Color32::from_rgb(56, 60, 67),    // widgets blend into the panel
                row_bg: Color32::from_rgb(66, 71, 80),      // inspector cards
                chrome: Color32::from_rgb(216, 222, 230),
                chrome_dim: Color32::from_rgb(150, 162, 178),
                accent: Color32::from_rgb(85, 183, 255),    // blue #55B7FF
                accent_ink: Color32::WHITE,
                field_bg: Color32::from_rgb(46, 50, 57),
                hover_bg: Color32::from_rgb(68, 74, 84),
                grad_top: [
                    Color32::from_rgb(44, 47, 53),
                    Color32::from_rgb(37, 39, 44),
                    Color32::from_rgb(40, 42, 48),
                ],
                grad_bot: [
                    Color32::from_rgb(40, 42, 47),
                    Color32::from_rgb(43, 46, 52),
                    Color32::from_rgb(35, 37, 42),
                ],
                petrol: Color32::from_rgb(56, 72, 78), // muted gray-teal, subtle
            },
            // Ink chosen against `code_bg`, not against white: the pane is
            // translucent over a pale ground, so the darkest thing behind the
            // text is the pane itself. Every pair here clears 4.5:1 on it.
            // Frosted is the light alternative the spec does not target; kept
            // working, with the new fields given light equivalents.
            DebugSkin::Frosted => Skin {
                // Frosted inverts panel and ground (operator): near-white paper
                // panels sit on a light-grey background.
                code_bg: Color32::from_rgb(250, 251, 253),
                code_bg2: Color32::from_rgb(245, 248, 252),
                line: Color32::from_rgb(150, 163, 184),
                ink: Color32::from_rgb(31, 41, 55),
                gutter: Color32::from_rgb(105, 116, 136),
                sel_bg: Color32::from_rgb(29, 78, 216),
                sel_ink: Color32::WHITE,
                cur_bg: Color32::from_rgb(214, 226, 247),
                cur_accent: Color32::from_rgb(194, 105, 8),
                cur_ink: Color32::from_rgb(124, 74, 12),
                kw: Color32::from_rgb(29, 78, 216),
                lit: Color32::from_rgb(166, 76, 9),
                cmt: Color32::from_rgb(58, 110, 66),
                num: Color32::from_rgb(109, 40, 217),
                window_bg: Color32::from_rgb(200, 208, 221),
                panel_bg: Color32::from_rgb(250, 251, 253),
                row_bg: Color32::from_rgb(233, 237, 244),
                chrome: Color32::from_rgb(38, 50, 66),
                // 4.7:1 on the dock, the panel and the sheet alike — the dim
                // ink has to clear the floor on the LIGHTEST surface it lands
                // on, which is the code sheet, not the window.
                chrome_dim: Color32::from_rgb(98, 112, 132),
                accent: Color32::from_rgb(29, 78, 216),
                accent_ink: Color32::WHITE,
                field_bg: Color32::from_rgb(238, 242, 248),
                hover_bg: Color32::from_rgb(222, 228, 238),
                grad_top: [
                    Color32::from_rgb(206, 214, 226),
                    Color32::from_rgb(212, 217, 226),
                    Color32::from_rgb(208, 214, 225),
                ],
                grad_bot: [
                    Color32::from_rgb(210, 216, 226),
                    Color32::from_rgb(204, 211, 223),
                    Color32::from_rgb(199, 206, 219),
                ],
                petrol: Color32::from_rgb(168, 198, 191),
            },
        }
    }
}

/// The code pane's own face — a step DARKER than the panes beside and below it.
///
/// The listing is the thing being read; the watches, the stack and the console
/// are apparatus around it. Giving the reading surface its own darker ground
/// separates the two without a border doing the work, and it is what the rest
/// of the window's dark palette already implies.
const CODE_BG: Color32 = Color32::from_rgb(10, 17, 24);

/// The dock's face: between [`CODE_BG`] and the window, so the three surfaces
/// read as a hierarchy rather than as one flat sheet.
const DOCK_BG: Color32 = Color32::from_rgb(16, 26, 34);

/// Dash and gap for every divider between two panes.
const DASH: f32 = 4.0;
const DASH_GAP: f32 = 3.0;

/// The band behind selected code.
///
/// A strong blue, not a tint: the listing's own ground is very dark, and a
/// muted band over it read as "slightly different dark" rather than as a
/// selection (operator, 2026-09-17). Paired with [`SEL_INK`], which is redrawn
/// over the band so the selected characters are the high-contrast pair — white
/// on this blue is about 8:1 — instead of syntax colours chosen for a dark
/// ground and left to fend for themselves on a light one.
const SEL_BG: Color32 = Color32::from_rgb(29, 96, 176);

/// The selected characters themselves, redrawn over [`SEL_BG`].
const SEL_INK: Color32 = Color32::WHITE;

/// The console prompt's row: the field, its spacing, and a little air.
///
/// Named because two places must agree on it — the dock's scroll area, which
/// stops short of it, and the panel, which is tall enough to hold it.
const PROMPT_H: f32 = 52.0;

/// Air on every side of every pane.
///
/// Without it the right-hand pane's tabs, filter box and table all began ON the
/// dividing line, so it read as a border drawn around that pane instead of as
/// the division between two (operator, 2026-09-17).
///
/// **10, by operator instruction the same day** — "the code pane should have
/// consistent 10x padding on all sides (bottom is touching the bottom pane)" —
/// and deliberately the same number as `PANE_RADIUS`, so a pane's gap from its
/// neighbour matches the curve of its own corner.
const PANE_PAD: f32 = 10.0;

/// Toolbar button footprint, and the icon inside it.
// +25% (operator): 30×28 → 38×35, icon 16 → 20.
const TB_BTN: Vec2 = Vec2::new(38.0, 35.0);
const TB_ICON: f32 = 20.0;

/// Longest value a hover tooltip prints before it is cut.
///
/// 100 by operator instruction (2026-09-17). It was 18, which is shorter than
/// most of the strings worth hovering: a caption, a path or a SQL fragment was
/// cut to a stub that answered nothing. The cap still exists because a DataGrid
/// caption can run to hundreds of characters and a tooltip that tall covers the
/// code it was asked about.
const TIP_VALUE_CHARS: usize = 100;

/// Longest quoted literal the listing DRAWS before eliding its middle.
///
/// Display only — see [`elide_long_literals`].
const LITERAL_DRAW_CHARS: usize = 60;

/// The data items a source line names, with their current values.
///
/// Restrained on purpose (spec: "show restrained inline values only for
/// relevant or recently changed data items"): only the line under the
/// execution pointer, and at most two items. An annotation on every line turns
/// a listing into a wall, and the whole value of the hint is that it is rare
/// enough to notice.
///
/// Matched by scanning the line for COBOL words that are actually declared,
/// rather than by parsing it: a data item is a word the environment knows, and
/// the parse would have to be redone here for no extra certainty.
fn inline_values(line: &str, vars: &[VarSnapshot], limit: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let upper = line.to_ascii_uppercase();
    for v in vars {
        if out.len() >= limit {
            break;
        }
        // Subscripted slots (`WS-ROW(3)`) are storage keys, not words in the
        // text; their parent is what the line names.
        let name = v.name.split('(').next().unwrap_or(&v.name);
        if name.len() < 3 || out.iter().any(|(n, _)| n == name) {
            continue;
        }
        let Some(at) = upper.find(name) else {
            continue;
        };
        // A whole word: `WS-N` must not match inside `WS-NAME`.
        let before_ok = at == 0
            || !upper.as_bytes()[at - 1].is_ascii_alphanumeric() && upper.as_bytes()[at - 1] != b'-';
        let after = at + name.len();
        let after_ok = after >= upper.len()
            || !upper.as_bytes()[after].is_ascii_alphanumeric() && upper.as_bytes()[after] != b'-';
        if before_ok && after_ok {
            let value = v.value.trim();
            if !value.is_empty() {
                out.push((name.to_owned(), value.to_owned()));
            }
        }
    }
    out
}

// ── Hover values ──────────────────────────────────────────────────────────────

/// The COBOL word under character `index` of `line`, or `None` between words.
///
/// A word here is what the developer would double-click: letters, digits and
/// the hyphens COBOL builds names from, plus the `::` that joins a control to
/// one of its properties — `Gauge-1::Value` is one thing to ask about, not two.
fn word_at(line: &str, index: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }
    // A cursor at the very end of a word still means that word.
    let at = index.min(chars.len().saturating_sub(1));
    let part = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == ':';
    if !part(chars[at]) {
        return None;
    }
    let mut start = at;
    while start > 0 && part(chars[start - 1]) {
        start -= 1;
    }
    let mut end = at + 1;
    while end < chars.len() && part(chars[end]) {
        end += 1;
    }
    let word: String = chars[start..end].iter().collect();
    // A lone separator run is not a name.
    let word = word.trim_matches(|c| c == '-' || c == ':').to_string();
    (!word.is_empty() && word.chars().any(|c| c.is_ascii_alphabetic())).then_some(word)
}

/// What the debuggee currently holds for `word`, if it holds anything.
///
/// Tried whole first, then cut back to the part before `::` — so hovering
/// `Gauge-1::Value` answers about the property when the runtime reports one and
/// about the control when it does not.
fn lookup_value<'a>(word: &str, vars: &'a [VarSnapshot]) -> Option<(&'a str, &'a str)> {
    let candidates = [word, word.split("::").next().unwrap_or(word)];
    for want in candidates {
        if want.is_empty() {
            continue;
        }
        for v in vars {
            let name = v.name.split('(').next().unwrap_or(&v.name);
            if name.eq_ignore_ascii_case(want) && !v.value.trim().is_empty() {
                return Some((name, v.value.trim()));
            }
        }
    }
    None
}

/// `name = value`, with a long value cut so the tooltip stays one short line.
///
/// Counted in **characters**: a value holding accented text has more bytes than
/// columns, and cutting by byte would split one in half.
fn hover_tip(name: &str, value: &str) -> String {
    if value.chars().count() > TIP_VALUE_CHARS {
        let head: String = value.chars().take(TIP_VALUE_CHARS).collect();
        format!("{name} = {head}...")
    } else {
        format!("{name} = {value}")
    }
}

/// Shorten over-long quoted literals **for drawing only**.
///
/// Generated forms carry documentation text in `VALUE` clauses and `MOVE`
/// statements — a DataGrid's own caption runs to several hundred characters —
/// and one such line made the whole listing scroll sideways. Every other line
/// then had to be read through a horizontal offset it did not need.
///
/// The value is untouched: this rewrites the string the listing PAINTS, never
/// the source, never the program. The elision is marked with `…` so nobody
/// mistakes the drawn text for the whole literal, and the closing quote is kept
/// so the line still reads as COBOL.
///
/// Counted in characters rather than bytes — these literals are exactly the
/// ones holding accented prose, and cutting mid-character would corrupt it.
fn elide_long_literals(line: &str, max: usize) -> std::borrow::Cow<'_, str> {
    if !line.contains('"') && !line.contains('\'') {
        return std::borrow::Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    let mut changed = false;
    while let Some(c) = chars.next() {
        if c != '"' && c != '\'' {
            out.push(c);
            continue;
        }
        // Inside a literal: take it up to its closing quote, or to end of line
        // for the unterminated one a half-typed source can hold.
        let quote = c;
        let mut body = String::new();
        let mut closed = false;
        for d in chars.by_ref() {
            if d == quote {
                closed = true;
                break;
            }
            body.push(d);
        }
        out.push(quote);
        if body.chars().count() > max {
            let head: String = body.chars().take(max).collect();
            out.push_str(&head);
            out.push('…');
            changed = true;
        } else {
            out.push_str(&body);
        }
        if closed {
            out.push(quote);
        }
    }
    if changed {
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(line)
    }
}

/// The span of the "word" under `idx`, as a character range.
///
/// A word here is what the operator asked for: a run bounded by spaces, by the
/// start or end of the line, or by a period (2026-09-17). That is coarser than
/// a COBOL identifier on purpose — double-clicking `WS-LINE` inside
/// `TRIM(WS-LINE))` takes the whole `TRIM(WS-LINE))`, because that is the thing
/// sitting between two spaces — and it is deliberately NOT the rule
/// [`word_at`] uses for the hover tooltip, which must find the data ITEM under
/// the pointer and nothing around it.
///
/// `None` when the character under the pointer is itself a separator: there is
/// no word there to take.
fn word_span_at(line: &str, idx: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }
    // A double-click just past the end takes the last word, which is what the
    // pointer landing in the blank tail of a line means.
    let at = idx.min(chars.len() - 1);
    let sep = |c: char| c.is_whitespace() || c == '.';
    if sep(chars[at]) {
        return None;
    }
    let mut from = at;
    while from > 0 && !sep(chars[from - 1]) {
        from -= 1;
    }
    let mut to = at + 1;
    while to < chars.len() && !sep(chars[to]) {
        to += 1;
    }
    Some((from, to))
}

/// A dashed divider between two panes, vertical.
fn dashed_vline(painter: &egui::Painter, x: f32, y: egui::Rangef, color: Color32) {
    painter.extend(egui::Shape::dashed_line(
        &[egui::pos2(x, y.min), egui::pos2(x, y.max)],
        egui::Stroke::new(1.0, color),
        DASH,
        DASH_GAP,
    ));
}

/// A dashed divider between two panes, horizontal.
fn dashed_hline(painter: &egui::Painter, x: egui::Rangef, y: f32, color: Color32) {
    painter.extend(egui::Shape::dashed_line(
        &[egui::pos2(x.min, y), egui::pos2(x.max, y)],
        egui::Stroke::new(1.0, color),
        DASH,
        DASH_GAP,
    ));
}

// ── Toolbar buttons ───────────────────────────────────────────────────────────

/// One icon-only toolbar button, drawn from the platform's own icon catalogue.
///
/// The label lives in the tooltip. A debugger toolbar is pressed by muscle
/// memory, and the words cost more width than they buy once the shapes are
/// learned — which is the whole reason the strip had run out of room.
fn tool_icon(
    ui: &mut egui::Ui,
    icon: &str,
    enabled: bool,
    active: bool,
    tint: Option<Color32>,
    tip: impl Into<String>,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(TB_BTN, sense);

    let visuals = ui.visuals();
    // The mockup gives every toolbar button the same lozenge: a translucent
    // wash of the skin's ink over the strip, a hairline of the same, radius 8.
    // Derived from the ink (near-white on Smoked, near-slate on Frosted) so the
    // one rule reads on both grounds.
    // High contrast (operator): a clearly-filled lozenge with a firm border and
    // a bright icon, not a faint wash.
    let base = visuals.text_color();
    let wash = |a: u8| Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), a);
    let (bg, fg, border) = if !enabled {
        (wash(14), visuals.weak_text_color(), wash(34))
    } else if active {
        (
            visuals.selection.bg_fill,
            visuals.strong_text_color(),
            visuals.selection.bg_fill,
        )
    } else if resp.hovered() {
        (wash(120), visuals.strong_text_color(), wash(150))
    } else {
        (wash(64), visuals.strong_text_color(), wash(96))
    };
    let fg = tint.filter(|_| enabled).unwrap_or(fg);

    let r = egui::CornerRadius::same(9);
    ui.painter().rect_filled(rect, r, bg);
    ui.painter()
        .rect_stroke(rect, r, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    let icon_rect = egui::Rect::from_center_size(rect.center(), Vec2::splat(TB_ICON));
    cobolt_forms::icons::draw_menu_icon(ui.painter(), icon_rect, icon, fg);
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip.into())
}

/// A pill tab — the mockup's tab: a translucent-blue lozenge with blue ink when
/// selected, ghost text otherwise. egui's own `selectable_value` draws an
/// underline-style highlight instead, which is not this window's look.
fn pill_tab(ui: &mut egui::Ui, selected: bool, label: &str, sk: Skin) -> egui::Response {
    let ink = if selected { sk.kw } else { sk.chrome_dim };
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(15.0), ink);
    let pad = egui::vec2(12.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + pad * 2.0, egui::Sense::click());
    let r = egui::CornerRadius::same(9);
    let k = sk.kw;
    if selected {
        ui.painter().rect_filled(
            rect,
            r,
            Color32::from_rgba_unmultiplied(k.r(), k.g(), k.b(), 40),
        );
        ui.painter().rect_stroke(
            rect,
            r,
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(k.r(), k.g(), k.b(), 87)),
            egui::StrokeKind::Inside,
        );
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, r, Color32::from_rgba_unmultiplied(255, 255, 255, 14));
    }
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, ink);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// The run-state pill, as the mockup draws it: a rounded lozenge washed in its
/// accent, a dot, and ink that is the accent lifted toward white. Amber while
/// stopped, green while running.
fn status_pill(ui: &mut egui::Ui, text: &str, accent: Color32) {
    let a = |x: u8| Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), x);
    let lift = |c: u8| c + ((255 - c) as f32 * 0.42) as u8;
    let ink = Color32::from_rgb(lift(accent.r()), lift(accent.g()), lift(accent.b()));
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), egui::FontId::proportional(14.0), ink);
    let pad = egui::vec2(11.0, 4.0);
    let dot_w = 13.0;
    let size = Vec2::new(galley.size().x + dot_w + pad.x * 2.0, galley.size().y + pad.y * 2.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let r = egui::CornerRadius::same((size.y * 0.5) as u8);
    ui.painter().rect_filled(rect, r, a(46));
    ui.painter().rect_stroke(
        rect,
        r,
        egui::Stroke::new(1.0, a(115)),
        egui::StrokeKind::Inside,
    );
    let x = rect.left() + pad.x;
    ui.painter()
        .circle_filled(egui::pos2(x + 3.0, rect.center().y), 3.0, accent);
    ui.painter().galley(
        egui::pos2(x + dot_w, rect.center().y - galley.size().y * 0.5),
        galley,
        ink,
    );
}

/// A folded-run chip, as the mockup draws it: a translucent lozenge, indented
/// past the gutter, carrying a chevron and the run's name. Replaces the plain
/// grey label the runs used to fold to.
fn fold_chip(ui: &mut egui::Ui, tip: &str, sk: Skin) -> egui::Response {
    const INDENT: f32 = 66.0;
    // A compact down-arrow, indented past the gutter (operator: an arrow, not a
    // labelled pill). The run's description is the arrow's tooltip, and a click
    // still expands the run.
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(INDENT + 24.0, 22.0), egui::Sense::click());
    let cx = rect.left() + INDENT + 12.0;
    let cy = rect.center().y;
    let (half, depth) = (8.0, 9.0);
    let col = if resp.hovered() {
        Color32::from_rgb(150, 205, 255)
    } else {
        sk.kw
    };
    // Downward triangle ▼, painter-drawn (never a system glyph).
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(cx - half, cy - depth * 0.5),
            egui::pos2(cx + half, cy - depth * 0.5),
            egui::pos2(cx, cy + depth * 0.5),
        ],
        col,
        egui::Stroke::NONE,
    ));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(tip)
}

/// A full-width rule between two frames.
///
/// The theme's own separator is drawn from the non-interactive widget stroke,
/// which on this dark surface is close enough to the background that the panes
/// looked like one undivided area.
fn frame_rule(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    dashed_hline(ui.painter(), rect.x_range(), rect.center().y, FRAME_LINE);
    ui.add_space(3.0);
}

// ── DebuggerPanel ─────────────────────────────────────────────────────────────

/// State for the floating debugger window.
pub struct DebuggerPanel {
    // Runtime state
    var_filter: String,
    vars: Vec<VarSnapshot>,
    current_para: String,
    current_line: u32,
    is_paused: bool,
    /// Frames to keep repainting after a session opens. The debugger is its own
    /// immediate viewport; its first frame lays out against a not-yet-final
    /// window size, and egui — being reactive — would leave that first layout on
    /// screen (the bottom gutter missing) until an event forced a fresh pass.
    /// A short warm-up burst gives it those passes automatically.
    warmup_frames: u8,
    /// Why the program stopped, for the session strip. `None` while running —
    /// the strip then says Running rather than inventing a reason.
    stop_reason: Option<StopReason>,
    /// The logical COBOL call stack at the current stop, innermost first.
    frames: Vec<DebugFrame>,
    /// Which frame the inspector and watches evaluate against. Always a valid
    /// index into `frames`, or 0 when there is no stack.
    selected_frame: usize,
    pub pending_output: Vec<RunMsg>,

    // Source viewer
    source_lines: Vec<String>,
    source_path: String,
    breakpoints: HashSet<u32>,
    /// A line the developer asked to clear from the Breakpoints list this frame
    /// (its ✕ button). Drained by `split_body` into a `ToggleBreakpoint`, the
    /// same action a gutter click raises, so removal and the gutter share one
    /// path back to the editor's breakpoint set.
    bp_remove_request: Option<u32>,
    last_scrolled_line: u32,
    force_center_current: bool,

    // UI state
    active_tab: Tab,
    selected_var: Option<VarSnapshot>,
    /// "Only my code" — stepping crosses IDE-generated scaffolding instead of
    /// walking it. **On by default**: a form's generated `.cbl` is mostly the
    /// event loop and its plumbing, which is an internal construct the
    /// developer did not write and has no reason to single-step through.
    pub only_user_code: bool,
    animate: bool,
    animate_speed_lps: f32,
    last_animate_step: Option<Instant>,
    /// The line the developer last clicked in the source pane — the target for
    /// Run to Cursor. `None` until they pick one, which is why the button is
    /// disabled rather than guessing the current line.
    cursor_line: Option<u32>,
    // ── The data inspector (lazy) ────────────────────────────────────────
    /// Scopes of the selected frame, as answered at the current stop.
    scopes: Vec<ScopeInfo>,
    /// Rows already fetched, keyed by the handle they were fetched under.
    /// Cleared at every stop, because a handle never outlives its frame.
    rows: HashMap<i64, Vec<VarInfo>>,
    /// Handles the developer has opened.
    open: HashSet<i64>,
    /// Handles asked for but not yet answered, so one expand does not fire a
    /// query on every frame while the answer is in flight.
    inflight: HashSet<i64>,
    next_query_id: u64,
    /// Queries built while rendering, drained into `DebugAction`s afterwards —
    /// the tree is drawn inside a closure and cannot return an action itself.
    pending_queries: Vec<DebugQuery>,
    /// Queries that already carry the id their answer will be matched by — a
    /// watch or the console prompt. `pending_queries` is numbered on the way
    /// out; these cannot be, because the id is the correlation key.
    pending_ident_queries: Vec<(u64, DebugQuery)>,
    /// Watch expressions, in the order the developer added them. Persisted per
    /// project by the host.
    pub watches: Vec<Watch>,
    watch_input: String,
    /// Which watch each outstanding Evaluate belongs to, by query id. Watches
    /// are evaluated together at every stop, so several are in flight at once
    /// and the answers cannot be matched by "the only one outstanding".
    watch_pending: HashMap<u64, usize>,
    /// The console prompt's own outstanding evaluation.
    console_pending: Option<u64>,
    console_input: String,
    /// Entries typed at the prompt, newest last; ↑/↓ walk it.
    console_history: Vec<String>,
    history_pos: Option<usize>,
    dock_tab: DockTab,
    /// Height of the investigation dock, in points.
    ///
    /// An explicit stored number, not a fraction of what is available and never
    /// derived from the dock's own content — that is the feedback loop that
    /// makes a pane grow every frame until it fills the window. It changes only
    /// when the developer drags the grip.
    dock_height: f32,
    /// Where a code selection started and where it is now, each as
    /// `(0-based source line, character index in that line)`.
    ///
    /// The listing is read-only and stays read-only: this exists so the
    /// developer can take a copy of what they are looking at — a paragraph
    /// name, a PICTURE, a whole handler — without retyping it (operator,
    /// 2026-09-17).
    sel_anchor: Option<(usize, usize)>,
    sel_cursor: Option<(usize, usize)>,
    /// Which material this window is made of. Debugger-local: the IDE keeps
    /// its own theme, and switching here changes nothing outside this window.
    pub skin: DebugSkin,

    /// The find bar: what is typed, every hit, and which one is current.
    ///
    /// Find only — there is no replace and there will not be: the listing is
    /// generated code the developer cannot edit from here, so a replace box
    /// would be a control that refuses every use of it.
    find_query: String,
    /// Every match, as `(0-based line, char from, char to)`, in reading order.
    find_hits: Vec<(usize, usize, usize)>,
    /// Which hit is current. Meaningless when `find_hits` is empty.
    find_at: usize,
    /// Take keyboard focus on the frame the bar opens, so the developer types
    /// into it rather than at the listing.
    find_focus: bool,
    /// The query the hit list was built from, so it is rebuilt only when the
    /// text or the source actually changed.
    find_built_for: String,
    /// A line the find bar wants brought into view on the next frame.
    current_find_line: Option<u32>,

    /// What this debugger last copied. It is what the Paste button offers.
    ///
    /// egui cannot READ the system clipboard — it only writes to it and
    /// delivers a paste the window manager hands over — so the button is armed
    /// by our own copy rather than by whatever else is on the clipboard. That
    /// is exactly the journey it exists for: select a name in the listing, copy
    /// it, paste it into a watch expression or the variable filter.
    copied: Option<String>,
    dock: Vec<DockLine>,
    session_started: Option<Instant>,
    /// The wall-clock time at session start, captured next to `session_started`
    /// so a dock line's absolute time is `this + at_ms` — the local HH:MM:SS:mmm
    /// the investigation dock shows. `Instant` alone (monotonic) cannot name a
    /// clock time; this supplies the anchor, and the two are read together.
    session_started_wall: Option<chrono::DateTime<chrono::Local>>,
    /// Which row is being edited, and the text so far.
    editing: Option<(i64, String, String)>,
    /// The last refusal from the debuggee — a failed edit or a stale handle.
    inspect_error: Option<String>,
    /// Runs of lines the empty-block filter folds away, recomputed only when
    /// the source changes.
    hidden: Vec<HiddenRun>,
    /// Runs the developer has opened by clicking their marker, keyed by the
    /// run's first line. Nothing is ever destroyed — a fold is one click from
    /// giving the code back.
    expanded_runs: HashSet<u32>,
    /// Fold away divisions, sections and paragraphs with no executable
    /// statement. A VIEW filter only: real line numbers are preserved and
    /// stepping is untouched (operator ruling, 2026-09-02).
    hide_empty_blocks: bool,
    /// The code listing's font size, in points. The developer changes it with
    /// the toolbar's A−/A+ controls; the line height and gutter follow it.
    code_font_pt: f32,
    /// Fold the `*> <NAME>` regions codegen marks. On by default: the generated
    /// scaffolding is assumed to work, and scrolling past it to reach a handler
    /// is the developer's most common complaint about the pane.
    hide_generated: bool,
}

impl Default for DebuggerPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DebuggerPanel {
    pub fn new() -> Self {
        Self {
            var_filter: String::new(),
            vars: Vec::new(),
            current_para: String::new(),
            current_line: 0,
            is_paused: false,
            warmup_frames: 0,
            code_font_pt: 12.0,
            stop_reason: None,
            frames: Vec::new(),
            selected_frame: 0,
            pending_output: Vec::new(),
            source_lines: Vec::new(),
            source_path: String::new(),
            breakpoints: HashSet::new(),
            bp_remove_request: None,
            session_started_wall: None,
            last_scrolled_line: 0,
            force_center_current: false,
            active_tab: Tab::default(),
            selected_var: None,
            only_user_code: true,
            animate: false,
            animate_speed_lps: 4.0,
            last_animate_step: None,
            scopes: Vec::new(),
            rows: HashMap::new(),
            open: HashSet::new(),
            inflight: HashSet::new(),
            next_query_id: 1,
            pending_queries: Vec::new(),
            pending_ident_queries: Vec::new(),
            watches: Vec::new(),
            watch_input: String::new(),
            watch_pending: HashMap::new(),
            console_pending: None,
            console_input: String::new(),
            console_history: Vec::new(),
            history_pos: None,
            dock_tab: DockTab::default(),
            dock_height: 170.0,
            sel_anchor: None,
            sel_cursor: None,
            skin: DebugSkin::default(),
            find_query: String::new(),
            find_hits: Vec::new(),
            find_at: 0,
            find_focus: false,
            find_built_for: String::new(),
            current_find_line: None,
            copied: None,
            dock: Vec::new(),
            session_started: None,
            editing: None,
            inspect_error: None,
            cursor_line: None,
            hidden: Vec::new(),
            expanded_runs: HashSet::new(),
            hide_empty_blocks: true,
            hide_generated: true,
        }
    }

    /// Reset all runtime state (call when starting a new session).
    pub fn reset(&mut self) {
        self.vars.clear();
        self.current_para.clear();
        self.current_line = 0;
        self.is_paused = false;
        // Repaint the first handful of frames so the layout settles against the
        // real window size instead of the placeholder the first frame sees.
        self.warmup_frames = 6;
        self.stop_reason = None;
        self.frames.clear();
        self.selected_frame = 0;
        self.pending_output.clear();
        self.last_scrolled_line = 0;
        self.force_center_current = false;
        self.animate = false;
        self.last_animate_step = None;
        self.cursor_line = None;
    }

    /// Supply the COBOL source text and initial breakpoint set at session start.
    pub fn set_source(&mut self, path: String, source: &str, bps: &HashSet<u32>) {
        // A new session: the dock's clock starts here. `session_started` is the
        // monotonic base every `at_ms` measures from; `session_started_wall` is
        // the matching wall-clock instant, so a line's absolute local time is
        // `session_started_wall + at_ms`.
        self.session_started = Some(Instant::now());
        self.session_started_wall = Some(chrono::Local::now());
        self.dock.clear();
        self.source_path = path;
        self.source_lines = source.lines().map(|l| l.to_owned()).collect();
        self.hidden = folds(&self.source_lines, self.hide_empty_blocks, self.hide_generated);
        self.expanded_runs.clear();
        self.breakpoints = bps.clone();
    }

    /// Sync the live breakpoint set from the editor gutter.
    pub fn set_breakpoints(&mut self, bps: &HashSet<u32>) {
        self.breakpoints = bps.clone();
    }

    /// Replace the watch list — the project's saved expressions, on open.
    pub fn set_watches(&mut self, expressions: &[String]) {
        self.watches = expressions
            .iter()
            .map(|e| Watch {
                expression: e.clone(),
                value: None,
                error: None,
            })
            .collect();
    }

    /// The watch expressions, for saving. Values are deliberately not saved:
    /// a value belongs to a stop, and restoring one next session would show a
    /// reading from a program run that has ended.
    pub fn watch_expressions(&self) -> Vec<String> {
        self.watches.iter().map(|w| w.expression.clone()).collect()
    }

    /// Has the watch list changed since `saved`? The host polls this rather than
    /// writing `cobolt.toml` on every frame.
    pub fn watches_differ_from(&self, saved: &[String]) -> bool {
        self.watches.len() != saved.len()
            || self
                .watches
                .iter()
                .zip(saved)
                .any(|(w, s)| w.expression != *s)
    }

    /// Take the queries the last frame's rendering queued.
    ///
    /// Drained by the host, which turns each into a `DebugAction` — the tree is
    /// painted inside a closure and cannot dispatch on its own.
    pub fn take_queries(&mut self) -> Vec<(u64, DebugQuery)> {
        let mut out: Vec<(u64, DebugQuery)> = self.pending_ident_queries.drain(..).collect();
        out.extend(self.pending_queries.drain(..).map(|q| {
            let id = self.next_query_id;
            self.next_query_id += 1;
            (id, q)
        }));
        out
    }

    /// The file this session is showing — the key its breakpoints are stored
    /// under in the editor. Empty before a session starts.
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Apply one interpreter event to the panel state. Shared by the in-IDE
    /// `DebugRunner` path and the remote (`rcrun run-form --debug`) path.
    pub fn apply_event(&mut self, ev: DebugEvent) {
        match ev {
            // The stop's *reason* and the logical stack. It arrives just before
            // the `Paused` snapshot below, so both are applied for one stop.
            DebugEvent::Stopped {
                line,
                paragraph,
                reason,
                frames,
                ..
            } => {
                self.is_paused = true;
                self.current_line = line;
                self.current_para = paragraph;
                // Animate steps on a timer; a breakpoint is the developer
                // saying "stop here", and it must win over the timer. The
                // toggle goes OFF, so the program waits for their next press
                // — it used to step straight through the breakpoint on the
                // next tick (operator, 2026-09-19). Both the interpreter's
                // verdict and the panel's own set are consulted: a stop that
                // was reported as a step but sits on a line the developer
                // has marked is a breakpoint to them.
                if self.animate
                    && (matches!(reason, StopReason::Breakpoint(_))
                        || (line > 0 && self.breakpoints.contains(&line)))
                {
                    self.animate = false;
                    self.last_animate_step = None;
                }
                self.stop_reason = Some(reason);
                self.frames = frames;
                // Every handle issued at the previous stop is now stale, so the
                // cache goes with them. What the developer had OPEN is kept:
                // re-expanding the same rows by hand at every step would make
                // single-stepping unusable.
                self.scopes.clear();
                self.rows.clear();
                self.inflight.clear();
                self.editing = None;
                self.inspect_error = None;
                self.watch_pending.clear();
                for w in &mut self.watches {
                    w.value = None;
                    w.error = None;
                }
                // A new stop is a new stack: anything the developer had
                // selected belonged to frames that may no longer exist.
                self.selected_frame = 0;
                self.force_center_current = true;
            }
            DebugEvent::Paused {
                line,
                paragraph,
                vars,
                ..
            } => {
                self.is_paused = true;
                self.current_line = line;
                self.current_para = paragraph;
                self.vars = vars;
                if let Some(selected) = self.selected_var.as_ref() {
                    self.selected_var = self
                        .vars
                        .iter()
                        .find(|v| {
                            v.name == selected.name
                                && v.scope == selected.scope
                                && v.origin == selected.origin
                        })
                        .cloned();
                }
                self.force_center_current = true;
            }
            DebugEvent::Output { text, channel } => {
                let at_ms = self
                    .session_started
                    .map(|t| t.elapsed().as_millis() as u64)
                    .unwrap_or(0);
                self.dock.push(DockLine {
                    channel,
                    text,
                    at_ms,
                });
                // Bounded: a logpoint in a tight loop would otherwise grow the
                // dock until the IDE is the thing that stops responding.
                const MAX_DOCK_LINES: usize = 5000;
                if self.dock.len() > MAX_DOCK_LINES {
                    self.dock.drain(..self.dock.len() - MAX_DOCK_LINES);
                }
            }
            DebugEvent::Answer { id, answer } => match answer {
                DebugAnswer::Scopes(scopes) => {
                    for sc in &scopes {
                        self.inflight.remove(&sc.reference);
                    }
                    self.scopes = scopes;
                }
                DebugAnswer::Variables(rows) => {
                    // The answer does not name the handle it belongs to, so the
                    // single outstanding request is matched by what is in
                    // flight. Requests are issued one at a time for exactly
                    // this reason.
                    if let Some(&r) = self.inflight.iter().next() {
                        self.inflight.remove(&r);
                        self.rows.insert(r, rows);
                    }
                }
                DebugAnswer::Set { .. } => {
                    // The written value is read back by refetching the row's
                    // parent, so the tree shows what the PROGRAM sees rather
                    // than what was typed.
                    self.editing = None;
                    self.rows.clear();
                    self.inflight.clear();
                }
                DebugAnswer::Evaluated { result, pic } => {
                    if let Some(i) = self.watch_pending.remove(&id) {
                        if let Some(w) = self.watches.get_mut(i) {
                            w.value = Some(result);
                            w.error = None;
                        }
                    } else if self.console_pending == Some(id) {
                        self.console_pending = None;
                        let shown = if pic.is_empty() {
                            result
                        } else {
                            format!("{result}    {pic}")
                        };
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Console,
                            text: shown,
                            at_ms,
                        });
                    }
                }
                DebugAnswer::Error(msg) => {
                    if let Some(i) = self.watch_pending.remove(&id) {
                        if let Some(w) = self.watches.get_mut(i) {
                            w.value = None;
                            w.error = Some(msg);
                        }
                    } else if self.console_pending == Some(id) {
                        self.console_pending = None;
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Problems,
                            text: msg,
                            at_ms,
                        });
                    } else {
                        self.inflight.clear();
                        self.inspect_error = Some(msg);
                    }
                }
            },
            DebugEvent::Resumed => {
                self.is_paused = false;
                self.stop_reason = None;
            }
            DebugEvent::Finished => {
                self.is_paused = false;
                self.stop_reason = None;
                self.scopes.clear();
                self.rows.clear();
                self.open.clear();
                self.inflight.clear();
                self.frames.clear();
                self.selected_frame = 0;
                self.vars.clear();
            }
        }
    }

    /// Process events from `DebugRunner`; returns `true` if the UI needs to repaint.
    pub fn process(&mut self, runner: &mut DebugRunner) -> bool {
        let mut dirty = false;
        for ev in runner.drain_events() {
            dirty = true;
            self.apply_event(ev);
        }
        for msg in runner.drain_run() {
            dirty = true;
            self.pending_output.push(msg);
        }
        dirty
    }

    /// Title for the standalone debugger OS window.
    pub fn window_title(&self) -> String {
        let debug_name = std::path::Path::new(&self.source_path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("generated code");
        format!("🐞 Debugging {debug_name} generated code")
    }

    /// Render as the full content of a dedicated viewport — a standalone OS
    /// window the user can place next to the running form. The OS window is
    /// the sole size authority (the user drags its edges); content just fills
    /// it, so there is no self-inflation path.
    ///
    /// Returns a [`DebugAction`] when the user presses a control or shortcut.
    pub fn show_viewport_body(&mut self, panel_ui: &mut egui::Ui, tr: &Tr) -> Option<DebugAction> {
        let ctx = panel_ui.ctx().clone();
        let ctx = &ctx;

        // Warm-up: force a few extra passes right after the window opens, so the
        // panels re-lay-out once the viewport reports its real size and the
        // bottom gutter is not left missing until the developer resizes.
        if self.warmup_frames > 0 {
            self.warmup_frames -= 1;
            ctx.request_repaint();
        }

        let mut action: Option<DebugAction> = None;

        // Global keyboard shortcuts — active even when the window is not focused.
        if self.is_paused {
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F10)) {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
            }
            // Shift+F11 FIRST: `key_pressed` ignores extra modifiers, so a
            // plain-F11 test would also swallow the shifted chord and Step Out
            // would never fire.
            if action.is_none()
                && ctx.input(|i| i.key_pressed(Key::F11) && i.modifiers.shift)
                && self.is_paused
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11) && !i.modifiers.shift) {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
            }
        }

        if action.is_none() {
            action = self.maybe_animate_step(ctx);
        }

        self.copy_shortcut(ctx);
        self.find_shortcuts(ctx);

        let need_scroll = self.should_center_current_line();

        let sk = self.skin.palette();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(panel_ui, |ui| {
            paint_backdrop(ui.painter(), ui.max_rect(), sk);
            apply_skin_visuals(sk, ui);
            self.status_row(ui, tr);
            if let Some(a) = self.toolbar(ui, tr) {
                action = Some(a);
            }
            frame_rule(ui);
            if let Some(line) = self.split_body(ui, tr, need_scroll) {
                action = Some(DebugAction::ToggleBreakpoint(line));
            }
        });

        self.variable_value_window(ctx);

        if need_scroll {
            self.last_scrolled_line = self.current_line;
            self.force_center_current = false;
        }

        action
    }

    /// Render the floating debugger window.
    ///
    /// Returns a [`DebugAction`] when the user presses a control or keyboard shortcut.
    ///
    /// # Sizing (anti self-inflation)
    ///
    /// The window frame itself is NOT resizable: the inner [`egui::Resize`] is
    /// the single size authority. Its size comes from a constant default seed
    /// plus the user's grip drag only — never from measured content — and the
    /// window auto-sizes to that box. Because no child size is derived from
    /// "remaining space" that the same subtree then fills, the window cannot
    /// grow on its own, on any egui context/viewport it is rendered in.
    #[allow(dead_code)]
    pub fn show(&mut self, ctx: &Context, tr: &Tr) -> Option<DebugAction> {
        let mut action: Option<DebugAction> = None;

        // Global keyboard shortcuts — active even when the window is not focused.
        if self.is_paused {
            if ctx.input(|i| i.key_pressed(Key::F5)) {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F10)) {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
            }
            // Shift+F11 FIRST: `key_pressed` ignores extra modifiers, so a
            // plain-F11 test would also swallow the shifted chord and Step Out
            // would never fire.
            if action.is_none()
                && ctx.input(|i| i.key_pressed(Key::F11) && i.modifiers.shift)
                && self.is_paused
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }
            if action.is_none() && ctx.input(|i| i.key_pressed(Key::F11) && !i.modifiers.shift) {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
            }
        }

        if action.is_none() {
            action = self.maybe_animate_step(ctx);
        }

        self.copy_shortcut(ctx);
        self.find_shortcuts(ctx);

        let debug_name = std::path::Path::new(&self.source_path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("generated code");

        let need_scroll = self.should_center_current_line();

        egui::Window::new(format!("Debugging {debug_name} generated code"))
            .id(egui::Id::new("debugger_window"))
            .resizable(false) // the inner `Resize` grip is the sole size control
            .collapsible(false)
            .show(ctx, |ui| {
                egui::Resize::default()
                    .id_salt("debugger_resize")
                    .resizable([true, true])
                    .min_size(egui::vec2(480.0, 320.0))
                    .max_size(egui::vec2(4000.0, 4000.0))
                    .default_size(egui::vec2(860.0, 460.0)) // seed only
                    .show(ui, |ui| {
                        // `sz` is the Resize box: user/default state, bounded —
                        // NOT "remaining space" of an auto-sizing container.
                        let sz = ui.available_size();
                        ui.allocate_ui(sz, |ui| {
                            // Fill the box exactly so the reported content
                            // min-size equals the box: the Resize can neither
                            // auto-grow nor auto-shrink to measured content.
                            ui.set_min_size(sz);
                            let sk = self.skin.palette();
                            paint_backdrop(ui.painter(), ui.max_rect(), sk);
                            apply_skin_visuals(sk, ui);

                            self.status_row(ui, tr);
                            if let Some(a) = self.toolbar(ui, tr) {
                                action = Some(a);
                            }
                            frame_rule(ui);
                            if let Some(line) = self.split_body(ui, tr, need_scroll) {
                                action = Some(DebugAction::ToggleBreakpoint(line));
                            }
                        });
                    });
            });

        self.variable_value_window(ctx);

        if need_scroll {
            self.last_scrolled_line = self.current_line;
            self.force_center_current = false;
        }

        action
    }

    // ── Status row ────────────────────────────────────────────────────────────

    /// The stop reason in the developer's language.
    ///
    /// `StopReason` is a runtime type and carries an English `headline()` for
    /// logs; the UI must not show that — every user-facing string is a `Tr`
    /// field in six languages.
    fn reason_label(reason: &StopReason, tr: &Tr) -> String {
        match reason {
            StopReason::Entry => tr.dbg_reason_entry.to_owned(),
            StopReason::Breakpoint(_) => tr.dbg_reason_breakpoint.to_owned(),
            StopReason::Step => tr.dbg_reason_step.to_owned(),
            StopReason::Pause => tr.dbg_reason_pause.to_owned(),
            StopReason::DataChanged { name, .. } => {
                format!("{} · {name}", tr.dbg_reason_data_changed)
            }
            StopReason::Exception { filter, .. } => {
                format!("{} · {filter}", tr.dbg_reason_runtime_error)
            }
            StopReason::Goto => tr.dbg_reason_goto.to_owned(),
        }
    }

    fn status_row(&self, ui: &mut egui::Ui, tr: &Tr) {
        let sk = self.skin.palette();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.add_space(PANE_PAD); // line the state row up with the code panel
            // Amber while stopped, green while running — the mockup's rounded
            // pill, its ink and dot the accent lifted toward white. The colours
            // are fixed, not read from `ui.visuals()`, which on a glass ground
            // renders dark-on-dark.
            if self.is_paused {
                let mut t = tr.dbg_state_paused.to_owned();
                if let Some(r) = &self.stop_reason {
                    t.push_str(" · ");
                    t.push_str(&Self::reason_label(r, tr));
                }
                status_pill(ui, &t, Color32::from_rgb(245, 158, 11));
            } else {
                status_pill(ui, tr.dbg_state_running, Color32::from_rgb(90, 200, 110));
            }

            if self.is_paused && !self.current_para.is_empty() {
                ui.label(
                    RichText::new(&self.current_para)
                        .monospace()
                        .color(sk.kw)
                        .size(14.0),
                );
                ui.label(
                    RichText::new(format!("line {}", self.current_line))
                        .color(sk.chrome_dim)
                        .size(13.5),
                );
            }
        });
    }

    // ── Controls toolbar ──────────────────────────────────────────────────────

    fn toolbar(&mut self, ui: &mut egui::Ui, tr: &Tr) -> Option<DebugAction> {
        let mut action: Option<DebugAction> = None;
        let mut refold = false;

        ui.horizontal(|ui| {
            // 4 px between buttons (operator), and a left indent that lines the
            // toolbar up with the code panel below it (PANE_PAD).
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(PANE_PAD);
            // NOT `is_paused`: while Animate runs, that flag flips at the
            // animation's rate and every button gated on it strobed with it.
            let paused = self.stopped_controls_available();

            if tool_icon(ui, "stop", true, false, Some(Color32::from_rgb(220, 80, 80)), tr.dbg_stop)
                .clicked()
            {
                self.center_current_line_next_frame();
                action = Some(DebugAction::Stop);
            }

            ui.separator();

            if tool_icon(ui, "play", paused, false, None, format!("{} — F5", tr.dbg_continue))
                .clicked()
            {
                action = Some(DebugAction::Continue);
                self.is_paused = false;
                // Continue means RUN, so it ends an animation rather than
                // leaving it armed to resume at the next stop.
                self.animate = false;
                self.last_animate_step = None;
            }

            if tool_icon(
                ui,
                "corner-up-right",
                paused,
                false,
                None,
                format!("{} — F10", tr.dbg_step_over),
            )
            .clicked()
            {
                action = Some(DebugAction::StepOver);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            if tool_icon(ui, "arrow-down", paused, false, None, format!("{} — F11", tr.dbg_step_into))
                .clicked()
            {
                action = Some(DebugAction::StepIn);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            if tool_icon(ui, "arrow-up", paused, false, None, format!("{} — ⇧F11", tr.dbg_step_out))
                .clicked()
            {
                action = Some(DebugAction::StepOut);
                self.is_paused = false;
                self.last_animate_step = None;
            }

            // Pause sits immediately LEFT of Go-to-current-line (operator).
            if tool_icon(ui, "pause", self.pause_available(), false, None, tr.dbg_pause).clicked() {
                self.on_pause_pressed();
                action = Some(DebugAction::Pause);
            }

            // Go to the line currently executing: centre the listing on it. It
            // does not run anything, so it is available whenever there IS a
            // current line, stopped or not (operator: replaces Run-to-Cursor).
            if tool_icon(
                ui,
                "crosshair",
                self.current_line > 0,
                false,
                None,
                tr.dbg_goto_running,
            )
            .clicked()
            {
                self.center_current_line_next_frame();
            }

            ui.separator();

            // The two materials, one button each. A pair rather than a toggle:
            // a toggle hides which one you are on until you read its tooltip,
            // and this is a choice people make once and want to see.
            for (skin, icon, tip) in [
                (DebugSkin::Smoked, "dark-mode", tr.dbg_skin_smoked),
                (DebugSkin::Frosted, "light-mode", tr.dbg_skin_frosted),
            ] {
                if tool_icon(ui, icon, true, self.skin == skin, None, tip).clicked() {
                    self.skin = skin;
                }
            }

            ui.separator();

            // Copy / Paste. The listing is read-only and stays that way: Copy
            // takes what is selected, and Paste puts what was copied into
            // whatever text field has focus — a watch expression or the
            // variable filter. Over the listing it lands nowhere, because the
            // listing is not a text field (operator, 2026-09-17).
            let has_selection = self.selection_range().is_some();
            if tool_icon(
                ui,
                "clipboard-copy",
                has_selection,
                false,
                None,
                format!(
                    "{}\n\nDrag across the code to select it. The listing cannot be \
                     edited — only copied.",
                    tr.dbg_copy
                ),
            )
            .clicked()
            {
                if let Some(text) = self.selected_text() {
                    ui.ctx().copy_text(text.clone());
                    self.copied = Some(text);
                }
            }

            let can_paste = self.copied.is_some();
            if tool_icon(
                ui,
                "clipboard-paste",
                can_paste,
                false,
                None,
                format!(
                    "{}\n\nInto a watch expression or the variable filter. Nothing \
                     happens over the code, which is read-only.",
                    tr.dbg_paste
                ),
            )
            .clicked()
            {
                if let Some(text) = self.copied.clone() {
                    // Delivered as a paste EVENT rather than written anywhere:
                    // egui hands it to whichever field has focus, and to nothing
                    // at all when none has. That is exactly the wanted
                    // behaviour, and it needs no knowledge here of which boxes
                    // exist.
                    ui.ctx()
                        .input_mut(|i| i.events.push(egui::Event::Paste(text)));
                }
            }

            ui.separator();

            if tool_icon(
                ui,
                "collapse",
                true,
                self.hide_empty_blocks,
                None,
                format!(
                    "{}\n\nFold away divisions, sections and paragraphs that hold no \
                     executable statement. A view filter only — line numbers are \
                     unchanged and stepping is unaffected.",
                    tr.dbg_hide_empty_blocks
                ),
            )
            .clicked()
            {
                self.hide_empty_blocks = !self.hide_empty_blocks;
                refold = true;
            }

            if tool_icon(
                ui,
                "eye-off",
                true,
                self.hide_generated,
                None,
                format!(
                    "{}\n\nFold the blocks the IDE generated — the event loop and its \
                     plumbing. They are assumed to work; what is left is the code \
                     you wrote.",
                    tr.dbg_hide_generated
                ),
            )
            .clicked()
            {
                self.hide_generated = !self.hide_generated;
                refold = true;
            }

            if tool_icon(
                ui,
                "user-check",
                true,
                self.only_user_code,
                None,
                format!(
                    "{}\n\nStep through your own handlers and procedures only. \
                     The generated event loop and the rest of the scaffolding \
                     are crossed without stopping. Breakpoints still fire \
                     wherever you set them.",
                    tr.dbg_only_my_code
                ),
            )
            .clicked()
            {
                self.only_user_code = !self.only_user_code;
            }

            ui.separator();

            if tool_icon(
                ui,
                "fast-forward",
                true,
                self.animate,
                None,
                format!(
                    "{}\n\nFollow execution one statement at a time",
                    tr.dbg_animate
                ),
            )
            .clicked()
            {
                self.animate = !self.animate;
                self.last_animate_step = None;
            }
            if self.animate {
                ui.add(
                    egui::Slider::new(&mut self.animate_speed_lps, 1.0..=10.0)
                        .text("lines/s")
                        .step_by(1.0),
                );
            }

            ui.separator();
            // Code font size (operator): A−/A+, clamped 8–22 pt.
            if tool_icon(ui, "minus", self.code_font_pt > 8.0, false, None, tr.dbg_font_smaller)
                .clicked()
            {
                self.code_font_pt = (self.code_font_pt - 1.0).max(8.0);
            }
            if tool_icon(ui, "plus", self.code_font_pt < 22.0, false, None, tr.dbg_font_larger)
                .clicked()
            {
                self.code_font_pt = (self.code_font_pt + 1.0).min(22.0);
            }
        });

        if refold {
            // Both filters feed one fold list, so a toggle recomputes it rather
            // than each filter keeping its own and the two disagreeing.
            self.hidden = folds(&self.source_lines, self.hide_empty_blocks, self.hide_generated);
        }
        action
    }

    pub fn center_current_line_next_frame(&mut self) {
        self.force_center_current = true;
        self.last_scrolled_line = 0;
    }

    fn should_center_current_line(&self) -> bool {
        self.current_line > 0
            && (self.force_center_current || self.current_line != self.last_scrolled_line)
    }

    /// Is **Pause** clickable?
    ///
    /// While the program runs, yes — that is what it is for. And whenever
    /// **Animate** is on, always: between one animated step and the next the
    /// panel is *stopped*, so gating Pause on "running" made it strobe
    /// enabled/disabled at the animation's own rate, and the developer could
    /// not reliably click the one control that stops the thing
    /// (operator, 2026-09-19).
    pub(crate) fn pause_available(&self) -> bool {
        !self.is_paused || self.animate
    }

    /// Are the stopped-program controls — Continue and the three steps —
    /// clickable?
    ///
    /// While stopped, yes. While animating, yes and for the same reason: they
    /// strobed with `is_paused` exactly as Pause did, and taking over from an
    /// animation with a step is how a developer says "I will drive from
    /// here".
    pub(crate) fn stopped_controls_available(&self) -> bool {
        self.is_paused || self.animate
    }

    /// The developer pressed **Pause**: any animation is over and the program
    /// stops where it is, waiting for them. Animate is re-armed by pressing
    /// it again; a step takes over by hand.
    pub(crate) fn on_pause_pressed(&mut self) {
        self.animate = false;
        self.last_animate_step = None;
        self.center_current_line_next_frame();
    }

    fn maybe_animate_step(&mut self, ctx: &Context) -> Option<DebugAction> {
        if !self.animate || !self.is_paused {
            if !self.animate {
                self.last_animate_step = None;
            }
            return None;
        }

        let speed = self.animate_speed_lps.clamp(1.0, 10.0);
        let interval = Duration::from_secs_f32(1.0 / speed);
        ctx.request_repaint_after(interval);

        let now = Instant::now();
        if self
            .last_animate_step
            .map(|last| now.duration_since(last) < interval)
            .unwrap_or(false)
        {
            return None;
        }

        self.last_animate_step = Some(now);
        self.force_center_current = true;
        self.is_paused = false;
        Some(DebugAction::StepOver)
    }

    // ── Split body ────────────────────────────────────────────────────────────

    /// Two-pane split (code viewer left, variables right), with a draggable
    /// divider whose position is persisted by egui's table state.
    /// Returns the gutter line the developer clicked, if any.
    /// The file tab and the breadcrumb: `generated › switch-form.cbl ›
    /// SWITCH-FORM--ONLOAD › PROCEDURE DIVISION`.
    ///
    /// The old header was the absolute path on one line, which is the least
    /// useful thing to show: the developer knows which project they opened and
    /// cannot read a 70-character path at a glance anyway. What they need is
    /// WHERE IN THE PROGRAM the pointer is, which is the trail.
    fn file_strip(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        // Owned up front: the find box on this row needs `&mut self`, and a
        // borrow of `source_path` would still be alive inside the closure.
        let (file, folder) = {
            let path = std::path::Path::new(&self.source_path);
            (
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("(no source)")
                    .to_owned(),
                path.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_owned(),
            )
        };
        let (file, folder) = (file.as_str(), folder.as_str());
        // Copied out once so the tab is painted in the skin, not in the
        // hardcoded slate it used to be — which was a dark tab on Frosted's
        // pale ground. `sk` is `Copy`, so this leaves no borrow to fight the
        // `&mut self` the find box needs below.
        let sk = self.skin.palette();

        // The tab. One file today — a session debugs one program — so it is
        // drawn as the tab it will be rather than promising a strip that does
        // not exist yet.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            let (rect, _) = ui.allocate_exact_size(
                // Sized from the label's own width, so a long file name is not
                // clipped and a short one does not leave a wide empty tab.
                Vec2::new(file.chars().count() as f32 * 7.0 + 22.0, 22.0),
                egui::Sense::hover(),
            );
            // The mockup's tab: 9%-white paper, rounded on top only (radius 9),
            // mono name in the chrome ink, an amber underline for "active".
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius {
                    nw: 9,
                    ne: 9,
                    sw: 0,
                    se: 0,
                },
                sk.field_bg,
            );
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius {
                    nw: 9,
                    ne: 9,
                    sw: 0,
                    se: 0,
                },
                egui::Stroke::new(1.0, sk.line),
                egui::StrokeKind::Inside,
            );
            ui.painter().text(
                rect.left_center() + Vec2::new(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                file,
                egui::FontId::monospace(13.5),
                sk.chrome,
            );
            // The amber underline is the "this is the active tab" marker, the
            // same amber the current-line pointer uses (the mockup's #f5b544).
            ui.painter().hline(
                rect.x_range(),
                rect.max.y - 1.0,
                egui::Stroke::new(2.0, sk.cur_accent),
            );
            // Find sits on this row, hard right: always there, never a panel
            // that appears and shifts the listing down when you press a key.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.find_bar(ui, tr);
            });
        });

        // The trail. Each crumb is what the debugger actually knows: the
        // folder, the file, the program (paragraph) it stopped in, and the
        // division that paragraph lives in.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(4.0);
            let mut crumbs: Vec<String> = Vec::new();
            if !folder.is_empty() {
                crumbs.push(folder.to_owned());
            }
            crumbs.push(file.to_owned());
            if let Some(f) = self.frames.get(self.selected_frame) {
                if !f.program.is_empty() {
                    crumbs.push(f.program.clone());
                }
                if let Some(sec) = &f.section {
                    crumbs.push(sec.clone());
                }
                if !f.paragraph.is_empty() {
                    crumbs.push(f.paragraph.clone());
                }
            } else if !self.current_para.is_empty() {
                crumbs.push(self.current_para.clone());
            }
            let last = crumbs.len().saturating_sub(1);
            for (i, c) in crumbs.iter().enumerate() {
                if i > 0 {
                    ui.label(
                        RichText::new("›")
                            .size(13.0)
                            .color(Color32::from_gray(90)),
                    );
                }
                ui.label(
                    RichText::new(c)
                        .size(13.0)
                        // The last crumb is where you ARE; the rest are context.
                        .color(if i == last {
                            Color32::from_rgb(215, 225, 240)
                        } else {
                            Color32::from_gray(125)
                        }),
                );
            }
        });
    }

    fn split_body(&mut self, ui: &mut egui::Ui, tr: &Tr, need_scroll: bool) -> Option<u32> {
        // The dock and the status bar are laid out as BOTTOM PANELS, before the
        // split, and the split takes what is left.
        //
        // They used to be allocated after the split table, from whatever height
        // the arithmetic said was left — and the table, with `auto_shrink` off,
        // takes the whole space it is offered. So the grip, the dock and the
        // status bar were placed past the bottom of the window and simply never
        // drawn: the operator saw a dead band under the code and asked what it
        // was for, twice (2026-09-17). A bottom panel cannot be pushed off,
        // because egui gives it its height before the central area sees any.
        const GRIP_H: f32 = 6.0;
        let total = ui.available_height();
        let dock_h = self.dock_height.clamp(80.0, (total - 160.0).max(80.0));
        let mut toggled: Option<u32> = None;

        // The status strip sits below the dock, so it is claimed first.
        egui::Panel::bottom("dbg_status_strip")
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: PANE_PAD as i8,
                right: PANE_PAD as i8,
                top: 2,
                bottom: 2,
            }))
            .show(ui, |ui| {
                self.status_bar(ui, tr);
            });

        let sk = self.skin.palette();
        let line_c = sk.line;
        egui::Panel::bottom("dbg_investigation_dock")
            .resizable(false)
            .show_separator_line(false)
            .exact_size(dock_h + GRIP_H + PROMPT_H)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // Inset so the dock has the same side gutter as the listing and
                // inspector, and its rounded corners are visible against the
                // darker window (operator: restore the padding between panels).
                paint_grad_pane_h(
                    ui.painter(),
                    ui.max_rect().shrink2(egui::vec2(PANE_PAD, 0.0)),
                    sk.code_bg,
                    sk.code_bg2,
                );
                // The grip: the ONE writer of `dock_height`, and it only ever
                // moves by the drag the developer performed.
                let (grip, grip_resp) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), GRIP_H),
                    egui::Sense::drag(),
                );
                if grip_resp.hovered() || grip_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if grip_resp.dragged() {
                    self.dock_height = (self.dock_height - grip_resp.drag_delta().y)
                        .clamp(80.0, (total - 160.0).max(80.0));
                }
                dashed_hline(ui.painter(), grip.x_range(), grip.center().y, line_c);
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: PANE_PAD as i8,
                        right: PANE_PAD as i8,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        self.investigation_dock(ui, tr);
                    });
            });

        // Rows sit flush; nothing between the listing and the dock but the
        // padding each pane draws for itself.
        ui.spacing_mut().item_spacing.y = 0.0;

        // The inspector is a RIGHT PANEL and the listing takes what is left.
        //
        // It was a two-column `TableBuilder`, and that is what wrecked the
        // layout (operator, 2026-09-17): a resizable table persists its column
        // widths under its own id, so after the window and the panels around it
        // changed, the two columns kept widths measured for a different box —
        // roughly 630 + 670 of a 2000-wide window, with the rest left as bare
        // backdrop and the column separators ruled down through the dock.
        //
        // A panel cannot do that. egui gives it its width before the central
        // area is measured, the central area is whatever remains, and there is
        // no stored geometry to go stale. Same reason the dock became a panel.
        egui::Panel::right("dbg_inspector")
            .resizable(true)
            .show_separator_line(false)
            .default_size(380.0)
            .min_size(260.0)
            .max_size(720.0)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                // The divider, with air on each side, then the pane itself.
                dashed_vline(
                    ui.painter(),
                    ui.max_rect().left(),
                    ui.max_rect().y_range(),
                    line_c,
                );
                // Inset on ALL four sides, not just left and right: inset
                // horizontally alone left this pane running flush to the
                // toolbar above and the dock below while the listing beside it
                // kept its gap, which is the uneven padding the operator
                // reported (2026-09-17). Same inset as the listing, so the two
                // panes start and stop on the same lines.
                let pane = ui.max_rect().shrink(PANE_PAD);
                paint_grad_pane_h(ui.painter(), pane, sk.code_bg, sk.code_bg2);
                egui::Frame::NONE
                    .inner_margin(egui::Margin::same((PANE_PAD * 2.0) as i8))
                    .show(ui, |ui| {
                        self.data_tabs(ui, tr);
                    });
            });

        // Whatever the panels left over is the listing.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let pane_w = (ui.available_width() - PANE_PAD * 2.0).max(80.0);
                egui::Frame::NONE
                    .inner_margin(egui::Margin::same(PANE_PAD as i8))
                    .show(ui, |ui| {
                        toggled = self.code_viewer(ui, need_scroll, pane_w, tr);
                    });
            });

        // A gutter click wins the frame; otherwise a ✕ pressed in the
        // Breakpoints list clears that line. Both are the same toggle.
        toggled.or_else(|| self.bp_remove_request.take())
    }

    /// Ctrl/Cmd+C over a selection, because that is the key everyone reaches
    /// for before they look for a button.
    ///
    /// Only when something is selected: swallowing the chord otherwise would
    /// take it from a watch expression box the developer was editing.
    fn copy_shortcut(&mut self, ctx: &Context) {
        if self.selection_range().is_none() {
            return;
        }
        let pressed = ctx.input(|i| {
            i.key_pressed(Key::C) && (i.modifiers.command || i.modifiers.ctrl)
        });
        if !pressed {
            return;
        }
        if let Some(text) = self.selected_text() {
            ctx.copy_text(text.clone());
            self.copied = Some(text);
        }
    }

    // ── Find ──────────────────────────────────────────────────────────────────

    /// Ctrl/Cmd+F opens the find bar; Escape closes it; F3 and Shift+F3 walk
    /// the hits from anywhere, without the bar needing focus.
    fn find_shortcuts(&mut self, ctx: &Context) {
        let (open, close, next, prev) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::F),
                i.key_pressed(egui::Key::Escape),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F3),
                i.consume_key(egui::Modifiers::SHIFT, egui::Key::F3),
            )
        });
        if open {
            self.find_focus = true;
        }
        // Escape empties the box rather than hiding it: the box is part of the
        // strip now, and a control that vanishes on Escape is one you have to
        // rediscover.
        if close && !self.find_query.is_empty() {
            self.find_query.clear();
            self.rebuild_find_hits();
            self.clear_selection();
        }
        if next || prev {
            self.rebuild_find_hits();
            self.step_find(next);
        }
    }

    /// The find bar: a query box, the hit count, and the two arrows.
    ///
    /// No replace box. The listing is generated code opened read-only, so a
    /// replace would be a control that refuses every use of it.
    fn find_bar(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        // Always on screen. It used to appear on Ctrl+F and take a row of its
        // own, which pushed the listing down the moment you reached for it;
        // the mockup has it sitting in the file strip, and a search box you can
        // see is one you remember you have (operator, 2026-09-17). Ctrl+F now
        // only puts the caret in it.
        //
        // Laid out right-to-left by the caller, so the controls are written in
        // reverse: the close button first, the field last.
        {
            let ui = &mut *ui;
            if tool_icon(ui, "x-mark", !self.find_query.is_empty(), false, None, tr.dbg_find_close)
                .clicked()
            {
                self.find_query.clear();
                self.rebuild_find_hits();
                self.clear_selection();
            }
            let total = self.find_hits.len();
            let label = if self.find_query.trim().is_empty() {
                String::new()
            } else if total == 0 {
                tr.dbg_find_none.to_owned()
            } else {
                format!("{} / {total}", self.find_at + 1)
            };
            ui.label(
                RichText::new(label)
                    .size(13.0)
                    .color(if total == 0 && !self.find_query.trim().is_empty() {
                        Color32::from_rgb(220, 120, 120)
                    } else {
                        self.skin.palette().chrome_dim
                    }),
            );
            let any = total > 0;
            if tool_icon(ui, "arrow-down", any, false, None, format!("{} — F3", tr.dbg_find_next))
                .clicked()
            {
                self.step_find(true);
            }
            if tool_icon(ui, "arrow-up", any, false, None, format!("{} — ⇧F3", tr.dbg_find_prev))
                .clicked()
            {
                self.step_find(false);
            }
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.find_query)
                    .desired_width(190.0)
                    .hint_text(tr.dbg_find_hint)
                    .font(egui::TextStyle::Monospace),
            );
            if std::mem::take(&mut self.find_focus) {
                field.request_focus();
            }
            self.rebuild_find_hits();
            // Enter walks forward, Shift+Enter back — the same pairing the
            // editor's own find uses, so the habit carries over.
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let back = ui.input(|i| i.modifiers.shift);
                self.step_find(!back);
                self.find_focus = true;
            }
        }
    }



    /// Rebuild the hit list when the query changed.
    ///
    /// Case-insensitive, because COBOL is: a developer looking for `ws-line`
    /// means `WS-LINE`, and making them match the listing's case would be a
    /// rule they have to remember for no gain.
    ///
    /// Searched against the DRAWN text, the same string the listing paints and
    /// the same one a selection copies — so a hit's character range lines up
    /// with what is on screen, elided literals included.
    fn rebuild_find_hits(&mut self) {
        if self.find_built_for == self.find_query {
            return;
        }
        self.find_built_for = self.find_query.clone();
        self.find_hits.clear();
        self.find_at = 0;
        let needle = self.find_query.trim().to_ascii_uppercase();
        if needle.is_empty() {
            return;
        }
        let width = needle.chars().count();
        for (idx, raw) in self.source_lines.iter().enumerate() {
            let drawn = elide_long_literals(raw, LITERAL_DRAW_CHARS);
            let hay: Vec<char> = drawn.chars().collect();
            let upper: String = drawn.to_ascii_uppercase();
            let upper: Vec<char> = upper.chars().collect();
            if upper.len() < width {
                continue;
            }
            // Character-indexed rather than byte-indexed: a line carrying
            // accented prose has more bytes than columns, and every consumer
            // here — the band, the copy, the caret — counts in characters.
            for start in 0..=(upper.len() - width) {
                if upper[start..start + width]
                    .iter()
                    .copied()
                    .eq(needle.chars())
                {
                    let _ = &hay;
                    self.find_hits.push((idx, start, start + width));
                }
            }
        }
    }

    /// Move to the next hit (`forward`) or the previous one, wrapping.
    ///
    /// The current hit is also selected, so Copy works on what was found and
    /// the band the developer is looking at is the one selection they have.
    fn step_find(&mut self, forward: bool) {
        if self.find_hits.is_empty() {
            return;
        }
        self.find_at = if forward {
            (self.find_at + 1) % self.find_hits.len()
        } else {
            (self.find_at + self.find_hits.len() - 1) % self.find_hits.len()
        };
        self.reveal_current_hit();
    }

    /// Select the current hit and scroll it into view.
    fn reveal_current_hit(&mut self) {
        let Some(&(line, from, to)) = self.find_hits.get(self.find_at) else {
            return;
        };
        self.sel_anchor = Some((line, from));
        self.sel_cursor = Some((line, to));
        // The listing centres on a line by number, the same road the stopped
        // line takes, so a hit off-screen is brought into view the same way.
        self.current_find_line = Some(line as u32 + 1);
    }

    // ── Code selection ────────────────────────────────────────────────────────

    /// The selection, ordered so the start never follows the end.
    ///
    /// Stored as anchor-and-cursor because that is what a drag produces; every
    /// reader wants it the other way round.
    fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        let (a, b) = (self.sel_anchor?, self.sel_cursor?);
        if a == b {
            return None;
        }
        Some(if a <= b { (a, b) } else { (b, a) })
    }

    /// The character range of `line` that is selected, if any.
    ///
    /// `None` means the line is untouched; a returned range may be empty at the
    /// very edges of the drag, which paints as nothing and copies as nothing.
    fn selection_on_line(&self, line: usize, len: usize) -> Option<(usize, usize)> {
        let ((s_line, s_col), (e_line, e_col)) = self.selection_range()?;
        if line < s_line || line > e_line {
            return None;
        }
        let from = if line == s_line { s_col.min(len) } else { 0 };
        let to = if line == e_line { e_col.min(len) } else { len };
        (from < to).then_some((from, to))
    }

    /// The selected text, exactly as the listing shows it.
    ///
    /// Taken from the DRAWN lines, so a literal the listing elided is copied
    /// elided too — copying text the developer cannot see would be the more
    /// surprising of the two. Lines are joined with `\n` whatever the platform,
    /// which is what every editor this is pasted into expects.
    fn selected_text(&self) -> Option<String> {
        let ((s_line, _), (e_line, _)) = self.selection_range()?;
        let mut out = String::new();
        for line in s_line..=e_line {
            let text = self.source_lines.get(line)?;
            let drawn = elide_long_literals(text, LITERAL_DRAW_CHARS);
            let chars: Vec<char> = drawn.chars().collect();
            if let Some((from, to)) = self.selection_on_line(line, chars.len()) {
                out.extend(&chars[from..to]);
            }
            if line < e_line {
                out.push('\n');
            }
        }
        (!out.is_empty()).then_some(out)
    }

    fn clear_selection(&mut self) {
        self.sel_anchor = None;
        self.sel_cursor = None;
    }

    // ── Code viewer ───────────────────────────────────────────────────────────

    /// Returns the line whose gutter was clicked, if any — the caller turns it
    /// into a [`DebugAction::ToggleBreakpoint`]. The viewer takes `&self`, so it
    /// reports the click rather than editing the set itself.
    fn code_viewer(
        &mut self,
        ui: &mut egui::Ui,
        need_scroll: bool,
        pane_w: f32,
        tr: &Tr,
    ) -> Option<u32> {
        // The file tab and the breadcrumb trail, in place of the bare absolute
        // path this used to print.
        self.file_strip(ui, tr);
        ui.add_space(2.0);

        // The gutter click collected this frame. `code_viewer` only reads the
        // panel, so the toggle travels out as a return value instead of being
        // applied here — the breakpoint set lives in the editor, and both the
        // panel and the running debuggee are synced from there.
        let sk = self.skin.palette();
        // The developer-chosen code font, and the row height and gutter that
        // follow it (operator: font-size controls).
        let fpt = self.code_font_pt;
        let line_h = (fpt + 3.0).max(14.0);
        let gutter_pt = (fpt - 1.5).max(8.0);
        let mut toggled: Option<u32> = None;
        // Collected inside the closure and applied after it, so nothing borrows
        // `self` mutably while the source list is being read.
        let mut expand_run: Option<u32> = None;
        let mut picked_line: Option<u32> = None;
        // Collected inside the scroll closure and applied after it: the closure
        // borrows `self` immutably to read the source, so it cannot also move
        // the selection.
        let mut drag_from: Option<(usize, usize)> = None;
        let mut drag_to: Option<(usize, usize)> = None;
        let mut clicked_bare = false;
        let mut dbl_word: Option<(usize, usize, usize)> = None;
        // Taken before the loop: the closure reads `self` immutably, so the
        // request cannot be cleared from inside it.
        let find_line = self.current_find_line;

        // The listing's own darker ground, painted under the whole remaining
        // pane before the rows go on top of it. Taken from `available_*` at the
        // point the scroll area starts, which is the pane the caller sized —
        // not from measured content, so it cannot feed a growth loop.
        // The Frame that wraps this pane reserves its TOP inner margin before
        // the content but adds the BOTTOM one only after, so `available_height`
        // runs all the way down to the dock. Reserve the bottom gutter here
        // explicitly, so the pane ends PANE_PAD above the dock and the window
        // shows through the gap (operator: the bottom padding was missing on
        // open, and a resize was the only thing that brought it back).
        let pane_h = (ui.available_height() - PANE_PAD).max(0.0);
        let face = egui::Rect::from_min_size(
            ui.cursor().min,
            Vec2::new(ui.available_width(), pane_h),
        );
        paint_grad_pane_h(ui.painter(), face, sk.code_bg, sk.code_bg2);

        // Horizontal wheel/trackpad scrolling is suppressed over the listing
        // (operator): a sideways swipe while reading no longer drifts the code.
        // The horizontal scrollbar KNOB still works — it is driven by a drag on
        // the bar, not by scroll delta — and vertical scrolling is untouched.
        if ui.rect_contains_pointer(face) {
            ui.input_mut(|i| {
                i.smooth_scroll_delta.x = 0.0;
                for ev in &mut i.events {
                    if let egui::Event::MouseWheel { delta, .. } = ev {
                        delta.x = 0.0;
                    }
                }
            });
        }

        ScrollArea::both()
            .id_salt("dbg_code_scroll")
            .auto_shrink([false, false])
            // Stop the listing at the reserved bottom gutter, not the dock.
            .max_height(pane_h)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let current = self.current_line;
                let bps = &self.breakpoints;
                let vars = &self.vars;

                let folding = self.hide_empty_blocks;
                let runs = &self.hidden;
                let expanded = &self.expanded_runs;

                for (idx, line_text) in self.source_lines.iter().enumerate() {
                    let line_num = (idx + 1) as u32;
                    let is_current = line_num == current;
                    let is_bp = bps.contains(&line_num);

                    // The empty-block filter. Purely visual: the line numbers
                    // below are the file's own, the breakpoint set is untouched,
                    // and the debuggee never hears about it. A folded run that
                    // holds the current statement or a breakpoint is shown
                    // anyway — hiding where the program actually stopped would
                    // be the one thing worse than the clutter.
                    if folding {
                        if let Some(run) = runs.iter().find(|r| r.contains(line_num)) {
                            let forced = run.contains(current)
                                || bps.iter().any(|b| run.contains(*b))
                                || expanded.contains(&run.start);
                            if !forced {
                                if line_num == run.start {
                                    // The run's description — shown only as the
                                    // arrow's tooltip now (operator), so a
                                    // generated region names itself and count and
                                    // an empty run says how many it swallowed.
                                    let tip = match run.kind {
                                        FoldKind::Generated => format!(
                                            "{}  ·  {}",
                                            run.label.as_deref().unwrap_or("generated"),
                                            run.lines()
                                        ),
                                        FoldKind::Empty => {
                                            marker_text(tr.dbg_empty_blocks_hidden, run)
                                        }
                                    };
                                    if fold_chip(ui, &tip, sk).clicked() {
                                        expand_run = Some(run.start);
                                    }
                                }
                                continue;
                            }
                        }
                    }

                    if line_text.trim().is_empty() {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(pane_w, CODE_BLANK_H), egui::Sense::hover());
                        if is_current {
                            let full = egui::Rect::from_x_y_ranges(
                                ui.clip_rect().x_range(),
                                rect.y_range(),
                            );
                            ui.painter().rect_filled(full, 0.0, sk.cur_bg);
                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    full.min,
                                    Vec2::new(CURRENT_STRIPE_W, full.height()),
                                ),
                                0.0,
                                sk.cur_accent,
                            );
                            if need_scroll {
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                            }
                        }
                        continue;
                    }

                    // The band behind the stopped line. Reserved BEFORE the row
                    // and filled in after it, from the row's own rectangle:
                    // guessing the height ahead of the layout is what left the
                    // band sitting above its text, and painting afterwards on
                    // top of the painter's queue would bury the text instead.
                    let band = is_current.then(|| ui.painter().add(egui::Shape::Noop));

                    let row = ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        // Line number
                        ui.add_sized(
                            [32.0, line_h],
                            egui::Label::new(
                                RichText::new(format!("{:>4}", line_num))
                                    .monospace()
                                    .size(gutter_pt)
                                    .color(if is_current { sk.cur_accent } else { sk.gutter }),
                            ),
                        );

                        // Gutter: breakpoint dot and/or ► arrow. Clickable — a
                        // developer sets a breakpoint where they are reading the
                        // code, which is here, not in a separate editor tab.
                        // A breakpoint belongs ONLY on a line that starts a COBOL
                        // statement — a verb (operator). Never a division or
                        // section header, a paragraph name, a data item, a scope
                        // terminator or a comment; those show no ring and swallow
                        // no click, because the program never stops on them.
                        let bp_allowed = line_allows_breakpoint(line_text);
                        let (gut_rect, gut_resp) =
                            ui.allocate_exact_size(
                                Vec2::new(18.0, line_h),
                                egui::Sense::click(),
                            );
                        // Clickable to SET on a verb line, or to CLEAR an
                        // existing breakpoint anywhere (so a stale one is never
                        // stuck).
                        if (bp_allowed || is_bp) && gut_resp.clicked() {
                            toggled = Some(line_num);
                        }
                        if (bp_allowed || is_bp) && gut_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if is_bp {
                            ui.painter().circle_filled(
                                gut_rect.center(),
                                4.5,
                                Color32::from_rgb(210, 50, 50),
                            );
                        } else if bp_allowed {
                            // Always a hollow high-contrast ring where a
                            // breakpoint CAN be set (operator), brighter under
                            // the pointer so the click target is unmistakable.
                            let ring = if gut_resp.hovered() {
                                Color32::from_rgb(232, 238, 246)
                            } else {
                                sk.chrome_dim
                            };
                            ui.painter().circle_stroke(
                                gut_rect.center(),
                                4.5,
                                egui::Stroke::new(1.5, ring),
                            );
                        }
                        if is_current {
                            ui.painter().text(
                                gut_rect.center() + Vec2::new(2.0, 0.0),
                                egui::Align2::CENTER_CENTER,
                                "▶",
                                egui::FontId::monospace(10.0),
                                sk.cur_accent,
                            );
                        }

                        // Syntax-highlighted source line. Clickable: clicking
                        // picks the Run-to-Cursor target, which is why that
                        // button stays disabled until there is one.
                        // Drawn with over-long literals elided, so one line of
                        // documentation prose cannot make the whole listing
                        // scroll sideways. `draw_text` is what the galley, the
                        // hover lookup and the inline values all use, so the
                        // character indices they trade in agree; the source and
                        // the program are untouched.
                        let draw_text = elide_long_literals(line_text, LITERAL_DRAW_CHARS);
                        let draw_text = draw_text.as_ref();
                        // No forced ink on the stopped line any more: the band
                        // below it is a tint, so the syntax palette is still
                        // legible and the line still reads as code.
                        let job = build_cobol_layout_job_inked(draw_text, sk, None, fpt);
                        // Laid out by hand rather than through `Label`, because
                        // the galley is what can answer "which word is the
                        // pointer over" — and that is the whole of the hover
                        // tooltip below.
                        let galley = ui.painter().layout_job(job);
                        let size = galley.size();
                        // `click_and_drag`, because a drag across the listing is
                        // how a selection is made. The click meaning is
                        // unchanged: it still picks the Run-to-Cursor target.
                        let (text_rect, resp) = ui.allocate_exact_size(
                            Vec2::new(size.x, size.y.max(line_h)),
                            egui::Sense::click_and_drag(),
                        );
                        let text_pos = egui::pos2(
                            text_rect.left(),
                            text_rect.center().y - size.y / 2.0,
                        );

                        // Where in this line the pointer is, in characters.
                        let char_at = |p: egui::Pos2| -> usize {
                            galley.cursor_from_pos(p - text_pos).index.0
                        };
                        // Double-click takes the whole word under the pointer.
                        if resp.double_clicked() {
                            if let Some(p) = resp.interact_pointer_pos() {
                                if let Some((from, to)) = word_span_at(draw_text, char_at(p)) {
                                    dbl_word = Some((idx, from, to));
                                }
                            }
                        }
                        if resp.drag_started() {
                            if let Some(p) = resp.interact_pointer_pos() {
                                drag_from = Some((idx, char_at(p)));
                            }
                        }
                        if resp.dragged() {
                            if let Some(p) = resp.interact_pointer_pos() {
                                drag_to = Some((idx, char_at(p)));
                            }
                        }

                        // The selection band, under the text so the ink stays
                        // on top of it.
                        let drawn_len = draw_text.chars().count();
                        let sel_band = self.selection_on_line(idx, drawn_len).map(|(from, to)| {
                            let x0 = galley.pos_from_cursor(egui::text::CCursor::new(from)).left();
                            let x1 = galley.pos_from_cursor(egui::text::CCursor::new(to)).left();
                            let band = egui::Rect::from_min_max(
                                egui::pos2(text_pos.x + x0, text_rect.top()),
                                egui::pos2(text_pos.x + x1, text_rect.bottom()),
                            );
                            ui.painter().rect_filled(band, 1.0, sk.sel_bg);
                            band
                        });

                        ui.painter().galley(
                            text_pos,
                            galley.clone(),
                            sk.ink,
                        );
                        // …and the selected characters again, in one high-contrast
                        // ink, clipped to the band. The first pass wears syntax
                        // colours picked for a dark ground; left on the blue band
                        // they are barely legible, and a selection you cannot read
                        // is not much of a selection.
                        if let Some(band) = sel_band {
                            let job = build_cobol_layout_job_inked(draw_text, sk, Some(sk.sel_ink), fpt);
                            let hot = ui.painter().layout_job(job);
                            ui.painter()
                                .with_clip_rect(band.intersect(ui.clip_rect()))
                                .galley(text_pos, hot, SEL_INK);
                        }
                        if resp.clicked() {
                            picked_line = Some(line_num);
                            clicked_bare = true;
                        }
                        // Hovering a data item says what it holds right now.
                        // Only the paused frame has values to report, so this
                        // is silent while the program runs.
                        if let Some(pos) = resp.hover_pos() {
                            let cursor = galley.cursor_from_pos(pos - text_pos);
                            if let Some(word) = word_at(draw_text, cursor.index.0) {
                                if let Some((name, value)) = lookup_value(&word, vars) {
                                    resp.clone().on_hover_text(hover_tip(name, value));
                                }
                            }
                        }
                        // Inline values, on the stopped line only.
                        if is_current {
                            for (name, value) in inline_values(draw_text, vars, 2) {
                                ui.add_space(14.0);
                                ui.label(
                                    RichText::new(format!("{name} = {value}"))
                                        .monospace()
                                        .size(12.0)
                                        .color(sk.cur_ink),
                                );
                            }
                        }
                    });

                    // Now the row's real extent is known: the band spans the
                    // whole visible width, whatever the horizontal scroll, and
                    // exactly the row's own height.
                    if let Some(idx) = band {
                        let full = egui::Rect::from_x_y_ranges(
                            ui.clip_rect().x_range(),
                            row.response.rect.y_range(),
                        );
                        ui.painter().set(
                            idx,
                            egui::Shape::Vec(vec![
                                egui::Shape::rect_filled(full, 0.0, sk.cur_bg),
                                // The accent bar down the left edge.
                                egui::Shape::rect_filled(
                                    egui::Rect::from_min_size(
                                        full.min,
                                        Vec2::new(CURRENT_STRIPE_W, full.height()),
                                    ),
                                    0.0,
                                    sk.cur_accent,
                                ),
                            ]),
                        );
                    }

                    // Auto-scroll when current line changes
                    if is_current && need_scroll {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                    // …and when the find bar has just moved to a hit. Same
                    // road, so a match off-screen is brought into view exactly
                    // as the stopped line is.
                    if find_line == Some(line_num) {
                        ui.scroll_to_cursor(Some(egui::Align::Center));
                    }
                }
            });

        self.current_find_line = None;
        if let Some(start) = expand_run {
            self.expanded_runs.insert(start);
        }
        if let Some(l) = picked_line {
            self.cursor_line = Some(l);
        }
        // A drag that began this frame starts a new selection; one already in
        // flight extends it. A bare click with no drag clears it, the way it
        // does in every editor — otherwise the band would sit there after the
        // developer had plainly moved on.
        if let Some(from) = drag_from {
            self.sel_anchor = Some(from);
            self.sel_cursor = Some(from);
        }
        if let Some(to) = drag_to {
            self.sel_cursor = Some(to);
        } else if clicked_bare && drag_from.is_none() && dbl_word.is_none() {
            self.clear_selection();
        }
        // A double-click outranks both: egui reports it as a click too, and the
        // clear above would otherwise undo the word in the same frame it was
        // taken.
        if let Some((line, from, to)) = dbl_word {
            self.sel_anchor = Some((line, from));
            self.sel_cursor = Some((line, to));
        }
        toggled
    }


    // ── Investigation dock ────────────────────────────────────────────────────

    /// The status strip along the very bottom: what this session is attached to.
    ///
    /// Facts the developer would otherwise have to remember — which runtime,
    /// which thread, which frame the inspector is answering about. Every item
    /// is something the debugger actually knows; nothing here is decoration.
    fn status_bar(&self, ui: &mut egui::Ui, tr: &Tr) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            ui.add_space(4.0);
            let (dot, label, colour) = if self.is_paused {
                ("●", tr.dbg_state_paused, Color32::from_rgb(220, 180, 50))
            } else if self.source_lines.is_empty() {
                ("○", tr.dbg_state_disconnected, Color32::from_gray(120))
            } else {
                ("○", tr.dbg_state_running, Color32::from_rgb(80, 200, 80))
            };
            ui.label(
                RichText::new(format!("{dot} {label}"))
                    .size(12.0)
                    .color(colour),
            );
            ui.label(RichText::new("│").size(12.0).color(Color32::from_gray(70)));
            ui.label(
                RichText::new("COBOL source mapping ✓")
                    .size(12.0)
                    .color(Color32::from_gray(130)),
            );
            ui.label(RichText::new("│").size(12.0).color(Color32::from_gray(70)));
            // One interpreter per debuggee, so the thread list is one entry —
            // named after the program rather than called "Thread 1", which
            // would say nothing.
            let thread = self
                .frames
                .last()
                .map(|f| f.program.clone())
                .unwrap_or_else(|| "—".to_owned());
            ui.label(
                RichText::new(format!("Main thread · {thread}"))
                    .size(12.0)
                    .color(Color32::from_gray(130)),
            );
            ui.label(RichText::new("│").size(12.0).color(Color32::from_gray(70)));
            ui.label(
                RichText::new(format!("Frame {}", self.selected_frame))
                    .size(12.0)
                    .color(Color32::from_gray(130)),
            );
            if self.current_line > 0 {
                ui.label(RichText::new("│").size(12.0).color(Color32::from_gray(70)));
                ui.label(
                    RichText::new(format!("line {}", self.current_line))
                        .size(12.0)
                        .color(Color32::from_gray(130)),
                );
            }
        });
    }

    /// A dock line's absolute wall-clock time as `HH:MM:SS:mmm` in LOCAL time
    /// (operator: ISO-8601 time-of-day). Reconstructed from the wall clock at
    /// session start plus the line's millisecond offset, so it stays exactly in
    /// step with `at_ms` (which still orders the Timeline). If the session's
    /// wall clock is somehow missing, the offset itself is shown in the same
    /// HH:MM:SS:mmm shape rather than a blank.
    fn dock_stamp(&self, at_ms: u64) -> String {
        if let Some(start) = self.session_started_wall {
            let t = start + chrono::TimeDelta::milliseconds(at_ms as i64);
            return t.format("%H:%M:%S:%3f").to_string();
        }
        format!(
            "{:02}:{:02}:{:02}:{:03}",
            at_ms / 3_600_000,
            (at_ms / 60_000) % 60,
            (at_ms / 1000) % 60,
            at_ms % 1000,
        )
    }

    /// The bottom dock: debugger output, split by channel, plus a prompt.
    ///
    /// Returns any query the prompt produced. The prompt is the same evaluator
    /// the watches use, so anything that works in one works in the other.
    fn investigation_dock(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        let sk = self.skin.palette();
        ui.horizontal(|ui| {
            let counts = |c: cobolt_runtime::OutputChannel| {
                self.dock.iter().filter(|l| l.channel == c).count()
            };
            use cobolt_runtime::OutputChannel as Ch;
            for (tab, label, ch) in [
                (DockTab::Console, tr.dbg_console, Ch::Console),
                (DockTab::Events, tr.dbg_events, Ch::Events),
                (DockTab::FileIo, tr.dbg_file_io, Ch::FileIo),
                (DockTab::Problems, tr.dbg_problems, Ch::Problems),
                (DockTab::Timeline, tr.dbg_timeline, Ch::Timeline),
            ] {
                let n = if tab == DockTab::Timeline {
                    self.dock.len()
                } else {
                    counts(ch)
                };
                let text = if n > 0 {
                    format!("{label}  {n}")
                } else {
                    label.to_owned()
                };
                if pill_tab(ui, self.dock_tab == tab, &text, sk).clicked() {
                    self.dock_tab = tab;
                }
            }
            if ui
                .add(egui::Label::new(RichText::new("🗑").size(15.0)).sense(egui::Sense::click()))
                .on_hover_text("Clear")
                .clicked()
            {
                self.dock.clear();
            }
        });
        ui.separator();

        use cobolt_runtime::OutputChannel as Ch;
        let want = match self.dock_tab {
            DockTab::Console => Some(Ch::Console),
            DockTab::Events => Some(Ch::Events),
            DockTab::FileIo => Some(Ch::FileIo),
            DockTab::Problems => Some(Ch::Problems),
            // The Timeline is every channel in the order it happened — that is
            // what makes it a timeline rather than a sixth console.
            DockTab::Timeline => None,
        };

        // What the scroll area may take: everything left, LESS the prompt's own
        // row. 26 px was not enough for a `TextEdit` plus its spacing, so the
        // prompt was drawn past the panel's edge and arrived clipped in half
        // (operator, 2026-09-17). `PROMPT_H` is what it actually occupies, and
        // the panel is sized with the same number.
        let rows = ui.available_height() - if self.dock_tab == DockTab::Console {
            PROMPT_H
        } else {
            4.0
        };
        ScrollArea::vertical()
            .id_salt("dbg_dock_scroll")
            .max_height(rows.max(40.0))
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                // Courier New for the whole dock line — timestamp, channel tag
                // and text — so the columns line up on their own (operator).
                // Cloned Context so `font_id` can run while `ui` is borrowed
                // mutably in the loop below.
                let ctx = ui.ctx().clone();
                let mono12 = dock_font(&ctx, 12.0);
                let mono13 = dock_font(&ctx, 13.0);
                let mut any = false;
                for line in self.dock.iter().filter(|l| want.is_none_or(|c| l.channel == c)) {
                    any = true;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            RichText::new(self.dock_stamp(line.at_ms))
                                .font(mono12.clone())
                                .color(sk.chrome_dim),
                        );
                        if want.is_none() {
                            ui.label(
                                RichText::new(match line.channel {
                                    Ch::Console => "con",
                                    Ch::Events => "evt",
                                    Ch::FileIo => "i/o",
                                    Ch::Problems => "prb",
                                    Ch::Timeline => "tml",
                                })
                                .font(mono12.clone())
                                .color(Color32::from_gray(120)),
                            );
                        }
                        ui.label(
                            RichText::new(&line.text)
                                .font(mono13.clone())
                                .color(if line.channel == Ch::Problems {
                                    Color32::from_rgb(230, 140, 140)
                                } else {
                                    Color32::from_rgb(210, 216, 232)
                                }),
                        );
                    });
                }
                if !any {
                    ui.label(
                        RichText::new(tr.dbg_dock_empty).size(13.0).color(sk.chrome_dim),
                    );
                }
            });

        // The prompt. Only on the console, and only while stopped: evaluating
        // against a running program would answer about a moment that has passed.
        if self.dock_tab == DockTab::Console {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(">")
                        .monospace()
                        .color(Color32::from_rgb(120, 190, 255)),
                );
                // Always editable (operator): the developer can type an
                // expression whenever, not only while stopped. It is evaluated
                // against the current frame when a stop is in effect.
                let resp = ui.add(
                    TextEdit::singleline(&mut self.console_input)
                        .hint_text(tr.dbg_console_hint)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        // Double the box height (operator) by padding inside the
                        // field — the font is unchanged, the caret just sits in
                        // a taller well.
                        .margin(egui::Margin {
                            left: 8,
                            right: 8,
                            top: 11,
                            bottom: 11,
                        }),
                );
                if resp.has_focus() {
                    // ↑/↓ walk the history, as every console does.
                    let (up, down) = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::ArrowUp),
                            i.key_pressed(egui::Key::ArrowDown),
                        )
                    });
                    if up && !self.console_history.is_empty() {
                        let pos = match self.history_pos {
                            None => self.console_history.len() - 1,
                            Some(0) => 0,
                            Some(p) => p - 1,
                        };
                        self.history_pos = Some(pos);
                        self.console_input = self.console_history[pos].clone();
                    } else if down {
                        match self.history_pos {
                            Some(p) if p + 1 < self.console_history.len() => {
                                self.history_pos = Some(p + 1);
                                self.console_input = self.console_history[p + 1].clone();
                            }
                            Some(_) => {
                                self.history_pos = None;
                                self.console_input.clear();
                            }
                            None => {}
                        }
                    }
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let expression = self.console_input.trim().to_owned();
                    if !expression.is_empty() {
                        let at_ms = self
                            .session_started
                            .map(|t| t.elapsed().as_millis() as u64)
                            .unwrap_or(0);
                        self.dock.push(DockLine {
                            channel: cobolt_runtime::OutputChannel::Console,
                            text: format!("> {expression}"),
                            at_ms,
                        });
                        let id = self.next_query_id;
                        self.next_query_id += 1;
                        self.console_pending = Some(id);
                        let frame = self.selected_frame;
                        self.pending_ident_queries
                            .push((id, DebugQuery::Evaluate { frame, expression: expression.clone() }));
                        self.console_history.push(expression);
                        self.history_pos = None;
                        self.console_input.clear();
                    }
                }
            });
            ui.add_space(5.0); // bottom padding under the prompt (operator)
        }
    }

    // ── Tabbed data panel ─────────────────────────────────────────────────────

    fn data_tabs(&mut self, ui: &mut egui::Ui, tr: &Tr) {
        let sk = self.skin.palette();
        // Collected in the closure, applied after: every one of these mutates
        // state the tree is being read from.
        let mut pick_frame: Option<usize> = None;
        let mut toggle: Option<i64> = None;
        let mut fetch: Option<i64> = None;
        let mut want_scopes = false;
        let mut begin_edit: Option<(i64, String, String)> = None;
        let mut edit_buf: Option<String> = None;
        let mut commit: Option<(i64, String, String)> = None;
        let mut add_watch: Option<String> = None;
        let mut drop_watch: Option<usize> = None;
        let mut evaluate_watches: Vec<(usize, String)> = Vec::new();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // Three primary tabs only (spec §9): Variáveis, Observações, Pilha.
            for (tab, label) in [
                (Tab::Variables, tr.dbg_variables),
                (Tab::Watches, tr.dbg_watches),
                (Tab::CallStack, tr.dbg_call_stack),
            ] {
                if pill_tab(ui, self.active_tab == tab, label, sk).clicked() {
                    self.active_tab = tab;
                }
            }
            // Breakpoints is moved off the three-tab row (spec §9), reachable
            // from a small overflow control on the right — a painter-drawn
            // breakpoint dot, never an emoji.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let active = self.active_tab == Tab::Breakpoints;
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(26.0, 22.0), egui::Sense::click());
                if active {
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(8),
                        Color32::from_rgba_unmultiplied(sk.kw.r(), sk.kw.g(), sk.kw.b(), 40),
                    );
                }
                let col = if active { sk.kw } else { sk.chrome_dim };
                ui.painter().circle_filled(rect.center(), 4.5, col);
                let resp = resp.on_hover_text(tr.dbg_breakpoints);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    self.active_tab = Tab::Breakpoints;
                }
            });
        });
        ui.add_space(8.0);

        match self.active_tab {
            Tab::Variables => {
                ui.add(
                    TextEdit::singleline(&mut self.var_filter)
                        .hint_text(tr.dbg_filter_hint)
                        .desired_width(f32::INFINITY),
                );
                if let Some(err) = &self.inspect_error {
                    ui.label(
                        RichText::new(format!("⚠ {err}"))
                            .color(Color32::from_rgb(230, 120, 120))
                            .size(13.0),
                    );
                }
                ui.add_space(2.0);

                if !self.is_paused {
                    ui.label(
                        RichText::new(tr.dbg_state_running)
                            .color(Color32::from_gray(110))
                            .size(13.0),
                    );
                    return;
                }
                if self.scopes.is_empty() {
                    want_scopes = true;
                }

                // The visible tree, flattened to rows first. A table needs a
                // row COUNT before it draws, and flattening is also what lets
                // it virtualise: only the rows on screen are built, so a
                // WORKING-STORAGE of thousands of items costs what fits.
                let filter = self.var_filter.to_ascii_lowercase();
                let rows = self.flatten_visible(&filter);

                TableBuilder::new(ui)
                    .id_salt("dbg_inspect_table")
                    .striped(true)
                    // Draggable dividers, as the mockup asks. `allocate_ui`
                    // reserves width but does not make a child FILL it, so the
                    // hand-laid version collapsed every cell onto the next and
                    // read `COBOL-CONTROL-IDPIC X(64)SPACES`.
                    .resizable(true)
                    .column(Column::initial(190.0).at_least(90.0).resizable(true))
                    .column(Column::initial(110.0).at_least(60.0).resizable(true))
                    .column(Column::remainder().at_least(60.0))
                    .header(18.0, |mut header| {
                        for label in [tr.dbg_col_name, tr.dbg_col_pic, tr.dbg_col_value] {
                            header.col(|ui| {
                                ui.label(
                                    RichText::new(label)
                                        .size(13.0)
                                        .color(Color32::from_gray(150)),
                                );
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(18.0, rows.len(), |mut row| {
                            let r = &rows[row.index()];
                            row.col(|ui| {
                                let marker = if !r.expandable {
                                    "  "
                                } else if r.open {
                                    "⌄"
                                } else {
                                    "›"
                                };
                                let resp = ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "{}{marker} {}",
                                            "   ".repeat(r.depth),
                                            r.name
                                        ))
                                        .monospace()
                                        .size(13.0)
                                        .color(r.name_colour),
                                    )
                                    .sense(egui::Sense::click()),
                                );
                                if r.expandable {
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        toggle = Some(r.reference);
                                    }
                                }
                            });
                            row.col(|ui| {
                                ui.label(
                                    RichText::new(&r.type_text)
                                        .monospace()
                                        .size(12.0)
                                        .color(Color32::from_gray(140)),
                                );
                            });
                            row.col(|ui| {
                                let editing_this = matches!(
                                    &self.editing,
                                    Some((pr, n, _)) if *pr == r.parent && *n == r.name
                                );
                                if editing_this {
                                    if let Some((_, _, buf)) = self.editing.as_ref() {
                                        let mut text = buf.clone();
                                        let resp = ui.add(
                                            TextEdit::singleline(&mut text)
                                                .desired_width(f32::INFINITY)
                                                .font(egui::TextStyle::Monospace),
                                        );
                                        if text != *buf {
                                            edit_buf = Some(text.clone());
                                        }
                                        if resp.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                        {
                                            commit = Some((r.parent, r.name.clone(), text));
                                        }
                                    }
                                } else {
                                    let mut rt = RichText::new(&r.value_text)
                                        .monospace()
                                        .size(13.0)
                                        .color(r.value_colour);
                                    if r.value_italic {
                                        rt = rt.italics();
                                    }
                                    let v =
                                        ui.add(egui::Label::new(rt).sense(egui::Sense::click()));
                                    if r.editable {
                                        if v.hovered() {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                                        }
                                        if v.clicked() {
                                            begin_edit = Some((
                                                r.parent,
                                                r.name.clone(),
                                                r.raw_value.clone(),
                                            ));
                                        }
                                    }
                                }
                            });
                        });
                    });
            }

            Tab::Watches => {
                ui.horizontal(|ui| {
                    let resp = ui.add(
                        TextEdit::singleline(&mut self.watch_input)
                            .hint_text(tr.dbg_watch_hint)
                            .desired_width(f32::INFINITY),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !self.watch_input.trim().is_empty()
                    {
                        add_watch = Some(self.watch_input.trim().to_owned());
                    }
                });
                ui.add_space(2.0);

                if self.watches.is_empty() {
                    ui.label(
                        RichText::new(tr.dbg_watch_empty)
                            .color(Color32::from_gray(110))
                            .size(13.0),
                    );
                } else {
                    // Its own two-column table, as the mockup asks — the same
                    // shape as the inspector so the two read as one pane rather
                    // than a table above a list.
                    let watches: Vec<(String, Option<String>, Option<String>)> = self
                        .watches
                        .iter()
                        .map(|w| (w.expression.clone(), w.value.clone(), w.error.clone()))
                        .collect();
                    let paused = self.is_paused;
                    TableBuilder::new(ui)
                        .id_salt("dbg_watch_table")
                        .striped(true)
                        .resizable(true)
                        .column(Column::initial(220.0).at_least(90.0).resizable(true))
                        .column(Column::remainder().at_least(60.0))
                        .column(Column::exact(20.0))
                        .header(18.0, |mut header| {
                            for label in [tr.dbg_col_expression, tr.dbg_col_value] {
                                header.col(|ui| {
                                    ui.label(
                                        RichText::new(label)
                                            .size(13.0)
                                            .color(Color32::from_gray(150)),
                                    );
                                });
                            }
                            header.col(|_| {});
                        })
                        .body(|body| {
                            body.rows(18.0, watches.len(), |mut row| {
                                let i = row.index();
                                let (expr, value, error) = &watches[i];
                                row.col(|ui| {
                                    ui.label(
                                        RichText::new(expr)
                                            .monospace()
                                            .size(13.0)
                                            .color(Color32::from_rgb(215, 220, 235)),
                                    );
                                });
                                row.col(|ui| match (value, error) {
                                    (_, Some(e)) => {
                                        ui.label(
                                            RichText::new(e)
                                                .size(13.0)
                                                .italics()
                                                .color(Color32::from_rgb(230, 120, 120)),
                                        );
                                    }
                                    (Some(v), _) => {
                                        ui.label(
                                            RichText::new(v.trim())
                                                .monospace()
                                                .size(13.0)
                                                .color(Color32::from_rgb(240, 200, 120)),
                                        );
                                    }
                                    // Not evaluated at THIS stop. Saying so beats
                                    // showing the previous stop's answer, which
                                    // would be a stale reading presented as live.
                                    (None, None) => {
                                        ui.label(
                                            RichText::new(if paused { "…" } else { "—" })
                                                .size(13.0)
                                                .color(Color32::from_gray(110)),
                                        );
                                    }
                                });
                                row.col(|ui| {
                                    if ui
                                        .add(
                                            egui::Label::new(
                                                RichText::new("🗑")
                                                    .size(14.0)
                                                    .color(Color32::from_gray(120)),
                                            )
                                            .sense(egui::Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        drop_watch = Some(i);
                                    }
                                });
                            });
                        });
                }

                if self.is_paused {
                    for (i, w) in self.watches.iter().enumerate() {
                        if w.value.is_none() && w.error.is_none() {
                            evaluate_watches.push((i, w.expression.clone()));
                        }
                    }
                }
            }

            Tab::CallStack => {
                ScrollArea::vertical()
                    .id_salt("dbg_stack_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.frames.is_empty() {
                            ui.label(
                                RichText::new(tr.dbg_no_frame).color(sk.chrome_dim),
                            );
                            return;
                        }
                        // Innermost first, as the interpreter sends it. Inline
                        // PERFORM loops are already filtered out upstream: they
                        // carry step depth, not a call.
                        let selected = self.selected_frame;
                        for (i, f) in self.frames.iter().enumerate() {
                            let is_top = i == selected;
                            let resp = ui.add(
                                egui::Label::new(
                                    RichText::new(format!(
                                        "{} {}",
                                        if is_top { "►" } else { " " },
                                        f.display_name()
                                    ))
                                    .monospace()
                                    .size(14.0)
                                    .color(if is_top {
                                        Color32::from_rgb(230, 180, 40)
                                    } else if f.generated {
                                        // Generated scaffolding is greyed, not
                                        // hidden: the developer can still see
                                        // the path their call actually took.
                                        Color32::from_gray(110)
                                    } else {
                                        Color32::from_rgb(120, 190, 255)
                                    }),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                pick_frame = Some(i);
                            }
                            if f.line > 0 {
                                ui.label(
                                    RichText::new(format!(
                                        "     {:?}  line {}",
                                        f.kind, f.line
                                    ))
                                    .monospace()
                                    .size(12.0)
                                    .color(Color32::from_gray(110)),
                                );
                            }
                        }
                    });
            }

            Tab::Breakpoints => {
                // A named column heading, not a bare bullet (operator).
                ui.label(RichText::new(tr.dbg_breakpoints).strong().color(sk.chrome));
                ui.add_space(4.0);
                ScrollArea::vertical()
                    .id_salt("dbg_bp_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.breakpoints.is_empty() {
                            ui.label(
                                RichText::new(tr.dbg_no_breakpoints).color(sk.chrome_dim),
                            );
                        } else {
                            let mut sorted: Vec<u32> = self.breakpoints.iter().cloned().collect();
                            sorted.sort_unstable();
                            egui::Grid::new("dbg_bp_grid")
                                .num_columns(3)
                                .striped(true)
                                .show(ui, |ui| {
                                    for line in &sorted {
                                        // The red dot — the "o" of the gutter,
                                        // so a row reads the same as the code.
                                        ui.label(
                                            RichText::new("●")
                                                .color(Color32::from_rgb(210, 50, 50)),
                                        );
                                        // `line N - <excerpt>`: the code the
                                        // breakpoint sits on, trimmed and
                                        // clipped so a long statement never
                                        // stretches the dock (operator).
                                        let excerpt = self
                                            .source_lines
                                            .get((*line as usize).saturating_sub(1))
                                            .map(|s| bp_line_excerpt(s))
                                            .unwrap_or_default();
                                        let text = if excerpt.is_empty() {
                                            format!("line {line}")
                                        } else {
                                            format!("line {line} - {excerpt}")
                                        };
                                        ui.label(
                                            RichText::new(text)
                                                .monospace()
                                                .color(Color32::from_rgb(100, 180, 255)),
                                        );
                                        // The ✕ clears this breakpoint — the
                                        // same toggle a gutter click raises,
                                        // routed out through `bp_remove_request`.
                                        let x = ui.add(
                                            egui::Button::new(
                                                RichText::new("✕").color(sk.chrome_dim),
                                            )
                                            .frame(false),
                                        );
                                        if x.on_hover_text(tr.dbg_remove_breakpoint).clicked() {
                                            self.bp_remove_request = Some(*line);
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
            }
        }

        if let Some(i) = pick_frame {
            // Changing the selected frame changes what the inspector,
            // watches and evaluations resolve against, so its answers go too.
            self.selected_frame = i;
            self.scopes.clear();
            self.rows.clear();
            self.inflight.clear();
        }
        if let Some(r) = toggle {
            if self.open.contains(&r) {
                self.open.remove(&r);
            } else {
                self.open.insert(r);
                // Opening a row it has never seen is what triggers the fetch —
                // this is the whole "lazy" in lazy expansion.
                if !self.rows.contains_key(&r) {
                    fetch = Some(r);
                }
            }
        }
        if let Some((r, name, seed)) = begin_edit {
            self.editing = Some((r, name, seed));
        }
        if let (Some(text), Some(cur)) = (edit_buf, self.editing.as_mut()) {
            cur.2 = text;
        }
        if let Some((reference, name, value)) = commit {
            self.pending_queries.push(DebugQuery::SetVariable {
                reference,
                name,
                value,
            });
        }
        if want_scopes && !self.inflight.contains(&0) {
            self.inflight.insert(0);
            let frame = self.selected_frame;
            self.pending_queries.push(DebugQuery::Scopes { frame });
        }
        if let Some(reference) = fetch {
            if self.inflight.insert(reference) {
                self.pending_queries.push(DebugQuery::Variables { reference });
            }
        }
        if let Some(expr) = add_watch {
            self.watches.push(Watch {
                expression: expr,
                value: None,
                error: None,
            });
            self.watch_input.clear();
        }
        if let Some(i) = drop_watch {
            if i < self.watches.len() {
                self.watches.remove(i);
            }
        }
        for (i, expression) in evaluate_watches {
            // One query per watch, tracked by id: several are in flight at once
            // at every stop, so "the only outstanding one" cannot match them.
            if self.watch_pending.values().any(|p| *p == i) {
                continue;
            }
            let id = self.next_query_id;
            self.next_query_id += 1;
            self.watch_pending.insert(id, i);
            let frame = self.selected_frame;
            self.pending_ident_queries.push((id, DebugQuery::Evaluate { frame, expression }));
        }
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn current_line(&self) -> u32 {
        self.current_line
    }

    fn is_generated_control_handler_var(var: &VarSnapshot) -> bool {
        let name = var.name.to_ascii_uppercase();
        name.starts_with("COBOL-")
            || name == "FORM-NAME"
            || name.starts_with("WS-ANIM-")
            || (name.starts_with("WS-") && name.contains("-SELECTED-"))
    }

    fn variable_value_window(&mut self, ctx: &Context) {
        let Some(var) = self.selected_var.clone() else {
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Data item ({})", var.name))
            .id(egui::Id::new("debugger_data_item_value"))
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(760.0, 460.0))
            .min_size(egui::vec2(460.0, 300.0))
            .open(&mut open)
            .show(ctx, |ui| {
                egui::Grid::new("debugger_data_item_details")
                    .num_columns(2)
                    .spacing([10.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("PIC:");
                        ui.label(if var.pic.is_empty() { "-" } else { &var.pic });
                        ui.end_row();
                        ui.strong("Scope:");
                        ui.label(&var.scope);
                        ui.end_row();
                        ui.strong("Origin:");
                        ui.label(&var.origin);
                        ui.end_row();
                    });

                ui.separator();

                let body_h = ui.available_height().max(160.0);
                TableBuilder::new(ui)
                    .id_salt("debugger_data_item_value_split")
                    .resizable(true)
                    .vscroll(false)
                    .auto_shrink([false, false])
                    .cell_layout(egui::Layout::top_down(egui::Align::Min))
                    .column(Column::remainder().at_least(180.0).resizable(true))
                    .column(Column::remainder().at_least(180.0).resizable(true))
                    .header(22.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Value");
                        });
                        header.col(|ui| {
                            ui.strong("Hex representation");
                        });
                    })
                    .body(|mut body| {
                        body.row(body_h, |mut row| {
                            row.col(|ui| {
                                ScrollArea::vertical()
                                    .id_salt("debugger_data_item_value_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.add(
                                            egui::Label::new(RichText::new(&var.value).monospace())
                                                .wrap(),
                                        );
                                    });
                            });
                            row.col(|ui| {
                                ScrollArea::vertical()
                                    .id_salt("debugger_data_item_hex_scroll")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(Self::hex_dump(&var.value)).monospace(),
                                        );
                                    });
                            });
                        });
                    });
            });

        if !open {
            self.selected_var = None;
        }
    }

    fn fit_value_preview(ui: &egui::Ui, value: &str, max_px: f32) -> String {
        let font_id = egui::FontId::monospace(13.0);
        let text_width = |text: &str| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(text.to_owned(), font_id.clone(), Color32::WHITE)
                    .size()
                    .x
            })
        };

        if text_width(value) <= max_px {
            return value.to_owned();
        }

        let mut out = String::new();
        for ch in value.chars() {
            let candidate = format!("{out}{ch}...");
            if text_width(&candidate) > max_px {
                break;
            }
            out.push(ch);
        }
        if out.is_empty() {
            "...".to_owned()
        } else {
            format!("{out}...")
        }
    }

    fn hex_dump(value: &str) -> String {
        value
            .as_bytes()
            .chunks(16)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── COBOL syntax highlighter ──────────────────────────────────────────────────

const COBOL_KEYWORDS: &[&str] = &[
    "ACCEPT",
    "ADD",
    "ALL",
    "AND",
    "BINARY",
    "BY",
    "CALL",
    "CLOSE",
    "COMP",
    "COMP-3",
    "COMP-4",
    "COMPUTE",
    "CONFIGURATION",
    "DATA",
    "DELETE",
    "DEPENDING",
    "DISPLAY",
    "DIVIDE",
    "END-CALL",
    "END-COMPUTE",
    "END-EVALUATE",
    "END-IF",
    "END-PERFORM",
    "END-READ",
    "END-WRITE",
    "ENVIRONMENT",
    "EQUAL",
    "ERROR",
    "EVALUATE",
    "EXIT",
    "EXTEND",
    "FROM",
    "GIVING",
    "GLOBAL",
    "GOBACK",
    "GREATER",
    "HIGH-VALUE",
    "HIGH-VALUES",
    "I-O",
    "IDENTIFICATION",
    "IF",
    "INITIALIZE",
    "INPUT",
    "INPUT-OUTPUT",
    "INSPECT",
    "IS",
    "LESS",
    "LINKAGE",
    "LOW-VALUE",
    "LOW-VALUES",
    "MOVE",
    "MULTIPLY",
    "NOT",
    "OCCURS",
    "OF",
    "ON",
    "OPEN",
    "OR",
    "OTHER",
    "OUTPUT",
    "OVERFLOW",
    "PACKED-DECIMAL",
    "PERFORM",
    "PIC",
    "PICTURE",
    "PROCEDURE",
    "PROGRAM",
    "PROGRAM-ID",
    "READ",
    "REDEFINES",
    "REMAINDER",
    "REWRITE",
    "ROUNDED",
    "RUN",
    "SECTION",
    "SET",
    "SIZE",
    "SPACE",
    "SPACES",
    "START",
    "STOP",
    "STRING",
    "SUBTRACT",
    "THAN",
    "THEN",
    "TIMES",
    "TO",
    "UNSTRING",
    "USING",
    "VALUE",
    "VALUES",
    "WHEN",
    "WORKING-STORAGE",
    "WRITE",
    "ZERO",
    "ZEROES",
    "ZEROS",
    "COMMON",
    "DIVISION",
    "ELSE",
];

fn is_cobol_keyword(word: &str) -> bool {
    let upper = word.to_ascii_uppercase();
    COBOL_KEYWORDS.iter().any(|&kw| kw == upper.as_str())
}

/// The COBOL-85 statement verbs — the words that OPEN an executable statement.
/// A breakpoint belongs only on a line that starts with one of these: never a
/// division or section header, a paragraph name, a data item, a scope
/// terminator (`END-IF`), a clause continuation (`WHEN`, `INTO`) or a comment.
const COBOL_STMT_VERBS: &[&str] = &[
    "ACCEPT", "ADD", "ALTER", "CALL", "CANCEL", "CLOSE", "COMPUTE", "CONTINUE",
    "DELETE", "DISPLAY", "DIVIDE", "EVALUATE", "EXEC", "EXIT", "GO", "GOBACK",
    "IF", "INITIALIZE", "INSPECT", "INVOKE", "MERGE", "MOVE", "MULTIPLY", "OPEN",
    "PERFORM", "READ", "RELEASE", "RETURN", "REWRITE", "SEARCH", "SET", "SORT",
    "START", "STOP", "STRING", "SUBTRACT", "UNLOCK", "UNSTRING", "WRITE",
];

/// `true` when a breakpoint may be set on `line`. Two shapes qualify, and only
/// these two (operator):
///
///  1. A line that OPENS an executable statement — its first word is a COBOL
///     verb.
///  2. An inline method invocation — `grid-1::rows::getItem(1)` — which begins
///     with an object name, not a verb, yet is a real statement the debuggee
///     stops on. The `::` after the first identifier is the tell.
///
/// Everything else — a division or section header, a paragraph name, a data
/// item, a scope terminator, a clause continuation, a comment — shows no ring
/// and swallows no click, because the program never stops on it.
/// Whether the OS provides the "Courier New" font. Cached in a `OnceLock`:
/// `system_fonts()` enumerates the whole system list, far too much to redo per
/// dock line per frame.
fn courier_available() -> bool {
    static AVAIL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAIL.get_or_init(|| {
        crate::fonts::system_fonts()
            .iter()
            .any(|f| f.eq_ignore_ascii_case("Courier New"))
    })
}

/// The investigation dock's monospace font at `size`: Courier New where the OS
/// has it (operator), so the columns line up on their own; egui's own
/// monospace otherwise. Never the proportional fallback `font_id` gives for an
/// absent family — that would defeat the alignment the dock exists to provide.
fn dock_font(ctx: &egui::Context, size: f32) -> egui::FontId {
    if courier_available() {
        crate::fonts::font_id(ctx, "Courier New", size)
    } else {
        egui::FontId::monospace(size)
    }
}

/// A one-line preview of the code a breakpoint sits on, for the Breakpoints
/// list: trimmed of its fixed-form indentation and clipped so a long statement
/// does not stretch the dock. A trailing `…` marks a clip.
fn bp_line_excerpt(line: &str) -> String {
    const MAX: usize = 44;
    let trimmed = line.trim();
    if trimmed.chars().count() > MAX {
        let mut s: String = trimmed.chars().take(MAX).collect();
        s.push('…');
        s
    } else {
        trimmed.to_owned()
    }
}

fn line_allows_breakpoint(line: &str) -> bool {
    let trimmed = line.trim_start();
    let first: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if COBOL_STMT_VERBS.contains(&first.to_ascii_uppercase().as_str()) {
        return true;
    }
    // `<identifier>::` — an inline invocation. The first token must be a real
    // identifier (so a bare `::` or a line starting with punctuation does not
    // qualify), and `::` must follow it, allowing for spacing.
    !first.is_empty() && trimmed[first.len()..].trim_start().starts_with("::")
}

fn build_cobol_layout_job(line: &str) -> egui::text::LayoutJob {
    build_cobol_layout_job_inked(line, DebugSkin::default().palette(), None, 12.0)
}

/// The same colouring, except that `ink` — when given — overrides every colour
/// in it.
///
/// The stopped line is painted on a lime band, and a syntax palette tuned for a
/// dark editor is close to unreadable there: the comment green and the string
/// brown both sit near the band's own luminance. One high-contrast ink for that
/// one line is worth more than the colouring it replaces.
fn build_cobol_layout_job_inked(
    line: &str,
    sk: Skin,
    ink: Option<Color32>,
    font_pt: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    // Disable wrapping — code lines extend to the right.
    job.wrap.max_width = f32::INFINITY;

    let mono = egui::FontId::monospace(font_pt);
    let paint = |c: Color32| ink.unwrap_or(c);
    let col_kw = paint(sk.kw);
    let col_str = paint(sk.lit);
    let col_cmt = paint(sk.cmt);
    let col_num = paint(sk.num);
    let col_def = paint(sk.ink);

    let trimmed = line.trim_start();
    if trimmed.starts_with("*>") {
        job.append(
            line,
            0.0,
            egui::text::TextFormat {
                font_id: mono,
                color: col_cmt,
                ..Default::default()
            },
        );
        return job;
    }

    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    macro_rules! fmt {
        ($color:expr) => {
            egui::text::TextFormat {
                font_id: mono.clone(),
                color: $color,
                ..Default::default()
            }
        };
    }

    while i < len {
        if chars[i] == '"' || chars[i] == '\'' {
            // A quoted literal is ONE colour end to end (operator): its content
            // is text, not code, so reserved words inside it are never
            // keyword-coloured. COBOL allows both quote styles, and a literal
            // closes only on its own quote.
            let quote = chars[i];
            let start = i;
            i += 1;
            while i < len && chars[i] != quote {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_str),
            );
        } else if chars[i].is_alphabetic() {
            let start = i;
            i += 1;
            while i < len {
                if chars[i].is_alphanumeric() {
                    i += 1;
                } else if chars[i] == '-' && i + 1 < len && chars[i + 1].is_alphanumeric() {
                    i += 1;
                } else {
                    break;
                }
            }
            let word: String = chars[start..i].iter().collect();
            let color = if is_cobol_keyword(&word) {
                col_kw
            } else {
                col_def
            };
            job.append(&word, 0.0, fmt!(color));
        } else if chars[i].is_ascii_digit()
            || (chars[i] == '-' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            if chars[i] == '-' {
                i += 1; // sign of a negative literal, e.g. BY -1
            }
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_num),
            );
        } else {
            let start = i;
            while i < len
                && !chars[i].is_alphabetic()
                && chars[i] != '"'
                && chars[i] != '\''
                && !chars[i].is_ascii_digit()
            {
                if chars[i] == '-' && i + 1 < len && chars[i + 1].is_alphanumeric() {
                    break;
                }
                i += 1;
            }
            // Guarantee forward progress: if the loop broke on its first char
            // (e.g. a `-` right before an alphanumeric), consume that char —
            // otherwise the outer loop would never advance and the UI would
            // spin forever appending empty sections.
            if i == start {
                i += 1;
            }
            job.append(
                &chars[start..i].iter().collect::<String>(),
                0.0,
                fmt!(col_def),
            );
        }
    }

    job
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A breakpoint may be set only on a line that starts a COBOL statement.
    #[test]
    fn breakpoints_on_statement_verbs_and_inline_invocations() {
        for ok in [
            "           MOVE X TO Y",
            "       IF WS-X = 1",
            "           STOP RUN.",
            "           CONTINUE.",
            "           PERFORM COBOL-EVENT-LOOP",
            "           CALL \"X\"",
            // Inline method invocations open with an object, not a verb, yet
            // are real statements the debuggee stops on (operator).
            "           grid-1::rows::getItem(1)",
            "           SNACKBAR-1::show",
        ] {
            assert!(line_allows_breakpoint(ok), "should allow: {ok:?}");
        }
        for no in [
            "       PROCEDURE DIVISION.",
            "       COBOL-MAIN.",
            "       01 WS-ROW-COUNT PIC 9(9).",
            "       WORKING-STORAGE SECTION.",
            "      *> a comment",
            "",
            "           END-IF",
            "               WHEN \"retry\"",
            "           MOVE-FLAG", // a data name, not the MOVE verb
        ] {
            assert!(!line_allows_breakpoint(no), "should refuse: {no:?}");
        }
    }

    /// Regression: a `-` directly followed by an alphanumeric (negative literal
    /// like `BY -1`) used to make the tokenizer loop forever with zero progress,
    /// freezing the whole IDE the moment the debugger rendered that line.
    #[test]
    fn layout_job_terminates_on_negative_literal() {
        let job = build_cobol_layout_job(
            "PERFORM VARYING I FROM FUNCTION LENGTH(WS-TXT) BY -1 UNTIL I < 1",
        );
        assert!(!job.text.is_empty());
    }

    #[test]
    fn layout_job_highlights_negative_number() {
        let job = build_cobol_layout_job("MOVE -12.5 TO WS-X");
        assert_eq!(job.text, "MOVE -12.5 TO WS-X");
    }

    #[test]
    fn layout_job_terminates_on_dash_before_letter() {
        let job = build_cobol_layout_job("COMPUTE X = Y -Z");
        assert_eq!(job.text, "COMPUTE X = Y -Z");
    }

    /// The window knows which file its breakpoints belong to.
    ///
    /// The debugger shows the **generated** `.cbl`, which is rarely the tab in
    /// front — so a gutter click has to be recorded against this path, not the
    /// active editor tab. `set_breakpoints` also has to actually take, because
    /// it had no callers at all and the panel's set went stale the moment a
    /// breakpoint moved.
    #[test]
    fn the_window_reports_the_file_its_breakpoints_belong_to() {
        let mut p = DebuggerPanel::new();
        assert_eq!(p.source_path(), "", "no session yet");

        let mut bps = HashSet::new();
        bps.insert(7u32);
        p.set_source("/proj/generated/form1.cbl".into(), "A\nB\nC\n", &bps);
        assert_eq!(p.source_path(), "/proj/generated/form1.cbl");
        assert!(p.breakpoints.contains(&7));

        // A later sync replaces the set wholesale — add and remove both land.
        let mut moved = HashSet::new();
        moved.insert(12u32);
        p.set_breakpoints(&moved);
        assert!(p.breakpoints.contains(&12));
        assert!(!p.breakpoints.contains(&7), "the old line is gone");

        // Reset keeps the source and its breakpoints: it clears the RUN, not
        // the developer's marks.
        p.reset();
        assert_eq!(p.source_path(), "/proj/generated/form1.cbl");
        assert!(p.breakpoints.contains(&12));
    }
}

/// One line of the flattened inspector tree, ready for a table row.
///
/// Built fresh each frame from the scopes, the fetched rows and which handles
/// are open. Flattening first is what lets the table virtualise — it needs a
/// row COUNT before it draws — and it keeps every column's content decided in
/// one place rather than inside three separate cell closures.
struct FlatRow {
    depth: usize,
    name: String,
    /// The handle this row expands to, 0 when it does not expand.
    reference: i64,
    /// The handle this row was LISTED under — what an edit is addressed by.
    parent: i64,
    expandable: bool,
    open: bool,
    editable: bool,
    type_text: String,
    value_text: String,
    raw_value: String,
    value_italic: bool,
    name_colour: Color32,
    value_colour: Color32,
}

impl DebuggerPanel {
    /// Flatten the open parts of the tree, applying the filter.
    ///
    /// A scope always shows: hiding an empty WORKING-STORAGE because its rows
    /// have not arrived yet would make the pane look broken while it loads.
    fn flatten_visible(&self, filter: &str) -> Vec<FlatRow> {
        let mut out = Vec::new();
        for sc in &self.scopes {
            let open = self.open.contains(&sc.reference);
            out.push(FlatRow {
                depth: 0,
                name: sc.name.clone(),
                reference: sc.reference,
                parent: 0,
                expandable: true,
                open,
                editable: false,
                type_text: format!("({})", sc.count),
                value_text: String::new(),
                raw_value: String::new(),
                value_italic: false,
                name_colour: Color32::from_rgb(150, 175, 215),
                value_colour: Color32::from_gray(140),
            });
            if open {
                self.flatten_children(sc.reference, 1, filter, &mut out);
            }
        }
        out
    }

    fn flatten_children(&self, reference: i64, depth: usize, filter: &str, out: &mut Vec<FlatRow>) {
        let Some(rows) = self.rows.get(&reference) else {
            // Asked for, not yet answered. Say so rather than render nothing,
            // which reads as "this group is empty".
            out.push(FlatRow {
                depth,
                name: "…".into(),
                reference: 0,
                parent: reference,
                expandable: false,
                open: false,
                editable: false,
                type_text: String::new(),
                value_text: String::new(),
                raw_value: String::new(),
                value_italic: false,
                name_colour: Color32::from_gray(110),
                value_colour: Color32::from_gray(110),
            });
            return;
        };
        for row in rows {
            if !filter.is_empty()
                && !row.name.to_ascii_lowercase().contains(filter)
                && !row.value.to_ascii_lowercase().contains(filter)
            {
                continue;
            }
            let expandable = row.reference != 0;
            let open = expandable && self.open.contains(&row.reference);

            // The PIC / Type column says what the row IS when it has no PICTURE
            // of its own — a column that is blank on half its rows is not a
            // column.
            let type_text = if !row.pic.is_empty() {
                format!("PIC {}", row.pic)
            } else if row.category == "group" {
                "Group".to_owned()
            } else if row.category == "condition" {
                "Condition".to_owned()
            } else if let Some(n) = row.occurs {
                format!("OCCURS {n} TIMES")
            } else {
                String::new()
            };

            // A "non-value" is named, never blank: an empty string, SPACES,
            // LOW-VALUES and HIGH-VALUES all look like nothing and mean four
            // different things.
            let (value_text, value_colour, value_italic) = match row.special {
                Some(SpecialValue::EmptyString) => {
                    ("(empty)".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::Spaces) => ("SPACES".to_owned(), Color32::from_gray(120), true),
                Some(SpecialValue::LowValues) => {
                    ("LOW-VALUES".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::HighValues) => {
                    ("HIGH-VALUES".to_owned(), Color32::from_gray(120), true)
                }
                Some(SpecialValue::Unset) => ("(unset)".to_owned(), Color32::from_gray(120), true),
                Some(SpecialValue::EvaluationError) => {
                    (row.value.clone(), Color32::from_rgb(230, 120, 120), true)
                }
                None => (
                    row.value.trim().to_owned(),
                    if row.value.trim() == "TRUE" {
                        Color32::from_rgb(120, 210, 140)
                    } else {
                        Color32::from_rgb(240, 200, 120)
                    },
                    false,
                ),
            };

            out.push(FlatRow {
                depth,
                name: row.name.clone(),
                reference: row.reference,
                parent: reference,
                expandable,
                open,
                editable: row.editable,
                type_text,
                value_text,
                raw_value: row.value.trim().to_owned(),
                value_italic,
                name_colour: if row.category == "condition" {
                    Color32::from_rgb(190, 160, 230)
                } else if row.category == "group" {
                    Color32::from_rgb(150, 175, 215)
                } else {
                    Color32::from_rgb(215, 220, 235)
                },
                value_colour,
            });
            if open {
                self.flatten_children(row.reference, depth + 1, filter, out);
            }
        }
    }
}

#[cfg(test)]
mod animate_breakpoint_tests {
    use super::*;

    fn stopped(line: u32, reason: StopReason) -> DebugEvent {
        DebugEvent::Stopped {
            line,
            col: 1,
            paragraph: String::new(),
            reason,
            frames: Vec::new(),
        }
    }

    /// Animate steps on a timer until a breakpoint is reached; then the
    /// toggle goes off and the program waits for the developer. It used to
    /// step straight through the breakpoint on the next tick (operator,
    /// 2026-09-19).
    #[test]
    fn a_breakpoint_switches_animate_off_and_a_plain_step_does_not() {
        let mut p = DebuggerPanel::new();
        p.set_breakpoints(&[10_u32].into_iter().collect());

        // A plain step on an unmarked line keeps animating.
        p.animate = true;
        p.apply_event(stopped(11, StopReason::Step));
        assert!(p.animate, "a step on a line without a breakpoint keeps animating");

        // The interpreter's own verdict.
        p.apply_event(stopped(12, StopReason::Breakpoint(vec![12])));
        assert!(!p.animate, "a breakpoint stop switches Animate off");

        // The panel's own set, even when the stop was reported as a step.
        p.animate = true;
        p.apply_event(stopped(10, StopReason::Step));
        assert!(!p.animate, "a step landing on a marked line is a breakpoint to the developer");
        assert!(p.last_animate_step.is_none(), "the timer is reset with the toggle");
    }
}

#[cfg(test)]
mod animate_control_tests {
    use super::*;

    /// While Animate runs, `is_paused` flips at the animation's own rate —
    /// stopped between steps, running during one. Every button gated on it
    /// strobed with it, Pause above all: the developer could not reliably
    /// click the one control that stops the animation (operator,
    /// 2026-09-19). Animating, both sides of the toolbar stay live.
    #[test]
    fn the_toolbar_does_not_strobe_while_animating() {
        let mut p = DebuggerPanel::new();

        // Running, not animating: Pause is live, the steps are not.
        p.is_paused = false;
        p.animate = false;
        assert!(p.pause_available(), "a running program can be paused");
        assert!(!p.stopped_controls_available(), "a running program cannot be stepped");

        // Stopped, not animating: the reverse.
        p.is_paused = true;
        assert!(!p.pause_available(), "a stopped program has nothing to pause");
        assert!(p.stopped_controls_available(), "a stopped program can be stepped");

        // Animating: both stay live whichever way `is_paused` happens to be
        // flipped at the instant the frame is painted.
        p.animate = true;
        for paused in [true, false] {
            p.is_paused = paused;
            assert!(
                p.pause_available(),
                "Pause must stay clickable while animating (is_paused={paused})"
            );
            assert!(
                p.stopped_controls_available(),
                "the steps must stay clickable while animating (is_paused={paused})"
            );
        }
    }

    /// Pause ends the animation; pressing Animate again resumes stepping from
    /// wherever the program stopped.
    #[test]
    fn pause_ends_the_animation_and_animate_resumes_it() {
        let ctx = egui::Context::default();
        let mut p = DebuggerPanel::new();
        p.is_paused = true;
        p.animate = true;

        assert!(
            matches!(p.maybe_animate_step(&ctx), Some(DebugAction::StepOver)),
            "an armed animation steps on its first tick"
        );

        // The developer presses Pause.
        p.is_paused = true;
        p.on_pause_pressed();
        assert!(!p.animate, "Pause ends the animation");
        assert!(p.last_animate_step.is_none(), "and resets its timer");
        assert!(
            p.maybe_animate_step(&ctx).is_none(),
            "a paused animation issues no further steps"
        );

        // Pressing Animate again resumes it.
        p.animate = true;
        assert!(
            matches!(p.maybe_animate_step(&ctx), Some(DebugAction::StepOver)),
            "Animate pressed again resumes stepping"
        );
    }
}

#[cfg(test)]
mod hover_value_tests {
    use super::{hover_tip, lookup_value, word_at};
    use cobolt_runtime::VarSnapshot;

    fn v(name: &str, value: &str) -> VarSnapshot {
        VarSnapshot {
            name: name.into(),
            scope: "WS".into(),
            pic: String::new(),
            origin: String::new(),
            value: value.into(),
        }
    }

    /// The whole point of the tooltip: the pointer lands somewhere in a word,
    /// and the word is what gets looked up — not the character under it.
    #[test]
    fn a_word_is_found_from_anywhere_inside_it() {
        let line = "           COMPUTE Gauge-1::Value = Gauge-1::Value + 1";
        for at in [19, 24, 30] {
            assert_eq!(word_at(line, at).as_deref(), Some("Gauge-1::Value"), "at {at}");
        }
        // A COBOL name keeps its hyphens.
        assert_eq!(word_at("    MOVE WS-CUST-NO TO X.", 12).as_deref(), Some("WS-CUST-NO"));
    }

    /// Between words there is nothing to ask about, and a run of punctuation is
    /// not a name however word-shaped its characters are.
    #[test]
    fn punctuation_and_gaps_name_nothing() {
        let line = "    MOVE  A  TO  B.";
        assert_eq!(word_at(line, 8), None, "a space");
        assert_eq!(word_at("  ::  ", 2), None, "a bare separator");
        assert_eq!(word_at("", 0), None, "an empty line");
        // Past the end clamps to the last character rather than panicking —
        // here a space, so there is nothing to report.
        assert_eq!(word_at("  AB  ", 999), None);
        // …and clamping still finds a word when the line ends in one.
        assert_eq!(word_at("  AB", 999).as_deref(), Some("AB"));
    }

    /// A control property is asked about whole, then by the control it belongs
    /// to — so a runtime that reports only one of the two still answers.
    #[test]
    fn a_property_falls_back_to_its_control() {
        let whole = vec![v("Gauge-1::Value", "42")];
        assert_eq!(lookup_value("Gauge-1::Value", &whole), Some(("Gauge-1::Value", "42")));

        let control = vec![v("GAUGE-1", "42")];
        assert_eq!(lookup_value("Gauge-1::Value", &control), Some(("GAUGE-1", "42")));

        assert_eq!(lookup_value("Gauge-2::Value", &control), None);
    }

    /// A subscripted slot is a storage key; the line names its parent.
    /// An empty value is not worth a tooltip.
    #[test]
    fn a_subscript_is_stripped_and_an_empty_value_is_no_answer() {
        let vars = vec![v("WS-ROW(3)", "seven"), v("WS-BLANK", "   ")];
        assert_eq!(lookup_value("ws-row", &vars), Some(("WS-ROW", "seven")));
        assert_eq!(lookup_value("WS-BLANK", &vars), None);
    }

    /// Long values are cut so the tooltip stays readable — counted in
    /// characters, because a value carrying accents has more bytes than columns
    /// and cutting by byte would split one in half.
    ///
    /// Written against `TIP_VALUE_CHARS` rather than a literal: the cap moved
    /// from 18 to 100 on operator instruction (2026-09-17), and a test that
    /// spells the number out has to be edited every time it does — which is how
    /// a test starts pinning yesterday's decision.
    #[test]
    fn a_long_value_is_cut_at_the_tooltip_cap() {
        use super::TIP_VALUE_CHARS;
        assert_eq!(hover_tip("WS-X", "short"), "WS-X = short");
        // Exactly the cap is not long.
        let at_cap: String = "1".repeat(TIP_VALUE_CHARS);
        assert_eq!(hover_tip("WS-X", &at_cap), format!("WS-X = {at_cap}"));
        // One over it is.
        let over: String = "1".repeat(TIP_VALUE_CHARS + 1);
        assert_eq!(hover_tip("WS-X", &over), format!("WS-X = {at_cap}..."));
        // Accented characters: cut at the character, never mid-byte.
        let accented = "\u{e7}\u{f5}".repeat(TIP_VALUE_CHARS);
        let tip = hover_tip("WS-X", &accented);
        assert!(tip.ends_with("..."), "{tip}");
        assert_eq!(tip.chars().count(), "WS-X = ".len() + TIP_VALUE_CHARS + 3);
    }
}

#[cfg(test)]
mod inline_value_tests {
    use super::inline_values;
    use cobolt_runtime::VarSnapshot;

    fn v(name: &str, value: &str) -> VarSnapshot {
        VarSnapshot {
            name: name.into(),
            scope: "WS".into(),
            pic: String::new(),
            origin: String::new(),
            value: value.into(),
        }
    }

    /// The bug this kind of matcher always has: a short name found INSIDE a
    /// longer one. `WS-N` must not annotate a line that only mentions
    /// `WS-NAME`.
    #[test]
    fn a_name_matches_only_as_a_whole_word() {
        let vars = vec![v("WS-N", "7"), v("WS-NAME", "ADA")];
        let hits = inline_values("    MOVE WS-NAME TO WS-OUT.", &vars, 4);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].0, "WS-NAME");

        let hits = inline_values("    ADD 1 TO WS-N.", &vars, 4);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].0, "WS-N");
    }

    /// Restraint is the point: an annotation on everything is a wall.
    #[test]
    fn the_limit_is_honoured_and_duplicates_are_not_repeated() {
        let vars = vec![v("WS-A", "1"), v("WS-B", "2"), v("WS-C", "3")];
        let line = "    COMPUTE WS-A = WS-B + WS-C.";
        assert_eq!(inline_values(line, &vars, 2).len(), 2);
        assert_eq!(inline_values(line, &vars, 0).len(), 0);
        // A subscripted slot annotates under its parent's name, once.
        let subs = vec![v("WS-ROW(1)", "10"), v("WS-ROW(2)", "20")];
        let hits = inline_values("    ADD 1 TO WS-ROW(1).", &subs, 4);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].0, "WS-ROW");
    }

    #[test]
    fn a_line_naming_nothing_annotates_nothing() {
        let vars = vec![v("WS-A", "1")];
        assert!(inline_values("    GOBACK.", &vars, 4).is_empty());
        assert!(inline_values("", &vars, 4).is_empty());
        // An item with no value is not worth a hint.
        assert!(inline_values("    ADD 1 TO WS-A.", &[v("WS-A", "   ")], 4).is_empty());
    }

    /// Case-insensitive, because the listing is COBOL and the storage keys are
    /// upper case whatever the developer typed.
    #[test]
    fn matching_ignores_case() {
        let vars = vec![v("WS-TOTAL", "42")];
        assert_eq!(inline_values("    move 1 to ws-total.", &vars, 4).len(), 1);
    }
}


#[cfg(test)]
mod selection_and_elision_tests {
    use super::{elide_long_literals, word_span_at, DebuggerPanel, LITERAL_DRAW_CHARS, TIP_VALUE_CHARS};

    fn panel(lines: &[&str]) -> DebuggerPanel {
        let mut p = DebuggerPanel::new();
        p.source_lines = lines.iter().map(|l| (*l).to_string()).collect();
        p
    }

    /// One line of documentation prose in a VALUE clause used to make the whole
    /// listing scroll sideways, so every other line had to be read through an
    /// offset it did not need.
    #[test]
    fn a_long_literal_is_elided_for_drawing_only() {
        let long = "X".repeat(400);
        let line = format!("           MOVE \"{long}\" TO WS-DOC.");
        let drawn = elide_long_literals(&line, LITERAL_DRAW_CHARS);

        assert!(drawn.chars().count() < line.chars().count(), "nothing was elided");
        assert!(drawn.contains('…'), "the cut is not marked: {drawn}");
        // Still recognisable as COBOL: the statement around it survives whole.
        assert!(drawn.starts_with("           MOVE \""), "got {drawn}");
        assert!(drawn.ends_with("\" TO WS-DOC."), "the tail was lost: {drawn}");
    }

    /// A literal that fits is returned untouched — and borrowed, so the common
    /// line costs no allocation.
    #[test]
    fn a_short_literal_is_left_exactly_as_written() {
        let line = "           MOVE \"OK\" TO WS-FLAG.";
        assert!(matches!(
            elide_long_literals(line, LITERAL_DRAW_CHARS),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(elide_long_literals(line, LITERAL_DRAW_CHARS), line);
    }

    /// Accented prose is exactly what these literals hold, and cutting by byte
    /// would split a character in half.
    #[test]
    fn eliding_counts_characters_not_bytes() {
        let accented = "á".repeat(200);
        let line = format!("      01 WS-T PIC X(200) VALUE \"{accented}\".");
        let drawn = elide_long_literals(&line, 10);
        assert!(drawn.contains("áááááááááá…"), "got {drawn}");
        // The proof it is not byte-cut: it is still valid UTF-8 text we can
        // count, and the head is exactly 10 characters.
        let head: String = drawn
            .chars()
            .skip_while(|c| *c != '"')
            .skip(1)
            .take_while(|c| *c != '…')
            .collect();
        assert_eq!(head.chars().count(), 10, "head was {head:?}");
    }

    /// An unterminated literal — a half-typed line — must not swallow the rest
    /// of the file or panic.
    #[test]
    fn an_unterminated_literal_is_handled() {
        let line = "           MOVE \"no closing quote here";
        let drawn = elide_long_literals(line, LITERAL_DRAW_CHARS);
        assert_eq!(drawn, line);
    }

    /// Dragging across two lines copies both, joined with a newline.
    #[test]
    fn a_selection_spanning_lines_copies_every_line_it_covers() {
        let mut p = panel(&["MOVE A TO B.", "ADD 1 TO C.", "DISPLAY C."]);
        // From "A" on line 0 to just past "1" on line 1.
        p.sel_anchor = Some((0, 5));
        p.sel_cursor = Some((1, 5));
        assert_eq!(p.selected_text().as_deref(), Some("A TO B.\nADD 1"));
    }

    /// A selection inside one line copies exactly that run.
    #[test]
    fn a_selection_within_one_line_copies_that_run() {
        let mut p = panel(&["           PERFORM VALIDATE-CUSTOMER."]);
        p.sel_anchor = Some((0, 19));
        p.sel_cursor = Some((0, 36));
        assert_eq!(p.selected_text().as_deref(), Some("VALIDATE-CUSTOMER"));
    }

    /// Dragging backwards selects the same text as dragging forwards.
    #[test]
    fn a_backwards_drag_selects_the_same_text() {
        let mut p = panel(&["MOVE A TO B.", "ADD 1 TO C."]);
        p.sel_anchor = Some((1, 5));
        p.sel_cursor = Some((0, 5));
        assert_eq!(p.selected_text().as_deref(), Some("A TO B.\nADD 1"));
    }

    /// Nothing selected means nothing to copy — which is what disables the
    /// Copy button rather than letting it put an empty string on the clipboard.
    #[test]
    fn an_empty_selection_offers_nothing() {
        let mut p = panel(&["MOVE A TO B."]);
        assert!(p.selection_range().is_none());
        assert!(p.selected_text().is_none());
        // Anchor and cursor at the same spot is a click, not a selection.
        p.sel_anchor = Some((0, 4));
        p.sel_cursor = Some((0, 4));
        assert!(p.selection_range().is_none());
        assert!(p.selected_text().is_none());
    }

    /// What is copied is what the listing SHOWS: a literal drawn elided is
    /// copied elided, because copying text the developer cannot see would be
    /// the more surprising of the two.
    #[test]
    fn copying_an_elided_line_copies_what_is_drawn() {
        let long = "Z".repeat(300);
        let mut p = panel(&[format!("MOVE \"{long}\" TO X.").as_str()]);
        p.sel_anchor = Some((0, 0));
        p.sel_cursor = Some((0, 10_000));
        let copied = p.selected_text().expect("a selection");
        assert!(copied.contains('…'), "the copy is not the drawn text: {copied}");
        assert!(copied.chars().count() < 300, "the whole literal was copied");
    }

    /// Double-click takes the whole run between two spaces.
    ///
    /// The operator's own rule (2026-09-17): a word is what is surrounded by
    /// spaces, bounded also by the start or end of the line and by a period.
    /// Deliberately coarser than a COBOL identifier — `TRIM(WS-LINE))` is one
    /// word, because that is the thing sitting between two spaces.
    #[test]
    fn a_double_click_takes_the_run_between_two_spaces() {
        let line = "           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).";
        let at = line.find("TRIM").unwrap();
        let (from, to) = word_span_at(line, at).expect("a word");
        assert_eq!(&line[from..to], "TRIM(WS-LINE))");

        // …and the token before it, from anywhere inside.
        let mid = line.find("AppendText").unwrap() + 3;
        let (from, to) = word_span_at(line, mid).expect("a word");
        assert_eq!(&line[from..to], "Txt-Log::AppendText(FUNCTION");
    }

    /// A word may begin in column 1 and may end at the end of the line.
    #[test]
    fn a_word_may_touch_either_end_of_the_line() {
        assert_eq!(word_span_at("MAIN-PARA", 0), Some((0, 9)));
        let line = "MOVE 1 TO WS-N";
        let (from, to) = word_span_at(line, line.len() - 1).expect("a word");
        assert_eq!(&line[from..to], "WS-N");
    }

    /// A period ends a word, so double-clicking the last one on a sentence does
    /// not drag the period in with it.
    #[test]
    fn a_period_ends_a_word() {
        let line = "           STOP RUN.";
        let at = line.find("RUN").unwrap();
        let (from, to) = word_span_at(line, at).expect("a word");
        assert_eq!(&line[from..to], "RUN");
    }

    /// On a separator there is no word to take — a double-click in the indent
    /// selects nothing rather than guessing at a neighbour.
    #[test]
    fn a_double_click_on_a_gap_takes_nothing() {
        assert_eq!(word_span_at("           STOP RUN.", 3), None);
        assert_eq!(word_span_at("           STOP RUN.", 19), None); // the period
        assert_eq!(word_span_at("", 0), None);
    }

    /// Find walks every match, in reading order, wrapping at both ends.
    #[test]
    fn find_locates_every_match_and_walks_them() {
        let mut p = panel(&[
            "           MOVE WS-LINE TO WS-NL.",
            "           DISPLAY WS-LINE.",
            "           STOP RUN.",
        ]);
        p.find_query = "ws-line".into();
        p.rebuild_find_hits();

        assert_eq!(p.find_hits.len(), 2, "both lines hold a match: {:?}", p.find_hits);
        assert_eq!(p.find_hits[0].0, 0);
        assert_eq!(p.find_hits[1].0, 1);
        // Case-insensitive: COBOL is, so the search is.
        assert_eq!(p.find_at, 0);

        p.step_find(true);
        assert_eq!(p.find_at, 1);
        p.step_find(true);
        assert_eq!(p.find_at, 0, "forward wraps");
        p.step_find(false);
        assert_eq!(p.find_at, 1, "backward wraps");
    }

    /// The current hit is selected, so Copy takes what was found.
    #[test]
    fn the_current_hit_becomes_the_selection() {
        let mut p = panel(&["           PERFORM VALIDATE-CUSTOMER."]);
        p.find_query = "validate".into();
        p.rebuild_find_hits();
        p.reveal_current_hit();
        assert_eq!(p.selected_text().as_deref(), Some("VALIDATE"));
    }

    /// A query matching nothing leaves no hits and no selection to copy.
    #[test]
    fn a_query_with_no_match_finds_nothing() {
        let mut p = panel(&["           STOP RUN."]);
        p.find_query = "WS-NOWHERE".into();
        p.rebuild_find_hits();
        assert!(p.find_hits.is_empty());
        // Stepping an empty list must not panic or wrap into nothing.
        p.step_find(true);
        assert_eq!(p.find_at, 0);
    }

    /// Two matches on ONE line are two hits, not one.
    #[test]
    fn two_matches_on_one_line_are_two_hits() {
        let mut p = panel(&["           ADD WS-N TO WS-N."]);
        p.find_query = "WS-N".into();
        p.rebuild_find_hits();
        assert_eq!(p.find_hits.len(), 2, "got {:?}", p.find_hits);
        assert_ne!(p.find_hits[0].1, p.find_hits[1].1, "both hits at one column");
    }

    /// The hover tooltip's cap, raised from 18 to 100 by operator instruction:
    /// 18 cut a caption or a path to a stub that answered nothing.
    #[test]
    fn the_tooltip_cap_is_a_hundred_characters() {
        assert_eq!(TIP_VALUE_CHARS, 100);
    }
}

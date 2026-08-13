// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The ONE breadcrumb renderer (spec 049).
//!
//! The breadcrumb is shell chrome: at run time it shows the navigation chain
//! and takes the operator back up it. But the developer has to SEE it while
//! designing, and the IDE deliberately takes no runtime dependency on the form
//! host (Run Form talks to it through the `rcrun` child process) — so the strip
//! cannot live there. It lives here, next to [`crate::sidebar`], and every
//! surface draws through it: the designer canvas, the preview, Run Form and the
//! running shell.
//!
//! Anatomy, left to right:
//!
//! ```text
//!   ┌────┬───────────────────────────────────────────┐
//!   │ ▶│ │  Main Menu  ›  CRM  ›  Customer List      │
//!   └────┴───────────────────────────────────────────┘
//!     ▲      ▲
//!     │      └── the chain, one segment per resident form
//!     └── the sidebar's Open/Collapsed control, a full-height cell
//! ```
//!
//! On the design surfaces the chain is STATIC — one segment, the form itself —
//! because a navigation chain is a runtime fact and there is nothing else to
//! honestly show. The toggle is drawn there all the same, so the developer sees
//! the control their operator will use.

use crate::model::Control;
use egui::{Color32, FontId, Pos2, Rect, Vec2};

/// Height of the breadcrumb strip, in points.
pub const HEIGHT: f32 = 28.0;

/// The strip's default chrome fill. Opaque by construction: in a transparent
/// shell window an unpainted strip is a hole to the desktop (R43). The form
/// host re-exports this as its `CHROME_FILL` so there is ONE such colour.
pub const CHROME: Color32 = Color32::from_rgb(0x2E, 0x31, 0x38);

/// The sidebar Open/Collapsed control's icon, for a rail in the given state.
///
/// The arrow shows the NEXT action, never the current state: an open rail
/// offers the arrow that closes it, a collapsed one the arrow that opens it.
/// A control that looks the same in both states tells the operator nothing.
pub fn toggle_icon(collapsed: bool) -> &'static str {
    if collapsed {
        "sidebar-expand"
    } else {
        "sidebar-collapse"
    }
}

/// Horizontal padding around the segment text.
const PAD_X: f32 = 10.0;
/// Gap either side of a `›` separator.
const SEP_GAP: f32 = 6.0;

/// Everything the strip needs that is not geometry.
pub struct BreadcrumbState<'a> {
    /// The chain, root first. One entry on the design surfaces.
    pub segments: &'a [String],
    /// The strip's own background — what the contrast rule measures against.
    pub bg: Color32,
    /// Segment text.
    pub fg: Color32,
    /// Separators, and the toggle's hover wash.
    pub dim: Color32,
    /// The toggle icon, styled the way the sidebar's own icons are.
    pub icon: crate::icons::IconStyle,
    /// The rail's current state — it picks which way the toggle's arrow points
    /// (see [`toggle_icon`]).
    pub collapsed: bool,
    /// Draw the toggle's hover wash this frame.
    pub toggle_hovered: bool,
    pub font: FontId,
}

/// Where the strip's parts landed. Painting and hit-testing both walk this, so
/// a click can never land somewhere other than what was drawn under it.
#[derive(Clone, Debug)]
pub struct BreadcrumbLayout {
    /// The sidebar toggle's cell — a square of the strip's FULL height.
    pub toggle: Rect,
    /// One rect per segment, chain order.
    pub segments: Vec<Rect>,
}

impl Default for BreadcrumbLayout {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            segments: Vec::new(),
        }
    }
}

/// Lift `color` off `bg` when it would be too faint to read there.
///
/// The toggle and the chain take the sidebar's own colours, so an application
/// styled for a dark rail keeps its palette in the strip. But the strip's
/// background is the shell's chrome, not the rail's, and a light-on-light (or
/// dark-on-dark) pairing would leave the control invisible — so anything under
/// the 3:1 the WCAG asks of non-text graphics is replaced by whichever of black
/// or white actually reads there.
pub fn readable_on(color: Color32, bg: Color32) -> Color32 {
    if crate::paint::contrast_ratio(color, bg) >= 3.0 {
        return color;
    }
    let white = crate::paint::contrast_ratio(Color32::WHITE, bg);
    let black = crate::paint::contrast_ratio(Color32::BLACK, bg);
    if white >= black {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// Build the strip's state from the SideMenu that owns the shell, so the
/// breadcrumb is styled by the application rather than by constants. `bg` is
/// the fill the strip is painted on — the contrast rule needs it.
pub fn state_for_control<'a>(
    ctx: &egui::Context,
    ctrl: &Control,
    segments: &'a [String],
    bg: Color32,
) -> BreadcrumbState<'a> {
    let pal = crate::sidebar::SidebarPalette::from_control(ctrl, 255);
    let fg = readable_on(pal.fg, bg);
    let icon_color = readable_on(pal.fg, bg);
    let font_name = ctrl
        .get_prop("FontName")
        .map(|v| v.as_str())
        .unwrap_or_default();
    BreadcrumbState {
        segments,
        bg,
        fg,
        dim: Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 150),
        icon: crate::icons::icon_style_for_effect(
            ctrl.get_prop("IconEffect")
                .map(|v| v.as_str())
                .unwrap_or("None"),
            icon_color,
        ),
        collapsed: ctrl.side_menu_collapsed(),
        toggle_hovered: false,
        font: crate::fonts::font_id(ctx, &font_name, crate::paint::ctrl_font_size(ctrl)),
    }
}

/// A state with no control to read — the strip still paints, in colours that
/// are legible on `bg`. Used where a form has no SideMenu to style it.
pub fn state_plain<'a>(segments: &'a [String], bg: Color32) -> BreadcrumbState<'a> {
    let fg = readable_on(Color32::from_rgb(225, 230, 250), bg);
    BreadcrumbState {
        segments,
        bg,
        fg,
        dim: Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 150),
        icon: crate::icons::IconStyle::tint(fg),
        collapsed: false,
        toggle_hovered: false,
        font: FontId::proportional(13.0),
    }
}

/// Lay the strip out without painting.
///
/// The toggle's cell is a square of the strip's FULL height at the left edge —
/// the operator's requirement, and the reason the icon reads at a glance next
/// to text half its size.
pub fn layout(
    painter: &egui::Painter,
    rect: Rect,
    state: &BreadcrumbState<'_>,
) -> BreadcrumbLayout {
    let side = rect.height();
    let toggle = Rect::from_min_size(rect.min, Vec2::splat(side));

    let mut segments = Vec::with_capacity(state.segments.len());
    let mut x = toggle.max.x + PAD_X;
    for (i, label) in state.segments.iter().enumerate() {
        if i > 0 {
            let sep = painter.layout_no_wrap("›".to_owned(), state.font.clone(), state.dim);
            x += SEP_GAP + sep.size().x + SEP_GAP;
        }
        let galley = painter.layout_no_wrap(label.clone(), state.font.clone(), state.fg);
        let w = galley.size().x;
        segments.push(Rect::from_min_size(
            Pos2::new(x, rect.min.y),
            Vec2::new(w, rect.height()),
        ));
        x += w;
    }
    BreadcrumbLayout { toggle, segments }
}

/// Paint the strip. `l` must come from [`layout`] for the same rect and state.
pub fn paint(
    painter: &egui::Painter,
    rect: Rect,
    state: &BreadcrumbState<'_>,
    l: &BreadcrumbLayout,
) {
    // R43 — the chrome paints itself, edge to edge. An unpainted strip in a
    // transparent shell window is a hole to the desktop.
    painter.rect_filled(rect, 0.0, state.bg);

    if state.toggle_hovered {
        painter.rect_filled(
            l.toggle.shrink(3.0),
            5.0,
            Color32::from_rgba_unmultiplied(state.dim.r(), state.dim.g(), state.dim.b(), 40),
        );
    }
    crate::icons::draw_menu_icon_styled(
        painter,
        l.toggle,
        toggle_icon(state.collapsed),
        &state.icon,
    );

    let p = painter.with_clip_rect(rect);
    for (i, seg) in l.segments.iter().enumerate() {
        if i > 0 {
            p.text(
                Pos2::new(seg.min.x - SEP_GAP, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "›",
                state.font.clone(),
                state.dim,
            );
        }
        // The last segment is where you ARE: full strength. The ones behind it
        // are links back, and read as quieter.
        let last = i + 1 == l.segments.len();
        p.text(
            Pos2::new(seg.min.x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &state.segments[i],
            state.font.clone(),
            if last { state.fg } else { state.dim },
        );
    }
}

/// Is `pos` on the sidebar toggle?
pub fn toggle_hit(l: &BreadcrumbLayout, pos: Pos2) -> bool {
    l.toggle.contains(pos)
}

/// Which segment contains `pos`, if any.
pub fn segment_at(l: &BreadcrumbLayout, pos: Pos2) -> Option<usize> {
    l.segments.iter().position(|r| r.contains(pos))
}

// ── The design surfaces ─────────────────────────────────────────────────────

/// The SideMenu that puts a form into SHELL mode — the first one on the form.
/// `None` means the form opens as a plain window, with no shell chrome at all.
pub fn shell_side_menu(form: &crate::model::Form) -> Option<&Control> {
    shell_side_menu_in(&form.controls)
}

/// As [`shell_side_menu`], for a surface that holds the controls on their own
/// (the preview renders from a snapshot, not from the `Form`).
pub fn shell_side_menu_in(controls: &[Control]) -> Option<&Control> {
    controls
        .iter()
        .find(|c| c.control_type == crate::ControlType::SideMenu)
}

/// Where the strip lands on a design surface showing a form `form_width` wide
/// at `origin`, whose shell sidebar is `side` shown `rail_width` wide.
///
/// It sits at the top of the CONTENT area, which is what the running shell
/// does: with `FullHeight` on the rail owns the window's whole height and the
/// strip starts at the rail's right edge; with it off the strip spans the
/// width and the rail hangs beneath.
///
/// `rail_width` is the width the rail is SHOWN at — [`crate::sidebar::shown_width`]
/// — not the designed one, so the strip follows the rail's edge when it
/// collapses instead of leaving a band of nothing between them.
pub fn strip_rect(side: &Control, rail_width: f32, form_width: f32, origin: Pos2) -> Option<Rect> {
    let left = if side.side_menu_full_height() {
        origin.x + rail_width
    } else {
        origin.x
    };
    let right = origin.x + form_width;
    if right - left <= 1.0 {
        return None;
    }
    Some(Rect::from_min_max(
        Pos2::new(left, origin.y),
        Pos2::new(right, origin.y + HEIGHT),
    ))
}

/// The strip's background: the CONTENT pane's own backdrop, so the breadcrumb
/// reads as the top of the content area rather than as a separate grey band.
/// Composited over [`CHROME`] so it is opaque whatever the form's transparency
/// (R43) — the strip is chrome, and a hole in it shows the desktop.
pub fn strip_background(form_background_hex: &str, transparency: u8) -> Color32 {
    let backdrop = crate::render::backdrop_color(form_background_hex, transparency);
    crate::paint::composite_premultiplied_over(backdrop, CHROME)
}

/// How a design surface is showing the rail, and how it drew the toggle.
///
/// `collapsed` is the state being SHOWN, which on a design surface is not the
/// same thing as the control's `Collapsed` property: that property is the state
/// the finished application opens in, and clicking the toggle while designing
/// must never rewrite it. The surface owns the shown state and hands it in.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesignView {
    pub collapsed: bool,
    pub toggle_hovered: bool,
}

/// Draw the STATIC strip a design surface shows, and hand back what it laid
/// out so the surface can hit-test the toggle it just drew.
///
/// The chain is one segment — the form itself — because a navigation chain is a
/// runtime fact and inventing more of it at design time would be decoration.
/// The sidebar's Open/Collapsed control is drawn all the same, so the developer
/// sees the control their operator will use.
pub fn draw_static_strip(
    painter: &egui::Painter,
    ctx: &egui::Context,
    side: &Control,
    label: &str,
    rect: Rect,
    bg: Color32,
    view: DesignView,
) -> BreadcrumbLayout {
    let segments = [label.to_owned()];
    let mut state = state_for_control(ctx, side, &segments, bg);
    state.collapsed = view.collapsed;
    state.toggle_hovered = view.toggle_hovered;
    let l = layout(painter, rect, &state);
    paint(painter, rect, &state, &l);
    l
}

/// The strip's rect for a whole [`crate::model::Form`] with its rail shown in
/// the given state; `None` when the form has no SideMenu and so opens as a
/// plain window, with no shell chrome.
pub fn design_strip_rect(form: &crate::model::Form, origin: Pos2, collapsed: bool) -> Option<Rect> {
    let side = shell_side_menu(form)?;
    strip_rect(
        side,
        crate::sidebar::shown_width(side, collapsed),
        form.width as f32,
        origin,
    )
}

/// The label the design surfaces show: the form's title, or its name when it
/// has none.
pub fn design_label(form: &crate::model::Form) -> String {
    if form.title.trim().is_empty() {
        form.name.clone()
    } else {
        form.title.clone()
    }
}

/// Draw the static strip for a whole form. Returns what it laid out, or `None`
/// when the form is not a shell.
///
/// Draw this BEFORE the form's controls where the surface allows it: a control
/// the developer deliberately placed in that band then paints over the strip,
/// so the indicator can never hide their work.
pub fn draw_design_strip(
    painter: &egui::Painter,
    ctx: &egui::Context,
    form: &crate::model::Form,
    origin: Pos2,
    view: DesignView,
) -> Option<BreadcrumbLayout> {
    let side = shell_side_menu(form)?;
    let rect = strip_rect(
        side,
        crate::sidebar::shown_width(side, view.collapsed),
        form.width as f32,
        origin,
    )?;
    let bg = strip_background(&form.background_color, form.transparency.min(100) as u8);
    Some(draw_static_strip(
        painter,
        ctx,
        side,
        &design_label(form),
        rect,
        bg,
        view,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    /// Run `f` with a painter on a headless context.
    fn with_painter<R>(ctx: &egui::Context, f: impl FnOnce(&egui::Painter) -> R) -> R {
        let mut out = None;
        let mut f = Some(f);
        let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui.painter()));
            }
        });
        full.textures_delta.clear();
        out.expect("ran")
    }

    fn strip() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, HEIGHT))
    }

    /// The toggle's cell is a square of the strip's FULL height, at the left
    /// edge, and every segment starts after it.
    #[test]
    fn the_toggle_is_a_full_height_cell_before_the_chain() {
        let ctx = ctx();
        let segs: Vec<String> = ["Main Menu", "CRM", "Customer List"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let st = state_plain(&segs, Color32::from_rgb(0x2E, 0x31, 0x38));
        let rect = strip();
        let l = with_painter(&ctx, |p| layout(p, rect, &st));

        assert_eq!(l.toggle.min, rect.min, "the toggle leads the strip");
        assert_eq!(
            l.toggle.height(),
            rect.height(),
            "the icon cell is the pane's own height"
        );
        assert_eq!(l.toggle.width(), l.toggle.height(), "and it is square");
        assert_eq!(l.segments.len(), 3, "one rect per chain entry");
        for (i, seg) in l.segments.iter().enumerate() {
            assert!(
                seg.min.x >= l.toggle.max.x,
                "segment {i} must start after the toggle"
            );
            assert!(rect.contains_rect(*seg), "segment {i} escaped the strip");
        }
        // Chain order, left to right, no overlap.
        for w in l.segments.windows(2) {
            assert!(w[1].min.x > w[0].max.x, "segments run left to right");
        }
        eprintln!(
            "049 breadcrumb — toggle cell {:.0}x{:.0} (= strip height {:.0}), \
             then {} segments left to right",
            l.toggle.width(),
            l.toggle.height(),
            rect.height(),
            l.segments.len()
        );
    }

    /// Hit-testing walks the SAME rects that were laid out.
    #[test]
    fn hit_testing_matches_the_layout() {
        let ctx = ctx();
        let segs: Vec<String> = ["Main Menu", "CRM"].iter().map(|s| s.to_string()).collect();
        let st = state_plain(&segs, Color32::BLACK);
        let rect = strip();
        let l = with_painter(&ctx, |p| layout(p, rect, &st));

        assert!(toggle_hit(&l, l.toggle.center()));
        assert!(!toggle_hit(&l, l.segments[0].center()));
        for (i, seg) in l.segments.iter().enumerate() {
            assert_eq!(segment_at(&l, seg.center()), Some(i));
        }
        assert_eq!(segment_at(&l, Pos2::new(795.0, 14.0)), None, "past the chain");
    }

    /// The strip takes the sidebar's colours — until they would be invisible on
    /// the chrome behind them, which is the whole point of the contrast rule.
    #[test]
    fn colours_come_from_the_control_but_never_at_the_cost_of_legibility() {
        let ctx = ctx();
        let segs = vec!["Main Menu".to_string()];

        // A light rail palette on the dark chrome: kept as designed.
        let mut light = Control::new("S", crate::ControlType::SideMenu, 0, 0);
        light.set_prop("ForegroundColor", "#E1E6FA");
        let dark_bg = Color32::from_rgb(0x2E, 0x31, 0x38);
        let st = state_for_control(&ctx, &light, &segs, dark_bg);
        assert_eq!(
            st.fg,
            Color32::from_rgb(0xE1, 0xE6, 0xFA),
            "a legible designed colour is used as designed"
        );

        // The SAME palette on a light strip: unreadable, so it is replaced.
        let st = state_for_control(&ctx, &light, &segs, Color32::from_rgb(0xF6, 0xF6, 0xF6));
        assert!(
            crate::paint::contrast_ratio(st.fg, Color32::from_rgb(0xF6, 0xF6, 0xF6)) >= 3.0,
            "near-white on near-white must be lifted, got {:?}",
            st.fg
        );
        eprintln!(
            "049 breadcrumb contrast — #E1E6FA kept on {:?} (ratio {:.1}), \
             replaced by {:?} on #F6F6F6",
            dark_bg,
            crate::paint::contrast_ratio(Color32::from_rgb(0xE1, 0xE6, 0xFA), dark_bg),
            st.fg
        );
    }

    /// A form is a shell because it has a SideMenu, and the strip then sits at
    /// the top of the CONTENT area — after the rail when it owns the full
    /// height, across the whole form when it does not.
    #[test]
    fn the_design_strip_follows_the_rail_and_only_exists_for_a_shell() {
        let mut form = crate::model::Form::new("SIDEBAR-FORM", "Main Menu", 960, 744);
        let origin = Pos2::new(100.0, 50.0);
        assert!(
            design_strip_rect(&form, origin, false).is_none(),
            "a form with no SideMenu opens as a plain window — no shell chrome"
        );

        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = crate::model::Rect::new(0, 0, 200, 744);
        side.set_prop("FullHeight", true);
        form.controls.push(side);

        let r = design_strip_rect(&form, origin, false).expect("a shell has a strip");
        assert_eq!(r.min.x, origin.x + 200.0, "FullHeight: after the rail");
        assert_eq!(r.max.x, origin.x + 960.0, "…out to the form's edge");
        assert_eq!(r.min.y, origin.y, "at the top of the content area");
        assert_eq!(r.height(), HEIGHT);

        // Shown collapsed, the strip follows the rail's edge in to the
        // collapsed width — it does not leave a band of nothing behind.
        let c = design_strip_rect(&form, origin, true).expect("still a shell");
        assert_eq!(
            c.min.x,
            origin.x + crate::sidebar::COLLAPSED_WIDTH,
            "collapsed: the strip starts at the icon rail's edge"
        );

        form.controls[0].set_prop("FullHeight", false);
        let r = design_strip_rect(&form, origin, false).expect("still a shell");
        assert_eq!(
            r.min.x, origin.x,
            "FullHeight off: the strip spans the form and the rail hangs beneath"
        );
        assert_eq!(r.width(), 960.0);

        assert_eq!(design_label(&form), "Main Menu", "the title names the form");
        form.title.clear();
        assert_eq!(design_label(&form), "SIDEBAR-FORM", "…its name when it has none");

        eprintln!(
            "049 breadcrumb (design) — 960px form, 200px rail: FullHeight on → \
             strip x {:.0}..{:.0}; off → full width; no SideMenu → no strip",
            origin.x + 200.0,
            origin.x + 960.0
        );
    }

    /// The toggle's arrow shows the NEXT action, not the current state: open
    /// offers the arrow that closes, collapsed the arrow that opens. It was
    /// stuck on one drawing in both states, which told the operator nothing.
    #[test]
    fn the_toggle_arrow_points_at_what_the_click_will_do() {
        assert_eq!(
            toggle_icon(false),
            "sidebar-collapse",
            "an OPEN rail offers the arrow that collapses it (points left)"
        );
        assert_eq!(
            toggle_icon(true),
            "sidebar-expand",
            "a COLLAPSED rail offers the arrow that opens it (points right)"
        );
        assert_ne!(
            toggle_icon(true),
            toggle_icon(false),
            "the two states must not share one drawing"
        );
        for name in [toggle_icon(true), toggle_icon(false)] {
            assert!(
                crate::icons::menu_icon_names().any(|n| n == name),
                "{name} must be a real catalogue drawing"
            );
        }
        eprintln!(
            "049 breadcrumb toggle — open → {} (◀), collapsed → {} (▶)",
            toggle_icon(false),
            toggle_icon(true)
        );
    }

    /// The strip carries the CONTENT pane's backdrop, and stays opaque (R43)
    /// even for a form the developer made transparent.
    #[test]
    fn the_strip_background_follows_the_content_pane() {
        // The form's own navy, which `backdrop_color` also gives the pane.
        let navy = crate::render::backdrop_color("00000000", 0);
        let bg = strip_background("00000000", 0);
        assert_eq!(bg.a(), 255, "R43: the strip is opaque chrome");
        assert_eq!(
            (bg.r(), bg.g(), bg.b()),
            (navy.r(), navy.g(), navy.b()),
            "an opaque form: the strip IS the content pane's colour"
        );
        assert_ne!(bg, CHROME, "…and no longer the shell's grey band");

        // A form made translucent still cannot punch a hole in the chrome.
        let faded = strip_background("102040", 70);
        assert_eq!(faded.a(), 255, "R43 holds at any form transparency");

        eprintln!(
            "049 breadcrumb background — form 00000000/0 → strip {bg:?} \
             (= content pane), was {CHROME:?}; form 102040/70 → {faded:?}, \
             still opaque"
        );
    }

    /// The strip paints headlessly, with and without a chain.
    #[test]
    fn paints_headlessly() {
        let ctx = ctx();
        for segs in [Vec::new(), vec!["Main Menu".to_string()]] {
            let st = state_plain(&segs, Color32::from_rgb(0x2E, 0x31, 0x38));
            let rect = strip();
            with_painter(&ctx, |p| {
                let l = layout(p, rect, &st);
                paint(p, rect, &st, &l);
            });
        }
    }
}

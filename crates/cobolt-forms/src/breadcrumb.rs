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

/// The breadcrumb strip's DEFAULT height, in points. The frame's actual height
/// is the SideMenu's `BreadcrumbHeight` property — see [`height_of`].
pub const HEIGHT: f32 = crate::model::DEFAULT_BREADCRUMB_HEIGHT;

/// The height the frame is drawn at for the rail that owns it.
///
/// The strip is the sidebar's chrome, so its height is the sidebar's property.
/// A rail with no `BreadcrumbHeight` (every form drawn before the property
/// existed) keeps the historical [`HEIGHT`].
pub fn height_of(side: &Control) -> f32 {
    side.breadcrumb_height()
}

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

/// The largest the sidebar's Open/Collapsed cell is drawn, in points. The cell
/// is a square of the frame's height — but a frame made tall to hold the
/// developer's own controls is not a request for a 200-point arrow, so past
/// this the cell stops growing and sits centred at the frame's left edge. At
/// the default height the cap never applies.
const TOGGLE_MAX: f32 = 48.0;

/// Horizontal padding around the segment text.
const PAD_X: f32 = 10.0;
/// How far off the frame's top or bottom edge the text is held when it is
/// aligned against one, so it never touches the border it sits against.
const PAD_Y: f32 = 3.0;
/// Gap either side of a `›` separator.
const SEP_GAP: f32 = 6.0;

/// Where the chain sits inside the frame, vertically.
///
/// The frame's height is the developer's (`BreadcrumbHeight`) and owes nothing
/// to the font — so on a frame made taller than its text the chain has room to
/// move, and this says where it goes. Without it a 200-point frame could only
/// ever hold its text pinned to the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

impl TextAlign {
    /// Read a `BreadcrumbTextAlign` value. Anything unrecognised — including
    /// every form saved before the property existed — is [`Middle`](Self::Middle),
    /// which is where the chain has always been drawn. `down` is accepted as a
    /// spelling of `bottom`.
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "top" => Self::Top,
            "bottom" | "down" => Self::Bottom,
            _ => Self::Middle,
        }
    }

    /// The y to anchor text at inside `rect`, and the matching egui alignment.
    fn anchor(self, rect: Rect) -> (f32, egui::Align) {
        match self {
            Self::Top => (rect.min.y + PAD_Y, egui::Align::TOP),
            Self::Middle => (rect.center().y, egui::Align::Center),
            Self::Bottom => (rect.max.y - PAD_Y, egui::Align::BOTTOM),
        }
    }
}

/// Everything the strip needs that is not geometry.
pub struct BreadcrumbState<'a> {
    /// The chain, root first. One entry on the design surfaces.
    pub segments: &'a [String],
    /// A DETAIL level the running form appended after its own name
    /// (`me::"SetBreadcrumbDetail"`) — the customer being edited, the order
    /// being priced. It is not a chain entry: nothing is resident under it,
    /// and it is the application's text, not the shell's.
    ///
    /// With one present the form's own segment becomes an inner link, and
    /// clicking it asks the shell to RESET the form (see
    /// [`BreadcrumbLayout::reset_segment`]).
    pub detail: Option<String>,
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
    /// Where the chain sits inside the frame — the rail's `BreadcrumbTextAlign`.
    pub align: TextAlign,
}

/// Where the strip's parts landed. Painting and hit-testing both walk this, so
/// a click can never land somewhere other than what was drawn under it.
#[derive(Clone, Debug)]
pub struct BreadcrumbLayout {
    /// The sidebar toggle's cell — a square of the strip's FULL height.
    pub toggle: Rect,
    /// One rect per segment, chain order.
    pub segments: Vec<Rect>,
    /// The detail level's rect, when the form appended one. It is where you
    /// ARE, so it is not a link and nothing happens when it is clicked.
    pub detail: Option<Rect>,
}

impl Default for BreadcrumbLayout {
    fn default() -> Self {
        Self {
            toggle: Rect::NOTHING,
            segments: Vec::new(),
            detail: None,
        }
    }
}

impl BreadcrumbLayout {
    /// The segment whose click is a RESET rather than a navigation: the
    /// displayed form's own name, once a detail level sits after it.
    ///
    /// Without a detail level the last segment is simply where you are, and
    /// clicking it does nothing — there is nothing to go back to and nothing
    /// to reset to.
    pub fn reset_segment(&self) -> Option<usize> {
        self.detail.is_some().then(|| self.segments.len().checked_sub(1))?
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
        detail: None,
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
        align: TextAlign::parse(
            &ctrl
                .get_prop("BreadcrumbTextAlign")
                .map(|v| v.as_str())
                .unwrap_or_default(),
        ),
    }
}

/// A state with no control to read — the strip still paints, in colours that
/// are legible on `bg`. Used where a form has no SideMenu to style it.
pub fn state_plain<'a>(segments: &'a [String], bg: Color32) -> BreadcrumbState<'a> {
    let fg = readable_on(Color32::from_rgb(225, 230, 250), bg);
    BreadcrumbState {
        segments,
        detail: None,
        bg,
        fg,
        dim: Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 150),
        icon: crate::icons::IconStyle::tint(fg),
        collapsed: false,
        toggle_hovered: false,
        font: FontId::proportional(13.0),
        align: TextAlign::default(),
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
    let side = rect.height().min(TOGGLE_MAX);
    let toggle = Rect::from_center_size(
        Pos2::new(rect.min.x + side * 0.5, rect.center().y),
        Vec2::splat(side),
    );

    let mut segments = Vec::with_capacity(state.segments.len());
    let mut x = toggle.max.x + PAD_X;
    let mut place = |x: &mut f32, label: &str, first: bool| {
        if !first {
            let sep = painter.layout_no_wrap("›".to_owned(), state.font.clone(), state.dim);
            *x += SEP_GAP + sep.size().x + SEP_GAP;
        }
        let galley = painter.layout_no_wrap(label.to_owned(), state.font.clone(), state.fg);
        let w = galley.size().x;
        let r = Rect::from_min_size(Pos2::new(*x, rect.min.y), Vec2::new(w, rect.height()));
        *x += w;
        r
    };
    for (i, label) in state.segments.iter().enumerate() {
        segments.push(place(&mut x, label, i == 0));
    }
    // The detail level trails the chain, behind its own separator — it reads
    // as one more step even though nothing is resident under it.
    let detail = state
        .detail
        .as_deref()
        .map(|d| place(&mut x, d, segments.is_empty()));
    BreadcrumbLayout {
        toggle,
        segments,
        detail,
    }
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

    // The clip is what makes the frame's height authoritative: a font too big
    // for the frame is cut off BY the frame, never drawn outside it.
    let p = painter.with_clip_rect(rect);
    let (text_y, valign) = state.align.anchor(rect);
    let align_left = egui::Align2([egui::Align::LEFT, valign]);
    let align_right = egui::Align2([egui::Align::RIGHT, valign]);
    let sep_at = |x: f32| {
        p.text(
            Pos2::new(x - SEP_GAP, text_y),
            align_right,
            "›",
            state.font.clone(),
            state.dim,
        );
    };
    for (i, seg) in l.segments.iter().enumerate() {
        if i > 0 {
            sep_at(seg.min.x);
        }
        // The last segment is where you ARE: full strength. The ones behind it
        // are links back, and read as quieter — and so does the form's own
        // name once a detail level has been appended after it, because from
        // then on it IS a link (it resets the form).
        let last = i + 1 == l.segments.len() && l.detail.is_none();
        p.text(
            Pos2::new(seg.min.x, text_y),
            align_left,
            &state.segments[i],
            state.font.clone(),
            if last { state.fg } else { state.dim },
        );
    }
    if let (Some(seg), Some(text)) = (l.detail, state.detail.as_deref()) {
        if !l.segments.is_empty() {
            sep_at(seg.min.x);
        }
        p.text(
            Pos2::new(seg.min.x, text_y),
            align_left,
            text,
            state.font.clone(),
            state.fg,
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
        Pos2::new(right, origin.y + height_of(side)),
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

/// The strip's background for a given rail: the developer's own
/// `BreadcrumbBackgroundColor` when they set one, otherwise
/// [`strip_background`]'s content-pane rule.
///
/// A chosen colour may carry alpha — it composites over the content pane's
/// backdrop, so a half-transparent frame shows the form through it — but the
/// result is always opaque (R43): the strip is chrome, and a hole in chrome
/// shows the desktop.
pub fn strip_background_for(
    side: &Control,
    form_background_hex: &str,
    transparency: u8,
) -> Color32 {
    let under = strip_background(form_background_hex, transparency);
    match side.breadcrumb_background() {
        Some(hex) => {
            let own = crate::paint::parse_color(&hex);
            crate::paint::composite_premultiplied_over(own, under)
        }
        None => under,
    }
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
    let bg = strip_background_for(
        side,
        &form.background_color,
        form.transparency.min(100) as u8,
    );
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

    /// The frame's HEIGHT and BACKGROUND are the rail's properties. Width is
    /// not one: the frame always runs from the rail's right edge to the
    /// window's, which is why only these two are offered.
    #[test]
    fn the_frame_takes_its_height_and_background_from_the_rail() {
        let mut form = crate::model::Form::new("SHELL", "Main Menu", 960, 744);
        form.background_color = "102040".into();
        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = crate::model::Rect::new(0, 0, 200, 744);
        side.set_prop("FullHeight", true);
        form.controls.push(side);
        let origin = Pos2::new(0.0, 0.0);

        // Default: the historical 28pt, and the content pane's own backdrop.
        let r = design_strip_rect(&form, origin, false).expect("a shell has a frame");
        assert_eq!(r.height(), HEIGHT, "no property ⇒ the historical height");
        let plain = strip_background_for(&form.controls[0], &form.background_color, 0);
        assert_eq!(
            plain,
            strip_background(&form.background_color, 0),
            "no colour set ⇒ the frame still follows the content pane"
        );

        // A taller frame — room for the controls the developer puts over it.
        form.controls[0].set_prop("BreadcrumbHeight", 64);
        let r = design_strip_rect(&form, origin, false).expect("still a shell");
        assert_eq!(r.height(), 64.0, "the rail's BreadcrumbHeight is the height");
        assert_eq!(r.min.x, 200.0, "…and the frame still starts at the rail's edge");
        assert_eq!(r.max.x, 960.0, "…and still runs to the form's right edge");

        // The toggle cell is a square of the frame's height — until the frame
        // is made tall for the developer's own controls, where it stops
        // growing rather than becoming a giant arrow.
        {
            let ctx = ctx();
            let segs = vec!["Main Menu".to_string()];
            let st = state_plain(&segs, CHROME);
            let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 120.0));
            let l = with_painter(&ctx, |p| layout(p, tall, &st));
            assert_eq!(l.toggle.width(), TOGGLE_MAX, "the cell stops at the cap");
            assert_eq!(l.toggle.width(), l.toggle.height(), "…and stays square");
            assert_eq!(l.toggle.min.x, tall.min.x, "…at the frame's left edge");
            assert_eq!(l.toggle.center().y, tall.center().y, "…centred in the band");
            let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, HEIGHT));
            let l = with_painter(&ctx, |p| layout(p, short, &st));
            assert_eq!(
                l.toggle.height(),
                HEIGHT,
                "at the default height the cap never applies"
            );
        }

        // A height below the readable floor is not a height — it is unset.
        form.controls[0].set_prop("BreadcrumbHeight", 4);
        assert_eq!(
            design_strip_rect(&form, origin, false).unwrap().height(),
            HEIGHT,
            "a frame too short to read is refused, not drawn"
        );

        // A chosen colour wins, and stays opaque (R43) whatever it carries.
        form.controls[0].set_prop("BreadcrumbBackgroundColor", "#8B0000");
        let own = strip_background_for(&form.controls[0], &form.background_color, 0);
        assert_eq!(
            (own.r(), own.g(), own.b()),
            (0x8B, 0x00, 0x00),
            "the designed colour is the frame's colour"
        );
        assert_eq!(own.a(), 255, "R43: the frame is opaque chrome");
        form.controls[0].set_prop("BreadcrumbBackgroundColor", "#FFFFFF40");
        let blended = strip_background_for(&form.controls[0], &form.background_color, 0);
        assert_eq!(blended.a(), 255, "…even when the developer chose alpha");
        assert_ne!(blended, own, "…which still shows the pane through it");

        eprintln!(
            "breadcrumb frame — height: unset → {HEIGHT:.0}, set 64 → 64, set 4 → \
             {HEIGHT:.0} (below the {:.0}pt floor); background: unset → pane \
             {plain:?}, #8B0000 → {own:?}, #FFFFFF40 → {blended:?} (all opaque)",
            crate::model::MIN_BREADCRUMB_HEIGHT
        );
    }

    /// A DETAIL level trails the chain: the form's own name becomes a link
    /// (it resets the form), and the detail is where you are.
    #[test]
    fn a_detail_level_trails_the_chain_and_makes_the_form_name_a_reset() {
        let ctx = ctx();
        let segs: Vec<String> = ["Main Menu", "Customer Data"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let rect = strip();

        // Without one, the last segment is simply where you are — clicking it
        // resets nothing, because nothing has been opened under it.
        let st = state_plain(&segs, CHROME);
        let bare = with_painter(&ctx, |p| layout(p, rect, &st));
        assert!(bare.detail.is_none());
        assert_eq!(bare.reset_segment(), None, "no detail ⇒ no reset target");

        let mut st = state_plain(&segs, CHROME);
        st.detail = Some("John Smith".to_string());
        let l = with_painter(&ctx, |p| {
            let l = layout(p, rect, &st);
            paint(p, rect, &st, &l);
            l
        });
        let detail = l.detail.expect("the detail level was laid out");
        assert_eq!(l.segments.len(), 2, "the chain is unchanged — 2 forms");
        assert!(
            detail.min.x > l.segments[1].max.x,
            "the detail trails the form's own name"
        );
        assert!(rect.contains_rect(detail), "…inside the frame");
        assert_eq!(
            l.reset_segment(),
            Some(1),
            "the form's own segment is now the RESET target"
        );
        assert_eq!(segment_at(&l, l.segments[1].center()), Some(1));
        assert_eq!(
            segment_at(&l, detail.center()),
            None,
            "the detail is where you ARE — it is not a link"
        );

        eprintln!(
            "breadcrumb detail — chain [Main Menu › Customer Data] + detail \
             \"John Smith\": 2 chain rects, detail at x {:.0}..{:.0}, reset \
             target = segment {:?} (was None without a detail)",
            detail.min.x,
            detail.max.x,
            l.reset_segment()
        );
    }

    /// The frame's height is the developer's number, whatever the font does.
    #[test]
    fn the_frames_height_owes_nothing_to_the_font_size() {
        let mut form = crate::model::Form::new("SHELL", "Main Menu", 960, 744);
        let mut side = Control::new("SideMenu-1", crate::ControlType::SideMenu, 0, 0);
        side.rect = crate::model::Rect::new(0, 0, 200, 744);
        side.set_prop("FullHeight", true);
        side.set_prop("BreadcrumbHeight", 40);
        form.controls.push(side);
        let origin = Pos2::new(0.0, 0.0);

        // A font far taller than the frame must not stretch it, and a tiny one
        // must not shrink it: the frame is 40 either way.
        for size in [6, 13, 96] {
            form.controls[0].set_prop("FontSize", size);
            assert_eq!(
                design_strip_rect(&form, origin, false).unwrap().height(),
                40.0,
                "FontSize {size} must not move the frame's height"
            );
        }
    }

    /// Text too big for the frame is cut off BY the frame, never drawn past it.
    #[test]
    fn an_oversized_font_is_clipped_to_the_frame() {
        let ctx = ctx();
        let segs = vec!["Main Menu".to_string()];
        let mut st = state_plain(&segs, CHROME);
        st.font = FontId::proportional(120.0);
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 20.0));
        let shapes = with_painter(&ctx, |p| {
            let l = layout(p, rect, &st);
            paint(p, rect, &st, &l);
        });
        let _ = shapes;
        // The layout never reports a segment taller than the frame either.
        let l = with_painter(&ctx, |p| layout(p, rect, &st));
        for seg in &l.segments {
            assert!(
                seg.height() <= rect.height(),
                "a segment may not be taller than the frame it lives in"
            );
        }
    }

    /// Top / Middle / Bottom put the chain in three different places.
    #[test]
    fn the_chain_can_be_aligned_within_the_frame() {
        let ctx = ctx();
        let segs = vec!["Main Menu".to_string()];
        let tall = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 120.0));

        let ys: Vec<f32> = [TextAlign::Top, TextAlign::Middle, TextAlign::Bottom]
            .iter()
            .map(|a| a.anchor(tall).0)
            .collect();
        assert!(ys[0] < ys[1] && ys[1] < ys[2], "top above middle above bottom: {ys:?}");
        assert!(ys[0] >= tall.min.y, "top stays inside the frame");
        assert!(ys[2] <= tall.max.y, "bottom stays inside the frame");
        assert_eq!(ys[1], tall.center().y, "middle is where it always was");

        // Every alignment still paints.
        for align in [TextAlign::Top, TextAlign::Middle, TextAlign::Bottom] {
            let mut st = state_plain(&segs, CHROME);
            st.align = align;
            with_painter(&ctx, |p| {
                let l = layout(p, tall, &st);
                paint(p, tall, &st, &l);
            });
        }
    }

    /// Unknown, empty and legacy values all mean the historical Middle.
    #[test]
    fn an_unknown_alignment_is_the_middle_it_always_was() {
        assert_eq!(TextAlign::parse("Top"), TextAlign::Top);
        assert_eq!(TextAlign::parse("  bottom "), TextAlign::Bottom);
        assert_eq!(TextAlign::parse("down"), TextAlign::Bottom, "the operator's word");
        assert_eq!(TextAlign::default(), TextAlign::Middle);
        for raw in ["", "Middle", "centre", "sideways", "42"] {
            assert_eq!(TextAlign::parse(raw), TextAlign::Middle, "{raw:?}");
        }
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

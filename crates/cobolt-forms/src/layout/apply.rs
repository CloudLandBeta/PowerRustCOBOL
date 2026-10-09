// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Surfaces prepare before rendering** (spec 056 plan §1.1, R26).
//!
//! The solver is pure; this is the render-side glue every surface calls on a
//! RESPONSIVE form before it hands controls to the engine:
//!
//! 1. measure `AutoSize` controls at their effective font size (R26 step 3),
//! 2. solve the layout for the surface's available size,
//! 3. hand back laid-out COPIES of the controls — rect replaced, marked
//!    `_LaidOut` so the engine's own AutoSize pass leaves them alone, and
//!    carrying `_EffectiveFontSize` where font scaling changed the size.
//!
//! The existing mechanisms (moved containers, Splitter panes, the SideMenu
//! rail, repeating groups) then run on those rects exactly as they run on
//! designed ones (R26 step 5). A non-responsive form never comes here (R3).

use std::collections::{BTreeMap, HashMap};

use crate::layout::breakpoints::Breakpoint;
use crate::layout::fonts::EFFECTIVE_FONT_SIZE;
use crate::layout::{solve, LayoutInput, LayoutOutput};
use crate::model::{Control, PropValue, Rect};
use crate::render::{FormState, RenderTransform};

/// Marks a control the layout placed: the engine's AutoSize pass skips it,
/// because its size was already measured before layout (R26 step 3).
pub const LAID_OUT: &str = "_LaidOut";

/// A control's visibility as the active breakpoint sets it, when it sets it
/// (R62): `false` hidden, `true` shown. Never saved.
pub const BREAKPOINT_VISIBLE: &str = "_BreakpointVisible";

/// What a responsive form looks like to the layout.
pub struct FormSpec<'a> {
    pub designed_size: (f32, f32),
    pub layout: &'a BTreeMap<String, PropValue>,
    pub breakpoints: &'a [Breakpoint],
    /// The operating system's text-size factor (R68), 1.0 where it has none.
    pub system_text_factor: f32,
    /// `me::Breakpoint` and `me::FontScale` pinned by the program (R84).
    pub pinned_breakpoint: Option<&'a str>,
    pub pinned_font_scale: Option<f32>,
}

/// The result of [`prepare`].
pub struct Prepared {
    /// The controls to render, laid out.
    pub controls: Vec<Control>,
    /// The size the form was laid out at — the surface, never below the form's
    /// minimum; what the surface's scroll area should hold.
    pub form_size: egui::Vec2,
    pub layout: LayoutOutput,
}

/// Lay a responsive form out for a surface of `available` size.
pub fn prepare(
    ctx: &egui::Context,
    controls: &[Control],
    state: &dyn FormState,
    form: &FormSpec<'_>,
    available: egui::Vec2,
) -> Prepared {
    // The effective font sizes come first: an AutoSize control is measured at
    // the size it will PAINT at (R26 steps 2–3). The solver is arithmetic, so
    // asking it twice costs nothing worth measuring.
    let mut first = LayoutInput::new(
        controls,
        form.designed_size,
        (available.x, available.y),
        form.layout,
        form.breakpoints,
    );
    first.system_text_factor = form.system_text_factor;
    first.pinned_breakpoint = form.pinned_breakpoint;
    first.pinned_font_scale = form.pinned_font_scale;
    let fonts = solve(&first).font_sizes;
    // AutoSize controls enter layout at the size their LIVE content needs.
    let mut intrinsic: HashMap<String, (f32, f32)> = HashMap::new();
    for c in controls {
        let mut live = state.live(c);
        if let Some(size) = fonts.get(&c.id) {
            live.set_prop(EFFECTIVE_FONT_SIZE, PropValue::String(size.to_string()));
        }
        if let Some(r) = crate::paint::autosize_rect(ctx, &live) {
            intrinsic.insert(c.id.clone(), (r.w as f32, r.h as f32));
        }
    }
    let mut input = LayoutInput::new(
        controls,
        form.designed_size,
        (available.x, available.y),
        form.layout,
        form.breakpoints,
    );
    input.intrinsic = Some(&intrinsic);
    input.system_text_factor = form.system_text_factor;
    input.pinned_breakpoint = form.pinned_breakpoint;
    input.pinned_font_scale = form.pinned_font_scale;
    let layout = solve(&input);
    let laid = laid_out_controls(controls, &layout);
    Prepared {
        controls: laid,
        form_size: egui::vec2(layout.laid_out_size.0, layout.laid_out_size.1),
        layout,
    }
}

/// [`prepare`], then the SideMenu rail drawn at the width it is SHOWN at —
/// `rail` is the SideMenu's id and whether it is shown collapsed — narrowing
/// on the LAID-OUT rects as it narrows on designed ones (R26 step 5). The one
/// order every surface that draws a rail itself follows: a child window, the
/// preview and the designer canvas.
pub fn prepare_with_rail(
    ctx: &egui::Context,
    controls: &[Control],
    state: &dyn FormState,
    form: &FormSpec<'_>,
    available: egui::Vec2,
    rail: Option<(&str, bool)>,
) -> Prepared {
    let mut p = prepare(ctx, controls, state, form, available);
    if let Some((side_id, collapsed)) = rail {
        if let Some(side) = p.controls.iter().find(|c| c.id == side_id).cloned() {
            p.controls = crate::sidebar::rail_view(&p.controls, &side, collapsed);
        }
    }
    p
}

/// `controls` with every laid-out rect applied (rounded to whole pixels, the
/// model's unit), marked [`LAID_OUT`], and with the effective font size where
/// it differs from the designed one.
pub fn laid_out_controls(controls: &[Control], layout: &LayoutOutput) -> Vec<Control> {
    // A control in a layer was not laid out (spec 091 R21): it is not marked, so
    // what a laid-out control is spared — an `AutoSize` measurement, say — is
    // still done for it.
    let layered = crate::layout::layered_flags(controls);
    controls
        .iter()
        .zip(layered)
        .map(|(c, in_layer)| {
            let mut c = c.clone();
            if in_layer {
                return c;
            }
            if let Some(r) = layout.rects.get(&c.id) {
                c.rect = Rect::new(
                    r.x.round() as i32,
                    r.y.round() as i32,
                    r.w.round() as i32,
                    r.h.round() as i32,
                );
            }
            c.set_prop(LAID_OUT, PropValue::Bool(true));
            if layout.hidden.contains(&c.id) || layout.expanded_away.contains(&c.id) {
                c.set_prop(BREAKPOINT_VISIBLE, PropValue::Bool(false));
            } else if layout.shown.contains(&c.id) {
                c.set_prop(BREAKPOINT_VISIBLE, PropValue::Bool(true));
            }
            if let Some(size) = layout.font_sizes.get(&c.id) {
                if let Some(designed) = crate::layout::fonts::designed_size_opt(&c) {
                    if (*size - designed).abs() > f32::EPSILON {
                        c.set_prop(EFFECTIVE_FONT_SIZE, PropValue::String(size.to_string()));
                    }
                }
            }
            c
        })
        .collect()
}

/// The host's live state, with each control kept at its LAID-OUT rect.
///
/// The engine merges live properties over the control it is given; a live
/// `X`/`Y`/`Width`/`Height` would put a laid-out control back at the value
/// seeded from its design, and `moved_ancestor_offset` would read that as the
/// container having MOVED and offset its children a second time. Geometry
/// COBOL writes reaches a responsive form through the layout instead (R38).
pub struct LaidOutState<'a> {
    pub inner: &'a dyn FormState,
}

impl FormState for LaidOutState<'_> {
    fn run_id(&self) -> u64 {
        self.inner.run_id()
    }
    fn live(&self, base: &Control) -> Control {
        let mut c = self.inner.live(base);
        c.rect = base.rect;
        c
    }
    fn visible(&self, base: &Control) -> bool {
        // The active breakpoint's word on it, else the live state (R62).
        // TODO(T7.1): a COBOL write to `Visible` wins over the breakpoint (R64).
        match base.get_prop(BREAKPOINT_VISIBLE) {
            Some(v) => v.as_bool(),
            None => self.inner.visible(base),
        }
    }
    fn enabled(&self, base: &Control) -> bool {
        self.inner.enabled(base)
    }
    fn transform(&self, base: &Control) -> RenderTransform {
        self.inner.transform(base)
    }
    fn decimal_comma(&self) -> bool {
        self.inner.decimal_comma()
    }
    fn currency(&self) -> char {
        self.inner.currency()
    }
    fn viewer_document(
        &self,
        base: &Control,
        source: &str,
    ) -> Option<std::sync::Arc<crate::paint::ViewerDocument>> {
        self.inner.viewer_document(base, source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::ActiveTabs;
    use crate::render::{merge_props, render_form, Backdrop, RenderInput, RenderMode};
    use crate::ControlType;

    /// The run-form host's merge: every designed prop stringified and merged.
    struct Stringified;
    impl FormState for Stringified {
        fn live(&self, base: &Control) -> Control {
            let props: HashMap<String, String> = base
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.to_xml_string()))
                .chain([("X".to_owned(), "300".to_owned())].into_iter().filter(|_| base.id == "PNL"))
                .collect();
            merge_props(base, props.iter())
        }
    }

    fn rects(controls: &[Control], state: &dyn FormState, size: egui::Vec2) -> HashMap<String, egui::Rect> {
        let ctx = egui::Context::default();
        let active = ActiveTabs::new();
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size));
        let mut out = HashMap::new();
        let mut full = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
                let inp = RenderInput {
                    controls,
                    state,
                    form_size: size,
                    glass: true,
                    mode: RenderMode::Interactive,
                    active_tabs: &active,
                    backdrop: Backdrop::default(),
                };
                out = render_form(ui, &inp).control_rects;
            });
        });
        full.textures_delta.clear();
        out
    }

    /// A Panel anchored `Top,Right`, designed at x 300 in a 600 form, lands at
    /// x 500 at 800; its `Top,Left` child moves with it by +200 — ONCE. The
    /// host's state still carries the panel's designed `X = 300`; through
    /// [`LaidOutState`] the engine neither puts the panel back there nor reads
    /// the difference as a COBOL move and offsets the child a second time.
    /// R62 — a control a breakpoint hides is marked on the laid-out list
    /// and the render's state reads it as hidden; one it shows reads as
    /// shown; the rest follow the live state.
    #[test]
    fn a_breakpoint_hides_and_shows_through_the_laid_out_state() {
        let a = Control::new("A", ControlType::Label, 0, 0);
        let b = Control::new("B", ControlType::Label, 0, 0);
        let c = Control::new("C", ControlType::Label, 0, 0);
        let mut layout = LayoutOutput::default();
        layout.hidden.insert("A".into());
        layout.shown.insert("B".into());
        let laid = laid_out_controls(&[a, b, c], &layout);
        let st = LaidOutState { inner: &crate::render::DesignedState };
        assert_eq!(laid.iter().map(|c| st.visible(c)).collect::<Vec<_>>(), vec![false, true, true]);
    }

    #[test]
    fn a_laid_out_container_carries_its_child_once() {
        let mut pnl = Control::new("PNL", ControlType::Panel, 0, 0);
        pnl.rect = Rect::new(300, 50, 200, 150);
        pnl.set_prop("Anchor", PropValue::String("Top,Right".into()));
        let mut kid = Control::new("KID", ControlType::Button, 0, 0);
        kid.rect = Rect::new(320, 70, 80, 30);
        kid.parent = Some("PNL".into());
        let controls = vec![pnl, kid];
        let bag = BTreeMap::new();
        let form = FormSpec { designed_size: (600.0, 400.0), layout: &bag, breakpoints: &[], system_text_factor: 1.0, pinned_breakpoint: None, pinned_font_scale: None };
        let ctx = egui::Context::default();
        let p = prepare(&ctx, &controls, &Stringified, &form, egui::vec2(800.0, 400.0));
        let state = LaidOutState { inner: &Stringified };
        let r = rects(&p.controls, &state, egui::vec2(800.0, 400.0));
        assert_eq!(r["PNL"].min.x, 500.0);
        assert_eq!(r["KID"].min.x, 520.0, "the child moved with the panel once, not twice");
        // Without the wrapper the designed X comes back and the child is dragged.
        let raw = rects(&p.controls, &Stringified, egui::vec2(800.0, 400.0));
        assert_ne!((raw["PNL"].min.x, raw["KID"].min.x), (500.0, 520.0));
        println!("moved-ancestor rule: panel {:.0} → {:.0}, child {:.0} (once); unwrapped state gives panel {:.0}, child {:.0}",
            300.0, r["PNL"].min.x, r["KID"].min.x, raw["PNL"].min.x, raw["KID"].min.x);
    }

    /// R26 step 3 — an AutoSize Label is measured at the size it will paint:
    /// under Fluid type at twice the designed width (factor capped at 1.5) it
    /// comes out wider than at the designed width.
    #[test]
    fn an_autosize_label_is_measured_at_its_effective_size() {
        let mut lbl = Control::new("LBL", ControlType::Label, 0, 0);
        lbl.rect = Rect::new(10, 10, 40, 20);
        lbl.set_prop("Caption", PropValue::String("Customer name".into()));
        lbl.set_prop("AutoSize", PropValue::Bool(true));
        lbl.set_prop("FontSize", PropValue::Int(14));
        let controls = vec![lbl];
        let bag = BTreeMap::from([("FontScaling".to_owned(), PropValue::String("Fluid".into()))]);
        let form = FormSpec { designed_size: (400.0, 300.0), layout: &bag, breakpoints: &[], system_text_factor: 1.0, pinned_breakpoint: None, pinned_font_scale: None };
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::fonts::base_font_definitions());
        let (mut base, mut wide) = (None, None);
        let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
            let at = |w: f32| prepare(ui.ctx(), &controls, &crate::render::DesignedState, &form, egui::vec2(w, 300.0));
            base = Some(at(400.0));
            wide = Some(at(800.0));
        });
        full.textures_delta.clear();
        let (base, wide) = (base.unwrap(), wide.unwrap());
        let (bw, ww) = (base.controls[0].rect.w, wide.controls[0].rect.w);
        assert!(ww > bw, "measured at 1.5× type it is wider: {bw} → {ww}");
        assert_eq!(wide.layout.font_sizes["LBL"], 21.0);
        assert_eq!(wide.controls[0].get_prop(EFFECTIVE_FONT_SIZE).map(|v| v.to_xml_string()).as_deref(), Some("21"));
        assert!(base.controls[0].get_prop(EFFECTIVE_FONT_SIZE).is_none(), "no change, no effective size");
        println!("AutoSize under Fluid: {bw} px at 1×, {ww} px at 1.5× (font 14 → 21)");
    }
}

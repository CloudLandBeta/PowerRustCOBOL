// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **Font scaling** (spec 056 §4.14, R66–R69).

use crate::layout::breakpoints::Breakpoint;
use crate::layout::defaults;
use crate::layout::props::{number_of, PropSource};
use crate::model::Control;

/// The form's `FontScaling`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontScaling {
    /// Today's sizes.
    None,
    /// Follows the surface's width between `MinFontScale` and `MaxFontScale`.
    Fluid,
    /// The active breakpoint's factor.
    Stepped,
}

pub fn font_scaling(form: &dyn PropSource) -> FontScaling {
    match form.text("FontScaling").trim().to_ascii_lowercase().as_str() {
        "fluid" => FontScaling::Fluid,
        "stepped" => FontScaling::Stepped,
        _ => FontScaling::None,
    }
}

/// The form's font factor (R67), times the system text factor (R68) — or the
/// total COBOL pinned through `me::FontScale` (R84).
pub fn form_factor(
    form: &dyn PropSource,
    designed_width: f32,
    available_width: f32,
    bp: Option<&Breakpoint>,
    system: f32,
    pinned: Option<f32>,
) -> f32 {
    if let Some(p) = pinned.filter(|p| *p > 0.0) {
        return p;
    }
    let base = match font_scaling(form) {
        FontScaling::None => 1.0,
        FontScaling::Fluid => {
            if designed_width > 0.0 {
                let lo = form.number("MinFontScale");
                let hi = form.number("MaxFontScale");
                (available_width / designed_width).max(lo).min(hi.max(lo))
            } else {
                1.0
            }
        }
        FontScaling::Stepped => bp
            .map(|b| b.font_factor)
            .unwrap_or(defaults::BREAKPOINT_FONT_FACTOR),
    };
    base * system
}

/// A control's designed `FontSize`, read as the engine will read it from now
/// on: trimmed, decimals accepted; absent or unreadable is the engine's
/// missing-size default.
pub fn designed_size(c: &Control) -> f32 {
    c.get_prop("FontSize")
        .and_then(number_of)
        .filter(|v| *v > 0.0)
        .unwrap_or(defaults::FONT_SIZE_MISSING)
}

/// The key the layout writes a control's effective font size under, for the
/// paint sites to read through [`resolve_font_size`] (R70). Never saved: a
/// leading underscore marks a runtime-only property.
pub const EFFECTIVE_FONT_SIZE: &str = "_EffectiveFontSize";

/// **The one font-size resolver** (R70): every paint site that reads a
/// control's `FontSize` asks here. The laid-out effective size when the
/// layout set one; otherwise the designed `FontSize`, trimmed and with
/// decimals accepted; otherwise the site's own `missing` default — then
/// clamped to the site's own `[min, max]`, exactly as each site clamped
/// before. Integer sizes resolve to what they always painted at; what
/// changes is that `"18.5"` is 18.5, where most sites used to read it as 0
/// and floor it to their minimum.
pub fn resolve_font_size(c: &Control, missing: f32, min: f32, max: f32) -> f32 {
    resolved(c).unwrap_or(missing).clamp(min, max)
}

/// [`resolve_font_size`] for a site that keeps its own style when the control
/// names no size: `None` then, and the site's bounds otherwise.
pub fn resolve_font_size_opt(c: &Control, min: f32, max: f32) -> Option<f32> {
    resolved(c).map(|v| v.clamp(min, max))
}

fn resolved(c: &Control) -> Option<f32> {
    let positive = |key: &str| c.get_prop(key).and_then(number_of).filter(|v| *v > 0.0);
    positive(EFFECTIVE_FONT_SIZE).or_else(|| positive("FontSize"))
}

/// The size a control paints at (R69): `FontSize` × `factor`, clamped to its
/// `MinFontSize`/`MaxFontSize` (0 = none) and to the engine's own bounds. A
/// control with `ScaleFont = false` keeps its designed size.
pub fn effective_size(c: &Control, factor: f32) -> f32 {
    let size = designed_size(c);
    let mut v = if c.flag("ScaleFont") { size * factor } else { size };
    if c.flag("ScaleFont") {
        let (lo, hi) = (c.number("MinFontSize"), c.number("MaxFontSize"));
        if hi > 0.0 && v > hi {
            v = hi;
        }
        if lo > 0.0 && v < lo {
            v = lo;
        }
    }
    v.clamp(defaults::FONT_SIZE_MIN, defaults::FONT_SIZE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::defaults::default_breakpoints;
    use crate::layout::props::FormBag;
    use crate::model::{ControlType, PropValue};
    use std::collections::BTreeMap;

    fn bag(scaling: &str) -> BTreeMap<String, PropValue> {
        BTreeMap::from([("FontScaling".to_owned(), PropValue::String(scaling.into()))])
    }

    /// AC32 — Fluid at 0.5×, 1× and 3× the designed width clamps to the
    /// default limits 0.85, 1.0 and 1.5.
    #[test]
    fn fluid_follows_the_width_between_its_limits() {
        let b = bag("Fluid");
        let f = |avail: f32| form_factor(&FormBag(&b), 800.0, avail, None, 1.0, None);
        assert_eq!(f(400.0), 0.85);
        assert_eq!(f(800.0), 1.0);
        assert_eq!(f(2400.0), 1.5);
        assert_eq!(f(960.0), 1.2);
        println!("Fluid: 0.5x → {}, 1x → {}, 1.2x → {}, 3x → {}", f(400.0), f(800.0), f(960.0), f(2400.0));
    }

    #[test]
    fn stepped_uses_the_active_breakpoints_factor_and_none_is_one() {
        let mut t = default_breakpoints();
        t[0].font_factor = 0.9;
        let b = bag("Stepped");
        assert_eq!(form_factor(&FormBag(&b), 800.0, 400.0, Some(&t[0]), 1.0, None), 0.9);
        assert_eq!(form_factor(&FormBag(&bag("None")), 800.0, 400.0, Some(&t[0]), 1.0, None), 1.0);
        // The system factor multiplies (R68); a pin replaces the total (R84).
        assert_eq!(form_factor(&FormBag(&bag("None")), 800.0, 400.0, None, 1.25, None), 1.25);
        assert_eq!(form_factor(&FormBag(&b), 800.0, 400.0, Some(&t[0]), 1.25, Some(2.0)), 2.0);
        assert_eq!(form_factor(&FormBag(&b), 800.0, 400.0, Some(&t[0]), 1.0, Some(0.0)), 0.9);
    }

    /// R70 — the resolver keeps each site's default and bounds for what they
    /// always read, and reads what they used to drop.
    #[test]
    fn the_resolver_keeps_every_site_and_reads_decimals() {
        let mut c = Control::new("L", ControlType::Label, 0, 0);
        c.set_prop("FontSize", PropValue::Int(14));
        assert_eq!(resolve_font_size(&c, 11.0, 4.0, 200.0), 14.0, "an integer resolves as it always did");
        c.set_prop("FontSize", PropValue::String(" 18.5 ".into()));
        assert_eq!(resolve_font_size(&c, 11.0, 4.0, 200.0), 18.5, "not 4 pt any more");
        c.set_prop("FontSize", PropValue::Int(300));
        assert_eq!(resolve_font_size(&c, 12.0, 6.0, 72.0), 72.0, "the site's own ceiling");
        c.properties.shift_remove("FontSize");
        assert_eq!(resolve_font_size(&c, 12.0, 6.0, 72.0), 12.0, "the site's own default");
        assert_eq!(resolve_font_size_opt(&c, 8.0, 28.0), None, "a site with its own style keeps it");
        c.set_prop(EFFECTIVE_FONT_SIZE, PropValue::String("23".into()));
        assert_eq!(resolve_font_size(&c, 11.0, 4.0, 200.0), 23.0, "the laid-out size wins");
        assert_eq!(resolve_font_size_opt(&c, 8.0, 28.0), Some(23.0));
    }

    #[test]
    fn effective_sizes_honour_the_control_limits_and_scale_font() {
        let mut c = Control::new("L", ControlType::Label, 0, 0);
        c.set_prop("FontSize", PropValue::String("18.5".into()));
        assert_eq!(effective_size(&c, 1.0), 18.5, "a decimal size is a size, not 4 pt");
        assert_eq!(effective_size(&c, 2.0), 37.0);
        c.set_prop("MaxFontSize", PropValue::Int(30));
        assert_eq!(effective_size(&c, 2.0), 30.0);
        c.set_prop("MinFontSize", PropValue::Int(16));
        assert_eq!(effective_size(&c, 0.5), 16.0);
        c.set_prop("ScaleFont", PropValue::Bool(false));
        assert_eq!(effective_size(&c, 2.0), 18.5);
        c.properties.shift_remove("FontSize");
        assert_eq!(effective_size(&c, 1.0), defaults::FONT_SIZE_MISSING);
    }
}

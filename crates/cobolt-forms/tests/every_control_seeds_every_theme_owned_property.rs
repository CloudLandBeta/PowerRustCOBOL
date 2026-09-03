//! Spec 016 Q2 — the lasting answer to the property-removal class.
//!
//! `Control::reset_theme_owned_props` can only put a property back to what a
//! **new** control of that type carries. A property no control seeds therefore
//! could not be put back at all, and the branch that handled that case used to
//! *remove* the key — which takes the row out of the inspector, so a developer
//! lost the ability to set a property because of a theme switch made for an
//! unrelated reason. That is how a Label kept losing its corner radius.
//!
//! 1.63.15 made the branch harmless (it leaves the value alone). This makes it
//! unreachable: every control that paints a face seeds every theme-owned
//! property, so there is always a seed to restore.
//!
//! The boundary is "paints a face". A non-visual control renders nothing at run
//! time and a Line is a stroke with no frame, so neither is given appearance
//! rows. That is not the "no real frame" reasoning spec 016 Q4 threw out — a
//! Label and the bars have faces, and were excluded for how they *look* by
//! default.

use cobolt_forms::model::{Control, ControlType, THEME_OWNED_PROPS};

/// Controls that paint a face — the ones the rule covers.
fn face_painting() -> Vec<ControlType> {
    ControlType::ALL
        .iter()
        .filter(|ct| !ct.is_non_visual() && !matches!(ct, ControlType::Line))
        .cloned()
        .collect()
}

#[test]
fn a_new_control_carries_every_theme_owned_property() {
    let mut gaps: Vec<String> = Vec::new();
    for ct in face_painting() {
        let c = Control::new("X-1", ct.clone(), 0, 0);
        for key in THEME_OWNED_PROPS {
            if c.get_prop(key).is_none() {
                gaps.push(format!("{ct:?} has no {key}"));
            }
        }
    }
    assert!(
        gaps.is_empty(),
        "every control that paints a face must seed every theme-owned property \
         — otherwise a theme reset has nothing to put back:\n  {}",
        gaps.join("\n  ")
    );
}

#[test]
fn a_theme_reset_can_always_put_every_property_back() {
    // The consequence, stated as the behaviour rather than the seed: stamp the
    // Neumorphic defaults over a fresh control, reset, and every theme-owned
    // property is back to the value a new control carries — none missing, none
    // left on the previous style's mark.
    for ct in face_painting() {
        let fresh = Control::new("X-1", ct.clone(), 0, 0);
        let mut c = fresh.clone();
        c.apply_glass_style_defaults(cobolt_forms::model::GlassStyle::Neumorphic);
        c.apply_glass_style_defaults(cobolt_forms::model::GlassStyle::Classic);
        for key in THEME_OWNED_PROPS {
            assert!(
                c.get_prop(key).is_some(),
                "{ct:?}: {key} went missing across a Neumorphic → Classic switch"
            );
            assert_eq!(
                c.get_prop(key),
                fresh.get_prop(key),
                "{ct:?}: {key} did not come back to what a new control carries"
            );
        }
    }
}

#[test]
fn the_new_seeds_are_what_the_renderer_already_assumed() {
    // A seed added to close the gap must change nothing on screen: it is the
    // value the painter already fell back to when the property was absent. One
    // that differed would silently restyle every existing form.
    //
    // A control that had an opinion of its own keeps it — a ToolBar seeds 10
    // deliberately (operator, 2026-08-16: its artwork was hard-wired to a 2 px
    // round and a seeded 0 would have squared off every existing bar), and that
    // is a designed default, not a gap being filled. It is named here so the
    // exception is a decision on the record rather than a hole in the test.
    let deliberate: &[(ControlType, i64)] = &[(ControlType::ToolBar, 10)];

    for ct in face_painting() {
        // `corner_radius` clamps to half the smaller side, so measure on a
        // control big enough not to be clamped.
        let mut big = Control::new("X-1", ct.clone(), 0, 0);
        big.rect = cobolt_forms::model::Rect::new(0, 0, 400, 400);
        let seeded = big.get_prop("CornerRadius").map(|v| v.as_i64()).unwrap_or(-1);

        if let Some((_, expected)) = deliberate.iter().find(|(k, _)| *k == ct) {
            assert_eq!(seeded, *expected, "{ct:?} keeps its own designed radius");
            continue;
        }
        let mut bare = big.clone();
        bare.properties.shift_remove("CornerRadius");
        assert_eq!(
            seeded as f32,
            cobolt_forms::paint::corner_radius(&bare),
            "{ct:?}: the seeded CornerRadius must equal what the renderer used \
             when the property was absent"
        );
    }
}

#[test]
fn the_seeded_border_style_is_the_renderers_own_fallback() {
    // `draw_control` reads BorderStyle with `.unwrap_or("Single")`, so a
    // control that had no seed was already being drawn as `Single`. Seeding it
    // makes the row appear and changes nothing.
    //
    // A control with its own opinion keeps it: CheckBox, RadioButton and Label
    // seed `None` on purpose — they are a glyph and a caption, not a card.
    for ct in face_painting() {
        let c = Control::new("X-1", ct.clone(), 0, 0);
        let style = c
            .get_prop("BorderStyle")
            .map(|v| v.as_str().to_owned())
            .unwrap_or_default();
        assert!(
            !style.is_empty(),
            "{ct:?} must seed BorderStyle so the row exists"
        );
    }
    for ct in [ControlType::CheckBox, ControlType::RadioButton, ControlType::Label] {
        let c = Control::new("X-1", ct.clone(), 0, 0);
        assert_eq!(
            c.get_prop("BorderStyle").map(|v| v.as_str().to_owned()),
            Some("None".to_owned()),
            "{ct:?} keeps its own `None` — the universal seed must not overrule \
             a control that already had an opinion"
        );
    }
}


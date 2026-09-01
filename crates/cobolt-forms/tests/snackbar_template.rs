// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Spec 055 T4 — the dropped Snackbar is a TEMPLATE, and it survives a `.cfrm`.
//!
//! Two things are pinned here. The first is the documented default of every one
//! of the template's properties (spec §6) — a table, printed, so a reader can
//! compare it against the spec by eye rather than trusting a count.
//!
//! The second is the serialisation risk plan §5 flagged: `ControlType` is
//! serialised **by name**, so appending a variant is backward compatible — but
//! `Expr` carries the opposite rule (bincode, ordinal), and the plan is explicit
//! that this must be *verified* rather than assumed from the neighbouring type.

use cobolt_forms::model::PropValue;
use cobolt_forms::{form_to_string, load_form_from_str, Control, ControlType, Form};

/// Spec §6, verbatim: property → documented default.
const DOCUMENTED: &[(&str, &str)] = &[
    ("Text", ""),
    ("Category", "Info"),
    ("Size", "Medium"),
    ("ShowCategoryIcon", "true"),
    ("CategoryIconSize", "0"),
    ("CategoryIconColor", ""),
    ("BackgroundColor", ""),
    ("BackgroundImage", ""),
    ("BackgroundImageMode", "Fill"),
    ("BackgroundImageOpacity", "15"),
    ("ForegroundColor", ""),
    ("FontName", ""),
    ("FontSize", "14"),
    ("Bold", "false"),
    ("TextWrap", "true"),
    ("CornerRadius", "12"),
    ("CornerRadiusTopLeft", "-1"),
    ("CornerRadiusTopRight", "-1"),
    ("CornerRadiusBottomLeft", "-1"),
    ("CornerRadiusBottomRight", "-1"),
    ("BorderStyle", "None"),
    ("BorderWidth", "1"),
    ("BorderColor", "#00000000"),
    ("ShadowEnabled", "true"),
    ("ShadowColor", "#000000"),
    ("ShadowOpacity", "25"),
    ("ShadowBlur", "12"),
    ("ShadowDirection", "270"),
    ("ShadowDistance", "4"),
    // `-1`, not `0`: "from the Category" (§6) had to be a value distinct from
    // the `0` R6 gives the meaning "never expires". See `effective_timeout_ms`.
    ("Timeout", "-1"),
    ("PauseTimeoutOnHover", "true"),
    ("StackAnchor", "BottomRight"),
    ("Margin", "16"),
    ("StackSpacing", "8"),
    ("StackOrder", "Auto"),
    ("MaximumVisible", "5"),
    ("OverflowBehavior", "Queue"),
    ("Buttons", ""),
];

fn shown(v: &PropValue) -> String {
    match v {
        PropValue::String(s) => s.clone(),
        PropValue::Int(i) => i.to_string(),
        PropValue::Bool(b) => b.to_string(),
    }
}

#[test]
fn a_dropped_snackbar_seeds_every_documented_default() {
    let c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);

    eprintln!("\n  Snackbar template — spec 055 §6");
    eprintln!("  {:<26} {:<14} {}", "property", "seeded", "documented");
    eprintln!("  {:-<26} {:-<14} {:-<12}", "", "", "");
    let mut wrong = Vec::new();
    for (name, want) in DOCUMENTED {
        let got = c.get_prop(name).map(shown);
        let got_s = got.clone().unwrap_or_else(|| "<MISSING>".into());
        eprintln!("  {name:<26} {got_s:<14} {want}");
        if got.as_deref() != Some(*want) {
            wrong.push(format!("{name}: seeded {got_s}, documented {want}"));
        }
    }
    assert!(wrong.is_empty(), "{} default(s) disagree with spec §6:\n  {}", wrong.len(), wrong.join("\n  "));

    // Beyond these sit the BASE properties `Control::new` gives every control
    // before the type-specific block — `Tooltip`, `Cursor`, `ZOrder`, the
    // gradient set and the rest. A Timer carries them too; they are not the
    // Snackbar's own vocabulary and spec §6 does not list them. What matters is
    // that the spec's own set is complete and correct, which is asserted above.
    let documented: std::collections::HashSet<&str> = DOCUMENTED.iter().map(|(n, _)| *n).collect();
    let base: Vec<&String> = c
        .properties
        .keys()
        .filter(|k| !documented.contains(k.as_str()))
        .collect();
    eprintln!(
        "  → {} documented properties all correct; {} inherited base properties alongside them\n",
        DOCUMENTED.len(),
        base.len()
    );
}

#[test]
fn the_template_is_non_visual_and_carries_the_five_lifecycle_events() {
    assert!(ControlType::Snackbar.is_non_visual(), "D1/R1: it lives in the non-visual tray");
    let evs = ControlType::Snackbar.supported_events();
    assert_eq!(
        evs,
        &["onShown", "onClosing", "onClosed", "onTimeout", "onButtonClick"],
        "spec §6's event list"
    );
    // A non-visual template has no mouse or geometry events — it is never on the
    // canvas for the operator to click, and it has no designed rect to resize.
    for absent in ["onClick", "onResize", "onGotFocus"] {
        assert!(!evs.contains(&absent), "a Snackbar template must not offer {absent}");
    }
    eprintln!("\n  Snackbar — non-visual, {} events: {evs:?}\n", evs.len());
}

#[test]
fn a_snackbar_survives_a_cfrm_round_trip_because_control_type_is_keyed_by_name() {
    let mut form = Form::new("SNACK-FORM", "Snack Form", 800, 600);
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    c.set_prop("Text", PropValue::String("Record saved".into()));
    c.set_prop("Category", PropValue::String("Warning".into()));
    c.set_prop("StackAnchor", PropValue::String("TopCenter".into()));
    c.set_prop("Buttons", PropValue::String("retry|Retry|refresh|Left|true".into()));
    form.controls.push(c);

    let xml = form_to_string(&form).expect("serialise");

    // The risk plan §5 named, checked directly: the type is written as its NAME.
    // If this were an ordinal, appending `Snackbar` to the enum would silently
    // re-point every existing form's last control type.
    assert!(
        xml.contains("type=\"Snackbar\""),
        "ControlType must serialise BY NAME, not by ordinal. XML was:\n{xml}"
    );

    let back = load_form_from_str(&xml).expect("deserialise");
    let r = &back.controls[0];
    assert_eq!(r.control_type, ControlType::Snackbar);
    assert_eq!(r.get_prop("Text").map(shown).as_deref(), Some("Record saved"));
    assert_eq!(r.get_prop("Category").map(shown).as_deref(), Some("Warning"));
    assert_eq!(r.get_prop("StackAnchor").map(shown).as_deref(), Some("TopCenter"));
    assert_eq!(
        r.get_prop("Buttons").map(shown).as_deref(),
        Some("retry|Retry|refresh|Left|true"),
        "the Buttons collection survives the pipe separators"
    );

    // Every property that carries a VALUE makes the round trip. The ones that
    // seed empty do not come back as keys at all — the `.cfrm` writer omits an
    // empty value, for every control, and always has.
    //
    // That is exactly why plan §5 requires every read to go through a defaulting
    // accessor: a reloaded Snackbar is *semantically* identical even though five
    // keys are physically absent, because "" and "absent" both mean "the
    // Category decides". Asserting the keys would pin the wrong thing; this
    // asserts the meaning.
    let mut lost = Vec::new();
    for (name, want) in DOCUMENTED {
        if want.is_empty() {
            continue;
        }
        if r.get_prop(name).is_none() {
            lost.push(*name);
        }
    }
    assert!(lost.is_empty(), "propert(ies) with a value lost in the round trip: {lost:?}");

    // The semantic check: a reloaded Snackbar resolves everything a fresh one
    // does, through the accessors that do the defaulting.
    use cobolt_forms::snackbar;
    let fresh = Control::new("SNACK-2", ControlType::Snackbar, 0, 0);
    let mut fresh_warning = fresh.clone();
    fresh_warning.set_prop("Category", PropValue::String("Warning".into()));
    assert_eq!(snackbar::effective_background(r), snackbar::effective_background(&fresh_warning));
    assert_eq!(snackbar::effective_foreground(r), snackbar::effective_foreground(&fresh_warning));
    assert_eq!(snackbar::effective_icon(r), snackbar::effective_icon(&fresh_warning));
    assert_eq!(snackbar::effective_timeout_ms(r), 6000, "Warning's 6000 ms survives the trip");

    let absent: Vec<&str> = DOCUMENTED
        .iter()
        .filter(|(n, w)| w.is_empty() && r.get_prop(n).is_none())
        .map(|(n, _)| *n)
        .collect();
    eprintln!(
        "\n  .cfrm round trip — type written as name (\"Snackbar\"); {} valued properties recovered, \
         0 lost; {} empty-by-default properties omitted by the writer and resolved by the \
         accessors instead: {absent:?}\n",
        DOCUMENTED.iter().filter(|(_, w)| !w.is_empty()).count(),
        absent.len()
    );
}

#[test]
fn an_unknown_control_type_becomes_custom_so_an_older_build_fails_honestly() {
    // The other half of the compatibility claim: an OLDER build reading a form
    // that names `Snackbar` does not silently mistake it for another control —
    // `from_str`'s catch-all turns an unrecognised name into `Custom`, which is
    // visible. That is the "honest outcome" plan §3 describes.
    let unknown = ControlType::from_str("SomethingFromTheFuture");
    match unknown {
        ControlType::Custom { plugin_id, control_id } => {
            assert_eq!(plugin_id, "unknown");
            assert_eq!(control_id, "SomethingFromTheFuture");
        }
        other => panic!("an unknown type must become Custom, got {other:?}"),
    }
    assert_eq!(ControlType::from_str("Snackbar"), ControlType::Snackbar);
    eprintln!("\n  forward compat — an unknown type name becomes Custom{{unknown}}, never a wrong variant\n");
}

#[test]
fn every_category_icon_is_a_real_catalogue_name() {
    // A category default naming an icon the catalogue does not have would draw
    // NOTHING, and a notification with a silently missing icon looks exactly
    // like one whose `ShowCategoryIcon` is off. Pinned here so a rename on
    // either side is a red test rather than a blank square.
    use cobolt_forms::icons::menu_icon_names;
    use cobolt_forms::snackbar::SnackCategory;

    let catalogue: std::collections::HashSet<&str> = menu_icon_names().collect();
    let mut missing = Vec::new();
    eprintln!("\n  category    icon               in catalogue");
    eprintln!("  ---------   ----------------   ------------");
    for cat in SnackCategory::ALL {
        let icon = cat.defaults().icon;
        let ok = catalogue.contains(icon);
        eprintln!("  {:<9}   {:<16}   {}", cat.as_str(), icon, ok);
        if !ok {
            missing.push(format!("{}: {icon}", cat.as_str()));
        }
    }
    assert!(missing.is_empty(), "category icon(s) not in the catalogue: {missing:?}");
    eprintln!("  → 5 category icons resolve; 4 reused from the existing Status set, 1 new\n");
}

#[test]
fn the_stack_anchor_does_not_collide_with_the_canvas_anchor_lock() {
    // `Anchor` is a BASE property on every control — a boolean that locks the
    // control's X/Y against mouse dragging on the design canvas
    // (`Control::is_anchored`). Spec §6 originally named the Snackbar's
    // nine-position stack placement `Anchor` too, which put two different
    // meanings on one key in one property map: toggling the designer's lock
    // checkbox (non-visual controls still show the geometry section) would have
    // written `Anchor: Bool(false)` straight over the notification's placement,
    // and `SnackAnchor::from_prop` would have read that back as BottomRight.
    //
    // They are separate keys now. This pins both halves.
    let mut c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);

    // The base lock is still there, still boolean, still false by default.
    assert_eq!(c.get_prop("Anchor"), Some(&PropValue::Bool(false)));
    assert!(!c.is_anchored());

    // The stack placement is its own key.
    assert_eq!(c.get_prop("StackAnchor").map(shown).as_deref(), Some("BottomRight"));

    // Locking the control on the canvas does NOT move its notifications…
    c.set_prop("StackAnchor", PropValue::String("TopLeft".into()));
    c.set_prop("Anchor", PropValue::Bool(true));
    assert!(c.is_anchored(), "the canvas lock still works");
    assert_eq!(
        cobolt_forms::snackbar::mint(&c).0.anchor,
        cobolt_forms::snackbar::SnackAnchor::TopLeft,
        "the canvas lock must not touch the stack anchor"
    );

    // …and choosing a stack anchor does not lock the control on the canvas.
    let mut d = Control::new("SNACK-2", ControlType::Snackbar, 0, 0);
    d.set_prop("StackAnchor", PropValue::String("BottomCenter".into()));
    assert!(!d.is_anchored(), "the stack anchor must not lock the control");

    eprintln!(
        "\n  Anchor (bool canvas lock) and StackAnchor (9-position placement) are \
         independent: lock=true + StackAnchor=TopLeft → anchored={}, stack={:?}\n",
        c.is_anchored(),
        cobolt_forms::snackbar::mint(&c).0.anchor
    );
}

#[test]
fn last_button_id_is_readable_so_the_documented_handler_compiles() {
    // The Developer's Guide shows `EVALUATE SNACK-1::LastButtonId` in an
    // onButtonClick handler. That property is written by the HOST at run time
    // and never seeded at design time, so it only works if it is listed as
    // runtime-readable — otherwise the IDE's handler lint rejects the very
    // example the guide prints. (Exactly the failure `TOOLBAR-1::LastButton`
    // still has; tracked separately as a fix.)
    let names = cobolt_forms::model::runtime_property_names_for("Snackbar");
    assert!(names.contains(&"LastButtonId"), "guide example needs LastButtonId: {names:?}");
    assert!(names.contains(&"LastButtonIndex"), "and LastButtonIndex: {names:?}");

    // It is deliberately NOT a design-time property — there is no answer until
    // a button is clicked, so seeding one would be a lie.
    let c = Control::new("SNACK-1", ControlType::Snackbar, 0, 0);
    assert!(c.get_prop("LastButtonId").is_none(), "runtime-only, never seeded");

    eprintln!("\n  runtime-readable on Snackbar: {names:?} (neither seeded at design time)\n");
}

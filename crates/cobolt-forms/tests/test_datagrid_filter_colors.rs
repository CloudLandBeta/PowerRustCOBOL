//! The DataGrid's column-filter row is readable, and its colours are the
//! developer's to set (operator, 2026-08-31: "I can barely read the filter text
//! and there are no properties to control the filter data entry field
//! foreground/background colors").
//!
//! Two faults, one cause. The filter well was painted with a hardcoded
//! `Color32::from_rgba_unmultiplied(0, 0, 0, 120)` and the input was built with
//! no `text_color` at all, so its ink came from whatever ambient egui visuals
//! happened to be — dark, under every glass theme. Dark text in a black box on
//! a dark teal header, and no property anywhere to correct it.
//!
//! What is asserted here is the RULE, not one palette: an unset colour must be
//! readable on the well it lands on, and a colour the developer chose must be
//! left exactly alone.

use cobolt_forms::model::{Control, ControlType};
use cobolt_forms::paint::{contrast_ratio, readable_ink_on};
use egui::Color32;

/// WCAG AA for normal text. The filter field is text, not a graphic, so 3:1
/// (which non-text graphics get away with) is not enough.
const AA_TEXT: f32 = 4.5;

#[test]
fn a_datagrid_seeds_both_filter_colour_properties() {
    let grid = Control::new("DG", ControlType::DataGrid, 0, 0);
    for prop in ["FilterForegroundColor", "FilterBackgroundColor"] {
        let v = grid
            .get_prop(prop)
            .unwrap_or_else(|| panic!("a DataGrid must carry {prop} — without it the filter row \
                                       cannot be styled at all, which is the reported bug"));
        assert_eq!(
            v.as_str(),
            "",
            "{prop} defaults to EMPTY so the surface theme decides. A concrete \
             default would pin one palette's colour onto all 32."
        );
    }
}

#[test]
fn an_unset_filter_ink_is_never_unreadable_on_its_well() {
    // The exact pairing that was shipping: a near-black well (the old hardcoded
    // fill over a dark teal header) with dark ambient ink.
    let well = Color32::from_rgb(10, 30, 32);
    let dark_ink = Color32::from_rgb(28, 34, 38);
    assert!(
        contrast_ratio(dark_ink, well) < AA_TEXT,
        "precondition: this is the unreadable pair the operator photographed"
    );

    let ink = readable_ink_on(None, dark_ink, well);
    let ratio = contrast_ratio(ink, well);
    println!("  unset ink on {well:?}: {dark_ink:?} -> {ink:?} ({ratio:.2}:1)");
    assert!(
        ratio >= AA_TEXT,
        "a theme-derived ink must reach {AA_TEXT}:1 on its well; got {ratio:.2}:1"
    );
}

#[test]
fn the_rule_holds_on_a_light_well_too() {
    // The mirror case — a pale filter field must not get white ink.
    let well = Color32::from_rgb(240, 244, 248);
    let pale_ink = Color32::from_rgb(228, 232, 236);
    let ink = readable_ink_on(None, pale_ink, well);
    let ratio = contrast_ratio(ink, well);
    println!("  unset ink on {well:?}: {pale_ink:?} -> {ink:?} ({ratio:.2}:1)");
    assert!(ratio >= AA_TEXT, "light well ⇒ dark ink; got {ratio:.2}:1");
    assert_eq!(ink, Color32::BLACK, "a pale well takes black ink, not white");
}

#[test]
fn a_developer_chosen_filter_ink_is_never_second_guessed() {
    // R8 — an explicit property wins. A deliberately quiet filter row is a
    // legitimate design choice, and the contrast net must not "fix" it.
    let well = Color32::from_rgb(10, 30, 32);
    let quiet = Color32::from_rgb(40, 60, 62);
    assert!(
        contrast_ratio(quiet, well) < AA_TEXT,
        "precondition: this choice is deliberately low-contrast"
    );
    assert_eq!(
        readable_ink_on(Some(quiet), Color32::WHITE, well),
        quiet,
        "an explicitly chosen ink is returned untouched — styling belongs to \
         the developer, and silently overriding it is the opposite of giving \
         them the property they asked for"
    );
}

#[test]
fn every_well_gets_a_readable_ink_whatever_the_theme_supplies() {
    // Sweep the luminance range: no well may leave the filter unreadable, which
    // is the guarantee that a hardcoded pair could never make.
    let mut worst = f32::INFINITY;
    let mut worst_well = Color32::BLACK;
    for step in 0..=32 {
        let v = (step * 8).min(255) as u8;
        let well = Color32::from_rgb(v, v, v);
        // Pretend the theme hands back an ink close to its own well — the
        // failure shape this test exists to catch.
        let ink = readable_ink_on(None, well, well);
        let ratio = contrast_ratio(ink, well);
        if ratio < worst {
            worst = ratio;
            worst_well = well;
        }
    }
    println!("  worst well across the sweep: {worst_well:?} at {worst:.2}:1");
    assert!(
        worst >= AA_TEXT,
        "well {worst_well:?} only reaches {worst:.2}:1 — some palette would \
         ship an unreadable filter row"
    );
}

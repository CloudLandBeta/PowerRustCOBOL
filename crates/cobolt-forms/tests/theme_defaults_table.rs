//! Spec 016 Q2 — the per-theme defaults table.
//!
//! The shape is the operator's (2026-09-03): one **base** every control takes,
//! plus the per-control-type **overrides** the theme's design calls for. That
//! is what the real themes need. Elegance is uniform across every control type;
//! Neumorphic Light is not — a Button is raised with a gradient, a Label has no
//! shadow and a transparent ground, a TextBox is *sunken*. A flat table cannot
//! say "Labels have no shadow"; a full per-type table is forty-three copies of
//! one answer.
//!
//! A table is authored by styling a real form and reading the numbers back off
//! it (`from_form`) — the operator's own way of working: "create/adjust themes
//! visually in a form and then apply to the theme".

use cobolt_forms::model::{
    Control, ControlType, Form, GlassStyle, PropValue, Rect as MRect, ThemeDefaults,
    THEME_OWNED_PROPS,
};

fn control(id: &str, ct: ControlType, props: &[(&str, PropValue)]) -> Control {
    let mut c = Control::new(id, ct, 0, 0);
    c.rect = MRect::new(0, 0, 100, 30);
    for (k, v) in props {
        c.set_prop(*k, v.clone());
    }

    c
}

fn form_of(controls: Vec<Control>) -> Form {
    let mut f = Form::new("F", "T", 640, 480);
    f.controls = controls;
    f
}

#[test]
fn an_override_wins_property_by_property_not_wholesale() {
    // A theme that says "Labels have no shadow" is not also saying they have no
    // corner radius.
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(10));
    d.base.insert("ShadowEnabled".into(), PropValue::Bool(true));
    d.overrides
        .entry("Label".into())
        .or_default()
        .insert("ShadowEnabled".into(), PropValue::Bool(false));

    assert_eq!(
        d.value_for(&ControlType::Label, "ShadowEnabled"),
        Some(&PropValue::Bool(false))
    );
    assert_eq!(
        d.value_for(&ControlType::Label, "CornerRadius"),
        Some(&PropValue::Int(10)),
        "the override names one property; the base still answers for the rest"
    );
    assert_eq!(
        d.value_for(&ControlType::Button, "ShadowEnabled"),
        Some(&PropValue::Bool(true))
    );
}

#[test]
fn a_table_stamps_appearance_and_nothing_else() {
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(14));
    // Not a theme-owned property: a theme owns appearance, never content.
    d.base.insert("Caption".into(), PropValue::String("stolen".into()));

    let mut c = control(
        "B1",
        ControlType::Button,
        &[("Caption", PropValue::String("Save".into()))],
    );
    d.apply_to(&mut c);

    assert_eq!(c.get_prop("CornerRadius"), Some(&PropValue::Int(14)));
    assert_eq!(
        c.get_prop("Caption").map(|v| v.as_str().to_owned()),
        Some("Save".to_owned()),
        "a table may not touch a caption — appearance is the theme's, content is not"
    );
}

#[test]
fn a_table_leaves_controls_that_paint_no_face_alone() {
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(14));
    for ct in [ControlType::Timer, ControlType::Line, ControlType::SqlDatabase] {
        let mut c = control("X", ct.clone(), &[]);
        let before = c.get_prop("CornerRadius").cloned();
        d.apply_to(&mut c);
        assert_eq!(
            c.get_prop("CornerRadius").cloned(),
            before,
            "{ct:?} paints no face, so a theme has nothing to say about it"
        );
    }
}

#[test]
fn harvesting_a_uniform_form_gives_a_base_and_no_overrides() {
    // Elegance's shape: every control type agrees.
    //
    // `ForegroundColor` is set explicitly along with the rest — a data-input
    // control seeds black ink and a Button white, so leaving it at the seeds
    // would be a form that genuinely disagrees, and the harvester would be
    // right to say so.
    let uniform = |id: &str, ct: ControlType| {
        control(
            id,
            ct,
            &[
                ("CornerRadius", PropValue::Int(10)),
                ("ShadowEnabled", PropValue::Bool(false)),
                ("BorderStyle", PropValue::String("None".into())),
                ("ForegroundColor", PropValue::String("#101010".into())),
            ],
        )
    };

    let f = form_of(vec![
        uniform("A", ControlType::Button),
        uniform("B", ControlType::TextBox),
        uniform("C", ControlType::ComboBox),
    ]);
    let d = ThemeDefaults::from_form(&f);

    assert_eq!(d.base.get("CornerRadius"), Some(&PropValue::Int(10)));
    assert_eq!(d.base.get("ShadowEnabled"), Some(&PropValue::Bool(false)));
    assert!(
        d.overrides.is_empty(),
        "nothing disagreed, so nothing is an exception — got {:?}",
        d.overrides
    );
}

#[test]
fn harvesting_records_the_types_that_disagree() {
    // Neumorphic Light's shape, from maps-demo: the Button is raised, the Label
    // is not, and the TextBox is sunken.
    let f = form_of(vec![
        control(
            "B",
            ControlType::Button,
            &[
                ("ShadowEnabled", PropValue::Bool(true)),
                ("ShadowBlurStrength", PropValue::Int(8)),
            ],
        ),
        control(
            "M",
            ControlType::Maps,
            &[
                ("ShadowEnabled", PropValue::Bool(true)),
                ("ShadowBlurStrength", PropValue::Int(8)),
            ],
        ),
        control(
            "L",
            ControlType::Label,
            &[
                ("ShadowEnabled", PropValue::Bool(false)),
                ("ShadowBlurStrength", PropValue::Int(8)),
            ],
        ),
        control(
            "T",
            ControlType::TextBox,
            &[
                ("ShadowEnabled", PropValue::Bool(true)),
                ("ShadowBlurStrength", PropValue::Int(-6)),
            ],
        ),
    ]);
    let d = ThemeDefaults::from_form(&f);

    assert_eq!(
        d.base.get("ShadowEnabled"),
        Some(&PropValue::Bool(true)),
        "three of four types say true, so true is the base"
    );
    assert_eq!(
        d.overrides.get("Label").and_then(|o| o.get("ShadowEnabled")),
        Some(&PropValue::Bool(false)),
        "the Label disagreed, so it is an exception"
    );
    assert_eq!(
        d.overrides
            .get("TextBox")
            .and_then(|o| o.get("ShadowBlurStrength")),
        Some(&PropValue::Int(-6)),
        "the sunken TextBox is an exception on the blur, not on the switch"
    );
    assert!(
        d.overrides.get("TextBox").map_or(true, |o| !o.contains_key("ShadowEnabled")),
        "…and it agreed about ShadowEnabled, so that is not recorded twice"
    );
}

#[test]
fn types_vote_not_controls() {
    // A form holding eleven Labels and one Button is not a theme made of
    // Labels: each control TYPE gets one vote.
    let mut controls: Vec<Control> = (0..11)
        .map(|i| {
            control(
                &format!("L{i}"),
                ControlType::Label,
                &[("CornerRadius", PropValue::Int(4))],
            )
        })
        .collect();
    controls.push(control(
        "B",
        ControlType::Button,
        &[("CornerRadius", PropValue::Int(4))],
    ));
    controls.push(control(
        "T",
        ControlType::TextBox,
        &[("CornerRadius", PropValue::Int(9))],
    ));
    let d = ThemeDefaults::from_form(&form_of(controls));
    assert_eq!(
        d.base.get("CornerRadius"),
        Some(&PropValue::Int(4)),
        "two types say 4 and one says 9"
    );
    assert_eq!(
        d.overrides.get("TextBox").and_then(|o| o.get("CornerRadius")),
        Some(&PropValue::Int(9))
    );
}

#[test]
fn harvesting_the_same_form_twice_gives_the_same_table() {
    // A table that changed between two reads of one file would be impossible to
    // review.
    let f = form_of(vec![
        control("A", ControlType::Button, &[("CornerRadius", PropValue::Int(3))]),
        control("B", ControlType::TextBox, &[("CornerRadius", PropValue::Int(9))]),
        control("C", ControlType::Label, &[("CornerRadius", PropValue::Int(3))]),
    ]);
    assert_eq!(ThemeDefaults::from_form(&f), ThemeDefaults::from_form(&f));
}

#[test]
fn the_table_wins_over_the_shipped_style() {
    // The operator's ruling: the table is the source of truth, and the built-in
    // appliers are the values PowerRustCOBOL merely ships.
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(22));

    let mut c = control("B1", ControlType::Button, &[]);
    c.apply_glass_style_defaults_with(GlassStyle::Neumorphic, Some(&d));
    assert_eq!(
        c.get_prop("CornerRadius"),
        Some(&PropValue::Int(22)),
        "Neumorphic ships 10; this project says 22"
    );

    // …and a property the table says nothing about keeps the shipped answer.
    let mut plain = control("B2", ControlType::Button, &[]);
    plain.apply_glass_style_defaults(GlassStyle::Neumorphic);
    for key in THEME_OWNED_PROPS.iter().filter(|k| **k != "CornerRadius") {
        assert_eq!(
            c.get_prop(key),
            plain.get_prop(key),
            "{key} is not in the table, so the shipped style still answers for it"
        );
    }
}

#[test]
fn a_form_switch_stamps_every_control_including_children() {
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(18));

    let mut panel = control("P1", ControlType::Panel, &[]);
    panel
        .children
        .push(control("B1", ControlType::Button, &[]));
    let mut f = form_of(vec![panel]);
    f.apply_glass_style_defaults_with(GlassStyle::Classic, Some(&d));

    assert_eq!(
        f.controls[0].get_prop("CornerRadius"),
        Some(&PropValue::Int(18))
    );
    assert_eq!(
        f.controls[0].children[0].get_prop("CornerRadius"),
        Some(&PropValue::Int(18)),
        "a control inside a container takes the theme too"
    );
}

#[test]
fn the_table_round_trips_through_toml_as_plain_scalars() {
    // It lives in cobolt.toml, where a developer reads and hand-edits it, so a
    // value must be the scalar itself and not `{ Int = 10 }`.
    let mut d = ThemeDefaults::default();
    d.base.insert("CornerRadius".into(), PropValue::Int(10));
    d.base.insert("BorderStyle".into(), PropValue::String("None".into()));
    d.base.insert("ShadowEnabled".into(), PropValue::Bool(true));
    d.overrides
        .entry("Label".into())
        .or_default()
        .insert("ShadowEnabled".into(), PropValue::Bool(false));

    let text = toml::to_string(&d).expect("serializes");
    assert!(text.contains("CornerRadius = 10"), "got:\n{text}");
    assert!(text.contains("BorderStyle = \"None\""), "got:\n{text}");
    assert!(text.contains("ShadowEnabled = true"), "got:\n{text}");

    let back: ThemeDefaults = toml::from_str(&text).expect("parses");
    assert_eq!(back, d);
}

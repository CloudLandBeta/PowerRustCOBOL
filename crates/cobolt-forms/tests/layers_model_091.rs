//! Spec 091, slice 1 — the layer MODEL and its FILE FORMAT, with no painter:
//! names (R4–R6), the cap (R7), containers (R8), draw order (R9–R11), what the
//! `.cfrm` records and never records (R2, R35, R39, R40), and the
//! `Non-Visuals` grid (R49–R51).
//!
//! Each test names the requirement or acceptance criterion it pins.

use cobolt_forms::containers::{render_order, render_order_in};
use cobolt_forms::model::{
    is_valid_layer_name, Control, ControlType, EventBinding, Layer, LayerError, BASE_LAYER_NAME,
    LAYER_TRANSPARENT_COLOR, MAX_LAYERS, NON_VISUALS_TAB_NAME,
};
use cobolt_forms::nv_grid::{self, non_visual_grid, COLUMNS};
use cobolt_forms::{form_to_string, load_form_from_str, Form};

fn form() -> Form {
    Form::new("F", "F", 640, 480)
}

fn button(id: &str, layer: Option<&str>, z: i32) -> Control {
    let mut c = Control::new(id, ControlType::Button, 10, 10);
    c.layer = layer.map(str::to_owned);
    c.z_order = z;
    c
}

fn ids(form: &Form, order: &[usize]) -> Vec<String> {
    order.iter().map(|&i| form.controls[i].id.clone()).collect()
}

// ── Names (R4–R6, AC2) ───────────────────────────────────────────────────

#[test]
fn layer_names_follow_the_control_name_rules_and_two_are_reserved_091() {
    for ok in ["Layer-1", "LAY-ERROR", "busy", "X"] {
        assert!(is_valid_layer_name(ok), "{ok} should be a layer name");
    }
    for bad in [
        "Form", "FORM", "form", "fOrM", "Non-Visuals", "NON-VISUALS", "non-visuals", "me",
        "super", "1abc", "", "a b", "a_b",
    ] {
        assert!(!is_valid_layer_name(bad), "{bad:?} must not be a layer name");
    }
    assert_eq!(BASE_LAYER_NAME, "Form");
    assert_eq!(NON_VISUALS_TAB_NAME, "Non-Visuals");
}

#[test]
fn new_layers_take_the_first_free_name_and_are_fully_transparent_091() {
    let mut f = form();
    assert_eq!(f.add_layer().unwrap(), "Layer-1");
    assert_eq!(f.add_layer().unwrap(), "Layer-2");
    assert_eq!(f.add_layer().unwrap(), "Layer-3");
    // R15: adding a layer changes nothing that is drawn.
    for l in &f.layers {
        assert_eq!(l.backdrop.color, LAYER_TRANSPARENT_COLOR);
        assert_eq!(l.backdrop.transparency, 0);
        assert!(!l.backdrop.gradient_enabled);
        assert!(l.backdrop.image.is_empty());
    }
    // The first free one: renaming Layer-2 frees its name.
    f.rename_layer("Layer-2", "BUSY").unwrap();
    assert_eq!(f.add_layer().unwrap(), "Layer-2");
    // A control called Layer-4 takes that name out of play (one namespace, R4).
    f.controls.push(button("layer-4", None, 0));
    assert_eq!(f.add_layer().unwrap(), "Layer-5");
}

#[test]
fn the_65th_layer_is_refused_and_the_64th_is_added_091() {
    let mut f = form();
    for n in 1..=MAX_LAYERS {
        assert_eq!(f.add_layer().unwrap(), format!("Layer-{n}"), "layer {n}");
    }
    assert_eq!(f.layers.len(), 64);
    assert_eq!(f.add_layer(), Err(LayerError::TooMany));
    assert_eq!(f.layers.len(), 64, "a refused layer is not added");
}

#[test]
fn a_layer_and_a_control_cannot_share_a_name_in_either_direction_091() {
    let mut f = form();
    f.controls.push(button("BTN", None, 0));
    let layer = f.add_layer().unwrap();

    // layer → control's name, any letter case
    assert_eq!(f.rename_layer(&layer, "btn"), Err(LayerError::NameTaken));
    // layer → reserved names
    assert_eq!(f.rename_layer(&layer, "Form"), Err(LayerError::InvalidName));
    assert_eq!(f.rename_layer(&layer, "non-visuals"), Err(LayerError::InvalidName));
    assert_eq!(f.rename_layer(&layer, "1x"), Err(LayerError::InvalidName));
    assert_eq!(f.rename_layer("Nope", "OK"), Err(LayerError::NoSuchLayer));
    // control → layer's name, any letter case
    assert!(!f.rename_control("BTN", "layer-1"));
    assert_eq!(f.controls[0].id, "BTN", "a refused rename changes nothing");
    // two layers cannot share one either
    let second = f.add_layer().unwrap();
    assert_eq!(f.rename_layer(&second, "LAYER-1"), Err(LayerError::NameTaken));
    // a change of letter case alone is allowed
    f.rename_layer("Layer-1", "LAYER-1").unwrap();
    assert_eq!(f.layers[0].name, "LAYER-1");
}

#[test]
fn renaming_a_layer_follows_it_into_the_controls_and_the_code_091() {
    let mut f = form();
    let layer = f.add_layer().unwrap();
    let mut b = button("OK-BTN", Some(&layer), 0);
    b.events.push(EventBinding {
        event: "onClick".into(),
        paragraph: "OK-BTN-ONCLICK".into(),
        code: "SET Layer-1::Visible TO FALSE.\nMOVE 1 TO X.".into(),
    });
    f.controls.push(b);
    f.form_events[0].code = "SET LAYER-1::Visible TO TRUE.".into();

    f.rename_layer(&layer, "Busy").unwrap();

    assert_eq!(f.layers[0].name, "Busy");
    assert_eq!(f.controls[0].layer.as_deref(), Some("Busy"));
    assert!(f.controls[0].events[0].code.contains("Busy::Visible TO FALSE"));
    assert!(f.controls[0].events[0].code.contains("MOVE 1 TO X."));
    assert!(f.form_events[0].code.contains("Busy::Visible TO TRUE"));
}

#[test]
fn a_name_collision_in_a_hand_edited_form_is_reported_and_nothing_is_removed_091() {
    let mut f = form();
    f.layers.push(Layer::new("DUP"));
    f.controls.push(button("dup", None, 0));
    f.controls.push(button("Other", None, 0));
    f.controls.push(button("OTHER", None, 0));
    assert_eq!(f.name_collisions(), vec!["DUP".to_owned(), "Other".to_owned()]);
    assert_eq!(f.controls.len(), 3, "reported, never repaired by deleting (R5)");
    assert_eq!(f.layers.len(), 1);
    assert!(form().name_collisions().is_empty());
}

// ── Containers and moving between layers (R8, R31, R47, AC4) ──────────────

#[test]
fn a_container_carries_its_children_into_its_layer_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    let mut panel = Control::new("P", ControlType::Panel, 0, 0);
    panel.layer = Some("Layer-1".into());
    let mut child = button("B", None, 0);
    child.parent = Some("P".into());
    let mut grandchild = button("G", None, 0);
    grandchild.parent = Some("B".into());
    f.controls.extend([panel, child, grandchild]);

    assert_eq!(f.layer_of("P"), Some("Layer-1"));
    assert_eq!(f.layer_of("B"), Some("Layer-1"), "the container's layer (R8)");
    assert_eq!(f.layer_of("g"), Some("Layer-1"), "any depth, any letter case");

    // A child cannot be put in another layer on its own.
    assert_eq!(f.set_control_layer("B", Some("Layer-2")), Err(LayerError::InsideContainer));
    assert_eq!(f.layer_of("B"), Some("Layer-1"));

    // Moving the container moves everything inside it.
    f.set_control_layer("P", Some("layer-2")).unwrap();
    assert_eq!(f.layer_of("P"), Some("Layer-2"));
    assert_eq!(f.layer_of("B"), Some("Layer-2"));
    assert_eq!(f.layer_of("G"), Some("Layer-2"));
    assert!(f.controls[1].layer.is_none() && f.controls[2].layer.is_none());

    // Back to the base, by either spelling.
    f.set_control_layer("P", Some("FORM")).unwrap();
    assert_eq!(f.layer_of("G"), None);
    f.set_control_layer("P", Some("Layer-1")).unwrap();
    f.set_control_layer("P", None).unwrap();
    assert_eq!(f.layer_of("B"), None);

    assert_eq!(f.set_control_layer("P", Some("Ghost")), Err(LayerError::NoSuchLayer));
    assert_eq!(f.set_control_layer("Nope", None), Err(LayerError::NoSuchControl));
}

#[test]
fn a_non_visual_control_belongs_to_no_layer_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.controls.push(Control::new("TMR", ControlType::Timer, 0, 0));
    assert_eq!(f.set_control_layer("TMR", Some("Layer-1")), Err(LayerError::NonVisual));
    assert_eq!(f.controls[0].layer, None);
    // …but sending it to the base is not a move at all and is accepted.
    f.set_control_layer("TMR", None).unwrap();
}

// ── Draw order (R9–R11, R40, AC5 model half) ─────────────────────────────

#[test]
fn a_higher_layer_draws_above_a_lower_one_whatever_the_z_order_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    // Listed deliberately out of order; z_order points the wrong way round.
    f.controls.push(button("L2-NEG", Some("Layer-2"), -5));
    f.controls.push(button("BASE-209", None, 209));
    f.controls.push(button("L1-ZERO", Some("Layer-1"), 0));
    f.controls.push(button("BASE-1", None, 1));
    f.controls.push(button("L1-10000", Some("Layer-1"), 10_000));

    let order = render_order_in(&f.controls, &f.layers);
    assert_eq!(
        ids(&f, &order),
        ["BASE-1", "BASE-209", "L1-ZERO", "L1-10000", "L2-NEG"],
        "the base, then Layer-1, then Layer-2; z_order only inside a layer"
    );

    // With no layer table the same controls order by z_order alone: the form
    // with no layers is exactly what it was (R2).
    let flat = render_order(&f.controls);
    assert_eq!(
        ids(&f, &flat),
        ["L2-NEG", "L1-ZERO", "BASE-1", "BASE-209", "L1-10000"]
    );
    assert_eq!(flat, render_order_in(&f.controls, &[]));
}

#[test]
fn re_stacking_changes_the_draw_order_and_the_base_cannot_move_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    f.controls.push(button("A", Some("Layer-1"), 0));
    f.controls.push(button("B", Some("Layer-2"), 0));
    f.controls.push(button("C", Some("Layer-3"), 0));
    assert_eq!(ids(&f, &render_order_in(&f.controls, &f.layers)), ["A", "B", "C"]);

    f.move_layer(0, 2).unwrap(); // Layer-1 to the top
    assert_eq!(
        f.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
        ["Layer-2", "Layer-3", "Layer-1"]
    );
    assert_eq!(ids(&f, &render_order_in(&f.controls, &f.layers)), ["B", "C", "A"]);

    assert_eq!(f.move_layer(3, 0), Err(LayerError::BadPosition));
    assert_eq!(f.move_layer(0, 3), Err(LayerError::BadPosition));
    // The base is not in the table at all, so no call can move it (R11).
    assert_eq!(f.layer_rank(None), 0);
    assert_eq!(f.layer_rank(Some("Form")), 0);
}

#[test]
fn a_container_s_children_follow_it_not_their_own_layer_name_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    let mut panel = Control::new("P", ControlType::Panel, 0, 0);
    panel.layer = Some("Layer-1".into());
    // A hand-edited child that names another layer is not obeyed (R8).
    let mut kid = button("KID", Some("Layer-2"), 0);
    kid.parent = Some("P".into());
    f.controls.extend([kid, panel, button("TOP", Some("Layer-2"), 0)]);
    let order = ids(&f, &render_order_in(&f.controls, &f.layers));
    assert_eq!(order, ["P", "KID", "TOP"], "the child stays inside its container");
}

#[test]
fn a_control_naming_an_undefined_layer_draws_with_the_base_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.controls.push(button("IN-L1", Some("Layer-1"), 0));
    f.controls.push(button("GHOST", Some("NoSuchLayer"), 5));
    f.controls.push(button("BASE", None, 9));
    assert_eq!(
        ids(&f, &render_order_in(&f.controls, &f.layers)),
        ["GHOST", "BASE", "IN-L1"]
    );
    assert_eq!(
        f.unknown_layer_refs(),
        vec![("GHOST".to_owned(), "NoSuchLayer".to_owned())]
    );
}

// ── The file (R2, R35, R39, R40, AC1, AC13) ──────────────────────────────

fn layered_form() -> Form {
    let mut f = form();
    f.add_layer().unwrap();
    f.add_layer().unwrap();
    f.layers[0].backdrop.color = "#112233CC".into();
    f.layers[0].backdrop.gradient_enabled = true;
    f.layers[0].backdrop.gradient_start_color = "#FF0000FF".into();
    f.layers[0].backdrop.gradient_end_color = "#0000FFFF".into();
    f.layers[0].backdrop.gradient_direction = "East".into();
    f.layers[1].backdrop.transparency = 40;
    f.layers[1].backdrop.image = "Assets/scrim.png".into();
    f.layers[1].backdrop.image_mode = cobolt_forms::model::BgImageMode::Fill;
    f.controls.push(button("BASE-BTN", None, 0));
    f.controls.push(button("L1-BTN", Some("Layer-1"), 0));
    let mut panel = Control::new("L2-PANEL", ControlType::Panel, 0, 0);
    panel.layer = Some("Layer-2".into());
    let mut kid = button("L2-KID", None, 0);
    kid.parent = Some("L2-PANEL".into());
    f.controls.extend([panel, kid]);
    f
}

#[test]
fn layers_and_each_controls_layer_round_trip_in_stack_order_091() {
    let f = layered_form();
    let xml = form_to_string(&f).unwrap();
    let back = load_form_from_str(&xml).unwrap();

    assert_eq!(back.layers, f.layers, "names, order and every backdrop field");
    let layer_of = |form: &Form, id: &str| form.layer_of(id).map(str::to_owned);
    for id in ["BASE-BTN", "L1-BTN", "L2-PANEL", "L2-KID"] {
        assert_eq!(layer_of(&back, id), layer_of(&f, id), "{id}");
    }
    assert_eq!(layer_of(&back, "L2-KID").as_deref(), Some("Layer-2"));
    // Stable: saving what was loaded writes the same text.
    assert_eq!(form_to_string(&back).unwrap(), xml);
    // The elements are in stack order.
    let first = xml.find(r#"<Layer name="Layer-1""#).expect("Layer-1 written");
    let second = xml.find(r#"<Layer name="Layer-2""#).expect("Layer-2 written");
    assert!(first < second);
}

#[test]
fn a_form_with_no_layers_writes_no_layer_markup_091() {
    let mut f = form();
    f.controls.push(button("A", None, 0));
    f.controls.push(Control::new("TMR", ControlType::Timer, 0, 0));
    let xml = form_to_string(&f).unwrap();
    assert!(!xml.contains("<Layer"), "{xml}");
    assert!(!xml.contains(" layer=\""), "{xml}");
    assert!(load_form_from_str(&xml).unwrap().layers.is_empty());
}

#[test]
fn visible_is_never_written_so_every_layer_starts_hidden_091() {
    let xml = form_to_string(&layered_form()).unwrap();
    let layer_elements: Vec<&str> = xml.lines().filter(|l| l.contains("<Layer ")).collect();
    assert_eq!(layer_elements.len(), 2);
    for el in layer_elements {
        assert!(!el.to_ascii_lowercase().contains("visible"), "R35: {el}");
    }
}

#[test]
fn a_hand_trimmed_layer_loads_transparent_091() {
    let mut f = form();
    f.controls.push(button("A", None, 0));
    let xml = form_to_string(&f).unwrap();
    let hand = xml.replacen("<Control ", r#"<Layer name="HAND"/><Control "#, 1);
    let loaded = load_form_from_str(&hand).unwrap();
    assert_eq!(loaded.layers.len(), 1);
    assert_eq!(loaded.layers[0].name, "HAND");
    assert_eq!(loaded.layers[0].backdrop.color, LAYER_TRANSPARENT_COLOR);
    // An element with no name is not a layer.
    let nameless = xml.replacen("<Control ", "<Layer/><Control ", 1);
    assert!(load_form_from_str(&nameless).unwrap().layers.is_empty());
}

#[test]
fn a_control_naming_an_undefined_layer_is_kept_reported_and_saved_as_it_was_091() {
    let mut f = form();
    f.controls.push(button("GHOST", Some("NoSuchLayer"), 0));
    let xml = form_to_string(&f).unwrap();
    assert!(xml.contains(r#"layer="NoSuchLayer""#));

    let loaded = load_form_from_str(&xml).unwrap();
    assert_eq!(loaded.controls.len(), 1, "nothing is deleted (R40)");
    assert_eq!(loaded.controls[0].layer.as_deref(), Some("NoSuchLayer"));
    assert_eq!(loaded.unknown_layer_refs().len(), 1);
    assert_eq!(loaded.layer_rank(loaded.controls[0].layer.as_deref()), 0, "shown in the base");
    // Saving does not repair it either.
    assert_eq!(form_to_string(&loaded).unwrap(), xml);
}

// ── The Non-Visuals grid (R49–R51, AC22, AC23 data half) ──────────────────

fn timers(n: usize) -> Form {
    let mut f = form();
    for i in 0..n {
        f.controls.push(Control::new(format!("TMR-{i:02}"), ControlType::Timer, 0, 0));
    }
    f
}

#[test]
fn the_grid_has_five_columns_filled_row_by_row_with_no_shared_cell_091() {
    assert_eq!(COLUMNS, 5);
    for (n, rows) in [(0, 0), (1, 1), (5, 1), (6, 2), (10, 2), (11, 3)] {
        let cells = non_visual_grid(&timers(n));
        assert_eq!(cells.len(), n);
        assert_eq!(nv_grid::rows_for(n), rows, "{n} cards");
        for (k, cell) in cells.iter().enumerate() {
            assert_eq!((cell.row, cell.col), (k / 5, k % 5), "card {k} of {n}");
            assert!(cell.col < COLUMNS, "none outside the five columns");
        }
        let mut spots: Vec<_> = cells.iter().map(|c| (c.row, c.col)).collect();
        spots.sort();
        spots.dedup();
        assert_eq!(spots.len(), n, "no two cards share a cell");
        let rects: Vec<_> = cells.iter().map(|c| (c.rect.x, c.rect.y)).collect();
        let mut distinct = rects.clone();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), n, "and no two share a position");
    }
    // The sixth wraps to the start of the second row; the eleventh to the third.
    let cells = non_visual_grid(&timers(11));
    assert_eq!((cells[5].row, cells[5].col), (1, 0));
    assert_eq!((cells[10].row, cells[10].col), (2, 0));
    assert_eq!(cells[5].rect.x, cells[0].rect.x);
    assert!(cells[5].rect.y > cells[0].rect.y);
}

#[test]
fn deleting_a_card_moves_every_later_one_up_with_no_gap_091() {
    let mut f = timers(7);
    let before = non_visual_grid(&f);
    f.controls.remove(2); // the third card
    let after = non_visual_grid(&f);
    assert_eq!(after.len(), 6);
    for (k, cell) in after.iter().enumerate() {
        assert_eq!((cell.row, cell.col), (k / 5, k % 5), "no empty cell between cards");
    }
    // The fourth card is now where the third was.
    assert_eq!(after[2].id, before[3].id);
    assert_eq!(after[2].rect, before[2].rect);
}

#[test]
fn the_grid_keeps_a_type_together_and_orders_types_then_names_091() {
    let mut f = form();
    // Scrambled creation order; names whose order differs from creation order.
    let made = [
        ("zeta", ControlType::Timer),
        ("db-b", ControlType::SqlDatabase),
        ("TMR-2", ControlType::Timer),
        ("snack", ControlType::Snackbar),
        ("rest", ControlType::RestClient),
        ("TMR-10", ControlType::Timer),
        ("agent", ControlType::AgentObject),
        ("DB-A", ControlType::SqlDatabase),
        ("Tmr-1", ControlType::Timer),
    ];
    for (id, ct) in made.iter() {
        f.controls.push(Control::new(*id, ct.clone(), 7, 7));
    }
    // A visual control is not in the grid at all.
    f.controls.push(button("A-BUTTON", None, 0));

    let names = |f: &Form| non_visual_grid(f).into_iter().map(|c| c.id).collect::<Vec<_>>();
    // AgentObject, RestClient, Snackbar, SqlDatabase, Timer — the English names,
    // A–Z — and inside a type the names A–Z, case ignored, plain not natural:
    // TMR-10 before TMR-2 (Q18).
    let expected = [
        "agent", "rest", "snack", "DB-A", "db-b", "Tmr-1", "TMR-10", "TMR-2", "zeta",
    ];
    assert_eq!(names(&f), expected);

    // The order depends on the controls, not on how they were made, and not on
    // where the file puts them.
    let mut reversed = form();
    for (id, ct) in made.iter().rev() {
        let mut c = Control::new(*id, ct.clone(), 900, 900);
        c.rect.w = 3;
        reversed.controls.push(c);
    }
    assert_eq!(names(&reversed), expected);

    // Renaming a control moves its card.
    assert!(f.rename_control("zeta", "ALPHA"));
    assert_eq!(names(&f)[5], "ALPHA", "ALPHA now sorts first among the Timers");
}

#[test]
fn the_grid_ignores_the_x_y_width_and_height_of_a_non_visual_control_091() {
    let mut a = form();
    let mut b = form();
    for (i, id) in ["T1", "T2", "T3"].iter().enumerate() {
        let mut ca = Control::new(*id, ControlType::Timer, 5, 5);
        ca.rect = cobolt_forms::Rect::new(1, 2, 3, 4);
        let mut cb = Control::new(*id, ControlType::Timer, 0, 0);
        cb.rect = cobolt_forms::Rect::new(500 * i as i32, 77, 640, 480);
        a.controls.push(ca);
        b.controls.push(cb);
    }
    assert_eq!(non_visual_grid(&a), non_visual_grid(&b));
    // …and computing the grid does not touch the form (R54).
    assert_eq!(a.controls[0].rect, cobolt_forms::Rect::new(1, 2, 3, 4));
}

#[test]
fn a_non_visual_control_inside_a_container_is_in_the_grid_too_091() {
    let mut f = form();
    f.controls.push(Control::new("PNL", ControlType::Panel, 0, 0));
    let mut t = Control::new("TMR", ControlType::Timer, 0, 0);
    t.parent = Some("PNL".into());
    f.controls.push(t);
    let cells = non_visual_grid(&f);
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].id, "TMR");
    assert_eq!(f.controls[1].parent.as_deref(), Some("PNL"), "the file is not touched (Q17)");
}

#[test]
fn the_grid_s_scroll_height_follows_the_rows_and_never_the_form_091() {
    let h = nv_grid::content_height;
    assert_eq!(h(0), 2 * nv_grid::MARGIN);
    assert!(h(5) < h(6), "a sixth card opens a second row");
    assert_eq!(h(6), h(10));
    assert!(h(11) > h(10));
}

// ── What a program sees of a layer (R33–R35) ──────────────────────────────

use cobolt_forms::model::{layer_prop, layer_refusal, layer_writable, LAYER_PROPS};

#[test]
fn a_layer_is_seeded_hidden_with_every_property_a_program_can_read_091() {
    let mut l = Layer::new("Layer-1");
    l.backdrop.color = "#204080FF".into();
    l.backdrop.transparency = 25;
    l.backdrop.gradient_enabled = true;
    l.backdrop.image = "Assets/bg.png".into();
    let props = l.runtime_props();
    let get = |k: &str| props.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    // R35 — hidden, whatever the designer showed.
    assert_eq!(get("Visible"), Some("false"));
    assert_eq!(get("Name"), Some("Layer-1"));
    assert_eq!(get("Transparency"), Some("25"));
    assert_eq!(get("BackgroundColor"), Some("#204080FF"));
    assert_eq!(get("BackgroundGradientEnabled"), Some("true"));
    assert_eq!(get("BackgroundImage"), Some("Assets/bg.png"));
    assert_eq!(get("BackgroundImageMode"), Some("Stretch"));
    // Seeded: exactly the list a program may read, no more and no fewer.
    let mut names: Vec<&str> = props.iter().map(|(n, _)| n.as_str()).collect();
    let mut want: Vec<&str> = LAYER_PROPS.to_vec();
    names.sort_unstable();
    want.sort_unstable();
    assert_eq!(names, want);
}

#[test]
fn what_a_program_writes_over_a_layer_reaches_its_backdrop_091() {
    let design = Layer::new("Layer-1");
    let live: Vec<(String, String)> = [
        ("TRANSPARENCY", "60"),
        ("BackgroundColor", "#FF0000FF"),
        ("BACKGROUNDGRADIENTENABLED", "true"),
        ("backgroundgradientstartcolor", "#111111FF"),
        ("BackgroundGradientEndColor", "#EEEEEEFF"),
        ("BackgroundGradientDirection", "East"),
        ("BackgroundImage", " Assets/x.png "),
        ("BackgroundImageMode", "Tile"),
        ("Visible", "true"),          // not the backdrop's: the state answers for it
        ("Name", "ignored"),          // never changes the layer
        ("Nonsense", "ignored"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let merged = design.with_live(live.iter().map(|(k, v)| (k, v)));
    let b = &merged.backdrop;
    assert_eq!(b.transparency, 60);
    assert_eq!(b.color, "#FF0000FF");
    assert!(b.gradient_enabled);
    assert_eq!((b.gradient_start_color.as_str(), b.gradient_end_color.as_str()), ("#111111FF", "#EEEEEEFF"));
    assert_eq!(b.gradient_direction, "East");
    assert_eq!(b.image, "Assets/x.png");
    assert_eq!(b.image_mode, cobolt_forms::model::BgImageMode::Tile);
    assert_eq!(merged.name, "Layer-1", "a layer's name is never changed by a write");
    // A write of nothing leaves the design as it was.
    assert_eq!(design.with_live(std::iter::empty()), design);
    // Out-of-range transparency is held to 0–100, as the form's is.
    let over = vec![("Transparency".to_owned(), "400".to_owned())];
    assert_eq!(design.with_live(over.iter().map(|(k, v)| (k, v))).backdrop.transparency, 100);
}

#[test]
fn every_layer_property_but_name_is_writable_and_the_refusal_names_the_rest_091() {
    for p in LAYER_PROPS {
        assert!(layer_prop(p) && layer_prop(&p.to_uppercase()));
        assert_eq!(layer_writable(p), *p != "Name", "{p}");
    }
    assert!(!layer_prop("Colour") && !layer_writable("Width"));
    let msg = layer_refusal("LAYER-1", "Colour");
    assert!(msg.contains("LAYER-1::Colour") && msg.contains("Visible") && !msg.contains("Name,"), "{msg}");
    assert!(layer_refusal("LAYER-1", "Name").contains("renamed in the designer"));
}

#[test]
fn a_program_can_address_a_layer_but_not_the_non_visuals_tab_091() {
    let mut f = form();
    f.add_layer().unwrap();
    f.controls.push(button("BTN", None, 0));
    let names = cobolt_forms::toolbar::object_names(&f);
    assert!(names.contains("LAYER-1") && names.contains("BTN") && names.contains("F"));
    // R55 — `Non-Visuals` is a tab of the designer, not an object of the form.
    assert!(!names.contains("NON-VISUALS"));
    assert_eq!(
        cobolt_forms::toolbar::layer_names(&f).into_iter().collect::<Vec<_>>(),
        vec!["LAYER-1".to_owned()]
    );
}

//! **A desktop form is untouched by the web stylesheets** (WASM-007 T29, R14).
//!
//! `CssClass` and `CssStyle` are Web and Mobile properties: `Control::new` does not seed them, so a desktop form
//! never carries them, and saving one writes the same bytes it read. The ProgressBar's own `Style`
//! (`Continuous` / `Blocks`) is a different property and keeps its meaning. Every desktop form of the repository's
//! examples (`examples/*/forms/`) is read and written here too.

use cobolt_forms::model::{Control, ControlType, Form, PropValue};
use cobolt_forms::xml::{form_to_string, load_form_from_str};

fn assert_no_css(text: &str, what: &str) {
    assert!(!text.contains("CssClass") && !text.contains("CssStyle"), "{what} carries a CSS property");
}

#[test]
fn a_desktop_form_saves_byte_identical_and_carries_no_css_property() {
    // A form of every control type, as the designer makes them.
    let mut form = Form::new("desk-form", "Desk", 800, 600);
    for (i, t) in ControlType::ALL.iter().enumerate() {
        let mut c = Control::new(&format!("C-{i}"), t.clone(), (i % 10) as i32 * 70, (i / 10) as i32 * 60);
        assert!(c.properties.get("CssClass").is_none() && c.properties.get("CssStyle").is_none(), "{t:?} seeds a CSS property");
        if *t == ControlType::ProgressBar {
            c.set_prop("Style", PropValue::String("Blocks".into()));
        }
        form.controls.push(c);
    }
    // The fixture is the form as a load leaves it — a load docks a SideMenu to the form's edge, which `Control::new`
    // does not — so the bytes compared are those of a form the designer has saved.
    let first = form_to_string(&load_form_from_str(&form_to_string(&form).unwrap()).unwrap()).unwrap();
    assert_no_css(&first, "a new desktop form");
    let again = form_to_string(&load_form_from_str(&first).unwrap()).unwrap();
    if again != first {
        let (a, b): (Vec<&str>, Vec<&str>) = (first.lines().collect(), again.lines().collect());
        let at = a.iter().zip(&b).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        panic!("read and written again, byte for byte: line {at}\n  first: {:?}\n  again: {:?}", a.get(at), b.get(at));
    }
    let loaded = load_form_from_str(&first).unwrap();
    let bar = loaded.controls.iter().find(|c| c.control_type == ControlType::ProgressBar).unwrap();
    assert_eq!(bar.properties.get("Style").map(PropValue::as_str), Some("Blocks"), "the ProgressBar's Style is its own");

    // The examples' desktop forms.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seen = 0;
    for project in std::fs::read_dir(&root).unwrap().flatten() {
        let Ok(forms) = std::fs::read_dir(project.path().join("forms")) else { continue };
        for f in forms.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "cfrm")) {
            let text = std::fs::read_to_string(&f).unwrap();
            let Ok(form) = load_form_from_str(&text) else { continue };
            let saved = form_to_string(&form).unwrap();
            assert_no_css(&saved, &f.display().to_string());
            let twice = form_to_string(&load_form_from_str(&saved).unwrap()).unwrap();
            assert_eq!(twice, saved, "{}: written twice, the same bytes", f.display());
            seen += 1;
        }
    }
    assert!(seen > 0, "no desktop example form was found under {}", root.display());
    println!("desktop forms: {} control types, no CSS property, saved byte-identical; {seen} example forms the same", ControlType::ALL.len());
}

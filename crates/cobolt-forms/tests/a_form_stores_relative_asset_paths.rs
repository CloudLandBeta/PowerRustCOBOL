#![cfg(feature = "render")]
//! **A form must carry its images as project-relative paths.**
//!
//! The designer's picker stored the full path, so every form saved with an
//! image carried `/Users/<someone>/Documents/<project>/…`. That is broken on
//! every other machine, and on the author's own as soon as the project moves —
//! which is exactly what happened to the shipped demo project (operator,
//! 2026-09-04).

use std::path::Path;

/// One test, three phases. The anchor is deliberately process-global — an
/// application opens one project — so separate `#[test]` functions in the same
/// binary would race each other setting it.
#[test]
fn a_form_stores_and_resolves_its_assets_relative_to_the_project() {
    // ── Absolute paths INSIDE the project are healed on load. ──────────────
    {
    let root = std::env::temp_dir().join("prc-relativize-test");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("assets/logo.png"), b"x").unwrap();
    cobolt_forms::assets::set_base(&root);

    let abs = root.join("assets/logo.png");
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="400" height="300" background-image="{abs}">
  <Control id="Pic-1" type="PictureBox" x="10" y="10" w="100" h="100">
    <Property name="ImagePath">{abs}</Property>
  </Control>
</Form>"#,
        abs = abs.display()
    );

    let form = cobolt_forms::load_form_from_str(&xml).expect("the form loads");
    assert_eq!(
        form.background_image, "assets/logo.png",
        "the form's background image must be stored relative"
    );
    let pic = form
        .controls
        .iter()
        .find(|c| c.id == "Pic-1")
        .expect("the PictureBox survives the load");
    assert_eq!(
        pic.get_prop("ImagePath").map(|v| v.as_str()),
        Some("assets/logo.png"),
        "a control's image path must be stored relative too"
    );
    std::fs::remove_dir_all(&root).ok();
    }

    // ── A path the developer deliberately pointed OUTSIDE the project stays
    //    absolute: relativizing it would silently repoint it. ───────────────
    {
    let root = std::env::temp_dir().join("prc-relativize-outside");
    std::fs::create_dir_all(&root).unwrap();
    cobolt_forms::assets::set_base(&root);

    let outside = "/opt/shared-branding/logo.png";
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="F" title="F" width="400" height="300" background-image="{outside}">
</Form>"#
    );
    let form = cobolt_forms::load_form_from_str(&xml).expect("the form loads");
    assert_eq!(form.background_image, outside);
    std::fs::remove_dir_all(&root).ok();
    }

    // ── The anchor resolves what the form stores — the other half. ─────────
    {
    let root = std::env::temp_dir().join("prc-resolve-test");
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let file = root.join("assets/logo.png");
    std::fs::write(&file, b"x").unwrap();

    cobolt_forms::assets::set_base(&root);
    assert_eq!(cobolt_forms::assets::resolve("assets/logo.png"), file);

    // …and the round trip: store then resolve returns the same file.
    let stored = cobolt_forms::assets::store(Path::new(&root), &file);
    assert_eq!(stored, "assets/logo.png");
    assert_eq!(cobolt_forms::assets::resolve(&stored), file);
    std::fs::remove_dir_all(&root).ok();
    }
}

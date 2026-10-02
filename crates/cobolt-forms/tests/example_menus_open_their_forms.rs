//! Every menu item of the example projects can open the form it names.
//!
//! A SideMenu or MenuBar item that loads a form into the content pane needs
//! that form to be `Embedded` or `Both`; one that opens it standalone needs
//! `Standalone` or `Both` (049 R17 / 051 R26). The build checks this
//! (`cobolt_compiler::build_core`, through `validate_menu_targets`), but no test
//! ran it over the shipped examples, so twelve PowerDemo3 demos were added to
//! the side menu as `Standalone` and the IDE refused the project (1.80.75).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cobolt_forms::menu::{load_menu, menu_yaml_path, validate_menu_targets};
use cobolt_forms::model::FormFormat;
use cobolt_forms::{ControlType, load_form};

fn forms_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            forms_in(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("cfrm") {
            out.push(p);
        }
    }
}

#[test]
fn every_example_menu_item_opens_a_form_of_the_right_format() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (mut menus, mut items) = (0, 0);
    let mut problems = Vec::new();
    for project in ["PowerDemo3", "PowerChat"] {
        let mut files = Vec::new();
        forms_in(&repo.join(project).join("forms"), &mut files);
        let forms: Vec<(PathBuf, cobolt_forms::Form)> =
            files.into_iter().filter_map(|p| load_form(&p).ok().map(|f| (p, f))).collect();
        // A menu names a form by its file stem.
        let formats: HashMap<String, FormFormat> = forms
            .iter()
            .filter_map(|(p, f)| Some((p.file_stem()?.to_str()?.to_ascii_uppercase(), f.form_format.clone())))
            .collect();
        let lookup = |name: &str| formats.get(&name.trim().to_ascii_uppercase()).cloned();
        for (path, form) in &forms {
            for ctrl in &form.controls {
                if !matches!(ctrl.control_type, ControlType::SideMenu | ControlType::MenuBar) {
                    continue;
                }
                let yaml = menu_yaml_path(path.parent().unwrap(), &ctrl.id);
                let Ok(def) = load_menu(&yaml) else { continue };
                menus += 1;
                fn count(items: &[cobolt_forms::menu::MenuItem]) -> usize {
                    items.iter().map(|i| 1 + count(&i.items)).sum()
                }
                items += count(&def.menu);
                for v in validate_menu_targets(&def, &lookup) {
                    problems.push(format!("{project}/{}: item '{}' -> form '{}' ({:?})", ctrl.id, v.item_id, v.form, v.kind));
                }
            }
        }
    }
    println!("example menus: {menus} menus, {items} items checked, {} problem(s)", problems.len());
    assert!(menus > 0, "no example menu was found - the test is not looking where the menus are");
    assert!(problems.is_empty(), "menu items whose form cannot be opened that way:\n{}", problems.join("\n"));
}

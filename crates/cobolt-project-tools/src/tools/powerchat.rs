// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **`add_powerchat`** (spec 085 R6–R13): add PowerChat to the open project
//! in one call — its forms, menu, pictures and defaults in folders of their
//! own, wearing the host's look and name, registered, generated and checked,
//! and an Assistant item in the host's side menu that opens it as a window of
//! its own (modeless).

use std::path::Path;

use serde_json::{json, Value};

use crate::host::ProjectHost;
use crate::powerchat::{self, Branding};
use crate::root::ProjectRoot;

/// The host's main form: its project-relative path and the form.
fn main_form(root: &ProjectRoot, forms: &[String]) -> Option<(String, cobolt_forms::Form)> {
    forms.iter().find_map(|rel| {
        let form = cobolt_forms::load_form(&root.resolve(rel).ok()?).ok()?;
        form.main_form.then(|| (rel.clone(), form))
    })
}

/// The project's icon (`[ide] project_icon`), when it names a file that exists.
fn project_icon(root: &ProjectRoot) -> Option<String> {
    let doc: toml_edit::DocumentMut = std::fs::read_to_string(root.manifest()).ok()?.parse().ok()?;
    let icon = doc.get("ide")?.get("project_icon")?.as_str()?.trim().to_owned();
    (!icon.is_empty() && root.dir().join(&icon).is_file()).then_some(icon)
}

fn write(abs: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = abs.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    std::fs::write(abs, bytes).map_err(|e| format!("cannot write {}: {e}", abs.display()))
}

pub fn run(host: &mut impl ProjectHost, root: &ProjectRoot, menu_label: Option<&str>) -> Result<Value, String> {
    let view = cobolt_compiler::project_manifest_view(root.manifest())?;
    // Refuse before writing anything: PowerChat already here, or a form of
    // the project's that one of PowerChat's would collide with.
    if root.dir().join(powerchat::FORMS_DIR).exists() {
        return Err(format!("{} already exists: PowerChat has been added to this project", powerchat::FORMS_DIR));
    }
    let mut collisions = Vec::new();
    for rel in &view.forms {
        let Ok(abs) = root.resolve(rel) else { continue };
        let stem = abs.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_ascii_lowercase();
        let name = cobolt_forms::load_form(&abs).map(|f| f.name.to_ascii_uppercase()).unwrap_or_default();
        for (pc_stem, _) in powerchat::FORMS {
            if stem == *pc_stem || name == pc_stem.to_ascii_uppercase() {
                collisions.push(rel.clone());
            }
        }
    }
    if !collisions.is_empty() {
        return Err(format!(
            "these forms of the project have the names PowerChat's forms use, so it cannot be added: {}",
            collisions.join(", ")
        ));
    }
    let main = main_form(root, &view.forms);
    let brand = Branding {
        name: main
            .as_ref()
            .map(|(_, f)| f.title.trim().to_owned())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| view.name.clone()),
        icon: project_icon(root),
    };

    // The files, the host's.
    let mut forms = Vec::new();
    for (stem, xml) in powerchat::FORMS {
        let rel = format!("{}/{stem}.cfrm", powerchat::FORMS_DIR);
        write(&root.dir().join(&rel), powerchat::adapt_form(xml, &brand).as_bytes())?;
        forms.push(rel);
    }
    write(&root.dir().join(powerchat::FORMS_DIR).join(powerchat::MENU.0), powerchat::MENU.1.as_bytes())?;
    let mut assets = Vec::new();
    for (path, bytes) in powerchat::ASSETS {
        let rel = format!("{}/{path}", powerchat::ASSETS_DIR);
        write(&root.dir().join(&rel), bytes)?;
        assets.push(rel);
    }
    for (file, text) in powerchat::SAMPLES {
        write(&root.dir().join(powerchat::SAMPLES_DIR).join(file), text.as_bytes())?;
    }
    std::fs::create_dir_all(root.dir().join(powerchat::DATA_DIR)).map_err(|e| e.to_string())?;

    // Registered, generated.
    for rel in forms.iter().chain(&assets) {
        super::register::run(host, root, rel, None)?;
    }
    for rel in &forms {
        super::regenerate::run(host, root, Some(rel))?;
    }

    // The Assistant item in the host's side menu: PowerChat opens as a window
    // of its own, modeless, beside whatever the operator is doing.
    let label = menu_label.map(str::trim).filter(|l| !l.is_empty()).unwrap_or("Assistant");
    let action = format!("open-standalone-async:{}", powerchat::MAIN_FORM);
    let side = main.as_ref().and_then(|(rel, form)| {
        let id = form.side_menu_control_id()?;
        let dir = root.resolve(rel).ok()?.parent()?.to_path_buf();
        Some((rel.clone(), id, dir))
    });
    let opens_with = match &side {
        Some((rel, id, dir)) => {
            let path = cobolt_forms::menu::menu_yaml_path(dir, id);
            let mut def = cobolt_forms::menu::load_menu(&path)
                .unwrap_or(cobolt_forms::menu::MenuDefinition { menu: Vec::new(), hash: String::new() });
            if !def.menu.iter().any(|i| i.action.as_deref() == Some(action.as_str())) {
                let mut item = cobolt_forms::menu::MenuItem::new_action("assistant", label);
                item.icon = Some("chat".into());
                item.action = Some(action.clone());
                def.menu.push(item);
                cobolt_forms::menu::save_menu(&path, &def).map_err(|e| format!("cannot save the menu of {rel}: {e:?}"))?;
            }
            json!({"menu_item": {"form": rel, "control": id, "label": label, "action": action}})
        }
        None => json!({
            "cobol": "INVOKE ME::\"OpenFormAsync\"(\"CHAT-FORM\")",
            "note": "the main form has no side menu: open PowerChat from a handler with this statement"
        }),
    };

    let checked = super::check::run(host, root, None)?;
    Ok(json!({
        "added": forms.len(),
        "forms": powerchat::FORMS_DIR,
        "assets": powerchat::ASSETS_DIR,
        "data": powerchat::DATA_DIR,
        "branding": {"name": brand.name, "icon": brand.icon},
        "opens_with": opens_with,
        "menu_label_translations": powerchat::MENU_LABELS.iter().map(|(l, t)| json!({"language": l, "text": t})).collect::<Vec<_>>(),
        "check": checked,
        "note": "PowerChat opens as a window of its own with its own side menu, in the application's theme. \
                 Put the Assistant label in the application's text table in all six languages. \
                 Its models are the application's: the runtime keeps them in settings/models.json, so \
                 the application's own AgentObjects can ask any model set up in PowerChat by name \
                 (ModelEntry), and PowerChat sees models the application adds. Without a project icon, \
                 PowerChat's side menu shows an empty logo box: suggest the developer give the project an icon.",
    }))
}

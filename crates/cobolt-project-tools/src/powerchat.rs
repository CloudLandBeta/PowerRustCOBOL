// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **PowerChat, ready to add to an application** (spec 085 R6–R13).
//!
//! The PowerChat example compiled into the binary — its forms, its menu, the
//! pictures it uses and the defaults it reads — and the one transformation
//! that makes a copy belong to the application it is added to:
//!
//! - its files live in folders of their own (`forms/powerchat/`,
//!   `Assets/powerchat/`, `samples/powerchat/`, data in `data/powerchat/`),
//!   so nothing collides with the host's;
//! - no form sets a theme or a glass style: they wear the host's (Spatial);
//! - no form is the main form;
//! - the host's name and icon replace PowerChat's own branding; the Titan
//!   Voyages logos are not copied.

/// Where the copied forms go.
pub const FORMS_DIR: &str = "forms/powerchat";
/// Where its pictures go.
pub const ASSETS_DIR: &str = "Assets/powerchat";
/// Where the defaults it reads go.
pub const SAMPLES_DIR: &str = "samples/powerchat";
/// Where it keeps its data.
pub const DATA_DIR: &str = "data/powerchat";
/// The form the host opens.
pub const MAIN_FORM: &str = "chat-form";

macro_rules! pc {
    ($path:literal) => {
        include_str!(concat!("../../../examples/PowerChat/", $path))
    };
}
macro_rules! pc_bytes {
    ($path:literal) => {
        include_bytes!(concat!("../../../examples/PowerChat/", $path))
    };
}

/// PowerChat's forms: `(file stem, XML)`.
pub const FORMS: &[(&str, &str)] = &[
    ("chat-form", pc!("forms/chat-form.cfrm")),
    ("topics-form", pc!("forms/topics-form.cfrm")),
    ("documents-form", pc!("forms/documents-form.cfrm")),
    ("settings-form", pc!("forms/settings-form.cfrm")),
    ("files-form", pc!("forms/files-form.cfrm")),
    ("prompts-form", pc!("forms/prompts-form.cfrm")),
    ("kb-folder-form", pc!("forms/kb-folder-form.cfrm")),
    ("providers-form", pc!("forms/providers-form.cfrm")),
    ("model-form", pc!("forms/model-form.cfrm")),
    ("agents-form", pc!("forms/agents-form.cfrm")),
    ("confirm-form", pc!("forms/confirm-form.cfrm")),
    ("pick-form", pc!("forms/pick-form.cfrm")),
    ("preview-form", pc!("forms/preview-form.cfrm")),
    ("welcome-form", pc!("forms/welcome-form.cfrm")),
];

/// Its side menu, beside its forms (sealed — copied byte for byte).
pub const MENU: (&str, &str) = ("SideMenu-1.menu.yaml", pc!("forms/SideMenu-1.menu.yaml"));

/// The pictures it shows: `(path under ASSETS_DIR, bytes)`.
pub const ASSETS: &[(&str, &[u8])] = &[
    ("flags/en.png", pc_bytes!("assets/flags/en.png")),
    ("flags/pt.png", pc_bytes!("assets/flags/pt.png")),
    ("flags/es.png", pc_bytes!("assets/flags/es.png")),
    ("flags/fr.png", pc_bytes!("assets/flags/fr.png")),
    ("flags/jp.png", pc_bytes!("assets/flags/jp.png")),
    ("flags/cn.png", pc_bytes!("assets/flags/cn.png")),
    ("robot.svg", pc_bytes!("assets/robot.svg")),
];

/// The defaults it reads: `(file under SAMPLES_DIR, text)`.
pub const SAMPLES: &[(&str, &str)] = &[
    ("main-prompt.md", pc!("samples/main-prompt.md")),
    ("report-templates.txt", pc!("samples/report-templates.txt")),
    ("report-templates-previous.txt", pc!("samples/report-templates-previous.txt")),
];

/// The host application's branding: its name, and its icon when it has one
/// (a project-relative path).
#[derive(Debug, Clone)]
pub struct Branding {
    pub name: String,
    pub icon: Option<String>,
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A COBOL literal's text: a quote doubled, so the host's name cannot end it.
fn cobol_text(s: &str) -> String {
    s.replace('"', "\"\"")
}

/// One copied form of PowerChat's, made the host's.
pub fn adapt_form(xml: &str, brand: &Branding) -> String {
    let mut out = String::with_capacity(xml.len());
    for (i, line) in xml.split_inclusive('\n').enumerate() {
        let mut l = line.to_owned();
        if i < 3 && l.trim_start().starts_with("<Form ") {
            // The host's look and the host's main form.
            l = strip_attr(&l, "theme");
            l = strip_attr(&l, "glass-style");
            l = strip_attr(&l, "main-form");
            l = l.replace("title=\"PowerChat\"", &format!("title=\"{}\"", xml_escape(&brand.name)));
        }
        let code = !l.trim_start().starts_with("*>");
        // The host's name in the texts the operator reads — every language's
        // row and the designed captions — never in a file format's marker.
        if code && (l.contains("VALUE \"") || l.contains("<Property name=\"Caption\">")) {
            l = l.replace("PowerChat", &if l.contains("VALUE \"") { cobol_text(&brand.name) } else { xml_escape(&brand.name) });
        }
        // Its own folders.
        l = l.replace("MOVE \"data\" TO WS-DATA-DIR", &format!("MOVE \"{DATA_DIR}\" TO WS-DATA-DIR"));
        l = l.replace("MOVE \"samples\" TO WS-SAMPLES-DIR", &format!("MOVE \"{SAMPLES_DIR}\" TO WS-SAMPLES-DIR"));
        l = l.replace("\"assets/KB\"", &format!("\"{DATA_DIR}/KB\""));
        l = l.replace(">assets/KB<", &format!(">{DATA_DIR}/KB<"));
        l = l.replace(">assets/flags/", &format!(">{ASSETS_DIR}/flags/"));
        l = l.replace(">assets/robot.svg<", &format!(">{ASSETS_DIR}/robot.svg<"));
        // The side menu's header: the host's icon and name, never Titan Voyages.
        if l.contains("<Property name=\"HeaderImage\">") && l.contains("Titan_Voyages") {
            l = format!(
                "    <Property name=\"HeaderImage\">{}</Property>\n",
                brand.icon.as_deref().map(xml_escape).unwrap_or_default()
            );
        }
        if l.contains("<Property name=\"HeaderIcon\">") && l.contains("Titan_Voyages") {
            l = format!(
                "    <Property name=\"HeaderIcon\">{}</Property>\n",
                brand.icon.as_deref().map(xml_escape).unwrap_or_default()
            );
        }
        if l.contains("<Property name=\"AppTitle\"></Property>") {
            l = format!("    <Property name=\"AppTitle\">{}</Property>\n", xml_escape(&brand.name));
        }
        out.push_str(&l);
    }
    out
}

/// `line` without the attribute `name="…"`.
fn strip_attr(line: &str, name: &str) -> String {
    let needle = format!(" {name}=\"");
    let Some(at) = line.find(&needle) else {
        return line.to_owned();
    };
    let rest = &line[at + needle.len()..];
    let Some(end) = rest.find('"') else {
        return line.to_owned();
    };
    format!("{}{}", &line[..at], &rest[end + 1..])
}

/// "Assistant" in the six languages, for the host's own text table.
pub const MENU_LABELS: [(&str, &str); 6] = [
    ("en", "Assistant"),
    ("pt", "Assistente"),
    ("es", "Asistente"),
    ("fr", "Assistant"),
    ("jp", "アシスタント"),
    ("cn", "助手"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The copy carries the host's name and look and its own folders, and
    /// nothing of PowerChat's branding: no main form, no theme, no Titan
    /// Voyages, no "PowerChat" an operator can read — while the settings
    /// file's format marker stays, so files exported before still import.
    #[test]
    fn a_copied_form_is_the_hosts() {
        let brand = Branding { name: "Inventory \"Pro\"".into(), icon: Some("Assets/icon.png".into()) };
        let mut visible = 0;
        for (stem, xml) in FORMS {
            let out = adapt_form(xml, &brand);
            let form_line = out.lines().find(|l| l.trim_start().starts_with("<Form ")).unwrap();
            for attr in ["theme=", "glass-style=", "main-form="] {
                assert!(!form_line.contains(attr), "{stem}: {attr} left on the form");
            }
            assert!(!out.contains("Titan_Voyages"), "{stem}: Titan Voyages");
            assert!(!out.contains("\"data\" TO WS-DATA-DIR"), "{stem}: PowerChat's data folder");
            assert!(!out.contains(">assets/") && !out.contains("\"assets/"), "{stem}: an asset outside {ASSETS_DIR}");
            for line in out.lines().filter(|l| !l.trim_start().starts_with("*>")) {
                if line.contains("PowerChat") {
                    assert!(
                        line.contains("application=\"PowerChat\"") || line.contains("POWERCHAT") || !line.contains("VALUE \"") && !line.contains("Caption"),
                        "{stem}: PowerChat shown to the operator: {line}"
                    );
                }
            }
            visible += out.matches("Inventory").count();
            cobolt_forms::load_form_from_str(&out).unwrap_or_else(|e| panic!("{stem} no longer loads: {e:?}"));
        }
        let chat = adapt_form(FORMS[0].1, &brand);
        assert!(chat.contains("<Property name=\"HeaderIcon\">Assets/icon.png</Property>"));
        assert!(chat.contains("<Property name=\"HeaderImage\">Assets/icon.png</Property>"));
        assert!(chat.contains("<Property name=\"AppTitle\">Inventory &quot;Pro&quot;</Property>"));
        let welcome = adapt_form(FORMS.iter().find(|f| f.0 == "welcome-form").unwrap().1, &brand);
        assert!(welcome.contains("Welcome! Inventory \"\"Pro\"\" needs a model"), "the host's name in the COBOL text, quotes doubled");
        println!("powerchat: {} forms adapted; the host's name shown {visible} times; no theme, no main form, no Titan Voyages", FORMS.len());
    }
}

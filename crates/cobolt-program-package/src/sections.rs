// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What a desktop module carries beside its code (DESKTOP-WASM-001 C1, T28): the forms, the menus,
//! the runner's settings and the theme art, each in a custom section of the one module, so the
//! program stays one self-describing `.wasm` file. A browser module carries none of them.
//!
//! | Section | Holds | Encoding |
//! |---|---|---|
//! | `prsettings` | [`RunnerSettings`]: what the Cargo path bakes into `main.rs` | JSON `{"format", "settings"}` |
//! | `prforms` | each form as a code-free [`FormDescription`], the main form first | JSON `{"format", "forms"}` |
//! | `prmenus` | each SideMenu's and MenuBar's `.menu.yaml`, keyed `FORM/CONTROL` | JSON `{"format", "menus"}` |
//! | `prart` | each asset-pack theme the forms use: its `theme.toml` and the art it draws | binary, below |
//!
//! **No code by construction.** A form travels as its description, which has no field for a
//! handler, the working storage or a procedure; the menus and the themes hold none.
//!
//! **Forever (R5).** Each section has its own format number. A reader takes every format up to its
//! own and refuses a newer one by name. A section the module lacks reads as empty, and missing
//! settings read as the defaults, so a module built before a section existed still runs.
//!
//! **`prart`, little-endian:** `u32` format, `u32` theme count, then per theme its id, its
//! `theme.toml`, a `u32` file count and per file its pack-relative path and its bytes. A string or
//! a byte run is a `u32` length and the bytes.

use std::fmt;

use cobolt_forms::description::FormDescription;
use serde::{Deserialize, Serialize};

use crate::RunnerSettings;

/// The runner's settings.
pub const SETTINGS_SECTION: &str = "prsettings";
/// The forms, as code-free descriptions.
pub const FORMS_SECTION: &str = "prforms";
/// The menu sidecars.
pub const MENUS_SECTION: &str = "prmenus";
/// The theme art.
pub const ART_SECTION: &str = "prart";

/// The format of `prsettings` this crate writes and the newest it reads.
pub const SETTINGS_FORMAT: u32 = 1;
/// The format of `prforms`.
pub const FORMS_FORMAT: u32 = 1;
/// The format of `prmenus`.
pub const MENUS_FORMAT: u32 = 1;
/// The format of `prart`.
pub const ART_FORMAT: u32 = 1;

/// One menu sidecar: the control that owns it (`cobolt_forms::menu::menu_key`) and its YAML.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSource {
    pub key: String,
    pub yaml: String,
}

/// One asset-pack theme: its id, its `theme.toml`, and each image it draws by its pack-relative
/// path, in the manifest's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeArt {
    pub id: String,
    pub manifest: String,
    pub files: Vec<(String, Vec<u8>)>,
}

/// Everything a desktop module carries beside its code.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesktopSections {
    pub settings: RunnerSettings,
    pub forms: Vec<FormDescription>,
    pub menus: Vec<MenuSource>,
    pub art: Vec<ThemeArt>,
}

/// A section that cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionError {
    pub section: &'static str,
    pub detail: String,
}

impl fmt::Display for SectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the program's `{}` section {}", self.section, self.detail)
    }
}

impl std::error::Error for SectionError {}

#[derive(Serialize, Deserialize)]
struct SettingsSection {
    format: u32,
    #[serde(default)]
    settings: RunnerSettings,
}

#[derive(Serialize, Deserialize)]
struct FormsSection {
    format: u32,
    #[serde(default)]
    forms: Vec<FormDescription>,
}

#[derive(Serialize, Deserialize)]
struct MenusSection {
    format: u32,
    #[serde(default)]
    menus: Vec<MenuSource>,
}

/// Just the format number of a JSON section, read before the rest so a newer format is named as
/// such and not as a damaged section.
#[derive(Deserialize)]
struct Format {
    format: u32,
}

impl DesktopSections {
    /// The sections as `(name, bytes)`, in the order a module carries them. The settings are always
    /// there; a section with nothing in it is left out.
    pub fn encode(&self) -> Vec<(&'static str, Vec<u8>)> {
        let mut out = vec![(SETTINGS_SECTION, json(&SettingsSection { format: SETTINGS_FORMAT, settings: self.settings.clone() }))];
        if !self.forms.is_empty() {
            out.push((FORMS_SECTION, json(&FormsSection { format: FORMS_FORMAT, forms: self.forms.clone() })));
        }
        if !self.menus.is_empty() {
            out.push((MENUS_SECTION, json(&MenusSection { format: MENUS_FORMAT, menus: self.menus.clone() })));
        }
        if !self.art.is_empty() {
            out.push((ART_SECTION, encode_art(&self.art)));
        }
        out
    }

    /// The sections of a module, `custom` answering a custom section's bytes by its name.
    pub fn decode<'a>(custom: impl Fn(&str) -> Option<&'a [u8]>) -> Result<Self, SectionError> {
        let settings = match custom(SETTINGS_SECTION) {
            Some(b) => json_section::<SettingsSection>(SETTINGS_SECTION, b, SETTINGS_FORMAT)?.settings,
            None => RunnerSettings::default(),
        };
        let forms = match custom(FORMS_SECTION) {
            Some(b) => json_section::<FormsSection>(FORMS_SECTION, b, FORMS_FORMAT)?.forms,
            None => Vec::new(),
        };
        let menus = match custom(MENUS_SECTION) {
            Some(b) => json_section::<MenusSection>(MENUS_SECTION, b, MENUS_FORMAT)?.menus,
            None => Vec::new(),
        };
        let art = match custom(ART_SECTION) {
            Some(b) => decode_art(b)?,
            None => Vec::new(),
        };
        Ok(DesktopSections { settings, forms, menus, art })
    }
}

fn json<T: Serialize>(section: &T) -> Vec<u8> {
    serde_json::to_vec(section).expect("a section serialises")
}

fn newer(section: &'static str, found: u32, max: u32) -> SectionError {
    SectionError {
        section,
        detail: format!("is format {found}, newer than this runner reads ({max}): run the program with a later PowerRustCOBOL runner"),
    }
}

fn json_section<T: for<'de> Deserialize<'de>>(section: &'static str, bytes: &[u8], max: u32) -> Result<T, SectionError> {
    let damaged = |e: serde_json::Error| SectionError { section, detail: format!("cannot be read: {e}") };
    let Format { format } = serde_json::from_slice(bytes).map_err(damaged)?;
    if format > max {
        return Err(newer(section, format, max));
    }
    serde_json::from_slice(bytes).map_err(damaged)
}

fn encode_art(themes: &[ThemeArt]) -> Vec<u8> {
    fn run(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&ART_FORMAT.to_le_bytes());
    out.extend_from_slice(&(themes.len() as u32).to_le_bytes());
    for t in themes {
        run(&mut out, t.id.as_bytes());
        run(&mut out, t.manifest.as_bytes());
        out.extend_from_slice(&(t.files.len() as u32).to_le_bytes());
        for (path, data) in &t.files {
            run(&mut out, path.as_bytes());
            run(&mut out, data);
        }
    }
    out
}

fn decode_art(bytes: &[u8]) -> Result<Vec<ThemeArt>, SectionError> {
    let damaged = |what: &str| SectionError { section: ART_SECTION, detail: format!("is damaged: {what}") };
    let mut at = 0usize;
    let u32_at = |at: &mut usize| -> Result<u32, SectionError> {
        let b = bytes.get(*at..*at + 4).ok_or_else(|| damaged("it ends inside a number"))?;
        *at += 4;
        Ok(u32::from_le_bytes(b.try_into().expect("four bytes")))
    };
    let format = u32_at(&mut at)?;
    if format > ART_FORMAT {
        return Err(newer(ART_SECTION, format, ART_FORMAT));
    }
    let run = |at: &mut usize, len: u32| -> Result<Vec<u8>, SectionError> {
        let end = at.checked_add(len as usize).filter(|&e| e <= bytes.len()).ok_or_else(|| damaged("a length runs past its end"))?;
        let v = bytes[*at..end].to_vec();
        *at = end;
        Ok(v)
    };
    let text = |v: Vec<u8>| String::from_utf8(v).map_err(|_| damaged("a name is not UTF-8"));
    let count = u32_at(&mut at)?;
    let mut themes = Vec::new();
    for _ in 0..count {
        let n = u32_at(&mut at)?;
        let id = text(run(&mut at, n)?)?;
        let n = u32_at(&mut at)?;
        let manifest = text(run(&mut at, n)?)?;
        let files_n = u32_at(&mut at)?;
        let mut files = Vec::new();
        for _ in 0..files_n {
            let n = u32_at(&mut at)?;
            let path = text(run(&mut at, n)?)?;
            let n = u32_at(&mut at)?;
            files.push((path, run(&mut at, n)?));
        }
        themes.push(ThemeArt { id, manifest, files });
    }
    if at != bytes.len() {
        return Err(damaged("bytes follow its last theme"));
    }
    Ok(themes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FocusRing;

    fn sample() -> DesktopSections {
        let form = cobolt_forms::xml::load_form_from_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form name="MAIN-FORM" title="Main" width="320" height="200" main-form="true">
  <Control id="BTN-OK" type="Button" x="10" y="10" w="80" h="30" tab-order="0" z-order="0" visible="true" enabled="true">
    <Property name="Caption">OK</Property>
    <Event name="onClick" paragraph="BTN-OK--ONCLICK"><![CDATA[       PROCEDURE DIVISION.
           MOVE "SECRET-HANDLER-LINE" TO WS-X.
]]></Event>
  </Control>
</Form>"#,
        )
        .expect("the sample form loads");
        DesktopSections {
            settings: RunnerSettings {
                app_name: "Sample".into(),
                version: "1.2.3".into(),
                main_form: "MAIN-FORM".into(),
                locale: "pt-BR".into(),
                focus_ring: FocusRing { enabled: true, color: "#FF8800".into(), pulse: true },
                link_schemes: vec!["mailto".into()],
                ..RunnerSettings::default()
            },
            forms: vec![cobolt_forms::description::describe(&form)],
            menus: vec![MenuSource { key: "MAIN-FORM/SIDEMENU-1".into(), yaml: "items: []\n".into() }],
            art: vec![ThemeArt { id: "aqua".into(), manifest: "name = \"Aqua\"\n".into(), files: vec![("bg.png".into(), vec![0, 1, 2, 255])] }],
        }
    }

    fn read(encoded: &[(&'static str, Vec<u8>)]) -> Result<DesktopSections, SectionError> {
        DesktopSections::decode(|name| encoded.iter().find(|(n, _)| *n == name).map(|(_, b)| b.as_slice()))
    }

    #[test]
    fn the_four_sections_round_trip_and_hold_no_handler_code() {
        let s = sample();
        let encoded = s.encode();
        let names: Vec<&str> = encoded.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, [SETTINGS_SECTION, FORMS_SECTION, MENUS_SECTION, ART_SECTION]);
        assert_eq!(read(&encoded).unwrap(), s);
        let forms = &encoded[1].1;
        assert!(!forms.windows(19).any(|w| w == b"SECRET-HANDLER-LINE"), "a handler's code is not in `prforms`");
        let bytes: usize = encoded.iter().map(|(_, b)| b.len()).sum();
        println!("sections {}, {bytes} bytes, round trip 1 of 1, handler lines found 0", encoded.len());
    }

    #[test]
    fn a_module_without_sections_reads_as_defaults_and_settings_are_always_written() {
        assert_eq!(read(&[]).unwrap(), DesktopSections::default());
        let only = DesktopSections::default().encode();
        assert_eq!(only.len(), 1, "an empty program still carries its settings");
        assert_eq!(only[0].0, SETTINGS_SECTION);
    }

    #[test]
    fn a_newer_format_is_named_and_a_damaged_section_refused() {
        let newer_settings = format!("{{\"format\":{},\"settings\":{{}}}}", SETTINGS_FORMAT + 1);
        let e = read(&[(SETTINGS_SECTION, newer_settings.into_bytes())]).unwrap_err();
        assert_eq!(e.section, SETTINGS_SECTION);
        assert!(e.to_string().contains("newer than this runner reads"), "{e}");
        let mut art = encode_art(&sample().art);
        art[0] = (ART_FORMAT + 1) as u8;
        assert!(read(&[(ART_SECTION, art)]).unwrap_err().to_string().contains("newer"));
        let mut cut = encode_art(&sample().art);
        cut.truncate(cut.len() - 1);
        assert!(read(&[(ART_SECTION, cut)]).unwrap_err().to_string().contains("damaged"));
        assert!(read(&[(FORMS_SECTION, b"{".to_vec())]).unwrap_err().to_string().contains("cannot be read"));
    }

    #[test]
    fn unknown_settings_fields_are_passed_by() {
        let text = br#"{"format":1,"settings":{"app_name":"A","a_field_from_a_later_build":7}}"#;
        let s = read(&[(SETTINGS_SECTION, text.to_vec())]).unwrap();
        assert_eq!(s.settings.app_name, "A");
    }
}

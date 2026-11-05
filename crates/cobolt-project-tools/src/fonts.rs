// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The fonts a project references** (WASM-018 T27, R8, Q7): every family a form or a program names, found,
//! checked for its licence, and handed to the Build to ship.
//!
//! A font is *referenced* three ways:
//!
//! 1. a control's `FontName` in a `.cfrm`;
//! 2. a **literal** passed to `SetFont` anywhere in the project's COBOL (the forms' handlers, Common Code, copybooks);
//! 3. a font file under `Assets/` (those are bytes already, and are always shipped).
//!
//! A family named only at run time, from a data item, cannot be found by reading: the Build says so, once per place,
//! and the program may ship the file under `Assets/`. The project's COBOL is **read, never written**.
//!
//! The families of 1 and 2 are looked for on the developer's machine with the same `fontdb` query the desktop painter
//! uses. What is found is shipped, so every host letters a text with the same font file (AC1, AC3). Shipping a font is
//! redistributing it, so each file's embedding permission (the OpenType `OS/2.fsType`) and licence string are read and
//! listed in the Build report, and a font that forbids embedding is refused.

use std::collections::BTreeMap;

/// The families every host letters without a font file: the built-in face and the CSS generics.
pub fn is_builtin_family(family: &str) -> bool {
    let f = family.trim().trim_matches(|c| c == '"' || c == '\'');
    f.is_empty()
        || [
            "arial",
            "helvetica",
            "sans-serif",
            "serif",
            "monospace",
            "cursive",
            "fantasy",
            "system-ui",
            "ui-sans-serif",
            "ui-serif",
            "ui-monospace",
            "inherit",
            "initial",
        ]
        .iter()
        .any(|g| f.eq_ignore_ascii_case(g))
}

/// A family as two spellings of it are compared: case, spaces, hyphens and underscores ignored.
pub fn family_key(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, ' ' | '-' | '_' | '"' | '\'')).flat_map(char::to_lowercase).collect()
}

/// What a program's text says about fonts: the families named by a `SetFont` literal, and the places that name none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CobolFonts {
    /// Families in the order met, each once.
    pub families: Vec<String>,
    /// 1-based lines of a `SetFont` whose argument is not a literal.
    pub non_literal: Vec<usize>,
}

/// Read the `SetFont` calls of a COBOL text. A call is the word `SetFont` (any case, bare or after `::`/`"`) and the
/// statement that follows it; its first quoted literal is the font. A text without a quoted literal before the
/// statement ends (`.`, `END-`, a blank line, or ~240 characters) is a non-literal call.
pub fn setfont_in_cobol(text: &str) -> CobolFonts {
    let mut out = CobolFonts::default();
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("setfont") {
        let start = from + at;
        let after = start + "setfont".len();
        from = after;
        // `SetFont` must be the whole word, not the tail of `GetFontSet` or a longer name.
        let before_ok = start == 0 || !lower.as_bytes()[start - 1].is_ascii_alphanumeric() && lower.as_bytes()[start - 1] != b'-';
        let after_ok = after >= lower.len() || !lower.as_bytes()[after].is_ascii_alphanumeric() && lower.as_bytes()[after] != b'-';
        if !before_ok || !after_ok {
            continue;
        }
        let line = text[..start].bytes().filter(|&b| b == b'\n').count() + 1;
        // A comment line names the word without calling it.
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        if text[line_start..start].trim_start().starts_with("*>") {
            continue;
        }
        // The statement after the word, up to its end; a name in quotes (`INVOKE X "SetFont" USING ...`) ends at its quote.
        let named_in_quotes = start > 0 && matches!(text.as_bytes()[start - 1], b'"' | b'\'') && text.as_bytes().get(after) == Some(&text.as_bytes()[start - 1]);
        let rest = &text[if named_in_quotes { after + 1 } else { after }..];
        let mut end = rest.len().min(240);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let window = &rest[..end];
        let mut stop = window.len();
        for pat in ["\n\n", "END-INVOKE", "END-CALL", "end-invoke", "end-call"] {
            if let Some(i) = window.find(pat) {
                stop = stop.min(i);
            }
        }
        let stmt = &window[..stop];
        match first_literal(stmt) {
            Some(lit) => {
                for fam in families_of_css(&lit) {
                    if !is_builtin_family(&fam) && !out.families.iter().any(|f| family_key(f) == family_key(&fam)) {
                        out.families.push(fam);
                    }
                }
            }
            None => out.non_literal.push(line),
        }
    }
    out
}

/// The first `"…"` or `'…'` literal of a statement, when no data name stands between the call and it.
fn first_literal(stmt: &str) -> Option<String> {
    let q = stmt.find(['"', '\''])?;
    let quote = stmt.as_bytes()[q] as char;
    // A `.` before the quote ends the statement: the literal belongs to the next one.
    if stmt[..q].contains(". ") || stmt[..q].trim_end().ends_with('.') {
        return None;
    }
    // A data name between `SetFont` and the literal (`USING WS-FONT "x"`) means the font is not the literal.
    let between = stmt[..q].trim();
    let words: Vec<&str> = between.split(|c: char| c.is_whitespace() || matches!(c, '(' | ',' | ':')).filter(|w| !w.is_empty()).collect();
    if words.iter().any(|w| !matches!(w.to_ascii_uppercase().as_str(), "USING" | "BY" | "CONTENT" | "REFERENCE" | "VALUE" | "SET" | "TO")) {
        return None;
    }
    let body = &stmt[q + 1..];
    let close = body.find(quote)?;
    Some(body[..close].to_owned())
}

/// The families of a CSS font shorthand (`"italic 700 16px Georgia, serif"`) or of a bare family list (`"Georgia"`, as
/// a Plot's `SetFont` takes), generics included.
pub fn families_of_css(css: &str) -> Vec<String> {
    if let Some(spec) = cobolt_graphics::textapi::FontSpec::parse(css) {
        return spec.families;
    }
    css.split(',').map(|s| s.trim().trim_matches(|c| c == '"' || c == '\'').to_owned()).filter(|s| !s.is_empty()).collect()
}

/// What a font file allows (OpenType `OS/2.fsType`) and says about its licence (name ids 13 and 14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Licence {
    /// The `fsType` word, `None` when the file has no `OS/2` table (it then states no restriction).
    pub fs_type: Option<u16>,
    /// What the embedding bits mean, in words.
    pub embedding: &'static str,
    /// Whether a file may be shipped inside a bundle (everything but "restricted licence embedding").
    pub may_embed: bool,
    /// The licence text and URL the font names, joined; empty when it names none.
    pub text: String,
}

/// Read a font's embedding permission and licence. `None` when the bytes are not a font.
pub fn licence_of(bytes: &[u8]) -> Option<Licence> {
    use ttf_parser::{name_id, Permissions};
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let (fs_type, embedding, may_embed) = match face.permissions() {
        None => (None, "no OS/2 table: no restriction stated", true),
        Some(Permissions::Installable) => (os2_fs_type(bytes), "installable embedding", true),
        Some(Permissions::Restricted) => (os2_fs_type(bytes), "restricted licence embedding: the font forbids embedding", false),
        Some(Permissions::PreviewAndPrint) => (os2_fs_type(bytes), "preview and print embedding", true),
        Some(Permissions::Editable) => (os2_fs_type(bytes), "editable embedding", true),
    };
    let mut parts = Vec::new();
    for id in [name_id::LICENSE, name_id::LICENSE_URL] {
        if let Some(s) = face.names().into_iter().filter(|n| n.name_id == id).find_map(|n| n.to_string()) {
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            if !s.is_empty() {
                parts.push(s);
            }
        }
    }
    Some(Licence { fs_type, embedding, may_embed, text: parts.join(" · ") })
}

/// The `OS/2.fsType` word of the first face, read from the table directory.
fn os2_fs_type(bytes: &[u8]) -> Option<u16> {
    let u16_at = |o: usize| bytes.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32_at = |o: usize| bytes.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    // A collection starts with `ttcf`; the first face's offset table follows the header.
    let base = if bytes.get(0..4) == Some(b"ttcf") { u32_at(12)? as usize } else { 0 };
    let tables = u16_at(base + 4)? as usize;
    for i in 0..tables {
        let rec = base + 12 + i * 16;
        if bytes.get(rec..rec + 4) == Some(b"OS/2") {
            let off = u32_at(rec + 8)? as usize;
            return u16_at(off + 8);
        }
    }
    None
}

/// One font file the Build ships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippedFont {
    /// The family the project names (as the project spells it).
    pub family: String,
    /// Where the bundle keeps it: `Assets/fonts/<file>`.
    pub path: String,
    pub size: u64,
    /// `Assets/` when the project supplies the file, `system` when the Build found it on the machine.
    pub origin: &'static str,
    pub licence: Option<Licence>,
}

/// A font file found for a family.
#[derive(Debug, Clone)]
pub struct Found {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Where families are looked for: the machine's fonts, or a fixed set in a test.
pub trait FontFinder {
    /// The faces (regular, bold, italic, bold italic at most) of a family; empty when the machine has none.
    fn find(&self, family: &str) -> Vec<Found>;
}

/// The operating system's fonts, through `fontdb`, the query the desktop painter makes.
pub struct SystemFonts {
    db: fontdb::Database,
}

impl SystemFonts {
    pub fn load() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Self { db }
    }
}

impl FontFinder for SystemFonts {
    fn find(&self, family: &str) -> Vec<Found> {
        let key = family_key(family);
        let mut chosen: Vec<(u16, bool, fontdb::ID)> = Vec::new();
        for f in self.db.faces() {
            if !f.families.iter().any(|(n, _)| family_key(n) == key) {
                continue;
            }
            // The four classic faces; a variable font's one file serves every weight.
            let (w, italic) = (f.weight.0, !matches!(f.style, fontdb::Style::Normal));
            if matches!(w, 400 | 700) && !chosen.iter().any(|c| c.0 == w && c.1 == italic) {
                chosen.push((w, italic, f.id));
            }
        }
        if chosen.is_empty() {
            // No 400/700 face: the family's first face stands in for the regular one.
            if let Some(f) = self.db.faces().find(|f| f.families.iter().any(|(n, _)| family_key(n) == key)) {
                chosen.push((f.weight.0, false, f.id));
            }
        }
        let mut out: Vec<Found> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for (_, _, id) in chosen {
            let Some(face) = self.db.face(id) else { continue };
            let file = match &face.source {
                fontdb::Source::File(p) | fontdb::Source::SharedFile(p, _) => p.file_name().map(|n| n.to_string_lossy().into_owned()),
                _ => None,
            }
            .unwrap_or_else(|| format!("{}.ttf", sanitize(family)));
            // A collection (`.ttc`) holds several faces in one file: it ships once.
            if seen.contains(&file) {
                continue;
            }
            seen.push(file.clone());
            if let Some(bytes) = self.db.with_face_data(id, |data, _| data.to_vec()) {
                out.push(Found { file_name: file, bytes });
            }
        }
        out
    }
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect()
}

/// What the Build decides about the fonts a project references.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FontOutcome {
    /// Files to add to the bundle's assets, as `(path, bytes)`; the project's own `Assets/` fonts are not repeated.
    pub files: Vec<(String, Vec<u8>)>,
    /// Every font the bundle carries (the project's own and the machine's), for the Build report.
    pub shipped: Vec<ShippedFont>,
    /// Refusals: a family nothing supplies on a web or mobile build, a font that forbids embedding.
    pub errors: Vec<String>,
    /// Things to know that do not stop the build.
    pub warnings: Vec<String>,
}

/// One family the project names and where (`form`, or a file for a `SetFont`).
pub type Needs = BTreeMap<String, (String, Vec<String>)>;

/// Add `family` as named in `place` to `needs`, keyed by its comparable spelling.
pub fn need(needs: &mut Needs, family: &str, place: &str) {
    let spelled = family.trim().trim_matches(|c| c == '"' || c == '\'').to_owned();
    if is_builtin_family(&spelled) {
        return;
    }
    let e = needs.entry(family_key(&spelled)).or_insert_with(|| (spelled, Vec::new()));
    if !e.1.iter().any(|p| p == place) {
        e.1.push(place.to_owned());
    }
}

/// What a project's forms and programs say about fonts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    pub needs: Needs,
    /// `(file, 1-based line)` of each `SetFont` whose argument is not a literal.
    pub non_literal: Vec<(String, usize)>,
}

/// Collect what the forms (`(file, form)`) and the program texts (`(file, text)`: every form file and Common Code) name:
/// each control's `FontName` and each `SetFont` literal. The texts are read, never written.
pub fn references<'a>(forms: impl IntoIterator<Item = (&'a str, &'a cobolt_forms::model::Form)>, texts: &[(String, String)]) -> References {
    fn walk(cs: &[cobolt_forms::model::Control], form: &str, needs: &mut Needs) {
        for c in cs {
            if let Some(cobolt_forms::model::PropValue::String(n)) = c.properties.get("FontName") {
                need(needs, n, &format!("form {form}"));
            }
            walk(&c.children, form, needs);
        }
    }
    let mut r = References::default();
    for (_, f) in forms {
        walk(&f.controls, &f.name, &mut r.needs);
    }
    for (file, text) in texts {
        let c = setfont_in_cobol(text);
        for fam in &c.families {
            need(&mut r.needs, fam, file);
        }
        r.non_literal.extend(c.non_literal.into_iter().map(|l| (file.clone(), l)));
    }
    r
}

/// The font files a desktop run registers besides the project's own `Assets/` ones: the families the project's forms and
/// programs name that the machine has, as `(Assets/fonts/<file>, bytes)` (WASM-018 T27: the same files a Web build ships,
/// so a picture is the same on every host). A family the machine does not have, or a font that forbids embedding, is
/// left out here: the desktop letters it in the stand-in face (the Build of a Web project refuses it).
pub fn project_font_files(manifest: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let Ok(view) = cobolt_compiler::project_manifest_view(manifest) else { return Vec::new() };
    let dir = manifest.parent().unwrap_or_else(|| std::path::Path::new("."));
    let mut forms = Vec::new();
    let mut texts: Vec<(String, String)> = Vec::new();
    for rel in &view.forms {
        let Ok(cfrm) = std::fs::read_to_string(dir.join(rel)) else { continue };
        if let Ok(f) = cobolt_forms::xml::load_form_from_str(&cfrm) {
            forms.push((rel.clone(), f));
        }
        texts.push((rel.clone(), cfrm));
    }
    for rel in &view.sources {
        if let Ok(t) = std::fs::read_to_string(dir.join(rel)) {
            texts.push((rel.replace('\\', "/"), t));
        }
    }
    let refs = references(forms.iter().map(|(r, f)| (r.as_str(), f)), &texts);
    if refs.needs.is_empty() {
        return Vec::new();
    }
    let assets: Vec<(String, Vec<u8>)> = view.assets.iter().filter_map(|rel| Some((rel.replace('\\', "/"), std::fs::read(dir.join(rel)).ok()?))).collect();
    plan(&refs.needs, &assets, &SystemFonts::load(), false).files
}

/// Decide the fonts of a build.
///
/// * `needs` — the families the forms and the programs name;
/// * `assets` — the project's `Assets/` files `(path, bytes)`; each font file there is listed and always shipped;
/// * `finder` — where a family no asset supplies is looked for;
/// * `strict` — `true` for a Web or Mobile build (an unresolvable family is an error: the picture would differ between
///   platforms), `false` for a Desktop one (a warning: the machine's own font letters it).
pub fn plan(needs: &Needs, assets: &[(String, Vec<u8>)], finder: &dyn FontFinder, strict: bool) -> FontOutcome {
    let mut out = FontOutcome::default();
    // The project's own font files, and the names each answers to.
    let mut asset_names: Vec<(String, Vec<String>)> = Vec::new();
    for (path, bytes) in assets {
        let p = path.to_ascii_lowercase();
        if ![".ttf", ".otf", ".ttc", ".woff", ".woff2"].iter().any(|e| p.ends_with(e)) {
            continue;
        }
        let stem = path.rsplit('/').next().unwrap_or(path).rsplit_once('.').map_or(path.as_str(), |(s, _)| s);
        let mut names = vec![family_key(stem)];
        if let Ok(face) = ttf_parser::Face::parse(bytes, 0) {
            for n in face.names() {
                if matches!(n.name_id, ttf_parser::name_id::FAMILY | ttf_parser::name_id::TYPOGRAPHIC_FAMILY) {
                    if let Some(s) = n.to_string() {
                        names.push(family_key(&s));
                    }
                }
            }
        }
        let licence = licence_of(bytes);
        if let Some(l) = &licence {
            if !l.may_embed {
                out.errors.push(format!("{path}: the font forbids embedding ({}); a bundle cannot carry it. Use a font whose licence allows it", l.embedding));
            }
        }
        out.shipped.push(ShippedFont { family: stem.to_owned(), path: path.clone(), size: bytes.len() as u64, origin: "Assets/", licence });
        asset_names.push((path.clone(), names));
    }
    for (key, (spelled, places)) in needs {
        if asset_names.iter().any(|(_, n)| n.contains(key)) {
            continue;
        }
        let found = finder.find(spelled);
        let where_ = places.join(", ");
        if found.is_empty() {
            let msg = format!(
                "the font `{spelled}` (named in {where_}) is not on this machine and no file under Assets/ supplies it: {}",
                if strict {
                    "a text would be lettered in another font on each platform. Put the font file under Assets/, or name a font this machine has"
                } else {
                    "the desktop letters it in its own copy, else in the built-in face"
                }
            );
            if strict {
                out.errors.push(msg);
            } else {
                out.warnings.push(msg);
            }
            continue;
        }
        for f in found {
            let licence = licence_of(&f.bytes);
            if let Some(l) = &licence {
                if !l.may_embed {
                    out.errors.push(format!(
                        "the font `{spelled}` (named in {where_}), file {}, forbids embedding ({}): it is not shipped. Use a font whose licence allows it, or a file under Assets/ you may redistribute",
                        f.file_name, l.embedding
                    ));
                    continue;
                }
            }
            let path = format!("Assets/fonts/{}", sanitize(&f.file_name));
            if out.files.iter().any(|(p, _)| *p == path) {
                continue;
            }
            out.shipped.push(ShippedFont { family: spelled.clone(), path: path.clone(), size: f.bytes.len() as u64, origin: "system", licence });
            out.files.push((path, f.bytes));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const UBUNTU: &[u8] = include_bytes!("../../../assets/web-fonts/Ubuntu-Light.ttf");

    /// The font with its `OS/2.fsType` set to `fs`.
    fn with_fs_type(fs: u16) -> Vec<u8> {
        let mut b = UBUNTU.to_vec();
        let tables = u16::from_be_bytes([b[4], b[5]]) as usize;
        for i in 0..tables {
            let rec = 12 + i * 16;
            if &b[rec..rec + 4] == b"OS/2" {
                let off = u32::from_be_bytes([b[rec + 8], b[rec + 9], b[rec + 10], b[rec + 11]]) as usize;
                b[off + 8..off + 10].copy_from_slice(&fs.to_be_bytes());
            }
        }
        b
    }

    struct Fixed(Vec<(&'static str, Vec<Found>)>);
    impl FontFinder for Fixed {
        fn find(&self, family: &str) -> Vec<Found> {
            self.0.iter().find(|(n, _)| family_key(n) == family_key(family)).map(|(_, f)| f.clone()).unwrap_or_default()
        }
    }
    fn found(name: &str, bytes: Vec<u8>) -> Vec<Found> {
        vec![Found { file_name: name.into(), bytes }]
    }

    #[test]
    fn a_setfont_literal_names_its_families_and_a_data_item_is_reported_by_line() {
        let cbl = "\
*> SetFont is in a comment: not a call
           CNV-1::SetFont(\"italic 700 16px Georgia, serif\")
           INVOKE PLT-1 \"SetFont\" USING \"Lato\" 12
           CNV-1::SetFont( WS-FONT )
           INVOKE CNV-1 \"SetFont\" USING WS-FONT-2
           CNV-1::SetFont(\"12px Arial\")
           CNV-1::GetFontSet()
";
        let r = setfont_in_cobol(cbl);
        assert_eq!(r.families, ["Georgia", "Lato"], "Arial and serif are every host's own; a comment is no call");
        assert_eq!(r.non_literal, [4, 5], "the two calls that name no literal, by line");
    }

    #[test]
    fn the_embedding_permission_and_the_licence_text_are_read_from_the_font() {
        let free = licence_of(UBUNTU).unwrap();
        assert!(free.may_embed, "{free:?}");
        assert!(licence_of(&with_fs_type(0x0002)).unwrap().may_embed == false, "fsType 2 is restricted licence embedding");
        let r = licence_of(&with_fs_type(0x0002)).unwrap();
        assert_eq!(r.fs_type, Some(2));
        assert!(r.embedding.contains("forbids"), "{}", r.embedding);
        assert!(licence_of(&with_fs_type(0x0008)).unwrap().may_embed, "editable embedding may be shipped");
        assert!(licence_of(b"not a font").is_none());
    }

    #[test]
    fn a_project_ships_exactly_the_fonts_it_references_in_a_cfrm_a_setfont_literal_and_assets() {
        let mut needs = Needs::new();
        need(&mut needs, "Lora", "form MAIN");
        need(&mut needs, "Georgia", "src/draw.cbl");
        need(&mut needs, "Arial", "form MAIN");
        need(&mut needs, "sans-serif", "form MAIN");
        let assets = vec![("Assets/Brand.ttf".to_owned(), UBUNTU.to_vec()), ("Assets/logo.png".to_owned(), vec![1, 2, 3])];
        let finder = Fixed(vec![("Lora", found("Lora-Regular.ttf", UBUNTU.to_vec())), ("Georgia", found("Georgia.ttf", UBUNTU.to_vec())), ("Verdana", found("Verdana.ttf", UBUNTU.to_vec()))]);
        let out = plan(&needs, &assets, &finder, true);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        let shipped: Vec<&str> = out.shipped.iter().map(|s| s.path.as_str()).collect();
        assert_eq!(shipped, ["Assets/Brand.ttf", "Assets/fonts/Georgia.ttf", "Assets/fonts/Lora-Regular.ttf"], "the asset, the SetFont family, the .cfrm family (in family order); Arial, generics, Verdana and the PNG are not shipped");
        assert_eq!(out.files.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), ["Assets/fonts/Georgia.ttf", "Assets/fonts/Lora-Regular.ttf"], "the project's own file is not copied again");
    }

    #[test]
    fn a_family_nothing_supplies_fails_a_web_build_and_only_warns_on_the_desktop() {
        let mut needs = Needs::new();
        need(&mut needs, "Nowhere Sans", "form MAIN");
        let none = Fixed(vec![]);
        let web = plan(&needs, &[], &none, true);
        assert_eq!(web.errors.len(), 1, "{web:?}");
        assert!(web.errors[0].contains("Nowhere Sans") && web.errors[0].contains("form MAIN") && web.errors[0].contains("Assets/"), "{}", web.errors[0]);
        let desktop = plan(&needs, &[], &none, false);
        assert!(desktop.errors.is_empty() && desktop.warnings.len() == 1, "{desktop:?}");
        // A project's own file that answers to the family settles it.
        let assets = vec![("Assets/NowhereSans.ttf".to_owned(), UBUNTU.to_vec())];
        assert!(plan(&needs, &assets, &none, true).errors.is_empty(), "the file name stands for the family");
    }

    #[test]
    fn a_font_that_forbids_embedding_is_refused_whether_found_or_supplied() {
        let mut needs = Needs::new();
        need(&mut needs, "Locked", "form MAIN");
        let finder = Fixed(vec![("Locked", found("Locked.ttf", with_fs_type(0x0002)))]);
        let out = plan(&needs, &[], &finder, true);
        assert_eq!(out.errors.len(), 1, "{out:?}");
        assert!(out.errors[0].contains("forbids embedding") && out.errors[0].contains("Locked.ttf"), "{}", out.errors[0]);
        assert!(out.files.is_empty(), "a refused font is not shipped");
        let own = plan(&Needs::new(), &[("Assets/Locked.ttf".to_owned(), with_fs_type(0x0002))], &finder, true);
        assert_eq!(own.errors.len(), 1, "{own:?}");
        assert!(own.errors[0].contains("Assets/Locked.ttf"), "{}", own.errors[0]);
    }
}

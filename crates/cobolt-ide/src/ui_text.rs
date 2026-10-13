// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The words of the IDE that are not `Tr` fields, in two tables.
//!
//! **The Properties pane's** ([`tx`], [`txf`], [`txc`]): the descriptive labels of its rows
//! ("Corner radius"), its section and group titles, hints, notes, buttons and the tooltips that
//! are not a property's explanation (those are `prop_help`'s).
//!
//! **The rest of the IDE's** ([`ide`], [`idef`]): the buttons, tooltips, headings and messages of
//! the error dialogs, the benchmark and model reports, the asset preview, the About box, the
//! code editor, the debugger, the Grace chat and the AI Assistant, the project tree, the
//! indexed-file dialogs, the Settings form, the documentation capture and the Designer's
//! right-click menu. A table of its own, so a plain word such as `Size`, `Type` or `File` there
//! can never turn the Properties pane's row of that name (a property) into another language.
//!
//! The pane has hundreds of them, written as English literals where they are drawn;
//! the rule that every IDE string exists in all six languages is applied here to a set
//! too large to be `Tr` fields, the way `prop_help` applies it to the property
//! explanations. A text is looked up **by its English wording**: [`tx`] answers the
//! translation for the language the pane is drawn in, and the wording itself where the
//! table has none — so a literal that is missing from the table shows in English and is
//! caught by `ui_text_tests`, never lost.
//!
//! What stays as written, by design: the **names of properties and events** (`Width`,
//! `BackgroundColor`, `onClick`) — they are the COBOL names the developer types, and
//! the pane explains each in the IDE's language on hover — control type names, option
//! values that are stored in the form (`Fill`, `Left`), sample data and examples.

use crate::i18n::Language;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One text in the six languages, in [`Language`] order for tables: English, Spanish,
/// Portuguese, Japanese, Chinese, French. The English wording is the key.
pub struct UiText {
    pub text: [&'static str; 6],
}

include!("ui_text_data.rs");
include!("ide_text_data.rs");

fn index() -> &'static HashMap<&'static str, &'static UiText> {
    static INDEX: OnceLock<HashMap<&'static str, &'static UiText>> = OnceLock::new();
    INDEX.get_or_init(|| UI_TEXT.iter().map(|t| (t.text[0], t)).collect())
}

fn ide_index() -> &'static HashMap<&'static str, &'static UiText> {
    static INDEX: OnceLock<HashMap<&'static str, &'static UiText>> = OnceLock::new();
    INDEX.get_or_init(|| IDE_TEXT.iter().map(|t| (t.text[0], t)).collect())
}

fn lang_index(lang: Language) -> usize {
    match lang {
        Language::English => 0,
        Language::Spanish => 1,
        Language::Portuguese => 2,
        Language::Japanese => 3,
        Language::Chinese => 4,
        Language::French => 5,
    }
}

thread_local! {
    /// The language the pane is being drawn in (set once per frame by the pane's entry
    /// points). English until something sets it.
    static CURRENT: Cell<Language> = const { Cell::new(Language::English) };
}

/// Draw the words that follow in `lang`.
pub fn begin(lang: Language) {
    CURRENT.with(|c| c.set(lang));
}

/// The language the pane is being drawn in.
pub fn current() -> Language {
    CURRENT.with(Cell::get)
}

/// `english` in `lang`; `english` itself when the table has no such text.
pub fn localized(lang: Language, english: &str) -> &str {
    index().get(english).map_or(english, |t| t.text[lang_index(lang)])
}

/// `english` in the language the pane is drawn in.
pub fn tx(english: &str) -> &str {
    localized(current(), english)
}

/// `english` in the language the pane is drawn in, looked up under a `context`: the table holds
/// it as `"<context>: <english>"`, so a word that is also a property name elsewhere (`Status`,
/// `Data`) can be a heading here without turning that property's row into another language.
/// English, and any word the table does not have, is `english`.
pub fn txc(context: &str, english: &str) -> String {
    let lang = current();
    if lang == Language::English {
        return english.to_owned();
    }
    index()
        .get(format!("{context}: {english}").as_str())
        .map_or_else(|| english.to_owned(), |t| t.text[lang_index(lang)].to_owned())
}

/// `template` with its `{}` holes filled in order: the translated wording may move them around
/// the sentence, but not drop or add one.
fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut args = args.iter();
    let mut rest = template;
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        match args.next() {
            Some(a) => out.push_str(&a.to_string()),
            None => out.push_str("{}"),
        }
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// `tx` for a text with `{}` holes, filled in order.
pub fn txf(english: &str, args: &[&dyn std::fmt::Display]) -> String {
    fill(tx(english), args)
}

/// `english` — a word of the IDE outside the Properties pane — in `lang`; `english` itself when
/// the IDE table has no such text.
pub fn ide_localized(lang: Language, english: &str) -> &str {
    ide_index().get(english).map_or(english, |t| t.text[lang_index(lang)])
}

/// `english` in the language the IDE is drawn in (see [`begin`], called once per frame).
pub fn ide(english: &str) -> &str {
    ide_localized(current(), english)
}

/// [`ide`] for a text with `{}` holes, filled in order.
pub fn idef(english: &str, args: &[&dyn std::fmt::Display]) -> String {
    fill(ide(english), args)
}

/// Whether the Properties pane's table has `english`.
#[cfg(test)]
pub(crate) fn has(english: &str) -> bool {
    index().contains_key(english)
}

/// Whether the IDE table has `english`.
pub(crate) fn has_ide(english: &str) -> bool {
    ide_index().contains_key(english)
}

/// For the tests that draw a part of the IDE in each language and hold every word of it to
/// the table: what is drawn, and which words come out in English in every language.
#[cfg(test)]
pub(crate) mod probe {
    use super::Language;
    use std::collections::BTreeSet;

    /// Names, values, samples and examples that are the same in every language on purpose.
    const SAME_IN_EVERY_LANGUAGE: &[&str] = &[
        "(Local)",
        "/api/chat (override)",
        "/path/to/image.png",
        "Actual,Budget",
        "FILE SECTION",
        "Liquid Glass",
        "Node 1\n  Child 1\n  Child 2\nNode 2",
        "SALES-AMOUNT,SALES-BUDGET",
        "Sales by Region",
        "Tab1\nTab2",
        "X(256)",
        "You are a helpful assistant.",
        "assets/KB",
        "assets/logo.png",
        "assets/mark.png",
        "/tmp/prc_inspector_dump.txt",
        "data/customers.idx",
        "dd/MM/yyyy HH:mm",
        "http://localhost:11434",
        "https://api.example.com",
        "https://api.example.com/v1/products",
        "sqlite::memory:",
        // A source button: its icon, then the source's name, which is the same word everywhere.
        "(cloud) REST",
        "(db) SQL",
        // The friendly names the binding editor suggests for its sample fields: the field's name, spelt out.
        "Acme Corporation",
        "Category Id",
        "Credit Limit",
        "Customer ID",
        "Customer Id",
        "Customer Name",
        "Customer Tier",
        "Is Active",
        "Region Id",
    ];

    /// A text that is a word of the IDE — not an identifier, a number or a sample.
    pub(crate) fn is_word(t: &str) -> bool {
        let t = t.trim();
        let identifier = t.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && t.trim_end_matches(':').chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        // A COBOL name or an acronym has no lower-case letter; a JSON or JSONPath sample starts with
        // a quote or `$`; a picture is `PIC …` (alone or under its data type).
        !identifier
            && t.chars().filter(|c| c.is_alphabetic()).count() >= 3
            && t.chars().any(char::is_lowercase)
            && !t.starts_with(['#', '[', '"', '$'])
            && !t.ends_with("\",")
            && !t.starts_with("PIC ")
            && !t.contains("\nPIC ")
            && !t.contains("INVOKE ")
            && !SAME_IN_EVERY_LANGUAGE.contains(&t)
    }

    /// Every text painted in `lang` by `draw` — called on a state `setup` makes, once per frame.
    /// Three frames settle the layout; then the pointer sits over the middle of the screen, where a
    /// window anchored at the centre is, and the wheel turns, so a window's rows below its fold are
    /// drawn too. What each frame paints is kept.
    pub(crate) fn texts<S>(
        lang: Language,
        setup: &dyn Fn() -> S,
        draw: &dyn Fn(&mut S, &mut egui::Ui, &crate::i18n::Tr),
    ) -> Vec<String> {
        let tr = &lang.tr();
        let ctx = egui::Context::default();
        crate::i18n::set_language(&ctx, lang);
        // A state that makes words while it is set up (a refusal, say) makes them in this language.
        super::begin(lang);
        let mut state = setup();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 20000.0));
        let mut out = BTreeSet::new();
        for frame in 0..18 {
            let mut events = Vec::new();
            if frame >= 3 {
                if frame == 3 {
                    events.push(egui::Event::PointerMoved(screen.center()));
                }
                events.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -300.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: Default::default(),
                });
            }
            let mut full = ctx.run_ui(
                egui::RawInput { screen_rect: Some(screen), time: Some(f64::from(frame) * 0.1), events, ..Default::default() },
                |ui| {
                    egui::CentralPanel::default().show_inside(ui, |ui| draw(&mut state, ui, tr));
                },
            );
            full.textures_delta.clear();
            fn collect(shape: &egui::Shape, out: &mut BTreeSet<String>) {
                match shape {
                    egui::Shape::Text(t) => {
                        out.insert(t.galley.text().to_owned());
                    }
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
                    _ => {}
                }
            }
            for cs in &full.shapes {
                collect(&cs.shape, &mut out);
            }
        }
        // (The language it was last drawn in is a per-thread setting.)
        super::begin(Language::English);
        out.into_iter().collect()
    }

    /// The words that come out in English in every language.
    pub(crate) fn left_in_english<S>(
        setup: &dyn Fn() -> S,
        draw: &dyn Fn(&mut S, &mut egui::Ui, &crate::i18n::Tr),
    ) -> BTreeSet<String> {
        let mut same: BTreeSet<String> =
            texts(Language::English, setup, draw).into_iter().filter(|t| is_word(t)).collect();
        for &lang in Language::ALL.iter().filter(|l| **l != Language::English) {
            let other: BTreeSet<String> = texts(lang, setup, draw).into_iter().collect();
            same.retain(|t| other.contains(t));
        }
        same
    }
}

#[cfg(test)]
mod ui_text_tests {
    use super::*;

    fn holes(s: &str) -> usize {
        s.matches("{}").count()
    }

    fn has_cjk(s: &str) -> bool {
        s.chars().any(|c| matches!(c as u32, 0x3040..=0x30ff | 0x4e00..=0x9fff | 0xff00..=0xffef))
    }

    /// Every text exists in all six languages, none is empty, the Japanese and the Chinese are
    /// really in those scripts, a text of several words is not just the English copied, and the
    /// holes of a template are the same in every language.
    #[test]
    fn every_text_exists_in_six_languages() {
        let mut bad = Vec::new();
        for (name, table) in [("pane", UI_TEXT), ("ide", IDE_TEXT)] {
            check_table(name, table, &mut bad);
        }
        assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
        println!("ui_text: {} pane texts, {} IDE texts × 6 languages", UI_TEXT.len(), IDE_TEXT.len());
    }

    fn check_table(name: &str, table: &[UiText], bad: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        for t in table {
            let en = t.text[0];
            if !seen.insert(en) {
                bad.push(format!("{name}: {en:?}: listed twice"));
            }
            for (i, text) in t.text.iter().enumerate() {
                if text.trim().is_empty() {
                    bad.push(format!("{name}: {en:?}: language {i} is empty"));
                }
                if holes(text) != holes(en) {
                    bad.push(format!("{name}: {en:?}: language {i} has {} holes, English has {}", holes(text), holes(en)));
                }
            }
            // An acronym ("URL:") is the same in every script.
            let acronym = !en.chars().any(char::is_lowercase);
            if !acronym && (!has_cjk(t.text[3]) || !has_cjk(t.text[4])) {
                bad.push(format!("{name}: {en:?}: the Japanese or the Chinese is not in its script"));
            }
            let words = en.split_whitespace().count();
            let copied = t.text.iter().skip(1).filter(|x| **x == en).count();
            if words >= 3 && copied > 1 {
                bad.push(format!("{name}: {en:?}: looks untranslated ({copied} of 5 copied)"));
            }
        }
    }

    // ── The source scan: no English word reaches a widget outside the tables ────────────────

    /// `code` with its test modules cut out (their lines kept, emptied, so line numbers hold).
    fn production(code: &str) -> String {
        let bytes = code.as_bytes();
        let mut out = String::with_capacity(code.len());
        let mut i = 0;
        while i < bytes.len() {
            // `#[cfg(test)]` … `mod name {` — or any `mod tests {` / `mod …_tests {`.
            let rest = &code[i..];
            let starts_cfg = rest.starts_with("#[cfg(test)]");
            let starts_mod = rest.starts_with("mod ") || rest.starts_with("pub mod ") || rest.starts_with("pub(crate) mod ");
            if starts_cfg || starts_mod {
                let head_end = rest.find(['{', ';']).map(|n| i + n);
                if let Some(h) = head_end.filter(|&h| bytes[h] == b'{') {
                    let head = &code[i..h];
                    let is_mod = head.contains("mod ");
                    let name_is_test = head.split_whitespace().last().is_some_and(|n| n == "tests" || n.ends_with("_tests"));
                    let attr_ok = head.len() < 240 && !head.contains("fn ") && !head.contains('=');
                    if is_mod && attr_ok && (starts_cfg || name_is_test) {
                        // Walk to the matching brace, respecting strings, chars and comments.
                        let (mut depth, mut j) = (0i32, h);
                        while j < bytes.len() {
                            match bytes[j] {
                                b'"' => {
                                    j += 1;
                                    while j < bytes.len() && bytes[j] != b'"' {
                                        if bytes[j] == b'\\' {
                                            j += 1;
                                        }
                                        j += 1;
                                    }
                                }
                                b'/' if bytes.get(j + 1) == Some(&b'/') => {
                                    while j < bytes.len() && bytes[j] != b'\n' {
                                        j += 1;
                                    }
                                    continue;
                                }
                                b'\'' if bytes.get(j + 2) == Some(&b'\'') => j += 2,
                                b'\'' if bytes.get(j + 1) == Some(&b'\\') => {
                                    j += 2;
                                    while j < bytes.len() && bytes[j] != b'\'' {
                                        j += 1;
                                    }
                                }
                                b'{' => depth += 1,
                                b'}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        out.extend(code[i..=j.min(bytes.len() - 1)].chars().filter(|c| *c == '\n'));
                        i = j + 1;
                        continue;
                    }
                }
            }
            let ch = code[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    /// Every call of `name(` in `code` as `(line, arguments)`, each argument as source text.
    fn calls(code: &str, name: &str) -> Vec<(usize, Vec<String>)> {
        let bytes = code.as_bytes();
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(at) = code[from..].find(name) {
            let start = from + at;
            from = start + name.len();
            if !name.starts_with('.') && start > 0 {
                let before = bytes[start - 1] as char;
                if before.is_ascii_alphanumeric() || before == '_' || before == ':' {
                    continue;
                }
            }
            let mut p = from;
            while bytes.get(p).is_some_and(|b| b.is_ascii_whitespace()) {
                p += 1;
            }
            if bytes.get(p) != Some(&b'(') {
                continue;
            }
            let (mut depth, mut i, mut arg_start) = (0usize, p, p + 1);
            let mut args = Vec::new();
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i += 1;
                        while i < bytes.len() && bytes[i] != b'"' {
                            if bytes[i] == b'\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                    }
                    b'\'' if bytes.get(i + 2) == Some(&b'\'') => i += 2,
                    b'\'' if bytes.get(i + 1) == Some(&b'\\') => {
                        i += 2;
                        while i < bytes.len() && bytes[i] != b'\'' {
                            i += 1;
                        }
                    }
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            args.push(code[arg_start..i].trim().to_owned());
                            break;
                        }
                    }
                    b',' if depth == 1 => {
                        args.push(code[arg_start..i].trim().to_owned());
                        arg_start = i + 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            out.push((code[..start].matches('\n').count() + 1, args));
        }
        out
    }

    /// The text of an argument that is a plain string literal (`"…"` or `&"…"`).
    fn literal_text(arg: &str) -> Option<String> {
        let t = arg.trim().trim_start_matches('&').trim();
        let inner = t.strip_prefix('"')?.strip_suffix('"')?;
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                return None; // two literals, or a literal and more
            }
        }
        Some(inner.replace("\\n", "\n").replace("\\\"", "\""))
    }

    /// The `format!` template at the start of an argument, if it is one.
    fn format_template(arg: &str) -> Option<String> {
        let t = arg.trim().trim_start_matches('&').trim().strip_prefix("format!")?;
        let t = t.trim_start().strip_prefix('(')?.trim_start();
        let rest = t.strip_prefix('"')?;
        let mut end = None;
        let mut it = rest.char_indices();
        while let Some((i, c)) = it.next() {
            if c == '\\' {
                it.next();
            } else if c == '"' {
                end = Some(i);
                break;
            }
        }
        Some(rest[..end?].to_owned())
    }

    /// An English word of the IDE, as opposed to an identifier, a number or a symbol.
    fn wants_translation(t: &str) -> bool {
        let letters_run = t
            .as_bytes()
            .windows(3)
            .any(|w| w.iter().all(u8::is_ascii_alphabetic));
        let identifier = t.chars().all(|c| c.is_ascii_alphanumeric() || "_-.#/{}:()[]".contains(c));
        let one_word = t.len() >= 4
            && t.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && t.chars().skip(1).all(|c| c.is_ascii_lowercase());
        (letters_run && !identifier) || one_word
    }

    /// Whether a `format!` template carries an English word of its own, once every `{…}` hole or
    /// named capture is taken out of it.
    fn english_words_in(template: &str) -> bool {
        let (mut outside, mut depth) = (String::new(), 0usize);
        for c in template.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                _ if depth == 0 => outside.push(c),
                _ => {}
            }
        }
        outside.as_bytes().windows(3).any(|w| w.iter().all(u8::is_ascii_alphabetic))
    }

    /// The text a message is made of when it is a string literal (with the conversion it is given
    /// on its way to the Output panel: `.to_owned()`, `.to_string()`, `.into()`) or a `format!`
    /// template. `None` when it is anything else: a variable, a call, a text already translated.
    fn message_text(arg: &str) -> Option<String> {
        let t = arg.trim().trim_start_matches('&').trim();
        if let Some(template) = format_template(t) {
            return Some(template);
        }
        let rest = t.strip_prefix('"')?;
        let mut end = None;
        let mut it = rest.char_indices();
        while let Some((i, c)) = it.next() {
            if c == '\\' {
                it.next();
            } else if c == '"' {
                end = Some(i);
                break;
            }
        }
        let end = end?;
        matches!(rest[end + 1..].trim(), "" | ".to_owned()" | ".to_string()" | ".into()")
            .then(|| rest[..end].to_owned())
    }

    /// Every `<name> = <message>` where the name says it holds a message the person reads (a
    /// status, an error, a notice, a hint …) and the message is a literal or a `format!` template,
    /// as `(line, text)`.
    fn message_assignments(code: &str) -> Vec<(usize, String)> {
        const WORDS: [&str; 7] = ["status", "error", "message", "notice", "warning", "msg", "hint"];
        let bytes = code.as_bytes();
        let mut out = Vec::new();
        for (at, _) in code.match_indices('=') {
            let prev = at.checked_sub(1).map(|i| bytes[i]);
            let next = bytes.get(at + 1).copied();
            if matches!(prev, Some(b'=' | b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/' | b'|' | b'&'))
                || matches!(next, Some(b'=' | b'>'))
            {
                continue;
            }
            let name: String = code[..at]
                .trim_end()
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if !WORDS.iter().any(|w| name.to_ascii_lowercase().contains(w)) {
                continue;
            }
            let line_start = code[..at].rfind('\n').map_or(0, |i| i + 1);
            if code[line_start..].trim_start().starts_with("//") {
                continue;
            }
            let Some(end) = code[at..].find(';') else { continue };
            let mut rhs = code[at + 1..at + end].trim();
            if let Some(inner) = rhs.strip_prefix("Some(").and_then(|r| r.trim_end().strip_suffix(')')) {
                rhs = inner;
            }
            if let Some(text) = message_text(rhs) {
                out.push((code[..at].matches('\n').count() + 1, text));
            }
        }
        out
    }

    /// The calls whose argument at `index` is text the person reads.
    const TEXT_CALLS: &[(&str, usize)] = &[
        (".weak", 0), (".strong", 0), (".small", 0), (".monospace", 0), (".heading", 0), (".label", 0),
        (".colored_label", 1), (".button", 0), (".small_button", 0), (".selectable_label", 1),
        (".checkbox", 1), (".radio_value", 2), (".selectable_value", 2), (".toggle_value", 1),
        (".hyperlink_to", 0), (".link", 0), (".menu_button", 0), (".collapsing", 0),
        ("CollapsingHeader::new", 0), ("Window::new", 0), (".on_hover_text", 0),
        (".on_disabled_hover_text", 0), (".hint_text", 0), ("RichText::new", 0), ("Label::new", 0),
        ("Button::new", 0), (".text", 2), (".title", 0), (".selected_text", 0),
        ("ComboBox::from_label", 0), (".radio", 1), ("icon_btn", 3),
    ];

    /// Literals that are not words of the IDE: a shortcut, COBOL the editor shows, a legal line,
    /// a product's name, a marker.
    const NOT_TRANSLATED: &[&str] = &[
        "Command+F",
        "       IDENTIFICATION DIVISION.",
        "           GOBACK.",
        "© 2026 Emerson Lopes and PowerRustCOBOL contributors",
        "👑 Grace",
        "▦ ARRAY",
    ];

    /// `format!` templates handed to a widget that carry no English word of their own: COBOL the
    /// editor shows, names and numbers, a process line.
    const TEMPLATES_WITHOUT_WORDS: &[&str] = &[
        "       PROGRAM-ID. {}.",
        "       END PROGRAM {}.",
        "- {item}",
        "{name} = {value}",
        "{} — Markdown",
        "{indent}{branch}{name}  ·  pid {pid}  ·  CPU {cpu:.0}%  ·  RSS {mb:.0} MB",
        "{} agentic_ai/{}/{}_prompt.md",
        "agentic_ai/{n}/\\n├── {n}_prompt.md\\n├── steering/\\n├── policies.md\\n├── skills/\\n├── mcp.json\\n├── knowledge/\\n└── agent.json",
    ];

    /// The keys that reach `ide()` through a variable (a table of names and descriptions, a
    /// helper's parameter): held to the table here, since no call site shows them.
    const KEYS_REACHED_THROUGH_VARIABLES: &[&str] = &[
        "Provider", "Model", "Subscription", "Input tokens", "Output tokens", "Tokenizer", "Max out",
        "Best at", "Watch", "Failure pattern",
        "No structured strengths were returned.", "No structured weaknesses were returned.",
        "No structured failure patterns were returned.",
        "Process CPU", "Memory (RSS)", "Child procs", "System Mem",
        "Program structure", "Data descriptions", "Control flow", "File handling", "Forms/extensions",
        "Avoid unsupported", "Code preservation", "Formatting", "COBOL-85 coverage",
        "PowerRustCOBOL coverage", "Unsupported avoided",
        "Keeps COBOL divisions and program shape valid.", "Uses PIC, WS, FD, and storage items correctly.",
        "Builds valid decisions, loops, and handler flow.", "Handles indexed-file and record operations safely.",
        "Uses controls, events, properties, and methods.", "Stays inside implemented PowerRustCOBOL features.",
        "Edits without deleting required existing code.", "Preserves readable COBOL layout and spacing.",
        "Understands supported COBOL-85 syntax and patterns.", "Understands PowerRustCOBOL GUI extensions.",
        "Avoids invented APIs and unsupported syntax.", "Benchmark score for this capability.",
        "Structure", "Data", "Flow", "Files", "Forms", "Supported", "Preserve", "Format", "Score",
        "Still", "Still, after 3 seconds", "Recording",
        "One photograph of this window, taken as soon as this chooser is out of the frame.",
        "Long enough to grab the mouse and hold a menu open.",
        "Then click the window you want to record — that click starts it, and F12 stops it.",
        "Step 1: Set File Properties", "Step 2: Edit Record Structure (COBOL-85)",
        "👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator",
    ];

    /// The sinks a message reaches the person through: the Output panel, an alert, a form's error
    /// line, a status line. The index is the argument that holds the text.
    const STATUS_CALLS: &[(&str, usize)] = &[
        (".push_status", 0), (".push_line", 0), (".push_ai_line", 1), (".set_form_error", 0),
        (".set_form_error_with_links", 0), (".set_alert_error", 0), (".set_llm_test_error", 0),
        (".set_indent_error", 0), (".set_status", 0), (".show_error", 1), (".notify", 0),
    ];

    /// Files whose messages are made on a worker thread, where the frame's language is not set,
    /// or that are the AI trace log: a translation there would be English anyway, and a trace is
    /// read by the person who debugs the IDE, not by the one who builds with it.
    const MESSAGES_WITHOUT_A_FRAME: &[&str] = &["grace_host.rs", "llm.rs", "model_policy.rs"];

    /// Messages that are a protocol line or a diagnostic for whoever debugs the IDE.
    const MESSAGES_NOT_TRANSLATED: &[&str] = &["debug: bad @DBG line: {json}"];

    fn source_files() -> Vec<(String, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            for e in std::fs::read_dir(dir).expect("src is readable").flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut paths = Vec::new();
        walk(&root, &mut paths);
        paths.sort();
        paths
            .into_iter()
            .filter(|p| {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                // The tables, the translated Tr, the pre-translated tooltips and the Properties
                // pane (its own test holds it) are not scanned; test files are not the product.
                !matches!(name, "ui_text.rs" | "ui_text_data.rs" | "ide_text_data.rs" | "i18n.rs" | "prop_help_data.rs" | "properties.rs")
                    && !name.ends_with("_tests.rs")
            })
            .map(|p| {
                let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
                (rel, production(&std::fs::read_to_string(&p).expect("source is readable")))
            })
            .collect()
    }

    /// No English word of the IDE reaches a widget without the table: not a literal handed to a
    /// label, a button, a heading, a hover text, a window title or a toolbar tooltip; not a
    /// `format!` template with words in it; and every key given to `ide()` / `idef()` is in the
    /// table. (The Properties pane is held by its own test.)
    #[test]
    fn no_english_word_reaches_a_widget_outside_the_tables_anywhere_in_the_ide() {
        let mut bare = std::collections::BTreeSet::new();
        let mut missing = std::collections::BTreeSet::new();
        let mut keys = 0usize;
        for (file, code) in source_files() {
            for &(name, index) in TEXT_CALLS {
                for (line, args) in calls(&code, name) {
                    let Some(arg) = args.get(index) else { continue };
                    if let Some(text) = literal_text(arg) {
                        if wants_translation(&text) && !NOT_TRANSLATED.contains(&text.as_str()) {
                            bare.insert(format!("{file}:{line}: {name}({text:?})"));
                        }
                    } else if let Some(template) = format_template(arg) {
                        if english_words_in(&template) && !TEMPLATES_WITHOUT_WORDS.contains(&template.as_str()) {
                            bare.insert(format!("{file}:{line}: {name}(format!({template:?}))"));
                        }
                    }
                }
            }
            for name in ["crate::ui_text::ide", "crate::ui_text::idef"] {
                for (line, args) in calls(&code, name) {
                    if let Some(text) = args.first().and_then(|a| literal_text(a)) {
                        keys += 1;
                        if !has_ide(&text) {
                            missing.insert(format!("{file}:{line}: {text:?}"));
                        }
                    }
                }
            }
        }
        for key in KEYS_REACHED_THROUGH_VARIABLES {
            if !has_ide(key) {
                missing.insert(format!("(through a variable): {key:?}"));
            }
        }
        for b in &bare {
            println!("English literal reaches a widget -> {b}");
        }
        for m in &missing {
            println!("not in the IDE table -> {m}");
        }
        println!("{keys} ide()/idef() keys checked against {} table entries", IDE_TEXT.len());
        assert!(bare.is_empty(), "{} English literals go straight to a widget", bare.len());
        assert!(missing.is_empty(), "{} keys are not in the IDE table", missing.len());
    }

    /// No English message reaches the Output panel, an alert, a form's error line or a status line
    /// without the table: not a literal, not a `format!` template with words in it, not a message
    /// assigned to a status or error field. (A message made on a worker thread is not held: the
    /// frame's language is not set there.)
    #[test]
    fn no_english_message_reaches_the_output_panel_or_an_alert_anywhere_in_the_ide() {
        let mut bare = std::collections::BTreeSet::new();
        let mut held = 0usize;
        for (file, code) in source_files() {
            if MESSAGES_WITHOUT_A_FRAME.iter().any(|f| file.ends_with(f)) {
                continue;
            }
            let mut check = |line: usize, how: &str, text: String| {
                held += 1;
                if english_words_in(&text) && !MESSAGES_NOT_TRANSLATED.contains(&text.as_str()) {
                    bare.insert(format!("{file}:{line}: {how}({text:?})"));
                }
            };
            for &(name, index) in STATUS_CALLS {
                for (line, args) in calls(&code, name) {
                    if let Some(text) = args.get(index).and_then(|a| message_text(a)) {
                        check(line, name, text);
                    }
                }
            }
            for (line, text) in message_assignments(&code) {
                check(line, "=", text);
            }
        }
        for b in &bare {
            println!("English message reaches the person -> {b}");
        }
        println!("{held} literal or format! messages checked");
        assert!(bare.is_empty(), "{} English messages go straight to the person", bare.len());
    }

    #[test]
    fn a_text_is_found_by_its_english_wording_and_a_template_is_filled_in_order() {
        assert_eq!(localized(Language::Spanish, "Corner radius"), "Radio de esquina");
        assert_eq!(localized(Language::English, "Corner radius"), "Corner radius");
        assert_eq!(localized(Language::French, "no such text in the table"), "no such text in the table");
        begin(Language::Spanish);
        assert_eq!(txf("{} columns, {} frozen column(s), {} frozen row(s)", &[&3, &1, &0]), "3 columnas, 1 columna(s) inmovilizada(s), 0 fila(s) inmovilizada(s)");
        begin(Language::English);
    }

    /// The status messages read as sentences in each language, including the ones that are built
    /// from a verb of their own ("Cannot {}: …") and the ones that name the project's own files.
    #[test]
    fn status_messages_read_as_sentences_in_each_language() {
        let cases: &[(Language, String, &str)] = &[
            (Language::Spanish, idef_in(Language::Spanish, "Project saved → {}", &[&"a.toml"]), "Proyecto guardado → a.toml"),
            (Language::Portuguese, idef_in(Language::Portuguese, "Deleted form {}", &[&"F1"]), "Formulário excluído: F1"),
            (Language::Japanese, idef_in(Language::Japanese, "Packaged {} files → {}", &[&3, &"bin"]), "3 個のファイルをパッケージ化しました → bin"),
            (Language::Chinese, idef_in(Language::Chinese, "Could not open {}", &[&"x.cbl"]), "无法打开 x.cbl"),
            (Language::French, idef_in(Language::French, "Created project '{}'", &[&"P"]), "Projet 'P' créé"),
        ];
        for (lang, got, want) in cases {
            assert_eq!(got, want, "{lang:?}");
        }
        // A sentence built around a verb of its own: the verb is the table's, the frame is too.
        let composed = |lang: Language, verb: &str| {
            begin(lang);
            let v = ide(verb);
            let sentence = idef("Cannot {}: form COBOL ID '{}' is already used by {}.", &[&v, &"FRM-A", &"b.cfrm"]);
            begin(Language::English);
            sentence
        };
        assert_eq!(
            composed(Language::English, "create form"),
            "Cannot create form: form COBOL ID 'FRM-A' is already used by b.cfrm."
        );
        assert_eq!(
            composed(Language::Spanish, "import form"),
            "No se puede importar el formulario: el ID COBOL de formulario 'FRM-A' ya lo usa b.cfrm."
        );
        assert_eq!(
            composed(Language::Portuguese, "save form"),
            "Não é possível salvar o formulário: o ID COBOL de formulário 'FRM-A' já é usado por b.cfrm."
        );
        assert_eq!(
            composed(Language::Japanese, "create form"),
            "フォームを作成できません: フォームの COBOL ID 'FRM-A' は b.cfrm で既に使われています。"
        );
        assert_eq!(
            composed(Language::Chinese, "save form"),
            "无法保存窗体：窗体 COBOL ID 'FRM-A' 已被 b.cfrm 使用。"
        );
        assert_eq!(
            composed(Language::French, "import form"),
            "Importation du formulaire impossible : l'ID COBOL de formulaire 'FRM-A' est déjà utilisé par b.cfrm."
        );
        println!("5 status messages and 6 composed sentences read correctly");
    }

    fn idef_in(lang: Language, key: &str, args: &[&dyn std::fmt::Display]) -> String {
        begin(lang);
        let s = idef(key, args);
        begin(Language::English);
        s
    }

    /// The IDE table answers in the language the frame was begun in, fills its holes in order,
    /// and stays English where the table has no such text.
    #[test]
    fn the_ide_table_answers_in_the_language_of_the_frame() {
        begin(Language::English);
        assert_eq!(ide("Zoom in"), "Zoom in");
        assert_eq!(idef("Frame {}/{}", &[&2, &5]), "Frame 2/5");
        begin(Language::Spanish);
        assert_eq!(ide("Zoom in"), "Acercar");
        assert_eq!(idef("Frame {}/{}", &[&2, &5]), "Fotograma 2/5");
        assert_eq!(idef("Version {}", &[&"1.2"]), "Versión 1.2");
        begin(Language::Japanese);
        assert_eq!(ide("Close"), "閉じる");
        assert_eq!(idef("line {}:{}", &[&3, &14]), "行 3:14");
        begin(Language::French);
        assert_eq!(ide("no such text in the IDE table"), "no such text in the IDE table");
        // A word that is also a property name is not in the pane's table by this route.
        assert_eq!(tx("Zoom in"), "Zoom in", "the two tables are separate");
        begin(Language::English);
    }
}

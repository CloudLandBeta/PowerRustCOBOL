// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The stylesheet of a web or mobile form, in the IDE (WASM-007 R5–R7): a modal window with a code
//! editor (line numbers, CSS colouring), the findings of `cobolt_forms::stylesheet_lint` in the
//! developer's language, and **Apply**, **Cancel**, **Revert to theme default** and **Undo last apply**.
//!
//! Nothing reaches the disk before Apply: Revert replaces the editor's text only, asks first, and is
//! undone by Ctrl+Z or by Cancel (R6, user code is sacred). The window owns its size; the grip is the
//! only writer of it (R7, `leaderboard_modal.rs`), and inside it a bottom panel and a central panel
//! partition the box exactly, so no height is estimated.

use std::path::PathBuf;

use cobolt_forms::stylesheet_lint::{Code, Diagnostic, Severity};

use crate::i18n::Tr;
use crate::theme::Theme;

/// Opening size; from then on the size is whatever the developer dragged it to.
pub const DEFAULT_W: f32 = 880.0;
pub const DEFAULT_H: f32 = 600.0;
const MIN_W: f32 = 520.0;
const MIN_H: f32 = 320.0;
const MAX_W: f32 = 2400.0;
const MAX_H: f32 = 1600.0;
const GRIP: f32 = 14.0;
const GRIP_INSET: f32 = 3.0;
/// The findings list's height: fixed, it scrolls.
const FINDINGS_H: f32 = 96.0;
const FONT: f32 = 13.0;
pub const WINDOW_ID: &str = "stylesheet_modal";

/// What a piece of a stylesheet is, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Comment,
    Str,
    AtRule,
    Selector,
    Property,
    Value,
    Punct,
}

/// The first of `;`, `{`, `}` from `i` on, outside comments and strings.
fn next_delimiter(b: &[u8], mut i: usize) -> Option<u8> {
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'*') => i = find(b, i + 2, b"*/").map_or(b.len(), |e| e + 2),
            q @ (b'"' | b'\'') => i = string_end(b, i, q),
            c @ (b';' | b'{' | b'}') => return Some(c),
            _ => i += 1,
        }
    }
    None
}

fn find(b: &[u8], from: usize, what: &[u8]) -> Option<usize> {
    (from..b.len().saturating_sub(what.len() - 1)).find(|&i| &b[i..i + what.len()] == what)
}

/// The end of a string opened at `i` (after its closing quote, or at the end of its line).
fn string_end(b: &[u8], i: usize, q: u8) -> usize {
    let mut j = i + 1;
    while j < b.len() && b[j] != q && b[j] != b'\n' {
        j += if b[j] == b'\\' { 2 } else { 1 };
    }
    (j + 1).min(b.len())
}

/// The pieces of a stylesheet, covering every byte, in order: comments, strings, at-rule names,
/// selectors, property names, values and punctuation. A segment followed by `{` is a selector (or an
/// at-rule's prelude); one followed by `;` or `}` inside a block is a declaration.
pub fn tokens(css: &str) -> Vec<(std::ops::Range<usize>, Tok)> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Start,
        Prelude { at_word: bool },
        Decl { colon: bool },
        AtStatement { at_word: bool },
    }
    let b = css.as_bytes();
    let mut out: Vec<(std::ops::Range<usize>, Tok)> = Vec::new();
    let mut push = |r: std::ops::Range<usize>, t: Tok| {
        if let Some((last, lt)) = out.last_mut() {
            if *lt == t && last.end == r.start {
                last.end = r.end;
                return;
            }
        }
        out.push((r, t));
    };
    let (mut i, mut depth, mut mode) = (0usize, 0usize, Mode::Start);
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = find(b, i + 2, b"*/").map_or(b.len(), |e| e + 2);
            push(i..end, Tok::Comment);
            i = end;
            continue;
        }
        if c == b'"' || c == b'\'' {
            let end = string_end(b, i, c);
            push(i..end, Tok::Str);
            i = end;
            continue;
        }
        match c {
            b'{' | b'}' | b';' => {
                if c == b'{' {
                    depth += 1;
                } else if c == b'}' {
                    depth = depth.saturating_sub(1);
                }
                push(i..i + 1, Tok::Punct);
                mode = Mode::Start;
                i += 1;
                continue;
            }
            _ => {}
        }
        if mode == Mode::Start && !c.is_ascii_whitespace() {
            mode = match (next_delimiter(b, i), c == b'@') {
                (Some(b'{'), at) => Mode::Prelude { at_word: at },
                (_, true) if depth == 0 => Mode::AtStatement { at_word: true },
                _ if depth == 0 => Mode::Prelude { at_word: false },
                _ => Mode::Decl { colon: false },
            };
        }
        // One character (a whole UTF-8 sequence) at a time.
        let len = css[i..].chars().next().map_or(1, char::len_utf8);
        let tok = match &mut mode {
            Mode::Start => Tok::Punct,
            Mode::Prelude { at_word } | Mode::AtStatement { at_word } if *at_word => {
                if c.is_ascii_whitespace() || c == b'(' {
                    *at_word = false;
                    Tok::Selector
                } else {
                    Tok::AtRule
                }
            }
            Mode::Prelude { .. } => Tok::Selector,
            Mode::AtStatement { .. } => Tok::Value,
            Mode::Decl { colon } => {
                if !*colon && c == b':' {
                    *colon = true;
                    Tok::Punct
                } else if *colon {
                    Tok::Value
                } else {
                    Tok::Property
                }
            }
        };
        push(i..i + len, tok);
        i += len;
    }
    out
}

fn colour(theme: &Theme, t: Tok) -> egui::Color32 {
    match t {
        Tok::Comment => theme.ed_comment,
        Tok::Str => theme.ed_string,
        Tok::AtRule => theme.ed_keyword,
        Tok::Selector => theme.ed_paragraph,
        Tok::Property => theme.ed_data,
        Tok::Value | Tok::Punct => theme.ed_plain,
    }
}

/// A question the modal is asking before it does something the developer may regret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Replace the editor's text with the theme's generated stylesheet?
    Revert,
    /// Close and lose the changes?
    Discard,
}

/// What the modal asks its host to do this frame.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StylesheetAction {
    /// Write this text to this file, as bytes.
    pub write: Option<(PathBuf, String)>,
    /// The modal closed.
    pub closed: bool,
}

pub struct StylesheetModal {
    /// The form's stylesheet file (`<forms folder>/<Form>.css`).
    pub sheet: PathBuf,
    /// The form, for the title.
    form: String,
    /// The form's layers, for the layer-guard finding (R16).
    layers: Vec<String>,
    /// What the file holds (`None`: there is no file yet).
    saved: Option<String>,
    /// The editor's text.
    pub buf: String,
    /// The stylesheet generated from the form's theme (Revert's text).
    generated: String,
    /// The text Revert replaced, until it is edited or undone.
    before_revert: Option<String>,
    /// The file's text before the last Apply, for this session.
    last_apply: Option<String>,
    pub ask: Option<Ask>,
    /// The window's size, owned here; the grip is its only writer.
    pub size: egui::Vec2,
    /// A line to bring into view and put the cursor on (1-based).
    jump_to: Option<usize>,
    /// The findings of the text they were found in; the check runs again only when it changes.
    found: Option<(String, Vec<Diagnostic>)>,
}

impl StylesheetModal {
    /// Open on a form's sheet: the file's text, or the generated one when there is no file yet.
    pub fn new(sheet: PathBuf, form: &str, layers: Vec<String>, saved: Option<String>, generated: String) -> Self {
        Self {
            sheet,
            form: form.to_owned(),
            layers,
            buf: saved.clone().unwrap_or_else(|| generated.clone()),
            saved,
            generated,
            before_revert: None,
            last_apply: None,
            ask: None,
            size: egui::vec2(DEFAULT_W, DEFAULT_H),
            jump_to: None,
            found: None,
        }
    }

    /// Has the editor's text left what the file holds?
    pub fn changed(&self) -> bool {
        self.saved.as_deref() != Some(self.buf.as_str())
    }

    /// The findings of the editor's text.
    pub fn findings(&self) -> Vec<Diagnostic> {
        let opts = cobolt_forms::stylesheet_lint::Options { layers: &self.layers, ..Default::default() };
        cobolt_forms::stylesheet_lint::lint(&self.buf, &opts)
    }

    /// Apply: write the editor's text; the file's previous text becomes "Undo last apply".
    pub fn press_apply(&mut self) -> Option<(PathBuf, String)> {
        if !self.changed() {
            return None;
        }
        self.last_apply = self.saved.replace(self.buf.clone());
        self.before_revert = None;
        Some((self.sheet.clone(), self.buf.clone()))
    }

    /// Undo last apply: write back what the file held before it.
    pub fn press_undo_apply(&mut self) -> Option<(PathBuf, String)> {
        let previous = self.last_apply.take()?;
        self.last_apply = self.saved.replace(previous.clone());
        self.buf = previous.clone();
        Some((self.sheet.clone(), previous))
    }

    /// Cancel: close, or ask first when the text changed.
    pub fn press_cancel(&mut self) -> bool {
        if self.changed() {
            self.ask = Some(Ask::Discard);
            false
        } else {
            true
        }
    }

    pub fn press_revert(&mut self) {
        self.ask = Some(Ask::Revert);
    }

    /// The answer to [`Self::ask`]; `true` when the modal closes.
    pub fn answer(&mut self, yes: bool) -> bool {
        match self.ask.take() {
            Some(Ask::Revert) if yes => {
                self.before_revert = Some(std::mem::replace(&mut self.buf, self.generated.clone()));
                false
            }
            Some(Ask::Discard) => yes,
            _ => false,
        }
    }

    /// Ctrl+Z right after a Revert: the developer's text back. Once the reverted text is edited, the
    /// editor's own undo takes over.
    pub fn undo_revert(&mut self) -> bool {
        match self.before_revert.take() {
            Some(t) if self.buf == self.generated => {
                self.buf = t;
                true
            }
            _ => false,
        }
    }

    /// What the grip does with its drag.
    pub fn drag_by(&mut self, delta: egui::Vec2) {
        self.size += delta;
        self.size.x = self.size.x.clamp(MIN_W, MAX_W);
        self.size.y = self.size.y.clamp(MIN_H, MAX_H);
    }

    fn resize_grip(&mut self, ctx: &egui::Context, theme: &Theme, window: egui::Rect) {
        let origin = window.max - egui::vec2(GRIP_INSET + GRIP, GRIP_INSET + GRIP);
        egui::Area::new(egui::Id::new("stylesheet_modal_grip")).order(egui::Order::Foreground).fixed_pos(origin).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(GRIP, GRIP), egui::Sense::drag());
            if response.dragged() {
                self.drag_by(response.drag_delta());
            }
            let hot = response.hovered() || response.dragged();
            if hot {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNorthWest);
            }
            let stroke = egui::Stroke::new(1.2, if hot { theme.accent } else { theme.text_dim });
            for offset in [2.0_f32, 6.0, 10.0] {
                let len = GRIP - offset - 1.0;
                ui.painter().line_segment([egui::pos2(rect.right() - len, rect.bottom()), egui::pos2(rect.right(), rect.bottom() - len)], stroke);
            }
        });
    }

    pub fn show(&mut self, ctx: &egui::Context, tr: &Tr) -> StylesheetAction {
        let theme = &crate::theme::active();
        let mut action = StylesheetAction::default();
        let title = tr.stylesheet_title.replacen("{}", &self.form, 1);
        if self.found.as_ref().is_none_or(|(text, _)| *text != self.buf) {
            self.found = Some((self.buf.clone(), self.findings()));
        }
        let findings = self.found.as_ref().map(|(_, f)| f.clone()).unwrap_or_default();
        let window = egui::Window::new(egui::RichText::new(title).strong())
            .id(egui::Id::new(WINDOW_ID))
            .collapsible(false)
            // Never `resizable(true)`: the size is ours and only the grip changes it (R7).
            .resizable(false)
            .fixed_size(self.size)
            .default_pos([60.0, 60.0])
            .show(ctx, |ui| {
                let margin = ui.style().spacing.window_margin.sum().x;
                let stroke = 2.0 * ui.style().visuals.window_stroke.width;
                let width = self.size.x - margin - stroke;
                ui.set_width(width);
                egui::Panel::bottom(egui::Id::new("stylesheet_modal_footer"))
                    .resizable(false)
                    .show_separator_line(false)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.footer(ui, &findings, tr, &mut action));
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| self.editor(ui, theme));
            });
        if let Some(w) = window {
            self.resize_grip(ctx, theme, w.response.rect);
        }
        action
    }

    fn footer(&mut self, ui: &mut egui::Ui, findings: &[Diagnostic], tr: &Tr, action: &mut StylesheetAction) {
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("stylesheet_modal_findings")
            .auto_shrink([false, false])
            .min_scrolled_height(FINDINGS_H)
            .max_height(FINDINGS_H)
            .show(ui, |ui| {
                if findings.is_empty() {
                    ui.label(egui::RichText::new(tr.css_no_findings).small());
                }
                for d in findings {
                    let colour = match d.severity() {
                        Severity::Error => egui::Color32::from_rgb(0xE5, 0x48, 0x4D),
                        Severity::Warning => egui::Color32::from_rgb(0xD9, 0x9A, 0x2B),
                    };
                    let text = egui::RichText::new(lint_line(tr, d)).small().color(colour);
                    if ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_text(tr.css_findings_hint).clicked() {
                        self.jump_to = Some(d.line);
                    }
                }
            });
        ui.add_space(6.0);
        match self.ask {
            Some(ask) => {
                ui.label(match ask {
                    Ask::Revert => tr.css_revert_confirm,
                    Ask::Discard => tr.css_discard_confirm,
                });
                ui.horizontal(|ui| {
                    let (no, yes) = match ask {
                        Ask::Revert => (tr.btn_cancel, tr.btn_css_revert_ok),
                        Ask::Discard => (tr.btn_css_keep_editing, tr.btn_css_discard),
                    };
                    if ui.button(no).clicked() {
                        self.answer(false);
                    }
                    if ui.button(yes).clicked() && self.answer(true) {
                        action.closed = true;
                    }
                });
            }
            None => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.changed(), egui::Button::new(tr.btn_css_apply)).clicked() {
                        action.write = self.press_apply();
                    }
                    if ui.add_enabled(self.last_apply.is_some(), egui::Button::new(tr.btn_css_undo_apply)).clicked() {
                        action.write = self.press_undo_apply();
                    }
                    if ui.button(tr.btn_css_revert).clicked() {
                        self.press_revert();
                    }
                    if ui.button(tr.btn_cancel).clicked() && self.press_cancel() {
                        action.closed = true;
                    }
                });
            }
        }
        ui.add_space(GRIP);
    }

    fn editor(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        // Ctrl+Z (not Ctrl+Shift+Z) right after a Revert gives the developer's text back.
        if self.before_revert.is_some() && ui.input(|i| !i.modifiers.shift) && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo_revert();
        }
        let font = egui::FontId::monospace(FONT);
        let lines = self.buf.split('\n').count().max(1);
        let numbers: String = (1..=lines).map(|n| format!("{n:>4}")).collect::<Vec<_>>().join("\n");
        let edit_id = egui::Id::new("stylesheet_modal_editor");
        if let Some(line) = self.jump_to {
            let at: usize = self.buf.split('\n').take(line.saturating_sub(1)).map(|l| l.chars().count() + 1).sum();
            let mut state = egui::text_edit::TextEditState::load(ui.ctx(), edit_id).unwrap_or_default();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(at))));
            state.store(ui.ctx(), edit_id);
            ui.memory_mut(|m| m.request_focus(edit_id));
        }
        let theme_for_layout = *theme;
        let layout_font = font.clone();
        let mut layouter = move |ui: &egui::Ui, buf: &dyn egui::TextBuffer, _wrap: f32| {
            let text = buf.as_str();
            let mut job = egui::text::LayoutJob::default();
            for (r, t) in tokens(text) {
                job.append(&text[r], 0.0, egui::TextFormat::simple(layout_font.clone(), colour(&theme_for_layout, t)));
            }
            // No wrapping: a line of the editor is a line of the file, as the numbers say.
            job.wrap.max_width = f32::INFINITY;
            ui.ctx().fonts_mut(|f| f.layout_job(job))
        };
        let jump = self.jump_to.take();
        egui::ScrollArea::both().id_salt("stylesheet_modal_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(numbers).font(font.clone()).color(theme.text_dim)).selectable(false));
                let out = egui::TextEdit::multiline(&mut self.buf)
                    .id(edit_id)
                    .font(font.clone())
                    .code_editor()
                    .desired_rows(lines)
                    .desired_width(f32::INFINITY)
                    .lock_focus(true)
                    .layouter(&mut layouter)
                    .show(ui);
                if let (Some(_), Some(range)) = (jump, out.cursor_range) {
                    let rect = out.galley.pos_from_cursor(range.primary).translate(out.galley_pos.to_vec2());
                    ui.scroll_to_rect(rect, Some(egui::Align::Center));
                }
                if out.response.changed() {
                    self.before_revert = self.before_revert.take().filter(|_| self.buf == self.generated);
                }
            });
        });
    }
}

/// The text of a finding's code in `tr`, with `{}` where its arguments go, in the order
/// [`lint_message`] fills them.
pub fn code_text(tr: &Tr, code: Code) -> &'static str {
    match code {
        Code::UnbalancedBrace => tr.css_lint_unbalanced_brace,
        Code::UnterminatedString => tr.css_lint_unterminated_string,
        Code::UnterminatedComment => tr.css_lint_unterminated_comment,
        Code::NotADeclaration => tr.css_lint_not_a_declaration,
        Code::UnknownProperty => tr.css_lint_unknown_property,
        Code::BadValue => tr.css_lint_bad_value,
        Code::Unsupported => tr.css_lint_unsupported,
        Code::Refused => tr.css_lint_refused,
        Code::RemoteUrl => tr.css_lint_remote_url,
        Code::LayerRule => tr.css_lint_layer_rule,
    }
}

/// A finding in the developer's language, without its line.
pub fn lint_message(tr: &Tr, d: &Diagnostic) -> String {
    let a = |i: usize| d.args.get(i).map(String::as_str).unwrap_or("");
    // `BadValue`'s arguments are (property, value); its sentence names the value first.
    let args: Vec<&str> = match d.code {
        Code::BadValue => vec![a(1), a(0)],
        _ => d.args.iter().map(String::as_str).collect(),
    };
    let mut out = code_text(tr, d.code).to_owned();
    for x in args {
        if out.contains("{}") {
            out = out.replacen("{}", x, 1);
        }
    }
    out
}

/// A finding with its line: "line 3: …".
pub fn lint_line(tr: &Tr, d: &Diagnostic) -> String {
    tr.css_lint_line.replacen("{}", &d.line.to_string(), 1).replacen("{}", &lint_message(tr, d), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    /// WASM-007 T24: every finding the check can return reads as a sentence in each language,
    /// with a hole for each argument it has.
    #[test]
    fn every_lint_code_has_a_message_in_every_language() {
        let holes = |c: Code| match c {
            Code::UnbalancedBrace | Code::UnterminatedString | Code::UnterminatedComment => 0,
            Code::NotADeclaration | Code::UnknownProperty | Code::Refused | Code::RemoteUrl => 1,
            Code::BadValue | Code::Unsupported | Code::LayerRule => 2,
        };
        for lang in Language::ALL {
            let tr = lang.tr();
            for c in Code::ALL {
                let t = code_text(&tr, c);
                assert!(!t.trim().is_empty(), "{lang:?} {c:?}");
                assert_eq!(t.matches("{}").count(), holes(c), "{lang:?} {c:?}: {t}");
            }
            assert_eq!(tr.css_lint_line.matches("{}").count(), 2, "{lang:?}");
        }
        let tr = Language::English.tr();
        let d = Diagnostic { code: Code::BadValue, line: 3, args: vec!["color".into(), "reddish".into()] };
        assert_eq!(lint_line(&tr, &d), "line 3: `reddish` is not a valid value for `color`");
        assert_eq!(lint_message(&tr, &d), cobolt_forms::stylesheet_lint::message(&d), "the English text is the lint's own");
    }

    fn modal(saved: Option<&str>) -> StylesheetModal {
        StylesheetModal::new(PathBuf::from("F.css"), "F", vec!["LAYER-1".into()], saved.map(str::to_owned), "/* generated */\n".into())
    }

    #[test]
    fn the_tokens_cover_every_byte_and_tell_selectors_properties_and_values_apart() {
        let css = "/* é */\n@layer pr-app;\n@media (width > 600px) {\n  #BTN-1:hover, .a { color: red; background: url(\"x;y\") }\n}\n";
        let toks = tokens(css);
        let mut at = 0;
        for (r, _) in &toks {
            assert_eq!(r.start, at, "no gap: {toks:?}");
            at = r.end;
        }
        assert_eq!(at, css.len());
        let of = |piece: &str| {
            let i = css.find(piece).unwrap();
            toks.iter().find(|(r, _)| r.contains(&i)).unwrap().1
        };
        assert_eq!(of("/* é"), Tok::Comment);
        assert_eq!(of("@layer"), Tok::AtRule);
        assert_eq!(of("pr-app"), Tok::Value);
        assert_eq!(of("@media"), Tok::AtRule);
        assert_eq!(of("(width"), Tok::Selector);
        assert_eq!(of("#BTN-1"), Tok::Selector);
        assert_eq!(of("color"), Tok::Property);
        assert_eq!(of("red"), Tok::Value);
        assert_eq!(of("background"), Tok::Property, "the `;` inside the string does not end the declaration early");
        assert_eq!(of("\"x;y\""), Tok::Str);
    }

    #[test]
    fn apply_writes_cancel_asks_and_revert_is_undone_without_touching_the_disk() {
        // A new sheet opens on the generated text; Apply is what creates the file.
        let mut m = modal(None);
        assert!(m.changed(), "nothing on disk yet");
        assert_eq!(m.press_apply(), Some((PathBuf::from("F.css"), "/* generated */\n".into())));
        assert!(!m.changed() && m.press_apply().is_none(), "nothing more to write");

        let mut m = modal(Some(".a { color: red }\r\n"));
        assert!(m.press_cancel(), "unchanged: Cancel just closes");
        m.buf.push_str(".b { color: blue }\r\n");
        assert!(!m.press_cancel() && m.ask == Some(Ask::Discard), "changed: Cancel asks first");
        assert!(!m.answer(false) && m.ask.is_none(), "Keep editing keeps it open");
        let mine = m.buf.clone();

        // Revert asks, replaces only the editor's text, and Ctrl+Z gives the developer's text back.
        m.press_revert();
        assert!(!m.answer(true));
        assert_eq!(m.buf, "/* generated */\n");
        assert!(m.undo_revert());
        assert_eq!(m.buf, mine);
        m.press_revert();
        m.answer(false);
        assert_eq!(m.buf, mine, "declining Revert changes nothing");

        // Apply writes the bytes as they are; Undo last apply writes back what the file held.
        assert_eq!(m.press_apply(), Some((PathBuf::from("F.css"), mine.clone())));
        assert_eq!(m.press_undo_apply(), Some((PathBuf::from("F.css"), ".a { color: red }\r\n".into())));
        assert_eq!(m.buf, ".a { color: red }\r\n");

        // Revert then Cancel: nothing was written, the question is the discard one.
        m.press_revert();
        m.answer(true);
        assert!(!m.press_cancel());
        assert!(m.answer(true), "Discard closes; the file still holds the developer's text");
    }

    #[test]
    fn the_findings_carry_their_line() {
        let mut m = modal(Some(""));
        m.buf = ".a { color: red }\n.b { colr: red }\n.pr-form-layer { display: none }\n".into();
        let lines: Vec<(usize, Code)> = m.findings().iter().map(|d| (d.line, d.code)).collect();
        assert!(lines.contains(&(2, Code::UnknownProperty)), "{lines:?}");
        assert!(lines.iter().any(|&(l, c)| l == 3 && c == Code::LayerRule), "{lines:?}");
    }

    /// R7 / AC9: the window keeps the size it owns, frame after frame, in every language, with a
    /// sheet far longer and wider than the window; and the grip's drag is the size it then keeps.
    fn frames(lang: Language, m: &mut StylesheetModal, n: usize) -> Vec<egui::Rect> {
        let ctx = egui::Context::default();
        let tr = lang.tr();
        let mut seen = Vec::new();
        for _ in 0..n {
            let mut input = egui::RawInput::default();
            input.screen_rect = Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(3000.0, 2000.0)));
            ctx.begin_pass(input);
            let _ = m.show(&ctx, &tr);
            ctx.end_pass().textures_delta.clear();
            seen.push(ctx.memory(|mem| mem.area_rect(egui::Id::new(WINDOW_ID))).expect("the window registered its area"));
        }
        seen
    }

    #[test]
    fn the_window_keeps_its_size_for_120_frames_in_every_language() {
        let long: String = (0..150).map(|i| format!(".c{i} {{ colr: red; background: {} }}\n", "x".repeat(i % 7 * 90))).collect();
        for lang in Language::ALL {
            let mut m = modal(Some(&long));
            let rects = frames(*lang, &mut m, 120);
            let settled = rects[2];
            for (i, r) in rects.iter().enumerate().skip(2) {
                assert_eq!(*r, settled, "{lang:?}: frame {i} moved or grew");
            }
            assert!((settled.width() - DEFAULT_W).abs() <= 2.0 && (settled.height() - DEFAULT_H).abs() <= 40.0, "{lang:?}: {settled:?}");
            m.drag_by(egui::vec2(-200.0, -150.0));
            let rects = frames(*lang, &mut m, 30);
            assert!(rects.iter().skip(2).all(|r| *r == rects[2]), "{lang:?}: the dragged size holds");
            assert!((rects[2].width() - (DEFAULT_W - 200.0)).abs() <= 2.0, "{lang:?}: {:?}", rects[2]);
            m.drag_by(egui::vec2(-5000.0, -5000.0));
            assert_eq!(m.size, egui::vec2(MIN_W, MIN_H), "the grip stops at the minimum");
        }
    }
}

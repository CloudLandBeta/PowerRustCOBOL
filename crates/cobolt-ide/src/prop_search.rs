// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Properties and Events search (operator, 2026-10-04): what the developer
//! types finds a row by its NAME, by a word in its EXPLANATION (the hover
//! text), by the SECTION it sits in, or by its PURPOSE — "size" finds Width,
//! Height, FontSize, AutoSize and the rest, though none of them is called
//! "size".
//!
//! Every word typed must be found (so "font size" narrows to the font's
//! size). A word is found when it appears in any of those texts, or when it
//! names a purpose and the row's name carries one of that purpose's terms.
//! Purposes are recognised in all six IDE languages, because the developer
//! types in theirs while property names stay English.

/// What the developer typed, ready to match.
#[derive(Debug, Clone, Default)]
pub struct Query {
    words: Vec<String>,
}

impl Query {
    pub fn parse(text: &str) -> Self {
        Query {
            words: text
                .split_whitespace()
                .map(|w| w.to_lowercase())
                .filter(|w| !w.is_empty())
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Whether a row matches: every typed word is found in `names` (the row's
    /// label and property name) or `texts` (its section, its explanations), or
    /// names a purpose one of whose terms is in `names`.
    pub fn matches(&self, names: &[&str], texts: &[&str]) -> bool {
        let names: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
        let texts: Vec<String> = texts.iter().map(|t| t.to_lowercase()).collect();
        self.words.iter().all(|w| {
            names.iter().chain(texts.iter()).any(|h| h.contains(w.as_str()))
                || purpose_terms(w).iter().any(|term| {
                    // "x" and "y" are whole names, never letters inside one.
                    names.iter().any(|n| if term.len() <= 2 { n == term } else { n.contains(term) })
                })
        })
    }

    /// Whether a section's own name answers the whole query — then every row
    /// in it is shown.
    pub fn matches_section(&self, title: &str) -> bool {
        let t = title.to_lowercase();
        !self.words.is_empty() && self.words.iter().all(|w| t.contains(w.as_str()))
    }
}

/// The purposes a word can name, and the terms a property's name carries when
/// it serves that purpose. A word in any of the six IDE languages, or the
/// start of one (≥ 3 letters, so "siz" already finds the sizes), names it.
const PURPOSES: &[(&[&str], &[&str])] = &[
    (
        &["size", "dimension", "tamanho", "tamaño", "taille", "dimensão", "dimensión", "サイズ", "大きさ", "大小", "尺寸"],
        &["width", "height", "size", "autosize", "minimum", "maximum", "min", "max", "screenfill", "resizable", "scale", "zoom"],
    ),
    (
        &["color", "colour", "cor", "couleur", "色", "颜色"],
        &["color", "colour", "background", "foreground", "ink", "tint", "fill", "accent"],
    ),
    (
        &["position", "place", "location", "where", "posição", "posición", "lugar", "emplacement", "位置"],
        &["x", "y", "left", "top", "right", "bottom", "anchor", "dock", "startposition", "alignment", "align"],
    ),
    (
        &["font", "type", "typeface", "letra", "fonte", "fuente", "police", "フォント", "字体"],
        &["font", "bold", "italic", "underline", "strikethrough"],
    ),
    (
        &["text", "caption", "label", "texto", "texte", "文字", "テキスト", "文本"],
        &["text", "caption", "title", "hint", "placeholder", "tooltip", "label", "value"],
    ),
    (
        &["border", "outline", "frame", "borda", "borde", "bordure", "cadre", "枠", "边框"],
        &["border", "frame", "outline", "stroke"],
    ),
    (
        &["round", "rounded", "corner", "radius", "canto", "esquina", "coin", "arrondi", "角", "圆角"],
        &["corner", "radius", "round"],
    ),
    (
        &["shadow", "glow", "sombra", "ombre", "影", "阴影"],
        &["shadow", "glow"],
    ),
    (
        &["visible", "visibility", "hide", "hidden", "show", "visibilidade", "visibilidad", "ocultar", "masquer", "表示", "可见"],
        &["visible", "hidden", "hide", "show", "opacity", "transparency"],
    ),
    (
        &["transparent", "transparency", "opacity", "alpha", "transparente", "transparência", "transparencia", "透明"],
        &["transparency", "opacity", "alpha", "transparent"],
    ),
    (
        &["space", "spacing", "padding", "margin", "gap", "espaço", "espacio", "espacement", "余白", "间距"],
        &["padding", "margin", "gap", "spacing"],
    ),
    (
        &["image", "picture", "icon", "photo", "imagem", "imagen", "ícone", "icône", "画像", "图片", "图标"],
        &["image", "picture", "icon", "logo"],
    ),
    (
        &["window", "janela", "ventana", "fenêtre", "ウィンドウ", "窗口"],
        &["title", "minimize", "maximize", "fullscreen", "windowstate", "startposition", "resizable", "screenfill", "dock", "taskbar", "corner"],
    ),
    (
        &["click", "mouse", "press", "clique", "clic", "ratón", "souris", "クリック", "点击", "鼠标"],
        &["click", "mouse", "press", "pointer"],
    ),
    (
        &["key", "keyboard", "teclado", "clavier", "キー", "键盘"],
        &["key", "shortcut", "tab"],
    ),
    (
        &["scroll", "rolagem", "desplazamiento", "défilement", "スクロール", "滚动"],
        &["scroll"],
    ),
    (
        &["animation", "effect", "animação", "animación", "efeito", "efecto", "effet", "アニメーション", "动画"],
        &["animation", "effect", "fade", "slide", "transition"],
    ),
    (
        &["data", "binding", "dados", "datos", "données", "データ", "数据"],
        &["data", "binding", "bind", "source", "items", "rows", "columns", "file"],
    ),
];

/// The terms of every purpose `word` names: the word is one of its names, or
/// the start of one (three letters or more).
fn purpose_terms(word: &str) -> Vec<&'static str> {
    let short_enough = word.chars().count() >= 3;
    PURPOSES
        .iter()
        .filter(|(names, _)| {
            names
                .iter()
                .any(|n| *n == word || (short_enough && n.starts_with(word)))
        })
        .flat_map(|(_, terms)| terms.iter().copied())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(q: &str, name: &str, texts: &[&str]) -> bool {
        Query::parse(q).matches(&[name], texts)
    }

    #[test]
    fn a_purpose_finds_properties_named_otherwise() {
        for name in ["Width", "Height", "FontSize", "AutoSize", "MinimumWidth", "ScreenFill", "Resizable"] {
            assert!(hit("size", name, &[]), "size finds {name}");
        }
        assert!(hit("tamanho", "Height", &[]), "a Portuguese developer types tamanho");
        assert!(hit("サイズ", "Width", &[]), "and a Japanese one サイズ");
        assert!(hit("siz", "Height", &[]), "the start of a purpose already finds it");
        assert!(!hit("size", "Caption", &[]), "a caption is not a size");
        assert!(hit("colour", "BackgroundColor", &[]), "British spelling");
        assert!(hit("round", "CornerRadius", &[]));
        assert!(hit("position", "X", &[]), "X is a position");
        assert!(!hit("position", "TextBox", &[]), "a name with an x in it is not");
    }

    #[test]
    fn a_name_a_hover_word_or_a_section_finds_a_row() {
        assert!(hit("capt", "Caption", &[]), "part of the name");
        assert!(hit("letterbox", "BgImageMode", &["Fit letterboxes the picture inside the form"]), "a word in its explanation");
        assert!(hit("geometry", "X", &["Geometry"]), "the section it sits in");
        assert!(!hit("geometry", "X", &["Appearance"]));
    }

    #[test]
    fn every_word_must_be_found() {
        assert!(hit("font size", "FontSize", &[]));
        assert!(!hit("font size", "Width", &[]), "Width is a size but not the font's");
        assert!(Query::parse("   ").is_empty());
    }

    #[test]
    fn a_section_title_answers_the_whole_query() {
        let q = Query::parse("drop shadow");
        assert!(q.matches_section("DROP SHADOW"));
        assert!(!q.matches_section("Geometry"));
        assert!(!Query::parse("").matches_section("Geometry"));
    }
}

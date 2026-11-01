// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! **The player's own texts** (WASM-011 plan §1.12): what the player itself says to the user while it navigates —
//! a pop-up the browser refused, an address that names no form, a form left with unsaved changes. Six languages,
//! the IDE's (en, pt-BR, es, fr, ja, zh), chosen by the page's locale; the host page's `texts` (the developer's
//! translations, WASM-002 R15) replace any of them by key. The application's own texts stay the developer's.

use std::collections::BTreeMap;

/// The keys, as the host page's `texts` names them.
pub const KEYS: [&str; 5] = ["popup_blocked", "route_not_found", "leave_unsaved", "leave", "stay"];

/// The texts in one language. `route_not_found` holds `{form}`, replaced by the address's form name.
pub struct PlayerTexts {
    /// A BCP 47 tag; its first subtag is the language.
    pub lang: &'static str,
    pub popup_blocked: &'static str,
    pub route_not_found: &'static str,
    pub leave_unsaved: &'static str,
    pub leave: &'static str,
    pub stay: &'static str,
}

impl PlayerTexts {
    fn get(&self, key: &str) -> Option<&'static str> {
        Some(match key {
            "popup_blocked" => self.popup_blocked,
            "route_not_found" => self.route_not_found,
            "leave_unsaved" => self.leave_unsaved,
            "leave" => self.leave,
            "stay" => self.stay,
            _ => return None,
        })
    }
}

pub const PLAYER_TEXTS: [PlayerTexts; 6] = [
    PlayerTexts {
        lang: "en",
        popup_blocked: "The browser blocked the new window. Allow pop-ups for this site and try again.",
        route_not_found: "The application has no form named {form}.",
        leave_unsaved: "This form has unsaved changes. Leave it anyway?",
        leave: "Leave",
        stay: "Stay",
    },
    PlayerTexts {
        lang: "pt-BR",
        popup_blocked: "O navegador bloqueou a nova janela. Permita pop-ups para este site e tente novamente.",
        route_not_found: "A aplicação não tem um formulário chamado {form}.",
        leave_unsaved: "Este formulário tem alterações não salvas. Sair mesmo assim?",
        leave: "Sair",
        stay: "Ficar",
    },
    PlayerTexts {
        lang: "es",
        popup_blocked: "El navegador bloqueó la nueva ventana. Permita las ventanas emergentes para este sitio y vuelva a intentarlo.",
        route_not_found: "La aplicación no tiene ningún formulario llamado {form}.",
        leave_unsaved: "Este formulario tiene cambios sin guardar. ¿Salir de todos modos?",
        leave: "Salir",
        stay: "Quedarse",
    },
    PlayerTexts {
        lang: "fr",
        popup_blocked: "Le navigateur a bloqué la nouvelle fenêtre. Autorisez les fenêtres pop-up pour ce site et réessayez.",
        route_not_found: "L’application n’a aucun formulaire nommé {form}.",
        leave_unsaved: "Ce formulaire contient des modifications non enregistrées. Le quitter quand même ?",
        leave: "Quitter",
        stay: "Rester",
    },
    PlayerTexts {
        lang: "ja",
        popup_blocked: "ブラウザーが新しいウィンドウをブロックしました。このサイトのポップアップを許可して、もう一度お試しください。",
        route_not_found: "アプリケーションに {form} という名前のフォームはありません。",
        leave_unsaved: "このフォームには保存されていない変更があります。このまま移動しますか？",
        leave: "移動する",
        stay: "とどまる",
    },
    PlayerTexts {
        lang: "zh",
        popup_blocked: "浏览器阻止了新窗口。请允许此网站的弹出窗口，然后重试。",
        route_not_found: "应用程序中没有名为 {form} 的窗体。",
        leave_unsaved: "此窗体有未保存的更改。仍要离开吗？",
        leave: "离开",
        stay: "留下",
    },
];

/// The table of the page's language (its first subtag) among the six, English otherwise.
pub fn table(locale: &str) -> &'static PlayerTexts {
    let first = locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    PLAYER_TEXTS.iter().find(|t| t.lang.split('-').next() == Some(first.as_str())).unwrap_or(&PLAYER_TEXTS[0])
}

/// Text `key` in the page's language; the host page's own text for the key, when it gives a non-blank one, wins.
/// An unknown key answers itself (a programming error made visible, never a blank).
pub fn text(locale: &str, page: &BTreeMap<String, String>, key: &str) -> String {
    if let Some(own) = page.get(key).map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return own.to_owned();
    }
    table(locale).get(key).unwrap_or(key).to_owned()
}

/// The not-found notice for `form` (WASM-011 R6).
pub fn route_not_found(locale: &str, page: &BTreeMap<String, String>, form: &str) -> String {
    text(locale, page, "route_not_found").replace("{form}", form)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_every_key_is_present_in_all_six_languages() {
        let mut missing = Vec::new();
        for t in &PLAYER_TEXTS {
            for k in KEYS {
                if t.get(k).is_none_or(|s| s.trim().is_empty()) {
                    missing.push(format!("{}: {k}", t.lang));
                }
            }
            if !t.route_not_found.contains("{form}") {
                missing.push(format!("{}: route_not_found has no {{form}}", t.lang));
            }
        }
        assert!(missing.is_empty(), "missing or blank player texts: {missing:?}");
        let langs: Vec<&str> = PLAYER_TEXTS.iter().map(|t| t.lang).collect();
        assert_eq!(langs, ["en", "pt-BR", "es", "fr", "ja", "zh"]);
    }

    #[test]
    fn texts_follow_the_page_language_and_fall_back_to_english() {
        let none = BTreeMap::new();
        assert_eq!(text("pt-BR", &none, "stay"), "Ficar");
        assert_eq!(text("pt_PT", &none, "stay"), "Ficar");
        assert_eq!(text("ja-JP", &none, "leave"), "移動する");
        assert_eq!(text("de", &none, "stay"), "Stay");
        assert_eq!(text("", &none, "leave"), "Leave");
        assert_eq!(route_not_found("es", &none, "CUSTOMER-FORM"), "La aplicación no tiene ningún formulario llamado CUSTOMER-FORM.");
    }

    #[test]
    fn texts_the_host_page_override_wins_and_a_blank_one_keeps_the_default() {
        let page: BTreeMap<String, String> =
            [("popup_blocked", "Pop-ups, please."), ("stay", "   ")].into_iter().map(|(k, v)| (k.to_owned(), v.to_owned())).collect();
        assert_eq!(text("fr", &page, "popup_blocked"), "Pop-ups, please.");
        assert_eq!(text("fr", &page, "stay"), "Rester");
        assert_eq!(text("fr", &page, "no_such_key"), "no_such_key");
    }
}

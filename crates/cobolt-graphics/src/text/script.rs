// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The Unicode script of each character, as the shaper wants it.
//!
//! Shaping chooses its OpenType features by script, so a run handed to the
//! shaper must not mix scripts. Characters of the `Common` and `Inherited`
//! scripts (spaces, digits, punctuation, combining marks) take the script of the
//! text around them.

use icu_properties::props::Script as Icu;
use icu_properties::CodePointMapData;

/// What is known about one character's script.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScriptInfo {
    /// The script the shaper should use (resolved for neutrals), if it has
    /// special handling for it.
    pub script: Option<harfrust::Script>,
    /// Whether the character itself is `Common`/`Inherited` (it took its
    /// script from its neighbours).
    pub neutral: bool,
}

fn to_hb(s: Icu) -> Option<harfrust::Script> {
    use harfrust::script as hb;
    let m = [
        (Icu::Latin, hb::LATIN),
        (Icu::Cyrillic, hb::CYRILLIC),
        (Icu::Greek, hb::GREEK),
        (Icu::Arabic, hb::ARABIC),
        (Icu::Hebrew, hb::HEBREW),
        (Icu::Han, hb::HAN),
        (Icu::Hiragana, hb::HIRAGANA),
        (Icu::Katakana, hb::KATAKANA),
        (Icu::Hangul, hb::HANGUL),
        (Icu::Thai, hb::THAI),
        (Icu::Devanagari, hb::DEVANAGARI),
        (Icu::Bengali, hb::BENGALI),
        (Icu::Gurmukhi, hb::GURMUKHI),
        (Icu::Gujarati, hb::GUJARATI),
        (Icu::Tamil, hb::TAMIL),
        (Icu::Telugu, hb::TELUGU),
        (Icu::Kannada, hb::KANNADA),
        (Icu::Malayalam, hb::MALAYALAM),
        (Icu::Armenian, hb::ARMENIAN),
        (Icu::Georgian, hb::GEORGIAN),
        (Icu::Khmer, hb::KHMER),
        (Icu::Lao, hb::LAO),
        (Icu::Tibetan, hb::TIBETAN),
        (Icu::Myanmar, hb::MYANMAR),
        (Icu::Ethiopian, hb::ETHIOPIC),
        (Icu::Mongolian, hb::MONGOLIAN),
        (Icu::Bopomofo, hb::BOPOMOFO),
    ];
    m.iter().find(|(i, _)| *i == s).map(|(_, h)| *h)
}

/// The script of every character of `text`, with neutrals resolved to the
/// script of the preceding strong character (or the following one at the start
/// of the text).
pub(crate) fn resolve_scripts(text: &str) -> Vec<ScriptInfo> {
    let map = CodePointMapData::<Icu>::new();
    // None = neutral, Some(x) = strong with x.
    let raw: Vec<Option<Option<harfrust::Script>>> = text
        .chars()
        .map(|c| {
            let s = map.get(c);
            if s == Icu::Common || s == Icu::Inherited || s == Icu::Unknown {
                None
            } else {
                Some(to_hb(s))
            }
        })
        .collect();
    let mut out = vec![ScriptInfo { script: None, neutral: true }; raw.len()];
    let mut last: Option<Option<harfrust::Script>> = None;
    for (i, c) in raw.iter().enumerate() {
        match c {
            Some(s) => {
                last = Some(*s);
                out[i] = ScriptInfo { script: *s, neutral: false };
            }
            None => {
                if let Some(s) = last {
                    out[i] = ScriptInfo { script: s, neutral: true };
                }
            }
        }
    }
    // Leading neutrals take the first strong script that follows.
    if let Some(p) = raw.iter().position(|c| c.is_some()) {
        let s = out[p].script;
        for slot in out.iter_mut().take(p) {
            slot.script = s;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use harfrust::script as hb;

    #[test]
    fn scripts_resolve_with_neutrals_inheriting() {
        let text = "ab 12 日本 العربية !";
        let r = resolve_scripts(text);
        let chars: Vec<char> = text.chars().collect();
        let at = |c: char| r[chars.iter().position(|x| *x == c).unwrap()];
        assert_eq!(at('a').script, Some(hb::LATIN));
        assert!(!at('a').neutral);
        assert_eq!(at('1').script, Some(hb::LATIN), "digits follow the text before them");
        assert!(at('1').neutral);
        assert_eq!(at('日').script, Some(hb::HAN));
        assert_eq!(at('ع').script, Some(hb::ARABIC));
        assert_eq!(r.last().unwrap().script, Some(hb::ARABIC), "trailing punctuation follows");
    }

    #[test]
    fn leading_neutrals_take_the_following_script() {
        let r = resolve_scripts("  שלום");
        assert_eq!(r[0].script, Some(hb::HEBREW));
        assert_eq!(r[1].script, Some(hb::HEBREW));
        assert!(r[0].neutral);
    }

    #[test]
    fn all_neutral_text_has_no_script() {
        assert!(resolve_scripts("123 !?").iter().all(|s| s.script.is_none() && s.neutral));
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The application's model list (spec 076 §4.1).
//!
//! The list belongs to the **program**: it keeps its entries in an indexed file
//! of its own and hands each one over with `COBOL-MODEL-SET` while it runs.
//! The runtime holds them for the life of the process only — nothing here is
//! ever written to disk (R4) — and shares them with every form the process
//! hosts, child forms included (R18). An entry carries no key: keys live in
//! the [`crate::key_store`].
//!
//! Each name has a **generation** that moves whenever its entry changes or is
//! withdrawn, which is how an agent using it is told (R16).

use std::collections::HashMap;
use std::sync::Mutex;

/// One model a program has handed over (R1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// One of the protocols an `AgentObject` speaks: `OpenAI`, `Anthropic`,
    /// `Ollama`, `LMStudio`, `Custom`.
    pub api: String,
    pub url: String,
    pub model: String,
}

impl ModelEntry {
    /// Whether this entry's API cannot be used without a key.
    pub fn needs_key(&self) -> bool {
        matches!(self.api.trim().to_ascii_lowercase().as_str(), "openai" | "anthropic")
    }
}

/// Name → (entry, or `None` once withdrawn; its generation).
fn list() -> &'static Mutex<HashMap<String, (Option<ModelEntry>, u64)>> {
    static LIST: std::sync::OnceLock<Mutex<HashMap<String, (Option<ModelEntry>, u64)>>> =
        std::sync::OnceLock::new();
    LIST.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// Add or change an entry. `true` when anything changed.
pub fn set(name: &str, entry: ModelEntry) -> bool {
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    let slot = l.entry(key(name)).or_insert((None, 0));
    if slot.0.as_ref() == Some(&entry) {
        return false;
    }
    slot.0 = Some(entry);
    slot.1 += 1;
    true
}

/// Withdraw an entry. `true` when there was one.
pub fn remove(name: &str) -> bool {
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    match l.get_mut(&key(name)) {
        Some(slot) if slot.0.is_some() => {
            slot.0 = None;
            slot.1 += 1;
            true
        }
        _ => false,
    }
}

/// The entry and its generation, if the program has handed one over.
pub fn get(name: &str) -> Option<(ModelEntry, u64)> {
    let l = list().lock().unwrap_or_else(|p| p.into_inner());
    l.get(&key(name)).and_then(|(e, g)| e.clone().map(|e| (e, *g)))
}

/// The name's current generation (0 for a name never used).
pub fn generation(name: &str) -> u64 {
    let l = list().lock().unwrap_or_else(|p| p.into_inner());
    l.get(&key(name)).map(|(_, g)| *g).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(model: &str) -> ModelEntry {
        ModelEntry {
            api: "OpenAI".into(),
            url: "http://example/v1/chat/completions".into(),
            model: model.into(),
        }
    }

    #[test]
    fn entries_change_their_generation_only_when_they_change() {
        let name = "unit-test-company-model";
        assert!(get(name).is_none());
        assert!(set(name, entry("a")));
        let g1 = generation(name);
        assert!(!set(name, entry("a")), "the same entry again changes nothing");
        assert_eq!(generation(name), g1);
        assert!(set(" Unit-Test-Company-Model ", entry("b")), "names ignore case and spaces");
        assert!(generation(name) > g1);
        assert_eq!(get(name).unwrap().0.model, "b");
        let g2 = generation(name);
        assert!(remove(name));
        assert!(get(name).is_none());
        assert!(generation(name) > g2, "a withdrawal is a change too");
        assert!(!remove(name));
        assert!(entry("x").needs_key());
        assert!(!ModelEntry { api: "Ollama".into(), ..entry("x") }.needs_key());
    }
}

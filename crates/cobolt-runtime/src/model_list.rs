// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The application's model list (spec 076 §4.1, spec 085 D4a).
//!
//! The **application's** AI settings: the models its agents can ask, by name.
//! A program adds, changes and withdraws entries with `COBOL-MODEL-SET` /
//! `COBOL-MODEL-REMOVE` and lists them with `COBOL-MODEL-COUNT` /
//! `COBOL-MODEL-GET`; every form the process hosts shares them, child forms
//! included (076 R18).
//!
//! Since spec 085 the runtime **keeps** the list (operator, 2026-10-03 —
//! superseding 076 R2/R4, under which each program kept its own copy and
//! handed it over at start-up): it lives in `settings/models.json` in the
//! application's folder, beside the key store, is read the first time the
//! list is used and written on every change. So the list is the
//! application's, not one form's: an assistant added to an application and
//! the application's own agents see the same models. The file holds no
//! secret — an entry carries no key; keys stay in the [`crate::key_store`],
//! which no program can read back.
//!
//! Each name has a **generation** that moves whenever its entry changes or is
//! withdrawn, which is how an agent using it is told (076 R16).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// One model the application's agents can ask (076 R1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// A provider id from [`crate::providers::PROVIDERS`] (`openai`,
    /// `anthropic`, `groq`, `ollama`, … — the IDE's list), or one of the older
    /// protocol names an `AgentObject` also speaks: `LMStudio`, `Custom`.
    pub api: String,
    pub url: String,
    pub model: String,
}

impl ModelEntry {
    /// Whether this entry's API cannot be used without a key: a provider from
    /// the catalogue that needs one — every one but local Ollama, the IDE's
    /// `provider_requires_key` rule.
    pub fn needs_key(&self) -> bool {
        crate::providers::find(&self.api).is_some() && crate::providers::requires_key(&self.api)
    }
}

/// One name's slot: the name as it was given, its entry (`None` once
/// withdrawn) and its generation.
struct Slot {
    name: String,
    entry: Option<ModelEntry>,
    generation: u64,
}

struct List {
    slots: HashMap<String, Slot>,
    /// Where the list is kept; `None` = memory only (tests, or a host that
    /// asked for it).
    file: Option<PathBuf>,
    /// Why the file could not be read. While set, nothing is written, so the
    /// unreadable file is never overwritten — the key store's rule (076 R11).
    problem: Option<String>,
}

/// The file the list lives in, under the application's folder.
pub const FILE: &str = "settings/models.json";

fn memory_only() -> &'static std::sync::atomic::AtomicBool {
    static FLAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(cfg!(test));
    &FLAG
}

/// Keep the list in memory only, never on disk — for tests, which must not
/// write into the folder they run in. Takes effect even after first use.
pub fn use_memory_only() {
    memory_only().store(true, std::sync::atomic::Ordering::SeqCst);
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    l.file = None;
}

fn list() -> &'static Mutex<List> {
    static LIST: std::sync::OnceLock<Mutex<List>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| Mutex::new(open()))
}

/// The application's list, read from its file — the folder is the one the key
/// store uses: the application's (`assets::current_base`), else the folder it
/// runs in.
fn open() -> List {
    if memory_only().load(std::sync::atomic::Ordering::SeqCst) {
        return List { slots: HashMap::new(), file: None, problem: None };
    }
    let base = cobolt_forms::assets::current_base()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let path = base.join(FILE);
    let mut l = List { slots: HashMap::new(), file: Some(path.clone()), problem: None };
    match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => l.problem = Some(format!("{}: {e}", path.display())),
        Ok(bytes) => match parse(&bytes) {
            Some(entries) => {
                for (name, entry) in entries {
                    l.slots.insert(key(&name), Slot { name, entry: Some(entry), generation: 1 });
                }
            }
            None => l.problem = Some(format!("{} is not a model list", path.display())),
        },
    }
    if let Some(p) = &l.problem {
        eprintln!("model list: {p} — starting empty, and the file is left as it is");
    }
    l
}

fn parse(bytes: &[u8]) -> Option<Vec<(String, ModelEntry)>> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    v.get("entries")?
        .as_array()?
        .iter()
        .map(|e| {
            let text = |k: &str| e.get(k).and_then(|x| x.as_str()).map(str::to_owned);
            Some((text("name")?, ModelEntry { api: text("api")?, url: text("url")?, model: text("model")? }))
        })
        .collect()
}

/// Write the list to its file (a whole new file, renamed into place).
fn save(l: &List) {
    let Some(path) = &l.file else { return };
    if l.problem.is_some() {
        return;
    }
    let mut entries: Vec<&Slot> = l.slots.values().filter(|s| s.entry.is_some()).collect();
    entries.sort_by_key(|s| s.name.to_ascii_uppercase());
    let json = serde_json::json!({
        "version": 1,
        "entries": entries.iter().map(|s| {
            let e = s.entry.as_ref().unwrap();
            serde_json::json!({"name": s.name, "api": e.api, "url": e.url, "model": e.model})
        }).collect::<Vec<_>>(),
    });
    let result = (|| -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&json).unwrap_or_default())?;
        std::fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        eprintln!("model list: cannot write {}: {e}", path.display());
    }
}

fn key(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

/// Add or change an entry. `true` when anything changed.
pub fn set(name: &str, entry: ModelEntry) -> bool {
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    let slot = l.slots.entry(key(name)).or_insert(Slot { name: name.trim().to_owned(), entry: None, generation: 0 });
    if slot.entry.as_ref() == Some(&entry) {
        return false;
    }
    slot.name = name.trim().to_owned();
    slot.entry = Some(entry);
    slot.generation += 1;
    save(&l);
    true
}

/// Withdraw an entry. `true` when there was one.
pub fn remove(name: &str) -> bool {
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    let changed = match l.slots.get_mut(&key(name)) {
        Some(slot) if slot.entry.is_some() => {
            slot.entry = None;
            slot.generation += 1;
            true
        }
        _ => false,
    };
    if changed {
        save(&l);
    }
    changed
}

/// The entry and its generation, if there is one.
pub fn get(name: &str) -> Option<(ModelEntry, u64)> {
    let l = list().lock().unwrap_or_else(|p| p.into_inner());
    l.slots.get(&key(name)).and_then(|s| s.entry.clone().map(|e| (e, s.generation)))
}

/// The name's current generation (0 for a name never used).
pub fn generation(name: &str) -> u64 {
    let l = list().lock().unwrap_or_else(|p| p.into_inner());
    l.slots.get(&key(name)).map(|s| s.generation).unwrap_or(0)
}

/// Forget every entry, as if the application had none — for tests that run
/// one after another in a process, each expecting a fresh application.
pub fn clear() {
    let mut l = list().lock().unwrap_or_else(|p| p.into_inner());
    for slot in l.slots.values_mut() {
        if slot.entry.take().is_some() {
            slot.generation += 1;
        }
    }
    save(&l);
}

/// Every entry, by name (spec 085: `COBOL-MODEL-COUNT` / `COBOL-MODEL-GET`
/// number them 1… in this order).
pub fn entries() -> Vec<(String, ModelEntry)> {
    let l = list().lock().unwrap_or_else(|p| p.into_inner());
    let mut v: Vec<(String, ModelEntry)> =
        l.slots.values().filter_map(|s| s.entry.clone().map(|e| (s.name.clone(), e))).collect();
    v.sort_by_key(|(n, _)| n.to_ascii_uppercase());
    v
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

    /// Spec 085 — the list's file: what is written reads back as the same
    /// entries; a file that is not a model list is refused, never parsed into
    /// half a list.
    #[test]
    fn the_file_reads_back_what_was_written() {
        let json = serde_json::json!({"version": 1, "entries": [
            {"name": "Company model", "api": "openai", "url": "https://x/v1", "model": "gpt"},
        ]});
        let parsed = parse(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(parsed, vec![("Company model".to_owned(), ModelEntry { api: "openai".into(), url: "https://x/v1".into(), model: "gpt".into() })]);
        assert!(parse(b"PRCKEYS1 not json").is_none());
        assert!(parse(br#"{"entries":[{"name":"x"}]}"#).is_none(), "an entry missing a field refuses the file");
    }
}

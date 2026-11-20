// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The built-ins of the application's model list, its key store and the model providers (specs 076
//! and 048): `COBOL::"MODEL-SET"`, `"MODEL-REMOVE"`, `"MODEL-COUNT"`, `"MODEL-GET"`, `"KEY-SET"`,
//! `"KEY-REMOVE"`, `"KEY-IS-SET"`, `"PROVIDER-COUNT"`, `"PROVIDER-GET"`, `"MODEL-LIST"`,
//! `"MODEL-LIST-GET"` and `"MODEL-TEST"`. What each reads and what it answers, for the two hosts that
//! run them: the interpreter, and the runner of a desktop program built on the WebAssembly path
//! (DESKTOP-WASM-001 T48).
//!
//! A host hands [`run`] the call's argument count and a way to read argument `i` (an argument is read
//! only when the built-in needs it, and in the order it needs it); [`run`] answers the arguments it
//! writes, by position. The model list and the key store are the process's own (`model_list`,
//! `key_store`); the models a provider last listed belong to the program that asked
//! (`provider_models`).

use crate::HttpClient;

/// How a built-in reads one of its arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Read {
    /// Its text, as `DISPLAY` shows it.
    Text,
    /// Its value as a whole number (0 when it is not one), as decimal text.
    Number,
}

/// What a built-in writes into one of its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Text, moved as an alphanumeric item is.
    Text(String),
    /// A whole number, stored as a number is.
    Number(i64),
}

impl Answer {
    /// The answer as text: a number's decimal digits.
    pub fn text(&self) -> String {
        match self {
            Answer::Text(t) => t.clone(),
            Answer::Number(n) => n.to_string(),
        }
    }
}

/// The built-ins, without the `COBOL-` prefix, each with the fewest arguments it runs with (a call with
/// fewer is not one of these) and the arguments it writes (0-based), in the order it writes them. A
/// call with fewer arguments than one it writes leaves that one out.
pub const VERBS: &[(&str, usize, &[usize])] = &[
    ("MODEL-SET", 4, &[4]),
    ("MODEL-REMOVE", 1, &[1]),
    ("MODEL-COUNT", 1, &[0]),
    ("MODEL-GET", 2, &[1, 2, 3, 4]),
    ("KEY-SET", 1, &[2]),
    ("KEY-REMOVE", 1, &[1]),
    ("KEY-IS-SET", 2, &[1]),
    ("PROVIDER-COUNT", 1, &[0]),
    ("PROVIDER-GET", 2, &[1, 2, 3, 4]),
    ("MODEL-LIST", 4, &[3, 4]),
    ("MODEL-LIST-GET", 2, &[1]),
    ("MODEL-TEST", 4, &[4]),
];

/// `verb`'s row of [`VERBS`] (upper case, without `COBOL-`).
pub fn verb(verb: &str) -> Option<(usize, &'static [usize])> {
    VERBS.iter().find(|(v, _, _)| *v == verb).map(|(_, min, out)| (*min, *out))
}

/// Run `verb` (upper case, without `COBOL-`) with `n` arguments, reading argument `i` with
/// `arg(i, how)`: the arguments it writes, by position, in order. `None` when `verb` is not one of
/// these built-ins or `n` is fewer than it runs with. `provider_models` holds the models the calling
/// program's last `MODEL-LIST` found; `http` is that program's client.
pub fn run<E>(
    verb_name: &str,
    n: usize,
    arg: &mut dyn FnMut(usize, Read) -> Result<String, E>,
    http: &HttpClient,
    provider_models: &mut Vec<String>,
) -> Option<Result<Vec<(usize, Answer)>, E>> {
    let (min, _) = verb(verb_name)?;
    if n < min {
        return None;
    }
    Some(answer(verb_name, n, arg, http, provider_models))
}

fn answer<E>(
    verb_name: &str,
    n: usize,
    arg: &mut dyn FnMut(usize, Read) -> Result<String, E>,
    http: &HttpClient,
    provider_models: &mut Vec<String>,
) -> Result<Vec<(usize, Answer)>, E> {
    // The answers a call has room for: argument `i` only when the call has one.
    let mut out = Vec::new();
    let mut put = |i: usize, a: Answer| {
        if i < n {
            out.push((i, a));
        }
    };
    let status = |r: Result<(), String>| match r {
        Ok(()) => "OK".to_string(),
        Err(e) => e,
    };
    match verb_name {
        "MODEL-SET" => {
            let name = arg(0, Read::Text)?.trim().to_string();
            let entry = crate::model_list::ModelEntry {
                api: arg(1, Read::Text)?.trim().to_string(),
                url: arg(2, Read::Text)?.trim().to_string(),
                model: arg(3, Read::Text)?.trim().to_string(),
            };
            let status = if name.is_empty() {
                "the entry has no name".to_string()
            } else {
                crate::model_list::set(&name, entry);
                "OK".to_string()
            };
            put(4, Answer::Text(status));
        }
        "MODEL-COUNT" => put(0, Answer::Text(crate::model_list::entries().len().to_string())),
        "MODEL-GET" => {
            let ix = arg(0, Read::Text)?.trim().parse::<usize>().unwrap_or(0);
            let entries = crate::model_list::entries();
            let fields: [String; 4] = match ix.checked_sub(1).and_then(|i| entries.get(i)) {
                Some((name, e)) => [name.clone(), e.api.clone(), e.url.clone(), e.model.clone()],
                None => Default::default(),
            };
            for (i, value) in fields.into_iter().enumerate() {
                put(i + 1, Answer::Text(value));
            }
        }
        "MODEL-REMOVE" => {
            let name = arg(0, Read::Text)?;
            crate::model_list::remove(name.trim());
            put(1, Answer::Text("OK".to_string()));
        }
        "KEY-SET" => {
            let name = arg(0, Read::Text)?;
            let result = if n > 1 {
                let key = arg(1, Read::Text)?;
                crate::key_store::key_store().set(name.trim(), key.trim())
            } else {
                Err("no key was given".to_string())
            };
            put(2, Answer::Text(status(result)));
        }
        "KEY-REMOVE" => {
            let name = arg(0, Read::Text)?;
            put(1, Answer::Text(status(crate::key_store::key_store().remove(name.trim()))));
        }
        "KEY-IS-SET" => {
            let name = arg(0, Read::Text)?;
            let set = crate::key_store::key_store().is_set(name.trim());
            put(1, Answer::Text(if set { "Y" } else { "N" }.to_string()));
        }
        "PROVIDER-COUNT" => put(0, Answer::Number(crate::providers::PROVIDERS.len() as i64)),
        "PROVIDER-GET" => {
            let i = arg(0, Read::Number)?.trim().parse::<i64>().unwrap_or(0);
            let fields: [String; 4] = match usize::try_from(i - 1).ok().and_then(|i| crate::providers::PROVIDERS.get(i)) {
                Some(p) => [
                    p.id.to_string(),
                    p.label.to_string(),
                    p.default_endpoint.to_string(),
                    if crate::providers::requires_key(p.id) { "Y" } else { "N" }.to_string(),
                ],
                None => Default::default(),
            };
            for (i, value) in fields.into_iter().enumerate() {
                put(i + 1, Answer::Text(value));
            }
        }
        "MODEL-LIST" | "MODEL-TEST" => {
            let listing = verb_name == "MODEL-LIST";
            let provider = arg(0, Read::Text)?.trim().to_string();
            let endpoint = arg(1, Read::Text)?.trim().to_string();
            // LIST: provider endpoint key count status [entry]
            // TEST: provider endpoint model key status [entry]
            let (model, key_at) = if listing { (String::new(), 2) } else { (arg(2, Read::Text)?.trim().to_string(), 3) };
            let mut key = arg(key_at, Read::Text)?.trim().to_string();
            // `entry` is the sixth argument of both calls; its stored key stands in for a blank one.
            if key.is_empty() && n > 5 {
                let entry = arg(5, Read::Text)?;
                key = crate::key_store::key_store().get(entry.trim()).unwrap_or_default();
            }
            if listing {
                let status = match crate::providers::list_models(http, &provider, &endpoint, &key) {
                    Ok(models) => {
                        *provider_models = models;
                        if provider_models.is_empty() {
                            "The provider answered, but offers no models.".to_string()
                        } else {
                            "OK".to_string()
                        }
                    }
                    Err(e) => {
                        provider_models.clear();
                        format!("Could not list models: {e}")
                    }
                };
                put(3, Answer::Number(provider_models.len() as i64));
                put(4, Answer::Text(status));
            } else {
                let status = match crate::providers::test_connection(http, &provider, &endpoint, &model, &key) {
                    Ok(()) => "OK".to_string(),
                    Err(e) => e,
                };
                put(4, Answer::Text(status));
            }
        }
        "MODEL-LIST-GET" => {
            let i = arg(0, Read::Number)?.trim().parse::<i64>().unwrap_or(0);
            let name = usize::try_from(i - 1).ok().and_then(|i| provider_models.get(i)).cloned().unwrap_or_default();
            put(1, Answer::Text(name));
        }
        _ => {}
    }
    Ok(out)
}

/// A whole number read from an argument's text, as the interpreter reads a numeric argument: the
/// digits and sign of an integer, or the whole part of a decimal; 0 otherwise.
pub fn whole_number(text: &str) -> i64 {
    let t = text.trim();
    t.parse::<i64>()
        .ok()
        .or_else(|| t.parse::<f64>().ok().filter(|f| f.is_finite()).map(|f| f.trunc() as i64))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(verb_name: &str, args: &[&str], models: &mut Vec<String>) -> Option<Vec<(usize, Answer)>> {
        let mut read = |i: usize, how: Read| -> Result<String, ()> {
            let t = args.get(i).copied().unwrap_or("").to_string();
            Ok(match how {
                Read::Text => t,
                Read::Number => whole_number(&t).to_string(),
            })
        };
        run(verb_name, args.len(), &mut read, &HttpClient::default(), models).map(|r| r.unwrap())
    }

    #[test]
    fn the_verbs_answer_what_the_interpreter_answered() {
        crate::model_list::use_memory_only();
        let mut models = Vec::new();
        let mut checks = 0;
        let mut check = |ok: bool, what: &str| {
            assert!(ok, "{what}");
            checks += 1;
        };
        // Fewer arguments than a verb runs with, and a name that is not one of these.
        check(call("MODEL-SET", &["a", "b", "c"], &mut models).is_none(), "MODEL-SET needs four");
        check(call("MCP-SEARCH", &["t", "{}", ""], &mut models).is_none(), "MCP-SEARCH is not one");
        // The list.
        let set = call("MODEL-SET", &["door-test", "Ollama", "http://127.0.0.1:9/v1", "m1", ""], &mut models).unwrap();
        check(set == vec![(4, Answer::Text("OK".into()))], "MODEL-SET answers OK in its fifth argument");
        let unnamed = call("MODEL-SET", &[" ", "Ollama", "u", "m", ""], &mut models).unwrap();
        check(unnamed == vec![(4, Answer::Text("the entry has no name".into()))], "an unnamed entry is refused");
        let four = call("MODEL-SET", &["door-test-2", "Ollama", "u", "m"], &mut models).unwrap();
        check(four.is_empty(), "with four arguments MODEL-SET answers nothing");
        let got = call("MODEL-GET", &["99", "", ""], &mut models).unwrap();
        check(got == vec![(1, Answer::Text(String::new())), (2, Answer::Text(String::new()))], "an index out of range gives spaces, as many as asked");
        let removed = call("MODEL-REMOVE", &["door-test-2", ""], &mut models).unwrap();
        check(removed == vec![(1, Answer::Text("OK".into()))], "MODEL-REMOVE answers OK");
        // The providers.
        let n = call("PROVIDER-COUNT", &[""], &mut models).unwrap();
        check(n == vec![(0, Answer::Number(crate::providers::PROVIDERS.len() as i64))], "PROVIDER-COUNT is the catalogue's length");
        let p = call("PROVIDER-GET", &["1.0", "", "", "", ""], &mut models).unwrap();
        let first = &crate::providers::PROVIDERS[0];
        check(p[0] == (1, Answer::Text(first.id.to_string())) && p.len() == 4, "PROVIDER-GET reads its index as a number");
        // A provider's models, from a list the program asked for earlier.
        models.extend(["a".to_string(), "b".to_string()]);
        let m = call("MODEL-LIST-GET", &["2", ""], &mut models).unwrap();
        check(m == vec![(1, Answer::Text("b".into()))], "MODEL-LIST-GET reads the program's last list");
        println!("model_door: {checks} of {checks} checks passed");
    }
}

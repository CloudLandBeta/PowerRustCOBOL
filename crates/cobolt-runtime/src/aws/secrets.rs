// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! What an AWS control holds but must never show (spec 078 Q6): a Cognito
//! sign-in's tokens. Kept in this process's memory only, per control, put
//! into a later request by `{secret:Name}`, and never written to a property,
//! a row, `ResultJson`, a `Verbose` line or a file.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn store() -> &'static Mutex<HashMap<String, HashMap<String, String>>> {
    static S: OnceLock<Mutex<HashMap<String, HashMap<String, String>>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Keep `values` for `owner` (a connection and a control), replacing what it
/// held.
pub fn keep(owner: &str, values: Vec<(String, String)>) {
    let mut s = store().lock().unwrap_or_else(|e| e.into_inner());
    s.insert(owner.to_owned(), values.into_iter().filter(|(_, v)| !v.is_empty()).collect());
}

pub fn get(owner: &str, name: &str) -> Option<String> {
    store().lock().unwrap_or_else(|e| e.into_inner()).get(owner)?.get(name).cloned()
}

pub fn forget(owner: &str) {
    store().lock().unwrap_or_else(|e| e.into_inner()).remove(owner);
}

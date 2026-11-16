// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The menus a bundle delivers, which the host keeps for its forms (moved from the player's `view`
//! with the host, DESKTOP-WASM-001 T27; the player's `view` re-exports it).

use std::collections::BTreeMap;

use cobolt_forms::menu::MenuDefinition;

/// Menu definitions delivered with the bundle, by `(form name lower-case, control name upper-case)`.
#[derive(Debug, Clone, Default)]
pub struct MenuStore {
    pub menus: BTreeMap<(String, String), MenuDefinition>,
}

impl MenuStore {
    pub fn get(&self, form: &str, control: &str) -> Option<&MenuDefinition> {
        self.menus.get(&(form.to_ascii_lowercase(), control.to_ascii_uppercase()))
    }

    pub fn insert(&mut self, form: &str, control: &str, def: MenuDefinition) {
        self.menus.insert((form.to_ascii_lowercase(), control.to_ascii_uppercase()), def);
    }
}

// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! The AWS connections the running application knows (spec 078 R12, R13).
//!
//! Process-global, like the server pool: each host publishes the project's
//! connections once at start — `rcrun run-form` from the project, a built
//! application from its baked catalogue — and every form in the process, the
//! child forms included, resolves its controls' `Connection` here.

use std::sync::{OnceLock, RwLock};

pub use cobolt_forms::connections::AwsConnection;

fn registry() -> &'static RwLock<Vec<AwsConnection>> {
    static REG: OnceLock<RwLock<Vec<AwsConnection>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(Vec::new()))
}

/// Replace the known connections.
pub fn publish(connections: Vec<AwsConnection>) {
    *registry().write().unwrap_or_else(|e| e.into_inner()) = connections;
}

/// The connection a control names — by name or id, case-insensitively. An
/// empty name resolves to the project's only connection, when it has exactly
/// one (R13).
pub fn find(name: &str) -> Option<AwsConnection> {
    let all = registry().read().unwrap_or_else(|e| e.into_inner());
    let name = name.trim();
    if name.is_empty() {
        return (all.len() == 1).then(|| all[0].clone());
    }
    all.iter()
        .find(|c| c.name.eq_ignore_ascii_case(name) || c.id.eq_ignore_ascii_case(name))
        .cloned()
}

/// Every known connection's name, for messages.
pub fn names() -> Vec<String> {
    registry().read().unwrap_or_else(|e| e.into_inner()).iter().map(|c| c.name.clone()).collect()
}

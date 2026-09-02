// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Every property a non-visual control seeds must have a **declared reader**.
//!
//! `RestClient` shipped with `BaseURL`, `AuthType`, `AuthToken`,
//! `DefaultHeaders`, `DefaultMethod`, `FollowRedirects` and `VerifyTLS` seeded
//! on the control, editable in the property pane and documented in the System
//! KB — and read by nothing whatsoever. A developer configured the control
//! correctly and every request went out unauthenticated to the empty URL. The
//! property pane made a promise the runtime had never heard of.
//!
//! Nothing detected that, because "is this property read anywhere?" was a
//! question no test asked. This one asks it. Each seeded property is declared
//! as one of:
//!
//! * [`Reader::Runtime`]   — read by `cobolt-runtime`, checked against its sources
//! * [`Reader::Generated`] — turned into COBOL by `cobolt-codegen`
//! * [`Reader::Unread`]    — nothing reads it yet: tracked debt, with the reason
//!
//! The test fails when a seeded property is missing from the table, so adding
//! one to `Control::new` forces an answer to "what reads this?" — and fails
//! when a `Runtime` property stops appearing in the runtime sources, so a
//! refactor cannot quietly orphan one again.
//!
//! **Scope.** The five non-visual types whose properties describe *runtime*
//! behaviour. `Timer` (two properties, both live) and `Snackbar` (painted by
//! `cobolt-forms`, which this crate cannot see) are deliberately out.

use cobolt_forms::model::{Control, ControlType};
use std::collections::BTreeSet;

/// The sources a [`Reader::Runtime`] claim is checked against — where the
/// interpreter reads control properties.
const RUNTIME_SOURCES: &[&str] = &[
    include_str!("../src/interpreter.rs"),
    include_str!("../src/http_runtime.rs"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reader {
    /// `cobolt-runtime` reads it while the form runs.
    Runtime,
    /// `cobolt-codegen` consumes it when generating COBOL; the runtime never
    /// sees the property itself, only the generated source.
    Generated,
    /// Nothing functional reads it. The note says what would have to exist.
    /// A property here is **debt, not design** — it is offered in the property
    /// pane and setting it changes nothing.
    Unread(&'static str),
}

use Reader::{Generated, Runtime, Unread};

/// Every type-specific property of the five types, and what reads it.
fn declared_readers() -> Vec<(ControlType, Vec<(&'static str, Reader)>)> {
    vec![
        (
            ControlType::RestClient,
            vec![
                ("BaseURL", Runtime),
                ("DefaultMethod", Runtime),
                ("AuthType", Runtime),
                ("AuthToken", Runtime),
                ("DefaultHeaders", Runtime),
                ("TimeoutSeconds", Runtime),
                ("FollowRedirects", Runtime),
                ("VerifyTLS", Runtime),
                ("RequestDataItem", Unread(
                    "no verb sends a named data item as the body; a handler passes \
                     the body to post()/put() as an argument instead",
                )),
                ("ResponseDataItem", Generated),
                ("StatusDataItem", Generated),
                ("Mode", Runtime),
                ("Busy", Runtime),
                ("TimeoutMs", Runtime),
            ],
        ),
        (
            ControlType::SqlDatabase,
            vec![
                ("Driver", Generated),
                ("ConnectionString", Generated),
                ("AutoConnect", Unread(
                    "nothing opens the connection when the form loads; a handler \
                     must call Open() explicitly",
                )),
                ("MaximumConnections", Unread(
                    "there is no connection pool; every Open() is its own connection",
                )),
                ("ConnectionDataItem", Unread(
                    "the generator names the handle itself; this override is not consulted",
                )),
                ("ResultSetDataItem", Unread(
                    "the generator names the result set itself; this override is not consulted",
                )),
                ("Mode", Runtime),
                ("Busy", Runtime),
                ("TimeoutMs", Runtime),
            ],
        ),
        (
            ControlType::WebSearch,
            vec![
                ("SearchEngineId", Runtime),
                ("Query", Runtime),
                ("NumResults", Runtime),
                ("SafeSearch", Runtime),
                ("Mode", Runtime),
                ("Busy", Runtime),
                ("TimeoutMs", Runtime),
            ],
        ),
        (
            ControlType::AgentObject,
            vec![
                ("AgentURL", Generated),
                ("AgentModel", Generated),
                ("AgentAPI", Unread("the provider protocol is never selected from it")),
                ("AgentAPIKey", Unread("no request reads the key off the control")),
                ("AgentEndpoint", Unread("the endpoint override is never applied")),
                ("SystemPrompt", Runtime),
                ("Temperature", Unread("never reaches a request")),
                ("MaximumTokens", Unread("never reaches a request")),
                ("Stream", Unread("responses are not streamed")),
                ("TimeoutSeconds", Runtime),
                ("TargetControls", Unread(
                    "the write allow-list is not enforced against this property",
                )),
                ("ResponseDataItem", Generated),
            ],
        ),
        (
            ControlType::IndexedFile,
            vec![
                ("IndexedFile", Generated),
                ("OpenMode", Generated),
                ("LoadStrategy", Generated),
                ("AutoOpen", Generated),
                ("RecordName", Generated),
                ("KeyName", Generated),
                ("CurrentKeyDataItem", Generated),
                ("StatusDataItem", Generated),
                ("CurrentRecordDataItem", Unread(
                    "the generator names the record item itself; this override is not consulted",
                )),
                ("OperatorName", Generated),
                ("Mode", Runtime),
                ("Busy", Runtime),
                ("TimeoutMs", Runtime),
            ],
        ),
    ]
}

/// The properties `Control::new` gives **every** control whatever its type —
/// the universal appearance/geometry block. Computed as the intersection over
/// the whole catalogue so it cannot drift out of date.
fn universal_properties() -> BTreeSet<String> {
    let mut sets = ControlType::ALL.iter().map(|t| {
        Control::new("probe", t.clone(), 0, 0)
            .properties
            .keys()
            .cloned()
            .collect::<BTreeSet<String>>()
    });
    let first = sets.next().unwrap_or_default();
    sets.fold(first, |acc, s| acc.intersection(&s).cloned().collect())
}

/// The properties a type seeds beyond the universal block.
fn type_specific_properties(t: &ControlType, universal: &BTreeSet<String>) -> BTreeSet<String> {
    Control::new("probe", t.clone(), 0, 0)
        .properties
        .keys()
        .filter(|k| !universal.contains(*k))
        .cloned()
        .collect()
}

#[test]
fn every_non_visual_property_declares_who_reads_it() {
    let universal = universal_properties();
    let mut problems = Vec::new();

    for (ct, declared) in declared_readers() {
        let seeded = type_specific_properties(&ct, &universal);
        let named: BTreeSet<String> = declared.iter().map(|(n, _)| (*n).to_owned()).collect();

        for missing in seeded.difference(&named) {
            problems.push(format!(
                "{ct:?}::{missing} is seeded by Control::new but declares no reader. \
                 Add it to this table saying what reads it — and if the answer is \
                 nothing, it is Unread and the property pane is offering a promise \
                 the product does not keep."
            ));
        }
        for stale in named.difference(&seeded) {
            problems.push(format!(
                "{ct:?}::{stale} is in this table but is no longer seeded by \
                 Control::new — drop the row."
            ));
        }
    }

    assert!(problems.is_empty(), "\n{}\n", problems.join("\n\n"));
}

#[test]
fn every_property_declared_runtime_read_is_actually_read_by_the_runtime() {
    let mut orphans = Vec::new();

    for (ct, declared) in declared_readers() {
        for (name, reader) in declared {
            if reader != Runtime {
                continue;
            }
            let quoted = format!("\"{name}\"");
            if !RUNTIME_SOURCES.iter().any(|s| s.contains(&quoted)) {
                orphans.push(format!(
                    "{ct:?}::{name} is declared Runtime-read, but no runtime source \
                     mentions {quoted}. Either the read was refactored away — which \
                     makes the property dead again — or it moved to a file this test \
                     does not scan."
                ));
            }
        }
    }

    assert!(orphans.is_empty(), "\n{}\n", orphans.join("\n\n"));
}

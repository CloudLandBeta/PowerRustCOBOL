// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `add_crate` and `remove_crate` — register a third-party Rust crate so that
//! `EXEC RUST` blocks can `use` it (spec 044), the way the IDE's Project's
//! Crates dialog does.
//!
//! Until now only that dialog could, and a session that could not reach the IDE
//! could not generate Rust code that names a crate (operator, 2026-10-11). The
//! tools run the dialog's own code ([`crate::external_crates`], moved down from
//! the IDE rather than copied): resolve on the registry, vendor into the
//! project's `crates/`, judge the clash with cargo's resolver, record the pin.
//! The pin goes through the host, which records it the way its owner of the
//! manifest would — the IDE through its project model, `rcrun mcp` by editing
//! the file.

use cobolt_compiler::external_crates::{lib_name, VENDOR_SUBDIR};
use cobolt_compiler::ExternalCrate;
use serde_json::{json, Value};

use super::opt_str;
use crate::external_crates::{self as registry_core, AddOutcome, Note, Registry};
use crate::host::ProjectHost;
use crate::root::ProjectRoot;

/// The project's registered crates, as the answers list them.
fn registered(pins: &[ExternalCrate]) -> Vec<Value> {
    pins.iter()
        .map(|p| {
            let mut row = json!({
                "name": p.name,
                "version": p.version,
                "use_as": p.lib_name(),
                "features": p.features,
            });
            if let Some(alias) = &p.alias {
                row["alias"] = json!(alias);
            }
            row
        })
        .collect()
}

fn refuse_if_unsaved<H: ProjectHost>(host: &H, root: &ProjectRoot) -> Result<(), String> {
    if host.unsaved(root.manifest()) {
        return Err(
            "the project has unsaved changes in the IDE; ask the developer to save it, then try again"
                .to_owned(),
        );
    }
    Ok(())
}

pub fn add<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let name = opt_str(args, "name").ok_or("add_crate needs a 'name'")?;
    let requirement = opt_str(args, "version");
    let alias = opt_str(args, "alias");
    let features: Vec<String> = args
        .get("features")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    refuse_if_unsaved(host, root)?;

    let mut pins = host.crate_pins()?;
    // The cheap refusal first: it must not cost the resolver runs below.
    if pins.iter().any(|c| lib_name(&c.name) == lib_name(&name)) {
        return Err(format!(
            "`{}` is already registered; remove_crate it first to register it again",
            lib_name(&name)
        ));
    }
    let client = Registry::with_user_agent(
        &host.crates_registry(),
        &format!("PowerRustCOBOL/{} (add_crate)", host.version()),
    );
    let workspace = host.workspace_root();
    // Spec 045 R11 — what the application already links is refused before any
    // network call, as the dialog does. Without a platform tree there is no
    // closure to read, and the resolver probe below says so in words.
    let system = cobolt_compiler::resolve_workspace_root(workspace.clone())
        .and_then(|w| registry_core::system_closure(&w).ok());

    let mut notes: Vec<String> = Vec::new();
    let mut log = |n: Note| {
        notes.push(match n {
            Note::Info(text) => text,
            Note::Warn(text) => format!("warning: {text}"),
        })
    };
    let outcome = {
        let mut save = |p: &[ExternalCrate]| host.record_crates(p);
        registry_core::add(
            &client,
            root.dir(),
            &mut pins,
            workspace.clone(),
            &name,
            requirement.as_deref(),
            features,
            system.as_ref(),
            &mut log,
            &mut save,
        )?
    };
    let message = match outcome {
        AddOutcome::Added(message) => message,
        // Spec 045 R1 — the name is one the application links directly, at a
        // version that cannot be shared. The dialog asks the developer for an
        // alias; here the caller gives it, or the download is cleared.
        AddOutcome::AliasOffered { candidate, linked_requirement, vendored } => match alias {
            Some(alias) => {
                let mut save = |p: &[ExternalCrate]| host.record_crates(p);
                match registry_core::confirm_alias(
                    root.dir(),
                    &mut pins,
                    workspace,
                    candidate,
                    &alias,
                    &mut log,
                    &mut save,
                ) {
                    Ok(message) => message,
                    Err(e) => {
                        let _ = registry_core::discard_alias_offer(&vendored);
                        return Err(e);
                    }
                }
            }
            None => {
                registry_core::discard_alias_offer(&vendored)?;
                return Err(format!(
                    "`{name}` has the name of a crate the application links directly ({linked_requirement}), \
                     at a version the two cannot share. Call add_crate again with an `alias` — the name the \
                     blocks will `use` — to register it under that name."
                ));
            }
        },
    };

    let added = pins.last().cloned().ok_or("the crate was not recorded")?;
    Ok(json!({
        "added": added.name,
        "version": added.version,
        "use_as": added.lib_name(),
        "vendored": format!("{VENDOR_SUBDIR}/{}-{}", added.name, added.version),
        "message": message,
        "notes": notes,
        "registered": registered(&pins),
    }))
}

pub fn remove<H: ProjectHost>(host: &mut H, root: &ProjectRoot, args: &Value) -> Result<Value, String> {
    let name = opt_str(args, "name").ok_or("remove_crate needs a 'name'")?;
    refuse_if_unsaved(host, root)?;
    let mut pins = host.crate_pins()?;
    let message = {
        let mut save = |p: &[ExternalCrate]| host.record_crates(p);
        registry_core::remove(root.dir(), &mut pins, &name, &mut save)?
    };
    Ok(json!({
        "removed": lib_name(&name),
        "message": message,
        "registered": registered(&pins),
    }))
}

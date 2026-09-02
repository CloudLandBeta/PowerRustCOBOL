// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Bind the DataGrid demo to the `actors` indexed file.
//!
//! The demo had a DataGrid with its columns configured and `DataSource` set to
//! `ACTORS / ACTORS`, and **no data binding at all** — so nothing ever
//! populated it. `DataSource` names a source; a `DataBindingDef` is what moves
//! the records into the grid, and codegen only emits
//! `COBOL-DATA-BINDINGS-POPULATE` when the form has one.
//!
//! Field layout comes from `misc/load-actors-idx.cbl`'s FD — the program that
//! wrote the file — so the binding cannot disagree with the data.

use cobolt_forms::model::{
    BindingDataType, BindingField, BindingSourceDescriptor, BindingTargetDescriptor,
    BindingTargetPath, DataBindingDef, FieldMapping,
};
use cobolt_forms::{load_form, save_form};
use std::path::Path;

/// `(field, display, type, mask, is_key)` — the FD of `load-actors-idx.cbl`.
const FIELDS: &[(&str, &str, BindingDataType, &str, bool)] = &[
    ("ACTOR-ID", "Actor Id", BindingDataType::Integer, "9(9)", true),
    ("ACTOR-THUMB", "Actor Thumb", BindingDataType::Text, "X(60)", false),
    ("ACTOR-CAPTION", "Actor Caption", BindingDataType::Text, "X(30)", false),
    ("ACTOR-SALARY", "Actor Salary", BindingDataType::Decimal, "9(9)V99", false),
    ("ACTOR-AWARDS", "Actor Awards", BindingDataType::Text, "X", false),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let Some(form_path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: bind_actors_grid <datagrid-form.cfrm> [--dry-run]");
        std::process::exit(2);
    };
    let form_path = Path::new(form_path);

    let mut form = match load_form(form_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read {}: {e}", form_path.display());
            std::process::exit(1);
        }
    };

    let Some(grid) = form
        .controls
        .iter()
        .find(|c| c.control_type == cobolt_forms::ControlType::DataGrid)
        .map(|c| c.id.clone())
    else {
        eprintln!("no DataGrid on {}", form_path.display());
        std::process::exit(1);
    };
    println!("  DataGrid: {grid}");

    // Idempotent: a binding for this grid already present is replaced, not
    // added beside itself.
    let before = form.data_bindings.len();
    form.data_bindings.retain(|b| {
        !matches!(&b.target, BindingTargetDescriptor::DataGrid { control_id } if *control_id == grid)
    });
    if before != form.data_bindings.len() {
        println!("  replaced an existing binding for this grid");
    }

    let fields: Vec<BindingField> = FIELDS
        .iter()
        .map(|(name, display, ty, mask, key)| {
            let mut f = BindingField::new(*name, ty.clone());
            f.display_name = (*display).to_owned();
            f.cobol_mask = (*mask).to_owned();
            f.key = *key;
            f
        })
        .collect();

    let source = BindingSourceDescriptor::IndexedFile {
        // Relative to the project root, as the IDE writes them.
        definition_path: "indexed/actors.cidx".to_owned(),
        record_name: "ACTORS-RECORD".to_owned(),
        key_field: Some("ACTOR-ID".to_owned()),
        fields: fields.clone(),
        // The demo READS the file; nothing about it is a place to type.
        writable: false,
    };
    let target = BindingTargetDescriptor::DataGrid {
        control_id: grid.clone(),
    };

    // One mapping per field, onto the column of the same name. The grid's
    // `AdvancedGrid` column ids ARE the COBOL field names, which is what makes
    // this a straight pairing rather than a guess.
    let mappings: Vec<FieldMapping> = FIELDS
        .iter()
        .map(|(name, ..)| {
            FieldMapping::new(
                *name,
                BindingTargetPath::GridColumn {
                    control_id: grid.clone(),
                    column_id: (*name).to_owned(),
                },
            )
        })
        .collect();

    let mut def = DataBindingDef::new(
        format!("bind-actors-{}", grid.to_lowercase()),
        format!("ACTORS -> {grid}"),
        source,
        target,
    )
    .with_mappings(mappings);
    // What the source looked like when the binding was made, so the IDE can
    // tell later that the schema moved.
    def.saved_source_metadata.fields = fields;

    for m in &def.mappings {
        println!("    {} -> {:?}", m.source_field, m.target);
    }
    form.data_bindings.push(def);
    println!(
        "  {} binding(s) on the form{}",
        form.data_bindings.len(),
        if dry_run { "  [DRY RUN — nothing written]" } else { "" }
    );
    if !dry_run {
        if let Err(e) = save_form(&form, form_path) {
            eprintln!("failed to write {}: {e}", form_path.display());
            std::process::exit(1);
        }
    }
}

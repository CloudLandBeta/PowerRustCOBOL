// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Write-back of a repeating group (spec 024 R17–R19): what the operator types into
//! a member of a bound card is written to the COBOL table the group is bound to.
//!
//! A repeating group bound to a COBOL table draws one card per occurrence, and each
//! card's members are drawn with their own ids, `"<group>.<group>-<n>.<member>"`
//! (`cobolt_forms::render::member_instance_id`). When the operator edits a member the
//! host reports the new value under that id, and [`Interpreter::drain_input`] hands
//! it here before it is stored as a property:
//!
//! - **Writable binding** (`_BindingWritable = 1`, seeded from the binding's mode):
//!   the value goes into the occurrence `n` of the source field the member's property
//!   is mapped to — and into that field only, so every other column of the row is
//!   untouched (R17). `CommitStatus` becomes `00` and `LastError` empty.
//! - **Read-only binding:** nothing is written (R18). `CommitStatus` becomes `91`.
//! - **A value the field cannot hold** — text typed into a numeric item — is not
//!   written. `CommitStatus` becomes `92`, `LastError` says which field and why, and
//!   the edit stays in `PendingEdits` (one `n`, member, property, value line each)
//!   until the same member is edited to something acceptable (R19). The member still
//!   shows what was typed, so the operator can repair it; `RefreshBinding()` re-reads
//!   the table and discards the pending edits.
//!
//! The codes are the DataGrid's own (`grid_edit`): `00` written, `91` not writable,
//! `92` the value is not acceptable for the field.

use crate::value::CobolValue;

use super::Interpreter;

use cobolt_forms::instance_id::parse_member_instance_id;

/// The rows of a group's `_BindingMappings` (`field\tmember\tproperty` each). The host
/// seeds them one per line; generated COBOL cannot put a line break inside a literal,
/// so it separates them with `;` — a character no field, control or property name holds.
pub(super) fn binding_mapping_rows(maps: &str) -> impl Iterator<Item = &str> {
    maps.split(['\n', ';'])
}

impl Interpreter {
    /// An edit reported under the drawn id of a member of a repeating group: write it
    /// back when that group is bound to a table. `true` when the edit was one of
    /// those (whatever became of it), so the caller still stores the property.
    pub(super) fn note_array_member_edit(&mut self, id: &str, prop: &str, value: &str) -> bool {
        let Some(edit) = parse_member_instance_id(id) else {
            return false;
        };
        if self.obj_get(edit.group, "_BindingArray").trim() != "1" {
            return false;
        }
        // The field this member's property is mapped to.
        let mappings = self.obj_get(edit.group, "_BindingMappings");
        let Some(field) = binding_mapping_rows(&mappings).find_map(|line| {
            let mut cells = line.split('\t');
            let (field, member, property) = (cells.next()?, cells.next()?, cells.next()?);
            (member.trim().eq_ignore_ascii_case(edit.member) && property.trim().eq_ignore_ascii_case(prop))
                .then(|| field.trim().to_owned())
        }) else {
            // A property of the member the binding does not map: not the binding's business.
            return false;
        };

        if self.obj_get(edit.group, "_BindingWritable").trim() != "1" {
            self.array_edit_outcome(edit.group, "91", format!("{field}: the binding is read-only, nothing was written"));
            return true;
        }
        let capacity = self.env.symbol(&field).and_then(|s| s.dims.last().copied()).unwrap_or(0);
        if edit.instance > capacity {
            self.array_edit_refused(&edit, &field, prop, value, format!("{field}: the table has room for {capacity} rows, not {}", edit.instance));
            return true;
        }
        let item = crate::environment::subscript_key(&field, &[edit.instance as i64]);
        let text = value.trim_end();
        if matches!(self.env.get(&item), Some(CobolValue::Numeric(_))) && !text.trim().is_empty() && text.trim().parse::<f64>().is_err() {
            self.array_edit_refused(&edit, &field, prop, value, format!("{field}: '{}' is not a number", text.trim()));
            return true;
        }
        self.env.set_str(&item, text);
        self.array_edit_forget(&edit, prop);
        self.array_edit_outcome(edit.group, "00", String::new());
        true
    }

    fn array_edit_outcome(&mut self, group: &str, status: &str, error: String) {
        self.obj_set(group, "CommitStatus", status.to_owned());
        self.obj_set(group, "LastError", error);
    }

    /// An edit the table could not take: the status, and the edit kept for repair.
    fn array_edit_refused(&mut self, edit: &cobolt_forms::instance_id::MemberInstance<'_>, field: &str, prop: &str, value: &str, why: String) {
        let _ = field;
        self.array_edit_forget(edit, prop);
        let mut pending = self.obj_get(edit.group, "PendingEdits");
        if !pending.is_empty() {
            pending.push('\n');
        }
        pending.push_str(&format!("{}\t{}\t{}\t{}", edit.instance, edit.member, prop, value));
        self.obj_set(edit.group, "PendingEdits", pending);
        self.array_edit_outcome(edit.group, "92", why);
    }

    /// A member edited to something the table took no longer waits for repair.
    fn array_edit_forget(&mut self, edit: &cobolt_forms::instance_id::MemberInstance<'_>, prop: &str) {
        let pending = self.obj_get(edit.group, "PendingEdits");
        if pending.is_empty() {
            return;
        }
        let kept: Vec<&str> = pending
            .lines()
            .filter(|line| {
                let mut cells = line.split('\t');
                let (n, member, property) = (cells.next(), cells.next(), cells.next());
                !(n.and_then(|n| n.parse::<usize>().ok()) == Some(edit.instance)
                    && member.is_some_and(|m| m.eq_ignore_ascii_case(edit.member))
                    && property.is_some_and(|p| p.eq_ignore_ascii_case(prop)))
            })
            .collect();
        self.obj_set(edit.group, "PendingEdits", kept.join("\n"));
    }

    /// `RefreshBinding()` re-reads the table: what was waiting for repair is dropped.
    pub(super) fn array_edit_discard_all(&mut self, group: &str) {
        if !self.obj_get(group, "PendingEdits").is_empty() {
            self.obj_set(group, "PendingEdits", String::new());
        }
    }
}

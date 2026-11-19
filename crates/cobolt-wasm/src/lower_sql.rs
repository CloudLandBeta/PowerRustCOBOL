// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `EXEC SQL` in a desktop module (DESKTOP-WASM-001 T41, ABI A41).
//!
//! The runner runs the statement with the interpreter's own SQL code, in a small interpreter built
//! from the program's declarations (the `prsql` section). The module keeps the host variables: each
//! statement names its **cells**, the elementary items it may read or write, and
//! `cobolt_wasm_rt::sql` sends their values, calls `prn.sql_exec` and stores what changed. Then the
//! generated code follows the statement's `WHENEVER`.
//!
//! The cells are found as the interpreter finds its items (`exec_sql.rs`): a host variable that is a
//! group stands for its elementary items, without `FILLER`, `REDEFINES` and 88-levels; an indicator
//! of a host structure is the first occurrences of its table; the status items are every item named
//! `SQLSTATE`, `SQLCODE`, `SQLMSG` or one of the SQLCA fields. Whatever the interpreter would
//! answer with a run-time error of its own (a name it cannot find, a table inside a host
//! structure) is refused here instead, so the Build takes the Cargo path and the interpreter's
//! answer stays the only one.

use crate::error::{ErrorKind, GenResult};
use crate::lower::Lower;
use cobolt_ast::sql::{ExecSql, SqlCursorQuery, SqlHostName, SqlHostRef, SqlInto, SqlKind, SqlUsing, SqlValue};
use cobolt_ast::expr::{Expr, Literal};
use cobolt_wasm_abi::sql::{SqlCell, SqlProgram, SqlStatement};
use wasm_encoder::Instruction as I;

/// The leaf names whose items the interpreter sets after every statement (`sql_set_status`).
const STATUS_LEAVES: &[&str] = &[
    "SQLSTATE",
    "SQLCODE",
    "SQLMSG",
    "SQLCA-MESSAGE",
    "SQLCA-MESSAGE-LENGTH",
    "SQLCA-ROWS",
    "SQLCA-NATIVE-CODE",
    "SQLCA-WARNING",
    "SQLCA-TRUNCATED",
    "SQLCA-CONNECTION",
];

const D_DESCRIPTOR: &str = "an SQL descriptor area (DESCRIBE, USING or INTO DESCRIPTOR) is not computed in a WebAssembly module yet: the interpreter fills its entries by their names and reaches the host variables through the pointers in its table, and a module's pointers are whole items outside any table, so the Build takes the Cargo path (DESKTOP-WASM-001 M6)";
const D_FILE_RECORD: &str = "is in a file record of a program containing this one, which the runner's copy of the declarations does not carry";

/// One cell: an item, and the subscripts of the occurrence (an indicator table's).
type Cell = (usize, Vec<i64>);

impl Lower<'_, '_> {
    /// `EXEC SQL` in a desktop module.
    pub(crate) fn exec_sql_native(&mut self, e: &ExecSql, line: u32) -> GenResult<()> {
        if matches!(e.kind, SqlKind::Declarative) {
            return Ok(());
        }
        let refuse = |why: &str| -> GenResult<()> { Err(crate::error::GenError::kinded(line, ErrorKind::Other, "EXEC SQL", format!("EXEC SQL: {why}"))) };
        // What the statement reads and what it writes.
        let mut reads: Vec<&SqlHostRef> = Vec::new();
        let mut targets: Vec<&SqlHostRef> = Vec::new();
        let prog = self.programs[self.cur_unit];
        match &e.kind {
            SqlKind::Declarative | SqlKind::Close { .. } | SqlKind::Commit | SqlKind::Rollback => {}
            SqlKind::Execute(t) => reads.extend(&t.inputs),
            SqlKind::SelectInto { text, into } => {
                reads.extend(&text.inputs);
                targets.extend(into.iter());
            }
            SqlKind::Open { cursor, using } => {
                let declared = prog.sql_cursors.iter().find(|c| c.name.eq_ignore_ascii_case(cursor.as_str()));
                if let Some(SqlCursorQuery::Static(t)) = declared.map(|c| &c.query) {
                    reads.extend(&t.inputs);
                }
                match using {
                    SqlUsing::None => {}
                    SqlUsing::Hosts(h) => reads.extend(h.iter()),
                    SqlUsing::Descriptor(_) => return refuse(D_DESCRIPTOR),
                }
            }
            SqlKind::Fetch { into, .. } => match into {
                SqlInto::Hosts(h) => targets.extend(h.iter()),
                SqlInto::Descriptor(_) => return refuse(D_DESCRIPTOR),
            },
            SqlKind::Connect { target, user, password, .. } => {
                reads.extend([Some(target), user.as_ref(), password.as_ref()].into_iter().flatten().filter_map(host));
            }
            SqlKind::SetConnection(v) | SqlKind::Prepare { from: v, .. } | SqlKind::ExecuteImmediate(v) => reads.extend(host(v)),
            SqlKind::Disconnect(d) => {
                if let cobolt_ast::sql::SqlDisconnect::Named(v) = d {
                    reads.extend(host(v));
                }
            }
            SqlKind::ExecutePrepared { using, .. } => match using {
                SqlUsing::None => {}
                SqlUsing::Hosts(h) => reads.extend(h.iter()),
                SqlUsing::Descriptor(_) => return refuse(D_DESCRIPTOR),
            },
            SqlKind::Describe { .. } => return refuse(D_DESCRIPTOR),
        }
        // The cells, each once, in the order the statement meets them.
        let mut cells: Vec<Cell> = Vec::new();
        let mut add = |c: Cell, cells: &mut Vec<Cell>| {
            if !cells.contains(&c) {
                cells.push(c);
            }
        };
        for h in reads.iter().chain(targets.iter()) {
            let idx = match self.sql_item(&h.var) {
                Ok(i) => i,
                Err(why) => return refuse(&why),
            };
            let structure = self.layout.items[idx].is_group;
            let leaves = match self.sql_leaves(idx) {
                Ok(l) => l,
                Err(why) => return refuse(&why),
            };
            let n = leaves.len();
            for l in leaves {
                add((l, Vec::new()), &mut cells);
            }
            if let Some(ind) = &h.indicator {
                match self.sql_indicators(ind, n, structure) {
                    Ok(list) => list.into_iter().for_each(|c| add(c, &mut cells)),
                    Err(why) => return refuse(&why),
                }
            }
        }
        for leaf in STATUS_LEAVES {
            for idx in self.layout.named(leaf).to_vec() {
                let it = &self.layout.items[idx];
                if it.hidden || !self.layout.visible(idx) || it.level == 88 || it.level == 66 || it.is_group {
                    continue;
                }
                if self.sql_in_table(idx) {
                    return refuse(&format!("the status item {leaf} is inside a table"));
                }
                add((idx, Vec::new()), &mut cells);
            }
        }

        // The values and the answer must fit the routines' scratch area.
        let lens: u32 = cells.iter().map(|(i, _)| cobolt_wasm_rt::sql::cell_bound(self.layout.items[*i].size)).sum();
        let answer = 5 + lens + 4 * cells.len() as u32;
        if lens.max(answer) > cobolt_wasm_rt::state::SCRATCH_SIZE {
            return refuse(&format!("its host variables hold {lens} bytes, more than a module statement can carry"));
        }

        // The section entry.
        let mut section_cells = Vec::with_capacity(cells.len());
        for (idx, _) in &cells {
            if !self.sql_carried(*idx) {
                return refuse(&format!("{} {D_FILE_RECORD}", self.layout.items[*idx].name.as_deref().unwrap_or("FILLER")));
            }
        }
        for (idx, subs) in &cells {
            let mut quals = Vec::new();
            let mut p = self.layout.items[*idx].parent;
            while let Some(q) = p {
                if let Some(n) = &self.layout.items[q].name {
                    quals.push(n.clone());
                }
                p = self.layout.items[q].parent;
            }
            let name = self.layout.items[*idx].name.clone().unwrap_or_default();
            section_cells.push(SqlCell { name, quals, subs: subs.clone() });
        }
        let program = self.sql_program_number(line)?;
        let exec = serde_json::to_value(e).map_err(|err| crate::error::GenError::new(line, format!("EXEC SQL: {err}")))?;
        let stmt = self.names.sql_statements.len() as u32;
        self.names.sql_statements.push(SqlStatement { program, exec, cells: section_cells });

        // The call: the spec record, then the routine, which leaves the condition.
        let mut words = vec![stmt, cells.len() as u32];
        for (idx, subs) in &cells {
            let indices: Vec<Expr> = subs.iter().map(|s| Expr::Literal(Literal::Integer(*s), cobolt_lexer::Span::new(0, 0, line, 0))).collect();
            words.push(self.item_operand(*idx, &indices, None, line)?);
        }
        let spec = self.pool.spec(&words);
        self.base();
        self.i32c(spec as i32);
        self.call_rt("rt_sql_exec");
        self.sql_whenever(e, line)
    }

    /// `WHENEVER … GO TO`: the condition the statement ended in is on the stack.
    fn sql_whenever(&mut self, e: &ExecSql, line: u32) -> GenResult<()> {
        use cobolt_wasm_rt::sql::wire;
        let mut arms = Vec::new();
        for (code, target) in [(wire::OUT_ERROR, &e.whenever.sqlerror), (wire::OUT_WARNING, &e.whenever.sqlwarning), (wire::OUT_NOT_FOUND, &e.whenever.not_found)] {
            if let Some(t) = target {
                let p = self.goto_target(t, None, line)?;
                arms.push((code, self.entries[p]));
            }
        }
        if arms.is_empty() {
            self.raw(I::Drop);
            return Ok(());
        }
        let after = self.new_block("sql-continue");
        self.raw(I::LocalSet(0));
        for (code, t) in &arms {
            self.raw(I::LocalGet(0));
            self.i32c(*code as i32);
            self.raw(I::I32Eq);
            self.raw(I::If(wasm_encoder::BlockType::Result(wasm_encoder::ValType::I32)));
            self.emit(crate::ir::Ins::Pc(*t));
            self.raw(I::Else);
        }
        self.emit(crate::ir::Ins::Pc(after));
        for _ in &arms {
            self.raw(I::End);
        }
        let cur = self.cur.take().unwrap();
        self.blocks[cur].succ.push(after);
        self.blocks[cur].succ.extend(arms.iter().map(|(_, t)| *t));
        self.blocks[cur].done = true;
        self.begin(after);
        Ok(())
    }

    /// The item a host name names, trying both readings of `:A.B`, as the interpreter does.
    fn sql_item(&self, h: &SqlHostName) -> Result<usize, String> {
        if let Ok(i) = self.layout.lookup(&h.name, &h.quals) {
            return Ok(i);
        }
        if let Some((n, q)) = &h.swapped {
            if let Ok(i) = self.layout.lookup(n, q) {
                return Ok(i);
            }
        }
        Err(format!("host variable :{} is not declared", h.name))
    }

    /// Whether an item is, or lies inside, a table.
    fn sql_in_table(&self, idx: usize) -> bool {
        let mut p = Some(idx);
        while let Some(i) = p {
            if self.layout.items[i].occurs.is_some() {
                return true;
            }
            p = self.layout.items[i].parent;
        }
        false
    }

    /// The elementary items a host variable stands for (the interpreter's `host_structure_leaves`).
    fn sql_leaves(&self, idx: usize) -> Result<Vec<usize>, String> {
        let it = &self.layout.items[idx];
        if self.sql_in_table(idx) {
            return Err(format!("the host variable {} is inside a table", it.name.as_deref().unwrap_or("FILLER")));
        }
        if !it.is_group {
            return Ok(vec![idx]);
        }
        let mut out = Vec::new();
        self.sql_collect(idx, &mut out)?;
        Ok(out)
    }

    fn sql_collect(&self, idx: usize, out: &mut Vec<usize>) -> Result<(), String> {
        for &c in &self.layout.items[idx].children {
            let ci = &self.layout.items[c];
            if ci.redefines.is_some() || ci.level == 88 || ci.level == 66 || ci.name.is_none() {
                continue;
            }
            if ci.occurs.is_some() {
                return Err(format!("host structure {} holds {}, which has OCCURS", self.layout.items[idx].name.as_deref().unwrap_or("FILLER"), ci.name.as_deref().unwrap_or("FILLER")));
            }
            if ci.is_group {
                self.sql_collect(c, out)?;
            } else {
                out.push(c);
            }
        }
        Ok(())
    }

    /// The indicator cells for `n` items: the indicator itself, or for a host structure the first
    /// `n` occurrences of its table (the interpreter's `sql_indicator_keys`).
    fn sql_indicators(&self, ind: &SqlHostName, n: usize, structure: bool) -> Result<Vec<Cell>, String> {
        let idx = self.sql_item(ind)?;
        if !structure {
            if self.sql_in_table(idx) || self.layout.items[idx].is_group {
                return Err(format!("the indicator {} is not an elementary item outside a table", ind.name));
            }
            return Ok(vec![(idx, Vec::new())]);
        }
        let it = &self.layout.items[idx];
        let table = if it.is_group { it.children.iter().copied().find(|&c| self.layout.items[c].level != 88) } else { Some(idx) };
        let Some(table) = table else { return Err(format!("the indicator {} holds no table", ind.name)) };
        let t = &self.layout.items[table];
        let parent_in_table = t.parent.is_some_and(|p| self.sql_in_table(p));
        match t.occurs {
            Some((max, _)) if !t.is_group && !parent_in_table && n as u32 <= max => Ok((1..=n as i64).map(|i| (table, vec![i])).collect()),
            _ => Err(format!("the indicator {} is not a table of {n} elementary items", ind.name)),
        }
    }

    /// The number of the current program in the `prsql` section, adding it the first time.
    fn sql_program_number(&mut self, line: u32) -> GenResult<u32> {
        let prog = self.programs[self.cur_unit];
        let key = prog as *const cobolt_ast::program::Program as usize;
        if let Some(&n) = self.names.sql_program_keys.get(&key) {
            return Ok(n);
        }
        // The program's declarations and cursors; its procedure is not needed.
        let mut bare = prog.clone();
        bare.procedure.body = cobolt_ast::program::ProcedureBody::Paragraphs(Vec::new());
        bare.procedure.declaratives.clear();
        bare.nested_programs.clear();
        bare.rust_items.clear();
        // A contained program (a form's event handler) also sees the GLOBAL records of the programs
        // around it, and the cursors they declare in their DATA DIVISION. They join its own
        // declarations, after them, the nearest first, so a name it declares itself hides theirs.
        let scope = self.units[self.cur_unit].scope.clone();
        if scope.len() > 1 {
            let mut seen: std::collections::HashSet<String> =
                top_level(prog).filter_map(|d| d.name.as_ref().map(|n| n.to_ascii_uppercase())).collect();
            let mut globals = Vec::new();
            for &a in &scope[1..] {
                for d in global_records(self.programs[a as usize]) {
                    if d.name.as_ref().is_none_or(|n| seen.insert(n.to_ascii_uppercase())) {
                        globals.push(d.clone());
                    }
                }
            }
            if !globals.is_empty() {
                let span = cobolt_lexer::Span::new(0, 0, line, 0);
                let data = bare.data.get_or_insert_with(|| cobolt_ast::program::DataDivision { sections: Vec::new(), span });
                data.sections.push(cobolt_ast::program::DataSection::WorkingStorage(globals));
            }
            let mut cursors: Vec<cobolt_ast::sql::SqlCursor> = scope[1..]
                .iter()
                .rev()
                .flat_map(|&a| self.programs[a as usize].sql_cursors.iter().filter(|c| c.in_data_division).cloned())
                .collect();
            cursors.append(&mut bare.sql_cursors);
            bare.sql_cursors = cursors;
        }
        let program = serde_json::to_value(&bare).map_err(|err| crate::error::GenError::new(line, format!("EXEC SQL: {err}")))?;
        let n = self.names.sql_programs.len() as u32;
        self.names.sql_programs.push(SqlProgram { name: prog.identification.program_id.to_ascii_uppercase(), program });
        self.names.sql_program_keys.insert(key, n);
        Ok(n)
    }
}

impl Lower<'_, '_> {
    /// Whether item `idx` is in a record the runner's copy of the program's declarations carries:
    /// one of the program's own, or a `GLOBAL` record in the WORKING-STORAGE, LOCAL-STORAGE or
    /// LINKAGE SECTION of a program containing it.
    fn sql_carried(&self, idx: usize) -> bool {
        let mut r = idx;
        while let Some(p) = self.layout.items[r].parent {
            r = p;
        }
        let it = &self.layout.items[r];
        if it.unit == u32::MAX || it.unit as usize == self.cur_unit {
            return true;
        }
        let Some(name) = &it.name else { return false };
        global_records(self.programs[it.unit as usize]).any(|d| d.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name)))
    }
}

/// A program's records and level-77 items, in every section of its DATA DIVISION but the FILE
/// SECTION's records and the SCREEN SECTION.
fn top_level(p: &cobolt_ast::program::Program) -> impl Iterator<Item = &cobolt_ast::data::DataDecl> {
    use cobolt_ast::program::DataSection;
    p.data.iter().flat_map(|d| d.sections.iter()).flat_map(|s| match s {
        DataSection::WorkingStorage(v) | DataSection::LocalStorage(v) | DataSection::Linkage(v) => v.as_slice(),
        _ => &[],
    })
}

/// The `GLOBAL` records of a program's WORKING-STORAGE, LOCAL-STORAGE and LINKAGE sections.
fn global_records(p: &cobolt_ast::program::Program) -> impl Iterator<Item = &cobolt_ast::data::DataDecl> {
    top_level(p).filter(|d| d.is_global)
}

/// The host variable a value names, when it names one.
fn host(v: &SqlValue) -> Option<&SqlHostRef> {
    match v {
        SqlValue::Host(h) => Some(h),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    /// The runtime's copy of the values that cross `sql_exec` and `sql_db` is the ABI's: the runtime cannot link
    /// the ABI crate, so this crate, which links both, holds the two equal.
    #[test]
    fn the_runtime_speaks_the_abis_sql_wire() {
        use cobolt_wasm_abi::sql::wire as abi;
        use cobolt_wasm_rt::sql::wire as rt;
        let pairs = [
            ("DEC", abi::DEC, rt::DEC),
            ("FLOAT", abi::FLOAT, rt::FLOAT),
            ("BYTES", abi::BYTES, rt::BYTES),
            ("OUT_SUCCESS", abi::OUT_SUCCESS, rt::OUT_SUCCESS),
            ("OUT_WARNING", abi::OUT_WARNING, rt::OUT_WARNING),
            ("OUT_NOT_FOUND", abi::OUT_NOT_FOUND, rt::OUT_NOT_FOUND),
            ("OUT_ERROR", abi::OUT_ERROR, rt::OUT_ERROR),
        ];
        for (name, a, r) in pairs {
            assert_eq!(a, r, "wire::{name}: the ABI says {a}, the runtime {r}");
        }
        // The SQL built-ins (A42).
        use cobolt_wasm_abi::sql::db as abi_db;
        use cobolt_wasm_rt::sql::db as rt_db;
        let ops = [
            ("OPEN", abi_db::OPEN, rt_db::OPEN),
            ("EXEC", abi_db::EXEC, rt_db::EXEC),
            ("FETCH_ROW", abi_db::FETCH_ROW, rt_db::FETCH_ROW),
            ("NEXT_ROW", abi_db::NEXT_ROW, rt_db::NEXT_ROW),
            ("ROW_COUNT", abi_db::ROW_COUNT, rt_db::ROW_COUNT),
            ("CLOSE", abi_db::CLOSE, rt_db::CLOSE),
            ("NUMBER", u32::from(abi_db::NUMBER), u32::from(rt_db::NUMBER)),
            ("TEXT", u32::from(abi_db::TEXT), u32::from(rt_db::TEXT)),
        ];
        for (name, a, r) in ops {
            assert_eq!(a, r, "db::{name}: the ABI says {a}, the runtime {r}");
        }
        println!(
            "prsql wire: {} of {} constants equal in the ABI and the runtime; SQL built-ins: {} of {}",
            pairs.len(),
            pairs.len(),
            ops.len(),
            ops.len()
        );
    }
}

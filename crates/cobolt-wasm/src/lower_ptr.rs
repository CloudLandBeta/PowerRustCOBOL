// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `USAGE POINTER`, `SET ptr … TO ADDRESS OF item` and `SET ADDRESS OF item TO {ADDRESS OF item |
//! ptr | NULL}` (DESKTOP-WASM-001 T45b), computed in a desktop module as the interpreter computes
//! them. `SET ptr TO NULL` and `SET ptr TO ptr-2` are `MOVE`s for both engines (the parser makes
//! them so); `NULL` against a number is the number 0 (`cobolt_wasm_rt::cmp`).
//!
//! The interpreter's pointer holds a number: 1, 2, … in the order the program first takes the
//! address of each item, 0 for `NULL`. Its `SET ADDRESS OF item` makes every later reference to
//! `item` a reference to the other item, by name. A module does the same with addresses: the
//! program's table (`cobolt_wasm_rt::ptr`) hands out the numbers, and `SET ADDRESS OF` stores the
//! address in the LINKAGE item's slot, through which every reference to it already goes.
//!
//! The two readings agree when the LINKAGE item and every item whose address it can take are
//! described alike, when the LINKAGE item is an elementary record no other record redefines, and
//! when the item whose address is taken is named whole (the interpreter numbers a table, not its
//! element). The phase-A scan ([`Lower::ptr_scan`]) checks each `SET` against these rules before
//! anything is lowered, and refuses the rest at its line, so the Build takes the Cargo path. The
//! scan reports every refusal with the program's others, as the statements' lowering does.

use crate::error::{ErrorKind, GenResult};
use crate::lower::Lower;
use cobolt_ast::data::Usage;
use cobolt_ast::expr::Expr;
use cobolt_ast::stmt::{PointerSource, Stmt};

/// Why a pointer statement of a called program is refused.
const D_PTR_UNIT: &str = "pointers are computed in a WebAssembly module in the main program only; in a called program the interpreter numbers addresses apart, so the Build takes the Cargo path";
/// Why `SET ADDRESS OF` of any other item is refused.
const D_PTR_BASED: &str = "SET ADDRESS OF in a WebAssembly module names an elementary 01 or 77 item of the LINKAGE SECTION that no other record redefines; the interpreter moves only the name it is given, so the Build takes the Cargo path";
/// Why `ADDRESS OF` any other item is refused.
const D_PTR_ITEM: &str = "ADDRESS OF in a WebAssembly module names a whole data item outside the LINKAGE SECTION that is not in a table, with no subscript or reference modification; the interpreter numbers any other the same as the item it lies in, so the Build takes the Cargo path";
/// Why any other pointer operand is refused.
const D_PTR_POINTER: &str = "a pointer in a WebAssembly module is a USAGE POINTER item outside the LINKAGE SECTION, named whole or with every subscript of its table, so the Build takes the Cargo path";
/// Why an item described otherwise than the LINKAGE item it can reach is refused.
const D_PTR_SHAPE: &str = "in a WebAssembly module an item whose address a LINKAGE item takes is described as that item is (an elementary item of the same PICTURE and USAGE): the interpreter reads the LINKAGE item with the other item's description, a module with its own, so the Build takes the Cargo path";

impl<'a, 'b> Lower<'a, 'b> {
    /// Phase A, every unit: check each pointer statement against the rules of this module, and
    /// gather the items whose address the main program takes (they number the table). A statement
    /// that breaks a rule is reported at its line and skipped when the statements are lowered, and
    /// the scan goes on, so one run names every refusal, the program's other constructs included.
    pub(crate) fn ptr_scan(&mut self, unit: usize) -> GenResult<()> {
        // A browser module refuses every pointer statement at the statement (`support`).
        if !self.native {
            return Ok(());
        }
        let mut sets: Vec<Stmt> = Vec::new();
        for p in &self.paras {
            for s in p.stmts {
                s.walk(&mut |x| {
                    if let Stmt::SetPointer { .. } = x {
                        sets.push(x.clone());
                    }
                });
            }
        }
        if unit != 0 {
            for s in &sets {
                let line = s.span().line;
                let r = self.err_kind(line, ErrorKind::Other, "SET", format!("SET (pointer): {D_PTR_UNIT}"));
                self.ptr_record(line, r);
            }
            return Ok(());
        }
        // The LINKAGE items a pointer's value can point; the items whose address goes into a pointer.
        let mut bound: Vec<usize> = Vec::new();
        let mut taken_lines: Vec<u32> = Vec::new();
        for s in &sets {
            let Stmt::SetPointer { address_of, targets, source, span } = s else { continue };
            let r = self.ptr_check(address_of.as_ref(), targets, source, span.line, &mut bound, &mut taken_lines);
            self.ptr_record(span.line, r);
        }
        // Any address a pointer holds can reach any LINKAGE item a pointer is given to.
        for (n, &x) in self.ptr_items.clone().iter().enumerate() {
            if bound.iter().any(|&lk| !self.ptr_same_shape(lk, x)) {
                let r = self.ptr_refuse(taken_lines[n], x, D_PTR_SHAPE);
                self.ptr_record(taken_lines[n], r);
            }
        }
        Ok(())
    }

    /// One pointer statement against the rules (see [`Self::ptr_scan`]).
    fn ptr_check(
        &mut self,
        address_of: Option<&Expr>,
        targets: &[Expr],
        source: &PointerSource,
        line: u32,
        bound: &mut Vec<usize>,
        taken_lines: &mut Vec<u32>,
    ) -> GenResult<()> {
        match address_of {
            Some(lk) => {
                let lk = self.ptr_based(lk, line)?;
                match source {
                    PointerSource::Null => {}
                    PointerSource::AddressOf(x) => {
                        let x = self.ptr_taken(x, line)?;
                        if !self.ptr_same_shape(lk, x) {
                            return self.ptr_refuse(line, x, D_PTR_SHAPE);
                        }
                    }
                    PointerSource::Pointer(p) => {
                        self.ptr_pointer(p, line)?;
                        bound.push(lk);
                    }
                }
            }
            None => {
                for t in targets {
                    self.ptr_pointer(t, line)?;
                }
                // `SET ptr TO NULL` and `SET ptr TO ptr-2` are parsed as `MOVE`s.
                let PointerSource::AddressOf(x) = source else {
                    return self.err_kind(line, ErrorKind::Other, "SET", format!("SET (pointer): {D_PTR_POINTER}"));
                };
                let x = self.ptr_taken(x, line)?;
                if !self.ptr_items.contains(&x) {
                    self.ptr_items.push(x);
                    taken_lines.push(line);
                }
            }
        }
        Ok(())
    }

    /// A refused pointer statement: reported with the program's other refusals, and skipped when the
    /// statements are lowered (the module is not made).
    fn ptr_record(&mut self, line: u32, r: GenResult<()>) {
        if let Err(mut e) = r {
            if e.program.is_empty() {
                e.program = self.units[self.cur_unit].name.clone();
            }
            self.diags.push(e);
            if !self.ptr_refused.contains(&(self.cur_unit, line)) {
                self.ptr_refused.push((self.cur_unit, line));
            }
        }
    }

    /// The statement: its rules were checked by [`Self::ptr_scan`].
    pub(crate) fn set_pointer(&mut self, address_of: Option<&Expr>, targets: &[Expr], source: &PointerSource, line: u32) -> GenResult<()> {
        let area = self.ptr_area();
        if let Some(lk) = address_of {
            let i = self.resolve_item(lk)?;
            let Some(slot) = self.layout.items[i].link else {
                return self.ptr_refuse(line, i, D_PTR_BASED);
            };
            match source {
                PointerSource::Null => {
                    self.base();
                    self.i32c(slot as i32);
                    self.call_rt("rt_ptr_unbind");
                }
                PointerSource::AddressOf(x) => {
                    let o = self.ptr_operand(x, line)?;
                    self.base();
                    self.i32c(slot as i32);
                    self.i32c(o as i32);
                    self.call_rt("rt_ptr_point");
                }
                PointerSource::Pointer(p) => {
                    let o = self.ptr_operand(p, line)?;
                    self.base();
                    self.i32c(slot as i32);
                    self.i32c(o as i32);
                    self.i32c(area as i32);
                    self.call_rt("rt_ptr_bind");
                }
            }
            return Ok(());
        }
        let PointerSource::AddressOf(x) = source else {
            return self.err_kind(line, ErrorKind::Other, "SET", format!("SET (pointer): {D_PTR_POINTER}"));
        };
        let i = self.resolve_item(x)?;
        let Some(n) = self.ptr_items.iter().position(|&j| j == i) else {
            return self.ptr_refuse(line, i, D_PTR_ITEM);
        };
        let k = self.ptr_items.len() as i32;
        let o = self.ptr_operand(x, line)?;
        for t in targets {
            let to = self.ptr_operand(t, line)?;
            self.base();
            self.i32c(to as i32);
            self.i32c(o as i32);
            self.i32c(area as i32);
            self.i32c(k);
            self.i32c(n as i32);
            self.call_rt("rt_ptr_take");
        }
        Ok(())
    }

    /// The program's table: a count, an address per id and an id per item, zero until written.
    pub(crate) fn ptr_area(&mut self) -> u32 {
        match self.ptr_area {
            Some(a) => a,
            None => {
                let a = self.layout.alloc_hidden(4 + 8 * self.ptr_items.len() as u32);
                self.ptr_area = Some(a);
                a
            }
        }
    }

    /// A whole item, or a pointer element of a table, as an operand.
    fn ptr_operand(&mut self, e: &Expr, line: u32) -> GenResult<u32> {
        match self.operand(e)? {
            Some(o) => Ok(o),
            None => self.err(line, "a data-name was expected"),
        }
    }

    /// The item a name designates, when it is a name: no subscript, no reference modification.
    fn ptr_name(&mut self, e: &Expr) -> Option<usize> {
        match e {
            Expr::Identifier(..) | Expr::Qualified { .. } => self.resolve_item(e).ok(),
            _ => None,
        }
    }

    /// The item of `SET ADDRESS OF item`: an elementary LINKAGE 01 or 77 whose slot is its own.
    fn ptr_based(&mut self, e: &Expr, line: u32) -> GenResult<usize> {
        let Some(i) = self.ptr_name(e) else {
            return self.err_kind(line, ErrorKind::Other, "SET ADDRESS OF", format!("SET ADDRESS OF: {D_PTR_BASED}"));
        };
        let it = &self.layout.items[i];
        let shared = it.link.is_some_and(|l| self.layout.items.iter().enumerate().any(|(j, o)| j != i && o.level != 88 && o.link == Some(l)));
        let ok = it.link.is_some() && it.parent.is_none() && it.children.is_empty() && it.occurs.is_none() && matches!(it.level, 1 | 77) && !shared;
        if ok {
            Ok(i)
        } else {
            self.ptr_refuse(line, i, D_PTR_BASED)
        }
    }

    /// The item of `ADDRESS OF item`: a whole item outside the LINKAGE SECTION and outside any table.
    fn ptr_taken(&mut self, e: &Expr, line: u32) -> GenResult<usize> {
        let Some(i) = self.ptr_name(e) else {
            return self.err_kind(line, ErrorKind::Other, "ADDRESS OF", format!("ADDRESS OF: {D_PTR_ITEM}"));
        };
        let it = &self.layout.items[i];
        let mut in_table = false;
        let mut at = Some(i);
        while let Some(k) = at {
            in_table |= self.layout.items[k].occurs.is_some();
            at = self.layout.items[k].parent;
        }
        let ok = it.link.is_none() && !it.hidden && !it.external && !matches!(it.level, 66 | 88) && it.renames.is_none() && !in_table;
        if ok {
            Ok(i)
        } else {
            self.ptr_refuse(line, i, D_PTR_ITEM)
        }
    }

    /// A pointer operand: a `USAGE POINTER` item outside the LINKAGE SECTION, named whole, or an
    /// element of a table of them (`SQLDA-DATA (1)`, T50a5) with a subscript for each `OCCURS` it lies
    /// in. A table element is a place to hold an address like any other: `rt_ptr_take` stores the id
    /// in the operand it is given, whatever its subscripts.
    fn ptr_pointer(&mut self, e: &Expr, line: u32) -> GenResult<usize> {
        let (named, subscripts) = match e {
            Expr::Subscript { base, indices, .. } if !indices.is_empty() => (base.as_ref(), indices.len()),
            other => (other, 0),
        };
        let Some(i) = self.ptr_name(named) else {
            return self.err_kind(line, ErrorKind::Other, "SET", format!("SET (pointer): {D_PTR_POINTER}"));
        };
        let it = &self.layout.items[i];
        if it.usage == Usage::Pointer && it.link.is_none() && self.layout.occurs_chain(i).len() == subscripts {
            Ok(i)
        } else {
            self.ptr_refuse(line, i, D_PTR_POINTER)
        }
    }

    /// Two elementary items described alike.
    fn ptr_same_shape(&self, a: usize, b: usize) -> bool {
        let (a, b) = (&self.layout.items[a], &self.layout.items[b]);
        a.children.is_empty() && b.children.is_empty() && a.fd == b.fd && a.usage == b.usage && a.pic_template == b.pic_template
    }

    fn ptr_refuse<T>(&self, line: u32, item: usize, why: &str) -> GenResult<T> {
        let name = self.layout.items[item].name.clone().unwrap_or_else(|| "FILLER".into());
        self.err_kind(line, ErrorKind::Other, name.clone(), format!("{name}: {why}"))
    }
}

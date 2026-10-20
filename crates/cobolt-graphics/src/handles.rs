// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Handles: whole numbers a COBOL program holds in an ordinary data item
//! (WASM-018 R2).
//!
//! "Things they create - a series, a node, a light, a body - are **handles**:
//! whole numbers that `RETURNING` brings back into a data item and that the
//! object's methods take; a handle is released by `Release`, by the removal of
//! the thing it names, and when its object or form is destroyed."
//!
//! A handle is a non-zero number below 2^31, so it fits `PIC S9(9) COMP`. Its
//! low 22 bits are a slot index (plus one, so zero never names anything) and the
//! next 9 bits a *generation* that changes every time the slot is reused. A
//! handle that was released therefore answers "released" instead of silently
//! naming the next thing that took its slot - which is the failure a program
//! with a forgotten handle would otherwise meet.
//!
//! [`HandleTable`] is the building block every object uses for what it hands
//! out. Dropping the table (that is, dropping its object) drops everything in
//! it, so "owner-drop" needs no bookkeeping: the owner's `Drop` *is* the
//! release. The registry in [`crate::registry`] adds the top level: objects
//! owned by a form, all released together when the form is destroyed.

use crate::error::{GraphicsError, Result};

const INDEX_BITS: u32 = 22;
const GEN_BITS: u32 = 9;
const INDEX_MASK: u32 = (1 << INDEX_BITS) - 1;
const GEN_MASK: u32 = (1 << GEN_BITS) - 1;

/// The most slots one table can hold (one less than 2^22: slot 0 is not used).
pub const MAX_SLOTS: usize = INDEX_MASK as usize - 1;

/// A handle as a program sees it: a whole number, never 0, always below 2^31.
pub type Handle = u32;

fn pack(index: usize, generation: u32) -> Handle {
    (generation << INDEX_BITS) | (index as u32 + 1)
}

fn unpack(handle: Handle) -> Option<(usize, u32)> {
    if handle == 0 || handle >= (1 << (INDEX_BITS + GEN_BITS)) {
        return None;
    }
    let idx = handle & INDEX_MASK;
    if idx == 0 {
        return None;
    }
    Some(((idx - 1) as usize, handle >> INDEX_BITS))
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// A table of values addressed by [`Handle`].
pub struct HandleTable<T> {
    kind: &'static str,
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    live: usize,
}

impl<T> HandleTable<T> {
    /// A new empty table. `kind` names what it holds ("node", "light", "body"),
    /// for the error messages.
    pub fn new(kind: &'static str) -> Self {
        Self { kind, slots: Vec::new(), free: Vec::new(), live: 0 }
    }

    /// What the table holds.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    /// How many values are live.
    pub fn len(&self) -> usize {
        self.live
    }

    /// Whether no value is live.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// Store `value`, returning its handle.
    pub fn insert(&mut self, value: T) -> Result<Handle> {
        if let Some(idx) = self.free.pop() {
            let slot = &mut self.slots[idx as usize];
            slot.value = Some(value);
            self.live += 1;
            return Ok(pack(idx as usize, slot.generation));
        }
        if self.slots.len() >= MAX_SLOTS {
            return Err(GraphicsError::LimitExceeded {
                what: "live handles in one object",
                requested: self.slots.len() as u64 + 1,
                limit: MAX_SLOTS as u64,
                setting: "(fixed by the handle format)",
            });
        }
        self.slots.push(Slot { generation: 0, value: Some(value) });
        self.live += 1;
        Ok(pack(self.slots.len() - 1, 0))
    }

    fn locate(&self, handle: Handle) -> Result<usize> {
        let (idx, generation) = unpack(handle).ok_or(GraphicsError::UnknownHandle(handle))?;
        let slot = self.slots.get(idx).ok_or(GraphicsError::UnknownHandle(handle))?;
        if slot.generation != generation || slot.value.is_none() {
            return Err(GraphicsError::ReleasedHandle(handle));
        }
        Ok(idx)
    }

    /// Borrow the value a handle names.
    pub fn get(&self, handle: Handle) -> Result<&T> {
        let idx = self.locate(handle)?;
        Ok(self.slots[idx].value.as_ref().expect("located"))
    }

    /// Borrow the value a handle names, mutably.
    pub fn get_mut(&mut self, handle: Handle) -> Result<&mut T> {
        let idx = self.locate(handle)?;
        Ok(self.slots[idx].value.as_mut().expect("located"))
    }

    /// Whether `handle` names a live value.
    pub fn contains(&self, handle: Handle) -> bool {
        self.locate(handle).is_ok()
    }

    /// Release a handle, returning the value (which the caller usually drops).
    pub fn remove(&mut self, handle: Handle) -> Result<T> {
        let idx = self.locate(handle)?;
        let slot = &mut self.slots[idx];
        let value = slot.value.take().expect("located");
        slot.generation = (slot.generation + 1) & GEN_MASK;
        self.free.push(idx as u32);
        self.live -= 1;
        Ok(value)
    }

    /// Release everything. Every handle issued so far becomes "released".
    pub fn clear(&mut self) {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.value.take().is_some() {
                slot.generation = (slot.generation + 1) & GEN_MASK;
                self.free.push(i as u32);
            }
        }
        self.live = 0;
    }

    /// The live values with their handles, in slot order (deterministic).
    pub fn iter(&self) -> impl Iterator<Item = (Handle, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.value.as_ref().map(|v| (pack(i, s.generation), v)))
    }

    /// The live values with their handles, mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle, &mut T)> {
        self.slots
            .iter_mut()
            .enumerate()
            .filter_map(|(i, s)| {
                let g = s.generation;
                s.value.as_mut().map(|v| (pack(i, g), v))
            })
    }

    /// Typed error helper: this table's `kind` as "expected" for a mismatch.
    pub fn wrong_kind(&self, handle: Handle, found: &'static str) -> GraphicsError {
        GraphicsError::WrongHandleKind { handle, found, expected: self.kind }
    }
}

impl<T> std::fmt::Debug for HandleTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HandleTable").field("kind", &self.kind).field("live", &self.live).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_nonzero_whole_numbers_below_2_pow_31() {
        let mut t = HandleTable::new("thing");
        for i in 0..1000u32 {
            let h = t.insert(i).unwrap();
            assert!(h >= 1 && h < (1 << 31));
            assert_eq!(*t.get(h).unwrap(), i);
        }
        assert_eq!(t.len(), 1000);
    }

    #[test]
    fn a_released_handle_is_refused_not_reused() {
        let mut t = HandleTable::new("thing");
        let a = t.insert("a").unwrap();
        assert_eq!(t.remove(a).unwrap(), "a");
        assert_eq!(t.get(a), Err(GraphicsError::ReleasedHandle(a)));
        let b = t.insert("b").unwrap();
        assert_ne!(a, b, "the slot is reused but under a new generation");
        assert_eq!(t.get(a), Err(GraphicsError::ReleasedHandle(a)));
        assert_eq!(*t.get(b).unwrap(), "b");
        assert_eq!(t.remove(a), Err(GraphicsError::ReleasedHandle(a)));
    }

    #[test]
    fn zero_and_foreign_numbers_are_unknown() {
        let t: HandleTable<u8> = HandleTable::new("thing");
        assert_eq!(t.get(0), Err(GraphicsError::UnknownHandle(0)));
        assert_eq!(t.get(77), Err(GraphicsError::UnknownHandle(77)));
        assert_eq!(t.get(u32::MAX), Err(GraphicsError::UnknownHandle(u32::MAX)));
    }

    #[test]
    fn clear_releases_everything() {
        let mut t = HandleTable::new("thing");
        let hs: Vec<_> = (0..10).map(|i| t.insert(i).unwrap()).collect();
        t.clear();
        assert!(t.is_empty());
        for h in hs {
            assert_eq!(t.get(h), Err(GraphicsError::ReleasedHandle(h)));
        }
    }

    #[test]
    fn iteration_order_is_deterministic() {
        let mut t = HandleTable::new("thing");
        let h: Vec<_> = (0..5).map(|i| t.insert(i).unwrap()).collect();
        t.remove(h[2]).unwrap();
        let order: Vec<i32> = t.iter().map(|(_, v)| *v).collect();
        assert_eq!(order, vec![0, 1, 3, 4]);
    }

    #[test]
    fn dropping_the_table_drops_its_values() {
        use std::rc::Rc;
        let probe = Rc::new(());
        {
            let mut t = HandleTable::new("thing");
            for _ in 0..50 {
                t.insert(probe.clone()).unwrap();
            }
            assert_eq!(Rc::strong_count(&probe), 51);
        }
        assert_eq!(Rc::strong_count(&probe), 1, "owner-drop releases every child");
    }
}

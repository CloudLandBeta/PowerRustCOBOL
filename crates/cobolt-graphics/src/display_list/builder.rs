// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Recording commands into a list.
//!
//! The builder's drawing methods (`fill_rect`, `move_to`, ... one per command)
//! are generated from the command table in [`ops`](super::ops); this file holds
//! only the bookkeeping: the header, the counters and the limits.

use super::list::{HEADER_LEN, MAGIC, VERSION};
use super::ops::Command;
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;

/// Records commands and produces the bytes of a display list.
///
/// Calls chain (`b.save().fill_rect(...).restore();`) and never fail one by
/// one: the first limit hit is remembered and reported by
/// [`DisplayListBuilder::take`], which is where a host looks anyway.
#[derive(Debug, Clone)]
pub struct DisplayListBuilder {
    payload: Vec<u8>,
    commands: u32,
    limits: Limits,
    error: Option<GraphicsError>,
}

impl Default for DisplayListBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DisplayListBuilder {
    /// A builder with the default [`Limits`].
    pub fn new() -> Self {
        Self::with_limits(Limits::default())
    }

    /// A builder with explicit limits.
    pub fn with_limits(limits: Limits) -> Self {
        Self { payload: Vec::new(), commands: 0, limits, error: None }
    }

    pub(crate) fn push(&mut self, cmd: &Command<'_>) {
        if self.error.is_some() {
            return;
        }
        if let Err(e) = self.limits.check_display_list_commands(self.commands as u64 + 1) {
            self.error = Some(e);
            return;
        }
        let before = self.payload.len();
        cmd.encode(&mut self.payload);
        if let Err(e) = self.limits.check_display_list_bytes((HEADER_LEN + self.payload.len()) as u64) {
            self.payload.truncate(before);
            self.error = Some(e);
            return;
        }
        self.commands += 1;
    }

    /// Record one command already in wire form (what [`DisplayListBuilder::push_dynamic`] builds).
    pub(crate) fn push_encoded(&mut self, encoded: &[u8]) {
        if self.error.is_some() {
            return;
        }
        if let Err(e) = self.limits.check_display_list_commands(self.commands as u64 + 1) {
            self.error = Some(e);
            return;
        }
        if let Err(e) = self.limits.check_display_list_bytes((HEADER_LEN + self.payload.len() + encoded.len()) as u64) {
            self.error = Some(e);
            return;
        }
        self.payload.extend_from_slice(encoded);
        self.commands += 1;
    }

    /// Commands recorded so far.
    pub fn command_count(&self) -> u32 {
        self.commands
    }

    /// Bytes recorded so far (without the header).
    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    /// Whether nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.commands == 0
    }

    /// The error that stopped recording, if any.
    pub fn error(&self) -> Option<&GraphicsError> {
        self.error.as_ref()
    }

    /// Throw away what is recorded (and any error).
    pub fn discard(&mut self) {
        self.payload.clear();
        self.commands = 0;
        self.error = None;
    }

    /// The finished list (header and payload). The builder is left empty
    /// whether it succeeds or not.
    pub fn take(&mut self) -> Result<Vec<u8>> {
        if let Some(e) = self.error.take() {
            self.discard();
            return Err(e);
        }
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.commands.to_le_bytes());
        out.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.payload);
        self.discard();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::DisplayList;

    #[test]
    fn take_empties_the_builder() {
        let mut b = DisplayListBuilder::new();
        b.begin_path().move_to(0.0, 0.0).line_to(5.0, 5.0).stroke();
        assert_eq!(b.command_count(), 4);
        let bytes = b.take().unwrap();
        assert!(b.is_empty());
        assert_eq!(DisplayList::parse(&bytes, &Limits::default()).unwrap().command_count(), 4);
        // An empty list is valid.
        let empty = b.take().unwrap();
        assert_eq!(empty.len(), HEADER_LEN);
        DisplayList::parse(&empty, &Limits::default()).unwrap().validate().unwrap();
    }

    #[test]
    fn the_command_limit_is_sticky_and_reported_by_take() {
        let lim = Limits { max_display_list_commands: 3, ..Limits::default() };
        let mut b = DisplayListBuilder::with_limits(lim);
        b.save().save().save().save().save();
        assert_eq!(b.command_count(), 3);
        let e = b.take().unwrap_err();
        assert!(e.to_string().contains("max_display_list_commands"), "{e}");
        assert!(b.is_empty() && b.error().is_none(), "take() resets the builder");
        b.save();
        assert!(b.take().is_ok());
    }

    #[test]
    fn the_byte_limit_is_enforced_before_the_list_grows() {
        let lim = Limits { max_display_list_bytes: 64, ..Limits::default() };
        let mut b = DisplayListBuilder::with_limits(lim);
        for _ in 0..100 {
            b.fill_rect(0.0, 0.0, 1.0, 1.0);
        }
        assert!(b.payload_len() + HEADER_LEN <= 64);
        assert!(b.take().is_err());
    }

    #[test]
    fn strings_and_arrays_record() {
        let mut b = DisplayListBuilder::new();
        b.draw_polygon(&[(0.0, 0.0), (10.0, 0.0), (5.0, 8.0)], 7)
            .set_dash(&[3.0, 1.0], 0.5)
            .draw_text("Hello", 1.0, 2.0, 0.0, 1);
        let bytes = b.take().unwrap();
        let l = DisplayList::parse(&bytes, &Limits::default()).unwrap();
        l.validate().unwrap();
        assert_eq!(l.command_count(), 3);
    }
}

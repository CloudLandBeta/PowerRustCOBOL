// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Parsing and iterating a display list.

use super::ops::{Command, Reader};
use crate::error::{GraphicsError, Result};
use crate::limits::Limits;

/// The four magic bytes of every list.
pub const MAGIC: [u8; 4] = *b"PRGD";
/// The encoding version this build reads and writes.
pub const VERSION: u16 = 1;
/// Bytes before the payload.
pub const HEADER_LEN: usize = 16;

/// A validated header plus the payload it describes. Cheap to copy; borrows the
/// bytes.
#[derive(Debug, Clone, Copy)]
pub struct DisplayList<'a> {
    commands: u32,
    payload: &'a [u8],
}

impl<'a> DisplayList<'a> {
    /// Check the header against the bytes and the limits. Does **not** walk
    /// the commands (use [`DisplayList::validate`] or just iterate).
    pub fn parse(bytes: &'a [u8], limits: &Limits) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(GraphicsError::bad_list(format!(
                "{} byte(s) is shorter than the {HEADER_LEN}-byte header",
                bytes.len()
            )));
        }
        limits.check_display_list_bytes(bytes.len() as u64)?;
        if bytes[0..4] != MAGIC {
            return Err(GraphicsError::bad_list("the magic bytes are not PRGD"));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(GraphicsError::bad_list(format!(
                "version {version} is not supported (this build reads version {VERSION})"
            )));
        }
        let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
        if flags != 0 {
            return Err(GraphicsError::bad_list(format!("flags 0x{flags:04X} are reserved and must be 0")));
        }
        let commands = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let payload_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        if payload_len != bytes.len() - HEADER_LEN {
            return Err(GraphicsError::bad_list(format!(
                "the header says {payload_len} payload byte(s) but {} follow it",
                bytes.len() - HEADER_LEN
            )));
        }
        limits.check_display_list_commands(commands as u64)?;
        // A command is at least one byte, so a count above the payload is a lie
        // that would otherwise make the iterator promise commands that cannot exist.
        if commands as usize > payload_len {
            return Err(GraphicsError::bad_list(format!(
                "{commands} command(s) cannot fit in {payload_len} payload byte(s)"
            )));
        }
        Ok(Self { commands, payload: &bytes[HEADER_LEN..] })
    }

    /// Number of commands the header promises.
    pub fn command_count(&self) -> u32 {
        self.commands
    }

    /// The payload bytes.
    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// The commands, decoded one at a time.
    pub fn commands(&self) -> Commands<'a> {
        Commands { reader: Reader::new(self.payload), left: self.commands, done: false }
    }

    /// Decode every command, returning the first error. A list that validates
    /// cannot fail to decode when it is executed.
    pub fn validate(&self) -> Result<()> {
        for c in self.commands() {
            c?;
        }
        Ok(())
    }
}

/// Iterator over the commands of a list. After the first error it ends.
pub struct Commands<'a> {
    reader: Reader<'a>,
    left: u32,
    done: bool,
}

impl<'a> Iterator for Commands<'a> {
    type Item = Result<Command<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.left == 0 {
            self.done = true;
            if self.reader.remaining() != 0 {
                return Some(Err(GraphicsError::bad_list(format!(
                    "{} byte(s) left after the last command",
                    self.reader.remaining()
                ))));
            }
            return None;
        }
        self.left -= 1;
        match Command::decode(&mut self.reader) {
            Ok(c) => Some(Ok(c)),
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::DisplayListBuilder;

    fn tiny() -> Vec<u8> {
        let mut b = DisplayListBuilder::new();
        b.save().fill_rect(1.0, 2.0, 3.0, 4.0).restore();
        b.take().unwrap()
    }

    #[test]
    fn a_good_list_parses_and_iterates() {
        let bytes = tiny();
        let l = DisplayList::parse(&bytes, &Limits::default()).unwrap();
        assert_eq!(l.command_count(), 3);
        let names: Vec<_> = l.commands().map(|c| c.unwrap().name()).collect();
        assert_eq!(names, ["Save", "FillRect", "Restore"]);
        l.validate().unwrap();
    }

    #[test]
    fn header_damage_is_refused() {
        let good = tiny();
        let lim = Limits::default();
        let parse = |b: &[u8]| DisplayList::parse(b, &lim).map(|_| ());
        assert!(parse(&good[..10]).is_err(), "short");
        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(parse(&bad).is_err(), "magic");
        let mut bad = good.clone();
        bad[4] = 2;
        assert!(parse(&bad).is_err(), "version");
        let mut bad = good.clone();
        bad[6] = 1;
        assert!(parse(&bad).is_err(), "flags");
        let mut bad = good.clone();
        bad[12] ^= 1;
        assert!(parse(&bad).is_err(), "payload length");
        let mut bad = good.clone();
        bad.push(0);
        assert!(parse(&bad).is_err(), "trailing byte changes the exact payload length");
    }

    #[test]
    fn a_lying_count_is_refused_or_caught() {
        let mut good = tiny();
        good[8] = 19; // 19 commands promised (as many as there are bytes), 3 present
        let l = DisplayList::parse(&good, &Limits::default()).unwrap();
        assert!(l.validate().is_err());
        let mut good = tiny();
        good[8] = 2; // fewer promised: bytes are left over
        let l = DisplayList::parse(&good, &Limits::default()).unwrap();
        assert!(l.validate().is_err());
    }

    #[test]
    fn limits_refuse_a_long_list() {
        let bytes = tiny();
        let lim = Limits { max_display_list_bytes: 20, ..Limits::default() };
        let e = DisplayList::parse(&bytes, &lim).unwrap_err();
        assert!(e.to_string().contains("max_display_list_bytes"), "{e}");
        let lim = Limits { max_display_list_commands: 2, ..Limits::default() };
        let e = DisplayList::parse(&bytes, &lim).unwrap_err();
        assert!(e.to_string().contains("max_display_list_commands"), "{e}");
    }

    #[test]
    fn unknown_opcode_is_an_error_not_a_panic() {
        let mut b = tiny();
        b[HEADER_LEN] = 0xEE;
        let l = DisplayList::parse(&b, &Limits::default()).unwrap();
        assert!(l.validate().is_err());
    }
}

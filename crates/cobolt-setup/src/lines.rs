// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

//! How a child's output becomes lines.
//!
//! The supervisor reads the child's standard output from a pipe in whatever
//! pieces the pipe delivers: half a line, three and a half lines, a line that
//! ends in `\r\n`. The progress text is built from lines, so the cutting is done
//! once, here, where it is tested on every operating system.

/// The longest line kept. A child that prints a megabyte without a newline must not become a megabyte of progress text.
pub const MAX_LINE: usize = 400;

/// Collects bytes and hands back the complete lines in them.
#[derive(Debug, Default)]
pub struct LineBuffer {
    pending: Vec<u8>,
}

impl LineBuffer {
    /// Add what the pipe delivered; get every line it completed, in order, without their line ends and without empty lines.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(bytes);
        let mut lines = Vec::new();
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let raw: Vec<u8> = self.pending.drain(..=end).collect();
            if let Some(line) = clean(&raw) {
                lines.push(line);
            }
        }
        // A child that never ends its line must not grow the buffer without bound.
        if self.pending.len() > MAX_LINE * 4 {
            let keep = self.pending.len() - MAX_LINE * 4;
            self.pending.drain(..keep);
        }
        lines
    }

    /// The last line, when the child ended without a line end.
    pub fn finish(&mut self) -> Option<String> {
        let raw = std::mem::take(&mut self.pending);
        clean(&raw)
    }
}

fn clean(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let text = text.trim_matches(|c: char| c == '\r' || c == '\n' || c.is_whitespace());
    if text.is_empty() {
        return None;
    }
    let mut line: String = text.chars().take(MAX_LINE).collect();
    // A progress line is read by a person: control characters other than a tab are noise.
    line.retain(|c| !c.is_control() || c == '\t');
    (!line.is_empty()).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_split_across_reads_is_whole_when_it_arrives() {
        let mut b = LineBuffer::default();
        assert!(b.push(b"Compiling ser").is_empty());
        assert_eq!(b.push(b"de v1.0\n"), ["Compiling serde v1.0"]);
    }

    #[test]
    fn many_lines_in_one_read_and_both_line_ends() {
        let mut b = LineBuffer::default();
        assert_eq!(
            b.push(b"one\r\ntwo\n\n  \nthree\r\nfo"),
            ["one", "two", "three"]
        );
        assert_eq!(
            b.finish().as_deref(),
            Some("fo"),
            "the unfinished line comes out at the end"
        );
        assert_eq!(b.finish(), None);
    }

    #[test]
    fn bad_bytes_and_long_lines_are_tamed() {
        let mut b = LineBuffer::default();
        let mut input = vec![b'a'; MAX_LINE * 3];
        input.push(b'\n');
        let lines = b.push(&input);
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].chars().count(),
            MAX_LINE,
            "cut to the longest line kept"
        );

        assert_eq!(
            b.push(b"ok \xff\xfe bytes\x07\n"),
            ["ok \u{fffd}\u{fffd} bytes"],
            "invalid UTF-8 is replaced, a bell is dropped"
        );

        // A line that never ends does not grow the buffer without bound.
        let mut b = LineBuffer::default();
        for _ in 0..100 {
            b.push(&[b'x'; 1000]);
        }
        assert!(
            b.pending.len() <= MAX_LINE * 4,
            "pending is {} bytes",
            b.pending.len()
        );
    }
}

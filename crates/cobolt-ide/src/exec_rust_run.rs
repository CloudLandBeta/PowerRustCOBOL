// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Does this program need a build before it can run? (spec 041 T13)
//!
//! `EXEC RUST` is compiled, not interpreted. `rcrun run-form` walks the AST, so
//! it cannot execute a block: the compiled function for a block id lives in a
//! *built* binary, and running without one is a hard error by design (R11).
//!
//! So *Run* has two paths, chosen by one question — does the program contain a
//! block? A program without one keeps today's fast interpreter path untouched;
//! a program with one is built first and the built binary is what runs. That
//! keeps a single meaning for `EXEC RUST` (the spec forbids dual semantics) and
//! confines the compile-time cost to the programs that ask for it.
//!
//! # Why the lexer, not a substring search
//!
//! `source.contains("EXEC RUST")` would answer yes for a program that merely
//! mentions the words in a comment or a string literal, and every such program
//! would then pay a cargo build on every *Run* for nothing. The lexer already
//! captures a block as a single token, so asking it is both exact and cheap.

use std::path::Path;

use cobolt_lexer::{tokenize, SourceFormat, Token};

/// Whether `source` contains at least one `EXEC RUST … END-EXEC` block.
pub fn source_has_blocks(source: &str) -> bool {
    let fmt = if looks_fixed(source) {
        SourceFormat::Fixed
    } else {
        SourceFormat::Free
    };
    tokenize(source, fmt)
        .iter()
        .any(|t| matches!(t.token, Token::ExecRustBlock(_)))
}

/// Whether the COBOL file at `path` contains a block.
///
/// An unreadable file answers `false`: the run path that follows reports the
/// real problem, and guessing "yes" here would trade a clear error for a
/// pointless build.
pub fn file_has_blocks(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|s| source_has_blocks(&s))
        .unwrap_or(false)
}

/// Whether **any** of these programs contains a block.
///
/// A form application is not one program. `OpenForm` runs another form's
/// generated program in an interpreter of its own (spec 051 R1), and every one
/// of them looks a block up in the process's single compiled registry — so a
/// binary is needed as soon as *one* form has a block, wherever it lives.
///
/// Asking only about the form the developer pressed Run on answered "no" for a
/// block in a child form's handler. The IDE then took the interpreter path,
/// which cannot execute a block, and the failure surfaced at the button click
/// rather than at Run. Pass every form the project holds.
pub fn any_has_blocks<'a, I>(paths: I) -> bool
where
    I: IntoIterator<Item = &'a Path>,
{
    paths.into_iter().any(file_has_blocks)
}

/// Same heuristic the compiler uses to tell fixed from free format.
fn looks_fixed(source: &str) -> bool {
    source.lines().any(|line| {
        let b = line.as_bytes();
        b.len() > 6 && b[6] != b' ' && b[..6].iter().all(|&c| c == b' ' || c.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const WITH_BLOCK: &str = r#"
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HASBLOCK.
       PROCEDURE DIVISION.
           EXEC RUST
           println!("hi");
           END-EXEC.
           STOP RUN.
"#;

    #[test]
    fn a_program_with_a_block_needs_a_build() {
        assert!(source_has_blocks(WITH_BLOCK));
    }

    #[test]
    fn a_program_without_one_keeps_the_interpreter_path() {
        assert!(!source_has_blocks(
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. PLAIN.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"hello\".\n\
             \x20          STOP RUN.\n"
        ));
    }

    /// The reason this asks the lexer instead of searching for the words: a
    /// program that only *mentions* `EXEC RUST` must not pay a cargo build on
    /// every Run.
    #[test]
    fn merely_mentioning_the_words_does_not_count() {
        let src = "       IDENTIFICATION DIVISION.\n\
                   \x20      PROGRAM-ID. MENTIONS.\n\
                   \x20      DATA DIVISION.\n\
                   \x20      WORKING-STORAGE SECTION.\n\
                   \x20      01 WS-NOTE PIC X(30) VALUE \"EXEC RUST is compiled\".\n\
                   \x20      PROCEDURE DIVISION.\n\
                   \x20      *> EXEC RUST would go here one day\n\
                   \x20          DISPLAY WS-NOTE.\n\
                   \x20          STOP RUN.\n";
        assert!(
            !source_has_blocks(src),
            "a comment and a literal are not a block"
        );
    }

    #[test]
    fn an_unreadable_file_does_not_force_a_build() {
        assert!(!file_has_blocks(Path::new(
            "/nonexistent/definitely-not-here.cbl"
        )));
    }

    /// The bug this function exists for: the block is in the SECOND form, and a
    /// multi-form application still has to be built.
    #[test]
    fn a_block_in_any_form_needs_a_build() {
        let dir = std::env::temp_dir().join("prc-exec-rust-run-any");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let plain = dir.join("MAINFORM.cbl");
        let with_block = dir.join("CHILDFORM.cbl");
        std::fs::write(
            &plain,
            "       IDENTIFICATION DIVISION.\n\
             \x20      PROGRAM-ID. MAINFORM.\n\
             \x20      PROCEDURE DIVISION.\n\
             \x20          DISPLAY \"hello\".\n\
             \x20          STOP RUN.\n",
        )
        .expect("write");
        std::fs::write(&with_block, WITH_BLOCK).expect("write");

        assert!(
            !file_has_blocks(&plain),
            "the form the developer pressed Run on has no block"
        );
        assert!(
            any_has_blocks([plain.as_path(), with_block.as_path()]),
            "…but a child form does, so the run needs a build"
        );
        assert!(
            !any_has_blocks([plain.as_path()]),
            "a project of plain forms keeps the interpreter path"
        );
        assert!(!any_has_blocks(std::iter::empty()), "nothing to run, nothing to build");

        let _ = std::fs::remove_dir_all(&dir);
    }
}

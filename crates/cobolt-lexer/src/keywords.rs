// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Keyword lookup table for the Cobolt COBOL lexer.
//!
//! This module maps uppercase COBOL word strings to their [`Token`] variants.
//!
//! # Compound keywords
//!
//! COBOL has many hyphenated keywords that look syntactically identical to
//! user-defined hyphenated names (e.g. `WORKING-STORAGE` vs `MY-COUNTER`).
//! The entire hyphenated word is kept as a single logos `Word` token and
//! classified here, so `WORKING-STORAGE` produces a single `Token::WorkingStorage`
//! rather than `Token::Identifier("WORKING") + Token::Minus + Token::Identifier("STORAGE")`.
//!
//! # Adding a new keyword
//!
//! 1. Add a variant to [`Token`].
//! 2. Insert an entry in [`KEYWORDS`] below.
//! 3. If the keyword has an alias (e.g. COMPUTATIONAL = COMP), add both entries.

use crate::Token;

/// Look up a COBOL word (already upper-cased by the caller) and return the
/// corresponding keyword token, or `None` if it is a user-defined identifier.
///
/// The lookup is a plain `match` on `&str`, which the compiler turns into an
/// optimal jump table or hash — no `HashMap` allocation required.
pub fn lookup(word: &str) -> Option<Token> {
    // NOTE: All entries must be in UPPER CASE.
    let tok = match word {
        // ── Divisions ─────────────────────────────────────────────────────
        "IDENTIFICATION" | "ID"     => Token::Identification,
        "ENVIRONMENT"               => Token::Environment,
        "DATA"                      => Token::Data,
        "PROCEDURE"                 => Token::Procedure,
        "DIVISION"                  => Token::Division,

        // ── Sections ──────────────────────────────────────────────────────
        "SECTION"                   => Token::Section,
        "CONFIGURATION"             => Token::Configuration,
        "SOURCE-COMPUTER"           => Token::SourceComputer,
        "OBJECT-COMPUTER"           => Token::ObjectComputer,
        "INPUT-OUTPUT"              => Token::InputOutput,
        "FILE-CONTROL"              => Token::FileControl,
        "WORKING-STORAGE"           => Token::WorkingStorage,
        "LOCAL-STORAGE"             => Token::LocalStorage,
        "LINKAGE"                   => Token::Linkage,
        "SCREEN"                    => Token::Screen,

        // ── Program structure ─────────────────────────────────────────────
        "PROGRAM-ID"                => Token::ProgramId,
        "AUTHOR"                    => Token::Author,
        "DATE-WRITTEN"              => Token::DateWritten,
        "DATE-COMPILED"             => Token::DateCompiled,
        "PROGRAM"                   => Token::Program,
        "END"                       => Token::End,

        // ── Data definition ───────────────────────────────────────────────
        "PIC" | "PICTURE"           => Token::Pic,
        "VALUE"                     => Token::Value,
        "VALUES"                    => Token::Values,
        "OCCURS"                    => Token::Occurs,
        "TIMES"                     => Token::Times,
        "DEPENDING"                 => Token::Depending,
        "ON"                        => Token::On,
        "REDEFINES"                 => Token::Redefines,
        "RENAMES"                   => Token::Renames,
        "FILLER"                    => Token::Filler,
        "GLOBAL"                    => Token::Global,
        "EXTERNAL"                  => Token::External,
        "BLANK"                     => Token::Blank,
        "WHEN"                      => Token::When,
        "JUSTIFIED" | "JUST"        => Token::Justified,
        "RIGHT"                     => Token::Right,
        "LEFT"                      => Token::Left,
        "SYNCHRONIZED" | "SYNC"     => Token::Synchronized,
        "SIGN"                      => Token::Sign,
        "LEADING"                   => Token::Leading,
        "TRAILING"                  => Token::Trailing,
        "SEPARATE"                  => Token::Separate,
        "CHARACTER"                 => Token::Character,

        // ── USAGE clause ──────────────────────────────────────────────────
        "USAGE"                     => Token::Usage,
        "DISPLAY"                   => Token::Display,
        "BINARY"                    => Token::Binary,
        "COMP" | "COMPUTATIONAL"    => Token::Comp,
        "COMP-1" | "COMPUTATIONAL-1" => Token::Comp1,
        "COMP-2" | "COMPUTATIONAL-2" => Token::Comp2,
        "COMP-3" | "COMPUTATIONAL-3" => Token::Comp3,
        "COMP-4" | "COMPUTATIONAL-4" => Token::Comp, // binary, like COMP
        "COMP-5" | "COMPUTATIONAL-5" => Token::Comp5,
        "COMP-X" | "COMPUTATIONAL-X" => Token::Comp5, // unsigned binary

        "PACKED-DECIMAL"            => Token::PackedDecimal,
        "POINTER"                   => Token::Pointer,
        "INDEX"                     => Token::Index,
        "NATIONAL"                  => Token::NationalUsage,

        // ── Arithmetic verbs ──────────────────────────────────────────────
        "ADD"                       => Token::Add,
        "SUBTRACT"                  => Token::Subtract,
        "MULTIPLY"                  => Token::Multiply,
        "DIVIDE"                    => Token::Divide,
        "COMPUTE"                   => Token::Compute,
        "TO"                        => Token::To,
        "FROM"                      => Token::From,
        "BY"                        => Token::By,
        "GIVING"                    => Token::Giving,
        "REMAINDER"                 => Token::Remainder,
        "ROUNDED"                   => Token::Rounded,
        "END-ADD"                   => Token::EndAdd,
        "END-SUBTRACT"              => Token::EndSubtract,
        "END-MULTIPLY"              => Token::EndMultiply,
        "END-DIVIDE"                => Token::EndDivide,
        "END-COMPUTE"               => Token::EndCompute,
        "SIZE"                      => Token::SizeError,   // parser combines SIZE ERROR

        // ── Move / Set / Initialize ───────────────────────────────────────
        "MOVE"                      => Token::Move,
        "SET"                       => Token::Set,
        "INITIALIZE"                => Token::Initialize,
        "CORRESPONDING" | "CORR"   => Token::Corresponding,

        // ── Control flow ──────────────────────────────────────────────────
        "IF"                        => Token::If,
        "ELSE"                      => Token::Else,
        "END-IF"                    => Token::EndIf,
        "END-PERFORM"               => Token::EndPerform,
        "EVALUATE"                  => Token::Evaluate,
        "ALSO"                      => Token::Also,
        "OTHER"                     => Token::Other,
        "END-EVALUATE"              => Token::EndEvaluate,
        "PERFORM"                   => Token::Perform,
        "UNTIL"                     => Token::Until,
        "VARYING"                   => Token::Varying,
        "AFTER"                     => Token::After,
        "BEFORE"                    => Token::Before,
        "TEST"                      => Token::Test,
        "THROUGH" | "THRU"          => Token::Through,
        "WITH"                      => Token::With,
        "NO"                        => Token::No,
        "GO"                        => Token::Go,
        "GO-TO"                     => Token::GoTo,
        "GO-BACK"                   => Token::GoBack,
        "STOP"                      => Token::Stop,
        "RUN"                       => Token::Run,
        "EXIT"                      => Token::Exit,
        "CONTINUE"                  => Token::Continue,
        "NOT"                       => Token::Not,
        "AND"                       => Token::And,
        "OR"                        => Token::Or,
        "IS"                        => Token::Is,
        "ARE"                       => Token::Are,
        "IN"                        => Token::In,
        "OF"                        => Token::Of,
        "THAN"                      => Token::Than,
        "EQUAL"                     => Token::Equal,
        "GREATER"                   => Token::Greater,
        "LESS"                      => Token::Less,
        "TRUE"                      => Token::True_,
        "FALSE"                     => Token::False_,

        // ── String manipulation ───────────────────────────────────────────
        "STRING"                    => Token::StringVerb,
        "UNSTRING"                  => Token::Unstring,
        "INSPECT"                   => Token::Inspect,
        "TALLYING"                  => Token::Tallying,
        "REPLACING"                 => Token::Replacing,
        "CONVERTING"                => Token::Converting,
        "DELIMITED"                 => Token::Delimited,
        "INTO"                      => Token::Into,
        "COUNT"                     => Token::Count,
        "ALL"                       => Token::All,

        // ── I/O verbs ─────────────────────────────────────────────────────
        "OPEN"                      => Token::Open,
        "CLOSE"                     => Token::Close,
        "READ"                      => Token::Read,
        "WRITE"                     => Token::Write,
        "REWRITE"                   => Token::Rewrite,
        "DELETE"                    => Token::Delete,
        "START"                     => Token::Start,
        "ACCEPT"                    => Token::Accept,
        "INPUT"                     => Token::Input,
        "OUTPUT"                    => Token::Output,
        "I-O"                       => Token::IoMode,
        "EXTEND"                    => Token::Extend,
        "AT-END"                    => Token::AtEnd,
        "END-READ"                  => Token::EndRead,
        "END-WRITE"                 => Token::EndWrite,
        "END-REWRITE"               => Token::EndRewrite,
        "END-DELETE"                => Token::EndDelete,
        "END-START"                 => Token::EndStart,
        "END-STRING"                => Token::EndString,
        "END-UNSTRING"              => Token::EndUnstring,
        "END-SEARCH"                => Token::EndSearch,
        "INVALID"                   => Token::InvalidKey,  // parser combines INVALID KEY
        "UPON"                      => Token::Upon,
        "ADVANCING"                 => Token::Advancing,
        "LINE"                      => Token::Line,
        "LINES"                     => Token::Lines,
        "KEY"                       => Token::Key,
        "RECORD"                    => Token::Record,
        "FILE"                      => Token::File,
        "SELECT"                    => Token::Select,
        "ASSIGN"                    => Token::Assign,
        "ORGANIZATION"              => Token::Organization,
        "SEQUENTIAL"                => Token::Sequential,
        "INDEXED"                   => Token::Indexed,
        "RELATIVE"                  => Token::Relative,
        "ACCESS"                    => Token::Access,
        "MODE"                      => Token::Mode,
        "RANDOM"                    => Token::Random,
        "DYNAMIC"                   => Token::Dynamic,
        "RECORD-KEY"                => Token::RecordKey,
        "ALTERNATE-RECORD-KEY"      => Token::AlternateRecord,
        "STATUS"                    => Token::Status,
        "FD"                        => Token::Fd,
        "SD"                        => Token::Sd,
        "BLOCK"                     => Token::Block,
        "CONTAINS"                  => Token::Contains,
        "CHARACTERS"                => Token::Characters,
        "RECORDS"                   => Token::Records,
        "LABEL"                     => Token::Label,
        "STANDARD"                  => Token::Standard,

        // ── CALL / INVOKE / subprogram ────────────────────────────────────
        "CALL"                      => Token::Call,
        "INVOKE"                    => Token::Invoke,
        "USING"                     => Token::Using,
        "RETURNING"                 => Token::Returning,
        "REFERENCE"                 => Token::Reference,
        "END-CALL"                  => Token::EndCall,
        "CANCEL"                    => Token::Cancel,
        "ADDRESS-OF"                => Token::AddressOf,

        // ── Intrinsic functions ───────────────────────────────────────────
        "FUNCTION"                  => Token::Function,

        // ── Figurative constants ──────────────────────────────────────────
        "SPACE" | "SPACES"          => Token::Spaces,
        "ZERO" | "ZEROS" | "ZEROES" => Token::Zeros,
        "HIGH-VALUE" | "HIGH-VALUES" => Token::HighValues,
        "LOW-VALUE"  | "LOW-VALUES"  => Token::LowValues,
        "QUOTE" | "QUOTES"          => Token::Quotes,
        "NULL" | "NULLS"            => Token::Nulls,

        // ── Sort / merge ──────────────────────────────────────────────────
        "SORT"                      => Token::Sort,
        "MERGE"                     => Token::Merge,
        "ASCENDING"                 => Token::Ascending,
        "DESCENDING"                => Token::Descending,
        "SEQUENCE"                  => Token::Sequence,
        "END-SORT"                  => Token::EndSort,
        "END-MERGE"                 => Token::EndMerge,
        "RELEASE"                   => Token::Release,
        "RETURN"                    => Token::Return_,
        "END-RETURN"                => Token::EndReturn,
        "COMMIT"                    => Token::Commit,
        "ROLLBACK"                  => Token::Rollback,

        // ── PowerCOBOL / Fujitsu extensions ──────────────────────────────
        "WINDOW-STATUS"             => Token::WindowStatus,
        "WINDOW-OPEN"               => Token::WindowOpen,
        "WINDOW-CLOSE"              => Token::WindowClose,
        // Legacy COBOLT- names kept for backward compatibility
        "COBOLT-WAIT-EVENT"         => Token::CoboltWaitEvent,
        "COBOLT-SET-PROPERTY"       => Token::CoboltSetProperty,
        "COBOLT-GET-PROPERTY"       => Token::CoboltGetProperty,
        // Current COBOL- names (preferred)
        "COBOL-WAIT-EVENT"          => Token::CoboltWaitEvent,
        "COBOL-SET-PROPERTY"        => Token::CoboltSetProperty,
        "COBOL-GET-PROPERTY"        => Token::CoboltGetProperty,

        // ── CoBolt animation extensions ───────────────────────────────────
        "PLAY"                      => Token::Play,
        "STOP-ANIMATION"            => Token::StopAnim,

        // ── EXEC RUST inline Rust code block ──────────────────────────────
        // EXEC is only a keyword here; the lexer's block-capture logic in
        // Lexer::next_token() handles the full EXEC RUST … END-EXEC span.
        "EXEC"                      => Token::Exec,
        "END-EXEC"                  => Token::EndExec,

        // ── CoBolt exception handling (non-standard) ──────────────────────
        "TRY"                       => Token::Try,
        "CATCH"                     => Token::Catch,
        "EXCEPTION"                 => Token::Exception,
        "FINALLY"                   => Token::Finally,
        "END-TRY"                   => Token::EndTry,
        "THROW" | "RAISE"           => Token::Throw,

        // ── Not a keyword ─────────────────────────────────────────────────
        _ => return None,
    };
    Some(tok)
}

/// Returns `true` if a raw numeric value is a valid COBOL level number.
///
/// Valid levels: 01–49 (group/elementary items), 66 (RENAMES), 77 (independent),
/// 78 (constant), 88 (condition name).
pub fn is_level_number(n: u64) -> bool {
    matches!(n, 1..=49 | 66 | 77 | 78 | 88)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_keywords() {
        assert_eq!(lookup("MOVE"),            Some(Token::Move));
        assert_eq!(lookup("PERFORM"),         Some(Token::Perform));
        assert_eq!(lookup("WORKING-STORAGE"), Some(Token::WorkingStorage));
        assert_eq!(lookup("END-IF"),          Some(Token::EndIf));
        assert_eq!(lookup("COMP-3"),          Some(Token::Comp3));
    }

    #[test]
    fn aliases() {
        assert_eq!(lookup("PIC"),   lookup("PICTURE"));
        assert_eq!(lookup("COMP"),  lookup("COMPUTATIONAL"));
        assert_eq!(lookup("THRU"),  lookup("THROUGH"));
        assert_eq!(lookup("CORR"),  lookup("CORRESPONDING"));
        assert_eq!(lookup("SPACE"), lookup("SPACES"));
        assert_eq!(lookup("ZERO"),  lookup("ZEROS"));
    }

    #[test]
    fn unknown_is_none() {
        assert_eq!(lookup("MY-COUNTER"), None);
        assert_eq!(lookup("WS-NAME"),    None);
        assert_eq!(lookup("FOOBAR"),     None);
    }

    #[test]
    fn level_numbers() {
        assert!(is_level_number(1));
        assert!(is_level_number(49));
        assert!(is_level_number(77));
        assert!(is_level_number(88));
        assert!(!is_level_number(0));
        assert!(!is_level_number(50));
        assert!(!is_level_number(100));
    }
}

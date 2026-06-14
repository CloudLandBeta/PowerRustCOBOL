// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Token definitions for the Cobolt COBOL lexer.
//!
//! # Design notes
//!
//! COBOL keywords are case-insensitive and many span multiple words joined by
//! hyphens (e.g. `WORKING-STORAGE`, `END-IF`, `NOT-EQUAL`).  Rather than
//! teaching `logos` about ~130 case-insensitive multi-word patterns, we use a
//! two-pass strategy:
//!
//! 1. `logos` produces a raw [`RawToken`] stream that handles string/numeric
//!    literals and punctuation directly, but treats every identifier-shaped
//!    word (letters + digits + hyphens) as [`RawToken::Word`].
//!
//! 2. The [`Lexer`](crate::Lexer) inspects each `Word`, upper-cases it, and
//!    looks it up in the keyword table ([`crate::keywords`]).  Words that are
//!    not keywords become [`Token::Identifier`].
//!
//! This keeps the logos grammar tiny and makes keyword addition trivial.

use logos::Logos;

// ─────────────────────────────────────────────────────────────────────────────
// Raw token (logos layer)
// ─────────────────────────────────────────────────────────────────────────────

/// Internal token produced directly by the `logos` lexer.
///
/// Consumers should work with [`Token`] and [`SpannedToken`] instead.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r]+")] // skip horizontal whitespace; newlines are kept for line tracking
pub enum RawToken {
    // ── Newline (used to advance line counter) ─────────────────────────────
    #[token("\n")]
    Newline,

    // ── Free-form comment: *> to end of line ───────────────────────────────
    #[regex(r"\*>[^\n]*", |lex| lex.slice()[2..].trim().to_string())]
    FreeComment(String),

    // ── String literals ────────────────────────────────────────────────────
    // Double-quoted: "Hello, World!"  (doubled "" is an escaped quote)
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    StringDouble(String),

    // Single-quoted: 'Hello'  (doubled '' is an escaped quote)
    #[regex(r"'([^'\\]|\\.)*'", |lex| {
        let s = lex.slice();
        s[1..s.len()-1].to_string()
    })]
    StringSingle(String),

    // ── Numeric literals ───────────────────────────────────────────────────
    // Float must be tried before integer so "3.14" doesn't become "3" + ".14".
    // Note: signed literals (+3.14, -0.5) are NOT handled here — the lexer
    // emits the sign as a separate Plus/Minus token and the parser folds it.
    // This avoids ambiguity with `COMPUTE X = Y - 3.14`.
    // Capture the raw digits so the parser can build an *exact* fixed-point
    // decimal (parsing to f64 here would lose precision before arithmetic runs).
    #[regex(r"[0-9]+\.[0-9]+", |lex| Some(lex.slice().to_string()))]
    Float(Option<String>),

    #[regex(r"[0-9]+", |lex| lex.slice().parse::<u64>().ok())]
    Integer(Option<u64>),

    // ── Words: keywords and identifiers ───────────────────────────────────
    // COBOL words: start with a letter, may contain digits and hyphens,
    // must not end with a hyphen (the trailing-hyphen rule is enforced in
    // the Lexer layer, not here, to keep this regex simple).
    #[regex(r"[A-Za-z][A-Za-z0-9\-]*", |lex| lex.slice().to_string())]
    Word(String),

    // ── Operators (longest match first) ────────────────────────────────────
    #[token("**")]  Power,
    #[token("<=")]  LtEq,
    #[token(">=")]  GtEq,
    #[token("<>")]  NotEq,
    #[token("=")]   Eq,
    #[token("<")]   Lt,
    #[token(">")]   Gt,
    #[token("+")]   Plus,
    #[token("-")]   Minus,
    #[token("*")]   Star,
    #[token("/")]   Slash,

    // ── Punctuation ────────────────────────────────────────────────────────
    #[token(".")]   Period,
    #[token(",")]   Comma,
    #[token(";")]   Semicolon,
    #[token("(")]   LParen,
    #[token(")")]   RParen,
    #[token(":")]   Colon,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public Token
// ─────────────────────────────────────────────────────────────────────────────

/// The canonical token type produced by the Cobolt lexer.
///
/// Every keyword is its own variant so the parser can `match` on it without
/// string comparisons.  Identifiers, literals, and errors carry their payload.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Division headers
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Identification,
    Environment,
    Data,
    Procedure,
    Division,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Section names
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Section,
    Configuration,
    SourceComputer,     // SOURCE-COMPUTER
    ObjectComputer,     // OBJECT-COMPUTER
    InputOutput,        // INPUT-OUTPUT
    FileControl,        // FILE-CONTROL
    FileSection,        // FILE (when used as section header)
    WorkingStorage,     // WORKING-STORAGE
    LocalStorage,       // LOCAL-STORAGE
    Linkage,
    Screen,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Program structure
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ProgramId,          // PROGRAM-ID
    Author,
    DateWritten,        // DATE-WRITTEN
    DateCompiled,       // DATE-COMPILED

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Data definition
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Pic,                // PIC  (alias for PICTURE)
    Picture,            // PICTURE
    Value,
    Values,
    Occurs,
    Times,
    Depending,          // DEPENDING ON
    On,
    Redefines,
    Renames,
    Filler,
    Global,
    External,
    Blank,              // BLANK WHEN ZERO
    When,
    Zero,               // also figurative constant
    Justified,          // JUSTIFIED RIGHT
    Right,
    Synchronized,       // SYNCHRONIZED LEFT/RIGHT
    Left,
    Sign,
    Leading,
    Trailing,
    Separate,
    Character,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // USAGE clause keywords
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Usage,
    Display,            // USAGE DISPLAY  (also DISPLAY verb)
    Binary,
    Comp,               // COMPUTATIONAL / COMP
    Comp1,              // COMP-1 (float32)
    Comp2,              // COMP-2 (float64)
    Comp3,              // COMP-3 / PACKED-DECIMAL
    Comp5,              // COMP-5 (native binary)
    PackedDecimal,      // PACKED-DECIMAL
    Pointer,            // POINTER usage
    Index,
    NationalUsage,      // NATIONAL

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Arithmetic verbs
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Add,
    Subtract,
    Multiply,
    Divide,
    Compute,
    To,
    From,
    By,
    Giving,
    Remainder,
    Rounded,
    EndAdd,             // END-ADD
    EndSubtract,        // END-SUBTRACT
    EndMultiply,        // END-MULTIPLY
    EndDivide,          // END-DIVIDE
    EndCompute,         // END-COMPUTE
    SizeError,          // SIZE ERROR (ON SIZE ERROR)
    NotSizeError,       // NOT ON SIZE ERROR (handled as compound)

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Move / Set / Initialize
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Move,
    Set,
    Initialize,
    Corresponding,      // CORRESPONDING / CORR
    To_,                // alias, same meaning as To but used in SET ... TO TRUE

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Conditional / control flow
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    If,
    Else,
    EndIf,              // END-IF
    Evaluate,
    Also,
    Other,
    EndEvaluate,        // END-EVALUATE
    EndPerform,         // END-PERFORM
    Perform,
    Until,
    Varying,
    After,
    Before,
    Test,
    Through,            // THROUGH / THRU
    Thru,
    With,
    No,
    Go,
    GoTo,               // GO TO (two words, treated as one token for convenience)
    GoBack,             // GO BACK
    Stop,
    Run,
    Exit,
    End,                // END (bare — used in END PROGRAM / END DECLARATIVES)
    Program,
    Declaratives,       // DECLARATIVES (procedure-division declaratives block)
    Use,                // USE (USE AFTER STANDARD ERROR PROCEDURE …)
    Continue,
    Not,
    And,
    Or,
    Is,
    Are,
    In,
    Of,
    Than,
    Equal,
    Greater,
    Less,
    OrEqual,            // part of GREATER OR EQUAL / LESS OR EQUAL
    True_,              // TRUE (condition)
    False_,             // FALSE (condition)

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // String manipulation
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    StringVerb,         // STRING (verb, distinct from string literal)
    Unstring,
    Inspect,
    Tallying,
    Replacing,
    Converting,
    Delimited,
    By_,                // BY (inside STRING/UNSTRING)
    Into,
    Count,
    All,
    Leading_,           // ALL/LEADING inside INSPECT

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // I/O verbs and clauses
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Open,
    Close,
    Read,
    Write,
    Rewrite,
    Delete,
    Start,
    Accept,
    DisplayVerb,        // DISPLAY (verb form; same text as Display usage kw)
    Input,
    Output,
    IoMode,             // I-O
    Extend,
    AtEnd,              // AT END
    NotAtEnd,           // NOT AT END
    EndRead,            // END-READ
    EndWrite,           // END-WRITE
    EndRewrite,         // END-REWRITE
    EndDelete,          // END-DELETE
    EndStart,           // END-START
    EndString,          // END-STRING
    EndUnstring,        // END-UNSTRING
    EndSearch,          // END-SEARCH
    InvalidKey,         // INVALID KEY
    NotInvalidKey,      // NOT INVALID KEY
    Upon,               // DISPLAY ... UPON
    From_,              // ACCEPT ... FROM
    With_,              // WITH NO ADVANCING
    Advancing,
    Line,
    Lines,
    Key,
    Record,
    File,
    Select,
    Assign,
    Organization,
    Sequential,
    Indexed,
    Relative,
    Access,
    Mode,
    Random,
    Dynamic,
    RecordKey,          // RECORD KEY
    AlternateRecord,    // ALTERNATE RECORD KEY
    Status,
    Fd,                 // FD file descriptor
    Sd,                 // SD sort descriptor
    Block,
    Contains,
    Characters,
    Records,
    Label,
    Standard,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // CALL / INVOKE / subprogram
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Call,
    Invoke,             // OO-COBOL INVOKE object 'method' [USING ...] [RETURNING ...]
    Using,
    Returning,
    Reference,
    Content_,           // CALL ... BY CONTENT
    Value_,             // CALL ... BY VALUE
    EndCall,            // END-CALL
    Cancel,
    AddressOf,          // ADDRESS OF

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Intrinsic functions
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Function,
    // Individual function names are kept as Identifier tokens;
    // the parser recognises them by name after FUNCTION.

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Figurative constants
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Spaces,             // SPACES / SPACE
    Zeros,              // ZEROS / ZEROES / ZERO
    HighValues,         // HIGH-VALUES / HIGH-VALUE
    LowValues,          // LOW-VALUES / LOW-VALUE
    Quotes,             // QUOTES / QUOTE
    Nulls,              // NULLS / NULL
    AllLiteral,         // ALL "x"

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Sort / merge verbs
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Sort,
    Merge,
    Ascending,
    Descending,
    Sequence,
    Output_,            // OUTPUT PROCEDURE
    Input_,             // INPUT PROCEDURE
    EndSort,            // END-SORT
    EndMerge,           // END-MERGE
    Release,
    Return_,            // RETURN (from sort)
    EndReturn,          // END-RETURN
    Commit,             // COMMIT (indexed-file transaction)
    Rollback,           // ROLLBACK (indexed-file transaction)

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // PowerCOBOL / Fujitsu GUI extensions
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    WindowStatus,       // WINDOW-STATUS
    WindowOpen,         // WINDOW-OPEN  (legacy)
    WindowClose,        // WINDOW-CLOSE (legacy)
    CoboltWaitEvent,    // COBOLT-WAIT-EVENT / COBOL-WAIT-EVENT
    CoboltSetProperty,  // COBOLT-SET-PROPERTY / COBOL-SET-PROPERTY
    CoboltGetProperty,  // COBOLT-GET-PROPERTY / COBOL-GET-PROPERTY

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // CoBolt animation extensions
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    /// PLAY 'anim-name' [ON ctrl-id]  — trigger a named animation
    Play,
    /// STOP-ANIMATION [ON ctrl-id]  — halt a running animation
    StopAnim,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // EXEC RUST inline Rust code block
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// The `EXEC` keyword (part of `EXEC RUST … END-EXEC`).
    /// Normally consumed internally by the block-capture logic; only emitted
    /// standalone when `EXEC` appears without `RUST` (parser emits an error).
    Exec,

    /// `END-EXEC` — the closing delimiter of an `EXEC RUST` block.
    /// Only emitted standalone if found without a matching `EXEC RUST`.
    EndExec,

    /// An `EXEC RUST … END-EXEC` block captured verbatim.
    ///
    /// The string payload is the raw Rust source text between
    /// `EXEC RUST` and `END-EXEC`, with leading/trailing whitespace trimmed.
    ///
    /// # Binding conventions (at runtime)
    ///
    /// The Cobolt runtime compiles this source with an auto-generated preamble
    /// that binds every DATA DIVISION item as a Rust variable:
    ///
    /// | COBOL name  | Rust variable     | Rust type                  |
    /// |-------------|-------------------|----------------------------|
    /// | `WS-COUNT`  | `ws_count`        | `&mut i64`                 |
    /// | `WS-NAME`   | `ws_name`         | `&mut CobolString`         |
    /// | `WS-RATE`   | `ws_rate`         | `&mut Decimal`             |
    ///
    /// Additionally, the following handles are always in scope:
    ///
    /// * `cobol_env: &mut CobolEnvironment` — dynamic key/value access
    ///   (`cobol_env.get("WS-NAME")`, `cobol_env.set("WS-COUNT", 42)`)
    /// * `cobolt_objects: &mut ObjectRegistry` 
    ///   (`cobolt_objects.get("FORM1")?.set_property("Text", "Hello")`)
    ///
    /// Variable naming rule: hyphens are replaced with underscores and the
    /// name is lower-cased.  `WS-MY-FIELD` → `ws_my_field`.
    ExecRustBlock(String),

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // CoBolt exception handling extensions (non-standard COBOL)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    /// TRY  — opens a guarded block
    Try,
    /// CATCH — handles exceptions thrown inside TRY
    Catch,
    /// EXCEPTION — used in `CATCH EXCEPTION <name>`
    Exception,
    /// FINALLY — optional cleanup block
    Finally,
    /// END-TRY — closes the TRY/CATCH/FINALLY construct
    EndTry,
    /// THROW / RAISE — explicitly raise an exception
    Throw,

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Literals
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// Integer literal, e.g. `42` or `007`.
    IntegerLiteral(i64),

    /// Fixed-point decimal literal, e.g. `3.14` → `{ mantissa: 314, scale: 2 }`.
    /// Stored exactly (integer mantissa + decimal scale) — no `f64` rounding.
    DecimalLiteral { mantissa: i128, scale: u8 },

    /// String literal (contents without the surrounding quotes), e.g. `Hello`.
    StringLiteral(String),

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Level numbers (01–49, 66, 77, 78, 88)
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// A COBOL data-item level number such as `01`, `05`, `77`, `88`.
    LevelNumber(u8),

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Identifiers, comments, operators, punctuation
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// A user-defined name (data item, paragraph, section, program-id, etc.).
    Identifier(String),

    /// Source comment text (without the `*>` or `*` prefix).
    Comment(String),

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Power,      // **
    Eq,         // =
    Lt,         // <
    Gt,         // >
    LtEq,       // <=
    GtEq,       // >=
    NotEq,      // <>

    // Punctuation
    Period,     // .
    Comma,      // ,
    Semicolon,  // ;
    LParen,     // (
    RParen,     // )
    Colon,      // :

    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    // Sentinels
    // ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    /// End of the token stream.
    Eof,

    /// Unrecognised character(s) — the lexer never panics; errors are tokens.
    Error(String),
}

impl Token {
    /// `true` for tokens that carry no semantic content (comments).
    pub fn is_trivia(&self) -> bool {
        matches!(self, Token::Comment(_))
    }

    /// Human-readable name used in diagnostic messages.
    pub fn description(&self) -> &'static str {
        match self {
            Token::Identifier(_)      => "identifier",
            Token::IntegerLiteral(_)  => "integer literal",
            Token::DecimalLiteral { .. } => "decimal literal",
            Token::StringLiteral(_)   => "string literal",
            Token::LevelNumber(_)     => "level number",
            Token::ExecRustBlock(_)   => "EXEC RUST block",
            Token::Period             => "'.'",
            Token::Comma              => "','",
            Token::LParen             => "'('",
            Token::RParen             => "')'",
            Token::Eq                 => "'='",
            Token::Eof                => "end of file",
            Token::Error(_)           => "unexpected character",
            _                         => "keyword",
        }
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Identifier(s)       => write!(f, "{s}"),
            Token::IntegerLiteral(n)   => write!(f, "{n}"),
            Token::DecimalLiteral { mantissa, scale } => {
                if *scale == 0 {
                    write!(f, "{mantissa}")
                } else {
                    let s = mantissa.unsigned_abs().to_string();
                    let sc = *scale as usize;
                    let padded = if s.len() <= sc {
                        format!("{}{}", "0".repeat(sc + 1 - s.len()), s)
                    } else { s };
                    let at = padded.len() - sc;
                    write!(f, "{}{}.{}", if *mantissa < 0 { "-" } else { "" },
                           &padded[..at], &padded[at..])
                }
            }
            Token::StringLiteral(s)    => write!(f, "\"{s}\""),
            Token::LevelNumber(n)      => write!(f, "{n:02}"),
            Token::Comment(c)          => write!(f, "*> {c}"),
            Token::Error(e)            => write!(f, "ERROR({e})"),
            Token::ExecRustBlock(src)  => write!(f, "EXEC RUST {} END-EXEC", src),
            _                          => write!(f, "{}", self.description()),
        }
    }
}

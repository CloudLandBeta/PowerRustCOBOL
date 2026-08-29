// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `CobolEnvironment` — the runtime data store for one COBOL program.
//!
//! Holds the current value of every data item declared in the DATA DIVISION
//! and provides the API used by statement executors and `EXEC RUST` blocks.
//!
//! # Initialisation
//!
//! `CobolEnvironment::from_data_division` walks the parsed DATA DIVISION AST,
//! allocates a `CobolValue` for each named item, and applies any `VALUE`
//! clause initial values.

use indexmap::IndexMap;

use cobolt_ast::{
    data::{ConditionValue, DataDecl, PicKind, Usage},
    expr::Literal,
    program::{DataDivision, DataSection},
};

use crate::value::{CobolNumeric, CobolValue};

// ── ItemScope ────────────────────────────────────────────────────────────────

/// DATA DIVISION origin for a runtime data item, used by debugger snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemScope {
    WorkingStorage,
    FileDescription,
    LocalStorage,
    Linkage,
    Screen,
}

impl ItemScope {
    pub fn abbrev(self) -> &'static str {
        match self {
            Self::WorkingStorage => "WS",
            Self::FileDescription => "FD",
            Self::LocalStorage => "LS",
            Self::Linkage => "LK",
            Self::Screen => "SCREEN",
        }
    }
}

// ── CobolEnvironment ──────────────────────────────────────────────────────────

/// Hierarchy / occurrence metadata for one declared data item.
#[derive(Debug, Clone, Default)]
pub struct ItemSym {
    /// OCCURS counts of this item plus its ancestor groups, outermost first.
    /// Empty for a non-table item. A subscripted reference supplies one index
    /// per entry.
    pub dims: Vec<usize>,
    /// Immediate child item names (uppercased), for `CORRESPONDING`. Empty for
    /// an elementary item. 88-level condition-names and 66-level RENAMES are
    /// left out — COBOL-85 6.18.4 GR1 excludes both from correspondence.
    pub children: Vec<String>,
    /// Canonical storage keys of the immediate children, parallel to
    /// [`children`]. Used by `CORRESPONDING` to address the right occurrence of
    /// a duplicated child name.
    pub child_keys: Vec<String>,
    /// Storage keys of **every** subordinate item in declaration order,
    /// including unnamed `FILLER`. This is the group's byte layout, as opposed
    /// to [`child_keys`], which is the `CORRESPONDING` list and excludes items
    /// that have no name to correspond by. Empty for an elementary item.
    pub layout_keys: Vec<String>,
    /// Ancestor group names (uppercased), outermost first, for qualified-name
    /// (`A OF B`) disambiguation.
    pub quals: Vec<String>,
    /// True if this item is a group (has children).
    pub is_group: bool,
    /// `INDEXED BY` index-item names of this table's OCCURS (uppercased).
    pub index_names: Vec<String>,
    /// This table's own OCCURS count (its last dimension), 0 if not a table.
    pub occurs: usize,
    /// `ASCENDING/DESCENDING KEY` fields (uppercased, major-to-minor), each with
    /// its ascending flag. Empty unless the OCCURS declared sort keys; drives the
    /// `SEARCH ALL` binary search.
    pub keys: Vec<(String, bool)>,
    /// DATA DIVISION section that declared this item.
    pub scope: Option<ItemScope>,
    /// True if this item belongs to a GLOBAL declaration.
    pub is_global: bool,
    /// Raw PIC template, or an empty string for group/index items.
    pub pic: String,
    /// Digits after the implied decimal point (`V`), from the item's PICTURE.
    ///
    /// Needed to read a numeric item back through its own scale when its slot
    /// holds characters — a group `MOVE` puts `"123456"` into both `PIC 9(6)`
    /// and `PIC 9(4)V99`, and those are 123456 and 1234.56.
    pub pic_decimals: u16,
    /// Program/form/procedure that declared this item.
    pub origin: String,
    /// The `OCCURS … DEPENDING ON` item (uppercased), for a variable-length
    /// table. `None` for a fixed table and for a non-table item. The table's
    /// *current* length is read from it — [`occurs`](Self::occurs) stays the
    /// declared maximum, which is the storage that was reserved.
    pub depending_on: Option<String>,
}

/// The data store for a running COBOL program.
///
/// Data items are addressed by their COBOL name (uppercase, hyphens preserved).
/// Subscripted table elements are stored under synthesized keys `NAME(i[,j…])`
/// created lazily from the base item's default; the base `NAME` slot doubles as
/// the template. Items that have not been initialised hold `CobolValue::Unset`.
#[derive(Debug, Default)]
pub struct CobolEnvironment {
    /// `name → value` store.  Insertion order is preserved (declaration order).
    store: IndexMap<String, CobolValue>,
    /// `name → (integer-digit capacity, decimal places)` for numeric items,
    /// used to detect ON SIZE ERROR overflow at store time.
    field_caps: IndexMap<String, (u8, u8)>,
    /// `name → raw PIC template` for numeric-edited items. A numeric value stored
    /// into such a field is run through the edit engine and kept as a string.
    edited_templates: IndexMap<String, String>,
    /// Names of edited items declared `BLANK WHEN ZERO` — storing a zero value
    /// blanks the whole field.
    blank_when_zero: std::collections::HashSet<String>,
    /// `DECIMAL-POINT IS COMMA` — comma is the decimal point and period the
    /// grouping symbol in edited PICs.
    decimal_comma: bool,
    /// `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` — the character a currency
    /// position prints in an edited PICTURE.
    ///
    /// `None` means the standard's own default, `$`. It is an `Option` rather
    /// than a plain `char` so `#[derive(Default)]` still gives the right
    /// answer: a defaulted `char` is NUL, which would print as one.
    currency_symbol: Option<char>,
    /// Hierarchy / OCCURS metadata, keyed by the item's canonical storage key.
    symbols: IndexMap<String, ItemSym>,
    /// Leaf names that occur more than once in the program (under different
    /// groups). Only these need qualified (disambiguated) storage keys; every
    /// other name keys directly by itself, preserving the flat-store fast path.
    dup_names: std::collections::HashSet<String>,
    /// Leaf name → the canonical storage keys that share it (for resolution of
    /// `A OF B` qualified references). Only populated for duplicated names.
    by_leaf: IndexMap<String, Vec<String>>,
    /// 88-level condition-names → their parent item key + VALUE set. One name
    /// may be declared under several groups (CCVS85 declares `EQUALS-A` under
    /// three separate tables), so this holds **every** declaration in source
    /// order and the reference's `OF`/`IN` chain picks between them.
    cond_names: IndexMap<String, Vec<CondName>>,
    /// Pointer address table: `addr_of(key)` returns `index + 1` (0 = NULL).
    addr_table: Vec<String>,
    /// `SET ADDRESS OF item TO ptr` aliases: alias key → target storage key.
    addr_aliases: IndexMap<String, String>,
    /// Elementary item keys in declaration order (for 66 RENAMES ranges).
    elem_order: Vec<String>,
    /// 66-level RENAMES items → the covered elementary keys (in order), keyed
    /// by the item's **canonical** storage key exactly as a data item is, so a
    /// name declared once per record (`66 RENAME-5` under both `T-RENAMES-DATA`
    /// and `U-RENAMES-DATA`) keeps one entry per declaration.
    renames: IndexMap<String, Vec<String>>,
    /// A 66-level item's own ancestor path (its logical record), outermost
    /// first. `build_tree` makes a 66 a *root*, so it has no parent to take a
    /// path from — but COBOL keeps it subordinate to the record whose items it
    /// regroups, and `RENAME-5 OF T-RENAMES-DATA` must resolve through that.
    /// `resolve_canonical` reads this for any candidate key that has no
    /// [`ItemSym`] of its own.
    renames_quals: IndexMap<String, Vec<String>>,
    /// The record (01/77) most recently initialised — the implicit qualifier of
    /// any 66-level RENAMES that follows it in declaration order.
    last_record: Option<String>,
    /// Canonical storage keys of `EXTERNAL` items (01/77-level and EXTERNAL FD
    /// records), including all of their subordinate keys. These are shared
    /// run-unit-wide via the [`ExternalStore`] (spec 005); GLOBAL items are not
    /// listed here — they are shared only within a program's nested units.
    external_names: std::collections::HashSet<String>,
    /// `REDEFINES` pairs as declared: `(redefining key, target key)`. Collected
    /// while the DATA DIVISION is walked and folded into [`Self::redefine_links`]
    /// once every symbol is known.
    redefine_pairs: Vec<(String, String)>,
    /// Every storage key declared **with** a `REDEFINES` clause. `redefine_pairs`
    /// cannot answer this — `build_redefine_links` takes it — and the fact is
    /// needed long after: COBOL-85 6.18.4 GR1 leaves a redefining item out of
    /// `CORRESPONDING`, along with everything subordinate to it.
    redefinitions: std::collections::HashSet<String>,
    /// Live `REDEFINES` overlay: a storage key that lies inside one of two
    /// overlaid descriptions maps to `(the description it belongs to, a
    /// description that shares those bytes)`. A write anywhere inside one is
    /// re-rendered into every other, which is what makes
    /// `MOVE x TO COMPUTED-N` visible when the program then reads `COMPUTED-A`
    /// or the group above them.
    redefine_links: IndexMap<String, Vec<(String, String)>>,
    /// Guard: the descriptions a refresh is **currently writing**. A refresh
    /// writes through `set`, which would otherwise bounce straight back into
    /// the description that started it.
    ///
    /// Per-description rather than a single flag, because overlays nest: a
    /// write inside `REDEF12 REDEFINES REDEF10` has to reach `REDEF10`, then
    /// `RDF3 REDEFINES RDFDATA3` inside it, then `RDF3-5-1 REDEFINES RDF3-5`
    /// inside *that* (NC252A RDF-TEST-12). One flag stopped the chain at the
    /// first hop; blocking only the description being written lets each further
    /// overlay fire exactly once and still terminates.
    syncing: std::collections::HashSet<String>,
    /// Numeric items whose PICTURE carries `P` scaling positions:
    /// `key → (trailing Ps, leading Ps)`. Those positions are not stored, so
    /// they always read as zero — see [`pic_scaling`].
    scaling_p: IndexMap<String, (u32, u32)>,
    /// Numeric items declared **without** `S`. They carry no operational sign,
    /// so a negative sender is stored as its absolute value — `PIC 9(18)`
    /// receiving `-733…` holds `733…`, which is what the standard's "the
    /// absolute value is used" rule means in practice.
    unsigned_numeric: std::collections::HashSet<String>,
    /// Alphanumeric items declared `JUSTIFIED RIGHT`. See [`Self::set`].
    justified: std::collections::HashSet<String>,
    /// Alphanumeric-**edited** items: `key → PICTURE template`. Their insertion
    /// characters (`B` → space, `0` → zero, `/` → slash) belong to the item, not
    /// to whatever is moved into it, so every store re-applies the template.
    /// See [`apply_alnum_edit`].
    alnum_edited: IndexMap<String, String>,
    /// Signed numeric DISPLAY items declared `SIGN IS … SEPARATE CHARACTER`:
    /// `key → leading?`. The sign occupies its own character position — one
    /// **more** than the item's digit positions — and always holds a literal
    /// `+` or `-`, never a space, even for a value that arrived unsigned.
    ///
    /// Only SEPARATE appears here. With an embedded sign the item is exactly
    /// its digit positions wide and the sign rides on the leading or trailing
    /// digit, which is the representation the store already has.
    sign_separate: IndexMap<String, bool>,
    /// `REDEFINES` descriptions that **share storage outright**, mapping each
    /// redefining item's template key to the corresponding key of the
    /// description it redefines (`ENTRY-1-1` → `ENTRY-1`).
    ///
    /// Only layout-identical descriptions qualify — same items, same OCCURS
    /// dimensions, same PICTUREs — because then the two readings are the same
    /// bytes read the same way and one slot can serve both. That is the case
    /// [`REDEFINE_SYNC_BUDGET`] exists to give up on: copying a redefined
    /// 10×10×10 table on every write is ruinous, but sharing it costs nothing.
    ///
    /// Storage only. The symbol table stays per-description, so each keeps its
    /// own `INDEXED BY` names — `SEARCH GRP-ENTRY-1` must still be driven by
    /// `IDX-1-1` and not by the redefined table's `IDX-1`.
    redefine_aliases: IndexMap<String, String>,
    /// Nesting depth of a group **move into** storage; zero when reading.
    ///
    /// A group containing an `OCCURS … DEPENDING ON` table uses the table's
    /// *current* length when it is a **sending** item and its declared
    /// **maximum** when it is a **receiving** one (COBOL-85 VI-26 5.8.3 SR5).
    /// The asymmetry is what makes `MOVE ODO-RECORD TO NEW-RECORD` copy all
    /// nine occurrences into a receiver whose own depending item still reads 3
    /// — the move is what sets that item, so its old value cannot bound the
    /// receiver. It is a depth counter so the recursion into subordinate groups
    /// inherits the mode and only the outermost call clears it.
    odo_receiving: u32,
}

/// Distribute `src`'s characters across an alphanumeric-edited PICTURE.
///
/// `X`, `A` and `9` take the sender's next character (padding with spaces once
/// it runs out); `B`, `0` and `/` are **insertion** characters and print
/// themselves. `PIC XXBXX/XX` receiving spaces reads `"     /  "`, which is
/// what `INITIALIZE` on such an item has to leave behind (NC223A) and what
/// `MOVE "ABCDEF" TO it` turns into `"AB CD/EF"` (NC114M).
/// Works on **bytes**, not characters: `HIGH-VALUE` is `0xFF`, which is not
/// valid UTF-8 and has no spelling one byte wide. Read through a `String` it
/// became the three bytes of U+FFFD, so `MOVE HIGH-VALUE TO <PIC XX0XXBXXX>`
/// filled the first source position with a replacement character and padded the
/// rest with spaces (NC105A `MOVE-TEST-F1-69`).
fn apply_alnum_edit(template: &str, src: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut chars = src.iter().copied();
    let up: Vec<char> = template.to_ascii_uppercase().chars().collect();
    let mut i = 0usize;
    while i < up.len() {
        let c = up[i];
        i += 1;
        // `X(4)` and friends: a repeat count applies to the symbol before it.
        let mut repeat = 1usize;
        if up.get(i) == Some(&'(') {
            let mut j = i + 1;
            let mut n = 0usize;
            while j < up.len() && up[j].is_ascii_digit() {
                n = n * 10 + (up[j] as usize - '0' as usize);
                j += 1;
            }
            if up.get(j) == Some(&')') {
                repeat = n.max(1);
                i = j + 1;
            }
        }
        for _ in 0..repeat {
            match c {
                'X' | 'A' | '9' => out.push(chars.next().unwrap_or(b' ')),
                'B' => out.push(b' '),
                '0' => out.push(b'0'),
                '/' => out.push(b'/'),
                // Anything else in an alphanumeric-edited picture takes a
                // character position of its own and is copied through.
                other => {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
                }
            }
        }
    }
    out
}

/// The bytes of `s` with trailing spaces removed — the significant part of a
/// COBOL alphanumeric value, which a `JUSTIFIED` receiver aligns at its right.
fn trim_trailing_spaces(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    &bytes[..end]
}

/// Run-unit-wide `EXTERNAL` data store.
///
/// COBOL `EXTERNAL` items are shared *by name* across every program in a run
/// unit (the process): there is exactly one physical copy. This map is that
/// copy. It lives behind `Arc<Mutex<…>>` so a run unit spanning several
/// interpreter instances — e.g. multiple form modules, possibly on different
/// threads — can share it. Create one per run unit and clone the `Arc` to hand
/// the same store to another interpreter.
pub type ExternalStore = std::sync::Arc<std::sync::Mutex<IndexMap<String, CobolValue>>>;

/// Create a fresh, empty run-unit `EXTERNAL` store.
pub fn new_external_store() -> ExternalStore {
    std::sync::Arc::new(std::sync::Mutex::new(IndexMap::new()))
}

/// An 88-level condition-name: the parent data item it qualifies and the set of
/// values (single or `THRU` ranges) for which the condition is true.
#[derive(Debug, Clone)]
pub struct CondName {
    /// Canonical storage key of the parent (host) item.
    pub parent: String,
    /// The `VALUE` entries that make the condition true.
    pub values: Vec<ConditionValue>,
    /// The condition-name's own ancestor path, **outermost first**: the host
    /// item's path plus the host's own name. A condition-name is qualified by
    /// the groups above it exactly as a data item is — including the host —
    /// so `EQUALS-A OF TABLE-LEVEL-5 OF … OF GROUP-1-TABLE` is matched against
    /// this path with the same subsequence rule `resolve_canonical` uses.
    pub quals: Vec<String>,
}

/// Tally every named (non-FILLER) leaf in a declaration subtree, so the
/// environment knows which names are duplicated and need qualified keys.
fn count_names(decl: &DataDecl, counts: &mut std::collections::HashMap<String, usize>) {
    if let Some(n) = &decl.name {
        let u = n.to_ascii_uppercase();
        if u != "FILLER" {
            *counts.entry(u).or_insert(0) += 1;
        }
    }
    for child in &decl.children {
        count_names(child, counts);
    }
}

/// `true` if `needle` appears as an (order-preserving, not necessarily
/// contiguous) subsequence of `haystack`.
fn is_subsequence(needle: &[String], haystack: &[&String]) -> bool {
    let mut it = haystack.iter();
    needle.iter().all(|q| it.any(|h| h.eq_ignore_ascii_case(q)))
}

/// The storage key of the `idx`-th subordinate item of `parent` when that item
/// is unnamed (`FILLER`, or the level number alone).
///
/// FILLER holds bytes and may carry a VALUE, so it has to be stored somewhere —
/// but it has no name, and nothing in the COBOL program may reach it. `\u{2}`
/// cannot appear in a COBOL word, so a key built with it is addressable by the
/// group's layout and by nothing else.
fn filler_key(parent: &str, idx: usize) -> String {
    format!("{parent}\u{2}{idx}")
}

/// `true` if `key` is one of those synthetic FILLER slots — used to keep them
/// out of anything a developer reads, such as the debugger's variable list.
fn is_filler_key(key: &str) -> bool {
    key.contains('\u{2}')
}

/// The base item name of a (possibly subscripted) storage key: `A(2)` → `A`.
pub fn base_name(key: &str) -> &str {
    match key.find('(') {
        Some(i) => &key[..i],
        None => key,
    }
}

/// Largest description, in expanded storage slots, that a live `REDEFINES`
/// overlay will keep in step on every write. See `build_redefine_links`.
const REDEFINE_SYNC_BUDGET: usize = 256;

/// The decimal scaling a plain numeric PICTURE's `P` positions apply.
///
/// `P` stands for a digit position the item does **not** store — it only says
/// where the decimal point sits relative to the digits that are. `PIC 999PP`
/// holds three digits standing for hundreds, so its value is the stored number
/// × 100 and its low two positions always read as zero; `PIC PP99` holds two
/// digits standing for ten-thousandths. An item therefore spans more digit
/// positions than its storage does, which is why the capacity the rest of this
/// file works with counts the `P`s while the record layout (bytes on disk)
/// does not.
///
/// Returns `(integer positions, decimal positions, trailing Ps, leading Ps)`,
/// or `None` for a picture with no `P` in it.
fn pic_scaling(template: &str) -> Option<(u16, u16, u32, u32)> {
    let chars: Vec<char> = template.to_ascii_uppercase().chars().collect();
    let mut syms: Vec<char> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        let mut n = 1usize;
        if chars.get(i) == Some(&'(') {
            let mut j = i + 1;
            let mut v = 0usize;
            while j < chars.len() && chars[j].is_ascii_digit() {
                v = v * 10 + (chars[j] as usize - '0' as usize);
                j += 1;
            }
            if chars.get(j) == Some(&')') {
                n = v.max(1);
                i = j + 1;
            }
        }
        if !matches!(c, '9' | 'P' | 'V') {
            continue; // `S` and anything else occupies no digit position here
        }
        for _ in 0..n {
            syms.push(c);
        }
    }
    if !syms.contains(&'P') {
        return None;
    }
    let leading_p = syms.iter().take_while(|c| **c == 'P').count() as u32;
    let nines = syms.iter().filter(|c| **c == '9').count() as u32;
    if leading_p > 0 {
        // The implied point sits to the LEFT of the `P` run: every position,
        // stored or not, is fractional.
        return Some((0, (leading_p + nines) as u16, 0, leading_p));
    }
    // Trailing `P`s: the implied point sits to their RIGHT. A `V` written after
    // them is that point spelled out.
    let body: &[char] = match syms.last() {
        Some('V') => &syms[..syms.len() - 1],
        _ => &syms[..],
    };
    let trailing_p = body.iter().rev().take_while(|c| **c == 'P').count() as u32;
    if trailing_p == 0 {
        return None; // `P`s in the middle are not a picture COBOL allows
    }
    let v_at = body.iter().position(|c| *c == 'V');
    let (int_nines, frac_nines) = match v_at {
        Some(p) => (
            body[..p].iter().filter(|c| **c == '9').count() as u32,
            body[p + 1..].iter().filter(|c| **c == '9').count() as u32,
        ),
        None => (nines, 0),
    };
    Some((
        (int_nines + trailing_p) as u16,
        frac_nines as u16,
        trailing_p,
        0,
    ))
}

/// The subscripts a storage key carries: `A(2,3)` → `[2, 3]`; `A` → `[]`.
/// The inverse of [`subscript_key`].
pub fn key_indices(key: &str) -> Vec<i64> {
    let (Some(open), Some(close)) = (key.find('('), key.rfind(')')) else {
        return Vec::new();
    };
    if close <= open + 1 {
        return Vec::new();
    }
    key[open + 1..close]
        .split(',')
        .filter_map(|p| p.trim().parse::<i64>().ok())
        .collect()
}

/// Build the storage key for a subscripted reference: `("A", [2])` → `"A(2)"`.
pub fn subscript_key(base: &str, indices: &[i64]) -> String {
    if indices.is_empty() {
        return base.to_ascii_uppercase();
    }
    let parts: Vec<String> = indices.iter().map(|i| i.to_string()).collect();
    format!("{}({})", base.to_ascii_uppercase(), parts.join(","))
}

impl CobolEnvironment {
    /// Create an empty environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an environment pre-populated from a DATA DIVISION AST node.
    ///
    /// Each named data item gets a default value appropriate for its PIC
    /// clause (zeros for numeric, spaces for alphanumeric), then any `VALUE`
    /// clause is applied on top.
    pub fn from_data_division(data: &DataDivision) -> Self {
        Self::from_data_division_with(data, false)
    }

    /// Like [`from_data_division`], but with the program's `DECIMAL-POINT IS COMMA`
    /// setting (affects how edited PICs are formatted).
    /// The character a currency position prints in an edited PICTURE.
    pub fn currency(&self) -> char {
        self.currency_symbol.unwrap_or('$')
    }

    /// Record the program's `SPECIAL-NAMES. CURRENCY` symbol.
    pub fn set_currency(&mut self, c: char) {
        self.currency_symbol = Some(c);
    }

    /// Whether `SPECIAL-NAMES. DECIMAL-POINT IS COMMA` is in force, which swaps
    /// the roles of `.` and `,` in every numeric literal and edited PICTURE.
    pub fn decimal_comma(&self) -> bool {
        self.decimal_comma
    }

    pub fn from_data_division_with(data: &DataDivision, decimal_comma: bool) -> Self {
        Self::from_data_division_with_origin(data, decimal_comma, '$', "")
    }

    /// Like [`from_data_division_with`], and records the declaring program/form
    /// name for debugger variable details.
    /// `currency` must be supplied here rather than set afterwards: an edited
    /// item with a numeric `VALUE` is formatted while its declaration is being
    /// read, so a symbol installed after construction would arrive too late for
    /// the initial value and show `$` for the rest of the item's life.
    pub fn from_data_division_with_origin(
        data: &DataDivision,
        decimal_comma: bool,
        currency: char,
        origin: &str,
    ) -> Self {
        let mut env = Self::new();
        env.decimal_comma = decimal_comma;
        env.set_currency(currency);
        let origin = origin.to_owned();
        // Pass 1: count every leaf name so we know which need disambiguation.
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        count_names(decl, &mut counts);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            count_names(rec, &mut counts);
                        }
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
        env.dup_names = counts
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|(n, _)| n)
            .collect();
        // Pass 2: initialise values + hierarchy under canonical keys, recording
        // EXTERNAL items (01/77 and EXTERNAL FD records) for run-unit sharing.
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        let external = decl.is_external && (decl.level == 1 || decl.level == 77);
                        let scope = match section {
                            DataSection::WorkingStorage(_) => ItemScope::WorkingStorage,
                            DataSection::LocalStorage(_) => ItemScope::LocalStorage,
                            DataSection::Linkage(_) => ItemScope::Linkage,
                            _ => unreachable!(),
                        };
                        env.init_decl_tracking_external(decl, external, scope, false, &origin);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            env.init_decl_tracking_external(
                                rec,
                                rec.is_external,
                                ItemScope::FileDescription,
                                fd.is_global,
                                &origin,
                            );
                        }
                        // An FD has ONE record area, however many `01`s
                        // describe it: they are implicit redefinitions of one
                        // another, so a write through any of them is visible
                        // through the rest. CCVS85 leans on this in every
                        // program — the report line is built in `PRINT-REC`
                        // and written as `DUMMY-RECORD` — and without the
                        // overlay each `01` kept its own storage, so every
                        // heading came out blank and the held line was written
                        // once per page-break statement instead of once.
                        let mut records = fd.records.iter().filter_map(|r| r.name.as_ref());
                        if let Some(first) = records.next() {
                            let target = env.canon_key(&first.to_ascii_uppercase(), &[]);
                            for other in records {
                                let key = env.canon_key(&other.to_ascii_uppercase(), &[]);
                                env.redefine_pairs.push((key, target.clone()));
                            }
                        }
                    }
                }
                DataSection::Screen(_) => {} // screen items handled by forms layer
            }
        }
        sync_redefines(&mut env, data);
        env.build_redefine_links();
        env
    }

    /// Fold the declared `REDEFINES` pairs into the live overlay map.
    ///
    /// Two descriptions that redefine the same target overlay each other too,
    /// so the pairs form equivalence classes: `COMPUTED-A`, `COMPUTED-N`,
    /// `COMPUTED-4V14` and `CM-18V0` are four readings of the same twenty
    /// bytes, and a write through any one of them has to be visible through the
    /// other three. Every storage key *inside* a description triggers the
    /// refresh, so writing `COMPUTED-18V0` — subordinate to `CM-18V0` — updates
    /// `COMPUTED-A` as well.
    fn build_redefine_links(&mut self) {
        if self.redefine_pairs.is_empty() {
            return;
        }
        // target key → every description overlaying those bytes, target first.
        let mut classes: IndexMap<String, Vec<String>> = IndexMap::new();
        for (redefining, target) in std::mem::take(&mut self.redefine_pairs) {
            // A target that is itself a REDEFINES joins that same class.
            let root = classes
                .iter()
                .find(|(_, members)| members.contains(&target))
                .map(|(k, _)| k.clone())
                .unwrap_or(target);
            let members = classes.entry(root.clone()).or_insert_with(|| vec![root]);
            if !members.contains(&redefining) {
                members.push(redefining);
            }
        }
        for members in classes.values() {
            // A refresh re-renders one whole description and distributes it
            // across another, on every write anywhere inside either. That is
            // affordable for the flat records this exists for — CCVS's
            // `COMPUTED-A` / `COMPUTED-N` are twenty bytes — and ruinous for a
            // redefined 10×10×10 table, where one `MOVE` would walk a thousand
            // occurrences twice. Classes above the budget keep the old,
            // storage-per-description behaviour rather than making the program
            // unrunnable.
            if members.iter().any(|m| self.occurrence_weight(m) > REDEFINE_SYNC_BUDGET) {
                // Too big to keep in step by copying — but when the members are
                // layout-identical there is nothing to keep in step: they are
                // the same bytes read the same way, so they can share the slots
                // outright. Free, exact, and no budget applies. NC234A's
                // `3-DEM-TBL REDEFINES 3-DIMENSION-TBL` is this shape, and
                // without it every name in the redefining description read as
                // spaces however the redefined table had been filled.
                let root = members[0].clone();
                if members[1..].iter().all(|m| self.layouts_match(&root, m)) {
                    let root_keys = self.subtree_keys(&root);
                    let pairs: Vec<(String, String)> = members[1..]
                        .iter()
                        .flat_map(|m| {
                            self.subtree_keys(m)
                                .into_iter()
                                .zip(root_keys.iter().cloned())
                        })
                        .collect();
                    for (from, to) in pairs {
                        self.redefine_aliases.insert(from, to);
                    }
                }
                continue;
            }
            for own in members {
                let peers: Vec<(String, String)> = members
                    .iter()
                    .filter(|p| *p != own)
                    .map(|p| (own.clone(), p.clone()))
                    .collect();
                if peers.is_empty() {
                    continue;
                }
                // **Append**, never replace: a key inside a nested overlay
                // belongs to more than one class, and each of them has to fire.
                // `RDFDATA5` lies in both `RDF3 REDEFINES RDFDATA3` and
                // `REDEF12 REDEFINES REDEF10`; keeping only the last class
                // built left the inner overlay unreachable, so a write through
                // the outer one never reached `RDF3-5` (NC252A RDF-TEST-12).
                for trigger in self.subtree_keys(own) {
                    let slot = self.redefine_links.entry(trigger).or_default();
                    for pair in &peers {
                        if !slot.contains(pair) {
                            slot.push(pair.clone());
                        }
                    }
                }
            }
        }
    }

    /// Follow a `REDEFINES` storage alias, keeping any subscript: with
    /// `ENTRY-1-1` aliased to `ENTRY-1`, the key `ENTRY-1-1(2,3)` becomes
    /// `ENTRY-1(2,3)`.
    ///
    /// Takes the already-uppercased key and hands it straight back when nothing
    /// is aliased, which is every program that declares no layout-identical
    /// `REDEFINES` — the map is empty and the check is one comparison.
    fn storage_key(&self, key: String) -> String {
        if self.redefine_aliases.is_empty() {
            return key;
        }
        let base = base_name(&key);
        match self.redefine_aliases.get(base) {
            Some(target) => format!("{target}{}", &key[base.len()..]),
            None => key,
        }
    }

    /// Whether two descriptions cover their shared bytes the **same way** —
    /// the same items in the same order, each with the same OCCURS dimensions
    /// and the same PICTURE. Only then may they share storage.
    ///
    /// Conservative on purpose: an unnamed `FILLER` slot carries no symbol
    /// entry to compare, so a description containing one does not qualify and
    /// keeps the copying overlay.
    fn layouts_match(&self, a: &str, b: &str) -> bool {
        let (ka, kb) = (self.subtree_keys(a), self.subtree_keys(b));
        if ka.len() != kb.len() {
            return false;
        }
        ka.iter()
            .zip(kb.iter())
            .all(|(x, y)| match (self.symbols.get(x), self.symbols.get(y)) {
                (Some(sx), Some(sy)) => {
                    sx.dims == sy.dims && sx.pic == sy.pic && sx.is_group == sy.is_group
                }
                _ => false,
            })
    }

    /// How many storage slots one description covers once its OCCURS
    /// dimensions are expanded — the cost of rendering or distributing it once.
    fn occurrence_weight(&self, key: &str) -> usize {
        self.subtree_keys(key)
            .iter()
            .map(|k| {
                self.symbols
                    .get(k)
                    .map(|s| s.dims.iter().product::<usize>().max(1))
                    .unwrap_or(1)
            })
            .sum()
    }

    /// A description's own key plus every subordinate storage key under it.
    fn subtree_keys(&self, key: &str) -> Vec<String> {
        let mut out = vec![key.to_string()];
        let mut i = 0;
        while i < out.len() {
            if let Some(sym) = self.symbols.get(&out[i]) {
                for ck in sym.layout_keys.clone() {
                    if !out.contains(&ck) {
                        out.push(ck);
                    }
                }
            }
            i += 1;
        }
        out
    }

    /// Re-render the description a write landed in into every description that
    /// shares its bytes. Guarded so the refresh does not bounce back.
    fn refresh_redefine_peers(&mut self, key: &str) {
        let Some(peers) = self.redefine_links.get(base_name(key)).cloned() else {
            return;
        };
        for (own, peer) in peers {
            // Already in flight: `own` means this is the write coming back from
            // the refresh that produced it, `peer` a description another
            // refresh is in the middle of writing.
            if self.syncing.contains(&own) || self.syncing.contains(&peer) {
                continue;
            }
            self.syncing.insert(peer.clone());
            let mut bytes = self.display_string(&own).unwrap_or_default();
            // A **01-level** REDEFINES may describe more storage than the item
            // it redefines, and CCVS85 tests exactly that: NC107A overlays a
            // 46-byte `REDEF10` with a 92-byte `REDEF11` and a 120-byte
            // `REDEF12`, then reads `RDFDATA18` at offset 106 and
            // `RDFDATA8 (14)` at 46 — both past the redefined item's end.
            //
            // Bytes beyond the source's own length are not the source's to
            // describe, so the peer keeps its own. Rendering the shorter
            // description onto the longer one padded that tail with spaces and
            // erased it (RDF-TEST-9, RDF-TEST-10).
            if let Some(existing) = self.display_string(&peer) {
                if let Some(tail) = existing.get(bytes.len()..) {
                    bytes.push_str(tail);
                }
            }
            self.set_from_bytes(&peer, &bytes);
            self.syncing.remove(&peer);
        }
    }

    /// Store the character form `s` into `key`, reading it the way `key` is
    /// described: a group distributes it, an edited item keeps the characters,
    /// and a plain numeric reads them as its own digit positions with the
    /// implied decimal point in its declared place.
    ///
    /// The elementary branches write the slot directly rather than through
    /// [`Self::set`], so the refresh that produced this write does not re-enter
    /// through it — but they still refresh `key`'s **other** overlay classes
    /// afterwards, or a description nested inside the one being written never
    /// hears about it. `syncing` blocks only the description currently being
    /// written, so that terminates. (The group and edited branches already
    /// refresh through the writers they delegate to.)
    fn set_from_bytes(&mut self, key: &str, s: &str) {
        if self.is_group(key) {
            self.set_group(key, s);
            return;
        }
        if self.edited_templates.contains_key(base_name(key)) {
            self.set_str_left(key, s);
            return;
        }
        if let Some(&(int_digits, decimals)) = self.field_caps.get(base_name(key)) {
            let width = int_digits as usize + decimals as usize;
            // A REDEFINES overlay carries the target's **bytes**. When they
            // spell digits the numeric item reads them as its own digit
            // positions, which is what the filter below is for. When they do
            // not, the characters stand as they are: filtering the letters out
            // of `"00ABCDEFGHI  4321 "` invented the number 004321000000000000
            // and `IS NUMERIC` then answered yes about an item full of letters
            // (NC174A CLASS-TEST-GF-8 and CLASS-TEST-GF-10).
            //
            // A numeric slot holding characters is exactly what REDEFINES over
            // an alphanumeric item produces; using it in arithmetic afterwards
            // is undefined in COBOL-85, not ours to prevent — the same reading
            // `set_group_bytes` already takes for a group move.
            if !s
                .chars()
                .all(|c| c.is_ascii_digit() || c == ' ' || c == '+' || c == '-')
            {
                let mut bytes = s.as_bytes().to_vec();
                bytes.truncate(width);
                bytes.resize(width, b' ');
                self.store.insert(
                    key.to_string(),
                    CobolValue::String {
                        bytes,
                        capacity: width,
                    },
                );
                self.refresh_redefine_peers(key);
                return;
            }
            let mut digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() < width {
                digits.push_str(&"0".repeat(width - digits.len()));
            }
            digits.truncate(width);
            let mantissa = digits.parse::<i128>().unwrap_or(0);
            let signed = if s.contains('-') { -mantissa } else { mantissa };
            self.store
                .insert(key.to_string(), CobolValue::Numeric(CobolNumeric::new(signed, decimals)));
            self.refresh_redefine_peers(key);
            return;
        }
        self.set_str_left(key, s);
    }

    /// Initialise `decl`; when `external` is set, record every storage key it
    /// creates (the item and all its subordinates) as a run-unit EXTERNAL key.
    fn init_decl_tracking_external(
        &mut self,
        decl: &DataDecl,
        external: bool,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        if !external {
            self.init_decl(decl, scope, inherited_global, origin);
            return;
        }
        let before: std::collections::HashSet<String> = self.store.keys().cloned().collect();
        self.init_decl(decl, scope, inherited_global, origin);
        for k in self.store.keys() {
            if !before.contains(k) {
                self.external_names.insert(k.clone());
            }
        }
    }

    /// Canonical storage keys that are `EXTERNAL` (shared run-unit-wide).
    pub fn external_names(&self) -> &std::collections::HashSet<String> {
        &self.external_names
    }

    /// Direct read of a canonical storage key — no name resolution, no group
    /// cascade. Used by the run-unit EXTERNAL sync, which already holds
    /// canonical keys (from [`external_names`](Self::external_names)).
    pub fn raw_get(&self, key: &str) -> Option<&CobolValue> {
        self.store.get(key)
    }

    /// Direct write of a canonical storage key — no name resolution, no group
    /// cascade. Counterpart to [`raw_get`](Self::raw_get).
    pub fn raw_set(&mut self, key: &str, value: CobolValue) {
        self.store.insert(key.to_string(), value);
    }

    /// Canonical storage key for a leaf with the given ancestor path
    /// (outermost first). Unique names key by themselves (flat-store fast path);
    /// duplicated names get a path-qualified key that cannot collide.
    fn canon_key(&self, leaf: &str, path: &[String]) -> String {
        if self.dup_names.contains(leaf) {
            let mut k = String::from(leaf);
            for q in path {
                k.push('\u{1}');
                k.push_str(q);
            }
            k
        } else {
            leaf.to_string()
        }
    }

    /// Resolve a (possibly qualified) reference to its canonical storage key.
    /// `quals` are the `OF`/`IN` qualifiers, innermost first. A unique name
    /// resolves to itself; a duplicated name is matched against the candidates'
    /// ancestor paths (an ambiguous reference picks the first declaration).
    pub fn resolve_name(&self, leaf: &str, quals: &[String]) -> String {
        let key = self.resolve_canonical(leaf, quals);
        // A `SET ADDRESS OF item TO ptr` aliases `item` onto another item's
        // storage — redirect here so every interpreter reference follows it.
        if let Some(target) = self.addr_aliases.get(&key) {
            return target.clone();
        }
        // A 66-level RENAMES over exactly one item *is* that item, description
        // and all — so it is redirected for the same reason: every read keeps
        // the renamed item's category (a numeric one stays numeric instead of
        // arriving as its digit string) and every write lands in the renamed
        // item's slot rather than in the RENAMES' own, which nothing reads.
        if let Some(target) = self.renames_alias(&key) {
            return target;
        }
        key
    }

    /// The canonical storage key for a reference, **without** following an
    /// address alias (used when (re)defining the alias itself).
    pub fn canonical_name(&self, leaf: &str, quals: &[String]) -> String {
        self.resolve_canonical(leaf, quals)
    }

    fn resolve_canonical(&self, leaf: &str, quals: &[String]) -> String {
        let leaf = leaf.to_ascii_uppercase();
        if !self.dup_names.contains(&leaf) {
            return leaf;
        }
        let cands = match self.by_leaf.get(&leaf) {
            Some(c) => c,
            None => return leaf,
        };
        if cands.len() == 1 {
            return cands[0].clone();
        }
        if quals.is_empty() {
            return cands[0].clone();
        }
        let qs: Vec<String> = quals.iter().map(|q| q.to_ascii_uppercase()).collect();
        for k in cands {
            // A 66-level RENAMES has no `ItemSym` — it owns no storage — but it
            // is qualified by its record like anything else, so its path comes
            // from `renames_quals` instead.
            let path = match self.symbols.get(k) {
                Some(sym) => &sym.quals,
                None => match self.renames_quals.get(k) {
                    Some(p) => p,
                    None => continue,
                },
            };
            // Qualifiers are innermost-first; the ancestor path is
            // outermost-first, so match against the reversed path.
            let rev: Vec<&String> = path.iter().rev().collect();
            if is_subsequence(&qs, &rev) {
                return k.clone();
            }
        }
        cands[0].clone()
    }

    /// Recursively initialise a data declaration and its children.
    fn init_decl(
        &mut self,
        decl: &DataDecl,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        self.init_decl_h(
            decl,
            &mut Vec::new(),
            &mut Vec::new(),
            scope,
            inherited_global,
            origin,
        );
    }

    /// Hierarchy-aware initialisation: `dims` accumulates the OCCURS counts of
    /// this item + its ancestors; `quals` the ancestor group names.
    fn init_decl_h(
        &mut self,
        decl: &DataDecl,
        dims: &mut Vec<usize>,
        quals: &mut Vec<String>,
        scope: ItemScope,
        inherited_global: bool,
        origin: &str,
    ) {
        let occ = decl.occurs.as_ref().map(|o| o.max.max(1) as usize);
        if let Some(n) = occ {
            dims.push(n);
        }

        let upper = decl.name.as_ref().map(|n| n.to_ascii_uppercase());
        let is_named = matches!(&upper, Some(n) if n != "FILLER");
        // This item's own storage key once it is known, so the children loop
        // below can key its unnamed FILLERs under it.
        let mut owner_key: Option<String> = None;

        // 66-level RENAMES: register the regrouping over already-declared
        // elementary items; it has no storage of its own.
        if decl.level == 66 {
            if let (Some(name), Some(ren)) = (&upper, &decl.renames) {
                // The record this 66 belongs to. It is its own ancestor path,
                // and it is also the context the renamed operands resolve in:
                // `RENAMES TAG-1A THRU NAME-2` names items of *this* record,
                // and `NAME-2` alone is declared in two of them (NC252A).
                let path: Vec<String> = self.last_record.iter().cloned().collect();
                let from = self.canonical_name(&ren.from, &path);
                let thru = ren.thru.as_ref().map(|t| self.canonical_name(t, &path));
                if let Some(covered) = self.renames_range(&from, thru.as_deref()) {
                    // Key it the way a data item is keyed, and make it visible
                    // to `resolve_canonical`, so `RENAME-5 OF T-RENAMES-DATA`
                    // reaches this declaration and not the last one parsed.
                    let key = self.canon_key(name, &path);
                    self.renames.insert(key.clone(), covered);
                    self.renames_quals.insert(key.clone(), path);
                    let slot = self.by_leaf.entry(name.clone()).or_default();
                    if !slot.contains(&key) {
                        slot.push(key);
                    }
                }
            }
            if occ.is_some() {
                dims.pop();
            }
            return;
        }

        if is_named {
            let leaf = upper.clone().unwrap();
            // A record description begins here; every 66 that follows belongs
            // to it until the next one starts.
            if quals.is_empty() {
                self.last_record = Some(leaf.clone());
            }
            let is_global = inherited_global || decl.is_global;
            // Canonical storage key: the leaf itself when unique, otherwise a
            // path-qualified key that disambiguates duplicated names.
            let key = self.canon_key(&leaf, quals);
            // `REDEFINES` names a SIBLING, so the target shares this item's
            // ancestor path. Record the pair; the overlay is wired up once
            // every symbol exists (`build_redefine_links`).
            if let Some(target) = &decl.redefines {
                let tkey = self.canon_key(&target.to_ascii_uppercase(), quals);
                self.redefine_pairs.push((key.clone(), tkey));
                self.redefinitions.insert(key.clone());
            }
            // Register any 88-level condition-names qualifying this item. The
            // condition-name's qualification path is this item's path plus this
            // item's own name — an 88 is reached through the host and every
            // group above it.
            for c in &decl.children {
                if c.level == 88 {
                    if let Some(cn) = &c.name {
                        let mut path = quals.clone();
                        path.push(leaf.clone());
                        self.cond_names
                            .entry(cn.to_ascii_uppercase())
                            .or_default()
                            .push(CondName {
                                parent: key.clone(),
                                values: c.condition_values.clone(),
                                quals: path,
                            });
                    }
                }
            }
            // The `CORRESPONDING` list. A 66-level RENAMES is left out for the
            // same reason an 88 is: COBOL-85 6.18.4 GR1 excludes it, and it is
            // a regrouping of items the group already lists rather than an item
            // of its own. Included, `66 HARRY RENAMES HARRY-A THRU HARRY-B`
            // received the sender's `HARRY` and overwrote both (NC209A
            // MOV-TEST-F2-5). The exclusion belongs here, on the declaration,
            // and not on the name: a plain `HARRY` under some other group is a
            // perfectly ordinary corresponding item (MOV-TEST-F2-4).
            let children: Vec<String> = decl
                .children
                .iter()
                .filter(|c| c.level != 88 && c.level != 66)
                .filter_map(|c| c.name.as_ref())
                .map(|n| n.to_ascii_uppercase())
                .filter(|n| n != "FILLER")
                .collect();
            // Canonical keys of those children (their path = our path + leaf).
            let mut child_path = quals.clone();
            child_path.push(leaf.clone());
            let child_keys: Vec<String> = children
                .iter()
                .map(|c| self.canon_key(c, &child_path))
                .collect();
            // The group's LAYOUT: every subordinate item in declaration order,
            // **including FILLER**. This is deliberately not `child_keys`, which
            // is the CORRESPONDING list and must leave unnamed items out — they
            // have no name to correspond by. A group's *bytes*, though, are all
            // of its items: drop the FILLERs and `01 T. 05 HH PIC 99. 05 PIC X
            // VALUE ":". 05 MM PIC 99.` reads back as "1234" instead of "12:34".
            //
            // A REDEFINES item is the exception: it is a second description of
            // storage its target already owns, not storage of its own, so
            // counting it made the group twice as wide as it is —
            // `04 G. 05 D PIC 9(10). 05 R REDEFINES D PIC 9(6)V9999.` read back
            // as twenty digits instead of ten.
            let layout_keys: Vec<String> = decl
                .children
                .iter()
                .enumerate()
                .filter(|(_, c)| c.level != 88 && c.level != 66 && c.redefines.is_none())
                .map(|(i, c)| match c.name.as_deref() {
                    Some(n) if !n.eq_ignore_ascii_case("FILLER") => {
                        self.canon_key(&n.to_ascii_uppercase(), &child_path)
                    }
                    _ => filler_key(&key, i),
                })
                .collect();
            let index_names: Vec<String> = decl
                .occurs
                .as_ref()
                .map(|o| {
                    o.indexed_by
                        .iter()
                        .map(|n| n.to_ascii_uppercase())
                        .collect()
                })
                .unwrap_or_default();
            let keys: Vec<(String, bool)> = decl
                .occurs
                .as_ref()
                .map(|o| {
                    o.keys
                        .iter()
                        .map(|(n, asc)| (n.to_ascii_uppercase(), *asc))
                        .collect()
                })
                .unwrap_or_default();
            self.symbols.insert(
                key.clone(),
                ItemSym {
                    dims: dims.clone(),
                    children,
                    child_keys,
                    layout_keys,
                    quals: quals.clone(),
                    is_group: !decl.children.is_empty(),
                    index_names: index_names.clone(),
                    occurs: occ.unwrap_or(0),
                    keys,
                    scope: Some(scope),
                    is_global,
                    pic: decl
                        .picture
                        .as_ref()
                        .map(|pic| pic.template.clone())
                        .unwrap_or_default(),
                    pic_decimals: decl.picture.as_ref().map_or(0, |pic| pic.decimals),
                    origin: origin.to_owned(),
                    depending_on: decl
                        .occurs
                        .as_ref()
                        .and_then(|o| o.depending_on.as_ref())
                        .map(|n| n.to_ascii_uppercase()),
                },
            );
            self.by_leaf
                .entry(leaf.clone())
                .or_default()
                .push(key.clone());
            // Record elementary (leaf) items in declaration order for 66 RENAMES.
            // 88-level condition-names do not make an item a group, so one that
            // carries them is still a leaf and still belongs in the order.
            if !has_storage_children(decl) {
                self.elem_order.push(key.clone());
            }
            // Base/template slot + caps/edited (one value; subscript slots are
            // created lazily from this template on first write).
            self.insert_value(&key, decl);
            // Register INDEXED BY index registers as numeric items (default 1).
            for ix in &index_names {
                self.field_caps.insert(ix.clone(), (9, 0));
                self.store
                    .entry(ix.clone())
                    .or_insert_with(|| CobolValue::from_i64(1));
            }
            owner_key = Some(key);
            quals.push(leaf);
        }

        for (i, child) in decl.children.iter().enumerate() {
            if child.level == 88 {
                continue; // condition-names are not data items
            }
            // An unnamed / FILLER item occupies bytes in its parent and may
            // carry a VALUE, but it has no name, so `is_named` below would give
            // it no storage at all and the group would read back with its
            // separators missing. Give it a synthetic key under its parent —
            // `\u{2}` cannot occur in a COBOL name, so nothing in the program
            // can reach it — and store its value there.
            let unnamed = !matches!(child.name.as_deref(), Some(n) if !n.eq_ignore_ascii_case("FILLER"));
            // An unnamed GROUP that REDEFINES describes bytes its target
            // already owns, under no name of its own. `is_named` therefore gave
            // it no storage key, so nothing recorded the pair and the overlay
            // never refreshed — NC204M's
            //
            //     02  ACCEPT-TEST-14-DATA  PIC X(15).
            //     02  FILLER  REDEFINES  ACCEPT-TEST-14-DATA.
            //       03  ACC-14-CHARS-1-10  PIC X(10).
            //
            // read back as spaces however the item it redescribes had been
            // filled. Give it the same synthetic key an unnamed *leaf* gets and
            // register it as a group, so the ordinary overlay machinery can see
            // a description here at all. Its children keep the keys the
            // recursion below gives them: an unnamed group contributes nothing
            // to the qualification path, so nothing a program writes changes.
            if unnamed && child.redefines.is_some() && has_storage_children(child) {
                if let (Some(parent), Some(target)) = (owner_key.as_deref(), &child.redefines) {
                    let fk = filler_key(parent, i);
                    let overlay_keys: Vec<String> = child
                        .children
                        .iter()
                        .filter(|g| g.level != 88 && g.level != 66 && g.redefines.is_none())
                        .filter_map(|g| g.name.as_ref())
                        .filter(|n| !n.eq_ignore_ascii_case("FILLER"))
                        .map(|n| self.canon_key(&n.to_ascii_uppercase(), quals))
                        .collect();
                    if !overlay_keys.is_empty() {
                        self.symbols.insert(
                            fk.clone(),
                            ItemSym {
                                dims: dims.clone(),
                                children: Vec::new(),
                                child_keys: Vec::new(),
                                layout_keys: overlay_keys,
                                quals: quals.clone(),
                                is_group: true,
                                index_names: Vec::new(),
                                occurs: 0,
                                keys: Vec::new(),
                                scope: Some(scope),
                                is_global: inherited_global || decl.is_global,
                                pic: String::new(),
                                pic_decimals: 0,
                                origin: origin.to_owned(),
                                depending_on: None,
                            },
                        );
                        let tkey = self.canon_key(&target.to_ascii_uppercase(), quals);
                        self.redefine_pairs.push((fk, tkey));
                    }
                }
            }
            if unnamed && child.children.is_empty() {
                if let Some(parent) = owner_key.as_deref() {
                    let fk = filler_key(parent, i);
                    // A FILLER holds bytes like any other leaf, so a 66 RENAMES
                    // range that spans it has to carry it too. Leaving it out of
                    // the order closed the gap it occupies: `RENAMES
                    // SUB-GRP-FOR-RENAMES-1 THRU ELEM-FOR-RENAMES-2` read
                    // "X123" instead of "X  123" (NC252A RENAM-TEST-5/6).
                    self.elem_order.push(fk.clone());
                    self.insert_value(&fk, child);
                    continue;
                }
            }
            self.init_decl_h(
                child,
                dims,
                quals,
                scope,
                inherited_global || decl.is_global,
                origin,
            );
        }

        // A `VALUE` on a **group** is the group's own bytes, distributed across
        // its subordinate items exactly as a group MOVE distributes them:
        // `01 G VALUE "$123.45". 02 E PIC $999.99.` leaves `E` holding
        // `"$123.45"`, insertion characters and all. `insert_value` wrote the
        // literal into the group's own slot, which nothing reads back — a
        // group's value is synthesized from its children — so the child kept
        // its default and NC104A's MOVE-TEST-F1-29 moved an empty item. This
        // runs after the children loop because it needs their slots to measure
        // each one's share.
        if let (Some(k), Some(lit)) = (owner_key.clone(), &decl.value) {
            if let Some(width) = self.group_width(&k) {
                let default = CobolValue::spaces(width.max(1));
                let filled = apply_literal(lit, &default);
                // Take the bytes, not the display string: `VALUE HIGH-VALUES`
                // is `0xFF`, which has no one-byte UTF-8 spelling.
                let bytes = match &filled {
                    CobolValue::String { bytes, .. } => bytes.clone(),
                    other => other.as_display_string().into_bytes(),
                };
                self.set_group_bytes(&k, &bytes);
            }
        }

        if is_named {
            quals.pop();
        }
        if occ.is_some() {
            dims.pop();
        }
    }

    /// Insert one item's base value + caps / edited template.
    fn insert_value(&mut self, upper: &str, decl: &DataDecl) {
        if let Some(pic) = &decl.picture {
            if pic.kind == PicKind::NumericEdited {
                self.init_edited(
                    upper,
                    &pic.template,
                    decl.value.as_ref(),
                    decl.blank_when_zero,
                );
                return;
            }
        }
        // `P` scaling positions are digit positions the item spans but does not
        // store, so its capacity and its scale both count them while
        // `pic.digits`/`pic.decimals` (the stored positions, and therefore the
        // record layout's byte width) do not.
        let scaled = decl
            .picture
            .as_ref()
            .filter(|pic| pic.kind == PicKind::Numeric)
            .and_then(|pic| pic_scaling(&pic.template));
        let mut default = default_value(decl);
        if let (Some((_, decimals, _, _)), CobolValue::Numeric(n)) = (scaled, &mut default) {
            n.decimals = decimals.min(u8::MAX as u16) as u8;
        }
        let value = if let Some(lit) = &decl.value {
            apply_literal(lit, &default)
        } else {
            default
        };
        if let Some(pic) = &decl.picture {
            if pic.kind == PicKind::Numeric {
                let (digits, decimals) = match scaled {
                    Some((d, dec, _, _)) => (d, dec),
                    None => (pic.digits, pic.decimals),
                };
                self.field_caps.insert(
                    upper.to_string(),
                    (
                        digits.min(u8::MAX as u16) as u8,
                        decimals.min(u8::MAX as u16) as u8,
                    ),
                );
                if let Some((_, _, trailing_p, leading_p)) = scaled {
                    self.scaling_p
                        .insert(upper.to_string(), (trailing_p, leading_p));
                }
                // No `S` in the template means the item has no sign position
                // to store one in, so it holds the **absolute** value of
                // whatever is moved or computed into it.
                // BLANK WHEN ZERO is not confined to an *edited* picture: the
                // standard allows it on any numeric DISPLAY item, and
                // `PIC 9(10) BLANK WHEN ZERO` holding zero reads as ten spaces.
                // Registering it only for NumericEdited meant a plain picture
                // kept its digits, and NC107A's BZERO-TEST-1/2 only passed
                // because comparing those digits against spaces coerced both
                // sides to 0.0.
                if decl.blank_when_zero {
                    self.blank_when_zero.insert(upper.to_string());
                }
                if !pic.template.to_ascii_uppercase().contains('S') {
                    self.unsigned_numeric.insert(upper.to_string());
                } else if let Some(sign) = decl.sign.filter(|s| s.separate) {
                    // SEPARATE only means anything on a signed DISPLAY item:
                    // an unsigned PICTURE has no sign to separate, and COMP
                    // items are not stored as characters at all.
                    if matches!(decl.usage, Usage::Display) {
                        self.sign_separate.insert(upper.to_string(), sign.leading);
                    }
                }
            }
            // An alphanumeric-edited item owns its insertion characters, so the
            // template is kept and re-applied on every store.
            if pic.kind == PicKind::AlphanumericEdited {
                self.alnum_edited
                    .insert(upper.to_string(), pic.template.clone());
            }
            // JUSTIFIED applies only to an alphanumeric/alphabetic receiver;
            // the standard does not allow it on a numeric or edited one.
            //
            // `PICTURE A(5) JUSTIFIED RIGHT` is the alphabetic half, and it was
            // missing: only `Alphanumeric` was matched, so the clause parsed,
            // the item was never recorded, and every `MOVE` into it left-
            // aligned. NC107A's JUST-TEST-03 and JUST-TEST-04 move into
            // `AJ-00005 PICTURE A(5)` and want `"  ABC"` and the rightmost five
            // characters of a longer sender.
            if decl.justified && matches!(pic.kind, PicKind::Alphanumeric | PicKind::Alphabetic) {
                self.justified.insert(upper.to_string());
            }
        }
        self.store.insert(upper.to_string(), value);
        self.truncate_to_capacity(upper);
    }

    // ── Hierarchy / occurrence accessors ────────────────────────────────────

    /// OCCURS dimensions of a (table) item; empty for a non-table item.
    pub fn dims_of(&self, name: &str) -> Vec<usize> {
        self.symbols
            .get(&name.to_ascii_uppercase())
            .map(|s| s.dims.clone())
            .unwrap_or_default()
    }

    /// Immediate child item names of a group (for CORRESPONDING).
    pub fn children_of(&self, name: &str) -> Vec<String> {
        self.symbols
            .get(&name.to_ascii_uppercase())
            .map(|s| s.children.clone())
            .unwrap_or_default()
    }

    /// The symbol-table entry for an item, if declared.
    pub fn symbol(&self, name: &str) -> Option<&ItemSym> {
        self.symbols.get(&name.to_ascii_uppercase())
    }

    /// Snapshot all symbol metadata entries for a temporary local scope.
    pub fn symbol_entries(&self) -> Vec<(String, ItemSym)> {
        self.symbols
            .iter()
            .map(|(key, sym)| (key.clone(), sym.clone()))
            .collect()
    }

    /// The 88-level condition-name metadata for `name`, if it is one.
    pub fn cond_name(&self, name: &str) -> Option<&CondName> {
        self.cond_name_qual(name, &[])
    }

    /// Resolve a condition-name against its `OF`/`IN` qualifiers (innermost
    /// first), the way [`resolve_canonical`](Self::resolve_canonical) resolves a
    /// data name: the qualifiers must appear, in order, somewhere in the
    /// condition-name's own ancestor path — intermediate levels may be skipped.
    ///
    /// Without this a duplicated 88 name resolved to whichever declaration came
    /// last, so `EQUALS-A OF … OF GROUP-1-TABLE` tested `GROUP-3-TABLE`'s item
    /// and its subscript addressed an occurrence that table does not have.
    pub fn cond_name_qual(&self, name: &str, quals: &[String]) -> Option<&CondName> {
        let cands = self.cond_names.get(&name.to_ascii_uppercase())?;
        if cands.len() == 1 || quals.is_empty() {
            return cands.first();
        }
        let qs: Vec<String> = quals.iter().map(|q| q.to_ascii_uppercase()).collect();
        cands
            .iter()
            .find(|c| {
                // Qualifiers are innermost-first; the stored path is
                // outermost-first, so match against the reversed path.
                let rev: Vec<&String> = c.quals.iter().rev().collect();
                is_subsequence(&qs, &rev)
            })
            .or_else(|| cands.first())
    }

    // ── Pointers (USAGE POINTER / SET ADDRESS OF) ───────────────────────────────

    /// A stable non-zero address id for the storage key `key` (0 is reserved
    /// for NULL). Idempotent — the same key always yields the same id.
    pub fn addr_of(&mut self, key: &str) -> i64 {
        let key = key.to_ascii_uppercase();
        if let Some(i) = self.addr_table.iter().position(|k| k == &key) {
            return i as i64 + 1;
        }
        self.addr_table.push(key);
        self.addr_table.len() as i64
    }

    /// The storage key an address id points at (`None` for NULL / unknown).
    pub fn addr_target(&self, id: i64) -> Option<String> {
        if id < 1 {
            return None;
        }
        self.addr_table.get((id - 1) as usize).cloned()
    }

    /// `SET ADDRESS OF alias TO …` — make `alias` read/write `target`'s storage.
    pub fn set_alias(&mut self, alias: &str, target: &str) {
        self.addr_aliases
            .insert(alias.to_ascii_uppercase(), target.to_ascii_uppercase());
    }

    /// Remove an address alias (`SET ADDRESS OF alias TO NULL`).
    pub fn clear_alias(&mut self, alias: &str) {
        self.addr_aliases.shift_remove(&alias.to_ascii_uppercase());
    }

    // ── 66-level RENAMES ────────────────────────────────────────────────────────

    /// The covered elementary keys for `RENAMES from [THRU thru]` — the slice of
    /// `elem_order` spanning `from`'s first leaf to `thru`'s (or `from`'s) last.
    fn renames_range(&self, from: &str, thru: Option<&str>) -> Option<Vec<String>> {
        let (lo, _) = self.leaf_span(from)?;
        let (_, hi) = self.leaf_span(thru.unwrap_or(from))?;
        if hi >= lo {
            Some(
                self.elem_order[lo..=hi]
                    .iter()
                    .flat_map(|k| self.table_element_keys(k))
                    .collect(),
            )
        } else {
            None
        }
    }

    /// The storage keys a covered elementary item contributes: itself, or one
    /// key per occurrence when it is a table.
    ///
    /// `elem_order` holds one entry per *declaration*, so an `OCCURS` item
    /// appears once however many occurrences it has. A RENAMES over a group
    /// containing one read only the base slot — `RENAME-7 RENAMES ITEM-1 THRU
    /// TABLE-2` came back as `"BOSTO"` instead of `"BOSTON MASSACHUSETTS"`
    /// (NC252A RENAM-TEST-11).
    fn table_element_keys(&self, key: &str) -> Vec<String> {
        let dims = match self.symbols.get(key) {
            Some(sym) if !sym.dims.is_empty() => sym.dims.clone(),
            _ => return vec![key.to_string()],
        };
        let total: usize = dims.iter().product();
        (0..total)
            .map(|n| {
                let mut rem = n;
                let mut idx = vec![0i64; dims.len()];
                for d in (0..dims.len()).rev() {
                    idx[d] = (rem % dims[d]) as i64 + 1;
                    rem /= dims[d];
                }
                subscript_key(key, &idx)
            })
            .collect()
    }

    /// The `[first, last]` `elem_order` index span of an item: itself if
    /// elementary, otherwise the range over all its descendant leaves.
    fn leaf_span(&self, key: &str) -> Option<(usize, usize)> {
        if let Some(p) = self.elem_order.iter().position(|k| k == key) {
            return Some((p, p));
        }
        // A group: collect its descendant leaf positions via the child tree.
        let mut positions = Vec::new();
        self.collect_leaf_positions(key, &mut positions);
        let lo = *positions.iter().min()?;
        let hi = *positions.iter().max()?;
        Some((lo, hi))
    }

    fn collect_leaf_positions(&self, key: &str, out: &mut Vec<usize>) {
        if let Some(p) = self.elem_order.iter().position(|k| k == key) {
            out.push(p);
            return;
        }
        if let Some(sym) = self.symbols.get(key) {
            for ck in &sym.child_keys {
                self.collect_leaf_positions(ck, out);
            }
        }
    }

    /// `true` if `name` is a 66-level RENAMES item.
    pub fn is_renames(&self, name: &str) -> bool {
        self.renames.contains_key(&name.to_ascii_uppercase())
    }

    /// The single item a 66-level RENAMES regroups, when it regroups exactly
    /// one.
    ///
    /// COBOL-85 gives such an item the *description* of the item it renames —
    /// `66 RENAME-12 RENAMES WIDGET-4` where `WIDGET-4 PIC 9(4)` is a numeric
    /// item four digits wide, not a group. Arithmetic on it therefore has a
    /// capacity to overflow and a slot to land in; without this the receiver
    /// was the RENAMES' own (unread) key, so `ADD 3500 TO RENAME-12` neither
    /// raised `ON SIZE ERROR` nor changed anything (NC252A RENAM-TEST-16).
    pub fn renames_alias(&self, name: &str) -> Option<String> {
        let covered = self.renames.get(&name.to_ascii_uppercase())?;
        match covered.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        }
    }

    /// `true` if the item was declared **with** a `REDEFINES` clause — a second
    /// description of storage another item already owns.
    ///
    /// COBOL-85 6.18.4 GR1 leaves such an item (and everything subordinate to
    /// it) out of `CORRESPONDING`: `04 DD-LEVEL REDEFINES DD-LEVEL-FALSE. 05
    /// HARRY PIC X(5).` must not receive the sender's `HARRY` (NC209A
    /// MOV-TEST-F2-6).
    pub fn is_redefinition(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        self.redefinitions.contains(&key) || self.redefinitions.contains(base_name(&key))
    }

    // ── Group items ───────────────────────────────────────────────────────────
    //
    // A group item is **not** a slot of its own that happens to sit above other
    // slots: in COBOL-85 it *is* its subordinate items, laid end to end, and it
    // is alphanumeric whatever its children are. Its size is the sum of theirs.
    //
    // Before this, a group had an independent slot that nothing kept in step
    // with the children, so `DISPLAY <group>` printed whatever had been moved
    // to the group itself — usually nothing — while the children held the real
    // data, and `MOVE … TO <group>` left every child untouched.
    //
    // Reads synthesize and writes distribute, exactly as 66 RENAMES already
    // did; the group's own slot is no longer consulted for either.

    /// `true` if `name` is a group item — one with subordinate **data** items.
    ///
    /// Not [`ItemSym::is_group`], which is `!decl.children.is_empty()` and so is
    /// also true for an elementary item carrying 88-level condition-names. Those
    /// are not data items and hold no bytes: `01 WS-GRADE PIC 9(3). 88 PASSING
    /// VALUE 60 THRU 100.` is an ordinary numeric field, and treating it as a
    /// group made it read back as the empty concatenation of no children.
    pub fn is_group(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        self.symbols
            .get(base_name(&key))
            .map(|s| s.is_group && !s.layout_keys.is_empty())
            .unwrap_or(false)
    }

    /// The storage keys that make up **one occurrence** of a group, in layout
    /// order. `prefix` is the subscript the group itself was referenced with.
    ///
    /// A subordinate item that adds its own OCCURS contributes one key per
    /// occurrence of the dimensions the prefix does not already fix, in
    /// row-major order (the last dimension varies fastest) — so `GRP-1 (2)` of
    /// `02 GRP-1 OCCURS 6. 03 ELEM1 PIC XXX OCCURS 4.` is
    /// `ELEM1 (2,1) … ELEM1 (2,4)`, not the bare `ELEM1` template slot.
    /// With an empty prefix this walks a whole table, which is what makes the
    /// enclosing `01` record read and write as the flat bytes of every
    /// occurrence.
    /// The **current** number of occurrences of an `OCCURS … DEPENDING ON`
    /// table, read from its depending item. `None` when `key` is not one, which
    /// is every fixed table and every ordinary item.
    ///
    /// Clamped to the declared maximum: the depending item is ordinary storage
    /// and can hold a larger number than the OCCURS reserved room for, but the
    /// table cannot be longer than itself.
    fn odo_count(&self, key: &str) -> Option<usize> {
        // A receiving group takes the declared maximum, so the table does not
        // shrink at all — `None` leaves the caller on the declared dimensions.
        if self.odo_receiving > 0 {
            return None;
        }
        let upper = key.to_ascii_uppercase();
        let sym = self.symbols.get(base_name(&upper))?;
        let dep = sym.depending_on.as_ref()?;
        let n = self
            .get(&self.resolve_name(dep, &[]))
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0) as usize;
        Some(n.min(sym.occurs))
    }

    /// How many occurrences a table has **right now**: its
    /// `OCCURS … DEPENDING ON` count when it has one, its declared maximum
    /// otherwise. `None` when `name` is not a table.
    ///
    /// This is the bound `SEARCH` and `SEARCH ALL` walk. Searching the declared
    /// maximum reached entries past the end of the active table, so a value
    /// sitting in a dormant occurrence was still found (NC247A SCH-TEST-F1-2
    /// and SCH-TEST-4, which both require `AT END`).
    pub fn table_occurrences(&self, name: &str) -> Option<usize> {
        let upper = name.to_ascii_uppercase();
        let sym = self.symbols.get(base_name(&upper))?;
        let declared = if sym.occurs > 0 {
            sym.occurs
        } else {
            sym.dims.last().copied().unwrap_or(0)
        };
        Some(self.odo_count(&upper).unwrap_or(declared).min(declared))
    }

    fn occurrence_keys(&self, sym: &ItemSym, prefix: &[i64]) -> Vec<String> {
        let mut out = Vec::new();
        for ck in &sym.layout_keys {
            // A synthetic FILLER slot has no symbol entry; it shares its
            // parent's dimensions and so takes the prefix unchanged.
            let dims: &[usize] = match self.symbols.get(ck) {
                Some(cs) if cs.dims.len() > prefix.len() => &cs.dims[prefix.len()..],
                _ => &[],
            };
            if dims.is_empty() {
                out.push(subscript_key(ck, prefix));
                continue;
            }
            // An `OCCURS … DEPENDING ON` table contributes only the occurrences
            // that are **active now**, not its declared maximum: the enclosing
            // group's length tracks the depending item. Walking the maximum
            // made a partial ODO group read, compare, STRING and INSPECT at
            // full width — `GRP-ODO` with 3 active entries still delivered all
            // 9 (NC247A). The table's own dimension is the last one; any
            // ancestor dimensions the prefix left open are unaffected.
            //
            // Only the ODO case allocates; a fixed table keeps the borrow.
            let shrunk;
            let dims: &[usize] = match self.odo_count(ck) {
                Some(n) if n < dims[dims.len() - 1] => {
                    let mut d = dims.to_vec();
                    let last = d.len() - 1;
                    d[last] = n;
                    shrunk = d;
                    &shrunk
                }
                _ => dims,
            };
            let total: usize = dims.iter().product();
            for n in 0..total {
                let mut rem = n;
                let mut idx = prefix.to_vec();
                let mut tail = vec![0i64; dims.len()];
                for d in (0..dims.len()).rev() {
                    tail[d] = (rem % dims[d]) as i64 + 1;
                    rem /= dims[d];
                }
                idx.extend_from_slice(&tail);
                out.push(subscript_key(ck, &idx));
            }
        }
        out
    }

    /// The synthesized value of a group: its subordinate items' display strings
    /// concatenated in declaration order. `None` when `name` is not a group.
    ///
    /// Accepts a subscripted key (`GRP-1(2)`), which reads that one occurrence
    /// of the group. Nested groups fold in through [`display_string`], which
    /// routes a group back here — so a group of groups reads as the whole
    /// flattened record.
    pub fn group_value(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let sym = self.symbols.get(base_name(&key))?;
        // `layout_keys` empty ⇒ no subordinate data items ⇒ elementary, whatever
        // `is_group` says (88-level condition-names count as children there).
        if !sym.is_group || sym.layout_keys.is_empty() {
            return None;
        }
        let mut out = String::new();
        for ck in self.occurrence_keys(sym, &key_indices(&key)) {
            out.push_str(&self.display_string(&ck).unwrap_or_default());
        }
        Some(out)
    }

    /// A group's record as **bytes** — [`Self::group_value`] without the UTF-8
    /// round trip.
    ///
    /// `HIGH-VALUE` is the byte `0xFF`, which is not valid UTF-8, so reading a
    /// record through a `String` replaced each such byte with U+FFFD — three
    /// bytes where the item stores one. Every field after it then shifted by
    /// two, and NC211A's FIG-TEST-3 read its `LOW-VALUES` item out of the
    /// middle of the mangled `HIGH-VALUES` one that preceded it.
    pub fn group_bytes(&self, name: &str) -> Option<Vec<u8>> {
        let key = name.to_ascii_uppercase();
        let sym = self.symbols.get(base_name(&key))?;
        if !sym.is_group || sym.layout_keys.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for ck in self.occurrence_keys(sym, &key_indices(&key)) {
            out.extend_from_slice(&self.display_bytes(&ck).unwrap_or_default());
        }
        Some(out)
    }

    /// One item's stored form as bytes: the raw bytes of an alphanumeric item,
    /// and the rendered characters of anything else.
    ///
    /// Only an alphanumeric slot can hold a byte that is not a character, so
    /// every other category is identical to [`Self::display_string`] and is
    /// taken from it rather than duplicated.
    pub fn display_bytes(&self, name: &str) -> Option<Vec<u8>> {
        let key = name.to_ascii_uppercase();
        if self.renames.contains_key(&key) {
            return self.renames_value(&key).map(String::into_bytes);
        }
        if let Some(g) = self.group_bytes(&key) {
            return Some(g);
        }
        match self.get(&key) {
            Some(CobolValue::String { bytes, .. }) => Some(bytes.clone()),
            _ => self.display_string(&key).map(String::into_bytes),
        }
    }

    /// The stored width of one item in bytes — its rendered, fixed-width form.
    /// For a group that is the sum of its children, by the same recursion.
    ///
    /// A group's width is summed from its parts rather than measured on the
    /// concatenation: building the string of a large table just to take its
    /// length allocated the whole record on every child of every group move.
    fn item_width(&self, key: &str) -> usize {
        if let Some(sym) = self.symbols.get(base_name(&key.to_ascii_uppercase())) {
            if sym.is_group && !sym.layout_keys.is_empty() {
                return self
                    .occurrence_keys(sym, &key_indices(&key.to_ascii_uppercase()))
                    .iter()
                    .map(|k| self.item_width(k))
                    .sum();
            }
        }
        self.display_string(key).map(|d| d.len()).unwrap_or(0)
    }

    /// The stored width of any item, in characters — a group's whole record or
    /// an elementary item's own field.
    ///
    /// This is the size `UNSTRING … INTO a b c` (with no `DELIMITED BY`) hands
    /// each receiver in turn: without a delimiter the standard splits the
    /// source by the receivers' own sizes.
    pub fn stored_width(&self, name: &str) -> usize {
        self.item_width(&name.to_ascii_uppercase())
    }

    /// The total stored width of a **group**, in bytes: every subordinate item
    /// of every occurrence, summed.
    ///
    /// `None` for an elementary item, whose fill width is
    /// [`Self::alphanumeric_capacity`]. A group needs its own accessor because
    /// `MOVE ALL literal TO <group>` fills the whole record — without one the
    /// literal was written once and the rest of the record stayed as it was, so
    /// `MOVE ALL "ABC…Z" TO <7-dimension table>` filled only the first
    /// occurrence of the outermost OCCURS (NC242A/NC243A).
    pub fn group_width(&self, name: &str) -> Option<usize> {
        let key = name.to_ascii_uppercase();
        let sym = self.symbols.get(base_name(&key))?;
        if !sym.is_group || sym.layout_keys.is_empty() {
            return None;
        }
        Some(self.item_width(&key))
    }

    /// Distribute `s` across a group's subordinate items by each one's stored
    /// width, left to right — a group move is alphanumeric, so the bytes land
    /// where they fall and each child is padded to its own width.
    ///
    /// Recurses into subordinate groups so a nested record fills correctly, and
    /// accepts a subscripted key (`GRP-1(2)`) so a group move into one
    /// occurrence of a table lands in that occurrence's own child slots.
    pub fn set_group(&mut self, name: &str, s: &str) {
        self.set_group_bytes(name, s.as_bytes());
    }

    /// [`Self::set_group`] over raw bytes, for a record that may hold a byte
    /// which is not a character — `HIGH-VALUE` is `0xFF` and has no UTF-8
    /// spelling of its own width. See [`Self::group_bytes`].
    pub fn set_group_bytes(&mut self, name: &str, bytes: &[u8]) {
        let key = name.to_ascii_uppercase();
        let Some(sym) = self.symbols.get(base_name(&key)) else {
            return;
        };
        if !sym.is_group || sym.layout_keys.is_empty() {
            return; // elementary (see `is_group` on 88-level children)
        }
        // Everything from here measures a **receiving** group, which takes each
        // ODO table's declared maximum rather than its current length. The
        // symbol is looked up again because raising the flag needs `self`.
        self.odo_receiving += 1;
        let child_keys = match self.symbols.get(base_name(&key)) {
            Some(sym) => self.occurrence_keys(sym, &key_indices(&key)),
            None => Vec::new(),
        };
        let mut pos = 0usize;
        for ck in child_keys {
            let width = self.item_width(&ck).max(1);
            let end = (pos + width).min(bytes.len());
            // Slice, then pad on the right to the child's own width — on the
            // bytes, so a `0xFF` lands as the one byte it is.
            let mut padded = if pos < bytes.len() {
                bytes[pos..end].to_vec()
            } else {
                Vec::new()
            };
            padded.resize(width, b' ');
            if self.is_group(&ck) {
                self.set_group_bytes(&ck, &padded);
            } else {
                // A group move carries **bytes**, not values: the sending item
                // is a group, so the standard makes the whole move alphanumeric
                // and each slice lands in its child exactly as it stands,
                // whatever that child's PICTURE says. `MOVE SPACE TO <group of
                // PIC 99 children>` has to leave the group reading as spaces,
                // and `MOVE <120 bytes of "A"> TO <group with PIC 9(5)
                // children>` has to leave it reading as `A`s (NC252A
                // RDF-TEST-11). A numeric item holding characters is exactly
                // what the program asked for; using it in arithmetic afterwards
                // is undefined in COBOL-85, not our problem to prevent.
                //
                // The same holds for an **alphanumeric-edited** child, which
                // used to be routed through `set_bytes` and had its insertion
                // characters re-imposed: `MOVE "1 A05" TO <group whose only
                // child is PIC XBA09>` stored `"1  0A"`, the edit re-applied to
                // an already-edited slice (NC105A MOVE-TEST-F1-13).
                self.set_verbatim_bytes(&ck, &padded);
            }
            pos += width;
        }
        self.odo_receiving -= 1;
    }

    /// The synthesized value of a RENAMES item: the concatenated display strings
    /// of the items it covers.
    pub fn renames_value(&self, name: &str) -> Option<String> {
        let covered = self.renames.get(&name.to_ascii_uppercase())?;
        let mut s = String::new();
        for k in covered {
            s.push_str(&self.display_string(k).unwrap_or_default());
        }
        Some(s)
    }

    /// The total stored width of a 66-level RENAMES item, in characters.
    ///
    /// `None` when `name` is not a RENAMES item. `MOVE ALL "X" TO <renames>`
    /// needs this for the same reason a group does: the fill has to reach every
    /// covered byte, not just the first (NC252A RENAM-TEST-3).
    pub fn renames_width(&self, name: &str) -> Option<usize> {
        let covered = self.renames.get(&name.to_ascii_uppercase())?;
        Some(
            covered
                .iter()
                .map(|k| self.display_string(k).map(|d| d.len()).unwrap_or(0))
                .sum(),
        )
    }

    /// Distribute `s` across the covered items of a RENAMES item by each item's
    /// stored width (left-to-right).
    pub fn set_renames(&mut self, name: &str, s: &str) {
        let Some(covered) = self.renames.get(&name.to_ascii_uppercase()).cloned() else {
            return;
        };
        let bytes = s.as_bytes();
        let mut pos = 0usize;
        for k in covered {
            let width = self.display_string(&k).map(|d| d.len()).unwrap_or(0).max(1);
            let end = (pos + width).min(bytes.len());
            let chunk = if pos < bytes.len() {
                String::from_utf8_lossy(&bytes[pos..end]).into_owned()
            } else {
                String::new()
            };
            // Pad the chunk to the field width.
            let padded = format!("{chunk:<width$}");
            if self.field_caps.contains_key(&k) {
                // numeric receiver — store the digits
                if let Ok(n) = padded.trim().parse::<i128>() {
                    self.set(&k, CobolValue::from_i64(n as i64));
                }
            } else {
                self.set_str(&k, &padded);
            }
            pos += width;
        }
    }

    /// Initialise a numeric-edited item: remember its template and store the
    /// edited string form of any VALUE (or spaces when there is none).
    fn init_edited(
        &mut self,
        name: &str,
        template: &str,
        value: Option<&Literal>,
        blank_when_zero: bool,
    ) {
        let dc = self.decimal_comma;
        let cur = self.currency();
        let width = crate::numedit::edited_width(template, dc);
        if blank_when_zero {
            self.blank_when_zero.insert(name.to_string());
        }
        let v = match value {
            Some(Literal::String(s)) => CobolValue::from_str(s, width),
            // The PICTURE decides the width of an edited item, so only the
            // literal's value matters here — leading zeros in the source change
            // nothing a template does not already say.
            Some(Literal::Integer(n) | Literal::IntegerDigits(n, _)) => CobolValue::from_str(
                &crate::numedit::format_edited(template, *n as i128, 0, dc, cur),
                width,
            ),
            Some(Literal::Decimal(m, s)) => CobolValue::from_str(
                &crate::numedit::format_edited(template, *m, *s, dc, cur),
                width,
            ),
            _ => CobolValue::spaces(width),
        };
        self.edited_templates
            .insert(name.to_string(), template.to_string());
        self.store.insert(name.to_string(), v);
    }

    // ── Data access ───────────────────────────────────────────────────────────

    /// Get an immutable reference to a data item's value. An un-written table
    /// occurrence falls back to the base item's (template) value.
    pub fn get(&self, name: &str) -> Option<&CobolValue> {
        let key = self.storage_key(name.to_ascii_uppercase());
        if let Some(v) = self.store.get(&key) {
            return Some(v);
        }
        if key.contains('(') {
            return self.store.get(base_name(&key));
        }
        None
    }

    /// Integer-digit capacity of a numeric field, if known (for ON SIZE ERROR).
    pub fn integer_capacity(&self, name: &str) -> Option<u8> {
        let key = name.to_ascii_uppercase();
        if let Some((d, _)) = self
            .field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))
        {
            return Some(*d);
        }
        // A numeric-edited item keeps no `field_caps` entry — it holds its
        // edited characters — so its capacity comes from the template's digit
        // positions before the decimal point, exactly as its scale does. With
        // no capacity the size error condition could never arise for one, and
        // `MULTIPLY 999999 BY 999999 GIVING <PIC $**.99> ON SIZE ERROR …`
        // truncated the product into the field instead of leaving it alone.
        let template = self.edited_templates.get(base_name(&key))?;
        let (int, _) = crate::numedit::digit_counts(template, self.decimal_comma);
        Some(int.min(u8::MAX as usize) as u8)
    }

    /// Declared decimal places of a numeric field, if known.
    ///
    /// Read from the PICTURE rather than from the value, because a
    /// **numeric-edited** item holds its edited character form: asking the
    /// stored value for its scale returned nothing, so `ROUNDED` silently did
    /// not round into one — `MULTIPLY .9 BY 80.12 GIVING <$$$$.99> ROUNDED`
    /// edited the truncated 72.10 instead of 72.11.
    pub fn decimal_places(&self, name: &str) -> Option<u8> {
        let key = name.to_ascii_uppercase();
        if let Some((_, d)) = self
            .field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))
        {
            return Some(*d);
        }
        // A numeric-edited item keeps no entry in `field_caps` — its scale is
        // the count of digit positions after the template's decimal point.
        let template = self.edited_templates.get(base_name(&key))?;
        let (_, dec) = crate::numedit::digit_counts(template, self.decimal_comma);
        Some(dec.min(u8::MAX as usize) as u8)
    }

    /// How many **trailing `P`** positions the item's PICTURE carries.
    ///
    /// Those positions are not stored but they *are* digit positions, so the
    /// item's least significant digit sits that many powers of ten above the
    /// units — which is the position `ROUNDED` has to round to. 0 for every
    /// item without trailing `P`s, which is nearly all of them.
    pub fn trailing_scale_positions(&self, name: &str) -> u32 {
        let key = name.to_ascii_uppercase();
        self.scaling_p
            .get(&key)
            .or_else(|| self.scaling_p.get(base_name(&key)))
            .map(|(trailing, _)| *trailing)
            .unwrap_or(0)
    }

    /// The de-edited character form of a plain numeric field for a MOVE to an
    /// alphanumeric receiver: absolute zero-padded digits, no sign, no point.
    /// `None` if the item isn't a plain numeric.
    pub fn deedited_digits(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        let (int_digits, _) = *self
            .field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))?;
        if let Some(CobolValue::Numeric(n)) = self.get(&key) {
            let total = int_digits as usize + n.decimals as usize;
            let digits = n.mantissa.unsigned_abs().to_string();
            let padded = if digits.len() < total {
                format!("{}{}", "0".repeat(total - digits.len()), digits)
            } else {
                digits
            };
            Some(padded)
        } else {
            None
        }
    }

    /// The value a numeric-**edited** item's characters spell out, or `None`
    /// when the item is not numeric-edited. See [`crate::numedit::deedit`].
    pub fn deedited_value(&self, name: &str) -> Option<CobolNumeric> {
        let key = name.to_ascii_uppercase();
        self.edited_templates.get(base_name(&key))?;
        let chars = self.display_string(&key)?;
        let (mantissa, decimals) = crate::numedit::deedit(&chars, self.decimal_comma);
        Some(CobolNumeric::new(mantissa, decimals))
    }

    /// `true` if the named item is a plain alphanumeric field (not numeric-edited).
    /// Digits after the implied decimal point in this item's PICTURE, or 0 for
    /// an item that has none. See [`ItemSym::pic_decimals`].
    pub fn field_decimals(&self, name: &str) -> u16 {
        self.symbols
            .get(base_name(&name.to_ascii_uppercase()))
            .map_or(0, |s| s.pic_decimals)
    }

    pub fn is_alphanumeric_field(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        if self.edited_templates.contains_key(base_name(&key)) {
            return false;
        }
        // A **declared** numeric item is never alphanumeric, whatever its slot
        // happens to be holding at the time.
        //
        // Asking the slot used to be a good enough proxy, because only an
        // alphanumeric item could hold characters. A group `MOVE` is an
        // alphanumeric move, so a `PIC 9(3)` child can legitimately be holding
        // `"000"` or `"AAA"` — and then the slot answers with what the last
        // statement put there rather than with what the item *is*. `MOVE 7 TO
        // DNAME-2` after `MOVE ZERO TO D-NAMES` wrote the characters `"7  "`,
        // left-justified, into a numeric item (NC112A MOVE-TEST-F1-1-2).
        if self.field_caps.contains_key(&key) || self.field_caps.contains_key(base_name(&key)) {
            return false;
        }
        // A **group** is category alphanumeric whatever its children are, and it
        // owns no slot of its own to answer with — so ask the declaration. A
        // group paired with a numeric operand takes the nonnumeric comparison,
        // in which the numeric side becomes its characters padded on the right:
        // `PIC 9(5) VALUE 12345` against a ten-byte group holding `0000012345`
        // is `"12345     "`, and unequal (NC250A IF--TEST-77).
        if self.is_group(&key) {
            return true;
        }
        matches!(self.get(&key), Some(CobolValue::String { .. }))
    }

    /// The declared `PIC X(n)` width of an alphanumeric item, if it is one.
    ///
    /// `None` for a numeric, numeric-edited or undeclared item — the callers
    /// that need a fill width (`MOVE ALL literal`) must not invent one.
    pub fn alphanumeric_capacity(&self, name: &str) -> Option<usize> {
        let key = name.to_ascii_uppercase();
        if self.edited_templates.contains_key(base_name(&key)) {
            return None;
        }
        // The same slot-as-declaration proxy [`Self::is_alphanumeric_field`]
        // gave up in 1.62.34, and for the same reason: after a group `MOVE`
        // leaves characters in a `PIC 9` child, the slot answers with what the
        // last statement put there rather than with what the item *is*.
        //
        // `MOVE ZERO TO DNAME-1` then took the `MOVE ALL`-style fill path and
        // wrote `"0"` repeated to the **byte slot's** width, so a `PICTURE 9`
        // item read back as `000` (NC112A MOVE-TEST-F1-2-*). It compared equal
        // to `0` anyway, because the old cross-type comparison coerced both
        // sides to numbers — an accidental pass over a genuinely wrong item.
        if self.field_caps.contains_key(&key) || self.field_caps.contains_key(base_name(&key)) {
            return None;
        }
        match self.get(&key) {
            Some(CobolValue::String { capacity, .. }) => Some(*capacity),
            _ => None,
        }
    }

    /// Store `s` left-justified (space-padded) into an alphanumeric field.
    pub fn set_str_left(&mut self, name: &str, s: &str) {
        let key = self.storage_key(name.to_ascii_uppercase());
        // An alphanumeric-edited receiver still imposes its template on the
        // characters: this shortcut would otherwise drop the insertions and
        // store the sender's digits raw.
        if self.alnum_edited.contains_key(base_name(&key)) {
            self.set(&key, CobolValue::from_str(s, s.len()));
            return;
        }
        let cap = match self.get(&key) {
            Some(CobolValue::String { capacity, .. }) => *capacity,
            _ => s.len(),
        };
        self.store.insert(key.clone(), CobolValue::from_str(s, cap));
        self.refresh_redefine_peers(&key);
    }

    /// `true` when `name` is declared `BLANK WHEN ZERO`.
    ///
    /// Such an item holding zero *is* spaces — that is its character form — but
    /// the value stays numeric so arithmetic on it is unaffected. A comparison
    /// against an alphanumeric operand has to read the character form, and the
    /// comparison itself sees only values, so the caller asks here.
    pub fn is_blank_when_zero(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        self.blank_when_zero.contains(&key) || self.blank_when_zero.contains(base_name(&key))
    }

    /// `true` when `name` is a numeric DISPLAY item whose operational sign is
    /// **overpunched** — folded into a digit position rather than occupying a
    /// character position of its own.
    ///
    /// That is every signed numeric item except one declared `SIGN IS …
    /// SEPARATE CHARACTER`. It matters to anything that reads an item's
    /// *character positions*: `INSPECT <PIC S9(5) holding -12345> TALLYING …
    /// FOR ALL "-"` must be 0, because no position holds a minus sign (NC216A
    /// INS-TEST-F1-23). A numeric-**edited** item is not one of these — it
    /// stores its edited characters, sign and all, and keeps no `field_caps`
    /// entry.
    pub fn sign_is_overpunched(&self, name: &str) -> bool {
        let key = name.to_ascii_uppercase();
        let numeric =
            self.field_caps.contains_key(&key) || self.field_caps.contains_key(base_name(&key));
        numeric && self.separate_sign_of(&key).is_none()
    }

    /// `Some(leading?)` when `key` is a `SIGN IS … SEPARATE CHARACTER` item.
    fn separate_sign_of(&self, key: &str) -> Option<bool> {
        self.sign_separate
            .get(key)
            .or_else(|| self.sign_separate.get(base_name(key)))
            .copied()
    }

    /// Put the operational sign of a `SIGN … SEPARATE CHARACTER` item into its
    /// own character position.
    ///
    /// [`format_display_numeric`] renders the embedded form: bare digits, with a
    /// `-` glued on only when the value is negative. A separate sign is a
    /// declared *storage* position that is always occupied, so the digits are
    /// stripped of any sign and an explicit `+` or `-` is placed at the front
    /// (LEADING) or the back (TRAILING). `MOVE 15759 TO <S9(5) SIGN LEADING
    /// SEPARATE>` therefore stores `+15759`, six characters, not `15759`
    /// (NC116A SIG-TEST-GF-1 / GF-15 / GF-16).
    /// Finish an unedited numeric item's character form: place a separate sign
    /// if it has one, then blank the whole field if it is `BLANK WHEN ZERO` and
    /// holds zero. Blanking comes last so the sign position is blanked too —
    /// the clause blanks the *item*, not just its digits.
    fn finish_numeric_display(&self, key: &str, n: &CobolNumeric, rendered: String) -> String {
        let out = self.apply_separate_sign(key, n, rendered);
        if n.mantissa == 0 && self.blank_when_zero.contains(base_name(key)) {
            return " ".repeat(out.chars().count());
        }
        out
    }

    fn apply_separate_sign(&self, key: &str, n: &CobolNumeric, rendered: String) -> String {
        let Some(leading) = self.separate_sign_of(key) else {
            return rendered;
        };
        let digits = rendered.strip_prefix('-').unwrap_or(&rendered);
        let sign = if n.mantissa < 0 { '-' } else { '+' };
        if leading {
            format!("{sign}{digits}")
        } else {
            format!("{digits}{sign}")
        }
    }

    /// Render a data item for `DISPLAY`. A USAGE-DISPLAY numeric item is shown as
    /// its full fixed-width digit string — leading zeros to the PIC width, the
    /// implied decimal point (`V`) not shown, and a leading `-` for negatives —
    /// i.e. the characters as they are stored. Non-numeric items render verbatim.
    pub fn display_string(&self, name: &str) -> Option<String> {
        let key = name.to_ascii_uppercase();
        if self.renames.contains_key(&key) {
            return self.renames_value(&key);
        }
        // A group IS its children (see the group-item section above), so it is
        // read from them and never from its own slot.
        if let Some(g) = self.group_value(&key) {
            return Some(g);
        }
        let val = self.get(&key)?;
        if let CobolValue::Numeric(n) = val {
            if let Some(&(int_digits, _)) = self
                .field_caps
                .get(&key)
                .or_else(|| self.field_caps.get(base_name(&key)))
            {
                // `P` positions are digit positions the item does **not**
                // store, so they take no character here — `PIC 9P` is one byte
                // wide holding tens, not two. `field_caps` counts them because
                // it describes the item's *value* range; the character form
                // has to take them back out, or every group containing such an
                // item reads one byte too wide per `P` and every field after it
                // shifts (NC253A's SUBTRACT CORRESPONDING group compare).
                let (trailing_p, leading_p) = self
                    .scaling_p
                    .get(&key)
                    .or_else(|| self.scaling_p.get(base_name(&key)))
                    .copied()
                    .unwrap_or((0, 0));
                if trailing_p > 0 || leading_p > 0 {
                    let stored_int = (int_digits as u32).saturating_sub(trailing_p);
                    let stored_dec = (n.decimals as u32).saturating_sub(leading_p);
                    let scaled = CobolNumeric::new(
                        n.mantissa / 10i128.pow(trailing_p.min(38)),
                        stored_dec.min(u8::MAX as u32) as u8,
                    );
                    return Some(self.finish_numeric_display(
                        &key,
                        &scaled,
                        format_display_numeric(&scaled, stored_int.min(u8::MAX as u32) as u8),
                    ));
                }
                return Some(self.finish_numeric_display(
                    &key,
                    n,
                    format_display_numeric(n, int_digits),
                ));
            }
        }
        Some(val.as_display_string())
    }

    /// Get a mutable reference to a data item's value.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut CobolValue> {
        let key = self.storage_key(name.to_ascii_uppercase());
        self.store.get_mut(&key)
    }

    /// Set a data item to a new value.
    ///
    /// If the item exists the new value is assigned via `CobolValue::assign`
    /// so that type coercions (rescaling, padding) are applied.
    /// If the item does not exist it is inserted directly.
    pub fn set(&mut self, name: &str, value: CobolValue) {
        let key = self.storage_key(name.to_ascii_uppercase());
        // An alphanumeric-edited receiver re-imposes its own insertion
        // characters on whatever arrives: the sender supplies only the
        // characters for the `X`/`A`/`9` positions.
        if let Some(template) = self.alnum_edited.get(base_name(&key)).cloned() {
            // The sender's **bytes**, so a `0xFF` reaches its source position
            // as the one byte it is — see [`apply_alnum_edit`].
            let src = match &value {
                CobolValue::String { bytes, .. } => bytes.clone(),
                other => other.as_display_string().into_bytes(),
            };
            let edited = apply_alnum_edit(&template, &src);
            let width = edited.len();
            self.store.insert(
                key.clone(),
                CobolValue::String {
                    bytes: edited,
                    capacity: width,
                },
            );
            self.refresh_redefine_peers(&key);
            return;
        }
        // `JUSTIFIED RIGHT` reverses the alignment rule for an alphanumeric
        // receiver: the sender's *right* end is placed at the receiver's right
        // end, so a short sender is padded on the left and a long one loses its
        // leftmost characters. Everything below (and `CobolValue::assign`)
        // left-aligns, so the value is re-aligned here, once, before it lands.
        let value = match (&value, self.justified.contains(base_name(&key))) {
            (CobolValue::String { bytes, .. }, true) => {
                match self.get(&key).and_then(|v| match v {
                    CobolValue::String { capacity, .. } => Some(*capacity),
                    _ => None,
                }) {
                    Some(cap) => {
                        let src = trim_trailing_spaces(bytes);
                        let mut out = vec![b' '; cap];
                        let take = src.len().min(cap);
                        // A sender wider than the receiver keeps its rightmost
                        // `cap` characters; a narrower one is pushed right.
                        out[cap - take..].copy_from_slice(&src[src.len() - take..]);
                        CobolValue::String {
                            bytes: out,
                            capacity: cap,
                        }
                    }
                    None => value,
                }
            }
            _ => value,
        };
        // Storing a numeric into a numeric-edited field runs the edit engine and
        // keeps the result as the edited string. (Edited template / BLANK WHEN
        // ZERO are keyed by the base item, shared by all occurrences.)
        if let Some(template) = self.edited_templates.get(base_name(&key)).cloned() {
            // Accept any numeric source (incl. COMP-1/COMP-2 floats) for editing.
            let num = match &value {
                CobolValue::Float(f) => Some(CobolNumeric::new((*f * 1e9_f64).round() as i128, 9)),
                other => other.as_exact(),
            };
            if let Some(num) = num {
                let dc = self.decimal_comma;
                let cur = self.currency();
                let width = crate::numedit::edited_width(&template, dc);
                let edited = if self.blank_when_zero.contains(base_name(&key)) && num.mantissa == 0
                {
                    " ".repeat(width)
                } else {
                    crate::numedit::format_edited(&template, num.mantissa, num.decimals, dc, cur)
                };
                self.store.insert(key.clone(), CobolValue::from_str(&edited, width));
                self.refresh_redefine_peers(&key);
                return;
            }
        }
        // Lazily materialise an un-written table occurrence from its base template.
        if !self.store.contains_key(&key) && key.contains('(') {
            if let Some(base_val) = self.store.get(base_name(&key)).cloned() {
                self.store.insert(key.clone(), base_val);
            }
        }
        // A group `MOVE` leaves a **byte image** in its children, including the
        // numeric ones — the standard makes that move alphanumeric, so a
        // `PIC 99` child can legitimately be holding `"AA"` or `"  "`. The next
        // *numeric* write has to restore the item's own category, because
        // everything downstream keys off it: `truncate_to_capacity` reads the
        // slot as `CobolValue::Numeric` to apply the unsigned-magnitude rule and
        // the high-order cut, and assigning into a `String` slot instead left
        // `SUBTRACT CORRESPONDING` writing `-11` into an unsigned `PIC 99`
        // (NC253A SUB-TEST-F3-1, NC220M, NC112A).
        //
        // The replacement is built at the item's **declared scale** rather than
        // taken from the source, so `MOVE 12 TO <PIC 9(3)V99>` still lands as
        // 12.00 exactly as it would have through `assign`.
        let declared = self
            .field_caps
            .get(&key)
            .or_else(|| self.field_caps.get(base_name(&key)))
            .copied();
        if let Some(existing) = self.store.get_mut(&key) {
            let byte_image = matches!(existing, CobolValue::String { .. });
            match declared {
                _ if matches!(existing, CobolValue::Unset) => {
                    // Replace an uninitialised slot outright so the value isn't
                    // lost.
                    *existing = value;
                }
                Some((_, decimals)) if byte_image && value.as_exact().is_some() => {
                    let mut fresh = CobolValue::Numeric(CobolNumeric::new(0, decimals as u8));
                    fresh.assign(&value);
                    *existing = fresh;
                }
                _ => existing.assign(&value),
            }
        } else {
            self.store.insert(key.clone(), value);
        }
        self.truncate_to_capacity(&key);
        self.refresh_redefine_peers(&key);
    }

    /// Drop the high-order digits a numeric item has no room for.
    ///
    /// A numeric receiver holds exactly its declared digit positions. The
    /// low-order end is already cut by the rescale in [`CobolValue::assign`];
    /// this is the other end, which the standard truncates just as silently:
    /// `01 M PIC 99V999.  MOVE 123.45 TO M.` leaves `23.450`, not `123.450`.
    ///
    /// Arithmetic reaches this through `store_arith`, which tests the capacity
    /// *first* — so a statement with `ON SIZE ERROR` never gets here and its
    /// receiver keeps its old value, while one without it truncates.
    fn truncate_to_capacity(&mut self, key: &str) {
        let Some(&(int_digits, _)) = self
            .field_caps
            .get(key)
            .or_else(|| self.field_caps.get(base_name(key)))
        else {
            return;
        };
        let scaling = self.scaling_p.get(base_name(key)).copied();
        let unsigned = self.unsigned_numeric.contains(key)
            || self.unsigned_numeric.contains(base_name(key));
        let Some(CobolValue::Numeric(n)) = self.store.get_mut(key) else {
            return;
        };
        // An item with no `S` has nowhere to keep a sign: it stores the
        // magnitude. Doing this first keeps the masks below working on the
        // value that is actually held.
        if unsigned {
            n.mantissa = n.mantissa.abs();
        }
        let total = int_digits as u32 + n.decimals as u32;
        // 38 decimal digits is the widest an `i128` mantissa can hold; a wider
        // declaration cannot overflow it, so there is nothing to cut.
        if total == 0 || total > 38 {
            return;
        }
        let modulus = 10i128.pow(total);
        if n.mantissa >= modulus || n.mantissa <= -modulus {
            n.mantissa %= modulus;
        }
        // `P` positions carry no digit: whatever the sender offered for them is
        // dropped, and they read back as zero. `%` truncates toward zero in
        // Rust, so a negative value keeps its sign through both masks.
        if let Some((trailing_p, leading_p)) = scaling {
            if trailing_p > 0 && trailing_p <= 38 {
                n.mantissa -= n.mantissa % 10i128.pow(trailing_p);
            }
            let stored = (n.decimals as u32).saturating_sub(leading_p);
            if leading_p > 0 && stored <= 38 {
                n.mantissa %= 10i128.pow(stored);
            }
        }
    }

    /// Get the numeric value of a data item as `i64` (integer part only).
    pub fn get_i64(&self, name: &str) -> Option<i64> {
        self.get(name)?.as_i64()
    }

    /// Get the numeric value of a data item as `f64`.
    pub fn get_f64(&self, name: &str) -> Option<f64> {
        Some(self.get(name)?.as_f64())
    }

    /// Get the string representation of a data item.
    pub fn get_string(&self, name: &str) -> Option<String> {
        Some(self.get(name)?.as_display_string())
    }

    /// Set a data item from an `i64`.
    pub fn set_i64(&mut self, name: &str, n: i64) {
        self.set(name, CobolValue::from_i64(n));
    }

    /// Set a data item from a `f64`.
    pub fn set_f64(&mut self, name: &str, v: f64) {
        self.set(name, CobolValue::from_f64(v));
    }

    /// Set a data item from a `&str`, padding/truncating to the existing capacity.
    pub fn set_str(&mut self, name: &str, s: &str) {
        let cap = match self.get(name) {
            Some(CobolValue::String { capacity, .. }) => *capacity,
            _ => s.len(),
        };
        // A `JUSTIFIED` receiver keeps the sender's **right** end, so the value
        // must reach `set` at its own length: cutting it to the receiver's
        // capacity here would throw away the very characters it wants and
        // right-align what is left. `set` does the truncation instead.
        let cap = if self
            .justified
            .contains(base_name(&name.to_ascii_uppercase()))
        {
            s.len()
        } else {
            cap
        };
        self.set(name, CobolValue::from_str(s, cap));
    }

    /// [`Self::set_str`] over raw bytes, for a value that may hold a byte which
    /// is not a character. See [`Self::group_bytes`].
    pub fn set_bytes(&mut self, name: &str, b: &[u8]) {
        let cap = match self.get(name) {
            Some(CobolValue::String { capacity, .. }) => *capacity,
            _ => b.len(),
        };
        let cap = if self
            .justified
            .contains(base_name(&name.to_ascii_uppercase()))
        {
            b.len()
        } else {
            cap
        };
        let mut bytes = b.to_vec();
        bytes.truncate(cap);
        bytes.resize(cap, b' ');
        self.set(
            name,
            CobolValue::String {
                bytes,
                capacity: cap,
            },
        );
    }

    /// Store `bytes` in an **elementary** item exactly as they stand, at the
    /// item's own width — no numeric conversion, no editing.
    ///
    /// This is the receiving half of a move whose sender is a **group**.
    /// COBOL-85 6.18.4 makes such a move alphanumeric-to-alphanumeric, so the
    /// receiver's PICTURE contributes its *size* and nothing else:
    /// `MOVE <group holding "123ABC"> TO <PIC 0XXXXX0>` leaves `"123ABC "`,
    /// not the edited `"0123AB0"` (NC105A `MOVE-TEST-F1-20`), and
    /// `MOVE <group holding "123ABC"> TO <PIC 9999V999>` leaves those six
    /// characters plus a space, which the item's `PIC X(7)` REDEFINES reads
    /// back (`MOVE-TEST-F1-17`). It is also how a group distributes bytes into
    /// its own children, where re-editing an alphanumeric-edited child turned
    /// `"1 A05"` into `"1  0A"` (`MOVE-TEST-F1-13`).
    ///
    /// `JUSTIFIED RIGHT` is **not** applied here, because this same writer
    /// places a group's slice into a child, where the bytes are already the
    /// child's own width and must land where they fall. A move that has a
    /// receiver to align goes through [`Self::set_move_bytes`] instead.
    pub fn set_verbatim_bytes(&mut self, name: &str, bytes: &[u8]) {
        let key = self.storage_key(name.to_ascii_uppercase());
        let width = self.item_width(&key).max(1);
        let mut out = bytes.to_vec();
        out.truncate(width);
        out.resize(width, b' ');
        self.store.insert(
            key.clone(),
            CobolValue::String {
                bytes: out,
                capacity: width,
            },
        );
        self.refresh_redefine_peers(&key);
    }

    /// The receiving half of an alphanumeric move into an **elementary** item,
    /// over raw bytes: `JUSTIFIED RIGHT` decides which end is padded and which
    /// end is lost, then the bytes are stored as they stand.
    ///
    /// `MOVE <group holding "ABC"> TO <PIC A(7) JUSTIFIED RIGHT>` leaves
    /// `"    ABC"`, and a fifteen-byte group into that same receiver keeps its
    /// **rightmost** seven (NC107A `JUST-TEST-04`). The alignment lives here
    /// rather than in [`Self::set_verbatim_bytes`] because that writer also
    /// serves group distribution, where a slice is already the child's own
    /// width and must not be re-aligned.
    pub fn set_move_bytes(&mut self, name: &str, bytes: &[u8]) {
        let key = self.storage_key(name.to_ascii_uppercase());
        if !self.justified.contains(base_name(&key)) {
            self.set_verbatim_bytes(&key, bytes);
            return;
        }
        let width = self.item_width(&key).max(1);
        let src = trim_trailing_spaces(bytes);
        let take = src.len().min(width);
        let mut out = vec![b' '; width];
        out[width - take..].copy_from_slice(&src[src.len() - take..]);
        self.set_verbatim_bytes(&key, &out);
    }

    /// `true` if the named data item is declared.
    pub fn contains(&self, name: &str) -> bool {
        let key = self.storage_key(name.to_ascii_uppercase());
        self.store.contains_key(&key)
    }

    /// Iterate all data items in declaration order.
    /// Every addressable data item and its value.
    ///
    /// Unnamed `FILLER` slots are left out: they exist so a group can render its
    /// own bytes, they carry a synthetic key no COBOL statement can name, and a
    /// developer watching variables in the debugger has no use for them.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &CobolValue)> {
        self.store.iter().filter(|(k, _)| !is_filler_key(k))
    }

    // ── Nested-program scope management ──────────────────────────────────────

    /// Push a set of local data items into this environment for the duration
    /// of a nested-program call.
    ///
    /// Items that do not yet exist are inserted; items that already exist
    /// (e.g. GLOBAL names that happen to collide) are *not* overwritten —
    /// the caller's value wins.
    ///
    /// Returns the list of keys that were *newly inserted* so that
    /// [`pop_local_scope`] can remove exactly those entries.
    pub fn push_local_scope(
        &mut self,
        items: &[(String, CobolValue)],
        symbols: &[(String, ItemSym)],
    ) -> Vec<String> {
        let mut inserted = Vec::with_capacity(items.len());
        for (key, val) in items {
            let upper = key.to_ascii_uppercase();
            if !self.store.contains_key(&upper) {
                self.store.insert(upper.clone(), val.clone());
                if let Some((_, sym)) = symbols
                    .iter()
                    .find(|(sym_key, _)| sym_key.eq_ignore_ascii_case(&upper))
                {
                    self.symbols.insert(upper.clone(), sym.clone());
                }
                inserted.push(upper);
            }
        }
        inserted
    }

    /// Remove the keys that were inserted by a matching [`push_local_scope`]
    /// call, restoring the environment to its pre-call state.
    pub fn pop_local_scope(&mut self, keys: &[String]) {
        for key in keys {
            self.store.shift_remove(key);
            self.symbols.shift_remove(key);
        }
    }

    /// Collect all GLOBAL-flagged items declared in a DATA DIVISION.
    ///
    /// Returns `(name, initial_value)` pairs, ready to be inserted into a
    /// parent or sibling program's environment so nested programs can read
    /// and write them without re-declaration.
    pub fn global_items_from_data_division(data: &DataDivision) -> Vec<(String, CobolValue)> {
        let mut out = Vec::new();
        for section in &data.sections {
            match section {
                DataSection::WorkingStorage(items)
                | DataSection::LocalStorage(items)
                | DataSection::Linkage(items) => {
                    for decl in items {
                        collect_global_items(decl, &mut out);
                    }
                }
                DataSection::FileSection(fds) => {
                    for fd in fds {
                        for rec in &fd.records {
                            collect_global_items(rec, &mut out);
                        }
                    }
                }
                DataSection::Screen(_) => {}
            }
        }
        out
    }
}

/// Recursively collect GLOBAL-flagged data items (and their children).
fn collect_global_items(decl: &DataDecl, out: &mut Vec<(String, CobolValue)>) {
    if decl.is_global {
        if let Some(name) = &decl.name {
            let upper = name.to_ascii_uppercase();
            let val = default_value(decl);
            let val = if let Some(lit) = &decl.value {
                apply_literal(lit, &val)
            } else {
                val
            };
            out.push((upper, val));
        }
        for child in &decl.children {
            collect_global_items(child, out);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Format a numeric value as its fixed-width DISPLAY digit string: zero-padded to
/// `int_digits` + scale, no decimal point (the `V` is implied), leading `-` if
/// negative.
fn format_display_numeric(n: &CobolNumeric, int_digits: u8) -> String {
    let total = int_digits as usize + n.decimals as usize;
    let digits = n.mantissa.unsigned_abs().to_string();
    let padded = if digits.len() < total {
        format!("{}{}", "0".repeat(total - digits.len()), digits)
    } else {
        digits
    };
    if n.mantissa < 0 {
        format!("-{padded}")
    } else {
        padded
    }
}

/// Build the default (zero / spaces) value for a data declaration.
fn default_value(decl: &DataDecl) -> CobolValue {
    // COMP-1 / COMP-2 are PIC-less floating point — default to 0.0, not Unset.
    if decl.picture.is_none() {
        if matches!(decl.usage, Usage::Comp1 | Usage::Comp2) {
            return CobolValue::Float(0.0);
        }
        // Group items with no PIC → treat as uninitialised.
        return CobolValue::Unset;
    }
    let pic = decl.picture.as_ref().unwrap();

    match pic.kind {
        PicKind::Numeric | PicKind::NumericEdited => {
            // Decimal places never exceed COBOL's 18-digit limit, so the narrowing
            // to u8 (CobolNumeric's scale) is safe.
            CobolValue::Numeric(CobolNumeric::new(0, pic.decimals.min(u8::MAX as u16) as u8))
        }
        PicKind::Alphabetic | PicKind::Alphanumeric | PicKind::AlphanumericEdited => {
            let cap = pic.digits as usize + pic.decimals as usize;
            CobolValue::spaces(cap.max(1))
        }
    }
}

/// Apply a `VALUE` clause literal on top of a default value.
fn apply_literal(lit: &Literal, default: &CobolValue) -> CobolValue {
    match lit {
        // A `VALUE` literal written with leading zeros carries them into an
        // alphanumeric item, exactly as a `MOVE` of the same literal would:
        // `PIC X(4) VALUE 0012` is `"0012"`, not `"12  "`. A numeric receiver
        // takes the value and its own PICTURE decides the width.
        Literal::Integer(n) | Literal::IntegerDigits(n, _) => match default {
            CobolValue::Numeric(num) => {
                let mut v = CobolValue::Numeric(num.clone());
                v.assign(&CobolValue::from_i64(*n));
                v
            }
            CobolValue::String { capacity, .. } => CobolValue::from_str(
                &lit.integer_digits().unwrap_or_else(|| n.to_string()),
                *capacity,
            ),
            _ => CobolValue::from_i64(*n),
        },
        Literal::Float(f) => CobolValue::from_f64(*f),
        Literal::Decimal(m, s) => {
            // Exact decimal VALUE — rescale into the receiving field's PIC.
            let src = CobolValue::Numeric(CobolNumeric::new(*m, *s));
            match default {
                CobolValue::Numeric(num) => {
                    let mut v = CobolValue::Numeric(num.clone());
                    v.assign(&src);
                    v
                }
                CobolValue::String { capacity, .. } => {
                    CobolValue::from_str(&src.as_display_string(), *capacity)
                }
                _ => src,
            }
        }
        Literal::String(s) => match default {
            CobolValue::String { capacity, .. } => CobolValue::from_str(s, *capacity),
            // A numeric item keeps its **category** whatever the literal's is:
            // `PICTURE IS 9 VALUE IS "5"` holds the number five, not the
            // character. Storing the characters instead replaced the item's
            // `Numeric` with a `String`, and every rule that asks whether the
            // item is numeric then answered no — `BLANK WHEN ZERO` among them,
            // which reads its own zero through the numeric display path
            // (NC108M FMT-TEST-GF-3). A literal that spells no number at all
            // leaves the item at its default rather than guessing.
            CobolValue::Numeric(num) => {
                let mut v = CobolValue::Numeric(num.clone());
                if let Some(src) = crate::value::parse_decimal(s) {
                    v.assign(&CobolValue::Numeric(src));
                }
                v
            }
            _ => CobolValue::from_str(s, s.len()),
        },
        Literal::Figurative(fig) => {
            use cobolt_ast::expr::FigurativeConstant;
            let cap = match default {
                CobolValue::String { capacity, .. } => *capacity,
                _ => 1,
            };
            match fig {
                FigurativeConstant::Space => CobolValue::spaces(cap),
                // ZERO must preserve the receiving field's PIC scale — a numeric
                // field keeps its decimal places (a scale-0 zero would wipe them).
                FigurativeConstant::Zero => match default {
                    CobolValue::Numeric(n) => CobolValue::Numeric(CobolNumeric::new(0, n.decimals)),
                    CobolValue::String { capacity, .. } => CobolValue::String {
                        bytes: vec![b'0'; *capacity],
                        capacity: *capacity,
                    },
                    _ => CobolValue::zero(0),
                },
                FigurativeConstant::HighValue => CobolValue::figurative_high_values(cap),
                FigurativeConstant::LowValue => CobolValue::figurative_low_values(cap),
                // QUOTE fills the field with the quotation character, exactly as
                // SPACE fills it with blanks. Falling through to `default` left
                // `VALUE QUOTE` reading as spaces — `NC109M`'s `ACCEPT-D18` is
                // `PICTURE X VALUE QUOTE` and compared equal to nothing.
                FigurativeConstant::Quote => CobolValue::from_str(&"\"".repeat(cap), cap),
                // `ALL literal` repeats its unit across the whole item:
                // `PIC X(6) VALUE ALL "ABC"` is `"ABCABC"`, `PIC XXX VALUE
                // ALL "Z"` is `"ZZZ"`. Every other figurative constant is one
                // character and the arms above fill with it; `ALL` is the only
                // one whose unit can be wider than a byte, so it fell through
                // to `default` and the item was left holding spaces — NC211A's
                // FIG-TEST-1 and FIG-TEST-2 both read that gap out of a group
                // MOVE. `ALL` in front of a *figurative* never arrives here:
                // the parser folds `ALL SPACES` down to `SPACES`, so `inner` is
                // always a real literal.
                FigurativeConstant::All(inner) => {
                    let unit = match inner.as_ref() {
                        Literal::String(s) => s.clone(),
                        Literal::Integer(n) | Literal::IntegerDigits(n, _) => {
                            inner.integer_digits().unwrap_or_else(|| n.to_string())
                        }
                        _ => String::new(),
                    };
                    match default {
                        CobolValue::String { capacity, .. } if !unit.is_empty() => {
                            let filled: String = unit.chars().cycle().take(*capacity).collect();
                            CobolValue::from_str(&filled, *capacity)
                        }
                        // A numeric receiver has no character positions to
                        // repeat into — it takes the literal's value once.
                        _ => apply_literal(inner, default),
                    }
                }
                _ => default.clone(),
            }
        }
    }
}

/// Copy initialized bytes from the redefined item to the redefining item.
/// The ancestor group names a declaration is qualified by, outermost first.
///
/// This has to be built exactly the way [`CobolEnvironment::init_decl_h`]
/// builds it, because both feed [`CobolEnvironment::canon_key`] and the two
/// must agree on the answer: an **unnamed** group contributes nothing to the
/// path (it has no name to qualify by), every named group contributes itself.
fn push_qual(path: &[String], decl: &DataDecl) -> Vec<String> {
    let mut out = path.to_vec();
    if let Some(n) = &decl.name {
        if !n.eq_ignore_ascii_case("FILLER") {
            out.push(n.to_ascii_uppercase());
        }
    }
    out
}

fn sync_redefines(env: &mut CobolEnvironment, data: &DataDivision) {
    let mut redefines_list = Vec::new();

    fn find_redefines(decl: &DataDecl, path: &[String], list: &mut Vec<(DataDecl, String, Vec<String>)>) {
        if let Some(ref tgt) = decl.redefines {
            list.push((decl.clone(), tgt.to_ascii_uppercase(), path.to_vec()));
        }
        let child_path = push_qual(path, decl);
        for c in &decl.children {
            find_redefines(c, &child_path, list);
        }
    }

    for section in &data.sections {
        match section {
            DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) => {
                for decl in items {
                    find_redefines(decl, &[], &mut redefines_list);
                }
            }
            DataSection::FileSection(fds) => {
                for fd in fds {
                    for rec in &fd.records {
                        find_redefines(rec, &[], &mut redefines_list);
                    }
                }
            }
            _ => {}
        }
    }

    for (redefining_decl, target_name, path) in redefines_list {
        // `REDEFINES` names a SIBLING, so the target carries the same ancestor
        // path as the description redefining it. Searching the whole division
        // for the name and starting from an empty path is what this used to do,
        // and it produced a key missing every outer qualifier — invisible while
        // the names inside were unique (`canon_key` hands a unique leaf straight
        // back) and wrong the moment one was duplicated, which is exactly when
        // the qualified key matters. NC204M declares `TAB-A` under both
        // `ACCEPT-D21` and `ACCEPT-D23`, and its overlay read back as spaces.
        if let Some(target_decl) = find_decl_by_name(data, &target_name) {
            let mut bytes = Vec::new();
            serialize_decl(
                env,
                &target_decl,
                &mut path.clone(),
                &mut Vec::new(),
                &mut bytes,
            );

            let mut offset = 0;
            deserialize_decl(
                env,
                &redefining_decl,
                &mut path.clone(),
                &mut Vec::new(),
                &bytes,
                &mut offset,
            );
        }
    }
}

fn find_decl_by_name(data: &DataDivision, name: &str) -> Option<DataDecl> {
    fn search(decl: &DataDecl, name: &str) -> Option<DataDecl> {
        if let Some(n) = &decl.name {
            if n.to_ascii_uppercase() == name {
                return Some(decl.clone());
            }
        }
        for c in &decl.children {
            if let Some(found) = search(c, name) {
                return Some(found);
            }
        }
        None
    }
    for section in &data.sections {
        match section {
            DataSection::WorkingStorage(items)
            | DataSection::LocalStorage(items)
            | DataSection::Linkage(items) => {
                for decl in items {
                    if let Some(found) = search(decl, name) {
                        return Some(found);
                    }
                }
            }
            DataSection::FileSection(fds) => {
                for fd in fds {
                    for rec in &fd.records {
                        if let Some(found) = search(rec, name) {
                            return Some(found);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether `decl` is a group for layout purposes — that is, whether anything
/// subordinate to it actually occupies storage.
///
/// 88-level condition-names do not: they name *values* of the item they hang
/// under, not fields inside it. Counting them made an elementary item with a
/// condition-name attached serialize to zero bytes, so `04 RDF3-5-15 PIC 9`
/// with `88 HARD` / `88 SOFT` below it silently dropped out of its parent's
/// image and kept its default while the next field took its byte (NC252A
/// RDF-TEST-1, RDF-TEST-12).
fn has_storage_children(decl: &DataDecl) -> bool {
    decl.children.iter().any(|c| c.level != 88)
}

/// `Some(leading?)` when `decl` stores its operational sign in a character
/// position of its own (`SIGN IS … SEPARATE CHARACTER`).
///
/// The REDEFINES serializer works from the declaration rather than from the
/// environment's `sign_separate` map, because it renders the *declared* image
/// of an item that may not have a live slot yet.
fn separate_sign_of_decl(decl: &DataDecl) -> Option<bool> {
    let pic = decl.picture.as_ref()?;
    let sign = decl.sign.filter(|s| s.separate)?;
    (pic.kind == PicKind::Numeric
        && matches!(decl.usage, Usage::Display)
        && pic.template.to_ascii_uppercase().contains('S'))
    .then_some(sign.leading)
}

/// Render a numeric item's stored characters, honouring a separate sign.
///
/// `len` is the item's whole width, sign position included, so the digits get
/// `len - 1` positions when there is a separate sign to place.
fn numeric_image(n: &CobolNumeric, len: usize, sep_sign: Option<bool>) -> Vec<u8> {
    let digit_len = if sep_sign.is_some() {
        len.saturating_sub(1)
    } else {
        len
    };
    let digits = n.mantissa.unsigned_abs().to_string();
    let mut s = if digits.len() < digit_len {
        format!("{}{}", "0".repeat(digit_len - digits.len()), digits)
    } else {
        digits
    };
    if s.len() > digit_len {
        s = s[s.len() - digit_len..].to_string();
    }
    let sign = if n.mantissa < 0 { '-' } else { '+' };
    match sep_sign {
        Some(true) => format!("{sign}{s}").into_bytes(),
        Some(false) => format!("{s}{sign}").into_bytes(),
        None => s.into_bytes(),
    }
}

/// The storage width, in characters, of one elementary item's PICTURE.
///
/// `digits + decimals` is the width for every category **except numeric-edited**,
/// where those two count *digit positions* and the item is as wide as its edited
/// form. `PIC $$$,$$$.99` occupies ten characters but reports two digits and no
/// decimals: `analyze_pic` splits the integer and fractional parts on `V`, and
/// this picture's decimal separator is a real `.`, so both nines land in the
/// integer part and every `$`, `,` and `.` position is counted by nothing.
///
/// A REDEFINES overlay that believed the two came to a two-byte item truncated
/// everything the item actually held, and every field after it in the record
/// shifted up by eight — NC108M's `COMPLETE-FORMAT (19)` read `<` where
/// ` <1,1` was stored.
fn pic_storage_len(pic: &cobolt_ast::data::PicClause, decimal_comma: bool) -> usize {
    if matches!(pic.kind, PicKind::NumericEdited) {
        return crate::numedit::edited_width(&pic.template, decimal_comma).max(1);
    }
    (pic.digits as usize + pic.decimals as usize).max(1)
}

fn serialize_decl(
    env: &CobolEnvironment,
    decl: &DataDecl,
    quals: &mut Vec<String>,
    indices: &mut Vec<usize>,
    bytes: &mut Vec<u8>,
) {
    let times = decl
        .occurs
        .as_ref()
        .map(|o| o.max.max(1) as usize)
        .unwrap_or(1);
    let name_upper = decl
        .name
        .as_ref()
        .map(|n| n.to_ascii_uppercase())
        .unwrap_or_else(|| "FILLER".to_string());

    for i in 1..=times {
        let mut local_indices = indices.clone();
        if decl.occurs.is_some() {
            local_indices.push(i);
        }
        let mut local_quals = quals.clone();
        if name_upper != "FILLER" {
            local_quals.push(name_upper.clone());
        }

        if has_storage_children(decl) {
            for c in &decl.children {
                if c.level == 88 {
                    continue;
                }
                // A subordinate REDEFINES entry redescribes bytes the sibling
                // it redefines has already contributed — it is another reading
                // of them, not more of them. Emitting it as well pushed every
                // later field down by its width, so `02 RDF3 REDEFINES
                // RDFDATA3` inserted a second copy of `ALLDONXX66` and the
                // 36-element overlay above it read 11 bytes off (NC252A
                // RDF-TEST-003/5, NC107A RDF-TEST-2/10/11).
                if c.redefines.is_some() {
                    continue;
                }
                serialize_decl(env, c, &mut local_quals, &mut local_indices, bytes);
            }
        } else if let Some(pic) = &decl.picture {
            // A separate sign is a declared storage position, so it widens the
            // item by one and a REDEFINES overlay sees it (NC116A GF-1/GF-2).
            let sep_sign = separate_sign_of_decl(decl);
            let len =
                pic_storage_len(pic, env.decimal_comma) + usize::from(sep_sign.is_some());
            let numeric = matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited);

            let key = env.canon_key(&name_upper, quals);
            let key = if !local_indices.is_empty() {
                let idx_i64: Vec<i64> = local_indices.iter().map(|&x| x as i64).collect();
                subscript_key(&key, &idx_i64)
            } else {
                key
            };

            let val = env.store.get(&key);
            let mut f_bytes = vec![b' '; len];

            if let Some(v) = val {
                match v {
                    CobolValue::Numeric(n) => {
                        f_bytes = numeric_image(n, len, sep_sign);
                    }
                    other => {
                        let mut b = other.as_display_string().into_bytes();
                        b.resize(len, b' ');
                        f_bytes = b;
                    }
                }
            } else {
                let mut fallback_v = if numeric {
                    CobolValue::Numeric(CobolNumeric::new(
                        0,
                        pic.decimals.min(u8::MAX as u16) as u8,
                    ))
                } else {
                    CobolValue::spaces(len)
                };
                if let Some(lit) = &decl.value {
                    fallback_v = apply_literal(lit, &fallback_v);
                }
                match fallback_v {
                    CobolValue::Numeric(n) => {
                        f_bytes = numeric_image(&n, len, sep_sign);
                    }
                    other => {
                        let mut b = other.as_display_string().into_bytes();
                        b.resize(len, b' ');
                        f_bytes = b;
                    }
                }
            }

            bytes.extend_from_slice(&f_bytes);
        }
    }
}

fn deserialize_decl(
    env: &mut CobolEnvironment,
    decl: &DataDecl,
    quals: &mut Vec<String>,
    indices: &mut Vec<usize>,
    bytes: &[u8],
    offset: &mut usize,
) {
    let times = decl
        .occurs
        .as_ref()
        .map(|o| o.max.max(1) as usize)
        .unwrap_or(1);
    let name_upper = decl
        .name
        .as_ref()
        .map(|n| n.to_ascii_uppercase())
        .unwrap_or_else(|| "FILLER".to_string());

    for i in 1..=times {
        let mut local_indices = indices.clone();
        if decl.occurs.is_some() {
            local_indices.push(i);
        }
        let mut local_quals = quals.clone();
        if name_upper != "FILLER" {
            local_quals.push(name_upper.clone());
        }

        if has_storage_children(decl) {
            // Where each named child's bytes began, so that a REDEFINES sibling
            // can be filled from the very same bytes rather than from the ones
            // that follow them — the mirror of the skip in `serialize_decl`.
            let mut child_start: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for c in &decl.children {
                if c.level == 88 {
                    continue;
                }
                if let Some(target) = &c.redefines {
                    // Overlay: read from where the target started and leave the
                    // cursor untouched, because those bytes are already spent.
                    if let Some(&start) = child_start.get(&target.to_ascii_uppercase()) {
                        let mut overlay = start;
                        deserialize_decl(
                            env,
                            c,
                            &mut local_quals,
                            &mut local_indices,
                            bytes,
                            &mut overlay,
                        );
                    }
                    continue;
                }
                if let Some(n) = &c.name {
                    child_start.insert(n.to_ascii_uppercase(), *offset);
                }
                deserialize_decl(env, c, &mut local_quals, &mut local_indices, bytes, offset);
            }
        } else if let Some(pic) = &decl.picture {
            let sep_sign = separate_sign_of_decl(decl);
            let len =
                pic_storage_len(pic, env.decimal_comma) + usize::from(sep_sign.is_some());
            let numeric = matches!(pic.kind, PicKind::Numeric | PicKind::NumericEdited);

            // A redefining description may be WIDER than the storage it
            // redescribes, so the cursor can walk past the end of the bytes.
            // Both ends are clamped: clamping only `end` left `start > end` and
            // the slice panicked.
            let start = (*offset).min(bytes.len());
            let end = (*offset + len).min(bytes.len());
            let slice = &bytes[start..end];
            *offset += len;

            if name_upper != "FILLER" {
                let key = env.canon_key(&name_upper, quals);
                let key = if !local_indices.is_empty() {
                    let idx_i64: Vec<i64> = local_indices.iter().map(|&x| x as i64).collect();
                    subscript_key(&key, &idx_i64)
                } else {
                    key
                };

                if numeric {
                    let digits: String = slice
                        .iter()
                        .map(|&b| if b.is_ascii_digit() { b as char } else { '0' })
                        .collect();
                    let mantissa: i128 = digits.parse().unwrap_or(0);
                    // A separate sign is a real character in these bytes, and
                    // the digit-only scan above has just turned it into a `0`.
                    // Recover it before the value is stored, or an overlay
                    // write through the redefining description silently drops
                    // the sign of the item it redescribes.
                    let mantissa = if sep_sign.is_some() && slice.contains(&b'-') {
                        -mantissa
                    } else {
                        mantissa
                    };
                    let decimals = pic.decimals.min(u8::MAX as u16) as u8;
                    env.set(
                        &key,
                        CobolValue::Numeric(CobolNumeric::new(mantissa, decimals)),
                    );
                } else {
                    env.set_str(&key, &String::from_utf8_lossy(slice));
                }
            }
        }
    }
}

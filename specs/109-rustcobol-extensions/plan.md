<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — RustCOBOL extensions for compact generated applications

- **Status:** draft (awaiting review)
- **Spec:** ./spec.md   **Date:** 2026-10-30
- **Line:** `features-2x` (operator, Q1, 2026-10-30). Implementation starts only
  after spec 108 is in `main` and its port is in `2.x.x` (operator ruling,
  2026-10-30); this plan changes no code.

## 1. Approach

Eight capabilities delivered as the spec's slices S1–S8, each one reviewable change
with its own golden fixtures, `z` bump and CHANGELOG entry (R20–R22), preceded by a
read-only inventory (T0) and followed by a measurement pass. No second engine: every
slice reuses a structure named in spec §2.

**Order** (the spec's table leaves it to the plan): T0 inventory, S3, S2, S1, S4, S8,
S6, S7, S5, then measurement. Reasons: S3 and S2 add the vocabulary everything else
generates (collection value, `THROUGH`, `F"…"`, `Format`), have no external
dependency and unblock S4 and the generator; S1 follows because WebAssembly cannot
run `FUNCTION` until the IF and IC modules land (WASM-PARITY row 4), so it ships with
a build-time refusal on Web hosts (R23). S5 goes last: it removes generated storage
and is safe only once T0 proves what is unused and the other slices have changed
what the generator emits.

| Step | Slice | Requirements | Mechanism |
|------|-------|--------------|-----------|
| T0 | Dependency inventory | R29, R36, Q9 | A read-only analysis over PowerAnalytics and `examples/`: which `WS-<control>` items and helper paragraphs authored code reads. Output is a checked-in report under `specs/109-…/`, not code. Not proven unused means kept. |
| S3 | Collections, `PERFORM VARYING … THROUGH` | R11–R16, R37, R38 | New collection value in the interpreter; methods `Join Map Filter Sum Max SortByDescending Take` added to `cobolt-ast/src/methods.rs` (`is_known_method`); `=>` token in the lexer; new `PerformTarget` variant; `Split` yields a collection only in a collection context (R38). Q3/Q4 semantics as settled. |
| S2 | `F"…"`, `::Format()`, `FORMAT`/`LOCALE` clauses | R4–R10, R28 | Lexer prefix `F"`; interpolation parsed to an AST node (options after a single `:`); data-clause parsing in `cobolt-parser/src/data.rs` (recognised by position, never reserved); one formatter in a new shared-source crate (as `cobolt-decimal`/`cobolt-numedit`) used by interpreter and `cobolt-wasm-rt`. |
| S1 | `FUNCTION-ID` | R1–R3 | Lexer/parser unit kind; units registered through REPOSITORY (existing parse and write-back, `cobolt-codegen/src/lib.rs:494-507`), visible project-wide from `src/*.cbl`; the existing `Expr::FunctionCall` reaches user functions; intrinsic-name clash refused (Q5). |
| S4 | `Descendants`, `Controls`, `SetProperties` | R17–R19 | Reuse the parent-membership walk of `cascade_ids_for` (`cobolt-ide/src/panels/designer.rs`) and the renderer's, moved to a shared place in `cobolt-forms`; `SetProperties("Name" value …)` pairs (Q11), validate-all-then-write. |
| S8 | Direct result binding | R34 | Extend `DataBindingDef` (`cobolt-forms/src/model.rs`) and `COBOL-DATA-BINDINGS-POPULATE`; begin with the PartiQL result set of spec 108 (R19–R21, R31–R35); the Scalar/multi-field APIs amend 108 (Q10), not this spec. |
| S6 | Declarative layout | R32 | Persist relationships in the form model; lower into `cobolt_forms::layout::solve` inputs (spec 056 R9, R10). Grammar and Web lowering (CSS, WASM-004 R8) are decided in the task that opens S6 (Q7). |
| S7 | Animation sequences | R33 | Entries scheduled on the existing animation/timer scheduler; `delay_ms` adds to `AFTER` (Q8); cancel/restart/close tracked per form instance. Syntax decided in the task that opens S7. |
| S5 | Selective storage, runtime services | R29–R31 | `write_control_group` (`cobolt-codegen/src/lib.rs:944-1151`) emits a group only when T0's report, or an authored reference found by the generator, needs it; helper paragraphs stay as compatibility entry points. |
| M | Measurement | R36, AC11, AC20 | Frozen PowerAnalytics baseline by hash, blob and generator commit; per-construct LOC report. Real numbers, no unmeasured percentage. |

## 2. Affected crates / files

Per spec §8 "Where the work lands"; representative paths only.

- `crates/cobolt-lexer/src/{keywords,token}.rs` — `F"` prefix, `=>`, `FUNCTION-ID`.
- `crates/cobolt-parser/src/{expr,stmt,data,parser}.rs`, `crates/cobolt-ast/src/{expr,stmt,data,methods}.rs` — nodes, `PerformTarget`, clauses, closed method list.
- `crates/cobolt-semantic` — signatures, locale references, lambda purity.
- `crates/cobolt-runtime/src/interpreter.rs`, `builtins.rs` — collection value, formatting calls, function dispatch; `crates/cobolt-stdlib` — shared helpers.
- New shared-source formatter crate (name fixed in S2) — consumed by `cobolt-runtime` and `cobolt-wasm-rt`.
- `crates/cobolt-forms/src/{model.rs,layout/}` — additive persisted layout, sequence and binding entries (`serde(default)`).
- `crates/cobolt-codegen/src/lib.rs` — direct emission, selective storage.
- `crates/cobolt-compiler`, `crates/cobolt-cli` — function-unit discovery, `run_form_app`.
- `crates/cobolt-wasm`, `crates/cobolt-wasm-rt` — build-time refusal of unsupported constructs, then support; `rt.wasm` rebuilt.
- `crates/cobolt-ide/src/{panels/editor.rs,beautify.rs,i18n.rs,ui_text_data.rs,version.rs}`, `crates/cobolt-dap` — completion, diagnostics, source maps, `Tr` ×6, version.
- `crates/cobolt-compiler` doc tables, `assets/knowledge/chunked.data` — KB regenerated per slice.
- `specs/WASM-PARITY.md` — a row per construct not yet supported on Web.
- **Not touched on 2.x.x:** `README.md`, `docs/` (R27); the Guide text is kept in `specs/109-…/` until the line is released, and written afresh when 2.x.x merges to main.
- **Not touched at all:** `specs/108-partiql-indexed-files/` (AC13), except the Q10 amendment, which is a separate 108 change.

## 3. Data / model changes

- `.cfrm`, `.wcfrm`, `.mcfrm`: three new optional entries (layout rules, animation sequences, result bindings), all `serde(default)`; old files load and regenerate unchanged (R35).
- Project manifest: a default currency beside `[project] locale` (`cobolt-ide/src/project_model.rs`); absent means no default, and `currency-short` without any currency is a diagnostic (R8).
- COBOL surface: `FUNCTION-ID`, `F"…"`, `FORMAT`/`LOCALE` data clauses, `=>`, `PERFORM VARYING … THROUGH`, the new methods. All additive; none reserved (R28).
- Generated `.cbl`: smaller; banner kept; deterministic.

## 4. Key decisions & alternatives

- **S3/S2 before S1** — Why: no external blocker, and the generator needs their vocabulary. Rejected: S1 first (the literal spec order), because WebAssembly refuses `FUNCTION` today.
- **Formatter as shared source** — Why: R9/R23 demand identical output on every host, and `cobolt-decimal` is the precedent. Rejected: two implementations held equal by tests alone (the French group separator already differs between CLDR releases).
- **Pinned built-in locale data, three locales** — Why: determinism; no OS locale (R8). Rejected: host locale lookup.
- **Method vocabulary first** — Why: an unlisted name silently becomes a subscript (spec §2); every method is added to `is_known_method` in the same commit as its runtime code, and the dispatch test must go red without it (R37).
- **Inventory before S5, conservative retention** — Why: no dependency data exists; user code is sacred.
- **Layout and sequences lowered onto existing engines** — Why: no second solver or scheduler; grammar chosen when the slice opens, against the solver's rules.
- **One branch, slice by slice** — each slice its own commit and `z`; fixes found on the way go to `fixes-2x` first (rule #5).

## 5. Risks & mitigations

- **Overlap with 108's files** (`interpreter.rs`, `builtins.rs`, `version.rs`, KB texts, CHANGELOG) → start only after 108 is in `main` and ported; re-sync (`git rebase 2.x.x`) before the first edit.
- **New tokens (`F"`, `=>`) change an existing COBOL-85 program** → NIST NC census plus every finished module's execution pass (rule #9) before each lexer/parser slice is reported done; corpus scan for `FORMAT`/`LOCALE` as data names (a fixture declares them).
- **Four hosts drift** → each slice's last task is the host sweep: `rcrun run-form`, embedded forms, `run_form_app`, `cobolt-wasm`; unsupported on Web is a build-time diagnostic plus a WASM-PARITY row, never a silent no-op.
- **Selective storage breaks hidden references** (dynamic names, Common Code, debugger watches) → T0 plus retention of anything not proven unused; AC15 legacy cases.
- **S6 grammar collides with anchors/dock/flex/grid or the Web CSS path** → resolved in the S6 opening task by the solver's own precedence; refuse on Web if it cannot lower.
- **The 50% LOC target is missed** (the study measured 4.29% conservative) → AC20 requires an honest gap report; compatibility gates pass regardless.
- **Scale: eight slices** → each is independently shippable; the operator can stop after any.

## 6. Test strategy

- Per slice, in the crate it touches: parser/lexer unit tests (spans, braces, `FORMAT` as a data name), interpreter tests under `tests/cobol/**` ending with ONE result block (rule #7: name, cases, per-phase ms and throughput, tally), generator golden fixtures (`UPDATE_GOLDEN=1` only after reading the diff), differential tests `tests/common::same_output` for host parity.
- Mapped to the spec: S1 AC1; S2 AC2–AC4; S3 AC5, AC6, AC22; S4 AC7, AC8; S5 AC15, AC16; S6 AC17; S7 AC18 (deterministic clock); S8 AC19; generator AC9, AC10; measurement AC11, AC20; hosts AC12; docs AC14; scan AC23.
- Standing gates, every slice: `cargo test -p cobolt-forms --features render`; `engine_reference_form_parity_static_vs_faces`; DataGrid tests (specs 094, 095); `cobolt-ide` `--bin`; sweeps `--no-fail-fast`, every `test result:` line read; known reds on 2.x.x reported as pre-existing; NIST census and finished modules for lexer/parser slices.
- Visual: only through the MCP tools (`render_form`, `run_form`); never drive the window.
- Credential scan of every staged diff; no example `.cfrm` copied into a fixture unscanned (AC23).

## 7. Steering compliance

- [ ] i18n: completion/hover text and any settings entry in 6 languages (`Tr`); COBOL names, format names, locale tags and compiler diagnostics stay English
- [ ] Generated-code banner and regenerate-on-action contract preserved (R22)
- [ ] Docs by line: nothing under `README.md`/`docs/` on 2.x.x; Guide text kept in `specs/109-…/` and written when 2.x.x merges to main; translations deleted then (rule #8)
- [ ] System KB: `cobolt-compiler` doc tables updated and `chunked.data` regenerated per slice (`build_chunked_kb`)
- [ ] Feature, not fix: `features-2x`, `z` bump and top CHANGELOG entry per slice (2.x.x's next `z`); fixes never share a commit or push with a feature
- [ ] Interpreter/binary/WASM parity per slice (`interpreter-binary-parity`; `rt.wasm` rebuilt)
- [ ] User code sacred; no window self-resize; no "cobolt" in user text; no credentials
- [ ] Push window rule #1; merge into 2.x.x only when asked; forum posts only on request

## 8. Decision notes for S6 and S7 (T6.1, T7.1) — proposed 2026-10-31, awaiting the operator

Grounded in the source: the solver reads **properties** (`LayoutMode`, `FlexDirection`,
`FlexWrap`, `Gap`/`RowGap`/`ColumnGap`, `GridColumns`, `GridRows`, `ColumnSpan`, `RowSpan`,
`Order`, `Padding*`, `MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight`, `Dock`, `Anchor`) and a
breakpoint table of sparse `(control, property, value)` overrides (`layout/breakpoints.rs`).
The animation side is `AnimRuntime` (`cobolt-forms/src/anim.rs`): a frame-delta clock, one
`AnimPlayback` per `(control, name)` with `delay_remaining`.

### 8.1 S6 — Declarative layout (Q7)

**Principle.** A `LAYOUT` block is *authored shorthand for properties the solver already
reads*. It adds no solver input, no coordinate system and no second solver (R32). At load
the block is lowered into a **derived overlay** of the controls' layout properties and the
breakpoint table's overrides; the designed values are never written (spec 056 R10).

**Grammar** (one block per container; `ME` is the form):

```cobol
LAYOUT PNL-FILTERS
    MODE FLEX ROW WRAP          *> MODE FLEX ROW|COLUMN [WRAP] | GRID | FLOW
    GAP 14                      *> GAP n  or  GAP row column
    PADDING HORIZONTAL 24       *> PADDING n | HORIZONTAL n | VERTICAL n
    COLUMNS 2                   *> grid only; COLUMNS WHEN COMPACT 1 OTHERWISE 2
    ORDER LV-DROP BTN-GO        *> reading order of children (the `Order` property)
    ITEM LV-DROP SPAN 2 GROW 1 MIN-WIDTH 160 MAX-WIDTH 480
    WHEN COMPACT
        MODE FLEX COLUMN
        GAP 8
    END-WHEN
END-LAYOUT
```

- Lowering table: `MODE`→`LayoutMode`/`FlexDirection`/`FlexWrap`; `GAP`→`Gap`/`RowGap`/`ColumnGap`;
  `PADDING`→`Padding*`; `COLUMNS`→`GridColumns`; `ORDER`→`Order` 1..n in the list given;
  `ITEM`→`ColumnSpan`, `FlexGrow`, `Min/MaxWidth`. Names are the solver's own, English.
- `WHEN <breakpoint>` lowers to the breakpoint table's overrides; the names are the table's
  (default Compact, Medium, Expanded; boundaries 599/600 and 1023/1024). An unknown name is
  an error. `COLUMNS WHEN … OTHERWISE …` is the one-line form of the same thing.
- **The sketch's `BELOW x GAP n` and `MARGIN` are not adopted as such.** The solver has no
  sibling-relative placement (designed rectangles in, placements out). `BELOW` is accepted
  only as `ORDER` within one Flex/Grid/Flow container, with `GAP` as the container's gap;
  between siblings of an Absolute parent it is a diagnostic (use `DOCK`/`ANCHOR` or a
  container mode). `MARGIN` has no engine property: `PADDING` of the container only.
- **Persisted** as the block's text in `<layout-rules>` on the `<Form>` (additive, default
  absent), parsed by a small parser in `cobolt-forms`; opt-in per form (R35). Source text is
  kept verbatim so the developer's own comments and order survive a save.
- **Precedence.** A control named by a block is *owned*: its `Dock`, `Anchor`, `Order`,
  span and size-limit properties come from the block, and the inspector shows them read-only
  ("set by LAYOUT"). Unnamed controls and forms without a block behave as today.
- **Imperative writes.** `SET CTRL::Left/Top/Width/Height/Dock/Anchor` on an owned control is a
  **Check error** when the property name is a literal in the source; a dynamic write is
  ignored at run time with a `RuntimeWarning`. (Option B: let the write win. Rejected: two
  writers is the competing-writer problem R32 forbids.)
- **Diagnostics with spans** (control id + `.cfrm` line): a control in two blocks; a container
  inside its own `ORDER`; a `MODE` that does not fit the control (`COLUMNS` on a Flex
  container); `ITEM` for a non-child; an unknown breakpoint.
- **Window.** Never resized by a rule. `MinFormWidth` is not touched by lowering; the form's
  minimum stays the narrowest breakpoint's (056 R18).
- **Web and Mobile.** The player already lowers the four modes and the breakpoint selection
  to CSS from these same properties (`layout_css.rs`), so a block that lowers to properties
  works there with no player change. Anything outside the table is a build-time refusal naming
  the construct, plus a `WASM-PARITY` row (R23).
- **Generator.** Emits nothing per control: the block travels with the form description; the
  generated COBOL has no `COMPUTE`/`SET` lists (R20).

### 8.2 S7 — Animation sequences (Q8)

**Principle.** A sequence is a list of timed entries run by a small `SequenceRuntime` that
sits beside `AnimRuntime` and is ticked by the same frame delta; one per form instance.

**Declaration** (persisted as the block's text in `<sequences>` on the `<Form>`):

```cobol
ANIMATION-SEQUENCE DASHBOARD-INTRO
    SHOW C-KPI    AFTER 0
    SHOW C-BAR    AFTER 50
    PLAY C-RADAR "fade-in" AFTER 100
    HIDE LBL-WAIT AFTER 100
END-SEQUENCE
```

- Actions: `SHOW`/`HIDE` set `Visible` through the path `SET ctrl::Visible` uses; `PLAY` starts
  a named `AnimationDef` on the control. `SHOW` alone never invents a `PlayAnimation` (§5.7).
  Anything else (a paragraph, a CALL) is not an entry: keep it in the handler that starts the
  sequence.
- `AFTER n`: whole milliseconds from the sequence's start, never from the previous entry.
  For `PLAY`, the animation's own `delay_ms` **adds** to `AFTER` (spec Q8 recommendation).
- Order: an entry fires when `elapsed >= n`; entries due in the same frame fire by `(n,
  declaration order)`; each fires at most once.
- **Clock.** Accumulated frame deltas in integer ms; no wall clock inside the runtime. Tests
  drive it with explicit deltas (AC18).
- **Verbs are inline built-ins, not new statements** (rule: built-in CALLs are inline; no new
  reserved words, no change to the lexer or the NIST census):
  `COBOL::"SEQUENCE-START" ( "DASHBOARD-INTRO" )`, `COBOL::"SEQUENCE-CANCEL" ( name )`,
  `COBOL::"SEQUENCE-RESTART" ( name )` (cancel, then start from 0) and
  `COBOL::"SEQUENCE-RUNNING" ( name flag )` (flag is `1` while entries remain).
  Starting a sequence that runs is a no-op; starting an unknown name is a runtime error.
- **Cancel** drops the entries not yet fired; what already happened stays (no undo), and a
  started animation keeps playing unless the handler stops it with the existing
  `_StopAnimation`. **Form close** drops the instance's runtime with the host, so nothing fires
  against a closed or replaced form; two open forms never share a sequence.
- **Existing timers and animations** are untouched. A sequence is adopted by writing one; the
  generator never rewrites a user's `STAGE-TICK`.
- **Web and Mobile.** First cut: refused at build time with the construct named, plus a
  `WASM-PARITY` row (the player has no scheduler for non-animation actions yet).
- **Generator.** Emits the invocation and nothing of the state machine (§5.7).

### 8.3 Questions for the operator

1. **`BELOW`/`MARGIN`:** accept the narrowing above (ORDER within a container; PADDING only)?
2. **Imperative write on an owned control:** Check error for a literal property, ignore plus
   warning for a dynamic one — or let the write win?
3. **Sequence verbs:** inline built-ins `COBOL::"SEQUENCE-START"` (recommended) or new
   `START SEQUENCE x` statements?
4. **Completion:** poll with `SEQUENCE-RUNNING` only (recommended for the first cut), or add an
   `onSequenceDone` event?
5. **Web:** refuse sequences at build time now (recommended), or implement a player scheduler in
   the same slice?

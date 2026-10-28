<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — RustCOBOL extensions for compact generated applications

- **Status:** draft, 2026-10-28. Codex drafted it from the operator's requests of
  that day; it was then checked against the source of the 2.x.x line at 2.0.80
  (and again at 2.0.84, when it was merged; see §2) and adjusted — §2 records
  what the source says, with `file:line`, and the adjustments are listed at the
  end of §9. The operator's rulings in the draft
  are carried in unchanged (§9, "Settled"); twelve questions are open, each with
  a recommendation. This spec changes no code, no example, no document and no
  version.
- **Folder:** specs/109-rustcobol-extensions/
- **Author:** Codex (draft) and Anthropic Claude Codex Agent (review against the
  source), for Emerson Lopes   **Date:** 2026-10-28
- **Classification:** feature — interpolation, `FORMAT` / `LOCALE`, collection
  methods and bulk control operations are RustCOBOL extensions, `FUNCTION-ID` is
  COBOL-2002, and declarative layout, animation sequences and direct result
  bindings are additive: every one a capability beyond COBOL-85 and beyond the
  IDE's existing scope. `features` branch (the precedent of spec 108; the 2.x.x
  line takes it by cherry-pick when the operator says so, §9 Q1), next fix number
  `z` per change, forum f=96 if ever announced (GOLDEN RULE #5 / #4b).

## 1. Overview

Reduce generated COBOL by expressing reusable functions, formatted text,
collection transformations and operations on groups of controls directly in
RustCOBOL. The form generator must emit these high-level constructs in its
published `.cbl`, rather than expand them into temporary variables, manual loops,
per-control statement lists or global argument/result plumbing.

The original `.cfrm` files and their authored code remain the source of truth.
Nothing here rewrites an example form or a developer's handler: every construct
is adopted explicitly (R35).

The PowerAnalytics investigation of 2026-10-27 measured 8,688 original lines and
8,315 lines in a conservative standalone optimization (373 fewer, 4.29%); §2
re-measured the first figure and it is exact. The remaining opportunity is
structural. Existing example spans include SET-FILT-HEIGHT (430 lines),
STAGE-TICK (270), Q-YEARS (196), and 98 nested programs — all re-measured, §2.
These counts explain the motivation. Declarative alternatives to layout and
animation plumbing are in scope for newly authored or explicitly migrated forms;
the existing event-handler architecture and user code remain compatible.

The optimization study shall pursue a 50% reduction from the frozen 8,688-line
PowerAnalytics baseline (at most 4,344 lines), without claiming this is already
achievable. Report measured progress and any remaining gap; do not trade away
compatibility, remove user comments or rewrite the original examples to reach a
count — the study works on a migrated copy.

**What that target is up against** (measured, §2). The baseline is 2,697 lines of
outer program and 5,991 lines of the 98 handler programs. A generator that wrote
nothing at all for the outer program would still leave 5,991 lines — 1,647 above
the target — so the target can be met only by a migrated copy whose handler
programs shrink, through the constructs of §5, in the authored code and in the
generated wrappers around it. It is the result of that study, not a promise of
the generator alone (R36, AC20).

## 2. What exists today

*Mapped 2026-10-28 against the 2.x.x working tree at 2.0.80 (`main` carries no
WebAssembly crates) and checked again at 2.0.84, when the spec was merged. The
four commits in between changed the WebAssembly runtime and player, the Web
project tools, Run for Web and the PowerAnalyticsWeb example; none touched the
lexer, parser, AST, interpreter, code generator, form model or form host, and one
citation moved (`SetProperty` in `cobolt-compiler`, now `:9487`). Line numbers
are the working tree's. What the draft assumed is checked here; where the source
differs, the sentence says so in bold.*

**There are no user-defined functions.** `FUNCTION name(args)` parses as
`Expr::FunctionCall { name, args }`, the form the intrinsic functions use
(`crates/cobolt-parser/src/expr.rs:363-399`; an argument list is bounded by `)`
and the comma between arguments is optional, `:383`, `:636-648`). The lexer knows
the word `FUNCTION` (`cobolt-lexer/src/keywords.rs:250`) and has no
`FUNCTION-ID`; the IDE editor lists it among the COBOL-2002 words it offers
for completion and its beautifier treats as reserved
(`cobolt-ide/src/panels/editor.rs:209-229`, `beautify.rs:459`). The REPOSITORY paragraph is parsed and
written back by codegen (`cobolt-codegen/src/lib.rs:494-507`); today it holds
`CLASS … IS "…"` entries, the curated Rust types of spec 041, and
`FUNCTION ALL INTRINSIC` (`lib.rs:3640`). A source file may hold several program
units written in sequence, each closed by `END PROGRAM name.`; the parser
attaches the trailing ones to the first as nested programs and the runtime
dispatches all of them from one flat program registry
(`cobolt-parser/src/parser.rs:508-525`). That is the nearest existing thing to
the "source-unit siblings" of §5.1, and it is not a function. The WebAssembly
generator refuses `FUNCTION` calls and nested programs at build time today
(`specs/WASM-PARITY.md` row 4, open with the IF and IC NIST modules).

**There is no interpolated string, no `FORMAT` clause and no `LOCALE` clause.** A
prefix glued to a quote lexes as one token — `X"…"`, `N"…"`, `NX"…"`, `U"…"`,
`UX"…"` (`cobolt-lexer/src/token.rs:157-185`); `F"…"` is not one. `<>` is a token
(`token.rs:305`); **`=>` is not**, so the lambda arrow of §5.3 is new. `FORMAT`
and `LOCALE` are not keywords (`keywords.rs`), and a data clause the parser does
not know is skipped with a warning (`cobolt-parser/src/data.rs:656`): today
`PIC 9(9) FORMAT IS "number"` is accepted, warned about, and the clause ignored.
**The corpus holds no collision:** no standalone `FORMAT` or `LOCALE` is used as a
COBOL word in `tests/cobol`, in the NIST suite (`NIST/newcob.val,cbl`, 28 MB —
its hits are inside literals, such as `"IX-28; FORMAT 2"`) or in the COBOL of the
examples (method: a hyphen-aware word match with comments and quoted text
excluded). **The one `Format` that exists is a control property** — the
DateTimePicker's `Format` (`Short`, `Long`, `Time`) and the Viewer's — read from a
control, never from a data item (§5.2, R6). Numbers are edited by edited pictures
(`cobolt-numedit`) and `DECIMAL-POINT IS COMMA` (the parser's `decimal_comma`);
nothing else is locale-aware. On the 2.x.x line a project's manifest carries
`[project] locale`, "the default locale of the application, such as `pt-BR`"
(`cobolt-ide/src/project_model.rs:706-709`, WASM-001 R3; not on `main`), and the
web player shows a date or number field with the desktop's formats, not the
browser's locale's (`cobolt-web-player/src/base_css.rs:56`) — the principle R8 and
R9 repeat.

**The method vocabulary is closed, and no value is a collection.** A data item's
value methods are `Trim`, `Upper`, `Lower`, `Replace`, `Len` and `Split`, plus
five older spellings that still run (`cobolt-ast/src/methods.rs:25-52`; the
operator ruled on 2026-10-10 that each method has ONE spelling). Whether
`X::name(…)` is a call or a subscript is decided by the closed list
`is_known_method` (`methods.rs:85`): a name missing from it **silently becomes a
subscript** (`methods.rs:7-13`, `:113-117`). A test guards the control methods the
runtime dispatches (`every_dispatched_control_method_is_spellable_inline`,
`cobolt-runtime/src/interpreter.rs:24300`). **`Join`, `Format`, `Map`, `Filter`,
`Sum`, `Max`, `SortByDescending`, `Take`, `Descendants`, `Controls` and
`SetProperties` do not exist** (`SetProperty(name, value)` does,
`cobolt-compiler/src/lib.rs:9487`). **`Split` is not a collection value:**
`WS-CSV::Split(",")(2)` evaluates the split and answers part 2
(`interpreter.rs:18169-18188`), and without a subscript it answers part 1
(`:15945-15954`); no value in the runtime is a list of strings. The draft's
"existing Split and Join" is therefore half right: `Split` exists as a subscripted
accessor, `Join` does not exist at all.

**`PERFORM VARYING element THROUGH collection` does not exist.** `PERFORM
VARYING` parses in the standard shape only (`cobolt-parser/src/stmt.rs:1043-1227`;
the AST's `PerformTarget` has no collection variant,
`cobolt-ast/src/stmt.rs:192-262`); `THROUGH` / `THRU` is one token, used by
`WHEN … THRU`, `CLASS` and `RENAMES` (`stmt.rs:965-985`, `parser.rs:920`,
`data.rs:576`). `EXIT PERFORM` and `EXIT PERFORM CYCLE` exist
(`cobolt-ast/src/stmt.rs:181-183`). Nothing in the parser, the AST, the runtime,
the Guide, the tests or another spec defines the iteration form: the draft's
"reuse the existing iteration contract" described an intent, not code. §5.3 and
R16 define the construct; what the draft called reconciling is decided in §9 Q4.

**The form is `ME`, and controls are written one property at a time.** A handler
addresses its form as `ME::` (`ME::Breakpoint`, `INVOKE ME::"OpenFormSync"(…)`;
`docs/developers-guide-en.md:2432`, `:5869`) or by the form's own id
(`CHAT-FORM::Height`, `:8208`); **`FORM` is not a handle** (the word occurs
nowhere in the interpreter), so §5.4 writes `ME::Controls(…)`. A control is set
one property at a time, or by name with `SetProperty(name, value)`; no method
takes several, and **the language has no named arguments** — an argument is an
expression, and `Name:` is not one. Walking a container's descendants exists
twice, but not as a run-time method: the Designer's delete and copy
(`cascade_ids_for`, `cobolt-ide/src/panels/designer.rs:5929`, spec 091 F13) and
the renderer, which moves a container's descendants with it (spec 056 F10).
`Descendants()` should reuse that parent-membership walk, not add a third.

**The generator writes storage for every control.** `write_control_group`
(`cobolt-codegen/src/lib.rs:944-1151`) writes a `01 WS-<ID>.` group with `-TEXT`,
`-VISIBLE`, `-ENABLED` and type-specific items for every control. In the
PowerAnalytics baseline that is 214 groups and 911 declaration lines, 10.5% of the
file. Whether any authored code reads a given item is recorded nowhere, so R29 has
no dependency data to start from: the plan produces it (§9 Q9).

**Layout, animation and bindings are engines that already exist.**

- *Layout.* One pure function, `cobolt_forms::layout::solve`
  (`cobolt-forms/src/layout/mod.rs:448`, spec 056): the designed controls, the
  designed size, the form's layout properties and breakpoint table, and the size
  of the surface in; every placement out. It never writes the design, depends on
  nothing of the previous frame (056 R9, R10, R24), and is the same call for the
  designer, the preview, the run-form window, a ContentPane occupant and the
  compiled binary (056 R22, R23). Its vocabulary is anchors, dock, flex, grid
  tracks, size limits and minimum sizes, and a breakpoint table whose default is
  Compact [0, 600), Medium [600, 1024), Expanded [1024, ∞)
  (`layout/breakpoints.rs:204`) — **Expanded, not "Wide"**. A Web or Mobile project
  does not run that solver: the player lowers the four layout modes to the
  browser's own CSS layout and reuses only the breakpoint selection (WASM-004 R8,
  `cobolt-web-player/src/layout_css.rs`).
- *Animation.* An `AnimationDef` (name, trigger, kind, `duration_ms`, `delay_ms`,
  easing, repeat, …; `cobolt-forms/src/model.rs:1199`) is played by name —
  `INVOKE ctrl 'PlayAnimation' USING BY VALUE "name"` — through the generated
  `COBOL-PLAY-ANIMATION` / `COBOL-STOP-ANIMATION` paragraphs
  (`cobolt-codegen/src/lib.rs:1667`) and the form host's `_PlayAnimation`,
  `_StopAnimation` and `_PauseAnimation` commands
  (`cobolt-form-host/src/host.rs:65-76`). There is no sequence: PowerAnalytics
  sequences its entrance by hand, in STAGE-TICK (270 lines), which a Timer
  (`TMR-STAGE`, 50 ms) calls and which shows one card every 400 ms
  (`generated/main-form.cbl:2780`, `:3719-3729`).
- *Bindings.* A `DataBindingDef` — source, target, mappings, mode, validation
  (`cobolt-forms/src/model.rs:2497`) — over five source kinds (IndexedFile, Sql,
  CobolTable, RestApi, AgentAi; `:2092`) and targets that include DataGrid, Chart,
  ComboBox, ListBox and ControlArray (`:2293`). It is populated after `onLoad` by
  the generated `COBOL-DATA-BINDINGS-POPULATE`, re-run by `RefreshBinding()`
  (`methods.rs:158`), and writable for a DataGrid (specs 094, 095).

**The baseline, measured.**

- `examples/PowerAnalytics/generated/main-form.cbl`: 8,688 lines, 387,801 bytes,
  SHA-256
  `281c6767a8d33754a1f395b2af475d2db7eea109e9e5ee03c5020557b3c99d4a`, file time
  2026-10-26 20:25 — exactly the investigation's figures. **It is not in git**
  (`.gitignore:41`, `examples/*/generated/`): it is the output of
  `forms/main-form.cfrm` (tracked; 11,292 lines; git blob
  `9086c77b86e3a05fed2b3106429424db304bee0d`; SHA-256
  `bdad6bcc4e3258f33c8ceaca8de117fba89bed956f98ee8b506d8e57ce18908c`; file time
  2026-10-12), written by the generator as of 2026-10-26, and it moves with every
  regeneration. This review did not regenerate it (the generator is at 2.0.84
  now; `cobolt-codegen` has not changed since 2.0.80).
- 99 `PROGRAM-ID`s: the outer MAIN-FORM and 98 `IS COMMON PROGRAM` handler
  programs. The spans the draft quotes are exact, from `PROGRAM-ID` to
  `END PROGRAM`: SET-FILT-HEIGHT 430 lines (7858-8287), STAGE-TICK 270
  (8369-8638), Q-YEARS 196 (5238-5433).
- The outer program is lines 1-2,697 (31.0%); the 98 handler programs and the
  closing line are lines 2,698-8,688, 5,991 lines (69.0%).
- The study's copy, `main-form.optimized.cbl` (8,315 lines, 380,788 bytes), lives
  outside the repository.

**Spec 108** (PartiQL for indexed files) is a draft. It defines the control's
result set — an ordered list of items (R19), read through `Execute`, `Fetch`,
`Get` and `ColumnNames` and as properties of the control (R20, R21) — and PartiQL
as a binding source for every bound control (R31-R35). **It contains no "Scalar"
or "multi-field" query API** (its only "scalar" is the Knob, Gauge or Switch
target). The draft's dependency on those additions has nothing to point at yet
(§9 Q10).

**The hosts.** A language change reaches four consumers: `rcrun run-form`,
embedded child forms, the compiled binary (`run_form_app` in `cobolt-compiler`)
and, on 2.x.x, the WebAssembly generator (`cobolt-wasm`, with `cobolt-wasm-rt`
committed as `assets/rt.wasm`). `cobolt-decimal` and `cobolt-numedit` are shared
source; the rest of the runtime is held equal to the interpreter by measurement —
the NIST NC, IF and IC runs, `crates/cobolt-wasm/tests/corpus.rs` and the
differential tests — and a difference that cannot be removed is a row of
`specs/WASM-PARITY.md` (`specs/steering/tech.md`).

## 3. Goals / Non-goals

### Goals

- Use COBOL-2002 FUNCTION-ID definitions and standard function invocation.
- Add interpolated strings and explicit locale-aware formatting.
- Add FORMAT IS and LOCALE IS clauses to PIC-bearing item declarations.
- Add collection transformations and aggregates, interoperable with OCCURS.
- Add Descendants and bulk shared-property operations on control collections.
- Iterate collections with `PERFORM VARYING element THROUGH collection`, the
  operator's spelling (new syntax, §2).
- Emit only necessary control storage and infrastructure, using existing runtime
  services where they can replace generator-authored COBOL plumbing safely.
- Add declarative layout and animation sequences through the existing engines.
- Bind grids, charts and other supported controls directly to typed query results.
- Measure reduced generated source while preserving application behavior and all
  existing event/procedure entry points.

### Non-goals

- A new `FOR EACH`, Python-style function declaration, or ad hoc RETURN statement.
- Scalar and multi-field SQL APIs, PartiQL implementation or changes to Spec 108.
- Direct event-to-procedure bindings, compact event-handler syntax, removal of
  existing nested handler programs or replacement of existing event dispatch.
  Existing CALL/PERFORM targets, handler signatures and ordering must keep
  working.
- Automatic migration of imperative layout, animation timers or user handlers.
  Declarative features are additive; existing programs do not need to adopt them.
- A general implementation of every COBOL-2002 feature, external language FFI,
  arbitrary code evaluation inside format strings, or new runtime dependencies
  without a demonstrated need.
- Rewriting user-authored handlers automatically on regeneration.
- Reducing LOC by stripping documentation, compressing unrelated statements onto
  one line, or changing field widths, rounding, Unicode behavior or side effects.

## 4. User stories

- As a COBOL developer, I want named functions usable in expressions so callers
  need no shared scratch argument and return variables.
- As an application author, I want a locale variable to control numeric display
  without changing stored business values or rebuilding the application.
- As a form developer, I want operations over tables and descendant controls so
  I do not enumerate occurrences or controls manually.
- As a maintainer, I want generated COBOL to contain these concise constructs and
  remain debuggable across interpreted, compiled and WebAssembly execution.

## 5. The language surface (proposed)

Except for the standard FUNCTION-ID structure and the operator-specified PERFORM
iteration form, the examples below define proposed RustCOBOL extensions. They must
not be documented as supported until implementation and tests land. §2 says which
of their words already exist.

### 5.1 COBOL-2002 user-defined functions

```cobol
       IDENTIFICATION DIVISION.
       FUNCTION-ID. DOUBLE-AMOUNT.
       DATA DIVISION.
       LINKAGE SECTION.
       01 ARG-AMOUNT PIC S9(13)V99.
       01 RESULT-AMOUNT PIC S9(13)V99.
       PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT
           RETURNING RESULT-AMOUNT.
           COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
           GOBACK.
       END FUNCTION DOUBLE-AMOUNT.
```

Invocation uses `FUNCTION DOUBLE-AMOUNT(WS-AMOUNT)` as an expression, including
inside SET, IF, arithmetic, interpolation and method arguments — the existing
`FUNCTION name(args)` form (§2), now also reaching a user function. Function names
are made visible through the standard REPOSITORY mechanism. Definitions are
source-unit siblings, not nested PROGRAM-ID handlers relabeled as functions. The
generator emits required declarations and definitions once per applicable source
unit. A COBOL function's result is described by its RETURNING item; no untyped
Ruby/Python-style return convention is introduced.

### 5.2 Formatting, interpolation and PIC clauses

```cobol
       01 WS-LOCALE PIC X(16) VALUE "pt-BR".
       01 WS-ASSETS PIC S9(13)V99
           FORMAT IS "currency-short" LOCALE IS WS-LOCALE.
       01 WS-COUNT PIC 9(9)
           FORMAT IS "number" LOCALE IS "pt-BR".

           SET LBL-KPI-V::Caption TO WS-ASSETS::Format()
           SET LBL-PAGE::Caption TO
               F"Página {WS-PAGE:number} de {WS-PAGES:number}"
           SET LBL-KPI-C::Caption TO
               F"{WS-COUNT} fundo(s) | {WS-ASSETS}"
           SET LBL-TOTAL::Caption TO
               WS-ASSETS::Format("currency-short", WS-LOCALE)
```

FORMAT and LOCALE are reserved clause words in this declaration grammar only
(R28); optional IS is accepted. The canonical spelling is `currency-short` (the
operator's `currency-shot` is treated as a typographical spelling, not a second
style). Formats initially include number, currency, compact, currency-short and
percent. Format names are case-insensitive in PIC FORMAT clauses, ::Format()
arguments and interpolation options: `currency-short`, `CURRENCY-SHORT` and
`Currency-Short` identify the same format. Validation and lookup shall apply the
same rule without altering source text or stored values. Currency is not
determined by locale alone: an explicit currency code or an explicit application
default is required; examples for PowerAnalytics use BRL. The plan must fix the
currency-option spelling before implementation. *(The `currency-short` examples
above carry no currency: by that rule it comes from an application default, and
§9 Q2 asks how one is declared.)*

LOCALE accepts a string literal or a reference to an alphanumeric data item,
including a qualified item. A referenced variable is resolved in the
declaration's scope and evaluated on each formatting operation, not frozen at
load time. Changing WS-LOCALE affects subsequent displays without changing
WS-ASSETS. Locale values are case-insensitive: `pt-BR`, `PT-br` and `pt-br`
identify the same locale, whether supplied as literals or variable contents.
Validation, lookup and formatting shall use the same case-insensitive matching
rule without modifying the source literal or the locale variable's stored value.
The trailing spaces of a PIC X locale item are not part of the tag: `WS-LOCALE`
above holds `pt-BR` and eleven spaces, and means `pt-BR`.

Formatting precedence: explicit method/interpolation options, then the source
item's FORMAT/LOCALE metadata, then deterministic application defaults. Formatting
metadata is applied by Format(), interpolation and textual output contexts such as
DISPLAY and STRING. Numeric expressions, comparisons and numeric-to-numeric
assignment remain numeric and never see a localized intermediate string. MOVE
moves the item's bytes as it always has, and a group item is its bytes: FORMAT is
applied only when the elementary item itself is the operand of one of the
contexts above. Text receivers retain existing PIC overflow/truncation rules and
diagnostics. A property assignment needing display formatting should use Format()
or interpolation explicitly; no method-specific implicit formatting is introduced.

`::Format()` is a method of data items; a control's own `Format` property (the
DateTimePicker's, the Viewer's — §2) keeps its meaning. An interpolated string is
text, not a query: building SQL from it does no escaping, and SQL keeps using host
variables or `?` parameters.

### 5.3 Collection methods and PERFORM iteration

```cobol
           SET CLEAN-NAMES TO RAW-TEXT::Split(X"0A")
               ::Map(ITEM => ITEM::Trim())
               ::Filter(ITEM => ITEM <> "")
           SET CSV-TEXT TO CLEAN-NAMES::Join(",")
           SET TOTAL TO AMOUNTS::Sum()
           SET LARGEST TO AMOUNTS::Max()
           SET TOP-ROWS TO ROWS::SortByDescending("assets")::Take(4)

           PERFORM VARYING ELEMENT THROUGH CLEAN-NAMES
               DISPLAY ELEMENT
           END-PERFORM
```

Map and Filter use expression-only lambdas with a lexical element parameter; the
arrow `=>` is a new token (§2). This is not a new statement/function-definition
language. They may invoke ordinary FUNCTION-ID functions and existing value
methods. Lambdas do not allow assignment, I/O or control mutation; their
diagnostic rules prevent hidden side effects. Item type and collection result type
are inferred and checked. `Join` and the collection value itself are new (§2):
`Split` keeps answering one part, subscripted or not, wherever the context is not a
collection (R38).

Collection results may be temporary expression values or assigned to compatible
OCCURS tables. Iteration uses the operator's PERFORM spelling, not FOR EACH. The
construct is new syntax (§2) and is not COBOL-2002 standard; it leaves
`PERFORM VARYING id FROM … BY … UNTIL …`, `PERFORM … THRU …` and `EXIT PERFORM
[CYCLE]` as they are (R16), and no parallel loop engine is added — the new form
runs through the interpreter's one PERFORM.

### 5.4 Descendants and shared property operations

```cobol
           PERFORM VARYING CONTROL THROUGH C-KPI::Descendants()
               CONTROL::PlayAnimation("intro")
           END-PERFORM

           ME::Controls("BTN-M1", "BTN-M2", "BTN-M3")::SetProperties(
               BackgroundColor: "#FFFFFF",
               ForegroundColor: "#1B2A4E")
```

*(`ME` is the form's handle; the draft wrote `FORM::`, which does not exist, §2.
The `Name: value` arguments are a form the language does not have; §9 Q11
recommends name/value pairs.)*

Descendants follows actual parent membership, not geometric overlap. It excludes
the root and includes visual descendants, including hidden controls; nonvisual
components are not descendants. Traversal is stable depth-first preorder using the
form's persisted sibling order. A returned collection snapshots membership but
carries references to live controls: property/method writes target controls, not
detached copies. Collection syntax must not be mistaken for PIC X methods.

Controls(id...) preserves argument order, rejects missing IDs and duplicates, and
validates membership in the current form instance. SetProperties evaluates its
values once and validates all targets/property types before writing; an invalid
target causes no partial write. Successful writes follow selection order, preserve
normal property notifications and use existing runtime/UI channels. This is
synchronous validation plus application, not a new cross-thread transaction or a
promise to undo external event-handler side effects. No implicit animation cascade
is required; explicit PERFORM preserves the author's selected behavior.

### 5.5 Selective control storage and runtime infrastructure

The runtime already owns control state described by the form. New generator output
should avoid redundant WS groups where no authored code or supported runtime
contract requires them. This is dependency-aware emission, not blanket deletion of
WS-<control> declarations. Preserve referenced groups/fields, PIC conversion and
initialization, GLOBAL visibility, group moves, REDEFINES, BY REFERENCE usage,
bindings and debugger-visible values. Where both a legacy WS item and direct
property access exist, preserve their established behavior; do not silently alias
independent values or split an established shared value.

Unknown/dynamic references require conservative retention. Old generated helper
paragraph names are compatibility entry points when reachable, including from
Common Code and nested programs. Runtime services replace only generator-owned
plumbing and must preserve side effects, status/error results and call order. Do
not hide the same COBOL expansion in COPY/include files and count it as saved. The
resulting `.cbl` should contain a concise high-level invocation/configuration.

### 5.6 Declarative layout through the existing engine

The following is a requirements sketch; finalize grammar in /plan:

```cobol
           LAYOUT LV-DROP
               BELOW FILT GAP 8
               MARGIN HORIZONTAL 24
               COLUMNS WHEN COMPACT 1 OTHERWISE 2
               GAP 14
           END-LAYOUT
```

Persist relationships in the form model and execute them through the existing
shared layout engine (`cobolt_forms::layout::solve`, §2). Do not build a second
coordinate solver. Define parent coordinate space, content/border bounds, gaps,
breakpoint selection (the default breakpoints are Compact, Medium and Expanded),
rounding, minimum/maximum sizes and precedence against existing anchors/layout
rules. Detect cycles and conflicting rules with control/source diagnostics.
Recompute only on relevant changes and never resize the application window
autonomously. Imperative property assignments and existing forms retain their old
semantics; explicit opt-in and documented ownership/override rules prevent
competing writers. The generator emits the declarative construct, not per-control
COMPUTE/SET lists.

### 5.7 Declarative animation sequences

Requirements sketch, with timing and trigger grammar to settle in /plan:

```cobol
           ANIMATION-SEQUENCE DASHBOARD-INTRO
               SHOW C-KPI AFTER 0
               SHOW C-BAR AFTER 50
               SHOW C-RADAR AFTER 100
           END-SEQUENCE
```

AFTER values are nonnegative milliseconds relative to sequence start, not to
completion of the previous entry. Entries with equal times follow declaration
order. The plan must distinguish visibility changes from named animation playback:
SHOW alone must not invent a PlayAnimation call. Sequences use the existing
animation/timer scheduler; define explicit start, cancel, restart, completion and
form-close cleanup without blocking the UI or firing stale work against a
closed/replaced form instance. Multiple concurrent form instances are isolated.
Existing timers and animation events keep working unchanged. Sequence adoption is
explicit, not a regeneration-time replacement of a user's STAGE-TICK. The
generator emits the sequence and invocation, not its timer state machine.

### 5.8 Direct result-to-control binding

Extend existing form data bindings (`DataBindingDef`, §2) so grids, lists, charts
and repeating groups consume typed results directly, including the result
contracts supplied by Spec 108 when available. Persist source/query references,
parameters, field mappings, ordering, refresh triggers and formatting options in
the existing binding model. This spec owns result consumption and concise
generated binding declarations; Spec 108 continues to own PartiQL execution, and
the Scalar and multi-field query APIs are owned by whichever spec §9 Q10 names.

Eliminate generator-authored fetch/unstring/convert/add-row/add-point loops for
supported bindings. Preserve numeric scale, null/empty distinction, row identity,
selected items, ordering and the established error/notification behavior. Define
refresh evaluation, result ownership/lifetime and empty/error behavior; evaluate
parameters once per refresh. Use existing paging/windowing and virtualization
rather than materialize unbounded result sets into the UI. Manual population and
legacy binding APIs remain supported and are not converted automatically. The plan
must define precedence when an opted-in binding and explicit user writes both
target a control. Source changes or failures must not leave mixed old/new rows. The
published COBOL emits the high-level binding/refresh operation directly.

## 6. Requirements (EARS)

- **R1 — Ubiquitous:** The system shall support FUNCTION-ID, LINKAGE parameters,
  PROCEDURE DIVISION USING/RETURNING, GOBACK and END FUNCTION using COBOL-2002
  function semantics, with documented standard visibility and passing rules.
- **R2 — Event:** When invoking a user function in an expression, the system shall
  validate its visibility, arity and argument/result categories, evaluate each
  argument once, and preserve declared sign, precision, scale and length.
- **R3 — State:** While functions call other functions or recurse, the system
  shall isolate activation state and return values according to the supported
  COBOL storage semantics, without global argument/result mailboxes.
- **R4 — Event:** When parsing an F-prefixed literal, the system shall recognize
  embedded RustCOBOL expressions, explicit format options and escaped braces
  `{{`/`}}`, and report unmatched braces and invalid expressions with source
  spans. A format option is introduced by a single `:`; a `::` is always the member
  operator, so `{WS-A::Trim():number}` is the expression `WS-A::Trim()` with the
  option `number`. A string literal inside an expression uses the other quote
  character or the doubled quote, and a brace inside it is text.
- **R5 — Constraint:** The system shall evaluate interpolation expressions once,
  left to right, without executing constructed source text or SQL.
- **R6 — Ubiquitous:** The system shall support ::Format() and FORMAT [IS] /
  LOCALE [IS] clauses on supported PIC-bearing elementary numeric items. Format
  names shall be validated and resolved case-insensitively in declarations, method
  arguments and interpolation options, without altering source text or stored
  values. `::Format()` is a method of data items; a control's `Format` property
  keeps its meaning.
- **R7 — Event:** When LOCALE contains a literal, the system shall validate it;
  when LOCALE references an alphanumeric item, the system shall resolve that
  reference and read its current value on every formatting operation, ignoring the
  item's trailing spaces. Locale validation and lookup shall be case-insensitive
  for both operand forms; case-only differences shall produce identical formatting
  without mutating the locale operand.
- **R8 — Constraint:** The system shall diagnose unknown locale variables,
  nonalphanumeric locale operands, invalid format names, malformed/unsupported
  locale tags and invalid currency settings rather than silently use the host OS.
- **R9 — Ubiquitous:** The system shall define deterministic formatting defaults,
  option precedence, currency code, decimal rounding, negative/zero behavior,
  percent input convention and compact suffix thresholds identically across hosts
  (R23).
- **R10 — Constraint:** Formatting shall not mutate underlying numeric values or
  change arithmetic, comparison, storage, serialization or reference-modification
  semantics. MOVE, group items, file and SQL input/output and REDEFINES move
  bytes and never format. Existing unformatted PIC declarations shall retain their
  behavior.
- **R11 — Ubiquitous:** The system shall provide Join, Map, Filter, Sum, Max,
  SortByDescending and Take, and shall let Split produce a collection where a
  collection is expected (R38).
- **R12 — Event:** When applying Map/Filter, the system shall bind the lexical
  element parameter, type-check the expression and return an ordered collection
  without changing the input collection. Sort shall be stable and nonmutating.
- **R13 — State:** While operating on OCCURS, methods shall observe the declared
  fixed extent or current OCCURS DEPENDING ON extent, not guess occupancy from
  blank or zero values. An explicitly selected slice limits the extent.
- **R14 — Event:** When assigning a collection into a table, the system shall check
  element compatibility and capacity, diagnose overflow instead of silently
  dropping rows, and initialize unused target occurrences deterministically.
- **R15 — Constraint:** The system shall specify empty collection results (Sum =
  numeric zero; Max raises a recoverable empty-collection error — §9 Q12 asks what
  recoverable means for an expression), reject invalid negative Take counts, and
  preserve fixed-width element conversion rules.
- **R16 — Ubiquitous:** The system shall define `PERFORM VARYING element THROUGH
  collection … END-PERFORM` as the one iteration form for every collection this
  feature emits, preserving the exit and nesting behavior of the other inline
  PERFORM forms and leaving `PERFORM VARYING … FROM … BY … UNTIL …`, `PERFORM …
  THRU …` and `EXIT PERFORM [CYCLE]` unchanged; no FOR EACH is introduced.
- **R17 — Event:** When Descendants is requested, the system shall return the
  ordered, root-excluding, instance-specific control collection defined in §5.4.
- **R18 — Event:** When bulk shared properties are applied, the system shall
  validate selectors, evaluate values once and apply the §5.4 notification and
  no-partial-validation-failure contract through existing form channels.
- **R19 — Constraint:** Collection/control operations shall preserve existing
  parent membership, hidden-state handling, DataGrid identity, border geometry,
  styling semantics and bound-event behavior unless explicitly changed by code.
- **R20 — Event:** When generating a form using these capabilities, the generator
  shall emit FUNCTION calls/definitions, formatting, interpolation, collection
  methods and PERFORM-through constructs directly where applicable. It shall not
  expand them into equivalent COBOL plumbing in the emitted `.cbl`.
- **R21 — Constraint:** Internal AST, Rust or WASM lowering may use temporaries or
  loops; R20 governs the published COBOL artifact, not machine implementation.
- **R22 — Constraint:** Regeneration shall preserve authored code verbatim except
  existing explicitly authorized transformations, retain the generated banner, omit
  unused helpers, and produce deterministic source and accurate source maps.
- **R23 — Ubiquitous:** The same semantics shall work in CLI execution, live and
  embedded forms, compiled binaries, and WASM/Web hosts where applicable. The
  formatting rules and locale data of R9 shall be shared source for the
  interpreter and the WebAssembly runtime (as `cobolt-decimal` and `cobolt-numedit`
  are), or held equal by differential tests; any difference goes to
  `specs/WASM-PARITY.md` with who is right and is never accepted silently. Any
  unsupported host capability — today the WebAssembly generator refuses `FUNCTION`
  calls and nested programs (§2) — shall be diagnosed at build time naming the
  construct, never a silent no-op.
- **R24 — Event:** When checking/debugging these constructs, the IDE shall offer
  syntax support, completion, diagnostics and source-site mapping (the editor
  already lists the word `FUNCTION-ID` and nothing of its construct, §2); stepping
  into a function or interpolation expression shall identify the original code
  site.
- **R25 — Ubiquitous:** Changes shall remain compatible with Rust 1.92 (the
  workspace MSRV, `Cargo.toml:52`), workspace resolver 2 and existing project/form
  persistence; no new foreign code ships.
- **R26 — Constraint:** This work shall not implement or duplicate Spec 108's
  PartiQL execution or its result-set API, nor define the Scalar and multi-field
  query APIs the operator assigned elsewhere (§2, §9 Q10). It shall consume the
  result contracts as they stand in Spec 108 (R19-R21, R31-R35) and, once they
  land, the Scalar and multi-field additions, with tests separated by dependency.
- **R27 — Event:** When implementing this spec, the project shall update, in the
  same change and by the rules of the line it lands on (§8): on 1.x.x the English
  Developer's Guide (GOLDEN RULE #3), deleting its five translations (GOLDEN RULE
  #8); on 2.x.x nothing under `README.md` or `docs/`, the text being kept in
  `specs/` until the line is released; on both, the IDE's `Tr` strings in all six
  languages, the `cobolt-compiler` reference tables and the regenerated
  `assets/knowledge/chunked.data`.
- **R28 — Constraint:** FORMAT and LOCALE shall be recognised only where a data
  description expects a clause, and shall not become reserved words: a program that
  uses either as a data name compiles and runs unchanged (R35). The corpus scan of
  §2 found no collision; user code beyond the corpus is the reason for the rule. A
  FORMAT or LOCALE clause shall never reach the parser's skip-with-a-warning path
  (§2): a valid one is applied, an invalid one is an error (R8).
- **R29 — Event:** When generating control storage, the generator shall omit
  redundant unreferenced backing items only after checking authored/project code
  and runtime contracts; uncertain dependencies shall cause conservative retention.
  Referenced legacy items shall preserve names, layout, scope and observable
  behavior.
- **R30 — Event:** When runtime services can replace generator-owned
  infrastructure, the generator shall emit concise invocations while retaining
  reachable legacy helper entry points and their status, lifecycle and side-effect
  contracts.
- **R31 — Constraint:** The system shall preserve existing event dispatch, nested
  handlers, procedure entry points, signatures, CALL/PERFORM behavior and
  bound-event ordering. No direct event-to-procedure binding replacement is
  introduced.
- **R32 — Event:** When a form explicitly opts into declarative layout, the system
  shall persist and execute its rules through the existing engine, diagnose cycles
  and conflicts, and preserve §5.6 geometry, breakpoint and window-size
  constraints.
- **R33 — Event:** When a declarative animation sequence starts, the existing
  scheduler shall execute its entries with §5.7 timing/order and instance
  isolation; cancel/restart/close shall prevent stale scheduled actions. Legacy
  timers remain valid.
- **R34 — Event:** When a control uses direct result binding, the existing binding
  engine shall apply §5.8 typed mapping, paging, identity and refresh semantics;
  the generator shall emit a high-level binding operation without row-conversion
  loops.
- **R35 — Constraint:** New persisted features shall be additive and opt-in; old
  `.cfrm`, project files and user-authored code shall load, regenerate and execute
  without migration. Declarative adoption shall not silently rewrite existing
  logic.
- **R36 — Event:** When measuring the expanded optimization, the system shall
  report progress toward 4,344 total PowerAnalytics lines against a frozen original
  form/code/asset baseline, with behavioral parity and legacy compatibility
  evidence. The baseline is identified by content hash, git blob and generator
  commit (§2), not by path: the generated file is git-ignored and moves with every
  regeneration. A gap shall be reported honestly, not filled by deleting comments,
  reducing the workload, hiding expanded COBOL elsewhere or disabling legacy code.
- **R37 — Constraint** *(added by the review)*: The system shall list each method
  this spec adds in the closed method vocabulary (`cobolt-ast` `methods`) under
  one canonical spelling and no older-spelling alias (the operator's ruling of
  2026-10-10), so that a call is never read as a subscript; the vocabulary's own
  test shall fail for a dispatched method that is not listed (§2).
- **R38 — Constraint** *(added by the review)*: The system shall keep
  `X::Split(sep)(n)` answering part n, and the unsubscripted form answering part 1,
  wherever the context is not a collection. `Split` yields a collection only as the
  receiver of a collection method, the source of PERFORM … THROUGH, or the source
  of an assignment to a declared table (R14); an assignment to an undeclared item
  keeps today's meaning.

## 7. Acceptance criteria

- [ ] **AC1 (R1–R3):** Standard-shaped standalone and project FUNCTION-ID units
  parse, check and execute; numeric/text returns, nested calls, expression
  arguments, BY VALUE/BY REFERENCE, visibility and error cases are covered.
  Standard parameter/storage rules are verified against authoritative references.
- [ ] **AC2 (R4–R5):** Interpolation covers braces, quotes, Unicode, nested method
  expressions and explicit formats; `{WS-A::Trim():number}` reads as the expression
  `WS-A::Trim()` with the option `number`; a quote and a brace inside a nested
  string literal behave as R4 says. A counter proves one evaluation per expression
  in left-to-right order. Malformed strings identify the originating expression.
- [ ] **AC3 (R6–R10, R28):** PIC declarations cover literal and variable LOCALE,
  qualified references, changed locale between two displays, lexical scope,
  equivalent `pt-BR`/`PT-br`/`pt-br` literal and variable values with identical
  output and unchanged variable contents, a locale item whose trailing spaces differ,
  invalid locales/types, and a fixture declaring `01 FORMAT PIC X.` and
  `01 LOCALE PIC X.` that compiles and runs unchanged. `currency-short` /
  `CURRENCY-SHORT` / `Currency-Short` shall produce identical output in PIC FORMAT
  clauses, ::Format() arguments and interpolation options, without changing source
  text or stored values. A control's `Format` property (DateTimePicker, Viewer)
  reads and writes as before.
- [ ] **AC4 (R6–R10):** Fixed expected outputs cover pt-BR/en-US/fr-FR, explicit
  BRL currency, signs, zero, rounding boundaries, compact thresholds and percent
  convention. Input numeric bytes/values remain unchanged after formatting.
- [ ] **AC5 (R11–R15, R38):** Map/Filter/Join chaining, table slices, fixed/ODO
  extents, stable equal-key sorting, empty inputs, bad types and capacity overflow
  pass; blank and zero entries are not silently discarded; `X::Split(",")(2)` and
  the unsubscripted `Split` behave as before, and the existing tests of both stay
  green.
- [ ] **AC6 (R16):** Collections from §5.3 and §5.4 and OCCURS tables iterate
  through the same PERFORM syntax; tests cover nested loops, early exit and
  evaluation count, and the three old PERFORM forms and `EXIT PERFORM [CYCLE]`
  behave as before.
- [ ] **AC7 (R17–R19):** Descendants respects true parents, sibling order, hidden
  controls, repeated-group instances and root exclusion. A visually overlapping
  sibling is excluded. Live method/property writes reach the correct instance.
- [ ] **AC8 (R18–R19):** Bulk styling covers valid mixed controls, duplicate/missing
  IDs, unsupported properties, invalid types, bound notifications and a target
  validation error with zero writes. The DataGrid binding and write-back tests
  (specs 094, 095), `engine_reference_form_parity_static_vs_faces`,
  `cargo test -p cobolt-forms --features render` and the `rounded-corners` skill's
  checks stay green and any code touched in those areas is reviewed against them,
  with automated or visual evidence, before implementation completion.
- [ ] **AC9 (R20–R22):** Golden generator fixtures contain the high-level syntax
  itself, contain no legacy expansion for those operations, preserve authored
  code/banner, regenerate identically and parse/check with zero new diagnostics.
- [ ] **AC10 (R20–R23):** Equivalent PowerAnalytics workloads are generated from
  original and updated form inputs, then compared for totals, selected filters,
  query parameters, chart data, grid rows/order, displayed text and animation
  invocation order. Source maps are regenerated and verified after line changes.
- [ ] **AC11 (R20–R22, R36):** Report total/nonblank/executable/generated-only/
  user-code LOC and bytes per fixture and per construct, with the baseline
  identified by hash, blob and generator commit and the outer/handler split of §2
  re-measured. Each construct's representative workload has fewer
  executable/scaffolding lines than its plumbing equivalent; comment or whitespace
  removal alone cannot satisfy this criterion. Report real aggregate savings; no
  unmeasured percentage is a release claim.
- [ ] **AC12 (R23, R25):** Differential cases pass interpreted, compiled and
  relevant WASM paths, the formatter's output identical on every host or the
  difference a `WASM-PARITY` row; debugger step/breakpoint/watch behavior and
  serialization tests pass; the build on Rust 1.92 adds no dependency (the
  `Cargo.lock` diff is empty or justified).
- [ ] **AC13 (R26):** No file under `specs/108-partiql-indexed-files/` is changed
  by the implementation of this spec; dependent tests name the Spec 108 result API
  they use and report unavailable dependencies explicitly.
- [ ] **AC14 (R27):** The Guide (on 1.x.x), the six-language `Tr` strings, the
  `cobolt-compiler` reference tables and a regenerated `chunked.data` describe
  exactly the implemented features; `prebuilt_chunked_kb_matches_the_published_documentation`
  is green; on 2.x.x no file under `README.md` or `docs/` is touched.
- [ ] **AC15 (R29–R30, R35):** Golden fixtures show omitted storage/helpers only
  where safe. Legacy cases cover whole-group MOVE, referenced PIC fields, GLOBAL,
  REDEFINES, reference arguments, Common Code, dynamic references, binding and
  debugger watches; each retains its old behavior and reachable names.
- [ ] **AC16 (R31, R35):** Existing event/procedure regression suites and original
  unmodified example forms pass. Verify all old handler names/signatures, lifecycle
  order, CALL/PERFORM entry points, event bindings, status outputs and exceptions.
  No test requires converting a user handler into a direct event binding.
- [ ] **AC17 (R32, R35):** Layout fixtures cover Compact/Medium/Expanded (the
  default breakpoints) and their boundary widths 599/600 and 1023/1024,
  nested/repeating parents, explicit user overrides, fractional rounding,
  minimum/maximum sizes and resize repetition. Cycles/conflicts diagnose cleanly;
  equivalent layouts match measured geometry and windows never resize themselves.
- [ ] **AC18 (R33, R35):** A deterministic clock tests absolute offsets, equal-time
  order, cancellation, restart, simultaneous forms and close-before-completion.
  Compare visibility and animation-call traces with the imperative workload;
  existing timers/events retain their original trace and behavior.
- [ ] **AC19 (R34–R35, R26):** Binding comparisons cover grid/chart/list/repeating
  groups, nulls, decimals, empty results, selections/identity, sort order, paging,
  refresh parameters and failures. No mixed partial result appears; manual data
  population remains valid. Spec 108 execution APIs are reused, not duplicated.
- [ ] **AC20 (R20–R23, R29–R36):** Generator goldens contain direct layout,
  sequence, runtime-service and binding constructs, not expanded COBOL. Publish
  non-overlapping per-category savings and a paired application validation report.
  If the 50% study target is missed, state remaining LOC, cause and residual
  opportunities without claiming 50% achieved. All legacy compatibility gates must
  pass regardless of the measured reduction.
- [ ] **AC21 (R24)** *(added by the review)*: The editor highlights, completes and
  documents the new constructs; a diagnostic carries its source span; stepping into
  a function or an interpolation expression stops at the original code site; the
  source map is rebuilt after a line change.
- [ ] **AC22 (R37)** *(added by the review)*: Every method this spec adds is on the
  closed vocabulary under one spelling and no alias; removing any of them from the
  list turns the dispatch test red.
- [ ] **AC23 (§8)** *(added by the review)*: Every new COBOL test under
  `tests/cobol/**` ends with ONE result block (GOLDEN RULE #7); a credential scan of
  every staged diff finds none, and no example `.cfrm` is copied into a fixture
  unscanned.

## 8. Constraints & steering check

- **i18n (6 languages):** yes for what the IDE shows — the completion and hover
  text of the new constructs and any settings entry, each a `Tr` field with all six
  languages (`i18n.rs`, `ui_text_data.rs`). Format names, locale tags, COBOL names
  and the diagnostics of the lexer, parser and analyser stay as every existing
  diagnostic of those crates is: English text.
- **Generated-code / regenerate contract:** the generator emits these constructs in
  its published `.cbl` (R20), keeps the developer banner, omits unused helpers and
  regenerates identically (R22); authored code is preserved verbatim, and source
  maps are rebuilt after every line change (AC10, AC21).
- **Form file:** additive only. Layout relationships, sequences and result bindings
  are new opt-in persisted entries (serde default); old `.cfrm`, `.wcfrm`, `.mcfrm`
  and project files load, regenerate and run unchanged (R35).
- **The closed method vocabulary:** every new method is listed (R37, AC22). Without
  it `X::Join(",")` is read as a subscript and does nothing (§2).
- **Four hosts:** `rcrun run-form`, embedded child forms and `run_form_app` in the
  compiled binary — the `interpreter-binary-parity` skill — and, on 2.x.x, the
  WebAssembly generator, whose `rt.wasm` is rebuilt with
  `cargo run -p cobolt-wasm --example build_rt` and whose differences are
  `WASM-PARITY` rows (R23, AC12). GOLDEN RULE #9's gate — the census and every
  finished module's execution pass — is the check that new tokens (`F"`, `=>`) and
  clauses change nothing for a COBOL-85 program.
- **System KB:** the `cobolt-compiler` doc tables change (R27), so
  `assets/knowledge/chunked.data` is regenerated in the same change with
  `cargo run -p cobolt-ide --example build_chunked_kb`; a red
  `prebuilt_chunked_kb_matches_the_published_documentation` is real.
- **Docs, by line:** on 1.x.x the Developer's Guide
  (`docs/developers-guide-en.md`; audience COBOL developers from Fujitsu PowerCOBOL
  and Veryant isCOBOL; COBOL and prose, never Rust) is updated in the same change
  as each slice and its five translations are deleted (GOLDEN RULES #3 and #8). On
  2.x.x nothing under `README.md` or `docs/` is touched: what the line says about
  itself waits in `specs/` until the operator releases it. *(`specs/steering/tech.md`
  and `structure.md` still say the translations are user-maintained and never
  edited; the operator's GOLDEN RULE #8 is the current rule and is applied here.
  Correcting the steering files is outside this spec.)*
- **Tests:** every new COBOL test under `tests/cobol/**` follows GOLDEN RULE #7 — ONE
  result block with the test name, the cases/forms/PICTUREs exercised, per-phase
  timings and throughput, and a pass/fail tally; real measurements only.
  `cargo test -p cobolt-forms --features render` stays green; sweeps use
  `--no-fail-fast` and every `test result:` line is read (`cobolt-ide` needs
  `--bin`).
- **Credentials:** the PowerAnalytics forms and other example `.cfrm` files may
  carry live keys. A fixture derived from an example is scanned before it is staged,
  and the baseline is pinned by hash, not copied into the repository (R36).
- **User code is sacred:** no developer code is deleted, rewritten or reordered by
  regeneration; migration of a handler to a new construct is an explicit act of the
  developer (R35, Non-goals).
- **Window size:** a layout rule never resizes a window (R32; the rule of `CLAUDE.md`
  — only the user's grip does).
- **Originality:** the Micro Focus and IBM pages in §10 were read as a map of
  FUNCTION-ID; no vendor wording or example is reproduced, and every example here is
  original (product steering, "Original work only").
- **Where the work lands (for /plan):** syntax in `cobolt-lexer`, `cobolt-parser`
  and `cobolt-ast`; signatures, types and locale references in `cobolt-semantic`;
  execution and formatting in `cobolt-runtime` and `cobolt-stdlib`; persisted
  declarations in `cobolt-forms`; direct emission in `cobolt-codegen`; function-unit
  discovery and the bundled execution in `cobolt-compiler` and `cobolt-cli`;
  authoring and source maps in `cobolt-ide` and `cobolt-dap`; host parity in
  `cobolt-wasm` and `cobolt-wasm-rt`. Existing structures and module boundaries are
  reused; no second engine is created.
- **Fix vs feature:** **feature.** `features` branch; `z` bump and a top
  `CHANGELOG.md` entry per change. A spec-only change bumps neither — the commit
  of spec 108's last clarification (`ef41f4f0`) touched only its `spec.md`. No
  precedent turns any part of this into a fix: none of it completes COBOL-85
  behaviour (`CLAUDE.md`, Classification precedents).

### Delivery slices (recommended, for /plan)

The scope is eight independent capabilities and a measurement. The slices keep each
change reviewable; each ships with its own golden fixtures, `z` bump and CHANGELOG
entry, and none weakens a requirement. The order is the plan's.

| Slice | What | Requirements | Acceptance | Depends on |
|-------|------|--------------|------------|------------|
| S1 | FUNCTION-ID | R1–R3 | AC1 | WebAssembly: the IF and IC modules (WASM-PARITY row 4) |
| S2 | Interpolation, `::Format()`, FORMAT / LOCALE | R4–R10, R28 | AC2–AC4 | the shared formatter source (R23) |
| S3 | Collections, PERFORM … THROUGH | R11–R16, R37, R38 | AC5, AC6, AC22 | — |
| S4 | Descendants, Controls, SetProperties | R17–R19 | AC7, AC8 | S3 (iteration) |
| S5 | Selective storage and runtime services | R29–R31 | AC15, AC16 | the dependency inventory (§9 Q9) |
| S6 | Declarative layout | R32 | AC17 | the layout solver (spec 056) |
| S7 | Animation sequences | R33 | AC18 | the animation/timer scheduler |
| S8 | Direct result binding | R34 | AC19 | Spec 108's result contracts |
| — | Generator, hosts, IDE, documentation, measurement | R20–R27, R35, R36 | AC9–AC14, AC20, AC21, AC23 | each slice |

## 9. Open questions

Nothing here has been put to the operator yet; each question carries a
recommendation, and `/clarify` is the next phase.

**Settled by the operator** (carried from the draft, 2026-10-28; not reopened):

- LOCALE accepts a string literal or a reference to an alphanumeric item, qualified
  or not, read on every formatting operation (§5.2, R7).
- Locale values and format names are case-insensitive, in literals and in
  variables, without altering either (R6, R7).
- The canonical spelling is `currency-short`; `currency-shot` is a typographical
  spelling and gets no alias.
- The scope includes selective control storage, runtime infrastructure, declarative
  layout, animation sequences and direct data binding (§5.5–§5.8); direct
  event-to-procedure binding and any replacement of event dispatch are excluded, and
  existing user code keeps working (R31, R35).
- Iteration is `PERFORM VARYING element THROUGH collection`; no `FOR EACH` (R16).
- Functions are COBOL-2002 FUNCTION-ID units, not a Python-style declaration (§5.1).

**Open:**

- **Q1 — Which line builds it** (an assumption from precedent). *Recommendation:*
  as spec 108 does — `features` on 1.x.x, with the Guide and the translations by
  GOLDEN RULES #3 and #8; 2.x.x takes the language work by cherry-pick when the
  operator says so, and the WebAssembly generator's support is 2.x.x work of its own
  (each construct a `WASM-PARITY` row until it lands; `cobolt-wasm` does not exist
  on `main`). The alternative — `features-2x` — gives all four hosts in one line but
  leaves the Guide untouched until the line is released (§8).
- **Q2 — The format-option grammar** (the draft's decision 1): the full list of
  options (currency code, rounding, compact suffixes, precision, locale overrides)
  and the defaults. (a) *Percent:* ratio (0.425 shows as 42,5%) or already scaled
  (42.5). *Recommendation:* ratio, as the CLDR percent pattern multiplies by 100; a
  value already in percent is shown with `number` and a literal `%`. (b) *The
  application default currency:* the 2.x.x manifest has a default locale
  (`[project] locale`, §2) and no currency, and the `currency-short` examples of
  §5.2 carry none. *Recommendation:* a project-level default currency beside
  `locale`, and an explicit per-format override whose spelling the plan fixes. (c)
  *Locale data:* a closed built-in set to start (`pt-BR`, `en-US`, `fr-FR`, AC4),
  with the data's source and version pinned — the French group separator, for one,
  differs between CLDR releases — and identical on every host (R9, R23).
  (d) *`DECIMAL-POINT IS COMMA` and `CURRENCY SIGN`:* *Recommendation:* edited
  pictures keep them; FORMAT reads neither.
- **Q3 — The lambda** (decision 2): `ITEM => expression`, the new `=>` token, which
  calls are pure (user FUNCTIONs, R2, and the value methods), and how a failing
  element propagates. *Recommendation:* as written; a failing element fails the
  whole expression and names the element's position.
- **Q4 — The semantics of `PERFORM VARYING element THROUGH collection`** (decision
  3, now a decision rather than a reconciliation, §2): alias or copy of the element,
  mutation of the source inside the loop, when the collection is snapshotted, the
  element's type and lifetime, and nesting. *Recommendation:* the collection is
  snapshotted when the loop starts; the element names the occurrence of an OCCURS
  table, refers to the live control for a control collection (§5.4) and is a copy
  for a computed collection; it exists only inside the loop.
- **Q5 — FUNCTION-ID packaging and visibility** (decision 4): where function source
  lives, whether it is visible across forms, recursion (R3), and a user function
  that has the name of an intrinsic function. *Recommendation:* Common Code
  (`src/*.cbl`) as sibling units registered through REPOSITORY, visible
  project-wide; a name that is also an intrinsic function's is refused with a
  diagnostic naming both, never shadowed silently.
- **Q6 — Formatting contexts** (decision 5): DISPLAY, STRING, interpolation and
  `Format()` apply FORMAT; MOVE, group items, file and SQL I/O, reference
  modification, REDEFINES and serialization never do (R10). *Recommendation:* as
  written.
- **Q7 — Declarative layout** (decision 6): the grammar, the relationship
  coordinates, the precedence against anchors, dock, flex and grid, the ownership
  rule when COBOL also writes a control's geometry, and how a rule lowers on a Web or
  Mobile project, where layout is CSS (WASM-004 R8), or is refused there (R23).
  *Recommendation:* decide in /plan against the solver's own rules (spec 056 R9, R10:
  designed rectangles in, placements out, the design never written).
- **Q8 — Sequences and bindings** (decision 7): start/cancel/restart syntax, the
  scheduler's clock, the binding declaration's syntax, refresh triggers and the
  failure policy — and how `AFTER n` relates to the `delay_ms` an `AnimationDef`
  already carries (§2). *Recommendation:* decide in /plan; where an entry plays a
  named animation, its own `delay_ms` adds to the `AFTER`.
- **Q9 — The inventory** (decision 8): which backing items and helper paragraphs are
  public contracts, the dynamic-reference cases, and the frozen benchmark inputs
  (§2 gives the baseline's hashes; no dependency analysis exists yet, §2).
  *Recommendation:* the plan's first task is that inventory over PowerAnalytics and
  `examples/`, before any emission rule; anything not proven unused is kept.
- **Q10 — The Scalar and multi-field query APIs** (new; §2): Spec 108's text has
  none. Where do they live — an amendment to 108 or a spec of their own — and is
  §5.8's binding of SQL and Backend results blocked on them? *Recommendation:* amend
  108, which owns the result set, and let §5.8 begin with the PartiQL result set
  (108 R19–R21, R31–R35).
- **Q11 — `SetProperties` arguments** (new; §2): the draft writes
  `BackgroundColor: "#FFFFFF"`, a named-argument form the language does not have; it
  needs a new rule in every argument list and looks like reference modification
  `(a:b)`. *Recommendation:* name/value pairs —
  `SetProperties("BackgroundColor" "#FFFFFF" "ForegroundColor" "#1B2A4E")` — which
  add no syntax and read like `SetProperty(name value)`.
- **Q12 — What "recoverable" means for `Max` of an empty collection** (new; R15): an
  expression has no `ON EXCEPTION` phrase, and a value-method error today is a
  run-time error that stops the run (the way an unknown method does, §2). Options:
  (a) a run-time error naming the method; (b) an empty answer plus a status item the
  program can test. *Recommendation:* (a), which relaxes the draft's word
  "recoverable"; `Sum` of an empty collection stays zero either way.

**Adjustments the review made to the draft** (not asked; each has one reasonable
answer, written into the requirement named, and visible here so the operator can
overturn it):

- `PERFORM … THROUGH` is specified here as new syntax; the draft's "existing
  iteration contract" does not exist in the source (§2, R16, Q4).
- `Join` is added to the vocabulary and `Split` keeps its subscripted meaning
  outside a collection context (R11, R38).
- `FORMAT` and `LOCALE` are recognised by position and never reserved; R35 requires
  it, and the corpus scan found no collision (R28).
- `ME` is the form's handle; `FORM::` does not exist (§5.4).
- A locale item's trailing spaces are not part of the tag; the draft's own
  `PIC X(16)` example would fail validation otherwise (R7).
- A group item is its bytes, and MOVE never formats (R10, §5.2).
- The baseline is identified by hashes and generator commit, not by path, since the
  generated file is not in git (R36).
- The default breakpoints are Compact, Medium and Expanded, not "Wide" (AC17).
- The draft's "DataGrid and border guardians" are not agents in this repository; the
  gates are the DataGrid tests, the render parity guard, the `rounded-corners` skill
  and `cargo test -p cobolt-forms --features render` (AC8).
- Documentation follows the rules of the line (R27, §8): the draft's single
  requirement collided with 2.x.x's ban on touching `docs/`.
- Two requirements and three acceptance criteria are added for what the draft left
  untested or unstated: R37, R38, AC21, AC22, AC23.
- The draft's "Claude Code handoff" and its pointers at files outside the repository
  are removed; the evidence is kept in §10.

## 10. References and evidence

- Operator conversation, 2026-10-28: scope selections 1–5, dynamic/case-insensitive
  LOCALE and case-insensitive FORMAT; subsequent approval of the structural
  reduction recommendations except direct event-to-procedure bindings, explicitly
  requiring that existing user code continue to work.
- `specs/108-partiql-indexed-files/spec.md`: dependency boundary (§2, §9 Q10).
- Kept outside the repository by Codex, in `~/Documents/codegpt/claude/`:
  `main-form.optimization.md` (the measured baseline and the Unicode, length and
  SQL-escaping compatibility findings), `form-generator-investigation.md`,
  `main-form.optimized.cbl` and the first draft of this spec. The numbers they
  state were re-measured on 2026-10-28 (§2); the investigation's reading of
  `write_control_group` (then `lib.rs:933`) matches the source (now `:944`).
- [Micro Focus PROCEDURE DIVISION header](https://www.microfocus.com/documentation/reuze/60d/lhpdf602.htm):
  ISO2002-marked RETURNING/USING rules and parameter/return conformance.
- [IBM user-defined function structure](https://www.ibm.com/docs/en/cobol-zos/6.4.0?topic=structure-cobol-user-defined-function-definition):
  vendor corroboration of FUNCTION-ID source-unit structure, not a normative ISO
  standard. Full ISO compliance must be checked against the applicable standard
  during planning; this spec does not claim unrelated vendor extensions.
- In the repository, checked 2026-10-28: `specs/steering/*`, `specs/WASM-PARITY.md`,
  `specs/056-responsive-design`, `specs/091-form-layers-and-non-visual-deployment`
  and the source files named in §2.
- How §2's numbers were measured: lines with `wc -l`; hashes with `shasum -a 256`;
  a handler's span from its `PROGRAM-ID. name IS COMMON PROGRAM.` line to its
  `END PROGRAM name.` line; a control group as a `01 WS-x.` header followed by
  `05 WS-x-TEXT`; the collision scan as a hyphen-aware whole-word match over
  `tests/cobol`, `NIST/newcob.val,cbl` and the examples' COBOL, comments and quoted
  text excluded.

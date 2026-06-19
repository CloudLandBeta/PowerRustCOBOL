# Spec — Form module model & project organization

- **Status:** draft → approved
- **Folder:** specs/009-form-module-model/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-19

## 1. Overview

This spec **formalises the form-module model** of a PowerRustCOBOL application —
how a project is organised into form modules, how each form module is laid out as
a COBOL program, how control enters through the event loop, and how procedures and
data are scoped and shared. Much of the model is already implemented (the
nested-program codegen) and specified for shared data in
[005](../005-cobol-structure-shared-data/spec.md); this spec records the model as a
whole, marks what is already satisfied, and pins down the **gaps**: `IS COMMON` on
**every** woven procedure, `FD … IS GLOBAL`, cross-module `EXTERNAL` sharing,
procedure-local data privacy, the static-by-default lifecycle, **`INVOKE-FORM`**
(form invokes another form), and the Procedure-Division **"New" encapsulation +
`#INCLUDE`** authoring path. It also folds in one **Rust-FFI** carry-over from
spec 005's AC6: evaluating the inline `obj::method()` call as a **value operand**
(e.g. `DISPLAY S::len()`).

It is the Fujitsu **PowerCOBOL** form-module model, expressed for PowerRustCOBOL.

## 2. Goals / Non-goals

### Goals
- A single authoritative description of the **form-module ↔ COBOL-program**
  mapping and project organization (one program per form; 1..n forms per project).
- The **scoping & sharing contract**: form-level `GLOBAL`, run-unit `EXTERNAL`,
  procedure-local privacy, and the **static-by-default** procedure lifecycle.
- **`IS COMMON` on all woven procedures** (event *and* user procedures) so any
  procedure is callable from anywhere within the form module.
- A safe **Procedure-Division authoring path**: the **"New"** action encapsulates
  raw code into an embedded program with a unique `PROGRAM-ID`; **`#INCLUDE`**
  pulls in external embedded programs.
- **Inline `obj::method()` as a value operand** — evaluate the method-call
  expression inside `DISPLAY`/`MOVE`/`COMPUTE` (folded in from 005 AC6), so
  `DISPLAY S::len()` works (today `::` dispatches only as a statement).

### Non-goals
- Re-specifying the COBOL Structure editor, the GLOBAL/EXTERNAL run-unit store, or
  the Rust FFI — those are [005](../005-cobol-structure-shared-data/spec.md).
- The web/WASM target ([006](../006-web-wasm-projects/spec.md)) — though the model
  must remain portable to it.
- Authoring full COBOL-2002 OO in COBOL.

## 3. Statement assessment (traceability)

The model was given as a set of statements; each maps to a requirement and a
current status. **Satisfied** = already implemented and/or specified — no action.
**New** = this spec adds/sharpens the requirement.

| # | Statement (abridged) | Req | Status |
|---|----------------------|-----|--------|
| P1 | Each form ⇒ a separate COBOL program module; project has 1..n form modules | R1 | **Satisfied** (codegen: one `PROGRAM-ID` per form; project tracks forms) |
| P2 | Each form module has one general area for file/data + form-level COBOL declarations | R2 | **Satisfied** (005 COBOL Structure) |
| P3 | Each form module has an event loop; runtime feeds OS events; form branches to the event procedure; user procedures may be added | R3 | **Satisfied** (`COBOL-EVENT-LOOP` + `COBOL-WAIT-EVENT` + `EVALUATE` dispatch) |
| P4 | Procedures exist as embedded programs; `IS COMMON` is added to **each** | R4 | **New** (today only *user* procedures are `IS COMMON`; event handlers are not) |
| P5 | A form may declare `01`/`77` data `IS GLOBAL`, shared by its procedures | R5 | **Satisfied** (005 R6) |
| P6 | A form may declare an `FD … IS GLOBAL` | R6 | **New** (005 covers `01`/`77` GLOBAL + `FD` EXTERNAL; GLOBAL `FD` not explicit) |
| P7 | Data/`FD` shared across forms in the same executable via `IS EXTERNAL` (identical decls) | R7 | **Satisfied** (005 R7/R9 + run-unit store) |
| P8 | Data/`FD` shared across **separate** executables via `IS EXTERNAL` (identical decls) | R8 | **New** (005's store is single-process; cross-executable needs shared memory/IPC) |
| P9 | A procedure's own file/data are private; `IS GLOBAL` there shares nothing outside it | R9 | **New** (true by the nested model, not stated) |
| P10 | Procedures are **static** by default (state preserved across re-entry, not cancelled on exit); use `INITIALIZE` to reset | R10 | **New** (COBOL-85 default + `CANCEL` reset exist; not stated as a guarantee) |
| P11 | A form module may invoke another form module via **`INVOKE-FORM`** (same or separate executable) | R11 | **Deferred** (absent; not implemented now — recorded for a future spec) |
| P12 | The Procedure-Division **"New"** action encapsulates raw code into an embedded program (unique `PROGRAM-ID`); **`#INCLUDE`** copies in external embedded programs (each unique `PROGRAM-ID` + `END PROGRAM`) | R12 | **Deferred** (the "New" path already exists as Add-procedure; `#INCLUDE` + validation + IDE deferred — operator decision) |
| P13 | Inline `obj::method()` works as a **value operand** (`DISPLAY S::len()`) — folded from 005 AC6 | R16 | **New** (today `::` is statement-only; `INVOKE … RETURNING` is the wired path) |

## 4. User stories
- As a developer, I want each form to be its own COBOL program so I can reason
  about its data and procedures in isolation.
- As a developer, I want any procedure in a form to be callable from any other
  procedure in that form.
- As a developer, I want to open/run another form from a handler. *(deferred —
  see R11)*
- As a developer, I want to drop raw COBOL into a form safely — wrapped as a real
  embedded program — or pull in a library of embedded programs with `#INCLUDE`.
- As a developer, I want procedure state to persist between calls unless I
  explicitly `INITIALIZE` it.

## 5. Requirements (EARS)

**Module & project structure**
- **R1 (ubiquitous):** The system shall generate, for **each** form, a separate
  COBOL **program module** (`PROGRAM-ID` = the form's name); a project may contain
  **one or more** form modules. *(Satisfied.)*
- **R2 (ubiquitous):** Each form module shall have **one** form-level area for its
  file and data declarations plus COBOL-specific declarations (the COBOL
  Structure: `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE`,
  `WORKING-STORAGE`). *(Satisfied — see 005.)*

**Event loop & control flow**
- **R3 (ubiquitous):** Each form module's `PROCEDURE DIVISION` shall contain an
  **event loop**; the run-time system shall manage OS events and pass each into
  the appropriate form module, which shall evaluate the event and **branch to the
  matching event procedure**. User-specified procedures may also be present.
  *(Satisfied.)*

**Procedures as embedded programs**
- **R4 (ubiquitous):** Every procedure in a form — **event procedures and user
  procedures** — shall physically exist as an **embedded (nested) COBOL program**,
  and the generator shall add **`IS COMMON`** to each procedure's `PROGRAM-ID`, so
  any procedure is callable from anywhere within the form module. *(New: extends
  the current behaviour, where only user procedures are `IS COMMON`.)*

**Data & file scoping within a form**
- **R5:** A form may declare one or more `01`/`77` data items `IS GLOBAL`; such
  items shall be visible to **all** of that form's event/user procedures.
  *(Satisfied — 005 R6.)*
- **R6:** A form may declare one or more **`FD … IS GLOBAL`** file definitions;
  such files shall be visible to all of that form's event/user procedures. *(New —
  make GLOBAL `FD` explicit and supported in weaving + runtime visibility.)*

**Sharing across modules**
- **R7:** A `01`/`77` item or `FD` declared `IS EXTERNAL` shall be shared across
  **forms in the same executable**, provided each form declares it **identically**;
  the system backs this with the run-unit-wide store. *(Satisfied — 005 R7.)*
- **R8:** A `01`/`77` item or `FD` declared `IS EXTERNAL` shall be shareable across
  **separate executable application modules** when **identical** declarations
  (including `IS EXTERNAL`) exist in each. *(New — requires a cross-process shared
  backing; scope/mechanism is Open Question Q1.)*

**Procedure-local data**
- **R9 (constraint):** An individual event/user procedure may contain its own file
  and data definitions; an `IS GLOBAL` clause there shall **not** share those items
  outside that procedure. *(New — codify the nested-leaf privacy.)*

**Procedure lifecycle**
- **R10 (ubiquitous):** Event/user procedures shall be **static by default**:
  on re-entry their data is **not** auto-re-initialised, and on exit the procedure
  is **not** cancelled (state is preserved). To force re-initialisation, the
  developer uses the COBOL **`INITIALIZE`** verb. *(New — state the guarantee;
  `CANCEL` remains the explicit reset.)*

**Form invocation**
- **R11 (event) — DEFERRED, not implemented now.** When a form module executes
  **`INVOKE-FORM`**, the system shall invoke the named form module — within the
  same executable, and (subject to Q1) across a separate executable application
  module. *Recorded for completeness; carved out of the active scope at the
  operator's direction and to be planned/implemented in a later spec.*

**Procedure-Division authoring (safety)**
- **R12 (event):** When the developer adds procedural code via the Procedure
  Division editor's **"New"** action, the system shall **encapsulate** that code
  into an embedded program with a **unique `PROGRAM-ID`** (which becomes the user
  procedure's name) and a closing `END PROGRAM`. Raw procedural code added
  **outside** "New" is unsafe and shall be flagged. The editor shall also support a
  **`#INCLUDE`** directive that copies in external embedded programs; each included
  program must carry a unique `PROGRAM-ID` and a terminating `END PROGRAM` (the
  system validates this on Build/Check).

**Rust-FFI carry-over (from 005 AC6)**
- **R16 (event):** When COBOL evaluates an inline `obj::method(…)` call **as a
  value operand** — e.g. `DISPLAY S::len()`, `MOVE S::len() TO N`,
  `COMPUTE … = S::len()` — the runtime shall dispatch the method and use its
  returned value in place (marshaling the result to a COBOL value). Today `::`
  dispatches only as a *statement*; this extends `Expr::MethodCall` evaluation to
  expression position. The `INVOKE … RETURNING` form remains valid.

**Cross-cutting**
- **R13 (constraint):** All new IDE UI strings (e.g. "New", `INVOKE-FORM`/Invoke
  Form, `#INCLUDE`) shall be `Tr` fields in **all six** languages.
- **R14 (constraint):** The English `docs/developers-guide-en.md` shall document
  the form-module model (this spec); translations are user-maintained.
- **R15 (constraint):** All behaviour shall remain **portable** to the web/WASM
  target (006); no native-only assumption in the model (cross-executable sharing,
  R8, is the one place this is in tension — see Q1).

## 6. Acceptance criteria
- [ ] **AC1** — A project with two forms generates two COBOL program modules, each
  `PROGRAM-ID` = its form name (R1).
- [ ] **AC2** — Every woven procedure (an event handler **and** a user procedure)
  is emitted with `IS COMMON`, and an event handler can `CALL` another event
  handler in the same form at run time (R4).
- [ ] **AC3** — A `GLOBAL` `01`/`77` item and a `GLOBAL` `FD` declared on a form
  are usable from that form's handlers; a procedure-local item with `GLOBAL` is
  not visible outside that procedure (R5, R6, R9).
- [ ] **AC4** — An `EXTERNAL` item/`FD` set by form A is seen by form B in the same
  executable; identical declarations are required (R7). *(R8 cross-executable —
  demoed once Q1 is resolved.)*
- [ ] **AC5** — A value written into a user procedure's `WORKING-STORAGE` is still
  present on the next call (static); after an `INITIALIZE` it is reset (R10).
- [ ] ~~**AC6** — A handler runs `INVOKE-FORM` and the named form opens/runs (R11).~~
  *(Deferred with R11 — not in the active scope.)*
- [ ] ~~**AC7** — Adding code via **"New"** produces an embedded program with a
  unique `PROGRAM-ID` + `END PROGRAM`; a `#INCLUDE` file of embedded programs is
  copied in, and a missing/duplicate `PROGRAM-ID` or `END PROGRAM` is reported on
  Check (R12).~~ *(Deferred with R12 — the `#INCLUDE` engine/validation/IDE are a
  follow-up; the "New" embedded-program path already exists.)*
- [ ] **AC8** — New IDE strings exist in six languages; the English guide documents
  the model; the generated banner + regenerate-on-action contract are preserved
  (R13, R14).
- [ ] **AC9** — The 005 AC6 demo runs with the inline form: `DISPLAY S::len()`
  displays the Rust string's length (and `MOVE S::len() TO N` works), with the
  object dropped (no leak) (R16).

## 7. Constraints & steering check
- **i18n (6 languages):** New UI strings ×6 (R13).
- **Generated-code / regenerate contract:** The model is realised by codegen into
  the form's outer program + embedded programs; banner preserved; generated `.cbl`
  stays a build artifact (no hand-editing).
- **Docs (English guide):** New "Form module model" section (R14).
- **Fix vs feature:** **Feature** → minor (`y`) bump + `CHANGELOG.md` at finalize.
- **COBOL identifiers/source English; no "cobolt" in user-facing text.**
- **Portability (006):** keep the model wasm-portable; flag R8 (see Q1).

## 8. Open questions
- **Q1 (cross-executable `EXTERNAL`, R8):** Standard COBOL `EXTERNAL` is shared
  **within one run unit** (process). Sharing across **separate executables** needs
  an explicit backing — shared memory, a memory-mapped file, or a small IPC/broker
  — and has no portable web/WASM equivalent. *Decision needed:* (a) scope R8 to a
  single run unit (CALLed sub-modules) and treat cross-process as a **future**
  capability; or (b) commit to a desktop-only shared-memory backing now. → /plan.
- **Q2 (`INVOKE-FORM` semantics) — DEFERRED with R11:** modal vs modeless; does the
  caller block; argument passing to the invoked form; lifecycle of the invoked
  form. To be resolved when R11 is picked up in a future spec — not now.
- **Q3 (`#INCLUDE` resolution):** path resolution (project-relative? a configured
  include path?), ordering vs the woven procedures, and how `#INCLUDE`d programs
  participate in `IS COMMON`/`GLOBAL` visibility. → /plan.
- **Q4 ("New" vs raw code):** how strongly to **prevent** raw procedural code
  outside "New" — hard block, or a Check-time diagnostic only (R12 currently says
  flag/validate). → /plan.

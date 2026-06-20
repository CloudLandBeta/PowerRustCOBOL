# Spec — RustCOBOL standard property & method access

- **Status:** draft → approved
- **Folder:** specs/010-rustcobol-property-access/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-20

## 1. Overview

Standardise **all** control property/method access on the **RustCOBOL** syntax —
the inline `control::member` form and the `INVOKE` verb — and **remove** the
inherited Fujitsu PowerCOBOL property syntax `"Property" OF Control`. One
consistent model covers reads (GET) and writes (SET), inline and via `INVOKE`,
with the IntelliSense list driven by the `::` / `::"` triggers. This removes a
redundant, non-RustCOBOL idiom and makes the generated/edited code uniform.

## 2. Goals / Non-goals

> **No legacy.** There is currently **no** source using `"Property" OF Control`,
> so this is a clean removal — no migration path, diagnostic, or breaking-change
> handling, and no remaining reference to the form anywhere in the code or docs.

### Goals
- **Remove** `"Property" OF Control` from the language entirely (parser, AST,
  runtime, IntelliSense, docs) — no trace of it remains.
- **One property/method model**, all forms equivalent:
  - inline **GET**: `control::property` and `control::"property"` (a value).
  - `INVOKE` **GET**: `INVOKE control "property" RETURNING item`.
  - inline **SET**: `MOVE value TO control::property` / `SET control::"property" TO value`.
  - `INVOKE` **SET**: `INVOKE control "property" USING value`.
  - Explicit accessor prefixes: `GET-property` (returns) / `SET-property` (USING).
- **IntelliSense** rules for `::` and `::"` (properties green, methods light-blue;
  type-to-filter).
- Docs: §11 documents only the `::` / `INVOKE` forms; no change to the
  generated-code banner contract.

### Non-goals
- Removing the `INVOKE`/`::` **method** machinery (it stays; this only standardises
  **property** access onto it).
- New control properties or methods.
- Property **path / collection** access beyond a single member (`Items(2)` etc.) —
  see Q1.
- Auto-migrating existing source (a diagnostic + guide note, not a rewriter).

## 3. User stories
- As a developer, I read and write any control property with one syntax —
  `BUTTON-1::Caption` — inline or via `INVOKE`, for GET and SET.
- As a developer, IntelliSense after `::` (or `::"`) lists the control's
  properties and methods and filters as I type.
- As a developer, there is exactly **one** way to touch a property, so my code and
  the examples are uniform.

## 4. Requirements (EARS)

**Removal**
- **R1 (ubiquitous):** The `"<literal>" OF <name>` property form shall be removed
  entirely: the `Expr::PropertyRef` parse path, AST variant, and runtime handling
  are deleted. `"<literal>" OF …` is no longer special-cased (it parses as an
  ordinary string literal, so the construct simply ceases to exist). No reference
  to the form remains in code or docs.

**GET**
- **R2:** Inline `control::property` and `control::"property"` used as a **value
  operand** (e.g. `DISPLAY`, `MOVE … TO x`, `IF`, `COMPUTE`) shall evaluate to the
  control's current value of `property`.
- **R3:** `INVOKE control "property" RETURNING item` shall place the property value
  into `item`.
- **R4:** `INVOKE control "GET-property" RETURNING item` shall do the same
  (explicit `GET-` prefix).

**SET**
- **R5:** Assigning to an inline target — `MOVE value TO control::property` and
  `MOVE value TO control::"property"` — shall set the control's `property`.
- **R6:** `SET control::"property" TO value` (and `SET control::property TO value`)
  shall set the control's `property`.
- **R7:** `INVOKE control "property" USING value` shall set the control's
  `property` (a `USING` argument ⇒ SET).
- **R8:** `INVOKE control "SET-property" USING value` shall set the control's
  `property` (explicit `SET-` prefix).

**Unified dispatch**
- **R9 (ubiquitous):** The runtime method dispatcher shall resolve a member name
  as a **property accessor** when it is not an explicit control method:
  a `GET-`/`SET-` prefix selects get/set explicitly; otherwise a bare member is a
  **get** with no `USING` argument (value / `RETURNING`) and a **set** with a
  `USING` argument or when it is an assignment target. Existing explicit methods
  (`SetCaption`, `GetText`, …) keep priority.

**IntelliSense**
- **R10 (event):** When the developer types `::`, the editor shall show the list of
  the control's **properties (green)** and **methods (light-blue)**; continuing to
  type characters other than `"` shall filter the list by the typed text.
- **R11 (event):** When the developer types `::"`, the editor shall show the same
  list; continuing to type characters other than `"` shall filter it (and complete
  with the closing `"`).
- **R12 (constraint):** A bare `"` (string literal) shall **not** trigger any
  property popup (it is a literal — unchanged from 005).

**Cross-cutting**
- **R13 (constraint):** Any new user-facing IDE string shall be a `Tr` field in all
  **six** languages.
- **R14 (constraint):** The English `docs/developers-guide-en.md` §11 shall be
  rewritten to document **only** the `::` / `INVOKE` forms (GET + SET), with no
  mention of the removed `OF` form; translations untouched.
- **R15 (constraint):** Generated `.cbl` is unaffected (codegen never emitted the
  `OF` form), and no example/template references it.

## 5. Acceptance criteria
- [ ] **AC1** — `"Caption" OF BUTTON-1` no longer parses as a property reference
  (the `OF` property form is gone); no reference to it remains in code/docs.
- [ ] **AC2 (GET)** — `DISPLAY BUTTON-1::Caption`, `DISPLAY BUTTON-1::"Caption"`,
  and `INVOKE BUTTON-1 "Caption" RETURNING X` all yield the button's caption;
  `INVOKE BUTTON-1 "GET-Caption" RETURNING X` does too (R2–R4, R9).
- [ ] **AC3 (SET)** — `MOVE "Hi" TO BUTTON-1::Caption`,
  `SET BUTTON-1::"Caption" TO "Hi"`, `INVOKE BUTTON-1 "Caption" USING "Hi"`, and
  `INVOKE BUTTON-1 "SET-Caption" USING "Hi"` all set the caption (R5–R8, R9).
- [ ] **AC4** — Round trip: set then get returns the set value, at run time, for a
  representative property (e.g. `Text` on a TextBox).
- [ ] **AC5 (IntelliSense)** — Typing `BUTTON-1::` lists properties (green) +
  methods (light-blue); typing `Cap` filters to `Caption`; `BUTTON-1::"` shows the
  list and filters the same; a lone `"` shows no property popup (R10–R12).
- [ ] **AC6** — Six-language i18n green; §11 rewritten to the `::`/`INVOKE` forms
  only; generated banner/regenerate contract intact (R13–R15).

## 6. Constraints & steering check
- **i18n (6 languages):** only if new UI strings are added (R13).
- **Generated-code / regenerate contract:** unaffected — codegen does not emit the
  `OF` form; this is a parser/runtime/editor/docs change.
- **Docs (English guide):** §11 rewrite to the `::`/`INVOKE` forms (R14).
- **Fix vs feature:** language standardisation → **feature** (minor bump +
  CHANGELOG). **No legacy code uses the `OF` form, so the removal is not breaking.**
- **No "cobolt" in user text; COBOL identifiers/source English.**

## 7. Open questions
- **Q1 (collections/paths):** the removed `OF` form supported a path
  (`"Items"(2) OF LIST-1`). Does the `::` form need `LIST-1::Items(2)` /
  `LIST-1::"Items"(2)`? *Recommendation:* support a single trailing subscript on
  the member (`control::member(index)`) for parity; resolve exact grammar in /plan.
- **Q2 (AST cleanup): RESOLVED — remove now.** No legacy code exists, so the
  `Expr::PropertyRef` variant + its runtime arms are deleted outright in this
  change; the `property_ref_key` resolution logic is reused for the `::`
  assignment target.
- **Q3 (unknown member):** a bare member that is neither an explicit method nor a
  known property — error, or lenient generic get/set (like `SetProperty`)?
  *Recommendation:* **lenient** generic property get/set (any name works), so
  custom/dynamic properties keep working; an *unknown method with args that look
  like a call* is unaffected. Confirm in /plan.
- **Q4 (`SET … TO` ambiguity):** `SET` is overloaded (pointers, indices, condition
  names). Ensure `SET control::member TO value` is unambiguously the property form
  (the `::` token disambiguates). Validate in /plan.

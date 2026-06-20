# Spec — Member-access chains, nested object model, property-as-receiver

- **Status:** draft → approved → implemented
- **Folder:** specs/011-member-access-chains/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-20

## 1. Overview

Generalise the spec-010 single-level `control::member` access into a **chainable**
member-access syntax that reaches object members to any depth with one consistent
operator (`::`), backed by a **real nested object model** (addressable
rows → columns → cells, list items). Make a control **property a first-class
receiving field** for *every* content-changing verb (not just `MOVE`/`SET`).
Closes spec-010 Q1 (collections / indexed paths) and the property-as-receiver
gap.

## 2. Goals / Non-goals

### Goals
- **Chain grammar:** `root::member[(args)]::member[(args)]…` parses to a nested
  node; `::` is the only member separator.
- **Nested model:** properties may hold nested objects and indexable
  collections; a chain navigates them (`Grid::Rows(I)::Columns(2)::Value`).
- **Indexed form:** `ctrl::Items(4)`, `ctrl::Rows(var)` index a collection (real
  list, or the legacy newline-string item list via interop).
- **Property-as-receiver for all verbs:** any verb that resolves its receiving
  field through the runtime lvalue resolver may write a property / nested cell.
- **lvalue vs rvalue by trailing `()`:** a property/indexed-cell tail is
  assignable; a method-call tail is a value only.
- **INITIALIZE rules:** `obj` → its `Value`; `obj::prop` → that property;
  `obj name` → each operand by its own rules.
- **IntelliSense:** member list still triggers on `::`/`::"` and on chain tails.

### Non-goals
- Deep element-type inference in IntelliSense (offer the root control's members).
- Method-returning-object *mid-chain* navigation beyond a value transform tail.
- A dot (`.`) member separator (the example dot was a typo — `::` only).

## 3. User stories
- As a developer I read and write a grid cell with one syntax —
  `Grid::Rows(I)::Columns(2)::Value` — for GET and for any writing verb.
- As a developer I delete a row with `List::Rows(I)::Delete()` and transform a
  value with `obj::Value::toUpperCase()`.
- As a developer I `STRING … INTO`, `ADD … TO`, or `COMPUTE` directly into a
  control property, with no intermediate `PIC` item.

## 4. Requirements (EARS)

**Grammar / AST**
- **R1:** `root::member` chains shall parse to a left-recursive `Expr::Member
  { recv, member, args, parens, span }`; `parens` records whether `()` was
  written; `args` carries subscript indices or call arguments.
- **R2:** An inline chain used as a statement shall parse to `Stmt::InvokeExpr`
  and be evaluated for effect (result discarded).
- **R3:** A member word that collides with a COBOL keyword (`Value`, `Delete`,
  `Count`, …) shall still be accepted as a member name.

**Runtime model**
- **R4:** A property may hold a nested object or an indexable list; a chain shall
  navigate them, auto-vivifying intermediate containers on write.
- **R5:** A parens segment shall resolve as a **method call** when its name is a
  known method (or it has no args), otherwise as a **collection index**.
- **R6:** `ctrl::Items(n)` shall index the legacy newline-string item list when
  the property is a string.

**GET / SET / verbs**
- **R7 (GET):** a property / indexed-cell chain evaluates to its value (numeric
  values stay algebraic); a method-call tail evaluates to the method result
  (collection verbs `Count`/`Delete`/`Clear`/`Add`; transforms `toUpperCase`/
  `toLowerCase`/`trim`/`len`).
- **R8 (SET, all verbs):** any verb whose receiving field is a property /
  indexed-cell chain shall write it and notify the UI; the dormant property-shadow
  mechanism is re-keyed on the chain so this needs no per-verb code.
- **R9 (lvalue rule):** a chain ending in a method call `()` is **not** a
  receiving field — a runtime error and a compile-time diagnostic.

**INITIALIZE**
- **R10:** `INITIALIZE obj` resets `obj::Value`; `INITIALIZE obj::prop` resets
  that property; `INITIALIZE obj name` initialises each operand by its own rules.

**IntelliSense / docs**
- **R11:** the member list triggers on `::`, `::"`, and chain tails (`…)::`,
  `…::member::`), resolved against the chain's root control; properties green,
  methods light-blue; type-to-filter.
- **R12:** `docs/developers-guide-en.md` §11 documents chains, the indexed form,
  the `()` lvalue/rvalue rule, and the INITIALIZE rules (English guide only).

## 5. Acceptance criteria
- [x] **AC1** — `Grid::Rows(0)::Columns(1)::Value` round-trips (set then get);
  `…::Value::toUpperCase()` yields the upper-cased value (R1, R4, R7).
- [x] **AC2** — `List::Items(1)` reads the 2nd list line; `List::Items::Count()`
  returns the count (R6, R7).
- [x] **AC3** — `List::Rows(0)::Delete()` removes the element and shifts the rest
  (R5, R7).
- [x] **AC4** — `STRING … INTO ctrl::Text` and `ADD n TO ctrl::Value` write the
  property (R8).
- [x] **AC5** — `MOVE name TO obj::UpperCase()` is a runtime error / diagnostic;
  `obj::UpperCase().` as a statement changes nothing (R9, R2).
- [x] **AC6** — `INITIALIZE obj` clears only `Value`; `INITIALIZE obj::Value name`
  clears the property and resets the data item (R10).
- [x] **AC7** — IntelliSense lists members on `::`, `::"`, and chain tails; full
  `cargo test --workspace` green; §11 rewritten (R11, R12).

## 6. Constraints & steering check
- **Fix vs feature:** classified as a **fix** (per the maintainer's call —
  completing/correcting the spec-010 `::` model) → patch bump 1.27.2 + CHANGELOG.
- **Docs:** English guide §11 only; translations untouched; no "cobolt" in
  user-facing text; COBOL identifiers English.
- **i18n:** no new user-facing IDE strings added.
- **Generated-code / regenerate contract:** unaffected.

## 7. Resolved questions
- **Q1 (runtime depth):** **real nested object model** (user's choice), not
  flattened keys.
- **Q2 (separator):** `::` only — the dot in one example was a typo.

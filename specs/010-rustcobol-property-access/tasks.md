# Tasks — RustCOBOL standard property & method access

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-06-20

Ordered, independently-verifiable tasks by the plan's phases (§8). The project
stays green after each task. **No legacy** uses the `OF` form, so its removal is a
clean deletion (no migration/diagnostic). Inline `::` GET already evaluates as a
value (009 R16); this feature closes the remaining gaps and removes the `OF` form.

## Phase 1 — Remove `"Property" OF Control` entirely

- [x] **T1 — Delete the `OF` property form** (R1, R15; AC1)
  - Files: `crates/cobolt-parser/src/expr.rs` (remove the `StringLiteral` + `OF`
    trigger at ~169 and `parse_property_ref` ~256); `crates/cobolt-ast/src/expr.rs`
    (delete `Expr::PropertyRef` + its path-segment type); `crates/cobolt-runtime/
    src/interpreter.rs` (extract `property_ref_key`'s resolution into a standalone
    `resolve_member(control, member) -> (ctrl, key)` helper, then delete the four
    `PropertyRef` arms — ~1121, 1218, 3863, 4397 — and the old `property_ref_key`).
  - Do: the `OF`-property construct ceases to exist; `"X" OF Y` parses as a plain
    string literal; nothing references the removed form.
  - Verify: `cargo build --workspace` (compiler proves no dangling `PropertyRef`
    use). `grep -rn "PropertyRef\|parse_property_ref\|property_ref_key" crates/`
    returns nothing. `cargo test -p cobolt-parser` — a `"X" OF Y` snippet yields a
    plain string literal, **not** a property ref (new/updated test). Report counts.

## Phase 2 — Unified dispatch: GET, INVOKE SET, GET-/SET- prefixes

- [x] **T2 — Property accessor fallback in `exec_method`** (R2–R4, R7–R9; AC2 partial)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (`exec_method` ~3714):
    after the explicit-verb `match`, replace the `_ =>` fallback — strip a
    `GET-`/`SET-` prefix (explicit get/set of the remainder); else `args.is_empty()`
    ⇒ `obj_get(member)` (get); else ⇒ `obj_set(member, arg0)` + none (set).
    Explicit methods keep priority.
  - Do: a bare property name and `GET-`/`SET-` prefixes work as accessors via the
    inline `::` value path (009) and `INVOKE`.
  - Verify: `cargo test -p cobolt-runtime` — `DISPLAY ctrl::Caption`,
    `DISPLAY ctrl::"Caption"`, `INVOKE ctrl "Caption" RETURNING X`,
    `INVOKE ctrl "GET-Caption" RETURNING X` all return the caption;
    `INVOKE ctrl "Caption" USING "Hi"` and `INVOKE ctrl "SET-Caption" USING "Hi"`
    set it; an explicit method (`SetText`/`GetText`) still wins (new test). Report.

## Phase 3 — Inline SET targets (`MOVE … TO` / `SET … TO`)

- [x] **T3 — `MethodCall` as an assignment target** (R5; AC3 partial, AC4)
  - Files: `crates/cobolt-runtime/src/interpreter.rs` (MOVE/SET target loop ~1210):
    add an `Expr::MethodCall` arm using `resolve_member` — `objects.set_property` +
    `state_tx` (mirrors the deleted `PropertyRef` set arm).
  - Do: `MOVE value TO ctrl::property` and `MOVE value TO ctrl::"property"` set the
    property at run time.
  - Verify: `cargo test -p cobolt-runtime` — `MOVE "Hi" TO ctrl::Caption` then
    `DISPLAY ctrl::Caption` ⇒ `Hi`; round-trip on `Text` of a TextBox (new test).
    Report counts.

- [x] **T4 — `SET control::property TO value`** (R6; AC3 partial)
  - Files: `crates/cobolt-parser/src/stmt.rs` (`parse_set` ~2066): when the SET
    operand is a `control::member` (`::`/`MethodCall`) followed by `TO`, emit the
    property-set form (a `Stmt::SetProperty` or reuse the MOVE assignment path);
    `crates/cobolt-runtime/src/interpreter.rs` (execute it via `resolve_member`).
  - Do: `SET ctrl::"property" TO value` (and bare) sets the property; existing SET
    forms (`SET ptr TO`, `SET idx UP BY`, condition-name SET) are unchanged (the
    `::` token disambiguates).
  - Verify: `cargo test -p cobolt-parser` — `SET ctrl::Caption TO "Hi"` parses to
    the property form; `SET ptr TO X` / `SET idx UP BY 1` / condition-name SET
    unchanged. `cargo test -p cobolt-runtime` — `SET ctrl::"Caption" TO "Hi"` sets
    it (new tests). Report counts.

## Phase 4 — IntelliSense `::` / `::"` rules

- [x] **T5 — `::` and `::"` trigger + filter** (R10–R12, R13)
  - Files: `crates/cobolt-ide/src/panels/editor.rs` (trigger the property+method
    list on `::` and on `::"`; filter on subsequent non-`"` characters; never popup
    on a lone `"`; properties green (`AcKind::Property`), methods light-blue
    (`AcKind::Method`); insert the right closing token per form `::Name` vs
    `::"Name"`); `crates/cobolt-ide/src/i18n.rs` only if any new UI string is added.
  - Do: the four spec rules (R10–R12) hold exactly.
  - Verify: `cargo test -p cobolt-ide` — trigger-detector unit tests for `::`,
    `::Cap`, `::"`, `::"Cap`, and a lone `"` (popup vs no-popup; filter text).
    `cargo test -p cobolt-ide i18n` (×6, no empty) if strings added. Manual: type
    `BUTTON-1::` → green props + blue methods; `Cap` filters to `Caption`; `::"`
    behaves the same.

## Phase 5 — Docs & finalize

- [x] **T6 — Docs (English guide §11 rewrite)** (R14; AC6)
  - Files: `docs/developers-guide-en.md` (rewrite §11 "Talking to the UI from
    COBOL" to document **only** the `::` / `INVOKE` GET+SET forms and the
    `GET-`/`SET-` prefixes; remove every mention of the `"X" OF Y` form).
  - Verify: review; no occurrence of the `OF`-property syntax remains in the guide;
    English guide only (translations untouched).

- [x] **T7 — Finalize** (all ACs)
  - Files: `crates/cobolt-ide/src/version.rs` (+ `CHANGELOG.md`) — feature minor
    bump (standardise property access on `::`/`INVOKE`; remove the unused `OF`
    form; not breaking — no legacy).
  - Verify: `cargo build --workspace` + `cargo test --workspace` green;
    `cargo test -p cobolt-ide i18n`. Manual AC walkthrough: AC1 (`OF` gone, no
    references), AC2 (all GET forms), AC3 (all SET forms), AC4 (set→get round trip),
    AC5 (IntelliSense `::`/`::"`), AC6 (i18n ×6 + §11 rewrite + banner intact).

## Done criteria
All acceptance criteria are covered (AC1: T1 · AC2: T2 · AC3: T2/T3/T4 · AC4: T3 ·
AC5: T5 · AC6: T5/T6/T7), tests pass, **no reference to the `OF`-property form
remains** anywhere, the generated-code banner/regenerate contract is intact, docs
updated, and the work is committed as feature commit(s) per the operator's rules
(do **not** commit/push unless asked).

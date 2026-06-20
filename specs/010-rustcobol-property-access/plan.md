# Plan — RustCOBOL standard property & method access

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-20

## 1. Approach

Property access already *mostly* rides on the `Expr::MethodCall` + `INVOKE`
machinery (tasks 047–049, and 009 R16 made inline `::` a value operand). This plan
(1) **removes** the `"Property" OF Control` parse path and (2) closes the small
gaps so a **bare property name** behaves as a getter/setter across all forms, and
the inline `control::member` works as an **assignment target**. The dispatcher
becomes the single resolution point (R9): explicit methods first, then
`GET-`/`SET-` prefixes, then a generic property get/set.

- **Removal (R1).** No legacy code uses the `OF` form, so remove it outright: drop
  the parser trigger `StringLiteral` + `OF` → `parse_property_ref` (expr.rs:169,
  256) — no replacement diagnostic — reuse the `property_ref_key` resolution logic
  for the `::` assignment target, then delete the `Expr::PropertyRef` variant + its
  four runtime arms. `"X" OF Y` thereafter parses as an ordinary string literal.

- **GET (R2–R4).** Inline `control::property` (value) already evaluates via
  `eval_expr(Expr::MethodCall)` → `exec_method`. `INVOKE … RETURNING` already calls
  `exec_method`. The gap is the dispatcher: today a bare property name (`Caption`)
  has no arm and returns empty. Add a fallback (R9) so a no-arg member returns
  `obj_get(member)`, and a `GET-<prop>` member returns `obj_get(prop)`.

- **SET (R5–R8).** Two paths:
  - **inline assignment target** (`MOVE v TO control::property`, `SET control::"property" TO v`):
    the MOVE/SET target loop currently special-cases `Expr::PropertyRef`
    (interpreter.rs:1218) and `Expr::RefMod`. Add an `Expr::MethodCall` target arm
    that resolves `(control, member)` and does `objects.set_property` + `state_tx`
    (mirroring the PropertyRef arm). For `SET … TO …`, `parse_set` (stmt.rs:2066)
    gains a "property target" form: when the SET operand is a `control::member`
    (a `MethodCall`/`::`) followed by `TO`, emit a property-set statement (reuse
    `Stmt::Move`-style assignment or a small `Stmt::SetProperty`).
  - **`INVOKE … USING`**: the dispatcher (R9) treats a member **with** a `USING`
    arg as a set — `obj_set(member, arg0)` — and `SET-<prop>` as an explicit set.

- **Unified dispatch (R9).** In `exec_method`, after the explicit-verb `match`,
  replace the `_ =>` fallback with: strip a `GET-`/`SET-` prefix (→ explicit get/
  set of the remainder); else if `args.is_empty()` → `obj_get(member)` (get); else
  → `obj_set(member, arg0)` (set). Explicit methods keep priority (they are matched
  first). This makes inline GET, INVOKE GET/SET, and the prefixes all consistent.

- **IntelliSense (R10–R12).** `editor.rs` already lists properties (green
  `AcKind::Property`) + methods (light-blue `AcKind::Method`) after `::`. Tighten
  the trigger/filtit logic to the spec's exact rules: show on `::` and on `::"`;
  filter on subsequent non-`"` characters; never popup on a lone `"` (already true
  since 005). Ensure property completion inserts the right closing token for each
  form (`::Name` vs `::"Name"`).

## 2. Affected crates / files
- `crates/cobolt-parser/src/expr.rs` — remove the `"lit" OF` property-ref trigger
  (169) + `parse_property_ref` (256); keep `MethodCall` parsing (already produces
  it in operand and, via `parse_set`, target position).
- `crates/cobolt-parser/src/stmt.rs` — `parse_set` (2066): accept
  `SET control::member TO value` (property-set form); confirm `MOVE … TO control::member`
  already parses the target via `parse_expr` (it does → `MethodCall`).
- `crates/cobolt-ast/src/expr.rs` — delete the `Expr::PropertyRef` variant +
  `PropertyPathSeg` once all uses are gone (Q2).
- `crates/cobolt-runtime/src/interpreter.rs` —
  - Extract `property_ref_key`'s `(ctrl, key)` resolution into a standalone helper
    (`resolve_member(control, member) -> (ctrl, key)`) that does **not** depend on
    `Expr::PropertyRef`; then delete the four `PropertyRef` arms (1121, 1218, 3863,
    4397) and the old `property_ref_key`.
  - `exec_method` (~3714): R9 fallback (GET-/SET- prefix + bare get/set).
  - MOVE/SET target loop (~1210): add an `Expr::MethodCall` assignment-target arm
    using the extracted helper (mirrors the old PropertyRef set: `set_property` +
    `state_tx`).
  - `Stmt::SetProperty`/property-set execution if a new stmt variant is chosen.
- `crates/cobolt-ide/src/panels/editor.rs` — IntelliSense `::` / `::"` trigger +
  filter rules; ensure green/blue kinds; insert text per form.
- `docs/developers-guide-en.md` — §11 rewrite (GET/SET via `::`/`INVOKE` only; no
  mention of the `OF` form).
- `CHANGELOG.md`, `crates/cobolt-ide/src/version.rs` — feature minor bump
  (standardise property access on `::`/`INVOKE`; remove the unused `OF` form).

## 3. Data / model changes
- **AST:** remove `Expr::PropertyRef` + its path-segment type. Optionally add a
  light `Stmt::SetProperty { control, member, value }` (or reuse the MOVE
  assignment machinery) for `SET control::member TO value`.
- **No `.cfrm` / `cobolt.toml` change.** **No generated-code contract change.**

## 4. Key decisions & alternatives
- **Single dispatch point (R9) over per-form special-casing.** — Why: one place
  defines GET/SET, so inline and `INVOKE`, prefixed and bare, all agree. Rejected:
  handling properties only in the assignment/eval sites (drift, inconsistent
  `INVOKE` behaviour).
- **Lenient generic property get/set for unknown members (Q3).** — Why: custom or
  not-yet-known properties keep working (matches today's `SetProperty`/
  `GetProperty`); a bare member never silently fails to a wrong method. Rejected:
  hard error on unknown member (breaks dynamic property names).
- **`::` is the disambiguator for `SET … TO` (Q4).** — Why: `SET x::y TO z`
  contains the `::` token, unambiguous vs `SET ptr TO`, `SET idx UP BY`, condition
  names. Rejected: a new keyword.
- **Remove `Expr::PropertyRef` now (Q2).** — Why: its resolution logic is reused
  for the `::` target, so keeping a dead variant adds confusion. Rejected: leave
  dead for a release (no benefit; codegen never used it).
- **Single trailing subscript for collections (Q1).** — `control::Items(2)` /
  `control::"Items"(2)` parsed as a subscript on the member; defer multi-segment
  paths. Rejected: full path grammar (the old `OF` chain) — unused in practice.

## 5. Risks & mitigations
- **Leftover references to the removed form.** → Grep the whole tree (code, tests,
  examples, docs) for `OF`-property usage and `PropertyRef`/`parse_property_ref`/
  `property_ref_key` and delete every one; the workspace must build with the AST
  variant gone (the compiler enforces no dangling reference).
- **Dispatcher fallback could mask typos** (a misspelled method silently becomes a
  property get returning ""). → Keep explicit methods first; the fallback only
  applies to non-method members; document that bare members are property accessors.
- **`SET … TO` grammar ambiguity.** → Gate on the `::` token; add parser unit tests
  for `SET ptr TO`, `SET idx`, condition-name SET, and `SET ctrl::prop TO` to prove
  no regression.
- **IntelliSense over/under-triggering** on `::"` vs `::`. → Unit-test the trigger
  detector for `::`, `::Cap`, `::"`, `::"Cap`, and a lone `"`.

## 6. Test strategy
- **`cobolt-parser`:** `"X" OF Y` no longer yields a `PropertyRef` (it parses as a
  plain string literal); `control::member`, `control::"member"`, and `control::member(2)`
  parse as `MethodCall`(+subscript) in operand **and** target position;
  `SET ctrl::prop TO v` parses; `SET ptr TO` / `SET idx` unchanged. Report counts.
- **`cobolt-runtime`:** GET — `DISPLAY ctrl::Caption`, `::"Caption"`,
  `INVOKE … "Caption" RETURNING`, `"GET-Caption" RETURNING` all return the value;
  SET — `MOVE v TO ctrl::Caption`, `SET ctrl::"Caption" TO v`,
  `INVOKE … "Caption" USING v`, `"SET-Caption" USING v` all set it; a set-then-get
  round trip; explicit methods (`SetText`/`GetText`) still win. Report counts.
- **`cobolt-ide`:** IntelliSense trigger/filter unit tests (the four R10–R12
  cases); `cargo test -p cobolt-ide i18n` (×6) if strings added.
- **Manual:** in the editor, type `BUTTON-1::` → list (green props / blue methods);
  filter; `::"` form; run a form that sets and reads a property both ways.

## 7. Steering compliance
- [ ] i18n: any new UI string in 6 languages (R13).
- [ ] Generated-code banner + regenerate-on-action unaffected (parser/runtime/
      editor/docs change only).
- [ ] English dev guide §11 rewritten; translations untouched (R14).
- [ ] Fix vs feature: **feature** → minor bump + CHANGELOG (no legacy ⇒ not a
      breaking change).
- [ ] No "cobolt" in user text; COBOL source English.

## 8. Phasing (proposed for /tasks)
- **Phase 1 — Remove `"X" OF Y` entirely.** Delete the parser trigger/
  `parse_property_ref`, the `Expr::PropertyRef` AST variant, and its runtime arms;
  grep the tree to confirm no reference remains; parser tests. (R1, R15.)
- **Phase 2 — Unified dispatch (GET + INVOKE SET + prefixes).** `exec_method` R9
  fallback; runtime GET/INVOKE tests. (R2–R4, R7–R9.)
- **Phase 3 — Inline SET targets.** `MethodCall` assignment-target arm (using the
  extracted resolver) + `parse_set` property form; runtime SET tests. (R5, R6.)
- **Phase 4 — IntelliSense `::` / `::"` rules.** Trigger + filter + colours; editor
  unit tests; i18n if needed. (R10–R12, R13.)
- **Phase 5 — Docs + finalize.** §11 rewrite + migration note; version/CHANGELOG
  (breaking-change note); full `cargo test --workspace`; AC walkthrough. (R14.)

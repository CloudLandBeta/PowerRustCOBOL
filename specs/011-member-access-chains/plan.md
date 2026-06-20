# Plan — Member-access chains, nested object model, property-as-receiver

- **Status:** approved → implemented
- **Spec:** ./spec.md   **Date:** 2026-06-20

## 1. Approach

A single chainable AST node plus a real nested object model, with the existing
property-shadow mechanism re-keyed on chains so "all verbs" needs no per-verb
code. `::` is the only separator; the index-vs-method ambiguity is resolved by a
closed known-method vocabulary (properties/collections are open, methods are not).

- **AST (`cobolt-ast/src/expr.rs`):** replace `Expr::MethodCall { object: String,
  … }` with `Expr::Member { recv: Box<Expr>, member, args, parens, span }`. Add
  `Stmt::InvokeExpr { expr, span }` for inline chain statements.
- **Parser (`cobolt-parser/src/expr.rs`, `stmt.rs`):** a postfix `::` loop
  (`parse_member_chain`) over a parsed primary builds the nested `Member`; an
  inline chain statement parses the full expression into `Stmt::InvokeExpr`.
- **Lexer (`cobolt-lexer/src/lexer.rs`):** a post-pass (`reclassify_member_words`)
  turns any keyword token immediately after `::` back into an `Identifier`
  (recovering its spelling from the preprocessed source) so keyword-named members
  (`Value`, `Delete`, `Count`) work. `::` appears in no other construct.
- **Object model (`cobolt-runtime/src/objects.rs`):** extend `PropertyValue` with
  `Object(Box<CoboltObject>)` and `List(Vec<PropertyValue>)`; add path navigation
  (`get_path`/`set_path`/`remove_path`/`path_len` over `PathSeg::{Prop,Index}`)
  with auto-vivify and legacy `Items`-string line interop.
- **Interpreter (`cobolt-runtime/src/interpreter.rs`):** `lower_member_chain` →
  root + segments; `resolve_member` → `Resolved::{Path,Method}` using
  `is_known_method`; `eval_member` (GET), `assign_member` (SET + lvalue-rule
  error), `exec_member_method` (collection verbs + scalar transforms; empty path
  delegates to `exec_method`), `set_member`/`append_member` (+ `StateUpdate`).
  `resolve_lvalue` registers a chain shadow (seeded numeric/alnum by value shape)
  and `flush_property_shadows` writes it back via `set_member` → all verbs.
  `exec_initialize` handles bare-control and `Member` operands.
- **Semantic (`cobolt-semantic/src/resolver.rs`):** `Expr::Member` arm (no warn on
  the control root); `check_receiving` warns on an empty-parens method-call tail
  in `MOVE`/`COMPUTE` receiving positions.
- **IDE (`cobolt-ide/src/panels/editor.rs`):** `detect_invoke_context` resolves
  the chain root for chain-tail completion; `member_completions` unchanged.

## 2. Key decisions
- **Known-method vocabulary disambiguates index vs call** — independent of whether
  the collection exists yet, so auto-vivifying writes classify correctly.
- **Re-key the existing shadow mechanism** — `property_shadows` value becomes
  `(control, Vec<PathSeg>)`; `flush` writes through `set_member`. No per-verb edits.
- **Lexer post-pass for keyword members** — robust and contained; avoids a large
  reverse keyword map and avoids threading source into the parser.
- **`Items`-string interop** — `Items(n)` indexes newline lines so existing
  list/combo controls keep working alongside the real `List` model.

## 3. Risks & mitigations
- **Receiver-site coverage** — verified the receiving-field verbs route through
  `resolve_lvalue` (MOVE special-cases `Member` via `assign_member`; STRING,
  UNSTRING, ADD/GIVING, COMPUTE, ACCEPT, INSPECT, DIVIDE-remainder use
  `resolve_lvalue`). Shadow seeding picks numeric vs roomy-alnum by value shape.
- **Keyword-as-member** — covered by the lexer post-pass + parser tests.
- **lvalue rule on parens-tail** — runtime errors; semantic flags the
  unambiguous empty-parens case (an indexed tail keeps its args and is allowed).

## 4. Test strategy
- **parser:** chain nesting, subscripts vs calls, `Stmt::InvokeExpr`, the
  existing `::` property forms (`test_statements.rs`).
- **runtime:** nested get/set + transform, `Items(n)`/`Count`, `Delete`,
  `STRING`/`ADD` into a property, INITIALIZE rules, invalid method-lvalue error,
  no-effect method statement (`test_property_access.rs`).
- **semantic:** existing suite green (no warn on control roots).
- **ide:** member list both kinds + chain-tail root resolution (`editor.rs`).
- **finalize:** `cargo test --workspace`; manual IDE run.

## 5. Steering compliance
- [x] Classified as a **fix** (maintainer's call) → patch bump 1.27.2 + CHANGELOG.
- [x] English dev guide §11 only; translations untouched.
- [x] No new i18n strings; generated-code contract unaffected.
- [x] No "cobolt" in user text; COBOL source English.

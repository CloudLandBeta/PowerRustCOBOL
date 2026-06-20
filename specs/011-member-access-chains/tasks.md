# Tasks — Member-access chains, nested object model, property-as-receiver

- **Status:** implemented   **Spec:** ./spec.md   **Plan:** ./plan.md

All tasks complete. `cargo test --workspace` green (70 test binaries, 0 failures).

- [x] **T1 — AST.** Replace `Expr::MethodCall` with `Expr::Member
  { recv, member, args, parens, span }`; add `Stmt::InvokeExpr`; update `span()`.
  Files: `cobolt-ast/src/{expr,stmt}.rs`. (R1, R2)
- [x] **T2 — Parser.** Postfix `::` chain (`parse_member_chain`) in `parse_primary`;
  inline chain statement → `Stmt::InvokeExpr`; drop `parse_method_tail`.
  Files: `cobolt-parser/src/{expr,stmt}.rs`. Tests: chain nesting, subscript vs
  call, `InvokeExpr`, `::` property forms. (R1–R3)
  Verify: `cargo test -p cobolt-parser`.
- [x] **T3 — Lexer.** `reclassify_member_words` post-pass: keyword after `::` →
  `Identifier`. File: `cobolt-lexer/src/lexer.rs`. Verify: `cargo test -p cobolt-lexer`. (R3)
- [x] **T4 — Object model.** `PropertyValue::{Object,List}` + `PathSeg` +
  `get_path`/`set_path`/`remove_path`/`path_len` + `Items`-string interop;
  `CoboltObject: PartialEq`. File: `cobolt-runtime/src/objects.rs`. (R4–R6)
- [x] **T5 — Runtime GET.** `lower_member_chain`, `resolve_member`+`is_known_method`,
  `eval_member`, `exec_member_method`, `prop_to_value`. (R5, R7)
- [x] **T6 — Runtime SET / all-verb.** `assign_member` (+ lvalue-rule error),
  `set_member`/`append_member`, re-keyed `property_shadows` +
  `flush_property_shadows`; retarget MOVE/RETURNING `Member` arms. (R8, R9)
  Files: `cobolt-runtime/src/interpreter.rs`.
- [x] **T7 — INITIALIZE.** Bare-control → `Value`; `Member` operand;
  `init_default_for_member`. (R10)
- [x] **T8 — Semantic.** `Expr::Member` arm (no warn on control root);
  `check_receiving` diagnostic on empty-parens method-call tail.
  File: `cobolt-semantic/src/resolver.rs`. (R9, R11)
- [x] **T9 — IDE.** `detect_invoke_context` chain-root resolution for chain-tail
  completion. File: `cobolt-ide/src/panels/editor.rs`. (R11)
- [x] **T10 — Tests.** Runtime `test_property_access.rs` (nested get/set+transform,
  `Items`/`Count`, `Delete`, STRING/ADD receivers, INITIALIZE, invalid lvalue,
  no-effect statement); parser chain tests; editor chain-completion test.
- [x] **T11 — Docs + finalize.** §11 (chains, indexed form, `()` rule,
  INITIALIZE); version 1.27.2 (fix) + CHANGELOG; `cargo test --workspace`. (R12)

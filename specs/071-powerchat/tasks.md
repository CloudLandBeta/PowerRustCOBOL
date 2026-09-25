# Tasks — PowerChat, Phase 1

- **Status:** in progress
- **Plan:** ./plan.md   **Date:** 2026-09-24

`features` branch. Tests print quantified results (GOLDEN RULE #7).

- [ ] **T1 — Regen tool** — `crates/cobolt-ide/examples/powerchat_regen.rs`: for every form in
  the manifest, writes `generated/<form>.cbl` (`cobolt_codegen::generate`), the SideMenu
  `.menu.yaml` (`save_menu`) and the manifest's main-form seal.
- [ ] **T2 — Project skeleton** — manifest, `indexed/*.cidx`, `COPYBOOKS/*`, README.
- [ ] **T3 — File-handling COBOL** — open-or-create, COMMIT per change, the five files;
  proven by `test_powerchat_files.rs` as a console program.
- [ ] **T4 — topics-form** — list, create (+ KB collection), select.
- [ ] **T5 — settings-form** — KB location, model list, keys through 076.
- [ ] **T6 — documents-form** — list, import, delete, refresh with the progress overlay.
- [ ] **T7 — chat-form (main shell)** — SideMenu, chat, conversations, tokens.
- [ ] **T8 — home-form** — the month's tokens and conversations.
- [ ] **T9 — Compile test** — `powerchat_compiles.rs`: freshness, parse, semantics, menu hash,
  seal, no secrets.
- [ ] **T10 — Docs and finalize** — Developer's Guide section on the example; CHANGELOG + `z`;
  the operator's run.

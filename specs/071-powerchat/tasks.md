# Tasks — PowerChat, Phase 1

- **Status:** Phase 1 done (1.70.197) — the operator's live run pending
- **Plan:** ./plan.md   **Date:** 2026-09-24

`features` branch. Tests print quantified results (GOLDEN RULE #7).

- [x] **T1 — Regen tool** — `crates/cobolt-ide/examples/powerchat_regen.rs`: for every form in
  the manifest, writes `generated/<form>.cbl` (`cobolt_codegen::generate`), the SideMenu
  `.menu.yaml` (`save_menu`) and the manifest's main-form seal.
- [x] **T2 — Project skeleton** — manifest, `indexed/*.cidx`, `COPYBOOKS/*`, README.
- [x] **T3 — File-handling COBOL** — open-or-create, COMMIT per change, the five files;
  proven by `test_powerchat_files.rs` as a console program.
- [x] **T4 — topics-form** — list, create (+ KB collection), select.
- [x] **T5 — settings-form** — KB location, model list, keys through 076.
- [x] **T6 — documents-form** — list, import, delete, refresh with the progress overlay.
- [x] **T7 — chat-form (main shell)** — SideMenu, chat, conversations, tokens.
- [x] **T8 — home-form** — the month's tokens and conversations.
- [x] **T9 — Compile test** — `powerchat_compiles.rs`: freshness, parse, semantics, menu hash,
  seal, no secrets.
- [x] **T10 — Docs and finalize** — Developer's Guide section on the example; CHANGELOG + `z`;
  the operator's run.

## Deviations from the plan (Phase 1)

- **T8 home-form folded into chat-form.** The month's tokens and conversations
  show in the chat's status line (`PC-STATUS`) rather than on a form of their
  own. One fewer form; same R42 figures.
- **No `indexed/*.cidx` or `COPYBOOKS/` for PowerChat's own files.** Their
  SELECT/FD live in each form's `<file-control>`/`<file-section>`. `.cidx`
  descriptions matter for the user's data the model queries (075), which is
  Phase 2.
- **T3** is proven by `crates/cobolt-ide/tests/powerchat_runs.rs`, which runs
  the real generated programs rather than a separate console copy of their
  file code.
- **Written around three runtime defects found while testing (for `fixes`):**
  1. `ACCEPT x FROM ENVIRONMENT "name"` is misparsed: `ENVIRONMENT` lexes as
     the division keyword, so the source falls back to `DATE` and the leftover
     tokens silently swallow the rest of the procedure. `rcrun check` reports
     OK. PowerChat uses `DISPLAY … UPON ENVIRONMENT-NAME` /
     `ACCEPT … FROM ENVIRONMENT-VALUE`.
  2. `COMMIT` after an `OPEN` that failed panics the interpreter
     (`indexed_disk.rs` "file open"): the unopened file is still committed.
  3. A control property whose value is all digits is `MOVE`d into a `PIC X`
     item right-justified, as if numeric. PowerChat prefixes its
     conversation row ids with `c`.
- **Ambiguity, documented rather than fixed:** a method call written as a
  statement straight after a `MOVE` is parsed as another receiving field.
  PowerChat writes every call statement as `MOVE X::M(...) TO WS-OK`, and the
  guide now warns about it.

# Phase 2

- [x] **P2-1 — Agent mesh** — MODELS capability fields; three agents; election; PLAN →
  WORK → COMPOSE; re-election on change. Test: `powerchat_runs` gains a two-agent
  mesh run against the scripted model (plan, parallel work, composed answer).
- [x] **P2-2 — Registered indexed files** (R20–R25)
- [ ] **P2-3 — Prompt versions** (R48)
- [ ] **P2-4 — Sample topics installer** (Q1)
- [ ] **P2-5 — Six languages** (R44–R46)
- [ ] **P2-6 — Documents folder tree** (R49, R50)

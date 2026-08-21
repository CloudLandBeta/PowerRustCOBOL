# Handoff — only the main form starts an application

Written 2026-08-20, ~09:40 São Paulo, at the end of the session that built it.
Read `CLAUDE.md` first; this covers only this change and what is still open.

> **Later the same session (1.61.113):** two operator-reported IDE fixes landed
> on top, in `crates/cobolt-ide/src/panels/designer.rs` + `i18n.rs` + the guide —
> the Form Designer AI Assistant's pane id (was shared across designers) and its
> clarifying-question answers (went out as bare new requests). They are a
> separate deliverable from the main-form work and are described in the
> CHANGELOG under 1.61.113. Same rule applies: uncommitted, nothing to git
> before 18:00.

## State of the tree — read before you touch git

`main` is at **9a909d3**, exactly where the session started. **Nothing is
committed, nothing is staged, nothing is pushed.** The operator's instruction
was explicit: *nothing goes to git before 18:00* (GOLDEN RULE #1, the São Paulo
window was open all session).

**Another front is live in this same working tree.** A crash-recovery /
autosave feature is being written by a parallel session: `crates/cobolt-ide/src/crash.rs`
(untracked) plus roughly 108 lines inside `crates/cobolt-ide/src/app.rs`
(`pending_recovery`, `last_autosave`, `show_recovery_prompt`). During this
session it also removed a batch of untracked translation drafts and edited
`docs/observability{,-cn,-es,-fr,-jp,-pt}.md`. **Never `git add -A` here.**

Worse, three files carry *both* fronts and cannot be separated by path:

| File | Mine | Not mine |
|---|---|---|
| `crates/cobolt-ide/src/app.rs` | upgrade dialog, `reseal_project_designation`, 4 call sites | crash recovery / autosave |
| `docs/developers-guide-en.md` | the main-form section + CLI flag/exit-code rows | a large pre-existing edit |
| `README.md` | one clause in the Form-lifecycle bullet | pre-existing edits |

Splitting those needs `git add -p`, or an agreement with the other session.

### Mine, in full

New — `crates/cobolt-compiler/src/main_form_guard.rs`,
`crates/cobolt-ide/src/project_upgrade.rs`,
`crates/cobolt-cli/tests/main_form_gate.rs`.

Modified — `crates/cobolt-compiler/{Cargo.toml,src/lib.rs}`,
`crates/cobolt-cli/src/{form_gui.rs,main.rs}`,
`crates/cobolt-ide/src/{app.rs*,form_runtime.rs,i18n.rs,main.rs,project_model.rs,version.rs}`,
`Cargo.lock`, `CHANGELOG.md`, `README.md*`, `docs/developers-guide-en.md*`,
`assets/knowledge/chunked.data`. (`*` = shared with the other front.)

## The rule, and where it lives

**Only the main form starts an application.** The IDE runs any form — that is
what a designer is for. `rcrun` and a built binary open the project's main form
and nothing else, so a sign-on main form cannot be stepped over.

| Where | What it does |
|---|---|
| `cobolt_compiler::main_form_guard` | The one resolver + the seal. `read_designation` (which form is main, from the `.cfrm` marks, first-form fallback for pre-037 projects), `seal`, `authorize_form_start` → `Allowed` / `AllowedUnsealed` / `Refused` / `Corrupt`. |
| `rcrun run-form` (`form_gui.rs::enforce_main_form`) | Acts on the verdict **before** loading, parsing or drawing. Exit **3** corrupt, **4** not-main. `--designer` exempts and says so on stderr. |
| `rcrun build` (`lib.rs`) | Orders the embedded form table by the same resolver; `CompilerError::AmbiguousMainForm` refuses a project where two forms claim the mark. |
| The built binary (`generate_main_rs`) | Bakes `const MAIN_FORM` beside the form table and checks `FORMS[0]` against it at startup → `CORRUPTED APPLICATION`, exit 3. Emitted even when the table is empty (a generated program that compiles only sometimes is not a generated program — that bug was caught by `generated_binary_source_actually_compiles`). |
| The IDE | Passes `--designer` (`form_runtime.rs`); `save_project` restates the seal; `reseal_project_designation` is called from `apply_main_form_invariant`, `after_form_saved` and the 037 R2 settlement. |

The designation is recorded **twice** — the `main-form` mark inside the `.cfrm`,
and `[forms] main-form` + `main-form-seal` in the project file. Disagreement is
corruption. The seal's key is a constant in `main_form_guard.rs`: it is
**tamper-evidence, not tamper-proofing**, and the module says so.

## Project structure upgrades — offered, never imposed

`crates/cobolt-ide/src/project_upgrade.rs`. `[project] structure` numbers the
shape of a project file; `CURRENT_STRUCTURE` is what this IDE writes and new
projects are born at it. Anything lower is **offered** the steps in between, in
a modal on project open, in all six languages.

The promise "declining changes nothing" is enforced in code, not prose:
`save_project` seals only at `structure >= STRUCTURE_MAIN_FORM_SEAL`, and
`reseal_project_designation` returns early below it — so opening an older
project does not even rewrite its `cobolt.toml`.

**To add the next upgrade:** a `STRUCTURE_*` constant one above the last (raise
`CURRENT_STRUCTURE`), a unit struct implementing `ProjectUpgrade` (`applies`
reads the project as it is on disk, not just the number; `apply` makes the
change and raises `structure`), one line in `UPGRADES` in ascending order, and
two `Tr` fields in all six languages. The dialog, ordering, save and partial-run
handling come with it. `the_registry_is_in_ascending_order` guards the order.

## Verified

```bash
cargo test -p cobolt-compiler --lib main_form          # 10 passed
cargo test -p cobolt-cli --test main_form_gate         #  4 passed (drives the real rcrun binary)
cargo test -p cobolt-ide --bin cobolt-ide project_upgrade   # 6 passed
cargo test -p cobolt-ide --bin cobolt-ide main_form_seal_tests  # 1 passed
cargo test -p cobolt-ide --bin cobolt-ide i18n         #  4 passed
```

Full sweeps, both green and **both including the other front's work**:
`cargo test -p cobolt-compiler --lib --no-fail-fast` → **80 passed, 0 failed**
(246 s — it really compiles a generated binary);
`cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` → **827 passed,
0 failed, 3 ignored**.

The CLI tests drive the real `rcrun` and never reach a window: refusal and
corruption exit before the form is read, and the one case that must pass the
gate is pointed at a `.cbl` that does not exist, so it dies at the next step.

`assets/knowledge/chunked.data` was rebuilt (Metal, 1132 records) after editing
the `MainForm` KB constant in `cobolt-compiler/src/lib.rs`;
`prebuilt_chunked_kb_matches_the_published_documentation` is green.

Release binaries are current: `target/release/cobolt-ide` (1.61.112) and
`target/release/rcrun`. **They must ship together** — a new `rcrun` with an old
IDE would refuse Run Form on secondary forms.

## Open

1. **Commit and push after 18:00**, on a branch (the hook refuses a commit on
   `main`), selecting paths — see the shared-file table above. Classification is
   the operator's call and was left open: the gate reads as a **fix** (f=97),
   the upgrade mechanism as a **feature** (f=96). They are now **one deliverable
   at 1.61.112** with a single CHANGELOG entry, because the gate's
   `save_project` condition depends on `project_upgrade::STRUCTURE_MAIN_FORM_SEAL`.
   Splitting them means inverting that dependency (seal unconditionally first,
   add the gating in the feature commit). Forum posts follow GOLDEN RULES #4/#4b
   **after** the merge to `main`.
2. **The guide delta is English-only.** `docs/developers-guide-en.md` carries
   the new section; the five translations do not. This falls under the standing
   guide debt recorded in `CLAUDE.md` (es/pt/jp/cn stale, fr missing) — but
   GOLDEN RULE #8 is not satisfied until they carry it.
3. **Residual hole, decided knowingly.** Removing *both* `[forms] main-form` and
   `main-form-seal` drops a project back to legacy mode, where the mark can be
   moved consistently and only a warning is printed. Closing it means refusing
   unsealed projects outright, which breaks every project until it is upgraded.
   The operator chose to leave it.
4. **`--designer` is forgeable** and stays that way (settled). The real mitigation
   is deployment shape: a built binary has no `run-form` and no `.cfrm` on disk.
   If the operator ever ships *source projects* + `rcrun`, the idea on the table
   is a project setting "refuse designer runs" covered by the seal.

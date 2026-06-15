<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Technical steering — PowerRustCOBOL

> Stack, conventions, and hard constraints. `/plan` must respect this file.

## Stack

- **Rust** workspace (`crates/*`), edition per workspace `Cargo.toml`.
- **egui / eframe 0.29** for the IDE UI (immediate-mode GUI; multi-viewport via
  `ctx.show_viewport_immediate`).
- Tree-walking COBOL runtime (no external COBOL toolchain at runtime).

## Crates (roles)

| Crate | Role |
|-------|------|
| `cobolt-lexer` | Tokeniser (fixed/free source formats). |
| `cobolt-parser` | Parser → AST + diagnostics. |
| `cobolt-ast` | AST types. |
| `cobolt-semantic` | Semantic analysis. |
| `cobolt-runtime` | Tree-walking interpreter, indexed-file engines (in-mem, redb). |
| `cobolt-stdlib` | Intrinsics / built-ins. |
| `cobolt-forms` | Form model (`.cfrm`), controls, load/save. |
| `cobolt-codegen` | Form → COBOL generator (`generate`, `write_header`). |
| `cobolt-compiler` | Single-binary build (`build_project`). |
| `cobolt-media` | Media helpers. |
| `cobolt-ide` | The egui IDE app. |
| `cobolt-cli` | `rcrun` CLI. |

## Build / test / run

```sh
cargo build -p cobolt-ide          # build the IDE
cargo test  -p <crate>             # test a crate
cargo run   -p cobolt-ide          # launch the IDE
```

Always build **and** test the touched crates before declaring a task done.

## Hard constraints (non-negotiable)

- **i18n:** every user-facing IDE string is a `Tr` field translated in **all six**
  languages (EN/ES/PT/JA/ZH/FR) in `crates/cobolt-ide/src/i18n.rs`. No hard-coded
  UI literals.
- **Generated COBOL:** every RAD-generated `.cbl` starts with the developer
  banner (`cobolt-codegen::write_header`) and is **regenerated on Build / Run /
  Debug / Check** (`App::regenerate_all_forms`). Never hand-edit generated code.
- **COBOL identifiers/source stay English**; UI text never says "cobolt".
- **Docs:** the **English** `docs/developers-guide-en.md` is canonical and kept
  current. The `-es/-pt/-jp/-cn` translations are **user-maintained — never edit
  them**.
- **Tests:** user-provided tests are *report-or-fix*, never silently changed.
  New tests report quantified, human-readable results. **Verify-first** — never
  assert a measurement the run didn't produce.
- **Versioning:** features bump the **minor** (`y`) in
  `crates/cobolt-ide/src/version.rs` + a `CHANGELOG.md` entry; fixes are `z`.
- **Commits:** never mix fixes and features in one commit. Respect the push
  window and forum-announcement rules in the operator's CLAUDE.md.

> The operator's full golden rules live outside the repo (CLAUDE.md). When in
> doubt, ask rather than guess.

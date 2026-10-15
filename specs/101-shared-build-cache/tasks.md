# Tasks — builds share one compiled-crate cache

- **Status:** done (the review gate of /tasks was taken with /implement, by the operator's choice, 2026-10-15)
- **Plan:** ./plan.md   **Date:** 2026-10-15

Every verification builds with `CARGO_TARGET_DIR=/Users/emersonlopes/Documents/PowerRustCOBOL/.claude/worktrees/fixes/target CARGO_INCREMENTAL=0`. Real-build tests take minutes and gigabytes: run them one test at a time and check `df -h /` first.

- [x] **T1 — The cache module, without cargo** (R5, R6, R15, R4, R8)
  - Files: `crates/cobolt-compiler/src/build_cache.rs` (new), `lib.rs` (`mod build_cache;`).
  - Do: `cache_dir`, `platform_state` (sha256 per SDK crate + newest file time), `verdict` (the table), `read_marker`/`write_marker` (temp + rename), `has_artefacts`, `seed_lock`, `is_lock_wait_line`, `stale_platform` (state + markers → stale list, markers to record).
  - Verify: `cargo test -p cobolt-compiler build_cache` green; the verdict test prints the table.

- [x] **T2 — Build into the shared folder** (R1, R9, R10, R12, R14)
  - Files: `lib.rs` (`build_core`, `BuildResult`, `BuildOptions` doc), the two profile tests.
  - Do: `target_dir` from `build_cache::open` or the private folder with a log line on failure; `CARGO_TARGET_DIR` and the install path use it; `BuildResult.target_dir` and `.forced_full`.
  - Verify: `cargo build -p cobolt-compiler`; `profile_probe` tests and `an_unchanged_program_recompiles_nothing` green.

- [x] **T3 — Same versions in every program** (R4)
  - Files: `lib.rs` (`build_core`).
  - Do: seed `Cargo.lock` from the workspace root when the staging folder has none; on a failed first attempt with nothing compiled, drop the seed and run again offline.
  - Verify: unit test of `seed_lock`; `test_external_crates_e2e` green.

- [x] **T4 — A stale platform crate forces a full build** (R5, R6, R15)
  - Files: `lib.rs` (`build_core`), `build_cache.rs`.
  - Do: the check before the staging folder is touched; `full = opts.full || stale`; after the manifest and lock are written, `cargo clean -p` of every SDK crate in the staged lock, both profiles, "did not match" skipped; markers written; the log names the crates.
  - Verify: the real-build tests of T7 (stale and not stale).

- [x] **T5 — Waiting is said** (R8)
  - Files: `lib.rs`. Do: log once on cargo's `Blocking waiting for file lock` line.
  - Verify: the line test of T1; the concurrent build of T7 shows it or finishes without error.

- [x] **T6 — The old private folder goes** (R11)
  - Files: `lib.rs`. Do: after a good shared build, `discard_dir` the staging `target/` and log the bytes freed.
  - Verify: the T7 sharing test plants a `target/` and finds it gone.

- [x] **T7 — Real-build tests** (AC1–AC6, AC8, AC13, AC14, AC4)
  - Files: `lib.rs` tests, plus the `stage_sdk` lock test.
  - Do: the three tests of plan §6, each in its own build root, printing every phase's crate count.
  - Verify: each passes alone, then `cargo test -p cobolt-compiler`.

- [x] **T8 — Words in the IDE** (R13)
  - Files: `ide_text_data.rs` (one text ×6), `app.rs` (`show_build_folder`), `i18n.rs` (`stale_build_body` ×6).
  - Verify: `cargo test -p cobolt-ide i18n` and `every_text_exists_in_six_languages`, `no_english_word_reaches_a_widget…` green.

- [x] **T9 — The CLI's words** (R13)
  - Files: `crates/cobolt-cli/src/main.rs`, `BuildOptions::full` doc. Verify: `cargo test -p cobolt-cli`.

- [x] **T10 — Developer's Guide** (R13)
  - Files: `docs/developers-guide-en.md` (the four places of plan §2). Verify: read back; no Rust named; the registry guard tests in `docs_embed.rs` green.

- [x] **T11 — Finalize**
  - Files: `version.rs` (1.91.160), `CHANGELOG.md`, `spec.md` (acceptance criteria), this file.
  - Do: the sweeps of plan §6 (batches, disk checked), the AC12 measurement run by hand with its numbers in the CHANGELOG, every AC checked with what proved it.
  - Verify: all touched crates build and test; the numbers in the CHANGELOG are the ones the runs printed.

## Done criteria
All acceptance criteria in spec.md are checked with what proved them, tests pass, the Guide is updated, and the change is one `fixes` commit (not committed or pushed unless the operator asks).

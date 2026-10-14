# Tasks — the build folder

- **Status:** implemented in 1.91.154   **Plan:** ./plan.md   **Date:** 2026-10-14

- [x] **T1 — resolution in the compiler** (R2–R4; AC1, AC2) — `lib.rs` + four unit tests (serialised with the real-build guard, because the override and the environment are global to the process). `cargo test -p cobolt-compiler --lib` on the five build-folder tests: 5 passed.
- [x] **T2 — the preference** (R5; AC3) — `ui_prefs.rs` + two tests. `cargo test -p cobolt-ide --bin cobolt-ide ui_prefs`: passed.
- [x] **T3 — the check and the hand-over** (R6, R7; AC4, AC5) — `check_build_root_usable` + `BuildRootProblem` (tested); the IDE calls `set_build_root_override` at start-up and from the dialog, and passes `build_dir` to its one `BuildOptions`.
- [x] **T4 — the dialog and the menu item** (R1, R8, R9; AC6) — `show_build_folder`, `Item::BuildFolder`, nine texts × 6 languages in `ide_text_data.rs`. `ui_text`, `menu_layout` and `i18n` tests: 30 passed.
- [x] **T5 — one real build** (AC7) — `a_build_lands_in_the_chosen_folder_and_leaves_the_default_alone`, 24 s.
- [x] **T6 — Guide** (R9) — "Where your builds are kept (Settings ▸ Build Folder)" in chapter 18, the Settings table (six items) and the ToC.
- [x] **T7 — finalize** — version 1.91.154, CHANGELOG; commit on `features`.

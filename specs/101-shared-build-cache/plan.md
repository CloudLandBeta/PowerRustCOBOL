# Plan — builds share one compiled-crate cache

- **Status:** implemented in 1.91.160 (approved by the operator, 2026-10-15, by running /implement)
- **Spec:** ./spec.md   **Date:** 2026-10-15   **Branch:** `fixes`

## 1. Approach
All in `cobolt-compiler`, in a new module `build_cache.rs` (the crate's `lib.rs` is 11,000 lines; the new logic is mostly pure functions that can be tested without cargo), called from `build_core` at five points. Three changes in the IDE and the CLI are words only.

1. **One target folder (R1, R2, R3, R9, R12).** `cache_dir(build_root) = <build root>/shared-target`. `build_core` sets `CARGO_TARGET_DIR` to it instead of `<staging>/target`, and installs the binary from `<shared-target>/<profile>/<exe>`. Debug and release keep their own sub-folders, as cargo makes them. The staging folder keeps the generated crate and its `Cargo.lock`. `BuildResult` gains `target_dir` (where this build's artefacts are) and `forced_full` (the platform crates that made the build a full one, empty normally); the two tests that read `build_dir/target/<profile>` read `target_dir` instead.
2. **Same versions in every program (R4).** After the manifest is written, if the staging folder has no `Cargo.lock` and `<workspace root>/Cargo.lock` exists, copy it in. Measured: cargo takes it offline, keeps the SDK's versions and prunes what the program does not use. If the build then fails with nothing compiled (`compiled == 0`) while a seed was used, delete the seeded lock and run the same offline build again, then the existing online retry as before. The existing retry keys on the word `--offline` in the error, which a pinned-but-missing version does **not** print (measured), so the seed needs its own retry.
3. **Stale platform crates force a full build (R5, R6, R15 — the operator's rule).** *Before* the staging folder is touched, for each of the fourteen `SDK_CRATES`:
   - hash its files (relative path, length, bytes, sorted; `target/` excluded) with `sha2`, which `cobolt-compiler` already uses (files whose name starts with a dot, such as `.DS_Store`, and `target/` are left out), and for `cobolt-form-host` the icon it bakes in (the one compile-time asset outside the crates — a test proves it is the only one); note the newest file time. The root `Cargo.toml` is deliberately **not** hashed: a dependency added for another crate would otherwise make every platform crate look changed;
   - compare with the marker `<shared-target>/platform/<crate>.sha256` (the hash; its own file time is when it was written). The verdict is a pure function:

     | marker | hash | files newer than the marker | artefacts in the cache | verdict |
     |---|---|---|---|---|
     | same | same | — | — | **Fresh** |
     | none | — | — | none | **Record** — write the marker |
     | differs | differs | yes | — | **Record** — cargo sees the newer files itself |
     | differs | differs | no | — | **Stale** — the files look old, so cargo would believe the old units |
     | none | — | — | some | **Stale** |

   - **If any crate is Stale, the build is a full build** (`full = opts.full || !stale.is_empty()`), and the log names the crates. Whether a stale crate matters is decided after the manifest and lock are written: `cargo tree --offline` in the staging folder names the platform crates *this program* uses (and prunes its lock), and only a stale crate in that graph forces the build; a crate stale in the cache but unused here is left for the program that uses it. A full build is what R5 says: an explicit one discards the staging folder as today, and either kind cleans every platform crate in the program's graph from the cache with one `cargo clean -p a -p b … [--release] --offline` per profile, run in the staging folder (measured: it removes every compiled variant of each named crate and nothing else). Any failure of cargo there is R14's. Third-party crates stay; the program is rebuilt because its staging folder is new. Fresh and Record crates are not cleaned; their markers are written (Record) before the build starts, so a build that fails on the developer's own COBOL does not repeat the clean.
   - The ordinary upgrade, with truthful file times, is **Record** for the changed crates: no forced full build, and cargo rebuilds exactly what changed and what depends on it. That keeps the platform developer's incremental builds.
4. **The three small ones.** *Waiting (R8):* the stderr loop that counts `Compiling` lines also recognises `Blocking waiting for file lock` (cargo 1.95 prints `… on artifact directory`; the match is the prefix) and logs once: *"Another build is using the compiled-crate cache — waiting for it to finish…"*. *Old folders (R11):* after a successful build in the shared cache, if `<staging>/target` exists, `dir_size` it, `discard_dir` it (rename aside, then delete — the existing incident-proof helper) and log the bytes freed. *Degrade (R14):* `build_cache::open` creates `shared-target/platform`; any failure in it, in hashing, or in a clean makes the build use `<staging>/target` exactly as today and log why. The cache can only make a build faster.

## 2. Affected crates / files
- `crates/cobolt-compiler/src/build_cache.rs` — **new**: `cache_dir`, `open`, `crate_state` (hash + newest time), `check_platform`, `verdict` (pure), `has_artefacts`, `sdk_crates_in_graph`, `clean_platform`, `read_marker`/`write_marker` (a unique temporary file renamed into place — two builds in one process wrote the same name until a test caught it), `seed_lock`, `is_lock_wait_line`, `human_size`.
- `crates/cobolt-compiler/src/lib.rs` — `mod build_cache;` `BuildResult.target_dir` and `.forced_full`; `build_core`: stale check and promotion to full, target folder, seed, clean, retry-without-seed, lock-wait line, install path, old-`target/` removal; the wording of `BuildOptions::full`; tests.
- `crates/cobolt-cli/src/main.rs` — the `--full` help line.
- `crates/cobolt-ide/src/ide_text_data.rs` — one added text (×6) for the Build Folder dialog; `crates/cobolt-ide/src/app.rs` — `show_build_folder` shows it under the existing note; `crates/cobolt-ide/src/i18n.rs` — `stale_build_body` (×6) no longer says "recompiles everything".
- `docs/developers-guide-en.md` — "Where your builds are kept" (what the working folder holds now, the new shared cache, safe to delete, the caveat about a new folder), "Full builds and the recorded version" (what a full build discards, and that a build can become one by itself), the note at line 15360, the `rcrun build --full` text. No translation exists.
- `crates/cobolt-ide/src/version.rs` (1.91.160), `CHANGELOG.md`, `specs/101-shared-build-cache/{spec,plan,tasks}.md`.
- **Not touched:** the generated `Cargo.toml` profile (R10, AC9), the System KB (no property, method or event changes), `built_with_version` and the stale-build prompt's logic, `external_crates.rs`.

## 3. Data / model changes
On disk, inside the build folder: `shared-target/` (cargo's own layout: `debug/`, `release/`, `.rustc_info.json`) and `shared-target/platform/<crate>.sha256` (one hex line each). Nothing in a project, in `cobolt.toml`, in `ui.toml`, or in a `.cfrm`. A folder from the old layout (`cobolt-build-<name>/target`) is left alone until that program's first good shared build removes it (R11). Deleting `shared-target` is always safe: the next build is one cold build. `BuildResult` gets two `pub` fields; its only constructor is in `lib.rs`.

## 4. Key decisions & alternatives
- **Stale means a full build (operator).** Instead of choosing which crates to clean, any stale platform crate promotes the build to the full build the project already has. Simpler to explain and to trust than a per-crate scheme, and the full build is the project's existing answer to "it behaves oddly since I updated". The cost is bounded: only the platform crates and the program are compiled again, not the ~490 third-party ones (Q8).
- **Detection per crate, action for the whole platform.** The markers are per crate so the log can name what changed; the response is not per crate.
- **The file-time guard.** Files newer than the marker are left to cargo (**Record**), so a normal edit is never a forced full build and incremental debug builds survive.
- **`cargo clean -p` from the program's staging folder.** Measured to match exactly the crates in the staged lock, with the lock untouched, and the build owns that folder. Rejected: cleaning from the SDK workspace (in a shipped SDK `--locked` is refused and without it cargo rewrites the SDK's own lock — measured); deleting artefact files ourselves while holding cargo's lock (re-implements cargo's file naming); a cache generation per PowerRustCOBOL version (third-party crates rebuilt every release — the cost we are removing).
- **Markers are written before the build, not after.** After a clean (or when nothing exists to clean) the marker is true from then on; writing it after a successful build would repeat the full build after every build that fails on the developer's COBOL.
- **A seed that can be dropped.** The SDK lock is a preference, not a requirement: dropping it on a failed first attempt costs one retry and keeps a machine with another registry cache building offline, as today.
- **Not cleaning the program on a full build.** Its staging folder is rewritten, so cargo sees new sources.
- **The folder name `shared-target`**, directly in the build root, so spec 100's setting moves it with everything else (Q2).
- **Rejected for now (spec §2):** `sccache`/content-hash freshness for retouched-but-identical sources, pruning, a "clear cache" button, any change to the release profile.

## 5. Risks & mitigations
- **The cache's own failure modes stay out of the build's way.** → R14: any error in `build_cache` falls back to the private folder with a log line. Test: AC13.
- **A restored-old-files upgrade is a full platform rebuild by design (R15).** → the log says why, as the IDE's full builds already do; the third-party crates are kept.
- **Seeding versus pinned External Crates (spec 044).** A pin that conflicts with the seed makes cargo choose another version, but the e2e test asserts "exactly one `serde`" and path-resolved pins. → `test_external_crates_e2e` is run in full and must stay green; if the seed makes it fail, the build retries without it (compiled == 0), and the e2e failure is reported, not worked around.
- **Two builds at once take turns.** Cargo locks the cache. → R8's log line; nothing fails (Q7). The tests' `HEAVY_BUILD` still serialises the real builds of one test process.
- **Disk.** The cache grows with each toolchain, profile and feature variant and is never pruned (Q5); the old per-program folders go (R11). → the Guide says the folder may be deleted; AC12 reports the bytes before and after.
- **The hash walks 18 MB on every build.** → measured in AC12 (expected tens of milliseconds, to be reported, not assumed).
- **Windows and Linux were not run here.** The code uses `std` paths, `Command` and `rename` (as `discard_dir` does), and cargo's lock is a file lock on each. The release workflow (`build-all-platforms.yml`) builds and stages the SDK on every platform but runs no tests, so the new tests run on the developer's machine only — macOS here. The CHANGELOG entry says that, as earlier build changes did.
- **The tests share the default build root.** A test that builds with the default root now leaves compiled crates in the real `shared-target`, which the next run reuses (faster) and which the developer's own builds share. The tests that count crates use their own root so a warm default cache cannot change their numbers.

## 6. Test strategy
**Without cargo (fast, in `build_cache.rs`):**
- `platform_hash_ignores_order_and_target_and_sees_one_changed_byte`.
- `the_verdict_follows_the_table` — every row of §1's table, printed as a table with the count passed (GOLDEN RULE #7).
- `has_artefacts_matches_whole_crate_names` — `cobolt-forms` is not `cobolt-form-host`.
- `a_marker_is_replaced_whole` and `seed_lock_copies_only_when_absent`.
- `the_lock_wait_line_of_cargo_1_95_is_recognised` — on the line captured on 2026-10-15.
- `a_staged_sdk_holds_the_lock_the_source_tree_has` (`stage_sdk`; no test says so today).

**Real builds (in `lib.rs`, under `HEAVY_BUILD`, each in its own build root and, where a platform crate is edited, its own SDK copy made with `stage_sdk`; each prints the crate count of every phase):**
1. *Sharing:* program A cold (count printed), program B: 1 crate (AC1); A again: 0 (AC2); A's binary unchanged after B (AC8); a planted `A/target` is removed and the bytes freed are logged (R11).
2. *Platform freshness:* edit one line of a leaf platform crate with a newer file time → that crate, its dependants and the program, no third-party, `forced_full` empty (AC3, AC5); edit with a file time older than the marker → the build is forced to be full, names the crate, no third-party crate is compiled, and the new code runs (AC14, AC5); every source unchanged → never forced; delete `shared-target` → the next build succeeds cold (AC3).
3. *Safety:* two threads build two programs at once, both succeed, no "No such file" (AC6); a plain file where `shared-target` should be → the build succeeds in its own folder and says why (AC13); a seeded lock that pins an unavailable version is dropped and the build succeeds (AC4).

**Kept green:** `an_unchanged_program_recompiles_nothing`, `a_build_lands_in_the_chosen_folder_and_leaves_the_default_alone` (it will also see `shared-target` in the chosen folder), `test_external_crates_e2e`, `test_esql_build`, `test_national_build`, the `discard_dir_tests`, `every_text_exists_in_six_languages`, `no_english_word_reaches_a_widget…`, `cargo test -p cobolt-compiler`, `-p cobolt-ide`, `-p cobolt-cli`, `-p cobolt-project-tools`.

**Measured (AC12, an `#[ignore]`d test run by hand, numbers into the CHANGELOG):** PowerDemo3 cold, then another form program warm — wall-clock, crates compiled, the stale check's own time, and the bytes in `shared-target` against the sum of the old private folders.

**Manual / visual:** the Build Folder dialog shows the new line in each of the six languages (the IDE window is not driven from the tools; the operator looks).

## 7. Steering compliance
- [x] i18n: one added dialog text and one reworded `Tr` field, all six languages.
- [x] Generated-code banner + regenerate-on-action contract: untouched (the staged `Cargo.toml` differs only by the seeded lock beside it).
- [ ] English dev guide updated (translations untouched — none exist) — a task.
- [x] Fix vs feature: **fix** (operator, 2026-10-15) → `fixes`, `z` bump to 1.91.160, CHANGELOG entry; no forum post unless asked.
- [x] No "cobolt" in user-facing text: the log lines and dialog say "compiled-crate cache", "platform"; the folder is `shared-target`.
- [x] Cargo-side deletes only through cargo (R7); the one tree we remove ourselves is a program's own old `target/`, through `discard_dir`.
- [x] No script is added to the repository; the experiments ran in the scratchpad.

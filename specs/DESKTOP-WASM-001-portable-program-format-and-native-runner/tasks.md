<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — The WebAssembly module as the portable program format: native builds without a Rust toolchain per program (DESKTOP-WASM-001)

- **Status:** implementing since 2026-11-14. F1–F3 and T1–T12 are done (2.0.483–2.0.497, with the fix 2.0.492 found by T8): every task of M0, and the first four of M1. The CI dispatches of T4 and T7 are owed until their workflows are on `main`.
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-14
- **Branch:** `features-2x-desktop-wasm` (feature), cut from `main` at 2.0.484. The spec commits were copied onto `main` (2.0.478–2.0.482), and `features-2x-wasm-program-format` is kept as it was. The two fixes F1 and F2 went on `fixes-2x-wasm-emit-order`, cut from `main`, and are in `main`.

These are small, ordered tasks that can each be checked on their own. Each one names:
- the files it touches;
- the requirements it satisfies;
- how it is verified.

Every test that measures prints one result block (GOLDEN RULE #7): the cases it ran, the counts, and for timed ones the milliseconds and the throughput. Real measurements only. Any throwaway tooling stays outside the repository (PRIME DIRECTIVE).

**Order.** The two fixes come first (#5). Then the milestones of plan §1.3, M0 to M6, then closing. The workspace builds and its tests stay green after every task. Inside M0 the order matters: the frames of today's path are recorded before anything in the form host changes (see "Refinements" below).

**Each task as it lands:**
- bumps `z` in `crates/cobolt-ide/src/version.rs`;
- adds a top `CHANGELOG.md` entry;
- updates this spec's row in `specs/WASM-PROGRESS.md`;
- is committed on its branch and pushed outside the forbidden window (GOLDEN RULE #1).

Tasks committed together share one bump. Nothing merges into `main` unless the operator asks (operator, 2026-11-13: "do not merge into main, push it in its own branch for now").

**Each milestone closes with** its "close" task, which:
- runs `cobolt-bench pixel-parity` and `cobolt-bench build-goal` over the reference projects;
- runs the AC9 measurement (NIST on both axes, then the `tests/cobol` corpus);
- updates the Developer's Guide's list of what still takes the Cargo path, deletes its five translations (#8) and regenerates the System KB;
- records in `WASM-PROGRESS.md`, per project: the path taken, the fallback reason, the AC19 result and the slowest Build against the 30 s goal.

**Before every long build:** `df -h /` shows at least 10 GB free. Reclaim space from stale `target/` folders and merged branches, never from uncommitted work.

### Refinements found while tasking

The code says five things the plan did not settle. Each is applied in the tasks below.

1. **The golden hashes need repeatable frames.** Today's frames repeat only where nothing on screen depends on time, because `host.rs` reads the real clock. So M0 proceeds in four steps:
   1. It adds the runtime pins (T5) and a deterministic loop in `headless.rs` alone (T6), with no change to `host.rs`.
   2. It records goldens for every form whose frames already repeat, checked by running each script twice (T7).
   3. Then it adds `HostClock` (T8).
   4. Last, it checks those goldens still match and records the rest. Each recording names its forms in its CHANGELOG entry.

   AC19's second guard therefore holds every form that could be measured before the refactor.
2. **The reference projects have no check script.** None of the five has a headless script (checked: no `.json` outside `agentic_ai/`). AC1's "the project's own check script" is therefore the base script of its main form, written in T7.
3. **The console part of R27 needs console programs.** The five reference projects all have forms. So at M1, R27's console part is judged on console fixtures kept in `tests/pixel-parity/console/` (T24). The `tests/cobol` corpus is measured too (AC9).
4. **The 30 s goal is first judged at M2.** No reference project takes the WebAssembly path at M1 (each has forms). The ten-times project has at least 10 indexed files, so it moves at M3 at the earliest. Until a project moves, `build-goal` reports its fallback reason in place of a WebAssembly time.
5. **The Developer's Guide applies now.** `main` has carried the 2.x line since 2026-11-11 (`WASM-PROGRESS.md`, operating rules). So GOLDEN RULE #3 holds: the guide changes in the same task as behaviour a developer sees, and its translations are deleted (#8).
   - The headless script format is not in the guide (checked: no `PRC_HEADLESS` there). The steps `shot` and `size` are documented in `headless.rs`'s step table, beside the other steps.
   - This corrects plan §7, which put them in the guide.

**Commands used below:**

| What | Command |
|---|---|
| generator | `cargo test -p cobolt-wasm <filter>` |
| ABI | `cargo test -p cobolt-wasm-abi` |
| Web bundle | `cargo test -p cobolt-web-bundle` |
| Web fixtures | `cargo test -p cobolt-web-tests <filter>` |
| runtime | `cargo test -p cobolt-runtime <filter>` |
| form host | `cargo test -p cobolt-form-host <filter>` |
| forms | `cargo test -p cobolt-forms --features render` |
| compiler | `cargo test -p cobolt-compiler <filter>` |
| new crates | `cargo test -p cobolt-runner-engine`, `-p cobolt-program-package`, `-p cobolt-runner`, `-p cobolt-abi-host` |
| player (outside the workspace) | `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib` |
| `rcrun` | `cargo build -p cobolt-cli --bin rcrun` |
| `rcrun` host tests | `cargo test -p cobolt-cli --test <name>_hosts` |
| IDE | `cargo test -p cobolt-ide --bin cobolt-ide <filter>` |
| project tools | `cargo test -p cobolt-project-tools <filter>` |
| bench | `cargo run -p cobolt-bench --release -- build-goal …` and `… -- pixel-parity …` |
| NIST on the generator | `cargo run -p cobolt-wasm --example nist_wasm -- census <MOD>` (compile axis); `nist_wasm_run` as `RCRUN` for `nist_conformance run <MOD>` (execution axis) |
| System KB | `cargo run -p cobolt-ide --example build_chunked_kb` |

**The three window sizes of AC19:** the form's designed size; **Medium**, 800 px wide; **Compact**, 480 px wide. Medium and Compact keep the designed height. The widths fall inside the default breakpoints (`cobolt-forms/src/layout/breakpoints.rs`: Compact [0, 600), Medium [600, 1024)).

**Out of scope:** the stale worktree `fixes-2x-motion-lockstep` (another session's staged fix at 2.0.454). It is not touched.

---

## Phase F — The two fixes, landed first (`fixes-2x-wasm-emit-order`)

**Before F1, one question for the operator: how the fixes reach this branch.**
- **The constraint:** this branch is pushed, so CLAUDE.md allows it only `--ff-only`. A fix branch cut from today's `main` can never be fast-forwarded into it, because this branch carries the spec commits.
- **Recommended: every step a fast-forward.**
  1. The operator merges this branch's spec commits (2.0.478–2.0.482: the spec documents, CHANGELOG entries and version number only) into `main`.
  2. The fix branch is cut from that `main`.
  3. Once the fixes are on `main`, this branch follows with `git merge --ff-only main`.
- **Otherwise:** the fixes wait on their branch. Only T22 (determinism across Rust releases) needs them; every other task can go ahead.

- [x] **F1 — The generator emits in a fixed order** (R2; plan §4.9)
  - *Done 2026-11-14 (2.0.483):*
    - **Fixed:** `alter_slots` is a `BTreeMap`.
    - **The audit:** about 50 maps and sets, of which 13 iteration sites were read. One other order-dependent walk was found: `sort_file_names`' `records.iter().find` (lower_idx.rs:101). It is unreachable, because a record name shared by two files is refused at `build_files` ("has no record description"), and the second fixture pins that.
    - **The test:** `compile_determinism` failed on the unfixed tree (the ALTER fixture differed at compile 2 of 20, in both variants). After the fix, 126 of 126 variants give the same outcome every time (38 identical modules, 88 identical refusals; 2 520 compiles in 24.9 s).
    - **Unchanged:** the 18 corpus modules are byte-identical to before. The census gives NC 95/95, IF 45/45, IC 47/47 members.
    - **Reds outside the fix:** the crate's other tests are green except two. `corpus` fails on a timing line its clock filter misses (`conditionals/test-arith-relation-after-and.cbl`). `dependency_guard` expects Rust 1.92 at most, while the workspace has declared 1.95 since 2.0.469.
  - Files:
    - `crates/cobolt-wasm/src/lower.rs`: `alter_slots` becomes a `BTreeMap` (:259), walked by `init_alter_slots` (:1084);
    - every other `HashMap` or `HashSet` walked while emitting, in `crates/cobolt-wasm/src/*.rs`;
    - new `crates/cobolt-wasm/tests/compile_determinism.rs`.
  - Do:
    - **The audit:** list every `HashMap` and `HashSet` iteration in the generator. Each one in the evidence is either changed to an ordered walk or shown to have no effect on the emitted bytes.
    - **The test `same_input_same_bytes`** compiles 20 times each:
      - an ALTER fixture with at least eight `ALTER` targets in three paragraphs;
      - every program of `tests/cobol/` the generator accepts.
    - **Before the change,** record the hash of every module the corpus produces. Modules with no `ALTER` must keep their bytes.
  - Verify:
    - `cargo test -p cobolt-wasm --test compile_determinism -- --nocapture` prints "programs N, compiles 20 each, identical N of N".
    - The ALTER fixture fails this test on the unfixed tree. Run it once before the change and keep its output in the evidence.
    - `cargo test -p cobolt-wasm` green.
    - Every corpus module without `ALTER` is byte-identical to its hash from before the change.
    - The `nist_wasm` census for NC, IF and IC gives the counts recorded in `WASM-PROGRESS.md`.

- [x] **F2 — `SET` pointer is classified as it is compiled** (C13)
  - *Done 2026-11-14 (2.0.484):*
    - **Before:** a program using it was refused at its line with the generic "not yet supported by the WebAssembly generator" (the `stmt` fallback arm), while the table said `Module`. Nothing was miscompiled.
    - **Now:** `BackendOnly(POINTERS)` in both `support()` and `STATEMENT_TABLE`. `subset_table` gives 7 of 7 refusals, the KB is regenerated (4 348 records), and `prebuilt_chunked_kb_matches_the_published_documentation` is green.
  - Files: `crates/cobolt-wasm/src/support.rs` (the `SET` pointer entry), `crates/cobolt-wasm/tests/subset_table.rs`.
  - Do:
    - Record what a program using `SET` pointer does today under the generator: compiled, silently skipped, or wrong code.
    - Reclassify the entry as refused, with the construct named at its line and the alternative, as the other refusals are. Lowering it is the feature's job (T45).
  - Verify:
    - `constructs_outside_the_subset_are_refused_at_their_line_with_the_alternative` gains a `SET` pointer case and is green.
    - `cargo test -p cobolt-wasm --test subset_table` green.
    - The System KB text that reads the table is regenerated, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.

- [x] **F3 — Land the fixes, and bring them to this branch**
  - *Done 2026-11-14:*
    - `main` could not take the spec branch as a fast-forward, because it held a commit of its own (README). So the operator chose to copy the spec commits onto `main` and continue on a new branch.
    - `main` now holds the spec commits (2.0.478–2.0.482), then F1 (`f72c3d03`) and F2 (`dd88944e`), all fast-forwards.
    - `features-2x-desktop-wasm` was cut from that `main`, so it carries both fixes, and `compile_determinism` is green on it.
  - Do:
    - Commit F1 and F2 separately on `fixes-2x-wasm-emit-order`. Each takes the next `z` not used on any open branch: this branch holds 2.0.478 onwards.
    - Push outside the window, and ask the operator to merge them into `main`.
    - Bring them to this branch as the operator decided before F1.
  - Verify: `git log main --oneline` shows both fixes; `cargo test -p cobolt-wasm --test compile_determinism` green on this branch before T22 starts.

## Phase M0 — The baseline, the deterministic mode and the golden hashes (the WebAssembly path is off)

- [x] **T1 — The Web module's hash, recorded before anything changes** (R24; AC16)
  - *Done 2026-11-14 (2.0.485):*
    - **Pinned:** three files, not two. The single-memory module is pinned too, because the Build writes both variants.
      - `program.wasm`: 729 228 bytes, `f7fec25a…`;
      - `program.single.wasm`: 729 244 bytes, `289e75c5…`;
      - the manifest: 27 320 bytes, `d3b8c806…`.
    - **Runs:** 3 of 3 match in four runs: one recording, two checks, and one from a clean checkout at 2.0.484. The uncommitted edits to the example's project file do not reach the module.
    - **Timing:** the Build takes 9.6 s. `UPDATE_PIN=1` re-records the pins.
  - Files: new `crates/cobolt-web-tests/tests/desktop_wasm_unchanged.rs`; its fixture `crates/cobolt-web-tests/tests/fixtures/power-analytics-web.sha256`.
  - Do: build the Web module of `examples/PowerAnalyticsWeb` (from a copy, never in place), and pin the sha256 of `program.wasm` and of its manifest.
  - Verify: `cargo test -p cobolt-web-tests --test desktop_wasm_unchanged -- --nocapture` prints both hashes and passes. It stays green through every later task.

- [x] **T2 — The Build reports its phases and its peak memory** (R18, R20; AC7 harness)
  - *Done 2026-11-14 (2.0.486):*
    - **Phase order:** the enum is declared in the order a Build runs its phases. Regenerate comes before front end, because the forms' COBOL is written before it is parsed. The toolchain check counts as `Cargo`; T16 moves it after classification (C8).
    - **What the peak covers:** the Build's own process plus every process it started after the Build began. On the Cargo path that is where the memory goes, in `rustc`.
    - **The new `path` field** has one variant, `Cargo`. T16 adds the WebAssembly one.
    - **Measured by `build_reports_phases_and_peak`,** a one-form project on the shared cache: regenerate 2.2 ms, front end 7.6 ms, cargo 7 498.8 ms, install 1.5 ms. The sum is 7 510.1 ms against 7 524.5 ms of wall time, the peak 860.2 MB, and 6 of 6 checks passed.
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `BuildProgress` (:1834) gains `phase: BuildPhase` and `elapsed`;
      - `BuildResult` (:1929) gains `phases: Vec<(BuildPhase, Duration)>`, `peak_rss: Option<u64>` and `path: BuildPath`, which is always `Cargo` for now;
      - a sampler thread reads resident memory every 25 ms with `sysinfo` (already in `Cargo.lock`), added to `cobolt-compiler`'s `Cargo.toml`.
    - The readers of `BuildProgress` in `cobolt-ide` and `cobolt-project-tools` compile against the new fields, unchanged in what they show.
  - Do: the phases are `FrontEnd`, `Regenerate`, `Classify`, `Generate`, `MachineCode`, `Assemble`, `Cargo` and `Install`. A phase that did not run is absent.
  - Verify:
    - A new compiler test, `build_reports_phases_and_peak` (`-- --nocapture`), builds a one-form fixture on the Cargo path. Its result block prints each phase in ms, their sum against the wall time, and the peak in MB. The sum is ≤ the wall time, and the peak is > 0.
    - The three existing build tests of plan §6 are green.
    - `cargo build -p cobolt-ide -p cobolt-project-tools -p cobolt-cli` succeeds.

- [x] **T3 — The ten-times reference project** (R26; AC18 generation)
  - *Done 2026-11-14 (2.0.487):*
    - **COBOL lines** means the non-blank lines of every program a Build compiles, counted after regenerating a copy as a Build does.
    - **The five references:** PowerGraphics is the largest in forms (9) and in lines (6 655); none has an indexed file. So the target is 90 forms, 66 550 lines and 10 indexed files.
    - **The generated project hits the target exactly.** The Common Code is sized so that the total comes out right. It is byte-identical across two generations (`232c5720…`), and `check` reports 0 errors and 0 warnings, in 137 s.
    - **The test passed 5 of 5 checks,** with the same numbers from a clean checkout.
    - **Why the targets are computed, not written down:** they come from the examples each time the test runs, as R17 defines them. T4 records the targets and the hash with each run.
  - Files: new `crates/cobolt-bench/src/ten_times.rs`, with its tests in the same file.
  - Do:
    - **Measure the five reference projects** (C7): forms, COBOL lines and indexed files.
    - **Write a project** with ten times the largest of each dimension, and at least 10 indexed files.
    - **Keep it deterministic:** ordered maps only, a fixed seed, `/` in every path, and no timestamp in any file.
    - **Use only names and constructs the reference** (`kb_lookup`) lists for a Desktop project.
  - Verify:
    - `cargo test -p cobolt-bench ten_times -- --nocapture` prints:
      - each project's dimensions;
      - the target dimensions;
      - the generated dimensions;
      - the sha256 of the whole tree, equal across two generations.
    - `check` on the generated project reports no error.

- [x] **T4 — `cobolt-bench build-goal`, and the baseline** (R17, R19, R20; AC6 and AC7 harness, AC18 hash)
  - *Done 2026-11-14 (2.0.488):*
    - **This machine's baseline** is recorded in `specs/WASM-BUILD-GOAL.json` (`macos-aarch64`): Apple M3 Pro, 11 cores, 18 GB, **not a reference box**. 30 of 30 Builds succeeded in 89.3 min, all on the Cargo path, and all six projects are over the goal.
      - **Slowest Builds:** PowerCallOfDuty 190.7 s, PowerCloud 156.4 s, PowerCobolCraft 198.5 s, PowerGraphics 198.7 s, PowerSpatial 163.3 s, TenTimes 191.8 s.
      - **Where the time goes:** Cargo takes 156–199 s, because with no cache it compiles the whole dependency tree. Regeneration takes under 40 ms and install under 52 ms.
      - **Peak memory:** 4 086–5 754 MB.
    - **The front end** takes 20–61 ms on the five examples and **30 409.5 ms on TenTimes**, alone over the goal. Not profiled. The likely place is the per-form loop at `cobolt-compiler/src/lib.rs:2777–2830`, which links the whole Common Code into each of the 89 other forms' programs, then serializes the program and compresses it at the best level. It must be dealt with before the goal is judged at M2, since the WebAssembly path runs the same front end.
    - **The generator** takes the project as the WebAssembly path will: one module, each form with its controls and bound events. Until T16 gives Desktop its own form descriptions, the bench builds them as the Web Build's `form_specs` does, without the Web lowering.
      - **Accepted:** PowerCallOfDuty (5.1 ms) and PowerGraphics (9 forms, 45.8 ms).
      - **Refused**, each at the developer's line:
        - PowerCloud at `COBOL::"AWS-SET-CREDENTIALS"`, and 37 more;
        - PowerCobolCraft at a statement that reads more than four properties, and 18 more;
        - PowerSpatial at `ACCEPT FROM Environment`, and 10 more;
        - TenTimes at an INDEXED file on disk, and 7 more.
      - **A first version compiled each program alone.** That gave false refusals ("SCN-1 is not defined"), because a form's controls are unknown without its form description. It was corrected and the whole baseline run again; only the second run is recorded.
    - **The ten-times hash** is `232c5720…`, equal to T3's.
    - **Owed: the CI dispatch.** `build-goal.yml` is written for the four platforms, but GitHub dispatches only from the default branch, and the workflow is not on `main`. The four result blocks and AC18's four equal hashes are recorded at the first dispatch after the workflow reaches `main`, or at the first `v*` tag. The macOS arm64 job runs on the hosted runner, and its record says "not a reference box" (C9).
    - **Flags:** `--runs 0` times the generator alone and builds nothing; `--record` refuses it.
  - Files:
    - `crates/cobolt-bench/src/main.rs`: a `build-goal` mode, dispatched before the existing filter;
    - new `crates/cobolt-bench/src/build_goal.rs`;
    - `crates/cobolt-bench/Cargo.toml`: depends on `cobolt-compiler`;
    - new `specs/WASM-BUILD-GOAL.json`, written by `--record`;
    - new `.github/workflows/build-goal.yml`: the four platforms, on tags and on dispatch.
  - Do:
    - **Each run:** copy every reference project, the five examples and the ten-times project, into a temporary folder, never building an example in place. Build each one five times with no cache: a fresh target folder and the shared build cache off. Time the generator alone where it accepts the project, and name the construct where it refuses.
    - **Flags:** `--runs N` shortens a local run; the default is 5.
    - **The result block,** per platform and project, gives:
      - the path taken;
      - the slowest Build against 30 s;
      - each phase in ms;
      - the peak memory;
      - the generator's time or its refusal;
      - the ten-times project's hash.
    - **The macOS arm64 job** runs on a larger or self-hosted runner. Otherwise it records the deviation (C9).
  - Verify:
    - `cargo run -p cobolt-bench --release -- build-goal --record` prints the result block and writes the JSON.
    - The numbers from this machine go into `WASM-PROGRESS.md` with its cores and memory, named as "not a reference box" where it is not one.
    - The CI job is dispatched once, outside the window. Its four result blocks are recorded, and the four ten-times hashes are equal (AC18).

- [x] **T5 — The run clock and the random seed, pinned for tests** (R28, the program half)
  - *Done 2026-11-14 (2.0.489):*
    - **`cobolt-runtime/src/test_pins.rs`** reads `PRC_TEST_CLOCK` (RFC 3339) and `PRC_TEST_SEED` (u64) once.
      - The offset written in the pinned value is the program's time zone, so a pinned run shows the same date on every machine.
      - A value that cannot be read is named on stderr and pins nothing.
      - `set_for_test` and `clear_for_test` pin for the whole process without touching the environment, and restart the generator.
    - **The audit:** every clock or random read a program can see now goes through the pins:
      - `ACCEPT … FROM DATE`, `TIME`, `DAY` and `DAY-OF-WEEK`;
      - `FUNCTION CURRENT-DATE` and `WHEN-COMPILED`;
      - `YEAR-TO-YYYY`, which keeps its UTC year;
      - the first state of `FUNCTION RANDOM`, which a pinned seed sets as `FUNCTION RANDOM(seed)` would.
    - **The other reads never give a program a value,** and are listed in the module's documentation:
      - timeouts and progress of asynchronous operations;
      - the waits of file locks, `WS-PAUSE` and the AWS pool;
      - the frame clock of animation sequences, which is T8's;
      - AWS signing and credential expiry, which need the real time;
      - indexed-file log stamps, key-file nonces and temporary names;
      - measurements inside tests.
      - `cobolt-stdlib`'s intrinsic table has its own `CURRENT-DATE`, but no code calls the table, so it is left as it is.
    - **`pinned_clock_and_seed`:** 7 of 7 checks.
      - The clock, pinned at `2026-01-02T03:04:05.67-03:00`, gives CURRENT-DATE `2026010203040567-0300`, DATE `260102` (YYMMDD in a PIC 9(6)), DAY `26002`, DAY-OF-WEEK `5` and TIME `03040567`, the same in two runs.
      - RANDOM, pinned at seed 20 261 114, gives the same 1 000 values in two runs (1 000 distinct, about 72 000 values/s), equal to what `FUNCTION RANDOM(20261114)` gives with nothing pinned.
      - Unpinned, the first value is still 0.485917337.
      - `test_local_clock` (unpinned, 3 of 3) is unchanged.
    - **Interpreter-binary parity:** the new compiler test `a_built_program_takes_the_clock_and_seed_pins` builds a console fixture with Cargo and runs it with both variables set. It prints `2026010203040567-0300` and `0198382315`, the interpreter's first value for that seed (2 of 2, 41.4 s).
    - **`cargo test -p cobolt-runtime`:** all 210 targets ok, 1 674 passed, 0 failed, 29 ignored. It was run one target at a time, freeing each binary after its run: each of the 208 test binaries takes about 228 MB, about 47 GB if all are linked at once.
    - **The pinned clock does not move yet.** T8 makes it follow the host's virtual time.
  - Files:
    - new `crates/cobolt-runtime/src/test_pins.rs`: read once from `PRC_TEST_CLOCK` (RFC 3339) and `PRC_TEST_SEED` (u64), with a `set_for_test` for tests, which never touch the environment;
    - `crates/cobolt-runtime/src/interpreter.rs`: the clock sites at :19016 and :21041–21174;
    - `crates/cobolt-stdlib/src/intrinsics.rs`: `RANDOM`'s seed at :170, and `CURRENT-DATE`.
  - Do:
    - **The audit:** every read of the system clock or of a random source in the two crates goes through the pins, or is listed in the evidence as never reaching a program.
    - **When pinned,** the clock starts at the pinned instant and moves only when the form host advances virtual time (T8). A console program sees the pinned instant.
    - **When the variables are unset,** nothing changes.
  - Verify:
    - The new `cargo test -p cobolt-runtime --test pinned_clock_and_seed -- --nocapture` checks, with pins, that these are equal across two runs and to the expected values:
      - `FUNCTION CURRENT-DATE`;
      - `ACCEPT … FROM DATE`, `DAY`, `DAY-OF-WEEK` and `TIME`;
      - 1 000 values of `FUNCTION RANDOM`.
    - `cargo test -p cobolt-runtime` green.
    - The compiled binary takes the pins through the same crate. A Cargo-built fixture run with `PRC_TEST_CLOCK` set prints the pinned date (interpreter-binary parity).

- [x] **T6 — The deterministic headless loop, and the steps `shot`, `size` and `answer`** (R28, the headless half)
  - *Done 2026-11-14 (2.0.490):*
    - **Deterministic mode:** with `PRC_TEST_CLOCK` set (`test_pins::clock_pinned`), frames are drawn at a virtual time counted in 1/60 s ticks.
      - **Refinement:** only a wait adds ticks (`wait_ms` and the opening wait). A frame that only waits for the program adds none. Otherwise the number of those frames, which depends on how fast the interpreter thread runs, would move the time, and two runs would draw at different times.
      - **Deviation until T8:** a wait still lets the same real time pass. `host.rs`'s timers, its 450 ms opening hold and `onShow` read the real clock, and skipping the real time would change what a script sees. T8 removes the sleep when `HostClock` follows the virtual time.
    - **The steps:**
      - `{"shot": "<name>"}` writes the frame to `<folder>/<name>.png` with the CPU rasteriser; a name with a folder in it is refused;
      - `{"size": [w, h]}` resizes, each side from 1 to 8 192, then settles;
      - `{"answer": {"path": …}}` and `{"answer": {"cancel": true}}` queue the answer for the next file dialog.
    - **The API:** `run_headless_with` takes the folder for pictures. `run_headless` keeps its signature, so its callers are unchanged. `run_script_file` takes the folder, and the generated `main.rs` passes `PRC_HEADLESS_SHOTS`.
    - **The report** adds `virtual_ms` in deterministic mode and `answers_unused` when an answer was left; otherwise it is what it was.
    - **Correction: a Desktop form has no message box.** `MessageBox`, `Alert`, `Confirm` and `InputBox` exist only in the Web player, so `answer` answers the dialogs a Desktop program can raise:
      - the program's `OPEN-FILE-DIALOG`, `SAVE-FILE-DIALOG` and `FOLDER-DIALOG` (`file_dialog::answer_program`);
      - the host's own panels, a Viewer's Save As and a grid's export (`file_dialog::begin`).
    - **The agents' `run_form` script** (`cobolt-project-tools/src/script.rs`) does not offer the three steps. They serve the parity harness, and `shot` needs a folder `run_form` does not give.
    - **`headless_shots`:** 11 of 11 checks, 12 steps in 2 117 ms (virtual 600 ms).
      - Shots at 420 × 200 (6 523 bytes), 640 × 360 (9 930 bytes) and 320 × 240 (6 231 bytes).
      - The open dialog took its scripted path, the save dialog its scripted cancel, and a host Save As panel its queued answer.
      - `../outside` was refused; no answer was left unused.
    - `headless_dump` and `headless_pointer_click` are unchanged and green.
    - **Sweep of `cobolt-form-host`** (debug, `--no-fail-fast`, every `test result:` read):
      - the library: 189 passed, 1 failed;
      - the integration targets, each green: `backend_events` 1, `headless_dump` 1, `headless_pointer_click` 1, `headless_shots` 1, `progress_cancel` 1, `remote_call_pending` 1, `snackbar_stack` 21;
      - the built binary, `side_menu_rows_hosts --ignored`: 1 passed in 60.5 s.
      - **The one red is not T6's:** `host::parity::corpus_golden` differs on `PowerChat/chat-form.cfrm` (SideMenu-1 at 594 px high, golden 792). It is the known red already named in the 2.x merge sweep. T6 touches neither `host.rs` nor the render path.
  - Files:
    - `crates/cobolt-form-host/src/headless.rs`: the loop at :130–162 and the step table at :20–27;
    - `crates/cobolt-form-host/src/file_dialog.rs` and the message-box path: scripted answers;
    - `crates/cobolt-compiler/src/lib.rs`: the generated `main.rs` at :4873 passes `PRC_HEADLESS_SHOTS`;
    - new `crates/cobolt-form-host/tests/headless_shots.rs`.
  - Do:
    - **With `PRC_TEST_CLOCK` set,** each frame advances virtual `t` by 1/60 s, nothing sleeps, and `wait_ms` advances virtual time. `host.rs` is not touched yet.
    - **`{"shot": "<name>"}`** writes the current frame to `PRC_HEADLESS_SHOTS/<name>.png` with the CPU rasteriser (`cobolt_forms::raster`).
    - **`{"size": [w, h]}`** resizes the window.
    - **`{"answer": {…}}`** queues the answer for the next message box or file dialog, so no run waits on a person.
    - **The step table** in the module comment gains the three steps.
  - Verify:
    - `cargo test -p cobolt-form-host --test headless_shots -- --nocapture` (`shot_and_size_steps_write_pictures`) checks that:
      - three shots at two sizes give PNGs of those sizes;
      - a queued answer closes a message box;
      - a file dialog takes its scripted path.
    - With the variables unset, `headless_dump` and `headless_pointer_click` are green and unchanged.

- [x] **T7 — The pixel-parity harness, Cargo only; the base scripts; the first golden hashes** (R27 harness; AC19 second guard, AC20 per form; AC1 check scripts)
  - *Done 2026-11-14 (2.0.491):*
    - **`crates/cobolt-bench/src/pixel_parity.rs`**, the `pixel-parity` mode, with `--write-scripts`, `--cargo`, `--record` and `--project <name>`.
    - **Refinements and decisions:**
      - **The projects are taken as committed** (`git ls-tree` and `git show` of `HEAD`), never from the working tree, so a golden recorded on one machine is the one a clean checkout compares with. The operator's working tree differs from `HEAD` in some of them (for example `war-form.cfrm` binds 40 events there, 2 at `HEAD`).
      - **No product change.** A built program already opens at any of its embedded forms with `COBOLT_DESIGNER_FORM`, and the headless variables still apply, so each form's script runs against the project's own binary.
      - **The network is cut off rather than stubbed.** The copy's AWS connections are routed to a server that does not exist. Each run gets an empty home folder, no AWS variables, and `HTTP(S)_PROXY`/`ALL_PROXY` set to a port that refuses connections. Handlers that would reach a model, a web service, a database or AWS are not raised, and the script's comment names them.
      - **The script format:** `{"comment", "form", "steps"}`. A `shot` once the form is open and after every step. A dialog handler gets `{"answer": {"cancel": true}}` before it.
      - **A handler that opens a form modally** (`OpenFormSync`) is checked before one that ends the form, because a handler can do both (PowerCobolCraft's BTN-PLAY opens the world and then closes the title). It is raised without waiting (`"wait": false`), then given 500 ms. Once one has run, the ending handlers are raised without waiting too; otherwise the run waits on a program the modal form holds up, until the time limit.
      - **Each size is its own run of the program**, with `{"size": [w, h]}` first for 800 and 480 wide, so a run at one size cannot leave state for the next. `data/` is restored from a pristine copy before every run.
      - **Three runs to record, two to check.** In the first full recording two runs of PowerGraphics' bridge form matched by chance, and the check then found 6 of its frames different. With three runs, a form that fails to repeat is far less likely to be recorded.
      - **Audio plays during a run** (the reference projects play sounds); nothing in a frame depends on it.
    - **Measured** (macOS arm64, release build):
      - **`--cargo --record`:** 5 projects, 22 forms, 22 scripts, 263 steps, 369 frames, three runs each, in 18.5 min.
        - Per project, forms / steps / frames / identical in all three runs / build s / runs s:
          - PowerCallOfDuty 1 / 5 / 9 / 0 / 273.2 / 15.4;
          - PowerCloud 5 / 39 / 57 / 25 / 104.6 / 74.7;
          - PowerCobolCraft 2 / 41 / 60 / 4 / 7.2 / 78.6;
          - PowerGraphics 9 / 95 / 120 / 108 / 137.8 / 174.7;
          - PowerSpatial 5 / 83 / 123 / 56 / 69.7 / 170.0.
        - 193 of 369 frames identical; **6 of 22 forms repeatable** (PowerGraphics: charts, imagelab, pitfall, pool2d, pool3d, viewer3d), **75 golden frames** written.
      - **The second run without `--record`** (PowerGraphics, twice, two runs each): **75 of 75 golden frames matched both times**, 0 differing (6.1 and 5.6 min, exit 0). The golden frames are all PowerGraphics', so the other projects had nothing to match.
      - **The unit test** `cargo test -p cobolt-bench pixel_parity` (`base_script_orders_and_names_the_handlers`): 9 of 9 checks. A fixture of 7 controls and 8 handlers gives the 14 expected steps, and the comment names its modal, ending, dialog and network handlers. It also checks a form's file stem, the CALLs a handler makes, and a frame's hash. (Found while writing it: an AgentObject written as a self-closed `<Control …/>` was not among the loaded form's controls, and no error said so. The reference forms never write one. Noted for the operator, not changed here.)
    - **Not repeatable before `HostClock`, 16 forms** (frames that differ between runs): PowerCallOfDuty war-form 9 of 9; PowerCloud main-form 2 of 27, pet-form 12 of 12, view-form 3 of 3, confirm-form 9 of 9, working-form 6 of 6; PowerCobolCraft world-form 38 of 42, title-form 18 of 18; PowerGraphics game-form 9 of 33, bridge-form 2 of 6, runner-form 1 of 6; PowerSpatial main-form 7 of 60, toolbar-form 9 of 9, rail-form 22 of 24, rooms-form 21 of 21, settings-form 8 of 9. The causes, read from the code: the entrance and exit effects (MatrixRain is seeded from the form's name and size, but its progress reads the real clock), snackbars, timers, the 450 ms opening hold, and canvas `onFrame` events handled while the next frame is drawn. T8 removes them.
    - **Owed:** the CI dispatch on four platforms, and the three other platforms' golden files from its artefacts. GitHub runs `workflow_dispatch` only from the default branch, and `main` is not touched while the feature is on its branch.
  - Files:
    - new `crates/cobolt-bench/src/pixel_parity.rs` (a `pixel-parity` mode);
    - new `tests/pixel-parity/scripts/<project>/<form>.json` for the five reference projects;
    - new `tests/pixel-parity/golden/macos-aarch64.toml`;
    - new `.github/workflows/pixel-parity.yml`: the four platforms, on tags and on dispatch, with `--record` on dispatch only.
  - Do:
    - **`pixel-parity --write-scripts`** makes each form's base script:
      - a `shot` after every step;
      - every bound event raised once, in document order;
      - a handler that ends the form or the program goes last, and is named in the script's comment field;
      - a dialog gets an `answer` step.
    - **The script runs three times:** at the designed size, then after `{"size": [800, h]}`, then after `{"size": [480, h]}`.
    - **`pixel-parity --cargo`** builds each project on the Cargo path (from a copy) and runs every script twice under the pins. Two outcomes per form:
      - frames that repeat: their sha256 (of the RGBA buffer) is recorded with `--record`;
      - frames that do not: the form is named as "not repeatable before `HostClock`".
    - **On a mismatch,** the diff image and both frames go under `target/pixel-parity/`.
  - Verify:
    - `cargo run -p cobolt-bench --release -- pixel-parity --cargo --record` prints, per project:
      - the forms, steps and sizes;
      - frames compared, and frames identical across the two runs;
      - the forms not yet repeatable.
    - The golden file is written. A second run without `--record` matches every golden.
    - The CI job is dispatched once, outside the window, and the three other platforms' goldens are committed from its artefacts.

- [x] **T8 — `HostClock`, then the rest of the golden hashes** (R28; AC20)
  - *Done 2026-11-14 (2.0.493; the fix 2.0.492 before it):*
    - **`crates/cobolt-form-host/src/clock.rs`:** `now()` and `since()`. It is the real clock unless a deterministic run has started the virtual time; then it is a fixed instant plus that time.
    - **The 16 reads in `host.rs` that reach a pixel** now read it:
      - the form's start, which gates the 450 ms hold and `onShow` (root and child bodies);
      - the entrance effect, the exit effect (two places) and the window effect;
      - the snackbars (raise and tick);
      - the animation step (two places);
      - the parked form's timers.
    - **Left on the real clock**, as nothing they do reaches a pixel: placing a group of windows on the screen (`group_fit_started`), the OS blur's retry, time limits, and the measurements in tests. The plan's "37 calls" counted those and the tests. `snackbar_stack.rs` reads no clock (every method takes `now`), so it is unchanged.
    - **The virtual time lives in `cobolt_runtime::test_pins`** (`start_virtual_time`, `set_virtual_time`, `stop_virtual_time`, `virtual_elapsed`), so the program's clock follows the same time: the pinned instant plus the virtual time while it runs.
    - **The headless loop** (`headless.rs`), with the clock pinned:
      - starts the virtual time before it builds the form, and ends it with the run;
      - no longer sleeps during a wait; each frame is one 1/60 s tick;
      - **refinement (`quiesce`):** before time moves on, and at the end of every step that waits for the program, the form on the pane finishes everything it was sent. When anything was sent since it was last at rest, a wake goes behind it (`FormHost::script_barrier`), and the loop draws frames at the same virtual time until the program has handled both. The frames that let the writes reach the state go round again if they raise an event (at most 16 rounds).
        - Why the wake: the program takes an event off its queue before handling it, so the pending count alone says nothing about a handler still running. The wake is taken only after the handler returns.
        - Why before time moves on: the first version waited only for events raised while the form was idle. That leaves the opening wait racing a long `onLoad` (war-form's builds a city): canvas frames raised meanwhile queue up, or are coalesced away past 8, in a number set by how fast the program ran. Whether that race ever bit here was not isolated, because the pixels war-form drew differently turned out to be the rasteriser's (below).
        - A form that a modal form holds up (`root_modal_blocked`), or whose program has ended, is not waited for, and neither are events sent to a child window.
      - `host::events_sent()` counts the events the host sends, so the loop can tell whether anything was sent since the form was last at rest.
    - **`cobolt-bench pixel-parity`:**
      - a check now fails when a form's runs differ (AC20), as a differing golden does; a recording only leaves the form out. Its wording and the golden file's header say so;
      - **the entrance wait:** under virtual time an event moves no clock. In the four projects that open their forms with MatrixRain (2000 ms), every picture after the first was therefore taken 600 ms in, mid-effect, and showed the effect rather than the form's answer (PowerCloud's confirm-form drew the same hash for every step). In such a project (`[forms] entrance-effect`, read as the product reads it), the base script now waits `MATRIX_HARD_MAX_MS` (6000 ms, longer than any effect) after `00-open` and takes `00-ready`. PowerGraphics opens without an effect, and its scripts are unchanged.
    - **A fix found on the way, landed first (2.0.492, `fixes-2x-raster-atlas`, picked onto this branch):**
      - war-form still drew 2 to 4 text pixels differently between runs, one step of 255 apart, at the designed size.
      - The CPU rasteriser summed a glyph's texture coordinate from absolute atlas coordinates in f32, so its rounding depended on where egui had put the glyph, which depends on the order text was first laid out in.
      - It now samples exactly in f64, relative to the first vertex. Its own test went from 5 of 12 differing pictures to 12 of 12 identical.
    - **Measured** (macOS arm64, release build):
      - **The refactor moved no pixel:**
        - the first version of this task, checked against T7's goldens, matched 75 of 75 golden, with 369 of 369 frames identical in two runs (a three-run recording after it found war-form differing in 3 of 9 frames; see the fix above);
        - the final code with the sampling before 2.0.492 matched PowerGraphics 120 of 120 frames identical, 9 of 9 forms repeatable, 75 of 75 golden (3.9 min).
      - **`--write-scripts --cargo --record`:** 5 projects, 22 forms, 289 steps, 408 frames, three runs each, 18.2 min.
        - Per project, forms / steps / frames / identical in all three runs / build s / runs s:
          - PowerCallOfDuty 1 / 7 / 12 / 12 / 205.8 / 19.4;
          - PowerCloud 5 / 49 / 72 / 72 / 82.8 / 116.0;
          - PowerCobolCraft 2 / 45 / 66 / 66 / 6.0 / 119.9;
          - PowerGraphics 9 / 95 / 120 / 120 / 102.5 / 58.2;
          - PowerSpatial 5 / 93 / 138 / 138 / 65.7 / 311.5.
        - **408 of 408 frames identical; 22 of 22 forms repeatable** (6 of 22 at T7); 408 golden frames written, `not_repeatable` empty.
        - Against T7's 75 frames, 42 are unchanged. The other 33 (every frame of PowerGraphics' pitfall, pool3d and viewer3d forms) moved, by 2.0.492's sampling alone, as the check above shows.
      - **The check without `--record`** (two runs each): **408 of 408 golden frames matched**, 22 of 22 forms repeatable, exit 0, 14.6 min.
      - **Tests:**
        - `deterministic_mode_repeats_frames`: 10 of 10 checks, 4 of 4 pictures identical, 41 ticks and 5 700 ms of virtual time per run;
        - `pinned_clock_and_seed`: 9 of 9; `base_script_orders_and_names_the_handlers`: 11 of 11;
        - `cobolt-form-host`: 189 of 190 in the library and 28 of 28 in 8 targets. The red, `corpus_golden`, is a layout golden that fails on PowerChat's chat-form, and was red before this task;
        - `cobolt-forms --features render`: 1 877 of 1 878, `engine_reference_form_parity_static_vs_faces` green. The red, `example_corpus_golden` (3 PowerDemo3 forms), is a shape golden, red on `main` without this work too;
        - the rcrun host tests (`powercloud_hosts`, `side_menu_hosted_controls`, `side_menu_rows_hosts`, `layers_hosts_091`, `perspective_hosts_096`, `model_list_hosts`, `partiql_hosts`, `aws_hosts`): 22 of 22, `side_menu_rows_hosts` with its built-binary test; the 5 other built-binary tests stay ignored, as by default.
    - **Owed:** the CI dispatch of `pixel-parity.yml` on four platforms and the other three platforms' golden files, as for T7 (the workflow must be on `main` first).
  - Files:
    - new `crates/cobolt-form-host/src/clock.rs`;
    - `crates/cobolt-form-host/src/host.rs`: its 37 `Instant::now` calls (timers, animations, entrance and exit effects, snackbars);
    - `crates/cobolt-form-host/src/snackbar_stack.rs`, where it reads the time;
    - new `crates/cobolt-form-host/tests/deterministic_mode.rs`.
  - Do:
    - **`HostClock`** is real by default, and virtual when `PRC_TEST_CLOCK` is set. The headless loop of T6 drives it, and the runtime clock of T5 follows its virtual time.
    - **The refactor changes nothing** with the variables unset.
  - Verify:
    - `cargo test -p cobolt-form-host --test deterministic_mode -- --nocapture` (`deterministic_mode_repeats_frames`). A fixture form has a timer, an entrance animation and a snackbar. Run twice, it prints "frames N, identical N".
    - `cargo test -p cobolt-form-host` and `cargo test -p cobolt-forms --features render` green. `engine_reference_form_parity_static_vs_faces` green.
    - `pixel-parity --cargo` without `--record` matches every golden from T7: the refactor moved no pixel.
    - Then, with `--record`, it adds the forms that now repeat. Their names are listed in this task's CHANGELOG entry.
    - The `rcrun` host tests stay green (`side_menu_hosted_controls`, `powercloud_hosts` and the others).

## Phase M1 — Console programs on the WebAssembly path, on by default

- [x] **T9 — The manifest's capabilities, the ABI range and the bundler's refusal** (R3, R4 web half, R5; AC5 web half, AC16)
  - Files:
    - `crates/cobolt-wasm-abi/src/manifest.rs`: `capabilities`, serialised only when not empty; `validate` (:241) accepts `1 ≤ abi ≤ ABI_VERSION` (C2);
    - `crates/cobolt-wasm-abi/src/codes.rs`: `NATIVE_IMPORTS`, namespace `prn`;
    - `crates/cobolt-wasm-abi/src/scan.rs`: `bundle_problems` names a native-only capability;
    - `specs/WASM-ABI-v1.md`: the §10 amendment, and §8 corrected to the code (C10).
  - Verify:
    - `cargo test -p cobolt-wasm-abi` green, with new tests:
      - a manifest with no capabilities is byte-identical to before (fixture bytes);
      - `validate` accepts 1 and `ABI_VERSION`, and refuses 0 and `ABI_VERSION + 1`;
      - `bundle_problems` names `files.indexed` in a module that declares it.
    - `cargo test -p cobolt-web-bundle` and T1's test green.
    - The player `--lib` is green. The shipped player is rebuilt if its freshness test asks for it, in the same commit.
  - *Done 2026-11-14 (2.0.494):*
    - **`codes.rs`:**
      - `cap`: the ten capability names of the plan, and `cap::ALL`;
      - `NATIVE_IMPORT_MODULE = "prn"`, and `NATIVE_IMPORTS` with the sets M1 uses: `console` → `con_read`, `env` → `env_get`, `exit`. Only those two, because a set is fixed once a release uses it (C2): the dialog, file, sort, SQL and HTTP sets are left to the tasks that implement them (T32, T38, T39, T41, T42), each amending A27;
      - `CONFORMANCE_IMPORTS` gains `switch_state`, which the conformance build has always imported (C10).
    - **`manifest.rs`:**
      - `capabilities: Vec<String>`, `serde(default, skip_serializing_if = "Vec::is_empty")`;
      - `validate` accepts `1..=ABI_VERSION`. The refusal now says what to do: for 0, rebuild the application; for a newer ABI, run it with a current release. "Rebuild with the current IDE" was wrong advice for a module newer than the host.
      - Struct literals: the field was added to the 20 places that build a manifest by hand (the ABI crate, `cobolt-wasm`'s `build_manifest`, the player's `tree.rs`, 15 player test files and `parity_page.rs`). `build_manifest` writes none: T12 fills it under `native`.
    - **`scan.rs` (`bundle_problems`):**
      - each capability the embedded manifest lists is a problem that names it: one only the desktop runner provides, or, for an unknown name, one this release does not know;
      - an import from `prn` is named as a runner import, no longer as "not allowed by §3".
      - The page's own loader (`hostpage.rs`) still accepts ABI 1 exactly, which is every ABI there is. It is left as it is, so the Web bundle stays byte-identical (AC16).
    - **`WASM-ABI-v1.md`:**
      - §10 gains the `capabilities` member;
      - §11 item 1 reads "refuse an ABI it does not speak";
      - §8 is rewritten to the code: the `adv` argument, modes 2 = I-O and 3 = EXTEND with the organisation in bits 8–15, end of file −10, failures as the FILE STATUS negated, handles from 1 with 0 as the console, and `switch_state`;
      - amendment rows **A27** (capabilities, `prn`, the ABI range) and **A28** (the §8 correction).
    - **Measured:**
      - `cargo test -p cobolt-wasm-abi`: 21 of 21; with `--features fixture`, 26 of 26. New:
        - `validate_accepts_every_abi_from_1_to_the_current_one`: 3 of 3 (1 accepted; 0 and 2 refused with what to do);
        - `capabilities_are_written_only_when_there_are_some`: 4 of 4 (568 bytes, as before A27; with two they round-trip in order);
        - `a_manifest_without_capabilities_is_the_bytes_it_was`: 2 of 2 (the fixture modules' embedded manifests, 2 165 and 2 166 bytes, have the sha256 taken from `write_fixtures` before the change);
        - `bundle_problems_names_a_capability_only_the_runner_provides`: 8 of 8 (both variants: `files.indexed` named, an unknown name named, none accepted).
      - `desktop_wasm_unchanged` (T1, AC16): 3 of 3 artefacts match the pin, regenerated and built in 9.2 s.
      - `cargo test -p cobolt-web-bundle`: 146 of 146 in 60 targets.
      - The player: `--lib` 474 of 474, `abi_host` 28 of 28.
      - **The shipped player is rebuilt** (`build_player --out assets/web-player`). Its freshness test asked for it, and was red before this task too: the player's inputs changed at 2.0.470 (Canvas3D in `onLoad`) and at the fix 2.0.492 without a rebuild, the last being 2.0.447.
        - plain 4 129 791 → 4 130 450 bytes, threaded 3 944 236 → 3 944 904, the graphics3d part 8 471 405 → 8 471 516; the other parts unchanged.
        - `freshness` 6 of 6; `cobolt-web-bundle` 146 of 146 again with the new player.

- [x] **T10 — The native column of the subset table** (R12; AC10)
  - Files:
    - `crates/cobolt-wasm/src/support.rs`: every entry of `STATEMENT_TABLE` and `BUILTIN_TABLE` gains `native: Native { Module, Host(Cap), Cargo(&'static str) }`;
    - `crates/cobolt-wasm/tests/subset_table.rs`;
    - `crates/cobolt-compiler/src/lib.rs`: a Desktop column in `browser_subset_doc`;
    - `assets/knowledge/chunked.data`.
  - Do: classify each entry for the Desktop kind as it stands at M1. What the WebAssembly path cannot do yet is `Cargo("<reason>")`.
  - Verify:
    - `every_statement_and_builtin_has_a_native_class` and a planted-unknown twin green.
    - The KB is regenerated, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.
  - *Done 2026-11-14 (2.0.495):*
    - **`support.rs`:**
      - `Native { Module, Host(&'static str), Cargo(&'static str) }`; `Host` carries a name of `cap` (A27, T9);
      - `STATEMENT_TABLE` is now `(name, Support, Native)`, and `BuiltinClass` gains `native`; `statement_native(name)` reads a statement's class, as `builtin_entry(name).native` reads a built-in's;
      - one reason per group, each naming the milestone that moves it: forms, controls and the application object (M2); dialogs (M2); files on disk, the file and image built-ins, SORT with USING or GIVING, MERGE, COMMIT, ROLLBACK and UNLOCK (M3); SQL and HTTP (M4); pointers (M5); the non-visual controls, keys, model lists, audio, animation sequences, layouts, the phone's secure storage, remote calls and identity (M6); EXEC RUST never moves.
    - **The classes at M1:**
      - 53 statements: 31 in the module; 3 through the runner (`ACCEPT` → `console`, `STOP` and `GOBACK` → `env`); 19 on the Cargo path.
      - 83 built-ins, all on the Cargo path. Each one drives a form, a file, a database, a model, a key or the network, so a console program needs none.
    - **Decisions:**
      - **The remote calls name M6.** R11's end state leaves only EXEC RUST, crates and Debug on the Cargo path, and the plan's milestone table does not list them elsewhere.
      - **`SECURESTORE`, `SECURELOAD` and `SECUREDELETE` have their own reason.** They are Mobile built-ins, at which a desktop program stops (`builtins.rs`). That stop is what the WebAssembly path must reproduce at M6.
      - **For T21:** a file verb's class is `Cargo`, with a reason that says an INDEXED file declared `STORAGE IS MEMORY` is compiled into the module, and so is a SORT with an INPUT and an OUTPUT PROCEDURE (RELEASE and RETURN are `Module`). A row names a statement, not its file, so the Cargo class of `OPEN`, `READ`, `WRITE`, `REWRITE`, `DELETE`, `START`, `CLOSE` and `SORT` applies to a file on disk. Whether a file is in memory is the generator's own check, the one that writes `MemoryFiles` for a browser module. T21 should take the refusal from there, not from the row, or M1 would send a program with only in-memory files to the Cargo path.
    - **`browser_subset_doc`:** both tables gain **On the Desktop** and **Why**, with a paragraph saying that every Desktop Build still takes the Cargo path today. The KB is regenerated: 4 389 records from 8 documents, 15 372 288 → 15 544 320 bytes.
    - **Measured:**
      - `cargo test -p cobolt-wasm --test subset_table`: 8 of 8. New:
        - `every_statement_and_builtin_has_a_native_class`: 136 entries (53 + 83), 0 faults, 0 Cargo reasons without a milestone, 5 of 5 fixed points of M1 (EXEC RUST, ACCEPT, STOP, GOBACK, MOVE), 1.3 ms;
        - `a_planted_entry_with_no_native_class_is_caught`: 4 of 4 planted faults caught (two unknown names, a capability A27 does not list, an empty reason), and the sound control passed, 1.1 ms.
      - `prebuilt_chunked_kb_matches_the_published_documentation`: green. The compiler's `published_documentation` tests: 5 of 5, including `the_browser_subset_section_names_every_builtin_and_statement`.
      - `cargo test -p cobolt-wasm --no-fail-fast`: 27 of 29 targets green. Two reds, both from before this task:
        - `corpus`: the program `conditionals/test-arith-relation-after-and.cbl` prints its own timing (460 ms in the module, 970 ms in the interpreter);
        - `dependency_guard`.
      - No browser class moved: the 136 rows, with the Desktop column taken out, are the same as at 2.0.494.

- [x] **T11 — WHEN-COMPILED and paths as Build inputs** (R2; C3)
  - Files: `crates/cobolt-wasm/src/lib.rs` (`Options.compiled_at`), the WHEN-COMPILED lowering, and the `prlines` writer.
  - Do:
    - `compiled_at` comes from `SOURCE_DATE_EPOCH` when it is set, and from the Build's clock otherwise.
    - Every path in `prlines` is written relative and with `/`.
  - Verify:
    - `compile_determinism` gains a case: with the same `SOURCE_DATE_EPOCH`, a program using WHEN-COMPILED gives identical bytes, built from two folders with different separators.
    - T1's test green.
  - *Done 2026-11-14 (2.0.496):*
    - **The field:** the option already existed as `Options.when_compiled: Option<String>` (`module.rs`). It keeps that name rather than becoming `compiled_at`, so that the six test files that name it are untouched. The WHEN-COMPILED lowering (`lower_more.rs`) is unchanged: it takes the text the module builder hands it.
    - **`build_time(source_date_epoch)`** (`module.rs`, public) gives the 21 characters when the options name none:
      - from `SOURCE_DATE_EPOCH` when it is set and not empty;
      - otherwise from the Build's clock, as before.
      - A value that is not a whole number of seconds, or is past 9999-12-31 23:59:59 UTC (the last second a four-digit year spells), is refused. The message names the variable and its value, and says to set it to a number or unset it, as reproducible-builds.org asks for a malformed value.
      - Every caller that leaves the option empty now honours the variable: the Web and Mobile generator today, the Desktop `wasm_build` from T21.
    - **The `prlines` writer** (`line_site`):
      - Today no file path reaches the line table. Its sites are form display paths (`MAIN ▸ BTN-OK ▸ onClick`) and form names, because codegen's source map has no Common Code spans. A Common Code file's display path is its bare `rel_path`, and `resolve_display_path` already treats a site with no `▸` as a file.
      - The writer uses that same rule. A file site is written with `/`; an absolute one (`/…` or a drive letter) is refused as an internal error, because it would put the building machine's folders into the module's bytes.
      - The fallback site (the form's own name, for a line the source map does not resolve) is left as it is.
    - **Measured:**
      - `source_date_epoch_and_separators_are_build_inputs` (new in `compile_determinism`): 8 of 8 checks, 358 ms. The fixture moves FUNCTION WHEN-COMPILED, with its three statements mapped to a Common Code file. The checks:
        - `src\common\stamp.cbl` and `src/common/stamp.cbl` give the same bytes in both variants (85 374 and 85 385 bytes);
        - 12 of 12 `prlines` rows naming the file are written with `/`;
        - the module answers `2025111400000000+0000` for `SOURCE_DATE_EPOCH=1763078400`;
        - the next day gives other bytes;
        - `tomorrow` is refused, naming the variable and its value;
        - `/Users/dev/…` and `C:\work\…` are refused;
        - unset, the clock gives 21 characters;
        - the last second of 9999 is spelled `9999123123595900+0000`, and the next second is refused.
      - `same_input_same_bytes` (F1): 126 variants, 0 differing; 2 520 compiles in 23.5 s (107 compiles/s).
      - `desktop_wasm_unchanged` (T1, AC16): 3 of 3 artefacts match the pin, 9.9 s.
      - `cargo test -p cobolt-wasm --no-fail-fast`: 27 of 29 targets green. The same two reds as before this task:
        - `corpus`: the program `conditionals/test-arith-relation-after-and.cbl` prints its own timing (450 ms in the module, 970 ms in the interpreter);
        - `dependency_guard`.

- [x] **T12 — `Options.native` and the console capabilities in the generator** (R3, R10, R11 console part)
  - Files:
    - `crates/cobolt-wasm/src/lib.rs` (`Options.native`), the `lower*.rs` files it reaches, and `module.rs` (the manifest's capabilities);
    - new `crates/cobolt-wasm/tests/native_console.rs`.
  - Do:
    - With `native` set, these are lowered to `prn` imports:
      - `ACCEPT` from the console → `con_read`;
      - the environment and the arguments → `env_get`;
      - the exit code and `RETURN-CODE` at `STOP RUN` → `exit`.
    - In-memory INDEXED and `SORT` stay in the module.
    - The capabilities list names exactly what the program reaches.
    - With `native` unset, the generator refuses these as it does today.
  - Verify:
    - `cargo test -p cobolt-wasm --test native_console -- --nocapture`: one program per construct compiles, and its manifest lists exactly its capabilities. The same program without `native` gives today's refusal.
    - `cargo test -p cobolt-wasm`, `compile_determinism` and T1's test green.
  - *Done 2026-11-15 (2.0.497):*
    - **The option:** `Options.native` (default off) is in `module.rs`, where `Options` is defined; `lib.rs` only re-exports it. The Web and Mobile generator leaves it off.
    - **What compiles to the runner's imports** (ABI **A29**, new):
      - `ACCEPT` from the console (plain, or from a console mnemonic) → `prn.con_read`. A conformance build keeps reading handle 0, as before.
      - `ACCEPT … FROM ENVIRONMENT "name"`, `ENVIRONMENT-VALUE`, `COMMAND-LINE`, `ARGUMENT-NUMBER` and `ARGUMENT-VALUE` → `prn.env_get`.
      - `DISPLAY … UPON ENVIRONMENT-NAME` and `… UPON ARGUMENT-NUMBER` set two hidden registers of the region (a 255-character name, an eight-byte pointer) and show nothing, as on the desktop.
      - `ACCEPT … FROM ESCAPE KEY` and `CRT STATUS` move `"00"` and `"0000"`, as the desktop does: in the module, with no capability.
    - **The exit code** (operator, 2026-11-14: "Match Cargo now"):
      - The task asked for `RETURN-CODE` at `STOP RUN` to reach `exit`. The source says otherwise: the Cargo-built program and `rcrun run` end with 0 after `STOP RUN` whatever `RETURN-CODE` holds. `RETURN-CODE` is not in the generator at all, so a program naming it takes the Cargo path.
      - So `STOP RUN` and `GOBACK` end the run inside the module, the runner will end with 0, and `exit` stays reserved in A29 until `RETURN-CODE` reaches both paths. That gap is a fix of its own, for both paths, proposed separately.
      - *Later (2.0.498, 2.0.499):* the fix landed on `fixes-2x-return-code` and was picked here. The Cargo-built program and `rcrun run` now end with the code `RETURN-CODE` holds. The generator still refuses `RETURN-CODE` (`native_console` checks it), so such a program takes the Cargo path, and a module, which cannot name it, ends with 0. `RETURN-CODE` in the generator, and `exit` with it, fall to T15.
      - `STOP` and `GOBACK` are now *in the module* in the Desktop column: `support.rs`, the KB's paragraph and `subset_table`'s fixed points. The KB is regenerated.
    - **The manifest:** `capabilities` lists `console` and `env` as the module's `prn` imports reach them, in A27's order. `imports` names them `prn.con_read` and `prn.env_get`. A module that reaches neither writes no `capabilities`, and its bytes are the browser build's.
    - **The routine blob** (`rt.wasm`) gains `rt_accept_con` and `rt_accept_env`. T1's pin failed at each step until four rules held, and they are written above the new code in `accept.rs`:
      - **Order.** The routines are compiled in symbol-name order, and each is placed where it is first named. The new functions are named to sort after `accept_clock` (`accept_native_console`, `accept_native_environment`). A separate module, `native_accept`, sorted first and pulled `put` and `locate_sized` ahead of their old places.
      - **Signatures.** They use only signatures the blob already had, because the generator copies the blob's types in order. An `i64` argument, or a six-argument import, shifted every type index. So `env_get` writes its answer over the name it reads, and the argument pointer travels as an operand.
      - **No checks.** They index nothing and divide by nothing that can be zero. Each such check adds a 16-byte panic location to the read-only data and moves every static after it.
      - **Lines.** They stand below every line of the existing code, because a panic location records its line.
    - **The test host** (`host.rs`) gains `NativeInput` (the console's lines, the environment, the arguments) and `run_native`. Its `prn` imports answer only from that list.
    - **Measured:**
      - **`native_console`** (new): 85 of 85 checks, 5.4 s. 8 programs × 2 memory variants, 66 lines compared, compile 114 ms in all:
        - `console` (`console`): `[Ada Lovelace]`, `[036]`, `[long]`, `[   ]`, `[     ]`, `[007]`. These are the lines `rcrun run` printed with the same four lines on its standard input: the line end is removed, a long line is cut, an empty line is spaces, and at the end of the input an alphanumeric item gets spaces while a numeric one keeps its value.
        - `environment`, `arguments`, `no-arguments` (`env`), `escape-key-crt-status` (none) and `sort-procedures` (none): equal to the interpreter run in the same process with the same variables and arguments;
        - `console-and-environment` lists `["console", "env"]`; `in-memory-indexed` (none) reads its three records back in key order;
        - each capability-free module equals the browser build byte for byte; each module that needs the runner is refused by the bundler, which names its `prn` import.
        - Without `native`: the environment, the arguments, both `DISPLAY UPON` registers and ESCAPE KEY are refused as before, and the console's six `ACCEPT`s are ignored with six warnings.
      - **`desktop_wasm_unchanged`** (T1, AC16): 3 of 3 artefacts match the pin.
      - **`compile_determinism`:** 2 of 2. `same_input_same_bytes` covers 126 variants, 0 differing; 2 520 compiles in 23.5 s (107 compiles/s).
      - **`subset_table`:** 8 of 8. 33 entries in the module, 1 through the runner (`console`), 102 on the Cargo path; 5 of 5 fixed points.
      - **`rt_blob`:** 2 of 2.
      - **The KB:** regenerated (4 388 records). `prebuilt_chunked_kb_matches_the_published_documentation` and `the_browser_subset_section_names_every_builtin_and_statement` are green.
      - **`cargo test -p cobolt-wasm --no-fail-fast`:** 28 of 30 targets green. The same two reds as before this task:
        - `corpus`: `conditionals/test-arith-relation-after-and.cbl` prints its own timing (430 ms in the module, 920 ms in the interpreter);
        - `dependency_guard`.

- [x] **T13 — `cobolt-runner-engine`: one wasmtime setup, machine code and its cache** (R6, R15, R16; AC8 engine half)
  - Files:
    - new `crates/cobolt-runner-engine/` (a workspace member);
    - `crates/cobolt-wasm/src/host.rs`: `engine()` delegates to it.
  - Do:
    - **One `Config`** for the tests and the runner: multi-memory, NaN canonicalisation, an explicit target, parallel compilation.
    - **`precompile(module, target)`** targets x86-64-v2, and aarch64 at the Apple M1 level.
    - **`load(module, code, cache)`** goes in this order: sha256 → compatibility hash → `deserialize` → the local cache (least recently used, 512 MiB) → compile at load. It returns the path it took.
    - **The M1 spike:** does a baseline `.cwasm` load in an engine that detects the host's CPU? The answer, and the settings it forces, go in the evidence.
  - Verify:
    - `cargo test -p cobolt-runner-engine -- --nocapture`. Each of these runs, gives the same result and names its path:
      - a foreign compatibility hash;
      - truncated code;
      - absent code;
      - a cache hit on the second load.

      The result block gives each path's ms.
    - `cargo test -p cobolt-wasm` green.
    - `cargo tree -d -p cobolt-runner-engine` shows one `wasmtime`, `=44.0.3`.
  - *Done 2026-11-15 (2.0.511):*
    - **The crate** `crates/cobolt-runner-engine` (a workspace member): wasmtime `=44.0.3` with `cranelift`, `runtime`, `std` and `parallel-compilation`; `target-lexicon` 0.13 for the host's triple; `sha2`.
    - **One `Config`** (`config(target)`): multi-memory, NaN canonicalisation, parallel compilation, and an explicit target with its CPU flags named, so nothing of the compiling machine leaks into the code.
      - **`Target::baseline(triple)`** is what the Build makes code for: x86-64-v2 on x86-64 (`has_sse3`, `has_ssse3`, `has_sse41`, `has_sse42`, `has_popcnt`, `has_cmpxchg16b`), the Apple M1's level on aarch64 (`has_lse`, `has_pauth`, `has_fp16`).
      - **`Target::host()`** is what the runner and the tests run on: this machine's triple at the same baseline, less any flag this CPU lacks. Reading the source showed why: wasmtime refuses to compile *at load* for an engine whose flags the CPU lacks (`Engine::_check_compatible_with_native_host`), so a fixed baseline engine would leave an older aarch64 Linux machine (no LSE) with no way to run. On every Apple silicon Mac and every x86-64-v2 PC the two are equal, and the Build's code loads.
    - **`precompile(module, target)`** returns `MachineCode { bytes, sha256, compat }`: wasmtime's serialised module, its sha256, and the sha256 of the engine's `precompile_compatibility_hash`.
    - **`load(engine, module, code, cache)`** goes in the task's order and returns the module, its `LoadPath` (`MachineCode`, `Cache`, `Compiled`), why the code was passed by (`Skipped`: `Absent`, `Damaged`, `Foreign`, `Refused` with wasmtime's words) and the ms to a runnable module.
      - The order is: the code's sha256 → the compatibility hash → `deserialize` → the local cache → compile at load, and then keep the result in the cache.
      - **The cache:** one `<key>.cwasm` per module and engine, keyed by the sha256 of both. Each entry holds its own sha256 followed by the code. Entries are written whole and renamed into place. A damaged entry is removed and the module compiled again. Past 512 MiB (`CACHE_LIMIT`) the least recently used go first. A cache that cannot be read or written never stops a program.
    - **`cobolt_wasm::host::engine()`** is now `cobolt_runner_engine::engine()`. The generator's tests run modules on the runner's own engine (feature `host`).
    - **The M1 spike** (`spike_baseline_code_in_a_host_detecting_engine`, Apple M3 Pro, aarch64-apple-darwin): **yes.**
      - The baseline `.cwasm` loads in an engine that detects the host's CPU, with the same settings, and answers the model's value.
      - **It also loads in an engine with wasmtime's default settings**, which has no NaN canonicalisation. wasmtime's own load check (`Metadata::check_compatible`) compares the triple, a fixed list of code-generation flags, the tunables and the wasm features. It checks the CPU flags against the CPU. `enable_nan_canonicalization` is on its list of settings whose value "shouldn't matter" (`Engine::check_compatible_with_shared_flag`), so it is **not** checked.
      - **The settings it forces:** none beyond the triple. So the compatibility-hash step is what keeps code made with other settings out, and it stays ahead of `deserialize`. The two engines' hashes differ (`812d5a0c…` for ours, `6786c619…` for the defaults).
      - The runner therefore keeps the explicit-target engine (`Target::host()`). A host-detecting engine would load the Build's code but hash differently, so `load` would compile instead.
    - **Measured** (macOS arm64, debug build): `cargo test -p cobolt-runner-engine -- --nocapture`, 5 of 5.
      - **`every_load_path_runs_the_same_program`:** 22 of 22 checks. The module has 300 functions × 64 steps, is 594 680 bytes, uses two memories, and folds in a canonical NaN every 16 steps. Every path answers the Rust model's −1672998543259288415. The machine code is 1 068 488 bytes and took 1 140 ms to make. Load times:
        - the Build's machine code: `MachineCode`, 77.6 ms;
        - a foreign compatibility hash: `Compiled` (`Foreign`), 642.6 ms;
        - truncated code: `Compiled` (`Damaged`), 723.0 ms;
        - absent code: `Compiled` (`Absent`), 643.2 ms;
        - an empty cache: `Compiled`, which fills it, 891.4 ms;
        - **a cache hit on the second load:** `Cache`, 61.8 ms;
        - truncated code with a cache hit: `Cache` (`Damaged`), 80.8 ms;
        - a damaged cache entry: `Compiled` again, 399.7 ms.
      - **`precompile_targets_are_explicit_and_repeatable`:** aarch64 for Linux, Windows and macOS. Each target's machine code is the same bytes twice, and the three differ from one another (1 199 560, 1 199 624 and 1 068 488 bytes). x86-64 needs wasmtime's `all-arch` and is not built on an aarch64 machine: the Build makes code for its own platform.
      - **`the_compatibility_hash_follows_the_settings`:** two engines with these settings hash equal, and wasmtime's defaults hash differently.
      - **`the_cache_keeps_the_most_recently_used_within_its_limit`:** with a limit of two and a half entries, the least recently used entry goes and the touched one stays. The loads go `Compiled, Compiled`; `Cache`; `Compiled`; `Cache, Compiled`, and 2 entries are left.
      - **`cargo tree -d -p cobolt-runner-engine`:** one `wasmtime`, v44.0.3. Its duplicates are wasmtime's own (`hashbrown` 0.16/0.17, `syn` 2/3) and the test-only `wasm-encoder` 0.244 beside wasmtime's 0.246.
      - **`cargo test -p cobolt-wasm --no-fail-fast`** on the new engine: 118 tests in 32 targets, 0 failed. That includes `dependency_guard` (3 of 3, with `rayon`, `target-lexicon` and `sha2` now in the host's tree), `corpus`, `compile_determinism`, `native_console` and `native_return_code`.
      - **`desktop_wasm_unchanged`** (`cobolt-web-tests`): 3 of 3 artefacts match the pin.
      - **`dependency_guards_063`** (`cobolt-compiler`, over `Cargo.lock`): 3 of 3.

- [ ] **T14 — `cobolt-program-package`: the payload inside one executable** (R1, R8)
  - Files: new `crates/cobolt-program-package/` (a workspace member).
  - Do:
    - **The table of contents:** `{kind, offset, len, sha256}` entries, kinds `MODULE` and `MACHINE_CODE`.
    - **The carriers:** an overlay on ELF and PE, found through `PRLOC`; a `__PRPAYLOAD` segment on Mach-O, signed again ad hoc in pure Rust.
    - **`RunnerSettings`:** the type only; T28 fills it from the project.
    - **Evaluate `libsui`** against the hand-written carriers, and record the choice in the evidence.
  - Verify:
    - `cargo test -p cobolt-program-package -- --nocapture`:
      - a round trip on a copy of the test's own executable, on this platform's carrier;
      - a damaged table of contents is refused;
      - a payload whose sha256 does not match is refused.
    - On macOS, `codesign --verify --strict` passes on the result.
    - The other carriers run in `build-all-platforms.yml`'s test step on their systems.

- [ ] **T15 — `prc-runner` for console programs** (R4 runner half, R5, R6, R10, R16, R28 runner half; AC5 runner half)
  - Files: new `crates/cobolt-runner/` (a workspace member; binary `prc-runner`).
  - Do:
    - **Loading:** read the payload through `PRLOC`, then validate the manifest's range.
    - **Capabilities:** refuse any it does not provide, naming it. Messages come in six languages, chosen by the system locale, and never say "cobolt".
    - **Running:** load the machine code through T13, link `pr` and the console part of `prn` with `linker_for(abi, imports)`, run, and exit with the program's code. That needs `RETURN-CODE` in the generator and the `exit` import's signature (A29), left open by T12 and 2.0.498.
    - **Test pins:** `now` and `random` come from `PRC_TEST_CLOCK` and `PRC_TEST_SEED`.
  - Verify:
    - `cargo test -p cobolt-runner -- --nocapture`:
      - a console fixture module, packaged with T14 into the built `prc-runner`, gives under the pins the same output as `rcrun run` of the same program, byte for byte;
      - a module that declares `sql` is refused, naming `sql`;
      - a module of ABI 1 runs;
      - the refusal text exists in six languages and contains no "cobolt".
  - **Generator half done (2.0.507, 2026-11-15)**; the runner half waits for T13 and T14.
    - **`RETURN-CODE` in a desktop module** is a hidden whole-number register (`BINARY`, 18 digits, signed), zero when the run starts. It compiles in `MOVE`, `ADD`, `SUBTRACT`, `MULTIPLY`, `COMPUTE`, `DISPLAY` and comparisons with a number, `EVALUATE` included.
    - **Why only whole numbers:** the interpreter holds `RETURN-CODE` as a register of its own whose type follows the last value moved in (a fraction or characters stay as they are). The module cannot follow that, so it compiles only what it can prove is a whole number and refuses the rest, naming why. The program then takes the Cargo path, which ends as the interpreter does. Refused: a value that may carry a fraction or characters (a literal, an item with decimals, a division), `DIVIDE`, `ON SIZE ERROR` with it as a receiver, and any other use (`STRING`, a move into characters, a comparison with characters). A browser build still refuses it as undefined.
    - **`prn.exit(code)`** (ABI A30, capability `env`): where the run ends (after `STOP RUN`'s flush, `GOBACK` or `EXIT PROGRAM` from the main program, past its last statement), the module hands `RETURN-CODE` over, held to the 32-bit range as the interpreter's exit code is. The test host records it in `Outcome.exit_code`. A program that never names `RETURN-CODE` imports nothing new, and its module is the browser module byte for byte.
    - **Tests:** `tests/native_return_code.rs`: 14 programs × 2 memory variants against the interpreter (lines and exit code), 9 refusals, 4 programs past 18 digits, the browser build and a module that never names it; 142 of 142 checks. `native_console` now expects `RETURN-CODE` to compile natively.
    - **Past 18 digits** the module keeps the low 18 digits; the interpreter keeps more: WASM-PARITY #52. The operator decided on 2026-11-15 to keep the truncation (2.0.510).
    - **Found in passing, each a fix of its own** (fixed in 2.0.508 and 2.0.509 on `fixes-2x-run-end-flush` and `fixes-2x-exit-code-wrap`, picked here; 2.0.510 lets the "last statement" case of `native_return_code` DISPLAY again):
      - `pr_run` does not flush when the main program goes past its last statement, so on the test host the DISPLAY output of such a run is lost. That is older than T15.
      - The interpreter's exit code wraps past 64 bits (`CobolValue::as_i64` casts its `i128`): 2^64 ends with 0, −(2^64)+5 with 5.

- [ ] **T16 — The Build classifies and takes the WebAssembly path** (R1, R7, R8, R13, R14, R21, R24, R25; AC1 local half, AC11, AC12, AC13, AC15, AC17 compiler half; C8, C11)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `classify`, in plan §4.7's order;
      - `wasm_build`;
      - `finish_install`, factored out of :3244–3338;
      - `CargoReason`;
      - `CompilerError::CargoNeedsToolchain`;
      - the toolchain check, moved after `classify` (:2219–2223);
      - `BuildResult.cargo: Option<…>`;
      - the runner locator, built like `cobolt-project-tools/src/web/preview.rs::helper_path` (:212);
    - `crates/cobolt-compiler/src/project_kind.rs`: `DesktopSettings` (:1059) gains `build_path` (default `Wasm`) and `machine_code` (default `true`).
  - Do:
    - **The reasons:** Debug → `DebugBuild`; `build_path = "cargo"` → `Chosen`; `[[crates]]` → `ExternalCrate`; `EXEC RUST` → `ExecRust { file, line }`; a generator refusal → `Refused { construct, file, line }`; no runner → `RunnerUnavailable`.
    - **Not yet covered:** a construct whose native class (T10) is `Cargo(_)` is a `Refused` that carries that class's reason. At M1 this is how every form project falls back.
  - Verify: `cargo test -p cobolt-compiler` green, with these new tests:
    - **AC1 (local half):** a console project builds with `PATH` holding no `cargo` or `rustc`, `CARGO_HOME` and `RUSTUP_HOME` pointing at empty folders, and `bin/<name>` runs.
    - **`exec_rust_falls_back_before_compiling` (AC12):** the message names the block's file and line before any compile.
    - **`refused_construct_falls_back_naming_file_and_line` (AC11).**
    - **`no_toolchain_stops_and_leaves_bin_untouched` (AC17):** `bin/`'s tree hash is equal before and after.
    - **The Debug Build** takes Cargo with `DebugBuild`; the chosen setting gives `Chosen`; a missing runner gives `RunnerUnavailable`.
    - **The three twins:** lands in the chosen folder; a missing toolchain is named, now a typed error; current code, not a stale generated file.
    - **`no_project_source_in_built_file` (AC15):** no `PROCEDURE DIVISION` statement line of the project is found in the built file. Literals, data names and `EXEC SQL` text are excluded (C1).
    - **AC13:** `bin/` holds one executable plus the assets and licences the Cargo path writes, and `dist/` is copied as today.
    - `cargo build -p cobolt-ide -p cobolt-project-tools -p cobolt-cli` succeeds.

- [ ] **T17 — `rcrun build --cargo`, `--no-machine-code`, and the install hint** (R15, R24, R25; A4; AC8 first half, AC17 CLI half)
  - Files: `crates/cobolt-cli/src/main.rs`.
  - Verify: new `cobolt-cli` tests, with `rcrun` built first:
    - `--cargo` gives path Cargo, reason `Chosen`;
    - `--no-machine-code` gives a file whose table of contents has no `MACHINE_CODE` entry, while the default Build's has one;
    - with no toolchain, an `EXEC RUST` project prints how to install Rust 1.95.0, exits non-zero, and leaves `bin/` untouched.

- [ ] **T18 — The IDE: the Build's path, its phases, the install offer, and the Desktop settings** (R18, R24, R25; AC17 IDE half; i18n)
  - Files:
    - `crates/cobolt-ide/src/app.rs`:
      - the toolchain failure, today matched by its text (:17237), becomes the typed error;
      - `offer_rust_install(reason)` reuses the first-run offer (:2616) at Build time;
      - the Output shows the path, the reason and the phase times;
    - `crates/cobolt-ide/src/panels/new_project.rs`: the Desktop option rows (:508–527, `pk_o_*` and `pk_c_*`);
    - Project Settings' Desktop section, found from the readers of `DesktopSettings`;
    - `crates/cobolt-ide/src/i18n.rs`.
  - Do: add new `Tr` keys in all six languages for:
    - the two settings and their choices;
    - the eight phase names;
    - the seven reasons;
    - the path line;
    - the offer's title, text and button.
  - Verify:
    - The new `cargo test -p cobolt-ide --bin cobolt-ide build_path_report`:
      - a typed failure opens the offer;
      - the report's lines come out in six languages;
      - a project without the new settings reads the defaults and is not rewritten.
    - `no_empty_ui_translations` and `non_english_is_actually_translated` green.
    - The manual check (plan §6) is owed to the operator: an IDE Build on a machine with no toolchain. The window is never driven by an agent; the MCP `build` tool shows the same report (T19).

- [ ] **T19 — The project tools' `build` reports the path, the reason and the timings** (R13, R18)
  - Files: `crates/cobolt-project-tools/src/tools/` (the `build` tool), and its schema text.
  - Verify: `cargo test -p cobolt-project-tools build` green. On fixtures, the tool's result names:
    - the path;
    - the reason, with the file and line for `ExecRust` and `Refused`;
    - each phase in ms;
    - the peak.

- [ ] **T20 — `prc-runner` ships with the IDE** (R6; AC13)
  - Files:
    - `.github/workflows/build-all-platforms.yml`:
      - build `cobolt-runner`;
      - stage `prc-runner` beside the IDE, as `prc-web-preview` is at :235 and :302;
      - sign it ad hoc on macOS, as at :351;
      - list it in the installers' file lists;
    - `THIRD_PARTY_NOTICES.md`: wasmtime and its dependencies;
    - `crates/cobolt-compiler/src/lib.rs`: `SDK_CRATES` (:3833) and `stage_sdk`, if the Platform SDK's IDE builds Desktop projects (decided from `stage_sdk`'s readers, and recorded).
  - Verify:
    - The runner locator finds `prc-runner` next to the IDE binary and, in a checkout, in `target/<profile>/`. This is a compiler test.
    - The release workflow is dispatched once, outside the window. Each of the four packages contains `prc-runner`, and on macOS `codesign --verify --strict` passes.
    - The package step and the installers are otherwise unchanged (AC13).

- [ ] **T21 — The module corpus, kept for ever** (R5, R23; AC4)
  - Files:
    - new `tests/module-corpus/`: an index of every entry ever added, and its first entry, the console fixtures built by this release, with their expected outputs;
    - new `crates/cobolt-runner/tests/module_corpus.rs`.
  - Do: each release adds one entry within 256 KiB. The index is append-only.
  - Verify: `cargo test -p cobolt-runner --test module_corpus -- --nocapture`:
    - **`corpus_runs_on_this_runner`:** every entry runs and gives its recorded output. The result block counts releases, modules and identical outputs.
    - **`corpus_entries_are_never_deleted`:** an entry in the index with its files missing fails, and a planted case proves it.

- [ ] **T22 — CI: no toolchain, Rust releases, and across platforms** (R2, R5; AC1, AC2, AC3 console)
  - Files: new `.github/workflows/desktop-wasm.yml`, on tags and on dispatch.
  - Do:
    - **`no-toolchain`, on the four platforms:** a clean runner with Rust removed from `PATH`, `~/.cargo` and `~/.rustup`. It installs the release's IDE package, builds every project that takes the WebAssembly path at this milestone, and runs its check script.
    - **`determinism`:** generators built by Rust 1.95.0 and by `stable` compile the corpus and the reference projects with the same `SOURCE_DATE_EPOCH`, then compare the bytes. **Waits for F3.**
    - **`cross-platform`:** modules built on each platform run under the other three platforms' runners, compared as AC3 says.
  - Verify: dispatched once, outside the window; each job's result block is recorded in `WASM-PROGRESS.md`. AC1 is met only once every reference project takes the WebAssembly path. Until then the job lists those still on the Cargo path, with their reasons.

- [ ] **T23 — AC9 at M1, and the parity list** (R11, R21, R22; AC9)
  - Files: new `crates/cobolt-runner/examples/nist_runner_run.rs` (the `nist_wasm_run` pattern, with the module run by `prc-runner`); `specs/WASM-PARITY.md`.
  - Do: measure on both axes (GOLDEN RULE #9):
    - the NIST modules the interpreter has finished, in the native mode;
    - `tests/cobol` against the interpreter's output.

    Each divergence found goes into `WASM-PARITY.md`, with who is right.
  - Verify: the compile and execution counts per module, reported separately, plus the corpus's identical-output count, are written to `WASM-PROGRESS.md`. Nothing gates on them (R21).

- [ ] **T24 — Console parity, byte for byte** (R27 console part)
  - Files: new `tests/pixel-parity/console/` (console fixtures using each M1 construct); `crates/cobolt-bench/src/pixel_parity.rs` (`--console`).
  - Verify: `cargo run -p cobolt-bench --release -- pixel-parity --console` builds each fixture on both paths, runs both under the pins, and prints "programs N, identical output N of N". Any difference is a failure.

- [ ] **T25 — The Developer's Guide and the System KB for M1** (GOLDEN RULE #3, #8; plan §7)
  - Files:
    - `docs/developers-guide-en.md` §18, "Building a distributable binary":
      - the Desktop Build's default path;
      - the machine code and the setting that turns it off;
      - choosing the Cargo path (Project Settings, `rcrun build --cargo`);
      - what still takes the Cargo path and why;
      - what happens with no Rust toolchain;
    - §17: the new `rcrun build` options;
    - delete the guide's five translations;
    - the compiler's doc tables for the settings and options;
    - `assets/knowledge/chunked.data`.
  - Do: write it in the guide's voice, for COBOL developers, with no Rust. Add a mermaid diagram of the two paths and a Caveat on the known divergences (`WASM-PARITY.md`).
  - Verify:
    - `prebuilt_chunked_kb_matches_the_published_documentation` green.
    - `git status` shows the five translations deleted, never the English file.

- [ ] **T26 — M1 close**
  - Do: the milestone close (preamble).
  - Verify:
    - `cargo test -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-runner-engine -p cobolt-program-package -p cobolt-runner -p cobolt-compiler --no-fail-fast`, every `test result:` line read.
    - `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`: only the known reds of the 2.x line.
    - T1's test green.
    - `build-goal` names each reference project's fallback ("forms").
    - The runner's size goes into `WASM-PROGRESS.md` (C12).

## Phase M2 — Forms

- [ ] **T27 — `cobolt-abi-host`, extracted from the player unchanged** (R9; plan §4.3)
  - Files:
    - new `crates/cobolt-abi-host/`: the player's pure `Host`, `FormTree` (`tree.rs`), `methods.rs` and their closure, moved with their tests;
    - `crates/cobolt-web-player/Cargo.toml`: a path dependency, plus a re-export;
    - `crates/cobolt-web-player/src/fingerprint.rs`: `INPUTS` (:33) gains the new crate.
  - Verify:
    - The player `--lib` gives the same pass count as before the move, and is green.
    - `cargo test -p cobolt-abi-host` green.
    - The player's `wasm32` build succeeds. The shipped player is rebuilt if the freshness test asks.
    - T1's test green.

- [ ] **T28 — The forms, menus, settings and art inside the module** (R1; C1; AC15, AC16)
  - Files:
    - `crates/cobolt-compiler/src/lib.rs`:
      - `desktop_form_specs`;
      - `RunnerSettings` made by the one function that also feeds the settings baked into `main.rs` (:3059–3084);
    - `crates/cobolt-project-tools/src/web/generator.rs` (:149–295) uses the shared builder; its browser-only lowering (:251–268) stays its own;
    - `crates/cobolt-wasm/src/module.rs`: the sections `prforms`, `prmenus`, `prsettings` and `prart`, each with its own version.
  - Verify:
    - T1's test green: the Web module's bytes are unchanged.
    - A compiler test: for each reference project, the settings baked on the Cargo path equal the `RunnerSettings` in the module.
    - `no_project_source_in_built_file` covers a form project.
    - `cargo test -p cobolt-project-tools` green.

- [ ] **T29 — `ProgramEngine` in the form host; today's path becomes `InterpreterEngine`** (R9; C5)
  - Files: `crates/cobolt-form-host/src/host.rs`: `build_form_instance_as` (:5314), and the interpreter thread (:5379–5447) behind the trait.
  - Verify:
    - `cargo test -p cobolt-form-host` green.
    - Every `rcrun` host test green.
    - `pixel-parity --cargo` matches every golden: the refactor moved no pixel.
    - `engine_reference_form_parity_static_vs_faces` green.

- [ ] **T30 — `WasmEngine`: one form** (R9, R10, R28 runner half)
  - Files: `crates/cobolt-runner/src/` (`WasmEngine`, one program thread, the ABI host from T27).
  - Do:
    - Run one form: its controls, properties, methods, events and timers.
    - The runner honours `PRC_HEADLESS_SCRIPT`, `PRC_HEADLESS_LIMIT` and `PRC_HEADLESS_SHOTS`, as `main.rs` does (:4873–4874).
  - Verify:
    - A new `cobolt-runner` test, `interpreter_and_wasm_engine_agree_on_one_form` (`-- --nocapture`), runs a fixture form headless under both engines and compares their dumps after every step (AC14 part).
    - The fixture's `shot` frames are equal in every pixel.

- [ ] **T31 — `WasmEngine`: several forms, the shell and the SideMenu** (R9, R10)
  - Do: `OpenFormSync`, `OpenFormAsync`, `super::`, the application shell with its SideMenu, ContentPane and breadcrumb.
  - Verify: runner twins of the `rcrun` host tests `side_menu_hosted_controls`, `side_menu_rows_hosts`, `layers_hosts_091` and `perspective_hosts_096`, equal in their dumps and frames.

- [ ] **T32 — `WasmEngine`: dialogs, file dialogs and the scripted answers** (R10)
  - Do: message boxes and file dialogs run under the runner, and the `answer` step of T6 drives them headless.
  - Verify: a runner test where a fixture opens each dialog kind, answered by script, gives equal dumps and frames under both engines.

- [ ] **T33 — Desktop controls the ABI host lacks fall back by name** (R13; plan §4.3)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`classify`: `DesktopControl { form, control }`), and the native column (T10) updated for what M2 covers.
  - Verify: a compiler test with a form holding one control the ABI host lacks. It builds on the Cargo path, and the message names the form and the control. The AC10 test stays green.

- [ ] **T34 — Pixel parity for forms** (R27; AC19)
  - Files: `crates/cobolt-bench/src/pixel_parity.rs` (the pairs mode, the default), and `.github/workflows/pixel-parity.yml`.
  - Do: for every reference project the WebAssembly path builds:
    - build it both ways;
    - run every script under both binaries, at the three sizes, with the same pins;
    - compare every pair exactly, and the reads and dumps.
  - Verify:
    - `cargo run -p cobolt-bench --release -- pixel-parity` prints, per project:
      - the path taken;
      - forms, steps and sizes;
      - frames compared;
      - differing pixels, which must be 0;
      - golden matches.
    - A project counts as moved only when it shows 0 differing pixels.
    - The CI job is dispatched once on the four platforms, outside the window, and its blocks are recorded.

- [ ] **T35 — The same form on every surface and on every platform** (R2, R5, R9; AC3 forms, AC14)
  - Verify:
    - **AC14:** a test renders a reference form in the designer's render path, under `rcrun run-form` and under the runner, and gets equal pixels. `engine_reference_form_parity_static_vs_faces` green.
    - **AC3:** the `cross-platform` job of T22 gains the form projects that moved.

- [ ] **T36 — M2 close**
  - Do: the milestone close (preamble).
  - Verify:
    - The first 30 s judgement on the WebAssembly path, for each reference project that moved (AC6).
    - The guide's fallback list is updated.
    - The sweeps of T26, plus `cargo test -p cobolt-form-host -p cobolt-abi-host` and `cargo test -p cobolt-forms --features render`.

## Phase M3 — Disk files, and `SORT` and `MERGE` with files

- [ ] **T37 — `SequentialFile` and the sort core, extracted from the interpreter unchanged** (C5)
  - Files: new `crates/cobolt-runtime/src/seqfile.rs`, the sort core beside it, and the interpreter's file and `SORT` code calling them.
  - Verify:
    - `cargo test -p cobolt-runtime` green.
    - The NIST execution runs of SQ, IX, RL and ST give the counts recorded for the interpreter (GOLDEN RULE #9: every finished module's execution pass).

- [ ] **T38 — Files of every organisation as capabilities** (R10, R11)
  - Files:
    - `crates/cobolt-wasm/src/` (the file verbs lowered to the `prn` `f_*` imports under `native`);
    - `crates/cobolt-runner/src/` (the `files.*` capabilities over `SequentialFile`, `RelativeFile` and `IndexedStore`);
    - the native column.
  - Do: records cross by `(ptr, len)` in memory 0. `FILE STATUS` values are the interpreter's.
  - Verify: one runner test per organisation and verb. Its file contents and `FILE STATUS` values are equal to the interpreter's.

- [ ] **T39 — `SORT` and `MERGE` with `USING`/`GIVING`; `COMMIT`, `ROLLBACK`, `UNLOCK`** (R11)
  - Verify: runner tests equal to the interpreter's output and files. The native column moves these entries to `Host(sort)` or `Host(files.*)`.

- [ ] **T40 — M3 close** (AC9 for SQ, IX, RL, ST)
  - Verify:
    - The NIST SQ, IX, RL and ST counts on both axes through the generator and the runner, reported separately.
    - The ten-times project's path and time.
    - The milestone close.

## Phase M4 — SQL and HTTP

- [ ] **T41 — `EXEC SQL`, the SQL object and the SqlDatabase control** (R10, R11)
  - Files: the `EXEC SQL` binding, extracted from `cobolt-runtime` unchanged (C5); `sql_exec`, with the statements in a `prsql` section; the runner's `sql` capability.
  - Verify: runner twins of the interpreter's `EXEC SQL` and backend tests give equal results. `cargo test -p cobolt-runtime` green.

- [ ] **T42 — HTTP and RestClient** (R10, R11)
  - Do: `http_call` suspends and resumes the module as `remote` does.
  - Verify: a runner test against a local stub server gives the interpreter's results. No reference script calls the network.

- [ ] **T43 — M4 close**
  - Do: the milestone close (preamble).

## Phase M5 — National and UTF-8 data, and the rest of the table

- [ ] **T44 — National and UTF-8 data, computed in the module** (R11; C4)
  - Files: the national code extracted from `cobolt-runtime` unchanged; its lowering in `cobolt-wasm` and in `cobolt-wasm-rt`; the blob rebuilt with `cargo run -p cobolt-wasm --example build_rt`.
  - Verify: twins of the interpreter's national tests give identical bytes. `rt_blob` green.

- [ ] **T45 — `SET` pointer, and every other `Cargo(_)` entry but `EXEC RUST`** (R11, R12; AC10)
  - Do: lower `SET` pointer (reclassified by F2) and each remaining entry the native column sends to Cargo.
  - Verify: the AC10 test gains an assertion that `EXEC RUST` is the only `Cargo(_)` entry left. Each entry has a runner test equal to the interpreter.

- [ ] **T46 — M5 close**
  - Do: the milestone close (preamble).

## Phase M6 — The non-visual controls

Each task here moves its controls out of the `DesktopControl` fallback. It is verified by runner twins of the `rcrun` host tests for those controls, and by pixel parity on every reference project that uses them.

- [ ] **T47 — AgentObject, KnowledgeBase and WebSearch** (R11)
  - Verify: twins of `model_list_hosts` and the agent and KB host tests, against local stubs.

- [ ] **T48 — Maps, the Aws controls, model lists and MCP** (R11)
  - Verify: twins of `aws_hosts`, `partiql_hosts` and `powercloud_hosts`, against local stubs. No twin sends a request signed with the live keys some example `.cfrm` files carry.

- [ ] **T49 — AUDIO, ANIMATIONSEQUENCE and LAYOUT** (R11)
  - Verify: runner twins equal in dumps and frames, with animations under the virtual clock.

- [ ] **T50 — Viewer verbs, the window methods and canvas frames** (R11)
  - Verify: runner twins. The ABI host's list of unimplemented window methods is empty.

- [ ] **T51 — M6 close** (R11's end state)
  - Verify:
    - The `DesktopControl` fallback is reached by no reference project.
    - Every reference project takes the WebAssembly path and passes AC19, so AC1 is met.
    - The milestone close.

## Phase Z — Closing

- [ ] **T52 — i18n audit** (spec §6)
  - Verify:
    - Every `Tr` key added by T18 and later exists in EN ES PT JA ZH FR.
    - `no_empty_ui_translations` and `non_english_is_actually_translated` green.
    - A `grep` of user-facing text, the runner's messages included, finds no "cobolt".

- [ ] **T53 — The System KB and the Developer's Guide, final** (GOLDEN RULE #3, #8)
  - Verify:
    - The guide's §17 and §18 describe the finished Build. The five translations are deleted.
    - `build_chunked_kb` is run, and `prebuilt_chunked_kb_matches_the_published_documentation` is green.

- [ ] **T54 — Finalize**
  - Do:
    - Check off the ACs in `spec.md` with the evidence named in `WASM-PROGRESS.md`. Set the status to done here and in the spec.
    - Before the commit, quarantine untracked program files in other languages and run the credential scan.
  - Verify:
    - `cargo test --workspace --no-fail-fast`, every `test result:` line read: only the known reds.
    - The player `--lib` green.
    - `rcrun` builds.
    - The four CI workflows (`build-goal`, `pixel-parity`, `desktop-wasm`, `build-all-platforms`) dispatched once and recorded.

## Acceptance criteria → tasks

| AC | Requirements | Verified in |
|---|---|---|
| AC1 | R7 | T16 (local), T22 (CI, as projects move), T51 (all moved) |
| AC2 | R2 | F1, T11, T22 (`determinism`) |
| AC3 | R2, R5 | T22 (console), T35 (forms) |
| AC4 | R5, R23 | T21 |
| AC5 | R3, R4 | T9 (bundler), T15 (runner) |
| AC6 | R17, R18 | T4 (harness, baseline), T36 (first judgement), each close, T54 |
| AC7 | R20 | T2, T4 |
| AC8 | R15, R16 | T13 (engine), T14 (payload), T17 (the setting) |
| AC9 | R11, R21, R22 | T23, T40, each close |
| AC10 | R12 | T10, T45 |
| AC11 | R13 | T16 |
| AC12 | R14 | T16 |
| AC13 | R8 | T16 (the `bin/` layout), T20 (packages, installers, SDK) |
| AC14 | R9 | T30, T35 |
| AC15 | R1 | T16 (console), T28 (forms) |
| AC16 | R24, WASM-010 | T1, held by T9, T11, T12, T27, T28 |
| AC17 | R25 | T16, T17, T18 |
| AC18 | R26 | T3 (generation), T4 (hashes on four platforms) |
| AC19 | R27 | T7 (goldens), T8 (goldens after `HostClock`), T29 (refactor guard), T34 (pairs), each close |
| AC20 | R28 | T7 (per form), T8 (`deterministic_mode_repeats_frames`) |

**Requirements with no AC of their own:**
- **R6, R10:** T13, T15, T20, and T30–T32.
- **R19:** T4.
- **R21:** T16, the default.
- **R22:** T23, `WASM-PARITY.md`.
- **R24's opt-out:** T16–T18.

## Done criteria

This spec is done when:
- every acceptance criterion in `spec.md` is checked, with its evidence named in `WASM-PROGRESS.md`;
- the sweeps of T54 are green apart from the known reds;
- the System KB is regenerated and the Developer's Guide describes the finished Build, with its translations deleted;
- F1 and F2 landed as fix commits on their own branch, and every feature task as a feature commit on `features-2x-wasm-program-format`, each with its own `z` and CHANGELOG entry, pushed outside the forbidden window;
- nothing was merged into `main` without the operator asking.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# WASM series — progress ledger

> The single source of truth for **where the 2.x.x work stands**. Read it first after any interruption; update it after every verified change. It says what is *measured*, not what is hoped. A row is **done** only when its acceptance criteria were run and passed, and the evidence is named.
> Modelled on `NIST/progress.json` (GOLDEN RULE #9: finish one thing end to end before the next; report two numbers, never a weaker claim as a stronger one).

## Operating rules (so that anyone can resume)

- **Where:** the worktree `/Users/emersonlopes/Documents/PowerRustCOBOL/.claude/worktrees/features-2x`, branch `features-2x` (GOLDEN RULE #5 for 2.x.x: classify, work on `features-2x`/`fixes-2x`, merge into `2.x.x` only when asked). `2.x.x` is a fast-forward away; `main` is the 1.x.x line.
- **Never push** during Mon–Fri 09:00–18:00 America/Sao_Paulo (the hook enforces it); the operator has not asked for a push at all.
- **README.md and `docs/` are not touched** on 2.x.x. What 2.x.x says about itself is written in `specs/`.
- **Every change** bumps `z` in `crates/cobolt-ide/src/version.rs` and adds a top `CHANGELOG.md` entry. **When inserting an entry, put the new entry above the old heading and keep that heading** — an edit that replaces the old heading swallows it. Check with:
  `grep '^## \[PowerRustCOBOL 2\.' CHANGELOG.md | sed -E 's/## \[PowerRustCOBOL 2\.0\.([0-9]+)\].*/\1/' | awk 'NR==1{p=$1;next}{if($1!=p-1)print "GAP "p" "$1;p=$1}'` (no output = no gap).
- **Gates before a commit:** build and test the crates touched; scan the staged diff for credentials (`AKIA…`, `ASIA…`, private keys, 12-digit ids, `arn:aws`) **as a command that aborts the commit**; no untracked foreign-language source; no `README.md`/`docs/` change.
- **i18n:** every IDE string is a `Tr` field in six languages. **System KB:** a change to controls, properties, methods, events or `CALL` phrases updates the compiler's doc tables and regenerates `assets/knowledge/chunked.data`.
- **Interpreter/binary parity:** a runtime fix reaches `rcrun run-form`, embedded child forms, the compiled binary and the backend; the generated module is held equal by the conformance runs (WASM-010 R16).
- **Throwaway scripts** live outside the repository (the session scratchpad or `PowerRustCOBOL-local-settings/`).
- **Environment:** Rust 1.95 stable (MSRV 1.92); `wasm32-unknown-unknown` installed 2026-10-20; `trunk`, `wasm-bindgen-cli`, `wasm-tools` installed with `cargo install` (see the install log in the scratchpad); Chrome and Safari are present for the browser runner; node is present; the machine had 78 GB free — clone `target/` with `cp -cRp` instead of rebuilding.

## Status by spec

Legend: **S** specified · **P** planned (plan.md) · **T** tasked (tasks.md) · **I** implementing · **V** verified (acceptance criteria run, evidence named).

| Spec | Subject | S | P | T | Implementation | Evidence / next action |
|---|---|---|---|---|---|---|
| WASM-001 | Project kinds, selector, tree entries | ✔ | | | | |
| WASM-002 | WebAssembly target, host page, isolation headers | ✔ | | | | |
| WASM-003 | Backend, WebSocket channel, JWT | ✔ | | | | |
| WASM-004 | Control reuse on the web (classes A–E) | ✔ | | | | |
| WASM-005 | `CALL … WITH WEBSOCKET LINKAGE`, JSON mapping, remote data | ✔ | | | | |
| WASM-006 | WebLink | ✔ | | | | |
| WASM-007 | CSS3 and the form stylesheet | ✔ | | | | |
| WASM-008 | Web-view Preview | ✔ | | | | |
| WASM-009 | Authentication control, identity providers | ✔ | | | | |
| WASM-010 | WebAssembly code generation | ✔ | ✔ | ✔ | V (2.0.36, natively) | T1–T25 done; see the 2.0.36 section; owed: the operator's manual check (plan §6) and a browser run against a live backend |
| WASM-011 | Navigation, SPA and standalone styles | ✔ | ✔ | ✔ | **I** — 23 of 26 tasks done (T1–T12, T14–T24; **T17 on 2.0.279, 2026-11-06**: a `super::` read waits for the opener's own answer — ABI A24 `GET_WAIT`/`CALLR_WAIT` and `-11`, the generator hoists a statement's `super::` reads in front of it and suspends once, the player asks the opener by id and resumes on the reply, a timeout or a closed opener ends in the error path; proved by 2 generator tests against a host that answers a plain read `STALE`, 5 player host tests and one test that runs the **generated** module `superread` in the real player host on both memory variants; player rebuilt, `freshness` green); **T25 on 2.0.280, 2026-11-06:** AC1, AC2, AC3, AC4 (Chromium, popup blocker played by `window.open` answering `null`), AC5 first half, AC7, AC8 run in headless Chrome 157 (`cobolt-web-bundle/tests/acceptance_spa_chrome.rs`, `acceptance_standalone_chrome.rs`); the AC5 run found and fixed a defect of 2.0.279 (the page's glue gave an empty answer for "on its way"); **2.0.281, 2026-11-06: `INVOKE … ON EXCEPTION … [NOT ON EXCEPTION …] [END-INVOKE]`** (ABI A25: soft queries `0x15`/`0x16`, `rt_query_failed`; parser, AST, resolver, desktop interpreter, generator for `super::`, player host that reports nothing for a failure the program handles) — **AC5 now passes whole**: with the opener closed the guarded child runs `ON EXCEPTION` (`gone`, 0.2 s), with 0 error banners, 0 exceptions, 0 console errors; proved also by 2 parser tests, 6 generator runs, 3 host tests, 8 runs of the generated module `superguard` in the real host and 4 desktop cases | Open: **T13** (blocked on WASM-009 R11/Q4), **T25** left: AC4 in Firefox (not installed), AC6 sign-in half, AC9 and AC11, two applications on one page, **T26** finalize. `EVALUATE TRUE WHEN super::…` conditions and a plain `super::` read in `MOVE`/`IF` still read the replica or end the handler with the error when the opener is gone (only the `INVOKE` form carries the clause; documented in A24/A25) |
| WASM-012 | Mobile shell (Tauri 2) | ✔ | | | | |
| WASM-013 | Security architecture | ✔ | | | **I** — all 25 tasks shipped (2.0.259–2.0.277; T1 and T16 on 2.0.273, the advisory baseline owed; T18 on 2.0.274; T19–T22 on 2.0.275; T23 and T24 on 2.0.276; T25 on 2.0.277); not **V** while the 5 owed rows and the device measurements stand | The scan of the tracked examples finds 10 secret shapes in 7 files (not opened, not edited): **for the operator**. The shipped player (`assets/web-player/`) was rebuilt on 2.0.277 (freshness green again; the player crate 465 passed, 0 failed). T25's sweep found and fixed three defects: the gate hid the class-C secret-on-form message behind the generic scan, the scan took the AgentObject's `MaximumTokens` for a token, and a WASM-014 test raced on a process-wide slot. Fuzz budgets (release, 1×; recorded `PR_FUZZ_ITERS=200`): `fuzz_frames` 0.6 s, `random_round_trip` 2.0 s (the wire pair 9.0 min), `fuzz_dispatch` 2.9 s, `fuzz_tokens` 1.4 s (the server pair 7.6 min), 0 panics. Pinning: the table is checked (T21); writing the platform files is WASM-012's. The threat model has 49 rows over 7 boundaries and **5 owed** (TM-30 WASM-016; TM-37 to TM-40 WASM-012): see the WASM-013 section at the end of this file. Next: nothing in this spec but its owed rows (WASM-012, WASM-016), the operator's items and the device measurements. |
| WASM-014 | Backend contract and evolution | ✔ | ✔ | ✔ | **I** — all 21 tasks shipped (T2–T6, T8–T17 on 2.0.2xx–2.0.233; T1 and T7 closed on 2.0.278: the `negotiate` table with its ten rows and the baseline counts, and `cobolt-web-player/tests/wire_codes.rs`, which holds the player's platform codes equal to the wire's with no hand-kept list): evolution rules with structured `Break`s, `check_contract` and the Output report (T16), idempotency/ordering, long calls and cancel (T12–T13), streams (T14), pieces upload/download and the player's progress/stream handling (T15), the Backend settings `[contract]` (T17), the System KB text (T18) | Evidence by AC, all in `cobolt-server/tests/client_contract.rs`: AC1 `ac1_ac10_a_client_built_for_an_older_minor_is_accepted_with_a_warning_and_a_newer_one_is_told_to_update_the_backend`; AC3 `ac3_a_deprecated_program_works_and_the_warning_reaches_the_observer`; AC4 `ac4_each_failure_class_is_produced_and_the_program_reads_class_and_code`; AC7 `ac7_calls_on_one_connection_keep_their_order_unless_the_program_is_independent`; AC5 and AC6 in the same file; AC8 streams `cobolt-runtime/tests/test_streams.rs` and `closing_the_form_unsubscribes`; AC9 `upload_3mb_in_pieces_resumes_after_a_cut_and_checksums` (player `net` tests, 3,145,728 bytes in 16 pieces, cut after 5, resumed at 983,040); AC2 evolution table `check_contract_reports_program_item_rule_without_cargo` (the `rcrun backend build --release` half needs cargo: **owed**). **Owed:** AC10 end-to-end (gated by WASM-010 T16 and module imports T15b); no run on Windows, Linux or behind a proxy; OQ1–OQ3 open (working assumptions are in the System KB); AC11 with WASM-003 T15b-3/T15b-4. Details: `specs/WASM-014-backend-contract-and-evolution/tasks.md` |
| WASM-015 | Testing, agent tooling, knowledge | ✔ | ✔ | ✔ | **I** — T1–T12, T15–T24 shipped (2.0.236–2.0.256): the shared `Step` script, `require_node`, the test kit in `cobolt-server` (`testkit` feature), the `cobolt-web-tests` crate, the browser runner (`web::runner`, WebKit measured) and the backend runner (in process, and the built binary with `PRC_WEB_E2E=1`), the control coverage table (71 of 71 rows; A/B 41 of 41; 21 known class C gaps), seven more desktop control files, web goldens (10), the agents' tools on web projects (`check` gates, `engine`, `secret_scan`) and both servers agreeing, the plugin rules (14 → 15, `web-layers`), `WebFormsDesigner` and the Web Form Designer Agent with its reviewer, the web knowledge census (156 of 156), `examples/PowerWebHello` and the CI jobs. **Owed:** T13b (the matrix cells on the browser runner; the size report is done, 2.0.257) (T14, bench rows, is done: 2.0.258); T11 (module runner, 2.0.255) and T12 (the ledger `wasm` column, `nist_wasm regress`, 2.0.256) are done, T9b, T10b, T16b, T22b (the click from the page to the Backend: the kit's server speaks TLS, no web view trusts it), the first CI run's engine per cell, and the operator's manual check (Grace, a SqlDatabase on a web form). Sweep of 2.0.254: 242 targets, 2794 passed, 0 failed (project-tools, web-tests, web-bundle, web-preview, wasm, agents, bench, compiler, server `testkit`, forms `render`); `cobolt-ide` bin 1960 passed, 8 failed (the 3 known and the 5 that need `rcrun`/a backend binary in a fresh worktree); `tests/controls` 50 binaries green. Evidence by AC (WebKit, this Mac): AC2 `cobolt-web-tests/tests/runner.rs::hello_world_bundle`; AC3 `cobolt-server/tests/runner.rs` (in process; binary mode with `PRC_WEB_E2E=1`); AC4 `coverage.rs` and `controls_roundtrip_chrome` (39 controls, 332 checks); AC5/AC13 the `*_comparison_chrome` and `layers_chrome` tests; AC7 `web_golden.rs`; AC8 `agent_tools.rs` and `servers_agree.rs::web_tool_sequence`; AC9 `servers_agree.rs::both_servers_agree`; AC10 `cobolt-project-tools/tests/web_rules.rs`; AC11 `web_knowledge_is_complete` and the Grace tests in `agents_db.rs`/`grace_host.rs`; AC12 `cobolt-web-tests/tests/examples.rs` (except T22b). Open: AC8 size report and bench rows with T13–T14; AC1 and AC6 shown with T12. |
| WASM-016 | Prebuilt player | ✔ | | | | |
| WASM-017 | Module architecture, memory, threads, boundary | ✔ | | | | |
| WASM-018 | Canvas 2D/3D and graphics objects | ✔ | ✔ | ✔ | I — part 1 (T1–T10) done; part 2: T11 (2.0.188), T12 (2.0.189), T13 (2.0.190), T15 (2.0.191), T16 (2.0.192) done | T11: `cargo test -p cobolt-graphics-wire -p cobolt-graphics --no-fail-fast` all green; 61 commands encode to the bytes recorded before the move. T12: shared device renders the golden; oversized texture refused naming the limit. T13: renderer explicit, egui_glow not built. T15: 313 graphics methods, 0 missing. Next: T14 (seven `ControlType` variants; large, touches every exhaustive match) and T16. Branch `features-2x-wasm018`. |

## Done elsewhere

- `CALL … RETURNING` (WASM-005 Q2, D6): fix `8a15f00d` (1.91.219) on `main`, forward-port `afe22c51` on 2.x.x — tests 14/14 runtime, NIST baselines held.

## The web page against the desktop — PowerAnalyticsWeb (2026-10-24, version 2.0.65, WASM-016 R14)

`examples/PowerAnalyticsWeb` is a Web project made from the desktop's `PowerAnalytics` (its dumped state, by a throwaway script kept outside the repository). It is a **look** clone, not a functional port: only `BTN-DASH` and `BTN-DATA` have handlers, so the page switch works and the Levels and search popups, the paging and the filters do not run. The popups are therefore checked as static pages with a desktop run's state baked in, and the search popup has no desktop reference at all (`run_form` with `set TXT-CAT.Text` does not open `POPUP-SEARCH`). A working port is a separate, large job.

**Measured** — the page's mean absolute error per pixel (0–255) against the desktop's own picture of the same form, and in brackets the share of pixels that differ by more than 24:

| Picture | Error |
|---|---|
| Dashboard at 1440 × 900, desktop run state | **2.31** (2.02 %) — 161 of 161 boxes within 0.5 px |
| Dados page, desktop run state | **4.18** (3.76 %) — the grid is 70 % of what is left, nearly all of it glyph anti-aliasing (#27) |
| Levels popup open, desktop run state | **2.16** (1.99 %) |
| The same form through `render_form`, at 1920 × 1080 / 1280 × 720 / 1100 × 800 | **2.40** (2.50 %) / **2.51** (2.67 %) / **2.70** (3.15 %) |
| Medium: 1000 × 700 / 800 × 900 / 700 × 700 | **4.31** (5.24 %) / **3.77** (4.66 %) / **4.55** (5.59 %) — 1.53 / 1.68 / 1.87 above the clip line of #26, which is all that separates them |
| Compact: 420 × 800 / 360 × 800 | **3.95** (4.73 %) / **4.05** (4.87 %) |
| The real bundle against the static page | **0.02** on a fresh load at 1920 × 1080, **0.00** after the live page is resized 1440 → 1000 × 700, 1000 → 1920 × 1080 and 1440 → 420 × 800 |

Geometry, the page's boxes against the desktop solver's (`examples/parity_page --geometry` and `--compare`): every control within half a pixel at **1080, 1100, 1280 × 720, 1366, 1440, 1600, 1920 and 2560** wide (Expanded), at **648 to 1023** (Medium) and at **360 × 800 to 599** (Compact). The exceptions are the surfaces below the form's minimum (#24) and six boxes the desktop squeezes to nothing at 1024–1079 (#25). Before this work Medium was 153 of 160, 600 × 900 was 45 of 160 and Compact 117 of 124.

What changed in the page: the desktop's text arithmetic (`cobolt_forms::textmetrics`), the stand-in face and its licence in every bundle, the shadows, Spatial's measured rules, the charts painted as the desktop paints them, the DataGrid, ComboBox and ListBox drawn the way the painter draws them, `FontScaling` (#33), a breakpoint's `Visible` (#34) and the floor of a card (#35). Still different, and why: #24–#32 in `WASM-PARITY.md`. Measured with throwaway tools outside the repository; the repository holds `crates/cobolt-web-player/examples/parity_page.rs` (the static page and the geometry comparison) and the tests.

## WASM-010 part 2 — the rest of the subset, suspension, the Build (2026-10-23, version 2.0.36, T14–T25)

**NIST on the final module** (`RCRUN=…/nist_wasm_run nist_conformance run NC|IF|IC`), against 2.0.35:

| Module | Compiled | Programs reporting 0 failures | Assertions PASS / FAIL / DELETED | 2.0.35 |
|---|---|---|---|---|
| NC (Nucleus) | **95 of 95** | **95 of 95** | **4 612 / 0 / 5** | the same |
| IF (Intrinsic Function) | **45 of 45** | **45 of 45** | **841 / 0 / 0** | the same |
| IC (Inter-Program Communication) | **25 of 25** | **25 of 25** | **309 / 0 / 4** | the same |

No program excluded, timed out or crashed.

**The interpreter, unchanged** (`cobolt-runtime` was not touched; release `rcrun`, `nist_conformance run`), every finished module against `NIST/progress.json`: NC 95/95 4 614 PASS, IF 45/45 841, IC 25/25 309, SM 16/16 311, ST 39/39 735, SQ 85/85 624, IX 41/41 574, RL 34/34 354 — 0 FAIL everywhere, equal to the ledger. The AC10 suites (`cobolt-runtime`, `-parser`, `-semantic`, `-server`, `-compiler`): 2 201 passed in 245 binaries, 4 failed — all pre-existing: `SDK_CRATES` lacks `cobolt-decimal` and `cobolt-numedit`, which `cobolt-runtime` and `cobolt-forms` already need at 2.0.35, so the SDK closure test and three SDK builds fail (a `fixes-2x` item).

**Phases (R18)**, each with what measures it:

| Phase | State | Evidence |
|---|---|---|
| A — layout, MOVE, arithmetic, IF, PERFORM, GO TO, EVALUATE, host properties and methods, events | done | NIST NC; `layout_golden.rs` (AC2, 148 bytes), `arith_table.rs` (AC3, 15 rows), `control_flow.rs` (AC4), `forms.rs` (AC1) |
| B — tables, STRING/UNSTRING/INSPECT, numeric-edited, intrinsic functions, nested programs, local CALL | done | NIST IF and IC; `corpus.rs` (17 of 17 compiled `tests/cobol` programs identical) |
| C — suspension, the remote CALL, the JSON mapping | done natively | `json_vectors.rs` (176 of 176 answers equal to `cobolt-wire`), `forms.rs::suspend_resume_with_queued_events` (AC5), `yield_is_invisible` (R9); **no browser run against a live backend yet** (WASM-014 T15) |
| D — the rest of the subset | done, with named exclusions | value methods, layers, parity gaps #7/#9/#10/#13, in-memory INDEXED and SORT (`memory_files.rs`); refused with their construct: disk and sequential files, `MERGE`, `SORT … USING/GIVING`, national and UTF-8 literals and functions, `SET` pointer |

**Sizes and start-up (Q6: measured, no budget set).** A one-form project (a button and a label): `program.wasm` 87 108 bytes, `program.single.wasm` 87 125 bytes, a 25-file bundle of 7 440 032 bytes (the player is most of it). Start-up as the Build reports it — `pr_init` + `pr_open_form(main)` on `wasmtime`, compilation included, in a **debug** build — 309 ms; a native proxy, not a browser figure. The player's generated fixture (one form, a handler that writes and displays): 96 499 / 96 516 bytes.

**Found on the way:** the interpreter stops on `COMPUTE … / 0 ON SIZE ERROR` (`WASM-PARITY.md` #18, a 1.x.x fix); the generator did not bind a LINKAGE `RETURNING` record (#13, fixed) and sent `DISPLAY` in fragments the player printed as separate lines (ABI A13, fixed); the bundler refused two variants for the size of memory 0, which differs by construction (fixed).

## NIST NC, IF and IC rerun on `wasmtime` (2026-10-22, version 2.0.35, WASM-010 T3)

The test host moved from `wasmi` 2.0 to `wasmtime` 44.0.3 (operator, 2026-10-22, WASM-010 Q2; `Config::wasm_multi_memory`, `cranelift_nan_canonicalization`). The batch is now decoded by `cobolt_wasm_abi::batch`, the codec the player uses. Same harness, same programs (`RCRUN=…/nist_wasm_run nist_conformance run NC|IF|IC`):

| Module | Compiled | Programs reporting 0 failures | Assertions PASS / FAIL / DELETED | Before (`wasmi`) |
|---|---|---|---|---|
| NC (Nucleus) | **95 of 95** | **95 of 95** | **4 612 / 0 / 5** | 95/95, 4 612 / 0 / 5 |
| IF (Intrinsic Function) | **45 of 45** | **45 of 45** | **841 / 0 / 0** | 45/45, 841 / 0 |
| IC (Inter-Program Communication) | **25 of 25** | **25 of 25** | **309 / 0 / 4** | 25/25, 309 / 0 / 4 |

No program was excluded, timed out or crashed: the engine change moved no number. Time per program, measured with a timing wrapper around `nist_wasm_run` (a throwaway script outside the repository), was at most **2.17 s** (NC105A, including the compile). The harness limit is 20 s. A full NC run takes 1 min 42 s, IF 40 s and IC 16 s. The root `[profile.dev.package.wasmi*]` entries are gone: `wasmi` is no longer in the root workspace's graph, so cargo warned that they matched no package. The player is a separate workspace with its own profiles, so they never applied to its `wasmi` tests.

## NIST NC on generated modules — finished (2026-10-20, version 2.0.19)

GOLDEN RULE #9, module one, both axes. The generator compiles all 95 NC programs and each runs to completion on the test host; the two numbers are kept apart on purpose:

| Axis | Generated module | Interpreter (`rcrun`) |
|---|---|---|
| Compile (`nist_wasm census NC`) | **95 of 95** | 95 of 95 |
| Execution, programs reporting 0 failures (`RCRUN=…/nist_wasm_run nist_conformance run NC`) | **95 of 95** | 95 of 95 |
| Assertions | **4 612 PASS / 0 FAIL / 5 DELETED** | 4 614 / 0 / 5 |

The 2-assertion difference is the harness' tally and not a test that did not run: it counts the marker `PASS ` across the whole print file, the interpreter writes 120-byte records with no separator (a marker can straddle two records) and the test host writes one record per line. Every program's own CCVS summary line (`N OF N TESTS WERE EXECUTED SUCCESSFULLY`) is identical in both engines on all 95 (checked program by program).

What the last stretch changed in the generator and the routines (each is a differential test in `crates/cobolt-wasm/tests/nist_nc_fixes.rs`): the comparison of a number with characters follows the standard pseudo-move (digits as written, no sign, `P` positions as zeros) and ZERO against an edited item is the character `0`; `BLANK WHEN ZERO` on numeric and edited items, and not on a `VALUE`; a `VALUE` ignores `JUSTIFIED`, reaches the children of a group and is stored as written in an edited item; a group sender into an alphanumeric-edited receiver is an alphanumeric move; de-editing with insertions; `INSPECT` on a signed numeric item sees its digits; groups holding an `OCCURS … DEPENDING ON` table are as long as the count says, and the receiver of a `MOVE` takes the maximum; 66-levels belong to their record (qualified by `OF`) and `RENAMES a` keeps `a`'s description; `PERFORM … VARYING … AFTER` follows 6.20.4 exactly for TEST BEFORE and TEST AFTER; stacked `WHEN` phrases share the statements of the next one; the remainder of `DIVIDE` uses the decimals of an edited `GIVING` item; external switches come from the host (`pr.switch_state`, conformance builds only). The test host runs `wasmi` optimised (`[profile.dev.package.wasmi*] opt-level = 1`): NC243A went from 14.5 s to 0.7 s, which the harness' 20 s limit needs (opt-level 3 overflows the stack in wasmi, so 1).

Divergences from the interpreter found on the way are in `WASM-PARITY.md`.

Next under GOLDEN RULE #9: **IF (Intrinsic Functions)**, then IC — IF needs `FUNCTION` calls in the generator (the module currently refuses them), IC needs nested programs and `CALL`/`LINKAGE`.

## The player, loader, host page and bundle — merged (2026-10-20, version 2.0.24)

The player agent's seven commits are on `features-2x` (cherry-picked in order). What stands, how it was measured and what is not done is in the CHANGELOG entry for 2.0.24; the points that bind the rest of the work:

- **ABI A5–A7 bind the generator**: one suspended task per form instance with tasks of different instances interleaving (`OpenFormSync`), `pr_cancel(-1)` discards a trapped run, `pr_close_form` returns a status, `pr_status` exists, `ERROR.code` is a magnitude, and §12.1/§12.2 give the verbs of `COBOL::`/`me::` and the argument text of each event. The form exports in `compile_project` are written against them.
- **Needs the operator's ruling**: Leptos is not used (WASM-016 R1/Q2). The deviation is contained behind the `Surface` trait; nothing else in the player depends on it.
- **Open items of the player** (to finish, none postponed): the plan and tasks files of WASM-016, 002, 004, 006, 007, 011; the threaded variant run in a browser (Rayon DataGrid sort); the shell route push re-checked in Chrome; the dead-code warnings; WASM-016 AC7 (a shipped player is fresh); the WASM-015 R20 browser matrix cells it could not measure.

## Where the work stopped (2026-10-20, 2.0.23; usage limit reached)

Next, in order: (1) finish `module.rs`'s form exports — instance table, `pr_open_form`/`pr_event`/`pr_resume`/`pr_cancel`/`pr_tick`/`pr_close_form`/`pr_status`, the `query` and `random` glue, manifest `forms`/`names`/`events`; (2) value methods on data items (`Trim`, `Upper`, `Lower`, `Replace`, `Len`, `Split`); (3) a form-driving test host and a button-click test; (4) phase C — `remote` glue, suspension, `CALL … WITH WEBSOCKET LINKAGE`, JSON mapping (golden vectors from the backend agent); (5) merge the backend agent (branch `worktree-agent-a24ab0673830285c1`: `CALL … WITH WEBSOCKET LINKAGE`, JSON mapping, `cobolt-server`) — still running; the player agent is merged (2.0.24); (6) end-to-end IDE Build → bundle → player; (7) NIST interpreter regression for the additive AST fields (`FileDescription::is_external`, `UseProcedure::is_global`); (8) security gates, KB tables, CLAUDE.md and memory refresh.

## NIST IC on generated modules — finished (2026-10-20, version 2.0.22)

| Axis | Generated module | Interpreter |
|---|---|---|
| Compile (callers with their `SUBRTN` callees as one source) | **25 of 25** | 25 of 25 |
| Execution, programs reporting 0 failures | **25 of 25** | 25 of 25 |
| Assertions | **309 PASS / 0 FAIL / 4 DELETED** | 309 / 0 / 4 |

Program units, `CALL`/`CANCEL`, LINKAGE, `GLOBAL`/`EXTERNAL`, `OPTIONAL` files and `USE` declaratives: see the 2.0.22 entry. Differential tests: `crates/cobolt-wasm/tests/calls.rs`. NC 95/95 (4 612) and IF 45/45 (841) re-run unchanged. **The three finished modules for the WebAssembly line are NC, IF and IC.** Not part of this line's gate (no files in a browser): SQ, IX, RL, ST, SM; SG is out of scope.

## NIST IF on generated modules — finished (2026-10-20, version 2.0.20)

| Axis | Generated module | Interpreter |
|---|---|---|
| Compile (`nist_wasm census IF`) | **45 of 45** | 45 of 45 |
| Execution, programs reporting 0 failures | **45 of 45** | 45 of 45 |
| Assertions | **841 PASS / 0 FAIL** | 841 / 0 |

Intrinsic functions are built as call records evaluated by `func.rs` (numeric → register stack, character → `T_CTEMP`); see the 2.0.20 entry. Differential tests: `crates/cobolt-wasm/tests/intrinsics.rs`; routine tests in `func.rs`. NC is unchanged at 95/95, 4 612/0 after the change. Next: **IC** (nested programs, `CALL`/`LINKAGE`).

The IDE-side sub-agent's five commits (WASM-001/008/015) are merged into `features-2x` (2.0.20) and the graphics agent's nine (`cobolt-graphics`, WASM-018 part 1) in 2.0.21; the backend and player/loader agents are still running in their own worktrees, unmerged.

## Status at the end of the first implementation session (2026-10-20, version 2.0.18)

Measured, not hoped. Commits on `features-2x`: `44371f53` … `42eac962` (2.0.11 – 2.0.17) plus this one.

| Area | State | Evidence |
|---|---|---|
| Planning | done | `WASM-PLAN.md`, `WASM-ABI-v1.md` (2.0.12) |
| `cobolt-decimal` (shared decimal kernel) | **V** | interpreter suite 1527 passed / 0 failed; `round`/`powi`/format proved equal to `std` |
| `cobolt-numedit` (shared edited-picture engine) | **V** | 199 233 random cases, 0 differences against the original; `cobolt-forms` numedit tests green |
| `cobolt-wasm-rt` (routines) | I | 15 native tests; codecs, MOVE, arithmetic, conditions, DISPLAY, edited pictures, INSPECT/STRING/UNSTRING, INITIALIZE, conformance files |
| `cobolt-wasm` (generator), WASM-010 phases A and B | I | programs match the interpreter byte for byte in both memory variants; modules validate; "hello" 15.8 KB |
| NIST **NC** on generated modules | I | **69 of 95 programs run clean; 81 of 95 compile; 3 811 PASS / 39 FAIL** (interpreter: 95/95, 4 614/0). Open: `ACCEPT`, `RENAMES`, user `CLASS`, `EVALUATE` over conditions, X-card/undeclared names, failures in NC104A/105A/107A/108M/114M/124A/203A/215A/216A/219A/222A/247A/254A |
| NIST IF, IC | not started (GOLDEN RULE #9: NC first) | |
| WASM-010 phase C (suspension, remote CALL), JSON mapping in the module | not started | |

Work running in other worktrees when the session's usage limit was reached (each commits on its own branch, **none merged into `features-2x`**): `cobolt-graphics` (WASM-018 part 1); backend + `CALL … WITH WEBSOCKET LINKAGE` + JSON mapping + `cobolt-server` + identity backend (WASM-005/003/014/009); player + loader + host page + bundle (WASM-016/002/004/006/007/011); IDE project kinds, selector, tree entries, preview, tooling (WASM-001/008/015). Their reports have not been read. Not started: Tauri shell (WASM-012), security gates (WASM-013), the Canvas control integration (WASM-018 part 2), CLAUDE.md/memory refresh.

Harness notes: `NIST_KEEP=1` keeps the work directories (they land under the *temp_dir*, which in this environment is a literal `${DaemonNameOrIdentifierHere}` TMPDIR — set `TMPDIR` explicitly before using it); drive the generator with `RCRUN=target/debug/examples/nist_wasm_run ./target/debug/examples/nist_conformance run NC`.

## WASM-017 baseline and matrix (T1, 2026-10-23)

Engines **released from 2024 onwards** (Q4), at the oldest such version. A cell says whether that oldest version has the feature;
where it does not, the first version that does and its date. Sources, each read 2026-10-23:

- **[WF]** [webassembly.org/features](https://webassembly.org/features/) data `features.json` (commit `3a8728d820`, 2026-10-21; read 2026-10-23) — every WebAssembly feature cell, including threads.
- **[CIU]** [caniuse.com/sharedarraybuffer](https://caniuse.com/sharedarraybuffer) (read 2026-10-23) — `SharedArrayBuffer` enabled (the cross-origin isolation it needs is the page's COOP/COEP, WASM-002).
- **Release dates:** Chrome — [chromiumdash milestone schedule](https://chromiumdash.appspot.com/schedule) (`fetch_milestone_schedule`:
  120 = 2023-12-05, 121 = 2024-01-23, 130 = 2024-10-15); Firefox — [product-details](https://product-details.mozilla.org/1.0/firefox_history_major_releases.json)
  (121 = 2023-12-19, 122 = 2024-01-23, 125.0.1 = 2024-04-16 — 125.0 was not released, 134 = 2025-01-07); Safari and iOS — [Apple security
  releases](https://support.apple.com/en-us/100100) (Safari 17.3 and iOS/iPadOS 17.3 = 22 Jan 2024; Safari 26.2 and iOS 26.2 = 12 Dec 2025).
- **ES-module source imports:** [WF] lists `esmIntegration` (phase 3) as supported by no browser; the syntax is TC39's
  [source phase imports](https://github.com/tc39/proposal-source-phase-imports), Stage 3; [MDN `import source`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import/source).

| Engine (oldest 2024+ release) | bulk memory | reference types | sign extension | non-trapping float→int | multi-value | **multi-memory** | threads + `SharedArrayBuffer` (isolated) | JS String Builtins | ES-module source imports |
|---|---|---|---|---|---|---|---|---|---|
| Chrome / Edge — Chromium 121 (2024-01-23) | yes (75) [WF] | yes (96) [WF] | yes (74) [WF] | yes (75) [WF] | yes (85) [WF] | **yes** (120, 2023-12-05) [WF] | yes (threads 74 [WF]; SAB 68 [CIU]) | no — from 130 (2024-10-15) [WF] | no [WF] |
| Firefox 122 (2024-01-23) | yes (79) [WF] | yes (79) [WF] | yes (62) [WF] | yes (64) [WF] | yes (78) [WF] | **no — from 125 (2024-04-16)** [WF] | yes (threads 79 [WF]; SAB 79 [CIU]) | no — from 134 (2025-01-07) [WF] | no [WF] |
| Safari (macOS) 17.3 (2024-01-22) | yes (15) [WF] | yes (15) [WF] | yes (14.1) [WF] | yes (15) [WF] | yes (13.1) [WF] | **no — in no version** [WF] | yes (threads 14.1 [WF]; SAB 15.2 [CIU]) | no — from 26.2 (2025-12-12) [WF] | no [WF] |
| iOS web view (WKWebView) — iOS 17.3 (2024-01-22) | yes (iOS Safari) [WF] | yes [WF] | yes (iOS 14.5) [WF] | yes [WF] | yes [WF] | **no — in no version** [WF] | iOS Safari: threads 14.5 [WF], SAB 15.2 [CIU]; an app's web view must still serve COOP/COEP — WASM-017 Q5 (open) | no — from 26.2 [WF] | no [WF] |
| Android WebView — Chromium 121 (2024-01-23) | yes (Chromium) [WF] | yes [WF] | yes [WF] | yes [WF] | yes [WF] | **yes** (Chromium 120) [WF] | threads: Chromium [WF]; [CIU] lists no WebView row, so isolation in a WebView is not established here | no — from Chromium 130 [WF] | no [WF] |

**Stop gate (T1): not triggered.** Neither Safari nor the iOS web view has multi-memory in any version, so R9's WebKit rule stands as
written: the single-memory variant is always built for iOS and for a matrix that includes Safari. T7 may start.

**Decided by the operator (2026-10-23): the Firefox minimum is 125.** Firefox 122–124 (2024 releases without multi-memory) are
outside the declared matrix; the single-memory fallback stays a WebKit rule only. The finding that led to it:
Firefox 122–124 are 2024 releases **without** multi-memory. With Q4's "2024 onwards",
a matrix that includes Firefox needs either the single-memory variant for Firefox too, or a Firefox minimum of **125**. The working
reading for T12/T13 is **Firefox 125** as the tested minimum for the multi-memory variant, with Firefox 122–124 served the single-memory
variant (the same fallback WebKit gets); to confirm.

**Minimum versions T12 and T13 test against:** Chromium 121 (Chrome, Edge, Android WebView), **Firefox 125**, Safari 17.3, iOS 17.3. Nothing in `scan::baseline_features()` is missing from any of them; JS String Builtins and
ES-module source imports are on none of the oldest versions, so the bundle uses neither.

## Revisit: Component Model and ES-module integration (WASM-017 T15; R6, Q7; written 2026-11-02, version 2.0.166)

**Decision in force (R6).** The Component Model is **not** used in v1: the operator kept it only if Chrome supported it, and that is not
established. The host API stays plain imports and exports (ABI v1 §3 and §4); the loader instantiates by `fetch` and streaming
instantiation. ES-module integration (importing a `.wasm` natively) is optional: probed, never required.

**Trigger to revisit (Q7).** Native support of the Component Model and of ES-module source imports in **Chrome, Firefox and Safari**, all
three. One engine is not enough: the player has to run on the whole matrix of "WASM-017 baseline and matrix" above.

**Owner (Q7).** The developer. The check is theirs to make against Chrome's own status page, which the session that wrote T1 could not
read; this record does not claim it was read.

**Date of the last check: 2026-10-23** (T1), from [webassembly.org/features](https://webassembly.org/features/) (`features.json`, commit
`3a8728d820`, 2026-10-21) and the vendors' release data. Nothing was re-read on 2026-11-02 when this record was written.

**What the probes found** (T1's table, "ES-module source imports" and "JS String Builtins" columns):

| Engine (oldest 2024+ release) | ES-module source imports | JS String Builtins |
|---|---|---|
| Chrome / Edge, Chromium 121 | no [WF] | no; from 130 (2024-10-15) |
| Firefox 122 | no [WF] | no; from 134 (2025-01-07) |
| Safari 17.3 and the iOS web view | no [WF] | no; from 26.2 (2025-12-12) |
| Android WebView, Chromium 121 | no [WF] | no; from Chromium 130 |

[WF] lists `esmIntegration` (phase 3) as supported by no browser. T8 put both probes into the generated loader (`WebAssembly.validate`
of a module importing `wasm:js-string`, and `WebAssembly.Module`'s prototype chain for source-phase imports) and reports them in
`choice.reasons`; **neither is used** (D2). The same T8 test proves no string conversion is on the call path.

**What would change when the trigger is met.** Only the loader's choice (`hostpage.rs`): use a native import where the probe says it
exists, keep `fetch` and streaming instantiation where it does not. The exports and imports the module has stay as they are.

## WASM-017 T12 — the player axis and the long loop (measured 2026-11-03, version 2.0.185)

Where each cell ran: node 22-class runtime on this Mac, the real loader and the real player, `Worker` over `worker_threads`. Command:
`cargo test -p cobolt-web-bundle --test e2e_node -- --nocapture` → `end-to-end scenarios: 28 passed, 0 failed`, three runs in a row (one
earlier run of the same code had 1 failure whose message was not captured; not explained).

| Cell (generated module) | Player | Module | Ran where | Rayon workers | 10 000 000-iteration loop | Longest main-thread gap |
|---|---|---|---|---|---|---|
| multi / main | plain | multi-memory | node, main thread | 0 | ~6 s, 1 499 yields | 5–11 ms |
| multi / Worker | plain | multi-memory | node, Worker | 0 | ~6 s, 0 (worker) | 1.7–2.1 ms |
| single / main | plain | single-memory | node, main thread | 0 | ~6 s, 1 499 yields | 5–9 ms |
| single / Worker | plain | single-memory | node, Worker | 0 | ~6 s, 0 (worker) | 1.7–3.2 ms |
| multi / main | threaded | multi-memory | node, main thread | 3 | ~6 s | 3–11 ms |
| multi / Worker | threaded | multi-memory | node, Worker | 3 | ~6 s | 1.3–3 ms |
| single / main | threaded | single-memory | node, main thread | 3 | ~6 s | 3–11 ms |
| single / Worker | threaded | single-memory | node, Worker | 3 | ~6 s | 1.3–3 ms |

Deadline asserted: 16 ms + `YIELD_BUDGET_MS` (6.9 ms) = 22.9 ms. Pool size read from the pool: 1, 2, 4, 9, 32 cores → 1, 1, 3, 8, 8 workers.
Isolated → threaded + Worker, not isolated → plain + main thread: the existing `choose` scenario. The fixture module stays at 3 000 000 iterations
(its spin is one instruction; its generator constant also feeds goldens), `maxGap < 50` kept.

**Rayon sort (added in 2.0.187, player rebuilt):** 50 000 rows through `sort_workers` — plain 1 thread; threaded under node, pool 3, 4 threads used in each of the four threaded cells; threaded in Chrome, pool 8, 9 threads used. Firefox, Safari, iOS and Android web views: not installed.

## WASM-017 T13 — embedding, isolation headers, cross-origin resources (measured 2026-11-03, version 2.0.186)

Engine that ran: Google Chrome (headless, the installed one), through `tests/support/cdp.rs`. Command:
`cargo test -p cobolt-web-bundle --test isolation_chrome -- --nocapture` → `test result: ok. 3 passed`.

| Cell | Result |
|---|---|
| Embedded in a host page without isolation headers (AC14) | `isolated=false, plain/main`; the host page's own text intact |
| Embedded in an isolated host page (AC14) | `isolated=true, threaded/worker` |
| Host without headers + service-worker shim (AC15) | isolated, `threaded/worker`, 65 ms from the start of the navigation (after two shim defects were fixed) |
| `_headers`, nginx, `.htaccess` × off / require-corp / credentialless | 9 of 9 carry exactly the headers of `security_headers` |
| Cross-origin image / tile, page `credentialless` | no header: loaded; CORP cross-origin: loaded; CORS: loaded; CORS mode without ACAO: refused (CORS message) |
| Cross-origin image / tile, page `require-corp` | no header: refused (`ERR_BLOCKED_BY_RESPONSE.NotSameOriginAfterDefaultedToSameOriginByCoep`); CORP cross-origin: loaded; CORS: loaded |
| Cross-origin font (CORS mode), both settings | with ACAO: loaded; without: refused ("blocked by CORS policy"); CORP alone: refused |

**Not run — commands for the engines not installed** (each cell is the same bundle served by the development server; none is claimed):
`cargo run -p cobolt-web-bundle --example serve -- <dist/web> --isolated` (isolated) and without `--isolated` (plain), then open
`http://127.0.0.1:8080/` in Firefox, in Safari (macOS), in Safari on an iOS device or simulator, and in an Android WebView/Chrome, and read
`PowerRustCOBOL.mount(document.getElementById('pr-app')).then(a => a.choice)` and `self.crossOriginIsolated` in the console. The Mobile kind's
plain-player rule was not exercised (no Mobile project builds here). The `focus_comparison_chrome` test of `cobolt-project-tools` fails in this
environment (the desktop side records no events); it touches none of this change and was not run against the base.

## WASM-017 T17 — finalization sweep (measured 2026-11-04, version 2.0.193)

Built with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` (the disk filled twice with full debug builds). Every `test result` line was read.

| Command | Result |
|---|---|
| `cargo test -p cobolt-forms -p cobolt-wasm -p cobolt-wasm-abi -p cobolt-compiler -p cobolt-project-tools -p cobolt-web-bundle --no-fail-fast` | 2 508 passed, 0 failed, 8 ignored |
| `cargo test -p cobolt-forms --features render --no-fail-fast` | 1 823 passed, 0 failed, 2 ignored |
| `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast` | 1 952 passed, 8 failed, 5 ignored — 3 known unrelated reds + 5 that need `target/debug/rcrun` (absent; not re-run) |
| `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --no-fail-fast` | first run: `the_shipped_player_was_built_from_the_source_that_is_here` FAILED (stale since 2.0.188); after `build_player`: all green |
| `cargo test -p cobolt-web-bundle --no-fail-fast` (after the rebuild) | all `ok` |
| `cargo test PLAYER --no-fail-fast` as written | not run (builds every workspace test binary; out of disk twice) |
| `cargo fmt --check` | not clean; diffs only in files this series never touched |
| `git diff --stat -- README.md docs/` | empty |

**Blocked or not measured, with the exact reason:** the five `rcrun`-dependent `cobolt-ide` tests (no `rcrun` binary in this worktree; building it costs disk); Firefox, Safari (macOS, iOS) and the Android WebView (not installed — commands under "WASM-017 T13"); plan §6 manual check (operator, through the MCP tools); `git push` of `2.x.x` (SSH to github.com:22 times out, HTTPS has no stored credential).

## WASM-013 — security architecture: what is owed, what is not measured, the fuzz budgets (T24, closed by T25; written 2026-11-06, version 2.0.277)

The model is `specs/WASM-013-security-architecture/threat-model.md`; its test is `cargo test -p cobolt-web-bundle --test threat_model -- --nocapture`. It prints **49 rows** over the seven boundaries (browser ⇄ backend 17, host page ⇄ module 7, module ⇄ player 6, IDE ⇄ preview web view 5, mobile shell ⇄ frontend 5, backend ⇄ identity provider 4, backend ⇄ data 5), 132 named tests that were each found in their crate, 42 STRIDE cells walked, 15 secret-scan patterns equal to `secrets::PATTERNS`, and **owed rows: 5**. The owed count below is that number; when a spec lands the test a row waits for, the row moves to a test name and the count falls.

| Row | Owed by | What is owed |
|---|---|---|
| TM-30 | WASM-016 | `build_player` writes the player's `components` (its lock closure) into `player-build.json`, and the bundle's SBOM lists them |
| TM-37 | WASM-012 | the capability file written by `mobile_build.rs`, and its test: main window only, no `remote` section, permissions equal to the union of the declared features |
| TM-38 | WASM-012 | Android's `network_security_config.xml` `pin-set` and iOS's `NSPinnedDomains` written from `[mobile.pinning]` (the table is already checked: T21) |
| TM-39 | WASM-012 | the mobile dependency guard and the shell's SBOM |
| TM-40 | WASM-012 | `store_required`: a signing or store secret refused rather than written to disk where the OS store is missing (the signing half of AC7) |

**Not owed any more** (named in the task and since answered by tests that exist): WASM-009's five rows — the refresh through `cors_headers` and the custom header the browser will not add cross-site (it is `X-PR-Identity`; `the_post_checks_refuse_what_a_cross_site_page_could_send`, `the_cors_grant_headers_are_written_only_inside_cors_headers`), the cookie-only refresh refused cross-site (`refresh_and_sign_out_go_through_the_cookie`), the full sign-in log test (`the_identity_routes_log_no_token_password_code_or_cookie`) and the identity secret fields (`a_known_secret_is_refused_by_file_and_setting_wherever_the_bundle_holds_it`); WASM-002's T7 wiring (T7 shipped on 2.0.262, the Build is linked); WASM-017's R15b rows (`every_exposed_type_is_listed_and_disposed_017`, `a_hundred_part_lifetimes_leave_no_object_and_no_growth_and_hostile_inputs_are_refused`). T9's Preview case passes when `target/debug/rcrun` is built and is red in a fresh worktree without it (the five `rcrun`-dependent `cobolt-ide` tests).

**Open and not a row** (each is reported, none is faked): (1) the advisory baseline in `deny.toml` — 19 distinct advisories left, each with a reason, to be retired by upgrades (quick-xml 0.41 needs entity events handled in every reader; lopdf 0.42 needs the PDF reading rewritten; calamine; wasmtime 44 is the operator's pin); (2) 10 secret shapes in 7 tracked example files that `examples_clean.rs` tolerates by name — **for the operator** (not opened here); (3) the shipped player under `assets/web-player/` was stale from 2.0.270 and is rebuilt in T25 (2.0.277; `freshness` green); it must be rebuilt again after any later change to the player's source. **T25's sweep** (2.0.277): `cobolt-web-bundle` 120, `cobolt-compiler` 256, `cobolt-project-tools` 248, `cobolt-wire` 50, `cobolt-server` 184, `cobolt-forms --features render` 1,837, `cobolt-runtime` 1,661, the player crate 465 passed with 0 failed; `cobolt-ide --bin` 1,967 passed with the 3 known unrelated reds; `csp_browser` and `supply_chain` ran under `PR_SECURITY_GATE=1`. The per-command table is in the Done note of T25 in `specs/WASM-013-security-architecture/tasks.md`.

**Cannot be measured here — the command, and nothing claimed:**

| Cell | Command | Why not here |
|---|---|---|
| The Content-Security-Policy and Trusted Types in Firefox and in Safari (Q2, best effort) | serve a built bundle: `cargo run -p cobolt-web-bundle --example serve -- <dist/web>`, open it in each browser, run `eval('1')` and `document.body.append(Object.assign(document.createElement('script'), {textContent: '1'}))` in the console and read the violation reports; Chrome's run is `PR_SECURITY_GATE=1 cargo test -p cobolt-web-bundle --test csp_browser -- --nocapture` | only Chrome is installed (same cell as WASM-017 T13) |
| Pinning on a real phone (TM-38) | build a Mobile app whose `[mobile.pinning]` pins a key the backend does not hold, run it on an Android and an iOS device, and read that the connection is refused | WASM-012's output files do not exist yet, and no device is attached |
| The capability file on a device (TM-37) | list the permissions the installed app holds and compare with the declared features | the same |
| The fuzz targets over TLS | the targets run on the loopback `ws://` with the real sealed channel; TLS is `tls_floor.rs` | a deliberate scope, stated in the threat model §9 |

**Fuzz budgets** (release; `PR_FUZZ_ITERS` multiplies the iteration counts, default 1; fixed seeds; every target prints its counts and seconds):

| Target | Default (1×) | Recorded run at `PR_FUZZ_ITERS=200` |
|---|---|---|
| `cobolt-wire::fuzz_frames` (both frame parsers) | 0.58 s | 99.5 s: 20,000,000 mutated frames per side |
| `cobolt-wire::random_round_trip` (JSON ⇄ group, `json::parse` on mutated bytes) | 1.97 s | 441.4 s: 17,665,227 mutated texts, 23,191,920 `json::parse` texts; the wire pair in all 541.07 s (9.0 min) |
| `cobolt-server::fuzz_dispatch` (frames, transport ends, floods) | 2.94 s | 164.1 s: 600,000 mutated frames, 1,190,712 server frames judged, 671,344 log records holding none of 6 planted strings |
| `cobolt-server::fuzz_tokens` (forged, mutated, wrongly-claimed tokens) | 1.44 s | 283.9 s: 1,200,000 tokens per configuration, 0 accepted that a key had not signed; the server pair in all 450.4 s of tests (7.6 min with the build) |

All four ended with 0 panics. In a debug build the same targets take 2.8 s, 4.0 s, 6.2 s and 22.0 s at 1× (the signature checks are slow unoptimised): the budgets are stated for `--release`, as the tasks' commands do.

**The release review (R16)** is machine-checked: when this ledger carries a line that begins `RELEASE CANDIDATE:` followed by `reached`, `threat_model.rs` requires `specs/WASM-013-security-architecture/release-review.md` with `kind`, `reviewer`, `author`, `scope` and `date`, and a reviewer other than the author. No such line exists, so the test prints "not a release candidate".

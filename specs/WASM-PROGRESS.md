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
| WASM-010 | WebAssembly code generation | ✔ | | | | |
| WASM-011 | Navigation, SPA and standalone styles | ✔ | | | | |
| WASM-012 | Mobile shell (Tauri 2) | ✔ | | | | |
| WASM-013 | Security architecture | ✔ | | | | |
| WASM-014 | Backend contract and evolution | ✔ | | | | |
| WASM-015 | Testing, agent tooling, knowledge | ✔ | | | | |
| WASM-016 | Prebuilt player | ✔ | | | | |
| WASM-017 | Module architecture, memory, threads, boundary | ✔ | | | | |
| WASM-018 | Canvas 2D/3D and graphics objects | ✔ | | | | |

## Done elsewhere

- `CALL … RETURNING` (WASM-005 Q2, D6): fix `8a15f00d` (1.91.219) on `main`, forward-port `afe22c51` on 2.x.x — tests 14/14 runtime, NIST baselines held.

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

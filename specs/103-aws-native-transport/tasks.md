<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — AWS without `uvx`

- **Status:** in progress
- **Plan:** ./plan.md   **Date:** 2026-10-16

One commit per phase on `features-aws-native`. Each commit bumps `z` (start at 1.91.183, re-check the highest number in any worktree
first) and adds a `CHANGELOG.md` entry. Before every commit: scan the staged diff for `AKIA`/`ASIA` + 16 characters,
`aws_secret_access_key`, `SessionToken`. Push only outside Mon–Fri 09:00–18:00 São Paulo.

## P0 — Spike, baseline, spec, plan, tasks
- [x] **T1 — Baseline** (AC11) — run the plan §6 commands in the cloned `target/`; record test counts and the expected-red list in `baseline.md` here.
- [x] **T2 — Lock the pinned set** (A1, R10, AC8) — add the `aws` dependencies in `cobolt-runtime/Cargo.toml`, resolve with `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`; `git diff Cargo.lock` is additions only; no package of R10 reachable. **Stop and ask if no 1.92-compatible set locks.**
- [x] **T3 — Compile check** — `ConfigLoader::http_client`, `credentials-login`, `aws_sigv4::http_request::sign` and the `HttpClient` trait exist on the pinned set (a throwaway `cargo check`).
- [x] **T4 — Spec amendments, plan, tasks** — spec A1–A3 (done), `plan.md`, `tasks.md`.

## P1 — Dependencies and guard (R10, R12, R17, R18)
- [x] **T5 — Manifest** — `aws` feature with the dependencies and `http`; dev-dependencies `hmac`, `sha2`; comment fixed.
- [x] **T6 — Guard** — `dependency_guards_063.rs`: allow-list + reachability + AC34 list; demonstrate failure on a planted `aws-sdk-s3`.
- [x] **T7 — Docs of the dependencies** — `DEPENDENCIES-en.md` AWS section, `THIRD_PARTY_NOTICES.md`; doc comments in `runtime_features.rs`, `aws/mod.rs`.

## P2 — Transport core with keys only (R1–R5, R13, R14, R16; AC1–AC5, AC9, AC10)
- [ ] **T8 — Route table** — `ServerDef` fields, `validate`, `launch_for`, `[servers.hosted]`; tests incl. the banned-strings list.
- [ ] **T9 — HTTP helper and launch type** — `http_runtime.rs` byte-body function; `HttpsEndpoint`, `Launch.https`, `Credentials::with_keys`.
- [ ] **T10 — Fake** — `fake_mcp` engine extracted; `tests/support/http_fake.rs` (JSON and SSE, session id, 404 once, status failures, request log).
- [ ] **T11 — Pool link and `https.rs`** — `Link`, shared handshake, signed POST, `_meta`, re-initialise, plain errors, `DELETE`.
- [ ] **T12 — Tests** — `tests/aws_https.rs`: AC1 (independent signature), AC2, AC3, AC4, AC5 (hash of 33 tables + snippets), AC9 (33 ok / 14 refused with `uvx` off `PATH`), AC10 refusal part (zero requests).

## P3 — Credential chain (R6–R8; AC6, AC7)
- [ ] **T13 — `UreqClient` and runtime** — `HttpClient` over the byte-body helper; the process-wide credentials runtime.
- [ ] **T14 — Provider chain and precedence** — keys set by the program first, then the `SdkConfig` provider; refresh at signing time.
- [ ] **T15 — Errors and the CLI question** — plain messages naming the profile and the fix (R7); decide Q1 (keep or drop `export-credentials`); decide Q2 (write-back).
- [ ] **T16 — Tests** — fixtures for static keys, `credential_process`, SSO, role assumption, `aws login`; secret scan over the native path (AC7).

## P4 — Test connection and IDE (R15; AC10)
- [ ] **T17 — `diagnose` and `https::probe`** — per-server reports; new variants.
- [ ] **T18 — IDE** — `settings_form.rs`, `i18n.rs` ×6, completeness test, Report × language loop.
- [ ] **T19 — Tests** — `aws_diagnose.rs` hosted cases: unreachable, no credentials, refused, all good.

## P5 — Docs, KB, help (R20; AC13)
- [ ] **T20 — Guide, matrix, README, steering registry** — hosted vs Labs; no "verified" before AC12.
- [ ] **T21 — KB constants and property help** — `lib.rs` 8299/8301/8311/7096; 16 `StartTimeoutMs` entries × 6 languages; comment cleanups.
- [ ] **T22 — Regenerate** — `cargo run -p cobolt-ide --example build_chunked_kb` (once, last); the freshness test green.

## P6 — Parity, regression, release (R19; AC11, AC12)
- [ ] **T23 — Hosts** — the hosted suite over HTTP under `rcrun run-form`, an embedded child form and the compiled binary.
- [ ] **T24 — Sweeps** — every baseline command again; `--no-fail-fast`; every `test result:` line read; a built app without AWS controls links none of the new packages.
- [ ] **T25 — Release** — CHANGELOG, version; the operator runs **AC12** (live, no `uvx`, keys from the environment); merge to `main` only on request and only after AC12.

## Done criteria
All acceptance criteria in spec.md are checked (AC12 by the operator), tests pass, docs updated, and the change is committed as feature
commits on `features-aws-native`. Nothing is merged to `main` or pushed unless the operator asks and the push window allows.

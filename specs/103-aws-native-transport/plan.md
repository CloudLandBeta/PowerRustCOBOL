<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — AWS without `uvx`: a native transport for the hosted AWS MCP Server

- **Status:** approved 2026-10-16 (operator, in the planning session)
- **Spec:** ./spec.md   **Date:** 2026-10-16
- **Branch:** `features-aws-native` (worktree `.claude/worktrees/aws-native`); `features` is held by another session.

## 1. Approach

**Seam: a `Link` inside the pool; the pool's public API does not change** (R1, R2, R19). `pool.rs`'s `Handle` keeps
`launch`, `mode` and `session`; its child-specific fields (`child`, `writer`, `pending`, `alive`, `stderr`) move into
`Link::Stdio { .. }`, and `Link::Https(https::Link)` is added. `call_tool`, `list_tools`, `stop`, `stop_connection`,
`shutdown`, `stderr_of` and `running` keep their signatures, so `ops.rs`, `interpreter/aws.rs`, every shutdown caller
(`cobolt_runtime::shutdown_child_processes`, reached by rcrun, `run-form`, the headless script runner and the generated
`main`) and every existing test are untouched. The handshake is factored out of `Handle::start` so both links share it.

Streamable HTTP has **no reader thread**: `Https::round_trip` makes one signed POST on the caller's thread and reads the
answer from the response body (`application/json`, or `text/event-stream` with each event routed through
`Session::route`). `cobolt-mcp` is not touched (R5a).

**Route table (R1, R13, R14).** `ServerDef` gains `transport` (`stdio` default | `https`), `url`, `service`,
`signing_region` and `metadata`; `command` becomes `#[serde(default)]`; `Default` is derived; `ServerDef::validate()` is
called from `Routes::parse` (stdio needs `command`; https needs `url`, `service`, `signing_region`, takes no
`readonly_args`/`write_args`, and requires `https://` — or `http://` **only to a loopback host**, which is what lets the
test fake exist and stops a route override from downgrading a real connection). `launch_for` keeps its signature (used by
`aws_live.rs` and `aws_delivery_b.rs`) and, for https, expands `url` and `metadata` with the same placeholder context into
`Launch.https: Option<HttpsEndpoint>`. Only the `[servers.hosted]` block of `routes.toml` changes:

```toml
[servers.hosted]
transport      = "https"
url            = "https://aws-mcp.us-east-1.api.aws/mcp"
service        = "aws-mcp"
signing_region = "us-east-1"
metadata       = { AWS_REGION = "{Connection.Region|or:us-east-1}" }
```

The signing region is route data; no code parses the host name.

**`aws/https.rs` (new).** Signed POST with `aws-sigv4` (`Accept-Encoding: identity`, no redirects), `Mcp-Session-Id` and
`MCP-Protocol-Version` after `initialize`, `_meta` injected into each outgoing message's `params` **before signing** (R4),
one serialised re-initialise on "session unknown" (R5), 401/403/expired-token answers turned into the plain message that
begins `The AWS profile "…"` (so `ops::plain_error` and `diagnose.rs:101` keep working), a timeout as
`CallError::Timeout`, and `stop` as a signed `DELETE` (at most 2 s inside `shutdown`; on a detached thread for `stop`,
`stop_connection` and a handle replacement, so `AWS-SET-CREDENTIALS` and the slot lock never wait on the network).
`https::probe(launch)` returns typed step results for Test connection. One `ureq::Agent` per handle.

**`http_runtime.rs`** gains one `#[cfg(feature = "http")] pub(crate)` function: send a byte body with headers and return
`(status, headers, Box<dyn Read>)`. Today there is none (`post_streaming_while` takes a `&str`, drops the headers and
returns text); `tls_connector()` stays private.

**Credentials (R6–R8).** In this order:
1. keys set by `AWS-SET-CREDENTIALS`, read through a new `Credentials::with_keys(|KeyRef| …)` closure (the second and last
   reader besides `built_env`; `KeyRef` has no `Debug`, `Clone` or `Copy`; `aws_credential_types::Credentials` is never
   formatted — its `Debug` prints the key id);
2. `aws_config::defaults(..).http_client(UreqClient).profile_name(..).load()` → the `SdkConfig`'s credentials provider
   (environment, shared files, `credential_process`, SSO, `aws login`, role assumption, instance role);
3. `aws configure export-credentials` — kept only if the spike finds a profile type the native chain cannot read (Q1).

`UreqClient` implements `aws_smithy_runtime_api::client::http::HttpClient` over the `http_runtime` helper, using
`spawn_blocking`. One lazily created process-wide tokio runtime serves credential resolution and nothing else (Q6). The
credential is re-resolved at the moment of signing (R3). Token write-back (Q2) follows the SDK's own behaviour if it writes
the CLI's cache in the CLI's format; otherwise the runtime reads only and says "sign in again".

**Test connection (R15).** `diagnose()` returns one `Report` per server (`Hosted`, `Lambda`). New variants:
`EndpointUnreachable { host, detail }`, `CredentialsNotFound { profile }`, `SessionRefused { status }`; the hosted
`AllGood` carries the tool count. `ProfileNotSignedIn` stays for an expired sign-in. `aws_report_text` is an exhaustive
match, so the IDE does not compile until every variant is handled.

**Dependencies (R9–R12, R17, R18).** `aws = ["dep:toml", "http", "dep:aws-config", "dep:aws-sigv4",
"dep:aws-credential-types", "dep:aws-smithy-runtime-api", "dep:aws-smithy-types", "dep:tokio"]`, the 1.8.x line (A1),
`default-features = false`; the lock is made additively with `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`. The
063 guard becomes an allow-list plus a reachability assertion.

## 2. Affected crates / files

- `crates/cobolt-runtime/Cargo.toml` — the `aws` feature, the pinned optional dependencies, dev-dependencies `hmac`, `sha2`, the comment at lines 79–88.
- `Cargo.lock` — additions only (about 46 package names plus `time 0.3`).
- `crates/cobolt-runtime/src/aws/routes.rs`, `routes.toml` — `ServerDef`, `validate`, `launch_for`; the `[servers.hosted]` block and its comments; the banned-strings test.
- `crates/cobolt-runtime/src/aws/process.rs` — `HttpsEndpoint`, `Launch.https`, `Credentials::with_keys`, doc comments.
- `crates/cobolt-runtime/src/aws/pool.rs` — `Link`, the shared handshake.
- `crates/cobolt-runtime/src/aws/https.rs` — new.
- `crates/cobolt-runtime/src/aws/diagnose.rs`, `mod.rs`, `lifetime.rs`, `ops.rs` (markers only) — Test connection and doc comments.
- `crates/cobolt-runtime/src/http_runtime.rs` — the byte-body helper.
- `crates/cobolt-runtime/src/bin/fake_mcp.rs` → `fake_mcp/{main,engine}.rs`; `tests/support/http_fake.rs`; `tests/aws_https.rs`; the three override helpers (`aws_delivery_c.rs`, `aws_hosts.rs`, `powercloud_hosts.rs`); `aws_diagnose.rs`.
- `crates/cobolt-compiler/tests/dependency_guards_063.rs`; `crates/cobolt-compiler/src/runtime_features.rs` (docs only); `crates/cobolt-compiler/src/lib.rs` (KB text at 8299, 8301, 8311, 7096).
- `crates/cobolt-ide/src/panels/settings_form.rs`, `i18n.rs` (six tables), `prop_help_data.rs` (16 `StartTimeoutMs` entries × 6 languages).
- `docs/developers-guide-en.md`, `docs/cobol-support-matrix-en.md`, `docs/DEPENDENCIES-en.md`, `THIRD_PARTY_NOTICES.md`, `examples/PowerCloud/README.md`, `specs/steering/docs.md`.
- `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`, `assets/knowledge/chunked.data` (regenerated once, last).

## 3. Data / model changes

No `.cfrm`, `.cidx` or `cobolt.toml` change. The route table gains five optional server fields (an old override that sets
only `command`/`args` still parses). No COBOL surface changes (a program written yesterday behaves the same).

## 4. Key decisions & alternatives

- **Replace the proxy, not the templates (D1).** The Python runs in AWS's sandbox; only the signer-forwarder is local.
  Rejected: per-service direct calls (several times the work, and no template reuse).
- **Official crates, MSRV pinned (D2, A1).** Rejected: hand-written signing (would have kept the 063 guard intact, but the
  operator chose the SDK crates); raising the workspace MSRV to 1.94.1 (touches README, BUILDING, Guide, AGENTS, tech.md and
  every user's toolchain check, and needs the operator's explicit say-so).
- **Pool `Link`, not a second pool.** One place decides restart, timeout and shutdown for both transports.
- **No reader thread for HTTP.** A reply arrives in the POST that asked for it.
- **Signature verification lives in the tests,** with an independent `hmac` + `sha2` implementation reading the fake's request
  log (a bin cannot use dev-dependencies, and a verifier built from the code under test proves nothing).
- **Loopback `http://` only for a loopback host** (`validate`), so a route override cannot send signed traffic in clear text.
- **One shared tokio runtime for credentials only.** Rejected: a runtime per call (thread churn) and an async pool.

## 5. Risks & mitigations

- *Pinned set does not lock under 1.92* → the first task proves it before any code; if not, stop and ask for the MSRV decision.
- *Signing name, JSON vs SSE, session id, `_meta` on `initialize`, the "unknown session" status are unverified* → encoded
  from documentation only in the fake; AC12 (live, operator) gates the merge.
- *`CallError` is stringly typed and `stderr_of` is `None` after a failed https start* → https errors are self-contained,
  first-line plain, and `diagnose` uses `https::probe`, not the pool.
- *Blocking POST cannot be aborted by `stop`* → the late result is dropped by generation, as today.
- *Early `process::exit` paths skip the `DELETE`* → the session expires server-side; documented.
- *ureq 2: HTTP/1.1 only, no proxy-from-env, gzip unless `identity`, timeout mapping* → `identity`, no redirects, a small
  spike for the timeout kind; corporate proxies stay unsupported, as everywhere else in the runtime.
- *Linux: `aws` now implies `http`, hence the OpenSSL development package for console builds* → form applications already link it.
- *Built applications with an AWS control need the new crates in the cargo cache* → the offline build retries online; the
  compiled-binary parity test covers it.
- *Merge conflicts with the `features`/`fixes` worktrees* (CHANGELOG, version, `i18n.rs`, `chunked.data`) → regenerate the
  binary after merging, never merge it; take the next free version number at each commit.
- *Disk: the volume was at 99 % when the plan started (6.3 GiB free)* → one APFS-cloned target directory; one test binary at
  a time; check `df` before blaming a compile error.

## 6. Test strategy

Baseline first (T1), then after every phase. Commands:
`cargo test -p cobolt-runtime --lib aws::`; `cargo test -p cobolt-runtime --test aws_controls --test aws_credentials --test aws_delivery_b --test aws_delivery_c --test aws_diagnose --test aws_fake_server --test aws_no_orphans --test aws_pool --test aws_process_env --test aws_readonly --test aws_routes`;
`cargo test -p cobolt-cli --test aws_hosts --test powercloud_hosts`; `cargo test -p cobolt-compiler --test dependency_guards_063`;
`cargo test -p cobolt-forms --test aws_controls_078`; `cargo test -p cobolt-compiler runtime_features`; `cargo test -p cobolt-ide --bin cobolt-ide`.

New, each with a quantified summary block (GOLDEN RULE #7): `tests/aws_https.rs` (operations exercised, requests made, ms per
request; 33 ok / 14 refused), the independent SigV4 recomputation over 100 % of logged requests, the byte-hash comparison of the 33
hosted tables and `[snippets]` against `main` 2e57112f, the credential-surface scan over the native path (zero hits), and the guard
demonstrations (planted `aws-sdk-s3`, one mutated template byte, one altered body byte). Live: AC12 by the operator.

## 7. Steering compliance

- [x] i18n: new Test connection strings in all six language tables, with a completeness test counting `{0}`.
- [x] Generated-code banner / regenerate contract: untouched.
- [x] English dev guide updated; it has no translations in the tree (A3).
- [x] Feature → `features-aws-native`, `z` bump per commit from 1.91.183 (re-check the highest number in any worktree), CHANGELOG entry each.
- [x] No "cobolt" in user-facing text; COBOL identifiers English.
- [x] Credentials never in git: the key file stays in `~/Documents` and is never opened by an agent; the staged diff is scanned before every commit.
- [x] System KB tables edited, `build_chunked_kb` run once, last.

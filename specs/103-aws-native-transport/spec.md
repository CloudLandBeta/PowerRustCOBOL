<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — AWS without `uvx`: a native transport for the hosted AWS MCP Server

- **Status:** approved for planning (operator decisions D1–D4 recorded 2026-10-16; amended
  2026-10-16 after the plan's research: A1 MSRV pin, A2 TLS wording, A3 translations — see §6)
- **Folder:** specs/103-aws-native-transport/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-16
- **Amends:** spec 078 (AWS controls through MCP — folder removed in 1.91.155; read it with
  `git show ccfa3449~1:specs/078-aws-mcp-controls/spec.md`), spec 092 (PowerCloud), spec 063
  R18 / AC8 (the dependency guard). §6 lists every sentence this spec changes.

> Operator, 2026-10-16, on seeing *"Not connected: AWS access needs the program "uvx", which is
> not installed on this machine."*: asked why, whether there is a Rust alternative, whether `uvx`
> could be reimplemented in Rust, then **"definitivamente não gosto da abordagem"** (of
> reimplementing `uvx`), and chose **"Build a Pure Rust-Native MCP Server"**.

## Decisions taken by the operator (2026-10-16)

| | Decision | Consequence |
|---|---|---|
| **D1** | **Native proxy, templates unchanged.** Rust replaces `uvx mcp-proxy-for-aws` (a SigV4 signer that forwards to AWS's hosted MCP endpoint). The Python templates in `routes.toml` keep running in AWS's sandbox. | 33 of the route table's 49 operations (`AwsMcp.AwsCall` among them) stop needing `uvx`. The other 16 are 14 on the five AWS Labs servers, which still need it (non-goal here), and 2 (`AwsMcp.Call`, `ListTools`) that name their server at run time. |
| **D2** | **Official AWS crates** (`aws-sigv4`, `aws-credential-types`, `aws-config` with default features off), **not** hand-written signing. | The 063 AC8 guard ("no package named `aws-*`") is **changed on purpose**, into an allow-list (R17). A TLS backend that is not `aws-lc-rs` is mandatory (R10). |
| **D3** | **Native credential chain, with the AWS CLI as a fallback.** | See R6–R8. Whether the fallback is still reachable once the native chain is in is Q1. |
| **D4** | **Replace `uvx` outright for the hosted server.** | No fallback to the old route in the table; the way back is `git revert`, not a setting. Mitigated by the live check (AC12) before this lands on `main`. |

## 1. Overview

An AWS control that is served by the hosted AWS MCP Server (DynamoDB, S3, S3 Vectors,
Rekognition, Polly, Comprehend, Textract, EC2, Cognito, and `AwsMcp.AwsCall`: 33 operations) starts a child
process, `uvx mcp-proxy-for-aws@1.7.0`, which is Python. On a machine without `uv` the control
fails with *"AWS access needs the program "uvx""*, and an end user of a built application cannot
be expected to install it.

The proxy does very little: it takes MCP JSON-RPC from stdin, **signs each request with SigV4**
and POSTs it to `https://aws-mcp.<region>.api.aws/mcp`, returns the answer on stdout. The real
work — the `aws___run_script` tool running a boto3 script from the route table — happens in
**AWS's sandbox**. So the proxy can be replaced by Rust code inside the application without
touching a single template: a streamable-HTTP MCP transport that signs with SigV4, using the
credential chain the AWS CLI already set up.

### What was checked while writing this spec (2026-10-16)

- **The Rust-MCP-server sample the operator was pointed at** — `aws-samples/sample-building-mcp-servers-with-rust`
  (MIT-0) — is a tutorial: four servers (calculator, RDS, S3, PostgreSQL) on `rmcp`, `aws-sdk-s3`,
  `aws-sdk-rds`, `aws-config`. It provides none of the operations this product routes (no DynamoDB,
  Rekognition, Polly, Comprehend, Textract, EC2, Cognito, Lambda, Bedrock) and it would bring the
  SDK's default TLS stack. It is **not used**; `cobolt-mcp` already is a Rust MCP client and server.
- **The hosted endpoint** is generally available (since 2026-05-06) in eight regions and accepts
  two kinds of sign-in: OAuth 2.1 (a browser; no proxy, no CLI) and SigV4 through the proxy
  (AWS documentation, *Setting up the AWS MCP Server*). This spec does the second one natively.
  OAuth stays a non-goal (§2).
- **What the proxy does**, from its public README and source (read as a summary, not line by
  line): service name for signing is inferred from the host `aws-mcp.<region>.api.aws` as
  `aws-mcp`; the upstream client is a streamable-HTTP client; credentials come from the boto3
  chain and are **re-read on every request**; `--metadata K=V` is injected into the `_meta` of
  every request. **None of these was exercised against the live endpoint** — AC12 is where that
  happens, and it needs an AWS account.
- **Crate probe** (a scratch project resolved against crates.io on 2026-10-16, `cargo tree`,
  on the **newest** releases — see the MSRV paragraph below for the pinned line):
  - `aws-sigv4` 1.6.0 (`sign-http`) + `aws-credential-types` 1.3.0 → 62 packages, **none** of
    `ring`, `aws-lc-rs`, `aws-lc-sys`, `rustls`, `hyper`, `cc`, `cmake`. Its `hmac` 0.13 and
    `sha2` 0.11 are the versions already in `Cargo.lock`.
  - `aws-config` 1.12.0 with `default-features = false` and `rt-tokio`, `credentials-process`,
    `sso`, `credentials-login` adds `aws-sdk-sso`, `aws-sdk-ssooidc`, `aws-sdk-sts`,
    `aws-sdk-signin`, `p256`, `rand`, `uuid`, `zeroize` — again none of the C-backed packages.
    `credentials-login` is the `aws login` provider.
  - Switched off, `aws-config` has **no HTTP client**. `aws-smithy-http-client` offers only
    rustls variants, no `native-tls`. The SDK crates that fetch credentials (SSO, STS, Signin)
    therefore need an `HttpClient` that this product supplies, over the `ureq` + `native-tls`
    stack its `http` feature already has (R11).
- **MSRV (found while planning, amendment A1).** Every `aws-*` release since 2026-07 declares
  `rust-version = 1.94.1`; this workspace is **1.92** and `AGENTS.md` forbids a newer floor unless
  the operator asks. The **1.8.x line of `aws-config`** (1.8.18, 2026-06-02) declares **1.91.1**,
  has `credentials-login`, and pairs with `aws-sigv4` 1.4.5, `aws-runtime` 1.7.5,
  `aws-smithy-runtime` 1.11.3 and `aws-sdk-signin` 1.14.0 (all 1.91.1; crates.io metadata). The
  feature pins that line. Its dependencies are caret ranges, so the lock is made with
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`. The probe above is repeated on the pinned
  line as the plan's first task, and **raising the workspace MSRV is not part of this spec**: if
  no compatible set can be locked, work stops and asks.
- **What the lock file already holds (amendment A2).** At `main` 2e57112f `Cargo.lock` lists
  `ring`, `rustls`, `hyper-rustls`, `hyper` and `cc` (a weak `reqwest` feature edge); none of the
  rustls/ring packages is compiled, `hyper` is (via `google_maps`). Only `aws-lc-*` and `cmake`
  are absent. So the rule this spec can keep is about what the **`aws` feature reaches**, not
  about the lock file as a whole (R10, AC8).

## 2. Goals / Non-goals

**Goals**

- Every operation routed to `[servers.hosted]` (33, `AwsMcp.AwsCall` included) works on a machine
  with no `uv`, no Python and no `uvx`.
- The request leaves the application signed with the developer's or end user's existing AWS
  sign-in (profile, SSO, `aws login`) or with keys the program set at run time.
- No change to any template, extractor, event, property or COBOL surface: a program written
  yesterday behaves the same today.
- Offline, deterministic tests: a loopback fake of the hosted endpoint, no AWS account. One
  opt-in live check for the things only AWS can answer.

**Non-goals**

- Reimplementing `uvx`, or running Python from Rust.
- Replacing the **five AWS Labs servers** (`lambda`, `bedrock-kb`, `agentcore`, `s3tables`,
  `dataprocessing`; 14 operations). They stay on `uvx`. Moving them (to the hosted server's
  `run_script`, or to direct service calls) is a separate spec.
- Direct, per-service AWS calls that bypass the hosted MCP Server (DynamoDB JSON 1.0, S3 REST/XML,
  EC2 query protocol, …).
- OAuth 2.1 sign-in to the hosted server (the AWS documentation's "Option A").
- Linking any AWS **service** client (`aws-sdk-s3`, `aws-sdk-dynamodb`, …). Only the four
  credential-fetching clients in R17 are allowed.
- A TLS stack other than the operating system's (`native-tls`). The `aws` feature reaches none of
  `rustls`, `aws-lc-rs`, `ring`, `hyper`, `hyper-rustls` (R10).
- Storing, displaying or logging AWS keys (078 non-goal, unchanged).
- Changing the 078 safety model: `AllowWrite` and the read-only rules are untouched.

## 3. User stories

- As an **end user of a built application** with a DynamoDB or S3 control, I want it to work on a
  machine that has the AWS sign-in but not Python or `uv`, so that I install nothing for it.
- As a **COBOL developer**, I want my existing forms, templates and COBOL to keep working
  unchanged, so that dropping `uvx` costs me nothing.
- As a **developer who signs in with `aws login`**, I want the application to use that session
  and to leave it valid for the CLI afterwards, so that the two never fight over a token.
- As a **developer testing offline**, I want the AWS test suites to keep running without a
  network or an account, so that CI stays deterministic.
- As the **operator**, I want the dependency guard to say exactly which `aws-*` packages may
  exist and fail on any other, so that "the SDK got in" cannot happen by accident.

## 4. Requirements (EARS)

### 4.1 The transport

- **R1 (ubiquitous):** A server in the route table shall be able to declare an **HTTPS
  transport** (an endpoint URL and the SigV4 service name) instead of a command. The pool shall
  serve such a server without starting a child process. `[servers.hosted]` shall use it; no
  `.rs` file shall name the endpoint or the service (078 R15).
- **R2 (ubiquitous):** The HTTPS transport shall speak MCP **streamable HTTP**: each JSON-RPC
  message a client sends is one `POST`; the answer is read whether the server returns it as
  `application/json` or as a `text/event-stream`; the session id the server assigns on
  `initialize` is sent back on every later request; the negotiated protocol version is sent as
  the transport requires; the session is ended with a `DELETE` when the pool stops it.
- **R3 (ubiquitous):** Each request shall be signed with **SigV4** — service name and region as
  the route declares, the body hash covering the exact bytes sent — with credentials resolved
  **at the moment of signing**, so a token refreshed since the last request is the one used.
- **R4 (ubiquitous):** Every request shall carry the metadata the route declares in the
  request's `_meta` (today: `AWS_REGION` = the connection's region, default `us-east-1`),
  as the proxy's `--metadata` does.
- **R5 (event):** When the server answers that the session is unknown (the session expired
  between two calls), the transport shall initialise a new session once and repeat the call;
  a second failure is an error. When the endpoint cannot be reached, answers `401`/`403`, or
  does not answer within the time allowed, the outcome shall be the existing classes —
  `CallError::Start`, `Failed`, `Timeout` — with the plain-language message rules of 078
  R22/R23 (the missing program is gone from this path; the profile and its fix are named).
- **R5a (constraint):** `cobolt-mcp` shall stay free of HTTP, TLS and AWS code (078 R2, 065):
  the transport is host code in `cobolt-runtime`'s `aws` module and reuses `cobolt-mcp`'s
  `Session` for the protocol, as the stdio path does.

### 4.2 Credentials

- **R6 (ubiquitous):** The credentials a request is signed with shall come, in this order, from:
  1. the **keys the program set** for the connection with `COBOL::"AWS-SET-CREDENTIALS"`
     (spec 092 A6) — now used to sign directly, never placed in an environment, an argument or a
     file;
  2. the **AWS credential chain for the connection's profile** (or the default chain when none
     is named): environment variables, shared credentials and config files (static keys),
     `credential_process`, IAM Identity Center (SSO), `aws login` sessions, and role
     assumption (`role_arn` with `source_profile`);
  3. **the AWS CLI** — `aws configure export-credentials --profile <p> --format process` — when
     the chain cannot produce credentials for a profile and the CLI is installed (D3).
- **R7 (event):** When no source yields credentials, the control shall raise `onError` with a
  `LastError` that names the profile and the fix in plain words (*"sign in with `aws login`"*,
  *"run `aws sso login --profile <p>`"*), never the raw SDK text (078 R23).
- **R8 (constraint):** Keys and tokens shall exist only in memory: never in a `.cfrm`, a project
  file, a log, `Verbose` output, an error message, a `Debug` rendering, a property, a command
  line or the environment of any process the runtime starts **on this path** (078 R11 and A6,
  restated for it; the Labs servers keep A6 exactly as it is). Whatever the credentials library holds shall be zeroised when released, as
  `process.rs` does for its environment today.
- **R8a (constraint):** When the runtime refreshes an SSO or `aws login` session, it shall
  persist the refreshed token exactly where, and in the format that, the AWS CLI itself uses —
  or not refresh at all and say so (Q2). It shall never leave the CLI's session invalid.

### 4.3 HTTP and TLS

- **R9 (constraint):** All traffic of this feature — the MCP requests and the credential
  providers' own calls (SSO, STS, Signin) — shall go through **one** HTTP stack: `ureq` with
  `native-tls`, the stack the `http` feature already links.
- **R10 (constraint):** The dependency closure of the `aws` feature of `cobolt-runtime` shall not
  reach `aws-lc-rs`, `aws-lc-sys`, `ring`, `rustls`, `rustls-webpki`, `tokio-rustls`, `hyper`,
  `hyper-rustls`, `h2`, `cc`, `cmake` or `aws-smithy-http-client` (the C-toolchain rule of 078 and
  063, and the one HTTP stack of R9). `aws-lc-rs`, `aws-lc-sys` and `cmake` stay absent from the
  whole `Cargo.lock`, as today (amendment A2).
- **R11 (ubiquitous):** The product shall supply the AWS SDK crates with an `HttpClient`
  implemented over that stack, so the SDK crates are built with their default HTTPS client
  switched off.
- **R12 (ubiquitous):** The `aws` feature of `cobolt-runtime` shall depend on `http`. A built
  application with no AWS control shall link none of this (078 R29).

### 4.4 The route table and the other servers

- **R13 (constraint):** The templates of the 33 hosted operations (`input`, `result`, `rows`,
  `fail`, `mutating`, `event`) and the `[snippets]` shall be **byte-identical** to the table before
  this change. Only the `[servers.hosted]` block changes.
- **R14 (ubiquitous):** `routes.toml` shall no longer contain `mcp-proxy-for-aws` or any `uvx`
  launch for `[servers.hosted]` (D4). The five Labs servers keep their `uvx` launches and the
  *"needs the program uvx"* message (078 R22) for them only.
- **R15 (ubiquitous):** **Test connection** (078 R24) shall test the hosted server too — endpoint
  reachable, credentials found, session opened, tools listed — and report which of those failed,
  in the IDE language. Today it tests only the Lambda server.
- **R16 (state):** While `AllowWrite` is off, the refusals of 078 R25/R26 shall happen before a
  request is built, so no signed request leaves the machine for a refused operation.

### 4.5 The dependency guard

- **R17 (constraint):** The guard in `dependency_guards_063.rs` shall change from "no package
  named `aws-*`" to an **allow-list**: only `aws-config`, `aws-credential-types`, `aws-runtime`,
  `aws-sigv4`, `aws-smithy-*`, `aws-types`, `aws-sdk-sso`, `aws-sdk-ssooidc`, `aws-sdk-sts`,
  `aws-sdk-signin` may appear. Any other `aws-*` package — above all a **service** client such as
  `aws-sdk-s3` — fails the test, as do the C-backed packages of R10 and the existing 063 AC34
  list (`bzip2`, `zstd`, …). The allow-list lives in the test, with a comment naming this spec.
- **R18 (constraint):** The packages of R17 shall be reachable from `cobolt-runtime` only through
  the `aws` feature, and from the delivered application only when the project has an AWS control.

### 4.6 One behaviour on every host, and documentation

- **R19 (ubiquitous):** Run Form, embedded child forms and the compiled binary shall use the same
  transport and the same credential chain (078 R28; the `interpreter-binary-parity` rule). The
  compiler's feature detection shall add the new dependencies exactly when it adds the `aws`
  feature today.
- **R20 (ubiquitous):** The Developer's Guide AWS chapter, the support matrix, the PowerCloud
  README, the System KB text that says what an AWS control needs, `DEPENDENCIES-en.md` and
  `THIRD_PARTY_NOTICES.md` shall say the new truth: no `uv` for the hosted services, `uv` still
  for Lambda, Knowledge Bases, AgentCore, S3 Tables and Glue; the added crates and their licences.

## 5. Acceptance criteria

All of AC1–AC11 run offline against a **loopback fake of the hosted endpoint** (a local HTTP
server inside the test, which also verifies the signature) and report a quantified summary block
(GOLDEN RULE #7): operations exercised, requests made, milliseconds per request.

- [ ] **AC1 (R3):** For each request the fake receives, an *independent* SigV4 computation in the
  test (its own `hmac` + `sha2` code, not the crate under test) reproduces the `Authorization`
  signature from the received method, path, headers and body. 100 % of requests in the run match;
  a request with a byte of the body altered after signing fails to verify (demonstrated).
- [ ] **AC2 (R2):** The handshake — `initialize`, session id, `notifications/initialized`,
  `tools/list` across two pages, `tools/call` — passes against the fake in **both** answer forms
  (`application/json` and `text/event-stream`), and the pool ends the session with a `DELETE`.
- [ ] **AC3 (R4):** `params._meta.AWS_REGION` equals the connection's region on every request of
  a run with two connections in two regions; the default is `us-east-1`.
- [ ] **AC4 (R5):** Fake answers *session unknown* → exactly one re-initialise and a repeated
  call; twice → `onError`. Fake never answers → `onTimeout` within `TimeoutMs` + 1 s. Unreachable
  endpoint, `401` and `403` → the messages of R5/R7, not raw text.
- [ ] **AC5 (R13):** A test hashes the 33 `[ops."…"]` tables (with their `.input` sub-tables)
  routed to `hosted`, and the `[snippets]` table, against hashes recorded from `main` at
  2e57112f: all 34 hashes equal. Mutating one template byte fails it (demonstrated).
- [ ] **AC6 (R6, R7):** Precedence — keys set by the program beat a profile; a profile beats the
  default chain. Each provider is exercised through the fake HTTP stack and fixture files: static
  keys, `credential_process` (a fixture script), SSO (a cached token plus a fake SSO endpoint),
  role assumption (a fake STS), `aws login` (a cached session plus a fake Signin endpoint), and
  the CLI fallback (a fake `aws` on `PATH`). Expired and missing credentials give the R7 message
  naming the profile. A credential that expires between two calls is replaced before the second.
- [ ] **AC7 (R8, R8a):** A test scans every argument, environment, log line, `Verbose` output,
  `Debug` rendering and error message produced in AC1–AC6 for a planted access key id, secret and
  session token: **zero** hits. After a refresh, the CLI's cache file parses with the CLI's
  expectations (fixture comparison) and the old token is not left as the only one.
- [ ] **AC8 (R9–R12):** `cargo tree -p cobolt-runtime --no-default-features --features aws -e normal`
  shows none of the packages of R10; the lock-file walk from the `aws` roots (the existing
  `closure()` helper) reaches none of them either; the guard of R17 is green and is shown to
  **fail** when `aws-sdk-s3` is added to a copy of the lock (demonstrated); a project with no AWS
  control builds a binary whose tree contains none of the R17 packages; `git diff Cargo.lock` of
  the change is **additions only** (no existing package moves version).
- [ ] **AC9 (headline, R1, R13, R14):** With `uvx` absent from `PATH`, every one of the 33 hosted
  operations (`AwsMcp.AwsCall` among them) completes against the fake, each raising its completion
  event then `onComplete`; the 14 Labs-server operations still answer with the *"needs the program
  uvx"* message. Counts reported: 33 succeeded / 14 correctly refused.
- [ ] **AC10 (R15, R16):** Test connection reports each of: endpoint unreachable, credentials not
  found, session refused, all good — for the hosted server. With `AllowWrite` off, every mutating
  hosted operation is refused and the fake records **zero** requests.
- [ ] **AC11 (R19):** The AC9 programs pass under `rcrun run-form`, as an embedded child form and
  as a compiled binary. The pre-existing AWS suites (`aws_*.rs`, `aws_hosts.rs`,
  `powercloud_hosts.rs`) pass unchanged; their counts before and after are recorded in the plan.
- [ ] **AC12 (live, opt-in, operator):** With an AWS account, `COBOLT_AWS_LIVE=1` and **no `uvx`**
  on `PATH`, one read-only operation per hosted service answers through the native transport.
  The run records what only AWS can say: the signing name the endpoint accepts, whether answers
  arrive as JSON or SSE, whether `_meta.AWS_REGION` is honoured, and the fixtures re-recorded
  from it. **This criterion gates the merge to `main`** (D4).
- [ ] **AC13 (R20):** Guide chapter, support matrix, README and KB text updated (English only — the
  Guide has no translations in the tree to delete, amendment A3, and the red
  `every_document_ships_in_every_language` is not made worse);
  `prebuilt_chunked_kb_matches_the_published_documentation` green after `build_chunked_kb`;
  `DEPENDENCIES-en.md` and `THIRD_PARTY_NOTICES.md` list the new crates with their licences.

## 6. Constraints & steering check

- **Classification:** a **feature** → a `features` branch. `features` is held by another
  session's uncommitted work, so this runs on `features-aws-native`, a worktree off `main`
  (`.claude/worktrees/aws-native`). `z` bump per change; CHANGELOG entry per change.
- **Sentences of earlier specs this one changes** (so no reader finds a contradiction):
  - 078 §1 reasons 1 and 2, and the "Non-goals: linking the AWS SDK for Rust, or any HTTP/TLS
    client, for AWS": **partly reversed** — the SDK's credential crates and an HTTP client are
    now linked for the hosted route; a service client and a second TLS stack are still not.
  - 078 §1 "The application talks to them over **stdio**": true now only for the five Labs servers.
  - 078 Non-goal "Streamable-HTTP transport … directly" and **Q3**: now done, with SigV4 (not
    OAuth), on the `native-tls`-only design Q3 asked for.
  - 078 R9, R10 and R7/R8 (child processes, stderr ring, restart on next call): unchanged for the
    Labs servers; **not applicable** to the HTTPS transport, which has no process.
  - 078 R22: now says "needs `uvx`" only for a Labs server.
  - 078 AC2 (`cargo tree` shows no TLS/HTTP crate for the AWS feature): **replaced by AC8**;
    `cobolt-mcp` itself still shows none (R5a).
  - 092 non-goal "Linking the AWS SDK, or any TLS stack other than the runtime's existing
    `native-tls`": the SDK half is reversed (R17); the TLS half stands (R9, R10).
  - 063 R18 / AC8 ("no AWS SDK package may exist in the build graph"): replaced by R17.
- **i18n (×6):** the IDE's Test connection texts (R15) are `Tr` fields in all six languages.
  Runtime `LastError` text follows how 078's does today (English literals produced by the runtime,
  reported by the control). COBOL identifiers stay English.
- **Generated code / regenerate contract:** none; no COBOL surface changes.
- **Docs:** R20. English only; the five Guide translations deleted. `cargo run -p cobolt-ide
  --example build_chunked_kb` in the same change if any KB table changes.
- **System KB:** the AWS section of `crates/cobolt-compiler/src/lib.rs` (the text at ~line 8299
  describing what an AWS control needs) is a KB source: edit the constant, then regenerate.
- **Secrets:** `CREDENTIAL_PROPS` unchanged — no credential property is added (R8). **Operator
  rule, 2026-10-16: credentials never go to git, in any file.** The live check (AC12) reads its
  keys from the environment only; the operator's key file lives outside the repository and is
  never opened by an agent; no fixture holds a real key (AWS's documented example ids, labelled as
  fakes, are the only ones). Before any commit, the staged diff is scanned for key patterns.
- **Dependencies:** this adds about **46 package names** to `Cargo.lock` (21 `aws-*`, plus `p256`
  and the RustCrypto curve crates, `base64-simd`, `time 0.3`, …); a lean built application gains
  `tokio`, `http`, `bytes`, `rand` and `zeroize` as well. All were seen resolving without a C
  build; the plan re-verifies on the pinned line (A1) and CI covers the platforms. **MSRV stays
  1.92** (A1).
- **Amendments written after approval:** **A1** the MSRV pin (§1); **A2** R10/AC8/the TLS
  non-goal speak of what the `aws` feature reaches, because the lock file already lists packages
  that are never compiled; **A3** AC13's translation clause (nothing to delete).
- **Windows:** no window is opened by this feature.
- **Process:** `/specify` → `/plan` → `/tasks` → implement. The operator likes quantified tests.

## 7. Open questions

- **Q1 — Is the CLI fallback (R6.3) still reachable?** `aws-config` carries `aws login` and role
  assumption natively (`credentials-login`, `sso`, STS). If the plan's spike shows every profile
  type the CLI understands is also handled natively, R6.3 is dead code. *Suggested:* keep it only
  if the spike finds a profile type the native chain cannot read; otherwise drop R6.3 and say so in
  the plan. (The operator chose "native chain + CLI fallback"; this asks only whether the
  fallback has anything to do.)
- **Q2 — Token refresh write-back (R8a).** `aws login` and SSO refresh tokens rotate; a refresh
  that is not written back can log the CLI out. *Suggested:* follow the SDK's own behaviour if it
  writes the CLI's cache in the CLI's format (verify in the plan); if it does not, read-only and
  say "sign in again" on expiry.
- **Q3 — Which endpoint region?** Today the route pins `aws-mcp.us-east-1.api.aws` and sends the
  connection's region as metadata. Eight regions have endpoints. *Suggested:* keep today's
  behaviour (this spec changes the transport, not the routing); a per-connection endpoint is a
  small later change in the route table.
- **Q4 — The unverified protocol details** (R2, R3, R4: signing name `aws-mcp`, SSE or JSON,
  `_meta` handling, protocol-version header). *Resolved by AC12*, which needs the operator's AWS
  account; until then the fake encodes the documented behaviour and nothing more.
- **Q5 — The Labs servers.** 14 operations stay on `uvx`. *Suggested:* a follow-up spec asking
  whether to route them through the hosted `run_script` (boto3 in AWS's sandbox, so no new
  code, subject to what the sandbox allows) or to call the services directly.
- **Q6 — Async SDK, sync runtime. Resolved by the plan:** signing is synchronous
  (`aws_sigv4::http_request::sign`); async is needed only to resolve credentials, so one lazily
  created process-wide tokio runtime serves that and nothing else, entered with `block_on` from
  the worker thread.
- **Q7 — MSRV.** Settled by A1 (pin the 1.8.x line; no workspace MSRV change). Reopens only if
  the spike cannot lock a 1.92-compatible set, in which case it is the operator's call.

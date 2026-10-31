<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — The Backend project and the secure frontend–backend channel

- **Spec:** [spec.md](spec.md). **Status:** revised **2026-10-31** against the spec as it stands after the operator's ruling of 2026-10-24
  (WASM-INDEX D21: R24–R26, AC11–AC12) and the decision of 2026-10-30 on R24 (both sides generate a key pair at start; X25519 + HKDF-SHA256 +
  ChaCha20-Poly1305). Everything the 2026-10-22 revision planned is **implemented** (tasks T1–T14, 2.0.39; evidence in tasks.md *Evidence
  (re-run, 2026-10-23)*). This revision designs the two new parts — the retry policy and blocked state (§2.8, task T15a) and the encrypted
  channel (§2.7, task T15b) — and brings the rest of the plan in line with what was built. **§2.7 and §2.8 are not implemented.**
- **Written:** 2026-10-20; revised 2026-10-22 and 2026-10-31 (this `/plan` run). Decisions the operator made are marked *operator*; the rest
  are this plan's, each with what it chose, why and what the alternative costs — for the operator to overturn.
- The contract half (WASM-014) and the identity half (WASM-009 backend) have their own plans. The modal that R26 shows is WASM-016 R20 (web)
  and WASM-012 R15 (device); the members of the handshake are WASM-014 R18; the threat model is WASM-013 R17.

## 0. What changed, and why

### 0.1 On 2026-10-31 (this revision)

| # | Spec change or finding | Was (2026-10-22 plan / code today) | Now (this plan) |
|---|---|---|---|
| C7 | **R24 + Decided (operator, 2026-10-30):** the channel is encrypted on top of TLS; **both sides** generate a fresh key pair at start (the backend when its process starts, each frontend when the application opens); a per-connection session key is agreed from the two; X25519, HKDF-SHA256, ChaCha20-Poly1305 from the RustCrypto crates | Plain JSON text frames under TLS (`crates/cobolt-wire/src/proto.rs`); the server refuses binary frames (`crates/cobolt-server/src/serve.rs` `session`, the `Message::Binary` arm, "binary frames are not part of the protocol") | A sealed channel: after the two `hello` messages every data frame in both directions is one ChaCha20-Poly1305 binary frame under per-direction keys (§2.7, D7–D10) |
| C8 | **R25:** a broken connection is retried with windows of **10, 20, 30 s**, at most 3; then every waiting call fails *transport* and the application is told; no fourth attempt on its own | Two different back-offs. Player: 250 ms doubling to 10 s with ±25 % jitter, only while a call waits (`crates/cobolt-web-player/src/net.rs` `Config::backoff_start_ms`/`backoff_max_ms`, the computation after `on_close`'s failure loop, `retry_at`). Rust client: 100 ms back-off within `ClientConfig::reconnect_deadline` = 20 s (`crates/cobolt-server/src/client.rs` `on_disconnect`), and four quick attempts for a first connection (`ensure`) | One policy value in `cobolt-wire` used by both clients; the two back-offs and the jitter go (§2.8, D11–D13) |
| C9 | **R26:** while it retries, the application is blocked and the "connecting" modal shows | Nothing: the player has no blocked state | The connection state machine raises and clears a **blocked** state; the modal and the event are WASM-016 T29b / WASM-012 T25b, which read it (§2.8) |
| C10 | **R8** now points at R25 for the client's reconnection | The client reconnected by its own back-off | Server keep-alive unchanged (pings every 20 s, 60 s idle cut); the client's reconnection is R25's |
| C11 | **AC11, AC12** added | — | Tests in §6 |
| C12 | **T15 split (operator, 2026-10-30)** into T15a (R25–R26; WASM-005 T18b waits only for it) and T15b (R24; waits for WASM-013 T24b and WASM-014 T19b) | One task | §2.8 is independent of §2.7 and goes first; nothing in §2.8 depends on the encryption |
| C13 | **ABI A15 (WASM-014 T6, 2026-10-26):** the player now speaks `proto.rs` (`hello`, `auth`, `call` with `key`, the `t`-tagged envelope) | Plan §5 R1: the player spoke A8 (`type`, no `hello`) and the two could not talk | **Risk R1 is closed.** The sealed channel of §2.7 is therefore one change made in three speakers of one protocol (§2.7) |
| C14 | Built since 2026-10-22 and now the design (tasks.md T5, T7, T10, T10a) | The plan predicted them | §2.2 (Leptos route table and server function, T10a), §2.6 (serialised by the ordering ticket; no `session: "new"`, T5), §2.1 (the IDE runs a child `rcrun backend run`, operator 2026-10-23, T10) |

### 0.2 On 2026-10-22 (kept for the record; all implemented, T1–T14, 2.0.39)

C1 the HTTP front on Leptos (R2) → T6, T7, T10a; C2 `[backend] state` (Q2) → T3, T5; C3 a closed HTTPS route table (Q3) → T7; C4 Q4–Q7
binding, `unsafe_plain_ws` removed → T4; C5 `backend/exports.toml` the one list → T2, T3, T9; C6 the envelope mismatch → closed by C13.

## 1. Decisions on the open questions

All seven questions of the spec are decided by the operator, and so is the construction of R24. What the plan does with each:

| Q | Operator's decision | Design |
|---|---|---|
| **Q1** | `backend/` inside the Web or Mobile project | §2.1; one list of exports, `backend/exports.toml` |
| **Q2** | A project property, default fresh `WORKING-STORAGE` per call | `[backend] state = "per-call" \| "per-connection"` (§2.6), built |
| **Q3** | Yes, ordinary HTTPS endpoints | The closed route table on the Leptos router (§2.2), built |
| **Q4** | No program is safe to repeat by default | `idempotent = false` unless an export says `true` (`backend.rs` `ExportDecl`); R25's reconnection repeats only those (§2.8) |
| **Q5** | Development token in a development run only | Built: `unsafe_dev_token_secret`, a release start refuses every `unsafe_*` (`config.rs`) |
| **Q6** | One binary per host, spec 041 | Built: `backend_build.rs` builds for `host_triple` only |
| **Q7** | Both: TLS in the binary, and behind a reverse proxy | Built: `native-tls` acceptor; plain `ws` on loopback only, in every mode (T4). With R24 the frames are also unreadable to that proxy (§2.7) |
| **R24** | Both sides generate a key pair at start; ephemeral–ephemeral agreement per connection; X25519, HKDF-SHA256, ChaCha20-Poly1305 (`x25519-dalek`, `hkdf`, `sha2`, `chacha20poly1305`) | §2.7 |

Decisions of 2026-10-20 that still hold: **TLS is `native-tls`** (`ring`, `aws-lc-rs` barred by the dependency guards); **JWT is `jsonwebtoken`
11 with `rust_crypto`**, used only to verify and decode, every policy check ours; macOS PEM/EC (`Identity::from_pkcs8` takes RSA only; EC via
`tls.pkcs12`); **a closed set of failure classes with stable codes** (`crates/cobolt-wire/src/error.rs`); **an unknown program answers "no
token" to an unauthenticated caller**; **strict configuration** (`deny_unknown_fields`); **secrets print as nothing** (`Secret`'s `Debug`).

## 2. Design

### 2.1 The project and the build (R1–R4, R20, R22) — built
- **Project shape** (`crates/cobolt-server/src/backend.rs`): `backend/exports.toml`, `backend/src/*.cbl`, `backend/COPYBOOKS/`, `backend/backend.toml`,
  `backend/contract/interface.json` (written by Build), `backend/contract/released/<version>.json`.
- **Exports** (R1): `exports.toml` is the whole of what the channel can run (`ExportDecl`: `program`, `roles`, `public`, `idempotent`,
  `independent`, `long_running`, `stream`, `directions`, `deprecated`). The IDE's Backend settings edit it (`crates/cobolt-ide/src/panels/web_settings.rs`
  `write_exports`); an old `[backend] exports` list is copied once into it on open (`migrate_exports`), `cobolt.toml` untouched.
- **Build** (`crates/cobolt-compiler/src/backend_build.rs`): read and check → `interface.json` (R4) → compare with the last released contract →
  a generated Cargo project (`cobolt-server[server]`, the bundle by `include_bytes!`, `main` = `cobolt_server::standalone::main`) → build →
  `bin/<project>-backend`. The generated project names no Leptos crate (R2 "invisible"); the bundle carries `state`. The compiler links only the
  loader (`cobolt-server` feature `loader`).
- **CLI** (`crates/cobolt-cli/src/main.rs` `cmd_backend`): `rcrun backend run|describe|check|release|build [dir]`.
- **IDE** (R20, *operator* 2026-10-23): Run → Start/Stop Backend runs `rcrun backend run <project>` as a child (`crates/cobolt-ide/src/backend_run.rs`
  `BackendRun`), its log live in the Output panel; the IDE links no server crate (guard `the_ide_runs_the_backend_as_a_child_and_links_none_of_the_server`).
- **Scaffold** (R22): `crates/cobolt-project-tools/src/create.rs` writes `backend/src/health.cbl`, `backend/exports.toml` (`HEALTH`, public) and a
  loopback `backend/backend.toml`. The WASM-009 authentication programs join the scaffold when WASM-009 M5 provides them.
- **R24 adds nothing here:** no key is generated at Build, none is written into the bundle, the generated project or `interface.json` (R24
  "never written into the build or the bundle").

### 2.2 The channel and the HTTPS surface (R2, R5–R8, R14, R16, Q3) — built
**Connection order:** accept → connection limit → TLS (`native-tls`) → hyper `http1` with `header_read_timeout` = handshake timeout and
`max_buf_size` = the head cap → route → for the upgrade: Origin check → upgrade with frame and message caps → `hello` within the handshake
timeout. **New with §2.7:** → key agreement → sealed frames.
**Call order (unchanged):** connection rate → (non-public) valid token → program lookup → user rate → role → concurrency permits → ordering
ticket → `spawn_blocking` run under a deadline → projection to the caller's contract.

- **Leptos (R2; operator 2026-10-23, "use Leptos de fato"):** `crates/cobolt-server/src/front.rs` — the platform routes are a `leptos_router`
  table (`PlatformRoutes`: `/`, `/ws`, `/dl/:token`) read by `leptos_axum::generate_route_list` and mounted in `serve.rs` `front_router`
  with `LeptosRoutes::leptos_routes_with_handler`; `GET /health` is a Leptos server function (`front::health`). Plain `cargo build`, no
  `cargo-leptos` (§7 O1).
- **Route table (Q3), closed:** `/health`, `/dl/{token}`, `/` and `/ws` with an upgrade, the WASM-009 identity routes when they exist; everything
  else 404 (`loopback::routes_are_closed`). Security headers as a response layer.
- **Keep-alive, Origin, limits:** WebSocket pings every 20 s, a 60 s idle cut, exact Origin match, limits 1 MiB / 30 s / 20 per s burst 40 /
  50 per s burst 100 / 8 / 64 / 1000, codes 7000–7004. Unchanged by this revision except that the frame cap is read as a plaintext cap (§2.7).

### 2.3 Tokens (R9–R13) — built, unchanged
`jwt.rs`: shape and 16 KiB cap → allowed algorithm (`RS256`/`ES256`; never `none`; HMAC only for the development token) → key by `kid` →
issuer, `exp`, `nbf`, leeway ≤ 300 s → audience → access token only. **R11:** the token travels as the message `{"t":"auth","token":…}` after
`hello`, never in a URL, query, header or sub-protocol; **with §2.7 that message is the first sealed frame**, so the token is never readable at a
TLS endpoint either. A call may carry a fresher token (R13, `authentication 2002`). JWKS TTL 3600 s, refresh ≥ 30 s apart. Claims through
`COBOL::"WS-CLAIM"` (R12), re-installed at every call; in `per-connection` mode a token naming another subject is refused `authorisation 3000`.

### 2.4 Hardening (R15, R17, R18, R21) — built, unchanged
Strict payload reader (`Interpreter::group_from_json`) before the program starts; faults answered `server 9001` without detail; `LogRecord` has
no field for a token, a password, a payload or a claim, and gains the session mode only. **New with §2.7:** no key material — private key,
shared secret, session keys, nonces — is ever logged, put in `/health`, or written to disk.

### 2.5 Concurrency across connections (R19) — built, unchanged
`Interpreter::set_file_lock_wait` (`crates/cobolt-runtime/src/interpreter.rs`): a `<path>.lck` advisory lock per `OPEN`, waited for up to the configured time,
else status 93; released after `CLOSE` has flushed. In `per-connection` mode a file left open holds its lock across that connection's calls; a
dropped connection releases everything (`a_file_held_open_across_calls_makes_others_wait_or_93`).

### 2.6 Statefulness as a project property (R18, Q2) — built
`per-call` (default): `exec::execute`, a fresh unit per call. `per-connection`: `exec::Session`, one run unit per exported program the connection
called, one `ExternalStore` shared through `Interpreter::new_with_channels_and_store`; the first subject is bound (`Session::admit`). **As built (T5),
and now the design:** a connection's calls run in turn through the **ordering ticket**, not a per-connection permit of 1 (a permit taken with
`try_acquire` would *refuse* a pipelined call, against R6); `independent` is ignored in this mode and the Build notes it. A call cut off, cancelled
or faulted drops that program's run unit. A reconnection starts fresh state whatever the client sends; **no `hello` member says "new"**
(WASM-014's `hello.session` is a client id for idempotency, the other direction; §7 O4).

### 2.7 The sealed channel (R24; AC11) — new, task T15b

**What the operator decided** (2026-10-30, binding): both sides generate a fresh X25519 key pair at start and never store it; each connection
agrees a session key from the two; HKDF-SHA256 and ChaCha20-Poly1305; RustCrypto crates, pure Rust, also for WebAssembly. **What this plan
adds:** where each key lives, the handshake members (a proposal to WASM-014, which owns them), the frame format, and the refusals.

- **Where the code lives (D7).** A module `seal` in `cobolt-wire`, behind a new feature **`channel`** (off by default): the key pair from 32
  random bytes, the agreement, the key derivation, `seal(&mut self, text) -> Vec<u8>` and `open(&mut self, bytes) -> Result<String, CallError>`.
  It takes its randomness from the caller, so it builds unchanged for `wasm32` and native and is tested natively. Four speakers turn it on and
  call it: the server (`cobolt-server` feature `server`), the Rust client (`client`), the player (`crates/cobolt-web-player`, its own workspace
  and lock), and the mock backend the browser tests use (`crates/cobolt-web-bundle/examples/mock_backend.rs`, `tests/e2e_chrome.rs`). The
  test helpers of `crates/cobolt-server/tests/support/mod.rs` and the minimal client of `crates/cobolt-compiler/tests/backend_build_wasm003.rs` go
  through the same module. The compiler, the semantic analyser and the interpreter, which link `cobolt-wire` without the feature, gain nothing.
- **Key pairs (R24 "generated when the application starts").**
  - *Backend:* one `x25519_dalek::StaticSecret` (feature `static_secrets` — the same secret serves every connection of the process, which
    `EphemeralSecret` cannot) made in `serve.rs` when the server starts (`Shared`), from `getrandom` (already a dependency of the `server` and
    `client` features, 0.4). One per process: `rcrun backend run`, the IDE's child and the built binary all make theirs at start.
  - *Player:* made when the application opens (where `api.rs` seeds the session, `client.seed_session(…)` with `Math.random`), but from
    **`crypto.getRandomValues`** — `Math.random` is not a cryptographic source — through `web-sys`/`js-sys`, handed to `net.rs` as 32 bytes;
    a reload is a new application start and a new key pair.
  - *Rust client* (desktop and native hosts of `CALL … WITH WEBSOCKET LINKAGE`): made when the `WsClient` is created (`WsClient::new`,
    reached from `install_default_from_env` at host start); its clones share it.
  - Secrets are zeroised on drop (`zeroize`, feature of `x25519-dalek`; `zeroize` 1.8.2 is in `Cargo.lock`), never serialised, never `Debug`.
- **Handshake (proposal to WASM-014 R18; D8).** The client's `hello` gains `key` (its X25519 public key) and `nonce` (32 fresh random bytes
  for this connection), both base64; the server's `hello` gains its `key` and its own fresh `nonce`. Both sides compute
  `shared = X25519(own secret, peer key)`; an all-zero `shared` (a low-order peer key) refuses the connection. Then
  `HKDF-SHA256(salt = SHA-256(transcript), ikm = shared)` expands two 32-byte keys, `info = "prc-channel/1 c2s"` and `"prc-channel/1 s2c"`.
  The **transcript** is the protocol number, both public keys and both nonces in a fixed order; binding it means a frame of one connection
  cannot be opened in another, and the per-connection nonces are what makes the session key new on every connection although each side keeps
  its key pair for its whole run (R24 "per-connection session key"). No secret is sent: a party that only reads frames, even holding the
  TLS session, sees two public keys and two nonces.
- **Frames.** After the two `hello`s, every data message in either direction — `auth` first, then calls, replies, errors, progress,
  subscriptions, uploads — is **one binary WebSocket message**: `ChaCha20-Poly1305(key of that direction, nonce = 4 zero bytes ‖ 64-bit
  big-endian counter, plaintext = the JSON text as today)`. The counter starts at 0 per direction per connection and is implicit (not sent);
  WebSocket over TCP is ordered, so a frame that does not open — tampered, replayed, reordered, dropped — **ends the connection** with
  *transport*, and the calls follow R7. A text message after the handshake is refused the same way (no plaintext downgrade). WebSocket
  ping/pong control frames stay as they are (empty, no content). Everything inside a sealed frame is the protocol of `proto.rs`
  unchanged, so every rule above — call order, tokens, limits, idempotency — is untouched.
- **Limits.** `max_frame_bytes` stays the plaintext limit announced in `ServerHello`; the socket's caps are raised by the 16-byte tag, so a
  developer's `max_payload_bytes` (WASM-005 R11) means what it meant.
- **Refusals (WASM-014 R18 "refused with the class *transport*").** A protocol-2 `hello` without a usable `key`/`nonce`, or an exchange
  that fails → error *transport* and close. A protocol-1 client (no exchange possible) → the existing version refusal at `hello`
  (`PROTOCOL_UNSUPPORTED` 6002), since `PROTOCOL`/`PROTOCOL_MIN` in `crates/cobolt-wire/src/contract.rs` (both 1 today) become 2 (D9). There
  is no unencrypted mode, in development or release.
- **What it does not protect** (to be stated with WASM-013 R17, as its decided text already says): the agreement is unauthenticated, so a
  party that actively rewrites the handshake is stopped by TLS's server authentication alone; behind a TLS-terminating proxy that rewrites
  `hello`s, R24 does not hold. AC11 asks for what it does give: a **capture** at the TLS endpoint holds no readable frame, and no capture opens
  under a later start.
- **Scope (§7 O5, working assumption).** R24 covers the WebSocket channel, where every call and answer travels (AC11). The plain HTTPS routes —
  `/health`, `/dl/<token>` downloads (WASM-014 R15) and WASM-009's identity routes — stay TLS-only.

### 2.8 Retries and the blocked state (R25, R26, R7, R8; AC12) — new, task T15a

- **One policy (D11).** `cobolt_wire::retry::Policy { first: 10 s, step: 10 s, attempts: 3 }`, plain arithmetic in `cobolt-wire` (no feature,
  no dependency): attempt *n* has the window `first + (n-1)·step` → 10, 20, 30 s. Both clients use it; the spec's numbers are its defaults and
  the only values a release uses; tests pass a shorter policy. It is not a project setting (R25 fixes the numbers).
- **An attempt (§7 O6, working assumption).** An attempt is one connect → `hello` (→ key agreement) → `auth` given its window to complete. An
  attempt that fails before its window ends (on loopback a stopped backend refuses at once) **waits out the window** before the next starts,
  so the three attempts take 10, 20 and 30 s whatever the failure, as AC12 measures them; one that succeeds ends the cycle at once.
- **What starts a cycle (§7 O7, working assumption).** The loss of an established connection, and a call that needs a connection the client does
  not have. A page that makes no call opens no connection, as today.
- **During the cycle.** The client is **blocked** (R26): the player's state machine (`net.rs` `LinkState`) gains `Retrying { attempt, window_ends }`,
  and it reports *blocked* on and off through its outgoing queue (`Client::out`) — on at the break, off when an attempt succeeds or the cycle gives
  up; WASM-016 T29b draws the modal from it, WASM-012 T25b on a device. Calls wait (WASM-005 R18): a call's own timeout is **paused** while the
  client is blocked, so the cycle and not the call's timer decides its fate (D12). New calls made meanwhile (a timer's handler) queue.
- **When an attempt succeeds:** a new connection (a new session key, §2.7) and `auth`; subscriptions subscribe again (as today); calls that were
  sent when the line dropped follow R7 / WASM-014 R9 — repeated once under the same idempotency key **only** when their program is declared
  `idempotent`, otherwise failed *transport* 1001 with `may_have_run: true`; calls that had not been sent go out.
- **When the third attempt fails:** every waiting call fails *transport* — 1001 with `may_have_run: true` if it had been sent, 1000 if it never was
  — with the message "the backend could not be reached"; *blocked* goes off with **unreachable** on, which the player turns into WASM-016 R21's
  failure caption and event; no further attempt starts on its own. The next call the application makes starts a new cycle (§7 O8). The
  codes reuse WASM-014 R7's closed list; a distinct code is WASM-014's to add.
- **What retires (D13).** In the player: `Config::backoff_start_ms`, `backoff_max_ms`, the jitter (`Client::jitter`) and "a failed attempt that leaves
  nothing waiting stops there" (R25 retries any break). In the Rust client: `ClientConfig::reconnect_deadline`, `on_disconnect`'s 100 ms back-off and
  `ensure`'s four quick attempts. Their tests (`net.rs` "doubling, capped at backoff_max", and the rest of that group) are replaced by the policy's.
- **The Rust client and R26 (§7 O9, working assumption).** The same policy applies to the Rust client (`client.rs` says "the rules are the
  same"); it exposes *blocked*/*unreachable* to its host, but no spec draws a desktop modal, so a desktop form is not blocked by this task.
- **Server side:** nothing changes for R25; R8's keep-alive stands.

## 3. Data / model changes
- **Wire** (with WASM-014, which owns the members): `proto.rs` `Hello` and `ServerHello` gain `key` and `nonce` (base64, required from
  protocol 2); `PROTOCOL` and `PROTOCOL_MIN` become 2; after the handshake the envelope is unchanged but carried sealed in binary frames
  (§2.7). `Hello` still has no `session` member: the player's (ABI A15) is ignored by serde, as today.
- **ABI** (`specs/WASM-ABI-v1.md`): a row amending A15 — the sealed frames, the two members, and that the module still sees frames of A12 only
  (no change to the module or the generator). Written by the task that changes the player.
- **Configuration:** `cobolt.toml`, `exports.toml`, `backend.toml`, the bundle and `interface.json` **unchanged**. Player `Config` loses
  `backoff_start_ms`/`backoff_max_ms`; `ClientConfig` loses `reconnect_deadline`; both gain the policy value (defaults = R25).
- **Prebuilt player:** `assets/web-player/` is rebuilt after `net.rs`/`api.rs` change, and its source fingerprint (`src/fingerprint.rs`, the
  freshness test) updated.
- **Formats a user holds:** a Web bundle built before §2.7 cannot talk to a backend built after it (and the reverse); the backend answers its
  `hello` with the version refusal, which the player already reports (*version* 6001/6002). Acceptable on the unreleased 2.x.x line; stated.

## 4. Key decisions & alternatives
- **D1 — Re-host the HTTP front on Leptos; keep the session code.** Built (T7, T10a). Rejected: a session on `axum::extract::ws`; a hand-rolled front.
- **D2 — Leptos inside `cobolt-server`, not in the generated project.** Built.
- **D3 — `per-connection` = one run unit per connection, in turn.** Built (serialised by the ordering ticket, §2.6).
- **D4 — `unsafe_plain_ws` removed from the server.** Built (T4). *Reading recorded, for the next `/clarify`:* R5's "only … in a development run"
  and Q7's reverse-proxy mode together allow loopback `ws` in a release start behind a proxy.
- **D5 — `backend/exports.toml` is the single list.** Built.
- **D6 — The route table is closed.** Built.
- **D7 — The sealed channel is one module of `cobolt-wire` behind a feature.** Why: four speakers (server, Rust client, player, mock) must
  agree byte for byte, and `cobolt-wire` is already the protocol's one home that the player links; the feature keeps the compiler,
  semantic analyser and interpreter closures free of crypto. Rejected: a new crate (one more workspace member and path for the player's
  separate workspace, for no isolation the feature does not give); code in each speaker (four implementations of the same frames).
- **D8 — Keys and nonces in the two `hello`s, client first.** Why: the protocol already opens with the client's `hello` and the server's answer,
  so the agreement costs no extra round trip, and `auth` — the token — is then the first sealed frame. *Differs in wording from WASM-014 R18's
  decided text* ("the backend's in its hello, the frontend's in its reply"), which would need a client message after the server's `hello` and
  leave a public connection (no `auth`) with nothing to carry the key: **a proposal to WASM-014, not decided here (§7 O10).** Rejected: a key
  in a sub-protocol or a header (R11's spirit, and the browser cannot set headers on a WebSocket).
- **D9 — Implicit per-direction counters as nonces; any failure to open ends the connection; no plaintext mode.** Why: a 96-bit nonce must
  never repeat under one key; per-connection keys from fresh nonces make that true by construction; an implicit counter also refuses replay and
  reordering for free on an ordered transport. Rejected: random nonces sent in each frame (12 bytes per frame, no replay refusal without a
  window); a development switch for plaintext frames (it is how an unencrypted channel ships by mistake — the reason `unsafe_plain_ws` went).
- **D10 — The protocol number goes to 2 with no fallback.** Why: an old client cannot do the exchange, and WASM-014 R18 refuses a connection
  that cannot. The version refusal names the problem; a silent fallback to plaintext would defeat R24. Whether the contract version (WASM-014
  R16) also moves is WASM-014's.
- **D11 — One retry policy in `cobolt-wire`, numbers from the spec.** Why: two clients drifted into two back-offs; one value keeps them equal and
  makes AC12 testable with a short policy. Rejected: a project setting (R25 fixes the numbers).
- **D12 — A call's timeout pauses while blocked.** Why: WASM-005 R18 says the call *waits* while the frontend retries and its fate is decided by
  the reconnection or the give-up; the player's default call timeout (30 s, `net.rs` `Config::default`) is shorter than the 60 s cycle and would
  otherwise fail calls with 1002 mid-cycle. Rejected: letting the timer run (AC12's "the waiting call reaches `ON EXCEPTION` with *transport*
  after the third attempt" would depend on the call timeout).
- **D13 — No jitter.** Why: R25 fixes 10/20/30 s and AC12 measures them. Cost: many clients of one restarted backend return together (§5 R8).

## 5. Risks & mitigations
- **R1 — closed** (C13): the player speaks `proto.rs` since WASM-014 T6.
- **R2 — Leptos's closure.** Held (T6, T10a): no barred crate, highest `rust-version` 1.90; guards in `crates/cobolt-server/tests/dependency_guards.rs`.
- **R3, R4 — re-entry and locks in `per-connection`.** Held (T1, T1a, T5).
- **R5 — Size and build time.** Measured on the Leptos front: 71 s cold debug build, 67.2 MB (T10a). §2.7 adds the crypto crates; re-measure.
- **R6 — Old export lists.** Held (T9 migration).
- **R7 — Windows and Linux were never run** (still true). Code stays on `std`, `tokio`, `native-tls`, `hyper`, RustCrypto.
- **R8 — Crypto crate generations.** `Cargo.lock` holds `curve25519-dalek` 4.1.3, `hkdf` 0.12.4, `sha2` 0.10.9 *and* 0.11.0, `chacha20` 0.10.0,
  `aead` 0.6.1, and no `x25519-dalek` or `chacha20poly1305`. → Pick one RustCrypto generation for the four crates (the one matching `sha2`
  0.10/`hkdf` 0.12 that `x25519-dalek` 2 on `curve25519-dalek` 4 fits, unless the newer one is required), use the **same versions** in the
  player's own lock, check each `rust-version` ≤ 1.92 and add them to both dependency guards (`cobolt-server/tests/dependency_guards.rs`,
  `cobolt-web-player/tests/dependency_guard.rs`). If a barred crate or a newer MSRV is unavoidable, stop and report.
- **R9 — Randomness in the browser.** `Math.random` seeds the session id today (`api.rs`); a key from it would be guessable. → The key's 32
  bytes come from `crypto.getRandomValues` only; `net.rs` takes bytes and cannot make a key itself; a test asserts the player's key source.
- **R10 — Unauthenticated agreement.** An active party holding the TLS session can substitute keys. → Stated in WASM-013 R17 (decided text) and
  here (§2.7); not claimed otherwise; AC11 tests the passive capture it asks for.
- **R11 — Everything that speaks the protocol breaks at once** (server, Rust client, player, mock backend, the test clients). → One task changes
  them together, behind `cobolt-wire`'s module; the loopback, client-contract, e2e Chrome, `backend_cli` and `backend_build_wasm003` suites
  are the gate; a bundle/backend mismatch is reported by the version refusal (§3).
- **R12 — Retry tests take a minute.** The spec values give a 60 s cycle. → Logic tested with a short policy and the player's simulated clock;
  **one** real-socket run at the spec values, its windows printed (AC12 says "measured").
- **R13 — Synchronised reconnection** (D13). Accepted by the spec's fixed timings; the server's connection limit and rate limits (§2.2) bound it.
- **R14 — The prebuilt player must be rebuilt.** → The freshness test fails until it is; build it with a warm `target-player` (a cold build is
  ~4 GB); report the `player.wasm` size before and after.

## 6. Test strategy
Kept (they ran green on 2026-10-23, tasks.md T13/T14): `cobolt-server` lib, `client_contract`, `config_rules`, `dependency_guards`, `loopback`,
`token_rules`; `cobolt-cli` `backend_cli`; `cobolt-compiler` `backend_build_wasm003`; `cobolt-project-tools` `backend_secret_not_scanned`;
`cobolt-ide` `backend_run` and `web_settings`. **All are re-run on the sealed channel unchanged in what they assert.**

New for T15a (R25, R26; AC12):
- `cobolt-wire` `retry::tests` — windows 10/20/30 s, three attempts, nothing after the third.
- `cobolt-web-player` `net.rs` tests on the simulated clock (replacing the back-off group): a break → *blocked* on, attempts at windows
  10 000 / 20 000 / 30 000 ms, an early refusal waits out its window, a fourth attempt never starts; success in attempt 2 → *blocked* off, an
  `idempotent` call repeated once under its key, another failed 1001 `may_have_run: true`, an unsent one sent; give-up → every waiting call
  1000/1001, *unreachable* on; a call's timeout does not fire while blocked; a call after give-up starts a new cycle. Each prints the
  transitions and times.
- `cobolt-server` `loopback` with the Rust client: the same cases with a short policy over real sockets (stop the backend, restart it inside
  attempt 2), and **one run at the spec values** with the backend stopped for good — the three windows measured and printed, the call reaches
  *transport* after the third, no fourth connection seen by a listener on the port.
- Browser (with WASM-016 T29b): `cobolt-web-bundle` `e2e_chrome` against `mock_backend` stopped and restarted — the blocked state and the event
  reach the page (the modal's look is WASM-016 AC13–AC14).

New for T15b (R24; AC11):
- `cobolt-wire` `seal::tests` — the RFC 7748 X25519 vector and the RFC 8439 ChaCha20-Poly1305 vector through our wrappers; both sides derive the
  same two keys; two connections of the same key pairs derive different keys; a tampered, replayed, reordered or truncated frame is refused;
  a low-order peer key is refused; a key pair made from other bytes differs.
- `cobolt-server` `loopback::the_channel_is_ciphertext_at_a_terminating_proxy` (AC11) — client → a test proxy that terminates TLS and records
  every WebSocket message it relays → backend. Asserts: only the two `hello`s are text; every later message is binary; no recorded byte
  string contains the program name, a payload value, the token or `"t":`; then the backend and the client restart, a new capture shows
  different public keys, and replaying the first capture's frames into a new connection is refused. Prints message counts and the two key
  fingerprints.
- `loopback::a_connection_without_the_exchange_is_refused` — protocol 1 → *version* 6002; protocol 2 without `key` → *transport*; a text
  frame after the handshake → *transport*, connection closed.
- `loopback::the_frame_limit_is_the_plaintext_limit` — a payload exactly at `max_frame_bytes` passes, one byte more → 7000.
- `loopback::concurrent_callers_…` (AC7) re-run sealed; its 400-call time compared with the unsealed run and printed.
- `the_log_has_neither_…` (AC8) extended: no key, shared secret or nonce in the log.
- Player: a native test that `net.rs` refuses to start a sealed connection without key bytes from its owner; `dependency_guard` with the crypto
  crates; `freshness` after the rebuild; `e2e_chrome` against the sealed `mock_backend`.
- Dependency guards: the four crates present in the `server`, `client` and player closures, absent from the `loader` closure and the
  compiler's; nothing barred; MSRV printed.

Manual (operator, never driven by the agent): a Web project's backend started from the IDE, the page opened, the backend stopped and restarted —
the modal and the calls behave as AC12 says.

## 7. Open items
- **O1 — `cargo-leptos` (WASM-003 R2 vs WASM-017 R17).** The operator ruled on 2026-10-23 that the front uses Leptos for real (built, T10a) with
  plain `cargo build`. Whether `cargo-leptos` must ever run the backend build stays **to confirm**; if yes, the Backend Build gains a tool check
  (IDE strings ×6) and WASM-017 R17 an amendment.
- **O2 — The words `per-call` / `per-connection`.** This plan's spelling; the visible text is in Tr keys.
- **O3 — WASM-INDEX D4 and D5 still say "Open"** although Q2 and Q3 are decided (checked 2026-10-31); another file, left to its owner.
- **O4 — A `hello` member saying "state is new" after a reconnection.** WASM-014 defines none; nothing added (§2.6).
- **O5 — R24's scope** (working assumption §2.7): the WebSocket channel only; `/health`, `/dl/<token>` and the identity routes stay TLS-only. **To confirm.**
- **O6 — What R25's "timeout" is** (working assumption §2.8): the window an attempt is given, waited out when the attempt fails early. **To confirm.**
- **O7 — Whether a first connection that fails runs the cycle** (working assumption §2.8: yes, when a call needs it). **To confirm.**
- **O8 — After the give-up,** whether the application's next call may start a new cycle (working assumption: yes; R25 forbids only an attempt
  "on its own"). **To confirm.**
- **O9 — The Rust client and R26** (working assumption §2.8: same policy, state exposed, no desktop modal — no spec draws one). **To confirm.**
- **O10 — Handshake order** (D8): keys in the client's `hello` and the server's answer, against WASM-014 R18's wording "the frontend's in its
  reply". WASM-014 owns the members (its T19b); **to confirm** there before T15b starts.
- **Not done, not this spec's code:** the IDE's Output panel does not show the `connect-src` the build reports (needs a Tr label; tasks.md T12);
  no real identity provider was ever contacted; the modal (WASM-016 T29b, WASM-012 T25b); the threat-model text (WASM-013 T24b).

## 8. Steering compliance
- [x] **i18n ×6:** no new IDE string in T15a/T15b — the captions are the host page's (WASM-016 R20–R21); wire errors are codes + English text
  (§6 of the spec). Earlier keys (state, exports, Start/Stop) are in all six tables (T9, T10).
- [x] **Generated-code contract:** `interface.json` and the generated backend project are regenerated at every Build, never hand-edited; R24 puts
  no key in either.
- [x] **Docs:** `README.md` and `docs/` untouched on 2.x.x; what 2.x.x says about itself is in `specs/` (and the ABI row of §3).
- [ ] **System KB:** *Calling the backend* (`crates/cobolt-compiler/src/lib.rs`) gains the encrypted channel (no setting, nothing to write) and
  the retry behaviour a developer sees (`ON EXCEPTION` with *transport* after three attempts of 10, 20 and 30 s; idempotent programs
  repeated); `assets/knowledge/chunked.data` regenerated in the same change (`cargo run -p cobolt-ide --example build_chunked_kb`). No new
  `COBOL::"…"` built-in.
- [ ] **Interpreter/binary parity:** the Rust client is the one `rcrun run-form`, embedded child forms and the compiled binary use for
  `CALL … WITH WEBSOCKET LINKAGE`; one change in `client.rs` reaches all of them — verified by the compiled-binary test reaching a sealed backend.
- [x] **PRIME DIRECTIVE:** Rust only; Web Crypto is called from Rust through `web-sys`/`js-sys`, no new JavaScript source.
- [x] **User code is sacred:** no developer file is changed or removed.
- [ ] **Tech steering:** pure-Rust crypto, no `ring`/`aws-lc-rs`, guarded by tests in both lock files (spec 063 AC8 / 103); MSRV ≤ 1.92 checked.
- [x] **Fix vs feature:** feature (2.x.x, bump `z` only, once per finalize — T15a and T15b each have one).

## 9. Evidence
The 2026-10-20 evidence (hand-rolled front) was superseded by the re-run of **2026-10-23 on the Leptos front** recorded in tasks.md *Evidence
(re-run)*: AC1–AC10 pass (AC9's IDE half by `backend_run` tests; the window itself is the operator's to look at), built binary 71 s / 67.2 MB,
`cobolt-server` 70 tests. macOS only; no real identity provider. **AC11 and AC12 have no evidence yet** (T15b, T15a).

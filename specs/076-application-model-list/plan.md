# Plan — Spec 076: application model list and key store

## Context

Spec 076 (`specs/076-application-model-list/spec.md`, questions settled) gives a running
application a **model list** its program maintains and a **key store** the runtime keeps,
so an end user can add a model or rotate a key without a rebuild. The program keeps the
list in its own indexed file and hands entries over with CALLs; the runtime keeps only the
keys, encrypted in a file in the installation folder, behind a seam an OS keychain can
replace later. An `AgentObject` — and a `KnowledgeBase`'s endpoint embedder — are pointed
at an entry by name through a new `ModelEntry` property.

Operator ask (2026-09-24): implement right after 075, so this plan and its tasks were
written and executed in one pass.

## Verified facts the design rests on

- `agent_ask` (`interpreter.rs` ~11667) builds the one `AskRequest` from `AgentAPI`,
  `AgentURL`, `AgentEndpoint`, `AgentModel`, `AgentAPIKey`; the tool loop reuses it, so one
  site takes the entry. `agent_failed` sets `LastError` and queues `onError`.
- A `Configuration` binding (`cobolt-forms` `connections::apply_agent`) sets only API and
  URL at seed time; the key arrives through `COBOLT_CONNECTION_KEY_<ID>`; the model stays
  the agent's own. That precedence is untouched — the entry is consulted at `Ask` time and
  wins when set (R14).
- The verbose agent log prints request headers **with** the key (opt-in, documented). For a
  key from the store the log masks it (R7).
- Built-in CALLs are arms of `exec_call` (`COBOL-HTTP-SET-HEADER` is the pattern);
  `drain_async_ops` runs on every event wait — where change notices are raised.
- The application folder is `cobolt_forms::assets::current_base()` (what 068's
  `app_base` uses), set identically by all three hosts.
- `aes-gcm` 0.11, `sha2`, `getrandom` are pure Rust and already in `Cargo.lock`.

## Design

### 1. `cobolt-runtime/src/model_list.rs` — the list (process-wide, never stored)

`ModelEntry { api, url, model }`, keyed by upper-cased name in a process global with a
per-name **generation** (a removal bumps it too, with a tombstone). `set` bumps only when
the entry actually changes. Child forms share the process, so they share the list (R18).
Nothing is written to disk (R4).

### 2. `cobolt-runtime/src/key_store.rs` — the seam and the file store

```rust
pub trait KeyStore: Send + Sync {
    fn set(&self, name: &str, key: &str) -> Result<(), String>;
    fn remove(&self, name: &str) -> Result<(), String>;
    fn is_set(&self, name: &str) -> bool;
    fn get(&self, name: &str) -> Option<String>; // runtime-internal: no COBOL path reaches it (R6)
}
```

`MemoryKeyStore` (tests, AC5); `FileKeyStore` — `<app>/settings/model-keys.dat`, shared by
every user of the installation (R9). Contents: magic `PRCKEYS1`, a 12-byte random nonce,
then AES-256-GCM of a JSON map name → key. The cipher key is SHA-256 of a fixed label, the
**canonical installation folder** and the **machine's host name** (R10): unreadable at a
glance, and a copy moved to another folder or machine decrypts nothing. Writes go to a
temporary file then rename. A missing file is an empty store; an unreadable or
undecryptable one is reported (`tracing::warn`, and every later `set` answers why) and is
**never overwritten** (R11). `set_key_store(Arc<dyn KeyStore>)` swaps the store — the seam
(R8); the default is created lazily on first use.

### 3. COBOL surface

CALLs (R11a): `COBOL-MODEL-SET USING name api url model [status]`,
`COBOL-MODEL-REMOVE USING name [status]`, `COBOL-KEY-SET USING name key [status]`,
`COBOL-KEY-REMOVE USING name [status]`, `COBOL-KEY-IS-SET USING name flag` (`Y`/`N`).
`status` receives `OK` or the reason. No CALL returns a key (R6).

Properties: `ModelEntry` on `AgentObject` and `KnowledgeBase` (designer + run time). Event
**`onModelChanged`** on `AgentObject` (R16).

### 4. Using an entry

- `agent_ask`: `ModelEntry` set → the entry's API, URL and key; its model when the entry
  names one, else the agent's own; temperature, token limit, timeout stay the agent's
  (R12, R17). Unknown entry, or no key where the API needs one (`OpenAI`, `Anthropic`) →
  `onError` at once naming the entry, nothing sent (R15). The entry's generation is
  remembered per agent.
- `drain_async_ops`: an agent whose remembered entry's generation moved gets
  `onModelChanged` once (R16) — from any interpreter in the process.
- `kb_config`: `ModelEntry` set → `EmbeddingAPI`/`URL`/`Model`/`Key` from the entry (R13);
  an unknown entry fails the operation with `onError`.

⚠️ **R12 vs R17 on the model.** R12 says the entry supplies the model; R17 says the model
an agent sets on itself still applies. Resolved as: the entry's model when it names one,
the agent's otherwise. Flagged to the operator.

### 5. IDE, docs

`ModelEntry` rows in the Properties pane for both controls (property names stay English);
`onModelChanged` in the AgentObject event list; System KB tables (CALLs, property, event)
and `chunked.data`; guide *Where an agent's credentials live* gains the list, the store,
R14's precedence, a settings-form sketch, and the plain security note of spec §6.

## Verification

- `model_list` / `key_store` unit tests: generations; memory store; file store round-trip,
  no plaintext in the file, other folder decrypts nothing, missing → empty, corrupt →
  reported, unchanged, `set` refused (AC2, AC5–AC7).
- `crates/cobolt-runtime/tests/test_model_list.rs`, COBOL programs against the scripted
  model server: two entries, repoint between questions (AC1); key set/replace/remove seen
  on the wire, `COBOL-KEY-IS-SET` only Y/N (AC3); key absent from display output, errors,
  and every captured request but the auth header it belongs in (AC4); a `MemoryKeyStore`
  swapped in (AC5); precedence entry > Configuration env > own properties (AC9); unknown
  entry and missing key → `onError`, zero requests (AC10); `onModelChanged` after
  `COBOL-MODEL-SET` (AC11); a KnowledgeBase embedding through an entry against a scripted
  embedding server (AC8).
- AC12: the list and store live in the runtime, used identically by every host;
  `cargo tree` shows no `cobolt-ide`/`cobolt-agents`. A full three-host run is an operator
  step, as in 068 and 075.
- Sweeps `--no-fail-fast`: runtime, forms `--features render`, form-host, cli, compiler,
  ide `--bin`; `chunked.data` freshness test green.

## Risks

- The keys file is readable by every user of the installation and decryptable by anyone
  who can run the application on that machine — by design (R9, R10), said plainly in the
  guide; the OS keychain behind the same seam is the fix.
- A host name change makes the stored keys undecryptable: the store reports it and the
  administrator re-enters the keys. Documented.

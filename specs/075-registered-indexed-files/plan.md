# Plan — Spec 075: registered indexed files (`AgentObject::RegisterFile`)

## Context

Spec 075 (`specs/075-registered-indexed-files/spec.md`, questions settled) lets a running
program hand an `AgentObject` an indexed file **by path** — local, the OS's own network
path, or `smb://` — with no `FD`. The record layout comes from the `.cidx`, checked against
the schema the data file stores about itself. The file is only ever read. It is held in
memory when it fits; a local `PRCIDXD1` that does not fit is read in place; anything else
that does not fit is refused with the numbers. A registration lives only as long as the
process.

Binding decisions (operator, 2026-09-24):

- **`smb://` client: [`smb2`](https://crates.io/crates/smb2) 0.26.0** (MIT OR Apache-2.0),
  pure Rust — RustCrypto primitives, no `ring`, no `sspi`, no `-sys` crate, no FFI. NTLM and
  Kerberos; `connect`, `connect_share`, `stat`, `read_file` / streaming `download`. Chosen
  over `smb` 0.12.1 (hard-requires `sspi` + `ring`, C/asm) and `remotefs-smb` (default back
  end is Samba's C `libsmbclient`; its other back end is `smb` again).
- **The memory limit** of 065 R34 becomes a **project setting**: `[agents]
  file_memory_limit_mb` in `cobolt.toml`, default 64, shown in Project Settings. Today
  nothing sets it (`IndexedToolSet::set_memory_limit`, `mcp_tool.rs:279`, has no caller).
- A local temporary copy of a downloaded file is acceptable only if it is deleted and never
  written back.

Precondition met: spec §6's two defects shipped on `fixes` as 1.70.184 and are on `main`
(`d87aebf`) — a `PRCIDXD1` `INPUT` open is `write(false)`, and journal replay or migration
runs on a temp copy.

## Verified facts the design rests on

- `IndexedToolSet` (`mcp_tool.rs:231`) is per interpreter (`interpreter.rs:1531/2032`).
  `call` (`:304`) reopens the file on every search through `open_for_reading` (`:530`);
  `scan` only calls `read_seq`. `exceeds_memory_limit` (`:513`) applies to the in-RAM
  container only; `DEFAULT_MEMORY_LIMIT_BYTES` is at `:507`.
- `agent_allow_file` (`agent_loop.rs:469`) needs an `FD` (`file_specs`), and replaces a
  same-named entry (`deny` then `allow`). The tool is `search_<slug>`; model-facing text
  names the file, never its path.
- `FileAccess::from_design_time_definition` (`mcp_tool.rs:182`) already builds record length,
  primary key and leaf columns from a `.cidx`, but is documented as not for a delivered one.
  `IndexedDefinition` (`cobolt-indexed/src/model.rs:279`) carries `comment` (the purpose),
  `keys`, and fields with `pic`/`usage`/`offset`/`length`/`occurs`/`redefines`/`comment`;
  `validate_definition` (`structure.rs:159`). `load_indexed` never fails on content — bad
  XML parses to "UNNAMED" with no fields.
- `IndexedFile::inspect_path` (`indexed.rs:1242`) reads a `PRCIDX1`'s stored schema;
  `load_prcidx` parses bytes. `DiskIndexedFile::inspect_path` (`indexed_disk.rs:3048`) reads
  the header without write access. `schema_matches` (`:2611`) compares record format, key
  count, each key's parts and duplicates. A `<file>.jrn`, a pending migration, or an
  unfinished reclaim make an `INPUT` open copy the file to temp (`:1776`).
- Project values reach all three hosts through process globals: `seeding::publish_connections`
  is filled by rcrun from the manifest (`form_gui.rs:217`) and by the built binary from a
  baked `const` (`lib.rs:3149`, `:3330`); child forms share the process. The memory limit
  follows this pattern — no interpreter constructor changes.
- `sysinfo` 0.31 is a workspace dependency, used only by the IDE. The runtime runs blocking
  work on a `std::thread`; `maps_bridge.rs:31` already owns a private current-thread tokio
  runtime.

**Found while planning — defects, for `fixes`, not this spec:** `open_for_reading` opens a
`PRCIDXD1` with no alternate keys and strict metadata on, so an `AllowFile` search of a DISK
file with alternates fails with FS 39; and `indexed_ide::key_specs_from_def` panics on a key
with no parts (`indexed_ide.rs:116`). 075 avoids both paths.

## Design

### 1. COBOL surface (AgentObject, synchronous)

- `RegisterFile(data-path, cidx-path [, name])` → `1`/`0`. `name` defaults to the `.cidx`'s
  file name; the model sees `search_<slug(name)>`, as for `AllowFile`. A second data file
  sharing one `.cidx` passes its own name. Registering a name again replaces the entry,
  whichever method made it.
- `UnregisterFile(name)` → `1` when a registered file was withdrawn; frees its records. It
  never removes an `AllowFile` entry (`DenyFile` still does).
- Run-time properties, written on every `RegisterFile` (kept apart from `LastError`, which an
  `Ask` in flight may overwrite): `RegisterResult` (`MEMORY`, `DISK`, or a refusal code),
  `RegisterMessage` (English, path masked), `RegisteredName`, `RegisterFileBytes`,
  `RegisterLimitBytes` — enough for a program to build its own translated message.
- **Synchronous** because the interpreter has its own thread (the GUI keeps painting), local
  registration is a stat, a header read and one load, and `smb://` is bounded by a deadline.
  A return value plus a code satisfies R10 without event machinery.

Refusal codes: `BAD-PATH`, `NOT-FOUND`, `CIDX-NOT-FOUND`, `ACCESS-DENIED` (local permission
or SMB login), `UNREACHABLE` (DNS, refused, timeout), `CIDX-INVALID`, `NO-PURPOSE`,
`NO-FIELDS`, `NO-FIELD-DESCRIPTIONS`, `NOT-INDEXED`, `FORMAT-UNSUPPORTED` (redb, legacy
`PRCISAM1` — no schema to check), `RECORD-LENGTH-MISMATCH`, `KEY-MISMATCH`, `CORRUPT`,
`JOURNAL-PRESENT` (R16), `NEEDS-UPGRADE` (a `PRCIDXD1` needing migration that does not fit:
reading it in place would copy it), `TOO-LARGE-FOR-LIMIT`, `TOO-LARGE-FOR-FREE-MEMORY`
(R20), `SMB-UNAVAILABLE` (built without the `smb` feature).

### 2. `crates/cobolt-runtime/src/registered_file.rs` (new)

- **Location:** `Smb(SmbUrl)` for `smb://…`, else `Fs(PathBuf)` — relative paths resolve
  through `cobolt_forms::assets::resolve`; UNC works through `std` on Windows; `/Volumes`,
  `/mnt`, `/media` are plain paths. OS network paths are `Fs`, so a `PRCIDXD1` there may be
  read in place (R19); only `smb://` is refused when too large (R20).
- **`SmbUrl`:** `smb://[domain;]user[:password]@server[:port]/share/path`, credentials
  percent-decoded. `Display`/`Debug` always print `user:****@`; the password is dropped as
  soon as the fetch returns and never stored (R6a, R13).
- **`.cidx` → `FileAccess::from_registered_definition`** (a new constructor, deliberately not
  the design-time one): load from a string (so an `smb://` `.cidx` works), `validate_definition`,
  every leaf has offset and length, `record_length()` ≤ the declared length; build the
  declared schema (format, primary + alternates, parts, duplicates) and compare it with the
  stored one through `schema_matches`' rule, extracted as `pub(crate) fn schema_equivalent`.
  A multi-part key is `KEY-MISMATCH` (engine keys are single-part). Columns are the leaves.
  The `FileDescription` comes from the same definition and must pass R5 (purpose, fields,
  field descriptions). `FileAccess` gains `alternates`, so a strict engine open matches.
- **Fit:** pure `decide(container, location, size, limit, free) → Memory | InPlace |
  Refuse(code, size, bound)`: fits iff `size ≤ limit` and `size ≤ free / 2` (R18); a misfit is
  `InPlace` only for `PRCIDXD1` on `Fs` with no migration pending. `free` comes from `trait
  FreeMemory` (default `sysinfo` available memory, RAM refresh only); tests inject a figure
  (`Interpreter::set_free_memory_probe`, `#[doc(hidden)]`) for AC11.
- **Order of checks** (nothing ever written to the user's file): parse path → `<data>.jrn`
  present (locally or on the share) → `JOURNAL-PRESENT` → size → container sniff → stored
  schema (`IndexedFile::inspect_bytes`, new, wrapping `load_prcidx`; `DiskIndexedFile::inspect_path`
  plus new `input_needs_copy(path)`) → `.cidx` validation → fit → load: `PRCIDX1` parsed from
  its bytes; local `PRCIDXD1` via `read_disk_container`; an `smb://` `PRCIDXD1` written to a
  private temp file (`create_new`, deleted by a drop guard even on error), read, deleted.

### 3. `mcp_tool.rs`

- `FileAccess.source: FileSource` — `Assigned` (today's reopen-per-call, unchanged),
  `Loaded(Arc<Vec<Bytes>>)` (records in key order; `scan` iterates them), `InPlace(PathBuf)`
  (before each search, refuse if a `.jrn` has since appeared; open `DiskIndexedFile` `INPUT`
  with the full key list).
- `IndexedToolSet::register(desc, access)` / `unregister(name)`, entries tagged registered.
- Process global `publish_file_memory_limit(bytes)` / `file_memory_limit()`, first call wins;
  `IndexedToolSet::default` reads it, so every interpreter — child forms included — gets the
  project value.
- Amend the module's R30 note: registered files are the one exception (071 R23). Record
  length and keys are checked against the file's own schema; column offsets are trusted from
  the `.cidx`, and the worst a wrong one can do is mislabel columns in a search that only
  reads.

### 4. `smb://` with `smb2` (`src/smb_source.rs`)

- Runtime feature `smb = ["dep:smb2", "dep:tokio"]`, `smb2 = { version = "=0.26.0",
  default-features = false, features = ["tokio"] }` — pinned exactly: the crate is young
  (0.26.0 released 2026-09-24) and moves fast. Pure Rust, so it could be always on; it stays
  a feature only to keep tokio's networking out of apps that have no `AgentObject`, the same
  trimming as SQL/HTTP/Maps. `RuntimeFeatures.smb` is set by `scan_forms` for
  `ControlType::AgentObject`; `cobolt-form-host` forwards it; `cobolt-cli` enables it (Run
  Form parity).
- `trait SmbFetch { fn stat(&SmbUrl) -> Result<Option<u64>, Refusal>; fn read_all(&SmbUrl,
  max) -> Result<Vec<u8>, Refusal> }` — `stat` serves the journal probe and the size check
  before any download. `RealSmb`: one `std::thread` owning a current-thread tokio runtime (as
  `maps_bridge.rs`), `connect(host:port, user, password)` → `connect_share` → `stat` /
  `download` streamed with a running `max` → `disconnect_share`, inside one `block_on`; the
  caller waits `recv_timeout(30 s)`.
- **Guest (R12):** `smb2` documents NTLM and Kerberos but not guest explicitly. The plan
  connects as `Guest` with an empty password over NTLM; the live test (below) is what proves
  it. If it fails, guest access is reported as a gap to the operator rather than faked.
- Without the feature, `smb://` → `SMB-UNAVAILABLE`; URL parsing and masking are always
  compiled.

### 5. Project setting → three hosts

- Compiler: `AgentsConfig { file_memory_limit_mb: u64 }` on `CoboltProject` (default 64;
  0/absent → 64) and `pub fn project_file_memory_limit(manifest) -> u64`. Generated source
  bakes `const PROJECT_FILE_MEMORY_LIMIT_MB`; `run_form_app` calls
  `publish_file_memory_limit` beside `publish_connections`. rcrun calls it in `form_gui.rs`;
  child forms inherit it.
- IDE: `AgentsSettings` on `project_model.rs` (`serde(default)`, skipped when default);
  Project Settings (`panels/settings_form.rs`, Runtime section) gets a 1–65536 MB field; new
  `Tr` keys `lbl_runtime_file_memory_limit`, `hint_runtime_file_memory_limit` in all six
  languages.

### 6. Interpreter

`exec_method`: `REGISTERFILE` / `UNREGISTERFILE` → `agent_register_file` /
`agent_unregister_file` in `agent_loop.rs`; both added to `is_known_method` (the sync test
stays green); the five properties added to the AgentObject run-time list
(`runtime_property_names_for`, `cobolt-forms` `model.rs`). Logging carries only the masked
location; no path reaches a tool description or result.

## Phases

1. Memory-limit plumbing: runtime global, compiler, rcrun, IDE setting + i18n, tests.
2. Engine helpers: `inspect_bytes`, `input_needs_copy`, `schema_equivalent`; `sysinfo` in the
   runtime.
3. `registered_file.rs`: location, `SmbUrl`, validation, `decide`, codes — unit tests.
4. `FileSource` and `register`/`unregister` in `mcp_tool.rs`.
5. Interpreter methods and properties; local end-to-end tests.
6. `smb` feature: `SmbFetch`, `RealSmb`, `RuntimeFeatures`, form-host/cli features.
7. Parity: generated source, `cargo tree`.
8. Docs: System KB tables (methods, properties, codes, fallback) + `chunked.data`; guide
   section *Registering a file by path* (GOLDEN RULE #8: delete the invalidated
   translations).
9. Measurements, sweeps, CHANGELOG + `z` per commit, on `features`.

## Verification

- **AC1–AC3, AC5 (local):** `crates/cobolt-runtime/tests/test_registered_files.rs`, through
  the interpreter and the `test_agent_tool_calling.rs` mock model server — a program with no
  `FD` registers a file and the model's tool answers; wrong record length / keys / no
  purpose / no fields / no descriptions each give their code and no tool is offered;
  `UnregisterFile` removes the tool; a missing file is reported and the next `Ask` still
  answers.
- **AC4:** the same fixture read by local path and through a fake `SmbFetch` serving its
  bytes gives identical tool output. A real OS network path and a real share run in an
  `#[ignore]` live test driven by `COBOLT_TEST_SMB_URL`, `COBOLT_TEST_SMB_GUEST_URL`,
  `COBOLT_TEST_NET_PATH` — **an operator step** (a Samba container or the operator's share),
  reported as such, never claimed.
- **AC5/AC6 (SMB):** the fake maps unreachable and login-refused to their codes; a capturing
  `log`/`tracing` subscriber, the mock server's request bodies and every `Register*` property
  are searched for the password — none may contain it. Credentialed and guest login against a
  real server: the live test only.
- **AC7:** an `assert_untouched` helper (bytes + mtime before/after) wraps every test.
- **AC8:** a file and folder made read-only are read; the read-only share is in the live test.
- **AC9:** a `.jrn` beside the file → `JOURNAL-PRESENT`, both unchanged.
- **AC10–AC12:** limit and injected free memory — `PRCIDXD1` under → `MEMORY`, over →
  `DISK`; free memory decides (AC11); `PRCIDX1` and a fake-SMB file over the limit refused
  with both numbers (AC12); `decide` also table-tested.
- **AC13:** generated source carries the baked limit and the publish call; the manifest
  carries `"smb"` when the project has an `AgentObject`; `cargo tree -p cobolt-runtime
  --features smb` shows no `cobolt-ide`, `cobolt-agents`, `ring`, `cc` or `-sys` crate; the
  run-form, child-form and binary paths give the same results.
- **AC14:** each test prints files registered, records searched, open time in memory and from
  disk, searches per second.
- Sweeps `--no-fail-fast`: `cobolt-runtime` (with and without `--features smb`),
  `cobolt-forms --features render`, `cobolt-form-host`, `cobolt-cli`, `cobolt-compiler`,
  `cobolt-ide --bin cobolt-ide`; the `chunked.data` freshness test green.

## Risks

- **`smb2` maturity:** 0.26.0, released the day of this plan, largely AI-written by its
  author's own account (with mock, property and Samba-in-Docker tests). Pinned exactly;
  guest access and signing against real servers are unproven until the live test runs.
- **"Free memory"** is `sysinfo`'s *available* figure, which differs by OS — documented.
- **A `.cidx` with shifted field offsets** that passes the record-length and key checks makes
  columns read wrong — read only, but misleading; accepted by 071 R23.
- **A slow share** blocks the interpreter thread up to 30 s; the GUI keeps running.

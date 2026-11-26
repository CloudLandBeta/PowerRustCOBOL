# Plan — Install package for a Web project

- **Status:** approved (operator, 2026-11-25)
- **Spec:** ./spec.md   **Date:** 2026-11-25

## 0. Before any code (sequencing)

**P0 — the nginx fix must be on `main` first.** The site this package writes uses
`csp::nginx_static`, which exists only in commit `38977a0a` (version 2.1.40, branch `fixes`, not
merged). CLAUDE.md rule #5 asks for pending fixes to be committed, merged and pushed before a
feature starts, and "merge into main only when asked": **the operator has to say so.** `/implement`
does not begin until `main` holds it and `features` is fast-forwarded to it.

**P1 — the version.** `main` becomes 2.1.40 with the fix. `features-windows-install-prep`
already spent 2.1.21–2.1.39 from the old `main`. This feature takes the next `z` that no branch
uses, checked on every branch at the time of each commit.

## 1. Approach

Four parts, each small, joined by one function.

**A. The Backend binary learns three things** (`cobolt-server`, the `standalone` entry that every
built Backend already runs; the generated `main.rs` does not change): to stop on the system's own
signal, to say it is ready, and to write a log file. R8–R11.

| System | How it is a native service | Where it logs |
|---|---|---|
| Linux, systemd | a `Type=notify` unit: the Backend sends `READY=1` after it listens and `STOPPING=1` when asked to stop; `SIGTERM` stops it | the journal (standard output) |
| macOS, launchd | a `LaunchDaemon` with `KeepAlive`; `SIGTERM` stops it | `logs/backend.log`, rotated by the Backend |
| Windows, SCM | `--service` makes the executable the service itself: it calls the Service Control Manager's dispatcher, reports start-pending, running, stop-pending, stopped | `logs\backend.log`, rotated by the Backend |

Three new arguments, all optional, none changing today's terminal run (R11):
`--env-file PATH` (the secrets, read before the configuration, same on all three systems),
`--log-file PATH` (size-rotated, 10 MB × 5), `--service` and `--service-name NAME` (Windows only).

**B. One function builds the package** (`cobolt-project-tools`, `web/package.rs`): checks the
project, builds the page and the Backend, makes the production `backend.toml`, runs the new
Backend with `--check` against it, writes the zip and the script. The IDE, `rcrun package` and the
MCP tool all call it (R1, R26).

**C. The install script is text written by Rust** (`cobolt-web-bundle`, `install.rs`, beside
`deploy.rs`): one generator per system family, the messages in six languages, the nginx site made
from `csp::nginx_static` (R14–R23).

**D. The IDE** (`cobolt-ide`): File ▸ Package Project for a Web project runs B on a background
thread and reports in the Output panel (R1, R24, R25).

### The package's content (R2, R3)

```text
<name>.zip
  web/                      the bundle, without hosting/
  backend/<name>-backend[.exe]
  backend/backend.toml      production: made from the project's (see 3.2)
  backend/data/             the project's data/, a seed for a first install
  backend/assets/KB/        the project's assets/KB, only when it has one (Q6, see below)
  LICENSE  NOTICE  POWERRUSTCOBOL-NOTICE.txt
<name>-install.sh | <name>-install.ps1
```

**Q6 settled.** The Guide lists `KnowledgeBase` among the controls that work through the Backend,
and the runtime resolves its default `assets/KB` against the Backend's working directory
(`kb_runtime::app_base`, falling back to the working directory when no application folder is
anchored). So the zip carries `assets/KB/` beside `backend.toml` when the project has that folder,
and the script seeds it like `data/`, never overwriting. A bare Backend reads nothing else from the
disk: the key store is a desktop-IDE concern and is not in the Backend's code path (checked: no
reference in `cobolt-server` or `backend_build.rs`). The task that builds the zip re-checks this
with a grep before it is closed.

A Web project with **no Backend** gets the same two files with no `backend/` and no service: the
script installs the site only. (Amendment A1.)

### What the script does (R14–R23), in order

1. Parse parameters. The nginx folder is the one required parameter: `./<name>-install.sh
   /etc/nginx` and `.\<name>-install.ps1 -NginxDir C:\nginx`. Missing or wrong → usage, exit 2,
   nothing changed. The nginx configuration is `<dir>/conf/nginx.conf` or `<dir>/nginx.conf`.
2. Check the system: root or Administrator, `unzip` (Linux, macOS), the zip beside the script, its
   SHA-256 against the one the script was written with (Amendment A2), the required secret names in
   the environment file, the certificate and key when the address is `wss://` (R7), a free port.
3. Save what it will change: `nginx.conf`, a previous site, a previous service definition.
4. Extract; install the web root and the Backend; **seed** `data/` only where a file is absent
   (R19).
5. Create the service account, write the unit, plist or service, restrict `data/`, `logs/` and the
   environment file to it.
6. Start the service and wait until its port answers.
7. Write the nginx site and, only if `nginx.conf` does not already read a directory of sites
   (`conf.d/*.conf`, `servers/*`), add one marked `include` line (R17). Run nginx's own test; reload
   only if it passes.
8. Any failure from step 3 on restores the saved files, removes what this run created, restarts
   the previous service if there was one, and exits non-zero (R18).
9. Print the address, the service name, the status and the log commands (R23).

Re-running the script is the update (R19, R20): steps 3–8 run again over the same installation,
`data/`, the environment file and the logs are kept, and the `include` line is added once.

## 2. Affected crates / files

- `crates/cobolt-server/src/standalone.rs` — the three arguments, the signal wait, readiness and
  stop; calls the modules below.
- `crates/cobolt-server/src/service.rs` (new) — `wait_for_stop()` for Unix (SIGTERM, SIGINT) and
  console, `notify_ready()` / `notify_stopping()` (the `NOTIFY_SOCKET` datagram, no crate), the
  environment-file reader.
- `crates/cobolt-server/src/service_windows.rs` (new, `cfg(windows)`) — the dispatcher, the control
  handler, the status reports, through `windows-sys`.
- `crates/cobolt-server/src/logfile.rs` (new) — a size-rotating writer used as both the record sink
  and the `tracing` writer.
- `crates/cobolt-server/Cargo.toml` — `windows-sys` for `cfg(windows)` only, at 0.61 (0.61.2 is
  already in `Cargo.lock`), with the `Win32_System_Services` and `Win32_Foundation` features.
- `crates/cobolt-compiler/src/backend_build.rs` — on Windows (MSVC) the Backend is linked with the
  C runtime inside the executable (R13), through a `.cargo/config.toml` written in the Backend's own
  staging folder, so no other build is affected.
- `crates/cobolt-web-bundle/src/install.rs` (new) — the script generators, the six-language
  messages, the site, the unit, the plist, the Windows service commands; `lib.rs` exports it.
- `crates/cobolt-project-tools/src/web/package.rs` (new), `web/mod.rs` — the package function, its
  progress callback and its result; `Cargo.toml` gains `zip` with the pin the IDE and `rcrun` already
  use (`version = "9"`, `default-features = false`, `features = ["deflate-flate2"]`: no C libraries).
- `crates/cobolt-cli/src/main.rs` — `rcrun package` for a Web manifest calls the function;
  `--lang CODE`; the usage text.
- `crates/cobolt-ide/src/app.rs` — `do_package_project` and `package_project_to` branch on
  `is_web_project()`; a background receiver polled once a frame; the one-build-at-a-time guard
  shared with Build.
- `crates/cobolt-ide/src/app/mcp_ide_tools/project.rs`, `crates/cobolt-project-tools/src/tools/ide.rs`
  — the `package_project` tool answers with the two paths for a Web project; its description says so.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields, six languages (see 3.4).
- `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` — `z` and the entry.
- `docs/developers-guide-en.md` — chapter 23, a new section "Installing a Web application on a
  server", with a flowchart, a Note and the Caveats; and chapter 24's list of Backend arguments.
- `.github/workflows/web-install-package.yml` (new, `workflow_dispatch` only) — proves the three
  systems on hosted runners (see 6).

## 3. Data / model changes

### 3.1 No change to the project format
`.project.toml`, `.wcfrm` and `exports.toml` are untouched. Nothing is migrated.

### 3.2 The production `backend.toml` (R5, R6)
Made with `toml_edit` from the project's own file, so the developer's comments survive, under a
first line that says it was made by Package and where the source is.

| Key | Rule |
|---|---|
| `[server] listen` | the project's, which must be a loopback address; a `[tls]` section or a network address stops the package: "this package installs behind nginx" |
| `[server] allowed_origins` | the project's list **plus** the origin of `backend_url` (`wss://h/ws` → `https://h`), once |
| `[auth]` / `[identity]` | the project's, unchanged; absent → the package stops (R5) |
| any `unsafe_…` | the package stops (R5) |

The check is the Backend's own: the packaged executable runs `--check --config <the file>` in a
temporary folder, with every `env:NAME` it finds given a harmless value in the **child's**
environment only, and every `file:PATH` pointed at a temporary file. A refusal is shown as the Backend
words it. The result also gives the port the script needs.

### 3.3 The Backend's arguments (R8–R11)
`--env-file`, `--log-file`, `--service`, `--service-name` join `--config`, `--check`, `--describe`,
`--version`. A built Backend is a release run: `--env-file` never overrides a variable already set,
never prints a value, and refuses a file that other users can read on Unix.

### 3.4 Strings
- IDE (`Tr`, six languages, none says "cobolt"): `pk_web_pack_started`, `pk_web_pack_step`
  (`{}` = the step), `pk_web_pack_done` (`{}` zip, `{}` script), `pk_web_pack_how` (the one command),
  `pk_web_pack_failed`, `pk_web_pack_needs_address`, `pk_web_pack_needs_settings`,
  `pk_web_pack_busy`.
- The script: about twenty messages in the six languages, one table in `install.rs`, selected by the
  IDE's language at packaging; English where a code is unknown (R22). The `.ps1` is written with a
  byte-order mark so Windows PowerShell 5.1 reads Japanese and Chinese; the `.sh` is plain UTF-8.

### 3.5 The names
`slug` is the existing `deploy::slug`. Service: `<slug>` (Linux, Windows), `<slug>` reversed under a
neutral prefix for launchd (`app.<slug>`). Account: `<slug>` on Linux, `_<slug>` on macOS, the
virtual account `NT SERVICE\<slug>` on Windows. Display name: "`<Project name>` Backend".

### 3.6 Default places
| System | Install root | Notes |
|---|---|---|
| Linux | `/opt/<slug>` | `web/`, `backend/` (the service's working folder) with `data/`, `logs/` owned by the account |
| macOS | `/usr/local/<slug>` | same layout |
| Windows | `%ProgramFiles%\<Project name>` | same layout; `data\`, `logs\` and `secrets.env` get an ACL for the virtual account and Administrators |

## 4. Key decisions & alternatives

- **The Backend is the service; no wrapper.** The three systems each have a native way: the Windows
  dispatcher, systemd's notify protocol, launchd's plain foreground process. — *Rejected:* NSSM or
  WinSW (a third-party download the operator would have to trust and ship, against R21 and R13), and
  a shell loop under cron (not a service).
- **Windows through `windows-sys`, not the `windows-service` crate.** `windows-sys` 0.61 is already
  locked and shipped with the SDK; the five calls are fixed and small. A new crate adds a lock entry,
  an SBOM component and a vendoring step to every machine's SDK. — *Rejected:* `windows-service`:
  nicer API, but all of that for a file of about 150 lines. *Cost accepted:* an `unsafe` module that
  only a Windows runner can run (6).
- **`Type=notify`, implemented without a crate.** The protocol is one datagram. — *Rejected:*
  `Type=simple` (systemd would call the service started before it listens, against R10).
- **`--env-file` in the Backend, not the system's own mechanism.** A Windows service's environment
  lives in the registry where any local user can read it. One reader, one permission rule on
  all three. — *Rejected:* `EnvironmentFile=` for systemd only (a different behaviour per system).
- **The check runs the real Backend.** What the package accepts is what the service will accept,
  by construction, with no second copy of the rules. — *Rejected:* a structural validator in the
  project tools (drifts from `config.rs`); making `config.rs` loader-level (a large refactor of a
  file the IDE does not need).
- **Orchestration in `cobolt-project-tools`.** It already holds the Web Build (`GeneratorBackend`),
  depends on `cobolt-compiler` (`build_backend`) and `cobolt-web-bundle`, and both the IDE and
  `rcrun` link it. — *Rejected:* in the IDE (the CLI and MCP would need a copy) and in `cobolt-cli`
  (the IDE would have to spawn `rcrun` and parse its words).
- **The nginx site goes where nginx already reads sites.** `conf.d/*.conf` and `servers/*` need no
  change to `nginx.conf`; only a configuration that reads neither gets one marked `include`.
  — *Rejected:* always editing `nginx.conf`.
- **Seed, never overwrite.** `data/` from the zip is copied file by file only where the server has
  none (R19; the developer's data is sacred).
- **The C runtime is linked into the Windows Backend** (R13, Q7). — *Rejected:* shipping the
  redistributable DLLs in the zip (a path on the build machine that changes with Visual Studio).
- **No separate `package` window.** The destination dialog is the one File ▸ Package Project already
  has; progress goes to the Output panel as the Build's does. — *Rejected:* a settings dialog; the
  decisions that remain belong to the script's parameters, not to the IDE.

## 5. Risks & mitigations

- **R-a. The Windows service cannot be run on this Mac.** → The module is small and reviewed against
  the SCM's documented states; the workflow runs it on `windows-latest` (install, stop, kill,
  start, reboot-less restart) and the result is recorded in the task. Until it runs, the CHANGELOG
  says "not verified on Windows".
- **R-b. systemd and `useradd` cannot be run here.** → The Linux mechanics run in a systemd
  container on this machine (Docker is available) with a stand-in Backend that speaks `sd_notify`; the
  real Backend's signal and notify code is unit-tested on this Mac and on `ubuntu-latest`.
- **R-c. macOS needs `sudo`, which this session cannot use.** → The macOS mechanics run on
  `macos-latest` (passwordless sudo); here the script is checked with `sh -n` and its generator is
  tested as text.
- **R-d. Linking the C runtime statically recompiles the Backend's whole dependency tree** on a
  Windows machine's first package (the shipped libraries were built with other flags), and a C
  dependency may not accept it. → Done only for the Backend, in its own staging folder; the first
  package says why it takes longer; the workflow checks the executable imports no `vcruntime`
  library. If it does not link, the fallback is stated in R13's task, not guessed.
- **R-e. `nginx.conf` varies** (Debian `conf.d`, Homebrew `servers`, Windows zip, source builds).
  → Detect, don't assume; a test table of five real layouts; when none fits, the added `include`
  is inserted at the first `http {` and `nginx -t` decides.
- **R-f. SELinux (RHEL, Fedora) blocks nginx from reading `/opt` and from reaching the Backend's
  port.** → The script sees `getenforce` = Enforcing and applies the labels and the
  `httpd_can_network_connect` boolean, or prints exactly the two commands when it cannot; stated in
  the Guide's Caveats. *Not testable here.*
- **R-g. PowerShell 5.1 and 7 differ** (encoding, `Expand-Archive`, `Get-FileHash`). → The script
  uses only what both have; the BOM; the workflow runs both (`powershell.exe` and `pwsh`).
- **R-h. A long first package** (the Backend builds with Cargo). → Progress per step in the Output
  panel; the platform-compile question (spec 112 / 2.1.19) applies as it does to a Build.
- **R-i. The script holds a hash and the zip must stay beside it.** → The script says so when it
  cannot find the zip; renaming both together works because it looks beside itself by its own stem.
- **R-j. A second feature in flight (`features-windows-install-prep`) touches the Windows
  installer.** → No shared file: this one adds `install.rs`, `package.rs`, `service*.rs`; the
  `CHANGELOG.md`, `version.rs` and the Guide will conflict textually and are resolved at merge.

## 6. Test strategy

Every test reports quantified, readable results (the files, the sizes, the times, the pass tally).

**cobolt-server (runs here, macOS):**
- the environment-file reader: comments, quotes, a missing file, a world-readable file refused, a
  variable already set kept;
- the rotating writer: rotates at the size, keeps five, never splits a record;
- `SIGTERM` stops the server within its grace time and exits 0 (a child process sent the signal);
- `notify_ready` / `notify_stopping` write the right datagrams to a path socket the test owns;
- the existing `cobolt-server` and `cobolt-compiler` suites stay green (AC6).

**cobolt-web-bundle (runs here):**
- the generated `.sh` for each of the six languages passes `sh -n`; no line holds "cobolt"; no
  credential pattern; the messages table has the same keys in the six languages (AC12, AC14, AC2);
- the nginx site and the unit/plist/service texts hold the fields the script fills, and the site
  passes the structural rule `nginx_rules_hold` from the fix;
- the five `nginx.conf` layouts (3.5/R-e) decide the right include and the right folder.

**cobolt-project-tools (runs here):**
- `package_web` on `PowerWebHello` and on `PowerOficina` with a `wss://` address: exactly two files, a
  listing of the zip with and without each entry R3 names, the production `backend.toml` has the
  origin and keeps the comments, no secret (AC1, AC2);
- the refusals of R5, one test per cause, each leaving **no** file (AC3);
- a project without a Backend packages the site only (A1);
- the Backend binary of the run is built once per sweep and `--check` is run on it (the long test,
  `#[ignore]` unless `PRC_BUILD_TESTS` is set, as the other build tests are).

**cobolt-ide (runs here):** the existing i18n tests (every `Tr` field in six languages); the
Desktop and Mobile packaging tests unchanged (AC13); a test that the Web branch is taken only for a
Web project.

**Linux mechanics (runs here, in a systemd container):** a Debian image with systemd and nginx, a
stand-in Backend, a zip made by the real function: AC8–AC11 — wrong parameter, missing certificate,
broken nginx configuration restored, update keeps an edited record, second run changes nothing.

**On hosted runners (the operator dispatches `web-install-package.yml`):** Linux, macOS, Windows
each build `rcrun`, package a Web example, run the script, and check AC4, AC5, AC7, AC10 and AC12
for real, with `powershell.exe` and `pwsh` on Windows. The result of each run is copied into the
task's note. A line not run is reported as not verified on that system.

**Manual:** open `PowerOficina` in the IDE, File ▸ Package Project, read the Output; the two files
in the folder; do not drive the application window (the operator judges the look).

## 7. Steering compliance

- [ ] i18n: the IDE's `pk_web_pack_*` fields and the script's table, six languages each
- [ ] Generated-code banner + regenerate-on-action: unchanged (`.cbl` is not touched); the package
  regenerates forms first, as Build does
- [ ] English Developer's Guide updated in the same change; translations: none exist, none edited
- [ ] System KB: no control, property, method or event changes; nothing to rebuild
- [ ] Interpreter/binary parity: not applicable (the interpreter is untouched; the Backend's
  runtime is unchanged); the WebAssembly generator is untouched
- [ ] Fix vs feature: **feature**, branch `features`, never in a commit with a fix; `z` +
  `CHANGELOG.md` entry; the next free `z` after P0/P1
- [ ] "Rust only": the scripts are Rust strings; no `.sh` or `.ps1` in the repository; the one
  YAML file is the CI workflow beside the existing ones
- [ ] No "cobolt" in user-facing text (IDE strings, script messages, service names); COBOL names
  stay English
- [ ] Credentials: none in the zip, the scripts or the workflow; `credscan.sh` gates the commit

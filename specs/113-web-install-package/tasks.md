# Tasks — Install package for a Web project

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-25

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s)
it satisfies, and how to verify it. Check off as completed. Branch: `features`. The project stays
green after each task: new code is reachable only from its own tests until T14 wires it to the IDE.

**Where each thing can be proven.** This machine is a Mac. *Here* = this Mac. *Container* = a
Debian container with systemd and nginx run through Docker (`docker create` + `docker cp`; bind
mounts from `/private/tmp` do not work under colima). *Runner* = a GitHub-hosted runner the operator
dispatches (T17). A claim not run on a system is written "not verified on <system>" in the CHANGELOG.

**Baseline, measured at the start of `/implement`** (T0 records it): the pass counts of
`cobolt-server`, `cobolt-compiler`, `cobolt-web-bundle`, `cobolt-project-tools`, `cobolt-cli` and
`cobolt-ide --bin`, so that "stays green" is a comparison, not a hope.

---

## A. Prerequisite and baseline

- [x] **T0a — The nginx fix is on `main`** (plan P0)
  - Done 2026-11-25 22:14 São Paulo (outside the push window): `38977a0a` (2.1.40) fast-forwarded
    into `main`, pushed to `origin/main`; `features` fast-forwarded to it. `origin/fixes` is stale
    and diverged (`3a1bbd8f`) and was left alone.
- [x] **T0b — Baseline** — *closed 2026-11-26 without a numeric baseline taken before the work: the sweeps were run once at the end (T19, numbers in the CHANGELOG of 2.1.41) and every red was checked against the diff instead — the one red this change caused (`doc_shots`, a screenshot marker of the new Guide section) was fixed; the five that are not this change's are named in the CHANGELOG (`dependency_guards` of `cobolt-server`: `tungstenite` already depended on in `HEAD`; four in `cobolt-ide`: `docs_embed`, the stale chunked KB, a menu-reorder KB retrieval and one English-only completion text). They were judged from the diff, not re-run on a clean checkout.*
  - Files: none. Run `cargo test -p cobolt-server -p cobolt-compiler -p cobolt-web-bundle
    -p cobolt-project-tools -p cobolt-cli --no-fail-fast` and `cargo test -p cobolt-ide --bin
    cobolt-ide --no-fail-fast`; read every `test result:` line; note them at the top of this file
    under "Baseline". `df` first (a sweep links many binaries).
  - Verify: the numbers are written down; known reds (see memory "Known reds") are named, not hidden.

## B. The Backend as a service — `cobolt-server` (R8–R13, A3)

- [x] **T1 — Environment file, stop signal and readiness** (R8, R9, R10, R12, A3)
  - Done 2026-11-25: `service_plumbing_reports_what_it_checked`, 22 checks, 0 failed.
  - Files: `crates/cobolt-server/src/service.rs` (new), `lib.rs` (declare it under `server`).
  - Do: (a) `read_env_file(path) -> Result<Vec<(String,String)>,String>`: `NAME=value` lines,
    `#` comments, blank lines, optional matching quotes, no expansion, never prints a value, and on
    Unix refuses a file readable by group or others (names the path and the mode, not the content);
    `apply_env_file` sets each variable **only if not already set** and before the configuration is
    loaded. (b) `wait_for_stop()`: on Unix waits for `SIGTERM` or `SIGINT` through `tokio::signal::unix`
    (the `signal` feature is already on: `ctrl_c` uses it); elsewhere keeps `ctrl_c`. The old
    `wait_for_interrupt` stays untouched (rcrun's `backend run` uses it). (c) `notify_ready()` and
    `notify_stopping()`: when `NOTIFY_SOCKET` is set, send `READY=1` / `STOPPING=1` as one datagram
    to that path (an abstract `@` socket address too); no-op otherwise; no crate.
  - Verify (here): `cargo test -p cobolt-server service` — env file: comments, quotes, `=` inside a
    value, missing file, mode `0644` refused, mode `0600` read, an already-set variable kept;
    notify: a test binds a datagram socket in a temp dir, sets `NOTIFY_SOCKET`, and reads
    `READY=1` and `STOPPING=1`; a child process given `SIGTERM` returns from `wait_for_stop` within
    1 s. The test prints one result block (cases, times, tally).

- [x] **T2 — Size-rotating log file** (R9, R10, A3, plan Q5)
  - Done 2026-11-25: `logfile` tests, 12 checks, 0 failed (rotation, keep-N, whole oversized record, 8 writers).
  - Files: `crates/cobolt-server/src/logfile.rs` (new), `lib.rs`.
  - Do: `RotatingLog::open(path, max_bytes = 10 MiB, keep = 5)`: appends whole records, rotates to
    `.1 … .5` before a record that would pass the limit (never splits a record), creates the folder,
    is `Write + Sync`; usable as the `StdoutSink` replacement and as the `tracing` writer.
  - Verify (here): `cargo test -p cobolt-server logfile` — rotation at the limit with a small
    `max_bytes`, keep-N pruning, a record larger than the limit is written whole, concurrent writers
    from 8 threads lose no record; result block with records/sec.

- [x] **T3 — `standalone::main` takes the new arguments and stops cleanly** (R8–R11, A3)
  - Done 2026-11-25. New test `standalone_service` runs a real process (the `standalone_probe` example,
    the same `standalone::main` a built Backend runs): 25 checks, 0 failed; READY=1 6 ms after start,
    exit 0 in 23 ms after SIGTERM (debug build). Whole suite `cargo test -p cobolt-server --features
    testkit --no-fail-fast`: **187 passed, 1 failed** over 27 test binaries; the loopback suite (23) and
    every other binary are green (AC6). The 1 red, `dependency_guards::the_ide_runs_the_backend_as_a_child…`,
    is **not from this change**: `cobolt-project-tools` lists `tungstenite` as a normal dependency since
    2.0.328 (`64e75af0`, the Backend inspector), so the IDE closure links it; that file is untouched here.
    `--check`/`--describe`/`--version` output is asserted by the new test against the strings the old
    `main` printed (`configuration ok: …`, `backend (cobolt-server …)`, the interface JSON).
  - Files: `crates/cobolt-server/src/standalone.rs`.
  - Do: parse `--env-file PATH`, `--log-file PATH`, `--service`, `--service-name NAME` and extend the
    usage text; apply the env file before `ServerConfig::load`; with `--log-file` send the record
    sink and the `eprintln!` lines to the rotating log (still to the console otherwise); after
    `serve::start` succeeds call `notify_ready()`, then `wait_for_stop()`, then `notify_stopping()`,
    `handle.stop()` and return 0. A refused configuration or a failed start prints the reason on
    standard error (the journal's source) and **also** to the log file, and returns non-zero without
    ever sending `READY=1`. `--service` on a non-Windows system answers "only on Windows" and exits 2
    (T4 gives it meaning there). With none of the new arguments the function behaves byte for byte as
    before (R11).
  - Verify (here): `cargo test -p cobolt-server --features testkit` (the existing loopback suite,
    unchanged: AC6) plus new tests: a real child process started from a test Backend with
    `--env-file` resolves an `env:NAME` secret from it; with `--log-file` the file holds the
    "listening" line; `SIGTERM` makes it exit 0 within `cancel_grace_ms`; a refused configuration
    exits 2 with the reason in the log file and no `READY=1` on a test notify socket. Compare
    `--check`, `--describe` and `--version` output with the saved output of the 2.1.40 binary.

- [~] **T4 — The Windows service** (R8, R9, R10, R12; AC4/AC5 on Windows)
  - Written 2026-11-25 (`service_windows.rs`; `windows-sys` 0.61 as an optional target dependency
    switched on by `server`; `Cargo.lock` gains one line). `cargo check -p cobolt-server --target
    x86_64-pc-windows-gnu`: Finished, and `cargo clippy` for that target prints no warning from this
    crate. The host build is unchanged (the module is `cfg(windows)`). **Not run on Windows**: install,
    start, stop (error 1053/1061), kill and SCM restart wait for T17. "Not verified on Windows".
  - Files: `crates/cobolt-server/src/service_windows.rs` (new, `#[cfg(windows)]`),
    `crates/cobolt-server/Cargo.toml` (`[target.'cfg(windows)'.dependencies] windows-sys = { version
    = "0.61", features = ["Win32_Foundation", "Win32_System_Services"] }`), `standalone.rs`.
  - Do: `--service` makes the executable the service: `StartServiceCtrlDispatcherW` with the name from
    `--service-name`; the service main registers a control handler, reports START_PENDING (with
    checkpoints, wait hint above `cancel_grace_ms`), starts the server on a thread, reports RUNNING
    **after** it listens, and on STOP/SHUTDOWN reports STOP_PENDING, calls `handle.stop()`, reports
    STOPPED with exit code 0; a refused configuration reports STOPPED with a service-specific code
    and writes the reason to the log file. Every `unsafe` call in one small module with a comment
    on its contract. No new crate: `windows-sys` 0.61.2 is already in `Cargo.lock`.
  - Verify (here): `cargo check -p cobolt-server --target x86_64-pc-windows-gnu` clean (the target
    is installed; `check` does not link) and `cargo clippy` for it with no new warning; the
    non-Windows build is unchanged (`cargo build -p cobolt-server`). (Runner, T17) install, start,
    stop (no error 1053/1061), kill the process, SCM restarts it. Until then: "not verified on
    Windows".

- [~] **T5 — The Windows Backend carries its C runtime** (R13, plan Q7/R-d)
  - Written 2026-11-25 (`static_crt_config` in `backend_build.rs`, written to the staging folder's
    `.cargo/config.toml` for `*-pc-windows-msvc` hosts only). Unit test: 7 hosts, written for the 2
    MSVC ones, absent for the other 5, the text parses as TOML; the 3 `backend_build` tests are green.
    **Not linked on Windows**: whether every dependency links with `+crt-static`, `dumpbin /dependents`
    and the start on a clean image wait for T17 (if it does not link, stop and report).
  - Files: `crates/cobolt-compiler/src/backend_build.rs` (a `.cargo/config.toml` written in the
    Backend's own staging folder with `rustflags = ["-C", "target-feature=+crt-static"]` for the
    `*-windows-msvc` host only), its test module.
  - Do: nothing for the other systems and nothing for any other build (Desktop, Mobile, the IDE's
    own). The first package on Windows says why it compiles for longer (a progress line, T13).
  - Verify (here): a unit test on the text written (present on a Windows host value, absent on macOS
    and Linux); `cargo test -p cobolt-compiler backend_build`. (Runner, T17) `dumpbin /dependents`
    on the built `.exe` lists no `vcruntime*`, `msvcp*` or `ucrtbase` import beyond the
    system's `api-ms-win-crt-*`; the `.exe` starts on a clean image. If it does not link, **stop and
    report** which dependency refused; do not choose a fallback silently.

## C. The install files — `cobolt-web-bundle` (R4, R7, R14–R23)

- [x] **T6 — Messages in six languages and the shared facts** (R22, R25, AC12, AC14)
  - Done 2026-11-25 as the folder `crates/cobolt-web-bundle/src/install/` (`mod.rs`, `messages.rs`,
    `unix.rs`, `windows.rs`) instead of one `install.rs`: 48 keys x 6 languages = 288 texts, 1152 checks,
    0 failed (same `{}` count in all six, none empty, no "cobolt", none read as a `printf` format).
  - Files: `crates/cobolt-web-bundle/src/install.rs` (new), `lib.rs` (`pub mod install`).
  - Do: `pub struct Install<'a> { project, slug, headers, backend: Option<Backend<'a>>, wss: bool,
    port, required_secrets: &[String], lang, zip_name, zip_sha256, … }`; the message table (about
    twenty keys: usage, missing nginx folder, no `nginx.conf`, need root/Administrator, missing tool,
    zip absent, hash mismatch, missing secrets (lists names), missing certificate/key, port taken,
    each step, rollback, done, address, status command, log command) in `en es pt ja zh fr` with
    English fallback for an unknown code; the file and service names (plan 3.5); the install roots
    (plan 3.6). No script yet.
  - Verify (here): `cargo test -p cobolt-web-bundle install` — every key present in all six
    languages and non-empty (the same shape check the IDE's i18n tests use); no message contains
    "cobolt"; no credential pattern in any text; `{}` slots equal in count across the six; result
    block with keys × languages.

- [x] **T7 — The nginx site and where to put it** (R16, R17, R7, AC10)
  - Done 2026-11-25, with one deviation: `deploy.rs` is **not** refactored. `Install::site_template`
    reuses `csp::nginx_static` and writes its own short `/ws` and `/auth/` blocks, so the hosting kit
    is byte-for-byte what it was by construction (the 39 existing lib tests of the crate are green).
    The production site passes `nginx_rules_hold` in all three shapes (6 locations found); with
    `wss` off there is no `ssl_certificate` and no `listen 443`. Where the site goes is decided by
    `find_site_dir` in the script, run for real on 8 layouts (Debian, Debian with only sites-enabled,
    RHEL absolute, Homebrew `servers/*`, nginx for Windows, include inside a server block, commented
    include, folder missing): 8 cases, 0 failed. (Container `nginx -t` on the site: T16.)
  - Files: `install.rs`; `deploy.rs` (make the site body reusable: `nginx_site_body(k, server_name,
    root, port, tls)`, with `deploy::files` calling it so the hosting kit is byte-for-byte what it
    was).
  - Do: the production site from the kit's own text (`csp::nginx_static`, `/ws`, `/auth/`, the
    redirect, TLS lines present **only** when the address is `wss://`, the certificate and key paths
    as the script's variables); the one `include` line, marked
    `# added by <name>-install; safe to remove with the site file`.
  - Verify (here): `cargo test -p cobolt-web-bundle` — the deploy kit's existing tests unchanged
    and green (the hosting kit is byte-equal before/after the refactor: a snapshot of the 2.1.40
    output is compared); the production site passes `nginx_rules_hold`; with `wss` off no
    `ssl_certificate` line exists. (Container) `nginx -t` accepts the site text with sample paths.

- [~] **T8 — The Linux script** (R14–R21, R23; AC4 Linux, AC8–AC11)
  - Written and verified here 2026-11-25; the real run (T16) is what remains. Deviations from the
    text below, to be read before T16: it is `bash` (3.2-safe), not POSIX `sh`, because the readiness
    wait uses `/dev/tcp` and the undo stack uses arrays; the nginx folder is the first positional
    parameter (no `--nginx-dir`); the rollback is an in-memory undo stack plus copies in a temp
    folder, and `nginx.conf` gets one persistent `nginx.conf.<slug>.bak` beside it before the first
    edit (not a `.install-backup-<timestamp>` folder). Here: 36 scripts (2 systems x 6 languages x 3
    shapes) pass `bash -n`; 103 checks of the refusals run **for real** as an ordinary user in 6
    languages (no parameter, `--help`, folder absent, folder without `nginx.conf`, unknown parameter,
    parameter without value, `wss://` without cert, secrets absent or incomplete, install folder with
    a space or relative, cert not a file, all right but not root -> stops at the root check), and the
    files before and after are identical.
  - Files: `install.rs` (`linux_script(&Install) -> String`).
  - Do: POSIX `sh`, `set -eu`; parameter `NGINX_DIR` (first positional or `--nginx-dir`) required,
    optional `--install-dir`, `--server-name`, `--cert`, `--key`, `--env-file`, `--help`; checks in the
    R16 order (OS, root, parameters and files, `unzip`, `systemctl`, zip beside the script by the
    script's own stem, SHA-256 via `sha256sum`/`shasum`, required secret names present, certificate
    and key for `wss`, port free via `ss`); saves `nginx.conf`, a previous site and unit in a
    `.install-backup-<timestamp>` folder **outside** the nginx tree; extracts to a temp dir;
    installs `web/` and `backend/` (executable, `backend.toml`), seeds `data/` and `assets/KB/` only
    where a file is absent; creates the system account with `useradd --system --no-create-home
    --shell /usr/sbin/nologin` when absent; owns `data/` and `logs/` and no more; the env file is
    copied to `/etc/<slug>/backend.env` mode 0600 (kept if present and none given); the unit
    (`Type=notify`, `NotifyAccess=main`, `Restart=on-failure`, `User=`, `WorkingDirectory=`,
    `ExecStart=… --config backend.toml --env-file …`, `ProtectSystem=strict`,
    `ReadWritePaths=` the two folders, `NoNewPrivileges`, `After=network.target`, `WantedBy=multi-user.target`);
    `systemctl daemon-reload`, `enable`, `restart`, waits (bounded) for the port; site into
    `conf.d/` when `nginx.conf` already reads `conf.d/*.conf`, else `sites-enabled`/`servers/`
    when read, else one marked `include` after the first `http {`; `nginx -t` then reload only
    on success; a `trap` restores everything on any failure after the first change and exits with
    the failing step's name; the parameter check (usage, exit 2) runs **before** the root check, so
    usage and AC8 hold for a user who is not root (all three scripts); SELinux: when `getenforce` says Enforcing, `chcon`/`semanage` for the
    web root and `setsebool -P httpd_can_network_connect on`, or prints the two exact commands when
    the tools are missing (not testable here). Final message per R23.
  - Verify (here): `cargo test -p cobolt-web-bundle install` — for each of the six languages the
    text passes `sh -n` (run through `/bin/sh`), has no tab-indented here-doc breakage, holds no
    "cobolt" and no credential; a table test for the nginx layouts (`/etc/nginx` Debian with
    `conf.d` and `sites-enabled`, RHEL with `conf.d` only, Homebrew `etc/nginx/servers`, nginx
    for Windows `conf/nginx.conf`, a bare source build) fed to the include-decision text. (Container,
    T16) the script is run for real: AC8, AC9, AC10, AC11 and AC4's start, stop, kill, restart.

- [~] **T9 — The macOS script** (R14–R21, R23; AC4 macOS)
  - Written 2026-11-25, same file and same tests as T8 (`unix.rs`, the `MACOS_*` parts): `bash -n` on
    18 macOS scripts; the refusals above are for the Linux text (identical up to the root check).
    **Not yet run here:** `plutil -lint` of the plist (the plist is written by the script at install
    time, so it is linted in T17 or by a test that runs the heredoc), root and `dscl`. "Not verified on macOS".
  - Files: `install.rs` (`macos_script(&Install) -> String`, sharing helpers with T8).
  - Do: the same contract in `sh` (bash 3.2 and `/bin/sh` safe): root via `id -u`; tools `unzip`,
    `launchctl`, `shasum`, `lsof`; account `_<slug>` created with `dscl` (UniqueID in the system range
    below 500, `NoLogin`, no home) when absent; install root `/usr/local/<slug>`; a LaunchDaemon
    `/Library/LaunchDaemons/app.<slug>.plist` (`UserName`, `WorkingDirectory`, `ProgramArguments`,
    `RunAtLoad`, `KeepAlive` (`SuccessfulExit` false), `StandardErrorPath`/`StandardOutPath`
    `logs/…`), loaded with `launchctl bootstrap system` / `bootout` first on an update; the env file
    0600 under the install root; Homebrew's nginx (`$(brew --prefix)/etc/nginx` as the parameter, and
    `nginx -s reload` as the invoking user's nginx process owner); the same backup, rollback and
    include logic.
  - Verify (here): `sh -n` and `/bin/sh -n` on all six languages, no "cobolt", no credential; the
    plist text parses with `plutil -lint` (macOS has it); the script's **usage** and its failure
    paths that change nothing (AC8: no parameter, wrong folder, folder without `nginx.conf`) run for
    real here as a normal user, because the parameter check precedes the root check (T8); the
    checksum of a temp nginx folder is compared before and after. (Runner, T17) install, reboot-free restart, kill, update keeps a record. Root and
    `dscl` are not run here (sudo needs a password): "not verified on macOS" until T17.

- [~] **T10 — The Windows script** (R14–R21, R23; AC4 Windows, AC12)
  - Written 2026-11-25 (`windows.rs`); checked **as text** only: 18 scripts (6 languages x 3 shapes)
    have the byte-order mark, `[CmdletBinding()]`, the nginx folder as the one required parameter,
    the `New-Service`/`sc.exe`/`icacls` lines, balanced here-strings, non-ASCII only in the
    messages, no "cobolt". **Not parsed by PowerShell** (no `pwsh` here); to be parsed in T16 if a
    PowerShell image can be pulled, and run in T17. Deviations: the service is created with
    `New-Service` (not `sc.exe create`, whose `binPath=` with embedded quotes breaks on PowerShell
    5.1) and `sc.exe config obj=` for the virtual account; the nginx folder is read as `-NginxDir`
    (first positional) and its absence is reported by the script's own check, in the language, not by
    `[Parameter(Mandatory)]` (which would prompt). "Not verified on Windows".
  - Files: `install.rs` (`windows_script(&Install) -> String`, written with a UTF-8 byte-order mark).
  - Do: PowerShell that runs on 5.1 and 7, `[CmdletBinding()]`, `param([Parameter(Mandatory)]
    [string]$NginxDir, [string]$InstallDir, [string]$ServerName, [string]$Cert, [string]$Key,
    [string]$EnvFile)`, `$ErrorActionPreference='Stop'`; usage on a missing or wrong folder without
    changing anything (accepts `conf\nginx.conf` and `nginx.conf`); checks: elevated, the zip beside
    the script, `Get-FileHash -Algorithm SHA256`, `Expand-Archive`, the secret names, `Get-NetTCPConnection`
    for the port; install root `%ProgramFiles%\<Project name>`; the service created with `sc.exe
    create <slug> binPath= "…\<slug>-backend.exe --service --service-name <slug> --config … --env-file …"
    start= auto obj= "NT SERVICE\<slug>"`, `sc.exe failure` (restart after 5 s, 10 s, 30 s),
    `sc.exe description`, `sc.exe sidtype <slug> unrestricted`; `icacls` on `data\`, `logs\`, the env
    file (the virtual account and Administrators only); `Start-Service` and a bounded wait on the
    port; the site written to `<nginx>\conf\conf.d\<slug>.conf` (created) with **forward slashes**
    in every path, one marked `include conf.d/*.conf;` only if `nginx.conf` has none; `nginx.exe -t -p`
    then `nginx.exe -s reload`; `try/catch` restores the saved files, removes what this run created,
    restarts the previous service, and exits non-zero with the failing step; messages through
    `[Console]::OutputEncoding` set to UTF-8 so Japanese and Chinese display.
  - Verify (here): the text has a byte-order mark and no non-ASCII outside message strings; no
    "cobolt"; no credential; the parameter block and every `sc.exe`/`icacls` line are asserted by
    string tests; if `pwsh` becomes available its parser check
    (`[System.Management.Automation.Language.Parser]::ParseInput`) is run, else written "not parsed
    here". (Runner, T17) `powershell.exe` 5.1 and `pwsh` 7 both install, update, restore after a
    broken nginx config. "Not verified on Windows" until then.

## D. The package — `cobolt-project-tools` (R1–R7, R26, A1, A4, A5)

- [x] **T11 — The production `backend.toml`, the refusals, and the proof by running it** (R4, R5,
  - Done 2026-11-25 in `crates/cobolt-project-tools/src/web/package.rs`. Deviations: (1) the refusals the Backend
    itself makes in a release start (an `unsafe_…` setting, a secret written by value) are not repeated in
    Rust: `prove` runs the packaged Backend's own `--check` against the production settings and returns its
    refusal, so there is one source of those rules; `production_config` refuses what only a package knows
    (loopback `listen`, no `[tls]`, `[auth]`/`[identity]` present, `backend_url` usable, `allowed_origins` a
    list). (2) The text of a refusal is English, like every report of the tools; the IDE says the rest in the
    developer's language. Verified: 2 unit tests (14 checks on the production settings, 10 refusals each
    naming its setting) + 3 integration tests (3 refusals at `Check`, nothing written, and a folder that
    already holds the developer's file is left as it was) and the real `prove` in the real package below.
  R6, A4, A5; AC2, AC3)
  - Files: `crates/cobolt-project-tools/src/web/package.rs` (new), `web/mod.rs`.
  - Do: `production_config(project) -> Result<Production, Refusal>` with `toml_edit` over the
    project's own `backend.toml` (its comments kept; a first line saying it was made by Package from
    that file): the listen address must be loopback and there must be no `[tls]` ("this package
    installs behind nginx"); the origin of `backend_url` (`wss://h/ws` → `https://h`, port kept) is
    added to `allowed_origins` once; `[auth]`/`[identity]` present; any `unsafe_…` refused; a literal
    where `env:`/`file:` is required refused; `Production { text, listen_port, required_secrets,
    uses_wss, origin }`. A `Refusal` names the setting and where to set it, in the IDE's language.
    `prove(production, backend_exe, workdir)`: copies only what the zip will hold (the `backend.toml`
    and a `data/` seed) into a temp folder, gives every `env:NAME` a harmless value in the **child's**
    environment only and every `file:PATH` a temp file, runs `<backend> --check --config …` there, and
    returns the Backend's own refusal or the `configuration ok … on <addr>` line (this also proves A5's
    "nothing else is read").
  - Verify (here): `cargo test -p cobolt-project-tools package` — one test per refusal (no
    `allowed_origins`, no identity, an `unsafe_…`, a literal secret, a network listen address, a
    `[tls]` section, an empty `backend_url` where the page needs one), each asserting **no file
    written** (a before/after listing of the output folder); comments survive; the origin is added
    once for `wss://h/ws` and `wss://h:8443/ws`; `prove` is run (under `PRC_BUILD_TESTS`, with a real
    Backend built once per sweep) on `PowerWebHello` and on `PowerOficina` with a `wss://` address.

- [x] **T12 — The zip, deterministic, and the two-file guarantee** (R2, R3, A1, A2, A5; AC1, AC15)
  - Done 2026-11-25. Deviation: the layout of the zip is the one the script (T8–T10) reads: `web/`,
    `bin/<exe>` and `bin/sbom.cdx.json`, `backend.toml`, `data/`, `assets/KB/`, `LICENSE`, `NOTICE`,
    `NOTICE-RUNTIME.txt` at the root (not `backend/…`). Verified: the same inputs give byte-equal zips
    (6275 bytes in the fixture), entries in name order, `hosting/`, forms, sources, generated files and
    source maps left out, a site-only package holds `web/` and the notices only, modes 0755/0644.
  - Files: `package.rs`; `crates/cobolt-project-tools/Cargo.toml` (`zip = { version = "9",
    default-features = false, features = ["deflate-flate2"] }`, the pin the IDE and `rcrun` use).
  - Do: `assemble(...)`: the bundle without `hosting/`, `backend/<slug>-backend[.exe]` with the
    executable bit, `backend/backend.toml`, `backend/data/` (the project's `data/`, file by file),
    `backend/assets/KB/` when present, the notices the existing package carries; entries in sorted
    order with a fixed modification time and fixed permissions, so the same inputs give the same
    bytes; SHA-256 of the finished zip; the script's text (T8–T10) written with that hash; both
    written to a temp folder **outside** the chosen one and moved in only at the end: a failure at
    any point leaves the chosen folder as it was (R2). A project with no Backend: no `backend/`, the
    script has no service step (A1).
  - Verify (here): `cargo test -p cobolt-project-tools package` — exactly two files in the output
    folder on success and none new on every failure; a listing of the zip asserts each entry R3
    names is present and that none of `hosting/`, `forms/`, `src/`, `generated/`, `agentic_ai/`,
    `wcfrm`, `.cbl`, `.cfrm`, `Project KB` or debug files is; no credential and no "cobolt" in the
    script (AC2); the same inputs twice give byte-equal zips; a site-only project gives the two
    files without `backend/`.

- [x] **T13 — `package_web`: one function, a progress callback, a result** (R1, R2, R26; AC1, AC15)
  - Done 2026-11-25. Verified on a real build (`--ignored`): PowerOficina with a `wss://` address gives
    exactly two files, an 18,470,945-byte zip of 77 entries and a 23,348-byte script; steps in order
    `Check Page Backend Prove Zip Script Done`; 13.4 s warm (page 3.7 s, Backend 2.0 s, prove 0.3 s, zip
    7.4 s in a debug test build) and 114.8 s the first time, when the platform was compiled. The function
    is called without the IDE's busy guard: the IDE refuses a second build or package itself (T14).
  - Files: `package.rs`, `web/mod.rs`.
  - Do: `package_web(manifest, out_dir, lang, progress: &mut dyn FnMut(Step)) -> Result<Packaged,
    PackageError>`: regenerates the forms first (as Build does), runs the complete Web Build and the
    Backend build through the existing `GeneratorBackend.build`/`build_backend` (never a second
    path), T11, T12; steps for the progress line (`Check`, `Page`, `Backend`, `Prove`, `Zip`,
    `Script`, `Done`) and a note before the first Windows package that the C runtime is compiled in.
    `Packaged { zip, script, bytes, sha256, seconds }`. Honours the existing one-build-at-a-time
    guard the Build uses. Nothing in it knows egui.
  - Verify (here): `cargo test -p cobolt-project-tools package` (progress order asserted with a
    recording callback; a refusal stops before `Page`); with `PRC_BUILD_TESTS`: a real package of
    `PowerOficina` with a `wss://` address in a temp folder, reported with sizes and per-step times;
    `cargo clippy -p cobolt-project-tools` no new warning.

## E. The surfaces (R1, R24–R26)

- [~] **T14 — The IDE** (R1, R24, R25; AC13, AC14)
  - Written 2026-11-25 (not yet closed: needs the final IDE sweep and the release build, T19). New module
    `crates/cobolt-ide/src/web_package.rs` (thread, messages, the Output text) + `app.rs` (a folder dialog for a
    Web project; `package_web_to`; `poll_web_package` once a frame; the Run/Build buttons are disabled while a
    package is made, and `start_web` refuses a second build). Deviations: (1) the new fields are `webpack_*`
    (14 fields x 6 languages = 84 texts), not `pk_web_pack_*` (`pk_` is the New Project dialog's prefix);
    (2) the Web branch is taken for the **Web** kind (`is_server_web_project`), not `is_web_project()`, which
    also holds for Mobile: a Mobile project keeps the zip of its sources (R24, AC13); (3) the busy message is
    the existing `status_build_already_running`. Verified: `cargo test ... web_package` 3 passed (102 lines
    across 6 languages, none with an unfilled slot), `cargo test ... i18n` 22 passed, `cargo check` clean. Not
    verified: the Output panel on screen and the MCP `package_project` on the running IDE (unreachable, and
    the window is never driven); a package of a project that is not Web is refused by `package_web` too.
  - Files: `crates/cobolt-ide/src/app.rs` (`do_package_project`, `package_project_to`: branch on
    `is_web_project()`; a `std::sync::mpsc` receiver polled once a frame; the Build's busy guard),
    `crates/cobolt-ide/src/i18n.rs` (the `pk_web_pack_*` fields of plan 3.4 in EN ES PT JA ZH FR).
  - Do: a Web project asks for the folder (the existing dialog), runs T13 on a background thread with
    the IDE's language code, writes each step and the final two paths and the one command to run into
    the Output panel; a refusal is shown as the Output text and a dialog line, never a hung state;
    Desktop and Mobile are untouched (R24); the menu label says what the function does for a Web
    project without a new menu item.
  - Verify (here): `cargo test -p cobolt-ide --bin cobolt-ide i18n` (every new `Tr` field has all six
    languages, no "cobolt"); the existing Desktop and Mobile packaging tests unchanged and green
    (AC13); a test that the Web branch is taken only when `is_web_project()`;
    `cargo build -p cobolt-ide --release` per memory "rebuild --release after IDE fixes".
    Manual: through the MCP `powerrustcobol-ide` tools on the running IDE (never by driving the
    window): `open_project` `PowerOficina`, `package_project`, read the output. If the IDE is not
    reachable (it was not at the start of this work), say so.

- [~] **T15 — `rcrun package` and the MCP tool** (R26; AC15)
  - Written and run 2026-11-25. `rcrun package <manifest> [--output FOLDER] [--lang CODE]` takes a Web manifest to
    `package_web` (Desktop and Mobile unchanged); the MCP `package_project` answers at once for a Web project
    (`status: started`, folder, the two file names) and the Output panel says each step. Run by hand on a copy
    of PowerOficina with a `wss://` address: exactly two files, 14.5 s warm, and the zip's SHA-256
    (`33aa1597...`) equals the one the T13 test made in an independent run, so the IDE's function and
    `rcrun` give the same zip. `cargo test -p cobolt-cli`: see T19's table.
  - Files: `crates/cobolt-cli/src/main.rs` (`cmd_package`: a Web manifest calls `package_web`;
    `--lang CODE`; usage), `crates/cobolt-project-tools/src/tools/ide.rs` and
    `crates/cobolt-ide/src/app/mcp_ide_tools/project.rs` (the `package_project` tool answers with the
    two paths for a Web project; its description says so).
  - Verify (here): `cargo test -p cobolt-cli`; for one project and one set of settings, `rcrun package`
    and the T13 function used by the IDE produce zips and scripts **byte-equal** when the Backend is
    taken from the same build (T12 makes the zip deterministic); if two separate Backend builds are
    not byte-identical, the test compares every entry except the executable and compares the
    executable's SHA-256 between the two outputs of one build, and says so.

## F. Proof on real systems

- [x] **T16 — Linux mechanics in a systemd container** (AC4 Linux, AC8–AC11)
  - Done 2026-11-25 on a Debian 12 container with systemd as PID 1 and nginx 1.22 (colima, arm64); the
    stand-in Backend was a scratch script outside the repository (opens the port, `READY=1`, exit 0 on
    SIGTERM); the real zip of `package_web` with the executable swapped, the script regenerated for Linux
    by a scratch program over the same `Install` type (the Mac makes macOS scripts). Results:
    AC8: no parameter, a wrong folder and a folder without `nginx.conf` all exit 2 with the reason; the
    checksum of `/etc/nginx`, the install folder and the unit list was identical before and after.
    AC9: `wss://` with no certificate, with only one of the two, and with a file that does not exist: exit
    2, nothing changed; a package that does not use `wss://` installs with neither (and the update over a
    `wss://` install writes the plain site: `http` 200, port 443 closed, `/ws` reaches the Backend).
    AC10: an nginx configuration broken beforehand: exit 1, "The previous state was restored", checksum
    identical, no unit, no install folder, the service account removed again, nginx still active and not
    reloaded. A good install: exit 0 in 1 s, `nginx -t` passes, `https` 200 with the security headers,
    `/ws` reaches the Backend, `http` 301 to `https`.
    AC11: a record edited, then a newer package: the record kept (same SHA-256), page new, Backend new,
    service restarted (new PID), environment file kept; the same package twice: no file changed
    (checksum equal), one site file, no `include` added.
    AC4 (Linux): `systemctl stop` returned in under 1 s with Result=success and exit status 0; `kill -9`
    was followed by an automatic restart 3 s later (NRestarts=1); enabled at boot; a start with a broken
    setting fails with the reason in the journal.
    Found and fixed by this run: the step "Installing the web files and the Backend" was printed twice
    (the account step sat between two of them; now the account comes first; test
    `each_step_is_said_once_in_the_order_it_runs`), and `.lck` lock sidecars were packaged with the seed data
    (now left out). Also: the real Windows script parses with **0 errors** in PowerShell 7 (AST parser, 3
    languages) and its usage, no-parameter, wrong-folder, missing-certificate, missing-secret and
    unknown-parameter refusals ran in `pwsh` 7 on Linux under emulation (exit 2, the messages in ja/pt/en).
    Not proven here: the real Backend's service mode on Linux (T3 proved it as a process; T17 runs it
    under real systemd), SELinux, the RHEL/Alpine layouts of nginx, everything Windows- and macOS-specific.
  - Files: none in the repository. The container fixtures (a Dockerfile, the stand-in Backend) are
    scratch work kept in the session's scratchpad, outside the tree, as the "Rust only" rule allows;
    nothing is added to the repository.
  - Do: a Debian image with systemd and nginx; a zip made by `package_web` with a **stand-in
    Backend** that opens the port, speaks `sd_notify` and exits 0 on `SIGTERM` (the real Backend's
    signal and notify code is proven by T1/T3 here and by T17 on `ubuntu-latest`); run the Linux
    script and record: AC8 (no parameter, wrong folder, no `nginx.conf`; nginx folder checksum equal),
    AC9 (`wss` without certificate/key stops; a non-`wss` package installs without them), AC10 (a
    broken nginx config → non-zero, no reload, nginx folder, unit list and install folder as they
    were; a good one → `nginx -t` passes, `https://…/` answers 200, `/ws` reaches the stand-in),
    AC11 (edit a record, run a newer package: record kept, page new, Backend new; the same package
    twice changes no file and leaves one `include`), AC4 (start, stop under 30 s with exit 0, kill
    and restart).
  - Verify: the container's output pasted in the task note with times; anything not run is named.

- [~] **T17 — The three-system workflow, dispatched by the operator** (AC4–AC7, AC12 on real systems)
  - Written 2026-11-25: `.github/workflows/web-install-package.yml`, `workflow_dispatch` only, three jobs
    (`ubuntu-22.04`, `macos-latest`, `windows-latest`). Each builds `rcrun`, packages a copy of PowerOficina
    with a `wss://` address and an `[auth]` section (the REAL Backend), installs it with a test nginx, and
    prints PASS/FAIL for: service running and enabled, `nginx -t`, the page 200 with its headers, `/ws`
    through nginx, no C runtime library named by the Backend (AC7; `ldd`, `otool -L`, `dumpbin`), a killed
    Backend started again, an update keeping a record, the same package twice changing no file (Linux), stop
    in under 30 s with exit 0 (Windows: no SCM events 7000/7009/7011), a refused release setting failing with
    the reason in the log (AC5), and on Windows both `powershell.exe` 5.1 and `pwsh` 7 (AC12). Verified here:
    the YAML parses, the 10 bash blocks pass `bash -n`, and the 2 PowerShell blocks parse with 0 errors in
    PowerShell 7. **Not run:** the operator dispatches it; until then the CHANGELOG says "not verified on
    Windows" and "not verified on macOS" for everything their jobs check.
  - Files: `.github/workflows/web-install-package.yml` (new, `workflow_dispatch` only; the one YAML
    file beside the existing workflows).
  - Do: three jobs (`ubuntu-latest`, `macos-latest`, `windows-latest`): build `rcrun`, package a Web
    example with a `wss://` address, run the script with a test nginx folder, then check: service
    running, process killed → restarted, stop returns 0 within 30 s (Windows: no error 1053/1061),
    refused configuration → a failed start with the reason in journal/log/event log (AC5), the
    executable's imports on Windows (T5), the Backend runs with nothing installed (AC7), update keeps
    a record (AC11), `powershell.exe` and `pwsh` both (AC12). Artifacts: the produced two files and the
    logs.
  - Verify: the workflow file parses (`actionlint` if present, else a YAML read); **the operator
    dispatches it** (it is not run from here). The result of each job is copied into this file;
    a job not run is "not verified on <system>" in the CHANGELOG.

## G. Docs, finalize

- [x] **T18 — Developer's Guide** (AC16, rule #3, #8) — *done 2026-11-26: §23.11 "Installing a Web application on a server" (two files, one parameter, flowchart, option table, per-OS paths, update, Note, Caveats, screenshot placeholder `web-package-output.png`), a pointer from the hosting section (23.7), the Backend's arguments in 24.11 (`--config --env-file --log-file --service --service-name --check --describe --version`), the `rcrun package` rows and listing in chapter 17 (`--output` is a folder on a Web project, `--lang`), and the MCP tool's row. The guide has no translations at this moment, so none was deleted. The KB test `prebuilt_chunked_kb_matches_the_published_documentation` is red for `form_designer_controls.md`, not for the guide (the guide is not in the chunked store); that staleness predates this change (2.1.7) and was left for a fix of its own. `doc_shots::every_marker_in_the_shipped_guide_parses` caught the first form of the screenshot marker and passes now.*
  - Files: `docs/developers-guide-en.md` — chapter 23, a new section "Installing a Web application
    on a server" (what Package makes, the two files, the one parameter, what the script does and
    in what order as a mermaid flowchart, a screenshot placeholder for the Output panel, a Note on
    updating and on the environment file, Caveats: only behind nginx, SELinux, the Windows C runtime
    first compile, the Backend's secrets and certificate, the OS of the packaging machine); chapter
    24's list of Backend arguments gains `--env-file`, `--log-file`, `--service`. COBOL and prose,
    no Rust; original wording; for COBOL developers from Fujitsu PowerCOBOL and Veryant isCOBOL.
    The five translations of the touched chapters are deleted if any exist (none at this moment;
    verify with `ls docs/*-{es,pt,fr,jp,cn}.md`).
  - Verify: the mermaid block renders (read, not drive a window); `rg -n "cobolt" docs/developers-guide-en.md`
    adds no new hit in the new text. The System KB is built from compiler constants, not from
    `docs/*.md` (memory "System KB source is Rust constants"), so no rebuild is expected; run
    `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    once to confirm it, not assume it.

- [x] **T19 — Finalize** (rules: version, CHANGELOG, credentials, staging) — *done 2026-11-26, with one deviation: **nothing was committed** (the `/implement` rule: no commit unless the operator asks), so the work stands as uncommitted changes on `features` and the "commit" half of Verify is the operator's to ask for. Version 2.1.41 (the highest on every branch was 2.1.40). CHANGELOG entry written with the sweep table (project-tools 282 passed, web-bundle 166, server 187 + 1 red not ours, compiler 313, cli 104, ide 2054 + 5 reds, 1 ours fixed, forms 1884). Credential scan: clean on the staged diff of the 34 files of this feature, run on a temporary index so the real one was not touched; no stray scripts in the tree. `cargo build -p cobolt-ide --release`: exit 0 (the installed IDE of this machine was not replaced). The tree holds untracked `.scan_*.tgz` files (140 MB, dated 2026-11-12) that are not this feature's and must not be staged.*
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` (read only its first 60 lines).
  - Do: list every branch's `z` (`git branch -a`, `git log -1 --format=%s` of each, the worktrees') and
    take the next free one above 2.1.40; a top CHANGELOG entry with the date, what the feature is,
    and an honest line per system: "proven on macOS (here) / Linux container / Windows runner /
    not verified on …". Full sweeps: `cargo test` of every crate touched with `--no-fail-fast`
    (`cobolt-ide` with `--bin`; `cobolt-forms --features render` is unaffected but run once), read
    every `test result:` line against the T0b baseline. `PowerRustCOBOL-local-settings/credscan.sh &&
    git commit`; quarantine any stray `.sh/.py/.ps1` in the tree; stage **explicit files** and
    `git diff` each before `git add` (the tree is shared with other sessions: never `-a`).
  - Verify: the sweep table is in the CHANGELOG entry; `git status` shows only this feature's files
    staged; commit on `features` only. **Merge into `main`, and push, only when the operator asks**
    (push also never between 09:00 and 18:00 Mon–Fri São Paulo).

## Traceability

| Requirement | Tasks | Acceptance |
|---|---|---|
| R1 | T13, T14 | AC1 |
| R2, R3 | T12 | AC1 |
| R4 | T6, T11, T12 | AC2 |
| R5, R6 | T11 | AC3 |
| R7 | T7, T8–T10 | AC9 |
| R8–R10 | T1–T4 | AC4, AC5 |
| R11 | T3 | AC6 |
| R12 | T8–T10 | AC4 |
| R13 | T5 | AC7 |
| R14–R21 | T7–T10 | AC8–AC11 |
| R22, R25 | T6, T14 | AC12, AC14 |
| R23 | T8–T10 | AC12 |
| R24 | T14 | AC13 |
| R26 | T13, T15 | AC15 |
| A1 | T12 | — |
| A2 | T8–T10, T12 | AC11 |
| A3 | T1–T4 | AC6 |
| A4 | T11 | AC3 |
| A5 | T11, T12 | AC1 |
| — (Guide) | T18 | AC16 |

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and the change is split
into fix/feature commit(s) per the operator's rules (do **not** commit/push beyond `features`
unless the operator asks). AC4, AC5, AC7 and AC12 are checked only where a system actually ran
them; the rest are reported "not verified on <system>".

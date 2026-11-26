# Spec — Install package for a Web project: the Backend as a native service, installed into nginx by one script

- **Status:** approved (operator, "Go", 2026-11-25); amendments A1–A4 and the answers to Q4–Q10 below
  come from `/plan`
- **Folder:** specs/113-web-install-package/
- **Author:** Claude (for the operator)   **Date:** 2026-11-25

## 1. Overview

Today **File ▸ Package Project** zips a project's sources, assets and a `run.sh` / `run.bat`
(`package_project` in `project_model.rs`). For a **Web** project that zip installs nothing: the
page is a static bundle under `dist/web/`, the Backend is a separate executable, and putting them
on a server is a manual job of copying files, writing an nginx site, and keeping a console
program alive.

This feature changes that function **for Web projects only**. Packaging becomes a process that
builds what is to be installed (the complete Web Build and the Backend executable) and writes
**exactly two files**:

1. a **zip** with everything the running application needs and nothing else;
2. an **install script** for the operating system of the machine that packaged (a shell script on
   Linux and macOS, a PowerShell script on Windows).

The script takes the **nginx folder** as its one required parameter. It installs the site into that
nginx, installs the Backend as a **native service** of the operating system (systemd, launchd,
the Windows Service Control Manager), starts it, and reloads nginx. For now nginx is the only
supported web server.

## 2. Goals / Non-goals

- **Goals**
  - One menu action from a Web project to a deployable pair of files.
  - The Backend binary behaves as a service of each of the three systems, natively: it answers
    that system's own start, stop and restart protocol.
  - An install that needs no manual editing on the server, and no network.
  - Running the script again with a newer package **updates** the installation without losing
    data, secrets or the certificate.
  - A failed install leaves the machine as it was.
- **Non-goals**
  - Web servers other than nginx (Apache, IIS, Caddy …).
  - A package for another operating system than the one that packaged: the Backend is compiled
    for the host only (spec 041). To serve from Windows, package on Windows.
  - An uninstall option (decided 2026-11-25): the developer's data is never removed by a script.
  - Installing nginx itself, opening firewall ports, issuing or renewing certificates.
  - Docker, ECR, ECS and API Gateway: the Web Build keeps writing its `hosting/deploy/` kit as
    today; this package does not carry it.
  - Desktop and Mobile projects: **Package Project** behaves for them exactly as it does today.
  - More than one instance of the same project on one machine.

## 3. User stories

- As a **COBOL developer** with a Web project, I want to choose Package Project and get two files,
  so that I can hand them to whoever runs the server.
- As a **server operator** on Linux, macOS or Windows, I want to run one script with the path of
  nginx, so that the site is served and the Backend runs as a service that starts at boot and
  restarts after a failure, with no step I have to remember.
- As an **operator** receiving a new version, I want to run the new script over the old
  installation, so that the application is updated and the records people entered are kept.

## 4. Requirements (EARS)

### Packaging

- **R1 (event):** When the developer chooses **File ▸ Package Project** on an open Web project,
  the IDE shall ask where to save, then run the complete Web Build and the Backend build on a
  background thread, assemble the package, and report progress and the result in the Output
  panel without blocking the IDE.
- **R2 (ubiquitous):** The packaging shall write **exactly two files** into the chosen folder:
  `<name>.zip` and a script named after it, `<name>-install.sh` (Linux, macOS) or
  `<name>-install.ps1` (Windows), for the operating system of the machine that packaged.
  Nothing else is left in that folder, on success or on failure (temporary work is done elsewhere
  and removed).
- **R3 (ubiquitous):** The zip shall hold what the running application reads and nothing else:
  the web bundle **without** its `hosting/` kit, the Backend executable, the production
  `backend.toml`, the project's `data/` folder, and the Apache-2.0 notices the existing package
  already carries. It shall hold none of the project's forms, sources, generated code, agent
  folders, Knowledge Base or debug files.
- **R4 (ubiquitous):** The zip and the script shall hold **no credential**. A secret the Backend
  reads is named in `backend.toml` as `env:NAME` or `file:PATH`; the package records which names
  are required, and the script takes their values from an optional environment file (R15).
- **R5 (event):** When the project's settings cannot give a Backend that a release start accepts
  (no `allowed_origins`, no `[auth]` issuer and audience or `[identity]`, an `unsafe_…` setting,
  an empty `backend_url` where the page needs one), the packaging shall stop **before writing
  any file** and name the setting and where to set it.
- **R6 (ubiquitous):** The page's Backend address (`backend_url`) and the Backend's
  `allowed_origins` shall come from the project's settings, so that the script needs no
  parameter to know either.
- **R7 (optional):** Where the project's Backend address is `wss://`, the script shall require a
  certificate and a private key; where it is not, it shall ask for neither.

### The Backend as a service

- **R8 (ubiquitous):** The Backend executable shall run as a native service of Linux (systemd),
  macOS (launchd) and Windows (Service Control Manager), answering each system's start, stop
  and restart.
- **R9 (event):** When the operating system asks the service to stop, the Backend shall stop
  accepting connections, let the calls in progress end within a bounded time, close its files
  and exit with code 0; on Windows it shall report stop-pending and then stopped, so that
  the Service Control Manager reports no error.
- **R10 (event):** When the Backend starts as a service, it shall report **running** to the
  system only after it listens, and when its configuration is refused it shall report the failure
  with the reason in the place that system keeps service logs, and exit non-zero.
- **R11 (ubiquitous):** The Backend shall keep running from a terminal exactly as it does today
  (`<name>-backend --config backend.toml`, `--check`, `--describe`); `rcrun backend run` and the
  Preview are unchanged.
- **R12 (ubiquitous):** The service shall start at boot, shall be restarted by the system after a
  failure, and shall run under a **dedicated, unprivileged account** that can write only the
  installation's `data/` and `logs/` folders.
- **R13 (constraint):** The Backend executable shall run on a clean installation of the operating
  system, with no Rust, no C++ redistributable and no other runtime installed.

### The install script

- **R14 (ubiquitous):** The script's **one required parameter** is the nginx folder. Both layouts
  are accepted: a folder with `conf/nginx.conf` (nginx for Windows, a build from source) and a
  folder with `nginx.conf` (`/etc/nginx`, Homebrew's `etc/nginx`). With the parameter missing,
  or the folder holding no `nginx.conf`, the script shall print its usage, change nothing and
  exit non-zero.
- **R15 (optional):** Where given, the script shall accept: the **installation folder**; the
  **server name** (default: the host of `backend_url`); the **certificate and key** (PEM) for R7;
  an **environment file** of `NAME=value` lines for the Backend's secrets. When a required secret
  name (R4) is missing from it, the script shall stop before changing anything and list the names.
- **R16 (event):** When run, the script shall, in this order: check the operating system, the
  privileges (root or Administrator), its parameters and the tools it needs; extract the zip that
  sits beside it; write nginx's site and make nginx read it; install and start the service; wait
  until the Backend answers; run nginx's own configuration test; and only then reload nginx.
- **R17 (ubiquitous):** The script shall make **one** change to the nginx configuration it does
  not own — a single `include` line, added once — after saving a copy of what it changes; the
  site itself is a file of its own.
- **R18 (event):** When any step after the first change fails, the script shall restore what it
  changed (the saved nginx configuration, the previous site and service when there was one),
  report the failing step, and exit non-zero.
- **R19 (state):** While the application is already installed, running the script of a newer
  package shall **update** it: replace the web files, the Backend executable and the site, restart
  the service, and **keep** the files in `data/`, the environment file, the logs and the
  certificate. A file of `data/` that exists on the server shall never be overwritten by the zip;
  the zip's `data/` only seeds a first install.
- **R20 (ubiquitous):** Running the script twice with the same package shall leave the same state
  as running it once.
- **R21 (ubiquitous):** The script shall use only what a default installation of its system has,
  plus `unzip` on Linux and macOS, and shall name any missing tool before changing anything. It
  shall not use the network.
- **R22 (ubiquitous):** The script's messages shall be in the language of the IDE at the time of
  packaging (the six languages of the product), English where that language has no text.
  Service names, file names and the COBOL names stay as they are.
- **R23 (ubiquitous):** At the end the script shall print the address, the service's name and the
  one command that shows its state and the one that shows its log.

### The IDE, `rcrun` and MCP

- **R24 (constraint):** For a Desktop or Mobile project **Package Project** shall behave as it
  does today.
- **R25 (ubiquitous):** The IDE's text for this function shall be a `Tr` field in the six
  languages; no user-facing text shall say "cobolt".
- **R26 (ubiquitous):** `rcrun package` and the MCP tool `package_project`, given a Web project,
  shall produce the same two files as the menu.

## 5. Acceptance criteria

- [ ] AC1 (R1, R2, R3) — Packaging `PowerOficina` (with a `wss://` address set) from the IDE
  leaves exactly two files in the folder; `unzip -l` lists `web/`, `backend/` and `data/` and no
  `hosting/`, `forms/`, `src/`, `generated/`, `agentic_ai/` entry.
- [ ] AC2 (R4) — A scan of the zip and the script finds no key, token or password; a project
  whose `backend.toml` holds a literal secret does not package.
- [ ] AC3 (R5) — A project with no `allowed_origins` or no identity settings does not package; no
  file is written and the message names the setting.
- [ ] AC4 (R8–R12) — On Linux with systemd, macOS and Windows: after the script, the service is
  running, survives a reboot, comes back after its process is killed, and `stop` returns without
  error within 30 s with exit code 0 (Windows: no error 1053/1061).
- [ ] AC5 (R10) — With a refused configuration the service does not stay "starting": the system
  reports a failed start and the reason is in the journal, the log file or the event log.
- [ ] AC6 (R11) — `<name>-backend --check` and a terminal run behave as before; the existing
  `cobolt-server` and `cobolt-compiler` tests stay green.
- [ ] AC7 (R13) — The Backend starts on a clean Windows runner image and a clean Linux image with
  nothing installed beside the system.
- [ ] AC8 (R14) — With no parameter, a wrong folder, or a folder without `nginx.conf`: usage is
  printed, exit code is non-zero, and a checksum of the nginx folder before and after is equal.
- [ ] AC9 (R7, R15) — A `wss://` package without certificate or key stops before any change and
  names both parameters; a package whose address is not `wss://` installs without them.
- [ ] AC10 (R16, R17, R18) — With a deliberately broken nginx configuration the script exits
  non-zero, nginx is not reloaded, and the nginx folder, the service list and the install folder
  are as they were; with a good one, `nginx -t` passes, `https://<name>/` answers 200 with the
  page and the `/ws` handshake reaches the Backend.
- [ ] AC11 (R19, R20) — After editing a record on the server, running the script of a newer
  package keeps the record, serves the new page and runs the new Backend; running the same
  package a second time changes no file and leaves one `include` line.
- [ ] AC12 (R21, R22) — The script runs on the stock shell of macOS and of a minimal Linux
  image and on Windows PowerShell 5.1 and PowerShell 7; its messages follow the IDE language in
  all six languages.
- [ ] AC13 (R24) — Packaging a Desktop and a Mobile project gives the same result as before this
  change (the existing tests).
- [ ] AC14 (R25) — Every new `Tr` field has all six languages (the existing i18n tests); no
  generated text contains "cobolt".
- [ ] AC15 (R26) — `rcrun package` and the MCP tool give files byte-equal to the menu's, for the
  same project and settings.
- [ ] AC16 — The Developer's Guide has the new behaviour (chapter 23, with a flowchart, a Note and
  the Caveats), in COBOL and prose with no Rust.

## 6. Constraints & steering check

- **i18n:** yes — the IDE's progress and error text, and the script's messages (R22), in EN ES PT
  JA ZH FR.
- **Generated-code / regenerate contract:** none for `.cbl`. The Backend executable gains a service
  mode in `cobolt-server`'s standalone entry; it runs the native runtime, so the WebAssembly
  generator is not affected (to be confirmed in `/plan`).
- **System KB:** no control, property, method or event changes; the tables stay as they are
  (confirm in `/plan`).
- **Repository rule "Rust only":** the install scripts are **text written by Rust code** (as the
  deploy kit's scripts are in `deploy.rs`); no `.sh` or `.ps1` file is stored in the repository.
- **Developer's data is sacred:** the script and the update never overwrite or delete a file of
  `data/`, and there is no uninstall that removes it.
- **Docs:** the English `docs/developers-guide-en.md` is updated in the same change; the
  translations are deleted per rule #8 (none exist at this moment).
- **Version / branches:** a **feature** (a capability beyond today's scope): branch `features`,
  its own commit and push, never mixed with a fix; bumps `z` with a `CHANGELOG.md` entry; the
  `z` is chosen after checking every branch (2.1.21–2.1.39 are used by
  `features-windows-install-prep`; 2.1.40 by the nginx fix on `fixes`).
- **Verification:** this machine is a Mac. macOS is proven here; Linux with systemd and Windows
  are proven on a VM or a runner, as spec 112 does. A claim not run on a system is written as
  not verified on it.

## 7. Open questions

- Resolved 2026-11-25 (operator): the package is for the operating system of the machine that
  packages (Q1); the certificate and key are required only when the project uses `wss://` (Q2);
  re-running the script updates, and there is no uninstall option (Q3).
- **Q4:** Script messages in the IDE's language (R22) or English only? The spec assumes the IDE's
  language, as the deploy kit's README does; it costs six sets of about twenty messages.
- **Q5:** Where the service writes a log **file** (Windows, macOS), should the Backend rotate it
  by size? The spec does not require it yet; an unbounded log on a server is a defect waiting.
- **Q6:** Which files does a running Backend read besides `data/`? Checked in the source: a
  Backend that is not given an application folder resolves every relative path (`data/…`, the
  Knowledge Base's `assets/KB`, the key store) against its **working directory**, and reads
  `backend.toml` from there. So the service must start in the Backend's own folder, which holds
  `backend.toml` and `data/`. R3 lists what is certain; `/plan` settles whether `assets/KB` and
  the key store are part of a Web project's Backend and so belong in the zip.
- **Q7:** Windows without the C++ redistributable (R13) probably means linking the C runtime
  statically for the Backend only; `/plan` checks it against the SDK's build and the SBOM.
- **Q8:** On macOS the service is a **LaunchDaemon** (system-wide, starts at boot, installed as
  root), not a per-user agent. Confirm.
- **Q9:** If the Backend's port (8443 by default, from `backend.toml`) is taken on the server,
  the spec says the script stops and names the port; it does not pick another.
- **Q10:** R26 (`rcrun package` and the MCP tool) is cheap if the work lives in one library crate;
  if `rcrun package`'s older manifest format makes it costly, it moves to a follow-up.

### Answered by `/plan` (2026-11-25; the operator may overrule any of them at the plan gate)

- **Q4:** the script's messages are in the IDE's language, six sets of about twenty, English where
  the code is unknown (plan 3.4).
- **Q5:** yes. The Backend rotates its own log file at 10 MB, keeping five (plan 1.A).
- **Q6:** the zip carries `data/` and, when the project has it, `assets/KB/`. Nothing else is read
  from disk (plan 1, "Q6 settled").
- **Q7:** the C runtime is linked into the Windows Backend only, in its own staging folder (plan 4,
  5 R-d); if it cannot be linked the task reports it and nothing is guessed.
- **Q8:** confirmed: a LaunchDaemon, installed by root, started at boot.
- **Q9:** confirmed: the script stops and names the port; it never picks another.
- **Q10:** inside the feature. The work is one function in `cobolt-project-tools`; `rcrun package`
  for a Web manifest and the MCP tool call it (plan 2).

### Amendments

- **A1.** A Web project with no Backend gets the same two files; the script installs the site only
  and creates no service (extends R1/R14).
- **A2.** The script verifies the zip's SHA-256 against the value it was written with and refuses a
  zip that does not match (extends R14).
- **A3.** The Backend gains three optional arguments, `--env-file`, `--log-file` and, on Windows,
  `--service` / `--service-name`; a plain terminal run is unchanged (extends R8–R11).
- **A5.** R3's "Knowledge Base" means the **Project** KB (design time) and the System KB, which stay
  out. The **Application** KB, `assets/KB/`, is read by a Backend that uses a `KnowledgeBase`
  control, so it ships in the zip as `backend/assets/KB/` when the project has one, and the script
  seeds it like `data/`. AC1's list of entries that must be absent does not include it. The
  Backend executable already carries its programs and exports inside it (`standalone::main` takes
  the bundle as bytes), so no `exports.toml` ships; T11 proves this by running the packaged
  Backend from a folder holding only what the zip holds.
- **A4.** The `backend.toml` in the zip is made from the project's own, with the origin of the
  address added to `allowed_origins`, and is checked by running the packaged Backend with `--check`
  before the zip is written (extends R5, R6).

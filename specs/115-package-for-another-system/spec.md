# Spec — Package a Web project for another system: a Linux server, from a Mac or a Windows machine

- **Status:** implemented on 2026-11-27 (2.1.77); AC11 and AC12 are **not verified** until the operator dispatches the two workflows (T23)
- **Folder:** specs/115-package-for-another-system/
- **Author:** Claude (for the operator)   **Date:** 2026-11-27

## 1. Overview

**File ▸ Package Project** on a Web project (spec 113) makes exactly two files: a zip of what the
running application needs and an install script that sets up nginx and registers the Backend as a
service. It is made **for the system of the machine that packages**. Checked on 2026-11-27 with a
real `rcrun package` of PowerOficina on the operator's Mac (4.6 s): the zip's `bin/poweroficina-backend`
is a Mach-O arm64 executable and the script says `OS=macos` and uses launchd. A package made on that
Mac cannot be installed on a Linux server, and a server is almost always Linux. Specs 113 and 114
left this out on purpose (113 non-goal; 114 Q3: the module is portable, but the IDE ships only its own
platform's runner).

The pieces that are specific to a system turn out to be few. The Backend of a Web project built the
default way is **the application's module (portable) joined to a prebuilt runner**
(`prc-backend-runner`, 37,580,112 bytes in release on the Mac) by one function
(`cobolt_program_package::embed`); the install script is already generated per system from facts, in
all six languages (`TargetOs` in `cobolt-web-bundle`), with the Linux branch proven in a Debian 12
systemd container (with a stand-in Backend, not the real one). What a Mac lacks is **the Linux runner**,
a way to **choose the system**, and a **Prove** step that does not depend on running the target's
executable on the packaging machine.

This feature lets the developer choose the system the package is for, and makes the package for
**Linux** from any of the three systems the IDE runs on.

## 2. Goals / Non-goals

- **Goals**
  - A Web project packaged on macOS (or Windows) for a Linux server, with the same two files, the
    same contents and the same script as if it had been packaged on that Linux.
  - The same choice from the three entry points: File ▸ Package Project, `rcrun package`, the
    coding agent's `package_project` tool.
  - Never a package with a runner that was not verified.
  - A package for the system the machine runs stays **byte for byte what 2.1.75 makes**.
- **Non-goals**
  - Desktop and Mobile projects: Package Project keeps writing the zip of their sources for now (the
    other gap the 2026-11-27 audit found; its own spec).
  - A target of macOS from Windows or Linux: a macOS executable carries a code signature (the release
    workflow signs the runner ad hoc), which only macOS can make and check, and how it behaves once
    the payload is joined was not examined for this spec. A target of Windows: not asked for. The
    mechanism is the same, so either can be added by naming another system, in its own spec.
  - Compiling a Backend for another system through Cargo (EXEC RUST, External Crates, anything the
    Build sends to the Cargo path): such a Backend is packaged on the system it will run on.
  - Running the target's executable on the packaging machine: Prove proves the settings and the
    module, not the executable (R6).
  - Web servers other than nginx, uninstall, certificates: unchanged from spec 113.

## 3. User stories

- As a **COBOL developer on a Mac**, I want to package my Web project for the Linux server it will run
  on, so that I do not need a Linux machine to make the two files.
- As a **server operator** receiving them, I want the Backend in the zip to be a program my Linux can
  run, and the script to be the one for my system, so that the install is the same as in the guide.
- As a **developer in a company that audits what runs on servers**, I want the program in the package
  to be one the release published and I can check by its hash, so that nothing unverified is
  installed on a production machine.

## 4. Requirements (EARS)

- **R1 (optional):** Where the open project is a Web project, the developer shall be able to choose
  the system the package is for. The systems offered are listed in one place and are, in this spec,
  `linux-x86_64` and, if Q2 is answered yes, `linux-aarch64`, plus the system of the machine that
  packages. The default is the system of the machine: no choice means today's behaviour.
- **R2 (ubiquitous):** The choice shall be the same at the three entry points: a choice in the
  Package Project flow of the IDE, `rcrun package --target <system>`, and a `target` argument of the
  `package_project` tool, from one list of systems; a word that is not in the list is refused
  naming the list.
- **R3 (event):** When the chosen system is not the machine's, the zip's `bin/<name>-backend` shall be
  the executable of that system: its runner joined to the application's payload by the function
  the Build already uses, with the file name of that system and the mode `0755`; and
  `bin/sbom.cdx.json` shall list the libraries of **that** runner's own bill of materials and the hash of
  the executable shipped.
- **R4 (event):** When the chosen system is not the machine's, the install script shall be the script
  of the chosen system (`<name>-install.sh` for Linux), in the language chosen, and equal to the one
  the same project packaged on that system makes.
- **R5 (constraint):** The system shall make a package for another system only when the Backend is built as a
  module. Where the Build would send the Backend to the Cargo path, packaging shall refuse, naming
  each reason the Build gives and saying that such a Backend is packaged on the system it will run on;
  nothing is written.
- **R6 (event):** When a package for another system is made, the Prove step shall start the Backend with
  `--check` on the production settings using **this machine's own runner and the same payload**, and
  the Output and the tools' reports shall say what was proven (the settings, the module) and what
  was not (that the target's executable was not run here).
- **R7 (constraint):** The system shall not put into a package a runner it has not verified: the SHA-256 of
  the runner file shall be the one the release this program belongs to published for that system.
  Where the runner is absent or does not verify, packaging shall refuse, nothing is written, and the
  message says what was expected, what was found, and how to get the runner.
- **R8 (event):** *(proposed; see Q1)* When the runner of the chosen system is not on this machine, the
  IDE shall ask before fetching it from the attachments of the release, once, shall verify it (R7) and
  keep it where the next package finds it. `rcrun` and the tool never fetch on their own: a flag asks.
  A developer can always name a folder of runners instead (`--runner-dir`, an environment variable),
  for a machine with no network.
- **R9 (ubiquitous):** A package for the machine's own system shall be unchanged by this feature: the
  same zip bytes (spec 113's "same bytes twice" property holds across the version) and the same
  script.
- **R10 (state):** While the chosen system is not the machine's, the payload shall carry machine code
  for the chosen system only if the engine can make it here; otherwise it carries none, the Backend
  compiles its module when it first starts (9 to 51 ms a module, spec 114 T2) and the Output says
  which happened (see Q5).
- **R11 (ubiquitous):** Every refusal shall name the setting or the flag and what to do; nothing is
  written into the chosen folder unless the whole package succeeded (spec 113's rule, unchanged).
- **R12 (ubiquitous):** Every new text of the IDE shall exist in the six languages. The install script's
  own texts already do.
- **R13 (event):** *(proposed; see Q1)* The release shall publish, for each system offered, the runner,
  its bill of materials and a manifest of their SHA-256 hashes, and the IDE of that release shall carry
  the manifest, so that R7 compares with something the installer itself brought.
- **R14 (ubiquitous):** The Developer's Guide (sections 23.11 and 24.11, which say today that a package is
  for the machine's own system) and the System KB shall describe the choice, in the same change.
- **R15 (constraint):** The system shall not need a network on the server (as before) and shall not need
  one on the packaging machine except for the one fetch of R8.

## 5. Acceptance criteria

- [ ] **AC1 (R1 R3 R4)** On macOS aarch64, `rcrun package --target linux-x86_64 --output F` on a copy
  of PowerOficina (with a public `backend_url` and `[auth]`, as the Check demands) writes exactly two
  files; `file` on the extracted `bin/<name>-backend` answers an ELF 64-bit x86-64 executable; its
  mode is `0755`; the script has `OS=linux` and a systemd unit; the module hash in the bill of
  materials equals the one of the package made for the Mac.
- [ ] **AC2 (goal)** The zip holds no source: no `.cbl .cob .cfrm .wcfrm .mcfrm .cidx .rs` and no `src/`
  or `COPYBOOKS/` folder (the check of the 2026-11-27 audit, as a test).
- [ ] **AC3 (R3 R4; the proof the specs never had)** The package of AC1 is installed in a **Debian 12
  container with systemd and nginx** (colima, as spec 113 did with its stand-in): the script runs
  with the nginx folder, the service is active, nginx serves the page (HTTP 200 through nginx) and a
  call the application defines is answered by **the real Backend of the package**; the same script
  run again over it keeps `data/`.
- [ ] **AC4 (R9)** The package for the machine's own system is byte-identical to the one 2.1.75 makes
  for the same inputs (hash of the zip and of the script, compared).
- [ ] **AC5 (R5)** A project whose Backend needs the Cargo path is refused for `linux-x86_64` with the Build's
  reasons, and the chosen folder is as it was.
- [ ] **AC6 (R7)** With one byte of the Linux runner changed, packaging refuses and writes nothing; the message
  names the expected and the found hash.
- [ ] **AC7 (R6)** The Output of AC1 says the settings and the module were proven with this machine's runner and
  that the Linux executable was not run here; a refusal of `backend.toml` (no `[auth]`) is shown
  as it is today.
- [ ] **AC8 (R2)** One list of systems: a test makes the three entry points read the same one, and a word
  outside it is refused naming the list.
- [ ] **AC9 (R12)** `cargo test -p cobolt-ide i18n` is green with the new texts in six languages.
- [ ] **AC10 (R14)** Guide sections 23.11 and 24.11 and the System KB say the new thing; the chunked KB is
  regenerated and its freshness test is green.
- [ ] **AC11 (R3)** The Linux runner **starts on the Linux systems the Guide names**: measured in the
  containers `debian:12`, `ubuntu:22.04` and `amazonlinux:2023` (`<runner> --help`-level start); a
  system on which it does not start is named in the Guide with the reason (the glibc floor, Q3).
- [ ] **AC12 (R3 R4)** The workflow `web-install-package.yml` packages the same project for Linux on a Linux, a
  macOS and a Windows machine and checks that the executable is an ELF and the script is the Linux
  one. *(The operator dispatches it; until then the Windows and the Linux-host results are "not
  verified".)*
- [ ] **AC13 (report)** The time of a package for Linux made on the Mac is reported beside the 4.6 s of the
  package for the Mac (same project, same machine).

## 6. Constraints & steering check

- **i18n (6 languages):** yes. New texts: the choice of the system, the Output lines for the step that
  names the system and for Prove, the refusals, the fetch question. All `Tr` fields in
  `crates/cobolt-ide/src/i18n.rs`; no literal in the UI.
- **Generated-code / regenerate contract:** no change. No COBOL is generated differently; the payload of a
  Backend is the same for every system.
- **Docs:** the English Developer's Guide only, 23.11 and 24.11 (rule #3), with the table of systems, a
  mermaid diagram of what is made where, a screenshot placeholder for the choice, Notes and
  Caveats; its translations are not touched (none exist).
- **System KB:** yes. The `rcrun package` flags and the tool's argument are published
  documentation; update the compiler's doc tables and regenerate `assets/knowledge/chunked.data`.
- **Interpreter / binary parity:** none; no run-time behaviour changes.
- **Fix vs feature:** a **feature** (CLAUDE.md precedent: a capability beyond the IDE's existing
  scope, platforms). Branch `features`, and the fixes pending on `main` are pushed first (rule #5).
- **Hard rules that bind this work:** never claim Fujitsu binary compatibility; the product and
  its texts never say "cobolt"; a package never carries a secret or a source file; no credential in
  any diff; the Windows and Linux-host runs are "not verified" until the workflow has run on them.

## 7. Open questions (resolve before approval; each has a recommendation)

- **Q1 — Where does the other system's runner come from?**
  - (a) *Ship every runner in every installer.* Simple and offline, but each runner is about 37 MB:
    each installer grows by two or three of them.
  - (b) *Fetch on demand from the release's attachments, checked against a manifest of hashes that
    the installer carries.* The pattern already in use for the platform's shipped libraries
    (`warmup.rs`: `manifest.txt` travels in the installer and a downloaded replacement is checked
    against it). Needs the release workflow to build the runners first, so the manifest exists
    before the installers are made.
  - (c) *The developer supplies the runner* (`--runner-dir`): always available, for a machine with
    no network.
  - **Recommendation: (b) with (c) as the escape hatch.** One trust anchor, the installer; the
    installer keeps its size; air-gapped machines still work.
- **Q2 — Which Linux systems?** The release matrix builds `linux-x86_64`, `windows-x86_64`, `macos-aarch64` and
  `macos-x86_64`; there is **no Linux aarch64** (a Graviton server needs one: one more runner leg of
  the workflow on an arm64 runner, for the runner only). **Recommendation: `linux-x86_64` in this
  spec; `linux-aarch64` if the operator's servers are arm64.**
- **Q3 — The Linux floor.** The release builds on `ubuntu-22.04` (glibc 2.35 by that image's
  publication, not measured here). A server older than that, or Amazon Linux 2023 (glibc 2.34, from
  the distribution's publication), could not start the runner. **Recommendation: build the Linux
  runner (only it) against an older glibc or statically (musl), and let AC11 decide by measurement
  in the three containers;** the Guide names the floor either way.
- **Q4 — Where is the choice made in the IDE?** (a) A small window before the folder dialog (system,
  language of the script), remembering the last choice in the project; (b) a project setting only.
  **Recommendation: (a)**, one window, fixed size, the system's default being the machine's.
- **Q5 — Machine code for another system.** wasmtime is built with `default-features = false,
  features = ["cranelift", ...]` (`cobolt-runner-engine/Cargo.toml`), and `Target::baseline(triple)` already
  accepts a triple; by wasmtime's documentation that build compiles for the host CPU only and another
  one needs its `all-arch` feature (a larger IDE and runner, a change to `cobolt-wasm`'s dependency
  guard). *Not tried here: the first task of the plan should try it.*
  The cost of not having it is measured: 9 to 51 ms to compile the module at the Backend's first
  start. **Recommendation: no machine code for another system (R10), the Output says so.**
- **Q6 — Name of the choice.** `--target linux-x86_64` (operating system, then processor) in `rcrun` and
  the tool; the IDE shows the same words in the developer's language. **Recommendation: as written.**

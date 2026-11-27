# Tasks — Package a Web project for another system (Linux from a Mac or a Windows machine)

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Spec:** ./spec.md   **Date:** 2026-11-27

Ordered, small, independently-verifiable tasks. Each names the files it touches, the requirement(s) it
satisfies, and how to verify it. Check off as completed. Branch: `features`. The project stays green
after each task: the new code is reachable only from its own tests until T9 (the package) and T10 to
T14 (the entry points) wire it up, and `target: None` (the machine's own system) is today's path at
every step (R9).

**Where each thing can be proven.** This machine is a Mac (aarch64). *Here* = this Mac. *Container* =
a Linux container run through colima (`docker create` + `docker cp`; bind mounts from `/private/tmp`
do not work under colima), native here as **aarch64**; an x86_64 container needs Rosetta or QEMU
emulation and is slower. *Runner* = a GitHub-hosted runner the operator dispatches (T23). A claim
not run on a system is written "not verified on <system>" in the CHANGELOG and the Guide.

**A correction made at `/tasks`** (written into plan §1 and §2 as well): `System` lives in
`cobolt-compiler::backend_runners`, not in `cobolt-web-bundle`, because `cobolt-compiler` does not
depend on `cobolt-web-bundle` and the Build, the CLI, the tool and the IDE all depend on
`cobolt-compiler`. `TargetOs` stays where it is; `cobolt-project-tools` maps `System.os` to it.

**Two assumptions the tasks make, to be corrected by the first thing that disproves them.**
(1) `System::TARGETS` (what the code *can* make) names both Linux processors, `linux-x86_64` and
`linux-aarch64`; `System::offered` (what the installer's manifest *has a runner for*) decides what the
developer sees, so the release builds `linux-x86_64` only (Q2) and the aarch64 mechanism can still be
proven natively in a container here (T17). (2) `Target::baseline` for another triple needs wasmtime's
`all-arch`; T1 measures it, T6 and T7 assume the answer in D8 (no machine code for another system).

---

## A. Prerequisite, baseline and the one experiment

- [ ] **T0a — The fixes are on `origin/main` and `features` is synced** (CLAUDE.md rule #5)
  - Files: none. The four Fix commits 2.1.72 to 2.1.75 (`dcc3c18e`, `c214e07f`, `62ba9abb`, `e1537f89`)
    are pushed by the 18:07 São Paulo job (or on the operator's "go" after 18:00); `origin/fixes` is
    deleted after. Then `git rebase main` on `features` (or `git merge --ff-only main` on a shared
    dirty tree), **before the first edit**.
  - Verify: `git merge-base --is-ancestor e1537f89 origin/main` exits 0; `git status -sb` on `features`
    shows it level with `main`; no feature commit exists that holds a fix.

- [x] **T0b — Baseline, and the packager of 2.1.75 kept for AC4** (R9, AC4)
  - Done 2026-11-27: reference packages `base1` and `base2` of the PowerOficina copy are byte-identical
    to each other (same bytes twice), made by the saved 2.1.75 `rcrun`; the sweep of the four crates is
    the baseline (counts at the end of this file, once it finishes); the `cobolt-ide` baseline is judged
    against `HEAD` at T22.
  - Files: none in the repo. Outside it, in the scratchpad: a copy of `target/release/rcrun` **as it is
    now** (2.1.75) with its sibling `prc-backend-runner` and `prc-backend-runner.sbom.json` (the packager
    looks for the runner beside itself), and the PowerOficina copy of the 2026-11-27 audit (public
    `backend_url`, `[auth]`).
  - Do: (a) `df`, then `cargo test -p cobolt-compiler -p cobolt-web-bundle -p cobolt-project-tools
    -p cobolt-cli --no-fail-fast` and `cargo test -p cobolt-ide --bin cobolt-ide --no-fail-fast`;
    read every `test result:` line and write the counts and the named reds at the top of this file under
    "Baseline". (b) Package the PowerOficina copy twice with the saved 2.1.75 `rcrun`; record the SHA-256
    of the zip and of the script each time, and `unzip -l` the first. (c) State whether the zip holds the
    IDE's version anywhere (the bill of materials, `backend.toml`): if it does, AC4 compares everything
    **except** that string and says so; if it does not, AC4 is a plain byte compare.
  - Verify: the two packages' hashes are equal to each other (spec 113's "same bytes twice" property,
    on this machine, now) or the difference is named; the baseline numbers and the known reds are
    written down, not hidden.

- [x] **T1 — Machine code for another system: try it, record it** (R10, plan D8 / spec Q5)
  - Done 2026-11-27: measured and written under D8 in `plan.md`; the decision stands.
  - Files: none in the repo. A throwaway crate in the scratchpad that depends on `wasmtime` with the
    features `cobolt-runner-engine` uses plus `all-arch`.
  - Do: compile one small module for `x86_64-unknown-linux-gnu` from this aarch64 Mac with
    `Target::baseline`-equivalent settings; record whether it works, the build time and the size
    difference of a runner-like binary with and without `all-arch` (a release build of the crate that
    pulls wasmtime, compared; not of the whole runner).
  - Verify: the numbers are written under D8 in `plan.md` ("tried 2026-11-27: …"). The decision (no
    machine code for another system) **stands** unless the cost is negligible **and** `cobolt-wasm`'s
    dependency guard needs no change; if it flips, stop and surface it, do not fold it in.

## B. The system and the runner — `cobolt-compiler` (R1, R2, R7, R8, R15; AC6, AC8)

- [x] **T2 — `System`: the one list of systems** (R1, R2, AC8)
  - Files: `crates/cobolt-compiler/src/backend_runners.rs` (new), `lib.rs` (`pub mod backend_runners`).
  - Do: `System { os, arch }` with `Os::{Linux, MacOs, Windows}`, `Arch::{X86_64, Aarch64}`;
    `System::host()`; `parse("linux-x86_64")` / `Display` (operating system, then processor, ASCII,
    lower case); `exe_suffix()` (`.exe` for Windows only; no `triple()`, since no machine code is made
    for another system, D8), `TARGETS` (the two Linux systems), and `offered(manifest) -> Vec<System>` = the host,
    then each `TARGETS` entry the manifest has a runner for, no duplicates, stable order. A word not in
    the list is refused with `not a system this version can package for: "<word>"; the systems are:
    <list>` (the list is the one from `offered`, or `TARGETS` when no manifest is known).
  - Verify (here): `cargo test -p cobolt-compiler backend_runners` — every spelling in and out; the host
    is always first and always offered; an entry the manifest lacks is not offered; a bad word names the
    list; a mixed-case word is refused (one spelling only). The test ends with one result block (cases,
    times, tally).

- [x] **T3 — The manifest and the verification of a runner** (R7, AC6)
  - Files: `crates/cobolt-compiler/src/backend_runners.rs`, `crates/cobolt-compiler/src/warmup.rs`
    (`parse_manifest` and `sha256_file` become `pub(crate)`; no behaviour change).
  - Do: `RunnerManifest::read(path)`: lines `<sha256>  <system>/prc-backend-runner` and
    `<sha256>  <system>/prc-backend-runner.sbom.json` (the format `warmup` already parses); found beside
    the running program in `runners/manifest.txt`, or at `PRC_RUNNERS_MANIFEST`. `verify(system, file,
    manifest) -> Result<(), RunnerError>`: absent manifest, absent entry, and a hash that differs are
    three distinct errors, the last naming **expected, found and the file**; nothing is read into
    memory beyond what `sha256_file` already streams.
  - Verify (here): `cargo test -p cobolt-compiler backend_runners` — good file verifies; one byte
    changed → the error names both hashes; an entry for another system is not used; no manifest → "no
    runner is offered for <system>: this installation carries no list of runners"; `warmup`'s own tests
    still green (`cargo test -p cobolt-compiler warmup`).

- [x] **T4 — Where a runner comes from: folder, cache, fetch** (R7, R8, R15)
  - Files: `crates/cobolt-compiler/src/backend_runners.rs`.
  - Do: `runner_for(system, how: &RunnerSource) -> Result<RunnerFiles, RunnerError>` where
    `RunnerSource { runner_dir: Option<PathBuf>, cache_root: PathBuf, fetch: Option<&dyn Fetch> }`.
    Order: the host's own system → `find_runner` exactly as today and nothing else (no manifest, no
    cache, no fetch: D10); otherwise (1) `runner_dir` (also `PRC_BACKEND_RUNNER_DIR`), (2) the cache
    `<cache_root>/runners/<version>/<system>/`, (3) `fetch` when the caller allowed it. **Every** file —
    folder, cache or fetched — is verified (T3) before it is returned; a fetch writes `.part`, verifies,
    then renames; a failed or wrong fetch leaves no `.part`. `Fetch` is a trait (`fn get(&self, url:
    &str, to: &Path) -> Result<(), String>`) so tests substitute it.
  - Verify (here): `cargo test -p cobolt-compiler backend_runners` with an injected fetcher — a verified
    file in `runner_dir` is used and the fetcher is **not called**; an altered file in the cache is
    refused (the cache is not trusted); no file and no permission to fetch → the refusal names `--runner-dir`,
    `--fetch-runner` and the expected hash; a fetcher that returns the wrong bytes → refused, no `.part`
    left; the host's own system never touches the manifest or the cache (a test that points both at
    nothing still succeeds). One result block.

- [x] **T5 — The real fetch: `curl`, both tag spellings** (R8, R15)
  - Files: `crates/cobolt-compiler/src/backend_runners.rs`.
  - Do: `CurlFetch` runs `curl --fail --location --proto =https --tlsv1.2 --silent --show-error --output
    <part> <url>` (the arguments `warmup`'s own fetch uses) against
    `https://github.com/<owner>/<repo>/releases/download/<version>/prc-backend-runner-<version>-<system>`
    and its `.sbom.json`, then the same with `v<version>`; `<owner>/<repo>` and the version are read from
    the same constants `warmup` reads.
  - Verify (here): `cargo test -p cobolt-compiler backend_runners` — the argument list is asserted
    (`--proto =https`, `--tlsv1.2`, `--fail`, `--location`) without a network; against
    `https://127.0.0.1:1/` the error names the URL and `--runner-dir`; both tag spellings are tried in
    order. **A real download is "not verified" until the release carries the runner (T23).**

## C. Build for a system — `cobolt-compiler` (R3, R5, R9, R10; AC5)

- [x] **T6 — The module compiles its machine code only when asked** (R9, R10)
  - Done 2026-11-27: `ModuleBuild.machine_code: bool` (not an `Option<Triple>`: the Build never makes
    machine code for another system, so a switch is all the module needs); the existing tests pass with
    unchanged assertions (19 `backend_` tests); the host/None pair is proven in `tests/backend_target.rs`
    (host Build holds `Kind::MachineCode`, the target Build does not, `Kind::Backend` bytes equal).
  - Files: `crates/cobolt-compiler/src/backend_module.rs`.
  - Do: the machine-code triple `build_module` takes from `Target::host()` becomes a parameter
    (`Option<Triple>`); every existing caller passes `Some(host)`, so the module is the same bytes.
  - Verify (here): `cargo test -p cobolt-compiler backend_module backend_build` green with **unchanged
    assertions**; a new test builds one module with `Some(host)` and with `None` and shows the payload
    differs only by the machine-code section (`Kind::MachineCode` present / absent) and that the
    `Kind::Backend` bytes are equal.

- [x] **T7 — `BackendTarget`: a Backend for a system** (R3, R5, R10, AC5)
  - Done 2026-11-27: `crates/cobolt-compiler/tests/backend_target.rs`, 15 cases, 15 passed, 649 ms
    (stand-in ELF runner; Cargo refusal by `path: Some(Cargo)` and by an `[identity]` section).
  - Files: `crates/cobolt-compiler/src/backend_build.rs`, `crates/cobolt-compiler/src/lib.rs` (the new
    `CompilerError::BackendForeignRefused`, worded for packaging: *"a Backend that needs the Cargo path is
    packaged on the system it will run on; the Build's reasons: …"*).
  - Do: `BackendBuildOptions.target: Option<BackendTarget { system, runner: RunnerFiles, bin_dir }>`.
    With a target: force the module path (a Cargo reason → `BackendForeignRefused` carrying each
    reason); install into `bin_dir`, **never** the project's `bin/`; name the file for the system
    (`<name>-backend`, `.exe` for Windows); skip machine code (T6 `None`); write the runner's own
    bill of materials beside it. `target: None` is today's code, untouched.
  - Verify (here): `cargo test -p cobolt-compiler backend_build` with a **stand-in ELF runner** (any
    bytes that begin with the ELF magic, joined by `embed` as it joins a real one; its sibling
    `.sbom.json` built as the `backend_module` tests build theirs): the executable and its bill of
    materials land in `bin_dir`, the project's `bin/` is **byte for byte as before** (hash before and
    after); the name has no `.exe` for Linux; the `Kind::Backend` bytes equal the host Build's (SHA-256
    equal); `locate` on the result returns the same payload; a project with `EXEC RUST` is refused with
    the Build's reasons and `bin_dir` is empty (AC5, in its unit form). One result block.

## D. The package — `cobolt-project-tools` (R3, R4, R6, R9, R11; AC1, AC2, AC5, AC6, AC7)

- [x] **T8 — `PackageOptions` replaces the `lang` argument** (R9)
  - Done 2026-11-27. `PackageOptions` also carries `version` (the release the runner's cache and download are
    named for: the CLI reads it from `version.rs`, the IDE from `VERSION`) and `cache_root`. **AC4 measured:**
    this tree's `rcrun` (a debug build) packaged the PowerOficina copy of T0b with the saved 2.1.75 runner:
    zip SHA-256 `38624a36…c6bcf` and script `160effa2…61bc2`, **byte-identical** to the 2.1.75 packager's
    (`base1`, `base2`). The comparison of the crates' full test counts with the T0b sweep is made at T22.
  - Files: `crates/cobolt-project-tools/src/web/package.rs`, its 4 callers:
    `crates/cobolt-cli/src/main.rs` (~958), `crates/cobolt-ide/src/web_package.rs` (~46),
    `crates/cobolt-project-tools/tests/package_web.rs` (3 calls) and the unit test in `package.rs` (~823).
  - Do: `PackageOptions { lang, system: Option<System>, runner_dir: Option<PathBuf>, fetch_runner: bool }`
    with `Default` = today's behaviour; `package_web(manifest, out, &options, progress)`; the callers
    pass `PackageOptions { lang, ..Default::default() }`. No other change.
  - Verify (here): `cargo test -p cobolt-project-tools -p cobolt-cli` and `cargo test -p cobolt-ide --bin
    cobolt-ide` **match the T0b baseline exactly**; a package made with the new binary equals the one
    made with the saved 2.1.75 `rcrun` (the T0b check, run now: zip and script hashes).

- [x] **T9 — Package for another system** (R3, R4, R5, R6, R7, R11; AC1, AC2, AC5, AC6, AC7)
  - Done 2026-11-27. `tests/package_web.rs`: 8 refusal cases in every sweep (324 ms: no list, a list without
    the system, one byte changed with both hashes named, an empty folder, no permission to download, a
    failed download, an unknown release, no `[auth]`), each stopping at `Check` with nothing written and
    nothing unverified in the cache; two real builds (`--ignored`, debug build): PowerOficina for
    `linux-x86_64` on this Mac in **13.8 s** (Backend step 2.9 s, Prove 2.1 s, zip 4.9 s; the Mac's own
    package took 28.5 s in the same build because it also compiles machine code) with the ELF stand-in,
    mode `0755`, `OS=linux`, no source in the zip, the module hash equal to this Mac's own Build, the
    "not run on this machine" note; and the Cargo-path refusal (AC5) at the Backend step with the project's
    `bin/` untouched. `package.rs`: the twin holds the same payload on this machine's runner.
  - Files: `crates/cobolt-project-tools/src/web/package.rs`; `crates/cobolt-project-tools/tests/package_web.rs`.
  - Do: with `options.system` that is not the host's: (a) `runner_for` (T4); (b) the Build with a
    `BackendTarget` into a scratch `bin_dir`; (c) the **twin** for Prove: `cobolt_program_package::locate`
    the payload of the target's executable, `embed` it into **this machine's** runner, write it to a
    scratch folder, and give that to the existing `prove` (`--check --config backend.toml`); (d)
    `zip_entries` takes the target's executable, the mode `0755` and the runner's own bill of
    materials with the SHA-256 of the executable shipped; (e) `Install.os` and the script's file name
    from the system (`System.os` → `TargetOs`), `file_names(project, system)`; (f) `Packaged` gains
    `system` and `executable_run: bool` (false for another system), and `notes` gains the sentence the
    Output and the tools' reports repeat: what was proven (the settings and the module, on this machine's
    runner) and what was not (that the target's executable was run here); (g) a refusal anywhere
    leaves the chosen folder as it was (the existing `.part` discipline, unchanged).
  - Verify (here): `cargo test -p cobolt-project-tools --test package_web` with a stand-in ELF runner and
    a manifest pointing at it (`PRC_RUNNERS_MANIFEST`), for a copy of PowerOficina: **(AC1)** two files;
    `bin/<name>-backend` begins with the ELF magic; mode `0755`; the script has `OS=linux` and a
    systemd unit; the module's SHA-256 in the bill of materials equals the host package's; **(AC2)** no
    entry ends in `.cbl .cob .cfrm .wcfrm .mcfrm .cidx .rs` and none starts with `src/` or `COPYBOOKS/`;
    **(AC5)** a Backend with `EXEC RUST` is refused with the Build's reasons and the folder is as it
    was; **(AC6)** one byte of the stand-in changed → refused, nothing written, the message names
    both hashes; **(AC7)** the notes carry the "not run here" sentence and a missing `[auth]` is refused
    exactly as before; the twin's payload equals the shipped one (`locate` both). A package for the host's
    system is still the T8 bytes. One result block: cases, ms per step, tally.

## E. The entry points (R2, R8; AC8)

- [x] **T10 — `rcrun package --target`** (R2, R8, AC8)
  - Done 2026-11-27. `crates/cobolt-cli/tests/package_target.rs`: 8 cases in every sweep (4.7 s, the first
    being the cold start of the binary) and a real package through the binary (`--ignored`, 21.6 s, exit 0, the two
    files, ELF magic, `OS=linux`); the `--fetch-runner` case is kept off the network by a proxy that refuses.
  - Files: `crates/cobolt-cli/src/main.rs` (`cmd_package`, `package_web_command`, the help text at ~768 and
    the module doc at ~16 and ~35), `crates/cobolt-cli/tests/` (a new `package_target.rs`).
  - Do: `--target <system>`, `--runner-dir <folder>`, `--fetch-runner`; `PRC_BACKEND_RUNNER_DIR` read as
    the default of `--runner-dir`; a system outside `System::offered` is refused naming that list;
    `--target` on a Desktop or Mobile project is refused ("only a Web project is packaged for another
    system"); the printed lines name the system and the "not run here" sentence.
  - Verify (here): `cargo test -p cobolt-cli --test package_target` — `--target bogus` exits 1 and
    prints the list; `--target linux-x86_64` with a stand-in runner and manifest packages a scratch
    project (ELF magic, `OS=linux`); `--fetch-runner` without a network exits 1 naming the URL; the
    help text lists the three flags. One result block.

- [x] **T11 — The `package_project` tool** (R2, R8, AC8)
  - Done 2026-11-27. The tool is served by the IDE only (`rcrun mcp` forwards it), so the argument reading is
    `Choice::from_tool_args` in `crates/cobolt-ide/src/web_package.rs` (12 cases, the refusal words equal to
    `System::choose`'s, which the CLI prints) and the catalogue entry is built from `System::offered`
    (`tools/ide.rs`, 1 test). The IDE has no headless app to call the handler through.
  - Files: `crates/cobolt-project-tools/src/tools/ide.rs` (schema, description, the text a call returns),
    `crates/cobolt-ide/src/app/mcp_ide_tools/project.rs` (~381: the reply already carries `files`;
    add `system` and `executable_run`), `crates/cobolt-cli/src/main.rs` (the `rcrun mcp` side, if it
    serves the same tool).
  - Do: arguments `target` (string), `runner_dir` (string), `fetch_runner` (bool, default false); the
    tool's description lists the systems **from `System::offered`**, not from a literal.
  - Verify (here): `cargo test -p cobolt-project-tools tools` and `cargo test -p cobolt-ide --bin
    cobolt-ide mcp` — a call with `target: "bogus"` returns the same list the CLI printed; a call with a
    stand-in runner packages and reports the system; the three entry points are shown to read the one
    function (a test compares the CLI's list, the tool's list and `System::offered`: **AC8**).

- [x] **T12 — The choice remembered in the project** (R1, D11)
  - *Done 2026-11-27:* `WebSettings.package_system` (project_kind test: 30/30; an old manifest saved unchanged);
    the row is `package_system_row` in `web_settings.rs` (this computer first and stored as nothing, then each
    other offered system by its `--target` word, a stored value nobody offers is kept so it can be seen and
    changed; the row appears for a Web project only, and only when there is a choice or one is stored). The list is
    `web_package::offered_systems()`, read once. A Mobile project's `WebSettings` view stores nothing.
    `cargo test -p cobolt-ide --bin cobolt-ide -- i18n web_settings web_package`: 48 passed, 0 failed.
  - Files: `crates/cobolt-compiler/src/project_kind.rs` (`WebSettings.package_system: String`, `#[serde(default,
    skip_serializing_if = "String::is_empty")]`), `crates/cobolt-ide/src/panels/web_settings.rs`.
  - Do: the field; the panel shows it as one row (a combo of `System::offered`, "this computer" first).
    Empty means the machine's system, so a project made before keeps its file byte for byte.
  - Verify (here): `cargo test -p cobolt-compiler project_kind` — a manifest without the field loads, and
    saved again is byte-identical; one with it round-trips; `cargo test -p cobolt-ide --bin cobolt-ide
    web_settings`.

## F. The IDE (R1, R8, R12; AC9)

- [x] **T13 — The words, in six languages** (R12, AC9)
  - *Done 2026-11-27:* 14 new `Tr` fields × 6 languages = 84 texts (`webpack_started_for`, `webpack_not_run`,
    `pkgsys_title/intro/system_label/this_computer/lang_label/continue/fetch_question/fetch_yes/fetch_no/cannot`,
    `pk_set_package_system(+_hint)`). Two choices against the list above: the systems are shown by their `--target`
    words (`linux-x86_64`), the same text the command line and the tool use, so no per-system name to translate; and
    the five refusals reach the window as the `{why}` of `pkgsys_cannot` / `webpack_refused`, the text the library
    makes (the way every refusal already reaches it), so no fifth-of-six copy of each can drift from it.
    Result block: `spec 115 T12: package-system row | 30 list cases and 84 texts in 6 languages | all pass`.
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do: new `Tr` fields, each in EN ES PT JA ZH FR, none a literal: the window's title and its labels
    (system, language of the script, "this computer", "Linux (x86-64)", "Linux (ARM64)"), the Output
    lines (the system a package is for; *Prove ran the settings and the module on this machine's runner,
    not the Linux executable*; the Backend step for another system), the five refusals (system not
    offered, runner missing, runner does not verify, a Backend that needs Cargo, no list of runners), the
    fetch question and its two buttons. The COBOL names and the script texts stay as they are.
  - Verify (here): `cargo test -p cobolt-ide --bin cobolt-ide i18n` green (no empty translation, all six
    present); the new keys are counted in the result block.

- [x] **T14 — The window and the flow** (R1, R8, D11)
  - *Done 2026-11-27 (the look is the operator's to judge):* `panels/package_modal.rs` is a plain state struct
    (`PackageModal`: `new` from the remembered word, `select`, `proceed`, `answer_download`, `show`) drawn through
    `panels::fixed_dialog` (the repo's helper for a window with one width of its own, as Build Folder and the browser
    confirmation use), not through a grip: this window has nothing to drag, so it has no resize path at all.
    `app.rs`: `do_package_project` opens it before the folder dialog only for a Web project with something to choose;
    `show_package_modal` writes the choice into `[web] package_system` (saving the project only when it changed) and
    opens the folder dialog; the folder's answer starts the package with `package_pending` (language of the script
    included: `Choice.lang`). The fetch question needs to know whether a download would happen:
    `cobolt_project_tools::web::package::needs_runner_download` (one `find_runner` lookup shared with the package, so the
    question and the package can never disagree) and `twin_note`, so the Output says what Prove did and did not run in
    the developer's language while the CLI and the tool keep the English sentence.
    Results: `package_modal web_package web_settings i18n mcp` 58 passed, 0 failed (modal: 20 cases without painting;
    1416 frames over 6 languages × 2 stages with one width; 36 Output checks for another system; 12 tool-argument
    cases); `cargo test -p cobolt-project-tools --test package_web -- the_download_is_asked`: 10 cases, 10 passed.
  - Files: `crates/cobolt-ide/src/panels/package_modal.rs` (new; the window rules of
    `panels/leaderboard_modal.rs`: its own `size: Vec2`, `.resizable(false).fixed_size(size)`, one grip is
    not needed because it does not resize; laid out from its own size, never `available_width()`),
    `crates/cobolt-ide/src/panels/mod.rs`, `crates/cobolt-ide/src/app.rs` (`do_package_project`,
    `package_web_to`, the `FileRequest::PackageWebProject` flow: **window first, then the folder dialog**),
    `crates/cobolt-ide/src/web_package.rs` (the `Run` carries the options; the Output lines of T13).
  - Do: the window lists `System::offered` with "this computer" preselected (or the project's remembered
    system, T12), the language of the script, and — only when the chosen runner is not on the machine —
    the one-time question before a fetch (D5: the IDE asks, never fetches silently); a refusal is shown
    in the Output as the words of T13; the last choice is written back to the project (T12).
    The window never opens for a Desktop or Mobile project (they keep their source zip) and never
    when no system but this computer is offered **and** none was remembered (nothing to choose: today's flow).
  - Verify (here): `cargo build -p cobolt-ide` and `cargo test -p cobolt-ide --bin cobolt-ide
    package_modal web_package` — the list, the default, the remembered choice, the fetch question's
    two answers and the host-only case, **without painting**; the window's geometry test is the one the
    other modals carry. **The look is the operator's to judge; no UI driving.** After
    `cargo build --release -p cobolt-ide`, the running IDE is restarted by the operator.

## G. The release and the workflow (R13; AC12)

- [x] **T15 — The release publishes the runners and their manifest** (R13) — 2026-11-27, **with one part of the
  task not done as written: the job builds on the `ubuntu-22.04` hosted runner, not in an older-glibc container
  (T19 measured the consequence: floor glibc 2.34; T24 is the follow-up).** Checked here: the three workflow files
  parse with Ruby's YAML (outside the repo, nothing written): `build-all-platforms.yml` 6 jobs (`runners-linux`,
  `build` needs it, `release` needs `build`, plus the web-tests, build-cache-tests and web-e2e-binary jobs),
  `web-install-package.yml` 8 jobs, `publish-existing-build.yml` 1 job; the job's own commands were run by hand in
  T17 (build 1,489 s, check step passed, list and attachment names written). **Not verified: the workflow as GitHub
  runs it (T23).**
  - Files: `.github/workflows/build-all-platforms.yml`.
  - Do: a job `runners-linux` (no installer; only the runner) in a container with an older glibc (Debian 11
    as the starting point, D9) that builds `prc-backend-runner` and its `runner_sbom`, writes
    `runners/manifest.txt` (`<sha256>  linux-x86_64/prc-backend-runner`, `…sbom.json`) and uploads all
    three as an artifact; `build` `needs` it and the installers (`tar.gz`, `.msi`, `.dmg`, `.deb`,
    `.rpm`, the zips) carry `runners/manifest.txt` beside their `libs/`, with the same asserts the other
    shipped files already have (the `dist/_deb-contents.txt` and `_rpm-contents.txt` greps near lines
    1045 and 1195); the `release` job attaches `prc-backend-runner-<version>-linux-x86_64` and its
    `.sbom.json` to the release, and its "every platform contributed" check names them.
  - Verify (here): the file is read twice against the surrounding jobs and parsed by whatever YAML
    parser the machine has, **outside** the repo; the job's own commands are run by hand in a Debian 11
    container in T17 (the recipe is tried before the first real run). **The workflow itself is "not
    verified" until the operator dispatches it (T23).**

- [x] **T16 — The workflow proves it on three hosts** (AC12) — *written 2026-11-27, **not run***
  - *What was written:* `web-install-package.yml` gets `runner-linux` (builds the Linux runner and the list that names it,
    as `runners-linux` does for a release), `macos-package-for-linux` and `windows-package-for-linux` (each builds rcrun
    and its own runner, downloads that artifact, runs `rcrun package --target linux-x86_64 --runner-dir …` with
    `PRC_RUNNERS_MANIFEST`, and checks the script is the Linux one and the zip's Backend starts with the ELF magic; the
    Windows leg reads the zip with .NET because a Windows runner has no `unzip` to rely on), and `linux-from-macos` /
    `linux-from-windows` (download each package and run the checks). The checks are **one copy**: the `linux` job's
    "Install, then check" step moved verbatim into the composite action `.github/actions/web-install-check-linux`
    and the `linux` job now calls it; a script compares the two (run text byte-identical, 5089 bytes, 69 lines) and
    shows the `macos` and `windows` jobs and every other `linux` step unchanged. Parsed as YAML and every `bash` step
    through `bash -n` outside the repo: 0 failures. **Nothing here has run on a GitHub runner**: it is "not verified on
    a Windows host or on a Linux host" until the operator dispatches it (T23).
  - Files: `.github/workflows/web-install-package.yml`.
  - Do: jobs that package the same PowerOficina project for `linux-x86_64` **on macOS and on Windows
    hosts** (with `--runner-dir` and the manifest of the `runners-linux` artifact), upload the two files,
    and a Linux job that downloads them and runs the existing `linux` job's install steps over them;
    each host's job checks the zip's `bin/<name>-backend` is an ELF and the script is the Linux one.
  - Verify (here): read against the existing `linux`, `macos` and `windows` jobs; **not run**. Written in
    the CHANGELOG as "not verified on a Windows host or on a Linux host" until T23.

## H. The proof the specs never had (R3, R4; AC1, AC3, AC11, AC13)

- [x] **T17 — Build the Linux runner in a container, with the job's own commands** (R3, AC11) — 2026-11-27,
  **measured** (an `ubuntu:22.04` container of `linux/amd64` on this Apple-silicon Mac, i.e. emulated; the commands
  of the `runners-linux` job: the OpenSSL variables, `cargo build --release --locked -p cobolt-backend-runner`,
  the `runner_sbom` example, from `git archive HEAD` of the checkout at 2.1.75): built in **1,489 s**; the file is
  **47,251,960 bytes**, `ELF 64-bit LSB pie executable, x86-64, dynamically linked, interpreter
  /lib64/ld-linux-x86-64.so.2, for GNU/Linux 3.2.0, not stripped`; the bill of materials is 151,347 bytes; `ldd`
  names only `libgcc_s.so.1`, `libm.so.6` and `libc.so.6` (**no `libasound`, no `libssl`/`libcrypto`**: the runner
  has no sound and OpenSSL is inside it); the newest glibc symbol it needs is **`GLIBC_2.34`** (the versions it
  uses are 2.29, 2.30, 2.32, 2.33 and 2.34). The body of the job's *Check the runner and write the list of runners*
  step, run by hand in the container on that file, **passed**: ELF x86-64, no dynamic OpenSSL, 2.34 is not above
  the 2.35 the job asserts, the bare runner answers "this is a bare Backend runner" and exits 1, the two-line
  `manifest.txt` is written (`104b0d72…  linux-x86_64/prc-backend-runner`, `aae73870…  …sbom.json`) and the
  attachments are named `prc-backend-runner-2.1.75-linux-x86_64[.sbom.json]`. **What this does not show:** the
  workflow as GitHub runs it (T23). **The floor is 2.34, not the 2.31 D9 started from** — see T19. The runner of
  this build is the one T18 and T19 use (`…/scratchpad/runner-out/u2204-x86_64/`).
  - Files: none in the repo. `colima start` (the machine is not running now), a Debian 11 container,
    the commands of T15's job, run by hand.
  - Do: build `prc-backend-runner` and its bill of materials; record the file size, `file` (ELF and the
    processor), the highest `GLIBC_` symbol version it needs (`objdump -T` or `readelf`), and the build
    time. On Apple silicon the native container makes `linux-aarch64`; try `linux-x86_64` under Rosetta
    (`colima start --arch x86_64 --vz-rosetta`) or QEMU and **time-box it** (the build of wasmtime and
    OpenSSL under emulation may take hours): if it cannot be done in the time, report the aarch64
    result and say plainly that `linux-x86_64` was proven only by the release job's runner (T23).
  - Verify: the numbers are written down; the manifest for T18 is made from the runner just built.

- [x] **T18 — Install the package in a Debian 12 container, with the real Backend** (R3, R4, AC1, AC3)
  - **Done 2026-11-27 (x86-64 container under colima's Rosetta, on an aarch64 Mac; "the system actually run" is
    `linux-x86_64` of Debian 12, glibc 2.36, nginx 1.22.1, systemd as PID 1).** The package: `rcrun package
    PowerOficina.project.toml --target linux-x86_64 --runner-dir <T17's folder> --output <folder>` with
    `PRC_RUNNERS_MANIFEST` naming a two-line list made from the runner's own hashes (`104b0d72…`, `aae73870…`, the
    ones T17 recorded) and `PRC_BACKEND_RUNNER` naming this Mac's runner for the check of the production settings:
    **38.1 s** (the log's stamps: page at 2.8 s, Backend at 10.2 s, settings check at 16.0 s, zip at 18.3 s, script at
    38.1 s; the machine was loaded by a test sweep, load average about 12; AC13's three timed runs are still owed),
    exactly two files: the zip (27,711,230 bytes, 61
    files) and the script (22,260 bytes). The run found two things the first tries had to be given: the runner folder
    must hold `<system>/prc-backend-runner` (the folder named is the parent of `linux-x86_64/`), and a package for
    another system needs **this machine's own runner** for the settings check (the message says so and names where it
    looked). **AC1 by hand:** `bin/poweroficina-backend` is `ELF 64-bit LSB pie executable, x86-64, dynamically
    linked, interpreter /lib64/ld-linux-x86-64.so.2` (53,299,813 bytes = the runner's 47,251,960 + the payload),
    mode `0755` in the zip (made by `unx`), the script says `OS=linux` and writes `/etc/systemd/system/<service>.service`;
    **the module's SHA-256 in `bin/sbom.cdx.json` is `6821e6f8…eded` in the Linux package and in the Mac package of the
    same project: the same module**; the SBOM's hash of the executable (`a8fce3cb…`) is the installed file's hash.
    **AC3 by hand** (the script with `/etc/nginx --cert … --key …`, the page's address is `wss://`): first run **exit 0
    in 4.2 s** (steps: system, extract, account, files, service, wait for the Backend on 8443, nginx site, `nginx -t`,
    reload); `poweroficina-project` **active and enabled**, run by the service account; the installed Backend's hash equals the
    zip's; `nginx -t` passes; `https://oficina.example.com/` through nginx answers **200, 4,876 bytes in 105 ms** with the
    CSP and the other headers the site writes; `/ws` answers **101**; **a call the application defines is answered by the
    real Backend of the package**: a COBOL program (`CALL "HEALTH" WITH WEBSOCKET LINKAGE USING WS-STATUS`, run with
    this Mac's `rcrun run` as the client, through nginx on the container's published port 443, certificate trusted by its
    CA) displayed `OK OK` in **0.286 s**, and the Backend's log has `call … "program":"HEALTH","duration_ms":33.18,
    "outcome":"ok"`. **Second run** (a new certificate): exit 0 in **5.2 s**, "Updated. The records in data/ and the
    environment file were kept.", the 18 files of `data/` have **identical SHA-256** before and after (plus a marker file
    that was added), the service restarted (PID 205 → 442) and is active, one nginx site file, the installed Backend
    unchanged. **Found, not caused by this spec:** (1) a client that trusts a self-signed *leaf* certificate as its root
    (rustls refuses it) is told "the backend could not be reached" after a minute, which does not say that the TLS
    check failed (the call worked at once with a CA-signed certificate); (2) `nginx -t` prints `duplicate extension
    "wasm"` for the site file the script writes (Debian's `mime.types` already has it): a warning, the test passes. Both
    belong to spec 113's code and are reported, not fixed here. **Not shown:** a real x86-64 CPU and kernel (this was
    Rosetta), RHEL-family layouts of nginx, SELinux. Files and fixtures: scratchpad only (`t18/`), nothing in the repo.
  - Files: none in the repo (a throwaway Dockerfile or `docker run` commands in the scratchpad).
  - Do: package a PowerOficina copy on this Mac with `--target <the system of T17> --runner-dir <T17's folder>`
    and a manifest from T17 (`PRC_RUNNERS_MANIFEST`); check the zip as AC1 says (`file`, mode, `OS=linux`
    and the systemd unit, the module hash equals the Mac package's); `docker create` a Debian 12 image
    with systemd and nginx (spec 113's method), `docker cp` the two files, run the script with the nginx
    folder; check the service is active, nginx answers 200 for the page and a call the application defines
    is answered by **the real Backend of the package**; run the script a second time and check `data/`
    is kept.
  - Verify: each check's result and time written in the CHANGELOG; **AC3 is met only for the system
    actually run** (aarch64 here; x86_64 once T17's emulation or the release's runner exists, T23).

- [x] **T19 — Where the runner starts, and what it costs** (R3, AC11, AC13) — 2026-11-27: the table of where it
  starts is below; **the timing of `rcrun package` (AC13), three runs each, same PowerOficina copy, wall seconds,
  load average 2.9 before and 3.2 after (the machine was not idle):** the saved **2.1.75 release** `rcrun` for this
  Mac: **3.273, 2.810, 2.856 s** (the 4.6 s of the audit was an earlier, colder run); the **spec 115 build** of
  `rcrun` (a *debug* build: no release build of it exists yet) for this Mac: **16.983, 15.168, 15.117 s**; the same
  debug binary for `linux-x86_64` (the runner of T17, 27,711,230-byte zip against 18,466,463 for the Mac package):
  **24.302, 24.042, 24.090 s**. Only the last two lines compare with each other (same binary): **the package for
  Linux costs about 9 s more than the one for this Mac, +59 %**, spent where the log's stamps put it (joining a
  47 MB runner and writing the zip: 18 → 38 s in the first run, under load); the debug against release gap (15 s
  against 3 s) is the build profile and not this feature, so **the release figure for both is owed to T22's release
  build**. (The zips of the three lines differ in size because the Backend in each is a different runner: A's is
  the one saved beside the 2.1.75 `rcrun` (zip 29,438,906 bytes), B's is `target/release/prc-backend-runner` named by
  `PRC_BACKEND_RUNNER` (18,466,463), C's is the Linux runner (27,711,230); the byte-for-byte AC4 compare is T9's
  and base1/new1's, not these runs'.)
  - Files: none in the repo.
  - Do: start the runner of T17 (`<runner>` with no arguments exits 1 and says "bare Backend runner", the
    way the workflow's check at line ~505 does) in `debian:12`, `ubuntu:22.04`, `amazonlinux:2023` and
    the older images that name the floor (`ubuntu:20.04`, `debian:11`); record which start and, for a
    refusal, the missing symbol; time `rcrun package` for the Mac and for Linux on this machine, same
    project, three runs each (AC13: beside the 4.6 s).
  - Verify: the table (system, starts yes/no, reason) goes into the Guide's Caveats exactly as measured; a
    system on which it does not start is named with the reason; if the floor is higher than a system the
    spec wants, **stop and surface it** (D9: build against an older glibc) rather than weakening AC11.
  - **Measured 2026-11-27 (first half: where it starts; the timing of `rcrun package`, AC13, is still owed).**
    The runner of T17 (`…/scratchpad/runner-out/u2204-x86_64/prc-backend-runner`, 47,251,960 bytes, newest symbol
    `GLIBC_2.34`), copied into each x86-64 container with `docker create` + `docker cp` (colima does not
    mount `/private/tmp`, so `docker run -v` failed) and started with no arguments, as the workflow's check does:

    | System (image) | glibc of the image | Starts | What it said | Time |
    |---|---|---|---|---|
    | Debian 12 (`debian:12`) | 2.36 | **yes** | "this is a bare Backend runner", exit 1 | 137 ms |
    | Ubuntu 22.04 (`ubuntu:22.04`) | 2.35 | **yes** | same, exit 1 | 145 ms |
    | Amazon Linux 2023 (`amazonlinux:2023`) | 2.34 | **yes** | same, exit 1 | 160 ms |
    | Ubuntu 20.04 (`ubuntu:20.04`) | 2.31 | **no** | `version 'GLIBC_2.34' not found (required by /prc-backend-runner)` (also `2.32`, `2.33`), exit 1 | 94 ms |
    | Debian 11 (`debian:11`) | 2.31 | **no** | same refusal, exit 1 | 101 ms |

    **Finding (stop-and-surface, D9 / Q3):** the floor of a runner built on `ubuntu-22.04` is **glibc 2.34**,
    measured, not the 2.31 D9 expected from "Debian 11 as the starting point"; D9's own sentence ("covers Debian
    11 and 12, Ubuntu 20.04 and 22.04…") is **false for a runner built on ubuntu-22.04** and is true only if
    the `runners-linux` job really builds in an older-glibc container. T15's text asked for one; the job as
    written (`.github/workflows/build-all-platforms.yml`, line 105) has `runs-on: ubuntu-22.04` and no `container:`,
    so it builds on the hosted runner's glibc 2.35. AC11
    is **not weakened**: its three systems (Debian 12, Ubuntu 22.04, Amazon Linux 2023) start. What does not
    start is named: Debian 11, Ubuntu 20.04 and anything older (RHEL/Rocky/Alma 8, glibc 2.28, supported to
    2029). **Decision owed to the operator (recorded, not taken):** (a) keep ubuntu-22.04 and write the floor
    into the Guide (cheapest; leaves the RHEL 8 family out); (b) build the runner in an older-glibc container
    (`almalinux:8`/`manylinux_2_28`, floor 2.28) — covers RHEL 8, Debian 11, Ubuntu 20.04 — at the cost of a
    container leg in the workflow and a first run that cannot be measured here faster than ~25 min emulated.
    The long-term recommendation is **(b)**; it is a change of T15's job, not of the product code.

## I. Documentation, KB, finalize

- [x] **T20 — Developer's Guide** (R14, AC10) — 2026-11-27: 23.11 has the choice (the window, `--target`, the tool),
  the mermaid, the runner's three sources and the manifest, the refusals table, the Notes and the Caveats, and now
  **"Which Linux servers start the Backend"**: the T19 table exactly as measured (Debian 12, Ubuntu 22.04, Amazon
  Linux 2023 start; Ubuntu 20.04 and Debian 11 refused with `GLIBC_2.34 not found`), the floor named (glibc 2.34,
  the Red Hat family 8 among those left out), distributions not run said so, and the no-machine-code Caveat (9 to 51
  ms). The `docs_embed` / `doc_shots` gate runs in T22 with the rest of the IDE tests. **If T24 lowers the floor,
  this table is measured again and rewritten.**
  - Files: `docs/developers-guide-en.md` (English only; no translation exists to delete).
  - Do: 23.11: the choice, the table of systems, where the runner comes from (the folder, the cache, the
    fetch and the manifest), the glibc floor **as measured in T19**, a mermaid of what is made where
    (the Mac makes the payload and the twin; the target's runner is joined; the script is the target's),
    a screenshot placeholder for the window, Notes and Caveats (*Prove did not run the target's
    executable*; no Cargo-path Backend; no machine code, so the first start compiles in 9 to 51 ms).
    24.11: the "for the system the machine runs" sentence now says "unless you choose another". The
    `rcrun` option table and the tool's row in the agent section. COBOL and prose, never Rust; original
    wording; for developers coming from Fujitsu PowerCOBOL and Veryant isCOBOL.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide docs_embed doc_shots` shows no **new** red against the
    T0b baseline; every claim in the new text is one of the measured results above.

- [x] **T21 — System KB** (R14, AC10) — 2026-11-27: `chunked.data` regenerated (4450 records from 8 documents,
  15,806,464 bytes, was 15,785,984); `prebuilt_chunked_kb_matches_the_published_documentation` 1 passed, 0 failed.
  **Done after the fix 2.1.76 (Preview page port) reached `main` (2026-11-27, 16:57):** that fix edits the same KB
  text and regenerates the same file, so the store was built again on the tree that holds both (4,452 records from 8
  documents, **15,814,656 bytes**; the fix's store alone was 15,794,176).
  - Files: the `cobolt-compiler` doc tables that describe `rcrun package` and the `package_project` tool
    (find them with `grep -rn "package_project\|rcrun package" crates/cobolt-compiler/src`);
    `assets/knowledge/chunked.data`.
  - Do: describe `--target`, `--runner-dir`, `--fetch-runner`, `target`, `runner_dir`, `fetch_runner`; then
    `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation`
    green (a red here is real).

- [ ] **T22 — Finalize** (all)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` (top entry, absolute date; read only its
    first 60 lines).
  - Do: one `z` and one CHANGELOG entry **per commit the operator asks for**, never mixing a fix and
    a feature; the CHANGELOG states each measurement of T17 to T19, what ran on which system, and
    **"not verified on a Windows host or a Linux host"** until T23. Before any commit: the credential scan
    (`credscan.sh && git commit`), the quarantine sweep for non-Rust program files (moved, not deleted,
    each named), `git diff` of every file before `git add` (hunk-level staging; sessions share one tree).
  - Verify: the full sweeps of T0b — `cargo test -p cobolt-compiler -p cobolt-web-bundle
    -p cobolt-project-tools -p cobolt-cli --no-fail-fast`, `cargo test -p cobolt-ide --bin cobolt-ide
    --no-fail-fast`, `cargo test -p cobolt-forms --features render` — every `test result:` line read and
    compared with the baseline: **no new red**, a red that was red before is named as known;
    `cargo build --release -p cobolt-ide -p cobolt-cli` succeeds; the manual check for the operator:
    restart the IDE, open PowerOficina, File ▸ Package Project, see the window list this computer and
    Linux, make the package, read the Output (it names the system and says the executable was not run
    here).
  - **Partly done 2026-11-27 (committed 2.1.77, merged to local `main`):** the credential scan and the quarantine
    sweep ran clean before the commit; the full sweeps ran on a clean checkout of `main`, crate by crate, every
    `test result:` line read: `cobolt-compiler` **336 passed, 1 failed, 10 ignored**; `cobolt-web-bundle`
    **166, 0, 0**; `cobolt-project-tools` **288, 0, 5**; `cobolt-cli` **108, 0, 8**; `cobolt-ide --bin cobolt-ide`
    **2,087, 1, 6**; `cobolt-forms --features render` **1,884, 0, 2**. The four crates of T0b: **898 passed,
    1 failed, 23 ignored** against 883, 2, 20. **No new red:** the two reds are known
    (`reference_projects::every_reference_project_builds_on_the_webassembly_path_with_no_toolchain`, which fails on
    a clean checkout as 2.1.76 records, and `docs_embed::every_document_ships_in_every_language`, rule #8).
    **Still owed:** `cargo build --release -p cobolt-ide -p cobolt-cli` and, with it, the release-build figure of
    AC13 (the disk does not hold a release build next to the sweeps); the operator's look at the window.

- [ ] **T24 — Lower the Linux runner's glibc floor** (D9, Q3; found by T19, **recommended, not yet decided by the
  operator**)
  - Why: every binary linked on glibc 2.34 or newer asks for `GLIBC_2.34` (`__libc_start_main`), so a runner built on
    `ubuntu-22.04` cannot start on Ubuntu 20.04, Debian 11 or the Red Hat family 8 (glibc 2.28, supported to 2029),
    the family a COBOL shop is most likely to run. D9 asked for an older-glibc container; T15's job did not do it.
  - Files: `.github/workflows/build-all-platforms.yml` (job `runners-linux`), the Guide's table (T20).
  - Do: build the runner in an older-glibc container (`almalinux:8`: glibc 2.28; the same commands as T17), with the
    `node` the JavaScript actions need present in it; keep the asserts of the job's check step but make the glibc
    ceiling the new one (2.28).
  - Verify (here): the commands run by hand in `almalinux:8` under emulation (about 25 minutes, 10 GB of disk), the
    runner started in the five images of T19 **and** `almalinux:8`, the table rewritten from what that shows.
    **The workflow itself stays "not verified" until the operator dispatches it (T23).**

- [ ] **T23 — The operator's dispatch** (AC11, AC12; R13)
  - Files: none. The operator dispatches `build-all-platforms.yml` (with `publish` on, so the runner and
    the manifest are attached to a release) and `web-install-package.yml`.
  - Verify: the Actions run is read once for each job's result (the public Actions API; job logs need the
    signed-in Chrome); T5's real fetch is run against the release; AC12's three hosts and AC3 for
    `linux-x86_64` are written as verified or not; **until then they stay "not verified"**.

---

## Coverage — every acceptance criterion has a task whose verification proves it

| Criterion | Proven by |
|---|---|
| AC1 (ELF, mode, `OS=linux`, module hash) | T9 (stand-in), T18 (the real runner) |
| AC2 (no source in the zip) | T9 |
| AC3 (Debian 12, real Backend, `data/` kept) | T18; `linux-x86_64` itself at T23 |
| AC4 (host package byte-identical to 2.1.75) | T0b (baseline), T8, T22 |
| AC5 (Cargo-path Backend refused) | T7 (unit), T9 (package) |
| AC6 (one byte changed → refused) | T3, T4, T9 |
| AC7 (Output says what was and was not proven) | T9, T13, T14 |
| AC8 (one list for three entry points) | T2, T10, T11 |
| AC9 (i18n ×6) | T13, T22 |
| AC10 (Guide + KB regenerated) | T20, T21 |
| AC11 (starts on three Linux systems) | T17, T19, T23 |
| AC12 (workflow on three hosts) | T16, T23 (not verified until dispatched) |
| AC13 (timing beside the 4.6 s) | T19 |

## Done criteria
All acceptance criteria in spec.md are checked **or named "not verified" with the reason**, tests pass
with no new red against the baseline, docs and KB updated, and the change is split into fix/feature
commit(s) per the operator's rules (do **not** commit or push unless the operator asks; no push 09:00 to
18:00 Mon to Fri São Paulo).

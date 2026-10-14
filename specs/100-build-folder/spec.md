# Spec — the build folder is a setting of the machine

- **Status:** implemented in 1.91.154 (the review gates of /specify, /plan and /tasks are taken together: the request is one sentence and its facts are in the source)
- **Folder:** specs/100-build-folder/
- **Author:** operator request, written by Claude   **Date:** 2026-10-14
- **Branch:** `features` (after spec 099, whose Settings menu carries it)

## 1. Overview
The operator: *"Atualmente quando compilamos qualquer projeto em Windows, cria uma pasta em `\AppData\Local\PowerRustCOBOL`. Parametrize esta pasta no menu File > Settings, de forma que todos os OSes usem o path definido (o default é o que já é usado hoje)."*

Facts, read in the source on 2026-10-14:

1. A build stages a generated Cargo crate in `<root>/cobolt-build-<name>/` and cargo keeps its artefacts there between builds. `<root>` is `build_cache_root()` (`cobolt-compiler/src/lib.rs`): **macOS** `~/Library/Caches/PowerRustCOBOL/builds`, **Windows** `%LOCALAPPDATA%\PowerRustCOBOL\builds`, **Linux** `$XDG_CACHE_HOME/PowerRustCOBOL/builds` else `~/.cache/PowerRustCOBOL/builds`, and the OS temp directory only when none resolves. It is not the OS temp directory on purpose: the temp reapers delete files that cargo still records as built.
2. There is **one** production use of it: `build_core` (`let build_dir = build_cache_root().join("cobolt-build-<bin>")`). The IDE builds `BuildOptions` in one place (`app.rs`), `rcrun build` in one (`cobolt-cli`), the project tools' `build` in one.
3. The IDE already keeps machine-wide preferences in `ui.toml` (`UiPrefs`), and one of them is the same kind of thing: `workspace_root`, the Platform SDK folder, empty meaning "work it out yourself", edited by a fixed-size dialog with *Browse* and *Clear* (`show_sdk_location`).
4. There is **no** "File ▸ Settings" menu entry (spec 099 §1.4); the dialog goes under the Settings menu, beside the SDK's.

## 2. Goals / Non-goals
- **Goals:** a **Build Folder…** dialog under Settings that names the folder builds are kept in; empty — the default — is exactly today's rule on every OS; every build the IDE starts, and the project tools running inside it, use the chosen folder; `rcrun` honours it through an environment variable; the dialog says what is in force and where it came from.
- **Non-goals:** moving or deleting the old folder's contents; the other `PowerRustCOBOL` folders (the knowledge-base data); a per-project setting — the folder is the machine's, so a project shared with a colleague never carries a path that exists on one disk only.

## 3. Requirements (EARS)
- **R1 (ubiquitous):** The Settings menu shall hold **Build Folder…**, which opens a dialog with the folder in force, where it came from (the setting, the environment, or the default), *Browse…*, *Use the default* and *Close*.
- **R2 (state):** While no folder is set, builds shall be staged exactly where they are today on every operating system.
- **R3 (state):** While a folder is set, a build shall stage `cobolt-build-<name>` directly inside it, and nothing shall be written under the default folder by that build.
- **R4 (ubiquitous):** The folder shall be resolved in this order: the explicit build option, the setting the IDE holds for the process (R7), the environment variable `POWERRUSTCOBOL_BUILD_DIR`, the default. A value that is empty, blank or not an absolute path shall be ignored and the next one used.
- **R5 (ubiquitous):** The setting shall be saved in `ui.toml` (per machine, not in `cobolt.toml`) and shall survive a restart; an older `ui.toml` shall read as "not set".
- **R6 (event):** When the developer picks a folder, the system shall check that it can be created and written and, if it cannot, shall say so and keep the previous setting.
- **R7 (ubiquitous):** The IDE shall hand the setting to every build it starts and to the project tools it hosts — all in its own process — through the same resolution (R4): once at start-up, and again whenever the developer changes it. (The IDE builds in-process and starts no `rcrun` to build; `rcrun build` run from a terminal or a pipeline has no IDE, and takes the environment variable.)
- **R8 (ubiquitous):** The dialog shall say that a new folder starts empty, so the next build is a full one, and that the old folder is left as it is.
- **R9 (ubiquitous):** The words of the dialog shall exist in the six languages, and the Developer's Guide shall say where builds are kept, on each OS, and how to change it.

## 4. Acceptance criteria
- [x] **AC1 (R2, R4)** — with nothing set the resolved folder equals the old rule; the order option > setting > environment > default holds; empty, blank and relative values fall through. *Proved by three tests in `cobolt-compiler` (`with_nothing_set…`, `the_build_folder_is_chosen_option_then_setting_then_environment_then_default`, `an_empty_blank_or_relative_build_folder_falls_through`).*
- [x] **AC2 (R3)** — the staging path of a build is `<chosen>/cobolt-build-<name>`; with the default it is the old path. *`build_staging_dir` is tested in the same three tests and by AC7.*
- [x] **AC3 (R5)** — `UiPrefs` round-trips the value; an older file reads as none. *`prefs_round_trip_through_toml`, `no_build_dir_or_a_blank_one_is_the_system_default`.*
- [x] **AC4 (R6)** — a folder that cannot be created is refused with a message and the setting is unchanged. *`check_build_root_usable` is tested (relative path, a path below a plain file, a fresh nested folder, the probe removed); the dialog's use of it — the message in the developer's language and the setting left alone — is code read, not a test: a `CoboltApp` cannot be built in a test.*
- [x] **AC5 (R7)** — `BuildOptions` built by the IDE carries the setting. *By reading the one place the IDE builds them (`app.rs`, `build_dir: load_build_dir()`) and the start-up and dialog calls to `set_build_root_override`; there is no test, for the reason above.*
- [x] **AC6 (R1, R8, R9)** — the dialog is drawn in six languages with no English left; the Guide has the section. *Nine texts added to `ide_text_data.rs`; `every_text_exists_in_six_languages` and `no_english_word_reaches_a_widget…` pass; the Guide section is "Where your builds are kept". Not seen drawn: the IDE window cannot be driven from the tools.*
- [x] **AC7** — a real build into a chosen folder lands there and leaves the default untouched. *`a_build_lands_in_the_chosen_folder_and_leaves_the_default_alone`: one real build, 24 s, `build_dir` = `<chosen>/cobolt-build-chosenfolder`, `Cargo.toml` there, the default staging path absent. It is not `#[ignore]`d — its peers in that module are not either.*

## 5. Constraints & steering check
- **i18n ×6:** the dialog's words (`ide_text_data.rs`, keyed by the English wording, as the other IDE texts). **Docs:** the English Guide. **Fix vs feature:** **feature**; `z` bump; CHANGELOG. **Never-self-resizing-window rule:** fixed size, as the SDK dialog.

## 6. Open questions (each taken at the answer assumed here)
- **Q1 — "the folder" is the root of the builds, not the `PowerRustCOBOL` folder above it.** *Assumed:* the chosen folder replaces `…/PowerRustCOBOL/builds`; `cobolt-build-<name>` goes directly inside it.
- **Q2 — typing a path.** *Assumed:* picking only, like the SDK dialog; the environment variable covers a scripted setup.

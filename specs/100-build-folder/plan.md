# Plan — the build folder

- **Status:** implemented in 1.91.154   **Spec:** ./spec.md   **Date:** 2026-10-14

## Approach
**Compiler.** `BuildOptions.build_dir: Option<PathBuf>`; `pub const BUILD_DIR_ENV`; `pub enum BuildRootSource { Setting, Environment, Default }`; `pub fn default_build_root()` (the old `build_cache_root`, unchanged, now public); `pub fn set_build_root_override(Option<PathBuf>)` (a process-wide setting behind a `Mutex`); `pub fn resolve_build_root(configured: Option<&Path>) -> (PathBuf, BuildRootSource)` (R4: option, override, environment, default; a blank or relative value falls through); `pub fn build_staging_dir`; `build_core` stages in `resolve_build_root(opts.build_dir.as_deref()).0.join("cobolt-build-<bin>")` and logs the folder and where it came from.
**Check.** `check_build_root_usable(&Path) -> Result<(), BuildRootProblem>`: absolute, `create_dir_all`, a probe file written and removed. The problem is an enum (`NotAbsolute`, `CannotCreate(os error)`, `CannotWrite(os error)`) so the IDE can say it in the developer's language and still show the OS's detail.
**IDE.** `UiPrefs.build_dir` + `load_build_dir`/`save_build_dir` (as `workspace_root`); `CoboltApp::new` calls `set_build_root_override(load_build_dir())`; `BuildOptions { build_dir: load_build_dir(), … }`; `show_build_folder` beside `show_sdk_location`; `Item::BuildFolder` in `menu_layout`, under Settings after Platform SDK Location.

## Affected files
`cobolt-compiler/src/lib.rs`; `cobolt-cli/src/main.rs` and `cobolt-project-tools/src/tools/build.rs` (the new field, `None`); `cobolt-ide/src/ui_prefs.rs`, `app.rs`, `menu_layout.rs`, `ide_text_data.rs`; `docs/developers-guide-en.md`; `version.rs`, `CHANGELOG.md`.

## Decisions
- **A process-wide override, not an environment variable, as the bridge inside the IDE.** The first draft had the IDE write `POWERRUSTCOBOL_BUILD_DIR` into its own environment so the hosted project tools would see it. Writing the environment of a running multi-threaded process is unsound on some platforms (and `unsafe` in the 2024 edition), and the compiler can simply hold the value. The variable stays, for what it is good at: a build run with no IDE (`rcrun build` in a pipeline, the headless tool server).
- **Setting beats environment:** what the developer chose in the IDE is the more specific statement.
- **The IDE builds in its own process**, so it starts no `rcrun` that would need the variable.

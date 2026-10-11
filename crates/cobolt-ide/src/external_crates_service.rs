// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! External Crates — the IDE-side service (spec 044).
//!
//! The registry client and the actions — search, resolve-to-pin, the
//! two-layer conflict check, vendoring, update, removal — live in
//! [`cobolt_project_tools::external_crates`], where the coding-agent tools
//! reach them too (a session that cannot reach the IDE registers crates
//! through `add_crate`). What stays here is the IDE's own: the IDE-wide
//! registry setting, the open project's crates published for analysis, and
//! the actions in the shape the dialog calls them — load the project through
//! its model, run the shared action, save through the model.
//!
//! State contract: actions load the project file, mutate, and save it on disk
//! (`project_model::{load_project, save_project}`); the caller saves any
//! in-memory project state *before* spawning an action and reloads after it
//! finishes. Progress narrates through a [`Note`] sink.

use std::path::{Path, PathBuf};

use cobolt_project_tools::external_crates as core;
pub use cobolt_project_tools::external_crates::{
    discard_alias_offer, system_closure, AddOutcome, Log, Note, Registry, RegistryError, Resolved,
    SearchHit, SearchPage, SystemCategory, SystemClosure, UpdateOutcome, DEFAULT_REGISTRY,
    RESULTS_PER_PAGE,
};
use cobolt_compiler::ExternalCrate;
use serde::{Deserialize, Serialize};

use crate::project_model::{load_project, save_project};

/// Identifying User-Agent, per crates.io's crawler policy.
fn user_agent() -> String {
    format!("PowerRustCOBOL/{} (Project's Crates)", crate::version::VERSION)
}

/// A registry client that names this IDE and its version, as the dialog
/// always has.
pub fn new_registry(base: &str) -> Registry {
    Registry::with_user_agent(base, &user_agent())
}

// ── IDE-wide registry setting (spec R4/R5, Q7) ───────────────────────────────

/// The pluggable registry endpoint — an **IDE-wide** setting (not stored in
/// `cobolt.toml`), persisted like `DebugSettings`: a small TOML in the IDE's
/// base dir. Changing it affects the *next* action; recorded pins keep the
/// URL they were added with (R5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCratesSettings {
    pub registry: String,
}

impl Default for ExternalCratesSettings {
    fn default() -> Self {
        Self { registry: DEFAULT_REGISTRY.to_string() }
    }
}

fn settings_path() -> PathBuf {
    crate::llm::base_dir().join("external_crates.toml")
}

impl ExternalCratesSettings {
    /// Load the persisted setting, falling back to crates.io on any error —
    /// a corrupt file must never keep the dialog from opening.
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(text) = toml::to_string_pretty(self) {
            let path = settings_path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, text);
        }
    }
}

// ── Analysis context (spec R20) ──────────────────────────────────────────────

/// The open project's registered crates as `use`-line names, published for
/// every semantic-analysis site in the IDE — the same per-frame in-process
/// sync `theme::set_active` and the debug switches use. `None` = no project
/// open (single-file semantics, R22). Behind an `RwLock` because Check/Run/
/// Debug analyze on worker threads.
static ACTIVE_LIB_NAMES: std::sync::OnceLock<std::sync::RwLock<Option<Vec<String>>>> =
    std::sync::OnceLock::new();

fn active_cell() -> &'static std::sync::RwLock<Option<Vec<String>>> {
    ACTIVE_LIB_NAMES.get_or_init(|| std::sync::RwLock::new(None))
}

/// Publish the open project's crates (the app calls this every frame, and
/// before spawning any analysis worker).
pub fn set_active_project_crates(names: Option<Vec<String>>) {
    if let Ok(mut cell) = active_cell().write() {
        *cell = names;
    }
}

/// The crates [`set_active_project_crates`] last published — what
/// [`analyze_project`] analyses under, and what the moved-down form
/// validation (`cobolt_project_tools::validate_source`, spec 080) is handed.
pub fn active_project_crates() -> Option<Vec<String>> {
    active_cell().read().ok().and_then(|cell| cell.clone())
}

/// [`cobolt_semantic::analyze_with`] under the active project's crates —
/// what every IDE analysis site calls instead of plain `analyze` (R20–R22).
pub fn analyze_project(
    program: &cobolt_ast::program::Program,
) -> cobolt_semantic::SemanticResult {
    let external_crates = active_project_crates();
    cobolt_semantic::analyze_with(
        program,
        &cobolt_semantic::AnalyzeOptions {
            external_crates,
            // 049 R17 — the IDE publishes the form map with the shell wiring
            // (spec 049 T27); until then only the build-path check runs.
            form_formats: None,
            // The IDE is a product gate: an undeclared item is an error.
            tolerate_undeclared: false,
            // No form context here: forms are checked by Run Form and Build.
            known_objects: None,
            // …nor a layer's properties (spec 091 R37): no form in view.
            known_layers: None,
            // No path here either: a literal CALL is checked by the form
            // check, Run Form and Build, which know the project.
            known_programs: None,
        },
    )
}


// ── Actions (R7–R19), in the dialog's shape ────────────────────────────────

fn project_dir_of(project_path: &Path) -> Result<PathBuf, String> {
    project_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no parent directory", project_path.display()))
}

/// Run a shared action over the project file's pins: load the project, hand the
/// action its pin list and a `save` that writes the changed list back through
/// the project model, so nothing but the pins changes.
fn with_pins<T>(
    project_path: &Path,
    run: impl FnOnce(&Path, &mut Vec<ExternalCrate>, core::Save) -> Result<T, String>,
) -> Result<T, String> {
    let project_dir = project_dir_of(project_path)?;
    let mut project = load_project(project_path).map_err(|e| e.to_string())?;
    let mut pins = project.crates.clone();
    let mut save = |changed: &[ExternalCrate]| -> Result<(), String> {
        project.crates = changed.to_vec();
        save_project(&project, project_path).map_err(|e| e.to_string())
    };
    run(&project_dir, &mut pins, &mut save)
}

/// R7–R15 — resolve, conflict-check, vendor, record the pin (or, spec 045 R1,
/// offer an alias).
#[allow(clippy::too_many_arguments)]
pub fn add(
    registry: &Registry,
    project_path: &Path,
    workspace_root: Option<PathBuf>,
    name: &str,
    requirement: Option<&str>,
    features: Vec<String>,
    system: Option<&SystemClosure>,
    log: Log,
) -> Result<AddOutcome, String> {
    with_pins(project_path, |dir, pins, save| {
        core::add(registry, dir, pins, workspace_root, name, requirement, features, system, log, save)
    })
}

/// Spec 045 R1 — the developer accepted an `AddOutcome::AliasOffered`.
pub fn confirm_alias(
    project_path: &Path,
    workspace_root: Option<PathBuf>,
    candidate: ExternalCrate,
    alias: &str,
    log: Log,
) -> Result<String, String> {
    with_pins(project_path, |dir, pins, save| {
        core::confirm_alias(dir, pins, workspace_root, candidate, alias, log, save)
    })
}

/// R16–R18 — update the named crates (empty = all).
pub fn update(
    registry: &Registry,
    project_path: &Path,
    workspace_root: Option<PathBuf>,
    targets: &[String],
    log: Log,
) -> Result<Vec<UpdateOutcome>, String> {
    with_pins(project_path, |dir, pins, save| {
        core::update(registry, dir, pins, workspace_root, targets, log, save)
    })
}

/// R19 — the **confirmed** removal (the dialog owns the confirmation): deletes
/// the record and the vendored source, never any COBOL.
pub fn remove(project_path: &Path, name: &str) -> Result<String, String> {
    with_pins(project_path, |dir, pins, save| core::remove(dir, pins, name, save))
}

// ── Flow tests: mock registry + live resolver verdicts (spec 044 T12) ────────
//
// In-src because `cobolt-ide` is a bin crate (integration tests cannot import
// it). The mock half exercises AC10/AC14 against a std-`TcpListener` server
// speaking the crates.io API shape — deterministic, no dependence on
// crates.io's release calendar. The live half exercises the real resolver
// verdicts (AC6/AC7/AC8), network + cargo, per the 041 heavy-test precedent.

#[cfg(test)]
mod flow_tests {
    use super::*;
    use cobolt_compiler::external_crates::vendor_dir;
    use std::io::Write as _;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    struct MockRegistry {
        base: String,
        /// Versions the mock "has published" — tests push to simulate time.
        versions: Arc<Mutex<Vec<String>>>,
        /// Versions whose download endpoint answers 500.
        broken_downloads: Arc<Mutex<Vec<String>>>,
    }

    fn start_mock() -> MockRegistry {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock registry");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let versions = Arc::new(Mutex::new(vec!["0.1.0".to_string()]));
        let broken_downloads = Arc::new(Mutex::new(Vec::<String>::new()));
        let (versions_bg, broken_bg) = (Arc::clone(&versions), Arc::clone(&broken_downloads));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let versions = Arc::clone(&versions_bg);
                let broken = Arc::clone(&broken_bg);
                std::thread::spawn(move || handle(stream, &versions, &broken));
            }
        });
        MockRegistry { base, versions, broken_downloads }
    }

    fn handle(
        mut stream: TcpStream,
        versions: &Mutex<Vec<String>>,
        broken: &Mutex<Vec<String>>,
    ) {
        use std::io::Read as _;
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let path = request
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap_or("/")
            .to_string();

        let (status, content_type, body): (&str, &str, Vec<u8>) = if path
            .starts_with("/api/v1/crates?")
        {
            // A registry with more matches than fit on one page, so the
            // dialog's paging is exercised for real: `mockcrate` is always
            // first (the tests add it), then filler up to TOTAL_MATCHES.
            const TOTAL_MATCHES: usize = 120;
            let query_of = |key: &str| -> usize {
                path.split(['?', '&'])
                    .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
            };
            let per_page = query_of("per_page").max(1);
            let page = query_of("page").max(1);
            let newest = versions.lock().unwrap().last().cloned().unwrap_or_default();
            let first = (page - 1) * per_page;
            let rows: Vec<serde_json::Value> = (first..(first + per_page).min(TOTAL_MATCHES))
                .map(|i| {
                    let name = if i == 0 {
                        "mockcrate".to_string()
                    } else {
                        format!("mockfiller{i}")
                    };
                    serde_json::json!({
                        "name": name,
                        "description": "a mock crate served by the test registry",
                        "max_stable_version": newest,
                        "downloads": (TOTAL_MATCHES - i) * 1000,
                    })
                })
                .collect();
            let json = serde_json::json!({
                "crates": rows,
                "meta": { "total": TOTAL_MATCHES },
            });
            ("200 OK", "application/json", json.to_string().into_bytes())
        } else if path == "/api/v1/crates/mockcrate" {
            let rows: Vec<serde_json::Value> = versions
                .lock()
                .unwrap()
                .iter()
                .map(|v| serde_json::json!({ "num": v, "yanked": false }))
                .collect();
            let json = serde_json::json!({ "versions": rows });
            ("200 OK", "application/json", json.to_string().into_bytes())
        } else if let Some(version) = path
            .strip_prefix("/api/v1/crates/mockcrate/")
            .and_then(|rest| rest.strip_suffix("/download"))
        {
            if broken.lock().unwrap().iter().any(|b| b == version) {
                ("500 Internal Server Error", "text/plain", b"boom".to_vec())
            } else {
                ("200 OK", "application/gzip", tarball(version))
            }
        } else {
            ("404 Not Found", "application/json", b"{}".to_vec())
        };

        let _ = write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(&body);
    }

    /// A minimal but real `.crate`: gzipped tar with `Cargo.toml` + `lib.rs`
    /// under `mockcrate-<version>/`.
    fn tarball(version: &str) -> Vec<u8> {
        fn append<W: std::io::Write>(builder: &mut tar::Builder<W>, path: &str, data: &[u8]) {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, path, data).unwrap();
        }
        let encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let manifest = format!(
            "[package]\nname = \"mockcrate\"\nversion = \"{version}\"\nedition = \"2021\"\n"
        );
        append(&mut builder, &format!("mockcrate-{version}/Cargo.toml"), manifest.as_bytes());
        append(
            &mut builder,
            &format!("mockcrate-{version}/src/lib.rs"),
            b"pub fn answer() -> i64 { 42 }\n",
        );
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc044-flow-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = crate::project_model::CoboltProject::new("FlowDemo", "src/main.cbl");
        crate::project_model::save_project(&project, &dir.join("cobolt.toml")).unwrap();
        dir
    }

    fn quiet() -> impl FnMut(Note) {
        |_| {}
    }

    /// AC10 + AC14, fully deterministic against the mock: search hits come
    /// from the configured registry; an add vendors from it and records its
    /// URL; switching the setting back leaves the pin untouched; a newer
    /// mock release updates old → new; a broken release fails and leaves
    /// everything as it was (R18). Timings reported.
    #[test]
    fn mock_registry_add_update_and_failure() {
        let t_all = Instant::now();
        let mock = start_mock();
        let dir = temp_project("mock");
        let manifest_path = dir.join("cobolt.toml");
        let registry = Registry::new(&mock.base);

        // Search — the rows come from the mock, not crates.io (R4/R6), a full
        // page at a time, and page 2 continues where page 1 stopped instead
        // of repeating it (no truncation to a handful).
        let first = registry
            .search("mock", RESULTS_PER_PAGE, 1)
            .expect("mock search page 1");
        assert_eq!(first.hits.len(), RESULTS_PER_PAGE);
        assert_eq!(first.total, 120);
        assert_eq!(first.hits[0].name, "mockcrate");
        assert!(first.hits[0].downloads > 0, "downloads must survive the parse");
        let second = registry
            .search("mock", RESULTS_PER_PAGE, 2)
            .expect("mock search page 2");
        assert_eq!(second.hits.len(), RESULTS_PER_PAGE);
        assert_eq!(second.total, 120);
        assert!(
            !second.hits.iter().any(|h| h.name == first.hits[0].name),
            "page 2 must not repeat page 1"
        );
        // The last page is the remainder, not a padded full page.
        let last = registry
            .search("mock", RESULTS_PER_PAGE, 3)
            .expect("mock search page 3");
        assert_eq!(last.hits.len(), 20);

        // Add — resolve/download from the mock; the probe runs for real.
        let t = Instant::now();
        let added = add(&registry, &manifest_path, None, "mockcrate", None, vec![], None, &mut quiet())
            .expect("mock add");
        let add_s = t.elapsed().as_secs_f32();
        let AddOutcome::Added(added) = added else {
            panic!("mockcrate must add cleanly, not offer an alias");
        };
        assert!(added.contains("0.1.0"));
        assert!(vendor_dir(&dir, "mockcrate", "0.1.0").is_dir());
        let project = load_project(&manifest_path).unwrap();
        assert_eq!(project.crates.len(), 1);
        assert!(
            project.crates[0].url.starts_with(&mock.base),
            "the recorded URL must point into the mock registry (AC14)"
        );
        // The manifest row carries that URL (AC14's manifest clause).
        let written =
            cobolt_compiler::external_crates::write_rust_manifest(
                &dir.join("dist"),
                "FlowDemo",
                &project.crates,
            )
            .unwrap()
            .expect("manifest written");
        let manifest_text = std::fs::read_to_string(written).unwrap();
        assert!(manifest_text.contains(&mock.base));

        // Switching the setting back to crates.io touches nothing (R5).
        let _crates_io = Registry::new(DEFAULT_REGISTRY);
        let untouched = load_project(&manifest_path).unwrap();
        assert_eq!(untouched.crates[0].url, project.crates[0].url);
        assert!(vendor_dir(&dir, "mockcrate", "0.1.0").is_dir());

        // The mock "publishes" 0.2.0 → Update All moves old → new (AC10).
        mock.versions.lock().unwrap().push("0.2.0".into());
        let t = Instant::now();
        let outcomes =
            update(&registry, &manifest_path, None, &[], &mut quiet()).expect("mock update");
        let update_s = t.elapsed().as_secs_f32();
        assert!(matches!(
            outcomes.as_slice(),
            [UpdateOutcome::Updated { old, new, .. }] if old == "0.1.0" && new == "0.2.0"
        ));
        assert!(vendor_dir(&dir, "mockcrate", "0.2.0").is_dir());
        assert!(!vendor_dir(&dir, "mockcrate", "0.1.0").exists());

        // A broken 0.3.0 fails the update and leaves 0.2.0 fully intact (R18).
        mock.versions.lock().unwrap().push("0.3.0".into());
        mock.broken_downloads.lock().unwrap().push("0.3.0".into());
        let outcomes =
            update(&registry, &manifest_path, None, &[], &mut quiet()).expect("update runs");
        assert!(matches!(outcomes.as_slice(), [UpdateOutcome::Failed { .. }]));
        let kept = load_project(&manifest_path).unwrap();
        assert_eq!(kept.crates[0].version, "0.2.0");
        assert!(vendor_dir(&dir, "mockcrate", "0.2.0").is_dir());

        println!("──────────────────────────────────────────────");
        println!("spec 044 mock-registry flow (AC10/AC14)");
        println!("cases: search-from-mock; add+vendor+URL; manifest URL;");
        println!("       setting switch-back; update 0.1.0→0.2.0;");
        println!("       broken release leaves pin+source untouched");
        println!("add (incl. resolver probe): {add_s:.1} s");
        println!("update (incl. probe):       {update_s:.1} s");
        println!("total:                      {:.1} s", t_all.elapsed().as_secs_f32());
        println!("──────────────────────────────────────────────");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// AC6/AC7/AC8 — the live verdicts, exactly as a developer meets them
    /// (crates.io + the real resolver). Timings reported.
    #[test]
    fn live_resolver_verdicts() {
        let t_all = Instant::now();
        let registry = Registry::new(DEFAULT_REGISTRY);

        // AC6 — egui is refused as already available; nothing recorded.
        let dir = temp_project("egui");
        let manifest_path = dir.join("cobolt.toml");
        let err = add(&registry, &manifest_path, None, "egui", None, vec![], None, &mut quiet())
            .expect_err("egui must be refused");
        assert!(err.contains("already available"), "got: {err}");
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        let _ = std::fs::remove_dir_all(&dir);

        // AC7 — an old rusqlite pins a libsqlite3-sys major that clashes
        // with the SQL runtime's on the native `sqlite3` links key; the
        // resolver's reason is surfaced and the download is cleaned up.
        let dir = temp_project("links");
        let manifest_path = dir.join("cobolt.toml");
        let t = Instant::now();
        let err = add(
            &registry,
            &manifest_path,
            None,
            "rusqlite",
            Some("=0.24.2"),
            vec![],
            None,
            &mut quiet(),
        )
        .expect_err("conflicting rusqlite must be refused");
        let links_s = t.elapsed().as_secs_f32();
        assert!(
            err.contains("sqlite3") || err.contains("links"),
            "the resolver's links reason must surface; got: {err}"
        );
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        assert!(
            !vendor_dir(&dir, "rusqlite", "0.24.2").exists(),
            "a refused add must clean its download up"
        );
        let _ = std::fs::remove_dir_all(&dir);

        // AC8 — a disjoint same-major pin coexists WITH the warning.
        let dir = temp_project("warn");
        let manifest_path = dir.join("cobolt.toml");
        let mut warnings = Vec::new();
        let t = Instant::now();
        add(&registry, &manifest_path, None, "itoa", Some("=1.0.10"), vec![], None, &mut |n| {
            if let Note::Warn(text) = n {
                warnings.push(text);
            }
        })
        .expect("itoa =1.0.10 must be allowed with a warning");
        let warn_s = t.elapsed().as_secs_f32();
        assert!(
            warnings.iter().any(|w| w.contains("exist twice")),
            "the coexistence warning must fire; warnings: {warnings:?}"
        );
        assert_eq!(load_project(&manifest_path).unwrap().crates.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);

        println!("──────────────────────────────────────────────");
        println!("spec 044 live resolver verdicts (AC6/AC7/AC8)");
        println!("egui already-available: refused (no resolver run)");
        println!("rusqlite =0.24.2 links clash: refused in {links_s:.1} s");
        println!("itoa =1.0.10 coexistence: warned in {warn_s:.1} s");
        println!("total: {:.1} s", t_all.elapsed().as_secs_f32());
        println!("──────────────────────────────────────────────");
    }

    /// Spec 045 R1/AC1 — a direct, version-incompatible collision (the exact
    /// case 044's R12 refused outright) is now an offer, not an `Err`; the
    /// download survives it so accepting doesn't pay for it twice.
    #[test]
    fn incompatible_direct_collision_offers_alias_not_error() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        let dir = temp_project("alias-offer");
        let manifest_path = dir.join("cobolt.toml");
        let outcome = add(&registry, &manifest_path, None, "egui", Some("=0.29.0"), vec![], None, &mut quiet())
            .expect("an incompatible direct collision must be an offer, not an Err");
        match outcome {
            AddOutcome::AliasOffered { candidate, linked_requirement, vendored } => {
                assert_eq!(candidate.name, "egui");
                assert_eq!(candidate.version, "0.29.0");
                assert_eq!(linked_requirement, "0.36");
                assert!(vendored.is_dir(), "the download must survive the offer");
            }
            AddOutcome::Added(_) => panic!("egui 0.29.0 must collide with the linked 0.36"),
        }
        assert!(
            load_project(&manifest_path).unwrap().crates.is_empty(),
            "not registered until the offer is confirmed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R2 — the other two `CollisionRefusal` variants are unchanged
    /// by R1: a compatible direct collision is still a plain refusal
    /// (`AlreadyAvailable`, informational — nothing to add). The third
    /// variant, `Reserved` (`cobolt-*`), can't be exercised through the live
    /// `add()` path — there is no real crate on crates.io by that name to
    /// resolve — so its regression coverage stays where it already is:
    /// `cobolt-compiler`'s `cobolt_prefix_is_reserved`, pure and untouched by
    /// this change (`layer1_collision` calls the same `name_collision` this
    /// change didn't modify; only what `add` does with an `Incompatible`
    /// result changed).
    #[test]
    fn compatible_and_reserved_collisions_still_refuse() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        let dir = temp_project("still-refused-compat");
        let manifest_path = dir.join("cobolt.toml");
        let err = add(&registry, &manifest_path, None, "egui", None, vec![], None, &mut quiet())
            .expect_err("a compatible direct collision must still be a plain refusal");
        assert!(err.contains("already available"), "got: {err}");
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R1 — declining the offer leaves no trace under `crates/`.
    #[test]
    fn declining_the_alias_offer_removes_the_vendored_download() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        let dir = temp_project("alias-decline");
        let manifest_path = dir.join("cobolt.toml");
        let outcome = add(&registry, &manifest_path, None, "egui", Some("=0.29.0"), vec![], None, &mut quiet())
            .expect("must offer, not refuse");
        let AddOutcome::AliasOffered { vendored, .. } = outcome else {
            panic!("expected an alias offer");
        };
        assert!(vendored.is_dir());
        discard_alias_offer(&vendored).expect("discard must succeed");
        assert!(!vendored.exists(), "declining must remove the download");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R1/R3 — accepting the offer pins the crate WITH its alias and
    /// probes the alias-shaped candidate (a bare path dependency — see
    /// `external_crates::pin_sections`), not the plain `version =` shape
    /// that already failed layer 1.
    #[test]
    fn confirming_the_alias_offer_pins_with_alias_and_probes_the_alias_shape() {
        let registry = Registry::new(DEFAULT_REGISTRY);
        let dir = temp_project("alias-confirm");
        let manifest_path = dir.join("cobolt.toml");
        let outcome = add(&registry, &manifest_path, None, "egui", Some("=0.29.0"), vec![], None, &mut quiet())
            .expect("must offer, not refuse");
        let AddOutcome::AliasOffered { candidate, .. } = outcome else {
            panic!("expected an alias offer");
        };
        let msg = confirm_alias(&manifest_path, None, candidate, "prj_egui", &mut quiet())
            .expect("the alias-shaped candidate must probe clean");
        assert!(msg.contains("prj_egui"), "got: {msg}");

        let project = load_project(&manifest_path).unwrap();
        assert_eq!(project.crates.len(), 1);
        assert_eq!(project.crates[0].alias.as_deref(), Some("prj_egui"));
        assert_eq!(project.crates[0].lib_name(), "prj_egui");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R11/AC7, as amended 2026-10-11 — a name the application links
    /// DIRECTLY is refused before any network call, and never with an alias
    /// offer; a name it only pulls in as a dependency of a linked crate is
    /// not: it goes on to the registry. The registry here points at an
    /// unreachable host, so the direct name's refusal must be the
    /// "cannot be registered" text (a bypassed check would fail as a
    /// network/DNS error instead), and the transitive name must fail as
    /// exactly that network error — proof it was let through.
    #[test]
    fn a_directly_linked_crate_is_refused_before_the_network_a_transitive_one_is_not() {
        let registry = Registry::new("https://unreachable.invalid.example");
        let dir = temp_project("system-refused");
        let manifest_path = dir.join("cobolt.toml");
        let closure = SystemClosure {
            direct: ["egui".to_string()].into_iter().collect(),
            transitive: ["epaint".to_string()].into_iter().collect(),
        };
        let try_add = |name: &str| {
            add(&registry, &manifest_path, None, name, None, vec![], Some(&closure), &mut quiet())
        };

        match try_add("egui") {
            Err(reason) => assert!(reason.contains("cannot be registered"), "direct refusal text: {reason}"),
            Ok(_) => panic!("egui is directly linked and must be refused"),
        }
        match try_add("epaint") {
            Err(reason) => assert!(
                reason.contains("registry unreachable or unusable"),
                "a transitive crate must reach the registry, not be refused: {reason}"
            ),
            Ok(_) => panic!("the registry is unreachable, so this cannot have been added"),
        }
        assert!(load_project(&manifest_path).unwrap().crates.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R11, as amended 2026-10-11 — the positive half, against the
    /// mock registry with the real resolver: a crate the application only
    /// pulls in transitively is registered, vendored and pinned like any
    /// other, and the log says it was already in the application.
    #[test]
    fn a_transitive_crate_is_registered_so_a_block_can_use_it() {
        let mock = start_mock();
        let registry = Registry::new(&mock.base);
        let dir = temp_project("transitive");
        let manifest_path = dir.join("cobolt.toml");
        let closure = SystemClosure {
            direct: ["egui".to_string()].into_iter().collect(),
            transitive: ["mockcrate".to_string()].into_iter().collect(),
        };
        let mut notes: Vec<String> = Vec::new();
        let outcome = add(
            &registry,
            &manifest_path,
            None,
            "mockcrate",
            None,
            vec![],
            Some(&closure),
            &mut |n| match n {
                Note::Info(t) | Note::Warn(t) => notes.push(t),
            },
        )
        .expect("a transitive crate is not refused");
        assert!(matches!(outcome, AddOutcome::Added(_)), "added, not offered an alias");
        assert!(notes.iter().any(|n| n.contains("already in the application")), "{notes:?}");
        let project = load_project(&manifest_path).unwrap();
        assert_eq!(project.crates.len(), 1);
        assert_eq!(project.crates[0].name, "mockcrate");
        assert!(vendor_dir(&dir, "mockcrate", "0.1.0").is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Spec 045 R12/AC4 — the live platform closure (real `cargo metadata`,
    /// same tier as `live_resolver_verdicts`): a name every generated program
    /// links directly lands in `direct`; a name that only rides along inside
    /// something directly linked (egui's own rendering crate, `epaint` — not
    /// itself in `DIRECT_LINKED`) lands in `transitive`; an unrelated crate
    /// lands in neither.
    #[test]
    fn system_closure_splits_direct_from_transitive() {
        let workspace = cobolt_compiler::resolve_workspace_root(None)
            .expect("cannot locate the PowerRustCOBOL workspace for this test");
        let t = Instant::now();
        let closure = system_closure(&workspace).expect("system_closure must resolve");
        let elapsed = t.elapsed().as_secs_f32();

        assert_eq!(closure.classify("egui"), Some(SystemCategory::Direct));
        assert_eq!(closure.classify("eframe"), Some(SystemCategory::Direct));
        assert_eq!(closure.classify("epaint"), Some(SystemCategory::Transitive));
        // `csv` rides in with document import (spec 074), so the unrelated
        // crate is one no platform crate reaches.
        assert_eq!(closure.classify("polars"), None);

        println!("──────────────────────────────────────────────");
        println!("spec 045 live system closure (AC4)");
        println!("direct: {} crates, transitive: {} crates", closure.direct.len(), closure.transitive.len());
        println!("resolved in {elapsed:.1} s");
        println!("──────────────────────────────────────────────");
    }
}

// ── Unit tests (no network) ────────────────────────────────────────────────
//
// The registry client's own unit tests live with it, in
// `cobolt_project_tools::external_crates`.

#[cfg(test)]
mod tests {
    use super::*;
    use cobolt_compiler::external_crates::vendor_dir;

    /// R4/Q7 — the setting round-trips through its IDE-wide file and a
    /// missing/corrupt file falls back to crates.io.
    #[test]
    fn settings_round_trip_and_default() {
        assert_eq!(ExternalCratesSettings::default().registry, DEFAULT_REGISTRY);
        let parsed: ExternalCratesSettings =
            toml::from_str("registry = \"https://mirror.example.com\"").unwrap();
        assert_eq!(parsed.registry, "https://mirror.example.com");
        let text = toml::to_string_pretty(&parsed).unwrap();
        let back: ExternalCratesSettings = toml::from_str(&text).unwrap();
        assert_eq!(back.registry, parsed.registry);
    }

    /// R19 (AC13's service half) — a confirmed removal deletes the record
    /// and the vendored source, and only them: other pins and their sources
    /// stay, and no other project file is touched.
    #[test]
    fn remove_deletes_record_and_vendored_source_only() {
        let dir = std::env::temp_dir().join(format!("prc044-svc-rm-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest_path = dir.join("cobolt.toml");
        let mut project = crate::project_model::CoboltProject::new("RmDemo", "src/main.cbl");
        for (name, version) in [("csv", "1.4.0"), ("itoa", "1.0.10")] {
            project.crates.push(ExternalCrate {
                name: name.into(),
                requirement: String::new(),
                version: version.into(),
                features: vec![],
                url: format!("https://crates.io/crates/{name}"),
                alias: None,
            });
            std::fs::create_dir_all(vendor_dir(&dir, name, version)).unwrap();
        }
        save_project(&project, &manifest_path).unwrap();

        let message = remove(&manifest_path, "csv").expect("remove csv");
        assert!(message.contains("csv"));
        let after = load_project(&manifest_path).unwrap();
        assert_eq!(after.crates.len(), 1);
        assert_eq!(after.crates[0].name, "itoa");
        assert!(!vendor_dir(&dir, "csv", "1.4.0").exists());
        assert!(vendor_dir(&dir, "itoa", "1.0.10").is_dir());
        // An unknown crate is a clean error, not a mutation.
        assert!(remove(&manifest_path, "nope").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// R20–R22 (AC3's Check half) — one test owns the process-global so the
    /// two states cannot race: with the project's crates published, a block
    /// using one passes analysis; with no project published, the same block
    /// fails with the single-file message.
    #[test]
    fn analyze_project_honours_the_published_crates() {
        let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
                   PROCEDURE DIVISION.\nMAIN.\n    EXEC RUST\n        use csv::x;\n    END-EXEC.\n    STOP RUN.\n";
        let program = cobolt_parser::parse(cobolt_lexer::tokenize(
            src,
            cobolt_lexer::SourceFormat::Free,
        ))
        .program
        .unwrap();

        set_active_project_crates(Some(vec!["csv".into()]));
        let allowed = analyze_project(&program);
        assert!(
            allowed
                .errors()
                .all(|d| !d.message.contains("does not link")),
            "registered csv must pass the Check path"
        );

        set_active_project_crates(None);
        let refused = analyze_project(&program);
        assert!(
            refused
                .errors()
                .any(|d| d.message.contains("require a project")),
            "with no project the single-file message must appear"
        );
    }

}

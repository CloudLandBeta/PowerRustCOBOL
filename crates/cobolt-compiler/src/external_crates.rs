// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! External Crates for `EXEC RUST` (spec 044) — the build-side model.
//!
//! A project registers third-party crates as `[[crates]]` pins in
//! `cobolt.toml`; their source is vendored under the project's `crates/`
//! folder by the IDE at add time. This module owns everything a build needs
//! — the pin record, the reserved-name collision check, the generated-manifest
//! additions, and the delivered `rust_manifest.md` — and deliberately **no
//! network**: resolution and download happen in the IDE when the developer
//! adds or updates a crate, never during a build (spec R7, R10).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The subdirectory of the project folder holding vendored crate sources —
/// also the on-disk root of the IDE's **External Crates** tree category
/// (spec R1).
pub const VENDOR_SUBDIR: &str = "crates";

/// One registered crate — everything an add records (spec R8).
///
/// `requirement` is the developer's own words (empty = "newest stable");
/// `version` is the exact pin every build uses until an explicit update
/// (spec R10). `url` is the crate's page on the registry it came from,
/// recorded at add time so a later registry switch cannot rewrite history
/// (spec R5) — it is what `rust_manifest.md` prints (spec R24). `alias`
/// (spec 045 R1/R3) is set only when this pin was added to escape a
/// direct, version-incompatible collision with a platform-linked crate — the
/// generated `[dependencies]` entry becomes a `package =` rename instead of
/// the normal exact-version pin, and a block writes `use <alias>::…` instead
/// of the crate's own name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCrate {
    pub name: String,
    #[serde(default)]
    pub requirement: String,
    pub version: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

impl ExternalCrate {
    /// The name a block writes in `use` lines (spec R20; spec 045 R3): an
    /// aliased pin uses its alias's lib name, never the underlying crate's —
    /// one `use` name must denote exactly one crate. Registry `-` becomes
    /// `_`, exactly cargo's lib-name convention.
    pub fn lib_name(&self) -> String {
        lib_name(self.alias.as_deref().unwrap_or(&self.name))
    }
}

/// Normalize a crate name for `use`-line and identity comparisons — cargo
/// treats `foo-bar` and `foo_bar` as the same package (spec R20).
pub fn lib_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

/// Where a registered crate's source is vendored:
/// `<project>/crates/<name>-<version>`.
pub fn vendor_dir(project_dir: &Path, name: &str, version: &str) -> PathBuf {
    project_dir.join(VENDOR_SUBDIR).join(format!("{name}-{version}"))
}

// ── Generated-manifest additions (spec R7, R10, R15) ─────────────────────────

/// A path as cargo must see it in a staged manifest: absolute (canonical when
/// it exists) and forward-slashed. Cargo resolves `path =` entries relative
/// to the manifest that names them — the staged build and probe manifests
/// live elsewhere — and a raw Windows backslash is a TOML escape sequence.
pub(crate) fn toml_path(path: &Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    });
    abs.display().to_string().replace('\\', "/")
}

/// The two sections a build adds for registered pins: exact-versioned
/// `[dependencies]` lines, and `[patch.crates-io]` entries routing each pin
/// at its vendored source. The patch is the R15 mechanism: it makes the
/// project-local copy THE copy for the whole graph, so a crate the base tree
/// also uses (serde) unifies to one copy — never a path copy and a registry
/// copy side by side. A pin without a vendored dir gets no patch entry (the
/// resolver probe runs before the candidate is downloaded); builds reject
/// that state via [`validate_pins`].
///
/// An **aliased** pin (spec 045 R1/R3 — the escape hatch for a name that
/// collides with a platform-linked crate at an incompatible version) takes a
/// deliberately different shape: a bare `package =` + `path =` dependency
/// under its alias name, with **no** `[patch.crates-io]` entry. A patch entry
/// keys on the crates.io source name and would apply to *every* consumer of
/// that name — including the platform's own, compatible-version dependency —
/// which would silently unify the two exactly where the alias exists to keep
/// them apart. An unpatched path dependency under a different local name is
/// how cargo resolves two genuinely semver-incompatible versions of the same
/// underlying crate as two separate packages (the handoff's experiment for
/// spec 045 proved this is only available when the versions are actually
/// incompatible — a compatible alias trips the ordinary unify-and-reject
/// cargo already does, which is why R1 only ever offers the alias for the
/// incompatible case).
pub(crate) fn pin_sections(project_dir: &Path, pins: &[ExternalCrate]) -> (String, String) {
    let mut deps = String::new();
    let mut patches = String::new();
    for c in pins {
        let features = if c.features.is_empty() {
            String::new()
        } else {
            format!(
                ", features = [{}]",
                c.features
                    .iter()
                    .map(|f| format!("\"{f}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let vendored = vendor_dir(project_dir, &c.name, &c.version);
        if let Some(alias) = &c.alias {
            // Deliberately no `version =` — the vendored path pins it, and a
            // registry requirement here would be misleading (this copy never
            // resolves from crates.io). No patch entry either (see above).
            deps.push_str(&format!(
                "{alias} = {{ package = \"{}\", path = \"{}\"{features} }}\n",
                c.name,
                toml_path(&vendored)
            ));
            continue;
        }
        deps.push_str(&format!(
            "{} = {{ version = \"={}\"{features} }}\n",
            c.name, c.version
        ));
        if vendored.is_dir() {
            patches.push_str(&format!(
                "{} = {{ path = \"{}\" }}\n",
                c.name,
                toml_path(&vendored)
            ));
        }
    }
    (deps, patches)
}

/// Spec R10 — a build uses each pin's project-local source. A pin whose
/// vendored dir is gone fails loudly here instead of quietly resolving from
/// the network (which would un-pin the build the day the registry moves).
pub fn validate_pins(project_dir: &Path, pins: &[ExternalCrate]) -> Result<(), String> {
    for c in pins {
        let dir = vendor_dir(project_dir, &c.name, &c.version);
        if !dir.is_dir() {
            return Err(format!(
                "external crate `{}` {} has no vendored source at {} — \
                 re-add it, or run Update in Project's Crates",
                c.name,
                c.version,
                dir.display()
            ));
        }
    }
    Ok(())
}

/// The resolver-probe manifest (spec R11–R15): the same dependency set a real
/// build stages — base block, registered pins, their patches — plus the
/// candidate being added or updated. The IDE runs `cargo metadata` over this
/// on a worker thread; cargo's own resolver is the conflict oracle.
///
/// `links_gui` is `true` in every probe: a program containing any `EXEC RUST`
/// block links the GUI stack, and blocks are the only reason crates get added.
pub fn probe_manifest(
    crates_path: &Path,
    project_dir: &Path,
    pins: &[ExternalCrate],
    candidate: Option<&ExternalCrate>,
) -> String {
    let mut all: Vec<ExternalCrate> = pins.to_vec();
    all.extend(candidate.cloned());
    let (pin_deps, patches) = pin_sections(project_dir, &all);
    let mut s = format!(
        "[package]\n\
         name    = \"prc-probe\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\n\
         # Detached: the probe must resolve as its own package even when the\n\
         # folder it is staged in lives under someone else's cargo workspace.\n\
         [workspace]\n\n\
         [dependencies]\n{}",
        crate::base_dependency_block(crates_path, true)
    );
    s.push_str(&pin_deps);
    if !patches.is_empty() {
        s.push_str("\n[patch.crates-io]\n");
        s.push_str(&patches);
    }
    s
}

// ── The delivered manifest (spec R24–R26) ────────────────────────────────────

/// The manifest's file name inside the destination folder.
pub const RUST_MANIFEST_FILE: &str = "rust_manifest.md";

/// Write `rust_manifest.md` beside the delivered binary (spec R24), or —
/// when the project registers no crates — delete a stale one left by an
/// earlier build (spec R25): the delivered folder never claims third-party
/// code the binary does not contain. Returns the written path, if any.
pub fn write_rust_manifest(
    dest_dir: &Path,
    project_name: &str,
    pins: &[ExternalCrate],
) -> Result<Option<PathBuf>, String> {
    let path = dest_dir.join(RUST_MANIFEST_FILE);
    if pins.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| format!("cannot remove stale {}: {e}", path.display()))?;
        }
        return Ok(None);
    }
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("cannot create {}: {e}", dest_dir.display()))?;
    std::fs::write(&path, render_rust_manifest(project_name, pins))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(Some(path))
}

/// Spec R24's columns exactly — name, exact built version, the URL recorded
/// at add time — under the generated-artefact banner (spec R26).
fn render_rust_manifest(project_name: &str, pins: &[ExternalCrate]) -> String {
    let mut s = format!(
        "<!-- Generated by PowerRustCOBOL — build artefact, do not edit.\n\
         \u{20}    Regenerated on every successful build of {project_name}. -->\n\n\
         # Rust crate manifest — {project_name}\n\n\
         Third-party Rust crates compiled into this application's binary:\n\n\
         | Crate | Version | URL |\n|-------|---------|-----|\n"
    );
    let mut sorted: Vec<&ExternalCrate> = pins.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    for c in sorted {
        // Spec 045 R4 — an aliased pin's Crate cell notes the rename a block
        // actually `use`s; a non-aliased row is unchanged.
        let crate_cell = match &c.alias {
            Some(alias) => format!("{} (as `{alias}`)", c.name),
            None => c.name.clone(),
        };
        s.push_str(&format!("| {} | {} | {} |\n", crate_cell, c.version, c.url));
    }
    s
}

// ── Reserved names (spec R12) ────────────────────────────────────────────────

/// The crates every generated program links **directly**, with the
/// requirement `generate_cargo_toml` writes for them. A block's `use` name
/// must denote exactly one crate, so these names are reserved: adding one at
/// a compatible version is redundant, at an incompatible version impossible.
///
/// Kept next to `generate_cargo_toml` in spirit: change one, change both —
/// `the_direct_linked_table_matches_the_generated_manifest` pins the two
/// together, and `cobolt-semantic`'s allowlist is checked against this table
/// (plan §4d, revised to a subset check — see that test's comment).
const DIRECT_LINKED: &[(&str, &str)] = &[
    // Always emitted.
    ("cobolt-ast", "*"),
    ("cobolt-runtime", "*"),
    ("flate2", "1"),
    ("bincode", "1"),
    ("tracing", "0.1"),
    ("tracing-subscriber", "0.3"),
    // Emitted when the GUI stack links (any form, or any EXEC RUST block).
    ("cobolt-form-host", "*"),
    ("cobolt-forms", "*"),
    ("cobolt-media", "*"),
    ("eframe", "0.36"),
    ("egui", "0.36"),
    ("egui_extras", "0.36"),
    ("rfd", "0.14"),
    ("pollster", "0.3"),
    // Direct dep only as a feature-union workaround (see the zune-jpeg FIX
    // note in `base_dependency_block`); reserved like any other direct dep —
    // a second `zune-jpeg` key in [dependencies] would not even parse.
    ("zune-jpeg", "0.5"),
];

/// The direct-linked table as `use`-line names, for allowlist parity checks.
pub fn direct_linked_lib_names() -> Vec<String> {
    DIRECT_LINKED.iter().map(|(n, _)| lib_name(n)).collect()
}

/// Why an add is refused before any network or resolver work (spec R12).
/// Structured, not prose: the IDE localizes by matching the variant; `rcrun`
/// and tests render the English `Display`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollisionRefusal {
    /// The name is semver-compatible with the linked copy — a block can
    /// `use` it today, no add needed (informational).
    AlreadyAvailable { name: String, linked_requirement: String },
    /// The name clashes with the linked copy at an incompatible version.
    Incompatible {
        name: String,
        requested: String,
        linked_requirement: String,
    },
    /// `cobolt-*` is PowerRustCOBOL itself.
    Reserved { name: String },
}

impl std::fmt::Display for CollisionRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollisionRefusal::AlreadyAvailable { name, linked_requirement } => write!(
                f,
                "`{name}` is already available — every program links \
                 `{name} {linked_requirement}`; use it in a block directly, no add needed"
            ),
            CollisionRefusal::Incompatible { name, requested, linked_requirement } => write!(
                f,
                "`{name}` {requested} clashes with the built-in `{name} {linked_requirement}`: \
                 one `use {}` cannot denote two crates",
                lib_name(name)
            ),
            CollisionRefusal::Reserved { name } => write!(
                f,
                "`{name}` is part of PowerRustCOBOL itself and is always linked"
            ),
        }
    }
}

/// Layer 1 of conflict checking (spec R12): the candidate's name against the
/// reserved set, decided locally and instantly. `resolved_version` is the
/// exact version the add would pin. `None` = no collision; the resolver
/// probe (layer 2) still has the final word.
pub fn name_collision(
    candidate: &str,
    resolved_version: &semver::Version,
) -> Option<CollisionRefusal> {
    let wanted = lib_name(candidate);
    if wanted.starts_with("cobolt_") {
        return Some(CollisionRefusal::Reserved { name: candidate.to_string() });
    }
    let (name, linked_req) = DIRECT_LINKED.iter().find(|(n, _)| lib_name(n) == wanted)?;
    if *linked_req == "*" {
        // Workspace path crates carry no registry requirement; the prefix
        // rule above already caught them, but keep the arm total.
        return Some(CollisionRefusal::Reserved { name: candidate.to_string() });
    }
    let req = semver::VersionReq::parse(linked_req).expect("DIRECT_LINKED requirements parse");
    Some(if req.matches(resolved_version) {
        CollisionRefusal::AlreadyAvailable {
            name: (*name).to_string(),
            linked_requirement: (*linked_req).to_string(),
        }
    } else {
        CollisionRefusal::Incompatible {
            name: (*name).to_string(),
            requested: resolved_version.to_string(),
            linked_requirement: (*linked_req).to_string(),
        }
    })
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    /// Spec R8/R10 — the pin record round-trips through `[[crates]]` TOML,
    /// and absent optional fields default (old projects load unchanged).
    #[test]
    fn pins_round_trip_and_default() {
        #[derive(Serialize, Deserialize)]
        struct Wrapper {
            #[serde(default, rename = "crates")]
            crates: Vec<ExternalCrate>,
        }
        let text = r#"
            [[crates]]
            name    = "csv"
            version = "1.4.0"

            [[crates]]
            name        = "serde"
            requirement = "^1"
            version     = "1.0.229"
            features    = ["derive"]
            url         = "https://crates.io/crates/serde"

            [[crates]]
            name    = "egui"
            version = "0.29.0"
            alias   = "prj_egui"
        "#;
        let parsed: Wrapper = toml::from_str(text).unwrap();
        assert_eq!(parsed.crates.len(), 3);
        assert_eq!(parsed.crates[0].requirement, "");
        assert_eq!(parsed.crates[0].features, Vec::<String>::new());
        assert_eq!(parsed.crates[0].alias, None);
        assert_eq!(parsed.crates[1].features, vec!["derive".to_string()]);
        assert_eq!(parsed.crates[2].alias, Some("prj_egui".to_string()));
        let back = toml::to_string(&parsed).unwrap();
        let reparsed: Wrapper = toml::from_str(&back).unwrap();
        assert_eq!(reparsed.crates, parsed.crates);
        // A pin with no alias never gains an `alias =` line — old projects'
        // files stay byte-identical to what they were before this field
        // existed. Only the one aliased pin above should emit it.
        assert_eq!(
            back.matches("alias").count(),
            1,
            "only the aliased pin may emit `alias =`; got:\n{back}"
        );
        // And a manifest with no [[crates]] at all parses to empty.
        let empty: Wrapper = toml::from_str("").unwrap();
        assert!(empty.crates.is_empty());
    }

    /// Spec R20 — `serde-json` is written `serde_json` inside a block.
    #[test]
    fn lib_name_swaps_dashes() {
        assert_eq!(lib_name("serde-json"), "serde_json");
        assert_eq!(lib_name("  CSV "), "csv");
        assert_eq!(
            ExternalCrate {
                name: "serde-json".into(),
                requirement: String::new(),
                version: "1.0.0".into(),
                features: vec![],
                url: String::new(),
                alias: None,
            }
            .lib_name(),
            "serde_json"
        );
    }

    /// Spec 045 R3 — an aliased pin's `lib_name` is the alias, never the
    /// underlying crate's own name: a block must `use prj_egui::…`, not
    /// `use egui::…`, since that name already denotes the platform's copy.
    #[test]
    fn lib_name_honors_alias() {
        let aliased = ExternalCrate {
            name: "egui".into(),
            requirement: String::new(),
            version: "0.29.0".into(),
            features: vec![],
            url: String::new(),
            alias: Some("prj_egui".into()),
        };
        assert_eq!(aliased.lib_name(), "prj_egui");
    }

    /// Spec R12 / AC6 — a compatible duplicate of a directly-linked crate is
    /// refused as already available; an incompatible one as a clash.
    #[test]
    fn direct_link_collisions_are_refused_both_ways() {
        match name_collision("egui", &v("0.36.1")) {
            Some(CollisionRefusal::AlreadyAvailable { name, .. }) => assert_eq!(name, "egui"),
            other => panic!("compatible egui must be AlreadyAvailable, got {other:?}"),
        }
        match name_collision("egui", &v("0.29.0")) {
            Some(CollisionRefusal::Incompatible { requested, linked_requirement, .. }) => {
                assert_eq!(requested, "0.29.0");
                assert_eq!(linked_requirement, "0.36");
            }
            other => panic!("incompatible egui must be Incompatible, got {other:?}"),
        }
    }

    /// Spec R12 — the whole `cobolt-*` prefix is PowerRustCOBOL itself,
    /// dash/underscore blind.
    #[test]
    fn cobolt_prefix_is_reserved() {
        for name in ["cobolt-forms", "cobolt_runtime", "cobolt-anything-future"] {
            match name_collision(name, &v("9.9.9")) {
                Some(CollisionRefusal::Reserved { .. }) => {}
                other => panic!("{name} must be Reserved, got {other:?}"),
            }
        }
    }

    /// A name nobody links passes layer 1 untouched (the probe still runs).
    #[test]
    fn unrelated_names_pass_layer_one() {
        assert_eq!(name_collision("csv", &v("1.4.0")), None);
    }

    /// The vendor path is `<project>/crates/<name>-<version>` (spec R1).
    #[test]
    fn vendor_dir_shape() {
        let d = vendor_dir(Path::new("/p"), "csv", "1.4.0");
        assert_eq!(d, Path::new("/p/crates/csv-1.4.0"));
    }

    fn pin(name: &str, version: &str, features: &[&str]) -> ExternalCrate {
        ExternalCrate {
            name: name.into(),
            requirement: String::new(),
            version: version.into(),
            features: features.iter().map(|f| f.to_string()).collect(),
            url: format!("https://crates.io/crates/{name}"),
            alias: None,
        }
    }

    /// Spec 045 — an aliased pin, as `confirm_alias` would build it.
    fn aliased_pin(name: &str, version: &str, alias: &str) -> ExternalCrate {
        ExternalCrate { alias: Some(alias.into()), ..pin(name, version, &[]) }
    }

    fn temp_project(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("prc044-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Spec R7/R10/R15 — a pin becomes an exact-versioned dep with its
    /// features, and a `[patch.crates-io]` entry exactly when its vendored
    /// source exists; patch paths are absolute and forward-slashed.
    #[test]
    fn pin_sections_emit_exact_pins_and_patches() {
        let project = temp_project("pins");
        std::fs::create_dir_all(vendor_dir(&project, "csv", "1.4.0")).unwrap();
        let pins = [pin("csv", "1.4.0", &[]), pin("serde", "1.0.229", &["derive"])];

        let (deps, patches) = pin_sections(&project, &pins);
        assert!(deps.contains("csv = { version = \"=1.4.0\" }"));
        assert!(deps.contains("serde = { version = \"=1.0.229\", features = [\"derive\"] }"));
        // csv is vendored → patched; serde is not (pre-download probe state).
        assert!(patches.contains("csv = { path = \""));
        assert!(!patches.contains("serde = { path"));
        let path_line = patches.lines().find(|l| l.starts_with("csv")).unwrap();
        assert!(!path_line.contains('\\'), "patch paths must be forward-slashed");
        let quoted = path_line.split('"').nth(1).unwrap();
        assert!(
            Path::new(quoted).is_absolute(),
            "patch paths must be absolute, got {quoted}"
        );
        let _ = std::fs::remove_dir_all(&project);
    }

    /// Spec 045 R1 — an aliased pin becomes a `package =` + `path =`
    /// dependency under its alias name, never a `version =` pin, and never
    /// gains a `[patch.crates-io]` entry (that would unify it with the
    /// platform's own, compatible-version copy — exactly what the alias
    /// exists to avoid). A co-present ordinary pin in the same call is
    /// unaffected.
    #[test]
    fn alias_pin_emits_package_path_not_version_or_patch() {
        let project = temp_project("alias-pins");
        std::fs::create_dir_all(vendor_dir(&project, "egui", "0.29.0")).unwrap();
        std::fs::create_dir_all(vendor_dir(&project, "csv", "1.4.0")).unwrap();
        let pins = [aliased_pin("egui", "0.29.0", "prj_egui"), pin("csv", "1.4.0", &[])];

        let (deps, patches) = pin_sections(&project, &pins);
        assert!(
            deps.contains("prj_egui = { package = \"egui\", path = \""),
            "got: {deps}"
        );
        assert!(!deps.contains("prj_egui = { version"), "got: {deps}");
        assert!(!patches.contains("egui"), "an aliased pin must not be patched: {patches}");
        // The ordinary co-present pin keeps its normal shape.
        assert!(deps.contains("csv = { version = \"=1.4.0\" }"));
        assert!(patches.contains("csv = { path = \""));
        let _ = std::fs::remove_dir_all(&project);
    }

    /// The backslash-bearing input case from plan §5: whatever the OS puts in
    /// the path, the emitted TOML carries `/` only.
    #[test]
    fn toml_path_never_emits_backslashes() {
        let odd = Path::new("/definitely-missing-prc044/a\\b");
        let rendered = toml_path(odd);
        assert!(!rendered.contains('\\'), "got {rendered}");
        assert!(Path::new(&rendered).is_absolute());
    }

    /// Spec R10 — a pin with no vendored source fails validation naming the
    /// missing dir and the remedy; a vendored one passes.
    #[test]
    fn validate_pins_requires_the_vendored_source() {
        let project = temp_project("validate");
        let pins = [pin("csv", "1.4.0", &[])];
        let err = validate_pins(&project, &pins).unwrap_err();
        assert!(err.contains("csv"));
        assert!(err.contains("crates/csv-1.4.0") || err.contains("crates\\csv-1.4.0"));
        assert!(err.contains("re-add"));
        std::fs::create_dir_all(vendor_dir(&project, "csv", "1.4.0")).unwrap();
        assert!(validate_pins(&project, &pins).is_ok());
        let _ = std::fs::remove_dir_all(&project);
    }

    /// Spec R11 — the probe manifest is detached (`[workspace]`), carries the
    /// GUI-linking base block, the registered pins, and the candidate.
    #[test]
    fn probe_manifest_stages_the_real_dependency_set() {
        let project = temp_project("probe");
        let pins = [pin("csv", "1.4.0", &[])];
        let candidate = pin("serde", "1.0.229", &["derive"]);
        let text = probe_manifest(Path::new("/ws/crates"), &project, &pins, Some(&candidate));
        assert!(text.contains("[workspace]"));
        assert!(text.contains("cobolt-runtime"));
        assert!(text.contains("eframe"), "probe always links the GUI stack");
        assert!(text.contains("csv = { version = \"=1.4.0\" }"));
        assert!(text.contains("serde = { version = \"=1.0.229\", features = [\"derive\"] }"));
        let _ = std::fs::remove_dir_all(&project);
    }

    /// Plan §4d — revised from "equal" to "subset", deliberately: semantic's
    /// allowlist must never admit a crate the generated manifest does not
    /// link (that is the promise that matters). The reverse is not required —
    /// flate2/bincode/tracing/rfd/pollster ARE linked but stay unadvertised,
    /// per 041 R16's "std, egui and eframe" wording.
    #[test]
    fn semantic_allowlist_is_a_subset_of_the_linked_table() {
        let intrinsic = ["std", "core", "alloc", "crate", "self", "super"];
        let linked = direct_linked_lib_names();
        for name in cobolt_semantic::exec_rust::LINKED_CRATES {
            if intrinsic.contains(name) {
                continue;
            }
            assert!(
                linked.contains(&lib_name(name)),
                "semantic allows `{name}`, which the generated manifest does not link"
            );
        }
    }

    /// Spec R24/R26 — every pin appears with name, version, and URL, sorted,
    /// under the generated-artefact banner.
    #[test]
    fn rust_manifest_lists_name_version_url() {
        let text = render_rust_manifest(
            "Demo",
            &[pin("serde", "1.0.229", &[]), pin("csv", "1.4.0", &[])],
        );
        assert!(text.contains("Generated by PowerRustCOBOL"));
        assert!(text.contains("| csv | 1.4.0 | https://crates.io/crates/csv |"));
        assert!(text.contains("| serde | 1.0.229 | https://crates.io/crates/serde |"));
        // Sorted: csv's row precedes serde's.
        assert!(text.find("| csv |").unwrap() < text.find("| serde |").unwrap());
    }

    /// Spec 045 R4 / open question 2 — an aliased pin's row notes the alias;
    /// a non-aliased row in the same manifest is byte-identical to today
    /// (the common case did not shift).
    #[test]
    fn rust_manifest_notes_the_alias() {
        let text = render_rust_manifest(
            "Demo",
            &[aliased_pin("egui", "0.29.0", "prj_egui"), pin("csv", "1.4.0", &[])],
        );
        assert!(
            text.contains("| egui (as `prj_egui`) | 0.29.0 | https://crates.io/crates/egui |"),
            "got: {text}"
        );
        assert!(text.contains("| csv | 1.4.0 | https://crates.io/crates/csv |"));
    }

    /// Spec R25 — zero pins removes a stale manifest instead of leaving the
    /// delivered folder claiming crates the binary does not contain.
    #[test]
    fn empty_pin_set_removes_the_stale_manifest() {
        let dest = temp_project("dist");
        let written = write_rust_manifest(&dest, "Demo", &[pin("csv", "1.4.0", &[])]).unwrap();
        assert!(written.is_some_and(|p| p.exists()));
        let removed = write_rust_manifest(&dest, "Demo", &[]).unwrap();
        assert!(removed.is_none());
        assert!(!dest.join(RUST_MANIFEST_FILE).exists());
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// The full generated manifest (spec R7/R10/R15 + plan §4h): `[workspace]`
    /// detachment, base deps, pins, and the patch section, in one artefact.
    #[test]
    fn generated_manifest_carries_workspace_pins_and_patches() {
        let project = temp_project("genmanifest");
        std::fs::create_dir_all(vendor_dir(&project, "csv", "1.4.0")).unwrap();
        let pins = [pin("csv", "1.4.0", &[])];
        let text = crate::generate_cargo_toml(
            "demo",
            "1.0.0",
            Path::new("/ws/crates"),
            true,
            &project,
            &pins,
        );
        assert!(text.contains("[workspace]"));
        assert!(text.contains("csv = { version = \"=1.4.0\" }"));
        assert!(text.contains("[patch.crates-io]"));
        assert!(text.contains("csv-1.4.0"));
        let _ = std::fs::remove_dir_all(&project);
    }
}

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec 044 prototype — External Crates for EXEC RUST

A standalone Rust prototype of the four mechanics the spec hangs on, written
so each module transplants into its final home. It is **not** product code:
it lives in the spec folder, detached from the workspace, and never touches a
real project or `cobolt.toml`.

## Run it

```sh
cd specs/044-external-crates/prototype
cargo run                        # the INTERACTIVE dialog (egui 0.36)
```

The window is the future IDE dialog: registry field on top (R4), search box
with pickable results (R6), crate/requirement/features row with **Add**
(R7–R15, verdicts in the log pane), the registered list with per-crate
**Update** / **Remove** and **Update All** (R2, R16–R18), a confirmed
removal modal (R19), and the manifest button (R24–R26).

The same actions, scripted:

```sh
cargo test                       # offline unit tests of the R-logic
cargo run -- search csv          # the add dialog's search box (R6)
cargo run -- add csv             # resolve → conflict probe → vendor → record
cargo run -- add serde --features derive
cargo run -- add egui            # refused: already linked (R12)
cargo run -- list
cargo run -- manifest            # writes demo-project/dist/rust_manifest.md
cargo run -- update --all
cargo run -- remove csv          # asks for confirmation (R19)
```

State lands in `./demo-project/` (`external-crates.toml`, `crates/`,
`dist/rust_manifest.md`, `.probe/`). `--registry <url>` swaps the endpoint
(R4); `--skip-probe` skips the resolver probe when offline.

## Module → final home

| Prototype module | Proves | Final home |
|---|---|---|
| `registry.rs` | Pluggable crates.io-compatible client: search, resolve-to-pin, download+unpack (R4–R9). Same blocking `ureq` + explicit `native_tls` connector as `cobolt-runtime::http_runtime`. | An IDE-side registry service module, called from the add dialog on a background thread. The base URL comes from the new IDE-wide setting. |
| `project.rs` | The pin record (name, requirement, exact version, features, URL) and its round-trip; `lib_name` dash→underscore (R8, R10, R20). | A `[[crates]]` array on `CoboltProject` (`project_model.rs`, serde-defaulted so old projects load); vendored sources under the project's `crates/` = the External Crates category root (R1). |
| `conflict.rs` | Layer 1: name collision against the direct-linked table (R12). Layer 2: `cargo metadata` over a synthesized probe manifest — cargo's own resolver as the oracle (R13), baseline-diff for coexistence warnings (R14), duplicate-copy guard (R15). | `base_manifest` is next-to-verbatim `cobolt-compiler::generate_cargo_toml` — the final build extends that function with the pinned deps + `[patch.crates-io]` entries; the probe runs from the IDE add/update flow. `DIRECT_LINKED` must become a shared constant exported by `cobolt-compiler`, the single source of truth also feeding `cobolt-semantic::LINKED_CRATES` (R21). |
| `manifest.rs` | `rust_manifest.md`: columns name/version/URL, generated-by banner, stale-file removal on empty (R24–R26). | `build_core` step 11c — the delivery step that already places the binary, assets, and license notices in `dist/`. |
| `ops.rs` | The action layer: add / update / remove / write-manifest as functions taking a progress sink, so one implementation narrates to both frontends. | The IDE-side service functions the dialog calls; the CLI printer becomes the Output panel. |
| `ui.rs` | The dialog itself, on egui **0.36** (`App::ui(&mut Ui, …)`, `egui::Panel`): slow actions on a worker thread reporting over `mpsc`, buttons disabled + spinner while busy, R19 confirm modal. | The `cobolt-ide` External Crates dialog — swap literals for `Tr` ×6, egui defaults for the glass theme, and the registry field for the IDE-wide setting. |

## The load-bearing design choice

Vendored crates enter the build through **`[patch.crates-io]`** plus an
exact-pinned dependency (`csv = "=1.3.1"`):

- the pin makes builds deterministic (R10);
- the patch makes the project-local source the one cargo actually uses (R10),
  and replaces the registry copy **everywhere in the graph** — so a crate the
  base tree also uses (serde!) resolves to exactly one copy, never a path
  copy and a registry copy side by side (R15);
- if the pinned version cannot satisfy the rest of the graph, `cargo
  metadata` refuses — which is precisely the add-time refusal R13 wants,
  with cargo's own explanation.

Sequencing note: the probe runs **before** the candidate is vendored, so the
candidate resolves from the registry index while the already-registered
crates resolve through their patches. After vendoring, the real build
manifest carries the same pins plus the candidate's patch — same single-copy
graph, now sourced from `crates/`. The final build must stage exactly that
(pins + patches) in `generate_cargo_toml`.

## Deliberately not prototyped

The egui dialog itself, i18n (`Tr` ×6), the tree category and its vector
icon, the semantic-diagnostic rewording (R21/R22), and the IDE settings
plumbing — all straightforward IDE work with established patterns, specified
in spec.md and left for `/plan` + `/implement`.

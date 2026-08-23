# Handoff — 1.61.128 → 1.61.139 (SUPERSEDED — everything here is pushed)

> ⚠️ **Start from `HANDOFF-2026-08-22.md` instead.** Everything this file
> describes as "uncommitted" was committed and pushed on 2026-08-21/22, and the
> version numbers below stop at 1.61.139 (`main` is now 1.61.144).
>
> Kept because its Maps and corner-system sections are still the best record of
> how those were diagnosed, and of the Maps demo's shape. Read it for that; read
> the newer file for the current state.

Started 2026-08-21, rewritten at the end of a long second session. Read
`CLAUDE.md` first; this covers only what a new session needs that is not
already there.

## Start here

1. ~~Nothing is committed.~~ **COMMITTED AND PUSHED, 2026-08-21 18:5x -03** (the
   window had opened). Two commits, per the operator's call:
   * `4009363` **feature** — OpenRouteService `TraceRoad` (1.61.132), merged as
     `705982e` and pushed **on its own**, so the f=96 announcement is clean.
   * `d2327f3` **fixes** — the other eleven changes (1.61.128–.131, .133–.139),
     merged as `26700d9` and pushed second.
   `origin/main` and `main` are level; `version.rs` carries **1.61.139**.
   **The operator settled 1.61.128 as a FIX** (f=97), which the section below
   still describes as an open question — it is not one any more.
   Shared files (`CHANGELOG.md`, `version.rs`, `assets/knowledge/chunked.data`,
   `crates/cobolt-compiler/src/lib.rs`, `docs/developers-guide-en.md`) went with
   the **fixes** commit rather than being carved hunk by hunk; the feature's own
   changelog section rides there too. Called out here because it is the one
   place the two commits are not perfectly pure.
   📣 **No per-push forum post — these fixes are announced with Release
   Candidate 3.** The operator, right after the push: *"do not publish"*, then
   *"we will announce the Release Candidate 3 with all this fixes."* So GOLDEN
   RULES #4 / #4b are not owed per commit here — **the unit of announcement is
   the RC**, and everything from 1.61.128 to 1.61.139 goes into that one post.
   Nothing was drafted or submitted. Do not post on your own initiative; the
   CHANGELOG is what the RC post gets assembled from, which is why each section
   still names its own version.
2. ~~One bug is still live~~ — **the corner leak is CLOSED** (1.61.135), and so
   is the dark hair the operator found once the wedge was gone (1.61.136). Both
   measurements, what they actually found (not what was hypothesised), and the
   guards that now pin them are under *“The corner leak — CLOSED”* below.
3. **The sweep is green**: 117 suites, **2216** passed, 0 failed.
4. Three things need the operator, not code: the fix/feature call on 1.61.128,
   whether the `neumorphic` pack ever gets real art, and whether `GlassStyle`
   should stop painting relief over pack art it does not own.
5. **The multi-selection work is a FIX, not a feature — operator's call, and it
   supersedes what this file said an hour earlier.** Multi-selection already
   existed (lasso, and Cmd/Ctrl+click machinery that reacted but never
   completed), so a pane that could not edit it was incomplete functionality,
   i.e. debt. Same call they made for the leaderboard and the Maps routes set;
   see *“Decisions worth not re-litigating”*. Landed as 1.61.137/138/139.
5b. **The second batch is COMMITTED AND PUSHED too** — 1.61.140 → 1.61.144
   (rows 13–17), commit `12e77fb`, merged as `8b993c4`, Saturday 2026-08-22
   ~07:30 -03 (weekend, so the window was open). All fixes, one commit, **no
   forum post** — the operator: *"commit, push, no publishing."* `origin/main`
   and `main` are level; `version.rs` carries **1.61.144**. Sweep: **117 suites,
   2230 passed, 0 failed.** Two of these came from OTHER users' reports rather
   than the operator's machine: the Windows 11 startup crash and the "built by an
   older PowerRustCOBOL" prompt.
   ⚠️ **The Windows 11 fix (1.61.140) has never run on Windows.** It is reasoned
   from the crash trace and pinned by tests, but nobody has confirmed the IDE
   actually starts on that machine. Worth asking that user before RC3 ships —
   it is the fix most likely to matter to strangers. Do not record it as verified.
6. **STILL OPEN — group resize knobs.** With several controls selected, the
   per-control resize handles should become ONE set of handles around the
   selection's bounding box, resizing all of them together; deselecting restores
   the individual handles (operator, 2026-08-21). **Not started.** Everything it
   needs is in place — the selection is correct, the drag is rigid, and
   `set_property_multi` already fans an edit out — so this is the overlay and a
   group `apply_resize`, not new plumbing.
7. **Routing buttons 4 and 6 are NOT a code defect** — see *“Buttons 4 and 6”*
   below. Both failures are credentials, measured against the live service.

---

## State of the tree

`main` is at **1.61.127**, **four commits ahead of `origin/main` — nothing is
pushed.** On top of that, **this session's work is uncommitted in the working
tree** and brings the source to **1.61.130**: three distinct changes, listed
below, that want **three separate commits**.

The São Paulo window (Mon–Fri 09:00–18:00) was open for the whole of this
session, so GOLDEN RULE #1 blocks both the commit and the push. Finished work
staying uncommitted inside the window is the correct state, not an oversight.

```bash
git push origin main          # after 18:00 -03, or at the weekend
```

Working branch: **`fix/session-2026-08-20`** (already merged into `main` with
`--no-ff`).

### The seven commits, and which files each takes

GOLDEN RULE #5 forbids mixing a feature with fixes, so the tree has to be split.
The file sets barely overlap; only `CHANGELOG.md` and `version.rs` are shared,
and the changelog's seven sections are adjacent hunks at the top of the file.

⚠️ **1.61.134 is five distinct fixes** that arrived across two sittings (pack
manifest, theme-switch defaults, Maps face, TextBox hint, corner mask). If the
operator wants them separated before the forum post, the file sets are almost
disjoint — say so before committing rather than after.

| # | Kind | Version | Files |
|---|---|---|---|
| 1 | **Feature** (see the classification question below) | 1.61.128 | `crates/cobolt-ide/src/{toolchain.rs,ui_prefs.rs,main.rs}`, the `show_toolchain_prompt` + `CoboltApp::new` hunks of `app.rs`, the `rust_check_*` hunks of `i18n.rs`, `docs/developers-guide-{en,es,pt,jp,cn}.md`, `docs/BUILDING*.md` |
| 2 | **Fix** (incomplete platform reference) | 1.61.129 | `crates/cobolt-compiler/src/lib.rs`, `crates/cobolt-ide/src/grace_host.rs`, `assets/knowledge/chunked.data` |
| 3 | **Fix** (route fidelity) | 1.61.130 | `crates/cobolt-forms/src/map_geometry.rs`, `crates/cobolt-runtime/src/maps_bridge.rs`, the two Maps hunks of `docs/developers-guide-en.md` |
| 4 | **Fix** (async reads refused) | 1.61.131 | `crates/cobolt-forms/src/model.rs`, `crates/cobolt-ide/src/{agent.rs,panels/editor.rs,panels/designer.rs}`, the `property_reference`/`Runtime Properties` hunks of `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`, the async note in `docs/developers-guide-en.md` |
| 6 | **Fix** (black captions after a theme switch) | 1.61.133 | `crates/cobolt-forms/src/paint.rs` |
| 7 | **Fix** (pack skinned with mock-ups; theme switch left residue) | 1.61.134 | `assets/themes/neumorphic/theme.toml`, `crates/cobolt-forms/src/{theme_pack.rs,model.rs}`, `crates/cobolt-ide/src/panels/designer.rs` |
| 8 | **Fix** (grey wedge at a rounded corner with a shadow) | 1.61.135 | `crates/cobolt-forms/src/{paint.rs,render.rs}`, the notch-mask hunk of `crates/cobolt-ide/src/panels/designer.rs`, `docs/developers-guide-{en,es,pt,jp,cn}.md` |
| 9 | **Fix** (dark hair on a running map's corners) | 1.61.136 | the `restore_container_outline` hunk of `crates/cobolt-forms/src/paint.rs`, its tests in `render.rs` |
| 10 | **Fix** (Cmd/Ctrl+click built no multi-selection) | 1.61.137 | the selection hunks of `crates/cobolt-ide/src/panels/designer.rs` |
| 11 | **Fix** (properties pane ignored a multi-selection) | 1.61.138 | `crates/cobolt-ide/src/panels/properties.rs`, the multi hunks of `designer.rs` + `app.rs`, 4 `props_multi_*` keys in `i18n.rs` (6 langs), `docs/developers-guide-{en,es,pt,jp,cn}.md` |
| 12 | **Fix** (a dropped selection lost its spacing) | 1.61.139 | the `MovingControls` release branch of `crates/cobolt-ide/src/panels/designer.rs` |
| 13 | **Fix** (IDE would not start on a clean Windows 11) | 1.61.140 | `crates/cobolt-ide/src/main.rs`, `crates/cobolt-form-host/src/{lib.rs,host.rs,shell.rs}` |
| 14 | **Fix** (new project accused of an older build) | 1.61.141 | `crates/cobolt-ide/src/{app.rs,project_model.rs}` |
| 15 | **Fix** (`IF ctl::Bool = FALSE` always true; one boolean spelling) | 1.61.142 | `crates/cobolt-forms/src/{model.rs,render.rs}`, `crates/cobolt-runtime/src/interpreter.rs`, `crates/cobolt-form-host/src/seeding.rs`, `crates/cobolt-runtime/tests/test_{async_rest,visible_property}.rs` |
| 16 | **Fix** (`SET x::prop TO` bypassed the canonicaliser) + `COBOLT_EVENT_TRACE` | 1.61.143 | `crates/cobolt-runtime/src/interpreter.rs`, `crates/cobolt-forms/src/diagnostics.rs`, `crates/cobolt-ide/src/{debug_settings.rs,i18n.rs}`, `crates/cobolt-form-host/src/{diagnostics.rs,host.rs}` |
| 17 | **Fix** (one click ran a handler twice) | 1.61.144 | the Switch arm + `control_pointer_events` in `crates/cobolt-forms/src/render.rs`, the probe tests in `cobolt-form-host` |
| 5 | **Feature** (keyless road routing) | 1.61.132 | `crates/cobolt-runtime/src/{ors_bridge.rs,lib.rs,interpreter.rs,http_runtime.rs}`, the Maps method-table hunks of `crates/cobolt-compiler/src/lib.rs`, `crates/cobolt-ide/tests/maps_demo_compiles.rs`, `crates/cobolt-forms/tests/test_maps_demo_form.rs`, `assets/knowledge/chunked.data`, the `TraceRoad` section of `docs/developers-guide-en.md` — **plus the demo itself, which is outside this repo** (`~/Documents/PowerDemo3/forms/Inner-Forms/maps-demo.cfrm`) |

`version.rs` carries **1.61.134** (the last change). Committing in order means
stepping it `.128 → .129 → … → .134` — or, if the operator prefers, committing
everything under one version and saying so in the message. Announce the two
**features** (1.61.128, 1.61.132) on **f=96** with the `[Noticia]` prefix and
the **fixes** on **f=97**.

### The sweep

```bash
cargo test -q --workspace --exclude cobolt-bench --features cobolt-forms/render \
  --no-fail-fast -- --skip generated_binary_source_actually_compiles --skip external_crates
```

**117 suites, 2216 passed, 0 failed, exit 0.**

(2208 after 1.61.134, plus **2** for 1.61.135 — the corner guard
`a_rounded_maps_corner_keeps_the_shadow_the_mask_paints_over` and the env-gated
dump `measure_maps_corner_notch_painters`, in `render.rs::maps_corner_tests`;
**2** for 1.61.136 — `restore_outline_skips_a_control_whose_face_draws_none` and
the env-gated stroke dump `measure_maps_corner_strokes`; **1** for 1.61.137 —
`modifier_click_adds_to_the_selection_and_clicking_again_removes_it`; **2** for
1.61.138 — `setting_a_property_on_a_multi_selection_changes_all_of_them_in_one_step`
and `a_mixed_selection_shares_only_what_its_types_agree_on`; and **1** for
1.61.139 — `dropping_a_selection_keeps_its_spacing_on_an_off_grid_pitch`.)

The arithmetic closes exactly: the previous session's **2159**, plus the **49**
tests added here — 15 in `toolchain`, 1 in `ui_prefs`, 1 in `cobolt-compiler`
(the platform-reference coverage guard), 5 in `map_geometry`, 4 in
`maps_bridge`, 7 in `ors_bridge`, 5 in `model` (runtime properties + the
theme-switch rules), 3 in the IDE's property gates, 1 in the designer (theme
undo), 5 in `paint` (neumorphic ink, Maps shadow, the TextBox hint, the Maps
face), 1 in `render` (the notch-mask rule), 1 in `theme_pack` (the mock-up
guard), and 1 new suite (`maps_demo_compiles`) — the 117th.

⚠️ **Seen once, and not reproduced:** an earlier run of the same sweep aborted
`cobolt-agents --lib` with **SIGSEGV** at test 46 of 53. Run alone it was 53/53
in 4 s, and the final sweep above completed it normally. Nothing this session
touches `cobolt-agents`; the crate loads an embedding model, and the abort has
the shape of a parallel-run resource problem (83 GB disk and 46 % memory free
when checked). If it recurs, it is a real latent bug in the embedder under load,
not a flake to skip.

Two exclusions, both deliberate:

* `generated_binary_source_actually_compiles` — points a nested `cargo build` at
  the workspace's own `target/`, pruning artefacts the outer run needs. Run it
  alone.
* `external_crates_*` (2 tests in `cobolt-compiler`) — fail on this machine with
  `could not compile libsqlite3-sys`. **Environmental, not ours**; they also
  failed before any of this work and pass in some runs. Do not chase them
  without first confirming the failure is not `libsqlite3-sys`.

Note: `resolve_main_tests::an_unchanged_program_recompiles_nothing` runs a real
nested `cargo build` and takes minutes. It is not hung.

### Left dirty on purpose

Five asset deletions are **unstaged and uncommitted**:

```
assets/drawings.svg
assets/images/screenshots/indexed-file-editor.{jpg,png}
assets/images/screenshots/indexed-inspector-{current,layout}.jpg
```

They were deleted by an earlier session and the removal could not be verified as
intended, so every commit has excluded them:

```bash
git add -A && git reset -- assets/drawings.svg assets/images/screenshots/
```

**Operator decision, still open:** `git checkout -- assets/` to restore them, or
stage and commit if the deletion was meant. Keep excluding them until they say.

---

## What is in the tree, uncommitted

### 1 · The first-run Rust toolchain check (1.61.128) — DONE

The operator's ask: on first run, check whether Rust is installed and meets a
minimum version; if not, offer to install it; **if the user declines, ask a
second time**, warning what they lose.

| File | What |
|---|---|
| `crates/cobolt-ide/src/toolchain.rs` | **new** — version parsing, detection, the installer, the two-stage prompt state machine, 15 unit tests |
| `crates/cobolt-ide/src/app.rs` | the probe in `CoboltApp::new`, `show_toolchain_prompt`, wired into `update`'s dialog block |
| `crates/cobolt-ide/src/ui_prefs.rs` | `rust_check_done` in `ui.toml` + `rust_check_done()` / `mark_rust_check_done()` |
| `crates/cobolt-ide/src/i18n.rs` | 14 `rust_check_*` keys in all six languages; five added to the `sample()` translation guard |
| `crates/cobolt-ide/src/{main.rs,version.rs}` | module registration; 1.61.127 → **1.61.128** |
| `CHANGELOG.md` | new `### Added` section |
| `docs/developers-guide-{en,es,pt,jp,cn}.md` | §3 gained *The first-run Rust check* |
| `docs/BUILDING{,-es,-pt,-fr,-jp,-cn}.md` | a note under the requirements table |

Decisions worth not re-litigating:

* **The minimum is read from the workspace manifest** (`include_str!` of the
  root `Cargo.toml`, `rust-version = "1.92"`), not written into the IDE. A unit
  test fails if the manifest ever stops declaring it, so the fallback constant
  can never quietly become the real rule.
* **No "am I the packaged app?" test exists, and none is needed.** Nothing is
  shown when Rust is present and recent enough, and a developer running
  `cargo run` has it by definition. The packaged first run is simply the only
  situation where the answer can be *no*.
* **PATH is part of the answer.** A Finder/Explorer-launched app inherits the
  desktop session's PATH, not the shell's, so `~/.cargo/bin` is invisible to it
  — and `cobolt-compiler` spawns a bare `Command::new("cargo")`. Detection
  probes PATH, `$CARGO_HOME/bin` and `~/.cargo/bin`; when the `rustc` that
  answers is outside PATH, its directory is prepended to the process PATH so the
  `cargo` Build spawns can be found. This runs on **every** start (one cheap
  probe), while the *question* is first-run only.
* **`ensure_on_path` is called from the UI thread, never the installer thread.**
  The environment is process-wide; writing it from a worker while the rest of
  the IDE reads it is the pattern Rust 2024 makes `unsafe` for good reason.
  `InstallOutcome::program` carries the path back for the UI thread to apply.
* **The command shown is the command run** — `install_command()` is both the
  string in the dialog and the last argument of `install_argv()`, with a test
  pinning that. Nothing installs on its own: only the button spawns it.
* Declining twice settles it permanently (`rust_check_done` in `ui.toml`).
  Quitting without answering does not — the next run asks again from stage one.

**Open question for the operator: is this a FIX or a FEATURE?** It is recorded
as a feature (a new dialog and a new action, so f=96 with the `[Noticia]`
prefix). The argument for calling it a fix is the operator's own rule — a
packaged IDE that cannot compile and does not say so until Build is arguably
debt. Settle this before the commit, because it decides the branch, the commit
split and which forum the announcement goes to.

**Not verified in a running IDE.** On this machine `rustc 1.95.0` is on PATH, so
detection returns `Ok` and the dialog is never raised — which is the designed
behaviour, and also means the dialog's *appearance* has not been eyeballed. To
see it, temporarily point the minimum above the installed version, or delete
`rust_check_done` from `<data_dir>/cobolt/ui.toml` on a machine without Rust.
The IDE has not been rebuilt `--release`, so the installed bundle still runs
1.61.127.

### 2 · The platform reference's three missing subjects (1.61.129) — DONE

The operator chose the **KB-first** route (see the architecture note under
*Outstanding work*). Three documents were added to
`publish_system_documentation` in `cobolt-compiler/src/lib.rs`, alongside the
five that were already there:

| Document | Covers |
|---|---|
| `form_themes.md` | what a theme is, the catalogue, resolution order (form → project → `liquid-glass`), self-contained themes and why `GlassStyle` is inert on one, what a theme never overrides, the asset-pack manifest |
| `form_layout_and_events.md` | the layout model, `Anchor` as a **design-time drag lock, not edge anchoring**, `StartPosition`, all 68 form events, `onDeactivate` vs `onDestroy`, hosting (`Standalone`/`Embedded`/`Both`), the shell, `me` / `super` |
| `project_model_and_settings.md` | the manifest is `<Name>.project.toml` (`cobolt.toml` is legacy), the tracked file lists, the seven tree categories, every per-project section |

Also changed:

* `grace_host.rs` — `ESSENTIAL_SYSTEM_DOCUMENTS` now lists 7, and the
  `KNOWLEDGE_PRECEDENCE_CONTRACT` **names the new subject areas** so an agent
  knows the platform has an answer to search for instead of reaching for general
  GUI knowledge. Retrieval is automatic (excerpts are injected, with
  `knowledge.search` for what they miss), so this contract — not a "search
  first" instruction — is where prompt work belongs.
* `assets/knowledge/chunked.data` rebuilt: **1,201 records from 8 documents**,
  8.96 MB.

Two facts to keep in mind when extending this:

* **Each `##` section must stand alone.** The chunker makes one record per
  heading and a hit injects that record, not the document around it.
* **A new test earns its keep already** —
  `the_platform_reference_covers_themes_layout_and_the_project_model` asserts
  that every form event the model supports appears in the reference and that
  every published document is named in the KB's own sibling list. It caught
  eight events missing from the first draft.

To rebuild the index after any further doc change:

```bash
cargo run --release -p cobolt-ide --example build_chunked_kb
```

(`--release` matters — the embedder in a debug build is unusably slow.)

### 3 · A traced route that sits on the road (1.61.130) — DONE

The operator's screenshot showed a Madrid → Granada trace that plainly left the
motorway. Two separate causes, and only one of them was ours:

* **The credentialed path was reading the wrong copy of the geometry.**
  `Directions` returns `overview_polyline` — Google's *simplified* line, meant
  for a thumbnail — and that is what field 6 carried. The shape that follows the
  road is one polyline per navigation **step**. Encoded pieces cannot be joined
  as text (each is delta-encoded from its own origin), so `map_geometry` gained
  `encode_polyline` (the exact inverse of the decoder it always had) and
  `maps_bridge::road_polyline` decodes → joins → re-encodes, dropping the
  endpoint each step shares with the next. No steps ⇒ the overview line still.
  **Fidelity is spent, not free**: field 6 is UNSTRINGed into an item the
  developer sized (the guide's example declares `PIC X(4096)`), and full step
  geometry for a 400 km route is tens of kilobytes — which would have truncated
  into a route that stops in the middle of nowhere. So it is fitted to a
  **4,000-character budget**: full detail when it fits, otherwise
  Ramer-Douglas-Peucker (`simplify_polyline`, longitude scaled by cos(lat))
  until it does. There was no bound before at all — `overview_polyline`'s length
  is limited by nothing — so this makes an existing silent-truncation hazard
  impossible rather than introducing one.
* **The keyless path is a hand-typed waypoint list and always was.** The demo's
  button 3 draws eight coordinates "down the A-4/A-44". The map draws every
  point and invents none, so eight points cut every curve between them. **This
  is not a bug and no setting fixes it** — see the operator question below.

The guide's Routes bullet now states the rule plainly: a route is exactly as
close to the road as the points given to it.

### 4 · Reading an async answer was refused as invented (1.61.131) — DONE

The operator hit *"Control 'MAP-1' has no property 'ResponseBody'"* on saving a
handler, and asked whether properties had been invented. **They had not.**
`ResponseBody` is where every async method on `Maps`, `RestClient` and
`WebSearch` delivers its answer — written by the interpreter, documented in the
KB's method text and in the guide, and used by the Maps demo.

The gate was wrong, in a way worth remembering: **`property_names_for` is built
from `Control::new`, which seeds only what the DESIGNER can set.** An async
answer is written by the runtime, so it is not seeded — and three separate
places judged a *read* against that settable list:

* `panels/designer.rs::validate_handler_members` (the save-time gate the
  operator hit), through `editor::build_known_controls`;
* `agent.rs::unknown_property_ref` (the change-set gate);
* IntelliSense, which never offered them.

`cobolt_forms::model::runtime_property_names_for` now names them —
`ResponseBody`, `StatusCode`, `Busy`, `LastError`, plus `SelectedMarkerId`,
`SelectedRegionId`, `HoveredMarkerId`, `HoveredRegionId` on Maps — and reading
is judged by `property_readable` (settable ∪ runtime) while **writing** still
uses `property_valid`. A genuinely invented property is still refused, which a
test pins alongside the positive case.

The reference had the identical hole: it listed settable properties and said
*elsewhere* that a method "delivers its answer in `ResponseBody`". Each control
now carries a **Runtime Properties (read-only)** section, and `ResponseBody` /
`StatusCode` / `LastError` got `property_reference` entries for the first time.
Index rebuilt: **1,217 records**.

> **The lesson for the next session:** when something looks like an invented
> property or method, check whether the RUNTIME sets it before assuming the code
> is wrong. `grep 'obj_set(obj, "Name"' crates/cobolt-runtime/src/interpreter.rs`
> is the fastest test.

### 5 · A road route with no Google key — OpenRouteService (1.61.132) — DONE

The operator settled the provider question: **OpenRouteService**, and *"ask the
user to enter the api key in the form, instead to hardcode it or save it in a
file (this will change when we start to support local/cloud vaults in golden
version)"*.

`TraceRoad(apiKey, fromLat, fromLng, toLat, toLng)` on the Maps control —
async like every other data method, answering `metres⇥seconds⇥polyline` in
`ResponseBody` on `onComplete`, fitted to the same 4,000-character budget as a
`Directions` answer.

* **`crates/cobolt-runtime/src/ors_bridge.rs`** (new) — the POST, the response
  parse, and the failure text. Reuses `http_runtime::agent` (now `pub(crate)`)
  so there is **one** TLS setup: a module that built its own and forgot the
  native-tls connector would fail with "no TLS backend" in production only.
* **`interpreter::spawn_ors_op`** mirrors `spawn_maps_op`, except the key comes
  from `args[0]` rather than `_ResolvedMapsApiKey`. Blank key ⇒ `LastError` +
  `onError`, no thread, no request.
* Registering the method in `control_method_docs` was enough for the IDE too —
  `panels/editor.rs::methods_for_type` merges that table in, so IntelliSense and
  the save-time gate picked it up with no second edit.

Two traps worth keeping in mind:

* **Coordinates go out as `[lng, lat]`.** ORS follows GeoJSON's axis order, the
  reverse of everything else here. Done in one function, pinned by a test.
* **Two services, one event, two shapes.** `Directions` answers with seven
  fields and `TraceRoad` with three, both on `onComplete`. The demo records
  which it called in `WS-PROVIDER` and branches; a handler that assumes seven
  reads metres as a distance *text*.

**The demo now shows all three lines at once** — blue the corridor written by
hand, green Google's road, orange OpenRouteService's — with a masked
`TXT-ORS-KEY` field the operator types into. A test asserts that field ships
**empty and masked**, so a well-meaning "fill in the default" edit cannot commit
a credential.

> **The demo lives outside this repository** (`~/Documents/PowerDemo3/`), so its
> changes are not in `git status` and will not be part of any commit here. Its
> generated `generated/maps-demo.cbl` is stale until the IDE next opens the form
> — it is regenerated on demand and read-only, so that is normal, not damage.
> `crates/cobolt-ide/tests/maps_demo_compiles.rs` (new) generates the program
> from the `.cfrm` and runs the parser and the semantic analyser over it, which
> is how the demo's COBOL was verified without opening the IDE.

---

## Outstanding work

### The corner leak — CLOSED (1.61.135)

**Symptom.** In the **run form**, a Maps control with a corner radius showed a
grey wedge at each rounded corner, easiest to see with a large `ShadowDistance` /
`ShadowBlurStrength`.

**The measurement came first this time, and it paid.** A shape-dump scene built
from the operator's *real* `.cfrm` values (`maps_corner_tests::maps_scene` in
`render.rs` — 880×700 at (32,96), `CornerRadius` **51** not 34, shadow SE/7/14 at
33 %, Neumorphic Light, backdrop `EAEBEFFF`) printed every painter covering a
point inside each corner notch, **in paint order**. The answer, in one dump:

```
NW notch (34,98)
  0. RECT [0 0 1280 860]      #eaebefff   <- form backdrop
  1-5 RECT (expanding, offset NW)         <- the neumorphic light halo
  6. RECT [32 96 912 796] r=0 #c8c8c8ff   <- map_tiles.rs:541, the SQUARE basemap
  7. MESH bbox=[32 96 912 796] #eaebefff  <- the notch mask, painting LAST
```

So the hypothesis was right in outline and incomplete in the part that mattered:
the mask does erase the halo, **and the thing it exists to trim is the map's own
radius-0 basemap fill**, not a child. The final colour in every notch was
`EAEBEF` — the form's flat backdrop — while the same shadow survived one pixel
outside the bbox, because the mask is clipped to the control's rect. That
discontinuity *is* the wedge.

**The fix.** `draw_container_notch_mask` now takes the control's own
`ShadowStack` and re-composites it on top of the backdrop it repaints:

* Both shadow painters (`draw_regular_drop_shadow`, `draw_neumorphic_shadow_only`
  — and `draw_glass_neumorphic`, whose raised branch was the same math copied)
  build their layers through **one** definition, so what the mask puts back
  cannot drift from what was drawn. That was the whole design constraint: a
  second derivation of the same geometry is how this project keeps producing two
  painters that quietly disagree.
* The notch is tessellated as a **radial grid** (`push_notch_rings`) rather than
  a fan. A fan's triangles all share the bbox corner, so a per-vertex colour gets
  two samples along the radius — fine for a flat repaint, useless for a falloff.
* `control_shadow_stack` reads the control's **own** properties rather than the
  egui temp store, because the mask runs after the whole control loop and that
  store holds whatever the *last* control published.
* The re-composite uses the alpha the control was **drawn** with
  (`control_alphas`, collected in the render loop — `tf.alpha` exists nowhere
  else), so a faded or animating control does not get a full-strength shadow.
* Wired at **both** call sites, `render.rs` and `panels/designer.rs`.

**The guard:** `a_rounded_maps_corner_keeps_the_shadow_the_mask_paints_over`.
It asserts the invariant rather than any painter — for 66 points in the notch,
the composited colour must match *what was painted before the control's own
face* (found by cutting the shape list at the first shape that fits inside the
control's rect, so no painter is named). Proven to fail on the broken code:
reverting just the `shadow` argument takes it to **208 levels** out, with every
failing point reading exactly `#eaebef`. With the fix the worst point is 20,
which is the ramp-vs-staircase error of sampling a layered shadow on a grid.

> **Still true, and the trap that cost the previous session the most:** a control
> is drawn by **two paths** — `paint.rs` (designer canvas) and `render.rs`
> (run/preview). A fix in one is invisible in the other.

**The follow-on the operator found (1.61.136).** With the wedge gone, a thin dark
hair traced each corner — **in the run form only, never in the RAD**, and that
asymmetry was the whole diagnosis. A stroke dump (fills and meshes cannot see a
hair) showed all 8 strokes in the frame clipped to the four corner squares: a
1.4 px glass rim plus an **opaque 1 px `#8c8ca0` border**. Both come from
`restore_container_outline` — which `panels/designer.rs` **never calls**, hence
the clean canvas. A Maps face paints halo → gradient → tiles and returns before
any rim or border, so there was no outline to restore and the "restore" invented
one on the arcs alone. Restore is now limited to the types whose face draws an
outline (`Panel`/`GroupBox`), stated positively so a new masked type must opt in.
Pinned by `restore_outline_skips_a_control_whose_face_draws_none`.

**Known remaining:** the same defect exists in principle for a **Panel/GroupBox**
that has a shadow *and* children reaching its corners — it goes through the same
mask, and `control_shadow_stack` answers for it too, so it is fixed by the same
change. What is **not** covered is the extra soft shadow `draw_glass` paints for
Classic/Enhanced surfaces, which is not part of either stack; nobody has reported
a wedge there, and adding it blind is exactly the kind of unverifiable fix this
file keeps warning about.

### Skills + KB for every component, and prompts that use them — KB HALF DONE

The operator's words: "well structured and detailed skills for each control,
theme, layout, form's behaviour, and project settings. Make sure the KB contains
all information regarding these components too, and the agent's prompts are
designed to use it (skills, KB, eventual tools etc)."

**Read this before writing forty markdown files.** The architectural concern was
raised with the operator and drew no objection, so this is the agreed shape:

> `agent::load_agent_references` (behind `load_skills`) concatenates **every**
> `.md` under an agent's skills directory into **every** request. Two skills is
> fine; forty would inject all forty on every call — tens of thousands of tokens
> per request, competing with the form inventory and the conversation, costing
> money per call and *reducing* answer quality by burying the relevant part.

So:

* **KB (retrieval, unbounded):** the exhaustive per-control, per-theme,
  per-layout, per-setting reference.
* **Skills (always injected, deliberately few):** one per *recurring failure
  mode*, not one per control. The Maps skill (`MAPS_SKILL` in `agent.rs`) is the
  model: it exists because three specific mistakes kept recurring.
* **Prompts:** teach the agents to search the KB first, and to say when they
  have not.

If the operator later insists on literal per-control skill files, gate the
loading so only relevant ones inject — do not simply add them to the directory.

#### Coverage audit (done this session, so the next one starts from facts)

The System KB is five documents generated from Rust constants in
`cobolt-compiler/src/lib.rs` and published to `~/PowerRustCOBOL/Knowledge Base/`.
Measured against what the operator named:

| Asked for | State | Where |
|---|---|---|
| **Controls** | **Covered.** 1,793 lines, every property with type, default, domain; plus the closed method vocabulary (193 lines). | `form_designer_controls.md`, `control_methods_reference.md` |
| **Themes** | **Thin.** Three bullets on the `GlassStyle` ↔ `Theme` interaction and the "explicit properties always win" rule. **No catalogue** of the themes that exist (`assets/themes/` holds `cobalt-steel` and `neumorphic`; `elegance` is named in prose), what each owns, which are self-contained, or how one is authored/installed. | `ide_functionalities.md` lines 15–17 |
| **Layout** | **Gap.** Property rows only (`X`/`Y`/`Width`/`Height`/`Anchor`, one line on container hierarchies). Nothing on the layout model — parenting, anchoring semantics, tab order, the ContentPane/shell. | — |
| **Form behaviour** | **Thin.** `StartPosition` is well covered; `OnFormLoad`, `MainForm` get one mention each. The form lifecycle and the application shell (specs 037/049/051) are essentially absent. | `ide_functionalities.md` |
| **Project settings** | **Gap.** `cobolt.toml` appears **zero** times. Nothing on the project model, the five tree categories, `IdeSettings`, or build settings. | — |

**That audit is the BEFORE picture — all five rows are addressed by change 2
above.** What remains of the operator's original ask:

* **The skills half is deliberately still small.** Only the Maps skill exists,
  and the KB now carries the reference material that per-component skills would
  otherwise have had to hold. If per-component skills are wanted anyway, the
  loader has to be gated **first** — `load_agent_references` concatenates the
  whole directory into every request — because adding files without that is
  exactly the failure described above.
* **A skill earns its place when a mistake recurs**, on the Maps model: that one
  exists because three specific errors kept coming back. Two candidates to watch
  now that the facts are documented: closing files in `onDeactivate` instead of
  `onDestroy`, and expecting `Anchor` to behave like edge anchoring. Whether
  agents still get those wrong is the evidence that decides.

---

## What is in the tree, uncommitted (continued)

Changes 6–9 all landed in the session's second sitting, after the operator ran
the IDE and reported what they saw. They are in the tree exactly like 1–5 above;
they are here rather than beside them only because they arrived later.

### 6 · Black captions after switching to an asset-pack theme (1.61.133) — DONE

The operator switched the Maps demo from `elegance` to the `neumorphic` pack
with `glass-style="Neumorphic Light"` and every caption on the form went black
on a dark ground — unreadable at once.

`paint.rs` defaulted the Neumorphic register's ink to a flat `Color32::BLACK`,
commented *"black text on light surface"* — an assumption about a surface the
code never looked at. It holds for a control that paints the register's light
face and fails for the one that paints **none**: a Label is frameless, so its
text lands on the FORM's backdrop, which `form_backdrop_of` has been publishing
all along. `neumorphic_default_ink` now derives it with `readable_ink`, the same
rule the map info window settled — the control's own face when it paints one,
the backdrop when it does not. Over a light form the ink stays dark, so this is
a repair, not a reversal.

**Elegance hid it rather than lacking it:** self-contained themes close the
glass gate, so the ink came from Elegance's own palette. Any theme that leaves
the register on would have shown the same thing.

### 7 · The pack skinned controls with mock-ups; theme switches left residue (1.61.134) — DONE

**First diagnosis was wrong, and the correction is the lesson.** From the code
alone it looked like the frameless-Label ink (change 6, which is a real defect
and stays). The operator's designer screenshots showed the labels reading fine
in *both* themes — only the buttons were ruined. Reading the actual PNGs settled
it in one step: `assets/themes/neumorphic/theme.toml` pointed every
`[controls.*]` entry at `<control>/…_ref.png` — **design references**, hundreds
of pixels wide, with placeholder text drawn INTO the picture. Stretched by the
9-slice middle, that fake caption landed inside every real one.

`cobalt-steel` shows the convention: its manifest uses the small top-level
tiles and never references its own `_ref` mock-ups. Neumorphic now does the
same — six control types skinned, the rest falling back to Liquid Glass. A test
(`no_shipped_pack_skins_a_control_with_a_design_reference`) refuses any pack
that skins with a `_ref` image or with insets that cannot stretch.

Worth knowing before touching this theme again: **its six top-level tiles are
byte-identical to cobalt-steel's** — only `background.png` differs, and that one
is genuinely its own (soft light clay, which is what its dark slate palette
foreground is chosen to read on). So the pack has no distinctive control art
yet; it renders as Cobalt Steel with its own background until someone cuts real
slices. The operator knows and chose this over withdrawing the theme.

#### The theme-switch rules (operator, 2026-08-21)

> Open a form in any theme → switch to another → the IDE checks the defaults a
> NEW form would get under the target → applies them to every control and the
> form → no property left behind → **only visuals**; behaviour and content
> (captions, input text) stay.

Implemented as `THEME_OWNED_PROPS` in `cobolt-forms::model` — **exactly the set
the style appliers write**, which is what makes "nothing left behind" checkable
rather than aspirational: anything a switch can stamp, a switch can un-stamp.
Fifteen appearance properties; a property the target does not carry is *removed*
rather than blanked (a `ShadowColor` from Neumorphic Dark used to outlive the
move). `Form::apply_theme_defaults` covers the Theme dropdown, which previously
recorded the choice and applied nothing at all.

Both switches are one undoable step: `StyleSnapshot` gained `theme` and
`Cmd::SetFormTheme` mirrors `Cmd::SetGlassStyle`, restoring through a shared
`restore_style_snapshot`. That closes the second half of the operator's
2026-07-28 complaint (*"if I change the theme I cannot undo it"*) — the style
half was answered then, the dropdown was not.

**Two operator rules that look opposed, and both hold.** The first cut of this
wiped every theme-owned property and broke the *older* rule
(`dark_to_light_style_switch_keeps_label_contrast`, from 2026-07-28: *"a
developer-chosen foreground survives the switch"*). The sweep caught it. They
are reconcilable because they speak about different values — the newer rule is
about the previous THEME's marks, the older about the developer's — so a switch
resets a property only when its current value is one a theme could have
written. `theme_stamped_values` answers that by running the appliers on a fresh
control, rather than a hand-written table that would stop recognising its own
marks as the styles change.

> **Trap that cost a second failing run:** `PropValue::as_str()` returns `""`
> for `Int` and `Bool`. Comparing property values through it makes **every
> numeric property look identical to every other**, so a hand-set
> `CornerRadius` read as theme residue and was wiped. Compare `PropValue`s
> (`prop_values_match`), never their `as_str`.

### 8 · A Maps control had no drop shadow at all (1.61.134) — DONE

Ticking **Drop Shadow ▸ Enabled** on a map did nothing, whatever distance or
blur was set. The Maps branch in `draw_control` painted its gradient and tiles
and `return`ed **before any shadow work**.

Invisible under Classic/Enhanced, where the shared path draws the shadow before
the branch is reached. Under the Neumorphic register it is not: while that
register is on, `drop_shadow_spec` returns `None` for *every* control — the
relief IS the shadow — and each branch draws its own halo. Maps drew neither.
It now calls `draw_neumorphic_shadow_only` before the tiles, as ProgressBar
does; a basemap is opaque, so a halo painted after it would be buried.

`a_maps_control_is_not_excluded_from_drop_shadows` pins the half that is
unit-testable — that nothing excludes Maps from the spec. The halo itself is a
painter call and there is no shape-capture harness here, so that half is
verified by eye like every other control's relief.

### 9 · The designer and the run form drew different things (1.61.134) — DONE

Four operator reports in a row turned out to share one root, and finding it late
cost several wrong fixes. **The runtime `CT::Maps` arm in `render.rs` never drew
the control's face** — it went straight to interaction and `paint_map`. So
everything in Appearance and Drop Shadow applied on the canvas (which draws the
face through `paint.rs`) and vanished when the form ran.

That is why the first shadow fix looked right and did nothing: it went into the
designer's Maps branch, which the runtime never reaches.

The arm could not simply call the face renderer — `draw_control` would paint a
second, static basemap under the live one. The mechanism already existed:
`with_label`, the flag meaning *"this caller draws its own live content"*, used
until now only for captions. The tiles are the same kind of stand-in, so
`draw_control_face` stops before them, and the run arm calls it first, exactly
as the TextBox and ComboBox arms beside it do.

> **Read this before the next Maps bug:** a Maps control is drawn by **two**
> paths — `paint.rs`'s branch (designer canvas, static) and `render.rs`'s
> `CT::Maps` arm (run/preview, live). A fix in one is invisible in the other.
> The same is true of TextBox and ComboBox; they were already wired correctly,
> which is what made them the model to copy.

Also fixed in the same pass:

* **The TextBox placeholder survived typing.** The run path blanked `Text` on a
  clone to silence the static caption — but the face previews `HintText`
  *precisely when `Text` is empty*, so blanking it switched the placeholder ON,
  painted under the live editor. `draw_control_face` silences caption and hint
  together; the editor's own `hint_text` is the one that should show, and now
  the only one.
* **`CornerRadius` did nothing to a map.** Tiles are square and egui clips to
  axis-aligned rects only, so the basemap filled the corners the face rounds
  away. A container earns the notch mask by having *children*; a map bleeds
  through its own tiles and never qualified. `notch_mask_rounding` now answers
  "which corners, if any" for both — in **one** place, because the rule lived at
  two call sites and the corner skill is explicit that both must agree.

#### Still open — the pack's art versus the glass register

The buttons in the operator's screenshot render as **dark embossed pills**,
while the `neumorphic` pack's own art is light clay. Verified statically: all
twenty manifest-referenced images exist, and the pack skin *is* drawn
(`paint.rs`, the `else if let Some((pack, skin)) = &theme_skin` arm). The likely
remainder is that `GlassStyle` paints neumorphic relief **on top of** art whose
slices already carry baked shadows — spec 050 closed that door only for
*self-contained* themes, and this pack does not declare itself one.

**Operator's call, deliberately not taken:** the pack ships art for ~12 of the
34 control types, so `self_contained = true` would leave everything it does not
cover with no relief at all. The alternative is suppressing the register only
for controls the pack actually skins. Either changes how that theme renders for
everyone using it.

---

### Buttons 4 and 6 of the Maps demo — measured, and NOT our bug

The operator reported both routing buttons failing and supplied two
OpenRouteService keys. Measured against the live service, same body/header/
endpoint the bridge sends:

| Key | Result |
|---|---|
| `pub_…` (ORS's newer public token) | **HTTP 403** — `{"error":"Access to this API has been disallowed"}`, plain **and** as `Bearer`. ORS refuses it for the classic v2 Directions API |
| the JWT-style key (`eyJvcmci…`) | **HTTP 200** — a real route, 419 197 m / 15 494 s, with geometry |

So `ors_bridge`'s endpoint, `Authorization` header, POST body and `[lng, lat]`
order are all correct, and **button 6 works with the JWT-style key**. There is
nothing to fix in the bridge; the `pub_` token is simply not valid for that API.

**Button 4 is a different service entirely.** `BTN-DRIVE` calls `Directions`,
which is **Google**, keyed from Settings → Integrations. An OpenRouteService key
can never work there — that is what button 6 exists for. Without a Google key it
fails on `onError`, and the demo's `MAP-1--ERROR` handler puts `LastError` into
`LBL-STATUS`, so the reason should already be on screen.

> ⚠️ Both keys were pasted into a chat transcript and **should be rotated**.
> Neither was written to any file: the standing rule (operator, 2026-08-21) is
> that the key is typed into the form and never stored, and
> `test_maps_demo_form.rs` pins `TXT-ORS-KEY` as shipping empty and masked.

---

## Blocked on the operator — do not attempt

* **Release Candidate 3 — and it is now what carries the forum announcement.**
  Both workflows are `workflow_dispatch` only, `gh` is not installed, and there
  is no GitHub token in the environment (`git push` uses SSH, which does not
  authenticate the Actions API). The operator must run **Actions → "Build
  PowerRustCOBOL (all platforms)"**, tick *Publish*, and set a tag and title.
  Tag and title are separate because a git refname cannot hold a space.
  **The tag should now be `v1.61.139-rc3`, not `v1.61.127-rc3`** — everything
  through 1.61.139 is on `origin/main` as of 2026-08-21 evening, and the
  operator has said RC3 announces all of it in one post rather than one post per
  push. Assemble that post from the CHANGELOG sections 1.61.128 → 1.61.139;
  eleven are fixes and one (1.61.132, `TraceRoad`) is the single feature.
* **Satellite imagery.** Asked for; not built. The tile URL is one format string
  so the code is an hour's work, but the constraint is licensing: Google's
  satellite tiles may only be used through their JS/mobile SDKs, so a key does
  not make raw tile fetching legal. Usable options are Esri World Imagery (free,
  attribution, own terms) or Mapbox Satellite (own key). **The operator must
  pick a provider.**
* **Real-time traffic overlay.** Not possible for us — Google exposes the traffic
  layer only through its own SDKs, never as XYZ tiles. Traffic-aware *duration*
  is shipped instead (field 7 of a `Directions` answer). Do not re-investigate.
* ~~**A road-following route WITHOUT a Google key**~~ — **SETTLED 2026-08-21:
  OpenRouteService**, with the key **entered in the form** rather than
  hardcoded or saved to a file. Shipped as change 5 below. The operator's note:
  *"this will change when we start to support local/cloud vaults in golden
  version"* — so the argument-passed key is the current answer, not the final
  one. When vaults land, the vault becomes the preferred source and this stays
  the fallback.

---

## Gotchas these sessions cost time on

### Git hooks

* A `PreToolUse` hook rejects **the whole Bash command** if it contains
  `git push` during the window — including compound `a && b && push` chains, so
  nothing in the chain runs. Commit and push in separate calls.
* Another hook refuses `git commit` while on `main`, and it reads the branch
  **before the command runs**. `git checkout branch && git commit …` is blocked
  because you are still on `main` at inspection time. Check out in one call,
  commit in the next.

### The System Knowledge Base

* The chunked KB is built from **Rust doc constants in `cobolt-compiler/src/lib.rs`**
  (`publish_system_documentation`, `controls_reference_doc`,
  `methods_reference_doc`), not from `docs/*.md`. Editing the markdown alone
  leaves the KB unchanged and the freshness test red.
* Two guards will catch you, and both are real failures:
  * `cobolt-compiler` `every_control_property_is_documented` — every new control
    property needs an entry in `property_reference()`.
  * `cobolt-ide` `prebuilt_chunked_kb_matches_the_published_documentation`.
* After changing the docs:
  ```bash
  cargo run -p cobolt-ide --example build_chunked_kb
  ```
  Needs the semantic model downloaded; ~1150 records, ~9 MB, into
  `assets/knowledge/chunked.data`.

### Editing `cobolt-forms/src/render.rs` and `panels/designer.rs`

Both contain **mojibake** from an old encoding incident (`â` where an em-dash
belongs). An `old_string` copied through those characters will not match.
**Anchor edits on pure-ASCII lines.** Check `git diff --numstat` after any
scripted edit — and prefer the Edit tool; the repo is Rust-only and shell/perl
editing is against the PRIME DIRECTIVE.

### Testing `cobolt-forms`

It needs `--features render` or its own `model.rs` tests fail to compile
(`crate::paint` is render-gated). `cargo test -p cobolt-forms` alone is a false
failure.

### The guides are not uniformly translated

`docs/developers-guide-{pt,jp,cn}.md` are byte-identical **English** copies of an
old (1,226-line) revision, `-es` is a partial machine translation of the same
old base, and **`-fr` does not exist**; the English canonical is 5,266 lines.
GOLDEN RULE #8 still applies to every *delta*: this session's new §3 section was
written in proper Spanish, Portuguese, Japanese and Chinese into those files even
though their surrounding text is stale English, and French was covered through
`docs/BUILDING-fr.md`, which *is* complete. Clearing the guide debt itself is a
~5,000-line job per language and needs its own session.

### The Maps demo form

The operator asked for `PowerDemo3/forms/maps/`; **the IDE moved it** to
`PowerDemo3/forms/Inner-Forms/maps-demo.cfrm` and generated
`generated/maps-demo.cbl`. `crates/cobolt-forms/tests/test_maps_demo_form.rs`
points at the new path and **skips** when PowerDemo3 is absent, so a fresh clone
does not fail for missing something it was never given.

Writing a `.cfrm` to disk does **not** put it in the project — it must be listed
under `[files] forms` in `PowerDemo3.project.toml` or the IDE tree will not show
it. That cost a round trip.

---

## How this session went wrong, so the next one does not

Four operator reports about the Maps control — no drop shadow, corner radius
doing nothing, corner bleed, appearance vanishing at run time — shared **one**
root: the runtime `CT::Maps` arm never drew the control's face. It was found
last. The three fixes attempted before it were applied to the designer path and
were invisible to the operator, who was looking at the run form.

Two habits would have caught it on the first report:

1. **Grep both draw paths for the control type before believing a rendering fix
   landed.** `paint.rs` and `render.rs` are independent.
2. **Ask which surface — or better, read the code and answer it yourself.** The
   previous session asked the operator which surface they were on when the
   split was findable in two greps.

A related pattern ran through the session's other bugs, worth naming because it
recurs: **a value assumed instead of asked for.** Black ink assuming a light
surface. `overview_polyline` assuming a thumbnail would do. A settable-property
list assuming it described every property. `as_str()` assuming every value is a
string. Blanking `Text` to hide a caption instead of asking for a face without
one. Each shipped because the assumption is usually right.

## Decisions worth not re-litigating

* **"Incomplete functionality is a FIX, not a feature."** The operator corrected
  this twice — for the leaderboard retirement and for the whole Maps
  routes/regions set. A control that ships in the toolbox and cannot do the work
  it exists for is debt, not a missing capability.
* **Empty means "follow the theme".** Every `Info*` property defers when empty.
  Do not give them concrete defaults.
* **The info window's ink is derived, never inherited.** Pure black/white chosen
  by contrast against the resolved background. A softened near-black drops the
  worst case to 4.41:1, under WCAG's 4.5:1 floor — the mid-grey test catches it.
* **Re-using an id replaces that record** for markers, routes and regions. A map
  redrawing itself must not accumulate duplicates it can never move.
* **Maps' two halves have different credential needs.** Basemap, markers, routes
  and regions need **no API key, ever**. Only Geocode / ReverseGeocode /
  Directions / DistanceMatrix / PlacesSearch do. Never tell a developer they need
  a key to draw on a map.

---

## What shipped before this (1.61.115 → 1.61.127)

Read `CHANGELOG.md` for the full text. In brief:

| Version | What |
|---|---|
| 115 | Grace's request-review announces itself (spinner + her own words) |
| 116 | Model Providers details pane scrolls |
| 117 | A form loaded into the ContentPane is drawn in the ContentPane |
| 118 | Opening the documentation no longer panics the IDE (font clobber) |
| 119 | Leaderboard retires decommissioned models; agent reassignment offered |
| 120 | AI errors quote the provider's own sentence at the top |
| 121 | `TRUE`/`FALSE` work as operands everywhere |
| 122 | **The Maps basemap draws at all** (ureq had no TLS backend) |
| 123 | Map pan/zoom: no distortion, controllable zoom, smooth pan |
| 124 | Maps: numeric distance, route tracing, concave region fill, demo form |
| 125 | Designer: selecting a control no longer moves it; multi-drag stays rigid |
| 126 | Maps info window (hover tooltip / click card) + a Maps agent skill |
| 127 | Info-window contrast derived, not inherited; traffic-aware drive time |
| **128** | **First-run Rust toolchain check — uncommitted, in the tree** |
| **129** | **KB: themes, layout/events, project model — uncommitted** |
| **130** | **Route geometry follows the road, within a budget — uncommitted** |
| **131** | **Async answers (`ResponseBody`) readable again — uncommitted** |
| **132** | **`TraceRoad` — OpenRouteService, key from the form — uncommitted** |
| **133** | **Neumorphic ink derived from its ground, not assumed — uncommitted** |
| **134** | **Pack skins, theme-switch defaults, Maps face, TextBox hint, corners — uncommitted** |
| **135** | **A rounded corner keeps the shadow the notch mask painted over — uncommitted** |

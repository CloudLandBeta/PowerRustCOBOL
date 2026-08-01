# Plan — Maps, Knob, Gauge, Switch, FileDropZone, and Web Search controls

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-08-01

## 1. Approach

Six controls, three shapes of work, each anchored on a specific external
crate rather than hand-rolled rendering:

- **Knob / Gauge / Switch / FileDropZone** (R1, R6–R15) wrap real widgets
  from the **egui-elegance** crate (confirmed: targets egui `"0.35"`
  exactly, matching this project's egui/eframe version — zero dependency
  conflict). Each gets a `ControlType` variant, default properties
  (`model.rs`) that map onto the wrapped widget's real builder API (not an
  invented one — see §4 Decision 4 for why the original track-thickness/
  gradient/sunken ask was dropped), designer/preview/runtime rendering
  through the existing shared renderer (`paint.rs`/`render.rs`), a
  Properties-pane section (`properties.rs`), and simple COBOL codegen (a
  `PIC` field per scalar property, no new native `CALL`).
- **Maps** (R16–R20) combines two jobs, neither of which ended up using
  the operator's originally-named rendering crate: the pannable/zoomable
  **OpenStreetMap** tile basemap is hand-rendered directly against this
  project's egui 0.35 (T1's spike found `egui-map-view` hard-pinned to
  the exact, non-unifiable `egui`/`eframe` `0.34.3` — see §4 Decision 1 for
  the full finding and why it's a real blocker, not a soft risk);
  **google_maps** (confirmed: a pure REST API client — Directions/
  Geocoding/Places/Distance-Matrix/Elevation/Time-Zone/Address-Validation —
  with zero rendering of its own, built on `reqwest`+`tokio`) supplies
  location data. Markers are drawn as a `cobolt-forms` overlay on top of
  the hand-rendered tile view, in the same coordinate space the tile
  renderer already computes.
- **Web Search (`WebSearch`, renamed from `AiSearch`)** (R26–R30) is
  structurally a `RestClient` with one fixed endpoint. It reuses
  `RestClient`'s runtime HTTP path (`self.http.get(...)`, the blocking
  `ureq` client already in `cobolt-runtime`) and spec 032's uniform async
  lifecycle (`Mode`, `Busy`, `TimeoutMs`, `Cancel()`,
  `onComplete`/`onError`/`onCancelled`/`onTimeout`) instead of inventing new
  async plumbing.
- **Data binding** (R21–R25) extends two existing enums in
  `cobolt-forms::model` — `ApprovedBindingTargetKind` /
  `BindingTargetDescriptor` — with two new variants (`ScalarControl` for
  Knob/Gauge/Switch, `MarkerCollection` for Maps), plus matching arms in
  `cobolt-ide::data_binding_guardian`. `WebSearch` as an approved binding
  **source** (R23) needs **no new enum variant at all** —
  `BindingSourceDescriptor::RestApi` already covers any REST API response.
  `FileDropZone` deliberately gets **no** Guardian integration (R25) — its
  output is event-shaped, not a bound display value.
- **Credentials** (R31–R33) extend the existing machine-local secret store
  (`cobolt-ide::llm::LlmConfig.api_keys` + `store_api_key`/`api_key_slot`)
  with two new well-known slots (`"google-maps"` for the google_maps crate,
  `"google-custom-search"` for WebSearch) — never OpenStreetMap tiles,
  which need no key at all.
- **Async bridge for `google_maps`** (new, not in the original plan): this
  crate is `reqwest`+`tokio`-based, while `cobolt-runtime` (the interpreter
  used by both IDE-hosted Run and standalone `rcrun`/compiled binaries) is
  synchronous and `ureq`-based, with no `tokio` dependency today. Rather
  than pulling `tokio` through the whole interpreter, Maps' Directions/
  Geocoding/Places/Distance-Matrix calls run on the **same kind of
  background worker thread spec 032 already uses** for RestClient's async
  GET/POST (`spawn_rest_op`) — the worker thread privately owns a small
  `tokio::runtime::Runtime` and `.block_on()`s the `google_maps` call
  inside it, delivering the result back through the existing FormEvent
  channel. The rest of the interpreter never becomes async; only that one
  worker thread pays tokio's cost, and only when a Maps data call is
  actually made.

## 2. Affected crates / files

- **`crates/cobolt-forms/Cargo.toml`** — `egui-elegance = "0.14"`, both
  gated behind the existing `render` feature (`optional = true` +
  `dep:egui-elegance` added to the `render` feature list), alongside
  `egui`/`fontdb`/`skrifa`/`cobolt-media`/`image`/`resvg` — `cobolt-forms`,
  not `cobolt-ide`, is where `paint.rs`/`render.rs` (the shared renderer
  across designer/preview/runtime/compiled binary) already live, so this
  is where a rendering-widget crate belongs, matching where `egui` itself
  is declared. **No `egui-map-view` dependency** — dropped per §4
  Decision 1's T1 finding; Maps' tile rendering is hand-rolled directly
  against the same `egui` the rest of this crate already uses, so it needs
  no separate crate/feature entry at all.
- **`crates/cobolt-runtime/Cargo.toml`** — `google_maps` (the data crate —
  `cobolt-runtime` has no UI, so no `egui-elegance` either) plus a `tokio`
  dependency scoped to the async-bridge worker thread described above.
  This is the one place this plan adds a new dependency to the crate
  compiled into headless `rcrun`/standalone binaries — flagged explicitly
  in §5 Risks (binary size / build time impact).
- `crates/cobolt-forms/src/model.rs` —
  - `ControlType` enum: add `Maps`, `Knob`, `Gauge`, `Switch`,
    `FileDropZone`, `WebSearch` (R1); `Display`/`FromStr` arms;
    `default_size()`; `primary_event()` (`onChange` for Knob/Switch;
    `onFilesDropped` for FileDropZone; none/baseline for Gauge and Maps;
    `onResultsReceived` for WebSearch, following RestClient's
    `onResponseReceived` precedent); `supported_events()` per control (R7,
    R12, R14, R15, R18, R19, R28).
  - Default property blocks (the `props.insert(...)` match, alongside
    `ControlType::Slider`/`ControlType::RestClient` at ~L3282/~L3310):
    - Knob: `Minimum`, `Maximum`, `Value`, `Step`, `Size`
      (`Small`/`Medium`/`Large`), `Accent`, `Bipolar`, `ShowValue`,
      `DefaultValue`, `Label` (R6).
    - Gauge: `GaugeStyle` (`Radial`/`Linear`/`Donut`), `Minimum`,
      `Maximum`, `Value`, `Color` (optional override), `WarningThreshold`,
      `CriticalThreshold`, `Unit`, `Text`, plus style-specific
      `ShowNeedle`/`ShowScale` (Radial), `BarHeight`/`ShowThumb` (Linear),
      `StrokeWidth` (Donut) (R8, R9).
    - Switch: `Checked` (R11).
    - FileDropZone: `Hint`, plus the runtime-only `DroppedFiles` collection
      (not a designer-editable default — populated at runtime only) (R13).
    - Maps: `Center`, `Zoom`, `Markers` (structured, see §3), `ApiKeySource`
      (R17).
    - WebSearch: `SearchEngineId`, `Query`, `NumResults`, `SafeSearch`,
      `Mode`, `TimeoutMs` (R27, R28).
  - `ApprovedBindingTargetKind` (~L1587) and `BindingTargetDescriptor`
    (~L1596): add `ScalarControl { control_id, control_type }` and
    `MarkerCollection { control_id }` variants (R21, R22).
- `crates/cobolt-forms/src/paint.rs` / `render.rs` — thin wrapper rendering
  that calls into egui-elegance's `Knob`/`RadialGauge`/`LinearGauge`/
  `ProgressRing`/`Switch`/`FileDropZone` widgets directly (these are
  drop-in `egui::Widget`-shaped calls, not something to reimplement), plus
  Maps' hand-rolled tile fetch/cache/paint (new code, no crate — §4
  Decision 1) and the hand-written marker overlay layer (R18) on top of it.
- `crates/cobolt-ide/src/panels/properties.rs` — new `ControlType::Knob`,
  `::Gauge`, `::Switch`, `::FileDropZone`, `::Maps`, `::WebSearch` match
  arms (alongside `ControlType::Slider` ~L5018, `ControlType::RestClient`
  ~L5915) rendering each control's Basic/Appearance property rows —
  Gauge's arm branches further on `GaugeStyle` for the style-specific
  fields (R9).
- `crates/cobolt-ide/src/data_binding_guardian.rs` — new match arms for
  `BindingTargetDescriptor::ScalarControl` / `::MarkerCollection` (alongside
  the existing `DataGrid`/`ComboBox`/`ListBox` arms ~L312–352): validate the
  target control exists, is the right type, and (for `MarkerCollection`)
  that the mapped fields cover lat/lng/label.
- `crates/cobolt-ide/src/panels/data_binding.rs` — binding-editor UI: let the
  developer pick a standalone Knob/Gauge/Switch or a Maps control as a bind
  target the same way DataGrid/ComboBox/ListBox are picked today.
- `crates/cobolt-codegen/src/lib.rs` —
  - Knob/Gauge/Switch: no new WORKING-STORAGE block beyond the ordinary
    per-control property fields every scalar control already gets.
  - FileDropZone: a `WS-<id>-FILE-COUNT`/`WS-<id>-FILE-PATH` (indexed)
    style block, modelled on how other multi-row properties are exposed to
    COBOL.
  - WebSearch: a WORKING-STORAGE block modelled on the RestClient block
    (~L296–330) plus a call-stub generator modelled on
    `write_rest_client_stubs` (~L774) — one paragraph (`<id>-SEARCH`) that
    builds the Custom Search URL
    (`https://www.googleapis.com/customsearch/v1?key=...&cx=...&q=...`)
    into `WS-REQUEST-URL` and calls the existing `COBOL-HTTP-GET`
    intrinsic — no new native `CALL` needed.
  - Maps: emits `Center`/`Zoom` as WORKING-STORAGE, `INVOKE`-based
    accessors for `Markers` (add/remove/update, R18) that update an
    in-memory table the runtime reads each frame to reconcile the overlay,
    and `INVOKE`-based Directions/Geocoding/Places/Distance-Matrix
    accessors (R20) that route to a **new native `CALL`**
    (`COBOL-GOOGLE-MAPS-<VERB>`, e.g. `COBOL-GOOGLE-MAPS-GEOCODE`) — unlike
    WebSearch, this genuinely needs a new native intrinsic, since it isn't
    a plain HTTP GET/POST the existing `COBOL-HTTP-*` calls already cover
    (the `google_maps` crate owns request signing/formatting internally).
  - Credential injection at **Build** time only (Decision 3): when a
    project has a Maps or WebSearch control and a Build/Package is run, the
    resolved key is written as a `PIC X(...) VALUE '<key>'` WORKING-STORAGE
    literal — the same mechanism `BaseURL` already uses for RestClient —
    into the **generated** `.cbl`, never into `cobolt.toml` or the `.cfrm`
    (R31 protects the *project source files*, not the build output).
- `crates/cobolt-runtime/src/interpreter.rs` — `INVOKE` dispatch for the new
  controls: Knob's drag→`Value` update (delegates the actual drag math to
  egui-elegance's own `Knob` widget, since it owns pointer/keyboard
  handling directly — the interpreter only reads back the resulting
  `Value`), Switch's click→`Checked` toggle, FileDropZone's drop/click
  handling (also owned by the wrapped widget — interpreter reads back
  `DroppedFiles`), WebSearch's `"SEARCH"` method (mirrors `"GET"` ~L6531:
  `rest_is_async`/`spawn_rest_op` gate, same as RestClient — R28), Maps'
  marker add/remove/update methods and the new `COBOL-GOOGLE-MAPS-*`
  native-call bridge (async-bridge worker thread, described in §1).
- `crates/cobolt-ide/src/form_runtime.rs` — the interpreted-**Run** path
  (R33, the "not configured" state, and the Run-time credential path,
  Decision 3): extend the existing `seed` mechanism (~L269–327, already used
  for `append_data_binding_seed_props`) to inject the resolved google_maps/
  WebSearch key as an in-memory seeded property when a project's stored key
  exists — so an interpreted Run **never** writes the key into generated
  COBOL text at all, only the Build path does.
- `crates/cobolt-ide/src/llm.rs` — two new well-known secret-store slots
  (`"google-maps"`, `"google-custom-search"`) alongside the existing
  `profile_api_key_slot`/`api_key_slot` scheme (R31); `SearchEngineId` (cx)
  is NOT a secret and lives as an ordinary field on `ProjectAiSettings` (or
  a small sibling struct) in `cobolt.toml`, not in the secret store.
- `crates/cobolt-ide/src/panels/settings_form.rs` — new fields for the two
  keys and `SearchEngineId`, in the same section pattern as the existing LLM
  provider key fields (~L140–230, ~L1340–1360) (R32).
- `crates/cobolt-compiler/src/lib.rs` — `publish_system_documentation` (R3):
  six new entries in the property/method/event docs tables (~L1876–2389),
  following the `Slider`/`RestClient` entries already there.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` fields for every new property
  label, section heading, and the "not configured" message (R4, R33), in
  all six languages.
- `docs/developers-guide-en.md` — one section per new control (properties,
  events, COBOL API) plus a project-settings subsection for the two new
  credential fields (English only — translations are user-maintained).
- `CHANGELOG.md` — one `### Added` entry, feature classification (minor
  version bump).

## 3. Data / model changes

- **`.cfrm` schema:** six new `ControlType` values are additive — an older
  IDE build opening a `.cfrm` that contains one simply won't recognise it
  (same as any new control type historically); no migration needed for
  *existing* files, since none can contain these types yet.
- **`Markers` property shape:** stored as a single `PropValue::String`
  holding a small serialized list (id, lat, lng, label, info-window text) —
  reusing the existing convention other multi-row properties use (e.g.
  DataGrid's advanced column metadata under `DATAGRID_ADVANCED_PROP`)
  rather than adding a new `PropValue` variant just for this.
- **`DroppedFiles` shape:** runtime-only (never serialized into the
  `.cfrm`'s design-time property defaults) — a list of (path, byte length,
  optionally the bytes themselves for small files) populated only while a
  form is running.
- **`cobolt.toml` (`ProjectAiSettings` or a sibling struct):** new
  non-secret field `search_engine_id: String` (the Custom Search "cx").
  `#[serde(default)]` so existing project files round-trip unchanged.
- **Secret store (`LlmConfig.api_keys`):** two new well-known keys
  (`"google-maps"`, `"google-custom-search"`) in the same
  `HashMap<slot, key>` LLM provider keys already use — additive, no schema
  version bump needed (the map already tolerates unknown/new slot strings).
- **Generated `.cbl`:** additive WORKING-STORAGE blocks, gated on control
  presence exactly like the existing REST/Agent/Animation blocks (~L247–330)
  — a project with none of the six new controls generates byte-identical
  output to before this change.

## 4. Key decisions & alternatives

1. **Decision — Maps' visual basemap is OpenStreetMap tiles, hand-rendered
   directly against this project's egui 0.35 (not via `egui-map-view`'s
   `Widget`); the `google_maps` crate supplies location data only, never
   rendering.**
   `egui-map-view` was the operator's named choice, and OpenStreetMap
   tiles over Google's unlicensed ones was confirmed with the operator
   (Google does not license raw XYZ map tiles for third-party embedding
   outside their official JS/Android/iOS SDKs or the Static Maps/Embed
   APIs — pointing a generic slippy-map widget at Google's unofficial tile
   endpoints would violate Google's Maps Platform Terms of Service). **The
   T1 spike (2026-08-01) found a hard blocker in using the crate itself,
   though:** `egui-map-view` 0.6.3 — and its `main` branch, checked
   directly, which is not yet even at a published release — hard-pins
   `egui`/`eframe` to the **exact** version `"0.34.3"` (`use eframe::egui;`
   in its `lib.rs`, and its `Map` type implements `egui::Widget` for
   THAT version's `Ui`). Cargo does not unify semver-incompatible minor
   versions of the same crate, so egui-map-view's `Ui`/`Widget`/`Response`
   types are a completely different, non-interchangeable Rust type from
   this workspace's egui 0.35 — `ui.add(&mut map)` inside a `cobolt-forms`
   0.35 `Ui` **cannot compile** against it, full stop, not a "might have
   friction." A real integration would need `egui-map-view` to run its own
   *nested* egui 0.34.3 context each frame, rasterise to an off-screen
   texture, and have the 0.35 code blit that texture in — a second,
   parallel immediate-mode UI stack, for one control's basemap tiles.
   Judged not worth that complexity for what is fundamentally "draw
   256×256 PNG tiles at the right screen position, panned and zoomed" —
   ubiquitous, well-documented slippy-map math with no algorithmic novelty.
   **Revised decision:** drop the `egui-map-view` dependency entirely;
   `cobolt-forms` fetches OpenStreetMap tiles directly (via the HTTP stack
   already in the relevant crate — `ureq` in `cobolt-runtime`-adjacent
   code, or a direct blocking `reqwest`/`ureq` call from `cobolt-forms`
   itself, TBD at T9), caches them as `egui::TextureHandle`s, and
   implements the standard Web Mercator lat/lng↔pixel formulas
   (`lon_to_x`/`lat_to_y`/inverses — the same functions egui-map-view's own
   `projection.rs` uses, independently re-derived rather than imported,
   since even that module's public signatures return egui-map-view's own
   0.34.3 `Pos2`/`Rect`) natively against egui 0.35. This still satisfies
   R16/R18 exactly — pan/zoom/drag OSM tiles plus a marker overlay — just
   without the named crate as a dependency. Flagging clearly: **this
   deviates from the operator's explicit crate choice**, decided
   unilaterally under the "do what you recommend instead of asking, fix
   later if wrong" instruction for this implementation pass, precisely
   because the alternative (a nested second egui context) was
   judged worse, not because the operator's instinct to reach for a
   pre-built widget was wrong in general.
   - *Rejected: nested egui 0.34.3 context + offscreen-texture blit,
     keeping `egui-map-view` as a dependency.* Technically possible but a
     disproportionate amount of new rendering-pipeline machinery (a second
     `egui::Context`, its own font/texture atlas, an offscreen render
     target, a per-frame composite step) to reuse a widget whose actual
     value-add over hand-rolled tile fetching + a Web Mercator formula is
     comparatively small.
   - *Rejected: fork/patch `egui-map-view` to target egui 0.35.* Checked
     upstream `main` directly — even the unreleased branch is still
     exact-pinned to `0.34.3` (not a lagging-published-version situation,
     a deliberate pin), suggesting real API breakage between 0.34 and 0.35
     that the crate's own author hasn't resolved. Open-ended effort with
     unknown size; not undertaken speculatively inside this spike.
   - *Rejected: point tile fetching at Google's unofficial tile URLs
     anyway.* Explicitly offered to and declined by the operator — real
     ToS/account-ban/legal risk for no compensating benefit once
     `google_maps` already supplies genuine Google-sourced location data.
     This rejection stands regardless of the egui-map-view finding above —
     OpenStreetMap tiles are the source either way.
   - *Rejected: Static Maps API image.* Would render genuine Google
     imagery, ToS-compliant — but the operator specifically wanted
     pan/zoom/drag-native interaction over a re-fetch-on-pan static image
     approach; still true after dropping egui-map-view, since hand-rolled
     tiles preserve that native interaction.
   - *Rejected (from the original draft): a native web-view (`wry`).*
     Superseded — no longer needed; hand-rolled OSM tiles plus
     `google_maps` for data cover both halves without any new native/
     browser-engine dependency.

2. **Decision — `WebSearch` needs no new `BindingSourceKind`.**
   `BindingSourceDescriptor::RestApi` already models "a REST API response,"
   with no assumption that it came specifically from a `RestClient` control.
   `WebSearch`'s Custom Search response is classified under the same kind.
   Rejected: a new `WebSearch`/`BindingSourceKind::CustomSearch` variant —
   more code, no behavioural difference.

3. **Decision — Maps/WebSearch credentials use the LLM-style secret store,
   not RestClient's plain `AuthToken` property.**
   `RestClient.AuthToken` is stored as a literal control property, persisted
   in the `.cfrm` in plain text — an existing precedent, but a weaker one.
   The operator explicitly chose the secret-store pattern (project-level
   settings, same mechanism as LLM keys, over a per-control property). A
   deliberate, operator-confirmed divergence from the RestClient precedent —
   noting it so a future reader doesn't "fix" WebSearch/Maps to match
   RestClient's weaker pattern by mistake.

4. **Decision — Knob/Gauge/Donut styling matches egui-elegance's real
   builder API exactly; the original track-thickness/inner-track-color/
   gradient-flat-sunken ask is dropped.**
   Checked each widget's actual public API directly against the operator's
   first-message wording (arbitrary track thickness, arbitrary inner-track
   color, a gradient/flat/sunken effect) and found real gaps: `Knob` has no
   thickness or arbitrary-color control at all (only a `Size` preset and a
   fixed theme `Accent`); none of `Knob`/`RadialGauge`/`LinearGauge`/
   `ProgressRing` support a gradient or sunken effect anywhere. Presented
   this gap explicitly; the operator chose to accept each widget's real
   surface over building a custom paint layer to restore the original
   properties.
   - *Rejected: wrap/fork the widgets to add thickness+gradient+sunken.*
     Explicitly offered and declined — more implementation work, and
     diverges from "use this crate" into "use this crate's interaction
     model, replace its rendering," which is most of the crate's value
     gone for a fraction of its API surface kept.
   - **Consequence:** spec.md's Goals/Non-goals were rewritten to describe
     what each widget actually offers (`Size`/`Accent`/`Bipolar` for Knob;
     `Color`/`GaugeZones` threshold auto-colouring for all three Gauge
     styles) rather than the originally-imagined property set. `GaugeZones`
     (warning/critical threshold auto-colouring) turned out to be a real,
     low-cost capability the crate provides for free — promoted from the
     original spec's "non-goal" (hand-rolled alarm zones) to an in-scope
     goal, since it needed zero extra implementation once the crate was
     adopted.

5. **Decision — `google_maps`'s async Directions/Geocoding/Places/
   Distance-Matrix calls run on a spec-032-style background worker thread
   with a private `tokio::Runtime`, not by making `cobolt-runtime` async.**
   `google_maps` is `reqwest`+`tokio`-based; `cobolt-runtime` is
   synchronous and `ureq`-based today, and must stay that way for
   `rcrun`/headless compiled binaries where pulling the full `tokio`
   executor into the interpreter's hot path would be a much bigger,
   unrelated change. Spec 032 already solved an analogous problem
   (RestClient's blocking `ureq` calls freezing the event loop) with a
   background-worker-thread + FormEvent-delivery pattern
   (`spawn_rest_op`) — reusing that shape, but with the worker thread
   privately running `tokio::runtime::Runtime::new()?.block_on(...)`
   around the `google_maps` call, keeps the rest of the interpreter
   untouched.
   - *Rejected: make the whole interpreter tokio-async.* Enormous blast
     radius (every other control's dispatch, the event loop itself, the
     compiled-binary story) to serve one crate's API shape.
   - *Rejected: hand-roll the Directions/Geocoding/Places HTTP calls
     against `ureq` instead of using `google_maps`.* Throws away the
     chosen crate's request-signing/response-parsing work entirely — the
     tokio dependency is a real but bounded cost (one worker thread, one
     runtime instance, created lazily only when a Maps data call is
     actually made) against reimplementing several Google API surfaces by
     hand.

6. **Decision — Knob's drag gesture, WebSearch's result shape, and Gauge's
   value-changed feedback follow the original spec's Q2/Q3/Q4
   recommendations, now confirmed rather than merely recommended.**
   Q2 (relative drag): moot in practice — egui-elegance's `Knob` owns its
   own drag handling entirely; the plan adopts whatever gesture the widget
   itself implements rather than second-guessing it. Q3 (both top-result
   shortcut and indexed access): implemented as originally recommended
   (R29). Q4 (no new Gauge event): implemented as originally recommended;
   `GaugeZones` (Decision 4) covers the practical need without one.

## 5. Risks & mitigations

- **Risk (RESOLVED by T1, 2026-08-01):** `egui-map-view` pins to egui
  `0.34.3` exactly, including its unreleased `main` branch — this project
  is on `0.35`, and Cargo cannot unify the two. Confirmed a hard compile
  blocker (`ui.add(&mut map)` cannot type-check across the versions), not
  a "might have friction." Resolved by dropping the dependency: Maps'
  basemap is hand-rendered tiles against this project's own egui 0.35
  (§4 Decision 1). No further action needed on this risk; superseded by
  the two risks below, which replace it.
- **Risk:** hand-rolling OSM tile fetch/cache/paint (replacing
  `egui-map-view`, per the resolved risk above) is new rendering-pipeline
  code with no upstream crate backing it — tile-URL construction, an
  `egui::TextureHandle` cache keyed by `(zoom, x, y)`, eviction as the
  viewport pans away from cached tiles, and the paint loop that draws
  visible tiles at the right screen offset all need to be written and
  tested from scratch, where a working crate would have supplied it.
  → **Mitigation:** scope v1 tightly — synchronous/blocking tile fetch on
  a background thread (mirroring the `spawn_rest_op` pattern already used
  elsewhere in this plan) delivering decoded images back for the main
  thread to upload as textures; a simple unbounded-for-v1 texture cache
  (eviction is a follow-up, not a launch blocker, given a single map
  control's tile count at reasonable zoom levels is small); reuse
  `image`(already a `cobolt-forms` dependency) for PNG decode.
- **Risk:** no public lat/lng→screen-pixel projection is being imported
  from any crate (since `egui-map-view` is no longer a dependency) — the
  Web Mercator formula must be implemented and verified independently.
  → **Mitigation:** the standard Web Mercator tile math is well-documented
  and used identically across essentially every slippy-map library
  (OpenStreetMap's own wiki publishes the reference formula); implement it
  as a small, pure, independently unit-tested function
  (`lat_lng_to_pixel(center, zoom, lat, lng) -> (f32, f32)` and its
  inverse) against known reference coordinate/zoom/pixel triples — the
  same test plan §6 already calls for — before wiring it into any actual
  paint code, so a projection bug is caught by a fast unit test rather
  than by a visually-misplaced marker.
- **Risk:** adding `google_maps` (+ a private `tokio` runtime) to
  `cobolt-runtime` — the crate compiled into every headless `rcrun`
  invocation and every standalone binary, whether or not a project
  actually uses a Maps control — grows binary size and build time even for
  projects with zero Maps controls.
  → **Mitigation:** gate the dependency behind a Cargo feature
  (`cobolt-runtime/google-maps`, on by default in `cobolt-ide`'s build but
  something `rcrun`/`cobolt-compiler` could in principle build without) if
  the size/build-time cost proves material during implementation; not
  pre-emptively engineered unless a real measurement shows it's needed.
- **Risk:** baking the Maps/WebSearch API key into generated `.cbl` at
  Build time (Decision 3's Build-path half) means a **distributed compiled
  binary** contains the operator's API key in its static data — anyone with
  the binary can extract it (e.g. `strings`).
  → **Mitigation:** matches the existing `RestClient.BaseURL`-style
  precedent for "what a build embeds"; no stronger mechanism exists in
  this codebase today. Document the exposure plainly in the developer
  guide's Maps/WebSearch sections so the operator/developer can rotate
  keys or apply provider-side usage restrictions (HTTP referrer / IP
  allow-list on the Google Cloud console) — solving the general "secrets
  in compiled binaries" problem is out of scope for this spec.
- **Risk:** extending `ApprovedBindingTargetKind`/`BindingTargetDescriptor`
  touches a validator (`data_binding_guardian.rs`) with real safety
  properties (spec 022's whole purpose) — a sloppy new match arm could
  under-validate a `ScalarControl`/`MarkerCollection` binding and let a
  corrupt binding through.
  → **Mitigation:** model the new arms directly on the existing
  `ComboBox`/`ListBox` arms (~L333–352, structurally closest to
  `ScalarControl`) and `DataGrid` arm (~L312, closest to
  `MarkerCollection`'s row-shaped data) rather than writing new validation
  logic from scratch; the existing Guardian test suite's patterns get
  mirrored for the two new variants, not invented fresh.
- **Risk:** a new native `CALL "COBOL-GOOGLE-MAPS-<VERB>"` intrinsic per
  google_maps API surface (Directions/Geocoding/Places/Distance-Matrix,
  R20) is more new runtime surface than WebSearch needed (which reused the
  existing `COBOL-HTTP-GET` intrinsic unchanged) — more code, more to keep
  in sync with the KB docs (R3).
  → **Mitigation:** implement the four verbs behind one shared native call
  (`CALL "COBOL-GOOGLE-MAPS" USING WS-VERB WS-REQUEST-PARAMS ...`) dispatching
  internally, rather than four separate `CALL` names — mirrors how
  `RestClient` already dispatches GET/POST/PUT through one HTTP layer with
  a verb parameter, keeping the new-intrinsic surface to one entry point.

## 6. Test strategy

- `crates/cobolt-forms` (unit):
  - Gauge ignores drag/click input entirely across all three `GaugeStyle`
    values — `Value` unchanged after a simulated drag (AC2, R10).
  - `WarningThreshold`/`CriticalThreshold` set on a Gauge produce the
    expected `GaugeZones` construction (asserted on the property→widget
    mapping, not on pixel colour) (AC2).
  - Switch click flips `Checked` and fires `onClick` (AC3).
  - `ApprovedBindingTargetKind`/`BindingTargetDescriptor` round-trip
    (serialize/deserialize) for the two new variants — same pattern as
    existing `DataGrid`/`ComboBox` coverage.
  - Marker lat/lng→pixel projection (once resolved per §5's risk) has a
    dedicated unit test against known coordinate/zoom/pixel triples,
    independent of any live map rendering.
- `crates/cobolt-ide::data_binding_guardian` (unit): a standalone Knob/Gauge/
  Switch (no control array) is accepted as a bind target (AC8); a Maps
  control bound to a source missing a required marker field (lat/lng/label)
  is rejected, following the existing malformed-binding rejection tests'
  style; a `WebSearch` response classified as `RestApi` source flows
  through binding validation identically to an existing `RestClient` source
  (R23); confirm `FileDropZone` is absent from the approved-target list
  entirely (R25) with a negative test.
- `crates/cobolt-codegen` (unit): generated `.cbl` for a form with a
  `WebSearch` control contains the `<id>-SEARCH` paragraph and the Custom
  Search URL construction; a form with a `Maps` control contains the
  `COBOL-GOOGLE-MAPS` call site; a form with none of the six new controls
  produces byte-identical output to before this change (regression guard
  on the additive-only claim in §3).
- `crates/cobolt-runtime` (unit): `INVOKE <id> 'SEARCH'` under `Mode =
  Async` sets `Busy`, returns immediately, and later delivers
  `onComplete`/`onError` — mirrors the existing async RestClient GET test
  coverage from spec 032 (AC7, R28). A `google_maps` call routed through
  the background-worker+private-tokio-runtime bridge (Decision 5) delivers
  its result via the same FormEvent channel, tested against a mock/stubbed
  HTTP layer rather than a live Google API call.
- `crates/cobolt-ide::llm` (unit): `store_api_key`/resolution round-trips
  for the two new slots; neither key appears in a serialized `cobolt.toml`
  or `.cfrm` (AC10) — a direct string-search assertion on the serialized
  output, same style as the existing `assert!(!text.contains("api_key"))`
  coverage already in `llm.rs`'s test module.
- **Manual/visual** (required before declaring Maps done, since live tile
  fetching and pointer-driven pan/zoom cannot be meaningfully unit-tested):
  - Launch the IDE, drop a `Maps` control on a form, Run with no
    google_maps key configured: confirm the OpenStreetMap basemap renders
    and pans/zooms correctly (AC5), and that Directions/Geocoding actions
    show the "not configured" state rather than a crash (AC11).
  - Configure a real (test) google_maps API key; confirm
    `INVOKE 'Geocode'` returns real coordinates (AC6).
  - Drag a Knob end to end; confirm `Size`/`Accent`/`Bipolar` all render as
    expected, and Alt+click resets to `DefaultValue`.
  - Set a Gauge's `WarningThreshold`/`CriticalThreshold` and sweep `Value`
    across them from COBOL; confirm the fill colour changes automatically
    at each threshold, for all three `GaugeStyle` values.
  - Toggle a Switch; confirm the visual state swap.
  - Drop a file onto a FileDropZone; confirm `DroppedFiles`/`onFilesDropped`
    fire correctly, then confirm clicking (no drag) opens a native picker
    with the same result (AC4).
  - Bind a standalone Gauge to an Indexed file field; confirm it updates
    live as the underlying file value changes, with no drag/click
    affordance shown.
  - `INVOKE` a `WebSearch` control against a real (test) Google Custom
    Search key; confirm `GetResult`/`TopTitle` etc. return real data, and
    that the example flow from the spec (search → AI Agent summarise → set
    a multiline TextBox) works end to end using only the developer's own
    COBOL.

## 7. Steering compliance

- [ ] i18n: all new property labels, section headings, and the
      "not configured" message added to `Tr` in all six languages (R4).
- [ ] Generated-code banner + regenerate-on-action contract preserved: new
      codegen paths (Knob/Gauge/Switch/FileDropZone/WebSearch/Maps) go
      through the existing `write_header`-banner path, and Build/Run/
      Debug/Check continue to call `regenerate_all_forms` unchanged.
- [ ] English dev guide updated (`docs/developers-guide-en.md`): one section
      per new control + credentials subsection; translated guides
      untouched (user-maintained).
- [ ] Fix vs feature: **feature** — bumps the **minor** version
      (`crates/cobolt-ide/src/version.rs`), one `CHANGELOG.md` `### Added`
      entry, announce on f=96 with `[Noticia]` once implemented (per
      `tech.md`/CONVENTIONS.md).
- [ ] No "cobolt" in user-facing text; COBOL identifiers (paragraph names,
      `WS-` fields, `INVOKE` method names) stay English throughout codegen
      and generated docs.
- [ ] System Knowledge Base updated in the same change (`tech.md` hard
      constraint) — `cobolt-compiler::publish_system_documentation` tables
      gain all six controls' properties/methods/events; note the chunked
      KB reindex stays operator-gated (suspended 2026-07-29) — do not run
      `build_chunked_kb` as part of this work unless asked.

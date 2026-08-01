# Spec — Maps, Knob, Gauge, Switch, FileDropZone, and Web Search controls

- **Status:** draft
- **Folder:** specs/039-knob-gauge-switch-maps-search-controls/
- **Author:** Claude (spec-driven)   **Date:** 2026-08-01

## 1. Overview

Add six new controls the operator has named as the last batch needed before a
production-ready **2.0.0**: **Maps** (a pannable/zoomable embedded map with
Google-sourced location data), **Knob** (a rotary dial that sets a numeric
value), **Gauge** (a read-only KPI display in one of three styles — Radial,
Linear, or Donut), **Switch** (a visual On/Off toggle), **FileDropZone** (a
drag-and-drop / click-to-browse file intake control), and **Web Search** (a
non-visual control that calls Google's Custom Search API and hands the caller
a result the developer's own COBOL wires wherever it wants). Knob, Gauge,
Switch, and FileDropZone are built on the **egui-elegance** widget crate
(confirmed to target egui 0.35 — this project's exact egui version, zero
conflict); Maps combines **egui-map-view** (a generic slippy-map tile
viewer) for the pannable/zoomable basemap with the **google_maps** crate
(a REST client for Directions/Geocoding/Places/Distance-Matrix — no
rendering of its own) for location data. All applicable controls gain data
binding by extending the Data Binding Guardian's (spec 022) approved target
list, which today excludes standalone scalar controls; Maps binds a marker
collection the same way DataGrid binds rows. Web Search follows the existing
non-visual, uniform-async-lifecycle contract (spec 032) already shared by
RestClient/SqlDatabase/IndexedFile, and is itself classified under the
Guardian's existing `RestApi` binding **source** kind — no new source kind
needed.

**A note on scope versus the operator's original ask:** Knob/Gauge were
first described with an arbitrary track-thickness, an arbitrary inner-track
color, and a gradient/flat/sunken fill effect. Once the operator chose to
build these on egui-elegance's real widgets (rather than a hand-rolled
custom paint layer), it turned out **none of the four widgets
(`Knob`/`RadialGauge`/`LinearGauge`/`ProgressRing`) support a gradient or
sunken effect**, and only two of the four (`LinearGauge`, `ProgressRing`)
expose a thickness control at all. The operator explicitly chose to accept
each widget's real customization surface over building a custom layer to
match the original ask exactly (see plan.md Decision 4) — this spec reflects
that decision, not the original literal wording.

## 2. Goals / Non-goals

### Goals

- Six new `ControlType` variants: `Maps`, `Knob`, `Gauge`, `Switch`,
  `FileDropZone`, `WebSearch` — each with canvas rendering (designer +
  preview + runtime, through the single shared renderer per spec 017),
  default size, Basic/Appearance property sections, supported events, and an
  entry in the System Knowledge Base's controls/properties/methods/events
  tables.
- **Knob** wraps egui-elegance's `Knob`: a numeric `Value` within
  `Minimum..Maximum`, a `Size` preset (Small/Medium/Large), a theme
  `Accent` color, optional `Bipolar` fill-from-center, `Step` quantisation,
  `ShowValue` readout, and a `Default` reset value (Alt+click/double-click).
  Fires `onChange` on interaction.
- **Gauge** is read-only (no drag/click changes its `Value`) and supports
  three visual styles via a `GaugeStyle` property:
  - `Radial` — egui-elegance's `RadialGauge` (half-circle speedometer with
    optional needle).
  - `Linear` — egui-elegance's `LinearGauge` (horizontal meter, adjustable
    bar height, optional thumb marker).
  - `Donut` — egui-elegance's `ProgressRing` (full circular ring,
    adjustable stroke width).
  All three share `Value` (0..1 fraction, or `Minimum`/`Maximum`/current
  `Value` mapped to that fraction), an explicit `Color` override (falls back
  to the theme accent when unset), and optional `WarningThreshold`/
  `CriticalThreshold` properties that drive automatic zone-based fill
  colouring (egui-elegance's `GaugeZones`) — a real, in-scope feature, not
  the hand-rolled alarm system the spec originally excluded.
- **Switch** wraps egui-elegance's `Switch` widget: boolean `Checked`,
  fires `onClick` on toggle (matching `CheckBox`'s existing convention).
- **FileDropZone** wraps egui-elegance's `FileDropZone`: accepts one or more
  dropped files (each exposing a path and/or raw bytes) or a click that
  opens a native file picker (via the `rfd` crate already used elsewhere in
  `cobolt-ide`), with an optional `Hint` text label. Fires `onFilesDropped`
  with the dropped file set available to the handler.
- **Maps** renders an interactive, pannable/zoomable map via
  **egui-map-view** using **OpenStreetMap** tiles (the only ToS-compliant,
  freely embeddable tile source available to a generic third-party slippy-
  map widget — see plan.md Decision 1 for why Google's own tiles are not
  used for the basemap image). The **google_maps** crate supplies
  Directions/Geocoding/Places/Distance-Matrix **data** — the control's
  markers, INVOKE-callable lookups, and any location data a developer's
  COBOL requests are genuinely Google-sourced, even though the visual
  basemap underneath them is OpenStreetMap tiles.
- **Web Search** is a non-visual control, presented on the canvas as an
  icon like `RestClient`/`AgentObject`, that calls the Google **Custom
  Search JSON API** given a query string set by the developer's own COBOL,
  and returns ranked results (title/snippet/link) the developer's own flow
  decides what to do with — the control does not itself write to any other
  control. It follows the uniform async lifecycle contract (spec 032):
  `Mode` (Sync/Async), `onComplete`/`onError`/`onCancelled`/`onTimeout`, plus
  its own `onResultsReceived` as primary event.
- Extend the Data Binding Guardian's (spec 022) approved target list to
  include `Knob`, `Gauge`, and `Switch` as **standalone** scalar targets —
  no control array required — each binding one source field to `Value`
  (`Knob`/`Gauge`) or `Checked` (`Switch`), and `Maps`, binding its markers
  collection to a data source the same way `DataGrid` binds
  `Rows`/`Columns`.
- Add `WebSearch` responses as an approved binding **source**, classified
  under the Guardian's existing `RestApi` source kind (no new source kind —
  a Custom Search response is structurally a REST API response, same as
  what `RestClient` already produces).
- Project-level credential storage for the **google_maps** API key (used
  for Directions/Geocoding/Places/Distance-Matrix calls — never for
  fetching OpenStreetMap tiles, which need no key) and the Google Custom
  Search API key, using the existing machine-local secret store mechanism
  (the same one LLM provider keys already use — never written into
  `cobolt.toml`, resolved at run time by a stable slot id).
  `SearchEngineId` (the Custom Search "cx" value) is not a secret and is a
  plain project-level setting alongside the key.
- i18n: every new property label, section heading, and error string is a
  `Tr` field in all six languages.

### Non-goals

- Rendering actual Google Maps tile imagery as the visual basemap — Google
  does not license raw tiles for third-party embedding outside their
  official JS/Android/iOS SDKs or the Static Maps/Embed APIs; the basemap
  is OpenStreetMap tiles instead (see Overview). A future spec could revisit
  a Static-Maps-API-image mode if genuine Google imagery is required.
  A generic non-OSM/non-Google tile-provider abstraction beyond what
  `egui-map-view` already supports out of the box.
- Any other Google (or non-Google) AI product beyond Custom Search JSON API
  — no Vertex AI Search / Gemini grounding in this spec.
- A gradient or sunken fill effect, or an arbitrary track-thickness
  property, on Knob/Gauge/Donut — none of egui-elegance's underlying
  widgets support this; each control's styling surface is exactly what its
  underlying widget exposes (see the Overview's scope note).
- A drag-to-rotate gesture for Gauge — it is read-only by design.
- Turn-by-turn directions, geocoding UI, or Street View **UI** inside the
  Maps control — the underlying `google_maps` crate's Directions/Geocoding
  data is available to COBOL via `INVOKE`, but no dedicated visual UI for
  entering/displaying it ships as part of this control.
- Offline/cached map tiles.
- File-type filtering or size-limit enforcement on FileDropZone — the
  underlying widget is intentionally stateless here; a `Hint` label can
  describe accepted formats, but the developer's own COBOL validates
  whatever was dropped.
- A native multi-file batch upload/transfer control — FileDropZone is local
  file intake only (path/bytes), not a client for any remote storage API.

## 3. User stories

- As a **form designer**, I want a **Knob** control so an operator can dial a
  numeric setting (e.g. a threshold or a target) the way a physical
  potentiometer works.
- As a **form designer**, I want a **Gauge** control, in Radial, Linear, or
  Donut style, so I can show a live KPI value (e.g. CPU load, order
  backlog) with automatic warning/critical colouring, without letting the
  user change it.
- As a **form designer**, I want a **Switch** control so a boolean setting
  reads as an obvious On/Off toggle instead of a checkbox.
- As a **form designer**, I want a **FileDropZone** control so a user can
  drag a file onto the form (or click to browse) and have my COBOL read its
  path or contents, without me building a file dialog by hand.
- As a **form designer**, I want an embedded **Maps** control so an
  application can show a location or a set of locations without leaving the
  form.
- As a **COBOL developer**, I want a non-visual **Web Search** control I
  can pass a string to and read a result back from, so I can build flows
  like "look up an address, summarise the first hit with the AI Agent
  control, and show the summary in a TextBox" entirely in my own COBOL —
  the control does one thing (search) and nothing else.
- As a **form designer**, I want to bind a Knob/Gauge/Switch's value straight
  to an Indexed file field or a SQL column, the same way I can already bind a
  chart or a DataGrid, so a live value updates the control without me writing
  a `SET` statement in an event handler.
- As a **form designer**, I want to bind Maps' markers to a data source (an
  indexed file of customer addresses, say), so the map populates from real
  data instead of one marker at a time by hand.

## 4. Requirements (EARS)

### Shared control-model requirements

- **R1 (ubiquitous):** The system shall add six `ControlType` variants:
  `Maps`, `Knob`, `Gauge`, `Switch`, `FileDropZone`, `WebSearch`.
- **R2 (ubiquitous):** Each new control shall render identically across the
  designer canvas, live preview, running (interpreted) form, and compiled
  binary, through the shared `cobolt-forms` renderer (spec 017) — no
  surface-specific rendering path.
- **R3 (ubiquitous):** Each new control's properties, methods, and events
  shall be documented in the System Knowledge Base tables the KB publisher
  emits, in the same change that adds the control (per `tech.md`'s hard
  constraint — no separate follow-up).
- **R4 (ubiquitous):** Every new user-facing string (property labels, section
  headings, tooltips, validation/error messages) introduced by this spec
  shall be a `Tr` field in `i18n.rs`, translated in all six supported
  languages (EN/ES/PT/JA/ZH/FR).
- **R5 (ubiquitous):** Each new control's supported-event list and
  human-language type-name matching (the filter that lets a prompt say
  "interruptor"/"switch"/"botão giratório" and have it resolve to the right
  `ControlType` for AI delegation context, per the 1.47.6 fix) shall be
  registered the same way existing control types are.

### Knob

- **R6 (ubiquitous):** `Knob` shall expose `Minimum`, `Maximum`, `Value`
  (clamped to `Minimum..Maximum`), `Step`, `Size` (`Small`|`Medium`|
  `Large`), `Accent` (a theme accent colour choice — one of the fixed set
  egui-elegance's theme already defines), `Bipolar` (bool, fill from
  centre), `ShowValue` (bool), `DefaultValue` (reset value), and `Label`.
- **R7 (event):** When the user drags a `Knob`, the system shall update
  `Value` continuously (quantised to `Step`, clamped to `Minimum..Maximum`)
  and fire `onChange` when the drag ends. Alt+click or double-click resets
  `Value` to `DefaultValue`.

### Gauge

- **R8 (ubiquitous):** `Gauge` shall expose `GaugeStyle`
  (`Radial`|`Linear`|`Donut`), `Minimum`, `Maximum`, `Value`, an explicit
  `Color` override (optional — falls back to the theme accent when unset),
  and optional `WarningThreshold`/`CriticalThreshold` (drives automatic
  zone-based fill colour via `GaugeZones`, when both are set).
- **R9 (ubiquitous):** `Radial` style shall additionally expose `ShowNeedle`
  (bool) and `ShowScale` (bool); `Linear` style shall expose `BarHeight`
  and `ShowThumb` (bool); `Donut` style shall expose `StrokeWidth`. Each
  style shall expose `Unit` (a suffix string) and an optional `Text`
  override for the value readout.
- **R10 (constraint):** `Gauge` shall not accept drag or click input to
  change `Value` — it changes only from COBOL (`SET`) or from a data
  binding (R17). Changing `GaugeStyle` at design time swaps the underlying
  rendered widget; it never becomes interactive in any style.

### Switch

- **R11 (ubiquitous):** `Switch` shall expose `Checked` (boolean).
- **R12 (event):** When the user clicks a `Switch`, the system shall flip
  `Checked` and fire `onClick` (matching `CheckBox`'s existing convention).

### FileDropZone

- **R13 (ubiquitous):** `FileDropZone` shall expose a read-only
  `DroppedFiles` collection (each entry carrying a file path and/or raw
  bytes, per what the OS drop event supplies) and a `Hint` text property
  (a label only — no enforced validation).
- **R14 (event):** When the user drops one or more files on the control,
  the system shall populate `DroppedFiles` and fire `onFilesDropped`.
- **R15 (event):** When the user clicks the control (no drag in progress),
  the system shall open a native file picker (via the existing `rfd`
  dependency); a file chosen this way populates `DroppedFiles` and fires
  `onFilesDropped` identically to a drop.

### Maps

- **R16 (ubiquitous):** `Maps` shall render an interactive, pannable/
  zoomable map (drag to pan, scroll/pinch to zoom, double-click to
  centre-and-zoom) using OpenStreetMap tiles via `egui-map-view`.
- **R17 (ubiquitous):** `Maps` shall expose `ApiKeySource` (resolves the
  **google_maps** API key from the project-level credential store, §
  Data & credentials — never a literal key on the control or in the
  `.cfrm`; used only for Directions/Geocoding/Places/Distance-Matrix calls,
  never for tile fetches), `Center` (lat/lng), `Zoom`, and a `Markers`
  collection (each with lat/lng, label, and an optional info-window text).
- **R18 (event):** When the developer or the runtime adds/removes/updates an
  entry in `Markers` (via property write, `INVOKE`, or a data binding), the
  system shall reflect it on the rendered map. Since `egui-map-view` has no
  built-in marker/pin primitive, markers are drawn as a `cobolt-forms`
  overlay layer on top of the tile view, positioned by the same lat/lng→
  screen-pixel projection the tile view itself uses.
- **R19 (ubiquitous):** `Maps` shall support the events `onMapClick`,
  `onMarkerClick` (hit-tested against the overlay markers from R18), and
  `onBoundsChanged`.
- **R20 (ubiquitous):** `Maps` shall expose `INVOKE`-callable
  Directions/Geocoding/Places/Distance-Matrix lookups backed by the
  `google_maps` crate, so a developer's COBOL can resolve an address to
  coordinates, request a route, or search nearby places without the
  control drawing anything itself for those results (R the developer's own
  flow decides what to do with the data, same principle as `WebSearch`).

### Data binding (extends spec 022)

- **R21 (ubiquitous):** The Data Binding Guardian's approved binding-target
  list shall be extended to include `Knob`, `Gauge`, and `Switch` as
  **standalone** scalar targets — no control array required — each binding
  one source field to `Value` (`Knob`/`Gauge`) or `Checked` (`Switch`).
- **R22 (ubiquitous):** The Guardian's approved binding-target list shall be
  extended to include `Maps`, binding its `Markers` collection to a data
  source the same way `DataGrid` binds `Rows`/`Columns` — one source field
  per marker attribute (lat, lng, label).
- **R23 (ubiquitous):** The Guardian's approved binding-**source** list
  shall be extended to include `WebSearch` responses, classified under the
  existing `RestApi` source kind (no new `BindingSourceKind` variant).
- **R24 (constraint):** All other Guardian rules from spec 022 (validation
  before accept/save/generate/run/debug/build; blocking bindings to
  non-approved targets) apply unchanged to the new controls.
- **R25 (constraint):** `FileDropZone` is **not** a Guardian binding target
  — `DroppedFiles` is event-shaped output (populated by user action), not a
  displayed value a data source drives.

### Web Search (non-visual)

- **R26 (ubiquitous):** `WebSearch` shall be a non-visual control, rendered
  on the canvas as a fixed-size icon (matching `RestClient`/`AgentObject`
  precedent), calling the Google **Custom Search JSON API**.
- **R27 (ubiquitous):** `WebSearch` shall expose `SearchEngineId` (the
  Custom Search "cx" value — a plain, non-secret property), `Query` (set by
  the developer's COBOL before invoking a search), `NumResults`, and
  `SafeSearch`.
- **R28 (ubiquitous):** `WebSearch` shall follow the uniform async lifecycle
  contract already shared by `RestClient`/`SqlDatabase`/`IndexedFile` (spec
  032): a `Mode` property (Sync/Async, default Async — matching
  `RestClient`'s default, since this is genuine external-network risk) and
  the events `onComplete`, `onError`, `onCancelled`, `onTimeout`, plus its
  own `onResultsReceived` as primary event.
- **R29 (ubiquitous):** A completed search's results shall be readable from
  COBOL as both a top-result shortcut (`TopTitle`/`TopSnippet`/`TopLink`)
  and indexed access (`ResultCount` plus
  `INVOKE <id> 'GetResult' USING <n>`), matching the shape
  `RestClient`/`SqlDatabase` multi-row responses already use. The control
  shall not write to any other control's property itself.
- **R30 (ubiquitous):** `WebSearch`'s API key shall resolve from the
  project-level credential store (§ Data & credentials), by a stable slot
  id distinct from the Maps/google_maps key and from any LLM provider key.

### Data & credentials

- **R31 (ubiquitous):** The **google_maps** crate's API key and the Google
  Custom Search API key shall each be stored in the existing machine-local
  secret store (the same mechanism `llm::store_api_key`/`api_key_slot`
  already provides for LLM provider credentials) — never written into
  `cobolt.toml` or the `.cfrm` file, and never held as a literal on any
  control property.
- **R32 (ubiquitous):** The project settings UI shall gain fields to set/
  clear the google_maps key, the Google Custom Search key, and the
  `SearchEngineId`, in the same section pattern used for existing provider
  credentials.
- **R33 (constraint):** A project opened without a configured google_maps
  key shall still render the Maps control's OpenStreetMap basemap (tiles
  need no key) but shows Directions/Geocoding/Places/Distance-Matrix
  lookups in a clear "not configured" state; a project without a Custom
  Search key shall show `WebSearch` in the same "not configured" state.
  Neither case crashes or fails silently.

## 5. Acceptance criteria

- [ ] AC1 — Dragging a `Knob` from `Minimum` to `Maximum` updates `Value`
      live, quantised to `Step`, and fires `onChange` once the drag ends;
      Alt+click resets to `DefaultValue`. **Partially automated:**
      `engine_knob_drag_changes_value` (cobolt-forms/src/render.rs) proves
      a drag increases `Value` and fires `onChange`/`onValueChanged`
      through the real interactive render engine. `Step` quantisation and
      Alt+click-reset are the `egui-elegance::Knob` widget's own internal
      contract (this codebase only passes `.step()`/`.default()` through
      to it) — not independently unit-tested here. **Manual:** the
      operator confirming the exact drag feel and Alt+click in a running
      IDE.
- [ ] AC2 — A `Gauge` with `GaugeStyle = Donut`, `WarningThreshold = 0.6`,
      `CriticalThreshold = 0.85` renders green/yellow/red automatically as
      `Value` crosses those fractions; clicking or dragging on any Gauge
      style does not change `Value`. **Partially automated:**
      `engine_gauge_ignores_click_and_drag_in_every_style` proves the
      never-changes-`Value` half across all three `GaugeStyle`s. **Manual:**
      the actual zone-colour rendering is visual only.
- [x] AC3 — Clicking a `Switch` flips `Checked` and fires `onClick`.
      Fully automated: `engine_switch_click_toggles_checked` asserts both
      `Checked → "true"` and `onClick` firing through the real interactive
      render engine.
- [ ] AC4 — Dropping a file on a `FileDropZone` populates `DroppedFiles`
      and fires `onFilesDropped`; clicking the zone (no drag) opens a
      native file picker with the same result. **Partially automated:**
      `engine_file_drop_zone_click_requests_a_native_picker` proves the
      click → native-picker-request half. An actual OS-level drag-and-drop
      deposit cannot be driven from a unit test — **manual**.
- [ ] AC5 — A `Maps` control renders an interactive OpenStreetMap-tiled map
      that pans on drag and zooms on scroll with no API key configured;
      adding an entry to `Markers` from COBOL places a marker overlay at
      the correct lat/lng without reloading the control. **Partially
      automated:** `engine_maps_drag_pans_center_and_fires_bounds_changed`,
      `engine_maps_scroll_changes_zoom_only_while_hovered`, and
      `engine_maps_marker_click_sets_selected_marker_id_and_fires_on_marker
      _click` (cobolt-forms/src/render.rs) prove the pan/zoom/marker-click
      interaction logic; `maps_static_preview_paints_a_backdrop_without_
      panicking` (paint.rs) proves the no-key basemap renders without
      crashing. **Manual:** actually looking at real OSM tiles on screen.
- [ ] AC6 — With a google_maps API key configured, `INVOKE <maps-id>
      'Geocode' USING <address>` (or the spec's equivalent method name)
      returns coordinates from the real Google Geocoding API. **Manual**
      — needs a real, operator-supplied API key; `maps_bridge::run`'s
      field/method usage was verified against the real crate's source
      (T11), but no test in this suite calls the live API.
- [ ] AC7 — `WebSearch.Query` set, then `INVOKE <id> 'Search'`, returns
      ranked results readable from COBOL via both `TopTitle`/`TopSnippet`/
      `TopLink` and indexed `GetResult`; the control never writes to any
      other control's property on its own. **Partially automated:**
      `web_search_accessors_parse_result_count_top_result_and_indexed_
      result` proves the full read-back contract against a stubbed
      response body. **Manual** — the real end-to-end search → AI Agent
      summarise → TextBox flow from the spec's user story, with a real
      Custom Search key.
- [x] AC8 — A standalone `Knob` (not inside a control array) shows
      data-binding fields in its Properties pane and, once bound to an
      Indexed file field, updates `Value` when that field changes.
      Automated: `data_binding_properties_show_standalone_knob_gauge_
      switch_hide_file_drop_zone` (Properties-pane visibility) +
      `scalar_control_refresh_binding_writes_value_from_cobol_field`
      (the actual refresh mechanism) together cover this end to end at
      the model/runtime layer.
- [x] AC9 — A `Maps` control bound to a data source (e.g. an Indexed file of
      addresses with lat/lng columns) populates `Markers` from that source,
      the same way a bound `DataGrid` populates `Rows`. Automated:
      `maps_marker_refresh_binding_populates_markers_from_cobol_table` (T13).
- [x] AC10 — Neither the google_maps key nor the Custom Search key appears
      in `cobolt.toml` or in a saved `.cfrm` file after being configured.
      Automated: `project_integrations_round_trip_the_search_engine_id_
      never_the_keys` (T7) — the keys live only in `LlmConfig.api_keys`
      (machine-local settings), never in `ProjectIntegrationSettings`
      (which does round-trip through `cobolt.toml`).
- [x] AC11 — A project with no google_maps/Custom Search key configured
      still renders the Maps basemap, shows the "not configured" state for
      Directions/Geocoding/Places and for WebSearch, and neither crashes
      nor fails silently. Automated: `maps_op_without_a_configured_key_
      fails_synchronously_no_worker_spawned` and `web_search_op_without_a_
      configured_key_fails_synchronously_no_worker_spawned` both prove the
      synchronous `onError`/no-network-call contract; `maps_static_
      preview_paints_a_backdrop_without_panicking` proves the basemap
      still renders with no key. Visual confirmation of the basemap
      remains a nice-to-have manual check, not a gap in this AC's core
      claim.
- [x] AC12 — All six controls' properties, methods, and events appear in
      the System Knowledge Base tables, and the freshness test comparing
      the prebuilt chunked store to the published documentation is
      understood to need a reindex (per the current, operator-suspended
      reindex policy) rather than silently passing stale. **Exceeded:**
      the reindex suspension was lifted 2026-07-31 (before this task
      landed), so rather than merely detecting staleness, T17 actually ran
      `cargo run -p cobolt-ide --example build_chunked_kb` and confirmed
      `prebuilt_chunked_kb_matches_the_published_documentation` green.
      `spec_039_six_controls_are_fully_published_in_the_system_kb`
      (cobolt-compiler) covers the properties/methods/events-appear half.
- [x] AC13 — Every new IDE string introduced by this spec is present in
      `Tr` with all six language variants populated (no fallback-to-English
      placeholder left in a non-English locale). Automated:
      `no_empty_ui_translations` and `non_english_is_actually_translated`
      (cobolt-ide i18n tests) both pass with the 6 new fields in place.

## 6. Constraints & steering check

- **i18n (6 languages):** Yes — see R4/R5, AC13. Every new property label,
  section heading, and message needs all six languages.
- **Generated-code / regenerate contract:** Yes — `cobolt-codegen` needs a
  generator path for each new control (Knob/Gauge/Switch as simple property
  emitters; FileDropZone as a drop/picker event emitter; Maps and
  WebSearch as INVOKE-capable non-trivial controls following the
  RestClient precedent). Generated code keeps the developer banner and
  regenerates on Build/Run/Debug/Check like every other control.
- **Docs (English guide):** Yes — `docs/developers-guide-en.md` needs a
  section per new control (properties, events, COBOL API), plus an update to
  the project-settings section covering the two new credential fields. The
  translated guides are user-maintained and must not be touched.
- **Fix vs feature classification:** Feature — six new control types and a
  Guardian scope extension are new user-visible capability, not a defect.
  Bumps the **minor** version per `tech.md`, with a `CHANGELOG.md` entry;
  announce on f=96 with the `[Noticia]` prefix once implemented.

## 7. Open questions

All four questions raised in the original draft are now resolved (see
plan.md §4 for the reasoning behind each):

- ~~Q1: Maps rendering mechanism~~ — resolved: `egui-map-view` (OpenStreetMap
  tiles) + `google_maps` crate (data), not a native web-view.
- ~~Q2: Knob drag gesture shape~~ — resolved: egui-elegance's `Knob` owns
  its own drag handling; adopted as-is rather than a custom gesture.
- ~~Q3: WebSearch result shape~~ — resolved: both a top-result shortcut and
  indexed access (R29).
- ~~Q4: Gauge value-changed feedback~~ — resolved: no new event; `GaugeZones`
  threshold colouring (R8) covers the "developer wants to see a value cross
  a line" case without a dedicated event.

No open questions remain; ready for `/tasks`.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Changelog

## [PowerRustCOBOL 1.47.8] — 2026-07-31

### Changed

- **Half again the rain, and the clock gives way when it must.** The matrix
  effect gets one falling line per ~19 px of width instead of ~28 — 50% more
  lines — each still owning its own start, 10–25 ms behind the one before it
  (lines launched in pairs to buy density were tried and rejected). Line
  count and beat pull against each other: `n` lines cannot start in less than
  `n × 17.5 ms`, and each still has to fall, so with the duration fixed the
  only way to honour both is to drop lines. The configured duration is now
  the effect's FLOOR rather than its ceiling, bounded at 6 s. At the 4000 ms
  setting nothing overruns at all — a 1440 px window simply gets 72 lines
  instead of 48; at 2000 ms the effect takes ~3.5 s to land them, and the
  developer can trade the overrun back by lowering the duration. Every line
  still lands before the effect ends.

## [PowerRustCOBOL 1.47.6] — 2026-07-31

### Fixed

- **"caixa de texto" now names a TextBox.** The filter that decides which
  control types a task's context carries keyed on the English type name, so
  every everyday wording — "caixa de texto", "campo de digitação", "text
  box", "entrada de dados", "botões", "lista suspensa" — named nothing, and
  the specialist was left binding against a legend it never received. Types
  are matched word by word with plurals folded in, plus the everyday names
  for the dozen types a form request actually reaches for, in the three
  languages the IDE is used in. The guard that made the strict match worth
  having still holds: `PanelHeader` is not a `Panel`, `LineCharts` are not
  `Line`s.
- **A task no longer has to name the type itself.** Which types are in play
  is a property of the whole plan: the designer task says "deploy 15
  TextBoxes" and the event task then says "the handlers for those" — read
  alone, the second names no type at all. Both tasks work on the same form,
  so both get the same legend.
- **The RAD assistant's transcript follows the conversation again.** Nothing
  scrolled it: a returned turn, a Grace or specialist progress line, the
  "Thinking…" indicator appearing — all of it landed below the fold. The
  view now follows whenever material is appended, and holds still while a
  mouse button is down or while the developer has scrolled back to read
  earlier turns; returning to the bottom resumes the follow.

## [PowerRustCOBOL 1.47.5] — 2026-07-31

### Fixed

- **A plural cost the workflow its events.** "adicione 15 textboxes…" is a
  task about TextBoxes, but the filter that decides which control types a
  task's context carries matched whole words only, so "TextBoxes" did not
  name `TextBox`: the type's events and property keys were cut from the
  context of both the designer and the event agent. The event agent is
  required to bind only names its context lists — with none listed it
  guessed, writing `onFocus`/`onBlur` where the real names are
  `onGotFocus`/`onLostFocus`, and every handler carrying a guess was
  discarded at apply time. The filter now accepts the `s`/`es` plural, while
  still refusing to read `PanelHeader` as a `Panel` or `LineCharts` as a
  `Line`.
- **A discarded operation now says so.** The apply path skips operations that
  cannot be applied — by design, since an invalid one would corrupt the form
  — but it reported only how many it applied. A change-set whose handlers all
  named a nonexistent event therefore ended as "applied 1 change" with no
  events on the form and nothing to explain it. Each skipped operation is now
  named with its reason in the workflow log.

## [PowerRustCOBOL 1.47.4] — 2026-07-31

### Fixed

- **Pasting into the assistant's prompt resized the box.** Paste more lines
  than the box holds and its border grew past the pane, the bottom edge
  disappearing and the last line reading as cut off. The border belonged to
  the `TextEdit` itself, and a TextEdit is sized by its content — so the
  thing the developer took for the box's size was never the box's size. The
  bordered box is now a frame of exactly the dragged height, with a
  frameless editor scrolling inside it: the text scrolls, the box does not
  move. Its height still comes from one place only — the corner grip's drag,
  clamped to 1–6 rows — and the grip is anchored to that frame, so it tracks
  the corner instead of following the text down. The scroll viewport is
  pinned at both ends, which also stops egui's 64px `min_scrolled_height`
  from quietly overriding the smallest dragged sizes.

## [PowerRustCOBOL 1.47.3] — 2026-07-31

### Fixed

- **A form asked for 15 textboxes came back with twice that many.** A
  `deploy_control` naming an id the form already carried did not touch that
  control: the id was taken, so the designer minted a *new* one under an
  auto id. Agents re-emit their whole change-set as a matter of course, so
  a workflow with two form tasks over the same form deployed everything
  twice, and the developer got 32 controls with the second set named
  `TextBox-N`. A deploy whose id already names a control **of the same
  type** is now a redeploy: its properties are applied to that control,
  within the same change-set as well as across change-sets. An id that
  collides with a control of a *different* type is still a real collision
  and still gets a fresh id. The change-set contract tells the agents this,
  so they also know a bare redeploy overwrites the layout it omits.
- **Grace planned "validate the result" as a task.** The plan behind that
  same form ended with a third task asking the Form Designer Agent to
  confirm the controls matched the handlers. A specialist has exactly one
  output channel — its change-set — so a task it cannot answer with new
  operations it answers by re-emitting the ones it already sent, here with
  a different layout and no colours. Cross-task consistency is Grace's own
  integration step, shared with the Pedantic companions; the orchestrator
  is now told not to delegate verification, and to reopen a specialist's
  task only when the comparison finds a concrete mismatch to fix.
- **A 401 said "Unauthorized" and left the developer guessing.** The
  provider's rejection reads like an account problem, when the cause is
  almost always a credential that expired or was rotated since it was
  stored. The error now carries what to check, in order: that a valid API
  key for that provider is registered for the model; whether it has
  expired — **stating how long the key on file has been registered in
  PowerRustCOBOL**, since that is what settles it; and whether the provider
  still offers the selected model. Key dates are recorded from now on; a
  key stored by an earlier build says its age is unknown rather than
  guessing.

## [PowerRustCOBOL 1.47.2] — 2026-07-31

### Fixed

- **The assistant prompt's resize grip sat outside the box.** The corner
  grip was anchored to the slab allocated around the editor rather than to
  the editor's own frame — the bordered box you actually see. The slab
  carries the height as it is dragged, continuously, while the editor snaps
  to whole text rows, so the two bottoms disagreed: the grip hung below the
  border at rest and walked further out with every drag. It now follows the
  editor's frame and is inset past the stroke and corner radius, so it
  reads as sitting on the box's inner edge. Both assistants — the RAD
  designer's and the project chat — carried the same construct.

## [PowerRustCOBOL 1.47.1] — 2026-07-30

### Changed

- **A correction round now fixes only what was wrong and keeps what was
  right.** Until now every rejection — from the Pedantic reviewer or from
  machine validation — sent the specialist its whole submission back with
  "submit the COMPLETE result again, a full replacement". That reprocessed
  correct work to fix a defect that was often tiny, and it was not harmless:
  a specialist asked to redo everything rewrites operations nobody
  complained about (observed live, where three malformed handlers were
  flagged and the model silently reimplemented a fourth, correct one).
  The reviewer's verdict now carries **`defective_ops`**, naming the
  operations its findings belong to; machine validation attributes them by
  itself, since it proves them. The engine keeps every operation that was
  not named, shows the specialist only the defective ones — with an explicit
  "already accepted, do not resubmit" list — and splices the correction back
  onto the kept work. A specialist that ignores the instruction and
  resubmits everything still merges cleanly: its version supersedes the one
  it targets, and nothing is duplicated. When the defect belongs to no
  particular operation (or nothing would be left to keep), the old
  full-replacement round is used exactly as before.

## [PowerRustCOBOL 1.47.0] — 2026-07-30

### Added

- **Alibaba (Model Studio) is now a model provider.** Alibaba Cloud's
  DashScope serves the Qwen family on the OpenAI wire under
  `/compatible-mode/v1`, so it works with the existing bearer-token auth and
  model refresh. The shipped endpoint is the international (Singapore) host,
  `https://dashscope-intl.aliyuncs.com/compatible-mode/v1`; inside mainland
  China switch the model profile to `https://dashscope.aliyuncs.com/...`.

### Fixed

- **Running a form no longer wipes the assistant's conversation.** The run
  log and the agent trace share one pane, and starting a run cleared the
  whole thing — the request, the plan, the review findings and the generated
  code the developer was still reading all vanished. A run (and a build, a
  check, a debug session) now clears only its own half; the explicit **Clear**
  button still wipes everything.

### Changed

- **The Knowledge Base now documents what an event handler actually
  receives.** Almost every event delivers nothing: the dispatcher calls a
  handler with no arguments, so its `LINKAGE SECTION` is empty. The single
  payload in the platform is `CONTROL-ARRAY-INDEX`, and only for a control
  inside a repeating group. In particular **no event carries a key code** —
  the assistant was declaring a `KEY-CODE` linkage item that nothing
  populates — and a specific key has its own event: `onEnterPressed` for
  ENTER, `onEscapePressed` for ESC, since `onKeyDown` fires for any key and
  says nothing about which one.

## [PowerRustCOBOL 1.46.3] — 2026-07-30

### Fixed

- **Keyboard event handlers were rejected in an unwinnable loop.** A handler
  that binds its event payload — `PROCEDURE DIVISION USING KEY-CODE.`, the
  only way to write one — failed the change-set validator with "Code must
  include PROCEDURE DIVISION.", an instruction it had already followed. The
  agent resubmitted the same code, was rejected again, and the workflow burnt
  its whole correction budget creating nothing. The check matched the header
  line for exact equality; it now accepts a header carrying a phrase or an
  inline `*>` comment, and the message names the accepted form.

### Changed

- **Matrix rain glyphs are 50% larger** (13 px → 19.5 px). The row pitch and
  the band each falling line owns scale with the glyph, so bigger characters
  neither crowd the one above them nor overlap the line beside them.

## [PowerRustCOBOL 1.46.2] — 2026-07-30

### Changed

- **Matrix rain: lines fall in from the top, over a see-through window.** The
  effect no longer opens with lines already halfway down the screen — every
  line enters from above the top edge. The black world is gone with them:
  the form is now painted only down to each line's tail, so ground no tail
  has passed over is simply never painted and the window stays **completely
  transparent** there (the desktop shows through). That also lets the Matrix
  entrance join the see-through, title-bar-less window treatment.
- **Matrix rain: faster, and staggered by milliseconds.** Lines fall half
  again as fast, and they arrive on a real clock: the first ones 25 ms apart,
  then as many as the window fits, each 10–25 ms behind the last, at their
  own speeds (±15%). Every line still lands on the bottom edge before the
  animation ends, and on a long animation the stagger stretches to fill the
  time rather than leaving the form revealed but still waiting.

## [PowerRustCOBOL 1.46.1] — 2026-07-30

### Changed

- **The system Knowledge Base now states the COBOL naming rules.** Both
  `rustcobol_extensions.md` (a new *Naming rules* section) and the Form
  Controls Reference explain that a control id becomes a COBOL user-defined
  word in the generated program, so it may hold only letters, digits and
  hyphens, may not begin or end with a hyphen, and must never contain an
  underscore — with the failure spelled out (`WS-TEXTBOX_1-TEXT` is read as a
  word, an error token and a number, and the control loses its storage). The
  same rule is stated for every WORKING-STORAGE item and paragraph name a
  handler declares, so the assistant stops proposing ids like `textbox_1`.
  The `deploy_control` operation's own schema now carries the rule too, which
  reaches the model even when the KB chunk is not retrieved.

## [PowerRustCOBOL 1.46.0] — 2026-07-30

### Added

- **Window effects play with no chrome, and over the desktop.** A window that
  has an entrance effect now opens with **no title bar** — nothing sits still
  while the animation runs — and the bar arrives together with the finished
  form (only if the form was designed to show one). Exits drop the chrome the
  same way. On top of that, the effects that merely move, scale or fade the
  form's own face (fade, zoom, the four slides, expand-from-title-bar, genie)
  open a **see-through window**, so the form animates loose on the desktop
  instead of over a window-coloured rectangle; a fade now dims the face's own
  opacity rather than veiling it with the background colour. Two consequences
  worth knowing: on such a window the form's **`transparency` property finally
  reaches the desktop** (it never could before — the window was opaque), and
  the macOS drop shadow is off for that window's life, since it would outline
  the "invisible" window and winit only offers that switch at creation.
  The mask effects (radar, iris, blinds, checkerboard) and the Matrix rain
  keep an opaque window: they hide the form by painting over it, and
  "transparent" cannot erase pixels already painted.

### Fixed

- **Control ids with underscores generated broken COBOL.** A control named
  `textbox_1` emitted `WS-textbox_1-TEXT`, which the lexer read as an
  identifier, an error token for `_`, and then a number — so the compiler
  reported "skipping unknown data clause" and the control ended up with **no
  storage at all**. Control ids are now normalised into valid COBOL words
  (every non-alphanumeric character becomes a hyphen, runs collapse, the ends
  are trimmed) everywhere they reach the generated source: control data
  groups, IndexedFile and SQL data items, and the paragraph names those
  controls' facades define and PERFORM. The id's own case is preserved, and a
  control's default caption still carries its literal id inside quotes.

## [PowerRustCOBOL 1.45.7] — 2026-07-30

### Fixed

- **The TextBox caret is now always legible.** egui drew it from the ambient
  visuals, so on a dark BackgroundColor — or a dark form seen through Liquid
  Glass — the caret was dark on dark and effectively invisible. It now
  measures the field's real surface (the developer's BackgroundColor over the
  form's backdrop, or the active glass style's own surface) and keeps the
  text colour while that already clears WCAG AA contrast, flipping to black
  or white otherwise. Whatever the field colour, the caret clears AA.

## [PowerRustCOBOL 1.45.6] — 2026-07-30

### Added

- **A resized window keeps the form and stretches the background.** Maximize
  a running form, or drag its border out, and the controls stay exactly where
  and how big they were designed — but the **gradient (or background image)
  now covers the whole window** instead of stopping at the form's edge.
  Dragging the window *smaller* than the form does not crop the background:
  it stays at the form's size and the form scrolls inside it. Applies to the
  run form, the form running inside the IDE, and compiled applications; the
  designer canvas and the Preview keep the backdrop at the form's own size so
  the designed extent stays visible while editing.

### Fixed

- **Window effects revealed a form without its background.** The static face
  the effects animate painted the solid colour only, so a form with a
  gradient or a background image was revealed bare and then jumped to its
  real background the instant the animation ended — very visible under the
  Matrix entrance. Backdrop painting now lives in one engine function that
  the live render AND the effect face both call, so the reveal shows the
  finished form. The same face also stopped scaling controls against the form
  size, which stretched them on a window bigger than the form and snapped
  them back when the animation finished.

## [PowerRustCOBOL 1.45.5] — 2026-07-30

### Fixed

- **Matrix rain: the zoom is gone, the end of trail does the whole job.**
  The camera fly-through has been removed. What remains is the reveal the
  effect was always meant to have: the form's content never appears all at
  once — each column's **end of trail**, the faint top glyph of its falling
  line, descends over the window and progressively uncovers whatever stood
  behind it. What the tail has passed is revealed; what lies below it is
  still the black world, which is where the rain falls, so a line simply
  runs out of dark to fall through instead of ever crossing into uncovered
  ground. Columns keep their own speeds (±15%) and start delays, so the
  reveal is ragged rather than a single sweep, and every tail lands on the
  bottom edge by the end — the form is complete exactly when the last
  character leaves the window. Each column now runs three falling lines
  instead of one, so the dark stays properly rained-on while it lasts.

## [PowerRustCOBOL 1.45.4] — 2026-07-30

### Fixed

- **The Matrix reveal now follows the end of the trail.** The form no longer
  appears a strip at a time: from the halfway point each column runs one
  revealing line whose **end of trail** — the topmost, faintest glyph —
  walks down the window and progressively uncovers whatever sat behind it.
  Everything above that tail stands revealed; everything below is still the
  black world, which is where the rain falls, so a line entering uncovered
  ground simply has no dark left to fall through. The tail lands on the
  bottom edge at each column's own deadline, so the form is complete exactly
  when the last character leaves the window. The camera keeps coming in
  (glyphs, their spacing and the column pitch all grow, accelerating to 5×),
  and the black strips travel and widen with it, so magnification alone can
  never uncover anything — only the descending tail can.
- **Matrix-rain now ignores the easing setting.** Its choreography defines
  its own pacing; with the default ease-out on top, the "halfway point" fell
  at 29% of the wall clock and the easing cancelled the camera's
  acceleration. Every other effect still eases exactly as before.

## [PowerRustCOBOL 1.45.3] — 2026-07-30

### Fixed

- **The run form drew katakana as empty boxes.** The run-form process (and
  compiled applications) never installed a font set, so they had egui's
  Latin-only defaults while the IDE had the broad-Latin + CJK system
  fallbacks — the Matrix rain looked right in the settings preview and fell
  as tofu boxes in the real window. Both now install the same base font set
  the IDE uses. As a belt-and-braces measure the rain probes its own font
  first and falls back to **digits** on any host with no katakana, so it can
  never show boxes again.
- **Matrix fly-through, final choreography.** The form no longer fades in
  during the first half: it stays completely hidden behind the black world
  until the camera starts moving at the halfway point. The characters are
  **never faded out** — they keep full brightness and leave only by flying
  out of the window. Each column now uncovers *its own strip* of the form as
  it rushes off screen, so every element is revealed by the passage of a
  line of characters: the outer strips first, the ones dead ahead last, and
  the whole form exactly when the final column leaves the frame. Columns
  dead ahead of the camera are nudged off the axis so none can stay on
  screen, and the magnified glyph sizes are quantised onto a short capped
  ladder so a 35× zoom cannot blow up the font atlas mid-animation.

## [PowerRustCOBOL 1.45.2] — 2026-07-30

### Fixed

- **MatrixRain became a camera fly-through.** The rain no longer wipes or
  fades away: it keeps falling continuously (wall-clock driven) for the
  whole animation while the form **fades in behind it** through a thinning
  black veil. From the halfway point the "camera" flies forward — glyph
  columns zoom toward the observer with **speed proportional to elapsed
  time** (accelerating, scale ∝ z²), each column at its own depth, until
  the camera passes **between** the rows and only the fully-revealed form
  remains at the end. MatrixRain now owns its duration band, **1500–4000
  ms** (other effects keep 100–3000 ms); the settings duration field, the
  preview, spawn-arg parsing and the runtime all clamp into the selected
  effect's own bounds, and new projects default to a 2000 ms entrance.

## [PowerRustCOBOL 1.45.1] — 2026-07-30

### Fixed

- **MatrixRain got its proper choreography.** The window now starts fully
  BLACK — on any theme — and each falling line of characters wipes the
  blackness away, revealing the form above its trail; below each column's
  head the cover stays black, the glyphs vanish crossing the bottom edge,
  and the form is complete exactly when the last character disappears.
  Columns fall **individually** (the old per-column seeding made neighbours
  fall in visible pairs; a full-avalanche hash now decorrelates them) at
  **varied speeds, −15% to +15%** around nominal, normalised so every
  column still finishes by the animation's end. Glyphs mutate naturally
  while falling (choice keyed to the cell being crossed).

## [PowerRustCOBOL 1.45.0] — 2026-07-30

### Added

- **Project window entrance & exit effects (spec 038).** Configure once per
  project (settings → Appearance): an entrance and an exit effect, each
  with duration (100–3000 ms) and easing, applied to every form. The
  catalogue: fade, dBASE-style zoom, slides, expand-from-title-bar, radar
  wipe, iris wipe, venetian blinds, checkerboard, **Matrix falling code**
  (classic katakana/digit glyphs — the default entrance for NEW projects)
  and a genie approximation. Effects animate the form's static face through
  the shared designer painter, so the look matches the designer exactly and
  the form is interactive the moment the entrance ends. Forms carry a
  single `WindowEffects` opt-out checkbox; control load-time animations
  start right after the entrance finishes; an optional project setting
  replays the entrance on restore-after-minimize (visual only, no events).
  Exit effects play after the FormState veto check — a refused close plays
  nothing — and the actual close (onClose, once) happens when the animation
  completes; STOP RUN closes take the same choreography. Machine-wide
  kill-switch in Help → Debug Settings ("Disable window effects",
  `PRC_NO_WINDOW_FX=1` for bare `rcrun run-form`); a live preview button in
  the settings plays entrance → hold → exit over a sample card. Older
  projects load with effects off and behave exactly as before.

## [PowerRustCOBOL 1.44.4] — 2026-07-30

### Fixed

- **Build no longer dies on project names with dots.** The generated build
  crate took the project name almost verbatim, so `PowerDemo3.project`
  produced an invalid Cargo package name (`powerdemo3.project`) and cargo
  aborted with "invalid character `.` in package name". The name is now
  sanitized: lowercased, `.project` suffix dropped, anything outside
  `[a-z0-9_-]` replaced by `_` (`PowerDemo3.project` → `powerdemo3`).
- **Build details reveal as a feed.** The details window shows one new line
  every 250 ms instead of dumping the whole log at once, and keeps
  auto-scrolling while the view sits at the bottom (scrolling up pauses the
  follow, back to bottom resumes it).

## [PowerRustCOBOL 1.44.3] — 2026-07-30

### Added

- **Build-details window.** The Building dialog gains a "Details…" button
  opening the full build log: every phase milestone (theme foreground),
  the compiler's supplementary counts and sizes (dimmed — these previously
  reached stderr only in verbose builds), success in green and errors in
  red, high-contrast in light and dark themes. The window opens centered,
  is freely movable and resizable, offers Copy and Save… (plain text), and
  **opens automatically when a build fails**.

## [PowerRustCOBOL 1.44.2] — 2026-07-30

### Fixed

- **Run starts the main form, not "whatever was in the editor".** With a
  project open (every project is a desktop project today), the Run button
  launched the editor's active COBOL source — or the project's entry
  program — in the console runner. It now resolves the MAIN form (repairing
  the exactly-one invariant if needed) and launches it as a standalone
  `rcrun run-form` window: an open designer's live state wins, a closed
  main form is loaded from disk after the usual regenerate-all pass.
  Single-file mode (no project) still runs the editor's source.
- **Build progress narrates its real phases.** The Building dialog now
  reports tokenizing, parsing, semantic analysis, form/generated-code
  collection, solution packaging, compiling and binary copy as distinct
  steps instead of one coarse "Parsing & analysing…".
- **Both progress modals (Build, KB reindex) gained a Hide button** — the
  dialog can be dismissed while the work continues in the background; the
  result still lands in the Output panel. (Label translated ×6.)

## [PowerRustCOBOL 1.44.1] — 2026-07-30

### Fixed

- **File → Reindex Knowledge Bases now shows a progress modal.** The
  reindex reported only Output-panel lines, so selecting it looked like
  nothing happened. It now dims the IDE and shows the same modal shape as
  the Building dialog: a determinate bar driven per embedded record, a
  spinner, and the current "n/m — subject" label (the started status line
  until the first record reports). The Output panel keeps the final
  summary, including the embedding device.

## [PowerRustCOBOL 1.44.0] — 2026-07-30

### Added

- **File → Reindex Knowledge Bases.** A manual trigger for the same
  incremental sync every Grace workflow runs at start: the System KB always,
  the Project KB when a project is open. Runs on a worker thread — coarse
  progress lines (about one per 5%) and a final summary, including the
  embedding device, land in the Output panel; the menu item is disabled
  while a reindex is running. Only new/changed/deleted documents are
  re-embedded, so an up-to-date store finishes instantly. (Label, hover
  hint, and status lines translated ×6.)

### Removed

- **File-menu cleanup:** "Open COBOL…", "Open Form…" and "Import Form…"
  left the File menu — forms and sources open from the project tree
  (Ctrl+O still opens a COBOL file). The now-unreachable Open Form file
  dialog plumbing was deleted with them.

## [PowerRustCOBOL 1.43.2] — 2026-07-30

### Fixed

- **The welcome pane finally speaks French.** French no longer borrows the
  English quote pool: it gets its own curated set — the same subset the
  other languages carry, Segond-style scripture included — plus the newly
  added Marc 9:23, so the rotating quote follows the IDE's language in all
  six languages.

## [PowerRustCOBOL 1.43.1] — 2026-07-30

### Fixed

- **Welcome pane: Mark 9:23 joins the rotating quote pool** in all five
  localised sets (EN/ES/PT/JA/ZH; French shares the English pool), with the
  scripture reference localised per language like the existing Proverbs
  entries.

## [PowerRustCOBOL 1.43.0] — 2026-07-30

### Added

- **GPU embedding with a cool-running CPU fallback — one policy for both
  Knowledge Bases.** The semantic embedder (System KB and project KBs share
  it, for indexing and queries alike) now probes for a GPU at load: Metal is
  carried by every macOS build automatically, CUDA on NVIDIA Linux/Windows
  is the opt-in `embed-cuda` feature (building it needs the CUDA toolkit).
  A GPU runs at
  full speed; the CPU fallback runs **low-power** by default, capping the
  compute pool at 2 threads so a reindex no longer pins every core and spins
  the fans — an operator-set `RAYON_NUM_THREADS` is always respected, and
  `PRC_EMBED_DEVICE=cpu|metal|cuda` forces a backend. The chosen device is
  reported by `build_chunked_kb` ("embedding device: …") and in the Models
  modal next to the semantic-model status (new label translated ×6). Default
  builds remain pure-CPU and self-contained; AMD/Intel GPUs on Linux/Windows
  stay on the CPU path (candle has no backend for them).

## [PowerRustCOBOL 1.42.2] — 2026-07-30

### Fixed

- **Grace could not set the new window properties.** The agent-side form
  property validator predated spec 037, so asking Grace to change MainForm,
  TaskbarIcon, CanMinimize, CanMaximize, WindowState, FullScreen or
  TitleVisible was rejected as "invalid property" even though the designer
  applies all seven. The validator and the designer's applier now agree on
  the full set, and the list-agreement test that caught the drift covers
  the new keys in both directions.

## [PowerRustCOBOL 1.42.1] — 2026-07-30

### Fixed

- **Setting the main form from the project panel left two crowns.** The
  "Form properties" view in the project panel edits a form through its own
  embedded designer state, and its MainForm claim never reached the
  app-level settlement that demotes the previous holder — checking the box
  there produced two forms with `main-form="true"` (and two crowns in the
  Forms tree). Claims from the inspect view now settle through the same
  path as designer claims: the previous holder is demoted in memory and on
  disk, the demoted form's cached tree entry is refreshed so its crown
  falls off immediately, and the crown override also honours the inspected
  form so the transfer is visible the same frame.

## [PowerRustCOBOL 1.42.0] — 2026-07-29

### Added — Main form, window lifecycle & multi-form invocation (spec 037)

- **Main form designation.** Every project now has exactly one **main form**
  — the form shown first and the app's single taskbar/dock identity. The
  first form created takes the role; move it with the new read-only-guarded
  **Main form** checkbox (checking it on another form un-checks the holder in
  one undoable action, and undo restores exactly the previous holder). The
  Forms tree crowns the main form; projects loading with zero or several
  holders normalise to the first in the list with a status notice.
- **Taskbar identity.** Only the main form carries the new **Taskbar icon**
  property; its window is the app's one taskbar entry (child windows are
  created skip-taskbar; the macOS Dock naturally shows one icon per app).
- **Window chrome & state.** New per-form properties honoured by the run
  form: `CanMinimize`, `CanMaximize`, `TitleVisible` (chromeless windows),
  `WindowState` (Normal/Minimized/Maximized, also settable at runtime) and
  `FullScreen` — orthogonal to WindowState, with an **onFullScreenChanged**
  event fired once per ACTUAL transition.
- **FormState (Ready/Waiting).** A `Waiting` form cannot be closed by any
  path — title-bar, handle `Close`, or cascade — and fires the new
  **onCloseRejected** event instead; a Sync caller is equally blocked while
  any of its Sync children is Waiting. Protects unsaved work.
- **OpenFormSync / OpenFormAsync.** `INVOKE me::"OpenFormSync"("FORM-ID",
  [windowState], [x], [y], [width], [height], [modal]) RETURNING H` — comma
  form with optional, RAD-defaulted trailing parameters (modal defaults
  true); the COBOL-standard space form requires every parameter and
  mismatches fail **at compile time**. Returns a `windowHandler`
  (USAGE OBJECT) with `Close` / `Focus` / `SetWindowState` /
  `SetFullScreen` / `SetTitleVisible` / `FormState`; handles NULL
  automatically when their form closes. Main form is a singleton; Sync
  children die with their caller; Async children survive it (main-form
  close still closes everything); modal Sync blocks the caller's COBOL flow
  until the child closes.
- **Status:** all lifecycle rules run headless-tested in the runtime and are
  live in the single-window run form. Hosting the OpenForm* child windows
  lands with the multi-viewport host (T1 spike findings pending) — until
  then child opens are accepted, logged, and immediately released so
  programs never deadlock.

## [PowerRustCOBOL 1.41.14] — 2026-07-29

### Fixed

- **Center/Right-aligned TextBox text overflows toward the correct side in
  the run form.** When the text grew wider than the box, the runtime editor
  always revealed the HEAD of the text regardless of alignment (egui's
  single-line editor only scrolls to follow the caret while focused, and
  anchors to the start otherwise). An unfocused overflowing box now shows
  the window the alignment implies — Left the head, Center the middle,
  Right the tail — matching the designer face. While the box is focused the
  caret stays in view as you type, so entering text at the end of a
  Right-aligned box naturally shows the tail.

## [PowerRustCOBOL 1.41.13] — 2026-07-29

### Fixed

- **TextBox and Label gain full text-alignment properties in the RAD.** The
  TextBox Basic properties showed no alignment at all, and the Label only
  offered horizontal Left/Center/Right. Both now expose **Horizontal
  alignment** (Left, Center, Right, Justified) and **Vertical alignment**
  (Top, Middle, Bottom), honoured by the designer face and the run form
  alike. Notes: Justified stretches wrapped lines of static text — a TextBox
  being edited falls back to left (egui's editor cannot justify live text) —
  and a multiline TextBox stays top-anchored vertically since its editor
  scrolls. Existing forms keep their exact look: missing properties default
  to Left / Middle, the historical behaviour.

## [PowerRustCOBOL 1.41.12] — 2026-07-29

### Fixed

- **TextBox font size (and family) now reach the run form.** The run-form
  editable overlay never set a font on its egui `TextEdit`, so every TextBox
  ran at the default ~14 px regardless of the designer's FontSize/FontName.
  Both the single-line and multiline editors now use the same
  `FontName` + `FontSize` the designer face paints with.
- **TextBox HintText is now actually shown.** The property existed in the
  model and the properties panel but nothing ever rendered it. The run form
  now shows it as the editor's placeholder while the box is empty (same font
  as the text, foreground colour at 55% so it reads as a hint on light and
  dark faces), and the designer canvas previews the same faded hint on an
  empty TextBox.

## [PowerRustCOBOL 1.41.11] — 2026-07-29

### Fixed

- **RAG efficiency now measures retrieval selectivity.** The statistics
  footer's `RAG efficiency` line previously subtracted the workflow's TOTAL
  consumed input tokens (system prompts, conversation, tool results, resent
  on every model call) from the indexed corpus, so a chatty workflow over a
  small Knowledge Base reported a misleading ~4% even when retrieval injected
  almost nothing. It now reports what selectivity actually saved —
  `available − injected` as a share of the corpus, the same measurement the
  verbose "Token savings" line always used — so injecting ~340 of ~31,000
  corpus tokens reads as ~98.9%, regardless of how much non-KB context the
  agents consumed. Injection is clamped to the corpus size (excerpt headers
  count toward injected but not toward the corpus).

## [PowerRustCOBOL 1.41.10] — 2026-07-29

### Fixed

- **Welcome-pane text outlined against bright backgrounds.** The welcome
  title, license line, quotation, and author credit are now painted with a
  1 px black outline (the glyphs drawn at the eight 1 px neighbour offsets in
  black beneath the coloured text), so they stay readable over the bright
  regions of the daily background photo. The outline follows the quote's
  fade-in/fade-out so it never lingers after its text.

## [PowerRustCOBOL 1.41.9] — 2026-07-29

### Added

- **Run statistics in the final balloon.** Every Grace workflow now closes its
  final chat balloon with a measured statistics footer: overall wall time,
  time by agent (busiest first, `×N` = model calls, typed extraction
  included), exact input/output token totals, the peak single-call context,
  and the RAG efficiency — `RAG efficiency: {}% ({} input tokens saved)`,
  computed as the indexed Knowledge Base corpus MINUS the input tokens the run
  actually consumed, as a share of that corpus. The same footer rides the
  contextual RAD-designer and code-editor chats (it travels inside the applied
  change-set's note, so "Applied N changes." carries it), the per-agent
  numbers persist on the run record (`agent_stats`, `total_elapsed_ms`,
  `peak_context_tokens`), and all six UI languages are covered.

## [PowerRustCOBOL 1.41.8] — 2026-07-29

### Added

- **The System Knowledge Base now teaches types, domains, and methods.** The
  generated `form_designer_controls.md` documents every property with its value
  type (Boolean/Integer/String — derived from the very defaults `Control::new`
  seeds, so it can never drift from the code), its default, and its allowed
  domain (enum values, ranges, formats), plus described events and per-control
  method signatures; a new `control_methods_reference.md` catalogues the entire
  closed inline-method vocabulary with parameter types and return values, and
  `rustcobol_extensions.md` gains the value conventions (booleans as `1`/`0`,
  hex colors, newline `Items`, TAB-separated grid rows, 0-based indexes) and an
  explicit warning that unknown methods degrade to property writes. A test pins
  the curated layer: a control property without documentation fails the build.

### Fixed

- **`Chart::AddPoint(label, value)` now actually plots.** The inline chart
  methods the Knowledge Base always advertised (`AddPoint`, `Clear`, `Refresh`)
  fell through to the generic property-write path and silently did nothing to
  the chart; they now drive the same `__ChartData` pipeline as the
  `CALL "COBOL-CHART-*"` runtime calls, and both forms share one canonical
  (upper-cased) data store so mixing them no longer forks the series. The
  misleading `IndexedFile-1::Open()` example was replaced with the correct
  `PERFORM <id>-OPEN` paragraph workflow.

## [PowerRustCOBOL 1.41.6] — 2026-07-29

### Fixed

- **The last undo gaps are closed — with redo.** Four designer actions still
  bypassed the history stack; all ride it now, both directions:
  *data-binding application* (snapshots the pre-apply bindings and target
  controls — binding rewrites DataGrid columns, sources, and preview
  values); *MenuBar definition saves* (the menu lives in a YAML next to the
  `.cfrm`; undo restores the previous file or removes one that did not
  exist, and the paint cache reloads on the next frame); *adding and
  deleting user procedures* in the COBOL Structure panel (the deleted
  procedure's code rides the stack and is restored verbatim); and
  *animations* — add, remove, and every field edit
  (`_AddAnimation` / `_RemoveAnimN` / `AnimN_*` returned before the stack).

### Added

- **Procedure history asks first.** Undoing or redoing any step that changes
  COBOL procedure code — a procedure add/delete, a procedure body edit, or
  an agent batch containing one — now waits for the developer's explicit
  confirmation (operator, 2026-07-29). Declining leaves the history
  untouched; further Ctrl+Z presses while the question is up do nothing.
  Translated in all six IDE languages.

## [PowerRustCOBOL 1.41.5] — 2026-07-28

### Added

- **Custom control backgrounds on styled forms — with a consent gate.** On a
  form whose GlassStyle is anything other than Classic, setting a control's
  BackgroundColor in the properties pane now asks once per form: the change
  breaks the unit of the themed style — continue? On confirm the colour is
  actually painted: an explicit background rides as a solid, opacity-aware
  layer under the styled face (Classic/Enhanced frost, Neumorphic surface —
  the DataGrid's spec-019 underlay, generalised to every rect control), and a
  Label with an explicit background finally gains a face at all instead of
  staying frameless. Style-seeded values (the universal `#F0F0F0` default and
  the Neumorphic surface colours) still mean "not chosen", so themed forms
  keep their unit untouched.

### Fixed

- **Dark → Light style switch kept white foregrounds — no contrast.**
  Neumorphic Dark forces every control's foreground to white; switching to
  Neumorphic Light only re-coloured data-input controls, leaving labels (and
  friends) white-on-light. The light applier now remaps the dark style's own
  white default to black on every control — a developer-chosen colour is left
  alone.
- **Form-level changes are undoable.** Changing the form's GlassStyle, Theme,
  Title, size, gradients, or any other form property bypassed the undo stack
  entirely. All form properties now ride it; a GlassStyle switch — which
  rewrites appearance defaults across every control — snapshots the full
  pre-switch appearance, so one Undo restores the exact previous look,
  user-chosen control colours included. Also swept in by the same audit:
  Visible, Enabled, and TabOrder mutated struct fields directly and were
  invisible to undo — they are undoable now.

## [PowerRustCOBOL 1.41.4] — 2026-07-28

### Added

- **Groq as a model provider.** New entry in the provider list (Settings →
  AI and the Models Manager), default endpoint
  `https://api.groq.com/openai/v1`. Groq speaks the OpenAI wire under the
  `/openai/v1` root: model listing at `/models` with Bearer auth (a stored
  chat-completions endpoint is stripped back to the root before the
  listing request), chat through the same OpenAI-compatible transport every
  non-Anthropic provider already uses. Note Groq's listing includes
  non-chat models (whisper, TTS, guard) — pick a chat model for agents.

## [PowerRustCOBOL 1.41.3] — 2026-07-28

### Fixed

- **A dead provider no longer holds a request open forever.** Observed
  live: the COBOL proficiency check hung endlessly on its Pedantic review
  round when the HuggingFace router stopped answering — the transport had
  no timeout and the spinner no cancel, so only restarting the IDE got out.
  Every rig provider client (workflow agents, typed extraction,
  chatbot/benchmark — Anthropic and OpenAI-compatible branches alike) now
  carries a **15-second connect timeout**: an unreachable or black-holed
  host fails the round fast, and the existing error path surfaces the
  failure instead of spinning. Streaming reads stay unlimited — a long
  generation is legitimate; only establishing the connection is bounded.

## [PowerRustCOBOL 1.41.2] — 2026-07-28

### Fixed

- **Thinking models no longer fail the connection test with an empty
  reply.** Observed live with `moonshotai/Kimi-K3` via the HuggingFace
  router: the Models Manager test clamps its budget to 16 tokens, which a
  hidden-reasoning model spends entirely on thinking ("56 reasoning
  character(s) but no assistant message content"). Two policy-first fixes:
  a built-in `model_policy` rule floors the token budget for the Kimi
  family (provider-agnostic — the same weights serve through HuggingFace,
  OpenRouter, and Moonshot's own API), and the mesh request funnel (which
  serves the connection test, the direct editor chat, and compaction) now
  applies `model_policy` budget floors exactly like the agent path already
  did.

## [PowerRustCOBOL 1.41.1] — 2026-07-28

### Fixed

- **HuggingFace moved to the Inference Providers router.** The legacy
  `api-inference.huggingface.co` host was shut down upstream — its DNS no
  longer resolves, so the Models Manager's model refresh died with "error
  sending request". The provider default is now
  `https://router.huggingface.co/v1` (OpenAI wire): model listing at
  `/v1/models`, chat at `/v1/chat/completions`. Stored configurations are
  migrated everywhere the dead host can hide: model profiles and the legacy
  top-level endpoint are rewritten on load (a dead host is never preserved
  as a "user-edited" choice), and both the model-list fetch and the Rig
  transport heal the endpoint at request time as a safety net — path
  included, since the old `/models/{id}` scheme died with the host.

## [PowerRustCOBOL 1.41.0] — 2026-07-28

### Added

- **Agent performance ratings, kept by Grace.** Every finished workflow is
  scored mechanically from its own record: approved with no correction
  round **+3★**, after one correction **+1★**, after two or more **0★**,
  task failed **−5** (totals can go negative; blocked agents are not scored
  — their dependency already took the −5). After the workflow, the Grace
  chat shows a per-agent star row: clicking fills stars cumulatively in
  gold — **4–5★ records the developer's praise (+5)**, **1–2★ a rejection
  (−10)**, 3★ neutral; clicking the same star again clears, re-rating
  replaces. Totals run over a rolling window of the last 20 rated tasks per
  agent, persisted in `agentic_ai/Grace/ratings.json`; the progress log
  reports each task's stars and the running total. Localized ×6.
- **Agents receive lessons, never scores.** A specialist's next task prompt
  carries a factual "RECENT REVIEW LESSONS" digest — its own recent
  correction reasons, deduplicated, capped at three. Deliberately NOT the
  star count or any praise/displeasure framing: a score is non-actionable,
  and pressure framing is the same gradient that once fabricated an
  approval claim. The correction reasons are what actually change the next
  completion.
- **Request-clarity pre-check, before any retrieval.** Grace privately
  rates every request's clarity and conciseness (0–10) BEFORE the Knowledge
  Bases are synced, embedded, or searched — on a tool-less call that cannot
  reach `knowledge_search`. Below 7, she returns her interpretation of the
  request plus targeted questions for confirmation instead of spending the
  full RAG + planning cost on a guess. The score is stored on the workflow
  record as private telemetry: agents never see it. The check fails open —
  an unparseable verdict lets the workflow proceed normally.

### Fixed

- **Fabricated review approvals are now a named critical defect.** The
  runtime Pedantic-relationship contract used to tell every specialist to
  "submit your complete work for that review" — an action a specialist
  cannot perform (the engine routes the review AFTER the reply), so models
  narrated having done it, up to "Aprovação obtida" with no review call
  anywhere in the run. The contract now states the real mechanics (the
  review runs only after the reply; the specialist never talks to the
  reviewer) and declares FABRICATED APPROVAL = CRITICAL DEFECT: any
  "submitted to the reviewer / review passed / approval obtained" sentence
  is false by construction, treated like a fabricated tool result, and
  voids the submission. The Form Designer's default prompt carries the same
  severe rule; being composed at runtime, the contract reaches every
  existing project immediately.

- **Rejected change-set operations now feed the correction loop instead of
  vanishing.** The Grace engine gained a machine-validation gate: every
  change-set-producing submission (Form Designer, COBOL Event Handler) is
  checked by the Form-independent half of the IDE's change-set validator —
  handler/procedure bodies missing the three division headers, unknown
  deploy control types, invalid deploy property keys — BEFORE any Pedantic
  round is spent on it. Proven defects go back to the specialist as a
  bounded correction round carrying the validator's errors verbatim,
  recorded on the audit trail under the "change-set validator" name; the
  gate also guards unreviewed tasks. Previously such operations validated
  invalid at apply time and were silently skipped — observed live as 60
  placeholder hover handlers that created nothing while the workflow
  reported success.
- **Grace no longer plans placeholder event-wiring tasks.** An event handler
  exists exactly when its approved COBOL implementation is applied — there is
  no dormant event slot to reserve first. Grace's prompt now forbids planning
  "connect/wire the events now with placeholder code, implement later" tasks
  (the pattern that produced the 60 rejected placeholders), and the Form
  Designer prompt states that it never emits `generate_event_handler`
  operations — placeholder or otherwise — returning delegation material for
  the COBOL Event Handler Script Agent instead. Both prompts upgrade on
  project open when the stored copy is the unmodified old default; edited
  prompts are never touched.

## [PowerRustCOBOL 1.40.0] — 2026-07-28

### Added

- **Chunked Knowledge Base — one record per subject.** Documents are
  converted into many small records — one per control, property, method,
  event, or prose section — each with a COBOL-style `PIC X(512)` content
  field; content that does not fit continues in records **linked to the
  previous one** (parent-record chains that search reassembles). Every
  record's content is embedded individually, the vector stored on the record
  it describes. Two stores: the IDE's `~/PowerRustCOBOL/data/chunked.data`
  (System KB) and each project's `data/<name-no-spaces>-chunked.data`.
  Syncing preserves the source files: a new or updated document has its old
  records and embeddings removed and is chunked and embedded again; a
  document deleted on disk loses its records.
- **The IDE ships its chunked store pre-embedded.** The System KB index
  (`assets/knowledge/chunked.data`, regenerated with
  `cargo run -p cobolt-ide --example build_chunked_kb` and embedded in the
  binary) installs itself to `~/PowerRustCOBOL/data/chunked.data` on first
  run — a cloned IDE reindexes nothing unless a Knowledge Base document is
  removed, changed, or replaced. A freshness test fails the build when the
  shipped store drifts behind the published documentation. Machines without
  the semantic model keep the shipped embeddings intact and search them
  lexically until the model arrives.
- **Indexing progress bar.** While chunk records are being embedded, the
  Grace chat and the form-inspector chatbot show a live progress bar
  ("Indexing Knowledge Base (n of m records)", translated ×6), streamed
  per record from the indexer.
- **The product is now "PowerRustCOBOL AI".** Everywhere the IDE shows the
  product name — window titles, the welcome screen, the About dialog — it
  reads PowerRustCOBOL AI, with the "AI" always in the brand cyan
  `#70f3fc` where text can be colored (`theme::brand_layout_job`). Folder
  names on disk keep the original spelling.

### Changed

- **Grace and the specialists now retrieve only what the task needs.** The
  planning context receives the top-scoring subject records from both
  chunked stores instead of the four essential documents injected wholesale
  — asking about `DataGrid` events now injects the DataGrid records, not
  the whole 34-control catalogue. The `knowledge_search` tool answers from
  the chunked stores (project + system) with complete subject records. The
  essential documents remain published, indexed, and rebuild-checked; their
  content simply arrives through retrieval. The verbose "Token savings"
  line now measures against the chunked corpus, so the effect is visible
  per run.

## [PowerRustCOBOL 1.39.0] — 2026-07-28

### Added

- **Verbose "Token savings" report.** With the verbose AI setting on, the
  Grace conversation ends each run with a measured retrieval-economy line —
  `Token savings: 92.50% — Knowledge Base retrieval injected ≈750 of ≈10000
  available tokens into the context.` — comparing what the RAG actually
  injected against the full indexed corpus a push-everything approach would
  have sent (≈4 chars/token estimate; translated in all six IDE languages).
  The measured counts persist on the run record (`kb_available_tokens`,
  `kb_injected_tokens`; older records still load).

## [PowerRustCOBOL 1.38.1] — 2026-07-28

One retrieval system, not three. The redb + Candle RAG (the
`multilingual-e5-small` model and pure-Rust tokenizer) is the only Knowledge
Base implementation; the superseded experiments are gone.

### Removed

- **The `local-retrieval` feature and its stack** — `embedding.rs` (ONNX
  Runtime), `retrieval/` (rig-sqlite + sqlite-vec + tantivy lexical index) —
  the pre-1.36.28, C/C++-dependent path. Off by default and unused; its
  dependencies (`ort`, `rig-sqlite`, `sqlite-vec`, `tantivy`, `ndarray`,
  `tokio-rusqlite`, optional `rusqlite`) leave the crate. SQLite remains in
  the COBOL runtime, untouched.
- **The dormant embedvec/Fjall vector store** in `knowledge_store.rs`
  (HNSW, E8 quantization) and the `embedvec` dependency. The module keeps
  what the live system uses: the `Embedder` seam, the hashing fallback, the
  shared vector width, and the data-directory location.
- **Unused `opentelemetry`, `opentelemetry-otlp`, `num_cpus`, and
  `cobolt-forms` dependencies** of the agents crate (verified unreferenced;
  full clean workspace rebuild passes without them).

### Fixed

- The `knowledge_search`/`documentation.write` tool contracts and the guide
  no longer call the index "SQLite" — it has been the pure-Rust redb store
  since 1.36.28.

## [PowerRustCOBOL 1.38.0] — 2026-07-28

Agent progress transparency (spec 036): the conversation always shows what the
agents are doing.

### Added

- **Live action status in the agent chats.** While Grace and the specialists
  work, the project Grace chat and the Form Designer's inspector chatbot show
  a per-agent status line naming the current step (`Form Designer Agent:
  Drafting response — T1`), throttled to at most one change per second, in
  place of the generic "Thinking…" indicator. Every step is kept in a
  collapsed **Agent actions (N)** history — attributed, ordered, lossless —
  that persists with the chat history and re-localizes when the IDE language
  changes. The canonical action vocabulary is translated in all six IDE
  languages.
- **Action log on the workflow record.** Each run's typed action history is
  saved as `action_log` in `agentic_ai/Grace/runs/<id>.json` (stable,
  language-neutral kinds; older records without the field still load), so a
  past run's steps stay reviewable in later sessions.

### Changed

- **The chat panes no longer show the raw progress log.** Status lines name
  actions only; retrieved context, tool payloads, and verbose payload lines
  (e.g. "Verbose: Loaded Skills…") no longer appear in the conversation in
  any mode. The full trace now flows to the Output panel's AI log, the
  diagnostics dump, and the saved run record. The finished run's
  "Coordination log" markdown balloon is superseded by the collapsed action
  history.

## [PowerRustCOBOL 1.37.0] — 2026-07-27

Milestone marker. The work from here to the production version is optimization
— speed, memory, and binary size under load — and this is the line it starts
from.

### Added

- **A per-OS build guide** at [`docs/BUILDING.md`](docs/BUILDING.md): Windows,
  Linux and macOS from a clean machine to a running IDE, with package lists for
  Debian/Ubuntu, Fedora and Arch, the test commands, where the artifacts land,
  and the failures worth naming with their fix. It is explicit that a C compiler
  **is** required — `libsqlite3-sys` (bundled SQLite for the COBOL database
  runtime) and `onig_sys` (the tokenizer's regex engine) compile C — while no
  C++ compiler, CMake, NASM, Python, Node or JVM is involved anywhere.
- **A benchmark harness** (`cobolt-bench`) and the 1.37.0 baseline at
  [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md). Every COBOL workload runs the same
  path a shipped binary takes — tokenize, parse, analyse, `Interpreter::run` —
  under a counting global allocator, so speed and memory churn are measured on
  the interpreter that is actually inside the binaries you hand over. Six
  workloads: interpreter dispatch (inline and paragraph-call), decimal
  `COMPUTE`, a 1000-row record batch written and read back, object
  create/destroy churn, and the INDEXED redb engine under bulk insert plus
  random-key reads. The last of those recovers the `open_table_cost`
  micro-benchmark that was `#[ignore]`d inside `cobolt-runtime::indexed_redb`
  and only ran when someone remembered an exact `--ignored` invocation.

  The baseline's finding: **the allocator, not the tree-walk, is the
  bottleneck**. 5.7 M statements/sec, but 24 M allocations to run 6 M
  statements — `ADD 1 TO ACC` on two `COMP` fields costs four trips through the
  allocator. A bytecode VM would make dispatch cheaper and leave that untouched,
  so the optimization work starts with allocation instead.
- **A crate inventory** at [`docs/DEPENDENCIES.md`](docs/DEPENDENCIES.md): every
  direct dependency with the version actually linked, which workspace crate uses
  it and what it is for, plus the ones declared behind an off-by-default feature
  and therefore absent from a normal build.

## [PowerRustCOBOL 1.36.29] — 2026-07-27

### Changed

- **The delivered package no longer carries `rcrun`.** Every build dropped a
  copy of the runner into the destination folder, roughly doubling what the
  developer hands over — about 99 MB of runner beside a 94 MB application — for
  something the application never calls. A built binary embeds its own compiled
  AST and links the interpreter and the render engine directly; it launches no
  process, so the runner was dead weight and an unexplained second executable
  next to the app. `rcrun` is unchanged as the developer's tool inside the IDE,
  where Run Form and debugging do spawn it.
- **The Apache-2.0 notices ship with the package, not just with `bin/`.**
  `LICENSE`, `NOTICE` and the runtime notice were written next to the build
  output but not into the destination folder the developer actually
  distributes. They are now written to both.

## [PowerRustCOBOL 1.36.28] — 2026-07-27

### Fixed

- **Themed forms keep their theme in the app you ship.** A form skinned with an
  asset-pack theme looked right in the designer, the preview and Run Form, and
  then rendered as plain procedural Liquid Glass the moment it was built into a
  binary and handed to someone else. The compiled-binary template published only
  the glass style to the renderer; it never resolved a theme pack, and the packs
  live in `assets/themes/` next to the IDE — a folder an end user's machine does
  not have. `rcrun build` now resolves each form's theme exactly as the IDE does
  (per-form override, else the project's `[forms] theme`, else Liquid Glass) and
  **embeds the pack into the executable**: the manifest plus the art its
  manifest actually references, keeping the binary self-contained with nothing
  to install beside it and none of the packs' authoring imagery along for the
  ride. The renderer reads that art from the embedded bytes on Windows, macOS
  and Linux alike, decoding the same PNGs into the same textures the designer
  drew with, so a themed form is identical in the designer, the preview, Run
  Form and the shipped app.
- **`UseThemeBackground` reaches every surface.** A form that opted into its
  theme's background got it on the designer canvas only — the preview, Run Form
  and compiled binaries painted the form's own background instead, because the
  themed backdrop was drawn by the canvas rather than by the shared render
  engine. It now lives in the engine's backdrop step, in the same order and with
  the same "themed wins over the form's own image" rule, so all four surfaces
  agree.

## [PowerRustCOBOL 1.36.27] — 2026-07-27

### Fixed

- **Control animations play in Run Form.** A fly-in, fade or pulse configured on
  a control ran in the designer preview and then did nothing at all once the
  form was launched: the animation clock lived inside the IDE, and the
  standalone `rcrun run-form` process had none, so every animated control simply
  drew in its final place. The clock, the per-kind transform math and the
  trigger mapping now live in `cobolt-forms` (`anim::AnimRuntime`), shared by
  every surface. The run form starts `OnFormLoad` / `OnShow` animations with the
  window, fires `OnClick`, `OnHover`, `OnFocus` and `OnTimer` ones from real
  interaction, and honours `Repeat` (Once / Loop / PingPong / Count) and the
  configured delay. COBOL's `PLAY ANIMATION`, `STOP-ANIMATION` and `PAUSE` also
  do something for the first time — they were being recorded and never acted on.
  Compiled binaries (`rcrun build`) had the same empty transform and now animate
  through the same runtime, so a form animates identically in the designer, in
  Run Form, and in the app you ship.

### Changed (build toolchain)

- **Building PowerRustCOBOL no longer needs a C or C++ compiler for its network
  stack.** Four dependency chains were pulling native code in for work that pure
  Rust already does:
  - **TLS** — `rustls` brought `aws-lc-rs` + `ring` (C and assembly) and `cmake`.
    Everything that speaks HTTPS (`reqwest` in cobolt-agents, `rig-core`, and
    `ureq` in the COBOL HTTP runtime) now uses the operating system's own TLS:
    schannel on Windows, Security.framework on macOS, OpenSSL on Linux — all
    reached through pure-Rust bindings, nothing compiled. `ureq`'s native-tls
    support is an adapter that its crate-level helpers never pick up on their
    own, so `http_runtime` routes every request through one shared agent that
    carries the connector; verified against a live HTTPS endpoint.
  - **`hf-hub`** — used in exactly one place, to fetch three embedding-model
    files, but it dragged in the whole xet transfer stack (`hf-xet`, `xet-client`,
    `xet-runtime`, `blake3`'s C SIMD) plus its own rustls. Replaced with three
    plain HTTPS GETs against Hugging Face's public `resolve` endpoint, written
    through a `.partial` temporary so an interrupted download cannot leave a
    truncated file that looks cached.
  - **`zip`** — we asked for `deflate` (pure Rust) but never turned its defaults
    off, so bzip2, lzma and zstd were compiled as C for codecs we never call.
  - **`esaxx-rs`** — see below.

  The project Knowledge Base index moved off bundled SQLite too. It was never a
  relational store — one table of path → (content, embedding), read by full scan
  and scored by dot product — so it now runs on `redb`, the pure-Rust embedded
  store the COBOL runtime already uses. The index is derived from the files in
  the Knowledge Base folder and every search re-syncs before it reads, so the new
  index rebuilds itself on the first query; nothing is lost and nothing needs
  migrating. The old `data/project-knowledge.sqlite` is simply left behind, and
  can be deleted whenever you like. `rusqlite` is now an optional dependency,
  reached only by the `local-retrieval` feature (its sqlite-vec extension needs
  rusqlite's FFI) — enabling that feature brings bundled SQLite, and a C
  toolchain requirement, back with it.

  Still native, and unchanged: `libsqlite3-sys` behind the COBOL-facing
  `db_runtime` (programs open real `.db` files, so it waits for a pure-Rust
  engine with a stable on-disk format and a synchronous API), and `onig_sys`,
  which `candle-core` hardcodes on its own tokenizers copy — candle 0.11.0 is the
  newest release and its dependency is neither optional nor feature-gated, so
  removing it would mean vendoring a patched candle.

### Documentation

- **The install instructions now match what the build actually needs.** They say
  explicitly that no Python, Node, JVM, CMake, NASM or C++ compiler is involved,
  that the single Visual Studio "Desktop development with C++" workload covers
  Windows end to end (the linker and SDK rustc needs for any Rust binary, plus
  `cl.exe` for the two remaining C dependencies), and why `libssl-dev` is
  load-bearing on Linux now that TLS is the operating system's.

### Fixed (Windows build)

- **`cargo build` links again on MSVC, and the build no longer compiles any C++.**
  Linking `cobolt-ide` failed with `unresolved external symbol "void __cdecl
  std::_Xlength_error(char const *)"`: tokenizers' default `esaxx_fast` feature
  switches esaxx-rs to a C++ suffix-array built against the STATIC C++ runtime,
  which the project's `/NODEFAULTLIB:libcpmt` deliberately bans. That code is
  only ever used to *train* a Unigram tokenizer — the BERT embedder only encodes
  — so `cobolt-agents` now takes tokenizers without default features and
  tokenizers falls back to `esaxx_rs::suffix_rs`, the pure-Rust equivalent. No
  C++ compiler is involved in a build any more, and `progressbar` (CLI progress
  bars, useless in a GUI) goes with it, dropping indicatif + console + two more
  crates. The MSVC targets also link `/defaultlib:msvcprt` now, as insurance for
  any future C++ dependency.

## [PowerRustCOBOL 1.36.16] — 2026-07-26

### Changed

- **Groundwork for a Knowledge Base the agents can query instead of being
  handed everything.** A vector store (embedvec with E8 quantization and cosine
  similarity, persisted through Fjall) and a semantic embedder (a pure-Rust BERT
  running on Candle, `all-MiniLM-L6-v2`) now exist as their own layer, with the
  IDE's own index living outside any project at `~/PowerRustCOBOL/data` and a
  project's own at `<project>/data`. Re-indexing an edited document replaces its
  vector rather than adding a second one, and a document deleted on disk leaves
  the index, so retrieval cannot serve text that no longer exists. Nothing
  queries this yet and no behaviour changes in this version — the agents are
  still handed their context directly. When the model is not present the
  embedder falls back to the previous word-matching behaviour and says so
  rather than pretending to understand meaning.

## [PowerRustCOBOL 1.36.15] — 2026-07-26

### Changed

- **Grace plans against the controls in play instead of the whole catalogue.**
  The context handed to her carried the property keys and the events of all 34
  control types on every request — measured on a one-control form, 32,758 of
  34,843 characters, the same bytes for every project and every request, while
  the form actually being edited accounted for about a thousand. She routes work
  to specialists; deciding that a button click belongs to the event-handler agent
  never needed a tree view's property list. Her copy now carries only the types
  on the form plus any the request names, the same trimming her specialists
  already received. The untrimmed context is still what each specialist's own
  view is sliced from, so a task whose objective names a type the request never
  mentioned — "deploy a Timer, then wire its tick" — still reaches the agent
  with that type intact.

## [PowerRustCOBOL 1.36.14] — 2026-07-26

### Fixed

- **The event agent can now see the events it is told to bind to.** Its prompt
  requires "the EXACT event name from the delegation context", and its own
  self-check requires every reference to appear in that context — but the
  `EVENTS BY TYPE` legend sat between the two slices it was sent, so it reached
  no one. The name is then checked on the way in, and a control that has no such
  event makes the operation invalid, and an invalid operation is skipped in
  silence: guessing `onChange` for a data grid that calls it `onSelectionChanged`
  produced an approved workflow and no handler, with nothing said. The legend now
  travels with the task, trimmed to the types the task can actually touch — those
  on the form plus any named in the objective, so wiring a control the same
  workflow is about to create still works.
- **It can also see the procedures it is told to call.** The prompt tells it to
  factor shared logic into a common procedure and `CALL` it by name while the
  existing names were sliced away. A call target is not checked on the way in, so
  a guessed name reached the form and failed later at compile time rather than at
  the point it was written.

## [PowerRustCOBOL 1.36.13] — 2026-07-26

### Fixed

- **An event handler asked for on its own now reaches the form.** "Add code on
  the onClick event of Button-1" is planned as a single COBOL Event Handler
  Script Agent task — no form-design task is needed when every control already
  exists — and that whole branch was disconnected from the form. The agent was
  told to return its work "as a `generate_event_handler` operation inside the
  operations array" but was never shown the fenced-JSON envelope that phrase
  refers to, because the change-set contract was handed to the Form Designer
  alone; so it described the operation in prose instead. Even had it emitted a
  perfect block, the three places that collect, recover, and apply change-sets
  each matched on the Form Designer's name and would have skipped it. The
  workflow ran, the pedantic reviewer approved the handler on its merits, and
  the form was left untouched. All four sites now share one definition of which
  agents produce form change-sets, so the handler is written to the control.
- **A request that adds a control *and* wires its event no longer loses one of
  them.** The contextual designer chat took the first approved change-set it
  found and dropped the rest; the operations from every approved task are now
  merged into the single block it applies.
- **An approved task that changed nothing says so.** When a form or event task
  passes review but returns no applicable operation, the reply now states that
  the form is unchanged instead of reporting a bare success — the silence is
  what left the missing handler to be discovered by hand.

## [PowerRustCOBOL 1.36.12] — 2026-07-26

### Changed

- **Neumorphic Light seeds new forms and controls with its own colours.** A form
  created in that style now takes `EAEBEF` as its background, and a control
  arriving on such a form is seeded with a South gradient from `F8F8F8` to
  `DFE0E1` — the same shape Neumorphic Dark already used, which had the gradient
  while Light did not. These are default *property values* written once at
  creation, not painting constants: the painter still reads whatever each
  control carries, and every one of them stays editable in the properties grid
  afterwards. The flat surface colour underneath is unchanged, so turning a
  control's gradient off returns it to the previous look.
- **A control pasted from another form adopts this form's style.** Paste cloned
  the source control verbatim, so a control copied from a Classic form kept its
  Classic styling while sitting on a neumorphic surface — the toolbox drop had
  seeded the style since it was introduced, but paste had not. A paste *within*
  the same form is still a plain duplicate and keeps whatever the developer
  customised on the original.

## [PowerRustCOBOL 1.36.11] — 2026-07-26

### Fixed

- **The current Claude models answer again instead of returning 400.** Every
  request carried the profile's `temperature`, and Anthropic removed the
  sampling parameters from Claude Opus 4.7 onward: Opus 4.7 and 4.8, Opus 5,
  Sonnet 5, Fable 5 and Mythos 5 reject a request carrying one outright —
  `` `temperature` is deprecated for this model `` — so the whole call failed
  rather than degrading, and Grace reported a model error for a perfectly valid
  profile. The parameter is now withheld from the models that reject it, on both
  paths that build a request: the streamed chat and the tool-calling loop.
  Fixing one and not the other would have left every agent that uses a tool
  still failing.

  The models that accept it are held as an **allowlist**, so an Anthropic model
  this build has never heard of — every model released after it — is assumed to
  reject the parameter. That is the fail-safe direction: omitting `temperature`
  costs only a setting those models ignore, while sending it costs the entire
  request. Every other provider keeps receiving it exactly as before. Note that
  on those models the profile's temperature now has no effect, because the model
  has no such control to set.

## [PowerRustCOBOL 1.36.10] — 2026-07-26

### Added

- **A Neumorphic Cobalt theme.** It is Neumorphic Dark's construction wearing
  Cobalt2's palette: Panel, Control and Code share one flat opaque surface —
  Cobalt2's own deep navy — so depth comes from the soft relief halo rather than
  from differently-coloured surfaces, with Hover lifted and the alternating-row
  tint recessed by the same steps Neumorphic Dark uses. Every hue is Cobalt2's,
  including the gold accent, the cyan hyperlink and the whole editor syntax set,
  so code reads identically in either theme and only the chrome differs. The
  relief's lit rim is navy rather than Neumorphic Dark's mid-grey, which on a
  blue surface would read as a smear.

### Changed

- **The Models Manager fills its model list on open.** The list was empty every
  time the manager opened and stayed empty until you pressed Refresh — and
  selecting a different profile cleared it again, so the same press was needed
  once per profile. It now fetches when the manager opens and whenever the
  selection moves to another profile. The fetch is skipped where it could only
  fail — a profile with no provider yet, or a remote provider with no key — so
  opening the manager never raises a provider error you did not ask for; a local
  Ollama, which needs no key, lists freely. Refresh stays where it was.

## [PowerRustCOBOL 1.36.9] — 2026-07-26

### Fixed

- **The Models Manager can list the Anthropic models again.** Refreshing the
  list returned "404 Not Found" with an empty body, which reads as a dead
  endpoint or a rejected key when it was neither. The request was wrong three
  times over. It asked the API root, `https://api.anthropic.com/v1`, because
  only the OpenAI branch ever appended `/models` — every other provider had its
  configured endpoint used verbatim, and a GET at that root answers nothing.
  It sent the key as `Authorization: Bearer`, which at Anthropic means an OAuth
  token rather than an API key. And it omitted `anthropic-version`, which every
  Anthropic request must carry whether or not it carries a credential. The
  listing URL is now derived for Anthropic as it already was for OpenAI —
  from the provider default `…/v1` and from the `…/v1/messages` endpoint a
  saved profile holds — the key goes out as `x-api-key`, and the version header
  is always sent.
- **A Google Gemini key is sent as a Gemini key.** Its `x-goog-api-key` header
  was attached only in the branch that ran when no key was configured, so a
  real key was sent as a bearer token and an empty one was sent as the Gemini
  header. Both are now chosen by provider rather than by whether the key is
  blank. The OpenAI family keeps the bearer token it has always used.

## [PowerRustCOBOL 1.36.8] — 2026-07-26

### Changed

- **A folder in the project tree creates files in itself.** The category `[+]`
  always creates in the category root, so the only way to get a form into
  `forms/customers/` was to create it in `forms/` and drag it down. Every folder
  row now carries its own `[+]`, which opens the same New Form dialog pointed at
  that folder — and the same for the Indexed Files and Common Code trees, whose
  headers already offered the matching `[+]`. Generated Code has no `[+]` on its
  folders, for the reason its header has none: the form compiler writes it.
- **A folder row carries a delete icon next to its name.** Deleting a folder was
  reachable only through the right-click menu. The icon is the same one form and
  indexed rows already show, and it takes the same route: it asks for the
  confirmation window — which names the folder and warns that everything inside
  it goes with it — rather than deleting anything itself. Confirming removes the
  folder from disk, drops every file under it from the project, and closes their
  open designers and inspectors.

### Fixed

- **The "Delete form" confirmation is translated.** It was the one confirmation
  window still hard-coded in English, so a developer working in Spanish,
  Portuguese, Japanese, Chinese or French was asked to confirm a deletion in a
  language the rest of the IDE was not using. It now reads from the same string
  table as every other dialog.

## [PowerRustCOBOL 1.36.7] — 2026-07-26

### Fixed

- **A property whose name the AI spelled in a different case no longer vanishes.**
  Property names are case-insensitive and the change-set validator accepted any
  casing, but the apply path matched the canonical spelling exactly: a form
  property sent as `title` fell through to a do-nothing default arm, and a
  control property sent as `caption` was inserted as a *second* map entry beside
  the real `Caption`, where the exact-match lookup kept returning the old value.
  Either way the operation was counted and reported to you as applied while
  nothing changed. Both paths now resolve the name case-insensitively and write
  through the canonical key, and undo captures the true previous value. A test
  holds the validator's list and the designer's list in step, so a property that
  can be validated but not applied fails the build.

### Changed

- **Grace's Form Designer is given the property list it is required to obey.**
  Its prompt forbids any property key not listed under `FORM PROPERTIES` or
  `PROPERTY KEYS BY TYPE`, and its reviewer treats those lists as authoritative
  evidence — but a task delegated through Grace had both blocks cut from its
  context to protect the token budget, so the specialist was obeying a list it
  could not see. It now receives the form-level block plus the property keys of
  the control types actually in play: those already on the form, and any named
  in the task objective (the control a deploy task is about to create is not on
  the form yet). The other thirty-odd types stay out, so the budget the cut was
  protecting is still protected.
- **The project-wide Grace chatbot can see the open form.** It sent only the
  surface name and the conversation, so a request typed there — including the
  panel's own suggestion, "Add a data bound datagrid to form xxxxx" — reached the
  delegated designer with no control ids, no geometry and no property keys. It
  now carries the same context the designer's own AI panel sends when a form is
  open, and the project tree inventory when none is, so Grace can name real
  forms, indexed files and sources instead of inventing them.

## [PowerRustCOBOL 1.36.6] — 2026-07-26

### Changed

- **The Event Handler agent now carries the RustCOBOL language contract.** Its
  prompt used to say "emit COBOL-85 conformant code" and "format COBOL strictly
  for the IDE parser" without ever stating which verbs, intrinsic functions,
  level numbers or source format this toolchain actually accepts — so the agent
  filled the gap from general COBOL knowledge, and standard-looking code that
  this parser rejects was the predictable result. The prompt now names all of
  it: the 51 implemented statements, the 39 resolvable intrinsics, the DATA
  DIVISION rules, the `::` control syntax, the file and exception extensions,
  and an eleven-point semantic self-check drawn from what the analyzer actually
  reports (undeclared identifiers, undefined PERFORM targets, non-numeric
  receivers, duplicate names). It also corrects the source-format rule: RustCOBOL
  is parsed free-form with **no line-length limit**, and the punched-card 72-column
  truncation applies only to a file that carries a real fixed-format indicator in
  column 7 — which is now stated as a trap to avoid rather than a margin to obey.
  A project seeded with the previous prompt is upgraded when it is opened; a
  prompt the developer has edited is never touched.
- **Its Pedantic reviewer checks that contract clause by clause.** The reviewer
  was told to treat the agent's prompt as the authoritative specification, which
  it could not do while that prompt named no rules. It now carries the matching
  checklist — unlisted verbs, unlisted intrinsics (which fail silently as zero or
  spaces rather than erroring), undeclared identifiers, undefined PERFORM
  targets, non-numeric receivers, duplicate names, a `PIC` on a group, level
  `78`, a wrapper division inside a handler body, `CALL "COBOL-SET-PROPERTY"` in
  place of `::` — and is told to cite the clause it is rejecting against. It is
  also barred from two false positives that would reject correct work: demanding
  a column-72 margin (there is none) and demanding proof that a handler already
  ran (it is reviewing a proposal, so no such evidence can exist). The reviewer
  is upgraded on the same terms as the agent.

## [PowerRustCOBOL 1.36.5] — 2026-07-26

### Fixed

- **The "Set up this project's AI" invitation no longer disappears behind your
  back.** Clicking *Models Manager* or *Manage Agents* closed the invitation, so
  when you finished with the manager you landed in the IDE — after setting the
  model you had to go find Project Settings yourself to set the agent. The
  invitation now stays open for as long as you need it: it steps aside while a
  manager is up and comes back when that manager closes, so both the model and
  the agent can be set from the one place. Only ✕ or *Later* dismisses it.

## [PowerRustCOBOL 1.36.4] — 2026-07-26

### Changed

- **The language selector has flags again — drawn in the theme's own ink.** They
  are painted, not emoji (this toolkit never renders an emoji flag as a flag),
  and monochrome: every tone comes from the active theme, so the flags read as
  part of the interface in all 31 themes instead of as a pasted-on colour patch.
  Each flag is identified by its shape — canton and stripes, rhombus and disc,
  horizontal versus vertical bands, a lone disc, a star — because two theme ink
  tones can be as close as 1.17:1 and tone alone could not carry the difference.
  A test holds every theme to the WCAG AA contrast minimums (4.5:1 for the
  primary charge, 3:1 for the secondary), so a future theme with washed-out text
  colours fails the build rather than shipping an unreadable flag.

## [PowerRustCOBOL 1.36.3] — 2026-07-26

### Changed

- **Plain language names in the selector.** The flags are gone — the list now
  shows only each language's own name (English, Português, Español, Français,
  日本語, 中文). Flag glyphs render differently on every platform, and the emoji
  form does not render as a flag at all in this toolkit.

### Fixed

- **Diagnostic files land in the right place on Windows.** Five diagnostics
  (`databinding.log`, the per-control diagnostics dump, the render-side databind
  trace, the debugger log and the Run-Form inspector dump) opened `/tmp/…`
  literally. Windows has no `/tmp`, so every one of those writes silently failed
  and the diagnostics appeared to do nothing. They now resolve per platform:
  `/tmp` on Linux and macOS exactly as before, `%TEMP%` on Windows.
- **Run Form finds its runtime on Windows.** The IDE looked for the bundled
  `rcrun` next to itself by that exact name; on Windows the sibling is
  `rcrun.exe`, so the lookup missed it and Run Form worked only if `rcrun` also
  happened to be on `PATH`. The executable suffix is now applied, matching how
  the rest of the toolchain already resolved it.
- **`inspect_actors` takes its paths as arguments.** This developer aid had two
  absolute macOS paths into one contributor's project folder compiled in, so it
  could not run anywhere else.

## [PowerRustCOBOL 1.36.2] — 2026-07-26

### Fixed

- **Appearance → Back colour now paints a Shape.** The renderer took a Shape's
  face from its type-specific **Fill colour** whenever that property was present
  — and every new Shape is created with one, so the Appearance section was dead
  for Shapes: setting a back colour changed nothing. Each property is now
  honoured only when it differs from its own default, so a Shape left on the
  standard silver fill takes the back colour you set, while a fill colour you
  chose yourself still wins over it.

## [PowerRustCOBOL 1.36.1] — 2026-07-26

### Fixed

- **The IDE remembers the language you chose.** It was held in memory only, so
  every start went back to English. The choice is now saved on the machine (not
  in the project, so the selector still works with no project open and nobody
  inherits a colleague's language) and restored at startup.
- **Real flags in the language selector.** The flags were emoji, and emoji flags
  are pairs of "regional indicator" letters that only become a flag if the text
  engine ligates them — egui does not, and the emoji font it bundles draws those
  codepoints as plain boxed letters, so Português showed as `B R`. The flags are
  now painted directly: the Brazilian green field, yellow rhombus and blue globe,
  and likewise for the other five. They are crisp at any size and identical on
  every platform.

## [PowerRustCOBOL 1.36.0] — 2026-07-26

### Added

- **Debug Settings (Help → 🐞 Debug Settings)** — every debug switch the IDE
  knows about now has one home, instead of a set of environment variables you
  had to remember and export. The modal groups them in one tab per area —
  **User Interface** (frame diagnostics, DataGrid component frames,
  rounded-corner GL clip), **Data Binding** (data-bind trace), **Agentic AI**
  (AI-pane layout debug), **Indexed Files** (INDEXED transaction log level and
  format) and **Logging** (tracing filter) — and each row's tooltip still names
  the variable it mirrors, so a shell run of `rcrun` remains reproducible. Two
  of these were never exposed in the UI before: the tracing filter (`COBOLT_LOG`)
  and the INDEXED transaction log. The switches are **IDE-wide**, saved on the
  machine rather than in `cobolt.toml`: design-canvas switches apply on the spot,
  and Run Form receives the rest as environment on its next launch.
- **License text loads with the license** — picking MIT, Apache-2.0, GPL-3.0 or
  any other entry in Project Settings now fills the license box with that
  license's canonical text (SPDX originals, placeholders intact) ready to edit.
  Terms you wrote yourself are never overwritten — only an empty box or
  still-untouched stock text is replaced — and a project that names a license but
  has no text picks it up when its settings open. The box scrolls at a fixed
  height, so a long license no longer stretches the form.

### Changed

- **Diagnostics moved out of Project Settings.** They were per-project settings
  in `cobolt.toml`; they are developer-machine aids, not project data, so they
  now live in the Debug Settings modal above. Old keys in an existing
  `cobolt.toml` are ignored. The Run-Form inspector dump rows stay in Project
  Settings.

### Fixed

- **An agent's model can be set back to "(no model selected)".** The choice was
  written to disk, but the built-in agent repair that runs on every project open
  could not tell "explicitly no model" from "never configured" and handed the
  agent a model back — so the setting looked like it never saved. The choice is
  now recorded explicitly and survives reopening; it also overrides the dormant
  connection kept on the agent for rollback, so nothing resurrects it.
- **Databound card members were blank in the run form.** A control-state entry
  created on the fly — a repeating-group card instance the interpreter writes to
  before the id exists — started out hidden and disabled, so the renderer skipped
  exactly the controls a data binding had just filled in, while a DataGrid on the
  same form painted normally. Such an entry now starts visible and enabled and
  inherits its designed control's properties, in both the `rcrun` run form and
  the compiled-binary template.
- **`/tmp/databinding.log` no longer grows unattended.** The control-array
  data-binding trace wrote to it on every refresh, once per row per mapping,
  whether or not anyone had asked for diagnostics (one report reached 1.4 MB).
  The writes are now behind the data-bind trace switch, and the summary lines
  also reach the `databinding` tracing target.

## [PowerRustCOBOL 1.34.0] — 2026-07-24

### Added

- **Collapsible RAD designer panels** — more room on the form-designer screen.
  The **toolbox** sidebar collapses (◀ chevron) to a narrow icon-only rail and
  expands (▶) back; the **properties** pane is now a drawer with a vertically-
  centered tab (◀ to hide, ▶ to slide back). Both open by default and stay
  **user-resizable** when open, and each restores the width you dragged it to
  when re-expanded — the panels never grow on their own.

## [PowerRustCOBOL 1.33.0] — 2026-07-24

### Added

- **Animated agent control moves (spec 035)** — when the AI agent repositions
  controls on a form, each moved control now **glides** from its old position to
  the new one, all controls together, over ~1 second with an ease-in-out curve,
  so the agent's layout work is visible instead of a jump. The effect is purely
  visual: the form model, saved `.cfrm`, and generated COBOL hold the final
  coordinates the instant the change is applied (a save or regenerate mid-glide
  is always correct), and only agent-applied moves animate — manual drags and
  newly created controls do not. A change-set arriving mid-animation retargets
  smoothly from the controls' current on-screen positions.

## [PowerRustCOBOL 1.32.0] — 2026-07-24

### Added

- **Grace target disambiguation (spec 034)** — now that the project tree has
  folders, a name can exist in more than one place, so the AI agent asks you to
  pick the target before it acts. When you ask Grace to **create** an element it
  opens a centered project-tree window to choose the destination **folder** (you
  can create a new folder inline); when you ask it to **edit** by name and more
  than one element matches, the window lets you pick **which one** (a single
  match proceeds with no prompt). Cancelling stops the operation. Implemented as
  a declared `project.select_target` agent tool that drives the modal over a
  blocking worker↔UI handshake; the chosen path is always project-relative.
  *Known limitation:* the compact editor/designer chat surfaces have no modal
  host, so an ambiguous request there is deferred to the full project Grace chat.

## [PowerRustCOBOL 1.31.0] — 2026-07-23

### Added

- **Project-tree folder management (spec 033)** — every top-level category
  (Forms, Indexed Files, Common Code, Generated Code, Assets, Knowledge Base)
  can now be organised into an arbitrary hierarchy of folders, so large,
  enterprise-grade projects stay navigable.
  - **Create / rename / delete folders** from a category's **📁+** button or a
    folder's right-click menu. Renaming rewrites every tracked path and open
    editor tab under the folder; **deleting a folder permanently removes it and
    all of its contents from disk**, drops those files from the project, and
    closes any editors showing them (after a confirmation).
  - **Drag-and-drop moves** — drag a file onto another folder within the tree,
    or drop files in from the OS file manager to import them into a folder. A
    document icon rides the cursor while dragging so the gesture clearly reads as
    moving a file. Same-name overwrites, moves into a folder's own descendant, and
    files whose type does not match the destination category are rejected.
  - **Keyboard navigation** — with the pointer over the tree, use ↑/↓ to move
    between rows, → to expand a folder (or step into it), ← to move up to the
    parent folder, and Enter to open the selected item.
  - All folder and file paths are stored **relative to the project folder**, so
    projects remain portable when moved, zipped, or shared.

## [PowerRustCOBOL 1.30.62] — 2026-07-22

### Fixed

- **Every event in the design view now fires at runtime (spec 021 completed)**
  — the Events panel advertised ~110 events but the runtime fired only ~40.
  Now implemented: keyboard (`onKeyDown`/`onKeyUp`/`onKeyPress`/
  `onEnterPressed`/`onEscapePressed`) and focus (`onGotFocus`/`onLostFocus`)
  for all focusable controls, not just TextBox; geometry
  (`onResize`/`onResized`/`onMove`/`onMoved`) and state
  (`onVisibleChanged`/`onEnabledChanged`) for every visual control; ComboBox
  `onDropDownClosed`; ListBox `onItemDoubleClick`; TabControl
  `onTabClick`/`onTabChanged`; TreeView node selection
  (`onNodeClick`/`onNodeSelect`/`onNodeDblClick`, with a visible selection
  highlight and `SelectedNode`); MenuBar `onMenuOpen`/`onMenuClose`; DataGrid
  `onCellClick`/`onCellDoubleClick`/`onRowDoubleClick`/`onColumnClick`/
  `onScroll`; Panel `onScroll`; PictureBox `onImageLoaded`/`onImageError`;
  Animator `onStarted`/`onFrameChanged`/`onLooped`/`onEnded`; charts
  `onDataChanged`; ProgressBar `onValueChanged`/`onCompleted`. Data controls
  dispatch operation events through the event loop: SqlDatabase
  `onConnectOk`/`onConnectError`/`onQueryComplete`/`onQueryError`/
  `onRowFetched`; the AI agent `onResponse`. Events with **no engine behind
  them** were removed from the design view instead of lying (drag-and-drop
  family, tree expand/checkbox/drag states, grid sort/column-resize/cell-edit,
  chart zoom/series-click, `onTooltipShow`, `onPropertyChanged`, IndexedFile
  per-operation events, agent streaming events). Spec 021's remaining
  event-data tasks (WS-EVENT-DATA items) stay open; payloads are now captured
  on the wire (`UiEvent.value`) ready for that plumbing.
- **Hardcoded values converted to properties** — the 200 ms hover threshold is
  now every control's `HoverDelayMs` property; Grace's correction-loop bound
  (was a fixed 2) is the project-wide "Pedantic correction rounds" setting in
  the Models Manager's project panel, persisted in `cobolt.toml`.

## [PowerRustCOBOL 1.30.61] — 2026-07-22

### Fixed

- **A valid plan whose final ```json fence the model never closed parsed as
  "no plan"** — observed live: Grace ended a correct one-task plan with `…}]}`
  and no closing backticks, the deterministic parser found no terminated
  block, and a needless extraction + re-ask roundtrip followed. An
  unterminated final fence now parses (in `last_json_block` and in the
  verbose pretty-printer alike).
- **Coordination gate false positive on schema documentation** — "Write the
  approved schema documentation … for the final .cidx resources. Do not
  modify .cidx resources." was rejected as restricted indexed-file mutation
  because "write" + ".cidx" tripped the heuristic and "do not modify" was not
  an exemption marker. The guard-phrase list now covers "do not modify" /
  "do not create or modify" / "must not modify" / "without modifying".
- **Reviewer names are enforced mechanically** — Grace's plan carried a
  fabricated reviewer ("COBOL Pedantic Agent") and referenced companions the
  operator had DISABLED, which would have failed tasks at review time. Each
  task's reviewer is now set host-side to exactly the responsible agent's
  ENABLED companion (fabrications replaced, disabled/missing companions clear
  the gate, every correction logged), and the planning registry only
  advertises enabled companions.
- **Typed-extraction failures are no longer invisible** — the deterministic
  parse failure's cause (the exact serde error) is logged, extraction errors
  land in the AI log and connection log, and verbose mode shows the
  extraction request, result, and token usage like any other call.

## [PowerRustCOBOL 1.30.60] — 2026-07-22

### Fixed

- **Agents on OpenAI-compatible providers failed with `JsonError: invalid
  type: null, expected a boolean`** — rig 0.40's `openai::Client` defaults to
  the OpenAI **Responses API** (`/responses`), which compatible gateways
  implement partially or not at all: Ollama Cloud's implementation echoes tool
  definitions with `"strict": null` where the Responses types demand a
  boolean, killing the whole task (seen live on Documentation Agent →
  `ollama_cloud/gemma4:31b`). Every OpenAI-compatible provider now goes
  through rig's **chat-completions** client (`CompletionsClient`) — the wire
  these providers actually mean by "OpenAI-compatible" and the one the
  pre-Rig transport always spoke. Applies to agent invocations, typed
  extraction, and the streamed editor/chat entry points alike. On that wire,
  real OpenAI rejects the classic `max_tokens` on current models (HTTP 400,
  "use `max_completion_tokens`"), so the provider-keyed parameter switch the
  legacy transport used is restored: profiles with provider `openai` send
  `max_completion_tokens`; every other gateway keeps `max_tokens`.
- **Agentic AI log: `rig ·` prefix dropped** — log lines now read
  `Grace → openai/gpt-5.6-terra`, not `rig · Grace → …`.
- **Verbose mode now logs the full interaction** — the developer request; per
  agent call the resolved wire target (`POST <base>/chat/completions`) and
  the complete composed request (system prompt, skills/knowledge, user
  message); the full response with fenced JSON pretty-printed for human
  reading; native tool calls with pretty-printed arguments and outcomes; and
  durations plus exact token usage. Everything goes to both the Agentic AI
  log and the connection log; errors log the same way. Non-verbose keeps the
  concise one-line entries.

## [PowerRustCOBOL 1.30.59] — 2026-07-21

### Fixed

- **Verbose AI log is presented as the project-wide setting it always was** —
  the toggle rendered inside the Models Manager's selected-profile editor
  (reading as a per-agent/per-model setting) while its intended home, the
  ⚙ Settings AI section, hid it behind the retired legacy-fields block. The
  single control now lives in ⚙ Settings → **AI Assistants** (section renamed
  from "AI assistant"); it governs every agent and chat surface and persists
  in `cobolt.toml`. The Agentic AI master switch moved out of the profile
  editor too, into its own "Project-wide AI settings (apply to every agent)"
  panel of the Models Manager, with both toggles' labels and hints properly
  localized in all six UI languages (they were English-only literals before).

- **Agentic AI: malformed plans, verdicts, and change-sets no longer burn
  correction roundtrips (Rig migration phase 3)** — Grace's workflow plan, each
  Pedantic round verdict, and the Form Designer's change-set are now obtained
  as **typed structures**: the deterministic fenced-JSON parse runs first (free,
  unchanged for well-behaved replies), and when it fails the SAME reply is
  recovered through the provider's native typed extraction (schema-forced
  `submit` tool call) instead of asking the agent to resend everything. The
  "malformed workflow plan → request one corrected plan" roundtrip is deleted —
  a full re-plan could silently drift from the original. A review whose verdict
  cannot be parsed no longer silently counts as "defects" (which charged the
  *specialist* a correction round for the *reviewer's* formatting); the verdict
  is extracted, and only an unobtainable verdict fails the task, honestly.
  Approved Form-Designer submissions are canonicalized on the worker thread, so
  the apply path stays deterministic and the audit record keeps the original
  submission as evidence. Extraction token usage joins the workflow totals.
  A reply extraction cannot repair — one carrying **no tasks at all** — is
  Grace talking (a clarifying question, a refusal, an answer) despite the
  action classification: the engine re-asks once (plan, or ask the developer
  plainly), and if the retry still has no plan, Grace's actual words are
  relayed to the developer as a direct reply instead of the previous opaque
  "could not produce an executable workflow plan" error. Both plan-less
  responses are captured verbatim in the connection log.
- **Agentic AI: the hand-rolled HTTP orchestrator is retired (Rig migration
  phase 4)** — the last four legacy entry points (AI Dev Agent chat, editor
  assistant, history compaction, connection test) now run on the same Rig
  transport as Grace's workflows, and the bespoke wire/streaming code
  (`cobolt-agents::orchestrator`, ~950 lines) is deleted. Preserved unchanged:
  multilingual specialist routing and prompt composition (now in
  `cobolt-agents::specialist`), live text streaming into the chat panels,
  change-set pagination (`has_more`/`next_cursor` batches merge into one
  change-set), the provider-scoped endpoint heal, the reasoning-only-reply
  guard, verbose request/response traces in the connection log, and exact
  token usage. Behaviour notes: every OpenAI-compatible provider (including
  Ollama's `/v1` endpoint) speaks the chat-completions wire — the
  Ollama-native `/api/chat` wire and the OpenAI Responses wire are no longer
  used by these entry points; Anthropic profiles now work here through the
  native client (the legacy path could not reach them at all); a multi-batch
  paginated change-set no longer drops the `note` of single-batch replies.

## [PowerRustCOBOL 1.30.58] — 2026-07-20

### Added

- **Async I/O for RestClient (spec 032)** — A `RestClient` HTTP call no longer
  blocks the whole form while it runs. `RestClient` is now **async by default**:
  `GET`/`POST`/`PUT`/`DELETE` start a background worker, set the control's new
  `Busy` flag, and return immediately; the event loop keeps dispatching (timer
  ticks, other controls) and the response arrives as an `onComplete` (or
  `onError`) event. A generic async engine in the runtime delivers completions
  through the existing event channel — `COBOL-WAIT-EVENT` drains results and
  dispatches their lifecycle event, one per return. New per-control surface on
  `RestClient`, `SqlDatabase`, and `IndexedFile`: properties `Mode`
  (`Async`/`Sync`), `Busy`, `TimeoutMs`; a `Cancel()` method; and the uniform
  events `onComplete`, `onError`, `onCancelled`, `onTimeout`. `Cancel()` and
  timeouts abandon the worker safely (a generation check discards any late
  result). `SqlDatabase` and `IndexedFile` remain **synchronous by default**
  (fast local ops), with the new properties/events available for opt-in.
- **Compatibility note** — Existing `RestClient` forms that read `ResponseBody`
  on the statement after a `GET` should set the control's `Mode` to `Sync` to
  keep the original blocking, same-statement-result behaviour.

### Fixed

- **Light strip on the run-form window's right and bottom edges** — A running
  form window was created 4px larger than the form and hosted its content in a
  `ScrollArea::both`, so the scrollbar gutter / panel background showed as a
  faint light border along the right and bottom edges. The window is now sized
  exactly to the form and the host panel uses floating scrollbars (which overlay
  content only when a resized window actually needs to scroll), so no border
  shows.

## [PowerRustCOBOL 1.30.57] — 2026-07-19

### Fixed

- **Intermittent 401 from the model provider** — Every config write staged
  through one shared `llm_config.json.tmp`. `File::create` truncates, so two
  concurrent writes could interleave into that staging file and publish a
  corrupt primary config on rename. A corrupt primary falls back to the backup,
  and if that is also unusable, to defaults — whose `api_keys` are empty.
  Requests then went out with a blank credential and the provider answered
  `401 insufficient permissions`, which reads like an account problem. Each
  write now stages through its own file.
- **Blank credentials are reported as themselves** — A request with no API key
  for a remote endpoint is refused before it is sent, naming the provider,
  model, and endpoint, instead of surfacing the provider's opaque 401. Local
  providers, which need no key, are unaffected.
- **Pedantic reviewers may not be task agents** — Grace sometimes planned a
  redundant "review the completed work" task with the Pedantic reviewer as its
  responsible agent. Every task already carries its reviewer, and reviewers are
  provisioned with no model of their own, so that task also resolved empty
  credentials and failed. Such a plan is now rejected and Grace is asked to
  reassign the work to the owning specialist.

## [PowerRustCOBOL 1.30.56] — 2026-07-19

### Fixed

- **Existing projects kept the broken agent prompts** — Agent prompts are files
  inside each project, so correcting the shipped defaults in 1.30.55 only
  reached newly created projects. An existing project went on instructing its
  Form Designer to set `Theme` to an invented `"neumorphic-dark"` through an
  operation the applier discards, and its Pedantic Reviewer to demand proof
  that a change had already been applied. Opening a project now replaces any
  built-in prompt still carrying that superseded guidance, while genuine
  project prompt edits are still preserved.

## [PowerRustCOBOL 1.30.55] — 2026-07-19

### Fixed

- **Agent form-style requests** — Asking Grace to restyle a form (for example
  "change the form theme to neumorphic dark") now applies. The agent prompts
  named the wrong property and an operation the applier never accepted, so the
  work was discarded even when the reasoning was right. Restyling is now the
  form-level `GlassStyle` property, set with a single `set_property` operation
  targeting `Form`, and the accepted values are stated exactly as the parser
  spells them — an unrecognised value silently fell back to `Classic`.
- **Form Designer change-set schema** — The Form Designer Agent is now told the
  change-set schema its submissions are parsed with. It previously described
  edits in prose or tables, which applied nothing.
- **Reviewer approval deadlock** — The Form Designer Pedantic Reviewer no
  longer demands proof that a change was already applied. Change-sets are
  applied only after approval, so that evidence could never exist and every
  correction loop ran to exhaustion. The reviewer now judges the proposed
  change-set on what is checkable before approval.
- **Form properties in agent CONTEXT** — Requests now carry the form's current
  `GlassStyle`, `Theme`, and `UseThemeBackground` values plus the supported
  style names, so agents can read the current style instead of guessing it.

### Changed

- **Agent project-scope boundary** — Every delegated agent now receives an
  explicit boundary: it may create and modify forms, controls, events, indexed
  files, Knowledge Base documents, and project sources in the developer's open
  project, but may never change the IDE's appearance, settings, or
  configuration, nor add or reconfigure agents and model profiles. The
  read-only `egui.*` tools observe the live IDE window, and IDE widgets in
  their output are never valid change-set targets.

## [PowerRustCOBOL 1.30.54] — 2026-07-19

### Fixed

- **Neumorphic Dark gradients** — Form and control backgrounds now carry the
  optional eight-direction gradient data through the shared renderer. The
  Neumorphic Dark style applies the requested charcoal surface, non-white
  highlight shadow, and southward `#4E4E4EFF` to `#000000FF` gradients.

## [PowerRustCOBOL 1.30.53] — 2026-07-19

### Fixed

- **Project Knowledge Base precedence** — Grace now synchronizes and searches
  the project-local Knowledge Base before every request, treats relevant
  project evidence as authoritative over model training, cites matching paths,
  and avoids inventing missing project facts.
- **Knowledge Base project tree** — New projects use `Knowledge Base/`; legacy
  `Documentation/` and `docs/` trees migrate without overwriting conflicts.
  The project explorer now renders nested Knowledge Base folders and supports
  explicit subfolder creation and confirmed recursive deletion with manifest
  and SQLite-index cleanup.

## [PowerRustCOBOL 1.30.52] — 2026-07-19

### Fixed

- **Indexed schema handoff routing** — Documentation Agent may prepare,
  normalize, and describe an indexed-file schema without being misclassified
  as the agent that writes the `.cidx`. Only an explicit Indexed File write is
  treated as mutation reserved for Data (Indexed File) Agent.
- **Grace direct Markdown responses** — Read-only questions and requests to
  describe, explain, summarize, compare, suggest, or recommend now return
  readable Markdown directly in the chatbot. Structured workflow JSON remains
  mandatory when the request also changes project resources.

## [PowerRustCOBOL 1.30.51] — 2026-07-19

### Fixed

- **Indexed-file specialist tandem** — New and existing projects now receive a
  fixed Data (Indexed File) Agent and its purpose-specific Pedantic reviewer.
  Grace enforces a Documentation Agent schema handoff covering the file name,
  purpose, project knowledge, 1NF/2NF/3NF, helper files, and the developer's
  explicit UUID-or-PIC choice before any mutation.
- **Governed Indexed File UI tools** — Only the Data agent can list, inspect,
  or write `.cidx` definitions. Writes validate records and keys, regenerate
  Indexed File UI COBOL/copybook artifacts, preserve existing indexed data, and
  refresh the open project's Indexed Files tree.

## [PowerRustCOBOL 1.30.50] — 2026-07-19

### Fixed

- **Chatbot response actions** — Completed agent balloons now show icon-only
  Copy and Save as Markdown commands with tooltips. Markdown saves are confined
  to the open project's `Documentation/` folder, indexed in the project's
  SQLite knowledge database, and added to the project tree without reopening
  the project.

## [PowerRustCOBOL 1.30.49] — 2026-07-19

### Fixed

- **Agents Manager lifecycle controls** — Delete Agent is hidden again after
  the legacy dangling-reviewer cleanup. Its implementation remains available
  behind the independent visibility flag.

## [PowerRustCOBOL 1.30.48] — 2026-07-19

### Fixed

- **Agents Manager Delete command** — Delete Agent is visible again and uses
  its existing confirmation and fixed-agent protection rules. New Agent remains
  hidden independently.

## [PowerRustCOBOL 1.30.47] — 2026-07-19

### Fixed

- **Markdown editor status bar** — Markdown tabs no longer show the
  COBOL-specific Beautify command. The underlying beautifier also ignores
  `.md` and `.markdown` documents if invoked programmatically.

## [PowerRustCOBOL 1.30.46] — 2026-07-19

### Fixed

- **Complete built-in agent tandems** — Every new project now receives Grace
  and all four fixed specialists together with a purpose-specific Pedantic
  reviewer. Reviewer names follow `<agent name> Pedantic Reviewer`; prompts,
  descriptions, routing, and one-to-one links are preconfigured, while model
  selection remains the developer's responsibility.
- **Project-open reviewer repair** — Opening an older project recreates and
  relinks any missing built-in reviewer without replacing project-edited
  prompts, skills, knowledge, tools, or selected model profiles. Legacy Grace,
  UI, and COBOL reviewer names migrate in place with stable IDs.
- **Agents Manager lifecycle policy** — New Agent and Delete Agent controls are
  hidden because the complete agent mesh is project-provisioned. Their code is
  retained behind a visibility flag for possible future use.

## [PowerRustCOBOL 1.30.45] — 2026-07-18

### Fixed

- **Chatbot composer layout** — The project-wide Grace chatbot now keeps its
  Send button immediately to the right of the multiline prompt instead of
  placing it below. Code-editor and Form Inspector chatbars use the same
  right-side command layout and reserve the button width while their panes
  resize.

## [PowerRustCOBOL 1.30.44] — 2026-07-18

### Fixed

- **Grace prompt isolation** — Named project agents now bypass unrelated
  built-in mesh-specialist preambles. Grace is routed as `Grace project agent`
  and no longer receives the contradictory CodeGenerator instruction to avoid
  JSON while producing a workflow plan.
- **Grace conversation handling** — Capability/help questions such as
  `What can you do?` return Grace's direct prose response without requiring a
  synthetic workflow. Actionable requests still require workflow JSON and get
  one automatic correction attempt when the first plan is malformed.
- **Complete planning diagnostics** — If both planning attempts are malformed,
  the error includes both parser failures and the complete corrected response
  payload for the IDE error modal and log pane.

## [PowerRustCOBOL 1.30.43] — 2026-07-18

### Changed

- **Canonical built-in agent definitions** — COBOL Event Handler Script Agent,
  Documentation Agent, Form Designer Agent, Grace Pedantic Reviewer Agent, and
  Version Control Agent now receive explicit role-specific routing and default
  prompts. Empty and known legacy defaults are repaired without replacing
  project-edited non-empty prompts.
- **Agent naming** — `DocumentationAgent` is now `Documentation Agent`, and
  Grace's built-in reviewer is now `Grace Pedantic Reviewer Agent` throughout
  planning, tool governance, prompts, and Agents Manager.

### Fixed

- **Non-destructive built-in migration** — Existing agent folders and prompt
  files migrate to the canonical names while preserving stable IDs, model
  profiles, companion links, tools, and custom prompts. The redundant
  `Orchestrator Pedantic Reviewer Agent` is merged and removed.

## [PowerRustCOBOL 1.30.42] — 2026-07-18

### Changed

- **Pedantic companion editor** — A Pedantic agent's companion section is now
  an editable `Pedantic Companion for` selector listing eligible Grace or
  specialist agents. The existing primary-side selector follows the same
  project-local one-to-one relationship.

### Fixed

- **One-to-one reviewer ownership** — Assigning a Pedantic reviewer now
  detaches any prior owner, replaces the selected agent's previous companion,
  and repairs legacy duplicate links deterministically.
- **Runtime reviewer awareness** — Grace planning and each agent's runtime
  instructions now identify the exact one-to-one reviewer relationship and
  prohibit substituting or sharing another agent's Pedantic companion.

## [PowerRustCOBOL 1.30.41] — 2026-07-18

### Added

- **Pedantic Grace Reviewer default** — New projects now receive a protected
  `Pedantic Grace Reviewer` linked as Grace's companion, with the complete
  orchestration-review prompt stored in the project's agent database. A
  configured legacy reviewer connection is adopted when available; otherwise
  the reviewer remains ready for a project model profile to be selected.

### Fixed

- **Reviewer prompt preservation** — Fixed-agent repair restores a missing
  Grace reviewer and its empty prompt, but never overwrites prompt content the
  developer edited in Agents Manager or replaces an explicitly selected custom
  Pedantic companion.

## [PowerRustCOBOL 1.30.40] — 2026-07-18

### Fixed

- **Grace property-pane title** — The project-wide chatbot header now reads
  `👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator` and wraps cleanly
  when the main property pane is narrow.

## [PowerRustCOBOL 1.30.39] — 2026-07-18

### Fixed

- **Agents Manager prompt editor height** — Agent prompts now use a vertically
  resizable editor with a hard maximum height of 20 text rows. Longer prompts
  remain editable through internal scrolling and can no longer expand the
  surrounding detail pane.

## [PowerRustCOBOL 1.30.38] — 2026-07-18

### Fixed

- **Internal agent folder visibility** — The project tree now hides the
  internal `agentic_ai/` directory in both normal project mode and raw-tree
  fallback mode. Agent configuration and workflow records remain on disk and
  continue to be managed through Agents Manager and Grace.

## [PowerRustCOBOL 1.30.37] — 2026-07-18

### Fixed

- **Documentation workflow ownership** — Grace now requires domain specialists
  to prepare authoritative source material and delegates all document formatting
  and project-document writes exclusively to `DocumentationAgent`. Interface
  documentation, for example, must collect its source from the Form Designer
  Agent before the document task can run.
- **Agent dependency handoff** — Approved specialist output is now included in
  every dependent agent task, allowing `DocumentationAgent` to format verified
  project facts instead of reconstructing or inventing them.
- **Documentation plan validation** — Invalid documentation plans are rejected
  structurally and returned to Grace for one correction attempt before any task
  executes.

## [PowerRustCOBOL 1.30.36] — 2026-07-18

### Added

- **Grace Chat welcome** — An empty project-wide Grace conversation now shows
  the requested getting-started, CRUD, data-binding, ERP planning, tasking, and
  implementation suggestions.
- **Fixed DocumentationAgent** — Every project now receives a protected
  `DocumentationAgent`. Grace must delegate project-document creation and
  updates to it, and the tool backend rejects documentation writes from every
  other agent.
- **Project knowledge vectors** — Text documents under `Documentation/` and
  `docs/` are synchronized into `data/project-knowledge.sqlite`. Documentation
  writes are indexed atomically, specialists receive governed
  `knowledge.search` retrieval, and relevant project knowledge is supplied to
  Grace while planning.

### Fixed

- **Grace-generated document tracking** — Documents created by Grace are added
  to the project's Documentation list and appear in the IDE project tree
  without reopening the project.

## [PowerRustCOBOL 1.30.35] — 2026-07-18

### Fixed

- **Form Designer window activation** — Double-clicking a form in either the
  IDE project tree or a RAD Forms list now restores, raises, and focuses its
  existing designer window. Newly opened designer windows receive the same
  activation request.

## [PowerRustCOBOL 1.30.34] — 2026-07-18

### Fixed

- **Chatbot user-message colour** — User chat balloons now use the requested
  opaque green `#61C654FF` consistently across every IDE chatbot surface.

## [PowerRustCOBOL 1.30.33] — 2026-07-18

### Fixed

- **Connection-test sampling** — Model connection tests now preserve the
  profile's configured temperature instead of silently forcing `0.0`, allowing
  models that accept only their default `1.0` temperature to be tested.

## [PowerRustCOBOL 1.30.32] — 2026-07-18

### Fixed

- **Project-wide Grace chatbot** — A width-responsive `👑 Grace` command now
  sits above the project tree and opens a project-scoped chatbot in the main
  pane with persistent history, progress, approvals, and conversation controls.
- **Contextual Grace routing** — IDE chat surfaces now enter through Grace with
  an advisory surface specialist preference. Grace remains free to delegate
  mixed requests across every enabled project specialist, including form and
  event-handler work in the same workflow.

## [PowerRustCOBOL 1.30.31] — 2026-07-18

### Fixed

- **Models Manager endpoint authority** — Conventional Chat Completions paths
  are appended only to untouched provider defaults. After the developer edits
  an endpoint, requests use that exact URL; saved non-default URLs migrate to
  the same behavior automatically.
- **Responses API wire format** — Explicit `/responses` endpoints now send a
  Responses payload and consume streamed Responses events instead of sending a
  Chat Completions payload to the corrected URL.

## [PowerRustCOBOL 1.30.30] — 2026-07-18

### Fixed

- **Models Manager Save behavior** — Save now commits the edited model profile
  and then closes Models Manager; unsuccessful commits leave the modal open.

## [PowerRustCOBOL 1.30.29] — 2026-07-18

### Fixed

- **Project-owned AI configuration** — Model profiles, provider/model choices,
  sampling, Agentic AI, verbose logging, and AI prompts now persist in the
  active project's `cobolt.toml`, alongside its project-owned agents.
- **Project-scoped credentials** — API keys remain machine-local but are keyed
  by stable project model-profile ids, so switching or saving another project
  cannot replace a project's models or credentials.
- **Credential deletion safety** — Machine configuration saves merge non-empty
  credentials and remove keys only after confirmed Models Manager deletion;
  deletion markers also prevent backup recovery from resurrecting deleted keys.
- **Legacy AI migration** — Projects without an `[ai]` configuration receive a
  one-time non-destructive import of legacy global model metadata, while
  missing keyed profiles and credentials can be recovered from the valid backup
  store unless they carry an explicit deletion marker.

## [PowerRustCOBOL 1.30.28] — 2026-07-18

### Fixed

- **OpenAI completion limits** — OpenAI Chat Completions requests now send the
  supported `max_completion_tokens` field, while other compatible providers
  retain `max_tokens` and Ollama-native requests retain `num_predict`.

## [PowerRustCOBOL 1.30.27] — 2026-07-18

### Fixed

- **Models Manager credential safety** — Empty API-key fields and stale Project
  Settings drafts can no longer remove saved credentials; only a confirmed
  profile Delete removes its profile key.
- **Models Manager draft isolation** — New and duplicated profiles remain local
  drafts until Save, so unrelated settings writes cannot persist incomplete
  records.
- **Models Manager model selection** — The model id is selected through one
  dropdown, defaults to the first fetched model when empty, preserves a saved
  selection, and is the authoritative model id used by Agents Manager calls.
- **Models Manager action layout** — Clear log now sits beside Test Connection,
  leaving the footer for Save and Close.
- **AI configuration recovery** — Global AI configuration writes now use a
  synced temporary file and last-known-good backup, with automatic backup
  recovery when the primary JSON is missing or malformed.

## [PowerRustCOBOL 1.30.26] — 2026-07-18

### Fixed

- **Models Manager profile preservation** — Saved model profiles now repair
  blank endpoints from their provider defaults and store API keys under stable
  profile ids, with the legacy provider/model key kept as a fallback.
- **Models Manager draft persistence** — New and duplicated profiles no longer
  write incomplete draft records to disk before Save is clicked.

## [PowerRustCOBOL 1.30.25] — 2026-07-18

### Fixed

- **OpenAI model picker noise** — Models Manager now filters OpenAI refresh
  results to likely chat/text models, hiding embeddings, image, audio,
  transcription, moderation, realtime, search, and Sora entries that cannot be
  used by the connection test.

## [PowerRustCOBOL 1.30.24] — 2026-07-18

### Fixed

- **Models Manager draft test keys** — Test Connection now uses the API key
  currently visible in the Models Manager draft instead of resolving a key only
  from the saved provider/model slot.

## [PowerRustCOBOL 1.30.23] — 2026-07-18

### Fixed

- **Models Manager draft editing** — Editing or testing a different provider in
  Models Manager no longer mutates the selected saved profile until Save is
  clicked, preventing a failed provider test from overwriting a working model.

## [PowerRustCOBOL 1.30.22] — 2026-07-18

### Fixed

- **Models Manager feedback clearing** — Added a Clear button to reset the
  latest test-connection and refresh-model feedback, fetched model list, modal
  error state, and detailed AI connection payload.

## [PowerRustCOBOL 1.30.21] — 2026-07-18

### Fixed

- **OpenAI model refresh** — Refreshing models for the OpenAI provider now calls
  the documented `/v1/models` endpoint when the form contains the API root or a
  chat endpoint.

## [PowerRustCOBOL 1.30.20] — 2026-07-18

### Fixed

- **Ollama Cloud chat endpoint normalization** — Saved list endpoints such as
  `https://ollama.com/api/tags` now start conversations through
  `https://ollama.com/api/chat` instead of appending `/api/chat` to the list URL.

## [PowerRustCOBOL 1.30.19] — 2026-07-18

### Fixed

- **Models Manager API key display** — Opening Models Manager now hydrates the
  selected profile's saved API key immediately, instead of only after switching
  profile rows.

## [PowerRustCOBOL 1.30.18] — 2026-07-18

### Fixed

- **Ollama Cloud endpoints** — Ollama Cloud now defaults conversations to
  `https://ollama.com/api/chat`, refreshes models through
  `https://ollama.com/api/tags`, and heals older `api.ollama.com` endpoints to
  `ollama.com` before chat requests.

## [PowerRustCOBOL 1.30.17] — 2026-07-18

### Fixed

- **Models Manager refresh URL** — Model refresh now calls exactly the endpoint
  URL typed in the Models Manager form for non-Ollama providers, without
  appending or rewriting path segments.

## [PowerRustCOBOL 1.30.16] — 2026-07-18

### Fixed

- **xAI model refresh URL** — Models Manager now derives xAI model-list URLs
  from the API root even when the saved endpoint points at `/v1/responses`, and
  no longer renders provider errors in the modal footer.

## [PowerRustCOBOL 1.30.15] — 2026-07-18

### Fixed

- **Models Manager refresh errors** — Model refresh failures no longer render
  inline beside the Refresh button; errors go to the alert dialog and IDE
  output pane instead.

## [PowerRustCOBOL 1.30.14] — 2026-07-18

### Fixed

- **Models Manager persistence and alerts** — Selecting a model no longer clears
  a freshly typed API key, Models Manager save/test/refresh events now write to
  the IDE output pane, and provider errors open the alert dialog with the full
  request/response payload.

## [PowerRustCOBOL 1.30.13] — 2026-07-18

### Fixed

- **Models Manager controls** — Added explicit Save, global Agentic AI enable,
  and Verbose AI log controls to the Models Manager.
- **xAI model discovery** — xAI model refresh now uses the documented
  language-model discovery endpoint and reports API status/body details when a
  provider rejects model-list requests.

## [PowerRustCOBOL 1.30.12] — 2026-07-18

### Fixed

- **Models Manager providers** — Removed the incorrect Groq provider entry from
  the shared AI provider list used by Models Manager and related settings UI.

## [PowerRustCOBOL 1.30.11] — 2026-07-18

### Fixed

- **Models Manager window chrome** — The Models Manager modal is now user
  resizable and its title no longer uses an emoji glyph that could render as a
  stray square in the window title bar.

## [PowerRustCOBOL 1.30.10] — 2026-07-18

### Fixed

- **Agents Manager wording** — Project Settings and related AI messages now use
  the grammatically correct "Agents Manager" label.

## [PowerRustCOBOL 1.30.9] — 2026-07-16

### Fixed

- **SVG asset preview zoom** — SVG assets now re-rasterize from vector data at
  the requested zoom size, avoiding enlarged bitmap artifacts, and image
  previews include a zoom slider for smoother control.

## [PowerRustCOBOL 1.30.8] — 2026-07-16

### Fixed

- **Transparent asset previews** — Image and animated asset previews now render
  on a light checkerboard contrast mat so transparent SVG/PNG/GIF assets remain
  visible in the dark IDE theme.

## [PowerRustCOBOL 1.30.7] — 2026-07-16

### Fixed

- **Asset metadata viewer padding** — The Assets preview metadata footer now
  reserves additional vertical room and bottom padding so the last metadata row
  remains visible instead of being clipped by the pane boundary.

## [PowerRustCOBOL 1.30.6] — 2026-07-16

### Fixed

- **Asset preview formats and tools** — The internal Assets viewer now supports
  SVG previews through the shared renderer path, enables AVIF decoding, adds
  image zoom controls, and can play supported animated image previews.
- **Asset viewer inspection** — The viewer now includes a bottom metadata table,
  Command/Ctrl+F search for text assets, and a standard offset/hex/ascii view
  for binary files.

## [PowerRustCOBOL 1.30.5] — 2026-07-15

### Fixed

- **Folder-backed Assets tree** — The Project Tree Assets node now reflects the
  project's on-disk `Assets/` folder, including nested subfolders and files,
  instead of only showing manifest entries.
- **Asset import, preview, and delete** — Importing an asset copies it into the
  project Assets folder, opens it in an internal read-only preview, and asset
  rows now offer deletion with confirmation. Text-like files render as text and
  image/animation formats supported by the decoder render as image previews.

## [PowerRustCOBOL 1.30.4] — 2026-07-15

### Fixed

- **Project-local agent files** — The Project Tree now exposes an Agentic AI
  branch with editable project-owned files for the Form Designer Agent and the
  COBOL Event Handler Script Agent, including each agent's system prompt,
  steering file, and skills.
- **Agent prompt routing** — The Form Designer Agent and COBOL Event Handler
  Script Agent now resolve their prompts/skills from the project-local agent
  folders so projects can tune those agents independently.

## [PowerRustCOBOL 1.30.3] — 2026-07-15

### Fixed

- **COBOL proficiency prompt settings** — Project Settings now edits the COBOL
  proficiency benchmark prompt instead of the generic assistant system prompt.
  The field is populated from the actual benchmark prompt, can be saved to the
  LLM configuration, and includes a Restore action to recover the built-in
  working prompt.

## [PowerRustCOBOL 1.30.2] — 2026-07-15

### Fixed

- **AI model selector stability** — The Project Settings model selector is back
  to a simple native dropdown without the inline search box, avoiding the
  empty custom picker panel seen with some egui layouts/themes.

## [PowerRustCOBOL 1.30.1] — 2026-07-15

### Fixed

- **AI model picker layout** — The Project Settings model selector now uses an
  in-form filtered list with a stable 250px scroll area instead of a floating
  combo popup, preventing the model list from collapsing or overlaying adjacent
  settings.

## [PowerRustCOBOL 1.30.0] — 2026-07-15

### Added

- **Resizable error windows** — Error messages (COBOL runtime errors, IDE
  alerts, and the Form Designer AI assistant) now open in a modal window that
  starts at 800×450 and resizes only when the user drags the grip. Each window
  offers Copy to clipboard, Save to a text file via a native dialog, an A−/A+
  font-size control, and an OK button.
- **AI COBOL proficiency benchmark** — New benchmark that scores the configured
  model on COBOL-85 / PowerRustCOBOL tasks and shows the results in a dashboard
  with per-scope metric bars, a radar chart, tested-points summary, and PDF
  export of the full report.
- **AI connection test** — Selecting a model can immediately run a minimal
  "reply with OK" round-trip to verify endpoint, key, and model access.
- **Output log font size** — The IDE log pane gained a −/+ font-size control
  (9–28 px).

### Fixed

- **Reasoning models rejected safely** — The agent orchestrator now counts
  streamed `reasoning` deltas, asks Ollama endpoints to disable thinking
  (`think: false`), and reports a clear error when a model returns hidden
  reasoning but no message content instead of applying an empty response.
- **Error windows growing on their own** — The error modals no longer size
  their content from remaining window space (the egui self-inflation feedback
  loop); the resize box is the single size authority, seeded at 800×450.
- **Project tree** — The first level of project folders expands automatically
  when a project opens.

## [PowerRustCOBOL 1.29.18] — 2026-07-15

### Fixed

- **AI model picker NaN crash** — The Project Settings model dropdown no longer
  passes infinite dimensions to egui while sizing the filter row, preventing
  `assertion failed: !max_rect.any_nan()` when opening the picker.

## [PowerRustCOBOL 1.29.17] — 2026-07-15

### Fixed

- **AI model picker spacing** — The Project Settings model dropdown now uses
  taller filter and model rows with extra vertical spacing, making model
  selection easier when many provider models are listed.

## [PowerRustCOBOL 1.29.16] — 2026-07-15

### Fixed

- **Retired AI model guard** — The IDE now clears and blocks the retired
  `qwen3-coder-next` model from saved AI settings, outbound requests, test
  requests, and refreshed model lists, avoiding Ollama Cloud HTTP 410 failures.

## [PowerRustCOBOL 1.29.15] — 2026-07-15

### Fixed

- **SVG button icon rendering** — Button SVG icons are now rendered from vector
  data at their final on-screen icon rectangle size, avoiding artifacts from
  scaling a cached raster texture.

## [PowerRustCOBOL 1.29.14] — 2026-07-15

### Fixed

- **Button top/bottom icon text limits** — Button text now aligns within the
  vertical space left after a top or bottom icon is placed, so captions no
  longer overlap icons and zero icon padding makes the text touch the icon edge
  exactly.

## [PowerRustCOBOL 1.29.13] — 2026-07-15

### Fixed

- **Button right-icon spacing** — Button captions are now measured by their
  actual glyph width before laying out left/right icons, so `IconAlignment=Right`
  with zero padding places the icon immediately after the last text character
  instead of after hidden paragraph space.

## [PowerRustCOBOL 1.29.12] — 2026-07-15

### Fixed

- **Button icon/text block layout** — Left/right Button icons now stay
  immediately before or after the text as a single aligned block, shrinking the
  icon when needed so it never overlaps the caption or spills past the button.
  Top/bottom icon alignment remains independent from text alignment.

## [PowerRustCOBOL 1.29.11] — 2026-07-15

### Fixed

- **Button icon properties** — Renamed Button `ImagePath`, `ImageAlignment`,
  and `ImagePadding` to `IconPath`, `IconAlignment`, and `IconPadding`, added
  `IconSize` with fixed dropdown sizes from 16px to 128px, and kept legacy
  image-property fallbacks for existing forms.

## [PowerRustCOBOL 1.29.10] — 2026-07-15

### Fixed

- **Button text/image alignment rules** — Button `TextAlignment` now places text
  in the requested 3x3 grid position. Top/bottom images are centered to the
  button edge independently of text alignment, while left/right images stay next
  to the text with the configured padding.

## [PowerRustCOBOL 1.29.9] — 2026-07-15

### Fixed

- **Button border and hover behavior** — Buttons now expose `BorderWidth`, apply
  their configured border color/thickness, render `Fixed3D`, `Raised`, and
  `Sunken` border styles as bevels, and honor `Tooltip` plus `Cursor` while
  hovering in interactive forms.

## [PowerRustCOBOL 1.29.8] — 2026-07-15

### Fixed

- **Button text and image layout** — Button `TextAlignment` now controls caption
  placement, `ImagePath` renders an image beside the caption, and
  `ImageAlignment` supports `Left`, `Right`, `Top`, and `Bottom` with a new
  `ImagePadding` property defaulting to 10px.

## [PowerRustCOBOL 1.29.7] — 2026-07-15

### Fixed

- **Button property cleanup** — Removed unused or duplicate Button inspector
  properties: `FlatStyle`, type-specific `CornerRadius`, `IsCancel`, and
  `ModalResult`. Button corner radius remains available in the Geometry section.

## [PowerRustCOBOL 1.29.6] — 2026-07-15

### Fixed

- **Default button Enter key** — Pressing Enter in preview/run/compiled forms
  now triggers the explicit `IsDefault` button when no input control has focus,
  and is ignored when the form has no default button.

## [PowerRustCOBOL 1.29.5] — 2026-07-15

### Fixed

- **Runtime tab order traversal** — Pressing Tab or Shift+Tab in preview/run/
  compiled forms now moves keyboard focus through enabled visible controls using
  the form's `TabOrder` values instead of leaving focus stuck.

## [PowerRustCOBOL 1.29.4] — 2026-07-15

### Fixed

- **Control font styles** — Bold, Italic, Underline, and Strikethrough now render
  through the shared text painter for text-bearing controls instead of only
  applying to Labels.

## [PowerRustCOBOL 1.29.3] — 2026-07-15

### Fixed

- **Agent control invocation prompts** — All embedded agent prompts now instruct
  COBOL generation to use COBOL-2002-style inline control members
  (`<control>::<method>(...)` and `<control>::<property>`) instead of `CALL` or
  legacy `INVOKE Control "Method" USING ...` forms for control methods,
  property access, chart actions, data bindings, REST/SQL controls, and
  IndexedFile controls.

## [PowerRustCOBOL 1.29.2] — 2026-07-15

### Fixed

- **Neumorphic control defaults** — Applying the Neumorphic style now forces
  black foreground text for buttons, tab headers, and data-input controls, and
  applies the expected 15px corner radius so controls remain readable and match
  the soft-UI style.
- **IDE OS app label** — The native IDE app/process name now uses
  `PowerRustCOBOL <version>` so OS dock/taskbar hover labels no longer fall back
  to the internal binary name `cobolt-ide`.

## [PowerRustCOBOL 1.29.1] — 2026-07-14

### Fixed

- **Form Designer Copy Style latch** — Clicking the Copy Style toolbar icon now
  keeps the format painter active so the captured style can be applied to one
  or more controls until the icon is clicked again or Escape is pressed.

## [PowerRustCOBOL 1.29.0] — 2026-07-14

### Added

- **IndexedFile non-visual control** — Introduced a non-visual Form Designer
  control for project-registered indexed files, with INPUT/I-O mode,
  Disk/Memory load strategy, auto-open lifecycle support, project `.cidx`
  selection, and generated COBOL paragraphs for open, start, navigation reads,
  write/rewrite/delete, commit, rollback, and close.
- **Form Designer Agent desktop-design skill** — Added a dedicated desktop form
  design skill for the agent, covering typography, readable foreground colors,
  15px corner radius, Neumorphic defaults, label/input alignment,
  tab/container ownership, preservation rules, resize behavior, accessibility,
  and atomic operation output.

### Fixed

- **Agent indexed-file CRUD awareness** — The agent now knows to use
  `IndexedFile` controls, real properties, generated method paragraphs, and
  project-tree `.cidx` discovery when creating CRUD, browse, search, navigation,
  and grid forms over indexed files.
- **Form Designer AI project discovery and routing** — The agent receives a
  project-tree inventory, routes CRUD/navigation event wiring to EventBinder,
  and avoids producing raw COBOL that is not attached to control events.
- **Windows IDE link error** — The optional local embedding/retrieval stack is no
  longer linked by default, avoiding the MSVC runtime-library mismatch from the
  ONNX/tokenizers dependency chain.
- **Designer usability polish** — Added generated-COBOL delete actions, improved
  form delete affordances, stabilized the AI pane resize line, and documented the
  egui nested-panel resizing guardrail.

## [PowerRustCOBOL 1.28.43] — 2026-07-14

### Fixed

- **Form Designer Agent desktop-design skill** — Added the
  `rustcobol-desktop-form-design` skill to the agent skill set and embedded IDE
  assets, and anchored the FormsDesigner specialist to its desktop form design
  rules for typography, colors, corner radius, alignment, themes, tab/container
  ownership, preservation, validation, resizing, accessibility, and atomic
  operation output.

## [PowerRustCOBOL 1.28.42] — 2026-07-14

### Fixed

- **Agent IndexedFile awareness** — Updated the agent prompts and
  RustCOBOL control-property skill so CRUD, browse, search, and grid requests
  over indexed files prefer the non-visual `IndexedFile` control, its real
  properties, and its generated method paragraphs instead of hand-rolled
  indexed-file boilerplate.

## [PowerRustCOBOL 1.28.41] — 2026-07-14

### Added

- **IndexedFile non-visual control** — Added an IndexedFile designer control
  with project-registered indexed-file selection, INPUT/I-O open mode,
  Disk/Memory load strategy, auto-open on form load, generated close on form
  shutdown, and generated COBOL method paragraphs for open, start, reads,
  write/rewrite/delete, commit, rollback, and close.

## [PowerRustCOBOL 1.28.40] — 2026-07-14

### Fixed

- **Form Designer AI event wiring** — Requests to wire/connect CRUD and
  navigation buttons for indexed files now route to the Event Binder specialist
  instead of the raw Code Generator, so generated handler code is returned as
  `generate_event_handler` operations and creates the control `onClick`
  bindings on apply.

## [PowerRustCOBOL 1.28.39] — 2026-07-14

### Fixed

- **Windows IDE link error** — The default `cobolt-agents` build no longer links
  the local ONNX/tokenizers retrieval stack used for optional local embeddings.
  This avoids the MSVC runtime-library mismatch between `ort_sys` and
  `esaxx-rs` when building `cobolt-ide` on Windows, while keeping the retrieval
  modules available behind the `local-retrieval` feature.

## [PowerRustCOBOL 1.28.38] — 2026-07-14

### Fixed

- **Generated COBOL delete button** — The Project pane's Generated Code tree
  now shows a trashcan-only delete button for generated `.cbl` files, with a
  confirmation dialog that removes the file from the project, closes an open
  editor tab for it, and deletes the `.cbl` from disk.

## [PowerRustCOBOL 1.28.37] — 2026-07-14

### Fixed

- **Form Designer AI project discovery** — The Form Designer assistant and the
  IDE agent bar now receive a project-tree inventory in their request context,
  including forms, common/generated COBOL, assets, documentation, project user
  controls, and indexed-file definitions with record/key/field summaries when
  the `.cidx` can be read.

## [PowerRustCOBOL 1.28.36] — 2026-07-14

### Fixed

- **AI assistant pane resize affordance** — The Form Designer AI assistant pane
  now draws one stable 3px white resize line while making egui's hover/active
  separator stroke transparent, removing the flickering ghost line above the
  pane boundary. The internal prompt separator remains 1px.

## [PowerRustCOBOL 1.28.31] — 2026-07-14

### Fixed

- **Project tree form delete button** — The trashcan-only form delete button now
  appears next to form names in the Project pane's Forms tree as well as in the
  Form Designer's Forms pane, using the same confirmation flow.

## [PowerRustCOBOL 1.28.30] — 2026-07-14

### Added

- **Delete form from the Forms list** — The Form Designer's Forms pane now shows
  a trashcan-only button before each form name. Clicking it asks for
  confirmation, then removes the `.cfrm` from disk, removes it from the project,
  closes any open designer for that form, and refreshes the list.

## [PowerRustCOBOL 1.28.29] — 2026-07-14

### Fixed

- **AI assistant pane resize guardrail** — Identified
  `egui::TopBottomPanel::show_inside(...)` / `SidePanel::show_inside(...)` as
  the nested-panel pattern that can make resizable panes snap back to their
  minimum size, and added explicit project/agent prompt rules forbidding it for
  user-resizable panes.

## [PowerRustCOBOL 1.28.28] — 2026-07-14

### Fixed

- **Control defaults and New Form theme selection** — New controls now default
  to 14px fonts, data-entry controls default to black foreground text, and the
  New Form dialog includes a procedural Theme dropdown. Choosing `Neumorphic`
  applies the requested soft-shadow defaults and the `E1E6F8FF` form/control
  surface color.

## [PowerRustCOBOL 1.28.27] — 2026-07-14

### Added

- **Copy full IDE log** — The Log/Output pane now has a `Copy log` button that
  copies every line currently in the log to the OS clipboard.

## [PowerRustCOBOL 1.28.26] — 2026-07-14

### Fixed

- **Form COBOL IDs must be unique** — The IDE now blocks duplicate form
  `name`/COBOL IDs when creating a form, saving a renamed/edited form, or
  importing an existing `.cfrm` into a project. The check is case-insensitive and
  ignores the form's own file while saving.

## [PowerRustCOBOL 1.28.25] — 2026-07-14

### Fixed

- **TabControl selected-tab color and page layout** — Existing TabControls now
  show the `ActiveTabColor` picker even when opened from older `.cfrm` files,
  and non-top tab navbars now reserve page-frame space instead of overlaying the
  TabControl content.

## [PowerRustCOBOL 1.28.24] — 2026-07-14

### Fixed

- **TabControl selection and navbar behavior** — Added an `ActiveTabColor`
  property, made preview/run/binary tab selection update `SelectedTab`, and made
  TabControl drawing, hit testing, and child clipping obey `TabPosition`
  (`Top`, `Bottom`, `Left`, `Right`) consistently across surfaces.

## [PowerRustCOBOL 1.28.23] — 2026-07-14

### Fixed

- **AI agent routing now covers every supported UI language** — form/control
  requests and event-binding requests are recognized across English, Spanish,
  Portuguese, Japanese, Chinese, and French. Event intent now takes precedence
  over control nouns, so “bind the click event to my button” routes to the
  Event Binder instead of the Forms Designer.

## [PowerRustCOBOL 1.28.22] — 2026-07-13

### Fixed

- **AI agent can place controls inside TabControl pages** — Spanish/Portuguese
  form-edit requests such as “Añade un botón a la Tab1 del TabControl-1” now
  route to the Forms Designer specialist instead of the COBOL Code Generator.
  Agent `deploy_control` operations now accept structural `Parent`/`Tab`
  placement fields and resolve tab-page names like `Tab1` to the owning
  `TabControl` plus the correct tab index.

## [PowerRustCOBOL 1.28.21] — 2026-07-13

### Added

- **Common Code COBOL Editor Buttons** — Added Save and Cancel buttons at the bottom of the editor panel for common code procedures.
- **AI Agent `message` Operation** — Added a dedicated `message` operation to the agent protocol to support clean conversational responses and help messages without corrupting JSON payloads.

### Changed

- **AI Assistant UX & Theme Consistency** — Styled AI assistant chat bubbles with a 15px corner radius, white foreground color, and distinct blue (AI) and green (User) backgrounds.
- **AI Assistant Height Constraint** — Replaced auto-resizing behavior with a fixed panel height in egui, preventing the AI pane from expanding to fit growing history.
- **Absolute Coordinates in Forms** — Enforced absolute positioning for child controls nested within `TabControl` or `Panel` components. Updated the layout designer skill and system prompts to make the agent aware of absolute coordinates.
- **Architecture & System Prompts** — Documented the multi-agent mesh architecture with detailed diagrams, restructured agent skills (types, extensions, layout, properties, concise), and updated prompts.

## [PowerRustCOBOL 1.28.20] — 2026-07-12

### Fixed

- **Ollama Cloud calls work** — the provider pointed at the wrong host
  (`api.ollama.com`); the service lives at `ollama.com` (`/api` native,
  `/v1` OpenAI-compatible). The default endpoint is corrected and every
  outbound path (requests, connection test, model listing) heals endpoints
  saved with the old host, so existing configs work without re-selecting the
  provider. Verified live: HTTP 200 with Bearer auth against
  `https://ollama.com/v1/chat/completions`.
- **Local Ollama native responses parse** — the orchestrator only understood
  the OpenAI reply shape (`choices[0].message.content`); it now also parses
  Ollama-native (`message.content`), and picks the right wire format from the
  endpoint suffix (`/api` → native, `/v1` → OpenAI-compatible).
- **Agent requests carry the real prompt again** — the mesh rewrite dropped
  the composed system prompt, skills, and per-request context on the floor
  (parameters received but discarded), running every request on a canned
  one-liner. The full spec-025 composition (system prompt → skills → history →
  prompt+context) is restored for the dev agent, the editor assistant, and
  history compaction.
- **Agentic AI log is real and live** — the orchestrator now streams every
  step through a log callback into the AI activity pane: specialist routing,
  resolved URL and wire format, HTTP status with duration and payload size,
  and token usage (both OpenAI and Ollama accounting). The connection log
  always records a compact request/response trace (full bodies with the
  verbose setting), so a "successful" call can no longer be invisible —
  or faked.

## [PowerRustCOBOL 1.28.19] — 2026-07-12

### Fixed

- **Editor IntelliSense no longer panics on Unicode separators** — completion
  prefix detection now advances by UTF-8 character boundaries instead of raw
  bytes, fixing crashes such as typing after `a˜`.

## [PowerRustCOBOL 1.28.18] — 2026-07-12

### Fixed

- **Generated COBOL panes no longer show the AI assistant** — read-only
  generated source tabs now hide the editor assistant bar, since the assistant
  cannot safely modify generated code.

## [PowerRustCOBOL 1.28.17] — 2026-07-12

### Fixed

- **Debugger variables show control properties again** — removed the over-broad
  numeric `WS-*` filter so generated control-property data items return to the
  Variables list while explicit runtime/event support fields remain hidden.

## [PowerRustCOBOL 1.28.16] — 2026-07-12

### Fixed

- **Debugger data inspection now follows live values** — the data-item detail
  window refreshes from each new paused variable snapshot instead of keeping the
  value captured when the item was first clicked.
- **Debugger variables hide more generated control plumbing** — numeric
  control-handler support fields such as `WS-SLIDER-2` are filtered out of the
  Variables list along with the existing runtime event fields.
- **Run Form windows now have an OS icon** — project settings include a project
  icon image picker, and `rcrun run-form` applies that icon to the native
  Dock/Taskbar window with the bundled PowerRustCOBOL icon as fallback.

## [PowerRustCOBOL 1.28.15] — 2026-07-12

### Fixed

- **Debugger code tracking is denser and easier to follow** — blank generated
  COBOL lines now render as 3 px spacers instead of full-height rows, Step In
  is available from the toolbar and F11, and the new Animate control advances
  paused execution at 1–10 lines per second while keeping the active line
  vertically centered.
- **Debugger variables focus on user data** — generated control-handler support
  fields are hidden from the Variables list, and value previews now use the
  actual Value column width before adding a visual-only ellipsis.

## [PowerRustCOBOL 1.28.14] — 2026-07-12

### Fixed

- **Debugger is a standalone always-on-top OS window** — the debugger no longer
  renders inside the designer viewport (where watching the running form meant
  switching back to the RAD window). It now opens as its own OS window, always
  on top — like the Run-Form Inspector — so the running form, the debugger, and
  the designer can be arranged side by side. The OS window is the sole size
  authority: it opens at 900×520 once per session and afterwards only the
  user's own resizes apply (no self-inflation path). Closing the debugger
  window stops the debug session. F5/F10 shortcuts, the pane split, variable
  details, and both session kinds (external rcrun / in-IDE editor) behave as
  before.

## [PowerRustCOBOL 1.28.13] — 2026-07-12

### Fixed

- **Debug Form now runs the live application window** — debugging from the RAD
  toolbar previously ran only the interpreter, so there was no form to interact
  with. Debug Form now launches the same standalone `rcrun run-form` process as
  Run Form, with a new `--debug` flag: the form window is fully interactive
  while the IDE debugger controls the interpreter **across the process
  boundary**. Commands (Continue/Step Over/Pause, breakpoint set) travel as
  `@DBG <json>` lines on the child's stdin; Paused/Resumed/Finished events (with
  full variable snapshots) return as `@DBG <json>` lines on stdout — plain lines
  remain DISPLAY output. The program starts paused at line 1; Stop kills the
  process (closing the form); the session ends when the form window closes.
  This line-based JSON protocol is transport-agnostic by design, so the same
  debugger can later drive Android/iOS debuggees over adb/ssh.

## [PowerRustCOBOL 1.28.12] — 2026-07-12

### Fixed

- **Debugger variables stay compact and inspectable** — Variable values in the
  grid are capped to 90 pixels with `...`, and clicking a data-item name opens
  a resizable detail window with PIC, Scope, Origin, wrapped value text, and a
  hexadecimal representation pane.

## [PowerRustCOBOL 1.28.11] — 2026-07-12

### Fixed

- **Debugger panes are user-resizable** — The generated-code pane and debugger
  data pane now use a draggable split, so users can give more space to source,
  variables, call stack, or breakpoints as needed.
- **Debugger title describes the generated form code** — The window title now
  reads `Debugging <form name> generated code`.
- **Variables view shows name, scope, and value** — Variable snapshots now carry
  DATA DIVISION origin metadata so the debugger can show `Global`/`Local` plus
  `WS`, `FD`, or `LS` in a three-column, filterable table. The parser now also
  preserves `FD ... GLOBAL` so FD record variables can be labelled `Global (FD)`.

## [PowerRustCOBOL 1.28.10] — 2026-07-12

### Fixed

- **Run-Form Inspector chart grouping is clearer** — Added a left-aligned
  `IDE stats` title above the first row of charts, matching the section title
  style used by `Per-form CPU (rcrun)`.

## [PowerRustCOBOL 1.28.9] — 2026-07-12

### Fixed

- **RAD Debug Form no longer prints temporary debug breadcrumbs** — Removed the
  leftover `[DBG]` Output-pane messages and internal trace calls from the
  form-debug launch path while preserving the debugger window placement in the
  owning designer viewport.

## [PowerRustCOBOL 1.28.8] — 2026-07-12

### Fixed

- **Run-form events appeared not to fire** — COBOL upper-cases control ids, so a
  handler's property writes (`LABEL-1::Caption`) arrived under `"LABEL-1"` while
  the renderer reads the designer-case `"Label-1"` state entry: handlers ran but
  their effects never appeared on screen. `rcrun run-form` now resolves incoming
  control ids case-insensitively (and routes repeating-group member writes to
  the drawn card instance), exactly like the IDE's form runtime. The compiled-
  binary template had the same latent bug and got the same fix.
- **Run-form event dispatch parity** — instanced repeating-group member events
  (`group.group-N.member`) dispatch to the designed member id with the correct
  `CONTROL-ARRAY-INDEX`, and timer `onTick` events are coalesced against a
  queued backlog (WinForms semantics) as in the IDE runtime.
- **Live DISPLAY output from run-form** — the runner's stdout is block-buffered
  when piped to the IDE, so DISPLAY lines could sit unseen in the buffer; the
  runner now flushes after each drained batch, making DISPLAY output stream
  into the Output pane live.

## [PowerRustCOBOL 1.28.7] — 2026-07-12

### Fixed

- **Debugger window opens in front of the form designer** — A debug session
  started from the RAD toolbar now renders its debugger window inside that
  form's designer viewport (in front of the canvas) instead of in the main IDE
  window, where it opened hidden behind the designer. Editor-started sessions
  keep rendering in the main window. The session tracks its owning surface and
  resets it when debugging ends.
- **Debugger window no longer grows by itself** — the recurring egui
  self-inflation loop hit the debugger window too: its split body sized panes
  from the measured available space, which fed back into the window size every
  frame. The window now auto-sizes to an inner `egui::Resize` box that opens at
  860×460 and changes only when the user drags the resize grip; the code/data
  panes are carved as fixed rects in cursor-detached child UIs, so content size
  structurally cannot flow back into window size.
- **Run-form window theme parity** — the standalone `rcrun run-form` window (and
  binaries produced by `rcrun build`) rendered with egui's default dark visuals,
  leaking dark widget fills into forms designed on the light canvas. Both now
  set light visuals, apply the form's saved glass style, and resolve the form's
  theme pack (per-form override → project default → Liquid Glass) exactly like
  the designer canvas; the IDE forwards the project default via
  `--theme-default`.

## [PowerRustCOBOL 1.28.6] — 2026-07-12

### Fixed

- **Run Form now runs as a standalone process** — The Run Form button spawns the
  new `rcrun run-form <form.cfrm> <program.cbl>` command instead of rendering the
  form inside the IDE. The form gets its own window, event loop, and interpreter —
  exactly what an `rcrun build` binary ships — while the IDE stays idle (a slow
  0.2 s heartbeat only to pipe the form's DISPLAY output into the Output pane and
  notice process exit). Measured: the standalone form idles at ~3 % CPU where the
  in-IDE path previously kept the whole IDE render loop hot. Stop kills the
  process; syntax/semantic errors are still pre-checked in the IDE with the modal
  and red tree semaphore; a failed process exit surfaces its stderr in a modal.
  The Run-Form Inspector samples the child process via the PID tree as before.
  Trade-offs of the process boundary: the designer's live glass/theme toggling no
  longer affects a running form (it reads the saved form, like production), and
  "Apply layout to design" is not available from an external run.
- **Compiled binaries no longer spin at max FPS** — the form event loop generated
  by `rcrun build` ended every frame with an unconditional repaint request,
  pegging a core even while the form sat idle. It now uses the same reactive
  scheduling as the IDE and `rcrun run-form`: fast polling only while interpreter
  traffic flows, a 0.2 s heartbeat otherwise, with Timer controls scheduling
  their own precise wake-ups.

## [PowerRustCOBOL 1.28.5] — 2026-07-12

### Fixed

- **IDE frame-cost instrumentation for Run Form** — The Run-Form Inspector header
  now shows the IDE's own render load live: repaints per second and average/max
  milliseconds spent per frame. While a form runs, a `[PERF]` line with the same
  numbers (plus busy-frame count and open-window counts) is appended once per
  second to `/tmp/cobolt-debug.log`. This tells apart the two possible causes of
  high IDE CPU during Run Form: too many repaints vs. too-expensive frames.
- **Window title set only on change** — the build-mode window title was re-sent
  to the OS on every frame; it is now only sent when the text actually changes.

## [PowerRustCOBOL 1.28.4] — 2026-07-12

### Fixed

- **Debugger froze the whole IDE (infinite loop in syntax highlighter)** — The
  debugger's COBOL syntax highlighter (`build_cobol_layout_job`) entered an
  infinite zero-progress loop whenever a source line contained `-` immediately
  followed by an alphanumeric character outside an identifier — e.g. the negative
  literal in `PERFORM … BY -1`. The tokenizer's fallback branch broke out without
  consuming the character, so the render loop appended empty text sections forever,
  pinning the UI thread and beach-balling the IDE the moment the debugger window
  drew that line. Root cause confirmed with `sample(1)` stack captures of the hung
  process. Negative numeric literals are now tokenized (and coloured) correctly,
  and the fallback branch guarantees forward progress. Regression tests added.
- **Debug instrumentation** — Debug sessions now log each pipeline stage
  (tokenize, parse, semantic, interpreter) with millisecond timestamps to
  `/tmp/cobolt-debug.log` and echo `[DBG]` progress lines to the Output pane.

## [PowerRustCOBOL 1.28.3] — 2026-07-11

### Fixed

- **RAD debug hang** — Clicking the Debug button in the form-designer toolbar no
  longer freezes the IDE. Three root causes were fixed: (1) `DebugRunner::stop()`
  now detaches the interpreter thread instead of joining it, so stopping a session
  never blocks the UI thread; (2) the Debug button is now disabled while a debug
  session is active, preventing a re-click that would have triggered the blocking
  join; (3) the redundant `do_generate_cobol` call inside `do_debug_form` was
  removed — `do_save_designer` already writes the `.cbl` file via `after_form_saved`,
  and the duplicate call also set `pending_open_in_editor`, which stole editor focus
  away from the debug panel.

## [PowerRustCOBOL 1.28.2] — 2026-07-11

### Fixed

- **Window title build-mode indicator** — When a project is open the window
  title now appends " — Debug Mode" (or the localised equivalent) when
  *Debug Compilation* is enabled in project settings, and " — Release Mode"
  when it is disabled. The suffix is absent when no project is loaded.
  All six interface languages (EN, ES, PT, JP, ZH, FR) are covered.

## [PowerRustCOBOL 1.28.1] — 2026-07-11

### Fixed

- **Debugger: format detection** — `detect_format` in the IDE's debug/run
  pipeline now delegates to the canonical `SourceFormat::detect`, which
  correctly identifies generated COBOL as free-format when the first `*>`
  comment uses six-space indentation. The previous heuristic treated those
  lines as fixed-format column-7 indicators, causing `flatten_fixed` to
  truncate lines at column 72. This silently stripped the `-1` from
  `BY -1` (error: *expected expression, found Until*) and the closing `)`
  from multi-level arithmetic expressions (error: *expected RParen, found
  EndIf / EndPerform*), aborting every debug session that used
  `PERFORM VARYING … BY -1` or a COMPUTE with nested parentheses past column 72.

## [PowerRustCOBOL 1.28.0] — 2026-07-11

### Added

- **Form debugger (work in progress)** — The RAD (form designer) toolbar now
  has a Debug button (breakpoint-ring icon) in the Run group, between Run and
  Inspector. Clicking it saves the form, regenerates the COBOL event-handler
  source, and starts a debug session for that generated `.cbl` file. The
  session opens in the existing resizable floating debugger window
  (Continue F5, Step Over F10, Pause, Stop) with syntax-highlighted source,
  gutter breakpoint dots, amber current-line arrow, and auto-scroll to the
  paused line. Variables, Call Stack, and Breakpoints tabs are available. The
  Debug button stays highlighted while a session is active and is disabled
  while the form is running. Full breakpoint-setting workflow and multi-form
  session management are still in progress.

## [PowerRustCOBOL 1.27.157] — 2026-07-11

### Fixed

- **Debugger floating window** — Debug controls (Continue F5, Step Over F10,
  Pause, Stop) and all session state are now shown in a resizable floating window
  instead of a side panel and secondary toolbar. The window contains a
  syntax-highlighted COBOL source viewer with line numbers, a red breakpoint dot
  (●) in the gutter, an amber current-line arrow (►), and auto-scroll to the
  paused line. Three tabs cover Variables (filterable), Call Stack, and
  Breakpoints. The IDE toolbar retains only the Debug launch button; the
  secondary toolbar that appeared above the editor during a session is removed.

## [PowerRustCOBOL 1.27.156] — 2026-07-11

### Fixed

- **Vertical slider drag direction** — Vertical sliders now map upward knob drags
  to increasing values, matching their painted scale, so the knob follows the
  pointer instead of moving in the opposite direction.

## [PowerRustCOBOL 1.27.155] — 2026-07-11

### Fixed

- **AI prompt IntelliSense filtering** — AI assistant prompt editors now use a
  context-only completion mode: control names, control properties, FD records,
  and local/global data items are offered, while COBOL reserved words and
  paragraph labels are suppressed.

## [PowerRustCOBOL 1.27.154] — 2026-07-11

### Fixed

- **User chat bubble colors** — Developer/user chat balloons now use background
  `#3dcd8bff` and foreground `#555753ff`.

## [PowerRustCOBOL 1.27.153] — 2026-07-11

### Fixed

- **Event editor debug ID overlay** — Embedded COBOL editors now use distinct
  egui widget ID namespaces, so the handler editor and AI prompt editor no
  longer trigger duplicate-ID debug labels such as “First/Second use of widget
  ID”.

## [PowerRustCOBOL 1.27.152] — 2026-07-11

### Fixed

- **COBOL response bubble colors** — Assistant/COBOL response chat balloons now
  use background `#3d8bcdff` and foreground `#e6e6e6ff` for better contrast.

## [PowerRustCOBOL 1.27.151] — 2026-07-11

### Fixed

- **Grok Responses payload compatibility** — Legacy saved Grok chat endpoint URLs
  are now normalized to `https://api.x.ai/v1/responses`, and Responses endpoint
  detection ignores query/fragment suffixes so requests never send unsupported
  `max_tokens` to xAI's Responses API.

## [PowerRustCOBOL 1.27.150] — 2026-07-11

### Fixed

- **Grok endpoint uses xAI Responses API** — Selecting Grok now fills
  `https://api.x.ai/v1/responses`, and the AI transport sends/parses Responses
  API payloads while keeping existing chat-completions providers unchanged.

## [PowerRustCOBOL 1.27.149] — 2026-07-11

### Fixed

- **Event AI prompt IntelliSense** — The Event Handler assistant prompt now uses
  the shared COBOL editor engine instead of a plain multiline text box, so typing
  `Control::` in the chat prompt offers the same live control property/method
  completions as the handler editor.

## [PowerRustCOBOL 1.27.148] — 2026-07-11

### Fixed

- **Animation badge is designer-only** — Controls with animations still show the
  yellow play badge on the RAD Designer canvas, but the badge is no longer drawn
  by the shared control painter. Preview, Run Form, and compiled binaries now
  render animated controls without editor-only chrome.

## [PowerRustCOBOL 1.27.147] — 2026-07-11

### Fixed

- **AI assistant prompt editor IntelliSense** — Settings → AI now edits the
  assistant system prompt through the shared COBOL editor surface instead of a
  plain multiline field, so prompt examples get the same RustCOBOL keyword and
  control-member IntelliSense used by the Event Handler editor. When a form is
  available, the editor receives that form's live control/property/method context
  to help prompt authors avoid hallucinated property names.

## [PowerRustCOBOL 1.27.146] — 2026-07-11

### Fixed

- **Agentic AI COBOL/property guardrails** — The dev agent and Event Editor
  chatbot now receive stronger control/property context, including per-control API
  lists and an intent map (`dropshadow` / shadow on → `ShadowEnabled`, depth /
  relief → `ShadowBlurStrength`, etc.). Agent-generated handlers/procedures are
  also rejected unless they keep the required nested-body divisions
  (`ENVIRONMENT`, `DATA`, `PROCEDURE`), so a model cannot silently replace a
  valid handler with a partial fragment; the bounded repair loop asks for a full
  corrected body or a clarification instead.

## [PowerRustCOBOL 1.27.145] — 2026-07-11

### Fixed

- **Event-editor AI assistant — real conversation + safety** — The COBOL Event
  Editor's assistant now keeps an independent, persisted conversation per handler,
  shown as a chat transcript with **Save / Compact / Clear** controls (Clear
  confirms first; Compact summarises on a worker thread) and auto-scroll to the
  newest turn. Before an AI reply replaces the handler it is **parsed and checked**
  (syntax + control-member references + a full-form semantic pass); if it's broken
  the parser errors are looped back to the model for up to three automatic fixes
  instead of writing invalid code. Questions/answers with no code block are never
  applied — they surface in the activity log.
- **Live chart data (all chart types)** — `COBOL-CHART-ADD-POINT` /
  `-CLEAR` / `-SET-TABLE` / `-REFRESH` now actually feed the renderer. Line, Area,
  Bar, Pie, Donut and Scatter charts plot the pushed values (auto-scaled), falling
  back to the representative sample only when no data has been set.
- **AI assistant in the COBOL Structure editor** — The shared assistant bar is now
  available in the Structure editor (WORKING-STORAGE, FILE SECTION, procedures, …),
  with its own per-block conversation history.
- **Chat-style transcript** — Assistant conversations render as balloons: developer
  turns in light green on the right, assistant turns in light gray on the left,
  dark text, monospace.
- **AI activity log** — A model question/answer (a reply with no code block) is
  shown prominently (2× font) and never written into the editor. Fenced COBOL inside
  the model's reasoning is hidden unless *Verbose AI log* is on. Assistant and
  dev-agent prompts updated: answer questions in prose, only emit code for an actual
  change.
- **Settings → AI** — The Detect / Test / Details / Timeout row moved directly above
  the system prompt. Selecting a model tests the connection immediately, and a failed
  test now opens an error modal (with a Details button) instead of only a status line.
  The Model picker gained a **type-to-filter** box and a taller popup so at least six
  models are visible at once.
- **New providers: OpenRouter & Hugging Face** — Both OpenAI-compatible aggregators
  are selectable in the provider picker; Refresh lists their catalogues (OpenRouter's
  is public, so it works before a key is entered), giving access to the free coding
  models (Qwen coders, DeepSeek, Mistral Nemo, StarCoder2, …).
- **Vertical slider glass frame** — A vertical Slider no longer paints a large stray
  white→transparent gradient. The glass-pill helper now rounds on the short axis and
  skips the top sheen when it would collapse on a tall, narrow track.

## [PowerRustCOBOL 1.27.144] — 2026-07-10

### Fixed

- **Conversation history controls in the AI assistant** — The editor/event-editor AI
  bar now exposes three explicit per-handler controls. **💾 Save** force-persists the
  selected handler's conversation to the project's indexed store (it is also
  auto-restored when the handler is reopened). **🗜 Compact** summarises the
  conversation on a worker thread into one concise, structured summary (user intent,
  decisions, constraints, assumptions, code changes applied, pending tasks,
  behavioural requirements) and replaces the history with it, so the thread keeps its
  meaning without growing unbounded. **🗑 Clear** now asks for confirmation before
  deleting and then starts a fresh conversation. Each event handler keeps its own
  independent history, so saving, compacting, or clearing one never affects another.
- **Hallucinated control member caught at save time** — Saving an event handler now
  validates every `Control::property` / `Control::method(...)` reference against the
  control's real member set (the same registries IntelliSense and the dev-agent gate
  use), in addition to syntax. An invented member (e.g. `TextBox-2::Depth`) raises the
  error modal — with a plain-English hint pointing at the real property — instead of
  saving silently. User/Custom controls are exempt.
- **Comments normalised to `*>`** — Deployed handler/procedure code is now run through
  a deterministic `*` → `*>` comment normaliser, and the dev-agent prompt plus the
  `rustcobol-concise` / `rustcobol-extensions` skills spell out the rule (always `*>`
  and a space, indented to the code it describes, wrapped at column 80), so the agent
  stops emitting bare fixed-format `*` comments.
- **Agent skill: control properties** — New `agentic_ai/skills/rustcobol-control-properties.md`
  teaching the agent/assistant to use only properties that exist on a control and to
  map natural-language intent (any language) to the real name — e.g. a control's
  "depth" under the Neumorphic style is `ShadowBlurStrength`, there is no `Depth`
  property. Scaffolded non-destructively and always injected into context.

## [PowerRustCOBOL 1.27.143] — 2026-07-10

### Fixed

- **Syntax-error modal in the COBOL Event Editor** — Saving a handler with syntax
  errors no longer saves silently. A modal lists each error (the raw parser
  message + `line:col`) with a plain-English explanation, and offers **Auto-fix**
  (deterministic reformat/normalise, then re-check), **Keep editing**, or **Save
  anyway**. Validation parses the handler in isolation and reports syntax errors
  only, so it never false-flags the form's shared data items.
- **Agent skill: RustCOBOL type system** — New `agentic_ai/skills/rustcobol-types.md`
  giving the AI agent and code/event assistant a deep, authoritative type contract
  (level numbers, every PICTURE symbol and rule, USAGE ↔ PICTURE validity, numeric
  scale/ranges, control `::` property types, event LINKAGE, and every deviation
  from the COBOL-85 standard) so it does not emit invalid types.
- **Agent skill: concise RustCOBOL** — New `agentic_ai/skills/rustcobol-concise.md`
  teaching inline expression sources for `MOVE`/`SET`/`COMPUTE` and direct control-
  property assignment (e.g. `SET TextBox-1::Value TO Slider-1::Value * 10`), so the
  agent stops emitting one-shot scratch `PIC` items and verbose temp-var chains.
  Both skills are scaffolded non-destructively and always injected into context.

## [PowerRustCOBOL 1.27.142] — 2026-07-10

### Fixed

- **IntelliSense in the COBOL Event Editor** — Completion now offers the form's
  control names **and** its global data items (from the form-level
  WORKING-STORAGE) while writing an event handler, so the developer no longer has
  to leave the editor to check a name.
- **Draggable Event Editor modal** — The COBOL Event Editor window can now be
  moved by dragging its title bar (replaced the fixed anchor with a centred,
  movable, constrained window that remembers its position).
- **Optional verbose AI log** — New "Verbose AI log" toggle in Settings → AI. When
  off (default) the output pane shows only the concise lifecycle (sending /
  streaming / completed), reasoning, and errors; when on it also logs the model /
  message details, the full context sent to the model, and connection timings.
- **Labelled model Refresh button** — The model picker's refresh control is now a
  clear "⟳ Refresh" button that reloads the provider's model list (so a
  local/remote model added or removed is picked up) while keeping the current
  selection.
- **Assistant prompt & skills from `agentic_ai/`** — The code/event assistant now
  draws its system prompt from the project's `agentic_ai/assistant-prompt.md`
  (loaded into Settings only when the prompt field is empty, never overwriting a
  developer's edit) and injects the `agentic_ai/skills/` reference material into
  every request — the same skills the dev agent uses. The assistant prompt is a
  separate file from the dev agent's `system-prompt.md` so the two never collide;
  it is scaffolded non-destructively on project open/create.

## [PowerRustCOBOL 1.27.141] — 2026-07-09

### Fixed

- **AI development agent (spec 025)** — Optional dev-time assistant that can
  deploy/edit controls, generate COBOL event handlers, and create procedures,
  applied only on explicit request through a validated preview with Approve/
  Reject (one undo step). Per-project `agentic_ai/` folder with an editable
  system prompt and a RustCOBOL skill file, scaffolded non-destructively on
  project open/create.
- **AI provider picker** — New "AI Provider" combo in Settings (Ollama Local/
  Cloud, Anthropic, OpenAI, Google, Amazon, Alibaba, Grok). Selecting a provider
  fills its default endpoint URL and recommended prompt, clears the model, and
  fetches the current model list (live where the provider allows, otherwise a
  curated fallback); the Model field is now a populated dropdown with a refresh
  button. Auto-detect and a configurable request timeout round out the AI
  settings.
- **Streamed AI responses** — Requests now stream (SSE) with a per-read timeout
  instead of a single deadline, so slow local models no longer trip the
  "timed out reading response" error mid-generation.
- **AI activity log** — The output/console pane now shows each AI request as it
  unfolds (sending, the full context sent to the model, connecting, status,
  first-token timing, live model reasoning, completion size/elapsed, and errors).
- **AI assistant in the COBOL Event Editor** — The event-handler editor gained a
  multiline, vertically-resizable prompt box (Send on the right) to write/modify
  the handler's COBOL in place.
- **COBOL Event Editor sizing** — The editor and prompt boxes open at a default
  height and only resize when the user drags their grip; fixed a feedback loop
  that made the editor grow on its own until it hit the window borders.
- **Indexed file navigator validation** — The record editor now rejects a
  missing `PIC`/`PICTURE` clause and non-COBOL-85 syntax, and offers "Fix
  errors" and "Beautify" actions to repair/format the record layout.
- **Rounded-corner rendering (corner guardian)** — Container fills only mask the
  corners a child actually reaches, eliminating the transparency "bleed" on
  Panel corners; added guardian instructions and regression tests so it can't
  recur. TextBox multiline now respects the control's vertical size and word-wrap.
- **Property pane** — Removed the Layout section and the Dock property; Anchor is
  now a boolean in the Geometry section; tightened row rendering and value
  alignment.
- **COBOL editor indentation and beautify polish** — Plain Enter now inserts a
  new line aligned to the first non-space column of the previous line; Beautify
  uppercases COBOL reserved words outside string literals, keeps `01`/`77`/`78`/
  `88` data levels in column 8, indents deeper data levels by 3 spaces, and
  suppresses IntelliSense while the cursor is inside a plain quoted string.

## [PowerRustCOBOL 1.27.140] — 2026-07-08

### Fixed

- **Anchor is now a position lock** — Replaced the free-text Anchor edges with a
  boolean Anchor moved into the Geometry section. When on, a control can no longer
  be moved by dragging it with the mouse on the canvas (neither during the drag nor
  on release); X and Y stay editable from the property pane. Removed the Layout
  section and the unused Dock property.
- **Property-pane row rendering** — Removed the dark strip below each dashed grid
  line (the default inter-row gap that read as a drop shadow), added top/bottom
  padding, and vertically centred the value editor so it no longer touches the
  dashed lines. Applies to every control's property rows.
- **Offscreen rounded-corner child clip (opt-in)** — Added a GL capture/re-blit
  path, enabled with `COBOLT_ROUNDED_CLIP`, that clips a rounded Panel/GroupBox's
  children to the arc using the real backdrop + shadow captured mid-walk — fixing
  corner bleed the flat notch mask cannot cover over translucent surfaces, and the
  corner-shadow seam it left behind. Off by default (RAD designer path only); the
  legacy notch mask remains the fallback.

## [PowerRustCOBOL 1.27.139] — 2026-07-08

### Fixed

- **Dormant Panel/GroupBox frame diagnostics** — Added the same red/yellow
  offset frame labels used to isolate chart corner artifacts to the generic
  Panel/GroupBox frame, shadow, notch-mask, and outline-restore layers, kept off
  by default for normal RAD, Preview, and Run Form rendering.

## [PowerRustCOBOL 1.27.138] — 2026-07-08

### Fixed

- **Panel rounded-corner mask frame** — Rounded Panel/GroupBox corner-notch
  masks now run only when the container actually has child content that could
  bleed past the rounded border, preventing empty Panels from showing the dark
  mask-frame artifact in RAD, Preview, and Run Form.

## [PowerRustCOBOL 1.27.137] — 2026-07-08

### Fixed

- **Instant designer drags** — Toolbox drag-and-drop and form-canvas control
  movement now capture on primary-button press instead of waiting for egui's drag
  threshold, so fast mouse movement no longer cancels or misses the drag start.

## [PowerRustCOBOL 1.27.136] — 2026-07-08

### Fixed

- **PictureBox rounded image clipping** — PictureBox image painting now uses a
  textured rounded-rectangle mesh whenever `CornerRadius` is positive, so image
  pixels follow the control radius instead of drawing square corners under the
  rounded frame.

## [PowerRustCOBOL 1.27.135] — 2026-07-08

### Fixed

- **Legacy GroupBox auto-captions** — GroupBox captions that only contain the
  old generated `GroupBox-<n>` control id are now treated as internal defaults
  and suppressed, including stale captions left behind after renaming a
  GroupBox.

## [PowerRustCOBOL 1.27.134] — 2026-07-08

### Fixed

- **GroupBox empty captions** — New GroupBox controls now start with an empty
  `Caption` instead of copying the control id, so the renderer never shows the
  control name unless the user explicitly defines a caption.

## [PowerRustCOBOL 1.27.133] — 2026-07-08

### Fixed

- **PictureBox SVG Center/Normal artifacts** — PictureBox image painting now
  leaves true empty margins when the SizeMode destination is smaller than the
  control, preventing texture edge samples from drawing cross-shaped artifacts
  behind centered SVGs. SVG ICC colour fallback syntax is also sanitized before
  parsing, avoiding repeated terminal warnings from `icc-color(...)` fills.

## [PowerRustCOBOL 1.27.132] — 2026-07-08

### Fixed

- **PictureBox SVG rendering path** — the generic `draw_control` PictureBox
  branch now renders through the destination-sized SVG-aware image painter
  before any preloaded texture fallback, preventing RAD/Preview/Run Form paths
  from scaling an intrinsic SVG bitmap.

## [PowerRustCOBOL 1.27.131] — 2026-07-08

### Fixed

- **PictureBox SVG SizeMode sharpness** — SVG PictureBox images now rasterize
  at the actual Stretch/Zoom/Fill destination size, including display scale, so
  vector artwork stays sharp instead of pixelating from an initially cached
  bitmap.

## [PowerRustCOBOL 1.27.130] — 2026-07-08

### Fixed

- **PictureBox SVG image support** — PictureBox and shared image-loading paths
  now rasterize `.svg` files through the unified forms renderer, allowing SVG
  assets to display consistently in RAD, Preview, and Run Form.

## [PowerRustCOBOL 1.27.129] — 2026-07-08

### Fixed

- **Cross-form control paste handlers** — designer copy/paste now keeps full
  control event-handler payloads in the copy buffer, strips handlers when
  pasting back into the same form, and preserves/remaps handlers with safe
  unique procedure names when pasting controls into another form.

## [PowerRustCOBOL 1.27.128] — 2026-07-08

### Fixed

- **TabControl Run Form content panel** — restored the shared TabControl face
  drawing in interactive Preview/Run Form while keeping tab clicks aligned with
  the new outside-tab layout.

## [PowerRustCOBOL 1.27.127] — 2026-07-08

### Fixed

- **TabControl layout parity** — TabControl tabs now render outside the content
  panel in RAD, Preview, and Run Form, with a new `TabPadding` property that
  controls both tab-to-tab spacing and the tab strip distance from the content
  area.

## [PowerRustCOBOL 1.27.126] — 2026-07-08

### Fixed

- **Stronger Neumorphic negative blur** — doubled the inset relief strength for
  negative blur values so max blur (`-20`) reads as a clearly sunken surface
  while still respecting rounded control geometry.

## [PowerRustCOBOL 1.27.125] — 2026-07-08

### Fixed

- **Chart Neumorphic inset shadow** — chart backgrounds now draw the same
  negative-blur front-plane inset relief used by other Neumorphic controls,
  restoring the sunken effect without reintroducing the rounded-corner artifact.

## [PowerRustCOBOL 1.27.124] — 2026-07-08

### Fixed

- **Dormant chart frame diagnostics** — hid the temporary chart frame labels and
  offsets behind a disabled local switch, keeping the debugging tool available
  without affecting normal RAD, Preview, or Run Form rendering.

## [PowerRustCOBOL 1.27.123] — 2026-07-08

### Fixed

- **Chart notch-mask corner bleed** — removed the chart-only rounded notch mask
  now that chart content is clipped to the plot area, eliminating the dark corner
  wedges identified by the diagnostics while preserving the rounded outline.

## [PowerRustCOBOL 1.27.122] — 2026-07-08

### Fixed

- **Chart frame diagnostics diagonal offset** — restored temporary chart layer
  diagnostics with each candidate frame and label shifted 60 px on both X and Y
  axes to isolate the rounded-corner bleed source.

## [PowerRustCOBOL 1.27.121] — 2026-07-08

### Fixed

- **Chart content corner bleed** — chart grid, axes, and data marks are now
  clipped to the plot area instead of the full chart card, preventing the
  content layer from painting into rounded chart corners.

## [PowerRustCOBOL 1.27.120] — 2026-07-08

### Fixed

- **Chart frame diagnostics offset** — diagnostic frame labels now move together
  with their actual drawn frame rectangles, spacing each candidate layer 60 px
  farther right for visual isolation.

## [PowerRustCOBOL 1.27.119] — 2026-07-08

### Fixed

- **Chart frame diagnostics** — added temporary red/yellow layer labels to chart
  and glass frame drawing paths so the active frame producing rounded-corner
  bleed can be identified visually.

## [PowerRustCOBOL 1.27.118] — 2026-07-08

### Fixed

- **Chart dark corner frame** — chart backgrounds no longer use the generic
  glass card renderer, whose built-in black depth layers could bleed through
  rounded chart corners independently of the configured drop shadow colour.

## [PowerRustCOBOL 1.27.117] — 2026-07-08

### Fixed

- **Chart corner fringe cleanup** — chart rounded-corner masks now overpaint a
  one-pixel cleanup margin while restoring the original chart outline, removing
  dark antialias/shadow wedges at RAD chart corners.

## [PowerRustCOBOL 1.27.116] — 2026-07-08

### Fixed

- **Chart RAD corner cleanup** — rounded chart corner masks now run even when
  the chart background is hidden or transparent, preventing chart internals from
  bleeding into RAD canvas corner notches.

## [PowerRustCOBOL 1.27.115] — 2026-07-08

### Fixed

- **TabControl Neumorphic styling** — TabControl tab buttons now render through
  the shared Button painter so Preview and Run Form keep the same style as the
  RAD canvas, including Neumorphic relief.
- **Neumorphic shadow defaults** — new controls now start with the baseline
  shadow values: 6% opacity, black, SouthEast, 7 px distance, blur enabled, and
  blur strength 8.

## [PowerRustCOBOL 1.27.114] — 2026-07-08

### Fixed

- **Chart preview/run corner bleed** — chart controls now bypass the generic
  glass under-frame and let the chart painter own the rounded background, border,
  and corner mask, preventing residual dark corner bleed in Preview and Run Form.

## [PowerRustCOBOL 1.27.113] — 2026-07-08

### Fixed

- **Chart rounded-corner bleed** — chart corner masks now repaint through the
  full control frame painter instead of the clipped chart-content painter,
  preventing dark chart internals from showing through rounded RAD corners.

## [PowerRustCOBOL 1.27.112] — 2026-07-08

### Fixed

- **TextBox inner padding** — added an `InnerPadding` property for TextBox
  controls and applied it to both design-time text rendering and runtime editing
  placement.

## [PowerRustCOBOL 1.27.111] — 2026-07-08

### Fixed

- **Neumorphic inset blur geometry** — negative blur now renders as rounded
  inward edge bands that fade from the border toward the center, avoiding the
  previous rectangular overlay look and keeping the selection outline above the
  inset effect.

## [PowerRustCOBOL 1.27.110] — 2026-07-08

### Fixed

- **Neumorphic negative blur corners** — negative blur now renders as inward
  rounded relief from the control border, preserving the configured corner
  radius instead of exposing a square clipped shadow rectangle.

## [PowerRustCOBOL 1.27.109] — 2026-07-07

### Fixed

- **Property pane grid separators** — replaced the solid/raised grid separator
  treatment with simple dashed lines in the property inspector.

## [PowerRustCOBOL 1.27.108] — 2026-07-07

### Fixed

- **Neumorphic inset shadow strength** — negative blur now scales the inset
  relief across the full `-1..-20` range instead of appearing nearly constant.

## [PowerRustCOBOL 1.27.107] — 2026-07-07

### Fixed

- **Neumorphic inset shadow polarity** — negative blur now places the
  user-selected drop-shadow colour on the top/left inset edge and the white
  highlight on the bottom/right edge.

## [PowerRustCOBOL 1.27.106] — 2026-07-07

### Fixed

- **Neumorphic inset shadow shape** — negative blur in Neumorphic style now
  renders a two-sided clipped inset relief from the rounded border inward,
  preserving both the user-selected shadow colour and white highlight.

## [PowerRustCOBOL 1.27.105] — 2026-07-07

### Fixed

- **Neumorphic negative blur overlay** — negative `ShadowBlurStrength` in
  Neumorphic style now draws the selected drop-shadow colour as a front-plane
  blurred overlay instead of an inverted/clipped inset shadow.

## [PowerRustCOBOL 1.27.104] — 2026-07-07

### Fixed

- **Negative blur shadow layering** — regular control shadows now preserve
  negative `ShadowBlurStrength` values and draw those shadows above the control
  body, while positive blur shadows continue to render behind it.

## [PowerRustCOBOL 1.27.103] — 2026-07-07

### Fixed

- **Drop shadow corner bleed** — regular control shadows now reuse the same
  canonical corner radius as the control body, including `BorderRadius` fallback
  and size clamping, so zero-blur shadows stay hidden behind rounded controls.

## [PowerRustCOBOL 1.27.102] — 2026-07-07

### Fixed

- **Inspector cleanup** — removed the obsolete `Label for` property from default
  control properties and from the property inspector surface.

## [PowerRustCOBOL 1.27.101] — 2026-07-07

### Fixed

- **Rounded chart corner clipping** — chart internals now repaint rounded-corner
  cutouts with the active form backdrop after drawing their grid/data content,
  preventing corner bleed when shadows use zero or inset blur.

## [PowerRustCOBOL 1.27.100] — 2026-07-07

### Fixed

- **Form property inspector tabs** — applied the same Visuals, Events, and
  Animations tabbed property-grid model to form-level properties, matching the
  selected-control inspector structure.

## [PowerRustCOBOL 1.27.99] — 2026-07-07

### Fixed

- **Property inspector full-grid conversion** — moved remaining selected-control
  and form-level inspector sections onto the shared two-column property grid,
  removing local mini-grids and section-card wrappers from the side inspector.

## [PowerRustCOBOL 1.27.98] — 2026-07-07

### Fixed

- **Property inspector object grid** — changed the selected-control inspector
  rows to a true shared two-column property grid with in-row draggable
  separator, spanning section headers, wrapping label cells, and no forced
  properties pane width.

## [PowerRustCOBOL 1.27.97] — 2026-07-07

### Fixed

- **Property inspector grid model** — refactored the selected-control Visuals
  and Animations inspector tabs to use one continuous two-column property grid
  with shared draggable separator, spanning section rows, and preserved editors
  instead of independent local grid/card layouts.

## [PowerRustCOBOL 1.27.96] — 2026-07-07

### Fixed

- **GroupBox array placement timing** — designer card placement effects now
  restart on mouse release, so the elastic zoom/rubber-band motion begins from
  the committed final card layout instead of aging while the user drags.

## [PowerRustCOBOL 1.27.95] — 2026-07-07

### Fixed

- **Property inspector layout** — grouped the control inspector into Visuals,
  Events, and Animations tabs with theme-aware section headers, shared
  two-column row styling, 3 px cell padding, and a draggable column separator
  without forcing the properties pane width.

## [PowerRustCOBOL 1.27.94] — 2026-07-07

### Fixed

- **GroupBox array placement effects** — added ZoomIn and ZoomOut card
  placement animations with elastic scale-only motion anchored at each card's
  final layout position.

## [PowerRustCOBOL 1.27.93] — 2026-07-07

### Fixed

- **Neumorphic shadow controls** — merged the pending Neumorphic visual update
  so shadow direction, opacity, distance, blur strength, and inset/raised
  behavior are honored consistently by the renderer.

## [PowerRustCOBOL 1.27.92] — 2026-07-06

### Fixed

- **Indexed File Browser tooltip i18n** — the toolbar tooltip now uses the
  localized action label and the feature name now uses Browser instead of
  Viewer.

## [PowerRustCOBOL 1.27.91] — 2026-07-06

### Fixed

- **Indexed File Browser tooltip** — replaced the remaining old-name tooltip
  and action label with the new Indexed File Browser name.

## [PowerRustCOBOL 1.27.90] — 2026-07-06

### Fixed

- **Indexed file grid browser title** — renamed the window title to
  **Indexed File Browser**.

## [PowerRustCOBOL 1.27.89] — 2026-07-06

### Fixed

- **Version bumped to 1.27.89** — updated the IDE version constant for the next
  PowerRustCOBOL build.

## [PowerRustCOBOL 1.27.88] — 2026-07-04

### Fixed

- **Transparency / "growing frame with no content" over inner controls inside databound repeating GroupBox cards (ControlArray) when the parent Panel is scrolled.**
  The symptom (visible in screenshot.png): rounded card frames appeared and moved on scroll, but Labels, Buttons, PictureBoxes and bound data inside the cards were missing or only partially visible; the card's own gradient/fill showed through as if a clip rect or mask was applying full transparency over the children. Upper cards sometimes showed partial data; scrolled-in cards showed "[Loading...]" at bottom of the empty frame area.
  Root cause: in `render_form`, control `screen` rects for cards and members were correctly shifted by `ancestor_auto_scroll_offset` (`-scroll`), and `picturebox_container_border` subtracted scroll for `_ContainerClip`. However, the axis-aligned `clip` passed to `painter.with_clip_rect(...)` (and thus to `draw_control`) was always built from `containers::clip_rect` at raw form-space positions (`origin + cm`) with no scroll adjustment. For a label inside a card inside a VScroll Panel, the card's `content_rect` contribution to clip stayed at its laid-out y while the label drew at y - scroll → draw happened outside the active clip → nothing (or only bg) rendered inside the moved card.
  Fix (minimal, unified engine only):
  - Added `ancestor_clip_rect` (modeled on the existing ancestor scroll walk): walks parents, subtracts scroll *only* for non-scroller ancestors (the repeating cards live in scrolled content space); keeps scroller Panel clips fixed so content does not escape the viewport.
  - Updated `picturebox_container_border` to skip the scroll subtraction when the *immediate* parent itself carries HScroll/VScroll (correct fixed border clip for direct PictureBox children of a rounded scrolling Panel).
  - The general clip site in the render loop now calls the new helper.
  Affects Preview and Run Form (both use `render_form`). Designer canvas (`render_faces`) is unaffected (scroll always zero). Databinding, expansion, PlacementEffect, and H/VScroll drive are unchanged. Backward compatible.
  Guardrail note: per AGENTS.md DataGrid guardrail (this fixes databound repeating visual "cards" that were designed to act like databound lists/grids, plus render/clip/rounded/embedded controls in `render.rs`), the datagrid-quality checklist was applied (see below).

## [PowerRustCOBOL 1.27.87] — 2026-07-04

### Fixed

- **Databound repeating GroupBox (ControlArray) now shows per-row data on cards (not clones of the first/template).**
  - Codegen now emits `INVOKE array 'RefreshBinding'` during `COBOL-DATA-BINDINGS-POPULATE` for ControlArray targets. Combined with load order (LOAD before POPULATE), this automatically computes live ItemCount from the table, recreates the N visual instances, re-applies PlacementEffects, *and* pushes the current row values into each instanced member's properties.
  - Runtime hydration in `refresh_control_array_binding` now also directly pushes StateUpdates under the exact instanced ids (`Group.Group-N.Member`) in addition to the indexed Member path. Guarantees `RunState` / `live(instanced)` in render sees distinct per-card values even across timing or id-resolution edges.
  - Setting `ItemCount` on a `_BindingArray` group now auto-rehydrates current table rows (hook in `obj_set`).
  - Cards with data now appear on initial form load / after RefreshBinding exactly as requested.

## [PowerRustCOBOL 1.27.86] — 2026-07-04

### Fixed

- **"index out of bounds: the len is 26 but the index is 26" crash when running a form containing a databound repeating GroupBox (ControlArray) + ~26 total controls.**
  Root cause: `render_form` performed live+`expand_repeating_groups` producing an expanded control list, then passed indices from its `render_order` (into the expanded list) to `picturebox_container_border`, which always indexed `input.controls` (the original designed list, len=26). Instanced members therefore OOB'd when looking up their (instanced) parent's border for `_ContainerClip`.
  Fix: `picturebox_container_border` now receives the effective `controls` slice + `&dyn FormState` explicitly. Callers in `render_form` (post-expand) and `render_faces` (unchanged) pass the right list. Parent lookup and clip now work for cards inside rounded repeating groups.
  Also removed last stray `[RUN-FORM-DATABIND]` eprintln (converted to targeted tracing debug).

- **RefreshBinding on databound ControlArray/GroupBox-2 now fully recreates cards, reapplies PlacementEffect, and hydrates member values.**
  `refresh_control_array_binding` now:
  - Sets live `ItemCount` (drives re-expansion in render using live state).
  - Bumps `_BindSeq` (forces appear-clock key change).
  - Re-hydrates every mapped member prop for 1..N from the current COBOL table rows (via `_BindingMappings` seeded at codegen/launch time + `set_member_indexed` + subscript values) so cards show fresh data.
  - Stamps `_CardEffect` / `_Card*` metadata during next `expand_repeating_groups` (using live props).
  The appear clock key now incorporates N + seq so deployment (Deal/FadeIn) replays on refresh exactly like first load.
  Codegen emits the `_BindingMappings` seed for ControlArray targets; IDE launch path seeds it too.

- **No more 0-instanced cards after SEED with positive ItemCount on nested or top-level databound GroupBox-2.**
  (Follow-up to prior live_controls + removal of parent guard; expansion now consistently produces instances for IsRepeatingGroup + ItemCount>0.)

## [PowerRustCOBOL 1.27.85] — 2026-07-04

### Fixed / Diagnostics

- **Data-binding instrumentation restricted to run-form execution only (no RAD/designer noise).**
  Removed unconditional `[DATABIND]` output from designer apply/seed/refresh, preview row helpers, canvas ghosts, render expand, and codegen.
  Focused debug now only in the run-form path: interpreter `binding_load`/`binding_populate`/`refresh_datagrid_binding` + REFRESHBINDING (emits `[RUN-FORM-DATABIND]` on stderr during actual "Run Form").
  Includes note highlighting that ControlArray/GroupBox databind has no auto member hydration in `POPULATE` (unlike DataGrid's `_Binding*` + refresh path). Use the same data source on datagrid-1 vs groupbox-2 and observe the difference at runtime.
  Tracing debug remains available with `COBOLT_LOG=debug`. No behavior changes.

## [PowerRustCOBOL 1.27.84] — 2026-07-04

### Fixed

- **Arrayed-control (repeating GroupBox) data binding now shows each row at runtime.**
  A `Member(idx)::Prop` write no longer drops its subscript: `StateUpdate` carries a
  1-based `instance_index`, the interpreter tags array-member writes with it, and the
  IDE routes each write to the matching cloned card. Cloned ids use a collision-safe,
  group-prefixed scheme (`<group>.<group>-<n>.<member>`) shared by the renderer, the
  preview seed, and the runtime router.
- **Numeric expressions parse in the last remaining position.** DISPLAY/MOVE/IF/
  subscripts already accepted arithmetic; the screen-position phrase now accepts a
  bare `LINE`/`COL` (no leading `AT`) with expression operands, e.g.
  `ACCEPT ITEM LINE A + B COL C + D`.

### Changed

- **Repeating-group cards: layout, indexing, and empty-source behavior.** Instances
  start at index 1 and are placed by direction — full height+spacing down (Vertical)
  or width+spacing across (Horizontal), Grid wrapping every `ItemsPerRow`. A databound
  group with **0 rows renders no card at all**; an unbound group still shows its
  template.
- **New `PlacementEffect` for card appearance** (repeating GroupBox): `None` (instant),
  `Deal` (all cards start stacked on the first card, then deal out to their final spots
  one after another — off-screen cards are placed instantly, no phantom fly-in), or
  `FadeIn` (each fades in at its final spot, 200 ms, one after the previous finishes).

## [PowerRustCOBOL 1.27.83] — 2026-07-03

### Fixed

- **Control Array (repeating GroupBox) databinding now produces instances.** When a
  GroupBox is marked IsRepeatingGroup and bound as ControlArray to a CobolTable (or
  other) source, `apply_data_binding_target_properties` + `seed` now correctly drive
  `ItemCount`/`PreviewItemCount` from `OCCURS` (preferred) or row count. Preview render
  snapshots controls *after* seeding so `expand_repeating_groups` (and designer ghosts)
  see n>1. `PreviewState::live` and designer ghost clones now inject per-instance
  `#N` values for mapped member controls (including non-default props like ImagePath,
  Checked via extra writes + updated `preview_value_key`). Added richer `[DB-ARRAY]`,
  `[DB-ARRAY live]`, `[DESIGNER-DB]` instrumentation. Unmapped source fields no longer
  affect general binding or count logic (mappings are a subset). DataGrid and scalar
  bindings unaffected.
- **Deleting a databound control no longer leaves an orphaned binding that blocks
  Run.** When a control (or an array member/host GroupBox) is deleted, its data
  binding — and any dangling field mapping — is pruned automatically. Forms whose
  orphan predates this are self-healed: the binding is dropped before the guardian
  runs, so a since-deleted target no longer triggers `missing-target-control`.
- **The data-binding editor reopens a control's saved configuration.** A control
  that is already bound now offers "Edit current binding", and re-selecting its
  saved source pre-fills the source selection, field rows, and (for control arrays)
  the field→member mappings — instead of starting blank every time.

### Changed

- **Slider: Fore color drives the knob, Back color the track body.** The Appearance
  section's Fore/Back colour now tints the thumb and the track along the scale
  (overriding the Liquid Glass default only when set to a non-default colour). The
  legacy Track/Thumb/Fill colour pickers — which the renderer never used — were
  removed from the inspector.
- **COBOL-table binding no longer asks for a separate occurs item.** A 01-level
  table with OCCURS is enough; the occurs item is derived from the selected 01
  automatically, so the redundant (read-only) occurs-item field is gone.

## [PowerRustCOBOL 1.27.82] — 2026-07-03

### Added

- **Neumorphic form properties (illumination & shadows).** When Theme=Neumorphic the
  Form Properties panel (Appearance section) now shows style-specific editors:
  gradient colors for the illumination effect and for the shadow effect; separate
  blur strength sliders for each; transparency intensity; distance (shadow offset);
  tint color + line weight + blur strength for the extra 3-sided border
  (top-right → bottom-right → bottom-left). All are per-form, stored in .cfrm,
  round-tripped, and affect every render surface. Other themes are unaffected.
  Defaults preserve the previous recipe look.

### Fixed

- **Neumorphic illumination no longer darkens the highlight sides.** The gradient
  color lerp ignored the stops' alpha, so a "transparent" stop (stored as
  transparent black) dragged the highlight toward black — the top/left edges and
  top-left corner rendered dark, like a second shadow. The RGB lerp is now
  alpha-weighted and the layer opacity scales with the interpolated stop alpha, so
  transparent stops fade the effect out instead of darkening it.
- **Neumorphic illumination color pickers were missing from the Appearance grid.**
  A bare separator consumed the first cell of the two-column grid, shifting the
  "Illum. grad." row's pickers into a clipped third column. The separator now
  occupies its own full row.
- **Neumorphic tinted rim now lands on the corner junctions.** The extra 3-sided
  border is drawn as a single connected polyline that begins at the 45° midpoint
  of the top-right corner arc and ends at the 45° midpoint of the bottom-left arc,
  following each corner's own radius (± the per-layer blur offset) — it no longer
  passes half a corner, wraps onto the left edge, or leaves square smudges from the
  old rectangular top/left masks. The outer contour and inner bevel share the same
  path so all three accents stay aligned.

## [PowerRustCOBOL 1.27.81] — 2026-07-03

### Fixed

- **Procedural Neumorphic (soft-UI) effect now fully functional.** The four-layer relief
  (very light neutral bg, raised panel, opposite soft shadows via translate+expand
  rounded rects for highlight top-left / shadow bottom-right, plus two subtle inset
  inner rims) is implemented following the reference recipe. Uses only egui 0.29
  drawing primitives. When Neumorphic glass style is active the form page defaults
  to the recipe bg (#ECEFF4) unless the designer set a distinct colour. Buttons
  suppress incompatible specular overlays. All forms tests and renders remain
  pixel-parity compliant.
- **Charts adapt to the Neumorphic style.** Light chart face (instead of the dark
  navy glass face), soft pastel data palette, faint gray-blue grid/axis lines,
  gray-blue badge/hint text, an engraved inner "tray" contour on the card, molded
  (gently domed) pie slices and bars, white sector separators, and a soft drop
  shadow under the pie disc. The preview and Run-Form viewports also switch to
  light soft-UI widget visuals with gray-blue text (the glass near-white text was
  invisible on the light surface). The dual soft shadows are now truly
  directional — highlight up-left, shadow down-right — instead of a uniform halo.
- **Charts: `BarCornerRadius` is honoured again.** The bar chart's corner-radius
  property existed in the inspector but the renderer hardcoded the radius; both
  the flat and gradient bar paths now apply it (clamped per bar).

## [PowerRustCOBOL 1.27.80] — 2026-07-03

### Changed

- **Neumorphic theme is now 100% procedural — no images.** Neumorphic is a third
  surface style alongside Classic/Enhanced Liquid Glass: elements share the
  background colour and "emerge" from it via a dual soft shadow (dark toward the
  bottom-right, light toward the top-left), with no frost and no hard border.
  Selecting it sets the glass style and clears any image theme-pack override, so
  the neumorphic look no longer loads PNG assets.

### Fixed

- **Forms/DataGrid: rounded corners render correctly while running, even nested.**
  A DataGrid with a corner radius rendered square: its opaque cell/row fills and
  its straight outer-border lines painted over the rounded background, and the
  corner-notch mask (used for Panels) skips nested containers. The grid's own
  fills are now clamped to the grid rect and rounded at the bottom corners (the
  header already rounds the top), and the outer border is drawn as an inset
  rounded stroke — so nothing square pokes past the radius and the outline no
  longer bleeds a light rim outside the corner.

## [PowerRustCOBOL 1.27.79] — 2026-07-03

### Fixed

- **IDE: the running form now runs in its own process.** "Run Form" no longer
  drives the form's interpreter and viewport inside the IDE's own event loop;
  it spawns an isolated `rcrun run-form-ipc` child that hosts the interpreter and
  talks to the IDE over a framed IPC channel (stdin/stdout). A busy or spinning
  form can no longer peg the IDE's UI thread — the IDE stays responsive while the
  form window carries its own cost.
- **Forms/DataGrid: appearance background now rules the whole grid interior.**
  Regions with no explicit cell/column colour — the gap around a framed "pill"
  cell, the filler area right of the last column, and the gutter beneath the
  vertical separators — now fall back to the DataGrid's appearance
  `BackgroundColor` instead of showing the translucent glass (which read as a grey
  wash over the form backdrop). A fully-transparent column colour is treated as
  "unset" so the fallback applies.
- **Forms/DataGrid: column background image honours its configured opacity.** The
  per-column background image was painted at a fixed alpha; it now scales by the
  column's "Cell background" opacity and the control's own Opacity.
- **Forms/DataGrid: rounded corners are kept while running.** Opaque cell/row
  fills no longer poke a square corner past the grid's rounded background — the
  corner-notch mask now trims the DataGrid the same way it trims Panels/GroupBoxes.
- **Forms/Panel: rounded corners keep their border line.** The corner-notch mask
  repainted the backdrop over each rounded corner, erasing the container's
  border/rim there (border visible on the straight edges, missing at the corners).
  The outline is now restored on all four corner arcs.

### Changed

- **Themes: asset-based theme packs.** Added a Neumorphic form theme option and
  bundled theme assets (updated cobalt-steel control skins; new emerald-glass and
  neumorphic packs).
- **Chore: removed the temporary `[TIMER-DBG]`/`tdbg` diagnostic instrumentation**
  left over from the idle-CPU investigation.

## [PowerRustCOBOL 1.27.78] — 2026-07-01

### Fixed

- **IDE/Forms: precise Timer-driven repaint scheduling.** Live forms and Timers no longer poll the repaint loop at a fixed fraction of the interval. The Timer now wakes the UI exactly when the next `onTick` is due (with a small floor to avoid spin), and the running-form viewport removes its unconditional `request_repaint()`. Combined with the prior reactive root loop, idle forms no longer peg a CPU core.

## [PowerRustCOBOL 1.27.77] — 2026-07-02

### Fixed

- **Parser: `FUNCTION RANDOM` now parses, and the FUNCTION-argument loop can no
  longer hang.** `RANDOM` lexes as a keyword (from `ACCESS MODE IS RANDOM`), so
  the FUNCTION-name reader rejected it and left the token stuck — inside another
  function's arguments (e.g. `FUNCTION INTEGER(FUNCTION RANDOM * 4)`) that spun
  the parser forever and froze the IDE. The intrinsic name is now accepted and
  the argument loop has a no-progress guard, so malformed input always terminates
  with a diagnostic.
- **Parser: optional `IS` before `GLOBAL` / `EXTERNAL`.** The COBOL-85 `[IS]
  GLOBAL` / `[IS] EXTERNAL` connective is now consumed instead of warning.
- **Forms: DataGrid scrolling no longer bleeds into its container.** While the
  pointer is over a DataGrid the grid consumes the wheel and zeroes the frame
  scroll deltas, so the surrounding GroupBox / form no longer scrolls too.
- **Forms: a Timer honours its `Enabled` property.** The tick is gated on the
  Timer's own `Enabled` property (default true), not the generic control-enabled
  chrome flag, so a non-visual Timer with `enabled="false"` still fires `onTick`.
- **IDE: reactive repaint loop.** A running form no longer pegs a CPU core while
  idle — the event loop repaints only when there is work to drain and sleeps
  otherwise.

### Added

- **IDE: event-handler validation with the project-tree semaphore.** Each form's
  generated COBOL is validated (syntax + semantic) on save, on Run, before Build,
  and on project open; the tree dot turns green/red per form and Run/Build are
  refused with a clear message until the code is fixed.
- **IDE: apply runtime DataGrid layout back to the design.** While a form runs, a
  floating "Apply layout to design" button persists interactively-adjusted column
  widths / row height into the form as the control's new defaults.
- **IDE: Run-Form process inspector.** A toolbar toggle (in the designer RAD
  toolbar, next to Run Form) opens an always-on-top window with real-time line
  charts (Process CPU, Memory RSS, Child processes, System Memory), a process
  tree, and leak / runaway-CPU / rogue-subprocess detection that dumps to the
  console and a per-project-configurable file. Samples only while the Live
  Interpreter runs. (Adds the `sysinfo` dependency.)

## [PowerRustCOBOL 1.27.76] — 2026-07-02

### Fixed

- **`FUNCTION RANDOM` now honours its seed argument (COBOL-85).** The intrinsic
  previously ignored any argument, so the standard way to seed the generator —
  `FUNCTION RANDOM(seed)` — did nothing and every run replayed the same
  sequence. A seed argument now (re)seeds the generator deterministically and
  returns the first value of that sequence, while an unseeded `FUNCTION RANDOM`
  continues the current sequence. The same seed reproduces the same sequence
  (e.g. `FUNCTION RANDOM(12345)` for stable demo data); seed from a varying
  value for a fresh sequence each run (e.g. `ACCEPT ws-time FROM TIME` then
  `FUNCTION RANDOM(ws-time)`).
- **`ACCEPT … FROM TIME` resolves to real centiseconds.** The TIME register uses
  the standard `HHMMSSss` 8-digit layout, but the hundredths were hard-coded to
  `00` (whole-second resolution). They are now populated from the sub-second
  clock — still COBOL-85 compliant (hundredths of a second, not milliseconds).
  This also sharpens the time portion of `FUNCTION CURRENT-DATE` and lets a
  time-seeded `FUNCTION RANDOM` differ between runs launched more than ~1/100 s
  apart.

## [PowerRustCOBOL 1.27.75] — 2026-07-02

### Fixed

- **Non-visual controls (Timer) can no longer freeze the IDE/RAD.** The form
  interpreter now honours a cooperative cancellation flag checked between every
  statement — which covers every PERFORM iteration and paragraph body — so a
  long-running or looping event handler (for example a `Timer` `onTick`, or a
  heavy `onLoad`) aborts promptly instead of pinning the interpreter thread.
  Closing the running-form window, relaunching, or exiting the IDE now sets that
  flag and no longer blocks the UI thread: `stop()` waits only a short bounded
  grace period for the thread to unwind and then detaches it, so the application
  stays responsive and is always closeable. A blocking statement (e.g. a large
  file read) can finish its current step, but can never hang the whole IDE.
- **Timer tick coalescing.** A `Timer` emitted `onTick` on every elapsed
  interval regardless of whether the previous tick's handler had finished; a
  handler slower than the interval flooded the unbounded UI→interpreter event
  queue, starving the quit sentinel and eventually hanging a relaunch. Ticks are
  now skipped while the interpreter's event queue is still non-empty
  (WinForms-style coalescing), while user events — clicks, edits, focus changes,
  quit — are never dropped.
- **COBOL errors surface in a dialog and stop cleanly, without closing the
  IDE.** A parse/semantic (syntax) error when launching a form, or a fatal
  runtime error reported by the interpreter, is now shown in a modal "⛔ COBOL
  error" window (with the message and a pointer to the Output panel) in addition
  to the console line. Processing stops and the IDE/RAD stays open — it no longer
  fails silently or leaves the run window in limbo.
- **Find bar no longer drifts while searching.** The editor's floating Find/
  Replace bar was anchored to the scrolling text-content rect, so jumping
  between matches (which scrolls the editor) dragged the bar up and down. It is
  now anchored to the stable editor viewport and stays where it opened; it is
  also draggable — move it anywhere and it keeps that position.

## [PowerRustCOBOL 1.27.74] — 2026-07-01

### Fixed

- **Editor search has a case-insensitivity toggle** — the code editor's Find bar
  now has an "Aa" toggle (on by default) to switch between case-insensitive and
  case-sensitive matching. Matching also switched to ASCII-lowercasing so match
  offsets stay byte-accurate.

## [PowerRustCOBOL 1.27.73] — 2026-07-01

### Fixed

- **Controls can be renamed from the Properties inspector** — the control id in
  the Identity header is now an editable field. Renaming to a unique, valid
  identifier updates every reference form-wide: child `parent` links, `LabelFor`
  associations, the control's event-handler paragraph names, data-binding
  target/source/member references, and control references in handler/procedure
  code (`Old::…` / `Old(i)…`). The rename is undoable; a taken or invalid name is
  rejected.

## [PowerRustCOBOL 1.27.72] — 2026-07-01

### Fixed

- **DataGrid frozen columns also clip the filter row** — with frozen columns and
  column filters shown, horizontally scrolling drew the scrollable columns' filter
  input boxes *over* the frozen band. The filter inputs (egui widgets, not
  painter-drawn) are now clipped to the region right of the frozen columns, so the
  whole filter row scrolls behind the frozen columns like the header and body.

## [PowerRustCOBOL 1.27.71] — 2026-07-01

### Fixed

- **Editor Find box keeps focus while typing** — typing in the code editor's
  Find field no longer kicks focus back into the editor after each keystroke.
  Incremental search still scrolls to the first match, but keyboard focus only
  moves into the editor on an explicit navigation (Next/Prev/Enter) or after
  applying an autocomplete suggestion. The Replace field was unaffected.

## [PowerRustCOBOL 1.27.70] — 2026-07-01

### Fixed

- **Repeating groups now render their instances at run time** — the shared render
  engine expands a repeating GroupBox into N cards (one per `ItemCount`, falling
  back to `PreviewItemCount`), laid out by the group's Vertical / Horizontal /
  Grid direction and spacing. Each instance's controls are cloned with
  instance-unique ids so they render and interact independently. This is the
  runtime foundation for control-array data binding (data-driven population of
  each card is the next step).

## [PowerRustCOBOL 1.27.69] — 2026-07-01

### Fixed

- **Array-member event handlers receive the array index** — a control that
  belongs to a repeating group (array) now gets an event-handler stub that
  declares `01 CONTROL-ARRAY-INDEX PIC S9(4) COMP-5.` in its LINKAGE SECTION and
  `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX`, with a hint showing indexed
  member access (`Name(CONTROL-ARRAY-INDEX)::Property`). Both the generated
  `.cbl` stub and the handler skeleton opened in the IDE editor use it; regular
  (non-array) controls keep the plain stub.

## [PowerRustCOBOL 1.27.68] — 2026-07-01

### Fixed

- **Repeating-group binding editor can map fields to member controls** — the
  control-array (repeating GroupBox) binding modal now has a "Map fields to
  controls" section: each source field can be assigned to a member control, and
  the control's default bindable property is shown and applied (Label→Caption,
  TextBox→Text, CheckBox→Checked, **PictureBox→ImagePath**, ComboBox/ListBox and
  numeric controls→Value). Applying the binding records a `ControlProperty`
  mapping per mapped field; unmapped fields are skipped.

## [PowerRustCOBOL 1.27.67] — 2026-07-01

### Fixed

- **DataGrid frozen panes can cast a drop shadow** — a new "Frozen pane shadow"
  toggle (on by default) draws a soft shadow from the last frozen column
  (rightward) and the frozen header/rows (downward) onto the content that
  scrolls behind them, giving the usual spreadsheet freeze cue. The shadow only
  appears when the grid actually scrolls in that direction.

## [PowerRustCOBOL 1.27.66] — 2026-07-01

### Fixed

- **Data-binding source buttons work for a repeating GroupBox (control array)** —
  choosing a source (COBOL table, SQL, …) on a repeating GroupBox did nothing:
  the editor was keyed by the array **name** rather than the GroupBox's control
  id, so the settings modal opened and instantly closed, and apply couldn't
  resolve the target. The binding editor is now keyed by the control id, so the
  modal stays open and the binding applies to the control array.

## [PowerRustCOBOL 1.27.65] — 2026-07-01

### Fixed

- **DataGrid Image cells support corner radius and a drop shadow** — a column
  whose Edit control is **Image** now exposes an "Image corner radius" and an
  "Image drop shadow" setting in the column editor. The cell picture is rounded
  to the chosen radius and, when enabled, drawn over a soft two-layer shadow.

## [PowerRustCOBOL 1.27.64] — 2026-07-01

### Fixed

- **DataGrid frozen columns now clip the scrollable columns** — with one or more
  frozen columns, horizontally scrolling the grid drew the scrollable columns
  *over* the frozen band. Scrollable header cells, body cells, and column
  separators are now clipped to the region right of the frozen columns, so they
  slide behind the frozen band (mirroring the already-correct frozen-row
  behavior).

## [PowerRustCOBOL 1.27.63] — 2026-07-01

### Fixed

- **DataGrid COBOL masks now honour edited pictures** — a column mask such as
  `ZZZ,ZZZ,ZZ9.99` now zero-suppresses, inserts digit-group commas and the
  displayed decimal point, and signs negatives, so a bound `S9(9)V99` value like
  `000003000.00` renders as `3,000.00` instead of the raw zero-padded digits.
  Check-protection (`*`) fill and `9(n)`/`S9(n)V99` plain pictures are unchanged.
- **DataGrid columns can render their value as an image** — a new **Image** edit
  control treats the (alphanumeric) cell value as an image file path and draws
  the picture fitted to the cell (falling back to the path text when the image
  can't be loaded), useful for thumbnail columns.

## [PowerRustCOBOL 1.27.62] — 2026-07-01

### Fixed

- **Run Form no longer fails silently on a startup error** — a runtime error
  while a form starts (e.g. in its `onLoad`) was swallowed, so the interpreter
  thread died and the run window never appeared with no message at all. Fatal
  form-runtime errors are now surfaced to the Output pane
  (`⛔ Form runtime error: …`) so the cause is visible instead of the run
  silently doing nothing.
- **Clearer error when assigning to a control method** — using a method call as a
  MOVE/assignment target (e.g. `MOVE … TO Grid::RefreshBinding()`) now reports
  which method it was and that it must be called as a statement, not used as a
  receiving field.

## [PowerRustCOBOL 1.27.61] — 2026-07-01

### Fixed

- **Data-bound DataGrid COBOL mask can be changed and is applied** — a COBOL
  mask typed into a bound column's editor was reset to the bound field's
  PICTURE on every save/run binding refresh, so it could never be changed and
  cell values did not pass through it. The binding refresh now seeds a column's
  mask from the field only when the column has none, preserving a user-typed
  mask as a deliberate override; the DataGrid renderer already formats each
  bound value through that mask before display.

## [PowerRustCOBOL 1.27.60] — 2026-07-01

### Fixed

- **DataGrid alternating highlight can now stripe columns** — a new "Alternating
  mode" setting (Rows / Columns / None) chooses whether the alternating
  background color highlights every other row (default, unchanged for existing
  forms), every other column, or nothing. Column striping reuses the same
  alternating color and opacity and sits beneath any per-cell or per-column
  background.

## [PowerRustCOBOL 1.27.59] — 2026-07-01

### Fixed

- **DataGrid background patterns tile evenly** — dot, stripe, cross, X, X-dots,
  and O background patterns previously started from a fixed top-left offset and
  left a ragged, uneven gap at the right and bottom edges. Patterns now pick the
  tile count that fits the grid and spread the tiles with balanced margins on all
  sides, so the automatic tiling looks evenly distributed at any size.

## [PowerRustCOBOL 1.27.58] — 2026-07-01

### Fixed

- **Every control is fully Liquid Glass again** — the solid background layer
  added in 1.27.57 flattened glass-backed controls (buttons, PictureBoxes,
  menu/tool bars) into opaque slabs. That underlay is removed from the shared
  glass renderer, so all controls return to translucent Liquid Glass. The one
  exception is the DataGrid, which keeps fine-grained control over its grid,
  column, row, and cell backgrounds: a DataGrid still on the default background
  renders as glass, and a chosen grid background color paints solid beneath the
  frost.
- **DataGrid grid-line color is now the grid's foreground** — the Appearance
  section's Fore color drives the DataGrid grid-line color, replacing the
  separate entry in the grid settings modal. A grid left on the default
  foreground uses the subtle built-in line color, and existing forms with a
  `GridLineColor` continue to render via a compatibility fallback.

## [PowerRustCOBOL 1.27.57] — 2026-07-01

### Fixed

- **Control background opacity can now reach true solid colors** — glass-backed
  controls paint their selected background color as an opacity-aware base layer,
  and custom interactive backgrounds no longer cap full opacity below 100%.

## [PowerRustCOBOL 1.27.56] — 2026-07-01

### Fixed

- **DataGrid column filters are now editable in the header** — filter rows use
  real text inputs instead of painted placeholder text, and edits update the
  same `AdvancedGrid`/`ColumnFilters` metadata used by DataGrid filtering.

## [PowerRustCOBOL 1.27.55] — 2026-07-01

### Fixed

- **DataGrid inner shape colors can now be driven by cell values** — the
  DataGrid column settings modal exposes value/color definitions for inner
  shapes, allowing values such as `ACTIVE`, `SUSPENDED`, and `CANCELED` to map
  to their own shape background colors.

## [PowerRustCOBOL 1.27.54] — 2026-07-01

### Fixed

- **DataGrid data-binding debug output no longer floods the console** — removed
  temporary `[data-binding]` console diagnostics from binding apply and shared
  DataGrid render paths while leaving binding hydration and preview rows
  unchanged.

## [PowerRustCOBOL 1.27.53] — 2026-07-01

### Fixed

- **DataGrid frozen panes and keyboard navigation now work in the shared
  renderer** — frozen columns/rows use the resolved advanced grid state,
  scrollable rows no longer displace the frozen row band, keyboard movement
  selects cells with arrows/Page/Home/End, column resize booleans honor typed
  values, explicit text alignment wins, and grid/row/column backgrounds support
  cross, X, X-dots, and O patterns.

## [PowerRustCOBOL 1.27.52] — 2026-07-01

### Fixed

- **DataGrid headers and COBOL masks now render correctly** — DataGrid headers
  apply `CornerRadius` only to the top-left and top-right corners, and bound
  columns now use their COBOL mask when formatting displayed cell values.

## [PowerRustCOBOL 1.27.51] — 2026-06-30

### Fixed

- **DataGrid settings moved into a focused modal and rendering options now apply
  in the shared renderer** — the right-side properties pane now exposes a compact
  DataGrid editor entry, while the modal handles grid backgrounds, column masks,
  edit controls, column fonts, filter headers, inner cell frames, gauges, and
  line styles without forcing minimum modal dimensions.

## [PowerRustCOBOL 1.27.50] — 2026-06-30

### Fixed

- **Advanced DataGrid behavior is now guarded across runtime, binding, CSV,
  i18n, and docs** — DataGrid runtime methods, CSV export mode/order,
  advanced binding metadata preservation, localized property labels, and the
  English developer guide now cover the advanced grid feature set.

## [PowerRustCOBOL 1.27.49] — 2026-06-30

### Fixed

- **Runtime controls now honor `CornerRadius` in custom interactive renderers** —
  runtime-only drawing paths for DataGrid, ListBox, NumericUpDown, TabControl,
  TreeView, Splitter, MenuBar, ToolBar, StatusBar, and Button hover/press
  overlays now use the same corner-radius helper as the Form Designer.

## [PowerRustCOBOL 1.27.48] — 2026-06-30

### Fixed

- **DataGrid rows now stay inside the grid and scroll** — the shared renderer
  clips DataGrid content to the control bounds, keeps the header fixed, supports
  mouse-wheel scrolling through overflowing rows, and draws a small scrollbar
  indicator when additional rows are available.

## [PowerRustCOBOL 1.27.47] — 2026-06-30

### Fixed

- **DataGrid alternating row highlight is subtle by default** — added
  `AlternatingRowOpacity` with a 20% default, applied it in the shared renderer,
  exposed it in the DataGrid properties panel, and included it in format-painter
  style copying.

## [PowerRustCOBOL 1.27.46] — 2026-06-30

### Fixed

- **DataGrid cells now clip text to their own columns** — long bound values
  such as thumbnail image paths no longer spill across column separators and
  visually overlap adjacent captions in the shared form renderer.

## [PowerRustCOBOL 1.27.45] — 2026-06-30

### Fixed

- **DataGrid now exposes `RefreshBinding()` for live COBOL tables** — running
  forms seed bound DataGrid metadata into the interpreter, the runtime
  `RefreshBinding` method rebuilds `Rows` from current `FIELD(n)` COBOL table
  values, and the editor autocomplete now lists the method for DataGrid
  controls.

## [PowerRustCOBOL 1.27.44] — 2026-06-30

### Fixed

- **DataGrid COBOL table bindings now read indexed MOVE initialization rows** —
  bound grids now hydrate rows from form event, control event, and user
  procedure statements like `MOVE value TO FIELD(n)` before falling back to
  synthetic preview data, so COBOL table examples populated in OnLoad/OnShow
  display their real row values.

## [PowerRustCOBOL 1.27.43] — 2026-06-30

### Fixed

- **DataGrid bindings now hydrate preview rows** — DataGrid binding refresh now
  fills the grid's `Rows` property from COBOL table initial values when
  available, falls back to deterministic preview rows from binding fields when
  only definitions exist, and refreshes bound grid properties before save/run so
  existing bindings do not stay header-only.

## [PowerRustCOBOL 1.27.42] — 2026-06-30

### Fixed

- **DataGrid binding diagnostics now expose the render gap** — applying a
  DataGrid binding now writes renderer-compatible `Name:Type` column
  definitions, the Properties panel edits those definitions as multiline text,
  and Apply/render paths emit console diagnostics for columns, rows, data
  source, and binding field counts.

## [PowerRustCOBOL 1.27.41] — 2026-06-30

### Fixed

- **Data binding Apply now hydrates DataGrid basics** — applying a DataGrid
  binding updates the grid's Columns and DataSource properties from the binding
  definitions immediately, and replaces the previous binding for that target so
  the visible grid stays wired to the latest settings.

## [PowerRustCOBOL 1.27.40] — 2026-06-30

### Fixed

- **COBOL table Add field now chooses from missing real fields** — the COBOL
  table binding editor shows a selector of fields that exist in the selected
  working-storage table but are not yet mapped, and hides the add flow once all
  table fields are present.

## [PowerRustCOBOL 1.27.39] — 2026-06-30

### Fixed

- **COBOL table data binding now uses real working-storage tables** — the table
  selector no longer invents a placeholder value, lists eligible 01-level
  GLOBAL OCCURS tables from the form working-storage section, limits added
  fields to missing fields from the selected table, and shows an explicit
  dropdown settings button only for Dropdown edit controls.

## [PowerRustCOBOL 1.27.38] — 2026-06-30

### Fixed

- **Data-binding source fields use aligned grid columns again** — source-field
  rows now render through an egui grid while dropdown details remain in their
  separate modal, keeping columns aligned without reintroducing inline
  dropdown-detail width pressure.

## [PowerRustCOBOL 1.27.37] — 2026-06-30

### Fixed

- **Dropdown configuration now opens in its own modal** — selecting Dropdown
  for a data-binding field or clicking an existing dropdown row opens a separate
  configuration window, keeping source-field rows compact and avoiding inline
  dropdown-detail width pressure.

## [PowerRustCOBOL 1.27.36] — 2026-06-30

### Fixed

- **Dropdown configuration panels no longer widen the source-field grid** — the
  expanded data-binding dropdown editor stays aligned under the Picture column
  while using a bounded in-row panel instead of forcing horizontal scrolling.

## [PowerRustCOBOL 1.27.35] — 2026-06-30

### Fixed

- **Data-binding settings no longer auto-grow beyond the working area** — the
  modal width is capped and wide source-field grids scroll horizontally inside
  the window instead of forcing the data-binding window wider than the screen.

## [PowerRustCOBOL 1.27.34] — 2026-06-30

### Fixed

- **COBOL table data-binding settings now open a real configuration form** —
  selecting COBOL table shows the table and occurs item, COBOL field mappings,
  nested dropdown lookup configuration with COBOL/indexed origins, add/restore
  behavior, and COBOL-table Apply validation inside the data-binding modal.

## [PowerRustCOBOL 1.27.33] — 2026-06-30

### Fixed

- **REST API data-binding settings now open a real configuration form** —
  selecting REST API shows endpoint, method, headers, authentication, JSON
  preview, JSONPath guidance, REST field mappings, add/restore behavior, and
  REST-specific Apply validation inside the data-binding modal.

## [PowerRustCOBOL 1.27.32] — 2026-06-30

### Fixed

- **SQL data-binding settings now match the reference form details** — SQL
  pagination uses the requested navigation glyphs, dropdown lookup mock data
  uses the current Indexed-file samples, nested dropdown panels include the
  separator styling, and Apply validation rejects non-positive lookup line
  limits.

## [PowerRustCOBOL 1.27.31] — 2026-06-30

### Fixed

- **Data Binding settings now include the SQL control configuration form** —
  selecting SQL opens an interactive SQL-control source section with paginated
  result-set preview controls, SQL field mappings, dropdown lookup
  configuration for SQL controls and COBOL tables, line limits, add/restore
  behavior, and Apply validation.

## [PowerRustCOBOL 1.27.30] — 2026-06-30

### Fixed

- **Data Binding settings now open a full Indexed file configuration modal** —
  the Properties panel opens an interactive, scrollable editor with source
  selection, clear confirmation, indexed-file preview pagination, sample record
  grid, source-field mapping rows, dropdown sub-configuration panels, restore
  removed fields, and Apply validation.

## [PowerRustCOBOL 1.27.29] — 2026-06-30

### Fixed

- **Data Binding source buttons now open a configuration editor** — choosing
  Indexed, SQL, COBOL table, REST, or Agent AI in the Properties panel opens an
  inline binding editor for the selected approved target, allowing the developer
  to review and edit binding IDs, source details, fields, and generated mappings
  before applying the form-level binding.

## [PowerRustCOBOL 1.27.28] — 2026-06-29

### Fixed

- **Data binding is now guarded from source to runtime** — form-level bindings
  can wire Indexed files, SQL, COBOL tables, REST schemas, and Agent AI
  structured outputs into grids, charts, dropdowns, listboxes, and explicit
  control arrays, while the Data Binding Guardian blocks unsafe saves, runs,
  checks, builds, and packages before mappings can corrupt bound data.
- **Bound controls keep writeback state recoverable** — generated binding code
  loads and populates targets deterministically, writable bindings preserve row
  identity and pending edits, read-only bindings never write back, and failed
  updates keep the pending value available for repair.

## [PowerRustCOBOL 1.27.27] — 2026-06-29

### Fixed

- **Run Form property updates now treat quoted and bare property names the
  same** — live interpreter updates such as `MOVE Slider-1::Value TO
  label-5::Caption` now overwrite the designed `Caption` property instead of
  creating a separate uppercase `CAPTION` shadow key, matching the behavior of
  `label-5::"Caption"`.

## [PowerRustCOBOL 1.27.26] — 2026-06-29

### Fixed

- **Run Form now fires the newly exposed live control events** — the unified
  form renderer emits right-click/context-menu, double-click alias, mouse move,
  mouse wheel, hover enter/leave, control load, TextBox text/key aliases,
  checkbox/radio value aliases, and Slider final value events, and a regression
  test verifies generated `onClick` handlers execute through the live
  interpreter channel.

## [PowerRustCOBOL 1.27.25] — 2026-06-29

### Fixed

- **Designer clipboard actions are now reachable from the RAD UI** — Cut, Copy,
  Paste, and Duplicate are available in the Form Designer toolbar and the canvas
  right-click menu, using the same selection-aware clipboard behavior as the
  keyboard shortcuts.

## [PowerRustCOBOL 1.27.24] — 2026-06-29

### Fixed

- **Existing controls now show the expanded Events list** — the Properties
  panel already reads events dynamically from each control type, and those
  supported event lists now include the comprehensive design-time events such as
  `onRightClick`, `onDoubleClick`, `onHoverEnter`, `onResize`, and
  `onPropertyChanged` while preserving non-visual control event lists.

## [PowerRustCOBOL 1.27.23] — 2026-06-29

### Fixed

- **Reusable User Controls are now project-backed designer components** — a
  selected GroupBox can be saved as a named User Control, shown in the Toolbox,
  deployed as regular qualified controls, nested inside other User Controls, and
  removed from the project without breaking existing form instances.
- **Designer clipboard workflows are safer and more complete** — `Cmd+C`,
  `Cmd+X`, `Cmd+V`, and `Cmd+D` now copy, cut, paste, and duplicate selected
  controls while preserving child containment and regenerating IDs/handlers for
  pasted instances.
- **Deletion confirmation now protects event-handler code** — removing controls
  with handler bodies shows a confirmation dialog with handler/control counts,
  while confirmed deletions still recycle the removed code for recovery.
- **User Control child properties and events resolve by qualified IDs** —
  selecting a deployed User Control shows grouped child properties, runtime
  `GetProperty`/`SetProperty` can target `Child.Property`, and generated event
  dispatch keeps full child IDs such as `CUSTOMERCARD-1-BUTTON1--ONCLICK`.

## [PowerRustCOBOL 1.27.20] — 2026-06-26

### Fixed

- **Child controls no longer bleed past a rounded container's corner** — a
  PictureBox, Animator, chart, or any control inside a rounded GroupBox/Panel is
  now clipped to the parent's **border path** instead of its own bounds. The
  control keeps its size; whatever overflows the container's rounded corner is cut
  by the container shape. The render engine widens a child's clip to the parent
  border and the unified `draw_control` rounds each face (image, film, glass card,
  chart background) on any corner that lands on the container arc.
- **Corner-notch masking for content egui can't round-clip** — egui only supports
  axis-aligned clipping, so grid lines and other fine chart/control content can't
  be rounded directly. After a rounded container's children are painted, its four
  corner notches are repainted with the backdrop (solid colour and/or the
  background image, tiled when the form tiles), covering any residual bleed. The
  solid fill is applied only when opaque, so a translucent canvas is never
  double-painted into a darker wedge.
- **`draw_glass` is now per-corner** — the frosted-glass card renderer accepts a
  full `Rounding` (not a single radius), so a control whose corner meets a rounded
  container follows that arc on that corner alone. Images load with Repeat wrap so
  a tiled backdrop also tiles inside the notch mask.

## [PowerRustCOBOL 1.27.19] — 2026-06-24

### Fixed

- **GroupBox no longer clips child content under an empty title area** — a
  GroupBox always reserved an 18px top "caption band" (plus a 6px inset on the
  other sides) for its content, so a child placed across the top edge was cut off
  even when the GroupBox had no caption. Children are now clipped to the border on
  every side, and the top caption band is reserved only when a caption is actually
  shown (sized to clear the legend text). With a caption, content clips just below
  it; without one, it reaches the border.

## [PowerRustCOBOL 1.27.18] — 2026-06-24

### Fixed

- **Captions only where they belong** — non-text controls (Panel, and any control
  without a real caption) no longer show a centered "<id>" placeholder. The
  **GroupBox** caption now renders as a title on the **top-left border** (classic
  legend look, editable in the property pane) instead of centered. Label, Button,
  CheckBox, and RadioButton keep their caption.
- **Form/image "Browse" button now sets the path in the window you clicked** — the
  background-image picker (and the control image picker) used shared keys across
  the in-window inspector and a detached Designer window, so whichever rendered
  first consumed the file-dialog result and the path didn't land where expected.
  The picker state is now namespaced per window.

## [PowerRustCOBOL 1.27.17] — 2026-06-24

### Fixed

- **PictureBox image no longer dimmed in the running form / preview / binary** —
  a photo shown vivid on the designer canvas looked washed-out everywhere else,
  because the runtime surfaces drew images through a different code path with a
  different tint. They now use the designer's exact path, so an image looks the
  same on every surface.

### Changed

- **The unified render engine now drives all four surfaces** — the Form Designer
  canvas, the live preview, the running form, and the compiled binary all render
  through one engine in `cobolt-forms`. The four separate, drifting draw loops
  (and the old `render_run_control`) are gone, so the designer, preview, run, and
  binary are guaranteed to match. The designer keeps its editor overlay
  (selection handles, badges, drop hints) on top. This completes the unification
  begun in 1.27.16.

## [PowerRustCOBOL 1.27.16] — 2026-06-23

### Fixed

- **Compiled binaries now look like the IDE** — a standalone built form rendered
  every control as a plain, unstyled native widget (no background image, no glass
  charts, no themed slider/date picker). The compiled binary now draws through the
  same unified render engine as the Form Designer, live preview, and running form,
  so a packaged form matches what you designed.
- **Slider no longer gets stuck in a built binary** — a freshly opened window
  could receive a burst of phantom pointer input that left the slider's drag in a
  bad state, so dragging the knob did nothing. Phantom input at window-open is now
  ignored during a short warm-up, and the slider clears any stale drag state, so it
  responds normally.
- **Chart "Data Binding — COBOL Table" properties stack one per row** — the table
  binding fields (Table item, Row count item, Label field, Value field(s), Series
  labels) were packed onto a single line, forcing the property pane to scroll
  horizontally. Each field is now on its own row. The Scatter chart's "Bubble size
  field" had the same defect and is fixed too.

### Changed

- **One render engine for every surface (internal)** — the Form Designer canvas,
  live preview, running form, and compiled binary now share a single rendering
  engine in `cobolt-forms`, replacing four separate draw loops. This is the
  groundwork that makes the designer, preview, run, and binary look identical.

## [PowerRustCOBOL 1.27.15] — 2026-06-23

### Fixed

- **A control's value now reaches its change handler in the running form** —
  binding a COBOL handler to a control's change event (e.g. a Slider's) and
  reading the control's value inside it returned the initial value (`0`) no
  matter how far you moved the knob. UI-driven value changes (slider drag, text
  edit, combo/list selection) are now synced to the interpreter the instant the
  event fires, so the handler reads the live value — not the seeded default.

## [PowerRustCOBOL 1.27.14] — 2026-06-21

### Fixed

- **Rotated lines no longer clip in the running form** — the running form clipped
  every control to its own bounding box, so a Line rotated past its (thin) box was
  cut off, while the designer drew it in full. The run now clips controls only to
  their container ancestors (like the designer), so a rotated/angled Line shows
  completely on every surface.
- **Line DashStyle works** — the Line control ignored DashStyle and always drew a
  solid line. **Dash**, **Dot**, and **DashDot** now render real dashed/dotted
  patterns (via egui dashed-line shapes); **Solid** is unchanged.

### Added

- **Line rounded ends** — a **Rounded ends** toggle on the Line control draws
  round caps at both endpoints.

### Fixed

- **PictureBox image aspect ratio** — the Form Designer canvas stretched the
  image to fill the box regardless of **SizeMode**, while the preview and running
  form preserved the aspect ratio. The designer now honours SizeMode too (using
  the image's native size), so Fit/Zoom/Center keep the aspect ratio and the image
  looks identical on the canvas, in the preview, and at run time.
- **Chart axis lines are controllable** — charts drew fixed X/Y axis lines with no
  way to remove them. All (non-pie/donut) charts now have **Show X axis line** and
  **Show Y axis line** toggles so the chart can show just its data.
- **Line direction is no longer limited to the presets** — the **Line** control
  gained an **Angle°** property (0–359) so it can point in any direction, not only
  Horizontal/Vertical/Diagonal. Setting the angle overrides the legacy preset;
  existing lines are unchanged. *(An on-canvas rotation knob is a follow-up.)*

## [PowerRustCOBOL 1.27.12] — 2026-06-21

Fix: apply the active theme to the preview & running-form viewports (spec 017).

### Fixed

- **Charts/themed controls use the same theme on every surface** — the live
  preview and the running form each render in their own egui `Context`, and only
  the designer canvas was calling `set_active_theme`. So `draw_chart_preview`
  (which reads the active theme for its palette/styling) fell back to defaults in
  the preview and run, making charts look different from the canvas. Both now set
  the owning designer's active theme pack on their context before rendering.
- Removed the temporary on-screen render diagnostics.



Fix: charts render through the designer's path on every surface (spec 017 step).

### Fixed

- **Charts match the designer in preview and the running form** — the live
  preview and the running form drew charts by calling the chart painter
  **directly**, bypassing the card-frame + glass layering that `draw_control`
  applies on the designer canvas, so a chart (e.g. an AreaChart) looked washed
  out / different when run. Both now render charts through **`draw_control`** — the
  exact path the Form Designer uses — so the chart is identical on the canvas, in
  the preview, and in the running form. (Part of the spec-017 move toward a single
  rendering engine; the Form Designer is the source of truth.)

## [PowerRustCOBOL 1.27.10] — 2026-06-21

Fix: running form now matches the designer/preview (backdrop + glass).

### Fixed

- **Glass toggle tracked reliably in the running form** — the run resolved the
  owning designer only by file path, which could miss (path normalisation) and
  fall back to a stale launch-time value, so a glass-on chart rendered dim
  (solid dark) instead of vivid (frosted). It now resolves by path **then form
  name** and keeps the runtime snapshot in sync, so the running form's glass
  matches the canvas — charts and other translucent content look identical.
- **Running form backdrop** — the live (interpreted) form derived its background
  straight from the form colour, so a pure-black / unset background rendered the
  window pure black. The preview and designer instead fall back to a default dark
  navy in that case. The run now uses the **same** rule (strip `#`, first 6 hex
  digits, black/unset ⇒ dark navy), so translucent glass content — charts in
  particular — no longer looks washed out over a black window and matches the
  canvas and preview.

## [PowerRustCOBOL 1.27.9] — 2026-06-21

Universal corner radius + rounded content (spec 016). Pre-production, treated as
a fix.

### Added

- **Corner radius on every bordered control** — buttons, text boxes, combo/list
  boxes, picture boxes, data grids, numeric/date pickers, progress bars, sliders,
  shapes, charts, and the containers now share one **Corner radius** property.
  The background and border round to it, and content is clipped to the rounded
  shape — a **PictureBox** image is trimmed to the rounded corners over any
  background (via a textured `RectShape`), and chart frames round too.
  `Corner radius = 0` keeps square corners and no clipping (the default, so
  existing forms are unchanged); the value is clamped to half the smaller side.
  Applies identically on the canvas, the live preview, and the running form.

### Changed

- The container property is unified under **CornerRadius**; the legacy
  `BorderRadius` is still read as an alias so older forms round correctly.

### Known limitations

- The editable text/scroll layer of run-time inputs stays square inside a rounded
  frame, and container **children** are clipped to the rectangular content area
  (egui has no rounded scissor; rounded corners are cosmetic on the frame).

## [PowerRustCOBOL 1.27.8] — 2026-06-21

Fix: preview/run rendering parity with the Form Designer.

### Fixed

- **Glass look now matches the designer** — the live preview and the running form
  always rendered with the Liquid-Glass look, even when the designer's glass
  toggle was off. They now mirror the launching designer's glass setting, so a
  flat (non-glass) canvas runs flat — charts and panels keep the same vivid,
  non-frosted appearance instead of looking washed out.
- **Containers (Panel) render in the live run** — a Panel previously fell through
  to a generic blue glass box with a "Panel" caption when the form was run; it now
  uses the shared `draw_control` renderer, so it looks identical to the designer
  (and to GroupBox). The generic run-time fallback for any other visual control
  also routes through `draw_control` instead of an approximate glass box.
- **TextBox look matches everywhere** — the live preview and the running form drew
  TextBoxes with a hard-coded dark-blue glass and fixed light text. They now draw
  the same `draw_control` face as the designer (honouring BackgroundColor /
  gradient / border) with the editable text in the control's ForegroundColor.
- **DateTimePicker field matches** — the running form's date field now uses the
  shared renderer for its face (the calendar popup is unchanged).
- **Containment in the running form** — the live run now clips children to their
  container's content area, fades them by ancestor opacity, and hides controls on
  a non-selected tab page, exactly like the designer and preview (e.g. a chart
  inside a GroupBox no longer spills past the box). The running form also tracks
  the designer's glass toggle live.

## [PowerRustCOBOL 1.27.7] — 2026-06-21

Visual repeating groups (GroupBox arrays) — spec 015, Phases 1–2 (designer +
model only). Pre-production, treated as a fix.

### Added

- **GroupBox appearance** — new **Hide caption**, **Hide background**, and a
  two-colour **Background gradient** (Vertical / Horizontal / DiagonalDown /
  DiagonalUp / Radial) alongside the existing background colour and border
  radius. Hide-background draws no fill/border while children stay visible.
- **Repeating groups** — a GroupBox can be marked as a repeating array template
  via the right-click menu (**Set / Unset as Repeating Group**). A **▦ ARRAY**
  badge marks it, and a **Repeating Group** properties section exposes array
  name, item count, data source, layout direction (Vertical / Horizontal /
  Grid), item spacing, items-per-row, auto-scroll-parent, clone-events and
  preview-items.
- **Design-time preview** — **Preview items > 1** renders render-only ghost
  instances laid out per the chosen direction, without adding controls to the
  form model (selection/undo unaffected).

Runtime instancing, indexed event dispatch, and data binding (spec 015 Phases
3–5) are not included in this release.

## [PowerRustCOBOL 1.27.6] — 2026-06-21

Fixes: form-designer scrolling regression + chart monochrome polish.

### Fixed

- **Form Designer scrolling restored** — the canvas `ScrollArea` now uses
  `auto_shrink([false, false])`, so a form larger than the viewport scrolls again
  (regressed alongside the spec-012 container work).
- **Monochrome colour picker** — compact 16×16 grid with **1px pure-white internal
  grid lines**, no external border and no padding between swatch and line (much
  smaller than before); the selected swatch is marked.
- **Greyscale column** — one hue column of the 256-colour selector is replaced by
  **16 shades of grey** (still no pure black/white).
- **Chart "Hide Background" honoured** — a chart with `HideBackground` set now
  draws **no** card/glass frame at all. Previously the generic control frame was
  painted behind the chart preview, so the background still showed through when
  the property was checked.

### Added

- **Monochrome gradient** — a `MonochromeGradient` toggle on charts. Each data
  element gets its **own** tonal gradient (±20% of the base): bars shade
  vertically, scatter bubbles and pie/donut slices shade radially; line and area
  charts get a **vertical** gradient fill (bright at the line, fading toward the
  baseline). Area/stacked translucency for the non-gradient case is unchanged.
- **Smooth line/area curves** — the `Smooth` chart property now actually renders
  a **Catmull-Rom spline** (line and area/stacked charts), matching the smooth
  reference look; `ShowPoints` gates the line markers.

## [PowerRustCOBOL 1.27.5] — 2026-06-20

Fix: **chart monochrome mode** (spec 013). Pre-production, treated as a fix.

### Fixed

- Charts gain a **Monochrome** toggle + a **MonochromeColor** chosen from a fixed
  **256-colour** selector (pure black/white and near-extremes excluded).
- When on, data elements (bars, slices, lines, points, areas, markers) render in
  distinguishable **tonal variations** of the base colour (same hue family) across
  all six chart types; grid lines use a soft **pastel** variant, axes a stronger
  pastel, and slice borders a lighter variant — so the chart isn't flat.
- Labels, legends, titles keep the **foreground colour** (not recoloured), and
  area/stacked **transparency** is unchanged.
- When off, charts render exactly as before. Grid visibility remains the existing
  **ShowGridLines** toggle (no duplicate property added).

## [PowerRustCOBOL 1.27.4] — 2026-06-20

Fix: **form container controls** — GroupBox, Panel, and TabControl become real
containers (spec 012). Pre-production, so treated as a fix that completes intended
behaviour.

### Fixed

- **Real containment & nesting** — controls can be placed inside GroupBox, Panel,
  and TabControl to any depth and in any combination, via a `parent` link on each
  control. The `.cfrm` round-trips it; legacy `<Children>` files are migrated on
  load, and the old Panel `Scrollable` flag maps to the new `AutoScroll`.
- **Reparent by drag-and-drop** — drop a control on the form to detach it, over a
  container's content area to nest it, or over another control to adopt that
  control's parent. Undoable, with a guard against dropping a container into its
  own descendant.
- **Move-with-parent & cascade delete** — moving a container moves its whole
  subtree; deleting a container removes its descendants.
- **Clipping + border radius** — children are clipped to the container's content
  area; each container has a configurable `BorderRadius`.
- **Working `Opacity`** — a container's `Opacity` now fades the container and its
  subtree (the property previously had no visual effect on any control).
- **TabControl pages** — each tab owns its own children; clicking a tab switches
  the active page; only the active page's controls are shown and interactive — at
  design time and in the IDE run-preview.
- **Auto-scroll property** — per-container `AutoScroll` (default off → overflow is
  clipped), editable in the properties pane.

Known follow-ups (spec 012): auto-scroll *scrollbars*, the drag-time drop-target
highlight, and standalone-binary render parity.

## [PowerRustCOBOL 1.27.3] — 2026-06-20

Fix: chart controls gain a **Hide background** property.

### Fixed

- Every chart (Bar, Line, Pie, Area, Scatter, Donut) now has a **Hide
  background** toggle in the properties pane. When checked, the panel's
  background fill and border frame are not drawn — only the chart content (grid,
  axes, labels, data) is rendered, so the chart sits transparently on the form.
  Default is off (unchanged appearance). Applies at design time and at run time
  (shared renderer).

## [PowerRustCOBOL 1.27.2] — 2026-06-20

Fix: complete the RustCOBOL `::` member-access model — IntelliSense now lists
properties alongside methods, the `::` operator chains to any depth over a real
nested object model, and a control property is a receiving field for every verb
(spec 011).

### Fixed

- **IntelliSense `::` popup** — the property/method list now shows **properties
  (green)** as well as methods (light blue); the `::` / `::"` member list and
  chain tails (`…)::`, `…::member::`) are resolved against the chain's root
  control.
- **Member-access chains** — the `::` operator now chains to any depth with one
  consistent syntax: `Grid-1::Rows(I)::Columns(2)::Value`,
  `obj::Value::toUpperCase()`. A `(n)` subscript indexes a collection, a bare
  name is a property, and `()` is a method call.
- **Nested object model** — controls hold nested objects and indexable
  collections (rows → columns → cells, list items), navigated by the chain;
  legacy newline-string item lists interoperate (`List-1::Items(3)`).
- **Property as a receiving field for every verb** — not just `MOVE`/`SET` but
  `STRING`/`UNSTRING INTO`, `ADD … TO`, `COMPUTE`, `ACCEPT`, `INSPECT`,
  `INVOKE … RETURNING`, … may write to `control::property` (and nested cells).
- **Collection / value helpers** on a chain element — `Count`, `Delete`,
  `Clear`, `Add`, and the transforms `toUpperCase`, `toLowerCase`, `trim`, `len`.
- **INITIALIZE on a control** — `INITIALIZE obj` resets its `Value` property;
  `INITIALIZE obj::prop` targets one property; `INITIALIZE obj name` initialises
  each operand by its own rules.
- A chain ending in a **method call** `()` is a value, never a receiving field —
  `MOVE name TO obj::method()` is rejected (runtime error + a compile-time
  diagnostic); a chain ending in a **property** or **indexed cell** is assignable.

## [PowerRustCOBOL 1.27.1] — 2026-06-20

Fix: standardise control property & method access on the RustCOBOL `::`/`INVOKE`
forms and remove the redundant Fujitsu `"Property" OF Control` syntax (spec 010).

### Changed / Fixed

- **One way to touch a control property** — the `::` member syntax and the
  `INVOKE` verb, for both read and write:
  - GET: `control::property`, `control::"property"`, `INVOKE control "property"
    RETURNING x`, `INVOKE control "GET-property" RETURNING x`.
  - SET: `MOVE v TO control::property`, `SET control::"property" TO v`,
    `INVOKE control "property" USING v`, `INVOKE control "SET-property" USING v`.
  - A bare member resolves as a property accessor (get with no argument, set with
    a `USING` argument); `GET-`/`SET-` are explicit prefixes; explicit methods
    (`SetCaption`, `GetText`, …) keep priority.
- **Case-insensitive property names**, and numeric properties read as numbers so
  `IF Slider1::Value > 50` and arithmetic stay algebraic.
- **Removed** the inherited Fujitsu `"Property" OF Control` syntax entirely
  (parser, AST, runtime, IntelliSense, docs). No legacy code used it, so this is
  not a breaking change. (This also drops the `OF` form's property-as-receiver in
  arbitrary verbs and indexed property paths; use `::`/`INVOKE` with a data item.)
- **IntelliSense:** typing `::` or `::"` after a control id lists its **properties
  (green)** and **methods (light blue)** and filters as you type; a lone `"` opens
  no popup.

## [PowerRustCOBOL 1.27.0] — 2026-06-19

Form module model (spec 009) — procedure scoping, sharing & lifecycle.

### New / Changed

- **All procedures are `IS COMMON`.** Every woven procedure — event handlers and
  user procedures alike — is now generated `IS COMMON PROGRAM`, so any procedure
  is callable from anywhere in the form module (a handler may `CALL` another
  handler, a user procedure may call a handler, …). Previously only user
  procedures were `COMMON`.
- **Static-by-default procedures.** A procedure's local `WORKING-STORAGE` is now
  initialised **once** and **persists across calls** (re-entering a handler keeps
  its values; exiting does not cancel it), matching COBOL-85. `CANCEL "<name>"`
  resets a procedure's state; `INITIALIZE` (unchanged) resets the items you
  choose, each call.
- **`FD … IS GLOBAL`.** A global `FD` is accepted and validated; the file and its
  record are visible to the form's nested procedures. `GLOBAL` placement is now
  validated (valid only on `01`/`77` items and `FD`s) alongside `EXTERNAL`.
- **Inline `obj::method()` as a value.** The inline method call now works as a
  value operand inside `DISPLAY`/`MOVE`/`COMPUTE` (e.g. `DISPLAY S::len()`), not
  only as a statement — folded in from the 005 Rust-FFI AC6.

### Notes

- `INVOKE-FORM` (form invoking another form) and `#INCLUDE` (copying in external
  embedded programs) are **deferred** to a follow-up; cross-process `EXTERNAL`
  sharing remains scoped to a single run unit.

## [PowerRustCOBOL 1.26.0] — 2026-06-19

Form themes (spec 007) — engine + selection + reference pack.

### New

- **Selectable form themes.** Forms can be skinned by a selectable, extensible
  catalogue of themes, applied by the shared control renderer so a themed form
  looks identical in the designer, the preview, and (once the web target lands)
  the compiled app. Two kinds sit under one picker: the built-in procedural
  **Liquid Glass** (the default, unchanged) and **asset-pack** themes.
- **Project default + per-form override.** A project default theme is set in
  *Settings → Appearance* (`[forms] theme` in `cobolt.toml`); any form can
  override it in its *Appearance → Form theme* property, or inherit the default.
  Resolution is per-form → project → Liquid Glass. (i18n across all six
  languages.)
- **Self-describing asset packs (9-slice).** A theme pack is a drop-in folder
  `assets/themes/<id>/` with a `theme.toml` manifest plus per-control /
  per-state 9-slice images, an optional themed background, a foreground colour,
  and a chart palette/stroke. New packs are discovered automatically and appear
  in the picker with no code change. A control a pack doesn't cover falls back to
  Liquid Glass; a control's explicit colours still win.
- **Themed charts.** Pie/line/bar data marks take the active theme's palette and
  stroke, not just the chart frame.
- **Optional themed background.** A form can opt into a pack's background image
  (*Appearance → Use theme background*); otherwise its own back-colour / image
  applies.
- **Reference pack `cobalt-steel`.** A small, procedurally generated, original
  pack (see `cargo run -p cobolt-forms --example gen_reference_theme`) that
  exercises the engine end-to-end.

### Changed

- **Unified control renderer.** The canonical `draw_control` (and the system-font
  module) now live in `cobolt-forms` (`cobolt_forms::paint`), so the designer,
  preview, run form, and future compiled/web binaries all draw through one
  renderer. Liquid Glass is byte-for-byte unchanged.

### Notes

- The four "special" art packs (stainless steel, dark wood, modeling clay,
  knitted wool) and the WASM/desktop-binary embedding are staged behind their
  asset and spec-006 dependencies; the engine is ready for both.

## [PowerRustCOBOL 1.25.0] — 2026-06-18

COBOL Structure & shared data (spec 005, Phase 1).

### New

- **COBOL Structure editor.** The form inspector lists the five shared COBOL
  blocks — `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
  `WORKING-STORAGE` — plus the form's user procedures; clicking a row opens a
  popup that edits that one block. Add / rename / delete user procedures from the
  list. The blocks are woven verbatim into the generated program. (i18n across
  all six languages.)
- **`GLOBAL` / `EXTERNAL` / `GLOBAL EXTERNAL` data sharing.** `EXTERNAL` `01`/`77`
  items (and `FD`s) are now shared run-unit-wide by their real name; `GLOBAL`
  items stay visible to a module's contained programs. The checker flags
  `EXTERNAL` on anything other than `01`/`77`/`FD`.
- **User procedures.** Named nested programs the event handlers can `CALL`;
  generated `IS COMMON` so siblings may call them.
- **COBOL-2002 `USAGE IS OBJECT REFERENCE <class>`** parses, and `REPOSITORY`
  starts pre-seeded with a curated Rust-FFI type bridge (all primitives + common
  std classes, `CLASS RUST-x IS "Rust.x"`). Declarations generate today; invoking
  Rust through them is Phase 2.

## [PowerRustCOBOL 1.24.1] — 2026-06-18

### Fixed

- **`EXTERNAL` data is now shared run-unit-wide.** `01`/`77`-level items (and
  `FD`s) declared `EXTERNAL` were silently ignored at run time. They are now
  registered in a single run-unit store and shared by their real name across
  program activations, so one program's update is seen by another in the same
  run unit. `GLOBAL`-only items stay private to each form, as before. (spec 005)

## [PowerRustCOBOL 1.24.0] — 2026-06-17

Per-control test example projects, plus form-rendering fixes surfaced by them.

### New

- **Per-control examples** — a runnable test project for every toolbox control
  under `examples/<control>/`: the subject control with a console
  `DISPLAY "<Event> working"` per supported event and one button per property
  that changes it from COBOL via `INVOKE … "SetProperty"`. `examples/build-all.sh`
  builds all 34; `cargo run -p cobolt-codegen --example check_examples` verifies
  event/property coverage.

### Fixed

- **Codegen** — Timer (`SetInterval`), DataGrid (`ExportCSV`), and AgentObject
  (`Ask`) emitted `INVOKE "<id>" '…'` with the control id quoted as a string
  literal, which the parser rejected; the id is now an unquoted identifier so
  forms using those controls build.
- **Run-form window** — scrollbars now appear automatically when a form is larger
  than its window, so off-screen content is reachable.
- **Default colours** — `ForegroundColor` now defaults to white, and Button/Label
  text falls back to white, so captions are legible on the dark run-form canvas.

## [PowerRustCOBOL 1.23.0] — 2026-06-15

Indexed File Editor, Grid Browser, and `.cidx` codegen in the IDE.

### New

- **Indexed Files** project-tree category (after Forms) listing `.cidx` definitions.
- **Indexed File Editor** — separate viewport to define or inspect record layout,
  keys, storage flags, and per-field grid controls; structural lock after finalize.
- **Import existing…** — register an on-disk indexed data file; schema inferred via
  `inspect_any_path` when available.
- **Indexed File Grid Browser** — virtualized table with add/edit/delete,
  Commit/Rollback, and schema-drift protection.
- **Codegen** — `generated/<stem>-indexed.cbl` regenerated on Build / Run / Debug /
  Check (same contract as forms).
- **`cobolt-indexed` crate** — `.cidx` XML model shared by IDE, codegen, and runtime.

## [PowerRustCOBOL 1.22.0] — 2026-06-14

Branding, About box, generated-code lifecycle, and spec-driven development infrastructure.

### New

- **Application icon.** The IDE ships with the PowerRustCOBOL samurai icon
  (`assets/images/powerrustcobol-icon.png`), used as the window/taskbar icon and
  overridable via an `app-icon.png` in the config directory.
- **Help → About.** A new About window shows the mascot, version, copyright and
  the Apache-2.0 license.
- **"Powered by PowerRustCOBOL" badge.** A badge (`made-with-powerrustcobol.png`,
  plus a high-resolution `.webp` master) with README + Developer's Guide
  instructions for developers to add it to their own apps' About box.
- **Developer banner in generated COBOL.** Every RAD-generated `.cbl` now opens
  with a `*>` comment block telling the developer it is generated, must not be
  edited directly, and may change structure between versions.
- **Automatic regeneration.** Form COBOL is regenerated from the current forms
  on every **Build / Run / Debug / Check**, so what compiles and runs always
  matches the forms.
- The mascot now appears in the README and the Developer's Guide cover.

### Infrastructure

- **Spec-driven development.** Gated workflow (`/specify` → `/plan` → `/tasks` →
  `/implement` → `/docsync`) with steering docs, templates, and committed skills
  under `specs/` and `.claude/skills/`. See `specs/README.md`.

## [PowerRustCOBOL 1.21.0] — 2026-06-14

French interface language.

### New

- **French (Français) UI language.** A sixth interface language joins
  EN/ES/PT/JA/ZH; pick 🇫🇷 Français from the language selector. The full IDE UI
  is translated (menus, toolbar, settings, the form designer and property
  inspector, the debugger, the AI assistant, and the documentation viewer).
  - The Documentation viewer shows the English Developer's Guide for French
    until a French translation of the guide is provided.

## [PowerRustCOBOL 1.20.0] — 2026-06-14

Documentation viewer with Markdown + Mermaid rendering.

### New

- **Help → Documentation.** A new window renders the embedded PowerRustCOBOL
  documentation (Markdown) with its **Mermaid** diagrams drawn inline — rendered
  in pure Rust (`mermaid-rs-renderer` → SVG → `resvg`), no Node/Chromium.
  - Two-pane layout: a searchable document list and a rendered viewer; the docs
    are embedded at build time (offline), and `Cmd+O` opens any local `.md`.
  - **File** (Print → PDF, Close), **View** (Zoom In/Out, Full Screen, Outline)
    and **Help** (Shortcuts) menus.
  - In-document **search** with **blue-on-yellow** match highlighting, a `Go`
    button and `Enter` to jump to the first match, `◀ / ▶` (and `,` / `.`) to
    step between matches with a live `n/total` counter; the focused match shows
    in orange and is scrolled into view.
  - A clickable **outline** (table of contents) **and** clickable in-document
    `[…](#…)` links that jump to their section.
  - An **icon toolbar** (vector icons) mirroring the shortcuts: open a file
    (Cmd+O), view source (Opt+Cmd+U), keep on top (Cmd+T), print (Cmd+P), close
    (Cmd+W).
  - Adjustable **font size** (`A+ / A−`, Cmd+`+` / Cmd+`-`) that is **remembered
    across sessions**; plus zoom, full-screen, and a view-source modal.
  - A translucent **frosted-glass** window (uneven procedural fog).
  - **Print** renders the document to a PDF (with the diagrams embedded) and
    opens it in the OS viewer. The PDF font is a system sans-serif extracted at
    runtime — nothing is bundled.
  - Theme-aware (adopts the IDE style) and I18N-aware (EN/ES/PT/JA/ZH).

## [PowerRustCOBOL 1.19.0] — 2026-06-14

Optional persistence for in-memory indexed files (`STORAGE IS MEMORY`).

### New

- **`STORAGE IS MEMORY WITH PERSISTENCE`** (SELECT-clause extension). An in-RAM
  indexed file can now opt into being written to its disk container **on `CLOSE`
  only** — never on `COMMIT`, so the in-memory performance profile is preserved.
  The phrase combines with compression (`STORAGE IS MEMORY WITH COMPRESSION WITH
  PERSISTENCE`).

### Changed

- **`STORAGE IS MEMORY` is now ephemeral by default.** Without `WITH
  PERSISTENCE`, a MEMORY file's contents are discarded at `CLOSE` (an existing
  disk file is still *loaded* on `OPEN`). `COMMIT`/`ROLLBACK` on a MEMORY file
  are pure in-RAM transaction boundaries and never touch disk.
- **`OPEN OUTPUT` always (re)creates the on-disk container** for a MEMORY file,
  regardless of the persistence setting, so the file exists on disk.
- The two published `STORAGE IS MEMORY` file-I/O tests were updated to declare
  `WITH PERSISTENCE` (they verify cross-`CLOSE` persistence). New self-checking
  test `tests/cobol/fileio/idx_mem_persist.cbl` covers both modes.

### Docs

- Developer's Guide §14: "Two storage modes" and "When data reaches disk"
  updated for the ephemeral default and `WITH PERSISTENCE`.

## [PowerRustCOBOL 1.18.0] — 2026-06-13

COBOL-85 language features: binary table search and file-error declaratives.

### New

- **`SEARCH ALL` (binary search).** `SEARCH ALL` now parses and executes as a
  true binary search over an `OCCURS` table declared with an
  `ASCENDING`/`DESCENDING KEY`. The `OCCURS … KEY IS …` phrase is captured
  (previously skipped) and drives the bisection; the `ALL` keyword is recognised
  after `SEARCH` regardless of token form. Serial `SEARCH` is unchanged.
- **`DECLARATIVES` / `USE AFTER STANDARD ERROR PROCEDURE`.** A
  `DECLARATIVES … END DECLARATIVES` block at the head of the `PROCEDURE DIVISION`
  registers file-error handlers. When a file verb (`OPEN`/`READ`/`WRITE`/
  `REWRITE`/`DELETE`/`START`/`CLOSE`) ends with an error `FILE STATUS` that the
  statement did not handle with its own `AT END` / `INVALID KEY` phrase, the
  matching `USE` procedure runs. Targets may be file names, an open mode
  (`INPUT`/`OUTPUT`/`I-O`/`EXTEND`), or a catch-all. New lexer tokens
  (`DECLARATIVES`, `USE`), AST (`ProcedureDivision.declaratives`,
  `UseProcedure`), parser, and runtime dispatch with a re-entrancy guard.

### Fixed

- **`NOT =` (and other negated relations) after `AND`/`OR`.** A negated relational
  condition on the right of a combined condition — e.g. `IF A NOT = X AND B NOT =
  Y` — now parses; previously the bare identifier before `NOT` was mis-read as an
  88-level condition-name, orphaning the `NOT`.
- **Arithmetic statement before a `NOT …` phrase.** An `ADD`/`SUBTRACT`/
  `MULTIPLY`/`DIVIDE`/`COMPUTE` used as the imperative of an `INVALID KEY` /
  `AT END` / `ON EXCEPTION` / `ON OVERFLOW` branch no longer swallows the
  following `NOT` (it previously mis-read `NOT INVALID KEY` etc. as the start of
  `NOT ON SIZE ERROR`). The `NOT` is now consumed only when it actually
  introduces `NOT [ON] SIZE ERROR`.
- **`CALL … USING` parameter passing (nested programs).** Arguments are now bound
  to the called program's `PROCEDURE DIVISION USING` LINKAGE items: values are
  copied in before the call and `BY REFERENCE` arguments receive the updated
  values on return (`BY CONTENT` / `BY VALUE` are not written back). Previously
  the arguments were ignored, so LINKAGE items stayed at their defaults.
- **`STRING … WITH POINTER`.** The pointer is now honoured: text is placed
  starting at the 1-based pointer position (preserving earlier bytes) and the
  pointer is advanced past the last byte moved, with overflow detected from that
  position. Previously the pointer was ignored.
- **Inline `PERFORM WITH TEST BEFORE/AFTER UNTIL`.** The inline (no-paragraph)
  form now accepts the optional `WITH` before `TEST` — e.g.
  `PERFORM WITH TEST AFTER UNTIL … END-PERFORM` — matching the out-of-line form.
  `TEST AFTER` runs the body once before evaluating the condition.
- **`EVALUATE` stacked `WHEN`.** Several consecutive `WHEN` phrases that share a
  single following imperative (e.g. `WHEN 1 WHEN 3 WHEN 5 MOVE …`) now all select
  that imperative, as COBOL-85 requires (previously the value-only `WHEN`s ran an
  empty branch).

### Docs

- Developer's Guide §13: new "Searching tables" and "Centralised file-error
  handling" subsections.

## [PowerRustCOBOL 1.17.0] — 2026-06-10

IDE visual redesign — "dark glass" look.

### Changed / New

- **Glass card panels.** The project tree, output, main pane and property
  inspector now sit on rounded, subtly-bordered glass cards with soft shadows
  (`theme::glass_panel_frame`).
- **Opaque, pane-matched background.** The whole window is painted with an opaque
  floor + the optional background image + the same pane fill, so the area around
  the panes matches the panes (no desktop bleed / no bright wallpaper in the
  gaps). The "Transparent background" option was **removed**.
- **Collapsible property section cards** in the form inspector (Form Properties /
  Target Device / Appearance / Background Image / Size / Events) with blue ▸/▾
  headers (`section_card`); the control inspector shares the same blue card-style
  section headers for consistency.
- **New "Deep Blue" theme** (17 total) — near-black glass panes with blue accents.
- **Full-width selection pill** + hover highlight in the tree; **left-aligned**,
  snug rows (fixes centred/jittery labels); grey indent/divider lines removed.
- **Solid semaphore knobs** — the green/yellow/red item-status dots are now crisp
  filled circles.
- **Standardised non-visual control icons** — Timer/AI-Agent/REST/SQL share one
  glass card and consistent stroke-drawn icons (no more mismatched colours, emoji
  tofu boxes or the one-off orange SQL cylinder).
- **Toolbar** reordered to **Open · Save · Check · Build · Run · Debug · Stop · ⚙**;
  the separate Debug row now only appears during an active debug session.
- **RAD properties panel** resizes up to half the window width (was capped at
  320px, clipping long values); **project tree** defaults to 410px wide.
- Roomier spacing, 8px control corners, larger fonts retained.

## [PowerRustCOBOL 1.16.0] — 2026-06-10

IDE: transparent-background option, calmer background, roomier UI.

### New features

- **Transparent background option** (Appearance dialog). When enabled, the IDE
  background colour is fully transparent — the desktop shows through the glass
  panels — and a background image, if set, **keeps its own transparency** (its
  alpha is preserved, scaled only by the opacity slider). Per project
  (`[ide] transparent_background`). In this mode the panels become more
  translucent so the desktop/image reads through.

### Changed

- **Calmer background, more readable panels.** With an opaque background the
  image is now drawn over the themed base and a **low-noise dark overlay** so it
  reads as a subtle backdrop instead of competing with the editor; panels stay
  at full readable opacity (they are no longer force-thinned just because an
  image is set).
- **Roomier, softer UI.** More spacing between rows and around sections (larger
  item spacing, button padding, row height, window/menu margins) and softer
  control corners (8 px radius) for a less cramped, more polished feel.

## [PowerRustCOBOL 1.15.2] — 2026-06-10

IDE: assets can be added and ship with the build.

### Fixed

- **The Assets category now accepts any file** (images, audio, video, fonts,
  data, …). The "Add" picker passed a `"*"` filter to the native dialog, which
  greyed out every file on macOS/GTK; assets now open with **no extension
  filter** so anything is selectable.
- **Adding a file from outside the project now imports it.** Previously a file
  outside the project directory was rejected ("must be inside the project
  directory"). The chosen file is now **copied into a category subfolder**
  (`src/`, `forms/`, `assets/`, `docs/`) and tracked, so it becomes part of the
  project. The add is also routed to the category you clicked (not guessed from
  the extension).

### Changed

- **Bundled assets ship with the native build.** `cobolt build` now copies every
  tracked Assets/Documentation file next to the produced binary (under `bin/`,
  preserving the project-relative layout), so images/audio/fonts are available
  to the program at runtime. (The `.zip` package already included them.)

## [PowerRustCOBOL 1.15.1] — 2026-06-10

IDE: background image now actually shows, lighter divider lines on dark themes,
and 10 more colour themes.

### Fixed

- **The IDE background image now appears.** It was painted on the background
  layer but the panels tiled the whole window at ~80–95 % opacity, hiding it.
  Now, when a background image is set, the panels become noticeably more
  translucent (frosted glass), the image is drawn over an **opaque themed base**
  (replacing the desktop bleed-through) so it reads as a real wallpaper, and the
  opacity slider dims it via a scrim. Default opacity raised to **70 %**.
- **Divider/border lines are light-grey on dark themes** (and a mid-grey on
  light themes) so separators are clearly visible against the dark chrome.

### New features

- **10 more colour themes** (16 total): Dracula, Nord, One Dark, Gruvbox Dark,
  Tokyo Night, Night Owl, Cobalt2, Solarized Light, GitHub Dark, and Material
  Palenight — alongside the existing Dark Glass (default), Dark+, Light+,
  Monokai, Solarized Dark and High Contrast.

## [PowerRustCOBOL 1.15.0] — 2026-06-10

IDE: selectable colour themes + per-project background image, and a real fix for
form edits not reflecting in the Main Pane.

### New features

- **IDE colour themes (VSCode-inspired).** A new **Appearance** dialog (the ⚙
  button on the toolbar) lets you pick a colour theme. Six themes ship:
  **Dark Glass** (the default — identical to the previous look), **Dark+**,
  **Light+**, **Monokai**, **Solarized Dark**, and **High Contrast**. The theme
  drives the whole IDE chrome *and* the COBOL editor's syntax colours. The choice
  is saved **per project** (`cobolt.toml` → `[ide] theme`). New `theme` module
  (`crate::theme`): a flat `Theme` palette + registry; `apply_glass_visuals` and
  the editor's syntax layouter both read it.
- **Per-project background image with opacity (transparency) control** — just like
  the RAD form designer. In the same Appearance dialog you can browse for an image
  and set its opacity (0–100 %); it is painted behind the translucent glass panels
  of the main IDE window, scaled to cover. Stored per project
  (`[ide] background_image` + `background_opacity`). `IdeSettings` added to the
  project model with serde defaults so existing projects upgrade transparently.

### Fixed

- **Form property changes now reflect in the Main Pane.** The inline
  form/control inspector loaded the form once and never refreshed, so edits made
  (and saved) in the Designer window — or any external write of the `.cfrm` —
  were not shown when you returned to the Main Pane. The inspector now
  **live-reloads from disk on modification-time change** (preserving the selected
  control), so saving a form anywhere is reflected immediately. (Regression test:
  `inspect_refresh_tests`.)

## [PowerRustCOBOL 1.14.0] — 2026-06-10

IDE: controlled project tree, read-only generated code, richer toolbar.

### New features

- **Controlled project treeview** with five fixed, IDE-owned top categories —
  **Forms · Common Code · Generated Code · Assets · Documentation** — each with a
  professional icon. The four developer categories have a `[+]` to add
  sub-entries; developers can only add files *within* a category, never create
  top nodes. (`Documentation` is a new category; `cobolt.toml` gains
  `documentation` + `generated` lists, loaded with serde defaults so existing
  projects upgrade transparently.)
- **The project itself is the tree root** (project name + version); the five
  categories nest under it. Category and file **icons are 80 % larger**, and
  everything **below level 3 is collapsed by default** (Project · Category · Item
  stay open).
- **Forms expand to their controls**, grouped by RAD toolbox category with
  **Non-Visual first** (then Common, Container, Data, Graphics, Menus, Charts,
  Dialogs). **Single-click a file** opens it in the **Main Pane** (formerly the
  editor area); **single-click a form** shows its properties inline, **double-click**
  opens the RAD designer.
- **Widget events in the tree.** A control with event handlers expands to an
  **Events** group; clicking an event opens the form's generated COBOL at that
  event's paragraph (read-only).
- **Selection highlight** — the clicked tree element is highlighted as selected.
- **Debug is gated on a Generated Code selection** — the Debug button is enabled
  only when a generated-code item is selected in the tree (debugging targets the
  RAD-generated backend), with an explanatory tooltip otherwise.
- **Inline property inspector in the Main Pane.** Clicking a form or one of its
  controls in the tree shows the **same properties pane as the RAD** in the Main
  Pane — edit parameters and they're saved back to the `.cfrm` without opening
  the designer (an "Open in Designer" button is offered for deeper edits). It
  **reuses the designer's `PropertiesPanel`** (and its `set_property`/
  `set_form_prop` logic) via a transient panel — no duplicated property code, no
  designer window.
- **Semaphore status dot** to the left of every tree element: **green** = tested/
  checked OK and unchanged, **yellow** = changed since the last check (or never
  tested), **red** = check found an error / failed. `do_check` sets green/red;
  editing a file (since its last check) flips it back to yellow; controls inherit
  their form's status.
- **Generated Code is its own read-only category.** Each form's RAD-generated
  COBOL (output of the form designer, one entry per form, named after it) lives
  under the **Generated Code** node — IDE-owned (no `[+]`), shown in blue with a
  🔒 badge, and opened **non-editable** in the editor (a flat-blue layout, never
  saved over) for review/debug only. Hand-written **Common Code** — the pure
  COBOL-85 modules `CALL`ed by forms — stays fully editable and contains no
  generated files.
- **Toolbar gains Build (binary), Run (interpreted) and Debug**, alongside Stop /
  Check / Open / Save.
- **Compile-gating**: Run / Debug / Build are enabled only when the project has
  at least one COBOL program (hand-written or generated) **or** at least one
  form; otherwise they're disabled with an explanatory tooltip.
- i18n: new keys for all five languages (categories, tree affordances, toolbar
  Build/Debug, the compile-gating tooltip).

### Design (not yet implemented)

- `docs/ide-collaboration-design.md` — the multi-developer collaboration design
  (Phase B): a **pluggable `SyncBackend`** (local-only · local git · GitHub ·
  Google Drive), pessimistic file-level locking (warn-once, read-only for the
  second developer, re-offer on release), change propagation, and a phased
  rollout starting from a trivial local backend. Design only — no code.

### Theme

- **Fonts are 50 % larger** (UI text styles and the code editor). The colour
  palette is unchanged (the dark glass theme is kept).

### Fixed

- **Form property changes now reflect in the IDE on save.** Saving a form (from
  the RAD designer or the inline Main-Pane inspector) refreshes the tree's cached
  form, **regenerates the backend COBOL** (so Generated Code reflects the change),
  keeps it tracked, and reloads any open generated editor tab.

### Tests

- `project_model` unit tests (category routing, generated detection incl. legacy
  stem-match, compile-gating). Full suite 414 passing.

## [PowerRustCOBOL 1.13.1] — 2026-06-10

Bug fix: `IF … ELSE …` sentence scoping (and `NEXT SENTENCE` with it).

### Fixed

- **A period-terminated `IF … ELSE …` (no `END-IF`) no longer absorbs the
  following sentences into the `ELSE` branch.** The parser now treats a period
  as a terminator of an `IF` branch, so subsequent sentences are siblings of the
  `IF`. This also fixes **`NEXT SENTENCE` inside an `IF … ELSE …`**, which had
  jumped one sentence too far (the statement after the IF was skipped). `NEXT
  SENTENCE` now lands correctly for both the period- and `END-IF`-terminated
  forms. (`crates/cobolt-parser/src/stmt.rs`: `parse_if`/`parse_stmts`.)

### Cleanup

- Removed dead `parse_recognized_noop` (its "UNLOCK/ALTER/RELEASE/RETURN no-op"
  comment was stale — all four are implemented). Renamed
  `parse_initialize_as_move` → `parse_initialize` and corrected its comment
  (INITIALIZE is fully implemented, not a MOVE-SPACES shortcut).

### Tests

- `test_control_flow`: NEXT SENTENCE in `IF … ELSE` (period and `END-IF`) and a
  plain `IF/ELSE` sentence-scoping regression. Full suite 410 passing.

## [PowerRustCOBOL 1.13.0] — 2026-06-10

INDEXED log rotation — keep each log file under 100 KiB.

### New feature

- **The INDEXED observability log now rotates** (logrotate/Grafana style). When
  the active `<assign-path>.log` approaches **100 KiB** it is renamed to
  **`<user|no-user>.<datafile>.log.<timestamp>`** and a fresh active log is
  started, so no single file grows without bound.
  - `<user>` is the `OPEN … WITH REGISTERED USER` value (sanitized for the
    filesystem); when the OPEN supplies no user, **`no-user`** is used in the
    rotated file name.
  - `<timestamp>` is a compact UTC stamp, e.g. `20260610T120230461Z`.
  - Rotated archives are complete, parseable logs; the runtime never deletes
    them (prune/ship them with your log pipeline).

### Tests & docs

- `indexed_log` unit tests for rotation (active stays under the cap; rotated file
  named with the user, and `no-user` when absent). Verified end-to-end via
  `rcrun` (a 700-commit run rotates at 512 lines, active stays ~38 KiB). Full
  suite 407 passing.
- `docs/observability.md` §1.2 documents rotation.

## [PowerRustCOBOL 1.12.0] — 2026-06-10

`OPEN … WITH REGISTERED USER` — record the operator in the INDEXED log.

### New language feature

- **`OPEN {INPUT|OUTPUT|I-O|EXTEND} file … WITH REGISTERED [USER] {literal |
  data-item}`** (PowerRustCOBOL extension). Since COBOL programs rarely sit
  behind an authentication engine, the operator/user is supplied explicitly on
  `OPEN`; it is recorded as a `user=` field on **every** event line of that
  file's session in the INDEXED observability log (`OPEN`/`COMMIT`/`ROLLBACK`/
  `CLOSE`). `USER` is optional; the value may be a string literal or a data item.
  Purely observational — no authentication/authorization, and no effect when the
  log is off.

### Docs & tests

- `docs/observability.md` §1.3.1 (the new clause + examples); the `user` field
  added to the field table. `docs/cobol85-supported-syntax.md` updated.
- Tests: parser (`open_with_registered_user_literal_and_data_item`) and an
  end-to-end interpreter+log assertion (`open_with_registered_user_appears_in_log`).
  Full suite 405 passing.

## [PowerRustCOBOL 1.11.0] — 2026-06-10

redb engine: read/write optimizations + an optional per-file transaction log.

### New features

- **Per-file INDEXED observability log** (redb engine). Enable with
  `rcrun --indexed-log <basic|full>` (`--indexed-log true` = `basic`) or
  `COBOL_INDEXED_LOG`. Each file gets a sidecar log at `<assign-path>.log`
  (e.g. `customers.idx` → `customers.idx.log`). One `key=value` line per
  transaction event (`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE`) records: ISO-8601 UTC
  timestamp, tx id, kind, write/rewrite/delete counts, records, bytes, duration,
  rec/s + bytes/s, and the **ordering quality** of the written keys
  (`order=ordered|unordered`, `in_order`/`out_of_order`). The `full` level also
  appends redb **index statistics** on `CLOSE` (tree height, leaf/branch/
  allocated pages, stored/fragmented bytes) — this walks the index, so it is
  opt-in. Logging is off by default and never affects program behavior.
- **Grafana/Loki-ready log formats.** `--indexed-log-format <text|json>`
  (`COBOL_INDEXED_LOG_FORMAT`) selects the line format. `text` is logfmt
  (Loki `| logfmt`); `json` emits **NDJSON** (Loki `| json`) with numeric metrics
  as bare JSON numbers so Grafana can graph them directly. Default `text`.

### Performance

- **READ NEXT** by the primary key of reference now returns the record straight
  from the range cursor (one B+tree descent per record instead of two) —
  ~17 µs/record sequential scan at 200 k.
- **WRITE** opens the `primary`/`alt` tables once per operation (was twice for
  the duplicate-check + insert). A micro-benchmark showed that caching the table
  handle *across* calls adds only ~8% over once-per-operation, so the simpler,
  `unsafe`-free single-open path was chosen; write cost is dominated by redb's
  ACID insert (~44 µs/record). Durability/crash-safety is unchanged.

### Docs & tests

- New `docs/observability.md` — the observability reference (starts with the
  INDEXED transaction log: flags, field table, formats, Grafana/Loki pipeline,
  cost/safety; plus `COBOLT_LOG` tracing and a roadmap).
- `docs/indexed-redb-engine.md` updated (optimizations; observability log now
  summarized with a pointer to `observability.md`).
- Tests: `indexed_log` unit tests (ISO timestamp, level parsing) and an
  end-to-end log assertion + sequential-scan timing in `test_indexed_redb.rs`.
  Full suite 400 passing.

## [PowerRustCOBOL 1.10.0] — 2026-06-05

Crash-safe INDEXED engine on a redb substrate (opt-in).

### New features

- **New `STORAGE IS DISK` engine for `ORGANIZATION IS INDEXED`**, built on
  **redb** (pure-Rust embedded ACID key-value store; copy-on-write B+tree, dual
  meta pages, per-page checksums). Opt-in via `--indexed-engine redb` or
  `COBOL_INDEXED_ENGINE=redb`; the default disk engine stays `PRCIDXD1`. It meets
  four operational goals the bespoke engine could not at scale:
  - **OPEN is O(1)** — only the meta page is read; no in-RAM record directory and
    no recovery scan, even after a crash (~5 ms to OPEN a 200 000-record file).
  - **RANDOM/NEXT reads** are B+tree / range operations over redb's page cache
    (~21 µs per random read at 200 000 records).
  - **Resident RAM = working set**, not record count (≥250 M records).
  - **Crash safety** — `COMMIT` is a durable redb transaction commit, `ROLLBACK`
    is an abort; a power loss can never leave a torn index.
- Behavioral parity with the default engine: the same versioned fixtures
  (`idx_crud` / `idx_persist` / `idx_tx`) run identically under redb (CRUD,
  primary + alternate `WITH DUPLICATES` in creation order, persistence,
  `COMMIT`/`ROLLBACK`), with matching file-status codes.
- Pure-Rust dependency (`redb`), no system library — consistent with the bundled
  SQLite / rustls philosophy.

### Docs & tests

- New guide: `docs/indexed-redb-engine.md` (goals, table layout, transaction
  model, parity, limits). Cross-referenced from `docs/indexed-file-internals.md`.
- Tests: `test_indexed_redb.rs` — the three fixtures under redb + direct
  `IndexedStore` checks + an `#[ignore]`d scale smoke test. Full suite 397 passing.

### Notes

- Bulk `WRITE` throughput (~20 k rec/s in one transaction) is a one-time load
  cost; OPEN, reads, and crash-safety are unaffected. Faster bulk loading is a
  tracked future optimization. Promoting redb to the disk default is deferred
  until it has more mileage.

## [PowerRustCOBOL 1.9.0] — 2026-06-05

PostgreSQL and MySQL support for the database runtime.

### New features

- **The SQL database runtime now speaks three backends** — SQLite,
  **PostgreSQL**, and **MySQL** — behind one unchanged CALL surface
  (`COBOL-OPEN-DB` / `COBOL-EXEC-SQL` / `COBOL-FETCH-ROW` / `COBOL-NEXT-ROW` /
  `COBOL-ROW-COUNT` / `COBOL-CLOSE-DB`). The engine is selected from the
  connection string's scheme:
  - `:memory:` / `sqlite:<path>` / bare path → **SQLite** (bundled)
  - `postgres://…` / `postgresql://…` → **PostgreSQL** (`postgres`, sync)
  - `mysql://…` → **MySQL** (`mysql`, rustls)
  - A COBOL program is portable across all three — only the connection string
    literal changes.
- All values are normalised to text uniformly across backends (NULL → spaces,
  integers/reals as digits, dates as `YYYY-MM-DD[ HH:MM:SS]`), so existing
  `COBOL-FETCH-ROW` code is unaffected.
- **Pure-Rust drivers** — both new backends build with no system library
  (`libpq`/`libmysqlclient`) and no OpenSSL; MySQL uses rustls.
- Form-designer **SqlDatabase** control: the `Driver` property now labels
  generated comments as SQLite / PostgreSQL / MySQL (routing stays by
  connection string).

### Docs & tests

- New guide: `docs/database-runtime.md` (connection strings, CALL reference,
  value normalisation, transactions, TLS notes, testing).
- Tests: connection-string routing + value normalisation + in-memory SQLite CRUD
  (`db_runtime` unit tests, `test_sql.rs`), plus opt-in `#[ignore]`d live
  PostgreSQL/MySQL round-trips (`PRC_TEST_PG_URL` / `PRC_TEST_MYSQL_URL`).

### Notes

- The synchronous PostgreSQL driver connects without TLS (`NoTls`); see
  `docs/database-runtime.md` for the recommended TLS approach. The COBOL
  `COMMIT`/`ROLLBACK` verbs remain INDEXED-file transactions — use
  `COBOL-EXEC-SQL` with `BEGIN`/`COMMIT`/`ROLLBACK` for SQL.

## [PowerRustCOBOL 1.8.0] — 2026-06-05

Program-controlled `COMMIT` / `ROLLBACK` transactions for INDEXED files.

### New language features

- **`COMMIT` and `ROLLBACK`** are now real COBOL verbs (reserved keyword tokens,
  so a preceding `DISPLAY` no longer absorbs them). They apply to **every** open
  INDEXED file in the run unit:
  - `OPEN` begins a transaction; `COMMIT` makes all changes durable and starts a
    new one; `ROLLBACK` undoes every `WRITE`/`REWRITE`/`DELETE` since the last
    `COMMIT`/`OPEN`; `CLOSE` persists (implicit commit).
  - The **memory engine**'s existing journal is now wired through.
  - The **disk engine** gained a real in-run **undo log** (Insert/Update/Delete
    inverses) — `ROLLBACK` was previously a no-op there.

### Notes

- This is *program-level* rollback; crash-recovery via a durable write-ahead log
  remains future work.
- New tests: `test_transactions` (disk + memory engines). Full suite: **382
  passed, 0 failed**.

## [PowerRustCOBOL 1.7.2] — 2026-06-05

File-sharing / locking phrases and `CANCEL` — previously parse errors or no-ops.

### New language features

- **`OPEN … [SHARING WITH {ALL OTHER | NO OTHER | READ ONLY}] [WITH LOCK]`** —
  parses and is honoured where meaningful (advisory in the single-run-unit model;
  no longer a parse error).
- **`READ … WITH [NO] LOCK` / `WITH KEPT LOCK`** — `WITH NO LOCK` releases the
  record lock the INDEXED engine takes under `I-O`.
- **`UNLOCK file [RECORD[S]]`** now releases the file's INDEXED record locks
  (new `IndexedStore::unlock`).
- **`CANCEL program …`** — was silently dropped at parse; now a real statement
  that re-initialises the named (nested) program's WORKING-STORAGE so the next
  `CALL` starts fresh.

### Notes

- New tests: `test_file_locking` (lock flow + CANCEL) and parser cases in
  `test_statements`. Full suite: **378 passed, 0 failed**.

## [PowerRustCOBOL 1.7.1] — 2026-06-05

Completes the previously recognized-but-no-op `ACCEPT` register sources.

### New language features

- **`ACCEPT … FROM COMMAND-LINE`** — the whole command line (arguments joined).
- **`ACCEPT … FROM ARGUMENT-NUMBER`** — the count of command-line arguments;
  **`DISPLAY n UPON ARGUMENT-NUMBER`** sets the argument pointer, and
  **`ACCEPT … FROM ARGUMENT-VALUE`** returns the argument at that pointer.
- **`ACCEPT … FROM ENVIRONMENT-VALUE`** — the value of the variable named by
  **`DISPLAY "name" UPON ENVIRONMENT-NAME`** (paired registers).
- **`ACCEPT … FROM ESCAPE KEY`** → `"00"`, **`FROM CRT STATUS`** → `"0000"`.
- The CLI passes a program's own arguments through (`rcrun run prog.cbl a b c`),
  and a compiled binary uses its real `argv`.

### Notes

- New test: `test_accept_sources`. Full suite: **373 passed, 0 failed**.

## [PowerRustCOBOL 1.7.0] — 2026-06-04

Avoid-list clearance: the remaining ⚠️/❌ items in the RustCOBOL-85 Supported
Syntax Reference are now implemented. The COBOL-85 verb/clause set is fully
covered. The IDE is unchanged.

### New language features

- **Identifier-object abbreviated conditions** — `a = b OR c` (where `c` is a
  data item) is resolved at runtime via the 88-level metadata (new
  `Condition::NameOrAbbrev`): a known condition-name evaluates as one, otherwise
  it is the abbreviation object `a = c`.
- **`INITIALIZE … REPLACING {ALPHABETIC|ALPHANUMERIC|NUMERIC|…-EDITED} [DATA] BY
  value`** — sets each subordinate item of that category; others untouched.
- **`66 RENAMES item-1 [THRU item-2]`** — a regrouping alias; reads synthesize
  the concatenated value, writes distribute by field width.
- **Pointers** — `USAGE POINTER`; `SET ptr TO {ADDRESS OF id | NULL | ptr2}`;
  `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` (aliases `id` onto the
  target's storage — reads **and** writes follow it); `IF ptr = NULL`.
- **`ALTER para-1 TO [PROCEED TO] para-2`** redirects para-1's `GO TO`;
  **`UNLOCK file`** is a real statement (no-op in the auto-unlock model).
- **Faithful `NEXT SENTENCE`** — was never actually parsed; now recognized and
  it transfers control past the next sentence boundary (synthetic markers).
- **Remaining standard intrinsics** — `PRESENT-VALUE` (completes the COBOL-85
  set) plus `YEAR-TO-YYYY`, `BYTE-LENGTH`/`LENGTH-AN`, `NUMVAL-F`, `TEST-NUMVAL`.
- **Extended screen `ACCEPT`/`DISPLAY`** — `DISPLAY … AT {nnnn | LINE n COLUMN n}
  [WITH HIGHLIGHT|REVERSE-VIDEO|UNDERLINE]` and `ACCEPT … AT …` execute via ANSI
  cursor positioning + SGR in CLI mode (ignored in GUI mode — the designer
  supersedes SCREEN I/O there).

### Notes

- New tests: `test_pointers`, plus cases in `test_conditions`, `test_initialize`,
  `test_control_flow`, `test_intrinsics_date`, and `test_statements`. Full suite:
  **371 passed, 0 failed**.

## [PowerRustCOBOL 1.6.0] — 2026-06-04

A COBOL-85 verb-completeness pass: closing every remaining ⚠️/❌ item in the
RustCOBOL-85 Supported Syntax Reference. The IDE is unchanged.

### New language features

- **Multi-receiver `MULTIPLY`/`DIVIDE GIVING` + per-receiver `ROUNDED`** —
  `MULTIPLY a BY b GIVING r1 [ROUNDED] r2 …`, `DIVIDE … GIVING q1 [ROUNDED] q2 …
  [REMAINDER r]`, and per-receiver `ROUNDED` on `ADD`/`SUBTRACT`. (Also fixes
  `MULTIPLY a BY b` with no GIVING to store into `b`.)
- **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** via control-flow
  signals; plain `EXIT` is now a no-op return point and `EXIT PROGRAM` returns to
  the caller (both were wrongly `STOP RUN`).
- **`CALL … NOT ON EXCEPTION`** — the body now runs when the call resolves.
- **`INSPECT … TALLYING … REPLACING`** combined (the REPLACING half was dropped)
  and **`BEFORE/AFTER INITIAL`** region qualifiers on every TALLYING/REPLACING
  phrase; TALLYING now accumulates onto its counter.
- **Date / financial intrinsics** — `INTEGER-OF-DATE`, `DATE-OF-INTEGER`,
  `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `FRACTION-PART`, `ANNUITY` (were `0`).
- **Literal-object abbreviated conditions** — `A = 1 OR 2 OR 3` reuses the
  subject and operator.
- **`EVALUATE … ALSO`** multi-subject (positional AND matching) and **`WHEN NOT`**.
- **Real 88-level condition-names** — the host item is tested against the
  declared VALUEs/ranges, and `SET 88-name TO TRUE/FALSE` writes a satisfying /
  violating value to the host (previously a bogus standalone slot).
- **`PERFORM para VARYING …`** now executes the named paragraph each iteration.
- **Functional `SORT` / `MERGE`** — `RELEASE`/`RETURN`, `USING`/`GIVING`, and
  `INPUT`/`OUTPUT PROCEDURE`, with stable sort by ASCENDING/DESCENDING keys.

### Notes

- `UNLOCK` and `ALTER` remain recognized no-ops (correct for the auto-unlock
  model; ALTER is deprecated). `66 RENAMES`, `INITIALIZE … REPLACING`, and
  identifier-object abbreviation remain unsupported (documented in the reference).
- New tests: `test_arith_receivers`, `test_control_flow`, `test_inspect`,
  `test_intrinsics_date`, `test_conditions`, `test_sort` (cobolt-runtime).

## [PowerRustCOBOL 1.5.0] — 2026-06-04

Hierarchical / occurrence-aware runtime environment. One dedicated effort
unblocks four interrelated COBOL-85 capabilities that the flat data store
previously could not express. The IDE is unchanged.

### New language features

- **Runtime table subscripting** — `TABLE-ITEM(i)` (and multi-dimension
  `T(i, j)`) now read and write per-occurrence storage slots, materialised
  lazily from the item's template on first write. Variable subscripts
  (`T(WS-I)`) are evaluated each access.
- **Qualified-name disambiguation** — `data-item OF group` / `… IN group`
  now resolves to the correct item when a leaf name is **declared in more than
  one group**. Duplicated names are stored under path-qualified canonical keys,
  so `BALANCE OF ACCOUNT` and `BALANCE OF SUMMARY` are independent fields
  (previously they collided into one slot). Unique names are unaffected.
- **`MOVE CORRESPONDING g1 TO g2`** — moves each subordinate item that the two
  groups share by name, recursing through matching sub-groups; items present in
  only one group are untouched.
- **`ADD CORRESPONDING g1 TO g2 [ROUNDED]`** and
  **`SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]`** — new
  `Stmt::AddCorresponding` / `Stmt::SubtractCorresponding`; combine each matching
  numeric pair, recursing through matching sub-groups.
- **Functional `SEARCH` / `SEARCH ALL`** — `Stmt::Search` now drives the table's
  index (the `VARYING` item, else its first `INDEXED BY` index) from its current
  value to the table bound, evaluating each `WHEN` per occurrence and running the
  first matching imperative, else the `AT END` body. `INDEXED BY` index-names are
  registered as numeric index registers (recognised by `SET` and the resolver).
- **`DISPLAY` of qualified & subscripted numerics** now renders with full PIC
  width (leading zeros), matching plain-item DISPLAY.

### Internal

- `CobolEnvironment` gains a per-item symbol table (`ItemSym`: OCCURS dims, child
  names + canonical child keys, ancestor path, INDEXED BY names) plus a
  duplicate-name index; `resolve_name()` maps a (name, qualifiers) reference to
  its canonical storage key.
- Tests: `crates/cobolt-runtime/tests/test_hierarchy.rs`.

## [PowerRustCOBOL 1.4.0] — 2026-06-04

A COBOL-85 language-coverage pass: closing parser/runtime gaps surfaced by the
verb test matrix. The IDE is unchanged.

### New language features

- **Reference modification** `data-item(start:[length])` — new `Expr::RefMod`,
  parsed on any operand (disambiguated from subscripts by the `:`), evaluated as
  a substring (sender) and as a spliced partial assignment (receiver).
- **`COMPUTE` multiple receivers + per-receiver `ROUNDED`** —
  `COMPUTE r1 [ROUNDED] r2 [ROUNDED] … = expr` (was single receiver, one flag).
- **Category-aware `INITIALIZE`** — new `Stmt::Initialize`; numeric / numeric-
  edited items reset to ZERO, everything else to SPACES, recursing into groups
  (was a blanket `MOVE SPACES`).
- **`STRING` / `UNSTRING … ON OVERFLOW` / `NOT ON OVERFLOW`** + the
  `END-STRING` / `END-UNSTRING` / `END-SEARCH` scope-terminator tokens (which also
  fixes `DISPLAY` greedily swallowing a following `END-*` word).
- **`SET idx {UP|DOWN} BY n`** (encoded as ADD / SUBTRACT).
- **Inline `PERFORM n TIMES … END-PERFORM`** (no paragraph).
- **Operator-prefixed abbreviated conditions** — `a > 1 AND < 9`, `a = 5 OR = 7`.
- **`CALL … ON EXCEPTION / ON OVERFLOW`** — the handler now runs when the called
  program is unresolved (was parsed and discarded).
- **Extended `ACCEPT` / `DISPLAY` screen forms recognized** — `AT nnnn`,
  `AT LINE n COLUMN n`, `WITH <attributes>`, and `ACCEPT FROM
  {ARGUMENT-NUMBER|ARGUMENT-VALUE|ENVIRONMENT-VALUE|ESCAPE KEY|CRT STATUS}` parse
  (not executed — SCREEN I/O is superseded by the designer).
- **`SEARCH` / `SEARCH ALL`, `RELEASE`, `RETURN`, `UNLOCK`, `ALTER`** are now
  recognized statements (parse as no-ops) instead of breaking the parse.
- **Intrinsic functions** expanded: `ORD`, `CHAR`, `ORD-MAX`, `ORD-MIN`, `SUM`,
  `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION`,
  `FACTORIAL`, `SIN`/`COS`/`TAN`/`ASIN`/`ACOS`/`ATAN`, `LOG`/`LOG10`,
  `EXP`/`EXP10`, `PI`, `STORED-CHAR-LENGTH`, `WHEN-COMPILED` (was: unknown
  functions returned 0).

### Known gaps (documented)

- `MOVE/ADD/SUBTRACT CORRESPONDING`, runtime **table subscript indexing**,
  **qualified-name disambiguation**, and **functional `SEARCH`** all await an
  occurrence-aware data model (the runtime store is currently flat).
- Multiple receivers on `MULTIPLY`/`DIVIDE`; per-receiver `ROUNDED` on
  `ADD`/`SUBTRACT`; `SET ADDRESS OF`; identifier-object abbreviated conditions.

### Docs

- New [`docs/cobol85-verb-test-matrix.md`](docs/cobol85-verb-test-matrix.md)
  (what to test) and [`docs/cobol85-supported-syntax.md`](docs/cobol85-supported-syntax.md)
  (the exact grammar RustCOBOL accepts, with an avoid-list). README updated.

## [PowerRustCOBOL 1.3.1] — 2026-06-04

File I/O fixes surfaced by the storage/compression File I/O test pack
(`tests/cobol/fileio/`), now run end-to-end in the suite.

### Fixes

- **Record `ORGANIZATION IS SEQUENTIAL` READ** — fixed-length records (no
  terminator) are now read one record (`record_len` bytes) per `READ`, dispatched
  by organization. Previously the reader used line reads for every sequential
  file, so the first `READ` of a record-sequential file consumed the whole file
  and subsequent reads hit EOF. (`interpreter.rs`)
- **Source is always free form.** `rcrun` no longer auto-detects fixed vs free;
  it treats source as free form (set `COBOLT_FIXED=1` to opt into fixed-form
  parsing). This keeps long `ASSIGN` paths / `DISPLAY` literals from being
  truncated at column 72.

### Grammar (final, lean)

- The INDEXED storage clause is **`STORAGE [MODE] IS MEMORY | DISK`** (`MODE`
  optional) and compression is **`WITH COMPRESSION`** — in the storage clause or
  as a standalone clause (which uses the default storage backend). The earlier
  `WITH COMPRESSION` spelling and other variations were removed to keep the
  grammar clean.

### Behaviour

- **Default storage is `DISK`.** When an INDEXED file has no `STORAGE` clause,
  it now uses the on-disk paged B+tree engine (was MEMORY). `STORAGE IS MEMORY`
  selects the in-RAM engine explicitly.
- Writing a record that creates a duplicate value on an `ALTERNATE RECORD KEY …
  WITH DUPLICATES` is now a fully successful `00` write (previously the
  informational `02`). `WITHOUT DUPLICATES` violations still return `22`.

### Tests

- The File I/O test pack is vendored under `tests/cobol/fileio/` (baseline
  `fileiot.cbl` + six storage/compression variants) and driven end-to-end by
  `crates/cobolt-runtime/tests/test_fileio_storage.rs` (ASSIGN paths redirected
  to a temp dir; the 1,000,000-record profile loop shrunk for speed — the
  original files keep the full 1M profile for manual `rcrun` benchmarking).
- The earlier `tests/cobol/indexed-files/` programs (idxbasic, idxstorage) were
  removed — the File I/O suite supersedes them with broader indexed coverage.
  Focused inline engine checks remain in `test_indexed.rs`.

## [PowerRustCOBOL 1.3.0] — 2026-06-04

INDEXED files gain a selectable storage backend and record compression.

### `STORAGE IS MEMORY | DISK` (new) + persistent on-disk B+tree

- **New SELECT clause** `STORAGE IS MEMORY | DISK [WITH COMPRESSION]`
  for INDEXED files (a PowerRustCOBOL extension). `ASSIGN TO` is still required —
  it is where the data is persisted. Parsed in `parse_file_control_entry`
  (`StorageMode` on `FileControl`); the parser now also recognises the spaced
  `ALTERNATE RECORD KEY … [WITH DUPLICATES]` form.
- **`MEMORY`** (default) — the existing in-RAM `BTreeMap` engine (whole file in
  memory, persisted to the `PRCIDX1` container on close).
- **`DISK`** — a new **persistent, paged on-disk B+tree engine**
  (`cobolt-runtime/src/indexed_disk.rs`, container `PRCIDXD1`): records and
  indexes live in the `ASSIGN` file and are read on demand, so RAM use is bounded
  by the page cache rather than the whole data set. Built from 4 KiB pages with
  a **free list** (freed pages reused), one **B+tree per key** (primary +
  alternates; variable byte-packed nodes, split on insert, doubly-linked leaves
  for `START` + `READ NEXT/PREVIOUS`), a **RecordId directory** (a record that
  moves on `REWRITE` only updates the directory, not every index), and **slotted
  data pages** with an overflow chain for oversized records. The full COBOL verb
  set works on it (`OPEN`/`WRITE`/`READ` random+sequential/`REWRITE`/`DELETE`/
  `START` with all key relations, `INVALID KEY`), with FILE STATUS 22/23/35/39.
  Index deletes are lazy (no node merge; data pages are reclaimed).
- Both backends share one `IndexedStore` trait, dispatched from
  `make_indexed_engine` by `STORAGE MODE`.

### `WITH COMPRESSION` (new)

- Optional `WITH COMPRESSION` compresses stored record data in **both**
  storage modes via a self-contained, **dependency-free** PackBits-style RLE
  (`cobolt-runtime/src/compress.rs`) chosen for maximum speed; a one-byte tag
  guarantees the output never grows. On the padded, fixed-length records typical
  of COBOL it compresses well past the 50 % target; incompressible blocks fall
  back to raw.

### Tests

- `compress.rs` (round-trip, ≥50 % on padded records, raw fallback, long runs),
  `indexed_disk.rs` (pager/free-list, B+tree splits over 2 000 records +
  persistence, all `START` relations, NEXT/PREVIOUS, alt keys with/without
  duplicates, REWRITE/DELETE, compression round-trip, status 35/39), and
  end-to-end COBOL `STORAGE IS DISK [WITH COMPRESSION]` programs in
  `tests/test_indexed.rs`.

## [PowerRustCOBOL 1.2.0] — 2026-06-03

A COBOL-85 language milestone: exact numeric arithmetic, numeric-edited
PICTUREs, `COPY`/`REPLACE` copybooks, and a full **INDEXED (ISAM) file engine**.
The IDE interface is unchanged; all generated COBOL source stays in English.

### Indexed (ISAM) files — new

- **Built-in keyed-file engine** (`cobolt-runtime/src/indexed.rs`) — a
  dependency-free ISAM store: primary `RECORD KEY` plus
  `ALTERNATE RECORD KEY [WITH DUPLICATES]`, records held in ascending key order,
  a journaled write log with `COMMIT` / `ROLLBACK`, and record locking. No
  external libraries.
- **Self-describing `PRCIDX1` container** — the on-disk format now embeds the
  full file schema (record format + every key's byte-ranged composite parts,
  encoding, ordering, duplicate policy, and COBOL field name) plus timestamps
  and a CRC-32 trailer, modelled on Fujitsu's `cobfa_indexinfo()` metadata so a
  future Fujitsu importer can write faithful files. The legacy records-only
  `PRCISAM1` container is still read (and upgraded to `PRCIDX1` on next write).
  - **Discovery API** `IndexedFile::inspect_path()` reads a file's schema
    (`IndexedFileInfo`) without opening it for I/O.
  - **Strict open-time validation**: declared `SELECT`/`FD` keys + record format
    are checked against the stored schema → FILE STATUS **39** on mismatch;
    `OPEN INPUT` of a missing file → **35**; corrupt container (CRC) → **90**.
  - Format documented in [`docs/indexed-file-format.md`](docs/indexed-file-format.md).
- **Verbs dispatched by `ORGANIZATION`.** `OPEN` / `CLOSE` / `READ` / `WRITE`
  are wired to each file's declared organization (from its `SELECT`), not a
  single hard-coded type, so SEQUENTIAL / LINE SEQUENTIAL / INDEXED share the
  common verbs while each keeps its own semantics. (`interpreter.rs`,
  `cobolt-runtime/src/files.rs` `RecordLayout` materialize/distribute.)
- **Indexed verb set executes**: `OPEN INPUT/OUTPUT/I-O/EXTEND`,
  `WRITE`, random `READ` by `RECORD KEY`, `READ … NEXT / PREVIOUS`
  (sequential), `REWRITE`, `DELETE`, and `START … KEY IS = / > / >= / < / <=`
  (incl. `GREATER/LESS THAN`, `NOT LESS THAN`).
- **`ACCESS MODE SEQUENTIAL / RANDOM / DYNAMIC`** now all execute (an
  unqualified `READ` is random under RANDOM/DYNAMIC; `NEXT/PREVIOUS` force
  sequential).
- **`INVALID KEY` / `NOT INVALID KEY`** phrases added to `READ`/`WRITE`/
  `REWRITE`/`DELETE`/`START`, alongside full **FILE STATUS** codes
  (00/02/10/22/23/…).
- **Selectable engine** — `rcrun --indexed-engine <rust|rm-cobol85|fujitsu>`
  (or `-I`) and the `COBOL_INDEXED_ENGINE` environment variable choose the ISAM
  engine. All engines are behaviour-compatible; `rust` is the default and
  `rm-cobol85` / `fujitsu` currently delegate to it pending their native
  container formats.
- Verified by the File I/O suite [`tests/cobol/fileio/`](tests/cobol/fileio/)
  plus `cobolt-runtime` integration and unit tests.

### Exact numeric arithmetic

- `ADD` / `SUBTRACT` / `MULTIPLY` / `DIVIDE` / `COMPUTE` run on an `i128`
  fixed-point mantissa (no `f64` round-trips): exact to 18-digit standard and
  31-digit extended precision, with `ROUNDED` (half away from zero) and
  `ON SIZE ERROR` / `NOT ON SIZE ERROR`. Decimal literals are carried exactly
  from the lexer. Numeric `DISPLAY` renders at full PIC width.
  Verified by [`tests/cobol/numeric-precision/numprec.cbl`](tests/cobol/numeric-precision/numprec.cbl).

### Numeric-edited PICTUREs

- Edit engine (`cobolt-runtime/src/numedit.rs`): `Z` suppression, `*`
  check-protection, fixed/floating `$` and `+`/`-`, `,`/`.` insertion,
  `B`/`0`/`/` insertion, and `CR`/`DB`, applied on `MOVE`/`DISPLAY` into an
  edited field.
- **`DECIMAL-POINT IS COMMA`** — comma decimal separator for literals and the
  swapped `.`/`,` roles in edited PICs.
  Verified by [`tests/cobol/numeric-edited-pic/`](tests/cobol/numeric-edited-pic/).

### COPY / REPLACE copybooks

- Preprocessor (`cobolt-runtime/src/copybook.rs`) expands
  `COPY name [OF lib] [REPLACING ==a== BY ==b== …]` (pseudo-text + word
  replacement), resolves copybooks beside the source, expands nested `COPY`
  recursively, and applies `REPLACE … BY …` / `REPLACE OFF`.
  Verified by [`tests/cobol/copy-replace/`](tests/cobol/copy-replace/).

### Tests

- `tests/cobol/` reorganized into per-purpose subfolders
  (`numeric-precision/`, `numeric-edited-pic/`, `copy-replace/`,
  `indexed-files/`).

## [PowerRustCOBOL 1.1.0] — 2026-06-01

### Form Designer & rendering

- **New control: Animator.** Plays animated images — **GIF, WebP and APNG** (and
  any still image) — decoded natively via the `image` crate (no external/FFmpeg
  dependency). Properties: `Source`, `AutoPlay`, `Loop`, `SizeMode`
  (Fit/Fill/Stretch/Center), back/border. Decoding + frame-timed egui playback
  live in the new shared `cobolt-media` crate, so the control animates in the
  designer canvas, the preview, the run-form **and** the compiled standalone
  binary. (MP4 support is planned via a native decoder behind the same API.)


- **System font picker** — the Font property is now a dropdown of the fonts
  installed on the machine (via `fontdb`), each name rendered **in its own
  font**. The list is virtualised, so only the families you actually scroll
  past are loaded. The chosen font **family and size** are now applied to the
  rendered text in the **designer canvas, preview window and run form**, with a
  graceful fallback to the built-in (Arial-like) proportional font when a family
  is Arial/default or unavailable on the target system. Bitmap-only faces (e.g.
  `GB18030 Bitmap`) that egui can't rasterise are rejected up-front, fixing a
  crash when scrolling the font list. (`cobolt-ide/src/fonts.rs`)

- **#69 — Resize the form canvas by dragging its border.** Right, bottom and
  bottom-right corner grips; live resize with grid snap and a minimum size.
  (`designer.rs`)

- **#70 — Double-click an event row to jump to its COBOL paragraph.** The
  generated `.cbl` is opened in the editor and scrolled to the paragraph (or
  `PROGRAM-ID`) definition. Single-click still opens the per-event modal editor.
  (`properties.rs`, `app.rs`, `editor.rs`; i18n key `hint_dblclick_event`)

- **#129 — Preview animations now apply `scale`.** Zoom/spin/flip animations
  resize controls in the preview window, via the shared
  `designer::scale_rect_about_center()` (also used by the canvas). (`app.rs`)

### Runtime / language

- **COBOL sequential file I/O — `ORGANIZATION IS SEQUENTIAL` and
  `LINE SEQUENTIAL`.** The ENVIRONMENT DIVISION's `FILE-CONTROL` is now parsed
  (`SELECT … ASSIGN TO … ORGANIZATION IS [LINE] SEQUENTIAL [ACCESS MODE …]
  [FILE STATUS IS …]`), and the runtime implements `OPEN INPUT/OUTPUT/EXTEND/I-O`,
  `WRITE record [FROM …]`, `READ file [INTO …] [AT END …] [NOT AT END …]`, and
  `CLOSE`, updating the FILE STATUS item (00/10/30/35/…). LINE SEQUENTIAL writes
  newline-terminated records (trailing spaces dropped); record SEQUENTIAL writes
  fixed-length records. `ASSIGN TO` accepts a literal path or a data item holding
  the path. `READ … AT END` accepts the two-word `AT END` / `NOT AT END` forms.
  (`cobolt-ast`, `cobolt-parser`, `cobolt-runtime`)

- **New built-in CALLs `COBOL-APPEND-FILE` / `COBOL-WRITE-FILE`** —
  `USING path text [status]` append a line to (or truncate+write) a text file.
  COBOL `OPEN/WRITE` file I/O is still unimplemented; these cover the common
  "write a results/log file" need. (`interpreter.rs`)

- **PICTURE repetition counts are now honored.** `analyze_pic` ignored `(n)`, so
  `PIC X(20)` held 1 char and `PIC 9(5)` had 1 digit. Templates are now expanded
  (`X(20)`→20, `9(7)V99`→7.2), and `PicClause.digits/decimals` widened to `u16`
  so wide fields like `PIC X(4096)` / `PIC X(32767)` are exact. (`cobolt-parser`,
  `cobolt-ast`)

- **Alphanumeric comparison pads with spaces.** `compare_values` compared raw
  strings, so a space-padded `PIC X(64)` field never equalled a short literal
  (e.g. `EVALUATE control-id WHEN "BTN-OK"` never matched). The shorter operand
  is now space-padded per COBOL rules. (`interpreter.rs`)

- **`STRING … DELIMITED BY SIZE` works.** The bare word `SIZE` lexes to the
  `SizeError` token (reserved for ON SIZE ERROR); the STRING parser now accepts
  it as the SIZE delimiter, so `STRING` no longer dropped all operands.
  (`cobolt-parser`)

### Compiler (standalone binary)

- **Richer Label rendering in the generated form app.** The compiled binary's
  Label now honors BackColor, ForeColor, FontSize, Bold/Italic/Underline/
  Strikethrough, TextAlign, WordWrap, Padding, Opacity, BorderStyle/BorderColor,
  Cursor (on hover), per-control geometry overrides (`X/Y/Width/Height`) and
  `Dock` from `COBOL-SET-PROPERTY`, plus a short input warm-up so a click already
  underway as the window opens can't trigger a control. (`cobolt-compiler`)

### Fixes

- Fixed a long-broken `cobolt-codegen` test target (ambiguous `.into()` in
  `Control::new` calls) and corrected stale form-event paragraph-name
  expectations (`MAIN-FORM--ONLOAD`, not `--ON-LOAD`).

- **Lexer — fixed-form identification area now stripped.** `flatten_fixed` /
  `preprocess_fixed` were slicing active source out to char-column 255 instead
  of 72, so anything a program placed in columns 73–80 (the identification area)
  leaked into the token stream. Now correctly cut at column 72. (`source.rs`)

- **Lexer — `END-PERFORM` is a scope-terminator keyword.** Corrected stale tests
  that asserted it should be an identifier; the keyword table and parser have
  always treated it as `Token::EndPerform` (like `END-IF` / `END-EVALUATE`).

- **Parser — sequential program units in one file are no longer dropped.**
  `parse_program` now collects sibling program units that follow the first
  program's `END PROGRAM` terminator (e.g. `OUTER. … END PROGRAM OUTER.` then
  `SET-RESULT. … END PROGRAM SET-RESULT.`) into `nested_programs`, so the runtime
  can `CALL` them. True nesting (inner units before the outer terminator, the
  codegen shape) is unchanged. Fixes all 6 `cobolt-runtime` nested-program tests.
  New regression tests in `cobolt-parser/tests/test_nested_programs.rs`.

### Tests

- Added unit/behavioural tests: `fonts::tests` (enumeration, fallback, on-demand
  load, bitmap rejection), `designer::form_resize_tests`,
  `designer::anim_behavior_tests::scale_rect_…`, and `editor::goto_tests`.
  `cargo test -p cobolt-ide` → 35 passing.

## [2.5.0] — 2026-05-30

### Phase 11 — Embed+Bundle Binary Compiler

Cobolt projects can now be compiled into a **single self-contained native
executable** with no source code included.  The output binary embeds the
compressed AST and all form files, then runs them through the existing
interpreter at launch.

#### New crate: `cobolt-compiler`

The core build pipeline lives in `crates/cobolt-compiler/src/lib.rs`:

1. **Load manifest** — reads `cobolt.toml`, resolves main source + additional
   sources + form files.
2. **Lex → parse → semantic** — validates all COBOL sources; aborts on any
   error so only correct programs are compiled.
3. **Serialize + compress** — the `Program` AST is serialised with `bincode`
   and deflate-compressed with `flate2` (best compression).  Typical savings:
   60–75% smaller than raw bincode.
4. **Generate build project** — writes a temporary Cargo project to
   `/tmp/cobolt-build-<name>/` containing:
   - `Cargo.toml` — depends on `cobolt-runtime`, `cobolt-forms`, `eframe`/`egui`
     via path references to the local workspace.
   - `src/main.rs` — embeds assets via `include_bytes!`, contains a lazy form
     dispatch table, and launches either a headless interpreter or an eframe
     form application depending on whether forms are present.
   - `assets/program.bin` — compressed AST.
   - `assets/forms/<ID>.cfrm` — raw form XML for each form.
5. **`cargo build --release`** — compiles the generated project to a native binary.
6. **Copy to `bin/`** — the executable is placed at
   `<project-root>/bin/<project-name>` (`bin/<name>.exe` on Windows) with
   executable permissions set on Unix.

New workspace dependencies: `bincode = "1"`, `flate2 = "1"`.

#### Lazy form loader

The generated binary contains a `static FORMS: &[(&str, &[u8])]` dispatch
table.  A form is only deserialised from its embedded bytes when first
requested by the running COBOL program, keeping startup time constant
regardless of how many forms the project contains.

#### `cobolt build` CLI command

```
cobolt build [cobolt.toml] [--quiet]
```

Calls `cobolt_compiler::build_project()` and prints a summary on success:

```
✅ Build complete!
   Binary : myapp/bin/myapp
   Sources: 3
   Forms  : 2
   AST    : 8 412 bytes (compressed)
```

#### IDE — 🔨 Build Binary menu item

`File → 🔨 Build Binary (bin/)` triggers `do_build_binary()`, which:
- Spawns the compiler on a background thread (IDE stays responsive).
- Shows a ⏳ spinner label while building.
- Prints the binary path and stats in the Output panel when done.
- Shows an error message if the build fails.

---

## [2.4.0] — 2026-05-30

### Phase 10 — REST Client Runtime

COBOL programs can now make real HTTP requests — GET, POST, PUT, DELETE — using
standard `CALL` statements handled entirely inside the interpreter.  No external
tools, FFI, or async runtime are required.

#### New dependency: `ureq` (`cobolt-runtime/Cargo.toml`)

`ureq = { version = "2", features = ["json"] }` — a minimal blocking HTTP
client with built-in TLS support.  No async executor is pulled in.

#### New: `HttpClient` (`cobolt-runtime/src/http_runtime.rs`)

`HttpClient` manages per-session HTTP state for the interpreter:

- `get(url) -> (body, status)` — HTTP GET; returns the response body and
  numeric status code.  On network failure status is `0`.
- `post(url, body) -> (body, status)` — HTTP POST; Content-Type defaults to
  `application/json` unless overridden by `set_header`.
- `put(url, body) -> (body, status)` — HTTP PUT with the same body semantics.
- `delete(url) -> (body, status)` — HTTP DELETE.
- `set_header(name, value)` — adds / overwrites a persistent header sent on
  every subsequent request.
- `clear_headers()` — removes all persistent headers.

All methods strip trailing COBOL spaces from URL and body arguments before
sending.

#### Updated: `Interpreter` — 6 HTTP built-in `CALL` handlers

An `http: HttpClient` field is now part of `Interpreter` (initialised in
`new()`, inherited by `new_with_debug_channels()`).  `exec_call()` handles:

| CALL name                  | Arguments (BY REFERENCE)                          |
|----------------------------|---------------------------------------------------|
| `COBOL-HTTP-GET`           | url-var, response-var, status-var                 |
| `COBOL-HTTP-POST`          | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-PUT`           | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-DELETE`        | url-var, response-var, status-var                 |
| `COBOL-HTTP-SET-HEADER`    | name-var, value-var                               |
| `COBOL-HTTP-CLEAR-HEADERS` | (no arguments)                                    |

`response-var` receives the full response body (truncated by the `PIC X(32767)`
declaration if needed).  `status-var` (PIC 9(4)) receives the HTTP status code.

#### Updated: Codegen REST stubs (`cobolt-codegen/src/lib.rs`)

The working-storage section for `RestClient` controls no longer uses INVOKE /
OO-style comments.  Generated variables are now:

```cobol
01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.
01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.
01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.
01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.
01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.
01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.
01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.
01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.
```

`write_rest_client_stubs()` now generates three CALL-based paragraphs per
RestClient control (replacing the `INVOKE`-based stubs):

- **`{ID}-GET`** — `CALL "COBOL-HTTP-GET"` with url, response, and status;
  dispatches to the response or error handler paragraph based on the status code.
- **`{ID}-POST`** — `CALL "COBOL-HTTP-POST"` with url, body, response, status.
- **`{ID}-PUT`** — `CALL "COBOL-HTTP-PUT"` with url, body, response, status.
- Response / error handler stub paragraphs are generated for each control.
- An optional `{ID}-SYNC-ITEMS` paragraph copies `WS-HTTP-RESPONSE` and
  `WS-HTTP-STATUS` into user-configured `ResponseDataItem` / `StatusDataItem`
  data fields.

---

## [2.3.0] — 2026-05-30

### Phase 9 — Project Packaging

Cobolt projects can now be bundled into a self-contained, runnable zip archive
both from the IDE and from the command line.

#### New: `cobolt package` CLI command (`cobolt-cli/src/main.rs`)

```
cobolt package [cobolt.toml] [--output path.zip]
```

- Reads a `cobolt.toml` project manifest (defaults to `./cobolt.toml`).
- Packs all tracked source files, forms, and assets with their relative paths
  preserved inside the archive.
- Generates a `run.sh` (Unix, executable) and `run.bat` (Windows) launcher
  so users can run the project without knowing `cobolt` CLI syntax.
- Generates a `README.txt` with installation instructions.
- If a `cobolt` / `cobolt.exe` binary is found next to the currently running
  executable it is automatically bundled, making the archive fully self-contained.
- `--output` / `-o` flag overrides the default output path (`<name>.zip`).
- Prints per-file progress, warnings for missing files, and a final summary.

New dependencies added to `cobolt-cli/Cargo.toml`:
`serde = { workspace = true }`, `toml = { workspace = true }`,
`zip = { version = "2", features = ["deflate"] }`.

#### New: `package_project()` (`cobolt-ide/src/project_model.rs`)

The same packaging logic is available as a library function consumed by the IDE:

- `package_project(project, project_dir, output_zip) -> Result<usize, ProjectError>`
  — packs all tracked files + launchers + README; returns the count of archived items.
- `find_cobolt_binary()` — looks for the runtime binary next to the IDE executable.

#### Updated: IDE — File → Package Project menu item

`CoboltApp::do_package_project()` wires the menu entry to `package_project()`:

- Opens a native Save dialog pre-filled with `<project-name>.zip`.
- Requires a project to be open; otherwise shows a helpful status message.
- Reports the file count and output path in the Output panel on success.

---

## [2.2.0] — 2026-05-30

### Phase 8 — Database Runtime Engine

COBOL programs can now open real SQLite databases, execute SQL, and iterate
over result sets — all from standard `CALL` statements.  No host-language
embedding or FFI required.

#### New dependency: `rusqlite` (`cobolt-runtime/Cargo.toml`)

`rusqlite = { version = "0.31", features = ["bundled"] }` — SQLite is compiled
in from source; no system library or external install is needed.

#### New: `DbConn` and `DbRegistry` (`cobolt-runtime/src/db_runtime.rs`)

`DbConn` wraps a `rusqlite::Connection` and a cached result-set cursor:

- `open(conn_str)` — accepts a bare file path, `sqlite:<path>`, or `:memory:`.
- `exec(sql)` — auto-detects `SELECT`/`WITH`/`PRAGMA` vs. DML.  SELECT results
  are cached as `Vec<Vec<String>>`; DML returns the affected-row count.
- `fetch_col(col)` — returns column `col` (1-based) of the current row.
- `next_row()` — advances the cursor; returns `false` when exhausted.
- `row_count()` / `is_exhausted()` — query result-set metadata.

`DbRegistry` manages all open connections for one interpreter instance as a
`HashMap<u32, DbConn>` keyed by integer *handle*:

- `open(conn_str) -> u32` — opens a connection and returns its handle.
- `exec(handle, sql)`, `fetch_col(handle, col)`, `next_row(handle)`,
  `row_count(handle)`, `is_exhausted(handle)`, `close(handle)`, `close_all()`.

#### Updated: `Interpreter` — 6 SQL built-in `CALL` handlers

A `db: DbRegistry` field is now part of `Interpreter`.  `exec_call()` handles
six new built-in names (matched case-insensitively):

| CALL name            | Arguments (BY REFERENCE)                                  |
|----------------------|-----------------------------------------------------------|
| `COBOL-OPEN-DB`      | conn-string, handle-var (PIC 9(9)), status-var (PIC X)    |
| `COBOL-EXEC-SQL`     | handle, query, row-count-var, status-var                  |
| `COBOL-FETCH-ROW`    | handle, col-index (1-based), dest-var, status-var         |
| `COBOL-NEXT-ROW`     | handle, more-flag-var (`Y`/`N`)                           |
| `COBOL-ROW-COUNT`    | handle, count-var                                         |
| `COBOL-CLOSE-DB`     | handle                                                    |

On interpreter shutdown (`send_debug_finished`) `db.close_all()` is called
to release all connections.

#### Updated: Codegen SQL stubs (`cobolt-codegen/src/lib.rs`)

Working-storage for `SqlDatabase` controls no longer uses `USAGE IS OBJECT`
items.  The generated variables are now:

```cobol
01 WS-{ID}-CONN-STRING   PIC X(512)   VALUE ':memory:'.
01 WS-{ID}-HANDLE        PIC 9(9)     VALUE 0.
01 WS-{ID}-STATUS        PIC X(512)   VALUE SPACES.
01 WS-SQL-QUERY           PIC X(4096)  VALUE SPACES.
01 WS-SQL-ERROR            PIC X(512)   VALUE SPACES.
01 WS-SQL-ROW-COUNT        PIC 9(9)     VALUE 0.
01 WS-SQL-COL-INDEX        PIC 9(4)     VALUE 1.
01 WS-SQL-CURRENT-VALUE    PIC X(512)   VALUE SPACES.
01 WS-SQL-MORE             PIC X(1)     VALUE 'N'.
```

`write_sql_stubs()` generates four CALL-based paragraphs per control:

- **`{ID}-CONNECT`** — `CALL "COBOL-OPEN-DB"` with conn-string, handle, status.
- **`{ID}-EXEC`** — `CALL "COBOL-EXEC-SQL"` with handle, query, row-count,
  status; initialises `WS-SQL-MORE` to `'Y'`.
- **`{ID}-FETCH-ALL`** — loops `PERFORM UNTIL WS-SQL-MORE = 'N'` calling
  `COBOL-FETCH-ROW` for each column index and `COBOL-NEXT-ROW` to advance.
- **`{ID}-CLOSE`** — `CALL "COBOL-CLOSE-DB"` with handle.

---

## [2.1.0] — 2026-05-30

### Phase 7 — Debugger

The IDE now has a full interactive debugger for COBOL programs.

#### New: `DebugCmd` and `DebugEvent` channel types (`cobolt-runtime/src/debugger.rs`)

Two typed enums cross the thread boundary between the IDE and the interpreter:

- **`DebugCmd`** — `Continue`, `StepOver`, `Pause` — sent from the IDE to the
  interpreter to control execution.
- **`DebugEvent`** — `Paused { line, col, paragraph, vars }`, `Resumed`,
  `Finished` — sent from the interpreter back to the IDE.
- **`Breakpoints`** (`Arc<Mutex<HashSet<u32>>>`) — a thread-safe shared set of
  active breakpoint line numbers, written by the IDE and read by the interpreter.

#### Updated: `Interpreter` — per-statement debug hook

`Interpreter::new_with_debug_channels()` is a new constructor that wires the
debug channels into the interpreter.  Before every statement `exec_stmts()` now
calls `debug_check()`, which:

1. Extracts the statement's source line via `Stmt::span()`.
2. Checks whether the line matches a breakpoint **or** `debug_stepping` is true
   (StepOver mode).
3. If a pause condition is met, sends `DebugEvent::Paused` with a complete
   variable snapshot (`CobolEnvironment::iter()` → `VarSnapshot` list) and
   **blocks** on `debug_cmd_rx.recv()` until the IDE sends `Continue` or
   `StepOver`.
4. An async `Pause` command is handled via a non-blocking `try_recv()` poll on
   every statement when not already paused.
5. `DebugEvent::Finished` is sent when `run()` exits normally or via STOP RUN.

`current_paragraph` is updated as each paragraph is entered, so the Paused event
always carries the correct paragraph name.

#### New: `DebugRunner` (`cobolt-ide/src/runner.rs`)

`DebugRunner` is a sister to `Runner` that manages one debug session:

- `start(file_name, source)` — runs the full lex → parse → semantic pipeline,
  then spawns `Interpreter::new_with_debug_channels()` in a background thread.
- `send_cmd(DebugCmd)` — forwards a step/continue/pause command to the thread.
- `drain_events() -> Vec<DebugEvent>` — collects pending debug events each frame.
- `drain_run() -> Vec<RunMsg>` — collects pending run messages (diagnostics,
  output, finished).
- `pub breakpoints: Breakpoints` — the IDE writes breakpoint lines here before
  calling `start()`; the shared pointer is passed directly to the interpreter.
- `stop()` — drops `cmd_tx` (which unblocks any `recv()` in the interpreter,
  causing `Err(_)` → `StopRun`), then joins the thread.

#### New: Debugger side panel (`cobolt-ide/src/panels/debugger.rs`)

`DebuggerPanel` renders in a resizable right-side panel while a debug session
is active:

- **Step toolbar** — ▶ Continue (F5), ⤵ Step Over (F10), ⏸ Pause.  Buttons
  are disabled when the interpreter is running (not paused).
- **Location indicator** — paragraph name and source line, with a colour-coded
  ● Running / ● Paused status indicator.
- **Variable watch table** — displays all `CobolEnvironment` data items as
  a two-column striped grid (name / value), searchable via a filter text box.

#### New: Breakpoint gutter in editor.rs

The code editor's line-number column is now a fully interactive breakpoint
gutter:

- **Click** any line number to toggle a red breakpoint circle (●) on that line.
- When the debugger pauses, a **yellow arrow (→)** and highlighted row mark the
  current execution line.
- `EditorPanel::breakpoints: HashMap<PathBuf, HashSet<u32>>` stores active
  breakpoints per file.
- `breakpoints_for(path)` returns the line set for a given file, used by
  `do_debug()` to initialise the shared `Breakpoints` before starting the session.

#### New: 🐛 Debug toolbar button and keyboard shortcuts

A secondary toolbar strip appears below the main toolbar:

- **🐛 Debug** — starts a debug session for the active file (disabled while a
  normal run is active).  Automatically syncs breakpoints from the editor gutter
  into `DebugRunner::breakpoints` before starting.
- **■ Stop Debug** — drops the command channel (graceful stop), resets the
  debugger panel, and clears the editor debug-line highlight.
- **F5** — Continue (while a session is active).
- **F10** — Step Over (while a session is active).

#### i18n additions (all 5 languages)

New keys: `panel_debugger`, `dbg_continue`, `dbg_step_over`, `dbg_pause`,
`dbg_stop`, `dbg_variables`, `dbg_filter_hint`, `dbg_debug`.

---

## [2.0.0] — 2026-05-29

### Phase 6 — Form Runtime Engine

Forms can now be **executed interactively** from inside the IDE.  Pressing the
new **▶ Run Form** button in the designer toolbar compiles the form's generated
COBOL and runs it in a live, interactive OS window — no external tools required.

#### New: `FormEvent` and `StateUpdate` channel types (`cobolt-runtime`)

`crates/cobolt-runtime/src/channels.rs` introduces two typed messages that cross
the thread boundary between the egui UI and the background interpreter:

- **`FormEvent`** — sent from the UI thread to the interpreter when the user
  interacts with a control (`click()`, `change()`, `got_focus()`, `lost_focus()`).
  A special `quit()` sentinel (`ctrl_id = "__QUIT__"`) is used to unblock and
  terminate the interpreter cleanly.
- **`StateUpdate`** — sent from the interpreter to the UI whenever
  `COBOL-SET-PROPERTY` executes, carrying `ctrl_id`, `prop`, and `value` so the
  UI can update the live control snapshot immediately.

#### Updated: `Interpreter` — GUI channel support

`Interpreter::new_with_channels()` is a new constructor that wires three
`mpsc` channels into the interpreter for GUI-mode execution:

- `event_rx: Receiver<FormEvent>` — **`COBOL-WAIT-EVENT`** now _blocks_ on this
  receiver instead of immediately setting `COBOL-QUIT = 1`, enabling a real COBOL
  event loop.  Receiving the quit sentinel sets `COBOL-QUIT = 1` and exits.
- `state_tx: Sender<StateUpdate>` — **`COBOL-SET-PROPERTY`** sends a
  `StateUpdate` through this channel in addition to writing to the ObjectRegistry,
  so property changes are reflected in the UI on the next frame.
- `display_tx: Sender<String>` — **`DISPLAY`** statements route their output
  through this channel instead of stdout when in GUI mode; the IDE output panel
  receives each line via `OutputPanel::push_line()`.

CLI-mode behaviour (channels `None`) is completely unchanged.

#### New: `FormRuntime` (`cobolt-ide`)

`crates/cobolt-ide/src/form_runtime.rs` manages one live COBOL form execution:

- `FormRuntime::launch(form, form_path)` — generates COBOL from the form model,
  lexes, parses, and runs semantic analysis, then spawns
  `Interpreter::new_with_channels()` in a background thread.  Returns `Err` if
  parse or semantic analysis fails, displaying the errors in the output panel.
- `send_event(FormEvent)` — forwards a UI event to the interpreter thread.
- `drain_state() -> bool` — drains all pending `StateUpdate` messages and applies
  them to the `ctrl_state` snapshot; returns `true` when the UI should repaint.
- `drain_display() -> Vec<String>` — collects all `DISPLAY` lines produced since
  the last frame.
- `is_running() -> bool` — checks whether the interpreter thread is still alive.
- `stop()` — sends the quit sentinel and joins the thread.
- `Drop` impl ensures `stop()` is always called when the runtime is released.

Two supporting types are also defined here:

- **`CtrlMeta`** — immutable snapshot of a control's type, rect, z-order, and
  animations (populated at launch and used only for rendering order).
- **`CtrlState`** — mutable per-control state (`props`, `visible`, `enabled`),
  updated in-place by `drain_state()`.

#### New: **▶ Run Form** / **■ Stop Form** toolbar button

The designer toolbar now shows a **▶ Run Form** button when the form is not
running, and a **■ Stop Form** button while a runtime is active for that form.

- **▶ Run Form** saves the form, calls `FormRuntime::launch()`, and adds the
  runtime to `CoboltApp::form_runtimes`.
- **■ Stop Form** calls `stop()` on the matching runtime and removes it from the
  list.
- Multiple forms can run simultaneously in separate windows.

#### New: live interactive form viewport (`show_running_form_window`)

Each running `FormRuntime` gets its own OS window via `show_viewport_immediate`.
Every frame:

1. `drain_display()` output is forwarded to the IDE output panel.
2. `drain_state()` applies property updates to the live snapshot.
3. Controls are rendered in `z_order` from `ctrl_state` — buttons, labels,
   text boxes, checkboxes, combo boxes, list boxes, sliders, progress bars, and
   image controls are all handled.
4. User interactions fire the corresponding `FormEvent` back to the interpreter
   (`Click`, `Change`, `GotFocus`, `LostFocus`).
5. Non-visual controls (Timer, AgentObject, SqlDatabase, RestClient) are skipped.
6. Closing the window sends `FormEvent::quit()`, which unblocks
   `COBOL-WAIT-EVENT` and terminates the interpreter thread cleanly.

`ctx.request_repaint()` is called every frame while any form runtime is active,
ensuring the UI stays responsive to interpreter-driven state changes.

#### Output panel — `push_line()`

`OutputPanel::push_line(impl Into<String>)` was added to accept plain DISPLAY
output routed from the form runtime engine, displayed in the same monospace
light-grey style as normal program output.

---

## [1.1.0] — 2026-05-29

### New features & fixes

#### Form Designer — Save-on-close guard

Closing a dirty form designer window (one with unsaved changes) now triggers a
**Save / Discard / Cancel** confirmation dialog instead of silently discarding work:

- When the user clicks the OS close button (×) on a designer viewport that has
  unsaved changes, `ViewportCommand::CancelClose` is sent back to the OS to
  prevent the window from disappearing immediately
- A centred modal dialog appears with three choices:
  - **💾 Save & Close** — saves the `.cfrm` file and regenerates the `.cbl` COBOL
    source, then closes the window
  - **🗑 Discard & Close** — closes the window without saving
  - **Cancel** — dismisses the dialog, leaving the designer open and unchanged
- Closing via the dialog's own × button is treated as Cancel
- Clean (non-dirty) windows still close immediately without prompting

#### Form Designer — Save always regenerates COBOL

The **💾 Save** button in the designer toolbar now saves the `.cfrm` form file
**and** regenerates the `.cbl` COBOL source in a single action, keeping both files
in sync at all times.  The hover tooltip reads "Save form and regenerate COBOL".

Previously, Save only wrote the `.cfrm`; the user had to click "⚙ Generate COBOL"
separately to update the COBOL output.

#### Form Designer — Cmd+S in the designer window

**Cmd+S** (or Ctrl+S on Windows/Linux) now works inside designer viewport windows,
triggering the same save + regenerate action as the toolbar button.  Previously
Cmd+S was only handled in the main code-editor window and had no effect when the
designer was focused.

#### Properties panel — SqlDatabase `AutoConnect` type fix

`AutoConnect` was being pushed as `PropValue::String("true"/"false")` instead of
`PropValue::Bool(true/false)`.  The checkbox read the value back via `as_bool()`,
which checks for the `Bool` variant, so toggling `AutoConnect` had no effect.
Fixed: `PropValue::Bool(v)` is now used.

#### Properties panel — SqlDatabase COBOL Data Items grid layout

The "SQL Database — COBOL Data Items" section used an `egui::Grid` with
`num_columns(2)` but each `text_row_hint` call adds only one cell (a horizontal
layout containing both label and field).  The cells were therefore shifted by half
a column, causing labels and text edits to land in the wrong positions.  Fixed by:

- Changing the grid to `num_columns(1)` (each item gets its own full-width row)
- Adding `ui.end_row()` after each of the five `text_row_hint` calls
  (ConnDataItem, ResultSetDataItem, ConnectPara, QueryCompletePara, ErrorPara)

The same missing `ui.end_row()` was also present for the `ConnectionString` row
inside the "SQL Database — Connection" grid; that is fixed too.

#### Format painter — geometry copy

**Copy Style / Paste Style** (🖌 Format Painter) now also copies the source
control's position and size (X, Y, Width, Height) to the target control.

- `FormatPainter::WaitingForTarget` gains a `src_rect: cobolt_forms::model::Rect`
  field that captures the source control's `rect` at copy time
- The paste step writes `tgt.rect = src_rect` alongside the visual style properties
  and animations, so the target control becomes an exact geometric and visual copy
  of the source

#### Dead code removal — `bind_event` / `set_event_code` wiring

Removed all remnants of the old inline-editor event wiring that was superseded by
the modal `EventEditorModal` in v1.0.0:

- `pub bind_event: Option<(String, String, String)>` field removed from
  `InspectorAction` (was always `None` after the modal refactor)
- `bind_event()` and `set_event_code()` methods removed from `DesignerPanel`
- The three-line `bind_event` dispatch block removed from `DesignerPanel::handle_drag`

#### Label word wrap

Labels whose `Caption` text exceeded the control width were bleeding outside the
control border.  Two bugs were fixed:

1. **Wrong `max_width`** — `LayoutJob::wrap.max_width` was not set, so egui laid
   out the text as a single infinite line
2. **Wrong anchor for centred text** — with `halign = Align::Center`,
   `painter.galley(pos, ...)` treats `pos` as the **top-centre** anchor, not
   top-left.  `text_pos.x` was being set to `rect.min.x` (left edge), shifting
   the entire text block half a control-width to the left.  Fixed to
   `rect.center().x`.

#### IntelliSense — selection on click and Tab

Three bugs prevented selecting an autocomplete suggestion:

1. **Popup dismissal race** — `else { self.ac.visible = false; }` ran on the same
   frame the user clicked a row (the click briefly steals `TextEdit` focus, making
   `cursor_range` return `None`); the popup vanished before the click was processed.
   Fixed by removing the `else` branch entirely — the popup is now only dismissed
   by an explicit selection or Escape.

2. **Click detection on `Frame` rows** — `row_resp.response.interact(Sense::click())`
   does not detect clicks on `egui::Frame` responses because frames only sense hover.
   Fixed by replacing with `ui.interact(rect, id, Sense::click())`.

3. **Char vs byte index mismatch** — `trigger_pos` is a char index returned by
   `word_before_cursor`, but it was used directly as a byte offset in
   `String::replace_range`, causing a panic or wrong replacement on non-ASCII input.
   Fixed by converting via `tab.content.char_indices().nth(self.ac.trigger_pos)`.

#### Pointing-hand cursor on clickable elements

All interactive elements that use custom interaction (not standard egui buttons or
selectable labels) now show the `PointingHand` cursor on hover:

- **Toolbox cells** — `ui.ctx().set_cursor_icon(CursorIcon::PointingHand)` on hover
- **Canvas controls** — pointer becomes a hand when hovering any placed control
- **Properties panel event rows** — `.on_hover_cursor(CursorIcon::PointingHand)`
  on both control-event and form-event rows
- **Autocomplete popup rows** — `.on_hover_cursor(CursorIcon::PointingHand)` via
  the `click_resp` interact result

---

## [1.0.0] — 2026-05-29

### Major — Nested-program architecture

This is the first major version bump.  The entire code generation and form storage
model has been redesigned: each event handler becomes a COBOL-85 nested
program; the `.cfrm` file is the single source of
truth; the generated `.cbl` is a build artifact the user never edits.

#### `.cfrm` file format (v1.0 — backward-compatible load)

Three new XML sections added to `.cfrm`:

- `<working-storage><![CDATA[...]]></working-storage>` — raw COBOL data declarations
  emitted verbatim into the outer program's WS; supports `GLOBAL` and `EXTERNAL`
  clauses for form-wide and cross-form data sharing
- `<form-events>` — `OnLoad` and `OnClose` lifecycle handlers stored as `<Event>`
  children with CDATA bodies
- `<deleted-controls>` — recycle bin: event code from deleted controls preserved
  here (never emitted into `.cbl`) so it can be restored later

`<Event>` elements now use start/end form with CDATA body for the user's COBOL
statements.  Old-format self-closing `<Event .../> ` tags still load correctly
(`code` will be empty).

#### Model changes (`cobolt-forms`)

- `EventBinding` gains `code: String` — raw COBOL statements for this handler
- `EventBinding::for_control(ctrl_id, event)` — auto-derives paragraph name as
  `"CTRL-ID--EVENT-NAME"` (double-hyphen separator)
- `EventBinding::has_code()`, `code_line_count()` — UI helpers
- `derive_paragraph_name(ctrl_id, event) -> String` — public utility function
- `Form` gains `user_ws_source: String`, `form_events: Vec<EventBinding>`,
  `deleted_code: Vec<DeletedControlCode>`
- `Form::new()` pre-populates `form_events` with empty `OnLoad` / `OnClose` stubs
- `Form::recycle_control(id, timestamp)` — moves event code to recycle bin before
  deleting; `restore_from_recycle(timestamp, target_id)` recovers it
- `Form::control_has_code(id)` — returns `[(event, line_count)]` for UI dialog
- `Control::ensure_event(event)` — idempotent event binding with auto-derived name
- `DeletedControlCode` struct — `control_id`, `deleted_at` (ISO timestamp), `events`

#### Properties panel (`cobolt-ide`)

- "Event Bindings" section replaced by read-only "Events" section showing `●`/`○`
  status dots and line counts per supported event; user directed to Code View to edit
- "COBOL Paragraphs" section removed from chart controls (superseded by Code View)
- `new_ev_name` / `new_ev_para` staging fields removed from `PropertiesPanel`

#### Code generation (`cobolt-codegen`) — Phase 2 complete

- `write_procedure_division()` fully rewritten to emit COBOL-85 nested-program structure
- Outer program (`COBOL-MAIN`) calls `CALL "MAIN-FORM--ON-LOAD"` / `CALL "MAIN-FORM--ON-CLOSE"` for lifecycle events; event loop dispatches to handlers via `CALL "BTN-OK--CLICK"` (not `PERFORM`)
- New `write_nested_programs()` iterates form-level events then per-control events and emits a nested program for each
- New `write_nested_program(prog_id, code, comment)` emits a self-contained `IDENTIFICATION … PROCEDURE … GOBACK. END PROGRAM name.` block; empty handlers get `CONTINUE.` with a TODO comment
- Outer program closes with `END PROGRAM <form-name>.`
- Tests updated: `generate_contains_nested_program`, `generate_contains_form_events_nested`, `generate_calls_on_load_nested`

#### Backward-compatibility removal (`cobolt-forms`)

- `Form::load_paragraph` and `Form::close_paragraph` fields removed
- `OwnedEvent::EventEmpty(String, String)` variant removed
- `load-paragraph` / `close-paragraph` attributes removed from XML save/load
- `backward_compat_empty_event_tag` test removed
- `PropertiesPanel` "On Load" / "On Close" paragraph text-edit rows removed
- `set_form_prop("LoadPara")` / `set_form_prop("ClosePara")` arms removed from designer
- Raw string delimiter in XML test changed from `r#"..."#` to `r##"..."##` (fix: `"#FFFFFF"` terminated the former prematurely)

#### IDE — Interactive event code editor (interim, Phase 5 preview)

- Events section in Properties panel replaced by a collapsible inline COBOL editor per event
- Each event row shows a `▸`/`▾` arrow, `●`/`○` code-presence dot, and line count
- Expanding a row shows the derived `PROGRAM-ID` hint and a 6-row monospace `TextEdit`
- Edits are propagated back to `EventBinding.code` via `InspectorAction::set_event_code`
- `#[derive(Default)]` added to `InspectorAction`; `set_event_code: Option<(String,String,String)>` field added

#### Toolbox icon size

- Icon buttons enlarged from 39 × 39 px to 49 × 49 px (+25 %)
- Top and left padding increased from 5 px to 10 px (+5 px each)

#### Parser — Phase 3: COBOL-85 nested program support

- `cobolt-lexer`: added `Token::End` for the bare word `"END"` (distinct from `END-IF`, `END-PERFORM`, etc.)
- `cobolt-ast/DataDecl`: added `is_global: bool` and `is_external: bool` fields
- `cobolt-ast/Program`: added `nested_programs: Vec<Program>` and `end_program_name: Option<String>` fields
- `cobolt-parser/data.rs`: `GLOBAL` and `EXTERNAL` clauses now set flags on `DataDecl` instead of being silently skipped; `Token::End` added to all stop-condition lists so data parsing halts before `END PROGRAM`
- `cobolt-parser/procedure.rs`: `Token::End` added to every stop condition in `parse_sections`, `parse_paragraphs_until_section`, `parse_paragraphs`, and the `parse_stmts` stop closures so paragraph/section collection halts before `END PROGRAM`
- `cobolt-parser/parser.rs`: `parse_program` delegates to new free function `parse_single_program`; after the `PROCEDURE DIVISION` the function loops collecting nested programs (each starting at `IDENTIFICATION`) and terminates on `END PROGRAM name.` or EOF; nested programs are stored in `Program::nested_programs`
- `cobolt-ast` tests updated with `is_global`, `is_external`, `nested_programs`, `end_program_name` fields

#### Runtime (`cobolt-runtime`) — Phase 4 complete

**`CobolEnvironment` scope management**

- `push_local_scope(items)` — inserts a nested program's own WORKING-STORAGE
  items into the shared env store and returns the list of keys that were newly
  added (items that already exist, e.g. GLOBAL names, are not overwritten)
- `pop_local_scope(keys)` — removes those keys on GOBACK, restoring the env
  to its pre-call state
- `global_items_from_data_division(data)` — collects all `is_global`-flagged
  data items from a DATA DIVISION; utility used internally by the registry builder

**`Interpreter` nested-program registry**

- New `NestedProgram` struct — holds `para_map`, `para_order`, and
  `local_items: Vec<(String, CobolValue)>` for one nested program
- New `nested_registry: HashMap<String, NestedProgram>` field on `Interpreter`
- `register_nested(prog, registry)` — free function that recursively registers a
  `Program` and all of its `nested_programs` into the registry (keyed by
  PROGRAM-ID, uppercase); called from `Interpreter::new()` at startup
- New `run_para_sequence(para_map, para_order)` method — executes a paragraph
  sequence from an explicit map (not `self.para_map`); handles GO TO within
  the nested program's own paragraph space; GOBACK propagated to caller

**`exec_call` dispatch**

- Added `_ if self.nested_registry.contains_key(&prog_name)` arm before the
  legacy flat-paragraph fallback
- On match: clones para_map + para_order + local_items out of registry (to
  avoid simultaneous mutable borrow), calls `push_local_scope`, runs
  `run_para_sequence`, calls `pop_local_scope` even on error
- GOBACK from a nested program is treated as a normal return (not an error)
- GLOBAL items from the outer program are naturally visible to nested programs
  because they live in the same `CobolEnvironment` store — no copying needed

**Tests** — `tests/test_nested_programs.rs`

- `call_nested_program_runs_and_returns` — CALL dispatches, nested program sets outer WS, returns
- `nested_local_ws_is_removed_after_goback` — local items do not persist after GOBACK
- `global_items_shared_with_nested_program` — GLOBAL WS mutations are visible in outer env
- `nested_program_internal_goto` — GO TO works within nested para_map; does not escape
- `multiple_nested_programs_dispatch_independently` — each CALL routes to the right program
- `nested_program_without_end_program_terminator` — unterminated last nested program still callable

#### IDE — modal event code editor — Phase 5 complete

The inline 6-row TextEdit in the Properties panel is replaced by a full-screen modal
editor.

- Clicking any event row (in either the control Properties or the Form Properties
  Events section) opens a centred `egui::Window` overlay
- The modal renders a read-only COBOL scaffold around two editable areas:
  - **WORKING-STORAGE SECTION** — local data items specific to this handler
    (e.g. `01 WS-MY-VAR PIC X(64) VALUE SPACES.`)
  - **PROCEDURE DIVISION body** — the user's COBOL statements
- Read-only scaffold lines are colour-coded (green for structural keywords, gray
  for division headers); editable areas use monospace 12pt with syntax hint text
- **Save** commits both `local_ws` and `code` to the model (dirty-flagged);
  **Cancel** discards changes and closes without writing
- A semi-transparent black overlay dims the canvas behind the modal
- `EventEditorModal` struct added to `designer.rs` with `ctrl_id`, `ctrl_display`,
  `event_name`, `program_id`, `ws_buf`, `proc_buf`, `orig_ws`, `orig_proc`, `saved`
- `DesignerPanel::open_event_modal(ctrl_id, event_name)` — opens the modal,
  pre-populating buffers from the model (or blank if the event has no binding yet)
- `DesignerPanel::save_event_handler(ctrl_id, event_name, ws, code)` — writes
  both buffers back into the form, for either control or form-level events
- `DesignerPanel::show_event_modal(ui)` — renders the modal; called at the end
  of `show()` so it floats above all other content

**Model** — `EventBinding` gains `local_ws: String` for per-handler WS declarations;
XML layer extended with `<LocalWS><![CDATA[...]]></LocalWS>` child element inside
`<Event>` (backward compatible: old files without `<LocalWS>` still load correctly);
codegen updated to emit `local_ws` content in the handler's WS section instead of a
placeholder comment.

**Properties panel**
- `selected_event` and `event_code_bufs` fields removed
- `InspectorAction::set_event_code` replaced by `open_event_editor: Option<(String, String)>`
  containing `(ctrl_id, event_name)`; empty `ctrl_id` = form-level event
- Form Properties section gains "⚡ Form Events" subsection with clickable `OnLoad` /
  `OnClose` rows that open the same modal

---

## [0.2.2] — 2026-05-29

### Fix — Chart SET-TABLE generates invalid COBOL when DataSource/DataCount not set

`write_chart_stubs()` used `.map().unwrap_or_else(fallback)` to default empty
DataSource / DataCount properties, but if the property exists as an empty string
`Some("")`, `unwrap_or_else` never fires.  The result was invalid generated COBOL:

```cobol
           MOVE         TO WS-LIN-13-SELECTED-IDX        *> missing source
           CALL "COBOL-CHART-SET-TABLE" USING "LIN-13"   *> missing args
```

Fix: added `.filter(|s| !s.is_empty())` before `unwrap_or_else` so empty strings
fall through to the placeholder-name fallback (`WS-<ID>-TABLE` / `WS-<ID>-COUNT`).
Generated code now compiles cleanly even when the chart has no data binding configured.

---

## [0.2.1] — 2026-05-29

### Fix — Runtime COBOL-* built-in calls not recognised (warn + infinite loop)

After task 64 renamed all generated identifiers from `COBOLT-*` to `COBOL-*`, the
cobolt interpreter's `match` still only recognised the old `COBOLT-WAIT-EVENT` /
`COBOLT-SET-PROPERTY` / `COBOLT-GET-PROPERTY` spellings.  Every generated form
program therefore hit `CALL to unknown program 'COBOL-WAIT-EVENT' — ignored` on
startup, and the event loop would spin forever in CLI mode.

Changes to `cobolt-runtime/src/interpreter.rs`:

- Added `"COBOL-INIT-FORM"` arm — no-op in CLI/non-GUI mode (suppress spurious warn)
- Renamed `"COBOLT-WAIT-EVENT"` → `"COBOL-WAIT-EVENT"` (old spelling kept as alias)
- **`COBOL-WAIT-EVENT` now sets `COBOL-QUIT = 1`** so the event loop exits cleanly
  in CLI mode instead of spinning until the process is killed
- Added `"COBOL-SET-PROPERTY"` / `"COBOL-GET-PROPERTY"` as primary spellings (old
  `COBOLT-*` aliases retained for backward compatibility)
- Added `"COBOL-CHART-SET-TABLE"`, `"COBOL-CHART-ADD-POINT"`, `"COBOL-CHART-CLEAR"`,
  `"COBOL-CHART-REFRESH"` stubs — log at DEBUG level in CLI mode, no warning

---

## [0.2.0] — 2026-05-29

### New feature — Rich chart controls

Six chart control types added to the Form Designer toolbox under a new **Charts**
category.  Charts are first-class form controls that participate in the full designer
workflow: placement on the canvas, property inspection, COBOL code generation, and
XML persistence.

**Control types added**

- `BarChart` — vertical bar chart; default size 320 × 220
- `LineChart` — line/trend chart; default size 320 × 220
- `PieChart` — pie chart; default size 240 × 240
- `AreaChart` — filled area chart; default size 320 × 220
- `ScatterChart` — scatter-plot chart; default size 320 × 220
- `DonutChart` — donut / ring chart; default size 240 × 240

**Data binding**

Charts accept data via two complementary mechanisms:

1. **COBOL table binding** — pass an existing WORKING-STORAGE table and its element
   count directly:
   ```cobol
   INVOKE CHART1 SET-TABLE USING WS-SALES-TABLE WS-SALES-COUNT
   ```
2. **Point-by-point accumulation**:
   ```cobol
   INVOKE CHART1 ADD-POINT USING 'January' WS-MONTHLY-TOTAL
   INVOKE CHART1 CLEAR
   INVOKE CHART1 REFRESH
   ```

**Properties inspector** — dedicated chart section covering:

- *Visual*: Title, ShowLegend, ShowGridLines, ShowTooltips, AnimateOnLoad,
  X-axis / Y-axis labels
- *Data Binding*: DataSource, DataCount, LabelField, ValueFields, SeriesLabels
- *Type-specific*: grouped/stacked bars, smooth/stepped lines, inner-radius for
  donut, log-scale Y axis, bubble size for scatter, fill-opacity for area
- *COBOL Paragraphs*: DataChanged event paragraph stub
- *INVOKE usage hint* displayed inline

**Designer canvas** — glass-styled chart previews rendered with sample data at
design time (bars, polylines, filled polygons, scatter dots, pie/donut fan slices).

**Code generation**

- `WORKING-STORAGE SECTION` — three items per chart:
  `WS-<ID>-SELECTED-IDX` (PIC 9(4)), `-SELECTED-LBL` (PIC X(64)),
  `-SELECTED-VAL` (PIC 9(12)V99)
- `PROCEDURE DIVISION` — four stub paragraphs per chart:
  `<ID>-SET-TABLE`, `<ID>-ADD-POINT`, `<ID>-CLEAR`, `<ID>-REFRESH`

**Toolbox** — hand-drawn vector icons for all six chart types; unique ID prefixes
(`BAR`, `LIN`, `PIE`, `ARE`, `SCT`, `DNT`).

---

## [0.1.0] — 2026-05-29

### New feature — Snap-to-grid toggle

- Added `snap_to_grid: bool` field to the `Form` model (default `true`); persisted
  as a `snap-to-grid` XML attribute in `.cfrm` files (backward-compatible: missing
  attribute defaults to `true`)
- `snap()` in the designer canvas is now dynamic — it takes `grid_px` and `enabled`
  parameters instead of using a hardcoded 4 px constant; all move/resize/place
  operations respect the per-form setting
- Added **"Snap to grid"** checkbox to the Grid section of Form Properties (sits
  directly below "Grid size"); checking/unchecking takes effect immediately for
  move, resize, and new-control placement
- Updated all `Form` struct literals in test/codegen code to include
  `snap_to_grid: true`

Versioning rules
- **PATCH** (`0.0.x`): bug fixes, polish, build corrections
- **MINOR** (`0.x.0`): new features — resets PATCH to 0
- **MAJOR** (`x.0.0`): any change to the interpreter — resets MINOR and PATCH to 0

---

## [0.0.1] — 2026-05-29  *(initial tagged release)*

### Foundation (pre-tag, post-parser)

All work below was completed before the 0.0.1 tag was applied.
It is catalogued here as the baseline feature set.

---

#### Runtime & Toolchain

- **cobolt-semantic** — semantic analysis crate scaffolded; identifier resolution and
  basic type checking
- **cobolt-runtime / interpreter** — tree-walking interpreter for all AST statement
  types including `Stmt::TryCatch` and `Stmt::Throw` (try/catch/finally semantics,
  `UserException` error variant, exception variable binding)
- **cobolt-stdlib** — standard-library crate with built-in COBOL helper functions
- **cobolt-cli** — command-line binary (`cobolt run <file>`) wrapping the interpreter
- **INVOKE keyword** — added `Token::Invoke` to the lexer and a pass-through
  `Stmt::Invoke` to the parser; codegen emits `INVOKE` correctly
- **PLAY / STOP animation verbs** — `PLAY ANIMATION` / `STOP ANIMATION` statements
  added to lexer and parser
- **TRY / CATCH EXCEPTION / FINALLY** — full exception-handling block added to
  lexer and parser; interpreter executes all three clauses with correct fall-through

---

#### IDE Shell (`cobolt-ide`)

- **eframe/egui shell** — main application window with liquid-glass translucent
  visuals, dark-navy palette, rounded controls, and frosted-glass panel fills
- **macOS dock icon** — programmatically generated 256×256 navy rounded-square
  with a blue "C" arc and terminal serifs
- **Code editor panel** — scrolling source editor, syntax-aware font (12 pt
  monospace), auto-completion stubs, search/replace with focus-restore fix
- **Output / console panel** — scrolling log for run output and diagnostics
- **Project system** — `cobolt.toml` project file, project explorer panel with
  grouped tree view (forms, sources, assets), new-project dialog
- **Run / stop** — background thread runner, real-time output streaming,
  diagnostic markers fed back into the editor
- **Keyboard shortcut handling** — Cmd/Ctrl+S save, Cmd/Ctrl+Z undo,
  Cmd/Ctrl+Shift+Z redo wired globally

---

#### Form Designer

- **cobolt-forms model** — `Form`, `Control`, `ControlRect`, `PropValue`,
  `Animation`, `AnimTrigger`, `AnimEasing`, `BgImageMode` data types;
  XML serialisation/deserialisation (`cobolt-forms/src/xml.rs`)
- **cobolt-codegen** — form-to-COBOL source generator; REST-API stub codegen;
  DataGrid CSV-export stubs; full PROCEDURE DIVISION with all control paragraphs
- **Multi-viewport designer windows** — each open `.cfrm` file gets its own OS
  window via `ctx.show_viewport_immediate`
- **Canvas** — pixel-accurate form canvas with dot grid (configurable density),
  drag-to-place, drag-to-move, rubber-band multi-select, snap-to-grid
- **Control types (29 total)**:
  Button, Label, TextBox, CheckBox, RadioButton, ComboBox, ListBox,
  NumericUpDown, DateTimePicker, GroupBox, Panel, TabControl, Splitter,
  DataGrid, TreeView, PictureBox, ProgressBar, Slider, Line, Shape,
  MenuBar, ToolBar, StatusBar, Timer, AgentObject, RestClient,
  SqlDatabase (non-visual), ModalWindow
- **Vector icon toolbox** — two-column icon grid with hand-drawn vector icons for
  every control type, collapsible categories, live search filter;
  buttons enlarged to 39 × 39 px with 5 px top/right padding
- **Properties inspector** — two-column table layout; universal properties
  (Name, Caption, Position, Size, Font, Colors, Opacity, Transparency, Enabled,
  Visible, Z-Order); per-type sections for every control type;
  `SqlDatabase` connection properties (driver, host, port, database, user,
  password, auto-connect, max connections); panel width capped at 320 px to
  prevent overflow
- **Forms list panel** — sidebar list of all `.cfrm` files in the project root,
  open-on-click
- **Undo / redo stack** — full snapshot-based undo/redo for all designer mutations
- **Alignment toolbar** — align left/right/top/bottom/center-H/center-V,
  bring-to-front/send-to-back, delete selected; double-height toolbar
- **Z-order** — per-control z_order field; `Bring to Front` / `Send to Back`
  commands; canvas renders controls in z-order
- **Multi-select** — rubber-band selection, Shift+click toggle, group move
- **Form background** — solid fill colour (hex picker), transparency slider (0–100 %),
  background image path + stretch/tile/center/fit display modes
- **Grid density** — grid size property (8/16/32 px) on the Form, adjustable in
  Form Properties
- **Animation system** — per-control animation list; properties: name, trigger
  (`OnFormLoad`, `OnClick`, `OnHover`), easing, direction, duration, delay,
  loop count; designer-time live preview with play/stop controls;
  `AnimState` struct tracks t, playing, forward, delay_remaining
- **Preview window** — live OS window (`with_transparent(true)`) showing the form
  with liquid-glass control rendering, per-control opacity/transparency, and
  `OnFormLoad` animations auto-started on open; glass visuals applied to preview
  viewport; main designer visuals restored every frame to prevent bleed-through
- **Delete key guard** — Delete/Backspace only removes selected controls when no
  text-input control has keyboard focus (`ctx.memory focused().is_none()`)
- **Target device presets** — "Target" dropdown in Form Properties with 24 device
  presets (iPhone, iPad, Apple Watch, Android phone/tablet/watch, custom);
  selecting a preset auto-sets form width × height
- **COBOL identifier rename** — `COBOLT-*` data-division identifiers renamed to
  `COBOL-*` throughout codegen and semantic crates

---

*Next version: increment PATCH for fixes, MINOR for new features,
MAJOR for interpreter changes.*

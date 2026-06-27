# Tasks — MenuControl revamp

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-06-26

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

---

- [x] **T1 — Add workspace dependencies** (R1, R6)
  - Files: `Cargo.toml` (workspace), `crates/cobolt-forms/Cargo.toml`
  - Do: Add `serde_yaml`, `hmac`, `sha2` to workspace `[dependencies]` and
    wire them into `cobolt-forms`.
  - Verify: `cargo check -p cobolt-forms` green.

- [x] **T2 — MenuDefinition data model + YAML persistence** (R1, R2, R3, R4, R6)
  - Files: `crates/cobolt-forms/src/menu.rs` (new),
    `crates/cobolt-forms/src/lib.rs`
  - Do: Create `menu.rs` with `MenuDefinition`, `MenuItem`, `MenuItemType`
    structs (all `Serialize`/`Deserialize`). Implement `load_menu(path)` (reads
    YAML, validates HMAC-SHA256, returns error on mismatch), `save_menu(path,
    &MenuDefinition)` (serialises, computes HMAC, writes file), and
    `validate_depth(max=3)`. HMAC key is a fixed `const` byte slice. Add
    `pub mod menu;` to `lib.rs`.
  - Verify: `cargo check -p cobolt-forms` + `cargo test -p cobolt-forms --lib`
    with unit tests: `round_trip_yaml`, `hmac_validates`, `depth_limit`.
    **Covers AC3, AC9.**

- [x] **T3 — Accelerator parsing** (R24)
  - Files: `crates/cobolt-forms/src/menu.rs`
  - Do: Add `parse_accelerator(s: &str) -> Option<(Modifiers, Key)>` that
    parses strings like `"Cmd+N"`, `"Shift+Ctrl+S"`, `"Alt+F4"`. Add
    `format_accelerator(mods, key) -> String` that renders platform-native
    symbols (`⌘` on macOS, `Ctrl` on others). Add unit tests.
  - Verify: `cargo test -p cobolt-forms --lib -- menu::tests::accelerator`
    green. **Covers AC5 (parsing half).**

- [x] **T4 — MenuBar model changes** (R13, R28)
  - Files: `crates/cobolt-forms/src/model.rs`,
    `crates/cobolt-forms/src/xml.rs`
  - Do: In `Control::new` for `ControlType::MenuBar`: remove `Items` default
    property; add `HighlightBgColor` (#4488FF), `HighlightFgColor` (#FFFFFF),
    `SelectedBgColor` (#3366CC), `SelectedFgColor` (#FFFFFF). Update
    `supported_events` for MenuBar to `["onMenuClick", "onMenuOpen",
    "onMenuClose"]`. In `seed_missing_props`: seed the four colour properties
    on existing MenuBar controls. Keep `Items` for ToolBar/StatusBar.
  - Verify: `cargo check -p cobolt-forms` green. Existing ToolBar/StatusBar
    tests unaffected. **Covers AC10 (model side).**

- [x] **T5 — Vector icon catalogue (100+ icons)** (R29, R30)
  - Files: `crates/cobolt-forms/src/icons.rs` (new),
    `crates/cobolt-forms/src/lib.rs`
  - Do: Create a dedicated `icons.rs` module with `pub fn draw_menu_icon(painter,
    rect, icon_name, color)` and `pub const MENU_ICON_NAMES: &[&str]`. Implement
    100+ hand-drawn vector icons covering common commercial/business application
    needs. Each icon: 10–20 lines of painter path/stroke calls, following the
    existing `nv_icon_*` pattern. Categories and minimum set:
    - **Document** (10): `doc-new`, `doc-open`, `doc-save`, `doc-save-as`,
      `doc-copy`, `doc-blank`, `doc-text`, `doc-pdf`, `doc-spreadsheet`, `doc-stack`
    - **Edit** (12): `scissors`, `clipboard-copy`, `clipboard-paste`, `pencil`,
      `eraser`, `pen`, `brush`, `type-text`, `bold`, `italic`, `underline`,
      `strikethrough`
    - **Navigation** (10): `arrow-left`, `arrow-right`, `arrow-up`, `arrow-down`,
      `chevron-left`, `chevron-right`, `chevron-up`, `chevron-down`, `home`,
      `external-link`
    - **Action** (12): `plus`, `minus`, `check`, `x-mark`, `refresh`, `sync`,
      `download`, `upload`, `share`, `export`, `import`, `link`
    - **UI/View** (10): `eye`, `eye-off`, `magnifier`, `zoom-in`, `zoom-out`,
      `fullscreen`, `collapse`, `expand`, `grid-view`, `list-view`
    - **Communication** (10): `mail`, `mail-open`, `send`, `inbox`, `chat`,
      `phone`, `video`, `bell`, `bell-off`, `at-sign`
    - **Social** (6): `heart`, `star`, `thumbs-up`, `thumbs-down`, `bookmark`,
      `flag`
    - **People/User** (6): `user`, `users`, `user-plus`, `user-minus`,
      `user-check`, `user-circle`
    - **Media** (8): `play`, `pause`, `stop`, `skip-forward`, `skip-back`,
      `volume`, `volume-off`, `music`
    - **Data** (8): `database`, `chart-bar`, `chart-line`, `chart-pie`,
      `table`, `filter`, `sort-asc`, `sort-desc`
    - **System** (10): `gear`, `wrench`, `shield`, `lock`, `unlock`, `key`,
      `terminal`, `code`, `bug`, `cpu`
    - **Status** (8): `info-circle`, `warning-triangle`, `error-circle`,
      `help-circle`, `check-circle`, `x-circle`, `clock`, `calendar`
    - **Commerce** (6): `cart`, `credit-card`, `wallet`, `receipt`, `tag`,
      `percent`
    - **File/Folder** (6): `folder`, `folder-open`, `folder-plus`, `archive`,
      `trash`, `printer`
  - Verify: `cargo check -p cobolt-forms` green. **Covers AC6 (drawing half).**

- [x] **T6 — Designer canvas rendering** (R15, R16)
  - Files: `crates/cobolt-forms/src/paint.rs`,
    `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Update the `CT::MenuBar` branch in `draw_control` to: read a cached
    `MenuDefinition` from egui temp storage (set by the designer); render
    top-level labels horizontally in the glass bar; fallback "☰ MenuBar
    (empty)" if no definition. In `designer.rs`: on form open, if a
    `<menu-id>.menu.yaml` exists beside the `.cfrm`, load it and store in
    egui temp. After tree editor save, refresh the cache.
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → drop MenuBar →
    see "☰ MenuBar (empty)". **Covers AC1 (empty state).**

- [x] **T7 — Properties panel: MenuBar section** (R7, R13, R14)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`
  - Do: Replace the MenuBar branch in `show_type_specific` (currently shares
    the ToolBar/StatusBar "Items" section). New MenuBar section: "Edit Menu..."
    button (sets `action.open_menu_editor = Some(id)`), then colour property
    rows for `HighlightBgColor`, `HighlightFgColor`, `SelectedBgColor`,
    `SelectedFgColor`. ToolBar/StatusBar keep their existing "Items" section.
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → select MenuBar →
    properties shows "Edit Menu..." button + 4 colour rows. **Covers AC8
    (property side).**

- [x] **T8 — i18n: tree editor & property strings** (AC11)
  - Files: `crates/cobolt-ide/src/i18n.rs`
  - Do: Add ~25 `Tr` fields with translations in all 6 languages: tree editor
    title, Add Item / Add Sub-menu / Add Separator / Delete / Move Up / Move
    Down / Save / Cancel, detail labels (Label / Icon / Accelerator / Action /
    Action Target / Enabled), property labels (Highlight Bg/Fg, Selected
    Bg/Fg), error messages (hash mismatch, max depth exceeded), "Edit Menu..."
    button text.
  - Verify: `cargo check -p cobolt-ide` green. **Covers AC11.**

- [x] **T9 — Menu tree editor modal** (R7, R8, R9, R10, R11, R12)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Add `MenuEditorModal` struct (holds a cloned `MenuDefinition`, selected
    node path, and the source `.cfrm` directory for save). Add
    `show_menu_editor()` method following the `show_event_modal` pattern: dim
    overlay, `egui::Window` at 70% screen, left pane with collapsible tree
    (icon preview, label, accel, action summary), toolbar buttons (Add Item,
    Add Sub-menu, Add Separator, Delete, Move Up, Move Down), right pane with
    detail form (label TextEdit, icon ComboBox from `MENU_ICON_NAMES`,
    accelerator TextEdit, action type ComboBox, action target TextEdit, enabled
    Checkbox). Save button calls `save_menu` and refreshes the designer cache.
    Cancel discards. Wire `action.open_menu_editor` to create the modal.
    Enforce 3-level max on Add Sub-menu.
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → drop MenuBar →
    click "Edit Menu..." → modal opens → add File > New, Save, separator,
    Exit → Save → "File" shows on canvas bar → YAML file created on disk.
    **Covers AC1, AC2, AC3.**

- [x] **T10 — Runtime: pulldown dropdown rendering** (R17, R18, R19, R20, R23, R25)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: In the `CT::MenuBar` interactive/runtime branch: load the
    `MenuDefinition` (from the form's directory, validate HMAC). For each
    top-level label, render a clickable region. On click, open an `egui::Area`
    (Order::Foreground) anchored below. Each dropdown row: icon via
    `draw_menu_icon` (16×16), label, right-aligned accelerator text (formatted
    via `format_accelerator`). Separator items: `ui.separator()`. Hover:
    highlight with `HighlightBgColor`/`HighlightFgColor`. Disabled items:
    dimmed alpha, no hover/click. Click outside: close all open areas.
    Store open-menu state in egui temp data.
  - Verify: `cargo check -p cobolt-ide` green. Run preview → click menu label
    → dropdown opens, hover highlights, separators visible, disabled items
    dimmed, click outside closes. **Covers AC4, AC6, AC8.**

- [x] **T11 — Runtime: sub-menu cascading** (R21)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: When a dropdown item has non-empty `items`, render a "▸" indicator on
    the right. On hover, open a second `egui::Area` to the right of the parent
    dropdown. Support up to 3 levels (top bar + dropdown + sub-menu). Each
    sub-level uses a unique `egui::Id`.
  - Verify: `cargo check -p cobolt-ide` green. Run preview → hover over item
    with children → sub-menu opens to the right. **Covers AC2 (runtime
    rendering of 3 levels).**

- [x] **T12 — Runtime: action execution & events** (R5, R17, R22, R28)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: On action-item click: close all menus, then execute the action:
    `open-form:<name>` → push a form-open event; `property:<id>.<prop>=<val>`
    → push a property-set event; `close-application` → push a close event;
    `event` → no built-in action. Then push `UiEvent { ctrl_id: menu_id,
    event: "onMenuClick", value: Some(item_id) }`. Also push `onMenuOpen`
    when a dropdown opens and `onMenuClose` when all close.
  - Verify: `cargo check -p cobolt-ide` green. Run preview → click "Exit"
    (close-application) → app closes. Click "New" (event action) → onMenuClick
    fires. **Covers AC4 (action execution).**

- [x] **T13 — Runtime: accelerator key dispatch** (R24)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: On menu load, build a `HashMap<(Modifiers, Key), (item_id, action)>`
    from all enabled items with accelerators. Each frame, check
    `ui.input(|i| i.events)` for key presses matching the map. If matched and
    item is enabled, execute the action + fire onMenuClick (same as T12).
  - Verify: `cargo check -p cobolt-ide` green. Run preview → press Cmd+N →
    action fires without opening menu. **Covers AC5.**

- [x] **T14 — Programmatic SetItemEnabled / GetItemEnabled** (R26, R27)
  - Files: `crates/cobolt-forms/src/render.rs` (or runtime bridge)
  - Do: Handle `INVOKE <menu-id> 'SetItemEnabled' USING <item-id> <bool>`:
    find the `MenuItem` by id in the live `MenuDefinition`, set `enabled`.
    Handle `GetItemEnabled(<item-id>) -> bool`: return the `enabled` field.
    Wire through the form runtime's method dispatch.
  - Verify: `cargo check -p cobolt-ide` green. In a COBOL handler, invoke
    SetItemEnabled → item dims in the dropdown; invoke GetItemEnabled → returns
    correct value. **Covers AC7.**

- [x] **T15 — HMAC tamper detection at runtime** (R6)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: When the runtime loads a menu YAML and the HMAC validation fails, show
    an error in the menu bar ("⚠ Menu integrity error") instead of rendering
    items. Log the error. Do not execute any menu actions.
  - Verify: `cargo check -p cobolt-ide` green. Manually edit YAML → run → bar
    shows error message, no dropdown. **Covers AC9.**

- [x] **T16 — Docs: developers-guide-en.md MenuBar section**
  - Files: `docs/developers-guide-en.md`
  - Do: Add a MenuBar section covering: the tree editor workflow, YAML file
    format, action types (open-form, property, close-application, event),
    accelerator syntax, built-in icon catalogue, colour properties, COBOL
    programmatic API (SetItemEnabled / GetItemEnabled), and HMAC integrity.
  - Verify: Read the section, confirm it covers all user-facing features.
    Translations untouched.

- [x] **T17 — Finalize: version bump, full test, manual check**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: Bump patch version (`z`). Add CHANGELOG entry under Fix. Run full
    `cargo test`. Launch the IDE and walk through the manual verification
    steps from plan §6.
  - Verify: `cargo test` all green. Manual check: tree editor flow, runtime
    dropdown, sub-menus, accelerators, programmatic disable, tamper detection,
    ToolBar/StatusBar unchanged. **Covers all ACs.**

---

## AC ↔ Task mapping

| AC | Covered by |
|----|------------|
| AC1 | T6 (empty state), T9 (full flow) |
| AC2 | T9 (editor 3-level limit), T11 (runtime sub-menus) |
| AC3 | T2 (YAML format), T9 (editor save) |
| AC4 | T10 (dropdown rendering), T12 (action execution) |
| AC5 | T3 (parsing), T10 (display), T13 (key dispatch) |
| AC6 | T5 (icon drawing), T10 (icon in dropdown) |
| AC7 | T14 (programmatic API) |
| AC8 | T7 (property panel), T10 (highlight rendering) |
| AC9 | T2 (HMAC unit test), T15 (runtime detection) |
| AC10 | T4 (model split), T7 (properties split) |
| AC11 | T8 (i18n strings) |

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and the
change is split into fix commit(s) per the operator's rules (do **not**
commit/push unless the operator asks).

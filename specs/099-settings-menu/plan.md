# Plan — the Settings menu

- **Status:** implemented in 1.91.153   **Spec:** ./spec.md   **Date:** 2026-10-14

## Approach
A new module `crates/cobolt-ide/src/menu_layout.rs` declares the bar as data: `enum Menu { File, Settings, Run, View, Help }`, `enum Item { … one per entry … }`, `enum Entry { Item(Item), Separator }`, the three ordered slices `FILE`, `SETTINGS`, `HELP`, and `top_level(has_active) -> Vec<Menu>`. `app.rs` draws File, Settings and Help by walking those slices through one helper, `show_menu_entries`, whose `match` arms hold the code that drew each item, moved unchanged. Run and View keep their code; Run is skipped unless `menu_has_active`.

## Affected files
- `menu_layout.rs` (new) with its tests; `main.rs` (`mod`); `app.rs` (the menu bar and the helper); `i18n.rs` (`menu_settings` ×6 and the two strings that name a path ×6); `docs/developers-guide-en.md`, `README.md`, `docs/BUILDING-en.md`, `docs/cobol-support-matrix-en.md`, `docs/observability-en.md`, and the five `docs/BUILDING-<lang>.md` (deleted); `version.rs`, `CHANGELOG.md`.

## Decisions
- **Data, then draw:** the only way to test the bar without a window; it also makes "a configuration item added to Help later" a failing test instead of a surprise.
- **Move, don't rewrite:** each item's closure body is cut and pasted into its `match` arm; a diff that shows anything else is a mistake.

# Spec — the Settings menu: every configuration of the IDE in one place

- **Status:** implemented in 1.91.153 (the review gates of /specify, /plan and /tasks are taken together: the operator gave the layout in two messages and the details below follow from the source)
- **Folder:** specs/099-settings-menu/
- **Author:** operator request, written by Claude   **Date:** 2026-10-14
- **Branch:** `features`

## 1. Overview
The operator asked (2026-10-14) for a new menu **Configurações** — in the English UI **Settings** — right after File, holding every File and Help item that is about any kind of configuration, plus *Reindex Knowledge Bases*; and then fixed the bar: **"O menu básico fica: File, Settings, View, Help"**.

Facts the requirements rest on, read in the source on 2026-10-14:

1. The bar is File, Run, View, Help (`app.rs`, `egui::MenuBar`). Run and View sit inside `add_enabled_ui(menu_has_active, …)`: a project or a source file must be active.
2. **File** holds: New Project…, Open Project…, Save Project, Package Project…, **Configure Claude Code**, 🔨 Build Binary, —, **Reindex Knowledge Bases**, —, Save, —, Quit.
3. **Help** holds: 📖 Documentation, 📦 Examples ▸, —, **🐞 Debug Settings…**, **Coding Agent Settings**, **🧰 Platform SDK Location…**, 🎓 IDE Walkthrough seen, —, ℹ About.
4. There is **no** "Settings" entry anywhere in the menus today. "Settings" in the IDE is the *Project Settings* form of the main pane, opened by clicking the project at the top of the tree.
5. A `CoboltApp` cannot be built without an eframe window, so the menu structure cannot be tested where it is written.

## 2. Goals / Non-goals
- **Goals:** the bar reads **File, Settings, View, Help**; Run appears, between Settings and View, only while a project or a source file is active; the five configuration items live under Settings, exactly as they behaved; every document and IDE string that names an old path names the new one; the menu's structure is data a test can read.
- **Non-goals:** changing what any item does; moving *IDE Walkthrough seen* (a tour toggle, not a configuration — Q1); adding a *Project Settings* entry (Q3); the build-folder dialog (spec 100 adds it to this menu).

## 3. Requirements (EARS)
- **R1 (ubiquitous):** The menu bar shall read File, Settings, View, Help, in that order.
- **R2 (state):** While a project or a source file is active, the bar shall also show Run, between Settings and View; otherwise Run shall not be shown.
- **R3 (ubiquitous):** The Settings menu shall hold, in this order with a separator between groups: *Configure Claude Code*, *Coding Agent Settings* — *Debug Settings…*, *Platform SDK Location…* — *Reindex Knowledge Bases*.
- **R4 (ubiquitous):** File shall no longer hold Configure Claude Code or Reindex Knowledge Bases, and Help shall no longer hold Debug Settings…, Coding Agent Settings or Platform SDK Location…; neither shall be left with a leading, trailing or doubled separator.
- **R5 (ubiquitous):** Every moved item shall behave as it did: same action, same enabled state, same hover text, and Reindex Knowledge Bases shall still show its busy label while the sync runs.
- **R6 (ubiquitous):** The menu's name shall exist in the six languages: Settings · Configuración · Configurações · Paramètres · 設定 · 设置.
- **R7 (ubiquitous):** Every IDE string and every document that names an old path (`Help ▸ Debug Settings`, `Help ▸ Coding Agent Settings`, `Help ▸ Platform SDK Location`, `File ▸ Configure Claude Code`, `File ▸ Reindex Knowledge Bases`) shall name the new one.
- **R8 (ubiquitous):** The bar, and the items of File, Settings and Help, shall be declared as data in one module, so a test can assert their order and membership.

## 4. Acceptance criteria
- [x] **AC1 (R1, R2)** — `top_level(false)` is File, Settings, View, Help; `top_level(true)` is File, Settings, Run, View, Help.
- [x] **AC2 (R3, R4, R8)** — Settings holds exactly the five items in the order of R3; no configuration item is in File or Help; no item is in two menus; no menu starts or ends with a separator or holds two in a row.
- [x] **AC3 (R6)** — the name is non-empty in six languages and the six differ.
- [x] **AC4 (R5)** — the code that draws each item is the code that drew it before, moved, not rewritten (reviewed in the diff); the IDE builds and its whole suite passes.
- [x] **AC5 (R7)** — no English document and no IDE string in the six languages still names an old path (`grep`).

## 5. Constraints & steering check
- **i18n ×6:** the menu name; the two IDE strings that name a path (six languages each).
- **Docs:** the English Guide, README, BUILDING, the support matrix, observability. `BUILDING` has five translations, which GOLDEN RULE #8 says to delete when its English changes; the Guide, the matrix and observability have none left.
- **Fix vs feature:** **feature**; `z` bump; CHANGELOG.

## 6. Open questions (each taken at the answer assumed here)
- **Q1 — IDE Walkthrough seen.** *Assumed:* stays in Help; it replays a tour, it configures nothing.
- **Q2 — Run.** *Assumed:* shown only while something is active, because the operator's "basic" bar omits it; View stays on the bar and greyed, as it is today.
- **Q3 — A *Project Settings* item.** *Assumed:* not added — the operator listed only File and Help items; say so and it is one line.

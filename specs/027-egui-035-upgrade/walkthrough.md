<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# T14 — Operator walkthrough (R2 / AC2): egui-035 vs main

Run the branch build (`git switch egui-035 && cargo run -p cobolt-ide`) side by
side with a `main` build. Tick each item when the branch matches main (or is
acceptably better, e.g. crisper text from the new font engine). Anything that
differs: note it here; I fix it and the list re-runs.

## Shell & chrome
- [ ] IDE opens; glass theme, background image, translucency as on main
- [ ] Menus: every top menu opens; submenus on hover; items act; no premature
      close (0.32 changed menu close behavior — watch for it)
- [ ] Toolbar: all buttons act; language selector switches all six languages
      (JA/ZH glyphs render — AC8)
- [ ] Panels: project tree, forms list, toolbox, output pane, properties —
      resize grips work, no self-inflation anywhere
- [ ] Theme switcher: a dark + a light + Classic theme look right

## Designer
- [ ] Designer window opens as its own OS window (multi-viewport)
- [ ] Drag-place controls; move/resize; multi-select align; undo/redo
- [ ] Rounded containers (GroupBox/Panel + CornerRadius): children clip to the
      arc; **no dark corner arcs** (the two fixed regressions stay fixed)
- [ ] Glass styles: Classic, Enhanced, Neumorphic panels render as on main
- [ ] Copy Style, control arrays, data-binding badges

## Forms runtime
- [ ] Preview window: opens, interacts, closes; corners clean
- [ ] Run Form: full app window runs, events fire, closes cleanly
- [ ] Build / Run / Debug / Check all regenerate generated COBOL (banner
      intact — steering contract)
- [ ] Debugger viewport: breakpoints, step, variable watch; window opens at
      sane size and only resizes on drag

## Editors & tools
- [ ] COBOL editor: syntax highlight, line-number gutter aligned, IntelliSense
      popup, breakpoint gutter
- [ ] Event editor modal; COBOL Structure editor
- [ ] Documentation viewer: opens, Markdown + Mermaid render, search, PDF print
- [ ] Indexed editor + grid browser
- [ ] Error modals (⛔): open at 800×450, hold size, Copy/Save/A±/OK work

## AI & agent access
- [ ] AI assistant pane: send a prompt, apply a change, error modal on failure
- [ ] Output console shows "Agent access (MCP/inspection) listening on
      127.0.0.1:5719" at startup; Settings → AI shows the port field
- [ ] Custom project font loads; a bitmap-only system font falls back to Arial
      instead of crashing (AC8)

## Sign-off
- [ ] All above ticked → AC2 satisfied; proceed to T16 merge gate.

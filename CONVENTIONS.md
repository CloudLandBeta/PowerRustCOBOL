<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Working Conventions

Operational rules every agent on this repo must follow. (Architecture and crate
layout live in `AGENTS.md`; this file is the do/don't list.)

## PRIME DIRECTIVE — this project is Rust, and Rust only

- **Never author or run code in any language other than Rust, for any
  purpose.** No Python, no shell scripts, no Node, no awk/sed programs — not
  in the repository, not as a "throwaway" helper in a scratch directory, not
  to inspect, diff, count, generate or bulk-edit anything, not even once.
- A mechanical, repetitive edit is **not** a licence to reach for a scripting
  language. Do it with the editing tools, however tedious, or — when it
  genuinely warrants automation — write it in Rust, as a test, an `examples/`
  binary or a small crate, so it lives in the language the rest of the project
  is reviewed in.
- Invoking the toolchain (`cargo …`, `git …`, `ls`, `grep`) is using a tool,
  not authoring code, and stays fine. Stringing those into a script — a
  heredoc, a `python3 - <<EOF`, a multi-statement pipeline written to do a
  job — is not.
- The only non-Rust code that legitimately exists here is what the product
  itself is made of: COBOL (the language served), and the markdown/TOML/XML
  that configure and document it.
- This rule outranks convenience and outranks speed. If a task looks like it
  needs a script, say so and ask.

## Versioning — `crates/cobolt-ide/src/version.rs` (`x.y.z`)

- **x** — a brand-new platform component (Web/WASM, Android, iOS, cloud backend). Reset y, z.
- **y** — new functionality within existing components (controls, properties, panels, toolbar actions, language features, built-in CALLs). Reset z.
- **z** — bug fixes, visual polish, performance, anything not user-visible-new.
- **Pre-production rule: treat EVERY change as a fix → bump `z`**, even features, until told otherwise.
- Every bump gets a top-of-file `CHANGELOG.md` entry, dated with the absolute date.

## Git

- **Do nothing irreversible/outward-facing unless explicitly asked.** "commit", "merge", "push", and "publish" are distinct — perform only what was requested.
- Commit messages **must end with**:
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`
- Stage only the files your change touches. Do **not** stage unrelated untracked files (e.g. `.agents/`, `AGENTS.md`).
- If making a PR, end the body with the Claude Code "Generated with" line.

## Documentation

- **GOLDEN RULE #3 — never edit the translated guides** `docs/developers-guide-*.md`.
  The user maintains translations. Edit **only** the English guide `docs/developers-guide-en.md`.

## Forum publishing — cobolforo.es (vBulletin 3.8.7)

- **GOLDEN RULE #4 — publish ONLY from `main`.** A release post is an invitation
  to download and try the build, and a reader must never have to work out which
  branch carries what. Nothing is announced until it is merged to `main`; if the
  work sits on `features` or `fixes`, merge first (asking when the merge is not
  already sanctioned) and publish afterwards. A post describing work that is not
  on `main` is wrong even when every word of it is accurate.

- The board is **windows-1252**. Post via the **native browser submit**, not a UTF-8 `fetch`,
  and keep the body **plain ASCII**, or accented characters mojibake.
- Subforums / prefixes:
  - `f=96` features — needs prefix `[Noticia]` (= *Información*)
  - `f=97` fixes — no prefix
  - `f=98` tests
  - Because all changes are treated as fixes → post to **`f=97`, no prefix**.
- **Sign every post**: `Anthropic Claude Codex Agent`.

## Build / test

- Forms engine: `cargo test -p cobolt-forms --features render` (keep green).
- IDE: `cargo build -p cobolt-ide`.
- `cobolt-forms` rendering requires the `render` feature flag.

## Rendering architecture (spec 017 — unified render engine)

- **One renderer for every surface.** The Form Designer canvas, live preview, running
  (interpreted) form, and compiled binary all render through `cobolt-forms`:
  - `render::render_form` — interactive surfaces (preview / run / compiled).
  - `render::render_faces` — the designer canvas (static faces + editor overlay on top).
  - Both wrap the single source-of-truth `paint::draw_control`.
- Keep **parity** between surfaces — see the test `engine_reference_form_parity_static_vs_faces`.
- egui (0.29) only clips **axis-aligned** rects: no rounded clip, no stencil, no mid-frame
  render-to-texture. Plan rounded-corner work around that limitation.

## UI conventions

- **Captions** appear only on **GroupBox** (editable top-left border legend) and **Label**;
  kept on Button/CheckBox/RadioButton. Never render a centered `<id>` placeholder on other controls.
- Container membership is by a control's **`parent`** field (set on drop via
  `containers::resolve_drop_target`), not geometry — a visually-overlapping sibling is **not** a child.
- **egui resizable panes:** never use `egui::TopBottomPanel::show_inside(...)`
  (or the equivalent `SidePanel::show_inside(...)`) for panes the user must resize.
  In egui 0.29, nested resizable panels renegotiate their parent rectangle every
  frame and can snap the pane back to its minimum size. Use a top-level panel,
  a manual splitter, or persisted explicit pane dimensions instead.

## Spec-driven workflow

- New features go through `specs/NNN-<slug>/` via the skills:
  `/specify → /clarify → /plan → /tasks → /analyze → /implement → /docsync`.

## Misc

- `/code-review ultra` (deprecated alias `/ultrareview`) is user-triggered, billed, and
  cloud-run — an agent cannot launch it.

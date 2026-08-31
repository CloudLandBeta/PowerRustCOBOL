<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Working Conventions

Operational rules every agent on this repo must follow. (Architecture and crate
layout live in `AGENTS.md`; this file is the do/don't list.)

## PRIME DIRECTIVE — the project is Rust and Rust only; the tools that edit it need not be

*Revised by operator ruling 2026-08-30. The earlier blanket ban — "never author
or run code in another language, for any purpose, not even once" — is no longer
in force; what replaced it is below.*

**What the rule protects: no foreign code or dependency ever ships.** Everything
in this repository is Rust — every committed line, every helper, every test,
every `examples/` binary. The only non-Rust code that legitimately *lives here*
is what the product itself is made of: COBOL (the language served), and the
markdown/TOML/XML that configure and document it, plus tracked tooling that
predates this ruling (`tools/check_bugs.sh`). **No new language is ever added to
the project.**

**What is allowed:** writing and running a **throwaway script in whatever
language fits** (Python, shell, Node, awk/sed, jq …) purely to **automate edits
to this repository's Rust sources when the edit is repetitive, tedious, or hard
to express in Rust alone** — a rename across 300 call sites, a mechanical
signature change, a bulk import rewrite, a census of what needs touching. The
script is a *power tool for the editing tools*, not a project artifact, and it
is disposable. Three conditions bind it; **all three are mandatory**, and
failing any one means doing the edit by hand instead.

1. **It lives outside the repository — always.** Create and run it anywhere
   *but* this repo and its subfolders; the session scratchpad and
   `~/Documents/PowerRustCOBOL-local-settings/` are the obvious homes. It reads
   and writes repo files by absolute path, but it is never *in* the tree, so it
   can never be staged, committed or shipped by accident. Writing one inside the
   repo "just for a second" is precisely the failure this condition prevents.
2. **Sweep before every commit and every push — for executable scripts, and
   nothing else.** Before `git commit` and before `git push`, scan the working
   tree for **untracked program source in a language other than Rust or
   COBOL**: `.py`, `.sh`, `.bash`, `.zsh`, `.js`, `.mjs`, `.cjs`, `.ts`, `.rb`,
   `.pl`, `.php`, `.lua`, `.awk`, `.jq`, `.ps1`, `.bat`, `.cmd`, and any
   untracked text file carrying a `#!` shebang or the executable bit. **Anything
   found is MOVED — never deleted — into**
   `~/Documents/PowerRustCOBOL-local-settings/quarantine` (create the folder if
   missing), and **every move is reported to the operator by name**. Deleting is
   never the answer: the *user code is sacred* rule applies here too, and a
   quarantined file may well be the operator's own. Commit only once the sweep
   comes back clean.

   **Two hard limits on the sweep — it exists to stop foreign code shipping,
   never to strip the project of files it needs:**
   - **Tracked files are never touched.** `git ls-files` is the authority on
     what legitimately belongs; a file already in the index is *in the project
     by decision* (`tools/check_bugs.sh` is exactly that case).
   - **Data, configuration, documentation and assets are never in scope, at any
     extension** — `.json`, `.toml`, `.yaml`/`.yml`, `.xml`, `.md`,
     `.csv`/`.tsv`, `.txt`, `.cfrm`, `.cidx`, `.data`, `.lock`, fonts, images,
     fixtures, test inputs, binary blobs. These are *inputs to* Rust and COBOL,
     not code in another language, and quarantining one breaks the build for no
     gain. **Ship-blocking foreign code is the target; a file a crate reads is
     not.** If a file is ambiguous, **leave it and ask** — a false positive here
     cripples the project, a false negative is caught at review.
3. **The output is reviewed as Rust.** A scripted pass is finished when
   `cargo build` and `cargo test` are green on every crate it touched **and**
   `git diff` has been *read*, not skimmed. A script that made 300 correct edits
   and one wrong one is a script that failed. If a pass cannot be verified that
   way, it does not get run.

Invoking the toolchain (`cargo …`, `git …`, `ls`, `grep`) was never in scope and
still is not — that is using a tool, not authoring code.

- **Corollary (unchanged): never run a `perl -pe 's/\x{..}/'`-style pass over a
  source file** — it re-encodes every non-ASCII byte in the file. Use the
  editing tools, and check `git diff --numstat` after any scripted edit.

## Versioning — `crates/cobolt-ide/src/version.rs` (`x.y.z`)

- **x** — a brand-new platform component (Web/WASM, Android, iOS, cloud backend). Reset y, z.
- **y** — new functionality within existing components (controls, properties, panels, toolbar actions, language features, built-in CALLs). Reset z.
- **z** — bug fixes, visual polish, performance, anything not user-visible-new.
- **Pre-production rule: treat EVERY change as a fix → bump `z`**, even features, until told otherwise.
- Every bump gets a top-of-file `CHANGELOG.md` entry, dated with the absolute date.

## Git

- **GOLDEN RULE #5 — branch by change type; `main` is never a workbench.**
  Two long-lived working branches carry all work:
  - `features` — new functionality.
  - `fixes` — bug corrections.

  Classify every new request *before* touching a file, `git checkout` the
  matching branch, and **merge from `main` immediately after the switch** so the
  work starts from the latest code — that merge comes before the first edit, not
  after it. Nothing is implemented on `main`. Finish whatever change is in
  flight before switching branches again. **Merge back into `main` only when
  explicitly asked**; committing and pushing the working branch needs no such
  request. A feature or fix is published only from `main`, and only after the
  merge, commit and push have all succeeded (GOLDEN RULE #4).

- **Do nothing irreversible/outward-facing unless explicitly asked.** "commit", "merge", "push", and "publish" are distinct — perform only what was requested.
- Commit messages **must end with**:
  `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`
- Stage only the files your change touches. Do **not** stage unrelated untracked files (e.g. `.agents/`, `AGENTS.md`).
- If making a PR, end the body with the Claude Code "Generated with" line.

## Documentation

- **GOLDEN RULE #3 — keep the Developer's Guide current.** After any new
  feature or any observable fix, update the guide in the same change.
- **GOLDEN RULE #8 — keep ALL documentation in ALL languages up to date.** The
  same change must carry the update into **every** supported language: **en, es,
  pt, fr, jp, cn**. Write the **English canonical first**, then carry the same
  delta into each translation (never translate from a translation). Files are
  named `<doc>-<lang>.md` beside the English one.
  - Keep in English inside every language: COBOL keywords, data-item and
    paragraph names, everything inside a `cobol` block, CLI commands/flags,
    paths, identifiers, property names, menu labels, and the product names
    RustCOBOL / PowerRustCOBOL / rcrun.
  - Never copy English into a translation file to make it "exist" — a file that
    claims to be a translation but is English is worse than a missing one.
  - **This reverses the former "never edit the translated guides" rule**
    (superseded 2026-08-20). Claude now writes the translations directly.

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
- **GOLDEN RULE — a window may NEVER resize itself.** A window changes size
  only because the developer dragged its grip. If it grows, shrinks or creeps
  on its own, that is a defect, not a layout quirk — and it is this project's
  most repeated one.

  **Never `egui::Window::…resizable(true)`** on a window whose content can ask
  for space (a `ScrollArea`, a `Grid`, wrapping text). egui then renegotiates
  the window rectangle against that content every frame: the content asks for
  what it was given plus its own margins, egui grants it, and the pair walk to
  the screen edge. Nothing about the drag is involved — it happens while the
  mouse is still.

  The pattern that holds, every time:

  1. **The window owns an explicit size** stored on the panel struct —
     `size: egui::Vec2`, seeded from a `DEFAULT_W`/`DEFAULT_H` constant.
  2. `.resizable(false).fixed_size(self.size)` — egui negotiates nothing.
  3. **Every child is laid out from that stored number**, never from
     `ui.available_width()`, `ui.available_height()` or `ui.max_rect()`. A
     child that measures the space it was handed is the feedback path; remove
     it and there is no loop left to run.
  4. **One custom grip** in its own `egui::Area` at `Order::Foreground`, pinned
     to the window's outer rect from `Window::show`'s response. It adds
     `response.drag_delta()` to `self.size`, clamped to MIN/MAX. It must be
     the *only* writer of that field — never read the size back from the
     window, or egui's rounding becomes a growth term of its own.
  5. Do **not** put the grip inside the content `Ui`: there it joins the
     layout, shifts the content, and the drag fights what it is sizing.

  `debug_settings.rs` uses `resizable(true)` and is stable **only** because its
  content is capped at a constant height and never asks for more. That is the
  exception, not the template — copying it onto a window with a live scroll
  area reintroduces the bug. `panels/leaderboard_modal.rs` is the reference
  implementation of the five points above.

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

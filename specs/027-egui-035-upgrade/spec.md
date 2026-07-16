<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — egui 0.35 platform upgrade (+ MCP agent access)

- **Status:** draft → approved
- **Folder:** specs/027-egui-035-upgrade/
- **Author:** Claude (agent), directed by Eslopes   **Date:** 2026-07-15

## 1. Overview

Upgrade the entire IDE/RAD UI stack from **egui/eframe 0.29** to **egui/eframe
0.35** (released 2026-06-25) on a dedicated long-lived branch **`egui-035`**.
The upgrade crosses two major breaking releases (0.33 "Plugin trait",
0.34 "More Ui, less Context" + skrifa font rendering) and unlocks 0.35's
headline capability: the **inspection protocol** and **`egui_mcp`**, an MCP
server that lets AI agents see and drive egui applications. For PowerRustCOBOL
this makes the IDE itself agent-operable — form generation, COBOL event-handler
authoring, and IDE workflows can be driven programmatically — while the newer
egui base is more stable than 0.29. This is a make-or-break platform change:
it must land with **zero feature loss** and no stubs.

## 2. Goals / Non-goals

- **Goals:**
  - The whole workspace builds and runs on egui-family **0.35** (egui, eframe,
    egui_extras, egui_glow, plus the matching egui_commonmark release).
  - Every existing IDE feature and behavior is preserved (adapting internals
    freely to the new APIs).
  - The IDE exposes an **MCP endpoint** (`egui_mcp` + inspection protocol) so
    agents can observe and operate the IDE.
  - Compiled/packaged COBOL applications and the `rcrun` runtime expose **no**
    MCP/inspection surface.
  - `main` keeps receiving fixes as usual; **every commit** that lands on
    `main` is replicated onto `egui-035` in the same session, adapted to 0.35
    behavior.
- **Non-goals:**
  - No visual redesign or new IDE features beyond the MCP exposure.
  - No changes to COBOL language, code generation, or runtime semantics.
  - No merge to `main` until every acceptance criterion passes (merge is its
    own gated event, not part of day-to-day work on the branch).
  - No MCP access to end-user (generated) applications — explicitly out of
    scope even as an option.

## 3. User stories

- As the **operator (Eslopes)**, I want the IDE on egui 0.35, so that the
  platform is stable, maintained, and ready for agent-era tooling.
- As an **AI agent** (the in-IDE assistant or an external client such as
  Claude), I want to see the IDE's widget tree and interact with it via MCP,
  so that I can generate forms and COBOL event handlers far more effectively
  than by emitting JSON operations blind.
- As a **COBOL developer**, I want every panel, designer, dialog, and runtime
  behavior I use today to keep working identically, so that the upgrade is
  invisible to my daily work.
- As the **operator**, I want fixes on `main` mirrored promptly into
  `egui-035`, so that the branch never rots while it matures.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The workspace shall build with egui, eframe,
  egui_extras, and egui_glow at version **0.35** (and a compatible
  egui_commonmark) in every crate that consumes them (`cobolt-ide`,
  `cobolt-forms` `render` feature, `cobolt-media`, `cobolt-cli`).
- **R2 (ubiquitous):** The system shall preserve all existing IDE
  functionality — panels, form designer (incl. multi-viewport designer
  windows), COBOL editor, event editor, output/log pane, debugger, project
  tree, file dialogs, form runtime, themes/glass styles, AI assistant, error
  modals — with behavior equivalent to the 0.29 baseline.
- **R3 (event):** When the IDE starts, it shall serve an MCP endpoint backed
  by `egui_mcp` / the 0.35 inspection protocol, through which an MCP client
  can enumerate the widget tree and perform interactions (click, type,
  focus) on the IDE.
- **R4 (constraint):** Compiled/packaged COBOL applications and `rcrun` shall
  not link, start, or expose any MCP or inspection-protocol surface.
- **R5 (constraint):** The system shall not add dependencies outside the
  official egui ecosystem; version bumps of already-present dependencies
  (including transitive ones such as winit and the skrifa font stack) are
  permitted.
- **R6 (state):** While the `egui-035` branch exists, every commit landing on
  `main` shall be replicated onto `egui-035` in the same working session,
  adapted to 0.35 APIs and behavior (not blind cherry-picks).
- **R7 (constraint):** Only this dedicated agent shall commit to `egui-035`.
  Before each work session the agent shall verify branch history; commits
  authored by anyone else shall be reverted back to the agent's latest commit
  (standing authorization from the operator, 2026-07-15).
- **R8 (constraint):** The system shall contain no stubs, placeholders, or
  blanket-suppressed deprecations: every 0.29-era API use is migrated to its
  0.35 equivalent with fully working code, grounded in analysis of the
  existing code and the egui changelogs — never guesswork. When genuinely
  stuck, the agent shall ask the operator for guidance rather than improvise.
- **R9 (ubiquitous):** All user-resizable windows/modals (error modals,
  debugger, resizable panes) shall retain the anti self-inflation contract:
  open at their seeded default size and change size only on a user drag,
  re-verified under 0.35's layout behavior.
- **R10 (ubiquitous):** Custom font loading/validation shall keep its
  guarantee of accepting exactly what epaint can render, updated to 0.35's
  font pipeline (skrifa replaced the 0.29-era parser in egui 0.34).
- **R11 (event):** When MCP-related user-facing UI is added (status
  indicator, settings), its strings shall be `Tr` fields translated in all
  six languages (EN/ES/PT/JA/ZH/FR).

## 5. Acceptance criteria

- [ ] AC1 — `cargo build` and `cargo test` pass for every touched crate on
  `egui-035`, with the egui-family pinned at 0.35 (R1).
- [ ] AC2 — Operator walkthrough checklist passes: each IDE surface listed in
  R2 is exercised on the branch build and behaves as on `main` (R2).
- [ ] AC3 — An external MCP client connects to the running IDE, lists the
  widget tree, and completes a scripted round-trip (open a form, place a
  control, open an event editor) driven only via MCP (R3).
- [ ] AC4 — A packaged demo app and `rcrun` are inspected (dependency tree +
  runtime listener check): no MCP/inspection surface exists (R4).
- [ ] AC5 — `cargo tree` diff vs `main` shows only egui-ecosystem additions
  and version bumps of pre-existing dependencies (R5).
- [ ] AC6 — Replication audit: every `main` commit since branch creation has a
  corresponding adapted commit on `egui-035` (log cross-reference) (R6).
- [ ] AC7 — Error modals and the debugger window open at their seeded sizes
  and hold them (no drift) through an idle observation period; user drag
  still resizes (R9).
- [ ] AC8 — IDE text renders correctly in all six UI languages (incl. JA/ZH
  glyphs) and a custom project font loads/validates as on 0.29 (R10, R11).
- [ ] AC9 — No `#[allow(deprecated)]`, `todo!`, `unimplemented!`, or stubbed
  code paths introduced by the migration (R8).
- [ ] AC10 — At merge time: minor version bump + `CHANGELOG.md` entry +
  English developers-guide section covering agent/MCP access (steering).

## 6. Constraints & steering check

- **i18n (6 languages):** yes — any MCP status/settings strings go through
  `Tr` with all six translations (R11).
- **Generated-code / regenerate contract:** unaffected; `write_header` and
  `App::regenerate_all_forms` behavior must be byte-identical after the
  upgrade (covered by R2/AC2).
- **Docs (English guide):** required at merge — new section on egui 0.35 base
  and MCP agent access (AC10). Translations remain user-maintained.
- **Fix vs feature:** **feature** (minor bump at merge). Interim ports of
  `main` fixes onto the branch keep their original fix classification and are
  never mixed with migration commits (one concern per commit).
- **tech.md stack note:** `tech.md` currently pins "egui / eframe 0.29" — it
  must be updated to 0.35 when the branch merges (part of AC10).
- **Branch discipline:** work happens only on `egui-035`; `main` stays the
  release base until the gate passes. Push-window and forum rules apply
  unchanged to branch pushes.

## 7. Open questions

- **Q1 (resolved 2026-07-15):** MCP exposure — **always on in the IDE** (all
  IDE builds, debug and release); never in generated/packaged end-user apps or
  `rcrun` (operator, confirmed twice).
- **Q2 (resolved 2026-07-15):** dependency rule — egui-family crates OK,
  version bumps of existing deps OK, nothing else (operator).
- **Q3 (resolved 2026-07-15):** sync policy — replicate every `main` commit in
  the same session (operator).
- **Q4 (open):** MCP transport specifics (stdio vs local TCP port, and if TCP,
  fixed port vs configurable) — defaults will be proposed in /plan after
  reading egui_mcp's docs; flag now if you have a preference.
- **Q5 (open):** egui_commonmark's 0.35-compatible release must exist (it is a
  community crate tracking egui). If it lags, options analyzed in /plan
  (pin to compatible fork? vendor? wait?) — no decision needed yet.

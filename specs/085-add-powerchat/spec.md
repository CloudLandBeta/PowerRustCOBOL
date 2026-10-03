# Spec — Add PowerChat to an application

- **Status:** draft — awaiting the operator's review
- **Folder:** specs/085-add-powerchat/
- **Author:** Claude (for Emerson Lopes)   **Date:** 2026-10-03

## 1. Overview

The golden rules for applications built by coding agents (1.80.130) say
PowerChat is added **only when the developer asks**, and that it must then be
**effortless to add, in Spatial**. Today it cannot be added at all without
reworking PowerChat: PowerChat is itself an application shell. Its main form,
`CHAT-FORM`, carries a SideMenu that its code drives (conversations added as
rows, sections, labels in six languages, `ActivateItem`), and its other
screens load into its ContentPane from that menu.

In a host application that is already a shell, the product decides a form
with its own SideMenu never loads into the host's ContentPane (two rails
collide — operator ruling, PowerDemo3 nested-sidebar report). It opens as a
**child window** instead, and a child window is a plain form: it draws the
rail and a breadcrumb strip, but it has **no ContentPane and no navigation
chain** (`host.rs`, the child-window branch). So `CHAT-FORM` would open, but
Topics, Documents, Files, Prompts and Settings would have nowhere to load.

This spec closes that gap in the product, generally, and then makes adding
PowerChat one tool call.

## 2. Goals / Non-goals

- **Goals:**
  - A form with its own SideMenu, opened as a child window, runs as a **full
    shell in that window**: its own ContentPane, navigation chain, breadcrumb
    and Home — exactly as it would as a main form.
  - A coding agent adds PowerChat to an open project with **one tool**,
    `add_powerchat`, and the result passes `check`, wears Spatial, and is
    reached from an **Assistant** item in the host's side menu.
  - PowerChat keeps its six languages and its own data files.
- **Non-goals:**
  - Nesting one rail beside another inside a single ContentPane (rejected by
    the operator's earlier ruling).
  - Merging PowerChat's menu into the host's menu (it would mean rewriting
    PowerChat's menu code to address another form's SideMenu).
  - Changing PowerChat's behaviour as a stand-alone example.

## 3. User stories

- As a developer, I want to tell my coding agent "add PowerChat" and get a
  working assistant in my application, in my application's look.
- As a developer of any shell application, I want a sub-application with its
  own side menu to open in its own window and work there, not to lose its
  navigation.

## 4. Requirements (EARS)

### The child-window shell (product, all three hosts)

- **R1 (event):** When a form carrying a SideMenu is opened as a child window
  (`OpenFormSync`, `OpenFormAsync`, or a standalone menu action), the window
  shall run that form as an application shell: its rail, its breadcrumb, its
  ContentPane, its navigation chain and its Home action, as the main form's
  shell does.
- **R2 (event):** When an item of that window's menu loads a form, the form
  shall load into **that window's** ContentPane, never the main window's.
- **R3 (ubiquitous):** `super::` from a form loaded in that window shall
  address the form that loaded it inside that window; the window's own form's
  `super` stays the form that opened the window.
- **R4 (ubiquitous):** R1–R3 shall hold in `rcrun run-form`, in embedded child
  forms of the form host, and in a built application (`run_form_app`) — the
  three hosts that construct the interpreter.
- **R5 (constraint):** A child window whose form has no SideMenu shall behave
  exactly as today.

### Adding PowerChat (coding-agent tools)

- **R6 (event):** When a coding agent calls `add_powerchat` on an open
  project, the tool shall copy PowerChat's forms and menu into
  `forms/powerchat/`, its images into `Assets/powerchat/`, register every file,
  regenerate and check them.
- **R7 (ubiquitous):** The copied forms shall wear the host's theme: no form
  `theme` and no `glass-style` of their own, so they inherit Spatial.
- **R8 (ubiquitous):** The copied `CHAT-FORM` shall not be a main form; the
  host's main form is unchanged.
- **R9 (event):** When the host's main form carries a SideMenu, the tool shall
  add an **Assistant** item to that menu that opens `CHAT-FORM` as a child
  window (R1); otherwise it shall report how to open it from COBOL.
- **R10 (constraint):** The tool shall refuse, changing nothing, when a form
  object name it would copy already exists in the project, naming the
  collisions.
- **R11 (ubiquitous):** PowerChat's data files shall live under
  `data/powerchat/`, so they never collide with the host's data.
- **R12 (ubiquitous):** The PowerChat that is copied is the one in
  `examples/PowerChat`, compiled into the binary, so the tool always adds the
  version that ships with the IDE.

## 5. Acceptance criteria

- [ ] AC1 (R1, R2) — A headless run (`run_form`) of a host shell opens a
  SideMenu form as a child window, an item of the child's menu loads a form,
  and that form is in the child window's ContentPane.
- [ ] AC2 (R4) — The same scenario passes in `rcrun run-form` and in a built
  application.
- [ ] AC3 (R6–R9) — `add_powerchat` on a fresh `create_project` project with a
  shell main form: `check` reports no error, exactly one main form, no form
  sets a theme, the host menu has an Assistant item.
- [ ] AC4 (R10) — `add_powerchat` on a project that already has a `CHAT-FORM`
  is refused and nothing is written.
- [ ] AC5 — `render_form` pictures `CHAT-FORM` in Spatial; `run_form` on the
  host opens the Assistant and loads Topics inside the PowerChat window.

## 6. Constraints & steering check

- **i18n:** PowerChat already carries six languages; the Assistant menu label
  needs the six translations; no new IDE strings beyond the tool's messages.
- **Generated code:** unchanged contract — the tool regenerates what it adds.
- **Docs:** the Developer's Guide chapter 22 (the shell) gains the child-window
  shell; the coding-agent section gains `add_powerchat`.
- **Fix vs feature:** R1–R5 make a child window do for a SideMenu what the
  main window already does — arguably a fix (the rail is drawn but its
  navigation is dead). R6–R12 are a feature. They would ship as separate
  commits.

## 7. Open questions

- **Q1:** R1 makes a SideMenu form a full shell in a child window. Is that the
  behaviour you want for every application (e.g. PowerDemo3's nested sidebar
  demo would then work in its window), or only for PowerChat?
- **Q2:** Should the Assistant open as a **modeless** window (the operator
  keeps working in the host while chatting — proposed) or modal?
- **Q3:** PowerChat's branding (Titan Voyages logos) — keep, or replace with
  the host application's name and icon?
- **Q4:** PowerChat's model providers and agents live in its own data files,
  set up by its Welcome screen. Should it instead read the host
  application's AI settings when there are any?

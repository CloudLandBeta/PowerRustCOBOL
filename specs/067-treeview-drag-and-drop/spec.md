# Spec — TreeView drag-and-drop

- **Status:** draft → awaiting operator review
- **Folder:** specs/067-treeview-drag-and-drop/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-22

> Feature `067` of the umbrella spec **063 — RAG / Transactional Chatbot
> boilerplate** (063 R41, AC16). Independent of every other 063 feature; any
> application with a TreeView benefits.

## 1. Overview

A TreeView cannot be rearranged by the user. Its nodes live in the `Items`
property — one node per line, two spaces per level — and a node's handle is its
0-based line index (`treenodes.rs`). On a running form every row is registered
with `Sense::click_and_drag()`, but a drag on a row currently means **scroll**
(`render.rs` TreeView arm). No control in `cobolt-forms` uses egui's
drag-and-drop at all; the IDE's own project tree does (`panels/project.rs`:
`dnd_set_drag_payload`, a cursor ghost, `accept_file_drop` highlighting the
target folder).

A document manager needs exactly that: drag a document from one folder onto
another. This feature lets the end user drag a node onto, before or after
another node, lets the program decide whether the move is allowed, and moves the
node — with its whole subtree — in `Items`.

## 2. Goals / Non-goals

**Goals**

- Opt-in drag-and-drop of nodes within one TreeView, off by default so every
  existing form behaves exactly as today.
- Three drop positions — onto a node (becomes its last child), before it,
  after it — with visible feedback while dragging.
- The program is told what the user is doing and can refuse the move.
- The moved node keeps its subtree, icon, colours, checked and collapsed state.

**Non-goals**

- Dragging between two different TreeViews, or between a TreeView and another
  control. *(Q4)*
- Dragging OS files into a TreeView. The form already has `onDrop` for files.
- Multi-select drag.
- Keyboard-driven moves (cut/paste of nodes).

## 3. User stories

- As an **end user**, I want to drag a document onto a folder, so that I can
  file it without a separate "Move to…" dialog.
- As a **developer**, I want to be told *before* a node moves and be able to
  say no, so that the tree never shows a move my program could not perform
  (a file that failed to move on disk, a folder that must stay first).
- As a **developer**, I want existing trees to keep scrolling by drag unless I
  switch the feature on.

## 4. Requirements (EARS)

### 4.1 Enabling

- **R1 (ubiquitous):** A TreeView shall have a design-time property enabling
  node drag-and-drop, **off** by default.
- **R2 (state):** While drag-and-drop is off, dragging a row shall scroll the
  tree exactly as it does today.
- **R3 (state):** While drag-and-drop is on, dragging a row shall drag the node;
  the mouse wheel and the scrollbar shall still scroll.

### 4.2 Dragging

- **R4 (state):** While a node is dragged, the tree shall show which node it
  would land on and in which position — onto, before or after.
- **R5 (state):** While a node is dragged near the top or bottom edge of the
  tree, the tree shall scroll in that direction.
- **R6 (state):** While a collapsed node is hovered during a drag for a short
  delay, the tree shall expand it, so a deep target can be reached.
- **R7 (constraint):** A node shall not be droppable onto itself or onto any of
  its own descendants; the tree shall show such a target as refused.
- **R8 (event):** When the user presses Escape, or releases outside the tree,
  the drag shall be cancelled and the tree left unchanged.

### 4.3 Deciding and moving

- **R9 (event):** When the user releases a node over a valid target, the
  TreeView shall fire an event carrying the dragged node, the target node and
  the drop position, **before** moving anything.
- **R10 (event):** When the handler for that event does not refuse, the node
  and its whole subtree shall move to the target position in `Items`,
  re-indented to its new level, and a second event shall report the completed
  move with the node's new index.
- **R11 (event):** When the handler refuses, `Items` shall be left unchanged
  and no completion event shall fire.
- **R12 (ubiquitous):** A moved node shall keep its label, icon, colours,
  checked state and collapsed state, and remain the selected node if it was.
- **R13 (ubiquitous):** A running program shall be able to perform the same move
  itself — node, target, position — with the same rules (R7, R10, R12), and be
  told whether it succeeded.
- **R14 (constraint):** The move shall be one `Items` update, so the program,
  the renderer and every host see the tree either before or after the move,
  never halfway.

### 4.4 Parity

- **R15 (constraint):** Drag-and-drop shall behave identically on form preview,
  Run Form, the shell, and the compiled binary. The designer canvas shows no
  drag feedback (it is a design surface).

## 5. Acceptance criteria

- [ ] **AC1** — A form saved before this feature loads with drag-and-drop off
      and a row drag still scrolls. *(R1, R2)*
- [ ] **AC2** — With it on, dragging "Report.pdf" onto folder "Archive" makes it
      Archive's last child in `Items`, subtree and state intact. *(R10, R12)*
- [ ] **AC3** — Before/after drops place the node as the target's previous/next
      sibling at the target's level. *(R4, R10)*
- [ ] **AC4** — Dropping a folder onto its own grandchild is refused and `Items`
      is byte-identical afterwards. *(R7)*
- [ ] **AC5** — A handler that refuses the drop leaves `Items` unchanged and no
      completion event fires. *(R9, R11)*
- [ ] **AC6** — The program-driven move produces the same `Items` as the same
      drag, for a table of at least 10 cases (node, target, position →
      expected `Items`). *(R13)*
- [ ] **AC7** — Escape during a drag leaves the tree unchanged. *(R8)*
- [ ] **AC8** — A compiled binary and `rcrun run-form` produce the same events
      and the same final `Items` for the same scripted drop. *(R15)*
- [ ] **AC9** — `cargo test -p cobolt-forms --features render` green, including
      the parity guard. *(R15)*

## 6. Constraints & steering check

- **i18n:** the new property name is an identifier, not translated. Any new
  designer or IDE string is a `Tr` field in all six languages.
- **Generated code:** the new events need handler linkage. If they carry more
  than the existing `CONTROL-NODE-DATA` group holds (a target and a position),
  codegen gains a group for them — decided in `/plan`, English identifiers.
- **System KB:** new property, events and method → the `cobolt-compiler` doc
  tables are updated and `chunked.data` regenerated in the same change.
- **Methods must be callable:** the new method goes into `is_known_method`.
- **Three hosts:** `interpreter-binary-parity` applies — refusing a drop needs
  the handler's answer to reach the renderer, on every host.
- **Docs:** the Guide's TreeView section gains drag-and-drop. `-en.md` only
  exists, so no translation is deleted.
- **Fix vs feature:** a **feature** — user rearrangement of a tree is new
  capability. `features` branch, `z` bump.

### Defects found while surveying — not this feature's work

These are **fixes**, belong on `fixes`, and are listed so they are not lost:

1. `CONTROL-NODE-INDEX` is **1-based** (`render.rs` `node_payload`), while every
   `Node*(index)` method takes the **0-based** line index; the KB says they are
   the same number. A handler that passes one to the other gets the wrong node.
2. The editor's autocomplete advertises `RemoveNode`, `ExpandAll`,
   `CollapseAll`, `GetSelectedNode`, `SetSelectedNode`, none of which the
   runtime implements.
3. `onNodeCheck`, `onNodeExpand` and `onNodeCollapse` are emitted by the
   renderer but not declared in `supported_events`, so the designer cannot bind
   them; `onDataChanged` is filtered out the same way.
4. The KB's `Sorted` entry says TreeView "does not act on it yet"; it does.

Fix 1 matters here: this feature's events report node indexes, and should not
inherit an off-by-one. **Recommendation:** fix 1 lands on `fixes` before `/plan`
for this spec starts.

## 7. Open questions

- **Q1 — names.** Proposed: property `AllowDragDrop` (Boolean, default false);
  events `onNodeDragDrop` (before, refusable) and `onNodeMoved` (after); method
  `MoveNode(node-index, target-index, position)` with position `"ONTO"`,
  `"BEFORE"`, `"AFTER"`. *Recommendation:* these, unless the operator prefers
  PowerCOBOL-style names.
- **Q2 — how a handler refuses.** The event is asynchronous to the renderer.
  Options: (a) the handler sets a return flag the host waits for; (b) the
  control never moves on its own and the handler calls `MoveNode` to accept.
  *Recommendation:* (b) with an `AutoMove` property default true — when true the
  control moves and the handler may undo it with `MoveNode`; when false the
  handler must call `MoveNode`. That keeps the renderer from ever blocking on
  COBOL. This contradicts R9–R11's "before moving" wording for the default case,
  and the operator should choose which the spec keeps.
- **Q3 — auto-expand delay.** *Recommendation:* 600 ms, not a property.
- **Q4 — cross-tree drags.** Out of scope now; the event payload should still
  name the source control, so adding them later needs no handler change.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Product steering — PowerRustCOBOL

> Persistent product context for spec-driven development. Every `/specify` should
> be consistent with this file. Keep it short and current.

## What it is

**PowerRustCOBOL** is a modern, Rust-powered **RAD** (Rapid Application
Development) environment for **COBOL-85**: design forms visually, run them on a
fast tree-walking runtime, and compile to a single self-contained binary.

Three pieces:

- **RustCOBOL** — the COBOL-85 runtime/interpreter and language subset.
- **PowerRustCOBOL IDE** — the egui/eframe desktop app (form designer, editor,
  debugger, project model, AI assistant, documentation viewer).
- **rcrun** — the CLI: run · check · build · package.

## Who it's for

Developers who already know COBOL (e.g. Fujitsu COBOL, Veryant isCOBOL) and want
to build **graphical, form-based** applications without learning the host
language. No prior Rust knowledge is required to *use* it.

## Goals

- Faithful, useful **COBOL-85** support; missing/standard behaviour is treated as
  technical debt to fix, not optional.
- A productive **visual** RAD loop: design a form → generated COBOL → run/debug.
- Ship real apps: standalone binaries, no runtime install.
- Fully **localised** UI (EN/ES/PT/JA/ZH/FR) and theme-aware.

## Non-goals (by design)

- Not a general-purpose COBOL compiler farm or a mainframe emulator.
- The generated `.cbl` is a **build artifact**, not hand-edited source.
- Live cloud sync of docs/specs is out of scope.

## Voice & branding

- Never use the internal crate prefix "cobolt" in user-facing text.
- COBOL identifiers and source text stay **English**.
- Original work only; do not reproduce third-party material.

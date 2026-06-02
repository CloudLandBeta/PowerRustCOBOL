<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Bug Tracker

> **How it works**
> - `tools/check_bugs.sh` (or the daily scheduled scan) runs `cargo check --workspace` and
>   parses every `error[…]` line into this file.
> - A new bug gets a unique ID (`BUG-NNN`), the detection date, the affected crate, the Rust
>   error code, and a one-line summary.
> - When a bug is fixed, it is moved to the **Resolved** table and the fix is summarised in
>   `CHANGELOG.md` under the next version entry.
> - If the scheduled scan finds open bugs it posts a chat notification automatically.

---

## Open Bugs

| ID | Detected | Crate | Error | Summary |
|----|----------|-------|-------|---------|

_No open bugs — all clear! ✅_

---

## Resolved Bugs

| ID | Detected | Fixed | Crate | Error | Summary | Fix |
|----|----------|-------|-------|-------|---------|-----|

_None yet._

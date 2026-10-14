# Tasks — the corner pin

- **Status:** implemented in 1.91.145
- **Plan:** ./plan.md   **Date:** 2026-10-14

- [x] **T1 — The eight properties** (R1, R7; AC1) — `model.rs`; verify `perspective_properties` (extended).
- [x] **T2 — The projection** (R2, R3, R4, R5; AC2–AC5) — `perspective.rs` + `tests/perspective_pin.rs`; verify every `perspective_*` test file.
- [x] **T3 — Designer handles and drag** (R6; AC6) — `designer.rs` + `designer_perspective_096_tests.rs`; verify `cargo test -p cobolt-ide --bin cobolt-ide perspective_096`.
- [x] **T4 — Pane, help, IntelliSense, KB** (R8; AC7) — six languages; `build_chunked_kb`; verify the IDE guards.
- [x] **T5 — Three hosts** (R7; AC1) — `perspective_hosts_096.rs`; verify Run Form, child form, built binary.
- [x] **T6 — Guide and picture** (R8; AC7) — a pinned chart on a slanted quad, looked at.
- [x] **T7 — Finalize** — 1.91.145, CHANGELOG with the sweep's numbers.

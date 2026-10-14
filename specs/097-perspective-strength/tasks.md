# Tasks — `PerspectiveStrength`

- **Status:** implemented in 1.91.144
- **Plan:** ./plan.md   **Date:** 2026-10-14

- [x] **T1 — The property exists and means the same everywhere** (R1, R4, R5, R8; AC1, AC5 write side)
  - Files: `model.rs`. Do: constant, range 0–100, default 100; seed for the 12; add to `perspective_property_range`.
  - Verify: `cargo test -p cobolt-forms --features render --test perspective_properties`.
- [x] **T2 — The projection** (R2, R3, R6; AC2, AC3, AC4, AC6)
  - Files: `perspective.rs`, `tests/perspective_strength.rs` (new).
  - Verify: the edge-ratio table over strengths 0–100, the centre and size checks, the equality with the previous projection at 100,
    the convexity/positivity sweep; `cargo test -p cobolt-forms --features render --test perspective_strength -- --nocapture`.
- [x] **T3 — The pane, IntelliSense, help** (R7; AC7 in part)
  - Files: `properties.rs`, `prop_help_data.rs`, `ui_text_data.rs`, the pane/IntelliSense tests.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` (property help, ui_text, the perspective pane probe).
- [x] **T4 — System KB** (R7)
  - Files: `cobolt-compiler/src/lib.rs`, `chunked.data`. Verify: `prebuilt_chunked_kb_matches_the_published_documentation`.
- [x] **T5 — Three hosts** (R5; AC5)
  - Files: `cobolt-cli/tests/perspective_hosts_096.rs`. Verify: Run Form, child form and built binary read the same.
- [x] **T6 — Guide and picture** (R7; AC7)
  - Files: `docs/developers-guide-en.md`, `assets/images/perspective-strength.png`. Verify: the picture, looked at.
- [x] **T7 — Finalize**
  - `version.rs` 1.91.144, CHANGELOG; the suites of the crates touched; `cargo check --workspace --all-targets`.

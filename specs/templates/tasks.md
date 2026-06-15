# Tasks — <feature name>

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** <YYYY-MM-DD>

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

- [ ] **T1 — <title>** (R1)
  - Files: `…`
  - Do: …
  - Verify: `cargo build -p <crate>` + `cargo test -p <crate>` green; <observable check>

- [ ] **T2 — <title>** (R2, R3)
  - Files: `…`
  - Do: …
  - Verify: …

- [ ] **T<n> — Docs & i18n**
  - Update `docs/developers-guide-en.md`; add/verify `Tr` keys ×6 languages.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations).

- [ ] **T<n+1> — Finalize**
  - Bump version + CHANGELOG (if a feature); full `cargo test`.
  - Verify: all crates build & test; manual launch check per plan §6.

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and the
change is split into fix/feature commit(s) per the operator's rules (do **not**
commit/push unless the operator asks).

# Plan — <feature name>

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** <YYYY-MM-DD>

## 1. Approach
High-level design: how the requirements are satisfied. Reference R1…Rn.

## 2. Affected crates / files
- `crates/<crate>/src/<file>.rs` — <change>
- `docs/developers-guide-en.md` — <doc change, if user-facing>
- `crates/cobolt-ide/src/i18n.rs` — <new Tr keys ×6 languages, if UI strings>

## 3. Data / model changes
Types, `.cfrm` schema, config, on-disk formats. Note migration/compat.

## 4. Key decisions & alternatives
- Decision: … — Why: … — Rejected: …

## 5. Risks & mitigations
- Risk: … → Mitigation: …

## 6. Test strategy
- Unit/integration tests to add (which crate), what they assert and **report**.
- Manual/visual verification steps (launch the IDE, what to look for).

## 7. Steering compliance
- [ ] i18n: all new UI strings in 6 languages
- [ ] Generated-code banner + regenerate-on-action contract preserved
- [ ] English dev guide updated (translations untouched)
- [ ] Fix vs feature: <which> → version bump / changelog plan
- [ ] No "cobolt" in user-facing text; COBOL identifiers English

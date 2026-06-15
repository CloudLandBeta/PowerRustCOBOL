# Spec — <feature name>

- **Status:** draft → approved
- **Folder:** specs/NNN-<slug>/
- **Author:** <who>   **Date:** <YYYY-MM-DD>

## 1. Overview
One paragraph: what this feature is and the problem it solves.

## 2. Goals / Non-goals
- **Goals:** …
- **Non-goals:** … (explicitly out of scope)

## 3. User stories
- As a <role>, I want <capability>, so that <benefit>.

## 4. Requirements (EARS)
Use EARS phrasing; number each so plan/tasks can reference them.

- **R1 (ubiquitous):** The system shall …
- **R2 (event):** When <trigger>, the system shall …
- **R3 (state):** While <state>, the system shall …
- **R4 (optional):** Where <feature is enabled>, the system shall …
- **R5 (constraint):** The system shall not …

## 5. Acceptance criteria
Concrete, checkable conditions (map to requirements). These become the
verification steps in tasks.md.

- [ ] AC1 — …
- [ ] AC2 — …

## 6. Constraints & steering check
Confirm alignment with `specs/steering/*`:
- i18n (6 languages) impact? …
- Generated-code / regenerate contract impact? …
- Docs (English guide) update needed? …
- Fix vs feature classification: …

## 7. Open questions
- Q: … (resolve before approval)

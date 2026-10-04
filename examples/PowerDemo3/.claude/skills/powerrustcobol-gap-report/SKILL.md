---
name: powerrustcobol-gap-report
description: "Write a gap report when a request needs a verb, control, property, method, event, file feature or IDE capability the reference does not list."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-gap-report

Write a gap report when a request needs a verb, control, property, method, event, file feature or IDE capability the reference does not list.

## Steps

1. Confirm the gap: search the reference pack and call `kb_lookup` for the name. Only a name neither finds is a gap.
2. Do the part of the request that is supported, and check it.
3. Write `docs/compiler-requests/<YYYY-MM-DD>-<topic>.md` (today's date, a short hyphenated topic) with every section of the template below, in that order. The version line reads: kit and IDE 1.80.100.
4. Tell the developer what you did, what is missing, and where the report is.
5. Never invent the missing syntax, property, method, event or built-in instead.

## Tools

`kb_lookup` (`mcp__powerrustcobol-ide__kb_lookup`, or `mcp__powerrustcobol__kb_lookup` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

## Template

Save it as `docs/compiler-requests/<YYYY-MM-DD>-<topic>.md`.

```markdown
# Gap report: <topic>

## Request

<The developer's request, in their words.>

## Missing capability

<The verb, control, property, method, event, file feature or IDE capability that is missing, named precisely (e.g. `TextBox::SelectionStart`, `onDragOver` on `ListBox`).>

## Why it is needed

<What cannot be done without it, and why the supported features do not cover it.>

## Minimal example

<A minimal COBOL or `.cfrm` example of the wanted behaviour, as it would be written if it existed.>

## Workaround used

<What you did instead, if anything — or `None`.>

## Versions

<The kit and IDE version (the version this project's agent brief states).>

## Classification

<`Fix` — the behaviour is COBOL-85 standard and should already work; or `Feature` — a capability beyond the standard or beyond what the IDE offers today. Say why.>

```

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.

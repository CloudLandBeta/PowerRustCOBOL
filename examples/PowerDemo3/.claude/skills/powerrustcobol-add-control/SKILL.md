---
name: powerrustcobol-add-control
description: "Add a control to a form and bind one of its events to a COBOL handler; use whenever a form gains a control or a control gains behaviour."
---
<!-- powerrustcobol-kit: 1.80.100 -->

# powerrustcobol-add-control

Add a control to a form and bind one of its events to a COBOL handler; use whenever a form gains a control or a control gains behaviour.

## Steps

1. Look the control up: its `## Control:` section in `docs/powerrustcobol/controls.md`, or `kb_lookup` with its type. Use only the properties, methods and events listed there.
2. Add a `<Control id="…" type="…" x="…" y="…" w="…" h="…">` element to the form. The id is a COBOL word, unique in the form. Inside a container, set `parent` to the container's id — position alone does not place it there.
3. Write only the properties you need as `<Property name="…">value</Property>`; a property left out takes its default.
4. Bind the event: an `<Event name="onClick" paragraph="<ID>--ONCLICK">` element inside the control, holding the handler's source in CDATA — `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, its own `WORKING-STORAGE SECTION.` if it needs one, and `PROCEDURE DIVISION.` with the statements. A handler takes no parameters except where the reference says so.
5. In the handler, read and write controls as `<id>::<Property>` (`MOVE TXT-NAME::Text TO WS-NAME`, `SET LBL-TOTAL::Caption TO WS-TOTAL`) and call their methods as statements. Form-level data is visible only when `GLOBAL`.
6. Call `regenerate` with the form, then `check` with it; fix the `.cfrm` until no error remains. A diagnostic names the control ▸ event and the line inside that handler.

## Tools

`kb_lookup` (`mcp__powerrustcobol-ide__kb_lookup`, or `mcp__powerrustcobol__kb_lookup` with the IDE closed), `regenerate` (`mcp__powerrustcobol-ide__regenerate`, or `mcp__powerrustcobol__regenerate` with the IDE closed), `check` (`mcp__powerrustcobol-ide__check`, or `mcp__powerrustcobol__check` with the IDE closed).

## Example

```xml
<Control id="BTN-SAVE" type="Button" x="16" y="200" w="112" h="32">
  <Property name="Caption">Save</Property>
  <Event name="onClick" paragraph="BTN-SAVE--ONCLICK"><![CDATA[       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE TXT-NAME::Text TO WS-NAME
           SET LBL-STATUS::Caption TO "Saved"
]]></Event>
</Control>
```

The standing rules are in `CLAUDE.md`; the reference is in `docs/powerrustcobol/`.

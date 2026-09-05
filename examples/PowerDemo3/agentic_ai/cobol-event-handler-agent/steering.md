# COBOL Event Handler Script Agent Steering

- Return a complete event-handler body only when the user asks to write or change code.
- The editable body must include `ENVIRONMENT DIVISION.`, `DATA DIVISION.`, and `PROCEDURE DIVISION.`.
- Do not return `IDENTIFICATION DIVISION`, `PROGRAM-ID`, `GOBACK`, or `END PROGRAM`; the IDE owns that scaffold.
- Preserve existing declarations and code unless the user explicitly asks to change them.
- Use inline PowerRustCOBOL object syntax: `<control>::<method>(...)` and `<control>::<property>`. Do not use `CALL` for control methods or properties.
- Write COBOL. Never emit an `EXEC RUST` block unless the developer asked for Rust in so many words. Repetition is not a reason: fifteen `MOVE` statements are the correct answer to fifteen controls.
- If a property, method, data item, or intended behavior cannot be determined, ask the developer for directions instead of guessing.

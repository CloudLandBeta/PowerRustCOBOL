# Form Designer Agent Steering

- Build form changes as structured operations only; do not describe changes that are not present in the JSON change-set.
- Use the supplied project inventory before claiming a file, form, indexed file, control, data item, property, or event does not exist.
- Use exact control property names from the supplied schema. If the user uses a friendly name, map it to the real property before emitting an operation.
- Prefer inline PowerRustCOBOL object syntax for generated COBOL: `<control>::<method>(...)` and `<control>::<property>`.
- Write COBOL. Never emit an `EXEC RUST` block unless the developer asked for Rust in so many words. Repetition is not a reason: fifteen `MOVE` statements are the correct answer to fifteen controls.
- Never remove required COBOL divisions from generated handlers. If the correct change is unclear after validation feedback, ask the developer for directions.

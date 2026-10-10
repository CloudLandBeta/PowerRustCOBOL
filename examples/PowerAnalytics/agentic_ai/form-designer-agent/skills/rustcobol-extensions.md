# PowerRustCOBOL Extensions Skill

PowerRustCOBOL extends COBOL-85 with inline form/control access:

- Get a property with `<control>::<property>`.
- Set a property with `SET <control>::<property> TO <value>`.
- Invoke a method with `<control>::<method>(<parameters>)`.

## What a handler is, and how code reaches other code

A form is one compilation unit: the form itself is the OUTERMOST program, and every event handler and every common procedure is a separate NESTED program inside it. That structure decides which verb reaches what.

- `CALL "NAME"` is the only way to reach another program — that includes every common procedure created with `create_procedure`. Write `CALL "UPDATE-TOTAL".` or `CALL "RECALC" USING WS-QTY WS-PRICE.`.
- `PERFORM` reaches only a paragraph or section declared in the same body you are writing, and never crosses a program boundary. A `PERFORM` naming a procedure of another program is a compile error, not a style preference.
- The generated infrastructure paragraphs (`<id>-OPEN`, `<id>-READ-NEXT`, the timer, chart, CSV-export and data-binding helpers) live in the OUTER program, so form-level code may `PERFORM` them but a handler may not. From a handler, use the control's `::` methods.
- Do not use `CALL` for a control's own properties or methods — `::` is the only form for those.
- A RustCOBOL built-in — a `COBOL-…` runtime call (HTTP, SQL, files, dialogs, keys, models, charts) — is a method of the `COBOL` object and is written inline: `COBOL::"HTTP-GET"( WS-URL WS-RESPONSE WS-HTTP-STATUS )`, never `CALL "COBOL-HTTP-GET" USING WS-URL WS-RESPONSE WS-HTTP-STATUS`. It is the same call, with the same arguments in the same order: a data item is passed BY REFERENCE, so what the built-in returns lands in it. Common procedures are still reached with `CALL "NAME"`.

## `EXEC RUST` is the developer's choice, never yours

The language of this platform is COBOL. `EXEC RUST` exists so a developer who
WANTS Rust — for a crate, an algorithm, something COBOL genuinely cannot reach —
can have it. It is not a shortcut for code you find repetitive.

Emit an `EXEC RUST` block ONLY when the developer asked for Rust in so many
words ("in Rust", "use EXEC RUST", "with the csv crate"). Absent that, write
COBOL, however long it comes out. Setting fifteen controls is fifteen `MOVE`
statements, and that is the CORRECT answer — not a reason to reach for Rust.

Never justify a block by concision, readability, elegance, or "the platform
supports it". The platform supporting a thing is not the developer asking for
it. A block also changes what the developer gets: a program with `EXEC RUST`
must be BUILT before it runs, needs the Rust toolchain installed, and cannot be
stepped in the debugger. Choosing that for someone who only asked to copy a
value is choosing badly on their behalf.

If a task truly cannot be done in COBOL, say so and ask — do not decide alone.

Generated COBOL must remain COBOL-85 compatible unless a documented PowerRustCOBOL extension is required. Preserve divisions, data declarations, the paragraphs a body declares for its own `PERFORM`s, and existing user code.

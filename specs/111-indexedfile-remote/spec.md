# Spec 111 — IndexedFile on Web and Mobile through the Backend

Status: implemented (2.0.111). Branch `features-2x-indexedfile-remote`, cut from `2.x.x`. Classification: feature (a capability of the 2.x WebAssembly line).

## Problem

A DataGrid bound to a `.cidx` already reaches the remote file on a Web or Mobile form: the Build generates `RC-<n>-<FORM>-<n>-<GRID>-READ`/`-SAVE`, the data travels as JSON over the secure WebSocket (`CALL … WITH WEBSOCKET LINKAGE`), the runtime adds the bearer token. The non-visual `IndexedFile` control had no counterpart: its own verbs and its `<id>-OPEN … -CLOSE` paragraphs were "not served by the Backend yet", and a browser module has no disk file.

## Requirements (operator, 2026-10-31)

- **R1** An `IndexedFile` control on a Web/Mobile form reaches the remote file, like the bound grid. The developer's COBOL does not change.
- **R2** The plumbing is created automatically: each operation (OPEN, CLOSE, START, READ by key / NEXT / PREVIOUS, WRITE, REWRITE, DELETE) becomes a `CALL "<program>" WITH WEBSOCKET LINKAGE`, the data travelling as JSON groups.
- **R3** The Build generates the control's backend programs and their `[[export]]` entries under `backend/generated/` (`exports.toml`), in the same Build as the grids'.
- **R4** Any change to the control regenerates them; hand edits are lost. The banner convention holds: the Build replaces and removes files that carry its banner and refuses to overwrite one that does not.
- **R5** Every request carries a valid bearer token, invisible to the developer (the runtime owns connection, token, refresh, reconnection). Export roles come from the control's `BackendRoles`; any authenticated caller by default; never public.
- **R6** Check and Build stop with a clear message, naming the form and the control or the developer's line, when the file cannot be served.

## Decisions

- **D1 Stateless backend (O4).** Each program opens the file, does its verb, closes it. The browser sends the record area (its primary key is the key) and `KEY-MODE`; it keeps the cursor (`GT`/`LT` after a read, the `START`'s comparison after a `START`).
- **D2 Where the lowering happens.** In the parsed program of the form, after Common Code is linked (`cobolt_compiler::web_remote_indexed::lower_indexed_files`), for every program of the form that `SELECT`s the control's file — the form's own (the control's generated paragraphs) and a handler's. The developer's source is never touched. Every host runs a `CALL … WITH WEBSOCKET LINKAGE`, so no interpreter, `run_form_app`, `cobolt-wasm` or web-player change is needed.
- **D3 Served:** a file whose primary key is one field, fixed-length records of at most 4096 bytes with DISPLAY fields (the record travels as text). Refused at the developer's line: a read or `START` on an alternate key, `START … NOT =`, `ROLLBACK`. `COMMIT` is a no-op (each call is durable when it answers); the generated `<id>-ROLLBACK` paragraph answers status `94`.
- **D4 Status.** The answered `FILE-STATUS` goes to the `FILE STATUS` item, and to the control's `StatusDataItem`/`WS-<id>-STATUS` where the program can see it. A failed call leaves `30` and runs no phrase; the reason is in `COBOL::"WS-LAST-ERROR"`.
- **D5** `READ-FIRST`, `READ-LAST`, `COMMIT`, `ROLLBACK` keep their catalogue rows but have no program (compositions of the others, or nothing to do).

## Acceptance

- **AC1** Nine programs + `exports.toml` generated for a served control; roles reach every export; none public.
- **AC2** Over the real TLS channel with a token, a lowered form creates, writes (duplicate refused `22`), starts, reads next/previous/by key, rewrites, deletes and closes a `.cidx` through the backend; the same call without a token is refused; READ PREVIOUS after START `<=` equals the desktop's answer.
- **AC3** A roles change regenerates every export; a hand edit is lost; a file without the banner is not overwritten; removing the control removes its programs.
- **AC4** An unservable file stops Check and Build naming form and control.

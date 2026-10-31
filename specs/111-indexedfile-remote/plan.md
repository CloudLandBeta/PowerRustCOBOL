# Plan — spec 111

| Piece | Where |
|---|---|
| Catalogue rows: `KEY-MODE`, `RECORD-IMAGE` in; `FILE-STATUS`, `RESULT-IMAGE`, `PAST-END` out | `cobolt-wire/src/controls.rs` (`IX_PARAMS`, `IX_RESULT`) |
| Backend programs (nine per control) | `cobolt-codegen/src/remote_controls/indexed.rs`; hooked in `generate_backend_serving` |
| Frontend working storage (`RC-IX-*`, cursor per file) | `remote_controls::indexed::frontend_working_storage` |
| Lowering of the verbs | `cobolt-compiler/src/web_remote_indexed.rs` |
| Build / Check wiring, serve filter | `cobolt-project-tools/src/web/generator.rs` (`load_forms`, `backend_programs`) |
| System KB | `cobolt-compiler/src/lib.rs` (`"IndexedFile"` entry, section "The backend end of an IndexedFile control"); `assets/knowledge/chunked.data` |

`END`-words: the result flag is `PAST-END`, not `AT-END` (a COBOL keyword the parser would not take as a data name).

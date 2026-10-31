# Tasks — spec 111

- [x] T1 Catalogue rows (wire) — `cobolt-wire` tests green.
- [x] T2 Backend programs and exports (codegen) — `cobolt-project-tools/tests/indexed_file_backend.rs`.
- [x] T3 Lowering of OPEN/CLOSE/START/READ/WRITE/REWRITE/DELETE/COMMIT/ROLLBACK — `cobolt-compiler/tests/web_remote_indexed.rs` (5 tests).
- [x] T4 Build/Check wiring, regeneration, banner guard, refusals — `indexed_file_backend.rs` (3 tests).
- [x] T5 End to end over TLS with a bearer token — `cobolt-server/tests/remote_indexed_file.rs`.
- [x] T6 System KB doc tables and `chunked.data`.
- [x] T7 Version 2.0.111 and CHANGELOG.
- Not done / out of scope: alternate keys, split primary keys, COMP/COMP-3 records (the record travels as text), `ROLLBACK`, PartiQL members (`IXF-1::Execute`) on the web.

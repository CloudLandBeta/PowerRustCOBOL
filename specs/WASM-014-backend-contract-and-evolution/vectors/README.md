# WASM-014 wire transcript

`wire.v1.json` is one conversation between a client and a backend, in order: a refused `hello`, a `warn` `hello`, then a session (`hello` with a
`session`, `auth`, a call that succeeds, one call for each error class a program can cause, and `bye`).

- `dir` is `c2s` (client to backend) or `s2c`.
- After the `hello` the real frames travel sealed; the file holds them opened, as JSON.
- `key`, `nonce` and `token` are replaced by a word (`<base64 key>`, `<base64 nonce>`, `<token>`), and `expires_in` by `0`, so two runs agree.

Who checks it:

| Side | Where | What it checks |
|---|---|---|
| Backend | `crates/cobolt-server/tests/client_contract.rs`, `wire_transcript_matches_the_rig` | drives a real rig and compares every frame with the file; `UPDATE_VECTORS=1` rewrites it |
| Wire | `crates/cobolt-wire/tests/vectors.rs`, `wire_transcript_frames_parse` | every `s2c` frame parses with `parse_server`, every `c2s` frame with `parse_client` |

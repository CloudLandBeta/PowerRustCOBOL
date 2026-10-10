<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — PowerCloud and the platform it needs

Branches: `features-powercloud` (integration) ← `features-pc-aws`, `features-pc-creds`, `features-pc-media`,
`features-pc-grid`, `features-pc-audio`, `features-pc-example`. Order of merge: aws, creds, media, grid, audio, example.

## WP1a — AWS operations (branch features-pc-aws)
- [ ] T1  Route + event + method + docs for `AwsRekognition.RecognizeCelebrities`; BoundingBox fields on `DetectFaces`
- [ ] T2  Route + event + method + docs for `AwsS3.ListBuckets`
- [ ] T3  `AwsMcp.AwsCall(service, operation, paramsJson)` (literal-only, `mutating`, AWS error code → `onError`)
- [ ] T4  Fake-server answers, demo forms and `aws_hosts` tallies for T1–T3; per-op tests in `aws_delivery_c.rs`
- [ ] T5  Test: `MOVE "name" TO AWS-1::Connection` switches the connection for the next call (Q3)
- [ ] T6  KB tables, Guide AWS section, support matrix (not `chunked.data`)

## WP1b — run-time credentials, amendment A6 (branch features-pc-creds)
- [x] T7   Per-connection in-memory store, redacting `Launch.credentials`, `built_env` as the only reader, restart on change
- [x] T8   `AWS-SET-CREDENTIALS`, `AWS-CLEAR-CREDENTIALS`, `AWS-CREDENTIALS-SET` built-ins (+ BUILTINS, `exec_call`, KB, Guide)
- [x] T9   Launch without `--profile` when credentials are set; the server restarts when they change or clear
- [x] T10  The credential-surface scan test (R7) over arguments, an unrelated child's environment, logs, Verbose, errors, saved files
- [x] T11  Amend spec 078 (A6) and every text listed in plan §WP1; `i18n.rs` ×6 and `prop_help_data.rs` `Connection` help ×6

## WP2a — IMAGE-FIT, IMAGE-INFO, HTTP-DOWNLOAD (branch features-pc-media)
- [ ] T12  `images` runtime feature + shim, forwarding, `RuntimeFeatures` plumbing and pinned test strings
- [ ] T13  `IMAGE-FIT`, `IMAGE-INFO` (EXIF orientation, crop-to-fill, PNG/JPEG by extension, 64 MP guard)
- [ ] T14  `HTTP-DOWNLOAD` (bytes, redirects, `.part` rename, caps, no query string in logs) with a local test server
- [ ] T15  BUILTINS, KB table, Guide §11, support matrix; a quantified `tests/cobol/` program

## WP2b — DataGrid Image cell retry (branch features-pc-grid)
- [ ] T16  Retry a failed Image-cell load; render test (file appears after first paint); document the Image kind in the KB tables and the Guide

## WP3 — audio built-ins (branch features-pc-audio, after WP2a)
- [ ] T17  `audio` feature, dedicated device thread, null sink, state machine
- [ ] T18  `AUDIO-PLAY/PAUSE/RESUME/STOP/STATE`; BUILTINS, KB, Guide, support matrix, `scan_*`
- [ ] T19  BUILDING-en, README and CI package list (ALSA); `cargo tree` proof that a build without audio calls links none

## WP4 — the example (branch features-pc-example, after WP1–WP3)
- [ ] T20  Project, assets (wheel image), palette, frameless main form, title panel, tabs
- [ ] T21  Settings tab (credentials, Connect, Deploy, Save template, link) and `cloudformation/powercloud.yaml` + its test
- [ ] T22  DynamoDB tab (+ pet popup, view popup), S3 tab, Lambda tab
- [ ] T23  Polly tab (player), EC2 tab (+ Working… window), Rekognition tab (boxes)
- [ ] T24  Help ▸ Examples entry, `Tr` ×6, `EXAMPLES` tests, README
- [ ] T25  `powercloud_hosts.rs` with a quantified summary; goldens; 091 guard

## WP5 — documentation and translations (integration branch)
- [ ] T26  Guide, support matrix, DEPENDENCIES (English), CHANGELOG entries
- [ ] T27  Regenerate `chunked.data` once
- [ ] T28  Translate the Guide, the matrix and DEPENDENCIES; verify; `every_document_ships_in_every_language` green

## Done criteria
All acceptance criteria of the spec checked; `cargo test` sweeps (per crate, `--no-fail-fast`) with every `test result:` read;
`-p cobolt-forms --features render` green; the NIST gate unchanged; nothing merged into `main`.

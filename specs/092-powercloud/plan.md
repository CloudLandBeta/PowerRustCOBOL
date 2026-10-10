<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — PowerCloud and the platform it needs

Spec: `specs/092-powercloud/spec.md` (read it first; D1–D9 are decided).

## 1. Approach

Four independent platform work packages (WP1–WP3 and the DataGrid fix inside WP2), then the example (WP4) on top of
them, then documentation (WP5). The platform packages touch different crates and can be built in parallel; each is
tested on its own with the repository's own conventions, then merged into the integration branch
`features-powercloud`. The example is the integration test of all of them.

```
main ─ features-powercloud ─┬─ features-pc-aws      (WP1)
                            ├─ features-pc-media    (WP2)
                            ├─ features-pc-audio    (WP3)
                            └─ features-pc-example  (WP4, after WP1–3 are merged)
```

## 2. Work packages

### WP1 — AWS extensions (WP1a: R1–R4, R9) and run-time credentials (WP1b: R5–R8)

Facts (read-only investigation, 2026-10-09; "R:" = `crates/cobolt-runtime/src`):

- **Routes are data:** `R/aws/routes.toml` (grammar documented at its top, :7-77), structs in `R/aws/routes.rs`. An
  operation is `[ops."<ControlType>.<Method>"]` with `server`, `tool`, `event`, `input`, `mutating`, `result`, `rows`
  (`from`, `fields`, `where`), `fail`. Every hosted operation uses `server="hosted"` and `tool="aws___run_script"` with a
  Python body calling `call_boto3(service_name=…, operation_name=…, params=…, region_name=…)`; snippets at :289-341.
  Worked examples: `DetectFaces` :546-558, `S3.List` :432-444. A method exists exactly when a route key exists
  (`interpreter/aws.rs:146-155`).
- **To add an operation:** the route; the event name in `crates/cobolt-forms/src/aws_catalog.rs` (Rekognition :191,
  S3 :168, before `onComplete`); the uppercase method in `crates/cobolt-ast/src/methods.rs:165` (enforced by
  `crates/cobolt-runtime/tests/aws_routes.rs:66-74`); the doc tables in `crates/cobolt-compiler/src/lib.rs`
  (`control_method_docs` Rekognition :7682, S3 :7664; `event_reference` :7022; `property_reference`;
  `control_purpose` :7179; `control_usage_notes` :7769; the "AWS controls (spec 078)" table :8860-8885); the Guide AWS
  section (:13337) and `docs/cobol-support-matrix-en.md:281`. `crates/cobolt-project-tools/src/reference.rs` and the IDE
  completion read the tables by themselves. **Do not regenerate `assets/knowledge/chunked.data` on a work-package
  branch** (a binary file that would conflict); the integration branch regenerates it once.
- **AwsMcp** is `routes.toml:830-843` (`server={prop:ServerId}`, `tool={arg:1|or:{prop:ToolName}}`, `mutating="by-tool"`,
  no `fail` mapping). `AwsCall` is a new literal-only operation modelled on the "illustrative alternative" the
  investigation found compatible with `no_op_accepts_script_text_from_cobol` (`tests/aws_delivery_c.rs:261-301`):
  `service_name={arg:1|quote}`, `operation_name={arg:2|quote}`, `params=json.loads({arg:3|json|text|quote})`. It must
  map an AWS error to `fail` so `onError` carries the code.
- **Row shapes:** `DetectFaces` fields today are Confidence, AgeLow, AgeHigh, Smile, Emotion; BoundingBox is only in
  `ResultJson`. A field is one line: `Left="/BoundingBox/Left"`. `AwsEC2.Describe` rows (no `fields`): InstanceId, Name,
  State, Type, PublicIp, PrivateIp. `AwsS3.List`: Key, Size, LastModified; one page only.
- **Connections and credentials:** `[[integrations.aws_connections]]` (`crates/cobolt-forms/src/connections.rs:323-356`);
  published to the global registry `R/aws/connections.rs` (`find()` by name or id); a control's `Connection` property is
  re-read on every call (`interpreter/aws.rs:198-200`). **Region and profile are not control properties.** The server is
  launched by `launch_for` (`R/aws/routes.rs:488-504`) and `R/aws/process.rs:146-153` (`env_clear().envs(built_env())`);
  R11 is enforced there: `SECRET_VARS` :42, allow-list `INHERITED_VARS` :47, two strips in `built_env` :86-91, `mask()`
  :188, `plain_error` (`ops.rs:176`), sensitive ops hidden from Verbose (`interpreter/aws.rs:243`). Tests:
  `tests/aws_process_env.rs` (AC7), `process.rs:256/281`, `aws_delivery_c.rs:457`, `project_model.rs:2099`.
  `CREDENTIAL_PROPS` is in `crates/cobolt-forms/src/connections.rs:443`; `xml.rs:2078-2095` blanks them on save.
- **Design (A6):** a per-connection in-memory store (`R/aws/secrets.rs` already holds per-owner in-memory values for
  `{secret:}`), a separate `Launch.credentials` field whose `Debug` is redacted (`Launch` and `Prepared` derive `Debug`
  today), `built_env` the only reader, a changed `Launch` restarts the server (`pool.rs handle_for`). Entry is a built-in
  shaped like `COBOL-KEY-SET` (`interpreter.rs:11693-11725`), **not** a property (properties would meet
  `CREDENTIAL_PROPS` and the IDE's persist-to-store routing: `form-host/seeding.rs:200`, `ide/form_runtime.rs:241`,
  `ide/app.rs:18852`). The key store (`R/key_store.rs`) is not used: its default persists to disk.
- **Text to amend (A6):** spec 078 `spec.md` 38-42, 93-94, 172-175 (R11), 182 (R12), 318-323 (AC7/AC8), 375-376,
  `plan.md` 52, 130, 204-205, `tasks.md` 104, 108, 222, 228; the Guide :13361, :13391, :13401; `process.rs` module doc
  :11-16 and :36; `aws/mod.rs:6-8`; `cobolt-compiler/src/lib.rs:7923`; `i18n.rs:3341` (×6 languages) and the
  `Connection` help in `prop_help_data.rs` (16 controls × 6 languages) — **say exactly what A6 says and no more**.
- **Fake server and tests:** `R/bin/fake_mcp.rs` (answers, `answers_when`, `delay_ms`, `crash_after`, `env_dump`, …);
  fixtures `tests/fixtures/aws-mcp/*.tools.json` need a `_provenance` string; `hosted_answers()` in
  `crates/cobolt-cli/tests/aws_hosts.rs:154-182` (first rule whose `operation_name='X'` matches; order matters);
  per-op tests in `tests/aws_delivery_c.rs`; the pinned tallies `DEMOS` at `aws_hosts.rs:~262` and
  `tests/cobol/aws/README.md`; demo forms in `tests/cobol/aws/forms/`.
- **Q3:** add the test that `MOVE "name" TO AWS-1::Connection` switches the connection for the next call.

### WP2 — media built-ins (WP2a: R10–R13) and the DataGrid image retry (WP2b: R14)

- **Built-in end to end** (`R:` = `crates/cobolt-runtime/src`): `COBOL::"X"(…)` is lowered by the parser
  (`cobolt-parser/src/stmt.rs:3386 cobol_object_call`) to `CALL "COBOL-X"`, data items BY REFERENCE, literals BY CONTENT —
  no AST change. Touch: `R/builtins.rs:35 BUILTINS` (alphabetical; the test at :98 compares it with the `"COBOL-…" =>`
  arms), `interpreter.rs exec_call` (:11120; arms look like `"COBOL-HTTP-GET" if using.len() >= 3` :11566; model: read
  args with `eval_call_arg(&using[i])`, write outputs with `expr_to_name(call_arg_expr(..))` + `env.set_str`; too few
  arguments falls through to an "unknown program" warning :12340), the KB table `cobolt-compiler/src/lib.rs:4630-4670`
  (the test `grace_host.rs:4749` requires a `COBOL::"NAME"` row per built-in), Guide §11 (:7333, family list :7360),
  `docs/cobol-support-matrix-en.md`. IntelliSense and the agent reference are automatic from `BUILTINS`.
- **Features:** `cobolt-runtime/Cargo.toml` (`default` :19, `http = ["dep:ureq","dep:native-tls"]` :44); form-host
  forwards each feature (`cobolt-form-host/Cargo.toml:17-29`); `cobolt-compiler/src/runtime_features.rs`: `RuntimeFeatures`
  (:76), `all()`, `union`, `as_toml_features`, `scan_forms`, `scan_program`, `scan_rust`, `HTTP_CALLS` (:59); literal CALL
  names switch a feature on, a computed target turns everything on; `lib.rs:2479-2530` and `base_dependency_block`
  (:3490-3520); tests pin the exact feature strings (`runtime_features.rs:354, 376, 483-484, 619`). Add an `images`
  feature (optional `image` dependency `default-features=false, features=["png","jpeg","gif","webp","bmp","tiff"]`) with
  an off-build shim like `http_runtime.rs:284`. Form apps already link `cobolt-forms/render`, so `image` is already in
  every form app; only console programs gain it.
- **Image API** (image 0.25.10): `DynamicImage::resize_to_fill` (crop + resize), `image_dimensions(path)` (header only),
  `apply_orientation`, `JpegEncoder::new_with_quality`. Precedents for encode: `cobolt-forms/src/raster.rs:162 to_png`,
  `cobolt-ide/src/doc_shots.rs:940`.
- **HTTP:** native-tls through ureq 2.12.1 (`http_runtime.rs:84 tls_connector`, `:104 agent`); `agent(0)` follows 5 redirects and
  strips Authorization on redirect; precedent for a binary download to a `.part` file renamed on success:
  `cobolt-forms/src/viewer_remote.rs:236 fetch` (reads into memory, 256 MB cap). `HTTP-GET` is not binary-safe
  (`body_of` :158 uses lossy UTF-8, 10 MiB cap). Stream to the file instead; never log the query string.
- **DataGrid image retry:** `cobolt-forms/src/render.rs:11796-11809` caches `None` for a failed load, keyed (grid, path), so
  a thumbnail missing at first paint never appears; PictureBox does not cache failures (`paint.rs:7966`). Retry a failed
  path (e.g. re-check every ~500 ms or when the file's mtime appears). The Image kind (cell value = file path, drawn
  Fit, `image_corner_radius`, `image_shadow`; a `Columns` type `image|img|picture` is a fallback that never upscales) is
  undocumented in the KB tables (`cobolt-compiler/src/lib.rs` `property_reference` 6254-6290) and has no render test
  (existing icon-button test: `render.rs:25053`). Cell kinds also include Button (`icon:<name>` built-in vector icons in
  `icons.rs`: pencil, trash, eye, download, folder…), Checkbox, Dropdown, Gauge; clicks write `ClickedRow`/`ClickedColumn`
  and fire `onCellClick`; **header clicks sort and do not write `Clicked*`**; `SelectableText` (default true) gates click
  reporting; `RowHeight` is clamped 14–120.

### WP3 — audio built-ins (R15–R18)

- No audio crate or playback exists. Add `rodio` (with `symphonia` mp3/vorbis/flac/wav, `default-features=false`) behind an
  optional runtime feature `audio`; the device (`OutputStream` is not `Send` on every platform) lives on one dedicated
  thread fed by a channel; a `Sink` abstraction with a **null sink** (real-time clock) for tests and for "no device".
- Built-ins: `AUDIO-PLAY (file status)`, `AUDIO-PAUSE`, `AUDIO-RESUME`, `AUDIO-STOP (status)`, `AUDIO-STATE (state
  position-ms duration-ms)`. Same end-to-end checklist as WP2 (BUILTINS, `exec_call`, KB table, Guide, support matrix,
  feature plumbing and `scan_*` names).
- CI is `.github/workflows/build-all-platforms.yml` (manual `workflow_dispatch`, `cargo build --release --locked`, no
  `cargo test`; Linux installs `pkg-config libssl-dev libxkbcommon-dev libxcb-render0/shape0/xfixes0-dev` at :141-143;
  the `ldd` check greps only libssl/libcrypto :315; .deb/.rpm declare only Suggests :1013/:1141). Add the ALSA
  development package (`libasound2-dev`) to the workflow's package list, `docs/BUILDING-en.md:108` and the README. Spec 063
  R18 and `runtime_features.rs:1-37` are the rules: optional dependencies stay out of the build graph unless enabled.

### WP4 — the PowerCloud example (R19–R30), after WP1–WP3

- **Project folder** (model: `examples/PowerAnalytics`, `examples/PowerSpatial`): `<Name>.project.toml` with `[project]`,
  `[files]` (sources/forms/assets/generated), `[forms] main-form` and `main-form-seal`; `forms/*.cfrm`; `src/*.cbl`; `Assets/`;
  `README.md`. `ImagePath` is stored project-relative (`Assets/images/…`); `.webp` decodes by content (the `webp` feature is
  on). Files listed under `[files] assets` are copied to `bin/`.
- **Window:** `title-visible="false" corner-radius=… resizable=…` on the `<Form>`; the close box is a Panel + Button (see
  PowerSpatial `settings-form.cfrm:2, 89, 98, 130` and `toolbar-form.cfrm:296` — `IconPath` close.svg, `CornerRadius`,
  `INVOKE me::Close()`). A frameless window drags by its backdrop. Palette per spec §5; colours are `#RRGGBB[AA]`
  (alpha last; no 3-digit shorthand); explicit properties win over the Spatial defaults.
- **Tabs:** pages from `Tabs` (one title per line); a child sets `parent="<tab control>" tab="N"` (1-based); events
  `onTabChanged`, `onTabClick`, `onChange`; `SelectedTab` is writable. There is no property to hide the tab strip.
- **Grid:** a `Columns` + `AdvancedGrid` setup with an Image column, Button columns (`icon:eye`, `icon:pencil`,
  `icon:trash`) and a Switch/Checkbox column; PowerChat `files-form.cfrm:1323-1401` is the working model (icon columns with an
  `EVALUATE Dg-List::ClickedColumn` handler). `ColumnFilters` is `column=value` lines, case-insensitive substring;
  `SetFilter()`, `ClearFilters()`, `ShowColumnFilters`.
- **Dropzone:** `AllowedExtensions`, `DroppedFiles` (newline-separated absolute paths), `onFilesDropped`, `onFilesRejected`.
- **Modal forms:** `INVOKE ME::OpenFormSync("FORM")` blocks the caller's COBOL flow until the child closes, and a parent
  timer cannot close it; use `OpenFormAsync` (returns a handle; `H::"Close()"`) for the EC2 *Working…* window and poll from
  the parent's Timer; the child hands data back through `super::SetProperty`, the caller reads with `ME::GetProperty`
  (PowerChat `confirm-form.cfrm:172-276`).
- **Overlay boxes:** `Shape` controls repositioned from COBOL over the PictureBox; the displayed image rectangle follows
  `SizeMode` (Zoom letterboxes), so compute it from `IMAGE-INFO` and the PictureBox's rect.
- **Test:** `crates/cobolt-cli/tests/powercloud_hosts.rs`, modelled on `aws_hosts.rs` (`rcrun run-form <form> <program>
  --designer --headless steps.json`), with the fake server's `hosted_answers()` extended; `every_example_form_saves_unchanged_091`
  walks all of `examples/` and needs byte-stable round trips; `example_corpus_golden.rs` and `example_corpus_codegen.rs` hold
  hard-coded project lists to extend; `crates/cobolt-ide/tests/` example-compile tests are the model for a PowerCloud one.
- **Help ▸ Examples:** `crates/cobolt-ide/src/app.rs:1811` `EXAMPLES: [Example; 4]` → 5 (new entry **last**, tests use
  `&EXAMPLES[1]` at :22964); menu code :15912-15935 with a `match ex.project` arm for the description; `do_open_examples`
  :4584 copies a read-only shipped copy to `~/Documents/PowerRustCOBOL Examples`; the description is a `Tr` field
  (`examples_powercloud_about`) in six languages (`i18n.rs` pattern at :2273; the six tables at the existing
  `examples_poweranalytics_about` lines). CI stages `git archive HEAD examples` (tracked files only; `.gitignore` excludes
  `generated/`, `bin/`, `dist/`, `*.redb`); the WiX harvest and `--seed-examples` iterate `EXAMPLES`.

### WP5 — documentation, knowledge, translations

English Guide edits (built-ins §11, AWS chapter §16 incl. A6, chapter 8 grid cell kinds, §3 audio prerequisite, the
examples section, the PowerCloud example text), the support matrix, `DEPENDENCIES-en.md`, `CHANGELOG.md`; regenerate the
chunked KB once; then the regeneration cycle for the Guide, the matrix and DEPENDENCIES (GOLDEN RULE #8: temp files in the
scratchpad, split by chapter, verified).

## 3. Rules for every implementing agent

- Read `/Users/emersonlopes/Documents/PowerRustCOBOL-local-settings/CLAUDE.md` (the project's rules; the symlink is absent
  in a worktree), `CONVENTIONS.md`, `specs/steering/*.md`. **Do not guess: check the source.**
- Work only in your own git worktree on your own branch, off `features-powercloud`. Never touch another worktree.
  Never `git stash`; commit instead. **Do not push, and do not merge into `main`.** Commit with the trailer
  `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. **Do not bump the version and do not edit `CHANGELOG.md`**
  (every package would conflict on the same top lines): put the changelog text you would have written in your final
  message and the integrator adds it, with the right number, when the package is merged.
- Build and test with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TARGET_DIR=<the shared target dir given to you>`;
  read every `test result:` line; `--no-fail-fast`; `cargo test -p cobolt-forms --features render`; `cobolt-ide` needs `--bin`.
  Check `df -h` before a big build.
- Tests first for new behaviour. Every new COBOL test under `tests/cobol/` prints one quantified summary block
  (GOLDEN RULE #7). COBOL you write uses the RustCOBOL extensions (see Guide §13 *Code style*), built-ins inline.
- Update the **System KB tables** but do **not** regenerate `assets/knowledge/chunked.data`; update the English Guide in the
  sections named above; **do not touch translations** (the integration branch deletes/regenerates them).
- New IDE strings are `Tr` fields in all six languages. COBOL identifiers stay English.
- No credential, key or token in any file or log. Throwaway scripts live outside the repository.
- Finish with: the list of commits, the tests you ran with their result lines, what you could not verify, and anything
  the spec got wrong.

## 4. Risks

| Risk | Mitigation |
|---|---|
| The hosted proxy may not honour env credentials | launch without `--profile`; opt-in live test (Q1); document the profile route as the fallback |
| `rodio`/ALSA breaks a CI job or a build host | optional feature; null sink for tests; BUILDING + workflow package list |
| A new built-in shifts the feature scan and bloats a build | `scan_*` tests pin the exact feature strings; AC4/AC6 build-without checks |
| Parallel packages conflict on `Cargo.lock`, the Guide, the changelog | each edits distinct sections; the integrator resolves, regenerates the lock |
| The example grows into one unreviewable form | one main form + three popup forms; procedures in `src/`; the hosted test per tab |
| Disk | one shared target dir; `CARGO_INCREMENTAL=0`; clean merged worktrees' `target/` |

## 5. Test strategy

Unit and integration tests per work package (the lists above), then `powercloud_hosts` end to end, then the full sweeps
named in the repository (`cargo test` per crate with `--no-fail-fast`, `-p cobolt-forms --features render`, the NIST gate
for anything touching the interpreter), then the opt-in live smoke test by the operator.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — AWS controls through MCP (spec 078)

- **Status:** draft → awaiting approval (then `/implement`)
- **Plan:** ./plan.md (approved 2026-09-30)   **Date:** 2026-09-30
- **Line:** a **feature**. Work goes on the operator's working line (`1.80.x` today, or `features` if the operator says so), never on `main`, and is pushed only when the operator allows.

The tasks are small, ordered and each can be checked on its own. Each names the code it **reads first**, the files it touches, the requirements it satisfies, and how to verify it. Line numbers come from the 2026-09-30 surveys and **will have moved**: re-read at the moment of the task (CLAUDE.md: *do not trust your memory*). Tick each task off as it is completed.

## Standing rules for every task

- **Gate K (knowledge).** No property, method or event is committed without all of the following:
  - its `PropHelp` in 6 languages;
  - its row in the System KB tables (`cobolt-compiler/src/lib.rs`);
  - `cargo run -p cobolt-ide --example build_chunked_kb` and the regenerated `assets/knowledge/chunked.data`;
  - green runs of `prop_help_tests`, `every_control_property_is_documented` and `prebuilt_chunked_kb_matches_the_published_documentation`.
- **Gate P (parity).** No runtime task is done until the three hosts (`rcrun run-form`, embedded child forms, the compiled binary) are named in its commit message, with a reason for any host that does not apply (the `interpreter-binary-parity` skill). Sweep: `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler -p cobolt-cli --no-fail-fast`.
- **Gate G (golden).** Any task that touches `cobolt-forms`, `cobolt-form-host` or `cobolt-codegen` keeps all of these green, with each `test result:` line read (never judged by grepping for failures):
  - `cargo test -p cobolt-forms --features render --test example_corpus_golden`;
  - `cargo test -p cobolt-form-host corpus_golden`;
  - `cargo test -p cobolt-codegen --test example_corpus_codegen`.
  This feature adds no codegen, so these goldens must not move.
- **Gate F (full sweep)** at the end of each delivery runs, all with `--no-fail-fast`:
  - `cobolt-mcp`, `cobolt-runtime`, `cobolt-forms --features render`, `cobolt-form-host`, `cobolt-codegen`, `cobolt-compiler`, `cobolt-cli`;
  - `cobolt-ide --bin cobolt-ide`.
  Expected reds, listed by name:
  - `test_maps_demo_form` (a fix pending on `fixes`);
  - `docs_embed::every_document_ships_in_every_language` / `every_translation_is_complete_and_current`, until the next translation cycle;
  - `powerchat_documents_embed_with_the_builtin_model` (environmental, macOS Metal enumeration).
- **Every commit:** bump `z` in `crates/cobolt-ide/src/version.rs` and add a CHANGELOG entry dated with the absolute date. Never mix in a fix. **No crate is ever added** (AC2); `cargo tree` is checked at T-A3 and at each delivery's end.
- **Every new test** prints a quantified summary of what it measured (GOLDEN RULE #7).
- **A fix found on the way** is not made here. It goes to `fixes` from `main`.
- **Never drive the IDE.** The operator does the manual checks.

---

## Delivery A — client, transport, pool, connections, `AwsMcp`, `AwsLambda`

### A.1 The MCP client (`cobolt-mcp`)

- [ ] **T-A1 — Wire types for the client and a tolerant `Content`** (R1, R6; AC4)
  - Read first: `cobolt-mcp/src/types.rs` (the whole file), `server.rs` `handle_one`/`finish`.
  - Files: `crates/cobolt-mcp/src/types.rs`, `lib.rs`.
  - Do:
    - Add `InitializeParams`, `ClientInfo` (or reuse `ServerInfo`), `ListToolsParams { cursor }` and `ListToolsResult { tools, nextCursor }`, `CallToolParams { name, arguments, _meta }`.
    - Add `enum Message { Request(Request), Response(Response) }` with `Message::parse(&[u8])`, classified by the `method` key versus `result`/`error`.
    - `Tool` gains `annotations: Option<Value>`.
    - `Content` gets a raw fallback variant (`#[serde(other)]`, or an untagged `Other(Value)`) so image and resource content decode. The server still only writes `Text`.
    - `SUPPORTED_VERSIONS` becomes `["2025-11-25", "2025-06-18", "2024-11-05"]`, plus the `STATELESS_REVISION = "2026-07-28"` constant. The server's `negotiate` behaviour must not change (its tests pin that).
  - Verify:
    - `cargo test -p cobolt-mcp` is green.
    - New tests: `an_image_content_decodes_as_other`, `message_classifies_request_notification_and_response`, and `every_client_request_parses_as_a_server_request`, which round-trips each client request through the server's parser (AC4).

- [ ] **T-A2 — `client.rs`: session, id correlation and the pump** (R1, R3, R4, R5)
  - Read first: T-A1's types, `transport.rs` `read_message`/`write_message`.
  - Files: `crates/cobolt-mcp/src/client.rs` (new), `lib.rs`.
  - Do:
    - `Session` holds the id counter, a pending map from id to slot, and the negotiated revision or the stateless flag.
    - `Session::initialize(w)` sends `initialize` then `notifications/initialized`, and checks the answer's `protocolVersion` against `SUPPORTED_VERSIONS`.
    - `Session::pump(r, w)` routes one inbound frame:
      - a response goes to its id;
      - `ping` gets `{}`;
      - `sampling/*`, `roots/list` and `elicitation/*` get `METHOD_NOT_FOUND`;
      - `notifications/*` are dropped.
    - `list_tools_all()` follows `nextCursor`.
    - `call_tool(name, args)` in stateless mode adds `_meta["io.modelcontextprotocol/protocolVersion"]` and sends no handshake.
    - The crate stays pure: no threads, no clock, no process. Timeouts belong to the caller (T-A4).
  - Verify: unit tests over `io::Cursor` against scripted server bytes cover:
    - the handshake, with 2025-11-25 and with 2025-06-18 answers;
    - two-page `tools/list`;
    - a response arriving after a progress notification and a server `ping`;
    - `isError` surfaced;
    - a stateless call carrying `_meta`;
    - an unsupported answered version rejected with a clear error.
    `cargo test -p cobolt-mcp` is green (AC1 at the unit level, AC3 notifications, AC4).

- [ ] **T-A3 — Dependency audit** (R2; AC2)
  - Do: confirm `cobolt-mcp/Cargo.toml` still lists only `serde` and `serde_json`.
  - Verify: `cargo tree -p cobolt-mcp -e normal` shows no `rustls`, `ring`, `aws-lc`, `hyper`, `reqwest` or `ureq`. Record the output in the commit message.

### A.2 Transport, pool, lifetime (`cobolt-runtime`, feature `aws`)

- [ ] **T-A4 — Feature `aws` and the fake MCP server** (R29; test infrastructure for AC1, AC3, AC5, AC12, AC13, AC15)
  - Read first: `cobolt-runtime/Cargo.toml` `[features]`, `cobolt-form-host/Cargo.toml` `[features]`, and how existing test binaries are declared in the workspace.
  - Files:
    - `crates/cobolt-runtime/Cargo.toml`, `crates/cobolt-form-host/Cargo.toml` (`aws = []`, forwarded, in `default`);
    - `crates/cobolt-runtime/src/lib.rs` (`#[cfg(feature = "aws")] pub mod aws;`);
    - `crates/cobolt-runtime/src/bin/fake_mcp.rs` (new, `required-features = ["aws"]`, test-only by name and doc).
  - Do:
    - The fake is a stdio MCP server built on `cobolt_mcp::server` plus the T-A1 types. A JSON script (path in argv) declares its revision (handshake or stateless), tools (with `readOnlyHint`), canned answers, delays, notifications to send before an answer, and `crash_after`.
    - It appends one JSON line per received request to a call log, and writes its PID and argv marker to a file.
  - Verify: `cargo build -p cobolt-runtime --features aws --bin fake_mcp`; a smoke test runs it over pipes and lists its tools.

- [ ] **T-A5 — `aws/process.rs`: spawn, the built environment, stderr ring** (R7, R10, R11; AC7)
  - Read first: `cobolt-ide/src/form_runtime.rs` `BuiltAppRun::spawn`/`Drop` (the closest template), `cobolt-form-host/src/os_handoff.rs` (NotFound mapping).
  - Files: `crates/cobolt-runtime/src/aws/process.rs` (new).
  - Do:
    - `Command` with piped stdin and stdout; stderr drained by a thread into a 64 KiB ring.
    - `env_clear()`, then **only** `PATH`, `HOME`/`USERPROFILE`, `SystemRoot`/`TEMP` on Windows, `AWS_PROFILE`, `AWS_REGION`, `AWS_CONFIG_FILE`/`AWS_SHARED_CREDENTIALS_FILE` when set, and the route's declared variables. **Never** the access key id, secret access key or session token.
    - A spawn failure with `NotFound` becomes the plain-language R22 error, naming the program and the fix (install `uv`).
    - `mask(&str)` masks `AKIA`/`ASIA` key ids, 40-character secrets, session tokens and `X-Amz-Signature=` values (R27).
  - Verify:
    - `env_never_carries_aws_secrets`: plant the three secrets in the parent; the fake dumps its environment; assert none is present (AC7).
    - `mask_hides_planted_token`.
    - `missing_program_names_uv`.

- [ ] **T-A6 — `aws/pool.rs`: shared servers, timeouts, restart** (R5, R8; AC3, AC5)
  - Read first: T-A2, T-A5, `interpreter.rs` `async_result_tx` (how workers report).
  - Files: `crates/cobolt-runtime/src/aws/pool.rs` (new).
  - Do:
    - A process-global `OnceLock<Mutex<HashMap<(ConnId, ServerId), Handle>>>`. A `Handle` holds the child, the writer, a reader thread running `Session::pump`, and an `mpsc` reply per request id.
    - `call(conn, server, tool, args, timeout, start_timeout)`: start on first use, bounding the handshake by `StartTimeoutMs` (amendment A3); then send, wait on the reply with `timeout`, and return `Timeout` without hanging.
    - On EOF or a broken pipe, fail every pending call and drop the handle, so the next call spawns fresh.
  - Verify:
    - `two_calls_one_connection_one_process` (the fake's PID file is written once) (AC5);
    - `killed_server_errors_then_restarts` (AC5);
    - `silent_server_times_out_within_budget` (≤ `TimeoutMs` + 1 s) (AC3);
    - `progress_and_log_notifications_do_not_stall_a_call` (AC3);
    - `first_start_uses_start_timeout`.

- [ ] **T-A7 — No orphaned servers: EOF, orderly shutdown, OS backstop** (R9; AC6)
  - Read first: `cobolt-forms/src/text_scale.rs` (hand-declared Windows FFI pattern), the three hosts' exit paths:
    - `cobolt-cli/src/form_gui.rs`, where the run loop returns;
    - `cobolt-form-host/src/host.rs`, at shell exit;
    - `cobolt-compiler/src/lib.rs`, the generated `run_form_app` main.
  - Files: `aws/pool.rs` (`shutdown()`), `aws/job_windows.rs` (new, `#[cfg(windows)]`), `aws/process.rs` (Linux `pre_exec` with `PR_SET_PDEATHSIG` via a hand-declared `prctl`, and `process_group(0)` on Unix), and the three host exit paths.
  - Do:
    - `shutdown()` closes stdin, waits 2 s, then kills and waits. It is called from each host's exit and from a `Drop` guard owned by the interpreter.
    - Windows: every child is assigned to one Job Object created with `KILL_ON_JOB_CLOSE`.
    - No new crate.
  - Verify: `no_server_outlives_its_application` spawns a small host binary that starts the fake, then:
    - (a) exits normally;
    - (b) is killed with SIGKILL / `TerminateProcess`;
    - then scans for the fake's marker PID. It is run for the Run Form path, a child-form session and a built binary (the last in T-A16).
    The result is reported per platform the test ran on (AC6). The operator runs it on Windows and Linux before release, and the release notes record which platforms were measured.

### A.3 Route table

- [ ] **T-A8 — `routes.toml`, the loader, placeholders and override** (R14, R15, R16)
  - Files: `crates/cobolt-runtime/src/aws/{routes.rs, routes.toml}` (new).
  - Do:
    - Schema: `[servers.<id>]` has `command`, `args` (pinned `@version`), `env`, `readonly_args`, `write_args` and `protocol`. `[ops."<Type>.<Method>"]` has `server`, `tool`, `input`, `result`, `rows` and `mutating`.
    - Placeholders: `{arg:N}`, `{arg:N:json}`, `{prop:Name}`, `{Connection.X}`. Result paths: `$text`, `$isError`, `$json:/ptr`, `$rows:/ptr`.
    - A `routes_override` TOML string is merged per key.
    - Delivery A ships the `lambda` server (`awslabs.lambda-tool-mcp-server@2.1.1`) and an `AwsMcp.Call` generic op.
    - A grep test keeps every server or tool name out of `.rs` files (R15).
  - Verify: `routes_parse_and_every_op_names_a_server`, `placeholders_expand`, `override_replaces_one_key`, `no_server_or_tool_named_in_code`.

- [ ] **T-A9 — Recorded fixtures and the drift test** (R17; AC9)
  - Files: `crates/cobolt-runtime/tests/fixtures/aws-mcp/lambda.tools.json` (recorded with `uvx awslabs.lambda-tool-mcp-server@2.1.1` against an empty function selection, or from the pinned source's schema if no account is available, stating which in the fixture's header), `tests/aws_routes.rs` (new).
  - Do: for each op, check that the tool exists (or matches the dynamic shape, for Lambda) and that every `required` input of its `inputSchema` is filled by `input`.
  - Verify: `every_route_names_a_real_tool_with_its_required_inputs` is green. `a_renamed_tool_fails_the_route_test` runs against an in-test mutated copy and must fail the check (AC9).

### A.4 Model, runtime dispatch, the two controls

- [ ] **T-A10 — `AwsMcp` and `AwsLambda` in the model** (R18, R19, R25; AC10 in part)
  - Read first: `cobolt-forms/src/model.rs`:
    - `ControlType` and `ALL`, `as_str`, `from_str`, `default_size`, `primary_event`, `supported_events`, `is_non_visual`;
    - the WebSearch arm of `Control::new`;
    - `runtime_property_names_for`.
    Also `tests/non_visual_controls_stay_in_the_designer.rs`.
  - Files: `crates/cobolt-forms/src/model.rs`, that test file, `crates/cobolt-ide/src/panels/designer.rs` `control_type_name` (it has no wildcard, so it will not compile without the new arm).
  - Do:
    - Common seeds: `Connection`, `Mode=Async`, `Busy`, `TimeoutMs=30000`, `StartTimeoutMs=120000`, `AllowWrite=false`, `Verbose=false`, `LastError`, `ResponseBody`, `ResultJson`, `RowCount`.
    - `AwsLambda` adds `FunctionName` and `FunctionError`. `AwsMcp` adds `ServerId` and `ToolName`.
    - Events: `onInvoked` / `onToolResult`, then `onComplete`, `onError`, `onTimeout`, `onCancelled`.
    - Add an `AWS_ASYNC` runtime-names const.
  - Verify: `cargo test -p cobolt-forms --features render` is green (Gate G unchanged), and `aws_controls_are_non_visual_and_round_trip_a_cfrm` passes.

- [ ] **T-A11 — `interpreter/aws.rs`: class-first dispatch and the async path** (R19–R21, R25, R26; AC11, AC12, AC15)
  - Read first: `interpreter/kb.rs` (`is_knowledge_base`, `kb_method`, the non-`kb` stub, `kb_delivered`), and `interpreter.rs`:
    - `exec_method` and its class routing;
    - `spawn_rest_op`;
    - `drain_async_ops`;
    - the timeout sweep;
    - `cancel_async_op`;
    - `completion_event_for`;
    - `queue_control_event`.
    Also `async_op.rs` and `cobolt-ast/src/methods.rs`, plus its guard test.
  - Files: `crates/cobolt-runtime/src/interpreter/aws.rs` (new), `interpreter.rs`, `async_op.rs`, `crates/cobolt-ast/src/methods.rs`.
  - Do:
    - `is_aws`, and `aws_method(obj, m, args) -> Option<String>`, called before the global match. Add a `#[cfg(not(feature = "aws"))]` stub that raises `onError`.
    - Methods: `AwsLambda.Invoke(name, payloadJson)`; `AwsMcp.Call(tool, argsJson)` and `AwsMcp.ListTools()`; `Cancel`, `IsBusy`, `GetRow(i)`, `GetField(i, name)`.
    - Validate the JSON before sending (R21). Refuse a mutating op while `AllowWrite` is off (R25). For `AwsMcp`, refuse any tool without `readOnlyHint` while `AllowWrite` is off (R26).
    - Add `AsyncOutcome::Aws { props, rows, event }`, delivered with a per-event payload (the `kb_raise` pattern).
    - `Mode=Sync` runs inline.
    - `Verbose` logs through `log_block` with `mask()`.
  - Verify: runtime tests against the fake:
    - `invoke_raises_oninvoked_then_oncomplete` (order asserted);
    - `invalid_json_names_the_argument_and_sends_nothing` (the call log is empty) (AC12);
    - `allowwrite_off_refuses_before_sending` for `Invoke` and for a non-read-only `AwsMcp` tool (AC15);
    - `allowwrite_on_sends`;
    - `a_stale_result_after_cancel_is_discarded`;
    - `sync_mode_returns_the_body`.
    Then `cargo test -p cobolt-runtime --features aws` is green, and the `is_known_method` guard test is green.

- [ ] **T-A12 — Read-only servers start read-only** (R25 as amended A4; AC15)
  - Do: when spawning, append `readonly_args` unless some control on that connection has `AllowWrite`. In that case append `write_args`, and restart the server if it is running read-only.
  - Verify: `a_readonly_connection_starts_the_server_readonly` (the fake records its argv) and `enabling_allowwrite_restarts_with_write_args`.

- [ ] **T-A13 — Prerequisite and profile diagnostics** (R22, R23; AC13)
  - Files: `aws/diagnose.rs` (new), `aws/process.rs`.
  - Do:
    - Program missing gives the message from T-A5.
    - When the server's stderr or its tool error matches the credential-chain failures (`ExpiredToken`, `NoCredentialProviders`, "The SSO session … has expired", "Unable to locate credentials"), `LastError` reads *"The AWS profile "<p>" is not signed in or has expired. Run: aws login --profile <p>"*. The server's raw text is never shown.
    - `diagnose(&conn) -> Report` covers four outcomes: program missing, profile not signed in, route mismatch, all good.
  - Verify: `path_without_uvx_reports_the_fix`; `a_rejected_profile_reports_aws_login` (the fake emits the credential error) (AC13); `diagnose_reports_each_outcome` (backs AC14).

### A.5 Connections and hosts

- [ ] **T-A14 — `AwsConnection`, the manifests and catalogue publishing in three hosts** (R12, R13, R28; AC8)
  - Read first: `cobolt-forms/src/connections.rs` (`SearchConnection`, `apply_search`, `Catalogue`, `resolve_search_all`); both manifest copies (`cobolt-ide/src/project_model.rs` `ProjectIntegrationSettings`, `cobolt-compiler/src/lib.rs` `ProjectIntegrations` and `project_connections`); `cobolt-form-host/src/seeding.rs` (`publish_search_connections`, `resolve_connections`, the per-type seeds); `cobolt-cli/src/form_gui.rs` (catalogue publishing); the compiler's baked `PROJECT_CONNECTIONS` and its read-back.
  - Files: all of the above.
  - Do:
    - `AwsConnection { id, name, profile, region, function_prefix, function_list, routes_override }` has **no secret field**.
    - Add `Catalogue.aws` (`#[serde(default)]`), `publish_aws_connections`, and the `_ResolvedAwsProfile`/`_ResolvedAwsRegion` seeds.
    - Each of the three hosts publishes the AWS list.
    - Add `[[integrations.aws_connections]]` to both manifests.
  - Verify:
    - `aws_connection_round_trips_through_cobolt_toml` (both copies);
    - `no_credential_field_in_cfrm_or_toml`, which scans the saved files for key, secret and token names (AC8);
    - the compiler's `PROJECT_CONNECTIONS` tests extended with `aws`.
    - Gate P names the three hosts.

- [ ] **T-A15 — Build feature detection** (R29; AC17)
  - Read first: `cobolt-compiler/src/runtime_features.rs` (the struct, `all`, `union`, `as_toml_features`, `scan_forms`, `scan_rust`, and its tests), and `base_dependency_block`.
  - Files: `runtime_features.rs`, `lib.rs` (the "no X reached" log line).
  - Do: add `aws` to the struct and every helper. `scan_forms` sets it for any `Aws*` type; `scan_rust` sets it for `cobolt_runtime::aws`.
  - Verify: `a_form_without_aws_controls_builds_without_aws` and `an_aws_control_turns_the_feature_on` (AC17), plus `cargo test -p cobolt-compiler`.

- [ ] **T-A16 — The same COBOL programs on three hosts** (R28; AC6 for the binary, AC11, AC16)
  - Files: `tests/cobol/aws/aws-lambda-demo.{cfrm,cbl}` and `aws-mcp-demo.*` (new), `crates/cobolt-runtime/tests/aws_hosts.rs` (new), wired into existing harnesses as the `props_demo_runs.rs` pattern shows.
  - Do: each form drives every Delivery A operation against the fake (through a connection whose route override points at `fake_mcp`) and prints the GOLDEN RULE #7 block. The block gives:
    - the operations exercised, by name;
    - calls made;
    - ms per call;
    - the event order seen;
    - the pass/fail tally.
  - Verify: the programs pass under `rcrun run-form`, as an embedded child form, and as a built binary, which also runs the orphan check from T-A7 (AC6, AC11, AC16). The commit names the three hosts.

### A.6 IDE, knowledge, docs

- [ ] **T-A17 — Toolbox category, glyphs, canvas cards** (R18, R30; AC10)
  - Read first: `panels/toolbox.rs` (`TOOLS`, `CATEGORIES`, `TREE_CATEGORY_ORDER`, `category_of`, `paint_control_icon`, and `the_toolbox_snackbar_is_the_controls_own_glyph`); `cobolt-forms/src/paint.rs` (the non-visual card branch and `nv_icon_*`); `i18n.rs` (`cat_*` and `category_name`).
  - Files: those three, plus `crates/cobolt-ide/src/i18n.rs` (`cat_aws` in 6 languages; the English value is "AWS" in every language).
  - Do:
    - `nv_icon_aws_lambda` and `nv_icon_aws_mcp` are original line glyphs, drawn by both the toolbox and the card.
    - Add the "AWS" category after NonVisual.
  - Verify: the `toolbox_layout_tests` gain `the_aws_category_lists_its_controls_in_every_language`; the glyph-parity test is extended; Gate G.

- [ ] **T-A18 — Inspector rows and automatic connection** (R13, R30)
  - Read first: `panels/properties.rs` (the non-visual early return, and the WebSearch arm's Configuration combo and `set_search_connections`), `app.rs`'s per-frame connection plumbing, and `designer.rs`'s drop path.
  - Files: `properties.rs`, `app.rs`, `designer.rs`.
  - Do:
    - An `Aws*` arm shows a Connection combo listing AWS connections, then the type's rows.
    - A dropped AWS control in a one-connection project gets that connection, in the same undo step.
  - Verify: `a_dropped_aws_control_takes_the_only_connection`; the Props-tab label-help test (`label_help_tests`) is green for the new types.

- [ ] **T-A19 — Settings → Integrations: AWS connections and Test connection** (R12, R24; AC14)
  - Read first: `panels/settings_form.rs` (the Search connections list: draft, load, save and UI), and `panels/models_modal.rs` `do_test` and its result drain (the threaded test pattern).
  - Files: `settings_form.rs`, `i18n.rs` (the section title, field labels, Test button, and the four outcomes, each ×6).
  - Do:
    - List, add and remove connections, with fields for name, profile, region, function prefix/list and route override.
    - **Test connection** runs `cobolt_runtime::aws::diagnose` on a thread and shows the outcome in the IDE language. It lists the Lambda functions found (Q4).
    - The window keeps its fixed size (GOLDEN RULE).
  - Verify: `aws_test_connection_reports_four_outcomes`, driven with the fake through `diagnose` rather than the UI (AC14); the `i18n_tests` are green.

- [ ] **T-A20 — Help, System KB, IntelliSense, Grace** (R30, R31; AC18)
  - Read first: `prop_help_data.rs` (the WebSearch entries); `cobolt-compiler/src/lib.rs` `property_reference_for`, `event_reference`, `control_purpose`, `control_method_docs`, `control_usage_notes`, `methods_reference_doc` (a hand-kept list) and `every_control_property_is_documented` (a hand-kept type list); `grace_host.rs` `type_aliases`; `agent.rs` `ALL_CONTROL_TYPES`; `crates/cobolt-runtime/tests/test_nonvisual_property_readers.rs` (`declared_readers`, `RUNTIME_SOURCES`).
  - Do:
    - Add every seeded property and event, in 6 languages.
    - Update every KB table, and add the AWS types to **each hand list** named above (the survey showed stale hand lists are where gaps hide).
    - Add `interpreter/aws.rs` to `RUNTIME_SOURCES`.
    - Regenerate `chunked.data`.
  - Verify (Gate K), all green:
    - `prop_help_tests`;
    - `every_control_property_is_documented`;
    - `intellisense_offers_every_method_the_knowledge_base_documents`;
    - `test_nonvisual_property_readers`;
    - `prebuilt_chunked_kb_matches_the_published_documentation`;
    - Grace's `names_type("add a lambda control", "AwsLambda")`.
    These cover AC18 for Delivery A.

- [ ] **T-A21 — Developer's Guide: "Calling AWS", part 1** (R32; AC19)
  - Files: `docs/developers-guide-en.md`, `docs/cobol-support-matrix-en.md`. Delete the five translations of each (GOLDEN RULE #8).
  - Do: write for a PowerCOBOL or isCOBOL developer, in COBOL only. Cover:
    - prerequisites (`uv`, Python ≥ 3.10, AWS CLI ≥ 2.32 and `aws login`);
    - AWS connections and profiles;
    - the read-only rule;
    - the first-start download;
    - what an end user installs;
    - a worked `AwsLambda` example and an `AwsMcp` example;
    - a mermaid diagram of the call path;
    - `📷 Screenshot needed` placeholders for the connections section and a card.
  - Verify: the Guide's COBOL examples pass `rcrun check` (AC19); `docs_embed` shows only the expected reds.

- [ ] **T-A22 — Delivery A finalize**
  - Run Gate F; `cargo tree` for `cobolt-runtime --features aws` (AC2, no `rustls`, `aws-lc` or `ring`); a CHANGELOG entry per commit.
  - The operator's manual check: drop both controls, set a connection, run Test connection. The agent does not drive the IDE.
  - Delivery A can ship here.

---

## Delivery B — dedicated-server controls: `AwsKnowledgeBase`, `AwsAgentCore`, `AwsAgentMemory`, `AwsS3Tables`, `AwsGlue`

Each task below repeats the A10 → A11 → A17 → A18 → A20 path for its controls. That means: the model, dispatch and ops, the route entries with a recorded fixture, glyphs, inspector rows, help in 6 languages, the KB, and a COBOL demo on three hosts. Gates K, P and G apply to each.

- [ ] **T-B1 — Routes and fixtures for the four servers** (R14, R17; AC9)
  - Servers, each with its version pinned:
    - `bedrock-kb-retrieval-mcp-server@1.1.2` (`QueryKnowledgeBases`, `ListKnowledgeBases`);
    - `amazon-bedrock-agentcore-mcp-server@0.2.1`, started with `AGENTCORE_ENABLE_TOOLS` limited to `invoke_agent_runtime`, `memory_create_event` and `memory_retrieve_records`;
    - `s3-tables-mcp-server@0.1.1` (`query_database`, `list_tables`, `append_rows_to_table`, `--allow-write` as `write_args`; the route records Python ≥ 3.11);
    - `aws-dataprocessing-mcp-server@0.2.2` (`manage_aws_glue_jobs`, `manage_aws_glue_crawlers`, `manage_aws_glue_tables`, `--allow-write` as `write_args`).
  - Verify: the drift test passes against four recorded fixtures, each with its provenance in the header.
- [ ] **T-B2 — `AwsKnowledgeBase`**: `Query(kbId, text)` gives passages as rows (`Text`, `Source`, `Score`); `ListKnowledgeBases()` (R18, R20).
- [ ] **T-B3 — `AwsAgentCore`**: `Invoke(runtimeArn, prompt, sessionId)` puts the reply in `ResponseBody`. Mutating, so `AllowWrite` applies (R18, R25).
- [ ] **T-B4 — `AwsAgentMemory`**: `RecordEvent(memoryId, actorId, sessionId, text)` is mutating; `Retrieve(memoryId, namespace, query)` gives rows (R18).
- [ ] **T-B5 — `AwsS3Tables`**: `ListTables(bucketArn, namespace)` and `Query(sql)` give rows; `AppendRows(table, rowsJson)` is mutating (R18).
- [ ] **T-B6 — `AwsGlue`**: `StartJobRun(job, argsJson)` and `StartCrawler(name)` are mutating; `GetJobRun(job, runId)` gives `State`; `GetTableSchema(db, table)` gives rows (R18).
- Each of T-B2 to T-B6 is verified by the same set:
  - runtime tests against the fake: the operation, the event order, and a mutating refusal where it applies (AC11, AC15);
  - a `tests/cobol/aws/<control>-demo` program on three hosts (AC16);
  - the toolbox, inspector and help tests;
  - Gate K.
- [ ] **T-B7 — Guide part 2**: one worked example per Delivery B control; the translations are deleted again.
- [ ] **T-B8 — Delivery B finalize**: Gate F, `cargo tree`, the operator's manual check. Delivery B can ship here.

---

## Delivery C — hosted-server controls: `AwsDynamoDB`, `AwsS3`, `AwsS3Vectors`, `AwsRekognition`, `AwsPolly`, `AwsComprehend`, `AwsTextract`, `AwsEC2`, `AwsCognito`

- [ ] **T-C0 — Probe the hosted AWS MCP Server** (Q1; **needs the operator's AWS profile**)
  - Do:
    - With the operator present, run `uvx mcp-proxy-for-aws-cli@<pin> https://aws-mcp.us-east-1.api.aws/mcp --profile <p> --read-only` through the T-A2 client, and record `tools/list` into `fixtures/aws-mcp/hosted.tools.json`.
    - Decide and record the answer in `plan.md` §9: `aws___call_aws` (with its schema) if it is listed, otherwise `aws___run_script` templates.
  - Verify: the operator confirms the decision before T-C1 starts. **The agent does not proceed on an assumption.**
- [ ] **T-C1 — The hosted route and its per-operation mapping** (R14–R17)
  - If `call_aws`: each op maps COBOL arguments to that tool's input.
  - If `run_script`: each op carries a **product-shipped** Python template in `routes.toml`. Parameters go in as JSON only, and **no control sends script text built from COBOL data** (plan §4).
  - Verify: the drift test against `hosted.tools.json`, plus `no_op_accepts_script_text_from_cobol`.
- [ ] **T-C2 — `AwsDynamoDB`**: `GetItem`, `Query` and `Scan(limit)` read; `PutItem`, `UpdateItem` and `DeleteItem` are mutating.
- [ ] **T-C3 — `AwsS3`**: `GetObject(bucket, key, toFile | toDataItem)` and `List(prefix)` read; `PutObject` and `DeleteObject` are mutating. Large transfers go through `aws___get_presigned_url` where the route says so.
- [ ] **T-C4 — `AwsS3Vectors`**: `QueryVectors(index, vectorJson, topK)` gives rows; `PutVectors` is mutating.
- [ ] **T-C5 — `AwsRekognition`**: `DetectLabels`, `DetectText` and `DetectFaces`, each from a file or an S3 object; results as rows.
- [ ] **T-C6 — `AwsPolly`**: `Synthesize(text, voiceId, format, toFile)`. It writes a local file, and it is read-only in AWS.
- [ ] **T-C7 — `AwsComprehend`**: `DetectSentiment`, `DetectEntities`, `DetectKeyPhrases`, `DetectLanguage`.
- [ ] **T-C8 — `AwsTextract`**: `DetectText` and `AnalyzeDocument(forms, tables)`, from a file or S3; key/value pairs and table cells as rows.
- [ ] **T-C9 — `AwsEC2`**: `Describe(ids)` reads; `Start(ids)` and `Stop(ids)` are mutating.
- [ ] **T-C10 — `AwsCognito`** (Q6): `SignUp`, `Confirm`, `SignIn`, `SignOut`; `SignedIn`, `UserName` and `GetAttribute(name)`. Tokens live only in `aws/cognito.rs` memory.
  - Verify: `no_cognito_token_reaches_a_property_or_disk`, which scans every property and the working directory after a sign-in against the fake.
- Each of T-C2 to T-C10 is verified by the same set as Delivery B (AC11, AC15, AC16, Gate K).
- [ ] **T-C11 — Guide part 3**: examples per control; the translations are deleted.
- [ ] **T-C12 — Live smoke test** (AC20)
  - `crates/cobolt-runtime/tests/aws_live.rs` is `#[ignore]` and runs only with `COBOLT_AWS_LIVE=1`. It makes one read-only call per service and prints a table of what answered and how fast.
  - Verify: the operator runs it with their profile; it is skipped otherwise.
- [ ] **T-C13 — Finalize the feature**
  - Gate F, and AC2's `cargo tree`.
  - The orphan test (AC6) is reported for the platforms measured.
  - All 16 controls are shown in the toolbox in six languages (AC10).
  - The operator's manual check.
  - The spec's acceptance boxes are ticked with the evidence.

---

## Acceptance-criterion coverage

| AC | Tasks |
|---|---|
| AC1 client, both revisions | T-A2, T-A4, T-A6 |
| AC2 no TLS / C crates | T-A3, T-A22, T-B8, T-C13 |
| AC3 notifications, timeout | T-A2, T-A6 |
| AC4 client ↔ server wire parity | T-A1, T-A2 |
| AC5 one process, restart | T-A6 |
| AC6 no orphans | T-A7, T-A16 |
| AC7 no secrets travel, masking | T-A5 |
| AC8 connections, no credential saved | T-A14 |
| AC9 route drift test | T-A9, T-B1, T-C1 |
| AC10 toolbox AWS category | T-A17, T-C13 |
| AC11 COBOL programs per control | T-A11, T-A16, T-B2…B6, T-C2…C10 |
| AC12 invalid JSON | T-A11 |
| AC13 missing uvx, bad profile | T-A13 |
| AC14 Test connection outcomes | T-A13, T-A19 |
| AC15 AllowWrite / read-only start | T-A11, T-A12, B and C controls |
| AC16 three hosts | T-A16, B and C demos |
| AC17 feature trimmed | T-A15 |
| AC18 help, KB, IntelliSense | T-A20, and each B and C control |
| AC19 Guide | T-A21, T-B7, T-C11 |
| AC20 live smoke | T-C12 |

## Done criteria

- Every acceptance criterion is checked, with the evidence (the test name and its `test result:` line) recorded in this file.
- Gate F is green except for the named expected reds.
- The docs are updated and their translations deleted.
- Each delivery is committed separately, as a feature, with a `z` bump and a CHANGELOG entry per commit.
- Nothing is pushed or merged unless the operator asks.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Threat model — the WebAssembly line (WASM-013 R1, T23)

**Written 2026-11-06** on `features-2x-wasm-pending`, after 2.0.275. The model is a spec file and a test:
`crates/cobolt-web-bundle/tests/threat_model.rs` reads this file and fails when a boundary has no row, a row has no
control, a named test does not exist, or the list of secret-scan patterns below is not the one the scan uses. Each later
WASM spec's `/analyze` updates it (R1); the test prints the **owed count** so the rows still waiting on another spec stay
visible, and the count must fall.

**How to read a row.** `id | boundary | actor | threat | control | test`. The threat carries its STRIDE letter in brackets
(**S**poofing, **T**ampering, **R**epudiation, **I**nformation disclosure, **D**enial of service, **E**levation of
privilege); the walk of every boundary through the six letters is the matrix in §5. A test is written
`` `crate::function_name` `` and the test file looks for `fn function_name` in that crate's sources; a row whose test is another
spec's to write says `owed: WASM-NNN` and what is owed, and names no test.

## 1. Assets

| id | asset | where it lives |
|---|---|---|
| S1 | **Accounts** — who may sign in, their credentials at the identity provider | the identity provider; the backend holds none (WASM-009) |
| S2 | **Tokens** — access, identity and refresh tokens | the page's memory; the `__Host-pr_refresh` cookie (web) or the OS secure store (mobile); the backend's key cache |
| S3 | **Application data** — what the developer's programs read and write | the developer's backend and storage |
| S4 | **Secrets** — keys, connection strings, provider client secrets, signing keys | `backend.toml` (as `env:NAME` or `file:PATH`), the environment, the OS store |
| S5 | **The developer's source** — forms, COBOL, stylesheets, the project folder | the developer's machine; what a Build copies into a bundle |

## 2. Actors

| id | actor | what they can do |
|---|---|---|
| A1 | An anonymous user | open the application, send frames to the backend, hold no token |
| A2 | A signed-in user | everything A1 can, with a valid token and its roles |
| A3 | A malicious page that embeds or opens the application | script of its own origin, a victim's browser and its cookies, a frame around the application |
| A4 | A malicious backend client | any bytes on the WebSocket and the HTTPS surface, from any machine |
| A5 | A malicious dependency | code that runs in the page, the backend, the Build or the IDE |
| A6 | A malicious project file opened in the IDE | a manifest, a form, a stylesheet or a copybook written to make the Build or the Preview do something the developer did not ask for |
| A7 | A party that holds or terminates the TLS session | read, replay, reorder and inject frames between the browser and the backend (R17) |

## 3. The seven boundaries

| id | boundary | what crosses it |
|---|---|---|
| B1 | **browser ⇄ backend** | frames on one WebSocket (sealed after the `hello`s) and the HTTPS routes; the backend process and its dependencies |
| B2 | **host page ⇄ module** | the generated page, its policy (CSP, Trusted Types, isolation), the loader, the program module's imports |
| B3 | **module ⇄ player** | the player's view of the module: events, writes, texts that become nodes, the console |
| B4 | **IDE ⇄ preview web view** | the development server and the page it serves to the IDE's web view, the events the page posts back |
| B5 | **mobile shell ⇄ frontend** | the native shell's capabilities, pinning, the signing and store keys (WASM-012) |
| B6 | **backend ⇄ identity provider** | discovery, signing keys, the hosted flow, the provider's secrets |
| B7 | **backend ⇄ data** | files, SQL, REST, what a program may touch; uploads and downloads; the backend's own configuration |

## 4. The table

| id | boundary | actor | threat | control (rule and code path) | test |
|---|---|---|---|---|---|
| TM-01 | B1 | A3 | **S** A foreign page opens a WebSocket from the victim's browser and calls the backend as the victim | WebSocket `Origin` is checked against the configured list at the upgrade and counted when refused (R6; WASM-003 R14): `cobolt-server/src/serve.rs` `origin_allowed` | `cobolt-server::a_foreign_origin_is_refused_and_plain_ws_is_loopback_only` |
| TM-02 | B1 | A4 | **S** A token the configured key did not sign is accepted: `alg: none`, HS256 signed with the public key, another issuer's key, a wrong `kid`, a header extension | the algorithm list comes from the configuration and never holds `none` or a shared secret; the key is found by `kid`; a `crit` header is refused even when the key signed it (WASM-003 R10; `cobolt-server/src/jwt.rs`) | `cobolt-server::no_token_the_configured_key_did_not_sign_is_accepted_and_nothing_a_stranger_sends_gets_past_a_named_refusal`; `cobolt-server::an_unsigned_token_and_a_shared_secret_token_are_never_accepted`; `cobolt-server::a_key_that_names_its_algorithm_is_not_used_for_another`; `cobolt-server::a_header_that_demands_an_extension_is_refused_even_when_the_key_signed_it`; `cobolt-server::no_and_shared_secret_algorithms_are_never_accepted_from_a_token` |
| TM-03 | B1 | A4 | **S** A genuine token is used where it does not belong: another application's, an identity token as an access token, one that has expired, one whose `exp` is not a whole number | `iss`, `aud` or client id, `exp`/`nbf` with a bounded skew, `token_use` and `typ` are judged on every use, not only when the key is fetched (`jwt.rs`) | `cobolt-server::the_issuer_and_the_audience_must_be_this_applications`; `cobolt-server::an_identity_token_is_never_an_access_token`; `cobolt-server::expiry_and_not_before_are_judged_with_a_bounded_skew`; `cobolt-server::a_token_that_was_verified_is_judged_again_for_expiry_when_it_is_used_again`; `cobolt-server::an_exp_that_is_not_a_whole_number_of_seconds_is_refused_not_read_as_zero` |
| TM-04 | B1 | A2 | **E** A caller runs a program that is not exported, or one its role does not allow, or an identity program through the WebSocket | only the Build's and the developer's exports answer; the role is checked before the program starts; the identity programs are not on the WebSocket (`cobolt-server/src/backend.rs`, `front.rs`) | `cobolt-server::an_exported_program_answers_and_nothing_else_does`; `cobolt-server::a_role_is_checked_before_the_program_starts`; `cobolt-server::the_identity_programs_are_not_on_the_websocket`; `cobolt-server::a_developer_export_of_an_identity_program_is_refused_naming_both_files` |
| TM-05 | B1 | A4 | **T** Malformed, oversize, binary or mutated frames, a frame written as a positional array, a hello sent twice | one strict reader for both sides (a message is a JSON object, unknown kinds are named refusals); every connection end carries the close code that names it; a second hello is refused (`cobolt-wire/src/proto.rs`, `cobolt-server/src/serve.rs`); fuzzed (R15) | `cobolt-wire::random_and_mutated_frames_never_panic_and_every_refusal_is_named`; `cobolt-wire::a_text_that_is_not_a_json_object_is_never_a_frame`; `cobolt-wire::every_byte_of_every_seed_changed_to_each_byte_that_matters_and_cut_at_every_boundary`; `cobolt-server::mutated_frames_never_break_the_dispatch_and_nothing_secret_reaches_the_log`; `cobolt-server::every_way_to_break_the_transport_ends_with_the_close_code_that_names_it` |
| TM-06 | B1 | A4 | **D** Floods and oversize: a message over the limit, a slow request, thousands of pings or hellos, more connections than the backend serves | `max_frame_bytes`, size, time and rate limits each with its own error, a connection cap with the slots returned, the handshake deadline, the request-head limit (`cobolt-server/src/limits.rs`, `serve.rs`) | `cobolt-server::size_time_and_rate_limits_are_each_refused_with_their_own_error`; `cobolt-server::a_payload_that_does_not_fit_is_refused_and_the_program_does_not_run`; `cobolt-server::a_large_or_slow_request_head_is_refused`; `cobolt-server::floods_are_absorbed_and_the_slots_come_back` |
| TM-07 | B1 | A7 | **I** A TLS endpoint or terminating proxy reads the frames; a captured session is replayed against a later start of the backend | after the two `hello`s every frame is one ChaCha20-Poly1305 message under keys from an ephemeral X25519 agreement; new key pairs at each start (R17; `cobolt-wire/src/seal.rs`) | `cobolt-server::the_channel_is_ciphertext_at_a_terminating_proxy`; `cobolt-wire::two_connections_of_the_same_key_pairs_derive_different_keys`; `cobolt-wire::a_frame_from_another_connection_is_refused` |
| TM-08 | B1 | A7 | **T** A party that holds the TLS session injects, replays, reorders or truncates a sealed frame, skips the exchange, restarts it mid-connection or offers a low-order key | the AEAD tag and the counter nonce refuse the frame; a connection that skips the exchange is refused; a second hello is refused; a low-order peer key is refused (`seal.rs`, `serve.rs`) | `cobolt-wire::a_frame_that_was_tampered_replayed_reordered_or_truncated_is_refused`; `cobolt-wire::a_low_order_peer_key_is_refused`; `cobolt-server::a_connection_without_the_exchange_is_refused`; `cobolt-server::the_frame_limit_is_the_plaintext_limit` |
| TM-09 | B1 | A7 | **S** A party that rewrites the two `hello`s stands in the middle of the unauthenticated key agreement | **accepted residual risk R-1**: only TLS's server authentication stops it, so the channel sits on top of TLS and never instead of it, and a plain `ws://` start is loopback-only (R17; WASM-003 R5) | `cobolt-server::plain_ws_is_never_served_on_a_non_loopback_address` |
| TM-10 | B1 | A7 | **I** TLS below the floor or a response without the security headers | TLS 1.2 or later (the cipher list is the operating system's: **accepted residual risk R-2**, plan O5); `Strict-Transport-Security`, `X-Content-Type-Options`, `Referrer-Policy`, `Permissions-Policy` on the HTTPS surface (R4; `cobolt-server/src/tlsutil.rs`, `http.rs`) | `cobolt-server::a_client_offering_nothing_above_tls_11_is_refused_and_a_modern_one_gets_every_security_header`; `cobolt-server::the_health_probe_answers_on_the_tls_port_and_carries_the_security_headers` |
| TM-11 | B1 | A4 | **I** A token, a password or a field the program was given reaches the backend's log | the log carries the connection, program, subject, duration and outcome and nothing the caller sent (R13; `cobolt-server/src/log.rs`) | `cobolt-server::a_development_sign_in_and_a_call_with_a_password_leave_neither_in_the_log`; `cobolt-server::the_log_has_neither_the_token_nor_a_field_the_program_was_given`; `cobolt-server::the_identity_routes_log_no_token_password_code_or_cookie` |
| TM-12 | B1 | A4 | **I** An error answer repeats a stranger's text, a secret or an internal detail | every parse reason is cut at 200 characters and every platform error message at 400; a refusal names parameters by position, never by text (`cobolt-wire/src/proto.rs`, `error.rs`) | `cobolt-wire::a_refusal_never_repeats_a_strangers_text_at_length`; `cobolt-wire::random_and_mutated_frames_never_panic_and_every_refusal_is_named` |
| TM-13 | B1 | A2 | **R** A caller disputes a call | each call record names the subject, the program and the duration; the subject is personal data (§6) kept as long as the deployment keeps logs (`log.rs`) | `cobolt-server::a_development_sign_in_and_a_call_with_a_password_leave_neither_in_the_log` |
| TM-14 | B1 | A3 | **E** A cross-site page makes the backend run a state-changing call with only the refresh cookie; a wildcard origin is allowed together with credentials | a state-changing route needs a bearer token or the custom header the browser will not add cross-site (`X-PR-Identity`), JSON content type and an allowed `Origin`; a wildcard origin is not an origin and the CORS grant is written only inside `cors_headers` (R6; AC5; `cobolt-server/src/http.rs`, `identity/`) | `cobolt-server::the_post_checks_refuse_what_a_cross_site_page_could_send`; `cobolt-server::refresh_and_sign_out_go_through_the_cookie`; `cobolt-server::origins_must_be_listed_and_a_wildcard_is_not_an_origin`; `cobolt-server::the_cors_grant_headers_are_written_only_inside_cors_headers` |
| TM-15 | B1 | A4 | **I** A token, a code or the refresh token appears in a URL, or the page can read the refresh token | the hosted flow returns a single-use handle, never a token in a URL; the refresh token is a sealed `HttpOnly` cookie the page never sees (WASM-009) | `cobolt-server::the_hosted_flow_returns_a_single_use_handle_and_never_a_token_in_a_url`; `cobolt-server::sign_in_sets_the_sealed_cookie_and_the_page_never_sees_the_refresh_token` |
| TM-16 | B1 | A4 | **I** Sign-in answers tell an existing account from one that does not | an unknown user and a wrong password answer the same in body and time; a reset for an unknown address is the reset for a known one (WASM-009) | `cobolt-server::an_unknown_user_and_a_wrong_password_look_the_same_in_body_and_time`; `cobolt-server::a_reset_for_an_unknown_address_is_the_reset_for_a_known_one` |
| TM-17 | B1 | A5 | **T** A dependency of the backend links C-built crypto, a second runtime or an unlisted crate; a known advisory or a licence outside the list enters the lock | the dependency-guard tests, `deny.toml` with the advisory baseline and the licence allow-list, the lock files committed (R9; AC8) | `cobolt-server::the_backend_is_headless_and_links_no_c_built_crypto`; `cobolt-server::the_sealed_channel_is_in_the_server_and_the_client_and_not_in_the_loader_or_the_compiler`; `cobolt-compiler::the_four_sections_pass_on_both_locks_and_the_check_can_fail` |
| TM-18 | B2 | A3 | **E** An injected inline script, an `eval` or a connection to an address outside `connect-src` runs in the page | a default-deny Content-Security-Policy generated at every Build: scripts from the origin and `'wasm-unsafe-eval'`, no `'unsafe-eval'`, no inline script, `connect-src` limited to the configured backend, `base-uri` and `form-action` restricted (R2; AC2; `cobolt-web-bundle/src/csp.rs`) | `cobolt-web-bundle::the_browser_enforces_the_policy_and_reports_what_it_blocked`; `cobolt-web-bundle::every_page_carries_the_file_notice_and_the_policy_allows_only_its_one_script`; `cobolt-web-bundle::without_a_site_key_no_google_origin_is_opened` |
| TM-19 | B2 | A5 | **T** The bundle's own JavaScript evaluates a string, builds a function from text or inserts markup | the host page's script is generated by the IDE and searched for `eval`, `new Function`, string timers, `innerHTML` and the like, threaded glue included (R5, R9; AC4) | `cobolt-web-bundle::no_javascript_of_the_bundle_evaluates_a_string_or_inserts_markup`; `cobolt-web-bundle::the_threaded_players_glue_and_worker_helper_are_clean_too`; `cobolt-web-bundle::the_crate_keeps_no_javascript_html_or_css_source` |
| TM-20 | B2 | A3 | **T** A file of the bundle is replaced after the Build, or a directive that only a header can carry is believed to be in the `<meta>` | integrity hashes on every file the loader fetches; the Build prints which directives are only deliverable as headers and keeps them out of the `<meta>` (R3, R10; AC3) | `cobolt-web-bundle::the_browser_enforces_the_policy_and_reports_what_it_blocked`; `cobolt-web-bundle::the_report_lists_the_header_only_directives_and_keeps_them_out_of_the_meta`; `cobolt-web-bundle::report_only_mode_and_the_report_uri_are_header_only_too` |
| TM-21 | B2 | A3 | **I** A hostile page embeds the application, or times it, or a cross-origin resource loads under isolation without opting in | `frame-ancestors` by the embedding mode; cross-origin isolation (`COOP`, `COEP`) when the page offers it, a plain path when it does not; the host set per documented host (R15b; WASM-017) | `cobolt-web-bundle::embedding_isolation_and_cross_origin_resources_in_chrome`; `cobolt-web-bundle::each_documented_host_carries_the_headers_of_its_isolation_setting`; `cobolt-web-bundle::the_shim_page_names_its_trusted_types_policy_and_a_page_without_the_shim_does_not` |
| TM-22 | B2 | A6 | **I** A secret, a connection string or a key written in a form, a copybook, a stylesheet, a text asset or the bundle ships to every browser that opens the application | the secret scan belongs to the product, runs over forms, Common Code and copybooks, stylesheets, text assets and the bundle, fails the Build with file and pattern and never prints the value; the backend's own COBOL is not scanned (R7; AC6; `cobolt-web-bundle/src/secrets.rs`, `generator::gate_inputs`) | `cobolt-project-tools::every_error_pattern_in_a_form_a_copybook_and_a_stylesheet_stops_the_build`; `cobolt-project-tools::secret_in_a_wcfrm_property_stops_the_build_naming_file_and_pattern`; `cobolt-web-bundle::a_planted_secret_in_a_property_stops_the_bundle`; `cobolt-web-bundle::every_pattern_in_the_public_table_is_found_when_planted`; `cobolt-web-bundle::a_known_secret_is_refused_by_file_and_setting_wherever_the_bundle_holds_it`; `cobolt-project-tools::a_connection_string_or_key_on_a_web_form_stops_check_and_build_and_on_the_backend_it_builds`; `cobolt-web-bundle::the_tracked_examples_hold_no_secret_shape_beyond_the_ones_the_operator_knows` |
| TM-23 | B2 | A6 | **E** A weakened setting reaches a release: a development token, plain `ws://` to a remote address, a wildcard origin, a weakened CSP, the secret scan skipped | every relaxation is a setting whose name says `unsafe`, warns in the IDE in six languages and fails a release Build (R11; AC9; `generator::gate_inputs`, `cobolt-server/src/config.rs`) | `cobolt-project-tools::each_unsafe_key_warns_in_development_and_fails_in_release`; `cobolt-project-tools::skip_and_the_backend_toml_follow_the_profile`; `cobolt-server::a_release_start_refuses_every_unsafe_setting_and_a_development_start_only_warns`; `cobolt-server::a_secret_written_in_the_file_stops_a_release_start_and_only_warns_in_development` |
| TM-24 | B2 | A6 | **I** The security contact is missing or stale, so a finder cannot report | `[security]` becomes `/.well-known/security.txt` (RFC 9116) in the bundle and on the backend's HTTPS surface; an incomplete or expired table stops the Build (Q4) | `cobolt-web-bundle::configured_the_file_is_written_with_contact_and_expires`; `cobolt-server::the_backend_serves_the_text_the_build_writes_and_a_404_without_the_table` |
| TM-25 | B3 | A4 | **E** A value reaches the page as markup: a caption, an item, a cell, a tooltip, a message, a link | every value is inserted as text; the player has no API that inserts an HTML string, and the Viewer builds nodes from a filtered tree; link schemes are an allow-list (R5; AC4; `cobolt-web-player/src/dom/`) | `cobolt-web-player::the_players_sources_insert_no_markup_run_no_string_and_leak_no_closure`; `cobolt-web-player::every_node_the_page_gets_is_made_with_the_dom_builder_and_the_viewer_never_takes_markup`; `cobolt-web-player::the_search_itself_finds_what_it_looks_for`; `cobolt-web-bundle::the_link_schemes_reach_the_player_under_the_new_key_and_the_old_one_is_still_read`; `cobolt-web-bundle::a_click_follows_the_target_and_the_modifiers_and_a_new_tab_has_no_opener_unless_rel_says_so` |
| TM-26 | B3 | A2 | **I** A password, a token or a marked field is quoted in an error report, a console line or a diagnostic | a password-mode control is the marked field (O1); the token type has no `Display` and a redacted `Debug`; parameters are named by position; `Host::report` replaces a marked value in any report (R13; AC10; `cobolt-web-player/src/host.rs`, `auth.rs`) | `cobolt-web-player::a_password_boxs_text_is_in_no_error_report_and_no_console_line`; `cobolt-web-player::a_parameter_that_is_refused_is_named_by_its_position_never_by_its_text`; `cobolt-web-player::a_token_and_a_password_in_a_call_reach_neither_a_console_warning_nor_an_error_answer` |
| TM-27 | B3 | A3 | **I** A script on the page reads the token from the page | the token lives in the player's memory and is refreshed once, never written to the DOM or to storage (R6; `cobolt-web-player/src/auth.rs`) | `cobolt-web-bundle::the_control_is_accessible_keeps_no_token_in_the_page_and_refreshes_once` |
| TM-28 | B3 | A4 | **D** A hostile payload or repeated opening exhausts the page's memory; a destroyed form's region or an unmounted application's module is not released | the module's memory never grows, the transport arena is fixed, every exposed type is listed and disposed, and a hundred part lifetimes leave no object and no growth (R15b; WASM-017) | `cobolt-web-player::every_exposed_type_is_listed_and_disposed_017`; `cobolt-web-player::the_shipped_player_has_no_test_hook_017`; `cobolt-web-bundle::a_hundred_part_lifetimes_leave_no_object_and_no_growth_and_hostile_inputs_are_refused` |
| TM-29 | B3 | A5 | **T** The player links a C toolchain, a TLS stack, a second transport or a licence outside the allow-list | the browser build's dependency guard, with the threaded variant and the sealed channel's four crates stated (R9; `cobolt-web-player/tests/dependency_guard.rs`) | `cobolt-web-player::the_browser_build_links_no_c_toolchain_no_tls_stack_and_no_second_transport`; `cobolt-web-player::every_package_of_the_browser_build_is_permissive_and_builds_on_rust_1_92`; `cobolt-web-player::the_browser_build_carries_the_sealed_channels_four_crates` |
| TM-30 | B3 | A5 | **T** The SBOM of a Web bundle does not list what the player is made of | one SBOM writer for the bundle, the backend binary and (WASM-012) the mobile shell; the player's own crates come from its lock closure into `player-build.json` (R10) | owed: WASM-016 — `build_player` writes the player's `components` into `player-build.json` and the bundle's SBOM lists them |
| TM-31 | B4 | A3 | **S** Another machine, or a page with a rebinding DNS name, reaches the development server | the development server listens on loopback only and refuses a foreign `Host` (R12; WASM-008 R8; `cobolt-project-tools/src/web/devserver.rs`) | `cobolt-project-tools::it_listens_on_loopback_only`; `cobolt-project-tools::a_foreign_host_header_is_refused`; `cobolt-project-tools::only_get_and_head_are_answered_and_head_has_no_body` |
| TM-32 | B4 | A6 | **I** A path leaves the bundle: encoded dots, a symlink in a project, a double decoding | one decode, no spelling of a path outside the bundle is served, a symlink that leaves the bundle is not followed | `cobolt-project-tools::no_spelling_of_a_path_outside_the_bundle_is_served`; `cobolt-project-tools::a_symlink_that_leaves_the_bundle_is_not_followed`; `cobolt-project-tools::the_path_parser_decodes_once_and_refuses_what_could_leave` |
| TM-33 | B4 | A3 | **T** A page the developer did not serve posts events to the IDE, or the preview script parses markup it was sent | page events are taken from the server's own page only; the relay script never parses markup (WASM-008) | `cobolt-project-tools::page_events_are_taken_from_our_own_page_only_008`; `cobolt-project-tools::the_preview_script_relays_and_never_parses_markup_008` |
| TM-34 | B4 | A3 | **E** The development backend accepts a connection from any origin, so a page on another port of the same machine drives the developer's backend | the development server hands the backend its own page origin at start; only that origin may connect; the development backend's address rides in the served configuration only (R12; T9) | `cobolt-server::only_the_pages_origin_may_connect_and_not_another_port_of_the_same_host`; `cobolt-project-tools::the_development_backend_rides_in_the_served_config_only_008` |
| TM-35 | B4 | A6 | **E** A project file makes the Build run something: a tool, a script, a handler's SQL in the page | the Web Build is a function of the project's text and the shipped player; it runs no child tool and needs no Rust toolchain; `EXEC SQL` in a frontend handler stops the Build at its line | `cobolt-project-tools::no_tool_child_build`; `cobolt-project-tools::a_web_project_builds_with_no_rust_toolchain_on_the_path`; `cobolt-project-tools::an_exec_sql_handler_refuses_the_build_at_its_line`; `cobolt-web-bundle::a_bundle_is_made_from_the_shipped_player_with_no_rust_toolchain_and_runs_under_node` |
| TM-36 | B5 | A5 | **T** A pinned mobile app cannot reach its backend after a key change, or pins a key that is not one | `[mobile.pinning]` needs two different, well-formed pins and an `expires` that has not passed, warns 30 days ahead, names a pin by position, and is off when absent; Check and Build share the gate (R14, Q5; `cobolt-compiler/src/project_kind.rs`) | `cobolt-compiler::the_pinning_table_needs_two_pins_and_a_date_and_warns_a_month_before_it`; `cobolt-project-tools::the_pinning_table_stops_check_and_build_when_wrong_and_warns_near_its_end` |
| TM-37 | B5 | A3 | **E** A remote origin, or a window that needs less, gets native access through the shell | the capability file gives the main window only what the declared device features need and no `remote` section (R14) | owed: WASM-012 — `mobile_build.rs` writes the capability file and a test asserts main window only, no `remote` section, permissions equal to the union of the declared features |
| TM-38 | B5 | A4 | **S** A pinned connection accepts another key on a real device | the pins are written as Android's `network_security_config.xml` `pin-set` and iOS's `NSPinnedDomains` (the keys T1 recorded) | owed: WASM-012 — `mobile_build.rs` writes both files from the table; a device is asked to refuse a wrong key (not measurable here) |
| TM-39 | B5 | A5 | **T** The mobile shell links a dependency the web build must not, or ships without a bill of materials | the mobile dependency guard and the shell's SBOM, written by the one SBOM writer (R9, R10) | owed: WASM-012 — the mobile dependency guard and the shell's SBOM |
| TM-40 | B5 | A5 | **I** A signing key or a store secret falls back to a plaintext file where the operating system's store is missing | the product stops and says so (R8; AC7) | owed: WASM-012 — `store_required` for the signing half of AC7 |
| TM-41 | B6 | A4 | **D** A flood of unknown key ids makes the backend fetch the provider's keys over and over; the provider is down | keys are fetched once per `kid` and cached; a provider outage does not stop callers whose keys are known (`cobolt-server/src/jwks.rs`) | `cobolt-server::keys_are_fetched_once_and_a_flood_of_unknown_key_ids_cannot_amplify`; `cobolt-server::an_issuer_outage_does_not_stop_callers_whose_keys_are_known`; `cobolt-server::what_is_not_a_token_is_refused_before_anything_is_fetched` |
| TM-42 | B6 | A3 | **S** The hosted flow is completed with a foreign return address, a wrong `state` or a handle used twice | the return is checked against the allowed origins, `state` is bound to the start, the handle is single-use (WASM-009) | `cobolt-server::the_hosted_flow_refuses_a_foreign_return_a_wrong_state_and_an_old_handle` |
| TM-43 | B6 | A6 | **I** A provider's client secret, a cookie key or a TLS password is printed, logged, written to the bundle or written literally in a release `backend.toml` | secrets come from the environment or a file and never print; a literal secret stops a release start and only warns in Development; the identity secrets are refused wherever the bundle holds them (R7; T4) | `cobolt-server::secrets_come_from_the_environment_or_a_file_and_never_print`; `cobolt-server::the_tls_identity_and_the_provider_settings_print_without_their_secrets`; `cobolt-server::a_literal_pkcs12_password_stops_a_release_start_and_only_warns_in_development`; `cobolt-server::without_a_32_byte_cookie_key_the_backend_does_not_start` |
| TM-44 | B6 | A5 | **T** The generic profile's rules differ from a provider's, so a token one provider would refuse is accepted | each provider profile states the claims it needs (Cognito `token_use`, Keycloak realm roles, Entra audience and `scp`/`roles`) and the conformance table agrees with each adapter | `cobolt-server::cognito_wants_token_use_access_and_the_app_client`; `cobolt-server::keycloak_wants_a_bearer_and_reads_the_realm_roles`; `cobolt-server::entra_wants_the_api_audience_and_scp_or_roles`; `cobolt-server::the_table_and_each_adapters_capabilities_agree` |
| TM-45 | B7 | A4 | **E** A browser sends SQL, a PartiQL text or a program's own text to run | a browser cannot send SQL: `EXEC SQL` in a frontend handler stops the Build, a generated endpoint takes values and never text, a binding is judged by the guardian against the `.cidx` | `cobolt-server::a_browser_cannot_make_a_generated_endpoint_run_its_text`; `cobolt-project-tools::every_r46_error_is_one_diagnostic_on_its_line`; `cobolt-project-tools::the_guardian_judges_a_partiql_binding` |
| TM-46 | B7 | A6 | **I** A grid on a web form is bound to a REST service or an agent from the page, or a form carries a connection string | such a binding stops Check and Build naming the form and the grid; a connection string or key on a web form stops them, and the backend's own is not scanned | `cobolt-project-tools::a_web_forms_grid_bound_to_rest_or_agent_stops_check_and_build_naming_form_and_grid`; `cobolt-project-tools::a_frontend_secret_stops_the_build_and_the_backends_own_does_not` |
| TM-47 | B7 | A1 | **E** A remote file's verbs run without a token | the verbs of a remote indexed file need a token and are refused without one | `cobolt-server::an_indexed_file_controls_verbs_reach_the_remote_file_with_a_token_and_are_refused_without_one` |
| TM-48 | B7 | A2 | **I** One caller sees another's state or answers; a session sees another's replayed answers | per-connection state is never shared; no state survives a call; a new session does not see another session's answers | `cobolt-server::concurrent_callers_get_their_own_results_and_no_state_survives_a_call`; `cobolt-server::per_connection_state_is_kept_and_never_shared`; `cobolt-server::ac6_a_new_session_does_not_see_another_sessions_answers` |
| TM-49 | B7 | A4 | **T** An upload or a download name leaves its folder, has the wrong type or checksum, or a download address outlives its use; a drop zone or a Viewer slot takes more than it should | names are checked (`..`, type, checksum, slot), a download address is short-lived, a slot takes what its control takes and a Viewer's documents up to ten MB (`cobolt-server/src/files.rs`) | `cobolt-server::ac9_a_name_with_dot_dot_a_wrong_type_a_wrong_checksum_and_an_unknown_slot_are_refused`; `cobolt-server::ac9_a_program_offers_a_file_and_the_download_address_is_short_lived_and_the_download_resumes`; `cobolt-server::a_file_drop_zones_slot_takes_what_the_zone_takes_and_nothing_else`; `cobolt-server::a_viewers_slot_takes_the_documents_it_renders_up_to_ten_mb` |

## 5. STRIDE: every boundary walked

Each cell names the rows that answer that letter at that boundary, or says why none applies. A boundary is **walked**
when no cell is empty; a dash with a reason is an answer.

| boundary | S | T | R | I | D | E |
|---|---|---|---|---|---|---|
| B1 browser ⇄ backend | TM-01, 02, 03, 09 | TM-05, 08, 17 | TM-13 | TM-07, 10, 11, 12, 15, 16 | TM-06 | TM-04, 14 |
| B2 host page ⇄ module | — the page is served by the developer's own origin; TM-20 detects a changed file | TM-19, 20 | — the page keeps no record; the backend's log (TM-13) is the trail | TM-21, 22, 24 | — a page can only exhaust itself (TM-28) | TM-18, 23 |
| B3 module ⇄ player | — the module is the developer's own program inside the player's sandbox | TM-29, 30 | — nothing here is attributable to a person | TM-26, 27 | TM-28 | TM-25 |
| B4 IDE ⇄ preview web view | TM-31 | TM-33, 35 | — a developer's own machine; no audit trail is claimed | TM-32 | — the preview serves one local developer | TM-34 |
| B5 mobile shell ⇄ frontend | TM-38 | TM-36, 39 | — no audit trail is claimed on the device | TM-40 | — a device can only exhaust itself | TM-37 |
| B6 backend ⇄ identity provider | TM-42 | TM-44 | — the provider keeps its own record | TM-43 | TM-41 | — the backend asks the provider for nothing it does not need |
| B7 backend ⇄ data | — the caller's identity is B1's | TM-49 | — the call record (TM-13) names the caller | TM-46, 48 | — limits are B1's (TM-06) | TM-45, 47 |

## 6. Personal data the product handles (R13)

The plan's table (§3.3), kept current here. **`subject` is personal data**: it stays in the backend log for audit
(TM-13) and is kept as long as the deployment keeps its logs.

| Data | Where it comes from | Where it goes | Kept |
|---|---|---|---|
| User name or e-mail, password | the Authentication control | backend → identity provider (WASM-009); the password never reaches the program module or a log (TM-11, TM-26) | not by the product |
| Token subject (`sub`) | the identity provider | the backend log's `subject` field (TM-13) | as long as the deployment keeps logs |
| Claims | the token | `GetClaim` answers (ABI §12.1); never logged | in the page's memory only |
| Source IP address | the connection | the attempt limiter (`attempts_per_minute`), in memory | the limiter's window |
| Refresh token | the identity provider | the `__Host-pr_refresh` cookie (web), the OS secure store (mobile) | the cookie's lifetime |
| reCAPTCHA token, browser signals | Google, when configured | backend → Google verification | Google's terms |
| Map-tile requests (IP, viewed area) | the browser | the tile server named in the CSP | the tile server's policy |
| Uploaded files | the user | the backend's `files` directory (`cobolt-server/src/files.rs`) | the developer's retention |
| Application data | the developer's program | the developer's backend and storage | the developer's responsibility |

## 7. Secret-scan patterns (`secrets::PATTERNS`)

The product's scan reports exactly these, and the test fails when this list and the code's differ in name, order or
severity. Findings print the file, the line and the pattern, never the value.

| pattern | severity |
|---|---|
| private key block | error |
| cloud access key id | error |
| JSON Web Token | error |
| connection string with a password | error |
| connection string with a Key=value secret | error |
| Google API key | error |
| GitHub token | error |
| Slack token | error |
| Stripe live key | error |
| Anthropic API key | error |
| Hugging Face token | error |
| API key | error |
| a property named like a secret holds a value | error |
| a secret access key | error |
| high-entropy string | warning |

## 8. Accepted residual risks

| id | risk | why it is accepted |
|---|---|---|
| R-1 | The channel's key agreement is unauthenticated: a party that rewrites the two `hello`s can stand in the middle (TM-09) | the backend's key is not known in advance; TLS's server authentication stops it, which is why the channel is on top of TLS and a plain `ws://` start is loopback-only |
| R-2 | The TLS cipher floor is TLS 1.2 plus the operating system's defaults (TM-10; plan O5) | a stricter list means terminating TLS in front of the backend; the test reports which side refused |
| R-3 | Traffic analysis: the channel hides content, not message sizes, timing or counts | not a goal of the channel (plan §1.11) |
| R-4 | An attacker inside the browser (the page's own script holds the channel's key) or a compromised backend (it holds the other key) | outside what a channel between the two can protect |
| R-5 | "Marked field" is a password-mode control until a program has a way to mark others (plan O1; WASM-014's `sensitive` contract field) | the working assumption of T18, stated in its tests |

## 9. What is not measured here

Stated so that no row is read as more than it is (T24 carries the commands):

- **Firefox and Safari** enforcement of the Content-Security-Policy and Trusted Types (Q2, best effort): the browser rows
  (TM-18 to TM-21) run in Chrome.
- **A real phone** refusing a connection with a key that is not pinned (TM-38), and the capability file on a device
  (TM-37): WASM-012's.
- **The fuzz targets** run over the loopback `ws://` with the real sealed channel, not over TLS (TM-06's TLS side is
  `tls_floor.rs`); only the generic token profile is fuzzed (TM-44 pins the provider profiles one by one).
- **Dependencies' advisories** are held against a recorded baseline (`deny.toml`); retiring each baseline entry waits for
  a fixed release upstream (quick-xml, lopdf, calamine) or the operator's decision (wasmtime).

## 10. The release review (R16)

When the series reaches a release candidate, a review by someone other than the author — and, if the operator decides (Q1),
an external penetration test — is recorded **before the line is released**. The rule is machine-checked:

- **The marker.** `specs/WASM-PROGRESS.md` marks the line as a release candidate with a line that begins
  `RELEASE CANDIDATE:` followed by `reached`. Until that line exists the test prints "not a release candidate".
- **The record.** From then on `specs/WASM-013-security-architecture/release-review.md` must exist and begin with
  `key = value` lines: `kind` (`review` or `penetration-test`), `reviewer`, `author`, `scope`, `date`, and the file lists
  the findings and what was done with each. The reviewer must be a different person from the author.

The release-candidate step of WASM-028 writes that line only after this file exists.

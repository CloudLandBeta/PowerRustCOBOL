<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — AWS controls through MCP

- **Status:** draft → awaiting operator review
- **Folder:** specs/078-aws-mcp-controls/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-30

> Operator request, 2026-09-30: *"spec the creation of AWS support via MCP
> (call Lambda functions passing parameters, DynamoDB, S3, S3 Tables, S3 Vector
> Tables, AWS Glue, Amazon Bedrock AgentCore/Memory/KB, Rekognition, Polly,
> Comprehend, Textract, EC2 Start/Stop, Cognito)"* — *"all AWS controls are
> non-visual controls"* — *"they live in the toolbox under AWS category"*.

## 1. Overview

A PowerRustCOBOL application can call a REST API, a database, a model and its
own indexed files. It cannot reach AWS. This feature adds a family of
**non-visual controls in a new `AWS` toolbox category**: one per service the
operator named. A COBOL developer drops one on a form, picks a connection,
and calls AWS from an event handler the way they already call a `RestClient`:
a method, result properties, and `onComplete` / `onError` events.

**The controls reach AWS through the Model Context Protocol (MCP), as an MCP
client of AWS's own MCP servers.** That choice is the whole design, and it is
made for three reasons this repository already holds:

1. **No TLS stack, no AWS SDK in the binary.** The workspace keeps rustls,
   aws-lc-rs and ring out of every build because they need a C toolchain and
   cmake. `cobolt-runtime` pins `native-tls`, and spec 063 R18 records that
   *"the AWS SDK defaults to aws-lc-rs"*. The AWS MCP servers are separate
   processes that do their own TLS and SigV4 signing. The application talks to
   them over **stdio**, a byte stream, so nothing in the application links a
   cryptographic library for AWS.
2. **No AWS credentials in the application.** The server process reads the
   standard AWS credential chain (a named profile, SSO, environment, instance
   role). The application passes a profile *name* and a region, never a key.
   A `.cfrm` already can never hold a credential (`xml.rs`, `CREDENTIAL_PROPS`);
   this feature introduces no credential for it to hold.
3. **The protocol is already half here.** `cobolt-mcp` (spec 065) implements
   the server side of MCP over a host-supplied byte stream, with no dependencies
   beyond serde. The client side is the missing half, and the same crate can
   carry it.

### What exists on the AWS side today (verified 2026-09-30)

AWS publishes MCP servers in two forms, and neither covers every service:

| Service | Official route | Notes |
|---|---|---|
| Lambda | `awslabs.lambda-tool-mcp-server` (stdio) | Each selected function becomes a tool, selected by name prefix, list or tag. |
| S3 Tables | `awslabs.s3-tables-mcp-server` (stdio) | Read-only unless started with `--allow-write`. |
| Glue | `awslabs.aws-dataprocessing-mcp-server` (stdio) | `manage_aws_glue_*` tools; `--allow-write` off by default. |
| Bedrock AgentCore (Runtime, Memory) | `awslabs.amazon-bedrock-agentcore-mcp-server` (stdio) | `invoke_agent_runtime`, `memory_create_event`, `memory_retrieve_records`, … |
| Bedrock Knowledge Bases | `awslabs.bedrock-kb-retrieval-mcp-server` (stdio) | `ListKnowledgeBases`, `QueryKnowledgeBases`. |
| DynamoDB items, S3 objects, S3 Vectors, Rekognition, Polly, Comprehend, Textract, EC2, Cognito | **Hosted AWS MCP Server** (GA 2026-05-06), reached over stdio through AWS's `mcp-proxy-for-aws` (SigV4) | The awslabs DynamoDB server does data *modelling*, not item access; the Rekognition server was withdrawn in favour of the API server, which is itself superseded by the hosted one. |

The landscape moved twice in a year, so this spec **does not wire any service
to a server in code**. It defines a *route table* (R14–R17) as data, so moving a
service to a new server is a data change and a test run, not a code change.

All AWS servers need **`uv` / `uvx` and Python ≥ 3.10** on the machine that runs
them. That is a real prerequisite for the developer and for the end user of a
built application, and the spec treats it as one (R22–R24).

## 2. Goals / Non-goals

**Goals**

- Fifteen non-visual controls in an `AWS` toolbox category, one per service
  named by the operator, plus one generic control that calls any MCP tool.
- An MCP **client** in `cobolt-mcp`: stdio transport, the current protocol
  revision and the older handshake-based ones, tool discovery and tool calls.
- **Project AWS connections**: named profile + region + route, set once in the
  project and shared by every AWS control, with nothing secret stored.
- COBOL-first surface: methods with COBOL arguments, results as properties,
  asynchronous by default with the same events the `RestClient` and
  `WebSearch` controls raise.
- **Read-only unless the developer opts in**, per control.
- The same behaviour in the IDE's Run Form, embedded child forms and the
  compiled binary.
- Offline, deterministic tests: a fake MCP server, no AWS account needed.

**Non-goals**

- Linking the AWS SDK for Rust, or any HTTP/TLS client, for AWS.
- Streamable-HTTP transport or OAuth sign-in to the hosted AWS MCP Server
  directly. The SigV4 proxy over stdio reaches the same server without either
  (§7 Q3 keeps the door open).
- Storing, entering or displaying AWS access keys anywhere in the IDE or in a
  built application. The AWS CLI / SSO login is the only credential path.
- Administering AWS resources (creating tables, buckets, functions, user pools,
  IAM). The controls *use* resources that exist.
- Exposing the application's own MCP server to AWS (that is spec 065's server
  direction).
- A visual AWS designer, an S3 file browser or a cost dashboard.
- Bundling Python or `uv` inside a built application (§7 Q2).

## 3. User stories

- As a **COBOL developer**, I want to call a Lambda function with a JSON
  payload built from my working-storage and read its reply, so that I can reuse
  business logic my company already runs on AWS.
- As a **developer**, I want to read and write DynamoDB items and S3 objects
  from an event handler, so that my form works on cloud data without a
  hand-written REST gateway.
- As a **developer**, I want to extract text from a scanned form (Textract),
  detect labels in an image (Rekognition), speak a message (Polly) and read the
  sentiment of a comment (Comprehend), so that my application gets these
  capabilities with one control each.
- As a **developer**, I want to ask a Bedrock Knowledge Base or an AgentCore
  agent a question, and keep conversation memory in AgentCore Memory, so that my
  chatbot can use company knowledge hosted on AWS.
- As a **developer**, I want to start and stop an EC2 instance and sign a user
  in against a Cognito user pool, so that operations and authentication stay in
  my COBOL application.
- As a **developer**, I want to name my AWS profile and region once per project,
  and to know that no key ever lands in my form files, so that sharing a project
  never leaks an account.
- As an **end user of a built application**, I want a clear message when the
  machine lacks what AWS access needs, so that I know what to install rather
  than seeing a silent failure.

## 4. Requirements (EARS)

### 4.1 The MCP client (`cobolt-mcp`)

- **R1 (ubiquitous):** `cobolt-mcp` shall implement the MCP **client** role over
  JSON-RPC 2.0: tool discovery (`tools/list`, with pagination) and tool
  invocation (`tools/call`), including structured and text results and
  tool-level errors (`isError`).
- **R2 (constraint):** The client shall keep spec 065's crate rules: no
  dependency on COBOL, egui, the filesystem, a TLS implementation, an HTTP
  stack, or any crate requiring a C toolchain. Transport is a byte stream the
  host supplies.
- **R3 (ubiquitous):** The client shall speak the **current** protocol revision
  (2026-07-28, stateless) and fall back to the **handshake-based** revisions
  (`initialize` / `initialized`, 2025-06-18 and 2025-11-25) when a server
  answers with one of them, so it works with every AWS server whatever revision
  that server ships.
- **R4 (event):** When a server sends a notification or a request the client
  does not handle (progress, logging, sampling, elicitation), the client shall
  answer or ignore it as the protocol requires, and never stall a pending call.
- **R5 (ubiquitous):** Every call shall carry a timeout; a server that does not
  answer in time yields a timeout outcome, not a hang.
- **R6 (constraint):** One module shall hold the client's wire types together
  with the server's, so the two directions cannot drift apart. A test shall
  round-trip every message the client sends through the server's parser.

### 4.2 The stdio transport and the server process

- **R7 (ubiquitous):** The runtime shall start an MCP server as a **child
  process** and talk to it over its stdin/stdout, with newline-delimited
  JSON-RPC framing.
- **R8 (ubiquitous):** One server process shall serve every control that uses
  the same connection and route in one running application: started on first
  use, shared, and stopped when the application ends. A crashed process shall be
  restarted on the next call, and the failed call reported as an error.
- **R9 (constraint):** The runtime shall never leave a server process running
  after the application exits, on any host (Run Form, child form, compiled
  binary), including a crash of the application.
- **R10 (ubiquitous):** A server's stderr shall be captured to the application's
  diagnostic log, never mixed with the protocol stream and never shown raw to an
  end user.
- **R11 (constraint):** The runtime shall pass AWS settings to a server only as
  a profile *name*, a region, route flags and the environment variables the
  route declares. It shall never pass, read, store or log an access key, a
  secret key or a session token.

### 4.3 Project AWS connections

- **R12 (ubiquitous):** A project shall hold zero or more **AWS connections**,
  each with a name, an AWS profile name, a region and, optionally, a route
  override. They are stored in `cobolt.toml` with the other integrations and
  contain nothing secret.
- **R13 (ubiquitous):** Every AWS control shall have a `Connection` property
  naming one of the project's AWS connections. A project with exactly one AWS
  connection shall assign it to a newly dropped AWS control.

### 4.4 The route table

- **R14 (ubiquitous):** Which MCP server serves each control operation shall be
  defined in **one route table**, as data: for each operation, the server launch
  (package, arguments, environment), the tool name, and how the COBOL arguments
  map to the tool's input and the tool's result maps back to properties.
- **R15 (constraint):** No control operation shall name a server, a tool or a
  launch command in code outside the route table.
- **R16 (ubiquitous):** The route table shall ship with the product and be
  versioned with it. A connection's route override may point a service at
  another server for one project.
- **R17 (ubiquitous):** A test shall check the route table against **recorded
  `tools/list` answers** of each routed server: every tool the table names
  exists, and every required input it fills is in that tool's schema. A route
  that no longer matches a recorded server fails the build, not the end user.

### 4.5 The controls

- **R18 (ubiquitous):** The following controls shall exist, each **non-visual**
  (`is_non_visual()` true) and listed in the toolbox under a new **`AWS`**
  category:

  | Control | Operations (COBOL methods) |
  |---|---|
  | `AwsLambda` | Invoke a function by name with a JSON payload; read its reply and whether the function itself reported an error |
  | `AwsDynamoDB` | Get, put, update and delete an item by key; query by key condition; scan with a limit |
  | `AwsS3` | Get an object to a file or to a data item; put a file or a data item; list a prefix; delete an object |
  | `AwsS3Tables` | List tables in a namespace; run a read query and return the rows; append rows |
  | `AwsS3Vectors` | Put vectors with metadata into an index; query the nearest vectors to a vector |
  | `AwsGlue` | Start a job run and read its state; start a crawler; read a table's schema from the Data Catalog |
  | `AwsAgentCore` | Invoke an agent runtime with a prompt and a session id; read the reply |
  | `AwsAgentMemory` | Record a conversation event; retrieve memory records relevant to a query |
  | `AwsKnowledgeBase` | Query a Bedrock Knowledge Base; return the passages and their sources |
  | `AwsRekognition` | Detect labels, text or faces in an image file or S3 object |
  | `AwsPolly` | Synthesise speech from text to an audio file, with a voice and format |
  | `AwsComprehend` | Detect sentiment, entities, key phrases or dominant language of a text |
  | `AwsTextract` | Extract text, forms (key/value pairs) and tables from a document file or S3 object |
  | `AwsEC2` | Start, stop and describe instances by id |
  | `AwsCognito` | Sign up, confirm, sign in and sign out a user in a user pool; read the signed-in user's attributes |
  | `AwsMcp` | Call any tool of the connection's server by name with a JSON argument (the escape hatch) |

- **R19 (ubiquitous):** Every AWS control shall be asynchronous by default, with
  `Mode = Async | Sync`, `Busy`, `TimeoutMs`, `LastError` and `Verbose`, and
  shall raise the events the asynchronous non-visual controls already raise:
  its own completion event, then `onComplete`; `onError`; `onTimeout`;
  `onCancelled`. One operation per control is in flight at a time, and a stale
  result is discarded, as for `RestClient` and `WebSearch`.
- **R20 (ubiquitous):** Results shall be readable from COBOL as properties:
  scalar results as text, and structured results (items, rows, labels,
  passages, vectors) as a row set the program walks by index, plus the raw JSON
  for developers who want it.
- **R21 (event):** When a JSON argument a method receives is not valid JSON,
  the control shall report `onError` with a message naming the argument, and
  send nothing.

### 4.6 Prerequisites and diagnostics

- **R22 (event):** When an AWS control is first used and `uvx` (or the command a
  route names) cannot be found, the control shall raise `onError` with a
  `LastError` that names the missing program and the fix, in plain words.
- **R23 (event):** When the AWS profile is missing, expired or not signed in, the
  control shall raise `onError` with a `LastError` that says so and names the
  profile, rather than the server's raw output.
- **R24 (ubiquitous):** The IDE shall offer a **Test connection** action on an
  AWS connection that starts its server, lists its tools and reports, in the
  IDE language, whether the program, the profile and the route are all good.

### 4.7 Safety

- **R25 (ubiquitous):** Every AWS control shall have `AllowWrite`, **off by
  default**. While it is off, an operation that changes AWS state (put, update,
  delete, append, start, stop, sign-up, invoke a Lambda, run a job) shall be
  refused with `onError` before anything is sent, and servers that have a
  read-only mode shall be started in it.
- **R26 (ubiquitous):** `AwsMcp` shall refuse every tool the route table does not
  mark read-only unless `AllowWrite` is on.
- **R27 (constraint):** `Verbose` output shall mask anything that looks like a
  credential, session token or pre-signed URL signature, as `WebSearch` masks
  its key.

### 4.8 One behaviour on every host

- **R28 (ubiquitous):** Run Form (`rcrun run-form`), embedded child forms and the
  compiled binary shall run AWS controls identically: same seeding, same client,
  same route table (the `interpreter-binary-parity` rule).
- **R29 (constraint):** A built application that contains no AWS control shall
  link none of this feature's runtime code; the build shall detect AWS controls
  from the forms, as it already does for the `http`, `maps` and `kb` features.

### 4.9 IDE, documentation and knowledge

- **R30 (ubiquitous):** Each AWS control shall have an original tray icon (not an
  AWS logo), property rows in the inspector, property help in all six languages,
  IntelliSense for its methods, and a Grace synonym so it can be asked for in
  plain words.
- **R31 (ubiquitous):** The System KB tables (`property_reference`,
  `event_reference`, `control_purpose`, `control_method_docs`) shall document
  every AWS control, and `assets/knowledge/chunked.data` shall be regenerated in
  the same change.
- **R32 (ubiquitous):** The Developer's Guide shall gain an AWS chapter written
  for a PowerCOBOL / isCOBOL developer, in COBOL only: prerequisites, AWS
  connections and profiles, one worked example per control, the read-only rule,
  and what an end user must install.

## 5. Acceptance criteria

- [ ] **AC1 (R1, R3):** Against the fake MCP server, the client lists tools
  (across two pages) and calls a tool, once with the 2026-07-28 revision and
  once with the handshake-based 2025-06-18 revision; results agree.
- [ ] **AC2 (R2):** `cargo tree -p cobolt-mcp` shows no TLS, HTTP or C-toolchain
  crate; `cargo tree -p cobolt-runtime --features <aws feature>` shows no
  rustls, aws-lc-rs or ring.
- [ ] **AC3 (R4, R5):** A fake server that sends a progress notification and a
  logging message before answering is served; a fake server that never answers
  yields `onTimeout` within `TimeoutMs` + 1 s.
- [ ] **AC4 (R6):** Every client request type round-trips through the server
  parser in a unit test.
- [ ] **AC5 (R7, R8):** Two AWS controls on one connection start **one** server
  process (counted); killing it makes the next call report `onError` and the
  call after that succeed on a fresh process.
- [ ] **AC6 (R9):** After a Run Form session, a child form session and a compiled
  binary each exit (normally and by kill), no server process they started is
  alive.
- [ ] **AC7 (R10, R11, R27):** A test scans the arguments, environment and
  captured logs of a started server: no value matching an AWS key id, secret or
  session-token pattern appears; `Verbose` output masks a planted fake token.
- [ ] **AC8 (R12, R13):** An AWS connection round-trips through `cobolt.toml`;
  a dropped AWS control in a one-connection project gets it; the saved `.cfrm`
  and `cobolt.toml` contain no credential field.
- [ ] **AC9 (R14–R17):** The route-table test passes against the recorded
  `tools/list` fixtures, and fails when a fixture renames a tool the table uses
  (demonstrated with a mutated fixture).
- [ ] **AC10 (R18):** The toolbox shows an `AWS` category holding the sixteen
  controls, in every IDE language; each is non-visual (painted as a tray card in
  the designer, nothing at run time).
- [ ] **AC11 (R18–R20):** For each control, one COBOL test program drives each
  listed operation against the fake server and checks the result properties and
  the event order (completion event, then `onComplete`), with a quantified
  summary block (GOLDEN RULE #7): operations exercised, calls made, time per
  call.
- [ ] **AC12 (R21):** An invalid JSON payload raises `onError` naming the
  argument, and the fake server records no call.
- [ ] **AC13 (R22, R23):** With `uvx` removed from `PATH`, and with a profile the
  fake credential check rejects, `LastError` carries the plain-language message
  of each case.
- [ ] **AC14 (R24):** Test connection reports each of: program missing, profile
  not signed in, route mismatch, all good.
- [ ] **AC15 (R25, R26):** With `AllowWrite` off, every mutating operation of
  every control is refused before sending (the fake server records zero calls);
  with it on, the call is sent. Read-only servers are started with their
  read-only flag.
- [ ] **AC16 (R28):** The AC11 programs pass under `rcrun run-form`, as an
  embedded child form and as a compiled binary.
- [ ] **AC17 (R29):** A project without AWS controls builds a binary whose
  dependency tree contains none of the AWS feature's code.
- [ ] **AC18 (R30, R31):** `prop_help_tests`, `every_control_property_is_documented`
  and `prebuilt_chunked_kb_matches_the_published_documentation` are green with
  the new controls; IntelliSense lists each control's methods.
- [ ] **AC19 (R32):** The Guide chapter exists, its COBOL examples compile with
  `rcrun check`, and the Guide's translations are deleted (GOLDEN RULE #8).
- [ ] **AC20 (live, opt-in):** With an operator-provided AWS profile and
  `COBOLT_AWS_LIVE=1`, a smoke test calls one read-only operation per service
  and reports what answered; skipped otherwise.

## 6. Constraints & steering check

- **Classification:** a **feature** (new controls and a new integration beyond
  the IDE's existing scope) → `features` branch, `z` bump per commit, f=96 if
  ever announced.
- **i18n (×6):** the `AWS` category name, connection dialog, Test-connection
  messages and every property help text are `Tr` / `PropHelp` entries in all six
  languages. COBOL identifiers, method names and property names stay English.
- **Generated code:** codegen emits working-storage for AWS controls the way it
  does for `RestClient` / `WebSearch`; the generated `.cbl` stays a build
  artifact, written with the inline `COBOL::"…"` form where a built-in is used.
- **Docs:** Developer's Guide AWS chapter (English, translations deleted);
  `cobol-support-matrix-en.md` row; `DEPENDENCIES-en.md` only if a crate is
  added (none is expected); System KB tables + `chunked.data`.
- **No C toolchain:** no rustls, aws-lc-rs, ring or AWS SDK anywhere (spec 063
  R18, the `native-tls` pin).
- **Secrets:** nothing is added to `CREDENTIAL_PROPS` because no credential
  property exists; AC7 proves none travels.
- **Interpreter-binary parity:** R28 / AC16.
- **Windows:** the golden rule that a window never resizes itself applies to the
  connection dialog and the Test-connection report.
- **Trademarks:** "AWS" and service names are used descriptively; no AWS logo or
  artwork is reproduced (R30).

## 7. Open questions

- **Q1 — The generic tool on the hosted AWS MCP Server.** AWS's current tool
  page lists `aws___run_script` (sandboxed Python with AWS API access) and
  `aws___get_presigned_url`, but a secondary source names `aws___call_aws`.
  Which one carries a single API call decides how ten of the sixteen controls
  are routed. *Suggested:* record a live `tools/list` of the hosted server
  during `/plan` and route through whichever tool it lists; the route table
  (R14) keeps the choice out of code either way.
- **Q2 — What an end user must install.** The servers need `uv` and Python ≥
  3.10. *Suggested:* the installer does not bundle them; a built application
  checks on first use (R22) and the Guide documents the one-line install. Bundle
  later only if operators ask.
- **Q3 — Streamable HTTP later?** Direct HTTP to the hosted server would drop
  the Python prerequisite for those services but needs TLS and OAuth or SigV4
  in the application. *Suggested:* not in this spec; revisit only with a
  `native-tls`-only design.
- **Q4 — Lambda routing.** The Lambda server exposes each selected function as
  its own tool (by prefix, list or tag). `AwsLambda.Invoke("name", payload)`
  maps to that tool; functions outside the selection are unreachable.
  *Suggested:* the connection's route override carries `FUNCTION_PREFIX` /
  `FUNCTION_LIST`, and Test connection lists the functions it found.
- **Q5 — Scope of the first delivery.** Sixteen controls is large. *Suggested
  phasing:* (1) client + transport + connections + `AwsMcp` + `AwsLambda`;
  (2) the dedicated-server controls (Knowledge Base, AgentCore, Agent Memory,
  S3 Tables, Glue); (3) the hosted-server controls (DynamoDB, S3, S3 Vectors,
  Rekognition, Polly, Comprehend, Textract, EC2, Cognito). Each phase ships on
  its own.
- **Q6 — Cognito tokens.** Signing a user in yields tokens. *Suggested:* the
  control keeps them in memory for the session only, exposes the signed-in
  state and attributes to COBOL, and never writes a token to a property COBOL
  can read or to disk.

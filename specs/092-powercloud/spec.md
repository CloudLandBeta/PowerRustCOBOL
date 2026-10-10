<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Spec — PowerCloud: an AWS example, and the platform it needs

- **Status:** approved by the operator's instruction of 2026-10-09 ("implement PowerCloud as defined"); decisions D1–D9
  below were taken by the implementing agent while the operator was away, on the operator's standing rule
  *"do what is right, not what is easier or less risky"*.
- **Folder:** specs/092-powercloud/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-09
- **Classification:** a **feature** → branch `features-powercloud`, `z` bump per commit (the minor is already 1.91.0,
  raised by the operator for the documentation regeneration), merged into `main` only when the operator asks.

## 1. Overview

Help ▸ Examples gains a fifth example, **PowerCloud**: a Spatial-theme desktop application that uses **one AWS control
of each kind** the product ships for something useful — DynamoDB, S3, Lambda, Polly, EC2, Rekognition — and a Settings
tab that takes the credentials, deploys the CloudFormation stack the demo needs and offers the template for download.

Building it showed that the platform is short of what the definition asks for. This spec therefore covers **the
capabilities PowerCloud needs** (Part A–C) and **the example itself** (Part D), because the example is only honest if it
is built on real, documented, tested features of the product and not on special cases.

### The operator's definition (verbatim intent)

> A responsive window with a panel titled *PowerCloud* and an **x** to close it (no window title). A tab control with a
> tab for each example. The application asks for the credentials to access the cloud. Use the colours of the image
> (the *AWS Services Wheel*), Spatial theme.
>
> - **Settings (first tab):** the image on the left, the cloud access settings beside it. Ask the user to run the
>   CloudFormation in the cloud using the credentials provided; also offer a link to download the CloudFormation
>   script so the user can run it by themselves.
> - **DynamoDB:** create a table; CRUD of pets; a datagrid of the records with filters to narrow the list. Data entry
>   in a popup. Columns: ID, Photo (dropzone in the popup; 40×40 preview in the grid), Name, Race, Age, then **View,
>   Edit, Delete** icon columns. A photo is formatted to **400×400**, stored in an **S3 bucket** and read back through a
>   **temporary URL**; deleting a record deletes its file from the bucket.
> - **S3:** dropdown to choose the bucket; datagrid of its files (no edit/delete).
> - **Lambda:** generates the temporary URL used by the DynamoDB tab; the tab only explains how it was implemented.
> - **Polly:** language dropdown, multiline text editor, send button, an audio **player** for the returned audio.
> - **EC2:** region dropdown; datagrid of servers (Instance ID, Name, Class, vCPU, Memory, State, a **switch** to turn
>   on/off — a **modal window shows the processing** and closes when the state updates).
> - **Rekognition:** dropzone (only the formats Rekognition accepts), photo preview, send button; detects
>   **celebrities**; the **rounded rectangles returned are drawn over the preview**, the details of the celebrity to the
>   right of it.
> - Generate a CloudFormation script for everything the cloud side needs.

### What already exists (verified 2026-10-09 against the code — the investigation that shaped this spec)

| Need | State today |
|---|---|
| Frameless window, rounded corners, transparency | `Form.title_visible=false`, `corner_radius`, `transparency`; PowerSpatial builds its own close box from a Panel + Button (`INVOKE me::Close()`) |
| TabControl with pages, `onTabChanged`, `SelectedTab` | exists |
| DataGrid **Image** cells (file path, fit), **Button** cells with `icon:pencil`/`icon:trash`/`icon:eye`, `ClickedRow`/`ClickedColumn`, `onCellClick`, `ColumnFilters` | **exist** (PowerChat `files-form.cfrm` uses icon columns). Gaps: a failed image load is cached forever; the Image kind is undocumented and untested |
| FileDropZone (`AllowedExtensions`, `DroppedFiles`, `onFilesDropped`) | exists |
| Modal forms (`OpenFormSync`, `super::SetProperty`), `OpenFormAsync` + handle `Close()`, Timer | exist |
| AWS controls: AwsDynamoDB (Scan/Put/Update/Delete), AwsS3 (List/Get/Put/Delete), AwsLambda, AwsPolly (writes a file), AwsEC2 (Describe/Start/Stop), AwsRekognition (labels/text/faces), AwsMcp | exist (spec 078); routes are data in `crates/cobolt-runtime/src/aws/routes.toml` |
| **Rekognition celebrities** | **absent**; faces rows do not expose the BoundingBox |
| **S3 list buckets** | **absent** (`AwsS3` lists one bucket's objects) |
| **A way to call any AWS API** (CloudFormation `create_stack`, DynamoDB `create_table`) | `AwsMcp.Call` reaches a server tool but COBOL-built script text is refused by design; no literal-only generic operation exists |
| **Credentials entered at run time** | **forbidden** today (spec 078 R11, AC7, a stated non-goal): controls take a profile *name* |
| **Image resize / crop / dimensions** from COBOL | **absent** (the `image` crate is linked through `cobolt-forms` but the runtime has no image built-in) |
| **Binary HTTP download to a file** | **absent** (`HTTP-GET` is text-only and capped) |
| **Audio playback** | **absent** (no audio crate in the workspace) |

## 2. Decisions (D1–D9) — taken in the operator's absence

- **D1 — Credentials: typed in the application, held in memory only.** The operator's definition says the application
  *asks for the credentials*, which amends spec 078's "no key in the application". The amendment (**A6**, §4.1) is as
  narrow and as safe as it can be made: a new built-in hands a session's key id, secret and optional token to the
  runtime; they live in memory, are never written to a `.cfrm`, `cobolt.toml`, log or `Verbose` output, are masked
  everywhere, reach the server child process **only through its environment**, and die with the process. A
  connection's **profile** keeps working and stays the recommended route for a developer's own machine.
  *Rejected:* profile-only (it would not do what the definition says); storing keys in the OS key store (the product's
  default key store writes to disk, and "remember my cloud keys" is a decision for the operator).
- **D2 — The table is created from the DynamoDB tab, and by CloudFormation if the user prefers.** The tab's
  *Create table* button creates the pets table through the new generic call (`AwsMcp.AwsCall`), idempotently
  (it describes first). The template carries the same table behind a parameter `CreateTable` (default `false`), so a user who
  deploys by hand can choose either route without the two colliding.
- **D3 — Audio is built-ins, and the player is composed from controls.** `AUDIO-PLAY` / `AUDIO-PAUSE` /
  `AUDIO-RESUME` / `AUDIO-STOP` / `AUDIO-STATE` give any COBOL program, console or form, sound; the example's player is a
  Play/Pause/Stop row, a progress bar and a time label driven by a Timer. A visual `AudioPlayer` control can be built on
  the same engine later; it is not needed to meet the definition, and a new control type touches thirty files.
- **D4 — Photos are fitted by a built-in.** `IMAGE-FIT` crops-to-fill and resizes (400×400 for storage, 40×40 for the
  grid) and `IMAGE-INFO` reports the dimensions (needed to place the Rekognition boxes). Both sit behind a new
  `images` runtime feature, so a program that does not use them links nothing new.
- **D5 — The temporary URL comes from Lambda, the bytes come from `HTTP-DOWNLOAD`.** The Lambda function
  `powercloud-presign` returns a pre-signed URL for an object key; `HTTP-DOWNLOAD` saves it to a local cache file that the
  grid and the preview show. (S3's own `GetObject(toFile)` would work, but the definition names the temporary URL, and it
  is what makes the Lambda tab real.)
- **D6 — EC2 regions are named connections.** A control's region is a property of its *connection*, and COBOL can switch a
  control's `Connection`; the project therefore declares one connection per offered region and the region dropdown
  switches among them. The session credentials are applied to all of them.
- **D7 — The wheel image ships as an operator-supplied asset** (`Assets/images/aws-services-wheel.webp`, 800×900). It
  carries AWS logos, which spec 078 R30 keeps out of the product's *control icons*; this is the operator's own supplied
  artwork for one example, and the README says so. It is the operator's to keep or remove.
- **D8 — Nothing is merged into `main` until the operator asks.** The work is committed and pushed on its feature branch
  and its sub-branches.
- **D9 — Translations last.** A change to the English Guide deletes its translations (GOLDEN RULE #8); PowerCloud edits
  the Guide, the support matrix and DEPENDENCIES, so those three are translated after the last English edit.

## 3. Goals / Non-goals

**Goals**

- PowerCloud runs end to end under `rcrun run-form`, as embedded child forms and as a compiled binary, against the
  fake MCP server in tests, and against real AWS when the operator supplies an account (AC-live, opt-in).
- Every capability added is general, documented in the Guide, in the System KB and in IntelliSense, and tested on its own.
- No key is ever written anywhere; the tests prove it.

**Non-goals**

- Persisting AWS keys (no key store, no "remember me").
- A visual `AudioPlayer` control, an S3 *file manager*, EC2 provisioning, IAM administration beyond the stack.
- Linking the AWS SDK, or any TLS stack other than the runtime's existing `native-tls`.
- Live verification against a real AWS account (none is available to the implementing agent; the operator's
  opt-in smoke test, AC20 of spec 078, is extended to cover the new operations).

## 4. Requirements (EARS)

### 4.1 Part A — AWS extensions (amending spec 078)

- **R1 (ubiquitous):** `AwsRekognition` shall offer `RecognizeCelebrities(image)` for a local file or `s3://bucket/key`;
  rows `Name`, `Id`, `MatchConfidence`, `Urls` and the face's `Left`, `Top`, `Width`, `Height` (ratios of the image),
  raising `onCelebrities`. It is read-only.
- **R2 (ubiquitous):** The `DetectFaces` rows shall also expose `Left`, `Top`, `Width`, `Height`.
- **R3 (ubiquitous):** `AwsS3` shall offer `ListBuckets()` (rows `Name`, `CreationDate`), raising `onBucketsListed`; read-only.
- **R4 (ubiquitous):** `AwsMcp` shall offer `AwsCall(service, operation, paramsJson)` calling one AWS API through the hosted
  server with the service and operation **as literals the route renders, never as script text** (so the existing
  guard against COBOL-built scripts still holds), `mutating = true` (needs `AllowWrite`), rows from the API's answer,
  raising `onAwsCalled`. It is the generic path CloudFormation and `create_table` use. *(A failing API call shall raise
  `onError` with the AWS error code, not a silent empty answer.)*
- **R5 (event):** When COBOL calls `COBOL::"AWS-SET-CREDENTIALS" ( connection key-id secret token )`, the runtime shall
  keep the three values **in memory only** for that connection and use them, for any server it starts afterwards or
  restarts, as `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and `AWS_SESSION_TOKEN` in the server's environment,
  and shall launch that server **without** the profile argument. An empty token is omitted.
- **R6 (ubiquitous):** `COBOL::"AWS-CLEAR-CREDENTIALS" ( connection )` shall forget them (and stop using the server that
  held them); `COBOL::"AWS-CREDENTIALS-SET" ( connection flag )` shall report `Y`/`N`, never the values.
- **R7 (constraint):** The credentials shall never appear in a `.cfrm`, a project file, a log, `Verbose` output, an error
  message, a `Debug` rendering or a COBOL-readable property; the type that holds them shall redact itself; the
  child-environment builder shall be the only reader. *A test shall plant recognisable fake values and scan every
  surface (arguments, environment of an unrelated child, logs, `Verbose`, errors, saved files) and find none.*
- **R8 (ubiquitous):** Spec 078's R11, AC7, AC8, the Guide's AWS chapter, the `Connection` property help in six languages and
  the module documentation in `aws/` shall be amended to say exactly this and no more (amendment **A6** in spec 078).
- **R9 (ubiquitous):** The fake MCP server fixtures shall answer the new operations, and the AWS demo forms and
  `aws_hosts` shall exercise each (the `DEMOS` tallies are updated).

### 4.2 Part B — media

- **R10 (ubiquitous):** `COBOL::"IMAGE-FIT" ( source target width height status )` shall decode a PNG, JPEG, GIF, WebP,
  BMP or TIFF, correct its EXIF orientation, **crop to fill and resize** it to exactly `width`×`height`, and write
  `target` as PNG or JPEG chosen by the target's extension (JPEG quality 85); `status` is `OK` or an error word. It
  shall refuse an input larger than 64 MP.
- **R11 (ubiquitous):** `COBOL::"IMAGE-INFO" ( path width height format status )` shall read the dimensions and format
  from the file header without decoding the pixels.
- **R12 (ubiquitous):** `COBOL::"HTTP-DOWNLOAD" ( url file http-status status )` shall fetch an `http(s)` URL **as bytes**,
  following up to five redirects, sending only the headers set with `HTTP-SET-HEADER`, with a total timeout, a 256 MB
  cap and a `.part` file renamed on success; it shall never log the URL's query string.
- **R13 (ubiquitous):** The three built-ins shall live behind runtime features (`images`, and `http` for the download),
  be detected from the forms and sources the way `HTTP-GET` is, add nothing to a build that does not call them, and be
  listed in `BUILTINS`, IntelliSense, the System KB, the Guide and the support matrix.
- **R14 (ubiquitous):** A DataGrid **Image** cell whose file does not exist yet shall be retried (not cached as failed for
  the session), so a thumbnail that arrives after the first paint appears; the Image cell kind shall be documented in the
  System KB and the Guide and covered by a render test.

### 4.3 Part C — audio

- **R15 (ubiquitous):** `COBOL::"AUDIO-PLAY" ( file status )` shall start playing an MP3, OGG Vorbis, WAV or FLAC file,
  replacing any sound already playing; `AUDIO-PAUSE`, `AUDIO-RESUME` and `AUDIO-STOP` ( `status` ) control it;
  `COBOL::"AUDIO-STATE" ( state position-ms duration-ms )` reports `IDLE`, `PLAYING`, `PAUSED`, `ENDED` or `ERROR`
  and the position and length in milliseconds.
- **R16 (constraint):** The audio device shall be owned by one dedicated thread; a missing device, an unreadable file
  or an undecodable format shall give `status`/`state` `ERROR`/`NODEVICE` and never a panic or a hang.
- **R17 (ubiquitous):** Audio shall sit behind an `audio` runtime feature; a build whose forms and sources never call
  `AUDIO-*` shall link no audio code. The Linux build prerequisite (ALSA headers) shall be added to `BUILDING-en.md`,
  the README and the CI workflow's package list; macOS and Windows need nothing new.
- **R18 (ubiquitous):** The engine shall be testable without a device: a null sink that "plays" in real time lets the
  state machine, the position clock and the end-of-file transition be tested everywhere, including CI.

### 4.4 Part D — the PowerCloud example

- **R19 (ubiquitous):** `examples/PowerCloud/` shall hold a project (`PowerCloud.project.toml`, `forms/`, `src/`,
  `Assets/`, `cloudformation/`, `README.md`), listed as the fifth entry of `EXAMPLES` with its description in six
  languages, opening like the other four from Help ▸ Examples.
- **R20 (ubiquitous):** The main form shall be frameless (`title-visible="false"`), responsive and rounded, painted in the
  Spatial theme with the image's palette (§5), holding a title panel (*PowerCloud*, brand chip, a close **x**) and a
  TabControl of seven tabs: *Settings*, *DynamoDB*, *S3*, *Lambda*, *Polly*, *EC2*, *Rekognition*.
- **R21 (ubiquitous):** **Settings**: the wheel image on the left; on the right the profile and region, the access key id,
  the secret (masked) and the optional session token, a *Connect* button that proves the credentials with a read-only
  call and says so, the stack name, a *Deploy CloudFormation* button that creates the stack through `AwsCall` and shows its
  progress and result, and a *Save template…* button plus a link to download the template. Keys are handed to the
  runtime with `AWS-SET-CREDENTIALS` and cleared from the form's own fields at once.
- **R22 (ubiquitous):** **DynamoDB**: *Create table* (idempotent), a filter row, a DataGrid (ID, Photo 40×40, Name, Race,
  Age, View, Edit, Delete), *New pet*; a popup form for new/edit with the photo dropzone; View opens a read-only popup
  with the 400×400 photo. Saving a photo fits it to 400×400, uploads it to the bucket as `photos/<id>.jpg` and stores the
  key; thumbnails come from the Lambda's temporary URL and `HTTP-DOWNLOAD` into a local cache; Delete removes the item and
  the object after a confirmation.
- **R23 (ubiquitous):** **S3**: a bucket dropdown filled by `ListBuckets`, a DataGrid of the chosen bucket's objects
  (key, size, modified), read-only.
- **R24 (ubiquitous):** **Lambda**: the function's source, how the template deploys it, how `AwsLambda.Invoke` calls it and
  what it returns, with a *Try it* button that invokes it for a key and shows the temporary URL (masked past the host).
- **R25 (ubiquitous):** **Polly**: a language dropdown (mapped to a voice), a multiline text box, *Speak*, and the player
  row (Play/Pause/Stop, progress, time) driven by `AUDIO-*`.
- **R26 (ubiquitous):** **EC2**: a region dropdown (named connections), a DataGrid (Instance ID, Name, Class, vCPU, Memory,
  State, a Switch column), vCPU and Memory from a table of instance types in COBOL; turning a switch starts or stops the
  instance, opens a modal *Working…* window and closes it when `Describe` reports the target state.
- **R27 (ubiquitous):** **Rekognition**: a dropzone accepting only `jpg`, `jpeg` and `png`, the preview, *Recognise*; each
  recognised celebrity's box is drawn over the preview (a pool of rounded-rectangle shapes placed from the returned
  ratios and the picture's displayed rectangle) and listed at the right with name, confidence and links.
- **R28 (ubiquitous):** `examples/PowerCloud/cloudformation/powercloud.yaml` shall create the S3 bucket (private, encrypted,
  with a lifecycle rule for abandoned uploads), the Lambda function `powercloud-presign` (inline Python) with a
  least-privilege role, an IAM managed policy for the operator who runs the demo, optionally the table
  (`CreateTable`) and optionally a small demo instance (`CreateDemoInstance`), and shall output everything the application
  needs (bucket name, function name, policy ARN). It shall contain no wildcard administrator permission.
- **R29 (ubiquitous):** The example shall hold **no credential**; `every_example_form_saves_unchanged_091`,
  the example credential scan and the corpus goldens shall cover it.
- **R30 (ubiquitous):** A hosted test (`powercloud_hosts.rs`, modelled on `aws_hosts.rs`) shall run each tab's flow under
  `rcrun run-form --headless` against the fake MCP server and report a quantified summary (GOLDEN RULE #7).

### 4.5 Part E — documentation, knowledge, i18n

- **R31:** The Developer's Guide shall document the new built-ins (§11 *The built-ins*), the AWS additions and A6 (the AWS
  chapter), the DataGrid Image/Button/icon cell kinds (chapter 8), the audio prerequisites (§3) and the PowerCloud example
  (the examples section); the support matrix and `DEPENDENCIES-en.md` shall be updated; **English only** (GOLDEN RULE #8).
- **R32:** The System KB tables and `assets/knowledge/chunked.data` shall document every new built-in and operation, and
  be regenerated once, on the integration branch.
- **R33:** Every new user-visible IDE string (the example's description, any message) shall be a `Tr` field in six languages.

## 5. The palette (measured from the operator's image, 800×900)

| Role | Value |
|---|---|
| Window / page background | `#050A26` (deep navy) |
| Panel and wedge grey | `#292A35`, ring `#202133` |
| Text | `#F0F0F0` |
| Title chip gradient | `#4AC9F4` → `#6952EF` |
| Accent (AWS orange) | `#ED9B22` |

## 6. Acceptance criteria

- [ ] **AC1 (R1–R3, R9):** each new operation is driven by a demo form against the fake server and checks its rows and
  events; `aws_hosts` passes with the updated tallies.
- [ ] **AC2 (R4):** `AwsCall` is refused without `AllowWrite`, raises `onError` carrying the AWS error code on a failing call,
  and passes `no_op_accepts_script_text_from_cobol`.
- [ ] **AC3 (R5–R8):** the credential-surface scan (R7) finds none of the planted values anywhere; with credentials set the
  server's environment carries exactly the three variables and the launch carries no profile; clearing them restarts
  the server without them.
- [ ] **AC4 (R10–R13):** `IMAGE-FIT` of a 1200×800 JPEG gives a 400×400 file whose centre is the source's centre; a
  portrait with EXIF rotation is upright; `IMAGE-INFO` agrees with the decoder; `HTTP-DOWNLOAD` saves the exact bytes of
  a binary body from a local test server, follows a redirect, refuses a 404 with `http-status 404`, and a program
  without these calls builds without the `images` feature.
- [ ] **AC5 (R14):** a grid whose Image cell file appears after the first paint shows it within one repaint.
- [ ] **AC6 (R15–R18):** the null-sink engine plays a 2 s WAV: `PLAYING`, a position that advances, `ENDED` at the length;
  pause holds the position; stop gives `IDLE`; a missing file gives `ERROR`; no device gives `NODEVICE`, no panic.
- [ ] **AC7 (R19–R27, R30):** `powercloud_hosts` runs every tab headless against the fake server and reports
  the operations exercised, the calls made and the time per phase.
- [ ] **AC8 (R28):** a test parses the template (YAML), checks its resources, parameters and outputs, that no statement
  allows `*:*`, and that the Lambda's inline code compiles as Python-free text (no tab characters, under the 4 KB inline limit).
- [ ] **AC9 (R29):** no credential in any new file; the 091 round-trip guard passes for the new forms.
- [ ] **AC10 (R31–R33):** the Guide, the KB (`prebuilt_chunked_kb_matches_the_published_documentation`), IntelliSense and
  the six-language `Tr` tables are complete.
- [ ] **AC-live (opt-in):** with an operator-provided account and `COBOLT_AWS_LIVE=1`, a smoke test deploys nothing but
  calls one read-only operation per service the example uses and reports what answered.

## 7. Constraints & steering check

- **No C toolchain beyond the existing set:** `image`'s pure-Rust codecs only; audio is `rodio` + `symphonia` (pure
  Rust decoders) and needs, on Linux only, the ALSA development package at build time — recorded in BUILDING and CI.
  macOS (CoreAudio) and Windows (WASAPI) add no prerequisite.
- **Interpreter-binary parity:** every built-in lives in the interpreter, so `rcrun run-form`, embedded child forms and the
  compiled binary get it; the feature scan in `cobolt-compiler` must switch the features on for the compiled binary.
- **Windows never resize themselves:** the popups and the *Working…* window own explicit sizes.
- **Built-ins are written inline** (`COBOL::"NAME" ( … )`); COBOL in the example uses the RustCOBOL extensions
  (properties as data items, method results as expressions, value methods, inline built-ins) and declares no scratch item
  that only carries a value.
- **COBOL identifiers stay English** whatever the IDE language.
- **Secrets:** nothing is added to `CREDENTIAL_PROPS`; the new store is not a property.

## 8. Open questions

- **Q1 — Does the hosted AWS MCP proxy honour credentials from the environment instead of a profile?** It is boto3-based
  and boto3's default chain reads `AWS_ACCESS_KEY_ID` & co., but the repository only records that it signs "with the
  connection's profile". *Plan:* launch without the profile argument when credentials are set; the opt-in live smoke test
  (AC-live) is where this is finally confirmed.
- **Q2 — Which exact tool carries a single API call on the hosted server (`aws___run_script` vs `aws___call_aws`)?**
  Spec 078 Q1, still open; the route table keeps the answer in data, and `AwsCall` follows whatever Q1 settles.
- **Q3 — Can a `Connection` be switched at run time?** The runtime re-reads the property on every call, so it should;
  no test covers it. *Plan:* add one (WP1) before the EC2 tab relies on it.

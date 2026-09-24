# Spec — Application model list and key store

- **Status:** implemented (1.70.194); AC9 and AC12 partly open, see §6a
- **Folder:** specs/076-application-model-list/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-24
- **Parents:** `specs/071-powerchat/spec.md` (§4.6 R33–R37),
  `specs/068-application-knowledge-base/spec.md` (R24 — the endpoint embedder),
  `specs/063-rag-chatbot-boilerplate/spec.md` (R62). The operator's answers of
  2026-09-24 win where these differ.

## 1. Overview

Today an `AgentObject` in a built application takes its model from one of two
places, both fixed before the application starts: its own properties, or a
Model Provider named in its `Configuration` property, which a deployed
application receives through the `COBOLT_AGENT_PROVIDERS` and
`COBOLT_CONNECTION_KEY_<PROVIDER>` environment variables. An end user has no
way to add a model or change a key.

This spec gives a running application a **model list** its users maintain — in a
settings form the application's developer writes in COBOL — and a **key store**
the runtime keeps. The program owns the list, in its own indexed file, and
hands its entries to the runtime; the runtime owns only the keys. An
`AgentObject` can then be pointed at a list entry by name, as can a
`KnowledgeBase`'s endpoint embedder.

The key store is a **seam**: for now it keeps keys in a settings file; the
operating system's keychain can replace that later without the form, the COBOL
or the list changing.

## 2. Goals / Non-goals

**Goals**

- An end user adds a model, changes a key, or retires a model without a
  rebuild.
- Keys are entered once, never shown again, never written anywhere a program,
  a log or a model can read them back.
- The storage behind the key store can change later without touching any
  application.
- Existing deployments that use the environment variables keep working.

**Non-goals**

- The settings form itself. It is COBOL in each application (PowerChat's is
  071's); this spec supplies what the form needs.
- The OS keychain. The seam is built; the keychain store is not.
- Sharing the list or keys across machines or users on a LAN.
- Validating that a model entry actually works before it is used.

## 3. User stories

- As an **administrator**, I add our company's model server to the
  application's settings, enter its key once, and every agent can use it.
- As an **administrator**, I rotate a key, and the application uses the new one
  from the next request, with no restart.
- As a **COBOL developer**, I write a settings form that lists, adds, edits and
  removes models, keeping the list in my own indexed file, and I never handle a
  key after the user types it.
- As a **developer**, I point an agent at "the company model" by name, not by
  URL, so the administrator decides what that is.
- As an **operator**, my existing deployment that sets
  `COBOLT_AGENT_PROVIDERS` keeps working after the upgrade.

## 4. Requirements (EARS)

### 4.1 The list belongs to the program

- **R1 (ubiquitous):** A model entry shall carry a name, an API (the protocols
  `AgentObject` already supports), an endpoint URL and a model; its key is kept
  apart (§4.2).
- **R2 (ubiquitous):** The program shall keep its model list in an indexed file
  of its own, and hand each entry to the runtime by name while the application
  runs (operator, 2026-09-24).
- **R3 (ubiquitous):** A program shall be able to add, change and withdraw an
  entry at any time; a change reaches the next request that uses it, without a
  restart.
- **R4 (constraint):** The runtime shall not store the list. Entries last for
  the life of the program; it hands them over again at start-up (as 075 R6a
  does for registered files).

### 4.2 The key store belongs to the runtime

- **R5 (ubiquitous):** A program shall be able to store a key for an entry
  name, replace it, and remove it.
- **R6 (constraint):** A program shall never be able to read a stored key back.
  It may ask only whether a key is stored for a name, so a form can show "a key
  is set" without showing the key.
- **R7 (constraint):** A key shall never appear in a log, an error message, a
  diagnostic dump, or anything sent to a model. Wherever a request is reported,
  its key is masked, as `AgentObject` already masks it.
- **R8 (ubiquitous):** The key store shall be reached through a seam, so that a
  store backed by the operating system's keychain can replace the file-backed
  one without any change to programs, forms or lists (071 R34).
- **R9 (ubiquitous):** The first store shall keep keys in a settings file **in
  the application's installation folder, shared by every user of that
  installation** (operator, 2026-09-24).
- **R10 (constraint):** Keys in that file shall be stored **encrypted with a key
  derived from the installation**, so the file is unreadable at a glance and
  useless copied to another machine; it shall hold nothing but keys (operator,
  2026-09-24).
- **R11 (event):** When the settings file is missing, the store shall start
  empty; when it is unreadable, the store shall report that and start empty,
  never overwriting the unreadable file.

### 4.3 Using an entry

- **R11a (ubiquitous):** A program shall manage entries and keys through runtime
  CALLs — `COBOL-MODEL-SET`, `COBOL-MODEL-REMOVE`, `COBOL-KEY-SET`,
  `COBOL-KEY-REMOVE`, `COBOL-KEY-IS-SET` — since both are process-wide
  (operator, 2026-09-24).
- **R12 (ubiquitous):** An `AgentObject` shall be able to take its API,
  endpoint, model and key from a list entry, chosen by name at run time through
  its **`ModelEntry`** property.
- **R13 (ubiquitous):** A `KnowledgeBase`'s endpoint embedder shall be able to
  take its endpoint, model and key from a list entry the same way, through its
  own `ModelEntry` property (068 R24).
- **R14 (ubiquitous):** Where an `AgentObject` uses a list entry, that entry
  wins. Otherwise its design-time `Configuration` resolves through the
  environment variables as today; otherwise its own properties (operator,
  2026-09-24).
- **R15 (event):** When an agent names an entry that does not exist, or one with
  no key where its API needs one, the agent shall fail its request at once with
  `onError`, naming the entry and what is missing, and send nothing.
- **R16 (event):** When an entry an agent uses is changed or withdrawn, the
  program shall be told, so it can act — PowerChat re-runs its election
  (071 R36).
- **R17 (ubiquitous):** The model, temperature, token limit and timeout an
  `AgentObject` sets on itself shall still apply when it uses an entry, as they
  do with a `Configuration` today.

### 4.4 Everywhere the same

- **R18 (ubiquitous):** The list and the key store shall behave identically
  under `rcrun run-form`, in an embedded child form, and in the compiled
  binary.
- **R19 (constraint):** They shall live in the runtime, with no dependency on
  the IDE or on `cobolt-agents`. They shall not read the IDE's Model Providers
  or its machine-local key store: a built application has its own.

## 5. Acceptance criteria

- [x] **AC1** — A program hands the runtime two entries at start-up, an agent
      pointed at one by name answers through it, and repointing it at the other
      takes effect on the next question with no restart. *(R1–R3, R12)*
- [x] **AC2** — After a restart with no entries handed over, no entry exists;
      the runtime wrote none to disk. *(R4)*
- [x] **AC3** — A key stored, replaced and removed changes what the next request
      sends; a program asking for the key gets only "set" or "not set". *(R5,
      R6)*
- [x] **AC4** — With a key stored, a search of every log, error, diagnostic dump
      and captured model request finds no occurrence of it. *(R7)*
- [x] **AC5** — A test key store (in memory) replaces the file store with no
      change to the program under test, which behaves identically. *(R8)*
- [x] **AC6** — The keys file sits in the installation folder, holds no key in
      plain text, and a second user of the installation uses the stored key;
      the same file copied to another installation decrypts nothing. *(R9,
      R10)*
- [x] **AC7** — A missing keys file gives an empty store; a corrupt one is
      reported, left as it was, and the store starts empty. *(R11)*
- [x] **AC8** — A `KnowledgeBase` embeds through a list entry. *(R13)*
- [ ] **AC9** — An agent with a list entry, a `Configuration` and its own
      properties uses the entry; without the entry, the `Configuration` through
      the environment variables; without both, its own properties. *(R14)*
- [x] **AC10** — An unknown entry, and an entry missing a required key, each fail
      at once with `onError` naming the problem, and no request is sent. *(R15)*
- [x] **AC11** — Changing an entry in use raises the notification R16 names.
      *(R16)*
- [ ] **AC12** — The same program behaves identically under `rcrun run-form`, as
      an embedded child form, and as a compiled binary; `cargo tree` shows
      neither `cobolt-ide` nor `cobolt-agents`. *(R18, R19)*

## 6. Constraints & steering check

- **Fix vs feature.** Feature: a new run-time capability. `features` branch.
- **i18n.** No IDE text beyond any new property or method names, which stay
  English. What a settings form shows its users is the application's own text.
- **System KB.** New `AgentObject` and `KnowledgeBase` members and the key-store
  surface are documented in the `cobolt-compiler` doc tables, and
  `assets/knowledge/chunked.data` is regenerated in the same change.
- **Developer's Guide.** *Where an agent's credentials live* gains the run-time
  list, the key store, the precedence of R14, and a short settings-form
  example. GOLDEN RULE #8 applies.
- **Security note for the guide.** R9 puts the keys file where every user of
  the installation can read it, obscured but not encrypted with a secret the
  user holds. The guide must say so plainly, and name the OS keychain as the
  way that risk will be closed.
- **Interpreter–binary parity.** R18; read the `interpreter-binary-parity`
  skill before planning.

## 6a. Implementation notes (1.70.194)

- **⚠️ R12 vs R17 on the model — resolved one way, for the operator to
  confirm.** R12 says the entry supplies the model; R17 says the model an
  agent sets on itself still applies. Implemented as: **the entry's model when
  it names one, the agent's own otherwise.** Temperature, token limit and
  timeout are always the agent's.
- **AC2.** `model_list` has no file-system code; entries live in a process
  global. Verified by construction, not by a restart test.
- **AC4.** Tested: the program's display output with the verbose agent log on
  (46 lines), `LastError`, and every captured request apart from the
  Authorization header the key belongs in. The IDE's diagnostic dump is not
  exercised by these tests.
- **AC6.** "A second user of the installation" is tested as a second store
  opened on the same folder. The file is shared, and the cipher key depends
  on the folder and host name, not on the user.
- **AC9 — partly open.** Entry over own settings is tested end to end.
  `Configuration` resolves at seed time into the same `AgentAPI`/`AgentURL`/
  `AgentAPIKey` properties that an entry overrides at `Ask` time, so the entry
  also wins over it. That order is unchanged code, but it has no end-to-end
  test here.
- **AC12 — partly open.** The list and store are process globals in the
  runtime, used identically by every host (child forms share the process).
  `cargo tree` shows no `cobolt-ide` or `cobolt-agents`. A full three-host run
  is an operator step.
- **Keys file:** `<app>/settings/model-keys.dat`, where `<app>` is the folder
  every host anchors assets on (`cobolt_forms::assets::current_base`).

## 7. Open questions

All settled with the operator on 2026-09-24:

- **Q1 — ✅ The COBOL surface:** runtime CALLs for the list and the keys, and a
  `ModelEntry` property on `AgentObject` and `KnowledgeBase` (R11a, R12, R13).
- **Q2 — ✅ "Obscured":** encrypted with a key derived from the installation;
  the guide states that anyone who can run the application on that machine can
  use the keys (R10).

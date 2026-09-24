# Spec — Registered indexed files

- **Status:** draft → awaiting operator review
- **Folder:** specs/075-registered-indexed-files/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-09-24
- **Parents:** `specs/065-cobolt-mcp/spec.md` (the indexed-file tool),
  `specs/063-rag-chatbot-boilerplate/spec.md` (R42 "External Files"),
  `specs/071-powerchat/spec.md` (§4.4 R20–R25, R10b–R10d). The operator's
  answers of 2026-09-24 win where these differ.

## 1. Overview

Today a model can query an indexed file only if the program was **compiled**
with that file's `FD`: 065's tool takes the record layout from the program and
trusts a delivered `.cidx` only for its descriptions. That limits a chatbot to
the files its developer knew about.

This spec lets a program hand the model an indexed file **by its path**, while
the application runs: a file on the user's disk, on a network share in the
operating system's own form, or at an `smb://` address. The record layout comes
from the file's `.cidx`. The file is the user's data, so it is only ever read —
`OPEN INPUT`, never changed — and it is held in memory when it fits, read from
disk when it does not.

Nothing here is specific to PowerChat: any application built with
PowerRustCOBOL gets it.

## 2. Goals / Non-goals

**Goals**

- An end user can give the chatbot a new data file without a rebuild.
- A registered file is described to the model as well as a compiled one.
- The user's file is never changed, by anything, ever.
- A file that is too large for memory is still searchable when it can be, and
  refused with the reason when it cannot.

**Non-goals**

- Writing to a registered file, in any open mode.
- Discovering files: the program supplies each path.
- A place to keep share credentials securely. For now they travel in the path,
  or the share is open to guests (R11, R12); the key store is spec 076.
- Mounting network shares on the user's behalf.

## 3. User stories

- As an **end user**, I point the chatbot at the orders file on the finance
  server, and it answers questions about orders.
- As an **end user**, I register a file that lives on a colleague's shared
  folder by its `smb://` address, on any operating system.
- As an **end user**, I am told clearly when a file cannot be used — missing,
  unreachable, undescribed, or too large — and the chatbot answers without it.
- As an **operator**, I know the chatbot can never alter the data it reads.
- As a **developer**, I register a file with one call and let the model use it,
  exactly as I would a compiled one.

## 4. Requirements (EARS)

### 4.1 Registering a file

- **R1 (ubiquitous):** A program shall be able to give an `AgentObject` an
  indexed file by the path of its data file and the path of its `.cidx`, while
  the application runs, with no `FD` for it in the program.
- **R2 (ubiquitous):** The model shall then query it with the same tool and the
  same search behaviour as a file allowed with 065's `AllowFile`.
- **R3 (ubiquitous):** The record layout — fields, offsets, lengths, `PIC`,
  `USAGE`, `OCCURS`, `REDEFINES` — and the keys shall be taken from the `.cidx`
  (071 R23; reverses, for registered files only, 065's rule that a delivered
  `.cidx` is trusted for descriptions alone).
- **R4 (event):** When the `.cidx` does not match the data file — a record
  length or key layout the file does not have — the registration shall be
  refused with the reason, before the model sees the file.
- **R5 (constraint):** A `.cidx` that describes no purpose, or no fields, shall
  be refused with the reason (065's rule; 071 R25).
- **R6 (ubiquitous):** A program shall be able to withdraw a registration.
- **R6a (constraint):** A registration shall last for the life of the program
  only. The runtime shall not store it; a program that wants it back after a
  restart registers the file again at start-up, so the runtime never keeps a
  path — or a password inside one (operator, 2026-09-24).

### 4.2 Where the file may be

- **R7 (ubiquitous):** A path may be a local path in the operating system's own
  form.
- **R8 (ubiquitous):** A path may be a network path in the operating system's
  own form: a UNC path (`\\server\share\…`) on Windows, a mounted share
  (`/Volumes/…`) on macOS, a mount point (`/mnt/…`, `/media/…`) on Linux.
- **R9 (ubiquitous):** A path may be an `smb://server/share/…` address on every
  operating system, read without the share being mounted (operator,
  2026-09-24).
- **R10 (event):** When a path cannot be reached — no such file, server down,
  access refused — the program shall be told which file and why, and the model
  shall answer without it rather than fail (071 R22).
- **R11 (optional):** Where an `smb://` address carries credentials
  (`smb://user:password@server/share/…`), they shall be used to log in
  (operator, 2026-09-24: for now).
- **R12 (optional):** Where it carries none, the share shall be read as a guest.
- **R13 (constraint):** A password in an `smb://` address shall never be shown,
  logged, or passed to the model; wherever the address is displayed or
  reported, the password is masked.

### 4.3 Read only, always

- **R14 (constraint):** A registered file shall be opened `OPEN INPUT` only,
  never `I-O`, `OUTPUT` or `EXTEND` (071 R10c).
- **R15 (constraint):** Opening and reading a registered file shall never change
  a byte of it — no write-back, no conversion to another format, no journal
  recovery — and shall need **no write permission**, so a file on a read-only
  share or marked read-only is readable.
- **R16 (event):** When a file cannot be read without writing to it — an
  interrupted write left a recovery journal beside it — the registration shall
  be refused with the reason, and the file left as it is.

### 4.4 Memory, with a disk fallback

- **R17 (ubiquitous):** A registered file shall be held in memory
  (`STORAGE MODE IS MEMORY`) when it fits (071 R10b).
- **R18 (ubiquitous):** A file fits when it is under the project's memory limit
  (065's setting) **and** its size is at most **half** of the machine's free
  memory at the moment it is opened (operator, 2026-09-24).
- **R19 (event):** When a file does not fit, and it is a **local** file in the
  DISK format (`PRCIDXD1`), it shall be read **in place** from disk, and the
  program told that it was (operator, 2026-09-24; 071 R10d).
- **R20 (event):** When a file does not fit and cannot be read in place — an
  `smb://` file, or a file in the MEMORY format (`PRCIDX1`) — it shall be
  refused with the reason and both numbers (its size, and the limit or free
  memory it exceeded).
- **R21 (constraint):** The fallback shall never copy, convert or rewrite the
  user's file (R15).

### 4.5 Everywhere the same

- **R22 (ubiquitous):** Registration shall behave identically under `rcrun
  run-form`, in an embedded child form, and in the compiled binary.
- **R23 (constraint):** The capability shall live in the runtime, with no
  dependency on the IDE or on `cobolt-agents`.

## 5. Acceptance criteria

- [ ] **AC1** — A program with no `FD` for a file registers it by path, and the
      model answers a question from it. *(R1–R3)*
- [ ] **AC2** — A `.cidx` whose record length or keys disagree with the file is
      refused before the model sees it; one with no purpose or no field
      descriptions is refused with that reason. *(R4, R5)*
- [ ] **AC3** — A withdrawn registration is no longer offered to the model.
      *(R6)*
- [ ] **AC4** — The same file registered by a local path, by the OS's network
      path, and by `smb://` gives the same answers. *(R7–R9)*
- [ ] **AC5** — A missing file, an unreachable server and a refused login are
      each reported by name with the reason, and the model still answers.
      *(R10)*
- [ ] **AC6** — An `smb://` share is read with credentials in the address, and a
      guest share with none; no log line, report or model request contains the
      password. *(R11–R13)*
- [ ] **AC7** — After every test in this spec, each registered file is
      byte-identical to its state before, with the same modification time.
      *(R14, R15, R21)*
- [ ] **AC8** — A registered file with no write permission, and one on a
      read-only share, are both read. *(R15)*
- [ ] **AC9** — A file with a recovery journal beside it is refused, and neither
      it nor the journal changes. *(R16)*
- [ ] **AC10** — A local `PRCIDXD1` file over the limit is read in place and the
      program is told so; the same file under the limit is held in memory.
      *(R17–R19)*
- [ ] **AC11** — A file under the project limit but larger than a test's free
      memory figure falls back as in AC10. *(R18)*
- [ ] **AC12** — An `smb://` file and a local `PRCIDX1` file, each too large, are
      refused with both numbers. *(R20)*
- [ ] **AC13** — The same program gives the same results under `rcrun
      run-form`, as an embedded child form, and as a compiled binary; `cargo
      tree` shows neither `cobolt-ide` nor `cobolt-agents`. *(R22, R23)*
- [ ] **AC14** — Tests report quantified results: files registered, records
      searched, open time in memory and from disk, searches per second
      (GOLDEN RULE #7).

## 6. Constraints & steering check

- **Fix vs feature.** Feature: files the model can use beyond those compiled
  in. `features` branch. **R15/R16 also expose a defect in the shipped runtime**
  (below), which is a fix and goes separately on `fixes`.
- **i18n.** No IDE text. Reasons reach the program as codes plus a message it
  can translate in its own tables.
- **System KB.** New `AgentObject` members, the refusal reasons, and the
  fallback are documented in the `cobolt-compiler` doc tables, and
  `assets/knowledge/chunked.data` is regenerated in the same change.
- **Developer's Guide.** The indexed-file-tool chapter gains *Registering a
  file by path*: path forms, `smb://`, read-only guarantees, the fit test and
  the fallback. GOLDEN RULE #8 applies.
- **Toolchain.** The `smb://` client must be pure Rust: **`smb2` 0.26.0**
  (MIT OR Apache-2.0; operator, 2026-09-24). `smb` 0.12.1 was rejected — it
  hard-requires `sspi` with `ring` (C/asm) — and so was `remotefs-smb`, whose
  default back end is Samba's C `libsmbclient`. See `plan.md`.
- **Interpreter–binary parity.** R22; read the `interpreter-binary-parity`
  skill before planning.

**Found while specifying — defects in shipped code (fixes, not this spec):**

1. **The DISK engine opens every file with write access, even for `INPUT`**
   (`indexed_disk.rs`, `open`: `OpenOptions::new().read(true).write(true)`),
   and runs journal recovery, which writes. A read-only file cannot be opened
   `INPUT` at all.
2. **1.70.173's conversion runs on `OPEN INPUT` too.** A DISK program opening a
   MEMORY-format file converts it in place even when it only reads, which
   changes a file that was opened only to be read.

Both contradict the operator's rule that an `INPUT` open never changes a file,
and both would break R15 and AC7.

## 7. Open questions

All settled with the operator on 2026-09-24:

- **Q1 — ✅ The free-memory margin:** a file fits only if it is at most half of
  the machine's free memory when opened, as well as under the project limit
  (R18).
- **Q2 — ✅ Registrations do not survive a restart:** the program re-registers
  at start-up, and the runtime stores no path (R6a).

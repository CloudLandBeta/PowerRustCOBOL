<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# WASM ABI v1 — the contract between the program module, the player and the host page

- **Status:** draft 1, 2026-10-20. Normative for `cobolt-wasm` (producer of the program module), `cobolt-web-player` and the host-page loader (consumers). A change to this file is a change to all three; bump `abi` in the manifest when a rule here changes incompatibly.
- **Implements:** WASM-010 R9–R14, WASM-017 R7–R12, WASM-002 (host page), WASM-005 (frames), WASM-016 (player).
- **Conventions:** all integers little-endian (WebAssembly's); `varint` is unsigned LEB128 (max 5 bytes, `u32`); `text` is `varint length` followed by that many UTF-8 bytes; offsets are byte offsets into the **arena**.

## 1. Artefacts

| File | Made by | Content |
|---|---|---|
| `program.wasm` (multi-memory variant) | Build (`cobolt-wasm`) | the COBOL of the whole project; memories: 0 = COBOL data, 1 = arena |
| `program.single.wasm` (single-memory variant) | Build | the same code; the arena is a region of memory 0; **no** multi-memory instruction anywhere |
| `player.wasm`, `player.js`, … | the product's build | draws any form; plain and threaded variants |
| `manifest.json` | Build | also embedded in the module as the custom section `prmanifest` |
| forms, stylesheet, assets | Build | as WASM-002/007 |

The module's **only imports** are the ones in §3 (a module with any other import is rejected by the loader). Its **only exports** are the ones in §4 plus its memories.

## 2. Memory

- **Data memory (memory 0):** `fixed size`, declared with equal minimum and maximum pages; generated code contains **no `memory.grow`**. Layout (decided by the generator, recorded in the manifest): `[constants][descriptors][form templates][instance regions: Instances × size per form][task stacks][scratch registers]`.
- **Arena:** a byte array of `arena_size` bytes (from the project's payload limit, WASM-005 R11; default 1 MiB) laid out as

| Region | Offset | Size | Written by |
|---|---|---|---|
| batch | 0 | `batch_size` (default 64 KiB) | module |
| answer | `batch_size` | `answer_size` (default 64 KiB) | host (reply to a `query`) |
| frame-out | … | `frame_size` | module (a remote request) |
| frame-in | … | `frame_size` | host (a remote answer) |

  In the **multi-memory** variant the arena is memory 1 and these offsets are absolute in it. In the **single-memory** variant the arena is a region of memory 0 starting at `pr_arena_base()`; every offset in this document is relative to that base. The host never assumes which variant: it reads `pr_variant()` (§4) and the export `arena` (a `WebAssembly.Memory`; the same object as `memory` in the single variant).
- The host takes a view of a memory **at each use** and never keeps one across a call into the module (WASM-017 R10).

## 3. Imports (module → host), namespace `"pr"`

| Import | Signature | Meaning |
|---|---|---|
| `flush` | `(len: i32)` | The batch region holds `len` bytes of commands (§5). The host applies them in order. Called when the handler ends, suspends, yields, when the batch region is full, and before every `query`. |
| `query` | `(len: i32) -> i32` | The batch region holds `len` bytes: zero or more commands followed by **exactly one query** (§6). The host applies the commands, evaluates the query, writes the answer **text** at the answer region and returns its byte length, or a negative error code (§9). |
| `remote` | `(len: i32) -> i32` | The frame-out region holds a request frame of `len` bytes (WASM-005/014). The host sends it and returns a non-negative **request id**, or a negative error code. The task then returns `SUSPENDED` (§4). |
| `now` | `() -> f64` | Milliseconds since the Unix epoch (UTC). |
| `random` | `() -> f64` | A uniform number in `[0, 1)`. |
| `abort` | `(len: i32)` | Fatal: the batch region holds an `ERROR` command (§5). The host reports it with the form, event and COBOL line, ends the task and keeps the application running. Does not return to the module (the module then traps). |
| *(conformance modules only)* `file_open`, `file_read`, `file_write`, `file_close` | §8 | The sequential-file verbs of the conformance build. The player refuses a module that imports them. |

A host that cannot provide an import the module declares **refuses to start** with the message of WASM-002 R15 (WASM-010 AC9).

## 4. Exports (host → module)

| Export | Signature | Meaning |
|---|---|---|
| `memory` / `arena` | memory | data memory / arena (the same object in the single-memory variant) |
| `pr_abi` | `() -> i32` | the ABI version this module was built for (`1`) |
| `pr_variant` | `() -> i32` | bit 0 = multi-memory, bit 1 = conformance build |
| `pr_arena_base` | `() -> i32` | base offset of the arena inside its memory (0 in the multi-memory variant) |
| `pr_init` | `() -> i32` | run once after instantiation: initialises static data. `0` = ok |
| `pr_open_form` | `(form: i32, opener: i32) -> i32` | create an instance of form `form` (index in the manifest), `opener` = the instance that opened it or `-1`; allocates its region from the form's template and runs the form's start-up (spec 037 lifecycle). Returns the **instance id** (≥ 0) or a negative error (`-3` = no free instance: the run-time error of WASM-017 R2 with the form's name). The status of the start-up run is available through `pr_status` |
| `pr_close_form` | `(inst: i32)` | run the form's closing handlers, release and reinitialise its region (WASM-017 R5) |
| `pr_event` | `(inst: i32, ctrl: i32, event: i32, argc: i32) -> i32` | run the handler bound to `(ctrl, event)` of instance `inst` with `argc` argument texts read from the answer region as `argc` consecutive `text` values (WASM-010 R10). Returns a **status** (below) |
| `pr_resume` | `(task: i32, len: i32) -> i32` | continue task `task` after a `SUSPENDED` or `YIELDED` status; for a suspension on a remote call the answer frame of `len` bytes is in frame-in. Returns a status |
| `pr_cancel` | `(task: i32)` | abandon a suspended task (the pending request is the host's to cancel) |
| `pr_tick` | `() -> i32` | the task currently owned by the module, or `-1`: lets the host test whether the module is idle |

**Status** (the return value of `pr_event` / `pr_resume`): `0` = DONE (the handler finished; the batch was flushed), `1` = SUSPENDED (waiting; `task` = `pr_tick()`; the host calls `pr_resume` when the answer arrives), `2` = YIELDED (budget exhausted at a loop back-edge; the host calls `pr_resume(task, 0)` on its next turn), `3` = STOPPED (`STOP RUN`/`GOBACK` of the main program: the application ends), negative = error (§9; the `ERROR` command was flushed).

**One task at a time.** The module runs a single task. While it is `SUSPENDED` or `YIELDED` the host **queues** every other event, in order, and delivers them after the task is `DONE` (WASM-010 AC5). A task is therefore identified by `pr_tick()`; `task` arguments exist so that the ABI does not change if concurrency is ever allowed.

## 5. The command batch (module → host)

A batch is a concatenation of commands. Each starts with an **opcode** byte.

| Op | Name | Body | Meaning |
|---|---|---|---|
| `0x01` | `SET` | `path`, `text` | assign the property or indexed member at `path` the value `text` (the interpreter's `COBOL-SET-PROPERTY`: the host converts by the property's type exactly as the desktop host does) |
| `0x02` | `CALL` | `path` | invoke the method at `path` (last segment) and discard any result |
| `0x03` | `DISPLAY` | `text` | `DISPLAY` output to the host's console |
| `0x04` | `ERROR` | `varint code`, `text message`, `varint unit`, `varint line`, `varint form`, `varint event` | a run-time error with its COBOL position (`unit` and `line` index the `prlines` custom section) |
| `0x05` | `LOG` | `varint level`, `text` | diagnostic output, ignored unless the host's debug level admits it |

`path := varint inst, varint ctrl, varint nseg, seg*`; `seg := varint name, varint nargs, text*` (`nargs` argument values as text). The reserved control ids are **`0` = the form itself** (`THIS`), **`1` = the form that opened this one** (`super::`), **`2` = the application object** (`COBOL::` host-side verbs such as `OpenFormSync`, dialogs, navigation); controls of a form are numbered from `3` in the order of the manifest's `forms[i].controls`.

Name ids index the manifest's `names` table, shared by the whole module.

## 6. Queries (module → host, synchronous)

The batch region ends with one query, which is a command whose op is in `0x10…0x1F`:

| Op | Name | Body | Answer |
|---|---|---|---|
| `0x10` | `GET` | `path` | the property's text |
| `0x11` | `CALLR` | `path` | the method's result as text |
| `0x12` | `COUNT` | `path` | the number of members of the collection at `path`, as decimal text |

The host applies the commands before the query so that **a read reflects the handler's own earlier writes** (WASM-017 R11) and returns the answer text at the answer region; the module reads it and continues. No query is issued per loop iteration by generated code unless the COBOL itself reads a control in the loop.

## 7. Events (host → module)

`pr_event(inst, ctrl, event, argc)`: `ctrl` is a control id as in §5 (the form itself is `0`); `event` indexes the manifest's `events` table (`onClick`, `onChange`, … by the names the desktop host uses); the arguments are `argc` `text` values the host has written at the start of the answer region. The module looks up the bound handler; an event with no handler is not delivered by the host (the manifest lists which `(ctrl, event)` pairs have handlers, so the host does not call for the rest — matching "events fire only for bound handlers").

## 8. Conformance build (test hosts only)

The module is built with `--conformance` (WASM-PLAN P6). It additionally imports

| Import | Signature |
|---|---|
| `file_open` | `(name_len: i32, mode: i32) -> i32` — the file name text is at the start of the batch region; `mode` 0 input, 1 output, 2 extend, 3 input/output; returns a handle ≥ 0 or a negative status |
| `file_read` | `(handle: i32, cap: i32) -> i32` — reads the next record into the answer region; returns its length, `-1` end of file, other negatives a status |
| `file_write` | `(handle: i32, len: i32) -> i32` — writes the record in the batch region; returns a status |
| `file_close` | `(handle: i32) -> i32` |

`pr_variant()` bit 1 is set. The loader **refuses** such a module in a page, a Tauri shell or the preview.

## 9. Error codes

`-1` generic · `-2` bad argument · `-3` no free form instance · `-4` unknown form/control/event · `-5` payload too large for the arena · `-6` host refused (permission or policy) · `-7` not connected / transport failure · `-8` cancelled · `-9` timeout · `-10` module trapped. A COBOL run-time error is **not** a trap: it arrives as an `ERROR` command and a negative status; a trap is a bug in the generator and is reported as `-10` with the last known line (the global `$line`, §10).

## 10. The manifest (`prmanifest`, JSON; also `manifest.json`)

```json
{
  "abi": 1,
  "generator": "cobolt-wasm 2.0.x",
  "project": "name",
  "variant": { "multi_memory": true, "conformance": false },
  "limits": { "arena_size": 1048576, "batch_size": 65536, "answer_size": 65536, "frame_size": 393216,
              "budget_blocks": 20000, "data_pages": 17 },
  "names": ["Caption", "Text", "..."],
  "events": ["onClick", "onChange", "..."],
  "forms": [
    { "id": 0, "name": "main-form", "main": true, "instances": 1, "region_size": 4096,
      "controls": [ { "id": 3, "name": "BTN-OK", "type": "Button" } ],
      "handlers": [ { "ctrl": 3, "event": 0, "entry": 17 } ] }
  ],
  "builtins": ["..."],
  "imports": ["flush", "query", "remote", "now", "random", "abort"],
  "sizes": { "module": 0, "rt_functions": 0, "rt_bytes": 0 }
}
```

A custom section **`prlines`** maps a line id to `(unit name, source line, column)`; the generator stores the id of the statement about to run in the exported-for-diagnostics global `$line` before every statement that can raise a run-time error. The standard `name` section names every function (`form.paragraph.block`).

## 11. What a host must do (checklist for the player and the loader)

1. Fetch the module with streaming compilation; `WebAssembly.validate` a minimal multi-memory module to choose the variant; read `pr_abi` and refuse a mismatch.
2. Provide exactly the §3 imports; read the manifest; call `pr_init`; open the main form with `pr_open_form`.
3. After every call into the module apply what the batch asked, in order; never keep a memory view across a call; queue events while the module is busy; resume on the next turn after `YIELDED`.
4. On form destruction call `pr_close_form` and drop every reference the instance held (WASM-017 R5).
5. Treat text as UTF-8 both ways and convert once per direction (WASM-017 R12).

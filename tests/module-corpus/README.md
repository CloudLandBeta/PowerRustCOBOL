# The module corpus

A Desktop Build on the WebAssembly path puts the program into a copy of the PowerRustCOBOL runner as a WebAssembly module (DESKTOP-WASM-001). A program built by any release must keep running on every later runner, unchanged (R5, R23; AC4).

This folder holds the evidence. For every release since 2.0.520, it keeps the modules that release's Build made from the console fixtures. The runner's test runs each of them and requires the output its release recorded:

```
cargo test -p cobolt-runner --test module_corpus -- --nocapture
```

## What is here

| Path | What it is |
|---|---|
| `fixtures/<name>/` | A console program: its sources, the main program first, and `run.toml`. `run.toml` says what the program is given (standard input, arguments, variables, the pinned clock and `RANDOM` seed) and what it must print and answer. |
| `releases/<release>/<name>.wasm.gz` | The module that release's Build made from the fixture, compressed. |
| `releases/<release>/<name>.toml` | The fixture's `run.toml` as it was at that release. |
| `index.toml` | One entry per release: its files with their sha256, and a `chain`. |

The fixtures today:
- **console:** the console, the environment, the arguments, the pinned clock, `RANDOM` and `RETURN-CODE`.
- **ledger:** arithmetic and rounding, edited pictures, strings, a table with `SEARCH`, a `CALL` into Common Code, and `SORT` with input and output procedures. Until 2.0.522 it also wrote an `INDEXED` file declared `STORAGE IS MEMORY`, and the 2.0.520 entry keeps that version. From 2.0.523 a desktop module refuses such a file until disk files arrive (M3), because on the desktop it is read from its file on disk.

When a fixture's output was recorded, it was first checked against `rcrun run`, the interpreter.

## The rules

- **Append-only.** An entry is never deleted or edited, and neither are its files.
  - Each file's sha256 is in the index.
  - Each entry's `chain` covers the previous entry's chain.
  - The test's `KNOWN` list names every entry.

  So a file removed or changed, an entry taken out of the middle, or the last entry taken out all fail the test.
- **One entry per release, at most 256 KiB of compressed modules.**
- **Kept byte for byte on every platform.** `.gitattributes` turns off line-ending conversion here, since a conversion would change the recorded sha256.

## Adding a release's entry

```
cargo run -p cobolt-compiler --example module_corpus_add
```

The tool does the following:
1. Builds each fixture as a Desktop Build does. The fixture must take the WebAssembly path.
2. Takes the module out of the built program.
3. Runs the module alone on this checkout's runner, under the fixture's pins.
4. Refuses unless the module prints and answers exactly what `run.toml` says.
5. Writes `releases/<release>/`, appends the entry, and prints the line to add to `KNOWN` in `crates/cobolt-runner/tests/module_corpus.rs`.

By default, the release is the IDE's version and the date is today. A new fixture added to `fixtures/` joins the next release's entry. The entries already made never change.

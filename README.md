<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL

**PowerRustCOBOL** is a Rust-based RAD (Rapid Application Development) environment
for **RustCOBOL** — a modern COBOL dialect — with a visual form designer, an
interpreter/runtime, a code generator, a debugger, and a single-binary compiler.

| Name | Role |
|------|------|
| **RustCOBOL** | The language / compiler |
| **PowerRustCOBOL** | The RAD IDE (desktop app) |
| **rcrun** | The CLI runtime binary |

## Quick start

```sh
# Build everything
cargo build

# Launch the PowerRustCOBOL IDE
cargo run -p cobolt-ide

# Run a RustCOBOL program with the CLI
cargo run -p cobolt-cli -- run myprogram.cbl

# Check a program (parse + semantic analysis only)
cargo run -p cobolt-cli -- check myprogram.cbl

# Compile a project into a single native binary
cargo run -p cobolt-cli -- build cobolt.toml
```

## License and generated applications

PowerRustCOBOL is licensed under the Apache License, Version 2.0.

Applications, source code, forms, assets, project files, binaries, packages, and
other artifacts created by users with PowerRustCOBOL are owned by their
respective authors and may be licensed under any terms chosen by those authors,
including proprietary commercial terms.

The use of PowerRustCOBOL to create, edit, compile, debug, package, or distribute
software does not impose the PowerRustCOBOL project license on the user's own
application code.

However, PowerRustCOBOL itself, including its runtime, standard library, compiler
support code, generated support modules, form engine components, runtime modules,
templates, helper libraries, and any other PowerRustCOBOL-provided components
included, linked, copied, embedded, or bundled with a user application, remain
PowerRustCOBOL components licensed under the Apache License, Version 2.0.

Distribution of applications that include PowerRustCOBOL components must preserve
the required copyright notices, license notices, patent notices, trademark
notices, attribution notices, and NOTICE file contents where applicable.

Using PowerRustCOBOL does not transfer ownership of PowerRustCOBOL components to
the application author.

See [`LICENSE`](LICENSE) for the full Apache-2.0 text and [`NOTICE`](NOTICE) for
the project notice. Additional details are in
[`docs/licensing/`](docs/licensing/) (runtime license, generated-code policy,
third-party notices, and the per-file header templates).

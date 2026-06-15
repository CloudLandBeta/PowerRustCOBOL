# Third-Party Notices

This file lists third-party components distributed with PowerRustCOBOL or included in
PowerRustCOBOL binary distributions.

Keep this file updated whenever third-party code, assets, libraries, fonts, templates, or
binary components are added to the repository or to distributed packages.

## Rust dependencies

PowerRustCOBOL uses Rust crates declared in the workspace `Cargo.toml` and member crate
manifests. Each dependency remains under its own license.

Before distributing binary packages, generate or review dependency license metadata using
your preferred Rust license/compliance tooling (for example `cargo license`).

## Current project-level license

PowerRustCOBOL project code is licensed under the Apache License, Version 2.0, unless a
file explicitly states otherwise.

## Representative third-party components

This is not an exhaustive SPDX bill of materials — run license tooling before release.
Entries reflect major external crates in active use as of v1.22.0:

```text
Component: egui / eframe / egui_extras
Homepage: https://github.com/emilk/egui
License: MIT OR Apache-2.0
Used by: cobolt-ide (desktop UI)

Component: logos
Homepage: https://github.com/maciejhirsz/logos
License: MIT OR Apache-2.0
Used by: cobolt-lexer (COBOL tokeniser)

Component: syntect
Homepage: https://github.com/trishume/syntect
License: MIT
Used by: cobolt-ide (editor syntax highlighting)

Component: rusqlite
Homepage: https://github.com/rusqlite/rusqlite
License: MIT
Used by: cobolt-runtime (SQLite SQL widget backend)

Component: postgres (rust-postgres)
Homepage: https://github.com/sfackler/rust-postgres
License: MIT OR Apache-2.0
Used by: cobolt-runtime (PostgreSQL SQL widget backend)

Component: mysql
Homepage: https://github.com/blackbeam/rust_mysql
License: MIT OR Apache-2.0
Used by: cobolt-runtime (MySQL SQL widget backend)

Component: redb
Homepage: https://github.com/cberner/redb
License: Apache-2.0
Used by: cobolt-runtime (optional indexed-file engine)

Component: bincode
Homepage: https://github.com/bincode-org/bincode
License: MIT OR Apache-2.0
Used by: cobolt-compiler (embedded AST serialisation)

Component: flate2
Homepage: https://github.com/rust-lang/flate2-rs
License: MIT OR Apache-2.0
Used by: cobolt-compiler (AST compression)

Component: ureq
Homepage: https://github.com/algesten/ureq
License: MIT OR Apache-2.0
Used by: cobolt-ide (AI assistant), cobolt-runtime (HTTP built-ins)

Component: mermaid-rs-renderer / resvg / genpdf
Homepage: https://github.com/mermaid-js/mermaid-rs-renderer (and related)
License: varies (MIT / Apache-2.0 per crate — verify before release)
Used by: cobolt-ide (documentation viewer: Mermaid diagrams, PDF export)
```

## Template entry

```text
Component:
Homepage:
License:
Copyright:
Used by:
Notes:
```
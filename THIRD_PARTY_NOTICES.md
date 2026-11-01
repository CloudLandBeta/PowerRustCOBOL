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
Component: Bulma 0.9.4 (CSS framework)
Homepage: https://bulma.io — https://github.com/jgthms/bulma
License: MIT — Copyright (c) 2022 Jeremy Thomas (full text: crates/cobolt-forms/src/bulma-0.9.4.LICENSE)
File: crates/cobolt-forms/src/bulma-0.9.4.min.css (unmodified), embedded in every binary
Used by: cobolt-forms (the Viewer styles a page that links Bulma with it)

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

Component: rodio 0.22
Homepage: https://github.com/RustAudio/rodio
License: MIT OR Apache-2.0
Used by: cobolt-runtime, feature `audio` (the AUDIO-… built-ins); in rcrun, the IDE
and any built application that plays sound

Component: cpal 0.17
Homepage: https://github.com/RustAudio/cpal
License: Apache-2.0
Used by: rodio (the audio output device)

Component: symphonia 0.5 (symphonia, -core, -metadata, -bundle-mp3, -bundle-flac,
           -codec-vorbis, -codec-pcm, -format-ogg, -format-riff, -utils-xiph)
Homepage: https://github.com/pdeljanov/Symphonia
License: MPL-2.0
Used by: rodio (MP3, FLAC, Vorbis/OGG and WAV decoding)
Notes: Unmodified. MPL-2.0 is a file-level copyleft: its Source Code Form is
       available from the homepage above and from https://crates.io/crates/symphonia
       (each crate of the family under its own name, version 0.5.5).
```

```text
Component: AWS SDK for Rust — credential and signing crates: aws-config 1.8, aws-sigv4 1.4,
           aws-credential-types, aws-runtime, aws-types, aws-smithy-async, -http, -json,
           -observability, -query, -runtime, -runtime-api, -runtime-api-macros, -schema, -types, -xml,
           and the four sign-in clients aws-sdk-sso, aws-sdk-ssooidc, aws-sdk-sts, aws-sdk-signin
Homepage: https://github.com/smithy-lang/smithy-rs — https://github.com/awslabs/aws-sdk-rust
License: Apache-2.0
Used by: cobolt-runtime, feature `aws` (signing the requests to the hosted AWS MCP Server and reading
the AWS sign-in); in rcrun, the IDE and any built application that contains an AWS control
Notes: Unmodified. Built with the SDK's default HTTPS client (hyper + rustls/aws-lc-rs) switched off; the
       product supplies its own over ureq and the operating system's TLS. No AWS service client is linked.

Component: RustCrypto elliptic-curve family — p256 0.13, ecdsa, elliptic-curve, primeorder, crypto-bigint,
           rfc6979, sec1, signature, der, spki, pkcs8, pem-rfc7468, base16ct, base64ct, const-oid; and ff, group
Homepage: https://github.com/RustCrypto — https://github.com/zkcrypto/ff
License: Apache-2.0 OR MIT (ff and group: MIT/Apache-2.0)
Used by: aws-config (the P-256 key of an `aws login` session); pure Rust

Component: time 0.3 (time, time-core, time-macros), deranged, num-conv, powerfmt
Homepage: https://github.com/time-rs/time
License: MIT OR Apache-2.0
Used by: the AWS crates (date and time handling in aws-smithy-types)

Component: arc-swap, base64-simd, vsimd, outref, bytes-utils, regex-lite, xmlparser, http 0.2, http-body 0.4
Homepage: https://crates.io (each under its own name)
License: MIT, or MIT OR Apache-2.0, per crate
Used by: the AWS crates
```

```text
Component: wry 0.57 (web view) and tao 0.35 (window)
Homepage: https://github.com/tauri-apps/wry — https://github.com/tauri-apps/tao
License: wry Apache-2.0 OR MIT; tao Apache-2.0
Used by: cobolt-web-preview (`prc-web-preview`, the headless helper that draws a web form for
render_form, run_form and the tests); nothing a built application links
Notes: Unmodified. No TLS or crypto crate comes with them (no ring, no aws-lc-rs on any target).

Component: objc2 0.6, block2 0.6, objc2-foundation 0.3, objc2-app-kit 0.3, objc2-web-kit 0.3 (macOS)
Homepage: https://github.com/madsmtm/objc2
License: objc2, block2, objc2-foundation MIT; objc2-app-kit, objc2-web-kit Zlib OR Apache-2.0 OR MIT
Used by: cobolt-web-preview (WKWebView's snapshot for `render`); the same versions wry uses

Component: webview2-com 0.39, windows 0.62 (Windows)
Homepage: https://github.com/wravery/webview2-rs — https://github.com/microsoft/windows-rs
License: webview2-com MIT; windows MIT OR Apache-2.0
Used by: cobolt-web-preview (WebView2's CapturePreview for `render`); the same versions wry uses
Notes: The WebView2 runtime itself is Microsoft's, installed with Windows or Edge; not distributed here.

Component: webkit2gtk 2.0.2, webkit2gtk-sys, javascriptcore-rs 1.1, soup3 0.5, gtk 0.18, gdk 0.18,
           gio 0.18, glib 0.18, cairo-rs 0.18 and their -sys crates (Linux)
Homepage: https://github.com/tauri-apps/webkit2gtk-rs — https://github.com/gtk-rs/gtk3-rs — https://github.com/gtk-rs/gtk-rs-core
License: MIT
Used by: cobolt-web-preview on Linux (wry's WebKitGTK backend; WebKitGTK's snapshot for `render`)
Notes: The bindings link the system's WebKitGTK 4.1, libsoup 3, GTK 3 and cairo dynamically (each
       under the LGPL, in its own version; cairo LGPL-2.1 OR MPL-1.1); they are not distributed with PowerRustCOBOL, the
       .deb/.rpm packages name them as dependencies.

Component: option-ext 0.2
Homepage: https://github.com/soc/option-ext
License: MPL-2.0
Used by: dirs-sys, through wry's dirs 7 (cobolt-web-preview)
Notes: Unmodified. MPL-2.0 is a file-level copyleft: its Source Code Form is available from
       https://crates.io/crates/option-ext (version 0.2.0).
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
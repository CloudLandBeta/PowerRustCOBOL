<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Building PowerRustCOBOL

From a clean machine to a running IDE, on **Windows**, **Linux** and **macOS**.

Everything here is the same three steps on every platform — install a toolchain,
clone, `cargo build`. Only the first step differs per OS.

---

## What the build needs

| Requirement | Why |
|---|---|
| **Rust**, stable channel, **1.92 or newer** | builds the whole workspace |
| **Git** | clones the repository |
| **A C compiler and a linker** | the linker Rust needs for *any* binary, plus two C dependencies |
| **Native GUI libraries** (Linux only) | window creation and the native file dialogs |

> **The packaged IDE checks the Rust requirement itself.** Somebody who *uses*
> PowerRustCOBOL rather than building it never reads this page, so the IDE looks
> for Rust on its first run and offers to install it when this same **1.92**
> minimum is not met. It reads the number from this workspace's own manifest, so
> the two cannot disagree. See §3 of the Developer's Guide.

### About the C compiler

Two crates in the tree compile C source, so a C compiler is genuinely required:

- **`libsqlite3-sys`** — SQLite, bundled from its C amalgamation. This is the
  COBOL database runtime's SQLite support, so no system SQLite has to be
  installed or version-matched on the end user's machine.
- **`onig_sys`** — the Oniguruma regex engine, which the tokenizer behind
  semantic search uses.

What the build does **not** need, and never invokes:

> **no C++ compiler · no CMake · no NASM · no Python · no Node · no JVM**

That is deliberate and is kept that way. TLS goes through the operating system's
own stack (schannel on Windows, Security.framework on macOS, OpenSSL on Linux)
via pure-Rust bindings rather than a bundled crypto library that would need C,
assembly and CMake on every machine; the tokenizer's C++ suffix-array
(`esaxx_fast`) is switched off because nothing here trains a model; and the
Knowledge Base index is `redb`, pure Rust.

On every platform the C compiler arrives inside the same package that provides
the linker Rust already requires, so in practice this adds nothing to install.

---

## 1. Install the toolchain

### Windows

1. Install the **Visual Studio Build Tools** with the **"Desktop development with
   C++"** workload —
   [download](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   The workload is named for C++, but what it delivers is what every Rust build
   on Windows needs anyway: `link.exe`, the Windows SDK, and `cl.exe` for the two
   C dependencies above. There is nothing else to download.

2. Install Rust from [rustup.rs](https://rustup.rs). It selects the MSVC
   toolchain automatically.

3. Verify, from a normal PowerShell prompt:

   ```powershell
   rustc --version
   cargo --version
   ```

No linker flags to set by hand: the repository's `.cargo/config.toml` already
puts every object on the dynamic CRT, which is what keeps the C dependencies and
Rust's own runtime from colliding at link time.

### macOS

Install the Xcode Command Line Tools — that is the whole of it:

```sh
xcode-select --install
```

Then Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon and Intel are both supported; rustup picks the right host target.

### Linux

**Debian / Ubuntu:**

```sh
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libssl-dev
```

**Fedora / RHEL:**

```sh
sudo dnf install -y @development-tools pkgconf-pkg-config \
    gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel
```

**Arch:**

```sh
sudo pacman -S --needed base-devel pkgconf gtk3 libxcb libxkbcommon openssl
```

Then Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Two of those packages are load-bearing and worth naming:

- **`libssl-dev` / `openssl-devel`** — HTTPS uses the system's TLS on Linux, and
  this is it.
- **`libgtk-3-dev` / `gtk3-devel`** — the native Open/Save dialogs.

X11 and Wayland are both supported; the window layer picks whichever session is
running, so neither is a separate install.

---

## 2. Get the code

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Build

```sh
cargo build
```

> The first build fetches every crate and compiles the workspace, so expect a few
> minutes and a `target/` cache around 1.5 GB. Later builds are incremental.
> `cargo clean` reclaims the space whenever you want it back.

To build just the two things you run:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Launch the IDE

```sh
cargo run -p cobolt-ide
```

For day-to-day use prefer a release build — slower to compile once, far smoother
to use:

```sh
cargo run --release -p cobolt-ide
```

---

## Running the tests

```sh
cargo test --workspace
```

The forms engine needs its `render` feature to test the rendering paths:

```sh
cargo test -p cobolt-forms --features render
```

---

## Where the artifacts land

| Artifact | Path |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` on Windows) |
| CLI runtime / builder | `target/release/rcrun` (`.exe` on Windows) |
| An application **you** build from a project | `<project>/bin/` and the project's destination folder |

An application built with `rcrun build` is a single self-contained executable: it
embeds its compiled program, its forms and any asset-pack theme they use, so
there is nothing to install beside it on the machine you hand it to.

---

## Installing the IDE elsewhere — ship the platform SDK

The IDE executable is **not** self-contained the way an application you build is.
Building an application runs a real `cargo build` against the platform's Rust
sources, so those sources must exist on the machine doing the building. Copy
`cobolt-ide` somewhere on its own and Build fails, naming every folder it looked
in — the toolchain is fine, the sources are simply absent.

Stage them beside the executable. From the source tree:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

That writes `Cargo.toml` and `crates/` into `<install-dir>` — 6.0 MB, the ten
crates a built application compiles against. Pass `--sdk` to put them in
`<install-dir>/sdk/` instead when the install folder holds other things. The IDE
finds either layout with no configuration, and also looks one level up and, on
macOS, inside the bundle's `Resources`.

The machine still needs the Rust toolchain — Build is a real compile — and its
first build downloads the dependency crates from the registry, so it needs
network access once.

> **Note.** For a checkout that lives somewhere else entirely, set the folder by
> hand under **Help → Platform SDK Location**. It is remembered per machine
> rather than per project, so it never travels to a colleague in `cobolt.toml`.
> Leave it blank to go back to the automatic search.

---

## Troubleshooting

**`linker 'cc' not found` (Linux)** — `build-essential` (or `@development-tools`)
is missing.

**`link.exe not found` (Windows)** — the Build Tools installed without the
"Desktop development with C++" workload. Re-run the installer and tick it.

**`Could not find directory of OpenSSL installation` (Linux)** — install
`libssl-dev` / `openssl-devel` and `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**The IDE builds but no window opens (Linux)** — check that `libxkbcommon-dev`
is installed and that `$DISPLAY` or `$WAYLAND_DISPLAY` is set; a bare TTY or an
SSH session without X forwarding has no display to open onto.

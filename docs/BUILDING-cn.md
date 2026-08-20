<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 构建 PowerRustCOBOL

从一台干净的机器到一个跑起来的 IDE，涵盖 **Windows**、**Linux** 和 **macOS**。

这里的全部内容在每个平台上都是同样的三步——安装工具链、克隆、`cargo build`。
只有第一步因操作系统而异。

---

## 构建需要什么

| 需求 | 用途 |
|---|---|
| **Rust**，stable 通道，**1.92 或更新** | 构建整个 workspace |
| **Git** | 克隆仓库 |
| **一个 C 编译器和一个链接器** | Rust 构建*任何*二进制文件都需要的链接器，外加两个 C 依赖 |
| **原生 GUI 库**（仅 Linux） | 创建窗口和原生文件对话框 |

### 关于 C 编译器

代码树中有两个 crate 会编译 C 源码，所以 C 编译器确实是必需的：

- **`libsqlite3-sys`** —— SQLite，由其 C 合并源（amalgamation）内置而来。这就是
  COBOL 数据库运行时的 SQLite 支持，因此终端用户的机器上无需安装系统 SQLite，也
  不必匹配版本。
- **`onig_sys`** —— Oniguruma 正则表达式引擎，语义搜索背后的分词器会用到它。

构建**不**需要、也从不调用的东西：

> **无需 C++ 编译器 · 无需 CMake · 无需 NASM · 无需 Python · 无需 Node · 无需 JVM**

这是刻意为之，并会一直保持。TLS 走操作系统自身的协议栈（Windows 上是 schannel，
macOS 上是 Security.framework，Linux 上是 OpenSSL），通过纯 Rust 绑定接入，而不是
内置一个会在每台机器上都要求 C、汇编和 CMake 的加密库；分词器的 C++ 后缀数组
（`esaxx_fast`）被关闭，因为这里不训练任何模型；而 Knowledge Base 的索引是纯 Rust
的 `redb`。

在每个平台上，C 编译器都随着提供 Rust 本就需要的链接器的那个软件包一起到来，因此
实际上这并未增加任何要安装的东西。

---

## 1. 安装工具链

### Windows

1. 安装 **Visual Studio Build Tools**，并勾选 **“Desktop development with C++”**
   工作负载 ——
   [下载](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   这个工作负载以 C++ 命名，但它交付的正是 Windows 上任何 Rust 构建本就需要的
   东西：`link.exe`、Windows SDK，以及供上述两个 C 依赖使用的 `cl.exe`。此外无需
   下载任何东西。

2. 从 [rustup.rs](https://rustup.rs) 安装 Rust。它会自动选择 MSVC 工具链。

3. 在普通的 PowerShell 提示符下验证：

   ```powershell
   rustc --version
   cargo --version
   ```

无需手工设置链接器标志：仓库的 `.cargo/config.toml` 已经把每个目标文件都放在动态
CRT 之上，正是这一点让 C 依赖与 Rust 自身的运行时在链接时不会相互冲突。

### macOS

安装 Xcode Command Line Tools —— 全部内容就这些：

```sh
xcode-select --install
```

然后安装 Rust：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon 和 Intel 都受支持；rustup 会挑选正确的宿主目标。

### Linux

**Debian / Ubuntu：**

```sh
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libssl-dev
```

**Fedora / RHEL：**

```sh
sudo dnf install -y @development-tools pkgconf-pkg-config \
    gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel
```

**Arch：**

```sh
sudo pacman -S --needed base-devel pkgconf gtk3 libxcb libxkbcommon openssl
```

然后安装 Rust：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

其中有两个软件包是关键性的，值得点名：

- **`libssl-dev` / `openssl-devel`** —— 在 Linux 上 HTTPS 使用系统的 TLS，指的
  就是它。
- **`libgtk-3-dev` / `gtk3-devel`** —— 原生的“打开”与“保存”对话框。

X11 和 Wayland 都受支持；窗口层会自行选择正在运行的会话，因此两者都不需要单独
安装。

---

## 2. 获取代码

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. 构建

```sh
cargo build
```

> 首次构建会拉取每一个 crate 并编译整个 workspace，因此请预留几分钟时间和大约
> 1.5 GB 的 `target/` 缓存。之后的构建是增量式的。任何时候想把空间收回来，运行
> `cargo clean` 即可。

若只想构建你实际会运行的那两样东西：

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. 启动 IDE

```sh
cargo run -p cobolt-ide
```

日常使用请优先选择 release 构建——编译一次会慢一些，但用起来流畅得多：

```sh
cargo run --release -p cobolt-ide
```

---

## 运行测试

```sh
cargo test --workspace
```

forms 引擎需要它的 `render` feature 才能测试渲染路径：

```sh
cargo test -p cobolt-forms --features render
```

---

## 构件落在哪里

| 构件 | 路径 |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows 上为 `.exe`） |
| 命令行运行时 / 构建器 | `target/release/rcrun`（Windows 上为 `.exe`） |
| **你** 从一个 project 构建出的应用程序 | `<project>/bin/` 以及该 project 的目标文件夹 |

用 `rcrun build` 构建出的应用程序是一个自包含的单一可执行文件：它把编译后的程序、
它的 forms 以及它们用到的任何 asset-pack 主题都内嵌其中，因此在你交付到的那台
机器上，无需在它旁边安装任何东西。

---

## 疑难排解

**`linker 'cc' not found`（Linux）** —— 缺少 `build-essential`（或
`@development-tools`）。

**`link.exe not found`（Windows）** —— 安装 Build Tools 时没有勾选 “Desktop
development with C++” 工作负载。重新运行安装程序并勾选它。

**`Could not find directory of OpenSSL installation`（Linux）** —— 请安装
`libssl-dev` / `openssl-devel` 和 `pkg-config`。

**`error: package requires rustc 1.92 or newer`** —— 运行 `rustup update stable`。

**IDE 能构建但不弹出窗口（Linux）** —— 检查是否已安装 `libxkbcommon-dev`，以及
`$DISPLAY` 或 `$WAYLAND_DISPLAY` 是否已设置；纯 TTY 或没有 X 转发的 SSH 会话没有
可供打开的显示。

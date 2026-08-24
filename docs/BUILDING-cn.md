<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 构建 PowerRustCOBOL

从一台干净的机器到跑起来的 IDE，涵盖 **Windows**、**Linux** 和 **macOS**。

这里的全部内容在任何平台上都是同样的三步——装好工具链、克隆、`cargo build`。
只有第一步因操作系统而异。

---

## 构建需要什么

| 要求 | 原因 |
|---|---|
| **Rust**，stable 通道，**1.92 或更新版本** | 构建整个 workspace |
| **Git** | 克隆仓库 |
| **一个 C 编译器和一个链接器** | Rust 构建*任何*二进制文件都需要的链接器，外加两个 C 依赖 |
| **原生 GUI 库**（仅 Linux） | 创建窗口以及原生文件对话框 |

> **打包好的 IDE 会自己检查 Rust 这项要求。** *使用* PowerRustCOBOL 而不是构建
> 它的人从来不会读这一页，因此 IDE 会在首次运行时寻找 Rust，并在不满足同一个
> **1.92** 下限时提出代为安装。版本号读自本 workspace 自己的清单文件，所以两者
> 不可能对不上。参见《开发者指南》第 3 节。

### 关于 C 编译器

代码树中有两个 crate 会编译 C 源码，所以确实需要一个 C 编译器：

- **`libsqlite3-sys`** —— SQLite，由其 C amalgamation 内置而来。这就是 COBOL
  数据库运行时的 SQLite 支持，因此终端用户的机器上不必安装系统 SQLite，也不必
  对版本。
- **`onig_sys`** —— Oniguruma 正则引擎，语义搜索背后的分词器要用它。

构建**不需要**、也从不调用的东西：

> **不需要 C++ 编译器 · 不需要 CMake · 不需要 NASM · 不需要 Python · 不需要
> Node · 不需要 JVM**

这是有意为之，并且一直保持如此。TLS 走操作系统自己的栈（Windows 上是 schannel，
macOS 上是 Security.framework，Linux 上是 OpenSSL），通过纯 Rust 绑定接入，而不是
内置一个会在每台机器上都要求 C、汇编和 CMake 的加密库；分词器的 C++ 后缀数组
（`esaxx_fast`）是关掉的，因为这里不训练任何模型；知识库索引用的是 `redb`，纯
Rust 实现。

在所有平台上，C 编译器都随着提供 Rust 本就需要的链接器的那个包一起到来，所以
实际上这并没有增加任何要安装的东西。

---

## 1. 安装工具链

### Windows

1. 安装 **Visual Studio Build Tools**，并勾选
   **"Desktop development with C++"** 工作负载——
   [下载](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   这个工作负载虽以 C++ 命名，但它带来的正是 Windows 上任何 Rust 构建本来就需要
   的东西：`link.exe`、Windows SDK，以及给上面两个 C 依赖用的 `cl.exe`。除此之外
   没有别的要下载。

2. 从 [rustup.rs](https://rustup.rs) 安装 Rust。它会自动选择 MSVC 工具链。

3. 在普通的 PowerShell 提示符下验证：

   ```powershell
   rustc --version
   cargo --version
   ```

不需要手工设置链接参数：仓库的 `.cargo/config.toml` 已经把所有目标文件放在动态
CRT 上，正是这一点让 C 依赖和 Rust 自己的运行时不会在链接时相撞。

### macOS

安装 Xcode Command Line Tools，就这些：

```sh
xcode-select --install
```

然后是 Rust：

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

然后是 Rust：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

其中两个包起着承重作用，值得点名：

- **`libssl-dev` / `openssl-devel`** —— 在 Linux 上 HTTPS 使用系统的 TLS，指的
  就是它。
- **`libgtk-3-dev` / `gtk3-devel`** —— 原生的打开／保存对话框。

X11 和 Wayland 都受支持；窗口层会自己挑选正在运行的会话，所以两者都不是额外的
安装项。

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

> 首次构建会拉取全部 crate 并编译整个 workspace，因此请预留几分钟时间和大约
> 1.5 GB 的 `target/` 缓存。之后的构建是增量的。想把空间收回来时，`cargo clean`
> 随时可以。

只构建你真正要运行的那两样东西：

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. 启动 IDE

```sh
cargo run -p cobolt-ide
```

日常使用请优先选择 release 构建——只是第一次编译更慢，用起来则流畅得多：

```sh
cargo run --release -p cobolt-ide
```

---

## 运行测试

```sh
cargo test --workspace
```

表单引擎需要它的 `render` feature 才能测试渲染路径：

```sh
cargo test -p cobolt-forms --features render
```

---

## 产物落在哪里

| 产物 | 路径 |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows 上为 `.exe`） |
| CLI 运行时／构建器 | `target/release/rcrun`（Windows 上为 `.exe`） |
| **你**从一个项目构建出的应用程序 | `<project>/bin/` 以及该项目的目标文件夹 |

用 `rcrun build` 构建出的应用程序是一个自包含的单一可执行文件：它内嵌了编译好的
程序、它的表单，以及它们用到的任何 asset pack 主题，因此在你交付它的那台机器上
无需再装别的东西。

---

## 疑难排解

**`linker 'cc' not found`（Linux）** —— 缺少 `build-essential`（或
`@development-tools`）。

**`link.exe not found`（Windows）** —— 安装 Build Tools 时没有勾选 "Desktop
development with C++" 工作负载。重新运行安装程序并勾上它。

**`Could not find directory of OpenSSL installation`（Linux）** —— 安装
`libssl-dev` / `openssl-devel` 和 `pkg-config`。

**`error: package requires rustc 1.92 or newer`** —— `rustup update stable`。

**IDE 能构建但不弹出窗口（Linux）** —— 检查 `libxkbcommon-dev` 是否已安装，以及
`$DISPLAY` 或 `$WAYLAND_DISPLAY` 是否已设置；裸 TTY 或没有 X 转发的 SSH 会话没有
可供打开的显示。

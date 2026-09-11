<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# 构建 PowerRustCOBOL

从一台干净的机器到一个跑起来的 IDE，适用于 **Windows**、**Linux** 和 **macOS**。

这里的一切在每个平台上都是同样的三步——装工具链、克隆、`cargo build`。只有第一步
因操作系统而异。

---

## 构建需要什么

| 要求 | 为什么 |
|---|---|
| **Rust** 稳定版通道，**1.92 或更新** | 构建整个工作空间 |
| **Git** | 克隆仓库 |
| **一个 C 编译器和一个链接器** | Rust 构建*任何*二进制文件都需要的链接器，外加两个 C 依赖 |
| **原生 GUI 库**（仅 Linux） | 创建窗口和原生文件对话框 |

> **打包后的 IDE 会自己检查 Rust 这项要求。** 一个*使用* PowerRustCOBOL 而不是构建
> 它的人永远不会读这一页，所以 IDE 会在首次运行时寻找 Rust，并在不满足同样的
> **1.92** 下限时提出代为安装。它从本工作空间自己的清单里读取这个数字，因此两者不
> 可能不一致。参见《开发者指南》§3。

### 关于 C 编译器

树中有两个 crate 会编译 C 源码，所以 C 编译器确实是必需的：

- **`libsqlite3-sys`** —— SQLite，由其 C 合并源码内置。这是 COBOL 数据库运行时的
  SQLite 支持，因此最终用户的机器上不必安装系统 SQLite，也不必对版本。
- **`onig_sys`** —— Oniguruma 正则引擎，语义搜索背后的分词器会用到它。

构建**不**需要、也从不调用的东西：

> **不需要 C++ 编译器 · 不需要 CMake · 不需要 NASM · 不需要 Python · 不需要 Node ·
> 不需要 JVM**

这是有意为之，并会一直保持。TLS 通过操作系统自身的栈（Windows 上是 schannel，
macOS 上是 Security.framework，Linux 上是 OpenSSL）以纯 Rust 绑定接入，而不是内置
一个会让每台机器都需要 C、汇编和 CMake 的加密库；分词器的 C++ 后缀数组
（`esaxx_fast`）被关掉了，因为这里不训练任何模型；知识库索引用的是纯 Rust 的
`redb`。

在每个平台上，C 编译器都随着提供 Rust 本来就需要的链接器的那同一个包一起到来，所以
实际上这不增加任何安装项。

---

## 1. 安装工具链

### Windows

1. 安装 **Visual Studio Build Tools**，勾选 **"Desktop development with C++"**
   工作负载 ——
   [下载](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   这个工作负载以 C++ 命名，但它带来的正是 Windows 上任何 Rust 构建本来就需要的
   东西：`link.exe`、Windows SDK，以及供上面那两个 C 依赖使用的 `cl.exe`。没有别的
   要下载。

2. 从 [rustup.rs](https://rustup.rs) 安装 Rust。它会自动选择 MSVC 工具链。

3. 在普通的 PowerShell 提示符下验证：

   ```powershell
   rustc --version
   cargo --version
   ```

没有需要手工设置的链接器参数：仓库的 `.cargo/config.toml` 已经把每个目标文件放在
动态 CRT 上，正是这一点让 C 依赖和 Rust 自身的运行时在链接时不会撞车。

### macOS

安装 Xcode Command Line Tools ——就这么多：

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

其中两个包是关键，值得点名：

- **`libssl-dev` / `openssl-devel`** —— 在 Linux 上 HTTPS 使用系统的 TLS，指的就是
  它。
- **`libgtk-3-dev` / `gtk3-devel`** —— 原生的"打开/保存"对话框。

X11 和 Wayland 都受支持；窗口层会选择正在运行的那个会话，所以两者都不需要单独安装。

---

## 2. 取得代码

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. 构建

```sh
cargo build
```

> 第一次构建会拉取所有 crate 并编译整个工作空间，所以要预留几分钟和约 1.5 GB 的
> `target/` 缓存。之后的构建是增量的。想要回收这些空间时，`cargo clean` 随时可用。

只构建你真正会运行的那两样：

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. 启动 IDE

```sh
cargo run -p cobolt-ide
```

日常使用请优先用 release 构建——编译一次比较慢，用起来顺畅得多：

```sh
cargo run --release -p cobolt-ide
```

---

## 运行测试

```sh
cargo test --workspace
```

表单引擎需要它的 `render` 特性才能测试渲染路径：

```sh
cargo test -p cobolt-forms --features render
```

---

## 产物落在哪里

| 产物 | 路径 |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows 上为 `.exe`） |
| CLI 运行时 / 构建器 | `target/release/rcrun`（Windows 上为 `.exe`） |
| **你**从一个项目构建出的应用程序 | `<project>/bin/` 以及项目的目标文件夹 |

用 `rcrun build` 构建出的应用程序是一个自包含的单一可执行文件：它内嵌了编译后的
程序、它的表单，以及它们用到的任何资源包主题，所以在你交付的那台机器上，旁边不需要
再安装任何东西。

---

## 把 IDE 装到别处——请一并带上平台 SDK

IDE 的可执行文件**不像**你构建出的应用程序那样自包含。构建一个应用程序会针对平台的
Rust 源码运行一次真正的 `cargo build`，因此那些源码必须存在于执行构建的那台机器上。
把 `cobolt-ide` 单独复制到别处，Build 就会失败，并列出它查找过的每一个文件夹——
工具链没问题，只是源码不在。

把它们放到可执行文件旁边。从源码树里执行：

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

这会把 `Cargo.toml`、`Cargo.lock` 和 `crates/` 写入 `<install-dir>`，连同一个表单
应用程序所需的资源——主题树和窗口图标。一个构建出的应用程序所要编译的那十个 crate
是 **8.6 MiB**；加上 `assets/themes`，放置后的目录树约为 **21 MiB**。图标不是可选
的：省掉它，任何表单应用程序都根本编译不了。当安装文件夹里还有别的东西时，传入
`--sdk` 可以把它们放进 `<install-dir>/sdk/`。两种布局 IDE 都能在零配置的情况下找
到，它还会往上一级查找，在 macOS 上也会查找程序包的 `Resources`。

那台机器仍然需要 Rust 工具链——Build 是一次真正的编译——而且它的第一次构建会从
注册表下载依赖 crate，所以需要一次网络访问。

> **注意。** 如果检出目录完全在别的地方，请在 **Help → Platform SDK Location** 里
> 手动指定该文件夹。它按机器记忆而不是按项目记忆，因此绝不会随 `cobolt.toml` 传到
> 同事那里。留空即可回到自动查找。

---

## 疑难解答

**`linker 'cc' not found`（Linux）** —— 缺少 `build-essential`（或
`@development-tools`）。

**`link.exe not found`（Windows）** —— Build Tools 安装时没有勾选 "Desktop
development with C++" 工作负载。重新运行安装程序并勾选它。

**`Could not find directory of OpenSSL installation`（Linux）** —— 安装
`libssl-dev` / `openssl-devel` 和 `pkg-config`。

**`error: package requires rustc 1.92 or newer`** —— `rustup update stable`。

**IDE 能构建但不弹出窗口（Linux）** —— 检查是否安装了 `libxkbcommon-dev`，以及
`$DISPLAY` 或 `$WAYLAND_DISPLAY` 是否已设置；一个纯 TTY 或没有 X 转发的 SSH 会话
没有可供打开的显示。

.<<

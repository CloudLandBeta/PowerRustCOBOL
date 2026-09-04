<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 构建 PowerRustCOBOL

从一台干净的机器到一个跑起来的 IDE，在 **Windows**、**Linux** 和 **macOS** 上都
一样。

这里的一切在每个平台上都是同样的三步——装工具链、克隆、`cargo build`。只有第一
步因操作系统而异。

---

## 构建需要什么

| 要求 | 为什么 |
|---|---|
| **Rust**，stable 通道，**1.92 或更新** | 构建整个 workspace |
| **Git** | 克隆仓库 |
| **一个 C 编译器和一个链接器** | Rust 构建*任何*二进制文件都要的链接器，外加两个 C 依赖 |
| **原生 GUI 库**（仅 Linux） | 创建窗口和原生文件对话框 |

> **打包好的 IDE 会自己检查 Rust 这项要求。** *使用* PowerRustCOBOL 而不是构建
> 它的人永远不会读这一页，所以 IDE 会在首次启动时寻找 Rust，并在没有满足同样这个
> **1.92** 下限时提出替你安装。它从本 workspace 自己的清单里读取这个数字，因此
> 两者不可能对不上。参见《开发者指南》第 3 节。

### 关于 C 编译器

代码树里有两个 crate 会编译 C 源码，所以 C 编译器是实打实需要的：

- **`libsqlite3-sys`** —— SQLite，由它的 C 合并版内置打包。这是 COBOL 数据库运行
  时的 SQLite 支持，因此终端用户的机器上不必安装系统 SQLite，也不必对版本。
- **`onig_sys`** —— Oniguruma 正则引擎，语义搜索背后的分词器要用它。

构建**不**需要、也从不调用的东西：

> **不要 C++ 编译器 · 不要 CMake · 不要 NASM · 不要 Python · 不要 Node · 不要 JVM**

这是有意为之，并且会一直保持下去。TLS 走操作系统自带的那一套（Windows 上是
schannel，macOS 上是 Security.framework，Linux 上是 OpenSSL），经由纯 Rust 的绑定
接入，而不是内置一个加密库——那样每台机器都得备好 C、汇编和 CMake；分词器的 C++
后缀数组（`esaxx_fast`）被关掉了，因为这里没有任何东西要训练模型；知识库索引用的
是 `redb`，纯 Rust。

在每个平台上，C 编译器都随着提供 Rust 本就需要的链接器的那同一个包一起到来，所以
实际上这一项并没有增加任何要安装的东西。

---

## 1. 安装工具链

### Windows

1. 安装 **Visual Studio Build Tools**，并勾选 **"Desktop development with C++"**
   工作负载——[下载](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   这个工作负载以 C++ 命名，但它交付的正是 Windows 上任何一次 Rust 构建本来就
   需要的东西：`link.exe`、Windows SDK，以及给上面两个 C 依赖用的 `cl.exe`。
   没有别的要下载了。

2. 从 [rustup.rs](https://rustup.rs) 安装 Rust。它会自动选择 MSVC 工具链。

3. 在普通的 PowerShell 提示符下验证：

   ```powershell
   rustc --version
   cargo --version
   ```

没有需要手工设置的链接器标志：仓库里的 `.cargo/config.toml` 已经把每个目标文件都
放到动态 CRT 上，正是这一点让 C 依赖和 Rust 自己的运行时在链接时不会撞车。

### macOS

安装 Xcode Command Line Tools —— 就这些：

```sh
xcode-select --install
```

然后是 Rust：

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon 和 Intel 都受支持；rustup 会挑对宿主目标。

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

其中两个包是承重的，值得点名：

- **`libssl-dev` / `openssl-devel`** —— Linux 上 HTTPS 用的是系统的 TLS，就是它。
- **`libgtk-3-dev` / `gtk3-devel`** —— 原生的打开/保存对话框。

X11 和 Wayland 都受支持；窗口层会挑正在运行的那个会话，所以两者都不需要单独安装。

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

> 首次构建会拉取每一个 crate 并编译整个 workspace，所以预计要几分钟，`target/`
> 缓存约 1.5 GB。之后的构建是增量的。想把空间收回来时，`cargo clean` 随时可以。

只构建你要运行的那两样东西：

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. 启动 IDE

```sh
cargo run -p cobolt-ide
```

日常使用请优先选 release 构建——编译一次更慢，用起来顺畅得多：

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
| IDE | `target/release/cobolt-ide`（Windows 上带 `.exe`） |
| 命令行运行时 / 构建器 | `target/release/rcrun`（Windows 上带 `.exe`） |
| **你**从一个项目构建出来的应用程序 | `<project>/bin/` 以及项目的目标文件夹 |

用 `rcrun build` 构建出的应用程序是一个自包含的单一可执行文件：它内嵌了编译好的
程序、它的表单，以及它们用到的任何资源包主题，因此在你交付给对方的机器上，旁边
不需要再装任何东西。

---

## 把 IDE 装到别处 —— 一并带上平台 SDK

IDE 的可执行文件**不像**你构建出的应用程序那样自包含。构建一个应用程序会针对
平台的 Rust 源码跑一次真正的 `cargo build`，所以做构建的那台机器上必须有这些
源码。把 `cobolt-ide` 单独拷到某处，Build 就会失败，并列出它找过的每一个文件夹
——工具链没问题，只是源码不在。

把它们放到可执行文件旁边。在源码树里：

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

这会把 `Cargo.toml` 和 `crates/` 写进 `<install-dir>` —— 6.0 MB，即一个构建出的
应用程序所要编译依赖的那十个 crate。当安装文件夹里还放着别的东西时，传 `--sdk`
可以把它们改放到 `<install-dir>/sdk/`。这两种布局 IDE 都能零配置找到，而且它还
会往上找一层，在 macOS 上还会找 bundle 的 `Resources` 里面。

那台机器仍然需要 Rust 工具链——Build 是一次真正的编译——而且它的首次构建会从
registry 下载依赖 crate，所以需要联网一次。

> **注意。** 如果检出目录完全在别的地方，就在 **Help → Platform SDK Location**
> 里手工指定这个文件夹。它是按机器而不是按项目记住的，所以绝不会随 `cobolt.toml`
> 跑到同事那里去。留空即可回到自动查找。

---

## 疑难排解

**`linker 'cc' not found`（Linux）** —— 缺 `build-essential`（或
`@development-tools`）。

**`link.exe not found`（Windows）** —— Build Tools 安装时没带 "Desktop
development with C++" 工作负载。重新运行安装程序并勾上它。

**`Could not find directory of OpenSSL installation`（Linux）** —— 安装
`libssl-dev` / `openssl-devel` 和 `pkg-config`。

**`error: package requires rustc 1.92 or newer`** —— `rustup update stable`。

**IDE 构建成功但没有窗口打开（Linux）** —— 检查 `libxkbcommon-dev` 是否已安装，
以及 `$DISPLAY` 或 `$WAYLAND_DISPLAY` 是否已设置；一个裸 TTY 或没有 X 转发的 SSH
会话没有可供打开的显示。

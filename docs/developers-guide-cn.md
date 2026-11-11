<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 2.0.425 -->

# PowerRustCOBOL AI 开发者指南 2.0

<p align="center">
  <img src="../assets/images/powerrustcobol-mascot.png" alt="PowerRustCOBOL 吉祥物" width="300">
</p>


*用 PowerRustCOBOL 构建图形化 COBOL 应用程序的实用指南。*

> **本指南面向谁。** 你已经在写 COBOL，并且用图形化的 COBOL 工具集构建过基于屏幕
> 或窗口的应用程序——例如富士通的 **PowerCOBOL for Windows** 或 **Veryant isCOBOL**。
> 你熟悉 `IDENTIFICATION DIVISION`、`PERFORM`、`OPEN`/`READ`/`WRITE`、索引文件，
> 以及带有触发*事件*的*控件*的*窗体*这一概念。本指南把这些直觉迁移到
> PowerRustCOBOL 上，并向你展示所有新增的东西。**不预设也不要求任何宿主实现语言的
> 先备知识**——构建一个应用程序，你永远不需要读写 COBOL 以外的任何东西。

---

## 目录

1. [PowerRustCOBOL 是什么，以及它为何存在](#1-powerrustcobol-是什么以及它为何存在)
2. [三个部件：RustCOBOL、PowerRustCOBOL、rcrun](#2-三个部件)
3. [安装与启动](#3-安装与启动)
4. [你的第一个应用程序：Hello, Form](#4-你的第一个应用程序hello-form)
5. [IDE 一览](#5-ide-一览)
   - [Settings 菜单](#settings-菜单)
   - [窗口效果](#窗口效果)
6. [项目与项目模型](#6-项目与项目模型)
7. [Form Designer（RAD）](#7-form-designerrad)
8. [控件目录](#8-控件目录)
9. [属性](#9-属性)
    - [倾斜显示控件：3D 透视](#倾斜显示控件3d-透视)
10. [事件驱动的编程](#10-事件驱动的编程)
11. [从 COBOL 与界面对话](#11-从-cobol-与界面对话)
12. [生成的代码](#12-生成的代码)
13. [RustCOBOL 语言](#13-rustcobol-语言)
    - [代码风格](#代码风格)
    - [数据项的值方法](#数据项的值方法)
    - [按标准允许的写法来写](#按标准允许的写法来写)
    - [国际文本：national（`PIC N`）与 UTF-8（`PIC U`）数据](#国际文本nationalpic-n与-utf-8pic-u数据)
    - [把一整张表交给一个函数](#把一整张表交给一个函数)
    - [把一个文件彻底关掉：`WITH LOCK`](#把一个文件彻底关掉with-lock)
    - [调试行](#调试行)
    - [又长又别扭的文本：块字面量](#又长又别扭的文本-块字面量)
    - [不用 `FD` 写一个文本文件](#不用-fd-写一个文本文件)
    - [图片：适配与测量一张图像](#图片适配与测量一张图像)
    - [通过 HTTP 下载文件](#通过-http-下载文件)
14. [索引文件——一等公民级的资源](#14-索引文件一等公民级的资源)
15. [SQL 数据库](#15-sql-数据库)
    - [嵌入式 SQL：`EXEC SQL`](#嵌入式-sqlexec-sql)
    - [`COBOL::SqlDatabase`：不用 `EXEC SQL` 的 SQL](#cobolsqldatabase不用-exec-sql-的-sql)
16. [HTTP / REST 与 AI 智能体](#16-http--rest-与-ai-智能体)
    - [与编码智能体（Claude Code）协作](#与编码智能体claude-code协作)
17. [命令行（rcrun）](#17-命令行rcrun)
18. [构建一个可分发的二进制](#18-构建一个可分发的二进制)
    - [构建结果保存在哪里（Settings ▸ Build Folder）](#构建结果保存在哪里settings--build-folder)
19. [调试](#19-调试)
    - [诊断开关（Settings ▸ Debug Settings）](#诊断开关settings--debug-settings)
20. [外观与国际化](#20-外观与国际化)
21. [COBOL Structure 与共享数据](#21-cobol-structure-与共享数据)
22. [应用外壳与 `super` 接收者](#22-应用外壳与-super-接收者)
23. [Web 与移动应用程序](#23-web-与移动应用程序)
24. [Backend 与安全通道](#24-backend-与安全通道)
25. [移动应用程序：Android 与 iOS](#25-移动应用程序android-与-ios)
26. [图形：Canvas 2D、Canvas 3D 与图形对象](#26-图形canvas-2dcanvas-3d-与图形对象)
27. [注意事项与当前限制](#27-注意事项与当前限制)
28. [附录 A —— 从 PowerCOBOL / isCOBOL 过来](#附录-a--从-powercobol--iscobol-过来)
29. [附录 B —— 术语表](#附录-b--术语表)

---

## 1. PowerRustCOBOL 是什么，以及它为何存在

<!-- 📷 welcome.png — the welcome screen as it appears on first launch, before any project is open. -->

<p align="center"><img src="../assets/images/screenshots/welcome.png" alt="PowerRustCOBOL AI 的欢迎界面" width="900"></p>


几十年来，编写**带窗口、由事件驱动的 COBOL** 的唯一途径，就是购买一套绑定于某一个
操作系统、某一家厂商、某一种授权模式的专有工具链。那些工具在当年都很出色，但如今
大多被困在 Windows 上，封闭，而且越来越难部署到现代机器上。整整一代业务逻辑——薪资、
库存、银行后台——都是以这种风格写成的，而它们无处可去。

**PowerRustCOBOL 的存在，就是为了给这种开发风格一个崭新而开放的归宿。** 它是一个
快速应用开发（RAD）环境，你可以在其中：

- 把控件拖到画布上来设计窗口（“窗体”），
- 为这些控件挂上用 **COBOL** 写的事件处理程序，
- 然后运行、调试，并把结果作为**单个自包含的原生可执行文件**交付——目标机器上无需
  安装任何运行时，
- 或者，从版本 2 起，把它构建为 **Web 或移动应用程序**：你的 COBOL 被编译成
  WebAssembly，运行在浏览器和手机上，而必须留在服务器上的部分则由一个 COBOL
  **Backend** 承担（第 23 至 26 章）。

用平实的话说，它的设计目标是：


| 目标 | 对你意味着什么 |
| --- | --- |
| **COBOL 优先** | 应用程序*就是* COBOL。设计器生成 COBOL；你的事件处理程序是 COBOL-85 嵌套程序。你从不离开这门语言。 |
| **跨平台** | IDE 和生成的二进制文件都不绑定于单一操作系统。 |
| **自包含** | 构建出的应用程序内嵌了它所需的一切；最终用户不必安装 PowerRustCOBOL。 |
| **现代数据访问** | 抗崩溃的索引（ISAM）文件、SQL（SQLite / PostgreSQL / MySQL）以及 HTTP/REST，都通过普通的 `CALL` 语句触达。 |
| **开放** | 采用 Apache-2.0 许可。 |

> **注意。** PowerRustCOBOL *借鉴*了经典图形化 COBOL RAD 的生产力，但它是一个独立
> 的原创实现。“窗体”“控件”“事件”这类概念属于行业通用；而这里描述的语法、文件格式、
> 生成代码和内置服务都是 PowerRustCOBOL 特有的，与任何其他厂商的工具都不兼容。

---

## 2. 三个部件

PowerRustCOBOL 以三个相互协作的工具交付。尽早弄清哪个是哪个，能免去很多困惑。

```mermaid
flowchart LR
    subgraph Author["你在这里创作"]
        IDE["PowerRustCOBOL<br/>(RAD IDE)"]
    end
    subgraph Lang["语言"]
        LANG["RustCOBOL<br/>(COBOL-85 + 扩展)"]
    end
    subgraph Run["你在这里运行 / 交付"]
        CLI["rcrun<br/>(CLI: run · check · build · package)"]
        BIN["原生二进制<br/>(你交付的应用)"]
    end

    IDE -- "设计窗体，编写 COBOL" --> LANG
    IDE -- "Run / Debug" --> CLI
    IDE -- "Build" --> BIN
    LANG -- "rcrun run/check" --> CLI
    LANG -- "rcrun build" --> BIN
```


| 名称 | 角色 | 可以把它看作…… |
| --- | --- | --- |
| **RustCOBOL** | COBOL-85 语言方言，加上 PowerRustCOBOL 的扩展（GUI 调用、索引文件子句、SQL/HTTP）。 | 编译器/运行时的“语言”。 |
| **PowerRustCOBOL** | 桌面 IDE：项目资源管理器、代码编辑器、**Form Designer**、调试器。 | “工作台”/“Studio”。 |
| **rcrun** | 命令行运行时、检查器、打包器和二进制编译器。 | 可以在 CI 中编写脚本调用的“运行时 + 构建工具”。 |


> ⚠️ **命名说明。** 在内部，一些构建产物和文件夹以 `cobolt-*` 命名。那是实现细节；
> 面向用户的名称是 **RustCOBOL**、**PowerRustCOBOL** 和 **rcrun**。

---

## 3. 安装与启动

每个版本为每个平台提供**两种下载**，任何一种都是完整的——它们携带相同的应用程序、
相同的 `rcrun`、相同的主题、示例和平台 SDK。

| 你的机器 | 安装程序 | 压缩包 |
| --- | --- | --- |
| Windows 10 / 11，64 位 | `.msi` —— 双击，或用 `msiexec /i … /quiet` 静默部署 | `.zip` |
| Apple Silicon 的 Mac | `.dmg` —— 把 PowerRustCOBOL 拖到 Applications | `.tar.gz` |
| Intel 的 Mac | `.dmg` | `.tar.gz` |
| Debian、Ubuntu、Mint 及其衍生版 | `.deb` —— `sudo apt install ./PowerRustCOBOL-*.deb` | `.tar.gz` |
| Fedora、RHEL、CentOS Stream、openSUSE | `.rpm` —— `sudo dnf install ./PowerRustCOBOL-*.rpm` | `.tar.gz` |
| 任何其他 Linux，64 位 | — | `.tar.gz` |

如果你想要常见的那些东西——“开始”菜单或 Applications 中的条目、桌面启动器、位于
`PATH` 上的 `rcrun`，以及日后干净卸载的办法——就选**安装程序**。如果你宁愿什么都
不安装——解压到任何地方就能运行，包括在 U 盘上，或在无法安装软件的机器上——就选
**压缩包**。两种 Linux 软件包都把应用程序放在 `/opt/powerrustcobol`，并把
`powerrustcobol` 和 `rcrun` 链接到 `/usr/bin`；在任何其他发行版上，下载压缩包即可。

> ⚠️ **两者目前都未签名**，所以每个平台在首次运行时都会警告一次。
> 在 **macOS** 上：右键点击应用并选择 *Open*，或用
> `xattr -dr com.apple.quarantine PowerRustCOBOL.app` 清除隔离标志。在 **Windows**
> 上：SmartScreen 会提供 *More info* → *Run anyway*。在这里，安装程序并不比压缩包
> 更受信任——警告针对的是缺失的证书，而不是文件格式。

Linux 需要 glibc 2.35 或更新版本（Ubuntu 22.04+、Debian 12+、Fedora 36+），以及
你的桌面本来就提供的 OpenGL、X11 或 Wayland 库。

> **注意 —— Linux 上的声音。** IDE 和 `rcrun` 带有声音内置函数
> （§11，*播放声音*），它们通过 **ALSA** 访问声卡，因此在 Linux 上需要 ALSA 库
> `libasound2`（Ubuntu 24.04 和 Debian 13 上是 `libasound2t64`；Fedora 上是
> `alsa-lib`）。每个桌面环境都会随其声音系统一并装好；在最小化安装或服务器安装上，
> `.deb` 和 `.rpm` 会替你获取它，使用压缩包时则要自己安装
> （`sudo apt install libasound2`）。在 Linux 上**构建**一个会播放声音的应用程序，
> 还需要 ALSA 开发包——`libasound2-dev`，Fedora 上是 `alsa-lib-devel`；从不调用
> 任何 `AUDIO-…` 内置函数的应用程序，构建时不带声音，两者都不需要。macOS 和
> Windows 什么都不需要。

启动 IDE；首次运行时，你会看到一个空的工作区和提示
*"Open a COBOL file to get started."*。你可以打开单个 `.cbl` 文件，也可以创建一个
完整的**项目**（推荐——见 §6）。

<p align="center"><img src="../assets/images/screenshots/theide.png" alt="打开了一个项目的 PowerRustCOBOL AI IDE" width="900"></p>


你也可以在终端里用 `rcrun` 以无界面方式驱动一切（见 §17），持续集成流水线用的
就是这种方式。

### 首次运行的 Rust 检查



IDE 自己就能设计窗体并*运行*程序。**Build** 是个例外：它通过 **Rust 工具链**
（§18）把你的项目编译成原生应用程序，运行任何含有 `EXEC RUST` 块的程序也是如此。
所以首次运行时，PowerRustCOBOL 会去找 Rust——找到可用的版本时，它什么都不说。

找不到时，它会告诉你属于哪种情况——没有 Rust，或者版本低于 PowerRustCOBOL 所需的
**1.92**——显示官方的 [rustup.rs](https://rustup.rs) 命令，并提议替你运行。如果你
拒绝，它还会再问一次，因为拒绝是有代价的，值得说明：


| 没有 Rust 你会失去 | 你仍然保留 |
| --- | --- |
| **Build** —— 没有原生可执行文件，没有东西可打包 | Form Designer |
| 运行任何含有 `EXEC RUST` 块的程序 | 代码编辑器和 COBOL 工具 |
| | **Run**（解释执行）和调试器 |

第二次拒绝就此定案，不会再问。日后从 [rustup.rs](https://rustup.rs) 安装 Rust，
**Build** 就会直接开始工作——IDE 里不需要告知任何东西。

> **注意** —— rustup 把 Rust 装在 `~/.cargo/bin`，由你的 *shell 配置文件*把它加入
> `PATH`。从 Finder 或 Windows 桌面启动的应用程序从不读取那个配置文件，所以
> PowerRustCOBOL 自己会去那个位置查找，并使用在那里找到的东西。你不必从终端启动
> IDE，**Build** 也能工作。

#### Rust 已安装，但 Build 仍无法完成

还有第二个前置条件，rustup 既不安装它，也不提它：**链接器**。编译产生机器码；
链接器负责把这些代码收拢成一个可执行文件，而它属于操作系统，而不是 Rust。

| 平台 | 由什么提供链接器 |
| --- | --- |
| **Windows** | Microsoft C++ 生成工具——*Build Tools for Visual Studio*（或 Visual Studio），并带有 **Desktop development with C++** 工作负载。Visual Studio Code 是另一个产品，并不提供它们。 |
| **macOS** | Apple 的命令行开发者工具——`xcode-select --install` |
| **Linux** | 你所用发行版的 C 工具链——Debian 和 Ubuntu 上是 `build-essential`，Fedora 和 RHEL 上是 *Development Tools* |

首次运行检查也会问这个问题：让 Rust 链接一个什么都不做的程序——这是唯一可靠的
判断办法，因为在 Windows 上，链接器是通过 Visual Studio 的安装找到的，而不是通过
`PATH`。如果做不到，IDE 会在首次运行时说明这一点，指出链接器的名称，并显示安装它
的命令。没有什么需要接受或拒绝的——这不是选择，只是仍然缺少的那一样东西。

如果你是后来才遇到它——在构建结束时，这里原先正是它的出现之处——**Build** 会用
同样的话报告同一件事，而不是给出编译器自己的输出。与此同时，其他一切照常工作：
Form Designer、编辑器、**Run** 和调试器从来都不需要链接器。

### AI 的模型提供商：Ollama

没有**模型提供商**，PowerRustCOBOL AI 只保留它的基本功能——Form Designer、编辑器、
**Run** 和调试器。Grace、各智能体、COBOL 熟练度测试和代码生成，都需要一个语言模型
来运行。**Ollama** 是获得模型最简单的方式：它在你自己的电脑上运行模型，也可以访问
Ollama 云端的更大模型。这只是一个推荐，并非要求——任何其他提供商都可以，可随时在
**Model Providers Manager** 中设置。

所以，在 Rust 之后，最后一个首次运行的问题与 Ollama 有关。当 IDE 在这台机器上
找不到它时，会解释为什么需要提供商，显示推荐的设置和它将要运行的确切命令，并提供
**Install Ollama**——运行 Ollama 自己的安装程序——或 **Not now**。这个问题每台机器
只问一次。

| 平台 | **Install Ollama** 做什么 |
|---|---|
| **Windows** | 从 ollama.com 下载 `OllamaSetup.exe` 并运行 |
| **macOS** | 从 ollama.com 下载 `Ollama.dmg` 并打开——把 Ollama 拖到 Applications，并启动一次 |
| **Linux** | 通过 `pkexec` 运行官方的 `install.sh`，它会在一个窗口中询问管理员密码。没有 `pkexec` 时，对话框会显示要在终端里运行的命令 |

各安装程序也带有同样的步骤：Windows `.msi` 的最后一屏有一个 **Install Ollama**
按钮，macOS 的 `.dmg` 在应用程序旁边放有一个 **Install Ollama** 项，而 `.deb` 和
`.rpm` 在结束时会打印该命令。

**推荐设置：**

| 智能体 | 模型 | 运行位置 |
|---|---|---|
| Grace 和 Proficiency Judge | `gemma4:e2b` | 本地——`ollama pull gemma4:e2b` |
| 其他所有智能体，包括严苛评审员 | `gemma4:31b` | Ollama 云端——需要一个 ollama.com 账户和 API 密钥，在 Model Providers Manager 中作为 **Ollama Cloud** 添加 |

> **注意 —— 任何智能体都可以运行任何模型。** Grace、Judge、各专家和严苛的
> 评审员可以共用一个模型，单一一个模型也可以为它们全部服务。严苛评审员的
> 独立性来自它自己的提示和它自己的单独调用，而不是来自第二个模型，所以它可以与
> 被它评审的智能体运行在同一个模型上；Judge 对一次熟练度测试重新评分，在被它评分
> 的那个模型上同样可以做到。Agents Manager 从不因为另一个智能体已经在运行某个
> 模型，而对它发出警告、加以过滤或拒绝。

---

## 4. 你的第一个应用程序：Hello, Form

这个演练会产生一个带一个按钮的窗口，用来显示一条消息。

1. **创建项目。** `File ▸ New Project…`，给它起个名字（例如 `HelloPower`）和一个
   主程序。IDE 会在磁盘上创建标准的文件夹布局，**以及一个可运行的起始 `main`
   程序**（一段很小的 `DISPLAY`/`GOBACK`，你可以立即 Run），然后在编辑器中打开它
   （见 §6）。
2. **创建窗体。** 在项目树中，点击 **Forms** 旁边的 **➕**。这会打开 *New Form*
   对话框——设置名称（`main-form`）、标题和大小，然后创建。窗体保存在 `forms/` 下，
   并在 **Form Designer** 中打开。（该对话框也可以让你从模板开始；见下文*新建
   窗体的起点*。）
3. **放一个按钮。** 从工具箱把一个 **Button** 拖到画布上。选中它后，在属性窗格中把
   它的 `Caption` 设为 `Say hello`。
4. **放一个标签。** 从工具箱把一个 **Label** 拖到画布上。
5. **挂上处理程序。** 仍然在按钮上，找到它的 **`onClick`** 事件，点击它以打开
   COBOL 事件编辑器。输入，例如：

   ```cobol
              SET Label-1::Caption TO "Hello from COBOL!".
   ```

<!-- 📷 first-form-designer.png — Capture the Form Designer with the single button selected and the `onClick` event highlighted in the properties pane. -->
<p align="center"><img src="../assets/images/screenshots/first-form-designer.png" alt="选中了按钮、其 onClick 事件在属性窗格中高亮显示的 Form Designer" width="900"></p>



6. **运行。** 点击工具栏上的 **Run**（或设计器里的 ▶）。窗体出现；点击按钮就会执行
   你的处理程序。

<!-- 📷 firstform.png — Capture the running form after the button has been clicked, with the greeting showing in the label. -->
<p align="center"><img src="../assets/images/screenshots/firstform.png" alt="点击按钮之后正在运行的窗体，标签中显示着问候语" width="900"></p>


> **注意。** 当你保存或运行一个窗体时，PowerRustCOBOL 会为它**生成**一个 COBOL
> 源文件（见 §12）。你永远不要手工编辑那个文件——它是构建产物。


---

## 5. IDE 一览

```mermaid
flowchart TB
    MB["菜单栏 — File · Settings · (Run) · View · Help"]
    TB["工具栏 — Open · Save · Check · Build · Run · Debug · Stop · ⚙"]
    subgraph Body[" "]
        direction LR
        TREE["Project Explorer<br/>(分类树)"]
        MAIN["主窗格<br/>(代码编辑器 / 属性检查器)"]
    end
    OUT["Output 面板"]
    MB --> TB --> Body --> OUT
```

- **菜单栏（顶部）。** **File · Settings · View · Help** —— 一旦打开了项目或源文件，
  在 Settings 和 View 之间还有 **Run**。**File** 管的是项目（新建、打开、保存、
  打包、构建、退出）；**Settings** 管的是 IDE 和你运行它的这台机器；**Help** 是
  文档、示例、导览和 *About*。
- **Project Explorer（左侧）。** 以你的项目为根的一棵树。八个固定分类——**Forms**、
  **Indexed Files**、**SQL Connections**、**Common Code**、**Generated Code**、
  **Project's Crates (Beta)**、**Assets**、**Knowledge Base**——每个都带有一个
  **➕** 按钮，只有 **Generated Code** 例外，它由 Form Designer 自行填充，你从不
  手工往里添加。每个条目的左边是一个**状态“旋钮”**：🟢 绿色 = 已检查/测试通过，
  🟡 黄色 = 自上次检查以来已更改，🔴 红色 = 报告了问题。窗体会展开显示它的控件，
  按工具箱分类分组，而每个控件又会展开显示它的 **Events**。Indexed Files 会展开
  显示记录字段（像窗体控件一样）。**随时点击最顶部的根节点**（📁 YourProjectName），
  就能在主工作区调出完整的项目设置窗体。

### Settings 菜单

凡是用来配置什么的东西，都放在同一个菜单 **Settings** 之下，所以只有一个地方可找。
它有六个条目，按你够到它们的方式分组：

| 条目 | 它做什么 |
|------|--------------|
| **Configure Claude Code** | 把 Claude Code 连接到 IDE：为每个项目一次性安装 PowerRustCOBOL 插件（见[用 AI 智能体驱动 IDE](#用-ai-智能体驱动-idemcp)）。 |
| **Coding Agent Settings** | 为整个 IDE 显示该连接的状态，并让你更新或更换它。 |
| **🐞 Debug Settings…** | IDE 的诊断开关（见[诊断开关](#诊断开关settings--debug-settings)）。 |
| **🧰 Platform SDK Location…** | 告诉 IDE 在平台的 Rust 源码不在可执行文件旁边时它们位于何处——构建应用程序要针对它们编译。 |
| **📁 Build Folder…** | 选择构建所暂存的文件夹——编译在两次构建之间保留的工作文件。留空（默认）即为每用户的缓存文件夹（见[构建结果保存在哪里](#构建结果保存在哪里settings--build-folder)）。 |
| **Reindex Knowledge Bases** | 让 AI 智能体搜索的知识库保持最新，而无需发送消息。工作流启动时，同样的增量同步也会自动运行。 |

> **注意。** **Project Settings** 界面——名称、版本、许可证、目标文件夹、AI 模型——
> 不在这个菜单里：它是主工作区的一个页面，点击项目树的根节点即可打开。菜单容纳的
> 是属于**你和这台机器**的东西，该页面容纳的是属于**项目**的东西，打开同一个项目
> 的同事与你共享它。

### 用文件夹组织项目树

每个分类都可以容纳任意层级的**文件夹**，因此大型的企业级项目也能保持易于浏览
（例如 `forms/customers/`、`src/billing/`）。

- **创建文件夹。** 点击分类标题上的 **📁+** 按钮，在它的根部添加一个文件夹，或者
  右键点击任一文件夹并选择 **New folder…**，在其中嵌套一个。
- **重命名文件夹。** 右键点击该文件夹并选择 **Rename folder…**。项目在该文件夹下
  跟踪的每个文件——以及指向其中某个文件的任何已打开编辑器标签页——都会自动随之
  更改。
- **删除文件夹。** 右键点击并选择 **Delete folder…**。你确认之后，该文件夹及**其中的
  一切都会从磁盘上永久删除**，这些文件会从项目中移除，显示它们的所有编辑器都会被
  关闭。这无法撤销。

文件夹路径始终保存为**相对于项目文件夹**的路径，因此项目可以被移动、压缩或共享，
而不会破坏任何引用。

### 移动文件：拖放

- **在树内。** 把一个文件拖到另一个文件夹（或某个分类标题）上，即可把它移到那里；
  文件会在磁盘上被移动，其项目条目也会随之更新。文件不能覆盖同名的已有文件，文件夹
  也不能被放进它自己里面。
- **从操作系统。** 把文件从 Finder/Explorer 拖到文件夹或分类上即可导入。它们会被
  复制进项目，并以相对路径跟踪。类型与目标分类不匹配的文件（例如放到 Common Code 上
  的 `.cfrm`）会被拒绝。

### 键盘导航

指针停在项目树上时，你不用鼠标也能四处移动：

- **↑ / ↓** —— 移到上一行 / 下一行可见的行。该元素会立即加载（它的属性或编辑器，
  与单击一样），树会按需滚动，使高亮行始终保持在视野内，并与顶部或底部边缘相隔
  一行。
- **→** —— 展开一个折叠的文件夹；如果它已经打开，则移入它的第一个子项。
- **←** —— 移到上级文件夹。
- **Enter** —— 打开选中的条目（与单击相同）。

首次启动时（或任何没有打开项目的时候），IDE 会显示一个完整的欢迎窗格，它是位于菜单栏/
工具栏下方可用区域正中的一个居中信息块（标题 + 许可证 + 一个空行 + 名言 + 作者）：

Welcome to PowerRustCOBOL <version>
License: Apache 2.0

<blank line>
<quote text in green, randomly selected on each cycle from a built-in list>
— <author in light blue>

名言每 7.5 秒随机轮换一次（淡入 1 秒，显示 6 秒，淡出 0.5 秒）。在你使用 File ▸ New Project 或 File ▸ Open Project 之前，左侧的树、编辑器、输出以及编辑器专用的控件都是隐藏的。一旦打开了项目，就会出现常规的三窗格工作区。完整的指南位于 docs/ 文件夹中。

- **工具栏（顶部）。** `Open · Save · Check · Build · Run · Debug · Stop`，最右侧还有
  语言选择器。*Run* 解释执行程序；*Build* 编译出原生二进制；*Check* 只运行语法
  分析 + 语义分析；*Debug* 在选中某个 Generated Code 条目时启用。
- **主窗格（树的中间 / 右侧）。** 显示代码编辑器、**属性检查器**（当你在树中点击
  某个窗体或控件时），**或者项目设置窗体**（当你点击树顶部的项目根节点，或 IDE
  首次打开项目时自动显示——此时没有可见的编辑器）。项目树上方的 **👑 Grace** 按钮
  会在此窗格中打开整个项目范围的 Grace 聊天机器人。它使用与控件属性检查器完全
  相同的玻璃窗格构造（CentralPanel + 玻璃边框），因此宽度一致（右边框处没有缺口），
  并且高度完全占满 100%（窗格随窗口或分隔条的缩放，在 Output 面板上方的可用区域内
  伸缩）。卡片圆角的下边框/描边通过边框下方的外边距，明显地保持在输出/控制台上方，
  留有可见的间隙；Save/Cancel 按钮位于卡片的底部。随时点击项目树的顶部（📁
  ProjectName 那一行）即可打开它。它有一条从上到下贯穿内容的连续垂直分隔线。左侧
  的标签从不自动换行；它们会用 `…` 截断（例如 `Standard system p…`），开发者可以
  自由拖动分隔线（分隔位置的移动与任何标签的长度无关，最多可达窗格宽度的 80 %）。
  右侧的控件是弹性的，在 10 px 的间隙之后全部从同一个 x 位置开始，使每个属性值都
  做到完美的垂直对齐。各部分依次为：Project、AI assistant、Appearance、License、
  Integrations、Runtime——AI 设置（Agents Manager、Model Providers Manager、
  Model Leaderboard）直接位于 Project 之下，这样你无需翻过一段你只设置一次、很少
  再碰的许可证文本就能到达。卡片底部有明确的 **Save** 和 **Cancel** 按钮（Cancel
  只在有更改之后才可用；它会还原到上次保存的状态）。分隔线跟随当前主题（悬停或拖动
  时更亮）。代码编辑器（可见时）沿底部带有一个**状态栏**——插入符 `Ln, Col`、
  **Insert/Overwrite** 模式（用 `Insert` 键切换）、一个 **Trim on save** 开关（保存
  时去除行尾空白），而对于非 Markdown 文档，还有一个 **Beautify** 命令，它按下文
  *Beautify——布局规则*中描述的布局规则重新格式化 COBOL。Markdown 文件没有
  Beautify，因为 COBOL 格式化不适用于它们。

<!-- 📷 project-settings-form.png — Show the left tree with the root node highlighted (hand cursor), and the main area with the two-column settings form inside its glass card (single continuous vertical resizer line, labels truncated with … before the line, all value controls aligned on the right, Save/Cancel at the bottom of the card). The card's rounded bottom border must be clearly visible above the Output panel with a gap (no… -->

<p align="center"><img src="../assets/images/screenshots/project-settings-form.png" alt="选中了根节点的项目树，以及在它旁边打开的项目设置窗体" width="900"></p>

- **Output 面板（底部）。** 程序的 `DISPLAY` 输出、构建日志和状态消息。

<!-- 📷 ide-overview.png — A full-window capture with a project open, a form selected (so the property inspector is visible), and some text in the Output panel. Annotate the four regions if you can. -->

<p align="center"><img src="../assets/images/screenshots/ide-overview.png" alt="打开了项目、选中了一个窗体、并显示着属性检查器的 IDE" width="900"></p>

### AI 助手（可选）

PowerRustCOBOL 可以在代码编辑器的正上方放一个大语言模型——由你自己提供，最好是在
这份文档上训练过的。这个助手**完全是可选的，默认关闭**：在你填写连接详情之前，提示
栏永远不会出现。

**通过项目根节点的设置窗体来配置它。** 点击项目树的顶部节点（带有你项目名称的
📁 那一行）。在窗体的 **AI assistant** 部分可以输入连接详情。AI 行为和智能体属于
打开的项目，并随它的项目文件和 `agentic_ai/` 目录一起走；提供商配置和 API 密钥则
是本机的，永远不会随仓库传播：


| 字段 | 含义 |
| --- | --- |
| **Endpoint URL** | 完整的模型 URL。使用与 OpenAI 兼容的聊天端点，例如 `https://…/v1/chat/completions`，或 xAI/Grok 的 Responses 端点 `https://api.x.ai/v1/responses`。未改动的提供商默认值会自动获得它惯用的请求路径；你编辑了这个字段之后，IDE 就会原样使用你输入的 URL。 |
| **API key** | 以 `Authorization: Bearer …` 发送。对于无需密钥的本地端点请留空。在此输入的密钥会配置它的**提供商**，与 Model Providers Manager 的做法完全一样，并且只保存在这台机器上。字段为空表示这里没有为该提供商存储任何凭据。 |
| **Model** | 在每次请求中传递的模型标识符。 |
| **Reviewer model (Pedantic Agent)** | 可选的第二个模型，以毫不妥协的严格程度评审主智能体的回答。它可以是任何模型，包括主智能体自己的那个——评审员的独立性来自它自己的提示和它自己的单独调用，而不是一个不同的模型。配置了评审员之后，**COBOL Proficiency** 检查会协同运行：主模型作答，Pedantic Agent 以主提示作为权威规范对其进行评审，发现缺陷时要求完整地重新提交修正版，重新评审修订版，并给出最终的、毫不留情的诚实评估——仪表盘随后显示的是*评审员*的分数，而不是模型的自评分数。 |
| **Temperature** | 采样的随机性（0 = 确定性）。连接测试使用的正是这个值，因为有些模型只接受其提供商定义的默认值，通常是 `1.0`。 |
| **Standard system prompt** | 每次请求都会发送的指令。已提供一个合理的默认值；可根据你的模型修改它。 |

**Model Providers Manager。** 在 Project 设置中，紧挨着 *Manage agents…* 的是
**Model Providers Manager…**。你在这里配置一个**提供商**——它的端点和它的 API
密钥——别无其他。从某个提供商的密钥生效的那一刻起，**该提供商提供的每个模型都对
任何智能体可用**；不需要做任何逐个模型的设置。从左侧的列表中选一个提供商（实心
圆点标记已配置的那个），如果需要不同的主机就调整它的端点，粘贴密钥，然后用
**Refresh models** 拉取当前的目录。**Test** 会发送一次请求，让你在依赖凭据之前先
确认它。

管理器底部的两个开关决定 Grace 回答的快慢。两者默认都关闭，除非你把它们打开：
- **Grace reasons before answering。** 许多当前的模型在回复之前会默默思考，而在大型
  模型上，这种思考就是等待时间的大部分。关闭时，Grace 对 Ollama 模型的调用会要求
  它直接回答。其他提供商会忽略这个开关。
- **Review my request before Grace starts。** 开启时，Grace 会先从 Form Designer
  改写你的请求，标出可能有两种理解的地方，并让你在任何工作开始之前编辑它。这对
  冗长或含糊的请求是一种有用的检查，代价是多对模型调用一次。关闭时，你的请求按你
  所写的原样运行。

> ⚠️ **注意事项：** 不带推理地做规划的小模型，可能会误读复杂的请求。如果 Grace 的
> 计划开始漏掉步骤，请先把推理重新打开，或给 Grace 换一个更大的模型，然后再去责怪
> 各专家。

**共享 AI 设置：Export AI… / Import AI…。** 在与各管理器同一行上，**Export AI…**
会写出一个 JSON 文件，其中包含：
- 已配置的提供商及其端点，以及每个提供商的模型列表；
- 本项目的各智能体，每个都带有它的提示、引导、策略、技能、知识和 `mcp.json`；
- Model Leaderboard。

把该文件交给同事，或在你的下一台机器上打开它，**Import AI…** 就能一步完成全部
设置，而不必花一个下午去点击。

**该文件从不携带 API 密钥。** 密钥不是它的一部分。端点中的 `user:password@` 会被
丢弃。在 `mcp.json` 中，每个 `env` 和 `headers` 的值，以及每个名称像密钥、令牌、
机密或密码的字段，都会被清空，而服务器本身会被保留。而且如果你机器上存储的任何
密钥仍然出现在了文件里，导出就会被拒绝，什么都不会写入。IDE 会在一个错误对话框中
以你为它选择的语言说明这一点，并指出在哪里发现了密钥，但绝不显示密钥本身。

**它也不携带任何关于你的信息。** 无论它们出现在哪里（智能体的提示、`mcp.json` 中的
路径，或任何其他地方），下列内容都会被替换为中性的占位符：
- 你的主文件夹，它会变成 `~`；
- 你的登录名；
- 你的全局 git 配置所记录的你的姓名和电子邮件。

排行榜某一行的最后一条错误消息会被丢弃，因为提供商的错误文本可能会写出你的账户或
组织的名称。Output 面板会说明有多少细节被替换了。与密钥一样，如果有哪一项无法移除，
就不会写入任何内容。

正因为如此，导入以索要密钥作结：一个窗口会列出该文件使用的每个提供商，每个提供商
一个密钥字段，并说明你是否已经为它存储了密钥。输入你自己的密钥，或让该字段留空以
保留已存储的那个，或按 **Later**，稍后再到 Model Providers Manager 中添加。

导入是**合并**——不会删除任何东西：
- 每个提供商的端点会被导入的端点替换；
- 同名的智能体会被覆盖，但保留它的本地身份，因此它的伙伴链接仍然有效；
- 一条排行榜行，只会被同一模型更新的测试所替换。

智能体需要一个打开的项目；没有打开项目时，只导出或导入提供商和排行榜。大于 1 MB 的
智能体文件，以及不是文本的文件，都不会进入导出，Output 面板会点出它们的名字。

> ⚠️ **注意事项——分享之前请先读。** 只有上面列出的凭据和个人信息会被移除。粘贴进
> 智能体的提示或知识文件里的密码、同事的地址或客户的姓名，会随它们一起传出去。
> 该文件是纯文本：分享之前请先读一遍。

**调用失败时。** 错误窗口会在顶部、一条分隔线之上，单独一行给出原因，下面是完整的
连接日志。标题是提供商自己的那句话，原样引用——*"You exceeded your current quota,
please check your plan and billing details"*、*"'temperature' is not supported with
this model"*——当这句话本身没有写出所指的请求字段或错误代码时，会在它下面显示该
字段或代码。下面的日志保持不变且完整；**Copy** 和 **Save…** 取走的是整个日志，而
不是标题。如果错误的载荷不含这样的句子，就不会有标题：你绝不会看到对一件没有被说
出的事情的概括。

**注意——推理模型通过测试。** *Test* 只问一个问题：这个模型能连通并作答吗？有些
模型先思考再说话，对这么小的请求只返回隐藏的推理——端点解析成功、密钥被接受、令牌
返回了，但没有可见的文本。这算作通过，结果也会如此说明。只有智能体才需要可见的
文本：它们把回复解析成窗体操作，而它们从未见到的推理是无法应用的——因此，只用隐藏
推理来回答智能体的模型，仍会被报告为*在那里*不可用，并给出同样的建议：为它关闭
思考。

**思考超出预算的模型会得到更大的预算。** 当一个推理模型把它的整个输出预算都用在
思考上，始终没有开始作答时——无论是在 COBOL 熟练度检查、聊天还是智能体中——请求会
以两倍的预算再发送一次，最高 32,768 个令牌。只有在那个规模下它仍然没有开始作答，
才会被报告，此时的建议是先选一个不带推理就能作答的模型。没有需要编辑的文件。

右侧的提供商面板可以**滚动**——端点、密钥、模型和 *Where keys are kept* 无论你把
窗口缩得多矮都能到达，左侧的提供商列表则独立于它自行滚动。

提供商配置是**机器范围**的，存放在你其他的本机设置旁边，而不是项目中。配置一次
Anthropic，这台机器上的每个项目都能使用它。API 密钥**从不**写入项目文件、生成的
COBOL，或编译或打包后的应用程序。本地的 Ollama 完全不需要密钥——有一个可达的端点
就够了。

> **注意。** 这取代了较早的 *Models Manager*：在那里，连接是按*模型*定义一次的，
> 作为一个命名的“模型配置文件”，智能体引用它。要使用你已经付过费的提供商的第二个
> 模型，就得建立整整第二个配置文件，并再次粘贴同一个密钥。
>
> **你已有的项目会自行迁移。** 你第一次打开其中一个时，每个智能体会接管它所引用的
> 配置文件的提供商、模型、temperature、输出令牌上限和超时，每个提供商则按这些配置
> 文件所知道的内容来配置。不会向你询问任何东西，也不需要重新输入任何东西。⚠️ 现在
> 一个提供商只能持有**一个**密钥，所以如果你在同一个提供商上有几个带*不同*密钥的
> 配置文件，就会保留最近存储的那一个，其他的会在 Output 面板中点名——如果你想要的
> 是其中之一，请到 Model Providers Manager 中重新输入。

#### 你的密钥保存在哪里

默认情况下，密钥只存活**一次运行**。不会向磁盘写入任何东西，下次打开 IDE 时它会再
问一遍。这是刻意为之——磁盘上的密钥就是可以被复制、被备份或被提交的密钥——但这很
烦人，所以在 Model Providers Manager 的底部，由你自己来决定：


| 选择 | 会发生什么 |
| --- | --- |
| **Not kept** | 默认值。密钥只存在于这个进程中，下次运行时会再次询问。 |
| **A local file** | 整个模型配置（包括密钥）会写入你指定名称的文件。创建时仅所有者可读（macOS 和 Linux 上为模式 `0600`），并在顶部带有一条纯文本警告。重新打开 IDE 会直接取回这些密钥。 |
| **The OS credential store** | 你所在平台自己的保险库——Keychain、Credential Manager、Secret Service。已列出，但**尚不可选：它随正式版发布**，届时会有一个可以检视、轮换和清除其内容的界面。 |

**文件绝不能位于 git 仓库之内。** 这不是偏好，也没有覆盖的办法。如果你选的路径位于
任何 `.git` 之下——在仓库根目录、埋在十层文件夹之下，或在子模块或 `git worktree`
检出中——它都会被拒绝，拒绝信息会指出是哪个仓库，让你知道撞上了哪一个。一个被提交
的密钥就是被公开的密钥，而公开的密钥是收不回来的。

首先提供 `/tmp/llm_config.json` 正是出于这个原因：`/tmp` 里的任何东西都无法被提交，
而且它在重启后不会保留——对于凭据来说，这是一个特性。点击一个建议的路径或输入你
自己的，按 **Use this file**，密钥就会在配置被保存时写入。**Forget the file** 会
删除该文件，并回到完全不保存密钥的状态。

机器范围的配置文件没有变化：它仍然**不带任何凭据**，只记录你对密钥存放位置的选择
和你选的路径。在管理器中删除一个密钥仍然会把它删掉——明确的删除永远胜过一个会
记住的文件。

> ⚠️ **注意事项。** 文件以明文保存你的密钥。它只靠文件权限来保护，别无其他：以你
> 的身份运行的任何东西都能读取它，复制该文件夹的任何备份中也都会有它。如果这不可
> 接受，请把选择保持在 **Not kept**，直到操作系统凭据存储随正式版到来。

**Agents Manager。** *AI agents* 这一行会打开项目预置的智能体数据库，分为三个标签页。

**标签页 1 —— Agent × Model。** 每个智能体一行——Grace、每个专家、每个评审员以及
COBOL Proficiency Judge——并带有决定该智能体如何运行的各项设置。


| 列 | 含义 |
| --- | --- |
| **Agents** | 该行所配置的智能体。 |
| **Models** | 它运行在哪个模型上，从表格上方 **Model provider** 框中所选的提供商里挑选。选 **— no model —** 可以有意让某个智能体保持未配置。 |
| **Rating** | Leaderboard 对该模型所知的信息，如果它从未被评测过，则显示 *Not tested*。 |
| **Temp** | 仅对这个智能体生效的采样随机性（0 = 确定性）。 |
| **Output Tokens** | 这个智能体可以产生的最大回答。 |
| **Timeout** | 等待它多长时间，以秒为单位。 |

**Model provider** 框是一个*选择器范围*，而不是项目范围的开关。它决定你在配置时
Models 列提供哪个提供商的模型，并且不会改动任何你没有碰的智能体——所以 Grace 可以
运行在云端提供商上，而你的专家们运行本地的 Ollama。每个智能体都记得它的模型来自
哪个提供商。某些提供商提供数百个模型，选择器旁边的搜索框可以缩小列表。

**共用一个模型。** 任何智能体都可以运行任何模型——Grace、Judge、每个专家和每个
Pedantic 评审员——单一一个模型也可以为它们全部服务。表格里不会因为另一个智能体已经
在运行某个模型，而对它发出警告、将其置灰或拒绝；Judge 对 Leaderboard 测试重新评分
时，用的就是它所拥有的任何模型，包括正在被测试的那个。给不同的智能体不同的模型，
是关于成本与质量的选择，绝不是一种许可。页脚只报告*缺少*的东西：一个提供商还没有
密钥的已启用智能体，或一个没有评审员的专家，或评审员没有模型的专家。

**当提供商停用一个模型时。** 模型会被停用——Anthropic、OpenAI、Meta 等各自按自己的
时间表撤下它们——而一个已不存在的模型的排名，比没有排名更糟：它会诱使你选它。所以，
Model Providers Manager 中一次带回了目录的刷新，也会把该提供商的、目录里不再列出的
任何模型从 Leaderboard 上撤下，并在 Output 面板中说明是哪些。

只有**确实列出了模型**的刷新才能做到这一点，而且只针对它列出的那个提供商。失败的
请求、过期的密钥，以及一个你还没有刷新过的提供商，都会产生一个空列表，而这对实际
存在什么毫无说明——所以空的结果根本不会移除任何东西。你也可以自己停用一个模型：
Leaderboard 的每一行都有 **Remove**，用于提供商在其目录尚未跟上之前就关停了某个
模型的情形。它会先询问，因为一次排名要花费真实的令牌和真实的时间。

如果有智能体正在运行被撤下的那个模型，Agents Manager 会在那个智能体上打开，这样你
就可以立刻给它换一个——指向已撤下模型的智能体，才是真正会让一次运行出问题的部分，
而到下一个工作流才以连接错误的形式发现它，是代价高昂的得知方式。

移除是持久的：被停用的模型不会被下一次项目同步放回去，也不会被归档基准报告的重放
放回去。你在 `agentic_ai/model-benchmarks.jsonl` 中的归档不受影响——那些报告是你
运行过并为之付过费的记录，这一切都不会删除它们。**再次测试被停用的模型会把它带
回来**，连同它的新结果，所以对于你不同意的停用，撤销它只需要运行一次。

**标签页 2 —— Agent Configuration。** 左侧的智能体列表驱动右侧的详情窗格：**Agent
Details**（id、名称、种类、专长、用途、是否启用）、提示编辑器、能力、知识和关系。

**标签页 3 —— User Guide。** 一份书面指南，讲述模型与智能体如何配合，使用你的界面
语言。它的四个部分，每个都以一段平实的解释开头，然后深入一步，再给出精确的版本——
读到有用的地方就可以停下。它涵盖：把智能体与模型配对（以及把同一个模型给所有智能体）、
每项设置的作用、为什么你最强的模型应当用在评审员和 Judge 上而不是撰写者上，以及
术语（模型、智能体、Pedantic 评审员、Judge、令牌及其成本、本地模型、量化，以及为何
显存（VRAM）才是决定本地模型是否可用的那个数字）。
搜索会高亮匹配项并在它们之间跳转，文字大小可以调节，目录可以跳转，而 **Export PDF**
会把整份指南写出来。

页脚带有 **Cancel**、**Apply**（保存并继续工作）和 **Save**。内部的 `agentic_ai/` 目录
被有意从项目树中隐藏；智能体配置请使用 Agents Manager，而 Grace 会自动在那里保存它的
工作流记录。提示编辑器可以在四到二十行文本之间垂直调整大小；更长的提示在编辑器内部
滚动，而不是增加它的高度。**New Agent** 和 **Delete Agent** 目前是隐藏的，因为完整的
内置智能体网络会随项目一起创建和修复。这两个工作流仍然保持已实现，供将来维护之用。
智能体位于你的项目中的 `agentic_ai/<agent name>/`——多行的智能体提示在
`<agent name>_prompt.md` 中，另有 `steering/`、`policies.md`、`skills/`、`mcp.json`、
`knowledge/` 和 `agent.json`（身份和运行时配置——API 密钥**从不**存放在项目里；密钥
留在你的机器上，每个模型问一次）。智能体名称是唯一的，并在创建时固定，因为它们命名
了文件夹。每个主智能体都可以指定一个**严苛评审同伴**来评审它的回复。任何智能体都可以
使用任何模型——同伴可以运行在它所评审的智能体的同一个模型上，因为使评审保持独立的
是它自己的提示和它自己的单独调用。这个关系是一对一的：一个编排者或专家最多可以有
一个 Pedantic 同伴，而一个 Pedantic 评审员最多只能属于一个被评审的智能体。可以从
主智能体的 **Companion (Pedantic reviewer)** 部分，或 Pedantic 智能体的可编辑部分
**Pedantic Companion for** 选择这个关系；两个选择器写入的是同一份项目配置。Grace 的
规划者和参与的智能体在运行时会收到确切的关系，所以评审员不能被替换，也不能被复用于
另一个智能体。创建项目时会预置固定的专家——**Form Designer Agent**、**COBOL Event
Handler Script Agent**、**Documentation Agent**、**Data (Indexed File) Agent** 和
**Version Control Agent**——外加编排者 **Grace**。每一个后面都紧跟着它自己的评审员，
其规范名称是主名称加上后缀 **Pedantic Reviewer**：

- **Grace Pedantic Reviewer**
- **Form Designer Agent Pedantic Reviewer**
- **COBOL Event Handler Script Agent Pedantic Reviewer**
- **Documentation Agent Pedantic Reviewer**
- **Data (Indexed File) Agent Pedantic Reviewer**
- **Version Control Agent Pedantic Reviewer**

每个评审员在创建时都带有针对其用途的提示、描述、路由契约和一对一的同伴链接。开发者
选择它的模型配置文件，并可以调整它的提示、技能、工具和知识；不需要手工构建或关联任何
评审员。打开一个已有的项目会运行同样的幂等修复：缺失的内置评审员会被重新创建并重新
链接，而非空的项目提示和其他开发者配置仍然具有权威性。较旧的评审员名称会就地迁移，
不改变它们的稳定 ID 或所选的配置文件。

Grace 始终是唯一的协调权威（👑，永远叫 Grace，永远不可删除），它规划多智能体的
工作，按种类和专长委派给各专家，执行每一道 Pedantic 评审关卡，并汇集最终经过验证的
结果。**Grace Pedantic Reviewer** 的默认提示评审请求覆盖度、任务分解、归属、依赖关系、
文档治理、证据、跨智能体集成、失败以及完成声明。项目本地的评审员提示在 Agents Manager
中仍然可以编辑，固定智能体的修复会保留这些编辑。在启用某个评审员的评审连接之前，先在
运行时表格中给每个评审员一个模型；Pedantic 评审员可以运行任何模型，包括它的主智能体
所运行的那一个。

**当 Grace 提问而不是动手时。** 一个有不止一种理解的请求，得到的是一个问题而不是
一次猜测——同一个聊天中的一个红色气泡，准确指出什么是有歧义的。就在同一个框里回答
它，想多简短都行（“the Caption”、“UUID”、“aas-clientes”）：回答带着它所回答的那个
问题一起送回去，所以 Grace 会带着你的决定恢复原来的请求。你不必重述你问过什么。如果
你转而输入了别的东西，那就成了新的请求，这些问题会被丢弃。

内置的路由契约是明确的：Form Designer Agent 负责 RAD 窗体设计，并把事件实现委派出去；
COBOL Event Handler Script Agent 实现那些确切被委派的行为；Documentation Agent 独自
撰写项目文档，并准备规范化的索引文件模式交接；Data (Indexed File) Agent 独自通过 Indexed
File 界面模型维护 `.cidx` 定义；Version Control Agent 负责有证据支持的项目 Git 操作和
确认关卡；而 Grace Pedantic Reviewer 只评审 Grace 的编排。每个智能体都会收到一个针对其
角色的默认提示。空的或已知的旧版默认值会被修复，而非空的、项目编辑过的提示仍然具有
权威性。已有的 `DocumentationAgent`、`Pedantic Grace Reviewer`、`Grace Pedantic Reviewer Agent`、`Pedantic UI Agent` 和 `Pedantic COBOL Companion` 记录会在磁盘上被重命名，
而不改变它们的稳定 ID 或它们的模型。多余的 `Orchestrator Pedantic Reviewer Agent`
会被合并进 **Grace Pedantic Reviewer** 并被移除。

项目树上方的 **👑 Grace** 按钮会填满当前树窗格的宽度（最小 150 px），并在你调整窗格
大小时跟随它。它在主窗格中打开一个限定于项目的对话，带有持久的历史、工作流进度，以及
针对受限操作的批准控件。它的属性窗格标题标明它是
**👑 Grace - The PowerRustCOBOL Agentic AI Orchestrator**。

**选择东西放在哪里。** 因为项目树支持文件夹，一个名称可以存在于不止一个地方。当你
让 Grace **创建**一个元素（窗体、索引文件、公共代码源文件、文档文件或资源）时，它会
打开一个小的居中窗口，显示项目树，让你选择目标**文件夹**——你也可以当场在那里新建
一个文件夹。当你让 Grace 按名称**编辑**一个元素、而不止一个元素同名时，同一个窗口让
你挑选**是哪一个**；如果只有一个匹配，Grace 就直接编辑它。取消这个窗口会停止操作，
Grace 会报告没有创建或编辑任何东西。（这个提示出现在完整的项目 Grace 聊天中；紧凑的
编辑器/设计器聊天界面无法显示它，所以在那里，有歧义的请求会要求你使用项目 Grace
聊天。）

每一个 IDE 聊天机器人都经由 Grace 路由。所在界面提供一个建议性的偏好：RAD Form
Designer 偏好 Form Designer Agent，它的事件编辑器偏好 COBOL Event Handler Script
Agent，而代码编辑器则请 Grace 按能力来选择。这个偏好从来不是排他的。Grace 可以把一个
请求拆分给任何已启用的专家，所以一个创建按钮并为其 `onClick` 行为接线的请求，可以同时
协调窗体设计和事件处理程序两类任务。每个工作流都会运行它所配置的 Pedantic 评审、流式
输出进度，并在 `agentic_ai/Grace/runs/` 下保存一份可审计的记录。

**实时动作状态。** 在 Grace 和各专家工作时，对话会以一行简短的状态显示每个智能体此刻
正在*做*什么——例如 `Form Designer Agent: Drafting response — T1` 或
`Grace: Retrieving context`——最多每秒更新一次，使长时间的运行永远不会看起来卡住。每一
步也都会落入一个默认在对话中保持折叠的 **Agent actions (N)** 条目；展开它，就能复核
这次运行所走步骤的、按智能体分组的有序序列，它会与聊天历史和工作流记录一起保存，所以
在你重新打开项目之后依然可以复核。状态行**只点出动作**，并以你的界面语言显示。一个动作
所产生或消耗的内容——检索到的知识、工具输出、模型推理——绝不会出现在对话中：完整的
轨迹存在于 Output 面板的 AI 日志、诊断转储（当某个调试开关打开时）以及 `agentic_ai/Grace/runs/`
下保存的运行记录之中。在项目的 **verbose** AI 设置启用时，动作流会多出更细的步骤
（每次工具调用、每轮评审）——粒度更细，但仍然绝不包含内容。verbose 模式还会在每次
运行之后，往对话末尾追加一行 **Token savings**——检索让已索引的 Knowledge Base 语料
中有多大比例*没有*进入上下文（检索到的记录对比整个语料，按每令牌约 4 个字符估算）——
这样你就能看到检索层为你换来了什么。

**分块检索。** Knowledge Base 文档会被索引两次：整篇文档（用于文档管理），以及一个
**分块存储**——其中每个控件、属性、方法、事件和每一段文字说明各自是一条记录，带有一个
`PIC X(512)` 的内容字段——更长的内容会续接在与前一条相链接的记录中，搜索时会把这条链
重新拼装起来。每条记录的文本都被单独嵌入，所以当你向 Grace 询问比如 DataGrid 事件时，
上下文收到的是 DataGrid 的那些记录——而不是整个控件目录。IDE 自己的参考资料位于
`~/PowerRustCOBOL/data/chunked.data`；每个项目把它的文档保存在
`data/<project-name>-chunked.data` 中。保存、编辑或删除一份 Knowledge Base 文档，
文件本身保持不动，只在下次运行时对该文档的记录重新分块并重新嵌入。

IDE 的分块存储**随 IDE 本身一起交付**，已经用语义模型预先嵌入：全新的克隆或安装一开始
就带着现成的索引，除非某份 Knowledge Base 文档被删除、更改或替换，否则绝不会重新嵌入
参考资料。在尚未下载语义模型的机器上，随附的记录会被保留，并用词法方式搜索，直到模型
到来——不会丢弃任何东西。每当确实需要（重新）嵌入记录时——文档被更改，或是你自己的
项目文档——对话中会显示一个**进度条**（`Indexing Knowledge Base (n of m records)`），
使漫长的索引永远不会看起来卡住。

### 项目范围的代码搜索

如果你曾在 PowerCOBOL 中维护过应用程序，你会记得那套例行公事：“我还在哪里用过
`CUST-BALANCE`？”意味着要手工打开每一张表单和每一个事件过程。PowerRustCOBOL 在一个
窗口里就回答了它：**View ▸ Code Search…**、🔍 **Search** 工具栏按钮，或
**Ctrl+Shift+F**（macOS 上是 **Cmd+Shift+F**）会打开搜索窗口；普通的 **Ctrl+F**
保持它原来的含义，即在当前编辑器标签页中查找。

输入一个纯文本查询，然后按 **Search**。扫描覆盖项目中**你能写 COBOL 的每一个地方**：
每个控件事件处理程序、每个窗体的 `onLoad`/`onClose`、每个用户过程、每个窗体的五个结构
部分（`SPECIAL-NAMES`、`REPOSITORY`、`FILE-CONTROL`、`FILE SECTION`、`WORKING-STORAGE`）
——已打开的窗体读取的是它们的**实时文本，哪怕尚未保存**——外加每个 Common Code 文件。

- 结果先按窗体分组，再按位置分组，每一行显示*在该处理程序或部分之内*的行号，以及高亮
  了匹配项的匹配行，合计行统计出现次数和不同位置的数目。
- **Case sensitive** 和 **Whole word** 默认都是关闭的。Whole word 理解 COBOL 的词：
  `BAL` 不会在 `CUST-BAL` 之内匹配。
- **双击**一条结果，IDE 就会打开所属的编辑器——事件模态框、COBOL Structure 窗口，或
  Common Code 的代码编辑器——并把插入符放在那一行上；如果该窗体的设计器尚未打开，会先
  打开它。
- 这个窗口归你所有，直到你关闭它：你在跳转、编辑和重新 Check 的过程中它保持打开，
  只有当你拖动它的角上抓手时才会改变大小，只在点击它的 **✕** 或 **Cancel** 时关闭。

它刻意**不**搜索的东西：生成的 `.cbl` 文件（构建产物——其中的每一处命中都是其真实
位置上某处命中的重复）和已删除代码的回收站。

<!-- 📷 code-search.png — The search window over a project, showing grouped results with highlighted matches and the totals line. -->
<p align="center"><img src="../assets/images/screenshots/code-search.png" alt="针对一个项目的搜索窗口，显示着分组的结果、高亮的匹配项和合计行" width="900"></p>

### 窗口效果

每个项目都可以为它的窗口配置一种标志性的**进入和退出效果**，在项目设置（Appearance
部分）中配置一次，并应用于该项目的**所有**窗体：为每个方向选择一种效果、一个时长
（100–3000 ms；Matrix 数字雨使用它自己的 1500–4000 ms 区间，而 Transporter II 固定为
恰好 4000 ms）和一种缓动。这个目录从经典的过渡——淡入淡出、dBASE 风格的方框
**缩放**、滑动、从标题栏展开——到带遮罩的揭示（**雷达扫描**、光圈、百叶窗、棋盘格），
再到 **Matrix 下落代码**数字雨（经典的片假名和数字字形，从上边缘之上落入一个完全透明
的窗口；每一行轨迹的末端——那个暗淡的顶端字形——沿着它的列带向下走，并逐步揭开它
身后的东西，所以窗体恰好在最后一个字符离开时完整呈现。各行按真实的时钟到达，最先的
几行相隔 25 ms，其余的以各自的速度彼此相隔 10–25 ms；只有这一种效果忽略缓动设置，
按线性时间运行）、精灵式挤压，直到 **Transporter II**。新项目以 Matrix 进入效果开始、
没有退出效果；在这项功能之前创建的项目，在你另行选择之前，窗口保持瞬间出现。

**Transporter II** 是一种电影式的物质化揭示，也是唯一一种长度固定的效果：它恰好运行
**4000 ms**，分为两个阶段。

1. 两道细细的水平光束，各自约为窗体宽度的一半、水平居中，从**垂直中心线上相互重叠**
   的位置开始，然后分开——一道升向上边缘，一道落向下边缘。它们之间张开的缝隙里充满
   了密集的白色和黄色粒子云，粒子以不同的不透明度闪烁、漂移和发光：一个充满活力、
   但完全透明的物质化场。
2. 随着水平光束落在边缘上，它们逐渐淡出，两道**全高的垂直光束**在水平中心淡入。它们
   向左右两个边缘扫开，你的窗体就在它们之间不断变宽的带中被揭示出来，粒子云在光束
   经过的地方消散。在收尾阶段，粒子、辉光和光束本身都缓缓减弱直至消失，所以光在光束
   到达边框的那一刻就已不见，而完成的窗体独自矗立。

每一道光束都是分层的半透明渐变——轴线上是白色，两翼是暖黄色，外面裹着一层柔和的
泛光——绝不是实心的条或硬边的线。该效果在一个透明的窗口上播放，所以窗体是对着你的
桌面被揭示出来的，而不是对着一个填充的矩形。作为退出效果，它把整个序列倒着播放，
让窗体**去物质化**，这使它成为唯一值得在两个方向上都设置的效果：把窗口送上屏幕的那
同一批光束，又把它带走。

> **注意。** 这个效果的时长微调器固定为 4000 ms，缓动设置也不适用——两个阶段、光束的
> 交接和最终的淡出，都按那同一个时钟切分，拉伸或缓动它会让它们偏离各自的节拍。这与
> 让 Matrix 数字雨按线性时间运行的道理相同。

在进入或退出效果运行期间，窗口**没有标题栏**，所以动画播放时没有任何东西静止不动；
标题栏随完成的窗体一起到来（并且只在该窗体被设计为显示标题栏时才出现）。那些只是移动、
缩放或淡化窗体自身表面的效果——淡入淡出、缩放、滑动、从标题栏展开和精灵式——走得更
远，会打开一个**透明窗口**，使窗体在桌面上自由地动起来，Matrix 数字雨（它只把窗体
绘制到每条下落行的尾部，所以没有被碰到的底面根本不会被绘制）和 Transporter II（它通过
裁剪到两道光束之间的带来揭示窗体，所以光束尚未到达的底面同样不会被绘制）也是如此。
在这些窗口上，窗体的 **Transparency** 属性也会真正地作用到桌面上，而 macOS 不会在
窗口周围绘制投影（它会勾勒出那个不可见窗口的轮廓，而该平台只在窗口创建时提供这个
开关）。只有带遮罩的揭示保留不透明窗口：它们通过在窗体上绘制遮盖物来隐藏窗体，而
透明的东西无法撤销这一点。

窗体从不自己挑选效果——每个项目一种外观——但任何窗体都可以通过其 Form 属性中的
`WindowEffects` 复选框**选择退出**（模态警告可以瞬间出现，而应用程序的其余部分带着
动画）。进入效果在窗口第一次打开时播放；启用 **"Play entrance when restored"**，还会
在用户还原一个最小化的窗口时重播它（仅是视觉重播——不会触发任何窗体事件）。控件的
加载时动画会等到进入效果结束，所以窗口先物质化，控件随后立即活跃起来；COBOL 的
`onLoad` 时机不变。

一个*带有*加载时动画的控件会被**扣住，直到进入效果结束**——它根本不会被绘制进进入
效果里，而是在效果结束的那一刻凭自己的力量到来。这正是你想要的：一个被设置为从左边
飞入的按钮，不应在窗口物质化时就已经坐在原位，然后又跳回左边缘、第二次游进来。没有
加载动画的控件，一如既往地随窗口一起出现。

> ⚠️ **在 1.61.5 之前**，每个控件都会被绘制进进入效果里，所以带动画的控件确实会随窗口
> 一起物质化，然后又再飞入一次。如果你曾围绕这一点，通过给控件设置延迟来进行设计，
> 请移除这个延迟。

退出效果在窗口真正关闭之前播放——但处于 `Waiting` FormState 的窗体会在任何动画*之前*
就拒绝关闭，所以被否决的关闭什么都不播放，而 `onClose` 仍会在真正关闭时恰好触发一次。

效果在**你窗体的每一个宿主中**都会播放：从 IDE 运行窗体（Run Form）和**构建出来的
应用程序**一样（两者运行的是同一个窗口宿主，所以你在 Run Form 下看到的，就是你的用户
从 `dist/` 中的可执行文件看到的）。这些设置在构建时随二进制一起走——已交付的应用程序
旁边不需要任何项目文件。所设计的**窗口属性和生命周期**也是如此：构建出来的应用程序以
窗体自己的标题打开（只有当所设计的标题为空时，才回退到 *"AppName vVersion"*），遵从
`TitleVisible`、最小化/最大化按钮、全屏、起始的 WindowState 和 StartPosition，在程序
结束时关闭它的窗口（设置了退出效果时会经过退出效果），并且和 Run Form 一模一样地触发
`onShow`/`onActivate`/`onClose`。

两点实用说明。效果在窗口内部绘制：在原生标题栏可见时，动画覆盖内容区域；一个无边框的
窗体（`TitleVisible` 关闭）加上透明度，会给效果整个窗口矩形。另外，一个机器范围的总开关
位于 **Settings ▸ Debug Settings ▸ "Disable window effects"**——无需触碰任何项目，就让
各处的窗口瞬间出现，适用于对动态敏感、GPU 较弱或自动化的场合（`PRC_NO_WINDOW_FX=1`
对一个裸的 `rcrun run-form` **或一个构建出来的应用程序**也起同样的作用，后者遵从同一个
变量）。

**嵌入设备。** 一项策略同时适用于 System KB 和每个项目 KB，索引和搜索都一样：当有受
支持的 GPU 可用时，嵌入器会**全速**使用它——macOS 上是 Metal，NVIDIA 的 Linux/Windows
上是 CUDA（用 `embed-cuda` 选项做出的构建）——否则回退到 CPU 的**低功耗**模式，把它的
计算线程限制在两个，这样漫长的重新索引会保持安静，而不是占满每一个核心。高级用户可以
覆盖任一边：设置 `RAYON_NUM_THREADS` 来选择 CPU 线程数，或 `PRC_EMBED_DEVICE=cpu|metal|cuda`
来强制使用某个后端（被强制的 GPU 如果失败，仍会回退到 CPU 而不是崩溃）。当前的设备显示在
Models 模态框中，位于语义模型状态的旁边，并由命令行重新索引打印出来（`embedding device: …`）。
Linux/Windows 上的 AMD 和 Intel GPU 不受推理后端支持，使用 CPU 路径。

当智能体在窗体上**重新定位控件**时，受影响的控件会从它们原来的位置**滑行**到新位置——
全部同时进行，历时大约一秒——这样你就能看到布局的变化成形，而不是控件跳来跳去。智能体
**创建**的控件会以同样的方式宣告自己：它会播放一次性的 **ZoomOut** 脉冲，历时一秒——
全尺寸，缩到大约四分之一，再回到全尺寸——这样你一眼就能看出窗体上什么是新的。一次请求
创建的所有东西一起脉冲，与移动用同一个时钟，所以一个变更集读起来就是一个动作。智能体
只是重新发送的控件（智能体经常会重复整个变更集）不会再次脉冲。

这两种动画纯粹是视觉上的：窗体、它保存的 `.cfrm` 和它生成的代码立即就保存着最终的位置和
完成的控件，而脉冲从不写入控件——它不会随你的窗体进入构建出来的应用程序。

**你按下 Send 的那一刻会发生什么。** 在 Form Designer 的 AI Assistant 中，工作流不会
立即开始：Grace 首先把你的请求读回来以求清晰，把它改写成各专家将被要求遵守的措辞，并
标出仍有两种读法的任何段落。这一遍花多长时间取决于一次模型调用要多久，在它运行期间，
窗格会这样说明——一个转圈图标和 *Grace is reviewing the request…*，用 IDE 的语言——
既在提示框下面的那一行，也作为对话记录中的最后一个气泡。它结束时，你会得到供你阅读、
编辑和批准的评审；只有这时工作才开始。失败的、或返回内容无法读取的评审，不会让你付出
任何代价：你的请求会按你所写的原样发送。

每个聊天机器人的撰写框都把 **Send** 紧挨着放在提示的右边。提示占据剩余的宽度，而当
聊天窗格调整大小时，这个命令始终保持可见；多行的撰写框不会把 Send 移到下面的一行。
已完成的智能体回复气泡带有仅有图标的 **Copy** 和 **Save as Markdown** 命令，悬停时有
工具提示。Save 在当前项目的 `Knowledge Base/` 文件夹中打开，要求目标仍在该文件夹之内，
写出一个 `.md` 文件，把它索引进项目的向量 Knowledge Base 索引，并刷新项目树中的
Knowledge Base 分支。开发者的消息、静态的欢迎文字，以及正在流式输出的气泡，不会显示
这些回复操作。

Grace 把只读的对话和项目工作区分开来。能力和帮助类的问题，例如 **What can you do?**，
以及描述、解释、总结、比较、建议或推荐的请求，会得到直接的 Markdown 回复，而不会创建
合成的工作流。Markdown 是这些被动请求预期的聊天机器人格式，不会因为缺少工作流 JSON
而被拒绝。如果请求还要求 Grace 创建、修改、保存、删除、实现或以其他方式更改项目资源，
就需要可执行的工作流 JSON。具名的项目智能体只使用它们由项目定义的提示；网状传输从不
追加不相关的 CodeGenerator、FormsDesigner 或 EventBinder 前导。如果一个可执行的请求
返回了格式错误的工作流 JSON，Grace 会收到一次明确的更正请求。第二次格式错误的结果会
打开错误模态框，并把两次解析失败以及完整的更正后载荷记录到 IDE 日志中。

**列出项目的文件。** 只向 Grace 要一份项目文件的清单——*list the forms*、*which indexed
files are there?*——IDE 就会根据项目自己的清单（manifest）作答：它跟踪的每个窗体、索引
文件、源文件、生成的程序、资源或文档，连同其完整路径，位于 Grace 对你问题的一行解读
之下。没有 Knowledge Base 搜索，也没有规划轮次，所以答案是完整的，并且在 Grace 读完
请求之后立即给出。按文件内容缩小范围的清单（*the responsive forms*、*the forms with a
DataGrid*）仍然走正常的工作流，因为必须打开那些窗体才能回答。

**Grace 回答你刚问的那个问题。** Grace 在做任何其他事情之前对你的请求所作的解读——
如果不清楚，她会拿给你看的那一个——是这次运行其余部分的依据。同一个聊天中的新问题就是
新的请求：只有对*Grace 向你提出的*问题的答复，才会与你先前问的内容合并。

<!-- 📷 project-grace-chat.png — Show the width-responsive 👑 Grace button above the project tree and the project-wide Grace conversation open in the Main Pane, including transcript, prompt, and conversation controls. -->
<p align="center"><img src="../assets/images/screenshots/project-grace-chat.png" alt="项目树上方的 Grace 按钮，以及在主窗格中打开的整个项目范围的 Grace 对话" width="900"></p>

空的 Grace 对话一打开，就带着关于 Indexed Files、CRUD 窗体、数据绑定的 DataGrid，以及
计划 → 任务 → 实现工作流的实用示例。对于持久的项目文档，Grace 总是委派给固定的、不可
删除的 **Documentation Agent**。它是唯一被允许格式化、创建或更新项目文档的专家。各领域
专家准备权威的源材料；Grace 把这次交接表达为任务依赖，工作流把每一份获批的源输出提供给
Documentation Agent。例如，一个为窗体撰写文档的请求，会先向 Form Designer Agent 要控件、
布局、绑定和事件，再让 Documentation Agent 格式化并保存那份获批的材料。Documentation
Agent 不得捏造缺失的领域事实。

Documentation Agent 只能在项目的 `Knowledge Base/` 文件夹下创建、读取和列出文本文档。
成功的写入会立即被项目跟踪，并索引进位于 `data/project-knowledge.redb` 的项目本地向量
索引（纯 Rust，嵌入式）。Grace 在执行之前会验证这个协调结构，并在文档工作流把写作分配
给另一个专家、或遗漏了必需的源依赖时，请求一次更正后的计划。

搜索的是两个 Knowledge Base，绝不是一个。**System Knowledge Base** 是平台自己的参考——
控件及其属性、事件和方法、RustCOBOL 扩展、窗体主题、布局模型、项目模型——它位于每个项目
之外，所以从不会被复制进你的项目。**项目 Knowledge Base** 是你自己的材料：你和 Grace 在
项目的 `Knowledge Base/` 文件夹下撰写的文档。在每一次 Grace 请求之前，包括只读的问题，
IDE 都会同步这两个索引并搜索两者；摘录到来时标有它们所来自的存储，而 Grace 只对你自己的
文档引用相对于项目的路径。

相关的摘录优先于通用的模型训练；当两个 Knowledge Base 都没有相关的证据时，Grace 会这样
说，给任何通用性的指导加上标签，并索要缺失的项目事实，而不是捏造它们。每个专家都获得
对同样两个存储的受管控、只读的 `knowledge.search` 访问，所以平台的事实和先前的项目决定，
在之后的工作中都可以检索到。Grace 自己不搜索：她在一次对她模型的单次调用中进行规划，
依据的是她开始之前检索到的摘录，当一个问题需要这些摘录遗漏的条目时，她把查找作为任务
交给一个专家。她自己做的每一次搜索，都要让她的提示再完整地过一遍模型，在一个缓慢的
推理模型上这就是几分钟。

索引文件的工作使用由 Grace 协调的、强制性的两专家交接。Documentation Agent 首先取得缺失
的文件名，根据请求推导文件的用途，搜索项目知识，并依据第一范式（1NF）、第二范式（2NF）
和第三范式（3NF）分析其结构。它找出为消除重复组、部分依赖或传递依赖所需要的每一个辅助
索引文件。对于每个 ID 字段，它请开发者选择 **UUID** 或提供确切的 COBOL **PIC** 定义；
智能体从不凭假设去选择 ID 的表示方式。缺失的决定会产生一次澄清，而不是一次文件变更。

准备、提议或规范化这份模式交接，属于 Documentation Agent 的分析，不是索引文件变更。只有
真正的 `indexed_file.write` 或明确的 `.cidx` 保存，才是保留给 Data (Indexed File) Agent
的变更。

在那份模式交接通过 Documentation Agent 的 Pedantic 评审之后，Grace 把每个定义委派给
**Data (Indexed File) Agent**。这个专家只能通过受管控的 `indexed_file.*` 工具来列出、
检查和写入索引定义，这些工具以与 Indexed File 界面相同的模型为后盾。一次成功的写入会验证
记录和键、保存 `.cidx`、重新生成索引的 COBOL 和 copybook、只在被分配的数据文件尚不存在
时才初始化数据，并刷新项目的 Indexed Files 树。已有的索引数据在模式维护期间绝不会被截断。
每个辅助关系都是一个单独的定义。已定稿的定义保持 Indexed File 界面的结构锁；开发者必须
先在界面中明确地取消定稿，智能体才能更改它的模式。每个结果在 Grace 报告完成之前，都必须
通过 **Data (Indexed File) Agent Pedantic Reviewer**。

**Grace 能看到菜单的什么，以及她能改动什么。** SideMenu 或 MenuBar 的条目不是该控件的
属性：它们是你在它的菜单编辑器中构建的树，保存在窗体旁边的 `<control id>.menu.yaml`
中。Grace 和各专家会看到那棵树，逐个条目，连同每个条目的 id 和动作，所以像“*Samples ▸
Responsive Layout ▸ Responsive: grid* 打开哪个窗体？”这样的请求，是根据你真实的菜单来
回答的。

他们也可以改写它。要求“把 *Responsive Layout* 的条目按 A–Z 排序”，或“把 *Charts* 移到
*Samples* 之下并改名为 *Dashboards*”，Form Designer Agent 就会在一次操作 `set_menu_items`
中发送菜单的**整棵新树**。你像对待任何其他更改一样在预览中评审它，而一次 **Undo** 就能
让菜单文件恢复到原来的样子。

- **智能体保留的条目就是同一个条目。** 它的 id 不会变，所以拿 `SelectedItemId` 与它比较
  的处理程序仍能工作。智能体添加的条目会从 IDE 获得一个新的四字母 id，就像菜单编辑器会
  给出的那样。
- **智能体没有提到的东西保持不变。** 图标、徽章、快捷键或条目所承载的窗体，除非智能体
  设置了它，否则保持其值。
- **新树中缺失的条目会被移除。** 结果会列出每一个，连同窗体上仍然点名其 id 的每个处理
  程序。那些代码从不被更改或删除：它是你的。
- **IDE 会拒绝菜单编辑器不会允许的东西。** 嵌套超过三层的树、被使用了两次的 id、不存在
  的动作、指向一个不是 *Embedded* 或 *Both* 的窗体的 `open-form:` 条目、指向一个不是
  *Standalone* 或 *Both* 的窗体的 `open-standalone-sync:` / `open-standalone-async:`
  条目。被拒绝的操作会被跳过，变更集的其余部分照常应用，预览会说明是哪个条目以及原因。
- **程序在运行时添加的行不在文件中**（`AddItem` 和其他行方法），智能体从不碰它们。

**各专家执行它们的工具。** 在 Grace 之下，智能体不只是描述工作——它们会把它做完，但只
通过受管控的、有证据的渠道。智能体只可以调用授予它的工具（它的 `mcp.json` / 能力）；
未声明的或捏造的工具会被视为导致任务失败的严重缺陷。当 **Form Designer Agent** 的工作被
它的严苛评审同伴*批准*时，它的结果会通过你手工使用的那同一条经评审的预览/应用路径，作为
**一次可撤销的更改**应用到打开的窗体上——绝不会悄悄地改写窗体。Form Designer 也可以
*查看*实时的窗体（已渲染控件的只读视图）来检查它的工作；它从不通过驱动界面来编辑。
**Version Control Agent** 只在**你打开的项目的仓库内**运行真正的 Git（绝不是
PowerRustCOBOL 自己的）：日常的本地操作（status、diff、log、add、commit、branch、checkout、
stash）自行运行，而任何触及网络或改写历史的操作——push、fetch、pull、rebase、
`reset --hard`——都会**暂停以等待你明确的批准**，并在运行之前向你显示确切的命令。每次
工具调用，连同其真实的输出和退出状态，都被记录在工作流记录中；失败的命令会被报告为失败，
绝不会被粉饰成成功。

**Test connection** 按钮会向你的端点发送一个很小的请求，并报告模型是否可达、密钥/模型是否
被接受——在依赖这份设置之前，用它来确认。当这个提供商提供它时，请求发给正在使用的模型，
否则发给提供商的第一个模型，而答案会点出那个模型。回答 **402 Payment Required** 的提供商
已经接受了你的密钥，只是拒绝了那个模型——通常是免费套餐之外的付费模型——所以测试会把
连接报告为良好，并说明该模型不在你的套餐中：请选择另一个模型，或向提供商充值。一旦
**Endpoint URL** 和 **Model** 都已设置，助手就可用了。清空端点就能再次隐藏它。

**使用它。** 打开一个 COBOL 文件，在提示栏中输入一个请求（例如 *"add a paragraph that
totals WS-LINES and DISPLAYs it"*），然后按 **Send**。模型按这个顺序接收：

1. 你的**标准系统提示**；
2. *这个文件*的**对话历史**（它会在多次会话之间保存，按每个源文件）；
3. 你的**请求**，连同该文件的**当前源码**。

回复到达时，PowerRustCOBOL 会从中提取 COBOL，并**就地更新编辑器缓冲区**——所以你可以
立即评审、微调、运行，或像任何其他编辑一样撤销（Ctrl/Cmd-Z）这个结果。运行中的对话记录
显示在提示栏下面（💬），而 **Clear conversation**（🗑）会忘记该文件的历史。只读的 Generated
Code 绝不会被修改。

**检查器中也有。** 同样的提示栏出现在窗体/控件内联检查器的上方，以窗体的**生成的 COBOL**
作为它的（只读）上下文——便于询问如何为事件处理程序接线。由于生成的代码从不手工编辑，
那里的回复显示在对话记录中供参考，而不会被应用。

**对话保存在哪里。** 历史*不*保存在隐藏的缓存里——它保存在项目的 `data/` 文件夹中，
位于 PowerRustCOBOL **自己的索引（ISAM）文件**（`data/conversations.dat`）里，正是你的
COBOL 程序所用的 `ORGANIZATION IS INDEXED` 格式，以源文件的相对路径为键。（我们自己
吃自己的狗粮，使用自己的运行时。）因此，对话随项目一起走，并且需要打开项目才能持久化；
没有项目时，助手仍然可用，但只限于当前会话。

```mermaid
sequenceDiagram
    participant Dev as 开发者
    participant Ed as 代码编辑器
    participant LLM as 你的大语言模型
    Dev->>Ed: 输入一个请求，按 Send
    Ed->>LLM: 系统提示 + 历史 + 请求 + 当前源码
    LLM-->>Ed: 回复（代码块中的 COBOL）
    Ed->>Ed: 用返回的源码替换缓冲区
    Dev->>Ed: 评审 / 调整 / 运行 / 撤销
```

<!-- 📷 ide-ai-assistant.png — The code editor with the AI prompt bar visible above it and an expanded conversation transcript. -->
<p align="center"><img src="../assets/images/screenshots/ide-ai-assistant.png" alt="代码编辑器，其上方可见 AI 提示栏，并展开了对话记录" width="900"></p>

> **隐私说明。** 你的提示、对话历史和**打开文件的完整源码**，会被发送到你所配置的任何
> 端点。请只把它指向你信任的模型。
### 当处理程序失败时（`onUnhandledException`）

事件处理程序内部的 COBOL 失败**不会**关闭你的窗体。失败的处理程序被放弃，事件循环
继续处理下一个事件，所以一条糟糕的路径不会让操作员失去屏幕上的一切。

在窗体上绑定 **`onUnhandledException`** 来接管控制。详细信息会作为窗体自身的
**`LastException`** 到来：

```cobol
       PROCEDURE DIVISION.
           SET Lbl-Status::Caption TO me::LastException
           DISPLAY "handled: " me::LastException.
```

什么都不绑定，操作员看到的就是一条**严重通知**：

> A critical exception has occurred: &lt;details&gt;. Implement the event handler
> onUnhandledException to get better control over the exception.

它永不过期，并带有用来关闭它的 ✕，而且窗体上不需要 Snackbar 控件。它总是显示**完整的**
消息，无论多长：它会增长以适应，而不是用“…”把结尾截掉，因为结尾通常才是告诉你该怎么
做的部分。

**未受保护的大小错误是一个异常。** 当结果放不下时——包括除以零——`COMPUTE`、`ADD`、
`SUBTRACT`、`MULTIPLY` 和 `DIVIDE` 会引发 SIZE ERROR 条件。声明 `ON SIZE ERROR`，它就
归你处理：

```cobol
           DIVIDE WS-A BY WS-Z GIVING WS-A
               ON SIZE ERROR DISPLAY "cannot divide by zero"
           END-DIVIDE
```

什么都不声明，就没有人在处理它，所以该语句会引发一个异常，而不是悄悄地让接收项保持
不变——这正是一个错误的合计在没有任何迹象表明出了问题的情况下进入报表的方式。围绕该语句
的 `TRY … CATCH` 会像捕获任何其他异常一样捕获它；没有 `CATCH` 时，它会到达
`onUnhandledException`。

> ⚠️ 在 `onUnhandledException` **内部**引发的异常不会再交回给它——那会造成循环。它会像
> 任何其他失败一样被报告。
>
> 这仅适用于窗体，上面的大小错误规则也是如此。控制台程序失败，仍然是对它的调用方失败——
> 它没有可供报告的窗口——而那里未受保护的大小错误保持标准的沉默，因为缺少该短语时，
> COBOL-85 让结果保持未定义，而 CCVS85 测试套件依赖于被允许继续往下运行。
### 示例项目（Help ▸ Examples）

**Help ▸ Examples** 为每个示例项目列出一个条目：

- **Help ▸ Examples ▸ PowerChat** 会打开一个聊天机器人，它既支持基于你的文档的 RAG
  （检索增强生成），也支持对索引文件中数据的实时分析。它 100% 用 RustCOBOL 构建，并且
  可以轻易地改造并作为方案出售，用来把生成式 AI 带给 COBOL 应用程序。见下文
  *PowerChat——可以照抄的聊天机器人*。
- **Help ▸ Examples ▸ PowerDemo3** 会打开这样一个项目：每个工具箱控件各有一个演示窗体——
  每个控件都已接线并在运行，它的 COBOL 就在旁边。这是看清一个控件究竟如何被驱动的最快
  办法。
- **Help ▸ Examples ▸ PowerSpatial** 会打开一个 Spatial 主题的智能家居仪表盘：无边框的
  玻璃窗口、一个工具栏、一条侧栏和一个房间栏停靠在仪表盘周围并与它作为一个整体一起移动，
  还有一个恒温器表盘、一个音乐播放器，以及保存在一个设置里、由各窗体通过 Common Code
  共享的六种语言。对于一组浮动窗口，它就是该照抄的示例。
- **Help ▸ Examples ▸ PowerAnalytics** 会打开一个 Spatial 主题的基金分析仪表盘：43,750 只
  投资基金从一张电子表格读入一个内存中的 SQLite 数据库，玻璃面板一个接一个地放大，图表
  从零开始生长（条形图、折线图、环形图和 RadarChart，每个最多三个系列），带有可边输入边
  提示的列表、其条目可以勾选的搜索字段，范围过滤器，一个带数据网格的 Dashboard / Data
  菜单，以及一个可以一路适配到手机宽度的布局。对于数据仪表盘，它就是该照抄的示例。
- **Help ▸ Examples ▸ PowerCloud** 会打开一个窗口，每项 AWS 服务一个标签页：DynamoDB
  （带照片的宠物）、S3、Lambda、Polly（带播放器的文字转语音）、EC2（带开/关列的服务器）
  和 Rekognition（在照片上框出的人脸），还有一个 Settings 标签页，它接收一个 AWS 用户的
  密钥，并部署该演示所需的 CloudFormation 模板。对于与云端对话的应用程序，它就是该照抄的
  示例；见 AWS 一章中的*示例：PowerCloud*。

IDE 会自己找到每个项目，所以你不需要知道它放在哪里：在已安装的构建中，它在可执行文件
旁边；当你从源码运行时，则在构建该 IDE 的那棵树里。如果你把副本放在别处，请把
`PRC_EXAMPLES_ROOT` 指向另一个副本。当构建没有附带某个示例时，该条目会变灰，悬停时给出
原因。从只读安装中打开其中一个，会先把它复制到你的 Documents 文件夹中的
`PowerRustCOBOL Examples`，以便能够构建它。你已经有的副本会按原样打开，绝不会被覆盖。

> **注意。** 打开它会替换你当前打开的项目，与 *File ▸ Open Project* 完全一样。请先保存你的
> 工作。

#### PowerChat——可以照抄的聊天机器人

`examples/PowerChat` 是第二个完整的应用程序，而不是一个陈列馆：一个聊天机器人，每次
围绕**一个主题**，根据该主题自己的文档，通过该应用程序的用户所选的模型来回答问题。把
它复制过去，开始你自己的项目。它的十四个窗体里的一切都是 COBOL：

| 窗体 | 它向你展示如何做什么 |
|---|---|
| `chat-form`（主窗体） | 一个 SideMenu 外壳，其菜单在菜单编辑器中设计，用 `SetItemLabel` 按当前语言重新标注标签，并用 `SetItemEnabled` 保持关闭，直到某个智能体有了模型（而 **Chat** / **New conversation** 则要等到存在一个主题）；**New conversation** 通过 `pick-form` 询问要谈哪个主题，在其中开始一次全新的对话，并显示聊天（`ActivateItem("chat")`）；当该主题既没有文档也没有数据文件时，编排者的系统提示会告诉它：在添加之前，要说明它无法回答关于用户数据的问题；模型得到的每一条指令都来自**主提示**（在 `prompts-form` 中编辑）：编排者的系统提示是它的 `SYSTEM` 部分，其中填入了该主题的提示、无来源说明和报告模板列表，而其他智能体得到的是它的 `ASSISTANT` 部分；报告由一个**报告模板**构建——编排者列出 `data/templates.idx` 中的模板（首次由 `samples/report-templates.txt` 填充：Executive，那个稳重的，以及十种信息图类型——Informational、List、Timeline、Comparison、Map、Statistics、Flowchart、Hierarchy、Anatomical、Animated——每个都是一个页面，宽度为聊天宽度的 80 %，最高 1600 px，除非用户要求别的；与早先版本发布时完全一样的已存储模板（依 `samples/report-templates-previous.txt`），会在聊天启动时升级，而用户改过的则保持不变），推荐合适的两三个，并在用户选定之后回答 `TEMPLATE: <name>`，程序则以该模板的 HTML 页面和 `TEMPLATE` 部分作答；用户修改或描述的模板会在一个隐藏的 `<!--REPORT-TEMPLATE … -->` 块中返回，并被保存，以该回答的页面作为它的骨架，然后从回答中剪掉（有多个智能体时，规划者在尚未选定模板时回答 `ASK:` 而不是拆分问题，选定之后则在第一行回答 `TEMPLATE:`）；一个在运行时添加的 **Getting started** 行（`AddItem`）会打开欢迎窗体，而首次运行时，程序用 `ActivateItem` 自己打开它。其他每个窗体都由其行的 `open-form:` 动作以**嵌入**方式在 ContentPane 中打开（RAG 设置对话框除外——它们是模态窗口），当操作员回来时，`onActivate` 会刷新聊天；一个充当聊天的 `Viewer`；三个 `AgentObject`，它们选出一个编排者并分摊工作（见下文）——最多 12 个小任务，排队分给那些没有在编排的智能体，每个结果保存在 `data/taskresults.idx` 中；计划行 `JOIN:` 按所写的原样、按任务顺序给出各部分，失败的任务会在回答的末尾被点名，而不是被丢弃，并以带有 `AllowKnowledgeBase` 的 `KnowledgeBase` 为依据；作为运行时菜单行的对话；来自 `LastInputTokens` / `LastOutputTokens` 的令牌合计 |
| `welcome-form` | 首次运行的界面——84 点的名称、一个机器人、四个步骤——作为一个独立的窗体，而不是隐藏在聊天之上的控件 |
| `topics-form` | **CRUD 模式**（见下文）：DataGrid 中的主题，每行带有打开、编辑和删除的图标按钮，每个主题带有它自己的 Knowledge Base 集合（保存时 `CreateCollection`，删除时 `RemoveCollection`）；示例主题从一份纯文本列表安装，也可以再被移除 |
| `documents-form` | 该集合的文档，作为用 `AddNode` 构建的 `TreeView`，它的文件夹保存在一个索引文件中，所以空的文件夹也会显示；`onNodeSelect` 把 `FileDropZone` 指向所选的文件夹；`Refresh()`，以及一个进度条——始终在它自己的位置上，空闲时为空——由 `onProgress` / `onIndexed` 驱动 |
| `settings-form` | 一个摘要，不是编辑器：四个分组按钮——**Knowledge Base folder**、**Model providers**、**Model selection**、**Agents**——每个按钮下面有一行说明它所容纳的内容，另有模型列表、Export/Import 和状态行。每个按钮都以**模态对话框**打开它的分组（`INVOKE ME::"OpenFormSync"("FORM-ID")`），而对话框一关闭，摘要就会重新读取这些文件。**Model selection** 和 **Agents** 在至少存在一个提供商连接之前保持禁用。整个设置以 XML 导出和导入——导出从不写入密钥，而导入会用 `COBOL::"KEY-SET"` 存储手工添加到某个 `<model>` 上的任何 `key="…"`。每次更改之后，它都会用 `INVOKE super::"PC-REFRESH"()` 请求主窗体重新检查它的菜单 |
| `kb-folder-form` | KB 文件夹，手工输入或用 `COBOL::"FOLDER-DIALOG"` 选取；**Save** 和 **Cancel** 在字段的上方和下方各有一对，两对都调用同一个 `PC-SAVE` / `PC-CANCEL` |
| `providers-form` | 对话框中的 **CRUD 模式**：网格中的各连接（名称、提供商、端点、模型、是否设置了密钥）；Create/Update 包含名称、ComboBox 中的 IDE 提供商（`COBOL::"PROVIDER-COUNT"` / `"PROVIDER-GET"`）、端点和 API 密钥——用 `COBOL::"KEY-SET"` 存储，且不再显示——以及 **Test connection**，它向提供商询问其模型列表（`COBOL::"MODEL-LIST"`）。删除一个连接也会清除使用它的智能体 |
| `model-form` | 打开它就会**连接**：立即向第一个连接的提供商询问它的模型，而挑选另一个连接会再问一次；模型、它是否调用工具以及它的排名会保存到该连接上，并用 `COBOL::"MODEL-SET"` 移交。顶部和底部各有 Save/Cancel |
| `agents-form` | 每个智能体一个 ComboBox——**(off)** 或一个已保存的连接；顶部和底部各有 Save/Cancel |
| `prompts-form` | 针对某个主题的系统提示的各个版本的 **CRUD 模式**，最新的在前，当前生效的带标记；每行一个提升（promote）图标按钮，在确认之后让某个版本成为生效的提示，而生效的版本不能删除。**Main prompt** 把它切换到每个主题共享的提示——模型得到的每一条指令，用英文，分成命名的部分（`=== SYSTEM ===`、`ASSISTANT`、`NO SOURCES`、`PLAN`、`TASK`、`COMPOSE`、`TEMPLATE`），带有聊天填入的 `{…}` 词——保存在同一个版本文件中，主题 id 为 `*MAIN`；**Restore default** 把 `samples/main-prompt.md` 保存为新版本。各版本已从 `prompts.idx` 迁移到 `prompt-versions.idx`（32,000 字符的文本），并在第一次时被复制过去 |
| `files-form` | 某个主题自己的索引文件的 **CRUD 模式**，用 `RegisterFile` 按路径注册：每个文件在保存时都会被尝试一次，所以缺失的文件，或与它不匹配的 `.cidx`，会当场被拒绝并给出原因；聊天窗体在打开时注册该主题的文件，并点出它无法使用的任何文件 |
| `confirm-form` | 任何窗体都可以提出的一个是/否问题：调用方用 `ME::"SetProperty"` 在自己身上设置 `ConfirmText`（以及翻译过的 `ConfirmYes` / `ConfirmNo`），用 `OpenFormSync` 打开它，并在它返回时读取 `ConfirmAnswer`。不设置时，问题和两个按钮显示该对话框自己的“Are you sure?”、“Yes”和“No”，用的是用户的语言 |
| `pick-form` | “从这些中选一个”，方式相同：调用方设置 `PickItems`（每行一项）、`PickSelected`（1 = 第一项）以及翻译过的 `PickTitle` / `PickOk` / `PickCancel`，用 `OpenFormSync` 打开它，并读取 `PickAnswer`——所选的项，1 = 第一项，取消时为 0。不设置时，标题和按钮显示该对话框自己的“Choose one”、“OK”和“Cancel”，用的是用户的语言 |
| `preview-form` | 原样显示的文档，方式相同：调用方在自己身上设置 `PreviewPath`（文件）、`PreviewTitle` 和翻译过的 `PreviewClose`，并用 `OpenFormSync` 打开它；该窗体用 `super::"GetProperty"` 把它们读回到一个 `Viewer` 中，后者原样显示 Markdown、文本、HTML、PDF 和图像，并以文本形式显示 Word、PowerPoint、Excel 和 OpenDocument 文件。没有任何东西返回——它只是显示 |

**CRUD 模式。** 每个保存记录列表的窗体——主题、数据文件、提示版本、提供商连接——都用
同样的方式构建，所以操作员只需学一次：

- 一个带两个页面的 **TabControl**，**Browse** 和 **Create/Update**，由 COBOL 用
  `MOVE 2 TO Tab-Crud::SelectedTab` 来翻页；
- 在 **Browse** 上，记录的 **DataGrid** 上方有一个 **New** 按钮；每行以两个图标按钮
  结尾，一支铅笔和一个垃圾桶（Button 列中的 `icon:pencil`、`icon:trash`），而
  `onCellClick` 读取 `ClickedRow` / `ClickedColumn` 以得知是哪一行的按钮；表头用
  `SetColumnTitle` 跟随语言；
- 在 **Create/Update** 上，字段位于两对相同的 **Save** 和 **Cancel** 按钮之间，一对在
  顶部，一对在底部；**New** 打开时为空，铅笔打开时加载该行；
- **Save** 写入记录——`WRITE`，遇到 `INVALID KEY` 则 `REWRITE`——并返回刷新后的
  **Browse**；**Cancel** 返回时不刷新；
- **Delete** 先通过 `confirm-form` 询问。

没有任何控件会被隐藏在另一个之上、再由某个条件换进来：交替出现的东西各自放在独立的
窗体里。

它的模型是应用程序的（上文运行时的模型列表）：它自己的模型文件镜像该列表，并加上只有
PowerChat 才知道的每个模型的信息——它是否调用工具、它编排得有多好——并且在第一次运行
时，把它先前保存的模型移交给应用程序。

它自己的数据——设置、主题、对话、它们的回合、那个模型文件、已注册的文件、提示版本、
文档文件夹——是八个 `STORAGE MODE IS DISK` 的索引文件，以 `I-O` 打开（第一次用
`OUTPUT`），并在每次更改时提交。设置 `POWERCHAT_DATA` 可以把它们保存在 `data/` 之外的
某处。项目的 `README.md` 逐步讲解了第一次运行。

**三个智能体，一个回答。** 聊天窗体的三个智能体各自从 RAG 设置中得到一个模型，而每个
模型条目都说明它是否**调用工具**以及它的**编排**能力有多好（从 1 到 9 的排名）。只分配了
一个智能体时，它独自作答。有更多智能体时，它们每个会话举行一次选举。赢得选举的智能体
会**留任**，直到选举所依赖的东西发生变化——某个智能体的模型、它的*调用工具*标志或它的
排名，或者某个智能体被分配的是哪个条目——然后它们再次选举（`onModelChanged`）；打开
另一个主题或编辑提示，从不会改变它们：

- 模型**不能调用工具**的智能体来编排，如果有的话——有几个时，取其中排名最高的；如果
  每个智能体都能调用工具，则排名最高的来编排。平局归第一个智能体。如果每个智能体运行的
  是同一个模型，就随机挑一个，随机数发生器以时钟作种子，所以一次启动并不总是选出与上
  一次相同的智能体。
- 工具类的工作交给没有在编排的、排名最高且能使用工具的智能体。只有一个模型能调用工具
  时，它做工具类的工作，而另一个智能体来编排，即使那一个排名更高。没有能使用工具的
  智能体，就没有工具工作者，聊天会在不调用工具的情况下运行，而不是失败。
- 只有编排者得到该主题的系统提示。其他智能体保留设计在它们身上的角色提示，而只有工具
  工作者会被提供 Knowledge Base——其余的以 `ToolProtocol = None` 运行。

随后，一个问题要经过三个阶段，全在该窗体的 COBOL 中。编排者把它拆成 `TASK:` 行，其他
智能体同时处理它们，而编排者根据它们的结果撰写回答。值得读的过程是 `PC-ELECT`、
`PC-DISPATCH`、`PC-COMPOSE` 和 `PC-ON-REPLY`。

**示例主题。** Topics 窗体上的 *Install sample topics* 读取 `samples/samples.txt`——
每个主题、文档和数据文件一行——并据此构建三个示例主题：Human Resources 和 Legal 根据它们
的政策文档作答，而 Orders 还会搜索一个按路径注册的小型订单文件。文档被逐个导入，每次
导入都由前一次的 `onIndexed` 启动。*Remove sample topics* 恰好移除它所创建的主题（每个
都在它的记录中被标记）、它们的集合（被移到一边，绝不删除）、它们已注册的文件和它们的
提示版本；`samples/` 中的文件只会被读取。设置 `POWERCHAT_SAMPLES` 可以从另一个文件夹
安装。添加一个示例只需要一个文件夹和几行文本——不需要 COBOL。

**六种语言，立即切换。** 菜单页脚中的六面旗帜是带 `onClick` 的 `PictureBox`；一次点击
会保存这个选择并当场重新标注该窗体，而其他每个窗体都用它打开。文本存在于 COBOL 中，
而不在 IDE 里：每个窗体在它自己的 WORKING-STORAGE 中带有一张翻译表——每个文本一行，
每种语言一个 `FILLER`——由一个 `REDEFINES` 覆盖，所以语言只是一个列号。一个过程把那一列
复制到具名的数据项（`T-SEND`、`T-TOPIC-CREATED` ……）中，处理程序在以前写字面量的地方
使用它们，并把设计好的标题和提示移回它们的控件上：

```cobol
       01 PC-TEXT-DATA.
      *>   SEND
          05 FILLER PIC X(80) VALUE "Send".
          05 FILLER PIC X(80) VALUE "Enviar".
          ...
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA.
          05 PC-TEXT-ROW OCCURS 23.
             10 PC-TEXT PIC X(80) OCCURS 6.
      ...
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 23
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-SEND) TO Btn-Send::Caption
```

带有数字的消息是一个模式——`"Removed &1 sample topic(s)."`、
`"サンプルトピックを &1 件削除しました。"`——`PC-FMT` 把这些值放在每种语言想要的位置，
因为词序在六种语言中并不相同。

> ⚠️ **注意事项。**
> - 在菜单文件中设计的菜单行保留它的标签：`SetItemLabel` 只改变在运行时添加的行。
>   因此 PowerChat 用 `AddItem` 添加它所有的菜单行，使它们跟随语言。
> - 窗口的标题在窗口打开时设置。PowerChat 的窗口全都叫 *PowerChat*，而每个窗体的标题
>   标签带着它的名称。
> - `LENGTH OF item` 不是 COBOL-85——请使用 `FUNCTION LENGTH(item)`。
> - 只有界面被翻译了。PowerChat 发给模型的指令保持英文，而模型用向它们提问时所用的语言
>   作答。

> **注意。** 尚待完成：把文档做成文件夹树。
>
> ⚠️ **注意事项。** 紧跟在 `MOVE` 之后、写成语句的方法调用，会被读作该 `MOVE` 的又一个
> 接收字段——`MOVE A TO B` 后面跟着 `LIST-1::AddItem(X)`，就变成了
> `MOVE A TO B LIST-1::AddItem(X)`，这会使程序停止。尽可能用句点结束 `MOVE`，或者把
> 调用写成 `MOVE LIST-1::AddItem(X) TO WS-IGNORED`，PowerChat 自始至终就是这样做的。

📷 需要截图 — `powerchat-chat.png`：PowerChat 的聊天窗体，已打开一个主题，对话中有一个
问题及其回答，菜单中列出了过往的对话。
### 在 IDE 中阅读文档（Help ▸ Documentation）

**Help ▸ Documentation** 会打开一个专用窗口，渲染这份指南和 PowerRustCOBOL 的其他手册——
包括它们的 **Mermaid 图表**和**截图**，内联绘制（用纯 Rust 渲染，不需要浏览器）。文档
随 IDE 一起打包，所以离线也能工作；`Cmd+O` 也能打开任何本地的 Markdown 文件，并能在它
旁边找到它的图像。

窗口左侧是可搜索的**文档列表**，右侧是渲染后的文档，另有一个**图标工具栏**和
**File / View / Help** 菜单。文档内**搜索**会高亮匹配项（黄底蓝字）；按 **Go** 或
**Enter** 跳到第一个匹配项，用 **◀ / ▶**（或 `,` / `.`）在它们之间逐个跳转，并带有实时的
`n/total` 计数器。**目录**是可点击的——侧边的**大纲**和文档内的 `[…](#…)` 链接都会跳到
它们的章节。**在文档中移动**的方式就是文档应有的样子。**方向键**滚动它：轻点一下移动一行，
按住则以同样的阅读节奏开始，并加速到它的四倍，所以可以在不松手的情况下读过一本长手册。
`PageUp` / `PageDown` 一次移动一屏，`Home` 和 `End` 跳到两端。你也可以**用鼠标抓住页面并
甩出去**——按下、拖动、松开，它就会滑行直至停止。抓取必须从文档上开始，但从那里起，手势
就归你了：拖动会跟随指针去往任何地方，**你可以在屏幕上的任何位置松开**——在工具栏上、
文档列表上，或窗口之外——页面仍然会飞出去。在你的手已经静止时松开，它就停留在你放置的
地方；用一次按下去接住一个正在移动的页面，它会立刻停住。（插入符位于搜索框中时，方向键
属于搜索框，所以它们在那里是输入，而不是滚动。）

长手册保持响应灵敏，是因为窗口只排布你正在看的那一部分，并提前在两侧各保留几屏备用，
也因为图表和截图在你选择文档的那一刻就在**后台线程**上解码——远在你滚动到它们之前。
仍在准备中的图像会在它的位置上显示一个占位符。

你还可以得到一个可调节的**字号**，它会*在多次会话之间被记住*，还有缩放、全屏、保持置顶
（`⌘T`）、打开本地的 Markdown 文件（`⌘O`），以及一个查看源码的模态框（`⌥⌘U`）。
**Print**（`⌘P`）会把文档——包括 Mermaid 图表——导出为 PDF，并在你的操作系统查看器中
打开它，在那里系统打印对话框只需一次点击。该窗口是一块半透明的**磨砂玻璃**面板，并跟随
IDE 的主题和语言。

每本手册都以全部六种界面语言、各自一个文件的方式交付，而列表**每本手册只显示一行**——
你所选语言的那一份。在翻译尚未写成的地方，那一行会回退到英文文本而不是消失，所以无论
你用哪种语言阅读，列表的长度都一样。

### 导览（Walkthrough）

在新机器上第一次打开项目时，IDE 会让自己变暗，并逐个介绍它的六个主要部分：**Project
settings**、**Forms**、**Indexed Files**、**Assets**、**Knowledge Base** 和 **Output
面板**。每一步都会点亮它所描述的组件，并朝它指出一个对话气泡，所以窗口的哪一部分是所指，
绝不会有疑问。

用 **Next** 和 **Back** 在其中前进，随时用 **Skip** 或 `Esc` 离开。它显示期间，IDE 中
其他任何东西都不响应——这是刻意的，这样误点一下就不会把它半关掉。

它**每台机器只运行一次**，而不是每个项目一次：它描述的是 IDE，而你只需要学习一次 IDE。
无论你怎样离开它——完成、Skip 或 `Esc`——它都不会自行再回来。

> **重播它。** **Help ▸ IDE Walkthrough seen** 是一个勾选框，显示你是否已经走过一遍。
> 清除勾选，导览就会立即重新开始。没有打开项目时，该条目会解释说需要先有一个——它所指
> 向的六个部分中有五个是项目树中的节点，在加载项目之前它们并不存在。

导览从不重新排布任何东西。它会滚动项目树，让它所描述的部分可见，但不会展开分类、打开
窗体，或改变你屏幕上原有的东西。它结束时，你就恰好处在你离开的地方。

📷 需要截图 — `walkthrough-step.png`。在一台尚未运行过导览的机器上打开一个项目（或清除
**Help ▸ IDE Walkthrough seen**），并截取第 2 步——指向 **Forms** 的那一步——使变暗的
IDE、被点亮的树行和气泡的尾巴都出现在同一帧中。

### 任务向导（Task Wizard）

Walkthrough 告诉你*什么在哪里*。**Task Wizard** 则带你在真实的 IDE 中，一步一步地*做*
某件事。它看起来像 Walkthrough：IDE 变暗，一个部分保持点亮，一个气泡说明在那里要做什么。
区别在于，被点亮的部分是**活的**。你自己完成这一步，当 IDE 显示它完成了，向导就继续前进。

**打开它。** **🗺 Task Wizard** 开关位于 IDE 工具栏的右端，紧挨着语言选择器。同一个开关
也在每个 Form Designer 的工具栏中，位于 Magnifier 之后，以及 **Help ▸ 🗺 Task Wizard**
中。三者显示相同的状态，而 IDE 会在多次会话之间记住它。

- 打开开关，会在它之下打开任务列表。
- 开关旁边的 **▾** 会再次打开列表。如果向导是关闭的，▾ 会一次点击就把它打开并展开列表。
- 关闭开关会停止一个任务，而不会丢失你所在的位置。

**这些任务：**

| 任务 | 它带你走过什么 |
|---|---|
| Configure a model provider | Project settings → **Models Manager…** → 提供商 → 它的端点 → 它的 API 密钥和 **Test connection** → **Save** |
| Connect an agent to a model | **Agents Manager…** → **Agent × Model** → 提供商范围 → 每个智能体那一行上的模型 → **Save** |
| Bind a control to data | 选中控件 → Properties 中的 **Data Binding** → 数据源 → 它的字段 → **Apply** |
| Set up a DataGrid | 绑定它（或跳过）→ **Edit DataGrid settings…** → 列和宽度 → 关闭；未绑定的网格在画布上继续（双击它，添加一列） |
| Fill a TreeView | 它的 **Nodes** 框 → 行为选项 → **Events** 标签页 |
| Build a SideMenu | **Edit Menu…** → 添加一个条目 → 它做什么 → **Save** |
| Build a menu | 对 MenuBar 相同，外加一个快捷键 |
| Add an animation | **Anim** 标签页 → **➕ Add** → 触发器和效果 → **▶ Preview** |
| Set up a map | 它在哪里打开（*Start latitude*、*Start longitude*）→ *Start zoom* → 来自一张表的图钉（可选）→ 它的 **🎨 Colors** → **Events** 标签页，它还说明了要从代码调用什么，以及哪些查找需要 Google Maps 密钥 |

每个任务都被列为*新的*、*进行中*（针对打开的项目）或*已完成*。你可以开始它、继续它，或
重新开始。需要窗体的任务，一开始会请你先打开一个。

**在一个任务中前进。**

- 当你完成了一个步骤，它会自行结束。**Next** 始终在，用来在不做这一步的情况下继续；
  末尾的总结会列出你跳过了什么。
- 如果你到达某一步时它已经完成，气泡会这样说，而 Next 就是该按的按钮。
- **Back** 回到前一步。**Exit** 或 `Esc` 把任务留在原地，而 **Resume** 会把你带回那一步。
  `Esc` 总是先离开最内层的东西：先是文本字段（网格单元格的编辑像往常一样被取消），然后是
  展开的列表，然后是画布上网格的编辑模式。只有在没有任何东西可离开时，`Esc` 才会离开
  任务。
- 在某一步显示期间，只有 IDE 中被点亮的部分接受点击和输入。IDE 的快捷键要等到任务结束
  才生效。从被点亮的部分展开的列表或菜单，即使悬在变暗的区域之上也能工作。IDE 在这一步
  期间打开的窗口，例如错误或确认，永远不会被变暗。
- 如果你关闭了某一步所在的窗口，或切换到另一个标签页，向导会点亮能把它重新打开的东西，
  并说 **Reopen…**。它从不指向虚无。
- 当某一步在 Form Designer 窗口中时，其他每个窗口都会整个变暗，并带有一条说明任务在哪里
  继续的注释，以及一个把那个窗口带到前面的按钮。

**替我做。** 在少数几步上，气泡会提供 ★ **Do this for me**。它通过同样的代码，做按钮或
菜单会做的完全一样的事：

- 显示 Project Settings；
- 用提供商的默认值填写一个空的端点；
- 添加一个网格列（一次 Undo 即可撤回）；
- 往一个空菜单中放入一个起始条目：对 SideMenu 是 *Home*，对 MenuBar 是 *File*、*Edit*
  和 *Help*，用 IDE 的语言；
- 添加一个动画（一次 Undo 即可撤回）。

只有当它所填充的东西还是空的时，它才会被提供，所以它绝不会覆盖你写的任何东西。它从不做
会连接到提供商、保存了却无法撤销，或替你输入你自己的内容的那种步骤。

**提议。** 开关打开时，选中单个 DataGrid、TreeView、SideMenu、MenuBar 或 Maps 控件，会
在它旁边显示一个小标签，提供该控件的任务。它只是提议。它的 **✕** 会为那一个控件把它
隐藏。

**你的密钥仍归你。** 向导从不读取、显示、存储或记录 API 密钥。密钥那一步在
**Test connection** 通过时完成，而向导对密钥别无所知。

**它把东西保存在哪里。** 向导记住的一切都在这台机器上，与 IDE 的语言在同一个文件里。这
包括开关、你完成了哪些任务、每个项目未完成的任务进行到哪里，以及你忽略了哪些提议。没有
任何东西被写入你的项目。

📷 需要截图 — `task-wizard-step.png`。打开 Task Wizard，开始 **Configure a model provider**，
并截取端点那一步，使变暗的 IDE、被点亮的 Model Providers Manager、端点字段周围的圆环，
以及带有 **★ Do this for me** 的气泡都出现在同一帧中。

### What's going on——公告

PowerRustCOBOL 团队会不时发布一则公告：一个新版本、一项值得知道的变化、一次活动。IDE
把它显示在一个标题为 **What's going on** 的窗口中，占屏幕的 70 %。

- **何时。** IDE 在启动时查找一次，运行期间每六小时查找一次。它会等到屏幕上没有首次运行
  对话框、AI 设置邀请、智能体向导、构建窗口或 Walkthrough 时才显示。
- **每则公告一次。** 关闭窗口（它的 **Close** 按钮或它的 ×），那则公告就不会再在这台
  机器上显示。新的或已更改的公告会再显示一次。如果你不关闭它就退出，它会在下次启动时
  显示。
- **没有公告，就没有窗口。** 没有发布任何公告时、没有网络时，或无法连接 GitHub 时，
  什么都不会显示，也不会显示任何消息。
- **它来自哪里。** 该页面是 GitHub 上 PowerRustCOBOL 仓库中的
  `announcements/announcement.html`。IDE 读取它的方式，与它为更新检查读取最新发行版的
  方式相同，除了它的版本之外什么都不发送。
- **它如何绘制。** 该页面由与显示 HTML 的 **Viewer** 控件相同的引擎绘制，在一张白色
  页面上：标题、列表、表格、颜色、方框和 flex/grid 布局。不运行任何脚本，也不获取任何
  其他东西。链接被绘制成链接，但不会打开，图片则显示它的替代文字。

IDE 所记住的东西在这台机器上，与它的语言在同一个文件里：你上一次关闭的那则公告的标识。
没有任何东西被写入你的项目。

---

## 6. 项目与项目模型

**项目**是一个文件夹，其中包含一个以项目命名的清单文件——比如 `HelloPower.project.toml`；
本指南称它为**项目文件**——以及你的源码、窗体和资源。清单记录了项目名称、版本、主程序，
以及每个分类中的文件。

### 文件夹布局

创建项目时，PowerRustCOBOL 会在磁盘上搭建出这样的结构：

```text
HelloPower/
├── HelloPower.project.toml  ← 项目清单（项目文件）
├── src/                     ← Common Code  （手写的 COBOL 程序/copybook）
├── forms/                   ← Forms        （.cfrm 设计器文件）
├── indexed/                 ← Indexed Files （.cidx 定义）
├── generated/               ← Generated Code （RAD 生成的 .cbl——只读）
├── COPYBOOKS/               ← 每个索引文件一份：它的 SELECT、它的 FD，以及
│                              原始编辑器使用的可编辑 COBOL 描述符
├── crates/                  ← Project's Crates （注册了第一个之后才会出现）
├── Assets/                  ← Assets       （图像、音频、字体、数据文件）
├── Knowledge Base/          ← 项目专属的文档和已索引的知识
├── bin/                     ← 构建出的二进制
├── debug/                   ← 调试用的工作文件
├── temp/                    ← 临时文件
├── dist/                    ← Build 所安装进去的交付文件夹
└── data/                    ← 项目数据文件（例如 AI 对话存储）
```

> **`data/` 与 AI 助手共用。** 你程序的数据文件放在那里，Grace 的每项目文件也在那里：
> `project-knowledge.redb`、`<project>-chunked.data`，以及 `grace-conversation.json`——
> 你与她自己的聊天历史。构建会把 `data/` 复制到你应用程序的交付文件夹中，但会**把这些
> 助手文件留在后面**，并移除早先的某次构建已经复制到那里的任何此类文件。你的应用程序
> 从不读取它们，而对话只属于你一个人。

新项目还会得到一个**可运行的起始 `main` 程序**（默认是 `src/main.cbl`）——一段最小的
`IDENTIFICATION DIVISION` / `DISPLAY` / `GOBACK`，你可以立即 **Run**，然后在此基础上
扩展。

> **窗体优先的项目。** 如果你删掉起始的 `main`，并构建一个只由窗体组成的项目，**Build**
> 和 **Run** 仍然能用——而确切地知道是哪个程序在启动是值得的，因为窗体的优先级高于清单。
>
> 有窗体的项目总是从它的**主窗体**所生成的程序（§11）启动，这优先于 `[project].main`，
> 即使清单点名的是一个存在的文件。这是刻意的：IDE 创建的窗体项目同时也带着那个七行的
> 起始 `main`，而当起始程序胜出时，你得到的是一个把窗体画出来、然后运行那个桩程序的
> 二进制——每个按钮都是死的，因为编译后的程序里根本没有任何处理程序。如果没有哪个窗体
> 带有该指定，就使用第一个窗体。
>
> `[project].main` 只在项目**没有窗体**时才起决定作用。否则，使用第一个生成的程序，然后
> 是磁盘上存在的第一个普通源文件。

> **注意。** 打开一个早于这套布局的旧项目时，会自动**补回任何缺失的标准文件夹**。旧的
> `Documentation/` 和 `docs/` 项目文件夹下的内容会被移入 `Knowledge Base/`，不会覆盖
> 冲突的文件。

### Web 或移动项目的文件夹

项目有一个**种类**——Desktop、Web 或 Mobile——在 **File ▸ New Project** 中选择（第 23 章）。
Desktop 项目就是上面的布局，版本 2 之前写成的每个项目都是它。Web 或 Mobile 项目把它的
窗体放在 `webforms/`（`.wcfrm`）或 `mobileforms/`（`.mcfrm`）中，而不是 `forms/`，可以有
一个 **Backend**，并构建到 `dist/web/` 或 `dist/mobile/` 中：

```text
OrderApp/
├── OrderApp.project.toml    ← 清单（[project] kind = "web" 或 "mobile"）
├── webforms/                ← 窗体（Mobile 项目中是 mobileforms/）
│   └── application.css      ← 应用程序样式表；每个窗体旁边有 <Form>.css
├── backend/                 ← Backend：不得在页面里运行的东西
│   ├── src/                 ←   它的 COBOL 程序
│   ├── COPYBOOKS/
│   ├── exports.toml         ←   调用方可以到达的程序，以及契约版本
│   ├── backend.toml         ←   它在哪里、如何运行
│   ├── contract/            ←   interface.json 和已发布的契约
│   └── generated/           ←   Build 写出的程序（绝不编辑）
├── Assets/  indexed/  data/ ← 与 Desktop 项目中相同
└── dist/web/                ← Build 的包（Android 和 iOS 用 dist/mobile/）
```

项目树显示的是一个 **WebForms**（或 **MobileForms**）节点来取代 Forms，然后是一个
**Backend** 节点。

### 八个树分类


| 分类 | 容纳 | 可编辑？ |
| --- | --- | --- |
| **Forms** | `.cfrm` 窗体设计器文件 | 通过 Designer |
| **Indexed Files** | `.cidx` 索引文件定义 | 通过 Indexed File Editor |
| **SQL Connections** | 你的 `EXEC SQL` 和 `SqlDatabase` 按名称访问的数据库（见[嵌入式 SQL](#嵌入式-sqlexec-sql)） | 通过 SQL 连接编辑器 |
| **Common Code** | 你从窗体 `CALL` 或直接运行的手写 COBOL | 是 |
| **Generated Code** | PowerRustCOBOL 从每个窗体或 `.cidx` 生成的 `.cbl` | **只读**（蓝色，锁图标） |
| **Project's Crates (Beta)** | 你为 `EXEC RUST` 块注册的第三方库 | 通过 External Crates 对话框 |
| **Assets** | 随应用程序捆绑的图像、音频、字体、数据文件 | 导入 |
| **Knowledge Base** | 项目专属的 Markdown / 文本 / PDF 材料 | 是 |

### 从窗体调用 Common Code

**Common Code** 中的每个程序都被链接进每个窗体的程序——无论是在 Run Form 中还是在构建出
的应用程序里——所以处理程序可以通过它的 `PROGRAM-ID` 到达它，并用 `USING` 传递数据，
就像调用一个子程序那样：

```cobol
      *> src/calc-tax.cbl
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALC-TAX.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-AMOUNT PIC 9(7)V99.
       01 LK-TAX    PIC 9(7)V99.
       PROCEDURE DIVISION USING LK-AMOUNT LK-TAX.
           COMPUTE LK-TAX ROUNDED = LK-AMOUNT * 0.2
           GOBACK.
```

```cobol
      *> a button's onClick handler
           CALL "CALC-TAX" USING WS-AMOUNT WS-TAX
           MOVE WS-TAX TO LBL-TAX::Caption
```

被调用的程序在多次调用之间保持它自己的 WORKING-STORAGE，也保持它自己的文件：
`SELECT … ASSIGN TO WS-PATH` 打开的是它的 `WS-PATH` 所持有的路径。

> ⚠️ **注意事项。** 有三种源文件不会被链接：项目的主程序、本身就是某个窗体所生成程序的
> 源文件，以及包含 `EXEC RUST` 块的 Common Code 程序（它们的 id 是按应用程序编号的）。
> 定义了同名程序的窗体保留它自己的那一个。

> **对一个不存在的名称的 `CALL`，现在是编译错误。** 拼错的 `CALL "CALC-TXA"` 过去能编译
> 并运行，而这个调用会一声不响地被跳过，于是处理程序继续往下走，仿佛税款已经算过了。在
> 项目内部，Run Form、Build、窗体检查和 `rcrun check` 现在会拒绝一个字面量 `CALL`，只要
> 它的名称不是窗体所包含的程序、某个段落或节，也不是已链接的 Common Code 程序，并会说
> *did you mean "CALC-TAX"?*（你是想写 "CALC-TAX" 吗？）。因上述原因而被排除在外的
> Common Code 程序，算作不存在。有两种情况不受影响：带有 `ON EXCEPTION` 的 `CALL`，它表明
> 该程序是可选的；以及保存在数据项中的程序名，这在程序运行之前是无法得知的。`COBOL-…`
> 内置调用不受这条规则检查。

### 创建与导入

分类上的 **➕** **创建一个新条目**：

- **Forms ➕** → *New Form* 对话框。
- **Indexed Files ➕** → *New Indexed File* 向导（名称、指定路径、记录布局、键、存储）。
- **SQL Connections ➕** → 一个新的 SQL 连接，在它的编辑器中。
- **Common Code ➕** → 从起始模板创建的新 `.cbl`，在编辑器中打开。
- **Knowledge Base ➕** → 一个新的 Markdown 文件。
- **Assets ➕** → 文件选择器（资源是在外部创作的，所以“创建”= 导入）。

使用 **Knowledge Base** 旁边的“文件夹加号”命令来创建顶层子文件夹。右键点击任何
Knowledge Base 子文件夹，可以创建子文件夹或删除该文件夹。删除文件夹需要确认，并会递归
地移除它的文档、嵌套的文件夹、项目清单中的条目，以及陈旧的向量索引条目。`Knowledge Base/`
根目录本身不能被删除。

要把一个**已有的文件导入**到某个分类中，**右键点击 ➕** 并选择 *Import existing…*。对于
**Indexed Files**，这会选取一个磁盘上的 `.idx`（或类似的）数据文件，并据此构建一个
`.cidx`。

**索引文件携带它自己记录的描述。** 当程序创建一个——或 Indexed File Editor 的 *Finalize*
按钮创建一个——该文件会得到描述其记录的 `FD`：每个字段的名称、PICTURE 和 USAGE、
`REDEFINES` 和 `OCCURS`、条件名，以及每个数据项所占的字节数。导入会读取它，所以 `.cidx`
进来时是完整的：字段带着它们真实的名称和 PICTURE，键以它们所覆盖的字段命名，记录布局
与程序所用的相同。什么都不必重新输入，也不需要别的程序拿着你的源码才能理解该文件。该
格式见 `docs/indexed-file-format-en.md`。

**早先版本写出的文件**（1.91.169 之前）只带有它的**键模式**：每个键的位置、长度和编码、
是否允许重复，以及可选的名称。从中导入的 `.cidx` 让各个键处在它们真实的位置上，作为字母
数字字段（`PIC X(n)`），而记录的其他每个字节都作为 `FILLER-n` 进来。这样的文件会在**程序
第一次为写入而打开它时被转换**——`OPEN I-O`、`EXTEND` 或 `OUTPUT`——或者当网格浏览器打开
它时：程序所声明的 `FD` 会被加到文件中，每条记录都原封不动，此后它导入时就是完整的。
`OPEN INPUT` 从不改变文件，所以它从不转换任何文件。

> ⚠️ **注意事项。** 文件由最后一个创建它或转换它的程序来描述，并保持那份描述：另一个把
> 同样的字节声明得不同的程序，不会取代它。如果描述是错的，请重新创建该文件（`OPEN
> OUTPUT`）。对于完全没有描述的文件（早先的版本，此后从未为写入而打开过），请在 Indexed
> File Editor 中打开导入的 `.cidx`，给字段起下它们真实的名称和 PICTURE；数值、压缩和
> 二进制字段无法从键模式中恢复。你的程序所用的记录布局，始终来自它自己的 FD。

> **注意。** 生成的 `.cbl` 文件位于 `generated/`，被自动跟踪，并以只读方式打开。编辑属于
> 窗体（Designer）、`.cidx`（Indexed File Editor）或 Common Code——绝不要编辑生成的输出。

### 在项目之间复制窗体

右键点击 **Forms** 树中的任何窗体并选择 **Copy Form**。这会复制它的*一切*——每个控件的
属性、每个已绑定事件完整的 COBOL 处理程序主体、动画和数据绑定——到你操作系统的剪贴板，
而不仅仅是应用内的暂存空间。切换到（或打开）另一个项目——在同一个正在运行的
PowerRustCOBOL 窗口中，或完全在另一个窗口中——右键点击 **Forms** 分类，并选择
**Paste Form**。窗体会按原样在那里创建：不需要重命名任何控件 ID 或事件段落，因为每个
窗体本来就编译成它自己自包含的 COBOL 程序——粘贴的窗体中的 `BUTTON1` 不可能与该项目中
某个其他无关的窗体在内部碰巧用到的 `BUTTON1` 冲突。它的 Generated Code 会立即生成，所以
粘贴的窗体无需先单独做一次 Build 就可以直接 Run。

Paste Form 在你选择它的那一刻读取剪贴板。如果剪贴板是空的，或者里面是你在复制窗体之后
复制的东西，Output 面板会这样说——“The clipboard is empty”或“The clipboard doesn't
contain a copied form”——并且不会创建任何东西。请重新复制窗体再粘贴。

如果目标项目已经有一个同名的窗体，PowerRustCOBOL 会询问怎么做，而不是去猜：**重命名**
传入的窗体（输入新名称，并针对已有的名称实时重新检查），或**替换**已有的那一个——替换
会在任何东西被删除之前，为它自己单独要求一次确认，就像直接从树中删除一个窗体那样。

> **注意。** 如果窗体在 Designer 中带着未保存的更改而处于打开状态，Copy Form 读取的是
> 当前屏幕上的内容——“复制”始终意味着“复制我正在看的东西”，而不是早先的一份陈旧保存。
> 粘贴一个其代码块引用了目标项目尚不具备的东西（一个 Project's Crates 固定项、一个资源、
> 一个数据绑定所点名的索引文件）的窗体，会如实地带上*引用*，但不会带上被引用的资源本身
> ——请在目标项目中添加一个匹配的，就像你在那里手工输入了这个引用一样。

### Indexed File Editor 与 Grid Browser

> 📷 **需要截图 — `indexed-file-editor.png`** —— Indexed File Editor 的视口，含字段列表、
> 属性窗格和工具栏（Save / Save & Generate / Finalize / Open Grid Browser）。

双击一个 **Indexed Files** 条目，会在它自己的窗口中打开 **Indexed File Editor**（与
Form Designer 相同的多窗口模式）。中间的窗格列出记录字段；下方的窗格显示文件级或字段级
的属性。**Finalize** 会创建磁盘上的数据文件，并锁定结构性字段（PIC、偏移量、键、存储）。
注释和每个字段的**网格控件**在之后仍然可以编辑。

**Open Grid Browser**（在 Finalize 之后）会打开第二个视口：一个覆盖在实时索引数据文件
之上的虚拟化表格，支持添加 / 编辑 / 删除、**Commit** / **Rollback**，以及当磁盘上的文件
不再与 `.cidx` 匹配时的模式漂移保护。

每个 `.cidx` 会产生 `generated/<stem>-indexed.cbl`（`SELECT` / `FD` 片段），与窗体输出
一样，在 **Build / Run / Debug / Check** 时重新生成。

**你的描述会随它一起走。** 你写在文件上的注释会成为它的 `FD` 之上的一行注释，而每个字段
的注释则跟在该字段之后——与编辑器的原始文本视图向你显示的形式相同：

```cobol
      *> Customer master — one row per account
       FD  CUSTOMER-FILE.
           RECORD CONTAINS 80 CHARACTERS.
           01 CUSTOMER-RECORD. *> The record
               05 CUST-ID PIC 9(8). *> Primary key
```

一个你什么都没说的字段，生成的内容与以前完全一样。这些是普通的 COBOL 注释：编译器会
把它们剥掉，所以它们在运行时不花任何成本，只为下一个阅读该布局的人而存在。

这些同样的描述还有第二个读者。请参阅 *HTTP / REST 与 AI 智能体*之下的**让模型查询你的
数据（MCP）**，在那里文件的注释会成为工具描述，而每个字段的注释描述一个搜索参数。

---

## 7. Form Designer（RAD）

Form Designer 是你排布窗口的地方。每个打开的窗体都是它**自己的操作系统窗口**，所以你可以
让几个设计器和运行中的窗体并排。在 IDE 项目树中，或在设计器的 **Forms** 列表中，双击
一个窗体就会打开它；如果它已经打开，它的窗口会被还原并带到前面。

```mermaid
flowchart LR
    TBX["工具箱<br/>(控件，分组)"]
    CANVAS["设计画布<br/>(拖 · 放 · 缩放 · 对齐)"]
    PROP["属性窗格<br/>(针对当前选择)"]
    TBX -- "拖到" --> CANVAS
    CANVAS -- "选择" --> PROP
    PROP -- "编辑" --> CANVAS
```

- **工具箱（左侧）。** 控件分八组，按这个顺序：**Common**、**Containers**、**Data**、
  **Graphics**、**Menus & Bars**、**Non-Visual**、**AWS** 和 **Charts**。把任何控件拖到
  画布上。用 **◀** 箭头把侧栏折叠为窄的**图标栏**（从图标栏拖拽依然有效），用 **▶** 展开
  它；拖动它的边缘来调整大小，你设定的宽度会在你重新展开时恢复。
- **画布（中间）。** 移动、缩放（拖动边框抓手）、对齐和分布控件。吸附网格让一切保持整齐。
  你可以通过拖动**窗体本身**的边缘来调整它的大小。
- **属性窗格（右侧）。** 编辑选中的控件——或者在没有选中任何东西时，编辑**窗体**本身。
  窗格被组织成可折叠的**分区卡片**；对窗体来说，它们依次是 **Form Properties**、
  **COBOL Structure**、**Target Device**、**Window**、**Appearance**、**Form Events** 和
  **Animations**。在一张卡片之内，属于一起的属性依次排列：渐变的起始颜色后面跟着它的
  结束颜色，边框的颜色、样式和宽度排在同一段，字体的名称在它的大小之前，每个控件自己的
  各组也是如此——复选框的勾选标记、图表的坐标轴、滑块范围的两端。拖动它的**左边框**来
  加宽它——悬停在边框上时它会变亮。它是一个**抽屉**：垂直居中的 **◀** 标签把它隐藏
  （留下一个细的 **▶** 标签用来把它滑回来），它重新打开时是你上一次设定的宽度。
  **搜索属性和事件。** **Properties** 和 **Events** 标签页上方的框会在你输入时缩小列表。
  一行在它匹配时显示：按它的**名称**（`capt` 找到 Caption）、按**它说明中的某个词**、按
  它的**分区**（`geometry` 显示整个分区；对窗体的事件，则是诸如 `mouse` 或 `lifecycle`
  的类别），或按它的**用途**：`size` 能找到 Width、Height、FontSize、AutoSize 和其余的，
  尽管它们没有一个叫 size——`tamanho`、`taille` 或 `サイズ` 也一样，无论你用哪种语言输入。
  每个词都必须匹配，所以 `font size` 只找到字体的大小。各行保留它们的分区；没有可显示
  内容的分区会消失，而框中的 **✕** 会清除它并再次显示一切。**User procedures** 标签页的
  搜索也有同样的 ✕。**悬停在属性的名称上**，以界面语言读到它做什么。每一条说明都是根据
  使属性产生效果的代码写成的，所以它告诉你控件真正对这个值做了什么。凡是属性已被存储
  但尚未被执行的地方，说明都会这样说。

设计器工具栏的要点：**Save & Generate**、**Generate only**、**Preview**（非交互式渲染）、
**Run Form**（实时、交互式）、网格开关、**Magnifier**（见下文*放大镜*）、**Theme**
（程序化样式：Classic / Enhanced / Neumorphic Light / Neumorphic Dark）、对齐工具、
撤销/重做。

> **所见即所得——每个界面共用一个渲染器。** Form Designer 画布、实时 Preview、Run Form
> 和编译后的二进制，都通过 `cobolt-forms` 中的**单一渲染引擎**绘制（交互界面用
> `render::render_form`，设计器画布用 `render::render_faces`），它把共享的 `draw_control`
> 外观绘制器与窗体层面的关注点包在一起，这些关注点过去曾在四个独立的绘制循环之间出现
> 分歧：背景、渲染顺序、容器裁剪、祖先不透明度和标签页可见性。每个界面通过 `FormState`
> trait 接入它自己的实时值（设计器 = 所设计的窗体，预览 = 一张值映射，运行 = `CtrlState`，
> 二进制 = 编译后的状态）。结果是：同样的窗体 + 状态总是产生同样的像素——你在画布上设置
> 的样式，恰好就是运行出来的样子。

> **被调整大小的窗口保持窗体不变，拉伸背景。** 当用户把运行中的窗体最大化或把它的边框
> 拖开时，控件仍然恰好保持你设计的位置和大小——只有**背景**跟随窗口，所以渐变（或背景
> 图像）覆盖整个窗口，而不是停在窗体的边缘。把窗口拖得比窗体*更小*并不会裁剪背景：它保持
> 窗体的大小，窗体在其中滚动。窗口进入效果为这同一幅画面制作动画，背景也包括在内。
>
> **在 Preview 中，颜色跟随窗口，图片不跟随。** Preview 是一个真实的窗口，你可以把它拖得
> 比窗体更宽，而它的背景**颜色**（或渐变）覆盖整个窗口——所以大于其图片的窗体看起来更
> 大，而不是被切掉。背景**图像**保持固定在你设计的尺寸上，并*在那里*继续遵守它的 Mode：
> Fit 仍然在窗体内部加黑边，Fill 仍然裁剪到窗体，Tile 仍然停在它的边缘。图片之外的部分
> 只是背景颜色——这也是你在编辑时仍能看出所设计的范围在哪里结束的方式。在 1.62.135 之前，
> 那个区域根本不被绘制：标题栏继续变大，而它下面的窗体却戛然而止，IDE 从缝隙中透了出来。
>
> **背景跟随的是界面（SURFACE），而它并不总是窗口。** 加载到外壳 ContentPane 中的窗体只
> 占据窗口的一部分，而不是全部——宽度被侧栏窄条减去，高度被面包屑带减去。它的背景是对着
> **那个窗格**排布的，所以 *Fit* 在窗格内部加黑边，*Center* 居中于窗格的中心。在 1.62.132
> 之前，窗格的占用者是对着整个窗口排布的：黑边落在可见区域之外，每种模式看起来都像
> *Stretch*。

### 背景图像模式

窗体的 **Image path** 接收一张图片；**Mode** 决定它如何与界面相接。全部五种都保持图片自己
的像素——它们只在缩放和放置的方式上有所不同。


| 模式 | 它做什么 | 变形？ | 裁剪？ | 留边？ |
| --- | --- | --- | --- | --- |
| **Stretch** | 把图像恰好拉到界面上 | **是** | 否 | 否 |
| **Fill** | 保持形状，放大到覆盖为止 | 否 | 是 | 否 |
| **Fit** | 保持形状，缩放到全部放得下为止 | 否 | 否 | **是** |
| **Center** | 以它自己的大小绘制，位于中间 | 否 | 若更大 | 若更小 |
| **Tile** | 以它自己的大小重复，像壁纸一样 | 否 | 仅边缘 | 否 |

> **注意——当形状相符时，Fit、Fill 和 Stretch 重合。** 如果图像的比例已经就是界面的比例，
> 三者就产生完全相同的画面，并没有什么不对。一张 1600×1000 的图像放在 1600×1000 的窗体
> 上，既没有要加黑边的，也没有要裁剪的。如果你想*看到*这三者的表现不同，可以试试刻意
> 选一张偏高或偏宽的图像。

> ⚠️ **注意事项——Center 不缩放。** 比窗体大得多的图片只显示它的中间部分，而小的图片则
> 漂浮在背景颜色之中。这是该模式在起作用：如果你想让它适应窗体的大小，请选 *Fit* 或
> *Fill*。

> **Tile 是真的平铺。** 在 1.62.130 之前，*Tile* 只绘制一份被拉伸的副本——它与 *Stretch*
> 共用一条代码路径，这个模式什么也没做。现在它从界面的左上角开始，以图像的原始大小重复
> 它。DataGrid 自己的 **Grid background image mode** 在 1.62.132 中获得了同样的修复。

> **窗体边缘之外的空间——在窗口变大时才出现的控件。** 你设计的尺寸是一个*下限*，而不是
> 上限。在画布上把一个控件放在右边缘或下边缘之外，它会完全按你放置的样子保留；只是当
> 窗口只有窗体那么宽时，它没有地方可落。把那个窗口最大化——或把它拖开——这个控件就会出现
> 在腾出的空间里。什么都不被拉伸，什么都不被重新排布，这与上面的规则一致：控件按你给它的
> 位置和大小绘制，而窗口自己的边缘是唯一会切割它的东西。这是一种刻意的办法，把一个可选
> 的侧面板、一幅装饰性图形或一张宽图表，留给那些有这么大屏幕的操作员。
>
> **说明。** 嵌套不变——Panel 或 GroupBox 之内的控件仍然被裁剪到它的容器，无论窗口变得多
> 大；只有*窗体*自己的边缘不再是一堵墙。设计器画布本来就绘制这些控件，所以你在编辑时看到
> 的，现在就是运行出来的。
>
> ⚠️ **注意事项。** 比窗体*更小*的窗口会滚动窗体，但它不会滚动到放在设计边缘之外的控件
> 那里：可滚动的区域就是窗体所设计的矩形。任何操作员必须始终够得着的东西都应当放在它里面
> ——把边缘之外的空间当作额外的赠品，绝不要当作触及某个控件的唯一办法。

> **Run Form 隔离（性能）。** 为了让 IDE 保持响应，并避免窗体运行时（尤其是带有定时器、
> 循环或繁重渲染时）CPU 飙升，`Run Form` 会派生一个隔离的子 `rcrun` 进程。IDE 与子进程
> 通过 stdio 上带帧的 bincode IPC 通道通信（`FormIpcMessage` 用于事件、输入、状态快照、
> 显示、错误、完成）。IDE 把 stdout 泵送到本地通道，并通过 stdin 把界面事件转发回去。这
> 也启用了 **Run-Form Inspector**（CPU %、RSS、子进程、系统内存、进程树、异常检测）。对于
> IDE 可执行文件旁边的“rcrun”，使用同样的二进制路径解析。

运行时界面只增加实时的行为（按下反馈、焦点、文本输入、滑块拖动），而设计器在它之上增加
它的编辑器叠加层（选择手柄、徽章、放置提示）。徽章——标记带有动画的控件的黄色 ▶，重复
组的 ARRAY 标签——属于它所标记的控件：在那个角落前面绘制的控件，例如位于上方图层上的
面板，会连同它所覆盖的那一部分控件一起把徽章隐藏。

#### 放大镜

画布始终以窗体的真实大小显示它，这是你评判布局时想要的——但当一张卡片把 9 点的标签
挤得只相隔几个像素，而你分不清指针在哪一个标签、哪一个手柄上时，这就不是你想要的了。
点击设计器工具栏中的**放大镜**（位于网格和主题开关旁边），工具箱的顶部、**Common** 之上
就会多出一个 **Zoom** 区域：一个与工具箱同宽的方块，以**四倍大**显示指针下方那部分窗体，
指针所在处有一个小十字准线。

- 当指针位于窗体上时，它跟随指针。移出窗体，它就**停止更新**，保留最后一幅画面以便你仔细
  研究；在你第一次指向窗体之前，它显示 *Point at the form*。
- 它就是画布本身，被放大了——控件、选择手柄、网格点，一应俱全——而文字是以更大的尺寸绘制
  的，而不是被放大，所以 9 点的标题保持清晰。只显示窗体，绝不显示它周围的 IDE。
- 它只是一种**查看**的方式，仅此而已：它从不改变窗体，不把它标记为已修改，也不往撤销中
  添加任何东西，而你照常在画布上继续工作。把工具箱拖宽，这个方块也随之变大。
- 每个设计器窗口都有它自己的放大镜，而窗体打开时总是关闭的。再点一次放大镜就能隐藏该
  区域。

> 📷 **需要截图 — `designer-magnifier.png`。** 打开 PowerSpatial 的 `main-form`，打开放大镜，
> 并指向某张卡片上的一个小标签。截取设计器窗口，显示工具箱的 **Zoom** 区域，其中有被放大
> 的标签和十字准线，以及位于画布上的指针。

#### 选择多个控件

有两种方式，而且可以组合使用：

- 在空白画布上**拖出一个套索**——矩形碰到的每个控件都被选中。
- **按住 Command（macOS）或 Control（Windows/Linux）并点击**——把一个控件加入选择，或者
  在它已经在其中时把它移除。按住修饰键去拖动一个尚未被选中的控件，会把它加入，并在一次
  手势中移动整个选择。

选中一个**容器**，在移动时会连同它的子项一起选中，所以 GroupBox 会拖动它的整棵子树，并保持
它的布局刚性不变。第一个被选中的控件是**主**控件：对齐和大小命令以它为基准度量，而属性
窗格读取它的值。

**拖动选择是刚性的。** 整个组按同一个偏移量移动，该偏移量取自指针下方的控件，所以你排好
的间距在移动之后依然保留——即使这些控件没有落在网格线上。

**属性窗格编辑整个选择。** 选中了不止一个控件时，它显示它们的共同点，并把每次更改应用到
所有这些控件上：

- **类型相同**——完整的窗格。Button 有的每个属性，五个被选中的 Button 都有。
- **类型不同**——只显示它们的类型真正共有的属性，因为只有其中一些带有的行，看起来会起
  作用，却对其余的什么都不改变。

无论触及多少控件，一次编辑就是**一个撤销步骤**。没有该属性的控件会被置之不理，而不是被
赋予它；而身份——控件 ID、Tab 顺序和父级——永远不会被共享，因为两个控件不可能拥有同一个。

### Tab 顺序与 Enter 键

**Tab 顺序**是键盘穿过运行中窗体的路径：Tab 移到下一个控件，Shift+Tab 移到上一个，两者都会
循环回绕。每个控件的位置就是它的 `TabOrder` 属性。你放到窗体上的控件会取下一个编号，所以
在你另行指定之前，Tab 就遵循你放置东西的顺序——这与 PowerCOBOL 和 isCOBOL 给你的默认
行为相同。

在带有图层的窗体上，每个标签页——`Form` 和每个图层——都有它自己的顺序，从 1 开始编号
（§7）；下面的两个按钮作用于当前活动的标签页。

设计器工具栏右端的两个工具栏按钮用来设置它：

- **Visual Tab Order** —— 打开它，然后按你希望 Tab 访问的顺序点击这些控件。每个参与其中的
  控件都在旁边显示它的编号；本次会话中你已经点过的那些用强调色显示，其余的保留它们原来的
  顺序，排在它们后面。点错了一个？接着按正确的顺序点下去——再次被点击的控件会移到最新的
  编号。该模式打开期间，什么都不会移动或改变大小。**把按钮关闭即告完成**：新的顺序会作为
  一个撤销步骤写入窗体。
- **Tab Order list** —— 一个列出同样这些控件的窗口，从第一个到最后一个。拖动一行，或选中它
  并用 **▲ / ▼**，来移动它。选中某一行会在窗体上选中那个控件，而窗口打开期间，编号会显示在
  各控件旁边。**Apply** 写入该顺序（一个撤销步骤）；**Cancel** 让窗体保持原样。

> 📷 需要截图 — `designer-visual-tab-order.png`。截取一个带有四五个带标签字段的窗体，此时
> Visual Tab Order 已打开，并且点击了两个控件，使两种徽章颜色都显示出来。

> 📷 需要截图 — `designer-tab-order-list.png`。截取同一个窗体之上的 Tab Order list，其中
> 有一行被选中并正在拖动，显示出插入线。

**Tab 停留在你所在的窗体内。** 在应用外壳中，ContentPane 里的窗体和 SideMenu 页脚里的
控件是两套独立的 Tab 顺序。Tab 和 Shift+Tab 走的是持有焦点的那一套，绝不会跳进另一套。在
1.70.223 之前，页脚会接管每一次 Tab，所以窗格里的窗体可以用 Enter 走完，却不能用 Tab 走完。

**谁参与其中。** 能接收键盘的控件——Button、TextBox、CheckBox、RadioButton、ListBox、
ComboBox、DataGrid、DateTimePicker、NumericUpDown、TreeView、Slider、TabControl——以及
**Label**。运行时，只有可见的、已启用的才会被访问。

**键盘上的 TabControl**，与 Windows 和 Web 上的一样。Tab 落在它选中的标签上，用标签内部一条
细的虚线边框标记，颜色是该标签自己的文字颜色。从那里起，**Tab 进入活动页面**，到达它在 Tab
顺序中的第一个控件，并继续走过该页面；越过页面的最后一个控件，它就移到后面跟着的任何东西
上。**从页面的第一个控件按 Shift+Tab 会回到标签条。** 要切换标签页：沿着标签条的方向键——
← / →，或者当标签条在左侧或右侧时用 ↑ / ↓——以及 Home / End，或者用
**Ctrl+Tab / Ctrl+Shift+Tab** 切到下一个 / 上一个标签（循环回绕），它们在标签条上以及页面
内的任何地方都有效，并让焦点留在标签条上。每一种都像点击那样让该标签页成为活动的，并引发
`onTabClick`，而当选择发生移动时，还引发 `onTabChanged`。点击某个标签页也会把焦点交给
TabControl。在 1.91.208 之前，标签条上一次普通的 Tab 会选中下一个标签页，所以页面自己的
控件无法通过键盘到达。

**Label 从不保留焦点。** Label 被编了号，使顺序读起来就像窗体本身那样，但当 Tab、Enter 或
一次点击到达它时，它会引发 `onGotFocus`，焦点则径直继续走向下一个控件。这就是你实现无障碍
的钩子——例如，宣布即将被填写的字段：

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTOMER-LABEL--ONGOTFOCUS.
       PROCEDURE DIVISION.
      *> Hand the caption to whatever reads the screen aloud.
           DISPLAY "Customer name"
           GOBACK.
```

**Enter 可以像 Tab 一样前进。** 在 TextBox、ComboBox、NumericUpDown、DateTimePicker、
CheckBox 或 RadioButton 上，属性 **`EnterAsTab`**（默认开启）让 Enter 移到 Tab 顺序中的下一个
控件。`onEnterPressed` 仍然会先触发，所以在 Enter 上做验证的处理程序继续有效。在 Enter 应当
原地不动的控件上把它关掉。它对多行 TextBox 没有影响，在那里 Enter 是换行，而展开的 ComboBox
列表则保留 Enter 用来选取它的条目。

**`AutoEnter` 填满即前进。** 在 TextBox 上，`AutoEnter` 让把框填满的那次按键算作 Enter：
`onEnterPressed` 触发，并且在 `EnterAsTab` 开启时，焦点继续前进。“满”指的是该框已经强制的长度
——显式 `Picture` 的宽度，或 `MaximumLength`。两者都没有的框永远不会满，所以这个属性在那里
什么也不做。这是经典的数据录入节奏：往 `9(6)` 中输入一个六位数的日期，光标就已经在下一个字段中
了。

### 响应式设计

按像素设计的窗体保持那个大小。把它的窗口变大，窗体保持它设计的大小位于左上角，其余部分留空；
把它变小，它就滚动。在 PowerCOBOL 中，你要写一个 `onResize` 处理程序，用手工移动并拉伸每个
控件。**响应式**窗体会替你做这项工作：每个控件说出它想要什么——*留在这个角落*、
*随窗口拉伸*、*填满剩下的部分*、*与它的邻居排成一行*——而每当它的窗口，或它所加载进去的
窗格改变大小，窗体就会重新排布自己。你设计的矩形仍然是事实的来源；布局只是它们的一个视图。

用窗体的 **Responsive design** 属性（`.cfrm` 中的 `Responsive`）来打开它。用这个版本创建的项目
会为每个新窗体打开它；先前就存在的窗体，绝不会被替你更改。下面的一切都是带有默认值的属性，
在属性窗格的 **Layout** 部分设置，或从 COBOL 写入。

**锚定与停靠——每个控件相对于它的父级。**

| 属性 | 它的含义 |
|---|---|
| `Anchor` | 控件所跟随的父级边缘：`Top`、`Bottom`、`Left`、`Right` 中的任意几个，以逗号分隔。默认是 `Top,Left`。一个轴上只有一条边，就保持那段距离；两条边都有，就随它的父级拉伸该控件；两条边都没有，就使它的中心保持在同样的比例处。MenuBar 默认是 `Top,Left,Right`，StatusBar 默认是 `Bottom,Left,Right`。 |
| `Dock` | `Left`、`Top`、`Right`、`Bottom` 或 `Fill`。停靠的控件按 z 顺序，取走父级所剩部分的它那一条边，并忽略 `Anchor`。`Fill` 取走其余的部分。 |
| `MinWidth`、`MinHeight`、`MaxWidth`、`MaxHeight` | 对任何被拉伸、停靠或增长的尺寸的限制。0 表示没有限制。 |
| `Locked` | 不是布局属性：它阻止鼠标在设计器画布上拖动该控件。 |

锚定为 `Left,Right` 的文本字段随窗口增长；锚定为 `Bottom,Right` 的按钮留在角落里；一个停靠
`Left` 的面板，旁边有一个 `Fill` 面板，就是经典的列表加详情的界面。

> 📷 **需要截图 — `responsive-anchors.png`。** 打开 PowerDemo3，选择
> **Samples ▸ Responsive Layout ▸ Responsive: anchors and limits**，分别以其设计大小和两倍宽度
> 截取它。

**排布其子项的容器——`LayoutMode`。** 窗体、Panel、GroupBox 或 TabControl 都有一个
`LayoutMode`：

| `LayoutMode` | 把它想成 | 掌控它的属性 |
|---|---|---|
| `Absolute`（默认） | 锚定与停靠，如上所述 | `Anchor`、`Dock`、各项限制 |
| `Flex` | 一行或一列，其中各项共享空间 | `FlexDirection`、`FlexWrap`、`JustifyContent`、`AlignItems`、`AlignContent`、`Gap`；每一项上有 `FlexGrow`、`FlexShrink`、`FlexBasis`、`AlignSelf`、`Order` |
| `Grid` | 由轨道组成的表 | `GridColumns`、`GridRows`（`200px 1fr 2fr`、`Auto`、`MinMax(160px, 1fr)`、`Repeat(3, 1fr)`、`Repeat(AutoFill, MinMax(180px, 1fr))`）、`Gap`、`JustifyItems`；每一项上有 `GridColumn`、`GridRow`、`ColumnSpan`、`RowSpan` |
| `Flow` | 文本：各项排成一行并折行 | `FlowDirection`、`WrapContents`、`Gap`；每一项上有 `FlowBreak` |

容器的子项由容器来摆放：它们自己的 `Anchor` 和 `Dock` 会被忽略，而它们被绘制时的大小就是它们
所请求的大小。`Padding`（以及 `PaddingLeft`、`PaddingTop`、`PaddingRight`、`PaddingBottom`）在
每种模式下都让它们远离容器的边缘。容器可以嵌套：Grid 可以容纳一个 Flex 行，而它又容纳一个
Flow。

**断点——为不同的宽度提供不同的排布。** 响应式窗体有一张表，列出它必须在其中绘制的宽度的
命名范围。默认是 `Compact`（600 以下）、`Medium`（600 到 1023）和 `Expanded`（1024 及以上）；你可以
重命名它们、添加一些并移动它们的界限（窗体属性中的 **Breakpoints**）。你绘制窗体时所用的宽度
属于其中一个，那就是*设计*断点：它承载你所绘制的东西。任何其他断点都可以承载**覆盖**——对
某个控件，不同的 `Visible`、位置、大小、`Anchor`、`Dock`、容器或项属性，或者 `FontSize`。覆盖不会
叠加：任何宽度下的布局，都是你所绘制的，加上该宽度所在断点的覆盖。标题、文本、条目和处理程序
不能被覆盖：一个窗体仍然意味着一套数据和一套代码。

在 Form Designer 中，**View at** 栏会以你所选的宽度显示窗体，而你在选中某个断点期间所做的更改，
会成为该断点的覆盖，并带有把它重置为基础设计的办法。

> 📷 **需要截图 — `responsive-view-at.png`。** 在 PowerDemo3 的 **Responsive: breakpoints**
> 窗体中，选择 **View at ▸ Compact**，并截取选中了被覆盖控件的设计器。

**字体缩放。** `FontScaling` 是 `None`（默认：按设计的大小）、`Fluid`（大小跟随窗口的宽度除以
设计的宽度）或 `Stepped`（每个断点带一个系数）。`MinFontScale`（0.85）和 `MaxFontScale`（1.50）限定
这个系数。控件可以用 `ScaleFont = false` 选择退出，或者被限制在它自己的 `MinFontSize` 与
`MaxFontSize` 之间。缩放改变的是文字，绝不改变控件所设计的矩形。

**从 COBOL。** 窗体回答 `ME::Breakpoint`（活动断点的名称）和 `ME::FontScale`（系数），并在大小
变化时引发两个事件：`onBreakpointChanged`，仅当宽度跨入另一个断点时；然后是 `onResize`，在布局
完成之后，只引发一次：

```cobol
      *> In the form's onBreakpointChanged handler:
           IF me::Breakpoint = "Compact"
               SET     LBL-HINT::Caption             TO    "Narrow window"
           ELSE
               SET     LBL-HINT::Caption             TO    "Wide window"
           END-IF
```

在响应式窗体上，控件的 `X`、`Y`、`Width` 和 `Height` 读到的是它们*现在*被排布出来的样子，而写入
会被视为当前大小下在屏幕上的位置：`ADD 10 TO BTN-OK::X` 会把按钮移动十个像素，不管它的锚定
如何，而下一次调整大小会保留这次移动。这也是为什么用手工摆放控件的旧 `onResize` 处理程序依然
有效：它的写入与布局相一致，而不是互相对抗。更好的迁移办法是删除该处理程序，用锚定、Flex 或
Grid 来表达同样的意思。

```mermaid
flowchart TD
    W["窗口或窗格改变大小"] --> B["哪个断点包含这个宽度？"]
    B --> O["基础设计 + 该断点的覆盖"]
    O --> C{"容器的 LayoutMode？"}
    C -->|Absolute| A["锚定与停靠"]
    C -->|Flex, Grid, Flow| F["容器摆放它的子项"]
    A --> S["字体缩放、限制"]
    F --> S
    S --> R["onBreakpointChanged，然后 onResize"]
```

**说明**

- 不是响应式的窗体，其行为与以往完全一样。把窗体从 PowerCOBOL 迁移过来时，较旧的、按比例
  调整大小的办法是 *Obsolete scaling style*，下文会描述；对新窗体，请优先使用锚定和容器。
- PowerDemo3 对上面的每个想法都有一个窗体；见下文 *PowerDemo3 中的响应式示例*。
- 响应式窗体的窗口会拒绝缩小到两个控件会发生碰撞的大小之下；见下一节中的
  *响应式窗口会在其控件碰撞之前停住*那一段。

> ⚠️ **注意事项。**
> - `1fr` 的网格轨道永远不会比你在其中绘制的最宽的项更窄。写 `MinMax(160px, 1fr)` 来说明一列
>   最窄可以窄到多少。
> - Flex 列取其内容的高度。给应该填满空间的那一项设置 `FlexGrow 1`，并以它最小的有用高度来
>   绘制它。
> - 断点是由窗体所绘制的界面的宽度来选择的——加载到外壳内容窗格中的窗体，是按窗格来度量的，
>   而不是按屏幕。

### 新建窗体的起点

*New Form* 对话框有两个选择，决定窗体从第一分钟起是什么样子：

- **Start from** —— 一个 **Blank form**，或一个已排布好、可以直接填充的模板：
  **Record entry**（一个标题、一张带标签字段的卡片，以及右下角的 New / Delete / Save 按钮）、
  **List and details**（左边是一个搜索框和一个网格，右边是所选记录的字段）或
  **Dashboard**（三张共享宽度的指标卡片，下面有两张图表）。模板带有它自己的大小，用 IDE 的语言
  写出它的标题，并且是响应式的：它的字段会拉伸，它的按钮从第一次运行起就跟随窗口的边缘。它
  不写任何代码——处理程序归你。
- **Modern style** —— 默认开启。字段是扁平的，带一条细边框和圆角，卡片在浅色背景上是白色的，
  文字是深色的、统一大小，主按钮是强调色。**你之后放到窗体上的每个控件都采用同样的外观**，
  所以窗体在成长时保持一致。取消勾选可得到经典外观；模板总是现代的。

用这个版本创建的项目会生成**响应式**窗体（**Settings ▸ New forms are responsive**，已勾选）。
更早创建的项目会像以前一样继续生成窗体，直到你勾选它；已有的窗体绝不会被替你更改。

**响应式窗口会在其控件碰撞之前停住。** 当用户缩小或放大响应式窗体的窗口时，控件会随它们的锚定
而移动和拉伸。这可能把其中两个带到一起。例如，锚定在右边缘的按钮向左滑到一个锚定在左边的字段
上，或者拉伸在两个边缘之间的字段长进一个原地不动的邻居里。缩小也可能把控件推出视野。锚定为
`Top,Left` 且靠近窗体右边缘的按钮，一旦窗口比按钮的右边缘更窄就会被切掉。靠近面板底部、随窗口
缩小的标签，最终落在面板边缘之下。窗口拒绝走得那么远：它停在最后一个这样的大小上——在你的设计中
分开的每一对控件仍然是分开的，并且在你的设计中位于它的面板、分组框、标签页或分割窗格之内（或
窗体之内）的每个控件仍然位于其内。两个方向都算数，缩小和增长，宽度和高度都一样。同样的限制适用于
IDE 中运行的窗体、编译后的应用程序，以及设计器的视图大小抓手。

在 SideMenu 的内容窗格中打开的窗体没有它自己可供停住的窗口，所以窗格无法拒绝增长。窗体改为被
排布成不大于它的限制：超过限制之后，窗格的其余部分保持为空。在比窗体的最小值更小的窗格中，窗体
保持它的最小尺寸，而窗格滚动，与窗口本来会停住的布局相同。

你有意放得相互接触或重叠的控件，例如卡片上的标签，或两个紧挨着的工具栏按钮，会被置之不理。
你放得部分位于其容器之外，或窗体之外的控件，也会被置之不理。`WrapContents` 关闭的 Flow 容器
的各项也一样：那种布局是一行，在容器的边缘被裁剪，所以越过边缘的项是预期之中的。折行的 Flow，
或 Flex 行，会把它的各项保持在内部，并且确实会让窗口停住。隐藏的控件会被置之不理，位于
TabControl 不同页面上的控件也是，它们从不同时出现在屏幕上。控件从不相遇、也从不触及边缘的窗体，
除了它的 `MinFormWidth` × `MinFormHeight` 下限之外没有别的限制。

> 💡 如果窗口停得比你希望的更早，请给控件留出移动的空间。把字段锚定到两个边缘，使它收缩而不是
> 被碾过，或者给按钮一个 `MinWidth`、给字段一个 `MinWidth`，让它们能放在一起。

**更改窗体的主题时，文字保持可读。** 当你切换窗体的主题或玻璃样式时，任何其文字在新背景上不再
醒目的标签、按钮或字段——低于无障碍指南对文字所要求的 4.5:1 对比度——都会取黑色或白色，取哪个
看得清就用哪个。它是同一次更改的一部分，所以 **Undo** 会把旧颜色放回去。

**切换回某个主题，会带回你原来有的窗体。** 主题切换会给每个控件穿上新主题的默认外观。再次选择
先前的主题，而不是使用 Undo，会交还你离开它之前窗体所拥有的颜色、阴影、圆角和背景，而不是那个
主题的原装外观。只有主题切换所改变的东西才会被放回去。你编辑过的标题、你移动或添加的控件，以及
你的事件代码，保持它们现在的样子。IDE 在窗体打开期间记住这一点；在你关闭并重新打开它之后，切换
回去会再次给出该主题的默认值。

你放置的 **PictureBox** 起初 **ShowFrame** 是关闭的：只显示图像，而透明的 PNG 会让窗体透出来。
勾选它，就会在图像后面出现卡片。先前保存的窗体保留它们原有的边框。

**看到焦点在哪里。** 运行中的窗体里持有焦点的控件会戴上一圈边框——*焦点环*——所以数据录入的操作员
永远不会丢失他的位置，不管他是通过 Tab、Shift+Tab、Enter 还是一次点击到达那里的。它是一项项目
设置，对每个窗体都相同：**Settings ▸ Appearance ▸ Keyboard focus ring** 有一个复选框可以把它打开
或关闭（默认开启），选择颜色（默认是蓝色），以及可选的 **Pulse**，它会让边框缓慢地呼吸——每 6.4
秒一次完整的呼吸。焦点一离开该控件，焦环就消失。容器只在它裁剪控件的地方裁剪焦环：滚动到 Panel
一半之外的字段，会沿着 Panel 的边缘失去焦环，而在其他几边保留它，而停靠为填满 Panel 的网格则保留
它完整的焦环。Run Form 和你构建出来的应用程序以同样的方式显示它。

> 📷 需要截图 — `running-form-focus-ring.png`。截取一个按了两次 Tab 之后的运行中窗体，使第二个字段
> 显示出焦点环。

> ⚠️ **Enter 与默认按钮。** 已经移动了焦点的 Enter 就用掉了：它不会同时按下窗体的 `IsDefault`
> 按钮。当焦点在 `EnterAsTab` 关闭的控件上（或在 Button 上）时，Enter 会像以前一样到达默认按钮。

### 迁移 PowerCOBOL 窗体：*Obsolete scaling style*

PowerCOBOL 的窗体可以按比例跟随它的窗口：把窗口放大一半，每个控件就放大并移动一半，它的文字也
随之变化。PowerRustCOBOL 的窗体在新的工作中做得更好。锚定保持字段的边距，容器分配空间，而文字
跟随断点或宽度。但是，你带过来的窗体是为按比例缩放而绘制的，把它重新排布是你在第一天也许不想做的
工作。

窗体上的 **Obsolete scaling style**（文件和 COBOL 中的 `ObsoleteScalingStyle`）用一项设置就给你旧的
行为。这个名字说明了它是什么：用于迁移窗体的桥梁，而不是设计新窗体的办法。

| 值 | 跟随窗口的是什么 |
|---|---|
| 0 – None | 什么都不跟随：锚定、停靠和容器照常（默认） |
| 1 – Resize only | 每个控件的宽度和高度 |
| 2 – Reposition only | 每个控件的位置 |
| 3 – Resize and reposition | 大小和位置：整个窗体按比例 |
| 4 – Resize the font only | 文字大小 |
| 5 – Resize and resize the font | 大小和文字 |
| 6 – Reposition and resize the font | 位置和文字 |
| 7 – Resize, reposition and resize the font | 一切 |

这些值是可以相加的：1 是大小，2 是位置，4 是文字，所以 7 = 1 + 2 + 4。

**算术。** 如果窗体是按 400 × 300 绘制的，而它的窗口现在是 800 × 450，那么宽度比是 2，高度比是
1.5。一个绘制在 (100, 50)、80 × 30 的按钮：
- 取 **1**，保持它的位置，变成 160 × 45；
- 取 **2**，移到 (200, 75)，保持 80 × 30；
- 取 **3**，两者都做；
- 带有字体标志（4、5、6 或 7）时，它的文字按**较小**的比例 1.5 缩放，所以它始终能放进它的控件。

位置是在父级内部度量的。Panel 或 GroupBox 之内的控件随该容器缩放，而该容器随窗体缩放。一切每次
都是根据你的设计计算出来的，所以把窗口缩小回去，会让每个控件恰好回到你绘制它的位置。

**任何非 0 的值都会对窗体进行布局**，即使 *Responsive* 是关闭的。你不必再打开任何其他东西。

仍然适用的：
- `MinWidth`、`MaxWidth`、`MinHeight` 和 `MaxHeight` 仍然限定控件的大小，而窗口不会缩小到它们
  所需的大小之下。
- `MinFontScale` 和 `MaxFontScale`（除非你更改，否则是 0.85 和 1.5）限定文字，而 `ScaleFont` 关闭的
  控件保持它的文字大小。
- 断点覆盖仍然适用，且在缩放之前。
- 窗口仍然会在你设计中分开的两个控件会碰到一起之前停住。用 *Resize only* 时，增长可能把邻居带到
  一起；用 3 或 7 时一切一起增长，它们从不相遇。

**让某个控件置身事外。** 给控件一个它自己的锚定，或把它停靠起来，那么在窗体的其余部分缩放时，
它会继续那样做。典型的情形是停靠在顶部的工具栏，或锚定在底部边缘的状态行。留着其类型默认锚定的
控件——迁移窗体的每个控件都是如此——会缩放。

**从 COBOL**，窗体可以在运行时更改它自己的样式，例如向用户提供“随窗口缩放”的选项：

```cobol
           IF WS-ZOOM-WITH-WINDOW = "Y"
               MOVE 7 TO me::ObsoleteScalingStyle
           ELSE
               MOVE 0 TO me::ObsoleteScalingStyle
           END-IF.
```

窗体会在下一帧重新排布它自己。0–7 之外的值会被拒绝：样式保持原样，而 `me::ObsoleteScalingStyle`
随后读到的就是那个值。

> 💡 **有时间的时候，把窗体迁移到锚定。** 按比例缩放会把一切同样地拉伸：窗口变高时文本字段变高，
> 而按钮的文字增长到超出任何人所需要的程度。把字段锚定在左右，把按钮锚定到角落，得到的窗体会
> 利用大窗口，而不是放大它自己。从 *Responsive* 开启、样式为 0 开始，并把应该拉伸的东西锚定起来。

> ⚠️ **注意事项。** 不相等的比例会造成变形。宽而矮的窗口中的方形按钮会变成宽而矮的按钮，与它在
> PowerCOBOL 中的情形完全一样。

### PowerDemo3 中的响应式示例

PowerDemo3 有十二个展示响应式布局的窗体，归档在 `forms/Responsive Layout/` 中。从它的侧边菜单的
**Samples ▸ Responsive Layout** 打开它们；每一个都会在你调整大小时，沿它的底边显示窗口大小、活动
断点和字体缩放。

| 菜单条目 | 窗体 | 它展示什么 |
|---|---|---|
| Responsive: anchors and limits | `responsive-anchors-form` | 每一种锚定选择。包括一个其锚定由你用复选框从 COBOL 挑选的块，外加 `MinWidth` 和 `MaxWidth`。 |
| Responsive: docking | `responsive-dock-form` | `Dock` 的 Top、Bottom、Left、Right 和 Fill，按渲染顺序并嵌套，旁边是一个被锚定而不是被停靠的控件。按钮从 COBOL 重新停靠并隐藏面板。 |
| Responsive: flex | `responsive-flex-form` | 每个 Flex 主题一个标签页：方向、`JustifyContent`、`AlignItems`/`AlignSelf`、折行与 `AlignContent`、带限制的 grow/shrink/basis，以及 `Order`。最后一个标签页从 COBOL 重新连接一个容器。 |
| Responsive: grid | `responsive-grid-form` | 轨道大小（`px`、`%`、`fr`、`Auto`、`MinMax`、`Repeat`）、一个 `Repeat(AutoFill, …)` 画廊、显式的单元格和跨度、隐式网格、每一种对齐，以及一个在 1024 px 以下变成两列的数据录入窗体。 |
| Responsive: flow | `responsive-flow-form` | 每一种 `FlowDirection`、`WrapContents` 关闭、`FlowBreak`。窗体本身是一个 Flex 列。 |
| Responsive: breakpoints | `responsive-breakpoints-form` | 五个命名的断点，带有 Stepped 字体和各种覆盖。按钮固定一个断点（`me::Breakpoint`）或替换这张表（`me::Breakpoints`）。 |
| Responsive: font scaling | `responsive-fonts-form` | `FontScaling` 的 None、Fluid 和 Stepped；`MinFontScale`/`MaxFontScale`；`ScaleFont`、`MinFontSize`、`MaxFontSize`；一个 `AutoSize` 标签；被固定的 `me::FontScale`。 |
| Responsive: nested containers | `responsive-nested-form` | 一个 `LayoutMode` 为 Grid 的窗体，容纳 Flow、Flex、一个 TabControl、另一个 Grid 和 Flex 分组。包括每边的内边距，以及一个被断点或被 COBOL 隐藏的单元格。 |
| Responsive: from COBOL | `responsive-runtime-form` | 窗体运行期间写入的每个布局属性。包括移动一个控件并读回它被排布出来的矩形，以及 `me::Responsive` 关闭再开启。 |
| Responsive: collision limits | `responsive-collide-form` | 因为两个控件会碰到一起，窗口在哪里停止缩小和增长，旁边是声明的 `MinFormWidth`/`MinFormHeight`。 |
| Responsive: obsolete scaling | `responsive-obsolete-form` | 一个 PowerCOBOL 风格的窗体，`ObsoleteScalingStyle` 0–7 在运行时切换。停靠的工具栏和锚定的状态行选择退出，而一个标签在 `ScaleFont` 关闭时保持它的字体。 |
| Responsive: dashboard | `responsive-dashboard-form` | 全部合在一起：停靠的页眉（一个 Flex 行）和导航、指标卡片的 Grid、图表和一个数据网格、Fluid 字体，以及两个重新排布它的断点。 |

> 📷 需要截图 — `responsive-dashboard.png`：运行 PowerDemo3，打开
> **Samples ▸ Responsive Layout ▸ Responsive: dashboard**，并截取窗口两次：一次是宽的（约 1400 px），
> 一次是窄的（约 550 px）。把两张并排放置。

这些窗体不得不围绕着构建的四件事，你自己的窗体也会遇到：

- **`fr` 轨道永远不会比你所绘制的它最宽的项更窄。** 容纳一张你绘制为 600 px 宽的卡片的 `1fr` 列，
  不会低于 600 px，于是窗口无法缩小。写 `MinMax(160px, 1fr)` 来说明这一列最窄可以窄到多少。
- **`LayoutMode` 为 Flex 列的窗体或容器，取其内容的高度。** 各项保持它们所绘制的高度，而不是缩小
  到窗口的大小。把应该填满空间的那一项，以它*最小*的有用高度来绘制，并给它 `FlexGrow 1`：这样它
  会增长以填满高的窗口，而矮的窗口仍然放得下。
- **折行容器的最小值，是在窗口的最小宽度处度量的。** 这涵盖了折行的 Flex 或 Flow，以及其列为
  `Repeat(AutoFill, …)` 的 Grid。窗口的最小宽度先被找出来。然后窗体在那个宽度上被排布，容器索要的是
  它的各项在那里实际形成的行（或列）的高度。例如，一个有 12 张卡片的画廊，在最小宽度处每行放四张，
  它索要的是三行，而不是十二行。在窗口的最小尺寸处，什么都不会溢出。折行的 Flex *列*是反过来度量的：
  它索要的是它的各项在窗口的最小高度处所形成的列的宽度。如果没有别的东西设置最小宽度（或高度），
  那个最小值就是一项，所以各项仍然堆叠。给窗体一个 `MinFormWidth`（或 `MinFormHeight`），就能让
  一行容纳几项。
- **Splitter 会计入它的窗格所容纳的东西。** 如果窗格内的某个控件有最小值，例如一个锚定为
  `Left,Right` 且带 `MinWidth` 的字段，窗口就会让分割器保持足够的宽度，使那个窗格在分割器的
  `SplitPosition` 处能够遵守它。

> **注意。** TabControl 的每个页面都是各自、在整个页面区域内排布的：某一页上的 `Dock = Fill` 填满
> 那一页，不从其他页取走任何东西，而 `LayoutMode` 为 Flex、Grid 或 Flow 的 TabControl 会分别排布每一页
> 的控件。它的最小尺寸就是它最大那一页的最小尺寸。

### 在画布上编辑网格

`LayoutMode` 为 Grid 的容器由**轨道**构建：`GridColumns` 和 `GridRows` 列出它的列和行的大小
（`1fr 2fr`、`120px 1fr`、`Auto 30%`），放在其中的每个控件用 `GridColumn` 和 `GridRow` 指明它的
单元格（从 1 开始计数；0 把位置交给布局决定），并且可以用 `ColumnSpan` 和 `RowSpan` 覆盖多个
单元格。你可以在属性窗格中键入所有这些。Form Designer 也允许你在画布上编辑它们，而你在那里所做
的是一次普通的属性更改：它可以用 Ctrl+Z 撤销，保存在窗体中，而 Run Form 显示的恰好就是画布
显示的，因为同一个布局同时摆放两者。

**两种模式，一个按钮。** 设计器的工具栏有一个 **Show grid** 按钮（*Show layout grids and edit
their structure*），紧挨着 **Toggle Grid**——后者是帮助你对齐控件的那些点，是另一回事，保持原样。
每个设计器窗口启动时 Show grid 都是开启的。

| Show grid | 在 Grid 容器内部的一次按下会做什么 |
|---|---|
| **On** | 它作用于**网格**：每个 Grid 容器都画出它的线条，点击选中一个单元格，而放在其中的控件无法在画布上被选中、移动或调整大小。关闭这个按钮才能处理它们。 |
| **Off** | 它作用于**控件**，一如既往——只有一点不同：网格中的控件从它的单元格取得它的大小（见下文*调整网格中控件的大小*）。 |

这个按钮是设计器窗口的设置，不是窗体的设置：它不向窗体写入任何东西。

**网格只在 Form 标签页上编辑。** 在图层的标签页上，窗体的网格仍然显示在图层之下，但不在那里
编辑。在其上的一次按下不属于网格：它不选中任何单元格，不调出网格工具栏，也不抓取任何轨道线。
Non-Visuals 标签页也是如此。选择 **Form** 标签页来编辑网格。

> 📷 需要截图 — `grid-edit-mode.png`：在 Form Designer 中打开 PowerSpatial 的 `main-form`，
> Show grid 开启且没有选中任何东西，使每个 Grid 容器的轨道线都被画出来。截取工具栏（其中
> **Show grid** 按钮高亮）以及它下面的画布。

**选择单元格。** Show grid 开启时，点击一个单元格就选中它。拖过若干单元格，或用 Shift 点击第二个，
可以选中一个单元格矩形。被选中的单元格会着色，并且容器旁边会出现一个小的**网格工具栏**——在它
外面，绝不在它上面。它的按钮只带图标；悬停在其中一个上就能读到它做什么。不适用于当前选择的按钮
会变灰，并且它的提示会说明原因。

| 按钮 | 它做什么 |
|---|---|
| **Add column** / **Add row** | 在所选单元格的列或行之后插入一条轨道（没有选中单元格时则在末尾），大小与它所跟随的那条轨道相同。新轨道之后的每个控件都会被重新编号，使它保持在它的单元格中，而跨越它的控件会增加一格。 |
| **Remove column** / **Remove row** | 选中一个单元格时，移除该单元格自己的那一列或那一行。剩下的轨道按它们通常的规则分享空间，所以 `fr` 轨道会增长以填补空缺。它之后的控件会被重新编号，而跨越它的控件会减少一格。 |
| **Restore** | 放回这个网格最近被移除的列或行：大小相同，位置相同，并带有它所容纳的控件的位置和跨度。 |
| **Merge** | 让所选单元格成为**一个单元格**，不论它们容纳什么。该范围被写入容器的 `GridMerges`。其中一个控件会填满它。几个控件会在其中**并排**排列，各占它宽度的相等一份，互不重叠。空的单元格会保留给你之后放在那里的下一个控件，那个控件随后会占据它们全部。该范围内的每个控件都被写在它所在的单元格上，所以它留在合并后的单元格中。 |
| **Split** | 从合并后的单元格中的任何一个单元格，让它的各个单元格再次分开：每个控件回到写在它身上的那个单元格，而两个被写在同一个单元格里的控件会去往原先那个合并单元格里空闲的单元格。对于靠它自己的 `ColumnSpan` 或 `RowSpan` 跨越多个单元格的控件，它会让该控件的 `ColumnSpan` 和 `RowSpan` 重新变为 1，位于它的左上角单元格中。 |
| **Line color**（色块，最后一个，单独分开） | Form Designer 绘制网格线所用的颜色。点击它从 IDE 的取色器中选择一种；右键点击则回到主题的颜色。它适用于每个 Form Designer 窗口中的每个网格，并保存在这台机器上。它是你看到网格的方式，而不是网格的一部分：它不保存在窗体中，而 Preview 和 Run Form 根本不显示网格线。 |

> 📷 需要截图 — `grid-edit-toolbar.png`：选中一个网格中两个相邻的单元格，并截取网格及其旁边的
> 浮动工具栏，显示 Merge 可用而 Restore 变灰。

> **注意。** 移除列或行**从不删除控件**。只位于被移除轨道中的控件会留在窗体上，并在该轴上变为
> 自动放置（它的 `GridColumn` 或 `GridRow` 为 0）；画布之上的一条通知会点出它的名字。Restore 会把
> 它放回原来的位置。网格的最后一列或最后一行无法被移除。

> **注意。** Restore 不是 Undo。Ctrl+Z 撤销你的上一次编辑，不论它是什么。Restore 单独带回被移除的
> 列或行——自设计器打开以来你所做的任何移除，最新的在前——并让你在它之后做的编辑保持原位。撤销
> 一次 Restore 会再次移除该轨道；撤销一次移除会带回该轨道，并把它从 Restore 的列表中取下。

**合并的单元格。** 合并单元格是网格的属性，而不是控件的属性。容器把它的合并单元格保存在
`GridMerges` 中，你也可以在属性窗格中键入它（*Merged cells*），写成电子表格式的范围：
`R1C1:R1C3 R2C2:R3C3`——行和列从 1 开始计数，从左上到右下，以空格分隔。

- **一个单元格，无论在哪里排布。** 画布绘制合并单元格时，其内部没有线条，而在其中任何地方点击都会
  选中它的整体。伸入合并单元格的选择会包含它的整体。Preview、Run Form 和构建出的应用程序对合并
  单元格的排布，与画布完全一样。
- **其中放着什么。** `GridRow` 和 `GridColumn` 落在合并单元格之内的每个控件，都被放在其中。几个
  控件按阅读顺序并排，各占宽度的相等一份，彼此之间隔着 `ColumnGap`，高度与合并单元格一样高。
  每个控件在它那一份之内，由它自己的 `JustifySelf` 和 `AlignSelf` 对齐，与普通单元格相同。控件
  自己的 `ColumnSpan`/`RowSpan` 在合并单元格内不适用。
- **空的合并单元格会被保留。** 自动放置的控件——没有 `GridRow` 或 `GridColumn`——如果到达它，就
  占据整个合并单元格。合并单元格的其他单元格从不被自动填充。
- **它随它的轨道一起移动。** 在合并单元格之内添加一列或一行会加宽它，而在它之前添加则会把它移
  过去。在其内部移除一个会使它变窄；只剩一个单元格的合并单元格不再是合并的，而 Restore 会把它放回
  原样。
- **当 Merge 变灰时**，它的提示会说明原因：
  - 选中的是单个单元格；
  - 这些单元格已经合并了；
  - 或者这个选择会切穿某个合并单元格，或切穿某个跨越单元格的控件，把它的一部分纳入而把其余的留在
    外面。请选中它的全部，或一点也不选。

**拖动一条线。** Show grid 开启时，轨道之间的线条就是手柄：指针移到其上会变成调整大小的箭头，
拖动它会向相反的方向调整它所分隔的两条轨道的大小，所以容器的总量保持不变。画布跟随指针，而松手
就是一个撤销步骤。每条轨道保持它自己的单位：

| 轨道原来是 | 拖动之后 |
|---|---|
| `120px` | `150px`——一个新的像素大小。 |
| `25%` | 一个新的百分比。 |
| 两条 `fr` 轨道，`1fr 1fr` | 同样的总量按新的比例分享——向左拖动这一对宽度的四分之一，得到 `0.5fr 1.5fr`。 |
| `Auto` | 一条大小为你拖到的大小的像素轨道。 |

没有任何一次拖动会让轨道比 8 px 更窄。容器的边框不是一条线，而 `MinMax(…)` 或 `Repeat(…)` 轨道
没有手柄：指针在其上不会改变。

> 📷 需要截图 — `grid-edit-drag.png`：拖动两列之间的线，并截取松手之前的那一刻——线上的调整大小
> 光标，以及两列处于它们的新宽度。

**调整网格中控件的大小。** Show grid 关闭时，选中位于 Grid 容器中的一个控件。它所在容器的线条出现
——只有那个容器的——并且该控件的单元格着色，而即使 Show grid 是关闭的你也可以拖动它们：在响应式
窗体上，那个网格就是决定控件大小的东西。控件自己的手柄从另一边做同样的工作：拖动它的右边缘会移动
它最后一列之后的那条线，它的左边缘移动它第一列之前的那条线，而下边缘和上边缘对行做同样的事。一个
角会在每个方向上各移动一条线，作为一个步骤。对于跨越多条轨道的控件，移动的是它跨度的外侧那条线。
位于容器边框上的边缘不移动任何东西。在控件的其他任何地方按下，仍保持它通常的含义：把它拖到另一个
单元格以把它放在那里。

一次手势从不会给这样的控件它自己的位置或大小。它的大小就是它的单元格的大小。

窗体运行期间，同样的属性对你的程序也是开放的，所以处理程序可以像你在画布上那样改变网格的形状。
PowerDemo3 的 *Responsive: from COBOL* 窗体用类似这样的一行来做到这一点，其中 `RT-STAGE` 是 Grid
容器：

```cobol
           SET     RT-STAGE::GridColumns         TO    "1fr 2fr"
```

> ⚠️ **注意事项。**
> - `Repeat(…)` 列表被写成一个条目，所以在该轴上添加或移除轨道的按钮对它是禁用的，并且提示说要在
>   属性窗格中编辑该列表。它的线条也没有手柄：指针在其上不会改变。
> - 以不同于你设计它时的比例绘制的网格——例如展开的卡片——无法拖动它的线条；请在它自己的大小下
>   编辑它。
> - 这涵盖的是 Grid **容器**——`LayoutMode` 为 Grid 的 Panel、GroupBox 之类。窗体自己的网格，即窗体
>   本身被排布为 Grid 时，仍然在属性窗格中作为文本来编辑。
> - Flex 和 Flow 容器没有单元格，所以网格工具不适用于它们。

### 目标设备

**Target Device** 部分让你按真实的设备配置（各种 iPhone、iPad、Apple Watch、Android 手机/平板/手表
预设）或自定义大小来设定窗体的大小，并带有纵向/横向切换。这是一种设计辅助——它把窗体的宽度/高度
设为所选的配置。

> 📷 **需要截图 — `form-designer-full.png`。** 带有工具箱的 Designer，画布上放着若干控件（一个
> 标签、一个文本框、一个按钮和一张图表），以及显示着分区卡片的属性窗格。最好使用带背景图像的项目，
> 以便 Neumorphic 或玻璃样式可见。

> **注意（非可视控件）。** Timer、AI Agent、REST Client、SQL Database、Indexed File、WebSearch 和
> Snackbar 是**非可视的**：它们在设计时作为带标签的玻璃“芯片”出现在画布上，但在运行时不渲染任何
> 东西。它们的存在是为了被配置，并引发事件/被你的 COBOL `CALL`。
>
> 每个芯片都带有它自己的字形，以及一个报告你最需要一眼看到的那项设置的标题：Timer 的间隔、AI Agent
> 的模型、REST Client 的默认方法、SQL Database 的驱动、Indexed File 的打开模式、Snackbar 的类别，以及
> WebSearch 控件的搜索引擎 id——在你设置 `SearchEngineId` 之前它显示 `no engine`，因为没有它，该控件会
> 通过 `onError` 作答而不是去搜索。字形和标题是针对它们所处的卡片着色的，所以它们在浅色窗体主题上
> 和在深色主题上一样清晰。

### 图层与 Non-Visuals 标签页

窗体是一张平面的纸。当一个界面需要不止一张——数据录入页面之上的帮助叠层、一块“请稍候”的幕布、
各步骤共用一个框架的向导——你就把 Panel 一个叠一个地堆起来，并手工显示和隐藏它们。**图层**就是
把这个内置了：一整张透明的纸，躺在窗体之上，带有它自己的名称、它自己的背景和它自己的控件，你的
COBOL 用一条语句就能显示和隐藏它（§11）。窗体可以在它的**基础层**——一如既往的窗体，叫做
`Form`——之上最多有 **64** 个图层。没有图层的窗体，恰好就是你以前有的那个窗体；关于图层的任何东西
都不会写入它的文件。

在**每个**窗体的画布下面，都有一条**标签栏**：

📷 **需要截图 — `layers-tab-bar.png`。** 打开一个窗体，点击 **+** 两次，选中 `Layer-1`，往它上面放
一个 Button，关闭 `Layer-2` 的眼睛，并截取画布下面的那一条，连同它上方的画布：`Non-Visuals`、`Form`、
`Layer-1`（蓝色，被选中，带有睁开的眼睛和红色的 ✕）、`Layer-2`（白色，带有闭上的眼睛）和 `+`。

| 标签 | 它是什么 |
|-----|------------|
| **Non-Visuals** | 位于最左端的固定标签，始终存在。每个非可视控件——Timer、AgentObject、RestClient、SqlDatabase、IndexedFile、KnowledgeBase、WebSearch、Snackbar 和 AWS 控件——都在这里，不在别处。每个都是一张卡片，它的**名称**在卡片下面，**类型**在名称下面，居中，深色底板上的白字，使它们在任何背景上都能读出来。 |
| **Form** | 基础层。它不能被移动、重命名或删除。像图层一样，它有一只**眼睛**：默认是睁开的，所以它的控件是显示的；把它闭上，就能在设计时隐藏窗体自己的控件（你**锁定**的那些会保留——见下文）。它没有 ✕。 |
| **Layer-1, Layer-2 …** | 每个图层一个标签，从左到右，由最靠近 `Form` 的图层到最顶上的那个。每个都有一只**眼睛**——图层显示时睁开，隐藏时闭上——以及一个红色的 **✕**。 |
| **+** | 添加一个图层。 |

当图层多得超出了标签栏，`Non-Visuals` 留在原处，右边出现一个 **◀** 和一个 **▶**，其他标签在它们下面
继续延伸。按一个箭头让标签移动一步，或者在标签栏上**横向轻扫**——触控板上的两根手指，或鼠标滚轮——
让它们滑行。标签栏在第一个标签和最后一个标签处停住，而这两种手势都不会选中某个标签或改变窗体。

标签是倾斜的，彼此相接，并且位于一条深色的栏上，栏的上方有一条细的白线。不论你用什么 IDE 主题，
**活动**标签都以蓝色绘制、白字，而其他所有标签是白色、蓝字。你**一次编辑一个标签页**：

- **指针只看得见活动的标签页。** 点击、Ctrl 点击、拖动、橡皮筋套索、上下文菜单，以及 **Select All**
  （⌘A / Ctrl+A），都只触及活动标签页的控件；其他标签页的控件，如果它们的图层是显示的，会被绘制出来，
  但表现得就像它们不在那里一样——点在其中一个上就是点在空白画布上。你创建、粘贴或复制的控件落在活动
  标签页中。
- **Objects 列表跟随标签页。** 左侧栏中的列表只列出活动标签页的控件，所以你点击的一行总是你在画布上
  能够触及的控件；选择另一个标签页，列表也随之改变。在 `Non-Visuals` 上，它列出非可视控件。
- **每个标签页都有它自己的 Tab 顺序。** 键盘路径（见 *Tab 顺序与 Enter 键*）在**每个标签页之内**从 1 开始
  编号，所以 `Form` 和每个图层各有一个编号为 1、2、3……的控件。你创建、粘贴或移入某个标签页的控件，
  取*该标签页中*的下一个编号。**Visual Tab Order** 和 **Tab Order list** 只对活动的标签页起作用。程序
  运行时，Tab 先走 `Form`，然后是每个显示着的图层，从最低的到最顶上的，每个都按它自己的编号；隐藏图层
  的控件会被跳过。
- **选中一个图层就会显示它。** `Form` 始终被绘制，而你选中的图层被绘制在它之上，如果它的眼睛是闭着的，
  就会把它睁开。每个*其他*图层都被隐藏，因为你正在编辑这一个；之后可以睁开任何眼睛以一次看到更多，
  而在你选中另一个图层之前，没有什么会再次把它隐藏。
- **眼睛从不改变标签页。** 点击一只眼睛——活动图层自己的或别的图层的——只是显示或隐藏那个图层。眼睛
  闭着的图层不会被绘制，在你把它睁开之前，它的控件无法触及。
- **`Form` 标签页也有一只眼睛。** 它显示和隐藏**窗体自己的控件**——所有不在图层上的东西——默认是睁开的。
  **Locked** 属性打开的控件**始终保持显示**，不论眼睛怎么说（把你想在设计其上方的图层时保持在视野中
  的边框、卡片和背景锁定），其内含有被锁定控件的容器也是如此，它留下来是为了让被锁定的控件有地方
  可画；它里面的其他控件则消失。眼睛闭着时，被隐藏的控件无法被点击、套索或选中，而容纳过其中一个的
  选择会失去它。选择 `Form` 标签页会再次睁开它的眼睛；选择某个图层或 `Non-Visuals` 则让它保持原状，
  而图层的眼睛和控件从不受影响。
- **选择标签页和点击眼睛都不是编辑。** 窗体不会被标记为已修改，没有撤销步骤，而不论你让什么保持显示，
  保存的文件都是一样的。程序运行时，图层一开始是**隐藏的**（§11）。
- **只有在 `Form` 上才能调整窗体的大小。** 在图层或 `Non-Visuals` 上没有调整大小的抓手，窗体的 **Width**、
  **Height**、**Target** 和 **Orientation** 行是只读的。

| 要…… | 这样做 |
|------|---------|
| 添加图层 | 点击 **+**。新图层被命名为 `Layer-1`、`Layer-2` ……，完全透明，并被选中。第 65 个会被拒绝并给出一条消息。 |
| 重命名图层 | 双击它的标签，输入，按 Enter。每个点名它的控件，以及你的处理程序和过程中的每个 `OLD::Visible`，都会随之变化。如果名称不是 COBOL 风格的名称、是任何大小写形式的 `Form` 或 `Non-Visuals`，或已被某个控件或另一个图层使用，名称就会被拒绝——而且什么都不会改变：图层和控件共享**同一套**名称。 |
| 重新堆叠图层 | 拖动一个图层标签，把它放在其他标签之间。图层绘制在它之前的各图层之上。标签永远不会跑到 `Form` 的左边。 |
| 把控件移到某个图层 | 选中它们，在属性窗格中选择 **Layer** 行，或右键点击 ▸ **Move to layer**。容器带走它里面的一切；容器之内的控件不能单独移动。移入图层时，控件会失去它的 **Dock**，而它的 **Anchor** 回到默认值（Undo 会把它们还回去）。 |
| 删除图层 | 点击它的红色 **✕**。该图层被选中，并且一个窗口告诉你它的名称，以及有多少控件和事件处理程序会随它一起去。**Delete** 移除该图层、它上面的每个控件和每个控件的处理程序；**Cancel** 让一切保持原样。一次 **Undo** 就能把一切带回来。 |

📷 **需要截图 — `layer-delete-confirmation.png`。** 选中一个容纳着几个带处理程序的控件的图层，点击它的
红色 ✕，并截取确认窗口。

这其中的每一项都是单个撤销步骤。删除图层所走的路，与删除一个控件相同：它的处理程序进入窗体的回收站，
而仍然*引用*某个被删除控件的**公共过程**不会被删除——它留下来，而 Output 面板会告诉你是哪一个，这样你
就可以修正这个引用（在你修正之前，窗体不会构建）。

**运行时的指针。** 点击首先交给最近的图层。如果某个图层在那一点上没有带有已绘制部分的控件，点击就
落到下面的图层，一路落到 `Form`；如果它命中了某个控件，该控件就得到它——而如果该控件没有处理程序，
点击就被直接丢弃，不会再往下落。图层自己的背景从不运行处理程序。如果图层的背景在那一点上是**不透明的**，
它下面的任何东西都收不到这次点击。

```mermaid
flowchart TD
    CLICK["点击窗体上的某一点"] --> TOP["取最上面显示的图层"]
    TOP --> HIT{"这个图层的某个控件<br/>在该点上有绘制？"}
    HIT -- "是" --> RUN["那个控件得到这次点击：<br/>它的处理程序运行，或点击被丢弃"]
    HIT -- "否" --> OPAQUE{"该图层的背景<br/>在那里是不透明的吗？"}
    OPAQUE -- "是" --> STOP["点击在此停止 —<br/>下面的任何东西都收不到它"]
    OPAQUE -- "否" --> NEXT{"下面还有别的图层吗？"}
    NEXT -- "是" --> TOP2["取它，再问一次"] --> HIT
    NEXT -- "否" --> FORM["Form：它的控件，然后是它自己的背景"]
```

选中了图层而没有选择任何控件时，**属性窗格**显示该图层：它的 **Name** 和窗体所有的背景属性——颜色、
渐变、图像和模式、透明度——别无其他。图层没有圆角半径（它的圆角就是窗体的），没有窗口属性，也没有
它自己的事件。

**绘制与布局。** 图层的每个控件都绘制在它下面各图层的每个控件之上，不论它们的 `ZOrder`；`ZOrder`、
**Bring to Front** 和 **Send to Back** 只对共享同一个图层和同一个容器的控件排序。这个顺序涵盖控件绘制的
一切：GroupBox 的标题、TabControl 的标签条、圆角 Panel 的圆角以及 DataGrid，都被绘制在容器自己在堆叠
顺序中的位置上，所以在这个顺序中位于它之上的控件会盖住它们。同样的顺序决定谁来响应鼠标：当两个控件
重叠时，点击、滚轮转动或悬停由位于前面的那一个——以及它所在的容器——报告，而不是由它后面的控件报告。
在该点上没有任何绘制的控件，例如远离它文字的 Label，或带有 `HideBackground` 的 Panel，会让鼠标穿过去。
只有 `Form` 会被排布：图层中的控件保持你所设计的 `X`、`Y`、`Width` 和 `Height`，它的属性窗格不提供 Dock、
Anchor 或布局。图层恰好覆盖窗体——它从不改变窗体的大小——而图层控件在窗体之外绘制的任何东西，都在窗体
的边缘被裁掉。

**Non-Visuals 标签页。** 运行时不绘制任何东西的控件，在窗体上没有位置，所以它们坐在自己的标签页上，
成为**卡片**：

📷 **需要截图 — `non-visuals-tab.png`。** 添加一个 Timer、两个 RestClient 和一个 SqlDatabase，选择
`Non-Visuals`，并截取卡片的网格。

- 卡片填充一个**五列**的网格，逐行排列，按类型以 A–Z 顺序分组（按类型的英文名称，所以在每种 IDE 语言中
  顺序都相同），类型之内按名称 A–Z。网格决定每张卡片的位置：你可以用点击、Ctrl 点击、套索或 Select All
  选中一张卡片，但不能拖动它，而且它没有调整大小的旋钮。当行数多得放不下时，标签页会滚动；窗体和窗口
  不会增长。
- 在 **Toolbox** 中按下一个非可视控件，标签页就会替你打开，使你看到它落在哪里。`Non-Visuals` 处于活动
  状态时，Toolbox 中的可视条目会变灰——请先选择 `Form` 或某个图层——并且包含任何可视控件的粘贴会被
  拒绝。
- 卡片的属性窗格是该控件自己的——Properties、Events 和 Procs——不含位置和大小，它们归网格所有。按
  **Delete**（或使用上下文菜单），以与任何控件相同的途径移除一张卡片，带有的代码若有则同样需要确认，
  并且同样是单个撤销步骤。
- 已经有非可视控件的窗体，在打开时会在网格中显示它们。它们在文件中的 `X` 和 `Y` 保持不变。

**智能体与图层。** IDE 的 AI 智能体（§5，*Agents Manager*）处理图层的方式与你相同。每次请求时，
**Form Designer Agent** 都会被告知窗体有哪些图层——按堆叠顺序——每个图层容纳什么，以及每个控件所在的
图层，所以它可以

- 添加、重命名、删除和重新堆叠图层，并设置它的背景（`Transparency`、`BackgroundColor`、渐变和图像属性）；
- 直接把控件创建到某个图层中，或把已有的控件送到某个图层——或送回 `Form`；
- 在它做不到时坦率地告诉你：当你要求的是图层时，智能体绝不会用 Panel 来假冒图层。

像 *"Add a layer called HELP with a dark, half-transparent background, and put a Label and a Close button
on it"* 这样的请求，会成为一次经过评审的更改。智能体遵循你的规则：同样的名称规则（绝不是 `Form` 或
`Non-Visuals`，也绝不是某个控件已经使用的名称）、64 个图层的上限、容器带着它的子项，以及非可视控件不
放图层——它们自己会落在 `Non-Visuals` 上。被规则拒绝的操作会被跳过并报告，而更改的其余部分仍然应用。
**删除图层会删除它上面的控件**，所以智能体不能在同一次更改中既删除某个图层、又往其中放入控件——它会被
告知在两者中去掉一个。无论智能体触及了多少图层和控件，整个更改都是**一个撤销步骤**。

> **说明。**
> - 图层用于你显示和隐藏的*结构*。对于一次性的强调，`Panel` 仍然是合适的工具，而图层*之内*的 Panel
>   也没有问题。
> - 从 PowerCOBOL 过来：它的窗体里没有与图层对应的东西；最接近的习惯是你显示和隐藏的整个窗体大小的
>   Panel。区别在于，图层还带有它自己的背景，并按上述规则接收鼠标。
> - 带有图层的窗体，在其他一切方面都像以前一样打开、保存和构建：没有图层的窗体保持不变，逐字节
>   一致。
>
> **⚠️ 注意事项。**
> - 点名了一个它没有定义的图层的 `.cfrm`，会保留该控件，在 `Form` 上显示它，并在 Output 面板中告诉你；
>   一个把同一个名称出现了两次的文件会被报告，并保持原样——不会以删除来修复任何东西。
> - 在 **Corner radius** 很大的窗体上，有七种控件类型无法被裁到圆角上，可能会稍微画到圆角之外，在图层
>   中与在 `Form` 上一样：DataGrid、FileDropZone、Maps、TabControl、ToolBar、Viewer 和自定义的插件控件。
>   其他一切都留在圆弧之内。
> - 图层的**背景图像**对鼠标来说不被视为不透明（它的像素不被检查）：要阻止点击到达下面的东西，请在
>   图像之上放一个不透明的 Panel。
> - IDE 的 **Check** 不会查看 `LAYER-NAME::property` 引用的内部；拼错的图层或属性会在你 Build 或 Run
>   Form 时被报告。

### 控件动画

一个控件可以带有任意数量的**动画**，在属性窗格的 **Animations** 卡片中编辑。每一个都有：

| 字段 | 它做什么 |
|-------|--------------|
| **Name** | 你的 COBOL 如何启动它。新动画被命名为 `anim1`、`anim2`、…… |
| **Trigger** | 它何时播放：`OnFormLoad` / `OnShow`（窗口出现时）、`OnClick`、`OnHover`、`OnFocus`、`OnTimer`（所选 Timer 的每一次滴答），或 `Programmatic`（只在你的代码要求时）。 |
| **Kind** | 运动方式：从某一侧或某个角飞入、淡入/淡出、放大/缩小、弹跳、抖动、脉冲、旋转、翻转，或按 **Slide DX / Slide DY** 个像素滑动。 |
| **Duration (ms)** | 一遍要花多长时间。 |
| **Delay (ms)** | 在**第一**遍之前要等多久。 |
| **Easing** | 一遍的速度曲线。 |
| **Repeat** | `Once`、`Loop`（重新开始）、`PingPong`（先正向播放，再反向）或 `Count`。 |
| **Passes** | 当 Repeat 为 `Count` 时显示：它播放多少遍（默认 3），然后停在末尾。 |
| **Repeat delay (ms)** | 当 Repeat 不是 `Once` 时显示：控件在一遍与下一遍之间，停在这一遍结束处**休息**多久。`0` 会立即开始下一遍。 |

一个每秒跳动一次、两次跳动之间有停顿的、脉冲式的“新消息”徽章，就是一个 Duration 为 400、Repeat 为
`Loop`、Repeat delay 为 600 的 `Pulse` 动画。从处理程序中启动一个 `Programmatic` 动画：

```cobol
           INVOKE Lbl-Badge::PlayAnimation("anim1")
      *> ...and later
           INVOKE Lbl-Badge::StopAnimation()
```

设计器画布、窗体预览、**Run Form** 和构建出的应用程序，都通过同一个时钟来步进动画，所以循环及其停顿在
各处看起来都一样。

控件通常会在它所在容器的边缘被裁掉。让它**比它的位置更大**的动画——带有弹性缓动的 `ZoomOut` 或
`Pulse` 在稳定下来之前会膨胀超过 100 %——是唯一的例外：只要它比它的位置更大，它就会被绘制在它的
容器的边缘之上，它的投影也是如此，而不是沿着网格被切掉；它一回到它的大小，或更小，容器就再次裁剪它。
（在 1.90.18 之前，在网格或 Flex 行的边缘过冲的卡片会显示为被切掉。）

---

## 8. 控件目录

PowerRustCOBOL 附带以下控件。可视控件在运行时渲染；非可视控件则是服务。

**Common / input**
: Label、Button、TextBox、CheckBox、RadioButton、ComboBox、ListBox、
NumericUpDown、DateTimePicker、Slider、ProgressBar、PictureBox、**Switch**、
**Knob**、**Gauge**、**FileDropZone**。
**TextBox** 遵从五个输入属性，PowerCOBOL 开发者会立刻用到它们：


| 属性 | 它做什么 |
| --- | --- |
| `Picture` | **文本框内容所遵循的 COBOL `PICTURE`**——见下文。 |
| `ReadOnly` | 显示它的值，允许你选中并复制它，但不接受编辑——也不触发 `onChange`，因为什么都没有改变。这是*只读*，而不是*禁用*：被禁用的字段连选中都不行，而你的 COBOL 仍然可以写入 `Text`。 |
| `PasswordCharacter` | 把值绘制成**你所选的字符**，值中的每个字符对应一个。值本身不受影响：`Text` 仍然保存所输入的内容，所以你的程序可以正常读取密码。 |
| `MaximumLength` | 输入到这么多个字符就停止。`0`——默认值——表示没有限制。设置了 `Picture` 时被忽略：图片自己的宽度就是限制。 |
| `ScrollBars` | 在 **Multiline** 文本框上，取 `None` / `Vertical` / `Horizontal` / `Both`。`None` 仍然会滚动；它只是不画滚动条，所以文本框显示不下的文字永远不会变得无法触及。`Horizontal` 和 `Both` 会让文本不再折行，于是就有了可以横向滚动过去的东西。 |

**`Picture`——文本框保存的就是数据项保存的东西。** 把它设成一个 COBOL picture
（`9(6)`、`ZZ9.99`、`A(20)`、`X(30)`、`$$,$$9.99CR`），随之就有两件事。

它在你输入时**逐个字符位置地验证**：`PIC A(3)` 接受字母和空格，`PIC 9(3)` 接受数字，
`PIC X(3)` 接受任何字符。这是 COBOL-85 对 `A`、`9` 和 `X` 的理解，而不是一个宽松的理解。输入
仍然是普通文本——文本框**不会**预先填入分组字符，也不会让你把插入符走过它们。你输入
`1234.56`；由文本框来判断每一次按键是否被允许。

它还会**遮罩**：数值编辑型的 picture 在文本框没有获得焦点时显示它的编辑后形式，获得焦点时
显示朴素的存储值。保存着 `12.34` 的 `PIC ZZ9.99` 在静止时读作 `" 12.34"`——一个前导空格，因为
该 picture 宽六个字符位置——而在插入符之下则是 `12.34`。

小数分隔符和货币字符来自**窗体的 `SPECIAL-NAMES`**，而不是来自 picture，所以在 `DECIMAL-POINT IS
COMMA` 之下，逗号是小数点，句点用于分组。运行中的窗体和它生成的 COBOL 不会在这一点上产生分歧。

最重要的是，**生成的数据项带有同样的 picture**。带有 `PIC 9(6)V99` 的文本框会生成一个
`PIC 9(6)V99` 数据项，所以针对它的算术和比较都遵循 COBOL 自己的规则——运行时没有任何东西在你
背后被转换。

> **注意。** 让 `Picture` 为空表示“未设置”，而文本框的表现与它一贯的完全一样：有效的 picture 是按
> `MaximumLength` 定大小的 `X(n)`。在这个属性出现之前构建的窗体不受影响。

**Containers / layout**
: GroupBox、Panel、TabControl、Splitter、MenuBar、ToolBar、StatusBar、
**SideMenu**。
**GroupBox、Panel 和 TabControl 是真正的容器**——见下文*容器与嵌套*。
**Splitter 是被一分为二的面板**——是一种容器，与它上面的三个一样。放一个下去，你会在树中得到
**三个**控件：分割器本身，以及它所拥有的两个窗格 `<id>-Pane1` 和 `<id>-Pane2`。这两个窗格是
普通的 Panel——起初没有边框且是透明的——所以你往里面放控件，对它们设置样式、绑定它们，与你对
任何 Panel 所做的完全一样。你**不**设置的是它们所处的位置：由分割线来决定。

- **Orientation** 指明**窗格**如何排列，而不是线条如何延伸。`Horizontal` 把**窗格 1 放在左边、
  窗格 2 放在右边**，由一条垂直线分开；`Vertical` 把**窗格 1 放在上面、窗格 2 放在下面**，由一条
  水平线分开。
- **SplitPosition** 是分割器内部宽度（Horizontal）或高度（Vertical）的**百分比，0–100**。因为它是
  比例而不是像素偏移，所以当窗体或分割器被调整大小时，分割保持在你放置它的地方。你的 COBOL 可以
  读取它——`MOVE Splitter-1::GetProperty("SplitPosition") TO WS-N`——或设置它：
  `SET Splitter-1::SplitPosition TO 30`。
- **边框**是分割器自己的 `BorderStyle`、`BorderWidth` 和 `BorderColor`，而窗格位于它的里面：它们按
  `BorderWidth`（至少 2 点）向内缩进，所以宽的边框永远不会被窗格或你放在其中的东西盖住。
- **拖动线条**——沿着它的任何地方，而不只是在抓手上——两个窗格就会在指针之下重新分配。光标在线条
  之上会变成**抓手**，而**双击它会把分割放回 50 %**。同样的手势在设计器画布上和运行中的窗体里都有效。
- **0 % 和 100 % 是合法的。** 一个窗格完全关闭，另一个容纳一切；抓手会被分割器自己的边缘裁剪，所以
  它的一半保持可见，可以用来把它拖回来。
- **给线条设置样式**：`LineColor` 和 `LineSize` 用于那条线，`GripStyle`（`FilledPill`、`HollowPill`、
  `FilledCircle`、`HollowCircle`）、`GripSize` 和 `GripColor` 用于手柄。把某个颜色留空，它就跟随窗体
  主题。面板本身也跟随主题，直到你设置 `BackgroundColor`、`BorderStyle` 或 `BorderColor`。
- **线条移动时其内容如何表现**，由每个窗格自己选择，在窗格上（而不是分割器上）设置为
  **Pane Left/Right Resize Behavior**：

  | 行为 | 该窗格内的控件会做什么 |
  | --- | --- |
  | **Translate with divider**（默认） | 每个控件都保持它与分割线的距离，在两个窗格中皆如此：把线向右拖 40pt，两半中的一切都向右移动 40pt。控件可能被带过它所在窗格的远端边缘，在那里被裁剪。 |
  | **Scale within the pane** | 每个控件保持它在窗格中位置的*比例*，所以窗格增大时内容会散开，缩小时则挤在一起。大小从不缩放——只缩放位置——所以什么都不会变形，也没有什么会离开窗格。 |
  | **Anchor to the outer edge** | 每个控件保持它与它所在窗格自己前缘的距离。窗格 1 的前缘就是分割器的前缘，从不移动，所以它的内容原地不动；窗格 2 的前缘*就是*分割线，所以它的内容随它一起移动。这就是普通容器的行为方式。 |

  两个窗格是独立的——一侧是固定的控件条、另一侧是可缩放的画布，不过是一个窗格设为 *Anchor*、另一个
  设为 *Scale*。

  **窗格内的容器会带着它的内容**：你放入窗格的 Panel、GroupBox 或 TabControl 作为一个整体移动，它的
  控件随它一起走——在 *Scale* 之下也是如此，容器取它的比例位置，而其中的一切刚性地跟随它，而不是
  从它里面散开。无论嵌套得多深，这一点都成立，**包括窗格内的分割器**：内层分割器随外层的分割一起
  移动，而它自己的窗格及其内容随它一起走。

  **在响应式窗体上**，窗口的布局摆放分割器，而分割器摆放它的两个窗格。然后每个窗格在它实际收到的
  空间内，排布它自己的控件，与任何 Panel 的做法相同。所以 `Anchor`、`Dock`（包括 `Fill`）以及窗格
  自己的 `LayoutMode`（Flex、Grid、Flow）在窗格内都适用。窗格内停靠为 `Fill` 的控件会填满它，而锚定
  为 `Top,Left` 的控件在两个轴上都保持它与窗格角落的距离。上面的 Resize Behavior 只在窗体运行期间
  线条移动时才适用。

  > **窗格从不调整其中内容的大小。** 移动分割只改变窗格自己的矩形和其内容的*位置*——绝不改变它们的
  > `Width` 或 `Height`（只有响应式的窗口调整大小才会，并且是通过控件自己的锚定和停靠）。窗格是一个
  > **视口**：对它来说太大的控件会被窗格的边缘裁剪，而不是被缩小以适应。

  **在设计器中**拖动分割，**确实会移动控件**：它们的 X/Y 被改写并保存，而整个拖动——线条和它所带动的
  一切——是单个撤销步骤。

> **注意**——窗格自己的矩形是由分割推导出来的，所以手工移动或调整窗格的大小什么也不会做：它会立刻弹回
> 去。移动**分割器**来移动两个窗格，并拖动**线条**来改变它们的份额。

> ⚠️ **在 1.61.164 中有变化。** 在此之前，Splitter 是*两个相邻控件之间的一条栏*，而 `Orientation` 指的是
> 这条栏自己的方向——`Horizontal` 意味着一条分隔上下的宽栏，与它现在的含义相反。较早保存的窗体打开
> 时，它的窗格方向是反的，而它的 `SplitPosition`（过去是像素偏移）被重置为 50 %。请设置你想要的方向，
> 并把线条拖回原位——这是一次性的纠正，你放在窗体上的任何东西都不会丢失。

**StatusBar** 的宽度始终就是它的窗口的宽度。你不设置它的 `X` 或 `Width`——它们是窗体的，它们自行跟随
窗体的调整大小（并且在运行时，跟随操作员拉宽的运行窗口），而设计器把它们显示为灰色，只提供上、下两个
调整大小的旋钮。它的 `Y` 和 `Height` 仍然是你的：它位于底边的哪个位置，以及有多高，由你决定。它也是
**唯一不能放进容器的控件**——把它放到或拖到 Panel、GroupBox、Splitter 窗格或标签页之上，它依然属于
*窗体*，不会有任何容器亮起作为目标。状态栏报告的是窗口的状态，所以比窗口窄、或在面板里被裁剪的条，
就不是状态栏。（这不同于 MenuBar 的 `MenuBarStyle`，后者是一种选择，默认是你所绘制的宽度；状态栏没有
这样的选择。）

它的 **`Items`**——每行一个文本——从左到右，以栏自己的 `ForegroundColor`、字体和字体样式书写，位于它的
`BackgroundColor` 所给予的底面之上。设计器画布显示的恰好就是运行中的窗体所显示的。

> ⚠️ **注意事项。** 在 1.70.233 之前，运行中的栏会忽略它的设计：它画的是一条固定的海军蓝的条，配固定的
> 浅色文字，而画布只显示一个“▬ StatusBar”的替身。你从未着色的栏现在穿着与其他控件相同的默认外观；
> 如果你想要一条色带，请给它一个 `BackgroundColor`。

> **SideMenu** 是唯一一个改变整个应用程序启动方式的控件：把它放在主窗体上，应用程序就会以带有导航
> 侧栏的*外壳*打开，而不是每个窗体一个窗口——见
> [应用外壳](#22-应用外壳与-super-接收者)。

**Data**
: DataGrid、TreeView。

**Graphics / media**
: Line、Shape、Animator、**Maps**。
**Shape** 绘制一个矩形、一个圆或一个三角形。它在 *Basic properties* 中有它自己的 **FillColor**、
**FillStyle**、**LineColor**、**LineStyle** 和 **LineThickness**，并且还遵从 *Appearance* 中的
**Background gradient**：勾选它，渐变就在填充之上起主导作用，三种轮廓上皆如此。圆和三角形是沿着形状
本身着色的，而不是穿过绘制在它们周围的方框，所以 Radial 渐变在每一种上都读起来正确。不勾选它，形状
就穿着 **FillColor**（或者在你没有设置 FillColor 时，穿着 Appearance 的 **Background color**）。

**Charts**
: BarChart、LineChart、PieChart、AreaChart、ScatterChart、DonutChart、RadarChart。
每张图表都有一个 **Hide background** 属性：勾选时，图表面板的填充和边框都不绘制，只显示图表内容
（网格、坐标轴、标签、数据）——让图表透明地置于窗体之上。不到隐藏的程度，图表的 **Transparency** 只会
淡化它的背景——数据标记、坐标轴、图例和边框保持实心，所以透明的图表仍然可读。在你选择
**BackgroundColor** 之前，那个背景就是窗体主题的卡片，并且它采用每个控件都提供的同样的
**Background gradient**（勾选它，然后是起始颜色、结束颜色和一个罗盘方向）——渐变与图表的
Transparency 一起淡化，与纯色完全一样。它的边框是与任何其他控件一样的真正的边框
（**BorderStyle**、**BorderWidth**、**BorderColor**，遵循 **CornerRadius**），并且在属性窗格中有两个
仅限图表的额外项：勾选 **Border gradient**，让边框沿着一个罗盘方向从起始颜色渐变到结束颜色；并设置
**Border blur**（像素），沿边框的颜色向外产生柔和的光晕。**Border transparency**（0 = 不透明，
100 = 不可见）会把线条、渐变和模糊一起淡化——它自己独立于图表的 Transparency（后者只触及背景）。
`BorderStyle` 为 `None` 会把这一切全部移除。
图表还有一个 **Monochrome** 模式：勾选它，并从 256 色的色板选择器中选一个**基础颜色**，图表就会用那一种
颜色的、可区分的色调变化来渲染它的数据，而不是多色调色板。网格线和坐标轴线变成柔和的粉彩变体，而
扇区/柱的边框变成更浅/更深的变体；标签、图例和标题保持前景色，而面积图/堆叠的透明度不变。网格的
可见性仍由现有的 **Show grid lines** 开关控制。**Gradient** 选项给每个数据元素它自己的 ±20% 的色调渐变
（柱子垂直着色；散点气泡和饼图/环形图扇区则是径向着色），而折线图和面积图得到的是一个垂直的填充，在
线条处明亮，朝基线逐渐淡去。基础颜色选择器包含一列灰色。折线图和面积图遵从 **Smooth** 属性
（Catmull-Rom 曲线）。
图表还遵从它自己的**标题、标签和图例**：

| 属性 | 它做什么 |
| --- | --- |
| `Title` | 印在绘图上方的标题。为空则不画，也不占空间。 |
| `TitleFontSize` | 标题自己的磅值。**0**——默认值——让它跟随图表的 `FontSize`。绘图上方的带随它增长，所以大标题会占用空间，而不是印在数据之上。 |
| `TitleColor` | 标题自己的颜色。**为空**——默认值——保持自动选择：在能承载深色的底面上读作深色，不能时则切换到可读的那一极。 |
| `XAxisLabel` / `YAxisLabel` | 自由文本的坐标轴标题。边距中会为它们预留空间，所以标题从不横穿数据。为空表示没有标题，也不占空间。 |
| `ShowLegend` | 饼图或环形图旁边的扇区名称；柱状图、折线图、面积图、散点图或雷达图下面的系列名称。**默认勾选。** |
| `ShowLabels` | 每个饼图/环形图扇区上的一个标签。**默认勾选。** |
| `LabelFormat` | 该标签写什么：`percent`（扇区的占比）、`value`（数值）或 `label`（它的名称）。 |
| `SeriesColors` | 各系列的颜色（饼图的各扇区），以逗号分隔，按顺序。预置的列表表示“未选择”——在你更改它之前，由主题的调色板为图表着色。 |
| `SeriesLabels` | 图例给各系列起的名称，以逗号分隔；没有名称的系列显示为 `Series n`。 |
| `ShowPoints` | 折线图**或面积图**每个点上的标记，以及雷达图多边形的每个顶点上的标记。**默认勾选。** |
| `PointRadius` | 折线图和面积图标记的半径（4）、雷达顶点标记的半径（3），以及没有气泡大小时散点的半径，以像素计。 |
| `BubbleScale` | 散点图最大气泡的半径；其他的按它们的大小成比例确定大小（见下文）。 |
| `ShowTooltips` | 在运行中的窗体里，指针之下的柱、点或扇区——雷达图则是最近的顶点——显示 `label: value`。**默认勾选。** |
| `AnimateOnLoad` | 运行中的图表第一次有数据时，它的标记在 `AnimationDuration` 内生长到位——雷达图的多边形从中心展开。**默认勾选。** |
| `FillAlpha` | 面积图填充的不透明度，0–100 %。 |
| `AnimateValues` | 为**数据的变化**制作动画：图表从它正在显示的值行进到新的值，而不是直接切换。默认关闭。 |
| `AnimationDuration` | 那次移动——以及 `AnimateOnLoad` 的生长——所花的时间，以毫秒计。默认 2000；低于 250 的值会被提升到 250。 |

> **图表其余的文字跟随 `FontSize`。** 图例、坐标轴标题和数值标签都按图表自己的 `FontSize` 来确定大小，
> 与任何其他控件的文字一样——所以一个属性就能放大整张图表的文字，而 `TitleFontSize` 则用于只有标题应当
> 不同的时候。每个预留的带都随字号增长，所以更大的文字会占用空间，而不是压在绘图上。

**为数据的变化制作动画。** 勾选 `AnimateValues`，之后的每一次推送——`AddPoint`、`Clear`、`DataSource`
刷新——都是被*行进到*，而不是被跳到。**整个系列一起移动**，历时 `AnimationDuration`，所以图表无论有四个
点还是四十个点，都在同样的时间内稳定下来；新集合所添加的点在其他点移动时从零升起，而它所丢掉的点则
不再被绘制。标签从第一帧起就是新集合的，所以进行到一半的移动绝不会在某个点旧名称之下显示它。在移动
中途再次更改数据，它会**从屏幕上的那一帧**重新瞄准，而不是从它原本要去的那个集合，所以图表绝不会倒跳
回去再重新出发。

> **第一次填充属于 `AnimateOnLoad`，而不属于 `AnimateValues`。** 绘图会自动缩放到它自己的最大值，所以
> *数值*从零均匀上升，会在整个过程中画出同样的柱子。`AnimateOnLoad` 改为让**标记**生长——在应用了缩放
> 之后——所以柱子、线条和饼图的扫掠，会在图表第一次有数据时明显地升到位。取消勾选它，则让图表直接
> 出现。

> ⚠️ `ShowLegend` 和 `ShowLabels` 自图表存在以来就是勾选的，直到 1.61.97 才起作用，所以你在那之前构建
> 的图表会获得图例和扇区标签。取消勾选可得到旧的外观。

**按字段名绘制表。** 设置 `DataSource` 和 `DataCount`，生成的 `<id>-SET-TABLE` 段落就会绘制你的表。
默认情况下，每个出现项必须是一个 `PIC X(64)` 的标签，后面跟一个 `PIC 9(18)V9(6)` 的值。改为指明子
字段，它就会读取你自己的布局：

```cobol
       01  WS-SALES.
           05  WS-SALE OCCURS 12 TIMES.
               10  SALES-MONTH    PIC X(3).
               10  SALES-AMOUNT   PIC 9(7)V99.
               10  SALES-VOLUME   PIC 9(5).
       01  WS-SALES-COUNT         PIC 99 VALUE 12.
```

其中 `LabelField` = `SALES-MONTH`，`ValueFields` = `SALES-AMOUNT`。

**多个系列。** 在 `ValueFields` 中写出更多的字段——`SALES-AMOUNT, SALES-VOLUME`——柱状图、折线图或
面积图就会为每个字段绘制一个系列，按 `SeriesColors` 着色，并由 `SeriesLabels` 在图例中命名。一次一个点
时，`AddPoint` 为每个系列接收一个值：

```cobol
           INVOKE CH-SALES 'AddPoint' USING "APR" 7 9 4
```

给出的值比其他点少的点，在它所省略的系列中为 0。系列的数量没有限制：超过最后一个 `SeriesColors` 条目
后，颜色会重新开始。

**把图表绑定到表**，把一个字段映射到类别，并把任意数量的数值字段映射到系列——Data Binding 对话框会
映射它找到的每个数值字段。每个被映射的字段是一个系列，除非 `SeriesLabels` 给它命名，否则以字段命名，
而图表的 `LabelField` 和 `ValueFields` 会被相应地填入，所以设计器的预览已经在这些名称之下显示了这么多
个系列。绑定的优先级高于 `ValueFields` 先前所说的。

**`Stacked`**（柱状图和面积图）把各系列堆叠起来，而不是并排站立：每个标签成为一根由彩色段组成的柱——
在面积图上则是每个系列一条带——所以它读起来就是该标签的合计，而绘图缩放到最大的合计。

**负值**绘制在 X 轴之下，X 轴位于**零**处：-5 的柱从轴线向下悬垂，长度是 10 的柱升起时的一半。当每个值
都是正数时，轴线是绘图的底部，与以前完全一样；当每个值都是负数时，它是顶部。在堆叠时，正值从零处向
**上**堆起，而负值从零处向**下**堆，所以既有收入又有成本的月份两者都会显示。折线、面积和散点使用同一条
轴线；饼图或环形图会省略负值。

在**散点图**上只有一个系列，而 `AddPoint` 的第三个参数则是气泡的大小；`BubbleField` = `SALES-VOLUME`
根据表来确定每个气泡的大小（最大的横跨 `BubbleScale` 个半径）。饼图或环形图绘制第一个系列。

**雷达图。** `RadarChart` 在同一组度量上比较几个事物——一位候选人在六项技能上的得分，三种产品在同样
五条标准上的评级。它有柱状图、折线图和面积图的数据模型，只是读法不同：每个**点是一条轴**——雷达的
一根辐条，由它的标签命名——而每个**系列是一个多边形**，横跨各辐条绘制，每根辐条上有一个值。第一条轴
笔直朝上，其余的顺时针跟随，顺序就是各点被添加的顺序。你所知道的关于向这些图表供数据的一切都可以沿用：
`AddPoint(label, v1, v2, …)` 给出该轴在系列 1、2、3 ……中的值；`Clear`、`Refresh` 和 `RefreshBinding` 的工作
方式与那里相同；而 `DataSource`、`DataCount`、`LabelField`、`ValueFields`、`SeriesLabels` 和
`SeriesColors` 以同样的方式绑定并命名各系列，对数量没有限制。

```cobol
           INVOKE Radar-1::Clear()
           INVOKE Radar-1::AddPoint("Speed",   80, 55, 35)
           INVOKE Radar-1::AddPoint("Power",   60, 85, 40)
           INVOKE Radar-1::AddPoint("Range",   70, 45, 90)
           INVOKE Radar-1::AddPoint("Comfort", 90, 60, 45)
           INVOKE Radar-1::AddPoint("Safety",  55, 80, 65)
           INVOKE Radar-1::AddPoint("Value",   75, 50, 85)
```

在 `SeriesLabels` = `Team A,Team B,Team C` 时，它画出六根辐条和三个多边形，并在图例中命名。在第一个点到来
之前，该控件显示一个六轴三系列的示例，使设计器画布永远不会是空的；`Clear` 会把它带回来。

**把雷达图读好。** 每条轴共享同一个刻度，所以请给它一个明显的刻度：设置 `MinValue` 和 `MaxValue`（当各系列
单位不同时，通用的 0 到 100 指数很管用——每个除以它自己的领先者），并勾选 `ShowAxisValues`，把各环的值
写在第一根辐条上。请保持在两到三个系列：更多就变得无法阅读，而每个系列一个小倍数图（small multiple）读起来
更好。轮廓是不透明的，而填充是半透明的（`FillOpacity`），所以重叠的系列保持可见。离中心太近而无法与它
区分的值不会得到标记，而介于两根靠近中心的辐条之间、远远伸出的辐条会被绘制成普通的尖角——它不再从
图表中伸出一根发丝般的细线（在 1.90.9 之前，轮廓是一笔封闭的描边，而 egui 把这种拐角的羽化抛到了标题
之上）。

> 📷 需要截图 — `radar-chart-three-series.png`。把一个 RadarChart 放到深色背景的窗体上，把 **Transparency**
> 设为约 40，并在 `onLoad` 中带着上面的六行 `AddPoint` 和 `SeriesLabels` = `Team A,Team B,Team C` 运行
> 该窗体。截取显示着三个重叠多边形、边缘一圈轴名称以及下方图例的图表。

雷达图自己的属性，除了它与每张图表共享的标题、图例、颜色、边框和 `Monochrome` 之外：

| 属性 | 它做什么 |
| --- | --- |
| `GridLevels` | 网格画多少个同心环，1 到 10。默认 5。`ShowGridLines` 会把环和辐条一起隐藏。 |
| `FillOpacity` | 每个多边形的填充有多实，0–100 %。默认 35，所以相互叠放的多边形都可以读出来；0 则只留轮廓。它就是雷达图的 `FillAlpha`。 |
| `ShowPoints` | 在值高于 `MinValue` 的每个顶点上画一个标记，带有淡淡的光晕，使它在任何填充上都能读出来。**默认勾选。** `PointRadius`（默认 3）决定它的大小。 |
| `MinValue` | 中心处的值，由每条轴共享。默认 0。小数没问题：`-2.5`。 |
| `MaxValue` | 边缘处的值。**0——默认值——表示自动**：任何系列中的最大值，向上取整，使各环落在 10 的某次幂的 1、2、2.5、5 或 10 倍上（环在 20、40、60 ……而不是 17.4、34.8 ……）。把它设得高于 `MinValue`，可让刻度保持不动。 |
| `ShowAxisValues` | 在第一条轴旁边写出每个环所代表的值。默认关闭。 |

一个刻度服务于每条轴，这正是让多边形可以比较的原因：要比较两张雷达图，请给两者相同的 `MinValue` 和
`MaxValue`。超过顶部的值被画在边缘上。

雷达图也遵从共享的图表行为。`AnimateOnLoad` 让多边形从中心展开，而 `AnimateValues` 让它们行进到一组新的
值；`ShowTooltips` 显示最近顶点的轴、系列和值——`Power · Team B: 85`；`HideBackground`、`Transparency`、
边框属性和 `Monochrome`（连同它的渐变，较浅的中心淡向较深的边缘）与在任何图表上一样工作。不论
**Transparency** 是多少，它的文字和线条都绘制为实心，所以在深色窗体上设为 40 的雷达图仍然可读。

> ⚠️ **注意事项。**
> - **少于三条轴**无法围出一块面积：两条轴把每个系列画成一条线，一条轴则画成单个点。
> - **雷达图没有负的一侧。** 低于 `MinValue` 的值——在默认刻度上是负值——被画在中心，不是数字的值也是
>   如此。把 `MinValue` 设到零以下，才能绘制带符号的值。
> - **`MaxValue` 为 0 表示自动**，即使 `MinValue` 是负数；要把顶部固定在零，请使用刚好高于零的一个数。
> - 当轴名称会挤满边缘时，**它们会被 `…` 缩短**，而在小控件上圆会收缩，以便让每个名称都保持在它之内。
>   更大的控件、更短的标签或更小的 `FontSize`，会给圆更多空间。
> - **没有 X/Y 轴、坐标轴标题、`Horizontal` 或 `Stacked`。** 雷达图没有 `ShowXAxis`、`ShowYAxis`、
>   `XAxisLabel`、`YAxisLabel`、`Horizontal` 或 `Stacked` 中的任何一个；它不带有的属性从不被读取。
> - 生成的 `<id>-ADD-POINT` 段落带有一个标签和一个值（第一个系列），与每张图表上一样。要更多系列，请用带
>   多个值的 `AddPoint`，或者配合 `SET-TABLE` 使用 `ValueFields`。

> **注意。** 工具提示（`ShowTooltips`）报告指针之下的标记——柱或段，或者任何折线最近的点——并在图表
> 有多个系列时带上它的系列名称：`Q1 · Sales: 120`。
>
> 饼图或环形图不再带有 `ShowXAxis` / `ShowYAxis`——它没有坐标轴。`BarCornerRadius` 把柱的**每一个**
> 角都变圆——在堆叠柱上，只有顶部那一段的，所以接缝保持平整。

**Non-visual services**
: Timer、AgentObject（AI 智能体）、RestClient、SqlDatabase、**IndexedFile**、
**WebSearch**（Google、Brave、Serper、Tavily，或你自己托管的 SearXNG 实例）、
**Snackbar**（瞬时通知）。
**IndexedFile** 控件是索引文件在设计器一侧的面孔。记录及其键在项目的索引文件定义（`.cidx`）中只描述
一次，`SELECT` 和 `FD` 就是据此生成的；该控件随后指向那份定义，并给窗体提供驱动它所需的管道——
`OpenMode`、`AutoOpen` 和一个状态数据项——见
[索引文件](#14-索引文件一等公民级的资源)。

> **注意。** 存在一种 `Custom` 控件类型，作为定制/厂商控件的扩展点；请将它视为高级功能。

### 容器与嵌套

**GroupBox**、**Panel** 和 **TabControl** 是真正的**容器**：放在其中的控件成为它的**子项**，并随它一起
移动、裁剪和隐藏。容器可以任意组合地自由嵌套（TabControl 页面中的 GroupBox 里的 Panel，等等）。

- **把控件放进容器**——拖动它（从工具箱或某个已有位置），使它落在容器的**内容区域**之上；它就成为该容器
  的子项。之后移动容器，就会移动它的全部内容。
- **把控件拿出来**——把它拖到空白的窗体上，使它重新归属于窗体；把它拖到另一个容器上，就把它移到那里。把
  控件放到**非容器**控件之上，会让它成为那个控件的兄弟（同一个父级）。
- **裁剪与圆角**——子项被裁剪到容器的内容区域，而位于圆角处的子项会被保持在圆弧之内。每个控件都有一个
  **Corner radius** 属性（见下文*圆角半径*），它会把容器的框架变圆。
- **不透明度**——容器的 **Opacity**（0–100）会把容器*连同它的子项一起*淡化，所以你可以一次让整组变暗。
- **Enabled**——禁用容器会禁用其中的一切，所以 `SET MY-GROUP::Enabled TO 0` 一次就关掉整页的字段，而
  `SET MY-GROUP::Enabled TO 1` 再把它们打开。与可见性一样，子项自己的 `Enabled` 从不被写入：你单独禁用的
  控件——比如一个在窗体通过验证之前一直被关着的 Save 按钮——在这一组恢复时仍然保持禁用。
- **可见性**——隐藏容器会隐藏其中的一切。不被绘制的容器没有可供绘制的内部，所以
  `SET MY-GROUP::Visible TO 0` 会带走它的子项，而 `SET MY-GROUP::Visible TO 1` 把它们带回来。子项自己的
  `Visible` 从不被触碰，所以你单独隐藏的控件在这一组恢复时仍然保持隐藏——显示一个组，恢复的恰好是先前
  显示着的东西，而不是其中的一切。Form Designer 是例外：它会绘制 `Visible` 关闭的容器，**以及它里面的
  东西**，所以你仍然可以选中、移动和编辑你的程序在窗体打开片刻之后才显示的卡片的每个控件。（在 1.90.13
  之前，设计器只绘制卡片的表面，而把它的内容省略了。）
- **滚动**——对于其子项可能溢出其边界的容器，打开 **H-Scroll** 和 **V-Scroll**（`HScroll`、`VScroll`）。
  （关闭时，溢出的内容只是被裁剪。）
- **TabControl 页面**——每个标签页拥有它自己的一组子项。在设计器中点击一个标签页来编辑该页面；无论在设计时
  还是运行时，只有被选中的标签页的控件才会显示并可交互。

  **标签页看起来如何。** 标签页边靠边地位于 `TabPosition` 所指的那一侧的一条带中，而这条带与页面无缝相接：
  标签的外侧圆角是圆的，与页面共用的那一侧是直的，而被选中的标签与页面相融，二者之间没有线条。第一个
  标签与页面相接处的页面角是方的，所以标签和页面读起来是一个轮廓；它的另外三个角跟随 `CornerRadius`。
  在左侧或右侧时，标签是堆叠的，全都与最宽的一样宽，而它们的标题保持水平。

  在设计器中更改 `TabPosition` 会把页面的内容一并带走：控件保持它们相对于页面角落的位置，所以把标签条转到
  左边，会把它们向右移，移出标签之下。任何仍会位于标签条之下的东西都会被带进来，而如果内容随后越过了远侧，
  TabControl 就恰好增长到足以容纳它们，两侧的边距相同。一次 **Undo** 就能把标签条、控件和大小都放回去。

  | 属性 | 它设置什么 |
  |---|---|
  | `ActiveTabColor` | 被选中标签的填充（起初是蓝色） |
  | `ActiveTabForegroundColor` | 它的标题的颜色 |
  | `InactiveTabColor` | 其他标签的填充 |
  | `InactiveTabForegroundColor` | 它们的标题的颜色 |
  | `TabPadding` | 标题与它的标签的左右边缘之间的空间；每个标签的宽度是它的标题加上这个值 |

  把某个颜色留空，它就会替你选好：其他标签取一个与页面差一个层次的色调，而每个标题取在它的标签上能读出来
  的任何颜色（在默认的蓝色上是白色）。每个颜色都可以从 COBOL 更改，例如
  `MOVE "#FFFFFFFF" TO TAB-1::ActiveTabForegroundColor`。

  > ⚠️ **注意事项——`TabPadding` 在 1.70.272 改变了含义。** 它过去是标签之间的间隙；现在是每个标签内部的
  > 空间。把它设为 `0` 来合拢间隙的窗体，现在得到的是标题周围没有空间的标签——请把它设回大约 16。

删除容器会删除其中的控件。控件无论在哪里，都保持它唯一的 id，所以 `control::property` 访问和事件绑定不受
嵌套的影响。

#### 剪贴板

Form Designer 有一个用于快速布局工作的控件剪贴板：

- **复制**——选中一个或多个控件，按 `Cmd/Ctrl+C`。
- **剪切**——按 `Cmd/Ctrl+X`；控件及其子项会从画布上移除，并放到剪贴板上。
- **粘贴**——按 `Cmd/Ctrl+V`；粘贴的控件得到全新的 ID，保持它们的相对布局，并放在当前指针/画布焦点附近。
- **复制一份（Duplicate）**——按 `Cmd/Ctrl+D`；这是一步完成的复制 + 粘贴。

同样的操作也可以从 RAD 工具栏和画布的右键菜单中使用，所以用鼠标做布局工作不需要键盘快捷键。

在带有图层的窗体上，**粘贴总是落在活动的标签页中**——不论这些控件是从哪个标签页（或窗体）复制的。在
`Layer-1` 上复制一组控件，选中 `Layer-2` 并按 `Cmd/Ctrl+V`，它们就出现在 `Layer-2` 上；在某个图层上复制它们、
再在 `Form` 上粘贴，就能把它们带下来放到基础层。复制一份的控件留在它被创建所在的标签页中。粘贴的控件取它
所落入的标签页的下一个 Tab 顺序编号，而粘贴的 Panel 会把它的子项带到同一个标签页。在 `Non-Visuals` 标签页上，
只能粘贴非可视控件（见 §7）。

容器成员关系在被复制的选择之内得到保留。如果你复制带有子控件的 GroupBox，粘贴的副本有一个新的 GroupBox
ID，而子项被重新归属于那个新容器。事件处理程序代码在被复制的控件上得到保留，但粘贴的控件会收到根据它们的
新 ID 重新生成的处理程序名称。

#### 用方向键微调位置

选中的控件不必被拖动。**方向键**会移动选中的一切，每按一次一步：

- **Snap to grid** 开启时，一步就是一个网格单元——与拖动会把控件放置的位置相同，所以键盘和鼠标彼此一致。
- 吸附关闭时，一步就是一个像素。
- 按住 **Shift**，不论网格怎么说，都是单个像素，用于你想让控件有意偏离网格的时候。

选中的容器会带着它的子项，而每次按键都是一次撤销（`Cmd/Ctrl+Z`）。被微调到它的主体位于某个容器之上的控件，
会成为那个容器的子项，就像你把它拖到那里一样。

设置了 **Anchor** 的控件留在原处：锚定会锁住控件，使它不能被手工移动，而方向键就是在手工移动它。在属性窗格的
**X** 和 **Y** 中输入，来重新定位被锚定的控件。

> **注意**——方向键属于你正在输入的任何东西。当插入符位于属性字段中时，方向键移动的是插入符，画布上没有
> 任何东西移动。

#### 圆角半径（所有带边框的控件）

每个绘制边框的控件——按钮、文本框、组合框/列表框、图片框、数据网格、数值/日期选择器、进度条、滑块、形状、
图表以及各个容器——都有一个 **Corner radius** 属性：

- 控件的**背景和边框被圆化**到该半径。
- **内容被裁剪到圆角形状。** **PictureBox** 的图像会被修剪到圆角（在任何背景之上，包括窗体背景图像），图表
  的框架也会变圆。
- **Corner radius = 0** 表示方角且**不裁剪**——这是默认值，所以已有的窗体看起来与以前完全一样。该值会被
  限制，使它永远不超过控件较短一边的一半（完全圆化的“药丸”/圆）。
- **控件自己的投影会透过圆角显示出来。** 半径所削去的区域不再属于控件，所以那里它后面的东西——窗体的
  表面*以及*控件投在它上面的阴影——才是你看到的。正是这种连续性让圆角控件看起来像是坐在窗体上，而不是从
  它上面被剪下来，并且在 **Shadow distance** 和 **Shadow blur** 较大时最为明显。

同样的半径和裁剪，在设计画布、实时预览和运行中的窗体上完全一致地适用。*局限：*运行时输入的可编辑文本/滚动层
（例如输入时的 TextBox）在它的圆角框架里保持方形。容器的**子项**不会被裁成方的：到达其容器圆角的子项，会
被保持在圆弧之内。

**每一种边框样式都遵循那个半径**，在每个拥有它的控件上。`BorderStyle` 在属性窗格中取六个值（`Fixed3D` 和
`Raised` 画法相同，所以它们共用一行）：

| 样式 | 它绘制什么 |
| --- | --- |
| `None` | 完全没有边框。 |
| `Single` | 一条宽度为 `BorderWidth`、颜色为 `BorderColor` 的线，遵循圆角半径。 |
| `Fixed3D`, `Raised` | 从左上方打光的浮雕：上边和左边用 `BorderColor` 的较浅色调，下边和右边用较深的色调，在圆角弧线的半途相接。 |
| `Sunken` | 同样的浮雕反转过来，使控件读起来像被按进了窗体。 |
| `Glow` | 一条用 `BorderColor` 的含蓄的边，它的四个角发光，每个角有它自己的颜色，沿着两条边淡出，就像光线照在玻璃面板的角上。 |

**`Glow` 与它的四种颜色。** 选择 `Glow`，*BorderStyle* 之下会出现四行颜色：`BorderGlowTopLeft`、
`BorderGlowTopRight`、`BorderGlowBottomRight` 和 `BorderGlowBottomLeft`。把它们留空，得到的是白色的高光，在左上角
最亮；或者给每个角它自己的颜色。它们是普通的颜色属性，所以程序可以在运行时更改它们，例如在卡片被选中时把它
点亮：

```cobol
           IF WS-SELECTED = "Y"
               SET CARD-1::BorderGlowTopLeft     TO "#60BEFF"
               SET CARD-1::BorderGlowBottomRight TO "#A896FF"
           END-IF
```

发光与其他每种样式一样跟随 `CornerRadius` 和 `BorderWidth`；`BorderWidth` 为 2 或 3 时效果最佳。各角之间的边是你的
`BorderColor` 的 40 %，所以更深的 `BorderColor` 会让发光更突出。

> 📷 需要截图 — `border-glow.png`：一个 Spatial 窗体，其中三个面板使用 `BorderStyle` `Glow`：一个带默认的白色发光，
> 一个带天蓝/薄荷/薰衣草/桃色的角，一个只在左上角有一个明亮的角。

浮雕与 `Single` 一样，精确地跟随圆角半径——在 1.61.170 之前，它在外接框上画四条直线，并在每个角上伸出圆弧之外。
不论是什么在绘制控件的表面——玻璃样式、背景渐变、窗体主题或资源包——它的绘制也完全一致。

> **注意——Neumorphic。** 该样式从给整个窗体以其外观的同一个阴影栈中，绘制它自己的、从左上方打光的柔和浮雕，所以
> `Fixed3D` 和 `Raised` 在那里读起来是凸起的。**`Sunken` 把那个浮雕翻过来**——上边和左边是阴影，下边和右边是
> 高光——所以设为它的控件读起来像是被按*进*了窗体。**`Single` 保持为你自己的线条**，使用 `BorderColor` 和
> `BorderWidth`，与在其他每种样式上一样。
>
> ⚠️ 在 1.70.263 之前，`Single` 在 Neumorphic 之下也是浮雕，所以 `BorderColor` 和 `BorderWidth` 在 Neumorphic 窗体
> 上似乎不起作用。

> 使用容器 **Border radius** 的较旧窗体仍然可以加载并正确地圆化——它被读作 **Corner radius** 的别名。

#### 可展开卡片

仪表盘是一个卡片的网格，而偶尔会有一张值得占满整个屏幕：一张你想仔细阅读的图表，一个你想滚动浏览的网格。任何
**Panel** 或 **GroupBox** 都可以被**展开**——它取走它所有兄弟卡片之间所占有的空间——并再次**折叠**，把那些空间
还回去。

| 属性 | 它做什么 |
| --- | --- |
| `Expandable` | 打开它，卡片就在它的右上角显示标准的展开图标（两个朝外分开的箭头）。点击它会展开卡片；此时图标显示两个相向的箭头，点击它则折叠。默认关闭。 |
| `Expanded` | 状态：卡片展开时为开。与窗体一起保存，所以卡片可以以展开状态打开。 |

你的程序也可以驱动它，不论有没有图标：

```cobol
       INVOKE CARD-SALES::Expand()
       INVOKE CARD-SALES::Collapse()

       IF CARD-SALES::Expanded IS TRUE
           MOVE "Back to the dashboard" TO LBL-HINT::Caption
       END-IF
```

`Expand()` 和 `Collapse()` 是写入 `Expanded` 1 和 0 的简写。卡片展开期间，**其中的一切都随它一起增长**，在每一个
深度，**包括文字**，并且在两个方向上以**同一个系数**，所以什么都不会被拉得变形：该系数是卡片变宽了多少与变高了
多少之中较小的那个——也就是整个内容仍然放得下的最大系数。每个控件以那个系数保持它在卡片中的位置——位置、大小
和字号——不论它的 `Anchor` 怎么说，所以标签、柱、图表和带有它自己控件的面板都一起增长，就像一幅图片被缩放。
当卡片的形状与内容被设计时所针对的不同，增长后的块会被**居中**：剩下的空间平均分给两侧（或上下）。卡片内部的
容器的 `Dock` 和 `LayoutMode` 仍然进行它们自己的布局，而 `ScaleFont` 关闭的控件保持它的字号。折叠卡片，它的
内容就回到它们的锚定。

> **注意。** “它的兄弟们的空间”是卡片所在容器的整个客户区：对于 Grid 中的卡片，是网格的各轨道和间隙加在一起；
> 对于直接在窗体上的卡片，则是窗体。卡片展开期间，它的兄弟们以及它们里面的一切都不被绘制，也不响应鼠标。如果
> 两个兄弟同时展开，堆叠顺序中最高的那个胜出。点击图标不是对卡片的点击，所以卡片的 `onClick` 保持安静；这个
> 变化没有事件——需要时请读取 `Expanded`。

> 图标属于卡片自己的图层：绘制在卡片**之上**的控件——下拉列表的叠加卡片、铺在上面的面板——在覆盖它时会隐藏
> 卡片的图标，而在那里的点击是该控件的，而不是卡片的。（在 1.90.24 之前，下拉列表后面那些卡片的图标会显示在
> 它的上面。）

> ⚠️ **注意事项。** 摆放卡片的是布局，所以这在**响应式**窗体上才有效（`Responsive` 开启）——每个应用程序都应当如此
> ——而在不是响应式的窗体上不会绘制图标。在 Form Designer 中，在属性窗格里勾选 `Expanded` 会显示展开的卡片，并把
> 它的兄弟们从画布上拿掉，直到你取消勾选。

> 📷 需要截图 — `expandable-card.png`：一个运行中的仪表盘网格，其中一张卡片在右上角显示着展开图标；以及同一个
> 仪表盘，那张卡片已展开，显示着折叠图标。

#### GroupBox 外观

除了共享的容器属性之外，**GroupBox** 还在属性窗格的 **Appearance** 部分增加了视觉选项：

- **Hide caption**——保留该框作为容器，但不绘制标题文字。
- **Hide background**——让该框变透明（没有填充或边框），而它的子项保持可见。
- **Background color**——实心的填充颜色。
- **Background gradient**——打开双色渐变填充，带有**起始**和**结束**颜色以及一个**方向**。方向以罗盘方位给出——
  *North*、*NorthEast*、*East*、*SouthEast*、*South*、*SouthWest*、*West* 或 *NorthWest*——而全新的渐变从 *South*
  开始，自上而下运行。（渲染器还理解 *Radial*，以及线性别名 *Vertical*、*Horizontal*、*DiagonalUp* 和
  *DiagonalDown*，用于从 COBOL 设置的值或由主题提供的值；选择器本身列出的是八个罗盘方位。）

#### GroupBox 标题样式

默认情况下，GroupBox 的标题是 PowerCOBOL 或 isCOBOL 框架的经典图例：位于上边框之上、刚过圆角的朴素文字。给它一个
**Caption background**，它就变成带样式的标题框：

| 属性 | 取值 | 它做什么 |
|---|---|---|
| `CaptionBackgroundStyle` | `None`（默认）、`Flat`、`Gradient` | `None` 保持经典的图例。`Flat` 用 `CaptionBackColor` 填充该框；`Gradient` 沿 `CaptionGradientDirection` 从 `CaptionGradientStart` 到 `CaptionGradientEnd` 填充它。 |
| `CaptionShape` | `Rectangle`（默认）、`Pill`、`AngledLeft`、`AngledRight` | 该框的轮廓——见下面的草图。 |
| `CaptionSize` | `Text`（默认）、`Full`、`Inner` | `Text` 紧贴文字；`Full` 横跨圆角之间整条笔直的上边；`Inner` 做同样的事，但在每个角之前 10 px 处停住，留下一小段边框作为短桩。 |
| `CaptionPadding` | 0–64 像素（默认 4） | 文字在它的框内四周空间的简写：左右各这么多，上下各一半。 |
| `CaptionPaddingHorizontal` | 0–64 像素，或为空（默认） | 文字左右的空间；为空则跟随 `CaptionPadding`。 |
| `CaptionPaddingVertical` | 0–64 像素，或为空（默认） | 文字上下的空间；为空则跟随 `CaptionPadding` 的一半。 |
| `CaptionAlignment` | `Auto`（默认）、`Left`、`Center`、`Right` | 文字所在的位置。`Auto` 对 `Text` 标题是靠左，对 `Full` 和 `Inner` 是居中。 |

```text
Rectangle      [ Customer ]
Pill           ( Customer )
AngledLeft     \ Customer \
AngledRight    / Customer /

Full           [               Customer               ]
               |                                      |
Inner        +--[             Customer             ]--+
             |                                        |
```

该框以 GroupBox 自己的 **Border color**、按它的 **Border width** 勾勒轮廓，而标题文字会挑选一种在填充上能读出来
的墨色——所以深色的渐变会得到浅色的文字，而不需要你来选择。每个属性都可以像其他任何属性一样从 COBOL 设置：

```cobol
           MOVE "Gradient" TO GRP-CUSTOMER::CaptionBackgroundStyle
           MOVE "Pill"     TO GRP-CUSTOMER::CaptionShape
           MOVE "Inner"    TO GRP-CUSTOMER::CaptionSize
```

GroupBox 的上边框在标题**后面是开着的**——横跨整个标题区域，包括内边距，而不仅仅是字母后面——所以边框永远不会
穿过标题。标题从不伸进圆角：不论它的内边距多大，它都在左上角的半径之后开始，并在右上角的半径之前结束，所以
圆角保持它们的形状。`Full` 标题会把上边整条笔直的部分打开。

> **注意。** `CaptionBackgroundStyle` 保持为 `None` 时，`CaptionShape` 什么也不画——只有 `CaptionSize`、
> `CaptionAlignment` 和内边距仍然决定文字所在的位置，以及边框上的开口有多宽。

> 📷 **需要截图 — `groupbox-caption-styles.png`。** 一个窗体上的四个 GroupBox，分别显示：Flat 的 Rectangle
> `Text` 标题、Gradient 的 Pill `Inner` 标题、AngledLeft 的 `Full` 标题，以及带有 `Right` 对齐的 AngledRight 标题。

#### 重复组（GroupBox 数组）

**GroupBox** 可以被变成**重复组**——一个在运行时被重复的视觉模板，每个数组元素一个实例。把这个组设计一次
（它的子控件就是模板），并右键点击它 ▸ **Set as Repeating Group**（再右键点击则是 **Unset Repeating Group**）。
在设计器中，一个小小的 **▦ ARRAY** 徽章标记着重复组。

之后属性窗格中会出现一个 **Repeating Group** 部分：

- **Array name**——数组的逻辑名称（默认是 GroupBox 的 id）。
- **Item count**——运行时的实例数量。
- **Data source**——用来填充实例的可选数据源。
- **Layout direction**——*Vertical*、*Horizontal* 或 *Grid*。
- **Item spacing**——实例之间的间隙。
- **Items per row**——布局为 *Grid* 时的列数。
- **Placement effect**——可选的卡片摆放动画：*None*、*Deal*、*FadeIn*、*ZoomIn* 或 *ZoomOut*。缩放效果让每张卡片锚定在
  它最终的布局位置，并以弹性缓动缩放整个卡片组。
- **Scrolling**——该组没有它自己的滚动设置：要让实例溢出并滚动，请把该组放在打开了 **H-Scroll**/**V-Scroll** 的
  **Panel**、**GroupBox** 或 **TabControl** 之内——这三者都会滚动它们的子项。
- **Clone events**——开启（默认）时，子控件的所有实例共享同一个事件处理程序，由 `CONTROL-ARRAY-INDEX` 告知是哪张
  卡片触发的。关闭时，只有所设计的那张卡片（1）运行它的处理程序；克隆体只用于显示。
- **Preview items**——**设计器**预览多少个实例（它们只是渲染用的幽灵；它们*不会*被加入你的窗体，所以选择和撤销
  不受影响）。运行时，未绑定的组只有在它的 `ItemCount` 为 0 时才显示这么多个。

运行时，每个实例及其子项通过成员访问语法按索引寻址，例如 `CustomerCard(3)::CustomerName::Caption`——索引是
**从 1 开始**的。子项的事件处理程序被每个实例共享，并通过设计器为它预置的 `CONTROL-ARRAY-INDEX` 链接项得知
是哪张卡片触发的（§10）：

```cobol
       LINKAGE SECTION.
       01 CONTROL-ARRAY-INDEX     PIC S9(4) COMP-5.

       PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.
           DISPLAY "card " CONTROL-ARRAY-INDEX " was clicked".
```

为固定数量的卡片设置 `ItemCount`（在设计器中，或从 COBOL 用 `SET grp::ItemCount`），或者绑定 `DataSource` 并让数据来
决定；在你更改了 working-storage 之后，对该组调用 `RefreshBinding()` 会根据它重新填充各卡片。

#### 把重复组绑定到 COBOL 表

重复组可以用你的数据来填充，也可以把操作员所输入的内容写回去。DataGrid 把表显示为单元格的行，而绑定的重复组把它显示为
**卡片**：每行一个该组的实例，而每个源字段进入卡片的一个控件。

1. 选中重复组，并在属性窗格中打开 **Data Binding**。该组作为 **Control Array** 目标被提供；普通的 GroupBox 或任何其他
   控件则不会。
2. 选择一个 **COBOL table** 作为数据源（见下一节了解什么是表），并把每个字段映射到**一个成员控件及其一个属性**。适合该
   控件的属性会被预先选好：Label 或 Button 是 `Caption`，TextBox 是 `Text`，CheckBox 是 `Checked`，RadioButton 是
   `Selected`，PictureBox 是 `ImagePath`，ComboBox、ListBox、NumericUpDown、Slider、ProgressBar 或 DateTimePicker 是
   `Value`。如果你想要别的，可以挑选该控件的另一个属性。你没有映射的成员保持你所设计的值，而两个字段不能取同一个成员的
   同一个属性。
3. 运行窗体。卡片出现，每行一张，由表填充。在你自己的代码更改了表之后，对该组调用 `RefreshBinding()`。

在设计时，卡片显示来自字段定义的示例值，所以你可以在有任何数据之前就把卡片布置出来。在 **Preview** 中，你可以往卡片的
成员中输入来试用这张卡片：你输入的内容保留在那张卡片上，直到 Preview 关闭，而且不会写入任何地方——Preview 不运行你的
任何 COBOL，所以下面的写回只在窗体运行时才发生。

```cobol
      *> The group CUSTOMER-CARDS is bound to the table CUSTOMER-ROW; the table has changed.
           MOVE "Ana Lima" TO CUSTOMER-NAME(2)
           CUSTOMER-CARDS::RefreshBinding()
```

**写回。** 让绑定成为**可写的**（以及它的表是可写的），操作员输入到卡片 *n* 的某个成员中的内容，就会写入行 *n*，写入该成员所
映射到的字段，而不写入其他任何字段：

| 发生了什么 | `CommitStatus` | `LastError` | `PendingEdits` |
|---|---|---|---|
| 值已被写入 | `00` | 空 | 如果该编辑正在等待，它会被移除 |
| 绑定是只读的：什么都没有写入 | `91` | 如此说明 | 不变 |
| 字段无法容纳该值（把文本输入到数字中，卡片超过了最后一行） | `92` | 哪个字段以及为什么 | 该编辑被加入 |

被拒绝的编辑不会丢失：它留在 `PendingEdits` 中，每行一个 `card`、`member`、`property` 和 `value`，以制表符分隔，而该成员仍然
显示所输入的内容，使操作员可以更正。把该成员编辑成表能接受的内容，就会移除那一行；`RefreshBinding()` 会重新读取表并
清空列表。这三个是该组的只读属性。

```mermaid
flowchart LR
    E["操作员编辑卡片 n<br/>的某个成员"] --> W{"可写的绑定<br/>且表可写？"}
    W -->|否| R["CommitStatus 91<br/>什么都没写入"]
    W -->|是| F{"该字段<br/>接受这个值吗？"}
    F -->|是| T["行 n，仅那个字段<br/>CommitStatus 00"]
    F -->|否| P["CommitStatus 92<br/>保留在 PendingEdits 中"]
```

**它与 DataGrid 有何不同。**

| | DataGrid | 重复组 |
|---|---|---|
| 一个单元是 | 一列 | 一个成员控件及其一个属性 |
| 行的模板 | 一行单元格 | 你在该组中设计的控件 |
| 运行时它可以来自哪里 | 索引文件、COBOL 表或 SQL 表 | COBOL 表 |
| 编辑 | 单元格内编辑器，用 `Save()` 或 `AutoCommit` 保存 | 成员控件本身，在被编辑时写回 |
| 还有 | 表头、排序、过滤器、冻结窗格、CSV 导出 | 该组自己的布局（垂直、水平、网格）和卡片效果 |

**Data Binding Guardian** 会在保存、运行、构建或检查之前检查组的绑定，并且在某个被映射的控件不在该组之内、或两个字段取同一个
成员属性时，拒绝继续。在可写的绑定中，对于只显示其值的成员（Label、图片、条）它会**警告**但不拒绝：不会从它写回任何东西。

> ⚠️ **注意事项。**
> - 只有 COBOL 表才会在运行时填充各卡片。绑定编辑器允许你选择另一种数据源，这对布置卡片有用，但没有任何东西会从索引文件、
>   SQL 表、REST 调用或智能体来填充该组。
> - 只有当组的成员被某个映射绑定时，它才会写回；对未被映射的属性的编辑会被置之不理。
> - **嵌套**的重复组无法被绑定。

#### 数据绑定与 Guardian

数据绑定被配置为**窗体级绑定**，而不是每个标量控件上的独立属性。在 Form Designer 中选中一个被批准的目标，
然后使用属性窗格中的 **Data Binding** 部分，从下列数据源家族之一创建绑定：

- **Indexed**——项目的 `.cidx` 定义及其记录字段。
- **SQL**——一个 `SqlDatabase` 控件、查询和结果集。
- **COBOL table**——内存中的 COBOL 表：`GLOBAL` 01 之内**任意级别（01 到 49）**的带 `OCCURS` 的数据项。每张表
  以它的 `OCCURS` 数据项的名字列出。例外是位于 01 本身或它的直接子项上的表，它沿用 01 的名字。嵌套在另一张表的
  某次出现之内的表是那张表的一部分，不是独立的表。基本 `OCCURS` 数据项（`05 UF PIC XX OCCURS 27 TIMES.`）是
  一张单列的表，它的列就是该数据项自身。这张表可以 `REDEFINES` 由 `VALUE` 子句填充的存储：

  ```cobol
         01 ITEMS GLOBAL.
            03 ITEM-1       PIC XX VALUE "01".
            03 ITEM-2       PIC XX VALUE "02".
            03 ITEM-3       PIC XX VALUE "03".
         01 ITEMS-TABLE REDEFINES ITEMS GLOBAL PIC XX OCCURS 3.
  ```
- **REST**——一个 `RestClient` 的响应数据项、已保存的模式，或示例载荷。
- **Agent AI**——一个结构化的 `AgentObject` 输出。

被批准的绑定目标刻意限定在能显示或编辑结构化行的控件上：

- **DataGrid**——把字段映射到稳定的网格列。
- **Charts**——把一个字段映射到类别，把一个或多个数值字段映射到数值系列。
- **ComboBox** 和 **ListBox**——映射显示文本和一个可选的选中值。
- **Knob**、**Gauge** 和 **Switch**——*标量*目标：一个源字段驱动 `Value`（Knob、Gauge）或 `Checked`（Switch），
  不需要重复组。它们是下面这条规则的例外。
- **Maps**——标记集合：每行成为一个标记。
- **显式控件数组**——把字段映射到重复 GroupBox 或等价的数组契约之内的子控件属性。

**移除绑定。** 打开控件的绑定编辑器，按 **Clear selection**，确认，然后按 **Apply**。没有选定数据源时，对一个带有已保存
绑定的控件执行 Apply 会移除该绑定，而 **Undo** 会把它带回来。在 1.70.227 之前，这个 Apply 会被拒绝，并提示
"A binding source must be selected."，所以已保存的绑定无法被移除。

**让绑定可写。** 新绑定是**只读**的：网格显示数据源，而编辑只留在网格中。对于索引文件、SQL 表或 COBOL 表，绑定编辑器在字段列表
之下有一个 **Writable** 开关；打开它，网格的 `Save()`（以及 `AutoCommit`）才能把操作员的编辑写入数据源——见
*高级 DataGrid* 之下的*把编辑保存回数据源*。绑定保持它被保存时的模式，而 Apply 不会改变它。开关打开期间，编辑器用
通俗的话列出**网格仍然需要什么**：

- 一个**键字段**——在标识行的字段上勾选 **Key**（索引文件和 SQL 表）；
- 编辑所更新的**表**（SQL 表）；
- 网格上的 **Allow cell editing**——没有它，操作员无法编辑单元格，所以什么都不会被写入。对于这一项，编辑器提供一个
  **Turn on cell editing** 按钮；它把网格的属性作为单独的一步来更改，所以 **Undo** 会再把它关掉。

COBOL 表不需要键：保存时它由网格重新生成。REST 或 Agent AI 绑定没有 Writable 开关。

除了上面三个标量目标之外，独立的标量控件（例如单个 TextBox 或 Label）**不会**暴露数据绑定信息。如果标量控件属于某个显式控件
数组，它只能显示由数组拥有的映射上下文；它不能自己选择数据源。这样可以避免某个字段悄悄地偏离行契约。

> **哪些组合在运行时真正会填充（1.70.224）。**
> 设计器允许你把任何数据源家族与任何被批准的目标配对。绑定编辑器验证的是映射，而不是该配对在窗体运行后是否真的起作用。
> 目前会填充的有：
>
> - **COBOL 表**填充每一个被批准的目标。**DataGrid** 每次出现得到一行，每个字段得到一列。**ComboBox** 或 **ListBox**
>   对映射到它的显示文本的字段，每次出现得到一个条目。**图表**每次出现得到一个点：类别字段是标签，而映射到数值系列的
>   第一个字段是值。**Knob / Gauge / Switch**、**Maps** 和**控件数组**也会填充。所有这些都在窗体打开时、`onLoad` 之后加载，
>   所以由 `VALUE` 子句或你的 `onLoad` 填充的表已经在屏幕上了。更改了表之后，请对任何绑定的控件调用 `RefreshBinding()`。
>   （直到 1.70.226，绑定到 COBOL 表的 DataGrid 即使在启动时也要等待这个调用，所以绑定到 `VALUE` 表的网格是空着打开的。）
> - **Indexed 数据源**填充 **DataGrid**，直接读取 `.cidx` 的文件，按主键顺序，你的程序里不需要 `SELECT`/FD。它在绑定加载的那一刻
>   就自己刷新，因为没有需要等待的填充步骤。
>
> 其他每一种配对都可以配置并经过验证，但目前还没有东西会填充它：任何目标上的 SQL、REST 和 Agent AI，以及图表、ComboBox、
> ListBox 或控件数组上的 Indexed。请以这里记载为可用的为准来构建，而不要以设计器只是让你配置的为准。

> **在 1.70.224 之前，**绑定到 ComboBox、ListBox 或图表的 COBOL 表会被保存和生成，但从未被加载，所以控件在运行时保持为空。
> 设计器也只提供位于 01 或其直接子项上的表。

**Indexed 绑定到哪里去找它的文件。** 涉及两条路径，两者都**相对于你的项目**存储：绑定中记录的 `.cidx`，以及该 `.cidx` 自己的
assign 路径中记录的数据文件。两者都针对**项目文件夹**解析——而不是针对程序被启动时所在的目录——所以同一个绑定在设计器的
Indexed File Browser、**Run Form** 之下以及构建出的应用程序中，读到的是同样的记录。把路径指到项目之外，它就以绝对路径存储，
这同样可以工作；相对路径则简单地跟着项目走，所以提交到仓库并克隆到另一台机器上的窗体仍然能找到它的数据。

> ⚠️ **构建出的应用程序有它自己关于“项目”的概念。** 它以存放 `assets/` 的文件夹为锚点——开发期间是项目内的 `bin/`，
> 而交付时是 `dist/` 中的移交文件夹。
>
> **构建会带上绑定所需的部分。** 你的 `assets/` 和 `data/` 文件夹会被复制进交付物，项目声明的每一个 `.cidx` 也是，
> 每个都保持你的窗体已经存储的相对路径——所以 `indexed/BurguerTime/menu.cidx` 恰好落在它的窗体寻找它的地方。你不需要
> 手工复制它们。
>
> 只有**已声明**的定义才会随行。放在 `indexed/` 中但项目没有列出的 `.cidx` 不是应用程序的一部分，也不会被交付。在运行时
> 构造索引路径、而不是绑定到已声明定义的程序，仍然需要你自己来交付。

每个绑定都把它的数据源描述符、目标描述符、有序的字段映射、只读/可写模式、已保存的数据源元数据以及验证快照存储在 `.cfrm` 文件中。
没有绑定元数据的现有窗体照常加载和保存；旧的标量 `DataItem`/`DataFormat` 值仍然可以往返，但新的绑定行为来自顶层的绑定列表。

**Data Binding Guardian** 在窗体被保存、窗体被运行、调试开始、Check 运行、Build 开始或创建软件包之前验证绑定。发现分为三种
严重程度：

- **Blocker**——该操作被停止。例如：已删除的目标控件、缺失的源字段、不受支持的目标、仅大小写不同而含糊的标识符、可写绑定
  缺少行标识，或不安全的 Agent AI 目标范围。
- **Warning**——该操作可以继续，但请检查映射。例如：可强制转换的类型转换、可空到必填的映射，或不完整的 REST/Agent 模式信息。
- **Info**——不影响该操作的提示性信息。

REST 和 Agent AI 的验证是本地的、离线的。Guardian 使用已保存的模式、已保存的示例、响应数据项名称和显式映射；它不需要实时的
网络调用。除非你提供显式的更新元数据——请求模式、键/行标识字段和被批准的目标列表，否则 REST 和 Agent AI 绑定是只读的。

可写的绑定必须保留数据源的标识。可写的 Indexed、SQL、REST 或 Agent 绑定需要一个键或行标识字段，使更新能够指向正确的记录。
可写的 **COBOL table** 不需要：它由网格整体重写。初始加载会填充目标而不把它标记为已修改（dirty）。用户的编辑作为待处理的
绑定状态保留，直到显式的更新辅助过程或你窗体自己的事件契约把它们提交。绑定的 DataGrid 用 `Save()` 写入它们（见 *高级 DataGrid* 之下的*把编辑保存回数据源*）——每次更改都在一个事务中——而如果保存失败，什么都不会被写入，网格会回到它最近一次
加载的内容。

修复操作只涉及元数据，并保留视觉布局和事件处理程序：

- 重新映射缺失的字段；
- 移除过期的映射；
- 把绑定标记为只读；
- 从已保存的模式或示例元数据刷新字段；
- 从可用的项目数据源刷新字段；
- 重新选择目标控件。

#### 高级 DataGrid

**DataGrid** 是面向行的表格数据绑定目标，也是设计器中密度最高的可视控件。它为了兼容保留了旧的 `Columns` 和 `Rows`
属性，而较新的布局和格式设置则作为高级元数据存储在网格上（包括每列的背景/前景）。

`Rows` 每行一条线，其单元格以 TAB 分隔。**空行也是一行**——在单列网格中，它是唯一的单元格为空的那一行——而且每个界面
对它的计数都一样：设计器和运行中的窗体都会绘制它，`ClickedRow` 和 `onRowButtonClick` 的 `ROW-NUMBER` 会计入它，
`GetRowCount`、`GetCellValue` 和 `DeleteRow` 也看得到它，所以点击所指的那一行，就是你的处理程序读取的那一行。（`Rows`
最末尾的换行符结束最后一行；它不会再增加一行。）

**外观与边框规则（所有界面统一）**

- 在外观中定义的背景，现在会正确地应用到**最后一个数据绑定列**以及它之后的所有**非数据绑定列**。
- **网格线背景**（分隔列/行的填充）遵循网格外观设置中所设的背景。
- **外边框**使用 DataGrid 设置中的 `GridLineStyle`（Solid/Dash/Dots/DashDot/None），并且在半径 > 0 时画成向内缩进的
  圆角描边——此时总是实线，因为虚线无法沿着圆角的弧线走。
- 所有外观、线条样式和边框行为在设计画布、Preview、Run Form 和编译出的二进制中都一致（统一渲染引擎）。

**排序、行号与选择**

- **AllowSorting**（默认开启）：点击列标题会按该列对行排序——先升序，再点击则降序——并用 ▲/▼ 标记该列。数值列（声明为
  `number`，或其每个值都是数字的列）按值排序，所以 `9` 排在 `100` 之前；其他列按文本排序，忽略大小写。只有**显示**顺序
  改变：`Rows` 保持它的顺序，而 `onCellClick` 仍然报告每一行自己的索引，所以你的 COBOL 表仍然对得上。无论如何
  `onColumnClick` 都会触发。（相反，`Sort` 方法会重新排列 `Rows` 本身。）
- **ShowRowNumbers**：列左边的一条边栏用表头的颜色，从 1 开始给所显示的行编号。它的宽度取自各列，并且宽到足以容纳网格
  *拥有的*行数——所以在列筛选中输入（那会显示更少的行）永远不会移动各列——网格横向滚动时它也保持不动。
- **SelectionMode**：点击高亮什么——整行（`Row`，默认）、单个单元格（`Cell`）或整列（`Column`）。Ctrl+C 复制被高亮的内容：
  单元格、行（单元格以 `CSVDelimiter` 连接），或所显示的行中该列的值，每行一个。
- **ExportCSV** 是内置 CSV 按钮的总开关：关闭时，即使 `ShowCSVExportButton` 开着，按钮也是隐藏的。`ExportCSV` 方法和生成的
  `<id>-EXPORT-CSV` 段落不论哪种情况都能用。
- `RowHeight` 是每一行的高度，14–120 点；拖动行的边缘会写入它。**一行可以有它自己的高度**——一个备注行，一个容纳两行文字的
  行——这通过 **`RowHeightOverrides`** 实现：`row=height` 对，行号从 1 开始，与 COBOL 其他各处一样（`1=40;8=64`，
  14–400 点），在网格的设置中设置，或用 `SetRowHeight(row, pixels)` 设置。高度属于数据行，所以网格被排序或筛选时它跟着它的行走，
  而 `Sort`、`DeleteRow` 和 `ClearRows` 会带着它；拖动这样一行的边缘只会调整那一行的大小。
- **就地编辑单元格——`AllowCellEditing`**（默认关闭）。操作员双击一个单元格，或在选中的单元格上按 F2，它就作为文本框打开；
  Enter 或点击别处保留更改，Escape 放弃。被保留的更改会写入 `Rows` 并引发 **`onCellEdited`**，它的处理程序读取
  `EditedRow` 和 `EditedColumn`（数据行和数据列，像 `GetCellValue` 一样从 1 开始编号）、`EditedValue` 和 `PreviousValue`。
  网格只是*显示*新的文本——存储它是你的处理程序的工作，拒绝它也是：用 `SetCellValue` 把 `PreviousValue` 放回去。（**绑定到
  可写数据源**的网格自己保存这次编辑——见下面的*把编辑保存回数据源*。）背后没有数据的列，或以图像显示其值的列，是不可编辑的。
  （旧的 `ReadOnly` 已作废，不控制这一点——在编辑存在之前，它在每个网格上都被关着。）

```cobol
      *> Freeze the key column and filter to one city, from COBOL.
           MOVE 1        TO DG-CUSTOMERS::FrozenColumns
           MOVE "City=Rio" TO DG-CUSTOMERS::ColumnFilters
      *> Give the third row room for a two-line note; 0 hands it back.
           INVOKE DG-CUSTOMERS::SetRowHeight(3, 44)
```

```cobol
       DG-CUSTOMERS--ONCELLEDITED.
      *> Column 3 is the credit limit: it must be numeric.
           IF DG-CUSTOMERS::EditedColumn = 3
              AND FUNCTION TEST-NUMVAL(DG-CUSTOMERS::EditedValue) NOT = 0
               INVOKE DG-CUSTOMERS::SetCellValue(DG-CUSTOMERS::EditedRow,
                   3, DG-CUSTOMERS::PreviousValue)
           ELSE
               PERFORM SAVE-CUSTOMER-ROW
           END-IF.
```

**把编辑保存回数据源——`Save()`、`AutoCommit`、`DiscardEdit()`。**
绑定到可写数据源的网格——针对索引文件、COBOL 表或 SQL 表的**可写**绑定，或者其 **`DataSource`** 指向一个有主键的索引文件
的网格——会把操作员所做的更改写回那个数据源。三个条件必须同时成立：网格是**绑定的**，绑定是**可写的**，并且网格的
**`AllowCellEditing`** 已开启。任何其他网格的行为恰如上面所述：它显示编辑，其余留给你的处理程序。

单元格编辑会怎样，取决于网格的 **`AutoCommit`** 属性（默认开启，在 **Edit DataGrid settings…** 中位于 *Allow cell editing*
旁边）：

| `AutoCommit` | 运行时做什么 |
|---|---|
| **On** | `onCellEdited` 触发，你的处理程序运行。**当它返回时**，单元格的*当前*值被写入数据源并提交。所以用 `SetCellValue` 把 `PreviousValue` 放回去的处理程序就是**拒绝**了该编辑，什么都不会写入。 |
| **Off** | `onCellEdited` 触发，而编辑保持**待处理**：单元格带有一个小小的琥珀色标记，**什么都不会写入**。由你的代码决定——`DG::Save()` **在一个事务中**写入网格，`DG::DiscardEdit()` 把它恢复成数据源所拥有的样子。 |

```cobol
      *> AutoCommit on: validate, and refuse by putting the old value back.
       DG-ACTORS--ONCELLEDITED.
           IF DG-ACTORS::EditedColumn = 4
              AND FUNCTION TEST-NUMVAL(DG-ACTORS::EditedValue) NOT = 0
               DG-ACTORS::SetCellValue(DG-ACTORS::EditedRow, 4,
                   DG-ACTORS::PreviousValue)
           END-IF.

      *> The operator edits cells, adds and removes rows, then presses Save.
       BTN-SAVE--ONCLICK.
           SET WS-STATUS TO DG-ACTORS::Save()
           IF WS-STATUS NOT = "00"
               DISPLAY "Not saved: " DG-ACTORS::LastError
           END-IF.

       DG-ACTORS--ONSAVED.
           DISPLAY DG-ACTORS::SavedRows " record(s) saved".

       BTN-CANCEL--ONCLICK.
           SET WS-STATUS TO DG-ACTORS::DiscardEdit().
```

**保存写入什么。** `Save()` 写入所有与网格最近一次加载或保存的内容不同的东西，全部在数据源上的一个事务之内，要么全部成功要么
全部不成功，顺序如下：

| 网格中 | 写成 |
|---|---|
| 用 `DeleteRow` 删除的行 | `DELETE` |
| 编辑过的单元格 | 每个更改过的行一个 `UPDATE`（或 REWRITE），带着该行所有更改过的列 |
| 用 `AddRow` 添加的行 | `INSERT`（或 WRITE）——新行需要它的**键**；**空行从不会被插入** |

添加和删除的行只由显式的 `Save()` 写入；`AutoCommit` 只处理单元格编辑。`ClearRows()` 清空显示，并让网格忘记它所加载的内容，所以它
**绝不是**删除表的请求；请用 `RefreshBinding()` 重新加载。没有东西可写的保存不会打开事务，并回答 `00`。当它写了某些东西时，
**`SavedRows`** 说明多少行（更新过的行无论多少个单元格改变都只算一次；插入的或删除的行也一样），并且 **`onSaved`** 触发一次。

**行是按它被加载时的样子——它的键——找到的，绝不按它的位置。** 网格加载时，它会记住每一行以及随它而来的键单元格。所以在编辑与保存
之间对网格排序、筛选，或在你的代码中执行 `Sort`/`DeleteRow`，都不会把编辑送到错误的记录；已经从文件中消失的行回答 `23`。出于同样的
原因，**来自数据源的行的键是只读的**：操作员无法编辑它，而你自己创建的行（`AddRow`）自带它的键。键被你的代码用 `SetCellValue` 更改
过的行**绝不会被写入**：它不会被更新、不会被删除，也不会作为新行插入，而仍在它上面待处理的编辑回答 `21`。

**它在写入之前先检查。** 保存即将更新或删除的每一行，都会先被读回，并与网格所加载的内容比较：对于更新，比较被编辑过的列；对于删除，
比较每一列。如果有人在此期间更改了那一行，整个保存以 **`96`** 失败，什么都不会写入——他们的更改依然有效。别人对你已更新的行的
*另一*列所做的更改不是冲突，并会保留。

每种数据源的写入方式：

- **索引文件**——每条记录先被找到并比较，每个新文本由它字段的 PICTURE 编码（过长的文本，或数字中的字母，会被拒绝），然后才进行
  删除、REWRITE 和 WRITE，并且全部一起提交——或回滚。键已经在文件中的新行，回答该文件的 `22`。
- **COBOL 表**——表**由网格重新生成**：网格的每一行按网格的顺序进入表的各次出现，最后一行之后的各次出现被置空。所以网格中的编辑、
  `Sort`、`AddRow` 和 `DeleteRow` 都成为那张表；表不需要键。空行不会被写入，而行数多于表所能容纳的网格回答 `92`。每个值先按
  `MOVE` 将会存储的方式检查（数字必须是数字），所以一个坏值会让**一个也不**存储。网格不显示的表中数据项保持原样。表没有提交；
  `AutoCommit` 关闭时，编辑留在网格中等待 `Save()`。
- **SQL 表**——`BEGIN`、各个 `DELETE`、每个更改的行一个 `UPDATE <table> SET <field> = …, <field> = … WHERE <key> = …`、各个
  `INSERT`、`COMMIT`，在绑定所指名的 `SqlDatabase` 控件的连接上，该连接必须是打开的（绑定编辑器有一个 **Query**——填充网格的
  SELECT——和一个 **Table**——编辑所更新的东西）。任何语句失败都会发送 `ROLLBACK`，所以它之前的语句也不会保留。每个值都是为该引擎
  转义过的字面量；数字会先检查。新行的键必须手工输入：数据库生成的键不会被读回。处于 `Async` 模式的连接回答 `95`。

**你自己在连接上的事务。** `SqlDatabase` 控件有 `BeginTransaction()`、`Commit()`、`Rollback()` 和 `IsInTransaction()`。在**你的**
事务打开期间进行的保存会**加入它**：它设一个保存点，在其中写入，然后释放它——它不会 `BEGIN` 或 `COMMIT`——所以 `Commit()` 或
`Rollback()` 仍然归你，而失败的保存只撤销**它自己的**写入，不撤销你在它之前做的任何事。所以一个事务中的两次保存就是一次要么全部
成功要么全部不成功的更改。

```cobol
      *> Two grids, one transaction: both saved, or neither.
       BTN-SAVE-ALL--ONCLICK.
           SET WS-OK TO DB-1::BeginTransaction()
           SET WS-STATUS TO DG-ORDERS::Save()
           IF WS-STATUS = "00"
               SET WS-STATUS TO DG-LINES::Save()
           END-IF
           IF WS-STATUS = "00"
               SET WS-OK TO DB-1::Commit()
           ELSE
               SET WS-OK TO DB-1::Rollback()
               DISPLAY "Nothing saved: " DG-LINES::LastError
           END-IF.
```

**保存失败时**，什么都没有写入，并且**旧值被放回去**：网格显示它最近一次加载或保存的内容——你编辑过的单元格、你添加的行和你删除的行
全都被撤销（`AutoCommit` 开启时，只有失败的那次编辑的单元格）。**`CommitStatus`** 和 **`LastError`** 说明原因，**`onCommitError`**
触发一次（`EditedRow`、`EditedColumn`、`EditedValue` 和 `PreviousValue` 指明第一个出错的单元格），而那个单元格带着**红色**标记，直到
操作员再次编辑。要显示冲突中较新的数据，请调用 `RefreshBinding()`。

| `CommitStatus` | 含义 |
|---|---|
| `00` | 已写入——或者没有东西可写。 |
| `21` | 某次编辑会更改键，或某行的键被更改了。 |
| `22`, `23`, `35` … | 索引文件自己的 FILE STATUS：重复键、没有这条记录（网格加载之后它被删除了）、文件缺失。`23` 也指一个不再在网格中的行，或一条没有找到行的 SQL 语句。 |
| `90` | 文件、驱动程序或连接失败（未打开的 SQL 连接就是这个）。 |
| `91` | 网格没有绑定到可以被写入的东西，或者键没有显示在网格中。 |
| `92` | 该值放不进字段（它的 PICTURE，或不是数字）、新行没有键，或 COBOL 表的出现次数比网格的行数少。 |
| `95` | SQL 连接处于 `Async` 模式；保存需要它是 `Sync`。什么都没有写入，而且 `onCommitError` 不会触发。 |
| `96` | 自从网格加载以来，某一行被别人更改了。 |
| `98` | 编辑仍然待处理（见下面的注意事项）。 |

> ⚠️ **注意事项。** **`RefreshBinding()` 不会扔掉待处理的编辑**：它重新加载各行，把待处理的单元格放回到它们之上——放在它们被
> 编辑所在的那些行上，即使重新加载把行重新排序了——并让 `CommitStatus` 停在 `98`，所以请先调用 `Save()` 或 `DiscardEdit()`；
> 自加载以来添加或删除的行不会在重新加载之后保留。绑定到 **SQL** 的网格在你的程序打开连接之后调用 `RefreshBinding()` 时加载——
> 启动时连接尚未打开，网格保持原样。`Save()` 在 `SqlDatabase` 控件自己的连接上运行，所以它会替换该控件当前的结果集。编辑使用文件自己的
> 锁来写入：被另一个进程以写入方式打开的文件回答 `90` 或 `30`，什么都不会写入。在 **MySQL** 上，存储该行已经持有的值的 `UPDATE`
> 报告没有行被更改，保存会把它读成“没有这一行”（`23`）。

**Checkbox 单元格接受点击。** **Edit control** 为 **Checkbox** 的列（在 **Edit DataGrid settings…** 中按列设置）把每个单元格画成一个
方框，勾选或未勾选。在 **`AllowCellEditing`** 开启的网格上，点击方框——或在选中的单元格上按 **Space**——就会切换它。单元格的文本成为该列的
**Checked value** 或 **Unchecked value**；把它们留空，则这一对就是单元格自己的数据已在使用的那一对（`Y`/`N`、`y`/`n`、`1`/`0`、
`true`/`false`、`yes`/`no`、`X`/空格），而空单元格用 `Y`/`N`。设置了它们的列，恰好在单元格的文本等于 Checked value 时把它画成勾选，所以屏幕上的
方框与你读取的值总是一致。

切换是一次普通的编辑：它把新文本写入 `Rows`，并引发带有 `EditedRow`、`EditedColumn`、`EditedValue` 和 `PreviousValue` 的
**`onCellEdited`**，所以你的处理程序可以拒绝它（用 `PreviousValue` 调用 `SetCellValue`），而写回的网格保存它恰如保存键入的值——用
`Save()` 或 `AutoCommit`。双击是两次切换，不会打开文本框。`AllowCellEditing` 关闭，或网格被禁用时，方框只是选中单元格，并且画法完全相同。

```cobol
      *> Done (column 2) is a Checkbox column whose pair is "T" / "F".
       DG-TASKS--ONCELLEDITED.
           IF DG-TASKS::EditedColumn = 2
               IF DG-TASKS::EditedValue = "T"
                   ADD 1 TO WS-DONE-COUNT
               ELSE
                   SUBTRACT 1 FROM WS-DONE-COUNT
               END-IF
           END-IF.
```

**Dropdown 单元格从列表中选取。** **Edit control** 为 **Dropdown** 的列把每个单元格画成带 ▼ 的小框。单元格**保存**条目的*值*，而**显示**它的
*文本*——一列影片类别可以保存 `D`、`C`、`T`，而显示 Drama、Crime、Thriller。在 **`AllowCellEditing`** 开启的网格上，点击单元格——或在选中
的单元格上按 **F2** 或 **Alt+Down**——会在单元格之下打开它的条目列表。**Up** 和 **Down** 在其中移动，**Enter**（或点击）选取，键入一个字母会跳到
第一个以它开头的条目，而 **Escape** 或点击别处会关闭列表且不改变任何东西。列表在单元格已经持有的条目上打开。

选取是一次普通的编辑，就像切换：条目的*值*（不是它的文本）进入 `Rows`，并引发带有 `EditedRow`、`EditedColumn`、`EditedValue`（该值）和
`PreviousValue` 的 **`onCellEdited`**，所以你的处理程序可以拒绝它，而写回的网格用 `Save()` 或 `AutoCommit` 保存它。选取单元格已经持有的条目
不算编辑。

**列表来自哪里。**

| 来源 | 你提供什么 | 何时读取 |
|--------|---------------|------|
| **Static items** | 列设置中每行一个 `CODE=Label`（没有 `=` 的行既是值也是文本） | 网格绘制时 |
| **COBOL table** | 窗体的一张表，以及它的两个数据项：值和显示文本 | 每次网格的绑定加载时——你的 `RefreshBinding()` |
| **Indexed file** | `.cidx` 及其两个字段 | 同上 |
| **SQL control**、**REST API** | 由 Data Binding 编辑器保存，**运行时不读取** | — |

COBOL 表或索引文件列表是在 **Data Binding** 编辑器中的某个字段上设置的（选择 *Dropdown* 作为该字段的控件），并与窗体一起保存；它以列表的
**limit** 为上限（0 表示没有限制）。两个数据项都没有填的出现会被略去，而尚不存在的数据文件就是空列表。因为列表是在绑定加载时读取的，所以在你的程序
更改了表或文件之后，请调用 `RefreshBinding()`。

不在任何条目中的值按原样显示，数字不论怎么写都能找到它的条目（`3` 和 `003` 是同一个），而没有文本的条目显示它的值。**没有**可选条目的 Dropdown 列——
什么都没有配置，或数据源没有给出——像列表出现之前一样按文本编辑。`AllowCellEditing` 关闭，或网格被禁用时，单元格显示它的文本，列表不会打开；它的画法完全相同。

```cobol
      *> Category (column 2) is a Dropdown over D=Drama, C=Crime, T=Thriller.
       DG-FILMS--ONCELLEDITED.
           IF DG-FILMS::EditedColumn = 2
               DISPLAY "Row " DG-FILMS::EditedRow " is now " DG-FILMS::EditedValue
           END-IF.
```

> ⚠️ **注意事项。** 设计器不读取你的表或文件：在它的画布上，这样的单元格显示存储的值（代码），而不是文本。键入为 static items 的列表在那里显示它的文本，
> 而在那里的选取只更改示例数据。

**在设计器中，网格显示的是假数据。** 无论网格是否绑定，画布绘制的都是网格**自己的**列和行——示例数据——而且从不读取数据源：你在设计器中编辑示例，
而绑定唯一的作用，就是在它被应用时（或在你选择 *Reset sample data* 时）**一次性**填充那些列和行。运行时，真实的行取代它们。

**哪个单元格被点击了，以及每行的 Edit/Delete 按钮。** 每次点击单元格都会写入 **`ClickedRow`** 和 **`ClickedColumn`**——数据行和数据列，像
`GetCellValue` 一样从 1 开始编号——就在 `onCellClick`（和 `onCellDoubleClick`）触发之前。用方向键（以及 Page Up/Down、Home、End）移到某个单元格上，算作
点击它：同样的两个属性会被写入，并且 `onCellClick` 为新的单元格触发，所以加载被点击行的处理程序也会跟着键盘走。无法移动的按键，例如第一行上的 Up，
什么都不触发。**Edit control** 为 **Button** 的列（在 **Edit DataGrid settings…** 中按列设置）把每个单元格画成按钮；值为 `icon:<name>` 的单元格改为画
目录中的那个图标，扁平的，用网格的文字颜色——`icon:pencil` 和 `icon:trash` 给每一行它的 Edit 和 Delete 按钮：

```cobol
      *> Rows: name, then the two action columns
           STRING WS-NAME X"09" "icon:pencil" X"09" "icon:trash"
               DELIMITED BY SIZE INTO WS-ROW
           MOVE DG-TOPICS::AddRow(WS-ROW) TO WS-IGNORED.

       DG-TOPICS--ONCELLCLICK.
           EVALUATE DG-TOPICS::ClickedColumn
               WHEN 2 PERFORM EDIT-TOPIC      *> the pencil
               WHEN 3 PERFORM DELETE-TOPIC    *> the trash can
           END-EVALUATE.
```

> ⚠️ **注意事项。** 在 1.70.258 之前，点击不会告诉处理程序任何东西：单元格随事件一起传递，而 COBOL 处理程序从不接收它。

有两件事*不是*单元格点击，假设它们是的处理程序会读到过期的行：

- **点击列标题**会对网格排序（当 `AllowSorting` 开启且该列允许时）并引发 `onColumnClick`。它**不会**写入 `ClickedRow` 或 `ClickedColumn`，它们保持
  最近一次单元格点击留在其中的值。
- **`SelectableText` 关闭时的点击。** 该属性默认开启，也正是它让单元格能被点击：关闭后，单元格忽略鼠标——什么都不会被选中，`ClickedRow` 和
  `ClickedColumn` 不会被写入，`onCellClick` 和 `onCellDoubleClick` 也都不触发。标题仍然会排序。**Button 列是例外：**它的按钮是动作，所以不论哪种情况
  都会接受它们的点击。

**把行交给处理程序的按钮——Content: Button。** 在 **Edit DataGrid settings…** 中，每一列的详细信息以它的 **Content** 开头：

- **Data** 是绑定或 `Columns` 所给的内容。
- **Button** 代表一个*动作*。给按钮一个 **caption**、一个来自系统目录的 **icon**（**…** 打开选择器），或二者都给。于是每一行都显示同一个按钮，不论它的数据是
  什么，并且该列不需要背后有字段：重新应用网格的数据绑定会把它保留在原位。既没有 caption 也没有 icon 的 Button 列保持旧行为：每行显示它自己的
  单元格值。

点击按钮会如上所述引发 `onCellClick`，然后引发 **`onRowButtonClick`**。在选中的按钮单元格上按 Space 或 Enter 效果相同，所以键盘也能触及一行的各个动作。被禁用的
网格的按钮什么都不做。Button 列容纳的是动作，不是值：它没有筛选框，它的标题也不排序。

`onRowButtonClick` 处理程序把整行作为 `DATAGRID-ROW-CONTENT` 接收（见*事件载荷*）。所以它不必按行号和列号去取单元格，而是按名字读取它们：

```cobol
      *> Columns: Actor Id (mask 9(5)) | Actor Name | Edit (Button) | Delete (Button)
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OK PIC 9.
       LINKAGE SECTION.
       COPY DATAGRID-ROW-CONTENT.
       PROCEDURE DIVISION USING DATAGRID-ROW-CONTENT.
           EVALUATE ROW-NAME
               WHEN "EDIT"   MOVE ACTOR-NAME TO TXT-NAME::Text
               WHEN "DELETE" MOVE DG-1::DeleteRow(ROW-NUMBER) TO WS-OK
           END-EVALUATE.
```

- **`ROW-NUMBER`** 是该行在 `Rows` 中的编号，所以排序或筛选之后它也是对的。
- **`ROW-NAME`** 是 Button 列的 COBOL 名称，大写。
- **每个数据列**以它自己的 COBOL 掩码到达，恰如把记录 READ 进相同的描述一样：带符号的数字保留它的符号，而隐含的小数点不占字节。

`COPY DATAGRID-ROW-CONTENT.` 这一行就是整个描述。每次生成都根据网格当时的列，在那里写入该组。所以重命名、添加或移除某列，会在下一次生成时改变处理程序所见的
内容，而你自己的行从不被触动。如果处理程序仍在使用网格已经没有的名称，Check 会报告它。

因为该组的名称就是各列自己的名称，所以 **Check 会拒绝**这样的列名：

- 不能成为 COBOL 数据名，或长于 30 个字符；
- 是 COBOL 保留字；
- 是该组自己的名称之一（`ROW-NUMBER`、`ROW-NAME` ……）；
- 被两列使用；
- 已经是窗体的数据项。

当你键入列名时，设置窗口会在它下面显示同样的警告。Button 列的名称只作为 `ROW-NAME` 的文本到达处理程序，所以 Check 对 Button 列所说的只会是警告：它既没有 caption
也没有 icon，两个 Button 列共用一个名称（处理程序无法区分它们），或者名称长于 `ROW-NAME` 的 30 个字符（处理程序看到的是被截断的）。

**设置窗口的各列。** 在 **Edit DataGrid settings…** 中，各列列在左边，选中列的详细信息在右边。两边各自滚动，所以窗口保持它的大小。COLUMNS 标题有四个按钮：

- **+** 在选中列之后添加一列。
- **−** 在询问之后删除选中的列：它的单元格会从每一行中消失。
- **▲** 和 **▼** 移动选中的列；在列表中拖动一列效果相同，并有一条线显示它将落在哪里。

每一个都是一个 Undo 步骤。每个都像在画布的编辑模式下执行同样的动作那样，更改 `Columns`、`Rows` 和显示顺序。窗口只能通过它的角上的把手来调整大小。

**单元格中的图片——Image 类型。** 在 **Edit DataGrid settings…** 中把列的 **Edit control** 设为 **Image**，每个单元格就显示图片而不是文本。单元格的值是**图片文件的路径**——
PNG、JPEG、GIF、WebP、BMP、TIFF 或 SVG——这使它非常适合缩略图列，其文件由你的程序下载或生成：

```cobol
      *> Columns: Photo (Image) | Name | Edit (Button) | Delete (Button)
           MOVE SPACES TO WS-ROW
           STRING FUNCTION TRIM(WS-THUMB-PATH) X"09"
                  FUNCTION TRIM(WS-NAME) X"09"
                  "icon:pencil" X"09" "icon:trash"
               DELIMITED BY SIZE INTO WS-ROW
           MOVE DG-PETS::AddRow(WS-ROW) TO WS-OK.
```

- **适配。** 图片被缩放以适应单元格，保持它的比例；小图片会被放大到单元格。单元格与行一样高，所以请给网格一个适合缩略图的 `RowHeight`（14–120 点；单独的一行可以用
  `RowHeightOverrides` 更高）。
- **外观。** Image 列的设置增加了 **Image corner radius** 和 **Image drop shadow**。它们在设置中设计；COBOL 没有它们的属性。
- **文件在哪里。** 绝对路径按所写的使用。相对路径先在项目文件夹中查找（在构建出的应用程序中是可执行文件旁边），然后在工作目录中查找。
- **尚不存在的文件。** 单元格显示路径本身而不是空白，网格每半秒再次查找该文件，并为此唤醒窗体。所以你的程序在网格已经显示在屏幕上*之后*才写入的缩略图，会在文件出现后大约半秒之内自己
  出现——不需要刷新调用。存在但不是图片的文件会被读取一次，并被搁置，直到它的修改时间或大小改变。
- **被重写的文件。** 已加载的图片不会在每一帧都被重新读取。网格每隔几秒检查一次文件的修改时间或大小是否改变，如果改变了就加载新图片——所以以同一个名称重新生成的缩略图会在下一次重绘之后不久被拾取。
  在文件系统时间戳精度之内被一个大小完全相同的文件替换，是不会被注意到的；把新图片写在一个新名称之下总是有效。
- **编辑与排序。** Image 列不能就地编辑，并按它的路径作为文本排序。

> **注意。** 只有屏幕上的行才加载它们的图片，所以有成千上万缩略图行的网格在打开时不会读取成千上万个文件。

> **注意。** 在带有 **Data Binding** 的网格上，每当窗体被保存、运行或预览，以及设计器重绘它时，IDE 都会从绑定的字段刷新各列。你设为 Image、Button 或 Gauge 的列在刷新中保持
> 那个类型；只有你在绑定编辑器中设为 Dropdown 或 Checkbox 的字段，才会改变它那一列的控件。

> ⚠️ **注意事项。** 在 `Columns` 中类型为 `image` 的列（`Photo:image`；`img` 和 `picture` 意思相同）也显示图片，但形式更朴素：图片绝不会放大超过它自己的大小，缺失的文件显示一个
> 空的带轮廓的框而不是路径，并且没有圆角半径或阴影。请优先使用 Image **Edit control**。

> 📷 **需要截图——`datagrid-image-cells.png`。** 一个有四行的 DataGrid：一个 Photo 缩略图列（行高 48 点，圆角半径 6，投影开启）、一个 Name 列，以及 Edit 和 Delete 图标按钮。它旁边是
> **Edit DataGrid settings…**，打开在 Photo 列上，**Edit control** 设为 Image，并且圆角半径和投影字段可见。

**运行语言中的标题。** 在 **Edit DataGrid settings…** 中设计的标题只有一种语言。`SetColumnTitle(column, title)` 在程序运行期间替换它——请在你翻译其他标题的任何地方调用它。用你在
设置中给列的 **id** 来指名该列：

```cobol
           INVOKE DG-TOPICS::SetColumnTitle("Name", T-COL-NAME)
```

> **注意。** 从 COBOL 写入 `FrozenColumns`、`FrozenRows`、`ColumnFilters`、`GridLineStyle` 或 `RowHeight` 会立即生效——恰如 `FreezeColumns`、`FreezeRows`、`SetFilter` 和
> `SetRowHeight`——在你用 **Edit DataGrid settings…** 配置过的网格上也一样，那里已保存的设置过去会胜过属性。在设计器中设置的 `ColumnFilters` 让网格一开始就是被筛选的。

**其他功能**

- 虚拟滚动、可调整大小的列/行、列重排（只是显示顺序；源字段的标识得以保留）、以 AND 串联的筛选、冻结窗格、仪表、样式规则、可选文本 + `CopySelection`、`ExportCSV`、
  `RefreshBinding()` 等等。
- **Grid fonts** 和 **Grid line styles**。
- 遵循控件/容器的 `CornerRadius`（内容和边框被裁剪）。
- 对于表绑定，网格在窗体打开时加载（在 `onLoad` 之后），而 `RefreshBinding()` 在你的代码更改了表之后从 working-storage 重新填充它。

绑定时，高级元数据（宽度、样式、顺序、筛选……）会为匹配的字段保留；Data Binding Guardian 防止偏离。完整的集合请见属性窗格。

#### 给 DataGrid 的筛选行着色

打开 `ShowColumnFilters`，每一列都会在它的标题之下增加一个小的输入字段；操作员在那里键入的内容会筛选网格。该字段位于表头带*之内*，所以它需要自己的颜色——表头的文字颜色属于标题，而不属于输入框。

有两个属性承载它们：


| 属性                | 它给什么着色                     |
| ----------------------- | ----------------------------------- |
| `FilterBackgroundColor` | 筛选输入字段的填充  |
| `FilterForegroundColor` | 操作员在其中键入的文字 |

两者都在 DataGrid 样式面板中，位于 `HeaderBackgroundColor` 和 `HeaderForegroundColor` 旁边，并且**默认都是空的**。空不意味着黑色——它意味着*让窗体主题来决定*。所以未动过的网格
从与 TextBox 相同的调色板条目（输入井和正文文字）来绘制它的筛选行，因此不论窗体穿着哪个主题，它都保持可读，并且它随主题而变，而不是把某一个主题的颜色钉死在所有主题上。

设置其中任何一个，你的颜色就按所给的原样使用：

```cobol
           MOVE "#0B1F2A" TO GRID-ACTORS::FilterBackgroundColor.
           MOVE "#E8F4F8" TO GRID-ACTORS::FilterForegroundColor.
```

它们同样可以在设计器中设置一次，而在代码中从不提及。

> **注意——刻意低调的筛选行会得到尊重。** 当你选择颜色时，它们按所写的原样使用，即使这一对的对比度很低。只有*由主题派生的*默认值才会被检查可读性，所以调色板永远不会递给你一个读不出来的筛选字段；
> 你自己的选择从不被质疑。

> ⚠️ **注意事项——占位符不是文本。** 空字段中显示的灰色 `Filter...` 提示，被画成 `FilterForegroundColor` 的变暗形式，而不是一个单独的颜色。如果你选的前景非常接近背景，提示会先于键入的文字褪去——请通过看
> 一个空的列来挑选这一对，而不是看填了内容的列。

从 PowerCOBOL 过来，本能的反应是去找一个带有自己属性表的嵌套“筛选控件”。没有这样的东西：筛选行是 DataGrid 的一部分，而这两个属性就是它样式的全部。

#### 给 Slider 着色

Slider 的滑轨由三个分别着色的部分组成，每个部分各有一个属性：


| 属性     | 绘制                                           |
| ------------ | ------------------------------------------------ |
| `FillColor`  | **已走过**的部分——从 `Minimum` 到 `Value` |
| `TrackColor` | **剩余**的部分——从 `Value` 到 `Maximum` |
| `ThumbColor` | 旋钮本身                                  |

保持默认时，由当前主题绘制这三者，并且已走过的部分是被高亮的那一部分。这三个属性优先于 Appearance 部分的 `BackgroundColor`（滑轨）和
`ForegroundColor`（旋钮），后两者对设置了它们的窗体仍然有效。

> **注意。** 如果你是从 PowerCOBOL 过来的，这正是你从滑动条（track bar）所期望的划分：“已完成”的一侧带有颜色，而仍待走过的一侧保持中性。

#### 用键盘操作 Slider 和 NumericUpDown

点击其中任一控件（或用 Tab 切到它），它就获得键盘：


| 按键                  | Slider                        | NumericUpDown        |
| -------------------- | ----------------------------- | -------------------- |
| ↑ / →                | 按 `Step` 增加                  | ↑ 按 `Step` 增加       |
| ↓ / ←                | 按 `Step` 减少                | ↓ 按 `Step` 减少     |
| Page Up / Page Down  | 按 `LargeChange` 增加 / 减少    | —                    |
| Home / End           | 到 `Minimum` / `Maximum`      | —                    |

每一次移动都保持在 `Minimum..Maximum` 之内，并落在 `Step` 网格上，就像拖动那样——所以当 Slider 预置的 `Step` 为 10 时，`LargeChange` 为 25 会落在最近的 10 的倍数上。一次按键就是
一次完成的更改：Slider 立即引发 `onValueChanged`，而不只是在拖动结束时。`Increment()` / `Decrement()` 方法同样停在 `Minimum` 和 `Maximum`，并且在 `Step` 为小数时按小数步进。

**NumericUpDown** 以 `DecimalPlaces` 位数字显示它的值（绝不少于 `Step` 所带的位数，所以 `Step` 为 0.25 时总是可见），并且在 `ThousandsSeparator` 开启时按千位分组——`12,345.50`。
`Value` 本身仍是纯数字。`ReadOnly` 开启时，操作员无法通过拖动、滚轮或按键更改它；它仍然被显示并且可以获得焦点，而你的程序仍然可以设置它。

在**垂直**的 Slider 上，`TickStyle` 为 `Top` 时把刻度放在左边，为 `Bottom` 时放在右边。

Slider 在滑轨两端打印它的 `Minimum` 和 `Maximum`。对于只有滑轨和旋钮的滑块，请关闭 `ShowRange`——如果 `TickStyle` 也是 `None`，水平的滑轨就会在控件中居中。

#### 给 ProgressBar 设置样式

进度条报告 `Value` 位于 `Minimum` 与 `Maximum` 之间的什么位置。这些属性决定这个读数看起来如何：


| 属性                 | 绘制                                                                                                                                                                                                                                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Orientation`            | `Horizontal` 从左到右填充；`Vertical` **从下到上**填充，像一根上升的柱子。                                                                                                                                                                                               |
| `Style`                  | `Continuous` 绘制一整段不间断的颜色；`Blocks` 绘制一排分段。                                                                                                                                                                                                        |
| `BlockSize`              | 一个块沿进度条前进的轴线有多长，以像素计。只有 `Blocks` 使用它，所以选择该样式之后，这一行才会出现在属性窗格中。**0**——默认值——按进度条自己的厚度来决定每个块的大小，所以高的进度条得到长的块，细的得到短的块。 |
| `BarColor`               | 已填充的部分——它走了多远。保持默认时，进度条采用当前主题的绿色，所以它像其他每个控件一样属于它周围的调色板；你选择的任何颜色都会胜出。                                                                                                                                                              |
| `BackgroundColor`        | **槽**——尚未走过的部分；这是 Appearance 窗格中的 *Back colour* 行。保持默认时它跟随当前主题，一如既往；你选择的任何颜色都会胜出。进度条的两半现在都归你了：这一行过去在这里什么也不做，因为槽只向主题询问颜色。                           |
| `ShowValue`              | 在进度条中央画出百分比。                                                                                                                                                                                                                                                                        |
| `ForegroundColor`        | 百分比的颜色。保持默认时，进度条会挑选一种在主题所绘制的槽上能读出来的颜色。                                                                                                                                                                          |

`CornerRadius` 对进度条的圆化方式与它对每个其他带边框控件的方式相同（见上文*圆角半径*）——槽、已填充部分和边框一起，为 `0` 时是方的。进度条是唯一**不**从 `0` 开始的控件：它生来就是圆的，为 `10`。
框架本身与任何其他带边框的控件一样，响应相同的 `BorderStyle`、`BorderColor` 和 `BorderWidth`，而 `BorderStyle = None` 让进度条完全没有框架。

> **注意。** `Blocks` 进度条从不隐藏小的进度：最后一个块被裁剪到 `Value` 到达的位置，所以只前进了 3 % 的进度条显示一小片，而不是什么都没有。

> **从 PowerCOBOL 来的？** 这就是你已经从 Windows 进度控件所熟悉的两种样式——平滑的和分段的——只是块的长度掌握在你手中，而不是由控件的高度固定。

#### Knob、Gauge 和 Switch

**Knob** 是一个旋转拨盘，用户拖动它，在 `Minimum..Maximum`（默认 0–100）之内设置一个数值 `Value`。属性：`Step`（`Increment()`/`Decrement()` 的增量，二者停在 `Minimum` 和 `Maximum`）、
`DefaultValue`（在旋钮上**双击**，或从 COBOL 调用 `Reset()`，会把它恢复到的值）、`Accent`（弧线和指示器的颜色——任何颜色，来自属性窗格的选择器）、`Bipolar`（弧线从拨盘的顶部——范围的中点——向
值填充，往哪边都可以：声像或平衡控制）、`ShowValue`（画出数值读数），以及 `Label`（居中在拨盘之下、读数下面的标题；拨盘会缩小以腾出空间）。

另外三个属性绘制拨盘本身，这原先完全由主题拥有：`FaceColor`（指示器在其上转动的圆形表面）、`RimColor`（边缘和围绕该表面的细内环），以及 `TrackColor`（弧线中尚待走过的部分，从 `Value`
绕到 `Maximum`）。每个默认为空，而空表示当前主题完全照旧绘制该部分，所以你从未着色的旋钮看起来没有变化。`Accent` 仍然同时涵盖已走过的弧线和指示器。边缘的填充是表面颜色的提亮，所以仅设置 `FaceColor` 就
承载了整个拨盘。透明色（`#00000000`）也是一种选择：该部分不被绘制，所以透明的表面让拨盘后面的东西透出来。

`IndicatorStyle` 说明值如何显示：`Line`（默认）是横过表面的指针；`Thumb` 是沿着弧线、停在该值处的圆盘。表面透明并用 thumb 时，拨盘的中央是空的——把 Label 叠在它上面做恒温器的读数（"COOLING TO 28°C"），并保持
`ShowValue` 关闭。

它的主要事件是 `onChange`（也叫 `onValueChanged`），在用户拖动时触发。方法：`SetValue()` / `GetValue()` / `Increment()` / `Decrement()` / `Reset()`——与 `Slider`/`NumericUpDown` 相同的值控件契约。

**Gauge** 是**只读**的 KPI 显示——它从不因用户交互而改变，只因你自己的 COBOL（`SetValue()` 或 `SET Gauge1::Value TO …`）而改变。`GaugeStyle` 选择基本外观：`Radial`（指针 + 刻度，加上
`ShowNeedle`/`ShowScale`）、`Linear`（水平条，加上 `BarHeight`/`ShowThumb`），或 `Donut`（完整的环，加上 `StrokeWidth`——并且它画出与 Radial 相同的 `ShowNeedle` 指针，从顶部扫过整个圆，用仪表自己的颜色）。
`Color` 覆盖填充（空 = 主题强调色）；`NeedleColor` 给指针及其轴心一个自己的颜色，与仪表的颜色无关（空 = 仪表的颜色，这过去是指针唯一拥有的墨色）；`Unit` 在每种样式中给数值读数追加一个后缀；`Text` 覆盖整个读数字符串。

`Unit` 与数字之间的间隔，采用读者会书写的方式：以字母或数字开头的单位得到一个空格——`"Parts"` 读作 `23 Parts`，`"rpm"` 读作 `1450 rpm`——而符号则紧贴着它：`"%"` 读作 `23%`，`"°C"` 读作 `19°C`，`"$"` 读作 `40$`。
你键入的前导空格会被原样保留，所以 `" rpm"` 仍读作 `1450 rpm`。

`ReadoutPosition` 选择 **Radial** 在哪里打印该读数：`Up`（默认）在拨盘之内、指针枢轴之上，或 `Down` 在枢轴之下 5 px，这是速度表打印它的数字的地方。在 `Down` 时拨盘让出那么多高度，所以读数总是落在控件之内。该属性只对 Radial 有效——`Donut` 在它的环的中央读出，
`Linear` 在它的条的旁边读出，两者都没有第二个地方可放。

同时设置 `WarningThreshold` 和 `CriticalThreshold`——`Minimum..Maximum` 跨度的分数，介于 `0.0` 和 `1.0` 之间——可打开自动分区着色。填充随后**沿着自己的那一段保持每个区的颜色**：到警告线为止是绿色，从那里到临界线是琥珀色，
再往上是红色。读数为 88、两条线在 70 和 90 的仪表，到 70 为止是绿色，从 70 到 88 是琥珀色——完全没有红色，因为读数从未到达它。指针（以及 `Linear` 的拇指）采用读数*所处*的那个区的颜色，所以它仍然一眼就能说明你
在哪个区。分区开启期间，它们拥有填充颜色，所以 `Color` 被忽略；把任一阈值留空，就可保持分区关闭、由 `Color` 做主。

**这三种颜色归你**——检查器中的 **Normal zone**、**Warning zone** 和 **Critical zone**（`NormalColor`、`WarningColor`、`CriticalColor`），每个都来自同一个颜色选择器，具有同样的颜色记忆，与 IDE 中其他每个颜色行一样。每个起初为空，表示仪表一直绘制的内置绿色 `#2E7D32`、
琥珀色 `#F57C00` 和红色 `#C62828`，所以你从未改过样式的仪表看起来与以前完全一样。在 1.61.154 之前，这三个是固定在平台中的，而在一个其他每种颜色都是属性的控件上。

> ⚠️ **注意事项。** 阈值是跨度的分数，不是跨度上的读数。在 `0..250` 的仪表上，`0.8` 是位于 200 的警告线——而不是 `200`。

**Switch** 是布尔的开/关切换：`Checked`（Boolean）以及它的 ON 轨道的颜色，检查器称之为 **Checked color**——任何颜色，来自其他每个颜色行所用的同一个选择器（和同样的颜色记忆）。存储的属性仍然是 `Accent`，并且
`Blue` / `Green` / `Red` / `Purple` / `Amber` / `Sky` 这六个名称仍然可以解析，所以用其中之一保存的窗体会保留它；在 1.61.152 之前，Switch 只接受这六个，而且标题借用自某一个主题的调色板。它的主要事件是 `onClick`；方法是
`IsChecked()` / `SetChecked()` / `Toggle()`——与 `CheckBox` 相同的勾选控件契约，只是没有 `Select()`（Switch 没有单选组的概念）。

这三者都**可以作为独立的标量目标进行数据绑定**——与上面的 DataGrid/Chart/ComboBox/数组目标不同，单独的 Knob、Gauge 或 Switch 可以直接绑定到一个源字段，不需要重复组。每当绑定刷新时，绑定的字段会自动驱动 `Value`（Knob/Gauge）或 `Checked`（Switch）。

#### ListBox——活动行、选择集与勾选集

ListBox 携带三样各自独立的东西，窗体读取它需要的那一样：


| 属性                  | 它保存什么                                                                                                                         |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `Value` / `SelectedIndex` | **活动**行——光标所在的那一行，以完整的高亮绘制。                                                             |
| `SelectedItems`           | 用户用 Ctrl-click（Mac 上是 Cmd）建立的**选择集**，以同一高亮的变暗版本绘制。需要 `MultiSelect`。 |
| `CheckedItems`            | 在 `ShowCheckBoxes` 开启时，被**勾选**的行。                                                                                      |

它们刻意分开。点击一行会使它成为活动行，*并且*开始一个单行的选择集；Ctrl-click 把一行加入选择集或把它从中取出，并且无论哪种情况都把光标移到那里。勾选一个方框只改变
`CheckedItems`——活动行不动——并引发 `onItemChecked`，所以列表可以同时既是一组选项又是一个光标。`CheckedItems` 保持用户勾选的顺序，缺口也保留；它不是一段连续的范围。

```cobol
      *>   every ticked row, one per line:
           MOVE LIST-1::CheckedItems TO WS-TICKED
      *>   …and the row the cursor is on:
           MOVE LIST-1::Value        TO WS-ACTIVE
```

**操作员如何在列表中移动。** 三种手势，它们都在两端停住，而不是回绕或跑出去：


| 手势            | 它做什么                                                                                                                                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Click**          | 使该行成为活动行，并开始一个单行的选择集。                                                                                                                                                              |
| **Press and drag** | 以所按下的行为锚点，并延伸到指针之下的那一行——*向上或向下*。反转方向会把范围**缩回**。拖到第一行之上时停在第一行；拖到最后一行之下时停在最后一行。 |
| **↑ / ↓**        | 在列表被点击（或用 Tab 切到）之后，把活动行移动一行。                                                                                                                                                                  |

在 `ShowCheckBoxes` 开启时还有第四种：**Space** 勾选活动行，或在它已被勾选时取消勾选——相当于点击它的方框，只是用键盘——并像点击那样，带着整个勾选集引发 `onItemChecked`。
与 ↑ / ↓ 一起，这就是一个不用鼠标就能操作的勾选列表。没有勾选框的列表不理会 Space。

无论什么移动了活动行，列表都会**滚动以让它保持在视野中**，把它落在第一个或最后一个可见行上——所以拖过框架底部的拖动会带着视图一起走，而操作员永远不会选中一个他们看不到的行。滚轮和滚动条仍然各自滚动
列表；拖动是选择，不是轻扫。

**外观归你。** ListBox（和 TreeView）穿着你设计的背景——**Background color**，或带起点、终点和方向的 **Background gradient**——连同它的边框和圆角半径，在每个界面上：设计画布、预览、Run Form 和编译出的二进制。

> **TreeView，自 1.61.153 起。** `Items` **就是**树：每行一个节点，每一层级缩进**两个空格**（或一个制表符）。它由一个渲染器在画布和运行中的窗体上绘制，所以你布置的就是运行的——
> 在此之前，画布显示的是一个没有任何节点的 `[TreeView]` 占位符，而运行中的窗体是固定 12pt 字体的扁平项目符号列表。
>
> 树用控件自己的 **FontName / FontSize / Foreground color** 写出它的节点，按 **Show lines** / **Root lines** 以 **LineColor** 画它的连接线，在 **Checkboxes** 之下给每个节点一个勾选框，
> 并在 **Hot tracking** 之下把指针之下的行抬起。点击选中（`SelectedNode`、`onNodeClick` / `onNodeSelect`）；**点击勾选框**则改为勾选，而被勾选的节点在 `CheckedNodes` 中，每行一个，
> `onNodeCheck` 指明节点。它还增加了 **Border style** 和 **Border width**：它原先有 `BorderColor`，却没有办法选择被着色的边框。
>
> **它会折叠，自 1.61.157 起。** 下面有任何东西的节点会画一个展开箭头——关闭时朝右，打开时朝下。点击它会写入 **`CollapsedNodes`**（一个**关闭**的内容的列表，所以空表示整棵树都是打开的）
> 并引发指明该节点的 `onNodeCollapse` / `onNodeExpand`，处理程序正是借此在第一次打开时加载子项，而不必自己跟踪状态。从 COBOL 写入 `CollapsedNodes` 可以把树折叠成任何形状，而不触动 `Items`。
>
> **并且它有图标**——来自平台自己的目录，与菜单和工具栏所用的是同一批 1100 多个图标。节点在它的 `Items` 行中、一个 **TAB** 之后指名它自己的图标，就像 Markers 和 Routes 指名它们的那样：
>
> ```text
> Warehouse	box
>   Bolts	wrench
> ```
>
> 自 1.61.161 起，你**挑选**树的三个默认图标——**Folder icon (shut)**、**(open)** 和 **Leaf icon**，描述见下——而不是拼写它们：检查器中的每一行都带有一个打开图标目录的 **…** 按钮——与工具栏编辑器所用的是同一个——
> 以及一个把该行清回到平台自己默认值的 **✕**。清除写入的是*空*，而不是当今默认值的名称，所以该行继续跟随平台，而不是把一个答案冻结进 `.cfrm`。预览、两个按钮和名称都在同一个带标签的单元格中，就像 Button 的图像行那样。
>
> **Nodes** 框本身上限为十二行，超过则滚动，所以有六十个节点的树不会再把它下面的每个属性都推出窗格的底部。
>
> 没有指名图标的节点采用 **Folder icon (shut)** / **(open)** / **Leaf icon**——默认是 `folder`、`folder-open` 和 `doc-text`，所以未经改动的树看起来就像一棵树。**Show icons** 关闭这一列，标签会收回那块空间。
>
> **一行的任何东西都不再是固定的了：** **Row height**、**Indent per level**、**Icon size** 和 **Checkbox size** 都是属性，**Icon color**、**Selected row** 和 **Hot-track row** 也是。箭头的位置在
> *每一*行上都被保留，不论节点是否折叠，所以标签对齐成一列——只为父节点保留它，会让叶子的标签滑到它自己父节点的标签的左边。
>
> **高对比度文字默认开启。** 节点的墨色按对比度，针对树实际绘制所在的表面来挑选，所以它在白色表面、深色卡片或玻璃表面上都保持可读，而不必被告知。关闭 **High-contrast text**，则使用主题自己的文字颜色；明确设置的
> **Foreground color** 优先于两者。
>
> **哪个节点触发了？** 每个节点事件都把节点交给它的处理程序，放在设计器为你生成的 LINKAGE 组中：
>
> ```cobol
>        LINKAGE SECTION.
>        01 CONTROL-NODE-DATA.
>           05 CONTROL-NODE                 PIC X(256).
>           05 CONTROL-NODE-INDEX           PIC S9(4) COMP-5.
>           05 CONTROL-NODE-LEVEL           PIC S9(4) COMP-5.
>           05 CONTROL-NODE-CHECKED         PIC 9.
>
>        PROCEDURE DIVISION USING CONTROL-NODE-DATA.
> ```
>
> `CONTROL-NODE` 是标签——`SelectedNode`、`CheckedNodes` 和 `CollapsedNodes` 都使用的键。`CONTROL-NODE-INDEX` 是它在 `Items` 中**按你所写**的从 1 开始的行号，所以 `Sorted` 可以重新排列显示，而不必给你的处理程序重新编号；
> `CONTROL-NODE-LEVEL` 是它从 1 开始的深度；`CONTROL-NODE-CHECKED` 在它的方框被勾选时为 `1`，未勾选（或树没有方框）时为 `0`。这是平台的三种事件载荷之一，另两种是
> `CONTROL-ARRAY-INDEX` 和 DataGrid 的 `DATAGRID-ROW-CONTENT`——在 1.61.158 之前，`onNodeCheck`、`onNodeCollapse` 或 `onNodeExpand` 的处理程序无法分辨是哪个节点动了。
>
> **遍历树，自 1.61.159 起。** 知道哪个节点触发了只是一半；另一半是从那里找到你的路。`CONTROL-NODE-INDEX` **就是节点的句柄**——下面的每个调用都接受它，而遍历调用*返回*一个句柄，所以它们可以串联：
>
> ```cobol
>       *> Climb from the node that fired to the one it hangs under.
>            MOVE TREE-1::NodeParent(CONTROL-NODE-INDEX) TO WS-IDX
>            IF WS-IDX > 0
>                MOVE TREE-1::NodeText(WS-IDX) TO WS-PARENT-NAME
>            END-IF
>
>       *> Run along everything under it — and no further.
>            MOVE TREE-1::NodeFirstChild(CONTROL-NODE-INDEX) TO WS-IDX
>            PERFORM UNTIL WS-IDX = 0
>                MOVE TREE-1::NodeText(WS-IDX) TO WS-NAME
>                DISPLAY "child: " WS-NAME
>                MOVE TREE-1::NodeNextSibling(WS-IDX) TO WS-IDX
>            END-PERFORM
> ```
>
> **`0` 表示没有这样的节点**——根节点之上没有父节点，最后一个之后没有兄弟节点——这正是结束循环的东西。兄弟遍历从不下降到子节点，也从不逃到下一个父节点中。
>
>
> | 调用                                                | 回答                                                       |
> | --------------------------------------------------- | ------------------------------------------------------------- |
> | `NodeParent(i)`                                     | 它所挂靠的节点，根节点为 `0`                        |
> | `NodeFirstChild(i)` / `NodeLastChild(i)`            | 它的第一个 / 最后一个直接子节点                                 |
> | `NodeNextSibling(i)` / `NodePrevSibling(i)`         | 同一层级、同一父节点下的下一个 / 上一个节点       |
> | `NodeChildCount(i)` / `NodeHasChildren(i)`          | 只算直接子节点——孙节点不是子节点        |
> | `NodeText(i)` / `NodePath(i)` / `NodeLevel(i)`      | 它的标签、它的 `Root/Child/Leaf` 路径、它的深度（1 = 根节点）  |
> | `NodeIcon(i)` / `NodeColor(i)` / `NodeBackColor(i)` | 节点自己携带的东西                                  |
> | `NodeChecked(i)` / `NodeCollapsed(i)`               | `1`/`0`，从实时的 `CheckedNodes` / `CollapsedNodes` 读取 |
> | `NodeCount()` / `NodeIndexOf(text)`                 | 有多少个节点；你已知的标签的句柄       |
> | `RemoveNode(i)`                                     | 移除该节点及其下的一切；返回 `1`，如果没有这个节点则返回 `0` |
> | `ExpandAll()` / `CollapseAll()`                     | 打开每个节点 / 折叠每个有子节点的节点           |
> | `GetSelectedNode()` / `SetSelectedNode(label)`      | 按标签读取 / 设置选中的节点                        |
>
> 句柄从 **1** 开始计数——`Items` 的第一行是节点 1——所以 `CONTROL-NODE-INDEX` 中的数字可以直接放进上面的任何调用。（在 1.70.156 之前，这些调用从 0 开始计数，并悄悄地读取了事件所指名节点*之后*的那个节点。）
>
> 刻意**没有可以持有的节点对象**。你保存的句柄会在 `Items` 在它之下改变的那一刻变得过期；而索引只是针对树现在所持有的内容被重新读取。出于同样的原因，询问一个不存在的节点会回答*空*而不是抛出——遍历设计上就会跑到树的末尾，而
> `0` 是守卫，不是每个循环都得去捕获的错误。
>
> **从 COBOL 构建树：**使用 `AddNode`，**不要**用 `AddItem`。
>
> ```cobol
>            TREE-1::AddNode(1, "Warehouse")
>            TREE-1::AddNode(2, "Inbound")
>            TREE-1::AddNode(3, "Dock A")
> ```
>
> ⚠️ `AddItem` **会修剪它的参数**——它不得不这样，因为 `PIC X` 字段到达时被空格填充——而节点的层级*就是*前导空格，所以缩进的字面量永远不可能构建出子节点。`AddNode` 把层级作为数字接收，说出了一对空格只是暗示的东西。
>
> **节点可以给自己着装，自 1.61.159 起。** `Items` 行是 `label`，后面最多跟三个它自己的、以 TAB 分隔的字段：
>
> ```text
> label ⇥ icon ⇥ colour ⇥ background
> ```
>
> 所以 `Overdue⇥⇥#C81E1E` 是一个用红色书写、图标留给树决定的节点——每个字段都是可选的，空字段表示“按树的画法”。行颜色绘制在选择条**之下**，所以带颜色的行在被选中时仍然可见。`AddNode` 也写入这些：
> `TREE-1::AddNode(1, "Overdue", "alert", "#C81E1E", " ")`。
>
> **勾选框的装扮像 CheckBox，自 1.61.159 起。** 它穿着同样的六个属性，含义相同：**Box colour**、**Box border**（带它的颜色和宽度）、**Tick colour** 和 **Tick size %**——并且它画出同样的勾号。在此之前，它是一个黑色的
> 井、一个 1px 的边缘和一个占方框 28 % 的勾：画笔里的三个数字，没有一个可以触及。
>
>> **注意。** **Checkbox size** 是方框，以点计；**Tick size %** 是勾号填满该方框的多少。这与 CheckBox 的划分相同，那里方框来自字体，只有勾号有百分比。
>>
>
> **它会滚动，自 1.61.160 起。** 比你画的控件更高的树过去会把溢出的部分丢在地上——节点在那里，但没有什么能触及它们。三种移动它的方式，你不需要为其中任何一种设置属性：
>
> - **滚轮**，当指针在树上时；
> - 在它上面任何地方的**拖动**（点击仍然选中——两者通过指针是否移动来区分）；
> - 在它获得焦点（点击会给它焦点）之后的 **Up / Down / Home / End**。选择会逐步走过树所显示的每一行，包括滚出视野之外的行，而视图**只在必要的程度上**跟随，以把新行带到屏幕上。
>
> 跨在边缘上的行被绘制并裁剪，而不是被丢弃，所以树是滑动的，而不是一次跳一行——而那半行正是操作员得知下面还有更多的方式。
>
>> **注意。** 树能滚动多远，是按它*显示*的行来测量的，所以折叠一个分支会让它变短。并且刻意**没有滚动属性**：操作员滚动到了哪里是视图状态，不是设计，它不会被写入 `.cfrm`。
>>
>
> **一行永远不会缩到它所容纳的内容之下。** `RowHeight` 是下限，所以增大 **Icon size** 或 **Checkbox size** 会让行随之增长，而不是让大图标画到它的邻居上；**Gap between nodes**
> （`NodeSpacing`）在此之上再增加空间。
>
> **就地重命名节点——Allow rename（`AllowEdit`）。** 默认关闭。打开它，操作员双击节点的标签，或在选中的节点上按 F2，标签就作为文本框打开：Enter 或点击别处保留新名称，Escape 放弃，而空名称会被拒绝。保留下来的名称
> 被写入 `Items`——节点的缩进、图标和颜色保持原样——`SelectedNode`、`CheckedNodes` 和 `CollapsedNodes` 跟着它走，并且 **`onNodeRenamed`** 带着节点（新标签在 `CONTROL-NODE` 中）和 `PreviousNodeText` 中的旧标签触发。存储新名称是你的
> 处理程序的工作；要拒绝它，把 `Items` 写回去。
>
> ```cobol
>        TRV-1--ONNODERENAMED.
>            MOVE TRV-1::PreviousNodeText TO WS-OLD-NAME
>            MOVE CONTROL-NODE            TO WS-NEW-NAME
>            PERFORM RENAME-DEPARTMENT.
> ```
>
> **把一个节点拖到另一个上——Allow drag（`AllowDrag`）。** 默认关闭，此时拖动会滚动树。打开它，操作员按下一个节点并带着它走：指针之下的节点被圈出，标签跟着指针走，并且当指针接近树的顶部或底部边缘时树会滚动（滚轮仍然滚动）。在另一个节点上放手会触发
> **`onNodeDrop`**。它的处理程序以通常的四个名称接收被拖动的节点，外加另外两个：`CONTROL-TARGET-INDEX`（被放下的目标节点）和 `CONTROL-TARGET-NODE`（该节点的标签）。在树的空白处放手，给出的目标索引是 **0** 和空白标签，你可以把它读作“顶层”。
> 在树之外放手，或放回同一个节点上，什么也不触发。树自己不移动任何东西：由你的处理程序决定放下意味着什么，去做它，并重建 `Items`。
>
> ```cobol
>        TRV-1--ONNODEDROP.
>            IF CONTROL-TARGET-INDEX = 0
>                MOVE SPACES TO WS-NEW-PARENT
>            ELSE
>                MOVE CONTROL-TARGET-NODE TO WS-NEW-PARENT
>            END-IF
>            MOVE CONTROL-NODE TO WS-MOVED
>            PERFORM MOVE-ITEM.
> ```

**高亮也一样归你。** 高亮行背后的颜色和其他任何属性一样，是一个属性，而且有两个，因为列表高亮两种不同的东西：


| 属性             | 检查器行     | 它背后的高亮                                                              |
| -------------------- | ----------------- | --------------------------------------------------------------------------------- |
| `ActiveItemColor`    | **Active row**    | 活动行——`Value` / `SelectedIndex` 所报告的那一行。                       |
| `SelectedItemsColor` | **Selected rows** | `MultiSelect` 选择集中的*其他*行——`SelectedItems` 所报告的那些。 |

把任何一个留**空**，就表示*你没有选择*：活动行采用主题自己的选择颜色，选择集采用把该颜色变暗到 45 % 的颜色——这就是这些属性存在之前列表所绘制的，所以你已经设计的任何东西都不会改变。变暗的颜色跟随活动颜色最终是什么，所以单独设置
**Active row** 会重新给整个列表设置样式，并让两者保持关联。一旦你设置了一个颜色，它就被钉住了；该行的 **↺** 把它交还给主题。

钉住比看起来更重要。主题颜色不是一个颜色：IDE 内的预览带着 IDE 主题的颜色，而编译出的二进制带着它自己的。指名了它的高亮的列表，才是在设计器中、在 Run Form 之下以及在你交付的应用程序中看起来相同的列表。

两者都接受运行时写入，所以高亮可以回应数据：

```cobol
      *>   an overdrawn account highlights in red while it is being reviewed
           IF WS-BALANCE < 0
              MOVE "#B00020" TO ACCOUNTS-LIST::ActiveItemColor
           ELSE
              MOVE "#1B7F3B" TO ACCOUNTS-LIST::ActiveItemColor
           END-IF
```

> **注意。** ListBox 不能被画得比它自己的一行文字更矮——设计器的调整大小会停在那里，而下限随 `FontSize` 升高。

> **设计条目。** 检查器的 **Items (one per line)** 框显示五行，超过则滚动，所以有五十个条目的列表不会再把检查器的其余部分推出窗格。

#### ComboBox 与 ListBox——来自文本文件的条目

条目存放在纯文本文件中的列表——例如一个国家的各个州，或一组产品代码——不必键入 `Items`，也不必在循环中用 `AddItem` 构建。指名该文件，列表就会读取它。

**在设计器中。** 在列表的 `Items` 之下，**Items file** 行有一个 📂 按钮：选一个 `.txt`，它的路径就存入 `ItemsFile` 属性（文件在项目之内时相对于项目）。✕ 清除路径**以及**条目。Preview 立即显示该文件的条目。

**运行时。** 文件在**每次窗体打开时**被读取，每行一个条目，空行被略去。在 Run Form 之下、嵌入的窗体中和构建出的应用程序中同样会读取它，所以编辑文本文件就改变列表，而不必触动窗体。如果无法读取该文件，则改为显示设计的
`Items`。

> ⚠️ **把文件放在应用程序能找到的地方。** 构建出的应用程序在它的可执行文件旁边解析路径，而构建会把项目的 `assets/` 文件夹复制进 `dist/`。因此放在 `assets/` 中的文本文件随应用程序一起走；放在别处的则不会。

**从 COBOL。** 两个方法在任何时刻都做同样的事：

```cobol
      *>   Replace the items with the file's lines; the selection is cleared.
      *>   Returns how many items were loaded, or -1 if the file could not
      *>   be read (the items are then left as they were).
           MOVE ComboBox-1::LoadFromFile("assets/states.txt") TO WS-COUNT
           IF WS-COUNT < 0
               MOVE "The list of states is missing." TO Lbl-Status::Caption
           END-IF

      *>   Empty the list and its selection.
           INVOKE ComboBox-1 "Clear"
```

`LoadFromFile` 中的相对路径与 `ItemsFile` 以同样的方式解析。`WS-COUNT` 应当带符号（`PIC S9(4)`），这样它才能容纳 `-1`。

#### ComboBox——三种样式

ComboBox 是三种东西之一，由 **DropDownStyle** 选择——与 Windows 组合框一直提供的那三种相同：

| DropDownStyle              | 操作员得到什么                                                                                                   |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| **DropDown**（默认） | 一个可以**键入**的文本字段，右边有一个 ▼ 按钮。按文本会放置插入符；只有按钮才会打开列表。 |
| **DropDownList**           | 仅可选取。在组合框的任何位置按下都会打开列表，并且键入被拒绝。                                           |
| **Simple**                 | 文本字段，列表**总是显示在它的下方**，位于控件之内。没有下拉，所以没有 `onDropDown`。  |

**Editable** 在不改变外观的情况下关掉键入：`Editable` 关闭的 `DropDown` 组合框的行为与 `DropDownList` 完全一样。

键入的内容成为 `Value`——**即使它不是条目之一**，这正是接受文本的组合框的意义所在（一个还不在列表中的城市）。`SelectedIndex` 随之变化：文本恰好指名的那个条目，或者 `0`。每次击键都会引发
`onChange` 和 `onTextChanged`，并且在列表打开期间，高亮会跳到第一个以所键入内容开头的条目，所以 Enter 会选中它。

```cobol
       CBO-CITY-ONCHANGE.
           MOVE CBO-CITY::Value         TO WS-CITY
           MOVE CBO-CITY::SelectedIndex TO WS-CITY-IX
           IF WS-CITY-IX = 0
               DISPLAY "New city typed: " WS-CITY
           END-IF.
```

> **注意。** 新的 ComboBox 是 `DropDown`——可以键入。对于操作员必须从中选择的列表，请把 **DropDownStyle** 设为 `DropDownList`，就像你在 PowerCOBOL 或 isCOBOL 中那样。

> ⚠️ **注意事项。** 在 1.70.232 之前，`DropDownStyle` 和 `Editable` 都没有被读取：每个组合框的行为都像仅可选取的列表，不管它声明了什么。当时用默认的
> `DropDown` 构建的窗体，现在会让操作员键入。在那不是你想要的地方，请设置 `DropDownList`。

#### ComboBox——手势、外观与展开下拉列表的颜色

**操作员如何在下拉列表中移动。** 与 ListBox 响应的是同样的三种手势，它们都在两端停住，而不是回绕或跑出去：


| 手势              | 它做什么                                                                                                                                                                                                                                                                                                                                        |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Click the header** | 打开列表（在可键入的 `DropDown` 组合框上，点击 ▼ 按钮——点击文本会放置插入符）。它*不会*同时选中指针之下的任何东西。                                                                                                                                                                                 |
| **Press and drag**   | 按下表头，拖进列表，在某个条目上松开以选中它——经典的组合框手势。高亮*向上或向下*跟随指针；反转方向会把它走回来。拖到第一个条目之上时停在第一个；拖到最后一个之下时停在最后一个，所以离开控件的拖动会停在某个条目上，而不是什么都没选。 |
| **↑ / ↓**          | 在组合框被点击（或用 Tab 切到）之后，逐个走过条目。                                                                                                                                                                                                                                                                                     |

箭头的*含义*取决于列表是否打开：


| 列表是 | ↑ / ↓                                                                                             | Enter                        | Escape                                 |
| ----------- | --------------------------------------------------------------------------------------------------- | ---------------------------- | -------------------------------------- |
| **关闭的**    | 直接更改值，像点击那样报告 `onChange` 和 `onSelectedIndexChanged` | —                           | —                                     |
| **打开的**    | 移动高亮，不提交任何东西                                                              | 提交被高亮的条目 | 关闭，让值保持原样 |

> **注意。** `Editable` 对箭头没有影响。它们属于列表，即使在可以键入的组合框中也是如此；插入符用 ← 和 → 移动。

列表会**滚动以让被高亮的条目保持在视野中**，把它落在第一个或最后一个可见行上，而打开列表时会直接滚动到它已经持有的值——所以有两百个国家的组合框打开时显示的是你选的那个，而不是字母 A。滚轮和滚动条仍然各自滚动列表；拖动是选择，不是轻扫。

**键入搜索。** 在仅可选取的组合框（`DropDownList`，或 `Editable` 关闭）和 ListBox 中，键入一个字母或数字，列表就会跳到第一个以它开头的条目——忽略大小写——并把该条目滚动到**顶部**。关闭的组合框会打开以显示它；Enter 选中它，而在 ListBox 中它会被立即选中，就像箭头会选中它一样。在 **700 ms** 之内继续键入，搜索就会增长（`S` 找到 *São Paulo*，`SE` 接着找到 *Sergipe*）；停顿更久，则下一个按键以它自己开始一次新的搜索。箭头从找到的条目继续：↓ 到它之后的条目，↑ 到它之前的条目。可以键入的组合框则改用它自己的文本进行搜索，随你键入而搜索。

**索引从 1 开始计数。** 控件交给你的 COBOL 的每一个位置，都按 COBOL 表的方式计数：条目 1 是第一个。这包括 `SelectedIndex`（ListBox 或 ComboBox）、`SelectedTab` 和控件的 `Tab` 页（TabControl）、Snackbar 的 `LastButtonIndex`、Viewer 的 `SearchCurrentMatch`、DataGrid 的行和列、
`Grid-1::Rows(1)` 和 `List-1::Items(1)` 这样的链式下标，以及 TreeView 的节点句柄和深度（`AddNode(1, …)` 是一个根节点）。**`0` 表示没有**——没有选中的、没有匹配、没有这样的节点。

```cobol
       MOVE CBO-UF::GetSelectedIndex() TO WS-IX
       IF WS-IX > 0
           MOVE WS-UF-TABLE(WS-IX) TO EMP-UF      *> the same number indexes your table
       END-IF
       MOVE CBO-UF::GetCount() TO WS-N
       MOVE WS-N TO CBO-UF::SelectedIndex          *> the last item
       Lst-Tasks::RemoveAt(Lst-Tasks::GetIndex())  *> remove the selected item
       Lst-Tasks::RemoveItem("Done")              *> remove an item by its text
```

> ⚠️ **从早期版本过来。** 在 1.80.148 之前，这些从 0 开始计数，用 `-1` 表示没有——除了 TreeView 的节点句柄，它们自 1.70.156 起就从 1 开始计数；它的深度，以及表示没有节点的 `0`，在 1.80.148 改变。在那之前保存的窗体会在第一次打开时被转换——设计器存储的值（`SelectedIndex`、`SelectedTab`、
> 每个控件的 `Tab` 页）加一，并且下一次保存会把文件标记为已转换——所以你的窗体看起来和打开时都与以前一样。**你的 COBOL 从不被更改**：与 `-1` 比较、把 `0` 移入以选中第一个条目、或加 1 以到达表条目的处理程序需要手工调整，而 Check 无法告诉你在哪里。请在你的处理程序中搜索
> `SelectedIndex`、`SelectedTab`、`GetIndex`、`SetSelectedIndex`、`LastButtonIndex`、`RemoveItem`（它现在接受条目的**文本**；按位置请用 `RemoveAt`）、`Rows(` 和 `Items(`。

**给条目排序。** 勾选 **Sorted**，列表就按字母顺序显示它的条目。有三点值得了解：

- 它**按文本排序，忽略大小写**——这是每个 RAD 所说的“排序”的含义，也是列表条目的本质。因此数字按它们作为字符串的方式排序：`1`、`10`、`11`、`2`、…… `9`。要得到数值顺序，请填充到固定宽度——`01`、`02`、…… `11`——它们就会按你预期的方式排序。
- 它只改变**显示**的内容。你键入的 `Items` 原样保留，所以清除该框会直接还给你自己的顺序。
- `SelectedIndex` 是条目**按显示顺序**的索引，所以它与操作员所选的相符。`Value` 是条目的文本，两种情况下都一样。设置 `SelectedIndex`——在设计器中，或从 COBOL 用 `MOVE 2 TO CBO-CITY::SelectedIndex` 或 `SetSelectedIndex`——会选中那个条目并让
  `Value` 随之移动——`1` 是第一个条目——而 `0` 清除选择。反过来也成立：`MOVE EMP-UF TO CBO-UF::Value` 选中具有该文本的条目，并把 `SelectedIndex` 移到它，若没有条目具有该文本则移到 `0`（字段中的尾随空格不计）。ListBox 也是如此。

> **TreeView** 也带有 `Sorted`，并且自 1.61.153 起它会据此行动——对**兄弟节点**排序，让每个子节点留在你所写的父节点之下。（平铺的排序会让节点有序而树毁掉。）事件所指名的节点仍然是你所写的那一行，不管排序对它做了什么。

**列表有多高。** 与它的条目所需一样高——加上它与自己的边框保持的小边距——最高到 `DropDownHeight`（检查器中的 **DropDownHeight** 行，默认 200 px），超过则滚动。不论有多少条目，每个都可以触及，而短得足以容纳的列表不会滚动。滚动条紧贴在边框的内侧，就像 ListBox 的那样。

**外观归你。** ComboBox 穿着你设计的背景——**Background color**，或带起点、终点和方向的 **Background gradient**——连同它的边框和圆角半径，在关闭的表头*和*打开的列表上，在每个界面上：设计画布、预览、Run Form 和编译出的二进制。

> ⚠️ **你从未设计过的组合框现在是方角的。** 表头过去不论 `CornerRadius` 怎么说，都以固定的 6 px 圆化，而设计画布把它画成方的。表头现在跟随该属性——它的预置值是 **0**——所以画布和运行中的窗体一致。把 **Corner radius** 设为 6，就能拿回旧的圆角，这一次是在所有四个界面上。

**字体也一样。** 条目用控件自己的 `FontName`、`FontSize` 和 `ForegroundColor` 书写，每一个都是一行该文字加上空隙——而这一切过去都是硬编码的，所以 20 pt 的组合框会在一个 12 pt 条目的列表上画出 20 pt 的值。

高亮本身被面板自己的圆角所切割，并在每一边都停在边框之前，在二者之间留下一条面板的细线——恰如 ListBox 的行那样，通过同一段代码，所以两者不会漂移开。

**高亮也一样。** 展开的下拉列表高亮两样东西，两者都归你：


| 属性          | 检查器行     | 它背后的高亮                                     |
| ----------------- | ----------------- | -------------------------------------------------------- |
| `ActiveItemColor` | **Selected item** | `Value` / `SelectedIndex` 所报告的条目。               |
| `HoverItemColor`  | **Hovered item**  | 指针、拖动或方向键所在的条目。 |

`ActiveItemColor` 刻意是 **ListBox 所携带的同一个属性**：在两个控件上，它都给 `Value` / `SelectedIndex` 所报告的条目着色，所以你在一个上学到的，在另一个上已经会了。

与列表有两点不同值得了解：

- **没有 `SelectedItemsColor`。** ComboBox 选中一个条目或不选，所以列表的第二种*选择*在这里没有什么可着色的。ComboBox 所拥有的是*悬停*，那是另一回事，并有它自己的属性。
- **两者是独立的。** 在 ListBox 上，变暗的颜色跟随活动颜色；而在这里，设置 **Selected item** 会让 **Hovered item** 恰好留在原处。重新设置样式时请两者都设，否则指针仍会在你的新颜色上闪出旧的蓝色。

**高亮上的文字是按高亮看起来的样子来判断的。** 默认的选择是半透明的中蓝色；在浅色的下拉列表之上，它是淡天蓝色，而白色文字在它上面读不出来。自 1.90.9 起，行的墨色是针对叠在面板上的高亮来选择的——你的 `ForegroundColor` 在那里读得出来时（WCAG AA，4.5:1）就用它，否则用黑色或白色——而不是针对高亮自己的颜色。要做高对比度的列表，请指名一个深色的 `ActiveItemColor` 和一个浅色的 `HoverItemColor`。

留空时，每个都回落到下拉列表一直以来绘制的高亮——而不是回落到主题，那是 ListBox 的回落方式。这两个从来不是从调色板绘制的，所以*空*表示*它以前画的样子*，你早先设计的 ComboBox 不受触动。悬停的默认值刻意是两者中较淡的，所以悬停在某个条目上永远不会看起来像选中它；如果你设置自己的，请保持这个差别，否则下拉列表会变得难以阅读。

两者都接受运行时写入，与列表的相同。

#### ToolBar

**ToolBar** 是**成组的按钮**。每一组是一个有自己的边框和圆角半径的框架；一个不可见的分隔符把一组与下一组分开；而组内的每个元素都是一个你完全掌控的按钮。

> **从 PowerCOBOL 或 isCOBOL 来的？** 它们的工具栏是一条扁平的命令按钮带。这个更接近功能区的分组：分组是模型的一部分，而不是你用间距伪造出来的东西。

**一切都在 Toolbar Editor 中设置。** 属性窗格只提供一个按钮——**Edit Toolbar…**——因为工具栏的旋钮远多于窗格所能容纳的，而且它是你一边看着一边布置的东西。编辑器在左边显示组及其按钮的树，在右边显示所选内容的属性，并在顶部显示该条的实时预览，由运行中窗体所用的同一个渲染器绘制。在你按 **Save** 之前，什么都不会写入控件，所以 Cancel 真的是取消。

**组**有：边框样式（`Single` / `None` / `Fixed3D`）、边框颜色和宽度、圆角半径、它自己在框架与按钮之间的内边距、背景，以及带宽度的 *Separator after this group*。`None` 仍然成组——内边距和分隔符仍然适用——它只是不画框架。

**按钮**有：标签**或**图标、工具提示、启用标志、**动作**，以及外观——图标大小和颜色、宽度和高度、圆角半径、背景（纯色，或带起止颜色和方向的渐变）、前景颜色和投影（颜色、不透明度、距离、模糊）。

**标签和图标是互斥的。** 工具栏按钮只显示一样东西，所以设置标签会清除图标，选择图标会清除标签。想要图标时，把文字放在工具提示里。

**圆角半径默认为 10**，组和按钮都是。

##### 三个外观层级

按钮自己的值胜出。按钮什么都没说的地方，由它的**组**决定。组也什么都没说的地方，由**窗体的主题**决定。

这就是组的价值所在：在组上设置一次图标大小、背景或阴影，其中的每个按钮都会跟随——而某个按钮仍然可以逐个字段地不同意。在编辑器中，被继承的行标记为 `group`（对组则是 `theme`），而你设置的值旁边的 ✕ 把它放回继承。

> **主题回落读取你窗体自己的背景。** 一路继承到主题的按钮或组，得到的是针对它实际所在的窗体来选择对比度的表面和墨色——而不是为某一种窗体调好的固定外观。保持默认的工具栏，不论它后面的窗体是深色还是浅色，都保持可读。

**添加按钮会复制前一个按钮的外观**——它的大小、颜色、渐变和阴影，但绝不复制它的图标、工具提示或动作。构建工具栏通常是六个只有图标和动作不同的按钮，所以你只需设置一次外观。

##### 工具栏自身的框架

与各个组分开，ToolBar 控件本身在属性窗格中有 `BorderStyle`、`BorderColor`、`BorderWidth`、`CornerRadius`、`Transparency` 和 `BackgroundColor`。

新工具栏**以 10 圆化，没有边框，并且 100 % 透明**——所以它读起来是按钮坐在你的窗体上，而不是叠在上面的一块面板。当你想让这条带本身可见时，请打开边框。

> **给工具栏一个 `BackgroundColor` 会打开它的框架。** 你不必再去找 `Transparency` 把它降低：那个 100 是每个工具栏出厂时就有的，不是你选的，所以选择一个颜色就被当作决定。你*确实*移动的 `Transparency` 仍然像在任何其他控件上一样淡化表面，而你从未触碰颜色的工具栏保持不可见。
>
> 你指名的颜色就是被绘制的颜色——当前主题没有资格用它自己的卡片填充来替换它。（在 1.61.150 之前，选定的背景完全不起作用：预置的透明度完全跳过了表面，而在经过那里时，主题用它自己的填充作答，从未到达你的。）

新工具栏还会带着**一个组、一个 folder-open 按钮**到来，所以你刚放下的 ToolBar 显示的是工具栏是什么，而不是一条空带。删除它、重命名它，或围绕它构建。

##### 按钮做什么


| 动作                   | 效果                                                                                                                                                |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `event`                  | 引发工具栏的 `onClick`，携带按钮的 id。默认值。                                                                                  |
| `procedure`              | 按名称运行窗体的某个过程。                                                                                                           |
| `open-modal`             | 把一个**独立**窗体作为模态窗口打开——按下会等待，直到该窗口关闭。仅限独立窗体：嵌入的窗体属于 ContentPane。 |
| `print`                  | 在平台的查看器中打开所指名的文档，打印对话框就在那里。                                                                         |
| `share`                  | 捕获这个窗体的窗口，并把图像交给操作系统去分享。                                                                                                |
| `screenshot`             | 把这个窗体的窗口的图像放到剪贴板上。                                                                                                 |
| `copy` / `cut` / `paste` | 操作系统剪贴板，作用于你所在的字段——见下文。                                                                                       |
| `run-app`                | 启动另一个应用程序。                                                                                                                         |
| `open-terminal`          | 打开一个终端，可选地在给定的文件夹中。                                                                                                       |

每一次平台按下都会报告它的结果——它做了什么，或为什么不能——作为运行中窗体窗口底部的简短通知，所以按下从来不会显得什么都没做。SideMenu 页脚面板中的工具栏与其他工具栏一样执行平台动作。

##### 剪贴板按钮

`copy`、`cut` 和 `paste` 作用于按下按钮时**拥有**键盘焦点的文本字段——按下工具栏按钮是在别处的一次点击，它会带走字段的焦点，所以起作用的是你所在的那个字段。每个动作随后都把焦点**还回去**，插入符在编辑结束的地方，所以键入从中断处继续。


| 动词    | 选中了文本时                                                         | 没有选中任何东西时                                                  |
| ------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `copy`  | **只复制选中的部分**；插入符紧跟在最后复制的字符之后。 | 复制整个字段；插入符在它的末尾。                              |
| `cut`   | 复制并移除选中的部分；插入符在被移除文本开始的地方。      | 取走整个字段并把它清空。                                              |
| `paste` | **替换选中的部分**；插入符紧跟在最后粘贴的字符之后。   | **在插入符处插入**；插入符紧跟在最后粘贴的字符之后。 |

完全没有字段获得焦点时，`paste` 什么也不改变，并如实说明。你没有键入过的字段给出的是你设计它时所用的文本。这些规则按**字符**而不是字节计数，所以带重音的文本和 CJK 文本永远不会在字符的中间被切开。

不论动作还做什么，窗体**总是**把按下作为工具栏上的 `onClick` 听到——所以一个处理程序可以通过读取是哪个按钮来服务整个工具栏（按钮也可以带有它**自己的**处理程序；见下文）：

```cobol
      *>   in the TOOLBAR-1 onClick handler:
           EVALUATE TOOLBAR-1::LastButton
               WHEN "bnsq"  PERFORM SAVE-RECORD
               WHEN "dlrx"  PERFORM DELETE-RECORD
               WHEN OTHER   CONTINUE
           END-EVALUATE
```

每个组和按钮都有一个固定的、由四个小写字母组成的 id，在你添加它时给定，并在 Toolbar Editor 中与它的名称一起显示，带一个 **Copy** 按钮——与菜单项相同的 id，规则也相同（见 MenuBar 之下的*哪个菜单项被选中了*）。

##### 给按钮它自己的处理程序

按钮可以携带它**自己的 COBOL**，而不是由工具栏上的一个 `onClick` 去判断是哪个按钮被按下。在 Toolbar Editor 中，选中一个按钮并看 **Events** 之下：带圆点的 `onClick`——没有代码时是空心的，有代码时是实心的——以及 **Edit code**。

点击它会像 **Save** 那样保留工具栏，并交给 COBOL 编辑器，所以你永远不会同时看着两个带两个 Save 的模态窗口。写好处理程序，保存它，它就会回到工具栏中。

`onClick` 是按钮提供的唯一事件，因为它是平台能为按钮引发的唯一事件：工具栏知道哪个按钮被按下，而对它别无所知。你可以绑定却从不触发的事件，比没有事件更糟。

两条路径同时起作用，并且顺序固定：

1. 工具栏的 `onClick`（`LastButton` 指名该按钮），
2. 按钮自己的 `onClick`，
3. 最后是按钮的**动作**，如果它有的话。

所以，处理程序为过程或窗体准备好所需之物的 `procedure` 或 `open-modal` 按钮，其工作方式正如你会写的那样——处理程序先运行。

##### 窗体运行时更改按钮

按钮允许你的 COBOL 更改它的**颜色**、它的**工具提示**，以及它是否**启用**：

```cobol
           MOVE "#204080FF" TO TOOLBAR-1-FMTG-BNSQ::BackgroundColor.
           MOVE "Record saved" TO TOOLBAR-1-FMTG-BNSQ::Tooltip.
           INVOKE TOOLBAR-1-FMTG-BNSQ::Disable().
```

被禁用的按钮变暗并忽略点击——工具栏的 `onClick` 和按钮自己的都不触发——直到 `Enable()` 把它重新打开。`MOVE 0 TO
TOOLBAR-1-FMTG-BNSQ::Enabled` 和 `MOVE 1 TO …` 做同样的事，所以 Save 按钮可以一直灰着，直到记录发生了变化：

```cobol
           IF WS-RECORD-CHANGED = "Y"
               INVOKE TOOLBAR-1-FMTG-BNSQ::Enable()
           ELSE
               INVOKE TOOLBAR-1-FMTG-BNSQ::Disable()
           END-IF
```


| 可写                                          |                                           |
| ------------------------------------------------- | ----------------------------------------- |
| `Enabled`                                         | 开（`1`）或关（`0`）；也可用 `Enable()`/`Disable()`。 |
| `Tooltip`                                         | 悬停文字。                           |
| `BackgroundColor`, `ForegroundColor`, `IconColor` | 按钮的表面、它的文字和它的图标。 |
| `GradientStartColor`, `GradientEndColor`          | 它的渐变，如果有的话。            |
| `ShadowColor`                                     | 它的投影。                           |

把颜色设为**空格**会把它放回继承——先从它的组，再从窗体的主题——恰如编辑器中它旁边的 ✕ 所做的那样。

**其他一切都被拒绝，而且被明说地拒绝。** 对按钮的宽度、高度、圆角半径、标签、图标或动作的写入是**运行时错误**，它会指名该属性，以及允许的替代做法：

```cobol
      *>   this stops the form with an error, on purpose:
           MOVE "200" TO TOOLBAR-1-FMTG-BNSQ::Width.
```

这是刻意的。工具栏拥有布局——它让按钮保持你构建时的排列，而一个能自己移动的按钮会让没有任何东西把它放回去。悄悄什么都不做的写入，是一个下午消失不见的原因，所以窗体明说了。COBOL 编辑器也知道：被拒绝的属性在你键入时就被标记出来，早在你运行窗体之前。

##### 按钮如何到达你的代码

工具栏按钮**不是**控件。工具栏拥有布局——这正是它让按钮排成一线、并避开设计器的拖动把手的原因——所以按钮在窗体的各个控件之中没有它自己的条目。

它仍然需要一个名称，因为有两样东西必须就一个名称达成一致：按下，以及分派它的生成的事件循环。这个名称是派生的，它是大写的 `<toolbar>-<group>-<button>`：

```text
   ToolBar  TOOLBAR-1
     group  fmtg
    button  dlrx          ⇒   TOOLBAR-1-FMTG-DLRX
```

你不需要在任何地方键入它——`procedure` 和 `open-modal` 会替你通过它接线——但它就是你将在生成的代码中看到的东西，是按下到达时所用的 id，也是你的 COBOL 寻址该按钮的方式：

> **按钮属于它们自己的窗体。** ToolBar 在**独立**窗体和加载到 ContentPane 中的**嵌入**窗体中工作方式相同，并且在两种情况下，它的按钮都存在于**那个窗体的**程序中——持有该工具栏的那个。请从那个窗体的 COBOL 中读取它们、给它们重新着色并处理它们，恰如你对待一个控件那样。带有同名工具栏的两个窗体永远看不到彼此的按钮。

```cobol
      *>   generated, in COBOL-EVENT-LOOP:
           EVALUATE COBOL-CONTROL-ID
               WHEN "TOOLBAR-1-FMTG-BNSQ"
                   EVALUATE COBOL-EVENT-ID
                       WHEN "onClick"
                           CALL "UPDATE-TOTAL"
                   END-EVALUATE
               WHEN "TOOLBAR-1-FMTG-DLRX"
                   EVALUATE COBOL-EVENT-ID
                       WHEN "onClick"
                           INVOKE ME::"OpenFormSync"("CUST-LOOKUP")
                   END-EVALUATE
           END-EVALUATE
```

> ⚠️ **注意事项。** `COBOL-CONTROL-ID` 容纳 **64 个字符**，所以这三个名称加在一起必须放得进 64。派生的 id 更长的按钮无法被分派；与其生成一个永远无法触发的分支，PowerRustCOBOL 会在生成的源码中写入一条注释，告诉你是哪个按钮以及要缩短什么。完全没有指名任何东西的 `procedure` 或 `open-modal` 按钮也是如此。

> **注意。** `run-app` 和 `open-terminal` 启动一个进程。目标按空白拆分，并**直接**交给操作系统——**绝不交给 shell**，所以用数据项构建的路径不会变成 shell 命令。但这仍然是你的窗体在启动一个真正的程序：请把目标当作代码，而不是数据。

##### 在 Preview 中试用工具栏

你不必运行窗体就能按一个按钮。**Preview 自己执行八个平台动作中的六个**——`print`、`run-app`、`open-terminal`、`copy`、`cut` 和 `paste`——并把发生了什么，或为什么不能，写入 **Output** 窗格。那是工具栏被构建的地方，所以那里是它的按钮必须工作的地方。

另外五个动作——两个窗口捕获和三个运行你的 COBOL 的——在 Preview 中**不**运行，并且每个都在 Output 面板中如此说明，而不是让你去猜：

| 动作                             | 为什么不                                                                                                                                                                                                                           |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `screenshot`, `share`              | 它们捕获窗体**自己的窗口**。在 Preview 中，窗体是 IDE 之内的一个窗格，所以捕获会递给你一张 IDE 的图片。Preview 如实说明，而不是悄悄地返回错误的图像——请使用 **Run Form**。 |
| `event`, `procedure`, `open-modal` | 这些是你窗体的 COBOL。Preview 绘制窗体但不运行解释器，所以它指名该动作并把它留给 **Run Form**。                                                                                                 |

> ⚠️ **注意事项。** 比它所在控件更宽的工具栏，会从右端丢掉整组，而不是画出半组。属性窗格显示它所需的宽度，并在控件太窄时发出警告。

> **现有的工具栏继续工作。** 在组存在之前构建的 ToolBar——带有普通 `Items` 列表的——会被读作按顺序排列的、带标签按钮的单个**无框**组。它看起来与以前完全一样；打开编辑器才会把它提升为真正的工具栏。

📷 需要截图 — `toolbar-editor.png`
: 打开一个带有 ToolBar 的窗体，按 **Edit Toolbar…**，并构建两个组——一个有三个图标按钮，一个只有一个按钮——二者之间有一个分隔符。捕获整个模态窗口，使树、属性窗格和实时预览条都可见。
#### FileDropZone

**FileDropZone** 在精神上是非可视的，但会被可见地渲染出来：它是一个放置目标，用户把文件拖到它上面，或点击它以打开平台原生的文件选择器。不论哪种方式，该区域都会应用它的接收规则，它接受的文件落入
`DroppedFiles`——每行一个绝对路径——并且 `onFilesDropped` 触发。

**没有 COBOL 方法**可以打开选择器，或以编程方式读取放置——把文件弄进来纯粹是界面手势。事件触发之后，以通常的方式读取结果：

```cobol
      *>   in the FDZ-1 onFilesDropped handler:
           MOVE FDZ-1::DroppedFiles TO WS-PATHS
      *>   WS-PATHS is newline-separated; UNSTRING or SEARCH it as usual.
```

该区域恰好有一个方法 `CommitFiles()`，它属于下面进一步描述的先确认后复制的流程。

**该区域接受什么，以及把它放到哪里。** 五个设计时属性决定这一点，而且两条进入的途径——放置和文件选择器——都遵守它们，所以一个文件不论怎样到达，都按同样的方式被判断：

| 属性            | 含义                                                                                                                                                   |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AllowedExtensions` | `csv, xlsx`——该区域接受什么。不区分大小写，点号可省略，以逗号、分号或空格分隔。留空则接受任何文件。                         |
| `MaximumFileSizeKB` | 该区域接受的最大文件，以 KB 计。`0` 表示没有限制。                                                                                                |
| `DestinationFolder` | 一个本地文件夹，被接受的文件会被**复制**进去。留空则让文件留在原处。                                                                 |
| `StageOnly`         | 关闭（默认）：放置立即复制。开启：放置只是*保留*这些文件供操作员复查，而你的 COBOL 调用 `CommitFiles()` 来复制它们。 |
| `FileListControl`   | 复查暂存接收内容的那个 ListBox 的 id。预置为设计器在新区域旁边创建的同伴；留空表示没有列表。               |

设计器中的 **Destination** 行带有一个打开系统文件夹选择器的 **📂** 按钮，以及一个再次清除所选内容的 **✕**。选择器把文件夹作为**绝对**路径写回。你也可以手工键入相对路径——或在运行时从你的 COBOL 设置一个——它从**应用程序的文件夹**开始，与 `KnowledgeBase` 控件的相对 `Location` 开始的位置相同，绝不从程序被启动时所在的目录开始。所以指向 `assets/KB/<collection>/documents` 的区域，把它的文件恰好放在该集合的知识库寻找它们的地方。清除该行会让属性**留空**而不是移除它——而留空就是“让文件留在原处”的意思。

设置了目的地时，如果文件夹不存在就会被创建，而已有的文件**绝不会**被覆盖：第二个 `report.csv` 落成 `report (2).csv`，第三个落成 `report (3).csv`。然后 `DroppedFiles` 按每个文件的新路径报告它——你的程序拥有的那份副本，而不是用户拖动的原件。

区域拒收的文件不会无声地丢失。它们落入 `RejectedFiles`，每行一个：路径、一个 TAB 和原因——`extension` 或 `too-big`——并且 `onFilesRejected` 触发。放置十个文件而其中三个被拒绝，会**同时**触发两个事件，所以窗体可以接受那七个，并且仍然说明其余的怎么了：

```cobol
      *>   in the FDZ-1 onFilesRejected handler:
           MOVE FDZ-1::RejectedFiles TO WS-REFUSED
           UNSTRING WS-REFUSED DELIMITED BY X"09"
               INTO WS-PATH WS-REASON
           STRING "Not accepted: " WS-PATH " (" WS-REASON ")"
               DELIMITED BY SIZE INTO WS-MESSAGE
           MOVE WS-MESSAGE TO LABEL-STATUS::Caption
```

> **注意。** 平台无法度量的文件（无法读取的路径、不肯报告大小的文件系统）会被**接受**而不是被拒绝——区域不能吞掉一个它仅仅是没能 stat 的文件。

> ⚠️ **注意事项。** 复制在窗体运行的任何地方都会发生，包括 IDE 的 **Preview** 之中——这正是让预览忠实的原因。设计时请把 `DestinationFolder` 指向一个临时文件夹。

##### 在复制任何东西之前让操作员确认

默认情况下，文件一落下就复制，这没有给操作员留下改变主意的余地——拖错的文件已经在文件夹里了。勾选 **Confirm before copying**（`StageOnly`），放置就*什么都不复制*：

1. 放置的判断与上述完全一样——被拒绝的文件仍然触发 `onFilesRejected`——而被接受的文件以它们原来的路径被**保留**在 `StagedFiles` 中。`onFilesDropped` 触发，`DroppedFiles` 保存*这一次*放置带来的内容，因为没有复制任何东西，所以是原来的路径，而 `StagedFiles` 是迄今为止保留的一切。`DestinationFolder` 甚至不会被创建。

   > **注意**——当你的处理程序想要刚刚放下的那个文件时，例如把一份文档打开到 Viewer 中，请读取 `DroppedFiles`；当你想要整个篮子时，请读取 `StagedFiles`。它们在第一次放置时相同，在之后的每一次放置时不同。
2. 它们出现在由 `FileListControl` 指名的 ListBox 中，每个一个带勾选框的行，显示路径和大小：`/Users/ana/report.csv (12.345 MB)`。`CommitSummary` 读作 `3 files staged, 24.310 MB`。
3. 操作员取消勾选任何他们并不打算发送的东西。取消勾选的行**留在**列表中，标记为 `(excluded)`，所以排除是可见的，并且他们可以把它放回去。
4. 你的窗体决定确认意味着什么——一个 Submit 按钮、一个经过验证的字段、主管的密码——并调用 `CommitFiles()`。被勾选的文件按上述规则被复制；取消勾选的则被跳过。
5. 每一行变成 `✓ <new path> (12.345 MB)` 或 `✗ <path> (12.345 MB) — <reason>`。`CommitSummary` 变成 `7 of 8 copied, 24.310 MB`，这也是该方法的返回值，而区域会把它沿着自己的底边画出来。`DroppedFiles` 变成被包含的文件，以它们的新路径。

```cobol
      *>   in the SUBMIT-BUTTON onClick handler:
           MOVE FDZ-1::CommitFiles() TO WS-SUMMARY
           MOVE WS-SUMMARY TO LABEL-STATUS::Caption
      *>   Now the files are in the folder — hand them to the application.
           MOVE FDZ-1::DroppedFiles TO WS-PATHS
           PERFORM SEND-TO-APPLICATION
```

**复查列表是普通的 ListBox。** 在设计器中放下 FileDropZone 会在它正下方创建一个，大小与区域自己相同，勾选框已打开，并在区域的 `FileListControl` 中指名它。从那一刻起，它就是和其他任何一个一样的 ListBox：移动它、调整它的大小、重新给它设置样式、把它放到另一个标签页上——或者删除它，而区域只是不带列表地继续工作。`FileListControl` 指名一个不再存在的控件，与什么都不指名意思相同。

第二次放置会**添加**到已经暂存的内容中，而不是替换它，并且同一个文件放下两次只保留一次。在什么都没保留的区域上调用 `CommitFiles()` 不是错误：它报告 `0 of 0 copied, 0.000 MB`。

> **注意。** 大小把一兆字节算作 1,000,000 字节，与操作员自己的文件浏览器一样，所以列表中的数字与他们在 Finder 或资源管理器中看到的数字相符。

> ⚠️ **注意事项。** 在提交时复制失败的文件——不可写的文件夹、已满的磁盘、此后已被移动的源——会以 `✗` 加原因报告，而它在 `DroppedFiles` 中的条目是**原始**路径。你的程序仍然收到交给它的那个文件；在把一批文件视为完成之前，请检查 `CommitSummary`（或数行数）。

> ⚠️ **注意事项。** `CommitFiles()` 复制被勾选的一切，在你调用它的任何时候。它与窗体关闭、与任何内置的“提交”概念都没有关联——PowerRustCOBOL 没有这样的东西。如果两个按钮都可以提交，两者都必须调用它，而调用两次会把被勾选的文件复制两次（落成 `report (2).csv`）。

`FileDropZone` 刻意**不是** Data Binding Guardian 的目标——它的输出是事件形态的（由用户操作填充），而不是由绑定的数据源驱动的值。

#### 用户控件

**User Control** 是存储在项目中的、基于 GroupBox 的可复用组件。设计一个带有子控件的 GroupBox，选中该 GroupBox，然后右键点击并选择 **Create User Control**。给它一个由字母、数字和连字符构成的名称；它必须以字母开头。设计器会拒绝重复的名称和循环的定义，包括间接的嵌套。

User Controls 出现在工具箱的 **User Controls** 之下。把一个拖到窗体上，或点击它以把它放在画布中心附近。部署会创建一个真正的 GroupBox 实例，加上真正的子控件。ID 由实例 ID 限定，例如 `CustomerCard-1-Button1`，所以每个部署的实例都是独立的，并且仍然使用普通控件的渲染、选择、属性和事件分派。

要自定义某个已部署的实例，请选中 User Control 的根。它的属性包括一个可折叠的 **Child Controls** 部分，它把可编辑的子属性分组为 `ChildId.PropertyName = value`。这些编辑只影响那个已部署的实例；项目级的 User Control 定义仍然是将来部署的模板。

COBOL 可以通过 User Control 的根触及子属性：

```cobol
INVOKE CustomerCard-1 "SetProperty"
    USING "Button1.Caption" "Save"
INVOKE CustomerCard-1 "GetProperty"
    USING "Button1.Caption"
    RETURNING WS-CAPTION.
```

运行时，`Button1.Caption` 解析为已部署的子控件 `CustomerCard-1-Button1` 及其 `Caption` 属性。如果没有匹配的子项存在，带点的名称就被当作根上的普通属性，从而保留直接使用带点属性名的旧窗体。

子事件使用已部署的限定子 ID。`CustomerCard-1` 内名为 `Button1` 的子按钮在 `WHEN "CustomerCard-1-Button1"` 之下分派，而它的处理程序名称派生自那个完整的 ID，例如 `CUSTOMERCARD-1-BUTTON1--ONCLICK`。

User Controls 可以包含其他 User Controls。部署时，嵌套的控件被递归展开，并在外层实例之下获得限定的 ID。要从项目中移除某个定义，请在设计器中右键点击并选择 **Remove User Control**；现有的窗体实例仍然作为普通控件保留。

> 📷 **需要截图——`control-gallery.png`。** 一个窗体（或预览窗口），显示每个主要控件各一个，让新来的人能够认出它们。图表尤其能从图像中受益。

### 各控件示例

仓库附带**一个**演示每个控件的应用程序：`examples/PowerDemo3`，`forms/` 之下有 **61 个窗体**。打开项目的 `sidebar-form` 位于根部；十二个响应式演示有它们自己的文件夹 `Responsive Layout/`；另外 48 个按工具箱所用的同样类别归档：
- `General/`（7）
- `Common/`（16）
- `Non-Visual/`（7）
- `Graphics/`（6）
- `Containers/`（4）
- `Menus & Bars/`（4）
- `Data/`（2）
- `Charts/` 和 `Rust/` 各一个

窗体以它的控件命名，所以你正在阅读的那个控件的演示就在你期望的地方：`forms/Common/knob-form.cfrm`、`forms/Containers/splitter-form.cfrm`、`forms/Non-Visual/websearch-form.cfrm`。

每一个都放置该控件，接好它所支持的事件，并给你每个属性一个从 COBOL 更改它的按钮——所以它同时也是从代码接线事件和设置属性的参考。处理程序是用**扩展方言**而不是长形式写的（对控件的内联调用、对属性的直接写入、`::` 链式、块字面量），并且其中 30 个窗体在每一行使用了扩展的代码上方都带有一条注释，用全部六种界面语言：英语、葡萄牙语、西班牙语、法语、日语和中文各有 461 条这样的注释。

用 **File ▸ Open Project** 打开该项目并运行它——它从一个能到达每个演示的侧边栏窗体开始。任何单个窗体也可以在设计器中独自运行，这是试用某个控件的快捷方式。从命令行：

```sh
rcrun build examples/PowerDemo3/PowerDemo3.project.toml
```

它是一个真正的项目而不是画廊：它带有自己的 `src/`、`COPYBOOKS/`，一个索引定义（`indexed/actors.cidx`），其数据在 `data/` 之下，资产，一个知识库，以及一个内置的 Project's Crates 库，`forms/Rust/ferris-says-form.cfrm` 从 `EXEC RUST` 块中调用它。

Non-Visual 演示中有三个会触及机器之外——`agent-form`、`restapi-form` 和 `websearch-form`。它们离线也能打开和构建，但需要它们的服务可达，或配置了它们的凭据，才能做任何事。`sqldatabase-form` 不在其中：它连接到 `sqlite::memory:`，而 SQLite 是捆绑的，所以什么都不用安装就能运行。

> **没有工作的东西也被写了下来。** `forms/DEMOS-TO-FIX.md` 是在构建演示期间保存的目录——每个条目都对照源码验证过而不是想当然，其中一些在已交付的演示中是实际存在的。在断定某个演示向你展示的是某控件的真实行为之前，请先读它。

### 默认主题设置（主题在*你的*项目中意味着什么）

主题决定窗体上的每个控件看起来如何。**Project settings ▸ Default Theme Settings** 是你说明那种外观是什么的地方。

PowerCOBOL 没有任何与此相当的东西：在那里，控件的外观是你逐个在每个控件上设置的属性，而“主题”是你手工维持的约定。在这里，主题是一张*表*，这张表属于项目，而把窗体切换到某个主题就是给它盖章。

```
[Theme]  [Glass style]        [form ▼] [📥 Import from a form…]
每个控件          <property, value>
按类型的例外     [control type ▼]  <property, value>
```

**基础加例外。** 大多数主题是统一的：一个圆角半径、一种边框样式、一个阴影，处处如此。有些则不然——凸起的 Button、扁平的 Label 和*凹陷的* TextBox，是同一种外观之内的三个不同答案。所以这张表有一个每个控件都采用的基础，以及叠在它之上的按控件类型的例外。例外**逐个属性**胜出：说“Label 没有阴影”并不同时说它们没有圆角半径。

**通过导入来编写。** 你不必把主题键入进去。把一个窗体设计到你想要主题看起来的样子——这正是设计器的用途——然后在窗体列表中选中它并按 **Import from a form**。最多控件*类型*一致认同的值成为基础，而每个不一致的类型成为一个例外。是类型投票，而不是控件：一个容纳十一个 Label 和一个 Button 的窗体，不是由 Label 构成的主题。

**它管辖什么，以及它绝不触碰什么。** 只有外观：`BackgroundColor`、`ForegroundColor`、`CornerRadius`、`BorderStyle`、整个 `Shadow*` 家族和背景渐变。标题和 `Text`、`Items` 和 `Value`、几何、Tab 顺序、`Enabled`/`Visible`、事件绑定和数据绑定归**你**，而主题切换从不改写它们。

> **注意。** *你*在某个特定控件上设置的值在主题切换中得以保留。切换只清除主题可能写下的标记，所以需要某个控件与它的主题不同的窗体，只需设置它并保留它。
>
> **注意。** 这张表存放在项目文件的 `theme_defaults` 之下，以主题和玻璃样式为键，是你可以手工阅读和编辑的纯值：
>
> ```toml
> [ide.theme_defaults."elegance/Classic".base]
> CornerRadius = 10
> BorderStyle = "None"
> ShadowEnabled = false
>
> [ide.theme_defaults."elegance/Classic".overrides.Label]
> BackgroundColor = "#00000000"
> ```
>
> ⚠️ **注意事项。** 这张表属于项目，而不属于窗体。共用一个 `.cfrm` 的两个项目不共用它的主题的含义——如果你想要同样的外观，请把 `theme_defaults` 块复制过去。

📷 需要截图 — `default-theme-settings.png`
*打开项目设置，按主题行之下的 Default Theme Settings 按钮，并捕获整个模态窗口，其中勾选了几个基础属性，并在 Exceptions 之下选中了一个控件类型。*

### DateTimePicker（日期*和*时间）

**DateTimePicker** 是一个会弹出选择器的字段。它弹出什么——日历、时钟，或二者——由它的 **`Format`** 属性决定，而同一个属性决定该字段显示什么。


| `Format`   | 弹出层提供                 | 字段显示                               |
| ---------- | -------------------------------- | --------------------------------------------- |
| `Short`    | 月历                 | 日期，`2026-09-03`                        |
| `Long`     | 月历                 | 长日期，`Thursday, 3 September 2026`     |
| `Time`     | 时/分时钟             | 时间，`09:30`                             |
| `Custom`   | `CustomFormat` 所要求的任何东西 | 通过 `CustomFormat` 排版的值     |

在 `Custom` 之下，模式承担两项工作。它的字母决定弹出层：`y`、`M` 或 `d` 要求日历，`H`、`h` 或 `m` 要求时钟，而两者兼有的模式——常见的 `dd/MM/yyyy HH:mm`——两者都得到。并且字段通过它来**显示**值：`yyyy`/`yy`、`MMMM`/`MMM`/`MM`/`M`（月份名称、短名称、数字）、`dddd`/`ddd`/`dd`/`d`（星期名称、短名称、日）、`HH`/`H`（24 小时制）、`hh`/`h`（12 小时制）、`mm`/`m` 和 `tt`（AM/PM）；其他任何东西都按所写打印。空字段把模式本身显示为它的提示。**大小写在这里重要，而在这个控件上的其他任何地方都不重要**：`M` 是月，`m` 是分。月份和星期的名称是英文的。

**`Value` 始终是 ISO**，不论 `Format` 显示什么：


| 选择器编辑 | `Value` 保存      |
| ---------------- | ------------------ |
| 日期           | `YYYY-MM-DD`       |
| 时间           | `HH:MM`            |
| 两者             | `YYYY-MM-DD HH:MM` |

这种分离是刻意的。PowerCOBOL 开发者习惯于显示格式和存储值是同一回事，而这正是让窗体中的日期处理变得脆弱的原因：为了报表更改格式，每个读取该字段的 `MOVE` 都开始看到别的东西。在这里，显示是呈现，`Value` 是数据，所以你的程序可以依赖一种形状：

```cobol
       01  WS-BOOKING.
           05  WS-BOOKING-DATE     PIC X(10).
           05  FILLER              PIC X.
           05  WS-BOOKING-TIME     PIC X(5).

       GET-BOOKING.
           MOVE DateTimePicker-1::Value TO WS-BOOKING
           DISPLAY "Booked for " WS-BOOKING-DATE
                   " at "        WS-BOOKING-TIME.
```

**从 COBOL 设置它**是同样的形状反过来——写入 ISO，字段就按 `Format` 所说的方式显示它：

```cobol
       SET-DEFAULT-SLOT.
           MOVE "2026-09-03 09:30" TO DateTimePicker-1::Value.
```

**时钟。** 两个步进器，小时和分钟。两者都会**回绕**——`23 ▶` 是 `00`，`59 ▶` 是 `00`——并且分钟步进器刻意**不**向小时进位：更改了你没有指向的字段的步进器，就是你不知不觉设错时间的原因。每次按下都会写入 `Value` 并立即触发 `onChange`，而弹出层保持打开，所以你可以一次访问就设置好小时和分钟。在同时编辑两半的选择器上，点击某一天会保留已设置的时间，并让弹出层保持打开以供时钟使用；在只有日期的选择器上，点击某一天会关闭它，一如既往。

> **注意。** 控件无法读成日期或时间的 `Value` 会被原样显示，就像你设置的那样，而不是被清空。它是你的数据，而隐藏它会看起来像控件把它弄丢了。

**限制。** `MinimumDate` 和 `MaximumDate`（`YYYY-MM-DD`，空 = 没有限制）限定操作员可以选取的范围：它们之外的日子在日历中变暗并拒绝点击，而步进或提交的日期永远不会离开它们。你的程序写入的 `Value` 原样保留。

**用步进器代替弹出层。** 打开 `ShowUpDown`，字段在它的右边缘带有 ▲▼，并且不打开弹出层：▲ / ▼、选择器拥有键盘时的 ↑ / ↓ 键，或其上的鼠标滚轮，一次移动一个**日**——在只有时间的选择器上是**分钟**——跨越月和年，在限制之内。每一步都会写入 `Value` 并触发 `onChange`。

> ⚠️ **注意事项。** `MinimumDate` / `MaximumDate` 只限定日期。没有最小或最大*时间*。在 1.70.234 之前，这两个限制、`ShowUpDown` 以及 `Long` 和 `CustomFormat` 的显示一侧都不起作用。

📷 需要截图 — `datetimepicker-clock.png`
*在窗体上放一个 DateTimePicker，把 `Format` 设为 `Custom`，把 `CustomFormat` 设为 `dd/MM/yyyy HH:mm`，运行窗体并点击字段，使弹出层打开。捕获整个弹出层——月份网格以及它下面的时/分条——指针停在小时的 `▶` 箭头上。*

### MenuBar（下拉菜单）

**MenuBar** 控件为你的应用程序提供三层的下拉菜单系统。菜单在 IDE 之内的**树编辑器**中编写，并作为 YAML 文件存储在 `.cfrm` 旁边。

**菜单栏有多宽——`MenuBarStyle`。** `Free`（默认）让菜单栏保持你画出来的宽度。`Responsive` 使它成为真正的窗口菜单栏：它从左边缘开始，在设计器中延伸到窗体的完整宽度，在你于那里调整窗体大小时跟随窗体，并且**在运行时横跨运行中的窗口**——当操作员加宽窗口时，菜单栏随之加宽到右边缘（它永远不会比窗体更窄）。它的 `Y` 和 `Height` 仍归你，并且没有其他控件移动：运行中的窗体保持它设计的布局。

**编辑菜单。** 在设计器中选中 MenuBar 控件，然后在它的属性中点击 "Edit Menu..."。树编辑器让你添加、移除和重新排列最多 3 层深的条目。每个条目有：

- **Label**——菜单中显示的文字。
- **Icon**——来自内置目录的可选图标：**37 个类别中的 1112 个纯矢量图标**——文档、编辑、导航、通信、媒体、商务、薪资、应收账款、付款、库存控制、运输、物流、财务、公司**部门**、交易种类（买入、卖出、退货、拒付，……）、民用**车辆**、**军用**车辆和装备、**设备**（计算机、复古计算机、平板电脑、智能手机、可穿戴设备）、**SaaS** 应用（CRM、ERP、BI、LMS、CMS、ITSM、POS、聊天机器人，……）、**PaaS** 服务（从 aPaaS 到 AIaaS）、**ERP 模块**（FI、CO、SD、MM、PP、QM、PM、SCM）、**选择**工具（选框、全选/全不选/反选、套索、移动）、**设计**工具（油漆桶、填充、调色板、旋转、翻转、适应窗口、缩略图）和**应用**对象（窗口、窗体、应用程序、包、组件、查找和替换、拼写、语音、睡眠、退出、地球、本地）、1.62.132 中增加的三组——**PowerRustCOBOL 控件**、**计算机科学**和**用户界面**，下面马上描述——以及**国旗**（`flag-br`、`flag-jp`、`flag-gb`、……——每个联合国会员国，外加教廷、巴勒斯坦和科索沃）。图标被画成与分辨率无关的线条图——同一个图标在 16 px 的菜单行或 128 px 的图块中都同样清晰——并采用菜单项的颜色。引擎还可以用第二个强调色、投影或新拟物浮雕来渲染任何图标。

  > ⚠️ **注意事项——国旗是线条画。** 目录中的每个图标都是单色的：它从你这里取一种颜色，而国旗主要由它们的颜色来定义。所以国旗带的是它们的**几何**——横带、十字、旗角方块、新月、星星、尼泊尔的三角旗、巴西的菱形——而只在颜色上不同的国旗在这里看起来是一样的。`flag-it` 和 `flag-ie` 都是三条竖带。请在国家已经在它旁边的行里被指名的地方使用它们，而不要把它们作为区分一个国家与另一个国家的唯一方式。

  > **每个控件都有一个图标（1.62.132）。** 构建一个关于控件的演示、调色板或帮助页面，过去意味着没有它们的图片：工具箱自己的图画存在于 IDE 中，从来没有提供给你的应用程序。现在每个控件有一个目录图标，名为 `control-` 后跟控件类型的小写加连字符形式——`control-button`、`control-data-grid`、`control-date-time-picker`、`control-side-menu`、`control-file-drop-zone`。每个控件都有一个，包括供插件提供的控件使用的 `control-custom`。在选择器的 **Find** 框中键入 `control`，即可看到整组。
  >
  > **以及你争论时用的词汇。** 随它们一起落地的还有两组，用于每个真正的应用程序最终都会长出来的图表和管理界面：
  >
  > - **计算机科学（79）**——`array`、`stack-structure`、`queue-structure`、`linked-list`、`hash-table`、`binary-tree`、`graph-nodes`、`compiler`、`parser`、`recursion`、`thread`、`mutex`、`deadlock`、`breakpoint`、`async`、`callback`、`event-loop`、`socket`、`packet`、`firewall`、`load-balancer`、`microservice`、`webhook`、`encryption`、`key-pair`、`two-factor`、`schema`、`primary-key`、`foreign-key`、`join-tables`、`replication`、`sharding`、`query`、`git-branch`、`git-merge`、`pull-request`、`diff`、`ci-cd`、`sorting`、`binary-search`、`state-machine`、`neural-network`，等等。
  > - **用户界面（49）**——`modal`、`dialog`、`tooltip`、`popover`、`dropdown`、`accordion`、`breadcrumb`、`pagination`、`stepper`、`wizard`、`carousel`、`drawer`、`toast`、`chip`、`skeleton`、`scrollbar`、`search-field`、`empty-state`、`wireframe`、`responsive`、`dark-mode`、`light-mode`、`accessibility`、`keyboard-shortcut`、`cursor-pointer`、`drag-drop`、`click`、`swipe`、`z-index`、`flex-layout`、`grid-layout`、`padding`、`margin`、`border-radius`、`drop-shadow`、`opacity`、`gradient`、`ruler`、`viewport`、`snap-grid`，等等。
  >
  > 为腾出位置，没有任何现有图标被移除或重命名：**名称是稳定的 API**，而你已经写在 `.menu.yaml` 中的名称仍然可以解析。

- **移动条目。** 除了 *Move Up*/*Move Down* 之外，**Indent** 按钮让选中的条目成为它上面那个条目的子项，而 **Outdent** 把它提升回它的父项旁边——两者合起来可以让条目在任意部分和层级之间移动（最多三层）。
- **Accelerator**——键盘快捷键（例如 `Cmd+N`、`Shift+Ctrl+S`）。以平台原生的符号渲染。
- **Action**——点击条目时发生什么：

  - *Event*——触发 `onMenuClick`（由你的事件处理程序决定做什么）。
  - *Open form*——打开/切换到一个指名的窗体。
  - *Set property*——设置控件属性（例如 `BUTTON-1.Enabled=false`）。
  - *Close application*——终止运行中的应用程序。
- **Enabled**——条目是否可点击（禁用时变暗）。

**YAML 文件。** 菜单结构保存为与 `.cfrm` 同一目录中的 `<control-id>.menu.yaml`。该文件包含一个 HMAC-SHA256 完整性哈希；运行时会验证哈希，被篡改的文件会被拒绝。

**颜色属性。** MenuBar 暴露四个颜色属性：`HighlightBgColor`、`HighlightFgColor`（悬停颜色——指针之下的标题，以及指针之下或点击后闪烁的下拉条目）、`SelectedBgColor`、`SelectedFgColor`（打开的标题的颜色）。`BackgroundColor` 和 `ForegroundColor` 也在那里，供你想自己挑选菜单栏自己的表面和标题墨色时使用；不动它们时，菜单栏读取它的周围——在新拟物窗体样式下它采用柔和的表面，并挑选与它最终所坐落的东西形成对比的标题墨色，所以你没有重新着色的菜单栏在深色和浅色窗体上都保持可见和可读。

**事件。** 当任何动作条目被点击或它的快捷键被按下时，`onMenuClick` 触发。下拉列表打开/关闭时，`onMenuOpen` / `onMenuClose` 触发。

**哪个菜单项被选中了。** 在 `onMenuClick` 处理程序中读取 MenuBar 的 **`SelectedItemId`** 属性。它保存条目的 **id**，并且它在事件触发之前刚刚被写入——所以它总是这次事件所涉及的那个条目，不论它是被点击的，还是通过它的快捷键到达的。

每个条目在你于菜单编辑器中添加它时得到它的 id：四个小写字母，例如 `kqpv`，在同一个窗口中绝不与任何菜单的另一个条目——或任何工具栏的组或按钮——共用。id 是固定的——重命名或移动条目不会改变它，而编辑器也没有提供键入它的办法。它显示在条目标签的旁边，带一个把它放到剪贴板上的 **Copy** 按钮，可以直接粘贴到你的处理程序中：

```cobol
       MENUBAR-1--ONMENUCLICK.
           MOVE MenuBar-1::SelectedItemId TO WS-ITEM
           EVALUATE WS-ITEM
               WHEN "kqpv" PERFORM SAVE-DOCUMENT
               WHEN "zmae" PERFORM OPEN-DOCUMENT
               WHEN "txbo" PERFORM SHOW-ABOUT
           END-EVALUATE.
```

> ⚠️ **`COBOL-CONTROL-ID` 是 MenuBar，不是条目。** 一个处理程序服务整个菜单，所以它收到的控件 id 是菜单栏自己的（`MenuBar-1`）。请根据 `SelectedItemId` 分支。
>
> ⚠️ **在编辑器中打开较旧的菜单或工具栏会给它重新编号。** 这同样适用于 MenuBar、SideMenu 和 ToolBar。不是四个小写字母的 id——`item-3`、`button-2`、手写的 `file-save`——或窗口中另一个条目已经使用的 id，会在编辑器打开的那一刻被替换成生成的 id；**OK**/**Save** 保留更改，**Cancel** 放弃它。请更新任何指名旧 id 的 COBOL：`SelectedItemId` 或 `LastButton` 上的 `WHEN`、SideMenu `AddItem` 的 `parent-id`，或按钮的派生名称（`TOOLBAR-1-GROUP-1-BUTTON-1::Tooltip` 变成 `TOOLBAR-1-FMTG-BNSQ::Tooltip`）。按钮自己的处理程序不需要任何改动——它随按钮一起移动。

**启用和禁用条目。** 每个条目都带有你在菜单编辑器中设置的 **enabled** 标志，被禁用的条目画成灰色，并且不引发 `onMenuClick`。

> ⚠️ **该标志是设计时的设置。** 没有 COBOL 调用可以在应用程序运行期间打开或关闭某个菜单项。如果某个动作在某些状态下必须不可用，请在该条目处理程序的顶部检查那个状态并返回，而不要试图把条目变灰。

### Snackbar（瞬时通知）

**Snackbar** 在不打断操作员的情况下告诉他们一件事。它是一条出现在窗体之上的简短消息，停留几秒，然后自己离开——没有需要点击关闭的 OK 按钮，没有模态循环，也不期待任何回答。

如果你曾想用消息框来说*“记录已保存”*或*“无法连接服务器”*，这才是你想要的。消息框要求点击之后操作员才能继续；Snackbar 则根本不打断他们。请把消息框留给你确实需要得到答案的问题。

**你放下的控件是模板，而不是消息。** 这是需要弄对的一个概念，并且它与大多数控件不同。Snackbar 位于设计器的非可视托盘中，在 `Timer` 和 `IndexedFile` 旁边——它在画布上没有大小也没有位置，并且什么都不画。它所持有的是*默认值*。每次 `Show()` 都根据那一刻这些值是什么，铸造一条**新的**通知：

```cobol
       MOVE "Record saved" TO SNACK-1::Text
       INVOKE SNACK-1::Show()
       MOVE "Index rebuilt" TO SNACK-1::Text
       INVOKE SNACK-1::Show()
```

这会让屏幕上有**两条**消息，一个叠在另一个之上。第一条仍然写着 `Record saved`——通知是一个快照，所以之后更改 `Text` 永远不会改写已经显示的消息。

> **注意。** 在其他每个控件上，`Show()` 的意思是“让这个控件可见”。Snackbar 是非可视的，没有什么可以让它可见，所以在那里 `Show()` 的意思是“发出通知”。你现有的窗体没有任何变化：`BTN-OK::Show()` 仍然显示按钮。

**类别替你完成样式。** 设置 `Category`，颜色、图标和超时就会随之而来：


| `Category` | 背景 | 墨色 | 图标 | 超时 | 用于 |
| ---------- | ---------- | --- | ---- | ------- | ---------- |
| `Info` | `#1E4E8C` 深蓝色 | `#F2F7FF` | `info-circle` | 4000 ms | 确认、进度，任何中性的东西 |
| `Question` | `#4B3A8C` 靛蓝色 | `#F5F2FF` | `help-circle` | 6000 ms | 邀请做出决定 |
| `Warning` | `#8A5A0B` 深琥珀色 | `#FFF7E8` | `warning-triangle` | 6000 ms | 有些地方看起来不对，但工作继续了 |
| `Error` | `#8C2323` 红色 | `#FFF0F0` | `error-circle` | 8000 ms | 某个操作失败了 |
| `Critical` | `#5A0F0F` 深红色 | `#FFEAEA` | `critical-octagon` | 保持到被关闭 | 严重；必须被确认 |

每种墨色都是它自己背景的淡色调，所以类别总是读得出来。`Critical` 刻意比 `Error` 更暗，并且带的是八边形而不是圆形。

这些是默认值，不是固定的外观。你自己设置任何属性，你的就胜出——而且*单独*胜出，所以选择一个 `BackgroundColor` 会让类别的图标和墨色保持原位。把颜色留**空**，表示“由类别决定”，这正是让对 `Category` 的一次 `MOVE` 就能给整条消息重新设置样式的原因：

```cobol
       MOVE "Cannot reach the server" TO SNACK-1::Text
       MOVE "Error" TO SNACK-1::Category
       INVOKE SNACK-1::Show()
```

> ⚠️ **覆盖会隐藏类别，而且很容易不小心设置了一个。** Snackbar 的 `BackgroundColor`、`ForegroundColor` 和 `CategoryIconColor` 刻意开始时为空。在检查器中，每一行显示通知实际将要绘制的颜色，并在未设置时读作 **"default"**；一旦你挑选了一个，该行就显示你的十六进制值，并提供一个把它放回“由类别决定”的 **↺**。如果 `Critical` 消息不是红色，请先看那里——显式的 `BackgroundColor` 是通常的原因。
>
> 由较旧版本保存的 Snackbar 可能带有 `#F0F0F0` 和 `#FFFFFF`，也就是每个控件过去起始时的颜色。这些同样算作未设置，由类别决定。

**通知总是读得出来。** 当你没有选择文字颜色时，文字、图标和按钮会用一种与当前生效的背景形成反差的颜色绘制，达到无障碍指南对文字要求的 4.5:1 对比度。如果类别的墨色相对于你挑选的背景达不到它，它就变成黑色或白色，哪个读得出来用哪个。自己选择 `ForegroundColor`，就会完全按你的设置使用。

**Timeout** 以毫秒计。`-1`——默认值——表示“使用类别的”。`0` 表示它一直停留，直到有东西把它关闭。大于 0 的任何值就是那么多毫秒：

```cobol
       MOVE 2500 TO SNACK-1::Timeout      *> two and a half seconds
       MOVE 0    TO SNACK-1::Timeout      *> stays until dismissed
       MOVE -1   TO SNACK-1::Timeout      *> back to the category default
```

指针停在通知上时，它的超时被**保持**，并在指针移开时按剩下的恰好那么多继续——正在阅读消息的操作员永远不会让它在光标之下消失。用 `PauseTimeoutOnHover` 把它关掉。

**每条通知都有一个内置的关闭按钮，在右上角**（1.63.30）——不论 `Category`，也不论你声明了什么按钮。这是操作员自己关闭某一条消息的方式，包括从不自己超时的 `Critical` 消息。它触发它自己的关闭原因 `User`——不同于 `Timeout`（自己过期）、`Action`（按钮的 `dismiss=true`）、`Programmatic`（`DismissAll()`）和 `Overflow`（为腾出位置被挤出——见 `OverflowBehavior`）——所以读取关闭原因的处理程序总是能把这五者分辨开。它只是界面上的功能：没有可以通过 CALL 调用的等价物来关闭单条通知；`DismissAll()` 仍然是唯一的编程式关闭，而且它清除这个控件发出的每一条活动通知，而不只是一条。

**按钮。** 最多三个，在 `Buttons` 属性中每行一个，字段以 `|` 分隔。末尾的字段可以省略：

```
retry|Retry|refresh|Left|true
later|Later|||false
```

字段是 `id|text|icon|position|dismiss`。**id** 是你的处理程序所读取的——它是你自己给按钮起的名字，并且像每个其他 COBOL 标识符一样保持英文。**icon** 是任何目录图标名称（`refresh`、`x-mark`、`undo`、`check`），**position** 是 `None`、`Left` 或 `Right`，而 **dismiss** 决定点击是否关闭通知（默认 `true`）。

按钮对指针的响应方式与工具栏的相同：它的井在指针之下变亮，在鼠标按钮按住时加深，所以在处理程序运行之前，按下就已在屏幕上得到确认。

**从 COBOL 声明按钮——`Clear()` 和 `AddButton()`。** 上面的属性是*设计器*书写一行的方式。在处理程序中，不要直接写 `Buttons`：分隔符是换行，而 COBOL 字面量不能包含换行，所以向 `Buttons` 的 `MOVE` 不论带多少个 `|`，都只能声明**一个**按钮。请改为一次调用一个地声明它们：

```cobol
           INVOKE SNACK-1::Clear()
           INVOKE SNACK-1::AddButton("id=undo,caption=Undo,icon=undo,position=1")
           INVOKE SNACK-1::AddButton("id=later,caption=Later,position=2,dismiss=false")

           MOVE "Saved. Undo?" TO SNACK-1::Text
           MOVE "Warning"      TO SNACK-1::Category
           INVOKE SNACK-1::Show()
```

`AddButton` 接受以逗号分隔的 `key=value` 对。除 **`id`** 之外，每个键都是可选的——它是 `onButtonClick` 所报告的，所以没有它的规格不声明任何按钮，并会在诊断跟踪中说明，而不是显示一个空白：


| 键                   | 含义                                                                              |
| --------------------- | ---------------------------------------------------------------------------------- |
| `id`                  | **必需。** 你自己的英文名称；作为 `LastButtonId` 返回。                 |
| `caption`（或 `text`） | 按钮上的文字。省略则为仅图标。                                     |
| `icon`                | 目录图标名称（`undo`、`refresh`、`x-mark`、`check`，……）。                  |
| `position`            | 按钮的序号，**从 1 开始，从左到右**。省略 = 在末尾，按调用顺序。 |
| `dismiss`             | `true`（默认）在点击时关闭通知；`false` 让它保持显示。           |
| `iconposition`        | `None`、`Left` 或 `Right`。省略 = 给出图标时为 `Left`。                 |

Snackbar 上的 `Clear()` 清空**按钮行，而不清空别的**——文字、类别和颜色保持它们所持有的。这刻意不同于 TextBox 或列表上的 `Clear()`，后者擦除内容：在这里，清除处理程序即将显示的消息会是个陷阱。它只影响模板，所以已经在屏幕上的通知不受触动。

没有 `Clear()` 时，`AddButton` 是**添加到**设计器设置的行上，这正是你给一对固定按钮再追加一个情境按钮的方式。`position` 是一个插入点而不是固定的位置，所以两个按钮永远不会都声称同一个位置。标题中的逗号会被保留（`caption=Saved, undo?` 是一个标题）；`|` 会被剥除，因为它是行自己的分隔符。声明第四个按钮会被报告，绝不会被悄悄丢弃：设计器会标记它，而运行时它会进入诊断跟踪。

绑定 `onButtonClick` 并读取按下的是哪一个：

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACK-1--ONBUTTONCLICK.
       PROCEDURE DIVISION.
           EVALUATE SNACK-1::LastButtonId
               WHEN "retry"
                   PERFORM SEND-THE-RECORD-AGAIN
               WHEN "later"
                   CONTINUE
           END-EVALUATE.
```

**它们出现在哪里。** `StackAnchor` 选择九个位置之一——`TopLeft`、`TopCenter`、`TopRight`、`CenterLeft`、`Center`、`CenterRight`、`BottomLeft`、`BottomCenter`、`BottomRight`——而 `Margin` 设置与边缘的间隙。堆叠**只有垂直方向**：Top 锚点向下增长，Bottom 锚点向上增长，并且两种情况下最新的消息都是最靠近锚点的那一条。从中间关闭一条，其余的会立即合拢间隙。

锚点是针对**你窗体自己的表面**度量的，而不是屏幕。在应用外壳中，`Embedded` 窗体的消息出现在它的 ContentPane 之内——永远不会在外壳的导轨或面包屑之上——所以它们落在操作员已经在看的地方。

> ⚠️ **是 `StackAnchor`，不是 `Anchor`。** 每个控件已经有一个 `Anchor` 属性，而它完全是另一回事：一个勾选框，用来在设计画布上锁住控件使其不能被拖动。Snackbar 的位置是 `StackAnchor`，它与 `StackSpacing` 和 `StackOrder` 放在一起。

**当多条同时到来。** `MaximumVisible`（默认 5）限制同一个 Snackbar 的消息同时出现的数量，而 `OverflowBehavior` 决定再多一次 `Show()` 会怎样：

- `Queue`——把它扣住，等有空位时再发出。它的超时从它*变得可见*时才开始，所以排队的消息仍然被完整地看到。
- `DiscardOldest`——关闭最旧的来腾出位置。
- `DiscardNewest`——丢弃新来的。

**它们如何移动。** 通知是有动画的，效果按固定的时长运行：

- 到来的那条在它将占据的位置**放大并淡入**，历时 **600 ms**——它不会从屏幕外飞进来。`Critical` 消息改用 **200 ms**：最紧急的类别应当是操作员抬头时已经在那里的。效果的其他方面都不取决于类别。
- 已有的通知向上或向下（取决于锚点往哪边堆叠）**滑动** **300 ms** 以腾出位置，并在有一条离开时滑回去合拢间隙。它们从不跳跃。
- 离开的通知——过期、被关闭，或被 `OverflowBehavior` 挤出——在它所站的位置用 **300 ms** **淡出**。它不会缩小消失，并且在它离去时，幸存者围绕它合拢间隙。

**它们一次到来一条。** 同一个处理程序中的两次 `Show()` 调用会放出两条消息，但它们不会一起进来：第二条要等到第一条完成到来，然后已经在上面的那些滑开，它才在那片空间中开始出现。所以一次发出三条，大约要两秒半才能全部出现在屏幕上，按它们被发出的顺序进入。锚定在*不同*角落的消息是独立的堆叠，彼此从不等待——队列是按锚点的。

消息的 `Timeout` 从它**变得可见**的那一刻起计时，而不是从发出它的 `Show()` 起，所以排在第三的一条仍然被读足它的完整时长。

这一切都不需要你去驱动：效果是自动的，而通知的事件**不会**等待它们。`onClosing` 和 `onClosed` 在它关闭的那一刻触发，它所占的位置对下一次 `Show()` 立即空闲——停留 300 ms 的是画面，而不是通知。

**清除它们。** `DismissAll()` 关闭**这个**控件发出的每一条通知，并丢弃它排队的任何东西。窗体上的其他 Snackbar 控件不受触动：

```cobol
       INVOKE SNACK-1::DismissAll()
```

没有可以通过 COBOL 调用的、针对单条通知的 `Hide()`。由于 `Show()` 每次都铸造一条新通知，`Hide()` 无法说明它指的是*哪一条*——那正是操作员自己的关闭按钮的用途（见上文）；它是界面，不是你的处理程序可以伸手去调用的 CALL。

**事件。** `onShown` 在消息加入堆叠时触发，`onTimeout` 在它的时间用尽时触发，然后 `onClosing` 和 `onClosed` 在它离开时触发——二者都携带原因（`Timeout`、`User`、`Action`、`Programmatic`、`Overflow`）——而 `onButtonClick` 在按钮被按下时触发。`dismiss` 为 `true` 的按钮**先**触发 `onButtonClick`，之后才关闭，所以你的处理程序仍然可以读取它被点击的那条通知。

> ⚠️ **`onShown` 是 `Show()`，不是画面。** 它在消息被接受进入堆叠时触发，这是在它于到来队列中等完它的轮次之前，也是在它完成放大进入之前。这是刻意的：没有任何事件会等待动画。如果你需要在消息真正出现在屏幕上时采取行动，你拥有的就是 `onShown` 加上上面的到来时间——没有单独的“完成到来”事件。

> **注意。** `Text` 是数据，不是格式字符串——没有任何东西会被替换进去。请先在 COBOL 中构建消息，就像你构建任何其他标题那样：
>
> ```cobol
>        STRING "Saved " DELIMITED BY SIZE
>               FUNCTION TRIM(WS-CUSTOMER-NAME) DELIMITED BY SIZE
>               INTO WS-MESSAGE
>        MOVE FUNCTION TRIM(WS-MESSAGE) TO SNACK-1::Text
>        INVOKE SNACK-1::Show()
> ```

> ⚠️ **注意事项——通知不是对话框。** 它从不阻塞，从不取得焦点，也从不等待。如果你的程序在操作员回答之前不得继续，Snackbar 就是错误的控件：`Show()` 之后的下一条语句会立即运行，而消息仍在屏幕上。

> ⚠️ **注意事项——`Size` 限制文字。** `Small`、`Medium` 和 `Large` 分别允许一行、两行和三行；更长的内容会被省略号截断，而不是让通知增长。窗口从不会为了容纳消息而调整它自己的大小。

📷 需要截图 — `snackbar-stack.png`。运行一个带有锚定在 `BottomRight` 的 Snackbar 的窗体，在一个按钮处理程序中发出三条不同类别（Info、Warning、Error）的通知，并在三条都堆叠着时捕获窗口，使垂直堆叠、类别颜色和图标都可见。

### Viewer（窗体内的文档）

**Viewer** 在你构建的窗体之内显示文档——纯文本、Markdown、图像、PDF 或 HTML 页面——带有工具栏、页面导航、缩放、页面缩略图导轨、Find、Print 和 Save As。

它的存在，是因为*生成*文档的 COBOL 程序，直到现在都没有地方来*显示*它们。你写了一条 PDF 语句或一份报表，然后把它交给某个别的程序，你的操作员就离开你的应用程序去阅读你的应用程序刚刚生成的东西。Viewer 把它保留在工作所在的地方。

如果你用过 PowerCOBOL 或 isCOBOL，你所知道的最接近的东西大概是 OLE 容器或嵌入的预览窗格——而最重要的区别就是这个：Viewer 是一个**普通控件**。你把它放在窗体上，设置它的属性，绑定它的事件。没有要注册的容器，没有要安装的外部应用程序，也没有会失败的第二个进程。

#### 打开文档

有三种方式。一个路径：

```cobol
       MOVE "reports/september.pdf" TO VWR-1::Source
```

一个网址：

```cobol
       MOVE "https://example.com/handbook.md" TO VWR-1::Source
```

或者你的程序已经持有的字节：

```cobol
       INVOKE VWR-1::LoadBytes(WS-DOCUMENT-BUFFER)
```

在设计器中，`Source` 不是一个你必须盲目键入的字段。它带有一个打开你操作系统自己的文件选择器的 **📂** 按钮，以及一个清除所选内容的 **✕**——并且它接受直接键入其中的 `http://` 或 `https://` 地址。

> **注意——网址是文档，不是网页。** Viewer 下载该地址返回的内容一次，然后把它当作磁盘上的文件那样打开：每种格式、每种布局和每种逐页读取都照常工作。什么都不会被执行。没有脚本会运行，页面引用的样式表或图像也不会被跟随，而 HTML 地址打开的是下表所描述的 HTML **子集**，而不是浏览器会渲染的样子。用它来把已发布的手册、发行说明或生成的报表呈现在操作员面前——而不是嵌入 Web 应用程序。
>
> 下载发生在与读取相同的后台线程上，所以慢的服务器表现得像慢的磁盘，而你的窗体继续重绘。文档会被缓存，所以重新打开窗体不会再次获取它。拒绝的服务器、无法解析的名称，或超过 256 MB 的文档，会引发 `onError`，原因在 `LastError` 中，恰如无法读取的文件那样。

不论哪种方式，Viewer 都先根据文档的内容、其次根据它的文件名来判断它**是什么**，并在 `Format` 中报告它的判断。被某人命名为 `.txt` 的 PNG 仍然作为图片打开。它无法理解的文件会引发 `onError` 并设置 `LastError`，并且——这是值得记住的部分——**让你已经在显示的文档留在屏幕上**。一个坏的文件名不会让你的窗体变空白。

加载会在进行时自我报告，所以大文档可以驱动进度条：

```cobol
       PROGRAM-ID. VWR-1--ONLOADPROGRESS.
       PROCEDURE DIVISION.
           MOVE VWR-1::Progress TO PROG-1::Value
           .

       PROGRAM-ID. VWR-1--ONLOADED.
       PROCEDURE DIVISION.
           MOVE 0 TO PROG-1::Value
           MOVE VWR-1::Format TO LBL-FORMAT::Caption
           .
```

> **注意——读取发生在绘制之外。** 为一个两吉字节的日志建立索引，不会让你的窗体停止重绘或响应鼠标，而且 Viewer 从不把整个文档保存在内存中：它保留一个有界的页面窗口，其余的按需读取。跳到这样一个文件的最后一页，花费的是一页的读取，而不是走完它之前的所有内容。

#### 每种格式实际给你什么

这是本节中唯一最重要的表，它是一份契约而不是一种期望。Viewer 所做的一切都是纯 COBOL 运行时代码，没有需要安装的外部解码器，而这个决定为某些格式设定了上限。

| 格式 | 你得到 | 你**得不到** |
|---|---|---|
| 纯文本 | 全部，任何大小 | — |
| Markdown，带表格、任务列表、脚注和删除线 | 全部，任何大小 | — |
| Mermaid 图——Markdown 围栏，或 HTML 页面的 `<div class="mermaid">` / `<pre class="mermaid">` | 流程图和时序图，由 Viewer 自己绘制（页面的 `mermaid.js` 从不运行，也不需要）。流程图的连接线是正交的——每一段不是水平就是垂直——并在每个形状一边的中点处与它相接，判断菱形则在顶点处相接；必须经过其他形状的连接线，会在它自己的通道中绕过它们 | 类图、状态图、甘特图、ER 图和旅程图——**按名称拒绝**，而不是只画一半。根本不是 Mermaid 的块——由括号和箭头构成的绘图——就按它本来的文本显示 |
| 图像：PNG、JPEG、GIF、WebP、APNG、BMP、TIFF、SVG | 全部，包括动画 | — |
| PDF | 它的文本、它的基本线条和矩形绘制、它的页面大小、它的分页，以及对所有这些内容的 Find | 复杂页面的忠实图片；不寻常的嵌入字体编码；表单；批注；超出它所嵌入的图像的扫描页面 |
| HTML | 一个**子集**：块和行内布局、常见排版、颜色、边框、表格、图像——**由页面自己的 CSS 设置样式**，包括 flex 行和 grid（见下文 *HTML 及其 CSS*） | 浮动和定位；动画和变换；`@media`；任何被获取的东西；JavaScript。**它不是浏览器** |
| 视频 | 没有——超出范围。动画 GIF、WebP 和 APNG 在上面作为图像涵盖 | 任何需要外部编解码器的格式 |
| Word、PowerPoint、Excel、OpenDocument | 它们的**文本**，作为 Markdown——标题、列表和表格得以保留——由知识库自己的转换器生成，所以文档在这里读起来与聊天读到的相同 | 页面布局、字体和图片 |

> **注意——边界在哪里显现。** 格式的限制是可见的，而不是无声的。Viewer 不绘制的 Mermaid 图，会在该图本来所在的地方按名称说明这一点。以此渲染器无法跟随的方式排版的 HTML 页面，**会丢失它的布局，但保留它内容的每一个词**，因为丢掉词语会是更糟的失败。没有文本层的扫描 PDF 报告没有文本，而不是编造一些。

#### HTML 及其 CSS

HTML 文档是**应用它自己的 CSS** 绘制的——其中任何位置的 `<style>` 块（包括它的 `<head>`）以及任何元素上的 `style="…"`。规则按你对浏览器所期望的方式层叠：更具体的选择器胜出，后面的规则胜过前面的，`!important` 胜过两者，元素自己的 `style` 胜过每一条样式表规则，而颜色和字体向下传递给元素所包含的东西。自定义属性也有效——`:root` 上的 `--primary: #667eea`，在用到它的任何地方用 `var(--primary)`，如果你给了回落值就带上回落。

CSS 能表达并得到遵从的内容：

| 领域 | 属性 |
|---|---|
| 文本 | `color`、`font-size`（px、em、rem、%、pt、关键字）、`font-weight`、`font-style`、`font-family`（等宽字体系列会被遵从；其他的映射到 Viewer 自己的字体）、`font`、`text-decoration`、`text-transform`、`letter-spacing`、`line-height` |
| 盒子 | `background`/`background-color`——颜色或 `linear-gradient(…)`；`border` 以及每一边自己的（`border-left: 4px solid …`）；`border-radius`；`padding`；`margin`，包括 `margin: 0 auto` 居中和负外边距；`width` 和 `max-width`，各以 px 或 % 计（`width: 80%; max-width: 1600px; margin: 0 auto` 是占用 Viewer 的 80 %、最多 1600 CSS px 的页面）；`box-shadow` |
| 盒子大小 | `height` 和 `min-height`（盒子至少那么高）；`border-radius: 50%`——方形盒子变成圆 |
| 对齐 | `text-align`——left、center、right |
| Flex | `display: flex`、`flex-direction: column`、`flex-wrap`、`flex-flow`、`gap` / `row-gap` / `column-gap`、`align-items`、`justify-content`（start、center、end、space-between、space-around、space-evenly）；对于条目：`flex`（`1`、`none`、`0 0 60px`）、`flex-grow`、`flex-basis`、`align-self` |
| Grid | `display: grid`、`grid-template-columns`——长度、百分比、`fr`、`auto`、`minmax()`、`repeat(3, 1fr)` 和 `repeat(auto-fit, minmax(220px, 1fr))`；`gap`；对于条目：`grid-column: span 2` 和 `1 / -1` |
| 可见性 | `display: none`、`visibility: hidden` |
| 表格 | 每个单元格的背景、内边距、对齐和边框——用 `tr:nth-child(even)` 写的条纹表格会有条纹 |
| 行内 | `<span>` 的背景显示为它文字后面的高亮——徽章、`<mark>` |
| 行内盒子 | `display: inline-block`、`inline-flex`、`inline-grid`：只容纳文本时，元素留在它的行内，它的背景是高亮；容纳块时，它是一个与其内容一样宽的盒子，而不是与其容器一样宽 |
| 长度 | px、em、rem、%、pt，以及对它们的 `calc()`——`calc(100% - 2rem)`、`calc(-1 * var(--gap))` |

选择器：标签、`.class`、`#id`、`*`、后代（`nav a`）、子元素（`ul > li`）、紧跟在另一个之后的元素（`h1 + p`）或任何后面的同级元素（`h1 ~ p`）、逗号分隔的组，以及 `:first-child`、`:last-child`、`:only-child`、`:nth-child(…)`、`:nth-last-child(…)`、`:root` 和 `:not(…)`（`.box:not(:last-child)`）。颜色以 CSS 所写的每种形式出现：`#rgb`、`#rrggbbaa`、`rgb()`/`rgba()`、`hsl()`/`hsla()` 以及所有的命名颜色。

文本按浏览器排版的方式排版：页面源码的换行和缩进折叠成单个空格，而无单位的 `line-height`（`1.5`）是每个元素应用于它自己字号的系数。在 flex 或 grid 行中，空单元格保留它的位置，被拉伸到行高的盒子仍然把它所容纳的内容居中（`align-items: center`），像标签这样的短条目永远不会被挤到它自己的宽度之下，而列遵从它的条目的 `width` 和它自己的 `justify-content`。

`@media` 按 **1024 px 宽**的桌面屏幕会回答的方式来回答：页面的 `(min-width: 769px)` 规则适用，它的 `(max-width: 768px)` 手机覆盖和它的 `print` 规则则不适用。

> **注意——尺寸跟随 Viewer 的缩放。** CSS 的 16 px 是 Viewer 的基础字号，页面上的每个长度都从它度量。缩放和 `FontSize` 把带样式的页面作为一个整体缩放——它的内边距和边框随它的文字一起增长。

> **注意——flex 行和 grid。** `display: flex` 容器把它的子项并排放置：每个取它所声明的宽度、`flex: 1` 给它的份额，或者——两者都没有声明时——它的内容所需的宽度，剩余的交给 `justify-content`。`align-items: center` 让每个条目在行内居中，而默认的 `stretch` 让一行中的每张卡片高度相同。`display: grid` 容器把它的子项排布在它的列中，每当一行满了就开始新的一行。两者的每个元素子项都是它自己的一个条目。信息图的步骤列表——每个步骤文字旁边一个圆形的、有颜色的数字——和三张卡片的一行，都是这样构建的。

> ⚠️ **注意事项——没有浮动，没有定位。** 由 `float` 或 `position` 构建的布局不会被构建：这些块按顺序堆叠，每个在它自己带样式的盒子中。此子集不理解的规则——`:hover`、`::before`、属性选择器——会被跳过，绝不猜测。1024 px 的 `@media` 回答是固定的：窄的 Viewer 不会切换到页面的手机布局。`width` 把边框和内边距算在内，就像 `box-sizing: border-box` 那样。

> ⚠️ **注意事项——什么都不会被获取。** `@import`、`<link rel="stylesheet">`、`url(…)` 背景和网页字体都被忽略。加载页面所要求的一切的查看器就是浏览器，带着浏览器的暴露风险。页面的样式必须在页面之内——有一个例外，下面的 Bulma。

> **注意——Bulma，内置。** `<head>` 链接了 Bulma 的页面——`<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css">`，或任何指名 `bulma` 的 `href`——由 **Bulma 0.9.4 本身**设置样式，它随 Viewer 一起交付：什么都不会被下载。浏览器打开同一个页面，会从链接加载同一个 Bulma，所以一个没有 JavaScript 的 HTML 文件在两者中看起来是一样的。Bulma 的布局和组件都能过来——`section`、`container`、`hero`（它的颜色和 `is-bold` 渐变）、`columns` / `column` 及它们的宽度、`level`、`media`、`tile`、`title` 和 `subtitle`、`box`、`card`、`notification`、`message`、`panel`、`tag`、`table`——以及辅助类 `has-text-*`、`has-background-*`、`is-size-*`、flex 辅助类和 `m-*` / `p-*` 间距。Bulma 在页面自己的 `<style>` 之前层叠，就像链接的样式表那样，所以页面仍然可以覆盖任何规则。
>
> ```html
> <head>
>   <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/bulma@0.9.4/css/bulma.min.css">
> </head>
> <body>
>   <section class="hero is-info is-bold"><div class="hero-body">
>     <p class="title">Leave report</p><p class="subtitle">First half</p>
>   </div></section>
>   <div class="columns">
>     <div class="column is-half"><div class="box">Operations: <strong>82%</strong></div></div>
>     <div class="column"><div class="notification is-warning is-light">Book overdue leave by July 31.</div></div>
>   </div>
> </body>
> ```
>
> 写出这样一个页面的 COBOL 程序——`ASSIGN TO VIEWER`、一个用 `Source` 显示的文件、来自 `AgentObject` 的回答——不需要其他任何东西。

> ⚠️ **注意事项——Bulma 在这里做不到什么。** Viewer 把 `@media` 当作 1024 px 的桌面来回答，所以显示的是 Bulma 的桌面布局。句子中的 `.tag` 或 `.button` 是没有内边距和圆角的高亮（在 `.tags` 或 `.buttons` 之内它是一个完整的盒子）。悬停状态、`::before`/`::after` 装饰（`delete` 按钮的叉、`select` 的箭头）和表单控件都不会被绘制，而由 JavaScript 驱动的部分（导航栏的汉堡按钮、下拉列表、模态窗口）超出范围：这个功能所针对的页面是报表。

#### 布局

`Layout` 决定文档如何呈现：

| `Layout` | 它看起来怎样 |
|---|---|
| `Raw` | 字面的源码，等宽且未格式化 |
| `Web` | 已格式化，没有页边距 |
| `Print` | 页边距、纸张边框和纸张阴影 |
| `Page` | 打印布局，不论你窗体的主题是什么，都是白底黑字的页面 |
| `Streamed` | 一个内容窗格，完全没有外框——用于聊天机器人对话，在本节末尾描述 |

`Page` 说的是分页，而不是文档的种类：Markdown 和纯文本与 PDF 一样乐意分页。

**文档是黑色的。** 它的文字在每种布局——`Raw`、`Web`、`Print`、`Page` 和 `Streamed`——中都是深色墨水，并且不跟随窗体的主题，因为文档不论在哪里显示都是文档。只有文档本身才能覆盖这一点：它所声明的颜色——`<font color="#E00000">`，或它的 CSS——就是它所得到的颜色。

`FontSize` **独立于** `Zoom` 缩放文字，所以想要更大文字的读者不必为此放大整个页面。

#### 在文档中穿行

读者能用鼠标做的一切，你的程序也能做——这个控件里没有 COBOL 触及不到的能力。

```cobol
       MOVE 150 TO VWR-1::Zoom
       MOVE 4 TO VWR-1::Page
       MOVE 1 TO VWR-1::Fullscreen
       MOVE "Cards" TO VWR-1::ViewMode
```

- **全屏意味着整个屏幕。** 打开它会让你窗体自己的窗口进入平台的全屏，并用 Viewer 覆盖它。工具栏随之而来——它的 Fullscreen 按钮，处于按下状态，就是回去的路，而 `Esc` 也做同样的事，从平台自己的控件离开全屏也是如此，控件会注意到并把它写回，使属性永远不会声称一个窗口并不处于的状态。`Fullscreen` 是普通属性，所以 COBOL 程序打开和关闭它，恰如操作员所做的那样。
- **Zoom** 最高可达十六倍。配合你平台的缩放修饰键使用滚轮，会围绕指针放大——指针之下的东西保持在指针之下。在 `Full` 模式下，双击缩放一步；在卡片之上则改为打开那一页。`Esc` 回到 100 %；在全屏中，它先离开全屏。它既作用于页面，也作用于**图片**：100 % 是适应视图的整幅图像，其他每个缩放都是按那个大小缩放，而比它的视图更大的图片像任何其他文档一样滚动。滑块停在常见的刻度上——50、75、100、125、150、200 等等——而不是停在指针碰巧所在的任何百分比上。
- **View mode**（`ViewMode`）是 `Full`（文档）或 `Cards`（每页一张卡片的网格，取代文档）。网格按**控件自己的宽度**重新排布——把控件加宽，就得到更多的列；把*窗口*加宽则什么都不变，因为控件不应依赖它看不到的东西。卡片是**缩小的页面**，由绘制页面的同一个渲染器绘制，所以标题就是标题，图片就是那幅图片。点击一次选中卡片；点击两次打开它，从 `Cards` 回到 `Full` 并停在那一页。读者没有打开过的页面不会被解码——控件从不读取没有被要求读取的文档——所以那些卡片显示一小段摘录和它们的编号。
- **关闭一侧会关闭它的文档。** 并排放着两个不同的文件时，要求在*左边*改为单视图，会让*右边*的文档留在幸存的视图中——你关闭的是左边那个。同一个文件的两个视图没有可以失去的文档，所以在那里只有视图被关闭。
- **一个滑块**，位于每个视图的右下角，在 `Full` 模式下驱动 `Zoom`，在 `Cards` 模式下驱动 `CardSize`。切换模式从不扰动你留在另一个模式的值。
- **Filmstrip** 是停靠在内容左边缘的页面缩略图导轨。它有两种关闭方式：再次点击它的工具栏按钮，或把它的分隔条一直拖到左边缘。点击缩略图一次选中那一页；点击两次在它的顶部显示它。它在 `Cards` 模式下不被绘制——网格本身已经就是那份列表——而 `ShowFilmstrip` 保持你设置的值，所以回到 `Full` 会把导轨带回来。
- **文本按词换行**，在布局给它的宽度处。对整行来说太长的词会被断开，而不是允许溢出，而不间断空格从不是换行点——这正是键入一个的理由。在围栏代码块之内，长的标记会在必须断开的任何地方断开，因为代码中没有可以断开的空格。
- **文本可以被取走。** 在文档上拖动以选中它，选择会跨越标题、段落、列表项和表格单元格，如读者所期望。`Ctrl`/`Cmd+A` 选中整页，`Ctrl`/`Cmd+C` 复制被选中的内容，而右键点击会在指针处打开一个有两项的菜单——**Select All** 和 **Copy**。开始一次选择会让 Viewer 获得键盘，所以即使指针已经移开，`Ctrl`/`Cmd+C` 也能复制它——例如移到聊天的问题框，否则它会拿走这个按键。并排排布的文本（表格单元格、列、报表的卡片）在你按下的地方选择，不论那是哪一列。到达剪贴板的内容让它的各个块保持在单独的行上，因为它们是单独的块。卡片和图像不可选择：缩略图总览不是页面，而图片没有文本。
- **滚轮属于 Viewer。** 指针在控件之上时，滚轮一格滚动文档并就此停住——它从不同时滚动窗体、或它后面的 Panel 或 GroupBox，即使文档已经在它的最后一行也是如此。
- **滚动**的表现与 IDE 自己的文档查看器相同：方向键轻点一行，按住时加速到那个速度的四倍；Page Up 和 Page Down 移动一屏减两行，让你保持你的位置；Home 和 End 跳到两端；而拖动页面会把它甩出去，在恒定的摩擦下减速到停止。Find 框拥有插入符时，这些都不会发生。

这些中的每一个都在它**稳定下来**时引发一个事件——`onZoomChanged`、`onCardSizeChanged`、`onScrolled`、`onViewModeChanged`、`onFilmstripToggled`、`onFullscreenEntered` 和 `onFullscreenExited`——所以一次滚轮手势是一个事件，而不是每格一个。

#### 查找文本

`Ctrl+F`（Mac 上是 `Cmd+F`）打开 Find 栏；`Esc` 关闭它。`F3` 和 `Shift+F3` 逐个走过匹配项，在两端回绕。每个匹配项都被标记，当前的那个被突出显示，而实时计数器说明你是在多少个中的第几个。

这一切都由你来驱动：

```cobol
       INVOKE VWR-1::Find("INVOICE")
       MOVE 1 TO VWR-1::SearchCaseSensitive
       INVOKE VWR-1::FindNext()
       MOVE VWR-1::SearchMatchCount TO WS-HOW-MANY
       MOVE VWR-1::SearchCurrentMatch TO WS-WHICH-ONE
```

没有可搜索文本的格式——独立的照片——报告没有匹配。这是一个答案，而不是错误。

#### 同时两份文档

把 `SplitMode` 设为 `LeftRight` 或 `TopBottom`，会给你两个视图，中间有一条可拖动的分隔线，而每一个都真正是自己的：它自己的文档、页面、缩放、滚动位置、视图模式、filmstrip 和 Find。

```cobol
       MOVE "LeftRight" TO VWR-1::SplitMode
       MOVE "statements/september.pdf" TO VWR-1::View1Source
       MOVE "statements/detail.pdf"    TO VWR-1::View2Source
```

把**两个**视图都指向同一份文档，它只被读取一次，而不是两次——所以操作员可以在浏览同一份报表的另一部分时研究其中一部分，没有额外的成本。

> **注意——朴素的名称属于第一个视图。** `Zoom` 指 `View1Zoom`，`SearchText` 指 `View1SearchText`，依此类推。在你拆分控件之前写好的程序，继续意味着它原来的意思。这两种写法是同一个属性：无论你用 `MOVE` 还是通过方法来写哪一个，另一个读到的都是同一个值。所以用 `MOVE path TO VWR-1::View1Source` 加载的文档，就是 `SaveAs()` 按名称提议并保存的那个。

#### 标记、复制与右键菜单

不论 Viewer 显示什么，操作员都可以标记它并把它带走——不需要 COBOL：

- **文本。** 在它上面拖动。纯文本文件、PDF 的文本、Markdown 或 HTML 页面以及对话，在每种布局中都以同样的方式标记。（`Cards` 是缩略图总览，不是页面：其中什么都不会被标记。）
- **图片。** 点击它。*本身就是*图像的文档，以及 Mermaid 图，是整体被标记的——周围一个实线框，上面一层着色。在图片上开始或结束的拖动会把它包括进来。
- **一切。** **Ctrl+A**（Mac 上是 ⌘A）标记页面的所有文本和所有图片。

**Ctrl+C**（⌘C）复制被标记的内容。该标记被设计成在任何页面上都能被看到：一条不透明的色带，而不是着色，其颜色针对它所在的页面来挑选——在浅色页面上是深蓝色配白字，在深色页面上是黄色配黑字，而当页面两者都不是时则是黑色或白色——所以它上面的字母总是读得出来，色带也总是从页面中突出。

**鼠标右键**在内容之上打开一个小菜单，每个条目旁边是键盘快捷键：

| 条目 | 快捷键 | 它做什么 |
|---|---|---|
| **Select All** | Ctrl+A | 标记页面的所有文本和图片 |
| **Copy** | Ctrl+C | 复制被标记的内容；在有东西被标记之前是灰色的 |
| **Print** | Ctrl+P | 把文档交给操作系统的打印对话框——与工具栏的 Print 是同一个请求，带有同样的 `onPrintComplete` / `onPrintCancelled` |

当指针在 Viewer 之上或 Viewer 拥有键盘时，这些按键才会响应——当另一个控件（例如 Find 框或聊天的问题框）拥有插入符时，从不响应。菜单条目使用 IDE 的语言。

<!-- 📷 需要截图 — viewer-context-menu.png：一个 Viewer，显示带有 Mermaid 图的 Markdown 页面，一个段落的一部分被标记（深蓝色色带上的白色字母），该图被标记（实线蓝色框），并且右键菜单在它旁边打开，显示 Select All、Copy 和 Print 及其快捷键。请在 IDE 为浅色主题时捕获。 -->

> **⚠️ 注意事项**
> - **Copy 只把一样东西放到剪贴板上。** 如果有任何文本被标记，那就是文本；只有在仅仅标记了图片时，图片才会去（如果不止一幅，则是第一幅）。要把图片从页面中复制出来，请单独点击该图片。
> - Markdown 或 HTML 的图像显示为它的描述行，而不是图片，并作为那段文本被标记。
> - **Print 打印文档，绝不是被标记的部分**——操作系统拿到的是文件。

#### 保存与打印

```cobol
       INVOKE VWR-1::SaveAs("archive/september-copy.pdf")
       INVOKE VWR-1::SaveAs()
       INVOKE VWR-1::Print()
```

**`SaveAs` 的两种方式。** 给定路径时，写入的就是那个路径——总是如此，没有默认处理，所以保存到它计算出的文件夹的程序得到它所要求的。**什么都不给**时，会询问操作员：你平台自己的 Save 面板打开，而工具栏的 Save As 按钮做的恰好是同一件事。

框中的名称是一个提议，操作员可以更改它。从 `Source` 打开的文档提议它已有的名称。用 `LoadBytes` 交来的文档没有名称，所以它提议**它自己内容的前三个词**，加上它解析出的 `Format` 所暗示的扩展名——由文档构成的提议胜过 `untitled`。

关闭面板不是错误：什么都不会被写入，并且 `onSaveCancelled` 被引发，因为操作员做了他们想做的事。

```cobol
       PROGRAM-ID. VWR-1--ONSAVECANCELLED.
       PROCEDURE DIVISION.
           MOVE "Nothing saved" TO LBL-STATUS::Caption
           .
```

**`SaveAsPdf` 就是 Viewer 所显示的。** `SaveAs` 复制文档自己的字节。`SaveAsPdf` 则把 Viewer *所绘制的*内容写成 PDF：Viewer 自己的画笔以 A4 页面的宽度排版内容，而它画的每个形状在 PDF 中都成为同样的形状。在 `Layout = Streamed` 之下，那就是对话，它根本没有文件可以复制——用户和智能体的气泡以它们的颜色、带有它自己 CSS（背景、渐变、边框、圆角、阴影、flex 行和 grid）的 HTML 回答、表格、代码、作为图片的 Mermaid 图，以及同样的字体和 emoji。文本保持为文本：在任何 PDF 阅读器中都可以被选中、复制和搜索。分页发生在行与行之间，从不穿过某一行。在对话之外，它以同样的方式处理 Markdown 或文本文档，原样复制 PDF，并对其他任何东西引发 `onError`。

```cobol
       PROGRAM-ID. BTN-PDF--ONCLICK.
       PROCEDURE DIVISION.
           INVOKE VWR-CHAT::SaveAsPdf()
           .
```

不带参数时，同样的 Save 面板打开，提议 `conversation.pdf` 并提供 PDF 类型。带路径时，写入那个路径。事件与 `SaveAs` 的相同：`onSaveComplete`、`onSaveCancelled`、`onError`。

> **注意——PDF 携带它的字体，经过裁剪。** 每种字体只嵌入文档所用的字母，所以几页的对话是几十千字节，而不是几兆字节。无法以这种方式嵌入的字面（轮廓不是 TrueType 的字体）会被画成它的字母形状：它看起来相同，但那段文本无法被选中。

> **注意——`Print` 做什么，以及它的事件意味着什么。** 平台接受的是*文件*，所以从 `Source` 打开的文档原样交出，而交给 `LoadBytes` 的文档则先被写出——使用 Save As 会提议的同一个名称。
>
> **Print** 交给打印系统：macOS 和 Linux 上是 `lp`，Windows 上是外壳自己的 `Print` 动词。假脱机程序接受的文档就是正在打印的文档，所以 `onPrintComplete` 的意思恰是如此。拒绝它的假脱机程序，或没有配置打印机的机器，会引发 `onPrintCancelled`，原因在 `LastError` 中。
>
> 它在绘制之外运行，所以慢的假脱机程序从不会让你的窗体停止重绘。

> ⚠️ **注意事项——`SaveAs` 写入原始字节，而且只有那些。** 它复制文件；它从不写出 Viewer 所画的东西——上面的 `SaveAsPdf` 才是做那件事的调用。这是刻意的，对 PDF 来说最为重要：Viewer 读取 PDF 的结构是为了绘制它，而把*那个*读取结果写回的保存器，会递给你的操作员一份同名的不同文档。Viewer 也从不修改它正在显示的文档。

Print 把文档交给操作系统。所以它的结果由操作系统来报告，事件正是从那里来的：

```cobol
       PROGRAM-ID. VWR-1--ONPRINTCOMPLETE.
       PROCEDURE DIVISION.
           MOVE "Sent to the printer" TO LBL-STATUS::Caption
           .

       PROGRAM-ID. VWR-1--ONPRINTCANCELLED.
       PROCEDURE DIVISION.
           MOVE "Printing cancelled" TO LBL-STATUS::Caption
           .
```

> ⚠️ **注意事项——只有操作系统知道。** `onPrintComplete` 和 `onPrintCancelled`（以及它们旁边的 Save 一对）报告的是操作系统告诉 Viewer 的内容——对于 `Print`，是假脱机程序是否接受了作业，而不是是否有页面出来。你的程序无法事先知道会得到哪一个，也不应该假定是令人愉快的那个。

#### 承载对话

这个控件的最后一部分是同一个盒子的另一项工作：把 `Layout` 设为 `Streamed`，会把 Viewer 变成**对话界面**——一个内容窗格，没有工具栏，没有 Find 栏，没有缩略图——你在回复到达时往里追加。

```cobol
       INVOKE VWR-1::AppendMarkdown("**You:** what were September's totals?")
       INVOKE VWR-1::AppendRaw(WS-REPLY-FROM-SOMEWHERE-ELSE)
```

有三种追加的方式，而且**每次都由你说明用哪一种**：

| 方法 | 内容会怎样 |
|---|---|
| `AppendHtml` | 通过 HTML 子集渲染 |
| `AppendMarkdown` | 作为 Markdown 渲染 |
| `AppendRaw` | **字面地**显示——其中的标记被显示出来，绝不被遵从 |

**聊天气泡。** 给追加一个第二个参数，说明消息来自谁，对话就被画成聊天：`"user"` 把它放在**右边**的气泡中，`"agent"`（或 `"assistant"`）放在**左边**的气泡中。每个气泡与它的文字一样宽，最多到窗格的四分之三。没有角色追加的消息像以前一样横跨窗格——适用于两个声音之间的系统备注。

```cobol
       INVOKE VWR-1::AppendMarkdown(WS-QUESTION, "user")
       INVOKE VWR-1::AppendMarkdown(WS-ANSWER, "agent")
```

颜色是 Viewer 的属性：`UserBubbleColor` / `UserBubbleTextColor`（起初是绿色配白字）和 `AgentBubbleColor` / `AgentBubbleTextColor`（蓝色配白字）。把 `UserBubbleBold` 设为 `1`，会用粗体绘制用户的话。用 `AppendToMessage` 流式写入的消息保持它开始时所在的气泡。

> **注意——粗体就是粗体。** Viewer 中的粗体文本——Markdown 中的 `**…**`、HTML 中的 `<strong>` 或 `<b>`，或页面 CSS 中 `font-weight: 600` 及更重——是用真正的粗体字面绘制的，即系统的粗体 Arial 或 Helvetica。

一点一点到来的回复会延长屏幕上已有的消息，而不是开始一条新的：

```cobol
       INVOKE VWR-1::AppendMarkdown("**Assistant:**") RETURNING WS-MESSAGE-ID
      *> ... and then, for each piece that arrives:
       INVOKE VWR-1::AppendToMessage(WS-MESSAGE-ID, WS-PIECE, "Raw")
```

消息也可以被**替换**——同一个 id、同一个气泡、同一个位置——或被取出。聊天正是这样显示它在做什么而不留下痕迹：一个随着工作推进而变化、最终*变成*答案的状态气泡。

```cobol
       INVOKE VWR-1::AppendMarkdown("*Thinking...*", "agent")
           RETURNING WS-STATUS-ID
      *> ... later, as the work moves on:
       INVOKE VWR-1::ReplaceMessage(WS-STATUS-ID,
           "*Preparing the answer...*", "Markdown")
      *> ... and when the answer arrives:
       INVOKE VWR-1::ReplaceMessage(WS-STATUS-ID, WS-ANSWER, "Markdown")
```

`RemoveMessage(id)` 取出一条消息。对于对话中没有的 id，两个方法都会引发 `onError`。

> **注意——只有读者已经在末尾时，视图才会跟随。** 如果你的操作员向上滚动去重读某些内容，新内容**不会**把他们拽回下面；而是会出现一个安静的指示器，并且在他们滚回到末尾的那一刻，跟随会自己恢复。任何向上的移动——无论多么小，包括触控板的几个像素——都算作离开末尾。

> **注意——在长对话中移动。** 操作员有三种方式触及每一行，从第一行到最后一行：**滚轮**（或触控板的两指滚动）；一旦对话比窗格更高就出现在窗格右边缘的**滚动条**——拖动它的滑块，或点击轨道以移动一页；以及，在指针位于对话之上时，**方向键**、**Page Up / Page Down**、**Home** 和 **End** 键。当另一个控件——问题框——拥有插入符时，这些键会退让。在文本上拖动仍然是*选中*它以便复制；它从不滚动。没有需要横向滚动的情形：代码会换行，表格适应窗格，而图像向下缩放到适合它。

> ⚠️ **注意事项——对于不是你写的内容，把 `RenderAsHtml` 设为零。** 它是一个总开关：关闭后，*每一次*追加都被视为原始内容，不论你调用的是哪个方法。如果回复来自你无法控制的地方，请在开始时说明一次，然后不必再为每次调用担心。

##### 对话，以及你自己构建的侧边栏

Viewer 不绘制聊天侧边栏、“新聊天”按钮或历史列表。这是有意为之：你的应用程序的外观应当看起来像你的应用程序。你得到的是机制，而你用你已经熟悉的控件来构建按钮。在应用外壳中，这份历史的自然归宿是侧边栏本身：见应用外壳一章中的**你的程序添加的行**。

| 方法或属性 | 它做什么 |
|---|---|
| `NewConversation()` | 把打开的对话归入历史，清空窗格，引发 `onConversationCreated`。在已经为空的窗格上，它什么都不做 |
| `SelectConversation(id)` | 把打开的那个归档，使 `id` 成为当前对话，清空窗格，引发 `onConversationSelected` |
| `RegisterConversation(id, title)` | 为早先一次运行的对话播种一个历史条目 |
| `HistoryList` | 最多十个条目，每行一个 `id|title` |

那么，最小的聊天机器人窗体就是一个 Viewer、一个 ListBox 和一个按钮：

```cobol
      *> The developer's own "New chat" button.
       PROGRAM-ID. BTN-NEW--ONCLICK.
       PROCEDURE DIVISION.
           MOVE "CHAT-0042" TO VWR-1::ConversationId
           INVOKE VWR-1::NewConversation()
           .

      *> Whenever history changes, refill your own list.
       PROGRAM-ID. VWR-1--ONCONVERSATIONCREATED.
       PROCEDURE DIVISION.
           MOVE VWR-1::HistoryList TO LST-HISTORY::Items
           .

      *> The operator picks a past conversation from your list. Keep the
      *> ids in a table of your own as you fill the list - the list shows
      *> titles, your program remembers which id each line came from.
       PROGRAM-ID. LST-HISTORY--ONSELECTEDINDEXCHANGED.
       PROCEDURE DIVISION.
           MOVE LST-HISTORY::SelectedIndex TO WS-ROW
           IF WS-ROW > 0
               MOVE WS-CHAT-ID-TABLE(WS-ROW) TO WS-CHAT-ID
               INVOKE VWR-1::SelectConversation(WS-CHAT-ID)
           END-IF
           .

      *> ... and the Viewer asks YOU for that conversation's content.
       PROGRAM-ID. VWR-1--ONCONVERSATIONSELECTED.
       PROCEDURE DIVISION.
           MOVE VWR-1::ConversationId TO WS-CHAT-ID
           PERFORM READ-CHAT-FROM-FILE
           PERFORM UNTIL WS-EOF = "Y"
               INVOKE VWR-1::AppendMarkdown(WS-CHAT-LINE)
               PERFORM READ-NEXT-CHAT-LINE
           END-PERFORM
           .
```

> ⚠️ **注意事项——历史保存的是名称，绝不是对话。** 十个条目，每个只有 id 和标题，别无其他。选择一个不会恢复任何东西：它通过 `onConversationSelected` **向你的程序索要**那个对话，而你的程序从它保存对话的任何地方把它读回来。这正是让一个整天运行的会话不会无限增长的原因——也意味着对话的存储由你来选择，通常是索引文件。

📷 需要截图 — `viewer-split-pdf.png`。在窗体上放一个 Viewer，把 `SplitMode` 设为 `LeftRight`，在左视图中打开一个多页 PDF，在右视图中打开同一个 PDF，把两者滚动到不同的页面，并在其中一个中打开 Find 栏——使分隔线、两个独立的滚动位置和每个视图的 Find 栏同时都可见。

📷 需要截图 — `viewer-streamed-chat.png`。构建上面的小型聊天机器人窗体（一个 `Layout = Streamed` 的 Viewer、一个过往对话的 ListBox 和一个“New chat”按钮），追加三四条消息使对话充满窗格，并捕获整个窗口——让读者能看到侧边栏是开发者自己的，而 Viewer 不绘制它自己的任何外框。

### 版本 2 中的新控件：Hyperlink、Authentication、Canvas2D 和 Canvas3D

**Hyperlink** 是打开一个地址的链接文本，如同 HTML 的 `<a>`：网页、电子邮件（`mailto:`）、电话号码（`tel:`、`sms:`）、下载，或窗体上的 `#name`。`Caption` 是友好的文字，`Url` 是地址。在浏览器中它**就是**锚点——Ctrl/⌘-点击、中键点击和上下文菜单的工作方式与任何网站相同；在桌面窗体上，点击会打开系统的默认浏览器，不论 `Target` 是什么。它是一个 Tab 停靠点，而 Enter 会跟随它。它具有 Label 的属性、事件和方法，外加链接自己的：

| 属性 | 它做什么 |
|---|---|
| `Url` | 地址：`http`、`https`、`mailto`（带 `subject`、`body`、`cc`、`bcc`）、`tel`、`sms`、相对地址或本窗体上的 `#name`。`javascript:`、`vbscript:` 和 `data:` **总是**被拒绝（`data:` 只对 `Download` 例外）；任何其他方案只有在项目于 `[links] schemes` 之下列出它时才允许。被拒绝的地址按所写保留，在 Properties 面板中以红色显示，而链接保持惰性 |
| `Target` | `Self`（默认）、`NewTab`、`NewWindow`、`Parent`、`Top` |
| `WindowFeatures`、`Rel`、`Download`、`HrefLang`、`MimeType`、`ReferrerPolicy` | 锚点自己的属性。在新标签页或新窗口中，空的 `Rel` 表示 `noopener noreferrer` |
| `LinkUnderline` | `Always`（默认）、`Hover` 或 `Never` |
| `LinkColor`、`HoverColor`、`ActiveColor`、`VisitedColor` | 链接的颜色；空表示主题的 |
| `Cancel` | 仅限运行时：在 `onClick` 或 `onBeforeNavigate` 中设置它，点击就不会导航 |

点击会引发 `onClick`，然后是 `onBeforeNavigate`，然后导航，而在应用程序仍然在场时随后有 `onNavigated`。`SetUrl( address )` 在运行时更改地址（被拒绝的地址让 `Url` 保持不变，并在 `LastError` 中说明原因）；`Open()` 和 `OpenIn( target )` 的行为就像一次普通点击。

```cobol
      *> onBeforeNavigate of LNK-1: do not leave a form with unsaved work.
           IF WS-UNSAVED = 1
               SET LNK-1::Cancel TO TRUE
           END-IF
```

**Authentication** 让应用程序的最终用户登录、注册和退出，**仅限 Web 或 Mobile 窗体**——第 24 章，*让用户登录*，有完整的说明：提供商（Cognito、Keycloak、Entra、Entra External ID）在 Backend 的 `[identity]` 设置中设置一次，控件通过 Backend 的 `/auth/*` 路由工作，令牌留在播放器中，而你的 COBOL 看到的是 `State`、`UserName`、`Email` 和 `GetClaim( name )`，外加 `SignOut()`、`Refresh()`、`GrantConsent()` 和 `DenyConsent()`，以及事件 `onSignedIn`、`onSignedOut` 和 `onConsentRequired`。

**Canvas2D** 和 **Canvas3D** 是两个绘图控件，旁边还有 **ImageBuffer**、**Plot**、**Scene3D**、**Physics2D** 和 **Physics3D**（非可视）——第 26 章。

---

## 9. 属性

每个控件都暴露**属性**——它的外观、行为和数据绑定——可以在属性窗格中编辑，并存储在 `.cfrm` 文件中。

PowerRustCOBOL 使用**完整拼写的属性名**（没有晦涩的缩写）。有几个你会经常用到：


| 属性                                 | 含义                                                                   |
| ---------------------------------------- | ------------------------------------------------------------------------- |
| `Caption` / `Text`                       | 控件的文字（标签/按钮用 `Caption`；文本框用 `Text`）。 |
| `BackgroundColor` / `ForegroundColor`    | 颜色（十六进制，例如 `#1E3A5F`）。                                             |
| `FontName`, `FontSize`, `Bold`, `Italic`, `Underline`, `Strikethrough` | 排版：标题、列表条目、网格单元格、GroupBox 的图例，以及 TextBox 中键入时的文字。 |
| `Visible`, `Enabled`                     | 状态。                                                                    |
| `TextAlignment`                          | 文字对齐。                                                       |
| `Padding`                                | 框架与内容之间的额外空间（0–128 点）；TextBox 把它加到它的 `InnerPadding` 上。 |
| `Tooltip`                                | 指针停在控件上时的弹出文字——每个可视控件都会显示它。大多数等待控件的 `HoverDelayMs`（除非你设置，否则是 200 ms）；Button、TextBox、ComboBox 和 CheckBox 不论 `HoverDelayMs` 怎么说，都在半秒之后显示它。 |
| `ZOrder`                                 | 堆叠顺序；`SET ctl::ZOrder`、`BringToFront` 和 `SendToBack` 在窗体运行时更改它。 |

> **把控件绑定到数据**是在窗体上、在 **Data Binding** 面板中完成的，而不是通过属性。较旧的窗体的控件上可能仍然带有 `DataItem` 和 `DataFormat`；它们原样加载，没有任何东西读取它们。

有几个属性的效果取决于控件：

| 控件 | 属性 | 它做什么 |
|---|---|---|
| Label | `AutoSize` | Label 采用它的标题的大小，以它的左上角为锚点。在设计器中你键入时它会调整大小，而当 COBOL 更改标题时也会再次调整。 |
| Label | `WordWrap` | 开：标题在 Label 的宽度处换行。关（默认）：它保持自己的行，并缩小字号以适应。 |
| TextBox | `WordWrap` | 多行框。开（默认）：长行换行。关：它们保持完整，而框横向滚动。 |
| CheckBox, RadioButton | `CheckAlignment` | `Right` 把方框或圆圈放在标题之后。 |
| CheckBox, RadioButton | `CheckSpacing` | 方框（或圆圈）与标题之间的点数，0–64；除非你设置，否则是 6。 |
| RadioButton | `CheckBoxColor` | 圆圈在两种状态下的填充。一个形成对比的圆点标记被选中的单选按钮。 |
| PictureBox | `SizeMode` | `Normal`：图像自己的大小，只在放不下时才缩小。`Zoom`：能放下的最大尺寸。`Stretch`：填满框。`CenterImage`：居中。`AutoSize`：控件采用图像的大小。 |
| PictureBox | `ImageAlignment` | 当图像没有填满框时它所在的位置（`Normal`、`Zoom`）：`TopLeft` … `BottomRight`。 |
| Shape | `FillStyle` | `Solid`、`None`，或 `Hatched`：在透明表面之上用 `FillColor` 画的斜线。 |
| Panel, GroupBox, TabControl | `HScroll`, `VScroll` | 子项在越过内容区域时在其中滚动。 |

> **注意。** 标准缩写被保留（`CSV`、`URL`、`API`、`TLS`）；其他一切都写全——例如 `BackgroundColor`（不是 `BackColor`）、`MaximumLength`（不是 `MaxLength`）和 `PasswordCharacter`（不是 `PasswordChar`）。

> **Caption 规则。** 只有 Label、Button、CheckBox、RadioButton 和 GroupBox 使用 `Caption`；TextBox 使用 `Text`；其他控件使用特定类型的键（`Value`、`Items`，……）。

> **空的 Caption 保持为空。** 清除按钮的 `Caption`，使它成为仅有图像的按钮（只有一个 `IconPath`，别无其他）：图像于是占有整个表面。清空的 Caption 在窗体被保存并再次打开时得以保留。在 1.70.221 之前，它在打开时丢失，而按钮回来时带着它自己的 id 作为标签，这个标签还会把图像挤出去。在 1.70.238 之前，图像本身也没有被绘制：仅有图像的按钮显示一个空的表面，在设计器中和运行时都是如此。
>
> ⚠️ **图标恰好按 `IconSize` × `IconSize` 绘制。** `IconSize` 是一个数字，即正方形的边长，而图像被缩放以填满它——它自己的比例不会保留。提供形状正确的图像是你的工作：画成 32 × 32 图标的 3:2 国旗会被压扁。请用透明色把非正方形的图片填充到正方形的画布上（72 × 48 的国旗变成 72 × 72，上下各有一条透明带），这样它在任何 `IconSize` 下都保持它的形状。请让按钮在每个方向上至少有 `IconSize` + 2 × `IconPadding`：放不下的一边会被挤压到剩下的空间，这会让图标变形。

> **Label 的文字可以被选中和复制。** 在运行时，Label 的 `Caption` 是活的文本，而不是文字的图片：操作员在它上面拖动来选中，而 `Cmd`/`Ctrl`+`C` 把选中的内容放到剪贴板上。从一个 Label 开始、在另一个上结束的拖动会把两者都包括进来，所以数字可以连同命名它的标题一起被复制。没有什么需要打开的——没有属性，也不需要写 COBOL。
>
> 从 PowerCOBOL 或 isCOBOL 过来，你会期望静态文本控件是惰性的，而这是 PowerRustCOBOL 改为遵循现代桌面的地方之一。Label 的其他一切都没有改变：绑定了 `onClick` 的仍然会触发它，`TAB` 仍然越过标签走向你设计的控件，而在设计画布上，拖动仍然是移动控件，而不是选中它的文字。
>
> **注意。** 在 Label 的文字可以被选中之前，可复制的文字意味着设置了 `ReadOnly` 的 TextBox。那仍然有效，并且当文字是操作员以后可能想要更正的*值*时，它仍然是正确的控件——但它不再是你仅仅为了让某人复制一个标题而去用的东西。

> **始终读得出来的文字。** 窗体不知道它的主题绘制什么，所以承载含义的颜色会针对它们所落在的表面进行检查：CheckBox 或 RadioButton 的标题、CheckBox 的 `CheckColor` 勾号、ListBox 的条目，以及文本插入符。只要它在那个表面上保持可读，你的颜色就完全按设置使用；在它不可读的地方，画笔回落到黑色或白色——哪个读得出来用哪个。这就是为什么当你把深色主题换成浅色主题而不触碰任何属性时，同一个窗体仍然可用。要绝对钉住一个颜色，请选择一个在你交付的主题上读得出来的。
>
> **每一个是针对哪个表面度量的。** 文字真正落在的那一个。CheckBox 有两个表面（见下文）：标题位于**框架**上，并针对 `BackgroundColor` 检查，而 `CheckColor` 勾号位于**方框**之内，并针对 `CheckBoxColor` 检查。因此给复选框一个深色的框架颜色不再会把它的勾号变白，而给方框着色也不再会把标题变白。
>
> **透明的框架留给你。** 一旦 `Transparency` 高于 70，框架绘制得太少而无法读取，而标题真正坐落的东西——窗体、GroupBox、背景图像——不是控件能看到的。那里什么都不度量，你的 `ForegroundColor` 完全按设置使用。CheckBox 默认是 100 % 透明的，所以这是常见情形：请挑选一个在你放置它的窗体上读得出来的标题颜色。
>
> CheckBox 的标题位于它的方框的右边，而 RadioButton 的标题位于它的选择圆圈的右边，两者距离相同。

> **单选按钮在每个主题上都是圆圈**——被选中时填充，未被选中时是空的边缘。它是绘制的，而不是键入的：早先的版本在除 Elegance 之外的每个主题上，都把 `(●)` 或 `( )` 放进标题，这就是为什么没有东西可以着色。
>
> 在主题描述了它自己的切换外观的地方，由该主题给它着色——Elegance 绘制你在它自己窗体中看到的绿色。在其他每个地方，圆圈采用控件的 **`CheckColor`**（给 CheckBox 的勾号着色的同一个属性；单选按钮的圆点就是那个勾号），而如果你想要，**`CheckBoxColor`** 设置圆圈的表面。未选中的圆圈的边缘，是针对你把控件放在其上的任何东西按**对比度**来选择的，所以它在深色窗体和浅色卡片上都可见，而不必被告知。
>
> ⚠️ **注意事项。** 单选按钮现在需要给那个圆圈留出空间，所以**新放下**的一个是 140 点宽而不是 120——足以在预置的字体下容纳它自己的标题。你已经保存的窗体保持它们被给定的宽度；没有任何东西在你不知情的情况下移动。

> **单选按钮是 `Selected`；复选框是 `Checked`（1.62.131）。** 属性网格过去给 RadioButton 提供一个 `Checked` 属性——CheckBox 的词。RadioButton 现在带有 **`Selected`**；CheckBox 和 Switch 保持 **`Checked`**，没有改变。
>
> ```cobol
> SET RADIO-CREDIT::SELECTED TO 1
> IF RADIO-CREDIT::SELECTED = 1
>     PERFORM CHARGE-THE-CARD
> END-IF
> ```
>
> **你已经写的任何东西都不会坏。** 这两种写法在运行时相互解析，所以写着 `RADIO-CREDIT::CHECKED` 的处理程序继续工作，而 `ISCHECKED` / `SETCHECKED` 仍然与 `ISSELECTED` / `SETSELECTED` 并列响应。在重命名之前保存的窗体会在加载时升级：旧键被重命名，它的值被保留。在新代码中请优先使用 `Selected`——它是属性网格所显示的，也是生成的代码所写的。

> **CheckBox 有两个表面，每个都有它自己的属性。** 从 PowerCOBOL 或 isCOBOL 过来，你会期望一个背景和一个边框；在这里，勾选方框本身就是一个表面，所以每样东西都有两个。一个属性指的是哪一个，从不取决于控件：
>
>
> | 表面   | 它是什么                                                              | 它的属性                                                                        |
> | --------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
> | **Frame** | 标题*和*方框后面的卡片——整个控件矩形 | `BackgroundColor`（或渐变对）、`BorderStyle`、`BorderColor`、`BorderWidth` |
> | **Box**   | 勾选方块本身——RadioButton 的选择圆圈              | `CheckBoxColor`、`CheckBoxBorderStyle`、`CheckBoxBorderColor`、`CheckBoxBorderWidth`  |
>
> `CheckColor` 和 `CheckSize` 保持它们一直以来的样子：画在方框*之内*的勾号，以及它填满方框的多少。
>
> 因此 `BackgroundColor` 在 CheckBox 上的含义，恰如它在 Label、TextBox 或 Panel 上的含义——控件自己的表面。复选框起初是 100 % 透明的，它的 `BorderStyle` 起初是 `None`，所以在你要求之前，框架完全什么都不显示；与此同时，方框起初的 `CheckBoxColor` 为空，这让它穿着当前主题所绘制的任何东西。指名一个颜色，你的就领先。
>
> ```cobol
>     MOVE "#1E3A5F" TO CHK-AGREE::BackgroundColor
>     MOVE "Single"  TO CHK-AGREE::BorderStyle
>     MOVE "#FFFFFF" TO CHK-AGREE::CheckBoxColor
> ```
>
> ⚠️ **注意事项。** 边框和表面是两个独立的决定。无框架的控件——保持透明的 CheckBox、没有背景的 Label——仍然会画你所要求的边框，画在什么都没有的地方之上。这是刻意的：`BorderStyle` 在此之前对这两者完全没有效果，那才是更令人意外的行为。

> **控件 ID。** 当你放下一个控件时，它得到一个可读的、按类型的 ID——`Button-1`、`Button-2`、`TextBox-1`、`ComboBox-1`，……——它成为它的 COBOL 数据名（`WS-BUTTON-1`）以及它的嵌套事件处理程序（`BUTTON-1--ONCLICK`）的基础。你可以在属性窗格中把控件的 ID 重命名为有意义的名称（例如 `BTN-SAVE`）；请让它保持为有效的 COBOL 字（字母、数字、连字符；没有开头/结尾的连字符）。

### 倾斜显示控件：3D 透视

图表、图片、标签、线条或形状可以在空间中转动，就像你抬起一条边时桌子上的一张卡片那样：远去的那条边看起来更短，保持靠近的那条边看起来更长，而整个东西汇聚起来。四个属性使它倾斜，另外八个让你把**它的角钉**在任何你喜欢的地方；控件的其他方面都没有改变。

| 属性 | 它做什么 | 取值 | 默认 |
|---|---|---|---|
| `RotateX` | 使控件绕它的**水平**轴倾斜，于是它的上边或下边远去。 | 整数度，-60 到 60 | 0 |
| `RotateY` | 使控件绕它的**垂直**轴倾斜，于是它的左边或右边远去。 | 整数度，-60 到 60 | 0 |
| `PerspectiveDistance` | 控件离观察者有多远：它被画得多大，以及倾斜汇聚得多强烈。 | 整数，0（最远）到 100（最近） | 50 |
| `PerspectiveStrength` | 倾斜汇聚得多强烈，**与大小无关**。100 是距离所给出的汇聚；更低则使它变平；0 是没有。 | 整数，0 到 100 | 100 |
| `PinTopLeftX`, `PinTopLeftY`, `PinTopRightX`, `PinTopRightY`, `PinBottomRightX`, `PinBottomRightY`, `PinBottomLeftX`, `PinBottomLeftY` | 轮廓的每个角从倾斜把它放到的地方移动了多远——**角钉**。 | 整数像素，-9999 到 9999 | 0 |

哪条边远去取决于符号：

| 值 | 远去的边 |
|---|---|
| `RotateX` 大于 0 | **上**边 |
| `RotateX` 小于 0 | **下**边 |
| `RotateY` 大于 0 | **右**边 |
| `RotateY` 小于 0 | **左**边 |

这十二个属性出现在属性窗格中它们自己的 **3D perspective** 部分，位于这十二个控件上：`BarChart`、`LineChart`、`PieChart`、`AreaChart`、`ScatterChart`、`DonutChart`、`RadarChart`、`PictureBox`、`Animator`、`Label`、`Line` 和 `Shape`。它们只*绘制*，这就是它们可以被转动的原因。接受输入的控件——TextBox、Button、ComboBox、Slider、DataGrid 等等——或容纳其他控件的控件——Panel、GroupBox、TabControl——永远没有它们。

![一个“Revenue by quarter”条形图，用 RotateY -30、RotateX 8 和 PerspectiveDistance 60 倾斜，位于深色窗体之上](../assets/images/perspective-revenue.png)

*上面的图表是一个 BarChart，`RotateY` = -30（它的左边远去）、`RotateX` = 8 和 `PerspectiveDistance` = 60。*

**从程序中移动它。** 这三个属性与其他任何属性一样被读取和写入，所以处理程序可以摆动图表或把它拉近：

```cobol
           *> Turn the revenue chart so its left side goes away, a little downhill,
           *> and bring it forward.
           MOVE -30 TO CHART-REVENUE::RotateY
           MOVE   8 TO CHART-REVENUE::RotateX
           MOVE  60 TO CHART-REVENUE::PerspectiveDistance
```

Timer 可以一次一度地步进角度，做缓慢的摆动：

```cobol
       TIMER-SWING--ONTICK.
           IF CHART-REVENUE::RotateY < 60
              ADD 1 TO CHART-REVENUE::RotateY
           END-IF.
```

超出范围的值从不是错误：它被带回范围之内，而你读回的是控件现在所拥有的（`MOVE 200 TO CHART-REVENUE::RotateX`，然后 `DISPLAY CHART-REVENUE::RotateX`，显示 60；`MOVE 150 TO CHART-REVENUE::PerspectiveStrength` 显示 100）。

**距离做什么。** `PerspectiveDistance` 是控件离观看窗体的人有多远。在 50 这个参考值时，未倾斜的控件按你设计的大小绘制。把它拉近，它就被画得更大——在 100 时是两倍大——并且倾斜汇聚得更强烈；把它送远，它就被画得更小——在 0 时是一半大小——也更平，朝它自己的中心收缩。相等的步长感觉相等：25 比 50 小多少，75 就比 50 大多少。

**强度做什么。** 距离同时改变大小*和*汇聚，所以单靠它无法让图表保持原来的大小而缓和它的透视。`PerspectiveStrength` 恰好只做这件事。在 100（默认）时，倾斜汇聚得与距离所造成的一样多：近边画得比远边长得多。把它降低，汇聚就变平，而控件保持它的大小；在 0 时，相对的边保持相等，所以倾斜只是转动控件，完全没有汇聚。它对未倾斜的控件什么都不做。

![三个倾斜相同（RotateY -38、RotateX 8、PerspectiveDistance 65）、PerspectiveStrength 分别为 100、50 和 0 的图表](../assets/images/perspective-strength.png)

*同一个图表，倾斜和距离相同，强度分别为 100、50 和 0。远侧从近侧的 0.68 增长到 0.83，然后相等。*

```cobol
           *> Keep the chart the size it is, but soften the perspective so it fits the panel.
           MOVE 40 TO CHART-REVENUE::PerspectiveStrength
```

**钉住各个角。** 倾斜始终是正面看到的矩形：它的轮廓关于穿过它的中心的水平线是对称的。背景图片中的面板很少是这样的——它被旋转了几度，或朝一侧汇聚——而没有任何倾斜、距离或强度能落到它上面。**角钉**可以：轮廓的四个角中的每一个都可以按它自己的 `Pin…X` / `Pin…Y` 移动，以像素计，从倾斜把它放到的地方开始，而控件以那个把它的矩形恰好送到这四个角上的透视来绘制。直线保持为直线，图片的各角落在轮廓的各角上，而轮廓可以是**任何凸四边形**。钉子在有倾斜和没有倾斜时都起作用。

在设计画布上，你不需要键入它们。一旦控件被倾斜、缩放或钉住，**琥珀色的把手**就会出现在它的轮廓的每个角上，位于保持在平坦矩形上的白色调整大小旋钮旁边：把角把手拖到你图片中面板的角上，那个角的两个钉子就会替你写好（每次拖动一个撤销步骤）。

![一个条形图被钉进一个顺时针旋转并向左汇聚的面板中；它后面的玻璃面板是以同样方式钉住的 Shape](../assets/images/perspective-pin.png)

*一个图表和它后面的 Shape，两者都以 `RotateY` -12 倾斜，并以相同的偏移钉住——左上 (0, -8)、右上 (0, 20)、右下 (18, 18)、左下 (-10, 0)——钉进一个上边和下边都向右下倾斜的面板中，就像图片中画的面板那样。单靠倾斜无法这样旋转控件。*

```cobol
           *> Drop the chart's bottom-right corner 18 px down and 18 px to the right,
           *> onto the corner of the panel in the picture.
           MOVE 18 TO CHART-REVENUE::PinBottomRightX
           MOVE 18 TO CHART-REVENUE::PinBottomRightY
```

> **说明。**
> - 倾斜绕控件矩形的**中心**转动。控件保持它在窗体布局、它的 Tab 顺序和它的事件绑定中的位置；只有它*如何绘制*改变了，所以倾斜从不移动它的邻居。
> - 控件在堆叠顺序中的位置与以往相同，并被同一个矩形所裁剪：在 Panel 之内，它不能溢出 Panel 的内容区域，在图层上它不能溢出窗体，即使把它拉近本来会让它比两者都更大。
> - 倾斜控件的点击、悬停、双击和其他指针事件，发生在**你看到它的地方**：在它的轮廓之内，即使那超出了你设计的矩形，而不是在它不再覆盖的空地上。它的 `Cursor` 和 `Tooltip` 遵循同一个轮廓。
> - 会让轮廓**折叠**的钉子——一个角被拖过了形状、绕向被反转、某条边短于 2 px，或远侧比近侧的二十分之一还薄——会被拒绝：控件只以它的倾斜绘制，而在画布上，把手只是在那里停止跟随指针。
> - 倾斜的 Label 随倾斜绘制它的标题，所以文字不再是读者可以选中和复制的东西。它仍然引发 `onClick` 和 `onGotFocus`，而 Tab 仍然能到达它。
> - 带有动画帧的 Animator 或 PictureBox 在被倾斜时继续播放。
> - 从程序写入这些属性，在 Run Form、加载到应用程序内容窗格中的窗体以及构建出的程序中都以同样方式工作。
> - 在这些属性存在之前保存的窗体打开时，控件是平的，并处于参考距离，与它原来看起来的完全一样。

> ⚠️ **注意事项。**
> - **在 Form Designer 中，选择保持在你设计的矩形上。** 白色旋钮、移动和调整大小作用于那个平坦的矩形，所以控件总是可以被抓住；倾斜的形状在它之内和周围实时绘制，而琥珀色的角把手位于你看到的形状上。平坦、未钉住的控件不显示琥珀色把手（它们会位于旋钮之下）：倾斜它，或在窗格中键入一个钉子，它们就会出现。
> - **极端值很强烈。** 在 -60 或 60、`PerspectiveDistance` 为 100 时，近边画得比远边长三倍多。那是“非常近”，它会在容器处被裁剪，但很少是仪表盘想要的；在 50 到 70 的距离下 10 到 35 度，读起来就像桌子上的一张卡片。
> - **两个倾斜的控件不按深度排序。** 它们按它们的堆叠顺序绘制，一个在另一个之上，即使它们在空间中会交叉。
> - **没有厚度。** 控件是一张平坦的卡片，除了它自己的 `ShadowEnabled` 之外，没有侧面、光照或阴影。
> - 动画效果正在移动或缩放的控件，是在效果移动它之后，绕它的新中心倾斜的。
> - 倾斜控件的绘制成本高于保持平坦——图表每一帧都被重新投影——所以请让有许多大型倾斜图表的窗体保持适度。保持默认值的控件完全没有成本。

### 窗体主题与样式

**主题**让你的窗体有一种独特的外观，而不必手工给每个控件设置样式。主题由设计器、预览、Run Form 和编译出的应用程序都在使用的同一个渲染器应用（按 spec 017 的 `cobolt-forms` 统一渲染引擎），所以带主题的窗体在每个地方看起来都一样。

**Theme** 下拉列表（在窗体 *Appearance* 中）现在选择程序化的表面样式：

- **Classic**——最初的磨砂玻璃外观。
- **Enhanced**——增加内描边、高光带、微噪点和结构性状态（完整的 Liquid Glass 配方）。
- **Neumorphic**——100 % 程序化的柔和 UI “黏土”/挤出浮雕（没有图像）。光从左上方来。低对比度、大圆角、柔和的分层阴影（高光在左上，阴影在右下）、细微的内缘，以及一个可选的额外的三边着色边框（右上 → 右下 → 左下），它遵循控件的 `CornerRadius`。

资产包“皮肤”（来自 `assets/themes/<id>/` 的 9-slice PNG）仍然受支持，用于完全照片级真实的外观，并且可以在项目层面组合；选择程序化样式会清除该窗体的任何按窗体的包覆盖。

**选择。**

- 项目默认：*Settings ▸ Appearance ▸ Default form theme*。
- 按窗体：Designer 窗体的 *Appearance ▸ Theme*（或留空以继承）。
- 创建时：*File ▸ New Form ▸ Theme*，它列出同样的目录，并默认继承项目的。

解析：按窗体 → 项目默认 → Classic/Liquid Glass。

自己的 Theme 未设置的窗体，显示被继承的那个，并标记为 **(from project)**，所以选择器所报告的，总是窗体实际使用的渲染。

**Use theme background**，即 Theme 之下的复选框，让带有自己背景图案的主题替换窗体的背景图像——在设计器中和窗体运行时都是如此。默认关闭；窗体保留它自己的背景。

#### Elegance

**Elegance** 是第二个内置主题，从与 Liquid Glass 和任何已安装的包相同的 Theme 下拉列表中选择。Liquid Glass 是半透明和磨砂的，而 Elegance 是**扁平且不透明的**：深石板色的表面、每个控件上的一条细边框，以及一种一致地用于按钮、选择和焦点的冷色强调色。它适合业务窗体——密集的数据录入、网格、仪表盘——在那里磨砂面板与数据争夺注意力。

选择它与选择任何其他主题没有区别：

```text
整个项目   Settings ▸ Appearance ▸ Default form theme ▸ Elegance
仅一个窗体  Designer ▸ form Appearance ▸ Theme ▸ Elegance
```

窗体上的一切立刻采用该主题——面板和分组框、按钮、文本框、复选框和单选按钮、列表和组合框、滑块、进度条、标签页、菜单/工具/状态栏、树视图、数据网格、全部六种图表类型，以及旋钮、仪表、开关和文件放置控件。图表用主题的强调色系绘制它们的系列，而不是内置的颜色，所以图表坐落在窗体之内，而不是在它之上。

#### Spatial

**Spatial** 是第三个内置主题，用于应当看起来像桌面之上浮动的玻璃面板的应用程序。它的表面是暖灰色半透明玻璃，配白色文字、大圆角和柔和的、不饱和的强调色。它是唯一**透明**的主题：

- **窗口是透明的。** 桌面透过窗体显示出来，而操作系统把它模糊，就像磨砂玻璃后面的房间被柔化那样。这在 macOS 和 Windows 上有效，在桌面提供它的 Linux 上也有效（Wayland 上的 KDE）。在其他地方，桌面透过来但没有模糊。
- **窗体的背景是主题的玻璃。** 窗体上的纯色 *Back color*、背景渐变或背景图片会遮住桌面，所以在 Spatial 之下它们被搁置。选择另一个主题，它们就原样回来。
- **请在较暗的桌面上阅读它。** 白色文字在深色或色彩丰富的壁纸上保持清晰；在非常明亮的壁纸上它会褪色。
- **文字是针对玻璃选择的，而不是针对窗体的颜色。** 在控件的文字在它所坐落的东西上读不出来的地方，PowerRustCOBOL 会把它变亮或变暗；在 Spatial 之下，它针对深色桌面之上的玻璃来度量——主题就是为它设计的——所以保持默认黑色墨水的 ComboBox 或 TextBox 显示为白色。自己设置 `ForegroundColor`，它就保持不变。
- **ComboBox 的列表可以悬出窗口之外。** 当展开的列表会被 Spatial 窗口的底部裁掉时，它会在方框正下方的一个自己的小窗口中打开，就像原生下拉列表那样；它获得键盘，并且在别处点击会关闭它。在其他每个主题之下，列表留在窗体之内。

```text
整个项目   Settings ▸ Appearance ▸ Default form theme ▸ Spatial
仅一个窗体  Designer ▸ form Appearance ▸ Theme ▸ Spatial
```

**Spatial 外观。** 在 Spatial 窗体上，IDE 给你添加的东西穿上的衣服，就像 PowerDemo3 中 *Buttons* 示例穿的那样：新窗体得到一个暗玻璃窗口（`BackgroundColor` `#40404008`，`Transparency` 50），而你放在它上面的每个控件得到一个深色表面——从 `#4E4E4E` 渐变到黑色，右下方一个柔和的阴影，没有边框，白色文字。按钮、面板和分组框的表面是 70 % 不透明的——设置在它们的颜色中（`#36383EB3`、`#4E4E4EB3` 到 `#000000B3`）——所以玻璃透过它们显示，而它们所容纳的东西保持它们自己的颜色。

**投影落在控件之外。** 阴影绘制在控件周围，而从不在它之下，所以透明的表面显示它后面的窗体——而不是它自己的阴影。这适用于每个 `Transparency` 高于 0、或背景颜色带有 alpha 的控件，上面 Spatial 的表面就是这样做出来的。不透明的控件不变。（在 1.90.8 之前，阴影是画在整个表面之下的，并透过它显示出来，用阴影的颜色给控件的颜色着色。）

**没有自己表面的容器不会挡住阴影。** `HideBackground` 开启的 Panel 或 GroupBox 什么都不绘制；它只是安排它所容纳的东西，就像 `Grid` 或 `Flex` 布局那样。它里面的控件的阴影落到最近的*确实*有表面的容器为止——如果没有，则落到窗口为止——所以紧贴网格边缘的卡片四周都保有它的阴影，一行的最后一个按钮也是如此。控件本身保持它的单元格的大小；只有阴影离开它。有表面的容器，或会滚动的容器（`HScroll` / `VScroll` 开启），是一条边缘：阴影留在它之内。（在 1.90.11 之前，阴影在网格或行的边缘被裁断。）

> ⚠️ **注意事项。** Panel 的 `Transparency` 会淡化它里面的一切，而不只是它的表面：`Transparency` 为 30 的 Panel 中的图片以 70 % 绘制。要透过卡片看、但不透过它的内容看，请改为给它的颜色一个 alpha。

把窗体切换到 Spatial，会让它的控件穿上上面 **Spatial 外观**所描述的样子，而把它切换走，会把那个外观再脱下来——你自己在控件上设置的值在两种情况下都得以保留。要更改 Spatial 对你的项目意味着什么，请使用项目设置中的 **Default Theme Settings** 按钮：项目的表叠加在交付的外观之上。

Spatial 还与 `BorderStyle` `Glow` 搭配：带有默认白色发光的卡片看起来像一条被点亮的玻璃边缘。同样的外观也可用于 IDE 本身，作为 **Spatial** IDE 主题（见*外观与国际化*）。

> 📷 需要截图 — `form-theme-spatial.png`：一个运行中的 Spatial 窗体，位于色彩丰富的桌面之上，它后面可以看到模糊，还有几个按钮、一个文本框和一个带有 `BorderStyle` `Glow` 的面板。

#### 拥有整个外观的主题

有些主题只提供外观的*一部分*，其余的交给 Liquid Glass 来填补。另一些则定义**完整的**外观，并且不希望在它之上叠加任何东西——Elegance 和 Spatial 就是其中两个。主题声明它属于哪一种，而 IDE 在每个地方都遵循这个声明。

对于拥有整个外观的主题：

- **Glass style 行变灰**，并附有说明原因的注释。Classic、Enhanced 和 Neumorphic Light/Dark 是 Liquid Glass 的变体，而扁平的主题没有磨砂，也没有凸起的浮雕供它们变化。提供选择却又忽略它，是令人困惑的地方，所以 IDE 不再提供它。你上一次的选择被记住，并在你回到 Liquid Glass 的那一刻回来。
- **选择它不会改变你的窗体文件中的任何东西。** 挑选主题从不改写你的背景颜色、渐变设置或每个控件的阴影属性，所以来回切换是无损的：你原来的窗体就是你得到的窗体。
- **你自己的属性仍然适用，全部适用。** *Back color*（`BackgroundColor`）、*Fore color*（`ForegroundColor`）、*Corner radius*（`CornerRadius`）、*Transparency*、*Shadow*——你在控件上设置的任何东西都胜过主题。特别是，你打开的投影**会被绘制**，不论是什么主题。

> ⚠️ **注意事项——这在 1.61.37 改变了。** 在那个版本之前，当自包含的主题处于活动状态时选择 Neumorphic Light 或 Neumorphic Dark，会悄悄地抑制窗体上的每个投影，并且可能在扁平表面上绘制凸起的边缘。如果你曾通过把 Glass style 留在 Classic 来绕过它，那个变通办法就不再需要了：阴影现在在所有四种设置下表现相同，因为该设置不再到达主题。

还有两件值得了解的事：

- **你自己的颜色仍然胜出。** 带有明确的 *Back color* 或 *Fore color* 的控件保留它。主题只提供默认值，所以你可以给整个窗体设置主题，并且仍然让一个字段是红色的。
- **Elegance 和 Spatial 拥有整个外观**，所以选中其中任一个时 Glass style 行被禁用——见上文。

Elegance 只是控件主题：它不提供窗体背景，所以窗体自己的 *Back color* / *Background Image* 像以前一样适用。

📷 需要截图 — `elegance-theme.png`
打开一个包含多种控件的窗体（一个带有文本框和组合框的分组框、一个有几行的数据网格、几个按钮和一个图表），把 *Appearance ▸ Theme* 设为 **Elegance**，并捕获设计画布。把同一个窗体在 Theme = Liquid Glass 时捕获为 `liquid-glass-theme.png`，以便两者可以并排展示。

当 **Neumorphic** 处于活动状态时，窗体页面会自动默认采用配方中非常浅的中性背景（#ECEFF4），除非你设置了明确的背景颜色。

**Neumorphic 专有的属性**（只在 Theme = Neumorphic 时出现）：

- **Illum. grad.**——用于左上方照明（高光）效果渐变的两种颜色。
- **Shadow grad.**——用于右下方阴影渐变的两种颜色。
- **Illum. blur** / **Shadow blur**——各自的柔和度/层数。
- **Transparency**——所有浮雕元素的总 alpha（0–100 %）。
- **Distance**——基础的阴影/照明偏移（类似投影距离）。
- **Rim tint**——额外的三边边框的颜色。
- **Rim weight**——该边框的粗细。
- **Rim blur**——额外边框的柔和度（分层偏移）。

这些使用控件的 `CornerRadius`，所以圆角的面板、图表等在 BR/BL 处得到正确的弧形浮雕（并且额外的边缘正确地到达右上和左下的边框交汇处）。照明和阴影效果是用多个扩展的圆角矩形加 alpha 衰减实现的，在没有真正模糊的情况下做出令人信服的柔和。

**带主题的背景和包。** 包可以提供背景 PNG。请使用 *Use theme background*。包还提供图表调色板。带有明确的 Foreground/Background 颜色的控件会覆盖包。

**添加包。** 放入带有 `theme.toml` + 9-slice 图像的 `assets/themes/<id>/`。请参阅 `cobalt-steel` 参考包或 `neumorphic` 示例包。

示例 `theme.toml` 摘录（包是叠加的；程序化的 Neumorphic 不加载图像）：

```toml
id = "my-neumorphic"
display_name = "My Neumorphic"

[controls.panel]
image = "panel/panel_normal_ref.png"
slice = [20, 20, 20, 20]
```

（完整的细节和 9-slice 规则见捆绑的参考包。）

> **Mermaid 图：主题解析**
>
> ```mermaid
> flowchart TD
>     A[窗体 Appearance ▸ Theme] --> B{程序化？}
>     B -->|Classic/Enhanced/Neumorphic| C[draw_neumorphic 或 glass]
>     B -->|包 id| D[来自 assets/themes/id/ 的 9-slice + 调色板]
>     E[项目默认] -->|回落| F[Liquid Glass / Classic]
>     C --> G[统一渲染器]
>     D --> G
>     F --> G
>     G --> H[设计画布 / Preview / Run Form / 二进制]
> ```

---

## 10. 事件驱动的编程

这是 GUI COBOL 的核心，它的工作方式正如你所期望的：窗体坐在一个**事件循环**中等待；当用户做了什么，匹配的**处理程序**就运行。

### 窗体事件循环

```mermaid
sequenceDiagram
    participant U as 用户
    participant W as 窗体窗口
    participant L as 事件循环（你的程序）
    participant H as 事件处理程序<br/>（嵌套的 COBOL 程序）

    Note over L: PERFORM UNTIL quit
    L->>L: COBOL::"WAIT-EVENT"( … )<br/>（阻塞）
    U->>W: 点击 "Say hello" 按钮
    W-->>L: event = (control = "BTN-OK", event = "onClick")
    L->>H: CALL "BTN-OK--ONCLICK"
    H->>H: 你的 COBOL 运行
    H-->>L: GOBACK
    L->>L: 下一次迭代（再次等待）
    U->>W: 关闭窗口
    W-->>L: 发出退出信号
    Note over L: 循环结束 → onClose 运行 → 程序结束
```

用文字说：

1. 生成的程序进入一个循环，并调用内置的 **`COBOL::"WAIT-EVENT"`**，它会阻塞，直到用户与窗体交互。
2. 当事件发生时，运行时交回**哪个控件**和**哪个事件**（例如 `BTN-OK` / `onClick`）。
3. 循环分派到该对的处理程序——一个以控件和事件命名的**嵌套 COBOL-85 程序**（`BTN-OK--ONCLICK`）。
4. 处理程序运行并 `GOBACK`；循环再次等待。
5. 关闭窗口结束循环；窗体的 `onClose` 处理程序最后运行。

### 你可以处理的事件

- **控件事件**遵循 `on` + 动作的约定：`onClick`、`onChange`、`onDoubleClick`、`onMouseEnter`、`onGotFocus`，等等。每个控件暴露对它有意义的那一组（Button 有 `onClick`/`onDblClick`/鼠标事件；TextBox 有 `onChange`/`onKeyPress`/焦点事件；图表有 `onDataChanged`；等等）。
- **窗体事件**——窗口本身支持一组丰富的事件，分为**生命周期、激活与焦点、窗口状态、布局与绘制、鼠标、触摸与指针、滚动、拖放、剪贴板、系统 / OS，以及错误处理**。生命周期的一对 `onLoad`（就在窗口显示之前）和 `onClose`（在它关闭时）为每个窗体预先创建；其余的按需附加。

> **工具提示说明字段是做什么的。** 每个可视控件在指针停在它上面时显示它的 `Tooltip`——用通俗的话告诉用户该键入或选择什么的地方。在设计器中或从 COBOL 设置它，翻译后的界面正是这样更改它的：`MOVE "The address of the model's server" TO TXT-URL::Tooltip`。ComboBox 在它的列表打开时隐藏它，所以它永远不会盖住条目。

> **设计视图中的每个事件都会在运行时触发。** 控件事件通过 *Run Form* 和编译输出中同一个生成的事件循环处理，按家族分组：
>
> - **每个可视控件**都获得通用的指针事件集——`onClick`、`onDblClick`/`onDoubleClick`、`onRightClick`、`onMiddleClick`、`onContextMenu`、`onMouseDown`、`onMouseUp`、`onMouseMove`、`onMouseEnter`、`onMouseLeave`、`onMouseWheel`、`onHoverEnter`、`onHoverLeave`（在控件的 `HoverDelayMs` 之后，默认 200 ms），以及 `onLoad`——外加**几何**事件集 `onResize`/`onResized` 和 `onMove`/`onMoved`，以及**状态**的一对 `onVisibleChanged`/`onEnabledChanged`。
> - **可获得焦点的控件**（Button、CheckBox、RadioButton、Slider、NumericUpDown、DateTimePicker、TextBox……）在拥有焦点期间触发 `onGotFocus`/`onLostFocus` 以及键盘事件集 `onKeyDown`/`onKeyUp`/`onKeyPress`、`onEnterPressed`、`onEscapePressed`。
> - **值控件**触发 `onChange` 以及它们的语义别名：`onCheckedChanged`/`onValueChanged`（复选框 / 单选按钮）、`onSelectedIndexChanged` 和 `onItemDoubleClick`（列表）、组合框的 `onDropDown`/`onDropDownClosed`、拖动结束时 Slider 的 `onValueChanged`，以及 COBOL 写入它的 Value 时 ProgressBar 的 `onValueChanged`/`onCompleted`。
> - **文本输入**另外触发 `onEnter`/`onLeave` 和 `onTextChanged`。
> - **容器与复合控件**——TabControl 的 `onTabClick`/`onTabChanged`；TreeView 的 `onNodeClick`/`onNodeSelect`/`onNodeDblClick`；Panel 的 `onScroll`（AutoScroll）；MenuBar 的 `onMenuOpen`/`onMenuClose`；DataGrid 的 `onCellClick`/`onCellDoubleClick`/`onRowDoubleClick`/`onColumnClick`/`onScroll` 加上它的选择事件、对 Content 为 Button 的列的 `onRowButtonClick`，以及——在编辑并写回的网格上——`onCellEdited`、`onSaved` 和 `onCommitError`。
> - **媒体与图表**——PictureBox 的 `onImageLoaded`/`onImageError`；Animator 的 `onStarted`/`onFrameChanged`/`onLooped`/`onEnded`；图表在其数据属性更改时的 `onDataChanged`。
> - **数据控件**——SqlDatabase 在 `Open` 时触发 `onConnectOk`/`onConnectError`，在 `Query`/`Execute` 时触发 `onQueryComplete`/`onQueryError`，在 `Fetch` 时触发 `onRowFetched`；RestClient 触发异步生命周期（`onComplete`/`onError`/`onCancelled`/`onTimeout`——§16）；AI 智能体在 `Ask` 返回回复时触发 `onResponse`。这些在下一次 `COBOL::"WAIT-EVENT"` 返回时分派。
> - **Timer** 在启用期间每个 `Interval` ms 触发 `onTick`（`Start`/`Stop`）。**`Enabled` 是定时器自己的开关**——它决定定时器是否运行，而不是控件是否变灰。对于等待被启动的定时器，请在属性窗格中取消勾选 **Enabled at start**，并从 COBOL 用 `SET Timer-1::Enabled TO 1` / `TO 0` 把它打开和关闭。（在 1.61.164 之前，两者都不起作用：二者都写的是通用的控件标志，而定时器不读取它，所以定时器根本无法被停止。）Timer 保持**稳定的节奏**：每个节拍安排下一个，间隔一个周期，所以速率不会随帧碰巧如何落下而漂移。它也从不**补偿**错过的时间——如果你的处理程序花的时间超过间隔，或窗体停滞了，它回来时你得到的是一个节拍，而不是你错过的那些的一阵连发。落后很多的处理程序（排了八个事件）会把它的节拍合并，直到它赶上；点击、编辑或焦点更改从不被合并。
> - **窗体级**触发 `onLoad`/`onClose`（启动时 / 关闭时）、`onShow`/`onActivate`（运行窗口第一次出现时）和 `onResize`（当它的大小改变时：窗口被拖动期间 `onResizing` 重复触发，稳定下来时 `onResize` 触发一次）——持有 `SideMenu` 的窗体也能听到它们，因为它的窗口就是外壳。它在那里的大小是它的内容窗格加上导轨的设计宽度——它的控件被设计时所用的坐标——所以折叠或打开导轨（它会调整窗口大小以保持窗格不变）不算调整大小。两者都发现新的大小已经在窗体的 `Width` 和 `Height` 中，所以处理程序据此排布：
>
>   ```cobol
>       *> CHAT-FORM onResize: the chat grows with the window,
>       *> the question box stays 24 px above the bottom edge.
>           COMPUTE WS-INPUT-Y = CHAT-FORM::Height - Txt-Input::Height - 24
>           MOVE WS-INPUT-Y TO Txt-Input::Y
>           MOVE WS-INPUT-Y TO Btn-Send::Y
>           COMPUTE WS-CHAT-H = WS-INPUT-Y - Vwr-Chat::Y - 20
>           MOVE WS-CHAT-H TO Vwr-Chat::Height
>   ```
>
> 背后没有引擎的事件（控件的拖放、列的排序/调整大小、图表缩放……）不在设计视图中列出——你能绑定的事件就是会触发的事件。（TreeView 的 `onNodeCheck`、`onNodeCollapse` 和 `onNodeExpand` 被列出，并会触发。）

### 添加处理程序

在树或属性窗格中，点击一个事件来为它打开 COBOL 编辑器。处理程序是自包含的**嵌套程序**，你在**一个**编辑器中编辑它的整个主体——没有单独的 working-storage 框。

事件编辑器与主代码编辑器是**同一个完整的编辑器**：键入时的 **IntelliSense**（关键字、动词和窗体的控件名称；`Ctrl+Space` 触发）、右上角的 **Find/Replace**（`Cmd/Ctrl+F`，带 *Replace* 和 *Replace All*），以及沿着底部的**状态栏**（插入符 `Ln, Col`、通过 `Insert` 键切换的 **Insert/Overwrite**、**Trim on save** 和 **Beautify**）。它以窗口的 70 % 打开，并且可以自由调整大小。

编辑器是设计器**之上**的一个窗口，而鼠标在它上面做的一切都归窗口所有：在其中选择文本永远不会到达它后面的工具箱或窗体——窗口之下的工具箱按钮不会开始它的拖动，而从工具箱拖出并在编辑器之上松开的控件不会被放到它后面的窗体上。（在 1.90.23 之前，在编辑器位于工具箱按钮之上的地方按在编辑器的文本上，会被当作按在那个按钮上：拖动选择文本会提起工具箱控件的一个副本，并在你松手时把它丢到编辑器后面的窗体上。）

**第一次**打开尚未编写的处理程序时，编辑器用标准骨架预置它，所以你只需要填空：

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.
```

从 `ENVIRONMENT DIVISION` 往下到你的语句，都由你编辑；PowerRustCOBOL 只提供 `IDENTIFICATION DIVISION` / `PROGRAM-ID` 头部以及结尾的 `GOBACK` / `END PROGRAM`（在编辑器周围以灰色显示）。

- **局部的临时变量**直接放进这个处理程序自己的 `WORKING-STORAGE SECTION`。
- **共享状态**存在于窗体的全局 working-storage 中（对每个处理程序可见，因为它在外层程序中被声明为 `GLOBAL`）。
- **事件数据**——当事件把数据交给它的处理程序时，这些数据项出现在 `LINKAGE SECTION` 中，并由 `PROCEDURE DIVISION USING …` 绑定。平台中恰好有**两种**这样的载荷，而设计器为你预置每一种。

  **重复组**内的控件接收触发的那张卡片的从 1 开始的索引：

  ```cobol
       LINKAGE SECTION.
       01 CONTROL-ARRAY-INDEX     PIC S9(4) COMP-5.

       PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX.
  ```

  **TreeView 节点事件**——`onNodeClick`、`onNodeSelect`、`onNodeDblClick`、`onNodeCheck`、`onNodeCollapse`、`onNodeExpand`、`onNodeRenamed`——接收节点本身，作为一个组，所以只想要文本的处理程序仍然可以单独读取 `CONTROL-NODE`：

  ```cobol
       LINKAGE SECTION.
       01 CONTROL-NODE-DATA.
          05 CONTROL-NODE           PIC X(256).
          05 CONTROL-NODE-INDEX     PIC S9(4) COMP-5.
          05 CONTROL-NODE-LEVEL     PIC S9(4) COMP-5.
          05 CONTROL-NODE-CHECKED   PIC 9.

       PROCEDURE DIVISION USING CONTROL-NODE-DATA.
  ```

  `CONTROL-NODE` 是节点的标签——每个 TreeView 属性使用的键——`CONTROL-NODE-INDEX` 是它在 `Items` 中**按所写**的从 1 开始的行号，所以 `Sorted` 无法给它重新编号，`CONTROL-NODE-LEVEL` 是它从 1 开始的深度，而 `CONTROL-NODE-CHECKED` 在它的方框被勾选时为 `1`，未勾选或树根本没有方框时为 `0`。

  DataGrid 的 **`onRowButtonClick`**——点击 **Content** 为 **Button** 的列的按钮——接收按钮所在的行。它的 LINKAGE SECTION 只有一行，而设计器为你写头部：

  ```cobol
       LINKAGE SECTION.
       COPY DATAGRID-ROW-CONTENT.

       PROCEDURE DIVISION USING DATAGRID-ROW-CONTENT.
  ```

  每次生成都会用该组替换那一行 `COPY`，该组由网格当时的列构建：

  ```cobol
       01 DATAGRID-ROW-CONTENT.
          05 ROW-NUMBER   PIC 9(9).
          05 ROW-NAME     PIC X(30).
          05 ROW-COLUMNS.
             10 ACTOR-ID     PIC 9(5).
             10 ACTOR-NAME   PIC X(30).
  ```

  - `ROW-NUMBER` 是该行在 `Rows` 中从 1 开始的编号，所以对网格排序或筛选不会改变它。
  - `ROW-NAME` 是被点击的 Button 列的名称，所以一个处理程序可以服务一个 Edit 列和一个 Delete 列。
  - 在 `ROW-COLUMNS` 之下，每个**数据**列有一个数据项，按显示顺序。每个都以列的方式命名：它的标题，否则是它的源字段，否则是它的 id，变成 COBOL 字（`Actor Name` 变成 `ACTOR-NAME`）。它的 picture 是列的 COBOL 掩码，没有时是 `X(30)`。

  细节请见 DataGrid 一节。

  其他每个事件都不携带数据：空的 `LINKAGE SECTION` 和不带 `USING` 的普通 `PROCEDURE DIVISION.`。

> 如果你保持预置的模板不动并关闭编辑器，什么都不会被保存——处理程序保持“未编写”，直到你真正添加代码。

---

## 11. 从 COBOL 与界面对话

### 读取和写入属性

控件的属性用 **`::`** 成员语法或 **`INVOKE`** 动词读取和写入——与用于方法的相同形式。成员就是属性名。同一件事有几种写法——`::`、带引号的成员名、带或不带 `GET-` / `SET-` 前缀的 `INVOKE`，以及 `COBOL` 对象的 `GET-PROPERTY` / `SET-PROPERTY` 内置调用（本章最后一节）——它们可以互换：挑选读起来最顺的。

属性的行为像它自己类型的 working-storage 数据项，所以你不需要为它声明临时字段——见 §13 中的*代码风格*。

**读取（GET）**——`control::property` 是可以在任何地方使用的值（DISPLAY、MOVE 的源、IF、COMPUTE），或者用 `INVOKE … RETURNING` 读取：

```cobol
      *> inline — used directly as a value
           DISPLAY Button-1::Caption.
           MOVE Button-1::Caption TO WS-NAME.
           IF TextBox-1::Text = SPACES
               DISPLAY "empty".

      *> quoted member name — identical
           MOVE Button-1::"Caption" TO WS-NAME.

      *> INVOKE verb (optionally the explicit GET- prefix)
           INVOKE Button-1 "Caption"     RETURNING WS-NAME.
           INVOKE Button-1 "GET-Caption" RETURNING WS-NAME.
```

**写入（SET）**——用 `MOVE`/`SET` 赋值给 `control::property`，或用 `INVOKE … USING` 传递值：

```cobol
      *> inline — MOVE or SET into the property
           MOVE "Hello!" TO Button-1::Caption.
           SET Button-1::"Caption" TO "Hello!".

      *> INVOKE verb (a USING argument means set; SET- is the explicit prefix)
           INVOKE Button-1 "Caption"     USING "Hello!".
           INVOKE Button-1 "SET-Caption" USING "Hello!".
```

属性名**不区分大小写**，并且恰好就是属性窗格中的那些（`Caption`、`Text`、`BackgroundColor`、`Value`，……）。**数值**属性读作数字，所以 `IF Slider1::Value > 50` 是代数比较，并且你可以在数据项与属性之间移动或计算——例如 `MOVE WS-N TO Spinner1::Value`——不需要中间的 `PIC` 数据项。

> **IntelliSense。** 在控件 id 之后键入 `::`（或 `::"`），编辑器会列出该控件的**属性（绿色）**和**方法（浅蓝色）**；继续键入以过滤（`Button-1::Cap…` → `Caption`）。单独的 `"` 只是字符串字面量——它不打开弹出层。列表是完整的——每个匹配项，滚动显示，从不是被截断的样本——并且同一个编辑器用于 **Form Designer 的事件处理程序**，所以它在那里的行为完全相同。
>
> 接收者就是 `::` 左边的表达式，不论它位于何处。左括号或逗号结束语句的操作数并开始一个新名称，恰如空格所做的那样，所以所有这些都能补全：
>
> ```cobol
>            COMPUTE WS-HALF = (Form-1::Width / 2) * 4
>            Grid-1::Fill(Slider-1::Value)
>            Grid-1::Fill(WS-ROW, Slider-1::Value)
> ```
>
> 在第二个和第三个中，**内层**的控件拥有被键入的成员——`Slider-1`，而不是 `Grid-1`。下标仍是它自己表达式的一部分，所以像 `Grid-1::Rows(1)::` 这样的链尾仍然列出 `Grid-1` 的成员。

### 调用控件方法

属性描述*控件是什么*；**方法**描述*它能做什么*——显示它、移动它、把值往上调、添加列表条目、发出 HTTP 请求。每个控件都理解一组**通用**方法，加上它自己的**特定类型**方法。你可以用三种方式调用方法，全都等价：

```cobol
      *> 1. Inline call — reads like a sentence, no result kept
           Lbl-Out::SetCaption("Saved.").

      *> 2. As an expression — the return value flows into a MOVE / IF / COMPUTE
           MOVE Txt-Name::GetText() TO WS-NAME.
           IF Chk-Agree::IsChecked() = "1"
               PERFORM SUBMIT-ORDER
           END-IF.

      *> 3. INVOKE verb — when you prefer the spelled-out keyword, with optional
      *>    USING arguments and RETURNING receiver
           INVOKE Db-1 "query"
               USING "SELECT id, name FROM customer"
               RETURNING WS-ROWS.
```

参数放在括号中（内联 / 表达式形式）或放在 `USING` 之后（`INVOKE` 形式）；返回值的方法可以直接用在表达式中，或用 `RETURNING` 捕获。编辑器的 IntelliSense 在你键入 `::` 之后列出控件的方法，每个带一行描述。

> ⚠️ **方法调用是语句，绝不是接收字段。** 属性可以接收值；方法调用不能，把它写成 `MOVE` 的目标——`MOVE X TO Btn::SetFocus()`——会在运行时引发 *"is a method call, not a receiving field"*。
>
> 写在 `MOVE` 之后那一行的方法调用**不是**那种错误：`MOVE` 的接收字段在方法调用之前结束，所以即使它们之间没有句号，该调用也是下一条语句。
>
> ```cobol
>            MOVE WSE-STAT TO WS-SENT
>            me::"SetProperty"("meuParametro", WSE-STAT)
>            INVOKE me::"OpenFormSync"("FRM-ERROARQ")
> ```
>
> 一个 `MOVE` 之下的多个接收者，只要它们都*是*接收者，就仍然合法：`MOVE GLOBAL-TOTAL TO GLOBAL-TOTAL-ED dgReceipt::X` 写入编辑后的数据项**以及** `X` 属性，而 `List::Items(4)` 是一个元素，所以它也接收。`ADD`、`SUBTRACT`、`MULTIPLY` 和 `DIVIDE` 的接收者也是如此。

**通用方法**（每个可见控件）：


| 方法                                           | 效果                                    |
| ------------------------------------------------ | ----------------------------------------- |
| `Show` / `Hide`                                  | 打开或关闭 `Visible` 属性。      |
| `Enable` / `Disable`                             | 打开或关闭 `Enabled` 属性。      |
| `SetFocus`                                       | 给控件键盘焦点。          |
| `MoveTo(x, y)`                                   | 重新定位控件（设置 `X` / `Y`）。   |
| `Resize(w, h)`                                   | 更改它的大小（设置 `Width` / `Height`）。 |
| `BringToFront` / `SendToBack`                    | 更改堆叠顺序。                    |
| `SetProperty(name, value)` / `GetProperty(name)` | 按名称对任何属性的通用访问。   |

**特定类型的要点**（完整列表在 IntelliSense 中）：


| 控件                      | 方法                                                                                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Label / Button              | `SetCaption`, `GetCaption`                                                                                                                                      |
| Text box                    | `SetText`, `GetText`, `AppendText`, `Clear`                                                                                                                     |
| Check box / radio           | `IsChecked`, `SetChecked`, `Toggle`, `Select`                                                                                                                   |
| Progress / slider / numeric | `SetValue`, `GetValue`, `Increment`, `Decrement`, `Reset`                                                                                                       |
| List / combo                | `AddItem`, `RemoveItem`, `RemoveAt`, `GetCount`, `GetSelected`, `SetIndex`, `LoadFromFile`, `Clear`, `RefreshBinding`                                                         |
| Timer                       | `Start`, `Stop`, `SetInterval`, `IsEnabled`                                                                                                                     |
| REST Client                 | `get`, `post`, `put`, `delete`, `call`, `setHeader`, `clearHeaders`                                                                                             |
| SQL Database                | `open`, `execute`, `query`, `fetch`, `fetchAll`, `close`                                                                                                        |
| AI Agent                    | `Ask`, `SetPrompt`, `SetModel`, `Stop`                                                                                                                          |
| DataGrid                    | `RefreshBinding`, `Save`, `DiscardEdit`, `ExportCSV`, `SetFilter`, `ClearFilters`, `FreezeColumns`, `FreezeRows`, `SetRowHeight`, `SetColumnWidth`, `GetSelectedText`, `CopySelection` |

方法名**不区分大小写**：`get`、`Get` 和 `GET` 是同一个调用。REST Client 和 SQL Database 两行只用小写书写，是因为它们的示例就是那样拼写的。

更改属性的方法会**立即**更新运行中的窗体——与属性语法使用的是同一个通道——所以 `Lbl-Out::SetCaption("Done")` 在运行的那一刻就重绘标签。方法和属性语法完全可以互换；挑选读起来最适合你正在写的那一行的。

> **设计值在你设置任何东西之前就可用。** 窗体启动时，每个控件都被预置了来自它的属性窗格的值，所以即使在第一个 setter 运行之前，`Txt-Name::GetText()`（或 `Txt-Name::Text`）也会返回你在设计时键入的文字。

### 显示和隐藏图层

图层（§7）在 COBOL 中通过它的名称来寻址，恰如控件那样，而 `Visible` 是它的主要属性。**每个图层在程序运行时都从隐藏开始**——不论设计器留下了显示什么——并且**保持你所设置的样子**，直到你再次设置它：

```cobol
      *> onClick of BTN-HELP, a button on Form
           SET HELP-LAYER::Visible TO TRUE.

      *> onClick of BTN-CLOSE-HELP, a button ON the layer
           SET HELP-LAYER::Visible TO FALSE.
```

图层的其他属性是它的背景，你以同样的方式更改它们：

```cobol
           MOVE "#10203080" TO HELP-LAYER::BackgroundColor.
           MOVE 40          TO HELP-LAYER::Transparency.
           MOVE "assets/paper.png" TO HELP-LAYER::BackgroundImage.
           MOVE "Tile"      TO HELP-LAYER::BackgroundImageMode.
           MOVE HELP-LAYER::Visible TO WS-STATE.   *> reads "true" or "false"
```

| 属性 | 含义 |
|----------|---------|
| `Name` | 图层的名称。只读——图层在设计器中重命名，每个引用都会跟随。 |
| `Visible` | 显示或隐藏。读作 `true` / `false`。从隐藏开始。 |
| `Transparency` | 0–100。新图层在颜色上是完全透明的，所以在你给它一个颜色之前，显示它不会改变任何东西。 |
| `BackgroundColor` | `#RRGGBB` 或 `#RRGGBBAA`。 |
| `BackgroundGradientEnabled`, `BackgroundGradientStartColor`, `BackgroundGradientEndColor`, `BackgroundGradientDirection` | 图层的控件后面的渐变。 |
| `BackgroundImage`, `BackgroundImageMode` | 一幅图像，以及它如何适应（`Fill`、`Fit`、`Stretch`、`Center`、`Tile`）。 |

图层没有方法也没有事件，没有圆角半径，也没有窗口属性；请求任何其他东西，都是 **Build 和 Run Form 时的错误**，绝不会被悄悄忽略——`HELP-LAYER::Colour` 和 `NO-SUCH-LAYER::Visible` 会让构建停止，就像未知控件那样。`Non-Visuals` 是设计器的一个标签页，而不是一个对象：`Non-Visuals::Visible` 以同样的方式失败。

图层上的控件像其他任何控件一样通过它的名称到达——`MOVE "Done" TO BTN-CLOSE-HELP::Caption`——不论它在哪个图层上。结果在从 IDE 运行的窗体、作为子窗体嵌入的窗体和构建出的二进制中都是一样的。

PowerDemo3 有一个可以工作的示例——**General ▸ Layers**（`layers-form`）：一个半透明的帮助图层，它的卡片带有一个在 0 处让图层变得不透明的滑块、一块由 Timer 撤下的忙碌帷幕，以及两者之上的一个错误横幅，每一个都由 COBOL 按名称显示和隐藏。

### 成员访问链与集合

`::` 运算符**可以链接**，所以你可以用一种一致的语法到达成员的成员，任意深度。下标 `(n)` 索引一个集合（网格的行、列表的条目、行的列）；裸名称是属性；带 `()` 的名称是方法调用：

```cobol
      *> read a nested cell, then a method on its value
           DISPLAY Grid-1::Rows(I)::Columns(2)::Value.
           DISPLAY Grid-1::Rows(I)::Columns(2)::Value::Upper().

      *> write a nested cell — the structure is created on demand
           MOVE "Total" TO Grid-1::Rows(1)::Columns(1)::Value.

      *> a method on a collection element (mutates it)
           List-1::Rows(I)::Delete().

      *> index the legacy item list (from 1); count its entries
           DISPLAY List-1::Items(3).
           DISPLAY List-1::Items::Count().
```

**属性是接收字段；方法结果不是。** 以**裸属性**（或带索引的单元格）结尾的链是*可读也可赋值的*——所以每个改变内容的动词都可以写入它，而不只是 `MOVE`/`SET`：

```cobol
           MOVE  WS-TEXT       TO Label-1::Caption.
           ADD   1             TO Counter-1::Value.
           STRING WS-A WS-B DELIMITED BY SIZE INTO Label-1::Caption.
           COMPUTE Slider-1::Value = Slider-1::Value * 2.
```

以**方法调用** `()` 结尾的链只是一个值：

```cobol
           MOVE name TO obj::Upper().   *> INVALID — not a receiving field
           SET  name TO obj::Upper().   *> valid — reads the transformed value
           obj::Upper().                *> valid as a statement, but changes nothing
```

链元素上可用的**集合 / 值辅助方法**：`Count` / `Size`（条目数）、`Delete` / `Remove`、`Clear`、`Add` / `Append`，以及值方法——`Trim`、`Upper`、`Lower`、`Len`、`Replace` 和 `Split`——与数据项所响应的相同（见 §13 中的*代码风格*）。方法名不区分大小写，而较旧的写法 `UpperCase` / `ToUpperCase`、`LowerCase` / `ToLowerCase` 和 `Length` 是被接受的别名。

**对控件使用 INITIALIZE。** 初始化控件会重置它的 **`Value`** 属性；你也可以明确指定某一个属性，并把控件与普通数据项混在一起——每个操作数遵循它自己的规则：

```cobol
           INITIALIZE Spinner-1.            *> resets Spinner-1::Value
           INITIALIZE Spinner-1::Value.     *> the same, explicitly
           INITIALIZE Spinner-1 WS-COUNT.   *> control → Value, data item → PIC default
```

“数据项”包括处理程序在它**自己的** WORKING-STORAGE 中声明的数据项（每个事件处理程序都是它自己的程序）：那里的 `INITIALIZE LK-FILE` 重置 `LK-FILE`。**组**会重置它之下的每个基本数据项——数值项置零，其他一切置空格，在表的每次出现中都如此——跳过它的 `FILLER` 项，而 `REDEFINES` 项跟随它的目标；`INITIALIZE WS-ROW (2)` 只重置那一次出现。`INITIALIZE group REPLACING NUMERIC DATA BY 0` 只触及它所指名的类别。在 1.91.4 之前，处理程序自己的数据项被当作控件而保持原样。每个程序都走同样的路径：自 1.91.5 起，最外层程序的表重置在语句之前写入的每一次出现（以前只重置未触动的那些），两个组共用的子名称在两者中都被重置，而带有 88 级条件名的数据项像任何其他数据项一样被重置（以前它被跳过）。

### 通过 `COBOL` 对象访问属性（同样受支持）

内置形式仍然可用，并且与上面的语法可以互换（见下面的*内置调用：`COBOL` 对象*）：


| 内置调用                 | 用途                                                     |
| ------------------------ | ----------------------------------------------------------- |
| `COBOL::"WAIT-EVENT"`    | 阻塞，直到下一个界面事件（由生成的循环使用）。 |
| `COBOL::"GET-PROPERTY"`  | 把控件属性读入数据项。                   |
| `COBOL::"SET-PROPERTY"`  | 从数据项写入控件属性。                     |

处理程序是嵌套程序，而不是段落，它的主体就是你写的内容——IDE 提供 `IDENTIFICATION DIVISION` / `PROGRAM-ID` 头部和 `END PROGRAM` 结束符。同样的问候处理程序，使用 `::`：

```cobol
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NAME     PIC X(40).
       01 WS-MESSAGE  PIC X(60).

       PROCEDURE DIVISION.
           MOVE TXT-NAME::Text TO WS-NAME.
           STRING "Hello, " DELIMITED BY SIZE
                  WS-NAME    DELIMITED BY SPACE
                  INTO WS-MESSAGE.
           SET LBL-OUT::Caption TO WS-MESSAGE.
           GOBACK.
```

如果改用这些内置调用来写，那两行属性语句会读作 `COBOL::"GET-PROPERTY"( "TXT-NAME" "Text" WS-NAME )` 和 `COBOL::"SET-PROPERTY"( "LBL-OUT" "Caption" WS-MESSAGE )`。它们仍然有效，但应当写的是控件本身上的 `::` 形式——智能体被指示永远不要为控件访问发出这些内置调用。

### 内置调用：`COBOL` 对象

在控件之外，运行时还有一组**内置调用**——HTTP、SQL、文本文件、图片、操作系统的文件对话框、API 密钥、模型列表、图表。它们是一个对象 **`COBOL`** 的方法，你把它们内联地写出来，就像任何其他方法：

```cobol
           COBOL::"HTTP-GET" ( WS-URL WS-RESPONSE WS-HTTP-STATUS )
           COBOL::"OPEN-FILE-DIALOG" ( "Import settings" "XML files|xml" WS-PATH )
```

如果你来自 PowerCOBOL 或 isCOBOL，请把它读作你会 `CALL` 的运行时库——而且你仍然可以：`COBOL::"HTTP-GET"( … )` **就是** `CALL "COBOL-HTTP-GET" USING …`。两者是同一个调用，参数相同，顺序相同。数据项按引用传递，所以内置调用返回的任何东西都落在其中；字面量按内容传递。应当写的是内联形式——它读起来就是它本来的样子，IntelliSense 会补全它（键入 `COBOL::` 以查看每个内置调用及其参数），而且它是智能体所写的形式。

> **注意。** 内置调用是**语句**：它的结果在它的参数中返回，所以它从不作为值使用（`MOVE COBOL::…` 是错误的）。名称可以裸写（`COBOL::HTTP-GET( … )`），也可以保留它的 `COBOL-` 前缀；加引号是惯用风格。参数以空格分隔。通用过程仍然用 `CALL "PROCEDURE-NAME"` 到达。

按家族列出的内置调用（每一个在它自己的一节中说明）：

- **图表：**`CHART-ADD-POINT`、`CHART-SET-TABLE`、`CHART-CLEAR`、`CHART-REFRESH`。
- **SQL：**`OPEN-DB`、`EXEC-SQL`、`FETCH-ROW`、`NEXT-ROW`、`ROW-COUNT`、`CLOSE-DB`。
- **HTTP：**`HTTP-GET`、`HTTP-POST`、`HTTP-PUT`、`HTTP-DELETE`、`HTTP-SET-HEADER`、`HTTP-CLEAR-HEADERS`，以及 `HTTP-DOWNLOAD`，它把响应**按字节**保存到文件中（见 §13 中的*通过 HTTP 下载文件*）。
- **文本文件：**`WRITE-FILE`、`APPEND-FILE`，以及 `FILE-STATUS`，用于 `FD` 文件的最近一次状态。
- **图片：**`IMAGE-FIT`，它把图片裁剪并调整为精确的大小，以及 `IMAGE-INFO`，它读取图片的大小和格式（见 §13 中的*图片：适配与测量一张图像*）。
- **声音：**`AUDIO-PLAY`、`AUDIO-PAUSE`、`AUDIO-RESUME`、`AUDIO-STOP`、`AUDIO-STATE`——见下面的*播放声音*。
- **请求文件或文件夹：**`OPEN-FILE-DIALOG`、`SAVE-FILE-DIALOG`、`FOLDER-DIALOG`。
- **模型与密钥：**`MODEL-SET`、`MODEL-REMOVE`、`MODEL-COUNT`、`MODEL-GET`、`MODEL-LIST`、`MODEL-LIST-GET`、`MODEL-TEST`、`PROVIDER-COUNT`、`PROVIDER-GET`、`KEY-SET`、`KEY-REMOVE`、`KEY-IS-SET`。
- **运行时键入的 AWS 密钥：**`AWS-SET-CREDENTIALS`、`AWS-CREDENTIALS-SET`、`AWS-CLEAR-CREDENTIALS`（见*调用 AWS*）。
- **数据绑定与 MCP：**生成的代码所发出的 `BINDING-…` 调用，以及 `MCP-SEARCH`。
- **生命周期：**`INIT-FORM` 和 `WAIT-EVENT`，由生成的程序替你调用——在那里也是内联书写的。

> **注意。** 传给 `GET`/`SET` 的属性名恰好就是属性窗格中显示的名称（例如 `"Text"`、`"Caption"`、`"BackgroundColor"`、`"Value"`）。控件 ID 是树中显示的 ID（例如 `"BTN-GREET"`）。

#### 播放声音：`AUDIO-…` 内置调用

PowerCOBOL 和 isCOBOL 把声音留给媒体控件或操作系统调用。在这里，五个内置调用给**任何**程序以声音——控制台程序与窗体一样——而你的用户看到的播放器是用普通控件构建的。

| 内置调用 | 参数，按顺序 | 它做什么 |
|---|---|---|
| `COBOL::"AUDIO-PLAY"` | file, [status] | 开始播放 MP3、OGG Vorbis、WAV 或 FLAC 文件，替换任何正在播放的声音 |
| `COBOL::"AUDIO-PAUSE"` | [status] | 暂停；位置保持 |
| `COBOL::"AUDIO-RESUME"` | [status] | 从暂停的地方继续 |
| `COBOL::"AUDIO-STOP"` | [status] | 停止，播放器再次是 `IDLE` |
| `COBOL::"AUDIO-STATE"` | state, [position-ms], [duration-ms] | 播放器处于什么状态，以及声音进行到多远 |

这些调用不等待声音：`AUDIO-PLAY` 在文件打开并且设备正在播放它时回答——通常是十分之一秒，在机器的声音系统第一次唤醒时会稍长一点——而声音自己继续播放，同时你的程序继续运行。请把状态声明为 `PIC X(10)`，把两个时间声明为 `PIC 9(7)`——整毫秒，足以表示两个又四分之三小时。

| 状态 | 含义 |
|---|---|
| `OK` | 完成 |
| `NOFILE` | 文件不存在、是文件夹，或无法读取 |
| `BADFORMAT` | 文件不是四种格式之一的声音，或已损坏 |
| `NODEVICE` | 机器没有可以打开的输出设备 |
| `NOFEATURE` | 程序构建时没有带声音（见最后一条注意事项） |
| `ERROR` | 没有东西可暂停或可恢复时的 `AUDIO-PAUSE` 或 `AUDIO-RESUME` |

状态是五个词之一：

```mermaid
stateDiagram-v2
    [*] --> IDLE
    IDLE --> PLAYING: AUDIO-PLAY 回答 OK
    IDLE --> ERROR: AUDIO-PLAY 回答 NOFILE、BADFORMAT 或 NODEVICE
    PLAYING --> PAUSED: AUDIO-PAUSE
    PAUSED --> PLAYING: AUDIO-RESUME
    PLAYING --> ENDED: 声音到达末尾
    PLAYING --> IDLE: AUDIO-STOP
    PAUSED --> IDLE: AUDIO-STOP
    ENDED --> IDLE: AUDIO-STOP
    ERROR --> IDLE: AUDIO-STOP
```

`AUDIO-PLAY` 可以在任何状态下调用：它结束正在播放的任何东西并重新开始——新文件播放时是 `PLAYING`，不能播放时是 `ERROR`。`ENDED` 让位置等于长度。长度是文件所声明的那个：WAV、FLAC 和 OGG 文件通常会声明，MP3 在带有长度头时会声明；当文件不声明时，长度在声音播放期间读作 `0`，并在它结束时才变得已知。

##### 示例：Polly 语音的播放器

窗体有一个文本框 `TXT-SAY`、一个 `AwsPolly` 控件 `POLLY-1`（§16）、按钮 `BTN-SPEAK`、`BTN-PLAY`、`BTN-PAUSE` 和 `BTN-STOP`、一个 ProgressBar `PRG-PLAY`（0 到 100）、一个 Label `LBL-TIME`，以及一个 `Interval` 为 200 ms、开始时关闭的 Timer `TMR-PLAY`。Polly 把语音写入文件；声音内置调用播放它，而定时器让读数保持最新。

```cobol
       PROGRAM-ID. BTN-SPEAK--ONCLICK.
       PROCEDURE DIVISION.
           INVOKE POLLY-1::Synthesize(TXT-SAY::Text, "Joanna", "mp3",
               "speech.mp3")
           .

       PROGRAM-ID. POLLY-1--ONSYNTHESIZED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS     PIC X(10).
       PROCEDURE DIVISION.
           COBOL::"AUDIO-PLAY" ( POLLY-1::SavedFile WS-STATUS )
           IF WS-STATUS = "OK"
               SET TMR-PLAY::Enabled TO 1
           ELSE
               MOVE WS-STATUS TO LBL-TIME::Caption
           END-IF
           .

       PROGRAM-ID. BTN-PAUSE--ONCLICK.
       PROCEDURE DIVISION.
           COBOL::"AUDIO-PAUSE" ( )
           .

       PROGRAM-ID. TMR-PLAY--ONTICK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATE      PIC X(10).
       01 WS-POS-MS     PIC 9(7).
       01 WS-LEN-MS     PIC 9(7).
       PROCEDURE DIVISION.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS-MS WS-LEN-MS )
           IF WS-LEN-MS > 0
               COMPUTE PRG-PLAY::Value = WS-POS-MS * 100 / WS-LEN-MS
           END-IF
           MOVE SPACES TO LBL-TIME::Caption
           STRING FUNCTION INTEGER-PART(WS-POS-MS / 1000)
                      DELIMITED BY SIZE
                  " of " DELIMITED BY SIZE
                  FUNCTION INTEGER-PART(WS-LEN-MS / 1000)
                      DELIMITED BY SIZE
                  " s" DELIMITED BY SIZE
               INTO LBL-TIME::Caption
           IF WS-STATE NOT = "PLAYING" AND WS-STATE NOT = "PAUSED"
               SET TMR-PLAY::Enabled TO 0
           END-IF
           .
```

`BTN-PLAY` 恢复（`COBOL::"AUDIO-RESUME" ( )`），而 `BTN-STOP` 停止（`COBOL::"AUDIO-STOP" ( )`），方式与 `BTN-PAUSE` 相同。

📷 需要截图 — `audio-player-form.png`：上面的窗体在语音播放时——`TXT-SAY` 中有文字，进度条走到一半，而 `LBL-TIME` 读作例如 "3 of 8 s"。

控制台程序没有定时器，所以为了让短的声音在程序结束之前播完，它会不断询问，直到声音结束：

```cobol
           COBOL::"AUDIO-PLAY" ( "chime.wav" WS-STATUS )
           COBOL::"AUDIO-STATE" ( WS-STATE )
           PERFORM UNTIL WS-STATE NOT = "PLAYING"
               COBOL::"AUDIO-STATE" ( WS-STATE )
           END-PERFORM
```

> **注意——每个应用程序一个声音。** `AUDIO-PLAY` 替换正在播放的任何东西，而且每个窗体与同一个播放器对话，所以窗体可以显示另一个窗体所启动的声音的进度。

> **注意——没有声卡时的测试。** 使用环境变量 `COBOLT_AUDIO=null` 时，不向设备发送任何东西：文件仍被打开和解码，所以 `NOFILE` 和 `BADFORMAT` 的含义一如既往，并且一个时钟按它的长度“播放”它——状态、位置和声音的结束，其表现与有扬声器时完全一样。产品自己的测试就使用它（`COBOLT_AUDIO=null rcrun run tests/cobol/audio/test-audio.cbl`）。没有它时，没有输出设备的机器回答 `NODEVICE`。

> ⚠️ **注意事项——声音随启动它的程序一起结束。** 当控制台程序到达 `STOP RUN`，或启动了声音的窗体关闭时，声音就会停止。如果必须把它听完，请像上面那样先等待 `ENDED`。

> ⚠️ **注意事项——相对路径**从程序运行所在的文件夹解析，与 `WRITE-FILE` 相同。没有音量或定位控制：适用的是操作系统的音量。

> ⚠️ **注意事项——构建链接什么。** 只有当程序的某个 `AUDIO-…` 调用按字面写出该内置调用的名称时，它才会带声音构建，而内联形式总是如此。这让声音——以及在 Linux 上的 ALSA 库（§3）——远离从不播放声音的每个应用程序。`NOFEATURE` 是不带声音构建的运行时所回答的；通过 IDE 和 `rcrun`，你不会遇到它。

### 多窗体应用程序与主窗体

每个项目恰好有**一个主窗体**——应用程序首先显示的窗体，也是应用程序在操作系统任务栏/程序坞中的唯一标识。你创建的第一个窗体自动承担这个角色；要移动它，请在另一个窗体的 Window 属性中勾选 **Main form**（当前持有者的复选框是只读的，所以项目永远不会没有主窗体）。移动在你勾选框的那一刻就记录在两个窗体文件中——即使另一个窗体在设计器中带着未保存的更改打开，即使你之后不保存就关闭它——所以撤销是把它拿回来的方法。Forms 树用**皇冠**标记主窗体。如果项目在打开、运行或构建时标记了零个或多个窗体（例如手工编辑的 `.cfrm`），项目列表中的第一个窗体胜出，状态行会这样说明；如果那不是你想要的那个，请在你想要的窗体上重新勾选 **Main form**。

#### 只有主窗体启动应用程序

IDE 运行你要求它运行的任何窗体——这正是设计器的用途。*运行时*则不然。构建出的二进制和 `rcrun` 总是打开项目的主窗体，而不是别的。在主窗体是你的登录窗体的地方，这条规则正是阻止某人直接启动第三个窗体而绕过它的原因。

这个指定被记录**两次**，而两份记录必须一致：

- **在窗体中**——它的 `.cfrm` 之内的 `main-form` 标记，IDE 把它恰好保持在一个窗体上。
- **在项目文件中**——`[forms]` 之下的 `main-form`，连同 `main-form-seal`，一个针对该指定和项目窗体列表的摘要。

你从不手工维护其中任何一个：IDE 在每次保存时重写两者。运行时从窗体文件重新推导该指定，并把它与项目文件比较。如果它们不一致——标记被移到另一个窗体、`[forms] main-form` 指向别处、封印被删除——应用程序就会报告**已损坏的应用程序**，并立即退出，不打开任何窗口：

```text
run-form: CORRUPTED APPLICATION — the main-form seal does not match this
project's forms.
This application will not start. Restore it from its original distribution.
```

向运行时请求仅仅*不是*主窗体的窗体，不算损坏。它会被拒绝，并且消息会指明应用程序实际启动的窗体。用通常的方式打开那个窗体——从运行中的应用程序用 `OpenFormSync` / `OpenFormAsync`，或菜单项——不受影响：应用程序自己决定谁能通过，这正是要点所在。

> **注意。** 文件早于封印的项目继续工作。没有记录指定时，运行时回落到标记为主的窗体——或者，在比标记更老的项目中，回落到项目中的第一个窗体——并警告一次该项目未封印。

**更新较旧的项目。** 在 PowerRustCOBOL 中打开一个，它会提供升级——*Update this project's structure*，列出会改变什么以及它带来什么。接受，指定就会被记录并封印。拒绝，则**什么都不会改变**：IDE 不会触动你没有要求它更改的项目的形状，即使在保存时也不会，并且这个提议会在你下次打开它时再次出现。

这个机制是通用的。`[project] structure` 给项目文件的形状编号；PowerRustCOBOL 把当前的编号写入它创建的每个项目，而低于它的任何项目都会被提供把它升上去的步骤。项目文件的将来更改以同样的方式到来——作为一个用你的语言描述的、你可以自由拒绝的提议。

⚠️ **注意事项——封印是什么，不是什么。** 它检测被*编辑*的项目，这正是这条规则所关心的。它不是锁。它的密钥随工具一起交付，所以任何持有项目文件夹和 PowerRustCOBOL 的人都可以指定另一个主窗体并封印它——恰如他们打开了项目并更改了它，因为那正是他们所做的。强的情形是**构建出的二进制**：它的窗体存在于可执行文件之内，它的主窗体是在构建时选定的，而磁盘上没有什么留下来可以编辑。当登录窗体是你要保护的东西时，请把应用程序作为构建出的二进制交付。

主窗体的 Window 部分还提供 **Taskbar icon**——单个任务栏/程序坞条目所用的图像；📂 挑选它，而相对于项目的路径在应用程序运行的任何地方都有效，包括构建出的二进制。从其他窗体打开的窗口从不创建任务栏条目。各操作系统的说明：在 macOS 上，程序坞自然地为每个应用程序显示一个图标；在 Windows/Linux 上，子窗口是带着跳过任务栏标志创建的。

**窗口外框与状态。** 每个窗体都有 `CanMinimize` / `CanMaximize`（标题栏按钮）、`TitleVisible`（`false` = 无外框窗口）、`WindowState`（`Normal` / `Minimized` / `Maximized`——窗口打开时所处的状态，可在运行时设置）和 `FullScreen`（与 WindowState 正交：离开全屏会回到先前的状态）。在运行时：

```cobol
    INVOKE me "SetWindowState"  USING "Maximized".
    INVOKE me "SetFullScreen"   USING "true".
    INVOKE me "SetTitleVisible" USING "false".
```

每一次**实际的**全屏转换都会触发窗体的 `onFullScreenChanged` 事件（操作系统可能拒绝请求——事件跟随现实，每次真实的变化一次；请读取 `me` 的 `FullScreen` 以获得新值）。

**从键盘关闭。** **Ctrl+W**（macOS 上是 **⌘W**）关闭拥有焦点的窗口，恰如它的关闭按钮所做的那样：主窗口的关闭结束应用程序（并且它的 `onClose` 处理程序仍然可以拒绝，引发 `onCloseRejected`）；子窗口的则只关闭那个窗口。

**移动没有标题栏的窗口。** 无外框窗口通过它的表面来移动：在窗体背景的任何位置按下并拖动。自己对鼠标什么都不做的控件——Label（在它的文字旁边；文字仍然可以选中）、PictureBox、Animator、不滚动的 Panel 或 GroupBox、Shape、Line、图表、ProgressBar、Gauge、StatusBar——也会移动窗口，从它上面的任何位置，**除非你给它的按下绑定了处理程序**（`onClick`、`onDoubleClick`、`onMouseDown`、`onMouseUp`、`onRightClick`、`onMiddleClick`、`onContextMenu`）。每个自己对鼠标有反应的控件都保留按下，不论你是否处理它：Button 仍然亮起并按下，Knob 或 Slider 仍然转动，字段仍然接受光标，列表仍然选择，滚动的 Panel 仍然滚动。确实有按下处理程序的控件保留它所绘制的部分，并让其余的部分通过：隐藏的控件，Label、Panel 或 GroupBox 的透明背景（它的文字、边框、标题和子项仍然属于它），以及没有框架的 PictureBox（`ShowFrame` 关闭）在它的图片旁边或在图片的透明像素上——挖空的 PNG 在它显示它后面窗体的每个地方都会移动窗口。这对主窗口和子窗口都有效；外壳应用程序的窗口（带有 SideMenu 的主窗体）目前还不能这样移动。

**圆角。** 关闭 **Show title bar** 后，窗体的 **Geometry** 部分提供 **Corner radius**（`CornerRadius`，以像素计，0 到 255；0 让角保持方形）。于是窗口的角被画成圆的，而桌面从曲线之外显示出来：背景颜色、渐变、每种模式下的图片，以及你放在角上的控件，都在那里停住。它对主窗口、子窗口和外壳应用程序的窗口都有效——在那里，导轨、面包屑条和 ContentPane 各自把它们接触的角变圆。设计画布显示的角就是它们运行时的样子，**Preview** 也是：没有标题栏的窗体在没有标题栏的窗口中预览——圆角、通过拖动它的背景来移动，并用 **Esc**（或 **Ctrl+W**，macOS 上是 **⌘W**）关闭，窗口中央的一条通知会在最初的五秒钟提醒这一点。标题栏打开时，角属于操作系统，所以半径被保留但不被使用（并且该行被隐藏）。

```cobol
    MOVE 24 TO me::CornerRadius.        *> a softer window
    MOVE 0  TO me::CornerRadius.        *> square again
```

> ⚠️ **注意事项。** 窗口只能在创建时被做成透明的，所以 `me::CornerRadius` 只有在它的窗体被**设计**为半径大于 0 时，才会在运行时把窗口变圆；请在设计器中设置一个，然后随你喜欢地从 COBOL 更改它。恰好位于角上的内容会被曲线裁掉，而放进角里的 DataGrid、FileDropZone、Maps、TabControl、ToolBar 或 Viewer——或紧贴一个角的 TreeView，它的第一行画在它的最边缘——仍然会画到曲线之外：请让这些离圆角一个半径远。你停放在窗口之外一部分、跨过圆角的控件，画在窗口所显示的那一部分中，所以它的标题在那里居中。操作系统通常画在窗口周围的投影，不会画在窗体有半径的窗口周围——即使它的标题栏正在显示。

> 📷 需要截图 — `rounded-window.png`：一个 Show title bar 关闭、Corner radius 为 24 的窗体，运行在形成对比的桌面之上，使四个圆角和它们后面的桌面都可见。

**它们适用于哪里。** 主窗口、**外壳**应用程序的窗口（带有 SideMenu 的主窗体），以及用 `OpenFormSync` / `OpenFormAsync` 或 Stand Alone 菜单动作打开的**子窗口**，都遵从 `CanMinimize`、`CanMaximize`、`FullScreen` 和 `WindowState`；当调用者没有传入位置时，子窗口还会在它的 `StartPosition` 所说的地方打开，而外壳窗口也采用 `TitleVisible`、`StartPosition` 和任务栏图标。进入/退出效果仍然属于主窗口。

**给窗口定大小，以及一起移动的窗口。** 窗体的 **Geometry** 部分中的五个设计器属性决定窗口如何打开：

- **Resizable**（`Resizable`，默认开启）。关闭时，边框不能拖动：窗口保持它打开时的大小，而响应式布局仍然适应那个大小。
- **Screen fill**（`ScreenFill`，0 到 100 %）。大于 0 时，窗口以屏幕每个轴上的那个份额打开——在窗体自己的最小和最大布局之内——而不是以它设计的大小。当有窗口停靠到它上面时（见下），这个份额是整个组的：主窗口取走已停靠的窗口及其间隙拿走之后剩下的部分。把它与响应式布局和 Center 起始位置搭配，窗口就适应任何显示器。
- **Dock to opener**（`DockToOpener`：None、Top、Bottom、Left、Right）和 **Dock gap**（`DockGap`，默认 16 px）。用 `OpenFormSync` 或 `OpenFormAsync` 从另一个窗体打开的窗口，位于它的打开者窗口的那条边的正外侧，沿着它居中。当打开者移动或改变大小时，已停靠的窗口跟随——每次更改一次，所以间隙保持设计时的样子。已停靠的窗口从不移动它的打开者：拖动打开者来移动整个组。在 macOS 上，已停靠的窗口作为系统自己的子窗口附着在它们的打开者上，所以窗口服务器在你拖动时于同一次屏幕更新中移动整个组——没有窗口拖在另一个后面。点击组中任何窗口，都会把整个组带到前面。在别处，它们在打开者移动之后立即跟随。
- **Dock length**（`DockLength`，0 到 100 %）。大于 0 时，已停靠的窗口有它的打开者那条边的那个份额那么长——100 时是打开者的整个宽度的工具栏，60 时是标签条——并且在打开者改变大小时保持那个份额，包括 **Screen fill** 在打开者打开时给它的大小。请让已停靠的窗体是响应式的，使它自己的布局适应给定的长度。

仪表盘上方的浮动工具栏、它下方的标签条和它旁边的导轨，是三个分别停靠在 Top、Bottom 和 Left 的小窗体，由仪表盘从它的 `onLoad` 打开：

```cobol
       INVOKE me::"OpenFormAsync"("TOOLBAR-FORM") RETURNING WS-TOOLBAR
       INVOKE me::"OpenFormAsync"("ROOMS-FORM") RETURNING WS-ROOMS
       INVOKE me::"OpenFormAsync"("RAIL-FORM") RETURNING WS-RAIL
```

**每个窗口都留在屏幕上。** 主窗口以及停靠到它的窗口，在应用程序启动时，以及每当屏幕改变（另一个分辨率、另一个显示器）时，会被一起适配到屏幕上——窗口可以使用的那部分，不包括菜单栏、程序坞或任务栏。主窗口先缩小，从不小于它的最小布局。只有当组的最小布局也放不下时，整个应用程序才会被向下缩放——每个窗口、它的文字和它的控件都一样，从不低于一半大小——所以四周有工具栏、导轨和标签条的 1000 × 600 仪表盘仍然能完整地显示在小的笔记本屏幕上。你随后移动或调整大小的窗口保持在你放它的地方。

> ⚠️ **注意事项。** 已停靠的窗口比它的打开者慢一帧跟随，所以快速的拖动会让间隙在稳定下来之前被瞬间拉伸。停靠是在窗口打开时读取的；加载到外壳的 ContentPane 中的窗体没有它自己的窗口，也不会被停靠。

**从 COBOL 更改窗口。** 写入窗体自己的属性会更改运行中的窗口，无论是从窗体本身还是从它打开的窗体：

```cobol
    MOVE "Saving…"   TO me::Title.
    MOVE 900         TO me::Width.
    MOVE "#1E3A5FFF" TO me::BackgroundColor.
    MOVE "Done"      TO super::Title.
```

`Title` 重命名标题，`Width`/`Height` 调整大小（64 到 8192），`X`/`Y` 移动，`BackgroundColor`/`Transparency` 重绘背景，而 `CornerRadius` 重新把它变圆。在外壳应用程序中，外壳拥有窗口，所以在那里只有背景改变。

**透明的窗体。** `Transparency` 大于 0 会让主窗口透过窗体显示桌面。`#000000FF` 的 `BackgroundColor` 是黑色，而颜色自己的 alpha 与 `Transparency` 相乘；只有 `#00000000` 或空值才会保留主题默认的深色背景。

> ⚠️ **注意事项。** 在 1.70.237 之前，这些都没有到达窗口：写入只更改了属性而没有别的，外壳或子窗口忽略了它大部分的设计，而黑色背景出来是深蓝色。子窗口不论它的 `Transparency` 是什么，仍然是不透明的。

**FormState——保护未保存的工作。** `FormState` 是只在运行时存在的窗体属性，有两个值，`Ready`（默认）和 `Waiting`。窗体处于 `Waiting` 时，它无法通过任何路径被关闭——标题栏按钮、`windowHandler` 的 `Close`，或级联——而它的 `onCloseRejected` 事件会触发。典型的模式：在 `onTextChanged` 处理程序中设置 `Waiting`，在成功保存之后设置 `Ready`：

```cobol
    INVOKE me "SetProperty" USING "FormState" "Waiting".
    *> … after saving …
    INVOKE me "SetProperty" USING "FormState" "Ready".
```

**从 COBOL 打开窗体。** `me` 上的两个方法，各有两种语法：

```cobol
    *> Comma form — trailing parameters are OPTIONAL and default to the
    *> target form's designed properties; modal defaults to true.
    INVOKE me::"OpenFormSync"("DETAIL-FORM") RETURNING WS-H.
    INVOKE me::"OpenFormAsync"("DETAIL-FORM", "Maximized", 100, 80)
        RETURNING WS-H.

    *> COBOL-standard space form — ALL parameters are required; a missing or
    *> wrongly-typed parameter is a COMPILE-TIME error.
    INVOKE me "OpenFormSync"
        USING "DETAIL-FORM" "Normal" 100 80 640 480 "true"
        RETURNING WS-H.
```

`WS-H` 是一个 **windowHandler**（把它声明为 `USAGE OBJECT`）。通过它你可以 `Close`、`Focus`（先恢复最小化的窗口）、`SetWindowState`、`SetFullScreen`、`SetTitleVisible`，并读取 `WS-H::FormState`。当窗体关闭时，每个引用它的 windowHandler 都会自动变成 **NULL**；通过 NULL 句柄调用是运行时错误。

**打开者在等待时看起来如何——`ModalOverlayStyle`。** 用 `OpenFormSync` 打开的窗体是模态的：在它关闭之前，打开者拒绝输入，对它的点击不会到达任何处理程序，而焦点留在子窗体上。这个行为从不改变。*你*在设计器的 Properties 窗格中按窗体选择的，是被阻塞的表面是否显示出来：

| 值 | 被阻塞的打开者 |
|---|---|
| `None`（默认） | 看起来恰如设计的那样——表现为被阻塞，不戴任何图层。 |
| `SemiTransparent` | 整个表面上一层浅灰色图层（25 % 不透明）。 |
| `Greyed` | 同样的灰色图层，更重（约 60 %）——经典的变暗背景。 |

请在**打开者**——等待的那个窗体——上设置它，而不是在它打开的窗体上：子窗体自己的 `ModalOverlayStyle` 只有在*它*用 `OpenFormSync` 打开另一个窗体时才起作用。打开者在等待期间也保持它自己的主题，不论子窗体是用什么主题设计的。

在其中任何一个之下，窗体都保持它自己设计的 **Transparency**；当打开者位于侧边栏的 ContentPane 中时，图层也会覆盖侧边栏和面包屑，所以整个窗口读起来就是一个等待的表面。从 COBOL 用 `me::"GetProperty"("ModalOverlayStyle")` 读取它。

`OpenFormAsync` 不同：两个窗口是**独立的**——打开者从不被阻塞，所以对于 Async 子窗体，每种样式都读作 `None`。子窗体仍然可以通过 `super::"SetProperty"` 把结果交回来。而关闭打开者会关闭它**任一**种类的子窗体，除非其中某个处于 `Waiting`（`FormState`），那会否决整个关闭并引发 `onCloseRejected`——这与 PowerCOBOL 开发者对带有未保存工作的子窗体所期望的规则相同。

**向子窗体传递数据并取回。** PowerCOBOL 开发者会去用共享的全局区；在这里，两个窗体是独立的程序，数据以**父窗体的属性**的形式传递，用你选择的名称：

```mermaid
sequenceDiagram
    participant P as 父窗体
    participant C as 子窗体
    P->>P: INVOKE ME::"SetProperty"("OrderRecord", WS-ORDER)
    P->>C: INVOKE ME::"OpenFormSync"("PROPS-CHILD-FORM")
    C->>P: INVOKE super::"GetProperty"("OrderRecord") RETURNING WS-ORDER
    Note over C: 操作员进行编辑
    C->>P: INVOKE super::"SetProperty"("OrderRecord", WS-ORDER)
    C->>C: INVOKE ME::Close()
    P->>P: INVOKE ME::"GetProperty"("OrderRecord") RETURNING WS-ORDER
```

父窗体在打开子窗体之前发布：

```cobol
           INVOKE ME::"SetProperty"("CustomerName", Txt-Name::Text)
           INVOKE ME::"SetProperty"("OrderRecord", WS-ORDER)
           INVOKE ME::"SetProperty"("ChildResult", "NONE")
           INVOKE ME::"OpenFormSync"("PROPS-CHILD-FORM")
           INVOKE ME::"GetProperty"("ChildResult") RETURNING WS-RESULT
           IF WS-RESULT = "OK"
               INVOKE ME::"GetProperty"("OrderRecord") RETURNING WS-ORDER
           END-IF
```

子窗体在它的 `onLoad` 中读取，并在关闭之前回答：

```cobol
           INVOKE super::"GetProperty"("CustomerName") RETURNING Txt-Name::Text
           INVOKE super::"GetProperty"("OrderRecord") RETURNING WS-ORDER
      *>   ... on OK:
           INVOKE super::"SetProperty"("OrderRecord", WS-ORDER)
           INVOKE super::"SetProperty"("ChildResult", "OK")
           INVOKE ME::Close()
```

- **简单的值**——名称、金额、标志——每个一个属性。`RETURNING` 可以指名数据项或控件的属性（`RETURNING Txt-Name::Text`）。
- **复杂的数据**——**组项目**，包括头部和 `OCCURS` 表——作为**一个**属性传递：它的字节原样传输，而 `RETURNING` 到同样布局中会逐字段填充它，包括带隐含小数的数值字段（`PIC 9(5)V99`）。两个窗体必须以同样的方式描述该记录；在应用程序中，请从同一个 copybook `COPY` 它。
- **父窗体自己的窗体属性**可以直接读取：`super::Title`。你自己命名的属性总是通过 `GetProperty` / `SetProperty`——直接写 `super::OrderRecord` 无法构建，因为裸名称是针对固定的窗体表面检查的。
- `COBOL::"GET-PROPERTY"` 只读取**它自己**程序的控件；父窗体通过 `super` 到达。

PowerDemo3 有完整的示例——**General ▸ Passing Data to a Child Form**（`props-parent-form` 和 `props-child-form`）。

> 📷 需要截图 — `props-demo.png`：PowerDemo3 的 *Passing Data to a Child Form*，Order Editor 打开在它之上，一个数量被编辑过。

**生命周期规则。**

- **主窗体是单例**：在它运行时打开它，会聚焦运行中的实例并返回它现有的句柄。其他窗体可以运行任意数量的并发实例。
- 子窗体与它们的调用者一起关闭，**Sync 和 Async 都一样**：关闭窗体会关闭它打开的每个窗口，以及这些窗口自己的子窗口。Async 子窗体只有在两者都打开期间才独立于它的打开者——打开者从不被它阻塞——但它不会比它活得更久。
- 只要这些子窗体中的任何一个处于 `Waiting`，调用者就无法关闭（它也会得到 `onCloseRejected`）。
- 当**主窗体**关闭时，每个窗体都关闭，应用程序退出。
- **模态**的 Sync 子窗体会阻塞调用者的输入及其 COBOL 流程，直到子窗体关闭；在调用者恢复时，`RETURNING` 句柄是 NULL。

> **状态。** 上述窗口生命周期规则（FormState 否决、`onCloseRejected`、窗口命令、`onFullScreenChanged`）在 Run Form 和构建出的应用程序中同样有效，`OpenFormSync` / `OpenFormAsync` 的**子窗口**也是如此：每个都在它自己的窗口中打开。

---

## 12. 生成的代码

当你保存/生成一个窗体时，PowerRustCOBOL 会向 `generated/` 写入一个 `.cbl`。
它的形状是可预测的：

- 窗体对应的一个 **PROGRAM-ID**；
- 每个控件状态的 working-storage；
- **事件循环**（围绕 `COBOL::"WAIT-EVENT"` 的 `PERFORM UNTIL`）；
- 每个事件处理程序对应一个**嵌套的 COBOL-85 程序**，命名为
  `CONTROL-ID--EVENTNAME`（大写，例如 `BTN-OK--ONCLICK`）；窗体的
  `onLoad` 在启动时运行，`onClose` 在关闭时运行。

```mermaid
flowchart TB
    CFRM["forms/main-form.cfrm"] -->|Save & Generate| GEN["generated/main-form.cbl"]
    GEN --> OUTER["外层程序：<br/>数据 + 事件循环"]
    OUTER --> P1["嵌套：BTN-OK--ONCLICK"]
    OUTER --> P2["嵌套：TXT-NAME--ONCHANGE"]
    OUTER --> P3["嵌套：MAIN-FORM--ONLOAD"]
```

每个生成的文件都以一段写给你的 `*>` 注释横幅开头：它说明该文件由 PowerRustCOBOL RAD
生成，你不得直接编辑它，并且它的结构可能在版本之间改变（为了性能、可观测性
或缺陷修复），而不会破坏你的代码。

> ⚠️ **注意事项。** 生成的 `.cbl` 是构建产物，所以**不要手工编辑它**——
> 你的编辑会被覆盖。PowerRustCOBOL 在你每次 Build、Run、Debug 或 Check 项目时，
> **都会自动重新生成每个窗体的 COBOL**（已打开的设计器使用它们的实时状态，
> 即使尚未保存；其他窗体从它们的 `.cfrm` 重新加载），所以被编译和运行的
> 总是与你的窗体一致。请把可重用的逻辑放进 **Common Code**，再从处理程序中 `CALL` 它。

### 阅读诊断信息

由于编译器看到的是织入后的 `.cbl`，错误过去是针对那个产物报告的——
`842:17: ✖ error: …`，是一个你从未写过的文件的第 842 行。现在，Check 报告的是
**你**写下的位置。Output 中的一行是这样的：

```
MAIN-FORM ▸ BTN-OK ▸ onClick — 3:12: ✖ error: syntax error near "DISPLYA"
    3 │            DISPLYA "HELLO".
      │            ^
```

- 左边的部分是**位置路径**——窗体，然后是控件和事件
  （或过程名，或章节关键字，例如
  `MAIN-FORM ▸ WORKING-STORAGE`）。它读起来就像你在 RAD 中导航的方式。
- 行号和列号是**在该处理程序或章节自己的文本之内**，
  恰如编辑器所显示的——而不是生成文件的编号。
- 出错的那一行连同被标出的列一起被引用，所以消息自己就能说明
  位置——在截图、论坛帖子里，或者有人从你肩后看到时。
- 这一行是一个**链接**：点击它，IDE 就会打开所属的编辑器——处理程序是
  事件模态框，章节或过程是 COBOL Structure 窗口，Common Code 文件是
  代码编辑器——并把光标放在那一行。生成的 `.cbl` 从不被打开。

有些行属于生成器自己（事件循环、尚未编写的处理程序的桩）。针对这些行的诊断被标为
`[generated code]`，并附上生成的文件和行号，而且有意
**不**归属于你的任何处理程序——如果你看到这样的诊断，错误出在
PowerRustCOBOL 的内部构造，或出在某个属性被设置的方式上，而不在你可以
编辑的代码里。

大多数生成的行是**为**你窗体上的某样东西而写的：数据
绑定的状态和段落，或控件的项目和段落（REST
客户端的字段、SQL 数据库的连接、网格的 CSV 导出）。针对这些行的
诊断会说明它是为什么而写的——
`[generated code] main-form.cbl:165 — written for the data binding
BIND-ORDERS of DG-ORDERS`——并且是一个链接：点击它，IDE 就会打开
选中该控件的窗体，当这一行是为某个绑定写的时，
还会在编辑器中打开那个绑定。那里就是可以把生成的内容
纠正过来的地方。生成的 `.cbl` 仍然从不被打开。

停止 Run Form 和 Build 的错误对话框以同样的方式、同样的措辞说明同一件事：
你写下出错行的位置，以及它在该位置内的行号（`MAIN-FORM ▸ BTN-OK ▸ onClick, line 3`），
或者生成代码的行以及它是为什么而写的。每一项都是指向它的链接。

> **注意。** 位置信息涵盖**编译期**诊断（Check，以及在 Build/Run/Debug 之前
> 运行的解析/分析）。运行时的中止目前仍然报告
> 生成程序的位置。

> **⚠️ 注意事项——未声明的数据项是错误。** `identifier 'X' is
> not declared in DATA DIVISION` 会停止 Check、Run Form 和 Build，如同在
> 任何 COBOL 编译器上一样。它过去是一个警告，而程序随后会运行：
> 运行时在第一次写入时创建该项，**大小取决于那个
> 第一个值**，并把之后的每个值都截到那个宽度——一个 `WS-RESULT`
> 第一次收到 `ButtonOk clicked`（16 个字符），此后就一直显示 `ButtonCancel
> cli`。在 RAD 代码中，有两件事会让 PowerCOBOL 或
> isCOBOL 开发者想不到地踩进这个坑：
>
> - **处理程序的 `WORKING-STORAGE` 属于那个处理程序。** 删除
>   控件，它的声明也随之而去——另一个处理程序
>   仍在使用的数据项，必须在使用它的地方声明，或在窗体级别声明。
> - **处理程序只能通过 `GLOBAL` 看到窗体级别的数据。** 每个事件
>   处理程序都是嵌套在窗体程序中的 `COMMON` 程序，所以窗体级别的
>   项目只有声明为 `01 WS-FS GLOBAL PIC
>   XX.` 时它才可见，而文件的记录只有当它的 `FD` 写着 `IS GLOBAL` 时才可见——
>   `FD … IS GLOBAL RECORD CONTAINS 424 CHARACTERS.` 这些子句要在
>   **同一个语句**中；在 `FD` 的句点之后另起一个 `RECORD CONTAINS` 语句，
>   会使记录条目脱离文件。

> **⚠️ 注意事项——不存在的控件名称同样是错误。**
> 在没有该控件的窗体上写 `INVOKE BTN-SAVX::Disable()`、`MOVE "ok" TO Lbl-Statsu::Caption` 或 `SET
> Lbl-Nope::Visible TO TRUE`，会让 Run Form 和
> Build 停止，并报 *this form has no control or object named 'BTN-SAVX'*，
> 同时给出最接近的真实名称（*Did you mean 'BTN-SAVE'?*）。它过去
> 能编译，而调用或写入在运行时什么也没到达，一声不响。
> 窗体的代码可以在 `::` 之前写出的名称有：
>
> - 它自己的控件，包括按 id 指名的工具栏按钮；
> - 窗体自己的名称、`me` 和 `super`；
> - `COBOL`，用于内置调用；
> - 你声明的数据项（`WS-TEXT::Upper()`）；
> - 程序的 `REPOSITORY` 中的类。
>
> IDE 在开始运行之前就检查这一点，所以错误会指出
> 仍在使用该名称的处理程序和行，点击它就会在那里打开那个
> 处理程序。
>
> 删除一个控件会移除它的名称。另一个控件的处理程序如果
> 仍然写入它，就会在这里被报告，而不是等到运行的那一天
> 什么也不做。

---

## 13. RustCOBOL 语言

RustCOBOL 实现了 **COBOL-85** 的一个相当大的子集，外加 PowerRustCOBOL
的扩展。以下是一位有经验的 COBOL 程序员会依赖的要点：

- **数据与结构：**组项目、`OCCURS`（带下标/索引）、
  `REDEFINES`、`RENAMES`（66 级）、条件名（88 级，带 `VALUE` /
  `THRU`）、`USAGE`，包括 `POINTER`。

> **`PERFORM a THRU b` 是一个段落范围。** 一个指名*位于*范围
> *之内*的段落的 `GO TO`，会在范围之内转移控制，而 `PERFORM`
> 在范围的最后一个段落结束时返回它的调用者——包括
> 那个段落是由 `GO TO` 到达的情形。这是经典的
> 退出段落惯用法，并且照原样写就能工作：
>
> ```cobol
>            PERFORM CHECK-IT THRU CHECK-IT-EX.
>        CHECK-IT.
>            IF WS-VALUE = SPACE GO TO CHECK-IT-EX.
>            MOVE "NON-BLANK" TO WS-NOTE.
>        CHECK-IT-EX. EXIT.
> ```
>
> 目标位于范围**之外**的 `GO TO` 仍会离开 `PERFORM`，
> 正如标准所要求的——控制不会回来。

> **组就是它的子项。** 组项目本身没有存储：它就是
> 其下各项首尾相接，不论它们是什么，它都是字母数字型，而它的
> 大小是它们大小之和。读取一个组得到整条记录，写入一个组
> 会按宽度把字节分布到各个子项，而对任何子项的更改
> 都会立即在组中显现。`FILLER` 也计算在内——它像任何其他项目一样保存它的字节和
> `VALUE`——并且这个词本身是可选的，所以 `05 PIC X VALUE ":".` 是一个完全合格的分隔符。
> 同一个组下共用一个名称的基本项目也是如此——
> 重复书写 `05 F PIC XX VALUE "AC".` 所构成的经典取值表：每一个都保持
> 自己的字节和 `VALUE`，只有*引用*那个名称才是错误，因为
> 没有任何限定可以区分这些副本：
>
> ```cobol
>        01 EDITED-TIME.
>           05 HH PIC 99.
>           05    PIC X VALUE ":".
>           05 MM PIC 99.
> ```
>
> 当 `HH` = 09 且 `MM` = 30 时，`DISPLAY EDITED-TIME` 显示 `09:30`。这是
> 用若干部分构建格式化字段的常规方式，也是组
> 从不需要自己的 `PIC` 的原因。
>
> ⚠️ **引用修改计的是字符，而不是值。** `T(1:2)` 取 `T` 的
> 前两个*字符位置*，所以一个保存着 `00224845` 的 `PIC 9(8)`
> 得到 `"00"`——前导零是该项目的一部分。正是这一点使
> 经典的拆包（`MOVE T(1:2) TO HH`、`MOVE T(3:2) TO MM`，……）能够对齐。
> 它对表条目的作用方式相同——`ROW(I)(2:3)`——不论该条目是
> 被发送、被接收（`MOVE "AB" TO ROW(I)(2:2)` 只更改那两个位置）
> 还是被检查（`INSPECT ROW(I)(1:3) CONVERTING …` 只触及前三个）。

- **算术：**`ADD/SUBTRACT/MULTIPLY/DIVIDE/COMPUTE`，支持多个接收者
  和每个接收者各自的 `ROUNDED`；数值编辑型 `PICTURE` 编辑。

> **size error 短语保护接收者——两半中的任何一半都行。** 如果
> 语句带有 `ON SIZE ERROR` *或* `NOT ON SIZE ERROR`，一个
> 放不下它结果的接收者会保留它原有的值，而其他
> 接收者仍然得到它们的结果。没有任何 size error 短语时，结果
> 会被截断放进字段。这让人措手不及，因为
> 这种保护读起来像是属于 `ON SIZE ERROR`；它其实属于
> 整个语句。
>
> ```cobol
> ADD  WS-BIG  6  GIVING WS-A WS-B
>      NOT ON SIZE ERROR  MOVE "OK" TO WS-FLAG.
> *>   WS-A and WS-B are unchanged if the sum will not fit them,
> *>   and WS-FLAG stays as it was.
> ```

- **控制流：**`IF/ELSE`、`EVALUATE`（带 `ALSO` 和 `WHEN NOT`）、行内和
  行外的 `PERFORM`（包括 `VARYING`、`UNTIL`、`TIMES`）、`GO TO`、`ALTER`、
  `EXIT PERFORM/PARAGRAPH/SECTION`，以及忠实实现的 `NEXT SENTENCE`。
- **字符串：**`STRING`、`UNSTRING`、`INSPECT`（`TALLYING` + `REPLACING`，带
  `BEFORE/AFTER INITIAL`）、`INITIALIZE … REPLACING`。

> **`UNSTRING` 全貌。** 每个短语都被遵循：`DELIMITED BY [ALL] … OR …`、
> `DELIMITER IN`、`COUNT IN`、`WITH POINTER`、`TALLYING`，以及
> `ON OVERFLOW` / `NOT ON OVERFLOW`。有三个细节值得了解，
> 因为手写的拆包代码通常正是在这里出错：
>
> - **`WITH POINTER` 既被读取*又*被写入。** 扫描从
>   该项目指名的那个字符开始（从 1 开始计数），而该项目最后指向
>   所检查的最后一个字符的下一位，所以下一个 `UNSTRING` 会从这一个停下的地方继续。
>   源之外的指针会引发 overflow，并且什么都不移动。
> - **`ALL` 吞掉整串，但只交付一个。** 对 `"1200000"` 使用 `DELIMITED BY ALL ZERO`，
>   会跳过全部五个零，而 `DELIMITER IN` 收到的是单个 `"0"`。
> - **没有 `DELIMITED BY` 意味着“按大小”。** 每个接收者依次恰好取
>   与它的宽度一样多的字符。
>
> ```cobol
> 01  WS-LINE   PIC X(7) VALUE "1200000".
> 01  WS-FIELD  PIC X.
> 01  WS-DELIM  PIC X(4).
> 01  WS-COUNT  PIC 99.
> 01  WS-PTR    PIC 99  VALUE 1.
> 01  WS-TALLY  PIC 99  VALUE 0.
> ...
>     UNSTRING WS-LINE DELIMITED BY ALL ZERO
>         INTO WS-FIELD DELIMITER IN WS-DELIM COUNT IN WS-COUNT
>         WITH POINTER WS-PTR TALLYING WS-TALLY.
> *>   WS-FIELD = "1"   (the field is "12", cut to one character)
> *>   WS-DELIM = "0"   WS-COUNT = 02   WS-PTR = 08   WS-TALLY = 01
> ```
>
> **`INSPECT … LEADING` / `TRAILING` 计的是完整的模式。** `FOR LEADING "AH"`
> 统计 `"AH"` 从区域的开头*连续*重复了多少次——
> 在 `"AH YES AH YES"` 中是一次，而不是两次，也不是“出现在
> 模式里的字符”。
>
> **一连串 `TALLYING` 操作数共享对字段的一次扫描，而你写下它们的
> 顺序决定了答案。** 字段只被检查一次，从左到右；在每个字符位置，
> 操作数按书写顺序逐个尝试，第一个匹配的占据该位置，而扫描
> 从它取走的字符之后继续。没有任何东西被计两次。
>
> ```cobol
>        01  SUBJ  PIC X(4)  VALUE "AABA".
>            INSPECT SUBJ TALLYING T1 FOR ALL "AA"  T2 FOR ALL "A".
>        *>  T1 = 1, T2 = 1   — "AA" takes positions 1-2, so only the last
>        *>                     "A" is left for T2
>            INSPECT SUBJ TALLYING T1 FOR ALL "A"   T2 FOR ALL "AA".
>        *>  T1 = 3, T2 = 0   — the same statement, operands swapped
> ```
>
> 这让人在使用 `CHARACTERS` 时措手不及——它只计没有被任何
> 更早的操作数占用的位置——并且在使用 `LEADING` 时也是如此，它的连续串必须从
> 区域的第一个位置开始：在它前面放一个在那里匹配的 `ALL` 操作数，
> 那么 `LEADING` 的连续串在开始之前就已经结束了。
>
> **`REPLACING` 的工作方式相同，而它的 `BEFORE`/`AFTER` 分隔符
> 是在任何替换发生之前找到的。** 这正是值得了解的部分：一个
> 操作数可以锚定在较早的操作数所覆盖的字符上，而它
> 仍然能找到它们，因为所有的窗口都是针对字段到达时的样子确定的。
>
> ```cobol
>        01  SUBJ  PIC X(20).
>            MOVE "CAN NOT BE ALL BAD." TO SUBJ.
>            INSPECT SUBJ REPLACING
>                FIRST "L "  BY "ZZ"  AFTER INITIAL "AL"
>                FIRST "BAD" BY "ZZZ" AFTER "L "
>                ALL   "."   BY "Z"   AFTER "AL".
>        *>  SUBJ = "CAN NOT BE ALZZZZZZ"
> ```
>
> 如果每个短语都单独应用于整个字段，第一个就会
> 抹去第二个所锚定的 `"L "`，而 `"BAD"` 仍然会留在那里。
>
> ⚠️ **有符号数值项没有可计数的负号。** `INSPECT` 读取
> 项目实际占据的字符位置，而保存着 `-12345` 的 `PIC S9(5)`
> 占据其中五个，全都是数字——符号作为过打孔跟在某个数字上，
> 而不是它自己的一个字符。所以
> `INSPECT AMT TALLYING T FOR ALL "-"` 得到**零**，而对
> 数字做的 `REPLACING` 不会触及符号。如果你希望符号成为一个字符位置，
> 请声明 `SIGN IS LEADING SEPARATE`；那时它就像任何其他字符一样被计数。
> 这是标准的 COBOL 行为，也是校验例程试图通过查找 `"-"` 来发现负数时
> 常见的意外。

- **表：**`SORT` / `MERGE`（带 `INPUT`/`OUTPUT PROCEDURE`、`USING`/`GIVING`、
  `RELEASE`/`RETURN`）；`SEARCH`（顺序）和 `SEARCH ALL`（对带
  `ASCENDING`/`DESCENDING KEY` 的表做二分查找）。
- **子程序：**`CALL … USING`（带 `ON EXCEPTION` / `NOT ON EXCEPTION`）、
  `CANCEL`、`GOBACK`/`EXIT PROGRAM`、嵌套程序。
- **错误处理：**带 `USE AFTER STANDARD ERROR PROCEDURE` 的 `DECLARATIVES`，
  用于集中处理文件错误。
- **内部函数：**标准的 `FUNCTION` 库，包括日期/时间
  和财务函数。
- **屏幕 ACCEPT/DISPLAY**，用于字符模式的交互（当你不是在
  构建窗口化的窗体时）。
- **范围终止符：**COBOL-85 的整套（`END-IF`、`END-PERFORM`、`END-READ`、
  `END-EVALUATE`、`END-STRING` 等等），外加 `END-ACCEPT` 和
  `END-DISPLAY`。它们每一个都是可选的——句点同样可以结束
  语句——但 `END-DISPLAY` 是唯一可能改变一行含义的那个，
  因为它结束**操作数列表**：

  ```cobol
           DISPLAY "A" END-DISPLAY
           DISPLAY "B".
  ```

  是两条语句。没有终止符时，`DISPLAY` 会一直运行到遇到句点
  或它认识的短语为止，所以把这两条写在不同的行上，
  二者之间既没有终止符也没有句点，就会使 `"B"` 成为第一个
  `DISPLAY` 的第三个操作数。如果你习惯于显式地结束每个动词，那么这个习惯
  在这里原样适用。

> **事实依据。** 受支持语法的权威、始终最新的列表是
> `docs/cobol85-supported-syntax-en.md`；逐动词的测试矩阵是
> `docs/cobol85-verb-test-matrix-en.md`。有疑问时，这些文件（以及测试
> 套件）才是定论。

> ⚠️ **范围之外（目前）：**跨进程的**记录**锁定，以及面向对象的
> `CLASS`/`METHOD` 定义，尚未实现。（以 `WITH LOCK` 或 `SHARING WITH NO OTHER`
> 打开的整个 INDEXED 文件*确实*是跨进程独占的——
> 另一个程序对它的 `OPEN` 会得到状态 `93`；记录锁是
> 单进程的。）**RELATIVE 文件
> 组织已实现**——见
> [按编号寻址记录：`ORGANIZATION IS RELATIVE`](#按编号寻址记录organization-is-relative)。

### 代码风格

PowerRustCOBOL AI 给 COBOL-85 增加了少数几个扩展，而它们都做同一件事：
它们免去了普通 COBOL 逼你声明的**临时字段**，那些字段只是用来把一个值
从一处搬运到下一处。请用它们来写。一个使用它们的
RustCOBOL 程序，长度只有原来的一小部分，需要发明并保持区分的
名称也更少。

**唯一的规则：不要声明一个 working-storage 项目，如果它唯一的作用只是搬运
一个值。** 如果这个值可以在原处被读取、测试或移动，就在原处做。

| 普通 COBOL-85 逼你……… | RustCOBOL 让你写 |
|---|---|
| 声明一个项目，把属性 `MOVE` 进去，然后测试该项目 | `IF NUD-AGE::Value > 17` |
| 把结果 `MOVE` 进一个临时项，在它上面做算术，再 `MOVE` 回去 | `COMPUTE SLD-1::Value = SLD-1::Value * 2` 或 `ADD 1 TO NUD-AGE::Value` |
| 在使用之前，先把方法的答案接收到一个项目中 | `IF LST-1::GetCount() > 1` |
| 为清理文本编写 `INSPECT … REPLACING` 或 `UNSTRING` | `MOVE WS-NAME::Replace(", MD", ", M.D.") TO WS-NAME` |
| 写 `CALL "COBOL-OPEN-DB" USING …` | `COBOL::"OPEN-DB" ( ":memory:" WS-DB WS-ST )` |

下面描述的每一种行为，都有一个在测试套件下运行的程序来检验——
`tests/cobol/code-style/test-code-style.cbl`（13 种
语法形式上的 30 个用例）和 `tests/cobol/string-ops/test-data-item-methods.cbl`（16 种
语法形式上的 32 个用例）——所以这些示例展示的就是编译器和运行时
实际所做的。

#### 属性就是数据项

控件的属性——`NUD-AGE::Value`、`PERSON::Name`、`LBL-1::Caption`、
`CHK-1::Checked`——可以用在**任何 working-storage 项目可以出现的地方**，无需
声明任何东西。编译器从值就知道它是什么：数字在算术和比较中就是
数字，文本就是文本。这就是这门语言所围绕的*类型
推断*，也是它让读取和写入控件看上去像 COBOL 的原因。

```cobol
      *> read it — IF, EVALUATE, PERFORM UNTIL, STRING, DISPLAY, a MOVE source
           IF PERSON::Age > 18
               DISPLAY "Not a minor"
           END-IF
           EVALUATE PERSON::Age
               WHEN 7 DISPLAY "seven"
           END-EVALUATE
           PERFORM UNTIL PERSON::Age >= 25
               ADD 1 TO PERSON::Age                 *> ... and write it back
           END-PERFORM
           STRING "Name: " PERSON::Name DELIMITED BY SIZE INTO WS-LINE

      *> write it — MOVE, SET, COMPUTE and ADD all take a property as target
           MOVE 17 TO PERSON::Age
           COMPUTE PERSON::Age = PERSON::Age + 2

      *> property to property — no intermediate item
           MOVE PERSON::Name TO LBL-B::Caption

      *> a Boolean property takes TRUE and FALSE
           MOVE TRUE TO CHK-1::Checked
           IF CHK-1::Checked = TRUE
               DISPLAY "ticked"
           END-IF
           SET CHK-1::Checked TO FALSE
```

比较一下没有它的同一个年龄测试：`MOVE NUD-AGE::Value TO WS-AGE`，然后
`IF WS-AGE > 17`——还有一个要命名、要用正确的 `PIC` 声明、
要与控件保持同步、并在界面更换时被遗弃的 `WS-AGE`。这个
扩展并不使第二种形式错误；它使它变得不必要。

> **注意。** `::` 之后的是控件自己的成员。请在
> 控件目录（§8）中查找这些名称，而不要猜测；不存在的**控件**
> 在 Check 和 Build 时是错误（见 §11 中的*从 COBOL 与界面对话*），
> 而图层只回答它自己的少数几个属性。

#### 返回值的方法就是表达式

回答一个值的方法，可以用在需要该值的地方——在
条件中、算术中、作为 `MOVE` 的来源——中间不需要任何接收项目：

```cobol
           IF LST-1::GetCount() > 1
               DISPLAY "more than one item"
           END-IF
           COMPUTE WS-N = LST-1::GetCount() * 10
           INVOKE NUD-AGE::Increment()               *> nothing to receive: INVOKE
```

当方法回答的是一条**记录**——记录的固定布局文本——时，
把它直接 `MOVE` 进描述它的 01 级组。它会按位置落入
该组的子项，恰如对组的任何 `MOVE` 一样，而且不需要
单独的“记录缓冲区”项目：

```cobol
       01 WS-RECORD.
          05 WS-REC-ID    PIC 9(4).
          05 WS-REC-NAME  PIC X(20).
      ...
           MOVE TXT-REC::GetText() TO WS-RECORD     *> "0042Gregory House"
           DISPLAY WS-REC-ID " " WS-REC-NAME         *> 0042 Gregory House
```

只有当你希望答案保存在一个项目中以备日后使用时，才用 `INVOKE obj::Method(args) RETURNING item`；
如果只用一次，就把调用放在它被使用的地方。

#### 属性直接传入调用

调用的参数是值，在调用的那一刻被读取，而
属性就是一个值。所以调用从不需要一个唯一的作用就是把
属性搬运给它的项目。同一个调用，三种写法——输入在
`TXT-SQL` 中的 SQL 文本，由 `DB-LOCAL` 数据库运行：

```cobol
      *> avoid — WS-SQL exists only to carry the text from the control to the call
           MOVE TXT-SQL::Text TO WS-SQL
           INVOKE DB-LOCAL "Query" USING WS-SQL RETURNING WS-N

      *> better — the property is the argument; WS-SQL is gone
           INVOKE DB-LOCAL "Query" USING TXT-SQL::Text RETURNING WS-N

      *> write this — the inline form reads like the call it is
           SET     WS-N                          TO    DB-LOCAL::Query(TXT-SQL::Text)
```

`INVOKE obj "Method" USING a b RETURNING r` 就是 `SET r TO obj::Method(a b)`；如果
没有需要保留的答案，它就是单独一条语句 `obj::Method(a b)`。值
方法可以在参数传入的途中清理它——
`LST-1::AddItem(TXT-A::Text::Trim())`——而方法的答案也可以是另一个调用的参数。

要留意三件事：

- **用空格分隔参数，但凡是一个参数
  可能被读成前一个参数的延续之处，就用逗号。** `LST-1::AddItem(WS-A -1)`
  是一个参数，即 `WS-A` 减 1；要传递负数，请写
  `OBJ::Method(WS-A, -1)`。带引号的字面量、标识符和属性用空格是安全的。
- **只有控件拥有的方法才能内联书写。** 控件
  不回答的名称会被读作下标，并且什么也不做——悄无声息。请在写之前
  在控件目录（§8）中查找它。
- **对窗口本身的调用仍然使用 `INVOKE`。** `INVOKE ME::OpenFormSync("X")`、
  `INVOKE ME::OpenFormAsync("X") RETURNING WS-H`（接收
  *windowHandler* 的唯一方式）、`INVOKE ME::SetProperty(…)`、`INVOKE ME::GetProperty(…)
  RETURNING …`、`INVOKE ME::Close()`，以及在 `super` 上的同样调用。窗口监管者
  运行它们，而 ✨ Beautify 保持它们原样。你传给它们的内容
  仍然直接传入：`INVOKE ME::SetProperty("ConfirmText", TXT-Q::Text::Trim())`。

当一个值是真正的工作值时，请保留一个项目——被使用多次、
被累加、在某件事改变它之前被取为快照，或是一个
不得再次被读取的循环界限。

#### 普通数据项也有方法

一个 `PIC X` 项目（以及组、表的某个元素、切片）回答一小组固定的
**值方法**：`Trim`、`Upper`、`Lower`、`Replace`、`Len` 和
`Split`。它们返回一个值，可以**链接**，并且同样适用于属性：

```cobol
       WORKING-STORAGE SECTION.
       01 person-name    PIC X(40) VALUE "Dr. House, MD".
      ...
       PROCEDURE DIVISION.
           MOVE person-name::Replace(", MD", ", M.D.")  TO person-name
           MOVE person-name::Trim()::Upper()            TO WS-HEADING
           MOVE PERSON::Name::Upper()                   TO LBL-B::Caption
           IF PERSON::Name::Len() > 5
               DISPLAY "a long name"
           END-IF
```

完整的表格、每个方法的两个完整示例，以及需要留意的要点
（填充、只读、其他任何名称的错误）都在下一小节
*数据项的值方法*中。这些方法不涵盖的任何事情——在文本中查找
文本、反转、填充——都是标准 COBOL 的工作：引用修改
`WS-A(start:length)`、`INSPECT`、`STRING` / `UNSTRING` 以及
内部函数。

#### COBOL-85 要求标识符的地方，可以写表达式

标准在大多数发送位置只允许标识符或字面量。
RustCOBOL 在那里计算整个表达式，所以一行计算既不需要
临时项目，也不需要 `COMPUTE`：

```cobol
           MOVE WS-N * 2 TO WS-CNT                   *> instead of COMPUTE + MOVE
           SET WS-CNT TO WS-CNT + 1                  *> SET an item TO an expression
           SET LBL-A::Caption TO WS-TEXT::Trim()     *> a property, from a method result
           STRING WS-N * 3 DELIMITED BY SIZE         *> an expression as a STRING source
                  " items" DELIMITED BY SIZE
               INTO WS-LINE
```

#### 不写 `COMPUTE` 的赋值

`COMPUTE` 是唯一一个动词什么都没增加的算术动词：它的 `=` 已经说明了。
以接收者和 `=` 开头的语句就是 `COMPUTE`，
含义与它完全相同：

```cobol
           TOTAL = PRICE * QTY
           TOTAL ROUNDED = PRICE * QTY
               ON SIZE ERROR
                   DISPLAY "Too large"
           END-COMPUTE
```

与 `COMPUTE TOTAL = PRICE * QTY` 是同一个程序，其中 `ROUNDED`、`ON SIZE
ERROR`、`NOT ON SIZE ERROR` 和 `END-COMPUTE` 的作用与在那里相同。接收者
是数据名，可以带限定（`F OF G`）、带下标
（`T(I, J)`）或带引用修改，也可以是控件的成员（`Txt-A::Text = WS-N + 1`）。
这种简写只取**一个**接收者；`COMPUTE A B = X` 仍然是书写
多个接收者的方式。调试器可以在这样的行上设置断点，Beautify 会像排版
任何其他语句一样排版它（它的 `ON SIZE ERROR` 缩进一级，`END-COMPUTE` 回到
它的下面），而解释器、编译器和生成的代码看到的只是一个
`COMPUTE`。

> ⚠️ **注意事项。**
> - 它是**算术**。对文本项写 `NAME = "x"` 并不是其他语言里的文本赋值：
>   它是 `COMPUTE`，而检查会拒绝它，报告
>   *"'NAME' is not numeric; an arithmetic assignment (COMPUTE, or `name =
>   expression`) requires a numeric receiver"*。对文本请使用 `MOVE` 或 `SET`。
> - 同时是保留字的数据名（`COUNT`、`DATE`）不会被读作
>   接收者；这一行会给出通常的错误，`MOVE` 对这样的名称也是如此。
> - 以 `NAME = …` 开头的一行也可能是条件的第二行
>   （`IF A > 1 AND` / `B = 2`）。解析器根据前面的内容把它们区分开；
>   当上一行以 `AND`、`OR`、`NOT` 或运算符结尾时，Beautify 和调试器会查看上面那一行，
>   并把它视为延续。
> - 它在 COBOL-85 之外，所以 NIST 套件从不使用它，而标准的
>   标记通道（它读取的是记号，而不是语句）也不会报告它。
> - 它不是风格规则所偏好的属性赋值：对于控件的
>   属性，`SET Txt-A::Text TO …` 和 `MOVE … TO Txt-A::Text` 仍然是
>   应当书写的形式。

#### 内置调用以内联方式书写

运行时的内置调用（HTTP、SQL、文件、对话框、密钥、模型、图表）
是 `COBOL` 对象的方法。请这样写它们——绝不要写成 `CALL
"COBOL-…" USING …`；二者是同一个调用，但内联形式说明了它是什么，
并且 IDE 会为它补全：

```cobol
           COBOL::"OPEN-DB"  ( ":memory:" WS-DB WS-ST )
           COBOL::"EXEC-SQL" ( WS-DB "SELECT A FROM T" WS-ROWS WS-ST )
           COBOL::"FETCH-ROW" ( WS-DB 1 WS-VAL WS-ST )
           COBOL::"CLOSE-DB" ( WS-DB )
```

列表见 §11 中的*内置调用：`COBOL` 对象*。你自己的通用过程仍然用 `CALL "PROCEDURE-NAME"` 到达。参数
以空格分隔，参数之间也接受逗号——`Replace("," ";")`
与 `Replace(",", ";")` 是同一个调用。

#### 规则汇总

1. **不要只为搬运一个值而声明字段。** 在使用该值的地方读写属性，
   或调用方法。
2. **把属性当作数据项**：`IF`、`EVALUATE`、`PERFORM UNTIL`、
   `COMPUTE`、`ADD … TO`、`MOVE`、`SET`、`STRING`、`DISPLAY`。
3. **把方法调用放进需要它的表达式里**；对于不需要答案的方法，
   把它作为独立语句来调用，不带 `INVOKE`
   （`Txt-Log::AppendText(WS-NL)`）；只有为了保留答案才用 `INVOKE … RETURNING`——
   而且那时要写成 `SET item TO obj::Method(args)`。对 `me` 和 `super` 的调用保留
   `INVOKE`。
4. **把返回的记录 `MOVE` 进它的 01 组**；不要手工拆开它。
5. **链接值方法**（`::Trim()::Upper()`），而不是为文本清理
   写 `INSPECT` 或临时项目。
6. **使用表达式**，在 `MOVE`、`SET` 和 `STRING` 中，凡是计算原本
   会需要 `COMPUTE` 和一个临时项的地方。
7. **内联书写内置调用**：`COBOL::"NAME" ( … )`。
8. 布尔属性用 **`TRUE` 和 `FALSE`**；图层用 `SET LAYER-NAME::Visible TO TRUE`。
9. **无论 IDE 使用什么语言，数据项和段落名称都保持英文**，并且
   用它的 `END-` 范围终止符结束每个动词。
10. **当扩展不涵盖这项工作时，请求助于标准 COBOL**——
    `INSPECT`、`UNSTRING`、引用修改、内部函数——
    而不是向数据项索要它没有的方法：数据项之后的任何其他
    名称都是**错误**。
11. **把属性作为参数传入**，绝不要经过临时项：
    `Txt-Log::AppendText(RestClient-1::BaseURL::Trim())`，而不是先 `MOVE` 进
    `WS-LINE` 再调用。参数之间用空格分隔，当某个参数以符号或圆括号开头时，
    用逗号分隔。
12. **只写控件拥有的方法。** 内联时，任何其他名称都什么也不做。

#### 布局与注释

同样的布局，就是 PowerRustCOBOL AI 插件要求编码智能体书写的布局
（`layout` 黄金规则），也是 **✨ Beautify** 替你写出的布局（见下面的
*Beautify — 布局规则*），所以你从智能体那里读到的代码和你手写的
代码看起来一样。

**在复杂代码之前写注释**——计算、带退出条件的循环、
有多种结果的 `EVALUATE`、业务规则、变通办法。几行
`*>`，说明代码*为什么*在那里以及它必须达成什么，而不是每个
动词做什么。注释块与它下面的代码从同一列开始，前面
有一个空行（如果上面一行已经是空行，就不再多加），后面也有一个
空行。

1. **每级缩进 4 个空格。**
2. **每个 `DIVISION` 和 `SECTION` 之前有一个空行**，`ENVIRONMENT DIVISION`
   除外。
3. **动词和它的谓语之间有 4 个空格**——每个动词都是如此：`DISPLAY    "A"`、
   `IF    WS-N > 1`、`PERFORM    SHOW`。`SET` 用 5 个，这样 `SET` 的谓语
   与上面或下面 `MOVE` 的谓语对齐。由两个词构成的动词
   （`EXIT PERFORM`、`STOP RUN`、`GO TO`）算作一个动词。
4. **在 `MOVE` 和 `SET` 中，`TO` 位于第 50 列**，后面跟 4 个空格。当它前面的
   操作数太长而到不了第 50 列时，把 `TO` 放在下一行的第 50 列，
   并在该语句之后留一个空行。
5. **在 `PROCEDURE DIVISION` 中，句点只用在需要的地方**：结束
   `SECTION` 或段落名称之前的最后一条语句的那一个，以及结束代码的那一个。其他每个动词
   都用它的 `END-<verb>` 来结束。
6. **在使用同一个动词的一组连续语句之前和之后各有一个空行。**
7. **方法本身就是一条语句**——`Txt-Log::AppendText(WS-NL)`，而不是
   `INVOKE Txt-Log::AppendText(WS-NL)`。
8. **优先用 `SET` 而不是 `MOVE`**；`MOVE` 留给移进它的 01 组的记录。
9. **在数据条目中，名称之后的第一个子句从第 50 列开始**——
   与 `TO` 是同一条列线：基本项目的 `PIC`、表的 `OCCURS`、88 级的
   `VALUE`。它后面跟 4 个空格和它的操作数，之后的任何
   子句再隔 4 个空格。数据级别缩进 4 个空格
   （`01` → `05` → `10`，88 级在它的项目之下缩进一级）。名称太长而到不了
   那条列线时，把它的子句放到下一行的第 50 列，并在其后留一个空行。

```cobol
       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-NL                                  PIC    X.

       PROCEDURE DIVISION.
           SET     WS-NL                         TO    FUNCTION CHAR(11)
           SET     VWR-1::Layout                 TO    "Page"
           SET     VWR-1::SplitMode              TO    "None"
           SET     VWR-1::View1ViewMode          TO    "Full"
           SET     VWR-1::View1Zoom              TO    100
           SET     VWR-1::FontSize               TO    14

           *> Something to look at before anything is picked. The sample
           *> ships with the project, so the demo does not depend on where
           *> the repository happens to sit.

           SET     VWR-1::View1Source            TO    "assets/docs/viewer-sample.md"
           SET     Lbl-Status::Caption           TO    "Ready. Pick a file, or press Start stream.".

       SETTING-VIEWER.
           Txt-Log::AppendText("Viewer demo ready.")
           Txt-Log::AppendText(WS-NL).
```

`tests/cobol/code-style/test-layout-style.cbl` 是一个用这种布局编写、
由测试套件运行的程序；一个 lint 对照规则 2 到 4、7 和 9 以及注释规则检查它，
也检查插件向智能体展示的范本，另一个
测试保证 ✨ Beautify 让这个范本保持原样。

#### 每个 `CALL` 之前的注释

`CALL` 运行位于别处的过程——在同一个窗体中，或在
Common Code 中——所以调用它的那一行并不告诉你即将发生什么。
请在每个 `CALL "NAME"` 的正上方写一条简短的注释，按这个顺序回答三个
问题：

- **这个过程将做什么。**第一行给它命名并说明这一点。
- **`Uses:`**——它读取的数据项和控件属性，以及在该次调用处，
  当上面各行设置了它们时，每一项所持有的值。
- **`Result:`**——它留下被更改的数据项、控件属性和文件，
  以及如何更改。没有更改时写 `No data item changes.`。

这个块的布局与任何其他注释块一样：与 `CALL` 在同一列，
前面有一个空行，后面有一个空行，最宽 100 列。

```cobol
           SET     WS-SET-NAME                   TO    "LANG"

           *> PS-SETTING-GET: reads one setting from the settings file by its name; it changes none.
           *> Uses:   WS-SET-NAME = "LANG" (set just above; it is the record key) and the file.
           *> Result: WS-SET-VALUE = the saved language code ("pt", "es", ...), or SPACES when
           *>         the setting was never kept or the file cannot be opened.

           CALL    "PS-SETTING-GET" USING WS-SET-NAME WS-SET-VALUE.
```

插件把同样的做法作为 `call-comments` 黄金规则交给编码智能体，所以智能体写的代码
和你写的代码读起来一样，而随产品附带的
示例——PowerChat、PowerAnalytics、PowerSpatial、
PowerCloud 和 PowerDemo3——也是这样写的。

**说明**

- 请根据过程*做什么*来写，而不是根据它叫什么名字，并且每次调用写
  一个：从五个地方调用的同一个过程有五个块，每一个
  都指明那次调用的项目以及它们在那里所持有的值。
- 内联书写的内置调用 `COBOL::"NAME" ( … )` 不是对
  过程的 `CALL`，不需要块。写在同一行的 `WHEN` 或 `NOT INVALID KEY`
  之后的 `CALL`，由该行上方的一个块来注释。
- 已经存在的注释保持原样：把块添加在它和
  `CALL` 之间。

**⚠️ 注意事项**

- 错误的块比没有更糟。当你更改过程读取
  或留下的内容时，请更改每个调用它的地方上方的块。
- 在 `.cfrm` 中，COBOL 位于 XML 的 `CDATA` 段中，而 `]]>` 会结束该段，所以
  绝不要在注释中写这三个字符。

### 数据项的值方法

一个 `PIC X` 字段——或组、表的某个元素、做过引用修改的切片——
可以用与你在控件上使用的同一个 `::`，来询问它的**值**。这是 PowerRustCOBOL 的一项便利，而不是
标准 COBOL：它替代日常文本工作中所需的临时字段以及 `INSPECT` 或 `FUNCTION`。
在任何允许表达式的地方使用它——`MOVE`、`COMPUTE`、`IF`、`DISPLAY`、`STRING`
——并且可以**链接**调用：`WS-NAME::Trim()::Upper()`。

数据项回答**这些，也只回答这些**：

| 方法 | 给出 | 它做什么 |
|--------|-------|--------------|
| `Trim()` | 文本 | 去掉两端的空格。 |
| `Upper()` | 文本 | 大写字母。`UpperCase()` 和 `ToUpperCase()` 是被接受的别名。 |
| `Lower()` | 文本 | 小写字母。`LowerCase()` 和 `ToLowerCase()` 是被接受的别名。 |
| `Replace(from, to)` | 文本 | 每一处 `from` 都变成 `to`。 |
| `Len()` | 数字 | **字段的声明长度**——保存着 `hello` 的 `PIC X(20)` 回答 20。`Length()` 是被接受的别名，而 `Length` 也可以不带括号地读取。 |
| `Split(sep)` | 文本 | 第一个 `sep` 之前的那一段。 |
| `Split(sep)(n)` | 文本 | 第 *n* 段，从 1 开始计数。 |

在编辑器中，在数据项之后键入 `WS-A::` 会打开 IntelliSense，其中恰好是
这些——上面的方法，以及作为唯一属性的 `Len`——在方法之后（`WS-A::Trim()::`）
也是同样的列表；它从不为数据项提供控件的方法
（`Show`、`Hide`、`MoveTo` ……）。

每个方法两个示例，以及它们给出的结果：

```cobol
       01 WS-A    PIC X(20).
       01 WS-CSV  PIC X(30).
       01 WS-OUT  PIC X(40).
       01 WS-N    PIC 9(4).

      *> Trim()
           MOVE "   Hello   " TO WS-A
           MOVE WS-A::Trim() TO WS-OUT              *> Hello
           MOVE "  a b  " TO WS-A
           MOVE WS-A::Trim() TO WS-OUT              *> a b   (the inner blank stays)

      *> Upper()
           MOVE "abc def" TO WS-A
           MOVE WS-A::Upper() TO WS-OUT             *> ABC DEF
           MOVE "a1b2" TO WS-A
           MOVE WS-A::Upper() TO WS-OUT             *> A1B2

      *> Lower()
           MOVE "MiXeD 123" TO WS-A
           MOVE WS-A::Lower() TO WS-OUT             *> mixed 123
           MOVE "HELLO" TO WS-A
           MOVE WS-A::Lower() TO WS-OUT             *> hello

      *> Replace(from, to)
           MOVE "Hello World" TO WS-A
           MOVE WS-A::Replace("World" "COBOL") TO WS-OUT   *> Hello COBOL
           MOVE "a-b-c" TO WS-A
           MOVE WS-A::Replace("-" " ") TO WS-OUT           *> a b c

      *> Len() — the field's length, not its content's
           COMPUTE WS-N = WS-A::Len()               *> 20   (PIC X(20))
           COMPUTE WS-N = WS-CSV::Len()             *> 30   (PIC X(30))
           MOVE "  hi  " TO WS-A
           COMPUTE WS-N = WS-A::Trim()::Len()       *> 2    (the content's length)

      *> Split(sep) and Split(sep)(n)
           MOVE "2024-10-09" TO WS-CSV
           MOVE WS-CSV::Split("-") TO WS-OUT        *> 2024
           MOVE WS-CSV::Split("-")(3) TO WS-OUT     *> 09
           MOVE "a-b-c" TO WS-CSV
           MOVE WS-CSV::Split("-")(2) TO WS-OUT     *> b
```

它们也适用于切片和表的元素——`WS-A(5:3)::Upper()`、
`WS-ROW(2)::Trim()`——以及条件之内：`IF WS-A::Trim() = "yes"`。

> **说明。**
> - 结果保留字段的**填充**。`Upper`、`Lower` 和 `Replace` 返回
>   整个带填充的字段，而 `Split` 的最后一段带着跟在它后面的
>   空格；当空格有关系时，请把调用包在 `::Trim()` 里。
> - 它们适用于代码能够指名的**每一个数据项**：窗体自己的项目、
>   **事件处理程序**或通用过程在它自己的
>   `WORKING-STORAGE` 中声明的项目（`LK-FILE::Trim()`，即使另一个处理程序声明了
>   同名的项目）、`GLOBAL` 项目和 `LINKAGE` 参数——以及
>   通过限定到达的项目，`WS-FLD OF REC-A::Trim()`。答案与
>   `FUNCTION TRIM(LK-FILE)` 给出的相同，所以这两种写法可以互换。
> - 这些方法是**只读的**：`MOVE x TO WS-A::Upper()` 会被拒绝，
>   因为方法的结果不是放东西的地方。要更改字段本身，请用
>   `MOVE WS-A::Upper() TO WS-A`。
> - 字面量不能作为接收者（`"a-b-c"::Split("-")` 是语法错误）；
>   请先把它放进数据项。

> **IDE 替你写出唯一的拼法。**当你保存文件、处理程序或
> 过程——或运行 Beautify——时，数据项之后的旧拼法
> （`UpperCase`、`ToUpperCase`、`LowerCase`、`ToLowerCase`、`Length`）会变成
> `Upper()`、`Lower()` 和 `Len()`；Output 面板用一行话说明这一点
> （*Value methods: 3 spellings changed to Upper()/Lower()/Len() in …*），而一次
> Undo 就能还原你键入的内容。尚不能编译的代码保持你键入的原样，
> IDE 不能确定是数据项的任何东西——控件
> 自己的属性、字符串、注释——也是如此。无论哪种情况，程序运行都一样。

> **⚠️ 注意事项——数据项之后的任何其他名称都是错误。** `WS-A::Contains("x")`、
> `WS-A::Reverse()`、`WS-A::Value` 和 `WS-A::Size` 不存在，它们过去
> 会一声不响地给出空字符串，所以一个输入错误会接着破坏
> 接收它的任何东西。它们现在被拒绝：Check 和 Build 报告 *a data item
> has no method or property 'CONTAINS'* 并列出上表中的名称，
> 而只在运行时才出现的名称，会以同样的消息停止程序。表中没有涵盖的任何事情，
> 请使用标准工具：
> 引用修改 `WS-A(start:length)`、`INSPECT`、`STRING` / `UNSTRING`，
> 以及内部函数（`FUNCTION REVERSE`、`FUNCTION TRIM`、
> `FUNCTION LENGTH` ……）。控件、图层和 `OBJECT REFERENCE` 保留
> 它们自己的成员。

### 按标准允许的写法来写

COBOL-85 允许几种拼法，PowerCOBOL 或 isCOBOL 开发者的手指
早已熟悉它们。所有这些都能工作，而没有一个是必需的。

**逗号和分号只是装饰。**后面*跟着空格*的 `,` 或 `;` 是
一个**分隔符**：它可以出现在空格可以出现的任何地方，含义与
空格完全相同。对编译器来说，这四行是同一条语句：

```cobol
       MOVE ZERO TO DN3, DN4.
       MOVE ZERO TO DN3 DN4.
       CALL "SUB" USING TABLE-1, TABLE-2, DN1.
       READ CUSTOMER-FILE ; AT END GO TO EOF-ROUTINE.
```

> ⚠️ **后面没有空格的逗号是另一回事。**十进制逗号
> （`DECIMAL-POINT IS COMMA` 下的 `1,5`）和 PICTURE 编辑逗号
> （`PIC ZZ,ZZ9.99`）正是靠这一点才能继续工作。这条规则是标准自己的：
> 分隔符逗号是后面*跟着空格*的逗号。

**编辑型图片仍然是数值项。** `Z`、`*` 以及浮动的 `$`、`+`
或 `-` 都是数字位置，所以数值编辑项是 `COMPUTE`、`ADD`、`SUBTRACT`、`MULTIPLY`
和 `DIVIDE … GIVING` 的合法接收者——编辑结果就是声明一个这样的项目的理由。
编辑点之后可以跟单个数字，而图片也可以完全不带 `9`：

```cobol
       01  DIV9        PICTURE IS ZZ,ZZZ.9.
       01  NET-PAY     PIC $**.**CR.
       01  RUNNING-QTY PIC ZZZZ.
           DIVIDE GROSS BY 12 GIVING DIV9.
           SUBTRACT TAX FROM GROSS GIVING NET-PAY.
```

> **注意。**值以它的*编辑后*形式存储，所以接收者读回的
> 是你在报表上看到的字符。当你两者都需要时，请用普通数值项做计算，
> 再把结果移进编辑项。

**检查保护（`*`）在值为零时填满整个字段。**这就是它在支票或汇款行上的
用意——没有任何东西可以写进
空白处。每个字符位置都变成星号，只有小数点
例外，这包括固定的 `$` 以及尾部的 `CR` 或 `DB`：

```cobol
       01  NET-PAY  PIC $**.**CR.
           MOVE ZERO TO NET-PAY.     *> ***.****
           MOVE -2.34 TO NET-PAY.    *> $*2.34CR
```

第二行是普通情形：值不为零时，只有*前导零*受到保护，所以固定的
`$` 保持它自己的位置，而 `CR` 因为值为负而打印出来。第一次使用时，
值得对照字段的声明宽度检查一个零——`PIC $**.**CR` 是八个字符
位置，因为 `CR` 占两个。

**货币符号由你来选。** `SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal` 指定填充货币位置的字符，而每条图片
规则随后都适用于那个字符而不是 `$`——包括浮动的连续符号，
重复的符号会向右漂移，贴着第一个有效数字：

```cobol
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           CURRENCY SIGN IS "£".
       ...
       01  INVOICE-TOTAL  PICTURE £(3),£££.99.
           MOVE 1234 TO INVOICE-TOTAL.      *> reads  £1,234.00
           MOVE ZERO TO INVOICE-TOTAL.      *> reads       £.00
```

> ⚠️ **它替代 `$`，而不是与它并存。**一旦程序声明了货币
> 符号，`$` 在该程序中就不再是图片字符，仍然使用它的图片会被拒绝。
> 如果你在移植一个混用两者的程序，
> 请在同一次编辑中更改每一个图片。
>
> 这个字面量是一个字符，而且标准排除了任何会
> 与图片字符或分隔符冲突的字符：不能是数字，不能是
> `A B C D E G N P R S V X Z` 之一，也不能是 `space * + - , . ; ( ) " / =` 中的任何一个。

**数值接收者恰好保存它声明的位数——两端都是。** `MOVE`
按小数点对齐，然后丢掉放不下的部分。
低位一端是大家熟悉的；高位一端同样被悄悄切掉：

```cobol
       01  M   PICTURE 99V999.
       01  W   PICTURE 9999V9.
           MOVE 123.45 TO M.        *> 23.450  — the hundreds digit is gone
           MOVE 123.45 TO W.        *> 123.4   — the hundredths digit is gone
```

> ⚠️ **这是静默的。**什么也不会报告，因为标准把它定义为
> 结果而不是错误。如果丢失高位数字在你的程序中会是
> 缺陷，请把接收者声明得足够宽——或者使用带 `ON SIZE ERROR` 的算术
> 语句，它会*先*测试接收者的容量，
> 并让它保持不变。

**`P` 移动小数点而不存储数字。**图片中的 `P` 是项目*跨越*但
不*保存*的数字位置——当字段记录的是千位或千分位，
而尾部或前导的零会浪费字节时很有用：

```cobol
       01  IN-HUNDREDS  PICTURE S999PP.     *> 3 digits, value × 100
       01  IN-TEN-THOUS PICTURE PP99.       *> 2 digits, value ÷ 10 000
           MOVE 12300 TO IN-HUNDREDS.       *> stored exactly
           MOVE 12345 TO IN-HUNDREDS.       *> stored as 12300
```

> **注意。** `P` 所代表的位置总是读回零，并且它们
> 占**零字节**——`PIC S999PP` 在记录中是三个字符位置，
> 而不是五个。比较和算术使用缩放后的值，所以
> 上面 `IF IN-HUNDREDS = 12300` 为真。

**`REDEFINES` 是对同一批字节的第二种读法，而不是第二个字段。**重新定义的
项目不给记录增加任何东西：它描述的是它的目标
已经拥有的存储，并且通过任一种描述所做的写入，都会立刻通过
另一种描述可见——也会通过位于它们两者之上的组可见。这是
报表程序赖以构建的惯用法：

```cobol
       01  TEST-CORRECT.
           02  FILLER      PIC X(17) VALUE "       CORRECT =".
           02  CORRECT-X.
               03  CORRECT-A               PIC X(20) VALUE SPACE.
               03  CORRECT-N REDEFINES CORRECT-A  PIC -9(9).9(9).
           ...
           MOVE 242.4332220110 TO CORRECT-N.
           MOVE TEST-CORRECT   TO PRINT-REC.   *> the edited number is there
```

> ⚠️ **注意事项——非常大的覆盖。**让两种描述保持同步，代价是
> 每次写入时对两者各扫一遍。超过 256 个存储位置——例如重新定义的
> 10×10×10 表——PowerRustCOBOL 就会停止镜像，并给每种
> 描述它自己的存储，因为在每次 `MOVE` 上刷新一千个元素
> 会使程序无法使用。请重新定义记录，而不是大型
> 表；如果你需要表的两种读法，请在它们之间显式地 `MOVE`。

**重新定义的描述不必有名称。**大型机的布局经常用没有名字的组
来重新描述字段，这样只有各个部分才有名称：

```cobol
       01  IN-RECORD.
           02  IN-DATE                     PIC X(8).
           02  FILLER REDEFINES IN-DATE.
               03  IN-DATE-YYYY            PIC X(4).
               03  IN-DATE-MM              PIC XX.
               03  IN-DATE-DD              PIC XX.
```

于是 `MOVE "20260828" TO IN-DATE` 使 `IN-DATE-MM` 读作 `08`。子项
**按布局顺序**分割目标的字节，恰如它们
在具名组下那样——没有名称的覆盖是一种描述，而不是它的第一个子项的
另一个名字。

> **注意——同一个字段的两个覆盖都从它的第一个字节开始。**声明
> 两次 `02 FILLER REDEFINES IN-DATE.` 会得到两个独立的读法，各自
> 从 `IN-DATE` 的第一个字符开始。第二个覆盖*不会*从
> 第一个结束的地方继续。要到达字段的后面部分，请在同一个覆盖之内，
> 在它前面放一个宽度合适的 `FILLER`。

> **注意——覆盖可以嵌套，而写入会沿着整条链传播。** `REDEFINES`
> 可以位于本身被重新定义的记录之内，而在*那个*覆盖之内
> 还可以有另一个。通过最外层的描述写入的两个字节，会通过
> 这些字节的每一种读法可见，不论多深——包括在
> 最内层项目上声明的条件名：
>
> ```cobol
>        01  REC-10.
>            02  PART-A.
>                08  FILLER   PIC X(6).
>                08  CODE-X   PIC XX99.
>            02  PART-B REDEFINES PART-A.
>                03  FILLER   PIC X(8).
>                03  FLAGS    PIC 99.
>                03  FLAG-BITS REDEFINES FLAGS.
>                    04  FLAG-1  PIC 9.
>                    04  FLAG-2  PIC 9.
>                        88  SOFT  VALUE 1.
>        01  REC-12 REDEFINES REC-10.
>            02  FILLER       PIC X(8).
>            02  STATUS-CODE  PIC 99.
>
>            MOVE 11 TO STATUS-CODE.     *> SOFT is now true
> ```
>
> 每种描述在每次写入时都被重新渲染一次，所以这是一项固定成本——
> 但它*确实*是成本。非常大的覆盖（重新定义的 10×10×10 表）会选择退出，
> 转而保留它自己的存储；见语法参考中的注意事项。

**`MOVE CORRESPONDING` 按名称配对项目，而一对中只需有一个是
基本项目。**这是把一条记录复制到次序不同的另一条记录中的捷径：
两个组按名称共有的项目被移动，只在其中一个里的项目
保持不动，而匹配的子组会被深入处理。

```cobol
       01  IN-REC.
           05  CUST-NO    PIC 9(6).
           05  CUST-NAME  PIC X(30).
           05  FILLER     PIC X(4).
       01  OUT-REC.
           05  CUST-NAME  PIC X(30).
           05  CUST-NO    PIC 9(6).
           MOVE CORRESPONDING IN-REC TO OUT-REC.   *> reordered, by name
```

配对是按名称，**而不是**按位置——这正是它的全部意义，
也是它的陷阱：在一侧重命名一个字段，它就会悄悄地不再被复制。

> **注意。**一对中可以让**组**与基本项目相对；标准
> 只要求二者之一是基本项目。跨越它的移动是普通的
> 字母数字移动，所以 `PIC XXX` 发送到由 `999` + `XXX` 构成的组时，
> 会填满全部六个字符。相对的两个*组*则会被
> 深入处理，对它们的子项配对。

> ⚠️ **有些项目从不参与。**用 `REDEFINES` 或
> `RENAMES` 描述的项目被排除在配对之外，从属于它的一切也是如此。
> 这是标准的规则，它的目的是阻止同一批字节在两个名称下
> 被移动两次——`66` 重新分组和它所重命名的项目
> 是同一个存储。如果某个字段莫名其妙地没有被复制，请检查它是否位于
> `REDEFINES` 分支之下。

> **注意——`66` 重新分组属于它的记录，并且可以像其他任何东西一样被限定。**
> `66` 位于级别层次之外，这使它看起来是游离的，
> 但它从属于它所重命名其项目的那条记录。所以
> 同一个 `66` 名称可以在每条记录中各出现一次，并用
> `OF`/`IN` 区分，不论读还是写：
>
> ```cobol
>        01  T-DATA.
>            02  TAG-1.
>                03  TAG-1A     PIC XXXX.
>                03  TAG-1B     PIC XXXXXX.
>        66  SPAN RENAMES TAG-1A THRU TAG-1B.
>        01  U-DATA.
>            02  UNIT-1.
>                03  UNIT-1A    PIC X(7).
>                03  UNIT-1B    PIC XXXX.
>        66  SPAN RENAMES UNIT-1A THRU UNIT-1B.
>
>            MOVE "CALIFORNIA" TO SPAN OF T-DATA.   *> TAG-1, not UNIT-1
> ```
>
> 从“`66` 就是它所覆盖的项目”还可以得出两点。跨越表的重新分组覆盖该表的
> **每一个元素**，而不只是第一个——
> `66 R RENAMES ITEM-1 THRU TABLE-2`，其中 `ITEM-1` 为 `PIC X(5)`，`TABLE-2`
> 为 `PIC XXX OCCURS 5`，宽度是二十个字符（5 + 5 × 3）。而对
> **恰好一个**项目的重新分组，会采用该项目的完整描述：`66 R RENAMES W`，其中 `W` 是 `PIC 9(4)`，
> 它就是一个四位数的数值项，所以当它里面是 8000 时，`ADD 3500 TO R` 会引发
> `ON SIZE ERROR` 并让它保持不变，恰如 `ADD 3500 TO W` 一样。

**表的一个元素是合法的操作数。**给组加下标，
配对就会写入那个元素自己的字段：

```cobol
       01  A-FLOCK.
           05  B-FLOCK OCCURS 4 TIMES.
               10  C-FLOCK.
                   15  CUST-NO    PIC 9(6).
                   15  CUST-NAME  PIC X(30).
           MOVE CORRESPONDING IN-REC TO C-FLOCK (4).   *> the 4th entry only
```

### 数字与文本的比较

`IF` 对两个数字做**代数**比较——按值，连符号都算。它对
两段文本做**逐字符**比较。当你把两者混合时它做什么，
才是值得了解的规则，因为屏幕字段、文件记录和
报表行都是文本：

> **一个数值操作数和一个非数值操作数，会使整个比较
> 成为非数值比较。**数字被当作已经被移到一个
> **与它同样大小**的字母数字项中，然后两者作为文本来比较。
> 那次移动携带的是项目的字符位置，**而不是它的符号**。

```cobol
       01  WS-AMOUNT   PIC S9(18).
       01  WS-TYPED    PIC X(18).
           MOVE -123456789012345678 TO WS-AMOUNT.
           MOVE "123456789012345678" TO WS-TYPED.
           IF WS-AMOUNT = WS-TYPED           *> TRUE — the sign is not compared
```

有三个细节决定这条规则是否适用：

- **数字必须是整数。** `PIC S9(9)V9(9)` 项目没有给它的小数点
  留字符位置，所以它没有可供比较的文本形式。标准
  不允许这样的比较，而 PowerRustCOBOL 对这样的关系置之不理，
  而不是发明一个答案。
- **“文本”指*声明*为文本。** `PIC 99` 项目即使在它碰巧保存着字符的
  时刻——例如在一次组 `MOVE` 之后——也是数值的，所以
  `IF WS-COUNT = 0` 仍然是普通的数值比较。
- **`ALL "x"` 采用另一个操作数的大小**，这是它唯一
  拥有的大小：对 `PIC 9` 项目，`ALL "00"` 是一个字符。

> ⚠️ **被比较的是项目的宽度，而不是值的宽度。** 保存着 12 的 `PIC 9(4)`
> 是四个字符 `0012`，所以它等于 `"0012"`，而*不*
> 等于 `"12"`。如果你把数字与用户键入的东西比较，
> 请对照一个相同宽度声明的字段来比较，或者先把数字 `MOVE` 进
> 一个编辑项，再比较那个编辑项。

**下标之间只需要一个空格。**这里逗号也是可选的：

```cobol
       MOVE 1 TO CELL (1 2).
       MOVE 1 TO CELL (1, 2).
       MOVE W-3 TO CELL OF COLS OF ROWS (IDX-A IDX-B).
```

最后一行值得注意：下标跟在**完整的**限定
名称之后，这正是标准规定的顺序。

**索引名、字面量和相对索引可以自由混用。**声明为
`INDEXED BY` 的表可以用它的索引名、字面量，或在
同一个引用中两者并用来作下标——而下标可以是*相对的*，即索引名加上或
减去一个整数：

```cobol
       01  GRP-TAB1.
           02  GRP-1 OCCURS 6 TIMES INDEXED BY IN1.
               03  ELEM1 PIC XXX OCCURS 4 TIMES INDEXED BY IN2.
           ...
           MOVE ELEM1 (IN1, 1)     TO TEMP.
           MOVE ELEM1 (1 IN2)      TO TEMP.
           MOVE ELEM1 (IN1 - 1, 3) TO TEMP.
           MOVE ELEM1 (IN1 +3)     TO TEMP.
```

> ⚠️ **空格放在哪里，决定了符号的含义。** `IN1 - 1`——两侧都有空格——
> 是*相对索引*：**一个**下标，比索引小一。
> `IN1 +3`——符号与它的数字粘在一起——是*开启下一个下标的
> 带符号字面量*：**两个**下标，与 `IN1, +3` 相同。而 `I+1`，
> 两侧都粘着，则是普通算术。这是标准自己的规则，
> 也是让 `3-DEM-TBL` 成为一个名称而不是
> 一次减法的同一条间距规则。

**组表每次只寻址一个元素。**上面的 `GRP-1 (2)`
不是它自己的槽：它*就是* `ELEM1 (2,1)` 到 `ELEM1 (2,4)`。写入
它会把字节分布到这四个上，读取它则把它们拼接起来，而
`GRP-TAB1`——位于表之上的记录——是每个元素首尾相接，
所以一次 `MOVE` 就复制整个表：

```cobol
           MOVE "AAABBBCCCDDD" TO GRP-1 (1).
           MOVE ELEM1 (1, 3)   TO TEMP.        *> CCC
           MOVE GRP-TAB1       TO GRP-TAB2.    *> the entire table
```

**名称可以以数字开头。**用户定义的词取自 `A-Z`、
`0-9` 和连字符；只有*数据名*必须至少包含一个字母，
而段落名或章节名甚至不需要这样：

```cobol
       01  25COUNT       PICTURE 99.
       01  3-DEM-TBL     REDEFINES 3-DIMENSION-TBL.
       0 SECTION.
```

> ⚠️ **运算符两侧需要空格。** `B - C` 是减法；`B-C` 是
> 数据名。这是标准的规则，也是它让 `3-DEM-TBL` 和
> `WRK-DS-18V00-S` 被读作它们本来就是的单个名称。如果你的意思是做减法，
> 请在符号两侧加空格。

**字面量通过把自己的定界符写两遍来转义它。** COBOL 没有反斜杠：

```cobol
       DISPLAY 'IT''S WORKING'.          *> IT'S WORKING
       DISPLAY "HE SAID ""HI""".         *> HE SAID "HI"
```

另一种定界符完全不需要转义，所以 `"IT'S"` 通常更简单。
反斜杠是普通字符，所以 `"\"` 是一个单字符字面量，
而 `'C:\TEMP\'` 的意思恰如它所写。

**在表意常量之前，`ALL` 是多余的，但是允许的。** `MOVE ALL ZEROS` 就是 `MOVE ZEROS`。在字面量之前，`ALL` *重复*它，
以填满整个接收字段：

```cobol
       01  WS-BAR PIC X(10).
           MOVE ALL "-" TO WS-BAR.       *> ----------
           MOVE ALL "ab" TO WS-BAR.      *> ababababab
```

**条件短语在句点处结束。** `ON SIZE ERROR`、`AT END`、
`INVALID KEY`、`ON OVERFLOW` 和 `ON EXCEPTION` 各自带一个*祈使*语句，
而结束句子的句点同时结束这个短语。这值得
了解，因为失败模式是静默的：你本来打算无条件运行的所有内容，
反而只会在条件触发时才运行。

```cobol
           DIVIDE A INTO B GIVING C
               ON SIZE ERROR MOVE "P" TO FLAG.
           DISPLAY FLAG.               *> always runs — the period closed the phrase
```

当你希望结束短语而不结束句子时，请写 `END-DIVIDE`，
这正是让算术语句能够放在 `IF` 之内的做法：

```cobol
           IF READY
               DIVIDE A INTO B GIVING C
                   ON SIZE ERROR MOVE "P" TO FLAG
               END-DIVIDE
               DISPLAY FLAG
           END-IF.
```

**`INTO` 和 `BY` 指名操作数的顺序相反。**这在每种 COBOL 方言中都会绊倒人，
所以值得直说：被除数是 `INTO` 所指*向*的
操作数，也是 `BY` 所指*出发*的那个。

```cobol
           DIVIDE 20 BY 5 GIVING C.        *> C = 4   — 20 ÷ 5
           DIVIDE 5 INTO 20 GIVING C.      *> C = 4   — 20 ÷ 5, written backwards
           DIVIDE 5 INTO B.                *> B = B ÷ 5, in place
           DIVIDE 2 INTO A B.              *> halves A, and halves B
```

> **注意——`REMAINDER` 使用你实际存储的商。**余数是
> 被除数减去*接收者的值*乘以除数，并截断到该
> 接收者的 PICTURE——而不是整数商。对 `C PIC 999V99`，
> `DIVIDE 7 INTO 23 GIVING C REMAINDER R` 给出 `C = 3.28` 和 `R = 0.04`，
> 因为 23 − (3.28 × 7) 是 0.04。如果你想要整数除法的余数，
> 请把 `C` 声明为整数。

**同一个 `FD` 之下的每个 `01` 描述同一个记录区。**一个 FD 拥有一个
缓冲区；每个 `01` 是对它的一种不同读法，恰如 `REDEFINES`。
通过一个移入的值立刻通过其他的就在那里，而 `WRITE`
可以指名任何方便的描述：

```cobol
       FD  PRINT-FILE.
       01  PRINT-REC     PICTURE X(120).
       01  DUMMY-RECORD  PICTURE X(120).
       ...
           MOVE REPORT-LINE TO PRINT-REC.
           WRITE DUMMY-RECORD AFTER ADVANCING 1 LINES.   *> writes REPORT-LINE
```

**`PERFORM` 一个章节名，整个章节就会运行。**章节就是它的
段落，从它的头部到下一个章节；指名章节的 `THRU` 在
该章节的最后一个段落结束。目标位于范围之内的 `GO TO`
停留在范围之内，而 `PERFORM` 仍然在范围结束时返回：

```cobol
           PERFORM CLEAN-UP-SECTION.
           PERFORM OPEN-FILES THRU CLEAN-UP-SECTION.
```

**`PERFORM … VARYING` 有三条让人措手不及的规则。**这三条都是
标准 COBOL，而只要循环在做比从 1 开始计数更不寻常的事情，
这三条就都有关系。

*`WITH TEST AFTER` 先运行循环体，再测试任何东西。*写在
短语的任一侧，行内或行外，它都把循环变成
do-while：不论条件怎么说，循环体都运行一次，然后才测试
条件——**最内层优先**。条件结果为
假的那一层被递增，它之内的每一层都重新回到它的 `FROM` 值，
循环体再次运行。变量只有在它自己的测试为假时才被递增，所以
结束循环的那个测试会让它保持循环体留给它的样子。

```cobol
           PERFORM COUNT-IT WITH TEST AFTER
                   VARYING WS-I FROM 9 BY 1 UNTIL WS-I > 5.
       *>  COUNT-IT runs once; WS-I is still 9 afterwards.
```

*`AFTER` 变量在它自己的循环结束时会回到它的 `FROM` 值。*
只有最外层的 `VARYING` 变量保留结束它的值。所以在

```cobol
           PERFORM COUNT-IT
                   VARYING WS-A FROM 2 BY 2 UNTIL WS-A > 4
                     AFTER WS-B FROM 10 BY -5 UNTIL WS-B = 0.
```

之后，`WS-A` 是 6，而 `WS-B` 是 **10**，而不是 0。在循环之后读取内层索引
来了解它在哪里停止，是行不通的——请把值放进你
自己的变量中带出来。

*带下标的 `VARYING` 标识符跟随它的下标。*它指名的是下标
在那一刻所选中的任何元素，所以移动下标的循环体
就会遍历表：

```cobol
           PERFORM STEP-IT
                   VARYING TBL (S1) FROM 10 BY INC (S2)
                   UNTIL TBL (S1) > 70.
```

如果 `STEP-IT` 给 `S1` 加 1，每一遍都会递增*下一个*元素。这在标准中是
有意为之的，也很有用——但如果你指的是一个元素，请让下标
离开循环体。

**段落名可以在各章节之间重复——请加以限定来说明是哪一个。**用来
区分数据名的同一个 `OF`/`IN` 也区分过程名，
它既适用于 `GO TO` 也适用于 `PERFORM`：

```cobol
       VALIDATE SECTION.
       WRITE-ERROR.
           MOVE "VALIDATION" TO ERR-STAGE.
           GO TO WRITE-ERROR IN REPORTING.
       ...
       REPORTING SECTION.
       WRITE-ERROR.
           WRITE ERR-LINE.
```

没有限定符时，跳转会去往程序中**第一个**同名的段落，
这很少是你想要的那个。限定符中指名的章节如果
不存在，会被忽略而不是致命错误——使用的是未限定的段落——所以
章节名中的笔误表现为运行了错误的分支，而不是
一条诊断。`GO TO … DEPENDING ON` 取一个普通列表，不带限定符。

**限定可以按需要深入。** `OF` 和 `IN` 是同一个词，
可以混用，而标准允许多达 49 层——足以让任何
重复的名称，通过指名它需要的那么多个上级而变得唯一：

```cobol
           ADD TBL-ITEM-1 OF TABLE-LEVEL-1A IN TABLE-LEVEL-2A
                          OF TABLE-LEVEL-3A IN TABLE-LEVEL-4A
                          OF TABLE-LEVEL-5A
               TO ACCUMULATOR1.
```

> **注意。**你只需要足够多的限定符来消除歧义，而且它们必须
> 按由内到外的顺序出现——但它们不需要是*连续的*层级。

### 国际文本：national（`PIC N`）与 UTF-8（`PIC U`）数据

字母数字项（`PIC X`）计的是**字节**。这对英文和
大多数带重音的拉丁文本来说没有问题，但保存着 `Configuração` 的 `PIC X(10)` 只有
十个字节的空间，而 `ç` 和 `ã` 各占两个。在
第十个字节处截断，文本可能结束在半个字符上。如果你在
PowerCOBOL 中用 `PIC N` 为日文构建过窗体，或在 isCOBOL 中用 `PIC N` 处理
Unicode，那么你已经知道解决办法：改为按**字符**
声明项目。RustCOBOL 给你两种方式来做到这一点。

| 声明 | 保存 | 存储（组、记录或 `BYTE-LENGTH` 所看到的） |
|---|---|---|
| `PIC N(n)` 或 `PIC N(n) USAGE NATIONAL` | n 个 national 字符 | 2 × n 字节，UTF-16 大端 |
| `PIC U(n)` 或 `PIC U(n) USAGE UTF-8` | n 个字符 | 4 × n 字节，UTF-8，以空格填充 |
| `PIC U BYTE-LENGTH n` | 放得进 n 字节的完整字符 | n 字节，UTF-8，以空格填充 |

```cobol
       01  WS-MSG     PIC N(30) VALUE N"Configuração concluída – ok".
       01  WS-CITY    PIC U(20) VALUE U"São Paulo".
       01  WS-CODE    PIC U BYTE-LENGTH 8.
       01  WS-LEN     PIC 9(4).

           MOVE FUNCTION LENGTH(WS-MSG)      TO WS-LEN    *> 30 — characters
           MOVE FUNCTION BYTE-LENGTH(WS-MSG) TO WS-LEN    *> 60 — bytes
           DISPLAY "[" WS-MSG "]"
```

`DISPLAY` 显示文本的 27 个字符，后面跟着三个空格。
该项目从不会把一个字符拆开：过长的 `MOVE` 保留放得下的
完整字符，并丢掉其余部分。

**字面量。** `N"…"`（或 `N'…'`）是 national 字面量，`U"…"` 是 UTF-8
字面量。有两种十六进制形式指名精确的码点：

- `NX"00410063"` 是每个 UTF-16 代码单元四位十六进制数字，这里是 `Ac`。
- `UX"C3A7"` 是字符的 UTF-8 字节，这里是 `ç`。

`U"…"` 字面量接受三种转义：`\uhhhh`、`\U00hhhhhh` 和 `\\`。
格式错误的十六进制字面量会由 **Check** 在它自己的那一行报告，所以
程序从不会带着它运行。这涵盖 `NX` 中的数字个数不是
四的倍数，以及 `UX` 中不是有效 UTF-8 的字节。

**在类别之间移动。** 字母数字数据在 RustCOBOL 中是 UTF-8 文本，所以
从 `PIC X` 到 `PIC N` 的 `MOVE` 取的是它的字符。把 national
数据移回 `PIC X`，是把字符放进接收者的字节中，
并且同样从不破坏一个字符。表意常量取该类别自己的
字符：`SPACES` 是空格字符，`ZEROS` 是数字零。如果你与 `HIGH-VALUES` 比较，有一个
差别很重要：在 national 项目中
它是 U+FFFF，而在 UTF-8 项目中是 U+10FFFF。`JUSTIFIED RIGHT` 的作用与
`PIC X` 相同，按字符计。

**组和 REDEFINES 看到的是字节。** 由 `PIC N(3)` 和
`PIC X(2)` 构成的组长八个字节，而 `REDEFINES` 它的 `PIC X(8)` 显示
这三个字符的 UTF-16 字节。把组 `MOVE` 到
形状相同的组，会准确地还原这些字符。

**与 national 或 UTF-8 操作数的比较**按字符的
Unicode 码点比较。较短的操作数先用空格填充。
`COLLATING SEQUENCE` 不适用于它们。

**字符串处理计的是字符。** 当被检查的项目是
national 或 UTF-8 时，这些动词按字符计位置：

- `INSPECT` 对 `CHARACTERS` 计数，查找 `BEFORE` 和 `AFTER` 定界符，并
  按字符替换；
- `UNSTRING` 给出以字符计的 `COUNT IN` 和 `POINTER`；
- `STRING` 进入这样的项目时，按字符填充和指向。

字母数字项仍然计字节，与以前完全一样。

```cobol
       01  WS-FRUIT   PIC N(13) VALUE N"maçã,pêra,uva".
       01  WS-1       PIC N(5).
       01  WS-2       PIC N(5).
       01  WS-COUNT   PIC 9(3).

           UNSTRING WS-FRUIT DELIMITED BY N","
               INTO WS-1 COUNT IN WS-COUNT  WS-2
      *>   WS-1 = "maçã ", WS-COUNT = 4, WS-2 = "pêra "
```

**ACCEPT。** 键入的文本按 UTF-8 读取。当控制台改为发送
Windows-1252 时（较旧的 Windows 控制台如此），`ACCEPT` 到
national 或 UTF-8 项目中仍然会收到正确的字符。

**函数。**

| 函数 | 返回 |
|---|---|
| `NATIONAL-OF(x [, code-page])` | `x` 的字节在该代码页中所拼出的字符（未给出时为 UTF-8） |
| `DISPLAY-OF(n [, code-page])` | `n` 的字符在该代码页中的字节（未给出时为 UTF-8） |
| `ULENGTH(x)` | `x` 保存了多少个字符 |
| `UPOS(x, n)` | 第 n 个字符开始处的字节位置（没有时为 0） |
| `UWIDTH(x, n)` | 第 n 个字符以字节计的宽度（没有时为 0） |
| `USUBSTR(x, start, length)` | 从第 `start` 个起的 `length` 个字符 |
| `UVALID(x)` | `x` 是格式良好的文本时为 0；否则是第一个坏字节所在的位置 |
| `USUPPLEMENTARY(x)` | 第一个超出 U+FFFF 的字符开始的位置，或 0 |

代码页可以用名称或编号书写：`"UTF-8"`（1208）、
`"WINDOWS-1252"`（1252）或 `"ISO-8859-1"`（819）。**Check** 会拒绝任何以字面量写出的其他
代码页。单字节代码页
放不下的字符会变成 X'7F'。对于 national 参数，`UVALID` 和
`USUPPLEMENTARY` 按 UTF-16 代码单元计，而 `UPOS` 和 `UWIDTH` 按
UTF-16 存储的字节计。`UPPER-CASE` 和 `LOWER-CASE` 对 national 和 UTF-8 数据遵循
Unicode，所以 `ação` 变成 `AÇÃO`。

**文件。** 在记录中，national 字段是它的 UTF-16 字节，而 UTF-8
字段是它填充后的字节，所以记录长度遵循上表的存储列。
基于 national 字段的键按码点排序记录。**LINE SEQUENTIAL**
文件是例外：它是供人和其他程序阅读的，所以 national 和 UTF-8 字段
以纯 UTF-8 文本写入，并从中读回。在 IDE 的索引文件编辑器中，声明为
`PIC N(n)` 或 `PIC U(n)` 的字段采用它的存储宽度，而它的网格单元
显示并接受字符。

**窗体。** 属性是文本，所以把 TextBox 的 `Text` 移进
`PIC N` 项目，再把该项目移进 Label 的 `Caption`，会保留每个字符。
项目的填充空格不属于 Label 所显示文本的一部分。同样的
程序在 `rcrun run`、Run Form 和构建出的应用程序下表现一致。

**在 IDE 中。** 编辑器用字符串颜色绘制 `N"…"`、`NX"…"`、`U"…"` 和 `UX"…"`，
包括前缀，而 IntelliSense 提供这些子句和
函数。当光标位于 COBOL Structure 编辑器中的 national 或 UTF-8 声明上时，
状态栏下的一行会给出它的大小，例如
“WS-MSG — 30 characters, 60 bytes”。调试器把这样的项目列在
`national` 或 `utf-8` 类别之下，值显示为它的字符，长度显示为
以字节计的存储。

> 📷 需要截图 — `national-structure-size.png`：打开窗体的 COBOL
> Structure ▸ WORKING-STORAGE，键入 `01 WS-MSG PIC N(30).`，并把
> 光标留在那一行上。截取编辑器，并带有状态栏下方的
> 大小行。

**Check 拒绝的内容**，各在它自己的那一行：

- 对 national 或 UTF-8 项目做算术，不论作为操作数还是接收者；
- `USAGE NATIONAL` 配 `PIC X`、`USAGE UTF-8` 配 `PIC N`，以及其他
  不匹配；
- `BYTE-LENGTH` 出现在单个 `PIC U` 以外的任何地方；
- 比它的项目更长的 `VALUE`。

> ⚠️ **注意事项**
>
> - National 数值项（`PIC 9 USAGE NATIONAL`）和 national 编辑型
>   图片尚不受支持；Check 会这样说明。
> - 一个 `PIC N` 位置是一个 UTF-16 代码单元，与其他 COBOL
>   实现相同，所以超出 U+FFFF 的字符（大多数 emoji）占两个
>   位置。
> - National 项目保存文本。放进其中的不是文本的字节，例如
>   通过 `REDEFINES` 写入的半个代理对，读回时是
>   U+FFFD。
> - 十六进制字面量 `X"…"` 目前把每一对数字读作一个
>   字符，而不是一个字节。所以 `X"C3A7"` 不是 `ç` 的两个
>   字节。对 UTF-8 文本，请写 `UX"C3A7"`。

### 把一整张表交给一个函数

统计类的内部函数接受数量可变的参数，而 COBOL-85
允许你用保留字 `ALL` 给整张表加下标来传递整张表：

```cobol
       01  READINGS.
           05  SAMPLE PIC 9(4) OCCURS 5 TIMES.
       ...
           COMPUTE WS-PEAK = FUNCTION MAX(SAMPLE(ALL)).
           COMPUTE WS-AVG  = FUNCTION MEAN(SAMPLE(ALL)).
```

写出的一个参数会变成每个元素各一个参数。它适用于 `MAX`、
`MIN`、`SUM`、`MEAN`、`MEDIAN`、`MIDRANGE`、`RANGE`、`VARIANCE`、
`STANDARD-DEVIATION`、`ORD-MAX` 和 `ORD-MIN`。

`ALL` 可以位于多维表的某一维，其他维用普通
下标，并按行优先顺序展开——所以这对一列求和：

```cobol
           COMPUTE WS-COL2 = FUNCTION SUM(CELL(ALL, 2)).
```

`OCCURS … DEPENDING ON` 表按函数被调用那一刻的计数展开。

> **未知的函数名称是编译错误。** 编译器不认识的 `FUNCTION`
> 过去会悄悄地返回 **0**，所以一个笔误会产生一个
> 自信的错误答案，而没有任何东西报告它。`FUNCTION SQRTT(4)` 现在无法
> 编译，并提示 *did you mean FUNCTION SQRT?*

### 把一个文件彻底关掉：`WITH LOCK`

```cobol
       CLOSE CUSTOMER-FILE WITH LOCK.
```

以 `WITH LOCK` 关闭的文件在同一次运行中不能被重新打开。之后的 `OPEN`
会设置 **文件状态 38** 而不是成功，所以这个锁是真正的
保证，而不是一条注释。磁带短语会被解析，并在磁盘上被作为
空操作接受：

```cobol
       CLOSE REEL-FILE REEL FOR REMOVAL.
       CLOSE TAPE-FILE WITH NO REWIND.
```

### 调试行

**第 7 列**中的 `D` 标记一个*调试行*。除非
程序要求，否则它是**注释**：

```cobol
       SOURCE-COMPUTER. XYZ WITH DEBUGGING MODE.
```

没有那个子句，这一行就不会被编译——这是标准的默认行为，
也是这个功能的意义所在：你把跟踪留在源码里，只在
需要时才打开它们。

> ⚠️ **仅限固定格式。** 自由格式没有指示符区域，所以它没有
> 调试行：那里的 `D` 是一个普通的 COBOL 词。

### 又长又别扭的文本：``` 块字面量

**这是 PowerRustCOBOL 的扩展，不是 COBOL-85。** 标准根本没有
多行字面量——续行是固定格式的列机制——
所以自由格式源码既没有办法写多行字面量，也没有办法在不把每个引号
都写两遍的情况下写一个满是引号的字面量。

块字面量像 Markdown 代码块那样用围栏括起来。文本是围栏
*之间*的各行，**逐字**取用：

````cobol
       MOVE
```
Hello, World!
```
       TO WS-GREETING.
````

`WS-GREETING` 收到 `Hello, World!`。

规则很简短：


| 规则                                                    | 细节                                                                    |
| ------------------------------------------------------- | ------------------------------------------------------------------------- |
| 文本从开始围栏的**下一行**开始 | 那一行上 ``` 之后的任何内容都是标签，如同 Markdown 的 `json`          |
| 结束围栏所在的行**不是**文本 | 它之前的换行符也不是，所以单行块没有尾随换行符 |
| 内部的换行符**会**被保留                            | 这正是它的全部意义                                               |
| **不转义**                                         | 引号和撇号都是字面字符                         |

这使得嵌入的 JSON、SQL 和 HTML 变得可读：

````cobol
       MOVE
```json
{"name": "O'Brien", "tags": ["a", "b"], "ok": true}
```
       TO WS-PAYLOAD.
       COBOL::"HTTP-POST" ( WS-URL WS-PAYLOAD WS-RESPONSE WS-HTTP-STATUS ).
````

> ⚠️ **仅限自由格式。** 固定格式有指示符列和序列
> 区域，所以那里的一行反引号意味着别的东西，并会被拒绝。

### 强制唯一声明

每个程序单元都必须**只声明且仅声明一次**它的强制性结构元素。
PowerRustCOBOL 在读取你的源码时检查这一点，并**拒绝
运行程序**，直到你修复它——恰如编译器会标记重复声明的
符号。这条规则涵盖：

- 单个 `PROGRAM-ID`；
- 至多一个 `ENVIRONMENT`、`DATA` 和 `PROCEDURE` DIVISION 头部；
- 程序内唯一的**章节**名称，以及在其章节内（或在不使用章节时在程序内）
  唯一的**段落**名称。

例如，下面的程序会被拒绝，因为它给自己命名了两次：

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. MYPROG.
       PROCEDURE DIVISION.
           DISPLAY "Hello".
       PROGRAM-ID. MYPROGNEWNAME.   *> ✗ PROGRAM-ID declared more than once
           STOP RUN.
```

IDE 在 **Problems** 面板中显示这个错误（CLI 也会打印它），并附上
出错的行，而 Run/Build 操作会被阻止，直到重复项
被移除。合法的多单元源码——依次排列的兄弟程序，每个都以
`END PROGRAM name.` 结束，或真正的嵌套程序——**不受**影响：每个单元
都有它自己的 `IDENTIFICATION DIVISION`，并被独立验证。

> 这是一项结构检查，不是风格建议。没有任何标志可以
> 覆盖它；重复声明唯一的元素始终是错误。

### 带智能默认定界符的 `STRING`

> **首先，它所建立在的标准规则：一个 `DELIMITED BY` 覆盖在它之前写出的
> 每一个发送项。**这个短语支配整个*系列*，而不是
> 它碰巧紧挨着的那个发送项：
>
> ```cobol
>            STRING WS-FIRST WS-MIDDLE WS-LAST
>                DELIMITED BY SPACE INTO WS-FULL-NAME
> ```
>
> 对这三个都起定界作用。如果写了多个短语，每一个支配自上一个以来的
> 发送项，所以这会把第一对按空格拆分，把第二对按
> 逗号拆分：
>
> ```cobol
>            STRING WS-FIRST WS-LAST   DELIMITED BY SPACE
>                   WS-CITY  WS-REGION DELIMITED BY ","
>                INTO WS-LINE
> ```
>
> 写在最后一个短语之后的发送项，每个都取它的全部。

标准 COBOL 要求你在**每一个** `STRING` 操作数上写 `DELIMITED BY`，即使
显而易见的选择是唯一合理的。RustCOBOL 保留这种显式
形式的可用，但当**没有短语支配某个操作数**时，它会根据操作数的类别选取正确的
默认值——所以常见的情形读起来就像普通文本：


| 操作数                         | 默认值               | 原因                               |
| ------------------------------- | --------------------- | --------------------------------- |
| 字符串字面量（`" earns "`）    | `DELIMITED BY SIZE`   | 逐字取用，包括空格 |
| 字母数字项（`PIC X`/`A`） | `DELIMITED BY SPACE`  | 去掉尾部的空格填充   |
| 数值项（`PIC 9`/`S9`）     | `DELIMITED BY SIZE`   | 移动字段的字符       |
| 数值编辑项（`PIC ZZ9.99`）   | `DELIMITED BY SIZE`   | 移动编辑后的字符        |
| `FUNCTION …` / 表达式      | `DELIMITED BY SIZE`   | 移动整个计算出来的值     |

数据项**以它的字段形式**被移动——恰好是它所存储的字符：保存着 `100000` 的
`PIC S9(9)` 贡献 `000100000`（完整的 PIC 宽度），
`PIC ZZZ,ZZ9.99` 贡献它编辑后的文本。所以这个：

```cobol
       01 NAME-X        PIC X(40)        VALUE "Joe".
       01 SALARY        PIC S9(09)       VALUE 100000.
       01 SALARY-EDITED PIC ZZZ,ZZZ,ZZ9.99.
       01 TEXT-OUT      PIC X(100).
       ...
           MOVE SALARY TO SALARY-EDITED
           STRING NAME-X
                  " earns "
                  SALARY
                  " or US$"
                  FUNCTION TRIM(SALARY-EDITED)
             INTO TEXT-OUT
```

产生：

```text
Joe earns 000100000 or US$100,000.00
```

这里的 `DELIMITED BY SPACE` 保留任何**内部**空格（`"Joe Smith"` 仍然是
`"Joe Smith"`），只修剪尾部的填充。写出显式的
`DELIMITED BY …` 短语总是会覆盖它所支配的每个发送项的默认值。

**`INTO` 一个组项目**是可行的，它把结果分布到该组的
从属项目上，按它们各自的宽度从左到右填充——
`STRING … INTO` 一个由 `PIC XX` 和 `PIC XXX` 构成的五字节组，会把
前两个字符留在一个中，接下来的三个留在另一个中。

结果是**逐字节**构建的，所以 `STRING HIGH-VALUE` 贡献它所指名的
那一个字节，并恰好占据接收者的一个字符位置。

### 搜索表：`SEARCH` 和 `SEARCH ALL`

COBOL 表搜索的两种形式都作用于声明了
`INDEXED BY` 索引的 `OCCURS` 表。

- **`SEARCH`** 是**顺序**扫描：它从*当前*索引
  值向上遍历表，运行第一个条件为真的 `WHEN`，如果走出了末尾，
  则运行 `AT END` 短语。请在搜索之前设置索引（`SET idx TO 1`），以控制扫描
  从哪里开始。
- **`SEARCH ALL`** 是**二分**搜索，在大表上快得多。
  它要求表按其 `ASCENDING KEY`（或 `DESCENDING KEY`）子句中指名的键**排序**，而每个 `WHEN` 必须对该键测试
  相等。RustCOBOL 执行真正的二分：平均来说它探测
  `log₂(n)` 个条目，而不是 `n` 个。

```cobol
       01  CITY-TABLE.
           05  CITY-ENTRY OCCURS 5 TIMES
               ASCENDING KEY IS CITY-CODE
               INDEXED BY CITY-IX.
               10 CITY-CODE PIC 9(2).
               10 CITY-NAME PIC X(12).
       ...
           SEARCH ALL CITY-ENTRY
               AT END   DISPLAY "not found"
               WHEN CITY-CODE (CITY-IX) = WS-WANTED
                   DISPLAY "found: " CITY-NAME (CITY-IX)
           END-SEARCH
```

> ⚠️ `SEARCH ALL` 假定表确实按它的键有序。与标准
> COBOL 一样，对未排序的表使用 `SEARCH ALL` 会得到未定义的结果——
> 如果数据不是按键的顺序，请使用顺序的 `SEARCH`。

### 集中的文件错误处理：`DECLARATIVES`

位于 `PROCEDURE DIVISION` 开头的 `DECLARATIVES … END DECLARATIVES` 块
让你在一个地方处理文件错误，而不必在每条语句上写
`INVALID KEY` / `AT END` 短语。每个 declarative 是一个 `SECTION`，它的第一条
语句是 `USE AFTER STANDARD ERROR PROCEDURE ON …`：

```cobol
       PROCEDURE DIVISION.
       DECLARATIVES.
       CUST-ERROR SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON CUSTOMER-FILE.
       REPORT-IT.
           DISPLAY "I/O error on customer file, status " CUST-STATUS.
       END DECLARATIVES.
       MAIN SECTION.
       MAIN-PARA.
           OPEN INPUT CUSTOMER-FILE.   *> if this fails, REPORT-IT runs
           ...
```

`USE` 的目标可以是一个或多个**文件名**（`ON file-1 file-2`）、
**打开模式**（`ON INPUT`、`ON OUTPUT`、`ON I-O`、`ON EXTEND`），或者什么也没有
（涵盖每个文件的通配）。**`ON` 是可选的**——
`USE AFTER STANDARD ERROR PROCEDURE OUTPUT.` 与
`… PROCEDURE ON OUTPUT.` 含义相同，而程序可以在它的
各个处理程序中混用这两种拼法。当文件操作（`OPEN`、`READ`、`WRITE`、`REWRITE`、`DELETE`、
`START`、`CLOSE`）以**错误**的 `FILE STATUS`（`0x` 之外的任何类别）结束时，
匹配的 declarative 就会运行——除非
同一条语句带有它自己的 `AT END` / `INVALID KEY` 短语，那个短语始终
优先。declarative 返回之后，控制继续执行失败操作之后的
语句。（declarative 自己的 I/O 不会
再次触发它自己。）

**处理程序是一个章节，带有真正的段落。**它从它的
章节顶部进入，并流经各个段落直到章节结束，而这些
段落保留它们的名称——所以处理程序可以像你写任何
其他过程那样来写：

```cobol
       DECLARATIVES.
       CUST-ERROR SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON CUSTOMER-FILE.
       CLASSIFY.
           IF CUST-STATUS = "35"
               PERFORM REPORT-MISSING
               GO TO CUST-ERROR-EXIT.
           PERFORM REPORT-OTHER.
       REPORT-MISSING.
           DISPLAY "Customer file not found.".
       REPORT-OTHER.
           DISPLAY "I/O error, status " CUST-STATUS.
       CUST-ERROR-EXIT.
           EXIT.
       END DECLARATIVES.
```

处理程序内部的 `PERFORM` 和 `GO TO` 可以到达该章节的段落、任何
*其他* declarative 章节的段落，以及普通主体的段落。

> ⚠️ **注意事项——两个部分互不贯通。** declaratives
> 是一个独立的过程区域：你的主体从不会*落入*处理程序，
> 而处理程序在它自己章节的末尾结束，而不会继续进入
> 下一个。如果一个段落名称在两个部分中都有声明，那么在处理程序内部
> 所作的引用解析到 declarative 的副本，而在
> 主体中所作的引用解析到主体的副本。对于从 PowerCOBOL 或 isCOBOL 过来的人，这是
> 熟悉的规则；值得记住的一点是，它是被强制执行的，而不是
> 偶然的。

**有些状态只有 declarative 才会告诉你。**顺序动词报告的三条错误路径
很容易被忽略，因为没有别的东西会把它们显现出来：


| 情形                                                                                       | `FILE STATUS` |
| ----------------------------------------------------------------------------------------------- | ------------: |
| 对**已经打开**的文件做 `OPEN`（文件保持原样——它*不会*被重新打开） |          `41` |
| 在 `AT END` **之后**做顺序 `READ`——结束之后没有留下有效的下一条记录                      |          `46` |
| 对从未打开过的文件做 `CLOSE`                                                         |          `42` |

`46` 是第 4 类状态，所以 `AT END` 和 `NOT AT END` 都不会为它运行：
declarative（或显式的 `FILE STATUS` 测试）是看到它的唯一办法。一个
新的 `OPEN`，或一次成功的 `START`，会再次建立一条记录。

> **注意。** `FILE STATUS` 可以指名一个两字符的**组**项目——
> `01 CUST-STATUS. 03 CS-1 PIC X. 03 CS-2 PIC X.`——也可以是普通的
> `PIC XX`。两者都接收这个代码。

> ⚠️ **注意事项。** `FILE STATUS IS` 所指名的项目必须**已声明**。未声明的名称——
> `SELECT` 中的笔误、在 `WORKING-STORAGE` 中被重命名的状态项——
> 是编译错误，`FILE STATUS 'XYZ' of file 'CUSTOMER' is not declared in
> DATA DIVISION`，如同任何未声明的名称。（在 1.90.22 之前它能编译并运行：
> 运行时会填充一个同名的项目，而你本来要测试的状态的每一次测试
> 都在无人察觉中通过。）

### `COPY` 中的 copybook 路径

`COPY` 以单词（`COPY CUSTREC.`）或字面量的形式接受 copybook，
而字面量可以带有文件夹。它**先在程序旁边查找，然后从
项目的文件夹查找**——即保存项目的 `.project.toml` 的那个文件夹。
如果没有文件与该名称精确匹配，就依次尝试扩展名 `.cpy`、`.cbl`、`.cob`、
`.cpb` 和 `.cobol`：

```cobol
       WORKING-STORAGE SECTION.
       COPY "txt\Padrao.ws".
       COPY "txt/Padrao.ws".
```

两行指名的是同一个文件。反斜杠在**每个**平台上都被读作文件夹分隔符，
所以从 Windows 带过来的源码（PowerCOBOL 的
习惯）在 macOS 和 Linux 上无需更改即可编译。本身包含
`COPY` 的 copybook 会先从它自己的文件夹解析它的名称。

> **注意——在 Windows 上写的源码。**保存为 UTF-8 的程序（`.cbl`）或 copybook
> 按 UTF-8 读取。不是这样的——由 PowerCOBOL 或
> 另一个 Windows 编辑器以 **Windows-1252**（或 Latin-1）保存，其中 `ç`、`ã` 或
> `é` 是单个字节——会按 Windows-1252 读取，所以它的重音会完整地到达：
> 通过 `rcrun`、通过 **Run** 和 **Build**，在每个 `COPY` 中都是如此。编辑器带着重音
> 打开这样的文件，并**以 Windows-1252 保存回去**，所以写出它的
> 那个编辑器仍然能读它；只有 Windows-1252 没有位置容纳的字符
> （例如日文或中文文本）才会让它改为把文件保存为 UTF-8。
> `COPY` 名称周围的单引号或双引号是一样的：
> `COPY 'txt/Padrao.ws'.`

> ⚠️ **注意事项——带重音的字母占两个字节。**文本在运行时
> 以 UTF-8 保存，不论文件的编码是什么，而 `PIC X(n)` 计的是字节：`ç`、
> `ã` 或 `é` 占两个，`–` 或 `€` 占三个。`PIC X(30) VALUE "Configuração
> concluída – ok"`——27 个字符，32 个字节——只保留放得进 30 个字节的部分。
> 请在保存带重音文本的字段中留出余地。

**在窗体中**，请从项目的文件夹开始写路径：窗体的程序
被生成到项目的 `generated/` 文件夹中，你从不在那里写东西，所以
`COPY "txt\Padrao.ws"` 找到的是 `<project>/txt/Padrao.ws`。它在你写的
每个块中都有效——FILE-CONTROL、FILE SECTION、WORKING-STORAGE，以及处理程序
自己的 WORKING-STORAGE 和 PROCEDURE DIVISION——在窗体检查、**Run
Form** 和构建出的应用程序中都一样。

> **注意——`GLOBAL` 随 copybook 一起走。**窗体的事件处理程序是
> 嵌套在窗体程序内的程序，它们只能看到声明为 `GLOBAL` 的窗体项目
> （`01 WS-PADRAO GLOBAL PIC X(20).`、`FD CUST-FILE IS
> GLOBAL.`）。复制到窗体的 WORKING-STORAGE 或 FILE SECTION 中的 copybook
> 遵循同样的规则，所以当处理程序使用它的项目时，请把它们声明为 `GLOBAL`。

> **注意——行号仍然是你的。** `COPY` 之后的每一行都保持
> 你所写的编号——在错误、调试器的当前行和
> 断点中都是如此。在 copybook *之内*报告的内容，会在把它带进来的
> `COPY` 那一行上被报告。

### 相对 `ASSIGN` 路径从哪里开始

`SELECT SETTINGS-FILE ASSIGN TO "data/settings.idx"`——或者保存在
数据项中的同一路径——指名的是**相对于应用程序文件夹**的文件：在 **Run Form** 下是
项目文件夹，在构建出的应用程序中是保存 `assets/` 的文件夹
（开发期间是 `bin/`，`dist/` 中是交付文件夹）。它与你的图像、你的知识库和你的 Indexed 绑定
已经使用的锚点相同，所以相对路径在程序运行的每个地方都指向同一个文件，
不论是谁启动的，也不论从哪里启动。

绝对路径按所写的原样使用。用 `rcrun run` 运行的控制台程序
没有应用程序文件夹，所以在那里相对路径从
当前目录开始，一向如此。

同样的规则适用于每一个接受路径的内置调用：
- `COBOL::"WRITE-FILE"`、`COBOL::"APPEND-FILE"` 和 `COBOL::"AUDIO-PLAY"`；
- `COBOL::"HTTP-DOWNLOAD"`、`COBOL::"IMAGE-FIT"` 和 `COBOL::"IMAGE-INFO"`；
- AWS 控件上传的本地文件（`PutObject` 的第三个参数）或把
  答案保存进的文件。

`COBOL::"IMAGE-FIT" ( photo "data/cache/1-400.jpg" 400 400 status )` 写入
项目的 `data/cache/`，在 Run Form 下和构建出的应用程序中都一样。
**Run Form** 也在项目文件夹中启动程序，所以运行时自己不解析的路径，
在那里的含义也相同。

> ⚠️ **文件夹必须存在。** `OPEN OUTPUT` 创建文件，而不是它上面的
> 文件夹：请随项目交付一个空的 `data/` 文件夹（里面放一个 `README.md`
> 就足以让它留在版本控制中），否则第一个 `OPEN OUTPUT` 会失败——
> 顺序文件是 `30`，索引文件是 `90`。

### 打开可能不存在的文件：`SELECT OPTIONAL`

只有 `OPEN OUTPUT` 会创建文件。`OPEN INPUT`、`OPEN I-O` 和 `OPEN EXTEND`
都期望文件存在，而它不存在时是 `FILE STATUS` **`35`**——这
通常正是你想要的，因为缺失的主文件是值得停下来处理的问题。

被拒绝的 `OPEN` 会让文件保持**关闭**，不论状态是什么。所以
在首次使用时创建带键文件的熟悉做法照原样可行：用
`35` 回应 `OPEN OUTPUT`、`CLOSE`，然后再次 `OPEN I-O`。紧接在被拒绝的 `OPEN` 之后的
`CLOSE` 回答 `42`——文件从未打开。

当它*不是*问题时——可选的事务文件、第一次运行时从空开始的
日志——请在 `SELECT` 中这样说明：

```cobol
       FILE-CONTROL.
           SELECT OPTIONAL DAILY-TRANSACTIONS
               ASSIGN TO "trans.dat"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS TRANS-STATUS.
```

现在缺失的文件会被创建而不是被拒绝，而 `OPEN` 报告 **`05`**，
所以程序可以区分这两种情形——`00` 表示文件已经
在那里，`05` 表示它不在。以 `INPUT` 打开时，不存在的文件表现
得像空文件：第一个 `READ` 引发 `AT END`。

### 把报表打印到 Viewer：`ASSIGN TO VIEWER`

报表一直有两个去处——打印机，或者你随后必须去
找到并用别的东西打开的文件。现在有第三个：

```cobol
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO VIEWER "VWR-1"
               ORGANIZATION IS MARKDOWN
               FILE STATUS IS REPORT-STATUS.
```

`"VWR-1"` 是**窗体上 Viewer 控件的 id**，恰如它在
Form Designer 中出现的那样。文件的其他一切都是你已经
在写的 COBOL：同样的 `FD`、同样的记录描述、同样的 `OPEN OUTPUT`、
`WRITE` 和 `CLOSE`。只是目的地变了。

随 `ORGANIZATION` 而变的是各行被**读取**的方式：

| `ORGANIZATION IS` | Viewer 显示 | 页面控制 |
|---|---|---|
| `SEQUENTIAL` | 纯文本 | **有**——`ADVANCING PAGE` 产生一页 |
| `MARKDOWN` | Markdown，已渲染：标题、表格、强调 | 无 |
| `HTML` | HTML，已渲染——Viewer 绘制的子集，由它的 CSS 设置样式，无脚本 | 无 |

每次 `WRITE` 贡献一行，去掉尾部的空格。前导和
中间的空格被保留，而这正是报表的列的构成方式。

#### 报表何时出现

在 `CLOSE` 时。不会更早——写了一半的报表不是文档，而读者
看着一页一行一行地拼装，看的是机器在工作，而不是
在阅读。`STOP RUN` 会关闭打开的文件，所以没有 `CLOSE` 就结束的程序
仍然会显示它的报表。

报表落在**视图 1**，而控件的其他方面都不会改变：你设置的
`Layout`、`Zoom`、`SplitMode` 和 `FontSize` 仍然是你的。

#### 它确实是一个文件

报表被写入操作系统临时目录中的一个真实文件，以窗体命名。
正是这一点使 Viewer 的其余部分无需你费力就能对它起作用：**Save As** 写出的恰好是那些字节，
**Print** 把那个文件交给平台，而搜索、缩放和卡片网格的
行为都与你自己打开的任何文档相同。

当临时目录无法写入时，报表会保存在内存中，并
从那里显示。程序的一切都不变；文件系统的
问题不会让你损失报表。

#### 分页报表

`ORGANIZATION IS SEQUENTIAL` 保留传统的打印页面控制，而你写下的分页
就是读者在 `Page` 布局中翻动的页面：

```cobol
       PRINT-HEADINGS.
           MOVE "SALES BY REGION" TO REPORT-LINE
           WRITE REPORT-LINE AFTER ADVANCING PAGE.

       PRINT-DETAIL.
           MOVE DETAIL-LINE TO REPORT-LINE
           WRITE REPORT-LINE AFTER ADVANCING 2 LINES.
```

`AFTER ADVANCING 2 LINES` 留下一个空行，与在纸上一样——
`ADVANCING 1 LINE` 是单倍行距。在 **Markdown** 报表中，那个空行
不是装饰：它把一个段落或一个表格与
下一个分开，所以 `ADVANCING n LINES` 在三种组织方式中都被接受。

`ADVANCING PAGE`、`FD` 的 `LINAGE` 子句和 `AT END-OF-PAGE` 描述的是
印刷页面，而 Markdown 和 HTML 是流式的——它们没有页面可以分。在
已渲染的报表上写其中任何一个，会在你检查程序时被报告，而不是
悄悄地丢掉你所要求的分页。

> **注意。** `LINAGE-COUNTER` 是整个程序共用的一个名称。同时拥有
> 两个 `LINAGE` 文件的程序在它们之间共享它；压倒多数的常见
> 形态是一份印刷报表，它所服务的正是这种形态。

#### 报表不会做什么

- **它不能被读取。** 对 `VIEWER` 文件做 `OPEN INPUT` 或 `OPEN I-O`
  会以 `FILE STATUS` **`37`** 被拒绝——Viewer 是用来打印的地方，而不是
  可以读回的文件。如果你想读取，请用 `Source` 打开该文档。
- **它需要窗体。** 控制台程序没有可以打印进去的 Viewer，它的
  `OPEN` 会以 **`93`** 被拒绝。
- 后面没有控件 id 的 `ASSIGN TO VIEWER` 是 **`31`**。
- **第二份**被关闭进同一个 Viewer 的报表会替换第一份。第一份的
  文件留在原处——操作系统拥有它的临时
  目录，而删除该文件会破坏读者尚未来得及做的 Save As。
- **没有记录**的报表仍然是报表：一份空的文档，这与任何其他
  结果一样，也是一种结果。

> ⚠️ **注意事项。** 保存在内存中的报表（上面的回退）是整体保存的。写入
> 文件的报表在读者浏览的过程中被建立索引，所以非常大的
> 报表放在磁盘上比放在内存中更省——这是常见的情形，
> 除非临时目录拒绝你，否则你得到的就是这种情形。

### 结束磁带卷：`CLOSE … REEL` / `CLOSE … UNIT`

`CLOSE file REEL` 和 `CLOSE file UNIT` 结束多卷磁带的一个*卷*。
它们**不会**关闭文件——文件保持打开，下一个 `READ` 或 `WRITE`
继续进行。在磁盘上没有卷，所以该语句报告 **`07`**：
成功，但这个文件不在卷/单元介质上。

> ⚠️ `07` 是第 0 类（成功）状态，所以它不会运行 `USE` declarative。
> 如果你在移植磁带作业，要检查的是你的代码没有
> 把 `CLOSE … REEL` 当作“文件结束了”——它从来都不是。

### 记录有多长？FD 的 `RECORD` 子句

从 PowerCOBOL 或 isCOBOL 过来的话，你多数时候写的是固定大小的
记录，而这仍然是默认情形：没有 `RECORD` 子句时，由
`01` 记录描述给出长度，文件就是一串
大小相等的记录。

当记录**可变**时，这个子句才有意义。它有三种拼法。

**固定**——文档说明，以及对记录描述的一项检查：

```cobol
       FD  LEDGER-FILE
           RECORD CONTAINS 120 CHARACTERS.
       01  LEDGER-RECORD PIC X(120).
```

**可变，由你所写的记录决定大小。**给出一个范围，然后为每种大小声明一个记录
描述。每个 `WRITE` 发送的字符数与它指名的记录一样多，而
每个 `READ` 恰好返回写入的内容：

```cobol
       FD  CUSTOMER-FILE
           RECORD CONTAINS 120 TO 151 CHARACTERS.
       01  SHORT-RECORD.
           02  CUST-KEY    PIC X(120).
       01  LONG-RECORD.
           02  CUST-KEY-2  PIC X(120).
           02  CUST-NOTES  PIC X(31).
       ...
           WRITE SHORT-RECORD.     *> 120 characters
           WRITE LONG-RECORD.      *> 151 characters
```

**可变，由数据项决定大小**——`DEPENDING ON` 让一个项目*成为*
长度，并且双向都有效：

```cobol
       FD  CUSTOMER-FILE
           RECORD IS VARYING IN SIZE FROM 120 TO 151 CHARACTERS
             DEPENDING ON WS-RECORD-LENGTH.
       ...
       WORKING-STORAGE SECTION.
       01  WS-RECORD-LENGTH PIC 999.
       ...
           MOVE 151 TO WS-RECORD-LENGTH.
           WRITE LONG-RECORD.              *> writes 151 characters
           ...
           READ CUSTOMER-FILE
               AT END SET END-OF-FILE TO TRUE
           END-READ.
           DISPLAY "read " WS-RECORD-LENGTH " characters".
```

在 `WRITE` 之前设置它；在 `READ` 之后读取它。超出
声明的 `FROM … TO` 范围的长度是边界违例——`FILE STATUS` **`44`**，
并且什么也不会写入。它不会被悄悄地取整到范围之内：FD 禁止的
记录是一个值得被听到的缺陷。

> **注意。**一个 FD，如果它的 `01` 记录**大小不同**，那么不论它是否这样声明，
> 它都是变长文件——`RECORD` 子句是
> 可选的，起作用的是记录描述。如果你指的是定长
> 记录，请让这些描述保持相同大小（或写 `RECORD CONTAINS n CHARACTERS`）。

> ⚠️ **变长文件与定长文件不可互换。**
> 它的记录自带长度，因为这是 `READ` 知道每条记录在哪里结束的唯一
> 办法。通过定长 FD 写入的文件，用变长 FD 读取
> 不正确，反过来也一样——所以如果两个
> 程序共用一个文件，请给它们相同的 `RECORD` 子句。

**FD 的各条记录是对同一个区域的各种读法。**正如*按标准允许的写法来写*
之下所描述的，上面的 `SHORT-RECORD` 和 `LONG-RECORD` 不是
各自独立的缓冲区。这里由此得出两点：`READ` 会同时填充两者——读取长
记录之后，长记录的 `CUST-NOTES` 就在那里——而 `WRITE` 发送
整个区域，包括它所指名的记录仅用
`FILLER` 覆盖的任何部分。

**`FILLER` 保存它的字节。**记录描述中没有名字的项目是你
无法按名称寻址的空间，而不是消失的空间：
`02 FILLER PIC X(120).` 是记录中的 120 个字符，而完全由 `FILLER` 构成的
记录仍然带着组 `MOVE` 放进其中的任何内容。

**`SIGN IS SEPARATE CHARACTER` 会多占一个字符。** `PIC S9(5)` 占五个
位置，符号附在一个数字上；`PIC S9(5) SIGN IS LEADING SEPARATE CHARACTER` 占**六个**，多出的那个保存字面的 `+` 或 `-`。当你
手工布局记录时请把它算进去。

### 直接读入 working storage：`READ … INTO`

`READ file INTO identifier` 就是先 `READ`，然后把记录做一次组 `MOVE` 到
`identifier`——这值得直说，因为这意味着
移动遵循**组移动规则**，而不是接收项目的 `PICTURE`：

```cobol
       01  WS-SUMMARY-AREA.
           02  WS-ACCOUNT  PIC X(12).
           02  WS-BALANCE  PIC X(10).
       ...
           READ LEDGER-FILE INTO WS-SUMMARY-AREA
               AT END SET END-OF-FILE TO TRUE
           END-READ.
```

记录的字符从左到右铺在接收者的从属项目上，
每个取它自己的宽度，而记录**在接收者的总
宽度处被截断**——把 120 个字符的记录读进 22 个字符的组，交付的是前 22 个
字符，并且不碰组之后声明的任何东西。因此接收者
比记录短是正常的，而不是错误。

接收者可以带下标（`READ LEDGER-FILE INTO TABLE-ENTRY (WS-I)`），而
记录区本身也同样保存着这条记录，所以你也可以通过 `01`
来读它。

### 原地更新顺序文件：`REWRITE`

`REWRITE` 替换最后一次 `READ` 交付的那条记录。文件必须以
`I-O` 打开，而模式始终是先读后重写：

```cobol
           OPEN I-O LEDGER-FILE.
           READ LEDGER-FILE
               AT END SET END-OF-FILE TO TRUE
           END-READ.
           MOVE "SETTLED" TO LEDGER-STATUS.
           REWRITE LEDGER-RECORD.
```

读位置不会被扰乱：下一个 `READ` 仍然给出*紧跟在*你所替换的那条记录
*之后*的记录，所以读-改-写的循环恰好
遍历文件一次。

它会拒绝三种情形，每一种都有一个值得测试的 `FILE STATUS`：


| 情形                                                                                                           | 状态 |
| ------------------------------------------------------------------------------------------------------------------- | ------ |
| 文件不是以 `I-O` 打开                                                                                           | `49`   |
| 没有成功的 `READ` 建立记录——包括在 `AT END` 之后，以及两次 `REWRITE` 之间没有 `READ` 的第二次 | `43`   |
| 新记录与所读记录的长度不同                                                               | `44`   |

长度规则会让从索引文件过来的人感到意外。
顺序文件没有地方让记录在原地增长——它之后的一切
都得移动——所以对 `RECORD … DEPENDING ON` 文件，该项目在
`REWRITE` 时的值必须等于 `READ` 报告的长度。更改它再
重写，就是你*请求*不同长度的方式，而答案是 `44`。

> **注意。** `REWRITE` 从不重新定位文件，所以不存在
> 重写“我三次读取之前读到的那条记录”这回事。请让循环保持紧凑：读、
> 改、重写、再读。

### 按编号寻址记录：`ORGANIZATION IS RELATIVE`

**相对**文件是一张带编号的槽位表，而不是记录列表。槽位
*n* 要么保存一条记录，要么是空的，而空槽位保留它的编号：
删除记录 7 不会给记录 8 重新编号。如果你用过 PowerCOBOL 或 isCOBOL 中的
相对文件，这个模型是熟悉的，它介于它的
两个邻居之间：只能遍历的顺序文件，通过记录内部的键寻址
的索引文件——以及相对文件，你通过
记录的*位置*来寻址它。

这个编号保存在 `RELATIVE KEY` 项目中，它位于 WORKING-STORAGE，**不在
记录中**：

```cobol
       SELECT CUSTOMER-FILE ASSIGN TO "customers.rel"
           ORGANIZATION IS RELATIVE
           ACCESS MODE IS DYNAMIC
           RELATIVE KEY IS CUST-SLOT
           FILE STATUS IS CUST-STATUS.
```

`RELATIVE KEY` 对 `RANDOM` 和 `DYNAMIC` 访问以及对 `START` 是必需的；
只用 `ACCESS MODE IS SEQUENTIAL` 遍历的文件可以省略它。 `KEY` 和
`IS` 都是可选的，所以 `RELATIVE KEY RK` 和简单的 `RELATIVE RK` 指名
同一个项目——阅读较旧的源码时知道这一点很有用。

**创建文件。**在顺序访问模式下你不选择
编号——每个 `WRITE` 取下一个槽位，而引擎会把它用过的编号放进
`RELATIVE KEY` 项目。创建文件的程序就是这样
了解它自己的记录编号的：

```cobol
           OPEN OUTPUT CUSTOMER-FILE.
           PERFORM 1000-BUILD-ONE UNTIL NO-MORE-INPUT.
      *    After each WRITE, CUST-SLOT holds the number just assigned.
```

**直接寻址一条记录。**在 `RANDOM` 或 `DYNAMIC` 下，你先设置
编号，然后每个动词都作用于那个槽位：

```cobol
           MOVE 417 TO CUST-SLOT.
           READ CUSTOMER-FILE
               INVALID KEY DISPLAY "NO RECORD 417"
           END-READ.
```

**遍历。** `READ … NEXT` 和 `READ … PREVIOUS` 按编号顺序访问被占用的槽位，
并跳过空的槽位，而每次读取都在 `RELATIVE KEY` 项目中报告它所
交付的槽位——这是了解你刚读到的记录实际位于*哪里*的
唯一办法。

**不读取而定位。** `START` 移到第一个符合比较的槽位，
但不交付任何东西；随后的 `READ NEXT` 返回那条记录：

```cobol
           MOVE 400 TO CUST-SLOT.
           START CUSTOMER-FILE KEY IS NOT LESS THAN CUST-SLOT
               INVALID KEY SET NO-SUCH-RECORD TO TRUE
           END-START.
           READ CUSTOMER-FILE NEXT RECORD AT END ...
```

**更改与删除。**在随机或动态访问下，`REWRITE` 和 `DELETE` 按编号指名它们的记录，
在顺序访问模式下则作用于最后一次 `READ` 交付的
记录。 `DELETE` 清空槽位；编号仍然可以
寻址，而之后的记录**不会**下移。

值得测试的状态：


| 情形                                                                    | 状态           |
| ---------------------------------------------------------------------------- | ---------------- |
| 对已经保存着记录的槽位做 `WRITE`                              | `22`             |
| `RELATIVE KEY` 为零时的 `WRITE`、`READ`、`REWRITE` 或 `DELETE`         | `24`             |
| 对空槽位或超出末尾一位的槽位做 `READ`、`REWRITE`、`DELETE` 或 `START` | `23`             |
| 没有更多记录时的 `READ NEXT` / `PREVIOUS`                            | `10`             |
| 记录编号放不进 `RELATIVE KEY` 项目的顺序 `READ` | `14`             |
| 之前没有 `READ` 的顺序 `REWRITE` 或 `DELETE`                    | `43`             |
| 文件没有以动词所需的模式打开                              | `47`、`48`、`49` |

**为整个文件确定键项目的大小。**状态 `14` 是这张列表中会
让人措手不及的那个，因为它是由*声明*而不是由
程序所做的任何事引起的。 `RELATIVE KEY` 的 PICTURE 宽度决定了能报告多大的
记录编号，所以覆盖 500 条记录的文件的 `PIC 99` 键
会愉快地走到记录 99，然后就说不出它在哪里了：

```cobol
       01  CUST-SLOT PIC 99.      *> reads 1-99, then status 14
```

`14` 是与 `10` 一样的 at-end 类条件，所以由 `AT END` 短语来
处理它——这意味着只检查 `AT END` 的循环会提前停止，
并且从外面看，就好像文件刚好结束了一样。

存储遵循与索引文件相同的 `STORAGE [MODE] IS MEMORY | DISK` 子句
（见 §14），并且要求两种容器的回答完全一致——程序
不得能够分辨它运行在哪一种上。 `RECORD IS VARYING`
的作用与别处相同：每个槽位保存它的记录自己的长度，所以短
记录不会被填充成含糊不清的东西。

> ⚠️ **注意事项。**槽位编号从 **1** 开始，绝不是 0，并且超出文件当前
> 末尾的随机 `WRITE` 是合法的——它跳过的槽位成为
> 文件的一部分，读作空。最高槽位为 10 000 而里面只有
> 三条记录的文件，是一个完全普通的相对文件，所以请审慎地设计
> 你的编号，而不要直接使用，比如说，客户编号。

### 带页面控制的打印报表：`LINAGE`

如果你一直在手工数行来决定何时打印页尾，
`LINAGE` 会替你做这件事。它把打印文件分成上边距、一个有若干行的**主体**，
以及下边距，并给你一个计数器和一个条件：

```cobol
       FD  PRINT-FILE
           LINAGE IS 60 LINES
               WITH FOOTING AT 55
               LINES AT TOP 3
               LINES AT BOTTOM 3.
       01  PRINT-REC PIC X(132).
```

`LINAGE-COUNTER` 保存主体的当前行，从 1 开始计，并且每当文件被
打开时回到 1。 `WRITE` 获得一个页面溢出短语：

```cobol
           WRITE PRINT-REC AFTER ADVANCING 1 LINE
               AT END-OF-PAGE     PERFORM PAGE-TRAILER
               NOT AT END-OF-PAGE ADD 1 TO WS-LINES-ON-PAGE
           END-WRITE.
```

`AT END-OF-PAGE`（或 `AT EOP`）从**页脚**行起变为真——
上面的第 55 行——这就给了你在主体写满之前打印页尾的余地。
没有 `FOOTING` 子句时，这个条件要等到主体写满。
`WRITE … AFTER ADVANCING PAGE` 开始新的一页并重置计数器。

**每个值都可以是数据项而不是数字**，这就是你在运行时确定
页面大小的方式——来自控制记录、参数文件或操作员：

```cobol
       FD  PRINT-FILE
           LINAGE LINAGE-CTR
               FOOTING FOOT-CTR
               TOP TOP-CTR
               BOTTOM BOTTOM-CTR.
       ...
       WORKING-STORAGE SECTION.
       77  LINAGE-CTR PIC 999 VALUE 66.
       01  FOOT-CTR   PIC 999 VALUE 60.
       01  TOP-CTR    PIC 999 VALUE 3.
       01  BOTTOM-CTR PIC 999 VALUE 3.
```

页面在每次 `WRITE` 时根据这些项目来度量，所以在两次
写入之间更改其中一个，会从那一刻起改变页面。

> ⚠️ **没有 `LINAGE` 子句的文件没有页面**，所以它上面的 `AT END-OF-PAGE`
> 永远不会变为真。写成“一直写到页尾”的循环于是
> 永远不会结束。如果你的报表失控，`LINAGE` 子句是第一个
> 要检查的东西。

### 不用 `FD` 写一个文本文件

日志行、审计轨迹、小型导出——这种工作不值得为
单个 `WRITE` 配上 `SELECT`、`FD` 和一对 `OPEN`/`CLOSE`。两个
内置调用写一行就完成：

```cobol
           COBOL::"WRITE-FILE"  ( WS-PATH WS-LINE WS-STATUS ).
           COBOL::"APPEND-FILE" ( WS-PATH WS-LINE WS-STATUS ).
```


| 调用                   | 它做什么                                                         |
| ---------------------- | -------------------------------------------------------------------- |
| `COBOL::"WRITE-FILE"`  | **替换**文件——这是你写第一行（表头行）的方式 |
| `COBOL::"APPEND-FILE"` | **追加**到末尾——这是你写它之后每一行的方式      |

两者在文件不存在时都会创建它，并且都在文本**后面写一个
换行符**，所以你从不需要自己加。

这三个参数是按位置的：


| 参数                  | 它做什么                                                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| 1 — path                  | 写到哪里。**两端**的空格会被去掉，所以保存着短路径的 `PIC X(120)` 项目可以照原样使用                    |
| 2 — text                  | 行本身。**尾部**的空格会被去掉，所以 `PIC X(200)` 项目不会把文件填充到 200 列                      |
| 3 — status *（可选）*   | 行已写入时设为空格，未写入时设为操作系统自己的错误文本                                   |

先写一行表头，再写各行，这就是整个模式：

```cobol
       WORKING-STORAGE SECTION.
       01  WS-PATH    PIC X(120) VALUE "audit.csv".
       01  WS-LINE    PIC X(200).
       01  WS-STATUS  PIC X(120).
       01  WS-I       PIC 9(4).
       01  WS-COUNT   PIC 9(4).
       01  CUSTOMERS.
           05  CUST OCCURS 100 TIMES.
               10  CUST-ID    PIC X(4).
               10  CUST-NAME  PIC X(20).
      *> ... (the program fills CUST and sets WS-COUNT)
       PROCEDURE DIVISION.
           COBOL::"WRITE-FILE" ( WS-PATH "id,name,total" WS-STATUS ).
           IF WS-STATUS NOT = SPACES
               DISPLAY "Cannot write the export: " WS-STATUS
               GOBACK
           END-IF.

           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-COUNT
               STRING CUST-ID   (WS-I) DELIMITED BY SIZE
                      ","             DELIMITED BY SIZE
                      CUST-NAME (WS-I) DELIMITED BY SIZE
                   INTO WS-LINE
               COBOL::"APPEND-FILE" ( WS-PATH WS-LINE WS-STATUS )
           END-PERFORM.
```

> **注意。**只要文件有关系，就请带上 status 参数。省略它，
> 失败——不能写入的文件夹、不存在的路径——会让
> 程序继续运行，仿佛这一行已经写入了一样。

> ⚠️ **每次调用都会打开并关闭文件。**这正是让这
> 两个调用对少数几行方便、对十万行却不合适的原因。对于批量输出，请声明
> 普通的 `LINE SEQUENTIAL` 文件并 `WRITE`
> 到它，它在整个运行期间保持文件打开。

### 向操作员请求文件或文件夹

需要操作员键入的路径，就是操作员会键入错误的路径。三个
内置调用改为打开操作系统自己的对话框，而程序
会在调用上**等待**，直到操作员做出选择：

```cobol
           COBOL::"OPEN-FILE-DIALOG" ( "Import settings"
                "XML files|xml" WS-PATH ).
           COBOL::"SAVE-FILE-DIALOG" ( "Export settings"
                "XML files|xml" "rag-settings.xml" WS-PATH ).
           COBOL::"FOLDER-DIALOG"    ( "Knowledge Base folder"
                WS-KB-FOLDER WS-PATH ).
           IF WS-PATH = SPACES
               DISPLAY "Nothing chosen."
           END-IF.
```

| 调用 | 参数，按顺序 |
|---|---|
| `COBOL::"OPEN-FILE-DIALOG"` | title · filter · *start folder* · **path** |
| `COBOL::"SAVE-FILE-DIALOG"` | title · filter · suggested file name · *start folder* · **path** |
| `COBOL::"FOLDER-DIALOG"` | title · *start folder* · **path** |

- **path** 始终是**最后一个**参数。它接收所选的文件或文件夹，
  操作员取消时接收 **SPACES**。
- *start folder* 是可选的；省略它，对话框就在
  操作系统所选的位置打开。
- **filter** 写成 `"Description|ext1,ext2"`——`"XML files|xml"`、
  `"Documents|pdf,docx,md"`——或者只写扩展名，`"xml"`。空的
  filter 显示每个文件。
- 保存对话框在覆盖现有文件之前会询问；随后程序
  像写任何其他路径那样写它（`COBOL::"WRITE-FILE"` 或 `FD`）。

> **注意。**对话框属于运行中窗体的窗口，所以它在 Run
> Form 和构建出的应用程序中有效。控制台程序没有窗口可以显示它：
> 在那里调用会立即返回 SPACES，而不是白白等待。

### 图片：适配与测量一张图像

操作员放到窗体上的照片，大小就是相机做出来的大小——
4000 像素宽，侧着放，几兆字节。你存储和显示的
通常是别的东西：为记录准备的 400 × 400 副本，为网格准备的
40 × 40 缩略图。两个内置调用完成这项工作，无需学习任何图像库：

```cobol
           COBOL::"IMAGE-FIT"  ( WS-SOURCE WS-TARGET 400 400 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( WS-PATH WS-WIDTH WS-HEIGHT WS-FORMAT
                                 WS-STATUS ).
```

| 调用 | 参数，按顺序 |
|---|---|
| `COBOL::"IMAGE-FIT"` | source · target · width · height · **status** |
| `COBOL::"IMAGE-INFO"` | path · **width** · **height** · **format** · **status** |

**`IMAGE-FIT`** 读取源图片，把它转正，并制作一个恰好是
你所要求的宽度和高度的副本：

- 它读取 **PNG、JPEG、GIF、WebP、BMP 和 TIFF**，并从文件的**内容**而不是它的名称
  得知它是哪一种——保存为 `photo.jpg` 的 PNG 仍然
  被读作 PNG。GIF 给出它的第一帧。
- 相机经常把竖拍的照片**侧着**存储，并在
  文件中附一条说明它应当朝哪边的备注。 `IMAGE-FIT` 读取这条备注，并
  在做其他任何事之前转动图片，所以副本是正的。
- 它**裁剪以填满**：副本从图片的中间切出，
  取具有你所要求形状的那么多部分，然后缩放。没有东西被拉伸，
  也不加黑边——适配到 400 × 400 的 1200 × 800 照片保留它中央的
  800 × 800 方块，而它的中心仍然是中心。
- **目标的扩展名**决定写出的格式：`.png`，或 `.jpg` /
  `.jpeg`（质量 85，适合照片的良好平衡）。透明区域在 JPEG 中变成
  白色，因为 JPEG 没有透明度；PNG 则保留它。任何其他
  扩展名都会被拒绝。
- 通往目标路径上尚不存在的文件夹会被创建。

```mermaid
flowchart LR
    A["photo.jpg<br/>1200 × 800"] --> B["转正<br/>（根据它的 EXIF 备注）"]
    B --> C["从中间裁成<br/>目标的形状"]
    C --> D["缩放到恰好<br/>400 × 400"]
    D --> E["pet-7.png 或 pet-7.jpg<br/>（由扩展名决定）"]
```

**`IMAGE-INFO`** 只读取文件的开头——从不读取图片
本身，所以即使对非常大的文件也很快——并把宽度和
高度作为数字返回，把格式作为一个词返回：`PNG`、`JPEG`、`GIF`、`WEBP`、
`BMP` 或 `TIFF`。大小是图片**转正之后**的大小，也就是
`IMAGE-FIT` 处理的同一张图片：侧着存储的竖拍照片报告的是，比如说，
3000 × 4000，而不是 4000 × 3000。这也是你制作不裁掉任何东西的
转正副本的方式——把它适配到 `IMAGE-INFO` 所报告的大小。

两个调用都在 `status` 中用**一个词**回答，你测试的就是它：

| `status` | 含义 |
|---|---|
| `OK` | 完成 |
| `NOFILE` | 源不存在或无法打开 |
| `BADIMAGE` | 不是六种格式之一的图片，或已损坏 |
| `TOOBIG` | 仅 `IMAGE-FIT`：源大于 **64 百万像素** |
| `BADEXT` | 仅 `IMAGE-FIT`：目标不是以 `.png`、`.jpg` 或 `.jpeg` 结尾 |
| `BADSIZE` | 仅 `IMAGE-FIT`：宽度或高度小于 1，或目标超过 64 百万像素 |
| `WRITE` | 仅 `IMAGE-FIT`：目标或它的文件夹无法写入 |
| `NOFEATURE` | 此程序在构建时没有带图片支持（见下面的注意事项） |

记录的照片和它的网格缩略图，来自操作员挑选的同一个文件：

```cobol
       WORKING-STORAGE SECTION.
       01  WS-CHOSEN    PIC X(260).
       01  WS-PHOTO     PIC X(260) VALUE "photos/pet-7.jpg".
       01  WS-THUMB     PIC X(260) VALUE "cache/pet-7-40.png".
       01  WS-WIDTH     PIC 9(6).
       01  WS-HEIGHT    PIC 9(6).
       01  WS-FORMAT    PIC X(8).
       01  WS-STATUS    PIC X(12).
      *> ...
       PROCEDURE DIVISION.
           COBOL::"OPEN-FILE-DIALOG" ( "Choose a photo"
                "Pictures|jpg,jpeg,png,webp" WS-CHOSEN ).
           IF WS-CHOSEN = SPACES
               GOBACK
           END-IF.
           COBOL::"IMAGE-INFO" ( WS-CHOSEN WS-WIDTH WS-HEIGHT WS-FORMAT
                                 WS-STATUS ).
           IF WS-STATUS NOT = "OK"
               MOVE "That file is not a picture." TO LBL-MESSAGE::Caption
               GOBACK
           END-IF.
           COBOL::"IMAGE-FIT" ( WS-CHOSEN WS-PHOTO 400 400 WS-STATUS ).
           IF WS-STATUS = "OK"
               COBOL::"IMAGE-FIT" ( WS-PHOTO WS-THUMB 40 40 WS-STATUS )
           END-IF.
           IF WS-STATUS = "TOOBIG"
               MOVE "The photo is over 64 megapixels." TO LBL-MESSAGE::Caption
           END-IF.
```

> **注意。**失败的适配**什么也不写**：副本以临时名称写在
> 目标旁边，只有在它完整之后才会取代目标的位置。已经存在的
> 缩略图在刷新失败之后仍然在那里，没有改变。

> ⚠️ **注意事项——64 百万像素的限制在解码之前检查。**这么大的图片
> 仅仅保存就需要四分之一吉字节的内存。 `IMAGE-FIT`
> 从文件头读取大小，并在触碰像素之前以 `TOOBIG` 拒绝；
> `IMAGE-INFO` 仍然告诉你大小，所以你可以说明
> 原因。

> ⚠️ **注意事项——`NOFEATURE`。**只有当构建看到按名称书写的 `IMAGE-FIT` 或 `IMAGE-INFO`
> 时，图片支持才会被链接进构建出的
> 应用程序，而你总是按名称书写它们。程序名称
> 在运行时才在数据项中拼装的 `CALL`，构建无法读出——那会链接
> 所有东西，所以在实践中你只会在有意不带它构建的
> 运行时中遇到 `NOFEATURE`。

### 通过 HTTP 下载文件

`COBOL::"HTTP-GET"` 把响应作为**文本**交给你，放在数据项中，这
对 JSON 是对的，对照片、PDF 或 ZIP 则是错的：不是文本的字节
撑不过这一趟，而且数据项本来就不是放几兆字节的地方。
`HTTP-DOWNLOAD` **原样、逐字节地把响应保存到文件中**：

```cobol
           COBOL::"HTTP-DOWNLOAD" ( WS-URL WS-FILE WS-HTTP-CODE WS-STATUS ).
```

| 参数 | 它做什么 |
|---|---|
| 1 — url | `http://` 或 `https://` 地址。两端的空格会被去掉 |
| 2 — file | 保存到哪里。通往它的路径上尚不存在的文件夹会被创建 |
| 3 — http-status *（输出）* | 服务器的状态码——`200`、`404`、……——或没有回答时为 `0` |
| 4 — status *（输出）* | 一个词，见下 |

它为你做的事：

- 它**跟随重定向**，连续最多五次。名为
  `Authorization` 的头部不会被带过重定向，所以为
  一个地址准备的凭据永远不会到达另一个地址。
- 它发送你用 `COBOL::"HTTP-SET-HEADER"` 设置的头部——不发送你的
  程序的其他任何头部。
- 整个下载在 **60 秒**后放弃，并拒绝大于
  **256 MB** 的文件——当服务器说明了它有多大时，在读取之前就拒绝；
  当服务器没有说明时，在它增长超过这个大小的那一刻拒绝。
- 它写入 `<file>.part`，只有当整个
  响应都到达之后，才把它重命名为 `<file>`。失败的下载**不会**留下 `.part`，而
  已经存在的文件**保持不变**——所以失败的刷新永远不会把
  昨天的好副本变成今天的半个副本。

| `status` | 含义 |
|---|---|
| `OK` | 已保存 |
| `BADURL` | 不是 `http://` / `https://` 地址 |
| `HTTP` | 服务器以错误回答（`http-status` 保存代码，例如 `404`），或重定向超过五次（`http-status` 为 `0`） |
| `TIMEOUT` | 在 60 秒内没有完成 |
| `TOOBIG` | 大于 256 MB |
| `NET` | 没有连接：未知的主机、被拒绝的连接、中断的传输 |
| `WRITE` | 文件或它的文件夹无法写入 |
| `NOFEATURE` | 此程序在构建时没有带 HTTP 支持 |

云服务发放的临时地址，被保存然后显示为
缩略图：

```cobol
           COBOL::"HTTP-DOWNLOAD" ( WS-PHOTO-URL "cache/pet-7.jpg"
                                    WS-HTTP-CODE WS-STATUS ).
           EVALUATE WS-STATUS
               WHEN "OK"
                   COBOL::"IMAGE-FIT" ( "cache/pet-7.jpg" "cache/pet-7-40.png"
                                        40 40 WS-STATUS )
                   MOVE "cache/pet-7-40.png" TO PIC-PET::ImagePath
               WHEN "HTTP"
                   MOVE "The server refused the download." TO LBL-MESSAGE::Caption
               WHEN OTHER
                   MOVE "The photo could not be fetched." TO LBL-MESSAGE::Caption
           END-EVALUATE.
```

> **注意。**临时（“预签名”）地址在 `?` 之后带着它的签名——
> 授予访问权限的那一部分。运行时从不把那一部分
> 写入日志：当下载失败时，警告只指名服务器和路径。

> ⚠️ **注意事项。** `HTTP-DOWNLOAD` 会等待下载完成，与
> 其他 `HTTP-…` 调用一样——最多一分钟。你的程序要等它
> 返回才继续，而窗体的其他事件处理程序在此期间都不运行，所以在窗体上
> 请让下载保持小，或每次点击只下载少数几个。

### COBOL 中的 Rust——`EXEC RUST`

`EXEC RUST … END-EXEC` 嵌入**真正的 Rust**，编译进你的程序。不是
子集，不是解释执行的仿制品：闭包、泛型、迭代器链、
`match`、`?` 以及整个 `std` 都能工作，因为每个块都会变成
PowerRustCOBOL 已经替你构建的 crate 中的一个普通 Rust 函数。

```cobol
       01 USER-NAME USAGE IS OBJECT REFERENCE RUST-STRING VALUE "ada".
       ...
           EXEC RUST
           user_name.push_str("-lovelace");
           let vowels = user_name.chars().filter(|c| "aeiou".contains(*c)).count();
           println!("{vowels} vowels");
           END-EXEC.
```

> **请用空格缩进，而不是制表符。** 当你按 Tab 时，IDE 的编辑器会插入**两个空格**，
> 所以你在这里键入的代码总是没有制表符。如果你从别处*粘贴* Rust，
> 请用空格粘贴。制表符在 COBOL
> 源码中不只是外观问题：当文件按固定格式读取时，第 1–6 列是序列区域，
> 第 7 列是指示符，两者在解析之前都会被剥掉——所以
> 用制表符缩进的行可能丢失它的前几个字符。用制表符缩进的 `END-EXEC.`
> 以 `D-EXEC.` 的样子到达解析器，会使块没有结束，而错误
> 随后在程序末尾而不是出错的那一行被报告。

**带有块的程序在运行之前先被构建。** *Run* 执行那次构建并
启动构建出的二进制；停顿会在 Output 面板中报告。没有块的程序
保持原来的快速解释器路径。构建需要
Rust 工具链（请从 [https://rustup.rs](https://rustup.rs) 安装）——**你
生成的应用程序不需要**：它在没有安装 Rust 的机器上运行。构建只针对
宿主操作系统，所以请在 Windows 上构建 Windows 应用程序，在 macOS 上构建
macOS 应用程序。

> **构建问题是针对整个项目来问的，这对 Run
> Form 很重要。**任何地方的一个块——即使在你没有运行的窗体中——都意味着该项目中的每个 Run
> Form 都走构建路径。它必须如此：应用程序会打开
> 子窗体，每一个都运行它自己的程序，而它们都共享一个编译后的
> 块注册表，所以只询问你按下 Run 的那个窗体，会让子窗体处理程序中的块
> 在按钮点击时而不是在 Run 时失败。
>
> 由此得出的结论值得了解。**Run Form 仍然运行你
> 按下它的那个窗体**，而不是应用程序的主窗体——IDE 指名它
> 想要的窗体，而构建出的应用程序打开那一个，运行*它的*程序。（在
> 1.62.137 之前，它打开的是主窗体，因为构建出的应用程序通常
> 拒绝在别处启动；这个拒绝仍然守护着你已经
> 分发出去的应用程序。）并且 **Stop 会停止它**：构建出的应用程序运行期间，工具栏的 Run 按钮
> 会变成 Stop，与解释执行的窗体完全一样。

#### 两种块


| 种类                | 位置                                                                                            | 它保存什么                                                                                  |
| ------------------- | ------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| **条目级**      | `CONFIGURATION SECTION`，在 `REPOSITORY` 之后（仅最外层程序，与那里的其他一切一样） | Rust *条目*：`struct`、`enum`、`impl`、`trait`、`use`——对程序中的每个块可见 |
| **语句级** | `PROCEDURE DIVISION`，语句可以出现的任何位置——包括事件处理程序                  | Rust *语句*：实际的工作                                                                     |

> **在窗体中，你究竟在哪里键入它？**窗体没有可供你瞄准的
> DIVISION 头部——它有的是 COBOL Structure 块。条目级块放在
> **REPOSITORY** 块中，在 `CLASS` 条目之下，因为那个块
> 被织入 `CONFIGURATION SECTION`：
>
> ```cobol
>     CLASS RUST-STRING IS "Rust.String".
>     EXEC RUST
>         pub fn shout(s: &str) -> String { s.to_uppercase() }
>     END-EXEC
> ```
>
> **不是 WORKING-STORAGE**——那个块被织入 `DATA DIVISION`，而块在那里
> 会被拒绝。语句级块放在事件处理程序或通用
> 过程中，它们是 `PROCEDURE DIVISION` 的代码。

#### 什么可以进入块

只有 `CLASS` 指名 Rust 类型的 `USAGE OBJECT REFERENCE` 项目。 `PIC`
项目会按名称被拒绝：它的值是带比例的十进制数或定宽的填充
字段，没有任何 Rust 类型*就是*它。请在块之前通过
`INVOKE` 经由对象来移动这样的值。

Rust 变量是你的 COBOL 名称，小写，连字符变成
下划线：`WS-USER-NAME` 变成 `ws_user_name`。落在 Rust
关键字上的名称（`01 TYPE` → `type`）或不能作为标识符开头的名称（`01 1ST-FLAG`）
会被拒绝——请重命名该项目。

**绑定的名称是 `&mut T`，而不是 `T`。**这使你可以通过
它赋值，而方法调用照常自动解引用：

```rust
*counter = 10;              // assign through the name
text.push_str("x");         // method call — no `*` needed
let n = text.chars().count();
```

每种整数类别都绑定为 `i64`，两种浮点类别都绑定为 `f64`，因为对象桥
就是这样存储它们的：`INVOKE` 和块总是看到同一个
值。**`CLASS RUST-I32` 项目在块内是 `i64`**——你写来
填充它的函数必须返回 `i64`，而不是 `i32`。集合保存桥自己的
值类型，所以由 `INVOKE` 填充的 `Rust.Vec` 与在块内填充的 `Rust.Vec` 保存
相同的东西。

**从 COBOL 读取绑定的项目会得到它的值。**块运行之后，
`DISPLAY clicked-button`、`MOVE clicked-button TO WS-N` 和
`SET Label-1::Caption TO clicked-button` 都能看到块所写的内容——
字符串、任何宽度的整数、浮点数和布尔值。集合和你自己的类型
没有单一的可打印值；读取它们会得到内部 id，所以对它们请
通过 `INVOKE`/`::methods` 来处理。

> ⚠️ **在 1.60.23 之前，每一次这样的读取都得到内部 id**——一个
> 遵循声明顺序的小整数，所以读取它第二个项目的程序
> 不论块计算了什么，总是显示“2”。如果标签在应该显示结果的地方显示一个
> 固定的小数字，请用当前的
> 版本重新构建。

**从 COBOL 写入绑定的项目会到达 Rust 的值。** `MOVE 5 TO clicked-button` 和 `SET cobol-text TO TextBox-1::Text` 会更新该
项目所指名的对象，所以下一个块能看到 COBOL 所写的内容——这就是你把
操作员的输入交给块的方式：

```cobol
       01 cobol-text  USAGE IS OBJECT REFERENCE RUST-STRING.
       01 rust-result USAGE IS OBJECT REFERENCE RUST-STRING.
       ...
           SET cobol-text TO TextBox-1::Text
           EXEC RUST
           *rust_result = ferris_say(cobol_text);
           END-EXEC
           SET Label-1::Caption TO rust-result
```

接受这种写入的类别，是那些只有单个标量值的类别：
`RUST-STRING`、每种整数宽度、浮点数和 `RUST-BOOL`。集合或
你自己的类型之一没有可以写入的标量，所以对它们的 `MOVE` 会被报告为
错误——请在块内填充它们。

> ⚠️ **在 1.61.2 之前，写入落在项目的内部句柄上，而不是
> 它的对象上**，这使对象无法到达：下一个绑定该
> 项目的块以 `EXEC RUST cannot bind <ITEM>: handle 0 is not live` 失败，通常
> 表现为处理程序的 `CATCH RUST-EXCEPTION` 中的 `FFI failed:`。请用
> 当前版本重新构建。

> ⚠️ **在 1.63.28 之前，以你的项目主窗口之外的任何方式打开的窗体**——
> 从侧边栏/菜单选入内容窗格，或用
> `OPEN FORM … AS WINDOW` 打开——**可能遇到同样的 `handle is not live` 失败，
> 或更糟：对它自己的某个 `OBJECT REFERENCE` 项目做普通的 `MOVE`，可能
> 悄悄地覆盖属于另一个已打开窗体的无关对象，**
> 并且没有任何错误。作为你项目主窗口运行的窗体
> 从未显示这个问题。请用当前版本重新构建——你书写
> `EXEC RUST` 的方式没有任何改变。

#### 块可以出现在哪里

语句可以出现的任何地方——包括 `IF`、`EVALUATE`、`PERFORM`、
`ON SIZE ERROR`、`INVALID KEY`、`AT END` 之内，以及 `TRY … END-TRY` 之内，
当你想捕获它可能做的事情时，就把它放在那里。

#### 你自己的 Rust 类型

随附的 48 个 `CLASS RUST-*` 类型是下限，而不是上限。在
条目级块中声明一个类型，用 `CLASS` 给它命名，然后像使用任何其他类型那样使用它：

```cobol
       REPOSITORY.
           CLASS MY-POINT IS "Rust.Point"
       EXEC RUST
       #[derive(Default)]
       pub struct Point { pub x: i64, pub y: i64 }
       impl Point {
           pub fn shift(&mut self, dx: i64, dy: i64) { self.x += dx; self.y += dy; }
       }
       END-EXEC.
```

你的类型必须实现 `Default`——第一个触及该项目的块就是从它开始的。

#### 块的行为

- **块体是一个返回 `Result<(), Box<dyn Error>>` 的 Rust 函数体，**
  这正是块内部可以使用 `?` 的原因。要提前离开，请写 `return Ok(())`，
  而不是 `return;`。向外传播的错误会变成 `RUST-EXCEPTION`。
- **panic 是可以捕获的。** `TRY … CATCH RUST-EXCEPTION e … END-TRY` 会捕获它，
  `DISPLAY e` 以纯文本打印 panic 的消息，程序继续
  运行。普通的 `CATCH EXCEPTION` *不会*捕获 panic，而 COBOL 的 `THROW`
  从不会到达 `RUST-EXCEPTION` 子句——一个 `TRY` 可以同时带有两种子句，
  各取各自的那一类。
- **状态在整个运行期间共享。** 不同段落中的两个块，或
  窗体事件处理程序中的块，看到的是相同的对象。 `CANCEL` 不会重置它。
- **事件处理程序可以声明它自己的 `OBJECT REFERENCE` 项目。** 处理程序是
  带有它自己 `WORKING-STORAGE` 的嵌套程序；在那里声明的项目可以
  像在窗体中声明的那样被绑定，它的对象的存活时间与
  运行一样长——处理程序的下一次点击会看到上一次留下的内容。当只有
  那个处理程序使用它时，请在处理程序中声明它；当
  有多个处理程序使用时，请在窗体中声明为 `GLOBAL`。⚠️ **在 1.61.2 之前，只有窗体自己的项目才会获得对象**，
  所以处理程序局部的项目会以 `handle 0 is not live` 失败；把它移到
  窗体并标记为 `GLOBAL` 是当时的变通办法，现在已不再需要。
- **Crate**：`std`，外加 `eframe`、`egui`、`egui_extras` 以及 PowerRustCOBOL 自己的
  crate。包含任何块的程序都会链接 GUI crate，即使它没有
  窗体，所以控制台程序也可以打开窗口。对其他任何东西的 `use` 都会
  被拒绝，并指出 crate 的名称——除非你已经在
  *项目 Crate（测试版）*（见后文）下注册了那个 crate，块就是这样获得第三方
  库的。
- **错误以你的语言来报告。** 块内部的 Rust 类型错误会让构建在
  *你的* `EXEC RUST` 行和列处失败，而不是在生成的代码处。

#### 调试包含块的程序

你可以调试它。像对任何其他窗体那样按 **Debug** 即可。

有一件事需要了解，它解释了其他的一切：带有
块的程序在运行之前总是被**构建**，Debug 也不例外。IDE
会在 Output 面板中说明这一点，构建，然后把调试器附加到
构建所产生的应用程序上。这不是在绕开的某个限制——
这是你的 Rust 在你单步执行时真正被执行的唯一安排，
而这正是你对调试器的期望。

**一个块算一步。** 单步在 `EXEC RUST` 那一行停下，因为语句
就在那里。从那里走一步会运行*整个*块，并落在
下一个 COBOL 语句上。无法在 Rust 中逐行单步：
那些行根本不是被解释的——它们在程序启动之前
就已经被编译成机器码了。

**断点。** 你可以把它们设在 COBOL 的任何地方，包括 `EXEC RUST`
那一行本身。试图把断点设在块*内部*的某一行上，IDE 会拒绝并
告诉你原因，而不是接受一个它永远无法兑现的断点。

其他一切都是普通的调试器：Continue、Step、Pause、每次停下时的
变量快照，以及用来跳过生成的脚手架的 **Only my code**。
你的 COBOL 数据项读起来与在任何其他会话中完全一样——包括
块所写入的项目，因为块在下一次停下之前确实已经运行了。

> **注意——从 PowerCOBOL 或 isCOBOL 过来。** 直觉上“编译过的”
> 与“可调试的”是相反的，因为你习惯的调试器单步执行的是
> 解释执行的代码。在这里，构建出的应用程序*就是*被调试对象：它自己说
> 调试器的协议，所以构建为你换来了可以工作的 Rust，
> 而不会让你失去这次会话。

> ⚠️ **注意事项——先构建，而构建要花多长时间就花多长时间。**
> 对带有块的程序按 Debug，不像对纯 COBOL 窗体那样立刻生效。Output 面板
> 会说明构建已经开始；构建完成后，调试器窗口
> 打开，停在第 1 行。失败的构建会报告
> 失败，并且什么也不启动——你不会被留在原地等待一个
> 永远不会到来的会话。

#### 一个完整的示例：来自 COBOL 的对话框

这会作为控制台程序构建并运行。它在条目级块中定义一个 `eframe` 应用程序，
然后在 `TRY` 内部的语句级块中调用它，
所以失败会以 `RUST-EXCEPTION` 的形式到来，而不会终止运行。

请注意是 `fn ui`，而不是 `fn update`：PowerRustCOBOL 链接的是 **eframe 0.36**，它的 `App`
trait 要求 `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut Frame)`。较旧的
eframe 教程中展示的 `update` 在这里无法编译。

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. WINDEMO.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-I32    IS "Rust.i32"

      *> Item-level block: items only. Emitted at module scope, so every
      *> statement-level block in the program can see these.
       EXEC RUST
           use eframe::egui;
           use std::sync::{Arc, Mutex};

           pub struct ButtonDialog {
               pub clicked: Arc<Mutex<i64>>,
           }

           impl eframe::App for ButtonDialog {
               fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
                   ui.horizontal(|ui| {
                       for caption in [1_i64, 2_i64] {
                           if ui.button(caption.to_string()).clicked() {
                               *self.clicked.lock().unwrap() = caption;
                               ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                           }
                       }
                   });
               }
           }

      *> Opens the window, blocks until a button closes it, and returns the
      *> caption. Zero means the window was closed instead.
           pub fn ask(title: &str) -> i64 {
               let clicked = Arc::new(Mutex::new(0_i64));
               let out = clicked.clone();
               let _ = eframe::run_native(
                   title,
                   eframe::NativeOptions::default(),
                   Box::new(move |_cc| Ok(Box::new(ButtonDialog { clicked: out }))),
               );
               let v = *clicked.lock().unwrap();
               v
           }
       END-EXEC.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *> Only USAGE OBJECT REFERENCE items may cross into a block, and their
      *> names must convert to valid Rust identifiers:
      *> window-title -> window_title, clicked-button -> clicked_button.
       01 window-title    USAGE IS OBJECT REFERENCE RUST-STRING
                          VALUE "Hello, From COBOL".
       01 clicked-button  USAGE IS OBJECT REFERENCE RUST-I32.
       01 ws-error        PIC X(120).

       PROCEDURE DIVISION.
       MAIN.
           TRY
               EXEC RUST
      *> `clicked_button` is a `&mut i64` — assign through it. `RUST-I32`
      *> binds as i64, which is why `ask` returns i64.
                   *clicked_button = ask(window_title.as_str());
               END-EXEC
           CATCH RUST-EXCEPTION ws-error
               DISPLAY "Window failed: " ws-error
           END-TRY.

           DISPLAY clicked-button.
           GOBACK.
```

> ### ⚠️ 不要把这个复制进窗体的事件处理程序
>
> **构建会阻止你**——自 1.60.14 起，如果带有窗体的项目中有块
> 调用 `run_native`，构建就会失败，并指向你自己的行和列：
>
> ```
> EXEC RUST error in 'checkboxes-form.cbl' at line 97, column 32:
> `run_native` cannot open a window from a form application …
> ```
>
> 在那之前它能构建，然后**什么都不做**——没有窗口，没有错误，没有
> 输出——这就是构建现在拒绝它的原因。
>
> 窗体应用程序已经拥有进程唯一的 winit 事件循环，它创建于
> 主线程，而 COBOL 解释器运行在工作线程上。winit 对第二个事件循环的
> 防护是进程全局的，并返回
> `Err(EventLoopError::RecreationAttempt)`。它**不会** panic，所以
> `CATCH RUST-EXCEPTION` 永远不会触发，而惯用的
> `let _ = eframe::run_native(...)` 会把错误丢掉。失败的
> 一切痕迹都消失了。
>
> 也没有视口方面的变通办法：块收到的是 `env`、`objects` 和
> `bridge`，所以它没有可以用来打开视口的 `egui::Context`。**在
> 处理程序中，请通过 `cobolt_objects` 驱动窗体自己的控件，或者显示
> 在设计器中构建的第二个窗体。** `run_native` 是给控制台程序用的，
> 在那里解释器拥有主线程。

#### 在块内部更改控件

块会得到 `cobolt_objects`，也就是运行中程序的对象注册表。在那里
写入控件属性，块返回时窗口就会被重绘：

```cobol
       PROCEDURE DIVISION.
       MAIN.
           EXEC RUST
           cobolt_objects.set_property("LABEL-1", "Caption", "Done");
           END-EXEC.
           GOBACK.
```

> **注意。** 属性名称在这里不区分大小写，与 PowerRustCOBOL 的其他地方一样：
> `Caption`、`CAPTION` 和 `caption` 指的是同一个属性。
>
> ⚠️ **在 1.60.14 之前，这些写入什么也不做。** 块的执行没有通往
> 窗口的通道，所以控件只在内存中改变，而窗体从未显示它。
> 如果你曾用 `COBOL::"SET-PROPERTY"` 绕开了这一点，它仍然有效，并且不需要
> 任何更改。
>
> ⚠️ **请用 `set_property` 写入；不要去用 `get_mut(..).unwrap()`。** 运行中的
> 窗体在有东西第一次写入某个控件时才注册它，所以
> 对于你还没有写过的控件，`get_mut` 什么也不返回，而
> `unwrap` 会 panic。出于同样的原因，块不能**读取**控件的
> 设计值——只能读取它自己设置的值。要读取操作员键入的内容，请在 COBOL 中使用
> `TextBox-1::Text`，并把该项目传进块。

#### 从块中打开窗口

块可以打开它自己的窗口，并在其中绘制它喜欢的任何 egui。请使用
`cobolt_windows`，它在每个块中都在作用域内：

```cobol
       PROCEDURE DIVISION.
       MAIN.
           EXEC RUST
           let picked = std::sync::Arc::new(std::sync::Mutex::new(0_i64));
           let out = picked.clone();

           let win = cobolt_windows::open(
               "pick-a-number",
               eframe::egui::ViewportBuilder::default().with_title("Pick"),
               move |ui, _class| {
                   ui.horizontal(|ui| {
                       for n in [1_i64, 2_i64] {
                           if ui.button(n.to_string()).clicked() {
                               *out.lock().unwrap() = n;
                           }
                       }
                   });
               },
           );

           win.wait();
           cobolt_objects.set_property("Label-1", "Caption",
                                       picked.lock().unwrap().to_string());
           END-EXEC.

           GOBACK.
```

`open` 接受一个 id、一个 `egui::ViewportBuilder` 以及绘制
窗口的闭包。它返回一个句柄：


| 句柄          | 它做什么                              |
| --------------- | ----------------------------------------- |
| `win.wait()`    | 让处理程序停住，直到窗口关闭 |
| `win.is_open()` | 窗口仍然在时为 `true`       |
| `win.close()`   | 从 COBOL 一侧关闭窗口       |

`cobolt_windows::is_open(id)` 和 `cobolt_windows::close(id)` 按 id 做同样的事，
可以从任何地方调用。打开一个已经打开的 id 会替换它所绘制的内容。

> ### ⚠️ 请用 `cobolt_windows::close` 关闭窗口，而不是 `send_viewport_cmd`
>
> 要从窗口自己的绘制闭包内部关闭窗口——OK 按钮、选中的
> 值——请调用 `cobolt_windows::close("your-id")`：
>
> ```rust
> if ui.button(caption.to_string()).clicked() {
>     *out.lock().unwrap() = caption;
>     cobolt_windows::close("ask");     // ← closes THIS window
> }
> ```
>
> 在那里**绝不要**用 `ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close)`，
> 不管有多少 eframe 教程展示它。该命令针对的是这一遍中
> 当前的视口——即**父**视口——所以它会关闭整个
> 应用程序。对话框确实消失了，这就是这个错误得以存活的原因：
> 窗体随之消失，而 `win.wait()` 之后的任何 COBOL（用结果设置
> 标签）于是与关闭过程发生竞争，所以标签有时更新、有时不更新。

> **`wait()` 是安全的。** 你的处理程序阻塞，但窗体不会：
> 解释器运行在它自己的线程上，所以处理程序等待时，窗口仍然在绘制，
> 并保持响应。

> **用 `Arc<Mutex<..>>` 共享状态。** 绘制闭包运行在 UI
> 线程而不是处理程序的线程上，所以两半就是这样交谈的——恰如
> 上面的示例。这也是闭包必须是 `Send + Sync` 的原因。

> ⚠️ **仅限窗体。** 在没有窗体的程序中没有东西在绘制，而
> `open` 会告诉你这一点，而不是注册一个永远不会出现的窗口。控制台
> 程序使用 `eframe::run_native`，它在那里可行，因为
> 解释器拥有主线程。

**为什么你注册一个闭包，而不是被交给 `egui::Context`。** `Context`
不是障碍——它完全可以顺利地传到你处理程序的线程。障碍在于 egui 的
`show_viewport_deferred` 必须**在 UI 线程上、在窗口应当存在的每一帧**被调用：
它把视口标记为在当前这一遍中被使用，否则就丢弃它。你的块只运行
一次，而且在主线程之外，所以它做不到这一点。它交出要绘制的内容，
而窗体应用程序代表你在每一帧重放它。

### 项目 Crate（测试版）——给你的块使用的第三方库

> **测试版。** 这个功能已经完整并经过测试，而树中把它称为
> *项目 Crate（测试版）*，让你知道它的边界仍在摸索之中——
> 项目文件中的固定格式、冲突的措辞和对话框
> 仍可能变动。项目今天所记录的内容会继续有效。

开箱即用时，块可以使用 Rust 标准库以及每个程序
都已链接的 GUI 栈。其他一切都来自**项目 Crate**：
一个项目级的第三方库目录，你从社区
注册表（crates.io）中挑选，就像你曾为 PowerCOBOL 或 isCOBOL 项目挑选 OCX 或 `.jar` 文件一样——
只不过这个目录可以在 IDE 内部搜索，而下载、版本固定和许可证
手续都替你处理好了。

**添加一个。** 在项目树中，**项目 Crate（测试版）**节点
位于 Generated Code 之下。点击它的 `[+]`（或任何 crate 行）以打开
对话框：

📷 需要截图 — project-crates-dialog.png（项目之上的 Project's Crates 对话框：
搜索“csv”，显示结果表，下面的列表中注册了一个 crate，
日志窗格叙述一次添加。在添加 `csv` 之后截取。）

1. **Search**——键入你需要的东西（“csv”、“regex”、“barcode”）并按
   Enter。匹配项以表的形式到来——**crate、版本、下载量、
   描述**——每页 50 条，下面有 `◀` / `▶` 和一个“Page 2/7 — 318
   results”计数器，所以你可以浏览注册表拥有的一切，
   而不是被截断的寥寥几个。下载量以缩写显示
   （`1.2K`、`3.4M`），所以一眼就能分辨成熟的库与
   被遗弃的实验；点击 **Crate** 或 **Downloads** 表头
   可以按名称或按真正的受欢迎程度对该页排序，再点一次则反转。
   **点击表中的 crate 名称**来选取它——这是填写下面名称字段的*唯一*方式；
   它不能键入，所以你添加的
   总是你确实找到过的东西。数值列只与
   它们的内容一样宽，让描述获得其余的空间；拖动任何
   列的边界来改变这种划分。

   一个默认隐藏的 **System** 列标出已经是
   你的应用程序一部分的结果：黄色表示 PowerRustCOBOL 直接链接的 crate
   （`egui`、`eframe`……），灰色表示仅作为某个已链接内容的
   依赖项被拉进来的 crate。黄色的不能被注册——你的块
   已经可以使用它，而 Add 会在不触碰网络的情况下拒绝。灰色的
   **可以**：应用程序带着它，但在你的项目注册它之前，你的块无法指名它，
   所以注册它正是块获得
   `use tokio::…` 或 `use serde_json::…` 的方式。即使隐藏了该列，灰色的结果
   也会被列出；勾选搜索按钮旁边的 **Show System crates**，
   就能看到该列和黄色的结果（用于在选择你自己的
   兼容版本之前，检查你的应用中已有的某个东西是什么版本）。
2. **Version requirement**（可选）——留空则取最新的
   稳定版本，或者写一个 cargo 风格的要求，例如 `^1.3` 或
   `=1.3.6` 来固定在一条版本线上。
3. **Features**（可选，逗号分隔）——有些库把它们自己的部分
   放在带名称的开关之后；crate 自己的页面（↗ 链接）列出
   它们。例如 `serde` 需要它的 `derive` 特性才有用。
4. **Add**——IDE 解析与你的要求匹配的最新版本，
   对照 PowerRustCOBOL 自己链接的一切来检查它，把它的
   源码下载到项目的 `crates/` 文件夹中，并把它记录在项目里。

从那时起，块只需指名它——没有其他仪式：

```cobol
           EXEC RUST
           use csv::ReaderBuilder;
           let mut rows = 0_i64;
           let mut rdr = ReaderBuilder::new()
               .from_reader(order_data.as_bytes());
           for rec in rdr.records() {
               let _ = rec?;
               rows += 1;
           }
           END-EXEC.
```

带连字符的库名称在块内部写成下划线：
注册 `serde-json`，写 `use serde_json::…;`。

**固定就是固定。**添加会记录*精确的*版本，并把它的
源码保存在你的项目内部。构建使用那份副本，别无其他——下个月
互联网上的发布在这里什么也不会改变。当*你*想要更新的版本时，请对一个 crate 按
**Update**，或对该类别按 **Update All**；每个 crate 会移到
其记录的要求所允许的最新版本，而对话框会按 crate 报告
`old → new`、`current` 或 `failed`。以 `=1.3.6` 添加的 crate
永远报告 `current`——这正是精确固定的用意；要更改
要求本身，请移除后重新添加。

**冲突在你添加时解决，而不是在你构建时。**三种结果：

- *Already available*——你请求的是每个程序都会链接的东西
  （`egui`、`eframe`……）。没有可添加的；直接使用它。
- *Refused*——该库无法与 PowerRustCOBOL 所链接的东西共存，例如
  两个争夺同一个原生库的竞争者。对话框显示确切的
  原因。你的项目保持不变。
- *Allowed with a warning*——该库拖进来第二份不兼容的、
  已经存在的某样东西的副本。它能工作，但两份副本不能混用；
  警告会指出它们，所以意外发生在现在，而不是凌晨三点。

**当你确实需要 PowerRustCOBOL 已经链接的东西的另一个版本时。**
比如说，你的块出于你自己的原因需要 `egui` 0.29，
而平台本身链接的是 `egui` 0.36——通常那是一个直接的
拒绝（“already available” / “clashes with the built-in”——这两条
消息来自构建，不论 IDE 使用什么语言，措辞都是英文）。对于恰好这种情形——名称与已链接的 crate 直接冲突，而且版本
确实无法与已链接的版本共存——对话框会提供一个
替代办法，而不是直接拒绝：把它添加在一个**别名**
（`prj_egui`）之下，即与平台自己的副本并存的第二个独立
副本。接受这个提议，你的块就写 `use prj_egui::…`，而不是
`use egui::…`；`rust_manifest.md` 和树中该 crate 的条目
都会注明别名。这是提供别名的*唯一*情形——每一个
其他的添加仍然使用库自己的名称并正常统一，而仅仅是
某个已链接内容的**依赖项**的 crate（上面灰色的
System 依赖项情形）从不需要别名：它以自己的名称注册，
与任何其他库一样。如果你固定的版本无法
与应用程序自己的副本共享，对话框会警告将存在两份副本；
否则两者会统一。

> ⚠️ **别名的副本与平台自己的副本不能互操作。**用
> `prj_egui::Color32` 构建的值，不能交给期望 `egui::Color32` 的
> PowerRustCOBOL API——它们有意地是两个不同的
> crate，只是碰巧同名。只有当你的块对该库的
> 使用是自包含的时，才使用这个办法。

**交付什么。**注册的 crate 与其他一切一样被编译进你程序的单个
二进制——最终用户仍然什么也不用安装。每次构建
还会在目标文件夹（除非你另选，否则是 `dist/`）中、二进制旁边写出 **`rust_manifest.md`**：
二进制中每个外部 crate 的表——名称、精确版本，以及
它来自的注册表页面——这是审计人员会索要的
文档。没有外部 crate 的构建会移除过时的
清单，所以该文件夹从不声称存在二进制并不包含的代码。

**移除。**✖ 按钮会请求确认，然后删除记录和
下载的源码——绝不会删除你的 COBOL。仍然指名该 crate 的块
会在下一次 Check 时失败，并附带指回项目 Crate 的消息。

**不用对话框。**通过 PowerRustCOBOL
AI 工具工作的编码智能体，用 `add_crate` 自己注册 crate，用
`remove_crate` 把它移除（两者都在后面的*与编码智能体协作*之下说明）。
这就是无法连接 IDE 的会话如何仍然获得它的
`EXEC RUST` 块所指名的库。这些工具做的事与对话框相同——对
同一个注册表做同样的搜索、同样地下载到 `crates/`、同样的冲突检查、
在项目文件中同样的记录——并且只添加智能体指名的 crate，
每次调用一个。第三方代码会被编译进你的应用程序，所以智能体
被告知要说明它添加了哪些 crate 以及原因，而每个答案都会列出项目
现在注册了什么；请看一看。

> **说明**
>
> - 添加和更新需要网络；构建不需要（源码
>   已经在你的项目中）。添加之后的第一次构建可能仍然会获取
>   该库自己的依赖项。
> - 被搜索的注册表是 IDE 范围的设置，显示在
>   对话框顶部——把它指向公司的镜像，那么每次搜索、添加和更新
>   都使用该镜像；已经固定的 crate 在你更新之前不受影响。
> - `crates/` 文件夹属于项目 Crate。不要手工编辑那里
>   供应的内容（更新会替换它），而且如果你自己的文件夹已经
>   位于 `crates/`，对话框会拒绝触碰它并说明这一点。
> - ⚠️ 在**较旧的** PowerRustCOBOL 中打开使用项目 Crate 的项目，
>   构建时没有它们，而块随后会因未注册 crate 的错误而无法通过
>   Check——请升级 IDE，而不是重新添加。

---

## 14. 索引文件——一等公民级的资源

索引（ISAM）文件在 PowerRustCOBOL 中得到异常深入的**原创**支持——
这是它的突出资源之一。你通过标准 COBOL 动词使用它们
（`OPEN`、`READ`、`WRITE`、`REWRITE`、`DELETE`、`START`），它们由
文件的 `ORGANIZATION` 自动分派。在此之上，PowerRustCOBOL 还增加了：

### 两种存储模式（SELECT 子句的扩展）

```cobol
       SELECT CUSTOMER-FILE ASSIGN TO "customers.idx"
           ORGANIZATION IS INDEXED
           ACCESS MODE IS DYNAMIC
           RECORD KEY IS CUST-ID
           ALTERNATE RECORD KEY IS CUST-NAME WITH DUPLICATES
           STORAGE MODE IS DISK WITH DATA COMPRESSION.
```

> **⚠️ 注意事项——键属于记录。** `RECORD KEY` 和每个
> `ALTERNATE RECORD KEY` 都必须指名该文件自己的 `FD` 中的字段。如果你
> 在 `FD` 中重命名了键字段而忘了 `SELECT`，程序就无法
> 编译：错误会指名该键和该 `FD`。已经用
> 不同的键布局写出的文件，对 `OPEN` 回答状态 **39**。状态 39
> 表示磁盘上的文件是为另一个描述制作的：某个键字段移动了，
> 或者它的长度改变了。请重新创建该文件，或者用
> 当前的描述把它的记录复制到新文件中。

- **`STORAGE [MODE] IS MEMORY | DISK`** 选择程序在文件打开期间
  如何保存它：整个文件放在 RAM 中，或随着进行在磁盘上读写。
  **默认是 DISK。**在磁盘上，两者写出的格式相同。
- **`WITH [DATA] COMPRESSION`** 透明地压缩记录（没有外部
  依赖）。
- **`WITH PERSISTENCE`**（仅 MEMORY）让内存中的文件可以被更改，并在
  `CLOSE` 时把它保存到磁盘。没有它，`STORAGE IS MEMORY` 文件是为快速查询而保存的
  **只读**副本（见[数据何时落到磁盘（持久化时机）](#数据何时落到磁盘持久化时机)）。这些短语
  可以组合：`STORAGE IS MEMORY WITH COMPRESSION WITH PERSISTENCE`。
- **复合键和备用键**、升序键顺序以及 `WITH DUPLICATES`
  语义都会得到遵循。

### 由多个字段构成的键（拆分键）

键可以按你指名的顺序把记录的多个字段连接起来，而这些
字段不必彼此相邻。PowerRustCOBOL 接受你可能带来的
两种拼法，对 `RECORD KEY` 和
`ALTERNATE RECORD KEY` 都一样：

```cobol
       SELECT ORDERS ASSIGN TO "orders.idx"
           ORGANIZATION IS INDEXED
           ACCESS MODE IS DYNAMIC
      *>   Micro Focus: the key has a name of its own
           RECORD KEY IS ORD-KEY = ORD-REGION ORD-NUMBER
      *>   Fujitsu (PowerCOBOL): a list of fields
           ALTERNATE RECORD KEY IS ORD-CUSTOMER, ORD-DATE
               WITH DUPLICATES
           ALTERNATE RECORD KEY IS ORD-REF = ORD-DATE ORD-NUMBER
           FILE STATUS IS WS-FS.

       FD ORDERS.
       01 ORD-REC.
          05 ORD-REGION    PIC X(2).
          05 ORD-CUSTOMER  PIC X(6).
          05 ORD-AMOUNT    PIC 9(5).
          05 ORD-DATE      PIC 9(8).
          05 ORD-NUMBER    PIC 9(4).
```

- 键的值是按顺序连接的各个字段：上面的 `ORD-KEY` 是区域
  后面跟着订单号，即使它们之间还有另外三个字段。
- `WITH DUPLICATES` 适用于整个键——只有在给出它时，两条记录才可以共享
  `ORD-CUSTOMER`，或共享 `ORD-CUSTOMER` 和 `ORD-DATE` 的组合。
- 要按拆分键定位或读取，请按它的拼法来指名：Micro
  Focus 的用它的键名（`START ORDERS KEY IS >= ORD-REF`、
  `READ ORDERS KEY IS ORD-REF`），Fujitsu 的用它的**第一个**字段
  （`START ORDERS KEY IS = ORD-CUSTOMER`）。请先把值移进
  键的每一个字段。
- Micro Focus 的键名是键的名称，而不是数据项：不要
  在 `FD` 中声明它。键所连接的每个字段都必须是
  `FD` 的字段；拼写错误的字段是会指名它的编译错误。
- **Indexed File Editor** 由多个字段构建的键，是以
  Micro Focus 的形式生成的，并以键命名。

> **注意。** 在 PowerCOBOL 中，你只能通过把字段并排放在
> 组项目之下来达到这一点。那仍然有效，而当字段本来就放在一起时，
> 组键仍然是正确的选择。

### 文件自带它的记录布局

索引文件不需要你的源码就能被理解。当程序为输出打开
文件时，文件会得到描述它的记录的 `FD`，而
从未见过你源码的程序，仅凭文件就能读出布局：
有哪些字段、它们的 PICTURE 和 USAGE、哪些重新定义了哪些，
以及每个字段占用的字节。

```cobol
       FD  CUST-FILE.
       01  CUST-REC.
           05  CUST-ID     PIC 9(6).
           05  CUST-NAME   PIC X(20).
           05  BALANCE     PIC S9(7)V99 COMP-3.
           05  BAL-TEXT    REDEFINES BALANCE PIC X(9).
           05  FLAG        PIC X.
               88  IS-VIP  VALUE "V".
```

这就是程序提供的全部描述；文件以规范
形式存储它，每个项目带一条说明其字节位置的注释——
`05 BALANCE PIC S9(7)V99 COMP-3. *> @26+9`，即从第 26 个字节起的九个字节。

- **在这个功能出现之前就制作的文件**，会在程序第一次为写入而打开它时
  获得它的描述（`OPEN I-O`、`EXTEND` 或 `OUTPUT`），每条记录
  都不受触动。 `OPEN INPUT` 从不更改文件，所以它不会。
- **描述从不碍事。**`FD` 与它不同的程序仍然可以打开该文件：
  记录长度和键决定状态 39，与以前一样，而描述只是描述而已。
- **第一份描述保留。**第二个用另外的措辞声明相同
  字节的程序不会替换它。要重新描述文件，请用
  `OPEN OUTPUT` 重新创建它。
- **导入是完整的。** IDE 中的 *Import existing…* 据此构建 `.cidx`——
  字段、PICTURE、usage 和 `REDEFINES`——而不是裸数据文件
  所能提供的键列和 `FILLER`。

> ⚠️ **注意事项。**注释 `@offset+length` 是字段位置的权威。
> 在这个版本中，任何 `USAGE` 的数值项都以它的数字保存，所以
> 上面的 `COMP-3` 字段占九个字节，而不是五个；从外部读取该文件的程序
> 应当从注释中取得位置和长度，而不是
> 根据子句去计算。

### 数据何时落到磁盘（持久化时机）

这两种存储模式在记录*何时*真正落到磁盘上有所不同——这
关系到性能，也关系到什么能在多次运行之间留存：

- **`STORAGE IS MEMORY`** 在 `OPEN` 时把整个文件加载进 RAM，所以之后的每个
  `READ` 和 `START` 都从内存提供。**没有 `WITH
  PERSISTENCE` 时它是只读的**：它只能以 `INPUT` 打开，而 `OPEN OUTPUT`、
  `I-O` 和 `EXTEND` 会以 FILE STATUS **37** 被拒绝，磁盘上的文件
  完全保持原样。这就是它的用途：为查询准备的文件的快速副本。
  （它过去会接受写入，并在 `CLOSE` 时把它们丢掉，而 `OPEN OUTPUT`
  会清空磁盘上的文件。看上去保存了而其实没有保存的数据才是危险所在，所以
  现在改为拒绝。）
  - **`WITH PERSISTENCE`** 使它可写。`WRITE`/`REWRITE`/`DELETE` 更改
    内存中的映像，`COMMIT`/`ROLLBACK` 是纯粹的**内存中事务
    边界**（**`COMMIT` 从不写入磁盘**），而文件**只在 `CLOSE` 时**写回。
    这就是你在多次运行之间保留内存文件、
    同时只在关闭时付出一次磁盘代价的方式。
- **`STORAGE IS DISK`**（默认的存储模式）**在操作发生时**就把每条记录及其
  索引页写入文件，并**在 `COMMIT` 和 `CLOSE` 时**刷新记录
  目录并做一次持久性同步（`fsync`）。它是连续写入的，并在这些
  点上达到完全一致/持久。
- **`WITH [DATA] COMPRESSION`** 与两者正交：记录在容器中以
  压缩形式存储，但键始终基于**未压缩的逻辑记录**求值，所以搜索顺序和键比较
  不受影响。

**磁盘上只有一种格式。**`MEMORY` 和 `DISK` 是*使用*文件的方式，而不是
两种文件。两者写出的是同一个容器（`PRCIDXD1`），所以一个程序
可以把文件声明为 `STORAGE IS DISK`，另一个声明为 `STORAGE IS MEMORY`，
并且两者读写的是同样的数据。外部读取者，例如驱动程序、
报表工具或 PowerChat，只有一种格式需要理解：数据文件
加上它的 `.cidx`。

- 以 **MEMORY** 打开时，文件连同它的所有记录一起加载。带有 `WITH
  PERSISTENCE` 时，`CLOSE` 以同样的格式把它写回，包括
  程序添加的记录，而 DISK 程序照常读取它。
- **在 1.80.37 之前由 MEMORY 程序写出的文件**位于较旧的
  容器（`PRCIDX1`）中。两种模式仍然都能读取它们。第一次
  为写入而打开其中一个时——由 DISK 程序以 `I-O` 或 `EXTEND`，或由
  MEMORY 程序带 `WITH PERSISTENCE`——它会以当前的格式被重写，
  每条记录都被保留。DISK 程序以 `INPUT` 打开时，读取的是转换后的
  临时副本，它在 `CLOSE` 时被删除，文件保持不变。

**`OPEN INPUT` 从不更改文件**，在两种存储模式下都是如此，并且不需要
写权限，所以只读文件，或位于只读共享上的文件，都可以
读取。当打开本来必须写入文件时（为了完成
崩溃所中断的事务、为了转换旧容器，或为了转换
MEMORY 格式的文件），`INPUT` 打开会在临时副本上完成这项工作，
并让文件保持原样。下一次 `I-O` 打开会在
文件本身上完成它。

根本不是索引文件的文件会在 `OPEN` 时被拒绝（FILE STATUS
90）；它从不被当作空文件读取，也从不被覆盖。

> ⚠️ **持久性注意事项。** `STORAGE IS MEMORY WITH PERSISTENCE` 只在
> `CLOSE` 时保存：如果程序在干净的 `CLOSE` 之前崩溃或 `STOP RUN`，
> 内存中的更改就会丢失。保存会重写整个文件，对于大
> 文件，这比 DISK 程序的 `CLOSE` 明显更久（在我们的
> 测量中，10 万条记录约 6 秒）。对于会变化的数据，
> 请优先选择 `STORAGE IS DISK`：持久性在每次 `COMMIT`/`CLOSE` 时到位，
> 并且没有任何东西被重写。`ROLLBACK` 总是撤销自上一次
> `COMMIT`/`OPEN` 以来在内存中的更改，两种模式都一样。

### `ACCESS MODE` 对写入和更新有什么改变

`ACCESS MODE IS SEQUENTIAL` 不只是另一种读取方式——它把
文件置于 `RANDOM` 和 `DYNAMIC` 所没有的顺序规则之下。如果你
来自 PowerCOBOL 或 isCOBOL，这是熟悉的领域，但仍然值得
显式地测试，因为这些状态是看到它的唯一办法。


| `ACCESS MODE IS SEQUENTIAL` 下的语句                                  | `FILE STATUS` |
| ---------------------------------------------------------------------------- | ------------- |
| `RECORD KEY` **不大于**前一次写入的键的 `WRITE`  | `21`          |
| 紧接在它之前没有成功 `READ` 的 `REWRITE` 或 `DELETE`        | `43`          |
| 两次之间没有 `READ` 的第二个 `REWRITE`/`DELETE`                         | `43`          |
| 在 `START`、`OPEN`、`WRITE` 或失败的 `READ` 之后的 `REWRITE`/`DELETE` | `43`          |

```cobol
       SELECT LEDGER-FILE ASSIGN TO "ledger.idx"
           ORGANIZATION IS INDEXED
           ACCESS MODE IS SEQUENTIAL
           RECORD KEY IS LEDGER-ID
           FILE STATUS IS LEDGER-STATUS.
      *
       OPEN OUTPUT LEDGER-FILE.
       MOVE 100 TO LEDGER-ID.  WRITE LEDGER-RECORD.   *> 00
       MOVE 200 TO LEDGER-ID.  WRITE LEDGER-RECORD.   *> 00
       MOVE 150 TO LEDGER-ID.  WRITE LEDGER-RECORD.   *> 21 — out of order
       MOVE 300 TO LEDGER-ID.  WRITE LEDGER-RECORD.   *> 00
```

**说明。**

- 被拒绝的 `WRITE` 什么也不存储，并且**不会**让顺序向前推进，
  所以下一个键是与实际写入的最后一个键比较的——上面的 `300`
  接在 `200` 之后，而不是被拒绝的 `150`。
- 仅仅与前一个键*相等*的键也不是更大的，所以它同样是 `21`——
  而不是你在 `RANDOM` 或 `DYNAMIC` 下会得到的重复键 `22`。
- `START` 会定位文件，但不交付记录。它不满足
  `REWRITE`/`DELETE` 的要求；只有成功的 `READ` 才满足。
- 在 `RANDOM` 或 `DYNAMIC` 下，这些都不适用：你喜欢以什么顺序写都行，
  并且通过 `RECORD KEY` 的值来寻址 `REWRITE`/`DELETE`，
  不需要之前的 `READ`。那里与现有记录的冲突是 `22`。

> ⚠️ **注意事项。**状态 `43` 属于第 4 类，不是 `INVALID KEY` 条件，所以
> `INVALID KEY` 短语不会捕获它。请测试 `FILE STATUS`，或者让文件的
> `USE AFTER STANDARD ERROR` declarative 来处理它。

### 用 `OF` / `IN` 区分同名的键

一个文件可以声明若干个数据名完全相同、仅靠各自所在的
组来区分的键。请像在 COBOL 的其他任何地方一样对它们加以限定：

```cobol
       SELECT ORDER-FILE ASSIGN TO "orders.idx"
           ORGANIZATION IS INDEXED
           ACCESS MODE IS DYNAMIC
           RECORD KEY IS ORDER-KEY IN PRIME-AREA
           ALTERNATE RECORD KEY IS ORDER-KEY OF ALT-AREA
           FILE STATUS IS ORDER-STATUS.
      *
       FD  ORDER-FILE.
       01  ORDER-RECORD.
           05  PRIME-AREA.
               10  ORDER-KEY   PIC X(10).
           05  ALT-AREA.
               10  ORDER-KEY   PIC X(10).
           05  ORDER-DETAIL    PIC X(60).
```

限定符属于键的身份，所以在指名参照键时请使用同样的形式：

```cobol
       MOVE "AX-4471" TO ORDER-KEY IN ALT-AREA.
       READ ORDER-FILE KEY IS ORDER-KEY IN ALT-AREA
           INVALID KEY     DISPLAY "no such order"
           NOT INVALID KEY DISPLAY ORDER-DETAIL
       END-READ.
```

**说明。**

- 限定是按*包含*关系，而不是直接的父子关系：即使字段位于更深的一层或多层组中，`ORDER-KEY OF ORDER-RECORD` 也指名该字段。
- 未限定的名称仍然表示第一个同名的字段，所以对于
  每个名称一个键的普通情形，没有任何变化。
- 同样适用于 `START … KEY IS`。

### 按键的一部分定位：通用 `START`

`START` 不必指名整个键。它可以指名键的一个**从属项目**
——最左边的部分——于是文件就定位在那个*前缀*上。
这就是通用键的形式，也是你在不知道键的其余部分时
扫描一族相关记录的方式：

```cobol
       FD  ORDER-FILE.
       01  ORDER-RECORD.
           05  ORDER-KEY.
               10  ORDER-BRANCH  PIC X(5).
               10  ORDER-SEQ     PIC X(8).
           05  ORDER-DETAIL      PIC X(60).
      *
       MOVE SPACES  TO ORDER-KEY.
       MOVE "LONDN" TO ORDER-BRANCH.
       START ORDER-FILE KEY IS EQUAL TO ORDER-BRANCH
           INVALID KEY DISPLAY "no orders for that branch"
       END-START.
       PERFORM UNTIL FINISHED
           READ ORDER-FILE NEXT AT END EXIT PERFORM END-READ
           IF ORDER-BRANCH NOT = "LONDN" EXIT PERFORM END-IF
           DISPLAY ORDER-DETAIL
       END-PERFORM.
```

**说明。**

- `EQUAL TO` 定位在键以该值开头的**第一条**记录上，
  而不是精确匹配整个键。
- `GREATER THAN` 越过共享该前缀的**每一条**记录，并落在
  它们之后的第一条上——所以上面的示例可以用
  `KEY IS GREATER THAN ORDER-BRANCH` 直接跳到下一个
  分支。
- `NOT LESS THAN` 的表现与 `GREATER THAN OR EQUAL` 相同，定位在前缀
  达到该值的第一条记录上。
- 该项目必须与键从同一个字符位置开始。指名键中间的
  一个项目不是通用键。
- 同样适用于 `ALTERNATE RECORD KEY`。
- 指名整个键只是前缀就是整个键的特例，
  所以普通的 `START` 不受影响。

> **注意。** 只有 `START` 以通用方式读取键。 `READ … KEY IS` 寻址的是一条
> 记录，需要完整的键值。

### 崩溃安全的事务

COBOL 动词 **`COMMIT`** 和 **`ROLLBACK`** 作用于你的*已打开的索引
文件*：`COMMIT` 确认待处理的 `WRITE`/`REWRITE`/`DELETE` 操作
（于是之后的 `ROLLBACK` 就不能再撤销它们）；`ROLLBACK` 丢弃自上一次
`COMMIT`/`OPEN` 以来所做的更改。对于 **`STORAGE IS DISK`**，`COMMIT` 还会
使这些更改*在磁盘上持久化*；对于 **`STORAGE IS MEMORY WITH
PERSISTENCE`**，它纯粹是内存中的边界（文件在 `CLOSE` 时写入
——见上文）。没有 `WITH PERSISTENCE` 的 MEMORY 文件是只读的，所以它
没有什么可提交的。（这些是**文件**事务——对于 SQL 事务，请使用
带 `BEGIN`/`COMMIT`/`ROLLBACK` 的 `COBOL::"EXEC-SQL"`。）

**如果程序没有结束就停止了**——断电、进程被
杀死、机器崩溃——那么 `STORAGE IS DISK` 文件在下一次
为写入而打开时，**恰好回到它最后一次 `COMMIT` 时的样子**（如果没有 `COMMIT`，
则是 `OPEN` 时的样子）。此后的每一个 `WRITE`、`REWRITE` 和 `DELETE` 都消失了，不论
有多少个：写入了十万条记录而在提交之前
被切断的批处理，不会留下其中任何一条，而已经提交的
则全都在。`OPEN` 自己完成这件事；程序不需要恢复代码。
想要安全重启点的程序在它的检查点处 `COMMIT`——比如
在长时间加载的每几百条记录之后——并从它到达的最后一个检查点重新开始。

⚠️ **说明。**

- 默认引擎和 `redb` 在崩溃之后给出相同的答案。它们的不同
  在于方式：`redb` 在它自己的文件内部把已提交和未提交的工作分开；
  默认引擎则在文件旁边写一份*恢复日志*，
  `<file>.jrn`，只要事务已经更改了文件本身，它就存在。
  **崩溃之后你可能会发现一个**；下一次 `OPEN … I-O`、`OUTPUT` 或
  `EXTEND` 会使用它并把它移除。如果你备份或复制数据文件，请把
  `.jrn` 一起复制，并且绝不要手工删除它。
- 事务打开期间，日志会占用磁盘空间：对于添加记录的加载，
  是几千字节；对于长事务所更改的*现有*记录，每页约
  4 KiB。`COMMIT` 或 `CLOSE` 会移除它。
- `OPEN INPUT` 从不更改文件，所以它不做恢复：它从
  临时副本读取恢复后的数据，并让文件及其日志保持
  原样，留给下一次可以写入的打开。当另一个程序正处于
  长事务当中，而某个程序以 `INPUT` 打开该文件时，情形也一样：
  它从一份私有副本中看到最后一次 `COMMIT`——对于
  非常大的文件，这需要时间。
- 如果有多个程序更新同一个文件，请以 **`WITH LOCK`** 打开它，正如本指南
  已经建议的那样。日志属于其事务被它所描述的那个程序；
  在 `OPEN` 时*没有*被拒绝的第二个程序会发现
  它，并在第一个程序脚下撤销那个事务。
- 如果日志本身已损坏——被截断，或被崩溃之外的
  某样东西更改——`OPEN` 会回答文件状态 **`90`**，并且什么也不触碰。
  请从备份中恢复文件及其日志；不要为了让错误消失而删除日志，
  因为它可能是唯一能把写了一半的文件
  纠正过来的东西。

```mermaid
flowchart LR
    OPEN["OPEN I-O CUSTOMER-FILE"] --> WORK["WRITE / REWRITE / DELETE …"]
    WORK --> DEC{提交还是回滚？}
    DEC -- "COMMIT" --> DUR["更改持久化"]
    DEC -- "ROLLBACK" --> UNDO["更改被丢弃"]
    DUR --> CLOSE["CLOSE"]
    UNDO --> CLOSE
```

### 可插拔的存储引擎

用 `rcrun --indexed-engine <name>`（或
`COBOL_INDEXED_ENGINE` 环境变量）选择引擎。对于项目，请在
**Settings ▸ Default Indexed File Engine** 中一次性选择。Run、Run Form、Debug Form 和 Build
都使用这个选择，对每个窗体都是如此，包括由另一个窗体打开的窗体。
构建出的应用程序携带这个选择。当项目把它留空时，
应用程序使用运行它的那台机器上的 `COBOL_INDEXED_ENGINE`。
引擎只决定**新**文件如何创建：已经存在的文件
总是用写出它的引擎打开。


| 引擎           | 用于                                                                                                                                                      |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rust`（默认） | 内置的 B 树存储；内存中和磁盘上的分页格式。可以有多个程序同时打开一个文件（更新时请使用 `WITH LOCK`）。崩溃会让它回到最后一次 `COMMIT`。 |
| `redb`           | **崩溃安全、ACID** 的磁盘引擎（写时复制的 B 树、校验和、双元数据页）——`COMMIT` 经得起断电；对非常大的数据集可以瞬间 `OPEN`。同一时间只能有一个程序以写入方式打开文件。 |
| `rm` / `fujitsu` | 保留的引擎名称，目前的行为与内置存储完全相同（原生格式是未来的工作）。                                                 |

### 操作日志（可观测性）

为了诊断，你可以打开**按文件的操作日志**
（`rcrun --indexed-log basic|full`，格式 `--indexed-log-format text|json`）。
它为每个 `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` 记录一行，带有时间戳、
写入/重写/删除计数、字节和吞吐量数字，以及键顺序质量——
可以直接送入日志工具。日志在大小上限之下会自动轮转。

### 记录操作员

```cobol
           OPEN I-O CUSTOMER-FILE WITH REGISTERED USER WS-OPERATOR
```

`OPEN … WITH REGISTERED [USER] {literal | data-item}` 在操作日志中记录*谁*打开了
文件。这**仅供观察**——PowerRustCOBOL 并不
提供认证或授权引擎；该字段只是用你提供的操作员标记日志
条目。

> **注意。** 默认的磁盘格式是自描述的，并存储完整的键
> 模式，所以文件在 `OPEN` 时可以被检查和验证（不匹配
> 会以标准的文件状态码显现）。该格式与任何第三方 ISAM
> **不是**二进制兼容的；不要假设可以与其他厂商的文件互换。

> ⚠️ **注意事项——锁定在两个层面上起作用。**以 `WITH LOCK` 或 `SHARING WITH NO OTHER`
> 打开的整个 INDEXED 文件是**跨进程**独占的：
> 另一个程序对它的 `OPEN` 会得到状态 `93`。**记录**锁
> （`READ … WITH LOCK`、`UNLOCK`）是单进程的——在一个运行的程序之内
> 采用 VSAM/RLS 风格的语义——所以两个进程看不到彼此的记录
> 锁。

---

## 15. SQL 数据库

关系型访问通过单一的 `CALL` 界面暴露，后端
由连接字符串选择：


| 连接字符串以……开头       | 后端          |
| ------------------------------------- | ---------------- |
| `:memory:`、`sqlite:`，或文件路径 | SQLite（内置） |
| `postgres://` / `postgresql://`       | PostgreSQL       |
| `mysql://`                            | MySQL            |

典型的流程：

```cobol
           COBOL::"OPEN-DB" ( "sqlite:app.db" WS-DB WS-STATUS )
           COBOL::"EXEC-SQL" ( WS-DB
               "SELECT id, name FROM customers WHERE active = 1"
               WS-ROWS WS-STATUS )
      *>   The first row is the current one; NEXT-ROW says whether
      *>   there is another.
           IF WS-ROWS > 0
               MOVE "Y" TO WS-MORE
           END-IF
           PERFORM UNTIL WS-MORE NOT = "Y"
               COBOL::"FETCH-ROW" ( WS-DB 1 WS-ID WS-STATUS )
               COBOL::"FETCH-ROW" ( WS-DB 2 WS-NAME WS-STATUS )
               ...
               COBOL::"NEXT-ROW" ( WS-DB WS-MORE )
           END-PERFORM
           COBOL::"CLOSE-DB" ( WS-DB ).
```

这些驱动是纯 Rust 实现并且内置的（无需安装 `libpq`/OpenSSL）。对 SQL 事务，请使用
带 `BEGIN`/`COMMIT`/`ROLLBACK` 的 `COBOL::"EXEC-SQL"`。完整
参考：`docs/database-runtime-en.md`。

> **注意。** 你可以用 **SQL Database**
> 非可视控件以可视化方式为数据库连接建模（它的属性保存连接字符串、驱动，以及
> 它的事件所填充的数据项），或者完全通过上面的
> `CALL` 在代码中驱动它。

### 嵌入式 SQL：`EXEC SQL`

如果你的 PowerCOBOL 或 isCOBOL 程序与数据库交谈，那么它们几乎
肯定是用经典的方式来做的：SQL 直接写在 COBOL 源码中，位于
`EXEC SQL` 和 `END-EXEC` 之间，并与称为
**宿主变量**的 COBOL 数据项交换值。RustCOBOL 直接读取这种风格。

> ⚠️ **注意事项——试过哪些服务器。** 嵌入式 SQL 可以对
> **SQLite、PostgreSQL 和 MySQL** 运行——本节中的每一条语句，在
> Run、Debug、Run Form 和 `rcrun` 下，以及在构建出的应用程序中都是如此。本版本
> 自己的测试把同样的程序对一个实际运行的 **PostgreSQL 17** 和一个实际运行的
> **MySQL 26** 运行（连接、每一种值类型、跨批次的游标、
> 工作单元、错误、被服务器结束的连接）；其他服务器
> 版本没有试过，所以请先在测试数据库上运行你自己的程序。重要的
> 差异见下面的**按数据库**。

**块。** `EXEC SQL`、语句、`END-EXEC`——可以是固定格式或自由格式，
跨越你喜欢的任意多行，没有续行标记。 `END-EXEC` 之后的句点
结束句子，恰如它出现在 `MOVE` 之后一样；没有句点时，
这个块只是另一条语句，所以它可以位于 `IF` 或
`PERFORM … END-PERFORM` 之内：

```cobol
           IF WS-DELETE-REQUESTED
               EXEC SQL
                   DELETE FROM ORDERS
                    WHERE ORDER-NO = :WS-ORDER-NO   -- the one on screen
               END-EXEC
               MOVE "Deleted" TO WS-MESSAGE
           END-IF
```

在块内部，你可以写 COBOL 注释行（固定格式）、`*>`
注释和 SQL 的 `--` 注释；它们都不会被发送到数据库。SQL 字符串（`'…END-EXEC…'`）、带引号的名称或注释
内部的 `END-EXEC` 不会结束这个块。

**宿主变量。** 数据项名称前的冒号——`:WS-ORDER-NO`——
使它成为宿主变量。你的程序声明的任何项目都可以是宿主变量；值
作为*参数*传给数据库，绝不会被粘贴进 SQL
文本。连字符只在紧跟冒号之后才属于名称：在
`SELECT QTY-1 INTO :WS-QTY-LESS-ONE` 中，`QTY-1` 里的连字符是 SQL 减法，
而 `:WS-QTY-LESS-ONE` 里的连字符是 COBOL 名称的一部分。当两个
项目同名时，请加以限定：`:CITY OF CUSTOMER`，或 `:CITY.CUSTOMER`。宿主变量从不带下标
或引用修改，并且 COBOL 的表意常量（`SPACES`、`ZERO` ……）
在 SQL 内部没有意义——请改写 SQL 字面量。

**指示变量**跟在它的宿主变量之后，带不带单词
`INDICATOR` 都可以——`:WS-PHONE:WS-PHONE-IND` 或 `:WS-PHONE INDICATOR :WS-PHONE-IND`。
指示变量是 `COMP-5`、`COMP`、`BINARY` 或 `DISPLAY`
usage 的 `PIC S9(4)` 项目；对于用作宿主结构的组，则是它们的表。

**在 DATA DIVISION 中。** WORKING-STORAGE、LOCAL-STORAGE 和 LINKAGE 接受
`BEGIN DECLARE SECTION` / `END DECLARE SECTION`（可选——每个项目
无论如何都可以使用）、`DECLARE … TABLE`（仅作文档）、`DECLARE … CURSOR`
和 `INCLUDE`。它们周围的项目仍然是普通的数据项：

```cobol
       WORKING-STORAGE SECTION.
           EXEC SQL BEGIN DECLARE SECTION END-EXEC.
       01  WS-ORDER-NO          PIC 9(8).
       01  WS-CUSTOMER          PIC X(40).
       01  WS-PHONE             PIC X(20).
       01  WS-PHONE-IND         PIC S9(4) COMP-5.
           EXEC SQL END DECLARE SECTION END-EXEC.
           EXEC SQL INCLUDE SQLCA END-EXEC.
```

**`INCLUDE`。** `EXEC SQL INCLUDE name END-EXEC` 引入一个 copybook，查找方式
与 `COPY name` 完全相同。有两个名称是保留的：`SQLCA`（状态
区域——`SQLCODE`、`SQLSTATE`、消息、受影响的行数）和
`SQLDA`（动态 SQL 的描述符）；这两个布局都是 PowerRustCOBOL 自己的。
`COPY` 和 `REPLACE` 从不更改 SQL 块内部的文本。

**`WHENEVER`** 的工作方式与预编译器一贯的做法相同：它适用于
源码中写在它*之后*的 SQL 语句，直到针对同一条件的下一个
`WHENEVER`——而不是碰巧在它之后运行的任何东西。

```cobol
           EXEC SQL WHENEVER SQLERROR GO TO DB-FAILED END-EXEC
           EXEC SQL WHENEVER NOT FOUND CONTINUE END-EXEC
```

**连接。** `CONNECT TO` 指名数据库。**连接字符串**
直接连接——`':memory:'`、`'sqlite:data/sales.db'`、诸如
`'data/sales.db'` 的路径；`AS name` 给连接一个名称，以便稍后切换到它：

```cobol
           EXEC SQL CONNECT TO 'sqlite:data/sales.db' AS SALES END-EXEC
           EXEC SQL CONNECT TO ':memory:' AS SCRATCH END-EXEC
           EXEC SQL SET CONNECTION SALES END-EXEC
           EXEC SQL DISCONNECT SCRATCH END-EXEC
```

只有形状明确的字符串才算连接字符串（诸如
`sqlite:` 的方案、`/` 或 `\`，或以 `.db`/`.sqlite`/`.sqlite3`/`.db3` 结尾）。任何
其他词都被当作项目的某个 SQL 连接的名称，所以拼错的
名称会以 `08001` 失败，而不是悄悄地创建一个空的数据库
文件。`DISCONNECT` 会回滚该连接尚未提交的内容。

**项目的 SQL 连接。** 与其在程序中写明文件或服务器，不如
在项目中命名一次，然后按该名称连接。项目
树中紧跟在 Indexed Files 之后的 **SQL Connections** 项列出它们——
默认的标有 *(default)*——它的 **➕** 添加一个。点击某一行会在主窗格中打开
该连接的编辑器：它的名称、数据库（SQLite，或带有主机、端口和数据库名称的
PostgreSQL 或 MySQL）、默认标记，以及对 SQLite 而言，
第一次运行是否可以创建文件。**Test connection**
用编辑器显示的内容——不论已保存与否——进行连接，并报告成功或
数据库自己的消息。**Remove** 会先询问。服务器连接的用户名和密码
保存在 IDE 的凭据保险库中，也就是保存你的模型 API 密钥的存储，绝不在项目中；
重命名连接会把它们一起带走，而 Run、Debug 和 Run Form 以下面这些变量的形式收到它们。

> 📷 **需要截图 — `sql-connections-editor.png`。** 打开一个项目，
> 点击 **SQL Connections** 上的 **➕**，选择 PostgreSQL，填入一个主机，
> 然后按 **Test connection**；截取带有新行的项目树，以及
> 显示数据库答复的编辑器。

在项目文件中，每一个都是一个 `[[sql-connections]]` 条目：

```toml
[[sql-connections]]
name = "SALES"
path = "data/sales.db"       # relative to the project folder
default = true               # used when the program connects to nothing
create-if-missing = false    # true lets the first run create the file
```

```cobol
           EXEC SQL CONNECT TO 'SALES' END-EXEC
           EXEC SQL CONNECT TO SALES AS ARCHIVE END-EXEC
```

名称的比较不区分大小写。SQLite 文件绝不会在你背后被创建：
除非条目写了 `create-if-missing = true`，否则不存在的路径
会以 `08001` 失败，并指出该文件。标记为
`default = true` 的 SQL 连接，是程序没有建立任何连接时语句所使用的那个——
在一个数据库中运行的程序不必在任何地方写
`CONNECT`。既没有默认也没有连接时，语句会以
`08003` 失败。

每个 SQL 连接都可以不触碰项目就被指向别处，办法是一个以应用程序和连接
命名的环境变量——
两者都大写，除字母或数字之外的每个字符都变成
`_`。对于名为 `Shop` 的项目：

| 变量 | 替换 |
|----------|----------|
| `SHOP_SQL_SALES_URL` | 整个目标——`sqlite:/srv/sales.db` |
| `SHOP_SQL_SALES_USER` | 用户名（服务器数据库） |
| `SHOP_SQL_SALES_PASSWORD` | 密码——唯一接受密码的地方 |

> ⚠️ **绝不要把密码放进项目文件。** 带有
> `password` 键的条目会被拒绝：通过它连接会以 `28000` 失败，而
> 消息会指出文件和应当改用的变量。

**构建出的应用程序的 SQL 连接。** 你用来测试的数据库
很少是应用程序将要使用的那些，所以构建出的应用程序不会
把它的连接带在二进制里面。**Build** 会在程序旁边——在 `bin/` 和
目标文件夹中——写出一个起始的
`sql-connections.toml`，把项目的 SQL 连接列为 `[[connection]]` 条目，
每个 SQLite 路径都变成绝对路径，并且没有用户名和密码。安装
该应用程序的人会编辑这个文件，把每个名称指向真正的
数据库；文件一旦存在，Build 就绝不会覆盖它，所以重新构建会保留
这些编辑：

```toml
[[connection]]
name = "SALES"
path = "/srv/shop/sales.db"
default = true
```

应用程序在启动时读取这个文件，而同样的环境
变量会覆盖它——`SHOP_SQL_SALES_URL`、`SHOP_SQL_SALES_USER`、
`SHOP_SQL_SALES_PASSWORD`。已部署的文件可以带有 `user`，但绝不能带 `password`。
除了它的变量之外，构建出的应用程序还会从它自己的加密密钥存储中取得密码，
条目为 `SQL:SALES`（连接名称的大写形式），由你的应用程序自己填充——比如
从一个设置窗体——并且永远无法读回：

```cobol
           COBOL::"KEY-SET" ( "SQL:SALES" WS-PASSWORD WS-STATUS )
```

没有这个文件时，程序仍然可以通过连接字符串连接。

> ⚠️ **打包的项目不是构建出的项目。** **Package** 交付的是
> 供 `rcrun` 运行的项目，而 `rcrun` 与 IDE 一样，从项目文件读取 SQL 连接；
> 只有 Build 才会写出 `sql-connections.toml`。

**工作单元。** 工作单元从连接之后或上一个 `COMMIT`/`ROLLBACK` 之后的
第一条语句开始——在 SQLite 上，只在*更改*数据的语句之前开始，所以只读取的
程序从不持有数据库的写锁。`EXEC SQL COMMIT` 保留更改，`EXEC SQL ROLLBACK`
丢弃它们。COBOL 动词 `COMMIT` 和 `ROLLBACK`（不带 `EXEC SQL`）
与此无关：它们管理 INDEXED 文件，而两者从不互相影响。

**运行结束。** 应用程序结束时仍然打开的工作会替你
处理妥当，就像 PowerCOBOL 或 isCOBOL 运行时在运行单元
结束时所做的那样：正常结束之后——`STOP RUN`、主程序的 `GOBACK`、
主窗口关闭——每个连接都会**提交**；在运行时错误之后，或
当你在 IDE 中按 **Stop** 时，每个连接都会**回滚**。然后每个
连接都被关闭。以显式的 `EXEC SQL COMMIT` 结束仍然是
更清楚的程序。

**窗体共享它们的 SQL 连接。** 应用程序的每个窗体——主
窗体、它打开的窗口、加载进侧边菜单窗格的窗体——使用
同样的 SQL 连接。（只有连接是共享的：每个窗体保留它自己的
`WORKING-STORAGE`。）进行连接的窗体会让该连接对所有窗体
成为当前连接，之后打开的窗体通过它读取（包括未提交的
行），而关闭窗体只释放它自己的游标和预处理语句：
连接以及其上的工作属于应用程序，并在**主**窗体结束时
处理妥当。子窗体中的运行时错误不会回滚
任何东西。

**游标**一次读取查询的一行。声明游标——在
WORKING-STORAGE 中或在 PROCEDURE DIVISION 中，位于使用它的语句
之前——然后打开它，一直取到没有数据，再关闭它：

```cobol
       WORKING-STORAGE SECTION.
           EXEC SQL
               DECLARE C-ORDERS CURSOR FOR
                   SELECT ORDER-NO, TOTAL FROM ORDERS
                    WHERE CUSTOMER = :WS-CUSTOMER
                    ORDER BY ORDER-NO
                   FOR UPDATE OF STATUS
           END-EXEC.
      ...
           EXEC SQL OPEN C-ORDERS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-ORDERS INTO :WS-ORDER-NO, :WS-TOTAL
               END-EXEC
               IF SQLSTATE = "00000" AND WS-TOTAL > 1000
                   EXEC SQL UPDATE ORDERS SET STATUS = 'REVIEW'
                             WHERE CURRENT OF C-ORDERS END-EXEC
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-ORDERS END-EXEC
```

- `OPEN` 在那一刻读取游标的宿主变量：之后更改
  `WS-CUSTOMER` 不会改变已打开的游标。
- 越过最后一行的 `FETCH` 报告没有数据（`02000`），并让 `INTO`
  项目保持不变。
- 对未打开的游标做 fetch 或 close，或打开已经打开的游标，
  都会以 `24000`（无效的游标状态）失败，并且什么也不改变。
- `UPDATE … WHERE CURRENT OF` 和 `DELETE … WHERE CURRENT OF` 作用于
  游标最后取到的那一行。游标必须只读取一张表——连接、表的
  列表、`DISTINCT` 或 `GROUP BY` 会得到 `0A000`。
- `FOR UPDATE OF STATUS` 按标准所说，指名游标可以更改的**列**。
  PostgreSQL 和 MySQL 把这些词读作表名，所以程序
  向它们发送 `FOR UPDATE`，而游标读取的行会被整体
  锁定；SQLite 两者都不需要。
- 在窗体的 WORKING-STORAGE 中声明的游标为窗体的事件
  处理程序所知；在处理程序内部声明的游标属于该处理程序。同一窗体的
  两个同时打开的副本各自有它们自己的游标。
- `EXEC SQL COMMIT` 关闭每个游标，声明为 `WITH HOLD` 的除外；
  `EXEC SQL ROLLBACK` 关闭所有游标。

> **注意——大型结果。** 在 SQLite 和 MySQL 上，游标的行是在它
> 打开时读取的，所以非常大的结果会一直保存在内存中，直到游标
> 被关闭；请在 `WHERE` 子句中过滤，而不是在 COBOL 中。PostgreSQL 保留
> 游标本身，并在程序 fetch 时分批交出行，
> 所以在那里大型结果不必放进内存。`FOR UPDATE` 在 SQLite 上
> 被接受并忽略——它会在写入期间锁定整个数据库。

**动态 SQL** 用于程序在运行时构建的语句——用户选择过滤条件的
搜索屏幕、对运行时才命名的表做的报表。先准备一次文本，然后按需要
多次运行；`?` 标记每个参数：

```cobol
           MOVE "UPDATE STOCK SET QTY = QTY - ? WHERE CODE = ?"
             TO WS-STMT
           EXEC SQL PREPARE TAKE-STOCK FROM :WS-STMT END-EXEC
           EXEC SQL EXECUTE TAKE-STOCK USING :WS-TAKEN, :WS-CODE
           END-EXEC
           EXEC SQL EXECUTE IMMEDIATE 'DELETE FROM STOCK WHERE QTY = 0'
           END-EXEC
```

- `PREPARE` 保留那一刻的文本；之后更改 `WS-STMT`
  不会改变 `TAKE-STOCK`。
- `EXECUTE IMMEDIATE` 立即准备并运行不带
  参数、不返回行的语句。
- 查询通过在语句名称上声明的游标读取：
  `DECLARE C-FIND CURSOR FOR FIND-STMT`，然后 `OPEN C-FIND USING :WS-LOW,
  :WS-HIGH`。
- 执行从未准备过的语句名称会以 `07003` 失败。

> ⚠️ **注意事项——动态 SQL 运行你准备的文本。** 宿主变量总是
> 被绑定，绝不会被粘贴进 SQL——但 PREPARE 的文本是你的，
> 而 RustCOBOL 原样发送它。绝不要通过拼接
> 用户键入的内容来构建它：把用户的值放在 `?` 之后，并用 `USING` 传入。

**描述符区域（SQLDA）**描述程序在编写时不知道的列或参数。
`EXEC SQL INCLUDE SQLDA END-EXEC` 声明一个可容纳 100 个条目的描述符：

| 字段 | 保存 |
|---|---|
| `SQLDA-TAG` `PIC X(8)` | `RCSQLDA1` |
| `SQLDA-CAPACITY` | 程序允许它使用多少个条目（100） |
| `SQLDA-NEEDED` | 最近一次 `DESCRIBE` 需要多少个条目 |
| `SQLDA-COUNT` | 正在使用多少个条目 |
| `SQLDA-ENTRY OCCURS 100` | 每个列或参数一个： |
| ·  `SQLDA-NAME` `PIC X(128)` | 列的名称（对于参数是 `?1`、`?2` ……） |
| ·  `SQLDA-TYPE`、`SQLDA-TYPE-NAME` | 类型代码——1 整数、2 十进制、3 浮点、4 字符、5 二进制、6 日期、7 时间、8 时间戳、9 布尔、0 未知——以及数据库自己的类型名称 |
| ·  `SQLDA-LENGTH`、`SQLDA-PRECISION`、`SQLDA-SCALE`、`SQLDA-NULLABLE` | 按声明；数据库没有说明时为 −1 |
| ·  `SQLDA-DATA`、`SQLDA-IND-PTR` `USAGE POINTER` | 值及其指示变量去往何处，设置之后 |
| ·  `SQLDA-IND`、`SQLDA-VALUE` `PIC X(1024)` | 当 `SQLDA-DATA` 为 NULL 时，以文本形式给出的指示变量和值 |

```cobol
           EXEC SQL PREPARE ANY-QUERY FROM :WS-STMT END-EXEC
           EXEC SQL DESCRIBE ANY-QUERY INTO SQLDA END-EXEC
           IF SQLSTATE = "01005"
      *>       More columns than SQLDA-CAPACITY: SQLDA-NEEDED says how many.
               ...
           END-IF
           EXEC SQL DECLARE C-ANY CURSOR FOR ANY-QUERY END-EXEC
           EXEC SQL OPEN C-ANY END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-ANY USING DESCRIPTOR SQLDA END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-I FROM 1 BY 1
                           UNTIL WS-I > SQLDA-COUNT
                       DISPLAY SQLDA-NAME (WS-I) " = " SQLDA-VALUE (WS-I)
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-ANY END-EXEC
```

每个条目可以用两种方式交付它的值。通过
`SET SQLDA-DATA (i) TO ADDRESS OF item` 设置了 `SQLDA-DATA (i)` 时，值直接进入该
项目，转换方式与 `INTO :item` 完全相同（`SQLDA-IND-PTR` 同样
指名它的指示变量）。`SQLDA-DATA (i)` 为 NULL——它的初始状态——时，
值以文本形式到达 `SQLDA-VALUE (i)`，它的指示变量到达 `SQLDA-IND (i)`，
这正是通用的显示屏幕所需要的。`DESCRIBE INPUT` 改为描述
语句的参数；SQLite 报告有多少个，但不报告
它们的类型，所以这些条目说的是*未知*。需要超过
100 个条目的程序，声明自己的布局副本，带有更大的 `OCCURS` 和
`SQLDA-CAPACITY`，并在 `INTO` / `USING DESCRIPTOR` 中指名它。

**值如何传递。** 每个宿主变量都作为绑定的
参数到达数据库。下表是双向使用的对应关系：

| COBOL 项目 | 发送为 | 接收自 |
|---|---|---|
| 带 `V` 的 `PIC 9`/`S9`——DISPLAY、`COMP-3`/`PACKED-DECIMAL`、`COMP`/`BINARY`/`COMP-5` | 精确的十进制数（没有小数时为整数） | 整数、十进制数、浮点数或数值文本，截到项目的小数位数 |
| `COMP-1`、`COMP-2` | 浮点数 | 任何数字 |
| `PIC X`、`PIC A` | 文本，**去掉尾部空格** | 任何值的文本，截到项目的长度 |
| 数值编辑型 | 它的编辑后文本 | 数字，按项目的 PICTURE 编辑 |
| 日期、时间或时间戳 | 保存 ISO 文本（`2026-10-06`）的 `PIC X(10)` / `X(8)` / `X(26)` | ISO 文本 |

接收到的值在存储任何东西之前都会被检查，并且要么每个
`INTO` 项目都被设置，要么都不设置：整数部分放不进
项目的数字——或者对无符号项目而言的负数——会以 `22003` 失败；
数值项目收到非数值文本会以 `22018` 失败；没有指示变量的项目收到 NULL 会以
`22002` 失败。比项目更长的文本会被截断，并报告为
警告 `01004`，原始长度放在指示变量中。找不到行的 `SELECT … INTO`
报告没有数据（`02000`），并保持它的项目不变；找到多行的
会以 `21000` 失败（请使用游标）。不触及任何行的 `INSERT`、`UPDATE` 或
`DELETE` 同样报告没有数据。

> **注意——SQLite 与十进制数。** SQLite 把最多 15 位
> 有效数字的十进制数存为精确的双精度数；更长的则作为文本发送，因此不会丢失
> 任何数字。请把金额列声明为 `NUMERIC`，并保持 `PIC S9(13)V99` 或
> 更小，如果你需要对它们做 SQLite 算术的话。

**了解语句执行得如何。** 每条语句之后，程序的状态
项目都会被设置——它声明了其中哪些就设置哪些：

- 独立的项目 `SQLSTATE`（`PIC X(5)`）、`SQLCODE`（有符号整数）
  和 `SQLMSG`（字母数字，数据库的消息，截到该项目的长度）——这是
  迁移来的 PowerCOBOL 程序所使用的风格；
- SQLCA，来自 `EXEC SQL INCLUDE SQLCA END-EXEC`：

| SQLCA 字段 | 保存 |
|---|---|
| `SQLCA-TAG` `PIC X(8)` | `RCSQLCA1` |
| `SQLCODE` `PIC S9(9) COMP-5` | 见下面的规则 |
| `SQLSTATE` `PIC X(5)` | 标准的五字符代码 |
| `SQLCA-ROWS` `PIC S9(18) COMP-5` | 受影响或读取的行数 |
| `SQLCA-MESSAGE-LENGTH`、`SQLCA-MESSAGE` `PIC X(512)` | 数据库的消息 |
| `SQLCA-NATIVE-CODE` | 数据库自己的错误编号 |
| `SQLCA-WARNING`、`SQLCA-TRUNCATED` `PIC X` | 任何警告之后 / 值被截断之后为 `W` |
| `SQLCA-CONNECTION` `PIC X(64)` | 当前连接的名称 |

SQLSTATE 遵循标准的类别——`00000` 成功、类别 `01` 警告、
`02000` 没有数据、其他一切都是错误——并保留数据库自己的消息。
SQLCODE 由它按一条规则推导，所以两者从不矛盾：

| SQLSTATE | SQLCODE |
|---|---|
| `00000` | 0 |
| `02000` | +100 |
| 警告（类别 `01`） | 五个字符全是数字时，取其数值（`01004` → +1004），否则为 +1000 |
| 错误，全是数字 | 该数字取负（`23505` → −23505） |
| 子类别带有字母的错误 | −(类别 × 1000)（`42P01` → −42000） |
| 任何其他错误 | −99000 |

SQL 失败**本身从不会停止程序**，与文件状态完全一样：
没有生效的 `WHENEVER SQLERROR GO TO` 时，下一条语句会运行，
而查看状态是程序的工作。

```cobol
           EXEC SQL
               SELECT NAME, BALANCE
                 INTO :CUST-NAME, :CUST-BALANCE:CUST-BALANCE-IND
                 FROM CUSTOMER
                WHERE ID = :CUST-ID
           END-EXEC
           EVALUATE SQLSTATE
               WHEN "00000" CONTINUE
               WHEN "02000" MOVE "No such customer" TO WS-MESSAGE
               WHEN OTHER   MOVE SQLMSG TO WS-MESSAGE
           END-EVALUATE
```

**Check 告诉你什么**——在任何东西运行之前，并且不接触任何
数据库。每条消息都指名它的行：

| Check 报告 | 示例 |
|---|---|
| 没有 `END-EXEC` 的块 | `unterminated EXEC SQL block (missing END-EXEC)` |
| 位于 SQL 不能出现之处的块（FILE SECTION），或 DATA DIVISION 中的可执行语句 | |
| 未声明或有歧义的宿主变量或指示变量 | `host variable :WS-NAM is not declared in the DATA DIVISION` |
| 不是 `PIC S9(4)` 的指示变量 | |
| 带下标的宿主变量，或 SQL 内部的表意常量 | |
| 在声明之前使用或被声明两次的游标 | |
| 对之前没有准备的语句做 `EXECUTE` 或 `DESCRIBE` | |
| 不指名任何段落或章节的 `WHENEVER … GO TO` | |
| 找不到其 copybook 的 `INCLUDE` | |
| 在 `CONNECT … USING` 中写成字面量的密码 | |

Check 只验证 COBOL 一侧：表名和列名，以及 SQL 方言
本身，要等到语句运行时才由数据库来判断。

**按数据库。** 同样的语句可以到达这三个数据库；以下是
程序可能注意到的差异。

| | SQLite | PostgreSQL | MySQL |
|---|---|---|---|
| 工作单元开始于 | 更改数据的第一条语句之前 | 第一条语句之前 | 第一条语句之前 |
| 工作单元之内失败的语句 | 该单元继续 | 该单元继续（每条语句都在保存点之下运行） | 该单元继续 |
| 工作单元之内的 `CREATE`/`ALTER`/`DROP` | 属于该单元 | 属于该单元 | **提交该单元**（MySQL 的规则）；下一条语句开始新的单元 |
| `WHERE CURRENT OF` 按什么指名行 | 它的 `rowid` | 游标自己的位置——服务器的 `CURRENT OF` | 表的**主键**——没有主键的表会得到 `0A000` |
| 游标的行 | 在它打开时读取 | 由服务器保留，分批 fetch，每批 1000 行（对用 `WHERE CURRENT OF` 更新的游标则一次一行） | 在它打开时读取 |
| 读取中的单元看到其他连接的提交的情况 | 最新的，每条语句都是 | 最新的，每条语句都是 | **该单元第一条语句时的快照**，直到 `COMMIT` 或 `ROLLBACK`——请结束轮询循环的单元以看到新行 |
| 被服务器结束的连接 | — | `57P01`，带有服务器自己的话 | `08006` |
| `UPDATE` 计数 | `WHERE` 匹配的行，即使某个值保持原样 | 相同 | 相同——连接请求 MySQL 统计匹配的行，而不是被更改的行 |
| 宿主变量无法接受的值 | — | 数组、范围、几何和其他特殊类型会得到 `0A000`：请在查询中转换它们（`::text`） | — |
| `DESCRIBE INPUT` 的参数类型 | 未知 | 已知 | 未知 |
| 描述符中列的名称 | 与书写时一致 | **小写**，除非带引号 | 与书写时一致 |
| 列的长度、精度、小数位数 | 来自声明的类型 | 来自类型：`VARCHAR(10)`、`NUMERIC(9,2)` | 来自类型，长度以字符计 |

在 SQLite 和 MySQL 上，对非常大的表的查询应当放在
缩小范围的 `WHERE` 之后。在 PostgreSQL 工作单元之内失败的语句
不会让它付出代价，但每条语句都要付出一个保存点——大约两次往返，而不是
一次——所以在慢速网络上运行数千条语句的程序
应当记住这一点。访问 PostgreSQL 和 MySQL 不带 TLS：请
在可信的网络上使用它们，或通过隧道使用。

**在编辑器和调试器中。** COBOL 编辑器把 `EXEC SQL`
块绘制为 SQL——关键字、字符串和注释使用它们的颜色，跨越块所占的
所有行，每个宿主变量都绘制为它本来的 COBOL 名称——
并在 `END-EXEC` 之后回到 COBOL。在宿主变量上的 **Go to definition**（F12，或在 macOS 上按 ⌘ 点击 /
其他系统按 Ctrl 点击）会落在数据
项的声明上；在窗体的事件处理程序中，它会打开声明它的位置，
例如窗体的 WORKING-STORAGE。调试器把块作为
一条语句来跳过，所以断点要设在它的 `EXEC SQL` 行上——设在
块内部的会被拒绝，并说明原因。每条语句之后，停靠区的 **SQL** 标签页
会显示发送了什么，宿主变量所在的位置显示 `?`，同时显示绑定到
它们的值，以及 SQLSTATE、SQLCODE、行数和消息；`CONNECT` 的密码
只显示为 `******`。

> 📷 **需要截图 — `debugger-sql-tab.png`。** 调试一个带有使用宿主变量的
> `EXEC SQL INSERT` 的程序，跳过它，并截取选中了
> **SQL** 标签页的停靠区。

### `COBOL::SqlDatabase`：不用 `EXEC SQL` 的 SQL

Web 或 Mobile 页面从不持有数据库，所以 SqlDatabase 控件**不在 Web 和
Mobile 的工具箱中**：数据库属于 Backend。Backend 程序——或任何在
原生运行时上运行的程序——使用 **`COBOL::SqlDatabase`**，即控件自己的动词，位于每个
程序都拥有的一个对象上：`Open`、`Execute`、`Query`、`Fetch`、`FetchAll`、`ColumnNames`、`BeginTransaction`、
`Commit`、`Rollback`、`Close`，以及 `LastError` 和 `StatusCode`，适用于 SQLite、PostgreSQL 和 MySQL。
窗体上没有控件，没有 `EXEC SQL`，没有宿主变量——因此**没有参数**：请用
程序所信任的值自己构建语句文本，并且绝不要把原始用户输入放进其中。这些
动词、它们的答案和一个完整的示例，见第 24 章的*数据库：`COBOL::SqlDatabase`*。

---

## 16. HTTP / REST 与 AI 智能体

- **HTTP/REST。** `COBOL::"HTTP-GET"`、`"HTTP-POST"`、`"HTTP-PUT"` 和 `"HTTP-DELETE"` 发出请求；
  `COBOL::"HTTP-SET-HEADER"` / `COBOL::"HTTP-CLEAR-HEADERS"` 管理头部；
  `COBOL::"HTTP-DOWNLOAD"` 把二进制响应——照片、PDF——直接
  保存到文件中（见 §13 中的*通过 HTTP 下载文件*）。
  **REST Client** 非可视控件为你提供可设计的端点，带有四个
  可绑定的事件：`onComplete`、`onError`、`onTimeout` 和 `onCancelled`。
- **AI 智能体。** **AI Agent** 非可视控件为与大型语言模型
  的连接建模——它的端点、模型、系统提示、温度和令牌
  限制——并为你的 COBOL 处理程序引发事件：回复到达时是 `onResponse`，
  没有到达时是 `onError`，并且——打开 `StreamReply` 时——
  在回复仍在写作时是 `onPartialReply`。

> ⚠️ **注意事项。** 网络功能会触及外部世界——请在 COBOL 中处理错误和
> 超时。请把凭据视为运行时配置，绝不要作为设计的
> 一部分。
>
> **窗体文件不能携带凭据。** 保存凭据的三个属性
> ——REST Client 的 `AuthToken`、Web Search 的 `ApiKey`、Agent Object 的
> `AgentAPIKey`——存储在**你的机器上**，而不在 `.cfrm` 中。把密钥
> 键入属性窗格，它就进入本地凭据文件；运行中的
> 窗体在启动时会得到它。保存窗体，用文本编辑器打开 `.cfrm`，
> 属性在那里，而且是空的。不论密钥是如何
> 输入的，都是如此，所以密钥不会因为在设计器中键入后被遗忘，
> 就进入你的团队共享的仓库。
>
> 输入框会告诉你它是哪种空：当该控件有密钥存档时是 *stored on this machine*，
> 没有时是 *no key on file*。清空输入框
> 同时也会从机器上撤回密钥——空的输入框绝不意味着
> 被遗忘的凭据仍在代表你进行认证。
>
> 对于多个窗体共享的凭据，请优先使用**命名连接**（REST 和
> Web Search）或 **Model Provider**（Agent Object）：一个输入的地方、一个
> 轮换的地方，而窗体只携带它的 id。

### 配置 REST Client

控件的属性配置**它发送的每个请求**，所以处理程序
通常只有一行——地址和凭据位于属性
窗格中，而不是在你的 COBOL 里重复出现。

#### 本地设置，或项目连接

每个 `RestClient` 都有一个 **`Configuration`** 属性，它决定它的
连接来自哪里：

- **`(Local)`**——默认值，也是迄今构建的每个窗体所使用的：下面列出的
  控件自己的属性。
- **命名的项目连接**——它的地址、方法、认证方案、
  头部和超时会在窗体运行之前取代控件自己的设置。

请在 **Settings ▸ Integrations ▸ REST connections** 中定义连接（点击
项目树的顶部节点 ▸ *Settings*）。给它一个名称、基础 URL、默认
方法、认证方案、超时，以及它的 API 密钥。然后让你喜欢的任意多个
窗体指向它：地址只改一次，使用它的每个窗体都会随之更改，
而不是六个窗体各行其是。

> **两半各存储在哪里——在提交之前这很重要。** 连接
> 本身（名称、URL、方法、认证*方案*、头部、超时）
> 保存在项目文件中，并且**是要提交的**：检出该项目的同事
> 会得到你的连接。**API 密钥不是其中的一部分。**密钥保存在项目之外的
> 本机存储中，
> 绝不会写入项目文件、`.cfrm` 或生成的 COBOL——所以每个
> 开发者提供他们自己的，而共享的仓库从不携带它。
>
> ⚠️ 本机的意思是*在这台机器上、在一个文件中*——它还不是操作
> 系统的钥匙串。请像对待任何本地凭据文件那样对待它。

> **说明。** 控件存储的是连接的 **id**，而不是它的名称，所以
> 在 Settings 中重命名连接不会破坏使用它的窗体。选中连接期间，
> 控件自己的设置会保留，并且在你切换回 `(Local)` 的那一刻
> 再次生效。如果窗体指名的连接
> 项目已经没有了，这会被报告，而不是悄悄地回退
> 到本地设置——控件被告知要忽略那些设置，而悄悄地
> 使用它们，会把请求发送到你已经覆盖掉的地址。

##### 交付使用连接的应用程序

连接随构建一起走：`rcrun build` 把项目的
连接烘焙进二进制，所以构建出的应用程序无需旁边的
项目文件就能解析它们。**密钥不会随构建一起走**——这
正是把它保持在项目之外的用意——所以运行该应用程序的机器通过环境变量
提供它，每个连接一个：

```bash
COBOLT_CONNECTION_KEY_<CONNECTION-ID> = <the key>
```

连接 id 是连接缺失时属性窗格中显示的那个，
破折号写成下划线，整个字符串大写——
所以 id 为 `3f2a-91bc` 的连接从
`COBOLT_CONNECTION_KEY_3F2A_91BC` 读取。每个连接一个变量，而不是一个
编码的整块，这样部署脚本就能准确设置那台机器
应当持有的密钥，运维团队也能看出哪个值放在哪里。

> **说明。** 当你在 IDE 中工作时，你从不需要设置这些：**Run Form**
> 会从你的本机存储中解析每个密钥，并把它交给运行中的
> 窗体本身，而且只针对该窗体实际使用的连接。在项目内部的
> `rcrun run-form` 也是如此。密钥缺失的控件
> 表现得像任何未认证的请求——服务以 401 回答，
> 它像任何其他失败一样到达 `onError`。

- **`BaseURL`**——控件请求的地址。**不带参数**调用的动词
  会原样使用它，这是普通的情形：

  ```cobol
           RestClient-1::get()
  ```

  **相对**参数会被连接到它上面（`orders/42` 变成
  `https://api.example.com/v1/orders/42`）；以 `?` 开头的参数
  会给它附加查询字符串；而带有自己方案的参数
  （`https://...`）会原样使用——所以已经传入
  完整 URL 的处理程序，行为与以前完全一样。
- **`AuthType`** 和 **`AuthToken`**——应用于每个请求：


  | `AuthType` | 发送的头部                                                                                     |
  | ---------- | ----------------------------------------------------------------------------------------------- |
  | `None`     | *（无）*                                                                                        |
  | `Bearer`   | `Authorization: Bearer <AuthToken>`                                                             |
  | `Basic`    | `Authorization: Basic <AuthToken>`——当令牌写成 `user:password` 时会替你编码 |
  | `APIKey`   | `X-API-Key: <AuthToken>`                                                                        |

  **空的 `AuthToken` 根本不发送头部**，而不是发送空的头部，所以
  未配置的控件会以“未认证”失败，而不是看起来像
  服务器故障。需要把密钥放在不同头部名称之下的 API，
  请为此使用 `DefaultHeaders`。
- **`DefaultHeaders`**——`key: value`，每行一个，随每个请求发送。没有
  冒号的行会被忽略。在运行时用
  `COBOL::"HTTP-SET-HEADER"` 设置的头部会**覆盖**这里所指名的：显式调用
  比设计时的配置更具体。
- **`DefaultMethod`**——当 `Call()` 没有被给出方法
  参数时它所使用的动词。具名动词（`get`、`post`、`put`、`delete`）总是使用它们自己的。
- **`FollowRedirects`**——跟随 `3xx` 响应（默认：是）。关闭时，
  交付的是重定向响应本身。
- **`VerifyTLS`**——验证服务器的证书和主机名
  （默认：是）。
- **`TimeoutSeconds`** / **`TimeoutMs`**——在 `Sync`
  和 `Async` **两种**模式下都限制请求。

一套完整的处理程序——启动调用的点击，以及它可以
结束的两种方式——其他一切都在设计器中配置：

```cobol
      *> Button-1 :: onClick
           RestClient-1::get()

      *> RestClient-1 :: onComplete
           MOVE RestClient-1::ResponseBody TO TextBox-1::Text

      *> RestClient-1 :: onError
           MOVE RestClient-1::LastError TO TextBox-1::Text
```

> **注意。** `Call()` 把动词作为它的第一个参数——
> `RestClient-1::Call("PATCH", "orders/42", WS-BODY)`——这就是你到达
> `PATCH` 以及任何没有自己具名方法的其他动词的方式。以空
> 动词调用时，它使用 `DefaultMethod`。

> ⚠️ **注意事项。** 只有在针对使用自签名证书的开发服务器时，才关闭 `VerifyTLS`。
> 验证关闭时，没有任何东西能把真正的服务器与在该地址回答的
> 其他东西区分开——绝不要以这种方式交付窗体。 `AuthToken` 不需要这样的
> 小心：窗体文件无法携带它（见上面的
> 注意事项），所以不论你是否有意如此，它都是运行时配置。

### 异步 I/O（`Mode`、`Busy`、`TimeoutMs`、`Cancel()`）

`RestClient` 调用在运行期间不再阻塞整个窗体。该控件
**默认是异步的**：`GET` / `POST` / `PUT` / `DELETE` 启动一个
后台工作线程，设置控件的 `Busy` 标志，并立即返回。事件
循环继续分派（定时器的滴答、点击、其他控件），而
响应稍后作为同一控件上的事件到达：

- `onComplete`——响应到达了；在处理程序中读取 `ResponseBody` / `StatusCode`。
- `onError`——传输失败（没有 HTTP 状态）；`LastError` 保存
  消息，而 `StatusCode` 是 `0`。
- `onCancelled`——你在请求进行中调用了 `Cancel()`。
- `onTimeout`——请求超过了 `TimeoutMs` 仍未完成。它已被
  取消，而 `LastError` 这样说明——*No answer within N seconds: the
  call was cancelled.*——所以在 `onError` 中显示 `LastError` 的处理程序
  在这里也可以显示它。AI 智能体的 `Ask` 也是如此。

控件界面，位于 `RestClient` 和 `WebSearch` 上：

- **`Mode`**（`Async` / `Sync`）——两者默认都是 `Async`。（`IndexedFile` 曾经
  也带有 `Mode`、`Busy` 和 `TimeoutMs`，用于一条计划过但从未构建的
  异步路径：它的外观是普通 COBOL，其中 `READ` 在下一条语句之前
  就填充了记录。这三个在那里已被淘汰——新的
  控件不再带有它们，仍然设置或读取它们的窗体或程序
  继续工作；这些值只是被忽略，而 `IsBusy()` 回答
  0。`SqlDatabase` 有真正的 `Mode`——见下文。）
- **`Busy`**（只读）——操作进行中时为 `1`。`Busy` 期间的第二次调用
  会被忽略；请轮询 `Busy` 或等待生命周期事件。
- **`TimeoutMs`**——按控件的超时，单位毫秒；`0` 回退到
  旧的 `TimeoutSeconds × 1000`。到期时控件触发 `onTimeout` 并
  清除 `Busy`。
- **`Cancel()`**——立即放弃进行中的操作：`Busy` 被清除，
  `onCancelled` 触发，而被放弃的工作线程的任何迟到结果
  都会被安全地丢弃。在没有进行中操作时调用 `Cancel()` 是空操作。

> ⚠️ **兼容性。** 在 `GET` *之后*的语句上读取 `ResponseBody` 的
> 现有窗体，依赖的是旧的阻塞行为。请把该
> 控件的 `Mode` 设为 `Sync`，以保持原来的同一语句内的结果，或者
> 把读取移进 `onComplete` 处理程序。`COBOL::"HTTP-…"` 内置调用
> 没有改变，并且始终是同步的。

#### 异步的 SqlDatabase

`SqlDatabase` **默认是同步的**——`Query` 和 `Execute` 在语句
内部完成，并返回它们的计数，一如既往。需要几秒钟的报表
查询只会冻结你自己的处理程序，但窗体在此期间
无法响应点击。把控件的 **`Mode`** 设为 `Async`，
`Query` / `Execute` 就改在后台工作线程上运行：它们立即
返回 `0`，`Busy` 打开，而结果以**同步调用引发的
相同事件**到达——所以你已经写好的处理程序继续工作：

- `onQueryComplete`——计数在 **`ResultCount`** 中（`Query` 是结果
  行数，`Execute` 是受影响的行数）；`Fetch()` 照常读取行。
- `onQueryError`——`LastError` 说明原因。
- `onTimeout`——语句运行得比 **`TimeoutMs`** 更久（`0`，即
  默认值，表示没有限制）。`onCancelled`——你调用了 `Cancel()`。

```cobol
       REFRESH-BUTTON--ONCLICK.
           MOVE "Loading…" TO Status-Label::Caption.
           SqlDatabase-1::Query("SELECT NAME, CITY FROM CUSTOMERS").

       SQLDATABASE-1--ONQUERYCOMPLETE.
           MOVE SqlDatabase-1::ResultCount TO WS-ROWS
           PERFORM UNTIL 1 = 2
               MOVE SqlDatabase-1::Fetch() TO WS-ROW
               IF WS-ROW = SPACES
                   EXIT PERFORM
               END-IF
               CustomerList::AddItem(WS-ROW)
           END-PERFORM
           MOVE SPACES TO Status-Label::Caption.
```

> **说明。** 每个控件一次一条语句：`Busy` 打开期间，
> 第二个 `Query` 或 `Execute` 会被忽略，而同一连接上的
> `COBOL::"EXEC-SQL"` 调用会回答它正忙——连接被放在
> 工作线程上了，所以它从不被两个东西同时使用。`Open`、`Fetch` 和
> `Close` 仍然是同步的。超时或被取消的语句不会
> 在数据库中被中断：它在它的工作线程上完成，它的结果
> 被丢弃，而连接在它完成的那一刻就又可以使用了。
>
> ⚠️ **注意事项。** 不要在异步 `Query` 之后的语句上读取 `ResultCount` 或 `Fetch()`
> ——什么都还没有到达。它们的归处是
> `onQueryComplete` 处理程序。

#### SqlDatabase 上的事务

`BeginTransaction()`、`Commit()` 和 `Rollback()` 在控件的连接上打开、保留和丢弃
事务，所以一组 `Execute` 语句要么全部成功要么全部不成功，
而无需把 `BEGIN` 作为文本发送；`IsInTransaction()` 在有事务打开时回答
`1`。每个动词在生效时回答 `1`，不能生效时回答 `0`
（带有 `LastError` 和 `onQueryError`）：第二个
`BeginTransaction()`、没有事务打开时的 `Commit()` 或 `Rollback()`，或者
处于 `Async` 模式的连接，因为它需要它的语句按顺序运行。此连接上
绑定的 DataGrid 的 `Save()` 会加入该事务——见*把编辑保存回数据源*——并把
`Commit()` 留给你。

```cobol
       TRANSFER-BUTTON--ONCLICK.
           MOVE SPACES TO SqlDatabase-1::LastError
           SET WS-OK TO SqlDatabase-1::BeginTransaction()
           SqlDatabase-1::Execute("UPDATE accounts SET balance = balance - 50 WHERE id = 1").
           SqlDatabase-1::Execute("UPDATE accounts SET balance = balance + 50 WHERE id = 2").
           IF SqlDatabase-1::LastError = SPACES
               SET WS-OK TO SqlDatabase-1::Commit()
           ELSE
               SET WS-OK TO SqlDatabase-1::Rollback()
           END-IF.
```

### 数据项：结果落在 WORKING-STORAGE 的何处

有几个非可视控件可以把它们的结果直接交给你的
程序所声明的项目，所以处理程序读取的是普通 COBOL 字段，而不是属性：


| 控件       | 属性                | 落在那里的内容                                                                                                     |
| ------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `RestClient`  | `ResponseDataItem`      | 每个响应体，来自 `::Get` / `Post` / `Put` / `Delete` / `Call`，`Sync` 或 `Async`（替你声明，`PIC X(32767)`）。 |
| `RestClient`  | `StatusDataItem`        | 每个 HTTP 状态；没有响应时为 `0`（替你声明，`PIC 9(4)`）。                                          |
| `RestClient`  | `RequestDataItem`       | *被读取*，而不是被写入：当 `Post` / `Put` / `Call` 没有给出请求体时，它的值就是请求体。                                |
| `SqlDatabase` | `ConnectionDataItem`    | 连接句柄，来自生成的 `<id>-CONNECT` 和 `Open()`。                                          |
| `SqlDatabase` | `ResultSetDataItem`     | `Fetch()` 返回的每一行，以制表符分隔；行用完之后是空格。                                             |
| `IndexedFile` | `StatusDataItem`        | 每个生成的操作之后引擎自己的 FILE STATUS——`00`、`10`、`22`、`23`、`35`、`39` ……                  |
| `IndexedFile` | `CurrentRecordDataItem` | 生成的段落读取的每条记录（`READ … INTO`）。                                                         |

除了表中说“替你声明”的以外，该项目要由你来声明——
与每个其他的 `…DataItem` 规则相同。

```cobol
       01  WS-ORDER-JSON   PIC X(200).
      *> RestClient-1 has RequestDataItem = WS-ORDER-JSON
           MOVE '{"sku":"A-1","qty":2}' TO WS-ORDER-JSON
           RestClient-1::Post("orders")
```

带有 **`AutoConnect`** 的 `SqlDatabase` 在窗体启动时连接——在任何
处理程序运行之前——并在窗体结束时关闭，而不带参数调用的 `Open()`
会打开控件自己的 `ConnectionString`——或者，当它的 **`SqlConnection`**
指名了项目的某个 SQL 连接（见[嵌入式 SQL](#嵌入式-sqlexec-sql)）时，
打开那个 SQL 连接，它会优先于 `ConnectionString`。请从属性面板的
下拉列表中选取名称；窗体只保留名称，
绝不保留数据库在哪里或谁来登录，所以同一个窗体在 IDE 下
访问测试数据库，在构建出的应用程序中访问生产数据库
（通过它的 `sql-connections.toml`）。项目没有定义的名称是
Check 错误，而下拉列表会用 ⚠ 标出它。控件的连接是它
自己的——与 `EXEC SQL` 使用的连接分开，即使两者指名的是
同一个 SQL 连接。`COBOL::"OPEN-DB" ( "sql-connection:SALES" WS-H WS-ERR )`
以同样的方式按名称到达某个 SQL 连接。`IndexedFile` 的
**`OperatorName`**（由 `OPEN … REGISTERED USER` 记录）作为字面量发送，
除非它指名了你的窗体所声明的项目。

> ⚠️ **注意事项。** 在 1.70.236 之前，`IndexedFile` 的 `StatusDataItem` 只保存
> 外观层的猜测——每次 `OPEN` 之后是 `00`，任何被拒绝的写入之后是 `23`——
> 所以缺失的文件或重复的键到达你的程序时是别的东西。
> 它现在保存真正的代码；针对旧的 `23`（重复键）
> 编写的测试必须改为查找 `22`。

> **已淘汰，且无害。** `LoadStrategy`（`.cidx` 定义拥有
> 存储——带有它的窗体仍然会得到它的 `WS-<id>-LOAD-STRATEGY` 项目）、
> `MaximumConnections`（没有连接池），以及 AgentObject 的 `Stream`
> （它在每个较旧的窗体中都被打开而从未被读取——流式输出是
> 新的 `StreamReply`，在你打开之前是关闭的）和 `TargetControls`（智能体
> 从不自己写控件）。它们都不再被提供；设置或读取其中之一的代码仍然
> 可以编译和运行，而该值被忽略。AgentObject 的
> `AgentEndpoint` 现在可以是路径——`/v1/chat/completions`——连接到
> `AgentURL` 的主机上。

### Maps（位置与路线）

**Maps** 控件是一个嵌入式的、可平移/缩放的 **OpenStreetMap**
视图，可选地由真正的 Google Maps API 支持，用于路线、
地理编码、地点和距离数据。底图和数据 API 是
独立的两半，对凭据的需求不同：

- **底图完全不需要 API 密钥。** `CenterLat` / `CenterLng` /
  `Zoom` 定位视图；用户平移并滚动滚轮来
  交互式缩放，当他们这样做时会触发 `onBoundsChanged`（并更新那三个
  属性）。请在属性
  窗格中设置**地图在哪里打开**——*Start latitude*、*Start longitude* 和 *Start zoom*，以十进制
  度为单位——画布就会在你设计时显示那个视图。从 COBOL 写入这三个
  属性会移动地图：

  ```cobol
  MOVE "-23.5614" TO MAP-1::CenterLat
  MOVE "-46.6558" TO MAP-1::CenterLng
  MOVE 16         TO MAP-1::Zoom
  ```

  **缩放是连续的。** 滚轮的一格是一个级别，一如既往，但地图会在几帧之内
  *滑行*到那里，而不是一次到达：在它移动期间，它被绘制在级别**之间**，
  缩放它已经拥有的图块，而指针下面的东西
  始终留在指针下面。标记、路线和区域随底图缩放，所以没有任何东西
  在滑行中漂移。

  `Zoom` 仍然是整数——被获取图块的级别，以及
  处理程序读取或写入的值。地图在滑行中间保持的小数部分是视图
  状态，从不被发布，所以 `onBoundsChanged` 仍然报告整数级别，
  而 `MOVE 16 TO MAP-1::Zoom` 仍然恰好落在 16。

  **尚未到达的图块显示的是地面，而不是一个洞。** 新级别的
  图像需要片刻才能下载下来，而地图已经有同一片地面
  在另一个比例下的图片——所以它使用那张图片，就像每个地图客户端那样：
  放大**时**，来自较低级别的最近一个已加载图块会被放大
  并裁剪到你正在看的那一块；缩小**时**，你刚离开的级别的
  四个图块会被缩小绘制到它们各自的四分之一中。真正的图像
  随后*覆盖在一张图片之上*到达并直接变清晰，而不是替换一块灰色
  方块。 `TileLoadingColor` 只在没有东西可借用时你才会看到——
  某个地方的第一个视图，或者加载失败的图块。

  ⚠️ **只有标记的灰色地图**意味着图块没有
  到达——控件没问题，是下载有问题。图块通过 HTTPS 来自
  `tile.openstreetmap.org`，不需要密钥，所以通常的原因是
  没有网络或有代理挡路。会话的第一次失败会在
  控制台上说明；居中和标记无论如何都继续工作，这就是为什么
  空的底图否则可能被当成一片空旷水域的地图。
- **驾车时间以数字返回**，而不仅是文字。`Directions` 在
  `onComplete` 上回答七个以 TAB 分隔的字段：以文本表示的距离和时长、
  路线摘要，然后是以**米**计的距离、以**秒**计的时长、
  路线的编码折线，以及**考虑当前交通**的时长，单位秒（Google 没有提供时为 0）。
  请根据数字计算；
  绝不要从 `"72,4 km"` 中解析出数字。

  折线就是道路本身，**一步一步**——而不是 Google 也发布的
  缩略图级摘要——所以据它绘制的轨迹贴在高速公路上，
  而不是在它附近。它从不超过 **4,000 个字符**：请为它声明
  `PIC X(4096)`。长到需要超过这个长度的路线，会
  放弃直线路段上多余的点，并保留它的弯道，这就是
  形状在裁剪后得以保留的原因。

  ⚠️ 交通**只提供数字**。Google 通过它自己的 JavaScript 和移动 SDK 提供它的
  交通*图层*，从不作为地图图块，所以
  没有可以绘制的彩色叠加层——但“现在出发需要多久”
  由最后那个字段回答。
- **Routes** 在地图上描绘线路——计划的巡回、驾驶过的路线。在 `Routes` 属性中
  每条路线一行（`id`⇥`colour`⇥`width`⇥`geometry`），或者
  `AddRoute(id, colour, width, geometry)` / `RemoveRoute(id)` / `ClearRoutes()`。
  几何要么是**编码折线**——`Directions` 答案的第六个字段，所以 Google 自己的路线无需转换就能描绘——
  要么是你自己得出的显式 `lat,lng;lat,lng;…` 列表。**不需要 API 密钥**：
  底图是 OpenStreetMap，而几何是你的。

  ⚠️ **路线与道路的贴近程度，恰好等于你给它的点所能达到的程度。** 地图
  绘制每个点而不发明任何点，所以手写的十几个
  路点的列表是一条*计划的走廊*，而不是道路——它在路点之间
  切掉每个弯道，放得越大，它就越明显地离开柏油路面。道路
  几何必须来自路线服务：`Directions` 答案的第 6 个字段
  **一步一步**地携带道路，这才使轨迹贴在
  高速公路上，而不是在它附近。没有任何设置可以让短的路点
  列表沿着道路走；要么添加点，要么询问路线服务。
- **没有 Google 密钥的道路路线**——`TraceRoad(apiKey, fromLat, fromLng, toLat, toLng)` 改为询问 **OpenRouteService**，并在 `onComplete` 上
  回答三个以 TAB 分隔的字段：以**米**计的距离、以
  **秒**计的时长，以及供 `AddRoute` 使用的编码折线。与 `Directions` 相同的
  4,000 字符上限，所以一个 `PIC X(4096)` 就能容纳任一答案。

  **密钥是参数，而不是设置。**请向你的操作员索取它——一个设置了
  `PasswordCharacter` 的 `TextBox`——并传入他们键入的内容：

  ```cobol
       MOVE TXT-ORS-KEY::Text TO WS-ORS-KEY
       IF WS-ORS-KEY = SPACES
           MOVE "Enter your OpenRouteService key first." TO LBL-STATUS::Caption
       ELSE
           INVOKE MAP-1 "TraceRoad" USING
               WS-ORS-KEY "40.4168" "-3.7038" "37.1773" "-3.5986"
       END-IF
  ```

  PowerRustCOBOL 从不存储那个密钥：不在窗体中，不在项目
  清单中，不在任何文件中。写进项目文件的密钥会传给
  与项目共享的每一个人，这就是原因。空白的密钥会在
  `onError` 上失败，而不发起网络调用。

  > **注意。** `Directions` 和 `TraceRoad` 都在同一个 `onComplete`
  > 事件上回答，并且**回答的形状不同**——七个字段对三个。
  > 请记录你调用了哪一个（在 WORKING-STORAGE 中放一个单字符标志
  > 就够了）并据此分支，否则处理程序会把米读作距离的
  > *文本*，把折线读作路线摘要。
- **Regions** 填充区域——销售区域、配送区、覆盖范围。每个区域一行
  （`id`⇥`fill`⇥`stroke`⇥`width`⇥`geometry`），或者 `AddRegion` /
  `RemoveRegion` / `ClearRegions`。请给填充一个 alpha（`#RRGGBBAA`），这样
  它下面的街道仍然清晰可读。区域**可以是凹的**——沿着海岸线的
  区域能够正确填充。同样不需要 API 密钥。

  重用某个 id 会**替换**该路线或区域。否则，随着数据变化而重绘自己的
  地图会堆叠起再也无法移动的重复项。
- **地图绘制的每一种颜色都是一个属性**——地图上没有任何东西是由
  平台固定的。它们位于检查器中 Maps 控件的 **🎨 Colors** 部分，
  起始位置之下，每一个都可以像任何
  其他属性那样从 COBOL 写入：


  | 属性              | 它为什么着色                                                                                       |
  | --------------------- | ----------------------------------------------------------------------------------------------------- |
  | `MarkerColor`         | 图钉本身                                                                                        |
  | `MarkerBorderColor`   | 图钉周围的圆环，使它在繁忙的底图上也能辨认                                                  |
  | `RouteColor`          | 自己的行没有指名颜色的路线                                                                |
  | `RouteCasingColor`    | **每条**路线之下的描边——使细线在混杂的地形上也清晰可读的明亮光晕 |
  | `RegionFillColor`     | 自己的行没有指名填充的区域                                                                 |
  | `RegionBorderColor`   | 自己的行没有指名描边的区域                                                                 |
  | `TileBackgroundColor` | 整个地图之下，在任何图块到达之前                                                      |
  | `TileLoadingColor`    | 尚未到达的单个图块                                                                       |

  每一个起始都是**空**的，意思是地图一直以来绘制的颜色，所以
  不设置其中任何一个的窗体看起来与以前完全一样。**数据所携带的**颜色
  仍然优先：用 `AddRoute` 绘制、带有自己颜色的路线保留它，
  `AddRegion` 的填充和描边也是如此——区域/路线的这三个属性
  只是不指名颜色的行所回退到的值。

  有三个是设置其颜色的*唯一*方式，因为数据没有
  相应的字段：标记没有颜色参数，所以 `MarkerColor` 和
  `MarkerBorderColor` 就是它，而 `RouteCasingColor` 适用于每条路线，
  不论路线自己指名了什么颜色。

  ```cobol
           MOVE "#0F7B6C" TO MAP-1::MarkerColor
           MOVE "#FFFFFF" TO MAP-1::MarkerBorderColor
  ```

  > ⚠️ **注意事项。** `RegionBorderColor` 是空值不是一种颜色
  > 而是一个决定的那个：自己的行没有指名描边的区域，绘制时**没有
  > 边框**。在这里指名一种颜色会给每个这样的区域加上轮廓——这
  > 在有很多小区域的地图上可能比你想要的更多。

  📄 **完整示例**——演示项目中的 `forms/maps/maps-demo.cfrm`：五名
  销售员作为标记、五个彩色的区域、描绘出的马德里 → 格拉纳达，以及
  以公里、分钟和费用表示的驾车时间。除了那个声称调用 Google 的按钮之外，
  每个按钮在没有配置任何凭据时都能工作。
- **Markers** 是地图上的图钉：在 `Markers`
  属性中每个标记一行，以 TAB 分隔（`id`⇥`lat`⇥`lng`⇥`label`⇥`info`）。请优先使用
  `AddMarker(id, lat, lng, label, info)` / `RemoveMarker(id)`，而不是
  自己手工格式化那个字符串。点击底图会触发
  `onMapClick`（主要事件）；点击标记会触发 `onMarkerClick`
  并设置 `SelectedMarkerId`。
- **下面这五个数据方法调用真正的 Google Maps API**，并需要为整个项目配置一次
  **Google Maps API 密钥**（见下面的*数据
  与凭据*）。没有配置密钥时，每一个都会立即失败
  ——`LastError` 说明原因，`onError` 触发——绝不会崩溃，也绝不会
  悄悄地尝试网络：

  ⚠️ **这五个都是异步的——它们不返回答案。** 调用
  启动查找，把 `Busy` 设为 `1`，并立刻带着**空
  字符串**返回；结果稍后在 `onComplete` 事件上，在
  `ResponseBody` 属性中到达。没有同步模式。所以这*行不通*，
  不管它读起来多么像应该行得通：

  ```cobol
        *> WRONG — Geocode returns immediately, before any answer exists,
        *> so WS-GEOCODE-RESULT is always empty.
             MOVE Map1::Geocode("1600 Amphitheatre Parkway") TO WS-GEOCODE-RESULT.
  ```

  请在一个处理程序中启动查找，并在另一个处理程序中读取答案：

  ```cobol
        *> Btn-Find :: onClick — start it
         FIND-ADDRESS-PARA.
             Map1::Geocode("1600 Amphitheatre Parkway, Mountain View").

        *> Map1 :: onComplete — the answer landed in ResponseBody
         ADDRESS-FOUND-PARA.
             MOVE Map1::ResponseBody TO WS-GEOCODE-RESULT.
        *>   WS-GEOCODE-RESULT = "lat<TAB>lng<TAB>formatted address"
             UNSTRING WS-GEOCODE-RESULT DELIMITED BY X"09"
                 INTO WS-LAT WS-LNG WS-ADDRESS.
             MOVE WS-LAT TO Map1::CenterLat.
             MOVE WS-LNG TO Map1::CenterLng.
             MOVE 16     TO Map1::Zoom.

        *> Map1 :: onError — LastError says why
         ADDRESS-FAILED-PARA.
             DISPLAY "Lookup failed: " Map1::LastError.
  ```

  > **注意——`ResponseBody`、`StatusCode`、`LastError` 和 `Busy` 是只读的
  > 运行时属性。** 不要到属性检查器中去找它们：
  > 运行时在有东西要报告时才写入它们，所以它们没有
  > 设计时的值，没有默认值，也不存储在窗体中。它们
  > 与任何其他属性一样被读取，而且只有读取才有意义——
  > 答案不是设置。

  | 方法                                | `onComplete` 在 `ResponseBody` 中留下                          |
  | ------------------------------------- | -------------------------------------------------------------- |
  | `Geocode(address)`                    | `lat`⇥`lng`⇥`formatted_address`                              |
  | `ReverseGeocode(lat, lng)`            | 格式化后的地址                                          |
  | `Directions(origin, destination)`     | 七个字段：`distance_text`⇥`duration_text`⇥`route_summary`⇥`distance_metres`⇥`duration_seconds`⇥`polyline`⇥`duration_in_traffic_seconds`（见上面的*驾车时间以数字返回*） |
  | `DistanceMatrix(origin, destination)` | `distance_text`⇥`duration_text`                               |
  | `PlacesSearch(query, radiusMeters)`   | 每个结果一行 `place_id`⇥`name`⇥`address`⇥`lat`⇥`lng` |

  与每个其他异步控件一样，Maps 提供四个生命周期事件——
  `onComplete`、`onError`、`onTimeout` 和 `onCancelled`——此外还有它自己的
  `onMapClick` / `onMarkerClick` / `onBoundsChanged`。

  > **注意。** 上面的 `X"09"` 是 TAB 的标准十六进制字面量。任何
  > 字节都可以这样写（`X"0D0A"` 是 CR LF）；每*一对*十六进制数字是一个
  > 字符，所以数字的个数总是偶数。

**数据绑定。** Maps 控件可以是独立的绑定目标：把
它的 `Markers` 集合绑定到映射了 `Lat`/`Lng`/`Label` 字段的数据源
（三者都是 Guardian 所要求的；`Id`/`Info` 是可选的），每个
绑定的行就成为一个标记，刷新的方式与绑定的 DataGrid
刷新它的 `Rows` 相同。

### Web Search（五个提供商）

**WebSearch** 控件是一个非可视的搜索客户端，具有与 `RestClient` 相同的异步
生命周期（`Mode`、`Busy`、
`onComplete`/`onError`/`onCancelled`/`onTimeout`，外加它自己的
`onResultsReceived` 作为主要事件）。

它**不绑定于某一个搜索引擎**。 `Provider` 属性选择
后端，而每个后端都通过同样的访问器回答，所以更换
提供商**不需要改动你的 COBOL**——无论你使用的是此表的哪一行，
下面的处理程序都是一样的：

| `Provider` | 凭据 | 另外需要 | `NumResults` 上限 | `SafeSearch` |
|---|---|---|---|---|
| `Google`（默认） | Custom Search API 密钥 | `SearchEngineId`（“cx”值——普通的 id，不是机密） | 10 | `Off` → 关，`Medium`/`High` → 开 |
| `Brave` | Brave Search API 密钥 | — | 20 | `Off` / `Medium` / `High` |
| `Serper` | Serper API 密钥 | — | 100 | **忽略** |
| `Tavily` | Tavily API 密钥 | — | 20 | **忽略** |
| `SearXNG` | **无** | `Endpoint`——你运行的实例的地址 | 50 | `Off` / `Medium` / `High` |

`Provider` 默认为 `Google`，无法识别的值会回退到它，所以
在控件有这个选择之前构建的窗体，行为与以前完全一样。

> ⚠️ **`SafeSearch` 并不通用。** Serper 和 Tavily 不提供过滤
> 级别，所以这个属性根本不会发送给它们。不要假定这两个
> 上有过滤在运行。

> **说明。** `NumResults` 会被限制到所选提供商自己的上限，而不是
> 原样传递，因为向提供商请求超出它所允许的数量，会是
> HTTP 错误，而不是更多的结果。 `SearchEngineId` 只被 Google 读取——其他
> 提供商在不被告知位置的情况下搜索整个网络。**SearXNG** 实例
> 必须在它自己的设置中启用 `format=json`；它默认是关闭的，
> 而禁用了 JSON 的实例会返回控件无法读取的页面（你
> 会得到零个结果，而不是错误）。

设置 `Query`、`NumResults` 和 `SafeSearch`，然后调用 `Search()`。结果
到达 **`onResultsReceived`**，这是控件的主要事件，也是双击所绑定的那个。统一的
`onComplete` 紧随其后被引发，所以绑定在任一个上的处理程序都有效——
请绑定读起来更好的那一个，不要两个都绑：

```cobol
       SEARCH-1--ONRESULTSRECEIVED.
           MOVE SEARCH-1::TopTitle   TO WS-TITLE
           MOVE SEARCH-1::TopSnippet TO WS-SNIPPET
           MOVE SEARCH-1::TopLink    TO WS-LINK
      *>   or walk every result:
           MOVE SEARCH-1::ResultCount TO WS-N
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               MOVE SEARCH-1::GetResult(WS-I) TO WS-RESULT-LINE
      *>       WS-RESULT-LINE = "title<TAB>snippet<TAB>link"
           END-PERFORM.
```

#### 一个搜索引擎，或多个

上面的一切配置的是**一个** WebSearch 控件。从多个窗体搜索的
项目——或者在一处需要 Brave、在另一处需要私有 SearXNG 的
项目——改为定义**命名的搜索连接**，与对
`RestClient` 所做的完全一样：

- **`Configuration` = `(Local)`**——默认值：这个控件自己的 `Provider`、
  `Endpoint`/`SearchEngineId`、`NumResults` 和 `SafeSearch`，密钥来自
  项目唯一的 **Web Search API key**。
- **`Configuration` = 命名的连接**——所有这些都改为来自
  该连接，包括它自己的密钥。本地的那些行会从
  窗格中消失，因为其中每一项都由连接决定。

请在 **Settings ▸ Integrations ▸ Web search connections** 中定义它们：名称、
提供商、它的引擎 id（Google）或实例 URL（SearXNG），以及它的 API 密钥。
SearXNG 不会被要求提供密钥，因为它没有账户。

> **连接不携带 `NumResults` 或 `SafeSearch`。** 它们留在
> 控件上，因为它们是你的 COBOL 在运行时更改的按调用设置——
> 在 `Search()` 之前 `MOVE 10 TO SEARCH-1::NumResults` 是一件
> 平常的事。携带它们的连接会覆盖你设计的内容，并且悄悄地压过
> 你的程序在启动时设置的任何值。

> 与 REST 连接相同的存储划分：连接保存在
> 项目文件中，是要提交的，密钥则绝不会。构建出的
> 应用程序带着烘焙进去的连接，并在运行它的机器上从
> `COBOLT_CONNECTION_KEY_<ID>` 读取每个密钥。

#### 当搜索似乎什么都没做时

请在控件上设置 **`Verbose`**。运行时随后会把整个调用叙述到
程序的输出中——提供商、方法和 URL、请求头、
发送的请求体、它是走了异步还是同步，然后是 HTTP 状态和
**未经裁剪的原始响应**，这样你就可以把它与提供商自己的
文档对照。配置错误也会在那里报告，早于任何东西被
发送。

它之所以存在，是因为什么也没返回的搜索和从未运行的搜索
会产生同样的沉默。`Verbose` 就是把它们区分开的东西。

> **凭据会被遮盖。** 请求头中的密钥，或 Google 在其中签名的 URL
> 查询里的密钥，只会打印它的前几个字符和一个长度——
> 足以看出存在密钥并区分两个密钥，而不会把它放进
> 最终被粘贴到缺陷报告中的输出里。

**密钥来自哪里。**对于 `(Local)` 上的控件：项目级的
搜索凭据（项目的 **Settings ▸ Integrations**），与 Maps
解析它的密钥的方式相同。当某个窗体必须使用与项目默认值不同的账户搜索时，
控件可以用它自己的 `ApiKey` 属性覆盖它——把 `ApiKey` 留空，
就会使用项目的密钥。`SearXNG` 完全不需要密钥；它需要 `Endpoint`。
无论哪种方式，检查都发生在**任何东西被发送之前**：缺少
密钥（或者对 SearXNG 而言缺少 `Endpoint`）的控件会立即以 `onError` 失败，
`LastError` 指出提供商和缺失的设置，并且不发出任何请求。

`WebSearch` 控件还会得到一个生成的 `<id>-SEARCH` 段落
（`PERFORM SEARCH-1-SEARCH`）作为底层的回退，但它做的是普通的、
**未编码**的字符串拼接（多词的 `Query` 会在它的第一个
空格处被截断），从不携带密钥，并且**仅限 Google**——它不遵循
`Provider`，因为其中两个提供商需要带认证头的 POST，
而 `COBOL::"HTTP-GET"` 无法发送它。**请优先使用 `Search()`**，它会
对查询做百分号编码，解析凭据，并遵循 `Provider`。

#### 智能体的凭据存放在哪里

`AgentObject` 也有一个 **`Configuration`** 属性，但它不指向
项目连接。它指向你在 IDE 中配置的某个**模型提供商**
（IDE 自己的 **Settings ▸ Models**，而不是项目的）——
与 Grace 和各位专家使用的是同一个列表。

- **`(Local)`**——默认值：这个控件自己的 `AgentAPI`、`AgentURL` 和
  `AgentAPIKey`。
- **已配置的提供商**——改为使用它的协议、端点和 API 密钥，并且 **API 密钥那一行会从属性窗格中消失**。
  这正是关键所在：提供商的密钥只在一个地方输入一次，
  绝不会被复制到窗体上。`.cfrm` 是人们会提交的文件。

**模型和调优仍然归你**：`AgentModel`、`Temperature`、
`MaximumTokens` 和 `TimeoutSeconds` 即使在控件被绑定时仍然留在控件上，
因为一个提供商提供许多模型，而这个智能体使用哪一个，是
这个智能体自己的属性。

> ⚠️ **这个绑定是限定于机器的。** 模型提供商是按机器配置的，
> 而不是按项目——配置一次 Anthropic 就服务于每个项目——
> 所以打开你的项目的同事，或运行你构建出的
> 应用程序的机器，也需要配置那个提供商。控件会明白地这样说
> （“this machine has no such model provider configured”），而不是假装
> 项目坏了。已部署的应用程序通过
> `COBOLT_AGENT_PROVIDERS` 环境变量接收它们，而每个提供商的密钥
> 通过 `COBOLT_CONNECTION_KEY_<ID>`——与 REST 或 Web Search
> 连接使用的变量相同，其中 `<ID>` 是提供商的 id，大写，
> 破折号写成下划线。

##### 由你的用户选择的模型：模型列表与密钥存储

`Configuration` 在你的应用程序启动之前就已固定。当*使用*
你的应用程序的人应当决定它与哪个模型交谈——添加
公司的模型服务器、轮换密钥、淘汰某个模型——请给他们一个你自己的设置
窗体；他们所选择的属于**应用程序**。

运行时把两样东西分开保存：

- **模型列表。** 每个条目指名你的智能体可以询问的一个模型：它的
  提供商、端点和模型。运行时把这个列表保存在你的应用程序文件夹的
  `settings/models.json` 中，所以从应用程序启动的那一刻起，
  它对应用程序的每个窗体都在那里——你自己的智能体，
  以及添加到应用程序中的诸如 PowerChat 之类的助手，看到的是同样的
  模型。用 `MODEL-SET` / `MODEL-REMOVE` 更改它，用
  `MODEL-COUNT` / `MODEL-GET` 列出它。该文件不保存密钥。
- **密钥。** 你的程序存储一次密钥，并且永远无法把它读回——
  只能询问是否已设置，所以你的窗体可以显示*“密钥已设置”*，
  而不显示密钥本身。

```cobol
       SHOW-MODELS.
      *>   The settings form's list: every model of the application's.
           COBOL::"MODEL-COUNT" ( WS-N )
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               COBOL::"MODEL-GET" ( WS-I WS-NAME WS-API WS-URL WS-MODEL )
               MOVE Lst-Models::AddItem(WS-NAME) TO WS-OK
           END-PERFORM.

       SETTINGS-FORM--SAVE-MODEL.
      *>   Kept by the runtime from here on; no file of yours to update.
           COBOL::"MODEL-SET" ( WS-NAME WS-API WS-URL WS-MODEL WS-STATUS )
           .

       SETTINGS-FORM--SAVE-KEY.
      *>   The administrator typed a key: store it, then forget it.
           COBOL::"KEY-SET" ( MOD-NAME WS-NEW-KEY WS-STATUS )
           MOVE SPACES TO WS-NEW-KEY
           COBOL::"KEY-IS-SET" ( MOD-NAME WS-KEY-FLAG )
      *>   WS-KEY-FLAG is "Y" or "N" — never the key.
           .

       ASK-FORM--ONLOAD.
           MOVE "company-model" TO AGENT-1::ModelEntry.
```

| 内置调用 | 它做什么 |
|---|---|
| `COBOL::"MODEL-SET"( name api url model [status] )` | 添加或更改一个条目。`api` 是下面列表中的提供商 id（`openai`、`anthropic`、`groq`、`ollama`、……），或 `LMStudio` / `Custom`；`model` 可以为空。 |
| `COBOL::"MODEL-REMOVE"( name [status] )` | 撤销一个条目。 |
| `COBOL::"MODEL-COUNT"( count )` | 列表有多少个条目。 |
| `COBOL::"MODEL-GET"( index name [api] [url] [model] )` | 第 *index* 个条目（从 1 开始，按名称顺序）；越过末尾则为空格。 |
| `COBOL::"KEY-SET"( name key [status] )` | 存储或替换某个条目的密钥。 |
| `COBOL::"KEY-REMOVE"( name [status] )` | 移除它。 |
| `COBOL::"KEY-IS-SET"( name flag )` | `Y` 或 `N`。 |

`status` 接收 `OK`，或失败的原因（例如，密钥文件无法写入）。

**向你的用户提供 IDE 的提供商。** IDE 的 Model Providers Manager
了解十七个提供商——OpenAI、Anthropic、Cohere、Google Gemini、
Perplexity、Mistral、Groq、OpenRouter、HuggingFace、Together AI、DeepSeek、
Alibaba、xAI、Voyage AI、Ollama（Local 和 Cloud，算作两个）和
Llamafile——每个都带有它的默认端点、是否需要密钥、如何列出它的模型，以及它的
连接如何测试。构建出的应用程序拥有同样的列表和同样的
规则，而不需要 IDE：

| 内置调用 | 它做什么 |
|---|---|
| `COBOL::"PROVIDER-COUNT"( count )` | 有多少个提供商（17）。 |
| `COBOL::"PROVIDER-GET"( index id label endpoint needs-key )` | 第 *index* 个提供商（从 1 开始，按 IDE 的顺序）：它的 id（`MODEL-SET` 把它当作 `api`）、要显示的名称、它的默认端点，以及 `Y`/`N`。 |
| `COBOL::"MODEL-LIST"( provider endpoint key count status [entry] )` | 询问提供商它提供哪些模型；`count` 接收数量，`status` 接收 `OK` 或 IDE 自己的消息（“Could not list models: …”）。空的端点就是提供商的默认值。 |
| `COBOL::"MODEL-LIST-GET"( index model )` | 该列表的第 *index* 个模型。 |
| `COBOL::"MODEL-TEST"( provider endpoint model key status [entry] )` | 向模型问一个很小的问题，就像 IDE 的 **Test** 按钮所做的那样。`status` 是 `OK`，或需要修复的内容。 |

`key` 为空并带有 `entry` 名称时，会使用为该条目存储的密钥——
所以设置屏幕可以测试已保存的模型，而无需读回它的密钥。

```cobol
           COBOL::"MODEL-LIST" ( WS-PROVIDER WS-ENDPOINT WS-KEY
                WS-COUNT WS-STATUS WS-ENTRY )
           IF WS-STATUS = "OK"
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-COUNT
                   COBOL::"MODEL-LIST-GET" ( WS-I WS-MODEL )
                   MOVE CMB-MODEL::AddItem(WS-MODEL) TO WS-OK
               END-PERFORM
           ELSE
               MOVE WS-STATUS TO LBL-STATUS::Caption
           END-IF
```

这些消息是 IDE 的。提供商已经淘汰的模型会被如此指明；
没有密钥的托管提供商会在任何东西被发送**之前**被拒绝
（未认证的调用会以 401 返回，读起来像是密钥被拒绝）；
401 会附带要检查的内容，按顺序——有效的密钥、过期或轮换过的
密钥、提供商不再提供的模型。

> **注意。** 两个调用都会等待提供商，最长 30 秒（列表）或
> 60 秒（测试）。窗口在此期间继续绘制；请在调用之前在状态
> 标签中放上“Testing…”，让操作员知道为什么没有别的事发生。

**智能体使用哪些设置。** 设置 **`ModelEntry`**——在属性窗格中
或在运行时——该条目就胜出：它的 API、端点和密钥，如果它指名了模型，
还有它的模型。否则是如上所述的 `Configuration`；再否则是
控件自己的属性。`Temperature`、`MaximumTokens` 和 `TimeoutSeconds`
始终是智能体自己的。`KnowledgeBase` 也有 `ModelEntry`，用于它的
`Endpoint` 嵌入器。

**长的答案绝不会悄无声息地被截短。** `MaximumTokens` 是模型
在一次回复中最多可写的量。每个提供商都会说明它的模型为什么停止，
而当原因是那个限制时，智能体不会交给你半个答案。它
把对话连同到目前为止的答案送回去，并要求模型从它停下的地方
继续，最多 `MaximumContinuations` 次（默认 4），然后
把各段拼接起来。`onResponse` 触发一次，带着完整的答案，而
`StreamReply` 智能体会在文本增长时持续显示它。三个属性说明
发生了什么：

| 属性 | 含义 |
|---|---|
| `StopReason` | 模型为什么停止，用提供商自己的话：`stop` 或 `end_turn`（完成）、`length` 或 `max_tokens`（被限制截断）、`tool_calls`。 |
| `ContinuationCount` | 答案不得不被续写多少次。 |
| `Truncated` | 当答案仍不完整时为 1：限制截断了它，而续写次数已经用完（或 `MaximumContinuations` 为 0）。 |

```cobol
       AGENT-1--ONRESPONSE.
           MOVE AGENT-1::LastReply TO WS-ANSWER
           IF AGENT-1::Truncated = "1"
               MOVE "Incomplete answer: ask for less." TO LBL-STATUS::Caption
           END-IF.
```

本地 Ollama 模型还会被告知要保留多少文本（`num_ctx`），按
请求来确定大小。Ollama 自己的默认值是几千个令牌，并且它会悄无声息地丢弃
放不下的提示的开头——也就是给模型的指令和
被询问的文档。

如果该条目不存在，或者它的提供商需要密钥（除本地 Ollama 之外的每个提供商）而
没有存储密钥，`Ask` 会立即以 `onError` 失败并指名
该条目——什么也不发送。当智能体用过的条目被更改或撤销时——由这个窗体
或任何其他窗体——该智能体会引发 **`onModelChanged`**，这样你就可以重新读取
你的设置或选择另一个条目。新的密钥从下一个请求起生效；无需
重启。

> ⚠️ **密钥在哪里，以及谁能使用它们。** 密钥保存在
> 应用程序文件夹的 `settings/model-keys.dat` 中，**由该安装的
> 每个用户共享**。它们用从
> 安装文件夹和机器名称派生的密钥加密：该文件一眼看去无法读取，
> 而在另一台机器上或另一个文件夹中的副本什么也解密不了。
> 它**不**受用户持有的秘密保护——**任何能在那台机器上运行
> 该应用程序的人都可以使用存储的密钥**（尽管绝不能通过你的程序
> 读取它们）。操作系统的钥匙串将在相同的 CALL 之后替换这个
> 文件，而你的程序无需改变。在那之前，
> 请把应用程序安装在只有应当使用这些密钥的人
> 才能运行它的地方。
>
> **说明。**
> - 如果密钥文件无法读取——已损坏，或从别处复制来——它会
>   被报告并保持原样，而存储密钥会在 `status` 中回答原因，
>   直到你把文件移开。重命名机器有同样的
>   效果：请重新输入密钥。
> - 密钥从不显示：不在 `LastError` 中，不在详细的智能体日志中
>   （它把密钥遮盖为 `****`），不在任何发送给模型的内容中。
> - 设置了 `COBOLT_AGENT_PROVIDERS` 的部署继续照常工作。

**与 AI Agent 结合。** 常见的模式：运行搜索，然后让
`AgentObject` 把结果总结到多行 TextBox 中。

`Ask` 是**异步的**。它把调用交给后台工作线程并
立即返回，所以窗体在模型思考时继续绘制并继续响应点击。
因此答案到达*第二个*处理程序——
`onResponse`——并从 `LastReply` 读取：

```cobol
       SEARCH-1--ONCOMPLETE.
           MOVE SPACES TO WS-SUMMARY-PROMPT
           STRING "Summarise these search results in three bullet points: "
                  SEARCH-1::TopTitle " — " SEARCH-1::TopSnippet
             INTO WS-SUMMARY-PROMPT
           AGENT-1::Ask(WS-SUMMARY-PROMPT).

       AGENT-1--ONRESPONSE.
           MOVE AGENT-1::LastReply TO Summary-Box::Text.

       AGENT-1--ONERROR.
           MOVE AGENT-1::LastError TO Summary-Box::Text.
```

> ⚠️ **不要写 `MOVE AGENT-1::Ask(...) TO X`。** `Ask` 返回空
> 字符串——语句结束时回复还不存在——所以那个
> `MOVE` 会悄悄地清空接收字段。这与其他每个
> 非可视控件遵循的约定相同（`RestClient::Get`、`Maps::Geocode`、
> `WebSearch::Search`）：动词启动工作，事件交付结果。
>
> **说明。** `Busy` 从 `Ask` 起一直为真，直到 `onResponse`、`onError` 或
> `onTimeout` 触发，而在它为真时的第二个 `Ask` 会被忽略，而不是
> 竞争——如果用户可能按两次，请测试 `Busy`（或禁用按钮）。
> 两个限制守护着这段等待，两者都以 `onTimeout` 结束，并由 `LastError`
> 说明是哪一个：
> - **`StartTimeoutSeconds`**（默认 60；0 表示没有限制）——模型
>   **开始**它的答案所能花的最长时间。从不开始的模型——卡住、
>   过载，或被要求做它做不到的事——会在那一点被**取消**：
>   它的连接被关闭，而不仅仅是被忽略，所以本地模型不会
>   白白为没人而工作。对于在第一个词之前先思考的模型，
>   或对必须加载的本地模型的第一个问题，请提高它。
> - **`TimeoutSeconds`**——答案一旦开始，它在两段之间
>   可以**沉默**的最长时间。每个智能体请求在线路上都是流式的
>   （你只有在 `StreamReply` 时才能看到各段），所以持续到来的长答案
>   不论总共需要多久，都不会被切断。（*提供工具*的 `Ask`
>   计时方式不同：在那里限制覆盖整个
>   问题——见*让智能体使用工具*。）

**在回复到达时显示它——`StreamReply`。** 长的答案可能需要
许多秒，而这期间一直是空的框会让人觉得出了故障。打开 `StreamReply`，
模型的文本在被写出时就到达你的程序：每秒最多
十次触发 `onPartialReply`，其中 `PartialReply` 保存
到目前为止的回复，`ReplyPiece` 只保存自上次以来新增的部分。Ask
仍然以它一向的方式结束——`onResponse`，一次，完整文本在
`LastReply` 中——所以为不流式的 Ask 编写的处理程序继续工作。

```cobol
       SEND-BUTTON--ONCLICK.
           INVOKE VWR-1::AppendMarkdown("**Assistant:**")
               RETURNING WS-MESSAGE-ID.
           AGENT-1::Ask(Prompt-Box::Text).

       AGENT-1--ONPARTIALREPLY.
           INVOKE VWR-1::AppendToMessage(WS-MESSAGE-ID,
               AGENT-1::ReplyPiece, "Raw").

       AGENT-1--ONRESPONSE.
           MOVE AGENT-1::LastReply TO WS-LAST-ANSWER.
```

> **说明。** 对于普通的 `Ask`，`TimeoutSeconds` 是对*沉默*的限制：
> 每一段到达都会重新开始计时，所以持续到来的长答案
> 不会被切断，而变得安静的提供商仍然会超时。`Busy` 保持为真，直到
> `onResponse`（或 `onError`）。提供工具的 Ask 在途中从不被*显示*——
> 它的各轮由你的程序回答，而不是被读取——尽管每个
> 回合在线路上都像其他任何请求一样是流式的。
>
> ⚠️ **注意事项。** 兼容 OpenAI 的提供商、Anthropic 和 Ollama 都
> 支持流式。忽略流式请求而一次性回答的 `Custom` 端点
> 仍然可以工作——它的回复被整体读取——但对它而言，第一
> 段就是整个答案，所以 `StartTimeoutSeconds` 限制的是整个
> 答案：如果这样的端点的答案很慢，请提高它（或把它设为 0）。

`WebSearch` 被归类为 `RestApi` 类的绑定**来源**（
`RestClient` 使用的同一类——没有单独的 `WebSearch` 来源类），所以
它的响应可以像 RestClient 的响应那样，为 DataGrid/Chart/ComboBox/数组绑定
提供数据。

### 数据与凭据

**Google Maps API 密钥**（Maps 的 Directions/Geocoding/Places/Distance-Matrix
方法）以及**搜索 API 密钥** + **搜索引擎 id**（WebSearch）
在每个项目中配置一次，位于项目 Settings 的 **Integrations** 部分
（点击项目树的顶部节点 ▸ *Integrations*）——
与已经用于 AI 提供商密钥的本机模式相同（见 §5 中的*AI 助手（可选）*）：


| 字段                     | 含义                                                                                         |
| ------------------------- | ----------------------------------------------------------------------------------------------- |
| **Google Maps API key**   | 供 Maps 的五个数据方法使用。OSM 底图本身完全不需要密钥。                    |
| **Search API key**        | 供 `WebSearch` 的 `Search()` 使用——控件所设置的任何 `Provider`（Google、Brave、Serper 或 Tavily）的密钥。`SearXNG` 不需要。控件可以用它自己的 `ApiKey` 属性覆盖它。 |
| **Search Engine id (cx)** | 要查询哪个 Google Custom Search 引擎——普通的、非机密的 id，与密钥分开输入。只有在 `Provider` 为 `Google` 时才读取。 |

这两个密钥——**Google Maps API 密钥**和**搜索 API 密钥**（
搜索引擎 id 是普通的 id，不是密钥）——是**本机的，绝不会写入
项目文件、`.cfrm` 窗体文件或任何生成的 `.cbl`**——AI 助手自己的 API 密钥
已经遵循同样的纪律。运行窗体会把解析出的
密钥作为仅限运行时的值播种到解释器中；它绝不会成为字面的
生成源码文本，所以不会通过共享的 `.cbl` 文件泄露
（Build 和 Run 编译的是完全相同的生成源码）。

### 用 AI 智能体驱动 IDE（MCP）

IDE 本身是可由智能体操作的。本节描述 IDE 通用的
**检查端点**，它让智能体能够查看和驱动 IDE 自己的
窗口。它不是智能体构建你的*应用程序*的方式：为此，
下一节描述的编码智能体工具运行在它们自己的服务器上
（默认端口 5720），并通过产品而不是通过它的
窗口工作。这两个端口始终不同。启动时，IDE 在 `127.0.0.1:5719`
提供 **egui 检查协议**（在 ⚙ *Settings* ▸ AI 下更改端口——
重启后生效；Output 控制台显示监听地址）。通过
它，智能体可以读取实时的控件树，在真实的 IDE
控件上点击和键入，调整窗口大小，并截取屏幕截图。

- **外部智能体**（Claude 和其他 MCP 客户端）通过
  官方的 `egui-mcp` 桥连接——把它配置为指向
  IDE 地址的 MCP 服务器，智能体就获得对每个 IDE 界面的查看和驱动访问。
- **内置的 AI 助手**在进程内使用同样的机制：每个
  请求都包括渲染出的控件树的快照以及窗体
  模型，所以模型能推理你的窗体实际上是什么样子——并且在
  应用更改之后，它会重新读取该树来验证它们。

> ⚠️ **注意事项。** 该端点只绑定到 `127.0.0.1`——从网络
> 永远无法到达。它也**只存在于 IDE 中**：你
> 构建并交付的应用程序以及 `rcrun` 根本不含检查端点。

### 与编码智能体（Claude Code）协作

诸如 **Claude Code** 的编码智能体可以编写你的窗体、程序和
索引文件——但它自己并不了解 RustCOBOL。它看不到
编译器，对控件和属性只能猜测，也不知道生成的
COBOL 是永远不被编辑的。PowerRustCOBOL AI 给了它所缺少的东西：规则、
与产品*这个*版本相匹配的参考、一种对照真正的编译器检查它的工作的
办法，以及一种报告产品目前还做不到什么的
固定方式。

你**对每个项目一次性**设置好：不会向你的
项目文件夹写入任何东西，而 IDE 更新会一次性到达每个项目。

```mermaid
flowchart LR
    CC["Claude Code<br/>（你的安装）"] -- "PowerRustCOBOL 插件：<br/>技能、评审员" --> P[(插件)]
    CC -- "HTTP 127.0.0.1，访问令牌" --> IDE["PowerRustCOBOL AI<br/>工具服务器"]
    CC -- "stdio，当 IDE 关闭时" --> R["rcrun mcp"]
    IDE --> PRJ["在 IDE 中<br/>打开的项目"]
    R --> PRJ2["位于 Claude Code<br/>文件夹中的项目"]
```

#### 第一次连接

当 PowerRustCOBOL AI 启动而没有连接任何编码智能体时，它会在一个小窗口中——每次
启动一次——询问：**Connect a coding agent**。选择智能体（目前
只有 Claude Code）并选择：

| 按钮 | 它做什么 |
| --- | --- |
| **Connect** | 运行 **Configure Claude Code**（见下），并在 Coding Agent Settings 中显示它的进度。在智能体未安装时禁用——窗口会这样说明，并说明以后在哪里连接。 |
| **Skip for now** | 关闭窗口；下次 IDE 启动时它会再次询问。 |
| **Don't show again** | 对该智能体不再询问。你仍然可以从 **Settings ▸ Configure Claude Code** 或 **Settings ▸ Coding Agent Settings** 连接。 |

> 📷 **需要截图 — `connect-coding-agent.png`。** 启动时的 Connect a coding
> agent 窗口：吉祥物和编码智能体各自在它们的笔记本电脑前，
> 它们之间的连接在左边，问题以及选中的 **Claude Code**
> 在右边，三个按钮在右下角。

#### Configure Claude Code

**Settings ▸ Configure Claude Code** 在你机器上已有的
Claude Code 中，为你的用户安装 **PowerRustCOBOL 插件**。它不需要打开
项目。该插件携带：

- 七个任务技能——创建窗体、添加控件并绑定它的事件、定义
  索引文件、添加资源、编写通用过程、检查并修复、编写
  缺口报告；
- 三个用于构建应用程序的技能——它的结构和构建顺序；应用程序
  外壳以及在窗体之间移动；布局和
  主题——它们让智能体先用 `render_form` 查看每个屏幕，并用
  `run_form` 试运行它，然后再继续；
- 智能体在宣布更改完成之前运行的评审员；
- 与 PowerRustCOBOL **工具服务器**的连接（见下）。

IDE 用 Claude Code 自己的 `claude plugin` 命令安装它——它从不
编辑 Claude Code 的文件——并在 Output 面板中说明结果。如果没有
安装 Claude Code，它会说明它在哪里找过。用同一个
IDE 版本再次配置不会改变任何东西；在你更新 PowerRustCOBOL AI 之后，请再次
配置（或在 Coding Agent Settings 中使用 **Update**），每个项目都会得到
新版本。请启动新的 Claude Code 会话来加载它。

**不会放进任何机密或个人信息。** 该插件不携带 API 密钥——如果
这台机器上存储的密钥最终会进入其中，Configure 会拒绝并且什么也不
安装——也不携带主文件夹：当 `rcrun` 位于
你的主文件夹中时，它被命名为 `${HOME}/…`。

#### 工具服务器

智能体通过 MCP 提供的工具工作，由两个服务器之一提供：

| 服务器 | 智能体何时使用它 |
| --- | --- |
| `powerrustcobol-ide` | 当 PowerRustCOBOL AI 正在运行时。它为在 IDE 中打开的项目提供服务，通过 `127.0.0.1` 上的 HTTP（默认端口 **5720**），并且只接受携带 IDE **访问令牌**的请求。 |
| `powerrustcobol` | 当 IDE 关闭时。Claude Code 自己启动 `rcrun mcp`，所以 `rcrun` 必须在 IDE 旁边或在你的 `PATH` 上。它为启动 Claude Code 所在的文件夹中的项目——或其上层文件夹中的项目——提供服务，所以在 `forms/` 中启动的会话也能工作。 |

两个服务器在智能体一连接上时，就会告诉它 PowerRustCOBOL
应用程序是如何构成的、每类问题该去哪里找，以及
**常设规则**；并且它们提供**参考**——本指南、受支持语法文档、每个
控件及其属性、方法和事件、每个内置调用，以及
`.cfrm` 和 `.cidx` 格式——从运行中的版本实时读取。不在
那里的名称对智能体来说就是不存在的。

**黄金规则。** 智能体构建的每个应用程序都遵循同样的
规则，服务器在连接时把它们交给智能体，它的评审员会检查：

1. **响应式**——每个窗体都适应任何屏幕分辨率和外形尺寸。
2. **Spatial**——IDE 和窗体都使用 Spatial 主题。
3. **在每个主题中都是高对比度的文本**——每个显示文本的控件
   都带有显式的墨色，与它所处的表面形成强烈对比
   （至少 WCAG AA），并在带有真实数据的屏幕上检查。
4. **侧边菜单和嵌入的窗体**——主窗体上有 SideMenu，每个屏幕
   都加载进它的 ContentPane，除非你要求别的。
5. **精致而精简**——每个屏幕的控件很少，间距均匀，一种强调
   色，没有任何仅仅为了装饰而存在的东西。
6. **优先使用内置控件**——DataGrid、DateTimePicker、Snackbar、各种图表、
   IndexedFile、RestClient 等等，而不是手工构建的等价物。
7. **六种语言**——操作员看到的每一段文本都有英语、葡萄牙语、
   西班牙语、法语、日语和中文，在运行时切换。
8. **如果你想要，就用 GitHub**——智能体会提出把项目保存在
   私有的 GitHub 仓库中，并帮你设置；应用程序即使没有它
   也同样能工作。
9. **只在被要求时才用 PowerChat**——智能体在侧边菜单中为
   Assistant 条目留出位置，只有当你要求时才添加 PowerChat（`add_powerchat`，
   见下）。
10. **地道的 RustCOBOL**——智能体使用这门语言的扩展来书写：像数据项那样
    使用的属性、在表达式内部使用的方法的答案、
    数据项的值方法、`MOVE`、`SET`
    和 `STRING` 中的表达式，以及内联书写的内置调用——见 §13 中的*代码风格*。
11. **每个 `CALL` 之前的注释**——过程将做什么、它使用哪些数据
    项以及它们在返回时保存什么，这样不必打开过程
    就能读懂一次调用——见 §13 中的*每个 `CALL` 之前的注释*。

它们还提供一个**范式包**：来自 PowerDemo3 和
PowerChat 示例的可工作的窗体，每个都带有关于它展示什么的说明——带有
侧边菜单的应用程序外壳、加载进 ContentPane 的窗体、在
索引文件中维护记录、绑定到索引文件的 DataGrid、REST 调用、通过
AgentObject 向模型提问，以及向它的调用者回答的小型模态对话框。
智能体在写它自己的之前会先读最接近的那一个。

智能体拥有的工具如下：

- **`list_files`**——项目跟踪的文件，以及缺口报告。
- **`check`**——IDE 自己的 Check：每个窗体的代码、每个源文件、每个
  `.cfrm` 和 `.cidx`、主窗体的指定以及数据绑定问题。窗体
  错误会指出窗体、控件 ▸ 事件，以及该处理程序中的行。
- **`regenerate`**——用 IDE 的生成器重写窗体（或索引文件）生成的
  COBOL。
- **`add_to_project`**——把新的窗体、索引文件、源文件或资源放进
  项目。
- **`build`**——重新生成，在 `check` 报告错误时拒绝，然后构建
  二进制。长时间的构建会回答“running”；智能体再问一次。
- **`validate`**——检查某个 `.cfrm` 或 `.cidx` 能否加载。
- **`kb_lookup`**——按名称查找控件、属性、方法、事件或内置调用。
- **`kb_search`**——在智能体不知道确切名称时，用普通的词搜索
  IDE 的助手所使用的同一个知识库。
- **`render_form`**——窗体在 Run Form 打开它时绘制的样子的图片，
  这样智能体就能看到它构建的东西。你的屏幕上不会显示任何东西。对于
  你的应用程序加载进它的 ContentPane 的窗体，智能体可以要求它
  **在外壳之内**——在侧边菜单旁边、面包屑之下。要做到
  这一点，应用程序要运行，所以主窗体和该窗体的打开
  处理程序也会运行。给定 **`width` 和 `height`** 时，响应式窗体
  会按那个大小的窗口来布局——智能体既能在小笔记本上
  也能在宽显示器上看到它。窗体的布局从不小于它的最小值，
  因为运行中的窗口不能被缩得更小；答案会说明它何时被卡在
  那里。处于透明主题（Spatial）中的窗体，会按它的窗口
  所显示的样子——主题的玻璃——绘制在暗色的桌面替身之上，所以它的
  白色文字读起来就像在屏幕上那样。
- **`run_form`**——通过脚本，用窗体真正的程序在屏幕外运行窗体：
  在控件中键入、点击、等待、读回属性——并且，在带有侧边菜单的
  应用程序中，先把窗体加载进 ContentPane。智能体
  得到程序 DISPLAY 的内容、它读取的值、任何运行时错误，以及
  窗体最终状态的图片。永不返回的处理程序会在时间限制处被停止
  （20 秒，除非智能体要求更少或更多，最多
  120 秒）。程序真正的效果会发生——它写入的文件、它调用的
  Web 服务——与你自己运行它时完全一样。
- **`add_powerchat`**——在你要求时，一步把 **PowerChat**——带有主题、
  文档、提示和它自己设置的完整 AI 聊天——添加到你的应用程序中。
  它进入它自己的文件夹（`forms/powerchat/`、
  `Assets/powerchat/`、`data/powerchat/`），采用你的应用程序的主题、
  名称和图标，而不是它自己的，并且会在
  你的主窗体的侧边菜单中添加一个 **Assistant** 项：它在它自己的窗口中
  打开 PowerChat，位于你正在做的事情旁边，PowerChat 自己的侧边菜单
  在那里工作。如果 PowerChat 已经在
  项目中，或者你的某个窗体使用了它的某个名称，它会拒绝并且什么也不更改。
- **`add_crate`**——为你的 `EXEC RUST`
  块注册第三方库，与 **项目 Crate（测试版）** 对话框所做的一样（见本指南前面的*项目 Crate（测试版）*）。它取
  最新的稳定版本——或匹配智能体所指名版本的最新
  版本——把它的源码下载到
  `crates/`，检查它不与应用程序已经
  链接的内容或你的其他 crate 冲突，并把它连同来源的
  注册表记录在项目文件中。什么也不构建。对于
  已经注册的 crate、注册表不认识的 crate、
  应用程序直接链接的 crate（块已经可以使用它）或冲突，它会拒绝并且什么也不更改；
  与应用程序直接链接的某个 crate 同名、但版本
  无法共享的 crate，可以注册在块改用 `use` 的**别名**之下。
  应用程序仅仅作为它所链接的某个 crate 的依赖项而携带的 crate——
  `tokio` 和 `serde_json` 就是两个——像其他任何 crate 一样被添加，
  这就是块得以使用它的方式。注册表是 crates.io——或者，当由 IDE 提供
  工具时，是 IDE 中设置的那个。它需要网络，耗时从几秒到一分钟不等。
- **`remove_crate`**——注销一个 crate：删除它的记录和它
  下载的源码，绝不会删除你的 COBOL。仍然指名它的块会无法通过 Check。
- **`create_project`**——在空文件夹中创建新项目，与
  **File ▸ New Project** 所做的一样，并打开它。智能体创建的项目起始就是
  **Spatial**，既用于 IDE，也用作它的窗体的默认主题。
- **`open_project`**——打开现有项目，与 **File ▸ Open Project**
  所做的一样。

每个答案都会指名它所作用的项目，而针对不同于当前打开项目的
调用会被拒绝，并列出两个名称。IDE 有未保存更改时，`create_project` 和
`open_project` 会被拒绝——它们会指名每个
未保存的文件，由你来决定。当 IDE 关闭时，智能体通过
`rcrun` 创建或打开项目，`rcrun` 会带着该
项目打开启动 PowerRustCOBOL AI，并告诉智能体此后使用 IDE 的服务器。

智能体从不手工编辑项目文件。它携带着对
主窗体指定的封印，而手工编辑会使你的应用程序报告自己
已损坏——所以新文件通过 `add_to_project`，它像 IDE 那样加封印。IDE 打开期间，
它仍然是项目文件的唯一写入者：会覆盖 IDE 中
带有未保存编辑的窗体的工具会拒绝并
指名该窗体，而智能体在磁盘上更改过的窗体，如果你在其中
没有未保存的编辑，就会被重新加载。

**第一次**智能体使用每个工具时，Claude Code 会请你允许它。
对 PowerRustCOBOL 的工具请回答 *Yes, and don't ask again*：Claude Code
没有给插件任何办法来替你批准它自己的工具。

#### Coding Agent Settings

**Settings ▸ Coding Agent Settings** 针对整个 IDE 显示：

| 行 | 它说明什么 |
| --- | --- |
| **Claude Code** | 它在哪里被找到，或者没有找到。 |
| **PowerRustCOBOL plugin** | 安装在 Claude Code 中的版本与这个 IDE 的版本对比：最新、较旧（按钮显示 **Update**），或未安装。 |
| **Tool server** | 它监听的地址，`http://127.0.0.1:<port>/mcp`。 |
| **Port** | 工具的端口。**Apply port** 立即移动服务器；请再次配置 Claude Code，让它也使用新端口。 |
| **Access token** | 是否已设置。它从不显示。**Renew token** 替换它——此后使用旧令牌的请求会被拒绝——所以请再次配置 Claude Code。 |

**Configure** / **Update** 所做的与 **Settings ▸ Configure Claude Code** 相同。

> ⚠️ **注意事项——令牌就是锁。** 工具服务器只监听
> `127.0.0.1`，所以网络上没有任何东西能到达它，并且它只接受
> 携带访问令牌的请求。令牌由你第一次
> Configure 时生成，保存在 IDE 自己的设置中（只有你的用户可读）和
> Claude Code 的安全存储中——绝不在项目、日志或 Output 面板中。
> 如果你认为它已经泄露，请 **Renew token**。

> ⚠️ **注意事项——智能体在你允许它的地方工作。** 规则和评审员
> 禁止智能体触碰项目之外的任何东西，包括 PowerRustCOBOL
> 本身，但 Claude Code 仍然可能请求这样做；请拒绝。

#### 当产品做不到时：缺口报告

当你要求 PowerRustCOBOL 不支持的东西——动词、
属性、事件、IDE 功能——时，智能体不会凭空发明它。它
做可行的部分，在 `docs/compiler-requests/` 中写一份**缺口报告**
（`<date>-<topic>.md`），并告诉你。报告会指明请求、缺失的
功能、为什么需要它、一个最小示例、所用的变通办法、
版本，以及它是标准 COBOL-85 的行为（一个修复）还是超出它的
内容（一个功能）——可以直接发送给 PowerRustCOBOL 团队。

智能体一写下报告，它就会出现在项目树的 **Compiler requests** 之下，
最新的排在最前。点击报告即可打开它。没有报告时，该节点
不存在。

> 📷 **需要截图 — `claude-code-settings.png`。** 成功 Configure 之后的 Help ▸ Coding Agent
> Settings：找到了 Claude Code，插件是
> 最新的，工具服务器在监听，访问令牌已设置；下方的 Output
> 面板显示“Claude Code is configured”这一行。

### 让模型查询你的数据（MCP）

上面的检查端点让智能体驱动 *IDE*。这是另一个
方向，并且它属于**你的应用程序**：让模型根据你自己的
索引文件回答问题。

它所解决的问题，是每个“针对业务数据的 AI 助手”都会
遇到的。用户问*“第三季度有多少承包商入职”*，而没有指名任何文件。
模型必须选择一个——并且它只能根据每个文件对自己的描述来选择。

**所以你的描述就是机制。**你写在索引
文件上的注释会成为搜索工具的描述，而每个字段的注释
描述该工具的一个参数。不涉及别的任何东西。被描述为
*“每条雇佣记录一行”*、带有被描述为
*“开始日期”*的字段的文件，可以被正确选中；被描述为*“由
`load-staff.cbl` 加载”*的则不能，不管这条备注对同事来说有多准确。

> 💡 **请为将要使用它的读者写注释。** 说明
> 文件来自哪里的备注帮助下一位开发者。说明
> 文件*保存什么*的一句话帮助模型选出它。如果两者你都想要，请先
> 写它保存什么。

#### 选择哪些文件可以被查询

描述一个文件**不会**发布它。在你的
应用程序用 `AllowFile` 或 `RegisterFile` 标记它之前，没有任何东西可以被查询——下一节*让智能体使用工具*会展示它们——
而且什么也不标记的应用程序什么也不回答：

```
    ACTORS-FILE: no consultable file is served by that tool
```

这是有意为之的默认值。一个半成品的应用程序不应当悄悄地
暴露你碰巧拥有的每个索引文件。

#### 从你自己的 COBOL 提问

你的程序提出的问题与外部客户端所提的相同，不需要端口或
往返：

```cobol
       01 WS-TOOL   PIC X(40) VALUE "search_actors_file".
       01 WS-ARGS   PIC X(80) VALUE '{"ACTOR-SALARY": "100000"}'.
       01 WS-RESULT PIC X(500).
       ...
           COBOL::"MCP-SEARCH" ( WS-TOOL WS-ARGS WS-RESULT ).
           DISPLAY WS-RESULT.
```

它回答响应的文件以及它匹配的记录：

```
    ACTORS-FILE: 2 record(s)
    ACTOR-ID=1  ACTOR-SALARY=100000
    ACTOR-ID=3  ACTOR-SALARY=100000
```

`WS-ARGS` 是从字段名到值的 JSON 对象——与外部
客户端发送的形状相同，所以两个调用者接受相同的输入，到达相同的代码。
匹配是按文本的，不区分大小写，而你省略的字段只是
不被过滤。

**没有匹配是一个答案，而不是失败。**什么也没匹配的搜索会明白地
这样说，这与无法运行的工具不同。无法
区分这两者的调用者会把同一个问题问两遍。

**结果是有界的。**传入 `"limit"` 来限制它们；回复会说明它何时
被截断以及还有更多。

#### 你的数据只会被读取，绝不被触碰

工具所读取的索引文件是**你的、已经存在的**。模型和
工具都不得更改其中任何一个——不能更改一条记录、一个键，也不能更改一个字节。这不是
工具检查而可能出错的规则：文件以 `INPUT` 打开，只发出
顺序读取，并且在那条路径上的任何地方都不存在写入、重写、删除或提交。
没有可以获得的可写句柄。

#### 内存，以及实际上由哪个引擎打开你的文件

已经存在的文件已经选定了它的容器，而
容器决定它如何被读取，而不是搜索：

| 文件的容器 | 如何读取 | 内存开销 |
|---|---|---|
| `PRCIDXD1`：自 1.80.37 起由任何程序写出，DISK 或 MEMORY | 按需读取记录 | 有界，任何大小 |
| redb | 按需读取记录 | 有界，任何大小 |
| `PRCIDX1`：由 1.80.37 之前的 MEMORY 程序写出 | 整体加载 | 随文件增长 |

所以项目带有一个**内存限制**——*Project Settings ▸ Runtime ▸ Model
file-search memory*，在项目文件中保存为 `[agents] file_memory_limit_mb`，
除非你更改，否则是 64 MB——并且它只适用于最后一行。
请求搜索大于该限制的这种文件时，工具
会拒绝，并指出两个数字：

```
    ACTORS-FILE is held in an in-memory container of 412000000 bytes, over
    this project's 67108864-byte limit, so it was not loaded. Raise the
    limit, or rebuild the file with STORAGE IS DISK so it can be read
    without loading it whole.
```

这有意是拒绝而不是尝试。因内存耗尽而被杀死的进程
什么也没告诉你；而这会告诉你两个数字和
两条出路。

> 💡 **经验法则。** 旧的 `PRCIDX1` 文件在程序第一次
> 为写入而打开它时就不再是个问题：它会以当前的
> 格式被重写，从此之后原地读取。

#### 对交付的 `.cidx` 信任什么

你的应用程序从随它一起交付的 `.cidx` 文件读取它的描述，
这使你可以**在现场纠正误导性的描述
而无需重新构建**——编辑该文件，重启，工具就会以不同的方式
描述它自己。

那个文件也可以被运行你的应用程序的任何人编辑，所以只有
描述性的那一半是被相信的。记录布局、字段偏移、键和存储
模式来自你编译好的程序，绝不会从交付物中读取。这个
后果值得了解：

> ⚠️ 编辑交付的 `.cidx` 的人可以让文件**错误地描述
> 它自己**——这是可见的，并且通过恢复文件来修复。他们无法移动字段、
> 更改键，或让记录被读作它不是的东西。

不描述任何字段的定义根本不会作为工具提供：
没有可搜索的东西，也没有可以告诉模型的东西。

> ⚠️ **没有认证。** 任何能到达服务器的东西都可以调用
> 你标记的工具。请把你标记的内容以及你在哪里提供它，视为
> 整个安全边界。

### 让智能体使用工具（`onToolCall`、`AllowFile`）

`AgentObject` 能做的不止是凭它已经知道的东西作答。给它
**工具**，一次 `Ask` 就变成一段简短的对话：模型请求一个工具，
工具运行，结果送回去，模型带着这个结果在手作答。你的 `onResponse`
处理程序仍然只收到一个完成的答案——中间的各轮
都替你完成了。

如果你写过 PowerCOBOL 或 isCOBOL 的事件处理程序，这个形态你很熟悉：
模型引发一个事件，你的处理程序填上答案，控制权
再交还给它。

```mermaid
sequenceDiagram
    participant P as 你的程序
    participant A as AgentObject
    participant M as 模型
    P->>A: Ask(question)
    A->>M: 问题 + 工具列表
    M-->>A: 调用 search_actors_file
    Note over A: 索引文件工具：<br/>在进程内搜索
    A->>M: 搜索结果
    M-->>A: 调用 get_rate(CURRENCY)
    A->>P: onToolCall (ToolName, ToolArguments)
    P->>A: SetToolResult(ToolCallId, text)
    A->>M: 你的结果
    M-->>A: 最终答案
    A->>P: onResponse (LastReply)
```

工具来自两个地方之一。

**你允许的索引文件。** `AllowFile` 把你的某个索引文件提供给
应用程序中的每个智能体，作为同样的只读 `search_<file>` 工具，
即上文*让模型查询你的数据（MCP）*中所描述的：

```cobol
           MOVE AGENT-1::AllowFile("ACTORS-FILE", "indexed/actors.cidx")
             TO WS-OK
```

第一个参数是该文件在你的 `SELECT` 中的名称；第二个是它的
`.cidx` 定义（省略它，就会在交付的 `indexed/` 文件夹中搜索
该文件的定义）。文件的*含义*——它的描述和
字段注释——来自 `.cidx`。它的记录是*如何布局的*，
来自你程序自己的 `FD`，绝不来自定义。搜索在
你的程序内部运行，从不到达处理程序。`DenyFile("ACTORS-FILE")`
撤回它。当找不到 `FD` 或定义时，`AllowFile` 回答 `0`，
该文件就干脆不被提供。

**你按路径注册的索引文件。** `AllowFile` 需要你的程序中有该文件的 `FD`，
所以它只能提供你在构建时就知道的文件。`RegisterFile`
提供的是你的*用户*在应用程序运行时指向的文件——
财务服务器上的订单文件、同事的客户列表——不需要
`FD`，也不需要重新构建：

```cobol
           MOVE AGENT-1::RegisterFile(
                "smb://finance/data/orders.dat",
                "smb://finance/data/orders.cidx") TO WS-OK
           IF WS-OK = "1"
               MOVE AGENT-1::RegisterResult TO WS-HOW
      *>       "MEMORY" or "DISK"
           ELSE
               MOVE AGENT-1::RegisterResult  TO WS-CODE
               MOVE AGENT-1::RegisterMessage TO WS-WHY
               PERFORM TELL-THE-USER
           END-IF
```

模型随后看到 `search_orders_file`——以 `.cidx` 命名，或者
在你给出第三个参数时以它命名——并且像搜索你允许的
文件那样搜索它。`UnregisterFile("ORDERS-FILE")` 撤回它。

*文件可以在哪里。*数据文件和它的 `.cidx` 都可以是下列任何一种：

| 路径 | 示例 |
|---|---|
| 本地路径 | `C:\Data\orders.dat`、`/home/ana/orders.dat`，或相对于你的应用程序 |
| 系统自己的网络路径 | Windows 上的 `\\finance\data\orders.dat`；macOS 上挂载的共享（`/Volumes/data/orders.dat`）或 Linux 上的（`/mnt/data/orders.dat`） |
| `smb://` 地址，适用于每个系统 | `smb://finance/data/orders.dat`，不挂载共享就能读取 |

没有用户名的 `smb://` 地址以访客身份登录。对于需要
登录的共享，把登录信息写进地址——`smb://CORP;ana:password@finance/data/orders.dat`。
密码只用于登录，别无他用：它绝不会出现在
`RegisterMessage`、日志，或发送给模型的任何内容中，在那些地方
地址显示为 `ana:****@`。

> ⚠️ **目前，`smb://` 地址中的密码保存在你的程序中。**请让它
> 不出现在源码里——在运行时由用户键入的内容构建地址，
> 或者使用允许访客的共享。

*`.cidx` 必须说明什么。*没有 `FD` 时，记录布局来自
`.cidx`——所以在模型看到任何东西之前，定义会与
数据文件保存的关于它自身的模式进行核对。记录长度或键
不匹配、定义没有说明文件是做什么用的，或者
字段没有描述，这个文件就会被拒绝，并附上原因。

*始终只读。*已注册的文件以 `INPUT` 打开，从不向它写入任何东西
——不写一条记录，不写一个头字节。它不需要写入
权限，所以只读文件或只读共享也没问题。如果被
中断的写入在文件旁边留下了恢复日志（`orders.dat.jrn`），
读取它就意味着要先修复它，而这不是助手的职责——
该文件会被拒绝，它和日志都保持原样。

*内存，还是磁盘。*已注册的文件在低于项目的内存限制，**并且**
不超过那一刻机器空闲内存的一半时，保存在内存中。更大的文件，
如果是本地（或操作系统网络路径）的 `STORAGE IS DISK` 文件，就在磁盘上
原地一页一页地读取。其他任何过大的文件——旧的 `PRCIDX1` 文件
（由 1.80.37 之前的 MEMORY 程序写出），
或 `smb://` 共享上的任何文件——都会被拒绝，而 `RegisterFileBytes` / `RegisterLimitBytes` 会给出这两个数字。

| `RegisterResult` | 含义 |
|---|---|
| `MEMORY` / `DISK` | 已注册——保存在内存中，或在磁盘上原地读取 |
| `NOT-FOUND`、`CIDX-NOT-FOUND` | 数据文件或它的 `.cidx` 不在那里 |
| `ACCESS-DENIED` | 没有权限，或共享拒绝了登录 |
| `UNREACHABLE` | 服务器没有应答 |
| `BAD-PATH` | 路径无法理解（例如 macOS 或 Linux 上的 Windows `\\server` 路径——请使用挂载路径或 `smb://`） |
| `RECORD-LENGTH-MISMATCH`、`KEY-MISMATCH` | `.cidx` 没有描述这个文件 |
| `NO-PURPOSE`、`NO-FIELDS`、`NO-FIELD-DESCRIPTIONS` | `.cidx` 说得不够，模型无法使用这个文件 |
| `CIDX-INVALID`、`CORRUPT` | `.cidx` 或数据文件已损坏 |
| `NOT-INDEXED`、`FORMAT-UNSUPPORTED` | 不是 PowerRustCOBOL 的索引文件，或是没有可供核对的模式的旧格式 |
| `JOURNAL-PRESENT` | 文件旁边有一份恢复日志 |
| `TOO-LARGE-FOR-LIMIT`、`TOO-LARGE-FOR-FREE-MEMORY` | 过大，无法保存在内存中，也无法原地读取 |
| `NEEDS-UPGRADE` | 过大，无法保存在内存中，而且处于较旧的磁盘格式，不更改它就无法原地读取 |
| `SMB-UNAVAILABLE` | 应用程序构建时没有 `smb://` 支持 |

`RegisterMessage` 用英语说的是同一件事；请在你自己的表中为
你的用户翻译这个代码。

> **说明。**
> - 注册持续到你的程序结束。关于它的任何内容——尤其是
>   带有密码的路径——都不会被保存。请在启动时
>   再次注册这些文件。
> - “空闲内存”是操作系统自己的估计，而每个系统
>   对可回收缓存的计算方式不同。请把它当作一道防护，而不是一个承诺。
> - `smb://` 通过 SMB 2 和 3 工作。不支持 SMB 1。

📷 需要截图 — `registerfile-project-settings.png`：*Project Settings ▸
Runtime*，其中可见 **Model file-search memory** 字段（设为 64 MB）。

**由你的程序回答的工具。**声明它，描述它的参数，并在
`onToolCall` 中回答它：

```cobol
       FORM-1--ONLOAD.
           MOVE AGENT-1::AddTool("get_rate",
                "Today's exchange rate from euros to a currency") TO WS-OK
           MOVE AGENT-1::AddToolParameter("get_rate", "CURRENCY",
                "Three-letter currency code, e.g. USD") TO WS-OK.

       AGENT-1--ONTOOLCALL.
           MOVE AGENT-1::ToolCallId    TO WS-CALL-ID
           MOVE AGENT-1::ToolArguments TO WS-ARGS
      *>   WS-ARGS holds {"CURRENCY":"USD"}
           PERFORM LOOK-UP-RATE
           MOVE AGENT-1::SetToolResult(WS-CALL-ID, WS-RATE-TEXT) TO WS-OK.

       AGENT-1--ONRESPONSE.
           MOVE AGENT-1::LastReply TO ANSWER-BOX::Text.
```

当你声明了不止一个工具时，`ToolName` 说明调用的是哪个工具。
`ToolArguments` 是一个 JSON 对象，其键是你声明的
参数名，其中的每个值都是字符串。

| 成员 | 它做什么 |
|---|---|
| `AddTool(name, description)` | 提供一个由你的程序回答的工具。描述是模型在决定是否调用该工具时所读的内容，所以请说明这个工具*返回*什么。 |
| `AddToolParameter(tool, name, description)` | 描述一个参数。 |
| `RemoveTool(name)` | 停止提供某个工具。 |
| `SetToolResult(call-id, text)` | 答案，在你的处理程序返回时发送。 |
| `AllowFile(fd-name [, cidx-path])` / `DenyFile(fd-name)` | 为每个智能体提供或撤回一个索引文件。 |
| `RegisterFile(data-path, cidx-path [, name])` / `UnregisterFile(name)` | 按路径提供或撤回一个索引文件——本地、网络或 `smb://`——不需要 `FD`。 |
| `ToolProtocol` | `Native`（默认）、`Fenced` 或 `None`——见下文。 |
| `MaximumToolRounds` | 一次 `Ask` 可以进行多少轮工具调用（默认 8）。 |
| `MaximumContinuations` | 被 `MaximumTokens` 截断的答案会被续写多少次（默认 4；见“长的答案绝不会悄无声息地被截短”）。 |
| `LastInputTokens`、`LastOutputTokens` | 提供商为上一次 `Ask` 报告的令牌数，对每一轮求和。 |
| `LastToolCallCount` | 模型在上一次 `Ask` 期间调用了多少个工具。 |
| `ToolCallId`、`ToolName`、`ToolArguments` | `onToolCall` 期间只读：要传给 `SetToolResult` 的调用 id、工具的名称，以及作为 JSON 对象的它的参数。`ToolName` 和 `ToolArguments` 在 `onToolUse` 期间也会被设置。 |
| `ToolKind` | `onToolUse` 期间只读：`KnowledgeBase` 或 `IndexedFile`。 |
| `RegisterResult`、`RegisterMessage` | `RegisterFile` 之后只读：结果代码（上表）和用英语表达的同一结果。 |
| `RegisterFileBytes`、`RegisterLimitBytes` | `RegisterFile` 之后只读：它所检查文件的大小，以及与它比较的限制，以字节为单位。 |

**`Native` 还是 `Fenced`。**使用 `Native` 时，工具采用提供商自己的
工具调用格式——兼容 OpenAI 的服务器、Ollama 和 Anthropic 各有
一种，控件三种都会说。有些本地模型根本没有工具
调用。对于它们，请把 `ToolProtocol` 设为 `Fenced`：工具在
系统提示中描述，而模型通过用一小段带围栏的
JSON 块作答来调用其中一个。它更慢，并且依赖模型遵循指令，但它
适用于任何能遵循指令的模型。控件从不根据名称去猜
某个模型需要哪一种——由你选择。

**`None`。**你允许的文件、知识库和工具会提供给程序中的
**每个**智能体。在其模型根本无法调用工具的智能体上把
`ToolProtocol` 设为 `None`，那个智能体就不会被提供其中任何一个——
它的请求是普通聊天，不管其他智能体怎样。PowerChat 的选举把它
设在除了负责工具工作的那个之外的每个智能体上。

⚠️ **Check 会对其智能体无法调用工具的窗体发出警告。**如果一个窗体上有两个或更多
`AgentObject` 被设为 `ToolProtocol = None`，**Check** 会在
Output 面板中指出，并点出窗体和智能体：这样的应用程序仍然可以运行
——它不用工具调用来作答——但你是在设计时听到，而不是
从用户那里。一个智能体设为 `None` 是普通聊天，不会提示任何东西。该警告
读取你所做的设置；它从不根据模型的名称去猜。（PowerChat 在运行时
根据模型列表设置 `None`，所以它没有什么需要警告的。）

> **说明。**
> - 不提供工具的智能体，发送的请求与它一直发送的完全一样。
>   在你声明一个之前，工具不花任何代价。
> - 你的程序**一次回答一个调用**，按模型发出它们的
>   顺序。如果模型一次请求三样东西，`onToolCall` 会触发
>   三次。
> - 从不调用 `SetToolResult` 的处理程序会给模型发送一个**空**的
>   结果。未绑定的 `onToolCall` 也一样，所以问题绝不会挂起。
> - 对从未被提供的工具的调用，或不是有效 JSON 的参数，会
>   作为错误结果**报告给模型**，让它可以重试。
>   你的处理程序从不会看到它们。
> - `LastInputTokens` / `LastOutputTokens` 在每次 `Ask` 时都会写入，无论
>   有没有工具。它们是你观察一个问题花费多少的方式。

> ⚠️ **注意事项。**
> - 对于提供了工具的 `Ask`，`TimeoutSeconds` 限制的是**整个**
>   问题——每一轮，以及每次等待你的处理程序——并且答案的各个片段
>   不会让它重新计时，这不同于普通的 `Ask`，在那里它只
>   限制片段之间的静默。运行长时间批处理作业的工具应当把
>   工作交出去并立即回答。`StartTimeoutSeconds` 分别适用于每一轮：
>   每个对模型的请求都必须及时开始。
> - 不停调用工具的模型会在 `MaximumToolRounds` 处被停止，
>   触发 `onError`，而 `LastError` 会这样说明。只有对真正
>   需要许多步骤的问题才提高这个限制。
> - `Cancel()` 在任何时刻都会停止问题，包括在你自己的
>   `onToolCall` 处理程序之内。`onCancelled` 会触发，并且不会再向
>   模型发送任何东西。
> - 被允许的文件对应用程序中的**每个**智能体可见。只允许
>   那些其内容你的任何智能体都可以向它的用户复述的文件。

**了解智能体何时在搜索——`onToolUse`。**运行时自己回答的工具从不到达
`onToolCall`：知识库搜索、已注册的
索引文件。`onToolUse` 在它们运行之前，当模型仍在工作时，
告诉你它们正在被使用。`ToolKind` 是 `KnowledgeBase` 或
`IndexedFile`，而 `ToolName` 和 `ToolArguments` 说明是哪个工具以及用了什么参数。
没有什么需要回答：结果直接送给模型。聊天需要它，
以便在这一切发生的同时说出“正在搜索知识库……”。

```cobol
       AGENT-1--ONTOOLUSE.
           IF AGENT-1::ToolKind = "KnowledgeBase"
               INVOKE VWR-1::ReplaceMessage(WS-STATUS-ID,
                   "*Searching the documents...*", "Markdown")
           END-IF.
```

### 你的用户的文档：KnowledgeBase 控件

**KnowledgeBase** 为你的应用程序提供一个其用户文档的资料库，
程序——或模型——可以搜索它：公司政策、手册、会议
记录。它属于应用程序自己。它与 IDE 的助手在你设计时
使用的知识库无关，构建出的应用程序也不会为它
携带 IDE 的任何东西。

从工具箱（它的 **Non-Visual** 组）把 **KnowledgeBase** 拖到
窗体上。它保存**集合**：相互独立的文档集，每个都在自己的文件夹中，带有
由它们构建的可搜索索引。

```text
<app>/assets/KB/                  ← Location (the default)
    hr/                           ← a collection
        documents/                ← the users' files
        collection.kbindex        ← the index, built from them
    legal/
        documents/
        collection.kbindex
```

| 属性 | 它做什么 |
|---|---|
| `Location` | 保存集合的文件夹。相对路径从应用程序的文件夹开始；它可以是你网络中另一台机器上的文件夹。 |
| `Collection` | 文档和搜索方法所作用的集合。 |
| `Embedder` | `Lexical`、`Endpoint` 或 `Builtin`——见*文本如何匹配*。 |
| `EmbeddingAPI`、`EmbeddingURL`、`EmbeddingModel` | 对 `Endpoint`：服务器的通信格式（`Ollama`、`OpenAI`、`LMStudio` 或 `Custom`）、它的基地址，以及所请求的嵌入模型（默认 `nomic-embed-text`）。 |
| `EmbeddingAPIKey` | 对 `Endpoint`：服务器的密钥。从不保存在窗体中——它在运行时来自机器的密钥存储或环境。 |
| `Configuration`、`ModelEntry` | 对 `Endpoint`：不用三个 `Embedding…` 行，而是指名你的某个模型提供商（`Configuration`）或模型列表条目（`ModelEntry`，在运行时设置）——见*智能体使用哪些设置*。 |
| `MaximumResults` | 当 `Search` 没有指名最大值时返回的命中数（5）。 |
| `WriteWaitMilliseconds` | 写入要等待另一个写入同一集合的应用程序多久（5000）。 |
| `ArchiveMaximumMegabytes`、`ArchiveMaximumFiles`、`ArchiveMaximumDepth` | 对读取文档压缩包的限制（500 MB、10,000 个文件、3 层）——见*它读取哪些文档*。 |

控件还在运行时只读地报告：

| 属性 | 它保存什么 |
|---|---|
| `LastError` | 上一次操作为什么失败，配合 `onError` 或 `onBusy`；成功的操作之后为空。 |
| `ProgressDocument`、`ProgressCurrent`、`ProgressTotal` | 在 `onProgress` 中：刚处理的文档、已完成多少文档，以及这次更新涉及多少文档。 |
| `ProgressPassage`、`ProgressPassages` | 在 `onProgress` 中，当一个文档正在被嵌入时：它的多少段落已经完成（每批 16 个之后报告），以及它被拆成了多少段——否则为 0。嵌入占了一次更新的大部分时间，所以这是用来对着进度条的一对值。 |
| `AddedCount`、`UpdatedCount`、`RemovedCount`、`SkippedCount`、`SkippedDocuments` | 在 `onIndexed` 中：这次更新做了什么，以及每个被跳过的文档，格式为 `document: code (explanation)`——见*它读取哪些文档*。 |
| `PassageCount` | 在 `onIndexed` 中：这次更新存储了多少段落。 |
| `SearchMode`、`SearchModeReason` | `Semantic` 或 `Lexical`：上一次搜索是如何评分的，或上一次更新是如何存储其文档的；`SearchModeReason` 说明它为什么是 `Lexical`（词法嵌入器、无法到达的嵌入服务器、尚未获取的模型、用另一个嵌入器建立索引的集合）。 |
| `ResultCount` | 在 `onSearchComplete` 中：有多少命中，用 `GetResultDocument` … `GetResultScore(n)` 读取。 |
| `CollectionCount`、`DocumentCount` | 上一次 `ListCollections` 和 `ListDocuments` 发现了什么。 |

#### 一切都在后台进行

添加、更改、删除、刷新和搜索，都在窗体保持
响应的同时运行。方法立即回答——工作已开始时为 `1`，
不能开始时为 `0`（读取 `LastError`）——结果则作为事件到达。**你在
事件分支内读取的属性，就是该事件所携带的值**，所以两个进度报告
在你的处理程序看到它们之前不会互相覆盖：

```cobol
           MOVE KB-1::AddDocument("policies/leave.md", WS-TEXT) TO WS-OK
      *>   …then, in the event loop:
           EVALUATE COBOL-EVENT-ID
               WHEN "onProgress"
                   MOVE KB-1::ProgressCurrent TO WS-DONE
                   MOVE KB-1::ProgressTotal   TO WS-TOTAL
                   PERFORM SHOW-PROGRESS
               WHEN "onIndexed"
                   MOVE KB-1::Search("annual leave") TO WS-OK
               WHEN "onSearchComplete"
                   MOVE KB-1::ResultCount TO WS-COUNT
                   PERFORM VARYING WS-N FROM 1 BY 1
                           UNTIL WS-N > WS-COUNT
                       MOVE KB-1::GetResultDocument(WS-N) TO WS-DOC
                       MOVE KB-1::GetResultPassage(WS-N)  TO WS-PASSAGE
                       PERFORM SHOW-HIT
                   END-PERFORM
               WHEN "onBusy"
                   MOVE KB-1::LastError TO WS-MESSAGE
           END-EVALUATE
```

| 方法 | |
|---|---|
| `AddDocument(name, text)`、`UpdateDocument(name, text)` | 保存一个文档（允许子文件夹：`policies/leave.md`）并为它建立索引。 |
| `ImportDocument(path [, name])` | 复制一个文件进来并为它建立索引。 |
| `DeleteDocument(name)` | 删除一个文档及它在索引中的条目。 |
| `Refresh()` | 为文件夹中变化的内容建立索引——见下文。 |
| `Search(query [, max])` | 搜索；然后用 `GetResultDocument`、`GetResultHeading`、`GetResultPassage`、`GetResultScore(n)`。 |
| `CreateCollection(name)`、`RemoveCollection(name)`、`ListCollections()`、`GetCollection(n)` | 管理集合。 |
| `ListDocuments()`、`GetDocument(n)` | 集合的文档。 |
| `Reindex()`、`FetchModel()`、`Cancel()` | 重新嵌入全部；获取内置模型；停止正在运行的操作。 |

文档按它的标题被拆成段落，所以搜索返回的是
回答问题的那一节——`GetResultHeading` 把它命名为
`leave › Leave › Carry-over`——而较长的一节会整体返回。

#### 它读取哪些文档

你的用户不需要先转换任何东西：他们把已经保存的文件丢进来，
每个文件都在你的应用程序内部被转换成文本。

| 格式 | 文件 | 命中指向什么 |
|---|---|---|
| Word | `.docx`、`.docm`、`.dotx`、`.dotm` | 它所在的标题 |
| PowerPoint | `.pptx`、`.pptm`、`.potx`、`.potm`、`.ppsx`、`.ppsm` | `Slide 4: Pricing` |
| Excel | `.xlsx` 和旧的二进制 `.xls` | 工作表（作为表格读取） |
| OpenDocument | `.odt` 和 `.ods`（及其模板） | 标题，或工作表 |
| 表格 | `.csv`、`.tsv` | 表格 |
| PDF | `.pdf` | `Page 7` |
| 网页 | `.html`、`.htm` | 标题；脚本、样式和导航被排除在外 |
| 文本 | `.md`、`.txt` | Markdown 标题 |
| 压缩包 | `.zip`、`.tar`、`.tar.gz` / `.tgz` | 里面的每个文档，按它的路径 |

- **由内容决定，而不是名称。**被人改名为 `.txt` 的 Word 文件
  仍然被当作 Word 读取。
- **Word 标题是按大纲级别找到的**，而不是按样式名称，所以
  用葡萄牙语或西班牙语 Word 写成的文档——`Título 1`——和英语的一样
  按标题拆分。
- **压缩包内的文档通过它来命名**，在搜索命中和
  `SkippedDocuments` 中都一样：`contracts-2025.zip › legal/nda.docx`，而通过
  ZIP 中的 ZIP 则是 `old.zip › 2024.zip › legal/nda.docx`。
- **只为文本建立索引。**图片被忽略，并且文档中包含的任何东西
  ——宏、脚本、嵌入对象——都绝不会被运行。

无法读取的文档会被**跳过，留在文件夹中，并且绝不会阻止其他
文档**。`onIndexed` 在 `SkippedDocuments` 中把每一个列为
`document: code (explanation)`，以 `; ` 分隔。代码是稳定的，所以你的
程序可以用用户的语言显示原因：

| 代码 | 含义 | 用户可以做什么 |
|---|---|---|
| `legacy_office` | 旧的二进制 Word 或 PowerPoint 文件（`.doc`、`.ppt`） | 把它另存为 `.docx` / `.pptx` |
| `password_protected` | 文档已加密 | 另存一个未受保护的副本 |
| `no_text` | 其中没有文本的 PDF——通常是扫描件 | 先让它经过 OCR |
| `damaged` | 文件不完整或已损坏 | 替换它 |
| `unsupported` | 不是它读取的文档格式——一幅图片、一个程序 | — |
| `too_large` | 超出其某个限制的压缩包 | 把它解包，或提高限制 |
| `unreadable` | 文件无法打开 | 检查它的权限 |

**压缩包是有界的。**压缩包在内存中读取，从不解包到
磁盘，而三个 `ArchiveMaximum…` 属性限制它——它解包后的
总量、文件数，以及嵌套多少层压缩包。这些上限也会统计
嵌套的压缩包，所以被构造得会膨胀到极大的小文件会被拒绝，
记为 `too_large`，而不是填满机器的内存。

> ⚠️ **注意事项：**这些限制适用于压缩包。单个非常大的电子表格
> 或 PDF 是整体读取的，所以请把单个文档保持在合理的大小。

#### 保持索引最新

通过控件所做的更改会立即更新索引。你的用户用文件管理器复制到 `documents/` 中的文件——或者同事在
共享文件夹上添加的文件——由 `Refresh()` 拾取：它**按内容**把文件夹与
索引进行比较，只为添加、更改或移除的内容建立索引。当你的窗体打开一个集合时调用它。
没有任何东西在后台监视文件夹；
在网络共享上，这样的监视并不可靠。

#### 文本如何匹配

- **`Lexical`**——内置，始终可用。匹配词：“leave”能找到
  “leave”，找不到“vacation”。
- **`Endpoint`**——服务器上的嵌入模型（`EmbeddingAPI`、
  `EmbeddingURL`、`EmbeddingModel`，或指名你的某个
  模型提供商的 **Configuration**）。它的密钥来自机器的密钥存储，绝不来自
  窗体。
- **`Builtin`**——语义模型在你的应用程序内部运行，并能
  离线找到含义。只有项目提出要求时才会链接它：

  ```toml
  [rag]
  embedder = "builtin"
  ```

  构建这样的应用程序会**随附模型**（约 470 MB），放在目标文件夹中
  `assets/` 旁边的 `models/` 文件夹里，所以它从第一次运行起就能
  离线嵌入。构建从项目自己的
  `models/` 文件夹获取它，或者从 PowerRustCOBOL AI 已经为它的
  助手保存的副本获取。两者都不存在时，构建会这样说明，而应用程序
  会在每次安装时用 `FetchModel()` 获取它一次。应用程序
  在 `<app>/models` 中寻找它，然后在 `<app>/assets/models` 中，旧的
  应用程序曾把它保存在那里。该安装的每个用户共享它。Run Form
  使用 IDE 自己的副本，所以从 IDE 运行不会下载任何东西。

`SearchMode` 告诉你一次搜索是如何评分的。当所选的嵌入器无法
使用时——服务器宕机、模型尚未获取，或集合是用不同的嵌入器
建立索引的——搜索会回退到词法，并由 `SearchModeReason` 说明原因。
其间添加的文档存储时没有向量，仍然可以按词找到；下一次带着可用
嵌入器的 `Refresh()` 会给它们向量。更改集合的嵌入器需要一次
`Reindex()`。

**即时显示一次更新，并确认它。**嵌入是一次更新耗费时间之处，
所以 `onProgress` 一段一段地报告它。当一个
文档正在被嵌入时，`ProgressPassages` 保存它被拆成了多少段，
`ProgressPassage` 保存已经完成了多少，每次一批 16 个。
每个文档之后它们回到 0。更新结束时，
`onIndexed` **每次**都会设置 `PassageCount` 和 `SearchMode`：
`Semantic` 确认这些段落带有嵌入，而 `Lexical` 表示
它们是为按词搜索而存储的，由 `SearchModeReason` 说明原因。

```cobol
       PROGRAM-ID. KB-1--ONPROGRESS.
       PROCEDURE DIVISION.
           MOVE KB-1::ProgressPassages TO WS-PASSAGES
           IF WS-PASSAGES NOT = SPACES AND WS-PASSAGES NOT = "0"
               MOVE KB-1::ProgressPassages TO BAR-1::Maximum
               MOVE KB-1::ProgressPassage  TO BAR-1::Value
           END-IF
           .
       PROGRAM-ID. KB-1--ONINDEXED.
       PROCEDURE DIVISION.
           MOVE KB-1::PassageCount TO WS-PASSAGES
           MOVE KB-1::SearchMode   TO WS-MODE
           .
```

> 📷 需要截图 — `knowledgebase-properties.png`。在窗体上选中一个
> `Embedder = Endpoint` 的 KnowledgeBase，截取显示
> Location、Collection、Embedder、Configuration 和 EmbeddingModel 的属性窗格。

#### 共享一个知识库

多个应用程序——在一台机器上，或在网络共享上的用户——可以同时搜索
并写入同一个集合。写入轮流进行；等待超过
`WriteWaitMilliseconds` 的写入会触发 `onBusy` 并且什么也不写，所以你可以告诉
用户重试。并发使用已在**本地磁盘**上验证过
（三个进程搜索并写入同一个集合）。只有在网络共享上试过之后，
才把 `Location` 指向那里：共享依赖共享所提供的文件锁。

#### 让智能体阅读这些文档

```cobol
           MOVE AGENT-1::AllowKnowledgeBase("KB-1") TO WS-OK
           MOVE AGENT-1::Ask("How much annual leave do we get?") TO WS-OK
```

模型现在有了一个搜索该集合的工具（控件的
`Collection`，或你作为第二个参数指名的那个）。它决定何时
搜索，并且它收到的每个段落都指明它的文档和章节，所以它可以
引用它们。`DenyKnowledgeBase` 撤回它。与 `AllowFile` 不同，集合
只授予**你对其调用的那个智能体**，不授予其他智能体：有多个智能体时，
请授予每一个应当搜索的智能体，而对一个拒绝它，绝不会把它从
另一个那里拿走。

> ⚠️ **注意事项。**
> - 每个控件一次只能做一个操作：在一个操作运行时的第二次调用会回答
>   `0`。需要并行工作时，请使用第二个 KnowledgeBase 控件。
> - `RemoveCollection` 把文件夹移到一边（`hr.removed-<time>`），而不是
>   删除它——它的文档可能是你的用户拥有的唯一副本。
> - 重新构建你的应用程序绝不会覆盖你的用户所拥有的东西：交付的 `assets/KB`
>   中已经存在的集合会被保持不动，
>   而 `models` 只会补上缺失的文件。你在
>   `assets/KB/<name>/documents` 中交付的集合，只在它尚不存在的地方被复制。
> - 安装在只读文件夹中的应用程序无法把它的
>   知识库保存在 `assets/KB` 之下；请把 `Location` 设为可写的文件夹。

### 调用 AWS：AWS 控件

工具箱 **AWS** 组中的非可视控件（在窗体上它们位于
*Non-Visuals* 标签页，与任何其他非可视控件一样）从 COBOL 访问
Amazon Web Services：

| 控件 | 它做什么 |
|---|---|
| `AwsLambda` | 用 JSON 载荷运行你的某个 Lambda 函数 |
| `AwsKnowledgeBase` | 向 Amazon Bedrock 知识库提问，并返回匹配的段落 |
| `AwsAgentCore` | 与托管在 Amazon Bedrock AgentCore 中的智能体对话，一次一场对话 |
| `AwsAgentMemory` | 在 AgentCore Memory 中记录一场对话，并搜索它所学到的东西 |
| `AwsS3Tables` | 列出 Amazon S3 Tables，用 SQL 读取它们，追加行 |
| `AwsGlue` | 启动 Glue 作业和爬虫，跟踪一次运行，读取表的列 |
| `AwsDynamoDB` | 以纯 JSON 读写 DynamoDB 条目 |
| `AwsS3` | 列出、读取、写入和删除存储桶的对象，并列出你的存储桶 |
| `AwsS3Vectors` | 找出与某个向量最近的向量，并存储向量 |
| `AwsRekognition` | 找出图像中的标签、文本、人脸和知名人物，以及每张人脸在哪里 |
| `AwsPolly` | 把一段文本朗读成一个音频文件 |
| `AwsComprehend` | 一段文本的情感、实体、关键短语和语言 |
| `AwsTextract` | 读取文档的行、表单字段和表格 |
| `AwsEC2` | 描述、启动和停止实例 |
| `AwsCognito` | 让你的应用程序自己的用户注册、登录和退出 |
| `AwsMcp` | 通用的门：AWS 服务器的任何工具，或任何单个 AWS API 调用，按名称 |

没有一个要求你写 HTTP 请求或给它签名，也没有一个需要你
处理凭据——你放下控件，填好它的属性，再调用
一个方法，就像 PowerCOBOL 或 isCOBOL 开发者调用控件的方法那样。

在它们背后，应用程序与 **AWS 自己的 MCP 服务器**对话，方式有两种。
大多数程序使用的服务——DynamoDB、S3、S3 Vectors、Rekognition、
Polly、Comprehend、Textract、EC2、Cognito 和 `AwsMcp.AwsCall`——
通过互联网访问 **AWS 的托管 MCP 服务**：应用程序自己为每个请求签名，
用 AWS CLI 的登录，或用你的程序交给它的密钥（见下文
*运行时键入的密钥*），而且**不需要安装任何东西**。
Lambda、Bedrock Knowledge Bases、AgentCore、S3 Tables 和 Glue 由
**AWS 发布的小程序**提供服务，应用程序在这台机器上用同样的凭据
启动它们：

```mermaid
flowchart LR
    H["你的 COBOL 处理程序<br/>INVOKE DDB-1 &quot;GetItem&quot;"] --> C["一个 AWS 控件"]
    C --> R["路由表<br/>（哪个服务器，哪个工具）"]
    R --> W["AWS 托管的 MCP 服务<br/>（请求由应用程序签名）"]
    R --> S["AWS 服务器程序<br/>（每个连接启动一次）"]
    W --> A["AWS<br/>用配置文件签名<br/>（或你设置的密钥）"]
    S --> A
    A --> C
    C --> E["控件的事件，然后是 onComplete<br/>(ResponseBody, RowCount, …)"]
```

#### 一台机器需要什么

- **对每个 AWS 控件**（`AwsLambda`、`AwsKnowledgeBase`、`AwsAgentCore`、
  `AwsAgentMemory`、`AwsS3Tables`、`AwsGlue`、`AwsDynamoDB`、`AwsS3`、
  `AwsS3Vectors`、`AwsRekognition`、`AwsPolly`、`AwsComprehend`、
  `AwsTextract`、`AwsEC2`、`AwsCognito`，以及 `AwsMcp` 的 `AwsCall`）：**无需
  安装任何东西**——不需要 Python，不需要 `uv`。每个都访问托管的 AWS MCP Server，
  机器只需要下面的**登录**。（直到 1.91.210，前六个
  还用 `uvx` 启动 AWS Labs 的服务器程序。）
- **只对其 `ServerId` 指名某个 AWS Labs 服务器程序的 `AwsMcp`**
  （`lambda`、`bedrock-kb`、`agentcore`、`s3tables`、`dataprocessing`）：需要 uv，
  由它启动该程序——`brew install uv`、`pip install uv` 或 `winget install
  astral-sh.uv`。uv 会带上它所运行的 Python。
- **对 `AwsS3Tables` 的 `Query` 和 `AppendRows`**，它们通过 Amazon
  Athena 运行：与 AWS 分析服务集成的表存储桶（一次，在
  S3 控制台中），以及 Athena 结果的存放位置——工作组自己的，
  或 `QueryResultsLocation`。
- 一个其 IAM 策略允许 AWS MCP Server 本身（AWS 的设置
  页面列出了所需权限）以及你所调用服务的配置文件。
- **一个登录。**AWS CLI（2.32 或更高版本）和一个已登录的配置文件：
  在那台机器上执行一次 `aws login --profile sales`（对使用 IAM Identity Center 的配置文件，
  则是 `aws sso login --profile
  sales`）。应用程序自己读取该配置文件的登录——共享的 AWS 文件、IAM Identity Center、
  `aws login`、`credential_process`、要承担的角色——并在它
  快要过期时续期。当连接没有指名配置文件时，使用 AWS
  环境变量。或者应用程序改为向它的用户索要密钥
  （见下文*运行时键入的密钥*）。

这就是全部清单，对你的机器**和你的用户的机器**都一样：构建出的
应用程序不携带任何 AWS 密钥，也不携带需要配置的 AWS 库。
一台机器上第一次托管调用会与 AWS 的服务建立会话，
需要一两秒；第一次调用服务器程序（仅 `AwsMcp`）
会下载它，这可能需要一分钟，此后的每次调用大约一秒就启动。一个
会话或服务器程序为其连接上的每个 AWS 控件提供服务，直到
应用程序结束。

#### AWS 连接

**Settings ▸ Integrations ▸ AWS** 保存项目的 AWS 连接：一个名称、一个
**AWS profile**（配置文件）、一个 **region**（区域）、应用程序可以访问哪些 Lambda 函数
（一个 **function prefix**，如 `sales-`，或以逗号分隔的 **function
list**），以及一个用于高级情形的可选路由覆盖。不保存任何机密——
配置文件为运行它的机器上的每次调用签名，除非
运行中的程序给连接交来它自己的密钥。

**Test connection** 检查每个 AWS 控件都使用的托管 AWS MCP Server，并以你的语言
显示结果：是无法到达该地址、配置文件未登录或已过期（以及应当运行的
`aws login` 或 `aws sso login` 命令）、AWS 拒绝了会话（该登录可能
不被允许使用该服务）、服务的应答与预期不符，还是
**已连接**。它不启动任何服务器程序，所以没有 `uv` 的机器不会
被告知这一点：自 1.91.210 起，没有控件需要它。（路由覆盖
仍把某个操作发往服务器程序的连接，会为它多一行说明。）

> 📷 **需要截图 — `aws-connections.png`。**打开 Settings ▸
> Integrations，添加一个带配置文件和区域的 AWS 连接，按 **Test
> connection**，并截取显示该连接及其结果行的 AWS 部分——“已连接到托管的 AWS 服务……”。

每个 AWS 控件的 **Connection** 属性按名称指名连接。在恰好有一个 AWS 连接的项目中
放下的控件会采用它，
而空的 Connection 就表示那唯一的一个。

> 📷 **需要截图 — `aws-card.png`。**把一个 `AwsLambda` 控件放到
> 窗体上，截取它在画布上的磁贴，以及旁边显示
> Connection、Mode、FunctionName 和 AllowWrite 的检查器。

#### 运行时键入的密钥：`AWS-SET-CREDENTIALS`

配置文件对你自己的机器，以及对你设置的任何机器都是正确的选择——
在那里请继续使用它。不过，有些应用程序运行在没有人
会键入 `aws login` 的地方：交给客户的演示、用户
有自己 AWS 访问密钥的工具。这样的应用程序可以向它的用户索要
**访问密钥 id**、**秘密访问密钥**，以及——对临时密钥——**会话令牌**，
并把它们交给项目的某个连接：

```cobol
       PROGRAM-ID. BTN-CONNECT--ONCLICK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-STATUS     PIC X(120).
       PROCEDURE DIVISION.
           COBOL::"AWS-SET-CREDENTIALS" ( "Sales" TXT-KEY-ID::Text
               TXT-SECRET::Text TXT-TOKEN::Text WS-STATUS )
           MOVE SPACES TO TXT-KEY-ID::Text TXT-SECRET::Text TXT-TOKEN::Text
           IF WS-STATUS = "OK"
               MOVE "Keys accepted for this session" TO LBL-STATE::Caption
           ELSE
               MOVE WS-STATUS TO LBL-STATE::Caption
           END-IF
           .
```

从那时起，直到应用程序结束，`Sales`
连接上的每个 AWS 控件都使用这些密钥而不是连接的配置文件工作：

```mermaid
flowchart LR
    U["你的用户键入密钥"] --> P["COBOL::&quot;AWS-SET-CREDENTIALS&quot;<br/>( connection key-id secret token status )"]
    P --> M["运行时的内存<br/>（仅本次运行）"]
    M --> W["托管服务：<br/>应用程序用它们签名"]
    M --> S["一个服务器程序（仅 AwsMcp）：<br/>在它的环境中接收它们，<br/>没有配置文件"]
    W --> A["AWS"]
    S --> A
```

| 内置调用 | 参数 | 它做什么 |
|---|---|---|
| `AWS-SET-CREDENTIALS` | connection, key id, secret, token, [status] | 在本次运行中给该连接这些密钥。令牌可以是空格（永久密钥没有令牌）。`status` 是 `OK`，或者说明为什么什么也没存储：没有这样的连接、缺少密钥 id 或秘密、密钥中有空格。 |
| `AWS-CREDENTIALS-SET` | connection, flag | 当连接设置了密钥时为 `Y`，没有时为 `N`。绝不是密钥本身。 |
| `AWS-CLEAR-CREDENTIALS` | connection, [status] | 忘记密钥；连接回到它的配置文件。 |

连接的指名方式与控件的 `Connection` 指名它的方式相同——按名称
（或 id），而空表示项目唯一的连接。

运行时对这些密钥做什么，以及它绝不做什么：

- 它们只保存在**内存中**，仅限应用程序的这一次运行。它们
  不是属性，所以没有窗体、项目文件、`Verbose` 行、错误
  消息或日志会保存它们，也没有任何内置调用会把它们交回。
- 对于托管的 AWS MCP Server——每个 AWS 控件——应用程序自己
  用它们签名，它们不去别的任何地方。由 `AwsMcp`
  通过它的 `ServerId` 启动的服务器程序在它的环境中接收它们——绝不在
  它的命令行上，绝不进入另一个程序。
- 设置新密钥，或清除它们，会立即结束该连接正在运行的服务器
  和托管会话；下一次调用用现在设置的内容打开或启动一个。服务器程序
  的启动要多花一秒。
- AWS 因密钥而拒绝的调用会触发 `onError`，其 `LastError`
  说明密钥被拒绝，应当重新设置——绝不会包含密钥。

> **注意。**文本框仍然保存着用户键入的内容，直到你清除
> 它们——这就是示例在调用之后马上把 `SPACES` 移到它们的原因。请把秘密所在文本框的
> `PasswordCharacter` 设为（比如）`*`，这样秘密在键入时绝不会显示。

> ⚠️ **注意事项。**
> - 这些密钥与它们所属的 IAM 用户或角色一样强大。请索要
>   其策略只允许应用程序所做之事的密钥。
> - 两次运行之间不记住任何东西：每次应用程序启动，用户都要再次
>   键入密钥。有意不提供保存它们的做法。

#### 在你说可以之前，没有什么会更改 AWS：`AllowWrite`

`AllowWrite` 在每个新的 AWS 控件上都是**关闭**的。它关闭时，每个更改某样东西、启动某样东西或
因动作而计费的操作都会被
拒绝。这是完整的清单：

| 控件 | `AllowWrite` 关闭时被拒绝的 |
|---|---|
| `AwsLambda` | `Invoke`——运行一个函数可以更改任何东西，所以它算作写入 |
| `AwsAgentCore` | `Invoke` |
| `AwsAgentMemory` | `RecordEvent` |
| `AwsS3Tables` | `AppendRows` |
| `AwsGlue` | `StartJobRun`、`StartCrawler` |
| `AwsDynamoDB` | `PutItem`、`UpdateItem`、`DeleteItem` |
| `AwsS3` | `PutObject`、`DeleteObject` |
| `AwsS3Vectors` | `PutVectors` |
| `AwsEC2` | `Start`、`Stop` |
| `AwsCognito` | `SignUp`、`Confirm` |
| `AwsMcp` | `Call`，对其服务器没有声明为只读的任何工具；`AwsCall`，始终如此——它无法区分读取的 API 调用和写入的 API 调用 |

其他每个操作都只读取，而 `AwsKnowledgeBase`、`AwsRekognition`、
`AwsPolly`、`AwsComprehend` 和 `AwsTextract` 在这张清单上根本没有
操作。有只读模式的服务器会以它启动。

被拒绝的调用什么也不发送，并触发 `onError`，由 `LastError` 说明
原因。请只为确实需要它的控件打开 `AllowWrite`。

#### 示例：运行 Lambda 函数

设计器预置 `Mode = Async`：`Invoke` 立即返回 `1`，而
答案作为 `onInvoked` 到达，然后是 `onComplete`。

```cobol
       PROGRAM-ID. BTN-QUOTE--ONCLICK.
       PROCEDURE DIVISION.
           LAMBDA-1::Invoke("sales-quote",
               '{"customer": 1042, "items": 3}')
           .

       PROGRAM-ID. LAMBDA-1--ONINVOKED.
       PROCEDURE DIVISION.
           IF LAMBDA-1::FunctionError = SPACES
               MOVE LAMBDA-1::ResponseBody TO LBL-QUOTE::Caption
           ELSE
               MOVE LAMBDA-1::FunctionError TO LBL-QUOTE::Caption
           END-IF
           .
```

- 载荷必须是 JSON。不是 JSON 的载荷会在发送任何东西之前被拒绝，
  并由 `LastError` 指明是哪个参数。
- `FunctionError` 是**函数**所报告的（例如
  `Unhandled`）：调用本身成功了，所以 `onInvoked` 仍然触发。
  `onError` 保留给没有到达函数的调用。
- 作为 JSON 数组的答案也是一个行集：`RowCount` 行，每一行
  用 `GetRow(n)`（作为 JSON 的行）或 `GetField(n, "name")` 读取。
- `ListFunctions` 列出连接所提供的函数
  （`onFunctionsListed`；`GetField(n, "Name")`）。

使用 `Mode = Sync` 时，同样的 `Invoke` 会等待并自己返回答案：

```cobol
           MOVE "Sync" TO LAMBDA-1::Mode
           MOVE LAMBDA-1::Invoke("sales-quote", '{"customer": 1042}')
               TO LBL-QUOTE::Caption
```

#### 示例：用 `AwsMcp` 调用任何 AWS 工具

`ServerId` 指名连接的某个 AWS 服务器，而 `Call` 用一个 JSON 参数对象
运行它的一个工具。`ListTools` 列出它们——`Name`、
`Description` 和 `ReadOnly`（当服务器把该工具标记为只读时为 `1`）。

```cobol
       PROGRAM-ID. BTN-TOOLS--ONCLICK.
       PROCEDURE DIVISION.
           MCP-1::Call("list_functions", "{}")
           .

       PROGRAM-ID. MCP-1--ONTOOLRESULT.
       PROCEDURE DIVISION.
           MOVE MCP-1::ResponseBody TO TXT-OUT::Text
           .
```

`ResponseBody` 是文本形式的工具答案，而 `ResultJson` 是同一个答案的
JSON 形式，供想要全部内容的开发者使用。

#### 每个参数都有默认值：控件自己的属性

每个 AWS 方法都以固定的顺序接收它的参数，而且**其中任何一个都可以
留空**（`SPACES`）：控件自己的属性会代替它。一个你在设计器中设置了
`KnowledgeBaseId` 的 `AwsKnowledgeBase`，用 `Query` 提问时不需要再指名任何别的东西；
一个完全没有参数的 `GetJobRun` 会跟踪上一次 `StartJobRun` 所启动的那次运行。这些
属性都在检查器中，所以大多数程序只传入那个会变的东西
——问题、提示、SQL。

作为列表的答案——段落、记录、表、列——以
**行**的形式到达：`RowCount` 说明有多少行，`GetField(n, "name")` 读取第 `n` 行
（从 1 开始）的一个字段。下面每个示例都会指名它的行所带有的字段。

#### 示例：询问 Bedrock 知识库

知识库就是你自己的文档，由 Amazon Bedrock 建立索引。设置
`KnowledgeBaseId`（以及 `MaxResults`，默认 10）；`Query` 返回回答该问题的
段落，最好的在前，并附上每个段落来自哪里，以及
匹配得多好。

```cobol
       PROGRAM-ID. BTN-ASK--ONCLICK.
       PROCEDURE DIVISION.
           BEDROCK-1::Query("", TXT-QUESTION::Text)
           .

       PROGRAM-ID. BEDROCK-1--ONQUERIED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-I          PIC 9(4).
       01 WS-ALL        PIC X(8000).
       01 WS-PTR        PIC 9(4).
       PROCEDURE DIVISION.
           MOVE SPACES TO WS-ALL
           MOVE 1 TO WS-PTR
           PERFORM VARYING WS-I FROM 1 BY 1
                   UNTIL WS-I > BEDROCK-1::RowCount
               STRING BEDROCK-1::GetField(WS-I, "Text")::Trim()
                      " ["
                      BEDROCK-1::GetField(WS-I, "Source")::Trim()
                      "] "
                   DELIMITED BY SIZE INTO WS-ALL WITH POINTER WS-PTR
           END-PERFORM
           MOVE WS-ALL TO TXT-OUT::Text
           .
```

- 每一行都有 `Text`、`Source`（文档的 S3 地址、网页或其他
  位置）和 `Score`。图像段落被排除在外。
- `ListKnowledgeBases` 列出你的登录可以看到的每个知识库——
  `Id`、`Name`、`Description`、`Type`——在 `onKnowledgeBasesListed` 中。（直到
  1.91.210，只有带 `mcp-multirag-kb` = `true` 标签的才会被访问到：那是
  当时控件所启动的服务器程序的约定。）
- `AwsKnowledgeBase` 的每个操作都只读取，所以它从不需要
  `AllowWrite`。

#### 示例：与 AgentCore 智能体的对话，并被记住

`AwsAgentCore` 把提示发送给你部署在 Amazon Bedrock
AgentCore Runtime 中的智能体（`RuntimeArn`），并把回复放进 `ResponseBody`。
第一次 `Invoke` 开始一场对话并把它的 id 写入 `SessionId`，所以
下一次 `Invoke` 会继续它；把空格移到 `SessionId` 即可重新开始。
`AwsAgentMemory` 把每一轮记录在 AgentCore Memory 中，这样以后的会话
就能回忆起用户说过的话。

```cobol
       PROGRAM-ID. BTN-CHAT--ONCLICK.
       PROCEDURE DIVISION.
           CORE-1::Invoke("", TXT-QUESTION::Text)
           .

       PROGRAM-ID. CORE-1--ONINVOKED.
       PROCEDURE DIVISION.
           MOVE CORE-1::ResponseBody TO TXT-OUT::Text
           MOVE CORE-1::SessionId TO MEMORY-1::SessionId
           MEMORY-1::RecordEvent("", "", "", CORE-1::ResponseBody,
               "ASSISTANT")
           .
```

- `Invoke` 把提示作为 `{"prompt": "…"}` 发送，这是用
  AWS 的入门工具包构建的智能体所读取的形态。智能体以 JSON 发送的回复
  会作为那段 JSON 文本到达。
- `RecordEvent(memoryId, actorId, sessionId, text, role)`——三个 id
  默认为 `MemoryId`、`ActorId` 和 `SessionId`，而 `role` 默认为 `USER`
  （智能体一方为 `ASSISTANT`）。`onEventRecorded` 给出 `EventId`。
- `Retrieve(memoryId, namespace, query)` 搜索记忆所学到的东西
  （它的长期记录，最多 `TopK` 条），并在 `onRetrieved` 中返回带有 `Text`、`Score`、
  `Id` 和 `CreatedAt` 的行。
- ⚠️ `Invoke` 和 `RecordEvent` 需要 `AllowWrite`：智能体可以行动，而每次
  调用都会计费。
- ⚠️ AgentCore 在它的答案内部报告的失败——未知的智能体、缺失的
  记忆——会触发 `onError`，并在 `LastError` 中带有 AgentCore 自己的消息。

#### 示例：用 SQL 读取 S3 Tables

`AwsS3Tables` 作用于一个表存储桶（`TableBucketArn`）和一个命名空间
（`Namespace`）。`Query` 通过 **Amazon
Athena** 运行一条只读 SQL 语句，每个结果行都成为一个其字段以
列命名的行，每个值都是文本。Athena 把存储桶读作目录
`s3tablescatalog/<bucket name>`，把命名空间读作它的数据库，所以请在 S3 控制台中
把表存储桶与 AWS 分析服务集成一次。它
在工作组 `WorkGroup`（默认 `primary`）中运行，并把结果写到
该工作组的结果位置，或者当你设置了 `QueryResultsLocation`（一个 `s3://`
文件夹）时写到那里。登录需要在工作组上有 Athena 的权限、
对目录有读取权限，以及对结果文件夹有写入权限：

```cobol
       PROGRAM-ID. BTN-SALES--ONCLICK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-SQL        PIC X(200) VALUE
           "SELECT region, SUM(total) AS total FROM orders GROUP BY region".
       PROCEDURE DIVISION.
           TABLES-1::Query(WS-SQL)
           .

       PROGRAM-ID. TABLES-1--ONQUERIED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-I          PIC 9(4).
       01 WS-ALL        PIC X(4000).
       01 WS-PTR        PIC 9(4).
       PROCEDURE DIVISION.
           MOVE SPACES TO WS-ALL
           MOVE 1 TO WS-PTR
           PERFORM VARYING WS-I FROM 1 BY 1
                   UNTIL WS-I > TABLES-1::RowCount
               STRING TABLES-1::GetField(WS-I, "region")::Trim()
                      ": "
                      TABLES-1::GetField(WS-I, "total")::Trim()
                      "  "
                   DELIMITED BY SIZE INTO WS-ALL WITH POINTER WS-PTR
           END-PERFORM
           MOVE WS-ALL TO TXT-OUT::Text
           .
```

- `ListTables(namespace)` 把该区域的表列为行——`Namespace`、
  `Name`、`TableArn`、`Type`、`ModifiedAt`。设置了 `Namespace`（或传入了
  命名空间）时，只保留该命名空间的表。
- `AppendRows(table, rows)` 把一个对象的 JSON 数组（每行一个）追加到
  `table`（为空时用 `TableName`），用的是 Athena 的 `INSERT INTO`，其中
  每个值都被转换成它所在列的类型；`onRowsAppended` 给出
  `RowsAppended`。它需要 `AllowWrite`，不是有效 JSON 的行会在发送任何东西之前
  被拒绝。
- ⚠️ `Query` 只读取：不是 `SELECT`、`WITH`、`SHOW`、
  `DESCRIBE`、`EXPLAIN` 或 `VALUES` 的语句会在发送任何东西之前被拒绝，
  并触发 `onError`。请用 `AppendRows` 来添加数据。

#### 示例：运行 Glue 作业并跟踪它

`AwsGlue` 启动一个作业（`JobName`），并告诉你它的运行进展如何。
`StartJobRun` 把新一次运行的 id 写入 `JobRunId`，所以没有
参数的 `GetJobRun` 恰好跟踪那次运行：

```cobol
       PROGRAM-ID. BTN-ETL--ONCLICK.
       PROCEDURE DIVISION.
           GLUE-1::StartJobRun("", '{"--day": "2026-10-07"}')
           .

       PROGRAM-ID. GLUE-1--ONJOBSTARTED.
       PROCEDURE DIVISION.
           MOVE GLUE-1::JobRunId TO LBL-STATE::Caption
           .

       PROGRAM-ID. BTN-CHECK--ONCLICK.
       PROCEDURE DIVISION.
           GLUE-1::GetJobRun()
           .

       PROGRAM-ID. GLUE-1--ONJOBRUN.
       PROCEDURE DIVISION.
           MOVE GLUE-1::State TO LBL-STATE::Caption
           .
```

- `State` 是 Glue 自己对运行的称呼：`STARTING`、`RUNNING`、`SUCCEEDED`、
  `FAILED`、`TIMEOUT`、`STOPPED` ……。一个每隔几秒调用 `GetJobRun`，直到它
  不再是 `RUNNING` 的 Timer，是等待的常用办法。
- `StartCrawler(name)` 启动一个爬虫（为空时用 `CrawlerName`），在
  `onCrawlerStarted` 中。
- `GetTableSchema(database, table)` 在 `onTableSchema` 中把 Data Catalog 表的列
  作为行返回——`Name`、`Type`、`Comment`。
- ⚠️ `StartJobRun` 和 `StartCrawler` 需要 `AllowWrite`。为了你的用户的
  隐私，服务器在它返回的内容中不包含运行的参数和错误消息；
  状态则始终都在。

#### 通用控件：一个 AWS 服务器，覆盖多项服务

`AwsDynamoDB`、`AwsS3`、`AwsS3Vectors`、`AwsRekognition`、`AwsPolly`、
`AwsComprehend`、`AwsTextract`、`AwsEC2` 和 `AwsCognito` 全都通过
**AWS MCP Server**——AWS 自己运行的那个。应用程序直接通过互联网访问它，
并用连接的配置文件（或你的程序设置的密钥）自己为每个请求签名；
不启动任何程序，也不为它安装任何东西。你对此毫无察觉：你调用 `GetItem`，
就像在其他控件上调用 `Query` 一样。有两点值得知道：

- 每个请求都是一个**随 PowerRustCOBOL 一起交付**的小型 Python 脚本，由
  AWS 在沙箱中运行。你的程序传入的值只作为数据进入其中
  ——一段文本、一个数字、一个文件的字节——绝不作为代码，所以用户
  键入的任何东西都不能改变所运行的内容。
- 这些调用比专用服务器的耗时更长——每次几秒，
  因为 AWS 每次都要启动沙箱。请为它们保持 `Mode = Async`，
  并且不要把其中一个放进对数千行的循环里。

#### 示例：DynamoDB 条目作为纯 JSON

DynamoDB 自己的 API 为每个值都写上它的类型（`{"N": "42"}`）。
`AwsDynamoDB` 不这样做：键、条目和值都是你的程序
本来就会写的 JSON，而条目会作为其字段就是
属性的行返回。

```cobol
       PROGRAM-ID. BTN-FIND--ONCLICK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-KEY        PIC X(100).
       PROCEDURE DIVISION.
           STRING '{"id": ' TXT-ID::Text::Trim() '}'
               DELIMITED BY SIZE INTO WS-KEY
           ORDERS-1::GetItem(WS-KEY)
           .

       PROGRAM-ID. ORDERS-1--ONITEM.
       PROCEDURE DIVISION.
           IF ORDERS-1::Found = "1"
               MOVE ORDERS-1::GetField(1, "customer") TO LBL-STATE::Caption
           ELSE
               MOVE "No such order" TO LBL-STATE::Caption
           END-IF
           .
```

- `Query(keyCondition, values, index)` 读取匹配某个键
  条件的条目——`customer = :c` 配 `{":c": "C-7"}`——最多 `Limit` 个，在没有给定索引时从
  `IndexName` 读取。`Scan(limit)` 读取整张表。
- `PutItem(item)`、`UpdateItem(key, expression, values)` 和
  `DeleteItem(key)` 需要 `AllowWrite`。`UpdateItem` 把条目的新
  状态作为第 1 行返回。

#### 示例：一幅图像、一份文档、一个声音

`AwsRekognition` 和 `AwsTextract` 接收图像或文档，既可以是
**本地文件**，也可以是 S3 中的对象（`s3://bucket/key`）。`AwsPolly` 把
语音写入本地文件。

```cobol
       PROGRAM-ID. BTN-PHOTO--ONCLICK.
       PROCEDURE DIVISION.
           VISION-1::DetectLabels("photos/dog.jpg")
           .

       PROGRAM-ID. VISION-1--ONLABELS.
       PROCEDURE DIVISION.
           MOVE VISION-1::GetField(1, "Name") TO LBL-STATE::Caption
           .
```

| 控件 | 方法 | 行 / 结果 |
|---|---|---|
| `AwsRekognition` | `DetectLabels`、`DetectText`、`DetectFaces`、`RecognizeCelebrities`（图像） | `Name`, `Confidence` · `Text`, `Type` · `AgeLow`, `AgeHigh`, `Smile`, `Emotion`, `Left`, `Top`, `Width`, `Height` · `Name`, `Id`, `MatchConfidence`, `Urls`, `Left`, `Top`, `Width`, `Height` |
| `AwsTextract` | `DetectText`、`AnalyzeDocument`（文档） | 行 `Text`, `Page` · 字段和单元格：`Kind`（FIELD 或 CELL）, `Key`, `Value`, `Table`, `Row`, `Column` |
| `AwsPolly` | `Synthesize(text, voice, format, toFile)` | `SavedFile`, `Characters`（Polly 计费的内容） |
| `AwsComprehend` | `DetectSentiment`、`DetectEntities`、`DetectKeyPhrases`、`DetectLanguage` | `Sentiment` · `Text`, `Type`, `Score` · `Language` |
| `AwsS3` | `List(prefix)`、`GetObject(key, toFile)`、`PutObject(key, text, fromFile)`、`DeleteObject(key)`、`ListBuckets()` | `Key`, `Size`, `LastModified` · 作为文本放在 `ResponseBody` 中的对象 · 你的存储桶：`Name`, `CreationDate` |
| `AwsS3Vectors` | `QueryVectors(index, vector, topK)`、`PutVectors(index, vectors)` | `Key`, `Distance`, `Metadata` |
| `AwsEC2` | `Describe(ids)`、`Start(ids)`、`Stop(ids)` | `InstanceId`, `Name`, `State`, `Type`, `PublicIp`, `PrivateIp` |

⚠️ 图像、文档和对象是在请求和答案之内传输的，
所以这些适合几兆字节的文件——一张照片、一份扫描的表单、一份报告——
而不是视频。

#### 示例：谁在图片里，在哪里

`RecognizeCelebrities` 点出 Rekognition 在
图片中找到的知名人物——每人一行，带有 `Name`、`Id`、`MatchConfidence`（0 到 100）和
`Urls`，即 Rekognition 为他们所知的链接，以空格分隔。没有
人能被认出的人脸不会成为一行。

每一行还会说明**人脸在哪里**：`Left`、`Top`、`Width` 和 `Height`
是占图片宽度和高度的比例（0 到 1），从它的
左上角算起——与每个 `DetectFaces` 行带有的四个字段相同。
把它们乘以图片显示时的大小，你就得到一个可以画在
它上面的方框。对于 `SizeMode` 为 `Stretch` 的 PictureBox，图片恰好填满
控件，所以控件自己的 `X`、`Y`、`Width` 和 `Height` 就是
图片的：

```cobol
       PROGRAM-ID. BTN-WHO--ONCLICK.
       DATA DIVISION.
       PROCEDURE DIVISION.
           INVOKE VISION-1::RecognizeCelebrities(PIC-PHOTO::ImagePath)
           .

       PROGRAM-ID. VISION-1--ONCELEBRITIES.
       DATA DIVISION.
       PROCEDURE DIVISION.
           IF VISION-1::RowCount = 0
               MOVE "Nobody I know" TO LBL-STATE::Caption
           ELSE
               MOVE VISION-1::GetField(1, "Name") TO LBL-STATE::Caption
               COMPUTE SHP-FACE::X = PIC-PHOTO::X
                   + VISION-1::GetField(1, "Left") * PIC-PHOTO::Width
               COMPUTE SHP-FACE::Y = PIC-PHOTO::Y
                   + VISION-1::GetField(1, "Top") * PIC-PHOTO::Height
               COMPUTE SHP-FACE::Width =
                   VISION-1::GetField(1, "Width") * PIC-PHOTO::Width
               COMPUTE SHP-FACE::Height =
                   VISION-1::GetField(1, "Height") * PIC-PHOTO::Height
               MOVE TRUE TO SHP-FACE::Visible
           END-IF
           .
```

- 每个想要框出的人脸一个 `Shape`；对 `RowCount` 做循环，有多少个就
  放置多少个。
- ⚠️ 使用 `SizeMode = Zoom` 时，图片保持它的比例，并在两侧留下
  边距，所以方框必须按图片真正占据的矩形来计算，
  而不是整个控件。
- Rekognition 接受 JPEG 和 PNG 图片。

> 📷 **需要截图 — `aws-celebrities.png`。**运行一个带有
> PictureBox（`SizeMode = Stretch`）、一个圆角 `Shape` 和一个
> `AwsRekognition` 的窗体，识别一张知名人物的照片，并截取
> 画在人脸上的方框，旁边的标签里是名字。

#### 示例：用 `AwsCall` 调用任何 AWS API

上面的控件涵盖了应用程序通常需要的东西。对于其他一切
——创建 CloudFormation 堆栈、创建 DynamoDB 表、询问某个配置文件属于
哪个账户——`AwsMcp` 有 **`AwsCall`**：对任何 AWS
API 的一次调用，按 AWS 命名它的方式来命名，通过通用控件所使用的
同一个 AWS MCP Server（不管它的 `ServerId` 怎么说）。

| 参数 | 写什么 | 示例 |
|---|---|---|
| `service` | 该服务在 AWS 的 SDK for Python 中的短名称——几乎总是 AWS CLI 的（`aws cloudformation …`），但 S3 的 API 例外，它是 `s3`（CLI 的 `s3api`） | `cloudformation`, `dynamodb`, `s3`, `sts`, `cognito-idp` |
| `operation` | API 操作自己的名称，采用 CamelCase，如该服务的 API Reference 所拼写 | `DescribeStacks`, `CreateStack`, `CreateTable`, `GetCallerIdentity` |
| `paramsJson` | 一个由该操作的参数构成的 JSON 对象，按 API Reference 所拼写的写法；为空表示没有 | `{"StackName": "powercloud"}` |

```cobol
       PROGRAM-ID. BTN-STACK--ONCLICK.
       DATA DIVISION.
       PROCEDURE DIVISION.
           INVOKE CLOUD-1::AwsCall("cloudformation", "DescribeStacks",
               '{"StackName": "powercloud"}')
           .

       PROGRAM-ID. CLOUD-1--ONAWSCALLED.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE CLOUD-1::GetField(1, "StackStatus") TO LBL-STATE::Caption
           .

       PROGRAM-ID. CLOUD-1--ONERROR.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE CLOUD-1::LastError TO LBL-STATE::Caption
           .
```

返回的内容：

- `ResponseBody` 和 `ResultJson` 以 JSON 保存 API 的答案，其中 AWS 的
  请求记录（`ResponseMetadata`）被排除在外，而每个日期都写成
  ISO 8601（`2026-10-09T12:00:00+00:00`）。
- **行**来自答案的形状：当它恰好包含一个列表
  （`Stacks`、`Buckets`、`TableNames`）时，每个条目是一行；当它唯一的
  成员是一个对象（`Table`、`TableDescription`）时，该对象是第 1 行；
  否则答案本身是第 1 行（`CreateStack` 之后的 `GetField(1, "StackId")`）。
  由普通值构成的列表给出只有一个字段 `Value` 的行。
- AWS 拒绝的调用会触发 **`onError`**，绝不会是一个空答案：`LastError`
  以 AWS 自己的错误代码开头——`ValidationException: Stack with id
  powercloud does not exist`、`AccessDeniedException: …`、
  `ResourceInUseException: …`——所以处理程序可以测试它所预期的那一个
  （一个已经存在的表、一个尚不存在的堆栈）。未
  登录的配置文件，会按每个 AWS 控件报告它的方式来报告。

```mermaid
flowchart LR
    C["INVOKE CLOUD-1::AwsCall(service, operation, params)"] --> W{"AllowWrite 已开启？"}
    W -- 否 --> R["onError——什么也没发送"]
    W -- 是 --> S["随附的脚本<br/>（三个值作为带引号的文本）"]
    S --> A["AWS API，在连接的区域中"]
    A -- 答案 --> E["onAwsCalled，然后是 onComplete<br/>ResponseBody，行"]
    A -- AWS 错误 --> F["onError<br/>LastError = Code: message"]
```

- ⚠️ `AwsCall` **始终需要 `AllowWrite`**：它无法区分读取的操作
  和更改某样东西的操作，所以把它们全都当作更改。
  请只在发出这些调用的控件上打开 `AllowWrite`。
- 这三个值作为**请求内部的文本传输，绝不作为代码**——
  与每个通用控件的规则相同——所以来自用户的名称
  可以指名一次调用，但绝不会变成一次调用。
- 不是有效 JSON 的参数会在发送任何东西之前被拒绝，并且
  `LastError` 会指明参数 3。
- 配置文件自己的 IAM 权限仍然决定这次调用可以做什么；
  不被允许的操作会回答 `AccessDeniedException`。

#### 示例：用 Cognito 让你的用户登录

`AwsCognito` 让**你的应用程序的用户**登录到一个 Cognito 用户
池：`SignUp`、`Confirm`（用 Cognito 通过电子邮件发送的代码）、`SignIn`、
`GetAttribute`、`SignOut`。请把 `ClientId` 设为一个**没有
客户端密钥**的应用客户端。

```cobol
       PROGRAM-ID. BTN-LOGIN--ONCLICK.
       PROCEDURE DIVISION.
           USERS-1::SignIn(TXT-USER::Text, TXT-PASSWORD::Text)
           MOVE SPACES TO TXT-PASSWORD::Text
           .

       PROGRAM-ID. USERS-1--ONSIGNEDIN.
       PROCEDURE DIVISION.
           IF USERS-1::SignedIn = "1"
               MOVE USERS-1::UserName TO LBL-STATE::Caption
           ELSE
               MOVE USERS-1::Challenge TO LBL-STATE::Caption
           END-IF
           .
```

- 登录所返回的令牌**从不到达你的程序**：运行时
  把它们保存在内存中，用于 `GetAttribute` 和 `SignOut`，并在
  用户退出或应用程序结束时忘掉它们。你的 COBOL
  看到的是 `SignedIn` 和 `UserName`——没有任何可能泄露的东西。
- `Verbose` 绝不会打印携带密码或令牌的请求。
- `Challenge` 指名 Cognito 所要求的、用来代替登录的东西（新
  密码、第二因素）；这些流程目前还不提供。
- ⚠️ 密码在请求内部传输到 AWS——经过加密，由
  连接的配置文件签名，并且 PowerRustCOBOL 绝不会把它写到任何地方。
  请像示例那样，清除保存它的字段。
#### 示例：PowerCloud——每类控件各一个

**Help ▸ Examples ▸ PowerCloud** 是一个带有七个标签页的单一窗口，每个
标签页展示一个 AWS 控件在做人们想要完成的事情。想看这些控件如何在一个应用程序中
而不是在一个演示窗体里配合，这是最快的办法。

| 标签页 | 它展示什么 | 它使用的控件和内置调用 |
|---|---|---|
| Settings | 键入密钥，用一次只读调用来证明；部署堆栈，或保存它的模板 | `AwsMcp` `AwsCall`、`AWS-SET-CREDENTIALS`、`SAVE-FILE-DIALOG` |
| DynamoDB | 创建表，过滤表格，在弹出窗口中添加 / 查看 / 编辑 / 删除 | `AwsDynamoDB`、`AwsS3`、`AwsLambda`、`IMAGE-FIT`、`HTTP-DOWNLOAD` |
| S3 | 账户的存储桶，以及其中一个的对象 | `AwsS3` `ListBuckets` 和 `List` |
| Lambda | 生成临时链接的函数，以及一个调用它的按钮 | `AwsLambda` `Invoke` |
| Polly | 朗读一句话，带有播放 / 暂停 / 停止的播放器 | `AwsPolly` `Synthesize`、`AUDIO-PLAY` 及其配套调用 |
| EC2 | 带有类别、vCPU 和内存的服务器；一个启动和停止某台服务器的列 | `AwsEC2`，一个用 `OpenFormAsync` 打开的 Working 窗口 |
| Rekognition | 在找到的每位名人上画一个方框 | `AwsRekognition` `RecognizeCelebrities`、`IMAGE-INFO`、`Shape` |

> 📷 **需要截图 — `powercloud-tabs.png`。**打开该示例，用一个可用的 AWS 用户按
> **Connect**，然后截取带有两三只宠物的 DynamoDB 标签页，让 40 × 40 的照片以及
> View、Edit 和 Delete 图标都显示出来。

其中有六个做法值得借鉴。

**每个区域一个连接。**控件的区域属于它的*连接*，
而程序可以把控件移到另一个连接，所以该示例为它提供的每个区域声明
一个连接，并让一个列表在它们之间选择。每个控件的下一次
调用就会去往新的区域：

```cobol
      *> In the region list's onSelectedIndexChanged handler:
           SET     AWS-DDB::Connection           TO    CMB-REGION::GetSelected()
           SET     AWS-S3::Connection            TO    CMB-REGION::GetSelected()
           SET     AWS-REK::Connection           TO    CMB-REGION::GetSelected()
```

`AWS-SET-CREDENTIALS` 的密钥会在循环中交给每一个这样的连接，
而键入它们的文本框在之后马上被清空。

**一张照片的旅程。**照片从不按用户拖入时的样子存储。
弹出窗口返回文件的路径；主窗体把它适配成 400 × 400 的方形，
放进存储桶并存储该条目。表格通过
一个临时链接把它读回来，绝不通过存储桶自己的地址：

```mermaid
flowchart LR
    D["照片拖入<br/>弹出窗口"] --> F["IMAGE-FIT<br/>400 x 400"]
    F --> P["AwsS3 PutObject<br/>photos/id.jpg"]
    P --> I["AwsDynamoDB PutItem<br/>name, race, age, key"]
    I --> S["AwsDynamoDB Scan"]
    S --> L["AwsLambda Invoke<br/>临时链接"]
    L --> H["HTTP-DOWNLOAD<br/>到本地缓存"]
    H --> T["IMAGE-FIT 40 x 40<br/>缩略图"]
    T --> G["DataGrid 的 Image 单元格"]
```

表格首先被填充，带着缩略图将来的路径，而定时器每个滴答获取
一张缩略图；文件尚不存在的 Image 单元格会再次尝试，
所以每幅图片一到达就会出现。

**是一个对象而不是列表的答案。**`GetField` 读取答案的*行*，
而行是列表的一个元素。返回单个
JSON 对象（`{"url": "…", "expires": 900}`）的 Lambda 函数没有行，所以该示例用
`Split` 值方法从答案文本中把
值切出来：

```cobol
           SET     WS-URL                        TO    WS-OUT::Replace( '": "', '":"' )::Split( '"url":"' )(2)::Split( '"' )(1)
```

**等待云端的窗口。**启动一台服务器需要一些时间。
EC2 标签页启动它，用 `OpenFormAsync` 打开一个小的 *Working* 窗体，
并关闭它自己的列表；定时器每两秒询问一次 AWS，当服务器
报告了所要求的状态时，主窗体通过它的
句柄关闭该窗口，并再次列出服务器。用 `OpenFormSync` 打开的
模态窗口不行：它打开期间，打开它的窗体的定时器无法运行。

**图片上的方框。**Rekognition 标签页根据服务返回的
*比例*（`Left`、`Top`、`Width`、`Height`，每个都是图片的 0 到 1）放置每个 `Shape`。
用 `SizeMode = Zoom` 显示的图片保持它的比例，并在一侧
留下空带，所以程序用 `IMAGE-INFO` 读取图片的大小，算出它真正位于
控件内部的什么位置，然后才相乘。

**六种语言的窗口。**关闭按钮旁边的一个列表选择
每段文本的语言，弹出窗口也随之变化。这些文本是工作存储中的一张表
——每段文本一行，每种语言一列，顺序为英语、
葡萄牙语、西班牙语、法语、日语、中文——而一个过程把
选中的那一列复制到具名数据项中，并把它们写入标题、提示、标签页
标题和表格表头：

```cobol
           SET     LBL-DDBHEAD::Caption          TO    T-DDB-HEAD::Trim()
           DG-PETS::SetColumnTitle( "Col-Name", T-COL-NAME::Trim() )
```

带有内容的消息会保留一个占位符，`&1` 或 `&2`，由程序在那里
放入值。显示它的状态行记住的是它显示了*哪条*
消息以及放进去了什么，而不是文本，这样
选择另一种语言时会把这一行重新写一遍，而不是把旧的词留在屏幕上：

```cobol
           SET     WS-DDB-MSG                    TO    M-PETS-COUNT
           SET     WS-DDB-A1                     TO    WS-NUM::Trim()
           SET     LBL-DDBSTATE::Caption         TO    PC-TEXT-NOW(WS-DDB-MSG)::Trim()::Replace( "&1", WS-DDB-A1::Trim() )
```

窗体从不共享数据项，所以弹出窗口无法读取这张表。主
窗体把语言发布为它自己的一个属性，而每个弹出窗口在它打开时
读取它，并在它自己的一张小表中携带它所需的少数文本：

```cobol
      *> In the main form, whenever the language changes:
           INVOKE    ME::SetProperty( "UiLanguage", WS-LANG-IX )

      *> In a popup's onLoad handler:
           INVOKE    super::GetProperty( "UiLanguage" ) RETURNING WS-TEXT
           SET     WS-LANG-IX                    TO    FUNCTION NUMVAL( WS-TEXT )
```

英语这一列恰好是窗体设计时所用的内容，所以程序
打开时是英语，没有翻译任何东西，测试也可以比较两者。

> ⚠️ **注意事项。**
> - 该示例会访问一个真实的 AWS 账户。费用写在该示例的
>   `README.md` 中：空闲时没有任何费用，除非你要求一台演示服务器。
> - 打开弹出窗口的窗口在它打开期间被阻塞，而无头
>   测试无法在其中点击。所以该示例的测试把每个弹出窗口替换成
>   立即应答的窗体，这样真实地检验了主窗体在对话中的
>   那一侧——它设置和读取的属性。
> - *On / off* 列是 Checkbox 单元格：DataGrid 没有看起来
>   像 Switch 控件的单元格。
> - 有些词是控件自己的，不遵循程序语言：DataGrid 在每个表头下绘制的
>   `Filter…` 字段，以及拖放区的 `Drop files here, or browse`
>   行。该示例自己在后者下面的提示是翻译过的。

> **注意——事件是异步控件自己的。**每个 AWS 控件
> 触发它自己的完成事件（`onInvoked`、`onFunctionsListed`、
> `onToolResult`、`onToolsListed`、`onAwsCalled`、`onQueried`、`onKnowledgeBasesListed`、
> `onEventRecorded`、`onRetrieved`、`onTablesListed`、`onRowsAppended`、
> `onJobStarted`、`onJobRun`、`onCrawlerStarted`、`onTableSchema`，以及
> 通用控件的 `onItem`、`onLabels`、`onSignedIn` ……在
> 每个控件的帮助中列出），然后是 `onComplete`；或者 `onError`、
> `onTimeout`（在 `TimeoutMs` 之后；打开托管会话或启动服务器
> 程序有它自己的 `StartTimeoutMs`）或 `onCancelled`（在 `Cancel()` 之后）。每个控件一次
> 只能有一个操作；`IsBusy()` 说明是否有一个正在运行。

> ⚠️ **注意事项。**
> - 未登录的配置文件会以 `onError` 失败，并带有一个 `LastError`，
>   其中指名该配置文件和 `aws login` 命令（对 IAM Identity Center 配置文件则是 `aws sso login`）；
>   AWS 自己的文本绝不会显示给你的用户。用 `AWS-SET-CREDENTIALS` 设置、
>   被 AWS 拒绝的密钥以同样的方式失败，说明密钥被拒绝。
> - **失败绝不会悄无声息。**不管 `Verbose` 怎么说，被拒绝、失败或
>   超时的操作都会把它的原因写入程序的输出——Run Form 之下 IDE 的
>   **Output** 面板——以及 `LastError` 中，所以即使你的窗体在一行标签里
>   把原因截短了，完整的原因仍在那里：
>   `[aws Aws-S3] PutObject failed: …`。
> - `Verbose = true` 把每次调用叙述到程序的输出中：请求，
>   然后是答案或错误，`Sync` 和 `Async` 都一样。任何看起来
>   像 AWS 密钥、会话令牌或签名 URL 的内容，在这里以及在
>   失败行中都会被遮蔽。
> - Run Form、你的应用程序打开的窗体，以及构建出的二进制，都以相同的方式
>   运行 AWS 控件——而没有 AWS 控件的构建出的应用程序不会链接
>   其中任何东西。

---

## 17. 命令行（rcrun）

IDE 所做的一切都可以用 `rcrun` 写成脚本：

```text
rcrun run      <file.cbl> [args…]       # interpret a COBOL source file
rcrun run-form <form.cfrm> <file.cbl>   # run the project's MAIN form in its own GUI window
rcrun check    <file.cbl>               # parse + semantic analysis only (no run)
rcrun build    <file.cbl>               # compile a single console program → bin/<name>
rcrun build    [<name>.project.toml]    # compile a project → one native binary in bin/
rcrun package  [<name>.project.toml]    # package the project into a .zip
rcrun mcp      [--project <path>]       # serve the coding-agent tools over stdio
rcrun backend  <run|describe|check|release|build|auth test> [dir]   # the Backend of a Web or Mobile project
rcrun version                           # print version
rcrun help                              # print usage
```

`rcrun run` 上位于源码路径之后的任何内容都会交给程序本身，
所以程序可以像其他任何命令一样
由 shell 脚本来驱动。

`rcrun build` 和 `rcrun package` 接收项目文件的路径——
`rcrun build MyApp.project.toml`。不带参数时，它们会在当前
文件夹中寻找一个采用较旧的固定名称的项目文件，而今天创建的项目
没有这样的文件，所以它们会以“manifest not found”停止：请指名该文件。

**标志**


| 命令        | 标志                               | 它做什么                                                                                                                                                                                                                    |
| -------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run`, `check` | `--source-format <fmt>`            | `free`（默认）、`fixed`、`fixed-relaxed`、`auto`——见下面的**带入卡片映像源码**                                                                                                                           |
| `run`, `run-form` | `--indexed-engine <name>`, `-I` | ISAM 引擎：`rust`（默认）、`rm-cobol85`、`fujitsu`、`redb`                                                                                                                                                                   |
| `run`          | `--indexed-log <basic|full>`       | 按文件的 INDEXED 事务日志 → `<assign-path>.log`                                                                                                                                                                          |
| `run`          | `--indexed-log-format <text|json>` | 日志行格式；`json` 是用于 Grafana/Loki 的 NDJSON                                                                                                                                                                               |
| `run`          | `--switch <NAME>=<ON|OFF>`         | `SPECIAL-NAMES` 外部开关的初始状态，按其实现者名称指定（可重复）——见**外部开关与用户定义的类**                                                                                   |
| `run-form`     | `--debug`                          | 通过 stdin/stdout 的调试器控制（`@DBG` 行）                                                                                                                                                                               |
| `run-form`     | `--designer`                       | 即使指名的窗体不是主窗体也运行它。IDE 为 **Run Form** 传入它；交付的应用程序从不这样做。它在 stderr 上自我声明，所以以这种方式运行的窗体不会被误认为应用程序正常启动。 |
| `build`        | `--full`, `--clean`                | 丢弃每个缓存的产物并从头重新构建                                                                                                                                                                          |
| `build`        | `--debug`                          | 构建调试器所附着的未优化二进制，安装为 `bin/<name>-debug`，绝不复制到目标文件夹。不带它时二进制是经过优化的——见**Build 与 Debug：你得到哪个二进制**         |
| `build`        | `--quiet`, `-q`                    | 只报告结果，不报告进度                                                                                                                                                                                       |
| `package`      | `--output <path.zip>`              | 覆盖输出压缩包的路径                                                                                                                                                                                                |
| `mcp`          | `--project <file\|folder>`         | 编码智能体工具所作用的项目——它的项目文件，或容纳它的文件夹。不带它时，是 Claude Code 文件夹（`CLAUDE_PROJECT_DIR`，否则是当前文件夹）中的项目，向上搜索。见**与编码智能体协作** |

> **退出码。**当应用程序已损坏——它的主窗体记录互相矛盾——时，
> `rcrun run-form` 返回 **3**；当所要求的窗体不是主窗体时返回 **4**。两者
> 都有别于普通的失败码，所以启动器可以区分被篡改的副本和
> 只是失败了的程序。

> **注意。**当 PowerRustCOBOL 升级之后构建表现异常时，请使用 `rcrun build --full`。
> 生成的源码在每次构建时都会被重写，
> 但 cargo 自己的产物会保留，所以增量构建可能会链接由
> 旧版本产生的对象。完整构建会重新编译 PowerRustCOBOL 自己的部分
> 和你的程序；来自其他厂商的库留在共享
> 缓存中（见[构建结果保存在哪里](#构建结果保存在哪里settings--build-folder)）。

**环境变量**——同样的设置，在 CI 中很方便：


| 变量                   | 它设置什么                                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `COBOLT_LOG`               | 日志过滤器，例如 `warn`、`debug`、`cobolt-runtime=trace`                                                   |
| `COBOLT_SOURCE_FORMAT`     | `--source-format` 的默认值                                                                                  |
| `COBOLT_FIXED`             | 设为 `1` 以强制采用固定格式的源码解析                                                                  |
| `COBOL_INDEXED_ENGINE`     | 与 `--indexed-engine` 相同的选项                                                                             |
| `COBOL_INDEXED_LOG`        | `off`（默认）、`basic`、`full`                                                                              |
| `COBOL_INDEXED_LOG_FORMAT` | `text`（默认）或 `json`                                                                                    |
| `COBOL_SWITCHES`           | `SPECIAL-NAMES` 外部开关，`NAME=ON|OFF`，以逗号分隔——与重复 `--switch NAME=ON` 相同 |
| `COBOLT_AUDIO`             | `null` 让 `AUDIO-…` 内置调用按时钟而不是声卡播放（§11，*播放声音*）；没有对应的标志      |

标志总是优先于它的环境变量。

> 📷 **需要截图 — `rcrun-terminal.png`。**一次终端会话，显示在一个小程序上
> 先 `rcrun check`，再 `rcrun run`，并带有输出。帮助
> 新手看到 CLI 平易近人。

### Backend 命令与编码智能体服务器

`rcrun backend` 运行、检查并构建 Web 或 Mobile 项目的 **Backend**（第 24 章）：

```text
rcrun backend run      [dir]    # serve the Backend for development on a loopback address; prints a development token
        [--listen host:port]    #   default 127.0.0.1:0 (a free port)
        [--page-origin origin]  #   the page's origin; only it may connect (default: any, with a warning)
rcrun backend describe [dir]    # write and print interface.json
rcrun backend check    [dir]    # check the Backend and compare its contract with the released ones
rcrun backend release  [dir]    # record the current contract as released
rcrun backend build    [dir]    # build the headless server → bin/<name>-backend
        [--debug] [--accept-breaking] [--quiet]
rcrun backend auth test [dir]   # test the [identity] provider and reCAPTCHA settings; one line per check, no value printed
```

`rcrun build` 生成**原生二进制**，并拒绝 Web 或 Mobile 项目，同时指出出路：
Web 或 Mobile 项目由 IDE 的 **Build** 按钮构建，或由 `rcrun mcp` 所提供的
编码智能体服务器的 **`build` 工具**构建，它会在 `dist/web/` 下生成 Web 包
（以及 Mobile 项目的 Android 包）。

### 带入卡片映像源码

你在 PowerCOBOL 或 isCOBOL 中编写的源码，很可能采用**经典的
参考格式**——穿孔卡布局，其中 1-6 列保存序列
号，第 7 列是指示符，程序位于第 8 到 72 列，
而 73-80 列保存编译器忽略的程序标记。来自
大型机的文件几乎总是 80 个字符宽，不管是否还有人
把它们当作卡片。

PowerRustCOBOL 自己的项目是**自由格式**——完全没有列规则——而且
这是默认值。所以导入的文件需要被告知它是什么：

```bash
rcrun check --source-format=fixed  PAYROLL.CBL
rcrun run   --source-format=fixed  PAYROLL.CBL
```

这会一次性打开所有列规则，包括续行，而自由
格式对此没有对应物：

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

第 7 列的连字符表示“这一行续接上一行”。对于字面量，
被续接的那一行没有结束引号，而续行
以一个引号重新开始；字面量就是这两半拼接起来。对于字，
两半只是直接相接：

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

⚠️ **不要为自由格式源码传入 `--source-format=fixed`。**这不是
无害的重新解释：第 72 列之后的所有内容都会被丢弃，而你
在前七列中写的任何内容都会被读作序列号和
指示符。写得过长的 `MOVE` 会悄悄丢掉它的尾部。

⚠️ **被续接的字面量只有在 `fixed` 下才字节精确。**规则是，
被续接的片段一直延伸到第 72 列，包括尾随空格——所以止于
第 72 列之前的一行仍然会把这些空格贡献给字面量。没有
第 72 列，就没有可以填充到的地方。

**注意。**如果你想要遵守序列区和指示符列，但*不*
做 72 列的截断——对多年来被重新格式化过的源码
很有用——请使用 `--source-format=fixed-relaxed`。

---

## 18. 构建一个可分发的二进制

`rcrun build`（或 IDE 的 **Build** 按钮）在 `bin/` 中生成一个**单一的、自包含的
原生可执行文件**。应用程序已解析的程序及其窗体都
嵌入在二进制之内；不交付任何 `.cbl` 源码，而且最终用户
**不**安装 PowerRustCOBOL。

```mermaid
flowchart LR
    SRC["src/*.cbl + forms/*.cfrm"] --> COMPILE["rcrun build"]
    COMPILE --> EMBED["解析 · 分析 · 嵌入（压缩）"]
    EMBED --> EXE["bin/yourapp（原生可执行文件）"]
    EXE --> DIST["dist/yourapp（交付文件夹）"]
    ASSETS["assets/ + data/"] -. "一并复制" .-> DIST
```

- 构建会把可执行文件安装到 `bin/` 中，并且——当它是经过优化的
  构建，而绝不是调试构建时——也安装到**目标文件夹**：除非
  项目设置指名了另一个，否则是 `dist/`。你交付的就是那个文件夹。除可执行文件之外，
  它还接收项目的 `assets/` 和 `data/` 文件夹，其布局与它们
  在项目中一样（所以像 `assets/logo.png` 这样存储的路径以相同的
  方式解析）、程序所使用的索引文件的定义，
  以及许可证声明。程序在运行时按相对路径找到这一切。
- 项目自己的 **Knowledge Base/** 文件夹——AI 助手在你开发时
  所读的文档——不会被交付。**KnowledgeBase**
  控件的集合文件位于 `assets/KB` 之下，而构建只把每个
  集合的 `documents/` 文件夹播种到交付物中：索引从不被复制，
  而目标位置已经存在的集合会被保持不动，所以重新构建
  绝不会覆盖你的用户所添加的内容。
- 所需的许可证/声明文件会自动放在二进制旁边。

> ⚠️ **注意事项。**你的应用程序的*最终用户*什么也不安装，但
> **构建**它的机器需要两样东西：Rust 工具链，以及
> 平台自己的源码。构建是真正的编译，而不是导出。随可执行文件一起
> 交付平台 SDK 的 PowerRustCOBOL
> 安装本身就满足这一点；如果你的安装没有，Build 会停下来并指出它搜索过的
> 每个文件夹。请在 **Settings ▸ Platform SDK Location** 下把它指向某个副本，
> 或见 `BUILDING-en.md` 中的*在别处安装 IDE*。

> **注意——构建会链接什么，以及这要付出什么代价。**SQL 桥接
> （`COBOL::"OPEN-DB"` 及其配套调用）会带来 SQLite，而 SQLite 是 C：
> 链接它意味着构建机器还需要一个 **C 编译器**——Windows 上是 Visual Studio Build Tools 中的
> `link.exe`，其他地方是 `build-essential` 或
> Xcode Command Line Tools 中的 `cc`。所以构建会先读取你的程序，
> 只有当其中有东西用到数据库驱动时才链接它们。一个
> 从不打开数据库的程序**只用 Rust**构建。
>
> **这条注意并不能免除你的是什么。**只有在确实要构建某些 C 内容时
> 才需要 C *编译器*。而**链接器**每次都需要，
> 因为每个可执行文件都必须被链接——所以从不打开
> 数据库的程序仍然需要安装平台的构建工具——见第 3 章的*Rust 已安装，但 Build 仍无法完成*。在 Windows 和
> macOS 上，以及在大多数 Linux 发行版上，同一个软件包同时提供两者，
> 这就是两者容易混淆的原因：变化的是 C 编译器，
> 而从不变化的是链接器。
>
> 这种读取倾向于链接，因为猜错的代价是
> 一个在 *Run Form* 下运行正常而只有构建之后才失败的
> 程序。它无法判定的任何东西——动词名存放在数据项而不是
> 引号中的 `CALL`、指名数据库模块的 `EXEC RUST`
> 块——都会链接驱动。你不必声明任何东西；要点只是，
> 普通程序不再为它从不打开的数据库付出代价。
>
> 网络也一样。`COBOL::"HTTP-…"` 会访问操作
> 系统的 TLS 栈，在 **Linux** 上那是 OpenSSL——又一个 C 库，
> 又一个需要安装的开发包。不调用任何 HTTP 动词的控制台程序
> 构建时没有它。Maps 客户端又是单独的，并且
> 在你项目中的某个窗体确实带有 **Maps** 或 **WebSearch**
> 控件时才会被链接；没有这样的控件的项目不为它付出代价。
>
> ⚠️ **窗体**应用程序始终会链接 TLS，不管它的 COBOL 做什么：
> 地图底图由 Form Designer 自己的渲染器通过 HTTPS 获取，所以
> 这个栈无论如何都在那里。在 Linux 上，这意味着窗体应用程序仍然需要
> 系统的 TLS 开发包。只有**控制台**程序才能
> 只用 Rust 构建。

### Build 与 Debug：你得到哪个二进制

你按下的按钮决定你的应用程序如何编译。项目没有
对它的设置。

| 你按下 | 你得到 | 它安装在哪里 |
|---|---|---|
| **Build**，或 `rcrun build` | **经过优化的**二进制——用于从命令行运行以及交付的那个 | `bin/<name>`，以及目标文件夹 |
| **Debug**，或 `rcrun build --debug` | **未优化的**二进制，构建出来是为了让调试器可以附着到它 | `bin/<name>-debug`——别无他处 |
| **Run Form** | 不构建。窗体在 IDE 自己经过优化的运行时上运行，它的窗口就是你设计的那个 | — |

有一件事让 Run Form 终究要构建：程序包含 `EXEC RUST`，
其块只存在于构建出的应用程序中。这时应用程序会被构建——
经过优化——并运行。没有块的窗体从不等待构建。

**Debug 先构建。**在项目中，对某个窗体按 **Debug** 会不经优化地编译
应用程序，启动它，并把调试器附着到它上，停在第 1 行。（按钮
本身只有在项目被你正在运行的 PowerRustCOBOL 构建过之后才会可用——见下面的**调试**——
而它随后所做的调试构建是它自己的，不是 Build 按钮的。）第一次调试
构建编译所有内容，需要一段时间；之后的构建只重新编译变化了的内容。不属于
某个项目的窗体没有什么可构建的，所以它的 Debug 仍然在 IDE 的运行时上运行。

> ⚠️ **注意事项——未优化的程序更慢，这是有意的设计。**Debug 的存在是为了
> 被单步执行，而不是为了快。在
> PowerAnalytics 示例（一个 43,000 行的数据集被加载进程序自己的
> 数据库）上测得，同一个应用程序启动时，经过优化为 1.0 秒，未优化为
> 3.0 秒，而应用过滤器并切换页面则是 2.2 秒对 12.5 秒。
> 如果构建出的程序感觉很慢，请检查你运行的是哪个文件：
> `bin/<name>-debug` 是调试构建，`bin/<name>` 和目标
> 文件夹中的副本是经过优化的那个。

> **注意——调试构建绝不会取代经过优化的构建。**它在 `bin/` 中有自己的名称，
> 并且不会被复制到目标文件夹，所以调试不会
> 把慢的程序留在你所交付的程序应在的位置。

> **注意——较旧的项目。**早先的版本在项目中保留了一个 *Debug Compilation* 设置，
> 新项目默认打开它，所以 Build 按钮悄悄地
> 生成了慢的二进制。这个设置已经没有了。较旧
> 项目文件中的 `debug_compilation` 行会被忽略，下一次 Build 是经过优化的。
### 构建结果保存在哪里（Settings ▸ Build Folder）

构建是真正的编译，而编译需要一个工作的地方。除了落在 `bin/` 中的
可执行文件（以及交付文件夹）之外，每个程序还有一个名为 `cobolt-build-<name>` 的
**工作文件夹**——为它生成的文件。它在两次构建之间被保留。已经*编译*好的内容被分开保存，
放在为你所有程序共用的一个**共享
缓存**中（下文描述），这就是为什么一个未更改的程序第二次 Build 很快，
以及为什么新程序不是从零开始。
两者都不属于你的项目，你也从不需要交付它们。

在你另行指定之前，它位于系统的每用户缓存文件夹中：

| 系统 | 所有构建的默认文件夹 |
|--------|-------------------------------|
| Windows | `%LOCALAPPDATA%\PowerRustCOBOL\builds`（通常是 `C:\Users\<you>\AppData\Local\PowerRustCOBOL\builds`） |
| macOS | `~/Library/Caches/PowerRustCOBOL/builds` |
| Linux | `$XDG_CACHE_HOME/PowerRustCOBOL/builds`，否则 `~/.cache/PowerRustCOBOL/builds` |

它是缓存文件夹而不是系统的临时文件夹，这是有意的：
系统会清理它的临时文件夹，包括编译仍然依赖的文件，
于是下一次 Build 会因为一个与你的程序毫无关系的原因而失败。

**为你所有程序共用的一个缓存。**在构建文件夹之内，工作
文件夹旁边，有一个名为 `shared-target` 的文件夹。它保存所有
已经编译的内容：PowerRustCOBOL 自己的部分，以及每个应用程序
都需要的来自其他厂商的库。一个部分只编译一次，并被每个
需要它的程序使用。所以一台机器上的第一次 Build 是漫长的那一次；你的
第二个程序的第一次 Build 除了程序本身几乎不编译什么；而新的
PowerRustCOBOL 只重新编译它更改过的部分。如果你删除
`shared-target`，什么也不会丢失——下一次 Build 会把所有内容再编译一遍。

在同一时刻从两个窗口或两个终端启动的两次 Build，会
轮流使用共享缓存。等待的那一个会在 Output 面板中这样说明
（*Another build is using the compiled-crate cache — waiting for it to finish…*），
并在另一个完成后继续。如果共享缓存完全无法使用——
比如文件夹无法创建——Build 会说明原因，并像旧版本一贯那样，
在程序自己的工作文件夹中编译；它更慢，但绝不会出错。

要把构建保存在别处——更大的驱动器、备份工具会跳过的文件夹——
请打开 **Settings ▸ Build Folder…**。窗口显示当前生效的文件夹以及
它的来源（在这里选定、由环境设置，还是系统的默认值），
并提供 **Choose folder…** 和 **Use the default**。这个选择属于
**这台机器**：它与 IDE 自己的偏好设置一起存储，而不在项目中，
所以打开同一个项目的同事绝不会被指向一个只存在于你磁盘上的路径。

在没有人打开 IDE 的机器上——运行 `rcrun build` 的构建服务器——
请改为把环境变量 `POWERRUSTCOBOL_BUILD_DIR` 设为绝对路径。
两者同时存在时，在 IDE 中选择的文件夹优先；为空或不是绝对路径的值会被忽略，
而默认值适用。

```text
POWERRUSTCOBOL_BUILD_DIR=/data/builds rcrun build myapp.project.toml
```

> **注意。**所选的文件夹保存你构建的*每个*程序的工作文件夹，每个
> 一个 `cobolt-build-<name>`，以及它们旁边的共享缓存。新的
> 文件夹两者都没有，所以更改文件夹之后的第一次 Build 会
> 把所有内容再编译一遍，就像你的第一次 Build 那样。

> ⚠️ **注意事项。**
> - 你离开的那个文件夹**不会**被移动或删除。它的内容只是
>   缓存，所以在不再需要它时请自己删除；共享缓存
>   可能占用几个千兆字节。
> - 由较旧的 PowerRustCOBOL 构建的程序，在它的工作文件夹内仍然带有自己编译的文件。
>   第一次使用共享缓存的 Build 会移除
>   它们，并且 Output 面板会说明这释放了多少空间。
> - IDE 会拒绝它无法创建或写入的文件夹，说明原因，并保留
>   先前的设置。本地驱动器上的文件夹是安全的选择：编译
>   会写入大量小文件，而缓慢的网络共享会让每次 Build 也变慢。
> - 已经在进行中的 Build 保留它启动时所用的文件夹；下一次则使用
>   新的文件夹。

<!-- 📷 build-folder.png — Settings ▸ Build Folder… open: the "In use" line, the
     source line, the two buttons. Capture it once with the default, and once after
     choosing a folder (the source line then says "This folder was chosen here."). -->

### 完整构建与记录的版本

项目会记录最近一次**完整**构建它的 PowerRustCOBOL 版本。当
你打开一个最近一次由**较旧的** PowerRustCOBOL 完整构建的项目——或者
从未被完整构建过的项目——时，**Build** 按钮会执行**完整**
构建：程序的工作文件夹会先被丢弃，PowerRustCOBOL 自己的
部分和你的程序都会从源码重新编译，所以较旧版本产生的任何东西
都不可能留存到新的可执行文件中。来自其他
厂商的库不会被重新编译——它们留在共享缓存中，因为关于 PowerRustCOBOL 的
任何事情都不可能改变它们。它比普通构建花的时间更长，而
Output 面板会说明它为什么这样做。

那次构建一成功，版本就被记录下来，之后的 Build 点击又是
普通的增量构建——长的那次构建**每次升级只发生一次**，
而不是每次点击一次。在仍然需要一次完整构建的项目上按 **Run**，
会在启动任何东西之前向你提供同样的完整构建。

Build 也可以自己变成完整构建。每次 Build 之前，PowerRustCOBOL
会把它自己已安装的每个部分，与上一次在共享缓存中编译的副本进行比较。
如果有一个不同，而它的文件看起来又不比缓存的
记录更新，这次 Build 就会成为完整构建，并且 Output 面板会指出是哪个部分。
当 PowerRustCOBOL 是从较旧的副本放回去的时候就会这样：它的文件带着较旧的
日期，这会骗过编译器去重用从不同源码编译出来的代码。
你什么也不用做；每次这样的更改只会发生一次。

在命令行中：

```text
rcrun build --full  [<name>.project.toml]   # recompile our parts and the program from source, then build
rcrun build --clean [<name>.project.toml]   # same thing, spelled the other way
```

> ⚠️ **注意事项。**只有完整构建才会更新记录的版本，而这是有意为之：
> 普通的增量构建无法保证由先前版本编译的
> 任何东西都没有仍然链接进结果中。

> **注意。**窗体在二进制内部是**延迟**加载的：一个有 20 个窗体的应用程序
> 即使用户只打开一个窗体，也会立即启动。

<!-- 📷 everopen.png — a built application starting and opening one form, showing
     that the other forms cost nothing until they are asked for. -->
<p align="center"><img src="../assets/animations/everopen.png" alt="一个构建出的应用程序即时启动并打开单个窗体" width="900"></p>

### “Powered by PowerRustCOBOL”徽标

如果你交付一个用 PowerRustCOBOL 构建的应用程序，请把 **“Powered by
PowerRustCOBOL”** 徽标添加到你应用程序的**关于框**中（也可以选择加入你的 README）：

<p align="center">
  <img src="../assets/images/made-with-powerrustcobol.png" alt="Powered by PowerRustCOBOL" width="320">
</p>

- 标准徽标：`assets/images/made-with-powerrustcobol.png`（800×268，透明）。
- 高分辨率母版（用于印刷或大型显示器）：`assets/images/made-with-powerrustcobol.webp`
  （6785×2270）——请把它缩小到你需要的任何尺寸。

IDE 自己的 **Help ▸ About** 框显示同样的徽标，所以你可以确切看到
它在应用程序中是什么样子。

### 构建 Web 或 Mobile 应用程序

在 Web 或 Mobile 项目上，**Build** 按钮不会生成原生可执行文件。它生成：

| 项目类型 | Build 生成 | 位置 |
|---|---|---|
| Web | 任何 Web 服务器都可以托管的静态 WebAssembly 包 | `dist/web/` |
| Mobile，Android | 先是包，然后是可安装的软件包 | `dist/mobile/android/<Name>-debug.apk` |
| Mobile，iOS（在装有 Xcode 16 或更高版本的 Mac 上） | iOS Simulator 所运行的应用程序 | `dist/mobile/ios/<Name>-debug.app` |
| 两者之一的 Backend | 无头服务器，用 `rcrun backend build` | `bin/<name>-backend` |

Web 构建不需要 Rust 工具链——IDE 会把预构建的播放器复制到你的程序旁边。
Backend 的发布二进制需要与桌面 Build 相同的工具链。第 23 章描述
Web 包和 Build，第 24 章描述 Backend，第 25 章描述 Mobile 软件包。

---

## 19. 调试

按 **Debug** 开始一次会话。在项目中，该按钮在**一次
Build 之后**才可用——也就是项目被你正在运行的 PowerRustCOBOL
构建过之后（它变灰时工具提示会这样说；由较旧
版本构建的项目需要先做一次新的 Build）。会话调试什么取决于
打开的是什么：编辑器中的 Common Code 文件被当作程序来调试；窗体的
生成程序（一个 Generated Code 条目）被路由到它的窗体，该窗体作为
真正的窗口启动；而什么都没有打开时，它调试项目的**主窗体**，
也就是 **Run** 会启动的那个窗体，调试器附着在它的窗口上。
你会得到：

- 编辑器边槽中**以及调试器窗口自己的
  边槽中**的**断点**——在任何一处，会话之前或会话期间，都可以点击任意一行旁边。你在程序停止时
  设置、移动或清除的断点会在
  下一条语句立即生效；你不必为了改变主意而重新启动。
- **单步**控制和**继续**——程序停止时，**F5** 继续，**F10** 单步跳过，
  **F11** 单步进入，**Shift+F11** 单步跳出，
- **变量监视**面板，
- **源码清单**：生成的程序，每行一行，每行的高度
  与代码字体一样高——调试器工具栏中的 **A−** 和 **A+** 按钮
  同时更改字体和行高——停止的那一行带有色带，
- **Only my code**，默认开启：单步直接穿过生成的
  脚手架——首先是事件循环——并且只在你写的处理程序和
  过程中停止。当你想观察这套机制时，请在调试器工具栏中把它关掉。
  断点从不被它过滤：你在生成的行上设置的断点
  仍然会在那里停止，因为设置它是你的决定。
- **Pause**，它对空闲的窗体也有效：在窗体坐等
  你点击时按下它，程序会停在它**最后执行的那一行**——
  开启 *Only my code* 时，是你自己处理程序的最后一行，
  而不是事件循环。Continue 让窗体回到等待状态。
- **Animate**，它按滑块上的速度替你单步——并且在某一步落在**断点**上的
  那一刻停止：开关会自己
  关闭，程序等待你，就像你手动
  单步时那样。**Pause** 随时停止动画，并且在动画期间始终
  可以点击；再次按 **Animate** 继续，或者用 **Step Into** /
  **Step Over** 手动接管。

会话期间会出现一个 *Stop Debug* 控件；否则调试从
工具栏的 **Debug** 按钮开始，它位于 **Run** 的右边。

> **注意——Breakpoints 列表是整个项目的。**调试器的
> **Breakpoints** 列表显示你在项目中任何地方设置的每个断点，
> 而不只是屏幕上清单中的那些：每个 Common Code 文件的，
> 归在文件之下，以及每个事件处理程序的，归在
> `Form ▸ Control ▸ event` 之下，每个都带有它的行和它所在的语句。
> 一行旁边的 ✕ 会清除那个断点，不管它在哪里。你
> 在处理程序自己的编辑器中设置的断点，在你关闭那个编辑器——
> 以及关闭窗体的设计器——时仍然有效，而每个处理程序的断点都会
> 停下窗体，包括应用程序所打开的第二个窗体的处理程序。
> 断点持续整个 IDE 会话。

> ⚠️ **要在事件处理程序内部停止，请调试窗体——而不是它生成的
> `.cbl`。**在窗体上按 **Debug** 会把它作为真正的窗口启动，所以它的
> 处理程序真的会运行，你在其中的断点也会到达。从编辑器中对
> 生成的文件按 **Debug**，会在没有窗口附着的情况下运行程序：
> `COBOL::"WAIT-EVENT"` 找不到可以等待的窗体，立即结束事件
> 循环，并且从不派发任何处理程序——所以其中的断点
> 无论设置得多么正确，都永远不会被经过。

### 调试由多个窗体组成的应用程序

应用程序很少只有一个窗体。当你的主窗体打开另一个——子
窗口、模态的 `OpenFormSync`，或显示在 **SideMenu**
内容窗格中的窗体——时，那个窗体运行的是**它自己的程序**，而调试器会跟随
你进入它。

你不需要做任何安排。在被调用窗体的生成程序中放一个断点，
从你按 **Debug** 的那个窗体开始会话，
然后像平常那样操作应用程序。当被调用窗体的
处理程序到达你的断点时，调试器会调出**那个窗体**的
代码，定位在你的那一行，带着它自己的断点和它自己的折叠。
调用者的清单不会丢失——稍后单步回到它，或者从子窗体
继续出来，它就和你离开时一模一样。Continue、各个单步和
Pause 总是作用于你正在看的那个窗体。

每个窗体都有自己的程序，由此带来几点值得知道的事情：

- **断点属于一个文件。**两个窗体生成程序中的第 42 行
  是两个不同的断点。在调用者中设置一个绝不会
  停下被调用的窗体，反之亦然。
- **停止会停下整个应用程序。**每个窗口都安静下来——没有
  哪个会接受点击——直到你继续。这是有意的：停下来的
  是你的程序，而你的程序就是它们全部。
- **窗体只在你于其中工作时才产生事件。**其他窗体
  待在 `COBOL::"WAIT-EVENT"` 之内等待，和你
  不在调试时完全一样。
- **两个窗体可以同时被停住。**应用程序暂停期间，Timer 仍然继续滴答，
  所以第一个窗体被停住时，第二个窗体可以到达它自己的断点。
  调试器显示最近的那个，当你继续它时，会调出仍在
  等待的那个。
- **Only my code** 是一个开关，但它读取每个窗体自己的行——所以
  单步经过被调用的窗体，跟随的是*它的*处理程序，而不是调用者的。
- **Step Into 不会进入被调用的窗体。**在 `OpenFormSync` 行上按 **F11**
  会打开被调用的窗体并让它自由运行；它只
  在它自己的断点处停止。当你关闭它——它的 Ok 或 Cancel——时，
  控制权回到调用者，而因为你在单步，调用者
  会停在 `OpenFormSync` 之后的那一行，准备读取结果。
  要同时跟随被调用窗体的处理程序，请在它的程序中放一个断点。

> ⚠️ **调试器无法定位的窗体。**如果窗体的 `.cfrm` 不在
> 打开的项目中，或者它的程序尚未生成，调试器会在 Output 面板中
> 这样说明，并让清单保持原位。它不会向你
> 展示另一个文件的行并称之为那个窗体的。

> 📷 **需要截图 — `debugger.png`。**一次调试会话停在
> 断点上，变量监视面板中有内容。

### 诊断开关（Settings ▸ Debug Settings）

有些故障在 IDE 叙述它正在做什么时要容易找到得多。
**Settings ▸ Debug Settings** 把每一个这样的开关汇集到一个模态窗口中，分成
五个标签页：**User Interface**、**Data Binding**、**Events**、**Indexed Files**
和 **Logging**。

它们是**机器设置，不是项目数据**——保存在 IDE 自己的设置
文件夹中，从不写入项目文件。所以它们跟着你从一个项目到另一个项目，
绝不会随提交传给同事，而且即使没有加载项目，该模态窗口也能打开。


| 标签页                | 开关                       | 它给你什么                                                                                                                                                                                                    |
| ------------------ | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **User Interface** | Frame diagnostics overlay    | 把每个控件的图层——阴影、表面、边框、内容、轮廓——展开成彩色、错位的框架。角上的瑕疵或圆角错误的图层就是这样变得可见的                                          |
| **User Interface** | DataGrid component frames    | 勾勒出 DataGrid 的每个内部部分——表头、主体、每一列、每个可见的行和单元格、冻结窗格、滚动条——每个都用自己的颜色                                                                   |
| **User Interface** | Disable window effects       | 跳过每个窗口的进入和退出效果，而无需编辑任何项目或窗体——适用于对动态敏感、GPU 较弱，或自动化                                                                                                             |
| **Data Binding**   | Data-bind trace              | 写入 `databinding.log`：重复组的播种和按行的控件数组绑定                                                                                                                                  |
| **Events**         | Event trace                  | 在通道的**两端**每个 UI 事件一行——发送它的宿主和派发它的解释器——与你自己的 `DISPLAY` 输出一起交错写入 `prc-event-trace.log`                                  |
| **Indexed Files**  | Transaction log, and format  | 第 14 章所述的按文件的 INDEXED 操作日志，以及每行是以 logfmt 文本还是以 JSON 写入                                                                                                      |
| **Logging**        | Tracing filter               | 运行时自己的跟踪过滤器——`warn`，或更窄的如 `cobolt-runtime=trace`                                                                                                                          |

跟踪文件落在机器的临时文件夹中：macOS 和
Linux 上是 `/tmp`，Windows 上是 `%TEMP%`。

**任何诊断都绝不会写入密钥。**诊断转储和实时跟踪
只显示这些内容存在以及它们有多长，绝不显示它们本身：
- 带有 `PasswordCharacter` 的文本框的文本；
- 每个以凭据命名的属性（`ApiKey`、`AgentAPIKey`、`AuthToken`、
  `…Password`、`…Secret`，……）。

任何其他值内部看起来像凭据的内容——AWS 密钥 id、秘密
密钥、会话令牌、签名 URL——都会在它所在的地方被遮蔽。

对于**事件跟踪**，物有所值的是这些行的*顺序*。它把
因事件被投递两次而运行了两次的处理程序，与在一次投递中运行了两次的处理程序区分开——
这两种故障从处理程序内部看来是一模一样的。

开关**立即**生效：设计画布在下一帧拾取它，
而 **Run Form** 在窗体的进程启动时得到它，所以你从不需要重启 IDE
来改变主意。只要任何诊断开着，IDE 还会
写出一份以项目命名的按控件诊断转储。

> **注意。**每个开关都对应运行时一直读取的一个环境变量，
> 所以独立的 `rcrun` 运行仍然遵守你手工导出的那个。这个
> 模态窗口是通往它们的更友好的入口，而不是对它们的取代。

> **注意。****User Interface** 标签页还带有一个绑定到 F12 的截屏开关。
> 那是用来制作本文档图像的创作工具，而不是
> 你所构建的应用程序会需要的东西。

---
## 20. 外观与国际化

- **主题。**⚙ ▸ *Settings* 提供 41 种配色主题，并在你到达每一个时
  把它显示出来：打开列表并按 ↑ 或 ↓，IDE 就会采用
  光标下的主题，不必等待 Enter（Enter 保留它；窗体上的 **Cancel**
  会恢复已保存的那个）——深色（Dark Glass
  [默认]、Deep Blue、Dark+、Monokai、Solarized Dark、Nord、Dracula，以及
  更多）、浅色（Light+、GitHub Light、One Light、Gruvbox Light、Ayu Light、
  Quiet Light、Tomorrow、Material Lighter、Nord Light、Rosé Pine Dawn、
  Catppuccin Latte、Solarized Light）、**Classic**，一种忠实的 Windows
  95 外观（银色外框、藏青色选择），带来完整的复古 RAD 体验，
  三种 **Neumorphic** 调色板——Light、Dark 和 Cobalt——其柔和的
  浮雕与 Neumorphic 窗体样式相匹配，以及 **Silver Glass**，同样的
  柔性界面构造，采用磨砂银色：略带透明的面板，让
  背景图像透过来形成磨砂效果，边缘是清晰的白色，文字是深色。
  **Aurora Pastel** 保持相同的窗格在相同的位置，但绘制
  方式不同：柔和的蓝色、薄荷色和薰衣草色渐变背景，底部有两道
  波浪，每个窗格都是半透明的粉彩卡片（项目树是薄荷色，智能体栏是奶油黄，
  主窗格是天蓝色，Output 是薰衣草色），其边框在角上发光，凸起的工具栏按钮
  带有各自的颜色，以及粉彩的检查器标签页。它的所有文字在
  其所在的窗格上都达到 7:1 的对比度。**Aurora Prime** 是同样的设计，
  但采用鲜活的颜色而不是粉彩：青绿、翠绿、靛紫、梅红和焦橙，
  绝不用纯红、纯黄或纯蓝。背景、角上的光芒、
  工具栏按钮和标签页承载浓烈的颜色，而窗格
  保持足够浅，让深色文字读起来同样清楚。
  五个 **Slick** 主题使用两种颜色加白色，没有任何粉彩：白色
  卡片置于白到浅灰的渐变背景上，一种深色用于文字、外框
  和实心的 Build 按钮，一种强调色用于 Run 按钮、链接和
  角上的光芒。它们是 **Navy & Teal**、**Graphite & Indigo**、
  **Slate & Coral**、**Charcoal & Emerald** 和 **Piano & Coral**（光泽的
  黑色配深珊瑚色）。
  **Spatial** 是一种深色主题，具有空间计算界面的风格：
  暖灰色的磨砂玻璃窗格漂浮在昏暗的房间之上，白色文字，大的
  圆角，胶囊形的按钮，没有饱和的颜色。光线
  落在每个窗格的上角，选中的标签页是明亮的
  玻璃。Run 是唯一的白色按钮，所以它从灰色按钮中脱颖而出。
  在 Spatial 下，IDE 窗口约 50 % 透明：在 macOS 上，
  它背后的桌面会透过来，被模糊，就像房间透过
  漂浮的面板显示出来一样。在无法模糊窗口的系统上，它显示为未模糊的。
  白色文字在深色或色彩丰富的壁纸上读起来很清楚；在非常
  明亮的壁纸上它会变淡，所以请在较暗的桌面上选择 Spatial。Form
  Designer、调试器以及 IDE 打开的其他窗口
  以同样的方式透明并模糊。项目背景图像
  会取代主窗口中的透视图。
  还有一个可选的**背景图像**，带有不透明度控制。
  在 **Settings → Appearance** 中，**Browse…** 选择图片，**Clear**
  把它拿掉。两者都立即作用于窗口，并立即写入
  项目文件——都不等待 **Save**，也都不会被 **Cancel** 撤销
  （**Cancel** 只会丢弃你键入但尚未保存的内容）。它们下面的
  不透明度滑块则与其余设置一起保存。
  在任一种 Aurora 主题下，图像会取代渐变背景。
  设置是**按项目**保存的，保存在项目文件中。项目树和
  面板文字会自动让它们的对比度适应主题——深色主题上是浅色文字，
  浅色主题上是深色文字。
- **IDE 语言。**IDE 界面提供**六**种语言——
  **English、Português、Español、Français、日本語 和 中文**（选择器用
  每种语言自己的名称来命名）——从工具栏的语言选择器中选择。CJK
  字形通过内置的后备字体渲染，所以 日本語 / 中文 在
  任何系统上都能正确显示。IDE 自己写出的消息——状态行、
  **Output** 面板的备注（“Project saved”、“Deleted form …”、“Build blocked …”）
  和错误对话框——遵循同一种语言并随之变化，代码编辑器补全列表中的
  单行说明（`CTRL::` 之后每个方法旁边的那句话）以及 Debug
  settings 窗口也是如此。编译器和构建工具所报告的内容，
  以及你自己的程序用 `DISPLAY` 打印的内容，则不会被翻译。
- **文本渲染。**IDE 用 UI 框架的现代字体
  引擎渲染文本（启用了 hinting），所以字形在小字号下比
  早先的版本明显更清晰。窗体的 **Font** 属性仍然和以前完全一样地工作：
  引擎无法栅格化的字体（例如仅有位图的系统字体）
  会被跳过，而控件回退到 Arial，而不是失败。
- **品牌。**IDE 的窗口/任务栏使用 PowerRustCOBOL 图标
  （用配置目录中的 `app-icon.png` 覆盖它）。**Help ▸ About**
  显示吉祥物、版本，以及 Apache-2.0 许可证。

> ⚠️ **关键规则。**IDE 语言只翻译**界面**。你的
> **COBOL 数据名、段落名以及所有生成的 COBOL 源码都保持为
> 英语**，不管选择了什么界面语言。这让代码在各团队之间保持可移植
> 且可评审。

---

## 21. COBOL Structure 与共享数据

窗体模块不只是它的控件和事件处理程序——它是一个真正的 COBOL
程序，带有 `ENVIRONMENT DIVISION` 和 `DATA DIVISION`。**COBOL
Structure** 编辑器让你直接编写这些共享部分，而运行时
为你提供忠实于 COBOL 的 `GLOBAL` / `EXTERNAL` 数据共享，跨越模块
和运行单元。

### 编辑器

选中窗体本身（点击空白画布，或窗体节点），然后在属性检查器中打开
**COBOL Structure** 部分。它列出五个共享块，每个都按正确的
division/section 顺序原样织入生成的程序中：


| 块             | 放入             | 用于                                                                                        |
| ----------------- | --------------------- | ------------------------------------------------------------------------------------------------- |
| `SPECIAL-NAMES`   | CONFIGURATION SECTION | `DECIMAL-POINT IS COMMA`、助记名、货币符号、外部开关、用户定义的类 |
| `REPOSITORY`      | CONFIGURATION SECTION | 类名——Rust-FFI 类型桥接（见下文）                                               |
| `FILE-CONTROL`    | INPUT-OUTPUT SECTION  | 窗体所打开文件的 `SELECT … ASSIGN`                                                       |
| `FILE SECTION`    | DATA DIVISION         | 这些文件的 `FD`                                                                          |
| `WORKING-STORAGE` | DATA DIVISION         | 窗体的共享数据项                                                                      |

点击一行会打开一个编辑**那一个块**的弹出窗口。代码框以
大约十二行打开，并且只在你拖动它右下角的
手柄时才改变大小——窗口跟随该框，而两者都不会
自己增长，不管块有多长。

用户过程在检查器中有它们自己的一个标签页，**User
procedures**，位于 **Events** 之后——不论选中哪个控件，列表都一样，
因为这些过程属于窗体。它们按**名称 A–Z** 列出；
**➕ Add procedure** 创建一个，**Open** 在同样的弹出窗口中编辑它的名称和主体，
而 🗑 移除它。一旦窗体有了过程，列表上方的搜索框会把它
缩小到包含你所键入内容的名称（忽略大小写）。
对列表排序不会重新排列生成程序中的过程：
它们仍然按创建的顺序写出，和以前一样。每次编辑都会把
窗体标记为已更改，所以下一次 **Build / Run / Debug / Check** 会用你的更改重新生成 `.cbl`。

### 外部开关与用户定义的类

`SPECIAL-NAMES` 带有两项 COBOL-85 设施，你在桌面上可能不需要，
但程序一旦必须针对特定的一次运行而表现不同——夜间
批处理、试运行、针对某位客户的一次处理——你就会想要它们。

**开关是从程序外部设置的运行时标志。**你声明
实现者的开关名、它的助记名，以及每个状态的条件名：

```cobol
       SPECIAL-NAMES.
           SWITCH-1 IS SW-REPRINT
               ON  STATUS IS REPRINTING
               OFF STATUS IS NOT-REPRINTING.
```

然后像测试任何其他条件名那样测试它，并在需要时从程序中设置它：

```cobol
           IF  REPRINTING
               PERFORM RE-PRINT-INVOICES
           ELSE
               PERFORM PRINT-NEW-INVOICES.

           SET SW-REPRINT TO OFF.
```

**COBOL 内部没有任何东西可以在运行开始之前设置开关**——这正是
它的全部意义——所以 `rcrun` 从命令行或环境中获取初始状态，按*实现者*
名称（助记名也可以）来指定：

```bash
rcrun run invoices.cbl --switch SWITCH-1=ON
```

```bash
COBOL_SWITCHES=SWITCH-1=ON,SWITCH-2=OFF rcrun run invoices.cbl
```

`--switch` 可以重复；`ON`/`1`/`TRUE`/`YES` 和 `OFF`/`0`/`FALSE`/`NO` 都
被接受。没有人设置的开关一开始是**关**的。

**类命名一组字符**，之后你可以用它测试某个数据项，这
省去了在五个地方写同一串 `OR` 的麻烦：

```cobol
       SPECIAL-NAMES.
           CLASS VALID-GRADE  IS "A" THRU "D" "F"
           CLASS HEX-DIGIT    IS "0" THRU "9" "A" THRU "F".
```

```cobol
           IF  WS-GRADE IS VALID-GRADE
               PERFORM RECORD-GRADE.

           IF  WS-TOKEN IS NOT HEX-DIGIT
               MOVE "BAD CHECKSUM" TO WS-ERROR.
```

数据项的**每个**字符都必须属于该类，测试才为
真——与内置的 `NUMERIC` 和 `ALPHABETIC`
测试所遵循的全字符规则相同。`IS` 是可选的，与内置的类测试一样。

> ⚠️ 类名是一个*类*，不是数据项：它没有存储，不能
> 被传入或传出，并且只会出现在条件中的 `IS [NOT]` 之后。
### 命名控制台：助记设备名

`SPECIAL-NAMES` 做的第三件事，是给操作员的终端取一个你自己的
名字，这样程序的其余部分通过那个名字读写，而不是
内联地指名设备：

```cobol
       SPECIAL-NAMES.
           CONSOLE IS OPERATOR-CONSOLE.
```

```cobol
           DISPLAY "ENTER THE RUN DATE (YYYYMMDD):"
                                   UPON OPERATOR-CONSOLE.
           ACCEPT  WS-RUN-DATE     FROM OPERATOR-CONSOLE.
```

`ACCEPT … FROM <mnemonic>` 是 **Format 1**——与光秃秃的
`ACCEPT WS-RUN-DATE` 所做的完全一样。它从操作员那里读取一行，这一行
被铺在接收数据项上：组接收项按各从属数据项的宽度被切分，
而比数据项短的一行会被空格填满到末尾。`IS` 是可选的，与
`SPECIAL-NAMES` 中其他任何地方一样。

这是大型机和验证套件 COBOL 到处使用的形态，而且
即使你只有一个控制台，也值得给它命名：如果程序以后由别处驱动，
助记名就是唯一需要更改的地方。

> **注意——助记名和环境变量是不同的来源。**
> PowerRustCOBOL 还允许 `ACCEPT id FROM SOME-NAME` 读取**环境
> 变量** `SOME-NAME`，这是一个扩展，而不是 COBOL-85。
> 声明决定你得到哪一种：`SPECIAL-NAMES` 声明过的名称读取
> 操作员，它没有声明的名称读取环境。所以声明
> 助记名才使这次读取成为标准的——而如果某个 `ACCEPT` 意外地
> 什么也没返回，请在查看别处之前先检查该名称是否已声明。

### 右对齐接收项与编辑型字母数字字段

PowerCOBOL 开发者在报表行上会用到的两项 `PICTURE` 级别的设施：

```cobol
       01  WS-RIGHT      PIC X(10) JUSTIFIED RIGHT.
       01  WS-NAME       PIC A(5)  JUSTIFIED RIGHT.
       01  WS-PART-NO    PIC XXBXX/XX.
```

`JUSTIFIED RIGHT` 颠倒字母数字接收项的对齐规则：较短的
发送项在**左边**补位，较长的则丢掉它**最左边**的
字符。`MOVE "AB" TO WS-RIGHT` 留下 `"        AB"`。

该子句以完全相同的方式适用于**字母**（`PIC A`）接收项。
`MOVE "ABC" TO WS-NAME` 留下 `"  ABC"`，而把十五个
字符 `"ABCDEFGHIJKLMNO"` 移入则留下 `"KLMNO"`——*右*端保留了下来，
这与未对齐的数据项所做的相反。

> ⚠️ 丢掉最左边的字符是让人意外的部分。在
> 普通数据项上，过大的发送项会在右边被截断，所以被截断的账号
> 仍然以正确的数字开头；而在 `JUSTIFIED` 的数据项上，它则以
> 它们结尾。请按你预期的最宽发送项来确定接收项的大小。

**字母数字编辑型** picture 拥有它自己的插入字符——`B` 打印一个
空格，`0` 打印一个零，`/` 打印一个斜杠——而发送项只填充 `X`、`A` 和 `9`
位置。`MOVE "AB12CD" TO WS-PART-NO` 得到 `"AB 12/CD"`。移入空格
会让插入字符留在原位（`"     /  "`），这就是 `INITIALIZE` 对
这样的字段所做的。

### 组操作数会暂停接收项的 PICTURE

这条规则最常用来解释一个“没做任何合理事情”的 `MOVE`。
当 `MOVE` 的**任一**操作数是组数据项时，标准会让
整个传送成为字母数字的：字节从左到右复制，而另一个
操作数的 `PICTURE` 只决定其中**能放下多少个**。没有编辑，没有
去编辑，也没有数值转换。

```cobol
       01  SRC-GRP.
           05  SRC-N   PIC 999  VALUE 123.
           05  SRC-A   PIC AAA  VALUE "ABC".
       01  RCV-EDITED  PIC 0XXXXX0.
       01  RCV-NUM     PIC 9999V999.
       01  RCV-CHAR    REDEFINES RCV-NUM PIC X(7).
           MOVE SRC-GRP TO RCV-EDITED.  *> "123ABC " — the 0s are NOT inserted
           MOVE SRC-GRP TO RCV-NUM.     *> RCV-CHAR reads "123ABC "
```

`JUSTIFIED RIGHT` 是接收项仍然有发言权的唯一一件事，因为
那是字母数字传送的对齐规则。

同样的规则往下一级也成立，当组把它的字节交给它自己的
字段时：每个子项都**原样**取走它的那一片，不管它的 `PICTURE` 怎么说。
一个保存着字母的 `PIC 99` 子项，恰好就是程序所要求的——你
随后绝不能做的是对它做算术运算。

> **组上的 `VALUE` 子句以同样的方式工作。**它初始化组的
> 字节，并按宽度把它们分摊到各子项中，所以
> `01 MONEY-GRP VALUE "$123.45". 05 MONEY-EDITED PIC $999.99.` 让
> `MONEY-EDITED` 保存 `"$123.45"`——已经编辑过，而不是再次编辑。

### 限定条件名

一个 `88` 可以在不止一个组之下声明——三张表可以各自带有
自己的 `EQUALS-A`——而 `OF`/`IN` 区分它们，就像对
数据名那样。中间的层级可以跳过，而下标属于
**宿主**数据项，它选择它的 `VALUE` 用哪一次出现来测试：

```cobol
           IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
                    IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
                    OF GROUP-1-TABLE (13)
               PERFORM FOUND-IT.
```

> ⚠️ 按标准，对被声明了不止一次的条件名的**未限定**引用
> 是有歧义的。RustCOBOL 取第一个声明而不是拒绝程序——
> 与它对有歧义的数据名所做的相同——所以请限定它，
> 不要依赖哪一个胜出。

### 表意常量取它所遇到之物的大小

表意常量没有自己的宽度。它被重复以填满
它所对照书写的任何东西，而这条规则涉及值得了解的四个地方：

```cobol
       01  WS-BANNER   PIC X(6) VALUE ALL "ABC".
       01  WS-MARKS    PIC XXX  VALUE QUOTES.
```

- **在 `VALUE` 子句中**它填满数据项。`WS-BANNER` 保存 `"ABCABC"`，
  而 `PIC X(9)` 中的 `ALL "XY"` 保存 `"XYXYXYXYX"`——最后一个单元
  在数据项结束处被截断。
- **在比较中**它被重复到*另一个*操作数的大小，所以
  `IF WS-MARKS = QUOTE` 为真：三个引号对三个引号。
- **在 `MOVE` 中**它填满接收项，不管接收项是什么。
  `WS-KEY PIC X(10)` 的 `MOVE HIGH-VALUE TO WS-KEY` 设置全部十个字节，而
  **组**接收项会把填充分布到它的每个字段上——
  这就是在表扫描之前把整条记录清成哨兵值的办法。
  字母数字**编辑型**接收项仍然放置它自己的插入字符，
  所以 `PIC XX0XXBXXX` 保留它的 `0` 和它的空格，并填充
  它们周围的七个位置。
- **在另一个表意常量前面的 `ALL` 是多余的**，意思
  相同——`ALL SPACES` 就是 `SPACES`。

> **`HIGH-VALUE` 和 `LOW-VALUE` 是字节，不是字母。**它们是整理序列中
> 最高和最低的字节值，无论出现在哪里，每个都恰好占一个
> 字符位置——在记录中、在组 `MOVE` 中，
> 以及作为 `STRING` 的发送项。它们是索引文件中哨兵键的
> 通常选择。`DISPLAY` 无法有意义地渲染它们，所以请与
> 该常量比较，而不是从控制台上读取它们。

### `NUMERIC` 类测试比解析更严格

`IF WS-FIELD IS NUMERIC` 问的是**每个字符位置是否都保存
一个数字**——而不是这些字符能否被读作一个数。对于其
`PICTURE` 不带操作符号的数据项，下面这些全都**不是**数字：

```text
       "+1234"    a sign the PICTURE does not provide for
       "1.234"    a decimal point is not a digit
       "12 45"    a space is not a digit
       "123  "    trailing pad from a shorter MOVE
```

最后一个让人栽跟头。`WS-X5` 为 `PIC X(5)` 时，`MOVE "123" TO WS-X5`
留下 `"123  "`，而类测试说不是。如果你在验证操作员
输入，请把它移到数值数据项并测试*那个*，或者先检查字段已用的
长度。

### 把编辑后的字段读回来——去编辑

把数值**编辑型**数据项传送给数值数据项，恢复的是它的字符所
拼出的*值*，而不是字符。货币符号、分组逗号、
星号保护、`/` 和 `B` 插入以及空白都会被丢弃；字段中任何位置的 `CR`、`DB`
或 `-` 都会让它变成负数：

```cobol
       01  WS-SHOWN   PIC $(4)9.99CR.
       01  WS-VALUE   PIC S9(4)V99.
...
           MOVE -123.45 TO WS-SHOWN.     *> WS-SHOWN  = " $123.45CR"
           MOVE WS-SHOWN TO WS-VALUE.    *> WS-VALUE  = -123.45
```

这是标准自己的规则，也是你可以放心地从报表行中重新读回
已打印的金额，而不必保留它的第二份副本的原因。

### Beautify — 布局规则

每个提供 **✨ Beautify** 的编辑器（代码编辑器标签页、事件
编辑器、COBOL Structure 块的弹出窗口，以及 Indexed 编辑器的规范
布局）都按同一套规则重新格式化。如果你用过大型机或
PowerCOBOL 的美化打印器，这些会让你觉得熟悉：

- **段落**位于第 8 列；**过程语句**从第 12 列开始。
- **层级号**：`01`/`77`/`78` 在第 8 列，每深一层嵌套再缩进 3 个
  空格（`88`/`66` 位于其数据项下一级）。
- **一个数据条目占一行**——折行的子句会被合并——并且连续声明的
  `PIC` 和 `VALUE` 子句**从同一列开始**，所以一块数据项读起来像一张表。
- **嵌套像结构化代码那样缩进**，每层 4 个空格；
  `END-IF`、`END-PERFORM`、`END-TRY`、`ELSE`、`WHEN`、`CATCH` 和 `FINALLY`
  与开启它们作用域的动词对齐。
- **`EXEC … END-EXEC` 的内部从不被触碰**——嵌入的代码保持
  它自己的格式，逐字节不变。
- **块字面量同样从不被触碰**——`` ``` `` 围栏以及
  它们之间的一切。那段文本是字面量的*值*，所以
  重新缩进一行、折叠一串空格或更改一个词的大小写，
  都会改变你的程序所传送的内容。256 字符的上限在
  块内部也不适用：一长行 JSON 仍然是一长行。**未闭合的
  围栏是错误**，并且像任何其他错误一样会停止美化。
- 每个 **`SECTION` 头的上方有一个空行**（绝不是两个），所以
  长程序的各个 division 仍然易于浏览。
- **缺失的句子句号**只在 COBOL 要求的地方添加
  （在段落头之前、`CATCH`/`FINALLY` 之前、在
  后面紧跟下一个条目的数据条目末尾）；已有的句号绝不会被重复。
- 输出的行上限为 **256 个字符**：过长的字面量会拆到
  第 7 列的续行上，余下部分重新加引号，其他
  内容则在字边界处折行。

点击 Beautify 会先打开一个带有三个选项的小对话框，它们会被记住，
作为你的默认值：如何设定 **COBOL 动词**的大小写（leave as written / UPPERCASE /
lowercase / Capitalized——标识符和字面量绝不会被触碰），
以及**注释**是保持原样，还是与周围的
代码对齐（下面的样式始终会对齐它们，所以这个选择是针对
不带该样式的 Beautify）；以及是否应用 **PowerRustCOBOL 样式**。

**PowerRustCOBOL 样式。**对话框中还有一个 **PowerRustCOBOL style**
复选框，默认开启，并与另外两个选择一起被记住。开启它时，
Beautify 会写出*布局与注释*（§13）的布局：第一个数据子句
和 `MOVE`/`SET` 的 `TO` 在第 50 列，数据层级缩进 4 个空格，每个 `DIVISION`
之前、注释之后以及同一动词的一块语句前后各有一个空行，注释位于
它下面代码所在的列，每个动词之后有 4 个空格。不勾选该复选框时，Beautify
遵循上面的规则，与以前完全一样。该样式还会改变文本本身——
普通的 `MOVE` 变成 `SET`，对控件的普通调用会去掉 `INVOKE`（`INVOKE
Txt-Log::AppendText(WS-NL)` 变成 `Txt-Log::AppendText(WS-NL)`），而在没有任何东西依赖它的
地方会去掉句子句号，作用域由它的
`END-<verb>` 关闭——但**只有当 Beautify 能证明程序的含义仍然
相同时才这样做**：它会读取之前和之后的代码，并且只有在两者读起来是同一个程序时，
才保留这些更改中的某一个。无法证明的地方（比如代码本身
无法解析），会应用布局，而语句则保持不动。有
几样东西绝不更改：对窗口本身的调用（`ME::…`、`SUPER::…`）
保留它的 `INVOKE`，`MOVE CORRESPONDING` 仍然是 `MOVE`，而段落名
之前以及代码末尾的句号会保留。

⚠️ **有错误的代码绝不会被美化。**代码会先被检查（整个
程序通过真正的编译器前端）；如果它有错误，对话框会
列出它们，而文本保持逐字节不被触碰——重新格式化损坏的
代码会掩埋你正需要修复的那一行。而且如果某个结果令你
意外，**撤销（⌘Z / Ctrl+Z）一步就恢复到确切的先前文本**。

> **注意。****✨ Beautify 完全不碰块字面量**——两个
> 围栏以及它们之间的每一行。因为那段文本是字面量的值，
> 其中没有任何东西是格式化器可以整理而不改变你的
> 程序所做之事的。周围的代码可以随意格式化；
> 围栏之内的是你的。

### GLOBAL、EXTERNAL 和 GLOBAL EXTERNAL

你自己书写共享子句，完全如 COBOL-85 所定义的那样，写在
`WORKING-STORAGE` 中的 `01`/`77` 数据项上：

- **`GLOBAL`**——对程序的*被包含*程序可见。事件
  处理程序和用户过程嵌套在窗体模块中，所以窗体 WORKING-STORAGE 中的
  `GLOBAL` 数据项可以从每个处理程序读写，无需传来传去。`GLOBAL` 也适用于 **`FD`**——`FD F IS GLOBAL` 让文件及其记录区对窗体的过程可见，所以
  处理程序或用户过程可以 `READ`/`WRITE` 窗体所打开的文件。
- **`EXTERNAL`**——一份在*整个运行单元*内共享的物理副本，按
  数据项的真实名称匹配。**每个窗体模块都是它自己的运行单元**，所以 `EXTERNAL`
  数据项在窗体与它 `CALL` 的、把同一数据项声明为 `EXTERNAL` 的
  每个程序之间共享；两个*不同的*窗体各自声明
  `01 WS-COUNTER PIC 9(4) EXTERNAL`，得到的是各自独立的存储。要访问另一个
  窗体的数据，请限定引用（见下文）。`EXTERNAL` 只对
  `01`/`77` 数据项和 `FD` 有效——检查器会在其他任何地方对它标出错误。
- **`GLOBAL EXTERNAL`**——两者兼具：在运行单元内共享，*并且*对
  被包含的程序可见。

```cobol
       01  WS-SESSION-ID   PIC X(32) GLOBAL.
       01  WS-OPEN-FORMS   PIC 9(4)  EXTERNAL.
       01  WS-APP-CONFIG   PIC X(80) GLOBAL EXTERNAL.
```

### 访问另一个窗体的数据——限定的 `EXTERNAL`

如果你用 PowerCOBOL 构建过，你会认出这个问题的形态。
每个窗体都是封闭的运行单元，所以一个窗体中的表格事件无法简单地更新
另一个窗体正在显示的内容。数据必须被带过边界，
而带着它过去的管线，就是操作员所感受到的延迟。

PowerRustCOBOL 保持 `EXTERNAL` 的标准含义，并定义了一项
补充：`EXTERNAL` 数据项可以**由声明它的窗体模块来限定**。

> ⚠️ **可用性——已定义，尚未生效。**限定的 `EXTERNAL`
> 要求一个应用程序的各窗体在共享的运行单元中运行，而今天没有
> 任何宿主把它们合并成一个：每个运行中的窗体仍然得到它自己私有的
> `EXTERNAL` 存储，并且 `rcrun check` 会报告限定符——另一个
> 窗体的名称——未声明。下面所述的一切描述的是该
> 功能被定义为具有的行为，而不是已经交付的东西。在它交付之前，请用
> 已发布的属性（`super::"GetProperty"` 和
> `me::"SetProperty"`，§11）或通过它们共享的文件在
> 窗体之间传递数据。
>
> 这与*嵌入式 SQL* 中的共享 SQL 连接是两回事，后者
> 已经能跨窗体工作：那里是应用程序的各窗体一起使用的一个连接
> 注册表，而不是它们的 `WORKING-STORAGE`。

窗体 `CRM-MAIN` 发布当前的选择：

```cobol
       01  WS-SELECTED-CUSTOMER EXTERNAL.
           05  WS-CUST-ID     PIC X(10).
           05  WS-CUST-NAME   PIC X(40).
```

任何其他窗体都可以通过指名所有者来读取或写入它：

```cobol
           MOVE WS-CUST-ID OF CRM-MAIN  TO WS-ORDER-CUSTOMER.
           MOVE "ACME LTD"              TO WS-CUST-NAME OF CRM-MAIN.
```

窗体名称是**最外层**的限定符，所以当某个名称在别处有歧义时，普通的
组限定仍然可以在它之内使用：

```cobol
           MOVE WS-CUST-ID OF WS-SELECTED-CUSTOMER OF CRM-MAIN
             TO WS-ORDER-CUSTOMER.
```

交付之后会是什么样子：


| 规则                        | 预期如何                                                                                                                                                                                                   |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **可访问的内容**       | 只有目标窗体声明为 `EXTERNAL` 的数据项。限定并不是通往窗体普通 `WORKING-STORAGE` 的后门。                                                                                        |
| **命名**                  | 限定符是窗体的名称，它必须是有效的 COBOL 字。                                                                                                                                              |
| **生命周期**                | 存储属于应用程序的这一次运行，而不属于窗体的窗口。无论目标窗体是否打开它都存在，并且在那个窗体关闭之后仍保留它的内容。                                           |
| **初始内容**         | COBOL-85 禁止在 `EXTERNAL` 数据项上使用 `VALUE` 子句，所以必须由某个窗体显式设置初始内容。                                                                                                   |
| **`CANCEL`**                | 不会重置它。取消一个程序会清除该程序自己的 `WORKING-STORAGE`；`EXTERNAL` 存储比它活得更久。                                                                                              |
| **描述必须一致** | 同一个 `EXTERNAL` 名称在它被声明的每个地方都必须被完全相同地描述。因为构建会看到项目中的每个窗体，不匹配会在你构建时被报告，而不是在运行时破坏数据。 |

> **注意——共享不等于通知。**写入另一个窗体的数据会改变
> 数据，而不是屏幕上的画面。另一个窗体在有东西告诉它时才重绘；
> 共享的数据项自己不会推送更新。

> ⚠️ **这是 PowerRustCOBOL 的扩展。**标准 COBOL-85 没有办法
> 按拥有它的模块来限定 `EXTERNAL` 数据项——`OF`/`IN` 按
> 包含它的*组*来限定，绝不按程序。未限定的 `EXTERNAL` 仍然是可移植的
> COBOL-85；限定的引用则不是，并且在其他厂商的编译器上无法编译。
> 请只把它留给确实需要跨窗体数据的地方。

### 过程：窗体模块模型

每个窗体都成为它**自己的 COBOL 程序模块**（`PROGRAM-ID` = 窗体
名称）；一个项目是一个或多个这样的模块。在模块内部，每个过程——
**每个事件处理程序和每个用户过程**——都被生成为标记为 **`IS COMMON`** 的
嵌入（嵌套）程序，所以*任何*过程都可以在模块内的任何地方被调用：处理程序可以 `CALL` 另一个处理程序，用户过程
可以调用处理程序，依此类推。运行时系统把操作系统事件送入
模块的事件循环，由它分支到匹配的事件过程。

```cobol
      *> in a button handler — call a user procedure, or another handler
           CALL "RECALC-TOTAL".
```

用户过程只是你通过 **➕ Add procedure**（COBOL
Structure 列表）添加的具名过程；它能看到窗体的 `GLOBAL` 数据，并可以按名称调用。

**每个过程名称在它的窗体中都是唯一的。**每个过程都是它自己的
嵌套程序，而 COBOL-85 只允许一份源码中每个名称有一个程序。
两个同名的用户过程——或者一个与窗体或处理程序同名的过程——会被 **Check**、Run 和 Build 以
*"program '…' is declared
more than once"* 拒绝。**➕ Add procedure** 总是提出一个仍然空闲的名称。

**过程局部的数据是私有的。**过程可以声明它自己的
`WORKING-STORAGE`；这些数据项只在它内部可见。过程局部数据项上的 `GLOBAL` 子句
不向外共享任何东西（过程是叶子——它下面没有
嵌套的东西）。

**过程是静态的。**过程的局部数据只初始化**一次**，
它的值在**两次调用之间保持**——重新进入处理程序不会重置它的
WORKING-STORAGE，退出也不会取消它。如果你想在
每次进入时得到新的值，那是你的决定：对想要重置的数据项使用 COBOL 的 **`INITIALIZE`** 动词，
或用 `CANCEL "<name>"` 重置整个过程的状态。

### Rust-FFI 类型桥接（预览）

新窗体的 `REPOSITORY` 一开始就预先填充了一组精心挑选的 Rust 类型，
它们被声明为 COBOL 类——所有基本类型加上常用的标准库
类型——所以你可以立即书写对象引用：

```cobol
       REPOSITORY.
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-I32 IS "Rust.i32"
           CLASS RUST-VEC IS "Rust.Vec"
      *> … 45 more
```

```cobol
       01  WS-NAME  USAGE IS OBJECT REFERENCE RUST-STRING.
```

字面量是该类型在 Rust 层次结构中的路径（想想 .NET 中的 `System.String`）。
如果你把 `REPOSITORY` 清空，它会在下次加载时重新播种；你写的任何
内容都保持不动，即使你删除 Rust 条目。

你有两种方式**调用** Rust 方法——`INVOKE` 动词，或内联的
`object::method(…)` 形式，它也可以作为**值**用在
`DISPLAY`/`MOVE`/`COMPUTE` 之内：

```cobol
       01  S  USAGE IS OBJECT REFERENCE RUST-STRING VALUE "hello".
       01  N  PIC 9(4).
      *> verb form, result into N
           INVOKE S "len" RETURNING N.
      *> inline form, used directly as a value
           DISPLAY S::len().
           MOVE S::len() TO N.
```

---

## 22. 应用外壳与 `super` 接收者

如果你构建过大型的 PowerCOBOL 应用程序，你就知道它所采取的形态：
几十个窗口，每个都是自己的孤岛。PowerRustCOBOL 为企业
应用程序增加了一种替代方案——**应用外壳**：一个窗口，分成
菜单窗格、面包屑，以及窗体就地加载的内容区。
想想一个 ERP，它的主菜单列出各个子系统（CRM、HR、Sales）；进入
其中一个，就会挂载它的菜单，并把它的屏幕加载到同一个窗口中。

带有**图层**（§7）的窗体在外壳中与在窗口中的表现一样：当它
被加载到内容区时，它的图层覆盖窗体所占的区域，并且
`SET LAYER-NAME::Visible TO TRUE` 在那里也会显示一个。

### 打开外壳

把一个 **SideMenu** 控件放在你的**主窗体**上。这就是全部的开关：

- 主窗体带有 SideMenu → 应用程序以**外壳模式**启动。
- 没有 SideMenu——包括带有经典 `MenuBar` 的窗体——→ 每个窗体
  都在它自己的窗口中打开，和以前完全一样。现有的项目绝不会
  意外变成外壳应用程序。

你在**与 `MenuBar` 相同的菜单编辑器**中填写侧边栏：选中
SideMenu，然后在属性检查器中按 **Edit Menu…**。你已经
知道的一切都会沿用——项、子菜单、分隔符、加速键、图标、
每项所执行的动作——因为菜单存储在一个按控件
而不是按控件种类来命名的附属文件中。SideMenu 增加了两样东西：
加载窗体的项上的 **Preserve previous form**（见*导航链*），以及一个**徽章**。

**徽章。**一行可以在它的右边缘带一个简短的标记——未读
计数、“New”标记。在菜单编辑器中，把它键入该行的 **Badge** 字段
（留空表示没有），并选择一种 **Badge style**：

| 样式 | 绘制为 | 适合 |
|---|---|---|
| **Pill**（默认） | 以强调色填充的圆角标签 | `New`、`Beta` |
| **Count** | 填充的圆 | 数字：`6` |
| **Outline** | 强调色轮廓和文字，没有填充 | `Pro`、`outlined` |

徽章显示在每个界面上——设计器、Preview、Run Form 和构建出的
应用程序。MenuBar 没有徽章，所以它的编辑器不提供这些字段。
从 COBOL 中，`SetItemBadge(id, text)` 只更改你的程序添加的行
上的徽章（见*你的程序添加的行*），绝不更改设计好的
行。
### 侧边栏布局、页眉与页脚

**FullHeight**（默认开启）表示侧边栏占据窗口的整个垂直
范围，面包屑从它的右边缘开始。把它关掉，
面包屑就改为横跨整个宽度，而侧边栏在它
下面填满高度。无论哪种方式，侧边栏都到达窗口的底部；这个
属性选择两者中由谁占据左上角。

FullHeight 开启时，SideMenu 的 **Y** 和 **Height** 由外壳来
决定，所以检查器会把它们灰掉，并且控件在设计器中沿着窗体的
整个高度绘制——调整窗体大小，侧边栏就会跟随。它的
**Width** 仍然归你。

**Collapsed**（默认关闭）是应用程序*打开*时所处的状态。一旦
操作员自己操作过 ☰，他们自己最后的选择就会按
应用程序被记住，并从此优先——所以这个属性设定的是第一
印象，而不是永久设置。设计器画布显示你选中的
任何状态，所以你设计的就是启动时的样子。

> **注意。**操作员始终可以用侧边栏顶部的 **☰**
> 按钮折叠和打开侧边栏，*即使你还没有添加任何
> 一个菜单项*。能够收回这份宽度是操作员对
> 窗口的控制权，所以它从不取决于你在菜单中放了什么。COBOL 也可以
> 驱动同样的事情。从拥有侧边栏的窗体：
>
> ```cobol
>            SIDE-1::Collapse().
>            SIDE-1::Open().
>            MOVE 1 TO SIDE-1::Collapsed.
> ```
>
> 而从加载进 ContentPane 的窗体，则是 `super::SIDE-1::Collapse()` /
> `::Open()`。无论哪种方式，导轨都会立即改变，窗口随之变窄或变宽，
> 而这个选择会被记住用于下一次启动，与 **☰**
> 按钮的选择完全一样。

> ⚠️ **注意事项。**从 COBOL 读回的 `Collapsed` 是*程序*最后
> 写入的值（或设计时的状态），而不是操作员用 **☰**
> 按钮所作的选择，并且没有 `Toggle()`：必须翻转导轨的程序要保留
> 它自己的标志。

**导轨关闭时你的控件会移动——在画布上也是如此。**折叠
侧边栏会把它的宽度还给内容，内容向左滑动去占用它，而
设计器画布显示的这一滑动与运行中的应用程序将要发生的完全一致。
没有任何东西被编辑：你 `.cfrm` 中的矩形没有被触碰，检查器
仍然报告你给出的位置，而打开导轨会让一切复原。
点击会在你看到控件的位置选中它，无论处于哪种状态——所以你可以
在导轨关闭的状态下布局窗体，并确信它在打开时也能保持不变。

侧边栏绘制的一切都锚定在它的**顶部**并向下增长——先是
☰，然后是菜单项。侧边栏是导轨，而不是居中的标题。

**页眉徽标。****HeaderImage** 是**打开的**侧边栏顶部的图片。它的框是 **270 x 80 点**，
而这个框是**限制**，而不是
要填满的形状：

- 能放进 270 x 80 的徽标以**它自己的大小**居中绘制。
- 比这更大的徽标会**缩小到适合**，并保持它的**宽高
  比**——所以 540 x 80 的横幅被绘制成 270 x 40，高的 270 x 240 标志
  被绘制成 90 x 80，而方形的仍然是方形。

按 270 x 80 设计，它就恰好落位；设计得更大，它就会被适配，绝不会
被挤得变形。SideMenu 默认的 **HeaderHeight** 120 容纳
整个框，所以你不必更改任何东西就能用满它——但是低于约 88 点的页眉，
或者折叠的导轨，会缩小这个框（保持它
27:8 的形状），徽标也随之缩小。

让 **HeaderImage** 留空，框就会被**勾出轮廓**，这样你在有徽标之前，
就能看到徽标放在哪里以及它会有多大。

**应用程序标题。**给 **AppTitle** 一段文本，它就会绘制在
徽标旁边，采用强调色（`SelectedBgColor`）。标题先分得它的空间
——最多占页眉的 60 %——而徽标框缩小，保持它的形状，
并移到左边，进入剩下的空间；会窄于
24 点的框不会被绘制，标题占据整个页眉。（在 1.70.233 之前，
标题只出现在全尺寸徽标旁边，而默认的 200 点导轨
从来没有放下它的空间，所以它干脆不显示。）

**悬停颜色。**指针下的那一行以 `HighlightBgColor` 的柔和**淡色**铺底
——大约是该颜色的五分之一——所以悬停的行绝不会被读作活动行，
活动行穿的是实心的 `SelectedBgColor`。它的文字和图标在淡色行上
只要保持可读（WCAG AA，4.5:1），就采用 `HighlightFgColor`；
在无法保持的地方——浅色导轨上默认的白色——该行保留它的
`ForegroundColor`。

**折叠的**导轨根本不显示徽标：它显示 **HeaderIcon**，一个
专门制作的 45 x 45 标志，因为为 270 点页眉绘制的图像
在导轨的宽度上无法辨认。不设 **HeaderIcon**，窗格就会绘制
**折叠/展开箭头**，这样折叠的导轨始终显示能再次
打开它的控件，而不是一条空白。这在**嵌入的
窗体**中最为重要，那里的导轨是 ContentPane 上的普通控件，
它上方没有它自己的面包屑来承载那个控件。

**页脚面板归你。**每个 SideMenu 在它的页脚带中拥有一个 Panel，
它是普通的容器：把控件放进去，通过检查器设置它的样式，
并对你放在那里的东西绑定和处理事件。时钟、已登录
用户的名字、版本字符串和 Log-out 按钮是常见的住户。

你*不*拥有的是 Panel 所处的位置。它的矩形在每次更改时都会被重新固定到
页脚带，所以它会随窗体调整大小、**FooterHeight**
编辑和折叠而移动，不用你去移动它——拖动它不是定位
它的办法，**FooterHeight** 才是。

> **注意。**在外壳中，导轨是画在 ContentPane 旁边的外框，所以
> 页脚 Panel 及其内容是由**导轨**绘制的，而不是与窗体的
> 其余部分一起。这对你是不可见的——控件位于设计器所显示的
> 位置，它的事件像往常一样触发——但这就是为什么页脚中的控件
> 是唯一一处控件设计的 X 不是从窗体左边缘量起的地方。（在 1.61.151 之前，页脚的内容
> 是与窗体的内容一起绘制的，所以它们在运行时出现在导轨*旁边*，
> 而在设计器中看起来却是正确的。）
>
> **侧边栏中的图标。**每个菜单项的图标（在菜单编辑器中选取）
> 在每个界面上都显示在它的标签旁边——设计器画布、预览、
> Run Form 窗格以及运行中外壳的 MenuPane。SideMenu 的
> **IconEffect** 属性（`None` | `Shadow` | `Neumorphic`）选择这些
> 图标如何绘制——`Neumorphic` 与 IDE 的 Neumorphic 界面样式相匹配。

**在你自己的控件放在徽标下面的一条带。**页脚不是导轨可以容纳你的控件的唯一地方。给 SideMenu 一个 **HeaderBandHeight**（以点为单位；默认的 0 表示没有这条带），设计器就会为导轨创建第二个 Panel，即 `<id>-Header`，位于徽标之下、菜单行之上的带中。它是普通的容器，与页脚的一模一样：放进一个 ComboBox 来选择助手，一个 TextBox 来搜索对话列表，并像在其他任何地方一样对它们绑定和处理事件。菜单行从这条带下面开始。

有两点与页脚不同：

- **这条带只在侧边栏打开时才存在。**折叠成图标导轨时，这条带及其中的每个控件都被隐藏，菜单拿回它的高度——在一个图标宽的导轨上，选择器没有位置。再次打开导轨，它们就原样回来。
- **关掉这条带不会删除任何东西。**把 **HeaderBandHeight** 设回 0，会保留 Panel 及你放进其中的一切，只是高度为零，直到你再给它一个高度。

与页脚一样，你不定位这个 Panel：它的矩形在每次更改时都会被重新固定在徽标下面，所以它会随窗体调整大小、**HeaderHeight** 编辑和导轨的宽度而移动；**HeaderBandHeight** 决定它有多高。并且与页脚一样，运行中的导轨绘制这条带的控件，所以其中的控件是另一个设计的 X 不是从窗体左边缘量起的地方。

> ⚠️ **注意事项。**Panel 是在**设计器**中，在高度设置之后的第一帧创建的——与页脚的一样。在文本编辑器中编辑过 **HeaderBandHeight** 的窗体，在设计器中打开一次并保存之前，会显示一条空的带。

<!-- 📷 sidemenu-header-band.png — a SideMenu with HeaderBandHeight 56: a ComboBox in the
     band under the logo, the menu rows below it. Capture it open, and again collapsed
     (the band gone, the rows moved up). -->

**每种导轨状态一个图标大小。**检查器提供两个：


| 属性            | 检查器行             | 它调整什么的大小                                                      |
| ------------------- | ------------------------- | ------------------------------------------------------------------ |
| `IconSize`          | **Icon size (Open)**      | 侧边栏打开时，菜单项图标在它们标签旁边的大小。    |
| `IconSizeCollapsed` | **Icon size (Collapsed)** | 折叠导轨上菜单项图标的大小，在那里图标*就是*这一行。 |

两者都默认为 22 点，并接受 8 到 64 的任何值。它们是分开的，
因为这两种状态是两种设计：在标签旁边，图标不能
压过文字，而在窄导轨上单独出现时，同样的大小读起来
显得孤零零的。图标是作为矢量绘制的，所以任何值都是干净的缩放，而不是
拉伸的位图。

> **注意。**在 **Icon size (Collapsed)** 出现之前设计的窗体，在两种状态下都
> 简单地使用它打开时的大小，所以在你设置它之前，你已经
> 画好的一切都不会改变。

**折叠导轨的宽度也归你。****Collapsed width** 行
（`CollapsedWidth`，默认 48，范围 24 到 200 点）设定侧边栏折叠时图标
导轨有多宽——并且它在**每个界面上都是同一个值**：运行中应用程序的窗格、设计器画布和
预览都把导轨收窄到恰好这个值，所以你所依照设计的导轨，
就是你的用户所看到的导轨。当导轨折叠时，在那些界面上
**内容也跟随它的边缘**——导轨右边的一切都向左
滑过导轨让出的那一列，与运行中的外壳移动
它的内容窗格完全一样，并在导轨打开时滑回来。*打开*的窗格不需要属性：它和
你画的控件一样宽。小于 24 的值会被提高到 24——低于它，图标
行就没有地方可以容纳了——大于 200 的值会被降低到 200，而在该属性出现之前
设计的窗体仍然折叠到 48，和以前一样。

**折叠的导轨承载什么。**导轨只有一个图标宽，所以一个项只有在能被
*通过它的图标*到达时，才在上面赢得一席之地：它有**图标**，
它有**动作**，并且它**不是组**。


| 被排除，以及原因                                                                                                                                                                                 |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **组。**它的含义是它所打开的列表，而导轨没有地方去打开一个。取而代之的是，它符合条件的子项会**出现在它的位置**——导轨是你的快捷方式，而不是你的结构。 |
| **有动作但没有图标的项。**没有东西可画，也没有东西可瞄准。                                                                                                                                                                                    |
| **有图标但没有动作的项。**一个戴着图片的标签；导轨没有放标签的空间。                                                                                                                                                                                               |

所以一个由 *Home*、*Samples*（一个组，其中有 *Dashboard*、*Form 1*、*Form 2*
和一个没有图标的 *CMS*）以及 *SaaS* 构成的菜单，会折叠成五个图标：Home、Dashboard、
Form 1、Form 2、SaaS。**打开的窗体的操作**遵循同样的规则，并
出现在分隔线下面。没有任何东西是按名称特殊处理的——*Home* 之所以在那里，
是因为它有图标和动作。

> **注意。**这是要*依照*来设计的规则：如果你想让某个屏幕出现在导轨上，
> 请给它的菜单项一个图标和一个动作。你也想让它可达的组，
> 应当有它自己的叶子项，而不是依赖组的那一行。

**缩进。**组的项在它下面**缩进**，每次一级。
整行一起移动：带有图标的项会把图标连同
标签一起带进来，所以图标在每一级都保持在文字旁边的位置，而每一级
都有它自己可以向下阅读的一列。

**Home 在导轨上自成一类。**在折叠的导轨上，动作为
**Home (main content pane)** 的项后面会跟着相当于一整行的额外空间，
所以从它到下面图标的距离是任意
另两个图标之间距离的两倍。赢得这块空间的是项的*动作*，而不是它的
标签——把一行叫作“Home”而没有 Home 动作，它就是普通图标，
而如果你把动作移到别处，空间就跟着动作走。（在
分节分隔线已经落在 Home 下面的地方，不会添加任何东西：分隔线起了分隔作用。）

**侧边栏在 Preview 和 Run Form 中是实时的。**点击 ☰ 会折叠和
打开导轨（触发 `onMenuOpen`/`onMenuClose`），点击项行会
设置 `SelectedItemId` 并触发 `onMenuItemClick`——与
外壳所交付的行为相同，所以你在预览中试的就是交付的。

外壳窗口有三个固定的区域：


| 区域          | 它是什么                                                                                                                                                                                                                                                                           |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **MenuPane**    | 主窗体的菜单（*根*槽，始终存在）加上当前子系统的菜单（*上下文*槽，整体替换）。Open 或 Collapsed——折叠是一条窄的图标导轨；两种状态都带有 ☰ 切换，而该状态会按应用程序被记住，跨越重新启动。 |
| **Breadcrumb**  | 导航链的每一步一个片段（`Main › CRM › Customers`）。点击片段会回到那里。由外壳绘制——加载的窗体的颜色从不影响它。                                                                                                      |
| **ContentPane** | 加载的窗体，位于左上角，大小为它设计的大小。                                                                                                                                                                                                                                     |

> **自带 SideMenu 的目标窗体会在新窗口中打开，
> 而不是在 ContentPane 中**（1.63.29）。导轨形的窗体加载在
> 外壳自己的导轨旁边，就是两个侧边栏争夺屏幕的同一条边——所以指向这样一个窗体的
> 菜单项，其行为与
> **open-standalone** 项完全一样：它自己的顶层窗口，可以自行关闭，
> 外壳的 ContentPane 不受触碰。带有普通
> `MenuBar`（水平条，而不是导轨）的目标仍然像往常一样加载到
> ContentPane 中——只有 `SideMenu` 会触发这一点。那个窗口是
> **它自己的外壳**——见下面的*拥有自己侧边菜单的窗口*。

### 你的程序添加的行

菜单编辑器在设计时画出你所熟悉的侧边栏。真正的侧边栏中有很多内容
只有在程序运行时才知道——用户过去的对话、
文件夹中的文档、他们可以打开的报表。请从 COBOL 中添加这些行；
它们出现在设计好的行之后，外观和点击都与它们完全一样，
并在程序结束时消失。没有任何东西被写回菜单文件。

```cobol
       PROGRAM-ID. MAIN-FORM--ONLOAD.
       PROCEDURE DIVISION.
      *> A section title, then one row per conversation.
           MOVE SIDEMENU-1::AddSection("History") TO WS-SECTION-ID
           MOVE SIDEMENU-1::AddItem("CHAT-0041", "Tuesday's invoice query",
                                    "chat") TO WS-OK
           MOVE SIDEMENU-1::AddItem("CHAT-0042", "Supplier onboarding",
                                    "chat") TO WS-OK
      *> A row under a designed row (its id copied from the menu editor),
      *> which opens a form when clicked.
           MOVE SIDEMENU-1::AddItem("RPT-Q3", "Q3 report", "report",
                                    "rpqx", "open-form:Q3-REPORT") TO WS-OK
           .

       PROGRAM-ID. SIDEMENU-1--ONMENUITEMCLICK.
       PROCEDURE DIVISION.
           MOVE SIDEMENU-1::SelectedItemId TO WS-CHAT-ID
           INVOKE VWR-1::SelectConversation(WS-CHAT-ID)
           .
```

| 调用 | 它做什么 |
|---|---|
| `AddItem(id, label [, icon [, parent-id [, action]]])` | 添加一行，或就地替换*你*用该 id 添加的那一行。`parent-id` 把它挂在任何一行之下，不论是设计的还是你的，最多三层。`action` 是设计行的动作会是什么；为空表示 `onMenuItemClick` |
| `AddSection(title)` | 添加一个分节标题；回答它的 id |
| `SetItemLabel` / `SetItemIcon` / `SetItemBadge` / `SetItemEnabled` / `SetItemAction` / `SetItemControl` `(id, value)` | 更改你的某一行。`SetItemLabel` 和 `SetItemEnabled` 也能作用于设计的行。`SetItemControl` 指名该行用来代替它的标签而显示的控件——见下面的*承载控件的行* |
| `RemoveItem(id)` | 移除你的某一行以及它之下的一切 |
| `Clear()` | 移除你的所有行；设计的菜单保持不变，带着你给它的任何标签或状态 |
| `GetCount()` / `HasItem(id)` | 你添加了多少行；某个 id 是否存在 |
| `ActivateItem(id)` | 做点击那一行所做的事——在 ContentPane 中打开它的窗体、回到主页，或触发 `onMenuItemClick`（行的 id 在 `SelectedItemId` 中）——从代码中。任何一行，设计的或你的；禁用的行什么也不做。这是程序自己把操作员送往某处的办法：首次运行时的欢迎窗体，已完成任务所通向的窗体 |

```cobol
      *> first run: nothing is configured yet, so show the welcome form
           IF WS-CONFIGURED NOT = "Y"
               MOVE SIDEMENU-1::ActivateItem("welc") TO WS-OK
           END-IF
```

> **注意。**`ActivateItem` 在**外壳**中有效——即其 SideMenu
> 是它的菜单窗格的主窗体——因为是外壳在
> ContentPane 中打开窗体。

每个更改某样东西的调用，在它做了时回答 `1`，没做时回答 `0`，所以
程序可以分辨。

> ⚠️ **设计的菜单的结构在设计时归你，而不是在运行
> 时。**程序绝不能移除或替换来自菜单
> 编辑器的行，也不能更改它的图标、徽章、动作或承载的控件——调用回答 `0`，
> 而菜单不受触碰。请给你自己的行取不会与
> 设计的行冲突的 id（像 `CHAT-` 这样的前缀就够了）。
>
> 设计行的两样东西*确实*归程序：它的**标签**以及
> 它是否**启用**。这就让你可以在
> 菜单编辑器中设计整个菜单——这样它在设计器和预览中都能显示——并且仍然
> 翻译它，并在应用程序设置好之前让它保持关闭：
>
> ```cobol
>            MOVE SIDEMENU-1::SetItemLabel("tpcs", T-MENU-TOPICS) TO WS-OK
>            MOVE SIDEMENU-1::SetItemEnabled("tpcs", "0") TO WS-OK
> ```
>
> 禁用的行会忽略点击。`Clear()` 会让这些设置保持原位。

> **注意——动作以及侧边栏位于何处。**在应用程序外壳中，
> 行的动作会导航，不论它是设计的还是添加的。普通窗口上的 SideMenu
> 会为每一行触发 `onMenuItemClick`，而把导航留给
> 你的处理程序——两种行都是如此。

> 📷 需要截图 — `sidemenu-runtime-rows.png`。截取一个外壳，其导轨
> 显示两个设计的行、一个“History”分节，以及程序
> 添加的三个对话行，其中一个被选中。

#### 承载控件的行

一行不必显示标签。它可以显示**你的某个控件**——用来选择助手的
ComboBox，用来过滤对话的 TextBox——位于
其他行之间。这与页脚和页眉带是同一个想法，
只不过控件属于一个*行*，所以它位于那一行在
菜单中所落的位置，随它滚动，并在它上面的菜单变化时移动。

**在设计器中：**

1. 把控件放到 SideMenu 上，使它成为 SideMenu 的**子项**
   （ComboBox 在项目树中的父项是 SideMenu）。放在窗体上
   其他任何地方的控件绝不会被某一行承载。
2. 打开 **Edit Menu…**，选中那一行，然后在 **Hosted
   control** 中选择该控件。列表提供 SideMenu 自己的子项；**(none)** 会把
   标签还给这一行。
3. 画布会立即把控件移到那一行中，并在你
   添加、移除或重新排序行时保持它在那里。不要手工定位它。

**这一行看起来是什么样子。**控件从该行
图标的右边一直填到该行的右内边距，在顶部和底部比该行短几个点，
这样该行自己的高亮仍然显示在它周围。没有
图标的行则从该行的内边距开始。该行保留它的图标、它的徽章
以及它在菜单中的位置，并且不绘制它的标签：控件就是
标签。控件自己的 **X**、**Y**、**Width** 和 **Height** 在
运行时无关紧要——由该行决定它们——而它保留它的所有其他属性和
每个事件，与在任何其他窗体上完全一样。

**当侧边栏折叠时**，该行只是它的图标，每一行都是如此，而
控件被**隐藏**；侧边栏打开时它原样回来。被
滚动到页眉或页脚下面一半的行，会连同它剪裁它的控件。

**从程序中**，你添加的行也可以用 `SetItemControl(row-id, control-id)`
承载控件；空的控件 id 会收回控件：

```cobol
      *> a picker row among the conversations
           MOVE SIDEMENU-1::AddItem("PICK", "Assistant", "user") TO WS-OK
           MOVE SIDEMENU-1::SetItemControl("PICK", "CMB-ASSISTANT") TO WS-OK
      *> later: the row shows its label again
           MOVE SIDEMENU-1::SetItemControl("PICK", "") TO WS-OK
```

> **说明。**
> - 当一行指名某个控件时，控件**只**显示在那一行中：当
>   该行不在屏幕上（侧边栏折叠，或该行被滚动出
>   菜单）时它被隐藏，绝不会留在它设计的位置。
> - 当一行放手——空的 `SetItemControl`，或者该行被移除——时，
>   它又成为窗体的普通控件，位于它设计时所在的
>   位置。只由程序来承载的控件，在那之前应当
>   位于你希望它所在的地方。
> - 读取和更改控件——`CMB-ASSISTANT::Value`、它的事件——
>   不变；它仍然是*你的*控件，只是由那一行来放置。只在
>   一行中指名某个控件。

> ⚠️ **注意事项。**
> - 控件必须是 **SideMenu 的直接子项**。位于本身在导轨上的
>   Panel 之内的控件，由那个 Panel 放置，而不是由某一行。
> - 承载**单个普通控件**——ComboBox、TextBox、Button、
>   Switch。容器（里面有其他控件的 Panel）不被
>   支持作为承载的控件。
> - 设计行承载的控件是在菜单编辑器中设置的；程序无法
>   更改它——对设计行使用 `SetItemControl` 会回答 `0`。

> 📷 需要截图 — `sidemenu-hosted-row.png`。截取一个外壳，其导轨
> 有 History 分节，并且在它的行之中有一行用 ComboBox 代替
> 它的标签来显示；然后是同一个折叠的导轨，该行被缩减为
> 它的图标。

### 面包屑框架

面包屑是一个**框架**，而不只是一行文字。它总是从
侧边栏的右边缘延伸到窗口的右边缘——没有宽度或
位置可设置，因为它只可能在一个地方——而侧边栏
拥有五样归你选择的东西：


| 属性（在 SideMenu 上）  | 检查器行                 | 它做什么                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BreadcrumbHeight`          | **Breadcrumb height**         | 框架被绘制得多高，16 到 200 点。默认 28。                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `BreadcrumbBackgroundColor` | **Breadcrumb background**     | 框架自己的颜色。让它**留空**，框架就保持跟随内容窗格的背景，这是它一向的做法。                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `BreadcrumbTextAlign`       | **Breadcrumb vertical align** | 链**以及 Open/Collapsed 切换**位于框架内部的哪里：`Top`、`Middle`（默认）或 `Bottom`。它们作为一组移动：对齐放置这一对，然后链在切换按钮自己的那一行上居中，所以文字在 `Top` 和 `Bottom` 时位于图标的中间，与在 `Middle` 时完全一样，不管图标多大。改为把每一个对齐到框架自己的边缘，会让大图标和小字体的中线分开，读起来像两个不相关的控件。 |
| `BreadcrumbFontSize`        | **Breadcrumb font size**      | 链自己的文字大小。`0`——默认值——让它保持跟随侧边栏的 `FontSize`，这是它一向的做法。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `BreadcrumbIconSize`        | **Breadcrumb icon size**      | Open/Collapsed 切换按钮自己的大小。`0`——默认值——让它保持为框架高度的正方形，这是它一向的做法。它绝不会被绘制得比框架更高。                                                                                                                                                                                                                                                                                                                                                                    |

你选择的颜色可以带有 alpha，此时窗格会透过它显示——
但框架最终总是被绘制为**不透明**的，因为它是外框：
其中的一个洞会显示出桌面。

**高度、文字大小和图标大小是三个独立的旋钮。**这值得
明说，因为它过去是假装成三个的一个旋钮：

- 更改 **`BreadcrumbHeight`** 只移动框架，别无其他。对它来说过大的
  文字是被框架*剪掉*的，而不是溢出到外面，所以你
  设置的高度就是你得到的高度。
- 更改 **`BreadcrumbFontSize`** 只移动链的文字，别无其他。
  链过去与菜单标签共用侧边栏的 `FontSize`，所以调整
  一个总会调整另一个；现在菜单标签保持不动。
- 更改 **`BreadcrumbIconSize`** 只移动切换按钮，别无其他。切换按钮
  过去是框架高度的正方形，所以把带加高以容纳
  你自己的控件时，箭头也随之变大。

把后两个保持为 `0`，一切都和以前完全一样——你已经画好的
窗体都不会改变外观。从 PowerCOBOL 过来，这是
各部分靠副作用互相决定大小的带，与每个部分都是你所设置的
属性的带之间的差别。

这种独立性正是对齐值得拥有的原因：设置 64 点的框架，
链就有了活动空间，所以由你决定它是靠在顶部、
居中，还是靠在底部。

**你可以把控件放在框架上方。**给它一些高度，它就成为一条
你可以在其中设计的带：标题、搜索框、状态标签、你自己的
工具栏。在设计器中把控件画在框架上，应用程序运行时它就
绘制在框架之上——在画布、Preview 和
外壳中是同样的画面。

> **注意。**框架**不是容器**。位于它上方的控件是碰巧重叠的
> 普通窗体控件：它不是任何东西的子项，不会被
> 框架剪裁，不会随它移动或滚动，并保留它在窗体上
> 其他任何地方都会有的每个属性和事件。它只是画在上面，
> 并接受点击——框架绝不会从你的控件那里抢走点击。

> ⚠️ **这适用于外壳窗体自己的控件，而且只适用于那些。**
> **加载进内容窗格**的窗体是另一个窗体，有它自己的坐标空间，
> 并从框架**下面**开始。不管你把带加得多高，它的第一排控件
> 都绝不会落在导航链上——你
> 不必为了避开它而在每个嵌入窗体的顶部留出边距。

📷 需要截图 — `breadcrumb-frame.png`。在 Form Designer 中，选中外壳窗体上的
SideMenu，把 **Breadcrumb height** 设为 64，并把 **Breadcrumb
background** 设为与窗体形成对比的颜色，然后在框架上放一个 Label 和一个
TextBox。截取画布，使更高的有色框架和
位于其上的两个控件都可见。

### 为你正在处理的内容命名——细节层级

面包屑片段命名的是一个*屏幕*。操作员常常需要知道这个屏幕所持有的是*哪条
记录*。请从窗体自己，在当前窗体
自己的名称之后添加一个**细节层级**：

```cobol
      *> Main Menu > Customer Data > John Smith
           me::"SetBreadcrumbDetail"(WS-CUSTOMER-NAME)
      *> ...and back to just Main Menu > Customer Data
           me::"ClearBreadcrumbDetail"()
```

值得了解的规则：

- 细节属于设置它的窗体，并属于它被显示的那一刻。
  导航到任何地方——另一个屏幕、某个面包屑片段、**Home**——它就
  被丢弃；进入的窗体以一个干净的面包屑开始，并设置它自己的。
- 只有**被显示的**窗体才能设置一个。不在窗格上运行的窗体在
  那里没有名称可以挂细节，所以它的调用会被忽略，而不是
  劫持别人的片段。
- 设置为空文本与清除它相同。
- 它是一个层级，不是一个栈：再次设置会替换它。

### 点击你自己的名字——重置

一旦显示了细节层级，窗体自己的片段就不再是“你所
在之处”，而成为一个链接。点击它会**让那个窗体重新开始**：

```mermaid
flowchart TD
    A["操作员点击窗体自己的片段<br/>(Main Menu > Customer Data > John Smith)"] --> B{"窗体是否设置了<br/>me::PreventReset？"}
    B -- "开" --> C["什么也不重置<br>onResetRejected 触发<br>面包屑保持不变"]
    B -- "关，窗格的占用者" --> D["旧实例上触发 onDestroy<br>新的实例取而代之<br>onShow / onActivate，空白的存储<br>面包屑被清除"]
    B -- "关，外壳自己的窗体" --> E["onReset 触发<br>面包屑被清除"]
```

**窗体有最后的决定权。**只要你持有值得失去的东西，就设置这个防护，
而不持有时就清除它：

```cobol
       CUSTOMER-CHANGED SECTION.        *> any field's onChange
           MOVE 1 TO me::PreventReset.

       SAVE-CUSTOMER SECTION.           *> after a successful write
           MOVE 0 TO me::PreventReset.

       RESET-REFUSED SECTION.           *> onResetRejected
           MOVE "Save or cancel first" TO Label-Status::Caption.
```

`PreventReset` 是通用窗体界面的一部分，就像 `Title` 或
`FormState`，所以它在构建时被检查，并且可以从另一个窗体通过 `super::` 读取。

**“重新开始”的含义**取决于显示的是哪个窗体：

- **加载进 ContentPane** 的窗体会被重建：它的 `onDestroy` 运行（关闭
  文件、COMMIT），它的实例和 WORKING-STORAGE 被释放，一个全新的
  实例取代它的位置——同一个屏幕，空白得像它第一次打开的那天，在
  链中的同一位置。重置**不是**导航：链
  不会移动。
- 外壳的**主窗体本身**没有第二个实例可以换入——重启它
  就会重启应用程序——所以它接收 **`onReset`**，并做它自己的
  清理工作（`INITIALIZE`、重新读取默认值、清空屏幕）。

> ⚠️ **注意事项。**框架是**内容区**的顶部带，所以控件
> 只有在侧边栏的 **FullHeight** 开启（默认）时才能位于它上方。
> FullHeight 关闭时，面包屑是整个窗口上方的一条带——也在
> 侧边栏上方——它下面没有窗体可以放置控件。

### FormFormat——窗体可以如何被加载

每个窗体都在属性检查器中声明它：

- **Standalone**（默认）——它自己的窗口，用 `OpenFormSync` /
  `OpenFormAsync` 打开。在应用程序外壳出现之前，每个窗体都是这样工作的，
  它现在仍然这样工作。
- **Embedded**——由菜单项加载进 ContentPane。
- **Both**——对两种路径都有效的可复用屏幕（一个客户查找，
  在 Sales 中是模态对话框，在 CRM 之内是浏览窗格）。

**构建会检查这种搭配**：指向 Standalone 窗体的菜单项，
或指名 Embedded 窗体的 `OpenFormSync` 调用，是会指出
该窗体的编译错误。主窗体始终是 Standalone——它拥有窗口。

窗体处于嵌入状态时，它仅限窗口的属性（WindowState、FullScreen、
TitleVisible、CanMinimize、CanMaximize）是无效的，并在
检查器中显示为灰色；`Width`/`Height` 报告**设计的**值。进入和退出的
窗口效果只为独立窗体播放——嵌入的窗体只是出现在那里。

**背景规则。**加载的窗体的背景绘制**整个
ContentPane**——颜色、渐变或图像，其图像/渐变的几何形状
是按*窗格*而不是窗体矩形计算的。窗体滚动时
（大于窗格的窗体在其中滚动），背景保持不动。
完全透明的窗体（Transparency = 100）会让桌面透过
窗格区域显示——菜单和面包屑保持不透明。

> ⚠️ **注意事项。**所以同一个 `Both` 窗体在嵌入时（窗格大小、固定）和
> 独立时（窗口规则，
> spec 037）显示背景的方式不同。这是有意的设计；请据此设计背景。

### 把 Embedded 窗体的大小定到 ContentPane

Embedded 窗体保持你设计的大小。窗格**不会**拉伸去容纳
它，窗体也**不会**被缩小去适应——所以如果窗体大于
窗格，多出的部分就会滚动。滚动条浮在窗体上方，
不保留任何槽，但只要窗体大于
窗格，它们就**可见**——沿窗格底部（横向）或右边缘（上下）
有一条细细的把手，在指针下会变粗，让操作员能看到窗体
还在延续，并且无需触控板就能拖到那里。滚动只移动窗体：
SideMenu 保持不动，而滚过窗格左边缘的任何东西都会从
导轨下面经过，绝不会盖在它上面。

在设计窗体之前先算出窗格：

```text
ContentPane width  = main form width  − SideMenu width
ContentPane height = main form height − BreadcrumbHeight
```

两个数字都来自**主窗体**：导轨就是你所画的 SideMenu 控件
（而不是固定的默认值），而带就是同一个控件的
`BreadcrumbHeight` 属性。一个 1584x936 的主窗体，带有 296 宽的 SideMenu 和
默认的 28 点面包屑，得到 **1288x908** 的窗格——所以设计为 1320 宽的 Embedded
窗体有 32 点永远无法出现在屏幕上，而且随着操作员把窗口缩小，
这个差距还会增大。

哪些控件会消失，取决于每个控件的**右边缘**，而不是
它从哪里开始：x=32、宽 456（右边缘 488）的控件，比 x=568、宽 704（右边缘 1272）的控件
能在窄得多的窗格中幸存。

Form Designer 会在大小仍由你选择时就警告你这一点——
画布上方有一条琥珀色的带，指出窗体的大小、窗格的
大小以及多出的部分：

> ⚠️ This Embedded form is 1320x720; the main form's ContentPane is 1288x908 —
> 32px will scroll out of view.

补救办法是把 Embedded 窗体变窄，或者把主窗体加宽（或通过画一个更窄的 SideMenu
来加宽它的窗格）。窗体一合适，这条带就会自己消失。

> **注意。**这个警告只对 Embedded 窗体出现。Standalone 窗体拥有
> 它自己的窗口，没有会溢出的窗格，而 `Both` 窗体的度量方式与
> Embedded 的相同，因为那是它们可能被裁剪的路径。

> ⚠️ **注意事项。**这条带与主窗体的**设计**大小进行比较。
> 把运行中的窗口拖得比那更窄的操作员会损失更多，
> 而把它最大化的则会拿回多出的部分。请按设计的大小来设计，
> 并把越过窗格边缘的任何东西视为可有可无。

### 导航链

从菜单加载的窗体形成一条链——主窗体 → 子系统 → 屏幕。链**中的**每个
窗体都**保持驻留**：它的 WORKING-STORAGE 活着，它的菜单
处理程序继续触发，即使它的主体没有被显示。面包屑就是
那条链。点击某个片段会销毁它之下的一切（最深的先销毁），
重新挂载那个窗体的菜单，并再次显示它的主体。

有两种菜单行为控制同级切换（菜单编辑器中，按每个项）：

- 默认：从屏幕 A 切换到屏幕 B 会**销毁** A。
- 勾选 **Preserve previous form**：A 保持驻留，而回到 A
  是即时的，它的数据与离开时完全一样。

有两个窗体事件把它们区分开——像绑定任何其他事件那样绑定它们：

- **onDeactivate**——主体离开了窗格；窗体仍然驻留。*不要*
  在这里关闭文件。
- **onDestroy**——存储即将被释放。请在这里关闭文件、COMMIT、释放
  资源。

### `super`——加载我的那个窗体

`me` 指的是当前窗体；**`super`** 指的是加载或打开它的
那个窗体——两种路径都是，菜单加载和 `OpenFormSync`/`OpenFormAsync`：

```cobol
      *> read and change the parent form's properties
           MOVE super::Title TO WS-T.
           MOVE "Processing…" TO super::Title.
      *> drive its window (any windowHandler method)
           super::"SetWindowState"("Minimized")
      *> walk further up: one loader per super
           MOVE super::super::Title TO WS-T.
      *> drive the menu pane (state persists per application)
           super::SIDE-1::Collapse().
           super::SIDE-1::Open().
```

预期的规则：

- **光秃秃的属性在构建时被检查**，对照通用窗体
  界面（Name、Title、Width、Height、X、Y、WindowState、FullScreen、
  TitleVisible、CanMinimize、CanMaximize、FormState、FormFormat、
  BackgroundColor、Transparency、PreventReset）——像 `super::Widht` 这样的拼写错误
  在任何深度都会使构建失败。
- **运行加载你的那个窗体的过程**——
  `super::"RecalcTotals"()`。这个名称是*父窗体*自己的某个过程（它的程序的
  一个段落或用户过程），它在那里的运行与
  `CALL "RecalcTotals"` 完全一样。调用会立即得到回答，而过程
  在父窗体下一次等待事件时运行，所以它不会给你
  返回任何东西；它所改变的，改变在父窗体上。这就是
  ContentPane 中的窗体告诉主窗体有东西变了的办法：

  ```cobol
      *> in the settings pane, after the agents were saved
           super::"PC-MENU-STATE"()
  ```

  父窗体没有对应过程的名称会在它的程序输出中被报告，
  且什么也不运行。⚠️ 在 1.70.255 之前，这里描述过这个调用，但它并不工作：
  它在运行时以“windowHandler has no method”停止。
- **`super` 可以是 NULL**：在主窗体中，以及在异步打开的窗体中，
  如果它的打开者已经关闭（子窗体从不让它的打开者保持存活）。
  引用 NULL 的 `super` 会引发标准的运行时错误。
- `me::<property>` 在窗体自己的界面上以同样的方式工作——
  `me::Width`、`MOVE "New" TO me::Title`——而 `me` 和窗体自己的名称
  指的是同一个东西。通过其中任何一个写入 `Title`、`Width`、`Height`、`X`、`Y`、
  `BackgroundColor` 或 `Transparency`，都会更改运行中的
  窗口（见*窗口外框与状态*）。

### 打开窗体——三扇门

应用程序同时持有许多活动的窗体。每个被打开的窗体都作为
**它自己的程序**运行，带有**它自己的 WORKING-STORAGE**——窗体从不读取彼此的
数据项。它们通过上面的界面交谈：已发布的窗体
属性、`super::X`，以及 windowHandler 方法。

打开窗体有三种方式，而 **Form format** 属性决定其中
哪些可以加载它：

1. **进入 ContentPane**——带有 **Open form** 动作的侧边栏项。
   目标需要格式 `Embedded` 或 `Both`。离开的占用者
   被停用（并且在点击的项勾选了 *Preserve previous
   form* 时被停放）；面包屑随之变化。
2. **从 COBOL 作为子窗口**——`me::"OpenFormSync"` /
   `"OpenFormAsync"`，以调用的窗体为父。目标需要
   `Standalone` 或 `Both`。
3. **从侧边栏作为子窗口**——**Open Stand Alone Form
   (Sync)** / **(Async)** 菜单动作，或者通过
   SideMenu 控件本身以编程方式：

```cobol
      *> block until the report window closes (Sync is implicitly modal —
      *> the whole shell waits with you)
           SideMenu-1::"OpenStandAloneFormSync"("RPT-MONTH", "Normal",
               80, 80, 640, 480, "true")
      *> or open it modeless and keep its handle
           INVOKE SideMenu-1::"OpenStandAloneFormAsync"("MONITOR")
               RETURNING WS-H.
           INVOKE WS-H "Focus".
```

以这种方式打开的窗口以**外壳**为父，不管是哪个窗体运行了
这个调用——关闭应用程序会关闭它们。目标需要
`Standalone` 或 `Both`。

#### 拥有自己侧边菜单的窗口

携带它自己的 **SideMenu**、作为窗口被打开的窗体——从 COBOL 用
`OpenFormSync` / `OpenFormAsync`，从独立的菜单项，或从
**Open form** 项（见上文）——在那个窗口中作为**它自己的外壳**运行，
与它作为主窗体时完全一样：

- 它的导轨、面包屑和 **ContentPane**，带有折叠控件，并且
  窗口随导轨变宽和变窄；
- 它的菜单的 **Open form** 项把窗体加载进**那个窗口的**
  ContentPane，绝不进主窗口的，而面包屑随之变化；
  **Home** 把窗口自己的窗体带回来；
- 加载在那里的窗体把该窗口的窗体作为它的 `super`；
- 关闭窗口会关闭加载进它的每个窗体。

所以一个完整的子应用程序——比如一个有自己的主题、文档和
设置屏幕的聊天——可以从任何应用程序作为一个窗口打开，
并在那里像它独自运行时那样工作。

```cobol
      *> open the assistant beside the application, modeless
           ME::"OpenFormAsync"("CHAT-FORM")
```

> **注意。**每个窗体的菜单都是它自己的：两个窗体可以都有
> `SideMenu-1`，各自在自己的文件夹中有自己的 `SideMenu-1.menu.yaml`，
> 而每个窗口显示它自己的菜单——在 Run Form 下和在构建出的应用程序中
> 都一样。

**Sync 隐含模态。**无论来自菜单点击还是 COBOL：在
Sync 打开的窗口存在期间，它的父窗体的整个界面——包括外壳的外框——
都不接受输入。Async 窗口绝不是模态的。

**Target 列表。**菜单编辑器的 **Target** 列表只提供所选动作
可以合法加载的窗体，而构建对 COBOL 中的字面量
窗体 id 也强制执行同样的规则——不匹配是编译错误，而不是运行时的意外。

#### 回到外壳自己的窗格——Home 动作

外壳窗体有它自己的 ContentPane 内容：你在携带 SideMenu 的
窗体上画的任何东西。一旦某个菜单项把另一个窗体加载进
那个窗格，外壳自己的内容就在它后面了。**Home** 动作把它带
回来——所以“主屏幕”**不需要它自己的窗体**。

给任何侧边栏项设置 **Home (main content pane)** 动作。它不带
目标，因为它什么也不打开：它只是显示侧边栏所属的
那个窗体。

> **Home 从不销毁。**原先在窗格上的窗体被**停放**，而不是
> 关闭：不触发 `onDestroy`，它的 WORKING-STORAGE 完好无损，而之后
> 再次加载它会让那个实例复活，而不是启动一个新的——
> 这与 *Preserve previous form* 给你的“即时返回”相同。其他每个
> 活动的窗体都不受触碰，包括子窗口：你在 Home 时它们
> 继续运行，并保持它们自己的状态。
>
> 面包屑折叠成只有外壳窗体，因为那是窗格
> 正在显示的内容，而上下文菜单分区出于同样的
> 原因被清空。已经在 Home 时再按 Home 什么也不做——没有 `onDeactivate`，
> 没有 `onActivate`。

⚠️ **Home 只是 SideMenu 的动作。**MenuBar 窗体没有可以恢复的
ContentPane，所以那里不提供该动作。

**被停放的窗体保持活着。**被保留的占用者保留它的存储，并且它启用的
Timer 控件在离开窗格期间继续滴答——定时器处理程序始终
在运行，当窗体的事件队列繁忙时，突发会被合并。

> ⚠️ **注意事项。**无法满足的打开——没有任何东西匹配的窗体 id，
> 或者在构建应用程序时其生成的程序缺失的窗体——
> 会引发可见的运行时错误，并让
> 句柄保持 NULL。请检查你的构建输出中是否有“form … omitted”警告。

---

## 23. Web 与移动应用程序

PowerRustCOBOL 的第 2 版在本指南其余部分所描述的桌面项目之外，又增加了两种**项目种类**。
**Web** 项目把同样的窗体和同样的
COBOL 构建成在浏览器中运行的 **WebAssembly** 应用程序；**Mobile** 项目
把同样的包构建给手机或平板（第 25 章）。语言、Form
Designer、控件目录、主题和 Properties 面板都是你
已经熟悉的——变化的是 **Build 生成什么**、**你的程序的每一部分
在哪里运行**，以及**机密保存在哪里**（第 24 章）。

### 23.1 选择种类

种类在 **File ▸ New Project** 中选择一次。左边的列表提供
*Desktop*、*Web*、*Android phone and tablet* 和 *iOS iPhone and iPad*；它所列出的其他
平台会显示出来，但尚不能创建。它被记录为清单中 `[project]` 之下的 `kind = "web"`
或 `kind = "mobile"`，而且**不会因编辑
清单而更改**——桌面项目不为它写入任何东西，所以每个在
种类出现之前写成的项目都是桌面项目，并原样加载。

| 种类 | 窗体位于 | 窗体文件 | 项目树节点 | Build 生成 |
|---|---|---|---|---|
| **Desktop** | `forms/` | `<name>.cfrm` | Forms | `bin/` 中的原生可执行文件 |
| **Web** | `webforms/` | `<name>.wcfrm` | WebForms | `dist/web/` 中的静态 WebAssembly 包 |
| **Mobile** | `mobileforms/` | `<name>.mcfrm` | MobileForms | 同样的包，然后是 Android 软件包或 iOS 模拟器应用程序（第 25 章） |

这三种窗体文件是同一种格式；只有名称不同，而以另一种
种类命名的窗体（在第 2 版早期制作的 Web 项目中的 `.cfrm`）仍然会加载——Check 会对
它的名称发出警告。每种种类都在 `[files] forms` 之下列出它的窗体。Web 或 Mobile 项目还可以
有一个 **Backend**——`backend/src/` 下的 COBOL——它出现在项目树中
窗体节点之后。Backend 是项目的一部分，而不是单独的一种种类：文件、SQL 和每一个
机密都在那里，因为**Web 包中的任何东西都无法对打开页面的人保密**
（第 24 章）。索引文件定义和 SQL 连接仍然是
项目级的。

编码智能体的 `create_project` 工具接受同样的选择：`kind` 是 `desktop`、
`web` 或 `mobile`，而对于 Mobile 还有 `platform`（`android`、`ios`）和 `device`（`phone`、
`tablet`）。

### 23.2 Web 应用程序是如何构成的

Web 构建**不是**在页面中运行的解释器。编译器直接生成
WebAssembly，而应用程序是两个 `.wasm` 文件和一个宿主页面：

```mermaid
flowchart LR
    subgraph PAGE["页面"]
        HP["宿主页面<br/>HTML、加载器、manifest"]
        PL["播放器<br/>预构建；绘制任何窗体"]
        PM["程序模块<br/>你编译后的 COBOL"]
        HP --> PL
        PL <-- "宿主 API" --> PM
    end
    PM -- "CALL … WITH WEBSOCKET LINKAGE" --> BE["Backend（第 24 章）"]
```

- **程序模块**是你的 COBOL——每个窗体的处理程序、Common Code 和
  copybook——被编译成**整个项目的一个自包含模块**。它携带
  它所需要的（十进制算术、传送、比较、每个打开的窗体
  实例的窗体数据），别无其他：浏览器中没有解释器，没有 COBOL 运行时。每个窗体
  实例的数据位于一个在窗体关闭时被释放的区域中。
- **播放器**是预构建的 WebAssembly 窗体引擎（用 Leptos 绘制），它根据窗体的描述
  绘制任何窗体：用 CSS3 设置样式的 HTML 元素。IDE 把它随可执行文件一起放在
  `assets/web-player/` 中，并把它复制进每个 Web 和 Mobile
  包，所以 **Web 构建不需要 Rust 工具链**。它的*普通*变体在任何地方运行；
  它的*多线程*变体在跨源隔离的页面上运行。Mermaid 图表、PDF
  阅读器和 Canvas 2D 光栅化器是它的一部分，在窗体第一次使用
  它们时加载。
- 每个窗体都作为带版本的 **JSON 描述**
  （`forms/<name>.<hash>.json`）交付，播放器据此绘制。窗体文件本身从不
  交付，而且描述中**没有处理程序代码，也没有凭据值**。播放器
  会拒绝它不读取的描述格式，并指出两个编号。
- **宿主页面**加载播放器，然后加载程序模块——浏览器能运行时
  加载多内存模块，否则加载单内存的那个（加载器会探测）——
  而包带有 Content-Security-Policy 和完整性哈希。

包是一个静态文件的文件夹（`dist/web/`）：把它放在任何以正确的 MIME 类型
提供文件的服务器上。要使用多线程播放器，服务器必须发送
跨源隔离头（`Cross-Origin-Opener-Policy` 和
`Cross-Origin-Embedder-Policy`；项目的 `isolation` 设置选择 `require-corp`、
`credentialless` 或 `off`）。Preview 自己的服务器会替你发送它们。

生成的模块通过一致性运行被保持与解释器相等：NIST
CCVS85 的 NC、IF 和 IC 模块都在生成的代码上运行到零失败（在
2.0.36 测得）。两者仍有差异的地方，`specs/WASM-PARITY.md` 列出每一处差异，并说明
谁是对的。

### 23.3 Web 项目的设置

Project Settings 为这些设置提供了一个 **Web** 页面（每个键都是可选的；
默认值已显示）。在清单中它们是 `[web]` 表：

```toml
[web]
style        = "spa"            # "spa" (single-page) or "standalone" (forms as separate pages)
multi_memory = true             # emit the multi-memory program module
single_memory_fallback = true   # also carry the single-memory module; false = multi-memory only
isolation    = "require-corp"   # "off", "require-corp" or "credentialless": the COOP/COEP headers
backend_url  = ""               # wss://…, or ws:// to this machine only (localhost, 127.0.0.1, ::1)
max_payload_bytes = 393216      # the largest text or JSON frame between page, program and Backend (384 KB)
call_timeout_ms = 60000         # how long a remote call waits for the Backend's answer; then TRANSPORT 1002
loading_effect = "moving-stripes"   # what a control with LoadingEffect on shows during a Backend call
# player     = "2.0.0"          # pin an older player this IDE keeps; absent = the player this IDE ships

[web.browsers]                  # the browsers the build promises to run in; all true
chromium = true
firefox = true
safari = true
ios_webview = true
android_webview = true
```

- **`style`** 是应用程序样式——见 23.6。
- **`multi_memory` 和 `single_memory_fallback`** 选择构建携带哪些程序模块：
  只要 `multi_memory` 开启就携带多内存模块；除非 `single_memory_fallback = false`，
  否则在它旁边携带单内存模块，而多内存关闭时则始终携带。
  在浏览器包括 Safari 或 iOS web view——只运行单内存模块的引擎——时
  选择“只用多内存”会被 Check 和 Build **拒绝**，并指出那些浏览器。
- **`backend_url`** 是页面找到它的 Backend 的地方。`ws://` 地址只被接受
  用于同一台机器，而被拒绝的地址绝不会被写入清单。
- **`max_payload_bytes`** 是在页面和程序之间传递，或在一次调用中
  发往 Backend 的最大文本或 JSON 帧。播放器会在发送之前拒绝更大的请求；
  Backend 保留它自己的限制。
- **`call_timeout_ms`** 是 `CALL … WITH WEBSOCKET LINKAGE`——以及绑定表格的
  读取或保存——等待 Backend 多久，之后以类别 `TRANSPORT`、代码 1002 失败
  （`ON EXCEPTION` 运行；绑定的表格为下一次 `Save()` 保留它的更改）。在 Backend 上
  声明为 `long_running` 的程序有它自己的、更长的限制。
- **`loading_effect`。**其 `LoadingEffect` 属性开启的控件，在任何对 Backend 的调用
  进行期间显示项目的效果，答案到达时就失去它。
  属性选择*哪些*控件显示它；设置选择它们显示*什么*：
  `"moving-stripes"`（在控件上移动的浅色斜条纹，其内容在
  它们后面被模糊）或 `"none"`。
- **`player`** 固定一个 IDE 保留的较旧播放器。固定到 IDE 没有保留的播放器
  会停止 Build 并列出它所保留的播放器。否则重新构建会复制进行构建的
  IDE 的播放器。

### 23.4 Web 上的控件：五个类别

控件目录（第 8 章）被复用。每个控件都按它能在哪里运行
被分类一次：

| 类别 | 含义 | 示例 |
|---|---|---|
| **A** | 变成普通的 HTML 和 CSS；行为来自模型 | Button、TextBox、Label、Hyperlink、CheckBox、ListBox、ComboBox、Panel、TabControl、SideMenu、MenuBar、Slider、TreeView、Timer、Snackbar |
| **B** | 目前由桌面绘制器绘制；在页面中是它自己元素内部的 SVG、`<img>` 或 `<canvas>` | 各种图表、Gauge、Knob、Maps、Viewer、Animator、Canvas2D、Canvas3D |
| **C** | **通过 Backend 工作**：它需要磁盘、数据库、网络或机密 | IndexedFile、RestClient、AgentObject、KnowledgeBase、WebSearch、AWS 控件、FileDropZone、绑定到文件或查询的 DataGrid、Authentication |
| **D** | 需要本地机器，所以某个操作**不可用** | Viewer 的 `Print` 和 `Share`；启动程序、打开终端或打印的工具栏动作 |
| **E** | 在播放器中运行、**没有 Backend** 的客户端对象 | ImageBuffer、Plot、Scene3D、Physics2D、Physics3D |

控件有一个主要类别；在某些操作不同的地方，控件的条目会说明
是哪个。带有内存中 `Rows` 的 DataGrid 是 A 类，而绑定到索引文件或
查询的表格是 C 类。

这对你设计时意味着：

- **工具箱**提供该种类可以使用的控件。**SqlDatabase** 控件
  从 Web 和 Mobile 工具箱中被*撤回*：数据库归 Backend，所以程序
  在 Backend 程序内部用 `COBOL::SqlDatabase` 访问它（24.9）。已经
  持有一个的窗体仍然会加载、构建和运行。
- C 类控件通过生成的 Backend 程序继续工作，所以它的处理程序读起来
  与桌面上一样（`REST-1::Get("customers")`）。第 24 章解释 Build
  生成什么、什么必须是*字面量*，以及保护每个程序的角色。
- D 类操作会被 Check 在那一行拒绝，并附上出路——例如
  *使用浏览器自己的 Print 命令，或 SaveAsPdf 并让用户打印该文件*。
- Properties 面板**隐藏** Web 无法绘制的外观属性，并在它的工具提示中标出
  *近似*的属性：Neumorphic 的双阴影是两个 `box-shadow`，
  `Raised`/`Sunken` 边框变成双色边框，而 3D 倾斜是 CSS 3D 变换。
  其他一切（颜色、渐变、字体、内边距、圆角半径、阴影、对齐、
  自动换行、光标、工具提示、边框）都被精确绘制。
- **图层**的工作方式与桌面一样——叠在窗体上方的平面，在
  程序显示它们之前保持隐藏——采用 HTML 页面的层叠、命中测试和裁剪。

### 23.5 窗体的 COBOL 在页面中能做什么

窗体的处理程序、Common Code 和 copybook 被编译成 WebAssembly，所以你
使用的语言就是第 13 章中的那种，包括算术、`MOVE`、`IF`、`PERFORM`、`EVALUATE`、
`STRING`/`UNSTRING`/`INSPECT`、表、内部函数、嵌套程序、本地 `CALL`，
以及内存中的索引文件和 `SORT … INPUT/OUTPUT PROCEDURE`。十进制算术
是精确的，与桌面上一样。

页面无法持有磁盘、数据库或机密，所以 Build 会**在那一行拒绝
无法在页面中运行的东西**，并说明如何把它办成：

- 文件输入/输出（磁盘文件的 `OPEN`、`READ`、`WRITE`）——*它在后端运行；用
  `CALL … WITH WEBSOCKET LINKAGE` 调用后端程序*；
- `EXEC SQL` 和 `COBOL::SqlDatabase`——数据库归 Backend（类别 `BackendOnly`）；
- 读取文件的 `SORT … USING/GIVING` 和 `MERGE`；
- 窗体的 `SCREEN SECTION`（*请使用窗体*），以及生成器尚未
  覆盖的一些构造（例如 `FORMAT IS …` 子句、national 和 UTF-8 字面量、某些形式的
  `PERFORM VARYING … THROUGH`）——每一个在构建停止时都会被指名。

Build 还会拒绝在某个属性中持有**机密**的控件——SqlDatabase 的
`ConnectionString`、RestClient 的 `AuthToken`、AgentObject 的
`AgentAPIKey`、KnowledgeBase 的 `EmbeddingAPIKey`、WebSearch 的 `ApiKey`——并指出
控件和属性，绝不指出值。请清除它，把值放进
`backend/backend.toml` 的 `[[configuration]]` 中，并把该配置的名称写进控件的
**BackendConfig**（24.8）。

控件属性可以是 `ADD`、`SUBTRACT`、`MULTIPLY`、`DIVIDE`
和 `COMPUTE` 的接收字段，在 Web 上与在桌面上一样（`ADD 1 TO NUD-1::Value`）；这个和作为文本交给
宿主，而这样的接收项不允许 `ROUNDED`。仍有一处差异：商
（`DIVIDE`，或做除法的 `COMPUTE`）在桌面上被截断到属性当前的小数位数
（`8 / 2` 放进保存 `0` 的属性会显示 `4`），而在页面上保留
商自己的刻度（`4.000000000`）——`specs/WASM-PARITY.md` 记录了它。

### 23.6 导航：单页还是独立

应用程序样式（`[web] style`，在 New Project 时选择，并可在 Build 之前更改）
是 **`spa`**（默认）或 **`standalone`**。同样的 COBOL 在两者中都运行；只有
打开动词所显示的内容不同。

- **单页应用程序（`spa`）**——一个浏览器页面。`OpenFormSync` 把窗体显示为
  模态对话框，调用者等到它被关闭，与桌面上一样；`OpenFormAsync`
  把它显示为打开者之上的面板，调用者继续运行。**SideMenu** 项把它的
  窗体加载进 **ContentPane**，所以第 22 章的应用程序外壳的工作方式与
  桌面上一样。
- **Standalone**——每个窗体都是它自己的浏览器标签页。`OpenFormSync` 和 `OpenFormAsync`
  打开新的标签页，而 `OpenFormSync` 的调用者等到那个标签页被关闭。拦截
  新标签页的浏览器是正常的失败：调用者被释放，一条通知告诉
  用户允许弹出窗口，并且在打开者的窗体上触发 **`onPopupBlocked`**，
  带有未被打开的窗体的名称。在窗体名称之后传入参数 **`pr-window=same`**，
  可以替换页面而不是打开标签页（`pr-window=new` 是
  默认值）；窗体随后取代这个页面的位置，Back 回到页面自己的地址，
  没有打开者，`super::` 会失败。

`FormFormat` 保持它的含义：Build 会拒绝以字面量指名 `Embedded` 窗体的
`OpenFormSync` 或 `OpenFormAsync`，以及加载 `Standalone` 窗体的菜单项。以
数据项指名的窗体在它运行时被检查：打开失败，并且语句的
`ON EXCEPTION` 运行。

**地址是一条路由。**单页应用程序把最上面的窗体写在页面的
地址中，形如 `#/form-name?name=value`，所以 Back 关闭它，Forward 再次打开它，
而被收藏或粘贴的地址会用**同样的打开参数**
（`GetParam`、`GetParams`）打开同一个窗体——绝不包括它所持有的数据，数据要由程序在
`onLoad` 中再次加载。没有指名应用程序任何窗体的地址会让主窗体保持显示，显示
一条通知，把地址恢复，并在主窗体上触发 **`onRouteNotFound`**，
带有地址所给的名称。

**未保存的更改。**`INVOKE me::"SetUnsavedChanges"(1)` 把窗体标记为持有尚未
保存的工作，而 `(0)` 清除标记（窗体被销毁时它的标记也会消失）。只要
任何显示的窗体被标记，关闭标签页就会问浏览器自己的问题，而 Back 或会
离开该窗体的菜单项会先问*Leave* 还是 *Stay*。在桌面上，该动词被
接受而什么也不做。

```cobol
           INVOKE me::"SetUnsavedChanges"(1).
           *> … the record is saved …
           INVOKE me::"SetUnsavedChanges"(0).
```

**跨标签页的 `super`（standalone）。**在自己的标签页中打开的窗体仍然通过
`super::` 访问它的打开者（第 22 章）：写入和过程调用会传到打开者的标签页，而
`MOVE`、算术语句、`IF`、`DISPLAY` 等之中的**读取**会**等待
打开者自己的回答**——处理程序在那条语句处停下，页面保持活动，
而语句在回答到达时运行。打开者在调用时间
限制（30 秒）内没有回答的读取，会像关闭的打开者一样失败，而 `INVOKE super::Method(…)` 可以携带
应对这种失败的子句：

```cobol
           INVOKE super::GetFormState() RETURNING WS-STATE
               ON EXCEPTION
                   MOVE "gone" TO LBL-OUT::Caption
               NOT ON EXCEPTION
                   MOVE WS-STATE TO LBL-OUT::Caption
           END-INVOKE
```

两个标签页必须是同一个浏览器配置文件中的同一个站点。在 Web 应用程序中，该子句
只在 `INVOKE super::…` 上被接受。
### 23.7 用 CSS3 设置样式

有两类 CSS 文件为页面设置样式；桌面项目两者都没有。**项目
样式表**是 `[web] stylesheet`（默认为 `webforms/application.css`；Mobile 项目为 `mobileforms/`）；每个**窗体样式表**是
窗体旁边的 `<Form>.css`。新项目一开始就带有两者，新窗体得到它自己的，被移动的窗体带着它的样式表一起走，
而删除窗体会删除未被改动的样式表，并在样式表已被编辑时先询问。

窗体的样式表由它的主题生成：一个横幅，窗体所使用的控件
类型的主题规则位于 `@layer pr-theme { … }` 之内，介于两个标记注释之间，然后是
一个**归你的**空白部分。切换主题只有在该块未被改动时才会替换它；
否则 IDE 会询问。你的部分绝不会被重写。从窗体的 **Stylesheet…**
按钮编辑窗体的样式表——一个带有行号、着色以及按行显示发现项的代码窗口。

**层叠**，从最低到最高：

1. 播放器的结构（`@layer pr-base`）；
2. 主题（`@layer pr-theme`）；
3. 控件的**属性**（`@layer pr-props`）；
4. **项目样式表**（`@layer pr-app`）；
5. 位于其生成块之外的**窗体样式表**——未分层，作用域限于那个窗体，并
   包括选择 `CssClass` 名称的规则；
6. 控件的 **`CssStyle`**。

任何未分层的规则都胜过每一条分层的规则，所以**样式表中的普通规则胜过
属性**——*CSS 胜出*。`CssStyle` 是在每个样式表之后、每个控件一条的规则（真正的内联
`style` 属性会破坏页面的 Content-Security-Policy）。窗体样式表留在
它的窗体之内；无法被保留在其中的样式表会被拒绝，绝不会应用于整个页面。
窗体根和每个图层元素保留它们的 `position`、`z-index`、`display` 和
`visibility`，所以样式表无法重新叠放、隐藏或禁用图层——试图这样做的规则
是 Check 和 Build 时的错误，并带有它的行。

Check 和 Build 会读取每个样式表：不平衡的花括号、未终止的字符串、`@import`、
`expression(`、`javascript:`、`behavior:`，以及既不属于包也不是
`data:` 的 `url()` 都是**错误**；未知的属性、无法解析的值，以及项目的浏览器
之外的厂商前缀则是警告。

**设计器显示页面的值。**在 Web 或 Mobile 窗体上，画布绘制
样式表所决定的颜色、边框和阴影，采用工具栏的 *Light / Dark* 按钮所选择的
配色方案（仅是一种视图）。Properties 面板显示每个属性**当前生效**的
值，并带有一个 **CSS** 标记，指出决定它的样式表和行；
可能适用的规则（位于 `@media`、伪类……之下）被标记为*可能被覆盖*。
更改由某条规则决定的值，会把新值写入那条声明中，保持你的
写法，作为一步撤销；右键点击 CSS 标记可选择 **Go to the rule** 和 **Remove the
CSS declaration**。Preview 打开时，**Apply** 会把样式表直接放进提供服务的
包中，而页面就地采用它。

### 23.8 Build、Run 和 Preview

在 Web 或 Mobile 项目上，**Build** 按钮构建的是包而不是二进制，并
说明花了多长时间、它有多大、播放器及其各部分有多大、它启动得有多快，
以及有什么发出了警告。（编码智能体的 `build` 工具做同样的事；`rcrun build`
生成原生二进制。）

设计器的 **Preview** 按钮会构建，在回环端口上以正确的
MIME 类型和隔离头提供该包，并在它自己窗口中的**系统 web view** 里显示它——一个辅助进程，
或者在辅助程序未安装时是系统浏览器。它的控件窗口选择一个**设备**（手机、平板或桌面尺寸）、**方向**、
**像素比**、**配色方案**、**页面语言**和**安全区域内边距**。
页面的控制台和未捕获的错误会到达 Output 面板。对窗体、
样式表、Common Code 或 Backend 的更改会重新构建，而打开的页面**就地**采用新的程序
模块、窗体描述和样式表——应用程序在
同一个页面中重新启动，页面不会被重新加载。Preview 只显示它自己的地址：试图
打开另一个地址的页面会被停止，并提供在浏览器中打开它的选项。**Run** 在系统浏览器中
打开同一个服务器。

**带有 Backend 的项目。**页面调用它的 Build 写入其中的 Backend 地址，所以
Run 和 Preview 会先启动项目的 Backend（`rcrun backend run`，它的日志在 Output
面板中），等待它说出它在哪里监听——最多 60 秒——然后才构建
并提供页面。`backend_url` 为空时，Backend 在这台机器上监听
`backend/backend.toml` 的端口，或一个空闲端口；设置了它时，则遵从它。你自己启动的
Backend（**Run ▸ Start Backend**）原样使用，而 **Stop**、关闭 Preview、
项目或 IDE，会停止由 Run 启动的那一个。不会往页面里放任何令牌：以这种方式
启动的页面可以访问标记为 `public = true` 的导出。

### 23.9 Build 在安全方面做什么

- 它会**扫描你项目自己的文件**——窗体、Common Code 和 copybook、样式表、
  文本资源以及 `backend/backend.toml`——查找机密：私钥、云和 API 密钥、
  令牌、带密码的连接字符串，以及名称像机密并保存了
  值的属性。它所认出的一个会停止 Build，指出文件、行和模式，绝不指出
  值；只是看起来像（高熵）的字符串则是警告。Backend 的
  COBOL 不被扫描：那是从环境中读取机密的地方。
- 包带有一个 **Content-Security-Policy**，它恰好允许你的
  样式表所加载自的来源，还有完整性哈希和隔离头。
- `[security]` 表会变成 `/.well-known/security.txt`（RFC 9116）：`contact` 和
  `expires` 是必需的，`policy` 和 `preferred_languages` 是可选的。Check 会拒绝没有它们的表，
  或其 `expires` 已过的表，而 Backend 在
  同一路径上提供同样的文本。
- `[web.unsafe]` 表（手机上是 `[mobile.unsafe]`）有三个键，默认全部关闭——
  `plain_ws`、`skip_secret_scan`、`csp_report_only`。每个都会削弱一项保护，
  在开发构建中是**警告**，在发布构建中是**拒绝**，并指出
  该键并要求把它移除。
- `[mobile.pinning]` 表固定 Mobile 应用程序 web view 的证书
  （第 25 章）。

### 23.10 示例

仓库在 `examples/` 下附带四个新种类的项目。从
**File ▸ Open Project** 打开其中一个，然后按 **Preview** 或 **Run**：

| 示例 | 种类 | 它展示什么 |
|---|---|---|
| `PowerWebHello` | Web | 最小的 Web 项目：一个带侧边菜单的窗体，以及一个 Backend |
| `PowerMobileHello` | Mobile | 最小的 Mobile 项目 |
| `PowerOficina` | Web | 一个车间管理应用程序——队列、报价、审批、付款、报表——带有一个基于索引文件的 Backend |
| `PowerAnalyticsWeb` | Web | 桌面 PowerAnalytics 仪表板在 Web 上的外观 |
---

## 24. Backend 与安全通道

网页是**任何打开它的人都可以读取**的代码——它的程序模块、窗体
描述、样式表。所以 Web 或 Mobile 应用程序把所有不能被
读取或篡改的东西——文件、数据库、密钥、决定金钱的业务规则——保存在
**Backend** 中：运行在**原生** RustCOBOL 运行时上的 COBOL（带有文件、索引
文件、SQL、HTTP 和线程），位于你控制的服务器上。页面用一条
语句 `CALL … WITH WEBSOCKET LINKAGE` 访问它。

```mermaid
flowchart LR
    PG["页面：你的窗体的 COBOL<br/>WebAssembly 模块"] -- "CALL 'NAME' WITH WEBSOCKET LINKAGE<br/>JSON、bearer 令牌、加密帧" --> BE["Backend：导出的程序<br/>原生运行时"]
    BE --> DATA[("索引文件 · SQL · REST · AWS")]
    BE -- "登录、注册、重置" --> IDP["身份提供商"]
```

### 24.1 Backend 在磁盘上是什么

Web 或 Mobile 项目一开始就带有一个 Backend（项目也可以不带它而创建）。它的
文件是：

| 路径 | 它保存什么 |
|---|---|
| `backend/src/` | 你的 COBOL 程序——你导出的程序，以及它们 `CALL` 的过程 |
| `backend/COPYBOOKS/` | Backend 程序共享的 copybook |
| `backend/exports.toml` | **调用者可以访问的程序的唯一清单**，以及契约版本 |
| `backend/backend.toml` | Backend 在哪里以及如何运行：地址、TLS 身份、允许的来源、令牌签发者、限制、配置、身份提供商 |
| `backend/contract/interface.json` | Backend 的接口描述，在构建 Backend 时写入——Check 用它来比较你的 `CALL` |
| `backend/contract/released/<version>.json` | 每个已发布的契约（24.7） |
| `backend/generated/` | Build 为你写入的程序（24.8）：每次 Build 都会被替换，绝不编辑 |

新项目一开始，`backend/exports.toml` 中列有 `HEALTH`（公开），并有一个开发用的
`backend/backend.toml`（回环，没有机密）。在 IDE 中，项目
树的 **Backend** 节点会列出它，而项目的设置页面（Web 项目的 **Web** 页面，Mobile 项目的 **Mobile
application**）有 *Backend address*、*Exported programs*（它们被
写入 `exports.toml`）、*Backend port (IDE run)* 和 *State between calls*（24.6）。

### 24.2 导出一个程序

Backend 程序是普通的 COBOL。下面是 `PowerWebHello` 示例所导出的那个：

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-NAME                                PIC    X(30).
       01 LK-GREETING                            PIC    X(40).
       PROCEDURE DIVISION USING LK-NAME RETURNING LK-GREETING.
           MOVE    SPACES                        TO    LK-GREETING
           STRING  "Hello, " DELIMITED BY SIZE LK-NAME DELIMITED BY SPACE INTO LK-GREETING
           GOBACK.
```

以及它在 `backend/exports.toml` 中的条目：

```toml
[contract]
version = "1.0.0"

[[export]]
program = "HELLO"
public = true
```

**只有导出的程序才能被调用**，不管它叫什么名字；其他每个程序都是
导出的程序所 `CALL` 的普通过程。`[[export]]` 条目可以说明：

| 键 | 含义 |
|---|---|
| `program` | PROGRAM-ID |
| `roles` | 可以调用它的角色——其中任何一个就够了；为空表示任何已认证的调用者 |
| `public` | 完全不带令牌地应答 |
| `independent` | 不在调用者的其他调用之后排队 |
| `idempotent` | 在连接丢失之后可以安全地再发送一次（24.6） |
| `long_running` | 可以运行 `long_call_timeout_ms`（10 分钟），并报告进度（24.6） |
| `stream` | 被订阅，而不是被调用（24.6） |
| `directions` | 每个参数的 `in`、`out` 或 `inout` |
| `deprecated` | `{ message, removal }`：程序仍然工作，并带有警告 |
| `status` | 它的 `RETURNING` 数据项之内的状态组的名称（24.4） |

在导出的程序内部，`COBOL::"WS-CLAIM" ( "sub" WS-USER )` 读取调用者已验证令牌的
一个声明（claim）——`sub`、`email`、`roles`、任何标量声明，用带点的名称
如 `realm_access.roles` 表示的嵌套声明——而在令牌缺少它时给出空格。请用它，以及
条目的 `roles`，来决定*谁可以做什么*：**边界是 Backend 的令牌
检查**，绝不是页面。

### 24.3 从窗体调用它

```cobol
           *> GET-CUSTOMER: asks the backend for one customer; CUSTOMER-REC carries the key and receives the record.
           *> Uses:   CUST-ID of CUSTOMER-REC = the key to look up.
           *> Result: CUSTOMER-REC = the record the backend sent back; RESULT-REC = the outcome.

           CALL "GET-CUSTOMER" WITH WEBSOCKET LINKAGE
               USING BY REFERENCE CUSTOMER-REC
               RETURNING RESULT-REC
               ON EXCEPTION
                   COBOL::"WS-LAST-ERROR" ( WS-CLASS WS-CODE WS-MESSAGE )
                   SET LBL-STATUS::Caption TO WS-MESSAGE
               NOT ON EXCEPTION
                   SET LBL-STATUS::Caption TO "Found"
           END-CALL
```

- **参数是组。**`USING` 的每个操作数以及 `RETURNING` 接收项，都是
  在 WORKING-STORAGE（或 LINKAGE）中声明的数据项，写成它的名称——绝不是
  字面量、表达式、表元素或文件记录。组作为按它自己的声明
  构建的 JSON 对象传输；基本数据项作为光秃秃的数字或字符串传输。
  `BY VALUE` 发送该组并绝不更改你的副本；`BY REFERENCE`（默认）还会
  把 Backend 的最终值写回其中；`RETURNING` 把 Backend 返回的
  数据项写入具名的组。这里没有 `BY CONTENT`——请写 `BY VALUE`。
- **其他都不归你。**连接、持有者令牌、分帧、
  关联、保活、重新连接和令牌刷新都属于运行时。
  调用挂起期间，窗体继续重绘，它的事件按顺序排队；
  `CALL` 之后的语句要等调用完成才运行。
- **在 Check 时**会导入 Backend 的接口描述：名称必须是
  Backend 导出的程序（`ON EXCEPTION` 并不能豁免契约中没有
  列出的名称），参数个数必须匹配，而且每个组必须适合它所落到的参数——
  名称与声明一致，以及容量、刻度、符号和 `OCCURS`。

**组到 JSON 的映射。**成员的键是它**按声明的**数据名，包括
大小写。`FILLER`、88 层条件名、66 层 `RENAMES` 以及
`REDEFINES` 的重定义一半都不是成员。成员**按名称**读取，顺序不限；
答案中省略的键会让它的数据项保持不变。

| COBOL | JSON |
|---|---|
| 组 | 对象，成员按声明顺序 |
| `OCCURS n` | 含 *n* 个元素的数组 |
| `OCCURS min TO max DEPENDING ON item` | 当前计数个元素的数组；计数数据项必须是在同一个组内更早声明的整数，读取数组会设置它 |
| 数值数据项 | 按它的十进制文本和 PICTURE 的刻度写成的数字（保存 12.5 的 `PIC S9(5)V99` 是 `12.50`）——绝不经过二进制浮点数，所以 38 位数字都能保留；`COMP`、`COMP-3`、`COMP-5` 和 `PACKED-DECIMAL` 按它们的值传输 |
| `PIC X(n)` | 最多 *n* 个字节的 UTF-8 字符串，去掉尾随空格 |
| `PIC N(n)`、`PIC U(n)` | 按 UTF-16 单元计数或按字符计数的字符串 |

`COMP-1`/`COMP-2`、`INDEX`、`POINTER` 和 `OBJECT REFERENCE` 数据项在 Check 时被拒绝。
**读取是严格的：**未知的键、JSON 类型错误的值、不适合它的 PICTURE 的数、
比它的 PIC 更长的字符串，或比它的 `OCCURS` 更长的数组，是
**指出数据项的错误**，绝不是截断——并且当答案的任何一部分被拒绝时，
不会写入任何东西。

### 24.4 调用失败时

当调用无法完成——没有连接、超时、令牌缺失或过期、
角色被拒绝、载荷不合适、未知的程序、版本不匹配、限制、
Backend 的程序所报告的失败，或 Backend 故障——时，`ON EXCEPTION` 运行；没有它时，
失败的调用是运行时错误。调用成功时 `NOT ON EXCEPTION` 运行。

`COBOL::"WS-LAST-ERROR" ( class code message [detail] )` 说明原因。类别是
`TRANSPORT`、`AUTHENTICATION`、`AUTHORISATION`、`VALIDATION`、`NOT-FOUND`、`VERSION`、`LIMIT`、
`APPLICATION`、`SERVER` 之一，而代码是稳定的：

| 代码 | 含义 |
|---|---|
| 1000 | 没有连接——调用从未发出 |
| 1001 | 连接在调用发出**之后**丢失——它可能已经被收到 |
| 1002 | 超时（`call_timeout_ms`） |
| 1003 | 没有配置 Backend |
| 2000 – 2002 | 令牌缺失、无效、过期 |
| 3000 | 角色被拒绝 |
| 4000 | 载荷不合适 |
| 5000 | 未知的程序 |
| 6000 – 6002 | 版本 |
| 7000 – 7004 | 限制 |
| 9000 – 9002 | 服务器 |

调用失败之后 `RETURN-CODE` 保存同样的数字代码，调用成功之后为 0。

**Backend 程序自己报告的失败。**`COBOL::"WS-FAIL" ( 4201 "card declined"
'{"retry":false}' )` 结束调用：调用者的 `ON EXCEPTION` 运行，类别为
`APPLICATION`，带有代码 4201、消息和详情，而程序
所更改的参数**不会**被送回。程序自己的代码是 1 到 7999（8000 是平台的
*已取消*）。支撑生成的或智能体编写的窗体的程序只用 `WS-FAIL` 报告业务
失败；手写的程序可以改为在它的
`[[export]]` 条目中声明 `status = "WS-STATUS"`，指名它的 `RETURNING` 数据项之内的一个组，
布局为代码 `9(4)`、消息 `X(200)` 和详情 `X(1000)`——当程序结束时其中的代码非零，
调用就像 `WS-FAIL` 那样失败。该声明是契约的一部分。

**连接丢失。**连接断开时，运行时**三次**重试，
分别等待 **10、20 和 30 秒**；其间应用程序被报告为阻塞，调用自己的
超时不运行，新的调用等待。某次尝试成功时，等待的调用
继续进行（已经发出的调用只有在它的程序是 `idempotent` 时才会重复，否则它会以
1001 失败）。第三次尝试失败时，每个等待的调用都到达 `ON EXCEPTION`——已发出的是 1001，
从未发出的是 1000——并且在应用程序的下一次调用之前不再尝试任何东西。没有什么需要设置的。

**在浏览器中**，播放器在尝试期间阻塞应用程序：整个
窗体上有一个模态，标题为 *Connecting to the server…*（用页面的语言），连接一旦
成功它就消失，并让窗体保持原样。第三次尝试失败时，
它会说 *The server could not be reached.*，并带有 **Reload** 按钮，而页面
最初启动时的那个窗体会接收 **`onBackendUnreachable`**——请绑定它来告知用户、记录日志，或提供
别的办法。在手机上，同样的模态会在屏幕位置绘制手机（或平板）；
尝试放弃之后从后台回来，会立即再次尝试。
桌面不绘制模态，也尚不触发该事件。

### 24.5 安全通道

通道是**基于 TLS 的 WebSocket**（`wss://`；普通的 `ws://` 只被接受用于
回环地址），每个请求都带有持有者令牌（JWT），并且**在 TLS 之上再次加密**。没有
什么需要设置，也没有什么需要写：连接打开时，页面和
Backend 各自生成一个新的密钥对，在两条 `hello` 消息中交换它们的公钥，
并为每个方向商定一个密钥（X25519、HKDF-SHA256、ChaCha20-Poly1305）；之后的每条消息
都作为加密帧传输，所以终结 TLS 的代理，或者对先前会话的录制，
什么也读不到，也重放不了。任何密钥都绝不被存储、记录或放进构建中。在线路协议
更改的两侧各自构建的页面和 Backend 会以*版本*错误互相拒绝：请
两者都重新构建。

令牌从你的身份提供商获得（24.10），由运行时管理，并且
你的 COBOL 绝不可见；过期的令牌会被换成新的，并且调用会被重复，
一次。除了 WebSocket 之外，Backend 还回答 `GET /health`；其他每个路径都是 404。

**应用程序在哪里找到它的 Backend。**Web 或 Mobile 项目的 `backend_url`（23.3）。
桌面或构建出的应用程序、`rcrun run` 和 `rcrun run-form` 读取环境
变量，只读一次，而应用程序的每个窗体共享一个连接：

| 变量 | 保存 |
|---|---|
| `COBOLT_BACKEND_URL` | `wss://` 地址（`ws://` 只用于回环） |
| `COBOLT_BACKEND_TOKEN` | 令牌，或 `env:NAME` / `file:PATH` |
| `COBOLT_BACKEND_INTERFACE` | Backend 的 `interface.json` |
| `COBOLT_BACKEND_CERT` | 再多一个受信任的证书，PEM |

### 24.6 重复调用、长调用、流和文件

- **重复调用。**运行时给每个调用一个*幂等键*。连接丢失之后，
  声明为 `idempotent = true` 的程序会在新连接上再发送一次；
  任何其他调用会以 `TRANSPORT` 1001 失败，并且*可能已经运行*。立即再做一次
  同样的 `CALL`，它会重用该键，于是 Backend 用它所保存的内容作答
  （保存 `idempotency_ttl_seconds`，600），**而不会第二次运行程序**；同一个
  键配上其他数据则会被拒绝。超时、限制和取消不会被保存，所以可以
  再次尝试。一个连接的调用按它们被发出的顺序运行，除非程序
  是 `independent`。`backend.toml` 的 `[contract]` 保存 `idempotency_store`（`memory`，
  默认值，或 `server`，它还会把每个答案写入文件，以便重启之后重复的调用仍能被识别）
  和 `session_idle_seconds`（900）。
- **长调用。**声明为 `long_running = true` 的程序可以运行
  `long_call_timeout_ms`（10 分钟），并用 `COBOL::"WS-PROGRESS" ( percent
  message )` 报告。调用者请求取消；程序把它看作 `COBOL::"WS-CANCELLED" ( flag
  )` = `Y`，而 `WS-PROGRESS` 是调用结束的安全点。没有到达
  安全点的程序会在请求之后 `cancel_grace_ms`（10 秒）被停止。被取消的调用会以
  `APPLICATION` 8000 失败；程序已经做的事情不会被撤销。
  `COBOL::"WS-PAUSE" ( milliseconds )` 等待，并在取消时提前结束。当窗体先用
  `COBOL::"WS-SHOW-PROGRESS"` 指名它们时，它会显示长
  调用的进度和一个 Cancel 按钮。
- **流。**声明为 `stream = true` 的程序是被*订阅*的：每次订阅运行一次，并用
  `COBOL::"WS-EMIT" ( group )` 发送每个条目，直到它结束、客户端
  取消订阅或断开连接。窗体用 `COBOL::"WS-SUBSCRIBE"` 开始一个，并通过
  `onStreamItem` 和 `onStreamEnd` 听到它（`COBOL::"WS-STREAM-ITEM"` 读取每个条目）。
  跟不上的订阅者会以 `LIMIT` 错误结束，而不是被无限制地缓冲。
- **文件。**上传进入部署在 `[files]` 中声明的一个*槽*（名称、
  它接受的 mime 类型、可以使用它的角色），分块进行，大小和 SHA-256
  先声明；丢失的连接会从停下的地方继续；文件会被检查（大小、
  校验和、图像、PDF 和 ZIP 的前几个字节与声明的类型对照）并
  以 Backend 自己的名称存放。`COBOL::"WS-FILE-PATH" ( file-id path status )` 给程序
  *调用者*上传的文件的路径；`COBOL::"WS-FILE-PUBLISH" ( path file-id status )`
  把文件作为与该调用者绑定的短期地址（`download_ttl_seconds`，60）提供给
  调用者。在窗体中，`COBOL::"WS-UPLOAD" ( slot path mime file-id status )` 和
  `COBOL::"WS-DOWNLOAD" ( file-id path status )` 回答 `OK`、`NOFILE`、`REFUSED`、`FAILED` 或
  `NOBACKEND`。
- **调用之间共享的文件**互相等待：以输出方式打开索引文件的程序
  在它的 `CLOSE` 之前独占它，读取者共享它，而在
  `file_lock_wait_ms`（5000）之内无法获得文件的 `OPEN` 会回答文件状态 93。请在对文件的工作
  一完成就关闭它。

**调用之间的状态**是项目的 `[backend] state`。`per-call`（默认）在
全新的解释器中运行每个调用：没有 WORKING-STORAGE 能存留，文件和数据库是唯一
共享的状态。`per-connection` 在连接存续期间为每个导出的程序保留一个运行单元，
所以 WORKING-STORAGE 会从该连接的一个调用延续到下一个；
连接的调用依次运行，连接只服务它第一次所服务的用户，
而连接结束时不保留任何东西。

### 24.7 契约及其版本

导出的程序及其参数就是 Backend 的**契约**，在 `exports.toml` 中按
`major.minor.patch` 编定版本。旧页面要在新 Backend 上继续工作，
仅当新接口仍然接受旧接口所发送的内容，并且仍然携带旧页面所读取的每个成员：
添加成员或程序，或加宽只*传向* Backend 的数据项，是兼容的；移除或重命名成员（在请求**或**答案中）、
更改刻度或符号、缩窄 picture、加宽 Backend 也会送回的数据项、
移除程序，或更改谁可以调用它，是**破坏性的**，需要新的主
版本。

- `rcrun backend describe` 写入并打印 `interface.json`；`rcrun backend release`
  把当前契约记录为已发布（`contract/released/<version>.json`）；
  `rcrun backend check` 把契约与已发布的契约比较。当**发布**构建
  破坏了最近一次已发布的契约时，它会被拒绝。
- 页面在连接时说明它所对照构建的契约。主版本相同而次版本较低的，
  会按它被构建时的形态提供服务（自那以后添加的成员不会被发送），并被告知要
  更新；比 Backend 的次版本更高，或另一个主版本，会被拒绝并给出
  更新消息。
- 窗体会听到这一点：**`onBackendVersion`**（Backend 服务了较旧或较新的
  应用程序；读取 `COBOL::"WS-VERSION"`）和 **`onBackendWarning`**（已弃用的程序或
  较旧的应用程序；读取 `COBOL::"WS-WARNING"`）。

### 24.8 通过 Backend 的数据控件

C 类控件（23.4）通过生成的 Backend 程序工作，所以你的处理程序读起来
与桌面上一样。对每一个，Web Build 会把程序写入
`backend/generated/`，其 `[[export]]` 条目位于 `backend/generated/exports.toml`，
可由控件的 **BackendRoles** 所指名的角色调用（默认是任何已认证的调用者，
**绝不公开**），并在每次 Build 时重新生成。**不要在那里保留任何你自己的东西**：
Build 会替换并移除带有它的横幅的文件，并拒绝
覆盖没有横幅的文件。

- **绑定的 DataGrid**（绑定到索引文件或 SQL 查询）只通过每个表格两个程序来读写，
  `…-READ`（一个行窗口——最多 100 行，每行最多 32 个单元格——连同
  总数），以及对于可写的绑定，`…-SAVE`（被删除、更改和添加的行作为
  **一个事务**写入：每个更改先被检查，若有一个失败则全部撤销）。
  表格的 `RefreshBinding()` 一个窗口一个窗口地请求，直到数据源耗尽；`Save()`
  发送该单元。**在浏览器中由播放器发出调用**，所以 `RefreshBinding()` 回答
  表格*当下*显示的计数，而 `Save()` 在该单元还在途中时回答 `98`：请处理
  **`onSaved`** 和 **`onCommitError`**，以便在桌面和 Web 上同样地了解保存的结果，
  而不要把 `Save()` 的回答读作结果。绑定到 COBOL 表的表格从不
  调用 Backend；绑定到 REST API 或智能体数据源的表格目前尚不能在 Web 上运行。
- **IndexedFile 控件**，若它指名一个 `.cidx` 且你的窗体程序
  `SELECT` 了它的文件，就会得到九个程序（`OPEN`、`CLOSE`、`START`、`READ-NEXT`、`READ-PREVIOUS`、
  `READ-INVALID`、`WRITE`、`REWRITE`、`DELETE`）；Build 把文件上的每个动词变成已解析程序中的一个
  `CALL … WITH WEBSOCKET LINKAGE`——你的源码绝不会被触碰。
  Backend 在调用之间不保留任何东西：每个程序打开文件，执行它的动词，然后关闭
  它，所以写入在调用回答时就是持久的；游标由浏览器保存。文件
  状态落在控件的 `StatusDataItem` 中，与桌面上一样，而 `30` 表示调用
  无法发出（原因在 `WS-LAST-ERROR` 中）。
- **C 类控件的方法**——对 RestClient、AgentObject、KnowledgeBase、
  WebSearch、Maps、SqlDatabase 或 AWS 控件的调用（`REST-1::Get("customers")`）——每个调用点
  变成一个 Backend 程序。Backend 先用控件设计时的
  属性（绝不是保存机密的那些）为它播种，然后用
  `backend/backend.toml` 中控件的 **BackendConfig** 所指名的 `[[configuration]]` 播种：

  ```toml
  [[configuration]]
  name = "CRM"
  BaseURL = "https://crm.example.com/api"
  AuthToken = "env:CRM_TOKEN"
  ```

  任何值都可以是 `env:NAME` 或 `file:PATH`；没有任何一个会被打印或送回。浏览器
  本来可以选择的内容**在 Build 时就固定了**：SQL 语句、REST 资源、
  函数、工具或作业名称必须在处理程序中是*字面量*，而当它不是、当调用传入连接字符串，
  或当它传入本地文件时，Check 和 Build 会在它的行停止。
  答案之后，调用的值会用在处理程序调用该方法的位置，
  Backend 的控件所更改的属性会设置到窗体的控件上，它的 `LastError`
  会被设置，而它所触发的事件（`onComplete`、`onQueryComplete`……）会在窗体的
  控件上触发——它们的处理程序在运行中的处理程序**之后**运行，与桌面上一样。当调用
  失败时，`LastError` 保存原因，并触发 `onError`。桌面上会有条件地或不止一次地
  发出的调用（在 `PERFORM … UNTIL` 条件、`EVALUATE` 的 `WHEN`、
  `AND`/`OR` 中）会被拒绝：请把它移到它自己的语句中。
- **FileDropZone** 得到一个只接受它的 `AllowedExtensions`、
  且最多为它的 `MaximumFileSizeKB` 的上传槽；**Viewer** 得到一个供用户打开的文档使用的槽，
  在 Web 窗体上带有同样的两个属性。`SaveAs` 和 `SaveAsPdf` 不经过 Backend：在 Web 上
  它们把文档下载给用户。
- **AWS 控件**与其他控件一样是 C 类：对它们某条路由的调用（DynamoDB
  `GetItem`、S3 `List`、Lambda `Invoke`……）在 Backend 上运行，而控件只保存
  连接的*名称*，绝不保存密钥。
### 24.9 数据库：`COBOL::SqlDatabase`

SqlDatabase 控件不在 Web 和 Mobile 的工具箱中。Backend 程序——或任何
在原生运行时上运行的程序——使用 **`COBOL::SqlDatabase`**，即该控件自己的动词，
作用在每个程序都有的一个对象上：窗体上没有控件，没有 `EXEC SQL`，没有宿主变量。它
运行在 SQLite、PostgreSQL 和 MySQL 上。

```cobol
           MOVE COBOL::SqlDatabase::Open("sql-connection:SALES") TO WS-HANDLE
           IF WS-HANDLE = 0
               *> The connection did not open: LastError says why.
               MOVE COBOL::SqlDatabase::LastError TO WS-MESSAGE
           ELSE
               MOVE COBOL::SqlDatabase::Query("SELECT id, name FROM customer ORDER BY id") TO WS-ROWS
               PERFORM UNTIL 1 = 2
                   MOVE COBOL::SqlDatabase::Fetch() TO WS-ROW
                   IF WS-ROW = SPACES
                       EXIT PERFORM
                   END-IF
                   *> WS-ROW holds one row, its columns separated by a TAB.
                   PERFORM SHOW-ROW
               END-PERFORM
               COBOL::SqlDatabase::Close()
           END-IF.
```

| 动词 | 回答 |
|---|---|
| `Open(connection)` | 句柄，无法连接时为 `0`；不带参数的 `Open()` 通过 `SqlConnection` 属性（某个项目 SQL 连接的名称）连接，否则通过 `ConnectionString` |
| `Execute(sql)` | 它更改的行数（失败时为 `0`） |
| `Query(sql)` | SELECT 的行数 |
| `Fetch()` | 下一行，作为文本，其列以 TAB 分隔——行取完之后则是空格 |
| `FetchAll()` | 行数 |
| `ColumnNames()`、`ColumnCount()`、`ColumnName(n)` | 名称（以 TAB 分隔）、有多少个、第 *n* 个（从 1 开始） |
| `BeginTransaction()`、`Commit()`、`Rollback()` | 生效时为 `1`，无法生效时为 `0` |
| `IsInTransaction()` | `1` 或 `0` |
| `Close()` | 关闭连接 |

`LastError`（数据库的消息）和 `StatusCode`（成功的 `Open` 之后为 `0`，失败的
之后为 `1`）像任何属性一样读取。连接的命名方式与 `EXEC SQL CONNECT TO` 相同：
`':memory:'`、SQLite 路径、`sqlite:path`、`postgres://…`、`mysql://…`，或者对
项目的某个 `[[sql-connections]]` 用 `sql-connection:NAME`。该对象保存一个连接——最后
打开的那个——与任何 `EXEC SQL` 连接互不相干。

> **没有参数。**`Execute` 和 `Query` 只接受语句的*文本*，别无其他：没有 `?`
> 标记，没有宿主变量。请用 `STRING` 从程序信任的值构建文本，
> 拒绝包含单引号的键入值，并在语句接受来自人的输入的地方
> 使用带宿主变量的 `EXEC SQL`。**绝不要把原始的用户输入放进文本。**

调用是行首的一条语句，或是值可以出现的任何地方（`MOVE`、`SET`、
`IF`、`DISPLAY`、参数）的一个值，绝不是接收字段。在 Web 或 Mobile 项目的窗体中，
它是 Build 和 Preview 的错误，类别 `BackendOnly`，并说明出路：编写使用它的 Backend
程序，并 `CALL "NAME" WITH WEBSOCKET LINKAGE USING …`。当任何程序指名它时，构建出的应用程序和
构建出的 Backend 会链接 SQL 驱动。

### 24.10 让用户登录：Authentication 控件

**Authentication** 控件让应用程序的最终用户在 Web 或
Mobile 窗体上登录、注册和退出，恢复密码并回答第二因素——在它自己的界面上（*应用内*）或
通过一个指向身份提供商页面的按钮（*托管*）。它**只与 Backend** 对话：
它的请求是对 Backend 的 `/auth/*` 路由的 HTTPS `POST`，而 Backend 与提供商对话。
页面从不持有刷新令牌（它存放在 HttpOnly cookie 中），而你的 COBOL
只看到 `State`、`UserName`、`Email` 和 `GetClaim`——绝不是令牌。失败是有意写得笼统的：
错误的用户名和错误的密码读起来一样，而重置请求无论账户是否存在
回答都相同。

提供商只在 Backend 的 `[identity]` 部分中设置**一次**（`backend/backend.toml`；
项目的 Web 页面 ▸ *Identity (sign-in)* 部分编辑它）：`provider`（`cognito`、`keycloak`、`entra` 或
`entra-external`）、它的设置、`flow`、`origins`、`[identity.recaptcha]` 和 `cookie_key`。
那里的每个机密都是 `env:NAME` 或 `file:PATH`，绝不是值本身。Build 把身份
程序（`AUTH-SIGN-IN` … `AUTH-CAPABILITIES`）写入 `backend/generated/identity/`。

| 操作 | Cognito | Keycloak | Entra workforce | Entra External ID |
|---|---|---|---|---|
| 默认流程（`Mode = Auto`） | 应用内 | 托管 | 托管 | 托管；`flow = "native"` 时为应用内 |
| 登录、注册、忘记 / 重置密码 | 应用内 | 托管 | 托管 | 应用内（native） |
| MFA：TOTP | 应用内 | 托管 | 托管 | 托管 |
| MFA：SMS / 电子邮件代码 | 应用内 | 托管 | 托管 | 电子邮件为应用内；SMS 为托管 |
| 刷新 | 是 | 是 | 是 | 是 |
| 退出 | 撤销 | 撤销 | 仅登出 | 仅登出 |
| 社交 / 联合登录 | 托管 | 托管 | 托管 | 托管 |

在其列中写着*托管*的提供商上使用 `Mode = InApp`，会被 Check 在控件处拒绝。

- **保护窗体。**窗体的 `RequiresSignIn`（Properties 面板，Web 和 Mobile 窗体）
  会把已退出的用户送到登录窗体——即 `[web] sign_in_form` 所指名的那个，否则是
  第一个持有 Authentication 控件的窗体——并在
  `onSignedIn` 之后打开受保护的窗体；退出会关闭每个受保护的窗体。**这只是用户的体验：
  边界是 Backend 的令牌检查**——请用 `exports.toml` 的 `roles` 来保护数据，
  绝不只靠 `RequiresSignIn`。
- **页面和 Backend 位于同一站点。**刷新 cookie 是 `SameSite=Strict`：当
  页面的来源和 Backend 的地址位于不同的站点时，浏览器绝不会发送
  它，而会话在它的访问令牌过期时结束。Check 会对每个这样的来源发出警告。
- **reCAPTCHA 和同意。**设置了 `[identity.recaptcha]` 时，登录、注册和忘记
  密码会携带 Backend 所验证的 reCAPTCHA v3 令牌。`RecaptchaConsent = Ask` 只在同意之后
  才加载 Google 的脚本：

  ```cobol
        *> onConsentRequired of AUTH-1: show your own consent text
           INVOKE PNL-CONSENT "Show"
        *> onClick of BTN-ACCEPT:
           INVOKE AUTH-1 "GrantConsent"
           INVOKE PNL-CONSENT "Hide"
        *> onSignedIn of AUTH-1:
           MOVE AUTH-1::UserName TO LBL-WELCOME::Caption
           SET WS-SUB TO AUTH-1::GetClaim( "sub" )
  ```

`rcrun backend auth test` 检查 `[identity]` 提供商和 reCAPTCHA 设置，每项检查一行，
绝不打印任何值。

### 24.11 开发、运行和构建 Backend

| 命令 | 它做什么 |
|---|---|
| `rcrun backend run [dir]` | 在回环地址上提供 Backend 供开发使用（`--listen host:port`，默认 `127.0.0.1:0`，一个空闲端口），打印一个仅对这次运行有效的开发令牌，并在每条日志记录发生时打印它；`--page-origin` 只让那个来源可以连接 |
| `rcrun backend describe [dir]` | 写入并打印 `interface.json` |
| `rcrun backend check [dir]` | 检查 Backend，并把它的契约与已发布的契约比较 |
| `rcrun backend release [dir]` | 把当前契约记录为已发布 |
| `rcrun backend build [dir]` | 构建无头服务器二进制 `bin/<name>-backend`（`--debug`、`--accept-breaking`、`--quiet`） |
| `rcrun backend auth test [dir]` | 测试 `[identity]` 和 reCAPTCHA 设置 |

在 IDE 中，**Run ▸ Start Backend** 为打开的项目运行第一个，在
Output 面板中显示它的输出，而 **Run ▸ Stop Backend**（或关闭项目或 IDE）停止它；Run 和
Preview 会替你启动它（23.8）。`rcrun backend run` 像 `rcrun run` 那样发布项目的
`[[sql-connections]]`，所以 Backend 程序中的 `EXEC SQL CONNECT TO 'NAME'`
能够解析。

构建出的 Backend 读取 `backend.toml`：地址、TLS 身份、令牌签发者、允许的来源、
限制。**机密写成 `env:NAME` 或 `file:PATH`，绝不按值写**——发布的 Backend
会拒绝字面量机密——并且 Backend 会在任何名为 `unsafe_…` 的设置上拒绝启动。
普通的 `ws://` 在每种模式下都只在回环地址上提供服务：在网络地址上，请给
`backend.toml` 一个 `[tls]` 部分，或者在前面放一个终结 TLS 的反向代理，并在
回环地址上监听。在发布之前，请指名前端的来源（`[server] allowed_origins`）和身份提供商
（`[auth] issuer`、`audiences`，或 `[identity]`）。
---

## 25. 移动应用程序：Android 与 iOS

**Mobile** 项目是在包外面套着手机外壳的 Web 项目（第 23 章）。它
有同样的窗体、同样的 COBOL 子集、同样的 Backend（第 24 章）和同样的
Preview；它增加的是可以安装在手机或平板上的软件包、手机自己的
功能（安全存储、深度链接、状态栏和刘海），以及一些设置。
Mobile 种类在创建项目时选择——**File ▸ New Project** 在它的左边列出
Android phone and tablet 和 iOS iPhone and iPad——并且不会因编辑
清单而更改。它的窗体位于 `mobileforms/` 中，名为 `<name>.mcfrm`，而项目树显示
**MobileForms** 节点来代替 Forms。

| 你构建 | Build 生成 | 你需要 |
|---|---|---|
| Android 手机或平板 | 先是 Web 包，然后是可安装的软件包 `dist/mobile/android/<Name>-debug.apk` | Android SDK 和 NDK、Java 17 到 21、该手机的 Rust 目标、Tauri 命令行 |
| iOS iPhone 或 iPad | **模拟器**所运行的应用程序，`dist/mobile/ios/<Name>-debug.app` | 装有 Xcode 16 或更高版本的 Mac |

> **尚未构建的内容。**面向设备或应用商店的 iOS 应用程序发布归档
> 需要你的 Apple 开发者账户，而且**不会构建**；这个版本也不为
> iOS 项目生成 Xcode 项目。浏览器覆盖是在 Chromium 和系统 web view 中
> 测得的，还没有在 Mac 上的 Safari 或实体设备上的 Android web view 中测得。

### 25.1 `[mobile]` 设置

Project Settings ▸ **Mobile application** 编辑它们；清单把它们保存在 `[mobile]` 之下：

```toml
[mobile]
platform     = "android"           # "android" or "ios"
device       = "phone"             # "phone" or "tablet"
orientation  = "auto"              # "auto", "portrait" or "landscape"
min_os       = "auto"              # the oldest supported: Android 15 (API "35") or iOS "18.0", or newer
style        = "spa"
multi_memory = true                # false is the iOS default; true for Android
backend_url  = ""
app_id       = "com.example.orders"
features     = ["secure-storage"]  # secure-storage, open-address, deep-link
icon         = "Assets/icon.png"   # a square PNG of at least 1024 px
[mobile.app_name]                  # the name under the icon, per language
en = "Orders"
pt = "Pedidos"
[mobile.signing]                   # the NAMES of the release signing material, never a password
android_keystore = "/Users/me/keys/orders.jks"   # outside the project folder
android_alias    = "orders"
```

`style`、`backend_url`、`max_payload_bytes`、`call_timeout_ms`、`loading_effect` 和 `player`
的含义与它们在 `[web]` 中的含义相同（23.3）。iOS 的 Mobile 项目始终携带
单内存程序模块。

**Application id** 是应用程序在手机上的身份：反向 DNS，两段或更多
段，每段以字母开头，且只包含字母和数字——没有 `_` 也没有
`-`，这两者 Android 接受，但 Tauri 标识符和 iOS bundle id 不接受。新的 Mobile
项目一开始是 `com.example.<name>`，所以它的第一次 Run 就能工作。**在你
发布之前请给它你自己的：**应用商店不接受 `com.example` id，而以后更改它会产生另一个
应用程序。设置页面会拒绝无效的值，并保留已保存的那个。

应用程序所使用的**设备功能**必须在 `features` 中声明，而 Check 会指出
每一处使用了未声明功能的地方（文件和行）。现有的是 `secure-storage`、`open-address` 和
`deep-link`。

**图标**是至少 1024 像素的方形 PNG；没有它，应用程序就带 PowerRustCOBOL 的图标。
**图标下方的名称**是 `[mobile.app_name]`，每种语言一个条目；没有时，用
项目的名称。

### 25.2 构建 Android 软件包

在 Android 的 Mobile 项目上 **Build** 会生成 Web 包，然后生成软件包，外面套着
PowerRustCOBOL 的移动外壳（Tauri 2）。在任何工具运行之前，Build 会**一次性指出项目的
每个问题**：没有 `app_id`、版本没有构建号（`1.2.3` 给出
`10203`）、使用了未声明的设备功能、图标不是至少
1024 px 的方形 PNG。然后它会指出每个缺失的工具以及如何获得它。工具的每一行都进入
构建文件夹（Settings ▸ Build Folder）中的 `package.log`；项目的第一次构建需要
几分钟，之后的则只需几秒。

**发布**软件包用**你的**密钥签名。密钥库（位于项目文件夹之外）
和别名在 `[mobile.signing]` 中指名；两个密码**只**来自进行构建的进程的
变量 `PRC_ANDROID_STORE_PASSWORD` 和 `PRC_ANDROID_KEY_PASSWORD`——
绝不来自文件。编码智能体的 `build` 工具为它接受 `release: true`。

### 25.3 在手机或模拟器上运行

连接了一部手机或运行着一个 Android 模拟器时执行 **Run**，会为那台
设备构建软件包，安装它，像点击图标那样启动它，并在
Output 面板中显示应用程序的日志：`DISPLAY` 所写的内容，按程序写出的样子，外壳的行，以及
崩溃。

- 没有任何东西在运行，而 Android Studio 的 Device Manager 中设置了一个模拟器时，Run 会先启动
  那个模拟器并等待它（冷启动一两分钟，恢复已保存状态时几秒）。之后它保持打开。
- 运行着多部手机或模拟器，或设置了几个但没有在运行的，会打开 **Run on
  device** 窗口（也可通过 **Run ▸ Run on device…**）。它列出正在运行的手机和模拟器，
  以及可以启动的模拟器；**Start** 在选中的那一个上运行，此后的每次
  Run 都会直接去往它而不再询问，直到 IDE 关闭。这个选择不会保存在
  项目中。**Stop** 会在最后启动应用程序的地方停止它。
- 没有设备也没有模拟器时，Preview 在浏览器中以手机的宽度打开。
- 窗体设计器的 **Run Form** 按钮在 Mobile 项目中做 Run 所做的事：它在手机或模拟器上**从应用程序的
  主窗体**运行，绝不在桌面窗口中，那里
  没有手机的任何功能。**Debug Form** 仍然在桌面窗口中运行窗体——
  调试器目前能到达的唯一地方——所以那里的设备动词会以它的异常结束，除非
  调用指名了 `ON EXCEPTION`。
- 失败的 Run 会在 Output 中说明原因，并且不打开任何其他东西。有 Backend 时，Run 会先
  启动它（23.8）。

### 25.4 Android Studio 项目

当 **New Project** 为 Android 创建 Mobile 项目时，它自己的一个步骤会提供同时创建
它的 **Android Studio 项目**。**Create** 在项目的 `android/` 文件夹中创建它；
**Not now** 则不创建。Project Settings ▸ Mobile application 显示它是否存在，
以及在哪里，带有 **Create**（或 **Create again**），所以没有它而创建的项目，或其文件夹
被删除的项目，可以把它拿回来。

Android Studio 打开 `android/src-tauri/gen/android`，一个以
PowerRustCOBOL AI 项目命名的 Gradle 项目。每次 Build 以及在设备上的每次 Run 都会用
同一个包让它保持最新，所以 Android Studio 运行的就是软件包所显示的。它的 Gradle 构建通过
Tauri 命令行编译外壳，而这个命令行必须同时在运行，所以请用同一行的 **Open in
Android Studio** 打开它：IDE 让项目保持最新，启动 Tauri 命令
行（在 `android/src-tauri` 中执行 `cargo tauri android build --ci --debug --apk --target aarch64 --open`），
它会构建一次应用程序，在项目上打开 Android Studio，并在你
于那里运行和调试时保持运行。**Stop** 会结束它，下一次 Open 和关闭
IDE 也会（此后 Android Studio 在再次被打开之前无法构建外壳）。它的行进入
构建文件夹中的 `android-studio.log`。没有 IDE 时，请自己在
`android/src-tauri` 中运行那条命令，并用 Ctrl+C 停止它。

该文件夹只保存生成的外壳和 Web 包——**没有 COBOL，没有窗体，没有机密**
——并且它的 `.gitignore` 排除了工具所构建和缓存的内容。没有 Android SDK、它的
NDK、Java 17 到 21 或 Tauri 命令行时，只会写入它的源码，而 Output 会指出
缺少什么。清单把它记录为 `[mobile.native]` 下的 `android = "android"`。

### 25.5 用于模拟器的 iOS 应用程序

在装有 Xcode 16 或更高版本的 Mac 上，iOS Mobile 项目的 Build 会生成
iOS Simulator 所运行的应用程序，`dist/mobile/ios/<Name>-debug.app`。它携带的始终是
`single-memory` 程序模块。iOS 项目在 IDE 中没有别的不同，窗体
和 Backend 也是同样的。

### 25.6 在手机屏幕上

应用程序避开**状态栏、刘海和手势栏**：窗体
从状态栏之下开始，而 Preview 的手机尺寸显示同样的情形。在手机的浅色
外观下，状态栏下面有一条浅色带，所以它深色的图标在深色
窗体上仍然可读；主题或页面设置自定义属性 `--pr-statusbar` 可以选择另一种颜色，
而不是手机 web view 的页面则以 `--pr-safe-top`、`--pr-safe-right`、
`--pr-safe-bottom` 和 `--pr-safe-left` 给出内边距。第一个窗体上的 **Back** 会关闭应用程序，
而下一次启动是全新的。

> **注意事项。**状态栏图标保持应用程序启动时手机所具有的外观；
> 在应用程序运行时把手机在浅色和深色之间切换，会让它们显示得不匹配，直到
> 下一次启动。

### 25.7 手机的安全存储

有了功能 `secure-storage`，机密保存在手机**自己的**存储中——Android
Keystore 或 iOS Keychain——绝不在页面中：

```cobol
           COBOL::SecureStore( "user-name" TXT-NAME::Text::Trim() )
           IF COBOL::SecureStatus = 0
               MOVE "Saved." TO LBL-RESULT::Caption
           END-IF
           COBOL::SecureLoad( "user-name" WS-SAVED-NAME )
           COBOL::SecureDelete( "user-name" )
```

每个都等待手机的回答，而 **`COBOL::SecureStatus`** 说明发生了什么：`0` 完成，`1`
那里什么也没有（找不到值的 `SecureLoad`、没有东西可删除的 `SecureDelete`），`-6` 被拒绝。
`SecureLoad` 把值放进它的第二个参数和 `COBOL::SecureValue` 中。键
是 1 到 128 个字母、数字、`-`、`_` 或 `.`；值最多 4096 个字节。在浏览器
**Preview** 中，这三个立即回答 `-6`，并在控制台中说明原因，所以同一个窗体在
Preview 和手机上都能运行。Web 项目的 Build 会在它们所在的行拒绝它们；桌面程序
会以指出它们在哪里能工作的运行时错误停止，除非调用指名了 `ON EXCEPTION`。

### 25.8 深度链接

有了功能 `deep-link`，形如 `com.example.orders://order-form?id=7` 的链接会带着它的参数
打开那个窗体。窗体用 `GetParam`（不存在时为空，
名称不区分大小写）按名称读取其中一个，或用 `GetParams`（`name=value` 行）读取全部，在它的
第一个处理程序中就可以：

```cobol
           *> onLoad of the order form: the link's id parameter picks the order to show.
           INVOKE me::"GetParam"("id") RETURNING WS-ORDER-ID
```

### 25.9 证书固定与其他安全表

Mobile 项目可以固定它的 web view 所信任的证书：

```toml
[mobile.pinning]
pins    = ["sha256/<base64>", "sha256/<base64>"]   # at least two different ones: the live key and a backup
expires = "2027-06-30"                              # the day the pins stop being the ones to trust
```

Check 和 Build 会拒绝少于两个不同的固定值、不是 `sha256/` 加上 32 个字节的 base64
的固定值、缺失或不可读的 `expires`，以及已经过期的；`expires`
不足 30 天的表是警告。固定值按它在消息中的位置命名，绝不被
引用。`[mobile.unsafe]`、`[security]` 以及 Build 对项目自己文件的机密扫描
与在 Web 项目中一样工作（23.9）。**Backend 的发布会拒绝** `backend.toml` 中的**字面量机密**
（24.11）。

### 25.10 示例展示什么

`examples/PowerMobileHello` 是最小的 Mobile 项目。`examples/PowerOficina`——一个
带有基于索引文件的 Backend 的车间管理应用程序——以及
`examples/PowerAnalyticsWeb`——桌面 PowerAnalytics 仪表板在 Web 上的外观——展示
更大的应用程序是什么样子；请从 **File ▸ Open Project** 打开它们。
---

## 26. 图形：Canvas 2D、Canvas 3D 与图形对象

必须绘制自己的东西的窗体——游戏、自定义图表、图像工具、3D 模型
查看器——有七个图表控件、一个 PictureBox 和一个 Animator，而在第 2 版之前
没有可以用来*绘制*的东西。两个**控件**和五个**非可视对象**填补了这个空白。它们
位于**每种项目种类**的工具箱中，并且一套 COBOL API 在**每个平台**上驱动它们：
IDE、`rcrun`、编译出的二进制、浏览器和手机。

| 类型 | 种类 | 它是什么 |
|---|---|---|
| **Canvas2D** | 控件 | 2D 绘制表面：程序用方法绘制，表面保存图画 |
| **Canvas3D** | 控件 | Scene3D 的 3D 视图，在 GPU 上绘制 |
| **ImageBuffer** | 非可视 | 用于加载、更改和保存的像素；它没有窗口——由 Canvas2D 绘制它 |
| **Plot** | 非可视 | 由数字绘制到 ImageBuffer 或 Canvas2D 上的图表 |
| **Scene3D** | 非可视 | 3D 场景——节点、网格、glTF 模型、材质、灯光、相机、拾取——由 Canvas3D 显示 |
| **Physics2D** | 非可视 | 由物体、形状、关节和接触构成的 2D 物理世界，由程序步进 |
| **Physics3D** | 非可视 | 三维中的同样东西 |

这一切都**只实现一次**，用纯 Rust，而桌面宿主和浏览器播放器是
薄薄的适配器，给它一个表面和一个事件循环。这就是为什么绘制在
**每个宿主上都是相同的像素**：浏览器并不用 HTML 的 `<canvas>` 来绘制 Canvas2D。七个图表
控件、PictureBox 和 Animator 保持原样；当它们能胜任时请使用它们，而
当你必须绘制它们无法绘制的东西时才使用 Canvas。

`examples/PowerGraphics` 有九个窗体，用来演练这一切：一个小游戏
（`game-form`）、图表（`charts-form`）、一个图像实验室（`imagelab-form`）、一个 3D 查看器
（`viewer3d-form`）、两个物理台球池（`pool2d-form`、`pool3d-form`）以及其他。请从
**File ▸ Open Project** 打开它。

### 26.1 Canvas2D

把 Canvas2D 放到窗体上；它的属性包括 `Width` 和 `Height`（逻辑像素）、
`PixelRatio`（设备像素比；`Width` 和 `Height` 保持为窗体像素）、`Smoothing`、
`BackgroundColor`、`TargetFps`、`AutoPresent` 和 `AccessibleLabel`。你通过调用
方法来绘制，就像在任何控件上那样：

```cobol
           CNV-GAME::Clear( "#101820" )
           CNV-GAME::SetFillColor( "#335577" )
           CNV-GAME::FillRect( 0 300 480 20 )
           CNV-GAME::BeginPath()
           CNV-GAME::Arc( WS-X WS-Y 16 0 6.2831853 0 )
           CNV-GAME::SetFillColor( "#E05555" )
           CNV-GAME::Fill( 0 )
           CNV-GAME::SetFont( "14px Ubuntu" )
           CNV-GAME::SetFillColor( "#FFFFFF" )
           CNV-GAME::DrawText( "Arrow keys push the yellow sprite" 10 24 0 1 )
           CNV-GAME::Present()
```

绘制词汇是 Web 开发者所熟悉的，名称相同：

| 分组 | 方法 |
|---|---|
| 状态 | `Save`、`Restore`、`Reset`、`Clear`、`Present`、`SetSmoothing` |
| 路径 | `BeginPath`、`MoveTo`、`LineTo`、`QuadTo`、`CubicTo`、`ArcTo`、`Arc`、`Ellipse`、`Rect`、`RoundRect`、`ClosePath`、`Fill`、`Stroke`、`Clip` |
| 形状 | `FillRect`、`StrokeRect`、`ClearRect`、`DrawLine`、`DrawCircle`、`DrawPolygon` |
| 绘制 | `SetFillColor`、`SetStrokeColor`、`SetLineWidth`、`SetLineCap`、`SetLineJoin`、`SetMiterLimit`、`SetDash`、`SetAlpha`、`SetBlendMode`、`SetLinearGradient`、`SetRadialGradient`、`SetConicGradient`、`SetPattern`、`SetShadow` |
| 变换 | `Translate`、`Rotate`、`Scale`、`Transform`、`SetTransform`、`ResetTransform` |
| 文本 | `SetFont`、`SetTextAlign`、`SetTextBaseline`、`SetTextDirection`、`DrawText`、`DrawTextBox`、`MeasureText`、`LoadFont`、`LoadFontAsset` |
| 图像 | `DrawImage`、`DrawImageScaled`、`DrawImageRegion`、`DrawSprite`、`ReadPixel`、`Capture`、`GetPixel`、`ToImage` |
| 命中测试 | `HitPath`、`HitStroke`、`IsPointInPath`、`IsPointInStroke` |
| 控制 | `Resize`、`SetPixelRatio`、`GetWidth`、`GetHeight`、`GetPixelRatio`、`Flush`、`Discard`、`PendingCommands`、`TakePresented`、`GetDiagnostics`、`GetHandle` |

除此之外，System KB（Grace 和编辑器的补全读取它）列出每个方法及其
参数，来自运行时所派发的同一张表——一个方法不可能被列出却缺失，
或有效却未列出。

**动画与输入。**

- **`TargetFps`**——设置它并绑定 **`onFrame`**：该事件以那个速率触发，带有已过的
  毫秒数。`FixedStep` 给出精确的步长。`TargetFps = 0` 时画布按
  需重绘。
- **`AutoPresent`**——图画在每个处理程序结束时显示。没有它，请调用
  `Present()`。
- **事件**——`onPointerDown`、`onPointerMove`、`onPointerUp`、`onWheel`、`onKeyDown`、
  `onKeyUp`、`onGamepadButton`（参数 `pad button 1|0`）和 `onGamepadAxis`（参数 `pad axis
  value`）**只有在绑定了处理程序时**才会触发。`PointerX`、`PointerY`、`IsKeyDown( key )`、
  `GamepadAxis( axis [pad] )` 和 `GamepadButton( button [pad] )` 可以**随时读取**，不管
  绑定了什么处理程序。上面的游戏在 `onFrame` 中轮询键盘：

  ```cobol
           IF      CNV-GAME::IsKeyDown( "ArrowLeft" ) = 1
               PHY-1::ApplyImpulse( WS-WORLD, WS-BALL(1), -2000, 0 )
           END-IF
  ```

- **错误的调用不是异常。**它设置 `LastError`，绘制继续进行。
- **使用窗体所指名字体的文本**在 Web 构建时需要 `Assets/` 下的字体文件；Build
  会停止并指出缺失的字体族。

**Canvas2D 与 HTML canvas 的比较。**

| HTML canvas | Canvas2D |
|---|---|
| `ctx.fillRect(x, y, w, h)` | `CNV-1::FillRect( x y w h )`——名称相同，作为内联方法调用 |
| `requestAnimationFrame` | 设置 `TargetFps` 并绑定 `onFrame` |
| 绘制立即显示 | 有 `AutoPresent` 时，图画在处理程序结束时显示；否则调用 `Present` |
| 元素上的事件 | 只有绑定了处理程序时才触发；`PointerX`、`IsKeyDown(key)` 和游戏手柄的读取随时都能用 |
| CSS 像素和 `devicePixelRatio` | `PixelRatio` 缩放表面；`Width` 和 `Height` 保持为窗体像素 |
| 错误的调用会引发异常 | 调用设置 `LastError`，绘制继续进行 |
| `getContext("webgl2")` | 一个单独的控件 Canvas3D，显示 Scene3D |

### 26.2 ImageBuffer

ImageBuffer 把像素保存在内存中——它没有窗口。创建一个空的（`Create`），加载一个
（来自 `Assets/` 的 `LoadAsset`、`LoadFromBytes`），更改它，并保存它（`Save`、`Encode`）：

| 分组 | 方法 |
|---|---|
| 创建与保存 | `Create`、`LoadAsset`、`LoadFromBytes`、`Encode`、`Save`、`Clone`、`GetWidth`、`GetHeight`、`GetHandle` |
| 几何 | `Resize`、`Crop`、`Rotate`、`Flip` |
| 像素 | `Fill`、`GetPixel`、`SetPixel`、`DrawOnto`、`Composite` |
| 颜色 | `Grayscale`、`Invert`、`Brightness`、`Contrast`、`Hue`、`Saturation`、`Threshold` |
| 滤镜 | `BlurGaussian`、`BlurBox`、`Sharpen`、`EdgeDetect`、`Convolve`、`Dilate`、`Erode` |
| 测量 | `Histogram` |

Canvas2D 用 `DrawImage` 绘制 ImageBuffer（上面的游戏就是这样绘制它的精灵的）。在
有多个核心的机器上，繁重的操作会使用它们，并在没有多核的
平台上给出**相同的结果**。

### 26.3 Plot

Plot 由数字绘制图表——点或值的系列、直方图、蜡烛图、误差线、
网格和 3D 曲面——绘制到 ImageBuffer（`Render`）、Canvas2D（`RenderToCanvas`）或作为 SVG
（`ToSvg`）。

| 分组 | 方法 |
|---|---|
| 框架 | `SetTitle`、`SetXLabel`、`SetYLabel`、`ShowLegend`、`ShowGrid`、`SetBackground`、`SetForeground`、`SetFont`、`SetXRange`、`SetYRange`、`ClearRanges`、`AddCategory`、`ClearCategories`、`SetHistogramBins`、`SetView3D` |
| 系列 | `AddSeries`、`RemoveSeries`、`ClearSeries`、`GetSeriesCount`、`GetSeriesPoints`、`SetSeriesName`、`SetSeriesColor`、`SetSeriesLineWidth`、`SetSeriesPointSize`、`SetSeriesClosed` |
| 数据 | `AddPoint`、`AddPoint3`、`AddValue`、`AddCandle`、`AddErrorPoint`、`SetSeriesGrid`、`SetSeriesSurface`、`SetSeriesFromTable` |
| 绘制 | `Render`、`RenderToCanvas`、`ToSvg` |

### 26.4 Canvas3D 与 Scene3D

**Scene3D** 保存世界：**节点**（`AddGroup`、`AddBox`、`AddSphere`、`AddPlane`、
`AddCylinder`、`AddCone`、`AddTorus`、`AddMesh`）、**模型**（`LoadModel`、`LoadModelFromBytes`——glTF）、
**灯光**（`AddDirectionalLight`、`AddPointLight`、`AddSpotLight`、`SetAmbient`）、**相机**
（`AddCamera`、`AddOrthographicCamera`）、**材质**（`CreateMaterial`、`SetMetallic`、
`SetRoughness`、`SetEmissive`、`SetTexture`、`SetAlphaMode`）、实例化（`SetInstances`）和
**动画**（`PlayAnimation`、`StopAnimation`、`SetAnimationTime`），以及常见的树操作
（`SetParent`、`FindNode`、`SetPosition`、`SetRotation`、`SetScale`、`LookAt`、`Update`）。

**Canvas3D** 显示 Scene3D，在 GPU 上绘制：`SetScene`、`SetCamera`、`Render`、`Pick`（
某个点之下有什么）、`Screenshot`、`SetMsaa`、`SetShadowMapSize`、`GetStats`。它**只有在
程序调用 `Render` 时才显示**——没有 AutoPresent。`Backend`（只读）说明是什么在
绘制它（`WebGPU`、`WebGL2`、`Vulkan`、`Metal`、`Dx12` 或 `Software`）。在没有可用
适配器的机器上，控件显示一条消息，而 `LastError` 说明原因；窗体的其余部分继续
工作。

### 26.5 Physics2D 与 Physics3D

物理世界由**程序**以固定的时间步长来**步进**，所以相同的输入在每个宿主上
给出相同的结果。`CreateWorld` 创建一个；`AddBody`、`AddBall`、`AddBox`、
`AddCapsule` 和 `AddConvexHull`（以及在 3D 中的 `AddTrimesh`）添加其中的内容；`AddFixedJoint`、
`AddRevoluteJoint` 和 `AddPrismaticJoint` 连接物体；`ApplyForce`、`ApplyImpulse` 和
`ApplyTorque` 移动它们；**`Step( world seconds )`** 推进时间；`RayCast` 询问射线击中什么；
`TakeEvents` 读取接触。`BindSprite`（2D）和 `BindNode`（3D）把物体与显示它的东西绑在一起，
而 `GetStateFingerprint` 给出一个值，当两个宿主的世界一致时，这个值在它们上面相同。

```cobol
           PHY-1::Step( WS-WORLD 0.016667 )
           SET     WS-POS                    TO    PHY-1::GetPosition( WS-WORLD WS-BALL(WS-I) )
           UNSTRING WS-POS DELIMITED BY SPACE INTO WS-PX WS-PY
           COMPUTE WS-X = FUNCTION NUMVAL( WS-PX )
```

### 26.6 在 COBOL 中书写这些调用

- 回答**句柄**的方法（`GetHandle()`、`AddSeries`、`AddBody`、`Render`……）回答
  最多 32 位的整数，它在槽被重用时会增长：请把它保存在 `PIC 9(10)` 的数据项中，
  **绝不要用 `PIC 9(4)`**。
- 接受**数字列表**的方法（`SetSeriesGrid`、`SetSeriesSurface`、`Convolve`、
  `AddMesh`、`SetInstances`）把它们作为**文本**接收，以空格或逗号分隔：
  `PLT-1::SetSeriesGrid( WS-SER 3 3 "1 2 3 4 5 6 7 8 9" )`。不是数字的词会在
  `LastError` 中被按名称拒绝。
- **行表**作为持有它的组交给 `SetSeriesFromTable`，附上行数、
  以字节计的行宽，以及一次性描述的列（`"0:zoned(6,0); 6:zoned(6,0)"`）。
- Physics2D 的**位置**回答文本 `x y`：用 `UNSTRING … DELIMITED BY SPACE` 把它拆开，
  并用 `FUNCTION NUMVAL` 读取每一部分，如上所示。

### 26.7 限制会怎样

这些对象有**限制**——画布大小、解码图像的像素数、模型
或纹理的大小、节点和物体的数量、记录的绘制的长度——由
项目设置，并带有安全的默认值。默认值的选择让手机的浏览器能够承受：它们允许
2400 万像素的照片和 4096 × 4096 的画布。会超过其中某个限制的调用会被**拒绝**，
并在 `LastError` 中给出一条可读的消息，指出要提高的设置，绝不会崩溃。

---
## 27. 注意事项与当前限制

一份汇总的清单，好让你永远不会被意外撞上：

- **事件的触发。**设计视图只列出运行时会触发的事件，而且它们
  每一个都会触发（§10）。没有引擎支持的事件——比如控件的
  拖放——不会被提供，所以你无法绑定一个永远不会运行的处理程序。请在 *Run Form* 中
  核实。
- **文件组织方式。**四种全都支持——SEQUENTIAL、LINE
  SEQUENTIAL、INDEXED 和 RELATIVE（§13）。每个动词都按
  文件所声明的 `ORGANIZATION` 派发。
- **锁定。**以 `WITH LOCK` 或 `SHARING WITH NO
  OTHER` 打开的整个 INDEXED 文件，在进程之间是独占的（另一个程序的 `OPEN` 得到状态
  `93`）。记录锁是单进程的（§14）。
- **一个 INDEXED 文件，两个活着的窗体。**每个窗体都是它自己的程序，所以两个
  往*同一个* INDEXED 文件里写的窗体，就是两个各自独立的写入者——它们的
  记录锁不会跨窗体协调。请给每个数据文件指定一个窗体作为主人，而把取值改为通过已公开的窗体属性传递。
- **跨窗体的 EXEC RUST。**对象桥是每个*应用程序*一份：
  在任何一个窗体的块里创建的句柄，在其他每一个窗体的
  块里都解析得到，而来自不同窗体的块会轮流使用它。正因如此，通过这座桥
  保存的值必须是线程安全的（`Send`）。
- **`rcrun build` 像 IDE 一样重新生成。**在编译之前，
  `rcrun build` 会从每个窗体的 `.cfrm` 重写它生成的 COBOL，并从每个
  索引文件的 `.cidx` 重写它的外观以及 `COPYBOOKS/<name>.SEL`/`.FD`。
  它使用与 IDE 的 Generate 相同的生成器和相同的文件，所以
  在 IDE 之外编辑过的窗体，构建的是它当前的代码。你的处理程序和
  过程位于 `.cfrm` 中，所以你写的任何东西都不会丢失。凡是
  手工键入 `generated/` 文件的内容，都会被替换，就像每次 IDE
  构建那样。`rcrun build` 不改动项目文件：它为从未有过生成程序的窗体
  创建的生成程序，供这次构建使用，而 IDE 会在下一次重新生成时
  把它加入项目。
- **面向对象的 COBOL。**`CLASS`/`METHOD` 定义不在范围之内。
- **ISAM 互换。**磁盘上的格式是原创的，并且与任何第三方 ISAM **都不**二进制兼容。
- **生成的代码是只读的。**要编辑就编辑窗体或 Common Code，绝不要编辑 `generated/`。
- **机密**绝不可嵌入交付出去的窗体里。
- **图层。**图层中的控件不做布局（没有 Dock、Anchor，也没有 flex、grid
  或 flow），`Visible` 是运行中程序和设计器的状态，
  绝不保存，而且有七种控件类型可能会越过图层中窗口的圆角
  绘制，就像在基础层中那样（§7）。IDE Check 不读取
  `LAYER-NAME::property`；Build 和 Run Form 会读取。
- **窗体主题。**窗体的 **Theme**（在它的 Appearance 部分）选择 Liquid
  Glass——默认——**Elegance**、**Spatial**，或任何已安装的资源包；
  不设置它的窗体采用项目的 **Default form theme**
  （*窗体主题与样式*，§9）。在 Liquid Glass 之下，另有一行 **Glass style**
  选择 Classic、Enhanced、Neumorphic Light 或 Neumorphic Dark（带有渐变、模糊、
  距离和镶边控制的程序化浮雕）；像 Elegance 或 Spatial 这样拥有
  整个外观的主题，则忽略它。
- **任何打开网页的人都可以读取它。**Web 或 Mobile 应用程序的窗体、它的
  程序模块和它的样式表可以被读取和篡改，所以文件、数据库、密钥和
  决定金钱的规则放在 Backend 中，而边界是 Backend 的令牌检查——绝不是
  `RequiresSignIn`，也绝不是页面所做的其他任何事（第 24 章）。
- **Web 窗体的 COBOL 是一个子集。**文件输入/输出、`EXEC SQL`、`COBOL::SqlDatabase`、
  `SORT … USING/GIVING`、`MERGE` 和 `SCREEN SECTION` 都不在页面中运行；Build 在
  每一处所在的行拒绝它，并指出出路（23.5）。
- **Web 和解释器在一个已知之处不同。**写入控件属性的商，在页面上
  保持它自己的小数位数（`4.000000000`），而在桌面上被截成属性的
  小数位数（`4`）。`specs/WASM-PARITY.md` 列出每一处已记录的差异。
- **浏览器覆盖。**它已经在 Chromium 和系统 web view 中测得；Mac 上的 Safari
  以及实体设备上的 Android web view 还没有测。WebAssembly 规格中的一些验收标准仍然
  开放：`specs/WASM-OPEN-ACS.md` 列出它们。
- **iOS。**Build 为**模拟器**制作应用程序；面向设备或应用商店的发布归档
  需要你的 Apple 开发者账户，而且不会构建，也不会生成 Xcode 项目。
- **Web 上的数据控件。**绑定到 REST API 或智能体来源的 DataGrid 目前还不能在
  Web 上运行（绑定到索引文件、SQL 查询或 COBOL 表的网格可以）。在浏览器中，绑定网格的
  `RefreshBinding()` 和 `Save()` 在 Backend 回答之前就返回：请处理 `onSaved` 和
  `onCommitError`（24.8）。
- **重新连接的模态框是浏览器的。**桌面不绘制 *Connecting to the server…* 模态框，
  目前也不触发 `onBackendUnreachable`（24.4）。
- **`COBOL::SqlDatabase` 不接受参数。**`Execute` 和 `Query` 只接受语句文本；在语句从人那里
  获取输入的地方，请使用带宿主变量的 `EXEC SQL`（24.9）。

---

## 附录 A —— 从 PowerCOBOL / isCOBOL 过来

一份大致的心智地图，好让你上手更快。这些是*类比*，不是精确的等价物。

| 你原本熟悉的（PowerCOBOL / isCOBOL） | 在 PowerRustCOBOL 里 |
| --- | --- |
| 一张带控件的 *sheet* / *form* | 一个在 **Form Designer** 里编辑的**窗体**（`.cfrm`） |
| 属性表 | **属性面板**（可折叠的分节卡片） |
| 挂在某个控件上的事件过程 | 一个 COBOL **事件处理程序**（名为 `CONTROL-ID--EVENTNAME` 的嵌套程序） |
| 被运行时藏起来的那个事件循环 | 生成代码里那个显式的 **`COBOL::"WAIT-EVENT"`** 循环 |
| 对控件做 `INVOKE` / 方法调用 | 一样——`Ctrl::Method(args)`、`INVOKE Ctrl "Method" USING …`，或者 `COBOL::"GET-PROPERTY"` / `"SET-PROPERTY"` 内置调用 |
| 厂商的 ISAM | PowerRustCOBOL 的**索引文件**（`STORAGE IS MEMORY/DISK`、`redb`、`COMMIT`/`ROLLBACK`） |
| 嵌入式 SQL / ODBC | 带宿主变量、游标、SQLCA 的 `EXEC SQL … END-EXEC`——[嵌入式 SQL](#嵌入式-sqlexec-sql)（SQLite、PostgreSQL、MySQL）；或者 `COBOL::"OPEN-DB"` + `COBOL::"EXEC-SQL"`（SQLite/PostgreSQL/MySQL） |
| 连着一个运行时 DLL 构建出 `.exe` | `rcrun build` → **一个自成一体的二进制**，没有运行时要装 |
| 项目 / 工作区文件 | 项目文件（`<name>.project.toml`）加上那套标准的文件夹布局 |
| 浏览器前端 | **Web** 或 **Mobile** 项目（§23）：同样的窗体和 COBOL，编译成 WebAssembly，用 **Backend**（§24）处理数据和机密 |

> ⚠️ **不要**指望与此前任何厂商的产品在源码级、文件格式级或二进制级上兼容。
> 概念可以搬过来；产物搬不过来。

---

## 附录 B —— 术语表

- **应用外壳**——主窗体上的 **SideMenu** 所开启的那种单窗口安排：一个菜单窗格、
  一条面包屑，以及一个供窗体就地载入的 **ContentPane**（§22）。
- **Backend**——Web 或 Mobile 项目中运行在原生运行时上的那一部分，位于 `backend/`：
  它的程序列在 `exports.toml` 中，是页面唯一可以 `CALL … WITH WEBSOCKET
  LINKAGE` 的程序。它保存文件、数据库和机密（§24）。
- **块字面量**——一个用反引号围起来的多行字面量，原样取用。这是
  PowerRustCOBOL 的扩展；只在自由格式下可用（§13）。
- **面包屑**——横贯外壳窗口顶部、标明导航链的那一条框。它的尺寸与颜色由
  SideMenu 的 `Breadcrumb*` 属性决定（§22）。
- **包**——Web Build 所生成的东西：一个静态文件夹（`dist/web/`），里面有 WebAssembly 程序模块、
  预构建的播放器、窗体描述、宿主页面和一个清单，任何 Web
  服务器都可以托管它（§23）。
- **Common Code**——你手写的 COBOL，位于 `src/`。可编辑，并可从处理程序里
  用 `CALL` 调用。
- **ContentPane**——外壳窗口中容纳所载入窗体的那块区域。它等于
  主窗体的尺寸减去 SideMenu 的宽度和面包屑的高度。
- **契约**——Backend 所导出的程序及其参数，在 `exports.toml` 中以
  `major.minor.patch` 标注版本；当发布构建会破坏上一个已发布的契约时，它会被拒绝
  （§24）。
- **控件**——窗体上的一个元素：按钮、文本框、图表，等等。
- **数据绑定**——一种窗体一级的映射，从某个数据源（索引文件、SQL、
  COBOL 表、REST、AI 智能体）映到一个获批的目标控件（§8）。
- **Data Binding Guardian**——在保存、运行、调试、Check、Build 或打包之前检查各项绑定的
  那个验证器，报出 Blockers、Warnings 和 Info。
- **引擎**——索引文件背后的存储后端，用
  `rcrun --indexed-engine` 来选，或者对一个项目来说，用 **Settings ▸ Default Indexed File
  Engine**。默认是分页的 `rust` 引擎；抗崩溃的 **`redb`**
  引擎按名字选择。
- **事件**——用户或系统所做的某件事；命名为 `onSomething`。
- **`EXEC RUST` 块**——嵌在某个处理程序里的一段宿主语言代码，在 Build 时被编译进应用程序
  （§13）。它通过 `cobolt_objects` 够到窗体，并能通过 `cobolt_windows` 开出属于它自己的
  一扇窗。
- **窗体**——你所设计的一扇窗口；以一个 `.cfrm` 文件保存。
- **窗体形态**——一个窗体是可以在它自己的窗口里打开（`Standalone`）、可以被载入一个
  ContentPane（`Embedded`），还是两者皆可（`Both`）（§22）。
- **生成的代码**——PowerRustCOBOL 从一个窗体产出的那份只读 `.cbl`，
  位于 `generated/`。绝不手工编辑；在每一次 Build、Run、Debug 和
  Check 时重新生成。
- **处理程序**——为某个事件而运行的那段 COBOL；生成为一个名叫
  `CONTROL-ID--EVENTNAME` 的嵌套程序。
- **幂等键**——运行时给 Backend 调用的键，这样在连接丢失之后重复的调用，会根据 Backend 所保留的内容来回答，
  而不会让程序运行两次（§24）。
- **索引文件**——一个 ISAM 文件（`ORGANIZATION IS INDEXED`），在
  项目里由一份 `.cidx` 定义来描述。
- **知识库**——项目里那个类别，装着 AI 助手可以取用的
  Markdown、文本与 PDF 材料。
- **主窗体**——一个项目里被标记为应用程序入口的那唯一一个窗体。一个构建出的二进制启动时
  进入的，就是它的生成程序。
- **`me`**——指代当前窗体的那个接收者，如 `me::Title`。
- **Mobile 外壳**——PowerRustCOBOL 的 Tauri 2 应用程序，它把 Web 包放进手机的 web
  view，并给它手机自己的功能（§25）。
- **非可视控件**——一项在运行期没有外观的服务：Timer、
  AI Agent、REST Client、SQL Database、Indexed File、Web Search、Snackbar。
- **播放器**——在浏览器中运行 Web 或 Mobile 应用程序窗体的预构建程序；
  Web 构建把它复制到你的 COBOL 旁边，所以不需要 Rust 工具链（§23）。
- **项目种类**——Desktop、Web 或 Mobile，在创建项目时选择（§23）。
- **Project's Crates**——项目一级的第三方库目录，
  登记在册以供 `EXEC RUST` 块使用（§13）。
- **属性**——一个控件或窗体的具名特性，用 `::` 成员语法来读写。
- **rcrun**——命令行的运行时、检查器、打包器和二进制编译器。
- **重复分组**——一个被变成卡片模板、按数组元素逐个重复的 GroupBox；其中某个成员的处理
  程序，会通过 `CONTROL-ARRAY-INDEX` 被告知是哪张卡片触发的（§8）。
- **位置路径**——一条诊断用来点明*你*所写的那个地方、而不是生成代码某一行的方式：
  `MAIN-FORM ▸ BTN-OK ▸ onClick`（§12）。
- **存储方式**——加在一个 `SELECT` 上的 `STORAGE [MODE] IS MEMORY | DISK` 子句，
  在内存中的表与磁盘上的持久仓库之间做选择。默认是 **DISK**（§14）。
- **`super`**——指代那个载入或打开了本窗体的窗体的接收者，如
  `super::Title`。在主窗体里它是 NULL（§22）。
- **User Control**——一个以 GroupBox 为基础、可复用的组件，保存在项目里，
  并以带限定 id 的真实控件形式被部署出来（§8）。

---

*本指南是一份活的文档。每当添加一项功能或改变一种行为时，它就会被扩充——如果这里的某处
与正在运行的工具有出入，那么以工具（以及 `docs/` 下的参考文件与测试套件）为准；请把那处
不一致报告出来。*

.<<

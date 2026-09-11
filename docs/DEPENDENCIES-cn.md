<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# crate 清单

PowerRustCOBOL **直接**依赖的每一个 crate，以及实际链接的版本（不是需求字符串，
而是 `Cargo.lock` 中解析出的版本）。

首次由 `cargo metadata` 生成于 **2026-07-27**，当时的产品版本是 **1.37.0**；下面
这些解析出的版本最后一次与 `Cargo.lock` 核对是在 **2026-09-11**，版本 **1.65.128**。
请注意这里有两套编号：*产品*版本位于 `crates/cobolt-ide/src/version.rs` 并显示在 IDE
中；`Cargo.toml` 里的 *crate* 版本是 `0.2.0`，由工作空间内所有 crate 共用。可以用下
面的命令重新生成版本列：

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

完整的依赖图共 **944 个包**。下面的表格列出的是工作空间自己点名的那 **59** 个；
其余一切都经由它们传递而来。

---

## 工作空间内的 crate

构成 PowerRustCOBOL 本身的那 17 个 crate —— 这里的权威是
`cargo metadata --no-deps`，而不是对 `Cargo.toml` 的 grep：在那里有两个成员共用一
行，直接数会得到 16。它们全都共用工作空间的 crate 版本 `0.2.0`（见上面的说明——
产品版本自成一套序列）。

| Crate | crate 版本 | 层 | 作用 |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | 前端 | Fujitsu COBOL 分词器——固定格式与自由格式源码——以及 `COPY`/`REPLACE` 预处理器 |
| `cobolt-parser` | 0.2.0 | 前端 | 递归下降语法分析器：词法单元流 → AST |
| `cobolt-ast` | 0.2.0 | 前端 | AST 节点类型 |
| `cobolt-semantic` | 0.2.0 | 前端 | 名字解析、类型检查、`EXEC RUST` 绑定 |
| `cobolt-runtime` | 0.2.0 | 执行 | 树遍历解释器、值系统、`EXEC RUST` 执行器、数据库/HTTP 运行时 |
| `cobolt-stdlib` | 0.2.0 | 执行 | 内建函数、I/O 后端、控制台辅助 |
| `cobolt-indexed` | 0.2.0 | 执行 | 索引文件定义模型（`.cidx`） |
| `cobolt-forms` | 0.2.0 | UI 引擎 | 窗体/控件模型（`.cfrm`）、统一渲染引擎、主题、动画 |
| `cobolt-form-host` | 0.2.0 | UI 引擎 | 唯一的窗体宿主（spec 042）——由 `rcrun run-form` 与编译出的应用程序共用 |
| `cobolt-media` | 0.2.0 | UI 引擎 | 动画图像（GIF/WebP/APNG）解码与播放，供 Animator 控件使用 |
| `cobolt-codegen` | 0.2.0 | 工具 | 窗体 → COBOL 源码生成器 |
| `cobolt-compiler` | 0.2.0 | 工具 | 内嵌＋打包编译器：项目 → 一个原生可执行文件 |
| `cobolt-dap` | 0.2.0 | 工具 | 线级兼容的 Debug Adapter Protocol——分帧、类型、客户端与适配器服务端 |
| `cobolt-agents` | 0.2.0 | AI | 智能体网格、知识库索引、嵌入向量、检索 |
| `cobolt-cli` | 0.2.0 | 可执行程序 | `rcrun` —— run、check、build、run-form |
| `cobolt-ide` | 0.2.0 | 可执行程序 | IDE 本身 |
| `cobolt-bench` | 0.2.0 | 可执行程序 | 性能与内存分配基线测试框架（参见 [BENCHMARKS-cn.md](BENCHMARKS-cn.md)） |

---

## 外部依赖

`Used by` 列出的是去掉 `cobolt-` 前缀后的工作空间 crate 名。

### 界面与渲染

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | 立即模式 GUI 工具包——整个界面 |
| `eframe` | 0.36.0 | cli, ide | 为 egui 提供窗口与事件循环的宿主 |
| `egui_extras` | 0.36.0 | cli, ide | 表格、图像加载器、额外控件 |
| `egui_commonmark` | 0.25.0 | ide | 文档/聊天面板中的 Markdown 渲染 |
| `egui_inspection` | 0.36.0 | ide | 实时的控件与布局检查器 |
| `image` | 0.25.10 | cli, forms, ide, media | PNG/JPEG/GIF/WebP/BMP 解码 |
| `resvg` | 0.46.0 | forms, ide | SVG 栅格化 |
| `fontdb` | 0.23.0 | forms, ide | 枚举系统字体 |
| `skrifa` | 0.42.1 | forms | 用 epaint 自身使用的同一个解析器校验字体 |
| `rfd` | 0.14.1 | ide | 原生的打开/保存对话框 |
| `syntect` | 5.3.0 | ide | 编辑器中的语法高亮 |
| `pulldown-cmark` | 0.12.2 | ide | Markdown 解析 |
| `mermaid-rs-renderer` | 0.2.2 | ide | mermaid 图渲染 |
| `genpdf` | 0.2.0 | ide | PDF 导出 |
| `pollster` | 0.3.0 | ide | 对 IDE 发起的少量异步调用进行阻塞等待 |

### 语言前端

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | 词法分析器生成器 |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | 保持插入顺序的映射——在 COBOL 里声明顺序是有语义的 |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | 错误类型 |

### 数据、存储与 I/O

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | 纯 Rust 的嵌入式 ACID 存储——INDEXED 文件与知识库索引 |
| `rusqlite` | 0.32.1 | runtime | 供 COBOL 数据库运行时使用的 SQLite（内置；会编译 C） |
| `postgres` | 0.19.13 | runtime | PostgreSQL 驱动（纯 Rust，同步） |
| `mysql` | 28.0.0 | runtime | MySQL 驱动（纯 Rust，`minimal-rust` 特性集——**无 TLS**；参见 [database-runtime-cn.md](database-runtime-cn.md)） |
| `ureq` | 2.12.1 | runtime | 供 COBOL REST 运行时使用的阻塞式 HTTP 客户端 |
| `native-tls` | 0.2.18 | runtime | 通过操作系统栈提供 TLS——没有需要编译的内置加密库 |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | 用于模型调用与网络调用的 HTTP 客户端 |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` 的序列化 |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | 序列化框架 |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML（上游已弃用；已固定版本） |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`、主题清单 |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | 已编译 AST 的紧凑二进制编码 |
| `flate2` | 1.1.9 | compiler | Deflate——压缩内嵌的 AST |
| `zip` | 2.4.2 | cli, ide | 项目压缩包的导入/导出 |
| `include_dir` | 0.7.4 | ide | 把随附文档烘焙进二进制文件 |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | 临时文件（同时也是开发依赖） |
| `dirs` | 5.0.1 | ide | 各平台的配置/数据目录 |

### AI 与检索

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | 智能体/LLM 编排（native-tls，不是 rustls） |
| `candle-core` | 0.11.0 | agents | 纯 Rust 张量运行时 |
| `candle-nn` | 0.11.0 | agents | Candle 的神经网络层 |
| `candle-transformers` | 0.11.0 | agents | BERT 之类——在进程内运行 `all-MiniLM-L6-v2` |
| `tokenizers` | 0.23.1 | agents | HuggingFace 分词器（`esaxx_fast` 关闭，`onig` 开启） |
| `schemars` | 1.2.1 | agents, ide | 用于工具定义的 JSON Schema |
| `tokio` | 1.52.3 | agents, ide | 智能体层的异步运行时 |
| `futures` | 0.3.32 | agents | 异步组合子 |

### 横切关注点

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | 结构化日志 |
| `tracing-subscriber` | 0.3.23 | cli, ide | 日志过滤与格式化 |
| `sysinfo` | 0.31.4 | ide | 进程/内存统计 |
| `num_cpus` | 1.17.0 | agents | 并行度设定 |
| `rand` | 0.8.6 | ide | 随机值 |
| `hmac` | 0.12.1 | forms | 用于绑定签名的 HMAC |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | 可读的测试差异（开发依赖） |

---

## 可选特性

`cobolt-agents` 只声明了一个可选特性，并且在默认构建中是**关闭**的：

| 特性 | 引入什么 | 为什么是可选的 |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | 在 Linux 和 Windows 上用 NVIDIA GPU 生成嵌入向量。构建它需要 CUDA 工具包，这正是它要显式开启的原因；没有它时嵌入生成会在 CPU 上运行 |

> **已在 1.41.4 移除。** 本节过去在 `local-retrieval` 和 `otel` 之下列出过
> `tantivy`、`sqlite-vec`、`rig-sqlite`、`tokio-rusqlite`、`ort`、`ndarray` 和
> `opentelemetry-otlp`。这两个特性都已不复存在，那些 crate 也没有在工作空间的任何
> 地方被声明——检索由 `cobolt-agents/src/knowledge_store.rs` 中树内的存储承担，它
> 把嵌入向量保存为 `Vec<f32>`，并用普通的点积来比较。

---

## 会编译 C 的两个 crate

配置机器时值得了解（参见 [BUILDING-cn.md](BUILDING-cn.md)）：

| Crate | 经由何处引入 | 它编译什么 |
|---|---|---|
| `libsqlite3-sys` | `rusqlite`（位于 `cobolt-runtime`） | SQLite 的 C 合并源码，已内置，因此无需与系统 SQLite 对版本 |
| `onig_sys` | `tokenizers` → `onig` | Oniguruma 正则引擎 |

树中没有任何东西编译 **C++**，也没有任何构建脚本调用 CMake、NASM、Python、Node
或 JVM。

.<<

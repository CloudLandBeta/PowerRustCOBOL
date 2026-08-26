<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# crate 清单

PowerRustCOBOL **直接**依赖的每一个 crate，以及实际链接的版本（不是需求字符串，
而是从 `Cargo.lock` 解析出来的那个）。

于 **2026-07-27** 由 `cargo metadata` 生成，对应产品版本 **1.37.0**。请注意这里
有两套编号：*产品*版本在 `crates/cobolt-ide/src/version.rs` 中，也是 IDE 里显示的
那个；`Cargo.toml` 中的 *crate* 版本是 `0.2.0`，由 workspace 的所有 crate 共享。
重新生成版本列：

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

完整依赖图共 **906 个包**。下面的表格列出的是 workspace 自己点名的约 56 个；其余
全部经由它们传递而来。

---

## workspace 内的 crate

*构成* PowerRustCOBOL 的 14 个 crate。它们共享 workspace 的 crate 版本
`0.2.0`（见上面的说明——产品版本是 1.37.0）。

| Crate | crate 版本 | 层次 | 作用 |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | 前端 | 富士通 COBOL 分词器——固定格式与自由格式源码 |
| `cobolt-parser` | 0.2.0 | 前端 | 递归下降语法分析器：词法流 → AST |
| `cobolt-ast` | 0.2.0 | 前端 | AST 节点类型 |
| `cobolt-semantic` | 0.2.0 | 前端 | 名称解析、类型检查、`EXEC RUST` 绑定 |
| `cobolt-runtime` | 0.2.0 | 执行 | 树遍历解释器、值系统、`EXEC RUST` 执行器、数据库／HTTP 运行时 |
| `cobolt-stdlib` | 0.2.0 | 执行 | 内建函数、I/O 后端、控制台辅助 |
| `cobolt-indexed` | 0.2.0 | 执行 | 索引文件定义模型（`.cidx`） |
| `cobolt-forms` | 0.2.0 | UI 引擎 | 表单／控件模型（`.cfrm`）、统一渲染引擎、主题、动画 |
| `cobolt-media` | 0.2.0 | UI 引擎 | 为 Animator 控件解码并播放动图（GIF/WebP/APNG） |
| `cobolt-codegen` | 0.2.0 | 工具链 | 由表单生成 COBOL 源码 |
| `cobolt-compiler` | 0.2.0 | 工具链 | 内嵌＋打包编译器：项目 → 一个原生可执行文件 |
| `cobolt-agents` | 0.2.0 | AI | 智能体网格、知识库索引、向量嵌入、检索 |
| `cobolt-cli` | 0.2.0 | 二进制 | `rcrun` —— run、check、build、run-form |
| `cobolt-ide` | 0.2.0 | 二进制 | IDE 本体 |

---

## 外部依赖

`使用方` 一列中的 workspace crate 名称省略了 `cobolt-` 前缀。

### 界面与渲染

| Crate | 版本 | 使用方 | 作用 |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | 立即模式 GUI 工具包——整个界面 |
| `eframe` | 0.35.0 | cli, ide | 承载 egui 的窗口与事件循环 |
| `egui_extras` | 0.35.0 | cli, ide | 表格、图片加载器、额外控件 |
| `egui_glow` | 0.35.0 | ide | OpenGL 绘制器——圆角裁剪钩子需要它 |
| `egui_commonmark` | 0.24.0 | ide | 文档／聊天面板中的 Markdown 渲染 |
| `egui_inspection` | 0.35.0 | ide | 实时控件／布局检查器 |
| `image` | 0.25.10 | cli, forms, ide, media | PNG/JPEG/GIF/WebP/BMP 解码 |
| `resvg` | 0.46.0 | forms, ide | SVG 栅格化 |
| `fontdb` | 0.23.0 | forms, ide | 枚举系统字体 |
| `skrifa` | 0.42.1 | forms | 用 epaint 自身所用的同一解析器校验字体 |
| `rfd` | 0.14.1 | ide | 原生的打开／保存对话框 |
| `syntect` | 5.3.0 | ide | 编辑器中的语法高亮 |
| `pulldown-cmark` | 0.12.2 | ide | Markdown 解析 |
| `mermaid-rs-renderer` | 0.2.2 | ide | mermaid 图渲染 |
| `genpdf` | 0.2.0 | ide | 导出 PDF |
| `pollster` | 0.3.0 | ide | 阻塞等待 IDE 少量的异步调用 |

### 语言前端

| Crate | 版本 | 使用方 | 作用 |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | 词法分析器生成器 |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | 保持插入顺序的映射——COBOL 中声明顺序具有语义 |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | 错误类型 |

### 数据、存储与 I/O

| Crate | 版本 | 使用方 | 作用 |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | 纯 Rust 的嵌入式 ACID 存储——INDEXED 文件与知识库索引 |
| `rusqlite` | 0.32.1 | runtime | 供 COBOL 数据库运行时使用的 SQLite（内置；会编译 C） |
| `postgres` | 0.19.13 | runtime | PostgreSQL 驱动（纯 Rust，同步） |
| `mysql` | 28.0.0 | runtime | MySQL 驱动（纯 Rust，rustls 特性组） |
| `ureq` | 2.12.1 | runtime | 供 COBOL REST 运行时使用的阻塞式 HTTP 客户端 |
| `native-tls` | 0.2.18 | runtime | 走操作系统栈的 TLS——没有需要编译的内置加密库 |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | 用于模型调用和网络调用的 HTTP 客户端 |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` 的序列化 |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | 序列化框架 |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML（上游已弃用；版本已锁定） |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`、主题清单 |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | 编译后 AST 的紧凑二进制编码 |
| `flate2` | 1.1.9 | compiler | Deflate——压缩内嵌的 AST |
| `zip` | 2.4.2 | cli, ide | 项目归档的导入／导出 |
| `include_dir` | 0.7.4 | ide | 把随附文档烘焙进二进制文件 |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | 临时文件（同时也是开发依赖） |
| `dirs` | 5.0.1 | ide | 各平台的配置／数据目录 |

### AI 与检索

| Crate | 版本 | 使用方 | 作用 |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | 智能体／LLM 编排（native-tls，而非 rustls） |
| `candle-core` | 0.11.0 | agents | 纯 Rust 张量运行时 |
| `candle-nn` | 0.11.0 | agents | Candle 的神经网络层 |
| `candle-transformers` | 0.11.0 | agents | BERT 之类——在进程内运行 `all-MiniLM-L6-v2` |
| `tokenizers` | 0.23.1 | agents | HuggingFace 分词器（`esaxx_fast` 关闭，`onig` 开启） |
| `embedvec` | 0.8.0 | agents | 向量存储：E8 量化、余弦相似度 |
| `schemars` | 1.2.1 | agents, ide | 工具定义所用的 JSON Schema |
| `opentelemetry` | 0.32.0 | agents | 追踪／指标 API |
| `tokio` | 1.52.3 | agents, ide | 智能体层的异步运行时 |
| `futures` | 0.3.32 | agents | 异步组合子 |

### 横切关注点

| Crate | 版本 | 使用方 | 作用 |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | 结构化日志 |
| `tracing-subscriber` | 0.3.23 | cli, ide | 日志过滤与格式化 |
| `sysinfo` | 0.31.4 | ide | 进程／内存统计 |
| `num_cpus` | 1.17.0 | agents | 并行度设定 |
| `rand` | 0.8.6 | ide | 随机值 |
| `hmac` | 0.12.1 | forms | 用于绑定签名的 HMAC |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | 可读性更好的测试差异（开发依赖） |

---

## 已声明但默认不链接

这些在某个 `Cargo.toml` 中被写在默认构建下**关闭**的 feature 之后，因此除非你
打开该 feature，它们对编译时间和二进制体积都毫无贡献：

| Crate | Feature | 为什么是可选的 |
|---|---|---|
| `tantivy` | `local-retrieval` | 词法索引——默认路径是 `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | 基于 SQLite 的向量检索；启用它会把内置 SQLite（以及一套 C 工具链）带进 `cobolt-agents` |
| `ort`, `ndarray` | `local-retrieval` | ONNX Runtime 推理路径 |
| `opentelemetry-otlp` | `otel` | OTLP 导出 |

---

## 会编译 C 的那两个 crate

在准备机器时值得知道（参见 [BUILDING-en.md](BUILDING-en.md)）：

| Crate | 经由 | 编译什么 |
|---|---|---|
| `libsqlite3-sys` | `rusqlite`（位于 `cobolt-runtime`） | SQLite 的 C amalgamation，内置进来，因此无需匹配系统上的 SQLite |
| `onig_sys` | `tokenizers` → `onig` | Oniguruma 正则引擎 |

代码树中没有任何东西编译 **C++**，也没有任何构建脚本调用 CMake、NASM、Python、
Node 或 JVM。

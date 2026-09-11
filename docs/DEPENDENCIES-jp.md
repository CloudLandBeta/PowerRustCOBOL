<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# crate 一覧

PowerRustCOBOL が**直接**依存しているすべての crate と、実際にリンクされる
バージョン（要求文字列ではなく、`Cargo.lock` で解決されたもの）。

最初に `cargo metadata` から生成したのは **2026-07-27**、製品バージョン
**1.37.0** の時点です。以下の解決済みバージョンは **2026-09-11**、**1.65.128** で
`Cargo.lock` と最後に突き合わせました。2 つの採番体系がある点に注意してください。
*製品*バージョンは `crates/cobolt-ide/src/version.rs` にあり IDE に表示されるもの、
*crate* バージョンは `Cargo.toml` の `0.2.0` で、ワークスペースのすべての crate が
共有します。バージョン列は次のコマンドで再生成できます。

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

依存グラフ全体は **944 パッケージ**です。以下の表は、ワークスペース自身が名前を
挙げている **59** 件です。それ以外はすべて、これらを通じて推移的に入ってきます。

---

## ワークスペースの crate

PowerRustCOBOL そのものである 17 個の crate です。ここでの正典は
`cargo metadata --no-deps` であって、`Cargo.toml` の grep ではありません。あちらで
は 2 つのメンバーが 1 行を共有しているため、素朴に数えると 16 と出ます。すべてが
ワークスペースの crate バージョン `0.2.0` を共有します（上の注記を参照。製品バージ
ョンは独自の系列です）。

| Crate | crate バージョン | 層 | 役割 |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | フロントエンド | Fujitsu COBOL のトークナイザー — 固定形式・自由形式の両方 — と `COPY`/`REPLACE` プリプロセッサー |
| `cobolt-parser` | 0.2.0 | フロントエンド | 再帰下降パーサー: トークン列 → AST |
| `cobolt-ast` | 0.2.0 | フロントエンド | AST のノード型 |
| `cobolt-semantic` | 0.2.0 | フロントエンド | 名前解決、型検査、`EXEC RUST` の束縛 |
| `cobolt-runtime` | 0.2.0 | 実行 | 木を歩くインタープリター、値システム、`EXEC RUST` の実行器、DB/HTTP ランタイム |
| `cobolt-stdlib` | 0.2.0 | 実行 | 組込み関数、I/O バックエンド、コンソール補助 |
| `cobolt-indexed` | 0.2.0 | 実行 | 索引ファイル定義モデル (`.cidx`) |
| `cobolt-forms` | 0.2.0 | UI エンジン | フォーム/コントロールのモデル (`.cfrm`)、統一レンダーエンジン、テーマ、アニメーション |
| `cobolt-form-host` | 0.2.0 | UI エンジン | 唯一のフォームホスト (spec 042) — `rcrun run-form` とコンパイル済みアプリケーションが共有 |
| `cobolt-media` | 0.2.0 | UI エンジン | アニメーション画像 (GIF/WebP/APNG) のデコードと再生。Animator ウィジェット用 |
| `cobolt-codegen` | 0.2.0 | ツール | フォーム → COBOL ソースの生成器 |
| `cobolt-compiler` | 0.2.0 | ツール | 埋め込み＋同梱コンパイラー: プロジェクト → 1 つのネイティブ実行ファイル |
| `cobolt-dap` | 0.2.0 | ツール | ワイヤー互換の Debug Adapter Protocol — フレーミング、型、クライアント、アダプターサーバー |
| `cobolt-agents` | 0.2.0 | AI | エージェントのメッシュ、ナレッジベース索引、埋め込み、検索 |
| `cobolt-cli` | 0.2.0 | バイナリ | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | バイナリ | IDE 本体 |
| `cobolt-bench` | 0.2.0 | バイナリ | 性能とアロケーションのベースライン計測ハーネス（[BENCHMARKS-jp.md](BENCHMARKS-jp.md) を参照） |

---

## 外部依存

`Used by` は、`cobolt-` 接頭辞を落としたワークスペースの crate 名です。

### UI と描画

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | 即時モードの GUI ツールキット — UI のすべて |
| `eframe` | 0.36.0 | cli, ide | egui のためのウィンドウとイベントループのホスト |
| `egui_extras` | 0.36.0 | cli, ide | テーブル、画像ローダー、追加ウィジェット |
| `egui_commonmark` | 0.25.0 | ide | ドキュメント/チャットパネルでの Markdown 描画 |
| `egui_inspection` | 0.36.0 | ide | ウィジェットとレイアウトのライブ検査ツール |
| `image` | 0.25.10 | cli, forms, ide, media | PNG/JPEG/GIF/WebP/BMP のデコード |
| `resvg` | 0.46.0 | forms, ide | SVG のラスタライズ |
| `fontdb` | 0.23.0 | forms, ide | システムフォントの列挙 |
| `skrifa` | 0.42.1 | forms | epaint 自身が使うのと同じパーサーによるフォントフェイスの検証 |
| `rfd` | 0.14.1 | ide | ネイティブの「開く/保存」ダイアログ |
| `syntect` | 5.3.0 | ide | エディターの構文ハイライト |
| `pulldown-cmark` | 0.12.2 | ide | Markdown の解析 |
| `mermaid-rs-renderer` | 0.2.2 | ide | mermaid 図の描画 |
| `genpdf` | 0.2.0 | ide | PDF 出力 |
| `pollster` | 0.3.0 | ide | IDE が行うわずかな非同期呼び出しをブロックして待つ |

### 言語フロントエンド

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | 字句解析器ジェネレーター |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | 挿入順を保つマップ — COBOL では宣言順に意味がある |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | エラー型 |

### データ・ストレージ・I/O

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | 純 Rust の組込み ACID ストア — INDEXED ファイルとナレッジベース索引 |
| `rusqlite` | 0.32.1 | runtime | COBOL データベースランタイム用の SQLite（同梱。C をコンパイルする） |
| `postgres` | 0.19.13 | runtime | PostgreSQL ドライバー（純 Rust、同期） |
| `mysql` | 28.0.0 | runtime | MySQL ドライバー（純 Rust、`minimal-rust` フィーチャー構成 — **TLS なし**。[database-runtime-jp.md](database-runtime-jp.md) を参照） |
| `ureq` | 2.12.1 | runtime | COBOL の REST ランタイム用のブロッキング HTTP クライアント |
| `native-tls` | 0.2.18 | runtime | OS のスタック経由の TLS — コンパイルすべき同梱の暗号ライブラリなし |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | モデル呼び出しと Web 呼び出しのための HTTP クライアント |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` のシリアライズ |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | シリアライズのフレームワーク |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML（上流では非推奨。バージョン固定） |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`、テーマのマニフェスト |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | コンパイル済み AST のコンパクトなバイナリ符号化 |
| `flate2` | 1.1.9 | compiler | Deflate — 埋め込み AST を圧縮する |
| `zip` | 2.4.2 | cli, ide | プロジェクト書庫の取り込みと書き出し |
| `include_dir` | 0.7.4 | ide | 同梱ドキュメントをバイナリに焼き込む |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | 一時ファイル（開発依存でもある） |
| `dirs` | 5.0.1 | ide | プラットフォームごとの設定/データディレクトリ |

### AI と検索

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | エージェント/LLM のオーケストレーション（rustls ではなく native-tls） |
| `candle-core` | 0.11.0 | agents | 純 Rust のテンソルランタイム |
| `candle-nn` | 0.11.0 | agents | Candle 用のニューラルネットワーク層 |
| `candle-transformers` | 0.11.0 | agents | BERT とその仲間 — `all-MiniLM-L6-v2` をプロセス内で動かす |
| `tokenizers` | 0.23.1 | agents | HuggingFace のトークナイザー（`esaxx_fast` は無効、`onig` は有効） |
| `schemars` | 1.2.1 | agents, ide | ツール定義のための JSON Schema |
| `tokio` | 1.52.3 | agents, ide | エージェント層のための非同期ランタイム |
| `futures` | 0.3.32 | agents | 非同期コンビネーター |

### 横断的なもの

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | 構造化ログ |
| `tracing-subscriber` | 0.3.23 | cli, ide | ログのフィルタリングと整形 |
| `sysinfo` | 0.31.4 | ide | プロセスとメモリの統計 |
| `num_cpus` | 1.17.0 | agents | 並列度の決定 |
| `rand` | 0.8.6 | ide | 乱数値 |
| `hmac` | 0.12.1 | forms | バインディング署名のための HMAC |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | 読みやすいテスト差分（開発依存） |

---

## 任意フィーチャー

`cobolt-agents` が宣言している任意フィーチャーはちょうど 1 つで、既定のビルドでは
**無効**です。

| フィーチャー | 何を引き込むか | なぜ任意なのか |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | Linux と Windows での NVIDIA GPU による埋め込み生成。ビルドに CUDA ツールキットが必要なため任意になっています。無い場合、埋め込み生成は CPU で動きます |

> **1.41.4 で削除。** この節にはかつて `local-retrieval` と `otel` の背後に
> `tantivy`、`sqlite-vec`、`rig-sqlite`、`tokio-rusqlite`、`ort`、`ndarray`、
> `opentelemetry-otlp` が並んでいました。どちらのフィーチャーももう存在せず、これら
> の crate はワークスペースのどこにも宣言されていません。検索は
> `cobolt-agents/src/knowledge_store.rs` にあるツリー内のストアが担っており、埋め込
> みを `Vec<f32>` として保持し、単純な内積で比較します。

---

## C をコンパイルする 2 つの crate

マシンを用意するときに知っておく価値があります（[BUILDING-jp.md](BUILDING-jp.md) を参照）。

| Crate | 到達経路 | 何をコンパイルするか |
|---|---|---|
| `libsqlite3-sys` | `rusqlite`（`cobolt-runtime` 内） | SQLite の C アマルガメーション。同梱されているので、システムの SQLite とバージョンを合わせる必要がない |
| `onig_sys` | `tokenizers` → `onig` | 鬼車（Oniguruma）正規表現エンジン |

ツリー内のどれも **C++** をコンパイルせず、どのビルドスクリプトも CMake、NASM、
Python、Node、JVM を呼び出しません。

.<<

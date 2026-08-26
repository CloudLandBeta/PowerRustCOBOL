<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# crate 一覧

PowerRustCOBOL が**直接**依存しているすべての crate と、実際にリンクされている
バージョン（要求文字列ではなく、`Cargo.lock` から解決された実バージョン）。

**2026-07-27** に `cargo metadata` から生成、製品バージョン **1.37.0** 時点。
2 つの番号体系がある点に注意してください。*製品*バージョンは
`crates/cobolt-ide/src/version.rs` にあり IDE に表示されるもの、`Cargo.toml` の
*crate* バージョンは `0.2.0` で、ワークスペースのすべての crate が共有します。
バージョン列を再生成するには:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

依存グラフ全体は **906 パッケージ**です。以下の表はワークスペース自身が名前を
挙げている約 56 個で、それ以外はすべてこれらを経由して推移的に入ってきます。

---

## ワークスペースの crate

PowerRustCOBOL を*構成している* 14 個の crate。すべてワークスペースの crate
バージョン `0.2.0` を共有します（上の注記のとおり、製品バージョンは 1.37.0）。

| Crate | crate バージョン | レイヤ | 役割 |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | フロントエンド | 富士通 COBOL トークナイザ — 固定形式・自由形式の両方 |
| `cobolt-parser` | 0.2.0 | フロントエンド | 再帰下降パーサ: トークン列 → AST |
| `cobolt-ast` | 0.2.0 | フロントエンド | AST ノードの型 |
| `cobolt-semantic` | 0.2.0 | フロントエンド | 名前解決、型検査、`EXEC RUST` の束縛 |
| `cobolt-runtime` | 0.2.0 | 実行 | 木を走査するインタプリタ、値システム、`EXEC RUST` 実行器、DB/HTTP ランタイム |
| `cobolt-stdlib` | 0.2.0 | 実行 | 組み込み関数、I/O バックエンド、コンソール補助 |
| `cobolt-indexed` | 0.2.0 | 実行 | 索引ファイル定義モデル（`.cidx`） |
| `cobolt-forms` | 0.2.0 | UI エンジン | フォーム／コントロールのモデル（`.cfrm`）、統一レンダリングエンジン、テーマ、アニメーション |
| `cobolt-media` | 0.2.0 | UI エンジン | Animator ウィジェット向けのアニメーション画像（GIF/WebP/APNG）のデコードと再生 |
| `cobolt-codegen` | 0.2.0 | ツール | フォームから COBOL ソースを生成 |
| `cobolt-compiler` | 0.2.0 | ツール | 埋め込み＋同梱コンパイラ: プロジェクト → 1 つのネイティブ実行ファイル |
| `cobolt-agents` | 0.2.0 | AI | エージェントメッシュ、ナレッジベース索引、埋め込み、検索 |
| `cobolt-cli` | 0.2.0 | バイナリ | `rcrun` — run、check、build、run-form |
| `cobolt-ide` | 0.2.0 | バイナリ | IDE 本体 |

---

## 外部依存

`使用元` の列は、ワークスペースの crate 名から `cobolt-` 接頭辞を落として示して
います。

### UI と描画

| Crate | バージョン | 使用元 | 役割 |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | イミディエイトモード GUI ツールキット — UI 全体 |
| `eframe` | 0.35.0 | cli, ide | egui のウィンドウとイベントループのホスト |
| `egui_extras` | 0.35.0 | cli, ide | テーブル、画像ローダ、追加ウィジェット |
| `egui_glow` | 0.35.0 | ide | OpenGL ペインタ — 角丸クリップのフックがこれを必要とする |
| `egui_commonmark` | 0.24.0 | ide | ドキュメント／チャットパネルでの Markdown 描画 |
| `egui_inspection` | 0.35.0 | ide | ウィジェット／レイアウトのライブインスペクタ |
| `image` | 0.25.10 | cli, forms, ide, media | PNG/JPEG/GIF/WebP/BMP のデコード |
| `resvg` | 0.46.0 | forms, ide | SVG のラスタライズ |
| `fontdb` | 0.23.0 | forms, ide | システムフォントの列挙 |
| `skrifa` | 0.42.1 | forms | epaint 自身が使うのと同じパーサでのフォント検証 |
| `rfd` | 0.14.1 | ide | ネイティブの開く／保存ダイアログ |
| `syntect` | 5.3.0 | ide | エディタの構文ハイライト |
| `pulldown-cmark` | 0.12.2 | ide | Markdown の解析 |
| `mermaid-rs-renderer` | 0.2.2 | ide | mermaid 図の描画 |
| `genpdf` | 0.2.0 | ide | PDF エクスポート |
| `pollster` | 0.3.0 | ide | IDE が行う数少ない非同期呼び出しをブロックして待つ |

### 言語フロントエンド

| Crate | バージョン | 使用元 | 役割 |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | 字句解析器ジェネレータ |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | 挿入順を保つマップ — COBOL では宣言順が意味を持つ |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | エラー型 |

### データ・ストレージ・入出力

| Crate | バージョン | 使用元 | 役割 |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | 純 Rust の組み込み ACID ストア — INDEXED ファイルとナレッジベース索引 |
| `rusqlite` | 0.32.1 | runtime | COBOL データベースランタイム用の SQLite（同梱、C をコンパイル） |
| `postgres` | 0.19.13 | runtime | PostgreSQL ドライバ（純 Rust、同期） |
| `mysql` | 28.0.0 | runtime | MySQL ドライバ（純 Rust、rustls の feature 構成） |
| `ureq` | 2.12.1 | runtime | COBOL REST ランタイム用のブロッキング HTTP クライアント |
| `native-tls` | 0.2.18 | runtime | OS のスタック経由の TLS — コンパイルすべき同梱暗号ライブラリなし |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | モデル呼び出しと Web 呼び出しのための HTTP クライアント |
| `quick-xml` | 0.36.2 | forms, indexed | `.cfrm` / `.cidx` のシリアライズ |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | シリアライズ基盤 |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML（上流で非推奨、バージョン固定） |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`、テーマのマニフェスト |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | コンパイル済み AST のコンパクトなバイナリ符号化 |
| `flate2` | 1.1.9 | compiler | Deflate — 埋め込む AST を圧縮する |
| `zip` | 2.4.2 | cli, ide | プロジェクト書庫の入出力 |
| `include_dir` | 0.7.4 | ide | 同梱ドキュメントをバイナリに焼き込む |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | 一時ファイル（開発用依存でもある） |
| `dirs` | 5.0.1 | ide | プラットフォームごとの設定／データディレクトリ |

### AI と検索

| Crate | バージョン | 使用元 | 役割 |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | エージェント／LLM のオーケストレーション（rustls ではなく native-tls） |
| `candle-core` | 0.11.0 | agents | 純 Rust のテンソルランタイム |
| `candle-nn` | 0.11.0 | agents | Candle 用のニューラルネットワーク層 |
| `candle-transformers` | 0.11.0 | agents | BERT ほか — `all-MiniLM-L6-v2` をプロセス内で実行 |
| `tokenizers` | 0.23.1 | agents | HuggingFace のトークナイザ（`esaxx_fast` オフ、`onig` オン） |
| `embedvec` | 0.8.0 | agents | ベクトルストア: E8 量子化、コサイン類似度 |
| `schemars` | 1.2.1 | agents, ide | ツール定義のための JSON Schema |
| `opentelemetry` | 0.32.0 | agents | トレース／メトリクス API |
| `tokio` | 1.52.3 | agents, ide | エージェント層の非同期ランタイム |
| `futures` | 0.3.32 | agents | 非同期コンビネータ |

### 横断的なもの

| Crate | バージョン | 使用元 | 役割 |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | 構造化ログ |
| `tracing-subscriber` | 0.3.23 | cli, ide | ログのフィルタリングと整形 |
| `sysinfo` | 0.31.4 | ide | プロセス／メモリの統計 |
| `num_cpus` | 1.17.0 | agents | 並列度の決定 |
| `rand` | 0.8.6 | ide | 乱数値 |
| `hmac` | 0.12.1 | forms | バインディング署名用の HMAC |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | 読みやすいテスト差分（開発用依存） |

---

## 宣言はされているが既定ではリンクされないもの

これらはどこかの `Cargo.toml` に、既定ビルドでは**オフ**の feature の後ろで
書かれています。feature を有効にしない限り、コンパイル時間にもバイナリサイズにも
一切寄与しません。

| Crate | Feature | 任意である理由 |
|---|---|---|
| `tantivy` | `local-retrieval` | 語彙索引 — 既定の経路は `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | SQLite を土台にしたベクトル検索。有効にすると同梱 SQLite（および C ツールチェーン）が `cobolt-agents` に入ってくる |
| `ort`, `ndarray` | `local-retrieval` | ONNX Runtime による推論経路 |
| `opentelemetry-otlp` | `otel` | OTLP エクスポート |

---

## C をコンパイルする 2 つの crate

マシンを用意するときに知っておくとよいことです（[BUILDING-en.md](BUILDING-en.md)
を参照）:

| Crate | 経由 | 何をコンパイルするか |
|---|---|---|
| `libsqlite3-sys` | `rusqlite`（`cobolt-runtime` 内） | SQLite の C amalgamation。同梱してあるのでシステム側の SQLite とバージョンを合わせる必要がない |
| `onig_sys` | `tokenizers` → `onig` | Oniguruma 正規表現エンジン |

ツリー内に **C++** をコンパイルするものはなく、CMake・NASM・Python・Node・JVM
を呼び出すビルドスクリプトもありません。

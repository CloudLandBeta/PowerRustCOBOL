<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# クラッシュに強い INDEXED エンジン (redb)

PowerRustCOBOL は `ORGANIZATION IS INDEXED` ファイル向けに、2 つめの
`STORAGE IS DISK` エンジンを同梱しています。基盤は **redb** — 純 Rust 製の組込み
ACID キーバリューストア（コピーオンライトの B+tree、二重のメタページ、ページ単位
のチェックサム）です。観測可能な COBOL の挙動は従来の `PRCIDXD1` エンジンと*まっ
たく同じ*でありながら、自前のエンジンでは規模が大きくなると満たせなかった 4 つの
運用目標を軸に設計されています。

**これが既定のエンジンであり、1.62.73 以降ずっとそうです**（オペレーター裁定、
2026-08-29）。`IndexedEngine` は `Redb` に `#[default]` を付けて `Default` を導出
しており（`crates/cobolt-runtime/src/indexed.rs:126`）、テストがそれを固定していま
す（`indexed.rs:1643`）。何も選択しなくてもこれが使われます。

従来のページ方式エンジンは名前を指定すれば今も利用でき、組込みの Rust コンテナーに
委譲する 2 つの別名も同様です。

```bash
rcrun run program.cbl --indexed-engine rust    # PRCIDXD1 のページ方式エンジン
# または
COBOL_INDEXED_ENGINE=rust rcrun run program.cbl
```

実装:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs)。

---

## なぜ — 4 つの目標

| 目標 | redb エンジンがどう満たすか |
|------|------------------------------|
| **OPEN は常に一瞬** | redb は開くときにメタページしか読みません。**RAM 上のレコードディレクトリを読み込むことも、復旧走査を行うこともありません** — クラッシュ直後であってもです。実測: 20 万レコードのファイルの OPEN に約 5 ms（レコード数に依存しません）。 |
| **READ RANDOM / NEXT が光速** | RANDOM は B+tree の下降、NEXT は逐次的な範囲イテレーターです。どちらも redb のページキャッシュ上で動きます。実測: 20 万レコードでランダム 1 読み取りあたり約 21 µs。 |
| **最大 2 億 5000 万レコード（データ量は無制限）** | 常駐 RAM はワーキングセット（redb のキャッシュ）で決まり、**レコード数では決まりません**。メモリ上に `O(レコード数)` の構造は一切保持しません。 |
| **安全性が最優先** | redb は完全な ACID です。`COMMIT` は耐久性のあるトランザクションコミット（fsync）、`ROLLBACK` はトランザクションのアボートです。電源断でインデックスが途中まで書けた状態が見えることは決してありません — redb は二重のメタページによって直前の正常なコミットへ戻ります。データ損失もインデックス破損もありません。 |

対する `PRCIDXD1` エンジンは、RecordId のディレクトリを OPEN 時に丸ごと RAM へ読み
込み（これまでに割り当てたすべての RecordId × 約 16 バイト）、トランザクションは
CLOSE 時にしか永続化されない RAM 上の取り消しログでした。だからこそ、規模が大きく
なると即座に開くこともできず、実行中の電源断に耐えることもできなかったのです。

---

## ディスク上の構成（redb テーブル）

| redb テーブル | 種類     | キー → 値                                      |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | 主キーのバイト列 → レコード（必要に応じて圧縮） |
| `alt`      | multimap | `[u16 idx][代替キーのバイト列]` → `[u64 seq][主キー]` |
| `seq`      | table    | 主キーのバイト列 → `u64` の挿入シーケンス      |
| `meta`     | table    | `schema`、`compress`、`nextseq` の各記述子     |

- **単一の `alt` マルチマップ**がすべての代替キーを保持し、2 バイトのビッグエン
  ディアンのキー索引で名前空間を分けます。したがってバイト順は
  `(キー索引, 代替値, 挿入シーケンス)` となり、重複する代替キーは**作成順**に走査
  されます。これはディスクエンジンの RecordId の順序、そして重複代替キーに関する
  COBOL の規則とちょうど一致します。
- `seq` / `meta:nextseq` の仕組みは、代替キーの重複を順序づけるため**だけ**に存在
  します。代替キーを持たないファイルはこれを完全に飛ばし、`WRITE` ごとに B+tree
  への挿入 1 回だけで済みます。
- レコードは固定幅の位置指定イメージとして格納されます
  （[`indexed-file-internals-jp.md`](indexed-file-internals-jp.md) §6 を参照）。
  `WITH COMPRESSION` は他のエンジンと同じ PackBits RLE を適用します。

---

## トランザクションモデル

書き込み可能なオープン（`OUTPUT` / `I-O` / `EXTEND`）は、OPEN の時点から redb の
`WriteTransaction` を 1 つ開いたまま保持します。そのトランザクション経由の読み取り
は、プログラム自身の未コミットの書き込みを見ることができます（COBOL の「自分の書き
込みを読む」）。COBOL の動詞は次のように直接対応します。

| COBOL | redb |
|-------|------|
| `OPEN`     | 書き込みトランザクションを開始（書き込み可能モード） |
| `COMMIT`   | トランザクションを `commit()`（耐久的）し、新しいものを開始 |
| `ROLLBACK` | トランザクションを `abort()`（直前の `COMMIT`/`OPEN` 以降をすべて破棄）し、新しいものを開始 |
| `CLOSE`    | `commit()`（暗黙のコミット） |

`INPUT` のオープンは短い読み取りトランザクションを使います。`ROLLBACK` が redb の
本物のアボートであるため、**取り消しログは一切不要です** — 耐久性とロールバックは
ストア自身の保証だからです。

> COBOL の `COMMIT` / `ROLLBACK` 動詞は **INDEXED ファイル**に作用し、SQL 接続には
> 作用しません（そちらは `COBOL-EXEC-SQL` に `BEGIN`/`COMMIT`/`ROLLBACK` を渡しま
> す）。

---

## 挙動の同等性

このエンジンは既定エンジンと寸分違わぬ挙動を要求されます。同じバージョン管理された
フィクスチャー（`tests/cobol/fileio/idx_crud.cbl`、`idx_persist.cbl`、
`idx_tx.cbl`）を `--indexed-engine redb` で実行し、DISPLAY の出力が完全に一致しな
ければなりません — 主キーと `WITH DUPLICATES` 付き代替キーでの CRUD、開き直しをま
たいだ永続性、そして `COMMIT`/`ROLLBACK` です。ファイル状態コード
（`00/02/10/22/23/35/39/46/47/48/49/90/...`）、参照キーの解決、`START` の意味論、
「REWRITE/DELETE には現在レコードが必要」という規則も、すべて一致します。

テスト: `crates/cobolt-runtime/tests/test_indexed_redb.rs`（redb でのフィクスチャー
＋ `IndexedStore` の直接検査 ＋ `#[ignore]` 付きの大規模スモークテスト）。

---

## 制限

このエンジンは要求時ページングであるため、実用上の制限は常駐 RAM ではなく redb と
ファイルシステムが決めます。

| 項目 | 制限 |
|-----------|-------|
| ファイルサイズ | redb / ファイルシステムの上限（テラバイト級） |
| レコード数 | レコード数ではなくワーキングセットの RAM で決まる（小さなキャッシュで 2 億 5000 万件以上） |
| レコードサイズ | 固定幅イメージ。大きなレコードは redb の値として格納 |
| キーサイズ | 複合キーのバイト長（COBOL 層が多部構成キーに対応） |
| 代替キー | 最大 65 535 個（2 バイトの索引名前空間） |

---

## 性能に関する注記

- 参照キーが主キーである場合の**逐次 `READ NEXT`** は、範囲カーソルからそのまま
  レコードを返します — レコードあたり B+tree の下降は 2 回ではなく 1 回です
  （20 万件でレコードあたり約 17 µs）。代替キーの走査は、いまも代替側の下降 1 回に
  加えて主キー側の取得を行います。
- **`WRITE`** は操作ごとに `primary`/`alt` テーブルを 1 度開きます（重複チェックと
  挿入がハンドルを共有します）。マイクロベンチマークでは、呼び出しを*またいで*
  ハンドルをキャッシュしても、操作ごとに開く方式に対して約 8 % しか上乗せされない
  と分かったため、エンジンは単純で `unsafe` のない経路を保っています。書き込みの
  コスト（レコードあたり約 44 µs）は redb の ACID な B+tree 挿入に支配されており、
  これが安全側の下限です — どの書き込み最適化もコミット地点や耐久性を変えません。
- したがって**一括 `WRITE`** は単一トランザクションで毎秒約 2 万レコードです（一度
  きりのロードコスト）。OPEN、読み取り、クラッシュ安全性はいずれも影響を受けませ
  ん。

---

## 可観測性ログ (`--indexed-log`)

redb エンジンは、ファイルごとの任意のトランザクションログ（既定では無効）を
**`<assign のパス>.log`**（例: `customers.idx` → `customers.idx.log`）に書き出せ
ます。`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` ごとに 1 行を記録し、タイムスタンプ、レコ
ード数とバイト数、スループット、書き込み時のキー順序の品質、そして `full` レベルで
は redb のインデックスページ統計を残します。

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

行の形式は `text`（logfmt）または `json`（NDJSON、Grafana/Loki 対応）です。

**完全なリファレンス** — フラグ、フィールド表、形式、Grafana/Loki のパイプライン
（Promtail + LogQL）、コストと安全性に関する注記 — は
[`observability-jp.md`](observability-jp.md) §1 にあります。

.<<

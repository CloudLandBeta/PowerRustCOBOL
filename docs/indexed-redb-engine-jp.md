<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# クラッシュ安全な INDEXED エンジン（redb）

PowerRustCOBOL は `ORGANIZATION IS INDEXED` ファイル向けに 2 つめの
`STORAGE IS DISK` エンジンを同梱しています。基盤は **redb** — 純 Rust の組み込み
ACID キーバリューストア（コピーオンライトの B+ 木、二重メタページ、ページごとの
チェックサム）です。COBOL から観測できる振る舞いは既定の `PRCIDXD1` エンジンと
*完全に同一*ですが、既製のエンジンでは規模が大きくなると満たせなかった 4 つの
運用目標を軸に設計されています。

現時点では**任意**です（既定のディスクエンジンは引き続き `PRCIDXD1`）:

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

実装:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs)。

---

## なぜ — 4 つの目標

| 目標 | redb エンジンがどう満たすか |
|------|------------------------------|
| **OPEN は常に一瞬** | redb は開くときにメタページしか読みません。**RAM 上のレコードディレクトリを読み込むこともなければ、復旧走査もありません**。クラッシュ後でも同じです。実測: 20 万件のファイルの OPEN に約 5 ms（レコード件数に依存しない）。 |
| **READ RANDOM / NEXT が高速** | RANDOM は B+ 木の下降、NEXT は範囲の逐次イテレータ。どちらも redb のページキャッシュ上で動きます。実測: 20 万件で 1 回のランダム読み出しあたり約 21 µs。 |
| **最大 2.5 億レコード（データ量は無制限）** | 常駐 RAM はワーキングセット（redb のキャッシュ）で決まり、レコード件数では**決まりません**。`O(レコード数)` の構造をメモリに保持しません。 |
| **安全性が最優先** | redb は完全な ACID です。`COMMIT` は耐久性のあるトランザクションコミット（fsync）、`ROLLBACK` はトランザクションの中止です。電源断で索引が途中で壊れた状態が露出することは決してありません — redb は二重メタページによって直前の正常なコミットへ戻ります。データ損失も索引の破損もありません。 |

`PRCIDXD1` エンジンと対照的です。あちらは OPEN 時に RecordId ディレクトリを丸ごと
RAM に読み込み（これまでに割り当てられた RecordId 1 件あたり約 16 バイト）、
トランザクションは CLOSE 時にのみ永続化される RAM 上の undo ログでした。その
ため、規模が大きいと即座に開くこともできず、実行中の電源断にも耐えられません
でした。

---

## ディスク上の構成（redb のテーブル）

| redb テーブル | 種別     | キー → 値                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | 主キーのバイト列 → レコード（必要に応じて圧縮） |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | 主キーのバイト列 → `u64` の挿入シーケンス  |
| `meta`     | table    | `schema`、`compress`、`nextseq` の記述子   |

- **単一の `alt` multimap** がすべての副キーを保持し、2 バイトのビッグエンディ
  アンのキー索引で名前空間を分けます。したがってバイト順は
  `(キー索引, 副キー値, 挿入シーケンス)` となり、副キーの重複が**作成順**で
  たどられます。これはディスクエンジンの RecordId 順序、そして重複副キーに関する
  COBOL の規則とちょうど一致します。
- `seq` / `meta:nextseq` の仕組みは、副キーの重複を順序づけるため**だけ**に
  存在します。副キーを持たないファイルはこれを完全に飛ばし、`WRITE` あたり
  B+ 木への挿入 1 回で済みます。
- レコードは位置固定・固定幅のイメージとして格納されます
  （[`indexed-file-internals-jp.md`](indexed-file-internals-jp.md) §6 を参照）。
  `WITH COMPRESSION` は他のエンジンと同じ PackBits RLE を適用します。

---

## トランザクションモデル

書き込み可能な OPEN（`OUTPUT` / `I-O` / `EXTEND`）は、OPEN の時点から redb の
`WriteTransaction` を 1 つ開いたまま保持します。そのトランザクション越しの読み
出しは、プログラム自身のまだコミットしていない書き込みを見ます（COBOL の
「自分の書いたものを読む」）。COBOL の動詞はそのまま対応します。

| COBOL | redb |
|-------|------|
| `OPEN`     | 書き込みトランザクションを開始する（書き込みモード） |
| `COMMIT`   | トランザクションを `commit()`（耐久性あり）し、新しいものを開始する |
| `ROLLBACK` | トランザクションを `abort()`（直前の `COMMIT`/`OPEN` 以降をすべて破棄）し、新しいものを開始する |
| `CLOSE`    | `commit()`（暗黙のコミット） |

`INPUT` の OPEN は短い読み出しトランザクションを使います。`ROLLBACK` が redb の
真の abort であるため、**undo ログは不要**です — 耐久性とロールバックはストア
自身の保証です。

> COBOL の `COMMIT` / `ROLLBACK` は **INDEXED ファイル**に作用し、SQL 接続には
> 作用しません（そちらは `COBOL-EXEC-SQL` で `BEGIN`/`COMMIT`/`ROLLBACK` を
> 使います）。

---

## 振る舞いの同一性

このエンジンは既定エンジンとまったく同じ振る舞いを求められます。同じバージョン
管理されたフィクスチャ（`tests/cobol/fileio/idx_crud.cbl`、`idx_persist.cbl`、
`idx_tx.cbl`）を `--indexed-engine redb` の下で実行し、DISPLAY 出力が完全に一致
しなければなりません — 主キーと `WITH DUPLICATES` 付き副キーによる CRUD、開き
直しをまたぐ永続性、そして `COMMIT`/`ROLLBACK`。ファイル状態コード
（`00/02/10/22/23/35/39/46/47/48/49/90/...`）、参照キーの解決、`START` の意味、
「REWRITE/DELETE には現在レコードが必要」という規則も、すべて一致します。

テスト: `crates/cobolt-runtime/tests/test_indexed_redb.rs`（redb 上でのフィクス
チャ＋`IndexedStore` の直接検査＋`#[ignore]` を付けた大規模スモークテスト）。

---

## 限界

このエンジンはデマンドページングで動くため、実用上の限界を決めるのは redb と
ファイルシステムであって、常駐 RAM ではありません。

| 次元 | 限界 |
|-----------|-------|
| ファイルサイズ | redb ／ファイルシステムの上限（テラバイト級） |
| レコード数 | ワーキングセットの RAM で決まり、件数では決まらない（小さなキャッシュで 2.5 億件以上） |
| レコードサイズ | 固定幅イメージ。大きなレコードは redb の値として格納される |
| キーサイズ | 複合キーのバイト列（多部構成のキーは COBOL 層が対応） |
| 副キー | 最大 65 535（2 バイトの索引名前空間） |

---

## 性能に関する覚え書き

- 参照キーが主キーである**逐次 `READ NEXT`** は、範囲カーソルからそのまま
  レコードを返します — レコードあたり B+ 木の下降は 2 回ではなく 1 回です
  （20 万件でレコードあたり約 17 µs）。副キーによる走査は、依然として副キーの
  下降 1 回に加えて主キーの取得が入ります。
- **`WRITE`** は 1 操作につき `primary`/`alt` テーブルを一度だけ開きます（重複
  チェックと挿入がハンドルを共有）。マイクロベンチマークによれば、呼び出しを
  *またいで*ハンドルをキャッシュしても、1 操作 1 回の方式に対して約 8 % しか
  改善しませんでした。そこでエンジンは、単純で `unsafe` を使わない経路を保って
  います。書き込みコスト（レコードあたり約 44 µs）は redb の ACID な B+ 木挿入が
  支配的で、これが安全側の下限です — 書き込みの最適化はいずれもコミット点や
  耐久性を変えません。
- したがって**一括 `WRITE`** は単一トランザクションで毎秒約 2 万レコードです
  （一度きりのロードコスト）。OPEN・読み出し・クラッシュ安全性は影響を受けません。

---

## 可観測性ログ（`--indexed-log`）

redb エンジンは、ファイルごとの任意のトランザクションログ（既定はオフ）を
**`<assign-path>.log`**（例: `customers.idx` → `customers.idx.log`）に書けます。
`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` ごとに 1 行で、タイムスタンプ、レコード数と
バイト数、スループット、書き込み時のキー順序の質、そして `full` レベルでは
redb の索引ページ統計を記録します。

```bash
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

行の形式は `text`（logfmt）または `json`（NDJSON、Grafana/Loki 対応）です。

**完全なリファレンス** — フラグ、フィールド表、形式、Grafana/Loki のパイプ
ライン（Promtail + LogQL）、コストと安全性の注記 — は
[`observability-jp.md`](observability-jp.md) §1 にあります。

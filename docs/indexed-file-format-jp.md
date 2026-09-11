<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL 索引ファイル形式 (`PRCIDX1`)

この文書は、PowerRustCOBOL で `ORGANIZATION IS INDEXED` ファイルを支えるディスク上
のコンテナーと、それが将来の **Fujitsu COBOL-85 → PowerRustCOBOL インポーター**が
必要とするメタデータにどう対応するかを説明します。

> **Fujitsu とバイナリ互換ではありません。** `PRCIDX1` は PowerRustCOBOL 自身の
> 自己記述コンテナーです。Fujitsu の File Access Subroutines が
> `cobfa_indexinfo()` を通じて公開するメタデータ（レコード形式、レコード長、キーの
> 個数と総長、主キー、代替キー）を*意味的に*模してはいますが、Fujitsu の
> `cobidx`/`cobi64` のバイト列を解析も再現も**しません**。インポーターは将来の課題
> であり、PowerRustCOBOL の外に置かれます。

実装: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs)。

---

## なぜ自己記述形式なのか

もとのコンテナー（`PRCISAM1`）が保持していたのはマジックナンバー、レコード長、レコ
ードのバイト列だけで、**キースキーマは持っていませんでした**。変換ツール（あるいは
任意の外部ツール）は、COBOL の `FD` がなければキーが何なのかを知りようがなかったの
です。

`PRCIDX1` はスキーマ全体をファイルに埋め込みます。レコード形式に加え、各キーのバイ
ト配置、順序、重複の可否、そして（任意で）COBOL の項目名です。これによりファイルは
**発見可能**になり（[`inspect_path`](#発見-api) を参照）、Fujitsu 用インポーターは
対応する `FD` を手元に持たなくても、Fujitsu ファイルから読み取ったメタデータだけで
忠実な PowerRustCOBOL ファイルを書けるようになります。

---

## メタデータモデル

次の Rust 型（`cobolt_runtime` から再エクスポート）がスキーマです。
`cobfa_indexinfo()` の概念を写したもので、オフセットと長さはすべて**バイト単位**で
す（文字数では決してありません — Unicode モードにおける Fujitsu の規則と同じです）。

```rust
pub enum RecordFormat {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

pub enum KeyEncoding {
    Bytes, DisplayAscii, DisplayUtf8,
    Ucs2Le, Ucs2Be, Utf32Le, Utf32Be,
    PackedDecimal, BinaryBigEndian, BinaryLittleEndian,
}

pub enum KeyOrdering { Ascending, Descending }

pub struct KeyPart { pub offset: u32, pub length: u32, pub encoding: KeyEncoding }

pub struct KeyDescriptor {
    pub key_number: u16,          // 1 = primary, 2.. = alternates (declaration order)
    pub name: Option<String>,     // descriptive COBOL field name (optional)
    pub parts: Vec<KeyPart>,      // concatenated → composite key value
    pub duplicates_allowed: bool,
    pub ordering: KeyOrdering,
}

pub struct IndexedFileInfo {
    pub record_format: RecordFormat,
    pub key_count: u16,           // primary + alternates
    pub total_key_length: u32,
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}
```

現在のランタイムが出すのは、**単一パート・`Bytes` エンコード・`Ascending`** のキー
です（COBOL の `FD` の `RECORD KEY` / `ALTERNATE RECORD KEY` はこれに解決されます）。
複合キー、別のエンコーディング、降順は**形式としては表現可能**で、インポーターが
損失なく記録できるようになっています。ランタイム側の完全な対応は将来の課題です。

---

## コンテナーの構成

整数はすべて**リトルエンディアン**です。ファイルは次のとおりです。

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### ヘッダー

| 項目             | 型        | 備考                                    |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | 予約（`0`）                             |
| `record_format`  | `u8`      | `1` = 固定、`2` = 可変                  |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | 固定のときのレコード長                  |
| `min_length`     | `u32`     | 可変のときの最小ペイロード              |
| `max_length`     | `u32`     | 可変のときの最大ペイロード              |
| `key_count`      | `u16`     | 主キー + 代替キー                       |
| `created_unix_ms`| `u64`     | 作成時刻。書き換えをまたいで保持される  |
| `updated_unix_ms`| `u64`     | 最終書き込み時刻                        |

### キースキーマ — `key_count` 回繰り返す（主キーが先頭）

| 項目           | 型        | 備考                                    |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` が主キー、`2..` が代替キー          |
| `duplicates`   | `u8`      | `0`/`1`                                 |
| `ordering`     | `u8`      | `0` 昇順、`1` 降順                      |
| `part_count`   | `u16`     | `KeyPart` の個数                        |
| `name_len`     | `u16`     | UTF-8 の名前の長さ（`0` = 名前なし）    |
| `name`         | `[u8]`    | `name_len` バイト                       |
| `parts`        | 繰り返し  | `part_count` × KeyPart（下記）          |

各 **KeyPart**:

| 項目       | 型    | 備考                                    |
|------------|-------|-----------------------------------------|
| `offset`   | `u32` | レコードペイロード内のバイトオフセット  |
| `length`   | `u32` | バイト長                                |
| `encoding` | `u8`  | `KeyEncoding` の判別子                  |
| `reserved` | `u8`  | `0`                                     |

### レコード

| 項目           | 型     | 備考                                    |
|----------------|--------|-----------------------------------------|
| `record_count` | `u64`  | 生存しているレコードの数                |
| レコードごと   | 繰り返し | `length: u32` に続けて `length` バイト |

レコードは**主キー**の昇順で書かれます。

### トレーラー

| 項目    | 型    | 備考                                             |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | トレーラーより前のすべてのバイトに対する CRC-32（IEEE 802.3、反転） |

CRC は読み込み時に検証されます。不一致は FILE STATUS `90`（入出力エラー）になります。

---

## 発見 API

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

`PRCIDX1` ファイルなら `Some(IndexedFileInfo)` を、（スキーマを持たない）旧来の
`PRCISAM1` コンテナーなら `None` を返します。これが、変換ツールや検査ツールから呼べ
る `cobfa_indexinfo()` 相当です。

---

## オープン時の検証（FILE STATUS）

**既存の**索引ファイルを `INPUT` / `I-O` で開くとき、ランタイムは `SELECT`/`FD` で
宣言されたキーとレコード形式を、保存されているスキーマと突き合わせて検証します
（厳密モード、既定で有効）。関係する状態は次のとおりです。

| 状態   | 条件                                                  |
|-------:|-------------------------------------------------------|
| `35`   | 存在しないファイルの `OPEN INPUT`                     |
| `39`   | 既存ファイルのスキーマが、宣言されたキーやレコード形式と一致しない |
| `90`   | コンテナーの破損（CRC 不一致）その他の入出力エラー    |

旧来の `PRCISAM1` コンテナーにはスキーマがないため、厳密な検証は省かれます（常に
寛容に読み込まれます）。

---

## ストレージモード (`STORAGE IS MEMORY | DISK`)

`STORAGE MODE` 句は、INDEXED ファイルをどのエンジンが — したがってディスク上のどの
コンテナーが — 支えるかを選びます。**既定のストレージモードは `DISK`** です
（`STORAGE` 句が無い場合）。`WITH COMPRESSION` はどちらのモードにも適用できます。
`WITH PERSISTENCE` は `MEMORY` にのみ適用されます。

| モード | エンジン | コンテナー | 備考 |
|------|--------|-----------|-------|
| `MEMORY` | RAM 上の `BTreeMap`（`indexed.rs`） | `PRCIDX1`（この文書） | ファイル全体をメモリに載せる。**既定では揮発性**で、`COMMIT` はディスクに書きません。`WITH PERSISTENCE` を付けた場合のみ、`CLOSE` 時に `PRCIDX1` として保存されます。`OPEN OUTPUT` は常にコンテナーを（再）作成します。 |
| `DISK`（既定） | 1.62.73 以降はクラッシュに強い redb ストア（`indexed_redb.rs`）。`--indexed-engine rust` ではページ方式の B+tree（`indexed_disk.rs`） | redb 自身のもの、またはページ方式エンジンなら `PRCIDXD1` | レコードと索引は必要に応じて読む。RAM は有界。常に永続的（操作ごとに書き込み、`COMMIT`/`CLOSE` で `fsync`） |

**`PRCIDXD1`** ディスクコンテナーは、単一のページ方式ファイル（4 KiB ページ）です。

* **ページ 0** — ヘッダー: 各キーの B+tree のルート、フリーリストの先頭、次のページ
  番号、`RecordId` のカウンター、レコード数、キースキーマ、圧縮フラグ。
* **B+tree ページ** — 内部ノードと葉ノード（可変長のバイトパッキング、挿入時に分割、
  順序走査のため葉は双方向リンク）。
* **データページ** — スロット方式のレコードセル（1 ページに複数レコード）、および
  1 ページに収まらないレコードのためのオーバーフローページの連鎖。
* **ディレクトリページ** — `RecordId` → 物理位置の対応表。
* **フリーリスト**が、解放済みのページを再利用のために繋いでいます。

`WITH COMPRESSION`（`compress.rs`）は依存関係を持たない PackBits 風の RLE で、格納
される各レコード（`PRCIDXD1`）、またはレコード区画内の各レコード（`PRCIDX1`）に適用
されます。1 バイトのタグにより符号化後に大きくなることは決してなく、コンテナーの
ヘッダーに圧縮が有効であることが記録されます。

> `PRCIDXD1` はネイティブな DISK モードの格納用です。上で述べた、発見可能で Fujitsu
> からのインポートを念頭に置いたメタデータは `PRCIDX1`（MEMORY モード）コンテナーの
> ものです。インポーターは、ページ方式のディスク配置が特に必要でない限り `PRCIDX1`
> を対象にすべきです。

## 後方互換性

* `PRCIDX1`（マジック `PRCIDX1\0`）— 現行の自己記述 MEMORY モード形式（読み書き）。
* `PRCIDXD1`（マジック `PRCIDXD1`）— DISK モードのページ方式 B+tree コンテナー。
* `PRCISAM1`（マジック `PRCISAM1`）— レコードのみの旧コンテナー（読み取り専用。書き
  込み可能なオープンの次の `CLOSE` で `PRCIDX1` として保存し直されます）。
* それ以外の内容 — 空のファイルとして扱われます。

---

## 将来の Fujitsu インポート経路

想定している移行の流れです（今日の時点ではすべて PowerRustCOBOL の範囲外）。

```text
Fujitsu runtime
  └─ cobfa_indexinfo()  → record format, record length, key list (primary + alternates)
  └─ sequential export  → record payloads
        │
        ▼
  converter (future, external)
        │  builds IndexedFileInfo + records
        ▼
  PRCIDX1 file  → opened natively by PowerRustCOBOL
```

`PRCIDX1` はすでに複合キー、キーのエンコーディング、キーの順序、重複の可否、可変長
レコードの上下限、キー項目名を*表現*できるため、変換ツールがすべきことは Fujitsu の
メタデータを `IndexedFileInfo` に翻訳し、レコードを流し込むことだけです。
PowerRustCOBOL 側の形式変更は要りません。

Fujitsu の `cobidx`/`cobi64` の生のバイト列を解析しようと**しないでください**。
Fujitsu の公開資料は File Access Subroutines を通じてメタデータを公開していますが、
物理的なバイト配置は公表していません。

.<<

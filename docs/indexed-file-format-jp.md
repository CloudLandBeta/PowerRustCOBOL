<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL インデックスファイル形式（`PRCIDX1`）

本書は、PowerRustCOBOL で `ORGANIZATION IS INDEXED` ファイルを支えるディスク上
のコンテナと、それが将来の **Fujitsu COBOL-85 → PowerRustCOBOL インポーター**
に必要となるメタデータへどう対応するかを説明します。

> **Fujitsu とバイナリ互換ではありません。** `PRCIDX1` は PowerRustCOBOL 独自
> の自己記述型コンテナです。Fujitsu の File Access Subroutines が
> `cobfa_indexinfo()` を通じて公開するメタデータ（レコード形式、レコード長、
> キー数と合計長、主キー、代替キー）を*意味的に*モデル化していますが、Fujitsu
> の `cobidx`/`cobi64` のバイト列を解析したり再現したりすることは**ありません**。
> インポーターは将来の課題であり、PowerRustCOBOL の外部に置かれます。

実装: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs)。

---

## 形式が自己記述的である理由

元のコンテナ（`PRCISAM1`）が保持していたのは、マジックナンバー、レコード長、
レコードのバイト列だけで、**キースキーマは一切持っていませんでした**。
コンバーター（あるいは任意の外部ツール）は、COBOL の `FD` なしにはキーが何で
あるかを知ることができませんでした。

`PRCIDX1` はスキーマ全体をファイルに埋め込みます。レコード形式に加えて、各キー
のバイト配置、並び順、重複ポリシー、そして（任意で）COBOL のフィールド名です。
これによりファイルは**探索可能**になり（[`inspect_path`](#探索-api) を参照）、
Fujitsu のインポーターは、対応する `FD` を手元に用意しなくても、Fujitsu ファイル
から読み取ったメタデータだけで忠実な PowerRustCOBOL ファイルを書き出せます。

---

## メタデータモデル

これらの Rust 型（`cobolt_runtime` から再エクスポート）がスキーマです。
`cobfa_indexinfo()` の概念を反映しており、オフセットと長さはすべて**バイト単位**
です（文字数ではありません。Fujitsu の Unicode モードの規則に合わせています）。

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

現在のランタイムが出力するのは、**単一パート・`Bytes` エンコード・`Ascending`**
のキーです（COBOL の `FD` の `RECORD KEY` / `ALTERNATE RECORD KEY` はこれに解決
されます）。複合キー、他のエンコード、降順は**形式として表現可能**であり、
インポーターは情報を失わずに記録できます。ランタイム側の完全な対応は将来の課題
です。

---

## コンテナのレイアウト

整数はすべて**リトルエンディアン**です。ファイルの構成は次のとおりです。

```text
┌─────────────────────────────────────────────────────────────┐
│ ヘッダー                                                    │
│ キースキーマ（key_count 個の記述子: 主キー、次に代替キー）  │
│ レコード                                                    │
│ CRC-32 トレーラー（直前までの全バイトが対象）               │
└─────────────────────────────────────────────────────────────┘
```

### ヘッダー

| 項目             | 型        | 備考                                    |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | 予約（`0`）                             |
| `record_format`  | `u8`      | `1` = 固定、`2` = 可変                  |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | 固定長の場合のレコード長                |
| `min_length`     | `u32`     | 可変長の場合の最小ペイロード            |
| `max_length`     | `u32`     | 可変長の場合の最大ペイロード            |
| `key_count`      | `u16`     | 主キー + 代替キー                       |
| `created_unix_ms`| `u64`     | 作成時刻。書き換えをまたいで保持される  |
| `updated_unix_ms`| `u64`     | 最終書き込み時刻                        |

### キースキーマ — `key_count` 回の繰り返し（主キーが先頭）

| 項目           | 型        | 備考                                    |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` は主キー、`2..` は代替キー          |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` は昇順、`1` は降順                  |
| `part_count`   | `u16`     | `KeyPart` の個数                        |
| `name_len`     | `u16`     | UTF-8 名の長さ（`0` = なし）            |
| `name`         | `[u8]`    | `name_len` バイト                       |
| `parts`        | 繰り返し  | `part_count` × KeyPart（下記）          |

各 **KeyPart**:

| 項目       | 型    | 備考                           |
|------------|-------|--------------------------------|
| `offset`   | `u32` | レコードペイロード内のバイトオフセット|
| `length`   | `u32` | バイト長                       |
| `encoding` | `u8`  | `KeyEncoding` の判別子         |
| `reserved` | `u8`  | `0`                            |

### レコード

| 項目           | 型       | 備考                                 |
|----------------|----------|--------------------------------------|
| `record_count` | `u64`    | 有効なレコードの件数                 |
| レコードごと   | 繰り返し | `length: u32` に続いて `length` バイト|

レコードは**主キー**の昇順で書き出されます。

### トレーラー

| 項目    | 型    | 備考                                             |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | トレーラーより前の全バイトに対する CRC-32（IEEE 802.3、反転）|

CRC は読み込み時に検証されます。不一致の場合は FILE STATUS `90`（入出力エラー）
になります。

---

## 探索 API

```rust
use cobolt_runtime::IndexedFile; // (engine type)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

`PRCIDX1` ファイルであれば `Some(IndexedFileInfo)` を、（スキーマを持たない）
旧来の `PRCISAM1` コンテナであれば `None` を返します。これは、コンバーターや
検査ツールが呼び出せる `cobfa_indexinfo()` 相当の API です。

---

## オープン時の検証（FILE STATUS）

**既存の**インデックスファイルを `INPUT` / `I-O` で開くとき、ランタイムは宣言
された `SELECT`/`FD` のキーとレコード形式を、保存されているスキーマと照合して
検証します（厳格モード。既定で有効）。関連するステータスは次のとおりです。

| ステータス | 条件                                               |
|-------:|-------------------------------------------------------|
| `35`   | 存在しないファイルの `OPEN INPUT`                     |
| `39`   | 既存ファイルのスキーマ ≠ 宣言されたキー／レコード形式 |
| `90`   | コンテナの破損（CRC 不一致）またはその他の入出力エラー|

旧来の `PRCISAM1` コンテナはスキーマを持たないため、厳格な検証は省略されます
（常に寛容に読み込まれます）。

---

## ストレージモード（`STORAGE IS MEMORY | DISK`）

`STORAGE MODE` 句は、INDEXED ファイルをどのエンジン、ひいてはどのディスク上の
コンテナが支えるかを選択します。**既定のストレージモードは `DISK`** です
（`STORAGE` 句がない場合）。`WITH COMPRESSION` はどちらのモードにも適用され、
`WITH PERSISTENCE` は `MEMORY` にのみ適用されます。

| モード | エンジン | コンテナ | 備考 |
|--------|----------|----------|------|
| `MEMORY` | RAM 上の `BTreeMap`（`indexed.rs`） | `PRCIDX1`（本書） | ファイル全体をメモリに保持。**既定では揮発的**で、`COMMIT` はディスクに書き込みません。`WITH PERSISTENCE` を指定した場合のみ、`CLOSE` 時に `PRCIDX1` へ保存されます。`OPEN OUTPUT` は常にコンテナを（再）作成します。 |
| `DISK`（既定） | 永続的なページ方式 B+ 木（`indexed_disk.rs`） | `PRCIDXD1` | レコードとインデックスをオンデマンドで読み込み。RAM 使用量は有界。常に永続的（操作ごとに書き込み、`COMMIT`/`CLOSE` で `fsync`） |

ディスクコンテナ **`PRCIDXD1`** は、単一のページ方式ファイルです（4 KiB
ページ）。

* **ページ 0** — ヘッダー: 各ルート（キーごとに 1 本の B+ 木）、フリーリストの
  先頭、次のページ ID、`RecordId` カウンター、レコード件数、キースキーマ、
  圧縮フラグ。
* **B+ 木ページ** — 内部ノードとリーフノード（可変長でバイト詰めされ、挿入時に
  分割。リーフは順序走査のために双方向にリンクされています）。
* **データページ** — スロット方式のレコードセル（1 ページに複数のレコード）と、
  1 ページより大きなレコード用のオーバーフローページのチェーン。
* **ディレクトリページ** — `RecordId` → 物理位置のマップ。
* **フリーリスト**が、解放済みのページを再利用のためにつなぎます。

`WITH COMPRESSION`（`compress.rs`）は依存関係のない PackBits 方式の RLE で、
保存される各レコード（`PRCIDXD1`）、またはレコードセクション内の各レコード
（`PRCIDX1`）に適用されます。1 バイトのタグによってエンコードでサイズが増えない
ことが保証され、圧縮が有効であることはコンテナのヘッダーに記録されます。

> `PRCIDXD1` は DISK モードのネイティブなストレージ用です。上で述べた探索可能で
> Fujitsu インポートを見据えたメタデータは `PRCIDX1`（MEMORY モード）コンテナの
> ものです。インポーターは、ページ方式のディスクレイアウトが特に必要でない限り
> `PRCIDX1` を対象にすべきです。

## 後方互換性

* `PRCIDX1`（マジックナンバー `PRCIDX1\0`）— 現行の自己記述型 MEMORY モード形式
  （読み書き対応）。
* `PRCIDXD1`（マジックナンバー `PRCIDXD1`）— DISK モードのページ方式 B+ 木
  コンテナ。
* `PRCISAM1`（マジックナンバー `PRCISAM1`）— レコードのみを保持する旧来の
  コンテナ（読み取り専用。書き込み可能で開いた次の `CLOSE` 時に `PRCIDX1` として
  保存し直されます）。
* それ以外の内容 — 空のファイルとして扱われます。

---

## 将来の Fujitsu インポート経路

想定している移行フローです（現時点ではすべて PowerRustCOBOL の範囲外です）。

```text
Fujitsu ランタイム
  └─ cobfa_indexinfo()  → レコード形式、レコード長、キー一覧（主キー + 代替キー）
  └─ 順次エクスポート   → レコードのペイロード
        │
        ▼
  コンバーター（将来、外部）
        │  IndexedFileInfo + レコードを構築
        ▼
  PRCIDX1 ファイル  → PowerRustCOBOL がネイティブに開く
```

`PRCIDX1` は複合キー、キーのエンコード、キーの並び順、重複ポリシー、可変長
レコードの範囲、キーフィールド名をすでに*表現*できるため、コンバーターは
Fujitsu のメタデータを `IndexedFileInfo` へ変換し、レコードを流し込むだけで
済みます。PowerRustCOBOL 側の形式変更は不要です。

Fujitsu の生の `cobidx`/`cobi64` バイト列を解析しようとしては**いけません**。
Fujitsu の公開ドキュメントは File Access Subroutines を通じてメタデータを公開
していますが、物理的なバイト配置は公表していません。

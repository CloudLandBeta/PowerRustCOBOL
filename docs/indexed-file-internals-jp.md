<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.134 -->

# PowerRustCOBOL 索引ファイルの内部（`PRCIDXD1` ページ方式エンジン）

この文書は、`STORAGE IS DISK`（既定）で宣言された `ORGANIZATION IS INDEXED` ファイル
を支える、**永続的でページ方式のディスク上**エンジンの概念スキーマです。B+tree と
スロット方式ページによる設計で、レコードを必要に応じて読むため、ファイルの大きさに
関わらず RAM の使用量は有界に保たれます。

> ⚠️ **これはもう既定のエンジンではありません。** `STORAGE IS DISK` は今も既定の
> *ストレージモード*ですが、**1.62.73** 以降それを担うエンジンは **redb** です
> （`crates/cobolt-runtime/src/indexed.rs:126`）。
> [`indexed-redb-engine-jp.md`](indexed-redb-engine-jp.md) を参照してください。以下の
> 内容はページ方式エンジンについては今も正確で、そのエンジンは
> `--indexed-engine rust` で今も使えます。単に、プログラムが既定で得るものではなく
> なったというだけです。
>
> **範囲。** ここで述べるのは*物理エンジン*（`DiskIndexedFile`、コンテナーのマジック
> は `PRCIDXD1`）です。これは
> [`indexed-file-format-jp.md`](indexed-file-format-jp.md) に記された、単一ブロックで
> 自己記述的な `PRCIDX1` コンテナーとは別の成果物であり、あちらは将来の Fujitsu
> インポーターが必要とするメタデータを模したものです。メモリ上のエンジン
> （`STORAGE IS MEMORY`、`IndexedFile`）は、同じ論理モデルを簡略化した部分集合です
> （ディスク上の B+tree の代わりに BTreeMap を使います）。
>
> 2 つめの、**クラッシュに強い** `STORAGE IS DISK` エンジン（任意。純 Rust の redb
> ACID ストア上）は、このエンジンの RAM に縛られたディレクトリと CLOSE 時にしか永続
> 化しない点を解決します。
> [`indexed-redb-engine-jp.md`](indexed-redb-engine-jp.md) を参照してください。

実装:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs)、
レコードの（脱）具体化は
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs)。

---

## 1. 設計を一文で

**1 枚のヘッダーページ + N 本の B+tree（キーごとに 1 本）→ RecordId ディレクトリ →
位置指定で固定幅のレコードイメージを収めたスロット方式のデータページ**からなるページ
方式のファイル。これにフリーリスト、オーバーフローの連鎖、任意の RLE 圧縮、そして
トランザクションのための実行中の取り消しログが加わります。

---

## 2. ファイルは 4 KiB 固定ページの配列

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

ページ 0 より**後ろ**のページはすべて、先頭バイト（ページ種別タグ）で自分が何かを
名乗ります。解放されたページはフリーリストを通じて再利用されるので、ディスク上の物理
的なページ順序はレコードの論理的な順序とは**一致しません**。

| タグ | 定数          | ページが保持するもの                          |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | B+tree の内部（経路選択）ノード               |
| `2` | `PT_LEAF`     | B+tree の葉ノード（隣接ノードと双方向リンク） |
| `3` | `PT_DATA`     | 複数のレコードイメージを詰めたスロット方式ページ |
| `4` | `PT_OVERFLOW` | インラインに収まらない大きなレコードの続き    |
| `5` | `PT_DIR`      | RecordId ディレクトリの一部                   |

---

## 3. ページ 0 — ヘッダー

ページ 0 は*スキーマ*が保存される唯一の場所で、一度だけ書かれます。項目はリトル
エンディアンで、次の順に並びます。

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (each u64)
 primary_root   dir_head         directory_len                 (each u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (one B+tree root per alt key)
 ──────────────────────────────────────────────────────────────────────
 KEY SCHEMA:  key_count (u16) → for each key (primary first, then alternates):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × parts   (composite-key parts)
```

| ヘッダー項目      | 意味                                                          |
|-------------------|---------------------------------------------------------------|
| `version`         | 形式のバージョン（現在は `1`）。                              |
| `page_size`       | ページサイズ（バイト、4096）。                                |
| `rec_fmt`         | レコード形式: `1` = 固定長。                                  |
| `compressing`     | ディスク上でレコードのペイロードを RLE 圧縮しているなら `1`。 |
| `record_len`      | 論理的な（非圧縮の）レコード長（バイト）。                    |
| `next_page_id`    | フリーリストが空のときに次に割り当てるページ番号。            |
| `free_list_head`  | 回収済みページのフリーリストの先頭ページ（`0` = なし）。      |
| `record_count`    | 生存しているレコードの数。                                    |
| `data_tail`       | インライン書き込みを受け付けている現在の `PT_DATA` ページ（`0` = なし）。 |
| `primary_root`    | 主キーの B+tree のルートページ。                              |
| `dir_head`        | RecordId ディレクトリの最初の `PT_DIR` ページ（`0` = なし）。 |
| `directory_len`   | ディレクトリのエントリー数（これまでに割り当てた RecordId）。 |
| `alt_root[k]`     | 代替キー *k* の B+tree のルートページ。                       |
| キースキーマ      | キーごとの重複の可否と、複合キー各部のバイト範囲。            |

**意図的にヘッダーに*入っていない*もの:** **データ項目の名前**も**レコードごとの
メタデータ**もありません。スキーマは純粋に*キーの幾何*（バイト範囲）だけです。レコード
に関するそれ以外はすべて位置で決まります — §6 を参照してください。

---

## 4. アクセス経路（キー指定の `READ` がどう解決されるか）

```
  COBOL key value (bytes)
        │
        ▼
  ┌──────────────┐   Start at primary_root (random READ by RECORD KEY) or
  │  B+tree      │   alt_roots[k] (READ KEY IS <alt>). Internal nodes route by
  │  (one per    │   key; leaves hold (key_bytes → RecordId) and are doubly
  │  key)        │   linked (next/prev) for READ NEXT / READ PREVIOUS / START.
  └──────┬───────┘
         │  RecordId (a stable integer, independent of physical location)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = free/tombstone, 1 = inline, 2 = overflow head
  │  directory   │     len : stored (possibly compressed) byte length
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Slotted DATA page → slot directory → (offset, len) →
  │  DATA page   │   raw record image (decompressed if `compressing`).
  └──────┬───────┘
         ▼
  the fixed-width record bytes
        │  RecordLayout.distribute()
        ▼
  scattered into the FD's elementary items in working memory
```

**1 つのレコードに多くのキー。** 主キーもすべての代替キーも*同じ* RecordId を指すの
で、各レコードの保存された実体はちょうど 1 つです。代替索引は、共有された RecordId
ディレクトリの上に重ねられた追加の B+tree にすぎません。そのキーが
`WITH DUPLICATES` と宣言されていれば、代替値の重複が許されます。

---

## 5. ページの内部

### 5.1 B+tree のノード（`PT_INTERNAL` / `PT_LEAF`）

ノードは操作のためにメモリへ読み込まれ、変更され、必要なら分割され、書き戻されます。

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- 葉は**双方向にリンク**されており（`next`/`prev`）、`START` の後の順序走査は隣接
  ノードを直接たどります。これが RustCOBOL の昇順キーによる `READ NEXT` です。
- 直列化したノードが `PAGE_SIZE` を超えそうなとき、挿入は**あふれたら分割**します。
  中央のキーが親へ昇格します。
- 内部ノードは `child0` と*（区切りキー, 子）*の組を持ちます。

### 5.2 スロット方式のデータページ（`PT_DATA`）

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- 5 バイトのページヘッダーに続いて、前方から伸びる**スロットディレクトリ**があり、
  **レコードのペイロード**は後方から伸びます。2 つの領域がぶつかるまでは、レコードは
  インラインに収まります。
- スロットは `(offset, len)` です。レコードを削除すると、そのスロットは `len = 0`
  になります（墓標）。ページ内のすべてのスロットが空きになると、そのページ全体が
  フリーリストへ戻されます。
- `RecLoc` の `slot` 項目は、このスロットディレクトリの添字です。

### 5.3 オーバーフローの連鎖（`PT_OVERFLOW`）

インラインの上限（`PAGE_SIZE − ヘッダー − スロット 1 つ`）より大きいレコードは、
オーバーフローページの連結リストとして保存されます。その `RecLoc.kind = 2` で、
`page` は連鎖の先頭を指します。

### 5.4 RecordId ディレクトリ（`PT_DIR`）

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

ファイルが開いている間、ディレクトリは `Vec<RecLoc>` として RAM に保持されます
（したがって RecordId の参照は O(1) の添字アクセスです）。閉じるときに、`dir_head`
から始まる `PT_DIR` ページの連鎖として永続化されます。B+tree が格納するのは RecordId
であって物理アドレスではないので、索引に一切触れずにレコードをディスク上で移動できま
す。

---

## 6. レコードイメージそのもの（位置で決まり、名前は持たない）

ディスク上のレコードは、フィールドの*オフセット*で並んだ単一の**固定幅バイト
バッファー**です。ペイロードの中にフィールド名もタグも区切りもありません。次の例では:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

保存されるイメージは 23 バイトです。

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- `RecordLayout::materialize()` は `WRITE`/`REWRITE` のために、`FD` の基本項目を
  オフセットに従ってこのバッファーへ詰めます。`RecordLayout::distribute()` は `READ`
  でその逆を行います。フィールド → オフセットの対応表は、プログラムの
  `RecordLayout`（`FD` から導かれる）の中にしか存在せず、ファイルの中には**決して**
  ありません。
- **同一性とは位置のことです。** これは「レコードごとにキーを繰り返さない」という
  考えの極限です。フィールドの同一性はレコードあたり*ゼロ*バイトで済み、フィールドへ
  のアクセスは事前計算したオフセットによる O(1)（解析不要）です。キーでないフィールド
  の名前を変えてもディスク上は何も変わりません。キーのフィールド名を変えても、書き
  換わるのはヘッダーのキースキーマだけで、レコードも索引も変わりません。フィールドの
  オフセットや幅を変えることだけが、データの書き直しを要する変更です。これは固定長
  レコードに（そして本物の ISAM/VSAM に）本質的なことです。

### 圧縮

`STORAGE IS DISK WITH COMPRESSION` を指定すると、**保存される**ペイロードは PackBits
方式の RLE で圧縮され（`compress.rs`）、`RecLoc.len` は*保存された*長さになります。
読み出し時にバッファーは `record_len` へ戻されます。圧縮はキーの幾何にもアクセス経路
にも透過です。

---

## 7. 空き領域と再利用

- **フリーリスト。** `free_list_head` は、空になったデータページや分割で孤立したノー
  ドなどから回収したページを連ねます。`allocate` は `next_page_id` を増やす前にここ
  から取り出すので、領域は再利用され、ファイルは単調には大きくなりません。
- **墓標。** `DELETE` はスロットを（そして遅延してデータページを）解放し、ディレクト
  リのエントリーを `RecLoc::FREE` と印します。その RecordId は退役します。

---

## 8. トランザクション（実行中の取り消しログ）

ディスクエンジンは、直前の `COMMIT`/`OPEN` 以降のすべての変更について、その逆操作を
並べた**取り消しログ**を保持します。

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- `OPEN` はトランザクションを開始します（ログを消去します）。`COMMIT` は変更を耐久化
  して新しいトランザクションを始めます。`ROLLBACK` は逆操作を逆順に再生します。
  `CLOSE` はフラッシュします（暗黙のコミット）。`tx_replay` のガードが、逆操作自身が
  再びログに積まれるのを防ぎます。
- これは**プログラムレベル**のロールバックです。耐久性のある先行書き込みログによる
  クラッシュ復旧は将来の課題です。言語リファレンスの COBOL の `COMMIT`/`ROLLBACK`
  動詞を参照してください。これらの動詞は **INDEXED ファイル**に作用するのであって、
  SQL 接続には作用しません。

---

## 9. OPEN 時の検証

`OPEN` では、ヘッダーに保存されたキースキーマがプログラムの `SELECT`（レコード長、
キーの数、各キーの構成部分と重複の可否）と突き合わされます。食い違えば COBOL の
ファイル状態 `39` を返します。存在しないファイルを `INPUT` で開けば `35`、壊れている
か短すぎるヘッダーなら `90` です。（厳密な検証は、エンジンの `strict_metadata` フラグ
で緩められます。）

---

## 10. 早見表 — 何がどこに保存されるか

| もの                          | どこにあるか                           | 数          |
|-------------------------------|----------------------------------------|-------------|
| キーの幾何（オフセットと幅）  | ヘッダー（ページ 0）のキースキーマ     | 1 つ        |
| データ項目の名前              | プログラムの `FD` だけ                 | ファイルには無い |
| レコードのバイト列            | `PT_DATA` / `PT_OVERFLOW` ページ        | レコードごとに 1 つ |
| キー → RecordId               | キーごとに 1 本の B+tree               | キーごとに 1 つ |
| RecordId → 物理位置           | RecordId ディレクトリ（`PT_DIR` の連鎖） | レコードごとに 1 つ |
| 空きページ                    | フリーリスト（`free_list_head`）       | —           |
| 未コミットの変更の逆操作      | RAM 上の取り消しログ                   | トランザクションごと |

.<<

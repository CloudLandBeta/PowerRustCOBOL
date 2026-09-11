<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL 索引文件格式（`PRCIDX1`）

本文档描述在 PowerRustCOBOL 中支撑 `ORGANIZATION IS INDEXED` 文件的磁盘容器，以及
它如何对应到未来的 **Fujitsu COBOL-85 → PowerRustCOBOL 导入器**所需的元数据。

> **与 Fujitsu 并非二进制兼容。** `PRCIDX1` 是 PowerRustCOBOL 自己的自描述容器。它
> 在*语义上*仿照了 Fujitsu 的 File Access Subroutines 通过 `cobfa_indexinfo()`
> 暴露的元数据（记录格式、记录长度、键的数量与总长度、主键、备用键），但它**不**解析
> 也不复现 Fujitsu 的 `cobidx`/`cobi64` 字节。导入器属于将来的工作，存在于
> PowerRustCOBOL 之外。

实现：[`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs)。

---

## 为什么这个格式是自描述的

最初的容器（`PRCISAM1`）只保存了一个魔数、记录长度和记录字节——它**不带键模式**。
转换器（或任何外部工具）在没有 COBOL `FD` 的情况下无从得知键是什么。

`PRCIDX1` 把完整的模式嵌进文件里：记录格式，以及每个键的字节布局、排序、重复策略，
还有（可选的）它的 COBOL 字段名。这让文件变得**可发现**——参见
[`inspect_path`](#发现-api)——并让 Fujitsu 导入器只凭从 Fujitsu 文件里读到的元数据，
就能写出一个忠实的 PowerRustCOBOL 文件，手边并不需要一份对应的 `FD`。

---

## 元数据模型

下面这些 Rust 类型（从 `cobolt_runtime` 重新导出）就是模式。它们映照
`cobfa_indexinfo()` 的概念；所有偏移量和长度都是**以字节为单位**的（绝不是字符
计数——与 Fujitsu 在 Unicode 模式下的规则一致）。

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

当前的运行时产出的是**单段、`Bytes` 编码、`Ascending`** 的键（这正是 COBOL `FD` 的
`RECORD KEY` / `ALTERNATE RECORD KEY` 所解析出来的结果）。复合键、其他编码方式和降序
在**格式上是可以表达的**，以便导入器无损地记录它们；运行时对它们的完整支持属于将来
的工作。

---

## 容器布局

所有整数都是**小端序**。文件的构成是：

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### 头部

| 字段             | 类型      | 说明                                    |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | 保留（`0`）                             |
| `record_format`  | `u8`      | `1` = 定长，`2` = 变长                  |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | 定长时的记录长度                        |
| `min_length`     | `u32`     | 变长时的最小负载                        |
| `max_length`     | `u32`     | 变长时的最大负载                        |
| `key_count`      | `u16`     | 主键 + 备用键                           |
| `created_unix_ms`| `u64`     | 创建时间，重写时予以保留                |
| `updated_unix_ms`| `u64`     | 最后写入时间                            |

### 键模式 —— 重复 `key_count` 次（主键在先）

| 字段           | 类型      | 说明                                    |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` 为主键，`2..` 为备用键              |
| `duplicates`   | `u8`      | `0`/`1`                                 |
| `ordering`     | `u8`      | `0` 升序，`1` 降序                      |
| `part_count`   | `u16`     | `KeyPart` 的个数                        |
| `name_len`     | `u16`     | UTF-8 名称的长度（`0` = 没有）          |
| `name`         | `[u8]`    | `name_len` 个字节                       |
| `parts`        | 重复      | `part_count` × KeyPart（见下）          |

每个 **KeyPart**：

| 字段       | 类型  | 说明                                    |
|------------|-------|-----------------------------------------|
| `offset`   | `u32` | 在记录负载中的字节偏移                  |
| `length`   | `u32` | 字节长度                                |
| `encoding` | `u8`  | `KeyEncoding` 的判别值                  |
| `reserved` | `u8`  | `0`                                     |

### 记录

| 字段           | 类型   | 说明                                    |
|----------------|--------|-----------------------------------------|
| `record_count` | `u64`  | 存活记录的数量                          |
| 每条记录       | 重复   | 先是 `length: u32`，随后是 `length` 个字节 |

记录按**主键**升序写入。

### 尾部

| 字段    | 类型  | 说明                                             |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | 对尾部之前所有字节计算的 CRC-32（IEEE 802.3，反射） |

CRC 在加载时校验；不匹配会给出 FILE STATUS `90`（I/O 错误）。

---

## 发现 API

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

对 `PRCIDX1` 文件返回 `Some(IndexedFileInfo)`，对（不带模式的）旧式 `PRCISAM1` 容器
返回 `None`。这就是转换器或检查工具可以调用的 `cobfa_indexinfo()` 对应物。

---

## 打开时的校验（FILE STATUS）

以 `INPUT` / `I-O` 方式打开一个**已存在**的索引文件时，运行时会把 `SELECT`/`FD` 中
声明的键和记录格式与已存储的模式做校验（严格模式，默认开启）。相关状态：

| 状态   | 条件                                                  |
|-------:|-------------------------------------------------------|
| `35`   | 对不存在的文件执行 `OPEN INPUT`                       |
| `39`   | 已存在文件的模式 ≠ 声明的键或记录格式                 |
| `90`   | 容器损坏（CRC 不匹配）或其他 I/O 错误                 |

旧式的 `PRCISAM1` 容器没有模式，因此对它会跳过严格校验（它总是以宽松方式加载）。

---

## 存储模式（`STORAGE IS MEMORY | DISK`）

`STORAGE MODE` 子句决定由哪个引擎——从而由哪个磁盘容器——支撑一个 INDEXED 文件。
**默认的存储模式是 `DISK`**（在没有 `STORAGE` 子句时）。`WITH COMPRESSION` 对两种
模式都适用；`WITH PERSISTENCE` 只适用于 `MEMORY`。

| 模式 | 引擎 | 容器 | 说明 |
|------|--------|-----------|-------|
| `MEMORY` | 内存中的 `BTreeMap`（`indexed.rs`） | `PRCIDX1`（本文档） | 整个文件在内存里；**默认是易失的**——`COMMIT` 从不写盘。加上 `WITH PERSISTENCE` 时，只在 `CLOSE` 时保存为 `PRCIDX1`。`OPEN OUTPUT` 总是（重新）创建容器。 |
| `DISK`（默认） | 自 1.62.73 起为抗崩溃的 redb 存储（`indexed_redb.rs`）；用 `--indexed-engine rust` 时为分页 B+tree（`indexed_disk.rs`） | redb 自己的容器，或分页引擎用的 `PRCIDXD1` | 记录与索引按需读取；内存占用有界；始终持久（逐操作写入，`COMMIT`/`CLOSE` 时 `fsync`） |

**`PRCIDXD1`** 磁盘容器是单个分页文件（4 KiB 一页）：

* **第 0 页** —— 头部：各个根（每个键一棵 B+tree）、空闲链表头、下一个页号、
  `RecordId` 计数器、记录数、键模式，以及压缩标志。
* **B+tree 页** —— 内部节点与叶节点（变长字节打包，插入时分裂，叶节点双向相连以便
  有序扫描）。
* **数据页** —— 带槽位的记录单元（一页多条记录），外加供大于一页的记录使用的溢出页
  链。
* **目录页** —— `RecordId` → 物理位置的映射表。
* 一个**空闲链表**把释放的页串起来以便重用。

`WITH COMPRESSION`（`compress.rs`）是一种无依赖的 PackBits 式 RLE，作用于每条被存储
的记录（`PRCIDXD1`），或记录区中的每条记录（`PRCIDX1`）；一个单字节标签保证编码后
绝不会变大，容器头部则记录压缩是否开启。

> `PRCIDXD1` 用于原生的 DISK 模式存储。上面那些可发现的、面向 Fujitsu 导入的元数据
> 属于 `PRCIDX1`（MEMORY 模式）容器；除非确实需要分页的磁盘布局，导入器应当以
> `PRCIDX1` 为目标。

## 向后兼容

* `PRCIDX1`（魔数 `PRCIDX1\0`）—— 当前的自描述 MEMORY 模式格式（可读可写）。
* `PRCIDXD1`（魔数 `PRCIDXD1`）—— DISK 模式的分页 B+tree 容器。
* `PRCISAM1`（魔数 `PRCISAM1`）—— 仅含记录的旧容器（只读；在可写打开的下一次
  `CLOSE` 时被重新保存为 `PRCIDX1`）。
* 其他任何内容 —— 视为空文件。

---

## 未来的 Fujitsu 导入路径

设想中的迁移流程（在今天全都不属于 PowerRustCOBOL 的范围）：

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

因为 `PRCIDX1` 已经能够*表达*复合键、键编码、键排序、重复策略、变长记录的上下界以及
键字段名，转换器要做的只剩下把 Fujitsu 的元数据翻译成 `IndexedFileInfo` 并把记录流
过来——不需要改动 PowerRustCOBOL 的格式。

**不要**试图解析 Fujitsu 的 `cobidx`/`cobi64` 原始字节。Fujitsu 的公开文档通过 File
Access Subroutines 暴露了元数据，但并未公布其物理字节布局。

.<<

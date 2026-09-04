<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL 索引文件格式（`PRCIDX1`）

本文档介绍在 PowerRustCOBOL 中支撑 `ORGANIZATION IS INDEXED` 文件的磁盘容器，
以及它如何映射到未来的 **Fujitsu COBOL-85 → PowerRustCOBOL 导入器**所需的元
数据。

> **与 Fujitsu 不是二进制兼容的。** `PRCIDX1` 是 PowerRustCOBOL 自有的自描述
> 容器。它在*语义上*参照了 Fujitsu 的 File Access Subroutines 通过
> `cobfa_indexinfo()` 暴露的元数据（记录格式、记录长度、键数量与键总长度、
> 主键、备用键）建模，但它**不会**解析或复现 Fujitsu 的 `cobidx`/`cobi64`
> 字节。导入器属于未来的工作，并且位于 PowerRustCOBOL 之外。

实现：[`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs)。

---

## 为什么该格式是自描述的

最初的容器（`PRCISAM1`）只保存魔数、记录长度和记录字节，**不携带任何键模式**。
转换器（或任何外部工具）在没有 COBOL `FD` 的情况下无法得知键是什么。

`PRCIDX1` 把完整的模式嵌入文件之中：记录格式，以及每个键的字节布局、排序方式、
重复策略和（可选的）COBOL 字段名。这让文件变得**可发现**——参见
[`inspect_path`](#发现-api)——并且让 Fujitsu 导入器能够仅凭从 Fujitsu 文件中读
出的元数据写出一个忠实的 PowerRustCOBOL 文件，而无须手头有对应的 `FD`。

---

## 元数据模型

这些 Rust 类型（从 `cobolt_runtime` 重新导出）就是该模式。它们对应
`cobfa_indexinfo()` 的概念；所有偏移量和长度都以**字节**为单位（绝不是字符
计数——与 Fujitsu 的 Unicode 模式规则一致）。

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

当前运行时发出的是**单部分、`Bytes` 编码、`Ascending`** 的键（COBOL `FD` 中的
`RECORD KEY` / `ALTERNATE RECORD KEY` 正是解析成这种键）。复合键、其他编码和
降序在**该格式中都是可表示的**，因此导入器可以无损地记录它们；运行时对它们的
完整支持属于未来的工作。

---

## 容器布局

所有整数均为**小端序**。文件结构如下：

```text
┌────────────────────────────────────────────────────────────┐
│ 文件头                                                     │
│ 键模式（key_count 个描述符：主键，然后是备用键）           │
│ 记录                                                       │
│ CRC-32 校验尾（覆盖此前的所有字节）                        │
└────────────────────────────────────────────────────────────┘
```

### 文件头

| 字段             | 类型      | 说明                                    |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | 保留（`0`）                             |
| `record_format`  | `u8`      | `1` = 定长，`2` = 变长                  |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | 定长时的记录长度                        |
| `min_length`     | `u32`     | 变长时的最小有效载荷                    |
| `max_length`     | `u32`     | 变长时的最大有效载荷                    |
| `key_count`      | `u16`     | 主键 + 备用键                           |
| `created_unix_ms`| `u64`     | 创建时间，跨重写保留                    |
| `updated_unix_ms`| `u64`     | 最后写入时间                            |

### 键模式 — 重复 `key_count` 次（主键在前）

| 字段           | 类型      | 说明                                    |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` 为主键，`2..` 为备用键              |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` 升序，`1` 降序                      |
| `part_count`   | `u16`     | `KeyPart` 的个数                        |
| `name_len`     | `u16`     | UTF-8 名称的长度（`0` = 无）            |
| `name`         | `[u8]`    | `name_len` 个字节                       |
| `parts`        | 重复      | `part_count` × KeyPart（见下）          |

每个 **KeyPart**：

| 字段       | 类型  | 说明                           |
|------------|-------|--------------------------------|
| `offset`   | `u32` | 在记录有效载荷中的字节偏移量   |
| `length`   | `u32` | 字节长度                       |
| `encoding` | `u8`  | `KeyEncoding` 判别值           |
| `reserved` | `u8`  | `0`                            |

### 记录

| 字段           | 类型 | 说明                                 |
|----------------|------|--------------------------------------|
| `record_count` | `u64`| 有效记录的数量                       |
| 每条记录       | 重复 | `length: u32`，随后是 `length` 个字节|

记录按**主键**升序写入。

### 校验尾

| 字段    | 类型  | 说明                                             |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | 对校验尾之前所有字节计算的 CRC-32（IEEE 802.3，反射）|

CRC 在加载时校验；不匹配时返回 FILE STATUS `90`（I/O 错误）。

---

## 发现 API

```rust
use cobolt_runtime::IndexedFile; // (engine type)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

对 `PRCIDX1` 文件返回 `Some(IndexedFileInfo)`，对旧版 `PRCISAM1` 容器（不携带
模式）返回 `None`。这就是转换器或检查工具可以调用的 `cobfa_indexinfo()` 对应
接口。

---

## 打开时的校验（FILE STATUS）

以 `INPUT` / `I-O` 方式打开**已存在的**索引文件时，运行时会把 `SELECT`/`FD`
中声明的键和记录格式与已存储的模式进行校验（严格模式，默认开启）。相关状态如下：

| 状态 | 条件                                                   |
|-------:|-------------------------------------------------------|
| `35`   | 对不存在的文件执行 `OPEN INPUT`                       |
| `39`   | 已有文件的模式 ≠ 声明的键／记录格式                   |
| `90`   | 容器损坏（CRC 不匹配）或其他 I/O 错误                 |

旧版 `PRCISAM1` 容器没有模式，因此对它跳过严格校验（始终以宽松方式加载）。

---

## 存储模式（`STORAGE IS MEMORY | DISK`）

`STORAGE MODE` 子句决定由哪个引擎——也就是哪种磁盘容器——来支撑一个 INDEXED
文件。**默认的存储模式是 `DISK`**（在没有 `STORAGE` 子句时）。`WITH COMPRESSION`
对两种模式都适用；`WITH PERSISTENCE` 只适用于 `MEMORY`。

| 模式 | 引擎 | 容器 | 说明 |
|------|------|------|------|
| `MEMORY` | 内存中的 `BTreeMap`（`indexed.rs`） | `PRCIDX1`（本文档） | 整个文件都在内存中；**默认是临时的**——`COMMIT` 从不写入磁盘。使用 `WITH PERSISTENCE` 时，仅在 `CLOSE` 时保存为 `PRCIDX1`。`OPEN OUTPUT` 总是（重新）创建容器。 |
| `DISK`（默认） | 持久化的分页 B+ 树（`indexed_disk.rs`） | `PRCIDXD1` | 按需读取记录与索引；内存占用有上限；始终持久化（逐操作写入，在 `COMMIT`/`CLOSE` 时 `fsync`） |

**`PRCIDXD1`** 磁盘容器是单个分页文件（4 KiB 的页）：

* **第 0 页**——文件头：各棵树的根（每个键一棵 B+ 树）、空闲页链表头、下一个页
  号、`RecordId` 计数器、记录数量、键模式以及压缩标志。
* **B+ 树页**——内部节点／叶子节点（按字节变长打包，插入时分裂，叶子节点双向
  链接以便有序扫描）。
* **数据页**——带槽的记录单元（每页多条记录），以及供大于一页的记录使用的溢出
  页链。
* **目录页**——`RecordId` → 物理位置的映射表。
* 一条**空闲页链表**把释放的页串起来以供复用。

`WITH COMPRESSION`（`compress.rs`）是一种无依赖的 PackBits 风格 RLE，作用于每
条存储的记录（`PRCIDXD1`）或记录区中的每条记录（`PRCIDX1`）；一个单字节标记
保证编码后体积绝不增大，并且容器文件头会记录压缩已开启。

> `PRCIDXD1` 用于 DISK 模式的原生存储。上文那些可发现、面向 Fujitsu 导入的元
> 数据属于 `PRCIDX1`（MEMORY 模式）容器；除非确实需要分页的磁盘布局，导入器
> 都应以 `PRCIDX1` 为目标。

## 向后兼容性

* `PRCIDX1`（魔数 `PRCIDX1\0`）——当前的自描述 MEMORY 模式格式（可读可写）。
* `PRCIDXD1`（魔数 `PRCIDXD1`）——DISK 模式的分页 B+ 树容器。
* `PRCISAM1`（魔数 `PRCISAM1`）——只含记录的旧版容器（只读；在下一次以可写方式
  打开后的 `CLOSE` 时，会重新保存为 `PRCIDX1`）。
* 任何其他内容——按空文件处理。

---

## 未来的 Fujitsu 导入路径

预期的迁移流程（目前全部都在 PowerRustCOBOL 的范围之外）：

```text
Fujitsu 运行时
  └─ cobfa_indexinfo()  → 记录格式、记录长度、键列表（主键 + 备用键）
  └─ 顺序导出           → 记录的有效载荷
        │
        ▼
  转换器（未来，外部）
        │  构建 IndexedFileInfo + 记录
        ▼
  PRCIDX1 文件  → 由 PowerRustCOBOL 原生打开
```

由于 `PRCIDX1` 已经能够*表示*复合键、键编码、键的排序方式、重复策略、变长记录
的长度范围以及键字段名，转换器只需把 Fujitsu 的元数据翻译成 `IndexedFileInfo`
并把记录流式写出即可——无须改动 PowerRustCOBOL 的格式。

**不要**尝试解析 Fujitsu 原始的 `cobidx`/`cobi64` 字节。Fujitsu 的公开文档通过
File Access Subroutines 暴露元数据，但并未公布物理字节布局。

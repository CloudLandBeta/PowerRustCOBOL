<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.134 -->

# PowerRustCOBOL 索引文件内部构造（`PRCIDXD1` 分页引擎）

本文档是对**持久化、分页的磁盘**引擎的概念性描述；以 `STORAGE IS DISK`（默认值）声明
的 `ORGANIZATION IS INDEXED` 文件由它支撑。它采用 B+tree 加带槽位页的设计，按需读取
记录，因此无论文件多大，内存占用都有界。

> ⚠️ **它已不再是默认引擎。** `STORAGE IS DISK` 仍然是默认的*存储模式*，但自
> **1.62.73** 起为其服务的引擎是 **redb**
> （`crates/cobolt-runtime/src/indexed.rs:126`）——参见
> [`indexed-redb-engine-cn.md`](indexed-redb-engine-cn.md)。下面的一切对分页引擎仍然
> 准确，而那个引擎依旧可以用 `--indexed-engine rust` 取到；它只是不再是程序默认会
> 得到的那一个。
>
> **范围。** 这里描述的是*物理引擎*（`DiskIndexedFile`，容器魔数 `PRCIDXD1`）。它与
> [`indexed-file-format-cn.md`](indexed-file-format-cn.md) 中记载的那个单块、自描述的
> `PRCIDX1` 容器是两个不同的产物；后者建模的是未来 Fujitsu 导入器所需的元数据。内存
> 引擎（`STORAGE IS MEMORY`，`IndexedFile`）是同一逻辑模型的简化子集（用 BTreeMap
> 代替磁盘上的 B+tree）。
>
> 第二个**抗崩溃**的 `STORAGE IS DISK` 引擎（需显式选择，构建在纯 Rust 的 redb ACID
> 存储之上）解决了本引擎受内存约束的目录和只在 CLOSE 时才持久化的问题——参见
> [`indexed-redb-engine-cn.md`](indexed-redb-engine-cn.md)。

实现：
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs)，
记录的（反）物化位于
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs)。

---

## 1. 一句话概括设计

一个分页文件，由**一页头部 + N 棵 B+tree（每个键一棵）→ 一个 RecordId 目录 → 存放
按位置排列、定宽记录映像的带槽位数据页**构成，再加上空闲链表、溢出链、可选的 RLE
压缩，以及一份供事务使用的运行期撤销日志。

---

## 2. 文件是一个由 4 KiB 定长页组成的数组

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

第 0 页**之后**的每一页都用它的第一个字节（页类型标签）表明自己的身份。被释放的页
会经由空闲链表回收再用，所以磁盘上的物理页顺序**并不**跟随记录的逻辑顺序。

| 标签 | 常量          | 该页存放什么                                  |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | B+tree 的内部（路由）节点                     |
| `2` | `PT_LEAF`     | B+tree 的叶节点（与相邻叶节点双向相连）       |
| `3` | `PT_DATA`     | 打包了若干条记录映像的带槽位页                |
| `4` | `PT_OVERFLOW` | 一条大到放不进行内的记录的续页                |
| `5` | `PT_DIR`      | RecordId 目录的一段                           |

---

## 3. 第 0 页 —— 头部

第 0 页是唯一存放*模式*的地方，而且只写一次。字段为小端序，顺序如下：

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

| 头部字段          | 含义                                                          |
|-------------------|---------------------------------------------------------------|
| `version`         | 格式版本（当前为 `1`）。                                      |
| `page_size`       | 页大小，单位字节（4096）。                                    |
| `rec_fmt`         | 记录格式：`1` = 定长。                                        |
| `compressing`     | 若磁盘上的记录负载经 RLE 压缩则为 `1`。                       |
| `record_len`      | 记录的逻辑（未压缩）长度，单位字节。                          |
| `next_page_id`    | 空闲链表为空时，下一个要分配的页号。                          |
| `free_list_head`  | 已回收页空闲链表的第一页（`0` = 没有）。                      |
| `record_count`    | 存活记录的数量。                                              |
| `data_tail`       | 当前接受行内写入的 `PT_DATA` 页（`0` = 没有）。               |
| `primary_root`    | 主键 B+tree 的根页。                                          |
| `dir_head`        | RecordId 目录的第一个 `PT_DIR` 页（`0` = 没有）。             |
| `directory_len`   | 目录条目数（历来分配过的 RecordId 数）。                      |
| `alt_root[k]`     | 第 *k* 个备用键的 B+tree 根页。                               |
| 键模式            | 每个键的重复策略，以及复合键各段的字节区间。                  |

**头部里刻意*没有*的东西：** 既**没有数据字段名**，也**没有逐条记录的元数据**。这个
模式纯粹是*键的几何形状*（字节区间）。一条记录的其余一切都由位置决定——参见 §6。

---

## 4. 访问路径（一次按键的 `READ` 如何解析）

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

**一条记录，多个键。** 主键和每个备用键指向的是*同一个* RecordId，因此每条记录只存
一份。备用索引不过是叠加在共享 RecordId 目录之上的额外 B+tree；当某个键被声明为
`WITH DUPLICATES` 时，重复的备用键值才被允许。

---

## 5. 页的内部构造

### 5.1 B+tree 节点（`PT_INTERNAL` / `PT_LEAF`）

一个节点会为某次操作被读入内存、被修改、必要时分裂，然后写回。

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- 叶节点是**双向相连**的（`next`/`prev`），因此 `START` 之后的有序扫描可以直接在
  相邻叶节点间行走——这就是 RustCOBOL 的升序键 `READ NEXT`。
- 当序列化后的节点会超过 `PAGE_SIZE` 时，插入会**因溢出而分裂**；中位键被提升到父
  节点。
- 内部节点保存 `child0`，外加一组*（分隔键, 子节点）*对。

### 5.2 带槽位的数据页（`PT_DATA`）

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- 5 字节的页头，随后是一张从前往后生长的**槽位目录**，而**记录负载**从后往前生长；
  只要这两个区域还没碰头，记录就能放在行内。
- 一个槽位是 `(offset, len)`；删除一条记录会把它的槽位置为 `len = 0`（墓碑）。当一页
  上所有槽位都空闲时，整页归还给空闲链表。
- `RecLoc` 的 `slot` 字段就是在这张槽位目录里的下标。

### 5.3 溢出链（`PT_OVERFLOW`）

大于行内上限（`PAGE_SIZE − 页头 − 一个槽位`）的记录会被存成一条由溢出页构成的链表；
它的 `RecLoc.kind = 2`，而 `page` 指向链首。

### 5.4 RecordId 目录（`PT_DIR`）

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

文件打开期间，目录以 `Vec<RecLoc>` 的形式保存在内存中（因此按 RecordId 查找就是一次
O(1) 的下标访问），并在关闭时以一串 `PT_DIR` 页（从 `dir_head` 开始）持久化。B+tree
里存的是 RecordId，绝不是物理地址，所以一条记录可以在磁盘上移动而不必碰任何索引。

---

## 6. 记录映像本身（按位置排列，不带名字）

磁盘上的一条记录是单个**定宽字节缓冲区**，按字段的*偏移量*排布——负载里没有字段名、
没有标签、也没有分隔符。对于：

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

存储下来的映像是 23 字节：

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- `RecordLayout::materialize()` 在 `WRITE`/`REWRITE` 时按偏移量把 `FD` 的基本项打包
  进这个缓冲区；`RecordLayout::distribute()` 在 `READ` 时做反向操作。字段 → 偏移量
  的映射只存在于程序的 `RecordLayout` 里（由 `FD` 推导而来），**绝不**在文件里。
- **身份就是位置。** 这是"不要在每条记录里重复键名"这一思路的极限情形：字段身份在每
  条记录上花费*零*字节，而字段访问凭预先算好的偏移量是 O(1) 的（无需解析）。重命名
  一个非键字段，磁盘上什么都不会变；重命名一个键字段，只会重写头部的键模式，记录和
  索引都不动。改变一个字段的偏移量或宽度，是唯一需要重写数据的改动——这是定长记录
  （以及真正的 ISAM/VSAM）固有的性质。

### 压缩

使用 `STORAGE IS DISK WITH COMPRESSION` 时，**存储的**负载会经 PackBits 式 RLE 压缩
（`compress.rs`），而 `RecLoc.len` 是*存储后*的长度；读取时缓冲区会被还原回
`record_len`。压缩对键的几何形状和访问路径都是透明的。

---

## 7. 空闲空间与重用

- **空闲链表。** `free_list_head` 串起从清空的数据页、分裂后成为孤儿的节点等处回收
  来的页；`allocate` 会先从这里取，再去递增 `next_page_id`，因此空间得到复用，文件
  不会单调增长。
- **墓碑。** 一次 `DELETE` 会释放槽位（并延迟释放数据页），并把目录条目标记为
  `RecLoc::FREE`；该 RecordId 就此退役。

---

## 8. 事务（运行期撤销日志）

磁盘引擎为自上一次 `COMMIT`/`OPEN` 以来的每一次改动，保留一份逆操作的**撤销日志**：

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- `OPEN` 开启一个事务（清空日志）；`COMMIT` 让改动持久化并开启新的事务；`ROLLBACK`
  按相反顺序重放这些逆操作；`CLOSE` 刷盘（隐式提交）。一个 `tx_replay` 守卫可以防止
  这些逆操作自己又被记进日志。
- 这是**程序级**的回滚。借助持久化预写日志实现的崩溃恢复属于将来的工作。参见语言参考
  中的 COBOL `COMMIT`/`ROLLBACK` 动词；请注意那些动词作用于 **INDEXED 文件**，而不是
  SQL 连接。

---

## 9. OPEN 时的校验

在 `OPEN` 时，头部中保存的键模式会与程序的 `SELECT` 作比对（记录长度、键的数量、每个
键的组成段及其重复策略）。不匹配会返回 COBOL 文件状态 `39`；以 `INPUT` 打开一个不存在
的文件返回 `35`；头部损坏或过短返回 `90`。（严格校验可以通过引擎的 `strict_metadata`
标志放宽。）

---

## 10. 速查 —— 谁存了什么

| 事物                          | 存放位置                               | 份数        |
|-------------------------------|----------------------------------------|-------------|
| 键的几何形状（偏移量/宽度）   | 头部（第 0 页）的键模式                | 一份        |
| 数据字段名                    | 只在程序的 `FD` 里                     | 不在文件中  |
| 记录字节                      | `PT_DATA` / `PT_OVERFLOW` 页            | 每条记录一份 |
| 键 → RecordId                 | 每个键一棵 B+tree                      | 每个键一份  |
| RecordId → 物理位置           | RecordId 目录（`PT_DIR` 链）           | 每条记录一份 |
| 空闲页                        | 空闲链表（`free_list_head`）           | —           |
| 未提交改动的逆操作            | 内存中的撤销日志                       | 每个事务一份 |

.<<

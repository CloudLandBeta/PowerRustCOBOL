<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL 索引文件内部结构（`PRCIDXD1` 分页引擎）

本文档是**持久化分页磁盘**引擎的概念性架构说明，该引擎支撑以
`STORAGE IS DISK`（默认值）声明的 `ORGANIZATION IS INDEXED` 文件。它是一种
B+ 树 / 槽式页设计，按需读取记录，因此无论文件多大，内存占用始终有界。

> **范围。** 本文描述的是*物理引擎*（`DiskIndexedFile`，容器魔数
> `PRCIDXD1`）。它与
> [`indexed-file-format-en.md`](indexed-file-format-cn.md) 中记载的单块、自描述的
> `PRCIDX1` 容器是不同的产物，后者建模的是未来的富士通导入器所需的元数据。
> 内存引擎（`STORAGE IS MEMORY`、`IndexedFile`）是同一逻辑模型的简化子集
> （用 BTreeMaps 代替磁盘上的 B+ 树）。
>
> 第二个**抗崩溃**的 `STORAGE IS DISK` 引擎（可选启用，构建在纯 Rust 的 redb
> ACID 存储之上）解决了本引擎受内存限制的目录以及仅在 CLOSE 时才持久化的问题
> —— 参见 [`indexed-redb-engine-cn.md`](indexed-redb-engine-cn.md)。

实现：
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs)，
记录的（反）实体化位于
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs)。

---

## 1. 一句话设计

一个分页文件，由**一个文件头页 + N 棵 B+ 树（每个键一棵）→ 一张 RecordId 目录 →
存放定宽、按位置定位的记录映像的槽式数据页**组成，并配有空闲链表、溢出链、可选
的 RLE 压缩，以及供事务使用的运行期撤销日志。

---

## 2. 文件是固定 4 KiB 页的数组

```
 字节 0                                                           文件末尾
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 字节（固定）。   页 ID = 字节偏移量 / 4096。
```

第 0 页**之后**的每一页都通过自己的第一个字节（页类型标记）标识自身。被释放的
页会经由空闲链表回收复用，因此磁盘上的物理页顺序**并不**对应记录的逻辑顺序。

| 标记 | 常量          | 该页存放的内容                                     |
|-----|---------------|--------------------------------------------------------|
| `1` | `PT_INTERNAL` | B+ 树内部（路由）节点                                   |
| `2` | `PT_LEAF`     | B+ 树叶子节点（与兄弟节点双向链接）                     |
| `3` | `PT_DATA`     | 打包多条记录映像的槽式页                                |
| `4` | `PT_OVERFLOW` | 记录太大无法内联存放时的续存页                          |
| `5` | `PT_DIR`      | RecordId 目录的一个分片                                 |

---

## 3. 第 0 页 —— 文件头

第 0 页是唯一存放*架构*的地方，而且只写入一次。字段按小端序排列，顺序如下：

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 字节)  (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (各 u64)
 primary_root   dir_head         directory_len                 (各 u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (每个替代键一棵 B+ 树根)
 ──────────────────────────────────────────────────────────────────────
 键架构:  key_count (u16) → 对每个键(先主键,再替代键):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × 部分数   (复合键的组成部分)
```

| 文件头字段        | 含义                                                    |
|-------------------|---------------------------------------------------------|
| `version`         | 格式版本（当前为 `1`）。                                |
| `page_size`       | 页大小，以字节计（4096）。                              |
| `rec_fmt`         | 记录格式：`1` = 定长。                                  |
| `compressing`     | 若记录负载在磁盘上经过 RLE 压缩则为 `1`。               |
| `record_len`      | 记录的逻辑（未压缩）长度，以字节计。                    |
| `next_page_id`    | 空闲链表为空时下一个要分配的页 ID。                     |
| `free_list_head`  | 已回收页空闲链表的首页（`0` = 无）。                    |
| `record_count`    | 存活记录的数量。                                        |
| `data_tail`       | 当前接受内联写入的 `PT_DATA` 页（`0` = 无）。           |
| `primary_root`    | 主键 B+ 树的根页。                                      |
| `dir_head`        | RecordId 目录的首个 `PT_DIR` 页（`0` = 无）。           |
| `directory_len`   | 目录条目数（历史上分配过的 RecordId 总数）。            |
| `alt_root[k]`     | 第 *k* 个替代键的 B+ 树根页。                           |
| 键架构            | 每个键的重复策略 + 复合部分的字节范围。                 |

**文件头里刻意*没有*的东西：**这里既**没有数据字段名**，也**没有按记录保存的元
数据**。架构纯粹是*键的几何布局*（字节范围）。记录的其余一切都靠位置决定 ——
参见 §6。

---

## 4. 访问路径（按键的 `READ` 如何解析）

```
  COBOL 键值(字节)
        │
        ▼
  ┌──────────────┐   从 primary_root(按 RECORD KEY 的随机 READ)或
  │  B+tree      │   alt_roots[k](READ KEY IS <alt>)开始。内部节点按键路由;
  │  (每键一棵)  │   叶子保存 (key_bytes → RecordId),并且为了
  │              │   READ NEXT / READ PREVIOUS / START 而双向链接
  └──────┬───────┘   (next/prev)。
         │  RecordId(一个稳定的整数,与物理位置无关)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = 空闲/墓碑,1 = 内联,2 = 溢出链首
  │  目录        │     len : 已存储的字节长度(可能是压缩后的)
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   槽式 DATA 页 → 槽目录 → (offset, len) →
  │  DATA 页     │   原始记录映像(若 `compressing` 则先解压)。
  └──────┬───────┘
         ▼
  定宽的记录字节
        │  RecordLayout.distribute()
        ▼
  分散到工作内存中 FD 的各基本项
```

**一条记录，多个键。** 主键和每个替代键都指向*同一个* RecordId，因此每条记录只
存储一份副本。替代索引不过是叠加在共享 RecordId 目录之上的额外 B+ 树；当某个键
声明为 `WITH DUPLICATES` 时，该键允许出现重复的替代值。

---

## 5. 页的内部结构

### 5.1 B+ 树节点（`PT_INTERNAL` / `PT_LEAF`）

节点会为某个操作被读入内存、被修改、必要时分裂，然后写回。

```
 叶子:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 内部:      type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- 叶子之间是**双向链接**的（`next`/`prev`），因此 `START` 之后的有序扫描可以直接
  沿兄弟节点行走 —— 这就是 RustCOBOL 的键升序 `READ NEXT`。
- 当序列化后的节点会超过 `PAGE_SIZE` 时，插入会**因溢出而分裂**；中位键被提升到
  父节点。
- 内部节点保存 `child0` 以及若干 *(分隔键, 子节点)* 对。

### 5.2 槽式数据页（`PT_DATA`）

```
 ┌─ 字节 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ 槽目录 ──────────────┬─ 空闲 ─┬─ 记录数据 ────┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  紧凑存放     │
 │          │ count   │ top     │ 增长  →               │        │  ←  增长      │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- 5 字节的页头，随后是从前往后增长的**槽目录**，而**记录负载**则从后往前增长；
  只要这两个区域尚未相遇，记录就能内联存放。
- 一个槽就是 `(offset, len)`；删除一条记录会把它的槽置为 `len = 0`（墓碑）。当一
  页上的所有槽都空闲时，整页会被归还给空闲链表。
- `RecLoc` 的 `slot` 字段就是这张槽目录的下标。

### 5.3 溢出链（`PT_OVERFLOW`）

大于内联上限（`PAGE_SIZE − 页头 − 一个槽`）的记录会被存成一条由溢出页组成的链
表；它的 `RecLoc.kind = 2`，而 `page` 指向链首。

### 5.4 RecordId 目录（`PT_DIR`）

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (每条 15 字节)
```

文件打开期间，目录以 `Vec<RecLoc>` 的形式常驻内存（因此按 RecordId 查找就是一次
O(1) 下标访问），并在关闭时持久化为一条 `PT_DIR` 页链（从 `dir_head` 开始）。
B+ 树里存的是 RecordId，绝不是物理地址，所以记录可以在磁盘上移动而无需改动任何
索引。

---

## 6. 记录映像本身（按位置定位，没有名字）

磁盘上的一条记录就是一个按字段*偏移量*排布的**定宽字节缓冲区** —— 负载里既没有
字段名，也没有标签或分隔符。对于：

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

存储的映像是 23 字节：

```
 偏移量:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 负载:    │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = 空格填充)
```

- `RecordLayout::materialize()` 在 `WRITE`/`REWRITE` 时按偏移量把 FD 的各基本项
  打包进这个缓冲区；`RecordLayout::distribute()` 在 `READ` 时做相反的事。字段 →
  偏移量的映射只存在于程序的 `RecordLayout`（由 `FD` 推导而来）中，**绝不**存在
  于文件里。
- **身份即位置。** 这是"不要在每条记录里重复键名"这一原则的极限情形：字段身份在
  每条记录上的开销是*零*字节，而字段访问是按预先算好的偏移量进行的 O(1) 操作
  （无需解析）。重命名一个非键字段，磁盘上什么都不会变；重命名一个键字段，只会
  重写文件头里的键架构，而不会动记录或索引。改变某个字段的偏移量或宽度，是唯一
  一种必须重写数据的改动 —— 这是定长记录（以及真实的 ISAM/VSAM）与生俱来的性质。

### 压缩

使用 `STORAGE IS DISK WITH COMPRESSION` 时，**存储的**负载会用 PackBits-RLE 压缩
（`compress.rs`），且 `RecLoc.len` 是*存储后的*长度；读取时缓冲区会被还原回
`record_len`。压缩对键的几何布局和访问路径都是透明的。

---

## 7. 空闲空间与复用

- **空闲链表。** `free_list_head` 把从清空的数据页、分裂后被遗弃的节点等处回收来
  的页串成链；`allocate` 会先从中弹出页，然后才递增 `next_page_id`，因此空间得以
  复用，文件不会单调增长。
- **墓碑。** `DELETE` 会释放槽（并惰性地释放数据页），并把目录条目标记为
  `RecLoc::FREE`；该 RecordId 就此退役。

---

## 8. 事务（运行期撤销日志）

磁盘引擎为自上一次 `COMMIT`/`OPEN` 以来的每一次改动保存一份逆操作的**撤销日志**：

```
 DiskUndo::Insert(key)        ← WRITE   → 通过删除该键来撤销
 DiskUndo::Update(prev_image) ← REWRITE → 通过重写先前的映像来撤销
 DiskUndo::Delete(prev_image) ← DELETE  → 通过把映像写回来撤销
```

- `OPEN` 开启一个事务（清空日志）；`COMMIT` 让改动持久化并开启新的事务；
  `ROLLBACK` 按相反顺序重放这些逆操作；`CLOSE` 刷盘（隐式提交）。一个 `tx_replay`
  守卫可防止逆操作把自身再次记入日志。
- 这是**程序级**的回滚。借助持久化预写日志实现的崩溃恢复属于未来的工作。COBOL 的
  `COMMIT`/`ROLLBACK` 动词请参见语言参考；注意这些动词作用于 **INDEXED 文件**，而
  不是 SQL 连接。

---

## 9. OPEN 校验

`OPEN` 时会把文件头中存储的键架构与程序的 `SELECT` 相比对（记录长度、键的数量、
每个键的组成部分及其重复策略）。不匹配会返回 COBOL 文件状态 `39`；以 `INPUT` 打开
一个不存在的文件返回 `35`；文件头损坏或过短则返回 `90`。（严格校验可以通过引擎的
`strict_metadata` 标志放宽。）

---

## 10. 快速参考 —— 什么存在哪里

| 事物                            | 存放位置                                | 份数         |
|-------------------------------|----------------------------------------|-------------|
| 键的几何布局（偏移量/宽度）      | 文件头（第 0 页）的键架构                | 一份        |
| 数据字段名                     | 仅在程序的 `FD` 中                      | 不在文件里  |
| 记录字节                       | `PT_DATA` / `PT_OVERFLOW` 页            | 每条记录一份 |
| 键 → RecordId                 | 每个键一棵 B+ 树                        | 每个键一棵  |
| RecordId → 物理位置           | RecordId 目录（`PT_DIR` 链）            | 每条记录一份 |
| 空闲页                         | 空闲链表（`free_list_head`）            | —           |
| 未提交改动的逆操作              | 内存中的撤销日志                        | 每个事务一份 |
```

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# 抗崩溃的 INDEXED 引擎（redb）

PowerRustCOBOL 为 `ORGANIZATION IS INDEXED` 文件提供了第二个 `STORAGE IS DISK`
引擎，构建在 **redb** 之上——一个纯 Rust 编写的嵌入式 ACID 键值存储（写时复制
B+tree、双元页、逐页校验和）。它呈现出与旧的 `PRCIDXD1` 引擎*完全相同*的可观察
COBOL 行为，但其设计围绕四个运维目标，而这些目标是自研引擎在规模上无法达成的。

**它是默认引擎，并且自 1.62.73 起一直如此**（操作者裁定，2026-08-29）。
`IndexedEngine` 在 `Redb` 上以 `#[default]` 派生 `Default`
（`crates/cobolt-runtime/src/indexed.rs:126`），并有一个测试把它固定在那里
（`indexed.rs:1643`）。无需选择任何东西即可获得它。

旧的分页引擎仍可按名称使用，委托给内置 Rust 容器的那两个别名同样可用：

```bash
rcrun run program.cbl --indexed-engine rust    # the PRCIDXD1 paged engine
# or
COBOL_INDEXED_ENGINE=rust rcrun run program.cbl
```

实现：
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs)。

---

## 为什么——四个目标

| 目标 | redb 引擎如何达成 |
|------|------------------------------|
| **OPEN 永远是瞬时的** | redb 打开时只读取它的元页。**没有需要载入内存的记录目录，也没有恢复扫描**，即使在崩溃之后也是如此。实测：打开一个 20 万条记录的文件约 5 ms（与记录数无关）。 |
| **READ RANDOM / NEXT 快如闪电** | RANDOM 是一次 B+tree 下降；NEXT 是一个顺序范围迭代器。两者都在 redb 的页缓存之上运行。实测：20 万条记录时每次随机读取约 21 µs。 |
| **最多 2.5 亿条记录（数据量不设限）** | 常驻内存取决于工作集（redb 的缓存），**而不是**记录数。内存中不保留任何 `O(记录数)` 的结构。 |
| **安全高于一切** | redb 完全符合 ACID。`COMMIT` 是一次持久的事务提交（fsync）；`ROLLBACK` 是一次事务中止。断电绝不可能暴露出被撕裂的索引——redb 会通过它的双元页回退到最后一次正确的提交。不丢数据，索引不损坏。 |

与 `PRCIDXD1` 引擎相比：后者的 RecordId 目录在 OPEN 时整个载入内存（≈16 字节 ×
曾经分配过的每一个 RecordId），其事务只是一份内存中的撤销日志、仅在 CLOSE 时才落
盘——因此它既无法在规模上做到瞬时打开，也无法在运行途中挺过一次断电。

---

## 磁盘布局（redb 表）

| redb 表 | 类型     | 键 → 值                                        |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | 主键字节 → 记录（可选压缩）                    |
| `alt`      | multimap | `[u16 idx][备用键字节]` → `[u64 seq][主键]`    |
| `seq`      | table    | 主键字节 → `u64` 插入序号                      |
| `meta`     | table    | `schema`、`compress`、`nextseq` 描述符         |

- **单个 `alt` 多重映射**保存全部备用键，以一个 2 字节大端键索引划分命名空间。因此
  字节序为 `(键索引, 备用值, 插入序号)`——这使得重复的备用键按**创建顺序**迭代，
  恰好与磁盘引擎的 RecordId 排序以及 COBOL 对重复备用键的规则一致。
- `seq` / `meta:nextseq` 这套机制**只**用于给备用键的重复项排序。没有备用键的文件
  会完全跳过它，每次 `WRITE` 只付出一次 B+tree 插入的代价。
- 记录以定宽的按位置排列映像存储（参见
  [`indexed-file-internals-cn.md`](indexed-file-internals-cn.md) §6）；
  `WITH COMPRESSION` 采用与其他引擎相同的 PackBits RLE。

---

## 事务模型

可写方式的打开（`OUTPUT` / `I-O` / `EXTEND`）会从 OPEN 起持有一个 redb
`WriteTransaction`。通过该事务进行的读取能看到程序自身尚未提交的写入（COBOL 的
"读到自己的写入"）。COBOL 动词直接对应：

| COBOL | redb |
|-------|------|
| `OPEN`     | 开启一个写事务（可写模式） |
| `COMMIT`   | 对事务执行 `commit()`（持久化），然后开启一个新的 |
| `ROLLBACK` | 对事务执行 `abort()`（丢弃自上一次 `COMMIT`/`OPEN` 以来的一切），然后开启一个新的 |
| `CLOSE`    | `commit()`（隐式提交） |

`INPUT` 方式的打开使用短的读事务。由于 `ROLLBACK` 是真正的 redb 中止，**无需任何
撤销日志**——持久性与回滚都是该存储自身的保证。

> COBOL 的 `COMMIT` / `ROLLBACK` 动词作用于 **INDEXED 文件**，而不是 SQL 连接
> （后者使用 `COBOL-EXEC-SQL` 配合 `BEGIN`/`COMMIT`/`ROLLBACK`）。

---

## 行为一致性

该引擎必须表现出与默认引擎完全一致的行为：同一批受版本管理的用例
（`tests/cobol/fileio/idx_crud.cbl`、`idx_persist.cbl`、`idx_tx.cbl`）在
`--indexed-engine redb` 下运行，必须产生完全相同的 DISPLAY 输出——包括主键加
`WITH DUPLICATES` 备用键的 CRUD、跨重新打开的持久性，以及 `COMMIT`/`ROLLBACK`。
文件状态码（`00/02/10/22/23/35/39/46/47/48/49/90/...`）、参照键的解析、`START` 的
语义，以及"REWRITE/DELETE 需要当前记录"这条规则，全都一致。

测试：`crates/cobolt-runtime/tests/test_indexed_redb.rs`（redb 下的用例 + 对
`IndexedStore` 的直接检查 + 一个标记为 `#[ignore]` 的规模冒烟测试）。

---

## 限制

由于该引擎按需分页，实际限制由 redb 和文件系统决定，而不是由常驻内存决定：

| 维度 | 限制 |
|-----------|-------|
| 文件大小 | 受 redb / 文件系统限制（TB 级） |
| 记录数 | 受工作集内存限制，而非记录数限制（小缓存下 ≥2.5 亿） |
| 记录大小 | 定宽映像；大记录作为 redb 值存储 |
| 键大小 | 复合键的字节数（COBOL 层支持多段键） |
| 备用键 | 最多 65 535 个（2 字节索引命名空间） |

---

## 性能说明

- 按主参照键进行的**顺序 `READ NEXT`** 直接从范围游标返回记录——每条记录一次
  B+tree 下降，而不是两次（20 万条时约 17 µs/记录）。按备用键扫描仍然是一次备用键
  下降加一次主键取值。
- **`WRITE`** 每次操作打开一次 `primary`/`alt` 表（重复检查与插入共用句柄）。一个
  微基准显示，在多次调用*之间*缓存句柄，相较每次操作打开一次只快约 8 %，因此引擎
  保留了这条简单、无 `unsafe` 的路径。写入开销（约 44 µs/记录）主要来自 redb 的
  ACID B+tree 插入，那是安全的下限——所有写入优化都不改变提交点或持久性。
- 因此**批量 `WRITE`** 在单个事务中约为每秒 2 万条记录（一次性的装载代价）。OPEN、
  读取和抗崩溃能力均不受影响。

---

## 可观测性日志（`--indexed-log`）

redb 引擎可以按文件写出一份可选的事务日志（默认关闭），路径为
**`<assign 路径>.log`**（例如 `customers.idx` → `customers.idx.log`），每次
`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` 一行，记录时间戳、记录数与字节数、吞吐量、写入
时的键有序程度，以及在 `full` 级别下的 redb 索引页统计。

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

行格式为 `text`（logfmt）或 `json`（NDJSON，可直接用于 Grafana/Loki）。

**完整参考**——参数、字段表、格式、Grafana/Loki 管线（Promtail + LogQL），以及
成本与安全说明——位于 [`observability-cn.md`](observability-cn.md) §1。

.<<

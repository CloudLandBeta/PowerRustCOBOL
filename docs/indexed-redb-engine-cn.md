<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 抗崩溃的 INDEXED 引擎（redb）

PowerRustCOBOL 为 `ORGANIZATION IS INDEXED` 文件提供了第二个
`STORAGE IS DISK` 引擎，构建在 **redb** 之上——一个纯 Rust 的嵌入式 ACID 键值
存储（写时复制 B+ 树、双元数据页、逐页校验和）。它对 COBOL 呈现的可观察行为与默认
的 `PRCIDXD1` 引擎*完全一致*，但其设计围绕着四个定制引擎在规模上无法达成的运维
目标。

目前它是**可选的**（默认的磁盘引擎仍是 `PRCIDXD1`）：

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

实现见
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs)。

---

## 为什么——四个目标

| 目标 | redb 引擎如何达成 |
|------|------------------------------|
| **OPEN 永远是瞬时的** | redb 打开时只读它的元数据页。**没有要加载的内存记录目录，也没有恢复扫描**，即使刚崩溃过也一样。实测：打开一个 20 万条记录的文件约 5 ms（与记录条数无关）。 |
| **READ RANDOM / NEXT 快如闪电** | RANDOM 是一次 B+ 树下降；NEXT 是顺序区间迭代器。两者都跑在 redb 的页缓存之上。实测：20 万条记录时每次随机读约 21 µs。 |
| **可达 2.5 亿条记录（数据量不设上限）** | 常驻内存对应的是工作集（redb 的缓存），**而不是**记录条数。内存中不保留任何 `O(记录数)` 的结构。 |
| **安全性至上** | redb 完全符合 ACID。`COMMIT` 是一次持久化的事务提交（fsync）；`ROLLBACK` 是事务中止。断电绝不会暴露出一个撕裂的索引——redb 会借助双元数据页回退到最后一个完好的提交。不丢数据，索引不损坏。 |

与 `PRCIDXD1` 引擎形成对照：后者在 OPEN 时把 RecordId 目录整个装入内存（每个曾
分配过的 RecordId 约 16 字节），其事务则是一份仅在 CLOSE 时才落盘的内存撤销日志
——因此它既无法在大规模下瞬时打开，也无法在运行中途的断电中幸存。

---

## 磁盘布局（redb 的表）

| redb 表 | 类型     | 键 → 值                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | 主键字节 → 记录（可选压缩） |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | 主键字节 → `u64` 插入序号  |
| `meta`     | table    | `schema`、`compress`、`nextseq` 描述符   |

- **单独一个 `alt` multimap** 存放所有的备用键，用 2 字节大端键索引来划分命名
  空间。因此字节序是 `(键索引, 备用键值, 插入序号)`——这使得重复的备用键按
  **创建顺序**迭代，恰好与磁盘引擎的 RecordId 排序以及 COBOL 关于重复备用键的
  规则相吻合。
- `seq` / `meta:nextseq` 这套机制**只**为给备用键的重复项排序而存在。没有备用键
  的文件会完全跳过它，每次 `WRITE` 只付出一次 B+ 树插入的代价。
- 记录以定长按位图像的形式存储（参见
  [`indexed-file-internals-cn.md`](indexed-file-internals-cn.md) §6）；
  `WITH COMPRESSION` 采用与其他引擎相同的 PackBits RLE。

---

## 事务模型

一次可写的打开（`OUTPUT` / `I-O` / `EXTEND`）会从 OPEN 起持有一个 redb
`WriteTransaction`。透过该事务的读取能看到程序自己尚未提交的写入（COBOL 的
"读自己的写"）。COBOL 动词直接对应：

| COBOL | redb |
|-------|------|
| `OPEN`     | 开启一个写事务（可写模式） |
| `COMMIT`   | 对事务执行 `commit()`（持久化），然后开启一个新的 |
| `ROLLBACK` | 对事务执行 `abort()`（丢弃自上次 `COMMIT`/`OPEN` 以来的一切），然后开启一个新的 |
| `CLOSE`    | `commit()`（隐式提交） |

以 `INPUT` 打开时使用短读事务。由于 `ROLLBACK` 就是 redb 真正的中止，
**不需要任何撤销日志**——持久性与回滚是存储本身的保证。

> COBOL 的 `COMMIT` / `ROLLBACK` 作用于 **INDEXED 文件**，而不是 SQL 连接
> （后者使用 `COBOL-EXEC-SQL` 配合 `BEGIN`/`COMMIT`/`ROLLBACK`）。

---

## 行为一致性

该引擎必须做到与默认引擎完全相同的行为：同一批纳入版本管理的固定用例
（`tests/cobol/fileio/idx_crud.cbl`、`idx_persist.cbl`、`idx_tx.cbl`）在
`--indexed-engine redb` 下运行，必须产生完全相同的 DISPLAY 输出——主键加
`WITH DUPLICATES` 备用键的 CRUD、跨重新打开的持久性，以及 `COMMIT`/`ROLLBACK`。
文件状态码（`00/02/10/22/23/35/39/46/47/48/49/90/...`）、参照键的解析、`START`
的语义，以及"REWRITE/DELETE 需要有当前记录"这条规则，也都一一吻合。

测试见 `crates/cobolt-runtime/tests/test_indexed_redb.rs`（redb 下的固定用例
＋对 `IndexedStore` 的直接检查＋一个标了 `#[ignore]` 的规模冒烟测试）。

---

## 限制

由于该引擎按需分页，实际限制由 redb 和文件系统决定，而不是常驻内存：

| 维度 | 限制 |
|-----------|-------|
| 文件大小 | 受 redb ／文件系统限制（TB 级） |
| 记录数 | 受工作集内存限制，而非记录条数限制（小缓存下 ≥2.5 亿条） |
| 记录大小 | 定长图像；大记录作为 redb 的值存储 |
| 键大小 | 复合键的字节数（COBOL 层支持多段键） |
| 备用键 | 最多 65 535 个（2 字节索引命名空间） |

---

## 性能说明

- 以主参照键进行的**顺序 `READ NEXT`** 直接从区间游标返回记录——每条记录一次
  B+ 树下降，而不是两次（20 万条时每条约 17 µs）。按备用键的扫描仍然是一次备用
  键下降加一次主键取值。
- **`WRITE`** 每次操作只打开一次 `primary`/`alt` 表（重复检查与插入共用同一个
  句柄）。一个微基准显示，*跨调用*缓存句柄相比每次操作打开一次只快约 8 %，因此
  引擎保留了简单、不含 `unsafe` 的路径。写入代价（每条约 44 µs）主要来自 redb
  的 ACID B+ 树插入，那是安全性的下限——任何写入优化都不会改变提交点或持久性。
- 因此**批量 `WRITE`** 在单个事务中约为每秒 2 万条（一次性的装载成本）。OPEN、
  读取和抗崩溃能力都不受影响。

---

## 可观测性日志（`--indexed-log`）

redb 引擎可以按文件写一份可选的事务日志（默认关闭），路径为
**`<assign-path>.log`**（例如 `customers.idx` → `customers.idx.log`），每次
`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` 记录一行，内容包括时间戳、记录数与字节数、
吞吐量、写入时键序的质量，以及在 `full` 级别下的 redb 索引页统计。

```bash
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

行格式为 `text`（logfmt）或 `json`（NDJSON，可直接对接 Grafana/Loki）。

**完整参考**——参数、字段表、格式、Grafana/Loki 流水线（Promtail + LogQL）以及
成本与安全说明——见 [`observability-cn.md`](observability-cn.md) §1。

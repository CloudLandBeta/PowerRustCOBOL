<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 基准测试

1.37.0 基线：运行时在负载下有多快，以及为达到这个速度它对分配器施加了多大压力。

```sh
cargo run --release -p cobolt-bench              # 全部
cargo run --release -p cobolt-bench -- dispatch  # 按子串只跑一个工作负载
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # 二十分之一，用于快速检查
```

`--release` 不是可选项。调试构建测量的是「没有优化」这件事本身，因此测试框架会在
表头中明确说明，而不是任由这些数字被引用。

## 测量了什么

每个 COBOL 工作负载都走**与交付的二进制文件相同的路径**——词法分析、语法分析、
语义分析、`Interpreter::run`——因为 `rcrun build` 生成的 `main.rs` 正是这样处理其
内嵌 AST 的。在同一进程内运行才使分配器计数成为可能：这些数字描述的正是你交付的
每一个二进制文件内部的那个解释器。

内存以分配行为的形式报告，而不是常驻集曲线。Rust 没有垃圾回收器，所以没有停顿可
测；负载之下真正要紧的是**周转量**——一个工作负载进入分配器多少次、有多少字节流经
它、以及峰值时有多少仍然存活。一个带计数功能的全局分配器
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) 在三个平台上
都能精确提供这三个数字，且无需任何外部性能分析器。

有两件事是刻意**不**测量的：进程启动时间和二进制文件大小。请在 `rcrun build` 产出
的真实构件上测量它们。

## 1.37.0 基线

Apple M3 Pro，18 GB，macOS 15.5，rustc 1.95.0，release 配置，2026-07-27。
绝对数值在不同机器之间难以迁移；**每次操作的分配次数**则迁移良好，是值得关注的
那一列。

| 工作负载 | Ops | 墙钟时间 | Ops/秒 | 分配次数 | 分配/op | 周转 MB | 峰值存活 MB |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1.049s | 5 721 961 | 24 000 334 | 4.00 | 72.5 | 0.0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0.729s | 686 318 | 9 000 356 | 18.00 | 409.6 | 0.0 |
| decimal COMPUTE | 500 000 compute | 0.824s | 606 461 | 10 000 499 | 20.00 | 41.0 | 0.0 |
| record batch (1000 rows, write+read) | 400 000 record | 2.179s | 183 612 | 26 023 007 | 65.06 | 227.9 | 0.8 |
| object churn (create/read/destroy) | 20 000 object | 0.092s | 216 320 | 1 100 000 | 55.00 | 27.5 | 0.0 |
| indexed redb (bulk insert) | 100 000 record | 0.710s | 140 922 | 65 854 | 0.66 | 188.9 | 22.4 |
| indexed redb (random read) | 50 000 read | 0.034s | 1 489 965 | 9 | 0.00 | 0.0 | 22.4 |

## 基线说明了什么

**瓶颈在分配器，而不在树遍历。** 每秒 570 万条语句是相当不错的分派速率——但达到
它付出了**600 万条语句对应 2400 万次分配**的代价。对两个 `COMP` 字段执行
`ADD 1 TO ACC` 本不该触及堆，却要走四趟分配器。这重新界定了优化方向：最先见效的
是值系统和操作数路径，而不是把树遍历解释器换成字节码虚拟机。虚拟机会让分派更
便宜，却会让每条语句四次分配原封不动。

**段落调用的开销高得不成比例。** 每次 `PERFORM <paragraph>` 有 18 次分配、约
820 字节，而每条内联语句只有 4 次。五十万次调用周转 410 MB。无论调用路径每次
调用时构造了什么，它都是表中密度最高的目标。

**字母数字记录按字段分配，符合预期。** 读写一行 4 个字段、每条记录 65 次分配，
这正是 `CobolValue::String` 为每个字段持有一个 `Vec<u8>`，再加上每次 `MOVE` 都
新建一个。改用内联短字符串表示，或直接切片到记录自己的缓冲区上，都会立刻在这里
体现出来。

**对象属性读取毫无必要地分配。** 24 次属性读取，每个对象 55 次分配。
`CoboltObject::get_property`、`get_str`、`get_bool` 和 `get_i64` 各自都调用
`name.to_ascii_uppercase()`——**每读一次**就分配并丢弃一个 `String`，仅仅是为了让
查找不区分大小写。用一个不区分大小写的键包装类型就能消除一整列。

**INDEXED 引擎不是问题所在。** redb 以每条记录 0.66 次分配的代价、每秒插入 14.1
万条记录，并以几乎零分配的方式提供每秒 150 万次随机读取。存储的余量远远领先于
供给它的解释器。

按预期收益排序，基线给出的优化顺序是：每条语句的分配，然后是段落调用路径，然后
是字母数字的 `CobolValue`，再然后是对象属性的大写转换。存储要排到远低于这些的
位置才会出现。

## 工作负载

| 工作负载 | 隔离出什么 |
|---|---|
| `dispatch (PERFORM VARYING)` | 树遍历开销：循环判断、自增、一条语句，底下的工作量最小 |
| `dispatch (PERFORM paragraph)` | 段落调用开销，与上面的内联情形对照 |
| `decimal COMPUTE` | `CobolNumeric` 的 i128 定标运算——COBOL 的金额计算 |
| `record batch` | 写入并回读带字母数字字段的 1000 行表格；批量负载下的值系统 |
| `object churn` | `ObjectRegistry` 的创建/读取/销毁——一个控件众多的 form 要付出的代价 |
| `indexed redb` | INDEXED 文件引擎：先批量插入，再按随机键读取 |

`indexed redb` 这两行，是对曾经带着 `#[ignore]` 标记躺在
`cobolt-runtime::indexed_redb` 内部的微基准 `open_table_cost` 的复原与推广。
它只有在有人记得那条确切的 `--ignored` 调用时才会运行，因此该引擎一直没有常设
基线；现在有了。它原本的结论予以保留——表句柄在整个写事务中只打开一次，实测比
每次插入都打开两次快约 16 %。

## 添加一个工作负载

在 [`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) 中添加
一个返回 `measure(name, unit, || { ...; ops_performed })` 的 `bench_*` 函数，并在
`main` 里将其注册到 `wanted(...)` 过滤器之后。计数器会自动包裹该闭包。返回的应是
*工作*单位数而非迭代次数，这样 `ops/sec` 和 `allocs/op` 才能在不同工作负载之间
保持可比。

请让新的工作负载保持确定性。随机读取探针使用固定的乘法步长而非随机数生成器，正是
出于这个原因：一个每次运行都重新洗牌的基准测试，无法与昨天的数字相比较。

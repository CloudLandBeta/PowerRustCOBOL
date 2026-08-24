<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# 基准测试

1.37.0 的基线：运行时在负载下有多快，以及为了达到这个速度，它有多依赖内存
分配器。

```sh
cargo run --release -p cobolt-bench              # everything
cargo run --release -p cobolt-bench -- dispatch  # one workload, by substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # a twentieth, for a quick check
```

`--release` 不是可选的。debug 构建测出来的是优化的*缺席*，测试框架会在表头里
直说这一点，而不是任由那些数字被引用。

## 测量的是什么

每一个 COBOL 工作负载走的都是**与交付出去的二进制文件相同的路径**——分词、
语法分析、语义分析、`Interpreter::run`——因为 `rcrun build` 生成的 `main.rs`
对其内嵌的 AST 做的正是这些。在同一进程内运行才使分配器计数成为可能：这些数字
描述的就是你交付的每个二进制文件里的那个解释器。

内存以分配行为的形式报告，而不是常驻集曲线。Rust 没有垃圾回收器，因此没有停顿
可测；负载下真正要紧的是**周转量**——一个工作负载进入分配器多少次、有多少字节
流经它、以及峰值时有多少仍然存活。一个带计数的全局分配器
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) 在三个平台
上都能精确给出这三项，无需任何外部性能分析器。

有两样东西是刻意**不**测的：进程启动时间和二进制体积。请在 `rcrun build`
产出的真实产物上测量它们。

## 1.37.0 基线

Apple M3 Pro、18 GB、macOS 15.5、rustc 1.95.0、release 配置、2026-07-27。
绝对数值跨机器可比性很差；**每次操作的分配次数**则很稳，是该盯住的那一列。

| 工作负载 | 操作数 | 耗时 | 操作/秒 | 分配次数 | 每操作分配 | 周转 MB | 峰值存活 MB |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1.049s | 5 721 961 | 24 000 334 | 4.00 | 72.5 | 0.0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0.729s | 686 318 | 9 000 356 | 18.00 | 409.6 | 0.0 |
| decimal COMPUTE | 500 000 compute | 0.824s | 606 461 | 10 000 499 | 20.00 | 41.0 | 0.0 |
| record batch (1000 rows, write+read) | 400 000 record | 2.179s | 183 612 | 26 023 007 | 65.06 | 227.9 | 0.8 |
| object churn (create/read/destroy) | 20 000 object | 0.092s | 216 320 | 1 100 000 | 55.00 | 27.5 | 0.0 |
| indexed redb (bulk insert) | 100 000 record | 0.710s | 140 922 | 65 854 | 0.66 | 188.9 | 22.4 |
| indexed redb (random read) | 50 000 read | 0.034s | 1 489 965 | 9 | 0.00 | 0.0 | 22.4 |

## 基线说明了什么

**瓶颈在分配器，不在树遍历。** 每秒 570 万条语句是相当体面的分派速率——但达到
它付出了**600 万条语句 2400 万次分配**的代价。两个 `COMP` 字段之间的
`ADD 1 TO ACC` 本不该碰堆，却要往分配器跑四趟。这重新框定了优化工作：最先见效的
地方在值系统和操作数路径，而不是把树遍历解释器换成字节码虚拟机。虚拟机会让分派
更便宜，却完全动不了每条语句四次分配这件事。

**段落调用贵得不成比例。** 每次 `PERFORM <paragraph>` 18 次分配、约 820 字节，
而每条内联语句只有 4 次。五十万次调用周转了 410 MB。无论调用路径每次到底在构造
什么，它都是表中密度最高的目标。

**字母数字记录按字段分配，符合预期。** 读写一行 4 字段的记录要 65 次分配，这是
`CobolValue::String` 为每个字段持有一个 `Vec<u8>`，外加每次 `MOVE` 都新建一个。
改成短字符串的内联表示，或者直接切片记录自身的缓冲区，都会立刻在这里显现出来。

**对象属性读取毫无必要地分配。** 24 次属性读取，每个对象 55 次分配。
`CoboltObject::get_property`、`get_str`、`get_bool` 和 `get_i64` 各自都调用了
`name.to_ascii_uppercase()`——**每读一次**就分配并丢弃一个 `String`，仅仅是为了
让查找不区分大小写。一个不区分大小写的键包装类型能把这一整列抹掉。

**INDEXED 引擎不是问题所在。** redb 以每条记录 0.66 次分配的代价，按每秒 14.1 万
条记录插入，并以几乎零分配的方式提供每秒 150 万次随机读取。存储的余量远超喂给它
的那个解释器。

按预期收益排序，基线给出的优化次序是：每条语句的分配，然后是段落调用路径，然后是
面向字母数字的 `CobolValue`，然后是对象属性的大写转换。存储要排到这些之后很远。

## 工作负载

| 工作负载 | 它隔离了什么 |
|---|---|
| `dispatch (PERFORM VARYING)` | 树遍历本身的开销：循环判断、自增、一条语句，底下的活儿极少 |
| `dispatch (PERFORM paragraph)` | 段落调用的开销，与上面的内联情形对照 |
| `decimal COMPUTE` | `CobolNumeric` 的 i128 定标算术——COBOL 的金额计算 |
| `record batch` | 写入并读回一张 1000 行、含字母数字字段的表；批量负载下的值系统 |
| `object churn` | `ObjectRegistry` 的创建／读取／销毁——一个控件众多的表单要花多少代价 |
| `indexed redb` | INDEXED 文件引擎：批量插入，然后按随机键读取 |

`indexed redb` 这两行，是把原本标着 `#[ignore]`、藏在
`cobolt-runtime::indexed_redb` 里的微基准 `open_table_cost` 恢复并一般化而来。
它只有在有人记得那个精确的 `--ignored` 调用时才会跑，因此这个引擎一直没有常设
基线；现在有了。它原来的结论保留了下来——表句柄在整个写事务里只打开一次，实测比
每次插入都打开两次快约 16 %。

## 添加一个工作负载

在 [`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) 中
添加一个返回 `measure(name, unit, || { ...; ops_performed })` 的 `bench_*`
函数，并在 `main` 里注册到 `wanted(...)` 过滤器之后。计数器会自动包住这个闭包。
返回的应当是*工作*单位的数量而不是迭代次数，这样 `ops/sec` 和 `allocs/op` 才能
在不同负载之间保持可比。

新的工作负载要保持确定性。随机读取探针使用固定的乘法步长而不是随机数生成器，
正是出于这个原因：一个每次运行都重新洗牌的基准，无法与昨天的数字相比。

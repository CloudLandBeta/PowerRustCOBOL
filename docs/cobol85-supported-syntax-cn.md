<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 已支持语法参考

**这份文档的用途：** 说明 RustCOBOL 究竟实现了 COBOL‑85 标准的多少 — 并且是拿
**NIST 官方 COBOL‑85 验证套件**去证明它，而不是空口断言。下面的
[记分板](#-一致性是测出来的不是宣称出来的--nist-ccvs85)就是结论；排在它之后
的一切都是那个数字背后的细节。

**关于 RustCOBOL 的 lexer/parser/runtime 今天实际接受什么的实况依据**，取自源码
（`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`），并与 `NIST/newcob.val,cbl`
对照核验。
请针对 ✅ 的写法编写测试；❌ 的写法要么解析失败，要么是空操作，而 ⚠️ 的写法能
解析但行为只是部分的。本文是
[`cobol85-verb-test-matrix-cn.md`](cobol85-verb-test-matrix-cn.md) 的配套文档：矩阵
说明*测什么*，本文说明 *RustCOBOL 认得哪种写法*。

图例：✅ 已支持 · ⚠️ 能解析但部分/简化 · ❌ 不认识（避免使用，或仅为确认差距而
测试）。

---

## ★ 一致性是测出来的，不是宣称出来的 — NIST CCVS85

**这就是本文档的要点。** 下面的每一条论断都拿 **NIST 官方 COBOL‑85 验证套件**核
对过 — CCVS85 版本 4.0（01 OCT 1992，COBOL 85 版本 4.2，Apr 1993 SSVG），也就是
美国 National Institute of Standards and Technology 用来认证 COBOL 编译器的那套
套件。它有 28 MB、348,271 行、**459 个 COBOL 程序**和 51 个 copybook 成员，就放在
本仓库的 `NIST/newcob.val,cbl`。

它是事实来源。凡是 RustCOBOL 与 CCVS85 意见不一致的地方，**CCVS85 是对的，
RustCOBOL 是错的**，差距会作为缺陷记录在
[`specs/nist/`](../specs/nist/README.md) — 每个修复一份规格，并点名列出失败的
程序。

### 记分板

在 2026‑08‑28 于版本 1.62.43 测得，针对未经改动的发行包：

| | 程序数 | 占比 | 含义 |
|---|---:|---:|---|
| ✅ **PASS** | **422** | **97.2 %** | 占范围内 434 个程序 |
| ❌ **FAIL** | **12** | 2.8 % | 占范围内 434 个程序 |
| ⬜ **N/A** | **25** | — | 在 RustCOBOL 范围之外的模块（见下） |
| | **459** | | 套件中的程序总数 |

复现方法：

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ 能编译是较弱的那个论断

上面的表数的是**前端接受**的程序。它没有说这些程序能跑。套件会给自己打分 — 每个
CCVS85 程序都会打印自己的 `PASS` / `FAIL*` 报告 — 所以还有第二个、严格更强的
数字：有多少个跑到结束并报告**零失败**。

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

两个数字都按模块分别报告，永远不混为一谈：

| 模块 | 编译 | 执行（0 失败） |
|---|---:|---:|
| **NC（Nucleus，核心）** | **95 / 95** | **83 / 95** |

工作**一次只推进一个模块**：只有当两个数字都到 95，NC 才算完成，在那之前不动任何
别的模块。一个横跨十个模块的宽泛编译分数，并不能说明其中任何一个能不能用。

##### 需要的不止一个打印文件的那五个 NC 成员 — 全部已计分

执行分数只在程序**自己的 CCVS 报告**没有失败时才算这个程序干净。有五个 NC 成员不
打印这样的报告，而且并不是因为哪里坏了。它们各自需要的是测试框架上的工作，而不是
编译器上的工作，如今每一个都已计分：

| 成员 | 它需要什么 | 如何计分 |
|---|---|---|
| **NC302M**、**NC303M**、**NC401M** | *标记（flagging）*类测试。它们完全不带 `PASS`/`FAIL` 机制 — 每个都以 `TOTAL NUMBER OF FLAGS EXPECTED = n` 结束，被验证的结果是**编译器针对废弃构造（NC302M/NC303M）或针对高子集之上的构造（NC401M）发出的诊断**集合。 | 测试框架把诊断与该成员自带的期望清单逐行比对。两类分成**两趟单独的执行**：`DATE-COMPILED` 既是废弃的*又*在高子集之上，所以合成一趟跑会让每个成员把另一类的标记当成误报收进来。 |
| **NC110M** | 用 `DISPLAY` 把报告写到操作员控制台，而不是写到测试框架读取的 CCVS 打印文件。 | 子进程的控制台输出被捕获到一个文件，从那里计分。 |
| **NC109M**、**NC204M** | 测试从操作员读入的 Format 1 `ACCEPT` — NC109M 直接裸写，NC204M 通过 `SPECIAL-NAMES` 关联到输入设备的助记名。验证者本应提供输入；没有 stdin 时每一次比较都会失败。 | 测试框架在子进程的 stdin 上提供一副操作员输入牌。这副牌是**从源码里还原出来的，不是编出来的**：每个被接收的项都会与一个配对项比较，而该配对项的值由程序在 `ACCEPT` 正上方设定，所以牌中的每一行就是那个值。 |

因此在执行这一轴上**并不存在低于 95 的结构性天花板**：范围内的每个 NC 程序都能
编译，而且每一个都按它自己报告的结果计分。

**已经**了结的可比案例是外部开关。NC174A、NC253A 和 NC254A 针对操作员在运行前设定
的开关测试 `ON STATUS` / `OFF STATUS` — COBOL 内部无法设定这种开关 — 所以测试框架
现在按 CCVS85 运行说明的要求，原样传入
`--switch XXXXX051=ON --switch XXXXX052=OFF`（以及替换写法 `SWITCH-1` /
`SWITCH-2`）。这是验证流程本身要求的配置，不是在秤上按手指：没有声明任何开关的
程序不受影响。

#### ⚠️ PASS 究竟意味着什么 — 引用这个数字之前先读这段

一个程序算作 **PASS**，是指它用 `--source-format=fixed` 走完 RustCOBOL 的前端 —
lexer、parser、语义分析器 — 而且**零错误**。

那是*编译*层面的一致性。它**不是**程序算出正确答案的证明。CCVS85 程序运行时也会
打印自己的 `PASS`/`FAIL` 统计，给那份输出计分是这项工作的**下一阶段** — 它不包含
在那 332 之内 — 见下面的执行记分板。两个实测案例说明了这个区分为什么重要：

- 35 个 RELATIVE 文件程序中曾有 30 个编译得干干净净，而那时的 runtime **根本没有
  RELATIVE 引擎** — 它们照跑不误，然后悄悄产生错误结果。这个缺口已经**补上**：
  引擎在 1.62.76 落地，模块在 1.62.77 完成（编译 35 / 35，执行 34 / 34）。它被留在
  这里，因为它最清楚地说明了只看编译这条轴所无法告诉你的东西。
- 跨两行续写的 literal 可能被错误地重新拼接，却依然能通过解析，让程序捧着错误的
  数据继续跑。

所以：**PASS = “RustCOBOL 接受这个程序里的每一种构造。”** 目前仅此而已。

#### 🔴 执行记分板 — 那个意味着“真的能用”的数字

以上一切衡量的都是**编译**。CCVS85 程序还会*运行*并打印自己的 `PASS`/`FAIL`
统计，而这份统计正是这套套件存在的目的。自 1.62.15 起测试框架会真的运行它们：

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

在 2026‑08‑28 于 1.62.43 测得。按照黄金法则 #9，一个模块完成之后才开始下一个：
**NC（Nucleus，核心）在两个轴上都已完成**，因此 **SQ（顺序 I/O）**是正在推进的
模块。

**NC — Nucleus（核心）**

| | 程序数 |
|---|---:|
| 范围内 | 95 |
| 没能编译 | 0 |
| 跑到结束 | 95 |
| **…其中报告 0 失败** | **95** |
| …报告了失败 | 0 |
| 跑了但没打印报告 | 0 |
| 超时（>20 s） | 0 |
| 崩溃或被 runtime 拒绝 | 0 |

程序自己报告的断言：**4 614 PASS / 0 FAIL**，占已计分 4 614 条的 100 %。（另有
5 条为 `DELETED` — 这是 CCVS 自己给程序主动跳过的测试所用的标记。）

作为对照，同一张表在 1.62.23 时是 95 个中 65 个干净、4 278 PASS / 226 FAIL。被
合上的正是“能编译”与“真的能用”之间的这道口子。

**SQ — 顺序 I/O（推进中）**

| | 程序数 |
|---|---:|
| 范围内 | 85 |
| 没能编译 | 0 |
| 跑到结束 | 83 |
| **…其中报告 0 失败** | **84** |
| …报告了失败 | 1 |
| 跑了但没打印报告 | 0 |
| 超时（>20 s） | 0 |
| 输出失控（>2 MB） | 0 |
| 崩溃或被 runtime 拒绝 | 0 |

断言：**623 PASS / 1 FAIL**，占已计分 624 条的 99.8 %，而且**每个程序都跑到
结束**。在 1.62.42 时同一张表是 85 个中 **10** 个干净、20 个崩溃、1 个超时，
215 PASS / 190 FAIL — 那一簇崩溃其实是一个缺陷，声明节的段落丢了名字；到 1.62.43
时是 44 个干净、471 PASS / 162 FAIL。变长记录、共享记录区、`FILLER` 宽度、
`READ … INTO` 和顺序 `REWRITE` 落在 1.62.44；按模式限定的 `USE`、
`CLOSE REEL/UNIT`、`SELECT OPTIONAL`、`OPEN` 时的 `LINAGE-COUNTER` 以及越界的记录
长度落在 1.62.45；用数据名给出的 `LINAGE` 值和顺序 I/O 的标记检测器落在 1.62.46。

还有一个成员没到位：

| 成员 | 还差什么 |
|---|---|
| SQ203A | 需要 `XXXXD001`，一个由 CCVS85 **安装过程**提供的数据文件。套件里没有任何成员会写出它，所以它那个 `SELECT OPTIONAL` 测试中“文件存在”的那一半在这里跑不了；“文件不存在”的那一半通过。这是缺失的安装输入，不是 RustCOBOL 的缺陷。 |

> `FAIL*` 明细行是**故意**写两遍的 — CCVS 的 `PRINT-DETAIL` 会执行
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — 而 `PASS ` 只写一遍。任何从
> 打印文件里直接数标记得到的原始计数，都必须先把失败数除以二，才有意义。

要读懂一个程序*为什么*失败，第三趟会打印它自己报告中携带的失败明细，方便按整个
模块归类：

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> 这就是为什么编译数字总是被表述为“RustCOBOL **接受**这些构造”。把它当作一致性
> 水平来引用是错的。

#### 按模块

| 模块 | 它测什么 | PASS / 总数 | |
|---|---|---:|---|
| NC | Nucleus（核心） | **95 / 95** | ✅ 完成 — 而且在**执行**上也完成了（见上面的记分板） |
| SQ | 顺序 I/O | **85 / 85** | ✅ 编译已完成；**执行 44 / 85** — 正在推进的模块 |
| IC | 程序间通信 | 45 / 47 | `END-CALL` 没有被它的 `CALL` 吃掉，反而一路走到了语句分派器；一个带下标的条件名 |
| IF | 内部函数 | **45 / 45** | ✅ 完成 |
| IX | 索引 I/O | **42 / 42** | ✅ 完成 |
| SG | 分段 | **13 / 13** | ✅ 完成 |
| ST | 排序 / 归并 | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | 相对 I/O | 35 / 35 | ✅ **两个轴上都已完成**（1.62.77）— 执行 34 / 34，354 条断言，0 个失败。一个真正的引擎（`cobolt-runtime/src/relative.rs`，`PRCREL1` 容器）在 1.62.76 落地；全部七个文件动词都会基于 `FileOrganization::Relative` 进行分派。RL301M 按照与 IX301M 相同的裁定被排除在执行之外，但它仍计入编译统计，并且在那里是通过的 |
| SM | 源文本操作（COPY/REPLACE） | 14 / 17 | 数据名里的一个 `$`；带限定/带下标的伪文本；`PERFORM … VARYING` 的一种写法 |
| DB | 调试 | 11 / 15 | `GO-TO` 被当作用户自定义字使用，与关键字对 `GO TO` 冲突；有一个程序用了通信动词 `DISABLE` |
| **范围内** | | **422 / 434** | |
| CM | 通信 | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | 废弃特性标记 | — | ⬜ N/A |
| EXEC85 | NIST 自带的 COBOL 驱动程序 | — | ⬜ N/A |

### ⬜ N/A — 哪些在 RustCOBOL 范围之外，以及为什么

这 25 个程序**不计为失败**。它们是 RustCOBOL 没有实现、也不打算实现的特性。完整
理由见
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md)。

| 模块 | 程序数 | 为什么在范围之外 |
|---|---:|---|
| **CM** — 通信 | 9 | `COMMUNICATION SECTION`、`CD` 条目、`SEND` / `RECEIVE` / `ENABLE` / `DISABLE`。它面向 1980 年代的远程处理监控器 — 由事务管理器持有的消息队列。这里没有那样的 runtime，而且该模块已从后来的 COBOL 标准中删除。 |
| **RW** — Report Writer | 6 | `REPORT SECTION`、`RD` 条目、`INITIATE` / `GENERATE` / `TERMINATE`、控制断点。这是一门庞大的声明式子语言；PowerRustCOBOL 对报表的答案是表单设计器和 PDF 导出。如果想要，日后它可以变成一项*功能* — 这是唯一一个对用户有真实价值的排除项。 |
| **OBSQ / OBIC / OBNC** | 9 | 它们重新测试前面的模块，并期望编译器*标记出* COBOL‑85 的废弃元素。它们的语言内容已被范围内的规格覆盖；在范围之外的是废弃特性的**标记**。 |
| **EXEC85** | 1 | 它不是测试。它是 NIST 自己的 COBOL 执行程序，负责切分发行包并驱动整套套件 — 这里已由一个 Rust 测试框架取代，所以它不需要能编译。 |

**面向对象 COBOL** 同样在 RustCOBOL 的范围之外，不过 CCVS85 完全早于它 — 套件里
没有任何 OO 程序。

### 剩下的 192 个失败来自哪里

每一个都是有规格的缺陷，不是未知数。按它作为*首个*错误出现的程序数排序：

| 程序数 | 根因 | 规格 |
|---:|---|---|
| 31 | 分隔用逗号 — `MOVE ZERO TO A, B, C` | [分隔符](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [内部函数](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [语句缺口](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | 用空格分隔的下标 — `TBL (1  2)` | [分隔符](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON`（开关名）以及 `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md)、[分隔符](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [语句缺口](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | 写在 B 区靠后位置或跨行拆开的 `COPY` | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | 分隔用分号 — `START F ; INVALID KEY` | [分隔符](../specs/nist/NIST-spec-separators.md) |
| 4 | 写在下一行的 `OCCURS` 整数 | [分隔符](../specs/nist/NIST-spec-separators.md) |
| 4 | 带优先级编号的 `SECTION` — `SORT-PARA SECTION 69.` | [分段](../specs/nist/NIST-spec-segmentation.md) |

> **每修一次，这个排名就会变动，而变动本身很有信息量。** 在早先版本里领跑这张表
> 的三行已经消失了 — IDENTIFICATION 的注释条目、数值 literal，以及那个落单的
> 引号。每一次，被清空那一行里的大多数程序**并没有**开始通过；它们只是挪到了下面
> 一行。1.62.12 释放出来的那四个 SG 程序，如今停在 `SORT-PARA SECTION 69.` 上，
> 这就是分段依然显示 0 / 13 的原因。请重新测量，而不要信赖以前的排名。

### 一致性历史

| 版本 | PASS / 434 | 改了什么 |
|---|---:|---|
| 1.62.7 | **0** | 什么都编译不了。经典参考格式的两条规则缺失：第 73‑80 列被当成源码读取，续行从未被拼接。 |
| 1.62.8 | **222** | `--source-format=fixed` — 经典参考格式，包括续行。见[源码格式](#源码格式)。 |
| 1.62.10 | **237** | 数值 literal 可以以小数点开头（`.999`）。内部函数 21 → 29，Nucleus 25 → 29，排序/归并 27 → 30。 |
| 1.62.11 | 241 | IDENTIFICATION 的注释条目段落。调试 5 → 9。收益比那个 32 个程序的桶所暗示的要小：其中 9 个是通信程序（N/A），其余大多数紧接着就撞上了第二个拦路石。 |
| 1.62.12 | 242 | literal 被限制在自己那一行内，所以一个落单的引号再也无法改变整个文件的奇偶配对。Nucleus 29 → 30。那个 6 个程序的桶清空了：4 个前进到段优先级编号，1 个现在通过。 |
| 1.62.13 | 292 | 分隔用的逗号和分号是标点，不是词法单元；下标可以只用空格分隔；下标可以跟在完整的限定名之后；literal 内部成对的定界符算作一个字符。Nucleus 30 → 56，程序间 32 → 44，索引 31 → 38。三整桶诊断被清空。 |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — 把整张表作为内部函数的实参；`MOVE ALL "X"` 填满字段；`CLOSE … WITH LOCK` / `NO REWIND` / `REEL`；带符号 literal 作为 `WHEN` 的对象；`PERFORM … TIMES` 用数据项给出次数；把整数次数写在续行上。**内部函数 45 / 45 — 模块完成。** |
| 1.62.15 | 332 | 未知的 `FUNCTION` 名是编译错误，而不是返回 0；用户自定义字可以以数字开头（`25COUNT`、`3-DEM-TBL`、`0 SECTION.`）；除非有 `WITH DEBUGGING MODE`，否则 `D` 行是注释。分段 0 → 10，Nucleus 58 → 61。 |
| 1.62.16 | 376 | `AT END` 里的 `AT` 是可选的，所以一个光秃秃的 `END` 短语不会再吞掉下一个段落首行（33 个程序）。COPY/REPLACE 预处理器把 literal 限制在自己那一行内，所以版权横幅里的 COPY 一词不是指令。数值 literal 可以用它的小数点开启 `ADD`/`SUBTRACT` 的操作数列表。**索引 I/O 完成，42 / 42。** |
| 1.62.17 | 380 | `LINAGE` 页面布局、`LINAGE-COUNTER`，以及 `WRITE … AT END-OF-PAGE` / `AT EOP` — 是实现，不是占位。顺序 I/O 77 → 81。 |
| **1.62.19** | **396** | numeric-edited 项就是数值项。编辑用的小数点会保留跟在它后面的数字（`PIC ZZ,ZZZ.9` 不再被截成 `ZZ,ZZZ`），而只由编辑字符组成的 picture — `ZZZZ`、`$.**`、`$**.**CR` — 是 numeric-edited 而非字母数字。这两点都会让一个合法的算术 `GIVING` 接收方看起来不是数值。 |
| **1.62.18** | **391** | 在期待表达式的位置上，开启续行的数字是一个操作数。类条件或符号条件中的 `IS` 是可选的，条件也可以充当 `EVALUATE` 的主语。过程名可以完全用数字书写，引用处和首行处都一样。 |
| **1.62.21** | **417** | Nucleus 那一趟。`ALTER` 是一个系列，而 `GO TO.` 就是被改写的那个 GO TO；全数字的过程名保留前导零；条件名可以带下标或加限定；带括号的算术表达式是操作数，不是嵌套条件；`MULTIPLY`/`DIVIDE` 格式 1 接受一串接收方；`WITH TEST` 可以放在 `VARYING` 之前，重复次数可以带下标；`PERFORM 命令式语句 … END-PERFORM` 不需要任何短语；段落名可以由它所属的节来限定；`ELSE` 不会被 `ON SIZE ERROR` 的命令式语句或嵌套的 ELSE 分支吞掉；缩略的组合关系接受算术以及类/符号对象；`INSPECT` 会把 ALL/LEADING 的类别带到后续操作数，`CONVERTING` 接受一个区域；`UNSTRING TALLYING` 跟在 `WITH POINTER` 之后。**Nucleus 编译 95 个中 76 → 92，干净执行 16 → 28。** |
| **1.62.43** | **422** | **顺序 I/O 模块完全能编译了 — 85 个中的 85 个 — 执行从 85 个中的 10 个升到 44 个。** 声明节的段落保住了名字，于是 `USE` 处理程序可以对它们 `PERFORM` 和 `GO TO`（20 个程序不再崩溃）；声明为两字符*组*项的 `FILE STATUS` 项能收到状态码；对已打开文件再 `OPEN` 得到 `41`，并且不会重新打开它；`AT END` 之后的顺序 `READ` 得到 `46`；而且同一个 `OPEN` 可以带多个模式组（`OPEN INPUT f1 OUTPUT f2`），编译方面的收益全部来自这一条。 |
| **1.62.42** | **420** | **Nucleus 模块完成了 — 95 个中的 95 个能编译，*并且* 95 个中的 95 个干净执行，4 614 条断言无一失败。** `66 RENAMES` 由它所在的记录来限定，覆盖它所跨表的每一次出现，并且当它恰好只重命名一项时它就是那一项；声明在组上的 88 检验的是该组的字节；具象常量按另一个操作数定尺寸，`VALUE` 也包括在内；组操作数的类别是字母数字；缩略式对象前面的 `NOT` 否定该关系；一串 `INSPECT … REPLACING` 共享一次扫描，带符号的 DISPLAY 项的字符中不含 `-`；`REDEFINES` 的覆盖可以嵌套；并且 `PERFORM … WITH TEST AFTER VARYING` 被遵守，`AFTER` 变量在它的循环结束时被重置，带下标的 `VARYING` 标识符跟随它的下标。NC201A 能跑完，靠的正是最后这一组。 |

> **诚实的小结。** RustCOBOL 今天接受范围内 NIST 套件的 **97.2 %**，而九个版本前
> 还是一个都没有。剩下的 12 个并不神秘 — 它们是被命名的缺陷，每一个都连同它所
> 阻塞的程序一起写进了规格。这张表就是进展的度量，每次发布都会更新。
>
> **而且有一个模块已经在真正要紧的那一轴上完成了。** Nucleus 是 95 个中的 95 个
> 干净跑完，而不只是通过编译 — 见上面的执行记分板。按照黄金法则 #9，那就是开始
> 下一个模块的门槛，所以**顺序 I/O 现已推进中**：编译完成，执行 85 个中的 44 个。

---

> **更新（缺口实现批次）：** 以下内容已实现，现在是 ✅ — **引用修饰**
> `id(start:len)`、**内联 `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING 的 `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、**能区分
> 类别的 `INITIALIZE`**、**以运算符开头的缩略条件**（`a > 1 AND < 9`）、
> **`CALL … ON EXCEPTION`**（在 CALL 无法解析时运行）、**`COMPUTE` 多接收方 +
> 逐接收方 `ROUNDED`**，以及一个大得多的**内部函数**集合。
>
> **更新（层次化 / 感知出现次数的环境批次 — 1.5.0）：** 四项曾被数据模型卡住的
> 特性现在是 ✅ — **运行时表下标** `t(i)` / `t(i, j)`（按出现分配存储）、
> **限定名消歧** `id OF/IN group`（重名的叶项解析到各自独立的存储）、
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**，以及**可用的 `SEARCH` / `SEARCH ALL`**。
>
> **更新（动词完备性批次 — 1.6.0）：** 现在还有这些是 ✅ — `ADD`/`SUBTRACT` 上的
> **多接收方 `MULTIPLY`/`DIVIDE GIVING` + 逐接收方 `ROUNDED`**；
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** 以及修正后的裸
> `EXIT`；**`CALL … NOT ON EXCEPTION`**；合并的
> **`INSPECT … TALLYING … REPLACING`** 与 **`BEFORE/AFTER INITIAL`** 区域；
> 日期/金融**内部函数**（`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、
> `INTEGER-OF-DAY`、`DAY-OF-INTEGER`、`ANNUITY`、`FRACTION-PART`）；**以 literal
> 为对象的缩略条件**（`A = 1 OR 2 OR 3`）；**`EVALUATE … ALSO`**（多主语）与
> **`WHEN NOT`**；**真正的 88 级条件名**（`SET … TO TRUE/FALSE`，宿主项按它的
> VALUE/取值范围受检）；**`PERFORM para VARYING`**；以及一个可用的
> **`SORT`/`MERGE`** runtime（`RELEASE`/`RETURN`、`USING`/`GIVING`、
> `INPUT`/`OUTPUT PROCEDURE`）。文末那份“避免使用”清单是最新的。
>
> **更新（清空“避免使用”清单的批次 — 1.7.0）：** 剩下的缺口现已实现 — **以标识符
> 为对象的缩略式**（`a = b OR c`，借助 88 级元数据解析）；
> **`INITIALIZE … REPLACING category DATA BY value`**；**`66 RENAMES`**（读取时
> 合成 / 写入时分发到被覆盖的各项）；**指针**（`USAGE POINTER`、
> `SET ptr TO ADDRESS OF x / NULL`、`SET ADDRESS OF item TO …` 的别名、
> `IF ptr = NULL`）；**`ALTER`** / **`UNLOCK`**；忠实的 **`NEXT SENTENCE`**；
> 余下的标准**内部函数**（`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、
> `NUMVAL-F`、`TEST-NUMVAL`）；以及扩展的**屏幕 `ACCEPT`/`DISPLAY`**（CLI 模式下
> 经由 ANSI 的 `AT`/`WITH` — 现在是真的*执行*，不只是解析）。
>
> **更新（1.7.1）：** `ACCEPT` 的寄存器来源现在真的可用了（之前是被识别的空操作） —
> **`FROM COMMAND-LINE`**、**`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`**（与
> `DISPLAY n UPON ARGUMENT-NUMBER` 配对）、**`ENVIRONMENT-VALUE`**（与
> `DISPLAY "name" UPON ENVIRONMENT-NAME` 配对）、**`ESCAPE KEY`** → `"00"`、
> **`CRT STATUS`** → `"0000"`。
>
> **更新（1.7.2）：** 文件共享 / 加锁短语与 `CANCEL`（之前是 ❌ / 空操作） —
> **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`**（释放该文件的 INDEXED 记录锁），以及 **`CANCEL program`**（重新
> 初始化该程序的存储）。
>
> **更新（1.8.0）：** **`COMMIT` / `ROLLBACK`** 现在是真正的 COBOL 动词了 — 对已
> 打开的 INDEXED 文件执行由程序控制的事务（内存引擎和磁盘引擎都支持）。磁盘引擎
> 有了真正的运行内撤销日志（此前是空操作）。文末那份“避免使用”清单是最新的。

---

## IDENTIFICATION DIVISION 的段落

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ **注释条目**段落 — `AUTHOR`、`INSTALLATION`、`DATE‑WRITTEN`、
  `DATE‑COMPILED`、`SECURITY` — 可以按**任意顺序、任意子集**出现。
- ✅ `REMARKS` 也被接受。它在 1985 年从 COBOL 中删除，因此不会被保存；接受它只是
  为了让从 COBOL‑74 沿用下来的源码仍然能编译。

**注释条目**就是自由文本，而 COBOL‑85 是照字面这么规定的：

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- 它可以包含**保留字** — 上面那个 `DATA` 并不会开启一个 DATA DIVISION。
- 它可以包含**句点**，而且不会在句点处结束。
- 你写多少行，它就**跨多少行**。
- 它在 A 区中**位于行首**的下一个段落标题或部标题处结束 — 上面的条目就是这样在
  `DATE-WRITTEN` 处结束的。

**这段文字里的引号被限制在它所在的那一行内**（自 1.62.12 起）。像
`THE COMPILER"S ABILITY` 这样的文本不再开启一个一直延伸到程序其余部分的字面量 —
参见[源码格式](#源码格式)。在注释条目里仍然值得避免不成对的引号，但现在它
只让你损失那一行，而不是整个文件。

⚠️ 在这里，`INSTALLATION`、`SECURITY` 和 `REMARKS` **不是保留字**。它们只在
IDENTIFICATION DIVISION 内部才被识别为段落名，所以名为 `SECURITY` 的数据项照常
可用。

---

## 源码格式

RustCOBOL 读取三种源码布局。这个选择是显式的 — **绝不**从文件内容去猜，因为把列
规则套用到并非为它们而写的源码上，会悄无声息地删掉代码。

| `--source-format` | 含义 |
|---|---|
| `free` | 完全没有列规则。`*>` 开始一条注释。**默认值**，也是 PowerRustCOBOL 自己的项目以及生成的窗体 `.cbl` 文件所使用的格式。 |
| `fixed` | ✅ **经典 COBOL-85 参考格式** — 标准所定义、卡片映像源码所采用的布局。见下文。 |
| `fixed-relaxed` | 顺序区和指示列仍然有效，但一行你写到哪里就到哪里 — 没有 72 列的限制。 |
| `auto` | 历史行为：`free`，除非 `COBOLT_FIXED=1`。 |

`COBOLT_SOURCE_FORMAT` 设置一次会话的默认值。

### `fixed` — 经典参考格式

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **第 1-6 列** — 顺序号区，忽略。
- **第 7 列** — 指示区：
  - `*` 或 `/` → 注释行
  - `-` → 上一行的**续行**
  - `D` → 调试行；当作注释（调试模式尚未实现）
  - 其他任何字符 → 按普通源码读取。标准保留了这一列，但卡片映像测试套件把它当作
    可选行的选择符使用，悄悄丢弃那些行就等于删代码。
- **第 8-72 列** — 源码本体。
- **第 73-80 列** — 标识区，**丢弃**。

### 续行 ✅

第 7 列上的连字符会续上一行。

**续接一个单词或数字字面量** — 被续接那一行的尾部空格被丢弃，两半之间不留任何
东西直接相接：

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

它声明了一个名为 `WRK-DS-18V00-CONTINUED` 的数据项。

**续接一个字母数字字面量** — 被续接那一行的字面量没有收尾引号；续行必须用一个
引号重新开启，字面量从它后面的那个字符继续：

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **被续接的片段一直延伸到第 72 列，尾部空格也算在内。** 一行即使没写满到第 72
列，那些空格仍然会计入字面量。这就是为什么只有在 `fixed` 下续接的字面量才是逐字节
精确的；其他格式没有第 72 列可停。

### 字面量绝不会意外跨行 ✅

续行是字面量跨越多行的**唯一**方式。没有在自己那一行内闭合的引号是一个错误，并在
它被写下的位置报告：

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

这件事比听上去更要紧。在 1.62.12 之前，一个不成对的引号会一直跑到文件中任何位置的
*下一个*引号，于是注释里一个走失的 `"` 就吞掉整整几个部，并把其后每一个引号的配对
全部错位 — 发现这个问题的那些 NIST 程序里引号的个数是**偶数**，所以没有任何东西是
未终结的；一个字符就改变了整个文件的奇偶配对。现在损害到换行符为止。

> **自由格式没有字面量续行。** 不是 `&` — 那是拼接*运算符* — 也不是围栏块。自由
> 格式的字面量必须放得下在一行里；如果很长，就拼接：
> `"first part" & "second part"`。

> **注意。** 给一个按自由格式写成的文件选择 `fixed` 会毁掉它 — 第 72 列之后的一切
> 都会消失，而第 8 列之前的文本会被当成顺序号读取。只对真正是卡片映像的源码使用它。

---

## 已识别的语句（动词）

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2`（改写 para-1 的 `GO TO` 目标）·
`UNLOCK file`（释放该文件的记录锁）· `OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK`（文件共享/加锁 — 在单一运行单元内属于建议性的）
✅ `COMMIT` / `ROLLBACK`（由程序控制的 INDEXED 文件事务 — 参见文件动词一节）·
`CANCEL`（重新初始化程序的存储）·
⚠️ `INVOKE`（能解析，但不做任何事）
项目扩展：`EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。一个块可以
`use` 那些始终被链接的 crate（std、egui、eframe 以及已链接的运行时集合），**外加
项目在 Project's Crates 中登记的任何 crate**（规格 044）：登记的 crate 会被固定到
一个确切的版本，以 vendoring 的方式收进项目的 `crates/` 里，并编译进二进制；未登记的 crate 会在
开发者所在的那一行让 Check/Build 失败，并指出补救办法。

✅ `SEARCH`（顺序查找）/ `SEARCH ALL`（对带 `ASCENDING`/`DESCENDING KEY` 的表做
二分查找 — 执行第一个匹配的 `WHEN`，否则走 `AT END`）。
✅ `SORT` / `MERGE` 配合 `RELEASE` / `RETURN`（可用 — 见下文）。
✅ `DECLARATIVES … END DECLARATIVES` 搭配 `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — 在出现未处理的错误 `FILE STATUS` 时触发
的文件错误处理程序。处理程序**从它所在节的开头进入，一直执行到该节结束**，而且它的
各个段落保留自己的名字，所以它可以对这些段落做 `PERFORM` 和 `GO TO` — 包括*另一个*
声明节里的段落。声明段落有自己的名字空间：控制流绝不会从主体落入其中；一个在两边
都声明过的名字，在处理程序运行期间解析为声明部分的那一份，在其他任何地方则解析为
主体的那一份。声明部分也可以 `PERFORM` 非声明部分的某个段落。
❌ **不识别 — 请勿使用：** `ENTRY`、
`GENERATE`/`INITIATE`/`TERMINATE`、`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 逐动词支持的形式

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]`（多个接收方）。
- ✅ **只要有一个操作数是组项，整个传送就是字母数字传送**（COBOL-85 6.18.4）。
  另一个操作数的 PICTURE 只贡献它的*大小*，别的什么都不贡献：不编辑、不去编辑、
  不做数值转换。`MOVE <group holding "123ABC">` 会在 `PIC 0XXXXX0` 中留下
  `"123ABC "`（不是编辑后的 `"0123AB0"`），在 `PIC 9999V999` 中留下同样的六个
  字符加一个空格，在 `PIC 99` 中留下 `"12"`。
  哪一端补齐、哪一端被丢掉，仍由 `JUSTIFIED RIGHT` 决定。
  同一条规则也管着组项自己的字节：每个子项原样取走自己那一段，
  所以字母数字编辑的子项**不会**被重新编辑。
- ✅ **写在组项上的 `VALUE` 子句**初始化该组项的字节，并分配到它的各个子项上 —
  `01 G VALUE "$123.45". 02 E PIC $999.99.`
  会让 `E` 里放着 `"$123.45"`。
- ✅ `MOVE CORRESPONDING g1 TO g2` — 传送两个组项按名字共有的每一个从属项，
  并递归进入名字相符的下级组项。
- ✅ **`CORRESPONDING` 排除以 `REDEFINES` 或 `RENAMES` 描述的项**
  （COBOL-85 6.18.4 GR1），两边都一样，连同从属于它的一切一起排除。
  排除针对的是*声明*，不是名字：一个普通的项，即使只是跟别处的某个 66 级同名，
  照样参与对应。
- ✅ **`CORRESPONDING` 的任一操作数都可以指名组项表的某一次出现** —
  `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` 写入那一次出现自己的存储位，
  而下标会一路带进递归。
- ✅ **一对项里只要有一个是基本项就够了。** 组项可以面对一个基本项，
  两者之间的传送就是一次字母数字传送：一个基本项 `PIC XXX` 发往一个由 `999` + `XXX`
  组成的组项，会填满它的六个字符；一个由 `XXX` + `99` 组成的组项发往一个普通的 `X(5)`，
  也会把它填满。两个组项面对面时仍然**递归** — 那种配对不属于基本项的情形。
  *（1.62.39 之前两个方向都什么也没传：组项不拥有存储位，于是写入去了没人读回的地方，
  而读取得到的是空串。）*
- ✅ **引用修改 `id(start:len)`** — 既可作发送方（取子串），也可作接收方
  （拼接式的部分赋值）；对每个动词的操作数都有效。`length` 可省略。
  它寻址的是**字符位置**，所以数值操作数按其 `PIC` 的完整宽度连同前导零一起取用：
  `01 T PIC 9(8) VALUE 00224845` 得到的 `T(1:2)` 是 `"00"`，不是 `"22"`。
- ✅ **组项是字母数字的聚合体** — 一个组项*就是*它那些从属项首尾相接排在一起，
  它的大小是各从属项大小之和。读一个组项会把子项（包括 `FILLER`）连接起来；
  往一个组项里传送则按宽度把字节分配到各子项上。`MOVE 11 TO A` 透过包含 `A` 的
  组项就能看见，而 `MOVE "1234" TO G` 设置的是 `G` 的各个子项，不是 `G` 自己的某个存储位。
- ✅ 下标 `t(i)`、`t(i, j)` — 读/写每一次出现对应的存储位；
  可变下标 `t(WS-I)` 在每次访问时求值。
- ✅ 限定 `id OF/IN group`（`… OF g1 OF g2`）— 即使叶子名字在不止一个组项下被声明过，
  也能解析到正确的那一项。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **逐接收方的 `ROUNDED`** — 每个接收方带着自己的 `ROUNDED` 标志。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — 把每一对名字相符的数值项合并起来，
  并递归进入名字相符的下级组项。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **多个 `GIVING` 接收方**，每个都有自己的 `ROUNDED`。
- ⚠️ `DIVIDE a BY b`（没有 `GIVING`）把 `a/b` 存回 `a`（这是 PowerRustCOBOL 提供的
  方便写法；标准 COBOL 在这里要求 `INTO` 或 `GIVING`）。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **多个接收方，每个都有自己的 `ROUNDED`**。
- ✅ 表达式运算符 `+ - * /` 和 `**`（乘方，右结合）、圆括号、
  `FUNCTION name(args)`。

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`。
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`。
- ✅ **`ALSO` 多主语** — `WHEN` 的每一列按位置与对应的主语比较，再用 AND 合并。
- ✅ **`WHEN NOT value`** 对某个选择对象取反；**`WHEN condition`**
  （例如 `EVALUATE TRUE WHEN a > b`）求值该布尔条件。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES`（n = 整数字面量或数据项）。
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`。
- ✅ 内联的 `PERFORM UNTIL cond … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ 内联的 `PERFORM n TIMES … END-PERFORM`（不用段落）。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — 每一轮迭代都执行该段落
  （非内联，没有 `END-PERFORM`）。
- ✅ **`WITH TEST AFTER` 也适用于 `VARYING`**，写在该短语的哪一侧都行，内联和非内联都行。
  循环体会在任何测试发生之前先跑一次，然后各条件**从最内层开始**依次测试；
  条件为假的那一层被递增，它里面的每一层都回到自己的 `FROM` 值，循环体再跑一次。
  变量只有在自己的测试结果为假时才被递增，所以结束循环的那次测试会让它保持循环体留下的样子。
- ✅ **`AFTER` 变量在自己的循环结束时被重置为它的 `FROM` 值**，
  而且是在外面一层被递增之前（COBOL-85 6.20.4 GR10(d)）。整个 `PERFORM` 结束之后，
  里层的变量读到的是它们的 `FROM` 值，只有最外层保留着结束循环的那个值。
- ✅ **带下标的 `VARYING` 标识符跟着它的下标走。**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` 递增的是
  `S1` 在那一刻所选中的那次出现，所以一个会推进 `S1` 的循环体就能遍历整张表。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`。
- ✅ **当一个段落名在多个节里重复出现时，`{OF|IN} section` 限定符挑出指的是哪一份**，
  跟它在 `PERFORM` 上的作用完全一样。**不认识的**节会退回到不带限定的查找，
  而不是把这次跳转丢掉。`GO TO … DEPENDING ON` 只接受一串光秃秃的名字，不带限定符；
  被 `ALTER` 改过向的 `GO TO` 则跟随那次改向 — 改向本身就直接指名了自己的目标。
  *（1.62.39 之前限定符被解析后就遭忽略，于是跳转落在了程序中任何地方的第一个定义上。）*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`。
- ✅ 单独的 `EXIT` 是一个什么也不做的返回点；`EXIT PROGRAM` 返回调用者。
- ✅ `EXIT PERFORM [CYCLE]`（中断 / 继续最近的那个内联 PERFORM）、
  `EXIT PARAGRAPH`、`EXIT SECTION`。
- ✅ `NEXT SENTENCE` — 把控制转移到下一个句子边界之后（分析器在每个句号处插入边界标记；
  这是忠实的实现，不只是一个 `CONTINUE`）。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`。
- ✅ **当 `SPECIAL-NAMES` 声明了该助记符时，`FROM mnemonic-name` 从操作员那里读取**
  （`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`）— 那是格式 1，与光秃秃的 `ACCEPT id` 完全相同。
  一个**没有任何 `SPECIAL-NAMES` 子句声明过**的名字则保留 PowerRustCOBOL 的扩展，
  去读同名的**环境变量**。到底适用哪一种，由声明决定，绝不由拼写决定。
  *（1.62.35 之前，普通的 `<implementor-name> IS <mnemonic>` 子句被整个跳过，
  于是每个助记符读的都是一个从未设置过的环境变量，接收项被留成了空的。）*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` 定位光标（ANSI，CLI）。
- ✅ `FROM COMMAND-LINE`（整条命令行）· `FROM ARGUMENT-NUMBER`（参数个数）
  · `FROM ARGUMENT-VALUE`（由 `DISPLAY n UPON ARGUMENT-NUMBER` 设定的指针处的参数）
  · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`
  （由 `DISPLAY "name" UPON ENVIRONMENT-NAME` 指名的那个变量）· `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`。
- ✅ `END-ACCEPT` 结束该语句（可选）。

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`。
- ✅ `END-DISPLAY` 结束操作数列表（可选），所以
  `DISPLAY A END-DISPLAY DISPLAY B` 是两条语句而不是一条。
- ✅ 屏幕形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — 在 **CLI 模式**（`rcrun`）下
  通过 ANSI 光标定位 + SGR 执行；在 GUI 模式下被忽略（那里由表单设计器取代了
  SCREEN I/O）。`ACCEPT id AT …` 先定位再读取。

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`。
  溢出 = 拼装出来的字符串比接收字段更宽。
- ✅ **一个 `DELIMITED BY` 短语管辖它前面的整串发送方**，
  而不只是紧挨着它写的那一个：
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` 会把三个都截断，
  拼出 `"ABC"`。一条语句里可以有好几个这样的短语，每个管辖自上一个短语以来的那些发送方；
  最后一个短语之后的发送方则整段取用。
  *（1.62.40 之前只有紧写在该短语之前的那个发送方会被截断。）*
- ✅ **`INTO` 一个组项**时，会分配到该组项的各从属项上。
- ✅ **结果是一个字节一个字节拼起来的**，所以 `STRING HIGH-VALUE` 传送的是那单独一个
  字节 `0xFF`，占用一个字符位置。
- ✅ **扩展 — 智能的默认 `DELIMITED BY`**（当没有短语管辖某个操作数时）：
  字母数字的 `PIC X`/`A` 项默认取 `SPACES`（丢掉尾部的填充）；字符串字面量、数值项、
  数字编辑项、`FUNCTION` 的结果以及表达式默认取 `SIZE`。数据项按其字段形态传送
  （数值 → PIC 完整宽度的数字；数字编辑 → 编辑后的字符）。

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`。溢出 = 源字段比接收方还多。

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` — **两半都会执行**。
- ✅ `BEFORE/AFTER INITIAL` 把每个短语限制在字段的一个子区域内。
  （按照 COBOL 的规定，TALLYING 是往计数器上累加。）
- ✅ **一串 TALLYING 操作数共享同一次从左到右的扫描**（COBOL-85 6.17.3）。
  在每一个字符位置上，操作数按书写顺序逐个尝试；第一个匹配上的拿走该位置，
  扫描从它消耗掉的那些字符之后继续。所以在 `"AABA"` 上
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` 得到 `t1 = 1, t2 = 1` —
  把两个操作数的顺序倒过来写，则得到 `t1 = 3, t2 = 0`。
  `LEADING` 必须从它那个窗口的左边缘起毫无间隙地匹配，所以只要靠前的操作数拿走了那个位置，
  这一串还没开始就结束了；而 `CHARACTERS` 只数那些没有被任何靠前的操作数认领过的位置。
- ✅ **一串 REPLACING 操作数同样共享同一次扫描**，规则一模一样：
  在某个位置上第一个匹配的操作数替换掉那些字符，扫描从它们之后继续，
  于是后面的操作数谁也看不到它们。每个操作数的 `BEFORE`/`AFTER` 窗口是
  **在任何替换发生之前**就定下来的，正因如此，一个操作数才能锚定在
  另一个更靠前的操作数会覆盖掉的字符上：

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  若是一个操作数一个操作数地依次施加，第一个短语就会抹掉第二个短语所锚定的那个 `"L "`，
  于是 `"BAD"` 会活下来。
- ✅ **带符号的 DISPLAY 项，它的字符位置里没有 `-`。** 运算符号是压印在某个数字上的，
  所以 `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` 得到 **0**，
  而 `FOR ALL "5"` 得到 1。符号事后会被恢复，所以在这些数字上做 `REPLACING`
  不会动到它。`SIGN IS … SEPARATE CHARACTER` 才是符号*确实*占一个位置的情形，
  那时它会被数进去。

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}`（编译成 MOVE）。
- ✅ `SET idx {UP|DOWN} BY n`（编码为 ADD / SUBTRACT）。
- ✅ `SET 88-name TO TRUE` 把宿主项置为该条件的第一个 VALUE；
  `TO FALSE` 置一个落在 VALUE 集合之外的值（尽力而为 — 没有 FALSE 子句）。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` 以及
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — 见下面的**指针**。

### INITIALIZE
- ✅ `INITIALIZE id …` — 按类别处理：数值 / 数字编辑 → ZERO，
  其余一律 → SPACES，并递归进入组项。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — 把该类别的每一个
  从属项都设成这个值；其余的不动。

### 指针（USAGE POINTER）
- ✅ `USAGE POINTER` 声明一个指针（初始为 NULL）。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — 把 `id` 变成目标存储的别名
  （读**和**写都跟随这个别名）；通常是一条 LINKAGE 记录。`IF ptr = NULL` 可用。

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`。
- ✅ 被调用的程序解析不到时，执行 `ON EXCEPTION` / `ON OVERFLOW` 的语句体；
  调用**解析成功**时，执行 `NOT ON EXCEPTION` 的语句体。
- ✅ `CANCEL program …` 重新初始化指名程序的 WORKING-STORAGE，
  这样它的下一次 `CALL` 就是从头开始的。

### 文件类动词（这里列的是支持的短语 — 完整覆盖在文件 I/O 测试套件里）
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`；`CLOSE f …`。
  （`SHARING` / `WITH LOCK` 会被解析，并在有意义的地方被遵守 —
  在单一运行单元的模型下它们只是建议性的。）
- ✅ **一条 `OPEN` 可以带好几组模式**，每一组有自己的文件：
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` 每一组各按自己的模式打开；
  `SHARING` / `WITH LOCK` / `REGISTERED USER` 作用于整条语句。
- ✅ **对一个已经打开的文件再做 `OPEN` 是 `41`**，而且文件保持原样 —
  这条语句**不会**把它重新打开。（重新打开一个 `OUTPUT` 文件会悄悄截断掉程序
  已经写进去的东西。）
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`**（PowerRustCOBOL 扩展）—
  把操作员/用户记进 INDEXED 的可观测性日志（该文件这次会话的每一行事件上的 `user=`
  字段）。它纯粹是观测性的；不做认证/授权。见
  [`observability-cn.md`](observability-cn.md) §1.3.1。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` 释放 INDEXED 引擎在 I-O 下所取的记录锁。
- ✅ **`READ … INTO id` 就是 `READ` 之后跟一次组项 `MOVE`。** 记录按宽度分配到
  接收方的各从属项上，并在接收方自己的宽度处截断；接收方可以带下标，
  而这次传送搬的是字节 — 一条含有非字符字节的记录会原封不动地到达。
- ✅ **FD 的 `RECORD` 子句 — 变长记录。** 三种写法全都支持：
  `RECORD CONTAINS n CHARACTERS`（定长）、`RECORD CONTAINS n TO m CHARACTERS`
  （变长；由 `WRITE` 指名的那个记录描述给出长度），以及
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  （那个数据项*就是*长度 — `WRITE` 之前由程序设定，`READ` 会把它设回来，
  并被夹到所声明的范围内）。一个 FD，只要它的各条 `01` 记录大小不同，
  那它就是变长的，不管有没有明说。变长文件把每条记录的长度和记录一起存起来，
  所以它的字节和定长文件的**不能**互换；定长文件则保持原样。
- ✅ **一个 FD 的那些 `01` 记录描述的是同一块记录区。** `READ` 会把字节透过每一条
  记录描述送达；`WRITE` 送出的是整块区域，所以别的记录描述放在
  “被写的那条记录里是 `FILLER`”的位置上的东西，会透出来。
- ✅ **`FILLER` 在 FD 记录里实实在在占着它那些字节**，而
  `SIGN IS SEPARATE CHARACTER` 会让带符号的 DISPLAY 项比它的数字位置多宽一个字符。
- ✅ **FD 的 `LINAGE` 除整数外也接受数据名** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`。
  页面在每次 `WRITE` 时都按那些项来量，所以程序可以在运行当中改变页面尺寸。
  文件被打开时 `LINAGE-COUNTER` 为一。
- ✅ **`AT END` 之后再做顺序 `READ` 是 `46`，不是第二个 `10`。**
  `AT END` 没有留下有效的下一条记录，所以继续往下读是与“读到了末尾”不同的另一种错误。
  `46` 是第 4 类状态，所以 `AT END` 和 `NOT AT END` 都不会为它运行 —
  处理它的是该文件的 `USE` 声明节。重新 `OPEN` 一次，或者一次成功的 `START`，
  会重新确立起一条记录。
- ✅ `UNLOCK f [RECORD[S]]` 释放该文件的记录锁。
- ✅ **`COMMIT` / `ROLLBACK`** — 覆盖**所有**已打开 INDEXED 文件的、由程序控制的事务。
  `OPEN` 开启一个事务；`COMMIT` 确认待定的 `WRITE`/`REWRITE`/`DELETE`
  （之后的 `ROLLBACK` 就再也撤不掉它们了）并开启一个新事务；
  `ROLLBACK` 撤销自上一次 `COMMIT`/`OPEN` 以来的每一处改动。
  **DISK** 存储让 `COMMIT`/`CLOSE` 在磁盘上持久化。**MEMORY** 存储把
  `COMMIT`/`ROLLBACK` 完全放在 RAM 里（从不写盘）；一个光是
  `STORAGE IS MEMORY` 的文件是易失的，而 `STORAGE IS MEMORY WITH PERSISTENCE`
  只在 `CLOSE` 时存盘。（借助持久的预写日志做崩溃恢复是后续的工作 —
  这里说的是运行期内、程序级别的回滚。）
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`**（INDEXED 文件；PowerRustCOBOL 扩展）。默认的存储是 `DISK`。
  `WITH COMPRESSION` 会压缩存下来的记录（键在未压缩的记录上求值）；
  `WITH PERSISTENCE`（只对 MEMORY）在 `CLOSE` 时把内存中的文件存下来。
  `OPEN OUTPUT` 总是（重新）创建磁盘上的容器。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`；
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ **对记录顺序（SEQUENTIAL）文件的 `REWRITE`** 就地替换上一次 `READ` 送来的那条记录，
  并把读取位置留在原处 — 下一次 `READ` 给出的仍然是它后面那条记录。它该给出的状态是：
  文件不是以 `I-O` 打开时是 **`49`**；没有一次成功的 `READ` 确立过记录时是 **`43`**
  （包括 `AT END` 之后，以及中间没有 `READ` 的第二次 `REWRITE`）；
  新记录与读到的那条长度不同时是 **`44`** —
  在带 `DEPENDING ON` 的文件上，那个数据项的值就是这个长度，程序正是靠它来要一个不同的长度。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ 跨*进程*的文件共享不做强制（单一运行单元）；`SHARING`/`LOCK` 这些短语会被解析，
  而 INDEXED 引擎在本次运行内的记录锁是被遵守的。

### SORT / MERGE / RELEASE / RETURN  ✅（可用，工作缓冲区在内存中）
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]`（在 INPUT PROCEDURE 里）向本次运行追加记录；
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` 把记录交回来。
- 记录按所声明的键（`ASCENDING`/`DESCENDING`）做稳定排序；
  `USING` 读、`GIVING` 写所指名的那些顺序文件。

---

## 条件（IF / EVALUATE / PERFORM UNTIL）

- ✅ 关系符号：`=` `<>` `<` `>` `<=` `>=`。
- ✅ 关系词写法：`[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`、`[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ 类别：`id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
  PICTURE 中**不带运算符号**的数据项，只有在每一个字符位置都是数字时才算
  `NUMERIC` — 存放着 `"+1234"`、`"1.234"` 或 `"12 45"` 的 `PIC X(5)` **不是**
  数字型。*（在 1.62.40 之前，这个判断会把字符当成一个数来解析，于是符号、小数点、
  指数以及前后的空格全都被接受。）*
- ✅ **用户自定义 `CLASS` 的操作数可以是一个序号位置** — `CLASS ORDINAL-A-ONLY IS
  66` 指的是本机字符集里的第 66 个字符 — 而且这个操作数可以单独占一行源码。
  `ALPHABET` 也一样。
- ✅ 符号：`id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 层的条件名（把名字单独写出来当作条件）。
- ✅ **把 `TRUE` / `FALSE` 当作操作数**（PowerRustCOBOL 扩展）— 它们是 `1` 和 `0`
  的语法糖，凡是允许出现值的地方都能用：`IF x = TRUE`、`IF x IS [NOT] FALSE`、
  `IF x NOT TRUE`（不带关系运算符、只有一个 `NOT` 的写法）、
  `PERFORM UNTIL x = FALSE`、`MOVE TRUE TO x`、`COMPUTE n = n + TRUE`、
  `INVOKE obj "m" USING TRUE`，以及针对一个取值主语的 `WHEN TRUE`。单独一个
  `TRUE`/`FALSE` 本身也是一个完整的条件（`IF TRUE`、`PERFORM UNTIL TRUE`）。
  ⚠️ 这**不会**改变这两个词原本就有含义的那两处：`SET <88‑name> TO TRUE` 仍然是把
  宿主数据项设成一个满足该条件的值（而不是数字 1），下文的 `EVALUATE
  TRUE`/`EVALUATE FALSE` 也仍然是标准的分支语句。
- ✅ `AND` / `OR` / `NOT` 的组合与括号（AND 的结合力强于 OR）。
- ✅ **运算符前置的缩略条件** — `a > 1 AND < 9`、`a = 5 OR = 7`（复用前一个比较的
  主语）。
- ✅ **以字面量为宾语的缩略写法** — `a = 1 OR 2 OR 3`（主语和运算符都被复用；宾语是
  一个字面量）。
- ✅ **以标识符为宾语的缩略写法** — `a = b OR c`（其中 `c` 是一个数据项）。比较之后
  跟在 AND/OR 后面的单个标识符在运行时解析：如果它是一个已知的 88 层条件名，就按
  条件名求值；否则它就是 `a = c` 的宾语。（紧跟着 `AND` 的标识符保持 AND 的优先
  级。）
- ✅ **放在缩略写法*宾语*之前的 `NOT` 否定的是这个关系**，而不是宾语：
  `a > b OR NOT c` 等于 `a > b OR NOT (a > c)`。`NOT <relational operator>` 这种
  写法（`AND NOT < x`）属于运算符形式，保持不变；而开启一个普通条件的 `NOT` —
  `NOT (…)`、`NOT x = y`、`NOT x NUMERIC` — 各自保留原本的含义。*（在 1.62.42
  之前，宾语形式被读成“这个宾语非零”，只有当宾语恰好存放着零时，这才给出相同的
  答案。）*
- ✅ **在组合项上声明的条件名检验的是这个组合项的字节。** 组合项自己不拥有存储 —
  它*就是*它的子项 — 所以
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` 比较的是记录里存放的
  那六个字符。
- ✅ **形象常量会被重复到与另一个操作数一样长**，写成某个 88 的 `VALUE` 时同样如
  此：宿主为 `PIC X(4)` 时，`88 B VALUE QUOTE` 是四个引号，而
  `88 D VALUE ALL "BAC"` 是 `"BACB"`。`ALL literal` 在**两个**方向上都会定长 —
  对一个十字符的 `X` 来说，`IF X EQUAL TO ALL "BA"` 比较的对象是
  `"BABABABABA"`，而不是用空格补齐的 `"BA"`。

---

## 表达式、字面量、USAGE

- ✅ 算术运算符 `+ - * /` 与 `**`；括号；一元 `+`/`-`。
- ✅ `FUNCTION 名称 ( 参数 [ , 参数 … ] )` —— **已实现**的内部函数：
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM（种子可选）, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`。
  （日期换算使用标准基准 1601‑01‑01 = 第 1 天。）**COBOL‑85 标准的内部函数集合**
  已完整实现。
- ✅ **日期与时间寄存器读取的是 LOCAL 本地时钟。** `ACCEPT … FROM DATE / TIME /
  DAY / DAY-OF-WEEK` 与 `FUNCTION CURRENT-DATE` 报告的都是机器自身的当地时刻，而
  不是 UTC —— 日期也一样，在午夜两侧会得到不同的结果。`CURRENT-DATE` 的最后五个
  字符携带相对 GMT 的**真实**偏移量（`…-0300`），因此程序可以判断自己正运行在哪个
  时区。
  ⚠️ 任何无法识别的 `FUNCTION` 名称仍可解析，但在运行时返回 **0**。
- ✅ 字面量：整数、小数、字符串，以及全部形象常量
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **形象常量会填满整个接收方**，`HIGH-VALUE` 也不例外 ——
  `MOVE HIGH-VALUE TO <PIC X(10)>` 得到十个 `0xFF` 字节；送入组项时则分配到各个
  子项。经过编辑的字母数字接收方仍会放置它的插入字符，因此 `PIC XX0XXBXXX` 保存
  的是 `FF FF '0' FF FF ' ' FF FF FF`。在 `PROGRAM COLLATING SEQUENCE` 之下，该
  常量指的是一个普通字符，于是改由那个字符来填充。
  ⚠️ `HIGH-VALUE` 是**字节** `0xFF`，而不是一个字符。读取组操作数、编辑以及所有
  传送路径都会逐字节地原样搬运它，但**引用修改尚未做到字节精确**：对于确实存放着
  `0xFF` 的数据项，`IF X (1:1) = HIGH-VALUE` 仍为假。
- ✅ **数值字面量可以以小数点开头** —— `.5`、`-.5`、`.000000001`。COBOL‑85 只要求
  字面量不以小数点*结尾*，因此 `5.` 依然是数字 5 后面跟着一个句子终止符。
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  前导零是有效且精确的：`.000000001` 是十亿分之一，而不是十分之一。在
  `DECIMAL-POINT IS COMMA` 之下，`,5` 同理。
  把字面量与句末句点区分开来的是**有没有空格** —— COBOL‑85 要求终止符之后必须有
  一个空格，因此 `MOVE X TO Y.` 绝不会被读成小数的开头，而 `MOVE X TO Y.5` 会是
  一个编译错误，而不是被悄悄改换解释。
- ✅ **一致性标记**（`cobolt_semantic::flagging`）—— 标准要求，符合规范的实现应当
  能够告诉程序：它所使用的特性中，哪些落在所选的一致性级别之外。两项分析回答了这
  个问题：
  - `flag_obsolete` —— COBOL‑85 的**废弃要素**集合：IDENTIFICATION DIVISION 的五
    个可选段落、`MEMORY SIZE`、`ALTER`、带字面量的 `STOP`，以及不带过程名的
    `GO TO`。
  - `flag_high_subset` —— 高**子集**之上的一切，从 `COMPUTE`、`EVALUATE` 与
    `INITIALIZE`，经由 `CORRESPONDING`、引用修改、限定、`SET … TO TRUE` 和第四个
    下标，直到跨卡片边界续行一个*单词*或一个*数值字面量*。（续行**字母数字**字面
    量属于子集之内，不会被报告。）

  两者都不是错误检查，也都不会在普通构建中运行：它们所列举的每一种结构都是
  RustCOBOL 已实现并能执行的合法 COBOL‑85。把它们做成独立的入口点，正是为了让一次
  普通编译永远不会开始对 `AUTHOR` 或 `COMPUTE` 发出警告。NIST 的 `NC302M`、
  `NC303M` 与 `NC401M` 对其进行了验证 —— 分别为 7、4 和 40 个标记，全部吻合。
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** —— 用来填充编辑型 PICTURE
  中货币位置的字符。它是**取代** `$`，而不是与之并存，所以一旦程序声明了它，`$`
  在那里就不再是一个 picture 字符：
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  此时 `MOVE ZERO TO FL-LESS` 读作 `      <.00`，`MOVE 1234` 读作 ` <1,234.00`
  —— 浮动串的行为与 `$$$,$$$.99` 完全一致。**字母**形式的货币符号同样适用：
  `CURRENCY SIGN IS "W"` 使 `PICTURE WWWWW` 成为一个五位的浮动货币串，于是
  `MOVE 12` 读作 `  W12`。*（在 1.62.40 之前，由字母符号构成的连续串会被读成一个
  单词而遭到拒绝，因此只有 `$` 能浮动。）* 该
  字面量必须是单个字符，而且 COBOL‑85 禁止使用会与 picture 字符或分隔符冲突的
  字符：不能是数字，不能是 `A B C D E G N P R S V X Z` 中的任何一个，也不能是
  `space * + - , . ; ( ) " / =` 中的任何一个。
- ✅ **十六进制字面量** —— `X"09"`、`x'0D0A'`（大小写不限，两种引号皆可）。每
  **一对**十六进制数字对应一个字符，因此位数必须是偶数；位数为奇数或出现非十六
  进制字符即属畸形字面量，会被报告出来，而不会被悄悄重新读成紧挨着字符串的单词
  `X`。凡是可以使用带引号字面量的地方都可以使用它（`DELIMITED BY`、`MOVE`、
  `VALUE`、比较）。

---

## DATA DIVISION 子句（接受的声明语法）

- ✅ 级别 `01`–`49`、`77`、`88`；`FILLER`；组合项/基本项。`FILLER` 这个词是
  **可选的** — `05 PIC X VALUE ":".` 声明出来的东西和
  `05 FILLER PIC X VALUE ":".` 完全一样，两种写法都在包含它的组合项内部占据
  自己的字节并保存自己的 `VALUE`。
- ✅ 带 `X A 9 S V P` 以及编辑符号（`Z * $ + - CR DB B 0 / , .`）的
  `PIC/PICTURE`。除非 `SPECIAL-NAMES. CURRENCY` 指定了别的符号，货币符号就是
  `$` — 参见上文的**表达式、字面量、USAGE**。**`P` 是十进制标度位置** — 项目
  跨越但并不存储的数字位置：`PIC S999PP` 保存三位代表百位的数字（`MOVE 12300`
  会原样存下，`MOVE 12345` 存下的是 12300），而 `PIC PP99` 保存两位代表万分位的
  数字。`P` 所占的位置读回来永远是零，并且在记录布局中**不占任何字节**。
- ✅ **星号保护会填满整个项目。** 当一个数字位置全为 `*` 的 PICTURE 里放的是零
  时，每一个字符位置都会被星号填满 — 小数位、分组逗号、固定的 `$`、以及末尾的
  `CR` 或 `DB` 一视同仁 — 只留下小数点本身：保存零的 `PIC $**.**CR` 读作
  `***.****`，`PIC *,***.**` 读作 `*****.**`。**非**零的值只保护前导零，因此固定
  的 `$` 保留自己的位置（`-2.34` → `$*2.34CR`）。*（1.62.37 之前 `CR`/`DB` 只
  贡献一个星号，而不是它们实际占据的两个字符位置，于是这样的项目返回时比它自身
  的宽度少一个字符。）*
- ✅ **数字文字量按写法搬运自己的字符。** 送往字母数字接收方时，文字量提供的是
  程序里写下的那些数字，左对齐并用空格填充 — `MOVE 2 TO <PIC X(4)>` 得到
  `"2   "`，而 `MOVE 060820000200 TO <six PIC 99 children>` 把它们填成
  `06 08 20 00 02 00`。**接收方**的宽度从不为文字量补位；只有文字量自己被写出来
  的宽度才会。*（1.62.38 之前 lexer 只保留数值，于是前导零丢失，后面的每个字符
  都向左移了一位。）*
- ✅ **数字操作数与非数字操作数之间的比较是非数字比较**
  （COBOL‑85 VI‑89 6.15.4 GR2）。数字操作数被当作已经搬到一个**与它自身同样
  大小**的字母数字项目中处理，这会传递它的字符位置而**不传递它的运算符号**：保存
  `-123456789012345678` 的 `PIC S9(18)` 与保存 `"123456789012345678"` 的
  `PIC X(18)` 比较结果为**相等**。三个条件限定了这条规则 — 数字操作数必须是
  **整数**；是否“非数字”由**声明**决定，所以在一次组合项 `MOVE` 之后保存着字符的
  `PIC 99` 子项目仍然是数字项目 — 而**组合项**无论其子项目是什么都属于非数字，
  所以保存 12345 的 `PIC 9(5)` 与保存 `"0000012345"` 的十字节组合项相比时是
  `"12345     "`，两者不相等；另外 `ALL literal` 取另一个操作数的大小。
  *（1.62.38 之前，只要文本一侧碰巧能被解析成数字，比较就按代数方式进行。）*
- ✅ **数字 MOVE 会发生高位截断。** 接收方在两端都只保存它声明的位数：
  `01 M PIC 99V999.  MOVE 123.45 TO M.` 留下的是 `23.450`。算术运算会先检验接收
  方的容量，因此带 `ON SIZE ERROR` 的语句会改为保留它原来的值。
- ✅ **组合项的表按出现项寻址。** `MOVE VALUES-1 TO GRP-1 (2)` 把值分配到该出现
  项自己的子项目上（`ELEM1 (2,1) … ELEM1 (2,4)`），而读取 `GRP-1 (2)` 恰好把它们
  连接起来。包住它的 `01` 记录是**每一个**出现项的字节，所以
  `MOVE GRP-TAB1 TO GRP-TAB2` 会复制整张表。
- ✅ **索引名、文字量和相对索引可以混作下标。** `ELEM1 (IN1, 1)`、
  `ELEM1 (1 IN2)`、`ELEM1 (IN1 +3)` — 紧贴着数字的符号是一个带符号文字量，它开启
  下一个下标 — 而 `ELEM1 (IN1 - 1, 3)` 中运算符两侧都有空格，那是相对索引。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}`（以及 `COMP-4`→COMP、`COMP-X`→COMP-5）。
- ✅ `VALUE`（数字/带符号/字母数字/形象常量/`ALL`）。**`VALUE ALL "literal"` 会
  把它的单元重复铺满整个项目** — `PIC X(6) VALUE ALL "ABC"` 是 `"ABCABC"`，
  `PIC X(9) VALUE ALL "XY"` 是 `"XYXYXYXYX"`。
  *（1.62.40 之前只有单字符的形象常量才会填满自己的项目，而 `ALL "literal"` 会让
  项目里留着空格。）*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES` — 对同一批字节的第二次**活的**解读。它不增加存储（因此不会加宽
  包含它的组合项），并且通过任一描述所做的写入都能从另一个描述看到：
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` 之后可以通过 `RESULT-A` 读回来。
  ⚠️ **注意事项：** 大于 256 个展开存储槽的重叠视图（例如一张被重定义的 10×10×10
  的表）会改为保留按描述分别存储的方式 — 每次写入都刷新它就要把一千个出现项走
  两遍。
- ✅ **重叠视图可以嵌套。** 位于一条本身也被重定义的记录内部的 `REDEFINES`，无论
  嵌套多深都能在两个方向上被触及：通过一个 01 级的重定义写入两个字节，会触及被
  重定义的记录、它内部某个组合项的 `REDEFINES`，以及*那个*组合项内部某个项目的
  `REDEFINES` — 包括声明在最内层项目上的 88。每一份描述在每次写入时都会被重新
  生成一次。*（1.62.42 之前，属于多个重叠视图的键只保留最后声明的那一个，而单一
  的防护判断在第一跳之后就中断了整条链。）*
- ✅ **没有名字的描述仍然是一份描述。** `02 FILLER REDEFINES <item>.` 在没有自己
  名字的情况下重新描述了目标的字节，而对目标的写入可以通过它的子项目看到。若有
  多个子项目，它们就按布局顺序瓜分那些字节 — 重叠视图*并不是*它第一个子项目的
  别名。对同一个项目的两个 `FILLER REDEFINES` 是两次互相独立的解读，每一次都从
  目标的**第一个**字节开始。*（1.62.36 之前，没有名字的重定义组合项根本拿不到
  存储键，于是无论目标被填成什么样，它的子项目读出来都是空格。）*
- ✅ **重叠视图内部重复的名字**会解析到程序其余部分所触及的同一块存储：在两个
  不同组合项下声明的 `TAB-A` 会为每一次声明保留一次解读。*（1.62.36 之前，重叠
  视图的初始副本是用一条缺少外层限定符的路径来做键的，而那正是只有重复的名字
  才能分辨的东西 — 于是恰恰是需要限定符的那种情形把限定符弄丢了。）*
- ✅ `JUSTIFIED [RIGHT]` — 在*字母数字*项目或*字母*项目上**按右对齐存储**。比
  接收方窄的发送方会在左侧被填充；比它宽的发送方保留自己的**右**端，丢掉最左边的
  字符 — 与通常的规则正好相反。*（1.62.40 之前这个子句只对字母数字项目被记录，
  于是 `PICTURE A(5) JUSTIFIED RIGHT` 能被解析，随后却像任何其他项目一样左
  对齐。）*
- ✅ `SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL` — 均被接受；
  `SIGN … SEPARATE` 目前还不会改变项目的存储方式。
- ✅ **01 级的 `REDEFINES` 可以描述比它所重定义的项目更多的存储**，而超出那个
  项目末尾的字节属于长到足以指名它们的那份描述。通过较短的描述写入不会动到较长
  描述的尾部。
- ✅ **`REDEFINES` 重叠视图会带上被重定义项目的字节**，包括带进一个数字对应项
  里：保存 `"00ABCDEFGHI  4321 "` 的 `X(18)` 上的 `PIC S9(18)` 重叠视图会把那些
  字符读回来，而 `IS NUMERIC` 对它们的回答是**否**。当那些字节确实拼成数字时，
  数字方式的读取不受影响。
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **真正的条件名**：88 级绑定到
  它的宿主项目；测试会拿宿主去比对那些 VALUE / 范围，而 `SET 88-name TO TRUE` 会
  把一个使条件成立的值存进宿主。
- ✅ **一个条件名可以声明在不止一个组合项之下，`OF`/`IN` 能把它们区分开** — 与
  数据名的情形完全相同，并且中间的层级可以省略：
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  下标属于宿主项目，因此它选定拿哪一个出现项去比对 VALUE。对重复的条件名作
  **未加限定**的引用在 COBOL‑85 中是有歧义的；运行时取第一次声明，这与它处理有
  歧义的数据名所用的规则相同。
- ✅ `USAGE INDEX` 声明一个整数索引寄存器（`SET`/`SEARCH` 会用到它）；
  `USAGE POINTER` — 参见上文的**指针**。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — 一个重新分组的别名；读取
  时把被覆盖的各项目连接起来，写入时按字段宽度分配。
  - ✅ **一个 66 由它所重新分组的那条记录来限定**，就像一个数据项由它上方的组合
    项来限定一样，因此同一个 66 名字可以每条记录声明一次，再用 `OF`/`IN` 区分：
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`。这在读和写上同样有效，而
    且当一个普通数据项碰巧同名时，66 优先。`RENAMES` 子句的操作数也在同一条记录
    中解析，所以重复的 `NAME-2` 指的是本记录的那一个。
  - ✅ **被覆盖的表会贡献它的每一个出现项**，而不只是第一个：当 `TABLE-2` 里是
    `03 T PIC XXX OCCURS 5` 时，`66 R RENAMES ITEM-1 THRU TABLE-2` 的宽度是 20
    个字符。
  - ✅ **恰好覆盖一个项目的 66 *就是*那个项目** — 相同的 PICTURE、相同的类别、
    相同的存储。当 `W` 是 `PIC 9(4)` 时，`66 R RENAMES W` 就是一个四位数字项目，
    所以里面放着 8000 时执行 `ADD 3500 TO R` 会触发 `ON SIZE ERROR` 并让它保持
    不变。
- 节：`WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`；`SCREEN` 会被解析
  但不执行。

---

## 仍不支持 —— 当前回避清单

> **2026‑08‑25 更正。** 本节过去的开头是「COBOL‑85 的动词／子句集合已**完全
> 覆盖**。」跑了一遍 NIST CCVS85 套件之后，这句话被推翻了：**那一天，范围内的
> 434 个程序中有 102 个失败**，失败之处正是本文档没有列为差距的那些结构 ——
> 作分隔符用的逗号与分号、`FUNCTION x(ALL)`、`CLOSE … WITH LOCK`、写在 B 区的
> `COPY`、IDENTIFICATION 的注释条目、节的优先级编号、以数字开头的数据名，以及
> —— 直到 1.62.10 为止 —— 以小数点开头的数值字面量。验证套件的意义正在于此。每
> 一项差距现在都在 [`specs/nist/`](../specs/nist/README.md) 中有了规格说明，并在
> 上面的[记分板](#-一致性是测出来的不是宣称出来的--nist-ccvs85)中被跟踪。

下面这份清单是**有意**排除在范围之外的内容，与上面那些正在逐一修复的 NIST 差距
（缺陷）不同：

1. **屏幕 `ACCEPT` 的输入编辑** —— `DISPLAY … AT/WITH` 与 `ACCEPT … AT` 在 CLI
   模式下（借助 ANSI）会被执行，但 SCREEN SECTION 完整的字段级编辑（自动跳格、
   字段校验、颜色映射）在 GUI 模式下**已由 form designer 取代**。
2. **跨*进程*的文件共享** —— `OPEN … SHARING/WITH LOCK`、
   `READ … WITH [NO] LOCK` 与 `UNLOCK` 可以解析，并会驱动 INDEXED 引擎在本次运行
   内的记录锁，但这些锁不会在不同的操作系统进程之间被强制执行（单一运行单元
   模型）。
3. **面向对象的 COBOL**（类／方法定义）—— 对 COBOL 对象而言 `INVOKE` 是空操作
   （它只驱动 GUI／运行时对象）。
4. 无法识别的内部函数名仍然返回 **0** —— 同样是这种无声失败的方式。规格说明：
   [内部函数](../specs/nist/NIST-spec-intrinsic-function-gaps.md)。
5. ⚠️ **无效的 `ACCESS MODE` ／ `ORGANIZATION` 取值会被悄悄吞掉，不给任何
   诊断** —— 又是同一个陷阱，而且这一个是由用户一次普通的笔误触发的。
   `ACCESS MODE IS` 只接受 `SEQUENTIAL`、`RANDOM` 或 `DYNAMIC`（`INDEXED` 是一种
   *组织方式*，不是访问模式），但 SELECT 子句的解析器在检验完这三者之后，会让其余
   任何取值落入「跳过一个未知记号」的通用分支，于是该文件悄悄保留了默认的
   `SEQUENTIAL`，并且不是编译失败，而是在运行时行为失常。`ORGANIZATION IS` 的形态
   完全相同。两者都应当抛出一个明确的编译期错误，并点名那个出问题的词。**这不是
   核心（Nucleus）的问题** —— 没有任何 NC 程序带有 `ACCESS MODE` 子句；该子句只
   出现在 DB、IC、IX、OBSQ、RL、RW、SQ 和 ST 这些模块中，因此按照黄金法则 #9，这
   件事要等到 NC 完成之后再做。
6. ⚠️ **`ALPHABET … IS EBCDIC` 会被接受，但仍旧沿用本机（ASCII）的排序。**
   字面短语（`"A" THRU "H" "I" ALSO "J" …`）、`NATIVE`、`STANDARD‑1` 与
   `STANDARD‑2` 都已实现，并且会真正驱动 `PROGRAM COLLATING SEQUENCE`；唯独缺少
   EBCDIC 表，写上它就会悄无声息地得到 ASCII 顺序。与第 4–6 项属于同一族陷阱。
7. **通信模块与 Report Writer** —— 参见
   [上文的 N/A](#-na--哪些在-rustcobol-范围之外以及为什么)。

> **已解决（1.5.0）：** 扁平的数据模型变为层次化／按出现次数感知，从而解锁了
> **CORRESPONDING**、**限定名**、**表下标**与 **`SEARCH`**。
> **已解决（1.6.0）：** 多接收方的 `MULTIPLY`/`DIVIDE` 与逐接收方的 `ROUNDED`；
> `EXIT PERFORM/PARAGRAPH/SECTION`；`CALL NOT ON EXCEPTION`；
> `INSPECT TALLYING REPLACING` 的合并使用与 `BEFORE/AFTER INITIAL`；日期与
> `ANNUITY` 内部函数；字面量宾语的缩写；`EVALUATE ALSO`/`WHEN NOT`；真正的 88 层
> 条件名；`PERFORM para VARYING`；以及带 `RELEASE`/`RETURN` 的 `SORT`/`MERGE`
> 运行时。
> **已解决（1.7.0）：** 标识符宾语的缩写；`INITIALIZE … REPLACING`；
> `66 RENAMES`；指针（`USAGE POINTER`、`SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`）；`ALTER` / `UNLOCK`；忠实的 `NEXT SENTENCE`；余下的标准内部函数；以及
> 扩展的屏幕 `ACCEPT`/`DISPLAY`（在 CLI 模式下执行）。
> **已解决（1.7.1）：** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS`（连同配对的
> `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` 寄存器）。
> **已解决（1.7.2）：** `OPEN … SHARING/WITH LOCK`、`READ … WITH [NO] LOCK`、
> `UNLOCK`（释放 INDEXED 记录锁），以及 `CANCEL 程序`。
> **已解决（1.8.0）：** `COMMIT` / `ROLLBACK` 作为由程序控制的 INDEXED 文件事务
> （内存与磁盘两种引擎；磁盘上有真正的撤销日志）。

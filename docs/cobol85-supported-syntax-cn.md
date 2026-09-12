<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.0 -->

# RustCOBOL‑85 支持语法参考

**本文档的用途：** 说明 RustCOBOL 究竟实现了 COBOL‑85 标准的多少，并且是对着
**NIST COBOL‑85 官方验证套件**加以证明，而不是空口声称。下面的
[记分板](#-一致性是测量出来的不是声称出来的--nist-ccvs85)是主标题；其后的一切都是
支撑那个数字的细节。

**关于 RustCOBOL 的词法分析器／语法分析器／运行时今天实际接受什么的实地事实**，
由源代码（`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`）推导而来，并与
`NIST/newcob.val,cbl` 相互核对。
请针对 ✅ 的形式编写测试；❌ 的形式无法完成语法分析或不执行任何操作，而 ⚠️ 的形式
能被分析但行为只是部分正确。本文档是
[`cobol85-verb-test-matrix-cn.md`](cobol85-verb-test-matrix-cn.md) 的姊妹篇：
矩阵说明*测试什么*，本文档说明*RustCOBOL 理解哪种写法*。

图例：✅ 支持 · ⚠️ 能分析但部分／简化 · ❌ 不识别（请避开，或仅为确认缺口而测试）。

---

## 目录

1. [★ 一致性是测量出来的，不是声称出来的 — NIST CCVS85](#-一致性是测量出来的不是声称出来的--nist-ccvs85)
2. [IDENTIFICATION DIVISION 段落](#identification-division-段落)
3. [源格式](#源格式)
4. [可识别的语句（动词）](#可识别的语句动词)
5. [各动词支持的形式](#各动词支持的形式)
6. [条件（IF / EVALUATE / PERFORM UNTIL）](#条件if--evaluate--perform-until)
7. [表达式、字面量、USAGE](#表达式字面量usage)
8. [DATA DIVISION 子句（接受的声明语法）](#data-division-子句接受的声明语法)
9. [仍然不支持 — 当前的规避清单](#仍然不支持--当前的规避清单)

---

## ★ 一致性是测量出来的，不是声称出来的 — NIST CCVS85

**这就是本文档的要点。** 下面的每一项断言都对着 **NIST COBOL‑85 官方验证套件**
加以检验 — CCVS85 版本 4.0（01 OCT 1992，COBOL 85 版本 4.2，1993 年 4 月 SSVG），
也就是美国国家标准与技术研究院当年用来认证 COBOL 编译器的那套套件。它有 28 MB、
348,271 行、**459 个 COBOL 程序**和 51 个 copybook 成员，存放在本仓库的
`NIST/newcob.val,cbl`。

它是真相的来源。凡 RustCOBOL 与 CCVS85 不一致之处，**CCVS85 是对的，RustCOBOL 是
错的**。

机器可读的台账是 [`NIST/progress.json`](../NIST/progress.json) — 纳入版本管理，
每次经过验证的改动之后更新。下面的数字取自它，而不是重新手打。

### 记分板

**于 2026‑08‑31 在 1.62.132 上测得**，针对未经改动的发行物。编译普查在 1.62.129
收尾。

| 轴 | 结果 | 含义 |
|---|---:|---|
| **编译** | **420 / 420** | 范围内的每个程序都被前端接受。FAIL 0。 |
| **执行** | **380 / 380** | 每个参与计分的程序都能运行，并在自己的 CCVS 报告中报告**零失败**。 |
| **断言** | **8,362 PASS / 0 FAIL** | 那些程序对自身所做的检查。 |

任一轴都可以复现：

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### 这两个轴从不混为一谈

编译是严格意义上较弱的断言：它说的是前端接受一个程序中的所有构造，而不是该程序
算出了正确答案。这套套件为自己打分 — 每个 CCVS85 程序都会打印自己的 `PASS` /
`FAIL*` 统计 — 因此意味着「它能工作」的是执行这个轴。两者都在下面按模块报告，各有
自己的分母，而且任何一个都绝不会被当作另一个来引用。

最清楚的例证就在本仓库自己的历史里：35 个 RELATIVE 文件程序中有 30 个编译得干干
净净，而运行时**根本没有 RELATIVE 引擎**。它们运行起来，悄无声息地给出错误结果。
引擎在 1.62.76 落地，模块在 1.62.77 完成。

#### 按模块

编译与执行带着不同的分母，原因有两条，都已明言。`*301M` 成员测试的是对 RustCOBOL
作为标准实现的那些特性做*中间子集标示*，这在设计上不可达，并依运行者的裁定从执行中
排除（IX301M、RL301M、ST301M、SM301M）；它们在编译普查中仍然计入，并且通过。而
大多数 IC 成员是**被调用方** — 没有自己报告的子程序 — 因此只有调用方程序参与计分。

| 模块 | 测试内容 | 编译 | 执行 | 断言 | 状态 |
|---|---|---:|---:|---:|---|
| **NC** | 核心 | **95 / 95** | **95 / 95** | 4,614 | ✅ 已完成 |
| **SQ** | 顺序 I/O | **85 / 85** | **85 / 85** | 624 | ✅ 已完成 |
| **IX** | 索引 I/O | **42 / 42** | **41 / 41** | 574 | ✅ 已完成 |
| **IF** | 内部函数 | **45 / 45** | **45 / 45** | 841 | ✅ 已完成 |
| **IC** | 程序间通信 | **47 / 47** | **25 / 25** | 309 | ✅ 已完成 |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ 已完成 |
| **SM** | 源文本操作 | **17 / 17** | **16 / 16** | 311 | ✅ 已完成 |
| **RL** | 相对 I/O | **35 / 35** | **34 / 34** | 354 | ✅ 已完成 |
| **DB** | 调试 | **14 / 14** | — | — | 仅编译轴（见下） |
| **范围内** | | **420 / 420** | **380 / 380** | **8,362** | |
| SG | 分段 | 13 / 13 | — | — | ⬜ 已裁定不在范围内（见下） |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB（调试）** 只在编译轴上计分。它的 14 个程序都被接受；调试模块的*运行时*语义
尚未实现，它的执行轴也未被裁定纳入范围。把它列在这里而不是藏起来，是为了让缺口
保持可见。

#### DELETED 计数 — 24，以及它的含义

`***** ****TEST DELETED****` 是 CCVS 自己的标记，表示程序自行跳过的一个用例。它
**不是**通过，正因如此才单独统计：在 1.62.53，这个计数从 108 → 1，而干净程序的
数量几乎没动 — 那是真实的进展，只看失败数的读法会把它漏掉。

在已完成的各模块中共有 24 个 DELETED 用例：NC 5、SQ 6、IX 1、IC 4、SM 3、RL 5。
**只有 SM 的 3 个被记载为发行物自身的**  — SM206A 的 PST‑TEST‑008 与 PST‑TEST‑11，
以及 SM208A 的 REP‑TEST‑7，在发行时就是注释掉的，因此对所发布源代码的一次合规运行
恰好报告这三个。其余 21 个已被记录，但台账中尚未逐一解释；请不要把它们当作有意为之
来引用。

### ⬜ N/A — 哪些内容不在 RustCOBOL 的范围内，以及为什么

这些模块**不计为失败** — 共 38 个程序被排除在所有计分之外。完整理由见
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md)。

| 模块 | 程序数 | 为什么不在范围内 |
|---|---:|---|
| **CM** — 通信 | 9 | `COMMUNICATION SECTION`、`CD` 条目、`SEND` / `RECEIVE` / `ENABLE` / `DISABLE`。针对的是 1980 年代的远程处理监控程序 — 由事务管理器持有的消息队列。这里并不存在这样的运行时，而该模块本身也已从后来的 COBOL 标准中移除。 |
| **RW** — Report Writer | 6 | `REPORT SECTION`、`RD` 条目、`INITIATE` / `GENERATE` / `TERMINATE`、控制断点。这是一门规模不小的声明式子语言；PowerRustCOBOL 对报表的答案是 Form Designer 和 PDF 导出。如果需要，它以后可以成为一项*功能* — 这是唯一一个对用户真正有价值的排除项。 |
| **SG** — 分段 | 13 | 运行者裁定，2026‑08‑29。分段的存在是为了把程序装进一台小到容不下它的机器：`SECTION` 标题带着段号，运行时把互相独立的段覆盖在彼此之上。RustCOBOL 是 64 位运行时，地址空间比任何 COBOL 程序能耗尽的都多，因此段号**能编译，并且完全没有任何效果**。这个模块没有可供测量的行为。它的 13 个程序仍然能编译，并且被报告为 N‑A 而不是删除，以便让这项排除保持可见。 |
| **OBSQ / OBIC / OBNC** | 9 | 它们重新测试先前的模块，并期待编译器*标示*出 COBOL‑85 的废止要素。它们的语言内容已被范围内的规格覆盖；不在范围内的是对废止特性的**标示**。 |
| **EXEC85** | 1 | 它不是测试。它是 NIST 自己的 COBOL 执行程序，用来切分发行物并驱动整套套件 — 在这里被一个 Rust 执行框架取代，因此不需要编译。 |

**面向对象 COBOL** 同样不在 RustCOBOL 的范围内，但 CCVS85 完全早于它 — 套件中没有
任何 OO 程序。

### 还剩什么

在参与计分的模块上，什么也不剩：两个轴都已闭合，没有任何断言失败。剩下的不是一份
缺陷清单，而是三项长期裁定 — DB 的执行轴、SG，以及 `*301M` 标示成员 — 每一项都在
上文连同理由一起记录，此外还有 21 个尚未逐一解释的 DELETED 用例。

执行框架会打印出任何回归背后的失败细节，可直接按模块归类：

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> 一条 `FAIL*` 细节行是**故意**写两次的 — CCVS 的 `PRINT-DETAIL` 执行
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — 而 `PASS ` 只写一次。任何从
> 打印文件里取得的原始标记计数，都必须先把失败数除以二才有意义。

### 一致性历史

编译轴，对照当时的范围内分母。分母本身在 SG 被裁定不在范围内时、以及 DB205A 被改
计入 CM 时发生过变动，因此早期各行是以 434 为分母，收尾那一行是以 420 为分母。

| 版本 | 编译 | 改动了什么 |
|---|---:|---|
| 1.62.7 | **0** / 434 | 什么都编译不了。经典参考格式的两条规则缺失：第 73‑80 列被当作源代码读取，而续行从不拼接。 |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — 经典参考格式，包含续行。参见[源格式](#源格式)。 |
| 1.62.13 | 292 / 434 | 作为分隔符的逗号和分号是标点，不是记号；下标之间可以只用空格分隔；字面量内部成双的定界符算作一个字符。三整桶诊断被清空。 |
| 1.62.14 | 317 / 434 | 整张表作为内部函数实参；`CLOSE … WITH LOCK` / `NO REWIND` / `REEL`。**内部函数在编译上达到 45 / 45。** |
| 1.62.16 | 376 / 434 | `AT END` 中的 `AT` 是可选的，因此一个单独的 `END` 短语不再吞掉紧随其后的段落标题（33 个程序）。**索引 I/O 在编译上达到 42 / 42。** |
| 1.62.21 | 417 / 434 | 核心那一轮 — `ALTER` 系列、带下标的条件名、缩写的组合关系、跨操作数的 `INSPECT` 类别。核心从 76 → 92 个能编译。 |
| **1.62.42** | 420 / 434 | **核心在两个轴上完成** — 95 / 95 能编译*并且*干净执行，4,614 条断言无一失败。 |
| **1.62.43** | 422 / 434 | 顺序 I/O 完全能编译，85 / 85，执行从 85 个中的 10 → 44。declarative 的段落保留了名字，因此 `USE` 处理程序可以对它们 `PERFORM` 和 `GO TO` — 20 个程序不再崩溃。 |
| **1.62.47** | — | **顺序 I/O 完成** — 两个轴均 85 / 85。最后一个缺口是 `XXXXD001`，一个由 CCVS85 *安装过程*提供、而没有任何成员写入的数据文件；现在由执行框架放置。 |
| **1.62.76** | — | **RELATIVE 引擎**落地（`cobolt-runtime/src/relative.rs`，容器 `PRCREL1`）。七个文件动词全部按 `FileOrganization::Relative` 分派。 |
| **1.62.77** | — | **相对 I/O 完成**（34 / 34），在一次会话中从 14 / 35 的基线达成；并且**索引 I/O 也完成** — 相对引擎关掉了 IX106A 最后四项失败，它们正是该程序与顺序、索引文件并列使用的那个相对文件。 |
| **1.62.81** | — | **内部函数在执行上完成**，45 / 45，从 24 / 45 的基线达成。五个原因，没有一个出在该模块本身的主题上：词法分析器的分隔符规则两次、一处标示缺口、`NUMVAL` 的实参文法、一处实参列表比较，以及除法的溢出路径。 |
| **1.62.107** | — | **程序间通信完成**，25 / 25。 |
| **1.62.119** | — | **Sort / Merge 完成**，39 / 39，735 PASS。最后一步是 SORT/MERGE 上的 `[COLLATING] SEQUENCE [IS] alphabet-name`，按 `SPECIAL-NAMES` 中命名的字母表对字母数字键排序。 |
| **1.62.127** | — | **源文本操作完成**，16 / 16。字符串字面量操作数保留自己的引号；标识符操作数覆盖其 `IN`/`OF` 链和下标；各对替换在一趟中完成，不再重新扫描替换结果。 |
| **1.62.129** | **420 / 420** | **编译普查以 100 % 收尾。** DB205A 依裁定计入 CM，这使范围内套件为 420 个。 |

> **诚实的总结。** 范围内的每个程序都能编译，参与计分的每个程序都能干净运行：
> **编译 420 / 420，执行 380 / 380，8,362 条断言无一失败。** 在上表第一行之前的
> 九个版本，编译数字还是零。仍然未决的部分在上文以裁定的形式明确写出，而不是藏在
> 一个百分比里 — DB 的执行轴、SG、`*301M` 标示成员，以及 21 个尚未逐一解释的
> DELETED 用例。

---

> **更新（缺口实现那一轮）：** 以下已实现，现在是 ✅ — **引用修饰**
> `id(start:len)`、**行内 `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING 的 `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、
> **按类别处理的 `INITIALIZE`**、**运算符前置的缩写条件**（`a > 1 AND < 9`）、
> **`CALL … ON EXCEPTION`**（在未解析的 CALL 上执行）、**`COMPUTE` 的多个接收项 +
> 每个接收项各自的 `ROUNDED`**，以及一个大得多的**内部函数**集合。
>
> **更新（层次化／按出现处理的环境那一轮 — 1.5.0）：** 四项曾被数据模型卡住的特性
> 现在是 ✅ — **运行时表下标** `t(i)` / `t(i, j)`（按出现的存储）、**限定名消歧**
> `id OF/IN group`（重名的叶子项解析到互相独立的存储）、
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**，以及**可用的 `SEARCH` / `SEARCH ALL`**。
>
> **更新（动词完整性那一轮 — 1.6.0）：** 现在还有 ✅ — `ADD`/`SUBTRACT` 上的
> **多接收项 `MULTIPLY`/`DIVIDE GIVING` + 每个接收项各自的 `ROUNDED`**；
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** 以及被修正的单独
> `EXIT`；**`CALL … NOT ON EXCEPTION`**；**`INSPECT … TALLYING … REPLACING`** 的
> 合并使用以及 **`BEFORE/AFTER INITIAL`** 区域；日期／财务**内部函数**
> （`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、`INTEGER-OF-DAY`、`DAY-OF-INTEGER`、
> `ANNUITY`、`FRACTION-PART`）；**以字面量为对象的缩写条件**（`A = 1 OR 2 OR 3`）；
> **`EVALUATE … ALSO`**（多主语）与 **`WHEN NOT`**；**真正的 88 级条件名**
> （`SET … TO TRUE/FALSE`，对宿主项按其 VALUE／范围进行检验）；
> **`PERFORM para VARYING`**；以及一个可用的 **`SORT`/`MERGE`** 运行时
> （`RELEASE`/`RETURN`、`USING`/`GIVING`、`INPUT`/`OUTPUT PROCEDURE`）。文末的规避
> 清单是最新的。
>
> **更新（清理规避清单那一轮 — 1.7.0）：** 剩余的缺口现已实现 — **以标识符为对象的
> 缩写**（`a = b OR c`，通过 88 级元数据解析）；
> **`INITIALIZE … REPLACING category DATA BY value`**；**`66 RENAMES`**（读取时合成，
> 写入时按所覆盖的各项分配）；**指针**（`USAGE POINTER`、
> `SET ptr TO ADDRESS OF x / NULL`、作为别名的 `SET ADDRESS OF item TO …`、
> `IF ptr = NULL`）；**`ALTER`** / **`UNLOCK`**；忠实的 **`NEXT SENTENCE`**；余下的
> 标准**内部函数**（`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、`NUMVAL-F`、
> `TEST-NUMVAL`）；以及扩展的屏幕 **`ACCEPT`/`DISPLAY`**（在 CLI 模式下通过 ANSI 实现
> `AT`/`WITH` — 现在是真正*执行*，不只是被分析）。
>
> **更新（1.7.1）：** `ACCEPT` 的寄存器来源现在可用了（此前是被识别的空操作） —
> **`FROM COMMAND-LINE`**、**`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`**（与
> `DISPLAY n UPON ARGUMENT-NUMBER` 配对）、**`ENVIRONMENT-VALUE`**（与
> `DISPLAY "name" UPON ENVIRONMENT-NAME` 配对）、**`ESCAPE KEY`** → `"00"`、
> **`CRT STATUS`** → `"0000"`。
>
> **更新（1.7.2）：** 文件共享／加锁短语和 `CANCEL`（此前是 ❌ / 空操作） —
> **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`**（释放该文件的 INDEXED 记录锁），以及 **`CANCEL program`**
> （重新初始化程序的存储）。
>
> **更新（1.8.0）：** **`COMMIT` / `ROLLBACK`** 现在是真正的 COBOL 动词 — 对已打开的
> INDEXED 文件（内存与磁盘两种引擎）进行由程序控制的事务。磁盘引擎获得了真正的运行
> 期撤销日志（此前是空操作）。文末的规避清单是最新的。

---

## IDENTIFICATION DIVISION 段落

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ **注释条目**段落 — `AUTHOR`、`INSTALLATION`、`DATE‑WRITTEN`、
  `DATE‑COMPILED`、`SECURITY` — **任意顺序、任意子集**。
- ✅ `REMARKS` 同样被接受。它在 1985 年从 COBOL 中删除，因此不予保存；接受它是为了
  让从 COBOL‑74 沿用过来的源代码仍然能够编译。

**注释条目**是自由文本，而 COBOL‑85 是字面意义上的这个说法：

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- 它可以包含**保留字** — 上面的 `DATA` 并不开启一个 DATA DIVISION。
- 它可以包含**句点**，并且不会在句点处结束。
- 它**跨越你所写的任意多行**。
- 它在下一个于 A 区**位于行首**的段落标题或 division 标题处结束 — 上面那个条目正是
  这样在 `DATE-WRITTEN` 处结束的。

**那段文字中的引号被限制在它所在的那一行内**（自 1.62.12 起）。诸如
`THE COMPILER"S ABILITY` 这样的文本，不再开启一个一直延伸到程序其余部分的字面量 —
参见[源格式](#源格式)。在注释条目中避免不成对的引号仍然值得，但现在它只让你付出
那一行，而不是整个文件。

⚠️ `INSTALLATION`、`SECURITY` 和 `REMARKS` 在这里**不是保留字**。它们仅在
IDENTIFICATION DIVISION 内部被识别为段落名，因此一个名为 `SECURITY` 的数据项仍然
可以使用。

---

## 源格式

RustCOBOL 读取三种源代码布局。这个选择是显式的 — **绝不会**根据文件内容去猜测，
因为把列规则套用到并非为其编写的源代码上，会悄无声息地删掉代码。

| `--source-format` | 含义 |
|---|---|
| `free` | 完全没有列规则。`*>` 开启注释。**默认值**，也是 PowerRustCOBOL 自身的项目以及生成的窗体 `.cbl` 文件所使用的格式。 |
| `fixed` | ✅ **COBOL-85 的经典参考格式** — 标准所定义、卡片映像源代码所采用的布局。见下文。 |
| `fixed-relaxed` | 序号区和指示列受到尊重，但一行可以一直写到你敲到的地方 — 没有 72 列的限制。 |
| `auto` | 历史行为：`free`，除非 `COBOLT_FIXED=1`。 |

`COBOLT_SOURCE_FORMAT` 设定一次会话的默认值。

### `fixed` — 经典参考格式

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **第 1-6 列** — 序号区，被忽略。
- **第 7 列** — 指示区：
  - `*` 或 `/` → 注释行
  - `-` → 上一行的**续行**
  - `D` → 调试行；按注释处理（调试模式尚未实现）
  - 其他任何字符 → 作为普通源代码读取。标准保留了这一列，但卡片映像套件把它用作
    可选行的选择符，而悄悄丢掉那些行就会删掉代码。
- **第 8-72 列** — 源代码。
- **第 73-80 列** — 标识区，**被丢弃**。

### 续行 ✅

第 7 列的连字符延续上一行。

**延续一个单词或一个数值字面量** — 被延续那一行末尾的空格被丢弃，两半之间不留
任何东西地接上：

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

这声明了一个名为 `WRK-DS-18V00-CONTINUED` 的数据项。

**延续一个字母数字字面量** — 被延续那一行的字面量没有闭合引号；续行必须以一个引号
重新开启，字面量从它之后的那个字符继续：

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **被延续的片段一直延伸到第 72 列，包括末尾的空格。** 一行即使在第 72 列之前就
结束，仍然会把那些空格贡献给字面量。这就是为什么被延续的字面量只在 `fixed` 之下
才是逐字节精确的；其他格式没有可供停下的第 72 列。

### 字面量绝不会意外跨行 ✅

续行是字面量跨越多行的**唯一**途径。没有在自己那一行闭合的引号是一个错误，并在它
被写下的位置报告出来：

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

这比听上去更重要。在 1.62.12 之前，一个不成对的引号会一直延伸到文件中任何位置的
*下一个*引号，因此注释里一个走失的 `"` 就会吞掉整整几个 division，并让其后所有引号
的配对发生偏移 — 发现这一问题的那些 NIST 程序，引号数量是**偶数**，所以没有任何
东西未终结；仅仅一个字符就改变了整个文件的配对奇偶性。现在损害止于换行。

> **自由格式没有字面量续行。** 不是 `&` — 那是连接*运算符* — 也不是围栏块。自由
> 格式的字面量必须放在一行内；较长的请使用连接：`"first part" & "second part"`。

> **注意。** 为一个以自由格式编写的文件选择 `fixed` 会损坏它 — 超过第 72 列的一切
> 都会消失，而第 8 列之前的文本会被当作序号读取。只对确实是卡片映像的源代码使用它。

---

## 可识别的语句（动词）

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2`（改变 para-1 的 `GO TO` 去向）·
`UNLOCK file`（释放该文件的记录锁）· `OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK`（文件共享／加锁 — 在单一运行单元内属于建议性）
✅ `COMMIT` / `ROLLBACK`（由程序控制的 INDEXED 文件事务 — 参见文件动词）·
`CANCEL`（重新初始化程序的存储）·
✅ `INVOKE` — 驱动 GUI／运行时对象（窗口、窗体、控件方法）；仅对 **COBOL** 对象
不执行任何操作，因为类／方法定义不在范围内
项目扩展：`EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。一个块可以
`use` 始终被链接的 crate（std、egui、eframe 以及被链接的运行时集合），**外加项目在
Project 的 Crates 中登记的任何 crate**（spec 044）：已登记的 crate 被固定到确切
版本，取入项目的 `crates/`，并编译进二进制文件；未登记的 crate 会在开发者所在的那
一行让 Check/Build 失败，并指出补救办法。

✅ `SEARCH`（顺序）/ `SEARCH ALL`（对带 `ASCENDING`/`DESCENDING KEY` 的表做二分
查找 — 执行第一个匹配的 `WHEN`，否则执行 `AT END`）。
✅ `SORT` / `MERGE` 配合 `RELEASE` / `RETURN`（功能完整 — 见下文）。
✅ `DECLARATIVES … END DECLARATIVES` 配合 `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — 在未处理的错误 `FILE STATUS` 上触发的
文件错误处理程序。处理程序**从其 section 的开头进入，并一直执行到该 section 的
末尾**，而且它的各个段落保留自己的名字，因此可以对它们使用 `PERFORM` 和 `GO TO` —
包括*另一个* declarative section 的段落。declarative 的段落活在自己的名字空间里：
控制权绝不会从主体落入其中，而在两处都被声明的名字，在处理程序运行期间解析到
declarative 的那一份，在其他任何地方解析到主体的那一份。declarative 也可以
`PERFORM` 非 declarative 部分的段落。
❌ **不识别 — 请勿使用：** `ENTRY`、`GENERATE`/`INITIATE`/`TERMINATE`、
`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 各动词支持的形式

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]`（多个接收项）。
- ✅ **只要有一个组操作数，整个传送就是字母数字的**（COBOL-85 6.18.4）。另一个操作数
  的 PICTURE 只贡献它的*大小*，别的什么都不贡献：不编辑、不反编辑、不做数值转换。
  `MOVE <持有 "123ABC" 的组项>` 会在 `PIC 0XXXXX0` 中留下 `"123ABC "`（而不是编辑后的
  `"0123AB0"`），在 `PIC 9999V999` 中留下同样的六个字符加一个空格，在 `PIC 99` 中留下
  `"12"`。`JUSTIFIED RIGHT` 仍然决定哪一端被填充、哪一端被丢弃。同一条规则支配组项
  自身的字节：每个子项原样取走自己那一段，因此一个字母数字编辑的子项**不会**被再次
  编辑。
- ✅ **加在组项上的 `VALUE` 子句**会初始化该组的字节并分配到它的各个子项 —
  `01 G VALUE "$123.45". 02 E PIC $999.99.` 会让 `E` 持有 `"$123.45"`。
- ✅ `MOVE CORRESPONDING g1 TO g2` — 传送两个组按名字共有的每个从属项，并递归进入
  互相匹配的子组。
- ✅ **`CORRESPONDING` 会排除以 `REDEFINES` 或 `RENAMES` 描述的项**
  （COBOL-85 6.18.4 GR1），两侧皆然，连同一切从属于它的东西。排除针对的是*声明*，
  而不是名字：一个仅仅与别处某个 66 级共享名字的普通数据项，仍然参与对应。
- ✅ **`CORRESPONDING` 的任一操作数都可以指名一张组表中的某一次出现** —
  `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` 写入该次出现自己的各个位置，而下标会
  沿着递归一路带下去。
- ✅ **一对项中只需要有一个是基本项。** 组项可以与基本项相对，它们之间的传送就是
  字母数字的：一个基本的 `PIC XXX` 发往由 `999` + `XXX` 构成的组，会填满它的六个
  字符；一个由 `XXX` + `99` 构成的组发往一个朴素的 `X(5)`，会把它填满。两个组彼此
  相对时仍然**递归** — 那种配对不是基本项的情形。*（在 1.62.39 之前，两个方向都什么
  也传不了：组项并不拥有存储位置，因此写入去到了没人会读回的地方，而读取得到的是
  空字符串。）*
- ✅ **引用修饰 `id(start:len)`** — 发送方（子串）与接收方（拼接式的部分赋值）；对
  所有动词的操作数都有效。`length` 可省。它寻址的是**字符位置**，因此数值操作数是按
  其 `PIC` 的完整宽度连同前导零一起取用的：`01 T PIC 9(8) VALUE 00224845` 给出的
  `T(1:2)` 是 `"00"`，不是 `"22"`。
- ✅ **组项是字母数字聚合体** — 一个组项*就是*它的从属项首尾相接排在一起，它的大小
  是各从属项大小之和。读取一个组项会把子项连接起来（包括 `FILLER`）；向一个组项传送
  则按宽度把字节分配到各子项。`MOVE 11 TO A` 透过包含 `A` 的那个组项可见，而
  `MOVE "1234" TO G` 设置的是 `G` 的子项，而不是它自己的某个位置。
- ✅ 下标 `t(i)`、`t(i, j)` — 读写按出现划分的存储位置；可变下标 `t(WS-I)` 在每次
  访问时求值。
- ✅ 限定 `id OF/IN group`（`… OF g1 OF g2`） — 即使叶子名在不止一个组下被声明，也能
  解析到正确的数据项。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **每个接收项各自的 `ROUNDED`** — 每个接收项带着自己的 `ROUNDED` 标记。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — 对每一对匹配的数值项做运算，并
  递归进入互相匹配的子组。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **多个 `GIVING` 接收项**，每个都有自己的 `ROUNDED`。
- ⚠️ `DIVIDE a BY b`（没有 `GIVING`）会把 `a/b` 存回 `a`（这是 PowerRustCOBOL 的
  便利做法；标准 COBOL 在这里要求 `INTO` 或 `GIVING`）。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **多个接收项，每个都有自己的 `ROUNDED`**。
- ✅ 表达式运算符 `+ - * /` 和 `**`（幂，右结合）、圆括号、`FUNCTION name(args)`。

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`。
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`。
- ✅ **`ALSO` 多主语** — 每个 `WHEN` 列按位置与它对应的主语比对，并以 AND 组合。
- ✅ **`WHEN NOT value`** 对选择对象取反；**`WHEN condition`**
  （例如 `EVALUATE TRUE WHEN a > b`）求值布尔条件。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES`（n 为整数字面量或数据项）。
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`。
- ✅ 行内 `PERFORM UNTIL cond … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ 行内 `PERFORM n TIMES … END-PERFORM`（不用段落）。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — 每次迭代都执行那个段落
  （行外形式，没有 `END-PERFORM`）。
- ✅ **`WITH TEST AFTER` 对 `VARYING` 同样适用**，写在短语的哪一侧都行，行内行外都行。
  循环体先执行一次，之后才检验任何条件，然后各条件**从最内层开始**检验；条件为假的
  那一层被增量，它内部的每一层都回到自己的 `FROM` 值，循环体再执行一次。变量只在自己
  的检验结果为假时才被增量，因此结束循环的那次检验会把它保持为循环体留下的样子。
- ✅ **`AFTER` 变量在它自己的循环结束时被重置为 `FROM` 值**，发生在外面一层被增量之前
  （COBOL-85 6.20.4 GR10(d)）。整个 `PERFORM` 结束后，内层变量读到的是各自的 `FROM`
  值，只有最外层保留着结束它的那个值。
- ✅ **带下标的 `VARYING` 标识符跟随它的下标。**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` 增量的是
  `S1` 在那一刻所选中的那次出现，因此一个推进 `S1` 的循环体会走遍整张表。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`。
- ✅ **当一个段落名在多个 section 中重复时，`{OF|IN} section` 限定符选择所指的是哪
  一份**，与它在 `PERFORM` 上的作用完全一致。遇到**未知的** section，会退回到不加
  限定的查找，而不是丢掉这次跳转。`GO TO … DEPENDING ON` 接受一串朴素的名字，不接受
  限定符；而被 `ALTER` 改过去向的 `GO TO` 会遵循那次改向 — 改向本身已经明确指名了自己
  的目标。*（在 1.62.39 之前，限定符被分析之后就被忽略，因此跳转会落到程序中任何位置
  的第一个定义上。）*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`。
- ✅ 单独的 `EXIT` 是一个不做任何事的返回点；`EXIT PROGRAM` 返回到调用方。
- ✅ `EXIT PERFORM [CYCLE]`（跳出／继续最近的行内 PERFORM）、`EXIT PARAGRAPH`、
  `EXIT SECTION`。
- ✅ `NEXT SENTENCE` — 把控制权转移到下一个句子边界之后（语法分析器在每个句点处插入
  边界标记；这是忠实实现，不只是 `CONTINUE`）。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`。
- ✅ **当 `SPECIAL-NAMES` 声明了该助忆名时，`FROM mnemonic-name` 从操作员读取**
  （`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`） — 那就是格式 1，与朴素的 `ACCEPT id` 完全相同。**没有任何
  `SPECIAL-NAMES` 子句声明的**名字保留 PowerRustCOBOL 的扩展，读取同名的**环境变量**。
  究竟适用哪一种，由声明决定，绝不由拼写决定。*（在 1.62.35 之前，普通的
  `<implementor-name> IS <mnemonic>` 子句被整个跳过，因此每个助忆名读的都是一个从未
  被设置的环境变量，接收项就被留空了。）*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` 定位光标（ANSI，CLI）。
- ✅ `FROM COMMAND-LINE`（整条命令行）· `FROM ARGUMENT-NUMBER`（实参个数）·
  `FROM ARGUMENT-VALUE`（位于由 `DISPLAY n UPON ARGUMENT-NUMBER` 设定的指针处的实参）·
  `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`（由
  `DISPLAY "name" UPON ENVIRONMENT-NAME` 指名的变量）· `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`。
- ✅ `END-ACCEPT` 结束该语句（可选）。

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`。
- ✅ `END-DISPLAY` 结束操作数列表（可选），因此
  `DISPLAY A END-DISPLAY DISPLAY B` 是两条语句而不是一条。
- ✅ 屏幕形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — 在 **CLI 模式**（`rcrun`）下通过
  ANSI 光标定位 + SGR 执行；在 GUI 模式下被忽略（在那里 Form Designer 取代了 SCREEN
  I/O）。`ACCEPT id AT …` 先定位再读取。

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`。
  溢出 = 拼装出来的字符串比接收字段更宽。
- ✅ **一个 `DELIMITED BY` 短语支配它前面的整串发送方**，而不只是紧写在它之前的那
  一个：`STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` 会把三个都做定界，拼出
  `"ABC"`。一条语句可以带多个短语，每个支配自上一个短语以来的各发送方；位于最后一个
  短语之后的发送方则各取其全部。*（在 1.62.40 之前，只有紧写在短语之前的那个发送方
  才被定界。）*
- ✅ **`INTO` 一个组项**会分配到该组的各从属项。
- ✅ **结果是逐字节拼装的**，因此 `STRING HIGH-VALUE` 传送的是单个字节 `0xFF`，占据
  一个字符位置。
- ✅ **扩展 — 智能的默认 `DELIMITED BY`**（当没有任何短语支配某个操作数时）：字母
  数字的 `PIC X`/`A` 项默认为 `SPACES`（末尾填充被丢弃）；字符串字面量、数值项、
  数值编辑项、`FUNCTION` 的结果以及表达式默认为 `SIZE`。数据项按其作为字段的形态
  传送（数值 → PIC 完整宽度的数字；数值编辑 → 编辑后的字符）。

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`。溢出 = 源字段比接收项更多。

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` — **两半都会施行**。
- ✅ `BEFORE/AFTER INITIAL` 把每个短语限定在字段的一个子区域内。
  （按 COBOL 的规定，TALLYING 是往计数器上累加。）
- ✅ **一串 TALLYING 操作数共用同一次从左到右的扫描**（COBOL-85 6.17.3）。在每个
  字符位置上，各操作数按它们被写下的顺序依次尝试；第一个匹配的取走该位置，扫描从它
  所消耗的字符之后继续。因此对 `"AABA"` 施行
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` 得到 `t1 = 1, t2 = 1` — 把两个操作数
  反过来写则得到 `t1 = 3, t2 = 0`。`LEADING` 必须从它那个窗口的左边界起毫无间隔地
  匹配，因此某个更早的操作数一旦取走那个位置，这一串就在开始之前结束了；而
  `CHARACTERS` 只统计没有被任何更早操作数占走的位置。
- ✅ **一串 REPLACING 操作数同样共用一次扫描**，依据同一条规则：在某个位置上第一个
  匹配的操作数替换掉那些字符，扫描从它们之后继续，因此后面的任何操作数都看不到它们。
  每个操作数的 `BEFORE`/`AFTER` 窗口是**在任何替换之前**就确定的，正是这一点使得一个
  操作数可以锚定在更早的操作数会覆盖掉的字符上：

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  若是一次只施行一个操作数，第一个短语就会抹掉第二个短语所锚定的 `"L "`，于是
  `"BAD"` 就会留存下来。
- ✅ **带符号的 DISPLAY 项在它的各字符位置中并没有 `-`。** 运算符号是叠印在某个数字
  上的，因此 `INSPECT <持有 -12345 的 PIC S9(5)> TALLYING c FOR ALL "-"` 得到 **0**，
  而 `FOR ALL "5"` 得到 1。符号随后会被复原，因此对各数字施行 `REPLACING` 不会碰到它。
  `SIGN IS … SEPARATE CHARACTER` 才是符号*本身就是*一个位置的情形，那时它会被计入。

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}`（编译为 MOVE）。
- ✅ `SET idx {UP|DOWN} BY n`（编码为 ADD / SUBTRACT）。
- ✅ `SET 88-name TO TRUE` 把宿主项设为该条件的第一个 VALUE；`TO FALSE` 设为 VALUE
  集合之外的某个值（尽力而为 — 没有 FALSE 子句）。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` 和
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — 参见下面的**指针**。

### INITIALIZE
- ✅ `INITIALIZE id …` — 按类别处理：数值／数值编辑 → ZERO，其余一切 → SPACES，并
  递归进入组项。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — 把该类别的每个从属项
  设为该值；其余不动。

### 指针（USAGE POINTER）
- ✅ `USAGE POINTER` 声明一个指针（初始为 NULL）。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — 把 `id` 变成目标存储的
  别名（读取**和**写入都遵循这个别名）；通常用于 LINKAGE 记录。`IF ptr = NULL` 有效。

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`。
- ✅ 当被调用的程序无法解析时，`ON EXCEPTION` / `ON OVERFLOW` 的主体会执行；当调用
  **成功解析**时，`NOT ON EXCEPTION` 的主体会执行。
- ✅ `CANCEL program …` 重新初始化所指名程序的 WORKING-STORAGE，因此它的下一次
  `CALL` 会从全新状态开始。

### 文件动词（受支持的短语 — 完整覆盖在文件 I/O 套件中）
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`；`CLOSE f …`。
  （`SHARING` / `WITH LOCK` 会被分析，并在有意义的地方得到遵循 — 在单一运行单元的
  模型下属于建议性。）
- ✅ **一条 `OPEN` 可以带多个模式组**，每组有自己的文件：
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` 每一组都以它自己的模式打开；
  `SHARING` / `WITH LOCK` / `REGISTERED USER` 作用于整条语句。
- ✅ **对一个已经打开的文件再 `OPEN` 是 `41`**，并且该文件保持原样 — 这条语句**不会**
  重新打开它。（重新打开一个 `OUTPUT` 文件会悄悄截断程序已经写入的内容。）
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`**（PowerRustCOBOL 扩展）
  — 把操作员／用户记录到 INDEXED 的可观测性日志中（该文件本次会话的每一行事件都会带
  `user=` 字段）。这纯属观测用途；不含身份认证／授权。参见
  [`observability-cn.md`](observability-cn.md) §1.3.1。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` 释放 INDEXED 引擎在 I‑O 下取得的记录锁。
- ✅ **`READ … INTO id` 就是 `READ` 之后紧跟一次组 `MOVE`。** 记录按宽度分配到接收项
  的各从属项，并在接收项自身的宽度处截断；接收项可以带下标，而这次传送搬运的是字节 —
  一条持有非字符字节的记录会完好无损地到达。
- ✅ **FD 的 `RECORD` 子句 — 变长记录。** 三种写法都支持：
  `RECORD CONTAINS n CHARACTERS`（定长）、`RECORD CONTAINS n TO m CHARACTERS`
  （变长；由 `WRITE` 指名的那个记录描述给出长度），以及
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  （那个数据项*就是*长度 — 在 `WRITE` 之前设定，由 `READ` 设回，并被限制在所声明的
  范围内）。一个各 `01` 记录大小互不相同的 FD 就是变长的，无论它是否这么写。变长文件
  会把每条记录的长度与记录一起存储，因此它的字节与定长文件的字节**不可互换**；定长
  文件则保持不变。
- ✅ **一个 FD 的各 `01` 记录描述的是同一块记录区。** `READ` 通过所有记录描述交付
  字节；`WRITE` 发送整块区域，因此另一个记录描述放在「被写出的那个描述在此处是
  `FILLER`」的位置上的内容会透出来。
- ✅ **`FILLER` 在 FD 记录中占据自己的字节**，而 `SIGN IS SEPARATE CHARACTER` 会让
  一个带符号的 DISPLAY 项比它的数字位置多宽一个字符。
- ✅ **FD 的 `LINAGE` 既接受整数也接受数据名** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`。页面是在每次
  `WRITE` 时根据那些数据项测定的，因此程序可以在运行中改变页面尺寸。文件被打开时
  `LINAGE-COUNTER` 为一。
- ✅ **在 `AT END` 之后再做顺序 `READ` 是 `46`，而不是第二次 `10`。** `AT END` 并
  没有留下有效的下一条记录，因此继续读下去是与「到达末尾」不同的错误。`46` 属于第 4
  类状态，因此 `AT END` 和 `NOT AT END` 都不会为它执行 — 处理它的是该文件的 `USE`
  declarative。一次新的 `OPEN`，或者一次成功的 `START`，会重新建立起记录。
- ✅ `UNLOCK f [RECORD[S]]` 释放该文件的记录锁。
- ✅ **`COMMIT` / `ROLLBACK`** — 对**所有**已打开的 INDEXED 文件进行由程序控制的
  事务。`OPEN` 开启一个事务；`COMMIT` 确认待定的 `WRITE`/`REWRITE`/`DELETE`（此后的
  `ROLLBACK` 再也无法撤销它们）并开启一个新事务；`ROLLBACK` 撤销自上一次
  `COMMIT`/`OPEN` 以来的所有改动。**DISK** 存储让 `COMMIT`/`CLOSE` 在磁盘上持久化。
  **MEMORY** 存储让 `COMMIT`/`ROLLBACK` 纯粹在 RAM 中进行（从不写盘）；一个朴素的
  `STORAGE IS MEMORY` 文件是临时的，而 `STORAGE IS MEMORY WITH PERSISTENCE` 只在
  `CLOSE` 时保存到磁盘。（通过持久的预写日志实现的崩溃恢复属于将来的工作 — 这里是
  运行期内、程序层面的回滚。）
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`**（INDEXED 文件；PowerRustCOBOL 扩展）。默认存储是 `DISK`。
  `WITH COMPRESSION` 压缩所存储的记录（键是在未压缩的记录上求值的）；
  `WITH PERSISTENCE`（仅 MEMORY）在 `CLOSE` 时把内存中的文件保存下来。`OPEN OUTPUT`
  总是（重新）创建磁盘上的容器。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`；
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ **对记录型 SEQUENTIAL 文件的 `REWRITE`** 就地替换上一次 `READ` 所交付的那条
  记录，并把读取位置留在原处 — 下一次 `READ` 仍然给出紧随其后的那条记录。它应当返回的
  状态是：文件未以 `I-O` 打开时为 **`49`**；没有任何成功的 `READ` 建立起记录时为
  **`43`**（包括在 `AT END` 之后，以及中间没有 `READ` 的第二次 `REWRITE`）；新记录与
  所读到的那条长度不同时为 **`44`** — 在带 `DEPENDING ON` 的文件上，那个数据项的值
  就是这个长度，程序正是以此来请求另一个长度的。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ 跨*进程*的文件共享未被强制（单一运行单元）；`SHARING`/`LOCK` 短语会被分析，而
  INDEXED 引擎在本次运行内的记录锁会得到遵循。

### SORT / MERGE / RELEASE / RETURN  ✅（功能完整，工作缓冲区在内存中）
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]`（在 INPUT PROCEDURE 中）向本次运行追加记录；
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` 把记录交还回来。
- 记录按所声明的键做稳定排序（`ASCENDING`/`DESCENDING`）；`USING` 读取、`GIVING`
  写出所指名的顺序文件。

---

## 条件（IF / EVALUATE / PERFORM UNTIL）

- ✅ 关系符号：`=` `<>` `<` `>` `<=` `>=`。
- ✅ 以词表示的关系：`[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`、`[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ 类：`id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
  PICTURE **不带运算符号**的数据项，只有在每个字符位置都是数字时才是 `NUMERIC` —
  持有 `"+1234"`、`"1.234"` 或 `"12 45"` 的 `PIC X(5)` **不是**数值。*（在 1.62.40
  之前，这项检验把字符当作数字来解析，因此符号、小数点、指数以及周围的空格全都被
  接受了。）*
- ✅ **用户定义的 `CLASS` 操作数可以是一个序数位置** — `CLASS ORDINAL-A-ONLY IS 66`
  指名本机字符集的第 66 个字符 — 而且该操作数可以单独占一行源代码。`ALPHABET` 同理。
- ✅ 符号：`id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 级条件名（朴素的名字本身作为条件）。
- ✅ **把 `TRUE` / `FALSE` 用作操作数**（PowerRustCOBOL 扩展） — 它们是 `1` 和 `0`
  的语法糖，凡是允许出现值的地方都可以用：`IF x = TRUE`、`IF x IS [NOT] FALSE`、
  `IF x NOT TRUE`（不带关系运算符的朴素 `NOT` 形式）、`PERFORM UNTIL x = FALSE`、
  `MOVE TRUE TO x`、`COMPUTE n = n + TRUE`、`INVOKE obj "m" USING TRUE`，以及针对值
  主语的 `WHEN TRUE`。朴素的 `TRUE`/`FALSE` 本身也是一个完整的条件（`IF TRUE`、
  `PERFORM UNTIL TRUE`）。
  ⚠️ 这**不会**改变这两个词原本已有含义的那两处：`SET <88‑name> TO TRUE` 仍然把宿主
  项设为一个满足该条件的值（而不是数字 1），下面的 `EVALUATE TRUE`/`EVALUATE FALSE`
  仍然是标准的分情形语句。
- ✅ `AND` / `OR` / `NOT` 的组合，圆括号（AND 的结合力强于 OR）。
- ✅ **运算符前置的缩写条件** — `a > 1 AND < 9`、`a = 5 OR = 7`（复用前面那次比较的
  主语）。
- ✅ **以字面量为对象的缩写** — `a = 1 OR 2 OR 3`（同时复用主语和运算符；对象是一个
  字面量）。
- ✅ **以标识符为对象的缩写** — `a = b OR c`（其中 `c` 是一个数据项）。跟在比较之后的
  AND/OR 后面的朴素标识符是在运行时解析的：如果它是一个已知的 88 级条件名，就按条件名
  求值，否则它就是对象，即 `a = c`。（紧跟着 `AND` 的标识符保持 AND 的优先级。）
- ✅ **缩写的*对象*前面的 `NOT` 否定的是关系**，而不是对象：`a > b OR NOT c` 等于
  `a > b OR NOT (a > c)`。`NOT <关系运算符>` 这种写法（`AND NOT < x`）是运算符形式，
  未作改动；而开启一个普通条件的 `NOT` — `NOT (…)`、`NOT x = y`、`NOT x NUMERIC` —
  保持它自己的含义。*（在 1.62.42 之前，对象形式被读作「对象非零」，而那只有在对象
  恰好持有零时才给出相同的答案。）*
- ✅ **声明在组项上的条件名检验的是该组的字节。** 组项并不拥有自己的存储 — 它*就是*
  它的子项 — 因此 `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` 比较的
  是该记录所持有的那六个字符。
- ✅ **表意常量会被重复到与另一个操作数同样的大小**，其中也包括写作某个 88 的
  `VALUE` 的那种：对 `PIC X(4)` 宿主而言，`88 B VALUE QUOTE` 是四个引号，而
  `88 D VALUE ALL "BAC"` 是 `"BACB"`。`ALL literal` 在**两个**方向上都会按大小调整 —
  对一个十字符的 `X` 而言，`IF X EQUAL TO ALL "BA"` 比较的是 `"BABABABABA"`，而不是
  用空格补齐的 `"BA"`。

---

## 表达式、字面量、USAGE

- ✅ 算术运算符 `+ - * /` 和 `**`；圆括号；一元的 `+`/`-`。
- ✅ `FUNCTION name ( arg [ , arg … ] )` — **已实现**的内部函数：
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM（种子可选）, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`。
  （日期换算使用标准基准 1601‑01‑01 = 第 1 天。）**COBOL‑85 标准的内部函数全集**均已
  实现。
- ✅ **日期与时间寄存器读取的是 LOCAL 时钟。**
  `ACCEPT … FROM DATE / TIME / DAY / DAY-OF-WEEK` 和 `FUNCTION CURRENT-DATE` 报告的
  都是机器自身的当地时刻，而不是 UTC — 日期也包括在内，而日期在午夜两侧是不同的。
  `CURRENT-DATE` 的最后五个字符带着相对 GMT 的**真实**偏移（`…-0300`），因此程序能够
  判断自己运行在哪个时区。
  ✅ 无法识别的 `FUNCTION` 名是一个**编译错误**，并会指名该函数；当某个真实的函数近
  得足以判断是拼写笔误时，还会给出建议。它过去能通过分析并在运行时返回 **0**，这就
  把一次拼写错误变成了一个自信十足的错误答案（1.62.15）。
- ✅ 字面量：整数、小数、字符串，以及所有表意常量
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **表意常量会填满它的整个接收项**，`HIGH-VALUE` 也是如此 —
  `MOVE HIGH-VALUE TO <PIC X(10)>` 是十个 `0xFF` 字节，而传送到一个组项时会分配到各
  子项。字母数字编辑的接收项仍然会摆放它的插入字符，因此 `PIC XX0XXBXXX` 持有的是
  `FF FF '0' FF FF ' ' FF FF FF`。在 `PROGRAM COLLATING SEQUENCE` 之下，该常量指名的
  是一个普通字符，于是改由那个字符来填充。
  ⚠️ `HIGH-VALUE` 是**字节** `0xFF`，而不是一个字符。读取组操作数、编辑以及所有传送
  路径都逐字节地搬运它，但**引用修饰目前还不是逐字节精确的** — 对一个确实持有 `0xFF`
  的数据项而言，`IF X (1:1) = HIGH-VALUE` 为假。
- ✅ **数值字面量可以以小数点开头** — `.5`、`-.5`、`.000000001`。COBOL‑85 只要求
  字面量不以小数点*结尾*，因此 `5.` 仍然是数字 5 后面跟着一个句子终结符。
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  前导零是有意义且精确的：`.000000001` 是十亿分之一，不是十分之一。在
  `DECIMAL-POINT IS COMMA` 之下，`,5` 同理。把字面量与句末句点区分开来的是**没有
  空格** — COBOL‑85 要求终结符之后必须有一个空格，因此 `MOVE X TO Y.` 绝不会被读作
  一个小数的开头，而 `MOVE X TO Y.5` 会是一个编译错误，而不是被悄悄重新解释。
- ✅ **一致性标示**（`cobolt_semantic::flagging`） — 标准要求一个合规的实现能够告诉
  某个程序，它所使用的特性中有哪些位于所选定的一致性层级之外。有两项分析回答这个
  问题：
  - `flag_obsolete` — COBOL‑85 的**废止要素**集合：IDENTIFICATION DIVISION 的那五个
    可选段落、`MEMORY SIZE`、`ALTER`、带字面量的 `STOP`，以及没有过程名的 `GO TO`。
  - `flag_high_subset` — 一切高于**高子集**的内容，从 `COMPUTE`、`EVALUATE` 和
    `INITIALIZE`，经由 `CORRESPONDING`、引用修饰、限定、`SET … TO TRUE` 和第四个下标，
    一直到把一个*单词*或一个*数值字面量*跨卡片边界延续。（延续一个**字母数字**字面量
    属于子集之内，不会被报告。）

  两者都不是错误检查，也都不会在普通构建中运行：它们所指名的每一种构造都是 RustCOBOL
  既实现也执行的合法 COBOL‑85。它们之所以是各自独立的入口，正是为了让一次普通编译
  永远不会开始就 `AUTHOR` 或 `COMPUTE` 发出警告。NIST 的 `NC302M`、`NC303M` 和
  `NC401M` 验证了它们 — 7、4 和 40 处标示，全部吻合。
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — 用来填充编辑 PICTURE 中货币
  位置的那个字符。它是**取代** `$`，而不是与 `$` 并存，因此一旦程序声明了一个，`$` 在
  那里就不再是 picture 字符：
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  于是 `MOVE ZERO TO FL-LESS` 读作 `      <.00`，而 `MOVE 1234` 读作 ` <1,234.00` —
  这一串浮动符的行为与 `$$$,$$$.99` 完全一致。用**字母**作货币符号也同样有效：
  `CURRENCY SIGN IS "W"` 让 `PICTURE WWWWW` 成为一个五位的浮动货币串，因此 `MOVE 12`
  读作 `  W12`。*（在 1.62.40 之前，一串字母符号会被读作一个单词并遭拒绝，因此只有
  `$` 会浮动。）* 该字面量必须是一个字符，而 COBOL‑85 禁止使用会与 picture 字符或
  分隔符冲突的字符：不能是数字，不能是 `A B C D E G N P R S V X Z` 之一，也不能是
  `space * + - , . ; ( ) " / =` 之中的任何一个。
- ✅ **十六进制字面量** — `X"09"`、`x'0D0A'`（大小写皆可，两种引号皆可）。每**一对**
  十六进制数字对应一个字符，因此数字的个数必须是偶数；个数为奇数或出现非十六进制
  数字，都是格式错误的字面量，会被报告出来，而不是被悄悄重读成挨着一个字符串的单词
  `X`。凡是可以使用带引号字面量的地方都可以使用它们（`DELIMITED BY`、`MOVE`、
  `VALUE`、各种比较）。

---

## DATA DIVISION 子句（接受的声明语法）

- ✅ 级别 `01`–`49`、`77`、`88`；`FILLER`；组项／基本项。`FILLER` 这个词是**可选的** —
  `05 PIC X VALUE ":".` 与 `05 FILLER PIC X VALUE ":".` 一样声明了一个，而且无论哪种
  写法，它都在包含它的那个组项内持有自己的字节和自己的 `VALUE`。
- ✅ `PIC/PICTURE`：`X A 9 S V P` 以及编辑符号（`Z * $ + - CR DB B 0 / , .`）。除非
  `SPECIAL-NAMES. CURRENCY` 指名了别的字符，货币符号就是 `$` — 参见上面的
  **表达式、字面量、USAGE**。**`P` 是一个小数定标位置** — 数据项跨越但并不存储的一个
  数字位置：`PIC S999PP` 持有代表百位的三个数字（`MOVE 12300` 会精确存下它；
  `MOVE 12345` 存下的是 12300），而 `PIC PP99` 持有代表万分位的两个数字。`P` 所占的
  那些位置读回来永远是零，并且在记录布局中**不占用任何字节**。
- ✅ **星号保护会填满整个数据项。** 在一个数字位置全是 `*` 的 picture 中，零值会把
  每一个字符位置都填成星号 — 小数部分的数字、分组用的逗号、固定的 `$`，以及末尾的
  `CR` 或 `DB`，一律如此 — 只留下小数点本身：持有零的 `PIC $**.**CR` 读作 `***.****`，
  而 `PIC *,***.**` 读作 `*****.**`。**非**零值只保护前导零，因此固定的 `$` 会保住
  自己的位置（`-2.34` → `$*2.34CR`）。*（在 1.62.37 之前，`CR`/`DB` 只贡献了一个星号，
  而不是它实际占据的两个字符位置，因此这样的数据项返回时会比它自己的宽度短一个
  字符。）*
- ✅ **数值字面量按其书写的样子搬运它的字符。** 传送到字母数字接收项时，字面量贡献的
  是程序所敲入的那些数字，左对齐并以空格补齐 — `MOVE 2 TO <PIC X(4)>` 是 `"2   "`，
  而 `MOVE 060820000200 TO <六个 PIC 99 子项>` 会把它们填成 `06 08 20 00 02 00`。
  **接收项**的宽度绝不会去补齐字面量；只有字面量自己被写下的宽度才会。*（在 1.62.38
  之前，词法分析器只保留数值，因此前导零会丢失，其后的每个字符都向左移了一位。）*
- ✅ **数值操作数与非数值操作数之间的关系是非数值的**（COBOL‑85 VI‑89 6.15.4 GR2）。
  数值操作数被当作已经传送到一个**与它自身同样大小**的字母数字项来处理，这会搬运它的
  各字符位置，而**不搬运它的运算符号**：持有 `-123456789012345678` 的 `PIC S9(18)` 与
  持有 `"123456789012345678"` 的 `PIC X(18)` 比较结果为**相等**。有三个条件限定这条
  规则 — 数值操作数必须是**整数**；「非数值」由**声明**决定，因此一个在组 `MOVE` 之后
  持有字符的 `PIC 99` 子项仍然是数值的 — 而**组项**无论其子项为何都是非数值的，因此
  持有 12345 的 `PIC 9(5)` 对上一个持有 `"0000012345"` 的十字节组项，得到的是
  `"12345     "`，二者不相等；另外 `ALL literal` 取另一个操作数的大小。*（在 1.62.38
  之前，只要文本那一侧恰好能被解析成数字，比较就是代数式的。）*
- ✅ **数值 MOVE 的高位截断。** 接收项在两端都精确地持有它所声明的位数：
  `01 M PIC 99V999.  MOVE 123.45 TO M.` 留下 `23.450`。算术会先检验接收项的容量，
  因此带 `ON SIZE ERROR` 的语句会改为保留它原来的值。
- ✅ **组表是按出现来寻址的。** `MOVE VALUES-1 TO GRP-1 (2)` 分配到该次出现自己的各
  子项（`ELEM1 (2,1) … ELEM1 (2,4)`），而读取 `GRP-1 (2)` 连接起来的恰好就是那些。
  外层的 `01` 记录是**所有**出现的字节，因此 `MOVE GRP-TAB1 TO GRP-TAB2` 会复制整张表。
- ✅ **索引名、字面量和相对索引可以混用作下标。** `ELEM1 (IN1, 1)`、`ELEM1 (1 IN2)`、
  `ELEM1 (IN1 +3)` — 紧贴着数字的符号是一个带符号字面量，用来开启下一个下标 — 而
  `ELEM1 (IN1 - 1, 3)`（运算符两侧都有空格）则是相对索引。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}`（以及 `COMP-4`→COMP、`COMP-X`→COMP-5）。
- ✅ `VALUE`（数值／带符号／字母数字／表意／`ALL`）。**`VALUE ALL "literal"` 会把它的
  单元重复贯穿整个数据项** — `PIC X(6) VALUE ALL "ABC"` 是 `"ABCABC"`，而
  `PIC X(9) VALUE ALL "XY"` 是 `"XYXYXYXYX"`。*（在 1.62.40 之前，只有单字符的表意
  常量会填满它的数据项，而 `ALL "literal"` 会让它持有空格。）*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES` — 对同一批字节的、**活的**第二种读法。它不增加存储（因此不会把包含它
  的那个组项撑宽），而通过任一描述所做的写入，透过另一描述都能看到：
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` 之后可以透过 `RESULT-A` 读回来。
  ⚠️ **注意事项：** 展开后超过 256 个存储位置的覆盖（比如一张被重定义的 10×10×10 的
  表）会改为保留按描述划分的存储 — 因为每次写入都去刷新它，将意味着把一千次出现走
  两遍。
- ✅ **覆盖可以嵌套。** 位于一条本身已被重定义的记录内部的 `REDEFINES`，无论多深都能
  从两个方向到达：透过一个 01 级的重定义写入两个字节，会触及被重定义的那条记录、它
  内部某个组项的 `REDEFINES`，以及*那个*内部某个数据项的 `REDEFINES` — 包括声明在最
  内层那一项上的某个 88。每个描述在每次写入时都会被重新生成一次。*（在 1.62.42 之前，
  属于不止一个覆盖的键只保留最后声明的那一个，而一个单独的守卫会在第一跳之后就把链条
  截断。）*
- ✅ **没有名字的描述仍然是一个描述。** `02 FILLER REDEFINES <item>.` 以它自己并无
  名字的方式重新描述其目标的字节，而对目标的写入透过它的各子项都能看到。多个子项按
  布局顺序把那些字节分摊开来 — 这个覆盖*并不是*它第一个子项的别名。对同一个数据项做
  两次 `FILLER REDEFINES` 就是两种互相独立的读法，各自都从目标的**第一个**字节开始。
  *（在 1.62.36 之前，一个没有名字的重定义组项根本没有被赋予存储键，因此无论目标被
  填成什么样，它的子项读出来都是空格。）*
- ✅ **覆盖内部重复的名字**会解析到程序其余部分所到达的同一块存储：在两个不同组项下
  声明的 `TAB-A`，会为每个声明各保留一种读法。*（在 1.62.36 之前，覆盖的初始副本是
  从一条缺少外层限定符的路径上建索引的，而能分辨这一点的只有重复的名字 — 于是恰恰是
  最需要限定符的那种情形丢掉了它。）*
- ✅ `JUSTIFIED [RIGHT]` — 在*字母数字*或*字母*数据项上**按右对齐存储**。比接收项窄
  的发送方在左侧被补齐；比接收项宽的发送方保住它的**右**端，丢掉最左边的字符 — 与
  通常的规则相反。*（在 1.62.40 之前，这个子句只对字母数字项被记录下来，因此
  `PICTURE A(5) JUSTIFIED RIGHT` 能通过分析，随后却像其他数据项一样左对齐。）*
- ✅ `SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL` — 均被接受；
  `SIGN … SEPARATE` 目前还不改变数据项的存储方式。
- ✅ **01 级的 `REDEFINES` 可以描述比它所重定义的那个数据项更多的存储**，而超出那个
  数据项末尾的字节，归属于长度足以指名它们的那个描述。透过较短的描述写入，不会动到
  较长描述的尾部。
- ✅ **`REDEFINES` 覆盖会搬运被重定义数据项的字节**，传给数值的对应项时也是如此：
  对持有 `"00ABCDEFGHI  4321 "` 的 `X(18)` 所做的 `PIC S9(18)` 覆盖会把那些字符读
  回来，而 `IS NUMERIC` 对它们的回答是**否**。当那些字节确实拼出数字时，按数值的读法
  不变。
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **真正的条件名**：88 级绑定到它的
  宿主数据项；检验时会拿宿主去比对那些 VALUE ／范围，而 `SET 88-name TO TRUE` 会把一个
  满足它们的值存入宿主。
- ✅ **一个条件名可以在不止一个组项下被声明，而 `OF`/`IN` 能把它们分辨开来** — 与数据
  名的情形完全一致，而且中间层级可以省略：
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  下标属于宿主数据项，因此它选择那些 VALUE 是针对哪一次出现来检验的。对一个重复的
  条件名做**不加限定**的引用，在 COBOL‑85 中是有歧义的；运行时取第一个声明，这与它对
  有歧义的数据名所用的规则相同。
- ✅ `USAGE INDEX` 声明一个整数索引寄存器（`SET`/`SEARCH` 会用到它）；`USAGE POINTER`
  — 参见上面的**指针**。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — 一个重新分组的别名；读取时把
  所覆盖的各项连接起来，写入时按字段宽度分配。
  - ✅ **一个 66 由它所重新分组的那条记录来限定**，就像一个数据项由它上面的组项来限定
    一样，因此同一个 66 的名字可以每条记录声明一次，并用 `OF`/`IN` 加以分辨：
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`。这在读取和写入时同样有效，而且
    66 会胜过一个恰好与它同名的普通数据项。`RENAMES` 子句的各操作数在同一条记录内解析，
    因此一个重复的 `NAME-2` 指的是本记录的那一个。
  - ✅ **被覆盖的表会贡献它的每一次出现**，而不只是第一次：对于持有
    `03 T PIC XXX OCCURS 5` 的 `TABLE-2` 而言，`66 R RENAMES ITEM-1 THRU TABLE-2` 的
    宽度是 20 个字符。
  - ✅ **恰好覆盖一个数据项的 66 *就是*那个数据项** — 同样的 PICTURE、同样的类别、
    同样的存储。当 `W` 是 `PIC 9(4)` 时，`66 R RENAMES W` 就是一个四位的数值项，因此
    在里面装着 8000 的情况下 `ADD 3500 TO R` 会引发 `ON SIZE ERROR`，并把它原样留下。
- 各节：`WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`；`SCREEN` 能被分析但
  不被执行。

---

## 仍然不支持 — 当前的规避清单

> **2026‑08‑25 更正。** 本节过去的开头是「COBOL‑85 的动词／子句集合已**完全覆盖**。」
> 运行 NIST CCVS85 套件推翻了这句话：那一天**范围内 434 个程序中有 102 个失败**，而且
> 都失败在本文档并未列为缺口的构造上 — 作为分隔符的逗号和分号、`FUNCTION x(ALL)`、
> `CLOSE … WITH LOCK`、位于 B 区的 `COPY`、IDENTIFICATION 的注释条目、section 的优先
> 级编号、以数字开头的数据名，以及 — 直到 1.62.10 为止 — 带前导小数点的数值字面量。
> 验证套件就是为此而存在的。每一个缺口现在都已在
> [`specs/nist/`](../specs/nist/README.md) 中写成规格，并在上面的
> [记分板](#-一致性是测量出来的不是声称出来的--nist-ccvs85)中被追踪。

下面这份清单是**有意**不纳入范围的内容，与上面那些 NIST 缺口形成对照 — 后者是正在
逐一处理的缺陷：

1. **屏幕 `ACCEPT` 的输入编辑** — `DISPLAY … AT/WITH` 和 `ACCEPT … AT` 在 CLI 模式下
   （通过 ANSI）是会执行的，但 SCREEN SECTION 在字段层面的完整编辑（自动跳格、字段
   校验、颜色映射）在 GUI 模式下已**由窗体设计器取代**。
2. **跨*进程*的文件共享** — `OPEN … SHARING/WITH LOCK`、`READ … WITH [NO] LOCK` 和
   `UNLOCK` 会被分析，并驱动 INDEXED 引擎在本次运行内的记录锁，但这些锁并不会在彼此
   独立的操作系统进程之间强制生效（单一运行单元模型）。
3. **面向对象 COBOL**（类／方法定义） — 对 COBOL 对象而言 `INVOKE` 不执行任何操作
   （它只驱动 GUI／运行时对象）。
4. ✅ **已解决（1.62.15）。** 无法识别的内部函数名过去会悄悄返回 **0**，于是程序就从
   一个拼写笔误算出了一个自信十足的错误答案。现在它是一个**编译错误**，会指名该函数，
   并在存在足够接近的匹配时建议最相近的那个真实函数
   （`cobolt-semantic/src/resolver.rs`，`Expr::FunctionCall`）。之所以保留在这里，是
   因为「静默的零」这种形态正是第 5 和第 6 条仍然带着的陷阱。
5. ⚠️ **无效的 `ACCESS MODE` / `ORGANIZATION` 取值会被无声吞掉** — 同一个陷阱再次出现，
   而这一个是由用户一次普通的拼写笔误触发的。`ACCESS MODE IS` 只接受 `SEQUENTIAL`、
   `RANDOM` 或 `DYNAMIC`（`INDEXED` 是一种*组织方式*，不是访问模式），但 SELECT 子句
   的分析器只检验这三个，其他一切都落进了「跳过一个未知记号」那个通用分支，于是该文件
   悄悄保留了默认的 `SEQUENTIAL`，并在运行时行为出错，而不是编译失败。
   `ORGANIZATION IS` 的形态完全相同（`cobolt-parser/src/parser.rs` 中的
   `Token::Access` 分支，以及它上面那个组织方式分支）。两者都应当抛出一个明确的编译期
   错误，并指名那个有问题的词。**NIST 的任何模块都永远不会捕捉到这一点** — 套件只写
   合法的子句，因此即便这个缺口依然敞开，每个模块也都能以 100 % 收尾。它是一个针对
   用户拼写笔误的陷阱，需要的是一个专门的测试，而不是某个模块的分数。
6. ⚠️ **`ALPHABET … IS EBCDIC` 会被接受，但仍让本机（ASCII）排序生效。** 字面量形式的
   短语（`"A" THRU "H" "I" ALSO "J" …`）、`NATIVE`、`STANDARD‑1` 和 `STANDARD‑2` 都
   已实现，并且确实驱动 `PROGRAM COLLATING SEQUENCE`；唯一缺的是 EBCDIC 表，而指名它
   会静默地得到 ASCII 顺序。与第 4–6 条同属一类陷阱。
7. **通信模块和 Report Writer** — 参见
   [上面的 N/A](#-na--哪些内容不在-rustcobol-的范围内以及为什么)。

> **已解决（1.5.0）：** 扁平的数据模型变成了层次化／按出现处理的模型，从而解开了
> **CORRESPONDING**、**限定名**、**表下标**和 **`SEARCH`**。
> **已解决（1.6.0）：** 多接收项的 `MULTIPLY`/`DIVIDE` + 每个接收项各自的 `ROUNDED`；
> `EXIT PERFORM/PARAGRAPH/SECTION`；`CALL NOT ON EXCEPTION`；
> `INSPECT TALLYING REPLACING` 的合并使用 + `BEFORE/AFTER INITIAL`；日期／`ANNUITY`
> 内部函数；以字面量为对象的缩写；`EVALUATE ALSO`/`WHEN NOT`；真正的 88 级条件名；
> `PERFORM para VARYING`；以及带 `RELEASE`/`RETURN` 的 `SORT`/`MERGE` 运行时。
> **已解决（1.7.0）：** 以标识符为对象的缩写；`INITIALIZE … REPLACING`；
> `66 RENAMES`；指针（`USAGE POINTER`、`SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`）；`ALTER` / `UNLOCK`；忠实的 `NEXT SENTENCE`；余下的标准内部函数；以及
> 扩展的屏幕 `ACCEPT`/`DISPLAY`（在 CLI 模式下执行）。
> **已解决（1.7.1）：** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS`（连同配对的
> `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` 寄存器）。
> **已解决（1.7.2）：** `OPEN … SHARING/WITH LOCK`、`READ … WITH [NO] LOCK`、
> `UNLOCK`（释放 INDEXED 记录锁），以及 `CANCEL program`。
> **已解决（1.8.0）：** `COMMIT` / `ROLLBACK` 成为由程序控制的 INDEXED 文件事务
> （内存与磁盘两种引擎；磁盘上有真正的撤销日志）。

.<<

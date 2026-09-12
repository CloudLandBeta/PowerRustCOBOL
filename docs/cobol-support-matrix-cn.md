<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.135 -->

# PowerRustCOBOL 支持矩阵

**这份文档的用途：** 一个可以一眼扫过的地方，回答*"PowerRustCOBOL 能不能做 X，
而 X 是标准 COBOL，还是这个平台加的东西？"*。每一项能力都是一行。没有散文式的罗列
——如果某样东西被支持，它就有一行可以指给你看。

这是**总览**。细节由两份配套文档承担：

| 文档 | 它回答什么 |
|---|---|
| [`cobol85-supported-syntax-en.md`](cobol85-supported-syntax-en.md) | 每条语句的**哪种写法**真正被词法分析器、语法分析器和运行时接受，以及 NIST CCVS85 一致性记分板 |
| [`cobol85-verb-test-matrix-cn.md`](cobol85-verb-test-matrix-cn.md) | 每个动词**要测什么** |
| [`developers-guide-en.md`](developers-guide-en.md) | 如何用这一切来构建应用程序 |

---

## 怎么读这些表

每一行能力都会对照三种来源打上标记，然后给出一个状态。

| 列 | 含义 |
|---|---|
| **85** | 由 **COBOL-85** 定义（ANSI X3.23-1985，在注明之处包括 1989 年的内建函数修订） |
| **20xx** | 由**更晚的 ISO 标准**定义 —— COBOL 2002 / 2014 / 2023，以及当前面向 2026 年起草中的内容 |
| **PRC** | 一项 **PowerRustCOBOL 扩展** —— 不在任何 COBOL 标准之中 |
| **状态** | 本实现对它的处理 |

一项能力可以在不止一个来源列里被标记：被后续标准扩展过的 COBOL-85 特性在两列中都
是 `●`，而**说明**列会讲明后续标准加了什么。

**来源标记：** `●` 在此定义 · `○` 在此扩展或澄清 · `—` 不在该标准之中。

**状态标记：** `✅` 已支持 · `🚧` 部分或简化 · `⛔` 已计划、尚未实现 · `🚫` 按
设计不在范围内，永远不会实现。

> **老实话。** PowerRustCOBOL 瞄准的是一个务实的、面向应用的子集，外加可视化的
> RAD 扩展。它**不是**经过认证的 COBOL-85 实现。一致性是对照官方 NIST CCVS85 套件
> *测量*出来的，而不是自行宣称的——参见
> [记分板](cobol85-supported-syntax-en.md)。

---

## 1. 源码格式与程序结构

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| 固定格式源码，**宽松版**（`fixed-relaxed`） | ● | ○ | ○ | ✅ | **默认值。** 序号区与指示符列予以尊重，但一行会延伸到开发者实际写到的地方——不在第 72 列截断。窗体生成的 `.cbl` 和 `EXEC RUST` 块需要它 |
| 固定格式源码，**COBOL-85 经典参考格式**（`--source-format=fixed`） | ● | ○ | — | ✅ | 所有列规则一律生效：1–6 为序号，7 为指示符（`*` `/` 注释、`-` 续行、`D` 调试行），8–72 为源码，**73–80 被丢弃**，续行的拼接也按标准进行，包括被续写的字母数字字面量。NIST CCVS85 的卡片映像套件就是用它写的。**只能显式选择，绝不靠探测** —— 把这些规则套到并非为之而写的源码上，会悄悄删掉代码 |
| 自由格式源码 | — | ● | — | ✅ | COBOL 2002（`--source-format=free`） |
| 源码格式开关 —— `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | 也可用 `COBOLT_SOURCE_FORMAT`；`auto` 会查看开头几行，且绝不会选中严格格式 |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ |  |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ |  |
| DATA DIVISION | ● | ○ | — | ✅ |  |
| PROCEDURE DIVISION | ● | ○ | — | ✅ |  |
| 嵌套程序 | ● | ○ | — | ✅ |  |
| 一个文件中包含多个顺序排列的程序单元 | ● | ○ | — | ✅ |  |
| `COPY` / `REPLACE` 副本库 | ● | ○ | — | ✅ | 伪文本与单词替换、嵌套 `COPY`、`REPLACE OFF`；在源码旁按不区分大小写的方式解析 `.cpy`/`.cbl`/`.cob` |
| `REPOSITORY` 段 | — | ● | ○ | ✅ | 用于类时是 COBOL 2002；PowerRustCOBOL 还在这里绑定 **Rust FFI** 类型 |
| 用 `EXEC RUST … END-EXEC` 内联 Rust | — | — | ● | ✅ | 编译进二进制文件；错误会报在开发者自己的 COBOL 行列上 |

## 2. DATA DIVISION 与数据描述

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ |  |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ |  |
| FILE SECTION | ● | ○ | — | ✅ |  |
| SCREEN SECTION | ● | ○ | — | 🚧 | 带 `AT`/`WITH` 的扩展 `ACCEPT`/`DISPLAY` 在 CLI 模式下经 ANSI 执行；字段级的屏幕编辑在 GUI 模式下由可视化窗体设计器取代 |
| COMMUNICATION SECTION（`CD`，消息控制） | ● | — | — | 🚫 | 远程通信处理；在后续标准中已废弃 |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | 按设计不在范围内 |
| 带 `(n)` 重复的 `PICTURE` X / A / 9 / S / V | ● | ○ | — | ✅ |  |
| 数字编辑 PICTURE（`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`） | ● | ○ | — | ✅ | 零抑制、星号保护、固定与浮动的 `$` 及符号 |
| `USAGE DISPLAY` | ● | ○ | — | ✅ |  |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ |  |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | 浮点数；一项厂商扩展，后来被标准化为 `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ |  |
| `USAGE COMP-5` | — | ○ | ● | ✅ | 原生二进制；厂商扩展 |
| `USAGE INDEX` | ● | ○ | — | ✅ |  |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002；别名可读**也**可写 |
| 定长 `OCCURS` | ● | ○ | — | ✅ |  |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ |  |
| `INDEXED BY` | ● | ○ | — | ✅ |  |
| 层级号 01–49、77 | ● | ○ | — | ✅ |  |
| 66 层 `RENAMES` | ● | ○ | — | ✅ |  |
| 88 层条件名 | ● | ○ | — | ✅ | 包括 `SET … TO TRUE` |
| `VALUE` 子句 | ● | ○ | — | ✅ |  |
| 组项、`FILLER` | ● | ○ | — | ✅ |  |
| `REDEFINES` | ● | ○ | — | ✅ |  |
| 形象常量（`SPACES`、`ZEROS`、`HIGH-`/`LOW-VALUES`、`QUOTES`、`NULLS`） | ● | ○ | — | ✅ |  |

## 3. PROCEDURE DIVISION —— 动词

| 动词 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | 按名字匹配组项下的子字段 |
| `DISPLAY` | ● | ○ | — | ✅ | 数值按 PIC 的完整宽度呈现 |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ |  |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | 多个接收方，`ROUNDED` 逐个接收方生效 |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | 多个接收方，`ROUNDED` 逐个接收方生效 |
| `COMPUTE` | ● | ○ | — | ✅ | 多个接收方，`ROUNDED` 逐个接收方生效 |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ |  |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ |  |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ |  |
| 行内 `PERFORM`、`TIMES`、`UNTIL`、`TEST BEFORE/AFTER`、`VARYING … AFTER`、`THRU` | ● | ○ | — | ✅ |  |
| `PERFORM para VARYING`（行外） | ● | ○ | — | ✅ |  |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ |  |
| `ALTER` | ● | ○ | — | ✅ | 在 COBOL-85 中属于废弃要素 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | 语义忠实；在 COBOL 2002 中属于废弃要素 |
| `CONTINUE` | ● | ○ | — | ✅ |  |
| `EXIT` | ● | ○ | — | ✅ |  |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ |  |
| `GOBACK` | — | ● | — | ✅ | 厂商扩展，在 COBOL 2002 中被标准化 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ |  |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | COBOL 2002 的指针 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | 区分类别，并递归进入组项 |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ |  |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` 合用 |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | 驱动表的索引项，执行第一个匹配的 `WHEN`，否则走 `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`、`INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` 与 `RETURNING` 属于 COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ |  |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ |  |
| `INVOKE` | — | ● | ○ | 🚧 | COBOL 2002 的面向对象特性。对 **GUI 与运行时对象以及 Rust FFI 插件**提供支持；用户自定义的类／方法定义尚未实现 |
| `UNLOCK` | — | ● | — | 🚧 | 驱动运行单元内的记录锁；不在操作系统进程之间强制执行 |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | 对 INDEXED 文件的程序控制事务，带有真正的撤销日志 |
| 面向对象的 `CLASS-ID` / `METHOD-ID` 定义 | — | ● | — | ⛔ | 计划中 |

## 4. 条件与表达式

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| 关系条件、类别条件、符号条件与条件名条件 | ● | ○ | — | ✅ |  |
| 缩写的复合关系，运算符前置（`a > 1 AND < 9`） | ● | ○ | — | ✅ |  |
| 缩写的复合关系，宾语为字面量（`a = 1 OR 2 OR 3`） | ● | ○ | — | ✅ |  |
| 缩写的复合关系，宾语为标识符（`a = b OR c`） | ● | ○ | — | ✅ |  |
| 引用修改 `item(start:length)` | ● | ○ | — | ✅ | 对任意操作数都可读**且**可做切片写入 |
| 运行期表下标 `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | 按出现项分配存储，下标可变 |
| 限定名 `id OF/IN group` | ● | ○ | — | ✅ | 在多个组项之下声明的同名末端项，会解析到各自独立的存储 |
| 符合 COBOL 规定的字母数字比较（以空格补齐） | ● | ○ | — | ✅ |  |
| **精确的定点运算** | ● | ○ | ○ | ✅ | 使用 `i128` 整数尾数，不经 `f64` 往返：标准的 18 位精度和**扩展的 31 位精度**都保持精确 |
| 简洁的属性表达式（`Output::Value`） | — | — | ● | ✅ | 在公式里直接读写控件属性，不需要任何临时的工作存储项 |

### 4.1 数据项上的值方法

`item::Method(args)` 是在**一个普通数据项的值**上调用方法——`PIC X` 字段、组项、
表的出现项、引用修改得到的切片或算术表达式——而不只是在控件上。这一切都不是标准
COBOL。

凡是能写表达式的地方都能用：作为 `MOVE` 的来源、在 `COMPUTE` 里、在条件内部，以及
直接写在 `DISPLAY` 中。方法可以**串接**：`WS-TEXT::Trim()::Len()`。

| 方法 | 返回 | 状态 | 说明 |
|---|---|:--:|---|
| `Trim()` | 文本 | ✅ | 去掉前导和尾随空格 |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | 文本 | ✅ | 同一个方法可接受的三种写法 |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | 文本 | ✅ |  |
| `Replace(from, to)` | 文本 | ✅ | 所有出现处 |
| `Len()` · `Length()` | 数值 | ✅ | 是**字段的**长度，因此装着 `hello` 的 `PIC X(20)` 会回答 `20`。想要内容的长度，请串接 `::Trim()::Len()` |
| `Split(sep)` | 文本 | ✅ | **第一个**字段 |
| `Split(sep)(n)` | 文本 | ✅ | 第 *n* 个字段，从 1 开始计。该下标只在接收方为数据项时才被接受 |

| 接收方 | 状态 | 说明 |
|---|:--:|---|
| 数据项（`PIC X`、组项、`01`/`77`） | ✅ | 常规情形 |
| 表的出现项、引用修改、限定名、算术表达式 | ✅ | 求值器接受 |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | 解释器接受字面量作为接收方，但语法分析器不接受：字面量之后的 `::` 是语法错误。请先把该字面量赋给一个数据项 |

### 4.2 在 COBOL-85 只允许写一个项的地方写表达式

COBOL-85 把大多数发送位置限制为一个标识符或一个字面量。RustCOBOL 在那里改为求值一个
完整的表达式，正是这一点省掉了标准逼你声明的那个临时工作存储项。

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`。标准只允许标识符或字面量作为发送字段 |
| `SET target TO <expression>` | — | — | ● | ✅ | 等价于 `COMPUTE` 形式；目标可以是数据项，也可以是作为左值的控件属性 |
| `STRING <expression> … INTO` | — | — | ● | ✅ | 发送项可以是算术表达式（`STRING WS-N * 2 …`）或值方法调用（`STRING WS-A::UpperCase() …`）；`DELIMITED BY` 及其余部分仍按标准 |
| **类型推断** —— 读取 `Ctrl::Property` 得到的是一等的带类型值 | — | — | ● | ✅ | 数值或文本的类型会贯穿整个表达式，因此一个属性可以直接进入算术、条件或发送位置，**中间无需任何 `PIC` 项**：`IF Slider-1::Value > 50`、`COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`。看起来像数字的属性值会按数值读回，因此比较和算术仍是代数式的，而不是逐字符的 |

## 5. 内建函数

COBOL-85 的内建函数集随 **1989 年修订**（ANSI X3.23a-1989）一同到来；由 COBOL 2002
及其后添加的函数在 `20xx` 列中标出。下面这些全部已经实现。

| 分组 | 函数 | 85 | 20xx | PRC | 状态 |
|---|---|:--:|:--:|:--:|:--:|
| 长度与字符 | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| 长度与字符（后加） | `BYTE-LENGTH`, `LENGTH-AN`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| 大小写与文本 | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| 文本（后加） | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| 数值转换 | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| 数值转换（后加） | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| 算术 | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| 排序 | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| 统计 | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| 三角函数与对数 | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| 组合数学 | `FACTORIAL` | ● | ○ | — | ✅ |
| 财务 | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| 日期与时间 | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. 文件 I/O —— 组织方式与访问

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | 定长记录 |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | 以换行结尾的文本；写入时丢弃尾随空格 |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | 内建的、无依赖的 ISAM 引擎 |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | 自有引擎（`cobolt-runtime/src/relative.rs`，`PRCREL1` 容器，磁盘与 MEMORY）。`RELATIVE KEY IS` 以从 1 起的整数记录号寻址记录；三种访问模式齐备；七个文件动词都会据此分派。NIST 的 **RL 模块两条轴线均已完成** —— 编译 35/35，执行 34/34，354 条断言，0 失败（引擎 1.62.76，模块 1.62.77） |
| `RELATIVE KEY IS data-name`（含省略 `KEY` 的写法） | ● | ○ | — | ✅ | 省略 `KEY` 一词的 `RELATIVE data-name` 子句指的是键，而不是一个单纯的组织方式子句 |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | 三者都能执行 |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | 磁盘上按键升序排列 |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ |  |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ |  |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` 属于 COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ |  |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | 包括 `GREATER/LESS THAN` 和 `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ |  |
| `FILE STATUS` 代码 | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | 会被解析并随语句携带，但只是**建议性的**——只有一个运行单元，因此不存在争用 |
| `OPEN … WITH LOCK`（以独占方式打开文件） | — | ● | — | 🚧 | 同样：在单运行单元模型下被接受，但只是建议性的 |
| `READ … WITH LOCK` | — | ● | — | ✅ | 在 `I-O` 之下引擎已经持有该记录；这个短语只是表明意图 |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | 会真正释放引擎在 `I-O` 之下取得的锁——这是目前唯一在运行时有实际效果的锁短语。`UNLOCK` 和其他动词一起放在 §3 |
| 跨进程的文件共享与记录锁强制 | — | ● | — | ⛔ | 计划中；目前是单运行单元模型 |

## 7. 文件 I/O —— INDEXED 引擎（PowerRustCOBOL）

本节中的一切，都是围绕上文标准 `ORGANIZATION IS INDEXED` 行为的平台扩展。细节见
[`indexed-file-format-cn.md`](indexed-file-format-cn.md)、
[`indexed-file-internals-cn.md`](indexed-file-internals-cn.md) 和
[`indexed-redb-engine-cn.md`](indexed-redb-engine-cn.md)。

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **默认的存储模式。** 记录和索引都存放在 `ASSIGN` 指定的文件里并按需读取，因此即使文件非常大，内存占用也保持有界。自 1.62.73 起由抗崩溃的 redb 引擎提供服务；仍可用 `--indexed-engine rust` 取到更早的分页 B+tree |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | 整个文件放在内存中，关闭时持久化到 `ASSIGN` 指定的路径 |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | 无依赖的 RLE；对典型 COBOL 记录中成片的填充字符，压缩率远超 50 % |
| 由程序控制的 `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | 真正的撤销日志，内存与磁盘两种引擎都有 |
| 运行单元内的记录加锁 | — | ○ | ● | ✅ | 参见上面关于跨进程的说明 |
| 可选择的引擎（`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`） | — | — | ● | ✅ | 也可用 `COBOL_INDEXED_ENGINE`；各引擎行为一致。自 1.62.73 起 **`redb` 是默认值**（`cobolt-runtime/src/indexed.rs:126`） |
| 抗崩溃的 `redb` ACID 引擎 | — | — | ● | ✅ | O(1) 的 OPEN（20 万条记录时约 5 ms）、按工作集占用内存（≥2.5 亿条记录）、断电后索引不会损坏 |
| 自描述的 `PRCIDX1` 容器 | — | — | ● | ✅ | 内嵌记录格式与键描述符；打开时的严格校验把模式不匹配映射为 `39`，文件缺失映射为 `35`。与 Fujitsu 并非逐字节兼容 |
| 按文件的事务日志（`--indexed-log basic\|full`） | — | — | ● | ✅ | logfmt，或可直接用于 Grafana/Loki 的 NDJSON —— 参见 [`observability-cn.md`](observability-cn.md) |

## 8. 运行时集成

从 COBOL 一侧以运行时 `CALL` 和 `INVOKE` 触及。这些都不是标准 COBOL；正是它们让这门
语言能够用于现代应用程序。

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | 三者共用同一套 CALL 接口；后端由连接字符串决定。**不需要系统库**——不从宿主链接任何东西——但“纯 Rust”只对其中两个成立：`postgres` 和 `mysql` 是，而 `rusqlite` 被固定为 `features = ["bundled"]`，会通过 `libsqlite3-sys` 编译 **SQLite 的 C 合并源码**。（那次 C 构建也正是 `test_external_crates_e2e` 在嵌套的 `cargo build` 中间歇性失败的原因。）参见 [`database-runtime-cn.md`](database-runtime-cn.md) |
| **SQL 结果集** —— `Fetch()`、`ColumnNames()`、`ColumnCount()`、`ColumnName(n)` | — | — | ● | ✅ | `Fetch()` 以制表符分隔返回下一行，取尽后返回空，因此它自己就能终止循环；`ColumnNames()` 按 SELECT 的顺序给出结果集的列名，即使一行都没匹配上也一样。而 `CALL` 接口则是按下标一次读当前行的一列——这两种遍历方式不可在同一个句柄上混用 |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | 自定义请求头 |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ |  |
| **图表** —— 柱状 / 折线 / 饼图 / 面积 / 散点 / 环形 | — | — | ● | ✅ | 绑定到 COBOL 表 |
| **文本文件** —— `COBOL-APPEND-FILE`、`COBOL-WRITE-FILE` | — | — | ● | ✅ |  |
| **定时器** | — | — | ● | ✅ |  |
| **AI 智能体对象挂钩** | — | — | ● | ✅ |  |
| **Rust FFI 插件** | — | — | ● | ✅ | 在 `REPOSITORY` 下声明的模块，经由 `INVOKE` 或直接的属性映射分派 |
| **用户过程** | — | — | ● | ✅ | 可在 IDE 中编辑、以 `CALL "PROCEDURE-NAME"` 调用的共享 COBOL 过程 |

## 9. 明确不在范围内

这些不会被实现。把它们列出来，是为了让答案可以被找到，而不是干脆缺席。

| 能力 | 85 | 20xx | PRC | 状态 | 说明 |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION（`CD`，消息控制／远程通信处理） | ● | — | — | 🚫 | 在后续标准中已废弃；没有现代用途 |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | 已被平台自身的报表功能和数据绑定取代 |
| ActiveX / OLE / COM 控件 | — | — | — | 🚫 | 与特定平台绑定，不可移植 |

---

## 10. 平台本身

这些不是 COBOL 语言特性，而是 IDE、编译器以及围绕它们的工具链。完整的走查见
[开发者指南](developers-guide-en.md)。

### 10.1 IDE

| 能力 | 状态 | 说明 |
|---|:--:|---|
| 可视化窗体设计器 | ✅ | 带多种主题（**Liquid Glass**、**Cobalt Steel**）的设计画布、网格吸附、控件与画布的拖拽缩放、多选对齐、层叠顺序 |
| 统一渲染引擎 | ✅ | 设计器、预览、运行中的应用程序和编译出的二进制之间做到像素级一致 |
| 控件目录 | ✅ | 横跨 Common、Container、Data、Graphics、Menu、Non-visual 与 Charts 的 **43 个控件**，外加一个由插件提供的 `Custom` 类型 |
| 通用圆角与圆角裁剪 | ✅ | 嵌套的子元素通过角部缺口遮罩，裁剪到父级的圆角边框上 |
| 逐控件的 `Transparency` | ✅ | 0 = 不透明 … 100 = 透视；在淡化表面、边框和阴影的同时，文字、字形和边线仍保持可读。相对于其后景低于 WCAG AA 的标题文字，会翻到能读得清的那一极 |
| Animator 控件 | ✅ | 原生渲染 **GIF / WebP / APNG** |
| Knob、Gauge、Switch、FileDropZone、Maps、Web Search | ✅ | 带双极填充的旋钮；自动划分警告区与危险区的径向／线性／环形 KPI；拖放或原生选择器 |
| 高级菜单编辑器 | ✅ | 可视化树形编辑器、分为 37 个类别的 **1112** 个内建矢量图标、层级嵌套，以及配置完整性的 HMAC 签名 |
| 数据绑定与控件数组 | ✅ | 直接绑定到 SQL 与数据源；**可视化重复组**会依据运行时 `DataSource` 的行数展开 GroupBox/Panel 数组 |
| 可视化校验与窗体检查器 | ✅ | 对格式错误的处理器、不完整的绑定和布局异常给出实时错误标记；`rcrun` 的进程管理器实时跟踪 CPU 百分比、RSS、日志和线程数 |
| 窗体调试器 | ✅ | 独立的置顶窗口：断点，单步进入／跳出／跳过，变量检查器，以每秒 1–10 行的速度动画回放 |
| 智能体式 AI 助手网格 | ✅ | **rig-core** LLM 编排器（Ollama、OpenAI、Groq、阿里云百炼及其他云 API）运行 Dev Agent、Editor Assistant 和 History Compactor，并带有实时的可观测性日志与 `↑input ↓output` 的 token 读数 |
| 编排者 Grace | ✅ | 把一个请求拆解开，将每个任务路由给负责它的专家，并强制执行一对一的 **Pedantic 评审** —— 没有任何专家可以批准自己的工作 |
| 带 RAG 的分块知识库 | ✅ | 按主题一条记录建立索引；随产品预先嵌入，支持 GPU 并带有低发热的 CPU 回退，以及 **File → Reindex Knowledge Bases** |
| 窗体生命周期与窗口管理 | ✅ | 一个被指定的**主窗体**启动应用程序；各窗体自身的外框与状态得到尊重；`OpenFormSync`/`OpenFormAsync`；窗口位置是设计期属性；按项目设置进场与退场效果 |
| 多窗口运行时 | ✅ | 预览与运行画面位于各自独立的操作系统视口中（egui 多视口） |
| 国际化界面 | ✅ | 6 种界面语言：英语、西班牙语、葡萄牙语、日语、汉语、法语 |
| 系统字体选择器 | ✅ | 任何已安装的字体，都以其自身字形显示，并实时应用到设计器、预览和运行中的窗体 |
| 非阻塞的原生文件对话框 | ✅ | 打开／保存／浏览时不会卡住界面事件循环 |

### 10.2 编译器

| 能力 | 状态 | 说明 |
|---|:--:|---|
| 输出单个原生二进制文件 | ✅ | 用 `bincode` + `flate2` 序列化 AST，连同所有窗体经 `include_bytes!` 内嵌，用 `cargo build --release` 构建，并在 `bin/` 输出一个二进制文件——**其中不含任何 `.cbl` 源码** |
| 再分发声明 | ✅ | `bin/` 会自动收到 `LICENSE`、`NOTICE` 和运行时声明，因此分发物带有 Apache-2.0 所要求的声明 |
| 构建失败时给出真实的 `rustc` 诊断 | ✅ | 构建失败时报告的是编译器自己的诊断信息，而不是一行摘要 |

.<<

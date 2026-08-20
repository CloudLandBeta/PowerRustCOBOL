<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL-85 受支持语法参考

**关于 RustCOBOL 的词法分析器／语法分析器／运行时今天实际接受什么的权威依据**，
由源码（`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`）推导而来。请针对 ✅ 的
形式来编写测试；❌ 的形式将无法解析或是空操作，而 ⚠️ 的形式可以解析但行为只是
部分实现。本文是
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md) 的姊妹篇：矩阵说明
*测试什么*，本文说明 *RustCOBOL 能看懂哪种写法*。

图例：✅ 支持 · ⚠️ 可解析但部分实现／已简化 · ❌ 不识别（请避免使用，或仅为确认
该缺口而测试）。

> **更新（缺口实现阶段）：** 下列内容已实现，现为 ✅ —— **引用修饰**
> `id(起始:长度)`、**内联 `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING 的 `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、
> **能识别类别的 `INITIALIZE`**、**以运算符打头的缩写条件**（`a > 1 AND < 9`）、
> **`CALL … ON EXCEPTION`**（在 CALL 无法解析时运行）、**`COMPUTE` 多接收方 +
> 逐接收方的 `ROUNDED`**，以及大为扩充的**内部函数**集合。
>
> **更新（层次化／按出现次数感知的环境阶段 —— 1.5.0）：** 四项曾被数据模型阻塞的
> 特性现为 ✅ —— **运行时表下标** `t(i)` / `t(i, j)`（按出现次数分配存储）、
> **限定名消歧** `id OF/IN 组`（重名的叶子项解析到各自独立的存储）、
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**，以及**可用的 `SEARCH` / `SEARCH ALL`**。
>
> **更新（动词完备性阶段 —— 1.6.0）：** 现在还包括 ✅ —— `ADD`/`SUBTRACT` 上的
> **多接收方 `MULTIPLY`/`DIVIDE GIVING` + 逐接收方 `ROUNDED`**；
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** 以及修正后的
> 单独 `EXIT`；**`CALL … NOT ON EXCEPTION`**；**`INSPECT … TALLYING …
> REPLACING`** 的合并使用与 **`BEFORE/AFTER INITIAL`** 区域；日期与财务
> **内部函数**（`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、`INTEGER-OF-DAY`、
> `DAY-OF-INTEGER`、`ANNUITY`、`FRACTION-PART`）；**字面量宾语的缩写条件**
> （`A = 1 OR 2 OR 3`）；**`EVALUATE … ALSO`**（多主语）与 **`WHEN NOT`**；
> **真正的 88 层条件名**（`SET … TO TRUE/FALSE`，以宿主项对照其 VALUE／范围进行
> 判断）；**`PERFORM para VARYING`**；以及可用的 **`SORT`/`MERGE`** 运行时
> （`RELEASE`/`RETURN`、`USING`/`GIVING`、`INPUT`/`OUTPUT PROCEDURE`）。文末的
> 回避清单是最新的。
>
> **更新（清空回避清单阶段 —— 1.7.0）：** 余下的缺口现已实现 —— **标识符宾语的
> 缩写**（`a = b OR c`，借助 88 层元数据解析）；
> **`INITIALIZE … REPLACING 类别 DATA BY 值`**；**`66 RENAMES`**（读取时合成／
> 写入时在所覆盖的项之间分配）；**指针**（`USAGE POINTER`、
> `SET ptr TO ADDRESS OF x / NULL`、`SET ADDRESS OF item TO …` 的别名机制、
> `IF ptr = NULL`）；**`ALTER`** / **`UNLOCK`**；忠实的 **`NEXT SENTENCE`**；
> 余下的标准**内部函数**（`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、
> `NUMVAL-F`、`TEST-NUMVAL`）；以及扩展的**屏幕 `ACCEPT`/`DISPLAY`**（CLI 模式下
> 通过 ANSI 实现 `AT`/`WITH`——现在是真正*执行*，而不只是解析）。
>
> **更新（1.7.1）：** `ACCEPT` 的寄存器来源现已可用（此前只是被识别的空操作）——
> **`FROM COMMAND-LINE`**、**`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`**（与
> `DISPLAY n UPON ARGUMENT-NUMBER` 配对）、**`ENVIRONMENT-VALUE`**（与
> `DISPLAY "name" UPON ENVIRONMENT-NAME` 配对）、**`ESCAPE KEY`** → `"00"`、
> **`CRT STATUS`** → `"0000"`。
>
> **更新（1.7.2）：** 文件共享／加锁子句与 `CANCEL`（此前为 ❌ ／空操作）——
> **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`**（释放该文件的 INDEXED 记录锁），以及 **`CANCEL 程序`**
> （重新初始化该程序的存储）。
>
> **更新（1.8.0）：** **`COMMIT` / `ROLLBACK`** 现在是真正的 COBOL 动词 —— 针对
> 已打开的 INDEXED 文件、由程序控制的事务（内存引擎与磁盘引擎均可）。磁盘引擎
> 获得了真正的运行期撤销日志（此前是空操作）。文末的回避清单是最新的。

---

## 可识别的语句（动词）

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2`（改变 para-1 的 `GO TO` 去向）·
`UNLOCK file`（释放该文件的记录锁）· `OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK`（文件共享／加锁——在单一运行单元内属建议性）
✅ `COMMIT` / `ROLLBACK`（由程序控制的 INDEXED 文件事务——参见「文件动词」）·
`CANCEL`（重新初始化程序存储）· ⚠️ `INVOKE`（按空操作解析）
项目扩展：`EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。一个块
可以 `use` 那些始终被链接的 crate（std、egui、eframe 以及已链接的运行时集合），
**再加上 project 在 Project's Crates 中登记的任何 crate**（spec 044）：已登记的
crate 会被钉在一个确切版本、随项目一起放入 project 的 `crates/` 并编译进二进制；
未登记的 crate 会在开发者所在的那一行让 Check/Build 失败，并给出补救办法。

✅ `SEARCH`（顺序）/ `SEARCH ALL`（对带 `ASCENDING`/`DESCENDING KEY` 的表做二分
查找——执行第一个匹配的 `WHEN`，否则执行 `AT END`）。
✅ 带 `RELEASE` / `RETURN` 的 `SORT` / `MERGE`（可用——见下文）。
✅ `DECLARATIVES … END DECLARATIVES` 配合 `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` —— 在出现未处理的错误 `FILE STATUS` 时
触发文件错误处理程序。
❌ **不识别——请勿使用：** `ENTRY`、`GENERATE`/`INITIATE`/`TERMINATE`、
`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 各动词支持的形式

### MOVE
- ✅ `MOVE {id|字面量|形象常量} TO id1 [id2 …]`（多个接收方）。
- ✅ `MOVE CORRESPONDING g1 TO g2` —— 逐一搬移两个组中同名的下级项，并递归进入
  相匹配的子组。
- ✅ **引用修饰 `id(起始:长度)`** —— 既可作发送方（取子串）也可作接收方（部分
  赋值）；对所有动词的操作数均有效。`长度` 可省略。
- ✅ 下标 `t(i)`、`t(i, j)` —— 读写该出现次数对应的存储槽；可变下标 `t(WS-I)`
  在每次访问时求值。
- ✅ 限定 `id OF/IN 组`（`… OF g1 OF g2`）—— 即便叶子名在多个组下都有声明，也能
  解析到正确的项。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **逐接收方的 `ROUNDED`** —— 每个接收方都带有自己的 `ROUNDED` 标志。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` —— 对每一对相匹配的数值项做运算，
  并递归进入相匹配的子组。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **多个 `GIVING` 接收方**，各自带有自己的 `ROUNDED`。
- ⚠️ `DIVIDE a BY b`（不带 `GIVING`）会把 `a/b` 存回 `a`（这是 PowerRustCOBOL 的
  便利做法；标准 COBOL 在此要求 `INTO` 或 `GIVING`）。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = 表达式 [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` —— **多个接收方，各自带有自己的 `ROUNDED`**。
- ✅ 表达式运算符 `+ - * /` 与 `**`（乘方，右结合）、括号、
  `FUNCTION 名称(参数)`。

### IF / EVALUATE
- ✅ `IF 条件 [THEN] 语句 [ELSE 语句] [END-IF]`。
- ✅ `EVALUATE {表达式 | TRUE | FALSE} [ALSO 主语 …]` … `WHEN {值 | 值 THRU 值 |
  NOT 值 | 条件 | ANY} [ALSO …] 语句 … [WHEN OTHER 语句] END-EVALUATE`。
- ✅ **`ALSO` 多主语** —— 每个 `WHEN` 列都按位置与其主语比较，再以 AND 合并。
- ✅ **`WHEN NOT 值`** 对选择对象取反；**`WHEN 条件`**（例如
  `EVALUATE TRUE WHEN a > b`）会对布尔条件求值。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES`（n 为整数字面量或数据项）。
- ✅ `PERFORM p UNTIL 条件 [WITH TEST {BEFORE|AFTER}]`。
- ✅ 内联 `PERFORM UNTIL 条件 … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL 条件 … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ 内联 `PERFORM n TIMES … END-PERFORM`（无需段落）。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` —— 每次迭代都执行该段落
  （非内联，无 `END-PERFORM`）。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`。
- ✅ `CONTINUE` · `STOP RUN` · `STOP 字面量`。
- ✅ 单独的 `EXIT` 是一个不做事的返回点；`EXIT PROGRAM` 返回调用者。
- ✅ `EXIT PERFORM [CYCLE]`（中断／继续最近的内联 PERFORM）、`EXIT PARAGRAPH`、
  `EXIT SECTION`。
- ✅ `NEXT SENTENCE` —— 把控制转移到下一个句子边界之后（分析器会在每个句点处插入
  边界标记；这是忠实实现，而不只是 `CONTINUE`）。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | 助记符}`。
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` 定位光标（ANSI，CLI）。
- ✅ `FROM COMMAND-LINE`（整条命令行）· `FROM ARGUMENT-NUMBER`（参数个数）·
  `FROM ARGUMENT-VALUE`（位于 `DISPLAY n UPON ARGUMENT-NUMBER` 所设指针处的
  参数）· `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`（由
  `DISPLAY "name" UPON ENVIRONMENT-NAME` 指定的变量）· `FROM ESCAPE KEY` →
  `"00"` · `FROM CRT STATUS` → `"0000"`。

### DISPLAY
- ✅ `DISPLAY {id|字面量} … [UPON 助记符] [[WITH] NO ADVANCING]`。
- ✅ 屏幕形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` —— 在 **CLI 模式**
  （`rcrun`）下通过 ANSI 光标定位 + SGR 执行；在 GUI 模式下被忽略（那里由
  form designer 取代 SCREEN I/O）。`ACCEPT id AT …` 先定位再读取。

### STRING
- ✅ `STRING {源 [DELIMITED BY {SIZE | SPACE[S] | 分隔符}]} … INTO 目标
  [WITH POINTER p] [[ON] OVERFLOW 语句] [NOT [ON] OVERFLOW 语句] [END-STRING]`。
  溢出＝拼装出的字符串比接收字段更宽。
- ✅ **扩展 —— 智能的默认 `DELIMITED BY`**（当某个操作数省略该子句时）：
  字母数字的 `PIC X`/`A` 项默认取 `SPACES`（丢弃尾部填充）；字符串字面量、数值项、
  数值编辑项、`FUNCTION` 的结果以及表达式默认取 `SIZE`。数据项按其字段形态搬移
  （数值 → 按 PIC 全宽的数字；数值编辑 → 编辑后的字符）。

### UNSTRING
- ✅ `UNSTRING 源 [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW 语句]
  [NOT [ON] OVERFLOW 语句] [END-UNSTRING]`。溢出＝源字段数多于接收方。

### INSPECT
- ✅ `INSPECT id CONVERTING 源字符 TO 目标字符`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` —— **两半都会执行**。
- ✅ `BEFORE/AFTER INITIAL` 把每个子句限制在字段的某个子区域内。
  （按 COBOL 规定，TALLYING 是在计数器上累加。）

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | 表达式}`（编译为 MOVE）。
- ✅ `SET idx {UP|DOWN} BY n`（编码为 ADD / SUBTRACT）。
- ✅ `SET 88-名称 TO TRUE` 把宿主项设为该条件的第一个 VALUE；`TO FALSE` 则设为
  VALUE 集合之外的某个值（尽力而为——并没有 FALSE 子句）。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | 另一个 ptr}` 以及
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` —— 参见下文**指针**。

### INITIALIZE
- ✅ `INITIALIZE id …` —— 能识别类别：数值／数值编辑 → ZERO，其余一律 → SPACES，
  并递归进入组项。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY 值 …` —— 把该类别的每个下级项
  设为该值；其余项不受影响。

### 指针（USAGE POINTER）
- ✅ `USAGE POINTER` 声明一个指针（初始为 NULL）。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` —— 让 `id` 成为目标存储的
  别名（读取**和**写入都会跟随该别名）；通常用于 LINKAGE 记录。`IF ptr = NULL`
  可用。

### CALL / CANCEL
- ✅ `CALL {字面量|id} [USING [BY {REFERENCE|CONTENT|VALUE}] 参数 …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} 语句] [NOT [ON] {EXCEPTION|OVERFLOW} 语句] [END-CALL]`。
- ✅ 当被调用的程序无法解析时，执行 `ON EXCEPTION` / `ON OVERFLOW` 的主体；当调用
  **成功解析**时，执行 `NOT ON EXCEPTION` 的主体。
- ✅ `CANCEL 程序 …` 会重新初始化所指程序的 WORKING-STORAGE，使其下一次 `CALL`
  从头开始。

### 文件动词（受支持的子句——完整覆盖见文件 I/O 测试套件）
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`；`CLOSE f …`。
  （`SHARING` / `WITH LOCK` 可解析，并在有意义之处被遵守——在单一运行单元模型中
  属建议性。）
- ✅ **`OPEN … WITH REGISTERED [USER] {字面量 | 数据项}`**（PowerRustCOBOL
  扩展）—— 把操作员／用户记入 INDEXED 可观测性日志（该文件本次会话的每一条事件行
  都会带上 `user=` 字段）。纯粹用于观测；不做认证／授权。参见
  [`observability.md`](observability.md) §1.3.1。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` 会释放 INDEXED 引擎在 I-O 下取得的记录锁。
- ✅ `UNLOCK f [RECORD[S]]` 释放该文件的记录锁。
- ✅ **`COMMIT` / `ROLLBACK`** —— 针对**所有**已打开的 INDEXED 文件、由程序控制的
  事务。`OPEN` 开启一个事务；`COMMIT` 确认挂起的
  `WRITE`/`REWRITE`/`DELETE`（此后的 `ROLLBACK` 便无法再撤销它们）并开启新的事务；
  `ROLLBACK` 撤销自上一次 `COMMIT`/`OPEN` 以来的一切更改。**DISK** 存储让
  `COMMIT`/`CLOSE` 在磁盘上持久化。**MEMORY** 存储则让 `COMMIT`/`ROLLBACK` 完全
  发生在 RAM 中（从不写盘）；一个普通的 `STORAGE IS MEMORY` 文件是临时的，而
  `STORAGE IS MEMORY WITH PERSISTENCE` 仅在 `CLOSE` 时保存到磁盘。（借助持久化
  预写日志实现的崩溃恢复尚待完成——这里说的是运行期内、程序层面的回滚。）
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`**（INDEXED 文件；PowerRustCOBOL 扩展）。默认存储为 `DISK`。
  `WITH COMPRESSION` 会压缩所存储的记录（键是在未压缩的记录上求值的）；
  `WITH PERSISTENCE`（仅限 MEMORY）在 `CLOSE` 时把内存中的文件保存下来。
  `OPEN OUTPUT` 总是（重新）创建磁盘上的容器。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`；
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ 跨*进程*的文件共享不会被强制执行（单一运行单元）；`SHARING`/`LOCK` 子句可以
  解析，且 INDEXED 引擎在本次运行内的记录锁会被遵守。

### SORT / MERGE / RELEASE / RETURN  ✅（可用，工作缓冲区在内存中）
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]`（在 INPUT PROCEDURE 中）向本次运行追加记录；
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` 把记录交还回来。
- 记录按所声明的键做稳定排序（`ASCENDING`/`DESCENDING`）；`USING` 负责读取、
  `GIVING` 负责写出所指定的顺序文件。

---

## 条件（IF / EVALUATE / PERFORM UNTIL）

- ✅ 关系符号：`=` `<>` `<` `>` `<=` `>=`。
- ✅ 词形关系：`[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN] [OR EQUAL TO]`、
  `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ 类别：`id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
- ✅ 符号：`id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 层条件名（直接以该名称作为条件）。
- ✅ `AND` / `OR` / `NOT` 组合以及括号（AND 的结合力强于 OR）。
- ✅ **以运算符打头的缩写条件** —— `a > 1 AND < 9`、`a = 5 OR = 7`（沿用前一个
  比较的主语）。
- ✅ **字面量宾语的缩写** —— `a = 1 OR 2 OR 3`（同时沿用主语与运算符；宾语是一个
  字面量）。
- ✅ **标识符宾语的缩写** —— `a = b OR c`（其中 `c` 是一个数据项）。跟在比较之后、
  AND/OR 后面的裸标识符会在运行时解析：若它是已知的 88 层条件名，就按条件求值；
  否则它就是宾语 `a = c`。（紧跟 `AND` 的标识符仍保持 AND 的优先级。）

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
  （日期换算使用标准基准 1601-01-01 = 第 1 天。）**COBOL-85 标准的内部函数集合**
  已完整实现。
  ⚠️ 任何无法识别的 `FUNCTION` 名称仍可解析，但在运行时返回 **0**。
- ✅ 字面量：整数、小数、字符串，以及全部形象常量
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **十六进制字面量** —— `X"09"`、`x'0D0A'`（大小写不限，两种引号皆可）。每
  **一对**十六进制数字对应一个字符，因此位数必须是偶数；位数为奇数或出现非十六
  进制字符即属畸形字面量，会被报告出来，而不会被悄悄重新读成紧挨着字符串的单词
  `X`。凡是可以使用带引号字面量的地方都可以使用它（`DELIMITED BY`、`MOVE`、
  `VALUE`、比较）。

---

## DATA DIVISION 子句（可接受的声明语法）

- ✅ 层号 `01`–`49`、`77`、`88`；`FILLER`；组项／基本项。
- ✅ `PIC/PICTURE`，含 `X A 9 S V P` 与编辑符号（`Z * $ + - CR DB B 0 / , .`）。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}`（以及 `COMP-4`→COMP、`COMP-X`→COMP-5）。
- ✅ `VALUE`（数值／带符号／字母数字／形象常量／`ALL`）。
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES`、`JUSTIFIED [RIGHT]`、`SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL`。
- ✅ `88 名称 VALUE v [v …]` / `VALUE a THRU b` —— **真正的条件名**：该 88 层绑定
  到它的宿主项；判断时会拿宿主项与这些 VALUE ／范围比对，而
  `SET 88-名称 TO TRUE` 会把一个能满足该条件的值存入宿主项。
- ✅ `USAGE INDEX` 声明一个整型索引寄存器（`SET`/`SEARCH` 会用到它）；
  `USAGE POINTER` —— 参见上文**指针**。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` —— 一个重新分组的别名；读取
  时把所覆盖的各项拼接起来，写入时按字段宽度分配下去。
- 节：`WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`；`SCREEN` 可解析但
  不执行。

---

## 仍不支持 —— 当前回避清单

COBOL-85 的动词／子句集合已**完全覆盖**。余下不在范围内的内容，要么是刻意为之，
要么属于 85 之后的标准：

1. **屏幕 `ACCEPT` 的输入编辑** —— `DISPLAY … AT/WITH` 与 `ACCEPT … AT` 在 CLI
   模式下（借助 ANSI）会被执行，但 SCREEN SECTION 完整的字段级编辑（自动跳格、
   字段校验、颜色映射）在 GUI 模式下**已由 form designer 取代**。
2. **跨*进程*的文件共享** —— `OPEN … SHARING/WITH LOCK`、
   `READ … WITH [NO] LOCK` 与 `UNLOCK` 可以解析，并会驱动 INDEXED 引擎在本次运行
   内的记录锁，但这些锁不会在不同的操作系统进程之间被强制执行（单一运行单元
   模型）。
3. **面向对象的 COBOL**（类／方法定义）—— 对 COBOL 对象而言 `INVOKE` 是空操作
   （它只驱动 GUI／运行时对象）。
4. **RELATIVE** 文件组织（SEQUENTIAL / LINE SEQUENTIAL / INDEXED 已完成）。
5. 无法识别的内部函数名仍然返回 **0**。

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

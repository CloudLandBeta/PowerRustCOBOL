<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 动词与数据节测试矩阵

一份用于在项目范围内完成 COBOL‑85 的测试规格。它**深入地**列举现有测试套件*尚未
覆盖*的内容，形式是语法骨架 + 排列轴 + 每个动词必须被驱动的数据类型组合。这些测试
的目标是**探索性的**：跑遍每一种变体，观察当前行为，然后决定要修什么 / 调什么 /
建什么 / 删什么。

> 已验证 — 不要在此重新规格化：精确的数值运算
> （ADD/SUB/MUL/DIV/COMPUTE 的结果值、ROUNDED、ON SIZE ERROR）、numeric‑edited 的
> PICTURE + `DECIMAL-POINT IS COMMA`、COPY/REPLACE、全部文件 I/O
> （SEQUENTIAL/LINE SEQUENTIAL/INDEXED、键、START/REWRITE/DELETE/INVALID KEY、
> STORAGE MODE MEMORY/DISK、压缩、MEMORY 持久化）、嵌套程序/基本 CALL、字母数字
> 比较、固定/自由格式 lexer。（下面的算术*语法*排列仍在范围内 — 只有数值运算本身
> 算“做完了”。）

## 记法

- `[ x ]` 可选，`{ a | b }` 择一，`…` 重复，`dn` = 第 n 个数据项。
- **类型混合轴（T）：** 每一个操作数位置都必须在下列接收方/发送方种类上被驱动，
  可行时双向进行：
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`。
- **每种类别的边界值：** 空、最小、最大、溢出一位、全空格、全零、
  LEADING/TRAILING [SEPARATE] 的符号、P 缩放、V 隐含小数点。
- 每个动词都要采集：结果值、**FILE STATUS / 特殊寄存器**
  （`RETURN-CODE`、`TALLY`）、走到的溢出/异常分支，以及出错时保持不变这一点。

---

## 第 A 部分 — DATA DIVISION 的各节（未测试的行为）

### WORKING-STORAGE SECTION
- **层级：** 01、02–49 的嵌套、77（独立项）、66 `RENAMES a THRU b`、88。
- **PIC：** 带 `(n)` 的 `X A 9 S V P`；`P` 缩放（左/右）；`V` 隐含小数点；
  编辑型组合；带 `PIC` 的组项与不带 PIC 的组项。
- **USAGE：** DISPLAY、COMP/COMP‑4/BINARY、COMP‑1、COMP‑2、COMP‑3/PACKED‑DECIMAL、
  COMP‑5、INDEX、POINTER — 声明 + 存储大小 + 值的往返。
- **VALUE：** 数值、带符号、字母数字、图形常量、`ALL "x"`；组项上的 VALUE；
  非法的 VALUE（长度 > PIC）。
- **OCCURS：** 定长；`DEPENDING ON`；`INDEXED BY`；`ASCENDING/DESCENDING KEY`；
  多维（2–3）；组项上的 OCCURS。
- **子句：** REDEFINES（等长/更小/更大、链式）、RENAMES、JUSTIFIED RIGHT、
  BLANK WHEN ZERO、`SIGN IS {LEADING|TRAILING} [SEPARATE]`、SYNCHRONIZED、FILLER。
- **88 条件名：** 单个值、值列表、`VALUE a THRU b`、多个区间、宿主为数值 /
  字母数字 / 编辑型；求值 + `SET … TO TRUE`。
- **初始化：** 默认值（按类别取空格/零）与 VALUE 的对比；**跨 PERFORM 与跨 CALL 的
  持久性**（WS 保留最后的值）。

### LOCAL-STORAGE SECTION
- **每次进入程序都重新初始化**（与 WS 的持久性形成对比）。
- VALUE 子句**每次进入都重新施加**。
- **递归：** 每次（递归的）CALL 都获得一个独立的 LOCAL-STORAGE 实例。
- 子句覆盖面与 WS 相同（OCCURS/REDEFINES/88/…），但要验证重新初始化的语义。

### LINKAGE SECTION
- 在调用方绑定之前，这些项**没有存储**；访问未绑定的 linkage。
- 通过 `CALL … USING` ↔ `PROCEDURE DIVISION USING` 绑定。
- **BY REFERENCE**（调用方能看到修改）与 **BY CONTENT**（被调方改的是副本）
  与 **BY VALUE**（标量）的对比。
- linkage 中的组项 + 基本项、OCCURS、REDEFINES、88。
- 实参与形参之间的长度/USAGE 不匹配（行为待观察）。
- `ADDRESS OF` / `SET ADDRESS OF … TO` 以及 POINTER 绑定（如果支持）。

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — 按位置绑定到 CALL 的实参；
  个数不匹配（实参更少/更多）；顺序。
- USING 列表上逐个参数的 `BY REFERENCE | BY VALUE`。
- `RETURNING dn` — 交回给 `CALL … RETURNING` 的值；与 `GIVING` 的对比；与
  `RETURN-CODE` 的对比。
- 主程序的 `USING` 由命令行绑定（如果支持）。
- 每个参数位置上的类型混合（施加 **T**）。

---

## 第 B 部分 — 动词排列矩阵

对每个操作数位置，让每个动词跑遍 **T**。下面列出的是叠加在类型混合之上的*结构性*
排列（子句/短语）。

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]`（多个接收方）。
- `MOVE CORRESPONDING g1 TO g2`（按名字匹配基本项）。
- 引用修饰的源/目标：`MOVE a(s:l) TO b(s:l)`。
- 带下标：`MOVE t(i) TO u(j)`、`t(i,j)`。
- 类型转换（双向施加 **T**）：num→edited、edited→num、alnum→num、
  num→alnum（对齐/填充/截断）、group→group（按字节拷贝）、符号处理、
  COMP‑3↔DISPLAY、float↔fixed、figurative→每一种类别。

### DISPLAY
- `DISPLAY {dn|literal} …`（拼接的操作数）。
- `[WITH NO ADVANCING]`；`UPON {CONSOLE|SYSOUT|mnemonic}`。
- 屏幕形式（观察后决定）：`DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`。
- 类型混合：数值（PIC 全宽）、编辑型、带符号、组项、图形常量。

### ACCEPT  *（规格化全部形式；很多属于屏幕/终端 — 标记出来以便决定范围）*
- `ACCEPT dn`（从控制台读入 alnum / numeric / edited / group）。
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`。
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`。
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`。
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`。
- 屏幕形式：`ACCEPT dn AT {nnnn|LINE n COL n}`、
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`、
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`、
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`。
- 接收到数值项、numeric-edited 项还是 alnum 项（去编辑 / 校验）。

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`。
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`。
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`。
- `SUBTRACT … FROM …`、`SUBTRACT … GIVING …`、`SUBTRACT CORRESPONDING …`。
- 多个接收方各自有自己的 ROUNDED/长度行为；USAGE 混合的操作数
  （COMP‑3 + DISPLAY + 编辑型）；带符号；引用修饰的操作数。

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`。
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`。
- 除以零 → ON SIZE ERROR；REMAINDER 的符号/标度；USAGE 混合。

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`。
- 运算符 `+ - * / **`、括号、优先级；表达式中的内部函数；USAGE 混合的操作数；
  多个接收方；截断与 ROUNDED 的对比。

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — 嵌套、空分支、`NEXT SENTENCE`。
- 条件：关系条件（`= < > <= >= NOT`）、类条件（`IS [NOT] {NUMERIC|ALPHABETIC|
  ALPHABETIC-UPPER|ALPHABETIC-LOWER}`）、符号条件（`POSITIVE|NEGATIVE|ZERO`）、
  引用 88 条件、复合条件（`AND/OR/NOT`）、**缩写形式**（`a = b OR c`）、
  带括号的条件。
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` 配合
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`。
- 比较中的类型混合（数值、alnum、编辑型、图形常量之间的对比）。

### PERFORM
- 行外的 `PERFORM p1 [THRU p2]`。
- `PERFORM p [THRU p2] n TIMES`（n = 字面量 / 数据项）。
- 带 `[WITH TEST {BEFORE|AFTER}]` 的 `PERFORM … UNTIL cond`。
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`。
- 行内的 `PERFORM … END-PERFORM`（带 TIMES/UNTIL/VARYING）。
- 嵌套/递归的 PERFORM；范围重叠；索引型与数值型循环变量的对比。

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`；`GO TO p1 p2 … DEPENDING ON dn`（在范围内/超出范围）。
- `CONTINUE`；`NEXT SENTENCE`。
- `EXIT`、`EXIT PERFORM [CYCLE]`、`EXIT PROGRAM`、`EXIT PARAGRAPH/SECTION`。
- `STOP RUN`、`STOP literal`、`GOBACK`（从主程序与从子程序的对比）。

### SET
- `SET index TO {n|index}`；`SET index {UP|DOWN} BY n`。
- `SET 88-name TO TRUE`。
- `SET pointer TO {ADDRESS OF dn|NULL}`；`SET ADDRESS OF linkage TO pointer`。
- `SET d1 TO {TRUE|FALSE}`（在支持的场合）。

### INITIALIZE
- `INITIALIZE dn …`（组项/基本项；按类别取默认值）。
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`。
- `[WITH FILLER]`、`[THEN TO DEFAULT]`；表（全部出现项）。

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]`（顺序查找）。
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH`（二分查找；
  要求 `ASCENDING/DESCENDING KEY` + `INDEXED BY`）。
- 找到/未找到；多个 WHEN；键的类型混合；表未排序时的行为。

### STRING  *（驱动用户的排列风格）*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`。
- 需要覆盖的排列：
  - 单个源 `DELIMITED BY SIZE` → alnum 目标。
  - 多个源、**混合分隔符**：`STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`。
  - 众多源/分隔符：`STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`。
  - `WITH POINTER` 的起始/推进；指针超出范围 → 溢出。
  - 目标太小 → `ON OVERFLOW`；`NOT ON OVERFLOW`。
  - **类型混合的源：** 数值、numeric-edited、带符号、组项、图形常量、
    引用修饰 — 观察每一种是怎样被串接成字符串的。

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`。
- 排列：单个分隔符与多个分隔符、`ALL`（合并重复）、`OR`、
  `DELIMITER IN`/`COUNT IN` 的捕获、POINTER、TALLYING、字段比数据多
  （溢出）、类型混合的目标（数值接收方会被去编辑）。

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`。
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`。
- `INSPECT dn TALLYING … REPLACING …`（组合形式）。
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`。
- BEFORE/AFTER 的作用域；重叠的匹配；多字符模式；类型混合的宿主。

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`。
- 静态（字面量）与动态（数据名）的程序名；无法解析 → ON EXCEPTION。
- 实参传递方式（观察调用方是否可见）；实参个数/类型不匹配。
- `RETURNING` 与 `RETURN-CODE` 的对比；递归；`EXTERNAL` 共享数据。
  （✅ `CANCEL prog` 已实现 — 重新初始化该程序的存储；
  `NOT ON EXCEPTION` 在解析成功的 CALL 上执行。）

### ARITHMETIC 特殊寄存器与其他动词
- `ADD/SUBTRACT … GIVING` 的零抑制与 `TO` 累加的对比。
- 对 `RETURN-CODE`、`TALLY` 的 `MOVE`/算术运算。
- ✅ `ALTER`（遗留的 GO TO）— 已实现（改写该段落的 `GO TO` 目标）。
- `ACCEPT/DISPLAY` 经由编辑型字段的往返。

### 文件动词 — *（只列文件 I/O 套件里没有的缺口）*
- ✅ **已实现并测试**（`test_file_locking`）：`OPEN … SHARING WITH …
  [WITH LOCK]`、`READ … WITH [NO] LOCK`、`UNLOCK`（在单个运行单元内属于建议性 —
  见受支持语法参考）。
- `READ … INTO`、`WRITE … FROM`、`REWRITE … FROM`、`START … KEY IS {= > >= < <=}`
  配合引用修饰的键；多个 FD 共享同一个记录区。

### 计划中的动词（实现时的规格）
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`；`RELEASE`、`RETURN`。
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`。
- `RELATIVE` 组织：按 `RELATIVE KEY` 的 `READ/WRITE/REWRITE/DELETE/START`。

---

## 第 C 部分 — 跨形式等价性测试框架

针对上面挑选出的一组程序，断言同一份源代码在三种执行形式下产生**完全相同**的可观察
输出（DISPLAY 的文本、FILE STATUS、RETURN-CODE、文件内容）：

1. **解释器**（`Interpreter::run`）。
2. **AST 往返** — 序列化（`bincode`+`flate2`）→ 反序列化 → 运行；断言 AST 逐字节
   相同且输出相同。
3. **打包/编译后的二进制** — `cobolt_compiler::build_project` → 执行生成的二进制；
   断言输出相同。

形式之间的任何分歧都是需要记录的缺陷（“一个编译器，一种行为”这一不变式）。

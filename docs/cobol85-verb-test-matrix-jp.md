<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# RustCOBOL‑85 動詞・データ節テストマトリクス

プロジェクトの範囲内で COBOL‑85 を完成させるためのテスト仕様書です。既存のテスト
スイートで*まだ覆えていない*ものを、構文スケルトン＋順列の軸＋各動詞を駆動すべき
データ型の組み合わせという形で、**深く**列挙します。これらのテストの目的は**探索
的**であること — あらゆる変種を実行し、現在の挙動を観察し、何を直し、調整し、作り、
取り除くかを決めることです。

> すでに検証済み — ここで再仕様化しないこと: 厳密な数値演算
> （ADD/SUB/MUL/DIV/COMPUTE の結果値、ROUNDED、ON SIZE ERROR）、数字編集 PICTURE と
> `DECIMAL-POINT IS COMMA`、COPY/REPLACE、ファイル入出力の全体
> （SEQUENTIAL/LINE SEQUENTIAL/INDEXED、キー、
> START/REWRITE/DELETE/INVALID KEY、STORAGE MODE MEMORY/DISK、圧縮、MEMORY の
> 永続化）、入れ子プログラムと基本的な CALL、英数字比較、固定形式・自由形式の
> 字句解析。（下にある算術の*構文*の順列はなお対象です。「済み」なのは値の計算だけ
> です。）

## 記法

- `[ x ]` は省略可、`{ a | b }` は選択、`…` は繰り返し、`dn` は n 番目のデータ項目。
- **型の組み合わせ軸 (T):** すべてのオペランド位置は、次の受け手・送り手の種別に
  わたって、適用できる場合は双方向で試験すること。
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`。
- **種別ごとの境界値:** 空、最小、最大、1 つ超過、全部空白、全部ゼロ、
  LEADING/TRAILING [SEPARATE] の符号、P による位取り、V による暗黙の小数点。
- 各動詞について次を記録すること: 結果の値、**FILE STATUS と特殊レジスター**
  （`RETURN-CODE`、`TALLY`）、どのオーバーフロー／例外分岐を通ったか、そしてエラー時
  に何も変わらないこと。

---

## パート A — DATA DIVISION の各節（未試験の挙動）

### WORKING-STORAGE SECTION
- **レベル:** 01、02–49 の入れ子、77（独立）、66 `RENAMES a THRU b`、88。
- **PIC:** `(n)` を伴う `X A 9 S V P`、`P` による位取り（左右）、`V` による暗黙の
  小数点、編集の組み合わせ、`PIC` のある集団項目と無い集団項目。
- **USAGE:** DISPLAY、COMP/COMP‑4/BINARY、COMP‑1、COMP‑2、
  COMP‑3/PACKED‑DECIMAL、COMP‑5、INDEX、POINTER — 宣言、記憶域の大きさ、値の往復。
- **VALUE:** 数値、符号付き、英数字、定数、`ALL "x"`、集団項目への VALUE、不正な
  VALUE（PIC より大きい）。
- **OCCURS:** 固定、`DEPENDING ON`、`INDEXED BY`、`ASCENDING/DESCENDING KEY`、
  多次元（2〜3）、集団項目への OCCURS。
- **各句:** REDEFINES（同じ／小さい／大きい、連鎖）、RENAMES、JUSTIFIED RIGHT、
  BLANK WHEN ZERO、`SIGN IS {LEADING|TRAILING} [SEPARATE]`、SYNCHRONIZED、
  FILLER。
- **88 条件名:** 単一値、値の並び、`VALUE a THRU b`、複数の範囲、数値／英数字／編集
  済みの母体上での評価と `SET … TO TRUE`。
- **初期化:** 既定（クラスに応じた空白／ゼロ）と VALUE の対比、**PERFORM をまたぐ
  持続と CALL をまたぐ持続**（WS は最後の値を保持する）。

### LOCAL-STORAGE SECTION
- **プログラムに入るたびに再初期化される**（WS の持続性との対比）。
- VALUE 句は**入るたびに再適用される**。
- **再帰:** （再帰的な）CALL ごとに独立した LOCAL-STORAGE の実体が得られる。
- 句のカバー範囲は WS と同じ（OCCURS/REDEFINES/88/…）だが、再初期化の意味論を検証
  すること。

### LINKAGE SECTION
- 項目は**呼び出し側が結び付けるまで記憶域を持たない**。結び付いていない連絡項目へ
  のアクセス。
- `CALL … USING` ↔ `PROCEDURE DIVISION USING` で結び付ける。
- **BY REFERENCE**（呼び出し側が変更を見る）と **BY CONTENT**（呼ばれた側が複製を
  編集する）と **BY VALUE**（スカラー）の対比。
- 連絡節における集団項目と基本項目、OCCURS、REDEFINES、88。
- 実引数と仮引数の間で大きさや USAGE が食い違う場合（挙動を観察する）。
- `ADDRESS OF` / `SET ADDRESS OF … TO` と POINTER の結び付け（対応していれば）。

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — CALL の引数への位置による結び付け、個数の
  不一致（少ない／多い）、順序。
- USING の並びにおけるパラメーターごとの `BY REFERENCE | BY VALUE`。
- `RETURNING dn` — `CALL … RETURNING` に返される値。`GIVING` との対比、
  `RETURN-CODE` との対比。
- 主プログラムの `USING` をコマンドラインから結び付ける（対応していれば）。
- すべてのパラメーター位置での型の組み合わせ（**T** を適用）。

---

## パート B — 動詞の順列マトリクス

各動詞を、すべてのオペランド位置について **T** にわたって駆動してください。以下は
型の組み合わせに上乗せされる*構造的*な順列（句・語句）の一覧です。

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]`（複数の受け手）。
- `MOVE CORRESPONDING g1 TO g2`（名前で一致する基本項目）。
- 部分参照した送り手／受け手: `MOVE a(s:l) TO b(s:l)`。
- 添字付き: `MOVE t(i) TO u(j)`、`t(i,j)`。
- 型変換（**T** を双方向に適用）: 数値→編集、編集→数値、英数字→数値、
  数値→英数字（右詰め／埋め／切り捨て）、集団→集団（バイトコピー）、符号の扱い、
  COMP‑3↔DISPLAY、浮動小数↔固定小数、定数→各種別。

### DISPLAY
- `DISPLAY {dn|literal} …`（連結されたオペランド）。
- `[WITH NO ADVANCING]`、`UPON {CONSOLE|SYSOUT|mnemonic}`。
- 画面形式（観察して判断する）: `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`。
- 型の組み合わせ: 数値（PIC の全幅）、編集済み、符号付き、集団、定数。

### ACCEPT  *（すべての形式を仕様化する。多くは画面／端末向け — 範囲の判断のため印を付けること）*
- `ACCEPT dn`（コンソールから英数字／数値／編集済み／集団へ）。
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`。
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`。
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`。
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`。
- 画面形式: `ACCEPT dn AT {nnnn|LINE n COL n}`、
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`、
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`、
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`。
- 数値、数字編集、英数字それぞれへの受け取り（編集解除と検証）。

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`。
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`。
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`。
- `SUBTRACT … FROM …`、`SUBTRACT … GIVING …`、`SUBTRACT CORRESPONDING …`。
- 複数の受け手それぞれが独自の ROUNDED と桁あふれの挙動を持つこと、USAGE の混ざった
  オペランド（COMP‑3 + DISPLAY + 編集済み）、符号付き、部分参照したオペランド。

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`。
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`。
- ゼロ除算 → ON SIZE ERROR、REMAINDER の符号と位取り、USAGE の混在。

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`。
- 演算子 `+ - * / **`、括弧、優先順位、式の中の組込み関数、USAGE の混ざった
  オペランド、複数の受け手、切り捨てと ROUNDED の対比。

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — 入れ子、空の分岐、`NEXT SENTENCE`。
- 条件: 関係条件（`= < > <= >= NOT`）、クラス条件
  （`IS [NOT] {NUMERIC|ALPHABETIC|ALPHABETIC-UPPER|ALPHABETIC-LOWER}`）、符号条件
  （`POSITIVE|NEGATIVE|ZERO`）、88 条件の参照、複合条件（`AND/OR/NOT`）、
  **略記**（`a = b OR c`）、括弧付き。
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` と
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`。
- 比較における型の組み合わせ（数値／英数字／編集済み／定数）。

### PERFORM
- 行外: `PERFORM p1 [THRU p2]`。
- `PERFORM p [THRU p2] n TIMES`（n は定数またはデータ項目）。
- `[WITH TEST {BEFORE|AFTER}]` を伴う `PERFORM … UNTIL cond`。
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`。
- 行内: `PERFORM … END-PERFORM`（TIMES/UNTIL/VARYING 付き）。
- 入れ子・再帰的な PERFORM、範囲の重なり、指標と数値の繰り返し変数の対比。

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`、`GO TO p1 p2 … DEPENDING ON dn`（範囲内と範囲外）。
- `CONTINUE`、`NEXT SENTENCE`。
- `EXIT`、`EXIT PERFORM [CYCLE]`、`EXIT PROGRAM`、`EXIT PARAGRAPH/SECTION`。
- `STOP RUN`、`STOP literal`、`GOBACK`（主プログラムからと副プログラムから）。

### SET
- `SET index TO {n|index}`、`SET index {UP|DOWN} BY n`。
- `SET 88-name TO TRUE`。
- `SET pointer TO {ADDRESS OF dn|NULL}`、`SET ADDRESS OF linkage TO pointer`。
- `SET d1 TO {TRUE|FALSE}`（対応している場合）。

### INITIALIZE
- `INITIALIZE dn …`（集団／基本項目。カテゴリーに応じた既定値）。
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`。
- `[WITH FILLER]`、`[THEN TO DEFAULT]`、表（すべての出現）。

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]`（逐次）。
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH`
  （二分探索。`ASCENDING/DESCENDING KEY` と `INDEXED BY` が必要）。
- 見つかった場合と見つからない場合、複数の WHEN、キーの型の組み合わせ、未整列の表で
  の挙動。

### STRING  *（利用者の順列スタイルを試す）*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`。
- 覆うべき順列:
  - 単一の送り手を `DELIMITED BY SIZE` で → 英数字の受け手へ。
  - 複数の送り手で**区切りを混ぜる**: `STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`。
  - 多数の送り手と区切り: `STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`。
  - `WITH POINTER` の開始と前進、範囲外のポインター → オーバーフロー。
  - 受け手が小さすぎる → `ON OVERFLOW`、`NOT ON OVERFLOW`。
  - **送り手の型の組み合わせ:** 数値、数字編集、符号付き、集団、定数、部分参照 —
    それぞれがどう文字列化されるかを観察する。

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`。
- 順列: 単一と複数の区切り、`ALL`（繰り返しをまとめる）、`OR`、
  `DELIMITER IN`/`COUNT IN` による捕捉、POINTER、TALLYING、データより項目が多い場合
  （オーバーフロー）、型の混ざった受け手（数値の受け手は編集解除される）。

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`。
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`。
- `INSPECT dn TALLYING … REPLACING …`（併用）。
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`。
- BEFORE/AFTER の適用範囲、重なる一致、複数文字のパターン、型の混ざった母体。

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`。
- 静的（定数）なプログラム名と動的（データ名）なプログラム名、解決できない場合 →
  ON EXCEPTION。
- 引数の渡し方（呼び出し側から見えるかを観察する）、引数の個数や型の不一致。
- `RETURNING` と `RETURN-CODE` の対比、再帰、`EXTERNAL` による共有データ。
  （✅ `CANCEL prog` は実装済み — そのプログラムの記憶域を再初期化する。
  `NOT ON EXCEPTION` は解決できた CALL で実行される。）

### 算術の特殊レジスターとその他の動詞
- `ADD/SUBTRACT … GIVING`（ゼロからの置き換え）と `TO` による累算の対比。
- `RETURN-CODE`、`TALLY` への／からの `MOVE` と算術。
- ✅ `ALTER`（旧来の GO TO）— 実装済み（その段落の `GO TO` を差し替える）。
- 編集済み項目を通した `ACCEPT/DISPLAY` の往復。

### ファイル動詞 — *（ファイル入出力スイートに無い穴だけ）*
- ✅ **実装済みかつ試験済み**（`test_file_locking`）: `OPEN … SHARING WITH …
  [WITH LOCK]`、`READ … WITH [NO] LOCK`、`UNLOCK`（単一の実行単位内では勧告的 —
  対応構文のリファレンスを参照）。
- `READ … INTO`、`WRITE … FROM`、`REWRITE … FROM`、
  `START … KEY IS {= > >= < <=}`（部分参照したキーを伴う）、複数の FD が 1 つの
  レコード領域を共有する場合。

### 実装より前にここで仕様化された動詞

> **これらはすべて実装済みです。** SORT/MERGE/RELEASE/RETURN は 1.62.119 で、
> RELATIVE エンジンは 1.62.76（`crates/cobolt-runtime/src/relative.rs`）で入りまし
> た。`docs/cobol85-supported-syntax-en.md` はこれらを ✅ と記しています。以下の
> 順列の軸は、もともとそうであったとおりテスト計画として残してあります。まだ*覆う*
> べきものを述べているのであって、まだ*作る*べきものではありません。
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`、`RELEASE`、`RETURN`。
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`。
- `RELATIVE` 編成: `RELATIVE KEY` による `READ/WRITE/REWRITE/DELETE/START`。

---

## パート C — 形式間の等価性ハーネス

上記のプログラムから選んだ一群について、同じソースを 3 つの実行形式で動かし、観測
できる出力（DISPLAY のテキスト、FILE STATUS、RETURN-CODE、ファイルの内容）が
**同一**であることを確かめます。

1. **インタープリター**（`Interpreter::run`）。
2. **AST の往復** — 直列化（`bincode`+`flate2`）→ 復元 → 実行。AST がバイト単位で
   同一であること、出力が同一であることを確かめる。
3. **同梱／コンパイル済みバイナリ** — `cobolt_compiler::build_project` で作った
   バイナリを実行し、出力が同一であることを確かめる。

形式間の食い違いはすべて記録すべき欠陥です（「1 つのコンパイラー、1 つの挙動」
という不変条件）。

.<<

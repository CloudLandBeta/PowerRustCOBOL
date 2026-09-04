<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 動詞・データ節テストマトリクス

プロジェクトの範囲内で COBOL‑85 を完成させるためのテスト仕様です。既存のテスト
スイートで*まだカバーされていない*ものを、構文スケルトン + 順列の軸 + 各動詞を
駆動すべきデータ型の組み合わせという形で、**掘り下げて**列挙します。これらの
テストの目的は**探索的**であること — すべてのバリエーションを実行し、現在の
挙動を観察し、何を修正 / 調整 / 新規作成 / 削除するかを決めることです。

> 検証済み — ここで再仕様化しないこと: 正確な数値演算
> (ADD/SUB/MUL/DIV/COMPUTE の結果値、ROUNDED、ON SIZE ERROR)、numeric‑edited の
> PICTURE + `DECIMAL-POINT IS COMMA`、COPY/REPLACE、ファイル入出力のすべて
> (SEQUENTIAL/LINE SEQUENTIAL/INDEXED、キー、START/REWRITE/DELETE/INVALID KEY、
> STORAGE MODE MEMORY/DISK、圧縮、MEMORY の永続化)、入れ子プログラム/基本の
> CALL、英数字比較、固定形式/自由形式の lexer。(下記の算術*構文*の順列は依然と
> して対象範囲です — 「完了」なのは値の計算だけです。)

## 記法

- `[ x ]` は省略可、`{ a | b }` は選択、`…` は繰り返し、`dn` = データ項目 n。
- **型混合の軸 (T):** すべてのオペランド位置は、次の受け手/送り手の種別にわたって、
  該当する場合は双方向で駆動しなければなりません:
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`。
- **種別ごとの境界値:** 空、最小、最大、1 だけ溢れる値、全スペース、全ゼロ、
  LEADING/TRAILING [SEPARATE] の符号、P によるスケーリング、V による暗黙小数点。
- 各動詞について捕捉するもの: 結果の値、**FILE STATUS / 特殊レジスタ**
  (`RETURN-CODE`、`TALLY`)、通過したオーバーフロー/例外の分岐、エラー時の無変更。

---

## パート A — DATA DIVISION の節 (未テストの挙動)

### WORKING-STORAGE SECTION
- **レベル:** 01、02–49 の入れ子、77 (独立)、66 `RENAMES a THRU b`、88。
- **PIC:** `(n)` を伴う `X A 9 S V P`、`P` によるスケーリング (左/右)、`V` の暗黙小数点、
  編集の組み合わせ、`PIC` 付きグループと PIC なしグループ。
- **USAGE:** DISPLAY、COMP/COMP‑4/BINARY、COMP‑1、COMP‑2、COMP‑3/PACKED‑DECIMAL、
  COMP‑5、INDEX、POINTER — 宣言 + 記憶域サイズ + 値のラウンドトリップ。
- **VALUE:** 数字、符号付き、英数字、表意定数、`ALL "x"`、グループへの VALUE、
  不正な VALUE (サイズ > PIC)。
- **OCCURS:** 固定長、`DEPENDING ON`、`INDEXED BY`、`ASCENDING/DESCENDING KEY`、
  多次元 (2–3)、グループへの OCCURS。
- **句:** REDEFINES (同サイズ/より小さい/より大きい、連鎖)、RENAMES、JUSTIFIED RIGHT、
  BLANK WHEN ZERO、`SIGN IS {LEADING|TRAILING} [SEPARATE]`、SYNCHRONIZED、FILLER。
- **88 条件名:** 単一値、値のリスト、`VALUE a THRU b`、複数の範囲、数字 / 英数字 /
  編集項目のホスト上、評価 + `SET … TO TRUE`。
- **初期化:** 既定値 (クラスに応じたスペース/ゼロ) と VALUE の対比、**PERFORM をまたぐ
  永続性と CALL をまたぐ永続性** (WS は最後の値を保持する)。

### LOCAL-STORAGE SECTION
- **プログラムに入るたびに再初期化される** (WS の永続性との対比)。
- VALUE 句は**入るたびに再適用される**。
- **再帰:** (再帰的な) CALL ごとに独立した LOCAL-STORAGE のインスタンスが与えられる。
- 句のカバー範囲は WS と同じ (OCCURS/REDEFINES/88/…) だが、再初期化の意味論を検証すること。

### LINKAGE SECTION
- 項目は**呼び出し側に結び付けられるまで記憶域を持たない**。結び付いていない linkage へのアクセス。
- `CALL … USING` ↔ `PROCEDURE DIVISION USING` で結び付けられる。
- **BY REFERENCE** (呼び出し側に変更が見える) と **BY CONTENT** (呼び出された側はコピーを編集する)
  と **BY VALUE** (スカラー) の対比。
- linkage 内のグループ + 基本項目、OCCURS、REDEFINES、88。
- 実引数と仮引数のあいだのサイズ/USAGE の不一致 (観察すべき挙動)。
- `ADDRESS OF` / `SET ADDRESS OF … TO` と POINTER の結び付け (サポートされている場合)。

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — CALL の引数への位置による結び付け、
  個数の不一致 (引数が少ない/多い)、順序。
- USING リスト上のパラメータごとの `BY REFERENCE | BY VALUE`。
- `RETURNING dn` — `CALL … RETURNING` に返される値、`GIVING` との対比、
  `RETURN-CODE` との対比。
- コマンドラインから結び付けられる主プログラムの `USING` (サポートされている場合)。
- すべてのパラメータ位置での型混合 (**T** を適用)。

---

## パート B — 動詞の順列マトリクス

各動詞をすべてのオペランド位置について **T** にわたって駆動してください。以下に挙げる
のは、型混合の上に重ねる*構造的な*順列 (句/文節) です。

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (複数の受け手)。
- `MOVE CORRESPONDING g1 TO g2` (名前による基本項目の対応付け)。
- 参照修飾された送り元/送り先: `MOVE a(s:l) TO b(s:l)`。
- 添字付き: `MOVE t(i) TO u(j)`、`t(i,j)`。
- 型変換 (**T** を双方向に適用): num→edited、edited→num、alnum→num、
  num→alnum (右詰め/パディング/切り捨て)、group→group (バイトコピー)、符号の扱い、
  COMP‑3↔DISPLAY、float↔fixed、figurative→各種別。

### DISPLAY
- `DISPLAY {dn|literal} …` (連結されたオペランド)。
- `[WITH NO ADVANCING]`、`UPON {CONSOLE|SYSOUT|mnemonic}`。
- 画面形式 (観察して決定する): `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`。
- 型混合: 数字 (PIC の全幅)、編集項目、符号付き、グループ、表意定数。

### ACCEPT  *(すべての形式を仕様化する。多くは画面/端末向け — 範囲の判断のために印を付ける)*
- `ACCEPT dn` (コンソールから alnum / numeric / edited / group へ)。
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`。
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`。
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`。
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`。
- 画面形式: `ACCEPT dn AT {nnnn|LINE n COL n}`、
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`、
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`、
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`。
- 数字項目・numeric-edited・alnum のいずれで受け取るか (逆編集 / 妥当性検査)。

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`。
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`。
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`。
- `SUBTRACT … FROM …`、`SUBTRACT … GIVING …`、`SUBTRACT CORRESPONDING …`。
- 複数の受け手がそれぞれ独自の ROUNDED/サイズ挙動を持つ場合、USAGE が混在した
  オペランド (COMP‑3 + DISPLAY + 編集項目)、符号付き、参照修飾されたオペランド。

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`。
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`。
- ゼロ除算 → ON SIZE ERROR、REMAINDER の符号/位取り、USAGE の混在。

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`。
- 演算子 `+ - * / **`、括弧、優先順位、式の中の組み込み関数、USAGE が混在した
  オペランド、複数の受け手、切り捨てと ROUNDED の対比。

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — 入れ子、空の分岐、`NEXT SENTENCE`。
- 条件: 関係条件 (`= < > <= >= NOT`)、クラス条件 (`IS [NOT] {NUMERIC|ALPHABETIC|
  ALPHABETIC-UPPER|ALPHABETIC-LOWER}`)、符号条件 (`POSITIVE|NEGATIVE|ZERO`)、
  88 条件の参照、複合条件 (`AND/OR/NOT`)、**省略形** (`a = b OR c`)、
  括弧付き。
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` と
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`。
- 比較における型混合 (数字・alnum・編集項目・表意定数の対比)。

### PERFORM
- 別置きの `PERFORM p1 [THRU p2]`。
- `PERFORM p [THRU p2] n TIMES` (n = 定数 / データ項目)。
- `[WITH TEST {BEFORE|AFTER}]` を伴う `PERFORM … UNTIL cond`。
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`。
- インラインの `PERFORM … END-PERFORM` (TIMES/UNTIL/VARYING 付き)。
- 入れ子/再帰の PERFORM、範囲の重なり、指標とループ変数が数字項目の場合の対比。

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`、`GO TO p1 p2 … DEPENDING ON dn` (範囲内/範囲外)。
- `CONTINUE`、`NEXT SENTENCE`。
- `EXIT`、`EXIT PERFORM [CYCLE]`、`EXIT PROGRAM`、`EXIT PARAGRAPH/SECTION`。
- `STOP RUN`、`STOP literal`、`GOBACK` (主プログラムからと副プログラムからの対比)。

### SET
- `SET index TO {n|index}`、`SET index {UP|DOWN} BY n`。
- `SET 88-name TO TRUE`。
- `SET pointer TO {ADDRESS OF dn|NULL}`、`SET ADDRESS OF linkage TO pointer`。
- `SET d1 TO {TRUE|FALSE}` (サポートされている場合)。

### INITIALIZE
- `INITIALIZE dn …` (グループ/基本項目、カテゴリに応じた既定値)。
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`。
- `[WITH FILLER]`、`[THEN TO DEFAULT]`、表 (すべての繰り返し)。

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (逐次)。
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH` (二分探索。
  `ASCENDING/DESCENDING KEY` + `INDEXED BY` が必要)。
- 発見/未発見、複数の WHEN、キーの型混合、未整列の表での挙動。

### STRING  *(利用者の順列スタイルを駆動する)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`。
- カバーすべき順列:
  - 単一の送り元 `DELIMITED BY SIZE` → alnum の送り先。
  - 複数の送り元で**区切りが混在**: `STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`。
  - 多数の送り元/区切り: `STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`。
  - `WITH POINTER` の開始/前進、範囲外のポインタ → オーバーフロー。
  - 送り先が小さすぎる → `ON OVERFLOW`、`NOT ON OVERFLOW`。
  - **型が混在した送り元:** 数字、numeric-edited、符号付き、グループ、表意定数、
    参照修飾 — それぞれがどう文字列化されるかを観察する。

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`。
- 順列: 区切りが単一の場合と複数の場合、`ALL` (繰り返しをまとめる)、`OR`、
  `DELIMITER IN`/`COUNT IN` による捕捉、POINTER、TALLYING、データよりフィールドが多い
  場合 (オーバーフロー)、型が混在した送り先 (数字の受け手は逆編集される)。

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`。
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`。
- `INSPECT dn TALLYING … REPLACING …` (組み合わせ)。
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`。
- BEFORE/AFTER のスコープ、重なり合う一致、複数文字のパターン、型が混在したホスト。

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`。
- 静的 (定数) と動的 (データ名) のプログラム名、解決できない場合 → ON EXCEPTION。
- 引数の受け渡し方式 (呼び出し側から見えるかを観察する)、引数の個数/型の不一致。
- `RETURNING` と `RETURN-CODE` の対比、再帰、`EXTERNAL` の共有データ。
  (✅ `CANCEL prog` は実装済み — そのプログラムの記憶域を再初期化する。
  `NOT ON EXCEPTION` は解決できた CALL で実行される。)

### ARITHMETIC の特殊レジスタとその他の動詞
- `ADD/SUBTRACT … GIVING` のゼロ抑制と `TO` による累積の対比。
- `RETURN-CODE`、`TALLY` への/からの `MOVE`/算術。
- ✅ `ALTER` (旧来の GO TO) — 実装済み (その段落の `GO TO` を付け替える)。
- 編集項目を通した `ACCEPT/DISPLAY` のラウンドトリップ。

### ファイル動詞 — *(ファイル入出力スイートに含まれない抜けのみ)*
- ✅ **実装済み・テスト済み** (`test_file_locking`): `OPEN … SHARING WITH …
  [WITH LOCK]`、`READ … WITH [NO] LOCK`、`UNLOCK` (単一の実行単位の中では勧告的 —
  サポート構文のリファレンスを参照)。
- `READ … INTO`、`WRITE … FROM`、`REWRITE … FROM`、`START … KEY IS {= > >= < <=}`
  を参照修飾されたキーとともに。1 つのレコード領域を共有する複数の FD。

### 予定されている動詞 (実装時に備えた仕様)
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`、`RELEASE`、`RETURN`。
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`。
- `RELATIVE` 編成: `RELATIVE KEY` による `READ/WRITE/REWRITE/DELETE/START`。

---

## パート C — 形式間の等価性ハーネス

上記のプログラムから選び出した一群について、同一ソースの 3 つの実行形式にわたって
観測可能な出力 (DISPLAY のテキスト、FILE STATUS、RETURN-CODE、ファイルの内容) が
**同一**であることを表明します:

1. **インタプリタ** (`Interpreter::run`)。
2. **AST のラウンドトリップ** — 直列化 (`bincode`+`flate2`) → 復元 → 実行。AST が
   バイト単位で同一であること、出力が同一であることを表明する。
3. **パック済み/コンパイル済みバイナリ** — `cobolt_compiler::build_project` → 生成された
   バイナリを実行し、出力が同一であることを表明する。

形式間のいかなる相違も記録すべき欠陥です (「一つのコンパイラ、一つの挙動」という
不変条件)。

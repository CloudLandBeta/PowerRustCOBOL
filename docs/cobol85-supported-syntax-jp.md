<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL-85 サポート構文リファレンス

**RustCOBOL のレキサ／パーサ／ランタイムが現時点で実際に受け付けるものの
根拠**であり、ソースコード（`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`）
から導出しています。テストは ✅ の形式に対して書いてください。❌ の形式は解析に
失敗するか何もしません。⚠️ の形式は解析はされますが動作は部分的です。本書は
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md) の対になる文書で、
マトリクスが*何を*テストするかを示すのに対し、本書は*どの綴りを RustCOBOL が
理解するか*を示します。

凡例：✅ サポート済み · ⚠️ 解析されるが部分的／簡略化 · ❌ 認識されない
（使用を避けるか、ギャップの確認のためだけにテストしてください）。

> **更新（ギャップ実装のパス）：** 以下が実装され、現在は ✅ です — **参照修飾**
> `id(開始:長さ)`、**インライン `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING の `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、
> **カテゴリを認識する `INITIALIZE`**、**演算子前置の省略条件**
> （`a > 1 AND < 9`）、**`CALL … ON EXCEPTION`**（解決できない CALL で実行）、
> **`COMPUTE` の複数受け取り + 受け取りごとの `ROUNDED`**、そして大幅に拡張され
> た**組み込み関数**群。
>
> **更新（階層的／オカレンス対応環境のパス — 1.5.0）：** データモデルが妨げて
> いた 4 つの機能が ✅ になりました — **実行時のテーブル添字**
> `t(i)` / `t(i, j)`（オカレンス単位のストレージ）、**修飾名の曖昧性解消**
> `id OF/IN グループ`（重複した末端名が独立したストレージに解決されます）、
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**、そして**機能する `SEARCH` /
> `SEARCH ALL`**。
>
> **更新（動詞の網羅性のパス — 1.6.0）：** さらに以下も ✅ です — `ADD`/`SUBTRACT`
> における**複数受け取りの `MULTIPLY`/`DIVIDE GIVING` + 受け取りごとの
> `ROUNDED`**、**`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`**
> および修正された単独の `EXIT`、**`CALL … NOT ON EXCEPTION`**、
> **`INSPECT … TALLYING … REPLACING`** の併用と **`BEFORE/AFTER INITIAL`**
> 領域、日付・財務系の**組み込み関数**（`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、
> `INTEGER-OF-DAY`、`DAY-OF-INTEGER`、`ANNUITY`、`FRACTION-PART`）、
> **リテラル目的語の省略条件**（`A = 1 OR 2 OR 3`）、**`EVALUATE … ALSO`**
> （複数主語）と **`WHEN NOT`**、**本物の 88 レベル条件名**
> （`SET … TO TRUE/FALSE`。ホスト項目をその VALUE／範囲と照合します）、
> **`PERFORM para VARYING`**、そして機能する **`SORT`/`MERGE`** ランタイム
> （`RELEASE`/`RETURN`、`USING`/`GIVING`、`INPUT`/`OUTPUT PROCEDURE`）。末尾の
> 回避リストは最新です。
>
> **更新（回避リスト解消のパス — 1.7.0）：** 残っていたギャップも実装済みです —
> **識別子目的語の省略**（`a = b OR c`。88 レベルのメタデータで解決）、
> **`INITIALIZE … REPLACING カテゴリ DATA BY 値`**、**`66 RENAMES`**（読み取りは
> 合成し、書き込みは対象項目へ分配）、**ポインタ**（`USAGE POINTER`、
> `SET ptr TO ADDRESS OF x / NULL`、`SET ADDRESS OF item TO …` による別名付け、
> `IF ptr = NULL`）、**`ALTER`** / **`UNLOCK`**、忠実な **`NEXT SENTENCE`**、
> 残りの標準**組み込み関数**（`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、
> `NUMVAL-F`、`TEST-NUMVAL`）、そして拡張された**画面 `ACCEPT`/`DISPLAY`**
> （CLI モードでは ANSI 経由の `AT`/`WITH`。解析されるだけでなく*実行*されるよう
> になりました）。
>
> **更新（1.7.1）：** `ACCEPT` のレジスタソースが機能するようになりました
> （以前は認識されるだけの無処理でした） — **`FROM COMMAND-LINE`**、
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`**（`DISPLAY n UPON
> ARGUMENT-NUMBER` と対で使用）、**`ENVIRONMENT-VALUE`**（`DISPLAY "name" UPON
> ENVIRONMENT-NAME` と対で使用）、**`ESCAPE KEY`** → `"00"`、
> **`CRT STATUS`** → `"0000"`。
>
> **更新（1.7.2）：** ファイル共有／ロックの句と `CANCEL`（以前は ❌ ／無処理）
> — **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`**（そのファイルの INDEXED レコードロックを解放）、そして
> **`CANCEL プログラム`**（プログラムのストレージを再初期化）。
>
> **更新（1.8.0）：** **`COMMIT` / `ROLLBACK`** が本物の COBOL 動詞になりました
> — 開いている INDEXED ファイルに対する、プログラム制御のトランザクションです
> （メモリエンジンとディスクエンジンの両方）。ディスクエンジンには実行中の本物の
> アンドゥログが加わりました（以前は無処理でした）。末尾の回避リストは最新です。

---

## 認識される文（動詞）

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2`（para-1 の `GO TO` を付け替えます）·
`UNLOCK file`（そのファイルのレコードロックを解放）· `OPEN … SHARING/WITH LOCK`
· `READ … WITH [NO] LOCK`（ファイルの共有／ロック。単一の実行単位内では助言的）
✅ `COMMIT` / `ROLLBACK`（プログラム制御の INDEXED ファイルトランザクション。
ファイル動詞の項を参照）· `CANCEL`（プログラムのストレージを再初期化）·
⚠️ `INVOKE`（無処理として解析）
プロジェクト拡張：`EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。
ブロックは常にリンクされる crate（std、egui、eframe、およびリンク済みランタイム
一式）に加え、**project が Project's Crates に登録した任意の crate**（spec 044）
を `use` できます。登録された crate は厳密なバージョンに固定され、project の
`crates/` にベンダリングされてバイナリにコンパイルされます。未登録の crate は
開発者の該当行で Check/Build を失敗させ、対処方法が示されます。

✅ `SEARCH`（逐次）/ `SEARCH ALL`（`ASCENDING`/`DESCENDING KEY` を持つテーブル
に対する二分探索。最初に一致した `WHEN` を実行し、なければ `AT END`）。
✅ `RELEASE` / `RETURN` を伴う `SORT` / `MERGE`（機能します。後述）。
✅ `USE AFTER STANDARD ERROR PROCEDURE ON {file… | INPUT | OUTPUT | I-O |
EXTEND}` を伴う `DECLARATIVES … END DECLARATIVES` — 未処理のエラー
`FILE STATUS` で発火するファイルエラーハンドラ。
❌ **認識されません — 使用しないでください：** `ENTRY`、
`GENERATE`/`INITIATE`/`TERMINATE`、`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 動詞ごとのサポート形式

### MOVE
- ✅ `MOVE {id|リテラル|定数} TO id1 [id2 …]`（複数の受け取り側）。
- ✅ `MOVE CORRESPONDING g1 TO g2` — 2 つのグループが名前を共有する従属項目を
  それぞれ転記し、一致するサブグループへ再帰します。
- ✅ **参照修飾 `id(開始:長さ)`** — 送り手（部分文字列）としても受け手（部分
  代入）としても使えます。すべての動詞のオペランドで機能します。`長さ` は省略
  可能です。
- ✅ 添字 `t(i)`、`t(i, j)` — そのオカレンスのストレージスロットを読み書きし
  ます。可変添字 `t(WS-I)` はアクセスのたびに評価されます。
- ✅ 修飾 `id OF/IN グループ`（`… OF g1 OF g2`）— 末端名が複数のグループの下で
  宣言されていても、正しい項目に解決されます。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **受け取り側ごとの `ROUNDED`** — 各受け取り側が自分の `ROUNDED` 指定を持ち
  ます。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — 一致する数値の組をそれぞれ
  演算し、一致するサブグループへ再帰します。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **複数の `GIVING` 受け取り側**。それぞれが自分の `ROUNDED` を持ちます。
- ⚠️ `DIVIDE a BY b`（`GIVING` なし）は `a/b` を `a` に書き戻します
  （PowerRustCOBOL の便宜。標準 COBOL はここで `INTO` か `GIVING` を要求します）。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = 式 [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **複数の受け取り側。それぞれが自分の `ROUNDED` を持ちます**。
- ✅ 式の演算子 `+ - * /` と `**`（べき乗、右結合）、括弧、
  `FUNCTION 名前(引数)`。

### IF / EVALUATE
- ✅ `IF 条件 [THEN] 文 [ELSE 文] [END-IF]`。
- ✅ `EVALUATE {式 | TRUE | FALSE} [ALSO 主語 …]` … `WHEN {値 | 値 THRU 値 |
  NOT 値 | 条件 | ANY} [ALSO …] 文 … [WHEN OTHER 文] END-EVALUATE`。
- ✅ **`ALSO` による複数主語** — 各 `WHEN` の列が対応する主語と位置的に照合され、
  AND で結合されます。
- ✅ **`WHEN NOT 値`** は選択対象を否定します。**`WHEN 条件`**
  （例：`EVALUATE TRUE WHEN a > b`）は真偽条件を評価します。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES`（n は整数リテラルまたはデータ項目）。
- ✅ `PERFORM p UNTIL 条件 [WITH TEST {BEFORE|AFTER}]`。
- ✅ インラインの `PERFORM UNTIL 条件 … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL 条件 … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ インラインの `PERFORM n TIMES … END-PERFORM`（段落なし）。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — 反復のたびに段落を
  実行します（アウトオブライン、`END-PERFORM` なし）。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`。
- ✅ `CONTINUE` · `STOP RUN` · `STOP リテラル`。
- ✅ 単独の `EXIT` は何もしない復帰点です。`EXIT PROGRAM` は呼び出し元へ戻り
  ます。
- ✅ `EXIT PERFORM [CYCLE]`（最も近いインライン PERFORM の break / continue）、
  `EXIT PARAGRAPH`、`EXIT SECTION`。
- ✅ `NEXT SENTENCE` — 次の文の境界の先へ制御を移します（パーサが各ピリオドに
  境界マーカーを挿入します。単なる `CONTINUE` ではなく忠実な実装です）。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | ニーモニック}`。
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` はカーソルを位置付けます（ANSI、
  CLI）。
- ✅ `FROM COMMAND-LINE`（コマンドライン全体）· `FROM ARGUMENT-NUMBER`（引数の
  個数）· `FROM ARGUMENT-VALUE`（`DISPLAY n UPON ARGUMENT-NUMBER` で設定した
  ポインタ位置の引数）· `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`
  （`DISPLAY "name" UPON ENVIRONMENT-NAME` で指定した変数）·
  `FROM ESCAPE KEY` → `"00"` · `FROM CRT STATUS` → `"0000"`。

### DISPLAY
- ✅ `DISPLAY {id|リテラル} … [UPON ニーモニック] [[WITH] NO ADVANCING]`。
- ✅ 画面形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — **CLI モード**（`rcrun`）
  では ANSI のカーソル位置指定 + SGR で実行されます。GUI モードでは無視されます
  （そこでは form designer が SCREEN I/O に取って代わります）。
  `ACCEPT id AT …` は位置付けてから読み取ります。

### STRING
- ✅ `STRING {送り元 [DELIMITED BY {SIZE | SPACE[S] | 区切り}]} … INTO 受け先
  [WITH POINTER p] [[ON] OVERFLOW 命令] [NOT [ON] OVERFLOW 命令] [END-STRING]`。
  オーバーフロー＝組み立てた文字列が受け取り項目より長い場合。
- ✅ **拡張 — 賢い既定の `DELIMITED BY`**（オペランドで句を省略した場合）：
  英数字の `PIC X`/`A` 項目は既定で `SPACES`（末尾の詰めを落とします）。文字列
  リテラル、数字項目、数字編集項目、`FUNCTION` の結果、式は既定で `SIZE` です。
  データ項目は項目の形のまま転記されます（数字 → PIC 幅いっぱいの数字、数字編集
  → 編集後の文字）。

### UNSTRING
- ✅ `UNSTRING 送り元 [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW 命令]
  [NOT [ON] OVERFLOW 命令] [END-UNSTRING]`。オーバーフロー＝送り元の項目数が受け
  取り側より多い場合。

### INSPECT
- ✅ `INSPECT id CONVERTING 変換元 TO 変換先`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` — **両方の処理が適用されます**。
- ✅ `BEFORE/AFTER INITIAL` は各句を項目の部分領域に限定します。
  （TALLYING は COBOL のとおりカウンタに加算します。）

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | 式}`（MOVE にコンパイルされます）。
- ✅ `SET idx {UP|DOWN} BY n`（ADD / SUBTRACT として符号化されます）。
- ✅ `SET 88-名前 TO TRUE` はホスト項目に条件の最初の VALUE を設定します。
  `TO FALSE` は VALUE の集合の外にある値を設定します（ベストエフォート。FALSE 句
  はありません）。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | 別の ptr}` および
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — 後述の**ポインタ**を参照。

### INITIALIZE
- ✅ `INITIALIZE id …` — カテゴリを認識します。数字／数字編集 → ZERO、それ以外
  → SPACES。グループ項目には再帰します。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY 値 …` — そのカテゴリの従属項目
  をすべてその値にします。ほかは変更されません。

### ポインタ（USAGE POINTER）
- ✅ `USAGE POINTER` はポインタを宣言します（初期値は NULL）。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — `id` を対象のストレージ
  への別名にします（読み取り**も**書き込みも別名に従います）。通常は LINKAGE の
  レコードに対して使います。`IF ptr = NULL` も機能します。

### CALL / CANCEL
- ✅ `CALL {リテラル|id} [USING [BY {REFERENCE|CONTENT|VALUE}] 引数 …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} 命令] [NOT [ON] {EXCEPTION|OVERFLOW} 命令] [END-CALL]`。
- ✅ `ON EXCEPTION` / `ON OVERFLOW` の本体は、呼び出し先プログラムが解決できない
  ときに実行されます。`NOT ON EXCEPTION` の本体は、呼び出しが**解決できた**とき
  に実行されます。
- ✅ `CANCEL プログラム …` は指定プログラムの WORKING-STORAGE を再初期化し、次の
  `CALL` が初期状態から始まるようにします。

### ファイル動詞（サポートされる句。完全な網羅はファイル I/O スイートにあります）
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`。
  （`SHARING` / `WITH LOCK` は解析され、意味のある場面では尊重されます。単一実行
  単位モデルでは助言的です。）
- ✅ **`OPEN … WITH REGISTERED [USER] {リテラル | データ項目}`**
  （PowerRustCOBOL 拡張）— 操作者／ユーザーを INDEXED の可観測性ログに記録します
  （そのファイルのセッションのすべてのイベント行に `user=` フィールドが付き
  ます）。純粋に観測用で、認証・認可は行いません。
  [`observability.md`](observability.md) §1.3.1 を参照。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` は、I-O のもとで INDEXED エンジンが取得するレコードロックを解放
  します。
- ✅ `UNLOCK f [RECORD[S]]` はそのファイルのレコードロックを解放します。
- ✅ **`COMMIT` / `ROLLBACK`** — 開いている**すべての** INDEXED ファイルに対する
  プログラム制御のトランザクション。`OPEN` がトランザクションを開始し、`COMMIT`
  が保留中の `WRITE`/`REWRITE`/`DELETE` を確定して（以後の `ROLLBACK` では取り
  消せなくなります）新しいトランザクションを開始します。`ROLLBACK` は直前の
  `COMMIT`/`OPEN` 以降のすべての変更を取り消します。**DISK** ストレージでは
  `COMMIT`/`CLOSE` がディスク上で永続化されます。**MEMORY** ストレージでは
  `COMMIT`/`ROLLBACK` は純粋に RAM 上で完結します（ディスクには一切書きません）。
  素の `STORAGE IS MEMORY` ファイルは一時的で、
  `STORAGE IS MEMORY WITH PERSISTENCE` は `CLOSE` のときだけディスクへ保存し
  ます。（永続的な write-ahead ログによるクラッシュ復旧は今後の課題です。これは
  実行中の、プログラムレベルのロールバックです。）
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`**（INDEXED ファイル。PowerRustCOBOL 拡張）。既定のストレージは
  `DISK` です。`WITH COMPRESSION` は保存されるレコードを圧縮します（キーは非圧縮
  のレコードに対して評価されます）。`WITH PERSISTENCE`（MEMORY のみ）は RAM 上の
  ファイルを `CLOSE` 時に保存します。`OPEN OUTPUT` は常にディスク上のコンテナを
  （再）作成します。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ *プロセス*をまたぐファイル共有は強制されません（単一実行単位）。
  `SHARING`/`LOCK` の句は解析され、INDEXED エンジンの実行単位内レコードロックは
  尊重されます。

### SORT / MERGE / RELEASE / RETURN  ✅（機能します。作業バッファはメモリ上）
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]`（INPUT PROCEDURE 内）は実行対象に追加します。
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` がレコードを返します。
- レコードは宣言されたキー（`ASCENDING`/`DESCENDING`）で安定ソートされます。
  `USING` が読み、`GIVING` が指定された順ファイルへ書き出します。

---

## 条件（IF / EVALUATE / PERFORM UNTIL）

- ✅ 関係記号：`=` `<>` `<` `>` `<=` `>=`。
- ✅ 語による関係：`[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`、`[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ クラス：`id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
- ✅ 符号：`id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 レベルの条件名（名前のみを条件として使用）。
- ✅ `AND` / `OR` / `NOT` の組み合わせ、括弧（AND のほうが OR より強く結合）。
- ✅ **演算子前置の省略条件** — `a > 1 AND < 9`、`a = 5 OR = 7`（直前の比較の
  主語が再利用されます）。
- ✅ **リテラル目的語の省略** — `a = 1 OR 2 OR 3`（主語と演算子の両方を再利用
  します。目的語はリテラルです）。
- ✅ **識別子目的語の省略** — `a = b OR c`（`c` はデータ項目）。比較に続く
  AND/OR の後ろの裸の識別子は実行時に解決されます。既知の 88 レベル条件名であれば
  条件として評価され、そうでなければ目的語 `a = c` になります。（直後に `AND` が
  続く識別子は AND の優先順位を保ちます。）

---

## 式、リテラル、USAGE

- ✅ 算術演算子 `+ - * /` と `**`、括弧、単項の `+`/`-`。
- ✅ `FUNCTION 名前 ( 引数 [ , 引数 … ] )` — **実装済み**の組み込み関数：
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM（シード省略可）, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`。
  （日付変換は標準の基準 1601-01-01 = 第 1 日を使用します。）**COBOL-85 標準の
  組み込み関数一式**が実装されています。
  ⚠️ 認識されない `FUNCTION` 名も解析はされますが、実行時に **0** を返します。
- ✅ リテラル：整数、小数、文字列、すべての定数
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **16 進リテラル** — `X"09"`、`x'0D0A'`（大文字小文字・引用符の種類は問い
  ません）。16 進数字の**ペア**ごとに 1 文字なので、桁数は偶数でなければなり
  ません。奇数桁や 16 進でない文字は不正なリテラルとして報告され、文字列の隣に
  ある単語 `X` として黙って読み直されることはありません。引用符付きリテラルが
  使える場所ならどこでも使えます（`DELIMITED BY`、`MOVE`、`VALUE`、比較）。

---

## DATA DIVISION の句（受け付ける宣言構文）

- ✅ レベル `01`–`49`、`77`、`88`、`FILLER`、グループ／基本項目。
- ✅ `PIC/PICTURE`（`X A 9 S V P` と編集記号 `Z * $ + - CR DB B 0 / , .`）。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}`（および `COMP-4`→COMP、`COMP-X`→COMP-5）。
- ✅ `VALUE`（数値／符号付き／英数字／定数／`ALL`）。
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES`、`JUSTIFIED [RIGHT]`、`SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL`。
- ✅ `88 名前 VALUE v [v …]` / `VALUE a THRU b` — **本物の条件名**です。88 レベル
  はホスト項目に束縛され、判定はホストを VALUE ／範囲と照合します。
  `SET 88-名前 TO TRUE` は条件を満たす値をホストに格納します。
- ✅ `USAGE INDEX` は整数の指標レジスタを宣言します（`SET`/`SEARCH` が使用）。
  `USAGE POINTER` — 上記の**ポインタ**を参照。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — 再グループ化の別名です。
  読み取りは対象項目を連結し、書き込みは項目幅に従って分配します。
- セクション：`WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`。`SCREEN` は
  解析されますが実行はされません。

---

## まだ未サポート — 現在の回避リスト

COBOL-85 の動詞・句の集合は**完全に網羅**されています。範囲外として残っている
ものは、意図的なものか 85 以降のものです。

1. **画面 `ACCEPT` の入力編集** — `DISPLAY … AT/WITH` と `ACCEPT … AT` は CLI
   モードで（ANSI により）実行されますが、SCREEN SECTION の完全な項目単位編集
   （オートタブ、項目の妥当性検査、カラーマップ）は GUI モードでは
   **form designer に取って代わられます**。
2. ***プロセス*をまたぐファイル共有** — `OPEN … SHARING/WITH LOCK`、
   `READ … WITH [NO] LOCK`、`UNLOCK` は解析され、INDEXED エンジンの実行単位内
   レコードロックを駆動しますが、別々の OS プロセス間ではロックは強制されません
   （単一実行単位モデル）。
3. **オブジェクト指向 COBOL**（クラス／メソッド定義）— `INVOKE` は COBOL の
   オブジェクトに対しては無処理です（GUI ／ランタイムのオブジェクトのみを操作
   します）。
4. **RELATIVE** ファイル編成（SEQUENTIAL / LINE SEQUENTIAL / INDEXED は完了）。
5. 認識されない組み込み関数名は依然として **0** を返します。

> **解決済み（1.5.0）：** 平坦なデータモデルが階層的／オカレンス対応になり、
> **CORRESPONDING**、**修飾名**、**テーブル添字**、**`SEARCH`** の障害が
> 取り除かれました。
> **解決済み（1.6.0）：** 複数受け取りの `MULTIPLY`/`DIVIDE` と受け取りごとの
> `ROUNDED`、`EXIT PERFORM/PARAGRAPH/SECTION`、`CALL NOT ON EXCEPTION`、
> `INSPECT TALLYING REPLACING` の併用と `BEFORE/AFTER INITIAL`、日付／`ANNUITY`
> の組み込み関数、リテラル目的語の省略、`EVALUATE ALSO`/`WHEN NOT`、本物の
> 88 レベル条件名、`PERFORM para VARYING`、そして `RELEASE`/`RETURN` を伴う
> `SORT`/`MERGE` ランタイム。
> **解決済み（1.7.0）：** 識別子目的語の省略、`INITIALIZE … REPLACING`、
> `66 RENAMES`、ポインタ（`USAGE POINTER`、`SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`）、`ALTER` / `UNLOCK`、忠実な `NEXT SENTENCE`、残りの標準組み込み関数、
> そして拡張された画面 `ACCEPT`/`DISPLAY`（CLI モードで実行）。
> **解決済み（1.7.1）：** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS`（対になる
> `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` レジスタとともに）。
> **解決済み（1.7.2）：** `OPEN … SHARING/WITH LOCK`、`READ … WITH [NO] LOCK`、
> `UNLOCK`（INDEXED のレコードロックを解放）、`CANCEL プログラム`。
> **解決済み（1.8.0）：** プログラム制御の INDEXED ファイルトランザクションとして
> の `COMMIT` / `ROLLBACK`（メモリ／ディスク両エンジン。ディスクには本物のアンドゥ
> ログ）。

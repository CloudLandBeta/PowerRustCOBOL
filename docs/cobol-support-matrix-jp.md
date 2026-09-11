<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.135 -->

# PowerRustCOBOL 対応マトリクス

**この文書の役割:** *「PowerRustCOBOL は X ができるのか、そして X は標準 COBOL
なのか、それともこのプラットフォームが足したものなのか」*に答える、ひと目で追える
一か所です。あらゆる機能が 1 行になっています。散文の列挙はしません — 対応している
なら、指させる行があります。

これは**概観**です。詳細は 2 つの姉妹文書が受け持ちます。

| 文書 | 何に答えるか |
|---|---|
| [`cobol85-supported-syntax-jp.md`](cobol85-supported-syntax-jp.md) | 各文の**どの書き方**を字句解析器・構文解析器・ランタイムが実際に受け付けるか、および NIST CCVS85 適合度のスコアボード |
| [`cobol85-verb-test-matrix-jp.md`](cobol85-verb-test-matrix-jp.md) | 各動詞について**何を試験するか** |
| [`developers-guide-en.md`](developers-guide-en.md) | それらすべてを使ってアプリケーションを作る方法 |

---

## 表の読み方

各機能の行には 3 つの出自の印が付き、そのうえで状態が与えられます。

| 列 | 意味 |
|---|---|
| **85** | **COBOL-85**（ANSI X3.23-1985。注記のある箇所では 1989 年の組込み関数の追補も含む）で定義 |
| **20xx** | **より新しい ISO 規格**で定義 — COBOL 2002 / 2014 / 2023、および現在 2026 年に向けて起草中のもの |
| **PRC** | **PowerRustCOBOL の拡張** — どの COBOL 規格にも無い |
| **状態** | この実装がそれをどう扱うか |

1 つの機能が複数の出自の列に印を持つこともあります。後年の規格が拡張した COBOL-85
の機能は両方に `●` が付き、**備考**の列に後年の規格が何を足したかを書きます。

**出自の印:** `●` ここで定義 · `○` ここで拡張または明確化 · `—` この規格には無い。

**状態の印:** `✅` 対応済み · `🚧` 部分的または簡略化 · `⛔` 予定、まだ未実装 ·
`🚫` 設計上の対象外で、実装されることはありません。

> **正直な断り書き。** PowerRustCOBOL が狙うのは、実務的でアプリケーション寄りの
> 部分集合に、ビジュアルな RAD 拡張を足したものです。認証を受けた COBOL-85 実装では
> **ありません**。適合性は主張するのではなく、公式の NIST CCVS85 一式に対して
> *測定*しています —
> [スコアボード](cobol85-supported-syntax-jp.md)を参照してください。

---

## 1. ソース形式とプログラム構造

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| 固定形式のソース、**緩和版**（`fixed-relaxed`） | ● | ○ | ○ | ✅ | **既定値。** 一連番号領域と標識欄は尊重しますが、行は開発者が書いたところまで続きます — 72 桁での切り捨てはありません。フォームから生成される `.cbl` と `EXEC RUST` ブロックはこれを必要とします |
| 固定形式のソース、**COBOL-85 の古典的な参照形式**（`--source-format=fixed`） | ● | ○ | — | ✅ | 桁の規則をすべて適用します。1–6 が一連番号、7 が標識（`*` `/` は注記、`-` は継続、`D` はデバッグ行）、8–72 が原文、**73–80 は捨てられ**、継続の連結も標準どおり（継続された英数字定数を含む）。NIST CCVS85 のカードイメージ一式はこれで書かれています。**明示的に選ぶものであり、検出で選ばれることは決してありません** — これを前提としていないソースにこの規則を当てると、黙ってコードを消してしまいます |
| 自由形式のソース | — | ● | — | ✅ | COBOL 2002（`--source-format=free`） |
| ソース形式の切り替え — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | `COBOLT_SOURCE_FORMAT` も使えます。`auto` は先頭の数行を調べますが、厳密形式を選ぶことは決してありません |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ |  |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ |  |
| DATA DIVISION | ● | ○ | — | ✅ |  |
| PROCEDURE DIVISION | ● | ○ | — | ✅ |  |
| 入れ子プログラム | ● | ○ | — | ✅ |  |
| 1 つのファイルに複数の逐次プログラム単位 | ● | ○ | — | ✅ |  |
| `COPY` / `REPLACE` のコピーブック | ● | ○ | — | ✅ | 疑似テキストと語の置換、入れ子の `COPY`、`REPLACE OFF`。ソースの隣にある `.cpy`/`.cbl`/`.cob` を大文字小文字を区別せずに解決します |
| `REPOSITORY` 段落 | — | ● | ○ | ✅ | クラスについては COBOL 2002。PowerRustCOBOL はここで **Rust の FFI** 型も束縛します |
| `EXEC RUST … END-EXEC` によるインラインの Rust | — | — | ● | ✅ | バイナリにコンパイルされます。エラーは開発者自身の COBOL の行と桁で報告されます |

## 2. DATA DIVISION とデータ記述

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ |  |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ |  |
| FILE SECTION | ● | ○ | — | ✅ |  |
| SCREEN SECTION | ● | ○ | — | 🚧 | `AT`/`WITH` を伴う拡張 `ACCEPT`/`DISPLAY` は CLI モードで ANSI 経由で動きます。項目単位の画面編集は、GUI モードではビジュアルなフォームデザイナーに取って代わられています |
| COMMUNICATION SECTION（`CD`、メッセージ制御） | ● | — | — | 🚫 | 通信処理。以降の規格では廃止されています |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | 設計上、対象外です |
| `(n)` の反復を伴う `PICTURE` X / A / 9 / S / V | ● | ○ | — | ✅ |  |
| 数字編集の PICTURE（`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`） | ● | ○ | — | ✅ | ゼロ抑制、小切手保護、固定および浮動の `$` と符号 |
| `USAGE DISPLAY` | ● | ○ | — | ✅ |  |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ |  |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | 浮動小数点。のちに `FLOAT-SHORT`/`FLOAT-LONG` として標準化されたベンダー拡張 |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ |  |
| `USAGE COMP-5` | — | ○ | ● | ✅ | ネイティブのバイナリ。ベンダー拡張 |
| `USAGE INDEX` | ● | ○ | — | ✅ |  |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002。読み取り**と**書き込みの別名 |
| 固定の `OCCURS` | ● | ○ | — | ✅ |  |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ |  |
| `INDEXED BY` | ● | ○ | — | ✅ |  |
| レベル番号 01–49、77 | ● | ○ | — | ✅ |  |
| レベル 66 の `RENAMES` | ● | ○ | — | ✅ |  |
| レベル 88 の条件名 | ● | ○ | — | ✅ | `SET … TO TRUE` を含む |
| `VALUE` 句 | ● | ○ | — | ✅ |  |
| 集団項目、`FILLER` | ● | ○ | — | ✅ |  |
| `REDEFINES` | ● | ○ | — | ✅ |  |
| 定数（`SPACES`、`ZEROS`、`HIGH-`/`LOW-VALUES`、`QUOTES`、`NULLS`） | ● | ○ | — | ✅ |  |

## 3. PROCEDURE DIVISION — 動詞

| 動詞 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | 集団項目の従属項目どうしの対応づけ |
| `DISPLAY` | ● | ○ | — | ✅ | 数値は PIC の全幅で描かれます |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ |  |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | 複数の受け手。`ROUNDED` は受け手ごと |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | 複数の受け手。`ROUNDED` は受け手ごと |
| `COMPUTE` | ● | ○ | — | ✅ | 複数の受け手。`ROUNDED` は受け手ごと |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ |  |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ |  |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ |  |
| 行内の `PERFORM`、`TIMES`、`UNTIL`、`TEST BEFORE/AFTER`、`VARYING … AFTER`、`THRU` | ● | ○ | — | ✅ |  |
| `PERFORM para VARYING`（行外） | ● | ○ | — | ✅ |  |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ |  |
| `ALTER` | ● | ○ | — | ✅ | COBOL-85 では廃要素 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | 忠実な意味論。COBOL 2002 では廃要素 |
| `CONTINUE` | ● | ○ | — | ✅ |  |
| `EXIT` | ● | ○ | — | ✅ |  |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ |  |
| `GOBACK` | — | ● | — | ✅ | COBOL 2002 で標準化されたベンダー拡張 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ |  |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | COBOL 2002 のポインター |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | 分類を意識し、集団項目を再帰的にたどります |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ |  |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` の併用 |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | 表の指標を動かし、最初に合致した `WHEN` を実行し、無ければ `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`、`INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` と `RETURNING` は COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ |  |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ |  |
| `INVOKE` | — | ● | ○ | 🚧 | COBOL 2002 の OO。**GUI と実行時のオブジェクト、および Rust FFI プラグイン**については対応しています。利用者によるクラス／メソッド定義は未実装です |
| `UNLOCK` | — | ● | — | 🚧 | 実行単位内のレコードロックを操作します。OS のプロセス間では強制されません |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | INDEXED ファイルに対するプログラム制御のトランザクション。本物の取り消しログ付き |
| OO の `CLASS-ID` / `METHOD-ID` 定義 | — | ● | — | ⛔ | 予定 |

## 4. 条件と式

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| 関係条件・クラス条件・符号条件・条件名条件 | ● | ○ | — | ✅ |  |
| 略記した複合関係、演算子を前置する形（`a > 1 AND < 9`） | ● | ○ | — | ✅ |  |
| 略記した複合関係、目的語が定数の形（`a = 1 OR 2 OR 3`） | ● | ○ | — | ✅ |  |
| 略記した複合関係、目的語が一意名の形（`a = b OR c`） | ● | ○ | — | ✅ |  |
| 部分参照 `item(start:length)` | ● | ○ | — | ✅ | どのオペランドでも、読み取り**と**部分書き込みの両方 |
| 実行時の表添字 `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | 出現ごとの記憶域、可変の添字 |
| 修飾名 `id OF/IN group` | ● | ○ | — | ✅ | 複数の集団項目の下に宣言された同名の末端項目は、それぞれ独立した記憶域に解決されます |
| COBOL として正しい英数字比較（空白で詰める） | ● | ○ | — | ✅ |  |
| **厳密な固定小数点演算** | ● | ○ | ○ | ✅ | `i128` の整数仮数を使い、`f64` を経由しません。標準の 18 桁精度と**拡張の 31 桁精度**がそのまま厳密に保たれます |
| 簡潔なプロパティ式（`Output::Value`） | — | — | ● | ✅ | 式の中でコントロールのプロパティを読み書きできます。作業記憶域の一時項目は要りません |

### 4.1 データ項目に対する値メソッド

`item::Method(args)` は、コントロールに対してだけでなく、**ごく普通のデータ項目の
値**に対してメソッドを呼びます — `PIC X` の項目、集団項目、表の出現、部分参照した
断片、算術式のいずれにも。これはどれも標準 COBOL ではありません。

式が書けるところならどこでも使えます。`MOVE` の送り手として、`COMPUTE` の中で、
条件の内側で、`DISPLAY` の中に直接。メソッドは**連ねられます**:
`WS-TEXT::Trim()::Len()`。

| メソッド | 戻り値 | 状態 | 備考 |
|---|---|:--:|---|
| `Trim()` | テキスト | ✅ | 前後の空白を取り除きます |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | テキスト | ✅ | 同じメソッドに対する 3 通りの綴りを受け付けます |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | テキスト | ✅ |  |
| `Replace(from, to)` | テキスト | ✅ | すべての出現 |
| `Len()` · `Length()` | 数値 | ✅ | **項目の**長さです。したがって `hello` を保持する `PIC X(20)` は `20` と答えます。内容の長さが欲しければ `::Trim()::Len()` と連ねてください |
| `Split(sep)` | テキスト | ✅ | **最初の**フィールド |
| `Split(sep)(n)` | テキスト | ✅ | *n* 番目のフィールド（1 起点）。添字はデータ項目の受け手に対してのみ受け付けます |

| 受け手 | 状態 | 備考 |
|---|:--:|---|
| データ項目（`PIC X`、集団項目、`01`/`77`） | ✅ | 普通の場合 |
| 表の出現、部分参照、修飾名、算術式 | ✅ | 評価器は受け付けます |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | インタープリターは定数の受け手を受け付けますが、構文解析器は受け付けません。定数のあとの `::` は構文エラーです。まず定数をデータ項目に代入してください |

### 4.2 COBOL-85 が項目しか許さないところに式を書く

COBOL-85 は送り手の位置のほとんどを一意名か定数に限っています。RustCOBOL はそこで
完全な式を評価します。規格が宣言を強いてくる間に合わせの作業記憶域項目が要らなくなる
のは、そのためです。

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`。規格は送り手として一意名か定数しか認めていません |
| `SET target TO <expression>` | — | — | ● | ✅ | `COMPUTE` の形と等価です。対象はデータ項目でも、左辺値としてのコントロールのプロパティでも構いません |
| `STRING <expression> … INTO` | — | — | ● | ✅ | 送り手の項目は算術式（`STRING WS-N * 2 …`）でも値メソッドの呼び出し（`STRING WS-A::UpperCase() …`）でも構いません。`DELIMITED BY` 以降は標準のままです |
| **型推論** — `Ctrl::Property` の読み取りは第一級の型付き値です | — | — | ● | ✅ | 数値か文字かの型が式の中を流れるので、プロパティはそのまま算術、条件、送り手の位置に入ります。**間に `PIC` 項目は要りません**。`IF Slider-1::Value > 50`、`COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`。数値に見えるプロパティ値は数値として読み戻されるので、比較も算術も文字単位ではなく代数的なままです |

## 5. 組込み関数

COBOL-85 の組込み関数一式は **1989 年の追補**（ANSI X3.23a-1989）で入りました。
COBOL 2002 以降で追加された関数は `20xx` の列に印が付いています。以下はすべて実装済み
です。

| 分類 | 関数 | 85 | 20xx | PRC | 状態 |
|---|---|:--:|:--:|:--:|:--:|
| 長さと文字 | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| 長さと文字（後年の追加） | `BYTE-LENGTH`, `LENGTH-AN`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| 大文字小文字とテキスト | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| テキスト（後年の追加） | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| 数値変換 | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| 数値変換（後年の追加） | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| 算術 | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| 順序 | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| 統計 | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| 三角関数と対数 | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| 組合せ | `FACTORIAL` | ● | ○ | — | ✅ |
| 財務 | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| 日付と時刻 | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. ファイル入出力 — 編成とアクセス

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | 固定長レコード |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | 改行で終わるテキスト。書き込み時に行末の空白は落とされます |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | 組込みで依存関係の無い ISAM エンジン |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | 独自エンジン（`cobolt-runtime/src/relative.rs`、`PRCREL1` コンテナー、ディスクと MEMORY）。`RELATIVE KEY IS` は 1 から始まる整数のレコード番号でレコードを指します。3 つのアクセスモードすべてに対応し、7 つのファイル動詞すべてがこれに振り分けられます。NIST の **RL モジュールは両軸で完了** — コンパイル 35/35、実行 34/34、354 個のアサーション、失敗 0（エンジン 1.62.76、モジュール 1.62.77） |
| `RELATIVE KEY IS data-name`（`KEY` を省いた書き方も含む） | ● | ○ | — | ✅ | `KEY` の語を省いた `RELATIVE data-name` 句はキーの指定であって、単なる編成句ではありません |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | 3 つとも動きます |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | ディスク上では昇順のキー順 |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ |  |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ |  |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` は COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ |  |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | `GREATER/LESS THAN` と `NOT LESS THAN` も含む |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ |  |
| `FILE STATUS` のコード | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | 解析されて文に保持されますが、**勧告的**です。実行単位は 1 つしかないので、競合するものがありません |
| `OPEN … WITH LOCK`（ファイルを排他で開く） | — | ● | — | 🚧 | 同じく、単一実行単位のモデルでは受け付けられますが勧告的です |
| `READ … WITH LOCK` | — | ● | — | ✅ | `I-O` のもとでエンジンはすでにそのレコードを保持しています。この語句は意図を表明するものです |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | `I-O` のもとでエンジンが取るロックを実際に解放します。今日、実行時に効果のある唯一のロック語句です。`UNLOCK` はほかの動詞と一緒に §3 にあります |
| プロセス間のファイル共有とレコードロックの強制 | — | ● | — | ⛔ | 予定。今日のモデルは単一実行単位です |

## 7. ファイル入出力 — INDEXED エンジン（PowerRustCOBOL）

この節の内容はすべて、上にある標準の `ORGANIZATION IS INDEXED` の挙動を取り巻く
プラットフォームの拡張です。詳細は
[`indexed-file-format-jp.md`](indexed-file-format-jp.md)、
[`indexed-file-internals-jp.md`](indexed-file-internals-jp.md)、
[`indexed-redb-engine-jp.md`](indexed-redb-engine-jp.md) にあります。

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **既定のストレージモード。** レコードと索引は `ASSIGN` のファイルに置かれ、必要に応じて読まれるので、非常に大きなファイルでも RAM は有界に保たれます。1.62.73 以降はクラッシュに強い redb エンジンが担っています。従来のページ方式 B+tree にも `--indexed-engine rust` で到達できます |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | ファイル全体を RAM に置き、閉じるときに `ASSIGN` のパスへ永続化します |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | 依存関係の無い RLE。典型的な COBOL レコードの詰め物の連なりを 50 % を大きく超えて圧縮します |
| プログラム制御の `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | 本物の取り消しログ。メモリとディスクの両エンジンで |
| 実行単位内のレコードロック | — | ○ | ● | ✅ | 上のプロセス間に関する注意を参照 |
| エンジンの選択（`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`） | — | — | ● | ✅ | `COBOL_INDEXED_ENGINE` も使えます。いずれも挙動は互換です。**`redb` が既定**で、1.62.73 以降そうなっています（`cobolt-runtime/src/indexed.rs:126`） |
| クラッシュに強い ACID エンジン `redb` | — | — | ● | ✅ | O(1) の OPEN（20 万レコードで約 5 ms）、ワーキングセット分の RAM（2 億 5000 万レコード以上）、電源断でも索引が壊れません |
| 自己記述コンテナー `PRCIDX1` | — | — | ● | ✅ | レコード形式とキー記述子を埋め込みます。オープン時の厳密な検証で、スキーマ不一致は `39`、ファイル無しは `35` になります。Fujitsu とバイト単位の互換はありません |
| ファイルごとのトランザクションログ（`--indexed-log basic\|full`） | — | — | ● | ✅ | logfmt か、Grafana/Loki にそのまま渡せる NDJSON — [`observability-jp.md`](observability-jp.md) を参照 |

## 8. ランタイムの統合

COBOL からは実行時の `CALL` と `INVOKE` として到達します。どれも標準 COBOL では
ありません。この言語を現代のアプリケーションに使えるものにしているのが、これです。

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | 3 つとも同じ CALL の面を使い、バックエンドは接続文字列から選ばれます。**システムライブラリは不要**でホストからは何もリンクしませんが、「純 Rust」が当てはまるのは 3 つのうち 2 つだけです。`postgres` と `mysql` はそうですが、`rusqlite` は `features = ["bundled"]` で固定されており、`libsqlite3-sys` を通じて **SQLite の C アマルガメーション**をコンパイルします。（その C のビルドは、入れ子の `cargo build` の中で `test_external_crates_e2e` が時折失敗する原因でもあります。）[`database-runtime-jp.md`](database-runtime-jp.md) を参照 |
| **SQL の結果集合** — `Fetch()`、`ColumnNames()`、`ColumnCount()`、`ColumnName(n)` | — | — | ● | ✅ | `Fetch()` は次の行をタブ区切りで返し、尽きると空を返すので、それ自体がループの終了条件になります。`ColumnNames()` は、1 行も合致しなかった場合でも SELECT の順で結果集合の列名を返します。一方 `CALL` の面は、現在行を添字で 1 列ずつ読みます。この 2 つのたどり方を 1 つのハンドル上で混ぜてはいけません |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | カスタムヘッダー |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ |  |
| **グラフ** — 棒 / 折れ線 / 円 / 面 / 散布 / ドーナツ | — | — | ● | ✅ | COBOL の表に束縛されます |
| **テキストファイル** — `COBOL-APPEND-FILE`、`COBOL-WRITE-FILE` | — | — | ● | ✅ |  |
| **タイマー** | — | — | ● | ✅ |  |
| **AI エージェントのオブジェクトフック** | — | — | ● | ✅ |  |
| **Rust の FFI プラグイン** | — | — | ● | ✅ | `REPOSITORY` の下で宣言し、`INVOKE` か直接のプロパティ対応づけで呼び出します |
| **ユーザー手続き** | — | — | ● | ✅ | IDE で編集でき、`CALL "PROCEDURE-NAME"` で呼べる共有の COBOL 手続き |

## 9. 明示的に対象外

これらは実装されません。答えが「無い」のではなく「見つかる」ようにするために挙げて
あります。

| 機能 | 85 | 20xx | PRC | 状態 | 備考 |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION（`CD`、メッセージ制御／通信処理） | ● | — | — | 🚫 | 以降の規格では廃止。現代的な用途はありません |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | プラットフォーム自身の帳票機能とデータバインドに取って代わられています |
| ActiveX / OLE / COM のコントロール | — | — | — | 🚫 | 特定のプラットフォームに固有で、移植性がありません |

---

## 10. プラットフォームそのもの

COBOL 言語の機能ではなく、IDE とコンパイラー、そしてそれらを取り巻く道具立てです。
詳しい道案内は[開発者ガイド](developers-guide-en.md)にあります。

### 10.1 IDE

| 機能 | 状態 | 備考 |
|---|:--:|---|
| ビジュアルなフォームデザイナー | ✅ | 複数のテーマ（**Liquid Glass**、**Cobalt Steel**）を持つ設計キャンバス、グリッドへの吸着、コントロールとキャンバスのドラッグによるサイズ変更、複数選択での整列、重なり順 |
| 統一レンダリングエンジン | ✅ | デザイナー、プレビュー、実行中のアプリケーション、コンパイル済みバイナリのあいだでピクセル単位の一致 |
| コントロールのカタログ | ✅ | Common、Container、Data、Graphics、Menu、Non-visual、Charts にまたがる **43 のウィジェット**。加えてプラグインが提供する `Custom` 型 |
| 全体に効く角丸と丸め切り抜き | ✅ | 入れ子の子要素は、角の切り欠きマスクによって親の丸い枠に沿って切り抜かれます |
| コントロールごとの `Transparency` | ✅ | 0 = 不透明 … 100 = 透ける。面と枠と影を薄めつつ、文字・字形・境界線は読めるまま保ちます。背後にあるものに対して WCAG AA を下回るキャプションは、読める側の極へ反転します |
| Animator ウィジェット | ✅ | **GIF / WebP / APNG** をネイティブに描画します |
| Knob、Gauge、Switch、FileDropZone、Maps、Web Search | ✅ | 双極の塗りを持つ回転ダイヤル、警告域と危険域が自動で付く放射状・線形・ドーナツ型の KPI、ドラッグ＆ドロップまたはネイティブの選択ダイアログ |
| 高度なメニューエディター | ✅ | ツリー形式のビジュアルエディター、37 カテゴリーに収められた **1112** 個の組込みベクターアイコン、階層的な入れ子、設定の完全性を保証する HMAC 署名 |
| データバインドとコントロール配列 | ✅ | SQL やデータソースへの直接バインド。**ビジュアル反復グループ**が、実行時の `DataSource` の行数から GroupBox や Panel の配列を展開します |
| ビジュアルな検証とフォームインスペクター | ✅ | 不正なハンドラー、不完全なバインド、レイアウトの異常に対するリアルタイムのエラーバッジ。`rcrun` のプロセスマネージャーが CPU 使用率、RSS、ログ、スレッド数をライブで追跡します |
| フォームデバッガー | ✅ | 常に手前に出る独立ウィンドウ。ブレークポイント、ステップ イン／アウト／オーバー、変数インスペクター、毎秒 1〜10 行のアニメーション再生 |
| エージェント型 AI アシスタントのメッシュ | ✅ | **rig-core** の LLM オーケストレーター（Ollama、OpenAI、Groq、Alibaba Model Studio、その他のクラウド API）が Dev Agent、Editor Assistant、History Compactor を動かし、ライブの可観測性ログと `↑input ↓output` のトークン表示を備えます |
| オーケストレーターの Grace | ✅ | 依頼を分解し、各タスクをそれを担当する専門エージェントへ振り分け、1 対 1 の **Pedantic レビュアー**を課します — どの専門エージェントも自分の成果を自分で承認できません |
| RAG 付きのチャンク化ナレッジベース | ✅ | 主題ごとに 1 レコードで索引化。埋め込み済みで出荷され、GPU と発熱の少ない CPU フォールバックを備え、**File → Reindex Knowledge Bases** があります |
| フォームのライフサイクルとウィンドウ | ✅ | 指定された**メインフォーム**がアプリケーションを起動します。フォームごとの外装と状態が尊重され、`OpenFormSync`/`OpenFormAsync` があり、ウィンドウ位置は設計時のプロパティで、プロジェクトごとの入場・退場エフェクトがあります |
| 複数ウィンドウでの実行 | ✅ | プレビュー画面と実行画面を、OS のビューポートに分けて表示（egui のマルチビューポート） |
| 国際化された UI | ✅ | 6 つのインターフェース言語: 英語、スペイン語、ポルトガル語、日本語、中国語、フランス語 |
| システムフォントの選択 | ✅ | インストール済みのどのフォントも、その書体自身で描いて一覧でき、デザイナー、プレビュー、実行中のフォームにその場で適用されます |
| ブロックしないネイティブのファイルダイアログ | ✅ | UI のイベントループを止めずに、開く・保存・参照ができます |

### 10.2 コンパイラー

| 機能 | 状態 | 備考 |
|---|:--:|---|
| 単一のネイティブバイナリを出力 | ✅ | AST を `bincode` + `flate2` で直列化し、すべてのフォームともども `include_bytes!` で埋め込み、`cargo build --release` でビルドして `bin/` にバイナリを 1 つ出します。**`.cbl` のソースは一切含まれません** |
| 再配布のための告知 | ✅ | `bin/` には `LICENSE`、`NOTICE`、ランタイムの告知が自動的に置かれるので、配布物は Apache-2.0 が求める告知を備えます |
| ビルド失敗時には `rustc` 本物の診断 | ✅ | ビルドが失敗したときは、要約の 1 行ではなくコンパイラー自身の診断を報告します |

.<<

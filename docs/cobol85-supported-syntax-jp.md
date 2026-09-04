<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL‑85 サポート済み構文リファレンス

**この文書の目的:** COBOL‑85 標準のうち RustCOBOL が実際にどこまで実装している
のかを述べること — そしてそれを主張するのではなく、**NIST 公式の COBOL‑85 検証
スイート**に対して証明することです。下の
[スコアボード](#-適合性は主張するものではなく測定するもの--nist-ccvs85)が結論であり、
その後に続くものはすべて、その数字の裏側にある詳細です。

**RustCOBOL の lexer/parser/runtime が今日実際に受け付けるもののグラウンド
トゥルース**であり、ソース (`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`)
から導き、`NIST/newcob.val,cbl` と突き合わせて確認しています。
テストは ✅ の形式に対して書いてください。❌ の形式は構文解析に失敗するか
no‑op であり、⚠️ の形式は解析はされるものの挙動は部分的です。本書は
[`cobol85-verb-test-matrix-jp.md`](cobol85-verb-test-matrix-jp.md) の姉妹編です。
マトリクスは*何を*テストするかを示し、本書は*どの書き方を RustCOBOL が理解する
のか*を示します。

凡例: ✅ サポート済み · ⚠️ 解析はされるが部分的/簡略化 · ❌ 認識されない
(避けるか、欠落を確認するためだけにテストする)。

---

## ★ 適合性は主張するものではなく測定するもの — NIST CCVS85

**これが本書の要点です。** 以下のすべての主張は、**NIST 公式の COBOL‑85 検証
スイート** — CCVS85 バージョン 4.0 (01 OCT 1992、COBOL 85 バージョン 4.2、
Apr 1993 SSVG)、米国 National Institute of Standards and Technology が COBOL
コンパイラを認証するために用いていたスイート — に対して検証されています。
28 MB、348,271 行、**459 本の COBOL プログラム**と 51 個の copybook メンバーから
なり、本リポジトリの `NIST/newcob.val,cbl` に置かれています。

これが真実の源です。RustCOBOL と CCVS85 が食い違う場合は、**CCVS85 が正しく
RustCOBOL が誤っている**とみなし、その差分は
[`specs/nist/`](../specs/nist/README.md) に欠陥として記録されます — 修正ごとに
仕様が 1 本、失敗するプログラムを名指しで挙げたうえで。

### スコアボード

2026‑08‑28 にバージョン 1.62.43 で、手を加えていない配布物に対して測定:

| | プログラム数 | 割合 | 意味 |
|---|---:|---:|---|
| ✅ **PASS** | **422** | **97.2 %** | 対象範囲の 434 本のうち |
| ❌ **FAIL** | **12** | 2.8 % | 対象範囲の 434 本のうち |
| ⬜ **N/A** | **25** | — | RustCOBOL の対象範囲外のモジュール (後述) |
| | **459** | | スイート全体のプログラム数 |

再現方法:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ コンパイルできることは弱いほうの主張

上の表が数えているのは、**フロントエンドが受け付ける**プログラムです。実行できる
とは言っていません。スイートは自分自身を採点します — CCVS85 の各プログラムは
自分の `PASS` / `FAIL*` レポートを印字します — したがって、より厳密に強い第二の
数字があります。すなわち、何本が最後まで走りきり、**失敗ゼロ**を報告するか、です。

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

両方の数字はモジュールごとに報告され、決して混同されません:

| モジュール | コンパイル | 実行 (失敗 0 件) |
|---|---:|---:|
| **NC (Nucleus、核)** | **95 / 95** | **83 / 95** |

作業は**一度に 1 モジュールずつ**進みます。NC は両方の数字が 95 に達して初めて
完了であり、そうなるまで他のモジュールには着手しません。10 モジュールにまたがる
広く浅いコンパイル成績は、そのどれかが動くかどうかについて何も語りません。

##### 印字ファイル以上のものを必要とする NC の 5 メンバー — すべて採点済み

実行スコアは、プログラム**自身の CCVS レポート**が失敗を示していないときに、その
プログラムをクリーンと数えます。NC の 5 メンバーはそのようなレポートを印字しま
せんが、それは何かが壊れているからではありません。いずれもコンパイラ側ではなく
テストハーネス側の作業を必要としたもので、いまはすべて採点されています:

| メンバー | 何を必要とするか | どう採点されるか |
|---|---|---|
| **NC302M**、**NC303M**、**NC401M** | *フラグ付け (flagging)* のテスト。`PASS`/`FAIL` の仕組みをまったく持たず — それぞれ `TOTAL NUMBER OF FLAGS EXPECTED = n` で終わり、検証される結果は、廃止された構文 (NC302M/NC303M) や高位サブセットを超える構文 (NC401M) に対して**コンパイラが出す診断**の集合です。 | ハーネスは診断を、そのメンバー自身の期待リストと行単位で比較します。2 つの分類は**別々のパス**として実行されます: `DATE-COMPILED` は廃止済みで*かつ*高位サブセットを超えているため、1 回にまとめたパスでは各メンバーにもう一方のフラグが偽陽性として入ってしまいます。 |
| **NC110M** | レポートを `DISPLAY` で、ハーネスが読む CCVS の印字ファイルではなく、オペレーターのコンソールへ書き出します。 | 子プロセスのコンソール出力をファイルに捕捉し、そこから採点します。 |
| **NC109M**、**NC204M** | オペレーターから読み取る Format 1 の `ACCEPT` をテストします — NC109M はそれを素のまま書き、NC204M は `SPECIAL-NAMES` が入力装置に結び付けたニーモニック経由で書きます。入力は検証者が与える前提であり、stdin がなければ比較はすべて失敗します。 | ハーネスが子プロセスの stdin にオペレーター用のデックを与えます。デックは**ソースから復元したものであり、でっち上げではありません**: 受け取られる各項目は、プログラムが `ACCEPT` の直上で値を設定する対の項目と比較されるため、デックの各行はその値そのものです。 |

したがって実行軸には**95 を下回る構造的な上限は存在しません**。対象範囲の NC
プログラムはすべてコンパイルでき、そのすべてが自分自身の報告に基づいて採点され
ます。

**すでに**決着した比較対象のケースは外部スイッチです。NC174A、NC253A、NC254A は、
実行前にオペレーターが設定するスイッチに対して `ON STATUS` / `OFF STATUS` を
テストします — COBOL の内側からはスイッチを設定できません — そこでハーネスは
いま `--switch XXXXX051=ON --switch XXXXX052=OFF` (および置き換えられた
`SWITCH-1` / `SWITCH-2` の綴り) を、CCVS85 の実行手順が求めるとおりに渡します。
これは検証手順が要求する設定であって、天秤に指を添える行為ではありません:
スイッチを宣言していないプログラムは何の影響も受けません。

#### ⚠️ PASS が実際に意味すること — この数字を引用する前に読むこと

プログラムが **PASS** と数えられるのは、`--source-format=fixed` を使って
RustCOBOL のフロントエンド — lexer、parser、意味解析器 — を**エラーゼロ**で
通過したときです。

それは*コンパイル*の適合性です。プログラムが正しい答えを計算することの証明では
**ありません**。CCVS85 のプログラムは実行時に自分自身の `PASS`/`FAIL` 集計も
印字し、その出力を採点することがこの作業の**次の段階**です — それは 332 には
含まれていません — 下の実行スコアボードを参照してください。この区別が重要である
理由は、測定された 2 つのケースが示しています:

- RELATIVE ファイルを使う 35 本のうち 30 本はきれいにコンパイルできますが、
  runtime には **RELATIVE エンジンがまったくありません** — 実行すれば、黙って
  誤った結果を出すことになります。
- 2 行にまたがって継続された literal は誤って再結合されてもなお構文解析が通る
  ことがあり、プログラムは誤ったデータを抱えたままになります。

つまり: **PASS = 「RustCOBOL はこのプログラム中のすべての構文を受け付ける」**。
いまのところ、それ以上ではありません。

#### 🔴 実行スコアボード — 「動く」を意味する数字

ここまではすべて**コンパイル**を測っています。CCVS85 のプログラムは*実行*もされ、
自分自身の `PASS`/`FAIL` 集計を印字します。その集計こそ、このスイートが存在する
目的です。1.62.15 以降、ハーネスはそれらを実行します:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

2026‑08‑28 に 1.62.43 で測定。ゴールデンルール #9 のもと、次のモジュールに移る前に
1 つのモジュールを終わらせます: **NC (Nucleus、核) は両方の軸で完了**しており、
したがって **SQ (順次入出力)** が進行中のモジュールです。

**NC — Nucleus (核)**

| | プログラム数 |
|---|---:|
| 対象範囲 | 95 |
| コンパイルできなかった | 0 |
| 最後まで実行された | 95 |
| **…うち失敗 0 件を報告** | **95** |
| …失敗を報告 | 0 |
| 実行はされたがレポートを印字しなかった | 0 |
| タイムアウト (>20 s) | 0 |
| クラッシュしたか runtime に拒否された | 0 |

プログラム自身が報告するアサーション: **4 614 PASS / 0 FAIL**、採点対象 4 614 の
100 %。(さらに 5 件は `DELETED` — プログラム自身がスキップするテストに対する
CCVS 独自のマーカーです。)

対比として、1.62.23 における同じ表は 95 本中 65 本がクリーン、4 278 PASS /
226 FAIL でした。埋まったのは「コンパイルできる」と「動く」の隔たりです。

**SQ — 順次入出力 (進行中)**

| | プログラム数 |
|---|---:|
| 対象範囲 | 85 |
| コンパイルできなかった | 0 |
| 最後まで実行された | 83 |
| **…うち失敗 0 件を報告** | **84** |
| …失敗を報告 | 1 |
| 実行はされたがレポートを印字しなかった | 0 |
| タイムアウト (>20 s) | 0 |
| 出力の暴走 (>2 MB) | 0 |
| クラッシュしたか runtime に拒否された | 0 |

アサーション: **623 PASS / 1 FAIL**、採点対象 624 の 99.8 %、そして**すべての
プログラムが最後まで走りきります**。1.62.42 では同じ表が 85 本中 **10** 本
クリーン、20 本クラッシュ、1 本タイムアウト、215 PASS / 190 FAIL でした — この
クラッシュの塊は 1 つの欠陥、宣言部の段落が名前を失うというものでした。1.62.43
では 44 本クリーン、471 PASS / 162 FAIL でした。可変長レコード、共有レコード領域、
`FILLER` の幅、`READ … INTO`、順次の `REWRITE` は 1.62.44 で入りました。モードで
限定した `USE`、`CLOSE REEL/UNIT`、`SELECT OPTIONAL`、`OPEN` 時の
`LINAGE-COUNTER`、範囲外のレコード長は 1.62.45 で。データ名で与える `LINAGE` の値
と順次入出力のフラグ検出器は 1.62.46 で入りました。

1 メンバーがまだ足りていません:

| メンバー | 何が残っているか |
|---|---|
| SQ203A | CCVS85 の**インストール**が用意するデータファイル `XXXXD001` を必要とします。スイートのどのメンバーもそれを書き出さないため、その `SELECT OPTIONAL` テストのうち「ファイルあり」の半分はここでは実行できません。「ファイルなし」の半分は通ります。これは不足しているインストール入力であって、RustCOBOL の欠陥ではありません。 |

> `FAIL*` の明細行は意図的に**2 回**書かれます — CCVS の `PRINT-DETAIL` が
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` を実行するためです — 一方
> `PASS ` は 1 回だけ書かれます。印字ファイルから素朴にマーカーを数えた値は、
> 失敗を半分にしてからでないと何の意味も持ちません。

プログラムが*なぜ*失敗するのかを読むには、3 つめのパスが、そのプログラム自身の
レポートが持つ失敗の明細を、モジュール全体で分類できる形で印字します:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> だからこそコンパイルの数字は常に「RustCOBOL はこれらの構文を**受け付ける**」
> として報告されます。これを適合レベルとして引用するのは誤りです。

#### モジュール別

| モジュール | 何をテストするか | PASS / 合計 | |
|---|---|---:|---|
| NC | Nucleus (核) | **95 / 95** | ✅ 完了 — しかも**実行**でも完了 (上のスコアボードを参照) |
| SQ | 順次入出力 | **85 / 85** | ✅ コンパイルは完了。**実行は 44 / 85** — 進行中のモジュール |
| IC | プログラム間通信 | 45 / 47 | `END-CALL` が自分の `CALL` に消費されず文ディスパッチャまで届く。添字付きの条件名が 1 件 |
| IF | 組み込み関数 | **45 / 45** | ✅ 完了 |
| IX | 索引入出力 | **42 / 42** | ✅ 完了 |
| SG | セグメンテーション | **13 / 13** | ✅ 完了 |
| ST | ソート / マージ | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | 相対入出力 | 34 / 35 | ⚠️ **コンパイルのみ — 実行エンジンなし。** `ORGANIZATION IS RELATIVE` は構文解析されるだけで実行時には一度も対応されないため、この行は実際の能力を過大に見せています。唯一の失敗は宙に浮いた `ELSE` |
| SM | 原始テキスト操作 (COPY/REPLACE) | 14 / 17 | データ名の中の `$`。修飾付き/添字付きの疑似テキスト。`PERFORM … VARYING` の一形式 |
| DB | デバッグ | 11 / 15 | `GO-TO` がユーザー定義語として使われ、キーワード対 `GO TO` と衝突する。1 本は通信の動詞 `DISABLE` を使う |
| **対象範囲** | | **422 / 434** | |
| CM | 通信 | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | 廃止機能のフラグ付け | — | ⬜ N/A |
| EXEC85 | NIST 自身の COBOL 制御プログラム | — | ⬜ N/A |

### ⬜ N/A — RustCOBOL の対象範囲外にあるものと、その理由

これら 25 本は**失敗として数えません**。RustCOBOL が実装しておらず、実装する予定
もない機能です。詳しい理由は
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md)
にあります。

| モジュール | プログラム数 | なぜ対象範囲外か |
|---|---:|---|
| **CM** — 通信 | 9 | `COMMUNICATION SECTION`、`CD` 項目、`SEND` / `RECEIVE` / `ENABLE` / `DISABLE`。1980 年代のテレプロセシング・モニター — トランザクション管理システムが所有するメッセージキュー — を対象としています。ここにはそのような runtime は存在せず、このモジュールは後の COBOL 標準から削除されました。 |
| **RW** — Report Writer | 6 | `REPORT SECTION`、`RD` 項目、`INITIATE` / `GENERATE` / `TERMINATE`、コントロールブレイク。大規模な宣言的サブ言語です。帳票に対する PowerRustCOBOL の答えはフォームデザイナーと PDF 書き出しです。望むなら後日*機能*になり得ます — 実ユーザー価値がある唯一の除外項目です。 |
| **OBSQ / OBIC / OBNC** | 9 | これらは先行モジュールを再テストし、COBOL‑85 の廃止要素をコンパイラが*フラグ付け*することを期待します。言語としての内容は対象範囲の仕様でカバーされています。対象範囲外なのは廃止機能の**フラグ付け**のほうです。 |
| **EXEC85** | 1 | テストではありません。配布物を分割してスイートを駆動する NIST 自身の COBOL エグゼクティブであり、ここでは Rust のハーネスに置き換えられているため、コンパイルできる必要がありません。 |

**オブジェクト指向 COBOL** も RustCOBOL の対象範囲外ですが、CCVS85 はそれより
完全に前の時代のものです — スイートに OO のプログラムは存在しません。

### 残る 192 件の失敗はどこから来るのか

いずれも仕様化された欠陥であって、未知のものではありません。それが*最初の*
エラーとなるプログラムの本数で順位付けしています:

| プログラム数 | 根本原因 | 仕様 |
|---:|---|---|
| 31 | 区切りのコンマ — `MOVE ZERO TO A, B, C` | [区切り文字](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [組み込み関数](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [文の欠落](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | 空白で区切られた添字 — `TBL (1  2)` | [区切り文字](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON` (スイッチ名) と `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md)、[区切り文字](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [文の欠落](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | B 領域の奥に置かれた、あるいは複数行に分かれた `COPY` | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | 区切りのセミコロン — `START F ; INVALID KEY` | [区切り文字](../specs/nist/NIST-spec-separators.md) |
| 4 | 次の行に置かれた `OCCURS` の整数 | [区切り文字](../specs/nist/NIST-spec-separators.md) |
| 4 | 優先番号付きの `SECTION` — `SORT-PARA SECTION 69.` | [セグメンテーション](../specs/nist/NIST-spec-segmentation.md) |

> **順位は修正のたびに動き、その動き自体が情報になります。** 以前のリリースで
> この表の先頭にあった 3 行は消えました — IDENTIFICATION のコメント項目、数値
> literal、そしてはぐれた引用符です。そのたびに、消えた行にいたプログラムの多くは
> 合格に**なりませんでした**。1 つ下の行へ移っただけです。1.62.12 で解放された
> SG の 4 本は、いまは `SORT-PARA SECTION 69.` で止まります。セグメンテーションが
> 依然として 0 / 13 と表示される理由はそこにあります。以前の順位を信用せず、
> 測り直してください。

### 適合性の履歴

| バージョン | PASS / 434 | 何が変わったか |
|---|---:|---|
| 1.62.7 | **0** | 何もコンパイルできませんでした。古典的な参照形式の規則が 2 つ欠けていたためです: 73‑80 桁がソースとして読まれ、継続行が一度も連結されませんでした。 |
| 1.62.8 | **222** | `--source-format=fixed` — 継続を含む、古典的な参照形式。[ソース形式](#ソース形式) を参照。 |
| 1.62.10 | **237** | 数値 literal を小数点で始めてよい (`.999`)。組み込み関数 21 → 29、Nucleus 25 → 29、ソート/マージ 27 → 30。 |
| 1.62.11 | 241 | IDENTIFICATION のコメント項目段落。デバッグ 5 → 9。32 本という分類の大きさが示唆するほどの伸びではありません: そのうち 9 本は通信のプログラム (N/A) で、残りの大半は直後に 2 つめの障害にぶつかりました。 |
| 1.62.12 | 242 | literal はその行に閉じ込められるため、はぐれた引用符 1 個がファイル全体のパリティをずらすことはもうありません。Nucleus 29 → 30。6 本の分類は解消しました: 4 本はセグメント優先番号へ進み、1 本はいま合格します。 |
| 1.62.13 | 292 | 区切りのコンマとセミコロンはトークンではなく句読点である。添字は空白だけで区切ってよい。添字は完全な修飾名の後に続いてよい。literal 内で二重にした区切り記号は 1 文字である。Nucleus 30 → 56、プログラム間 32 → 44、索引 31 → 38。診断の分類が丸ごと 3 つ空になりました。 |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — 表全体を組み込み関数の引数にする。`MOVE ALL "X"` が項目を埋める。`CLOSE … WITH LOCK` / `NO REWIND` / `REEL`。符号付き literal を `WHEN` の対象にする。`PERFORM … TIMES` の回数をデータ項目で与える。整数の回数を継続行に書く。**組み込み関数 45 / 45 — モジュール完了。** |
| 1.62.15 | 332 | 未知の `FUNCTION` 名は 0 を返すのではなくコンパイルエラーになる。ユーザー定義語は数字で始めてよい (`25COUNT`、`3-DEM-TBL`、`0 SECTION.`)。`D` 行は `WITH DEBUGGING MODE` がない限りコメントである。セグメンテーション 0 → 10、Nucleus 58 → 61。 |
| 1.62.16 | 376 | `AT END` の `AT` は省略可能なので、素の `END` 句が次の段落見出しを飲み込むことはもうありません (33 本)。COPY/REPLACE のプリプロセッサが literal をその行に閉じ込めるため、著作権表示の中の COPY という語は指令ではありません。数値 literal は小数点から `ADD`/`SUBTRACT` のオペランド並びを開始できます。**索引入出力が完了、42 / 42。** |
| 1.62.17 | 380 | `LINAGE` のページレイアウト、`LINAGE-COUNTER`、`WRITE … AT END-OF-PAGE` / `AT EOP` — スタブではなく実装。順次入出力 77 → 81。 |
| **1.62.19** | **396** | numeric-edited の項目は数字項目である。編集用の小数点はその後ろの桁を保持し (`PIC ZZ,ZZZ.9` はもう `ZZ,ZZZ` に切り詰められません)、編集文字だけで組んだ picture — `ZZZZ`、`$.**`、`$**.**CR` — は英数字ではなく numeric-edited である。どちらも、正当な算術の `GIVING` 受け手を非数字に見せていました。 |
| **1.62.18** | **391** | 継続行の先頭に来る数値は、式が期待される位置ではオペランドである。クラス条件や符号条件の `IS` は省略可能であり、条件は `EVALUATE` の主語になり得る。手続き名は参照でも見出しでも、すべて数字で書いてよい。 |
| **1.62.21** | **417** | Nucleus のためのパス。`ALTER` は並びであり `GO TO.` が変更対象の GO TO である。すべて数字の手続き名は先頭のゼロを保つ。条件名は添字付きにも修飾付きにもできる。括弧付きの算術式は入れ子の条件ではなくオペランドである。`MULTIPLY`/`DIVIDE` の形式 1 は受け手の並びを取る。`WITH TEST` は `VARYING` の前に置けて、繰り返し回数は添字付きにできる。`PERFORM 命令文 … END-PERFORM` はどの句も必要としない。段落名はその節で修飾できる。`ELSE` が `ON SIZE ERROR` の命令文にも入れ子の ELSE 分岐にも飲み込まれない。省略した組み合わせ関係は算術やクラス/符号の対象を受け付ける。`INSPECT` は ALL/LEADING の分類をオペランド間で引き継ぎ、`CONVERTING` は領域を取る。`UNSTRING TALLYING` は `WITH POINTER` の後に続く。**Nucleus はコンパイル 95 本中 76 → 92、クリーン実行 16 → 28。** |
| **1.62.43** | **422** | **順次入出力モジュールが完全にコンパイルできるようになり — 85 本中 85 本 — 実行は 85 本中 10 → 44 になりました。** 宣言部の段落が名前を保つため、`USE` ハンドラーからそれらを `PERFORM` し `GO TO` できます (20 本がクラッシュしなくなりました)。2 文字の*集団*項目として宣言された `FILE STATUS` 項目がコードを受け取ります。すでに開いているファイルの `OPEN` は `41` であり、開き直しません。`AT END` の後の順次 `READ` は `46` です。そして 1 つの `OPEN` が複数のモード群を持てます (`OPEN INPUT f1 OUTPUT f2`) — コンパイル面の伸びはすべてこれによるものです。 |
| **1.62.42** | **420** | **Nucleus モジュールが完了しました — 95 本中 95 本がコンパイルでき、*かつ* 95 本中 95 本がクリーンに実行され、4 614 件のアサーションが 1 件も失敗しません。** `66 RENAMES` はそのレコードで修飾され、またいだ表のすべての出現を覆い、ちょうど 1 項目を改名するときはその項目そのものになる。集団項目に宣言した 88 はその集団のバイト列を検査する。figurative 定数はもう一方のオペランドに合わせて寸法が決まり、`VALUE` も含む。集団オペランドの分類は英数字である。省略形の対象の前に置いた `NOT` は関係を否定する。`INSPECT … REPLACING` の並びは 1 回の走査を共有し、符号付き DISPLAY 項目の文字の中に `-` は含まれない。`REDEFINES` の重ね合わせは入れ子になる。そして `PERFORM … WITH TEST AFTER VARYING` が尊重され、`AFTER` の変数はそのループが終わるときにリセットされ、添字付きの `VARYING` 識別子はその添字に従う。NC201A がそもそも完走できたのは、この最後の一群のおかげです。 |

> **正直な要約。** RustCOBOL は今日、対象範囲の NIST スイートの **97.2 %** を
> 受け付けます。9 リリース前はゼロでした。残る 12 本は謎ではありません — いずれも
> 名前の付いた欠陥であり、それぞれがどのプログラムを阻んでいるかまで仕様化されて
> います。この表が進捗の物差しであり、リリースのたびに更新されます。
>
> **そして 1 つのモジュールが、意味のある軸で完了しています。** Nucleus は 95 本中
> 95 本をクリーンに実行します。単にコンパイルするだけではありません — 上の実行
> スコアボードを参照してください。ゴールデンルール #9 のもと、それが次のモジュール
> に着手するための関門なので、**順次入出力がいま進行中**です: コンパイルは完了、
> 実行は 85 本中 44 本。

---

> **更新 (欠落実装パス):** 次のものが実装され、いまは ✅ です — **部分参照**
> `id(start:len)`、**インラインの `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING の `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、
> **分類を意識した `INITIALIZE`**、**演算子を前置した省略条件** (`a > 1 AND < 9`)、
> **`CALL … ON EXCEPTION`** (解決できない CALL で動く)、**`COMPUTE` の複数受け手 +
> 受け手ごとの `ROUNDED`**、そして大幅に拡張された**組み込み関数**の集合。
>
> **更新 (階層的 / 出現を意識した環境パス — 1.5.0):** データモデルに阻まれていた
> 4 つの機能がいまは ✅ です — **実行時の表添字** `t(i)` / `t(i, j)` (出現ごとの
> 記憶域)、**修飾名による曖昧性解消** `id OF/IN group` (重複する末端名が独立した
> 記憶域に解決される)、**`MOVE/ADD/SUBTRACT CORRESPONDING`**、そして**機能する
> `SEARCH` / `SEARCH ALL`**。
>
> **更新 (動詞の網羅パス — 1.6.0):** さらに ✅ になったもの — `ADD`/`SUBTRACT` に
> おける**複数受け手の `MULTIPLY`/`DIVIDE GIVING` + 受け手ごとの `ROUNDED`**、
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** と修正された素の
> `EXIT`、**`CALL … NOT ON EXCEPTION`**、組み合わせた
> **`INSPECT … TALLYING … REPLACING`** と **`BEFORE/AFTER INITIAL`** の領域、
> 日付/財務の**組み込み関数** (`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、
> `INTEGER-OF-DAY`、`DAY-OF-INTEGER`、`ANNUITY`、`FRACTION-PART`)、**literal を
> 対象にした省略条件** (`A = 1 OR 2 OR 3`)、**`EVALUATE … ALSO`** (複数主語) と
> **`WHEN NOT`**、**本物の 88 レベル条件名** (`SET … TO TRUE/FALSE`、ホストは
> その VALUE や範囲に対して検査される)、**`PERFORM para VARYING`**、そして機能する
> **`SORT`/`MERGE`** の runtime (`RELEASE`/`RETURN`、`USING`/`GIVING`、
> `INPUT`/`OUTPUT PROCEDURE`)。末尾の「避けるべきもの」の一覧は最新です。
>
> **更新 (「避けるべきもの」一覧の解消パス — 1.7.0):** 残っていた欠落が実装され
> ました — **識別子を対象にした省略形** (`a = b OR c`、88 レベルのメタデータで
> 解決)、**`INITIALIZE … REPLACING category DATA BY value`**、**`66 RENAMES`**
> (読み取りは合成し、書き込みは覆われた項目に分配)、**ポインター**
> (`USAGE POINTER`、`SET ptr TO ADDRESS OF x / NULL`、
> `SET ADDRESS OF item TO …` による別名付け、`IF ptr = NULL`)、**`ALTER`** /
> **`UNLOCK`**、忠実な **`NEXT SENTENCE`**、残っていた標準の**組み込み関数**
> (`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、`NUMVAL-F`、`TEST-NUMVAL`)、
> そして拡張された**画面 `ACCEPT`/`DISPLAY`** (CLI モードでは ANSI 経由の
> `AT`/`WITH` — いまは構文解析されるだけでなく*実行*されます)。
>
> **更新 (1.7.1):** `ACCEPT` のレジスター入力元がいまは機能します (以前は認識される
> だけの no‑op でした) — **`FROM COMMAND-LINE`**、**`ARGUMENT-NUMBER`** /
> **`ARGUMENT-VALUE`** (`DISPLAY n UPON ARGUMENT-NUMBER` と対になる)、
> **`ENVIRONMENT-VALUE`** (`DISPLAY "name" UPON ENVIRONMENT-NAME` と対になる)、
> **`ESCAPE KEY`** → `"00"`、**`CRT STATUS`** → `"0000"`。
>
> **更新 (1.7.2):** ファイル共有 / ロックの句と `CANCEL` (以前は ❌ / no‑op) —
> **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`** (そのファイルの INDEXED レコードロックを解放)、そして
> **`CANCEL program`** (プログラムの記憶域を初期化し直す)。
>
> **更新 (1.8.0):** **`COMMIT` / `ROLLBACK`** が本物の COBOL の動詞になりました —
> 開いている INDEXED ファイルに対する、プログラム制御のトランザクションです
> (メモリーエンジンとディスクエンジンの両方)。ディスクエンジンは実行中の本物の
> やり直しログを備えました (以前は no‑op でした)。末尾の「避けるべきもの」の一覧は
> 最新です。

---

## IDENTIFICATION DIVISION の段落

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ **コメント記入項目**の段落 — `AUTHOR`、`INSTALLATION`、`DATE‑WRITTEN`、
  `DATE‑COMPILED`、`SECURITY` — を**任意の順序で、任意の部分集合だけ**。
- ✅ `REMARKS` も受け付けます。1985 年に COBOL から削除されたので保存はしません。
  COBOL‑74 から引き継いだソースが今でもコンパイルできるように受理するだけです。

**コメント記入項目**は自由記述のテキストであり、COBOL‑85 はそれを文字どおりの
意味で定めています:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- **予約語**を含んでもかまいません — 上の `DATA` は DATA DIVISION を開始しません。
- **ピリオド**を含んでもかまいませんし、ピリオドで終わりにもなりません。
- 書いた**行数だけ何行にもわたります**。
- A 領域で**行の先頭から始まる**次の段落見出しまたは部見出しで終わります — 上の
  記入項目が `DATE-WRITTEN` で終わっているのはそのためです。

**その文章中の引用符はその行の中に閉じ込められます** (1.62.12 以降)。
`THE COMPILER"S ABILITY` のようなテキストは、プログラムの残り全体まで走るリテラルを
もはや開きません — [ソース形式](#ソース形式)を参照してください。コメント記入
項目の中で対にならない引用符は避けるに越したことはありませんが、いまや代償はその
1 行だけで、ファイル全体ではありません。

⚠️ ここでは `INSTALLATION`、`SECURITY`、`REMARKS` は**予約語ではありません**。
これらは IDENTIFICATION DIVISION の中でのみ段落名として認識されるので、
`SECURITY` という名前のデータ項目はそのまま使えます。

---

## ソース形式

RustCOBOL は 3 つのソース配置を読みます。どれを使うかは明示的に指定します —
ファイルの中身から推測することは**決してありません**。桁位置の規則を、それを前提に
書かれていないソースへ適用すると、コードが黙って消えてしまうからです。

| `--source-format` | 意味 |
|---|---|
| `free` | 桁位置の規則はいっさいありません。`*>` がコメントを開始します。**既定値**であり、PowerRustCOBOL 自身のプロジェクトと生成されたフォームの `.cbl` ファイルが使う形式です。 |
| `fixed` | ✅ **古典的な COBOL-85 参照形式** — 規格が定める配置で、カードイメージのソースはこれで書かれています。下記を参照してください。 |
| `fixed-relaxed` | 一連番号領域と標識桁は尊重しますが、行は入力したところまで続きます — 72 桁の制限はありません。 |
| `auto` | 歴史的な挙動: `COBOLT_FIXED=1` でない限り `free`。 |

`COBOLT_SOURCE_FORMAT` はセッションの既定値を設定します。

### `fixed` — 古典的な参照形式

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **1-6 桁** — 一連番号領域。無視されます。
- **7 桁目** — 標識領域:
  - `*` または `/` → コメント行
  - `-` → 直前の行の**継続**
  - `D` → デバッグ行。コメント扱いです (デバッグモードは未実装)
  - それ以外 → 通常のソースとして読みます。規格はこの桁を予約していますが、
    カードイメージのテストスイートは省略可能な行の選択子として使っており、
    その行を黙って捨てるとコードを消すことになります。
- **8-72 桁** — ソース本体。
- **73-80 桁** — 識別領域。**破棄されます**。

### 継続行 ✅

7 桁目のハイフンは直前の行を継続します。

**語または数字定数の継続** — 継続される側の行の末尾の空白は捨てられ、両半分は
間に何も挟まずにつながります:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

これは `WRK-DS-18V00-CONTINUED` という名前の項目を 1 つ宣言します。

**英数字リテラルの継続** — 継続される側の行のリテラルには閉じ引用符がありません。
継続行は引用符で開き直さなければならず、リテラルはその次の文字から再開します:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **継続された断片は末尾の空白も含めて 72 桁まで伸びます。** 72 桁より手前で終わる
行でも、その空白はリテラルに寄与します。継続リテラルがバイト単位で正確なのが
`fixed` のときだけなのはこのためです。ほかの形式には、止まるべき 72 桁目が
ありません。

### リテラルが偶然に行をまたぐことはない ✅

リテラルが複数行にまたがる方法は継続**だけ**です。自分の行で閉じられていない
引用符はエラーであり、書かれた場所で報告されます:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

これは聞こえる以上に重要です。1.62.12 より前は、対にならない引用符がファイル内の
どこであれ*次の*引用符まで走っていたため、コメント中のたった 1 個の迷子の `"` が
部 (DIVISION) をまるごと飲み込み、それ以降のすべての引用符の対応をずらしていました
— これが見つかった NIST のプログラムは引用符の個数が**偶数**なので、未終了のものは
何もなく、たった 1 文字がファイル全体のパリティをずらしていたのです。被害はいまや
改行で止まります。

> **自由形式にはリテラルの継続がありません。** `&` でもなく — これは連結
> *演算子*です — 囲みブロックでもありません。自由形式のリテラルは 1 行に収まら
> なければなりません。長いものは連結してください:
> `"first part" & "second part"`。

> **注意。** 自由形式で書かれたファイルに `fixed` を選ぶと、そのファイルは壊れます
> — 72 桁より後ろはすべて消え、8 桁目より前のテキストは一連番号として読まれます。
> 本当にカードイメージであるソースにだけ指定してください。

---

## 認識される文 (動詞)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (para-1 の `GO TO` を付け替えます) ·
`UNLOCK file` (そのファイルのレコードロックを解放します) ·
`OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (ファイルの共有/ロック — 単一の実行単位の中では
勧告的です)
✅ `COMMIT` / `ROLLBACK` (プログラムが制御する INDEXED ファイルのトランザクション
— ファイル動詞の節を参照) · `CANCEL` (プログラムの記憶域を初期化し直します) ·
⚠️ `INVOKE` (解析はされますが何もしません)
プロジェクト拡張: `EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。
ブロックからは、常にリンクされる crate (std、egui、eframe とリンク済みランタイム
一式) に**加えて、プロジェクトが Project's Crates に登録した任意の crate** を
`use` できます (仕様 044)。登録された crate は厳密なバージョンに固定され、
プロジェクトの `crates/` にベンダリングされ、バイナリへコンパイルされます。
未登録の crate は開発者の行で Check/Build を失敗させ、対処方法が示されます。

✅ `SEARCH` (逐次) / `SEARCH ALL` (`ASCENDING`/`DESCENDING KEY` を持つ表に対する
二分探索 — 最初に一致した `WHEN` を実行し、なければ `AT END`)。
✅ `RELEASE` / `RETURN` を伴う `SORT` / `MERGE` (動作します — 下記参照)。
✅ `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` を伴う `DECLARATIVES … END DECLARATIVES`
— 処理されなかったエラーの `FILE STATUS` で起動されるファイルエラーハンドラです。
ハンドラは**その節の先頭から入り、節の終わりまで実行されます**。段落名はそのまま
残るので、ハンドラはそれらに `PERFORM` や `GO TO` ができます — *別の*宣言節の段落に
対してもです。宣言段落は独自の名前空間にあります: 制御が本体から宣言段落へ流れ落ちる
ことは決してなく、両方で宣言された名前は、ハンドラの実行中は宣言側のものへ、それ以外
の場所では本体側のものへ解決されます。宣言部から非宣言部の段落を `PERFORM` すること
もできます。
❌ **認識されません — 使わないでください:** `ENTRY`、
`GENERATE`/`INITIATE`/`TERMINATE`、`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 動詞ごとのサポート形式

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (複数の受け手)。
- ✅ **一方のオペランドが集団項目なら、転記全体が英数字転記になります** (COBOL-85 6.18.4)。
  もう一方のオペランドの PICTURE は*サイズ*だけを与え、それ以外は何も与えません。編集も、
  編集解除も、数値変換も行われません。`MOVE <group holding "123ABC">` は
  `PIC 0XXXXX0` に `"123ABC "` を残し (編集後の `"0123AB0"` ではありません)、
  `PIC 9999V999` にも同じ 6 文字と 1 個のスペースを残し、`PIC 99` には `"12"` を残します。
  どちらの端が詰められ、どちらの端が失われるかは、依然として `JUSTIFIED RIGHT` が決めます。
  同じ規則は集団項目自身のバイト列にも及びます。各子項目は自分の取り分をそのまま受け取るので、
  英数字編集の子項目が再び編集されることは**ありません**。
- ✅ **集団項目に付けた `VALUE` 句**は集団項目のバイト列を初期化し、その子項目へ
  分配されます — `01 G VALUE "$123.45". 02 E PIC $999.99.` は
  `E` に `"$123.45"` を保持させます。
- ✅ `MOVE CORRESPONDING g1 TO g2` — 2 つの集団項目が名前を共有している従属項目を
  それぞれ転記し、名前が一致する下位の集団項目へ再帰的に降りていきます。
- ✅ **`CORRESPONDING` は `REDEFINES` または `RENAMES` で記述された項目を除外します**
  (COBOL-85 6.18.4 GR1)。これはどちら側でも同じで、その項目に従属するものすべても
  一緒に除外されます。除外がかかるのは名前ではなく*宣言*です。別の場所にある 66 レベルと
  名前が同じなだけの通常の項目は、依然として対応付けられます。
- ✅ **`CORRESPONDING` のどちらのオペランドも、集団項目の表の 1 つのオカレンスを指定できます** —
  `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` はそのオカレンス自身の格納場所に書き込み、
  添字は再帰の間ずっと引き継がれます。
- ✅ **1 組のうち、基本項目であるものが片方だけあれば十分です。** 集団項目が基本項目と
  向かい合うことがあり、その間の転記は英数字転記になります。基本項目 `PIC XXX` から
  `999` + `XXX` の集団項目へ送れば、その 6 文字が埋まりますし、`XXX` + `99` の集団項目から
  単純な `X(5)` へ送れば、そちらが埋まります。集団項目どうしが向かい合う場合は依然として
  **再帰**します — その組み合わせは基本項目のケースではありません。*(1.62.39 より前は
  どちらの向きでも何も転記されませんでした。集団項目は格納スロットを持たないため、書き込みは
  誰も読み返さない場所へ行き、読み出しは空文字列を返していました。)*
- ✅ **参照修飾 `id(start:len)`** — 送り手 (部分文字列) としても受け手 (部分的な代入の
  差し込み) としても使え、あらゆる動詞のオペランドで機能します。`length` は省略可能です。
  これは**文字位置**を指すので、数値オペランドは `PIC` の全幅で、先行ゼロを含めて
  扱われます。`01 T PIC 9(8) VALUE 00224845` なら `T(1:2)` は `"22"` ではなく `"00"` です。
- ✅ **集団項目は英数字の集合体です** — 集団項目*とは*その従属項目を端から端まで
  並べたものであり、その大きさは従属項目の大きさの合計です。集団項目を読むと子項目が
  (`FILLER` も含めて) 連結され、集団項目へ転記するとバイト列が幅に応じて子項目へ
  分配されます。`MOVE 11 TO A` は `A` を含む集団項目を通して見えますし、
  `MOVE "1234" TO G` は `G` 自身のスロットではなく `G` の子項目を設定します。
- ✅ 添字 `t(i)`、`t(i, j)` — オカレンスごとの格納スロットを読み書きします。
  可変の添字 `t(WS-I)` はアクセスのたびに評価されます。
- ✅ 修飾 `id OF/IN group` (`… OF g1 OF g2`) — 末端の名前が複数の集団項目の下に
  宣言されている場合でも、正しい項目に解決します。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **受け手ごとの `ROUNDED`** — 各受け手が自分自身の `ROUNDED` 指定を持ちます。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — 名前が一致する数値の組を
  それぞれ計算し、名前が一致する下位の集団項目へ再帰的に降りていきます。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **複数の `GIVING` 受け手**。それぞれが自分自身の `ROUNDED` を持ちます。
- ⚠️ `DIVIDE a BY b` (`GIVING` なし) は `a/b` を `a` に書き戻します (PowerRustCOBOL の
  利便性のための拡張です。標準の COBOL はここで `INTO` か `GIVING` を要求します)。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **複数の受け手が可能で、それぞれが自分自身の `ROUNDED` を持ちます**。
- ✅ 式の演算子 `+ - * /` と `**` (べき乗、右結合)、括弧、
  `FUNCTION name(args)`。

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`。
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`。
- ✅ **`ALSO` による複数主辞** — `WHEN` の各列は対応する主辞と位置ごとに突き合わされ、
  AND で結合されます。
- ✅ **`WHEN NOT value`** は選択対象を否定します。**`WHEN condition`**
  (例: `EVALUATE TRUE WHEN a > b`) は真偽条件を評価します。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES` (n = 整数定数またはデータ項目)。
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`。
- ✅ インラインの `PERFORM UNTIL cond … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ インラインの `PERFORM n TIMES … END-PERFORM` (段落なし)。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — 繰り返しのたびに段落を
  実行します (アウトオブライン、`END-PERFORM` なし)。
- ✅ **`WITH TEST AFTER` は `VARYING` にも適用されます**。この句の前後どちらに書いても、
  インラインでもアウトオブラインでも同じです。本体は何かがテストされる前に一度実行され、
  その後に条件が**内側から順に**テストされます。条件が偽になった段が加算され、その内側の
  段はすべて `FROM` の値に戻され、本体がもう一度実行されます。変数はテストが偽になった
  ときにだけ加算されるので、ループを終わらせたテストは、その変数を本体が残したままにします。
- ✅ **`AFTER` の変数は、そのループが終わったときに `FROM` の値へ戻されます**。これは
  1 つ外側の段が加算される前に行われます (COBOL-85 6.20.4 GR10(d))。`PERFORM` 全体が
  終わった後、内側の変数は `FROM` の値を保持しており、最も外側の変数だけが
  ループを終わらせた値を保持しています。
- ✅ **添字付きの `VARYING` 識別子は自分の添字に従います。**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` は、その時点で
  `S1` が選んでいるオカレンスを加算するので、`S1` を進める本体を書けば表全体を
  たどれます。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`。
- ✅ **`{OF|IN} section` の修飾子は、どの複製を指しているのかを選びます。** これは段落名が
  複数のセクションにまたがって繰り返されるときに効き、`PERFORM` の場合とまったく同じです。
  **未知の**セクションが指定された場合は、ジャンプを失う代わりに修飾なしの探索へ
  フォールバックします。`GO TO … DEPENDING ON` は名前の単純な並びだけを取り、修飾子は
  取りません。また `ALTER` によって差し替えられた `GO TO` は、その差し替えに従います —
  差し替えは行き先そのものを直接指定するからです。*(1.62.39 より前は修飾子は構文解析された
  あと無視されていたので、ジャンプはプログラム内のどこであれ最初の定義に着地していました。)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`。
- ✅ 単独の `EXIT` は何もしない復帰点です。`EXIT PROGRAM` は呼び出し元に戻ります。
- ✅ `EXIT PERFORM [CYCLE]` (最も近いインライン PERFORM を中断 / 次の周回へ)、
  `EXIT PARAGRAPH`、`EXIT SECTION`。
- ✅ `NEXT SENTENCE` — 次の文の境界を越えて制御を移します (構文解析器が各ピリオドに
  境界マーカーを挿入します。単なる `CONTINUE` ではなく、規格に忠実な実装です)。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`。
- ✅ **`SPECIAL-NAMES` がそのニーモニックを宣言している場合、`FROM mnemonic-name` は
  オペレーターから読み取ります** (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` …
  `ACCEPT ACCEPT-D1 FROM ACCEPT-INPUT-DEVICE`)。これは形式 1 であり、単独の
  `ACCEPT id` と同じです。**どの `SPECIAL-NAMES` 句も宣言していない**名前は
  PowerRustCOBOL の拡張のままで、その名前の**環境変数**を読み取ります。どちらが
  適用されるかは宣言が決めるのであって、綴りが決めるのではありません。
  *(1.62.35 より前は、通常の `<implementor-name> IS <mnemonic>` 句がまるごと読み飛ばされて
  いたため、あらゆるニーモニックが一度も設定されていない環境変数を読み、受け取る側の
  項目は空のままになっていました。)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` はカーソルを位置付けます (ANSI、CLI)。
- ✅ `FROM COMMAND-LINE` (コマンドライン全体) · `FROM ARGUMENT-NUMBER` (引数の個数)
  · `FROM ARGUMENT-VALUE` (`DISPLAY n UPON ARGUMENT-NUMBER` で設定したポインタ位置の
  引数) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`
  (`DISPLAY "name" UPON ENVIRONMENT-NAME` で指定した変数) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`。
- ✅ `END-ACCEPT` は文を閉じます (省略可)。

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`。
- ✅ `END-DISPLAY` はオペランドの並びを閉じます (省略可)。したがって
  `DISPLAY A END-DISPLAY DISPLAY B` は 1 つではなく 2 つの文になります。
- ✅ 画面形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — **CLI モード** (`rcrun`) では
  ANSI のカーソル位置付け + SGR で実行されます。GUI モードでは無視されます (そこでは
  フォームデザイナーが SCREEN 入出力に取って代わります)。`ACCEPT id AT …` は
  位置付けてから読み取ります。

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`。
  オーバーフロー = 組み立てられた文字列が受け取る項目より長いこと。
- ✅ **`DELIMITED BY` 句は、その手前にある送り手の並び全体を支配します。** 直前に
  書かれた 1 つだけではありません。
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` は 3 つすべてを区切って
  `"ABC"` を組み立てます。1 つの文が複数の句を持つこともでき、各句は 1 つ前の句以降の
  送り手を支配します。最後の句より後の送り手は、それぞれ全体が取られます。
  *(1.62.40 より前は、句の直前に書かれた送り手だけが区切られていました。)*
- ✅ **集団項目への `INTO`** は、その集団項目の従属項目へ分配します。
- ✅ **結果は 1 バイトずつ組み立てられる**ので、`STRING HIGH-VALUE` は 1 バイトの
  `0xFF` を転記し、1 文字位置を占めます。
- ✅ **拡張 — 賢い既定の `DELIMITED BY`** (どの句もそのオペランドを支配していない場合):
  英数字の `PIC X`/`A` 項目は既定で `SPACES` になり (末尾の詰め物は落とされます)、
  文字列定数、数値項目、数字編集項目、`FUNCTION` の結果、および式は既定で `SIZE` になります。
  データ項目は項目の形のまま転記されます (数値 → PIC の全幅の数字、数字編集 → 編集後の文字)。

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`。オーバーフロー = 送り側の項目が受け手より
  多いこと。

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` — **両方の半分が適用されます**。
- ✅ `BEFORE/AFTER INITIAL` は各句を項目の部分領域に限定します。
  (TALLYING は COBOL のとおり、カウンタに加算していきます。)
- ✅ **TALLYING のオペランドの並びは、左から右への走査を 1 回だけ共有します** (COBOL-85
  6.17.3)。各文字位置で、オペランドは書かれた順に試されます。最初に一致したものが
  その位置を取り、走査はそれが消費した文字の先から再開します。したがって
  `"AABA"` に対する `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` は `t1 = 1, t2 = 1` になり、
  オペランドを逆順に書けば `t1 = 3, t2 = 0` になります。`LEADING` は自分の窓の左端から
  隙間なく一致しなければならないので、先行するオペランドがその位置を取ってしまうと、
  連なりは始まる前に終わります。また `CHARACTERS` は、先行するどのオペランドも
  取らなかった位置だけを数えます。
- ✅ **REPLACING のオペランドの並びも、同じ規則で走査を 1 回だけ共有します。**
  ある位置で最初に一致したオペランドがその文字を置き換え、走査はその先から再開するので、
  後続のどのオペランドもその文字を見ることはできません。各オペランドの `BEFORE`/`AFTER` の
  窓は**どの置換よりも前に**固定されます。だからこそ、先行するオペランドが上書きしてしまう
  文字を足がかりにして、別のオペランドを位置付けることができます。

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  オペランドを 1 つずつ適用していたなら、最初の句が 2 番目の句の足がかりである `"L "` を
  消してしまい、`"BAD"` は生き残っていたはずです。
- ✅ **符号付きの DISPLAY 項目は、その文字位置のどこにも `-` を持ちません。** 演算上の符号は
  数字へのオーバーパンチなので、
  `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` は **0** を与え、
  `FOR ALL "5"` は 1 を与えます。符号は後で復元されるため、数字にかけた `REPLACING` は
  符号に手を触れません。`SIGN IS … SEPARATE CHARACTER` は符号*そのものが* 1 つの位置に
  なる場合で、このときは数えられます。

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (MOVE にコンパイルされます)。
- ✅ `SET idx {UP|DOWN} BY n` (ADD / SUBTRACT として符号化されます)。
- ✅ `SET 88-name TO TRUE` は、その条件の最初の VALUE を親項目に設定します。
  `TO FALSE` は VALUE の集合の外にある値を設定します (最善努力です — FALSE 句はありません)。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` と
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — 下記の**ポインタ**を参照してください。

### INITIALIZE
- ✅ `INITIALIZE id …` — 項目の分類を意識します: 数値 / 数字編集 → ZERO、
  それ以外はすべて → SPACES。集団項目には再帰的に降りていきます。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — その分類に属する従属項目を
  すべてその値に設定し、それ以外には手を触れません。

### ポインタ (USAGE POINTER)
- ✅ `USAGE POINTER` はポインタを宣言します (初期値は NULL)。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — `id` を対象の記憶域への
  別名にします (読み取り**も**書き込みも別名に従います)。通常は LINKAGE のレコードです。
  `IF ptr = NULL` も動作します。

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`。
- ✅ `ON EXCEPTION` / `ON OVERFLOW` の本体は、呼び出されたプログラムが解決できないときに
  実行されます。`NOT ON EXCEPTION` の本体は、呼び出しが**解決したとき**に実行されます。
- ✅ `CANCEL program …` は指定したプログラムの WORKING-STORAGE を初期化し直すので、
  次の `CALL` は最初からやり直しになります。

### ファイル関連の動詞 (サポートされる句 — 網羅的な内容はファイル入出力のテストスイートにあります)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`。
  (`SHARING` / `WITH LOCK` は構文解析され、意味のあるところでは尊重されます — 単一
  実行単位のモデルでは助言的なものです。)
- ✅ **1 つの `OPEN` が複数のモードのまとまりを持てます**。まとまりごとに自分のファイルを
  持ちます: `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` 各まとまりはそれぞれのモードで
  開かれます。`SHARING` / `WITH LOCK` / `REGISTERED USER` は文全体に適用されます。
- ✅ **すでに開いているファイルの `OPEN` は `41`** であり、ファイルはそのままの状態で
  残ります — その文はファイルを開き直し**ません**。(`OUTPUT` のファイルを開き直すと、
  プログラムがすでに書いた内容を黙って切り捨ててしまうためです。)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (PowerRustCOBOL の
  拡張) — オペレーター/利用者を INDEXED の可観測性ログに記録します (そのファイルの
  セッションのすべてのイベント行の `user=` フィールド)。純粋に観測のためのもので、
  認証や認可は行いません。
  [`observability-jp.md`](observability-jp.md) §1.3.1 を参照してください。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` は、INDEXED エンジンが I-O のもとで取得するレコードロックを解放します。
- ✅ **`READ … INTO id` は `READ` に集団項目の `MOVE` が続いたものです。** レコードは
  受け手の従属項目へ幅に応じて分配され、受け手自身の幅で切り詰められます。受け手には
  添字を付けられ、この転記はバイト列を運ぶので、文字ではないバイトを含むレコードも
  そのまま届きます。
- ✅ **FD の `RECORD` 句 — 可変長レコード。** 3 つの書き方すべて:
  `RECORD CONTAINS n CHARACTERS` (固定長)、`RECORD CONTAINS n TO m CHARACTERS`
  (可変長。`WRITE` が指定するレコード記述が長さを与えます)、そして
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (そのデータ項目*が*長さそのもの — `WRITE` の前に設定し、`READ` が設定し直し、
  宣言された範囲に収められます)。`01` レコードの大きさが異なる FD は、そう書いてあるか
  どうかにかかわらず可変長です。可変長ファイルは各レコードの長さをレコードとともに
  格納するので、そのバイト列は固定長ファイルのものと交換**できません**。固定長ファイルは
  変わりません。
- ✅ **FD の `01` レコードは 1 つのレコード領域を記述します。** `READ` はすべてのレコード
  記述を通してバイト列を届けます。`WRITE` は領域全体を送るので、書き込む側のレコード記述が
  `FILLER` としている位置に別のレコード記述が置いたものは、そのまま透けて出ます。
- ✅ **`FILLER` は FD のレコードの中で自分のバイトを占めます。** また
  `SIGN IS SEPARATE CHARACTER` は、符号付きの DISPLAY 項目を数字位置より 1 文字だけ
  幅の広いものにします。
- ✅ **FD の `LINAGE` は整数だけでなくデータ名も取ります** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`。ページは
  `WRITE` のたびにそれらの項目から測られるので、プログラムは実行中にページの大きさを
  変えられます。`LINAGE-COUNTER` はファイルを開いた時点で 1 です。
- ✅ **`AT END` の後の順ファイルの `READ` は `46` であり、2 度目の `10` ではありません。**
  `AT END` は有効な次のレコードを残さなかったので、そのまま読み続けることは、終わりに
  到達することとは別の誤りです。`46` はクラス 4 の状態なので、`AT END` も `NOT AT END` も
  実行されません — これを処理するのはそのファイルの `USE` 宣言部です。新たな `OPEN`、または
  成功した `START` によって、レコードが再び確立されます。
- ✅ `UNLOCK f [RECORD[S]]` はそのファイルのレコードロックを解放します。
- ✅ **`COMMIT` / `ROLLBACK`** — 開いている**すべての** INDEXED ファイルにまたがる、
  プログラムが制御するトランザクションです。`OPEN` がトランザクションを開始します。
  `COMMIT` は保留中の `WRITE`/`REWRITE`/`DELETE` を確定し (その後の `ROLLBACK` では
  もう取り消せません)、新しいトランザクションを開始します。`ROLLBACK` は直近の
  `COMMIT`/`OPEN` 以降のすべての変更を取り消します。**DISK** の記憶方式では
  `COMMIT`/`CLOSE` がディスク上で永続化されます。**MEMORY** の記憶方式では
  `COMMIT`/`ROLLBACK` は完全に RAM の中だけで行われます (ディスクには一切書きません)。
  単なる `STORAGE IS MEMORY` のファイルは一時的なもので、
  `STORAGE IS MEMORY WITH PERSISTENCE` は `CLOSE` のときにだけディスクへ保存します。
  (永続的な先行書き込みログによるクラッシュ回復は今後の課題です — ここにあるのは実行中の、
  プログラムレベルのロールバックです。)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (INDEXED ファイル、PowerRustCOBOL の拡張)。既定の記憶方式は
  `DISK` です。`WITH COMPRESSION` は格納するレコードを圧縮します (キーは圧縮前の
  レコードで評価されます)。`WITH PERSISTENCE` (MEMORY のときだけ) は RAM 上のファイルを
  `CLOSE` のときに保存します。`OPEN OUTPUT` は常にディスク上の入れ物を作り直します。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ **レコード順ファイル (SEQUENTIAL) に対する `REWRITE`** は、直前の `READ` が届けた
  レコードをその場で置き換え、読み取り位置は元のままにします — 次の `READ` は依然として
  その後ろのレコードを返します。返すべき状態は次のとおりです。ファイルが `I-O` で
  開かれていないときは **`49`**、成功した `READ` がレコードを確立していないときは
  **`43`** (`AT END` の後や、間に `READ` を挟まない 2 度目の `REWRITE` を含みます)、
  そして新しいレコードが読んだレコードと同じ長さでないときは **`44`** です —
  `DEPENDING ON` のファイルでは、その項目の値がその長さであり、プログラムはそうやって
  別の長さを要求します。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ *プロセス*をまたぐファイル共有は強制されません (単一実行単位)。`SHARING`/`LOCK` の
  句は構文解析され、INDEXED エンジンの実行単位内のレコードロックは尊重されます。

### SORT / MERGE / RELEASE / RETURN  ✅ (機能します。作業バッファはメモリ上)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]` (INPUT PROCEDURE の中で) はその実行に追加します。
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` がレコードを返します。
- レコードは宣言されたキー (`ASCENDING`/`DESCENDING`) で安定ソートされます。
  `USING` は指定された順ファイルを読み、`GIVING` はそれらに書きます。

---

## 条件 (IF / EVALUATE / PERFORM UNTIL)

- ✅ 関係記号: `=` `<>` `<` `>` `<=` `>=`。
- ✅ 語による関係: `[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`、`[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ 級 (クラス): `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
  PICTURE に**演算符号を持たない**項目が `NUMERIC` になるのは、すべての文字位置が
  数字を保持しているときだけです — `"+1234"`、`"1.234"`、`"12 45"` を保持する
  `PIC X(5)` は数字**ではありません**。*(1.62.40 より前は、この判定が文字列を数値
  として解析していたため、符号・小数点・指数・前後の空白がいずれも受け入れられて
  いました。)*
- ✅ **利用者定義の `CLASS` のオペランドには順序位置を書けます** —
  `CLASS ORDINAL-A-ONLY IS 66` は固有文字集合の 66 番目の文字を指します — また、
  そのオペランドを独立した行に置くこともできます。`ALPHABET` でも同様です。
- ✅ 符号: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 レベルの条件名 (名前を単独で条件として書く形)。
- ✅ **オペランドとしての `TRUE` / `FALSE`** (PowerRustCOBOL の拡張) — 値が許される
  場所ならどこでも使える `1` と `0` の糖衣構文です: `IF x = TRUE`、
  `IF x IS [NOT] FALSE`、`IF x NOT TRUE` (関係演算子を伴わない、`NOT` 単独の形)、
  `PERFORM UNTIL x = FALSE`、`MOVE TRUE TO x`、`COMPUTE n = n + TRUE`、
  `INVOKE obj "m" USING TRUE`、そして値を主部とする `WHEN TRUE`。単独の
  `TRUE`/`FALSE` はそれ自体で完全な条件でもあります (`IF TRUE`、
  `PERFORM UNTIL TRUE`)。
  ⚠️ これは、これらの語がすでに意味を持っていた 2 か所を**変えません**:
  `SET <88‑name> TO TRUE` は従来どおり、その条件を満たす値を親項目に設定し
  (数値の 1 ではありません)、後述の `EVALUATE TRUE`/`EVALUATE FALSE` も標準の
  分岐文のままです。
- ✅ `AND` / `OR` / `NOT` の組み合わせと括弧 (AND は OR より強く結合します)。
- ✅ **演算子が前置された省略条件** — `a > 1 AND < 9`、`a = 5 OR = 7`
  (直前の比較の主部が再利用されます)。
- ✅ **定数を目的語とする省略形** — `a = 1 OR 2 OR 3` (主部と演算子の両方を再利用
  します。目的語は定数です)。
- ✅ **データ名を目的語とする省略形** — `a = b OR c` (`c` はデータ項目)。比較の
  あとの AND/OR に続く単独の識別子は実行時に解決されます: 既知の 88 レベル条件名
  ならそれとして評価され、そうでなければ `a = c` の目的語になります。(識別子の
  直後に `AND` が続く場合は AND の優先順位が保たれます。)
- ✅ **省略形の*目的語*の前に置いた `NOT` は関係を否定します**。目的語を否定するの
  ではありません: `a > b OR NOT c` は `a > b OR NOT (a > c)` です。`NOT
  <relational operator>` という書き方 (`AND NOT < x`) は演算子の形であり、従来
  どおりです。また、
  通常の条件を開始する `NOT` — `NOT (…)`、`NOT x = y`、`NOT x NUMERIC` — は本来の
  意味を保ちます。*(1.62.42 より前は、目的語の形が「目的語が非ゼロである」と読まれて
  いました。これが同じ答えになるのは、目的語がたまたまゼロを保持しているときだけ
  です。)*
- ✅ **集団項目に宣言された条件名は、その集団のバイト列を検査します。** 集団項目は
  自分自身の記憶域を持たず — それは*子項目そのもの*です — したがって
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` はレコードが保持する
  6 文字と比較します。
- ✅ **表意定数はもう一方のオペランドの大きさまで繰り返されます**。これは 88 の
  `VALUE` として書かれたものにも当てはまります: `PIC X(4)` の親項目に対する
  `88 B VALUE QUOTE` は引用符 4 個であり、`88 D VALUE ALL "BAC"` は `"BACB"` です。
  `ALL literal` は**両方向**に寸法が決まります — 10 文字の `X` に対する
  `IF X EQUAL TO ALL "BA"` は、空白で埋めた `"BA"` ではなく `"BABABABABA"` と
  比較します。

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
  （日付変換は標準の基準 1601‑01‑01 = 第 1 日を使用します。）**COBOL‑85 標準の
  組み込み関数一式**が実装されています。
- ✅ **日付・時刻のレジスタは LOCAL な時計を読みます。** `ACCEPT … FROM DATE /
  TIME / DAY / DAY-OF-WEEK` も `FUNCTION CURRENT-DATE` も、UTC ではなくマシン
  自身の時刻を報告します — 日付も同様で、真夜中を挟むと値が変わります。
  `CURRENT-DATE` の末尾 5 文字は GMT からの**実際の**オフセット（`…-0300`）を
  保持しているので、プログラムは自分がどのタイムゾーンで動いているかを判別でき
  ます。
  ⚠️ 認識されない `FUNCTION` 名も解析はされますが、実行時に **0** を返します。
- ✅ リテラル：整数、小数、文字列、すべての表意定数
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **表意定数は受け手の全体を埋めます。** `HIGH-VALUE` も同じで、
  `MOVE HIGH-VALUE TO <PIC X(10)>` は `0xFF` が 10 バイトになり、グループへ移す
  ときは子項目に分配されます。英数字編集の受け手は挿入文字を従来どおり配置
  するので、`PIC XX0XXBXXX` は `FF FF '0' FF FF ' ' FF FF FF` を保持します。
  `PROGRAM COLLATING SEQUENCE` の下では、この定数は通常の文字を指し、その文字が
  代わりに埋めます。
  ⚠️ `HIGH-VALUE` は文字ではなく**バイト** `0xFF` です。グループ被演算子の読み
  取り、編集、そしてあらゆる転記経路はこれをバイト単位でそのまま運びますが、
  **参照修飾はまだバイト単位で正確ではありません**。本当に `0xFF` を保持している
  項目に対しても `IF X (1:1) = HIGH-VALUE` は偽になります。
- ✅ **数字リテラルは小数点で始めることができます** — `.5`、`-.5`、
  `.000000001`。COBOL‑85 が求めているのはリテラルが小数点で*終わらない*ことだけ
  なので、`5.` は依然として数値 5 とそれに続く文の終止符です。
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  先頭のゼロは有効かつ正確です。`.000000001` は 10 億分の 1 であって 10 分の 1
  ではありません。`DECIMAL-POINT IS COMMA` の下では `,5` にも同じことが当ては
  まります。
  リテラルと文末のピリオドを分けるのは**空白の有無**です。COBOL‑85 は終止符の
  後に空白を 1 つ要求するため、`MOVE X TO Y.` が小数の始まりとして読まれること
  はなく、`MOVE X TO Y.5` は黙って解釈し直されるのではなくコンパイルエラーに
  なります。
- ✅ **適合性フラグ付け**（`cobolt_semantic::flagging`）— 標準は、適合実装が、
  プログラムの使用している機能のうちどれが選択した適合水準の外側にあるかを
  伝えられることを求めています。2 つの解析がそれに答えます。
  - `flag_obsolete` — COBOL‑85 の**廃止要素**の集合。IDENTIFICATION DIVISION の
    5 つの省略可能な段落、`MEMORY SIZE`、`ALTER`、リテラルを伴う `STOP`、および
    手続き名を伴わない `GO TO`。
  - `flag_high_subset` — **高位サブセット**を超えるすべて。`COMPUTE`、
    `EVALUATE`、`INITIALIZE` から始まり、`CORRESPONDING`、参照修飾、名前の修飾、
    `SET … TO TRUE`、4 番目の添字を経て、*語*や*数字リテラル*をカード境界を
    またいで継続することまで。（**英数字**リテラルの継続はサブセット内であり、
    報告されません。）

  どちらもエラー検査ではなく、通常のビルドでは実行されません。これらが挙げる
  構文はいずれも RustCOBOL が実装し実行する正当な COBOL‑85 です。通常のコンパイル
  が `AUTHOR` や `COMPUTE` について警告を出し始めることのないよう、あえて別の
  エントリポイントになっています。NIST の `NC302M`、`NC303M`、`NC401M` がこれらを
  検証しており、フラグは 7 件、4 件、40 件ですべて一致しています。
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — 編集用 PICTURE の通貨
  位置を埋める文字です。`$` に加わるのではなく `$` を**置き換える**ため、
  プログラムがいったんこれを宣言すると、その場所で `$` はもはや picture 文字では
  なくなります。
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  このとき `MOVE ZERO TO FL-LESS` は `      <.00`、`MOVE 1234` は ` <1,234.00`
  となります。浮動する並びは `$$$,$$$.99` とまったく同じように振る舞います。
  **英字**の通貨記号も同じように機能します。`CURRENCY SIGN IS "W"` とすると
  `PICTURE WWWWW` は 5 桁の浮動通貨文字列になり、`MOVE 12` は `  W12` となり
  ます。*（1.62.40 より前は、英字記号の並びが 1 つの語として読まれて拒否されて
  いたため、浮動するのは `$` だけでした。）* この
  リテラルは 1 文字でなければならず、COBOL‑85 は picture 文字や区切り文字と
  衝突するものを禁じています。数字は不可、`A B C D E G N P R S V X Z` のいずれも
  不可、`space * + - , . ; ( ) " / =` のいずれも不可です。
- ✅ **16 進リテラル** — `X"09"`、`x'0D0A'`（大文字小文字・引用符の種類は問い
  ません）。16 進数字の**ペア**ごとに 1 文字なので、桁数は偶数でなければなり
  ません。奇数桁や 16 進でない文字は不正なリテラルとして報告され、文字列の隣に
  ある単語 `X` として黙って読み直されることはありません。引用符付きリテラルが
  使える場所ならどこでも使えます（`DELIMITED BY`、`MOVE`、`VALUE`、比較）。

---

## DATA DIVISION の句 (受理される宣言構文)

- ✅ レベル `01`–`49`、`77`、`88`、`FILLER`、集団項目/基本項目。`FILLER` という
  語は**省略可能**です — `05 PIC X VALUE ":".` は
  `05 FILLER PIC X VALUE ":".` とまったく同じように FILLER を宣言し、どちらの
  書き方でも、それを含む集団項目の内部で自分のバイトを占め、自分の `VALUE` を
  保持します。
- ✅ `X A 9 S V P` と編集記号 (`Z * $ + - CR DB B 0 / , .`) を伴う
  `PIC/PICTURE`。通貨記号は、`SPECIAL-NAMES. CURRENCY` が別のものを指定して
  いない限り `$` です — 上記の**式、リテラル、USAGE**を参照してください。
  **`P` は小数位取り位置です** — 項目が範囲として覆うものの格納はしない数字位置
  のことで、`PIC S999PP` は百の位を表す 3 桁を保持し (`MOVE 12300` はそれを
  そのまま格納し、`MOVE 12345` は 12300 を格納します)、`PIC PP99` は 1 万分の 1
  の位を表す 2 桁を保持します。`P` が占める位置は常にゼロとして読み出され、
  レコードレイアウト上では**バイトを消費しません**。
- ✅ **アスタリスク保護は項目全体を埋めます。** 数字位置がすべて `*` である
  PICTURE にゼロが入ると、すべての文字位置がアスタリスクで埋まります — 小数部の
  桁も、桁区切りのカンマも、固定の `$` も、末尾の `CR` や `DB` も同じように埋ま
  り — 残るのは小数点そのものだけです。ゼロを保持する `PIC $**.**CR` は
  `***.****` と読め、`PIC *,***.**` は `*****.**` と読めます。ゼロ**でない**値
  では先行ゼロだけが保護されるため、固定の `$` は自分の位置を保ちます
  (`-2.34` → `$*2.34CR`)。*(1.62.37 より前は `CR`/`DB` が、実際に占める 2 つの
  文字位置ではなくアスタリスク 1 個分しか寄与していなかったため、そのような項目
  は自分の幅より 1 文字短く返ってきていました。)*
- ✅ **数字定数は、書かれたとおりに自分の文字を転記します。** 英数字の受け手に
  対して、定数はプログラムが書いた桁を左詰めで、空白を埋めて渡します —
  `MOVE 2 TO <PIC X(4)>` は `"2   "` になり、
  `MOVE 060820000200 TO <six PIC 99 children>` はそれらを `06 08 20 00 02 00`
  と埋めます。**受け手**の幅が定数を埋めることは決してなく、埋めるのは定数自身
  が書かれた幅だけです。*(1.62.38 より前は lexer が値だけを保持していたため、
  先行ゼロが失われ、後続のすべての文字が 1 桁左へずれていました。)*
- ✅ **数字オペランドと非数字オペランドの比較は非数字の比較です**
  (COBOL‑85 VI‑89 6.15.4 GR2)。数字オペランドは、**自分自身の大きさ**の英数字
  項目へ転記されたものとして扱われ、その文字位置は転送されますが**演算符号は
  転送されません**。`-123456789012345678` を保持する `PIC S9(18)` は、
  `"123456789012345678"` を保持する `PIC X(18)` と**等しい**と判定されます。
  この規則は 3 つの条件で限られます — 数字オペランドは**整数**でなければ
  なりません。「非数字」かどうかは**宣言**が決めるので、集団項目の `MOVE` の
  あとで文字を保持している `PIC 99` の従属項目は依然として数字項目です。そして
  **集団項目**は従属項目が何であれ非数字なので、12345 を保持する `PIC 9(5)` を
  `"0000012345"` を保持する 10 バイトの集団項目と比べると `"12345     "` と
  なり、等しくありません。また `ALL literal` は相手のオペランドの大きさを取り
  ます。*(1.62.38 より前は、テキスト側がたまたま数値として解釈できる場合には
  常に代数的な比較になっていました。)*
- ✅ **数字項目への MOVE では上位桁が切り捨てられます。** 受け手は宣言した桁数
  だけを両端で保持します。`01 M PIC 99V999.  MOVE 123.45 TO M.` の結果は
  `23.450` です。算術演算は先に受け手の容量を検査するため、`ON SIZE ERROR` を
  伴う文は代わりに元の値を保持します。
- ✅ **集団項目の表は出現ごとにアドレス指定されます。** `MOVE VALUES-1 TO
  GRP-1 (2)` はその出現自身の従属項目 (`ELEM1 (2,1) … ELEM1 (2,4)`) に値を配分
  し、`GRP-1 (2)` を読むとちょうどそれらが連結されます。それを囲む `01` レコード
  は**すべての**出現のバイトなので、`MOVE GRP-TAB1 TO GRP-TAB2` は表全体をコピー
  します。
- ✅ **指標名・定数・相対指標付けは添字として混在できます。**
  `ELEM1 (IN1, 1)`、`ELEM1 (1 IN2)`、`ELEM1 (IN1 +3)` — 符号が数字に密着して
  いる場合は符号付き定数であり、次の添字を開始します — また
  `ELEM1 (IN1 - 1, 3)` のように演算子の両側に空白がある場合は相対指標付けです。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (および `COMP-4`→COMP、`COMP-X`→COMP-5)。
- ✅ `VALUE` (数字/符号付き/英数字/表意定数/`ALL`)。**`VALUE ALL "literal"` は
  項目全体にその単位を繰り返します** — `PIC X(6) VALUE ALL "ABC"` は
  `"ABCABC"`、`PIC X(9) VALUE ALL "XY"` は `"XYXYXYXYX"` です。
  *(1.62.40 より前は 1 文字の表意定数だけが項目を埋め、`ALL "literal"` は項目を
  空白のままにしていました。)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES` — 同じバイトに対する**生きた** 2 つ目の見方です。記憶域を追加
  しないので (それを含む集団項目を広げません)、どちらの記述を通した書き込みも
  もう一方から見えます。
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` のあと `RESULT-A` を通して読み返せます。
  ⚠️ **注意:** 展開された記憶スロットが 256 個を超えるオーバーレイ (再定義された
  10×10×10 の表など) は、記述ごとの記憶域を保持します — 書き込みのたびに更新
  すると 1000 個の出現を 2 度走査することになるからです。
- ✅ **オーバーレイは入れ子になります。** それ自身が再定義されているレコードの
  内側にある `REDEFINES` は、どれだけ深くても双方向で到達されます。01 レベルの
  再定義を通して 2 バイトを書き込むと、再定義されたレコード、その内側の集団項目
  の `REDEFINES`、さらに*その*内側の項目の `REDEFINES` — 最も内側の項目に宣言
  された 88 も含めて — に届きます。各記述は書き込みごとに 1 回ずつ再生成され
  ます。*(1.62.42 より前は、複数のオーバーレイに属するキーが最後に宣言された
  ものだけを保持し、単一のガードが最初の 1 ホップで連鎖を止めていました。)*
- ✅ **名前のない記述も記述です。** `02 FILLER REDEFINES <item>.` は、自分自身
  の名前を持たないまま対象のバイトを記述し直し、対象への書き込みはその従属項目
  を通して見えます。従属項目が複数あれば、それらがレイアウト順にバイトを分け
  合います — オーバーレイは最初の従属項目の別名では*ありません*。同じ項目に
  対する 2 つの `FILLER REDEFINES` は独立した 2 つの見方であり、どちらも対象の
  **先頭**バイトから始まります。*(1.62.36 より前は、名前のない再定義集団項目に
  記憶キーがまったく与えられなかったため、対象がどう埋められていても従属項目は
  空白として読み出されていました。)*
- ✅ **オーバーレイ内の重複した名前**は、プログラムの他の部分が到達するのと同じ
  記憶域に解決されます。異なる 2 つの集団項目の下に宣言された `TAB-A` は、宣言
  ごとに 1 つの見方を保ちます。*(1.62.36 より前は、オーバーレイの初期コピーが
  外側の修飾子を欠いたパスでキー付けされていました。それは重複した名前でしか
  区別できないもの — つまり修飾子を必要とするまさにその場合に、修飾子が失われて
  いたのです。)*
- ✅ `JUSTIFIED [RIGHT]` — *英数字*項目または*英字*項目で、**右詰めで格納
  します**。受け手より狭い送り手は左側が埋められ、受け手より広い送り手は**右**端
  を残して最も左の文字を失います — 通常の規則とは逆です。*(1.62.40 より前はこの
  句が英数字項目についてしか記録されなかったため、`PICTURE A(5) JUSTIFIED RIGHT`
  は構文解析こそ通るものの、他の項目と同じように左詰めになっていました。)*
- ✅ `SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL` — 受理され
  ます。`SIGN … SEPARATE` はまだ項目の格納方法を変えません。
- ✅ **01 レベルの `REDEFINES` は、再定義する項目より多くの記憶域を記述できます**。
  その項目の末尾より先のバイトは、それらを名指せるだけの長さを持つ記述に属し
  ます。短いほうの記述を通して書き込んでも、長いほうの末尾部分はそのまま残り
  ます。
- ✅ **`REDEFINES` のオーバーレイは再定義された項目のバイトをそのまま持ち込み
  ます**。数字の相手方に対しても同様で、`"00ABCDEFGHI  4321 "` を保持する
  `X(18)` の `PIC S9(18)` オーバーレイはそれらの文字を読み返し、`IS NUMERIC` は
  それらに対して**いいえ**と答えます。バイトが実際に数字を綴っている場合、数字
  としての読み出しは変わりません。
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **本物の条件名**です。
  レベル 88 はホスト項目に束縛され、判定はホストを VALUE / 範囲と照合し、
  `SET 88-name TO TRUE` は条件を満たす値をホストに格納します。
- ✅ **条件名は複数の集団項目の下に宣言でき、`OF`/`IN` がそれらを区別します** —
  データ名の場合とまったく同じで、途中のレベルは省略できます:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  添字はホスト項目に属するので、VALUE がどの出現に対して判定されるかを選び
  ます。重複した条件名への**修飾なし**の参照は COBOL‑85 では曖昧です。ランタイム
  は最初の宣言を採用します — 曖昧なデータ名に適用するのと同じ規則です。
- ✅ `USAGE INDEX` は整数の指標レジスタを宣言します (`SET`/`SEARCH` がこれを
  使います)。`USAGE POINTER` — 上記の**ポインタ**を参照してください。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — 再グループ化の別名です。
  読み出しは対象となる項目群を連結し、書き込みは各フィールドの幅に従って配分し
  ます。
  - ✅ **66 はそれが再グループ化するレコードによって修飾されます**。データ項目
    がその上位の集団項目によって修飾されるのとまったく同じで、同じ 66 の名前を
    レコードごとに 1 回ずつ宣言し、`OF`/`IN` で区別できます:
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`。これは読み出しでも書き
    込みでも同じように働き、たまたま同じ名前を持つ通常のデータ項目よりも 66 が
    優先されます。`RENAMES` 句のオペランドも同じレコード内で解決されるため、
    重複した `NAME-2` はこのレコードのものを指します。
  - ✅ **対象に含まれる表は、最初の 1 つだけでなくすべての出現を寄与します**。
    `TABLE-2` が `03 T PIC XXX OCCURS 5` を保持しているとき、
    `66 R RENAMES ITEM-1 THRU TABLE-2` の幅は 20 文字です。
  - ✅ **ちょうど 1 つの項目にかかる 66 は、その項目*そのもの*です** — 同じ
    PICTURE、同じ種別、同じ記憶域になります。`W` が `PIC 9(4)` のとき
    `66 R RENAMES W` は 4 桁の数字項目なので、8000 が入った状態での
    `ADD 3500 TO R` は `ON SIZE ERROR` を発生させ、値は変わりません。
- セクション: `WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`。`SCREEN`
  は構文解析されますが実行されません。

---

## まだ未サポート — 現在の回避リスト

> **2026‑08‑25 に訂正。** この節はかつて「COBOL‑85 の動詞・句の集合は**完全に
> 網羅**されています。」という一文で始まっていました。NIST CCVS85 スイートを
> 実行したところ、それは覆されました。**対象 434 本のうち 102 本がその日
> 失敗した**のです。しかも、本書がギャップとして挙げていなかった構文が原因
> でした — 区切りのコンマとセミコロン、`FUNCTION x(ALL)`、
> `CLOSE … WITH LOCK`、B 領域の `COPY`、IDENTIFICATION の注記項目、
> セクションの優先番号、数字で始まるデータ名、そして — 1.62.10 までは —
> 小数点で始まる数字リテラル。検証スイートとはそのためにあります。各ギャップは
> 現在 [`specs/nist/`](../specs/nist/README.md) に仕様化され、上の
> [スコアボード](#-適合性は主張するものではなく測定するもの--nist-ccvs85)で追跡され
> ています。

以下のリストは**意図的に**対象外としているものであり、上記の NIST ギャップ
（対処が進行中の欠陥）とは異なります。

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
4. ⚠️ **RELATIVE** ファイル編成（SEQUENTIAL / LINE SEQUENTIAL / INDEXED は
   完了）。**これはきれいなギャップではなく罠です。** `ORGANIZATION IS RELATIVE`
   は*解析され*、しかもランタイムはそれに基づいて処理を振り分けることが一切
   ありません。つまり RELATIVE のプログラムはコンパイルが通り、その後は診断なしに
   誤動作します。NIST の RL モジュール 35 本のうち 30 本がまさにその状態です。
   未実装として扱ってください。仕様：
   [RELATIVE 編成](../specs/nist/NIST-spec-relative-organization.md)。
5. 認識されない組み込み関数名は依然として **0** を返します — 同じ静かな失敗の
   仕方です。仕様：
   [組み込み関数](../specs/nist/NIST-spec-intrinsic-function-gaps.md)。
6. ⚠️ **不正な `ACCESS MODE` ／ `ORGANIZATION` の値が診断なしに握りつぶされ
   ます。** これも同じ罠であり、しかもこちらは利用者のごく普通の打ち間違いで
   発生します。`ACCESS MODE IS` が受け付けるのは `SEQUENTIAL`、`RANDOM`、
   `DYNAMIC` だけですが（`INDEXED` は*編成*であってアクセスモードではありま
   せん）、SELECT 句のパーサはその 3 つを判定したうえで、それ以外は「未知の
   トークンを読み飛ばす」という汎用の分岐に落としてしまいます。その結果、
   ファイルは既定の `SEQUENTIAL` を黙って保持し、コンパイルに失敗する代わりに
   実行時に誤動作します。`ORGANIZATION IS` もまったく同じ形です。どちらも、
   問題の語を名指しする明確なコンパイル時エラーを出すべきです。**Nucleus の
   問題ではありません** — `ACCESS MODE` 句を持つ NC プログラムは 1 本もなく、
   この句が現れるのは DB、IC、IX、OBSQ、RL、RW、SQ、ST の各モジュールだけです。
   したがってゴールデンルール #9 のもと、これは NC が完了するまで待ちです。
7. ⚠️ **`ALPHABET … IS EBCDIC` は受け付けられますが、ネイティブ（ASCII）の
   順序がそのまま有効なままです。** リテラル句（`"A" THRU "H" "I" ALSO "J" …`）、
   `NATIVE`、`STANDARD‑1`、`STANDARD‑2` はいずれも実装済みで、
   `PROGRAM COLLATING SEQUENCE` を実際に駆動します。欠けているのは EBCDIC の表
   だけで、それを指定すると黙って ASCII 順になります。4–6 と同じ罠の系統です。
8. **通信モジュールと Report Writer** —
   [上の N/A](#-na--rustcobol-の対象範囲外にあるものとその理由) を参照してください。

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

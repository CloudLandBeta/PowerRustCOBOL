<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.0 -->

# RustCOBOL‑85 対応構文リファレンス

**この文書の目的：** RustCOBOL が COBOL‑85 標準をどれだけ実際に実装しているかを述べ、
それを主張するのではなく **NIST COBOL‑85 公式検証スイート** に対して証明することです。
下の[スコアボード](#-適合性は主張ではなく測定される--nist-ccvs85)が主見出しであり、
それ以降はすべてその数字の裏にある詳細です。

**RustCOBOL の字句解析器／構文解析器／ランタイムが今日実際に受け付けるものについての
実地の事実**であり、ソース（`cobolt-lexer`、`cobolt-parser`、`cobolt-runtime`）から導出し、
`NIST/newcob.val,cbl` と突き合わせています。
テストは ✅ の形式に対して書いてください。❌ の形式は構文解析に至らないか何もしません。
⚠️ の形式は解析されますが部分的にしか動作しません。本書は
[`cobol85-verb-test-matrix-jp.md`](cobol85-verb-test-matrix-jp.md) の対になる文書です。
マトリクスは*何を*テストするかを述べ、本書は*RustCOBOL がどの書き方を理解するか*を述べます。

凡例：✅ 対応 · ⚠️ 解析されるが部分的／簡略化 · ❌ 認識されない
（避けてください。あるいは欠落の確認のためだけにテストしてください）。

---

## 目次

1. [★ 適合性は主張ではなく測定される — NIST CCVS85](#-適合性は主張ではなく測定される--nist-ccvs85)
2. [IDENTIFICATION DIVISION の段落](#identification-division-の段落)
3. [ソース形式](#ソース形式)
4. [認識される文（動詞）](#認識される文動詞)
5. [動詞ごとの対応形式](#動詞ごとの対応形式)
6. [条件（IF / EVALUATE / PERFORM UNTIL）](#条件if--evaluate--perform-until)
7. [式・リテラル・USAGE](#式リテラルusage)
8. [DATA DIVISION の句（受け付ける宣言構文）](#data-division-の句受け付ける宣言構文)
9. [まだ未対応 — 現在の回避リスト](#まだ未対応--現在の回避リスト)

---

## ★ 適合性は主張ではなく測定される — NIST CCVS85

**これが本書の要点です。** 以下のすべての主張は **NIST COBOL‑85 公式検証スイート** に
対して検証されています — CCVS85 バージョン 4.0（01 OCT 1992、COBOL 85 バージョン 4.2、
1993 年 4 月 SSVG）。米国国立標準技術研究所が COBOL コンパイラを認証するために
使っていたスイートです。28 MB、348,271 行、**459 本の COBOL プログラム**と 51 個の
copybook メンバーからなり、本リポジトリの `NIST/newcob.val,cbl` に置かれています。

これが真実の源です。RustCOBOL と CCVS85 が食い違う場合、**CCVS85 が正しく RustCOBOL が
誤っています**。

機械可読の台帳は [`NIST/progress.json`](../NIST/progress.json) です —
版管理され、検証済みの変更ごとに更新されます。以下の数値は打ち直すのではなく
そこから取っています。

### スコアボード

**2026‑08‑31、1.62.132 で測定**、手を加えていない配布物に対して。コンパイル調査は
1.62.129 で締められました。

| 軸 | 結果 | 意味 |
|---|---:|---|
| **コンパイル** | **420 / 420** | 対象内のすべてのプログラムがフロントエンドに受け付けられます。FAIL 0。 |
| **実行** | **380 / 380** | 採点対象のすべてのプログラムが走り、自身の CCVS レポートで**失敗ゼロ**を報告します。 |
| **アサーション** | **8,362 PASS / 0 FAIL** | それらのプログラムが自分自身に対して行う検査。 |

どちらの軸も再現できます：

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### 2 つの軸は決して混同されません

コンパイルは厳密に弱い主張です。プログラム中のすべての構文をフロントエンドが
受け付けると述べているだけで、プログラムが正しい答えを計算するとは述べていません。
スイートは自分自身を採点します — CCVS85 の各プログラムが自身の `PASS` / `FAIL*` の
集計を印字します — したがって「動く」を意味するのは実行の軸です。両方とも下で
モジュールごとに、それぞれの分母とともに報告され、一方が他方として引用されることは
決してありません。

もっとも明快な実例は本リポジトリ自身の歴史にあります。RELATIVE ファイルの 35 本の
うち 30 本がきれいにコンパイルされていた一方で、ランタイムには **RELATIVE エンジンが
まったくありませんでした**。それらは実行され、黙って誤った結果を出していました。
エンジンは 1.62.76 で到着し、モジュールは 1.62.77 で完了しました。

#### モジュールごと

コンパイルと実行は、明言された 2 つの理由で異なる分母を持ちます。`*301M` メンバーは、
RustCOBOL が標準として実装している機能の*中間部分集合の標示*をテストするもので、
設計上到達不能であり、運用者の裁定により実行から除外されています（IX301M、RL301M、
ST301M、SM301M）。コンパイル調査では引き続き数えられ、そこでは合格します。また
IC メンバーの大半は**被呼び出し側** — 自分のレポートを持たない副プログラム — なので、
呼び出し側のプログラムだけが採点されます。

| モジュール | テスト内容 | コンパイル | 実行 | アサーション | 状態 |
|---|---|---:|---:|---:|---|
| **NC** | 中核 | **95 / 95** | **95 / 95** | 4,614 | ✅ 完了 |
| **SQ** | 順次入出力 | **85 / 85** | **85 / 85** | 624 | ✅ 完了 |
| **IX** | 索引入出力 | **42 / 42** | **41 / 41** | 574 | ✅ 完了 |
| **IF** | 組込み関数 | **45 / 45** | **45 / 45** | 841 | ✅ 完了 |
| **IC** | プログラム間通信 | **47 / 47** | **25 / 25** | 309 | ✅ 完了 |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ 完了 |
| **SM** | 原始テキスト操作 | **17 / 17** | **16 / 16** | 311 | ✅ 完了 |
| **RL** | 相対入出力 | **35 / 35** | **34 / 34** | 354 | ✅ 完了 |
| **DB** | デバッグ | **14 / 14** | — | — | コンパイル軸のみ（下記） |
| **対象内** | | **420 / 420** | **380 / 380** | **8,362** | |
| SG | 区分化 | 13 / 13 | — | — | ⬜ 対象外と裁定（下記） |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB（デバッグ）** はコンパイルのみで採点されます。14 本のプログラムは受け付けられ
ますが、デバッグモジュールの*実行*時意味論は実装されておらず、その実行軸は対象内と
裁定されていません。欠落が見えたままになるよう、隠さずここに載せています。

#### DELETED の数 — 24、およびその意味

`***** ****TEST DELETED****` は、プログラム自身が飛ばしたケースに対する CCVS 自身の
標識です。合格では**ありません**。そのため別に記録されています。1.62.53 でこの数が
108 → 1 に落ちた一方、きれいなプログラムの数はほとんど動きませんでした。失敗だけを
見る読み方では見落とされたはずの、実際の進捗でした。

完了したモジュール全体で 24 件が DELETED です：NC 5、SQ 6、IX 1、IC 4、SM 3、RL 5。
**配布物自身のものとして文書化されているのは SM の 3 件だけです** — SM206A の
PST‑TEST‑008 と PST‑TEST‑11、および SM208A の REP‑TEST‑7 はコメントアウトされた状態で
配布されるので、配布ソースを適合的に実行するとちょうどその 3 件が報告されます。
残りの 21 件は記録されていますが、台帳では個別に説明されていません。仕様どおりのもの
として引用しないでください。

### ⬜ N/A — RustCOBOL の対象外となるもの、およびその理由

これらのモジュールは**失敗として数えません** — すべての採点から除外された 38 本の
プログラムです。完全な論拠は
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md)
にあります。

| モジュール | プログラム数 | 対象外である理由 |
|---|---:|---|
| **CM** — 通信 | 9 | `COMMUNICATION SECTION`、`CD` 項目、`SEND` / `RECEIVE` / `ENABLE` / `DISABLE`。1980 年代の遠隔処理モニタ — トランザクション管理者が所有するメッセージキュー — を対象としています。ここにはそのようなランタイムが存在せず、モジュール自体も後続の COBOL 標準から削除されました。 |
| **RW** — Report Writer | 6 | `REPORT SECTION`、`RD` 項目、`INITIATE` / `GENERATE` / `TERMINATE`、制御切れ。大規模な宣言的下位言語です。帳票に対する PowerRustCOBOL の答えは Form Designer と PDF 出力です。望まれれば後から*機能*になり得ます — 利用者にとって実際の価値がある唯一の除外項目です。 |
| **SG** — 区分化 | 13 | 運用者の裁定、2026‑08‑29。区分化は、プログラムを収めきれないほど小さな機械に収めるために存在します。`SECTION` 見出しが区分番号を持ち、ランタイムが独立した区分を互いに重ね置きします。RustCOBOL は 64 ビットのランタイムで、どんな COBOL プログラムでも使い切れないほどのアドレス空間があるため、区分番号は**コンパイルされ、まったく効果を持ちません**。モジュールが測定できる振る舞いが存在しないのです。13 本のプログラムは今もコンパイルされ、除外が見えたままになるよう、削除ではなく N‑A として報告されます。 |
| **OBSQ / OBIC / OBNC** | 9 | 先行モジュールを再テストし、COBOL‑85 の廃要素をコンパイラが*標示*することを期待します。言語内容は対象内の仕様で網羅されています。対象外なのは廃機能の**標示**です。 |
| **EXEC85** | 1 | テストではありません。配布物を分割してスイートを駆動する NIST 自身の COBOL エグゼクティブで、ここでは Rust の実行機構に置き換えられているため、コンパイルの必要がありません。 |

**オブジェクト指向 COBOL** も RustCOBOL の対象外ですが、CCVS85 はそれより完全に前の
時代のものです — スイートに OO プログラムは 1 本もありません。

### 残っているもの

採点対象のモジュールについては何もありません。両方の軸が閉じ、失敗するアサーションも
ありません。残っているのは欠陥のリストではなく 3 つの恒久的な裁定です — DB の実行軸、
SG、`*301M` の標示メンバー — それぞれ理由とともに上に記録されており、加えて個別に
説明されていない 21 件の DELETED ケースがあります。

実行機構は、どんな退行の背後にある失敗の詳細も、モジュール全体で仕分けできる形で
印字します：

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> `FAIL*` の詳細行は意図的に**2 回**書かれます — CCVS の `PRINT-DETAIL` が
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` を実行するためです — 一方
> `PASS ` は 1 回です。印字ファイルから取った生の標識数は、意味を持つ前に失敗を
> 半分にしなければなりません。

### 適合性の履歴

コンパイル軸、その時点の対象内分母に対して。SG が対象外と裁定されたとき、および
DB205A が CM の下で再採点されたときに分母自体が動いたので、初期の行は 434 分の、
締めの行は 420 分のです。

| バージョン | コンパイル | 何が変わったか |
|---|---:|---|
| 1.62.7 | **0** / 434 | 何もコンパイルされませんでした。古典的な基準形式の 2 つの規則が欠けていました。カラム 73‑80 がソースとして読まれ、継続行が決して結合されませんでした。 |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — 継続を含む古典的な基準形式。[ソース形式](#ソース形式)を参照。 |
| 1.62.13 | 292 / 434 | 区切りのコンマとセミコロンはトークンではなく句読点。添字は空白だけで区切れる。リテラル内の二重化した区切り文字は 1 文字。診断の 3 つの区分がまるごと空になりました。 |
| 1.62.14 | 317 / 434 | 組込み関数の引数としての表全体。`CLOSE … WITH LOCK` / `NO REWIND` / `REEL`。**組込み関数がコンパイルで 45 / 45。** |
| 1.62.16 | 376 / 434 | `AT END` の `AT` は任意なので、単独の `END` 句が次の段落見出しを飲み込まなくなりました（33 本）。**索引入出力がコンパイルで 42 / 42。** |
| 1.62.21 | 417 / 434 | 中核の一巡 — `ALTER` 系列、添字付き条件名、省略した組合せ関係、オペランド間にまたがる `INSPECT` の範疇。中核が 76 → 92 本コンパイル。 |
| **1.62.42** | 420 / 434 | **中核が両軸で完了** — 95 / 95 がコンパイル*かつ*きれいに実行、4,614 のアサーションで失敗なし。 |
| **1.62.43** | 422 / 434 | 順次入出力が完全にコンパイル、85 / 85。実行は 85 本中 10 → 44 へ。declarative の段落が名前を保つので `USE` ハンドラが `PERFORM` と `GO TO` をかけられるようになり、20 本が落ちなくなりました。 |
| **1.62.47** | — | **順次入出力が完了** — 両軸で 85 / 85。最後の欠落は `XXXXD001`、CCVS85 の*インストール*が用意し、どのメンバーも書かないデータファイルでした。今は実行機構が置きます。 |
| **1.62.76** | — | **RELATIVE エンジン**が到着（`cobolt-runtime/src/relative.rs`、コンテナ `PRCREL1`）。7 つのファイル動詞すべてが `FileOrganization::Relative` で振り分けられます。 |
| **1.62.77** | — | **相対入出力が完了**（34 / 34）、14 / 35 の基準値から 1 セッションで。そして**索引入出力も完了** — 相対エンジンが IX106A の最後の 4 件の失敗を閉じました。それは順次・索引と並べて同プログラムが扱う相対ファイルだったのです。 |
| **1.62.81** | — | **組込み関数が実行で完了**、45 / 45、基準値 24 / 45 から。原因は 5 つ、いずれもモジュール本来の主題ではありませんでした。字句解析器の区切り規則が 2 度、標示の欠落、`NUMVAL` の引数文法、引数リストの比較、そして除算のあふれ経路です。 |
| **1.62.107** | — | **プログラム間通信が完了**、25 / 25。 |
| **1.62.119** | — | **Sort / Merge が完了**、39 / 39、735 PASS。最後の一歩は SORT/MERGE における `[COLLATING] SEQUENCE [IS] alphabet-name` で、英数字キーを `SPECIAL-NAMES` で名付けた字母順に並べます。 |
| **1.62.127** | — | **原始テキスト操作が完了**、16 / 16。文字列リテラルのオペランドは引用符を保ち、識別子のオペランドは `IN`/`OF` の連鎖と添字にまたがり、対は置換を再走査せず 1 回の走査で適用されます。 |
| **1.62.129** | **420 / 420** | **コンパイル調査が 100 % で締まりました。** DB205A は裁定により CM の下で採点され、対象内スイートは 420 本になります。 |

> **正直な要約。** 対象内のすべてのプログラムがコンパイルされ、採点対象のすべての
> プログラムがきれいに走ります：**コンパイル 420 / 420、実行 380 / 380、8,362 の
> アサーションで失敗なし。** それらの行の最初から 9 リリース前、コンパイルの数字は
> ゼロでした。まだ開いているものは、百分率の中に隠されるのではなく裁定として上に
> 明記されています — DB の実行軸、SG、`*301M` の標示メンバー、そして個別に説明されて
> いない 21 件の DELETED ケースです。

---

> **更新（欠落実装の一巡）：** 以下が実装され、現在 ✅ です — **参照修飾**
> `id(start:len)`、**行内 `PERFORM n TIMES`**、**`SET … UP/DOWN BY`**、
> **STRING/UNSTRING の `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**、
> **範疇を意識した `INITIALIZE`**、**演算子を前置した省略条件**（`a > 1 AND < 9`）、
> **`CALL … ON EXCEPTION`**（未解決の CALL で実行）、**`COMPUTE` の複数受取り側 +
> 受取り側ごとの `ROUNDED`**、そして大幅に拡張された**組込み関数**群。
>
> **更新（階層的／出現を意識した環境の一巡 — 1.5.0）：** データモデルに阻まれていた
> 4 つの機能が ✅ になりました — **実行時の表添字** `t(i)` / `t(i, j)`（出現ごとの
> 記憶域）、**修飾名の曖昧性解消** `id OF/IN group`（重複する末端名が独立した記憶域に
> 解決）、**`MOVE/ADD/SUBTRACT CORRESPONDING`**、および**機能する `SEARCH` /
> `SEARCH ALL`**。
>
> **更新（動詞の完全性の一巡 — 1.6.0）：** さらに ✅ — `ADD`/`SUBTRACT` における
> **複数受取り側の `MULTIPLY`/`DIVIDE GIVING` + 受取り側ごとの `ROUNDED`**、
> **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** および修正された
> 単独の `EXIT`、**`CALL … NOT ON EXCEPTION`**、
> **`INSPECT … TALLYING … REPLACING`** の併用と **`BEFORE/AFTER INITIAL`** の領域、
> 日付／財務の**組込み関数**（`INTEGER-OF-DATE`、`DATE-OF-INTEGER`、
> `INTEGER-OF-DAY`、`DAY-OF-INTEGER`、`ANNUITY`、`FRACTION-PART`）、
> **リテラルを対象とする省略条件**（`A = 1 OR 2 OR 3`）、**`EVALUATE … ALSO`**
> （複数主語）と **`WHEN NOT`**、**本物の 88 レベル条件名**（`SET … TO TRUE/FALSE`、
> 宿主項目をその VALUE／範囲に対して検査）、**`PERFORM para VARYING`**、そして
> 機能する **`SORT`/`MERGE`** ランタイム（`RELEASE`/`RETURN`、`USING`/`GIVING`、
> `INPUT`/`OUTPUT PROCEDURE`）。末尾の回避リストは最新です。
>
> **更新（回避リスト解消の一巡 — 1.7.0）：** 残っていた欠落が実装されました —
> **識別子を対象とする省略**（`a = b OR c`、88 レベルのメタデータで解決）、
> **`INITIALIZE … REPLACING category DATA BY value`**、**`66 RENAMES`**（読取りが
> 合成し、書込みが対象項目へ分配）、**ポインタ**（`USAGE POINTER`、
> `SET ptr TO ADDRESS OF x / NULL`、別名付けの `SET ADDRESS OF item TO …`、
> `IF ptr = NULL`）、**`ALTER`** / **`UNLOCK`**、忠実な **`NEXT SENTENCE`**、
> 残りの標準**組込み関数**（`PRESENT-VALUE`、`YEAR-TO-YYYY`、`BYTE-LENGTH`、
> `NUMVAL-F`、`TEST-NUMVAL`）、そして拡張された画面 **`ACCEPT`/`DISPLAY`**
> （CLI モードでは ANSI 経由の `AT`/`WITH` — 解析されるだけでなく*実行*されます）。
>
> **更新（1.7.1）：** `ACCEPT` のレジスタ取得元が機能するようになりました（認識される
> だけの no‑op でした） — **`FROM COMMAND-LINE`**、**`ARGUMENT-NUMBER`** /
> **`ARGUMENT-VALUE`**（`DISPLAY n UPON ARGUMENT-NUMBER` と対）、
> **`ENVIRONMENT-VALUE`**（`DISPLAY "name" UPON ENVIRONMENT-NAME` と対）、
> **`ESCAPE KEY`** → `"00"`、**`CRT STATUS`** → `"0000"`。
>
> **更新（1.7.2）：** ファイル共有／ロックの句と `CANCEL`（❌ / no‑op でした） —
> **`OPEN … SHARING WITH … [WITH LOCK]`**、**`READ … WITH [NO] LOCK`**、
> **`UNLOCK`**（そのファイルの INDEXED レコードロックを解放）、および
> **`CANCEL program`**（プログラムの記憶域を再初期化）。
>
> **更新（1.8.0）：** **`COMMIT` / `ROLLBACK`** が本物の COBOL 動詞になりました —
> 開いている INDEXED ファイル（メモリ／ディスク両エンジン）に対するプログラム制御の
> トランザクションです。ディスクエンジンは実行中の本物の取消しログを備えました
> （以前は no‑op でした）。末尾の回避リストは最新です。

---

## IDENTIFICATION DIVISION の段落

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ **注記項目**の段落 — `AUTHOR`、`INSTALLATION`、`DATE‑WRITTEN`、
  `DATE‑COMPILED`、`SECURITY` — **任意の順序、任意の部分集合**で。
- ✅ `REMARKS` も受け付けます。1985 年に COBOL から削除されたため保存はされません。
  COBOL‑74 から引き継いだソースが今もコンパイルできるように受け付けています。

**注記項目**は自由テキストであり、COBOL‑85 はそれを文字どおりの意味で言っています：

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- **予約語**を含めてもかまいません — 上の `DATA` は DATA DIVISION を開始しません。
- **ピリオド**を含めてもよく、そこで終わりません。
- 書いただけの**行数にわたって続きます**。
- A 領域で**行頭から始まる**次の段落見出しまたは division 見出しで終わります —
  上の項目が `DATE-WRITTEN` で終わるのはそのためです。

**その文中の引用符はその行の中に封じ込められます**（1.62.12 以降）。
`THE COMPILER"S ABILITY` のような文字列が、プログラムの残り全体へ流れ込むリテラルを
開くことはもうありません — [ソース形式](#ソース形式)を参照してください。
注記項目で対になっていない引用符を避けるのは今も得ですが、失うのはその行だけで、
ファイル全体ではありません。

⚠️ `INSTALLATION`、`SECURITY`、`REMARKS` はここでは**予約語ではありません**。
IDENTIFICATION DIVISION の内部でのみ段落名として認識されるので、
`SECURITY` という名前のデータ項目は引き続き使えます。

---

## ソース形式

RustCOBOL は 3 つのソース配置を読みます。選択は明示的で、ファイルの内容から
**推測されることはありません**。カラム規則を、それ向けに書かれていないソースへ
適用すると、コードが黙って消えるからです。

| `--source-format` | 意味 |
|---|---|
| `free` | カラム規則は一切なし。`*>` がコメントを開始します。**既定値**であり、PowerRustCOBOL 自身のプロジェクトと生成されるフォーム `.cbl` ファイルが使うものです。 |
| `fixed` | ✅ **COBOL-85 の古典的な基準形式** — 標準が定義し、カード・イメージのソースが書かれている配置です。下記参照。 |
| `fixed-relaxed` | 一連番号領域と標識カラムは尊重されますが、行は打ち込んだところまで続きます — 72 カラムの制限はありません。 |
| `auto` | 従来の動作：`COBOLT_FIXED=1` でなければ `free`。 |

`COBOLT_SOURCE_FORMAT` はセッションの既定値を設定します。

### `fixed` — 古典的な基準形式

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **カラム 1-6** — 一連番号領域、無視されます。
- **カラム 7** — 標識領域：
  - `*` または `/` → コメント行
  - `-` → 直前の行の**継続**
  - `D` → デバッグ行。コメント扱い（デバッグモードはまだ未実装）
  - それ以外 → 通常のソースとして読まれます。標準はこのカラムを予約していますが、
    カード・イメージのスイートは任意行の選択子として使うので、それらの行を黙って
    落とすとコードが消えてしまいます。
- **カラム 8-72** — ソース。
- **カラム 73-80** — 識別領域、**破棄されます**。

### 継続行 ✅

カラム 7 のハイフンが直前の行を継続します。

**語または数字リテラルの継続** — 継続される行の末尾の空白は破棄され、
2 つの半分が間に何も挟まずにつながります：

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

これは `WRK-DS-18V00-CONTINUED` という名前の項目 1 つを宣言します。

**英数字リテラルの継続** — 継続される行のリテラルには閉じ引用符がありません。
継続行は引用符で再開しなければならず、リテラルはその次の文字から再開します：

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **継続された断片は末尾の空白を含めてカラム 72 まで続きます。** カラム 72 より
手前で終わる行も、それらの空白をリテラルに寄与します。継続リテラルがバイト単位で
正確なのが `fixed` の下だけである理由はこれです。他の形式には止まるべきカラム 72 が
ありません。

### リテラルが偶然に行をまたぐことはありません ✅

継続はリテラルが行を越える**唯一の**手段です。自分の行で閉じられていない引用符は
エラーであり、書かれた位置で報告されます：

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

これは聞こえるより重要です。1.62.12 より前は、対になっていない引用符はファイル中
どこであれ*次の*引用符まで届いたので、コメント中の迷子の `"` 1 つが division を丸ごと
飲み込み、それ以降のすべての引用符の対応をずらしました — これが見つかった NIST の
プログラムは引用符の数が**偶数**なので、未終端になるものは何もありませんでした。
たった 1 文字がファイル全体のパリティをずらしていたのです。今では被害は改行で止まります。

> **自由形式にはリテラルの継続がありません。** `&` でもなく — それは連結*演算子*です
> — 囲みブロックでもありません。自由形式のリテラルは 1 行に収まらなければなりません。
> 長いものは連結してください：`"first part" & "second part"`。

> **注意。** 自由形式で書かれたファイルに `fixed` を選ぶと、そのファイルは壊れます —
> カラム 72 を超えるものは消え、カラム 8 より前のテキストは一連番号として読まれます。
> 本当にカード・イメージであるソースにのみ指定してください。

---

## 認識される文（動詞）

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2`（para-1 の `GO TO` を差し替えます）·
`UNLOCK file`（そのファイルのレコードロックを解放します）·
`OPEN … SHARING/WITH LOCK` · `READ … WITH [NO] LOCK`
（ファイル共有／ロック — 単一実行単位の中では助言的）
✅ `COMMIT` / `ROLLBACK`（プログラム制御の INDEXED ファイルトランザクション —
ファイル動詞を参照）· `CANCEL`（プログラムの記憶域を再初期化します）·
✅ `INVOKE` — GUI／ランタイムのオブジェクト（ウィンドウ、フォーム、コントロールの
メソッド）を操作します。何もしないのは **COBOL** のオブジェクトに対してだけで、
クラス／メソッド定義は対象外だからです。
プロジェクト拡張：`EXEC RUST … END-EXEC`、`TRY/CATCH/FINALLY/END-TRY`、`THROW`。
ブロックは常にリンクされる crate（std、egui、eframe、およびリンクされたランタイム
一式）に加えて、**プロジェクトが Project の Crates に登録した任意の crate**（spec
044）を `use` できます。登録された crate は正確なバージョンに固定され、プロジェクトの
`crates/` に取り込まれ、バイナリにコンパイルされます。未登録の crate は開発者の行で
Check/Build を失敗させ、対処法を名指しします。

✅ `SEARCH`（逐次）/ `SEARCH ALL`（`ASCENDING`/`DESCENDING KEY` を持つ表に対する
二分探索 — 最初に一致した `WHEN` を実行し、なければ `AT END`）。
✅ `SORT` / `MERGE` と `RELEASE` / `RETURN`（機能します — 下記参照）。
✅ `DECLARATIVES … END DECLARATIVES` と `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — 処理されないエラー `FILE STATUS` で
発火するファイルエラーハンドラ。ハンドラは**その section の先頭から入り、section の
終わりまで実行されます**。段落は名前を保つので、`PERFORM` や `GO TO` の対象にできます
— *別の* declarative section の段落も含みます。declarative の段落は独自の名前空間に
あります。制御が本体からそこへ落ちることはなく、両方で宣言された名前は、ハンドラの
実行中は declarative 側の複製に、それ以外のどこでも本体側に解決します。declarative は
非 declarative 部分の段落を `PERFORM` することもできます。
❌ **認識されません — 使わないでください：** `ENTRY`、
`GENERATE`/`INITIATE`/`TERMINATE`、`SEND`/`RECEIVE`、`ENABLE`/`DISABLE`。

---

## 動詞ごとの対応形式

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]`（複数受取り側）。
- ✅ **集団オペランドが 1 つあると転記全体が英数字になります**（COBOL-85 6.18.4）。
  もう一方のオペランドの PICTURE は*大きさ*だけを寄与し、それ以外は何も寄与しません。
  編集も逆編集も数値変換もありません。`MOVE <"123ABC" を保持する集団項目>` は
  `PIC 0XXXXX0` に `"123ABC "` を残し（編集済みの `"0123AB0"` ではありません）、
  `PIC 9999V999` には同じ 6 文字と空白を、`PIC 99` には `"12"` を残します。
  `JUSTIFIED RIGHT` はどちらの端が詰められどちらの端が失われるかを今も決めます。
  同じ規則が集団項目自身のバイトを支配します。各子項目が自分の区画をそのまま取るので、
  英数字編集の子項目が**再度編集されることはありません**。
- ✅ **集団項目に付けた `VALUE` 句**は集団のバイトを初期化し、子項目へ分配されます —
  `01 G VALUE "$123.45". 02 E PIC $999.99.` は `E` に `"$123.45"` を残します。
- ✅ `MOVE CORRESPONDING g1 TO g2` — 2 つの集団が名前で共有する従属項目それぞれを
  転記し、一致する下位集団へ再帰します。
- ✅ **`CORRESPONDING` は `REDEFINES` または `RENAMES` で記述された項目を除外します**
  （COBOL-85 6.18.4 GR1）。どちらの側でも、そしてそれに従属するすべてとともに。
  除外は*宣言*に対してであって名前に対してではありません。別の場所の 66 レベルと
  名前を共有しているだけの普通の項目は、今も対応します。
- ✅ **`CORRESPONDING` のどちらのオペランドも、集団の表の 1 出現を指名できます** —
  `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` はその出現自身の枠に書き込み、添字は
  再帰を通して引き継がれます。
- ✅ **対には 2 項目のうち 1 つが基本項目であれば足ります。** 集団項目が基本項目と
  向き合ってよく、その間の転記は英数字のものになります。基本項目の `PIC XXX` が
  `999` + `XXX` の集団へ送ればその 6 文字を埋め、`XXX` + `99` の集団が素の `X(5)` へ
  送ればそれを埋めます。集団同士が向き合う場合は今も**再帰します** — その組合せは
  基本項目の場合ではありません。*（1.62.39 より前はどちらの向きでも何も転記されませんでした。
  集団項目は記憶域の枠を持たないので、書込みは誰も読み返さない場所へ行き、読取りは
  空文字列を返していました。）*
- ✅ **参照修飾 `id(start:len)`** — 送り出し側（部分列）と受取り側（継ぎ合わせる
  部分代入）。すべての動詞のオペランドで機能します。`length` は任意です。
  **文字位置**を指すので、数値オペランドはその `PIC` の全幅と先行ゼロのまま取られます。
  `01 T PIC 9(8) VALUE 00224845` では `T(1:2)` は `"00"` であり `"22"` ではありません。
- ✅ **集団項目は英数字の集合体です** — 集団項目*は*その従属項目を端から端まで並べた
  ものであり、その大きさは従属項目の大きさの和です。集団項目を読むと子項目が連結され
  （`FILLER` も含みます）、集団項目へ転記すると幅に応じてバイトが子項目へ分配されます。
  `MOVE 11 TO A` は `A` を含む集団項目を通して見え、`MOVE "1234" TO G` は `G` 自身の
  枠ではなく `G` の子項目を設定します。
- ✅ 添字 `t(i)`、`t(i, j)` — 出現ごとの記憶域の枠を読み書きします。可変添字
  `t(WS-I)` はアクセスごとに評価されます。
- ✅ 修飾 `id OF/IN group`（`… OF g1 OF g2`） — 末端名が 2 つ以上の集団の下で宣言されて
  いても正しい項目に解決します。

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`。
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`。
- ✅ **受取り側ごとの `ROUNDED`** — 各受取り側が自分の `ROUNDED` 指定を持ちます。
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — 一致する数値の対それぞれを
  演算し、一致する下位集団へ再帰します。

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`。
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`。
- ✅ **複数の `GIVING` 受取り側**、それぞれが自分の `ROUNDED` を持ちます。
- ⚠️ `DIVIDE a BY b`（`GIVING` なし）は `a/b` を `a` に戻して格納します
  （PowerRustCOBOL の便宜。標準 COBOL はここで `INTO` か `GIVING` を要求します）。

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **複数受取り側、それぞれが自分の `ROUNDED` を持ちます**。
- ✅ 式の演算子 `+ - * /` と `**`（べき乗、右結合）、括弧、`FUNCTION name(args)`。

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`。
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`。
- ✅ **`ALSO` による複数主語** — 各 `WHEN` の列が位置に応じて対応する主語と照合され、
  AND で結合されます。
- ✅ **`WHEN NOT value`** は選択対象を否定します。**`WHEN condition`**
  （例：`EVALUATE TRUE WHEN a > b`）は論理条件を評価します。

### PERFORM
- ✅ `PERFORM p [THRU p2]`。
- ✅ `PERFORM p [THRU p2] n TIMES`（n は整数リテラルまたはデータ項目）。
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`。
- ✅ 行内の `PERFORM UNTIL cond … END-PERFORM`、
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`。
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`。
- ✅ 行内の `PERFORM n TIMES … END-PERFORM`（段落なし）。
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — 反復ごとに段落を実行します
  （行外、`END-PERFORM` なし）。
- ✅ **`WITH TEST AFTER` は `VARYING` にも適用されます**。句のどちら側に書いても、
  行内でも行外でもかまいません。本体は何も検査されないうちに 1 回実行され、その後
  条件が**内側から先に**検査されます。条件が偽である水準が増分され、その内側の
  すべての水準は自分の `FROM` 値に戻り、本体が再び実行されます。変数は自分の検査が
  偽と出たときにだけ増分されるので、ループを終わらせた検査はそれを本体が残したままに
  します。
- ✅ **`AFTER` 変数は自分のループが終わるとき `FROM` 値に戻されます**。1 つ外側の
  水準が増分される前に（COBOL-85 6.20.4 GR10(d)）。`PERFORM` 全体が終わった後、
  内側の変数は自分の `FROM` 値を読み、それを終わらせた値を保持するのは最も外側だけ
  です。
- ✅ **添字付きの `VARYING` 識別子は自分の添字に従います。**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` はその時点で
  `S1` が選ぶ出現を増分するので、`S1` を進める本体は表を歩きます。

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`。
- ✅ **`{OF|IN} section` 修飾子がどの複製を意図しているかを選びます** — 段落名が
  section をまたいで繰り返される場合に、`PERFORM` とまったく同じように働きます。
  **未知の** section の場合は、飛び先を失うのではなく修飾なしの検索に戻ります。
  `GO TO … DEPENDING ON` は名前の素のリストを取り修飾子は取りません。`ALTER` が
  差し替えた `GO TO` はその差し替えに従います — それは自分の飛び先を明示的に指名して
  います。*（1.62.39 より前は修飾子は解析された後に無視されていたので、飛び先は
  プログラム中どこであれ最初の定義に着地していました。）*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`。
- ✅ 単独の `EXIT` は何もしない戻り点です。`EXIT PROGRAM` は呼び出し側へ戻ります。
- ✅ `EXIT PERFORM [CYCLE]`（最も近い行内 PERFORM を抜ける／次の周回へ）、
  `EXIT PARAGRAPH`、`EXIT SECTION`。
- ✅ `NEXT SENTENCE` — 次の文の境界を越えて制御を移します（構文解析器が各ピリオドに
  境界標識を挿入します。単なる `CONTINUE` ではなく忠実な実装です）。

### ACCEPT
- ✅ `ACCEPT id`。
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`。
- ✅ **`SPECIAL-NAMES` がその呼び名を宣言している場合、`FROM mnemonic-name` は
  操作員から読みます**（`XXXXX057 IS ACCEPT-INPUT-DEVICE.` …
  `ACCEPT ACCEPT-D1 FROM ACCEPT-INPUT-DEVICE`） — それが形式 1 であり、素の
  `ACCEPT id` と同一です。**どの `SPECIAL-NAMES` 句も宣言していない**名前は
  PowerRustCOBOL の拡張を保ち、その名前の**環境変数**を読みます。どちらが適用されるかは
  宣言が決め、綴りが決めることは決してありません。*（1.62.35 より前は通常の
  `<implementor-name> IS <mnemonic>` 句がまるごと飛ばされていたので、すべての呼び名が
  設定されていない環境変数を読み、受取り項目は空のままでした。）*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` はカーソルを位置付けます（ANSI、CLI）。
- ✅ `FROM COMMAND-LINE`（コマンド行全体）· `FROM ARGUMENT-NUMBER`（引数の個数）·
  `FROM ARGUMENT-VALUE`（`DISPLAY n UPON ARGUMENT-NUMBER` で設定した位置の引数）·
  `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE`
  （`DISPLAY "name" UPON ENVIRONMENT-NAME` で指名した変数）· `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`。
- ✅ `END-ACCEPT` が文を閉じます（任意）。

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`。
- ✅ `END-DISPLAY` がオペランドのリストを閉じます（任意）。よって
  `DISPLAY A END-DISPLAY DISPLAY B` は 1 文ではなく 2 文です。
- ✅ 画面形式 `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — **CLI モード**（`rcrun`）では
  ANSI のカーソル位置付け + SGR で実行されます。GUI モードでは無視されます
  （そこでは Form Designer が SCREEN 入出力に取って代わります）。`ACCEPT id AT …` は
  位置付けてから読みます。

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`。
  あふれ = 組み立てた文字列が受取り項目より広いこと。
- ✅ **`DELIMITED BY` 句は、その前にある送り出し側の並び全体を支配します**。
  直後に書かれた 1 つだけではありません。
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` は 3 つすべてを区切り `"ABC"` を
  組み立てます。1 つの文が複数の句を持ってよく、各句は前の句からの送り出し側を
  支配します。最後の句より後の送り出し側はそれぞれ全体が取られます。
  *（1.62.40 より前は句の直前に書かれた送り出し側だけが区切られていました。）*
- ✅ **集団項目への `INTO`** はその集団の従属項目へ分配します。
- ✅ **結果はバイト単位で組み立てられる**ので、`STRING HIGH-VALUE` は単一バイト
  `0xFF` を転記し、1 文字位置を占めます。
- ✅ **拡張 — 賢い既定の `DELIMITED BY`**（どの句もそのオペランドを支配していない
  場合）：英数字の `PIC X`/`A` 項目は既定で `SPACES`（末尾の詰めは落とされます）。
  文字列リテラル、数値項目、数字編集項目、`FUNCTION` の結果、および式は既定で `SIZE`
  です。データ項目は自分の項目としての形で転記されます（数値 → PIC の全幅の数字、
  数字編集 → 編集された文字）。

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`。あふれ = 受取り側より送り出し側の項目が
  多いこと。

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`。
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`。
- ✅ `INSPECT … TALLYING … REPLACING …` — **両方の半分が適用されます**。
- ✅ `BEFORE/AFTER INITIAL` は各句を項目の部分領域に限定します。
  （TALLYING は COBOL に従い計数器へ積み上げます。）
- ✅ **TALLYING オペランドの並びは左から右への走査を 1 回だけ共有します**
  （COBOL-85 6.17.3）。各文字位置でオペランドは書かれた順に試され、最初に一致した
  ものがその位置を取り、走査は消費した文字の先から再開します。よって
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` を `"AABA"` に対して行うと
  `t1 = 1, t2 = 1` になります — オペランドを逆順に書くと `t1 = 3, t2 = 0` です。
  `LEADING` は自分の窓の左端から隙間なく一致しなければならないので、先行する
  オペランドがその位置を取ると連なりは始まる前に終わり、`CHARACTERS` は先行する
  どのオペランドも取らなかった位置だけを数えます。
- ✅ **REPLACING オペランドの並びも走査を 1 回だけ共有します**。同じ規則によります。
  ある位置で最初に一致したオペランドがその文字を置き換え、走査はその先から再開する
  ので、後続のどのオペランドもそれを見ることはできません。各オペランドの
  `BEFORE`/`AFTER` の窓は**どの置換よりも前に**確定します。これが、先行するオペランドが
  上書きする文字にあるオペランドを固定できる理由です：

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  1 オペランドずつ適用したなら、最初の句が 2 番目の句の固定先である `"L "` を消して
  しまい、`"BAD"` が生き残っていたでしょう。
- ✅ **符号付き DISPLAY 項目の文字位置に `-` はありません。** 作用符号は数字への
  重ね打ちなので、`INSPECT <-12345 を保持する PIC S9(5)> TALLYING c FOR ALL "-"` は
  **0** を与え、`FOR ALL "5"` は 1 を与えます。符号は後で復元されるので、数字に対する
  `REPLACING` は符号に手を触れません。`SIGN IS … SEPARATE CHARACTER` は符号が*位置で
  ある*場合であり、そのときは数えられます。

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}`（MOVE にコンパイルされます）。
- ✅ `SET idx {UP|DOWN} BY n`（ADD / SUBTRACT として符号化されます）。
- ✅ `SET 88-name TO TRUE` は宿主項目を条件の最初の VALUE に設定します。`TO FALSE` は
  VALUE の集合の外の値に設定します（最善努力 — FALSE 句はありません）。
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` と
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — 下の**ポインタ**を参照。

### INITIALIZE
- ✅ `INITIALIZE id …` — 範疇を意識します：数値／数字編集 → ZERO、それ以外すべて →
  SPACES、集団項目へは再帰します。
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — その範疇の従属項目
  それぞれをその値に設定します。他は手を触れません。

### ポインタ（USAGE POINTER）
- ✅ `USAGE POINTER` がポインタを宣言します（初期値は NULL）。
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`。
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — `id` を対象の記憶域への
  別名にします（読取り**と**書込みの両方が別名に従います）。通常は LINKAGE のレコード
  です。`IF ptr = NULL` は機能します。

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`。
- ✅ `ON EXCEPTION` / `ON OVERFLOW` の本体は呼び出されたプログラムが解決できないときに
  実行されます。`NOT ON EXCEPTION` の本体は呼び出しが**解決したとき**に実行されます。
- ✅ `CANCEL program …` は指名したプログラムの WORKING-STORAGE を再初期化するので、
  次の `CALL` は新規の状態から始まります。

### ファイル動詞（対応している句 — 完全な網羅はファイル入出力スイートにあります）
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`；`CLOSE f …`。
  （`SHARING` / `WITH LOCK` は解析され、意味を持つところでは尊重されます —
  単一実行単位のモデルでは助言的です。）
- ✅ **1 つの `OPEN` が複数のモード群を持てます**。それぞれが自分のファイルを持ちます：
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` 各群はそれ自身のモードで開かれます。
  `SHARING` / `WITH LOCK` / `REGISTERED USER` は文全体に適用されます。
- ✅ **すでに開いているファイルの `OPEN` は `41`** であり、ファイルはそのまま残されます
  — 文はそれを**開き直しません**。（`OUTPUT` のファイルを開き直せば、プログラムが
  すでに書いたものを黙って切り捨ててしまいます。）
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`**（PowerRustCOBOL の拡張）
  — 操作員／利用者を INDEXED の可観測性ログに記録します（そのファイルのセッションの
  全イベント行に `user=` 項目が付きます）。純粋に観測用であり、認証／認可はありません。
  [`observability-jp.md`](observability-jp.md) §1.3.1 を参照。
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`。
  `WITH NO LOCK` は I‑O のもとで INDEXED エンジンが取るレコードロックを解放します。
- ✅ **`READ … INTO id` は `READ` に集団 `MOVE` が続いたものです。** レコードは
  受取り側の従属項目へ幅に応じて分配され、受取り側自身の幅で切られます。受取り側には
  添字を付けてよく、転記はバイトを運ぶので、文字でないバイトを保持するレコードも
  そのまま届きます。
- ✅ **FD の `RECORD` 句 — 可変長レコード。** 3 つの書き方すべて：
  `RECORD CONTAINS n CHARACTERS`（固定）、`RECORD CONTAINS n TO m CHARACTERS`
  （可変。`WRITE` が指名するレコード記述が長さを与えます）、そして
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  （そのデータ項目*が*長さです — `WRITE` の前に設定し、`READ` が設定し戻し、宣言した
  範囲に収められます）。`01` レコードの大きさが異なる FD は、そう書いてあるかどうかに
  関わらず可変長です。可変長ファイルは各レコードの長さをレコードとともに格納するので、
  そのバイトは固定長ファイルのものと**交換できません**。固定長ファイルは変わりません。
- ✅ **FD の `01` レコード群は 1 つのレコード領域を記述します。** `READ` はすべての
  レコード記述を通してバイトを届けます。`WRITE` は領域全体を送るので、書かれた記述が
  `FILLER` を置いている場所に別のレコード記述が置いたものが透けて見えます。
- ✅ **`FILLER` は FD のレコードで自分のバイトを占めます**。そして
  `SIGN IS SEPARATE CHARACTER` は符号付き DISPLAY 項目を、その数字位置より 1 文字
  広くします。
- ✅ **FD の `LINAGE` は整数だけでなくデータ名も取ります** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`。ページは各
  `WRITE` の時点でそれらの項目から測られるので、プログラムは実行中にページの大きさを
  変えられます。ファイルを開いた時点で `LINAGE-COUNTER` は 1 です。
- ✅ **`AT END` の後の順次 `READ` は `46` であり、2 度目の `10` ではありません。**
  `AT END` は有効な次のレコードを残さなかったので、読み続けるのは終端に達することとは
  別の誤りです。`46` は 4 類の状態なので、`AT END` も `NOT AT END` もそれに対しては
  実行されません — それを処理するのはそのファイルの `USE` declarative です。
  新たな `OPEN`、または成功した `START` が、レコードを再び確立します。
- ✅ `UNLOCK f [RECORD[S]]` はそのファイルのレコードロックを解放します。
- ✅ **`COMMIT` / `ROLLBACK`** — 開いている**すべての** INDEXED ファイルに対する
  プログラム制御のトランザクション。`OPEN` がトランザクションを開始し、`COMMIT` は
  保留中の `WRITE`/`REWRITE`/`DELETE` を確定して（後の `ROLLBACK` ではもう取り消せません）
  新しいトランザクションを開始します。`ROLLBACK` は最後の `COMMIT`/`OPEN` 以降の変更を
  すべて取り消します。**DISK** 記憶では `COMMIT`/`CLOSE` がディスク上で永続化されます。
  **MEMORY** 記憶では `COMMIT`/`ROLLBACK` は純粋に RAM 内で行われます（ディスクへは
  決して書きません）。素の `STORAGE IS MEMORY` のファイルは一時的で、
  `STORAGE IS MEMORY WITH PERSISTENCE` は `CLOSE` のときだけディスクへ保存します。
  （永続的な先行書込みログによるクラッシュ回復は今後の課題です — これは実行中の、
  プログラム水準のロールバックです。）
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`**（INDEXED ファイル。PowerRustCOBOL の拡張）。既定の記憶は `DISK` です。
  `WITH COMPRESSION` は格納するレコードを圧縮します（キーは圧縮前のレコードで
  評価されます）。`WITH PERSISTENCE`（MEMORY のみ）は RAM 上のファイルを `CLOSE` で
  保存します。`OPEN OUTPUT` は常にディスク上のコンテナを（再）作成します。
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`。
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`；
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`。
- ✅ **レコード順次ファイルに対する `REWRITE`** は、最後の `READ` が届けたレコードを
  その場で置き換え、読取り位置はそのままにします — 次の `READ` は今も後続のレコードを
  与えます。返すべき状態は、ファイルが `I-O` で開かれていないとき **`49`**、成功した
  `READ` がレコードを確立していないとき **`43`**（`AT END` の後、および間に `READ` を
  挟まない 2 度目の `REWRITE` を含みます）、そして新しいレコードが読んだものと同じ
  長さでないとき **`44`** です — `DEPENDING ON` のファイルではその項目の値がその長さ
  であり、プログラムが別の長さを求める手段はそれです。
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`。
- ⚠️ *プロセス*間のファイル共有は強制されません（単一実行単位）。`SHARING`/`LOCK` の
  句は解析され、INDEXED エンジンの実行内レコードロックは尊重されます。

### SORT / MERGE / RELEASE / RETURN  ✅（機能します。作業バッファはメモリ内）
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`。
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`。
- ✅ `RELEASE record [FROM id]`（INPUT PROCEDURE 内）が実行に追加します。
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` がレコードを返します。
- レコードは宣言したキー（`ASCENDING`/`DESCENDING`）で安定的に並べ替えられます。
  `USING` が指名した順次ファイルを読み、`GIVING` が書きます。

---

## 条件（IF / EVALUATE / PERFORM UNTIL）

- ✅ 関係記号：`=` `<>` `<` `>` `<=` `>=`。
- ✅ 語による関係：`[IS] [NOT] EQUAL TO`、`[IS] [NOT] GREATER [THAN] [OR EQUAL TO]`、
  `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`。
- ✅ 類：`id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`。
  PICTURE が**作用符号を持たない**項目が `NUMERIC` であるのは、すべての文字位置が数字を
  保持しているときだけです — `"+1234"`、`"1.234"`、`"12 45"` を保持する `PIC X(5)` は
  数値**ではありません**。*（1.62.40 より前はこの検査が文字を数値として解析していたので、
  符号、小数点、指数、前後の空白がすべて受け入れられていました。）*
- ✅ **利用者定義の `CLASS` のオペランドは序数位置でもかまいません** —
  `CLASS ORDINAL-A-ONLY IS 66` は固有文字集合の 66 番目の文字を指名します — そして
  そのオペランドは独立したソース行に置いてもかまいません。`ALPHABET` も同様です。
- ✅ 符号：`id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`。
- ✅ 88 レベルの条件名（素の名前を条件として）。
- ✅ **オペランドとしての `TRUE` / `FALSE`**（PowerRustCOBOL の拡張） — `1` と `0` の
  糖衣であり、値が許されるところならどこでも使えます：`IF x = TRUE`、
  `IF x IS [NOT] FALSE`、`IF x NOT TRUE`（関係演算子のない素の `NOT` 形式）、
  `PERFORM UNTIL x = FALSE`、`MOVE TRUE TO x`、`COMPUTE n = n + TRUE`、
  `INVOKE obj "m" USING TRUE`、そして値を主語とする `WHEN TRUE`。素の `TRUE`/`FALSE`
  は完全な条件にもなります（`IF TRUE`、`PERFORM UNTIL TRUE`）。
  ⚠️ これはこれらの語がすでに意味を持っていた 2 か所を**変えません**：
  `SET <88‑name> TO TRUE` は今も宿主項目を条件を満たす値に設定し（数値の 1 ではあり
  ません）、下の `EVALUATE TRUE`/`EVALUATE FALSE` は標準の場合分け文のままです。
- ✅ `AND` / `OR` / `NOT` の組合せ、括弧（AND は OR より強く結びます）。
- ✅ **演算子を前置した省略条件** — `a > 1 AND < 9`、`a = 5 OR = 7`
  （直前の比較の主語が再利用されます）。
- ✅ **リテラルを対象とする省略** — `a = 1 OR 2 OR 3`（主語と演算子の両方を再利用します。
  対象はリテラルです）。
- ✅ **識別子を対象とする省略** — `a = b OR c`（`c` はデータ項目）。比較に続く AND/OR の
  後の素の識別子は実行時に解決されます。既知の 88 レベル条件名ならそれとして評価され、
  そうでなければ対象 `a = c` です。（直後に `AND` が続く識別子は AND の優先順位を
  保ちます。）
- ✅ **省略の*対象*の前の `NOT` は関係を否定します**。対象を否定するのではありません：
  `a > b OR NOT c` は `a > b OR NOT (a > c)` です。`NOT <関係演算子>` の書き方
  （`AND NOT < x`）は演算子形式であり変わりません。そして通常の条件を開く `NOT` —
  `NOT (…)`、`NOT x = y`、`NOT x NUMERIC` — は自分の意味を保ちます。
  *（1.62.42 より前は対象形式が「対象が 0 でない」と読まれていました。これは対象が
  たまたま 0 を保持しているときにだけ同じ答えを与えます。）*
- ✅ **集団項目に宣言した条件名はその集団のバイトを検査します。** 集団項目は自分の
  記憶域を持たず — 子項目*そのもの*なので —
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` はレコードが保持する
  6 文字と比較します。
- ✅ **形象定数は相手のオペランドの大きさまで繰り返されます**。88 の `VALUE` として
  書いたものも含みます：`PIC X(4)` の宿主に対する `88 B VALUE QUOTE` は引用符 4 つで、
  `88 D VALUE ALL "BAC"` は `"BACB"` です。`ALL literal` は**両方向**に大きさが
  合わせられます — 10 文字の `X` に対する `IF X EQUAL TO ALL "BA"` は、空白で詰めた
  `"BA"` ではなく `"BABABABABA"` と比較します。

---

## 式・リテラル・USAGE

- ✅ 算術演算子 `+ - * /` と `**`；括弧；単項の `+`/`-`。
- ✅ `FUNCTION name ( arg [ , arg … ] )` — **実装済み**の組込み関数：
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM（種は任意）, CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`。
  （日付の変換は標準の基準 1601‑01‑01 = 1 日目を使います。）
  **COBOL‑85 標準の組込み関数一式**が実装されています。
- ✅ **日付と時刻のレジスタは LOCAL の時計を読みます。**
  `ACCEPT … FROM DATE / TIME / DAY / DAY-OF-WEEK` と `FUNCTION CURRENT-DATE` は
  いずれも UTC ではなく機械自身の時刻を報告します — 日付も含みます。日付は真夜中の
  前後で異なります。`CURRENT-DATE` の最後の 5 文字は GMT からの**実際の**差
  （`…-0300`）を運ぶので、プログラムはどの時間帯で動いているかを知ることができます。
  ✅ 認識されない `FUNCTION` 名は**コンパイルエラー**になり、その関数を名指しします。
  本物の関数が打ち間違いとして十分に近い場合は提案も出します。以前は解析が通り実行時に
  **0** を返していたので、綴りの誤りが自信たっぷりの誤答に化けていました（1.62.15）。
- ✅ リテラル：整数、小数、文字列、すべての形象定数
  （`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`、
  `ALL "x"`）。
- ✅ **形象定数は受取り側の全体を埋めます**。`HIGH-VALUE` も含みます —
  `MOVE HIGH-VALUE TO <PIC X(10)>` は `0xFF` のバイト 10 個であり、集団項目へは
  子項目に分配されます。英数字編集の受取り側は今も自分の挿入文字を置くので、
  `PIC XX0XXBXXX` は `FF FF '0' FF FF ' ' FF FF FF` を保持します。
  `PROGRAM COLLATING SEQUENCE` のもとでは、その定数は通常の 1 文字を指名し、
  代わりにその文字が埋めます。
  ⚠️ `HIGH-VALUE` は**バイト** `0xFF` であり、文字ではありません。集団オペランドの
  読取り、編集、およびすべての転記経路はこれをバイト単位で運びますが、
  **参照修飾はまだバイト単位で正確ではありません** — 本当に `0xFF` を保持している
  項目に対して `IF X (1:1) = HIGH-VALUE` は偽になります。
- ✅ **数字リテラルは小数点から始めてもかまいません** — `.5`、`-.5`、`.000000001`。
  COBOL‑85 はリテラルが小数点で*終わらない*ことだけを要求するので、`5.` は今も
  数値 5 に文の終端子が続いたものです。
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  先行ゼロは有意であり正確です：`.000000001` は 10 億分の 1 であり、10 分の 1 では
  ありません。`DECIMAL-POINT IS COMMA` のもとでは `,5` に同じことが当てはまります。
  リテラルを文末のピリオドと分けるのは**空白がないこと**です — COBOL‑85 は終端子の後に
  空白を要求するので、`MOVE X TO Y.` が小数の始まりと読まれることは決してなく、
  `MOVE X TO Y.5` は黙って読み替えられるのではなくコンパイルエラーになります。
- ✅ **適合性の標示**（`cobolt_semantic::flagging`） — 標準は、適合する実装が、
  プログラムが使っている機能のうちどれが選んだ適合水準の外にあるかをそのプログラムに
  伝えられることを求めています。2 つの解析がそれに答えます：
  - `flag_obsolete` — COBOL‑85 の**廃要素**の集合：IDENTIFICATION DIVISION の任意の
    5 段落、`MEMORY SIZE`、`ALTER`、リテラル付きの `STOP`、そして手続き名のない
    `GO TO`。
  - `flag_high_subset` — **上位部分集合**を超えるものすべて。`COMPUTE`、`EVALUATE`、
    `INITIALIZE` から `CORRESPONDING`、参照修飾、修飾、`SET … TO TRUE`、4 つ目の添字を
    経て、*語*または*数字リテラル*をカード境界をまたいで継続することまで。
    （**英数字**リテラルの継続は部分集合の中なので報告されません。）

  どちらも誤り検査ではなく、どちらも通常のビルドでは走りません。両者が名指しする
  構文はすべて、RustCOBOL が実装し実行する正当な COBOL‑85 です。通常のコンパイルが
  `AUTHOR` や `COMPUTE` について警告を出し始めないよう、あえて別の入口になっています。
  NIST の `NC302M`、`NC303M`、`NC401M` がこれらを検証します — 7、4、40 件の標示、
  すべて一致しています。
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — 編集 PICTURE の通貨位置を
  埋める文字です。`$` に加わるのではなく `$` を**置き換える**ので、プログラムが
  ひとたび宣言すれば、そこでは `$` はもう picture 文字ではありません：
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` はそのとき `      <.00` と読め、`MOVE 1234` は
  ` <1,234.00` と読めます — 浮動する連なりは `$$$,$$$.99` とまったく同じに振る舞います。
  **英字**の通貨記号も同じように働きます：`CURRENCY SIGN IS "W"` は `PICTURE WWWWW` を
  5 位置の浮動通貨文字列にするので、`MOVE 12` は `  W12` と読めます。
  *（1.62.40 より前は英字記号の連なりが 1 語として読まれて拒否されていたので、浮動
  するのは `$` だけでした。）* リテラルは 1 文字でなければならず、COBOL‑85 は picture
  文字や区切り文字と衝突するものを禁じています：数字は不可、
  `A B C D E G N P R S V X Z` のいずれも不可、`space * + - , . ; ( ) " / =` のいずれも
  不可です。
- ✅ **16 進リテラル** — `X"09"`、`x'0D0A'`（大小文字どちらでも、引用符どちらでも）。
  16 進数字の**対**ごとに 1 文字なので、数字の個数は偶数でなければなりません。奇数個や
  16 進でない数字は不正なリテラルであり、文字列の隣の語 `X` として静かに読み替えられる
  のではなく報告されます。引用符付きリテラルが使えるところならどこでも使えます
  （`DELIMITED BY`、`MOVE`、`VALUE`、比較）。

---

## DATA DIVISION の句（受け付ける宣言構文）

- ✅ レベル `01`–`49`、`77`、`88`；`FILLER`；集団／基本。`FILLER` という語は
  **任意**です — `05 PIC X VALUE ":".` は `05 FILLER PIC X VALUE ":".` と同じように
  1 つを宣言し、どちらの書き方でも、それを含む集団項目の中で自分のバイトと `VALUE` を
  保持します。
- ✅ `PIC/PICTURE`：`X A 9 S V P` と編集記号（`Z * $ + - CR DB B 0 / , .`）。
  通貨記号は `SPECIAL-NAMES. CURRENCY` が別のものを指名していなければ `$` です —
  上の**式・リテラル・USAGE**を参照。**`P` は小数位取り位置**です — 項目がまたぐが
  格納はしない数字位置です：`PIC S999PP` は百の位を表す数字 3 つを保持し
  （`MOVE 12300` はそれを正確に格納します。`MOVE 12345` は 12300 を格納します）、
  `PIC PP99` は 1 万分の 1 を表す 2 つを保持します。`P` が占める位置は常にゼロとして
  読み返され、レコードの配置では**バイトを取りません**。
- ✅ **小切手保護は項目全体を埋めます。** 数字位置がすべて `*` である picture における
  ゼロ値は、すべての文字位置をアスタリスクで埋めます — 小数部の数字、桁区切りのコンマ、
  固定の `$`、末尾の `CR` や `DB` も同様に — 残るのは小数点そのものだけです：
  ゼロを保持する `PIC $**.**CR` は `***.****` と読め、`PIC *,***.**` は `*****.**` と
  読めます。ゼロ**でない**値は先行ゼロだけを保護するので、固定の `$` は自分の位置を
  保ちます（`-2.34` → `$*2.34CR`）。*（1.62.37 より前は `CR`/`DB` が、占める 2 文字位置
  ではなくアスタリスク 1 つを寄与していたので、そうした項目は自分の幅より 1 文字短く
  返ってきていました。）*
- ✅ **数字リテラルは書かれたとおりに文字を転記します。** 英数字の受取り側へは、
  リテラルはプログラムが打ち込んだ数字を左詰めで空白を詰めて寄与します —
  `MOVE 2 TO <PIC X(4)>` は `"2   "` であり、`MOVE 060820000200 TO <6 個の PIC 99 の
  子項目>` はそれらを `06 08 20 00 02 00` と埋めます。**受取り側**の幅がリテラルを詰める
  ことは決してなく、詰めるのはリテラル自身の書かれた幅だけです。*（1.62.38 より前は
  字句解析器が値だけを保持していたので、先行ゼロが失われ、後続のすべての文字が 1 つ
  左へずれていました。）*
- ✅ **数値オペランドと非数値オペランドの間の関係は非数値です**
  （COBOL‑85 VI‑89 6.15.4 GR2）。数値オペランドは**自分自身の大きさ**の英数字項目へ
  転記されたものとして扱われ、その文字位置は移されますが**作用符号は移されません**：
  `-123456789012345678` を保持する `PIC S9(18)` は、`"123456789012345678"` を保持する
  `PIC X(18)` と**等しく**比較されます。3 つの条件が規則を限ります — 数値オペランドは
  **整数**でなければならないこと。「非数値」は**宣言**が決めるので、集団 `MOVE` の後に
  文字を保持している `PIC 99` の子項目は今も数値であること — そして**集団項目**は
  子項目が何であれ非数値なので、12345 を保持する `PIC 9(5)` と `"0000012345"` を保持する
  10 バイトの集団項目では `"12345     "` となり等しくないこと。そして `ALL literal` は
  相手のオペランドの大きさを取ります。*（1.62.38 より前は、テキスト側がたまたま数値と
  して解析できるときはいつでも比較が代数的でした。）*
- ✅ **数値 MOVE における上位桁の切り捨て。** 受取り側は両端で宣言した桁数を正確に
  保持します：`01 M PIC 99V999.  MOVE 123.45 TO M.` は `23.450` を残します。
  算術は先に受取り側の容量を検査するので、`ON SIZE ERROR` のある文は代わりに古い値を
  保ちます。
- ✅ **集団の表は出現ごとに指定されます。** `MOVE VALUES-1 TO GRP-1 (2)` はその出現
  自身の子項目（`ELEM1 (2,1) … ELEM1 (2,4)`）へ分配し、`GRP-1 (2)` を読むとちょうど
  それらが連結されます。それを囲む `01` レコードは**すべての**出現のバイトなので、
  `MOVE GRP-TAB1 TO GRP-TAB2` は表を丸ごと複製します。
- ✅ **指標名、リテラル、相対指標は添字として混在できます。** `ELEM1 (IN1, 1)`、
  `ELEM1 (1 IN2)`、`ELEM1 (IN1 +3)` — 数字に貼り付いた符号は次の添字を開く符号付き
  リテラルです — そして両側に空白のある演算子を持つ `ELEM1 (IN1 - 1, 3)` は相対指標
  です。
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}`（および `COMP-4`→COMP、`COMP-X`→COMP-5）。
- ✅ `VALUE`（数値／符号付き／英数字／形象／`ALL`）。**`VALUE ALL "literal"` は項目
  全体にわたって単位を繰り返します** — `PIC X(6) VALUE ALL "ABC"` は `"ABCABC"`、
  `PIC X(9) VALUE ALL "XY"` は `"XYXYXYXYX"` です。*（1.62.40 より前は 1 文字の形象
  定数だけが項目を埋め、`ALL "literal"` は空白を保持したままにしていました。）*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`。
- ✅ `REDEFINES` — 同じバイトの**生きた**第 2 の読み方です。記憶域を追加しない
  （したがってそれを含む集団項目を広げない）ので、どちらの記述を通した書込みも
  もう一方を通して見えます：
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` はその後 `RESULT-A` を通して読み返せます。
  ⚠️ **注意点：** 展開後の記憶域の枠が 256 を超える重ね（たとえば再定義した
  10×10×10 の表）は、代わりに記述ごとの記憶域を保ちます — 書込みごとに更新すれば
  1000 の出現を 2 度歩くことになるからです。
- ✅ **重ねは入れ子になります。** それ自体が再定義されているレコードの内側の
  `REDEFINES` は、どれだけ深くても両方向から到達できます：01 レベルの再定義を通して
  2 バイト書くと、再定義されたレコード、その内側の集団項目の `REDEFINES`、そして
  *その*内側の項目の `REDEFINES` に届きます — 最も内側のものに宣言された 88 も含みます。
  各記述は書込みごとに 1 回作り直されます。*（1.62.42 より前は、複数の重ねに属する
  キーが最後に宣言されたものだけを保持し、1 つの番兵が最初の 1 跳びで連鎖を止めて
  いました。）*
- ✅ **名前のない記述も記述です。** `02 FILLER REDEFINES <item>.` は対象のバイトを
  自分の名前なしで再記述し、対象への書込みはその子項目を通して見えます。複数の子項目が
  そのバイトを配置順に分け合います — 重ねは最初の子項目の別名では*ありません*。1 つの
  項目に対する 2 つの `FILLER REDEFINES` は 2 つの独立した読み方であり、それぞれが
  対象の**先頭**バイトから始まります。*（1.62.36 より前は名前のない再定義集団項目に
  記憶域のキーがまったく与えられなかったので、対象がどう埋められていても子項目は空白と
  して読まれていました。）*
- ✅ **重ねの内側の重複した名前**は、プログラムの他の部分が到達するのと同じ記憶域に
  解決します：2 つの異なる集団項目の下で宣言された `TAB-A` は、宣言ごとに 1 つの
  読み方を保ちます。*（1.62.36 より前は重ねの初期複製が、外側の修飾子を欠いた経路から
  索引付けされていました。それを見分けられるのは重複した名前だけなので、まさに修飾子を
  必要とする場合がそれを失っていたのです。）*
- ✅ `JUSTIFIED [RIGHT]` — *英数字*または*英字*の項目で**右詰めで格納します**。
  受取り側より狭い送り出し側は左側が詰められます。受取り側より広い送り出し側は
  **右**端を保ち、最も左の文字を失います — 通常の規則の逆です。*（1.62.40 より前は
  この句が英数字項目についてだけ記録されていたので、`PICTURE A(5) JUSTIFIED RIGHT` は
  解析された後、他の項目と同じように左詰めになっていました。）*
- ✅ `SYNCHRONIZED/SYNC`、`BLANK [WHEN] ZERO`、
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`、`GLOBAL`、`EXTERNAL` — 受け付けます。
  `SIGN … SEPARATE` は項目の格納方法をまだ変えません。
- ✅ **01 レベルの `REDEFINES` は、再定義する項目より多くの記憶域を記述できます**。
  その項目の終わりを越えたバイトは、それらを指名できるだけ長い記述に属します。
  短い記述を通して書いても、長い記述の末尾はそのまま残ります。
- ✅ **`REDEFINES` の重ねは再定義された項目のバイトを運びます**。数値の相手へも同様
  です：`"00ABCDEFGHI  4321 "` を保持する `X(18)` に対する `PIC S9(18)` の重ねは
  それらの文字を読み返し、`IS NUMERIC` はそれらに対して**いいえ**と答えます。
  バイトが実際に数字を綴っている場合は、数値としての読み方は変わりません。
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **本物の条件名**です：88 レベルは
  自分の宿主項目に結び付きます。検査は宿主を VALUE ／範囲に対して調べ、
  `SET 88-name TO TRUE` はそれを満たす値を宿主に格納します。
- ✅ **条件名は 2 つ以上の集団項目の下で宣言でき、`OF`/`IN` がそれらを見分けます** —
  データ名の場合とまったく同じで、途中の水準は省略してかまいません：
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  添字は宿主項目に属するので、VALUE がどの出現に対して検査されるかを選びます。
  重複した条件名への**修飾のない**参照は COBOL‑85 では曖昧です。ランタイムは最初の
  宣言を取ります。曖昧なデータ名に適用するのと同じ規則です。
- ✅ `USAGE INDEX` は整数の指標レジスタを宣言します（`SET`/`SEARCH` が使います）。
  `USAGE POINTER` — 上の**ポインタ**を参照。
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — 再編成の別名です。読取りは
  対象の項目群を連結し、書込みは項目の幅に応じて分配します。
  - ✅ **66 は自分が再編成するレコードによって修飾されます**。データ項目が上位の集団
    項目によって修飾されるのとまったく同じです。したがって同じ 66 の名前をレコードごとに
    1 回宣言し、`OF`/`IN` で見分けられます：
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`。これは読取りでも書込みでも
    同じように働き、66 は名前をたまたま共有する普通のデータ項目に勝ちます。`RENAMES`
    句のオペランドは同じレコードの中で解決するので、重複した `NAME-2` はこのレコードの
    ものを指します。
  - ✅ **対象に含まれる表はすべての出現を寄与します**。最初の 1 つだけではありません：
    `03 T PIC XXX OCCURS 5` を含む `TABLE-2` に対する
    `66 R RENAMES ITEM-1 THRU TABLE-2` は幅 20 文字です。
  - ✅ **ちょうど 1 項目にかかる 66 はその項目*そのもの*です** — 同じ PICTURE、同じ
    範疇、同じ記憶域です。`W` が `PIC 9(4)` であるときの `66 R RENAMES W` は 4 桁の
    数値項目なので、8000 が入っている状態での `ADD 3500 TO R` は `ON SIZE ERROR` を
    起こし、値を変えずに残します。
- セクション：`WORKING-STORAGE`、`LOCAL-STORAGE`、`LINKAGE`、`FILE`；`SCREEN` は
  解析されますが実行されません。

---

## まだ未対応 — 現在の回避リスト

> **2026‑08‑25 に訂正。** この節はかつて「COBOL‑85 の動詞／句の集合は**完全に網羅
> されています**」で始まっていました。NIST CCVS85 スイートを実行してそれは覆りました：
> その日**対象内 434 本のうち 102 本が失敗しました**。しかも本書が欠落として挙げて
> いなかった構文で — 区切りのコンマとセミコロン、`FUNCTION x(ALL)`、
> `CLOSE … WITH LOCK`、B 領域の `COPY`、IDENTIFICATION の注記項目、section の優先
> 番号、数字で始まるデータ名、そして — 1.62.10 までは — 先頭に小数点のある数字
> リテラルです。検証スイートはそのためにあります。各欠落は今
> [`specs/nist/`](../specs/nist/README.md) に仕様化され、上の
> [スコアボード](#-適合性は主張ではなく測定される--nist-ccvs85)で追跡されています。

下のリストは**意図的に**対象外であるものです。上の NIST の欠落は取り組み中の欠陥で
あり、それとは対照的です：

1. **画面 `ACCEPT` の入力編集** — `DISPLAY … AT/WITH` と `ACCEPT … AT` は CLI モードで
   （ANSI で）実行されますが、SCREEN SECTION の項目水準の完全な編集（自動タブ送り、
   項目の検査、色対応表）は GUI モードでは**フォームデザイナに取って代わられています**。
2. ***プロセス*間のファイル共有** — `OPEN … SHARING/WITH LOCK`、
   `READ … WITH [NO] LOCK`、`UNLOCK` は解析され、INDEXED エンジンの実行内レコード
   ロックを操作しますが、ロックは別々の OS プロセス間では強制されません（単一実行単位の
   モデル）。
3. **オブジェクト指向 COBOL**（クラス／メソッド定義） — `INVOKE` は COBOL の
   オブジェクトに対しては何もしません（GUI／ランタイムのオブジェクトだけを操作します）。
4. ✅ **解決（1.62.15）。** 認識されない組込み関数名は黙って **0** を返していたので、
   プログラムは打ち間違いから自信たっぷりに誤った答えを計算していました。今は関数を
   名指しし、十分に近い一致があるときは最も近い本物を提案する**コンパイルエラー**です
   （`cobolt-semantic/src/resolver.rs`、`Expr::FunctionCall`）。「静かなゼロ」という形は
   項目 5 と 6 がまだ抱えている罠なので、ここに残しています。
5. ⚠️ **不正な `ACCESS MODE` / `ORGANIZATION` の値が診断なしに飲み込まれます** —
   同じ罠の再来であり、しかもこちらは利用者のごく普通の打ち間違いで発動します。
   `ACCESS MODE IS` は `SEQUENTIAL`、`RANDOM`、`DYNAMIC` だけを受け付けますが
   （`INDEXED` は*編成*であってアクセスモードではありません）、SELECT 句の解析器は
   その 3 つを検査し、それ以外は「未知のトークンを飛ばす」という汎用の分岐に落として
   しまうので、ファイルは黙って既定の `SEQUENTIAL` を保ち、コンパイルに失敗する
   代わりに実行時に誤って振る舞います。`ORGANIZATION IS` も同一の形です
   （`cobolt-parser/src/parser.rs` の `Token::Access` の分岐と、その上の編成の分岐）。
   どちらも問題の語を名指しする明確なコンパイル時エラーを出すべきです。
   **NIST のどのモジュールもこれを捉えることはありません** — スイートは正当な句しか
   書かないので、この欠落が開いたままでもすべてのモジュールが 100 % で終われます。
   これは利用者の打ち間違いの罠であり、モジュールの点数ではなく専用のテストを必要と
   します。
6. ⚠️ **`ALPHABET … IS EBCDIC` は受け付けられますが、固有（ASCII）の順序が有効なまま
   です。** リテラルの句（`"A" THRU "H" "I" ALSO "J" …`）、`NATIVE`、`STANDARD‑1`、
   `STANDARD‑2` はすべて実装されており、`PROGRAM COLLATING SEQUENCE` を実際に操作
   します。欠けているのは EBCDIC の表だけで、それを指名すると静かに ASCII 順になります。
   4–6 と同じ罠の系統です。
7. **通信モジュールと Report Writer** — 上の
   [N/A](#-na--rustcobol-の対象外となるものおよびその理由)を参照。

> **解決（1.5.0）：** 平坦だったデータモデルが階層的／出現を意識したものになり、
> **CORRESPONDING**、**修飾名**、**表の添字**、**`SEARCH`** が解放されました。
> **解決（1.6.0）：** 複数受取り側の `MULTIPLY`/`DIVIDE` + 受取り側ごとの `ROUNDED`；
> `EXIT PERFORM/PARAGRAPH/SECTION`；`CALL NOT ON EXCEPTION`；
> `INSPECT TALLYING REPLACING` の併用 + `BEFORE/AFTER INITIAL`；
> 日付／`ANNUITY` の組込み関数；リテラルを対象とする省略；
> `EVALUATE ALSO`/`WHEN NOT`；本物の 88 レベル条件名；`PERFORM para VARYING`；
> そして `RELEASE`/`RETURN` を伴う `SORT`/`MERGE` ランタイム。
> **解決（1.7.0）：** 識別子を対象とする省略；`INITIALIZE … REPLACING`；
> `66 RENAMES`；ポインタ（`USAGE POINTER`、`SET ADDRESS OF` / `TO ADDRESS OF` /
> `NULL`）；`ALTER` / `UNLOCK`；忠実な `NEXT SENTENCE`；残りの標準組込み関数；
> そして拡張された画面 `ACCEPT`/`DISPLAY`（CLI モードで実行）。
> **解決（1.7.1）：** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS`
> （対になる `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME` レジスタとともに）。
> **解決（1.7.2）：** `OPEN … SHARING/WITH LOCK`、`READ … WITH [NO] LOCK`、
> `UNLOCK`（INDEXED のレコードロックを解放）、`CANCEL program`。
> **解決（1.8.0）：** `COMMIT` / `ROLLBACK` をプログラム制御の INDEXED ファイル
> トランザクションとして（メモリ／ディスク両エンジン。ディスクには本物の取消しログ）。

.<<

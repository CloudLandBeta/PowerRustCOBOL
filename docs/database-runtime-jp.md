<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL データベースランタイム

RustCOBOL のプログラムは、少数の組み込み `CALL` を通じて SQL データベースと
やり取りします。同じ 6 つの動詞が **3 つのバックエンド**に対して機能します。
エンジンは接続文字列から自動的に選択されるため、SQLite 向けに書いたプログラム
が、リテラルを 1 つ変えるだけで PostgreSQL や MySQL に対してそのまま動きます。

| バックエンド | ドライバ（純 Rust、システムライブラリ不要） | 接続文字列 |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`（SQLite 同梱）              | `:memory:`、`sqlite:<パス>`、または素のファイルパス |
| **PostgreSQL** | `postgres`（rust-postgres、同期）   | `postgres://ユーザー:パスワード@ホスト:ポート/db`  |
| **MySQL**   | `mysql`（rustls、同期）                | `mysql://ユーザー:パスワード@ホスト:ポート/db`     |

3 つのドライバはいずれも静的にリンクされ、ビルドに**外部のクライアント
ライブラリ**（`libpq`、`libmysqlclient`）も **OpenSSL** も必要としません。
PowerRustCOBOL の他の部分と一貫した方針です。

---

## 1. 接続文字列

バックエンドは接続文字列のスキームのみから選ばれます。

| 形式 | バックエンド | 備考 |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | RAM 上のデータベース。クローズ時に破棄される。 |
| `sqlite:/var/data/app.db`                  | SQLite        | ファイルが存在しなければ作成される。   |
| `/var/data/app.db`                         | SQLite        | 素のパスは SQLite として扱われる。     |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` も受け付けられる。   |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

スキームの照合は大文字小文字を区別せず、前後の空白も許容します。
`postgres(ql)://` でも `mysql://` でもない URL は、すべて SQLite の対象として
扱われます。

---

## 2. CALL のインターフェース

どの CALL も引数を `BY REFERENCE` で渡します。ステータスやハンドルの値は通常の
COBOL データ項目に置かれるため、保持して段落をまたいで受け渡すことができます。

| CALL 名 | 引数（`BY REFERENCE`） |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | 接続文字列、ハンドル変数 `PIC 9(9)`、ステータス変数     |
| `COBOL-EXEC-SQL`   | ハンドル、クエリ、行数変数 `PIC 9(9)`、ステータス変数   |
| `COBOL-FETCH-ROW`  | ハンドル、列インデックス `PIC 9(n)`（1 起点）、格納先変数、ステータス |
| `COBOL-NEXT-ROW`   | ハンドル、継続フラグ変数 `PIC X`（`Y`/`N`）             |
| `COBOL-ROW-COUNT`  | ハンドル、件数変数 `PIC 9(9)`                           |
| `COBOL-CLOSE-DB`   | ハンドル                                                |

### 意味

- **`COBOL-OPEN-DB`** は接続を開き、正の整数のハンドルを*ハンドル変数*に書き
  込みます。成功時は*ステータス変数*が空白になり、失敗時は*ハンドル変数*が `0`
  となって*ステータス変数*にドライバのエラーメッセージが入ります。
- **`COBOL-EXEC-SQL`** は*ハンドル*上で 1 つの文を実行します。
  - 行を返す文（`SELECT`、CTE など）では結果セット全体がキャッシュされ、
    *行数変数*が**行数**を受け取ります。カーソルは最初の行から始まります。
  - `INSERT` / `UPDATE` / `DELETE` / DDL では、*行数変数*が**影響を受けた行数**
    を受け取り、結果セットは空になります。
  - エラー時は*ステータス変数*にメッセージが入り、*行数変数*は `0` になります。
- **`COBOL-FETCH-ROW`** は**現在**の行の*列インデックス*（1 起点）の列を、
  テキストとして*格納先変数*にコピーします。範囲外の列や、使い切ったカーソルは
  空白を返します。
- **`COBOL-NEXT-ROW`** はカーソルを進め、行が利用可能になれば*継続フラグ変数*を
  `Y` に、結果セットを使い切ったら `N` にします。
- **`COBOL-ROW-COUNT`** は直前のクエリのキャッシュ済み行数を返します。
- **`COBOL-CLOSE-DB`** は接続を閉じ、その結果セットを解放します。未知のハンドル
  は無視されます。開いている接続はすべて、プログラム終了時に閉じられます。

### 値の正規化

列の値は、バックエンドや SQL の型にかかわらず、すべて**テキスト**として COBOL
に渡されます。したがって `PIC X` の項目へそのまま `MOVE` できます（数字を
読み替える数値項目へも同様です）。正規化の規則は一様です。

| SQL の値 | COBOL に渡されるテキスト |
|----------------|----------------------------------------|
| `NULL`         | 空白（空文字列）                       |
| 整数           | 十進の数字。例：`42`、`-7`             |
| 実数 / 倍精度  | 往復可能な最短形式。例：`3.14`         |
| text / varchar | UTF-8 の文字列                         |
| date           | `YYYY-MM-DD`                           |
| datetime       | `YYYY-MM-DD HH:MM:SS`                  |
| time（MySQL）  | `HH:MM:SS`                             |
| blob（SQLite） | `<blob N bytes>` というプレースホルダ  |

---

## 3. 例 — 可搬な CRUD

このプログラムは 3 つのバックエンドの**どれに対しても**動きます。変わるのは
`WS-CONN` だけです。テストスイート
（`crates/cobolt-runtime/tests/test_sql.rs`）が実際に実行しているプログラム
そのものです。

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-CRUD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CONN     PIC X(64)  VALUE ":memory:".
      *>  PostgreSQL: VALUE "postgres://scott:tiger@localhost:5432/store".
      *>  MySQL:      VALUE "mysql://scott:tiger@localhost:3306/store".
       01 WS-HANDLE   PIC 9(9)   VALUE 0.
       01 WS-STATUS   PIC X(128) VALUE SPACES.
       01 WS-QUERY    PIC X(256) VALUE SPACES.
       01 WS-ROWCNT   PIC 9(9)   VALUE 0.
       01 WS-COL      PIC 9(4)   VALUE 1.
       01 WS-NAME     PIC X(16)  VALUE SPACES.
       01 WS-MORE     PIC X      VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-OPEN-DB" USING WS-CONN WS-HANDLE WS-STATUS
           IF WS-STATUS NOT = SPACES
               DISPLAY "OPEN FAILED: " WS-STATUS
               STOP RUN
           END-IF

           MOVE "CREATE TABLE c (id INTEGER, name TEXT)" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS

           MOVE "INSERT INTO c VALUES (1,'ANA'),(2,'BRUNO'),(3,'CARLOS')"
               TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "INSERTED " WS-ROWCNT

           MOVE "SELECT name FROM c ORDER BY id" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "ROWS " WS-ROWCNT

           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               MOVE 1 TO WS-COL
               CALL "COBOL-FETCH-ROW"
                   USING WS-HANDLE WS-COL WS-NAME WS-STATUS
               DISPLAY "NAME " WS-NAME
               CALL "COBOL-NEXT-ROW" USING WS-HANDLE WS-MORE
           END-PERFORM

           CALL "COBOL-CLOSE-DB" USING WS-HANDLE
           STOP RUN.
```

出力（インメモリの SQLite）：

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### 複数の列を読む

`COBOL-FETCH-ROW` は 1 回の呼び出しで 1 列を読みます。カーソルを進める前に
`WS-COL` を変えれば、同じ行の別の列を読めます。

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. トランザクション

トランザクションは `COBOL-EXEC-SQL` を通じて通常の SQL で制御します。したがって
振る舞いはお使いのサーバーのものそのままです。

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> COBOL の `COMMIT` / `ROLLBACK` **動詞**は別の機能で、RustCOBOL の
> **INDEXED ファイル**のトランザクションを制御します
> （[`docs/indexed-file-format.md`](indexed-file-format.md) を参照）。これらは
> SQL 接続には**作用しません**。データベースに対しては、上に示したとおり
> `COBOL-EXEC-SQL` に `BEGIN`/`COMMIT`/`ROLLBACK` を渡してください。

PostgreSQL と MySQL は既定でオートコミットのため、単独の文は即座にコミットされ
ます。ひとまとまりの作業を原子的にするには `BEGIN … COMMIT` で囲んでください。

---

## 5. IDE のデータコントロール

PowerRustCOBOL の form designer では、**SqlDatabase** コントロールが定型の段落
（`<id>-CONNECT`、`<id>-EXEC`、`<id>-FETCH-ALL`、`<id>-CLOSE`）を自動生成し
ます。重要な property は 2 つです。

- **`ConnectionString`** — 上記のいずれかの接続文字列。実行時にバックエンドを
  実際に選択しているのはこれです。
- **`Driver`** — `sqlite`（既定）、`postgres`、`mysql`。見た目だけのもので、
  生成されるコメントにラベルを付けるだけです。振り分けは接続文字列で行われ
  ます。

---

## 6. セキュリティと運用上の注意

- **TLS。** MySQL ドライバは rustls でビルドされており、サーバーが要求すれば
  TLS をネゴシエートします。同期版の PostgreSQL ドライバは **TLS なし**
  （`NoTls`）で接続します。ローカルソケットや信頼できるネットワークに適した
  構成です。TLS を必須とする PostgreSQL サーバーに対しては、ローカルのプロキシ
  （`stunnel`／`pgbouncer` など）で TLS を終端するか、SSH トンネル経由で接続して
  ください。
- **SQL インジェクション。** 文はテキストとして送られます。クエリは信頼できる
  入力から組み立てるか、SQL 文字列を組み立てる前にユーザー由来の値を検証・
  エスケープしてください。
- **接続の寿命。** 各ハンドルは 1 つの生きた接続を所有します。不要になった
  ハンドルは `COBOL-CLOSE-DB` で閉じてください。開いたままのものは、プログラム
  終了時にすべて閉じられます。

---

## 7. テスト

- **オフライン（常に実行）：** 接続文字列の振り分け、値の正規化、そして
  インメモリ SQLite での CRUD 一巡 —
  `cargo test -p cobolt-runtime --lib db_runtime` と
  `cargo test -p cobolt-runtime --test test_sql`。
- **実サーバー（任意）：** `#[ignore]` を付けた 2 つの往復テストが実際の
  サーバーに接続します。URL を与えて明示的に実行してください。

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. 実装

エンジンは `crates/cobolt-runtime/src/db_runtime.rs` にあります。`DbConn` が
`Backend` 列挙型（`Sqlite` / `Postgres` / `MySql`）を包み、
`BackendKind::classify` が接続文字列からバックエンドを選びます。各バックエンド
は独自の `exec_*` 経路を持ち、行を `Vec<Vec<String>>` に正規化します。それ以降
の共有カーソルロジック（`fetch_col` / `next_row` / `row_count`）はバックエンド
に依存しません。インタプリタの `exec_call`
（`crates/cobolt-runtime/src/interpreter.rs`）が、COBOL の 6 つの CALL を
`DbRegistry` に対応付け、`DbRegistry` が整数ハンドルで接続をプールします。

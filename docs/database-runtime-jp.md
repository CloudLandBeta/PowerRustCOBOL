<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# RustCOBOL データベースランタイム

RustCOBOL のプログラムは、少数の組み込み `CALL` を通じて SQL データベースと
やり取りします。同じ 6 つの動詞が**3 つのバックエンド**に対して同じように働き
ます — エンジンは接続文字列から自動的に選ばれるため、SQLite 向けに書いた
プログラムはリテラルを 1 つ書き換えるだけで、そのまま PostgreSQL や MySQL でも
動きます。

| バックエンド | ドライバ（純 Rust、システムライブラリ不要） | 接続文字列 |
|-------------|---------------------------------------------|------------|
| **SQLite**  | `rusqlite`（SQLite を同梱）                 | `:memory:`、`sqlite:<path>`、または単なるファイルパス |
| **PostgreSQL** | `postgres`（rust-postgres、同期）        | `postgres://user:pass@host:port/db` |
| **MySQL**   | `mysql`（rustls、同期）                     | `mysql://user:pass@host:port/db` |

3 つのドライバはいずれも静的にリンクされ、ビルドに**外部クライアント
ライブラリ**（`libpq`、`libmysqlclient`）も **OpenSSL** も必要としません —
PowerRustCOBOL のほかの部分と同じ方針です。

---

## 1. 接続文字列

バックエンドは、接続文字列のスキームだけで決まります。

| 形式 | バックエンド | 備考 |
|------|-------------|------|
| `:memory:`                                 | SQLite     | RAM 上のデータベース。クローズ時に破棄されます。 |
| `sqlite:/var/data/app.db`                  | SQLite     | ファイルが存在しなければ作成されます。 |
| `/var/data/app.db`                         | SQLite     | 単なるパスは SQLite として扱われます。 |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` も受け付けます。 |
| `mysql://scott:tiger@localhost:3306/store` | MySQL      | |

スキームの照合は大文字・小文字を区別せず、前後の空白も許容します。
`postgres(ql)://` でも `mysql://` でもない URL は、すべて SQLite のターゲットと
みなされます。

---

## 2. CALL の全体像

どの CALL も引数を `BY REFERENCE` で渡します。ステータスやハンドルの値は
ふつうの COBOL のデータ項目に置かれるので、保持して段落をまたいで渡せます。

| CALL 名            | 引数（`BY REFERENCE`）                                    |
|--------------------|-----------------------------------------------------------|
| `COBOL-OPEN-DB`    | conn-string、handle-var `PIC 9(9)`、status-var            |
| `COBOL-EXEC-SQL`   | handle、query、row-count-var `PIC 9(9)`、status-var       |
| `COBOL-FETCH-ROW`  | handle、col-index `PIC 9(n)`（1 起点）、dest-var、status  |
| `COBOL-NEXT-ROW`   | handle、more-flag-var `PIC X`（`Y`/`N`）                  |
| `COBOL-ROW-COUNT`  | handle、count-var `PIC 9(9)`                              |
| `COBOL-CLOSE-DB`   | handle                                                    |

### 意味

- **`COBOL-OPEN-DB`** は接続を開き、正の整数のハンドルを *handle-var* に
  書き込みます。成功すると *status-var* は空白になり、失敗すると *handle-var*
  は `0`、*status-var* にドライバのエラーメッセージが入ります。
- **`COBOL-EXEC-SQL`** は *handle* 上で 1 つの文を実行します。
  - 行を返す文（`SELECT`、CTE など）では結果セット全体がキャッシュされ、
    *row-count-var* が**行数**を受け取ります。カーソルは先頭行から始まります。
  - `INSERT` / `UPDATE` / `DELETE` / DDL では *row-count-var* が**影響を受けた
    行数**を受け取り、結果セットは空になります。
  - エラー時は *status-var* にメッセージが入り、*row-count-var* は `0` です。
- **`COBOL-FETCH-ROW`** は**現在**の行の *col-index*（1 起点）列をテキストとして
  *dest-var* にコピーします。範囲外の列や、尽きたカーソルでは空白になります。
- **`COBOL-NEXT-ROW`** はカーソルを進め、行が得られたなら *more-flag-var* を
  `Y` に、結果セットが尽きたなら `N` にします。
- **`COBOL-ROW-COUNT`** は直前のクエリのキャッシュ済み行数を返します。
- **`COBOL-CLOSE-DB`** は接続を閉じ、その結果セットを解放します。未知のハンドル
  は無視されます。開いたままの接続はプログラム終了時にすべて閉じられます。

### 値の正規化

どの列の値も — バックエンドや SQL の型を問わず — **テキスト**として COBOL に
渡されます。そのため `PIC X` の項目へそのまま `MOVE` できます（数字項目へ移せ
ば、その桁が数値として解釈されます）。正規化の規則は一様です。

| SQL の値       | COBOL に渡されるテキスト                     |
|----------------|----------------------------------------------|
| `NULL`         | 空白（空文字列）                             |
| integer        | 10 進数字。たとえば `42`、`-7`               |
| real / double  | 往復変換できる最短の表記。たとえば `3.14`    |
| text / varchar | UTF-8 の文字列                               |
| date           | `YYYY-MM-DD`                                 |
| datetime       | `YYYY-MM-DD HH:MM:SS`                        |
| time (MySQL)   | `HH:MM:SS`                                   |
| blob (SQLite)  | `<blob N bytes>` というプレースホルダ        |

---

## 3. 例 — 可搬な CRUD

このプログラムは 3 つのバックエンドの**どれ**に対しても動きます。変わるのは
`WS-CONN` だけです。テストスイート
（`crates/cobolt-runtime/tests/test_sql.rs`）が実際に動かしているものと同じ
プログラムです。

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

出力（メモリ上の SQLite）:

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### 複数の列を読む

`COBOL-FETCH-ROW` は 1 回の呼び出しで 1 列を読みます。カーソルを進める前に
`WS-COL` を変えれば、同じ行のほかの列も読めます。

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. トランザクション

トランザクションは `COBOL-EXEC-SQL` を通じてふつうの SQL で駆動します。
したがって振る舞いは、お使いのサーバーのものそのままです。

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> COBOL の `COMMIT` / `ROLLBACK` **動詞**は別の機能で、RustCOBOL の
> **INDEXED ファイル**のトランザクションを制御します
> （[`docs/indexed-file-format-jp.md`](indexed-file-format-jp.md) を参照）。
> これらは SQL 接続には**作用しません** — データベースには、上の例のように
> `COBOL-EXEC-SQL` と `BEGIN`/`COMMIT`/`ROLLBACK` を使ってください。

PostgreSQL と MySQL は既定で自動コミットなので、単独の文はただちにコミットされ
ます。ひとまとまりの処理を原子的にしたい場合は `BEGIN … COMMIT` で囲みます。

---

## 5. IDE のデータコントロール

PowerRustCOBOL のフォームデザイナーでは、**SqlDatabase** コントロールが定型の
段落（`<id>-CONNECT`、`<id>-EXEC`、`<id>-FETCH-ALL`、`<id>-CLOSE`）を自動生成
します。重要なプロパティは 2 つです。

- **`ConnectionString`** — 上に挙げた接続文字列のいずれか。実行時に
  バックエンドを実際に選ぶのはこれです。
- **`Driver`** — `sqlite`（既定）、`postgres`、`mysql`。見た目だけのもので、
  生成されるコメントのラベルに使われます。振り分けは接続文字列で決まります。

---

## 6. セキュリティと運用上の注意

- **TLS。** MySQL ドライバは rustls でビルドされており、サーバーが要求すれば
  TLS をネゴシエートします。同期版の PostgreSQL ドライバは **TLS なし**
  （`NoTls`）で接続します — ローカルソケットや信頼できるネットワーク向けです。
  TLS を必須とする PostgreSQL サーバーには、ローカルのプロキシ（`stunnel` /
  `pgbouncer` など）で TLS を終端するか、SSH トンネル越しに接続してください。
- **SQL インジェクション。** 文はテキストとして送られます。クエリは信頼できる
  入力から組み立てるか、SQL 文字列を組み立てる前に、利用者が与えた値を
  検証・エスケープしてください。
- **接続の寿命。** ハンドル 1 つが生きた接続 1 つを所有します。不要になった
  ハンドルは `COBOL-CLOSE-DB` で閉じてください。開いたまま残ったものは、
  プログラムの終了時に閉じられます。

---

## 7. テスト

- **オフライン（常に実行）:** 接続文字列の振り分け、値の正規化、そして
  メモリ上の SQLite での CRUD 一巡 —
  `cargo test -p cobolt-runtime --lib db_runtime` と
  `cargo test -p cobolt-runtime --test test_sql`。
- **実サーバー（オプトイン）:** `#[ignore]` を付けた 2 つの往復テストが実際の
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
`Backend` 列挙（`Sqlite` / `Postgres` / `MySql`）を包み、
`BackendKind::classify` が接続文字列からバックエンドを選びます。各
バックエンドは独自の `exec_*` 経路を持ち、行を `Vec<Vec<String>>` に正規化し
ます。そこから先の共通カーソル処理（`fetch_col` / `next_row` / `row_count`）は
バックエンドに依存しません。インタプリタの `exec_call`
（`crates/cobolt-runtime/src/interpreter.rs`）が COBOL の 6 つの CALL を
`DbRegistry` に対応づけ、`DbRegistry` が整数ハンドルごとに接続をプールします。

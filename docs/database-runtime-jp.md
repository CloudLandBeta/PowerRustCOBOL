<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# RustCOBOL データベースランタイム

RustCOBOL のプログラムは、少数の組込み `CALL` を通じて SQL データベースと対話しま
す。同じ 6 つの動詞が **3 つのバックエンド**に対して働きます。エンジンは接続文字列
から自動的に選ばれるので、SQLite 向けに書いたプログラムはリテラルを 1 つ変えるだけ
で PostgreSQL や MySQL でもそのまま動きます。

| バックエンド | ドライバー（システムライブラリは不要）     | 接続文字列                                          |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`、`features = ["bundled"]` — SQLite の **C** アマルガメーションをコンパイルするので、これだけは純 Rust ではありません | `:memory:`、`sqlite:<パス>`、または素のファイルパス |
| **PostgreSQL** | `postgres`（rust-postgres、同期）  | `postgres://user:pass@host:port/db`                |
| **MySQL**   | `mysql`（`minimal-rust`、同期、TLS なし） | `mysql://user:pass@host:port/db`               |

3 つのドライバーはいずれも静的にリンクされ、ビルドに**外部のクライアント
ライブラリ**（`libpq`、`libmysqlclient`）も **OpenSSL** も必要としません。
PowerRustCOBOL のほかの部分と同じ方針です。

---

## 1. 接続文字列

バックエンドは、接続文字列のスキームだけで決まります。

| 形式                                       | バックエンド  | 備考                                   |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | RAM 上のデータベース。閉じると破棄されます。 |
| `sqlite:/var/data/app.db`                  | SQLite        | ファイルが無ければ作成されます。       |
| `/var/data/app.db`                         | SQLite        | 素のパスは SQLite とみなされます。     |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` も受け付けます。    |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

スキームの照合は大文字小文字を区別せず、前後の空白も許容します。
`postgres(ql)://` でも `mysql://` でも**ない** URL は、すべて SQLite の対象として
扱われます。

---

## 2. CALL の一覧

どの CALL も引数を `BY REFERENCE` で渡します。状態やハンドルの値はごく普通の COBOL
データ項目に入るので、保持して段落間で受け渡せます。

| CALL 名            | 引数（`BY REFERENCE`）                                  |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | 接続文字列、ハンドル変数 `PIC 9(9)`、状態変数           |
| `COBOL-EXEC-SQL`   | ハンドル、問い合わせ、行数変数 `PIC 9(9)`、状態変数     |
| `COBOL-FETCH-ROW`  | ハンドル、列番号 `PIC 9(n)`（1 起点）、格納先変数、状態 |
| `COBOL-NEXT-ROW`   | ハンドル、継続フラグ変数 `PIC X`（`Y`/`N`）             |
| `COBOL-ROW-COUNT`  | ハンドル、件数変数 `PIC 9(9)`                           |
| `COBOL-CLOSE-DB`   | ハンドル                                                |

### 意味

- **`COBOL-OPEN-DB`** は接続を開き、正の整数のハンドルを *handle-var* に書き込みま
  す。成功すると *status-var* は空白になり、失敗すると *handle-var* は `0`、
  *status-var* にドライバーのエラーメッセージが入ります。
- **`COBOL-EXEC-SQL`** は *handle* 上で 1 つの文を実行します。
  - 行を返す文（`SELECT`、CTE など）では結果セット全体がキャッシュされ、
    *row-count-var* が**行数**を受け取ります。カーソルは先頭行から始まります。
  - `INSERT` / `UPDATE` / `DELETE` / DDL では、*row-count-var* が**影響を受けた
    行数**を受け取り、結果セットは空になります。
  - エラー時は *status-var* にメッセージが入り、*row-count-var* は `0` です。
- **`COBOL-FETCH-ROW`** は、**現在**行の *col-index*（1 起点）列をテキストとして
  *dest-var* にコピーします。範囲外の列や、尽きたカーソルは空白になります。
- **`COBOL-NEXT-ROW`** はカーソルを進め、行が利用できるようになれば
  *more-flag-var* を `Y` に、セットが尽きていれば `N` にします。
- **`COBOL-ROW-COUNT`** は、直前の問い合わせのキャッシュ済み行数を返します。
- **`COBOL-CLOSE-DB`** は接続を閉じ、その結果セットを解放します。未知のハンドルは
  無視されます。開いたままの接続は、プログラム終了時にすべて閉じられます。

### 値の正規化

どの列の値も — バックエンドや SQL の型を問わず — **テキスト**として COBOL に渡され
ます。ですからそのまま `PIC X` 項目へ `MOVE` できます（数字項目へ移せば、桁として
解釈し直されます）。正規化の仕方は一様です。

| SQL の値       | COBOL に渡されるテキスト               |
|----------------|----------------------------------------|
| `NULL`         | 空白（空文字列）                       |
| 整数           | 十進の数字列。例: `42`、`-7`           |
| 実数 / 倍精度  | 往復可能な最短表現。例: `3.14`         |
| テキスト / varchar | UTF-8 の文字列                     |
| 日付           | `YYYY-MM-DD`                           |
| 日時           | `YYYY-MM-DD HH:MM:SS`                  |
| 時刻（MySQL）  | `HH:MM:SS`                             |
| blob（SQLite） | `<blob N bytes>` というプレースホルダー |

---

## 3. 例 — 可搬な CRUD

このプログラムは 3 つのバックエンドの**どれでも**動きます。変えるのは `WS-CONN`
だけです。テストスイート（`crates/cobolt-runtime/tests/test_sql.rs`）が実際に動かして
いるのと同じプログラムです。

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

`COBOL-FETCH-ROW` は 1 回の呼び出しで 1 列を読みます。同じ行のほかの列を読むには、
次の行へ進む前に `WS-COL` を変えてください。

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. トランザクション

トランザクションは `COBOL-EXEC-SQL` を通じた普通の SQL で制御するので、振る舞いは
そのままお使いのサーバーのものです。

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> COBOL の `COMMIT` / `ROLLBACK` **動詞**は別の機能で、RustCOBOL の
> **INDEXED ファイル**のトランザクションを制御します
> （[`docs/indexed-file-format-jp.md`](indexed-file-format-jp.md) を参照）。SQL 接続
> には作用**しません**。データベースには、上の例のように `COBOL-EXEC-SQL` で
> `BEGIN`/`COMMIT`/`ROLLBACK` を使ってください。

PostgreSQL と MySQL は既定で自動コミットなので、単独の文は即座に確定します。作業の
まとまりを原子的にしたい場合は `BEGIN … COMMIT` で囲んでください。

---

## 5. IDE のデータコントロール

PowerRustCOBOL のフォームデザイナーでは、**SqlDatabase** コントロールが定型的な段落
（`<id>-CONNECT`、`<id>-EXEC`、`<id>-FETCH-ALL`、`<id>-CLOSE`）を自動生成します。
重要なプロパティは 2 つです。

- **`ConnectionString`** — 上記のいずれかの接続文字列。実行時にバックエンドを実際に
  選ぶのはこれです。
- **`Driver`** — `sqlite`（既定）、`postgres`、`mysql`。見た目だけの設定で、生成され
  るコメントのラベルになります。振り分けは接続文字列で決まります。

---

## 6. セキュリティと運用上の注意

- **TLS。** ⚠️ **今日、どちらの SQL ドライバーも TLS を話しません。** MySQL の
  ドライバーは `default-features = false, features = ["minimal-rust"]` でビルドされ
  ており、解決された `mysql 28` は TLS の crate をまったく引き込みません。サーバー
  が何を要求しようと、安全な接続を交渉できないのです。同期版の PostgreSQL ドライバー
  は構造上 `NoTls` で接続します。どちらもローカルのソケットや信頼できるネットワーク
  向けです。TLS を必須とするサーバーには、ローカルのプロキシ（`stunnel`/`pgbouncer`
  など）で終端するか、SSH トンネル越しに接続してください。
- **SQL インジェクション。** 文はテキストとして送られます。問い合わせは信頼できる
  入力から組み立てるか、SQL 文字列を組み立てる前に、利用者から与えられた値を検証・
  エスケープしてください。
- **接続の寿命。** ハンドル 1 つが生きた接続 1 つを所有します。不要になったハンドル
  は `COBOL-CLOSE-DB` で閉じてください。開いたまま残ったものは、プログラム終了時に
  閉じられます。

---

## 7. テスト

- **オフライン（常に実行）:** 接続文字列の振り分け、値の正規化、そしてメモリ上の
  SQLite による CRUD の往復一式 —
  `cargo test -p cobolt-runtime --lib db_runtime` と
  `cargo test -p cobolt-runtime --test test_sql`。
- **実サーバー（任意）:** `#[ignore]` を付けた 2 つの往復テストが本物のサーバーに
  接続します。URL を与えて明示的に実行してください。

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. 実装

エンジンは `crates/cobolt-runtime/src/db_runtime.rs` にあります。`DbConn` が
`Backend` 列挙（`Sqlite` / `Postgres` / `MySql`）を包み、`BackendKind::classify`
が接続文字列からバックエンドを選びます。各バックエンドは独自の `exec_*` 経路を持ち、
行を `Vec<Vec<String>>` に正規化します。そこから先の共有カーソル処理
（`fetch_col` / `next_row` / `row_count`）はバックエンドに依存しません。
インタープリターの `exec_call`（`crates/cobolt-runtime/src/interpreter.rs`）が、
6 つの COBOL CALL を `DbRegistry` に対応づけます。`DbRegistry` は整数ハンドルで接続
をプールします。

.<<

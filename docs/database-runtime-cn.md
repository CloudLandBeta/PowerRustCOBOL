<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# RustCOBOL 数据库运行时

RustCOBOL 程序通过一小组内建 `CALL` 与 SQL 数据库对话。同样这六个动词对**三种后端**
都有效——引擎会根据连接字符串自动选定，因此一个为 SQLite 写的程序，只要改一个字面
量，就能原封不动地跑在 PostgreSQL 或 MySQL 上。

| 后端        | 驱动（无需系统库）                      | 连接字符串                                          |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`，`features = ["bundled"]` —— 会编译 SQLite 的 **C** 合并源码，所以唯独它不是纯 Rust | `:memory:`、`sqlite:<路径>`，或一个裸文件路径 |
| **PostgreSQL** | `postgres`（rust-postgres，同步）   | `postgres://user:pass@host:port/db`                |
| **MySQL**   | `mysql`（`minimal-rust`，同步，无 TLS） | `mysql://user:pass@host:port/db`                 |

这三个驱动都是静态链接的，构建时**不需要任何外部客户端库**（`libpq`、
`libmysqlclient`），也**不需要 OpenSSL**——与 PowerRustCOBOL 其余部分保持一致。

---

## 1. 连接字符串

后端完全由连接字符串的方案（scheme）决定：

| 形式                                       | 后端          | 说明                                   |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | 内存中的数据库，关闭时丢弃。           |
| `sqlite:/var/data/app.db`                  | SQLite        | 文件不存在时会被创建。                 |
| `/var/data/app.db`                         | SQLite        | 裸路径按 SQLite 处理。                 |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | 也接受 `postgresql://`。            |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

方案匹配不区分大小写，并容忍前后空白。凡**不是** `postgres(ql)://` 或 `mysql://`
URL 的内容，一律按 SQLite 目标处理。

---

## 2. CALL 接口

每个 CALL 都以 `BY REFERENCE` 传递参数。状态值与句柄值存放在普通的 COBOL 数据项里，
因此可以保存并在段落之间传递。

| CALL 名称          | 参数（`BY REFERENCE`）                                  |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | 连接字符串、句柄变量 `PIC 9(9)`、状态变量               |
| `COBOL-EXEC-SQL`   | 句柄、查询语句、行数变量 `PIC 9(9)`、状态变量           |
| `COBOL-FETCH-ROW`  | 句柄、列序号 `PIC 9(n)`（从 1 开始）、目标变量、状态    |
| `COBOL-NEXT-ROW`   | 句柄、是否还有行的标志变量 `PIC X`（`Y`/`N`）           |
| `COBOL-ROW-COUNT`  | 句柄、计数变量 `PIC 9(9)`                               |
| `COBOL-CLOSE-DB`   | 句柄                                                    |

### 语义

- **`COBOL-OPEN-DB`** 打开一个连接，并把一个正整数句柄写入 *handle-var*。成功时
  *status-var* 置为空格；失败时 *handle-var* 为 `0`，*status-var* 里是驱动给出的
  错误信息。
- **`COBOL-EXEC-SQL`** 在 *handle* 上执行一条语句。
  - 对会返回行的语句（`SELECT`、CTE 等），整个结果集会被缓存，*row-count-var* 收到
    **行数**。游标从第一行开始。
  - 对 `INSERT` / `UPDATE` / `DELETE` / DDL，*row-count-var* 收到**受影响的行数**，
    结果集为空。
  - 出错时 *status-var* 里是错误信息，*row-count-var* 为 `0`。
- **`COBOL-FETCH-ROW`** 把**当前**行第 *col-index* 列（从 1 开始）以文本形式复制到
  *dest-var*。越界的列和已耗尽的游标都会得到空格。
- **`COBOL-NEXT-ROW`** 推进游标：若现在有可用的行，把 *more-flag-var* 置为 `Y`；结果
  集耗尽后置为 `N`。
- **`COBOL-ROW-COUNT`** 返回上一次查询缓存下来的行数。
- **`COBOL-CLOSE-DB`** 关闭连接并释放其结果集。未知句柄会被忽略。程序结束时，所有
  仍然打开的连接都会被关闭。

### 值的规范化

每个列值——无论后端或 SQL 类型为何——都以**文本**形式交给 COBOL，因此可以直接
`MOVE` 进一个 `PIC X` 字段（或移入数字字段，由它重新解释这些数字）。规范化是统一
的：

| SQL 值         | 交给 COBOL 的文本                      |
|----------------|----------------------------------------|
| `NULL`         | 空格（空字符串）                       |
| 整数           | 十进制数字，例如 `42`、`-7`            |
| 实数 / 双精度  | 最短的可往返形式，例如 `3.14`          |
| 文本 / varchar | 该 UTF-8 字符串                        |
| 日期           | `YYYY-MM-DD`                           |
| 日期时间       | `YYYY-MM-DD HH:MM:SS`                  |
| 时间（MySQL）  | `HH:MM:SS`                             |
| blob（SQLite） | `<blob N bytes>` 占位符                |

---

## 3. 示例 —— 可移植的 CRUD

这个程序在三种后端中的**任何一种**上都能跑；只有 `WS-CONN` 需要改。它正是测试套件
（`crates/cobolt-runtime/tests/test_sql.rs`）实际运行的那个程序。

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

输出（内存中的 SQLite）：

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### 读取多个列

`COBOL-FETCH-ROW` 每次调用读一列；在推进到下一行之前，改变 `WS-COL` 就能读取同一行
的其他列：

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. 事务

事务是通过 `COBOL-EXEC-SQL` 用普通 SQL 驱动的，因此行为完全就是你的服务器的行为：

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> COBOL 的 `COMMIT` / `ROLLBACK` **动词**是另一项独立功能，它控制的是 RustCOBOL
> 的 **INDEXED 文件**事务（参见
> [`docs/indexed-file-format-cn.md`](indexed-file-format-cn.md)）。它们**不**作用于
> SQL 连接——对数据库请按上面的示例，使用 `COBOL-EXEC-SQL` 配合
> `BEGIN`/`COMMIT`/`ROLLBACK`。

PostgreSQL 和 MySQL 默认自动提交，所以单独一条语句会立即提交。把一个工作单元包在
`BEGIN … COMMIT` 里，才能让它成为原子的。

---

## 5. IDE 的数据控件

在 PowerRustCOBOL 的窗体设计器中，一个 **SqlDatabase** 控件会自动生成样板段落
（`<id>-CONNECT`、`<id>-EXEC`、`<id>-FETCH-ALL`、`<id>-CLOSE`）。有两个属性要紧：

- **`ConnectionString`** —— 上面任何一种连接字符串。运行时真正选定后端的正是它。
- **`Driver`** —— `sqlite`（默认）、`postgres` 或 `mysql`。纯属外观：它只是给生成
  的注释加标签；路由由连接字符串决定。

---

## 6. 安全与运维说明

- **TLS。** ⚠️ **目前两个 SQL 驱动都不会说 TLS。** MySQL 驱动是以
  `default-features = false, features = ["minimal-rust"]` 构建的，而解析出的
  `mysql 28` 根本没有引入任何 TLS crate——无论服务器要求什么，它都无法协商安全
  连接。同步版的 PostgreSQL 驱动在构造上就以 `NoTls` 连接。两者都适用于本地套接字
  和可信网络。若服务器要求 TLS，请在本地代理上终结它（例如
  `stunnel`/`pgbouncer`），或走 SSH 隧道。
- **SQL 注入。** 语句是以文本形式发送的。请用可信的输入来构造查询，或者在拼接 SQL
  字符串之前，先对任何用户提供的值做校验／转义。
- **连接的生命周期。** 每个句柄拥有一条活动连接。用 `COBOL-CLOSE-DB` 关闭你不再需要
  的句柄；程序终止时，所有仍然打开的连接都会被关闭。

---

## 7. 测试

- **离线（总是运行）：** 连接字符串的路由、值的规范化，以及一次完整的内存 SQLite
  CRUD 往返 —— `cargo test -p cobolt-runtime --lib db_runtime` 和
  `cargo test -p cobolt-runtime --test test_sql`。
- **真实服务器（需显式开启）：** 两个标记为 `#[ignore]` 的往返测试会连接真实服务器。
  提供一个 URL 并显式运行它们：

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. 实现

引擎位于 `crates/cobolt-runtime/src/db_runtime.rs`。`DbConn` 包装了一个 `Backend`
枚举（`Sqlite` / `Postgres` / `MySql`）；`BackendKind::classify` 根据连接字符串选择
后端。每个后端有自己的 `exec_*` 路径，把行规范化为 `Vec<Vec<String>>`，此后共享的
游标逻辑（`fetch_col` / `next_row` / `row_count`）就与后端无关了。解释器的
`exec_call`（`crates/cobolt-runtime/src/interpreter.rs`）把这六个 COBOL CALL 映射到
`DbRegistry`，后者按整数句柄对连接进行池化。

.<<

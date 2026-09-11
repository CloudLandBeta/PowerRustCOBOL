<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Ambiente de execução de bases de dados do RustCOBOL

Os programas RustCOBOL falam com bases de dados SQL através de um pequeno
conjunto de `CALL` incorporados. Os mesmos seis verbos funcionam contra **três
backends** — o motor é selecionado automaticamente a partir da cadeia de ligação,
pelo que um programa escrito para SQLite corre sem alterações contra PostgreSQL
ou MySQL bastando mudar um literal.

| Backend     | Controlador (não é preciso biblioteca do sistema)     | Cadeia de ligação                                  |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`, `features = ["bundled"]` — compila a amálgama em **C** do SQLite, pelo que este não é Rust puro | `:memory:`, `sqlite:<caminho>`, ou um caminho de ficheiro simples |
| **PostgreSQL** | `postgres` (rust-postgres, síncrono) | `postgres://user:pass@host:port/db`                |
| **MySQL**   | `mysql` (`minimal-rust`, síncrono, sem TLS) | `mysql://user:pass@host:port/db`             |

Os três controladores são ligados estaticamente e não exigem **nenhuma biblioteca
cliente externa** (`libpq`, `libmysqlclient`) nem **OpenSSL** para compilar — em
consonância com o resto do PowerRustCOBOL.

---

## 1. Cadeias de ligação

O backend é escolhido puramente a partir do esquema da cadeia de ligação:

| Forma                                      | Backend       | Notas                                  |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | Base de dados em RAM, descartada ao fechar. |
| `sqlite:/var/data/app.db`                  | SQLite        | O ficheiro é criado se não existir.    |
| `/var/data/app.db`                         | SQLite        | Um caminho simples é tratado como SQLite. |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` também é aceite.    |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

A comparação do esquema ignora maiúsculas e tolera espaços em redor. Tudo o que
**não** for um URL `postgres(ql)://` ou `mysql://` é tratado como um destino
SQLite.

---

## 2. A superfície de CALL

Todas as CALL passam os seus argumentos `BY REFERENCE`. Os valores de estado e de
identificador vivem em itens de dados COBOL correntes, para poderem ser guardados
e passados entre parágrafos.

| Nome da CALL       | Argumentos (`BY REFERENCE`)                             |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | cadeia de ligação, variável de identificador `PIC 9(9)`, variável de estado |
| `COBOL-EXEC-SQL`   | identificador, consulta, variável de número de linhas `PIC 9(9)`, variável de estado |
| `COBOL-FETCH-ROW`  | identificador, índice de coluna `PIC 9(n)` (a partir de 1), variável de destino, estado |
| `COBOL-NEXT-ROW`   | identificador, variável de indicador de continuação `PIC X` (`Y`/`N`) |
| `COBOL-ROW-COUNT`  | identificador, variável de contagem `PIC 9(9)`          |
| `COBOL-CLOSE-DB`   | identificador                                           |

### Semântica

- O **`COBOL-OPEN-DB`** abre uma ligação e escreve um identificador inteiro
  positivo em *handle-var*. Em caso de sucesso, *status-var* fica com espaços; em
  caso de falha, *handle-var* é `0` e *status-var* contém a mensagem de erro do
  controlador.
- O **`COBOL-EXEC-SQL`** executa uma instrução sobre *handle*.
  - Para instruções que devolvem linhas (`SELECT`, CTE, …) todo o conjunto de
    resultados é colocado em cache e *row-count-var* recebe o **número de
    linhas**. O cursor começa na primeira linha.
  - Para `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* recebe o **número
    de linhas afetadas** e o conjunto de resultados fica vazio.
  - Em caso de erro, *status-var* contém a mensagem e *row-count-var* é `0`.
- O **`COBOL-FETCH-ROW`** copia a coluna *col-index* (a partir de 1) da linha
  **atual** para *dest-var* como texto. Colunas fora do intervalo e um cursor
  esgotado dão espaços.
- O **`COBOL-NEXT-ROW`** avança o cursor e põe *more-flag-var* a `Y` se já houver
  uma linha disponível, ou a `N` quando o conjunto se esgota.
- O **`COBOL-ROW-COUNT`** devolve a contagem de linhas em cache da última
  consulta.
- O **`COBOL-CLOSE-DB`** fecha a ligação e liberta o seu conjunto de resultados.
  Identificadores desconhecidos são ignorados. Todas as ligações abertas são
  fechadas quando o programa termina.

### Normalização de valores

Todo o valor de coluna — seja qual for o backend ou o tipo SQL — é entregue ao
COBOL como **texto**, para poder ser levado com `MOVE` diretamente para um campo
`PIC X` (ou para um campo numérico, que reinterpreta os dígitos). A normalização
é uniforme:

| Valor SQL      | Texto entregue ao COBOL                |
|----------------|----------------------------------------|
| `NULL`         | espaços (cadeia vazia)                 |
| inteiro        | dígitos decimais, p. ex. `42`, `-7`    |
| real / duplo   | a forma de ida e volta mais curta, p. ex. `3.14` |
| texto / varchar| a cadeia UTF-8                         |
| data           | `YYYY-MM-DD`                           |
| data e hora    | `YYYY-MM-DD HH:MM:SS`                  |
| hora (MySQL)   | `HH:MM:SS`                             |
| blob (SQLite)  | marcador `<blob N bytes>`              |

---

## 3. Exemplo — CRUD portável

Este programa corre contra **qualquer** um dos três backends; só `WS-CONN` muda.
É exatamente o programa exercitado pelo conjunto de testes
(`crates/cobolt-runtime/tests/test_sql.rs`).

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

Saída (SQLite em memória):

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### Ler várias colunas

O `COBOL-FETCH-ROW` lê uma coluna por chamada; mude `WS-COL` para ler outras da
mesma linha antes de avançar:

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. Transações

As transações são conduzidas com SQL corrente através do `COBOL-EXEC-SQL`, pelo
que o comportamento é exatamente o do seu servidor:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Os **verbos** COBOL `COMMIT` / `ROLLBACK` são uma funcionalidade separada que
> controla as transações de **ficheiros INDEXED** do RustCOBOL (ver
> [`docs/indexed-file-format-pt.md`](indexed-file-format-pt.md)). **Não** atuam
> sobre ligações SQL — para a base de dados use `COBOL-EXEC-SQL` com
> `BEGIN`/`COMMIT`/`ROLLBACK`, como se mostra acima.

O PostgreSQL e o MySQL usam autocommit por omissão, pelo que uma instrução
isolada é confirmada imediatamente. Envolva uma unidade de trabalho em
`BEGIN … COMMIT` para a tornar atómica.

---

## 5. O controlo de dados do IDE

No desenhador de formulários do PowerRustCOBOL, um controlo **SqlDatabase** gera
automaticamente os parágrafos repetitivos (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Duas propriedades importam:

- **`ConnectionString`** — qualquer uma das cadeias de ligação acima. É isto que
  realmente seleciona o backend em tempo de execução.
- **`Driver`** — `sqlite` (por omissão), `postgres` ou `mysql`. Apenas
  cosmético: rotula os comentários gerados; o encaminhamento é feito pela cadeia
  de ligação.

---

## 6. Notas de segurança e operação

- **TLS.** ⚠️ **Hoje nenhum dos controladores SQL fala TLS.** O controlador de
  MySQL é compilado com
  `default-features = false, features = ["minimal-rust"]`, e o `mysql 28`
  resolvido não puxa qualquer crate de TLS — não consegue negociar uma ligação
  segura, peça o servidor o que pedir. O controlador síncrono de PostgreSQL liga
  com `NoTls` por construção. Ambos servem para sockets locais e redes de
  confiança. Para um servidor que exija TLS, termine-o num proxy local (por
  exemplo `stunnel`/`pgbouncer`) ou passe por um túnel SSH.
- **Injeção de SQL.** As instruções são enviadas como texto. Construa as
  consultas a partir de entrada de confiança, ou valide/escape previamente
  quaisquer valores fornecidos pelo utilizador antes de compor a cadeia SQL.
- **Tempo de vida da ligação.** Cada identificador possui uma ligação viva. Feche
  com `COBOL-CLOSE-DB` os identificadores de que já não precisa; tudo o que ficar
  aberto é fechado quando o programa termina.

---

## 7. Testes

- **Offline (correm sempre):** o encaminhamento por cadeia de ligação, a
  normalização de valores e um CRUD completo de ida e volta com SQLite em memória
  — `cargo test -p cobolt-runtime --lib db_runtime` e
  `cargo test -p cobolt-runtime --test test_sql`.
- **Servidores reais (opcionais):** dois testes de ida e volta marcados
  `#[ignore]` ligam-se a servidores a sério. Forneça um URL e execute-os
  explicitamente:

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implementação

O `crates/cobolt-runtime/src/db_runtime.rs` contém o motor. Um `DbConn` envolve um
enum `Backend` (`Sqlite` / `Postgres` / `MySql`); o `BackendKind::classify`
escolhe o backend a partir da cadeia de ligação. Cada backend tem o seu próprio
caminho `exec_*` que normaliza as linhas para `Vec<Vec<String>>`, após o que a
lógica partilhada de cursor (`fetch_col` / `next_row` / `row_count`) é
independente do backend. O `exec_call` do interpretador
(`crates/cobolt-runtime/src/interpreter.rs`) mapeia as seis CALL de COBOL sobre o
`DbRegistry`, que agrupa as ligações por identificador inteiro.

.<<

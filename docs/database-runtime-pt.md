<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Runtime de banco de dados do RustCOBOL

Programas RustCOBOL conversam com bancos de dados SQL através de um pequeno
conjunto de `CALL`s embutidos. Os mesmos seis verbos funcionam contra **três
backends** — o motor é escolhido automaticamente a partir da string de conexão,
de modo que um programa escrito para SQLite roda sem alteração contra PostgreSQL
ou MySQL bastando trocar um literal.

| Backend     | Driver (puro Rust, sem biblioteca do sistema) | String de conexão                                     |
|-------------|-----------------------------------------------|--------------------------------------------------------|
| **SQLite**  | `rusqlite` (SQLite embutido)                  | `:memory:`, `sqlite:<path>` ou um caminho de arquivo simples |
| **PostgreSQL** | `postgres` (rust-postgres, síncrono)       | `postgres://user:pass@host:port/db`                    |
| **MySQL**   | `mysql` (rustls, síncrono)                    | `mysql://user:pass@host:port/db`                       |

Os três drivers são ligados estaticamente e não exigem **nenhuma biblioteca
cliente externa** (`libpq`, `libmysqlclient`) **nem OpenSSL** para compilar — em
linha com o restante do PowerRustCOBOL.

---

## 1. Strings de conexão

O backend é escolhido puramente pelo esquema da string de conexão:

| Forma                                      | Backend       | Observações                                |
|--------------------------------------------|---------------|--------------------------------------------|
| `:memory:`                                 | SQLite        | Banco em RAM, descartado ao fechar.        |
| `sqlite:/var/data/app.db`                  | SQLite        | O arquivo é criado se não existir.         |
| `/var/data/app.db`                         | SQLite        | Um caminho simples é tratado como SQLite.  |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` também é aceito.        |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                            |

A comparação do esquema ignora maiúsculas e minúsculas e tolera espaços em
branco ao redor. Tudo o que **não** for uma URL `postgres(ql)://` ou `mysql://` é
tratado como um destino SQLite.

---

## 2. A superfície de CALL

Todo CALL passa seus argumentos `BY REFERENCE`. Os valores de status e de
descritor ficam em itens de dados COBOL comuns, para que possam ser guardados e
passados entre parágrafos.

| Nome do CALL       | Argumentos (`BY REFERENCE`)                              |
|--------------------|----------------------------------------------------------|
| `COBOL-OPEN-DB`    | conn-string, handle-var `PIC 9(9)`, status-var           |
| `COBOL-EXEC-SQL`   | handle, query, row-count-var `PIC 9(9)`, status-var      |
| `COBOL-FETCH-ROW`  | handle, col-index `PIC 9(n)` (base 1), dest-var, status  |
| `COBOL-NEXT-ROW`   | handle, more-flag-var `PIC X` (`Y`/`N`)                  |
| `COBOL-ROW-COUNT`  | handle, count-var `PIC 9(9)`                             |
| `COBOL-CLOSE-DB`   | handle                                                   |

### Semântica

- **`COBOL-OPEN-DB`** abre uma conexão e escreve um descritor inteiro positivo em
  *handle-var*. Em caso de sucesso, *status-var* fica com espaços; em caso de
  falha, *handle-var* é `0` e *status-var* contém a mensagem de erro do driver.
- **`COBOL-EXEC-SQL`** executa um comando sobre *handle*.
  - Para comandos que retornam linhas (`SELECT`, CTEs, …) todo o conjunto de
    resultados é mantido em cache e *row-count-var* recebe o **número de linhas**.
    O cursor começa na primeira linha.
  - Para `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* recebe o **número
    de linhas afetadas** e o conjunto de resultados fica vazio.
  - Em caso de erro, *status-var* contém a mensagem e *row-count-var* é `0`.
- **`COBOL-FETCH-ROW`** copia a coluna *col-index* (base 1) da linha **atual**
  para *dest-var* como texto. Colunas fora do intervalo e um cursor esgotado
  devolvem espaços.
- **`COBOL-NEXT-ROW`** avança o cursor e coloca `Y` em *more-flag-var* se já
  houver uma linha disponível, ou `N` quando o conjunto se esgota.
- **`COBOL-ROW-COUNT`** devolve a contagem de linhas em cache da última consulta.
- **`COBOL-CLOSE-DB`** fecha a conexão e libera seu conjunto de resultados.
  Descritores desconhecidos são ignorados. Todas as conexões abertas são fechadas
  quando o programa termina.

### Normalização de valores

Todo valor de coluna — não importa o backend nem o tipo SQL — é entregue ao COBOL
como **texto**, para que possa ser levado com `MOVE` direto para um campo `PIC X`
(ou para um campo numérico, que reinterpreta os dígitos). A normalização é
uniforme:

| Valor SQL      | Texto entregue ao COBOL                        |
|----------------|------------------------------------------------|
| `NULL`         | espaços (string vazia)                         |
| integer        | dígitos decimais, por exemplo `42`, `-7`       |
| real / double  | a forma mais curta de ida e volta, por exemplo `3.14` |
| text / varchar | a string UTF-8                                 |
| date           | `YYYY-MM-DD`                                   |
| datetime       | `YYYY-MM-DD HH:MM:SS`                          |
| time (MySQL)   | `HH:MM:SS`                                     |
| blob (SQLite)  | marcador `<blob N bytes>`                      |

---

## 3. Exemplo — CRUD portável

Este programa roda contra **qualquer** um dos três backends; só `WS-CONN` muda.
É exatamente o programa exercitado pela suíte de testes
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

### Lendo várias colunas

`COBOL-FETCH-ROW` lê uma coluna por chamada; mude `WS-COL` para ler outras da
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

As transações são conduzidas com SQL comum por meio de `COBOL-EXEC-SQL`, então o
comportamento é exatamente o do seu servidor:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Os **verbos** COBOL `COMMIT` / `ROLLBACK` são um recurso separado, que controla
> as transações de **arquivos INDEXED** do RustCOBOL (veja
> [`docs/indexed-file-format-pt.md`](indexed-file-format-pt.md)). Eles **não**
> atuam sobre conexões SQL — para o banco de dados use `COBOL-EXEC-SQL` com
> `BEGIN`/`COMMIT`/`ROLLBACK`, como mostrado acima.

PostgreSQL e MySQL usam autocommit por padrão, então um comando isolado é
confirmado imediatamente. Envolva uma unidade de trabalho em `BEGIN … COMMIT`
para torná-la atômica.

---

## 5. O controle de dados do IDE

No designer de formulários do PowerRustCOBOL, um controle **SqlDatabase** gera
automaticamente os parágrafos repetitivos (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Duas propriedades importam:

- **`ConnectionString`** — qualquer uma das strings de conexão acima. É ela que
  de fato seleciona o backend em tempo de execução.
- **`Driver`** — `sqlite` (padrão), `postgres` ou `mysql`. Apenas cosmético: ele
  rotula os comentários gerados; o roteamento é feito pela string de conexão.

---

## 6. Notas de segurança e operação

- **TLS.** O driver MySQL é compilado com rustls e negocia TLS quando o servidor
  pede. O driver síncrono do PostgreSQL conecta **sem TLS** (`NoTls`) — adequado
  para sockets locais e redes confiáveis. Para um servidor PostgreSQL que exija
  TLS, termine o TLS em um proxy local (por exemplo `stunnel`/`pgbouncer`) ou
  trafegue por um túnel SSH.
- **Injeção de SQL.** Os comandos são enviados como texto. Monte as consultas a
  partir de entradas confiáveis, ou valide/escape previamente qualquer valor
  fornecido pelo usuário antes de compor a string SQL.
- **Tempo de vida da conexão.** Cada descritor é dono de uma conexão viva. Feche
  com `COBOL-CLOSE-DB` os descritores de que não precisa mais; tudo o que ficar
  aberto é fechado quando o programa termina.

---

## 7. Testes

- **Offline (sempre executados):** roteamento da string de conexão, normalização
  de valores e um CRUD completo de ida e volta em SQLite na memória —
  `cargo test -p cobolt-runtime --lib db_runtime` e
  `cargo test -p cobolt-runtime --test test_sql`.
- **Servidores reais (opcional):** dois testes de ida e volta marcados com
  `#[ignore]` conectam a servidores de verdade. Forneça uma URL e execute-os
  explicitamente:

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implementação

`crates/cobolt-runtime/src/db_runtime.rs` contém o motor. Um `DbConn` envolve um
enum `Backend` (`Sqlite` / `Postgres` / `MySql`); `BackendKind::classify` escolhe
o backend a partir da string de conexão. Cada backend tem seu próprio caminho
`exec_*`, que normaliza as linhas para `Vec<Vec<String>>`, depois do que a lógica
de cursor compartilhada (`fetch_col` / `next_row` / `row_count`) independe do
backend. O `exec_call` do interpretador
(`crates/cobolt-runtime/src/interpreter.rs`) mapeia os seis CALLs do COBOL sobre
o `DbRegistry`, que mantém um pool de conexões indexado por descritor inteiro.

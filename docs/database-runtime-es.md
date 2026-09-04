<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Runtime de bases de datos de RustCOBOL

Los programas RustCOBOL se comunican con bases de datos SQL mediante un pequeño
conjunto de `CALL`s integrados. Los mismos seis verbos funcionan contra **tres
backends**: el motor se selecciona automáticamente a partir de la cadena de
conexión, así que un programa escrito para SQLite se ejecuta sin cambios contra
PostgreSQL o MySQL con solo cambiar un literal.

| Backend     | Controlador (Rust puro, sin biblioteca del sistema) | Cadena de conexión                                        |
|-------------|-----------------------------------------------------|-----------------------------------------------------------|
| **SQLite**  | `rusqlite` (SQLite incorporado)                     | `:memory:`, `sqlite:<path>` o una ruta de archivo simple  |
| **PostgreSQL** | `postgres` (rust-postgres, síncrono)             | `postgres://user:pass@host:port/db`                       |
| **MySQL**   | `mysql` (rustls, síncrono)                          | `mysql://user:pass@host:port/db`                          |

Los tres controladores se enlazan estáticamente y no necesitan **ninguna
biblioteca cliente externa** (`libpq`, `libmysqlclient`) **ni OpenSSL** para
compilar, en coherencia con el resto de PowerRustCOBOL.

---

## 1. Cadenas de conexión

El backend se elige únicamente a partir del esquema de la cadena de conexión:

| Forma                                      | Backend       | Notas                                     |
|--------------------------------------------|---------------|-------------------------------------------|
| `:memory:`                                 | SQLite        | Base de datos en RAM, descartada al cerrar. |
| `sqlite:/var/data/app.db`                  | SQLite        | El archivo se crea si no existe.          |
| `/var/data/app.db`                         | SQLite        | Una ruta simple se trata como SQLite.     |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | También se acepta `postgresql://`.     |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                           |

La comparación del esquema no distingue mayúsculas de minúsculas y tolera
espacios en blanco alrededor. Todo lo que **no** sea una URL `postgres(ql)://` o
`mysql://` se trata como un destino SQLite.

---

## 2. La superficie de CALL

Cada CALL pasa sus argumentos `BY REFERENCE`. Los valores de estado y de
descriptor viven en elementos de datos COBOL corrientes, de modo que se pueden
conservar y pasar entre párrafos.

| Nombre del CALL    | Argumentos (`BY REFERENCE`)                             |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | conn-string, handle-var `PIC 9(9)`, status-var          |
| `COBOL-EXEC-SQL`   | handle, query, row-count-var `PIC 9(9)`, status-var     |
| `COBOL-FETCH-ROW`  | handle, col-index `PIC 9(n)` (base 1), dest-var, status |
| `COBOL-NEXT-ROW`   | handle, more-flag-var `PIC X` (`Y`/`N`)                 |
| `COBOL-ROW-COUNT`  | handle, count-var `PIC 9(9)`                            |
| `COBOL-CLOSE-DB`   | handle                                                  |

### Semántica

- **`COBOL-OPEN-DB`** abre una conexión y escribe un descriptor entero positivo
  en *handle-var*. Si tiene éxito, *status-var* queda en espacios; si falla,
  *handle-var* vale `0` y *status-var* contiene el mensaje de error del controlador.
- **`COBOL-EXEC-SQL`** ejecuta una sentencia sobre *handle*.
  - Para las sentencias que devuelven filas (`SELECT`, CTEs, …) se guarda en
    caché todo el conjunto de resultados y *row-count-var* recibe el **número de
    filas**. El cursor empieza en la primera fila.
  - Para `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* recibe el
    **número de filas afectadas** y el conjunto de resultados queda vacío.
  - En caso de error, *status-var* contiene el mensaje y *row-count-var* vale `0`.
- **`COBOL-FETCH-ROW`** copia la columna *col-index* (base 1) de la fila
  **actual** en *dest-var* como texto. Las columnas fuera de rango y un cursor
  agotado devuelven espacios.
- **`COBOL-NEXT-ROW`** avanza el cursor y pone *more-flag-var* en `Y` si ya hay
  una fila disponible, o en `N` cuando el conjunto se ha agotado.
- **`COBOL-ROW-COUNT`** devuelve el recuento de filas en caché de la última consulta.
- **`COBOL-CLOSE-DB`** cierra la conexión y libera su conjunto de resultados. Los
  descriptores desconocidos se ignoran. Todas las conexiones abiertas se cierran
  cuando el programa termina.

### Normalización de valores

Cada valor de columna —sea cual sea el backend o el tipo SQL— se entrega a COBOL
como **texto**, de forma que se puede `MOVE` directamente a un campo `PIC X` (o a
un campo numérico, que reinterpreta los dígitos). La normalización es uniforme:

| Valor SQL      | Texto entregado a COBOL                        |
|----------------|------------------------------------------------|
| `NULL`         | espacios (cadena vacía)                        |
| integer        | dígitos decimales, p. ej. `42`, `-7`           |
| real / double  | la forma más corta de ida y vuelta, p. ej. `3.14` |
| text / varchar | la cadena UTF-8                                |
| date           | `YYYY-MM-DD`                                   |
| datetime       | `YYYY-MM-DD HH:MM:SS`                          |
| time (MySQL)   | `HH:MM:SS`                                     |
| blob (SQLite)  | marcador `<blob N bytes>`                      |

---

## 3. Ejemplo — CRUD portable

Este programa se ejecuta contra **cualquiera** de los tres backends; solo cambia
`WS-CONN`. Es exactamente el programa que ejercita la batería de pruebas
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

Salida (SQLite en memoria):

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### Lectura de varias columnas

`COBOL-FETCH-ROW` lee una columna por llamada; cambia `WS-COL` para leer otras
de la misma fila antes de avanzar:

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. Transacciones

Las transacciones se gobiernan con SQL corriente a través de `COBOL-EXEC-SQL`,
así que el comportamiento es exactamente el de tu servidor:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Los **verbos** COBOL `COMMIT` / `ROLLBACK` son una funcionalidad aparte que
> controla las transacciones de **archivos INDEXED** de RustCOBOL (véase
> [`docs/indexed-file-format-es.md`](indexed-file-format-es.md)). **No** actúan
> sobre las conexiones SQL: para la base de datos usa `COBOL-EXEC-SQL` con
> `BEGIN`/`COMMIT`/`ROLLBACK`, como se muestra arriba.

PostgreSQL y MySQL usan autocommit por defecto, así que una sentencia suelta se
confirma de inmediato. Envuelve una unidad de trabajo en `BEGIN … COMMIT` para
hacerla atómica.

---

## 5. El control de datos del IDE

En el diseñador de formularios de PowerRustCOBOL, un control **SqlDatabase**
genera automáticamente los párrafos repetitivos (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Importan dos propiedades:

- **`ConnectionString`** — cualquiera de las cadenas de conexión anteriores. Es
  lo que realmente selecciona el backend en tiempo de ejecución.
- **`Driver`** — `sqlite` (por defecto), `postgres` o `mysql`. Solo cosmético:
  etiqueta los comentarios generados; el enrutamiento lo decide la cadena de conexión.

---

## 6. Notas de seguridad y operación

- **TLS.** El controlador de MySQL se compila con rustls y negocia TLS cuando el
  servidor lo solicita. El controlador síncrono de PostgreSQL se conecta **sin
  TLS** (`NoTls`), lo que resulta adecuado para sockets locales y redes de
  confianza. Para un servidor PostgreSQL que exija TLS, termina el TLS en un
  proxy local (p. ej. `stunnel`/`pgbouncer`) o trabaja sobre un túnel SSH.
- **Inyección SQL.** Las sentencias se envían como texto. Construye las consultas
  a partir de entradas de confianza, o valida y escapa de antemano cualquier
  valor proporcionado por el usuario antes de componer la cadena SQL.
- **Vida de la conexión.** Cada descriptor posee una conexión viva. Cierra con
  `COBOL-CLOSE-DB` los descriptores que ya no necesites; todo lo que quede
  abierto se cierra cuando el programa termina.

---

## 7. Pruebas

- **Sin conexión (siempre se ejecutan):** enrutamiento de la cadena de conexión,
  normalización de valores y un CRUD completo de ida y vuelta sobre SQLite en
  memoria — `cargo test -p cobolt-runtime --lib db_runtime` y
  `cargo test -p cobolt-runtime --test test_sql`.
- **Servidores reales (opcional):** dos pruebas de ida y vuelta marcadas con
  `#[ignore]` se conectan a servidores de verdad. Proporciona una URL y
  ejecútalas explícitamente:

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implementación

`crates/cobolt-runtime/src/db_runtime.rs` contiene el motor. Un `DbConn` envuelve
un enum `Backend` (`Sqlite` / `Postgres` / `MySql`); `BackendKind::classify`
elige el backend a partir de la cadena de conexión. Cada backend tiene su propia
ruta `exec_*`, que normaliza las filas a `Vec<Vec<String>>`, tras lo cual la
lógica de cursor compartida (`fetch_col` / `next_row` / `row_count`) es
independiente del backend. El `exec_call` del intérprete
(`crates/cobolt-runtime/src/interpreter.rs`) proyecta los seis CALLs de COBOL
sobre `DbRegistry`, que agrupa las conexiones por descriptor entero.

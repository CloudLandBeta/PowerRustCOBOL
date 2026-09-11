<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Entorno de ejecución de bases de datos de RustCOBOL

Los programas RustCOBOL hablan con bases de datos SQL mediante un pequeño juego
de `CALL` incorporados. Los mismos seis verbos funcionan contra **tres
backends**: el motor se selecciona automáticamente a partir de la cadena de
conexión, así que un programa escrito para SQLite se ejecuta sin cambios contra
PostgreSQL o MySQL con solo cambiar un literal.

| Backend     | Controlador (no hace falta biblioteca del sistema)     | Cadena de conexión                                 |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`, `features = ["bundled"]` — compila la amalgama en **C** de SQLite, así que este no es Rust puro | `:memory:`, `sqlite:<ruta>` o una ruta de fichero a secas |
| **PostgreSQL** | `postgres` (rust-postgres, síncrono) | `postgres://user:pass@host:port/db`                |
| **MySQL**   | `mysql` (`minimal-rust`, síncrono, sin TLS) | `mysql://user:pass@host:port/db`             |

Los tres controladores se enlazan estáticamente y no requieren **ninguna
biblioteca cliente externa** (`libpq`, `libmysqlclient`) ni **OpenSSL** para
compilar, en consonancia con el resto de PowerRustCOBOL.

---

## 1. Cadenas de conexión

El backend se elige únicamente a partir del esquema de la cadena de conexión:

| Forma                                      | Backend       | Notas                                  |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | Base de datos en RAM, descartada al cerrar. |
| `sqlite:/var/data/app.db`                  | SQLite        | El fichero se crea si no existe.       |
| `/var/data/app.db`                         | SQLite        | Una ruta a secas se trata como SQLite. |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | También se acepta `postgresql://`.  |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

La comparación del esquema no distingue mayúsculas y tolera espacios alrededor.
Todo lo que **no** sea una URL `postgres(ql)://` o `mysql://` se trata como un
destino SQLite.

---

## 2. La superficie de CALL

Todas las CALL pasan sus argumentos `BY REFERENCE`. Los valores de estado y de
manejador viven en elementos de datos COBOL corrientes, de modo que pueden
conservarse y pasarse entre párrafos.

| Nombre de la CALL  | Argumentos (`BY REFERENCE`)                             |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | cadena de conexión, variable de manejador `PIC 9(9)`, variable de estado |
| `COBOL-EXEC-SQL`   | manejador, consulta, variable de número de filas `PIC 9(9)`, variable de estado |
| `COBOL-FETCH-ROW`  | manejador, índice de columna `PIC 9(n)` (desde 1), variable destino, estado |
| `COBOL-NEXT-ROW`   | manejador, variable de indicador de continuación `PIC X` (`Y`/`N`) |
| `COBOL-ROW-COUNT`  | manejador, variable de recuento `PIC 9(9)`              |
| `COBOL-CLOSE-DB`   | manejador                                               |

### Semántica

- **`COBOL-OPEN-DB`** abre una conexión y escribe un manejador entero positivo en
  *handle-var*. Si tiene éxito, *status-var* queda con espacios; si falla,
  *handle-var* es `0` y *status-var* contiene el mensaje de error del
  controlador.
- **`COBOL-EXEC-SQL`** ejecuta una sentencia sobre *handle*.
  - Para sentencias que devuelven filas (`SELECT`, CTE, …) se almacena en caché
    todo el conjunto de resultados y *row-count-var* recibe el **número de
    filas**. El cursor arranca en la primera fila.
  - Para `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* recibe el **número
    de filas afectadas** y el conjunto de resultados queda vacío.
  - En caso de error, *status-var* contiene el mensaje y *row-count-var* es `0`.
- **`COBOL-FETCH-ROW`** copia la columna *col-index* (desde 1) de la fila
  **actual** en *dest-var* como texto. Las columnas fuera de rango y un cursor
  agotado dan espacios.
- **`COBOL-NEXT-ROW`** avanza el cursor y pone *more-flag-var* a `Y` si ya hay
  una fila disponible, o a `N` cuando el conjunto se agota.
- **`COBOL-ROW-COUNT`** devuelve el recuento de filas en caché de la última
  consulta.
- **`COBOL-CLOSE-DB`** cierra la conexión y libera su conjunto de resultados. Los
  manejadores desconocidos se ignoran. Todas las conexiones abiertas se cierran
  cuando termina el programa.

### Normalización de valores

Todo valor de columna —sea cual sea el backend o el tipo SQL— se entrega a COBOL
como **texto**, de modo que puede hacerse `MOVE` directamente a un campo `PIC X`
(o a un campo numérico, que reinterpreta los dígitos). La normalización es
uniforme:

| Valor SQL      | Texto entregado a COBOL                |
|----------------|----------------------------------------|
| `NULL`         | espacios (cadena vacía)                |
| entero         | dígitos decimales, p. ej. `42`, `-7`   |
| real / doble   | la forma de ida y vuelta más corta, p. ej. `3.14` |
| texto / varchar| la cadena UTF-8                        |
| fecha          | `YYYY-MM-DD`                           |
| fecha y hora   | `YYYY-MM-DD HH:MM:SS`                  |
| hora (MySQL)   | `HH:MM:SS`                             |
| blob (SQLite)  | marcador `<blob N bytes>`              |

---

## 3. Ejemplo — CRUD portable

Este programa se ejecuta contra **cualquiera** de los tres backends; solo cambia
`WS-CONN`. Es exactamente el programa que ejercita el conjunto de pruebas
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

### Leer varias columnas

`COBOL-FETCH-ROW` lee una columna por llamada; cambie `WS-COL` para leer otras de
la misma fila antes de avanzar:

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. Transacciones

Las transacciones se manejan con SQL corriente a través de `COBOL-EXEC-SQL`, así
que el comportamiento es exactamente el de su servidor:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Los **verbos** COBOL `COMMIT` / `ROLLBACK` son una funcionalidad distinta que
> controla las transacciones de **ficheros INDEXED** de RustCOBOL (véase
> [`docs/indexed-file-format-es.md`](indexed-file-format-es.md)). **No** actúan
> sobre conexiones SQL: para la base de datos use `COBOL-EXEC-SQL` con
> `BEGIN`/`COMMIT`/`ROLLBACK`, como se muestra arriba.

PostgreSQL y MySQL van por defecto en autoconfirmación, así que una sentencia
suelta se confirma de inmediato. Envuelva una unidad de trabajo en
`BEGIN … COMMIT` para hacerla atómica.

---

## 5. El control de datos del IDE

En el diseñador de formularios de PowerRustCOBOL, un control **SqlDatabase**
genera automáticamente los párrafos repetitivos (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Importan dos propiedades:

- **`ConnectionString`** — cualquiera de las cadenas de conexión de arriba. Esto
  es lo que realmente selecciona el backend en tiempo de ejecución.
- **`Driver`** — `sqlite` (por defecto), `postgres` o `mysql`. Solo es cosmético:
  etiqueta los comentarios generados; el enrutado lo decide la cadena de
  conexión.

---

## 6. Notas de seguridad y operación

- **TLS.** ⚠️ **Hoy ninguno de los dos controladores SQL habla TLS.** El
  controlador de MySQL se compila con
  `default-features = false, features = ["minimal-rust"]`, y el `mysql 28`
  resuelto no arrastra ningún crate de TLS: no puede negociar una conexión
  segura, pida lo que pida el servidor. El controlador síncrono de PostgreSQL se
  conecta con `NoTls` por construcción. Ambos sirven para sockets locales y redes
  de confianza. Para un servidor que exija TLS, termínelo en un proxy local (por
  ejemplo `stunnel`/`pgbouncer`) o pase por un túnel SSH.
- **Inyección de SQL.** Las sentencias se envían como texto. Construya las
  consultas a partir de entradas de confianza, o valide y escape de antemano
  cualquier valor suministrado por el usuario antes de componer la cadena SQL.
- **Vida de la conexión.** Cada manejador posee una conexión viva. Cierre con
  `COBOL-CLOSE-DB` los manejadores que ya no necesite; todo lo que quede abierto
  se cierra al terminar el programa.

---

## 7. Pruebas

- **Sin conexión (siempre se ejecutan):** el enrutado por cadena de conexión, la
  normalización de valores y un CRUD completo de ida y vuelta con SQLite en
  memoria — `cargo test -p cobolt-runtime --lib db_runtime` y
  `cargo test -p cobolt-runtime --test test_sql`.
- **Servidores reales (opcionales):** dos pruebas de ida y vuelta marcadas
  `#[ignore]` se conectan a servidores de verdad. Proporcione una URL y
  ejecútelas explícitamente:

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
elige el backend a partir de la cadena de conexión. Cada backend tiene su propio
camino `exec_*` que normaliza las filas a `Vec<Vec<String>>`, tras lo cual la
lógica compartida de cursor (`fetch_col` / `next_row` / `row_count`) es
independiente del backend. El `exec_call` del intérprete
(`crates/cobolt-runtime/src/interpreter.rs`) mapea las seis CALL de COBOL sobre
`DbRegistry`, que agrupa las conexiones por manejador entero.

.<<

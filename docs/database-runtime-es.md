<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Runtime de bases de datos de RustCOBOL

Los programas RustCOBOL hablan con bases de datos SQL mediante un pequeño
conjunto de `CALL` integrados. Los mismos seis verbos funcionan contra **tres
backends** — el motor se selecciona automáticamente a partir de la cadena de
conexión, de modo que un programa escrito para SQLite se ejecuta sin cambios
contra PostgreSQL o MySQL con sólo cambiar un literal.

| Backend | Driver (Rust puro, sin biblioteca del sistema) | Cadena de conexión |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite` (SQLite incluido)          | `:memory:`, `sqlite:<ruta>`, o una ruta de fichero simple |
| **PostgreSQL** | `postgres` (rust-postgres, síncrono) | `postgres://usuario:clave@host:puerto/bd`       |
| **MySQL**   | `mysql` (rustls, síncrono)            | `mysql://usuario:clave@host:puerto/bd`             |

Los tres drivers se enlazan estáticamente y no requieren **ninguna biblioteca
cliente externa** (`libpq`, `libmysqlclient`) **ni OpenSSL** para compilar, en
consonancia con el resto de PowerRustCOBOL.

---

## 1. Cadenas de conexión

El backend se elige únicamente a partir del esquema de la cadena de conexión:

| Forma | Backend | Notas |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | Base de datos en RAM, descartada al cerrar. |
| `sqlite:/var/data/app.db`                  | SQLite        | El fichero se crea si no existe.       |
| `/var/data/app.db`                         | SQLite        | Una ruta simple se trata como SQLite.  |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | También se acepta `postgresql://`.  |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

La comparación no distingue mayúsculas de minúsculas en el esquema y tolera
espacios alrededor. Todo lo que **no** sea una URL `postgres(ql)://` o
`mysql://` se trata como un destino SQLite.

---

## 2. La superficie de CALL

Cada CALL pasa sus argumentos `BY REFERENCE`. Los valores de estado y de
manejador viven en data items COBOL corrientes, de modo que pueden conservarse y
pasarse entre párrafos.

| Nombre del CALL | Argumentos (`BY REFERENCE`) |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | cadena-conexión, var-manejador `PIC 9(9)`, var-estado   |
| `COBOL-EXEC-SQL`   | manejador, consulta, var-nº-filas `PIC 9(9)`, var-estado |
| `COBOL-FETCH-ROW`  | manejador, índice-columna `PIC 9(n)` (base 1), var-destino, estado |
| `COBOL-NEXT-ROW`   | manejador, var-indicador-más `PIC X` (`Y`/`N`)          |
| `COBOL-ROW-COUNT`  | manejador, var-cuenta `PIC 9(9)`                        |
| `COBOL-CLOSE-DB`   | manejador                                               |

### Semántica

- **`COBOL-OPEN-DB`** abre una conexión y escribe un manejador entero positivo
  en *var-manejador*. Si tiene éxito, *var-estado* queda con espacios; si falla,
  *var-manejador* es `0` y *var-estado* contiene el mensaje de error del driver.
- **`COBOL-EXEC-SQL`** ejecuta una sentencia sobre *manejador*.
  - Para sentencias que devuelven filas (`SELECT`, CTE, …) se cachea el conjunto
    de resultados completo y *var-nº-filas* recibe el **número de filas**. El
    cursor comienza en la primera fila.
  - Para `INSERT` / `UPDATE` / `DELETE` / DDL, *var-nº-filas* recibe el **número
    de filas afectadas** y el conjunto de resultados queda vacío.
  - En caso de error, *var-estado* contiene el mensaje y *var-nº-filas* es `0`.
- **`COBOL-FETCH-ROW`** copia la columna *índice-columna* (base 1) de la fila
  **actual** dentro de *var-destino* como texto. Las columnas fuera de rango y un
  cursor agotado dan espacios.
- **`COBOL-NEXT-ROW`** avanza el cursor y pone *var-indicador-más* a `Y` si hay
  una fila disponible o a `N` una vez agotado el conjunto.
- **`COBOL-ROW-COUNT`** devuelve la cuenta de filas cacheada de la última
  consulta.
- **`COBOL-CLOSE-DB`** cierra la conexión y libera su conjunto de resultados.
  Los manejadores desconocidos se ignoran. Todas las conexiones abiertas se
  cierran cuando termina el programa.

### Normalización de valores

Todo valor de columna — sea cual sea el backend o el tipo SQL — se entrega a
COBOL como **texto**, de modo que puede hacerse `MOVE` directamente a un campo
`PIC X` (o a un campo numérico, que reinterpreta los dígitos). La normalización
es uniforme:

| Valor SQL | Texto entregado a COBOL |
|----------------|----------------------------------------|
| `NULL`         | espacios (cadena vacía)                |
| entero         | dígitos decimales, p. ej. `42`, `-7`   |
| real / doble   | forma de ida y vuelta más corta, p. ej. `3.14` |
| text / varchar | la cadena UTF-8                        |
| date           | `YYYY-MM-DD`                           |
| datetime       | `YYYY-MM-DD HH:MM:SS`                  |
| time (MySQL)   | `HH:MM:SS`                             |
| blob (SQLite)  | marcador `<blob N bytes>`              |

---

## 3. Ejemplo — CRUD portable

Este programa se ejecuta contra **cualquiera** de los tres backends; sólo cambia
`WS-CONN`. Es exactamente el programa que ejercita la batería de tests
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

`COBOL-FETCH-ROW` lee una columna por llamada; cambie `WS-COL` para leer otras
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
así que el comportamiento es exactamente el de su servidor:

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Los **verbos** COBOL `COMMIT` / `ROLLBACK` son una funcionalidad distinta que
> controla las transacciones de **ficheros INDEXED** de RustCOBOL (véase
> [`docs/indexed-file-format.md`](indexed-file-format.md)). **No** actúan sobre
> conexiones SQL — para la base de datos use `COBOL-EXEC-SQL` con
> `BEGIN`/`COMMIT`/`ROLLBACK`, como se muestra arriba.

PostgreSQL y MySQL usan autocommit por defecto, así que una sentencia suelta se
confirma inmediatamente. Envuelva una unidad de trabajo en `BEGIN … COMMIT` para
hacerla atómica.

---

## 5. El control de datos de la IDE

En el form designer de PowerRustCOBOL, un control **SqlDatabase** genera
automáticamente los párrafos de andamiaje (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Importan dos properties:

- **`ConnectionString`** — cualquiera de las cadenas de conexión anteriores. Es
  lo que realmente selecciona el backend en tiempo de ejecución.
- **`Driver`** — `sqlite` (por defecto), `postgres` o `mysql`. Sólo cosmético:
  etiqueta los comentarios generados; el enrutado lo decide la cadena de
  conexión.

---

## 6. Notas de seguridad y de operación

- **TLS.** El driver de MySQL está compilado con rustls y negocia TLS cuando el
  servidor lo solicita. El driver síncrono de PostgreSQL conecta **sin TLS**
  (`NoTls`) — adecuado para sockets locales y redes de confianza. Para un
  servidor PostgreSQL que exija TLS, termine el TLS en un proxy local (por
  ejemplo `stunnel`/`pgbouncer`) o trabaje sobre un túnel SSH.
- **Inyección SQL.** Las sentencias se envían como texto. Construya las consultas
  a partir de entrada de confianza, o valide/escape previamente cualquier valor
  suministrado por el usuario antes de componer la cadena SQL.
- **Vida de la conexión.** Cada manejador posee una conexión viva. Cierre con
  `COBOL-CLOSE-DB` los manejadores que ya no necesite; todo lo que quede abierto
  se cierra cuando el programa termina.

---

## 7. Tests

- **Sin conexión (siempre se ejecutan):** enrutado de la cadena de conexión,
  normalización de valores y un CRUD completo de ida y vuelta sobre SQLite en
  memoria — `cargo test -p cobolt-runtime --lib db_runtime` y
  `cargo test -p cobolt-runtime --test test_sql`.
- **Servidores reales (opcionales):** dos tests de ida y vuelta marcados con
  `#[ignore]` conectan con servidores reales. Proporcione una URL y ejecútelos
  explícitamente:

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
lógica compartida del cursor (`fetch_col` / `next_row` / `row_count`) es
independiente del backend. El `exec_call` del intérprete
(`crates/cobolt-runtime/src/interpreter.rs`) mapea los seis CALL de COBOL sobre
`DbRegistry`, que agrupa las conexiones por manejador entero.

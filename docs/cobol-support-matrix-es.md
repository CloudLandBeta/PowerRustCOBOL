<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.135 -->

# Matriz de soporte de PowerRustCOBOL

**Para qué sirve este documento:** un único sitio, hojeable, que responde a
*«¿hace PowerRustCOBOL X, y X es COBOL estándar o algo que añade esta
plataforma?»*. Cada capacidad es una fila. Sin listas en prosa: si algo está
soportado, tiene una línea que se puede señalar.

Esta es la **panorámica**. El detalle lo llevan dos documentos acompañantes:

| Documento | Qué responde |
|---|---|
| [`cobol85-supported-syntax-en.md`](cobol85-supported-syntax-en.md) | **Qué grafía** de cada sentencia aceptan realmente el lexer, el analizador y el entorno de ejecución, y el marcador de conformidad NIST CCVS85 |
| [`cobol85-verb-test-matrix-es.md`](cobol85-verb-test-matrix-es.md) | **Qué probar** de cada verbo |
| [`developers-guide-en.md`](developers-guide-en.md) | Cómo construir aplicaciones con todo ello |

---

## Cómo leer las tablas

Cada fila de capacidad se marca contra tres orígenes y después recibe un estado.

| Columna | Significado |
|---|---|
| **85** | Definido por **COBOL-85** (ANSI X3.23-1985, incluida la enmienda de funciones intrínsecas de 1989 donde se indica) |
| **20xx** | Definido por un **estándar ISO posterior** — COBOL 2002 / 2014 / 2023, y lo que hoy se está redactando hacia 2026 |
| **PRC** | Una **extensión de PowerRustCOBOL** — no está en ningún estándar COBOL |
| **Estado** | Qué hace esta implementación con ello |

Una capacidad puede estar marcada en más de una columna de origen: una función
de COBOL-85 que un estándar posterior amplió lleva `●` en ambas, y la columna
**Notas** dice qué añadió el estándar posterior.

**Marcas de origen:** `●` definido aquí · `○` ampliado o aclarado aquí · `—` no
está en este estándar.

**Marcas de estado:** `✅` soportado · `🚧` parcial o simplificado · `⛔`
previsto, aún sin implementar · `🚫` fuera de alcance por diseño, nunca se
implementará.

> **Nota de honestidad.** PowerRustCOBOL apunta a un subconjunto práctico y
> orientado a aplicaciones, más extensiones visuales de RAD. **No** es una
> implementación de COBOL-85 certificada. La conformidad se *mide* contra la
> suite oficial NIST CCVS85 en lugar de afirmarse — véase el
> [marcador](cobol85-supported-syntax-en.md).

---

## 1. Formato de fuente y estructura del programa

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Fuente de formato fijo, **relajado** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **El valor por defecto.** Se respetan el área de secuencia y la columna indicadora, pero la línea llega hasta donde escribió el desarrollador: sin corte en la columna 72. Los `.cbl` generados de los formularios y los bloques `EXEC RUST` lo necesitan |
| Fuente de formato fijo, **formato de referencia clásico de COBOL-85** (`--source-format=fixed`) | ● | ○ | — | ✅ | Se aplican todas las reglas de columnas: 1–6 secuencia, 7 indicador (`*` `/` comentario, `-` continuación, `D` línea de depuración), 8–72 fuente, **73–80 se descartan**, y la unión estándar de continuaciones, incluido un literal alfanumérico continuado. Es en lo que está escrita la suite de imágenes de tarjeta NIST CCVS85. **Se elige explícitamente, nunca por detección**: aplicar estas reglas a fuente que no fue escrita para ellas borra código en silencio |
| Fuente de formato libre | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Selector de formato de fuente — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | También `COBOLT_SOURCE_FORMAT`; `auto` inspecciona las primeras líneas y nunca elige el formato estricto |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ |  |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ |  |
| DATA DIVISION | ● | ○ | — | ✅ |  |
| PROCEDURE DIVISION | ● | ○ | — | ✅ |  |
| Programas anidados | ● | ○ | — | ✅ |  |
| Varias unidades de programa secuenciales en un mismo fichero | ● | ○ | — | ✅ |  |
| Copybooks `COPY` / `REPLACE` | ● | ○ | — | ✅ | Sustitución de pseudotexto y de palabras, `COPY` anidado, `REPLACE OFF`; resuelve `.cpy`/`.cbl`/`.cob` junto a la fuente, sin distinguir mayúsculas |
| Párrafo `REPOSITORY` | — | ● | ○ | ✅ | COBOL 2002 para clases; PowerRustCOBOL enlaza aquí además los tipos de **FFI de Rust** |
| Rust en línea con `EXEC RUST … END-EXEC` | — | — | ● | ✅ | Se compila dentro del binario; los errores se informan en la línea y la columna COBOL del propio desarrollador |

## 2. La DATA DIVISION y la descripción de datos

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ |  |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ |  |
| FILE SECTION | ● | ○ | — | ✅ |  |
| SCREEN SECTION | ● | ○ | — | 🚧 | Los `ACCEPT`/`DISPLAY` extendidos con `AT`/`WITH` se ejecutan mediante ANSI en modo CLI; la edición de pantalla campo a campo queda superada por el diseñador visual de formularios en modo GUI |
| COMMUNICATION SECTION (`CD`, control de mensajes) | ● | — | — | 🚫 | Teleproceso; obsoleto en los estándares posteriores |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Fuera de alcance por diseño |
| `PICTURE` X / A / 9 / S / V con repetición `(n)` | ● | ○ | — | ✅ |  |
| PICTURE numérica editada (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Supresión de ceros, protección con asteriscos, `$` y signos fijos y flotantes |
| `USAGE DISPLAY` | ● | ○ | — | ✅ |  |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ |  |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Coma flotante; una extensión de fabricante estandarizada después como `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ |  |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Binario nativo; extensión de fabricante |
| `USAGE INDEX` | ● | ○ | — | ✅ |  |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002; alias de lectura **y** de escritura |
| `OCCURS` fijo | ● | ○ | — | ✅ |  |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ |  |
| `INDEXED BY` | ● | ○ | — | ✅ |  |
| Números de nivel 01–49, 77 | ● | ○ | — | ✅ |  |
| Nivel 66 `RENAMES` | ● | ○ | — | ✅ |  |
| Nombres de condición de nivel 88 | ● | ○ | — | ✅ | Incluido `SET … TO TRUE` |
| Cláusula `VALUE` | ● | ○ | — | ✅ |  |
| Elementos de grupo, `FILLER` | ● | ○ | — | ✅ |  |
| `REDEFINES` | ● | ○ | — | ✅ |  |
| Constantes figurativas (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ |  |

## 3. La PROCEDURE DIVISION — verbos

| Verbo | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Emparejamiento de subcampos de grupo |
| `DISPLAY` | ● | ○ | — | ✅ | Lo numérico se muestra con el ancho completo de la PIC |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ |  |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | Varios receptores, `ROUNDED` por receptor |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Varios receptores, `ROUNDED` por receptor |
| `COMPUTE` | ● | ○ | — | ✅ | Varios receptores, `ROUNDED` por receptor |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ |  |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ |  |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ |  |
| `PERFORM` en línea, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ |  |
| `PERFORM para VARYING` (fuera de línea) | ● | ○ | — | ✅ |  |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ |  |
| `ALTER` | ● | ○ | — | ✅ | Elemento obsoleto en COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Semántica fiel; obsoleto en COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ |  |
| `EXIT` | ● | ○ | — | ✅ |  |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ |  |
| `GOBACK` | — | ● | — | ✅ | Extensión de fabricante estandarizada en COBOL 2002 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ |  |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | Punteros de COBOL 2002 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Consciente de la categoría, desciende por los grupos |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ |  |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` combinado |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Maneja el índice de la tabla, ejecuta el primer `WHEN` que casa y, si no, `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` y `RETURNING` son de COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ |  |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ |  |
| `INVOKE` | — | ● | ○ | 🚧 | OO de COBOL 2002. Soportado para **objetos de GUI y de ejecución y para complementos de FFI de Rust**; las definiciones de clase y método por el usuario no están implementadas |
| `UNLOCK` | — | ● | — | 🚧 | Maneja los bloqueos de registro dentro de la ejecución; no se impone entre procesos del sistema operativo |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Transacciones controladas por el programa sobre ficheros INDEXED, con un registro de deshacer de verdad |
| Definiciones OO `CLASS-ID` / `METHOD-ID` | — | ● | — | ⛔ | Previsto |

## 4. Condiciones y expresiones

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Condiciones de relación, de clase, de signo y de nombre de condición | ● | ○ | — | ✅ |  |
| Relaciones combinadas abreviadas, con operador antepuesto (`a > 1 AND < 9`) | ● | ○ | — | ✅ |  |
| Relaciones combinadas abreviadas, con objeto literal (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ |  |
| Relaciones combinadas abreviadas, con objeto identificador (`a = b OR c`) | ● | ○ | — | ✅ |  |
| Modificación de referencia `item(start:length)` | ● | ○ | — | ✅ | Lectura **y** escritura empalmada, sobre cualquier operando |
| Subíndices de tabla en ejecución `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Almacenamiento por ocurrencia, subíndices variables |
| Nombres cualificados `id OF/IN group` | ● | ○ | — | ✅ | Una hoja declarada bajo más de un grupo se resuelve a almacenamientos independientes |
| Comparación alfanumérica correcta según COBOL (rellenada con espacios) | ● | ○ | — | ✅ |  |
| **Aritmética exacta de coma fija** | ● | ○ | ○ | ✅ | Mantisa entera `i128`, sin viajes de ida y vuelta por `f64`: la precisión estándar de 18 dígitos y la **extendida de 31 dígitos** se mantienen exactas |
| Expresiones de propiedad concisas (`Output::Value`) | — | — | ● | ✅ | Leer o fijar una propiedad de un control dentro de una fórmula, sin ningún elemento temporal de working-storage |

### 4.1 Métodos de valor sobre un elemento de datos

`item::Method(args)` llama a un método sobre el **valor de un elemento de datos
corriente** —un campo `PIC X`, un grupo, una ocurrencia de tabla, un trozo con
modificación de referencia o una expresión aritmética—, no solo sobre un control.
Nada de esto es COBOL estándar.

Se puede usar allí donde quepa una expresión: como origen de un `MOVE`, en un
`COMPUTE`, dentro de una condición y en línea en un `DISPLAY`. Los métodos **se
encadenan**: `WS-TEXT::Trim()::Len()`.

| Método | Devuelve | Estado | Notas |
|---|---|:--:|---|
| `Trim()` | texto | ✅ | Se quitan los espacios iniciales y finales |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | texto | ✅ | Tres grafías aceptadas de un mismo método |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | texto | ✅ |  |
| `Replace(from, to)` | texto | ✅ | Todas las apariciones |
| `Len()` · `Length()` | numérico | ✅ | La longitud **del campo**, así que un `PIC X(20)` que contiene `hello` responde `20`. Encadene `::Trim()::Len()` para obtener la longitud del contenido |
| `Split(sep)` | texto | ✅ | El **primer** campo |
| `Split(sep)(n)` | texto | ✅ | El campo *n*-ésimo, contando desde 1. El subíndice solo se acepta sobre un receptor que sea un elemento de datos |

| Receptor | Estado | Notas |
|---|:--:|---|
| Elemento de datos (`PIC X`, grupo, `01`/`77`) | ✅ | El caso corriente |
| Ocurrencia de tabla, modificación de referencia, nombre cualificado, expresión aritmética | ✅ | Aceptado por el evaluador |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | El intérprete acepta un receptor literal, pero el analizador no: un `::` tras un literal es un error de sintaxis. Asigne primero el literal a un elemento de datos |

### 4.2 Una expresión allí donde COBOL-85 solo admite un elemento

COBOL-85 restringe la mayoría de las posiciones emisoras a un identificador o
un literal. RustCOBOL evalúa allí una expresión completa, y eso es lo que elimina
el elemento auxiliar de working-storage que el estándar obliga a declarar.

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. El estándar solo permite un identificador o un literal como campo emisor |
| `SET target TO <expression>` | — | — | ● | ✅ | Equivalente a la forma `COMPUTE`; el destino puede ser un elemento de datos o una propiedad de control como lvalue |
| `STRING <expression> … INTO` | — | — | ● | ✅ | Un elemento emisor puede ser una expresión aritmética (`STRING WS-N * 2 …`) o una llamada a un método de valor (`STRING WS-A::UpperCase() …`); `DELIMITED BY` y el resto siguen siendo estándar |
| **Inferencia de tipos** — leer `Ctrl::Property` da un valor tipado de primera clase | — | — | ● | ✅ | El tipo numérico o de texto fluye por la expresión, así que una propiedad entra directamente en una operación aritmética, en una condición o en una posición emisora **sin ningún elemento `PIC` de por medio**: `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. El valor de una propiedad que parece numérico se lee de vuelta como numérico, de modo que las comparaciones y la aritmética siguen siendo algebraicas y no carácter a carácter |

## 5. Funciones intrínsecas

El juego de intrínsecas de COBOL-85 llegó con la **enmienda de 1989** (ANSI
X3.23a-1989); las funciones añadidas por COBOL 2002 y posteriores van marcadas en
la columna `20xx`. Todas las de abajo están implementadas.

| Grupo | Funciones | 85 | 20xx | PRC | Estado |
|---|---|:--:|:--:|:--:|:--:|
| Longitud y caracteres | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Longitud y caracteres (posteriores) | `BYTE-LENGTH`, `LENGTH-AN`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Mayúsculas/minúsculas y texto | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| Texto (posteriores) | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| Conversión numérica | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| Conversión numérica (posteriores) | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| Aritmética | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| Ordenación | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| Estadística | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| Trigonometría y logaritmos | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| Combinatoria | `FACTORIAL` | ● | ○ | — | ✅ |
| Financieras | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| Fecha y hora | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. E/S de ficheros — organizaciones y acceso

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Registros de longitud fija |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Texto terminado en salto de línea; los espacios finales se descartan al escribir |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Motor ISAM incorporado y sin dependencias |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Motor propio (`cobolt-runtime/src/relative.rs`, contenedor `PRCREL1`, disco y MEMORY). `RELATIVE KEY IS` direcciona los registros por número entero de registro desde 1; los tres modos de acceso; los siete verbos de fichero se despachan sobre él. **El módulo RL de NIST está terminado en ambos ejes**: 35/35 de compilación, 34/34 de ejecución, 354 aserciones, 0 fallos (motor 1.62.76, módulo 1.62.77) |
| `RELATIVE KEY IS data-name` (incluida la grafía sin `KEY`) | ● | ○ | — | ✅ | Una cláusula `RELATIVE data-name` con la palabra `KEY` omitida es la clave, no una simple cláusula de organización |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | Los tres se ejecutan |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Orden de clave ascendente en disco |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ |  |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ |  |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` es de COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ |  |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Incluidos `GREATER/LESS THAN` y `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ |  |
| Códigos de `FILE STATUS` | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Se analiza y se lleva en la sentencia, pero es **consultivo**: hay una sola unidad de ejecución, así que nada compite |
| `OPEN … WITH LOCK` (abrir el fichero en exclusiva) | — | ● | — | 🚧 | Igual: aceptado y consultivo en el modelo de unidad de ejecución única |
| `READ … WITH LOCK` | — | ● | — | ✅ | El motor ya retiene el registro bajo `I-O`; la frase declara la intención |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | Libera de verdad el bloqueo que el motor toma bajo `I-O`: hoy es la única frase de bloqueo con efecto en ejecución. `UNLOCK` está en la §3 con los demás verbos |
| Compartición de ficheros entre procesos e imposición de bloqueos de registro | — | ● | — | ⛔ | Previsto; hoy el modelo es de una sola unidad de ejecución |

## 7. E/S de ficheros — el motor INDEXED (PowerRustCOBOL)

Todo lo de esta sección es una extensión de la plataforma alrededor del
comportamiento estándar de `ORGANIZATION IS INDEXED` de más arriba. El detalle
está en [`indexed-file-format-es.md`](indexed-file-format-es.md),
[`indexed-file-internals-es.md`](indexed-file-internals-es.md) y
[`indexed-redb-engine-es.md`](indexed-redb-engine-es.md).

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **El modo de almacenamiento por defecto.** Los registros y los índices viven en el fichero del `ASSIGN` y se leen bajo demanda, de modo que la RAM queda acotada incluso en ficheros muy grandes. Lo sirve el motor redb a prueba de caídas desde 1.62.73; al B+tree paginado anterior todavía se llega con `--indexed-engine rust` |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | El fichero entero en RAM, persistido en la ruta del `ASSIGN` al cerrar |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | RLE sin dependencias; aplasta bastante por encima del 50 % las tiradas de relleno típicas de los registros COBOL |
| `COMMIT` / `ROLLBACK` controlados por el programa | — | — | ● | ✅ | Registro de deshacer de verdad, en los motores de memoria y de disco |
| Bloqueo de registros dentro de una unidad de ejecución | — | ○ | ● | ✅ | Véase la advertencia sobre procesos de más arriba |
| Motor seleccionable (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | También `COBOL_INDEXED_ENGINE`; todos compatibles en comportamiento. **`redb` es el valor por defecto** desde 1.62.73 (`cobolt-runtime/src/indexed.rs:126`) |
| Motor ACID `redb` a prueba de caídas | — | — | ● | ✅ | OPEN en O(1) (~5 ms con 200 k registros), RAM del conjunto de trabajo (≥250 M registros), sobrevive a un corte de corriente sin corrupción del índice |
| Contenedor autodescriptivo `PRCIDX1` | — | — | ● | ✅ | Incrusta el formato de registro y los descriptores de clave; la validación estricta al abrir convierte una discrepancia de esquema en `39` y un fichero ausente en `35`. No es compatible byte a byte con Fujitsu |
| Registro de transacciones por fichero (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt o NDJSON listo para Grafana/Loki — véase [`observability-es.md`](observability-es.md) |

## 8. Integraciones del entorno de ejecución

Se alcanzan desde COBOL como `CALL` de ejecución y como `INVOKE`. Nada de esto
es COBOL estándar; es lo que hace el lenguaje utilizable para aplicaciones
modernas.

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | Una misma superficie de CALL para los tres; el backend se elige a partir de la cadena de conexión. **Sin bibliotecas del sistema** —no se enlaza nada del anfitrión—, pero «Rust puro» solo es cierto de dos de los tres: `postgres` y `mysql` lo son, mientras que `rusqlite` está fijado con `features = ["bundled"]` y compila la **amalgama en C de SQLite** a través de `libsqlite3-sys`. (Esa compilación de C es además la razón de que `test_external_crates_e2e` falle de forma intermitente dentro de un `cargo build` anidado.) Véase [`database-runtime-es.md`](database-runtime-es.md) |
| **Conjuntos de resultados SQL** — `Fetch()`, `ColumnNames()`, `ColumnCount()`, `ColumnName(n)` | — | — | ● | ✅ | `Fetch()` devuelve la siguiente fila separada por tabuladores y vacía cuando se agota, así que termina su propio bucle; `ColumnNames()` nombra el conjunto de resultados en el orden del SELECT, incluso cuando no casó ninguna fila. La superficie de `CALL` lee en cambio la fila actual columna a columna por índice: los dos recorridos no deben mezclarse sobre un mismo manejador |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | Cabeceras personalizadas |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ |  |
| **Gráficos** — barras / líneas / tarta / área / dispersión / anillo | — | — | ● | ✅ | Enlazados a tablas COBOL |
| **Ficheros de texto** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ |  |
| **Temporizadores** | — | — | ● | ✅ |  |
| **Gancho de objeto de agente de IA** | — | — | ● | ✅ |  |
| **Complementos de FFI de Rust** | — | — | ● | ✅ | Módulos declarados bajo `REPOSITORY`, despachados mediante `INVOKE` o por correspondencias directas de propiedades |
| **Procedimientos de usuario** | — | — | ● | ✅ | Procedimientos COBOL compartidos, editables en el IDE y llamables como `CALL "PROCEDURE-NAME"` |

## 9. Explícitamente fuera de alcance

Estas cosas no se implementarán. Se enumeran para que la respuesta se pueda
encontrar en lugar de faltar.

| Capacidad | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, control de mensajes / teleproceso) | ● | — | — | 🚫 | Obsoleto en los estándares posteriores; sin uso moderno |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Superado por los informes y el enlace de datos propios de la plataforma |
| Controles ActiveX / OLE / COM | — | — | — | 🚫 | Específicos de una plataforma y no portables |

---

## 10. La plataforma en sí

No son funciones del lenguaje COBOL, sino el IDE, el compilador y las
herramientas que los rodean. El recorrido completo está en la
[guía del desarrollador](developers-guide-en.md).

### 10.1 El IDE

| Capacidad | Estado | Notas |
|---|:--:|---|
| Diseñador visual de formularios | ✅ | Lienzo de diseño con varios temas (**Liquid Glass**, **Cobalt Steel**), ajuste a la rejilla, redimensionado por arrastre de controles y del lienzo, alineación con selección múltiple y orden de profundidad |
| Motor de renderizado unificado | ✅ | Paridad de píxel entre el diseñador, la vista previa, la aplicación en ejecución y el binario compilado |
| Catálogo de controles | ✅ | **43 widgets** repartidos en Common, Container, Data, Graphics, Menu, Non-visual y Charts, más un tipo `Custom` aportado por complementos |
| Radio de esquina universal y recorte redondeado | ✅ | Los hijos anidados se recortan al borde redondeado del padre mediante un enmascarado por muescas de esquina |
| `Transparency` por control | ✅ | 0 = opaco … 100 = transparente; atenúa la cara, el marco y la sombra mientras el texto, los glifos y el borde siguen legibles. Los rótulos que quedan por debajo de WCAG AA contra lo que tienen detrás saltan al polo que sí se lee |
| Widget Animator | ✅ | Renderiza **GIF / WebP / APNG** de forma nativa |
| Knob, Gauge, Switch, FileDropZone, Maps y Web Search | ✅ | Mando giratorio con relleno bipolar; KPI radial, lineal o de anillo con zonas de aviso y críticas automáticas; arrastrar y soltar o selector nativo |
| Editor de menús avanzado | ✅ | Editor visual de árbol, **1112** iconos vectoriales incorporados en 37 categorías, anidamiento jerárquico y firmas HMAC de integridad de la configuración |
| Enlace de datos y matrices de controles | ✅ | Enlace directo a fuentes SQL y de datos; los **grupos repetidores visuales** expanden matrices de GroupBox y Panel a partir del número de filas del `DataSource` en ejecución |
| Validación visual e inspector de formularios | ✅ | Distintivos de error en tiempo real para manejadores mal formados, enlaces incompletos y anomalías de disposición; el gestor de procesos de `rcrun` sigue en vivo el porcentaje de CPU, la RSS, los registros y el número de hilos |
| Depurador de formularios | ✅ | Ventana independiente siempre visible: puntos de ruptura, paso a paso dentro/fuera/por encima, inspector de variables y reproducción animada a 1–10 líneas por segundo |
| Malla de asistentes de IA agentiva | ✅ | Orquestador de LLM **rig-core** (Ollama, OpenAI, Groq, Alibaba Model Studio y otras API en la nube) que ejecuta el Dev Agent, el Editor Assistant y el History Compactor, con un registro de observabilidad en vivo y lecturas de tokens `↑input ↓output` |
| Grace, la orquestadora | ✅ | Descompone una petición, encamina cada tarea al especialista que la posee e impone un **revisor Pedantic** uno a uno: ningún especialista aprueba su propio trabajo |
| Base de conocimiento troceada con RAG | ✅ | Indexada con un registro por asunto; se distribuye ya embebida, con GPU y un repliegue a CPU que no calienta, y **File → Reindex Knowledge Bases** |
| Ciclo de vida de los formularios y ventanas | ✅ | Un **formulario principal** designado arranca la aplicación; se respetan el cromo y el estado de cada formulario; `OpenFormSync`/`OpenFormAsync`; la posición de la ventana es una propiedad de diseño; efectos de entrada y salida por proyecto |
| Ejecución con varias ventanas | ✅ | Pantallas de vista previa y de ejecución en viewports propios del sistema operativo (multi-viewport de egui) |
| Interfaz internacionalizada | ✅ | 6 idiomas de interfaz: inglés, español, portugués, japonés, chino y francés |
| Selector de tipografías del sistema | ✅ | Cualquier fuente instalada, dibujada con su propia tipografía y aplicada en vivo al diseñador, las vistas previas y los formularios en ejecución |
| Diálogos de fichero nativos y no bloqueantes | ✅ | Abrir, guardar y examinar sin detener el bucle de eventos de la interfaz |

### 10.2 El compilador

| Capacidad | Estado | Notas |
|---|:--:|---|
| Salida en un único binario nativo | ✅ | Serializa el AST con `bincode` + `flate2`, lo incrusta junto con todos los formularios mediante `include_bytes!`, compila con `cargo build --release` y emite un binario en `bin/`, **sin incluir ningún fuente `.cbl`** |
| Avisos de redistribución | ✅ | `bin/` recibe automáticamente `LICENSE`, `NOTICE` y el aviso del entorno de ejecución, de modo que las distribuciones llevan los avisos exigidos por Apache-2.0 |
| Diagnósticos reales de `rustc` cuando falla la compilación | ✅ | Un fallo de compilación informa de los diagnósticos del propio compilador, no de una línea de resumen |

.<<

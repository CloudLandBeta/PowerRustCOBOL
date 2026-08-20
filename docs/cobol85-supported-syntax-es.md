<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Referencia de sintaxis soportada de RustCOBOL-85

**Verdad de referencia sobre lo que el lexer / parser / runtime de RustCOBOL
aceptan hoy realmente**, derivada del código fuente (`cobolt-lexer`,
`cobolt-parser`, `cobolt-runtime`). Escriba sus tests contra las formas ✅; las
formas ❌ no se analizarán o son no-operaciones, y las formas ⚠️ se analizan pero
se comportan parcialmente. Éste es el compañero de
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md): la matriz dice *qué*
probar, esto dice *qué grafía entiende RustCOBOL*.

Leyenda: ✅ soportado · ⚠️ se analiza pero es parcial/simplificado · ❌ no
reconocido (evítelo, o pruébelo sólo para confirmar la carencia).

> **Actualización (pasada de implementación de carencias):** lo siguiente se
> implementó y ahora es ✅ — **modificación de referencia** `id(inicio:long)`,
> **`PERFORM n TIMES` en línea**, **`SET … UP/DOWN BY`**, **`ON OVERFLOW` de
> STRING/UNSTRING + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` consciente de
> categorías**, **condiciones abreviadas con operador delante** (`a > 1 AND < 9`),
> **`CALL … ON EXCEPTION`** (se ejecuta ante un CALL no resuelto), **`COMPUTE`
> con múltiples receptores + `ROUNDED` por receptor**, y un conjunto de
> **funciones intrínsecas** mucho mayor.
>
> **Actualización (pasada de entorno jerárquico / consciente de ocurrencias —
> 1.5.0):** cuatro funcionalidades que bloqueaba el modelo de datos son ahora
> ✅ — **subíndices de tablas en ejecución** `t(i)` / `t(i, j)` (almacenamiento
> por ocurrencia), **desambiguación de nombres cualificados** `id OF/IN grupo`
> (los nombres hoja duplicados se resuelven a almacenamientos independientes),
> **`MOVE/ADD/SUBTRACT CORRESPONDING`** y **`SEARCH` / `SEARCH ALL` funcionales**.
>
> **Actualización (pasada de completitud de verbos — 1.6.0):** ahora también
> ✅ — **`MULTIPLY`/`DIVIDE GIVING` con múltiples receptores + `ROUNDED` por
> receptor** en `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** y el `EXIT` simple corregido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado y las regiones
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de fecha y financieras
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condiciones abreviadas con objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-sujeto) y **`WHEN NOT`**;
> **nombres-condición de nivel 88 reales** (`SET … TO TRUE/FALSE`, el anfitrión
> se contrasta con sus VALUE / rangos); **`PERFORM para VARYING`**; y un runtime
> funcional de **`SORT`/`MERGE`** (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). La lista de evitación del final está al día.
>
> **Actualización (pasada de vaciado de la lista de evitación — 1.7.0):** las
> carencias restantes ya están implementadas — **abreviación con objeto
> identificador** (`a = b OR c`, resuelta mediante los metadatos de nivel 88);
> **`INITIALIZE … REPLACING categoría DATA BY valor`**; **`66 RENAMES`** (la
> lectura sintetiza / la escritura distribuye entre los elementos cubiertos);
> **punteros** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> aliasing con `SET ADDRESS OF item TO …`, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; **`NEXT SENTENCE`** fiel; las **intrínsecas** estándar restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`); y
> el **`ACCEPT`/`DISPLAY` de pantalla** extendido (`AT`/`WITH` mediante ANSI en
> modo CLI — ahora *ejecutado*, no sólo analizado).
>
> **Actualización (1.7.1):** las fuentes de registro de `ACCEPT` ya son
> funcionales (antes eran no-operaciones reconocidas) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (emparejadas con
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (emparejada con
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Actualización (1.7.2):** cláusulas de compartición / bloqueo de ficheros y
> `CANCEL` (antes ❌ / no-operación) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libera los bloqueos de registro
> INDEXED del fichero) y **`CANCEL programa`** (reinicializa el almacenamiento
> del programa).
>
> **Actualización (1.8.0):** **`COMMIT` / `ROLLBACK`** son ya verbos COBOL
> reales — transacciones controladas por el programa sobre los ficheros INDEXED
> abiertos (tanto el motor de memoria como el de disco). El motor de disco ganó
> un registro de deshacer real durante la ejecución (antes era una
> no-operación). La lista de evitación del final está al día.

---

## Sentencias reconocidas (verbos)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirige el `GO TO` de para-1) ·
`UNLOCK file` (libera los bloqueos de registro del fichero) ·
`OPEN … SHARING/WITH LOCK` · `READ … WITH [NO] LOCK` (compartición/bloqueo de
ficheros — orientativo dentro de la única unidad de ejecución)
✅ `COMMIT` / `ROLLBACK` (transacciones de ficheros INDEXED controladas por el
programa — véase Verbos de fichero) · `CANCEL` (reinicializa el almacenamiento
del programa) · ⚠️ `INVOKE` (se analiza como no-operación)
Extensiones del proyecto: `EXEC RUST … END-EXEC`,
`TRY/CATCH/FINALLY/END-TRY`, `THROW`. Un bloque puede hacer `use` de los crates
siempre enlazados (std, egui, eframe y el conjunto del runtime enlazado)
**más cualquier crate que el project registre en Project's Crates** (spec 044):
los crates registrados se fijan a una versión exacta, se incorporan en el
`crates/` del project y se compilan dentro del binario; los crates no
registrados hacen fallar Check/Build en la línea del desarrollador, nombrando el
remedio.

✅ `SEARCH` (serie) / `SEARCH ALL` (búsqueda binaria sobre una tabla con
`ASCENDING`/`DESCENDING KEY` — ejecuta el primer `WHEN` que coincida, si no
`AT END`).
✅ `SORT` / `MERGE` con `RELEASE` / `RETURN` (funcionales — véase más abajo).
✅ `DECLARATIVES … END DECLARATIVES` con `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — manejadores de error de fichero
disparados ante un `FILE STATUS` de error no tratado.
❌ **No reconocidos — no los use:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas soportadas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurativa} TO id1 [id2 …]` (varios receptores).
- ✅ `MOVE CORRESPONDING g1 TO g2` — mueve cada elemento subordinado que ambos
  grupos comparten por nombre, recorriendo recursivamente los subgrupos que
  coinciden.
- ✅ **Modificación de referencia `id(inicio:long)`** — como emisor (subcadena) y
  como receptor (asignación parcial insertada); funciona sobre los operandos de
  todos los verbos. `long` es opcional.
- ✅ subíndices `t(i)`, `t(i, j)` — leen/escriben la ranura de almacenamiento de
  esa ocurrencia; los subíndices variables `t(WS-I)` se evalúan en cada acceso.
- ✅ cualificación `id OF/IN grupo` (`… OF g1 OF g2`) — resuelve al elemento
  correcto incluso cuando el nombre hoja está declarado bajo más de un grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por receptor** — cada receptor lleva su propio indicador
  `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico
  coincidente, recorriendo recursivamente los subgrupos que coinciden.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **varios receptores `GIVING`**, cada uno con su propio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sin `GIVING`) guarda `a/b` de vuelta en `a` (una comodidad
  de PowerRustCOBOL; el COBOL estándar exige aquí `INTO` o `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **varios receptores, cada uno con su propio `ROUNDED`**.
- ✅ operadores de expresión `+ - * /` y `**` (potencia, asociativa por la
  derecha), paréntesis, `FUNCTION nombre(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] sentencias [ELSE sentencias] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO sujeto …]` … `WHEN {valor | valor THRU
  valor | NOT valor | condición | ANY} [ALSO …] sentencias … [WHEN OTHER
  sentencias] END-EVALUATE`.
- ✅ **`ALSO` multi-sujeto** — cada columna `WHEN` se compara posicionalmente con
  su sujeto y se combina con AND.
- ✅ **`WHEN NOT valor`** niega un objeto de selección; **`WHEN condición`**
  (p. ej. `EVALUATE TRUE WHEN a > b`) evalúa la condición booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal entero o data item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en línea,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en línea (sin párrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — ejecuta el párrafo en
  cada iteración (fuera de línea, sin `END-PERFORM`).

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ el `EXIT` simple es un punto de retorno sin efecto; `EXIT PROGRAM` vuelve al
  llamante.
- ✅ `EXIT PERFORM [CYCLE]` (romper / continuar el PERFORM en línea más cercano),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfiere el control más allá del siguiente límite de
  sentencia (el parser inserta marcadores de límite en cada punto; fiel, no un
  simple `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemónico}`.
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` posiciona el cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (la línea de órdenes completa) · `FROM ARGUMENT-NUMBER`
  (número de argumentos) · `FROM ARGUMENT-VALUE` (el argumento en el puntero
  fijado por `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (la variable nombrada por
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemónico] [[WITH] NO ADVANCING]`.
- ✅ formas de pantalla `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — ejecutadas mediante
  posicionamiento de cursor ANSI + SGR en **modo CLI** (`rcrun`); ignoradas en
  modo GUI (allí el form designer sustituye a la E/S de SCREEN).
  `ACCEPT id AT …` posiciona y después lee.

### STRING
- ✅ `STRING {origen [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO destino
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Desbordamiento = la cadena ensamblada es más ancha que el campo receptor.
- ✅ **Extensión — `DELIMITED BY` inteligente por defecto** (cuando se omite la
  cláusula en un operando): los elementos alfanuméricos `PIC X`/`A` toman por
  defecto `SPACES` (se descarta el relleno final); los literales de cadena, los
  numéricos, los numéricos editados, los resultados de `FUNCTION` y las
  expresiones toman por defecto `SIZE`. Los data items se mueven en su forma de
  campo (numérico → dígitos con el ancho completo del PIC; numérico editado →
  caracteres editados).

### UNSTRING
- ✅ `UNSTRING origen [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Desbordamiento = más campos de origen
  que receptores.

### INSPECT
- ✅ `INSPECT id CONVERTING desde TO hasta`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **se aplican ambas mitades**.
- ✅ `BEFORE/AFTER INITIAL` confina cada cláusula a una subregión del campo.
  (TALLYING acumula sobre el contador, según COBOL.)

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado a MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-nombre TO TRUE` pone en el elemento anfitrión el primer VALUE de la
  condición; `TO FALSE` pone un valor fuera del conjunto de VALUE (mejor
  esfuerzo — no hay cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | otro-ptr}` y
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — véase **Punteros** abajo.

### INITIALIZE
- ✅ `INITIALIZE id …` — consciente de la categoría: numérico / numérico editado
  → ZERO, todo lo demás → SPACES, recorriendo recursivamente los elementos de
  grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY valor …` — pone cada elemento
  subordinado de esa categoría al valor; los demás quedan intactos.

### Punteros (USAGE POINTER)
- ✅ `USAGE POINTER` declara un puntero (NULL al principio).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — hace que `id` sea un
  alias del almacenamiento del destino (tanto las lecturas **como** las
  escrituras siguen el alias); habitualmente un registro de LINKAGE.
  `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ El cuerpo de `ON EXCEPTION` / `ON OVERFLOW` se ejecuta cuando el programa
  llamado no se resuelve; el cuerpo de `NOT ON EXCEPTION` se ejecuta cuando la
  llamada **sí se resuelve**.
- ✅ `CANCEL programa …` reinicializa la WORKING-STORAGE del programa nombrado
  para que su siguiente `CALL` empiece de cero.

### Verbos de fichero (las cláusulas soportadas — la cobertura completa está en la batería de E/S de ficheros)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` se analizan y se respetan donde tiene sentido —
  orientativos en el modelo de unidad de ejecución única.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensión de
  PowerRustCOBOL) — registra el operador/usuario en el log de observabilidad
  INDEXED (campo `user=` en todas las líneas de evento de la sesión de ese
  fichero). Puramente observacional; sin autenticación ni autorización. Véase
  [`observability.md`](observability.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libera el bloqueo de registro que el motor INDEXED toma en I-O.
- ✅ `UNLOCK f [RECORD[S]]` libera los bloqueos de registro del fichero.
- ✅ **`COMMIT` / `ROLLBACK`** — transacciones controladas por el programa sobre
  **todos** los ficheros INDEXED abiertos. `OPEN` inicia una transacción;
  `COMMIT` confirma los `WRITE`/`REWRITE`/`DELETE` pendientes (un `ROLLBACK`
  posterior ya no puede deshacerlos) e inicia otra; `ROLLBACK` deshace todos los
  cambios desde el último `COMMIT`/`OPEN`. El almacenamiento **DISK** hace
  duraderos en disco `COMMIT`/`CLOSE`. El almacenamiento **MEMORY** mantiene
  `COMMIT`/`ROLLBACK` puramente en RAM (nunca escribe a disco); un fichero
  `STORAGE IS MEMORY` normal es efímero, y `STORAGE IS MEMORY WITH PERSISTENCE`
  guarda a disco sólo en el `CLOSE`. (La recuperación ante caídas mediante un
  registro write-ahead duradero es trabajo futuro — esto es un rollback a nivel
  de programa, dentro de la ejecución.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (ficheros INDEXED; extensión de PowerRustCOBOL). El
  almacenamiento por defecto es `DISK`. `WITH COMPRESSION` comprime el registro
  almacenado (las claves se evalúan sobre el registro sin comprimir);
  `WITH PERSISTENCE` (sólo MEMORY) guarda el fichero en RAM al hacer `CLOSE`.
  `OPEN OUTPUT` siempre (re)crea el contenedor en disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ La compartición de ficheros entre *procesos* no se hace cumplir (unidad de
  ejecución única); las cláusulas `SHARING`/`LOCK` se analizan y se respetan los
  bloqueos de registro por ejecución del motor INDEXED.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcionales, búfer de trabajo en memoria)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dentro de un INPUT PROCEDURE) añade a la
  ejecución; `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devuelve
  los registros.
- Los registros se ordenan de forma estable por las claves declaradas
  (`ASCENDING`/`DESCENDING`); `USING` lee y `GIVING` escribe los ficheros
  secuenciales nombrados.

---

## Condiciones (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionales: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relaciones en palabras: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Clase: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
- ✅ Signo: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nombre-condición de nivel 88 (el nombre desnudo como condición).
- ✅ `AND` / `OR` / `NOT` combinados, paréntesis (AND liga más fuerte que OR).
- ✅ **Condiciones abreviadas con operador delante** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (se reutiliza el sujeto de la comparación anterior).
- ✅ **Abreviación con objeto literal** — `a = 1 OR 2 OR 3` (reutiliza tanto el
  sujeto como el operador; el objeto es un literal).
- ✅ **Abreviación con objeto identificador** — `a = b OR c` (donde `c` es un
  data item). Un identificador desnudo tras AND/OR que sigue a una comparación se
  resuelve en ejecución: si es un nombre-condición de nivel 88 conocido, se
  evalúa como tal; si no, es el objeto `a = c`. (Un identificador seguido
  inmediatamente de `AND` conserva la precedencia de AND.)

---

## Expresiones, literales, USAGE

- ✅ Operadores aritméticos `+ - * /` y `**`; paréntesis; `+`/`-` unarios.
- ✅ `FUNCTION nombre ( arg [ , arg … ] )` — intrínsecas **implementadas**:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (con semilla opcional), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Las conversiones de fecha usan la base estándar 1601-01-01 = día 1.) El
  **conjunto completo de intrínsecas del estándar COBOL-85** está implementado.
  ⚠️ Cualquier nombre de `FUNCTION` no reconocido se analiza igualmente pero
  devuelve **0** en ejecución.
- ✅ Literales: entero, decimal, cadena, todas las constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Literales hexadecimales** — `X"09"`, `x'0D0A'` (cualquier caja, cualquier
  comilla). Un carácter por **par** de dígitos hexadecimales, así que la cuenta
  de dígitos debe ser par; una cuenta impar o un dígito no hexadecimal es un
  literal mal formado y se reporta, en lugar de releerse calladamente como la
  palabra `X` junto a una cadena. Usables allí donde valga un literal
  entrecomillado (`DELIMITED BY`, `MOVE`, `VALUE`, comparaciones).

---

## Cláusulas de la DATA DIVISION (sintaxis de declaración aceptada)

- ✅ Niveles `01`–`49`, `77`, `88`; `FILLER`; de grupo / elementales.
- ✅ `PIC/PICTURE` con `X A 9 S V P` y símbolos de edición (`Z * $ + - CR DB B 0 /
  , .`).
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (y `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/con signo/alfanumérico/figurativo/`ALL`).
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES`, `JUSTIFIED [RIGHT]`, `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL`.
- ✅ `88 nombre VALUE v [v …]` / `VALUE a THRU b` — **nombres-condición reales**:
  el nivel 88 se liga a su elemento anfitrión; la comprobación contrasta el
  anfitrión con los VALUE / rangos, y `SET 88-nombre TO TRUE` guarda en el
  anfitrión un valor que la satisface.
- ✅ `USAGE INDEX` declara un registro índice entero (`SET`/`SEARCH` lo usan);
  `USAGE POINTER` — véase **Punteros** arriba.
- ✅ `66 NEW RENAMES elemento-1 [{THRU|THROUGH} elemento-2]` — un alias de
  reagrupación; la lectura concatena los elementos cubiertos, la escritura los
  distribuye por el ancho de campo.
- Secciones: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` se
  analiza pero no se ejecuta.

---

## Todavía NO soportado — lista de evitación actual

El conjunto de verbos y cláusulas de COBOL-85 está **cubierto por completo**. Lo
que queda fuera del alcance es intencionado o posterior al 85:

1. **Edición de entrada con `ACCEPT` de pantalla** — `DISPLAY … AT/WITH` y
   `ACCEPT … AT` se ejecutan (ANSI) en modo CLI, pero la edición completa a nivel
   de campo de la SCREEN SECTION (tabulación automática, validación de campo,
   mapas de color) queda **sustituida por el form designer** en modo GUI.
2. **Compartición de ficheros entre *procesos*** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK` y `UNLOCK` se analizan y gobiernan los bloqueos de
   registro por ejecución del motor INDEXED, pero los bloqueos no se hacen
   cumplir entre procesos distintos del sistema operativo (modelo de unidad de
   ejecución única).
3. **COBOL orientado a objetos** (definiciones de clase/método) — `INVOKE` es una
   no-operación para los objetos COBOL (sólo gobierna objetos de GUI/runtime).
4. Organización de ficheros **RELATIVE** (SEQUENTIAL / LINE SEQUENTIAL / INDEXED
   están hechas).
5. Los nombres de función intrínseca no reconocidos siguen devolviendo **0**.

> **Resuelto (1.5.0):** el modelo de datos plano pasó a ser jerárquico /
> consciente de ocurrencias, desbloqueando **CORRESPONDING**, los **nombres
> cualificados**, los **subíndices de tabla** y **`SEARCH`**.
> **Resuelto (1.6.0):** `MULTIPLY`/`DIVIDE` con varios receptores + `ROUNDED` por
> receptor; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`;
> `INSPECT TALLYING REPLACING` combinado + `BEFORE/AFTER INITIAL`; intrínsecas de
> fecha y `ANNUITY`; abreviación con objeto literal; `EVALUATE ALSO`/`WHEN NOT`;
> nombres-condición de nivel 88 reales; `PERFORM para VARYING`; y el runtime de
> `SORT`/`MERGE` con `RELEASE`/`RETURN`.
> **Resuelto (1.7.0):** abreviación con objeto identificador;
> `INITIALIZE … REPLACING`; `66 RENAMES`; punteros (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`); `ALTER` / `UNLOCK`;
> `NEXT SENTENCE` fiel; las intrínsecas estándar restantes; y el
> `ACCEPT`/`DISPLAY` de pantalla extendido (ejecutado en modo CLI).
> **Resuelto (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (con los
> registros emparejados `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Resuelto (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (libera los bloqueos de registro INDEXED) y `CANCEL programa`.
> **Resuelto (1.8.0):** `COMMIT` / `ROLLBACK` como transacciones de ficheros
> INDEXED controladas por el programa (motores de memoria y disco; registro de
> deshacer real en disco).

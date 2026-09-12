<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Matriz de pruebas de verbos y secciones de datos de RustCOBOL‑85

Una especificación de pruebas para terminar COBOL‑85 dentro del alcance del
proyecto. Enumera, **en profundidad**, lo que *todavía no está cubierto* por los
conjuntos de pruebas existentes, en forma de esqueletos de sintaxis + ejes de
permutación + la mezcla de tipos de datos con la que hay que ejercitar cada
verbo. El objetivo de estas pruebas es **exploratorio**: ejecutar todas las
variantes, observar el comportamiento actual y decidir qué corregir, ajustar,
crear o eliminar.

> Ya verificado — NO volver a especificarlo aquí: la aritmética numérica exacta
> (valores de resultado de ADD/SUB/MUL/DIV/COMPUTE, ROUNDED, ON SIZE ERROR), las
> PICTURE numéricas editadas + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, toda la
> E/S de ficheros (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, claves,
> START/REWRITE/DELETE/INVALID KEY, STORAGE MODE MEMORY/DISK, compresión,
> persistencia en MEMORY), programas anidados y CALL básica, comparación
> alfanumérica, lexer fijo/libre. (Las permutaciones de *sintaxis* aritmética de
> más abajo siguen estando dentro del alcance: lo «terminado» es solo la
> matemática de los valores.)

## Notación

- `[ x ]` opcional, `{ a | b }` elección, `…` repetición, `dn` = elemento de
  datos n.
- **Eje de mezcla de tipos (T):** cada ranura de operando debe ejercitarse con
  estas clases de receptor y emisor, en ambos sentidos cuando proceda:
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Valores límite por clase:** vacío, mínimo, máximo, desbordamiento por uno,
  todo espacios, todo ceros, signo en LEADING/TRAILING [SEPARATE], escalado con
  P, punto implícito con V.
- De cada verbo hay que capturar: el valor o valores resultantes, **FILE STATUS y
  los registros especiales** (`RETURN-CODE`, `TALLY`), la rama de
  desbordamiento/excepción tomada y que nada cambie en caso de error.

---

## Parte A — Secciones de la DATA DIVISION (comportamientos sin probar)

### WORKING-STORAGE SECTION
- **Niveles:** 01, anidamiento 02–49, 77 (independiente), 66
  `RENAMES a THRU b`, 88.
- **PIC:** `X A 9 S V P` con `(n)`; escalado con `P` (izquierda/derecha); punto
  implícito con `V`; combinaciones editadas; grupo con `PIC` frente a grupo sin
  `PIC`.
- **USAGE:** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2, COMP‑3/PACKED‑DECIMAL,
  COMP‑5, INDEX, POINTER — declaración, tamaño de almacenamiento y viaje de ida y
  vuelta del valor.
- **VALUE:** numérico, con signo, alfanumérico, figurativo, `ALL "x"`; VALUE en un
  grupo; VALUE ilegal (tamaño > PIC).
- **OCCURS:** fijo; `DEPENDING ON`; `INDEXED BY`; `ASCENDING/DESCENDING KEY`;
  varias dimensiones (2–3); OCCURS sobre un grupo.
- **Cláusulas:** REDEFINES (igual, menor, mayor, encadenado), RENAMES, JUSTIFIED
  RIGHT, BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`, SYNCHRONIZED,
  FILLER.
- **Nombres de condición 88:** valor único, lista de valores, `VALUE a THRU b`,
  varios rangos, sobre anfitrión numérico / alfanumérico / editado; evaluación y
  `SET … TO TRUE`.
- **Inicialización:** por defecto (espacios/ceros según la clase) frente a VALUE;
  **persistencia a través de PERFORM y a través de CALL** (WS conserva el último
  valor).

### LOCAL-STORAGE SECTION
- **Se reinicializa en cada entrada al programa** (por contraste con la
  persistencia de WS).
- Las cláusulas VALUE se **vuelven a aplicar en cada entrada**.
- **Recursión:** cada CALL (recursiva) obtiene una instancia independiente de
  LOCAL-STORAGE.
- La misma cobertura de cláusulas que WS (OCCURS/REDEFINES/88/…), pero
  verificando la semántica de reinicialización.

### LINKAGE SECTION
- Los elementos **no tienen almacenamiento hasta que el llamante los vincula**;
  acceder a un enlace sin vincular.
- Vinculados mediante `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (el llamante ve los cambios) frente a **BY CONTENT** (el
  llamado edita una copia) frente a **BY VALUE** (escalar).
- Grupo y elemental, OCCURS, REDEFINES, 88 en la sección de enlace.
- Discrepancia de tamaño o USAGE entre el parámetro real y el formal
  (comportamiento a observar).
- `ADDRESS OF` / `SET ADDRESS OF … TO` y vinculación de POINTER (si está
  soportado).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — vinculación posicional con los argumentos
  de CALL; discrepancia de número (menos o más argumentos); orden.
- `BY REFERENCE | BY VALUE` por parámetro en la lista USING.
- `RETURNING dn` — valor devuelto a `CALL … RETURNING`; frente a `GIVING`; frente
  a `RETURN-CODE`.
- `USING` del programa principal vinculado desde la línea de órdenes (si está
  soportado).
- Mezcla de tipos en cada ranura de parámetro (aplicar **T**).

---

## Parte B — Matriz de permutaciones de verbos

Ejercite cada verbo a lo largo de **T** para cada ranura de operando. Lo que
sigue enumera las permutaciones *estructurales* (cláusulas y frases) que se
suman a la mezcla de tipos.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (varios receptores).
- `MOVE CORRESPONDING g1 TO g2` (elementales que coinciden por nombre).
- Origen y destino con modificación de referencia: `MOVE a(s:l) TO b(s:l)`.
- Con subíndices: `MOVE t(i) TO u(j)`, `t(i,j)`.
- Conversiones de tipo (aplicar **T** en ambos sentidos): num→editado,
  editado→num, alfanum→num, num→alfanum (justificar/rellenar/truncar),
  grupo→grupo (copia de bytes), tratamiento del signo, COMP‑3↔DISPLAY,
  flotante↔fijo, figurativo→cada clase.

### DISPLAY
- `DISPLAY {dn|literal} …` (operandos concatenados).
- `[WITH NO ADVANCING]`; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Forma de pantalla (observar y decidir): `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Mezcla de tipos: numérico (ancho completo de la PIC), editado, con signo,
  grupo, figurativo.

### ACCEPT  *(especificar todas las formas; muchas son de pantalla o terminal — marcar para decidir el alcance)*
- `ACCEPT dn` (de la consola a alfanum / numérico / editado / grupo).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Formas de pantalla: `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Recepción en numérico frente a numérico editado frente a alfanumérico
  (des-edición y validación).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Varios receptores, cada uno con su propio comportamiento de ROUNDED y de
  tamaño; operandos de USAGE mezclada (COMP‑3 + DISPLAY + editado); con signo;
  operandos con modificación de referencia.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- División por cero → ON SIZE ERROR; signo y escala de REMAINDER; USAGE mezclada.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Operadores `+ - * / **`, paréntesis, precedencia; funciones intrínsecas dentro
  de la expresión; operandos de USAGE mezclada; varios receptores; truncamiento
  frente a ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — anidamiento, ramas vacías,
  `NEXT SENTENCE`.
- Condiciones: de relación (`= < > <= >= NOT`), de clase
  (`IS [NOT] {NUMERIC|ALPHABETIC|ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), de signo
  (`POSITIVE|NEGATIVE|ZERO`), referencia a una condición 88, combinadas
  (`AND/OR/NOT`), **abreviadas** (`a = b OR c`), entre paréntesis.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` con
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Mezcla de tipos en las comparaciones (num frente a alfanum frente a editado
  frente a figurativo).

### PERFORM
- Fuera de línea: `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = literal o elemento de datos).
- `PERFORM … UNTIL cond` con `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- En línea: `PERFORM … END-PERFORM` (con TIMES/UNTIL/VARYING).
- PERFORM anidada y recursiva; solapamiento de rangos; índice frente a variable
  de bucle numérica.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`; `GO TO p1 p2 … DEPENDING ON dn` (dentro y fuera de rango).
- `CONTINUE`; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (desde el principal y desde un
  subprograma).

### SET
- `SET index TO {n|index}`; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}`; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (donde esté soportado).

### INITIALIZE
- `INITIALIZE dn …` (grupo o elemental; por defecto según la categoría).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]`; tablas (todas las ocurrencias).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (serie).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH`
  (binaria; requiere `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Encontrado y no encontrado; varios WHEN; mezcla de tipos de clave;
  comportamiento con la tabla sin ordenar.

### STRING  *(ejercitar el estilo de permutación del usuario)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutaciones que cubrir:
  - un solo origen con `DELIMITED BY SIZE` → destino alfanumérico.
  - varios orígenes, **delimitadores mezclados**: `STRING "lit" DELIMITED BY SIZE
    d1 DELIMITED BY SPACES INTO d3`.
  - muchos orígenes y delimitadores: `STRING "l1" DELIMITED BY SIZE "l2"
    DELIMITED BY SIZE d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` para empezar y avanzar; puntero fuera de rango →
    desbordamiento.
  - destino demasiado pequeño → `ON OVERFLOW`; `NOT ON OVERFLOW`.
  - **orígenes con mezcla de tipos:** numérico, numérico editado, con signo,
    grupo, figurativo, con modificación de referencia — observar cómo se convierte
    cada uno en cadena.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutaciones: uno frente a varios delimitadores, `ALL` (colapsar
  repeticiones), `OR`, captura con `DELIMITER IN`/`COUNT IN`, POINTER, TALLYING,
  más campos que datos (desbordamiento), destinos de tipos mezclados (los
  receptores numéricos se des-editan).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combinado).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- Alcance de BEFORE/AFTER; coincidencias solapadas; patrones de varios
  caracteres; anfitrión con mezcla de tipos.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Nombre de programa estático (literal) frente a dinámico (nombre de dato); sin
  resolver → ON EXCEPTION.
- Modos de paso de argumentos (observar la visibilidad desde el llamante);
  discrepancia de número o tipo de argumentos.
- `RETURNING` frente a `RETURN-CODE`; recursión; datos compartidos `EXTERNAL`.
  (✅ `CANCEL prog` implementado: reinicializa el almacenamiento del programa;
  `NOT ON EXCEPTION` se ejecuta con una CALL resuelta.)

### Registros especiales de aritmética y verbos varios
- `ADD/SUBTRACT … GIVING` (supresión a cero) frente a la acumulación de `TO`.
- `MOVE` y aritmética hacia y desde `RETURN-CODE`, `TALLY`.
- ✅ `ALTER` (el GO TO heredado) — implementado (redirige el `GO TO` del
  párrafo).
- Ida y vuelta de `ACCEPT/DISPLAY` a través de campos editados.

### Verbos de fichero — *(solo los huecos que no cubre el conjunto de E/S de ficheros)*
- ✅ **Implementado y probado** (`test_file_locking`): `OPEN … SHARING WITH …
  [WITH LOCK]`, `READ … WITH [NO] LOCK`, `UNLOCK` (consultivo dentro de la unidad
  de ejecución — véase la referencia de sintaxis soportada).
- `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  con claves modificadas por referencia; varios FD compartiendo un área de
  registro.

### Verbos especificados aquí antes de existir

> **Todos ellos están implementados.** SORT/MERGE/RELEASE/RETURN llegaron en
> 1.62.119 y el motor RELATIVE en 1.62.76
> (`crates/cobolt-runtime/src/relative.rs`); `docs/cobol85-supported-syntax-en.md`
> los marca con ✅. Los ejes de permutación de abajo se conservan como el plan de
> pruebas que siempre fueron: describen lo que falta *cubrir*, no lo que falta
> construir.
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- Organización `RELATIVE`: `READ/WRITE/REWRITE/DELETE/START` por
  `RELATIVE KEY`.

---

## Parte C — Banco de equivalencia entre formas

Para un conjunto escogido de los programas anteriores, comprobar que la salida
observable es **idéntica** (texto de DISPLAY, FILE STATUS, RETURN-CODE, contenido
de los ficheros) en las tres formas de ejecución del mismo fuente:

1. **Intérprete** (`Interpreter::run`).
2. **Ida y vuelta del AST** — serializar (`bincode`+`flate2`) → deserializar →
   ejecutar; comprobar que el AST es idéntico byte a byte y que la salida
   coincide.
3. **Binario empaquetado/compilado** — `cobolt_compiler::build_project` →
   ejecutar el binario producido; comprobar que la salida es idéntica.

Cualquier divergencia entre formas es un defecto que hay que registrar (el
invariante «un compilador, un comportamiento»).

.<<

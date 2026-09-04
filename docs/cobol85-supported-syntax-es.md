<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Referencia de la sintaxis soportada de RustCOBOL‑85

**Para qué sirve este documento:** para decir cuánto del estándar COBOL‑85
implementa realmente RustCOBOL — y para demostrarlo frente a la **suite oficial
de validación NIST COBOL‑85** en lugar de limitarse a afirmarlo. El
[marcador](#-la-conformidad-se-mide-no-se-afirma--nist-ccvs85) de más abajo es
el titular; todo lo que viene después es el detalle que hay tras esa cifra.

**Verdad de campo sobre lo que el lexer/parser/runtime de RustCOBOL acepta hoy
realmente**, derivada del código fuente (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`) y contrastada con `NIST/newcob.val,cbl`.
Escribe las pruebas contra las formas ✅; las formas ❌ no llegarán a analizarse o
son no‑operaciones, y las formas ⚠️ se analizan pero se comportan solo
parcialmente. Este es el documento complementario de
[`cobol85-verb-test-matrix-es.md`](cobol85-verb-test-matrix-es.md): la matriz dice
*qué* probar, y este dice *qué grafía entiende RustCOBOL*.

Leyenda: ✅ soportado · ⚠️ se analiza pero es parcial/simplificado · ❌ no
reconocido (evítalo, o pruébalo solo para confirmar la carencia).

---

## ★ La conformidad se mide, no se afirma — NIST CCVS85

**Este es el propósito del documento.** Cada afirmación de más abajo se contrasta
con la **suite oficial de validación NIST COBOL‑85** — CCVS85 versión 4.0
(01 OCT 1992, COBOL 85 versión 4.2, Apr 1993 SSVG), la suite que el National
Institute of Standards and Technology de los Estados Unidos usaba para
certificar compiladores COBOL. Ocupa 28 MB, 348,271 líneas, **459 programas
COBOL** y 51 miembros de copybook, y vive en este repositorio en
`NIST/newcob.val,cbl`.

Es la fuente de verdad. Donde RustCOBOL y CCVS85 discrepan, **CCVS85 tiene razón
y RustCOBOL está equivocado**, y la diferencia se registra como un defecto en
[`specs/nist/`](../specs/nist/README.md) — una especificación por corrección, con
los programas que fallan nombrados.

### El marcador

Medido el 2026‑08‑28 en la versión 1.62.43, sobre la distribución intacta:

| | Programas | Cuota | Significado |
|---|---:|---:|---|
| ✅ **PASS** | **422** | **97.2 %** | de los 434 programas dentro del alcance |
| ❌ **FAIL** | **12** | 2.8 % | de los 434 programas dentro del alcance |
| ⬜ **N/A** | **25** | — | módulos fuera del alcance de RustCOBOL (más abajo) |
| | **459** | | programas totales de la suite |

Reprodúcelo:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ Compilar es la afirmación más débil

La tabla de arriba cuenta los programas que **el front end acepta**. No dice que
se ejecuten. La suite se puntúa a sí misma — cada programa de CCVS85 imprime su
propio informe `PASS` / `FAIL*` — así que hay un segundo número, estrictamente
más fuerte: cuántos llegan al final y no informan **ningún fallo**.

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

Ambos números se informan por módulo, y nunca se mezclan:

| Módulo | Compilación | Ejecución (0 fallos) |
|---|---:|---:|
| **NC (Núcleo)** | **95 / 95** | **83 / 95** |

El trabajo avanza **de módulo en módulo**: NC no está terminado hasta que ambos
números llegan a 95, y no se trabaja en ningún otro módulo hasta entonces. Una
puntuación amplia de compilación repartida entre diez módulos no dice nada sobre
si alguno de ellos funciona.

##### Los cinco miembros de NC que necesitan algo más que un fichero de impresión — todos puntuados

La puntuación de ejecución considera limpio un programa cuando su **propio
informe CCVS** no muestra fallos. Cinco miembros de NC no imprimen tal informe, y
no porque nada esté roto. Cada uno necesitaba trabajo en el arnés en lugar de
trabajo en el compilador, y cada uno puntúa ya:

| Miembro | Qué necesita | Cómo se puntúa |
|---|---|---|
| **NC302M**, **NC303M**, **NC401M** | Pruebas de *marcado (flagging)*. No llevan maquinaria `PASS`/`FAIL` en absoluto — cada uno acaba con `TOTAL NUMBER OF FLAGS EXPECTED = n`, y el resultado que se valida es el conjunto de **diagnósticos que emite el compilador** para construcciones obsoletas (NC302M/NC303M) o para construcciones por encima del subconjunto alto (NC401M). | El arnés compara los diagnósticos con la propia lista de expectativas del miembro, línea a línea. Las dos clases se ejecutan en **pasadas separadas**: `DATE-COMPILED` es a la vez obsoleto *y* está por encima del subconjunto alto, así que una única pasada combinada le da a cada miembro las marcas del otro como falsos positivos. |
| **NC110M** | Escribe su informe con `DISPLAY`, hacia la consola del operador, no hacia el fichero de impresión CCVS que lee el arnés. | La salida de consola del proceso hijo se captura a un fichero y se puntúa desde ahí. |
| **NC109M**, **NC204M** | Prueban el `ACCEPT` de Formato 1, que lee del operador — NC109M escribiéndolo a secas, NC204M a través de un mnemónico que `SPECIAL-NAMES` asocia con el dispositivo de entrada. Se espera que el validador aporte la entrada; sin stdin toda comparación falla. | El arnés aporta un mazo del operador en el stdin del proceso hijo. El mazo se **recupera del código fuente, no se inventa**: cada elemento aceptado se compara con un elemento emparejado cuyo valor fija el programa justo encima del `ACCEPT`, así que cada línea del mazo es ese valor. |

Por tanto **no hay ningún techo estructural por debajo de 95** en el eje de
ejecución: todos los programas de NC dentro del alcance compilan, y cada uno de
ellos se puntúa por lo que él mismo informa.

El caso comparable que **sí** quedó zanjado es el conmutador externo. NC174A,
NC253A y NC254A prueban `ON STATUS` / `OFF STATUS` contra un conmutador que el
operador fija antes de la ejecución — nada dentro de COBOL puede fijar uno — así
que el arnés pasa ahora `--switch XXXXX051=ON --switch XXXXX052=OFF` (y las
grafías sustituidas `SWITCH-1` / `SWITCH-2`) exactamente como exigen las
instrucciones de ejecución de CCVS85. Eso es configuración que el procedimiento
de validación reclama, no un dedo en la balanza: un programa que no declara
ningún conmutador no se ve afectado.

#### ⚠️ Qué significa realmente PASS — lee esto antes de citar la cifra

Un programa cuenta como **PASS** cuando atraviesa el front end de RustCOBOL —
lexer, parser, analizador semántico — con **cero errores**, usando
`--source-format=fixed`.

Eso es conformidad de *compilación*. **No** es prueba de que el programa calcule
la respuesta correcta. Un programa de CCVS85 también imprime su propio recuento
`PASS`/`FAIL` cuando se ejecuta, y puntuar esa salida es la **siguiente etapa**
de este trabajo — no está incluida en los 332 — véase el marcador de ejecución
de más abajo. Dos casos medidos muestran por qué importa la distinción:

- 30 de los 35 programas de ficheros RELATIVE compilan limpiamente, y el runtime
  **no tiene ningún motor RELATIVE** — se ejecutarían y producirían resultados
  incorrectos en silencio.
- Un literal continuado a lo largo de dos líneas puede reensamblarse mal y aun
  así analizarse, dejando al programa con los datos equivocados.

Así que: **PASS = "RustCOBOL acepta todas las construcciones de este programa."**
Nada más, todavía.

#### 🔴 El marcador de ejecución — la cifra que significa "funciona"

Todo lo de arriba mide la **compilación**. Un programa de CCVS85 también *se
ejecuta* e imprime su propio recuento `PASS`/`FAIL`, y ese recuento es lo que la
suite existe para producir. Desde 1.62.15 el arnés los ejecuta:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

Medido el 2026‑08‑28 en 1.62.43. Bajo la REGLA DE ORO n.º 9 se termina un módulo
antes de empezar el siguiente: **NC (Núcleo) está completo en ambos ejes**, así
que **SQ (E/S secuencial)** es el módulo en curso.

**NC — Núcleo**

| | Programas |
|---|---:|
| dentro del alcance | 95 |
| no compilaron | 0 |
| llegaron al final | 95 |
| **…informando 0 fallos** | **95** |
| …informando fallos | 0 |
| se ejecutaron pero no imprimieron informe | 0 |
| agotaron el tiempo (>20 s) | 0 |
| se estrellaron o fueron rechazados por el runtime | 0 |

Las aserciones que los propios programas informan: **4 614 PASS / 0 FAIL**,
100 % de 4 614 puntuadas. (5 más están `DELETED` — el marcador propio de CCVS
para una prueba que el propio programa se salta.)

Como contraste, la misma tabla en 1.62.23 decía 65 limpios de 95, 4 278 PASS /
226 FAIL. Lo que se ha cerrado es la brecha entre "compila" y "funciona".

**SQ — E/S secuencial (en curso)**

| | Programas |
|---|---:|
| dentro del alcance | 85 |
| no compilaron | 0 |
| llegaron al final | 83 |
| **…informando 0 fallos** | **84** |
| …informando fallos | 1 |
| se ejecutaron pero no imprimieron informe | 0 |
| agotaron el tiempo (>20 s) | 0 |
| salida desbocada (>2 MB) | 0 |
| se estrellaron o fueron rechazados por el runtime | 0 |

Aserciones: **623 PASS / 1 FAIL**, 99.8 % de 624 puntuadas, y **todos los
programas llegan al final**. En 1.62.42 la misma tabla decía **10** limpios de
85, 20 estrellados, 1 con tiempo agotado y 215 PASS / 190 FAIL — el grupo de
caídas era un solo defecto, los párrafos declarativos perdiendo sus nombres; en
1.62.43 decía 44 limpios y 471 PASS / 162 FAIL. Los registros de longitud
variable, el área de registro compartida, las anchuras de `FILLER`,
`READ … INTO` y el `REWRITE` secuencial llegaron en 1.62.44; el `USE`
cualificado por modo, `CLOSE REEL/UNIT`, `SELECT OPTIONAL`, `LINAGE-COUNTER` en
el `OPEN` y las longitudes de registro fuera de rango en 1.62.45; los valores de
`LINAGE` dados por nombre de dato y los detectores de marcado de E/S secuencial
en 1.62.46.

Un miembro sigue quedándose corto:

| Miembro | Qué falta |
|---|---|
| SQ203A | Necesita `XXXXD001`, un fichero de datos que aporta la **instalación** de CCVS85. Ningún miembro de la suite lo escribe, así que la mitad "fichero presente" de su prueba de `SELECT OPTIONAL` no puede ejecutarse aquí; la mitad "fichero ausente" pasa. Esto es una entrada de instalación que falta, no un defecto de RustCOBOL. |

> Una línea de detalle `FAIL*` se escribe **dos veces** a propósito — el
> `PRINT-DETAIL` de CCVS ejecuta
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — mientras que `PASS ` se
> escribe una sola vez. Cualquier recuento crudo de marcadores tomado del
> fichero de impresión tiene que dividir los fallos por dos antes de significar
> algo.

Para leer *por qué* falla un programa, una tercera pasada imprime el detalle del
fallo que lleva su propio informe, listo para agruparlo en todo un módulo:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> Por eso la cifra de compilación se informa siempre como "RustCOBOL **acepta**
> estas construcciones". Citarla como un nivel de conformidad sería erróneo.

#### Por módulo

| Módulo | Qué prueba | PASS / Total | |
|---|---|---:|---|
| NC | Núcleo | **95 / 95** | ✅ completo — y completo también en **ejecución** (véase el marcador de arriba) |
| SQ | E/S secuencial | **85 / 85** | ✅ completo en compilación; **44 / 85 en ejecución** — el módulo en curso |
| IC | Comunicación entre programas | 45 / 47 | `END-CALL` llega al despachador de sentencias en lugar de ser consumido por su `CALL`; un nombre de condición con subíndice |
| IF | Funciones intrínsecas | **45 / 45** | ✅ completo |
| IX | E/S indexada | **42 / 42** | ✅ completo |
| SG | Segmentación | **13 / 13** | ✅ completo |
| ST | Ordenación / Fusión | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | E/S relativa | 34 / 35 | ⚠️ **solo compila — sin motor de ejecución.** `ORGANIZATION IS RELATIVE` se analiza y nunca se atiende en tiempo de ejecución, así que esta fila exagera la capacidad real. El único fallo es un `ELSE` colgante |
| SM | Manipulación del texto fuente (COPY/REPLACE) | 14 / 17 | un `$` dentro de un nombre de dato; pseudotexto cualificado/con subíndice; una forma de `PERFORM … VARYING` |
| DB | Depuración | 11 / 15 | `GO-TO` usado como palabra definida por el usuario, chocando con el par de palabras clave `GO TO`; un programa usa el verbo de Comunicación `DISABLE` |
| **Dentro del alcance** | | **422 / 434** | |
| CM | Comunicación | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | Marcado de características obsoletas | — | ⬜ N/A |
| EXEC85 | El propio programa controlador COBOL de NIST | — | ⬜ N/A |

### ⬜ N/A — qué queda fuera del alcance de RustCOBOL, y por qué

Estos 25 programas **no se cuentan como fallos**. Son características que
RustCOBOL no implementa ni piensa implementar. El razonamiento completo está en
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Módulo | Programas | Por qué queda fuera del alcance |
|---|---:|---|
| **CM** — Comunicación | 9 | `COMMUNICATION SECTION`, entradas `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Apunta a los monitores de teleproceso de los años 80 — colas de mensajes propiedad de un gestor de transacciones. Aquí no hay tal runtime, y el módulo se eliminó de los estándares COBOL posteriores. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entradas `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, cortes de control. Un sublenguaje declarativo extenso; la respuesta de PowerRustCOBOL a los informes es el Diseñador de Formularios y la exportación a PDF. Podría convertirse más adelante en una *característica* si se quiere — es la única exclusión con valor real para el usuario. |
| **OBSQ / OBIC / OBNC** | 9 | Estos vuelven a probar módulos anteriores y esperan que el compilador *marque* elementos obsoletos de COBOL‑85. Su contenido de lenguaje está cubierto por las especificaciones dentro del alcance; lo que queda fuera del alcance es el **marcado** de características obsoletas. |
| **EXEC85** | 1 | No es una prueba. Es el ejecutivo COBOL propio de NIST que parte la distribución y conduce la suite — sustituido aquí por un arnés en Rust, así que no necesita compilar. |

**COBOL orientado a objetos** también queda fuera del alcance de RustCOBOL, pero
CCVS85 es enteramente anterior — no hay programas OO en la suite.

### De dónde vienen los 192 fallos restantes

Cada uno es un defecto especificado, no una incógnita. Ordenados por el número de
programas en los que es su *primer* error:

| Programas | Causa raíz | Especificación |
|---:|---|---|
| 31 | coma separadora — `MOVE ZERO TO A, B, C` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [intrínsecas](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [carencias de sentencias](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | subíndices separados por espacios — `TBL (1  2)` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON` (nombres de conmutador) y `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md), [separadores](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [carencias de sentencias](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | `COPY` metido en el Área B o partido entre líneas | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | punto y coma separador — `START F ; INVALID KEY` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 4 | entero de `OCCURS` en la línea siguiente | [separadores](../specs/nist/NIST-spec-separators.md) |
| 4 | `SECTION` con un número de prioridad — `SORT-PARA SECTION 69.` | [segmentación](../specs/nist/NIST-spec-segmentation.md) |

> **La clasificación se mueve tras cada corrección, y los movimientos son
> informativos.** Tres filas que encabezaban esta tabla en versiones anteriores
> han desaparecido — las entradas de comentario de IDENTIFICATION, los literales
> numéricos y la comilla suelta. Cada vez, la mayoría de los programas de la fila
> despejada **no** empezaron a pasar; se mudaron a la fila de debajo. Los cuatro
> programas de SG liberados en 1.62.12 se paran ahora en
> `SORT-PARA SECTION 69.`, que es por lo que Segmentación sigue marcando 0 / 13.
> Vuelve a medir en vez de fiarte de una clasificación anterior.

### Historial de conformidad

| Versión | PASS / 434 | Qué cambió |
|---|---:|---|
| 1.62.7 | **0** | No compilaba nada. Faltaban dos reglas del formato de referencia clásico: las columnas 73‑80 se leían como código fuente, y las líneas de continuación nunca se unían. |
| 1.62.8 | **222** | `--source-format=fixed` — el formato de referencia clásico, incluida la continuación. Véase [Formatos de fuente](#formatos-de-fuente). |
| 1.62.10 | **237** | Los literales numéricos pueden empezar por un punto decimal (`.999`). Funciones intrínsecas 21 → 29, Núcleo 25 → 29, Ordenación/Fusión 27 → 30. |
| 1.62.11 | 241 | Párrafos de entrada de comentario de IDENTIFICATION. Depuración 5 → 9. Una ganancia menor de lo que sugiere el cubo de 32 programas: 9 de ellos son programas de Comunicación (N/A), y la mayoría del resto chocaba con un segundo bloqueo inmediatamente después. |
| 1.62.12 | 242 | Un literal queda confinado a su línea, así que una comilla suelta ya no puede cambiar la paridad de un fichero entero. Núcleo 29 → 30. El cubo de 6 programas se despejó: 4 pasaron a los números de prioridad de segmento, 1 pasa ya. |
| 1.62.13 | 292 | La coma y el punto y coma separadores son puntuación, no tokens; los subíndices pueden separarse solo con espacios; un subíndice puede seguir a un nombre cualificado completo; un delimitador duplicado dentro de un literal es un solo carácter. Núcleo 30 → 56, Entre programas 32 → 44, Indexada 31 → 38. Se vaciaron tres cubos de diagnóstico enteros. |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — una tabla entera como argumento de una intrínseca; `MOVE ALL "X"` rellena el campo; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`; un literal con signo como objeto de `WHEN`; `PERFORM … TIMES` con un contador en un elemento de datos; un contador entero escrito en una línea de continuación. **Funciones intrínsecas 45 / 45 — módulo completo.** |
| 1.62.15 | 332 | Un nombre de `FUNCTION` desconocido es un error de compilación en lugar de devolver 0; una palabra definida por el usuario puede empezar por un dígito (`25COUNT`, `3-DEM-TBL`, `0 SECTION.`); una línea `D` es un comentario salvo con `WITH DEBUGGING MODE`. Segmentación 0 → 10, Núcleo 58 → 61. |
| 1.62.16 | 376 | El `AT` de `AT END` es opcional, así que una cláusula `END` a secas ya no se traga la siguiente cabecera de párrafo (33 programas). El preprocesador COPY/REPLACE confina un literal a su línea, así que la palabra COPY del banner de copyright no es una directiva. Un literal numérico puede abrir con su punto decimal una lista de operandos de `ADD`/`SUBTRACT`. **E/S indexada completa, 42 / 42.** |
| 1.62.17 | 380 | El diseño de página de `LINAGE`, `LINAGE-COUNTER` y `WRITE … AT END-OF-PAGE` / `AT EOP` — implementados, no simulados. E/S secuencial 77 → 81. |
| **1.62.19** | **396** | Un elemento numeric-edited es un elemento numérico. El punto decimal de edición conserva el dígito que le sigue (`PIC ZZ,ZZZ.9` ya no se trunca a `ZZ,ZZZ`), y una picture construida solo con caracteres de edición — `ZZZZ`, `$.**`, `$**.**CR` — es numeric-edited y no alfanumérica. Ambas cosas hacían que un receptor `GIVING` aritmético legal pareciera no numérico. |
| **1.62.18** | **391** | Un número que abre una línea de continuación es un operando allí donde se espera una expresión. El `IS` es opcional en una condición de clase o de signo, y una condición puede ser sujeto de `EVALUATE`. Un nombre de procedimiento puede escribirse enteramente con dígitos, tanto en las referencias como en las cabeceras. |
| **1.62.21** | **417** | La pasada del Núcleo. `ALTER` es una serie y `GO TO.` es el GO TO alterado; un nombre de procedimiento todo dígitos conserva sus ceros iniciales; un nombre de condición puede llevar subíndice o cualificarse; una expresión aritmética entre paréntesis es un operando, no una condición anidada; `MULTIPLY`/`DIVIDE` en formato 1 admiten una serie de receptores; `WITH TEST` puede preceder a `VARYING` y un contador de repeticiones puede llevar subíndice; `PERFORM imperativo … END-PERFORM` no necesita ninguna cláusula; un nombre de párrafo puede cualificarse por su sección; el `ELSE` no se lo traga un imperativo de `ON SIZE ERROR` ni una rama ELSE anidada; las relaciones combinadas abreviadas aceptan objetos aritméticos y de clase/signo; `INSPECT` arrastra su categoría ALL/LEADING entre operandos y `CONVERTING` admite una región; `UNSTRING TALLYING` va detrás de `WITH POINTER`. **Núcleo 76 → 92 de 95 compilando, 16 → 28 ejecutándose limpios.** |
| **1.62.43** | **422** | **El módulo de E/S secuencial compila por completo — 85 de 85 — y pasa de 10 a 44 de 85 en ejecución.** Los párrafos de un declarativo conservan sus nombres, así que un manejador `USE` puede hacerles `PERFORM` y `GO TO` (20 programas dejaron de estrellarse); un elemento `FILE STATUS` declarado como *grupo* de dos caracteres recibe el código; el `OPEN` de un fichero ya abierto es `41` y no lo reabre; un `READ` secuencial después de `AT END` es `46`; y un mismo `OPEN` puede llevar varios grupos de modo (`OPEN INPUT f1 OUTPUT f2`), que es toda la ganancia de compilación. |
| **1.62.42** | **420** | **El módulo Núcleo está terminado — 95 de 95 compilando *y* 95 de 95 ejecutándose limpios, 4 614 aserciones sin ninguna que falle.** Un `66 RENAMES` se cualifica por su registro, cubre todas las ocurrencias de una tabla que abarca, y es el elemento que renombra cuando renombra exactamente uno; un 88 declarado sobre un grupo prueba los bytes del grupo; una constante figurativa se dimensiona según el otro operando, `VALUE` incluido; un operando de grupo es de categoría alfanumérica; un `NOT` delante del objeto de una abreviatura niega la relación; una serie `INSPECT … REPLACING` comparte un solo barrido y un elemento DISPLAY con signo no lleva un `-` entre sus caracteres; los solapamientos de `REDEFINES` se anidan; y se respeta `PERFORM … WITH TEST AFTER VARYING`, una variable `AFTER` se reinicia cuando su bucle acaba, y un identificador `VARYING` con subíndice sigue a su subíndice. Ese último grupo es la razón de que NC201A terminara siquiera. |

> **El resumen honesto.** RustCOBOL acepta hoy el **97.2 %** de la suite NIST
> dentro del alcance, partiendo de nada hace nueve versiones. Los 12 restantes no
> son un misterio — son defectos nombrados, cada uno especificado junto con los
> programas que bloquea. Esta tabla es la medida del progreso, y se actualiza con
> cada versión.
>
> **Y hay un módulo terminado en el eje que cuenta.** El Núcleo ejecuta 95 de 95
> programas limpios, no meramente los compila — véase el marcador de ejecución de
> más arriba. Bajo la REGLA DE ORO n.º 9 esa es la puerta para empezar el módulo
> siguiente, así que **la E/S secuencial está ahora en curso**: completa en
> compilación, 44 de 85 en ejecución.

---

> **Actualización (pasada de implementación de carencias):** se implementaron los
> siguientes y ahora son ✅ — **modificación de referencia** `id(start:len)`,
> **`PERFORM n TIMES` en línea**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING
> `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` consciente de la
> categoría**, **condiciones abreviadas con prefijo de operador**
> (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (se ejecuta con un CALL sin
> resolver), **múltiples receptores de `COMPUTE` + `ROUNDED` por receptor**, y un
> conjunto de **funciones intrínsecas** mucho mayor.
>
> **Actualización (pasada de entorno jerárquico / consciente de ocurrencias —
> 1.5.0):** cuatro características bloqueadas por el modelo de datos son ahora
> ✅ — **subíndices de tabla en tiempo de ejecución** `t(i)` / `t(i, j)`
> (almacenamiento por ocurrencia), **desambiguación por nombre cualificado**
> `id OF/IN group` (los nombres de hoja duplicados se resuelven a
> almacenamientos independientes), **`MOVE/ADD/SUBTRACT CORRESPONDING`**, y
> **`SEARCH` / `SEARCH ALL` funcionales**.
>
> **Actualización (pasada de completitud de verbos — 1.6.0):** ahora también ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` con múltiples receptores + `ROUNDED` por
> receptor** en `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** y el `EXIT` simple corregido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado y las regiones
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de fecha/finanzas
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condiciones abreviadas con objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-sujeto) y **`WHEN NOT`**;
> **nombres de condición de nivel 88 reales** (`SET … TO TRUE/FALSE`, el anfitrión
> se prueba contra sus VALUE/rangos); **`PERFORM para VARYING`**; y un runtime
> **`SORT`/`MERGE`** funcional (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). La lista de "evitar" del final está al día.
>
> **Actualización (pasada de despeje de la lista de "evitar" — 1.7.0):** las
> carencias restantes están ya implementadas — **abreviatura con objeto
> identificador** (`a = b OR c`, resuelta mediante metadatos de nivel 88);
> **`INITIALIZE … REPLACING category DATA BY value`**; **`66 RENAMES`** (la
> lectura sintetiza / la escritura distribuye entre los elementos cubiertos);
> **punteros** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> aliasing con `SET ADDRESS OF item TO …`, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; un **`NEXT SENTENCE`** fiel; las **intrínsecas** estándar que
> quedaban (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`,
> `TEST-NUMVAL`); y el **`ACCEPT`/`DISPLAY` de pantalla** extendido (`AT`/`WITH`
> vía ANSI en modo CLI — ahora *ejecutado*, no solo analizado).
>
> **Actualización (1.7.1):** las fuentes de registro de `ACCEPT` son ya
> funcionales (eran no-operaciones reconocidas) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (emparejadas con
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (emparejada con
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Actualización (1.7.2):** las cláusulas de compartición / bloqueo de ficheros y
> `CANCEL` (eran ❌ / no-operaciones) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libera los bloqueos de registro
> INDEXED del fichero), y **`CANCEL program`** (reinicializa el almacenamiento del
> programa).
>
> **Actualización (1.8.0):** **`COMMIT` / `ROLLBACK`** son ya verbos COBOL de
> verdad — transacciones controladas por el programa sobre los ficheros INDEXED
> abiertos (tanto en el motor de memoria como en el de disco). El motor de disco
> ganó un registro de deshacer real dentro de la ejecución (antes era una
> no-operación). La lista de "evitar" del final está al día.

---

## Párrafos de la IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Los párrafos de **entrada de comentario** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — en **cualquier orden y en
  cualquier subconjunto**.
- ✅ `REMARKS` también se acepta. Se eliminó de COBOL en 1985, así que no se
  almacena; se admite para que el código heredado de COBOL‑74 siga compilando.

Una **entrada de comentario** es texto libre, y COBOL‑85 lo dice literalmente:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Puede contener **palabras reservadas** — el `DATA` de arriba no abre una DATA
  DIVISION.
- Puede contener **puntos**, y no termina en uno de ellos.
- **Abarca tantas líneas** como escribas.
- Termina en el siguiente encabezado de párrafo o de división que **empiece una
  línea** en el Área A — así es como la entrada anterior termina en
  `DATE-WRITTEN`.

**Una comilla dentro de esa prosa queda contenida en su línea** (desde 1.62.12).
Un texto como `THE COMPILER"S ABILITY` ya no abre un literal que se prolonga por
el resto del programa — véase [Formatos de fuente](#formatos-de-fuente). Sigue
mereciendo la pena evitar una comilla sin pareja en una entrada de comentario,
pero ahora te cuesta esa línea, no el fichero.

⚠️ `INSTALLATION`, `SECURITY` y `REMARKS` **no son palabras reservadas** aquí.
Solo se reconocen como nombres de párrafo dentro de la IDENTIFICATION DIVISION,
así que un elemento de datos llamado `SECURITY` sigue funcionando.

---

## Formatos de fuente

RustCOBOL lee tres disposiciones de fuente. La elección es explícita — **nunca**
se adivina a partir del contenido del fichero, porque aplicar reglas de columnas
a un fuente que no se escribió para ellas borra código en silencio.

| `--source-format` | Qué significa |
|---|---|
| `free` | Ninguna regla de columnas. `*>` inicia un comentario. **El valor por defecto**, y lo que usan los propios proyectos de PowerRustCOBOL y los ficheros `.cbl` de formulario generados. |
| `fixed` | ✅ **Formato de referencia clásico de COBOL-85** — la disposición que define el estándar y en la que se escribe el fuente en imagen de tarjeta. Véase más abajo. |
| `fixed-relaxed` | Se respetan el área de secuencia y la columna indicadora, pero la línea llega hasta donde la hayas escrito — sin límite de 72 columnas. |
| `auto` | Comportamiento histórico: `free`, salvo que `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` fija el valor por defecto de una sesión.

### `fixed` — el formato de referencia clásico

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Columnas 1-6** — área del número de secuencia, ignorada.
- **Columna 7** — área indicadora:
  - `*` o `/` → línea de comentario
  - `-` → **continuación** de la línea anterior
  - `D` → línea de depuración; un comentario (el modo de depuración todavía no
    está implementado)
  - cualquier otra cosa → se lee como fuente normal. El estándar reserva esta
    columna, pero las suites en imagen de tarjeta la usan como selector de
    líneas opcionales, y descartar esas líneas en silencio borraría código.
- **Columnas 8-72** — el fuente.
- **Columnas 73-80** — área de identificación, **descartada**.

### Líneas de continuación ✅

Un guion en la columna 7 continúa la línea anterior.

**Continuar una palabra o un literal numérico** — los espacios finales de la
línea continuada se descartan y las dos mitades se juntan sin nada en medio:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

declara un único elemento llamado `WRK-DS-18V00-CONTINUED`.

**Continuar un literal alfanumérico** — el literal de la línea continuada no
lleva comilla de cierre; la línea de continuación debe reabrir con una, y el
literal se reanuda en el carácter siguiente:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **El fragmento continuado llega hasta la columna 72, espacios finales
incluidos.** Una línea que se queda antes de la columna 72 aporta igualmente
esos espacios al literal. Por eso un literal continuado solo es exacto byte a
byte bajo `fixed`; los demás formatos no tienen una columna 72 en la que parar.

### Un literal nunca abarca una línea por accidente ✅

La continuación es la **única** manera de que un literal alcance varias líneas.
Una comilla que no se cierra en su propia línea es un error, y se informa donde
está escrita:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Esto importa más de lo que parece. Antes de 1.62.12 una comilla sin pareja
llegaba hasta la *siguiente* comilla en cualquier punto del fichero, así que una
sola `"` perdida en un comentario se tragaba divisiones enteras y desplazaba el
emparejamiento de todas las comillas posteriores — los programas NIST donde se
encontró esto tienen un número **par** de comillas, así que nada quedaba sin
terminar; un solo carácter había cambiado la paridad del fichero entero. El daño
ahora se detiene en el salto de línea.

> **El formato libre no tiene continuación de literales.** Ni `&` — ese es el
> *operador* de concatenación — ni un bloque delimitado. Un literal en formato
> libre debe caber en una sola línea; para uno largo, concaténalo:
> `"first part" & "second part"`.

> **Nota.** Elegir `fixed` para un fichero escrito en formato libre lo dañará —
> todo lo que pase de la columna 72 desaparece, y el texto anterior a la columna
> 8 se lee como número de secuencia. Úsalo solo con fuente que sea de verdad
> imagen de tarjeta.

---

## Sentencias reconocidas (verbos)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirige el `GO TO` de para-1) ·
`UNLOCK file` (libera los bloqueos de registro del fichero) ·
`OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (compartición/bloqueo de ficheros — orientativo dentro
de la única unidad de ejecución)
✅ `COMMIT` / `ROLLBACK` (transacciones sobre ficheros INDEXED controladas por el
programa — véase Verbos de fichero) · `CANCEL` (reinicializa el almacenamiento
del programa) ·
⚠️ `INVOKE` (se analiza como una operación nula)
Extensiones del proyecto: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Un bloque puede hacer `use` de los crates siempre enlazados (std, egui,
eframe y el conjunto del runtime enlazado) **más cualquier crate que el proyecto
registre en Project's Crates** (especificación 044): los crates registrados se
fijan a una versión exacta, se copian (vendoring) en el `crates/` del proyecto y
se compilan dentro del binario; los crates no registrados hacen fallar
Check/Build en la línea del desarrollador, con el remedio indicado.

✅ `SEARCH` (secuencial) / `SEARCH ALL` (búsqueda binaria sobre una tabla con
`ASCENDING`/`DESCENDING KEY` — ejecuta el primer `WHEN` que coincida y, si no
hay ninguno, `AT END`).
✅ `SORT` / `MERGE` con `RELEASE` / `RETURN` (funcionales — véase más abajo).
✅ `DECLARATIVES … END DECLARATIVES` con `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — manejadores de errores de fichero que
se disparan ante un `FILE STATUS` de error no tratado. **Se entra en un
manejador por el principio de su sección y se ejecuta hasta el final de esa
sección**, y sus párrafos conservan sus nombres, así que puede hacerles
`PERFORM` y `GO TO` — incluido un párrafo de *otra* sección declarativa. Los
párrafos declarativos viven en su propio espacio de nombres: el control nunca
cae desde el cuerpo principal hacia ellos, y un nombre declarado en ambos sitios
se resuelve a la copia de la declarativa mientras se ejecuta un manejador, y a
la del cuerpo en todo lo demás. Una declarativa también puede hacer `PERFORM` de
un párrafo de la parte no declarativa.
❌ **No reconocidos — no los uses:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas admitidas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (varios receptores).
- ✅ **Un operando de grupo vuelve alfanumérico todo el movimiento** (COBOL-85 6.18.4).
  La PICTURE del otro operando aporta su *tamaño* y nada más: sin edición, sin
  des-edición, sin conversión numérica. `MOVE <group holding "123ABC">`
  deja `"123ABC "` en un `PIC 0XXXXX0` (no el editado `"0123AB0"`), los mismos
  seis caracteres y un espacio en un `PIC 9999V999`, y `"12"` en un `PIC 99`.
  `JUSTIFIED RIGHT` sigue decidiendo qué extremo se rellena y cuál se pierde.
  La misma regla gobierna los bytes propios de un grupo: cada hijo toma su
  porción tal cual, así que un hijo alfanumérico editado **no** se reedita.
- ✅ **Una cláusula `VALUE` sobre un grupo** inicializa los bytes del grupo y se
  reparte entre sus hijos — `01 G VALUE "$123.45". 02 E PIC $999.99.`
  deja `E` con `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — mueve cada elemento subordinado que los dos
  grupos comparten por nombre, recorriendo recursivamente los subgrupos que coinciden.
- ✅ **`CORRESPONDING` excluye un elemento descrito con `REDEFINES` o `RENAMES`**
  (COBOL-85 6.18.4 GR1), en cualquiera de los dos lados, junto con todo lo
  subordinado a él. La exclusión recae sobre la *declaración*, no sobre el nombre:
  un elemento normal que solo comparte su nombre con un nivel 66 de otro sitio sigue correspondiendo.
- ✅ **Cualquiera de los dos operandos de `CORRESPONDING` puede nombrar una ocurrencia
  de una tabla de grupos** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` escribe en
  los huecos propios de esa ocurrencia, y el subíndice se arrastra por la recursión.
- ✅ **A un par le basta con que UNO de sus dos elementos sea elemental.** Un grupo
  puede enfrentarse a un elemento elemental, y el movimiento entre ambos es
  alfanumérico: un elemento elemental `PIC XXX` que envía a un grupo de `999` + `XXX`
  llena sus seis caracteres, y un grupo de `XXX` + `99` que envía a un simple `X(5)` lo llena.
  Dos grupos enfrentados siguen **recursando** — ese emparejamiento no es el caso
  elemental. *(Antes de 1.62.39 ninguna de las dos direcciones movía nada en absoluto: un
  grupo no posee hueco de almacenamiento, así que la escritura iba donde nadie la vuelve a leer y
  la lectura devolvía la cadena vacía.)*
- ✅ **Modificación de referencia `id(start:len)`** — emisor (subcadena) y receptor
  (asignación parcial empalmada); funciona sobre los operandos de todos los verbos. `length` es opcional.
  Direcciona **posiciones de carácter**, así que un operando numérico se toma con todo su
  ancho de `PIC` y sus ceros a la izquierda: `01 T PIC 9(8) VALUE 00224845` da
  `T(1:2)` = `"00"`, no `"22"`.
- ✅ **Los elementos de grupo son agregados alfanuméricos** — un grupo *es* sus elementos
  subordinados puestos uno tras otro, y su tamaño es la suma de los de ellos. Leer uno
  concatena los hijos (incluido el `FILLER`); mover a uno reparte los
  bytes entre ellos según su ancho. `MOVE 11 TO A` se ve a través del grupo que
  contiene `A`, y `MOVE "1234" TO G` fija los hijos de `G`, no un hueco propio.
- ✅ subíndices `t(i)`, `t(i, j)` — leen/escriben el hueco de almacenamiento de cada ocurrencia;
  los subíndices variables `t(WS-I)` se evalúan en cada acceso.
- ✅ cualificación `id OF/IN group` (`… OF g1 OF g2`) — resuelve al elemento
  correcto incluso cuando el nombre de la hoja está declarado bajo más de un grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por receptor** — cada receptor lleva su propio indicador `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico que
  coincide, recorriendo recursivamente los subgrupos que coinciden.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **varios receptores `GIVING`**, cada uno con su propio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sin `GIVING`) guarda `a/b` de vuelta en `a` (una comodidad de
  PowerRustCOBOL; el COBOL estándar exige aquí `INTO` o `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **varios receptores, cada uno con su propio `ROUNDED`**.
- ✅ operadores de expresión `+ - * /` y `**` (potencia, asociativa por la derecha), paréntesis,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **`ALSO` con varios sujetos** — cada columna de `WHEN` se compara posicionalmente
  con su sujeto y se combina con AND.
- ✅ **`WHEN NOT value`** niega un objeto de selección; **`WHEN condition`**
  (p. ej. `EVALUATE TRUE WHEN a > b`) evalúa la condición booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal entero o elemento de datos).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en línea,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en línea (sin párrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — ejecuta el párrafo en cada
  iteración (fuera de línea, sin `END-PERFORM`).
- ✅ **`WITH TEST AFTER` se aplica a `VARYING`**, escrito a cualquiera de los dos lados de la
  frase, y tanto en línea como fuera de línea. El cuerpo se ejecuta una vez antes de que se
  pruebe nada, y las condiciones se prueban entonces **de la más interna hacia fuera**; el nivel cuya
  condición es falsa se incrementa, todos los niveles interiores vuelven a su valor `FROM`,
  y el cuerpo se ejecuta otra vez. Una variable solo se incrementa cuando su prueba
  resulta falsa, así que la prueba que termina el bucle la deja tal como la dejó el cuerpo.
- ✅ **Una variable de `AFTER` se repone a su valor `FROM` cuando su bucle termina**,
  antes de que se incremente el nivel inmediatamente superior (COBOL-85 6.20.4 GR10(d)). Terminado
  el `PERFORM` completo, las variables interiores contienen sus valores `FROM` y solo la
  más externa conserva el valor que lo terminó.
- ✅ **Un identificador de `VARYING` con subíndice sigue a su subíndice.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` incrementa
  la ocurrencia que `S1` seleccione en ese momento, así que un cuerpo que avanza `S1`
  recorre la tabla.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **El cualificador `{OF|IN} section` elige de qué copia se habla** cuando un
  nombre de párrafo se repite en varias secciones, exactamente igual que en `PERFORM`. Una
  sección **desconocida** recae en la búsqueda sin cualificar en lugar de perder
  el salto. `GO TO … DEPENDING ON` toma una lista simple de nombres y ningún cualificador,
  y un `GO TO` que un `ALTER` haya redirigido sigue la redirección — que nombra
  su propio destino sin rodeos. *(Antes de 1.62.39 el cualificador se analizaba y luego se
  ignoraba, así que el salto caía en la primera definición que hubiera en cualquier parte del programa.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ `EXIT` a secas es un punto de retorno sin efecto; `EXIT PROGRAM` vuelve al llamador.
- ✅ `EXIT PERFORM [CYCLE]` (romper / continuar el PERFORM en línea más cercano),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfiere el control más allá del siguiente límite de sentencia (el
  analizador inserta marcas de límite en cada punto; fiel, no un simple `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lee del operador** cuando `SPECIAL-NAMES` declara
  el mnemónico (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — ese es el Formato 1, idéntico a un `ACCEPT id` a secas.
  Un nombre que **ninguna cláusula `SPECIAL-NAMES` declara** conserva la extensión
  de PowerRustCOBOL y lee la **variable de entorno** de ese nombre. Cuál de los
  dos se aplica lo decide la declaración, nunca la grafía.
  *(Antes de 1.62.35 la cláusula corriente `<implementor-name> IS <mnemonic>` se
  saltaba por completo, así que todos los mnemónicos leían una variable de entorno que
  nunca se había fijado y el elemento receptor quedaba vacío.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` sitúa el cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (la línea de órdenes entera) · `FROM ARGUMENT-NUMBER` (número de argumentos)
  · `FROM ARGUMENT-VALUE` (el argumento en el puntero fijado por `DISPLAY n UPON
  ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE` (la
  variable nombrada por `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` cierra la sentencia (opcional).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` cierra la lista de operandos (opcional), de modo que
  `DISPLAY A END-DISPLAY DISPLAY B` son dos sentencias y no una.
- ✅ formas de pantalla `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — se ejecutan mediante posicionamiento
  de cursor ANSI + SGR en **modo CLI** (`rcrun`); se ignoran en modo GUI (allí el diseñador
  de formularios sustituye a la E/S de SCREEN). `ACCEPT id AT …` sitúa el cursor y luego lee.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Desbordamiento = la cadena ensamblada es más ancha que el campo receptor.
- ✅ **Una frase `DELIMITED BY` gobierna toda la serie de emisores que la precede**,
  no solo aquel tras el cual está escrita:
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` delimita los tres y
  construye `"ABC"`. Una sentencia puede llevar varias frases, cada una gobernando los
  emisores desde la anterior; los emisores posteriores a la última frase se toman enteros.
  *(Antes de 1.62.40 solo se delimitaba el emisor escrito inmediatamente antes de la
  frase.)*
- ✅ **`INTO` un elemento de grupo** reparte entre los elementos subordinados del grupo.
- ✅ **El resultado se ensambla byte a byte**, así que `STRING HIGH-VALUE` mueve el
  único byte `0xFF` y ocupa una posición de carácter.
- ✅ **Extensión — `DELIMITED BY` por omisión inteligente** (cuando ninguna frase gobierna un
  operando): los elementos alfanuméricos `PIC X`/`A` toman `SPACES` por omisión (se descarta el
  relleno final); los literales de cadena, los elementos numéricos, los numérico-editados, los resultados
  de `FUNCTION` y las expresiones toman `SIZE`. Los elementos de datos se mueven en su forma de campo
  (numérico → dígitos con todo el ancho de la PIC; numérico-editado → caracteres editados).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Desbordamiento = más campos de origen que
  receptores.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **se aplican las dos mitades**.
- ✅ `BEFORE/AFTER INITIAL` confina cada frase a una subregión del campo.
  (TALLYING acumula sobre el contador, como manda COBOL.)
- ✅ **Una serie de operandos TALLYING comparte UN ÚNICO recorrido de izquierda a derecha** (COBOL-85
  6.17.3). En cada posición de carácter se prueban los operandos en el orden en que
  fueron escritos; el primero que coincide se queda la posición y el recorrido continúa
  más allá de los caracteres que consumió. Así que `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"`
  sobre `"AABA"` da `t1 = 1, t2 = 1` — escribir los operandos al revés
  da `t1 = 3, t2 = 0`. `LEADING` tiene que coincidir desde el borde izquierdo de su ventana sin
  hueco, así que un operando anterior que se quede esa posición acaba la racha antes de que empiece,
  y `CHARACTERS` cuenta solo las posiciones que ningún operando anterior reclamó.
- ✅ **Una serie de operandos REPLACING comparte también UN ÚNICO recorrido**, por la misma regla:
  el primer operando que coincide en una posición sustituye esos caracteres y el
  recorrido continúa más allá de ellos, así que ningún operando posterior puede verlos. La ventana
  `BEFORE`/`AFTER` de cada operando se fija **antes de cualquier sustitución**, que es lo que
  permite anclar un operando en caracteres que otro anterior sobrescribe:

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Aplicadas de una en una, la primera frase borraría el `"L "` en el que está anclada
  la segunda, y `"BAD"` sobreviviría.
- ✅ **Un elemento DISPLAY con signo no tiene ningún `-` entre sus posiciones de carácter.** El
  signo operacional es una sobreperforación sobre un dígito, así que
  `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` da **0** mientras que
  `FOR ALL "5"` da 1. El signo se restaura después, así que un `REPLACING` sobre
  los dígitos lo deja intacto. `SIGN IS … SEPARATE CHARACTER` es el caso en que el
  signo *sí* es una posición, y entonces se cuenta.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado a MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` pone en el elemento anfitrión el primer VALUE de la condición;
  `TO FALSE` pone un valor fuera del conjunto de VALUE (con el mejor esfuerzo — no hay cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` y
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — véase **Punteros** más abajo.

### INITIALIZE
- ✅ `INITIALIZE id …` — consciente de la categoría: numérico / numérico-editado → ZERO,
  todo lo demás → SPACES, recorriendo recursivamente los elementos de grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — pone cada elemento
  subordinado de esa categoría al valor; los demás quedan intactos.

### Punteros (USAGE POINTER)
- ✅ `USAGE POINTER` declara un puntero (NULL al principio).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — hace de `id` un alias del
  almacenamiento del destino (las lecturas **y** las escrituras siguen el alias); habitualmente un registro
  de LINKAGE. `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ El cuerpo de `ON EXCEPTION` / `ON OVERFLOW` se ejecuta cuando el programa llamado no
  se resuelve; el cuerpo de `NOT ON EXCEPTION` se ejecuta cuando la llamada **sí se resuelve**.
- ✅ `CANCEL program …` reinicializa la WORKING-STORAGE del programa nombrado, de modo que su
  siguiente `CALL` empieza de cero.

### Verbos de ficheros (las frases admitidas — la cobertura completa está en la suite de E/S de ficheros)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` se analizan y se respetan donde tienen sentido — son
  consultivos en el modelo de una sola unidad de ejecución.)
- ✅ **Un solo `OPEN` puede llevar varios grupos de modo**, cada uno con sus propios ficheros:
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Cada grupo se abre en su propio
  modo; `SHARING` / `WITH LOCK` / `REGISTERED USER` se aplican a toda la sentencia.
- ✅ **Un `OPEN` de un fichero que ya está abierto es `41`**, y el fichero queda como
  estaba — la sentencia **no** lo vuelve a abrir. (Reabrir un fichero `OUTPUT`
  truncaría en silencio lo que el programa ya hubiera escrito.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensión de
  PowerRustCOBOL) — registra al operador/usuario en el registro de observabilidad de INDEXED
  (campo `user=` en cada línea de evento de la sesión de ese fichero). Es puramente
  observacional; sin autenticación ni autorización. Véase
  [`observability-es.md`](observability-es.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libera el bloqueo de registro que el motor INDEXED toma en I-O.
- ✅ **`READ … INTO id` es el `READ` seguido de un `MOVE` de grupo.** El registro se
  reparte entre los elementos subordinados del receptor según su ancho y se corta al
  ancho del propio receptor, el receptor puede llevar subíndice, y el movimiento transporta
  bytes — un registro que contenga un byte que no sea un carácter llega intacto.
- ✅ **Cláusula `RECORD` de la FD — registros de longitud variable.** Las tres grafías:
  `RECORD CONTAINS n CHARACTERS` (fija), `RECORD CONTAINS n TO m CHARACTERS`
  (variable; la descripción de registro que nombra el `WRITE` da la longitud), y
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (el elemento de datos *es* la longitud — se fija antes de un `WRITE`, un `READ` la vuelve a fijar,
  y se recorta al rango declarado). Una FD cuyos registros `01` difieren en tamaño es
  de longitud variable lo diga o no. Un fichero de longitud variable guarda la longitud
  de cada registro junto con el registro, así que sus bytes **no** son intercambiables con los de
  un fichero de longitud fija; un fichero de longitud fija no cambia.
- ✅ **Los registros `01` de una FD describen una única zona de registro.** Un `READ` entrega los
  bytes a través de todas las descripciones de registro; un `WRITE` envía la zona entera, así que lo que
  otra descripción de registro haya puesto donde la escrita tiene `FILLER` se
  transparenta.
- ✅ **`FILLER` ocupa sus bytes en un registro de FD**, y
  `SIGN IS SEPARATE CHARACTER` hace que un elemento DISPLAY con signo sea un carácter más ancho
  que sus posiciones de dígito.
- ✅ **`LINAGE` de la FD admite nombres de datos además de enteros** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. La página se
  mide a partir de esos elementos en cada `WRITE`, así que un programa puede redimensionarla mientras
  se ejecuta. `LINAGE-COUNTER` vale uno cuando se abre el fichero.
- ✅ **Un `READ` secuencial después de `AT END` es `46`, no un segundo `10`.** El
  `AT END` no dejó ningún registro siguiente válido, así que seguir leyendo es un error distinto de
  llegar al final. `46` es un estado de clase 4, así que ni `AT END` ni
  `NOT AT END` se ejecutan para él — quien lo trata es la declarativa `USE` del fichero.
  Un `OPEN` nuevo, o un `START` con éxito, vuelve a establecer un registro.
- ✅ `UNLOCK f [RECORD[S]]` libera los bloqueos de registro del fichero.
- ✅ **`COMMIT` / `ROLLBACK`** — transacciones controladas por el programa sobre **todos** los
  ficheros INDEXED abiertos. `OPEN` inicia una transacción; `COMMIT` confirma los
  `WRITE`/`REWRITE`/`DELETE` pendientes (un `ROLLBACK` posterior ya no puede deshacerlos) y
  comienza otra; `ROLLBACK` deshace todos los cambios desde el último `COMMIT`/`OPEN`.
  El almacenamiento **DISK** hace que `COMMIT`/`CLOSE` sean duraderos en disco. El almacenamiento **MEMORY**
  mantiene `COMMIT`/`ROLLBACK` puramente en RAM (nunca escribe en disco); un fichero
  `STORAGE IS MEMORY` a secas es efímero, y `STORAGE IS MEMORY WITH PERSISTENCE`
  guarda en disco solo al `CLOSE`. (La recuperación tras caída mediante un registro de escritura
  anticipada duradero queda para el futuro — esto es una reversión de programa, dentro de la ejecución.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (ficheros INDEXED; extensión de PowerRustCOBOL). El almacenamiento por omisión es
  `DISK`. `WITH COMPRESSION` comprime el registro almacenado (las claves se evalúan sobre el
  registro sin comprimir); `WITH PERSISTENCE` (solo con MEMORY) guarda el fichero en RAM al
  `CLOSE`. `OPEN OUTPUT` siempre (re)crea el contenedor en disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` sobre un fichero SEQUENTIAL de registros** sustituye en el sitio el registro que
  entregó el último `READ`, y deja la posición de lectura donde estaba — el
  siguiente `READ` sigue dando el registro que va detrás. Los estados que debe:
  **`49`** cuando el fichero no está abierto en `I-O`, **`43`** cuando ningún `READ` con éxito
  estableció un registro (incluido después de `AT END`, y en un segundo `REWRITE` sin
  `READ` en medio), y **`44`** cuando el registro nuevo no tiene la misma longitud que
  el leído — en un fichero con `DEPENDING ON` el valor del elemento es esa longitud, que es
  como un programa pide otra distinta.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ El uso compartido de ficheros entre *procesos* no se impone (una sola unidad de ejecución); las
  frases `SHARING`/`LOCK` se analizan y se respetan los bloqueos de registro por ejecución del
  motor INDEXED.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcional, con búfer de trabajo en memoria)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dentro de un INPUT PROCEDURE) añade a la ejecución;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devuelve los registros.
- Los registros se ordenan de forma estable por las claves declaradas (`ASCENDING`/`DESCENDING`);
  `USING` lee / `GIVING` escribe los ficheros secuenciales nombrados.

---

## Condiciones (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionales: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relaciones en palabras: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Clase: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Un elemento cuya PICTURE no lleva **signo operacional** es `NUMERIC` solo
  cuando todas sus posiciones de carácter contienen un dígito — un `PIC X(5)`
  que contiene `"+1234"`, `"1.234"` o `"12 45"` **no** es numérico. *(Antes de
  1.62.40 la prueba analizaba los caracteres como un número, así que se
  aceptaban un signo, un punto decimal, un exponente y los espacios de
  alrededor.)*
- ✅ **El operando de una `CLASS` definida por el usuario puede ser una posición
  ordinal** — `CLASS ORDINAL-A-ONLY IS 66` nombra el carácter 66.º del juego
  nativo — y el operando puede ir en su propia línea de fuente. Lo mismo vale
  para `ALPHABET`.
- ✅ Signo: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nombre de condición de nivel 88 (el nombre suelto como condición).
- ✅ **`TRUE` / `FALSE` como operandos** (extensión de PowerRustCOBOL) — azúcar
  sintáctico para `1` y `0`, allí donde se permita un valor: `IF x = TRUE`,
  `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (la forma con `NOT` suelto, sin
  operador relacional), `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`,
  `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE`, y `WHEN TRUE` frente a un
  sujeto que es un valor. Un `TRUE`/`FALSE` suelto también es una condición
  completa (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ Esto **no** cambia los dos sitios en los que esas palabras ya significaban
  algo: `SET <88‑name> TO TRUE` sigue dando al elemento anfitrión un valor que
  satisface la condición (no el número 1), y `EVALUATE TRUE`/`EVALUATE FALSE`
  más abajo siguen siendo la sentencia de casos estándar.
- ✅ `AND` / `OR` / `NOT` combinados, con paréntesis (AND liga más fuerte que OR).
- ✅ **Condiciones abreviadas con el operador delante** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (se reutiliza el sujeto de la comparación anterior).
- ✅ **Abreviatura con objeto literal** — `a = 1 OR 2 OR 3` (reutiliza tanto el
  sujeto como el operador; el objeto es un literal).
- ✅ **Abreviatura con objeto identificador** — `a = b OR c` (donde `c` es un
  elemento de datos). Un identificador suelto tras AND/OR después de una
  comparación se resuelve en tiempo de ejecución: si es un nombre de condición
  de nivel 88 conocido, se evalúa como tal; si no, es el objeto de `a = c`. (Un
  identificador seguido inmediatamente de `AND` conserva la precedencia de AND.)
- ✅ **Un `NOT` delante del *objeto* de una abreviatura niega la relación**, no
  el objeto: `a > b OR NOT c` es `a > b OR NOT (a > c)`. La grafía `NOT
  <relational operator>` (`AND NOT < x`) es la forma de operador y no cambia, y
  un `NOT` que abre una condición ordinaria — `NOT (…)`, `NOT x = y`,
  `NOT x NUMERIC` — conserva su propio significado. *(Antes de 1.62.42 la forma
  de objeto se leía como "el objeto es distinto de cero", que da la misma
  respuesta solo cuando el objeto resulta contener cero.)*
- ✅ **Un nombre de condición declarado sobre un grupo prueba los bytes del
  grupo.** Un grupo no posee almacenamiento propio — *es* sus hijos —, así que
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compara contra los
  seis caracteres que contiene el registro.
- ✅ **Una constante figurativa se repite hasta el tamaño del otro operando**, y
  eso incluye la escrita como `VALUE` de un 88: `88 B VALUE QUOTE` sobre un
  anfitrión `PIC X(4)` son cuatro comillas, y `88 D VALUE ALL "BAC"` es
  `"BACB"`. `ALL literal` se dimensiona en **ambas** direcciones — `IF X EQUAL
  TO ALL "BA"` sobre una `X` de diez caracteres compara contra `"BABABABABA"`,
  no contra `"BA"` rellenado con espacios.

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
  (Las conversiones de fecha usan la base estándar 1601‑01‑01 = día 1.) El
  **conjunto completo de intrínsecas del estándar COBOL‑85** está implementado.
- ✅ **Los registros de fecha y hora leen el reloj LOCAL.** `ACCEPT … FROM DATE /
  TIME / DAY / DAY-OF-WEEK` y `FUNCTION CURRENT-DATE` informan todos de la hora
  propia de la máquina, no de UTC — incluida la fecha, que difiere a uno y otro
  lado de la medianoche. Los últimos cinco caracteres de `CURRENT-DATE` llevan el
  desfase **real** respecto a GMT (`…-0300`), de modo que un programa puede saber
  en qué zona se está ejecutando.
  ⚠️ Cualquier nombre de `FUNCTION` no reconocido se analiza igualmente pero
  devuelve **0** en ejecución.
- ✅ Literales: entero, decimal, cadena, todas las constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Una constante figurativa llena su receptor entero**, incluido
  `HIGH-VALUE`: `MOVE HIGH-VALUE TO <PIC X(10)>` son diez bytes `0xFF`, y hacia
  un grupo se reparte entre los hijos. Un receptor alfanumérico editado sigue
  colocando sus caracteres de inserción, así que `PIC XX0XXBXXX` contiene
  `FF FF '0' FF FF ' ' FF FF FF`. Bajo una `PROGRAM COLLATING SEQUENCE` la
  constante nombra un carácter ordinario y es ese carácter el que rellena.
  ⚠️ `HIGH-VALUE` es el **byte** `0xFF`, no un carácter. La lectura de un
  operando de grupo, la edición y todas las rutas de movimiento lo transportan
  byte a byte, pero **la modificación por referencia todavía no es exacta a
  nivel de byte**: `IF X (1:1) = HIGH-VALUE` es falso para un elemento que
  realmente contiene `0xFF`.
- ✅ **Un literal numérico puede empezar por el punto decimal**: `.5`, `-.5`,
  `.000000001`. COBOL‑85 sólo exige que un literal no *termine* con uno, así que
  `5.` sigue siendo el número 5 seguido de un terminador de sentencia.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Los ceros a la izquierda son significativos y exactos: `.000000001` es una
  milmillonésima, no una décima. Bajo `DECIMAL-POINT IS COMMA` lo mismo se
  aplica a `,5`.
  Lo que separa el literal de un punto de fin de sentencia es la **ausencia de
  espacio**: COBOL‑85 exige uno tras un terminador, así que `MOVE X TO Y.` nunca
  se lee como el comienzo de una fracción, y `MOVE X TO Y.5` es un error de
  compilación en lugar de una reinterpretación silenciosa.
- ✅ **Marcado de conformidad** (`cobolt_semantic::flagging`) — el estándar pide
  que una implementación conforme sea capaz de indicarle a un programa cuáles de
  las características que usa quedan fuera de un nivel de conformidad elegido.
  Dos análisis responden a eso:
  - `flag_obsolete` — el conjunto de **elementos obsoletos** de COBOL‑85: los
    cinco párrafos opcionales de la IDENTIFICATION DIVISION, `MEMORY SIZE`,
    `ALTER`, `STOP` con un literal y `GO TO` sin nombre de procedimiento.
  - `flag_high_subset` — todo lo que está por encima del **subconjunto alto**,
    desde `COMPUTE`, `EVALUATE` e `INITIALIZE` pasando por `CORRESPONDING`, la
    modificación por referencia, la cualificación, `SET … TO TRUE` y un cuarto
    subíndice, hasta la continuación de una *palabra* o de un *literal numérico*
    a través del límite de la tarjeta. (Continuar un literal **alfanumérico**
    está dentro del subconjunto y no se reporta.)

  Ninguno de los dos es comprobación de errores, y ninguno se ejecuta en una
  compilación ordinaria: cada construcción que nombran es COBOL‑85 válido que
  RustCOBOL implementa y ejecuta. Son puntos de entrada separados precisamente
  para que una compilación normal nunca empiece a advertir sobre `AUTHOR` ni
  sobre `COMPUTE`. Los NIST `NC302M`, `NC303M` y `NC401M` los validan: 7, 4 y 40
  marcas, todas coincidentes.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — el carácter que llena
  una posición de moneda en un PICTURE editado. **Sustituye** a `$` en lugar de
  sumarse a él, así que en cuanto un programa declara uno, `$` deja de ser un
  carácter de picture ahí:
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` se lee entonces `      <.00`, y `MOVE 1234` se lee
  ` <1,234.00`: la serie flotante se comporta exactamente igual que
  `$$$,$$$.99`. Un símbolo de moneda que sea una **letra** funciona del mismo
  modo: `CURRENCY SIGN IS "W"` convierte `PICTURE WWWWW` en una cadena de moneda
  flotante de cinco posiciones, de manera que `MOVE 12` se lee `  W12`. *(Antes
  de 1.62.40 una serie de un símbolo de letra se leía como una sola palabra y se
  rechazaba, así que sólo `$` flotaba.)* El
  literal debe tener un solo carácter, y COBOL‑85 prohíbe uno que chocaría con
  un carácter de picture o con un separador: ni un dígito, ni uno de
  `A B C D E G N P R S V X Z`, ni ninguno de `space * + - , . ; ( ) " / =`.
- ✅ **Literales hexadecimales** — `X"09"`, `x'0D0A'` (cualquier caja, cualquier
  comilla). Un carácter por **par** de dígitos hexadecimales, así que la cuenta
  de dígitos debe ser par; una cuenta impar o un dígito no hexadecimal es un
  literal mal formado y se reporta, en lugar de releerse calladamente como la
  palabra `X` junto a una cadena. Usables allí donde valga un literal
  entrecomillado (`DELIMITED BY`, `MOVE`, `VALUE`, comparaciones).

---

## Cláusulas de la DATA DIVISION (sintaxis de declaración aceptada)

- ✅ Niveles `01`–`49`, `77`, `88`; `FILLER`; grupo/elemental. La palabra
  `FILLER` es **opcional** — `05 PIC X VALUE ":".` declara uno igual que lo hace
  `05 FILLER PIC X VALUE ":".`, y en cualquiera de los dos casos ocupa sus bytes
  y guarda su `VALUE` dentro del grupo que lo contiene.
- ✅ `PIC/PICTURE` con `X A 9 S V P` y símbolos de edición
  (`Z * $ + - CR DB B 0 / , .`). El símbolo de moneda es `$` salvo que
  `SPECIAL-NAMES. CURRENCY` haya nombrado otro — véase **Expresiones,
  literales, USAGE** más arriba. **`P` es una posición de escala decimal** — una
  posición de dígito que el elemento abarca pero no almacena: `PIC S999PP`
  guarda tres dígitos que representan centenas (`MOVE 12300` lo almacena
  exactamente; `MOVE 12345` almacena 12300), y `PIC PP99` guarda dos que
  representan diezmilésimas. Las posiciones que ocupan las `P` se leen siempre
  como cero y no ocupan **ningún byte** en el diseño de un registro.
- ✅ **La protección con asteriscos rellena el elemento entero.** Un valor cero
  en una PICTURE cuyas posiciones de dígito son todas `*` rellena con asteriscos
  todas las posiciones de carácter — los decimales, las comas de agrupación, un
  `$` fijo y un `CR` o `DB` final por igual — y deja únicamente el propio punto
  decimal: `PIC $**.**CR` con cero se lee `***.****`, y `PIC *,***.**` se lee
  `*****.**`. Un valor **distinto** de cero protege solo los ceros a la
  izquierda, así que el `$` fijo conserva su propia posición
  (`-2.34` → `$*2.34CR`). *(Antes de 1.62.37 `CR`/`DB` aportaban un único
  asterisco en lugar de las dos posiciones de carácter que ocupan, de modo que
  un elemento así volvía un carácter más corto que su propio ancho.)*
- ✅ **Un literal numérico mueve sus caracteres, tal como está escrito.** A un
  receptor alfanumérico un literal aporta los dígitos que el programa escribió,
  justificados a la izquierda y rellenados con espacios —
  `MOVE 2 TO <PIC X(4)>` da `"2   "`, y
  `MOVE 060820000200 TO <six PIC 99 children>` los llena como
  `06 08 20 00 02 00`. El ancho del **receptor** nunca rellena el literal; solo
  lo hace el ancho con el que fue escrito. *(Antes de 1.62.38 el lexer
  conservaba solo el valor, así que se perdía un cero a la izquierda y cada
  carácter siguiente se desplazaba un lugar hacia la izquierda.)*
- ✅ **Una relación entre un operando numérico y uno no numérico es no
  numérica** (COBOL‑85 VI‑89 6.15.4 GR2). El operando numérico se trata como si
  se hubiera movido a un elemento alfanumérico de **su propio tamaño**, lo que
  transfiere sus posiciones de carácter y **no su signo operacional**: un
  `PIC S9(18)` que contiene `-123456789012345678` compara como **igual** a un
  `PIC X(18)` que contiene `"123456789012345678"`. Tres condiciones acotan la
  regla — el operando numérico debe ser un **entero**; lo «no numérico» lo
  decide la **declaración**, así que un hijo `PIC 99` que contenga caracteres
  tras un `MOVE` de grupo sigue siendo numérico — y un **grupo** es no numérico
  sean cuales sean sus hijos, de modo que un `PIC 9(5)` con 12345 frente a un
  grupo de diez bytes que contiene `"0000012345"` es `"12345     "` y desigual;
  y `ALL literal` toma el tamaño del otro operando. *(Antes de 1.62.38 la
  comparación era algebraica siempre que el lado de texto resultaba
  interpretable como número.)*
- ✅ **Truncamiento de orden superior en un MOVE numérico.** Un receptor guarda
  exactamente los dígitos que declaró por ambos extremos:
  `01 M PIC 99V999.  MOVE 123.45 TO M.` deja `23.450`. La aritmética comprueba
  primero la capacidad del receptor, así que una sentencia con `ON SIZE ERROR`
  conserva en cambio su valor anterior.
- ✅ **Una tabla de grupos se direcciona por ocurrencia.** `MOVE VALUES-1 TO
  GRP-1 (2)` reparte el valor entre los hijos propios de esa ocurrencia
  (`ELEM1 (2,1) … ELEM1 (2,4)`), y leer `GRP-1 (2)` concatena exactamente esos.
  El registro `01` que la envuelve son los bytes de **todas** las ocurrencias,
  así que `MOVE GRP-TAB1 TO GRP-TAB2` copia una tabla entera.
- ✅ **Los nombres de índice, los literales y la indexación relativa se mezclan
  como subíndices.** `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — un
  signo pegado a sus dígitos es un literal con signo que abre el siguiente
  subíndice — y `ELEM1 (IN1 - 1, 3)`, donde el operador lleva espacio a ambos
  lados, es indexación relativa.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (y `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/con signo/alfanumérico/figurativo/`ALL`). **`VALUE ALL
  "literal"` repite su unidad por todo el elemento** — `PIC X(6) VALUE ALL
  "ABC"` es `"ABCABC"` y `PIC X(9) VALUE ALL "XY"` es `"XYXYXYXYX"`.
  *(Antes de 1.62.40 solo las constantes figurativas de un carácter llenaban su
  elemento y `ALL "literal"` lo dejaba con espacios.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — una segunda lectura **viva** de los mismos bytes. No añade
  almacenamiento (por lo que no ensancha el grupo que lo contiene), y una
  escritura hecha por cualquiera de las dos descripciones es visible desde la
  otra: `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` y luego se lee de vuelta por `RESULT-A`.
  ⚠️ **Salvedad:** un solapamiento de más de 256 posiciones de almacenamiento
  expandidas (una tabla 10×10×10 redefinida, por ejemplo) mantiene
  almacenamiento por descripción — refrescarlo en cada escritura recorrería mil
  ocurrencias dos veces.
- ✅ **Los solapamientos se anidan.** Un `REDEFINES` dentro de un registro que a
  su vez está redefinido se alcanza en ambos sentidos, por profundo que sea:
  escribir dos bytes a través de una redefinición de nivel 01 alcanza el
  registro redefinido, el `REDEFINES` de un grupo que hay dentro de él y el
  `REDEFINES` de un elemento que hay dentro de *ese* — incluido un 88 declarado
  sobre el más interno. Cada descripción se vuelve a representar una vez por
  escritura. *(Antes de 1.62.42 una clave que pertenecía a más de un
  solapamiento conservaba solo la declarada en último lugar, y una única guarda
  detenía la cadena tras su primer salto.)*
- ✅ **Una descripción sin nombre sigue siendo una descripción.** `02 FILLER
  REDEFINES <item>.` vuelve a describir los bytes de su objetivo sin nombre
  propio, y una escritura en el objetivo es visible a través de sus hijos.
  Varios hijos se reparten esos bytes entre ellos, en orden de disposición — el
  solapamiento *no* es un alias de su primer hijo. Dos `FILLER REDEFINES` del
  mismo elemento son dos lecturas independientes, y cada una arranca en el
  **primer** byte del objetivo. *(Antes de 1.62.36 a un grupo redefinidor sin
  nombre no se le daba ninguna clave de almacenamiento, así que sus hijos se
  leían como espacios por más relleno que tuviera el objetivo.)*
- ✅ **Un nombre duplicado dentro de un solapamiento** resuelve al mismo
  almacenamiento que alcanza el resto del programa: `TAB-A` declarado bajo dos
  grupos distintos mantiene una lectura por declaración. *(Antes de 1.62.36 la
  copia inicial del solapamiento se indexaba con una ruta a la que le faltaban
  sus calificadores exteriores, algo que solo un nombre duplicado permite
  distinguir — así que justo el caso que necesita el calificador lo perdía.)*
- ✅ `JUSTIFIED [RIGHT]` — **almacena alineado a la derecha**, en un elemento
  *alfanumérico* o *alfabético*. Un emisor más estrecho que el receptor se
  rellena por la izquierda; un emisor más ancho que él conserva su extremo
  **derecho** y pierde sus caracteres más a la izquierda — lo contrario de la
  regla ordinaria. *(Antes de 1.62.40 la cláusula solo se registraba para
  elementos alfanuméricos, así que `PICTURE A(5) JUSTIFIED RIGHT` se analizaba y
  luego se alineaba a la izquierda como cualquier otro elemento.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — aceptadas;
  `SIGN … SEPARATE` todavía no cambia cómo se almacena el elemento.
- ✅ **Un `REDEFINES` en el nivel 01 puede describir más almacenamiento que el
  elemento al que redefine**, y los bytes que van más allá del final de ese
  elemento pertenecen a la descripción que sea lo bastante larga para
  nombrarlos. Escribir a través de una descripción más corta deja intacta la
  cola de la más larga.
- ✅ **Un solapamiento `REDEFINES` arrastra los bytes del elemento redefinido**,
  incluso hacia un par numérico: un solapamiento `PIC S9(18)` de un `X(18)` que
  contiene `"00ABCDEFGHI  4321 "` lee de vuelta esos caracteres, e
  `IS NUMERIC` responde que **no** para ellos. Cuando los bytes sí deletrean
  dígitos, la lectura numérica no cambia.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **nombres de condición
  reales**: el nivel 88 se liga a su elemento anfitrión; la prueba comprueba el
  anfitrión contra los VALUE / rangos, y `SET 88-name TO TRUE` guarda en el
  anfitrión un valor que la satisfaga.
- ✅ **Un nombre de condición puede declararse bajo más de un grupo, y
  `OF`/`IN` los distingue** — exactamente igual que para un nombre de dato, y
  pueden omitirse los niveles intermedios:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  El subíndice pertenece al elemento anfitrión, así que selecciona contra qué
  ocurrencia se prueban los VALUE. Una referencia **sin calificar** a un nombre
  de condición duplicado es ambigua en COBOL‑85; el runtime toma la primera
  declaración, la misma regla que aplica a un nombre de dato ambiguo.
- ✅ `USAGE INDEX` declara un registro índice entero (`SET`/`SEARCH` lo usan);
  `USAGE POINTER` — véase **Punteros** más arriba.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — un alias de
  reagrupación; leer concatena los elementos cubiertos, escribir reparte según
  el ancho de cada campo.
  - ✅ **Un 66 se califica por el registro que reagrupa**, exactamente como un
    elemento de datos se califica por el grupo que tiene encima, así que el
    mismo nombre 66 puede declararse una vez por registro y distinguirse con
    `OF`/`IN`: `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Esto funciona
    igual en lecturas y en escrituras, y un 66 gana frente a un elemento de
    datos ordinario que resulte compartir su nombre. Los operandos de la
    cláusula `RENAMES` se resuelven en ese mismo registro, así que un `NAME-2`
    duplicado nombra el de este registro.
  - ✅ **Una tabla cubierta aporta todas sus ocurrencias**, no solo la primera:
    `66 R RENAMES ITEM-1 THRU TABLE-2`, donde `TABLE-2` contiene
    `03 T PIC XXX OCCURS 5`, tiene 20 caracteres de ancho.
  - ✅ **Un 66 sobre exactamente un elemento *es* ese elemento** — la misma
    PICTURE, la misma categoría, el mismo almacenamiento. `66 R RENAMES W`,
    donde `W` es `PIC 9(4)`, es un elemento numérico de cuatro dígitos, así que
    `ADD 3500 TO R` con 8000 dentro provoca `ON SIZE ERROR` y lo deja sin
    cambios.
- Secciones: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` se
  analiza pero no se ejecuta.

---

## Todavía NO soportado — lista de evitación actual

> **Corregido el 2026‑08‑25.** Esta sección empezaba antes con "El conjunto de
> verbos y cláusulas de COBOL‑85 está **cubierto por completo**." Ejecutar la
> suite NIST CCVS85 lo desmintió: **102 de los 434 programas dentro del alcance
> fallaron aquel día**, con construcciones que este documento no listaba como
> carencias — comas y puntos y coma separadores, `FUNCTION x(ALL)`,
> `CLOSE … WITH LOCK`, `COPY` en el Área B, entradas de comentario de
> IDENTIFICATION, números de prioridad de sección, nombres de datos que empiezan
> por un dígito y — hasta 1.62.10 — literales numéricos con un punto decimal
> inicial. Para eso sirve una suite de validación. Cada carencia está ahora
> especificada en [`specs/nist/`](../specs/nist/README.md) y se sigue en el
> [marcador](#-la-conformidad-se-mide-no-se-afirma--nist-ccvs85) de más arriba.

La lista de abajo es lo que queda fuera del alcance **de forma intencionada**, a
diferencia de las carencias NIST de arriba, que son defectos en curso de
resolución:

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
   no‑operación para los objetos COBOL (sólo gobierna objetos de GUI/runtime).
4. ⚠️ Organización de ficheros **RELATIVE** (SEQUENTIAL / LINE SEQUENTIAL /
   INDEXED están hechas). **Esta es una trampa, no una carencia limpia:**
   `ORGANIZATION IS RELATIVE` *se analiza*, y nada en el runtime despacha nunca
   sobre ella, así que un programa RELATIVE compila y luego se comporta mal sin
   ningún diagnóstico. 30 de los 35 programas del módulo RL de NIST están
   exactamente en ese estado. Trátala como no implementada.
   Especificación:
   [organización RELATIVE](../specs/nist/NIST-spec-relative-organization.md).
5. Los nombres de función intrínseca no reconocidos siguen devolviendo **0** — el
   mismo modo de fallo silencioso. Especificación:
   [intrínsecas](../specs/nist/NIST-spec-intrinsic-function-gaps.md).
6. ⚠️ **Un valor inválido de `ACCESS MODE` / `ORGANIZATION` se traga sin
   diagnóstico** — otra vez la misma trampa, y esta la dispara una errata
   corriente del usuario. `ACCESS MODE IS` sólo acepta `SEQUENTIAL`, `RANDOM` o
   `DYNAMIC` (`INDEXED` es una *organización*, no un modo de acceso), pero el
   analizador de la cláusula SELECT comprueba esos tres y deja que cualquier otra
   cosa caiga en la rama genérica de "saltar un token desconocido", así que el
   fichero conserva calladamente el `SEQUENTIAL` por defecto y se comporta mal en
   ejecución en lugar de no compilar. `ORGANIZATION IS` tiene la forma idéntica.
   Ambos deberían levantar un error claro de tiempo de compilación que nombre la
   palabra ofensiva. **No es un problema del Núcleo** — ningún programa NC lleva
   una cláusula `ACCESS MODE`; la cláusula aparece sólo en los módulos DB, IC,
   IX, OBSQ, RL, RW, SQ y ST, así que bajo la REGLA DE ORO n.º 9 esto espera a
   que NC esté terminado.
7. ⚠️ **`ALPHABET … IS EBCDIC` se acepta pero deja en vigor el orden nativo
   (ASCII).** La frase literal (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` y `STANDARD‑2` están todos implementados y gobiernan de verdad
   `PROGRAM COLLATING SEQUENCE`; sólo falta la tabla EBCDIC, y nombrarla da
   calladamente el orden ASCII. La misma familia de trampas que 4–6.
8. **El módulo de Comunicaciones y el Report Writer** — véase
   [N/A más arriba](#-na--qué-queda-fuera-del-alcance-de-rustcobol-y-por-qué).

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

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.0 -->

# Referencia de la sintaxis soportada de RustCOBOL‑85

**Para qué sirve este documento:** para decir cuánto del estándar COBOL‑85
implementa realmente RustCOBOL — y para demostrarlo frente a la **suite oficial
de validación NIST COBOL‑85** en lugar de limitarse a afirmarlo. El
[marcador](#-la-conformidad-se-mide-no-se-afirma--nist-ccvs85) de más abajo es
el titular; todo lo que viene después es el detalle que hay tras esa cifra.

**Verdad de campo sobre lo que el lexer/parser/runtime de RustCOBOL aceptan hoy
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

## Índice

1. [★ La conformidad se mide, no se afirma — NIST CCVS85](#-la-conformidad-se-mide-no-se-afirma--nist-ccvs85)
2. [Párrafos de la IDENTIFICATION DIVISION](#párrafos-de-la-identification-division)
3. [Formatos de fuente](#formatos-de-fuente)
4. [Sentencias reconocidas (verbos)](#sentencias-reconocidas-verbos)
5. [Formas soportadas por verbo](#formas-soportadas-por-verbo)
6. [Condiciones (IF / EVALUATE / PERFORM UNTIL)](#condiciones-if--evaluate--perform-until)
7. [Expresiones, literales, USAGE](#expresiones-literales-usage)
8. [Cláusulas de la DATA DIVISION (sintaxis de declaración aceptada)](#cláusulas-de-la-data-division-sintaxis-de-declaración-aceptada)
9. [Todavía NO soportado — lista de evitación actual](#todavía-no-soportado--lista-de-evitación-actual)

---

## ★ La conformidad se mide, no se afirma — NIST CCVS85

**Este es el sentido del documento.** Cada afirmación de más abajo se comprueba
contra la **suite oficial de validación NIST COBOL‑85** — CCVS85 versión 4.0
(01 OCT 1992, COBOL 85 versión 4.2, SSVG de abril de 1993), la suite que el
National Institute of Standards and Technology de los Estados Unidos usaba para
certificar compiladores COBOL. Ocupa 28 MB, 348.271 líneas, **459 programas
COBOL** y 51 miembros de copybook, y reside en este repositorio en
`NIST/newcob.val,cbl`.

Es la fuente de verdad. Cuando RustCOBOL y CCVS85 discrepan, **CCVS85 tiene razón
y RustCOBOL se equivoca**.

El libro de registro legible por máquina es
[`NIST/progress.json`](../NIST/progress.json) — versionado, actualizado tras cada
cambio verificado. Las cifras de abajo se toman de él en lugar de reescribirse a
mano.

### El marcador

Medido el **2026‑08‑31 en 1.62.132**, sobre la distribución intacta. El censo de
compilación se cerró en 1.62.129.

| Eje | Resultado | Significado |
|---|---:|---|
| **Compilación** | **420 / 420** | el front end acepta todos los programas dentro del alcance. FAIL 0. |
| **Ejecución** | **380 / 380** | todos los programas puntuados se ejecutan y reportan **cero fallos** en su propio informe CCVS. |
| **Aserciones** | **8.362 PASS / 0 FAIL** | las comprobaciones que esos programas hacen sobre sí mismos. |

Reproduce cualquiera de los dos ejes:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### Los dos ejes nunca se confunden

La compilación es la afirmación estrictamente más débil: dice que el front end
acepta todas las construcciones de un programa, no que el programa calcule la
respuesta correcta. La suite se puntúa a sí misma — cada programa CCVS85 imprime
su propio recuento `PASS` / `FAIL*` — de modo que el eje de ejecución es el que
significa «funciona». Ambos se reportan por módulo más abajo, con sus propios
denominadores, y ninguno se cita nunca como si fuera el otro.

La ilustración más clara está en la propia historia de este repositorio: 30 de
los 35 programas de ficheros RELATIVE compilaban limpiamente mientras el runtime
**no tenía motor RELATIVE en absoluto**. Se ejecutaban y producían resultados
erróneos en silencio. El motor llegó en 1.62.76 y el módulo se terminó en
1.62.77.

#### Por módulo

La compilación y la ejecución llevan denominadores distintos, por dos razones
declaradas. Los miembros `*301M` prueban el *marcado de subconjunto intermedio*
de características que RustCOBOL implementa como estándar, lo cual es
inalcanzable por diseño y queda excluido de la ejecución por decisión del
operador (IX301M, RL301M, ST301M, SM301M); siguen contando en el censo de
compilación, donde pasan. Y la mayoría de los miembros IC son **llamados** —
subprogramas sin informe propio — así que solo se puntúan los programas
llamadores.

| Módulo | Qué prueba | Compilación | Ejecución | Aserciones | Estado |
|---|---|---:|---:|---:|---|
| **NC** | Núcleo | **95 / 95** | **95 / 95** | 4.614 | ✅ terminado |
| **SQ** | E/S secuencial | **85 / 85** | **85 / 85** | 624 | ✅ terminado |
| **IX** | E/S indexada | **42 / 42** | **41 / 41** | 574 | ✅ terminado |
| **IF** | Funciones intrínsecas | **45 / 45** | **45 / 45** | 841 | ✅ terminado |
| **IC** | Comunicación entre programas | **47 / 47** | **25 / 25** | 309 | ✅ terminado |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ terminado |
| **SM** | Manipulación del texto fuente | **17 / 17** | **16 / 16** | 311 | ✅ terminado |
| **RL** | E/S relativa | **35 / 35** | **34 / 34** | 354 | ✅ terminado |
| **DB** | Depuración | **14 / 14** | — | — | solo eje de compilación (abajo) |
| **En alcance** | | **420 / 420** | **380 / 380** | **8.362** | |
| SG | Segmentación | 13 / 13 | — | — | ⬜ declarado fuera de alcance (abajo) |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB (Depuración)** se puntúa solo en compilación. Sus 14 programas se aceptan;
la semántica de *ejecución* del módulo de depuración no está implementada, y el
eje de ejecución para él no se ha declarado dentro de alcance. Se lista aquí en
lugar de ocultarse, para que la carencia siga visible.

#### El recuento DELETED — 24, y qué significa

`***** ****TEST DELETED****` es el propio marcador de CCVS para un caso que el
programa se saltó por sí mismo. **No** es un pase, y por eso se registra por
separado: el recuento cayó de 108 → 1 en 1.62.53 mientras el recuento de
programas limpios apenas se movía, lo cual era progreso real que una lectura
centrada solo en los fallos habría pasado por alto.

En los módulos terminados hay 24 casos DELETED: NC 5, SQ 6, IX 1, IC 4, SM 3,
RL 5. **Solo los 3 de SM están documentados como propios de la distribución** —
SM206A PST‑TEST‑008 y PST‑TEST‑11, y SM208A REP‑TEST‑7, se distribuyen
comentados, de modo que una ejecución conforme del fuente entregado reporta
exactamente esos tres. Los otros 21 están registrados pero aún no explicados
individualmente en el libro de registro; no los cites como intencionados.

### ⬜ N/A — qué queda fuera del alcance de RustCOBOL, y por qué

Estos módulos **no se cuentan como fallos** — 38 programas excluidos de toda
puntuación. El razonamiento completo está en
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Módulo | Programas | Por qué queda fuera de alcance |
|---|---:|---|
| **CM** — Comunicación | 9 | `COMMUNICATION SECTION`, entradas `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Apunta a los monitores de teleproceso de los años ochenta — colas de mensajes en poder de un gestor de transacciones. Aquí no existe tal runtime, y el módulo se retiró de los estándares COBOL posteriores. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entradas `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, cortes de control. Un sublenguaje declarativo extenso; la respuesta de PowerRustCOBOL a los informes es el Form Designer y la exportación a PDF. Podría convertirse más adelante en una *funcionalidad* si se quiere — es la única exclusión con valor real para el usuario. |
| **SG** — Segmentación | 13 | Decisión del operador, 2026‑08‑29. La segmentación existe para encajar un programa en una máquina demasiado pequeña para contenerlo: los encabezados `SECTION` llevan un número de segmento y el runtime superpone segmentos independientes unos sobre otros. RustCOBOL es un runtime de 64 bits con más espacio de direcciones del que cualquier programa COBOL puede agotar, así que un número de segmento **compila y no tiene ningún efecto**. No hay comportamiento que el módulo pueda medir. Sus 13 programas siguen compilando, y se reportan como N‑A en lugar de eliminados para que la exclusión siga visible. |
| **OBSQ / OBIC / OBNC** | 9 | Vuelven a probar módulos anteriores y esperan que el compilador *marque* elementos obsoletos de COBOL‑85. Su contenido de lenguaje está cubierto por las especificaciones dentro de alcance; lo que queda fuera de alcance es el **marcado** de características obsoletas. |
| **EXEC85** | 1 | No es una prueba. Es el propio ejecutivo COBOL de NIST que divide la distribución y dirige la suite — aquí reemplazado por un arnés en Rust, así que no necesita compilar. |

El **COBOL orientado a objetos** también queda fuera del alcance de RustCOBOL,
pero CCVS85 es anterior por completo — no hay programas OO en la suite.

### Qué queda pendiente

En los módulos puntuados, nada: ambos ejes están cerrados y no falla ninguna
aserción. Lo que queda no es una lista de defectos sino tres decisiones
permanentes — el eje de ejecución de DB, SG y los miembros de marcado `*301M` —
cada una registrada arriba con su motivo, más los 21 casos DELETED que no se han
explicado individualmente.

El arnés imprime el detalle del fallo que hay detrás de cualquier regresión,
listo para agrupar por módulo:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> Una línea de detalle `FAIL*` se escribe **dos veces** a propósito — el
> `PRINT-DETAIL` de CCVS ejecuta `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE`
> — mientras que `PASS ` se escribe una sola vez. Cualquier recuento en bruto de
> marcadores tomado del fichero de impresión tiene que dividir los fallos por dos
> antes de significar algo.

### Historia de conformidad

Eje de compilación, contra el denominador dentro de alcance de cada momento. El
denominador mismo se movió cuando SG se declaró fuera de alcance y cuando DB205A
se repuntuó bajo CM, así que las filas iniciales son sobre 434 y la fila de
cierre sobre 420.

| Versión | Compilación | Qué cambió |
|---|---:|---|
| 1.62.7 | **0** / 434 | Nada compilaba. Faltaban dos reglas del formato de referencia clásico: las columnas 73‑80 se leían como fuente, y las líneas de continuación nunca se unían. |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — el formato de referencia clásico, incluida la continuación. Véase [Formatos de fuente](#formatos-de-fuente). |
| 1.62.13 | 292 / 434 | La coma y el punto y coma separadores son puntuación, no tokens; los subíndices pueden separarse solo con espacios; un delimitador duplicado dentro de un literal es un único carácter. Se vaciaron tres cubos de diagnóstico completos. |
| 1.62.14 | 317 / 434 | Una tabla entera como argumento intrínseco; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`. **Funciones intrínsecas 45 / 45 en compilación.** |
| 1.62.16 | 376 / 434 | El `AT` de `AT END` es opcional, de modo que una frase `END` suelta ya no se traga el siguiente encabezado de párrafo (33 programas). **E/S indexada 42 / 42 en compilación.** |
| 1.62.21 | 417 / 434 | El paso del Núcleo — serie `ALTER`, nombres‑condición con subíndice, relaciones combinadas abreviadas, categorías de `INSPECT` entre operandos. Núcleo de 76 → 92 compilando. |
| **1.62.42** | 420 / 434 | **Núcleo terminado en ambos ejes** — 95 / 95 compilando *y* ejecutando limpio, 4.614 aserciones sin ninguna que falle. |
| **1.62.43** | 422 / 434 | La E/S secuencial compila por completo, 85 / 85, y pasa de 10 → 44 de 85 en ejecución. Los párrafos de un declarativo conservan sus nombres, así que un manejador `USE` puede hacerles `PERFORM` y `GO TO` — 20 programas dejaron de caerse. |
| **1.62.47** | — | **E/S secuencial terminada** — 85 / 85 en ambos ejes. La última carencia era `XXXXD001`, un fichero de datos que suministra la *instalación* de CCVS85 y que ningún miembro escribe; ahora lo planta el arnés. |
| **1.62.76** | — | Llega el **motor RELATIVE** (`cobolt-runtime/src/relative.rs`, contenedor `PRCREL1`). Los siete verbos de fichero despachan sobre `FileOrganization::Relative`. |
| **1.62.77** | — | **E/S relativa terminada** (34 / 34) desde una línea base de 14 / 35 en una sola sesión, y **E/S indexada terminada** — el motor relativo cerró los últimos cuatro fallos de IX106A, que eran el fichero relativo que ejercita junto a los secuenciales e indexados. |
| **1.62.81** | — | **Funciones intrínsecas terminadas** en ejecución, 45 / 45, desde una línea base de 24 / 45. Cinco causas, ninguna en la materia propia del módulo: una regla de separadores del lexer dos veces, una carencia de marcado, la gramática del argumento de `NUMVAL`, una comparación de listas de argumentos y una ruta de desbordamiento en la división. |
| **1.62.107** | — | **Comunicación entre programas terminada**, 25 / 25. |
| **1.62.119** | — | **Sort / Merge terminado**, 39 / 39, 735 PASS. El paso final fue `[COLLATING] SEQUENCE [IS] alphabet-name` en SORT/MERGE, ordenando claves alfanuméricas por el alfabeto nombrado en `SPECIAL-NAMES`. |
| **1.62.127** | — | **Manipulación del texto fuente terminada**, 16 / 16. Los operandos literales de cadena conservan sus comillas; los operandos identificador abarcan su cadena `IN`/`OF` y su subíndice; los pares se aplican en una sola pasada sin reexplorar los reemplazos. |
| **1.62.129** | **420 / 420** | **El censo de compilación cierra al 100 %.** DB205A se puntúa bajo CM por decisión, lo que sitúa la suite dentro de alcance en 420. |

> **El resumen honesto.** Todos los programas dentro de alcance compilan, y todos
> los programas puntuados se ejecutan limpios: **420 / 420 en compilación,
> 380 / 380 en ejecución, 8.362 aserciones sin ninguna que falle.** Nueve
> lanzamientos antes de la primera de esas filas la cifra de compilación era
> cero. Lo que sigue abierto se declara arriba como decisiones en lugar de
> esconderse dentro de un porcentaje — el eje de ejecución de DB, SG, los
> miembros de marcado `*301M`, y 21 casos DELETED que no se han explicado
> individualmente.

---

> **Actualización (paso de implementación de carencias):** los siguientes se
> implementaron y ahora son ✅ — **modificación de referencia** `id(start:len)`,
> **`PERFORM n TIMES` en línea**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING
> `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` consciente de
> categorías**, **condiciones abreviadas con operador antepuesto**
> (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (se ejecuta en un CALL no
> resuelto), **múltiples receptores en `COMPUTE` + `ROUNDED` por receptor**, y un
> conjunto de **funciones intrínsecas** mucho mayor.
>
> **Actualización (paso de entorno jerárquico / consciente de ocurrencias —
> 1.5.0):** cuatro características bloqueadas por el modelo de datos son ahora
> ✅ — **subindexación de tablas en tiempo de ejecución** `t(i)` / `t(i, j)`
> (almacenamiento por ocurrencia), **desambiguación de nombres cualificados**
> `id OF/IN group` (los nombres hoja duplicados resuelven a almacenamiento
> independiente), **`MOVE/ADD/SUBTRACT CORRESPONDING`**, y **`SEARCH` /
> `SEARCH ALL` funcionales**.
>
> **Actualización (paso de completitud de verbos — 1.6.0):** ahora también ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` con múltiples receptores + `ROUNDED` por
> receptor** en `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** y el `EXIT` simple corregido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado y regiones
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de fecha/finanzas
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condiciones abreviadas con objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multisujeto) y **`WHEN NOT`**;
> **nombres‑condición de nivel 88 reales** (`SET … TO TRUE/FALSE`, el anfitrión
> se prueba contra sus VALUEs/rangos); **`PERFORM para VARYING`**; y un runtime
> de **`SORT`/`MERGE`** funcional (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). La lista de evitación del final está al día.
>
> **Actualización (paso de liquidación de la lista de evitación — 1.7.0):** las
> carencias restantes están ahora implementadas — **abreviación con objeto
> identificador** (`a = b OR c`, resuelta mediante metadatos de nivel 88);
> **`INITIALIZE … REPLACING category DATA BY value`**; **`66 RENAMES`** (la
> lectura sintetiza / la escritura distribuye entre los elementos cubiertos);
> **punteros** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> `SET ADDRESS OF item TO …` con alias, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; **`NEXT SENTENCE`** fiel; las **intrínsecas** estándar restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`); y
> **`ACCEPT`/`DISPLAY`** de pantalla extendidos (`AT`/`WITH` mediante ANSI en
> modo CLI — ahora *ejecutados*, no solo analizados).
>
> **Actualización (1.7.1):** las fuentes de registro de `ACCEPT` ahora son
> funcionales (eran no‑operaciones reconocidas) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (emparejados con
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (emparejado con
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Actualización (1.7.2):** frases de compartición / bloqueo de ficheros y
> `CANCEL` (eran ❌ / no‑operaciones) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libera los bloqueos de registro
> INDEXED del fichero), y **`CANCEL program`** (reinicializa el almacenamiento
> del programa).
>
> **Actualización (1.8.0):** **`COMMIT` / `ROLLBACK`** son ahora verbos COBOL
> reales — transacciones controladas por el programa sobre los ficheros INDEXED
> abiertos (tanto el motor de memoria como el de disco). El motor de disco ganó
> un registro de deshacer real durante la ejecución (antes era una
> no‑operación). La lista de evitación del final está al día.

---

## Párrafos de la IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Los párrafos de **entrada‑comentario** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — en **cualquier orden y cualquier
  subconjunto**.
- ✅ `REMARKS` también se acepta. Se eliminó de COBOL en 1985, así que no se
  almacena; se admite para que el código heredado de COBOL‑74 siga compilando.

Una **entrada‑comentario** es texto libre, y COBOL‑85 lo dice literalmente:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Puede contener **palabras reservadas** — el `DATA` de arriba no inicia una DATA
  DIVISION.
- Puede contener **puntos**, y no termina en uno.
- **Abarca tantas líneas** como escribas.
- Termina en el siguiente encabezado de párrafo o de división que **empiece una
  línea** en el Área A — que es como la entrada de arriba termina en
  `DATE-WRITTEN`.

**Una comilla en esa prosa queda contenida en su línea** (desde 1.62.12). Un
texto como `THE COMPILER"S ABILITY` ya no abre un literal que se prolongue por
el resto del programa — véase [Formatos de fuente](#formatos-de-fuente). Sigue
mereciendo la pena evitar una comilla sin pareja en una entrada‑comentario, pero
ahora te cuesta esa línea, no el fichero.

⚠️ `INSTALLATION`, `SECURITY` y `REMARKS` **no son palabras reservadas** aquí. Se
reconocen como nombres de párrafo solo dentro de la IDENTIFICATION DIVISION, de
modo que un dato llamado `SECURITY` sigue funcionando.

---

## Formatos de fuente

RustCOBOL lee tres disposiciones de fuente. La elección es explícita — **nunca**
se adivina a partir del contenido del fichero, porque aplicar reglas de columnas
a un fuente que no se escribió para ellas borra código en silencio.

| `--source-format` | Qué significa |
|---|---|
| `free` | Sin reglas de columnas en absoluto. `*>` inicia un comentario. **El valor por omisión**, y lo que usan los propios proyectos de PowerRustCOBOL y los ficheros `.cbl` de formulario generados. |
| `fixed` | ✅ **Formato de referencia clásico de COBOL-85** — la disposición que define el estándar y en la que se escribe el fuente de imagen de tarjeta. Véase más abajo. |
| `fixed-relaxed` | El área de secuencia y la columna indicadora se respetan, pero la línea llega tan lejos como la hayas escrito — sin límite de 72 columnas. |
| `auto` | Comportamiento histórico: `free`, salvo que `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` fija el valor por omisión de una sesión.

### `fixed` — el formato de referencia clásico

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Columnas 1-6** — área de número de secuencia, ignorada.
- **Columna 7** — área indicadora:
  - `*` o `/` → línea de comentario
  - `-` → **continuación** de la línea anterior
  - `D` → línea de depuración; un comentario (el modo de depuración aún no está
    implementado)
  - cualquier otra cosa → se lee como fuente ordinario. El estándar reserva esta
    columna, pero las suites de imagen de tarjeta la usan como selector de líneas
    opcionales, y descartar esas líneas en silencio borraría código.
- **Columnas 8-72** — el fuente.
- **Columnas 73-80** — área de identificación, **descartada**.

### Líneas de continuación ✅

Un guion en la columna 7 continúa la línea anterior.

**Continuar una palabra o un literal numérico** — los espacios finales de la
línea continuada se descartan y las dos mitades se juntan sin nada entre ellas:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

declara un único elemento llamado `WRK-DS-18V00-CONTINUED`.

**Continuar un literal alfanumérico** — el literal de la línea continuada no
tiene comilla de cierre; la línea de continuación debe reabrir con una, y el
literal se reanuda en el carácter siguiente:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **El fragmento continuado llega hasta la columna 72, espacios finales
incluidos.** Una línea que se queda corta de la columna 72 sigue aportando esos
espacios al literal. Por eso un literal continuado solo es exacto byte a byte
bajo `fixed`; los otros formatos no tienen columna 72 en la que parar.

### Un literal nunca abarca una línea por accidente ✅

La continuación es la **única** forma de que un literal cruce de línea. Una
comilla que no se cierra en su propia línea es un error, reportado donde está
escrita:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Esto importa más de lo que parece. Antes de 1.62.12 una comilla sin pareja
llegaba hasta la *siguiente* comilla en cualquier punto del fichero, de modo que
un solo `"` perdido en un comentario se tragaba divisiones enteras y desplazaba
el emparejamiento de todas las comillas posteriores — los programas de NIST donde
se encontró esto tienen un número **par** de comillas, así que nada quedaba sin
terminar; un único carácter había desplazado la paridad de todo el fichero. Ahora
el daño se detiene en el salto de línea.

> **El formato libre no tiene continuación de literales.** Ni `&` — ese es el
> *operador* de concatenación — ni un bloque delimitado. Un literal en formato
> libre debe caber en una línea; para uno largo, concatena:
> `"first part" & "second part"`.

> **Nota.** Elegir `fixed` para un fichero que se escribió en formato libre lo
> dañará — todo lo que pase de la columna 72 desaparece, y el texto anterior a la
> columna 8 se lee como número de secuencia. Pásalo solo para fuente que
> realmente sea imagen de tarjeta.

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
ficheros — orientativo dentro de una única unidad de ejecución)
✅ `COMMIT` / `ROLLBACK` (transacciones de ficheros INDEXED controladas por el
programa — véanse los verbos de fichero) · `CANCEL` (reinicializa el
almacenamiento del programa) ·
✅ `INVOKE` — maneja objetos de GUI/runtime (ventanas, formularios, métodos de
control); es una no‑operación solo para los objetos **COBOL**, ya que las
definiciones de clase/método quedan fuera de alcance
Extensiones del proyecto: `EXEC RUST … END-EXEC`,
`TRY/CATCH/FINALLY/END-TRY`, `THROW`. Un bloque puede hacer `use` de los crates
siempre enlazados (std, egui, eframe y el conjunto de runtime enlazado) **más
cualquier crate que el proyecto registre en Crates del Proyecto** (spec 044): los
crates registrados se fijan a una versión exacta, se copian al `crates/` del
proyecto y se compilan dentro del binario; los crates no registrados hacen
fallar Check/Build en la línea del desarrollador, nombrando el remedio.

✅ `SEARCH` (serie) / `SEARCH ALL` (búsqueda binaria sobre una tabla con
`ASCENDING`/`DESCENDING KEY` — ejecuta el primer `WHEN` que coincida, y si no
`AT END`).
✅ `SORT` / `MERGE` con `RELEASE` / `RETURN` (funcionales — véase más abajo).
✅ `DECLARATIVES … END DECLARATIVES` con `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — manejadores de error de fichero
disparados ante un `FILE STATUS` de error no atendido. Un manejador **se entra
por el principio de su sección y se ejecuta hasta el final de la sección**, y sus
párrafos conservan sus nombres, así que puede hacerles `PERFORM` y `GO TO` —
incluido un párrafo de *otra* sección declarativa. Los párrafos declarativos
viven en su propio espacio de nombres: el control nunca cae del cuerpo principal
dentro de ellos, y un nombre declarado en ambos resuelve a la copia del
declarativo mientras un manejador está en ejecución y a la del cuerpo en todo lo
demás. Un declarativo también puede hacer `PERFORM` de un párrafo de la porción
no declarativa.
❌ **No reconocidos — no los uses:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas soportadas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (múltiples receptores).
- ✅ **Un operando de grupo hace que todo el movimiento sea alfanumérico**
  (COBOL-85 6.18.4). La PICTURE del otro operando aporta su *tamaño* y nada más:
  sin edición, sin des‑edición, sin conversión numérica.
  `MOVE <grupo que contiene "123ABC">` deja `"123ABC "` en un `PIC 0XXXXX0` (no el
  editado `"0123AB0"`), los mismos seis caracteres y un espacio en un
  `PIC 9999V999`, y `"12"` en un `PIC 99`. `JUSTIFIED RIGHT` sigue decidiendo qué
  extremo rellena y qué extremo se pierde. La misma regla rige los propios bytes
  de un grupo: cada hijo toma su porción literalmente, así que un hijo
  alfanumérico‑editado **no** se vuelve a editar.
- ✅ **Una cláusula `VALUE` sobre un grupo** inicializa los bytes del grupo y se
  distribuye entre sus hijos — `01 G VALUE "$123.45". 02 E PIC $999.99.` deja a
  `E` con `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — mueve cada elemento subordinado que los dos
  grupos comparten por nombre, recurriendo por los subgrupos coincidentes.
- ✅ **`CORRESPONDING` excluye un elemento descrito con `REDEFINES` o `RENAMES`**
  (COBOL-85 6.18.4 GR1), en cualquiera de los dos lados, junto con todo lo
  subordinado a él. La exclusión está en la *declaración*, no en el nombre: un
  elemento normal que simplemente comparte su nombre con un nivel 66 en otro
  lugar sigue correspondiendo.
- ✅ **Cualquiera de los dos operandos de `CORRESPONDING` puede nombrar una
  ocurrencia de una tabla de grupos** — `MOVE CORRESPONDING C-LEVEL TO
  C-FLOCK (4)` escribe los propios huecos de esa ocurrencia, y el subíndice se
  arrastra por la recursión.
- ✅ **Un par necesita que solo UNO de sus dos elementos sea elemental.** Un grupo
  puede enfrentarse a un elemento elemental, y el movimiento entre ellos es
  alfanumérico: un `PIC XXX` elemental que envía a un grupo de `999` + `XXX`
  rellena sus seis caracteres, y un grupo de `XXX` + `99` que envía a un `X(5)`
  simple lo rellena. Dos grupos enfrentados siguen **recurriendo** — ese
  emparejamiento no es el caso elemental. *(Antes de 1.62.39 ninguna de las dos
  direcciones movía nada en absoluto: un grupo no posee hueco de almacenamiento,
  así que la escritura iba donde nada la vuelve a leer y la lectura daba la cadena
  vacía.)*
- ✅ **Modificación de referencia `id(start:len)`** — emisor (subcadena) y receptor
  (asignación parcial empalmada); funciona sobre los operandos de todos los
  verbos. `length` es opcional. Direcciona **posiciones de carácter**, así que un
  operando numérico se toma con todo su ancho de `PIC` y sus ceros a la izquierda:
  `01 T PIC 9(8) VALUE 00224845` da `T(1:2)` = `"00"`, no `"22"`.
- ✅ **Los elementos de grupo son agregados alfanuméricos** — un grupo *es* sus
  elementos subordinados puestos uno tras otro, y su tamaño es la suma de los de
  ellos. Leer uno concatena los hijos (incluido `FILLER`); mover a uno distribuye
  los bytes entre ellos por ancho. `MOVE 11 TO A` es visible a través del grupo
  que contiene a `A`, y `MOVE "1234" TO G` fija los hijos de `G`, no un hueco
  propio.
- ✅ subíndices `t(i)`, `t(i, j)` — leen/escriben el hueco de almacenamiento por
  ocurrencia; los subíndices variables `t(WS-I)` se evalúan en cada acceso.
- ✅ cualificación `id OF/IN group` (`… OF g1 OF g2`) — resuelve al elemento
  correcto incluso cuando el nombre hoja está declarado bajo más de un grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por receptor** — cada receptor lleva su propia marca `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico
  coincidente, recurriendo por los subgrupos coincidentes.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **múltiples receptores `GIVING`**, cada uno con su propio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sin `GIVING`) guarda `a/b` de vuelta en `a` (una comodidad
  de PowerRustCOBOL; el COBOL estándar exige aquí `INTO` o `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **múltiples receptores, cada uno con su propio `ROUNDED`**.
- ✅ operadores de expresión `+ - * /` y `**` (potencia, asociativa por la
  derecha), paréntesis, `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **multisujeto con `ALSO`** — cada columna `WHEN` se compara posicionalmente
  con su sujeto y se combina con AND.
- ✅ **`WHEN NOT value`** niega un objeto de selección; **`WHEN condition`**
  (p. ej. `EVALUATE TRUE WHEN a > b`) evalúa la condición booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal entero o dato).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en línea,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en línea (sin párrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — ejecuta el párrafo en
  cada iteración (fuera de línea, sin `END-PERFORM`).
- ✅ **`WITH TEST AFTER` se aplica a `VARYING`**, escrito a cualquiera de los dos
  lados de la frase y en línea o fuera de línea. El cuerpo se ejecuta una vez
  antes de probar nada, y las condiciones se prueban entonces **de dentro hacia
  fuera**; el nivel cuya condición es falsa se incrementa, cada nivel interior a
  él reinicia en su valor `FROM`, y el cuerpo se ejecuta de nuevo. Una variable se
  incrementa solo cuando su prueba resulta falsa, así que la prueba que termina el
  bucle la deja como la dejó el cuerpo.
- ✅ **Una variable `AFTER` se reinicia a su valor `FROM` cuando su bucle termina**,
  antes de incrementar el siguiente nivel hacia fuera (COBOL-85 6.20.4 GR10(d)).
  Tras el `PERFORM` completo, las variables interiores leen sus valores `FROM` y
  solo la más exterior conserva el valor que lo terminó.
- ✅ **Un identificador `VARYING` con subíndice sigue a su subíndice.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` incrementa
  la ocurrencia que `S1` seleccione en ese momento, así que un cuerpo que avanza
  `S1` recorre la tabla.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **El cualificador `{OF|IN} section` elige qué copia se quiere** cuando un
  nombre de párrafo se repite entre secciones, exactamente como en `PERFORM`. Una
  sección **desconocida** cae de vuelta a la búsqueda sin cualificar en lugar de
  perder el salto. `GO TO … DEPENDING ON` toma una lista simple de nombres y
  ningún cualificador, y un `GO TO` que un `ALTER` haya redirigido sigue la
  redirección — que nombra su propio destino sin ambigüedad. *(Antes de 1.62.39 el
  cualificador se analizaba y luego se ignoraba, así que el salto aterrizaba en la
  primera definición de cualquier punto del programa.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ el `EXIT` simple es un punto de retorno sin efecto; `EXIT PROGRAM` devuelve
  el control al llamador.
- ✅ `EXIT PERFORM [CYCLE]` (rompe / continúa el PERFORM en línea más cercano),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfiere el control más allá del siguiente límite de
  frase (el analizador inserta marcadores de límite en cada punto; fiel, no solo
  un `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lee del operador** cuando `SPECIAL-NAMES` declara el
  mnemónico (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — ese es el Formato 1, idéntico a un `ACCEPT id` simple.
  Un nombre que **ninguna cláusula `SPECIAL-NAMES` declara** conserva la extensión
  de PowerRustCOBOL y lee la **variable de entorno** de ese nombre. Cuál de las
  dos se aplica lo decide la declaración, nunca la grafía. *(Antes de 1.62.35 la
  cláusula ordinaria `<implementor-name> IS <mnemonic>` se omitía por completo,
  así que todos los mnemónicos leían una variable de entorno que nunca se fijaba y
  el elemento receptor quedaba vacío.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` posiciona el cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (toda la línea de órdenes) · `FROM ARGUMENT-NUMBER`
  (número de argumentos) · `FROM ARGUMENT-VALUE` (el argumento en el puntero
  fijado por `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (la variable nombrada por
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` cierra la sentencia (opcional).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` cierra la lista de operandos (opcional), así que
  `DISPLAY A END-DISPLAY DISPLAY B` son dos sentencias en lugar de una.
- ✅ formas de pantalla `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — ejecutadas mediante
  posicionamiento de cursor ANSI + SGR en **modo CLI** (`rcrun`); ignoradas en
  modo GUI (allí el Form Designer sustituye a la E/S de SCREEN).
  `ACCEPT id AT …` posiciona y luego lee.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Desbordamiento = la cadena ensamblada es más ancha que el campo receptor.
- ✅ **Una frase `DELIMITED BY` rige toda la serie de emisores que la precede**,
  no solo aquel tras el que se escribe:
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` delimita los tres y construye
  `"ABC"`. Una sentencia puede llevar varias frases, cada una rigiendo los
  emisores desde la anterior; los emisores posteriores a la última frase toman
  cada uno por completo. *(Antes de 1.62.40 solo se delimitaba el emisor escrito
  inmediatamente antes de la frase.)*
- ✅ **`INTO` un elemento de grupo** distribuye entre los elementos subordinados
  del grupo.
- ✅ **El resultado se ensambla byte a byte**, así que `STRING HIGH-VALUE` mueve el
  único byte `0xFF` y ocupa una posición de carácter.
- ✅ **Extensión — `DELIMITED BY` inteligente por omisión** (cuando ninguna frase
  rige un operando): los elementos alfanuméricos `PIC X`/`A` toman `SPACES` por
  omisión (se descarta el relleno final); los literales de cadena, los elementos
  numéricos, los numéricos‑editados, los resultados de `FUNCTION` y las
  expresiones toman `SIZE`. Los datos se mueven en su forma de campo (numérico →
  dígitos con todo el ancho de PIC; numérico‑editado → caracteres editados).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Desbordamiento = más campos de origen
  que receptores.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **se aplican ambas mitades**.
- ✅ `BEFORE/AFTER INITIAL` confina cada frase a una subregión del campo.
  (TALLYING acumula sobre el contador, según COBOL.)
- ✅ **Una serie de operandos TALLYING comparte UN ÚNICO recorrido de izquierda a
  derecha** (COBOL-85 6.17.3). En cada posición de carácter los operandos se
  prueban en el orden en que se escribieron; el primero que coincide toma la
  posición y el recorrido se reanuda más allá de los caracteres que consumió. Así
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` sobre `"AABA"` da `t1 = 1, t2 = 1` —
  escribir los operandos al revés da `t1 = 3, t2 = 0`. `LEADING` debe coincidir
  desde el borde izquierdo de su ventana sin hueco, así que un operando anterior
  que tome esa posición termina la racha antes de que empiece, y `CHARACTERS`
  cuenta solo las posiciones que ningún operando anterior reclamó.
- ✅ **Una serie de operandos REPLACING comparte UN ÚNICO recorrido también**, por
  la misma regla: el primer operando que coincide en una posición reemplaza esos
  caracteres y el recorrido se reanuda más allá de ellos, así que ningún operando
  posterior puede verlos. La ventana `BEFORE`/`AFTER` de cada operando se fija
  **antes de cualquier reemplazo**, que es lo que permite anclar un operando en
  caracteres que otro anterior sobrescribe:

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Aplicado de un operando a la vez, la primera frase borraría el `"L "` en el que
  está anclada la segunda, y `"BAD"` sobreviviría.
- ✅ **Un elemento DISPLAY con signo no tiene un `-` entre sus posiciones de
  carácter.** El signo operacional es una sobreperforación sobre un dígito, así
  que `INSPECT <PIC S9(5) que contiene -12345> TALLYING c FOR ALL "-"` da **0**
  mientras que `FOR ALL "5"` da 1. El signo se restaura después, así que un
  `REPLACING` sobre los dígitos lo deja en paz. `SIGN IS … SEPARATE CHARACTER` es
  el caso en que el signo *sí* es una posición, y se cuenta.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado a MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` fija el elemento anfitrión al primer VALUE de la
  condición; `TO FALSE` fija un valor fuera del conjunto de VALUE (con el mejor
  esfuerzo — no hay cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` y
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — véase **Punteros** más
  abajo.

### INITIALIZE
- ✅ `INITIALIZE id …` — consciente de categorías: numérico / numérico‑editado →
  ZERO, todo lo demás → SPACES, recurriendo por los elementos de grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — fija cada elemento
  subordinado de esa categoría al valor; los demás quedan intactos.

### Punteros (USAGE POINTER)
- ✅ `USAGE POINTER` declara un puntero (NULL inicialmente).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — pone a `id` como alias
  del almacenamiento del destino (las lecturas **y** las escrituras siguen el
  alias); típicamente un registro de LINKAGE. `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ El cuerpo de `ON EXCEPTION` / `ON OVERFLOW` se ejecuta cuando el programa
  llamado no se resuelve; el cuerpo de `NOT ON EXCEPTION` se ejecuta cuando la
  llamada **sí se resuelve**.
- ✅ `CANCEL program …` reinicializa la WORKING-STORAGE del programa nombrado, de
  modo que su siguiente `CALL` empieza de cero.

### Verbos de fichero (las frases soportadas — la cobertura completa está en la suite de E/S de ficheros)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` se analizan y se respetan donde tienen sentido —
  orientativos en el modelo de una única unidad de ejecución.)
- ✅ **Un solo `OPEN` puede llevar varios grupos de modo**, cada uno con sus
  propios ficheros: `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Cada grupo se abre
  en su propio modo; `SHARING` / `WITH LOCK` / `REGISTERED USER` se aplican a la
  sentencia.
- ✅ **Un `OPEN` de un fichero que ya está abierto es `41`**, y el fichero se deja
  como estaba — la sentencia **no** lo vuelve a abrir. (Reabrir un fichero
  `OUTPUT` truncaría en silencio lo que el programa ya hubiera escrito.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensión de
  PowerRustCOBOL) — registra al operador/usuario en el registro de
  observabilidad de INDEXED (campo `user=` en cada línea de evento de la sesión de
  ese fichero). Puramente observacional; sin autenticación/autorización. Véase
  [`observability-es.md`](observability-es.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libera el bloqueo de registro que el motor INDEXED toma bajo I‑O.
- ✅ **`READ … INTO id` es el `READ` seguido de un `MOVE` de grupo.** El registro
  se distribuye entre los elementos subordinados del receptor por ancho y se corta
  al ancho propio del receptor, el receptor puede llevar subíndice, y el
  movimiento transporta bytes — un registro que contiene un byte que no es un
  carácter llega intacto.
- ✅ **Cláusula `RECORD` de la FD — registros de longitud variable.** Las tres
  grafías: `RECORD CONTAINS n CHARACTERS` (fija),
  `RECORD CONTAINS n TO m CHARACTERS` (variable; la descripción de registro que
  nombra el `WRITE` da la longitud), y
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (el dato *es* la longitud — se fija antes de un `WRITE`, lo repone un `READ`, y
  se acota al rango declarado). Una FD cuyos registros `01` difieren en tamaño es
  de longitud variable lo diga o no. Un fichero de longitud variable guarda la
  longitud de cada registro junto al registro, así que sus bytes **no** son
  intercambiables con los de un fichero de longitud fija; un fichero de longitud
  fija no cambia.
- ✅ **Los registros `01` de una FD describen una única área de registro.** Un
  `READ` entrega los bytes a través de todas las descripciones de registro; un
  `WRITE` envía el área completa, así que lo que otra descripción de registro
  puso donde la escrita tiene `FILLER` se transparenta.
- ✅ **`FILLER` ocupa sus bytes en un registro de FD**, y
  `SIGN IS SEPARATE CHARACTER` hace que un elemento DISPLAY con signo sea un
  carácter más ancho que sus posiciones de dígito.
- ✅ **El `LINAGE` de una FD admite nombres de datos además de enteros** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. La página se
  mide a partir de esos elementos en cada `WRITE`, así que un programa puede
  redimensionarla mientras se ejecuta. `LINAGE-COUNTER` vale uno cuando el fichero
  se abre.
- ✅ **Un `READ` secuencial después de `AT END` es `46`, no un segundo `10`.** El
  `AT END` no dejó un siguiente registro válido, así que seguir leyendo es un error
  distinto de llegar al final. `46` es un estado de clase 4, así que ni `AT END` ni
  `NOT AT END` se ejecutan para él — el declarativo `USE` del fichero es lo que lo
  atiende. Un `OPEN` nuevo, o un `START` con éxito, vuelve a establecer un
  registro.
- ✅ `UNLOCK f [RECORD[S]]` libera los bloqueos de registro del fichero.
- ✅ **`COMMIT` / `ROLLBACK`** — transacciones controladas por el programa sobre
  **todos** los ficheros INDEXED abiertos. `OPEN` inicia una transacción; `COMMIT`
  confirma los `WRITE`/`REWRITE`/`DELETE` pendientes (un `ROLLBACK` posterior ya no
  puede deshacerlos) e inicia una nueva; `ROLLBACK` deshace todos los cambios desde
  el último `COMMIT`/`OPEN`. El almacenamiento **DISK** hace que `COMMIT`/`CLOSE`
  sean durables en disco. El almacenamiento **MEMORY** mantiene `COMMIT`/`ROLLBACK`
  puramente en RAM (nunca escribe a disco); un fichero `STORAGE IS MEMORY` simple
  es efímero, y `STORAGE IS MEMORY WITH PERSISTENCE` guarda a disco solo en el
  `CLOSE`. (La recuperación ante caídas mediante un registro de escritura
  anticipada durable es trabajo futuro — esto es reversión a nivel de programa,
  dentro de la ejecución.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (ficheros INDEXED; extensión de PowerRustCOBOL). El
  almacenamiento por omisión es `DISK`. `WITH COMPRESSION` comprime el registro
  almacenado (las claves se evalúan sobre el registro sin comprimir);
  `WITH PERSISTENCE` (solo MEMORY) guarda el fichero en RAM al hacer `CLOSE`.
  `OPEN OUTPUT` siempre (re)crea el contenedor en disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` sobre un fichero SEQUENTIAL de registros** reemplaza el registro
  que entregó el último `READ`, en su sitio, y deja la posición de lectura donde
  estaba — el siguiente `READ` sigue dando el registro que va después. Los estados
  que debe: **`49`** cuando el fichero no está abierto en `I-O`, **`43`** cuando
  ningún `READ` con éxito estableció un registro (incluido después de `AT END`, y
  en un segundo `REWRITE` sin `READ` entre medias), y **`44`** cuando el nuevo
  registro no tiene la misma longitud que el leído — en un fichero con
  `DEPENDING ON` el valor del elemento es esa longitud, que es como un programa
  pide otra.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ La compartición de ficheros entre *procesos* no se impone (una única unidad
  de ejecución); las frases `SHARING`/`LOCK` se analizan y los bloqueos de registro
  por ejecución del motor INDEXED se respetan.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcionales, búfer de trabajo en memoria)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (en un INPUT PROCEDURE) añade a la ejecución;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devuelve los registros.
- Los registros se ordenan de forma estable por las claves declaradas
  (`ASCENDING`/`DESCENDING`); `USING` lee / `GIVING` escribe los ficheros
  secuenciales nombrados.

---

## Condiciones (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionales: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relaciones con palabras: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Clase: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Un elemento cuya PICTURE **no lleva signo operacional** es `NUMERIC` solo cuando
  todas sus posiciones de carácter contienen un dígito — un `PIC X(5)` que contiene
  `"+1234"`, `"1.234"` o `"12 45"` **no** es numérico. *(Antes de 1.62.40 la prueba
  analizaba los caracteres como un número, así que se aceptaban un signo, un punto
  decimal, un exponente y los espacios circundantes.)*
- ✅ **Un operando de `CLASS` definido por el usuario puede ser una posición
  ordinal** — `CLASS ORDINAL-A-ONLY IS 66` nombra el carácter 66.º del juego
  nativo — y el operando puede ir en su propia línea de fuente. Lo mismo vale para
  `ALPHABET`.
- ✅ Signo: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nombre‑condición de nivel 88 (el nombre suelto como condición).
- ✅ **`TRUE` / `FALSE` como operandos** (extensión de PowerRustCOBOL) — azúcar
  para `1` y `0`, donde sea que se permita un valor: `IF x = TRUE`,
  `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (la forma con `NOT` suelto, sin operador
  relacional), `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`,
  `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE`, y `WHEN TRUE` contra un
  sujeto de valor. Un `TRUE`/`FALSE` suelto también es una condición completa
  (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ Esto **no** cambia los dos lugares en los que esas palabras ya significaban
  algo: `SET <88‑name> TO TRUE` sigue fijando el elemento anfitrión a un valor que
  satisface la condición (no al número 1), y `EVALUATE TRUE`/`EVALUATE FALSE` de
  más abajo siguen siendo la sentencia de casos estándar.
- ✅ `AND` / `OR` / `NOT` combinados, paréntesis (AND liga más fuerte que OR).
- ✅ **Condiciones abreviadas con operador antepuesto** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (se reutiliza el sujeto de comparación precedente).
- ✅ **Abreviación con objeto literal** — `a = 1 OR 2 OR 3` (reutiliza tanto el
  sujeto como el operador; el objeto es un literal).
- ✅ **Abreviación con objeto identificador** — `a = b OR c` (donde `c` es un
  dato). Un identificador suelto tras AND/OR después de una comparación se
  resuelve en tiempo de ejecución: un nombre‑condición de nivel 88 conocido se
  evalúa como tal, y si no es el objeto `a = c`. (Un identificador seguido
  inmediatamente de `AND` conserva la precedencia de AND.)
- ✅ **Un `NOT` delante del *objeto* de una abreviación niega la relación**, no el
  objeto: `a > b OR NOT c` es `a > b OR NOT (a > c)`. La grafía
  `NOT <operador relacional>` (`AND NOT < x`) es la forma de operador y no cambia,
  y un `NOT` que abre una condición ordinaria — `NOT (…)`, `NOT x = y`,
  `NOT x NUMERIC` — conserva su propio significado. *(Antes de 1.62.42 la forma de
  objeto se leía como «el objeto es distinto de cero», lo que da la misma respuesta
  solo cuando el objeto resulta contener cero.)*
- ✅ **Un nombre‑condición declarado sobre un grupo prueba los bytes del grupo.**
  Un grupo no posee almacenamiento propio — *es* sus hijos — así que
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compara contra los
  seis caracteres que contiene el registro.
- ✅ **Una constante figurativa se repite hasta el tamaño del otro operando**, y
  eso incluye una escrita como el `VALUE` de un 88: `88 B VALUE QUOTE` sobre un
  anfitrión `PIC X(4)` son cuatro comillas, y `88 D VALUE ALL "BAC"` es `"BACB"`.
  `ALL literal` se dimensiona en **ambas** direcciones — `IF X EQUAL TO ALL "BA"`
  sobre una `X` de diez caracteres compara contra `"BABABABABA"`, no contra `"BA"`
  rellenado con espacios.

---

## Expresiones, literales, USAGE

- ✅ Operadores aritméticos `+ - * /` y `**`; paréntesis; `+`/`-` unarios.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — intrínsecas **implementadas**:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (con semilla opcional), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Las conversiones de fecha usan la base estándar 1601‑01‑01 = día 1.) El
  **conjunto completo de intrínsecas estándar de COBOL‑85** está implementado.
- ✅ **Los registros de fecha y hora leen el reloj LOCAL.** `ACCEPT … FROM DATE /
  TIME / DAY / DAY-OF-WEEK` y `FUNCTION CURRENT-DATE` reportan todos la hora
  propia de la máquina, no UTC — incluida la fecha, que difiere a un lado y otro
  de la medianoche. Los últimos cinco caracteres de `CURRENT-DATE` llevan el
  desplazamiento **real** respecto a GMT (`…-0300`), así que un programa puede
  saber en qué zona se está ejecutando.
  ✅ Un nombre de `FUNCTION` no reconocido es un **error de compilación** que
  nombra la función, con una sugerencia cuando una real se parece lo bastante como
  para ser un error de tecleo probable. Antes se analizaba y devolvía **0** en
  tiempo de ejecución, lo que convertía una falta de ortografía en una respuesta
  equivocada con aplomo (1.62.15).
- ✅ Literales: entero, decimal, cadena, todas las constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Una constante figurativa rellena todo su receptor**, incluida
  `HIGH-VALUE` — `MOVE HIGH-VALUE TO <PIC X(10)>` son diez bytes `0xFF`, y hacia
  un grupo se distribuye entre los hijos. Un receptor alfanumérico‑editado sigue
  colocando sus caracteres de inserción, así que un `PIC XX0XXBXXX` contiene
  `FF FF '0' FF FF ' ' FF FF FF`. Bajo una `PROGRAM COLLATING SEQUENCE` la
  constante nombra un carácter ordinario y es ese carácter el que rellena.
  ⚠️ `HIGH-VALUE` es el **byte** `0xFF`, no un carácter. La lectura de un operando
  de grupo, la edición y todas las rutas de movimiento lo transportan byte a byte,
  pero **la modificación de referencia aún no es exacta a nivel de byte** —
  `IF X (1:1) = HIGH-VALUE` es falso para un elemento que genuinamente contiene
  `0xFF`.
- ✅ **Un literal numérico puede empezar por el punto decimal** — `.5`, `-.5`,
  `.000000001`. COBOL‑85 solo exige que un literal no *termine* en uno, así que
  `5.` sigue siendo el número 5 seguido de un terminador de frase.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Los ceros a la izquierda son significativos y exactos: `.000000001` es una
  milmillonésima, no una décima. Bajo `DECIMAL-POINT IS COMMA` lo mismo vale para
  `,5`. Lo que separa el literal de un punto de fin de frase es la **ausencia de
  un espacio** — COBOL‑85 exige uno después de un terminador, así que
  `MOVE X TO Y.` nunca se lee como el inicio de una fracción, y `MOVE X TO Y.5` es
  un error de compilación en lugar de una reinterpretación silenciosa.
- ✅ **Marcado de conformidad** (`cobolt_semantic::flagging`) — el estándar pide
  que una implementación conforme sea capaz de decirle a un programa cuáles de las
  características que usa quedan fuera de un nivel de conformidad elegido. Dos
  análisis responden a eso:
  - `flag_obsolete` — el conjunto de **elementos obsoletos** de COBOL‑85: los cinco
    párrafos opcionales de la IDENTIFICATION DIVISION, `MEMORY SIZE`, `ALTER`,
    `STOP` con un literal, y `GO TO` sin nombre de procedimiento.
  - `flag_high_subset` — todo lo que está por encima del **subconjunto alto**,
    desde `COMPUTE`, `EVALUATE` e `INITIALIZE` pasando por `CORRESPONDING`, la
    modificación de referencia, la cualificación, `SET … TO TRUE` y un cuarto
    subíndice, hasta continuar una *palabra* o un *literal numérico* a través del
    límite de una tarjeta. (Continuar un literal **alfanumérico** está dentro del
    subconjunto y no se reporta.)

  Ninguno de los dos es comprobación de errores, y ninguno se ejecuta en una
  compilación ordinaria: cada construcción que nombran es COBOL‑85 válido que
  RustCOBOL implementa y ejecuta. Son puntos de entrada separados precisamente
  para que una compilación normal nunca empiece a advertir sobre `AUTHOR` o sobre
  `COMPUTE`. Los NIST `NC302M`, `NC303M` y `NC401M` los validan — 7, 4 y 40
  marcas, todas coincidentes.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — el carácter que rellena
  una posición de moneda en una PICTURE editada. **Sustituye** a `$` en lugar de
  sumarse a él, así que en cuanto un programa declara uno, `$` ya no es un carácter
  de picture ahí:
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` se lee entonces como `      <.00`, y `MOVE 1234` se lee
  como ` <1,234.00` — la serie flotante se comporta exactamente como lo hace
  `$$$,$$$.99`. Un símbolo de moneda que sea una **letra** funciona igual:
  `CURRENCY SIGN IS "W"` hace de `PICTURE WWWWW` una cadena de moneda flotante de
  cinco posiciones, así que `MOVE 12` se lee como `  W12`. *(Antes de 1.62.40 una
  serie de un símbolo de letra se leía como una sola palabra y se rechazaba, así
  que solo `$` flotaba.)* El literal debe ser de un carácter, y COBOL‑85 prohíbe
  uno que colisionaría con un carácter de picture o un separador: no un dígito, no
  uno de `A B C D E G N P R S V X Z`, y ninguno de `space * + - , . ; ( ) " / =`.
- ✅ **Literales hexadecimales** — `X"09"`, `x'0D0A'` (en cualquier caja, con
  cualquier comilla). Un carácter por **par** de dígitos hexadecimales, así que el
  número de dígitos debe ser par; un número impar o un dígito no hexadecimal es un
  literal mal formado y se reporta, no se vuelve a leer calladamente como la
  palabra `X` junto a una cadena. Utilizables donde sea que lo sea un literal
  entre comillas (`DELIMITED BY`, `MOVE`, `VALUE`, comparaciones).

---

## Cláusulas de la DATA DIVISION (sintaxis de declaración aceptada)

- ✅ Niveles `01`–`49`, `77`, `88`; `FILLER`; grupo/elemental. La palabra `FILLER`
  es **opcional** — `05 PIC X VALUE ":".` declara uno igual que
  `05 FILLER PIC X VALUE ":".`, y de cualquiera de las dos formas contiene sus
  bytes y su `VALUE` dentro del grupo que lo contiene.
- ✅ `PIC/PICTURE` con `X A 9 S V P` y símbolos de edición (`Z * $ + - CR DB B 0 /
  , .`). El símbolo de moneda es `$` salvo que `SPECIAL-NAMES. CURRENCY` nombrara
  otro — véase **Expresiones, literales, USAGE** más arriba. **`P` es una posición
  de escalado decimal** — una posición de dígito que el elemento abarca pero no
  almacena: `PIC S999PP` contiene tres dígitos que representan centenas
  (`MOVE 12300` lo almacena exactamente; `MOVE 12345` almacena 12300), y
  `PIC PP99` contiene dos que representan diezmilésimas. Las posiciones que ocupan
  las `P` se leen siempre como cero y no ocupan **ningún byte** en la disposición
  de un registro.
- ✅ **La protección con asteriscos rellena todo el elemento.** Un valor cero en
  una picture cuyas posiciones de dígito son todas `*` rellena todas las posiciones
  de carácter con asteriscos — los dígitos fraccionarios, las comas de agrupación,
  un `$` fijo, y un `CR` o `DB` final por igual — dejando solo el propio punto
  decimal: un `PIC $**.**CR` que contiene cero se lee `***.****`, y un
  `PIC *,***.**` se lee `*****.**`. Un valor **distinto** de cero protege solo los
  ceros a la izquierda, así que el `$` fijo conserva su propia posición
  (`-2.34` → `$*2.34CR`). *(Antes de 1.62.37 `CR`/`DB` aportaba un asterisco en
  lugar de las dos posiciones de carácter que ocupa, así que tal elemento volvía
  un carácter más corto que su propio ancho.)*
- ✅ **Un literal numérico mueve sus caracteres, tal como está escrito.** Hacia un
  receptor alfanumérico un literal aporta los dígitos que el programa tecleó,
  justificados a la izquierda y rellenados con espacios — `MOVE 2 TO <PIC X(4)>`
  es `"2   "`, y `MOVE 060820000200 TO <seis hijos PIC 99>` los rellena
  `06 08 20 00 02 00`. El ancho del **receptor** nunca rellena el literal; solo lo
  hace su propio ancho escrito. *(Antes de 1.62.38 el lexer conservaba solo el
  valor, así que un cero a la izquierda se perdía y todos los caracteres
  siguientes se desplazaban un lugar a la izquierda.)*
- ✅ **Una relación entre un operando numérico y uno no numérico es no numérica**
  (COBOL‑85 VI‑89 6.15.4 GR2). El operando numérico se trata como si se hubiera
  movido a un elemento alfanumérico de **su propio tamaño**, lo que transfiere sus
  posiciones de carácter y **no su signo operacional**: un `PIC S9(18)` que
  contiene `-123456789012345678` compara como **igual** a un `PIC X(18)` que
  contiene `"123456789012345678"`. Tres condiciones acotan la regla — el operando
  numérico debe ser un **entero**; «no numérico» lo decide la **declaración**, así
  que un hijo `PIC 99` que contiene caracteres tras un `MOVE` de grupo sigue siendo
  numérico — y un **grupo** es no numérico sean como sean sus hijos, así que un
  `PIC 9(5)` que contiene 12345 frente a un grupo de diez bytes que contiene
  `"0000012345"` es `"12345     "` y desigual; y `ALL literal` toma el tamaño del
  otro operando. *(Antes de 1.62.38 la comparación era algebraica siempre que el
  lado de texto resultara analizable como número.)*
- ✅ **Truncamiento por la izquierda en un MOVE numérico.** Un receptor contiene
  exactamente sus dígitos declarados por ambos extremos:
  `01 M PIC 99V999.  MOVE 123.45 TO M.` deja `23.450`. La aritmética prueba
  primero la capacidad del receptor, así que una sentencia con `ON SIZE ERROR`
  conserva en su lugar el valor antiguo.
- ✅ **Una tabla de grupos se direcciona por ocurrencia.** `MOVE VALUES-1 TO
  GRP-1 (2)` distribuye entre los hijos propios de esa ocurrencia
  (`ELEM1 (2,1) … ELEM1 (2,4)`), y leer `GRP-1 (2)` concatena exactamente esos. El
  registro `01` que los engloba son los bytes de **todas** las ocurrencias, así que
  `MOVE GRP-TAB1 TO GRP-TAB2` copia una tabla completa.
- ✅ **Los nombres de índice, los literales y la indexación relativa se mezclan
  como subíndices.** `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — un
  signo pegado a sus dígitos es un literal con signo que abre el siguiente
  subíndice — y `ELEM1 (IN1 - 1, 3)`, donde el operador lleva espacios a ambos
  lados, es indexación relativa.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (y `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/con signo/alfanumérico/figurativo/`ALL`). **`VALUE ALL
  "literal"` repite su unidad por todo el elemento** — `PIC X(6) VALUE ALL "ABC"`
  es `"ABCABC"` y `PIC X(9) VALUE ALL "XY"` es `"XYXYXYXYX"`. *(Antes de 1.62.40
  solo las constantes figurativas de un carácter rellenaban su elemento y
  `ALL "literal"` lo dejaba conteniendo espacios.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — una segunda lectura **viva** de los mismos bytes. No añade
  almacenamiento (así que no ensancha el grupo que lo contiene), y una escritura a
  través de cualquiera de las dos descripciones es visible a través de la otra:
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` se vuelve a leer luego a través de `RESULT-A`.
  ⚠️ **Salvedad:** una superposición mayor de 256 huecos de almacenamiento
  expandidos (una tabla 10×10×10 redefinida, por ejemplo) conserva en su lugar
  almacenamiento por descripción — refrescarla en cada escritura recorrería mil
  ocurrencias dos veces.
- ✅ **Las superposiciones se anidan.** Un `REDEFINES` dentro de un registro que a
  su vez está redefinido se alcanza en ambas direcciones, por profundo que sea:
  escribir dos bytes a través de una redefinición de nivel 01 alcanza el registro
  redefinido, el `REDEFINES` de un grupo dentro de él, y el `REDEFINES` de un
  elemento dentro de *ese* — incluido un 88 declarado sobre el más interno. Cada
  descripción se vuelve a materializar una vez por escritura. *(Antes de 1.62.42
  una clave que pertenecía a más de una superposición conservaba solo la declarada
  en último lugar, y un único guardián detenía la cadena tras su primer salto.)*
- ✅ **Una descripción sin nombre sigue siendo una descripción.**
  `02 FILLER REDEFINES <item>.` vuelve a describir los bytes de su objetivo bajo
  ningún nombre propio, y una escritura al objetivo es visible a través de sus
  hijos. Varios hijos reparten esos bytes entre ellos, en orden de disposición —
  la superposición *no* es un alias de su primer hijo. Dos `FILLER REDEFINES` de un
  mismo elemento son dos lecturas independientes, cada una empezando en el
  **primer** byte del objetivo. *(Antes de 1.62.36 a un grupo redefinidor sin
  nombre no se le daba ninguna clave de almacenamiento, así que sus hijos se leían
  como espacios por mucho que se hubiera rellenado el objetivo.)*
- ✅ **Un nombre duplicado dentro de una superposición** resuelve al mismo
  almacenamiento que alcanza el resto del programa: `TAB-A` declarado bajo dos
  grupos distintos conserva una lectura por declaración. *(Antes de 1.62.36 la
  copia inicial de la superposición se indexaba desde una ruta a la que le faltaban
  sus cualificadores exteriores, lo cual solo un nombre duplicado puede distinguir
  — así que precisamente el caso que necesita el cualificador lo perdía.)*
- ✅ `JUSTIFIED [RIGHT]` — **almacena alineado a la derecha**, sobre un elemento
  *alfanumérico* o *alfabético*. Un emisor más estrecho que el receptor se rellena
  por la izquierda; un emisor más ancho conserva su extremo **derecho**, perdiendo
  sus caracteres más a la izquierda — lo contrario de la regla ordinaria. *(Antes
  de 1.62.40 la cláusula se registraba solo para elementos alfanuméricos, así que
  `PICTURE A(5) JUSTIFIED RIGHT` se analizaba y luego se alineaba a la izquierda
  como cualquier otro elemento.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — aceptados;
  `SIGN … SEPARATE` todavía no cambia cómo se almacena el elemento.
- ✅ **Un `REDEFINES` en el nivel 01 puede describir más almacenamiento que el
  elemento que redefine**, y los bytes más allá del final de ese elemento
  pertenecen a la descripción que sea lo bastante larga para nombrarlos. Escribir a
  través de una descripción más corta deja intacta la cola de la más larga.
- ✅ **Una superposición `REDEFINES` transporta los bytes del elemento
  redefinido**, incluso hacia un par numérico: una superposición `PIC S9(18)` de
  una `X(18)` que contiene `"00ABCDEFGHI  4321 "` vuelve a leer esos caracteres, e
  `IS NUMERIC` responde **no** para ellos. Cuando los bytes sí forman dígitos la
  lectura numérica no cambia.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **nombres‑condición reales**: el
  nivel 88 se liga a su elemento anfitrión; la prueba comprueba el anfitrión contra
  los VALUE / rangos, y `SET 88-name TO TRUE` guarda un valor que los satisface en
  el anfitrión.
- ✅ **Un nombre‑condición puede declararse bajo más de un grupo, y `OF`/`IN` los
  distingue** — exactamente como para un nombre de dato, y se pueden omitir niveles
  intermedios:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  El subíndice pertenece al elemento anfitrión, así que selecciona contra qué
  ocurrencia se prueban los VALUE. Una referencia **sin cualificar** a un
  nombre‑condición duplicado es ambigua en COBOL‑85; el runtime toma la primera
  declaración, la misma regla que aplica a un nombre de dato ambiguo.
- ✅ `USAGE INDEX` declara un registro índice entero (`SET`/`SEARCH` lo usan);
  `USAGE POINTER` — véase **Punteros** más arriba.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — un alias de reagrupación; la
  lectura concatena los elementos cubiertos, la escritura distribuye por ancho de
  campo.
  - ✅ **Un 66 se cualifica por el registro que reagrupa**, exactamente como un dato
    se cualifica por el grupo que está por encima, así que el mismo nombre de 66
    puede declararse una vez por registro y distinguirse con `OF`/`IN`:
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Esto funciona igual en
    lecturas y en escrituras, y un 66 gana frente a un dato ordinario que resulte
    compartir su nombre. Los operandos de la cláusula `RENAMES` resuelven en ese
    mismo registro, así que un `NAME-2` duplicado nombra el de este registro.
  - ✅ **Una tabla cubierta aporta todas las ocurrencias**, no solo la primera:
    `66 R RENAMES ITEM-1 THRU TABLE-2`, donde `TABLE-2` contiene
    `03 T PIC XXX OCCURS 5`, tiene 20 caracteres de ancho.
  - ✅ **Un 66 sobre exactamente un elemento *es* ese elemento** — misma PICTURE,
    misma categoría, mismo almacenamiento. `66 R RENAMES W` donde `W` es `PIC 9(4)`
    es un elemento numérico de cuatro dígitos, así que `ADD 3500 TO R` con 8000
    dentro provoca `ON SIZE ERROR` y lo deja sin cambiar.
- Secciones: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` se
  analiza pero no se ejecuta.

---

## Todavía NO soportado — lista de evitación actual

> **Corregido el 2026‑08‑25.** Esta sección empezaba diciendo «El conjunto de
> verbos / cláusulas de COBOL‑85 está **cubierto por completo**». Ejecutar la suite
> NIST CCVS85 lo desmintió: **102 de los 434 programas dentro de alcance fallaron
> aquel día**, sobre construcciones que este documento no listaba como carencias —
> comas y puntos y comas separadores, `FUNCTION x(ALL)`, `CLOSE … WITH LOCK`,
> `COPY` en el Área B, entradas de comentario de IDENTIFICATION, números de
> prioridad de sección, nombres de datos que empiezan por dígito y — hasta
> 1.62.10 — literales numéricos con punto decimal inicial. Para eso sirve una
> suite de validación. Cada carencia está ahora especificada en
> [`specs/nist/`](../specs/nist/README.md) y seguida en el
> [marcador](#-la-conformidad-se-mide-no-se-afirma--nist-ccvs85) de más arriba.

La lista de abajo es lo que queda fuera de alcance **por intención**, en
contraposición a las carencias de NIST de arriba, que son defectos en curso de
resolución:

1. **Edición de entrada en `ACCEPT` de pantalla** — `DISPLAY … AT/WITH` y
   `ACCEPT … AT` se ejecutan (ANSI) en modo CLI, pero la edición completa de la
   SCREEN SECTION a nivel de campo (tabulación automática, validación de campos,
   mapas de color) queda **sustituida por el diseñador de formularios** en modo
   GUI.
2. **Compartición de ficheros entre *procesos*** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK` y `UNLOCK` se analizan y manejan los bloqueos de
   registro por ejecución del motor INDEXED, pero los bloqueos no se imponen entre
   procesos distintos del sistema operativo (modelo de una única unidad de
   ejecución).
3. **COBOL orientado a objetos** (definiciones de clase/método) — `INVOKE` es una
   no‑operación para los objetos COBOL (maneja únicamente objetos de GUI/runtime).
4. ✅ **Resuelto (1.62.15).** Un nombre de función intrínseca no reconocido
   devolvía **0** en silencio, así que un programa calculaba con aplomo una
   respuesta equivocada a partir de un error de tecleo. Ahora es un **error de
   compilación** que nombra la función y sugiere la real más cercana cuando hay una
   coincidencia lo bastante próxima (`cobolt-semantic/src/resolver.rs`,
   `Expr::FunctionCall`). Se mantiene aquí porque la forma del «cero silencioso» es
   la trampa que los puntos 5 y 6 siguen llevando.
5. ⚠️ **Un valor inválido de `ACCESS MODE` / `ORGANIZATION` se traga sin
   diagnóstico** — la misma trampa otra vez, y a esta la dispara un error de tecleo
   ordinario del usuario. `ACCESS MODE IS` acepta solo `SEQUENTIAL`, `RANDOM` o
   `DYNAMIC` (`INDEXED` es una *organización*, no un modo de acceso), pero el
   analizador de la cláusula SELECT prueba esos tres y deja que cualquier otra cosa
   caiga en la rama genérica de «saltar un token desconocido», así que el fichero
   conserva en silencio el `SEQUENTIAL` por omisión y se comporta mal en tiempo de
   ejecución en lugar de no compilar. `ORGANIZATION IS` tiene la forma idéntica
   (`cobolt-parser/src/parser.rs`, la rama `Token::Access` y la rama de
   organización por encima de ella). Ambas deberían levantar un error claro en
   tiempo de compilación nombrando la palabra ofensora. **Ningún módulo de NIST
   detectará esto nunca** — la suite escribe solo cláusulas válidas, así que todos
   los módulos pueden terminar al 100 % con la carencia aún abierta. Es una trampa
   de error de tecleo del usuario, y necesita una prueba propia en lugar de una
   puntuación de módulo.
6. ⚠️ **`ALPHABET … IS EBCDIC` se acepta pero deja en vigor el orden nativo
   (ASCII).** La frase literal (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` y `STANDARD‑2` están todas implementadas y manejan de verdad
   `PROGRAM COLLATING SEQUENCE`; solo falta la tabla EBCDIC, y nombrarla da
   calladamente el orden ASCII. Misma familia de trampas que 4–6.
7. **El módulo de Comunicación y el Report Writer** — véase
   [N/A más arriba](#-na--qué-queda-fuera-del-alcance-de-rustcobol-y-por-qué).

> **Resuelto (1.5.0):** el modelo de datos plano pasó a ser jerárquico /
> consciente de ocurrencias, desbloqueando **CORRESPONDING**, los **nombres
> cualificados**, la **subindexación de tablas** y **`SEARCH`**.
> **Resuelto (1.6.0):** `MULTIPLY`/`DIVIDE` con múltiples receptores + `ROUNDED`
> por receptor; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`;
> `INSPECT TALLYING REPLACING` combinado + `BEFORE/AFTER INITIAL`; intrínsecas de
> fecha/`ANNUITY`; abreviación con objeto literal; `EVALUATE ALSO`/`WHEN NOT`;
> nombres‑condición de nivel 88 reales; `PERFORM para VARYING`; y el runtime de
> `SORT`/`MERGE` con `RELEASE`/`RETURN`.
> **Resuelto (1.7.0):** abreviación con objeto identificador;
> `INITIALIZE … REPLACING`; `66 RENAMES`; punteros (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`); `ALTER` / `UNLOCK`;
> `NEXT SENTENCE` fiel; las intrínsecas estándar restantes; y `ACCEPT`/`DISPLAY`
> de pantalla extendidos (ejecutados en modo CLI).
> **Resuelto (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (con los registros
> emparejados `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Resuelto (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (libera los bloqueos de registro INDEXED) y `CANCEL program`.
> **Resuelto (1.8.0):** `COMMIT` / `ROLLBACK` como transacciones de ficheros
> INDEXED controladas por el programa (motores de memoria y disco; registro de
> deshacer real en disco).

.<<

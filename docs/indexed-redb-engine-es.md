<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Motor INDEXED a prueba de caídas (redb)

PowerRustCOBOL incluye un segundo motor `STORAGE IS DISK` para archivos
`ORGANIZATION IS INDEXED`, construido sobre **redb** — un almacén clave-valor
ACID embebido y de Rust puro (árbol B+ con copy-on-write, páginas de metadatos
duplicadas, sumas de comprobación por página). Presenta el comportamiento COBOL
observable *idéntico* al del motor por defecto `PRCIDXD1`, pero está diseñado en
torno a cuatro objetivos operativos que el motor a medida no podía cumplir a
escala.

Hoy es **opcional** (el motor de disco por defecto sigue siendo `PRCIDXD1`):

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

Implementación:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Por qué — los cuatro objetivos

| Objetivo | Cómo lo cumple el motor redb |
|------|------------------------------|
| **OPEN es instantáneo, siempre** | redb solo lee su página de metadatos al abrir. **No hay directorio de registros en RAM que cargar ni barrido de recuperación**, ni siquiera tras una caída. Medido: unos 5 ms para abrir un archivo de 200 000 registros (independiente del número de registros). |
| **READ RANDOM / NEXT a velocidad de vértigo** | RANDOM es un descenso por el árbol B+; NEXT es un iterador secuencial de rango. Ambos corren sobre la caché de páginas de redb. Medido: unos 21 µs por lectura aleatoria con 200 000 registros. |
| **Hasta 250 M de registros (datos sin límite)** | La RAM residente es el conjunto de trabajo (la caché de redb), **no** el número de registros. No hay ninguna estructura `O(registros)` retenida en memoria. |
| **La seguridad es lo primero** | redb es plenamente ACID. `COMMIT` es un commit de transacción duradero (fsync); `ROLLBACK` es un aborto de transacción. Un corte de luz nunca puede dejar a la vista un índice partido — redb retrocede al último commit bueno mediante sus páginas de metadatos duplicadas. Sin pérdida de datos, sin corrupción de índice. |

Contraste con el motor `PRCIDXD1`, cuyo directorio de RecordId se carga entero en
RAM al hacer OPEN (≈16 bytes × cada RecordId jamás asignado) y cuyas
transacciones eran un registro de deshacer en RAM, persistido solo al hacer
CLOSE — de modo que no podía ni abrir al instante a escala ni sobrevivir a un
corte de luz a mitad de ejecución.

---

## Disposición en disco (tablas de redb)

| Tabla redb | Tipo     | clave → valor                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | bytes de la clave primaria → registro (opcionalmente comprimido) |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | bytes de la clave primaria → secuencia de inserción `u64`  |
| `meta`     | table    | descriptores `schema`, `compress`, `nextseq`   |

- Un **único multimap `alt`** guarda todas las claves alternativas, separadas por
  un índice de clave de 2 bytes en big-endian. El orden de bytes es por tanto
  `(índice de clave, valor alternativo, secuencia de inserción)` — lo que hace
  que las alternativas duplicadas se recorran en **orden de creación**, encajando
  exactamente con la ordenación de RecordId del motor de disco y con la regla
  COBOL para claves alternativas duplicadas.
- La maquinaria `seq` / `meta:nextseq` existe **solo** para ordenar los
  duplicados de clave alternativa. Los archivos sin claves alternativas se la
  saltan por completo y pagan una sola inserción en el árbol B+ por `WRITE`.
- Los registros se almacenan como imágenes posicionales de anchura fija (véase
  [`indexed-file-internals-es.md`](indexed-file-internals-es.md) §6); `WITH
  COMPRESSION` aplica el mismo RLE PackBits que usan los otros motores.

---

## Modelo de transacciones

Una apertura con escritura (`OUTPUT` / `I-O` / `EXTEND`) mantiene abierta una
`WriteTransaction` de redb desde el OPEN. Las lecturas a través de esa
transacción ven las escrituras aún no confirmadas del propio programa (el «leer
tus propias escrituras» de COBOL). Los verbos COBOL se corresponden
directamente:

| COBOL | redb |
|-------|------|
| `OPEN`     | inicia una transacción de escritura (modos con escritura) |
| `COMMIT`   | hace `commit()` de la transacción (duradero) y abre una nueva |
| `ROLLBACK` | hace `abort()` de la transacción (descarta todo desde el último `COMMIT`/`OPEN`) y abre una nueva |
| `CLOSE`    | `commit()` (commit implícito) |

Las aperturas en `INPUT` usan transacciones de lectura cortas. Como `ROLLBACK` es
un aborto real de redb, **no hace falta ningún registro de deshacer** — la
durabilidad y la reversión son garantías del propio almacén.

> Los verbos COBOL `COMMIT` / `ROLLBACK` actúan sobre **archivos INDEXED**, no
> sobre conexiones SQL (esas usan `COBOL-EXEC-SQL` con
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Paridad de comportamiento

Al motor se le exige el comportamiento exacto del motor por defecto: los mismos
fixtures versionados (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) se ejecutan bajo `--indexed-engine redb` y deben producir una
salida de DISPLAY idéntica — CRUD con clave primaria más alternativa `WITH
DUPLICATES`, persistencia a través de una reapertura, y `COMMIT`/`ROLLBACK`. Los
códigos de estado de archivo (`00/02/10/22/23/35/39/46/47/48/49/90/...`), la
resolución de la clave de referencia, la semántica de `START` y la regla de que
«REWRITE/DELETE necesitan un registro actual» coinciden todos.

Pruebas: `crates/cobolt-runtime/tests/test_indexed_redb.rs` (los fixtures bajo
redb + comprobaciones directas de `IndexedStore` + una prueba de humo de escala
marcada con `#[ignore]`).

---

## Límites

Como el motor pagina bajo demanda, los límites prácticos los fijan redb y el
sistema de archivos, no la RAM residente:

| Dimensión | Límite |
|-----------|-------|
| Tamaño del archivo | límite de redb / del sistema de archivos (terabytes) |
| Registros | limitado por la RAM del conjunto de trabajo, no por el número de registros (≥250 M con una caché pequeña) |
| Tamaño del registro | imagen de anchura fija; los registros grandes se guardan como valores de redb |
| Tamaño de la clave | bytes de la clave compuesta (la capa COBOL admite claves de varias partes) |
| Claves alternativas | hasta 65 535 (espacio de índice de 2 bytes) |

---

## Notas de rendimiento

- El **`READ NEXT` secuencial** por la clave primaria de referencia devuelve el
  registro directamente desde el cursor de rango — un descenso por el árbol B+
  por registro, no dos (unos 17 µs por registro con 200 000). Los barridos por
  clave alternativa siguen haciendo un descenso por la alternativa más una
  búsqueda en la primaria.
- El **`WRITE`** abre las tablas `primary`/`alt` una vez por operación (la
  comprobación de duplicados y la inserción comparten el handle). Un
  micro-benchmark mostró que mantener el handle en caché *entre* llamadas añade
  solo un 8 % sobre abrirlo una vez por operación, así que el motor conserva el
  camino sencillo y libre de `unsafe`. El coste de escritura (unos 44 µs por
  registro) lo domina la inserción ACID en el árbol B+ de redb, que es el suelo
  seguro — ninguna de las optimizaciones de escritura cambia los puntos de commit
  ni la durabilidad.
- El **`WRITE` masivo** queda por tanto en unos 20 mil registros/s dentro de una
  sola transacción (un coste de carga que se paga una vez). OPEN, lecturas y
  seguridad frente a caídas no se ven afectados.

---

## Registro de observabilidad (`--indexed-log`)

El motor redb puede escribir un registro de transacciones opcional por archivo
(apagado por defecto) en **`<assign-path>.log`** (por ejemplo, `customers.idx` →
`customers.idx.log`), con una línea por `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` que
anota la marca de tiempo, los recuentos de registros y bytes, el rendimiento, la
calidad del orden de las claves al escribir y —en el nivel `full`— estadísticas
de páginas de índice de redb.

```bash
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

El formato de línea es `text` (logfmt) o `json` (NDJSON, listo para
Grafana/Loki).

**La referencia completa** — flags, la tabla de campos, los formatos, el
pipeline de Grafana/Loki (Promtail + LogQL) y las notas de coste y seguridad —
está en [`observability-es.md`](observability-es.md) §1.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Motor INDEXED a prueba de caídas (redb)

PowerRustCOBOL incluye un segundo motor `STORAGE IS DISK` para los ficheros
`ORGANIZATION IS INDEXED`, construido sobre **redb** — un almacén clave-valor
ACID embebido y escrito íntegramente en Rust (B+tree con copia en escritura,
páginas meta duplicadas, sumas de comprobación por página). Presenta un
comportamiento COBOL observable *idéntico* al del motor `PRCIDXD1` anterior,
pero está diseñado en torno a cuatro objetivos operativos que el motor propio no
podía cumplir a escala.

**Es el motor por defecto, y lo es desde 1.62.73** (resolución del operador,
2026-08-29). `IndexedEngine` deriva `Default` con `#[default]` sobre `Redb`
(`crates/cobolt-runtime/src/indexed.rs:126`), y una prueba lo mantiene ahí
(`indexed.rs:1643`). No hay que seleccionar nada para obtenerlo.

El motor paginado anterior sigue disponible por su nombre, igual que los dos
alias que delegan en el contenedor Rust incorporado:

```bash
rcrun run program.cbl --indexed-engine rust    # el motor paginado PRCIDXD1
# o
COBOL_INDEXED_ENGINE=rust rcrun run program.cbl
```

Implementación:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Por qué — los cuatro objetivos

| Objetivo | Cómo lo cumple el motor redb |
|------|------------------------------|
| **OPEN es instantáneo, siempre** | redb solo lee su página meta al abrir. **No hay un directorio de registros en RAM que cargar ni un recorrido de recuperación**, ni siquiera tras una caída. Medido: ~5 ms para hacer OPEN de un fichero de 200 000 registros (con independencia del número de registros). |
| **READ RANDOM / NEXT a la velocidad de la luz** | RANDOM es un descenso por el B+tree; NEXT es un iterador de rango secuencial. Ambos se ejecutan sobre la caché de páginas de redb. Medido: ~21 µs por lectura aleatoria con 200 000 registros. |
| **Hasta 250 M de registros (datos sin límite)** | La RAM residente es el conjunto de trabajo (la caché de redb), **no** el número de registros. No se mantiene en memoria ninguna estructura `O(registros)`. |
| **La seguridad es lo primero** | redb es plenamente ACID. `COMMIT` es un commit de transacción duradero (fsync); `ROLLBACK` es un aborto de transacción. Un corte de corriente jamás puede dejar visible un índice partido: redb vuelve al último commit correcto mediante sus páginas meta duplicadas. Sin pérdida de datos ni corrupción del índice. |

Compárese con el motor `PRCIDXD1`, cuyo directorio de RecordId se carga entero
en RAM al hacer OPEN (≈16 bytes × cada RecordId asignado alguna vez) y cuyas
transacciones eran un registro de deshacer en RAM persistido solo al hacer
CLOSE — de modo que no podía ni abrir al instante a escala ni sobrevivir a un
corte de corriente a mitad de ejecución.

---

## Disposición en disco (tablas redb)

| Tabla redb | Tipo     | clave → valor                                 |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | bytes de la clave primaria → registro (opcionalmente comprimido) |
| `alt`      | multimap | `[u16 idx][bytes de la clave alterna]` → `[u64 seq][clave primaria]` |
| `seq`      | table    | bytes de la clave primaria → secuencia `u64` de inserción |
| `meta`     | table    | descriptores `schema`, `compress`, `nextseq`  |

- Un **único multimapa `alt`** contiene todas las claves alternas, con espacio de
  nombres dado por un índice de clave de 2 bytes en big-endian. El orden de bytes
  es por tanto `(índice de clave, valor alterno, secuencia de inserción)` — lo
  que hace que las alternas duplicadas se recorran en **orden de creación**,
  exactamente igual que el orden de RecordId del motor de disco y que la regla
  COBOL para claves alternas duplicadas.
- La maquinaria `seq` / `meta:nextseq` existe **solo** para ordenar los
  duplicados de claves alternas. Los ficheros sin claves alternas la omiten por
  completo y pagan una sola inserción en el B+tree por `WRITE`.
- Los registros se almacenan como imágenes posicionales de ancho fijo (véase
  [`indexed-file-internals-es.md`](indexed-file-internals-es.md) §6); `WITH
  COMPRESSION` aplica el mismo RLE PackBits que usan los demás motores.

---

## Modelo transaccional

Una apertura en escritura (`OUTPUT` / `I-O` / `EXTEND`) mantiene abierta una
`WriteTransaction` de redb desde el OPEN. Las lecturas a través de esa
transacción ven las escrituras sin confirmar del propio programa (el «leer lo
que has escrito» de COBOL). Los verbos COBOL se corresponden directamente:

| COBOL | redb |
|-------|------|
| `OPEN`     | inicia una transacción de escritura (modos de escritura) |
| `COMMIT`   | `commit()` de la transacción (duradero) y después inicia una nueva |
| `ROLLBACK` | `abort()` de la transacción (descarta todo desde el último `COMMIT`/`OPEN`) y después inicia una nueva |
| `CLOSE`    | `commit()` (confirmación implícita) |

Las aperturas `INPUT` usan transacciones de lectura cortas. Como `ROLLBACK` es un
aborto real de redb, **no hace falta ningún registro de deshacer**: la
durabilidad y la reversión son garantías del propio almacén.

> Los verbos COBOL `COMMIT` / `ROLLBACK` actúan sobre **ficheros INDEXED**, no
> sobre conexiones SQL (esas usan `COBOL-EXEC-SQL` con
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Paridad de comportamiento

El motor se somete al comportamiento exacto del motor por defecto: las mismas
pruebas versionadas (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) se ejecutan con `--indexed-engine redb` y deben producir una salida
DISPLAY idéntica — CRUD con clave primaria y alterna `WITH DUPLICATES`,
persistencia entre reaperturas, y `COMMIT`/`ROLLBACK`. Los códigos de estado de
fichero (`00/02/10/22/23/35/39/46/47/48/49/90/...`), la resolución de la clave de
referencia, la semántica de `START` y la regla de que «REWRITE/DELETE necesitan
un registro actual» coinciden todos.

Pruebas: `crates/cobolt-runtime/tests/test_indexed_redb.rs` (las pruebas con redb
+ comprobaciones directas de `IndexedStore` + una prueba de humo a escala marcada
`#[ignore]`).

---

## Límites

Como el motor es paginado bajo demanda, los límites prácticos los fijan redb y el
sistema de ficheros, no la RAM residente:

| Dimensión | Límite |
|-----------|-------|
| Tamaño de fichero | límite de redb / del sistema de ficheros (terabytes) |
| Registros | limitado por la RAM del conjunto de trabajo, no por el número de registros (≥250 M con una caché pequeña) |
| Tamaño de registro | imagen de ancho fijo; los registros grandes se guardan como valores redb |
| Tamaño de clave | bytes de la clave compuesta (la capa COBOL admite claves de varias partes) |
| Claves alternas | hasta 65 535 (espacio de índice de 2 bytes) |

---

## Notas de rendimiento

- El **`READ NEXT` secuencial** por la clave primaria de referencia devuelve el
  registro directamente desde el cursor de rango — un descenso por el B+tree por
  registro, no dos (~17 µs/registro con 200 000). Los recorridos por clave
  alterna siguen haciendo un descenso alterno más una búsqueda primaria.
- **`WRITE`** abre las tablas `primary`/`alt` una vez por operación (la
  comprobación de duplicados y la inserción comparten el manejador). Un
  micro-benchmark mostró que cachear el manejador *entre* llamadas solo añade un
  ~8 % sobre abrirlo una vez por operación, así que el motor conserva el camino
  simple y sin `unsafe`. El coste de escritura (~44 µs/registro) lo domina la
  inserción ACID en el B+tree de redb, que es el suelo seguro: ninguna de las
  optimizaciones de escritura cambia los puntos de confirmación ni la
  durabilidad.
- Por eso el **`WRITE` masivo** ronda los 20 k registros/s en una sola
  transacción (un coste de carga que se paga una vez). El OPEN, las lecturas y la
  resistencia a caídas no se ven afectados.

---

## Registro de observabilidad (`--indexed-log`)

El motor redb puede escribir un registro de transacciones opcional por fichero
(desactivado por defecto) en **`<ruta-assign>.log`** (p. ej. `customers.idx` →
`customers.idx.log`), con una línea por `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` que
recoge marca de tiempo, recuentos de registros y bytes, rendimiento, calidad del
orden de claves en escritura y — en el nivel `full` — estadísticas de páginas del
índice redb.

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

El formato de línea es `text` (logfmt) o `json` (NDJSON, listo para
Grafana/Loki).

**La referencia completa** — opciones, la tabla de campos, los formatos, la
canalización Grafana/Loki (Promtail + LogQL) y las notas de coste y seguridad —
está en [`observability-es.md`](observability-es.md) §1.

.<<

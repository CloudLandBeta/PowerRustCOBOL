<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Formato de fichero indexado de PowerRustCOBOL (`PRCIDX1`)

Este documento describe el contenedor en disco que respalda los ficheros
`ORGANIZATION IS INDEXED` en PowerRustCOBOL, y cómo se corresponde con los
metadatos que necesitará un futuro **importador de Fujitsu COBOL-85 →
PowerRustCOBOL**.

> **No es binariamente compatible con Fujitsu.** `PRCIDX1` es el contenedor
> autodescriptivo propio de PowerRustCOBOL. Está modelado *semánticamente* sobre
> los metadatos que las File Access Subroutines de Fujitsu exponen mediante
> `cobfa_indexinfo()` (formato de registro, longitud de registro, número y
> longitud total de claves, clave primaria, claves alternas), pero **no** analiza
> ni reproduce los bytes de `cobidx`/`cobi64` de Fujitsu. El importador es trabajo
> futuro y vive fuera de PowerRustCOBOL.

Implementación: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Por qué el formato es autodescriptivo

El contenedor original (`PRCISAM1`) guardaba solo un número mágico, la longitud
de registro y los bytes de los registros: **no llevaba esquema de claves**. Un
conversor (o cualquier herramienta externa) no podía saber cuáles eran las claves
sin el `FD` de COBOL.

`PRCIDX1` incrusta el esquema completo en el fichero: el formato de registro y,
de cada clave, su disposición en bytes, su orden, su política de duplicados y
(opcionalmente) su nombre de campo COBOL. Eso hace el fichero **descubrible**
—véase [`inspect_path`](#api-de-descubrimiento)— y permite que un importador de
Fujitsu escriba un fichero PowerRustCOBOL fiel a partir de los metadatos que lee
de un fichero Fujitsu, sin tener a mano un `FD` equivalente.

---

## Modelo de metadatos

Estos tipos Rust (reexportados desde `cobolt_runtime`) son el esquema. Reflejan
los conceptos de `cobfa_indexinfo()`; todos los desplazamientos y longitudes son
**en bytes** (nunca cuentas de caracteres, igual que la regla de Fujitsu en modo
Unicode).

```rust
pub enum RecordFormat {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

pub enum KeyEncoding {
    Bytes, DisplayAscii, DisplayUtf8,
    Ucs2Le, Ucs2Be, Utf32Le, Utf32Be,
    PackedDecimal, BinaryBigEndian, BinaryLittleEndian,
}

pub enum KeyOrdering { Ascending, Descending }

pub struct KeyPart { pub offset: u32, pub length: u32, pub encoding: KeyEncoding }

pub struct KeyDescriptor {
    pub key_number: u16,          // 1 = primary, 2.. = alternates (declaration order)
    pub name: Option<String>,     // descriptive COBOL field name (optional)
    pub parts: Vec<KeyPart>,      // concatenated → composite key value
    pub duplicates_allowed: bool,
    pub ordering: KeyOrdering,
}

pub struct IndexedFileInfo {
    pub record_format: RecordFormat,
    pub key_count: u16,           // primary + alternates
    pub total_key_length: u32,
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}
```

El runtime actual emite claves **de una sola parte, codificadas como `Bytes` y
`Ascending`** (que es a lo que se resuelve un `RECORD KEY` /
`ALTERNATE RECORD KEY` de un `FD` COBOL). Las claves compuestas, las
codificaciones alternativas y el orden descendente son **representables en el
formato** para que un importador pueda registrarlos sin pérdida; el soporte
completo en el runtime es trabajo futuro.

---

## Disposición del contenedor

Todos los enteros son **little-endian**. El fichero es:

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### Cabecera

| Campo            | Tipo      | Notas                                   |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | reservado (`0`)                         |
| `record_format`  | `u8`      | `1` = fijo, `2` = variable              |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | longitud de registro cuando es fijo     |
| `min_length`     | `u32`     | carga útil mínima cuando es variable    |
| `max_length`     | `u32`     | carga útil máxima cuando es variable    |
| `key_count`      | `u16`     | primaria + alternas                     |
| `created_unix_ms`| `u64`     | hora de creación, preservada al reescribir|
| `updated_unix_ms`| `u64`     | hora de la última escritura             |

### Esquema de claves — repetido `key_count` veces (la primaria primero)

| Campo          | Tipo      | Notas                                   |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primaria, `2..` alternas            |
| `duplicates`   | `u8`      | `0`/`1`                                 |
| `ordering`     | `u8`      | `0` ascendente, `1` descendente         |
| `part_count`   | `u16`     | número de `KeyPart`                     |
| `name_len`     | `u16`     | longitud del nombre UTF-8 (`0` = ninguno)|
| `name`         | `[u8]`    | `name_len` bytes                        |
| `parts`        | repetido  | `part_count` × KeyPart (abajo)          |

Cada **KeyPart**:

| Campo      | Tipo  | Notas                                   |
|------------|-------|-----------------------------------------|
| `offset`   | `u32` | desplazamiento en bytes dentro de la carga útil del registro|
| `length`   | `u32` | longitud en bytes                       |
| `encoding` | `u8`  | discriminante de `KeyEncoding`          |
| `reserved` | `u8`  | `0`                                     |

### Registros

| Campo          | Tipo   | Notas                                   |
|----------------|--------|-----------------------------------------|
| `record_count` | `u64`  | número de registros vivos               |
| por registro   | repetido | `length: u32` y después `length` bytes |

Los registros se escriben en orden ascendente de **clave primaria**.

### Cola

| Campo   | Tipo  | Notas                                            |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, reflejado) sobre todos los bytes anteriores a la cola |

El CRC se valida al cargar; una discrepancia produce FILE STATUS `90` (error de
E/S).

---

## API de descubrimiento

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Devuelve `Some(IndexedFileInfo)` para un fichero `PRCIDX1` y `None` para el
contenedor heredado `PRCISAM1` (que no lleva esquema). Este es el análogo de
`cobfa_indexinfo()` al que puede llamar un conversor o una herramienta de
inspección.

---

## Validación al abrir (FILE STATUS)

Al abrir un fichero indexado **existente** para `INPUT` / `I-O`, el runtime
valida las claves y el formato de registro declarados en el `SELECT`/`FD` contra
el esquema almacenado (modo estricto, activado por defecto). Estados relevantes:

| Estado | Condición                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` de un fichero inexistente                |
| `39`   | el esquema del fichero existente ≠ claves o formato de registro declarados |
| `90`   | contenedor corrupto (CRC discrepante) u otro error de E/S |

El contenedor heredado `PRCISAM1` no tiene esquema, así que con él se omite la
validación estricta (siempre se carga de forma permisiva).

---

## Modos de almacenamiento (`STORAGE IS MEMORY | DISK`)

La cláusula `STORAGE MODE` selecciona qué motor —y por tanto qué contenedor en
disco— respalda un fichero INDEXED. **El modo de almacenamiento por defecto es
`DISK`** (cuando no hay cláusula `STORAGE`). `WITH COMPRESSION` se aplica a
cualquiera de los dos modos; `WITH PERSISTENCE` solo se aplica a `MEMORY`.

| Modo | Motor | Contenedor | Notas |
|------|--------|-----------|-------|
| `MEMORY` | `BTreeMap` en RAM (`indexed.rs`) | `PRCIDX1` (este documento) | el fichero entero en memoria; **efímero por defecto**: `COMMIT` nunca escribe en disco. Con `WITH PERSISTENCE`, se guarda como `PRCIDX1` solo al hacer `CLOSE`. `OPEN OUTPUT` siempre (re)crea el contenedor. |
| `DISK` (por defecto) | almacén redb a prueba de caídas (`indexed_redb.rs`) desde 1.62.73; el B+tree paginado (`indexed_disk.rs`) con `--indexed-engine rust` | el propio de redb, o `PRCIDXD1` para el motor paginado | registros e índices leídos bajo demanda; RAM acotada; siempre persistente (escrituras por operación, `fsync` en `COMMIT`/`CLOSE`) |

El contenedor de disco **`PRCIDXD1`** es un único fichero paginado (páginas de
4 KiB):

* **página 0** — cabecera: raíces (un B+tree por clave), cabeza de la lista
  libre, siguiente identificador de página, contador de `RecordId`, número de
  registros, el esquema de claves y el indicador de compresión.
* **páginas de B+tree** — nodos internos y hoja (empaquetados en bytes de tamaño
  variable, con división al insertar y hojas doblemente enlazadas para recorridos
  ordenados).
* **páginas de datos** — celdas de registro con ranuras (varios registros por
  página), más una cadena de páginas de desbordamiento para registros mayores que
  una página.
* **páginas de directorio** — el mapa `RecordId` → ubicación física.
* una **lista libre** enhebra las páginas liberadas para reutilizarlas.

`WITH COMPRESSION` (`compress.rs`) es un RLE al estilo PackBits sin dependencias,
aplicado a cada registro almacenado (`PRCIDXD1`) o a cada registro de la sección
de registros (`PRCIDX1`); una etiqueta de un byte garantiza que la codificación
nunca crece, y la cabecera del contenedor anota que la compresión está activa.

> `PRCIDXD1` es para el almacenamiento nativo en modo DISK. Los metadatos
> descubribles y orientados a la importación desde Fujitsu que describimos arriba
> son los del contenedor `PRCIDX1` (modo MEMORY); un importador debería apuntar a
> `PRCIDX1` salvo que necesite específicamente la disposición paginada en disco.

## Compatibilidad hacia atrás

* `PRCIDX1` (número mágico `PRCIDX1\0`) — el formato autodescriptivo actual de
  modo MEMORY (lectura y escritura).
* `PRCIDXD1` (número mágico `PRCIDXD1`) — contenedor paginado de B+tree en modo
  DISK.
* `PRCISAM1` (número mágico `PRCISAM1`) — contenedor heredado de solo registros
  (solo lectura; se vuelve a guardar como `PRCIDX1` en el siguiente `CLOSE` de
  una apertura con escritura).
* Cualquier otro contenido — se trata como un fichero vacío.

---

## Futura vía de importación desde Fujitsu

El flujo de migración previsto (todo él hoy fuera del alcance de PowerRustCOBOL):

```text
Fujitsu runtime
  └─ cobfa_indexinfo()  → record format, record length, key list (primary + alternates)
  └─ sequential export  → record payloads
        │
        ▼
  converter (future, external)
        │  builds IndexedFileInfo + records
        ▼
  PRCIDX1 file  → opened natively by PowerRustCOBOL
```

Como `PRCIDX1` ya puede *representar* claves compuestas, codificaciones de clave,
orden de clave, política de duplicados, límites de registros de longitud variable
y nombres de campos clave, al conversor solo le queda traducir los metadatos de
Fujitsu a `IndexedFileInfo` y volcar los registros: no hace falta cambiar el
formato de PowerRustCOBOL.

**No** intente analizar los bytes crudos de `cobidx`/`cobi64` de Fujitsu. La
documentación pública de Fujitsu expone los metadatos a través de las File Access
Subroutines, pero no publica la disposición física de los bytes.

.<<

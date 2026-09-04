<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Formato de archivo indexado de PowerRustCOBOL (`PRCIDX1`)

Este documento describe el contenedor en disco que respalda los archivos
`ORGANIZATION IS INDEXED` en PowerRustCOBOL, y cómo se corresponde con los
metadatos que necesitará un futuro **importador Fujitsu COBOL-85 →
PowerRustCOBOL**.

> **No es compatible a nivel binario con Fujitsu.** `PRCIDX1` es el contenedor
> autodescriptivo propio de PowerRustCOBOL. Está modelado *semánticamente* sobre
> los metadatos que las File Access Subroutines de Fujitsu exponen mediante
> `cobfa_indexinfo()` (formato de registro, longitud de registro, número y
> longitud total de claves, clave primaria, claves alternativas), pero **no**
> analiza ni reproduce los bytes `cobidx`/`cobi64` de Fujitsu. El importador es
> trabajo futuro y vive fuera de PowerRustCOBOL.

Implementación: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Por qué el formato es autodescriptivo

El contenedor original (`PRCISAM1`) almacenaba solo un número mágico, la
longitud del registro y los bytes del registro: **no llevaba ningún esquema de
claves**. Un conversor (o cualquier herramienta externa) no podía saber cuáles
eran las claves sin el `FD` de COBOL.

`PRCIDX1` incrusta el esquema completo en el archivo: el formato de registro y,
para cada clave, su disposición de bytes, su ordenación, su política de
duplicados y (opcionalmente) su nombre de campo COBOL. Eso hace que el archivo
sea **explorable** —véase [`inspect_path`](#api-de-descubrimiento)— y permite
que un importador de Fujitsu escriba un archivo PowerRustCOBOL fiel a partir de
los metadatos que lee de un archivo Fujitsu, sin tener a mano un `FD`
equivalente.

---

## Modelo de metadatos

Estos tipos de Rust (reexportados desde `cobolt_runtime`) son el esquema.
Reflejan los conceptos de `cobfa_indexinfo()`; todos los desplazamientos y
longitudes están **expresados en bytes** (nunca en número de caracteres, tal
como impone la regla de Fujitsu para el modo Unicode).

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
`Ascending`** (que es a lo que se resuelve un `RECORD KEY` / `ALTERNATE RECORD
KEY` de un `FD` de COBOL). Las claves compuestas, otras codificaciones y el
orden descendente son **representables en el formato**, de modo que un
importador puede registrarlos sin pérdida; el soporte completo en el runtime
queda como trabajo futuro.

---

## Disposición del contenedor

Todos los enteros son **little-endian**. El archivo es:

```text
┌────────────────────────────────────────────────────────────┐
│ Cabecera                                                   │
│ Esquema de claves (key_count descriptores: primaria+alts)  │
│ Registros                                                  │
│ Tráiler CRC-32 (sobre todos los bytes anteriores)          │
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
| `fixed_length`   | `u32`     | longitud del registro si es fijo        |
| `min_length`     | `u32`     | carga útil mínima si es variable        |
| `max_length`     | `u32`     | carga útil máxima si es variable        |
| `key_count`      | `u16`     | primaria + alternativas                 |
| `created_unix_ms`| `u64`     | hora de creación, conservada entre reescrituras|
| `updated_unix_ms`| `u64`     | hora de la última escritura             |

### Esquema de claves — repetido `key_count` veces (la primaria primero)

| Campo          | Tipo      | Notas                                   |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primaria, `2..` alternativas        |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` ascendente, `1` descendente         |
| `part_count`   | `u16`     | número de `KeyPart`                     |
| `name_len`     | `u16`     | longitud del nombre UTF-8 (`0` = ninguno)|
| `name`         | `[u8]`    | `name_len` bytes                        |
| `parts`        | repetido  | `part_count` × KeyPart (abajo)          |

Cada **KeyPart**:

| Campo      | Tipo  | Notas                          |
|------------|-------|--------------------------------|
| `offset`   | `u32` | desplazamiento en bytes dentro de la carga útil|
| `length`   | `u32` | longitud en bytes              |
| `encoding` | `u8`  | discriminante de `KeyEncoding` |
| `reserved` | `u8`  | `0`                            |

### Registros

| Campo          | Tipo     | Notas                                |
|----------------|----------|--------------------------------------|
| `record_count` | `u64`    | número de registros vivos            |
| por registro   | repetido | `length: u32` y luego `length` bytes |

Los registros se escriben en orden ascendente de **clave primaria**.

### Tráiler

| Campo   | Tipo  | Notas                                            |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, reflejado) sobre todos los bytes anteriores al tráiler |

El CRC se valida al cargar; una discrepancia produce FILE STATUS `90` (error de
E/S).

---

## API de descubrimiento

```rust
use cobolt_runtime::IndexedFile; // (engine type)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Devuelve `Some(IndexedFileInfo)` para un archivo `PRCIDX1` y `None` para el
contenedor heredado `PRCISAM1` (que no lleva esquema). Este es el análogo de
`cobfa_indexinfo()` que puede invocar un conversor o una herramienta de
inspección.

---

## Validación en la apertura (FILE STATUS)

Al abrir un archivo indexado **existente** para `INPUT` / `I-O`, el runtime
valida las claves declaradas en `SELECT`/`FD` y el formato de registro frente al
esquema almacenado (modo estricto, activo por defecto). Estados relevantes:

| Estado | Condición                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` de un archivo inexistente                |
| `39`   | el esquema del archivo existente ≠ las claves/el formato de registro declarados |
| `90`   | contenedor corrupto (CRC no coincide) u otro error de E/S |

El contenedor heredado `PRCISAM1` no tiene esquema, así que la validación
estricta se omite para él (siempre se carga de forma permisiva).

---

## Modos de almacenamiento (`STORAGE IS MEMORY | DISK`)

La cláusula `STORAGE MODE` selecciona qué motor —y por tanto qué contenedor en
disco— respalda un archivo INDEXED. **El modo de almacenamiento por defecto es
`DISK`** (cuando no hay ninguna cláusula `STORAGE`). `WITH COMPRESSION` se
aplica a cualquiera de los dos modos; `WITH PERSISTENCE` se aplica solo a
`MEMORY`.

| Modo | Motor | Contenedor | Notas |
|------|-------|------------|-------|
| `MEMORY` | `BTreeMap` en RAM (`indexed.rs`) | `PRCIDX1` (este documento) | archivo completo en memoria; **efímero por defecto**: `COMMIT` nunca escribe en disco. Con `WITH PERSISTENCE`, se guarda en `PRCIDX1` únicamente al hacer `CLOSE`. `OPEN OUTPUT` siempre (re)crea el contenedor. |
| `DISK` (por defecto) | árbol B+ paginado y persistente (`indexed_disk.rs`) | `PRCIDXD1` | registros e índices leídos bajo demanda; RAM acotada; siempre persistente (escrituras por operación, `fsync` en `COMMIT`/`CLOSE`) |

El contenedor de disco **`PRCIDXD1`** es un único archivo paginado (páginas de
4 KiB):

* **página 0**: cabecera con las raíces (un árbol B+ por clave), la cabeza de la
  lista de libres, el id de la siguiente página, el contador de `RecordId`, el
  número de registros, el esquema de claves y el indicador de compresión.
* **páginas de árbol B+**: nodos internos y hoja (empaquetados en bytes de
  tamaño variable, se dividen al insertar, con las hojas doblemente enlazadas
  para los recorridos ordenados).
* **páginas de datos**: celdas de registro con ranuras (varios registros por
  página), más una cadena de páginas de desbordamiento para los registros
  mayores que una página.
* **páginas de directorio**: el mapa `RecordId` → ubicación física.
* una **lista de libres** encadena las páginas liberadas para reutilizarlas.

`WITH COMPRESSION` (`compress.rs`) es un RLE de estilo PackBits, sin
dependencias, aplicado a cada registro almacenado (`PRCIDXD1`) o a cada registro
de la sección de registros (`PRCIDX1`); una etiqueta de un byte garantiza que la
codificación nunca crezca, y la cabecera del contenedor deja constancia de que
la compresión está activada.

> `PRCIDXD1` es el almacenamiento nativo del modo DISK. Los metadatos
> explorables y orientados a la importación desde Fujitsu descritos arriba
> corresponden al contenedor `PRCIDX1` (modo MEMORY); un importador debería
> apuntar a `PRCIDX1` salvo que necesite específicamente la disposición paginada
> en disco.

## Compatibilidad hacia atrás

* `PRCIDX1` (número mágico `PRCIDX1\0`): formato autodescriptivo actual del modo
  MEMORY (lectura y escritura).
* `PRCIDXD1` (número mágico `PRCIDXD1`): contenedor de árbol B+ paginado del
  modo DISK.
* `PRCISAM1` (número mágico `PRCISAM1`): contenedor heredado que solo guarda
  registros (solo lectura; se vuelve a guardar como `PRCIDX1` en el siguiente
  `CLOSE` de una apertura con escritura).
* Cualquier otro contenido: se trata como un archivo vacío.

---

## Futura vía de importación desde Fujitsu

El flujo de migración previsto (hoy, todo ello fuera del alcance de
PowerRustCOBOL):

```text
runtime de Fujitsu
  └─ cobfa_indexinfo()  → formato de registro, longitud de registro, lista de claves (primaria + alternativas)
  └─ exportación secuencial → cargas útiles de los registros
        │
        ▼
  conversor (futuro, externo)
        │  construye IndexedFileInfo + registros
        ▼
  archivo PRCIDX1  → abierto de forma nativa por PowerRustCOBOL
```

Como `PRCIDX1` ya puede *representar* claves compuestas, codificaciones de
clave, ordenación de claves, política de duplicados, límites de registros de
longitud variable y nombres de campos clave, el conversor solo tiene que
traducir los metadatos de Fujitsu a `IndexedFileInfo` y transmitir los
registros: no hace falta ningún cambio de formato en PowerRustCOBOL.

**No** intentes analizar los bytes `cobidx`/`cobi64` de Fujitsu en crudo. La
documentación pública de Fujitsu expone los metadatos a través de las File
Access Subroutines, pero no publica la disposición física de los bytes.

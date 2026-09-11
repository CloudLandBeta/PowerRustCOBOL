<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.134 -->

# Interioridades de los ficheros indexados de PowerRustCOBOL (motor paginado `PRCIDXD1`)

Este documento es un esquema conceptual del motor **persistente y paginado en
disco** que respalda los ficheros `ORGANIZATION IS INDEXED` declarados con
`STORAGE IS DISK` (el valor por defecto). Es un diseño de B+tree con páginas
ranuradas que lee los registros bajo demanda, de modo que la RAM queda acotada
sea cual sea el tamaño del fichero.

> ⚠️ **Este ya no es el motor por defecto.** `STORAGE IS DISK` sigue siendo el
> *modo de almacenamiento* por defecto, pero desde **1.62.73** el motor que lo
> sirve es **redb** (`crates/cobolt-runtime/src/indexed.rs:126`) — véase
> [`indexed-redb-engine-es.md`](indexed-redb-engine-es.md). Todo lo que sigue
> continúa siendo exacto para el motor paginado, al que todavía se llega con
> `--indexed-engine rust`; sencillamente no es lo que un programa obtiene por
> defecto.
>
> **Alcance.** Esto describe el *motor físico* (`DiskIndexedFile`, número mágico
> del contenedor `PRCIDXD1`). Es un artefacto distinto del contenedor `PRCIDX1`,
> de bloque único y autodescriptivo, documentado en
> [`indexed-file-format-es.md`](indexed-file-format-es.md), que modela los
> metadatos que necesitará un futuro importador de Fujitsu. El motor en memoria
> (`STORAGE IS MEMORY`, `IndexedFile`) es un subconjunto simplificado del mismo
> modelo lógico (BTreeMap en lugar de B+trees en disco).
>
> Un segundo motor `STORAGE IS DISK`, **a prueba de caídas** (opcional, sobre el
> almacén ACID redb escrito en Rust puro), resuelve el directorio acotado por RAM
> y la persistencia solo-en-CLOSE de este motor — véase
> [`indexed-redb-engine-es.md`](indexed-redb-engine-es.md).

Implementación:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
y la (des)materialización de registros en
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. El diseño en una frase

Un fichero paginado de **una página de cabecera + N B+trees (uno por clave) → un
directorio de RecordId → páginas de datos ranuradas con imágenes de registro
posicionales y de ancho fijo**, con una lista libre, cadenas de desbordamiento,
compresión RLE opcional y un registro de deshacer en memoria para las
transacciones.

---

## 2. El fichero es una tabla de páginas fijas de 4 KiB

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

Todas las páginas **posteriores** a la página 0 se identifican por su primer byte
(la etiqueta de tipo de página). Las páginas liberadas se reciclan mediante una
lista libre, así que el orden físico de las páginas en disco **no** sigue el
orden lógico de los registros.

| Etiqueta | Constante     | La página contiene                            |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | nodo interno (de encaminamiento) del B+tree   |
| `2` | `PT_LEAF`     | nodo hoja del B+tree (doblemente enlazado con sus hermanos) |
| `3` | `PT_DATA`     | página ranurada que empaqueta varias imágenes de registro |
| `4` | `PT_OVERFLOW` | continuación de un registro demasiado grande para caber en línea |
| `5` | `PT_DIR`      | un trozo del directorio de RecordId           |

---

## 3. Página 0 — la cabecera

La página 0 es el único lugar donde se guarda un *esquema*, y se escribe una sola
vez. Los campos son little-endian, en este orden:

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (each u64)
 primary_root   dir_head         directory_len                 (each u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (one B+tree root per alt key)
 ──────────────────────────────────────────────────────────────────────
 KEY SCHEMA:  key_count (u16) → for each key (primary first, then alternates):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × parts   (composite-key parts)
```

| Campo de cabecera | Significado                                                   |
|-------------------|---------------------------------------------------------------|
| `version`         | Versión del formato (actualmente `1`).                        |
| `page_size`       | Tamaño de página en bytes (4096).                             |
| `rec_fmt`         | Formato de registro: `1` = longitud fija.                     |
| `compressing`     | `1` si las cargas útiles de registro van comprimidas con RLE en disco. |
| `record_len`      | Longitud lógica (sin comprimir) del registro, en bytes.       |
| `next_page_id`    | Siguiente identificador de página a asignar cuando la lista libre está vacía. |
| `free_list_head`  | Primera página de la lista libre de páginas recuperadas (`0` = ninguna). |
| `record_count`    | Número de registros vivos.                                    |
| `data_tail`       | Página `PT_DATA` actual que acepta escrituras en línea (`0` = ninguna). |
| `primary_root`    | Página raíz del B+tree de la clave primaria.                  |
| `dir_head`        | Primera página `PT_DIR` del directorio de RecordId (`0` = ninguna). |
| `directory_len`   | Número de entradas del directorio (RecordId asignados alguna vez). |
| `alt_root[k]`     | Página raíz del B+tree de la clave alterna *k*.               |
| ESQUEMA DE CLAVES | Política de duplicados por clave y rangos de bytes de las partes compuestas. |

**Lo que deliberadamente *no* está en la cabecera:** no hay **nombres de campos
de datos** ni **metadatos por registro**. El esquema es puramente *geometría de
claves* (rangos de bytes). Todo lo demás de un registro es posicional — véase la
§6.

---

## 4. El camino de acceso (cómo se resuelve un `READ` por clave)

```
  COBOL key value (bytes)
        │
        ▼
  ┌──────────────┐   Start at primary_root (random READ by RECORD KEY) or
  │  B+tree      │   alt_roots[k] (READ KEY IS <alt>). Internal nodes route by
  │  (one per    │   key; leaves hold (key_bytes → RecordId) and are doubly
  │  key)        │   linked (next/prev) for READ NEXT / READ PREVIOUS / START.
  └──────┬───────┘
         │  RecordId (a stable integer, independent of physical location)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = free/tombstone, 1 = inline, 2 = overflow head
  │  directory   │     len : stored (possibly compressed) byte length
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Slotted DATA page → slot directory → (offset, len) →
  │  DATA page   │   raw record image (decompressed if `compressing`).
  └──────┬───────┘
         ▼
  the fixed-width record bytes
        │  RecordLayout.distribute()
        ▼
  scattered into the FD's elementary items in working memory
```

**Un registro, muchas claves.** La clave primaria y todas las alternas apuntan al
*mismo* RecordId, así que hay exactamente una copia almacenada de cada registro.
Los índices alternos son simplemente B+trees adicionales superpuestos al
directorio de RecordId compartido; un valor alterno duplicado se permite cuando
esa clave se declaró `WITH DUPLICATES`.

---

## 5. Interior de las páginas

### 5.1 Nodo del B+tree (`PT_INTERNAL` / `PT_LEAF`)

Un nodo se carga en memoria para una operación, se modifica, se divide si hace
falta y se vuelve a escribir.

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- Las hojas están **doblemente enlazadas** (`next`/`prev`), de modo que un
  recorrido ordenado tras un `START` camina directamente por los hermanos: eso es
  el `READ NEXT` de clave ascendente de RustCOBOL.
- La inserción **divide al desbordar** cuando el nodo serializado excedería
  `PAGE_SIZE`; la clave mediana asciende al padre.
- Los nodos internos contienen `child0` más pares *(clave separadora, hijo)*.

### 5.2 Página de datos ranurada (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- Cabecera de página de 5 bytes y después un **directorio de ranuras** que crece
  desde el principio, mientras que las **cargas útiles de los registros** crecen
  desde el final; un registro cabe en línea mientras las dos regiones no se hayan
  encontrado.
- Una ranura es `(offset, len)`; borrar un registro pone su ranura a `len = 0`
  (lápida). Cuando todas las ranuras de una página están libres, la página entera
  vuelve a la lista libre.
- El campo `slot` de un `RecLoc` indexa dentro de este directorio de ranuras.

### 5.3 Cadena de desbordamiento (`PT_OVERFLOW`)

Un registro mayor que el límite en línea (`PAGE_SIZE − cabecera − una ranura`) se
almacena como una cadena enlazada de páginas de desbordamiento; su
`RecLoc.kind = 2` y `page` apunta a la cabeza de la cadena.

### 5.4 Directorio de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

El directorio se mantiene en RAM como un `Vec<RecLoc>` mientras el fichero está
abierto (así, buscar un RecordId es un índice O(1)) y se persiste como una cadena
de páginas `PT_DIR` (empezando en `dir_head`) al cerrar. Los B+trees guardan
RecordId, nunca direcciones físicas, de modo que un registro puede moverse en
disco sin tocar ningún índice.

---

## 6. La imagen del registro (posicional, sin nombres)

Un registro en disco es un único **búfer de bytes de ancho fijo** dispuesto por
*desplazamiento* de campo: no hay nombres de campo, ni etiquetas, ni delimitadores
en la carga útil. Para:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

la imagen almacenada ocupa 23 bytes:

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- `RecordLayout::materialize()` empaqueta los elementos elementales del `FD` en
  este búfer por desplazamiento para `WRITE`/`REWRITE`;
  `RecordLayout::distribute()` lo invierte en el `READ`. El mapa de campo →
  desplazamiento vive únicamente en el `RecordLayout` del programa (derivado del
  `FD`), **nunca** en el fichero.
- **La identidad es la posición.** Este es el caso límite de «no repetir las
  claves en cada registro»: la identidad de un campo cuesta *cero* bytes por
  registro, y el acceso a un campo es O(1) por desplazamiento precalculado (sin
  análisis). Renombrar un campo que no es clave no cambia nada en disco;
  renombrar un campo clave reescribe solo el esquema de claves de la cabecera, no
  los registros ni los índices. Cambiar el desplazamiento o el ancho de un campo
  es el único cambio que obliga a reescribir los datos, algo inherente a los
  registros de longitud fija (y a los ISAM/VSAM de verdad).

### Compresión

Con `STORAGE IS DISK WITH COMPRESSION`, la carga útil **almacenada** va
comprimida con RLE PackBits (`compress.rs`), y `RecLoc.len` es la longitud
*almacenada*; el búfer se expande de vuelta a `record_len` al leer. La compresión
es transparente para la geometría de claves y para el camino de acceso.

---

## 7. Espacio libre y reutilización

- **Lista libre.** `free_list_head` encadena las páginas recuperadas de páginas de
  datos que se vaciaron, de nodos huérfanos tras una división, etc.; `allocate`
  saca de ahí antes de incrementar `next_page_id`, así que el espacio se reutiliza
  y el fichero no crece de forma monótona.
- **Lápidas.** Un `DELETE` libera la ranura (y perezosamente la página de datos)
  y marca la entrada del directorio como `RecLoc::FREE`; el RecordId se retira.

---

## 8. Transacciones (registro de deshacer en memoria)

El motor de disco mantiene un **registro de deshacer** con las operaciones
inversas de cada modificación desde el último `COMMIT`/`OPEN`:

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- `OPEN` inicia una transacción (limpia el registro); `COMMIT` hace durables los
  cambios y comienza otra; `ROLLBACK` reproduce las inversas en orden inverso;
  `CLOSE` vuelca (confirmación implícita). Un guardián `tx_replay` impide que las
  operaciones inversas se registren a su vez.
- Esto es reversión **a nivel de programa**. La recuperación ante caídas mediante
  un registro de escritura anticipada duradero es trabajo futuro. Véanse los
  verbos COBOL `COMMIT`/`ROLLBACK` en la referencia del lenguaje; recuerde que
  esos verbos actúan sobre **ficheros INDEXED**, no sobre conexiones SQL.

---

## 9. Validación al abrir

En el `OPEN`, el esquema de claves guardado en la cabecera se compara con el
`SELECT` del programa (longitud de registro, número de claves, las partes de cada
clave y su política de duplicados). Una discrepancia devuelve el estado de
fichero COBOL `39`; un fichero inexistente abierto como `INPUT` devuelve `35`;
una cabecera corrupta o truncada devuelve `90`. (La validación estricta puede
relajarse con el indicador `strict_metadata` del motor.)

---

## 10. Referencia rápida — quién guarda qué

| Cosa                          | Dónde vive                             | Copias      |
|-------------------------------|----------------------------------------|-------------|
| Geometría de claves (desplazamientos y anchos) | Esquema de claves de la cabecera (página 0) | una |
| Nombres de los campos de datos | Solo el `FD` del programa             | no está en el fichero |
| Bytes de los registros        | Páginas `PT_DATA` / `PT_OVERFLOW`       | una por registro |
| clave → RecordId              | un B+tree por clave                    | uno por clave |
| RecordId → ubicación física   | Directorio de RecordId (cadena `PT_DIR`) | una por registro |
| Páginas libres                | Lista libre (`free_list_head`)         | —           |
| Inversas de cambios sin confirmar | Registro de deshacer en RAM        | por transacción |

.<<

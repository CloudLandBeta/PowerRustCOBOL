<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Funcionamiento interno del archivo indexado de PowerRustCOBOL (motor paginado `PRCIDXD1`)

Este documento es un esquema conceptual del motor **persistente y paginado en
disco** que respalda los archivos `ORGANIZATION IS INDEXED` declarados con
`STORAGE IS DISK` (el valor por defecto). Es un diseño de árbol B+ / páginas con
ranuras que lee los registros bajo demanda, de modo que la RAM permanece acotada
sea cual sea el tamaño del archivo.

> **Alcance.** Aquí se describe el *motor físico* (`DiskIndexedFile`, magia de
> contenedor `PRCIDXD1`). Es un artefacto distinto del contenedor `PRCIDX1`,
> autodescriptivo y de blob único, documentado en
> [`indexed-file-format-en.md`](indexed-file-format-es.md), que modela los metadatos que
> necesitará un futuro importador de Fujitsu. El motor en memoria
> (`STORAGE IS MEMORY`, `IndexedFile`) es un subconjunto simplificado del mismo
> modelo lógico (BTreeMaps en lugar de árboles B+ en disco).
>
> Un segundo motor `STORAGE IS DISK`, **a prueba de caídas** (opcional, sobre el
> almacén ACID redb de Rust puro), resuelve el directorio acotado por RAM y la
> persistencia solo-en-CLOSE de este motor — véase
> [`indexed-redb-engine-es.md`](indexed-redb-engine-es.md).

Implementación:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
(des)materialización de registros en
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. El diseño en una frase

Un archivo paginado formado por **una página de cabecera + N árboles B+ (uno por
clave) → un directorio de RecordId → páginas de datos con ranuras que contienen
imágenes de registro posicionales y de ancho fijo**, con una lista de libres,
cadenas de desbordamiento, compresión RLE opcional y un registro de deshacer
válido durante la ejecución para las transacciones.

---

## 2. El archivo es una matriz de páginas fijas de 4 KiB

```
 byte 0                                                    fin del archivo
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fijo).   id de página = desplazamiento / 4096.
```

Toda página **posterior** a la página 0 se identifica a sí misma mediante su
primer byte (la etiqueta de tipo de página). Las páginas liberadas se reciclan a
través de una lista de libres, así que el orden físico de las páginas en disco
**no** sigue el orden lógico de los registros.

| Etiqueta | Constante     | La página contiene                                |
|-----|---------------|--------------------------------------------------------|
| `1` | `PT_INTERNAL` | nodo interno (de encaminamiento) del árbol B+           |
| `2` | `PT_LEAF`     | nodo hoja del árbol B+ (doblemente enlazado a hermanos) |
| `3` | `PT_DATA`     | página con ranuras que empaqueta varias imágenes de registro |
| `4` | `PT_OVERFLOW` | continuación de un registro demasiado grande para caber en línea |
| `5` | `PT_DIR`      | una porción del directorio de RecordId                  |

---

## 3. Página 0 — la cabecera

La página 0 es el único sitio donde se almacena un *esquema*, y se escribe una
sola vez. Los campos son little-endian, en este orden:

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (cada uno u64)
 primary_root   dir_head         directory_len                 (cada uno u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (una raíz B+ por clave alterna)
 ──────────────────────────────────────────────────────────────────────
 ESQUEMA DE CLAVES:  key_count (u16) → por cada clave (primaria primero, luego alternas):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × partes   (partes de clave compuesta)
```

| Campo de la cabecera | Significado                                            |
|-------------------|---------------------------------------------------------|
| `version`         | Versión del formato (actualmente `1`).                  |
| `page_size`       | Tamaño de página en bytes (4096).                       |
| `rec_fmt`         | Formato de registro: `1` = longitud fija.               |
| `compressing`     | `1` si las cargas de los registros se comprimen con RLE en disco. |
| `record_len`      | Longitud lógica (sin comprimir) del registro en bytes.  |
| `next_page_id`    | Siguiente id de página a asignar cuando la lista de libres está vacía. |
| `free_list_head`  | Primera página de la lista de libres de páginas recuperadas (`0` = ninguna). |
| `record_count`    | Número de registros vivos.                              |
| `data_tail`       | Página `PT_DATA` actual que acepta escrituras en línea (`0` = ninguna). |
| `primary_root`    | Página raíz del árbol B+ de la clave primaria.          |
| `dir_head`        | Primera página `PT_DIR` del directorio de RecordId (`0` = ninguna). |
| `directory_len`   | Número de entradas del directorio (RecordId asignados en total). |
| `alt_root[k]`     | Página raíz del árbol B+ de la clave alterna *k*.       |
| ESQUEMA DE CLAVES | Política de duplicados por clave + rangos de bytes de las partes compuestas. |

**Lo que deliberadamente *no* está en la cabecera:** no hay **nombres de campos
de datos** ni **metadatos por registro**. El esquema es pura *geometría de
claves* (rangos de bytes). Todo lo demás de un registro es posicional — véase §6.

---

## 4. La ruta de acceso (cómo se resuelve un `READ` por clave)

```
  valor de clave COBOL (bytes)
        │
        ▼
  ┌──────────────┐   Empieza en primary_root (READ aleatorio por RECORD KEY) o
  │  B+tree      │   en alt_roots[k] (READ KEY IS <alt>). Los nodos internos
  │  (uno por    │   encaminan por clave; las hojas guardan (key_bytes →
  │  clave)      │   RecordId) y están doblemente enlazadas (next/prev) para
  └──────┬───────┘   READ NEXT / READ PREVIOUS / START.
         │  RecordId (un entero estable, independiente de la ubicación física)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = libre/lápida, 1 = en línea, 2 = cabeza de desbordamiento
  │  directorio  │     len : longitud en bytes almacenada (quizá comprimida)
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Página DATA con ranuras → directorio de ranuras →
  │  página DATA │   (offset, len) → imagen bruta del registro (descomprimida
  └──────┬───────┘   si `compressing`).
         ▼
  los bytes del registro de ancho fijo
        │  RecordLayout.distribute()
        ▼
  repartidos en los ítems elementales del FD en la memoria de trabajo
```

**Un registro, muchas claves.** La clave primaria y todas las alternas apuntan al
*mismo* RecordId, así que existe exactamente una copia almacenada de cada
registro. Los índices alternos no son más que árboles B+ adicionales superpuestos
sobre el directorio de RecordId compartido; se admite un valor alterno duplicado
cuando esa clave se declaró `WITH DUPLICATES`.

---

## 5. Interior de las páginas

### 5.1 Nodo de árbol B+ (`PT_INTERNAL` / `PT_LEAF`)

Un nodo se carga en memoria para una operación, se modifica, se divide si hace
falta y se vuelve a escribir.

```
 Hoja:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Interno:   type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- Las hojas están **doblemente enlazadas** (`next`/`prev`), de modo que un
  recorrido ordenado tras un `START` camina directamente por los hermanos — eso
  es el `READ NEXT` de clave ascendente de RustCOBOL.
- La inserción **divide al desbordar** cuando el nodo serializado superaría
  `PAGE_SIZE`; la clave mediana se promociona al padre.
- Los nodos internos contienen `child0` más pares *(clave separadora, hijo)*.

### 5.2 Página de datos con ranuras (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ dir. de ranuras ─────┬─ libre ┬─ datos reg. ──┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  compactos    │
 │          │ count   │ top     │ crece  →              │        │  ←  crecen    │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- Cabecera de página de 5 bytes y a continuación un **directorio de ranuras** que
  crece desde el principio, mientras que las **cargas de los registros** crecen
  desde el final; un registro cabe en línea mientras ambas regiones no se hayan
  encontrado.
- Una ranura es `(offset, len)`; borrar un registro pone su ranura a `len = 0`
  (lápida). Cuando todas las ranuras de una página están libres, la página entera
  se devuelve a la lista de libres.
- El campo `slot` de un `RecLoc` indexa dentro de este directorio de ranuras.

### 5.3 Cadena de desbordamiento (`PT_OVERFLOW`)

Un registro mayor que el límite en línea (`PAGE_SIZE − cabecera − una ranura`) se
almacena como una cadena enlazada de páginas de desbordamiento; su
`RecLoc.kind = 2` y `page` apunta a la cabeza de la cadena.

### 5.4 Directorio de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entrada)
```

El directorio se mantiene en RAM como un `Vec<RecLoc>` mientras el archivo está
abierto (de modo que buscar un RecordId es un índice O(1)) y se persiste al
cerrar como una cadena de páginas `PT_DIR` (empezando en `dir_head`). Los árboles
B+ almacenan RecordId, nunca direcciones físicas, así que un registro puede
moverse en disco sin tocar ningún índice.

---

## 6. La imagen del registro en sí (posicional, sin nombres)

Un registro en disco es un único **búfer de bytes de ancho fijo** dispuesto por
*desplazamiento* de campo — no hay nombres de campo, etiquetas ni delimitadores
en la carga. Para:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

la imagen almacenada ocupa 23 bytes:

```
 desplazamiento: 0        5                     15              23
                 ┌────────┬─────────────────────┬───────────────┐
 carga útil:     │ 00001  │ John Doe░░          │ Sao Paulo     │
                 └────────┴─────────────────────┴───────────────┘
                   ID(5)     NAME(10)              CITY(8)
                   (░ = relleno con espacios)
```

- `RecordLayout::materialize()` empaqueta los ítems elementales del FD en este
  búfer por desplazamiento para `WRITE`/`REWRITE`; `RecordLayout::distribute()`
  hace lo inverso en el `READ`. El mapa campo → desplazamiento vive únicamente en
  el `RecordLayout` del programa (derivado del `FD`), **nunca** en el archivo.
- **La identidad es la posición.** Este es el caso límite de «no repetir las
  claves en cada registro»: la identidad de un campo cuesta *cero* bytes por
  registro y el acceso al campo es O(1) por desplazamiento precalculado (sin
  análisis sintáctico). Renombrar un campo que no es clave no cambia nada en
  disco; renombrar un campo clave reescribe solo el esquema de claves de la
  cabecera, no los registros ni los índices. Cambiar el desplazamiento o la
  anchura de un campo es el único cambio que obliga a reescribir los datos —
  inherente a los registros de longitud fija (y a los ISAM/VSAM reales).

### Compresión

Con `STORAGE IS DISK WITH COMPRESSION`, la carga **almacenada** se comprime con
PackBits-RLE (`compress.rs`) y `RecLoc.len` es la longitud *almacenada*; el búfer
se expande de nuevo hasta `record_len` al leer. La compresión es transparente
para la geometría de las claves y para la ruta de acceso.

---

## 7. Espacio libre y reutilización

- **Lista de libres.** `free_list_head` encadena las páginas recuperadas de
  páginas de datos vaciadas, nodos huérfanos por una división, etc.; `allocate`
  saca de ella antes de incrementar `next_page_id`, de modo que el espacio se
  reutiliza y el archivo no crece de forma monótona.
- **Lápidas.** Un `DELETE` libera la ranura (y de forma perezosa la página de
  datos) y marca la entrada del directorio como `RecLoc::FREE`; el RecordId se
  retira.

---

## 8. Transacciones (registro de deshacer en ejecución)

El motor de disco mantiene un **registro de deshacer** con las inversas de cada
mutación posterior al último `COMMIT`/`OPEN`:

```
 DiskUndo::Insert(key)        ← un WRITE   → se deshace borrando esa clave
 DiskUndo::Update(prev_image) ← un REWRITE → se deshace reescribiendo la imagen previa
 DiskUndo::Delete(prev_image) ← un DELETE  → se deshace volviendo a escribir la imagen
```

- `OPEN` inicia una transacción (limpia el registro); `COMMIT` hace duraderos los
  cambios e inicia otra; `ROLLBACK` reproduce las inversas en orden inverso;
  `CLOSE` vuelca a disco (commit implícito). Una guarda `tx_replay` impide que
  las operaciones inversas se vuelvan a registrar a sí mismas.
- Esto es una vuelta atrás **a nivel de programa**. La recuperación tras una
  caída mediante un registro de escritura anticipada duradero es trabajo futuro.
  Véanse los verbos COBOL `COMMIT`/`ROLLBACK` en la referencia del lenguaje;
  nótese que esos verbos actúan sobre **archivos INDEXED**, no sobre conexiones
  SQL.

---

## 9. Validación en OPEN

En el `OPEN`, el esquema de claves almacenado en la cabecera se compara con el
`SELECT` del programa (longitud de registro, número de claves, las partes de cada
clave y su política de duplicados). Una discrepancia devuelve el estado de
archivo COBOL `39`; un archivo inexistente abierto como `INPUT` devuelve `35`;
una cabecera corrupta o truncada devuelve `90`. (La validación estricta se puede
relajar mediante el indicador `strict_metadata` del motor.)

---

## 10. Referencia rápida — quién almacena qué

| Elemento                          | Dónde vive                             | Copias        |
|-------------------------------|----------------------------------------|-------------|
| Geometría de claves (desplazamientos/anchuras) | Esquema de claves de la cabecera (página 0) | una vez |
| Nombres de los campos de datos | Solo en el `FD` del programa          | no en el archivo |
| Bytes del registro            | Páginas `PT_DATA` / `PT_OVERFLOW`      | una/registro |
| clave → RecordId              | un árbol B+ por clave                  | uno/clave   |
| RecordId → ubicación física   | Directorio de RecordId (cadena `PT_DIR`) | una/registro |
| Páginas libres                | Lista de libres (`free_list_head`)     | —           |
| Inversas de cambios sin confirmar | Registro de deshacer en RAM        | por tx      |
```

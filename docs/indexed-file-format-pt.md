<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Formato de ficheiro indexado do PowerRustCOBOL (`PRCIDX1`)

Este documento descreve o contentor em disco que suporta os ficheiros
`ORGANIZATION IS INDEXED` no PowerRustCOBOL, e como ele corresponde aos metadados
de que um futuro **importador Fujitsu COBOL-85 → PowerRustCOBOL** irá precisar.

> **Não é compatível ao nível binário com a Fujitsu.** O `PRCIDX1` é o contentor
> autodescritivo do próprio PowerRustCOBOL. É modelado *semanticamente* sobre os
> metadados que as File Access Subroutines da Fujitsu expõem através de
> `cobfa_indexinfo()` (formato do registo, comprimento do registo, número e
> comprimento total das chaves, chave primária, chaves alternativas), mas **não**
> analisa nem reproduz os bytes de `cobidx`/`cobi64` da Fujitsu. O importador é
> trabalho futuro e vive fora do PowerRustCOBOL.

Implementação: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Porque é que o formato é autodescritivo

O contentor original (`PRCISAM1`) guardava apenas um número mágico, o comprimento
do registo e os bytes dos registos — **não levava esquema de chaves**. Um
conversor (ou qualquer ferramenta externa) não conseguia saber quais eram as
chaves sem o `FD` de COBOL.

O `PRCIDX1` embute o esquema completo no ficheiro: o formato do registo e, de
cada chave, a sua disposição em bytes, a ordenação, a política de duplicados e
(opcionalmente) o nome do campo COBOL. Isso torna o ficheiro **descobrível** —
ver [`inspect_path`](#api-de-descoberta) — e permite que um importador da Fujitsu
escreva um ficheiro PowerRustCOBOL fiel a partir dos metadados que lê de um
ficheiro Fujitsu, sem ter um `FD` correspondente à mão.

---

## Modelo de metadados

Estes tipos Rust (reexportados de `cobolt_runtime`) são o esquema. Espelham os
conceitos de `cobfa_indexinfo()`; todos os deslocamentos e comprimentos são **em
bytes** (nunca contagens de carateres — tal como a regra da Fujitsu em modo
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

O runtime atual emite chaves **de uma só parte, codificadas em `Bytes` e
`Ascending`** (é a isso que um `RECORD KEY` / `ALTERNATE RECORD KEY` de um `FD`
COBOL se resolve). Chaves compostas, codificações alternativas e ordem
descendente são **representáveis no formato**, para que um importador as possa
registar sem perdas; o suporte completo no runtime é trabalho futuro.

---

## Disposição do contentor

Todos os inteiros são **little-endian**. O ficheiro é:

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### Cabeçalho

| Campo            | Tipo      | Notas                                   |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | reservado (`0`)                         |
| `record_format`  | `u8`      | `1` = fixo, `2` = variável              |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | comprimento do registo quando é fixo    |
| `min_length`     | `u32`     | carga útil mínima quando é variável     |
| `max_length`     | `u32`     | carga útil máxima quando é variável     |
| `key_count`      | `u16`     | primária + alternativas                 |
| `created_unix_ms`| `u64`     | hora de criação, preservada entre reescritas|
| `updated_unix_ms`| `u64`     | hora da última escrita                  |

### Esquema de chaves — repetido `key_count` vezes (primária primeiro)

| Campo          | Tipo      | Notas                                   |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primária, `2..` alternativas        |
| `duplicates`   | `u8`      | `0`/`1`                                 |
| `ordering`     | `u8`      | `0` ascendente, `1` descendente         |
| `part_count`   | `u16`     | número de `KeyPart`                     |
| `name_len`     | `u16`     | comprimento do nome UTF-8 (`0` = nenhum)|
| `name`         | `[u8]`    | `name_len` bytes                        |
| `parts`        | repetido  | `part_count` × KeyPart (abaixo)         |

Cada **KeyPart**:

| Campo      | Tipo  | Notas                                   |
|------------|-------|-----------------------------------------|
| `offset`   | `u32` | deslocamento em bytes dentro da carga útil do registo|
| `length`   | `u32` | comprimento em bytes                    |
| `encoding` | `u8`  | discriminante de `KeyEncoding`          |
| `reserved` | `u8`  | `0`                                     |

### Registos

| Campo          | Tipo   | Notas                                   |
|----------------|--------|-----------------------------------------|
| `record_count` | `u64`  | número de registos vivos                |
| por registo    | repetido | `length: u32` e depois `length` bytes  |

Os registos são escritos por ordem ascendente de **chave primária**.

### Rodapé

| Campo   | Tipo  | Notas                                            |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, refletido) sobre todos os bytes anteriores ao rodapé |

O CRC é validado ao carregar; uma discrepância dá FILE STATUS `90` (erro de E/S).

---

## API de descoberta

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Devolve `Some(IndexedFileInfo)` para um ficheiro `PRCIDX1` e `None` para o
contentor antigo `PRCISAM1` (que não leva esquema). Este é o análogo de
`cobfa_indexinfo()` que um conversor ou uma ferramenta de inspeção pode chamar.

---

## Validação na abertura (FILE STATUS)

Ao abrir um ficheiro indexado **existente** para `INPUT` / `I-O`, o runtime valida
as chaves e o formato de registo declarados no `SELECT`/`FD` contra o esquema
guardado (modo estrito, ligado por omissão). Estados relevantes:

| Estado | Condição                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` de um ficheiro inexistente               |
| `39`   | esquema do ficheiro existente ≠ chaves ou formato de registo declarados |
| `90`   | contentor corrompido (CRC discrepante) ou outro erro de E/S |

O contentor antigo `PRCISAM1` não tem esquema, pelo que a validação estrita é
saltada para ele (carrega sempre de forma permissiva).

---

## Modos de armazenamento (`STORAGE IS MEMORY | DISK`)

A cláusula `STORAGE MODE` seleciona qual o motor — e portanto qual o contentor em
disco — que suporta um ficheiro INDEXED. **O modo de armazenamento por omissão é
`DISK`** (quando não há cláusula `STORAGE`). O `WITH COMPRESSION` aplica-se a
qualquer um dos modos; o `WITH PERSISTENCE` aplica-se apenas a `MEMORY`.

| Modo | Motor | Contentor | Notas |
|------|--------|-----------|-------|
| `MEMORY` | `BTreeMap` em RAM (`indexed.rs`) | `PRCIDX1` (este documento) | o ficheiro inteiro em memória; **efémero por omissão** — o `COMMIT` nunca escreve em disco. Com `WITH PERSISTENCE`, é guardado como `PRCIDX1` apenas no `CLOSE`. O `OPEN OUTPUT` (re)cria sempre o contentor. |
| `DISK` (por omissão) | armazém redb à prova de falhas (`indexed_redb.rs`) desde a 1.62.73; o B+tree paginado (`indexed_disk.rs`) com `--indexed-engine rust` | o do próprio redb, ou `PRCIDXD1` para o motor paginado | registos e índices lidos a pedido; RAM limitada; sempre persistente (escritas por operação, `fsync` no `COMMIT`/`CLOSE`) |

O contentor de disco **`PRCIDXD1`** é um único ficheiro paginado (páginas de
4 KiB):

* **página 0** — cabeçalho: raízes (um B+tree por chave), cabeça da lista livre,
  próximo identificador de página, contador de `RecordId`, número de registos, o
  esquema de chaves e a marca de compressão.
* **páginas de B+tree** — nós internos e folha (empacotados em bytes de tamanho
  variável, com divisão na inserção e folhas duplamente ligadas para varrimentos
  ordenados).
* **páginas de dados** — células de registo com ranhuras (vários registos por
  página), mais uma cadeia de páginas de transbordo para registos maiores do que
  uma página.
* **páginas de diretório** — o mapa `RecordId` → localização física.
* uma **lista livre** encadeia as páginas libertadas para reutilização.

O `WITH COMPRESSION` (`compress.rs`) é um RLE ao estilo PackBits sem dependências,
aplicado a cada registo guardado (`PRCIDXD1`) ou a cada registo da secção de
registos (`PRCIDX1`); uma etiqueta de um byte garante que a codificação nunca
cresce, e o cabeçalho do contentor regista que a compressão está ligada.

> O `PRCIDXD1` destina-se ao armazenamento nativo em modo DISK. Os metadados
> descobríveis e orientados à importação da Fujitsu descritos acima são os do
> contentor `PRCIDX1` (modo MEMORY); um importador deve visar o `PRCIDX1`, a menos
> que precise especificamente da disposição paginada em disco.

## Compatibilidade retroativa

* `PRCIDX1` (número mágico `PRCIDX1\0`) — o formato autodescritivo atual de modo
  MEMORY (leitura + escrita).
* `PRCIDXD1` (número mágico `PRCIDXD1`) — contentor paginado de B+tree em modo
  DISK.
* `PRCISAM1` (número mágico `PRCISAM1`) — contentor antigo só com registos
  (apenas leitura; volta a ser guardado como `PRCIDX1` no `CLOSE` seguinte de uma
  abertura com escrita).
* Qualquer outro conteúdo — tratado como um ficheiro vazio.

---

## Futuro caminho de importação da Fujitsu

O fluxo de migração previsto (hoje, todo ele fora do âmbito do PowerRustCOBOL):

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

Como o `PRCIDX1` já consegue *representar* chaves compostas, codificações de
chave, ordenação de chave, política de duplicados, limites de registos de
comprimento variável e nomes dos campos-chave, ao conversor resta apenas traduzir
os metadados da Fujitsu para `IndexedFileInfo` e debitar os registos — nenhuma
alteração de formato do PowerRustCOBOL é necessária.

**Não** tente analisar os bytes em bruto de `cobidx`/`cobi64` da Fujitsu. A
documentação pública da Fujitsu expõe os metadados através das File Access
Subroutines, mas não publica a disposição física dos bytes.

.<<

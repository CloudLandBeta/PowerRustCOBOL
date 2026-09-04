<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Formato de arquivo indexado do PowerRustCOBOL (`PRCIDX1`)

Este documento descreve o contêiner em disco que dá suporte aos arquivos
`ORGANIZATION IS INDEXED` no PowerRustCOBOL e como ele se relaciona com os
metadados de que um futuro **importador Fujitsu COBOL-85 → PowerRustCOBOL** vai
precisar.

> **Não é compatível em nível binário com a Fujitsu.** O `PRCIDX1` é o contêiner
> autodescritivo do próprio PowerRustCOBOL. Ele é modelado *semanticamente* nos
> metadados que as File Access Subroutines da Fujitsu expõem por meio de
> `cobfa_indexinfo()` (formato de registro, comprimento de registro, quantidade
> e comprimento total das chaves, chave primária, chaves alternativas), mas
> **não** interpreta nem reproduz os bytes `cobidx`/`cobi64` da Fujitsu. O
> importador é trabalho futuro e fica fora do PowerRustCOBOL.

Implementação: [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Por que o formato é autodescritivo

O contêiner original (`PRCISAM1`) armazenava apenas um número mágico, o
comprimento do registro e os bytes do registro — ele **não carregava nenhum
esquema de chaves**. Um conversor (ou qualquer ferramenta externa) não tinha
como saber quais eram as chaves sem o `FD` do COBOL.

O `PRCIDX1` embute o esquema completo no arquivo: o formato de registro e, para
cada chave, seu layout de bytes, sua ordenação, sua política de duplicatas e
(opcionalmente) o nome do campo COBOL. Isso torna o arquivo **descobrível** —
veja [`inspect_path`](#api-de-descoberta) — e permite que um importador da
Fujitsu escreva um arquivo PowerRustCOBOL fiel a partir dos metadados que lê de
um arquivo Fujitsu, sem ter um `FD` correspondente à mão.

---

## Modelo de metadados

Estes tipos Rust (reexportados de `cobolt_runtime`) são o esquema. Eles espelham
os conceitos de `cobfa_indexinfo()`; todos os deslocamentos e comprimentos são
**expressos em bytes** (nunca em contagem de caracteres — conforme a regra da
Fujitsu para o modo Unicode).

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

O runtime atual emite chaves **de uma única parte, codificadas como `Bytes` e
`Ascending`** (é nisso que um `RECORD KEY` / `ALTERNATE RECORD KEY` de um `FD`
COBOL se resolve). Chaves compostas, outras codificações e a ordem decrescente
são **representáveis no formato**, de modo que um importador pode registrá-las
sem perdas; o suporte completo no runtime é trabalho futuro.

---

## Layout do contêiner

Todos os inteiros são **little-endian**. O arquivo é:

```text
┌────────────────────────────────────────────────────────────┐
│ Cabeçalho                                                  │
│ Esquema de chaves (key_count descritores: primária+alts)   │
│ Registros                                                  │
│ Trailer CRC-32 (sobre todos os bytes anteriores)           │
└────────────────────────────────────────────────────────────┘
```

### Cabeçalho

| Campo            | Tipo      | Observações                             |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | reservado (`0`)                         |
| `record_format`  | `u8`      | `1` = fixo, `2` = variável              |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | comprimento do registro quando fixo     |
| `min_length`     | `u32`     | carga útil mínima quando variável       |
| `max_length`     | `u32`     | carga útil máxima quando variável       |
| `key_count`      | `u16`     | primária + alternativas                 |
| `created_unix_ms`| `u64`     | data de criação, preservada entre reescritas|
| `updated_unix_ms`| `u64`     | data da última escrita                  |

### Esquema de chaves — repetido `key_count` vezes (a primária primeiro)

| Campo          | Tipo      | Observações                             |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primária, `2..` alternativas        |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` crescente, `1` decrescente          |
| `part_count`   | `u16`     | quantidade de `KeyPart`                 |
| `name_len`     | `u16`     | comprimento do nome UTF-8 (`0` = nenhum)|
| `name`         | `[u8]`    | `name_len` bytes                        |
| `parts`        | repetido  | `part_count` × KeyPart (abaixo)         |

Cada **KeyPart**:

| Campo      | Tipo  | Observações                    |
|------------|-------|--------------------------------|
| `offset`   | `u32` | deslocamento em bytes dentro da carga útil|
| `length`   | `u32` | comprimento em bytes           |
| `encoding` | `u8`  | discriminante de `KeyEncoding` |
| `reserved` | `u8`  | `0`                            |

### Registros

| Campo          | Tipo     | Observações                            |
|----------------|----------|----------------------------------------|
| `record_count` | `u64`    | quantidade de registros vivos          |
| por registro   | repetido | `length: u32` e então `length` bytes   |

Os registros são gravados em ordem crescente de **chave primária**.

### Trailer

| Campo   | Tipo  | Observações                                      |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, refletido) sobre todos os bytes anteriores ao trailer |

O CRC é validado no carregamento; uma divergência resulta em FILE STATUS `90`
(erro de E/S).

---

## API de descoberta

```rust
use cobolt_runtime::IndexedFile; // (engine type)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Retorna `Some(IndexedFileInfo)` para um arquivo `PRCIDX1` e `None` para o
contêiner legado `PRCISAM1` (que não carrega esquema). Esse é o análogo de
`cobfa_indexinfo()` que um conversor ou uma ferramenta de inspeção pode chamar.

---

## Validação na abertura (FILE STATUS)

Ao abrir um arquivo indexado **existente** para `INPUT` / `I-O`, o runtime valida
as chaves declaradas em `SELECT`/`FD` e o formato de registro contra o esquema
armazenado (modo estrito, ligado por padrão). Status relevantes:

| Status | Condição                                               |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` de um arquivo inexistente                |
| `39`   | esquema do arquivo existente ≠ chaves/formato de registro declarados |
| `90`   | contêiner corrompido (divergência de CRC) ou outro erro de E/S |

O contêiner legado `PRCISAM1` não tem esquema, então a validação estrita é
ignorada para ele (ele sempre é carregado de forma tolerante).

---

## Modos de armazenamento (`STORAGE IS MEMORY | DISK`)

A cláusula `STORAGE MODE` seleciona qual motor — e, portanto, qual contêiner em
disco — dá suporte a um arquivo INDEXED. **O modo de armazenamento padrão é
`DISK`** (quando não há cláusula `STORAGE`). `WITH COMPRESSION` vale para
qualquer um dos modos; `WITH PERSISTENCE` vale somente para `MEMORY`.

| Modo | Motor | Contêiner | Observações |
|------|-------|-----------|-------------|
| `MEMORY` | `BTreeMap` em RAM (`indexed.rs`) | `PRCIDX1` (este documento) | arquivo inteiro na memória; **efêmero por padrão** — `COMMIT` nunca grava em disco. Com `WITH PERSISTENCE`, é salvo em `PRCIDX1` apenas no `CLOSE`. `OPEN OUTPUT` sempre (re)cria o contêiner. |
| `DISK` (padrão) | árvore B+ paginada e persistente (`indexed_disk.rs`) | `PRCIDXD1` | registros + índices lidos sob demanda; RAM limitada; sempre persistente (gravações por operação, `fsync` no `COMMIT`/`CLOSE`) |

O contêiner de disco **`PRCIDXD1`** é um único arquivo paginado (páginas de
4 KiB):

* **página 0** — cabeçalho: as raízes (uma árvore B+ por chave), o início da
  lista de livres, o id da próxima página, o contador de `RecordId`, a
  quantidade de registros, o esquema de chaves e o sinalizador de compressão.
* **páginas de árvore B+** — nós internos / folha (empacotados em bytes de
  tamanho variável, divididos na inserção, com folhas duplamente encadeadas para
  varreduras ordenadas).
* **páginas de dados** — células de registro com slots (vários registros por
  página), mais uma cadeia de páginas de estouro para registros maiores que uma
  página.
* **páginas de diretório** — o mapa `RecordId` → localização física.
* uma **lista de livres** encadeia as páginas liberadas para reutilização.

`WITH COMPRESSION` (`compress.rs`) é um RLE no estilo PackBits, sem
dependências, aplicado a cada registro armazenado (`PRCIDXD1`) ou a cada
registro da seção de registros (`PRCIDX1`); um marcador de um byte garante que a
codificação nunca cresça, e o cabeçalho do contêiner registra que a compressão
está ativa.

> O `PRCIDXD1` serve ao armazenamento nativo do modo DISK. Os metadados
> descobríveis e voltados à importação da Fujitsu descritos acima pertencem ao
> contêiner `PRCIDX1` (modo MEMORY); um importador deve mirar no `PRCIDX1` a
> menos que precise especificamente do layout paginado em disco.

## Compatibilidade retroativa

* `PRCIDX1` (número mágico `PRCIDX1\0`) — formato autodescritivo atual do modo
  MEMORY (leitura + escrita).
* `PRCIDXD1` (número mágico `PRCIDXD1`) — contêiner de árvore B+ paginada do
  modo DISK.
* `PRCISAM1` (número mágico `PRCISAM1`) — contêiner legado apenas com registros
  (somente leitura; regravado como `PRCIDX1` no próximo `CLOSE` de uma abertura
  com escrita).
* Qualquer outro conteúdo — tratado como um arquivo vazio.

---

## Futuro caminho de importação da Fujitsu

O fluxo de migração pretendido (tudo fora do escopo do PowerRustCOBOL hoje):

```text
runtime da Fujitsu
  └─ cobfa_indexinfo()  → formato de registro, comprimento de registro, lista de chaves (primária + alternativas)
  └─ exportação sequencial → cargas úteis dos registros
        │
        ▼
  conversor (futuro, externo)
        │  monta IndexedFileInfo + registros
        ▼
  arquivo PRCIDX1  → aberto nativamente pelo PowerRustCOBOL
```

Como o `PRCIDX1` já consegue *representar* chaves compostas, codificações de
chave, ordenação de chaves, política de duplicatas, limites de registros de
comprimento variável e nomes de campos-chave, o conversor só precisa traduzir os
metadados da Fujitsu para `IndexedFileInfo` e transmitir os registros — nenhuma
mudança de formato no PowerRustCOBOL é necessária.

**Não** tente interpretar os bytes `cobidx`/`cobi64` brutos da Fujitsu. A
documentação pública da Fujitsu expõe os metadados por meio das File Access
Subroutines, mas não publica o layout físico dos bytes.

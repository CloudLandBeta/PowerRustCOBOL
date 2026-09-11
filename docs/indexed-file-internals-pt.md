<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.134 -->

# Interioridades dos ficheiros indexados do PowerRustCOBOL (motor paginado `PRCIDXD1`)

Este documento é um esquema conceptual do motor **persistente e paginado em
disco** que suporta os ficheiros `ORGANIZATION IS INDEXED` declarados com
`STORAGE IS DISK` (o valor por omissão). É um desenho de B+tree com páginas
ranhuradas que lê os registos a pedido, para que a RAM fique limitada
independentemente do tamanho do ficheiro.

> ⚠️ **Este já não é o motor por omissão.** O `STORAGE IS DISK` continua a ser o
> *modo de armazenamento* por omissão, mas desde a **1.62.73** o motor que o
> serve é o **redb** (`crates/cobolt-runtime/src/indexed.rs:126`) — ver
> [`indexed-redb-engine-pt.md`](indexed-redb-engine-pt.md). Tudo o que se segue
> continua exato para o motor paginado, que ainda se alcança com
> `--indexed-engine rust`; simplesmente não é o que um programa recebe por
> omissão.
>
> **Âmbito.** Isto descreve o *motor físico* (`DiskIndexedFile`, número mágico do
> contentor `PRCIDXD1`). É um artefacto diferente do contentor `PRCIDX1`, de
> bloco único e autodescritivo, documentado em
> [`indexed-file-format-pt.md`](indexed-file-format-pt.md), que modela os
> metadados de que um futuro importador da Fujitsu precisa. O motor em memória
> (`STORAGE IS MEMORY`, `IndexedFile`) é um subconjunto simplificado do mesmo
> modelo lógico (BTreeMap em vez de B+trees em disco).
>
> Um segundo motor `STORAGE IS DISK`, **à prova de falhas** (opcional, sobre o
> armazém ACID redb escrito em Rust puro), resolve o diretório limitado pela RAM
> e a persistência só-no-CLOSE deste motor — ver
> [`indexed-redb-engine-pt.md`](indexed-redb-engine-pt.md).

Implementação:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
e a (des)materialização dos registos em
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. O desenho numa frase

Um ficheiro paginado com **uma página de cabeçalho + N B+trees (um por chave) →
um diretório de RecordId → páginas de dados ranhuradas com imagens de registo
posicionais e de largura fixa**, mais uma lista livre, cadeias de transbordo,
compressão RLE opcional e um registo de desfazer em memória para as transações.

---

## 2. O ficheiro é um vetor de páginas fixas de 4 KiB

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

Todas as páginas **posteriores** à página 0 identificam-se pelo seu primeiro byte
(a etiqueta de tipo de página). As páginas libertadas são recicladas através de
uma lista livre, pelo que a ordem física das páginas em disco **não** acompanha a
ordem lógica dos registos.

| Etiqueta | Constante     | A página contém                               |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | nó interno (de encaminhamento) do B+tree      |
| `2` | `PT_LEAF`     | nó folha do B+tree (duplamente ligado aos irmãos) |
| `3` | `PT_DATA`     | página ranhurada que empacota várias imagens de registo |
| `4` | `PT_OVERFLOW` | continuação de um registo demasiado grande para caber em linha |
| `5` | `PT_DIR`      | um troço do diretório de RecordId             |

---

## 3. Página 0 — o cabeçalho

A página 0 é o único sítio onde um *esquema* é guardado, e é escrita uma única
vez. Os campos são little-endian, por esta ordem:

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

| Campo do cabeçalho | Significado                                                  |
|-------------------|---------------------------------------------------------------|
| `version`         | Versão do formato (atualmente `1`).                           |
| `page_size`       | Tamanho da página em bytes (4096).                            |
| `rec_fmt`         | Formato do registo: `1` = comprimento fixo.                   |
| `compressing`     | `1` se as cargas úteis dos registos são comprimidas com RLE em disco. |
| `record_len`      | Comprimento lógico (não comprimido) do registo, em bytes.     |
| `next_page_id`    | Próximo identificador de página a atribuir quando a lista livre está vazia. |
| `free_list_head`  | Primeira página da lista livre de páginas recuperadas (`0` = nenhuma). |
| `record_count`    | Número de registos vivos.                                     |
| `data_tail`       | Página `PT_DATA` atual que aceita escritas em linha (`0` = nenhuma). |
| `primary_root`    | Página raiz do B+tree da chave primária.                      |
| `dir_head`        | Primeira página `PT_DIR` do diretório de RecordId (`0` = nenhuma). |
| `directory_len`   | Número de entradas do diretório (RecordId alguma vez atribuídos). |
| `alt_root[k]`     | Página raiz do B+tree da chave alternativa *k*.               |
| ESQUEMA DE CHAVES | Política de duplicados por chave e intervalos de bytes das partes compostas. |

**O que deliberadamente *não* está no cabeçalho:** não há **nomes de campos de
dados** nem **metadados por registo**. O esquema é puramente *geometria de
chaves* (intervalos de bytes). Tudo o resto acerca de um registo é posicional —
ver a §6.

---

## 4. O caminho de acesso (como se resolve um `READ` por chave)

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

**Um registo, muitas chaves.** A chave primária e todas as alternativas apontam
para o *mesmo* RecordId, pelo que existe exatamente uma cópia guardada de cada
registo. Os índices alternativos são apenas B+trees adicionais sobrepostos ao
diretório de RecordId partilhado; um valor alternativo duplicado é permitido
quando essa chave foi declarada `WITH DUPLICATES`.

---

## 5. Interior das páginas

### 5.1 Nó do B+tree (`PT_INTERNAL` / `PT_LEAF`)

Um nó é carregado para memória para uma operação, alterado, dividido se for
preciso, e escrito de volta.

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- As folhas estão **duplamente ligadas** (`next`/`prev`), pelo que um varrimento
  ordenado após um `START` percorre os irmãos diretamente — é esse o `READ NEXT`
  por chave ascendente do RustCOBOL.
- A inserção **divide no transbordo** quando o nó serializado ultrapassaria o
  `PAGE_SIZE`; a chave mediana sobe para o pai.
- Os nós internos contêm `child0` mais pares *(chave separadora, filho)*.

### 5.2 Página de dados ranhurada (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- Cabeçalho de página de 5 bytes e, a seguir, um **diretório de ranhuras** que
  cresce a partir da frente, enquanto as **cargas úteis dos registos** crescem a
  partir do fim; um registo cabe em linha enquanto as duas regiões não se
  encontrarem.
- Uma ranhura é `(offset, len)`; apagar um registo põe a sua ranhura a `len = 0`
  (lápide). Quando todas as ranhuras de uma página estão livres, a página inteira
  volta para a lista livre.
- O campo `slot` de um `RecLoc` indexa dentro deste diretório de ranhuras.

### 5.3 Cadeia de transbordo (`PT_OVERFLOW`)

Um registo maior do que o limite em linha (`PAGE_SIZE − cabeçalho − uma ranhura`)
é guardado como uma cadeia ligada de páginas de transbordo; o seu
`RecLoc.kind = 2` e `page` aponta para a cabeça da cadeia.

### 5.4 Diretório de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

O diretório é mantido em RAM como um `Vec<RecLoc>` enquanto o ficheiro está
aberto (por isso procurar um RecordId é um índice O(1)) e é persistido como uma
cadeia de páginas `PT_DIR` (a começar em `dir_head`) ao fechar. Os B+trees
guardam RecordId, nunca endereços físicos, pelo que um registo pode ser movido em
disco sem tocar em qualquer índice.

---

## 6. A própria imagem do registo (posicional, sem nomes)

Um registo em disco é um único **buffer de bytes de largura fixa** disposto por
*deslocamento* de campo — não há nomes de campo, etiquetas ou delimitadores na
carga útil. Para:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

a imagem guardada ocupa 23 bytes:

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- O `RecordLayout::materialize()` empacota os itens elementares do `FD` neste
  buffer por deslocamento, para `WRITE`/`REWRITE`; o
  `RecordLayout::distribute()` inverte-o no `READ`. O mapa campo →
  deslocamento vive apenas no `RecordLayout` do programa (derivado do `FD`),
  **nunca** no ficheiro.
- **A identidade é a posição.** Este é o caso-limite de «não repetir as chaves em
  cada registo»: a identidade de um campo custa *zero* bytes por registo, e o
  acesso a um campo é O(1) por deslocamento pré-calculado (sem análise).
  Renomear um campo que não é chave não muda nada em disco; renomear um campo
  chave reescreve apenas o esquema de chaves do cabeçalho, não os registos nem os
  índices. Alterar o deslocamento ou a largura de um campo é a única alteração que
  obriga a reescrever os dados — algo inerente aos registos de comprimento fixo
  (e aos ISAM/VSAM a sério).

### Compressão

Com `STORAGE IS DISK WITH COMPRESSION`, a carga útil **guardada** vai comprimida
com RLE PackBits (`compress.rs`), e o `RecLoc.len` é o comprimento *guardado*; o
buffer é expandido de volta para `record_len` na leitura. A compressão é
transparente para a geometria das chaves e para o caminho de acesso.

---

## 7. Espaço livre e reutilização

- **Lista livre.** O `free_list_head` encadeia páginas recuperadas de páginas de
  dados esvaziadas, de nós órfãos após uma divisão, etc.; o `allocate` retira
  dela antes de incrementar o `next_page_id`, pelo que o espaço é reutilizado e o
  ficheiro não cresce monotonicamente.
- **Lápides.** Um `DELETE` liberta a ranhura (e, preguiçosamente, a página de
  dados) e marca a entrada do diretório como `RecLoc::FREE`; o RecordId é
  reformado.

---

## 8. Transações (registo de desfazer em memória)

O motor de disco mantém um **registo de desfazer** com as inversas de cada
alteração desde o último `COMMIT`/`OPEN`:

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- O `OPEN` inicia uma transação (limpa o registo); o `COMMIT` torna as alterações
  duráveis e inicia outra; o `ROLLBACK` reproduz as inversas por ordem inversa; o
  `CLOSE` descarrega (confirmação implícita). Uma guarda `tx_replay` impede que
  as operações inversas se registem a si próprias.
- Isto é reversão **ao nível do programa**. A recuperação após falha através de
  um registo de escrita antecipada durável é trabalho futuro. Ver os verbos COBOL
  `COMMIT`/`ROLLBACK` na referência da linguagem; note que esses verbos atuam
  sobre **ficheiros INDEXED**, não sobre ligações SQL.

---

## 9. Validação na abertura

No `OPEN`, o esquema de chaves guardado no cabeçalho é comparado com o `SELECT`
do programa (comprimento do registo, número de chaves, as partes de cada chave e a
sua política de duplicados). Uma divergência devolve o estado de ficheiro COBOL
`39`; um ficheiro inexistente aberto como `INPUT` devolve `35`; um cabeçalho
corrompido ou truncado devolve `90`. (A validação estrita pode ser relaxada
através da marca `strict_metadata` do motor.)

---

## 10. Referência rápida — quem guarda o quê

| Coisa                         | Onde vive                              | Cópias      |
|-------------------------------|----------------------------------------|-------------|
| Geometria das chaves (deslocamentos/larguras) | Esquema de chaves do cabeçalho (página 0) | uma |
| Nomes dos campos de dados     | Apenas o `FD` do programa              | não está no ficheiro |
| Bytes dos registos            | Páginas `PT_DATA` / `PT_OVERFLOW`       | uma por registo |
| chave → RecordId              | um B+tree por chave                    | um por chave |
| RecordId → localização física | Diretório de RecordId (cadeia `PT_DIR`) | uma por registo |
| Páginas livres                | Lista livre (`free_list_head`)         | —           |
| Inversas de alterações por confirmar | Registo de desfazer em RAM       | por transação |

.<<

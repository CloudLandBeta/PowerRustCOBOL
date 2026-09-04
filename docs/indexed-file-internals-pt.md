<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Funcionamento interno do arquivo indexado do PowerRustCOBOL (motor paginado `PRCIDXD1`)

Este documento é um esquema conceitual do motor **persistente e paginado em
disco** que sustenta os arquivos `ORGANIZATION IS INDEXED` declarados com
`STORAGE IS DISK` (o padrão). É um projeto de árvore B+ / páginas com slots que
lê os registros sob demanda, de modo que a RAM permanece limitada
independentemente do tamanho do arquivo.

> **Escopo.** Aqui se descreve o *motor físico* (`DiskIndexedFile`, número mágico
> de contêiner `PRCIDXD1`). É um artefato diferente do contêiner `PRCIDX1`,
> autodescritivo e de blob único, documentado em
> [`indexed-file-format-en.md`](indexed-file-format-pt.md), que modela os metadados de que
> um futuro importador Fujitsu precisará. O motor em memória
> (`STORAGE IS MEMORY`, `IndexedFile`) é um subconjunto simplificado do mesmo
> modelo lógico (BTreeMaps em vez de árvores B+ em disco).
>
> Um segundo motor `STORAGE IS DISK`, **à prova de falhas** (opcional, sobre o
> armazenamento ACID redb em Rust puro), resolve o diretório limitado pela RAM e
> a persistência apenas-no-CLOSE deste motor — veja
> [`indexed-redb-engine-pt.md`](indexed-redb-engine-pt.md).

Implementação:
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
(des)materialização de registros em
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. O projeto em uma frase

Um arquivo paginado formado por **uma página de cabeçalho + N árvores B+ (uma por
chave) → um diretório de RecordId → páginas de dados com slots contendo imagens
de registro posicionais e de largura fixa**, com uma lista de livres, cadeias de
overflow, compressão RLE opcional e um log de desfazer válido durante a execução
para as transações.

---

## 2. O arquivo é um vetor de páginas fixas de 4 KiB

```
 byte 0                                                    fim do arquivo
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixo).   id da página = deslocamento / 4096.
```

Toda página **posterior** à página 0 identifica a si mesma pelo seu primeiro byte
(a etiqueta de tipo de página). As páginas liberadas são recicladas por meio de
uma lista de livres, portanto a ordem física das páginas em disco **não**
acompanha a ordem lógica dos registros.

| Etiqueta | Constante   | A página contém                                     |
|-----|---------------|--------------------------------------------------------|
| `1` | `PT_INTERNAL` | nó interno (de roteamento) da árvore B+                 |
| `2` | `PT_LEAF`     | nó folha da árvore B+ (duplamente ligado aos irmãos)    |
| `3` | `PT_DATA`     | página com slots que empacota várias imagens de registro |
| `4` | `PT_OVERFLOW` | continuação de um registro grande demais para caber em linha |
| `5` | `PT_DIR`      | uma fatia do diretório de RecordId                      |

---

## 3. Página 0 — o cabeçalho

A página 0 é o único lugar onde um *esquema* é armazenado, e ela é escrita uma
única vez. Os campos são little-endian, nesta ordem:

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (cada um u64)
 primary_root   dir_head         directory_len                 (cada um u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (uma raiz B+ por chave alt.)
 ──────────────────────────────────────────────────────────────────────
 ESQUEMA DE CHAVES:  key_count (u16) → para cada chave (primária primeiro, depois alternadas):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × partes   (partes de chave composta)
```

| Campo do cabeçalho | Significado                                              |
|-------------------|---------------------------------------------------------|
| `version`         | Versão do formato (atualmente `1`).                     |
| `page_size`       | Tamanho da página em bytes (4096).                      |
| `rec_fmt`         | Formato do registro: `1` = comprimento fixo.            |
| `compressing`     | `1` se as cargas dos registros são comprimidas com RLE em disco. |
| `record_len`      | Comprimento lógico (não comprimido) do registro em bytes. |
| `next_page_id`    | Próximo id de página a alocar quando a lista de livres está vazia. |
| `free_list_head`  | Primeira página da lista de livres de páginas recuperadas (`0` = nenhuma). |
| `record_count`    | Número de registros vivos.                              |
| `data_tail`       | Página `PT_DATA` atual que aceita escritas em linha (`0` = nenhuma). |
| `primary_root`    | Página raiz da árvore B+ da chave primária.             |
| `dir_head`        | Primeira página `PT_DIR` do diretório de RecordId (`0` = nenhuma). |
| `directory_len`   | Número de entradas do diretório (RecordId já alocados). |
| `alt_root[k]`     | Página raiz da árvore B+ da chave alternada *k*.        |
| ESQUEMA DE CHAVES | Política de duplicatas por chave + faixas de bytes das partes compostas. |

**O que deliberadamente *não* está no cabeçalho:** não há **nomes de campos de
dados** nem **metadados por registro**. O esquema é puramente *geometria de
chaves* (faixas de bytes). Todo o resto de um registro é posicional — veja §6.

---

## 4. O caminho de acesso (como um `READ` por chave é resolvido)

```
  valor da chave COBOL (bytes)
        │
        ▼
  ┌──────────────┐   Começa em primary_root (READ aleatório por RECORD KEY) ou
  │  B+tree      │   em alt_roots[k] (READ KEY IS <alt>). Os nós internos
  │  (um por     │   roteiam por chave; as folhas guardam (key_bytes →
  │  chave)      │   RecordId) e são duplamente ligadas (next/prev) para
  └──────┬───────┘   READ NEXT / READ PREVIOUS / START.
         │  RecordId (um inteiro estável, independente da localização física)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = livre/lápide, 1 = em linha, 2 = cabeça de overflow
  │  diretório   │     len : comprimento em bytes armazenado (talvez comprimido)
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Página DATA com slots → diretório de slots →
  │  página DATA │   (offset, len) → imagem bruta do registro (descomprimida
  └──────┬───────┘   se `compressing`).
         ▼
  os bytes do registro de largura fixa
        │  RecordLayout.distribute()
        ▼
  espalhados pelos itens elementares do FD na memória de trabalho
```

**Um registro, muitas chaves.** A chave primária e todas as alternadas apontam
para o *mesmo* RecordId, portanto existe exatamente uma cópia armazenada de cada
registro. Os índices alternados são apenas árvores B+ adicionais sobrepostas ao
diretório de RecordId compartilhado; um valor alternado duplicado é permitido
quando aquela chave foi declarada `WITH DUPLICATES`.

---

## 5. Interior das páginas

### 5.1 Nó de árvore B+ (`PT_INTERNAL` / `PT_LEAF`)

Um nó é carregado na memória para uma operação, alterado, dividido se necessário
e escrito de volta.

```
 Folha:     type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Interno:   type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- As folhas são **duplamente ligadas** (`next`/`prev`), de modo que uma varredura
  ordenada após um `START` percorre os irmãos diretamente — é esse o `READ NEXT`
  de chave ascendente do RustCOBOL.
- A inserção **divide no estouro** quando o nó serializado ultrapassaria
  `PAGE_SIZE`; a chave mediana é promovida ao pai.
- Os nós internos contêm `child0` mais pares *(chave separadora, filho)*.

### 5.2 Página de dados com slots (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ dir. de slots ───────┬─ livre ┬─ dados reg. ──┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  compactados  │
 │          │ count   │ top     │ cresce →              │        │  ←  crescem   │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- Cabeçalho de página de 5 bytes e, em seguida, um **diretório de slots** que
  cresce a partir do início, enquanto as **cargas dos registros** crescem a
  partir do fim; um registro cabe em linha enquanto as duas regiões não se
  encontrarem.
- Um slot é `(offset, len)`; apagar um registro define o `len = 0` do seu slot
  (lápide). Quando todos os slots de uma página estão livres, a página inteira é
  devolvida à lista de livres.
- O campo `slot` de um `RecLoc` indexa dentro deste diretório de slots.

### 5.3 Cadeia de overflow (`PT_OVERFLOW`)

Um registro maior do que o limite em linha (`PAGE_SIZE − cabeçalho − um slot`) é
armazenado como uma cadeia ligada de páginas de overflow; seu `RecLoc.kind = 2` e
`page` aponta para a cabeça da cadeia.

### 5.4 Diretório de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entrada)
```

O diretório é mantido na RAM como um `Vec<RecLoc>` enquanto o arquivo está aberto
(de modo que consultar um RecordId é um índice O(1)) e é persistido no fechamento
como uma cadeia de páginas `PT_DIR` (começando em `dir_head`). As árvores B+
armazenam RecordId, nunca endereços físicos, portanto um registro pode ser movido
em disco sem tocar em nenhum índice.

---

## 6. A imagem do registro em si (posicional, sem nomes)

Um registro em disco é um único **buffer de bytes de largura fixa** disposto por
*deslocamento* de campo — não há nomes de campo, etiquetas nem delimitadores na
carga. Para:

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

a imagem armazenada tem 23 bytes:

```
 deslocamento: 0        5                     15              23
               ┌────────┬─────────────────────┬───────────────┐
 carga útil:   │ 00001  │ John Doe░░          │ Sao Paulo     │
               └────────┴─────────────────────┴───────────────┘
                 ID(5)     NAME(10)              CITY(8)
                 (░ = preenchimento com espaços)
```

- `RecordLayout::materialize()` empacota os itens elementares do FD neste buffer
  por deslocamento para `WRITE`/`REWRITE`; `RecordLayout::distribute()` faz o
  inverso no `READ`. O mapa campo → deslocamento vive apenas no `RecordLayout` do
  programa (derivado do `FD`), **nunca** no arquivo.
- **A identidade é a posição.** Este é o caso-limite de "não repetir as chaves em
  cada registro": a identidade de um campo custa *zero* bytes por registro e o
  acesso ao campo é O(1) por deslocamento pré-calculado (sem análise sintática).
  Renomear um campo que não é chave não muda nada em disco; renomear um campo
  chave reescreve apenas o esquema de chaves do cabeçalho, não os registros nem
  os índices. Mudar o deslocamento ou a largura de um campo é a única mudança que
  exige reescrever os dados — algo inerente aos registros de comprimento fixo (e
  ao ISAM/VSAM reais).

### Compressão

Com `STORAGE IS DISK WITH COMPRESSION`, a carga **armazenada** é comprimida com
PackBits-RLE (`compress.rs`), e `RecLoc.len` é o comprimento *armazenado*; o
buffer é expandido de volta para `record_len` na leitura. A compressão é
transparente para a geometria das chaves e para o caminho de acesso.

---

## 7. Espaço livre e reaproveitamento

- **Lista de livres.** `free_list_head` encadeia as páginas recuperadas de
  páginas de dados esvaziadas, nós órfãos por uma divisão etc.; `allocate`
  desempilha dela antes de incrementar `next_page_id`, de modo que o espaço é
  reaproveitado e o arquivo não cresce monotonicamente.
- **Lápides.** Um `DELETE` libera o slot (e, preguiçosamente, a página de dados) e
  marca a entrada do diretório como `RecLoc::FREE`; o RecordId é aposentado.

---

## 8. Transações (log de desfazer em execução)

O motor de disco mantém um **log de desfazer** com as inversas de cada mutação
desde o último `COMMIT`/`OPEN`:

```
 DiskUndo::Insert(key)        ← um WRITE   → desfeito apagando aquela chave
 DiskUndo::Update(prev_image) ← um REWRITE → desfeito reescrevendo a imagem anterior
 DiskUndo::Delete(prev_image) ← um DELETE  → desfeito escrevendo a imagem de volta
```

- `OPEN` inicia uma transação (limpa o log); `COMMIT` torna as mudanças duráveis
  e inicia outra; `ROLLBACK` reexecuta as inversas em ordem reversa; `CLOSE`
  descarrega em disco (commit implícito). Uma guarda `tx_replay` impede que as
  operações inversas se registrem novamente.
- Isto é desfazimento **em nível de programa**. A recuperação após uma falha por
  meio de um log de escrita antecipada durável é trabalho futuro. Veja os verbos
  COBOL `COMMIT`/`ROLLBACK` na referência da linguagem; note que esses verbos
  atuam sobre **arquivos INDEXED**, não sobre conexões SQL.

---

## 9. Validação no OPEN

No `OPEN`, o esquema de chaves armazenado no cabeçalho é comparado com o `SELECT`
do programa (comprimento do registro, quantidade de chaves, as partes de cada
chave e sua política de duplicatas). Uma divergência devolve o file status COBOL
`39`; um arquivo inexistente aberto como `INPUT` devolve `35`; um cabeçalho
corrompido ou truncado devolve `90`. (A validação estrita pode ser relaxada pelo
sinalizador `strict_metadata` do motor.)

---

## 10. Referência rápida — quem armazena o quê

| Coisa                             | Onde ela vive                          | Cópias        |
|-------------------------------|----------------------------------------|-------------|
| Geometria das chaves (deslocamentos/larguras) | Esquema de chaves do cabeçalho (página 0) | uma vez |
| Nomes dos campos de dados     | Apenas no `FD` do programa             | não no arquivo |
| Bytes do registro             | Páginas `PT_DATA` / `PT_OVERFLOW`      | uma/registro |
| chave → RecordId              | uma árvore B+ por chave                | uma/chave   |
| RecordId → localização física | Diretório de RecordId (cadeia `PT_DIR`) | uma/registro |
| Páginas livres                | Lista de livres (`free_list_head`)     | —           |
| Inversas de mudanças não confirmadas | Log de desfazer na RAM          | por tx      |
```

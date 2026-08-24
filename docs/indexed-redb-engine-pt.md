<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Motor INDEXED à prova de falhas (redb)

O PowerRustCOBOL traz um segundo motor `STORAGE IS DISK` para arquivos
`ORGANIZATION IS INDEXED`, construído sobre o **redb** — um armazenamento
chave-valor ACID embutido e puro Rust (árvore B+ com copy-on-write, páginas de
metadados duplicadas, checksums por página). Ele apresenta o comportamento COBOL
observável *idêntico* ao do motor padrão `PRCIDXD1`, mas foi desenhado em torno
de quatro objetivos operacionais que o motor sob medida não conseguia atender em
escala.

Hoje ele é **opcional** (o motor de disco padrão continua sendo o `PRCIDXD1`):

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

Implementação:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Por quê — os quatro objetivos

| Objetivo | Como o motor redb o atende |
|------|------------------------------|
| **O OPEN é instantâneo, sempre** | O redb lê apenas a sua página de metadados na abertura. **Não há diretório de registros em RAM para carregar nem varredura de recuperação**, mesmo depois de uma queda. Medido: cerca de 5 ms para abrir um arquivo de 200 000 registros (independente da quantidade de registros). |
| **READ RANDOM / NEXT na velocidade da luz** | O RANDOM é uma descida na árvore B+; o NEXT é um iterador sequencial de intervalo. Ambos correm sobre o cache de páginas do redb. Medido: cerca de 21 µs por leitura aleatória com 200 000 registros. |
| **Até 250 M de registros (dados sem limite)** | A RAM residente é o conjunto de trabalho (o cache do redb), **não** a quantidade de registros. Não há nenhuma estrutura `O(registros)` mantida em memória. |
| **A segurança é o que mais importa** | O redb é totalmente ACID. O `COMMIT` é um commit de transação durável (fsync); o `ROLLBACK` é um aborto de transação. Uma queda de energia nunca pode expor um índice partido — o redb volta ao último commit bom por meio das suas páginas de metadados duplicadas. Sem perda de dados, sem corrupção de índice. |

Compare com o motor `PRCIDXD1`, cujo diretório de RecordId é carregado inteiro na
RAM no OPEN (≈16 bytes × cada RecordId já alocado) e cujas transações eram um log
de desfazer em RAM, persistido apenas no CLOSE — de modo que ele não conseguia
nem abrir instantaneamente em escala, nem sobreviver a uma queda de energia no
meio da execução.

---

## Disposição em disco (tabelas do redb)

| Tabela redb | Tipo     | chave → valor                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | bytes da chave primária → registro (opcionalmente comprimido) |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | bytes da chave primária → sequência de inserção `u64`  |
| `meta`     | table    | descritores `schema`, `compress`, `nextseq`   |

- Um **único multimap `alt`** guarda todas as chaves alternadas, separadas por um
  índice de chave de 2 bytes em big-endian. A ordem de bytes é portanto
  `(índice da chave, valor alternado, sequência de inserção)` — o que faz as
  alternadas duplicadas serem iteradas em **ordem de criação**, coincidindo
  exatamente com a ordenação de RecordId do motor de disco e com a regra COBOL
  para chaves alternadas duplicadas.
- A maquinaria `seq` / `meta:nextseq` existe **apenas** para ordenar duplicatas
  de chave alternada. Arquivos sem chaves alternadas a ignoram por completo e
  pagam apenas uma inserção na árvore B+ por `WRITE`.
- Os registros são armazenados como imagens posicionais de largura fixa (veja
  [`indexed-file-internals-pt.md`](indexed-file-internals-pt.md) §6); o `WITH
  COMPRESSION` aplica o mesmo RLE PackBits usado pelos outros motores.

---

## Modelo de transações

Uma abertura para escrita (`OUTPUT` / `I-O` / `EXTEND`) mantém uma
`WriteTransaction` do redb aberta desde o OPEN. As leituras através dessa
transação enxergam as escritas ainda não confirmadas do próprio programa (o "ler
as próprias escritas" do COBOL). Os verbos COBOL mapeiam diretamente:

| COBOL | redb |
|-------|------|
| `OPEN`     | inicia uma transação de escrita (modos com escrita) |
| `COMMIT`   | faz `commit()` da transação (durável) e inicia uma nova |
| `ROLLBACK` | faz `abort()` da transação (descarta tudo desde o último `COMMIT`/`OPEN`) e inicia uma nova |
| `CLOSE`    | `commit()` (commit implícito) |

As aberturas em `INPUT` usam transações de leitura curtas. Como o `ROLLBACK` é um
aborto de verdade do redb, **nenhum log de desfazer é necessário** — durabilidade
e reversão são garantias do próprio armazenamento.

> Os verbos COBOL `COMMIT` / `ROLLBACK` agem sobre **arquivos INDEXED**, não
> sobre conexões SQL (essas usam `COBOL-EXEC-SQL` com
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Paridade de comportamento

O motor é cobrado pelo comportamento exato do motor padrão: os mesmos fixtures
versionados (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) rodam sob `--indexed-engine redb` e precisam produzir saída de
DISPLAY idêntica — CRUD com chave primária mais alternada `WITH DUPLICATES`,
persistência através de uma reabertura, e `COMMIT`/`ROLLBACK`. Os códigos de
status de arquivo (`00/02/10/22/23/35/39/46/47/48/49/90/...`), a resolução da
chave de referência, a semântica do `START` e a regra de que "REWRITE/DELETE
precisam de um registro corrente" coincidem todos.

Testes: `crates/cobolt-runtime/tests/test_indexed_redb.rs` (os fixtures sob redb
+ verificações diretas de `IndexedStore` + um teste de fumaça de escala marcado
com `#[ignore]`).

---

## Limites

Como o motor pagina sob demanda, os limites práticos são impostos pelo redb e
pelo sistema de arquivos, não pela RAM residente:

| Dimensão | Limite |
|-----------|-------|
| Tamanho do arquivo | limite do redb / sistema de arquivos (terabytes) |
| Registros | limitado pela RAM do conjunto de trabalho, não pela contagem de registros (≥250 M com um cache pequeno) |
| Tamanho do registro | imagem de largura fixa; registros grandes ficam armazenados como valores do redb |
| Tamanho da chave | bytes da chave composta (chaves de várias partes são suportadas pela camada COBOL) |
| Chaves alternadas | até 65 535 (espaço de índice de 2 bytes) |

---

## Notas de desempenho

- O **`READ NEXT` sequencial** pela chave primária de referência devolve o
  registro direto do cursor de intervalo — uma descida na árvore B+ por registro,
  não duas (cerca de 17 µs por registro com 200 000). As varreduras por chave
  alternada ainda fazem uma descida na alternada mais uma busca na primária.
- O **`WRITE`** abre as tabelas `primary`/`alt` uma vez por operação (a
  verificação de duplicata e a inserção compartilham o handle). Um
  micro-benchmark mostrou que manter o handle em cache *entre* chamadas acrescenta
  apenas cerca de 8 % sobre abrir uma vez por operação, então o motor mantém o
  caminho simples e livre de `unsafe`. O custo de escrita (cerca de 44 µs por
  registro) é dominado pela inserção ACID na árvore B+ do redb, que é o piso
  seguro — nenhuma das otimizações de escrita altera os pontos de commit nem a
  durabilidade.
- O **`WRITE` em massa** fica portanto em cerca de 20 mil registros/s numa única
  transação (um custo de carga que se paga uma vez só). OPEN, leituras e
  segurança contra falhas não são afetados.

---

## Log de observabilidade (`--indexed-log`)

O motor redb pode escrever um log de transações opcional por arquivo (desligado
por padrão) em **`<assign-path>.log`** (por exemplo, `customers.idx` →
`customers.idx.log`), com uma linha por `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE`
registrando data e hora, contagens de registros e bytes, vazão, qualidade da
ordenação das chaves na escrita e — no nível `full` — estatísticas de páginas de
índice do redb.

```bash
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

O formato da linha é `text` (logfmt) ou `json` (NDJSON, pronto para
Grafana/Loki).

**A referência completa** — flags, a tabela de campos, os formatos, o pipeline
Grafana/Loki (Promtail + LogQL) e notas de custo e segurança — está em
[`observability-pt.md`](observability-pt.md) §1.

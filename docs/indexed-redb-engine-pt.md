<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Motor INDEXED à prova de falhas (redb)

O PowerRustCOBOL inclui um segundo motor `STORAGE IS DISK` para arquivos
`ORGANIZATION IS INDEXED`, construído sobre o **redb** — um armazém chave-valor
ACID embutido e escrito inteiramente em Rust (B+tree com cópia na escrita,
páginas meta duplicadas, somas de verificação por página). Apresenta um
comportamento COBOL observável *idêntico* ao do motor `PRCIDXD1` mais antigo,
mas foi desenhado em torno de quatro objetivos operacionais que o motor próprio
não conseguia cumprir em escala.

**É o motor padrão, e é-o desde a 1.62.73** (decisão do operador, 2026-08-29).
`IndexedEngine` deriva `Default` com `#[default]` em `Redb`
(`crates/cobolt-runtime/src/indexed.rs:126`), e um teste o mantém lá
(`indexed.rs:1643`). Nada precisa ser selecionado para obtê-lo.

O motor paginado mais antigo continua disponível pelo nome, tal como os dois
aliases que delegam no contentor Rust incorporado:

```bash
rcrun run program.cbl --indexed-engine rust    # o motor paginado PRCIDXD1
# ou
COBOL_INDEXED_ENGINE=rust rcrun run program.cbl
```

Implementação:
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Porquê — os quatro objetivos

| Objetivo | Como o motor redb o cumpre |
|------|------------------------------|
| **O OPEN é instantâneo, sempre** | O redb lê apenas a sua página meta ao abrir. **Não há diretório de registros em RAM para carregar nem varrimento de recuperação**, nem sequer depois de uma falha. Medido: ~5 ms para fazer OPEN de um arquivo de 200 000 registros (independentemente da contagem de registros). |
| **READ RANDOM / NEXT à velocidade da luz** | RANDOM é uma descida pelo B+tree; NEXT é um iterador de intervalo sequencial. Ambos correm sobre a cache de páginas do redb. Medido: ~21 µs por leitura aleatória com 200 000 registros. |
| **Até 250 M de registros (dados sem limite)** | A RAM residente é o conjunto de trabalho (a cache do redb), **não** a contagem de registros. Não existe qualquer estrutura `O(registos)` mantida em memória. |
| **A segurança está acima de tudo** | O redb é totalmente ACID. `COMMIT` é um commit de transação durável (fsync); `ROLLBACK` é um aborto de transação. Uma falha de energia nunca pode expor um índice partido — o redb recua para o último commit válido através das suas páginas meta duplicadas. Sem perda de dados, sem corrupção do índice. |

Compare-se com o motor `PRCIDXD1`, cujo diretório de RecordId é carregado por
inteiro para a RAM no OPEN (≈16 bytes × cada RecordId alguma vez atribuído) e
cujas transações eram um registro de desfazer em RAM persistido apenas no CLOSE —
de modo que não conseguia abrir instantaneamente em escala nem sobreviver a uma
falha de energia a meio da execução.

---

## Disposição em disco (tabelas redb)

| Tabela redb | Tipo     | chave → valor                                 |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | bytes da chave primária → registro (opcionalmente comprimido) |
| `alt`      | multimap | `[u16 idx][bytes da chave alternativa]` → `[u64 seq][chave primária]` |
| `seq`      | table    | bytes da chave primária → sequência `u64` de inserção |
| `meta`     | table    | descritores `schema`, `compress`, `nextseq`   |

- Um **único multimapa `alt`** guarda todas as chaves alternativas, com espaço de
  nomes dado por um índice de chave de 2 bytes em big-endian. A ordem de bytes é,
  portanto, `(índice da chave, valor alternativo, sequência de inserção)` — o que
  faz com que as alternativas duplicadas sejam percorridas por **ordem de
  criação**, exatamente como a ordenação de RecordId do motor de disco e como a
  regra COBOL para chaves alternativas duplicadas.
- A maquinaria `seq` / `meta:nextseq` existe **apenas** para ordenar duplicados
  de chaves alternativas. Arquivos sem chaves alternativas ignoram-na por
  completo e pagam uma única inserção no B+tree por `WRITE`.
- Os registros são guardados como imagens posicionais de largura fixa (ver
  [`indexed-file-internals-pt.md`](indexed-file-internals-pt.md) §6); `WITH
  COMPRESSION` aplica o mesmo RLE PackBits usado pelos outros motores.

---

## Modelo transacional

Uma abertura para escrita (`OUTPUT` / `I-O` / `EXTEND`) mantém aberta uma
`WriteTransaction` do redb desde o OPEN. As leituras através dessa transação veem
as escritas ainda não confirmadas do próprio programa (o «ler o que escreveste»
do COBOL). Os verbos COBOL correspondem diretamente:

| COBOL | redb |
|-------|------|
| `OPEN`     | inicia uma transação de escrita (modos de escrita) |
| `COMMIT`   | `commit()` da transação (durável) e depois inicia uma nova |
| `ROLLBACK` | `abort()` da transação (descarta tudo desde o último `COMMIT`/`OPEN`) e depois inicia uma nova |
| `CLOSE`    | `commit()` (confirmação implícita) |

As aberturas `INPUT` usam transações de leitura curtas. Como `ROLLBACK` é um
aborto verdadeiro do redb, **não é preciso qualquer registro de desfazer** — a
durabilidade e a reversão são garantias do próprio armazém.

> Os verbos COBOL `COMMIT` / `ROLLBACK` atuam sobre **arquivos INDEXED**, não
> sobre ligações SQL (essas usam `COBOL-EXEC-SQL` com
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Paridade de comportamento

O motor é mantido no comportamento exato do motor padrão: os mesmos testes
versionados (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`, `idx_tx.cbl`)
correm com `--indexed-engine redb` e têm de produzir saída DISPLAY idêntica —
CRUD com chave primária e alternativa `WITH DUPLICATES`, persistência entre
reaberturas, e `COMMIT`/`ROLLBACK`. Os códigos de estado de arquivo
(`00/02/10/22/23/35/39/46/47/48/49/90/...`), a resolução da chave de referência,
a semântica do `START` e a regra de que «REWRITE/DELETE precisam de um registro
atual» coincidem todos.

Testes: `crates/cobolt-runtime/tests/test_indexed_redb.rs` (os testes sob redb +
verificações diretas ao `IndexedStore` + um teste de fumo em escala marcado
`#[ignore]`).

---

## Limites

Como o motor é paginado a pedido, os limites práticos são fixados pelo redb e
pelo sistema de arquivos, não pela RAM residente:

| Dimensão | Limite |
|-----------|-------|
| Tamanho do arquivo | limite do redb / do sistema de arquivos (terabytes) |
| Registros | limitado pela RAM do conjunto de trabalho, não pela contagem de registros (≥250 M com uma cache pequena) |
| Tamanho do registro | imagem de largura fixa; registros grandes são guardados como valores redb |
| Tamanho da chave | bytes da chave composta (a camada COBOL suporta chaves de várias partes) |
| Chaves alternativas | até 65 535 (espaço de índice de 2 bytes) |

---

## Notas de desempenho

- O **`READ NEXT` sequencial** pela chave primária de referência devolve o
  registro diretamente do cursor de intervalo — uma descida pelo B+tree por
  registro, não duas (~17 µs/registro com 200 000). Os varrimentos por chave
  alternativa continuam a fazer uma descida alternativa mais uma procura
  primária.
- O **`WRITE`** abre as tabelas `primary`/`alt` uma vez por operação (a
  verificação de duplicados e a inserção partilham o manipulador). Um
  micro-benchmark mostrou que manter o manipulador em cache *entre* chamadas
  acrescenta apenas ~8 % face a abri-lo uma vez por operação, de modo que o motor
  mantém o caminho simples e sem `unsafe`. O custo de escrita (~44 µs/registro) é
  dominado pela inserção ACID no B+tree do redb, que é o patamar seguro — nenhuma
  das otimizações de escrita altera os pontos de confirmação nem a durabilidade.
- Por isso o **`WRITE` em massa** anda pelos 20 k registros/s numa única transação
  (um custo de carregamento pago uma vez). O OPEN, as leituras e a resistência a
  falhas não são afetados.

---

## Registro de observabilidade (`--indexed-log`)

O motor redb pode escrever um registro de transações opcional por arquivo
(desligado por padrão) em **`<caminho-assign>.log`** (p. ex. `customers.idx` →
`customers.idx.log`), com uma linha por `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` a
registrar data/hora, contagens de registros e bytes, débito, qualidade da ordenação
das chaves em escrita e — no nível `full` — estatísticas de páginas do índice
redb.

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

O formato de linha é `text` (logfmt) ou `json` (NDJSON, pronto para
Grafana/Loki).

**A referência completa** — opções, a tabela de campos, os formatos, a cadeia
Grafana/Loki (Promtail + LogQL) e as notas de custo e segurança — está em
[`observability-pt.md`](observability-pt.md) §1.

.<<

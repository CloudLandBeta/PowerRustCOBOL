<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Matriz de suporte do PowerRustCOBOL

**Para que serve este documento:** um único lugar percorrível de olho que
responde *"o PowerRustCOBOL faz X, e X é COBOL padrão ou algo que esta
plataforma acrescenta?"* Cada recurso é uma linha. Sem listas em prosa — se algo
é suportado, existe uma linha para a qual você pode apontar.

Este é o **panorama**. Dois documentos companheiros carregam o detalhe:

| Documento | O que ele responde |
|---|---|
| [`cobol85-supported-syntax-pt.md`](cobol85-supported-syntax-pt.md) | **Qual grafia** de cada instrução o lexer/parser/runtime de fato aceita, e o placar de conformidade NIST CCVS85 |
| [`cobol85-verb-test-matrix-pt.md`](cobol85-verb-test-matrix-pt.md) | **O que testar** para cada verbo |
| [`developers-guide-en.md`](developers-guide-en.md) | Como construir aplicações com tudo isso |

---

## Como ler as tabelas

Cada linha de recurso é marcada contra três origens e depois recebe um status.

| Coluna | Significado |
|---|---|
| **85** | Definido pelo **COBOL-85** (ANSI X3.23-1985, incluindo a emenda de funções intrínsecas de 1989 onde indicado) |
| **20xx** | Definido por um **padrão ISO posterior** — COBOL 2002 / 2014 / 2023, e o que está atualmente em rascunho rumo a 2026 |
| **PRC** | Uma **extensão do PowerRustCOBOL** — não está em nenhum padrão COBOL |
| **Status** | O que esta implementação faz com isso |

Um recurso pode ser marcado em mais de uma coluna de origem: um recurso do
COBOL-85 que um padrão posterior estendeu recebe `●` em ambas, e a coluna
**Notas** diz o que o padrão posterior acrescentou.

**Marcas de origem:** `●` definido aqui · `○` estendido/esclarecido aqui · `—`
ausente deste padrão.

**Marcas de status:** `✅` suportado · `🚧` parcial ou simplificado · `⛔` planejado,
ainda não implementado · `🚫` fora de escopo por decisão de projeto, nunca implementado.

> **Nota de honestidade.** O PowerRustCOBOL mira um subconjunto prático e
> orientado a aplicações, mais extensões visuais de RAD. Ele **não** é uma
> implementação certificada de COBOL-85. A conformidade é *medida* contra a
> suíte oficial NIST CCVS85 em vez de afirmada — veja o
> [placar](cobol85-supported-syntax-pt.md).

---

## 1. Formato de fonte e estrutura de programa

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Fonte em formato fixo, **relaxado** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **O padrão.** A área de sequência e a coluna indicadora são respeitadas, mas a linha vai até onde o desenvolvedor digitou — sem corte na coluna 72. Os `.cbl` de formulário gerados e os blocos `EXEC RUST` precisam disso |
| Fonte em formato fixo, **formato de referência clássico do COBOL-85** (`--source-format=fixed`) | ● | ○ | — | ✅ | Toda regra de coluna aplicada: 1–6 sequência, 7 indicador (`*` `/` comentário, `-` continuação, `D` linha de depuração), 8–72 fonte, **73–80 descartadas**, junção de continuação padrão, inclusive de literal alfanumérico continuado. É nele que a suíte de imagem de cartão NIST CCVS85 está escrita. **Escolhido explicitamente, nunca por detecção** — aplicar essas regras a um fonte que não foi escrito para elas apaga código silenciosamente |
| Fonte em formato livre | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Chave de formato de fonte — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | Também `COBOLT_SOURCE_FORMAT`; `auto` inspeciona as primeiras linhas e nunca seleciona o formato estrito |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ | |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ | |
| DATA DIVISION | ● | ○ | — | ✅ | |
| PROCEDURE DIVISION | ● | ○ | — | ✅ | |
| Programas aninhados | ● | ○ | — | ✅ | |
| Múltiplas unidades de programa sequenciais em um mesmo arquivo | ● | ○ | — | ✅ | |
| Copybooks `COPY` / `REPLACE` | ● | ○ | — | ✅ | Substituição de pseudotexto e de palavras, `COPY` aninhado, `REPLACE OFF`; resolve `.cpy`/`.cbl`/`.cob` ao lado do fonte, sem diferenciar maiúsculas de minúsculas |
| Parágrafo `REPOSITORY` | — | ● | ○ | ✅ | COBOL 2002 para classes; o PowerRustCOBOL também vincula tipos de **FFI Rust** aqui |
| Rust embutido `EXEC RUST … END-EXEC` | — | — | ● | ✅ | Compilado dentro do binário; os erros são reportados na linha e na coluna COBOL do próprio desenvolvedor |

## 2. Data division e descrição de dados

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ | |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ | |
| FILE SECTION | ● | ○ | — | ✅ | |
| SCREEN SECTION | ● | ○ | — | 🚧 | Os `ACCEPT`/`DISPLAY` estendidos com `AT`/`WITH` executam via ANSI no modo CLI; a edição de tela em nível de campo é substituída pelo designer visual de formulários no modo GUI |
| COMMUNICATION SECTION (`CD`, controle de mensagens) | ● | — | — | 🚫 | Teleprocessamento; obsoleta nos padrões posteriores |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Fora de escopo por decisão de projeto |
| `PICTURE` X / A / 9 / S / V com repetição `(n)` | ● | ○ | — | ✅ | |
| PICTURE numérica editada (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Supressão de zeros, proteção de cheque, `$` e sinais fixos e flutuantes |
| `USAGE DISPLAY` | ● | ○ | — | ✅ | |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ | |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Ponto flutuante; extensão de fornecedor padronizada depois como `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ | |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Binário nativo; extensão de fornecedor |
| `USAGE INDEX` | ● | ○ | — | ✅ | |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002; leitura **e** escrita pelo alias |
| `OCCURS` fixo | ● | ○ | — | ✅ | |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ | |
| `INDEXED BY` | ● | ○ | — | ✅ | |
| Números de nível 01–49, 77 | ● | ○ | — | ✅ | |
| `RENAMES` de nível 66 | ● | ○ | — | ✅ | |
| Nomes de condição de nível 88 | ● | ○ | — | ✅ | Inclusive `SET … TO TRUE` |
| Cláusula `VALUE` | ● | ○ | — | ✅ | |
| Itens de grupo, `FILLER` | ● | ○ | — | ✅ | |
| `REDEFINES` | ● | ○ | — | ✅ | |
| Constantes figurativas (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ | |

## 3. Procedure division — verbos

| Verbo | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Correspondência de subcampos de grupo |
| `DISPLAY` | ● | ○ | — | ✅ | Numérico renderizado na largura completa do PIC |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ | |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | Múltiplos receptores, `ROUNDED` por receptor |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Múltiplos receptores, `ROUNDED` por receptor |
| `COMPUTE` | ● | ○ | — | ✅ | Múltiplos receptores, `ROUNDED` por receptor |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ | |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ | |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ | |
| `PERFORM` inline, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ | |
| `PERFORM para VARYING` (fora de linha) | ● | ○ | — | ✅ | |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ | |
| `ALTER` | ● | ○ | — | ✅ | Elemento obsoleto no COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Semântica fiel; obsoleto no COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ | |
| `EXIT` | ● | ○ | — | ✅ | |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ | |
| `GOBACK` | — | ● | — | ✅ | Extensão de fornecedor padronizada no COBOL 2002 |
| `SET` (incl. `UP/DOWN BY`, `TO TRUE` de nível 88) | ● | ○ | — | ✅ | |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | Ponteiros do COBOL 2002 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Sensível à categoria, percorre grupos recursivamente |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ | |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` combinado |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Conduz o índice da tabela, executa o primeiro `WHEN` que casa, senão `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` e `RETURNING` são do COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ | |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ | |
| `INVOKE` | — | ● | ○ | 🚧 | OO do COBOL 2002. Suportado para **objetos de GUI e de runtime e plugins de FFI Rust**; definições de classe/método pelo usuário não estão implementadas |
| `UNLOCK` | — | ● | — | 🚧 | Conduz travas de registro por execução; não é imposto entre processos do sistema operacional |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Transações controladas pelo programa em arquivos INDEXED, com um log de desfazer real |
| Definições OO `CLASS-ID` / `METHOD-ID` | — | ● | — | ⛔ | Planejado |

## 4. Condições e expressões

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Condições de relação, de classe, de sinal e de nome de condição | ● | ○ | — | ✅ | |
| Relações combinadas abreviadas, com operador prefixado (`a > 1 AND < 9`) | ● | ○ | — | ✅ | |
| Relações combinadas abreviadas, com objeto literal (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ | |
| Relações combinadas abreviadas, com objeto identificador (`a = b OR c`) | ● | ○ | — | ✅ | |
| Modificação de referência `item(start:length)` | ● | ○ | — | ✅ | Leitura **e** escrita emendada, sobre qualquer operando |
| Subscrição de tabelas em tempo de execução `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Armazenamento por ocorrência, subscritos variáveis |
| Nomes qualificados `id OF/IN group` | ● | ○ | — | ✅ | Uma folha declarada sob mais de um grupo resolve para armazenamento independente |
| Comparação alfanumérica correta segundo o COBOL (preenchida com espaços) | ● | ○ | — | ✅ | |
| **Aritmética exata de ponto fixo** | ● | ○ | ○ | ✅ | Mantissa inteira `i128`, sem idas e vindas por `f64`: a precisão padrão de 18 dígitos e a **estendida de 31 dígitos** permanecem exatas |
| Expressões concisas de propriedade (`Output::Value`) | — | — | ● | ✅ | Ler/definir uma propriedade de controle dentro de uma fórmula, sem nenhum item temporário na working-storage |

### 4.1 Métodos de valor sobre um item de dados

`item::Method(args)` chama um método sobre o **valor de um item de dados comum**
— um campo `PIC X`, um grupo, uma ocorrência de tabela, uma fatia modificada por
referência ou uma expressão aritmética — não só sobre um controle. Nada disso é COBOL padrão.

Usável em qualquer lugar onde uma expressão cabe: como origem de um `MOVE`, num
`COMPUTE`, dentro de uma condição e embutido num `DISPLAY`. Os métodos
**encadeiam**: `WS-TEXT::Trim()::Len()`.

| Método | Retorna | Status | Notas |
|---|---|:--:|---|
| `Trim()` | texto | ✅ | Espaços à esquerda e à direita removidos |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | texto | ✅ | Três grafias aceitas de um mesmo método |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | texto | ✅ | |
| `Replace(from, to)` | texto | ✅ | Todas as ocorrências |
| `Len()` · `Length()` | numérico | ✅ | O comprimento **do campo**, então um `PIC X(20)` contendo `hello` responde `20`. Encadeie `::Trim()::Len()` para obter o comprimento do conteúdo |
| `Split(sep)` | texto | ✅ | O **primeiro** campo |
| `Split(sep)(n)` | texto | ✅ | O *n*-ésimo campo, começando em 1. O subscrito só é aceito num receptor que seja item de dados |

| Receptor | Status | Notas |
|---|:--:|---|
| Item de dados (`PIC X`, grupo, `01`/`77`) | ✅ | O caso comum |
| Ocorrência de tabela, modificação de referência, nome qualificado, expressão aritmética | ✅ | Aceitos pelo avaliador |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | O interpretador aceita um receptor literal, mas o parser não: `::` depois de um literal é erro de sintaxe. Atribua antes o literal a um item de dados |

### 4.2 Uma expressão onde o COBOL-85 permite apenas um item

O COBOL-85 restringe a maioria das posições de envio a um identificador ou a um
literal. O RustCOBOL avalia ali uma expressão completa, e é isso que elimina o
item descartável de working-storage que o padrão obriga a declarar.

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. O padrão só permite um identificador ou um literal como campo de envio |
| `SET target TO <expression>` | — | — | ● | ✅ | Equivalente à forma `COMPUTE`; o destino pode ser um item de dados ou um lvalue de propriedade de controle |
| `STRING <expression> … INTO` | — | — | ● | ✅ | Um item de envio pode ser uma expressão aritmética (`STRING WS-N * 2 …`) ou uma chamada de método de valor (`STRING WS-A::UpperCase() …`); `DELIMITED BY` e o resto continuam padrão |
| **Inferência de tipo** — uma leitura `Ctrl::Property` é um valor tipado de primeira classe | — | — | ● | ✅ | O tipo numérico/texto flui pela expressão, então uma propriedade entra direto numa aritmética, numa condição ou numa posição de envio **sem nenhum item `PIC` no meio**: `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. Um valor de propriedade com cara de número é lido de volta como numérico, para que comparações e aritmética permaneçam algébricas em vez de caractere a caractere |

## 5. Funções intrínsecas

O conjunto de intrínsecas do COBOL-85 chegou com a **emenda de 1989** (ANSI
X3.23a-1989); as funções acrescentadas pelo COBOL 2002 e posteriores estão
marcadas na coluna `20xx`. Todas as abaixo estão implementadas.

| Grupo | Funções | 85 | 20xx | PRC | Status |
|---|---|:--:|:--:|:--:|:--:|
| Comprimento e caractere | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Comprimento e caractere (posteriores) | `BYTE-LENGTH`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Caixa e texto | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| Texto (posteriores) | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| Conversão numérica | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| Conversão numérica (posteriores) | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| Aritmética | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| Ordenação | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| Estatística | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| Trigonometria e logaritmos | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| Combinatória | `FACTORIAL` | ● | ○ | — | ✅ |
| Financeiras | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| Data e hora | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. E/S de arquivos — organizações e acesso

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Registros de comprimento fixo |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Texto terminado por quebra de linha; espaços à direita descartados na escrita |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Motor ISAM embutido e sem dependências |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Motor próprio (`cobolt-runtime/src/relative.rs`, contêiner `PRCREL1`, disco e MEMORY). `RELATIVE KEY IS` endereça registros pelo número inteiro de registro a partir de 1; os três modos de acesso; os sete verbos de arquivo despacham sobre ele. **Módulo RL do NIST concluído nos dois eixos** — 35/35 na compilação, 34/34 na execução, 354 asserções, 0 falhas (motor 1.62.76, módulo 1.62.77) |
| `RELATIVE KEY IS data-name` (incl. a grafia sem `KEY`) | ● | ○ | — | ✅ | Uma cláusula `RELATIVE data-name` com a palavra `KEY` omitida é a chave, e não uma cláusula de organização isolada |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | Os três executam |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Ordem ascendente de chaves em disco |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ | |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ | |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` é do COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ | |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Inclusive `GREATER/LESS THAN`, `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ | |
| Códigos de `FILE STATUS` | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Analisado e carregado na instrução, **consultivo** — existe uma única unidade de execução, então nada disputa |
| `OPEN … WITH LOCK` (abrir o arquivo com exclusividade) | — | ● | — | 🚧 | Idem: aceito e consultivo no modelo de unidade de execução única |
| `READ … WITH LOCK` | — | ● | — | ✅ | O motor já mantém o registro travado sob `I-O`; a frase declara a intenção |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | De fato libera a trava que o motor obtém sob `I-O` — a única frase de trava com efeito em tempo de execução hoje. `UNLOCK` está na §3, junto com os demais verbos |
| Compartilhamento de arquivos entre processos / imposição de travas de registro | — | ● | — | ⛔ | Planejado; hoje vale o modelo de unidade de execução única |

## 7. E/S de arquivos — o motor INDEXED (PowerRustCOBOL)

Tudo nesta seção é uma extensão da plataforma em torno do comportamento padrão
de `ORGANIZATION IS INDEXED` acima. Detalhes:
[`indexed-file-format-pt.md`](indexed-file-format-pt.md),
[`indexed-file-internals-pt.md`](indexed-file-internals-pt.md),
[`indexed-redb-engine-pt.md`](indexed-redb-engine-pt.md).

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **O padrão.** B+tree paginada e persistente; registros e índices vivem no arquivo do `ASSIGN` e são lidos sob demanda, então a RAM permanece limitada mesmo em arquivos muito grandes |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | Arquivo inteiro em RAM, persistido no caminho do `ASSIGN` ao fechar |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | RLE sem dependências; comprime as sequências de preenchimento de registros COBOL típicos bem além de 50 % |
| `COMMIT` / `ROLLBACK` controlados pelo programa | — | — | ● | ✅ | Log de desfazer real, nos motores de memória e de disco |
| Travamento de registros dentro de uma unidade de execução | — | ○ | ● | ✅ | Veja a ressalva sobre múltiplos processos acima |
| Motor selecionável (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | Também `COBOL_INDEXED_ENGINE`; todos compatíveis em comportamento, `rust` é o padrão |
| Motor ACID `redb`, à prova de falhas | — | — | ● | ✅ | OPEN em O(1) (~5 ms com 200 k registros), RAM do conjunto de trabalho (≥250 M registros), sobrevive a queda de energia sem corromper índices |
| Contêiner autodescritivo `PRCIDX1` | — | — | ● | ✅ | Embute o formato do registro + os descritores de chave; a validação estrita na abertura mapeia divergência de esquema → `39` e arquivo ausente → `35`. Não é compatível byte a byte com o Fujitsu |
| Log de transações por arquivo (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt ou NDJSON pronto para Grafana/Loki — veja [`observability-pt.md`](observability-pt.md) |

## 8. Integrações de runtime

Alcançadas a partir do COBOL como `CALL`s de runtime e `INVOKE`. Nada disso é
COBOL padrão; é o que torna a linguagem utilizável em aplicações modernas.

| Recurso | 85 | 20xx | PRC | Status | Notas |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | Uma única superfície de CALL idêntica para os três; o backend é escolhido pela string de conexão. **Sem bibliotecas de sistema** — nada é ligado a partir do host — mas "Rust puro" só vale para dois dos três: `postgres` e `mysql` são, enquanto `rusqlite` está fixado com `features = ["bundled"]` e compila a **amálgama C do SQLite** por meio de `libsqlite3-sys`. (Essa compilação em C é também o motivo de `test_external_crates_e2e` falhar de forma intermitente dentro de um `cargo build` aninhado.) Veja [`database-runtime-pt.md`](database-runtime-pt.md) |
| **Conjuntos de resultados SQL** — `Fetch()`, `ColumnNames()`, `ColumnCount()`, `ColumnName(n)` | — | — | ● | ✅ | `Fetch()` devolve a próxima linha separada por TABULAÇÕES, e vazia quando se esgotam, de modo que encerra o próprio laço; `ColumnNames()` nomeia o conjunto de resultados na ordem do SELECT, mesmo quando nenhuma linha foi encontrada. Já a superfície `CALL` lê a linha atual uma coluna por vez, por índice — os dois percursos não devem ser misturados no mesmo manipulador |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | Cabeçalhos personalizados |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ | |
| **Gráficos** — barras / linhas / pizza / área / dispersão / rosca | — | — | ● | ✅ | Vinculados a tabelas COBOL |
| **Arquivos de texto** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ | |
| **Temporizadores** | — | — | ● | ✅ | |
| **Gancho de objeto de agente de IA** | — | — | ● | ✅ | |
| **Plugins de FFI Rust** | — | — | ● | ✅ | Módulos declarados sob `REPOSITORY`, despachados via `INVOKE` ou por mapeamentos diretos de propriedade |
| **Procedimentos de usuário** | — | — | ● | ✅ | Procedimentos COBOL compartilhados, editáveis na IDE e chamáveis como `CALL "PROCEDURE-NAME"` |

## 9. Explicitamente fora de escopo

Estes não serão implementados. Estão listados para que a resposta seja
encontrável em vez de ausente.

| Recurso | 85 | 20xx | PRC | Status | Por quê |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, controle de mensagens / teleprocessamento) | ● | — | — | 🚫 | Obsoleta nos padrões posteriores; sem uso moderno |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Substituída pelos próprios relatórios e pela vinculação de dados da plataforma |
| Controles ActiveX / OLE / COM | — | — | — | 🚫 | Específicos de plataforma e não portáveis |

---

## 10. A plataforma em si

Não são recursos da linguagem COBOL — são a IDE, o compilador e o ferramental em
volta deles. Percurso completo no [guia do desenvolvedor](developers-guide-en.md).

### 10.1 A IDE

| Recurso | Status | Notas |
|---|:--:|---|
| Designer visual de formulários | ✅ | Tela de design com múltiplos temas (**Liquid Glass**, **Cobalt Steel**), encaixe na grade, redimensionamento por arrasto de controles e da tela, alinhamento com seleção múltipla, ordenação em z |
| Motor de renderização unificado | ✅ | Paridade de pixels entre designer, pré-visualizador, aplicação em execução e binário compilado |
| Catálogo de controles | ✅ | **42 widgets** distribuídos entre Common, Container, Data, Graphics, Menu, Non-visual e Charts |
| Raio de canto universal e recorte arredondado | ✅ | Filhos aninhados são recortados pela borda arredondada do pai por meio de máscara de entalhe de canto |
| `Transparency` por controle | ✅ | 0 = opaco … 100 = transparente; esmaece face, moldura e sombra enquanto texto, glifos e borda permanecem legíveis. Legendas abaixo do WCAG AA em relação ao que está atrás delas viram para o polo que se lê |
| Widget Animator | ✅ | Renderiza nativamente **GIF / WebP / APNG** |
| Knob, Gauge, Switch, FileDropZone, Maps, Web Search | ✅ | Mostrador rotativo com preenchimento bipolar; KPI radial/linear/rosca com zonas automáticas de alerta e crítica; arrastar e soltar ou seletor nativo |
| Editor de menus avançado | ✅ | Editor visual em árvore, 122 ícones vetoriais embutidos, aninhamento hierárquico, assinaturas HMAC de integridade da configuração |
| Vinculação de dados e arrays de controles | ✅ | Vinculação direta a fontes SQL/de dados; os **Visual Repeating Groups** expandem arrays de GroupBox/Panel a partir da contagem de linhas do `DataSource` em tempo de execução |
| Validação visual e inspetor de formulários | ✅ | Selos de erro em tempo real para manipuladores malformados, vinculações incompletas e anomalias de layout; o gerenciador de processos do `rcrun` acompanha ao vivo % de CPU, RSS, logs e contagem de threads |
| Form Debugger | ✅ | Janela independente sempre no topo: pontos de parada, passo In/Out/Over, inspetor de variáveis, reprodução animada a 1–10 linhas por segundo |
| Malha de assistentes de IA agêntica | ✅ | Orquestrador de LLM **rig-core** (Ollama, OpenAI, Groq, Alibaba Model Studio, outras APIs de nuvem) executando Dev Agent, Editor Assistant e History Compactor, com um log de observabilidade ao vivo e leituras de tokens `↑input ↓output` |
| Grace, a orquestradora | ✅ | Decompõe um pedido, encaminha cada tarefa ao especialista que é dono dela e impõe um **revisor Pedantic** de um para um — nenhum especialista aprova o próprio trabalho |
| Base de conhecimento fragmentada com RAG | ✅ | Indexada com um registro por assunto; distribuída pré-embarcada, GPU com alternativa em CPU de execução fria, **File → Reindex Knowledge Bases** |
| Ciclo de vida de formulários e janelas | ✅ | Um **formulário principal** designado inicia a aplicação; a moldura e o estado de cada formulário são respeitados; `OpenFormSync`/`OpenFormAsync`; a posição da janela é uma propriedade de tempo de design; efeitos de entrada e de saída por projeto |
| Runtime multijanela | ✅ | Pré-visualiza e executa telas em viewports dedicadas do sistema operacional (multi-viewport do egui) |
| Interface internacionalizada | ✅ | 6 idiomas de interface: inglês, espanhol, português, japonês, chinês e francês |
| Seletor de fontes do sistema | ✅ | Qualquer fonte instalada, exibida em seu próprio tipo, aplicada ao vivo no designer, nas pré-visualizações e nos formulários em execução |
| Diálogos nativos de arquivo não bloqueantes | ✅ | Abrir/salvar/navegar sem travar o laço de eventos da interface |

### 10.2 O compilador

| Recurso | Status | Notas |
|---|:--:|---|
| Saída em um único binário nativo | ✅ | Serializa a AST com `bincode` + `flate2`, embute a AST e todos os formulários via `include_bytes!`, compila com `cargo build --release` e emite um binário em `bin/` — **sem incluir nenhum fonte `.cbl`** |
| Avisos de redistribuição | ✅ | `bin/` recebe automaticamente `LICENSE`, `NOTICE` e o aviso do runtime, de modo que as distribuições levam os avisos Apache-2.0 exigidos |
| Diagnósticos reais do `rustc` numa compilação com falha | ✅ | Uma falha de compilação reporta os diagnósticos do próprio compilador, e não uma linha de resumo |

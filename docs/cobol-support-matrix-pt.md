<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.135 -->

# Matriz de suporte do PowerRustCOBOL

**Para que serve este documento:** um único sítio, percorrível de relance, que
responde a *«o PowerRustCOBOL faz X, e X é COBOL normalizado ou algo que esta
plataforma acrescenta?»*. Cada capacidade é uma linha. Sem listas em prosa — se
algo é suportado, tem uma linha para a qual se pode apontar.

Esta é a **visão geral**. O detalhe é levado por dois documentos acompanhantes:

| Documento | O que responde |
|---|---|
| [`cobol85-supported-syntax-pt.md`](cobol85-supported-syntax-pt.md) | **Que grafia** de cada instrução é realmente aceite pelo lexer, pelo analisador e pelo ambiente de execução, e o marcador de conformidade NIST CCVS85 |
| [`cobol85-verb-test-matrix-pt.md`](cobol85-verb-test-matrix-pt.md) | **O que testar** em cada verbo |
| [`developers-guide-en.md`](developers-guide-en.md) | Como construir aplicações com tudo isto |

---

## Como ler as tabelas

Cada linha de capacidade é marcada contra três origens e depois recebe um estado.

| Coluna | Significado |
|---|---|
| **85** | Definido pelo **COBOL-85** (ANSI X3.23-1985, incluindo a emenda de funções intrínsecas de 1989 onde assinalado) |
| **20xx** | Definido por uma **norma ISO posterior** — COBOL 2002 / 2014 / 2023, e o que está sendo redigido rumo a 2026 |
| **PRC** | Uma **extensão do PowerRustCOBOL** — não consta de nenhuma norma COBOL |
| **Estado** | O que esta implementação faz com isso |

Uma capacidade pode estar marcada em mais do que uma coluna de origem: uma
funcionalidade do COBOL-85 que uma norma posterior estendeu leva `●` em ambas, e a
coluna **Notas** diz o que a norma posterior acrescentou.

**Marcas de origem:** `●` definido aqui · `○` estendido ou clarificado aqui ·
`—` não consta desta norma.

**Marcas de estado:** `✅` suportado · `🚧` parcial ou simplificado · `⛔`
previsto, ainda por implementar · `🚫` fora de escopo por desenho, nunca será
implementado.

> **Nota de honestidade.** O PowerRustCOBOL visa um subconjunto prático e
> orientado a aplicações, mais extensões visuais de RAD. **Não** é uma
> implementação certificada do COBOL-85. A conformidade é *medida* contra a suíte
> oficial NIST CCVS85 em vez de afirmada — ver o
> [marcador](cobol85-supported-syntax-pt.md).

---

## 1. Formato da fonte e estrutura do programa

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Fonte em formato fixo, **relaxado** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **O valor por padrão.** A área de sequência e a coluna indicadora são respeitadas, mas a linha vai até onde o programador escreveu — sem corte na coluna 72. Os `.cbl` gerados dos formulários e os blocos `EXEC RUST` precisam disto |
| Fonte em formato fixo, **formato de referência clássico do COBOL-85** (`--source-format=fixed`) | ● | ○ | — | ✅ | Todas as regras de colunas aplicadas: 1–6 sequência, 7 indicador (`*` `/` comentário, `-` continuação, `D` linha de depuração), 8–72 fonte, **73–80 descartadas**, e a junção padrão das continuações, incluindo um literal alfanumérico continuado. É nisto que está escrita a suíte de imagens de cartão NIST CCVS85. **Escolhido explicitamente, nunca por deteção** — aplicar estas regras a fonte que não foi escrita para elas apaga código em silêncio |
| Fonte em formato livre | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Seletor do formato da fonte — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | Também `COBOLT_SOURCE_FORMAT`; o `auto` inspeciona as primeiras linhas e nunca escolhe o formato estrito |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ |  |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ |  |
| DATA DIVISION | ● | ○ | — | ✅ |  |
| PROCEDURE DIVISION | ● | ○ | — | ✅ |  |
| Programas aninhados | ● | ○ | — | ✅ |  |
| Várias unidades de programa sequenciais num só arquivo | ● | ○ | — | ✅ |  |
| Copybooks `COPY` / `REPLACE` | ● | ○ | — | ✅ | Substituição de pseudotexto e de palavras, `COPY` aninhado, `REPLACE OFF`; resolve `.cpy`/`.cbl`/`.cob` ao lado da fonte, sem distinguir maiúsculas |
| Parágrafo `REPOSITORY` | — | ● | ○ | ✅ | COBOL 2002 para classes; o PowerRustCOBOL liga aqui também os tipos de **FFI de Rust** |
| Rust em linha com `EXEC RUST … END-EXEC` | — | — | ● | ✅ | Compilado dentro do binário; os erros são reportados na linha e coluna COBOL do próprio programador |

## 2. A DATA DIVISION e a descrição de dados

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ |  |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ |  |
| FILE SECTION | ● | ○ | — | ✅ |  |
| SCREEN SECTION | ● | ○ | — | 🚧 | Os `ACCEPT`/`DISPLAY` estendidos com `AT`/`WITH` executam via ANSI em modo CLI; a edição de tela campo a campo é substituída pelo desenhador visual de formulários em modo GUI |
| COMMUNICATION SECTION (`CD`, controle de mensagens) | ● | — | — | 🚫 | Teleprocessamento; obsoleto nas normas posteriores |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Fora de escopo por desenho |
| `PICTURE` X / A / 9 / S / V com repetição `(n)` | ● | ○ | — | ✅ |  |
| PICTURE numérica editada (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Supressão de zeros, proteção com asteriscos, `$` e sinais fixos e flutuantes |
| `USAGE DISPLAY` | ● | ○ | — | ✅ |  |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ |  |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Vírgula flutuante; uma extensão de fabricante normalizada mais tarde como `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ |  |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Binário nativo; extensão de fabricante |
| `USAGE INDEX` | ● | ○ | — | ✅ |  |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002; alias de leitura **e** de escrita |
| `OCCURS` fixo | ● | ○ | — | ✅ |  |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ |  |
| `INDEXED BY` | ● | ○ | — | ✅ |  |
| Números de nível 01–49, 77 | ● | ○ | — | ✅ |  |
| Nível 66 `RENAMES` | ● | ○ | — | ✅ |  |
| Nomes de condição de nível 88 | ● | ○ | — | ✅ | Incluindo `SET … TO TRUE` |
| Cláusula `VALUE` | ● | ○ | — | ✅ |  |
| Itens de grupo, `FILLER` | ● | ○ | — | ✅ |  |
| `REDEFINES` | ● | ○ | — | ✅ |  |
| Constantes figurativas (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ |  |

## 3. A PROCEDURE DIVISION — verbos

| Verbo | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Correspondência de subcampos de grupo |
| `DISPLAY` | ● | ○ | — | ✅ | O numérico é mostrado com a largura completa da PIC |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ |  |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | Vários recetores, `ROUNDED` por recetor |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Vários recetores, `ROUNDED` por recetor |
| `COMPUTE` | ● | ○ | — | ✅ | Vários recetores, `ROUNDED` por recetor |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ |  |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ |  |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ |  |
| `PERFORM` em linha, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ |  |
| `PERFORM para VARYING` (fora de linha) | ● | ○ | — | ✅ |  |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ |  |
| `ALTER` | ● | ○ | — | ✅ | Elemento obsoleto no COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Semântica fiel; obsoleto no COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ |  |
| `EXIT` | ● | ○ | — | ✅ |  |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ |  |
| `GOBACK` | — | ● | — | ✅ | Extensão de fabricante normalizada no COBOL 2002 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ |  |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | Ponteiros do COBOL 2002 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Consciente da categoria, desce pelos grupos |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ |  |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` combinado |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Conduz o índice da tabela, executa o primeiro `WHEN` que corresponde e, se não houver, o `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` e `RETURNING` são do COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ |  |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ |  |
| `INVOKE` | — | ● | ○ | 🚧 | OO do COBOL 2002. Suportado para **objetos de GUI e de execução e para plug-ins de FFI de Rust**; as definições de classe e método pelo usuário não estão implementadas |
| `UNLOCK` | — | ● | — | 🚧 | Conduz os bloqueios de registro dentro da execução; não é imposto entre processos do sistema operacional |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Transações controladas pelo programa sobre arquivos INDEXED, com um registro de desfazer a sério |
| Definições OO `CLASS-ID` / `METHOD-ID` | — | ● | — | ⛔ | Previsto |

## 4. Condições e expressões

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| Condições de relação, de classe, de sinal e de nome de condição | ● | ○ | — | ✅ |  |
| Relações combinadas abreviadas, com operador à frente (`a > 1 AND < 9`) | ● | ○ | — | ✅ |  |
| Relações combinadas abreviadas, com objeto literal (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ |  |
| Relações combinadas abreviadas, com objeto identificador (`a = b OR c`) | ● | ○ | — | ✅ |  |
| Modificação de referência `item(start:length)` | ● | ○ | — | ✅ | Leitura **e** escrita emendada, sobre qualquer operando |
| Índices de tabela em execução `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Armazenamento por ocorrência, índices variáveis |
| Nomes qualificados `id OF/IN group` | ● | ○ | — | ✅ | Uma folha declarada sob mais do que um grupo resolve para armazenamentos independentes |
| Comparação alfanumérica correta segundo o COBOL (preenchida com espaços) | ● | ○ | — | ✅ |  |
| **Aritmética exata de vírgula fixa** | ● | ○ | ○ | ✅ | Mantissa inteira `i128`, sem idas e voltas por `f64`: a precisão padrão de 18 dígitos e a **estendida de 31 dígitos** mantêm-se exatas |
| Expressões de propriedade concisas (`Output::Value`) | — | — | ● | ✅ | Ler ou definir uma propriedade de um controle dentro de uma fórmula, sem qualquer item temporário de working-storage |

### 4.1 Métodos de valor sobre um item de dados

`item::Method(args)` chama um método sobre o **valor de um item de dados
corrente** — um campo `PIC X`, um grupo, uma ocorrência de tabela, uma fatia com
modificação de referência ou uma expressão aritmética — e não apenas sobre um
controle. Nada disto é COBOL normalizado.

Pode usar-se onde quer que caiba uma expressão: como origem de um `MOVE`, num
`COMPUTE`, dentro de uma condição e em linha num `DISPLAY`. Os métodos
**encadeiam-se**: `WS-TEXT::Trim()::Len()`.

| Método | Devolve | Estado | Notas |
|---|---|:--:|---|
| `Trim()` | texto | ✅ | Espaços à esquerda e à direita removidos |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | texto | ✅ | Três grafias aceites de um mesmo método |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | texto | ✅ |  |
| `Replace(from, to)` | texto | ✅ | Todas as ocorrências |
| `Len()` · `Length()` | numérico | ✅ | O comprimento **do campo**, por isso um `PIC X(20)` que contém `hello` responde `20`. Encadeie `::Trim()::Len()` para obter o comprimento do conteúdo |
| `Split(sep)` | texto | ✅ | O **primeiro** campo |
| `Split(sep)(n)` | texto | ✅ | O *n*-ésimo campo, a contar de 1. O índice só é aceite sobre um recetor que seja um item de dados |

| Recetor | Estado | Notas |
|---|:--:|---|
| Item de dados (`PIC X`, grupo, `01`/`77`) | ✅ | O caso corrente |
| Ocorrência de tabela, modificação de referência, nome qualificado, expressão aritmética | ✅ | Aceite pelo avaliador |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | O interpretador aceita um recetor literal, mas o analisador não: um `::` depois de um literal é um erro de sintaxe. Atribua primeiro o literal a um item de dados |

### 4.2 Uma expressão onde o COBOL-85 só admite um item

O COBOL-85 restringe a maioria das posições emissoras a um identificador ou a um
literal. O RustCOBOL avalia aí uma expressão completa, e é isso que elimina o item
auxiliar de working-storage que a norma obriga a declarar.

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. A norma só permite um identificador ou um literal como campo emissor |
| `SET target TO <expression>` | — | — | ● | ✅ | Equivalente à forma `COMPUTE`; o destino pode ser um item de dados ou uma propriedade de controle como lvalue |
| `STRING <expression> … INTO` | — | — | ● | ✅ | Um item emissor pode ser uma expressão aritmética (`STRING WS-N * 2 …`) ou uma chamada a um método de valor (`STRING WS-A::UpperCase() …`); o `DELIMITED BY` e o resto mantêm-se normalizados |
| **Inferência de tipos** — ler `Ctrl::Property` dá um valor tipado de primeira classe | — | — | ● | ✅ | O tipo numérico ou de texto flui pela expressão, de modo que uma propriedade entra diretamente numa operação aritmética, numa condição ou numa posição emissora **sem qualquer item `PIC` pelo meio**: `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. O valor de uma propriedade que parece numérico é lido de volta como numérico, de modo que as comparações e a aritmética se mantêm algébricas e não caráter a caráter |

## 5. Funções intrínsecas

O conjunto de intrínsecas do COBOL-85 chegou com a **emenda de 1989** (ANSI
X3.23a-1989); as funções acrescentadas pelo COBOL 2002 e seguintes estão marcadas
na coluna `20xx`. Todas as que se seguem estão implementadas.

| Grupo | Funções | 85 | 20xx | PRC | Estado |
|---|---|:--:|:--:|:--:|:--:|
| Comprimento e carateres | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Comprimento e carateres (posteriores) | `BYTE-LENGTH`, `LENGTH-AN`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Maiúsculas/minúsculas e texto | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
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

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Registros de comprimento fixo |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Texto terminado por mudança de linha; os espaços finais são descartados na escrita |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Motor ISAM incorporado e sem dependências |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Motor próprio (`cobolt-runtime/src/relative.rs`, contentor `PRCREL1`, disco e MEMORY). O `RELATIVE KEY IS` endereça os registros por número inteiro de registro a partir de 1; os três modos de acesso; os sete verbos de arquivo despacham sobre ele. **O módulo RL do NIST está terminado nos dois eixos** — 35/35 de compilação, 34/34 de execução, 354 asserções, 0 falhas (motor 1.62.76, módulo 1.62.77) |
| `RELATIVE KEY IS data-name` (incluindo a grafia sem `KEY`) | ● | ○ | — | ✅ | Uma cláusula `RELATIVE data-name` com a palavra `KEY` omitida é a chave, não uma simples cláusula de organização |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | Os três executam |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Ordem de chave ascendente em disco |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ |  |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ |  |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | O `PREVIOUS` é do COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ |  |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Incluindo `GREATER/LESS THAN` e `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ |  |
| Códigos de `FILE STATUS` | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Analisado e transportado na instrução, mas **consultivo** — há uma única unidade de execução, por isso nada disputa |
| `OPEN … WITH LOCK` (abrir o arquivo em exclusivo) | — | ● | — | 🚧 | O mesmo: aceite e consultivo no modelo de unidade de execução única |
| `READ … WITH LOCK` | — | ● | — | ✅ | O motor já detém o registro sob `I-O`; a frase declara a intenção |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | Liberta mesmo o bloqueio que o motor toma sob `I-O` — hoje é a única frase de bloqueio com efeito em execução. O `UNLOCK` está na §3 com os restantes verbos |
| Partilha de arquivos entre processos e imposição de bloqueios de registro | — | ● | — | ⛔ | Previsto; hoje o modelo é de uma única unidade de execução |

## 7. E/S de arquivos — o motor INDEXED (PowerRustCOBOL)

Tudo nesta secção é uma extensão da plataforma em torno do comportamento
normalizado de `ORGANIZATION IS INDEXED` acima. O detalhe está em
[`indexed-file-format-pt.md`](indexed-file-format-pt.md),
[`indexed-file-internals-pt.md`](indexed-file-internals-pt.md) e
[`indexed-redb-engine-pt.md`](indexed-redb-engine-pt.md).

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **O modo de armazenamento por padrão.** Os registros e os índices vivem no arquivo do `ASSIGN` e são lidos a pedido, de modo que a RAM se mantém limitada mesmo em arquivos muito grandes. Servido pelo motor redb à prova de falhas desde a 1.62.73; ao B+tree paginado mais antigo ainda se chega com `--indexed-engine rust` |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | O arquivo inteiro em RAM, persistido no caminho do `ASSIGN` ao fechar |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | RLE sem dependências; esmaga bem acima dos 50 % as sequências de enchimento típicas dos registros COBOL |
| `COMMIT` / `ROLLBACK` controlados pelo programa | — | — | ● | ✅ | Registro de desfazer a sério, nos motores de memória e de disco |
| Bloqueio de registros dentro de uma unidade de execução | — | ○ | ● | ✅ | Ver a ressalva sobre processos acima |
| Motor selecionável (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | Também `COBOL_INDEXED_ENGINE`; todos compatíveis em comportamento. **O `redb` é o valor por padrão** desde a 1.62.73 (`cobolt-runtime/src/indexed.rs:126`) |
| Motor ACID `redb` à prova de falhas | — | — | ● | ✅ | OPEN em O(1) (~5 ms com 200 k registros), RAM do conjunto de trabalho (≥250 M registros), sobrevive a uma falha de energia sem corromper o índice |
| Contentor autodescritivo `PRCIDX1` | — | — | ● | ✅ | Embute o formato do registro e os descritores de chave; a validação estrita na abertura converte uma divergência de esquema em `39` e um arquivo ausente em `35`. Não é compatível byte a byte com a Fujitsu |
| Registro de transações por arquivo (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt ou NDJSON pronto para Grafana/Loki — ver [`observability-pt.md`](observability-pt.md) |

## 8. Integrações do ambiente de execução

Alcançadas a partir do COBOL como `CALL` de execução e como `INVOKE`. Nada disto
é COBOL normalizado; é o que torna a linguagem utilizável para aplicações
modernas.

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | Uma mesma superfície de CALL para os três; o backend é escolhido a partir da cadeia de ligação. **Sem bibliotecas do sistema** — nada é ligado a partir do host — mas «Rust puro» só é verdade para dois dos três: o `postgres` e o `mysql` são-no, enquanto o `rusqlite` está fixado com `features = ["bundled"]` e compila a **amálgama em C do SQLite** através do `libsqlite3-sys`. (Essa compilação de C é também a razão por que o `test_external_crates_e2e` falha de forma intermitente dentro de um `cargo build` aninhado.) Ver [`database-runtime-pt.md`](database-runtime-pt.md) |
| **Conjuntos de resultados SQL** — `Fetch()`, `ColumnNames()`, `ColumnCount()`, `ColumnName(n)` | — | — | ● | ✅ | O `Fetch()` devolve a linha seguinte separada por tabulações e vazia quando se esgota, de modo que termina o seu próprio ciclo; o `ColumnNames()` nomeia o conjunto de resultados pela ordem do SELECT, mesmo quando não correspondeu a nenhuma linha. A superfície de `CALL` lê antes a linha atual coluna a coluna por índice — as duas travessias não devem ser misturadas no mesmo identificador |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | Cabeçalhos personalizados |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ |  |
| **Gráficos** — barras / linhas / circular / área / dispersão / anel | — | — | ● | ✅ | Ligados a tabelas COBOL |
| **Arquivos de texto** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ |  |
| **Temporizadores** | — | — | ● | ✅ |  |
| **Gancho de objeto de agente de IA** | — | — | ● | ✅ |  |
| **Plug-ins de FFI de Rust** | — | — | ● | ✅ | Módulos declarados sob `REPOSITORY`, despachados por `INVOKE` ou por mapeamentos diretos de propriedades |
| **Procedimentos do usuário** | — | — | ● | ✅ | Procedimentos COBOL partilhados, editáveis no IDE e chamáveis como `CALL "PROCEDURE-NAME"` |

## 9. Explicitamente fora de escopo

Estas coisas não serão implementadas. Estão listadas para que a resposta se possa
encontrar em vez de faltar.

| Capacidade | 85 | 20xx | PRC | Estado | Notas |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, controle de mensagens / teleprocessamento) | ● | — | — | 🚫 | Obsoleto nas normas posteriores; sem uso moderno |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Substituído pelos relatórios e pela ligação de dados da própria plataforma |
| Controles ActiveX / OLE / COM | — | — | — | 🚫 | Específicos de uma plataforma e não portáveis |

---

## 10. A própria plataforma

Não são funcionalidades da linguagem COBOL, mas o IDE, o compilador e as
ferramentas à sua volta. O percurso completo está no
[guia do programador](developers-guide-en.md).

### 10.1 O IDE

| Capacidade | Estado | Notas |
|---|:--:|---|
| Desenhador visual de formulários | ✅ | Tela de desenho com vários temas (**Liquid Glass**, **Cobalt Steel**), ajuste à grelha, redimensionamento por arrasto de controles e da tela, alinhamento com seleção múltipla e ordenação em profundidade |
| Motor de renderização unificado | ✅ | Paridade ao pixel entre o desenhador, a pré-visualização, a aplicação em execução e o binário compilado |
| Catálogo de controles | ✅ | **43 widgets** distribuídos por Common, Container, Data, Graphics, Menu, Non-visual e Charts, mais um tipo `Custom` fornecido por plug-ins |
| Raio de canto universal e recorte arredondado | ✅ | Os filhos aninhados recortam-se à borda arredondada do pai através de um mascaramento por entalhes de canto |
| `Transparency` por controle | ✅ | 0 = opaco … 100 = transparente; esbate a face, a moldura e a sombra enquanto o texto, os glifos e a borda se mantêm legíveis. As legendas que ficam abaixo do WCAG AA face ao que têm por trás saltam para o polo que se lê |
| Widget Animator | ✅ | Renderiza **GIF / WebP / APNG** nativamente |
| Knob, Gauge, Switch, FileDropZone, Maps e Web Search | ✅ | Manípulo rotativo com enchimento bipolar; KPI radial, linear ou em anel com zonas de aviso e críticas automáticas; arrastar e largar ou seletor nativo |
| Editor de menus avançado | ✅ | Editor visual em árvore, **1112** ícones vetoriais incorporados em 37 categorias, aninhamento hierárquico e assinaturas HMAC de integridade da configuração |
| Ligação de dados e matrizes de controles | ✅ | Ligação direta a fontes SQL e de dados; os **grupos repetidores visuais** expandem matrizes de GroupBox e Panel a partir do número de linhas do `DataSource` em execução |
| Validação visual e inspetor de formulários | ✅ | Emblemas de erro em tempo real para manipuladores malformados, ligações incompletas e anomalias de disposição; o gerenciador de processos do `rcrun` acompanha ao vivo a percentagem de CPU, a RSS, os registros e o número de linhas de execução |
| Depurador de formulários | ✅ | Janela independente sempre à frente: pontos de paragem, passo a passo dentro/fora/por cima, inspetor de variáveis e reprodução animada a 1–10 linhas por segundo |
| Malha de assistentes de IA agêntica | ✅ | Orquestrador de LLM **rig-core** (Ollama, OpenAI, Groq, Alibaba Model Studio e outras API na nuvem) rodando o Dev Agent, o Editor Assistant e o History Compactor, com um registro de observabilidade ao vivo e leituras de tokens `↑input ↓output` |
| A Grace, a orquestradora | ✅ | Decompõe um pedido, encaminha cada tarefa para o especialista que a detém e impõe um **revisor Pedantic** um-para-um — nenhum especialista aprova o seu próprio trabalho |
| Base de conhecimento em pedaços com RAG | ✅ | Indexada com um registro por assunto; é distribuída já embebida, com GPU e um recuo para CPU que não aquece, e **File → Reindex Knowledge Bases** |
| Ciclo de vida dos formulários e janelas | ✅ | Um **formulário principal** designado arranca a aplicação; o cromado e o estado de cada formulário são respeitados; `OpenFormSync`/`OpenFormAsync`; a posição da janela é uma propriedade de desenho; efeitos de entrada e saída por projeto |
| Execução com várias janelas | ✅ | Telas de pré-visualização e de execução em viewports próprios do sistema operacional (multi-viewport do egui) |
| Interface internacionalizada | ✅ | 6 idiomas de interface: inglês, espanhol, português, japonês, chinês e francês |
| Seletor de tipos de letra do sistema | ✅ | Qualquer tipo de letra instalado, desenhado na sua própria tipografia e aplicado ao vivo ao desenhador, às pré-visualizações e aos formulários em execução |
| Diálogos de arquivo nativos e não bloqueantes | ✅ | Abrir, salvar e procurar sem travar o ciclo de eventos da interface |

### 10.2 O compilador

| Capacidade | Estado | Notas |
|---|:--:|---|
| Saída num único binário nativo | ✅ | Serializa a AST com `bincode` + `flate2`, embute-a e a todos os formulários via `include_bytes!`, compila com `cargo build --release` e emite um binário em `bin/` — **sem incluir qualquer fonte `.cbl`** |
| Avisos de redistribuição | ✅ | O `bin/` recebe automaticamente `LICENSE`, `NOTICE` e o aviso do ambiente de execução, de modo que as distribuições levam os avisos exigidos pela Apache-2.0 |
| Diagnósticos reais do `rustc` quando a compilação falha | ✅ | Uma falha de compilação reporta os diagnósticos do próprio compilador, não uma linha de resumo |

.<<

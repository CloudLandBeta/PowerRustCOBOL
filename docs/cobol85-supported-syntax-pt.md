<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Referência da sintaxe suportada do RustCOBOL‑85

**Para que serve este documento:** para dizer quanto do padrão COBOL‑85 o
RustCOBOL realmente implementa — e para provar isso contra a **suíte oficial de
validação NIST COBOL‑85**, em vez de apenas afirmar. O
[placar](#-a-conformidade-é-medida-não-afirmada--nist-ccvs85) abaixo é a
manchete; tudo o que vem depois dele é o detalhe por trás daquele número.

**Verdade de campo sobre o que o lexer/parser/runtime do RustCOBOL de fato
aceita hoje**, derivada do código-fonte (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`) e conferida contra `NIST/newcob.val,cbl`.
Escreva os testes contra as formas ✅; as formas ❌ não chegam a ser analisadas ou
são no‑ops, e as formas ⚠️ são analisadas mas se comportam apenas parcialmente.
Este é o documento companheiro de
[`cobol85-verb-test-matrix-pt.md`](cobol85-verb-test-matrix-pt.md): a matriz diz
*o que* testar, e este diz *qual grafia o RustCOBOL entende*.

Legenda: ✅ suportado · ⚠️ analisa mas é parcial/simplificado · ❌ não reconhecido
(evite, ou teste apenas para confirmar a lacuna).

---

## ★ A conformidade é medida, não afirmada — NIST CCVS85

**É este o ponto do documento.** Toda afirmação abaixo é conferida contra a
**suíte oficial de validação NIST COBOL‑85** — CCVS85 versão 4.0 (01 OCT 1992,
COBOL 85 versão 4.2, Apr 1993 SSVG), a suíte que o National Institute of
Standards and Technology dos Estados Unidos usava para certificar compiladores
COBOL. São 28 MB, 348,271 linhas, **459 programas COBOL** e 51 membros de
copybook, e ela vive neste repositório em `NIST/newcob.val,cbl`.

Ela é a fonte da verdade. Onde RustCOBOL e CCVS85 divergem, **o CCVS85 está certo
e o RustCOBOL está errado**, e a diferença é registrada como defeito em
[`specs/nist/`](../specs/nist/README.md) — uma especificação por correção, com os
programas que falham nomeados.

### O placar

Medido em 2026‑08‑28 na versão 1.62.43, sobre a distribuição intocada:

| | Programas | Fatia | Significado |
|---|---:|---:|---|
| ✅ **PASS** | **422** | **97.2 %** | dos 434 programas dentro do escopo |
| ❌ **FAIL** | **12** | 2.8 % | dos 434 programas dentro do escopo |
| ⬜ **N/A** | **25** | — | módulos fora do escopo do RustCOBOL (abaixo) |
| | **459** | | total de programas da suíte |

Reproduza assim:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ Compilar é a afirmação mais fraca

A tabela acima conta os programas que o **front end aceita**. Ela não diz que
eles executam. A suíte se pontua sozinha — todo programa do CCVS85 imprime o seu
próprio relatório `PASS` / `FAIL*` — então existe um segundo número, estritamente
mais forte: quantos rodam até o fim e relatam **zero falhas**.

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

Os dois números são relatados por módulo, e nunca são misturados:

| Módulo | Compilação | Execução (0 falhas) |
|---|---:|---:|
| **NC (Núcleo)** | **95 / 95** | **83 / 95** |

O trabalho avança **um módulo de cada vez**: NC só está terminado quando os dois
números chegam a 95, e nenhum outro módulo é trabalhado até lá. Uma pontuação
ampla de compilação espalhada por dez módulos não diz nada sobre algum deles
funcionar.

##### Os cinco membros de NC que precisam de mais que um arquivo de impressão — todos pontuados

A pontuação de execução considera um programa limpo quando o **relatório CCVS
dele mesmo** não mostra falhas. Cinco membros de NC não imprimem tal relatório, e
não porque algo esteja quebrado. Cada um exigia trabalho no arcabouço de testes,
não no compilador, e cada um já pontua:

| Membro | Do que precisa | Como é pontuado |
|---|---|---|
| **NC302M**, **NC303M**, **NC401M** | Testes de *sinalização (flagging)*. Não carregam maquinaria `PASS`/`FAIL` alguma — cada um termina com `TOTAL NUMBER OF FLAGS EXPECTED = n`, e o resultado que está sendo validado é o conjunto de **diagnósticos que o compilador emite** para construções obsoletas (NC302M/NC303M) ou para construções acima do subconjunto alto (NC401M). | O arcabouço compara os diagnósticos com a lista de expectativas do próprio membro, linha a linha. As duas classes rodam em **passagens separadas**: `DATE-COMPILED` é ao mesmo tempo obsoleto *e* acima do subconjunto alto, então uma única passagem combinada daria a cada membro as sinalizações do outro como falsos positivos. |
| **NC110M** | Escreve o relatório com `DISPLAY`, no console do operador, e não no arquivo de impressão CCVS que o arcabouço lê. | A saída de console do processo filho é capturada num arquivo e pontuada a partir dali. |
| **NC109M**, **NC204M** | Testam o `ACCEPT` de Formato 1, que lê do operador — NC109M escrevendo-o puro, NC204M através de um mnemônico que `SPECIAL-NAMES` associa ao dispositivo de entrada. Espera-se que o validador forneça a entrada; sem stdin toda comparação falha. | O arcabouço fornece um maço do operador no stdin do processo filho. O maço é **recuperado do fonte, não inventado**: cada item aceito é comparado com um item pareado cujo valor o programa define logo acima do `ACCEPT`, então toda linha do maço é esse valor. |

Portanto **não há teto estrutural abaixo de 95** no eixo de execução: todo
programa NC dentro do escopo compila, e cada um deles é pontuado por aquilo que
ele próprio relata.

O caso comparável que **de fato** foi resolvido é a chave externa. NC174A, NC253A
e NC254A testam `ON STATUS` / `OFF STATUS` contra uma chave que o operador ajusta
antes da execução — nada dentro do COBOL consegue ajustar uma — então o
arcabouço agora passa `--switch XXXXX051=ON --switch XXXXX052=OFF` (e as grafias
substituídas `SWITCH-1` / `SWITCH-2`) exatamente como as instruções de execução
do CCVS85 exigem. Isso é a configuração que o procedimento de validação pede, não
um dedo na balança: um programa que não declara chave alguma não é afetado.

#### ⚠️ O que PASS realmente significa — leia isto antes de citar o número

Um programa conta como **PASS** quando atravessa o front end do RustCOBOL —
lexer, parser, analisador semântico — com **zero erros**, usando
`--source-format=fixed`.

Isso é conformidade de *compilação*. **Não** é prova de que o programa calcula a
resposta certa. Um programa do CCVS85 também imprime a sua própria contagem
`PASS`/`FAIL` quando executa, e pontuar essa saída é a **próxima etapa** deste
trabalho — ela não está incluída nos 332 — veja o placar de execução abaixo. Dois
casos medidos mostram por que a distinção importa:

- 30 dos 35 programas de arquivo RELATIVE já compilaram limpos enquanto o runtime
  **não tinha motor RELATIVE nenhum** — eles executavam e produziam resultados
  errados silenciosamente. Essa lacuna está **fechada**: o motor chegou na 1.62.76
  e o módulo ficou terminado na 1.62.77 (35 / 35 em compilação, 34 / 34 em
  execução). Ele é mantido aqui como o exemplo mais claro do que o eixo de
  compilação sozinho não consegue lhe dizer.
- Um literal continuado por duas linhas pode ser remontado errado e ainda assim
  ser analisado, deixando o programa com os dados errados.

Ou seja: **PASS = "o RustCOBOL aceita toda construção deste programa."** Nada
além disso, por enquanto.

#### 🔴 O placar de execução — o número que significa "funciona"

Tudo acima mede **compilação**. Um programa do CCVS85 também *executa* e imprime
a sua própria contagem `PASS`/`FAIL`, e é essa contagem que a suíte existe para
produzir. Desde 1.62.15 o arcabouço os executa:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

Medido em 2026‑08‑28 na 1.62.43. Sob a REGRA DE OURO nº 9 um módulo é terminado
antes de o próximo começar: **NC (Núcleo) está completo nos dois eixos**, então
**SQ (E/S sequencial)** é o módulo em andamento.

**NC — Núcleo**

| | Programas |
|---|---:|
| dentro do escopo | 95 |
| não compilaram | 0 |
| rodaram até o fim | 95 |
| **…relatando 0 falhas** | **95** |
| …relatando falhas | 0 |
| rodaram mas não imprimiram relatório | 0 |
| estouraram o tempo (>20 s) | 0 |
| quebraram ou foram recusados pelo runtime | 0 |

As asserções que os próprios programas relatam: **4 614 PASS / 0 FAIL**, 100 % de
4 614 pontuadas. (Outras 5 são `DELETED` — o marcador do próprio CCVS para um
teste que o programa mesmo pula.)

Para contraste, a mesma tabela na 1.62.23 dizia 65 limpos de 95, 4 278 PASS /
226 FAIL. O que se fechou foi a distância entre "compila" e "funciona".

**SQ — E/S sequencial (em andamento)**

| | Programas |
|---|---:|
| dentro do escopo | 85 |
| não compilaram | 0 |
| rodaram até o fim | 83 |
| **…relatando 0 falhas** | **84** |
| …relatando falhas | 1 |
| rodaram mas não imprimiram relatório | 0 |
| estouraram o tempo (>20 s) | 0 |
| saída desgovernada (>2 MB) | 0 |
| quebraram ou foram recusados pelo runtime | 0 |

Asserções: **623 PASS / 1 FAIL**, 99.8 % de 624 pontuadas, e **todo programa roda
até o fim**. Na 1.62.42 a mesma tabela dizia **10** limpos de 85, 20 quebrados, 1
com tempo estourado e 215 PASS / 190 FAIL — o agrupamento de quebras era um único
defeito, parágrafos declarativos perdendo os seus nomes; na 1.62.43 ela dizia 44
limpos e 471 PASS / 162 FAIL. Registros de comprimento variável, a área de
registro compartilhada, larguras de `FILLER`, `READ … INTO` e o `REWRITE`
sequencial chegaram na 1.62.44; o `USE` qualificado por modo, `CLOSE REEL/UNIT`,
`SELECT OPTIONAL`, `LINAGE-COUNTER` no `OPEN` e comprimentos de registro fora de
faixa na 1.62.45; valores de `LINAGE` dados por nome de dado e os detectores de
sinalização de E/S sequencial na 1.62.46.

Um membro ainda está aquém:

| Membro | O que falta |
|---|---|
| SQ203A | Precisa de `XXXXD001`, um arquivo de dados que a **instalação** do CCVS85 fornece. Nenhum membro da suíte o escreve, então a metade "arquivo presente" do seu teste de `SELECT OPTIONAL` não pode rodar aqui; a metade "arquivo ausente" passa. Isso é uma entrada de instalação ausente, não um defeito do RustCOBOL. |

> Uma linha de detalhe `FAIL*` é escrita **duas vezes** de propósito — o
> `PRINT-DETAIL` do CCVS roda
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — enquanto `PASS ` é escrito
> uma vez só. Qualquer contagem crua de marcadores tirada do arquivo de impressão
> tem de dividir as falhas por dois antes de significar alguma coisa.

Para ler *por que* um programa falha, uma terceira passagem imprime o detalhe da
falha que o relatório dele carrega, pronto para agrupar por módulo inteiro:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> É por isso que o número de compilação é sempre relatado como "o RustCOBOL
> **aceita** estas construções". Citá-lo como nível de conformidade seria errado.

#### Por módulo

| Módulo | O que ele testa | PASS / Total | |
|---|---|---:|---|
| NC | Núcleo | **95 / 95** | ✅ completo — e completo em **execução** também (veja o placar acima) |
| SQ | E/S sequencial | **85 / 85** | ✅ completo na compilação; **44 / 85 na execução** — o módulo em andamento |
| IC | Comunicação entre programas | 45 / 47 | `END-CALL` chega ao despachante de comandos em vez de ser consumido pelo seu `CALL`; um nome de condição subscrito |
| IF | Funções intrínsecas | **45 / 45** | ✅ completo |
| IX | E/S indexada | **42 / 42** | ✅ completo |
| SG | Segmentação | **13 / 13** | ✅ completo |
| ST | Ordenação / Intercalação | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | E/S relativa | 35 / 35 | ✅ **terminado nos dois eixos** (1.62.77) — execução 34 / 34, 354 asserções, 0 falhas. Um motor de verdade (`cobolt-runtime/src/relative.rs`, contêiner `PRCREL1`) chegou na 1.62.76; todos os sete verbos de arquivo despacham com base em `FileOrganization::Relative`. RL301M fica excluído da execução pelo mesmo critério que IX301M e continua contando no censo de compilação, onde passa |
| SM | Manipulação do texto fonte (COPY/REPLACE) | 14 / 17 | um `$` dentro de um nome de dado; pseudotexto qualificado/subscrito; uma forma de `PERFORM … VARYING` |
| DB | Depuração | 11 / 15 | `GO-TO` usado como palavra definida pelo usuário, colidindo com o par de palavras reservadas `GO TO`; um programa usa o verbo de Comunicação `DISABLE` |
| **Dentro do escopo** | | **422 / 434** | |
| CM | Comunicação | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | Sinalização de recursos obsoletos | — | ⬜ N/A |
| EXEC85 | O próprio programa condutor COBOL do NIST | — | ⬜ N/A |

### ⬜ N/A — o que está fora do escopo do RustCOBOL, e por quê

Estes 25 programas **não são contados como falhas**. São recursos que o RustCOBOL
não implementa e não pretende implementar. O raciocínio completo está em
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Módulo | Programas | Por que está fora do escopo |
|---|---:|---|
| **CM** — Comunicação | 9 | `COMMUNICATION SECTION`, entradas `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Mira nos monitores de teleprocessamento dos anos 1980 — filas de mensagens de propriedade de um gerenciador de transações. Não existe runtime desse tipo aqui, e o módulo foi removido dos padrões COBOL posteriores. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entradas `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, quebras de controle. Uma sublinguagem declarativa extensa; a resposta do PowerRustCOBOL para relatórios é o Designer de Formulários e a exportação em PDF. Poderia virar um *recurso* mais adiante, se quisermos — é a única exclusão com valor real para o usuário. |
| **OBSQ / OBIC / OBNC** | 9 | Estes retestam módulos anteriores e esperam que o compilador *sinalize* elementos obsoletos do COBOL‑85. O conteúdo de linguagem deles está coberto pelas especificações dentro do escopo; o que está fora do escopo é a **sinalização** de recursos obsoletos. |
| **EXEC85** | 1 | Não é um teste. É o executivo COBOL do próprio NIST que fatia a distribuição e conduz a suíte — substituído aqui por um arcabouço em Rust, então ele não precisa compilar. |

**COBOL orientado a objetos** também está fora do escopo do RustCOBOL, mas o
CCVS85 é inteiramente anterior a ele — não há programas OO na suíte.

### De onde vêm as 192 falhas restantes

Cada uma é um defeito especificado, não uma incógnita. Ordenadas pelo número de
programas em que ela é o *primeiro* erro:

| Programas | Causa raiz | Especificação |
|---:|---|---|
| 31 | vírgula separadora — `MOVE ZERO TO A, B, C` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [intrínsecas](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [lacunas de comandos](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | subscritos separados por espaço — `TBL (1  2)` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON` (nomes de chave) e `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md), [separadores](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [lacunas de comandos](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | `COPY` fundo na Área B ou partido entre linhas | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | ponto e vírgula separador — `START F ; INVALID KEY` | [separadores](../specs/nist/NIST-spec-separators.md) |
| 4 | inteiro do `OCCURS` na linha seguinte | [separadores](../specs/nist/NIST-spec-separators.md) |
| 4 | `SECTION` com número de prioridade — `SORT-PARA SECTION 69.` | [segmentação](../specs/nist/NIST-spec-segmentation.md) |

> **A classificação se mexe depois de cada correção, e os movimentos são
> informativos.** Três linhas que lideravam esta tabela em versões anteriores
> sumiram — as entradas de comentário de IDENTIFICATION, os literais numéricos e
> a aspa solta. Em cada vez, a maioria dos programas da linha liberada **não**
> passou a ser aprovada; eles se mudaram para a linha de baixo. Os quatro
> programas de SG liberados na 1.62.12 hoje param em `SORT-PARA SECTION 69.`, que
> é por isso que Segmentação ainda marca 0 / 13. Meça de novo em vez de confiar
> numa classificação anterior.

### Histórico de conformidade

| Versão | PASS / 434 | O que mudou |
|---|---:|---|
| 1.62.7 | **0** | Nada compilava. Faltavam duas regras do formato de referência clássico: as colunas 73‑80 eram lidas como código-fonte, e as linhas de continuação nunca eram juntadas. |
| 1.62.8 | **222** | `--source-format=fixed` — o formato de referência clássico, inclusive a continuação. Veja [Formatos de fonte](#formatos-de-fonte). |
| 1.62.10 | **237** | Literais numéricos podem começar por um ponto decimal (`.999`). Funções intrínsecas 21 → 29, Núcleo 25 → 29, Ordenação/Intercalação 27 → 30. |
| 1.62.11 | 241 | Parágrafos de entrada de comentário de IDENTIFICATION. Depuração 5 → 9. Um ganho menor do que o balde de 32 programas sugere: 9 deles são programas de Comunicação (N/A), e a maioria do restante esbarrava num segundo bloqueio logo em seguida. |
| 1.62.12 | 242 | Um literal fica confinado à sua linha, então uma aspa solta não pode mais virar a paridade de um arquivo inteiro. Núcleo 29 → 30. O balde de 6 programas esvaziou: 4 seguiram para os números de prioridade de segmento, 1 já passa. |
| 1.62.13 | 292 | A vírgula e o ponto e vírgula separadores são pontuação, não tokens; subscritos podem ser separados só por espaços; um subscrito pode vir depois de um nome qualificado completo; um delimitador dobrado dentro de um literal é um caractere só. Núcleo 30 → 56, Entre programas 32 → 44, Indexada 31 → 38. Três baldes inteiros de diagnóstico esvaziaram. |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — uma tabela inteira como argumento de intrínseca; `MOVE ALL "X"` preenche o campo; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`; um literal com sinal como objeto de `WHEN`; `PERFORM … TIMES` com a contagem num item de dados; uma contagem inteira escrita numa linha de continuação. **Funções intrínsecas 45 / 45 — módulo completo.** |
| 1.62.15 | 332 | Um nome de `FUNCTION` desconhecido é erro de compilação em vez de devolver 0; uma palavra definida pelo usuário pode começar por dígito (`25COUNT`, `3-DEM-TBL`, `0 SECTION.`); uma linha `D` é comentário a menos que haja `WITH DEBUGGING MODE`. Segmentação 0 → 10, Núcleo 58 → 61. |
| 1.62.16 | 376 | O `AT` de `AT END` é opcional, então uma cláusula `END` sozinha não engole mais o cabeçalho do parágrafo seguinte (33 programas). O pré-processador COPY/REPLACE confina um literal à sua linha, então a palavra COPY no banner de copyright não é uma diretiva. Um literal numérico pode abrir com o seu ponto decimal uma lista de operandos de `ADD`/`SUBTRACT`. **E/S indexada completa, 42 / 42.** |
| 1.62.17 | 380 | O leiaute de página do `LINAGE`, o `LINAGE-COUNTER` e `WRITE … AT END-OF-PAGE` / `AT EOP` — implementados, não simulados. E/S sequencial 77 → 81. |
| **1.62.19** | **396** | Um item numeric-edited é um item numérico. O ponto decimal de edição mantém o dígito que vem depois dele (`PIC ZZ,ZZZ.9` não trunca mais para `ZZ,ZZZ`), e uma picture montada só com caracteres de edição — `ZZZZ`, `$.**`, `$**.**CR` — é numeric-edited e não alfanumérica. As duas coisas faziam um receptor `GIVING` aritmético legal parecer não numérico. |
| **1.62.18** | **391** | Um número abrindo uma linha de continuação é um operando onde se espera uma expressão. O `IS` é opcional numa condição de classe ou de sinal, e uma condição pode ser sujeito de `EVALUATE`. Um nome de procedimento pode ser escrito inteiramente com dígitos, tanto em referências quanto em cabeçalhos. |
| **1.62.21** | **417** | A passagem do Núcleo. `ALTER` é uma série e `GO TO.` é o GO TO alterado; um nome de procedimento todo em dígitos preserva os zeros à esquerda; um nome de condição pode ser subscrito ou qualificado; uma expressão aritmética entre parênteses é um operando, não uma condição aninhada; `MULTIPLY`/`DIVIDE` no formato 1 aceitam uma série de receptores; `WITH TEST` pode vir antes de `VARYING` e uma contagem de repetições pode ser subscrita; `PERFORM imperativo … END-PERFORM` não precisa de cláusula alguma; um nome de parágrafo pode ser qualificado pela sua seção; o `ELSE` não é engolido por um imperativo de `ON SIZE ERROR` nem por um ramo ELSE aninhado; relações combinadas abreviadas aceitam objetos aritméticos e de classe/sinal; `INSPECT` leva a sua categoria ALL/LEADING entre operandos e `CONVERTING` aceita uma região; `UNSTRING TALLYING` vem depois de `WITH POINTER`. **Núcleo 76 → 92 de 95 compilando, 16 → 28 executando limpos.** |
| **1.62.43** | **422** | **O módulo de E/S sequencial compila por inteiro — 85 de 85 — e vai de 10 a 44 de 85 na execução.** Os parágrafos de um declarativo mantêm os seus nomes, então um tratador `USE` pode dar `PERFORM` e `GO TO` neles (20 programas pararam de quebrar); um item `FILE STATUS` declarado como *grupo* de dois caracteres recebe o código; o `OPEN` de um arquivo já aberto é `41` e não o reabre; um `READ` sequencial depois de `AT END` é `46`; e um mesmo `OPEN` pode carregar vários grupos de modo (`OPEN INPUT f1 OUTPUT f2`), o que é todo o ganho de compilação. |
| **1.62.42** | **420** | **O módulo Núcleo está terminado — 95 de 95 compilando *e* 95 de 95 executando limpos, 4 614 asserções sem nenhuma falhando.** Um `66 RENAMES` é qualificado pelo seu registro, cobre toda ocorrência de uma tabela que ele alcança, e é o item que renomeia quando renomeia exatamente um; um 88 declarado sobre um grupo testa os bytes do grupo; uma constante figurativa é dimensionada pelo outro operando, `VALUE` incluído; um operando de grupo é da categoria alfanumérica; um `NOT` antes do objeto de uma abreviação nega a relação; uma série `INSPECT … REPLACING` compartilha uma única varredura e um item DISPLAY com sinal não tem um `-` entre os seus caracteres; sobreposições de `REDEFINES` se aninham; e `PERFORM … WITH TEST AFTER VARYING` é honrado, uma variável `AFTER` é reiniciada quando o seu laço acaba, e um identificador `VARYING` subscrito segue o seu subscrito. Esse último grupo é a razão de NC201A ter terminado. |

> **O resumo honesto.** O RustCOBOL aceita hoje **97.2 %** da suíte NIST dentro do
> escopo, vindo de nada nove versões atrás. Os 12 restantes não são misteriosos —
> são defeitos nomeados, cada um especificado com os programas que ele bloqueia.
> Esta tabela é a medida do progresso, e é atualizada a cada versão.
>
> **E um módulo está terminado no eixo que conta.** O Núcleo roda 95 de 95
> programas limpos, e não meramente os compila — veja o placar de execução acima.
> Sob a REGRA DE OURO nº 9 esse é o portão para começar o módulo seguinte, então
> **a E/S sequencial está agora em andamento**: completa na compilação, 44 de 85
> na execução.

---

> **Atualização (passagem de implementação de lacunas):** os seguintes foram
> implementados e agora são ✅ — **modificação de referência** `id(start:len)`,
> **`PERFORM n TIMES` em linha**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING
> `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` ciente da
> categoria**, **condições abreviadas prefixadas por operador** (`a > 1 AND < 9`),
> **`CALL … ON EXCEPTION`** (roda quando o CALL não é resolvido), **múltiplos
> receptores de `COMPUTE` + `ROUNDED` por receptor**, e um conjunto de **funções
> intrínsecas** bem maior.
>
> **Atualização (passagem de ambiente hierárquico / ciente de ocorrências —
> 1.5.0):** quatro recursos bloqueados pelo modelo de dados agora são ✅ —
> **subscrição de tabela em tempo de execução** `t(i)` / `t(i, j)`
> (armazenamento por ocorrência), **desambiguação por nome qualificado**
> `id OF/IN group` (nomes de folha duplicados resolvem para armazenamentos
> independentes), **`MOVE/ADD/SUBTRACT CORRESPONDING`**, e **`SEARCH` /
> `SEARCH ALL` funcionais**.
>
> **Atualização (passagem de completude de verbos — 1.6.0):** agora também ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` com múltiplos receptores + `ROUNDED` por
> receptor** em `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** e o `EXIT` simples corrigido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado e as regiões
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de data/finanças
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condições abreviadas com objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-sujeito) e **`WHEN NOT`**;
> **nomes de condição de nível 88 de verdade** (`SET … TO TRUE/FALSE`, o
> hospedeiro é testado contra os seus VALUE/faixas); **`PERFORM para VARYING`**; e
> um runtime **`SORT`/`MERGE`** funcional (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). A lista de "evitar" no fim está atualizada.
>
> **Atualização (passagem de limpeza da lista de "evitar" — 1.7.0):** as lacunas
> restantes já estão implementadas — **abreviação com objeto identificador**
> (`a = b OR c`, resolvida por metadados de nível 88);
> **`INITIALIZE … REPLACING category DATA BY value`**; **`66 RENAMES`** (a leitura
> sintetiza / a escrita distribui pelos itens cobertos); **ponteiros**
> (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`, aliasing com
> `SET ADDRESS OF item TO …`, `IF ptr = NULL`); **`ALTER`** / **`UNLOCK`**; um
> **`NEXT SENTENCE`** fiel; as **intrínsecas** padrão que faltavam
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`); e o
> **`ACCEPT`/`DISPLAY` de tela** estendido (`AT`/`WITH` via ANSI no modo CLI —
> agora *executado*, não apenas analisado).
>
> **Atualização (1.7.1):** as fontes de registrador do `ACCEPT` agora são
> funcionais (eram no-ops reconhecidos) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (pareados com
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (pareado com
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Atualização (1.7.2):** cláusulas de compartilhamento / travamento de arquivo e
> `CANCEL` (eram ❌ / no-op) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libera as travas de registro INDEXED
> do arquivo), e **`CANCEL program`** (reinicializa o armazenamento do programa).
>
> **Atualização (1.8.0):** **`COMMIT` / `ROLLBACK`** agora são verbos COBOL de
> verdade — transações controladas pelo programa sobre os arquivos INDEXED
> abertos (tanto no motor de memória quanto no de disco). O motor de disco ganhou
> um log de desfazer real dentro da execução (antes era um no-op). A lista de
> "evitar" no fim está atualizada.

---

## Parágrafos da IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Os parágrafos de **entrada de comentário** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — em **qualquer ordem e em
  qualquer subconjunto**.
- ✅ `REMARKS` também é aceito. Ele foi removido do COBOL em 1985, portanto não
  é armazenado; é aceito para que o código herdado do COBOL‑74 continue
  compilando.

Uma **entrada de comentário** é texto livre, e o COBOL‑85 diz isso ao pé da
letra:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Ela pode conter **palavras reservadas** — o `DATA` acima não abre uma DATA
  DIVISION.
- Ela pode conter **pontos**, e não termina em um deles.
- Ela **se estende por quantas linhas** você escrever.
- Ela termina no próximo cabeçalho de parágrafo ou de divisão que **comece uma
  linha** na Área A — é assim que a entrada acima termina em `DATE-WRITTEN`.

**Uma aspa dentro dessa prosa fica contida na sua linha** (desde 1.62.12). Um
texto como `THE COMPILER"S ABILITY` não abre mais um literal que avança pelo
resto do programa — veja [Formatos de fonte](#formatos-de-fonte). Ainda vale a pena
evitar uma aspa sem par em uma entrada de comentário, mas agora ela custa
aquela linha, não o arquivo.

⚠️ `INSTALLATION`, `SECURITY` e `REMARKS` **não são palavras reservadas** aqui.
Eles só são reconhecidos como nomes de parágrafo dentro da IDENTIFICATION
DIVISION, de modo que um item de dados chamado `SECURITY` continua funcionando.

---

## Formatos de fonte

O RustCOBOL lê três disposições de fonte. A escolha é explícita — ela **nunca** é
adivinhada a partir do conteúdo do arquivo, porque aplicar regras de coluna a um
fonte que não foi escrito para elas apaga código silenciosamente.

| `--source-format` | O que significa |
|---|---|
| `free` | Nenhuma regra de coluna. `*>` inicia um comentário. **O padrão**, e o que os próprios projetos do PowerRustCOBOL e os arquivos `.cbl` de formulário gerados usam. |
| `fixed` | ✅ **Formato de referência clássico do COBOL-85** — a disposição que o padrão define e na qual o fonte em imagem de cartão é escrito. Veja abaixo. |
| `fixed-relaxed` | A área de sequência e a coluna indicadora são respeitadas, mas a linha vai até onde você a digitou — sem limite de 72 colunas. |
| `auto` | Comportamento histórico: `free`, a menos que `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` define o padrão de uma sessão.

### `fixed` — o formato de referência clássico

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Colunas 1-6** — área do número de sequência, ignorada.
- **Coluna 7** — área indicadora:
  - `*` ou `/` → linha de comentário
  - `-` → **continuação** da linha anterior
  - `D` → linha de depuração; um comentário (o modo de depuração ainda não está
    implementado)
  - qualquer outra coisa → lida como fonte comum. O padrão reserva esta coluna,
    mas as suítes em imagem de cartão a usam como seletor de linhas opcionais, e
    descartar essas linhas silenciosamente apagaria código.
- **Colunas 8-72** — o fonte.
- **Colunas 73-80** — área de identificação, **descartada**.

### Linhas de continuação ✅

Um hífen na coluna 7 continua a linha anterior.

**Continuar uma palavra ou um literal numérico** — os espaços finais da linha
continuada são descartados e as duas metades se encontram sem nada entre elas:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

declara um único item chamado `WRK-DS-18V00-CONTINUED`.

**Continuar um literal alfanumérico** — o literal da linha continuada não tem
aspa de fechamento; a linha de continuação precisa reabrir com uma, e o literal
recomeça no caractere seguinte a ela:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **O fragmento continuado vai até a coluna 72, espaços finais incluídos.** Uma
linha que para antes da coluna 72 mesmo assim contribui com esses espaços para o
literal. É por isso que um literal continuado só é exato byte a byte sob
`fixed`; os demais formatos não têm uma coluna 72 onde parar.

### Um literal nunca atravessa uma linha por acidente ✅

A continuação é a **única** maneira de um literal alcançar várias linhas. Uma
aspa que não é fechada na própria linha é um erro, relatado onde ela está
escrita:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Isso importa mais do que parece. Antes de 1.62.12 uma aspa sem par ia até a
*próxima* aspa em qualquer ponto do arquivo, de modo que uma única `"` perdida
em um comentário engolia divisões inteiras e deslocava o pareamento de todas as
aspas seguintes — os programas NIST em que isso foi encontrado têm um número
**par** de aspas, então nada ficava sem terminação; um único caractere havia
deslocado a paridade do arquivo inteiro. O estrago agora para na quebra de
linha.

> **O formato livre não tem continuação de literal.** Nem `&` — esse é o
> *operador* de concatenação — nem um bloco delimitado. Um literal em formato
> livre precisa caber em uma linha; para um literal longo, concatene:
> `"first part" & "second part"`.

> **Nota.** Escolher `fixed` para um arquivo que foi escrito em formato livre vai
> danificá-lo — tudo o que passa da coluna 72 some, e o texto antes da coluna 8 é
> lido como número de sequência. Só use isso para fonte que realmente seja imagem
> de cartão.

---

## Instruções reconhecidas (verbos)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redireciona o `GO TO` de para-1) ·
`UNLOCK file` (libera os bloqueios de registro do arquivo) ·
`OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (compartilhamento/bloqueio de arquivos — consultivo
dentro da única unidade de execução)
✅ `COMMIT` / `ROLLBACK` (transações de arquivo INDEXED controladas pelo
programa — veja Verbos de arquivo) · `CANCEL` (reinicializa o armazenamento do
programa) ·
⚠️ `INVOKE` (analisado como operação nula)
Extensões do projeto: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Um bloco pode fazer `use` das crates sempre linkadas (std, egui, eframe
e o conjunto do runtime linkado) **mais qualquer crate que o projeto registre em
Project's Crates** (especificação 044): as crates registradas são fixadas em uma
versão exata, copiadas (vendoring) para o `crates/` do projeto e compiladas
dentro do binário; crates não registradas fazem o Check/Build falhar na linha do
desenvolvedor, com a correção indicada.

✅ `SEARCH` (sequencial) / `SEARCH ALL` (busca binária sobre uma tabela com
`ASCENDING`/`DESCENDING KEY` — executa o primeiro `WHEN` que casar; caso
contrário, `AT END`).
✅ `SORT` / `MERGE` com `RELEASE` / `RETURN` (funcionais — veja abaixo).
✅ `DECLARATIVES … END DECLARATIVES` com `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — tratadores de erro de arquivo
disparados diante de um `FILE STATUS` de erro não tratado. **Um tratador é
entrado pelo topo da sua seção e roda até o fim dessa seção**, e seus parágrafos
mantêm seus nomes, de modo que ele pode fazer `PERFORM` e `GO TO` neles —
inclusive em um parágrafo de *outra* seção declarativa. Os parágrafos
declarativos vivem no seu próprio espaço de nomes: o controle nunca cai do corpo
principal para dentro deles, e um nome declarado nos dois lugares resolve para a
cópia da declarativa enquanto um tratador está rodando e para a do corpo em
todos os outros pontos. Uma declarativa também pode fazer `PERFORM` de um
parágrafo da parte não declarativa.
❌ **Não reconhecidos — não use:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas suportadas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (vários receptores).
- ✅ **Um operando de grupo torna alfanumérica a movimentação inteira** (COBOL-85 6.18.4).
  A PICTURE do outro operando contribui com o seu *tamanho* e nada mais: sem edição,
  sem des-edição, sem conversão numérica. `MOVE <group holding "123ABC">`
  deixa `"123ABC "` em um `PIC 0XXXXX0` (não o editado `"0123AB0"`), os mesmos
  seis caracteres e um espaço em um `PIC 9999V999`, e `"12"` em um `PIC 99`.
  `JUSTIFIED RIGHT` continua decidindo qual ponta é preenchida e qual se perde.
  A mesma regra rege os bytes do próprio grupo: cada filho toma a sua
  fatia literalmente, de modo que um filho alfanumérico editado **não** é reeditado.
- ✅ **Uma cláusula `VALUE` sobre um grupo** inicializa os bytes do grupo e é
  distribuída entre os seus filhos — `01 G VALUE "$123.45". 02 E PIC $999.99.`
  deixa `E` com `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — move cada item subordinado que os dois grupos
  compartilham pelo nome, descendo recursivamente pelos subgrupos que casam.
- ✅ **`CORRESPONDING` exclui um item descrito com `REDEFINES` ou `RENAMES`**
  (COBOL-85 6.18.4 GR1), de qualquer um dos dois lados, junto com tudo que lhe é
  subordinado. A exclusão recai sobre a *declaração*, não sobre o nome: um item comum que
  apenas compartilha o nome com um nível 66 de outro lugar continua correspondendo.
- ✅ **Qualquer um dos dois operandos de `CORRESPONDING` pode nomear uma ocorrência de uma
  tabela de grupos** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` escreve nos
  espaços daquela ocorrência, e o subscrito é levado adiante pela recursão.
- ✅ **A um par basta que UM dos seus dois itens seja elementar.** Um grupo pode
  ficar diante de um item elementar, e a movimentação entre eles é alfanumérica: um
  item elementar `PIC XXX` enviando para um grupo de `999` + `XXX` preenche os seus seis
  caracteres, e um grupo de `XXX` + `99` enviando para um simples `X(5)` o preenche.
  Dois grupos frente a frente continuam **recursando** — esse par não é o caso
  elementar. *(Antes de 1.62.39 nenhuma das duas direções movia coisa alguma: um
  grupo não possui espaço de armazenamento, então a escrita ia para onde ninguém a lê de volta e
  a leitura devolvia a cadeia vazia.)*
- ✅ **Modificação de referência `id(start:len)`** — emissor (subcadeia) e receptor
  (atribuição parcial emendada); funciona sobre os operandos de todos os verbos. `length` é opcional.
  Ela endereça **posições de caractere**, então um operando numérico é tomado com toda a
  largura da sua `PIC` e com os seus zeros à esquerda: `01 T PIC 9(8) VALUE 00224845` dá
  `T(1:2)` = `"00"`, não `"22"`.
- ✅ **Itens de grupo são agregados alfanuméricos** — um grupo *é* os seus itens
  subordinados postos um após o outro, e o seu tamanho é a soma dos deles. Ler um
  concatena os filhos (inclusive o `FILLER`); mover para um distribui os
  bytes entre eles pela largura. `MOVE 11 TO A` é visível através do grupo que
  contém `A`, e `MOVE "1234" TO G` ajusta os filhos de `G`, não um espaço próprio dele.
- ✅ subscritos `t(i)`, `t(i, j)` — leem/escrevem o espaço de armazenamento de cada ocorrência;
  subscritos variáveis `t(WS-I)` são avaliados a cada acesso.
- ✅ qualificação `id OF/IN group` (`… OF g1 OF g2`) — resolve para o item
  correto mesmo quando o nome da folha está declarado sob mais de um grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por receptor** — cada receptor carrega o seu próprio indicador `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico que
  casa, descendo recursivamente pelos subgrupos que casam.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **vários receptores `GIVING`**, cada um com o seu próprio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sem `GIVING`) guarda `a/b` de volta em `a` (uma comodidade do
  PowerRustCOBOL; o COBOL padrão exige aqui `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **vários receptores, cada um com o seu próprio `ROUNDED`**.
- ✅ operadores de expressão `+ - * /` e `**` (potência, associativa à direita), parênteses,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **`ALSO` com vários sujeitos** — cada coluna de `WHEN` é comparada posicionalmente
  com o seu sujeito e combinada com AND.
- ✅ **`WHEN NOT value`** nega um objeto de seleção; **`WHEN condition`**
  (p. ex. `EVALUATE TRUE WHEN a > b`) avalia a condição booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal inteiro ou item de dados).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` em linha,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` em linha (sem parágrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — executa o parágrafo a cada
  iteração (fora de linha, sem `END-PERFORM`).
- ✅ **`WITH TEST AFTER` se aplica a `VARYING`**, escrito de qualquer um dos lados da
  frase e tanto em linha quanto fora de linha. O corpo roda uma vez antes de qualquer coisa
  ser testada, e as condições são então testadas **da mais interna para fora**; o nível cuja
  condição é falsa é incrementado, todos os níveis internos reiniciam no seu valor `FROM`,
  e o corpo roda de novo. Uma variável só é incrementada quando o seu teste dá
  falso, então o teste que encerra o laço a deixa como o corpo a deixou.
- ✅ **Uma variável de `AFTER` volta ao seu valor `FROM` quando o seu laço termina**,
  antes de o nível imediatamente acima ser incrementado (COBOL-85 6.20.4 GR10(d)). Depois do
  `PERFORM` inteiro, as variáveis internas trazem os seus valores `FROM` e só a
  mais externa guarda o valor que o encerrou.
- ✅ **Um identificador de `VARYING` com subscrito acompanha o seu subscrito.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` incrementa
  a ocorrência que `S1` selecionar naquele momento, então um corpo que avança `S1`
  percorre a tabela.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **O qualificador `{OF|IN} section` escolhe de qual cópia se fala** quando um
  nome de parágrafo se repete em várias seções, exatamente como acontece no `PERFORM`. Uma
  seção **desconhecida** recai na busca sem qualificação em vez de perder
  o salto. `GO TO … DEPENDING ON` recebe uma lista simples de nomes e nenhum qualificador,
  e um `GO TO` que um `ALTER` tenha redirecionado segue o redirecionamento — que nomeia
  o seu próprio destino sem rodeios. *(Antes de 1.62.39 o qualificador era analisado e depois
  ignorado, então o salto caía na primeira definição existente em qualquer parte do programa.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ o `EXIT` simples é um ponto de retorno sem efeito; `EXIT PROGRAM` volta ao chamador.
- ✅ `EXIT PERFORM [CYCLE]` (interromper / continuar o PERFORM em linha mais próximo),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfere o controle para além do próximo limite de sentença (o
  analisador insere marcas de limite em cada ponto; fiel, não um mero `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lê do operador** quando `SPECIAL-NAMES` declara
  o mnemônico (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — esse é o Formato 1, idêntico a um `ACCEPT id` simples.
  Um nome que **nenhuma cláusula `SPECIAL-NAMES` declara** mantém a extensão do
  PowerRustCOBOL e lê a **variável de ambiente** com aquele nome. Qual dos
  dois se aplica é decidido pela declaração, nunca pela grafia.
  *(Antes de 1.62.35 a cláusula comum `<implementor-name> IS <mnemonic>` era
  pulada por completo, então todo mnemônico lia uma variável de ambiente que
  nunca fora definida e o item receptor ficava vazio.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` posiciona o cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (a linha de comando inteira) · `FROM ARGUMENT-NUMBER` (contagem de argumentos)
  · `FROM ARGUMENT-VALUE` (o argumento no ponteiro definido por `DISPLAY n UPON
  ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE` (a
  variável nomeada por `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` encerra a instrução (opcional).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` encerra a lista de operandos (opcional), de modo que
  `DISPLAY A END-DISPLAY DISPLAY B` são duas instruções e não uma.
- ✅ formas de tela `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — executadas via posicionamento de
  cursor ANSI + SGR no **modo CLI** (`rcrun`); ignoradas no modo GUI (lá o designer
  de formulários substitui a E/S de SCREEN). `ACCEPT id AT …` posiciona e então lê.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Estouro = a cadeia montada é mais larga que o campo receptor.
- ✅ **Uma frase `DELIMITED BY` rege toda a série de emissores que a precede**,
  não apenas aquele depois do qual ela está escrita:
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` delimita os três e
  constrói `"ABC"`. Uma instrução pode trazer várias frases, cada uma regendo os
  emissores desde a anterior; os emissores após a última frase são tomados por inteiro.
  *(Antes de 1.62.40 só era delimitado o emissor escrito imediatamente antes da
  frase.)*
- ✅ **`INTO` um item de grupo** distribui entre os itens subordinados do grupo.
- ✅ **O resultado é montado byte a byte**, então `STRING HIGH-VALUE` move o
  único byte `0xFF` e ocupa uma posição de caractere.
- ✅ **Extensão — `DELIMITED BY` padrão inteligente** (quando nenhuma frase rege um
  operando): itens alfanuméricos `PIC X`/`A` assumem `SPACES` (o preenchimento
  final é descartado); literais de cadeia, itens numéricos, numérico-editados, resultados de
  `FUNCTION` e expressões assumem `SIZE`. Itens de dados são movidos na sua forma de campo
  (numérico → dígitos com toda a largura da PIC; numérico-editado → caracteres editados).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Estouro = mais campos de origem que
  receptores.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **as duas metades são aplicadas**.
- ✅ `BEFORE/AFTER INITIAL` confina cada frase a uma sub-região do campo.
  (TALLYING acumula sobre o contador, conforme o COBOL.)
- ✅ **Uma série de operandos TALLYING compartilha UMA ÚNICA varredura da esquerda para a direita** (COBOL-85
  6.17.3). Em cada posição de caractere os operandos são tentados na ordem em que
  foram escritos; o primeiro que casa toma a posição e a varredura prossegue
  além dos caracteres que ele consumiu. Assim `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"`
  sobre `"AABA"` dá `t1 = 1, t2 = 1` — escrever os operandos na ordem inversa
  dá `t1 = 3, t2 = 0`. `LEADING` precisa casar a partir da borda esquerda da sua janela sem
  intervalo, então um operando anterior que tome aquela posição encerra a sequência antes de ela começar,
  e `CHARACTERS` conta apenas as posições que nenhum operando anterior reivindicou.
- ✅ **Uma série de operandos REPLACING também compartilha UMA ÚNICA varredura**, pela mesma regra:
  o primeiro operando que casa em uma posição substitui aqueles caracteres e a
  varredura prossegue além deles, de modo que nenhum operando posterior consegue vê-los. A janela
  `BEFORE`/`AFTER` de cada operando é fixada **antes de qualquer substituição**, o que é o que
  permite ancorar um operando em caracteres que outro anterior sobrescreve:

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Aplicadas um operando de cada vez, a primeira frase apagaria o `"L "` no qual a
  segunda está ancorada, e `"BAD"` sobreviveria.
- ✅ **Um item DISPLAY com sinal não tem nenhum `-` entre as suas posições de caractere.** O
  sinal operacional é uma sobreperfuração em um dígito, então
  `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` dá **0** enquanto
  `FOR ALL "5"` dá 1. O sinal é restaurado em seguida, então um `REPLACING` sobre
  os dígitos o deixa intacto. `SIGN IS … SEPARATE CHARACTER` é o caso em que o
  sinal *é* uma posição, e aí ele é contado.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado para MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` põe no item hospedeiro o primeiro VALUE da condição;
  `TO FALSE` põe um valor fora do conjunto de VALUE (melhor esforço — não há cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` e
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — veja **Ponteiros** abaixo.

### INITIALIZE
- ✅ `INITIALIZE id …` — ciente da categoria: numérico / numérico-editado → ZERO,
  todo o resto → SPACES, descendo recursivamente pelos itens de grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — põe cada item
  subordinado daquela categoria com o valor; os demais ficam intocados.

### Ponteiros (USAGE POINTER)
- ✅ `USAGE POINTER` declara um ponteiro (NULL no início).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — faz de `id` um apelido do
  armazenamento do alvo (leituras **e** escritas seguem o apelido); tipicamente um registro
  de LINKAGE. `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ O corpo de `ON EXCEPTION` / `ON OVERFLOW` roda quando o programa chamado não é
  resolvido; o corpo de `NOT ON EXCEPTION` roda quando a chamada **é resolvida**.
- ✅ `CANCEL program …` reinicializa a WORKING-STORAGE do programa nomeado, de modo que o
  seu próximo `CALL` comece do zero.

### Verbos de arquivo (as frases suportadas — a cobertura completa está na suíte de E/S de arquivos)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` são analisados e respeitados onde fazem sentido — são
  consultivos no modelo de uma única unidade de execução.)
- ✅ **Um único `OPEN` pode trazer vários grupos de modo**, cada um com os seus arquivos:
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Cada grupo é aberto no seu próprio
  modo; `SHARING` / `WITH LOCK` / `REGISTERED USER` valem para a instrução inteira.
- ✅ **Um `OPEN` de um arquivo que já está aberto é `41`**, e o arquivo fica como
  estava — a instrução **não** o reabre. (Reabrir um arquivo `OUTPUT`
  truncaria em silêncio o que o programa já tivesse escrito.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensão do
  PowerRustCOBOL) — registra o operador/usuário no log de observabilidade do INDEXED
  (campo `user=` em cada linha de evento da sessão daquele arquivo). Puramente
  observacional; sem autenticação/autorização. Veja
  [`observability-pt.md`](observability-pt.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libera o bloqueio de registro que o motor INDEXED toma em I-O.
- ✅ **`READ … INTO id` é o `READ` seguido de um `MOVE` de grupo.** O registro é
  distribuído entre os itens subordinados do receptor pela largura e cortado na
  largura do próprio receptor, o receptor pode ter subscrito, e a movimentação carrega
  bytes — um registro que contenha um byte que não é um caractere chega intacto.
- ✅ **Cláusula `RECORD` da FD — registros de comprimento variável.** As três grafias:
  `RECORD CONTAINS n CHARACTERS` (fixo), `RECORD CONTAINS n TO m CHARACTERS`
  (variável; a descrição de registro que o `WRITE` nomeia dá o comprimento), e
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (o item de dados *é* o comprimento — definido antes de um `WRITE`, redefinido por um `READ`,
  e limitado à faixa declarada). Uma FD cujos registros `01` diferem em tamanho é
  de comprimento variável diga ela isso ou não. Um arquivo de comprimento variável guarda o
  comprimento de cada registro junto com o registro, então os seus bytes **não** são intercambiáveis
  com os de um arquivo de comprimento fixo; um arquivo de comprimento fixo não muda.
- ✅ **Os registros `01` de uma FD descrevem uma única área de registro.** Um `READ` entrega os
  bytes através de todas as descrições de registro; um `WRITE` envia a área inteira, então o que
  outra descrição de registro pôs onde a escrita tem `FILLER` acaba
  aparecendo.
- ✅ **`FILLER` ocupa os seus bytes em um registro de FD**, e
  `SIGN IS SEPARATE CHARACTER` faz um item DISPLAY com sinal ficar um caractere mais largo
  que as suas posições de dígito.
- ✅ **`LINAGE` da FD aceita nomes de dados além de inteiros** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. A página é
  medida a partir daqueles itens a cada `WRITE`, então um programa pode redimensioná-la enquanto
  roda. `LINAGE-COUNTER` vale um quando o arquivo é aberto.
- ✅ **Um `READ` sequencial depois de `AT END` é `46`, não um segundo `10`.** O
  `AT END` não deixou nenhum próximo registro válido, então continuar lendo é um erro diferente de
  chegar ao fim. `46` é um status de classe 4, então nem `AT END` nem
  `NOT AT END` rodam para ele — quem o trata é a declarativa `USE` do arquivo.
  Um `OPEN` novo, ou um `START` bem-sucedido, estabelece um registro de novo.
- ✅ `UNLOCK f [RECORD[S]]` libera os bloqueios de registro do arquivo.
- ✅ **`COMMIT` / `ROLLBACK`** — transações controladas pelo programa sobre **todos** os
  arquivos INDEXED abertos. `OPEN` inicia uma transação; `COMMIT` confirma os
  `WRITE`/`REWRITE`/`DELETE` pendentes (um `ROLLBACK` posterior já não consegue desfazê-los) e
  inicia outra; `ROLLBACK` desfaz toda mudança desde o último `COMMIT`/`OPEN`.
  O armazenamento **DISK** torna `COMMIT`/`CLOSE` duráveis em disco. O armazenamento **MEMORY**
  mantém `COMMIT`/`ROLLBACK` puramente na RAM (nunca escreve em disco); um arquivo
  `STORAGE IS MEMORY` simples é efêmero, e `STORAGE IS MEMORY WITH PERSISTENCE`
  grava em disco somente no `CLOSE`. (A recuperação após queda por um log de escrita
  antecipada durável fica para depois — isto é uma reversão em nível de programa, dentro da execução.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (arquivos INDEXED; extensão do PowerRustCOBOL). O armazenamento padrão é
  `DISK`. `WITH COMPRESSION` comprime o registro armazenado (as chaves são avaliadas sobre o
  registro não comprimido); `WITH PERSISTENCE` (só com MEMORY) grava no `CLOSE` o arquivo que
  está na RAM. `OPEN OUTPUT` sempre (re)cria o contêiner em disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` em um arquivo SEQUENTIAL de registros** substitui, no lugar, o registro que o
  último `READ` entregou, e deixa a posição de leitura onde estava — o
  próximo `READ` ainda dá o registro que vem depois. Os status que ele deve:
  **`49`** quando o arquivo não está aberto em `I-O`, **`43`** quando nenhum `READ` bem-sucedido
  estabeleceu um registro (inclusive depois de `AT END`, e em um segundo `REWRITE` sem
  `READ` no meio), e **`44`** quando o registro novo não tem o mesmo comprimento que
  o lido — em um arquivo com `DEPENDING ON` o valor do item é esse comprimento, e é assim
  que um programa pede um comprimento diferente.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ O compartilhamento de arquivos entre *processos* não é imposto (uma única unidade de execução); as
  frases `SHARING`/`LOCK` são analisadas e os bloqueios de registro por execução do motor INDEXED
  são respeitados.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcional, com buffer de trabalho em memória)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dentro de um INPUT PROCEDURE) acrescenta à execução;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devolve os registros.
- Os registros são ordenados de forma estável pelas chaves declaradas (`ASCENDING`/`DESCENDING`);
  `USING` lê / `GIVING` escreve os arquivos sequenciais nomeados.

---

## Condições (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionais: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relações por palavras: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Um item cuja PICTURE não carrega **sinal operacional** só é `NUMERIC` quando
  todas as posições de caractere contêm um dígito — um `PIC X(5)` contendo
  `"+1234"`, `"1.234"` ou `"12 45"` **não** é numérico. *(Antes de 1.62.40 o
  teste interpretava os caracteres como um número, então sinal, ponto decimal,
  expoente e espaços ao redor eram todos aceitos.)*
- ✅ **O operando de uma `CLASS` definida pelo usuário pode ser uma posição
  ordinal** — `CLASS ORDINAL-A-ONLY IS 66` nomeia o 66º caractere do conjunto
  nativo — e o operando pode ficar em uma linha de fonte só dele. O mesmo vale
  para `ALPHABET`.
- ✅ Sinal: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nome de condição de nível 88 (o nome sozinho como condição).
- ✅ **`TRUE` / `FALSE` como operandos** (extensão do PowerRustCOBOL) — açúcar
  sintático para `1` e `0`, em qualquer lugar em que um valor seja permitido:
  `IF x = TRUE`, `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (a forma com `NOT`
  sozinho, sem operador relacional), `PERFORM UNTIL x = FALSE`,
  `MOVE TRUE TO x`, `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE` e
  `WHEN TRUE` contra um sujeito que é um valor. Um `TRUE`/`FALSE` sozinho também
  é uma condição completa (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ Isso **não** muda os dois lugares em que essas palavras já significavam
  alguma coisa: `SET <88‑name> TO TRUE` continua colocando no item hospedeiro um
  valor que satisfaz a condição (não o número 1), e `EVALUATE TRUE`/`EVALUATE
  FALSE` mais abaixo continuam sendo a instrução de seleção padrão.
- ✅ `AND` / `OR` / `NOT` combinados, com parênteses (AND liga mais forte que OR).
- ✅ **Condições abreviadas com o operador na frente** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (o sujeito da comparação anterior é reaproveitado).
- ✅ **Abreviação com objeto literal** — `a = 1 OR 2 OR 3` (reaproveita tanto o
  sujeito quanto o operador; o objeto é um literal).
- ✅ **Abreviação com objeto identificador** — `a = b OR c` (onde `c` é um item
  de dados). Um identificador sozinho depois de AND/OR na sequência de uma
  comparação é resolvido em tempo de execução: se for um nome de condição de
  nível 88 conhecido, ele é avaliado como tal; caso contrário, é o objeto de
  `a = c`. (Um identificador imediatamente seguido de `AND` mantém a precedência
  do AND.)
- ✅ **Um `NOT` antes do *objeto* de uma abreviação nega a relação**, não o
  objeto: `a > b OR NOT c` é `a > b OR NOT (a > c)`. A grafia `NOT <relational
  operator>` (`AND NOT < x`) é a forma de operador e continua igual, e um
  `NOT` que abre uma condição comum — `NOT (…)`, `NOT x = y`, `NOT x NUMERIC` —
  mantém o seu próprio significado. *(Antes de 1.62.42 a forma de objeto era
  lida como "o objeto é diferente de zero", o que dá a mesma resposta apenas
  quando o objeto por acaso contém zero.)*
- ✅ **Um nome de condição declarado sobre um grupo testa os bytes do grupo.** Um
  grupo não possui armazenamento próprio — ele *é* os seus filhos —, portanto
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compara contra os
  seis caracteres que o registro contém.
- ✅ **Uma constante figurativa é repetida até o tamanho do outro operando**, e
  isso inclui uma escrita como `VALUE` de um 88: `88 B VALUE QUOTE` sobre um
  hospedeiro `PIC X(4)` são quatro aspas, e `88 D VALUE ALL "BAC"` é `"BACB"`.
  `ALL literal` é dimensionado nas **duas** direções — `IF X EQUAL TO ALL "BA"`
  sobre um `X` de dez caracteres compara contra `"BABABABABA"`, e não contra
  `"BA"` preenchido com espaços.

---

## Expressões, literais, USAGE

- ✅ Operadores aritméticos `+ - * /` e `**`; parênteses; `+`/`-` unários.
- ✅ `FUNCTION nome ( arg [ , arg … ] )` — intrínsecas **implementadas**:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (com semente opcional), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (As conversões de data usam a base padrão 1601‑01‑01 = dia 1.) O **conjunto
  completo de intrínsecas do padrão COBOL‑85** está implementado.
- ✅ **Os registradores de data e hora leem o relógio LOCAL.** `ACCEPT … FROM
  DATE / TIME / DAY / DAY-OF-WEEK` e `FUNCTION CURRENT-DATE` informam todos a
  hora da própria máquina, não a UTC — inclusive a data, que difere de um lado e
  de outro da meia-noite. Os últimos cinco caracteres de `CURRENT-DATE` carregam
  o deslocamento **real** em relação a GMT (`…-0300`), de modo que um programa
  consegue saber em qual fuso está sendo executado.
  ⚠️ Qualquer nome de `FUNCTION` não reconhecido ainda é analisado, mas retorna
  **0** em tempo de execução.
- ✅ Literais: inteiro, decimal, string, todas as constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Uma constante figurativa preenche o receptor inteiro**, inclusive
  `HIGH-VALUE`: `MOVE HIGH-VALUE TO <PIC X(10)>` são dez bytes `0xFF`, e para um
  grupo ela é distribuída entre os filhos. Um receptor alfanumérico editado
  continua colocando seus caracteres de inserção, então `PIC XX0XXBXXX` contém
  `FF FF '0' FF FF ' ' FF FF FF`. Sob uma `PROGRAM COLLATING SEQUENCE` a
  constante nomeia um caractere comum e é esse caractere que preenche.
  ⚠️ `HIGH-VALUE` é o **byte** `0xFF`, não um caractere. A leitura de um operando
  de grupo, a edição e todos os caminhos de movimentação o transportam byte a
  byte, mas **a modificação por referência ainda não é exata em nível de byte**:
  `IF X (1:1) = HIGH-VALUE` é falso para um item que de fato contém `0xFF`.
- ✅ **Um literal numérico pode começar pelo ponto decimal**: `.5`, `-.5`,
  `.000000001`. O COBOL‑85 exige apenas que um literal não *termine* com um,
  portanto `5.` continua sendo o número 5 seguido de um terminador de comando.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Os zeros à esquerda são significativos e exatos: `.000000001` é um bilionésimo,
  não um décimo. Sob `DECIMAL-POINT IS COMMA` o mesmo vale para `,5`.
  O que separa o literal de um ponto de fim de comando é a **ausência de
  espaço**: o COBOL‑85 exige um após um terminador, então `MOVE X TO Y.` nunca é
  lido como o início de uma fração, e `MOVE X TO Y.5` é um erro de compilação em
  vez de uma reinterpretação silenciosa.
- ✅ **Sinalização de conformidade** (`cobolt_semantic::flagging`) — o padrão
  pede que uma implementação conforme seja capaz de dizer a um programa quais dos
  recursos que ele usa ficam fora de um nível de conformidade escolhido. Duas
  análises respondem a isso:
  - `flag_obsolete` — o conjunto de **elementos obsoletos** do COBOL‑85: os cinco
    parágrafos opcionais da IDENTIFICATION DIVISION, `MEMORY SIZE`, `ALTER`,
    `STOP` com um literal e `GO TO` sem nome de procedimento.
  - `flag_high_subset` — tudo o que está acima do **subconjunto alto**, de
    `COMPUTE`, `EVALUATE` e `INITIALIZE` passando por `CORRESPONDING`, a
    modificação por referência, a qualificação, `SET … TO TRUE` e um quarto
    subscrito, até a continuação de uma *palavra* ou de um *literal numérico*
    através do limite do cartão. (Continuar um literal **alfanumérico** está
    dentro do subconjunto e não é reportado.)

  Nenhuma das duas é verificação de erros, e nenhuma roda em uma compilação
  comum: cada construção que elas nomeiam é COBOL‑85 válido que o RustCOBOL
  implementa e executa. São pontos de entrada separados justamente para que uma
  compilação normal nunca comece a avisar sobre `AUTHOR` ou sobre `COMPUTE`. Os
  NIST `NC302M`, `NC303M` e `NC401M` as validam: 7, 4 e 40 sinalizações, todas
  correspondentes.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — o caractere que preenche
  uma posição de moeda em um PICTURE editado. Ele **substitui** o `$` em vez de
  se juntar a ele, então, assim que um programa declara um, `$` deixa de ser um
  caractere de picture ali:
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` passa então a ser lido como `      <.00`, e `MOVE 1234`
  como ` <1,234.00` — a sequência flutuante se comporta exatamente como
  `$$$,$$$.99`. Um símbolo de moeda que seja uma **letra** funciona do mesmo
  jeito: `CURRENCY SIGN IS "W"` faz de `PICTURE WWWWW` uma cadeia de moeda
  flutuante de cinco posições, de modo que `MOVE 12` é lido como `  W12`. *(Antes
  da 1.62.40 uma sequência de um símbolo em letra era lida como uma única palavra
  e rejeitada, então só o `$` flutuava.)* O
  literal deve ter um único caractere, e o COBOL‑85 proíbe um que colidiria com
  um caractere de picture ou com um separador: nada de dígito, nada de
  `A B C D E G N P R S V X Z`, e nenhum de `space * + - , . ; ( ) " / =`.
- ✅ **Literais hexadecimais** — `X"09"`, `x'0D0A'` (qualquer caixa, qualquer tipo
  de aspas). Um caractere por **par** de dígitos hexadecimais, portanto a
  quantidade de dígitos deve ser par; uma quantidade ímpar ou um dígito não
  hexadecimal é um literal malformado e é reportado, em vez de silenciosamente
  relido como a palavra `X` ao lado de uma string. Utilizáveis onde quer que um
  literal entre aspas seja válido (`DELIMITED BY`, `MOVE`, `VALUE`, comparações).

---

## Cláusulas da DATA DIVISION (sintaxe de declaração aceita)

- ✅ Níveis `01`–`49`, `77`, `88`; `FILLER`; grupo/elementar. A palavra `FILLER`
  é **opcional** — `05 PIC X VALUE ":".` declara um exatamente como
  `05 FILLER PIC X VALUE ":".` faz, e de qualquer das formas ele ocupa seus
  bytes e guarda seu `VALUE` dentro do grupo que o contém.
- ✅ `PIC/PICTURE` com `X A 9 S V P` e símbolos de edição
  (`Z * $ + - CR DB B 0 / , .`). O símbolo monetário é `$` a menos que
  `SPECIAL-NAMES. CURRENCY` tenha nomeado outro — veja **Expressões, literais,
  USAGE** acima. **`P` é uma posição de escala decimal** — uma posição de dígito
  que o item abrange mas não armazena: `PIC S999PP` guarda três dígitos que
  representam centenas (`MOVE 12300` o armazena exatamente; `MOVE 12345`
  armazena 12300), e `PIC PP99` guarda dois que representam décimos de milésimo.
  As posições ocupadas pelos `P` são sempre lidas como zero e não ocupam
  **nenhum byte** no layout de um registro.
- ✅ **A proteção com asteriscos preenche o item inteiro.** Um valor zero em uma
  PICTURE cujas posições de dígito são todas `*` preenche com asteriscos todas
  as posições de caractere — as casas decimais, as vírgulas de agrupamento, um
  `$` fixo e um `CR` ou `DB` final igualmente — deixando apenas o próprio ponto
  decimal: `PIC $**.**CR` contendo zero é lido como `***.****`, e `PIC *,***.**`
  é lido como `*****.**`. Um valor **diferente** de zero protege apenas os zeros
  à esquerda, de modo que o `$` fixo mantém a sua própria posição
  (`-2.34` → `$*2.34CR`). *(Antes da 1.62.37 `CR`/`DB` contribuíam com um único
  asterisco em vez das duas posições de caractere que ocupam, então um item
  desses voltava um caractere mais curto que a sua própria largura.)*
- ✅ **Um literal numérico move os seus caracteres, tal como foi escrito.** Para
  um receptor alfanumérico um literal contribui com os dígitos que o programa
  digitou, justificados à esquerda e preenchidos com espaços —
  `MOVE 2 TO <PIC X(4)>` dá `"2   "`, e
  `MOVE 060820000200 TO <six PIC 99 children>` os preenche como
  `06 08 20 00 02 00`. A largura do **receptor** nunca preenche o literal;
  apenas a largura com que ele foi escrito faz isso. *(Antes da 1.62.38 o lexer
  guardava apenas o valor, então um zero à esquerda se perdia e cada caractere
  seguinte deslizava uma posição para a esquerda.)*
- ✅ **Uma relação entre um operando numérico e um não numérico é não numérica**
  (COBOL‑85 VI‑89 6.15.4 GR2). O operando numérico é tratado como se tivesse
  sido movido para um item alfanumérico do **seu próprio tamanho**, o que
  transfere as suas posições de caractere e **não o seu sinal operacional**: um
  `PIC S9(18)` contendo `-123456789012345678` compara como **igual** a um
  `PIC X(18)` contendo `"123456789012345678"`. Três condições delimitam a regra
  — o operando numérico precisa ser um **inteiro**; o que é "não numérico" é
  decidido pela **declaração**, então um filho `PIC 99` contendo caracteres após
  um `MOVE` de grupo continua numérico — e um **grupo** é não numérico sejam
  quais forem os seus filhos, de modo que um `PIC 9(5)` com 12345 diante de um
  grupo de dez bytes contendo `"0000012345"` é `"12345     "` e diferente; e
  `ALL literal` assume o tamanho do outro operando. *(Antes da 1.62.38 a
  comparação era algébrica sempre que o lado de texto por acaso podia ser lido
  como número.)*
- ✅ **Truncamento de ordem superior em um MOVE numérico.** Um receptor guarda
  exatamente os dígitos que declarou nas duas pontas:
  `01 M PIC 99V999.  MOVE 123.45 TO M.` deixa `23.450`. A aritmética testa
  primeiro a capacidade do receptor, então um comando com `ON SIZE ERROR`
  mantém o valor antigo em vez disso.
- ✅ **Uma tabela de grupos é endereçada por ocorrência.** `MOVE VALUES-1 TO
  GRP-1 (2)` distribui o valor entre os filhos daquela ocorrência
  (`ELEM1 (2,1) … ELEM1 (2,4)`), e ler `GRP-1 (2)` concatena exatamente esses. O
  registro `01` que a envolve são os bytes de **todas** as ocorrências, então
  `MOVE GRP-TAB1 TO GRP-TAB2` copia uma tabela inteira.
- ✅ **Nomes de índice, literais e indexação relativa se misturam como
  subscritos.** `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — um sinal
  colado aos seus dígitos é um literal com sinal que abre o subscrito seguinte —
  e `ELEM1 (IN1 - 1, 3)`, onde o operador tem espaço dos dois lados, é
  indexação relativa.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (e `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/com sinal/alfanumérico/figurativo/`ALL`). **`VALUE ALL
  "literal"` repete a sua unidade por todo o item** — `PIC X(6) VALUE ALL
  "ABC"` é `"ABCABC"` e `PIC X(9) VALUE ALL "XY"` é `"XYXYXYXYX"`.
  *(Antes da 1.62.40 apenas as constantes figurativas de um caractere
  preenchiam o seu item e `ALL "literal"` o deixava com espaços.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — uma segunda leitura **viva** dos mesmos bytes. Não acrescenta
  armazenamento (portanto não alarga o grupo que o contém), e uma escrita feita
  por qualquer uma das descrições é visível pela outra:
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` e em seguida se lê de volta por `RESULT-A`.
  ⚠️ **Ressalva:** uma sobreposição maior que 256 slots de armazenamento
  expandidos (uma tabela 10×10×10 redefinida, por exemplo) mantém armazenamento
  por descrição — atualizá-la a cada escrita percorreria mil ocorrências duas
  vezes.
- ✅ **As sobreposições se aninham.** Um `REDEFINES` dentro de um registro que
  ele mesmo é redefinido é alcançado nos dois sentidos, por mais fundo que
  esteja: escrever dois bytes através de uma redefinição de nível 01 alcança o
  registro redefinido, o `REDEFINES` de um grupo dentro dele e o `REDEFINES` de
  um item dentro *desse* — inclusive um 88 declarado sobre o mais interno. Cada
  descrição é re-renderizada uma vez por escrita. *(Antes da 1.62.42 uma chave
  pertencente a mais de uma sobreposição guardava apenas a declarada por
  último, e uma única guarda interrompia a cadeia depois do primeiro salto.)*
- ✅ **Uma descrição sem nome ainda é uma descrição.** `02 FILLER REDEFINES
  <item>.` redescreve os bytes do seu alvo sem nome próprio, e uma escrita no
  alvo fica visível pelos seus filhos. Vários filhos dividem esses bytes entre
  si, na ordem do layout — a sobreposição *não* é um apelido do seu primeiro
  filho. Dois `FILLER REDEFINES` de um mesmo item são duas leituras
  independentes, cada uma começando no **primeiro** byte do alvo. *(Antes da
  1.62.36 um grupo redefinidor sem nome não recebia chave de armazenamento
  alguma, então os seus filhos eram lidos como espaços por mais preenchido que
  o alvo estivesse.)*
- ✅ **Um nome duplicado dentro de uma sobreposição** resolve para o mesmo
  armazenamento que o resto do programa alcança: `TAB-A` declarado sob dois
  grupos diferentes mantém uma leitura por declaração. *(Antes da 1.62.36 a
  cópia inicial da sobreposição era chaveada por um caminho sem os seus
  qualificadores externos, algo que só um nome duplicado permite distinguir —
  ou seja, exatamente o caso que precisa do qualificador o perdia.)*
- ✅ `JUSTIFIED [RIGHT]` — **armazena alinhado à direita**, em um item
  *alfanumérico* ou *alfabético*. Um emissor mais estreito que o receptor é
  preenchido à esquerda; um emissor mais largo que ele mantém a sua ponta
  **direita** e perde os caracteres mais à esquerda — o oposto da regra comum.
  *(Antes da 1.62.40 a cláusula só era registrada para itens alfanuméricos,
  então `PICTURE A(5) JUSTIFIED RIGHT` era analisada e depois alinhava à
  esquerda como qualquer outro item.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — aceitas;
  `SIGN … SEPARATE` ainda não muda como o item é armazenado.
- ✅ **Um `REDEFINES` no nível 01 pode descrever mais armazenamento do que o
  item que ele redefine**, e os bytes além do fim daquele item pertencem à
  descrição que for longa o bastante para nomeá-los. Escrever através de uma
  descrição mais curta deixa intacta a cauda da mais longa.
- ✅ **Uma sobreposição `REDEFINES` carrega os bytes do item redefinido**,
  inclusive para dentro de um par numérico: uma sobreposição `PIC S9(18)` de um
  `X(18)` contendo `"00ABCDEFGHI  4321 "` lê aqueles caracteres de volta, e
  `IS NUMERIC` responde **não** para eles. Quando os bytes de fato soletram
  dígitos, a leitura numérica não muda.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **nomes de condição de
  verdade**: o nível 88 se liga ao seu item hospedeiro; o teste confere o
  hospedeiro contra os VALUE / faixas, e `SET 88-name TO TRUE` grava no
  hospedeiro um valor que a satisfaça.
- ✅ **Um nome de condição pode ser declarado sob mais de um grupo, e `OF`/`IN`
  os distingue** — exatamente como acontece com um nome de dado, e níveis
  intermediários podem ser pulados:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  O subscrito pertence ao item hospedeiro, então ele seleciona contra qual
  ocorrência os VALUE são testados. Uma referência **não qualificada** a um nome
  de condição duplicado é ambígua em COBOL‑85; o runtime toma a primeira
  declaração, a mesma regra que aplica a um nome de dado ambíguo.
- ✅ `USAGE INDEX` declara um registrador de índice inteiro (`SET`/`SEARCH` o
  usam); `USAGE POINTER` — veja **Ponteiros** acima.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — um apelido de
  reagrupamento; ler concatena os itens cobertos, escrever distribui conforme a
  largura de cada campo.
  - ✅ **Um 66 é qualificado pelo registro que ele reagrupa**, exatamente como
    um item de dados é qualificado pelo grupo acima dele, então o mesmo nome 66
    pode ser declarado uma vez por registro e distinguido com `OF`/`IN`:
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Isso funciona igualmente
    em leituras e escritas, e um 66 vence um item de dados comum que por acaso
    compartilhe o seu nome. Os operandos da cláusula `RENAMES` são resolvidos
    nesse mesmo registro, então um `NAME-2` duplicado nomeia o deste registro.
  - ✅ **Uma tabela coberta contribui com todas as suas ocorrências**, não
    apenas a primeira: `66 R RENAMES ITEM-1 THRU TABLE-2`, onde `TABLE-2`
    contém `03 T PIC XXX OCCURS 5`, tem 20 caracteres de largura.
  - ✅ **Um 66 sobre exatamente um item *é* aquele item** — mesma PICTURE,
    mesma categoria, mesmo armazenamento. `66 R RENAMES W`, onde `W` é
    `PIC 9(4)`, é um item numérico de quatro dígitos, então `ADD 3500 TO R` com
    8000 dentro dele levanta `ON SIZE ERROR` e o deixa inalterado.
- Seções: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` é
  analisada mas não executada.

---

## Ainda NÃO suportado — lista atual de itens a evitar

> **Corrigido em 2026‑08‑25.** Esta seção começava antes com "O conjunto de
> verbos e cláusulas do COBOL‑85 está **totalmente coberto**." Rodar a suíte NIST
> CCVS85 desmentiu isso: **102 dos 434 programas dentro do escopo falharam
> naquele dia**, em construções que este documento não listava como lacunas —
> vírgulas e ponto e vírgula separadores, `FUNCTION x(ALL)`,
> `CLOSE … WITH LOCK`, `COPY` na Área B, entradas de comentário da
> IDENTIFICATION, números de prioridade de seção, nomes de dados começando por
> dígito e — até a 1.62.10 — literais numéricos com ponto decimal inicial. É para
> isso que serve uma suíte de validação. Cada lacuna está agora especificada em
> [`specs/nist/`](../specs/nist/README.md) e é acompanhada no
> [placar](#-a-conformidade-é-medida-não-afirmada--nist-ccvs85) acima.

A lista abaixo é o que está fora do escopo **por intenção**, ao contrário das
lacunas NIST acima, que são defeitos em processo de correção:

1. **Edição de entrada com `ACCEPT` de tela** — `DISPLAY … AT/WITH` e
   `ACCEPT … AT` são executados (ANSI) no modo CLI, mas a edição completa em nível
   de campo da SCREEN SECTION (tabulação automática, validação de campo, mapas de
   cor) é **substituída pelo form designer** no modo GUI.
2. **Compartilhamento de arquivos entre *processos*** — `OPEN … SHARING/WITH
   LOCK`, `READ … WITH [NO] LOCK` e `UNLOCK` são analisados e acionam as travas de
   registro por execução do motor INDEXED, mas as travas não são impostas entre
   processos distintos do sistema operacional (modelo de unidade de execução
   única).
3. **COBOL orientado a objetos** (definições de classe/método) — `INVOKE` é uma
   operação nula para objetos COBOL (ele aciona apenas objetos de GUI/runtime).
4. Nomes de função intrínseca não reconhecidos ainda retornam **0** — o mesmo modo
   de falha silenciosa. Especificação:
   [intrínsecas](../specs/nist/NIST-spec-intrinsic-function-gaps.md).
5. ⚠️ **Um valor inválido de `ACCESS MODE` / `ORGANIZATION` é engolido sem
   diagnóstico** — a mesma armadilha de novo, e esta é disparada por um erro de
   digitação comum do usuário. `ACCESS MODE IS` aceita apenas `SEQUENTIAL`,
   `RANDOM` ou `DYNAMIC` (`INDEXED` é uma *organização*, não um modo de acesso),
   mas o analisador da cláusula SELECT testa esses três e deixa qualquer outra
   coisa cair no ramo genérico de "pular um token desconhecido", de modo que o
   arquivo mantém silenciosamente o `SEQUENTIAL` padrão e se comporta mal em tempo
   de execução em vez de falhar na compilação. `ORGANIZATION IS` tem exatamente a
   mesma forma. Ambos deveriam levantar um erro claro de tempo de compilação
   nomeando a palavra ofensora. **Não é um problema do Núcleo** — nenhum programa
   NC traz uma cláusula `ACCESS MODE`; a cláusula aparece apenas nos módulos DB,
   IC, IX, OBSQ, RL, RW, SQ e ST, portanto, sob a REGRA DE OURO nº 9, isso
   aguarda até que o NC esteja concluído.
6. ⚠️ **`ALPHABET … IS EBCDIC` é aceito, mas deixa em vigor a ordenação nativa
   (ASCII).** A frase literal (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` e `STANDARD‑2` estão todos implementados e acionam de verdade o
   `PROGRAM COLLATING SEQUENCE`; só falta a tabela EBCDIC, e nomeá-la resulta
   silenciosamente na ordem ASCII. A mesma família de armadilhas de 4–6.
7. **O módulo de Comunicação e o Report Writer** — veja
   [N/A acima](#-na--o-que-está-fora-do-escopo-do-rustcobol-e-por-quê).

> **Resolvido (1.5.0):** o modelo de dados plano tornou-se hierárquico / ciente de
> ocorrências, desbloqueando **CORRESPONDING**, os **nomes qualificados**, a
> **subscrição de tabelas** e **`SEARCH`**.
> **Resolvido (1.6.0):** `MULTIPLY`/`DIVIDE` com vários receptores + `ROUNDED` por
> receptor; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`;
> `INSPECT TALLYING REPLACING` combinado + `BEFORE/AFTER INITIAL`; intrínsecas de
> data e `ANNUITY`; abreviação com objeto literal; `EVALUATE ALSO`/`WHEN NOT`;
> nomes-condição de nível 88 reais; `PERFORM para VARYING`; e o runtime de
> `SORT`/`MERGE` com `RELEASE`/`RETURN`.
> **Resolvido (1.7.0):** abreviação com objeto identificador;
> `INITIALIZE … REPLACING`; `66 RENAMES`; ponteiros (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`); `ALTER` / `UNLOCK`;
> `NEXT SENTENCE` fiel; as intrínsecas padrão restantes; e o `ACCEPT`/`DISPLAY`
> de tela estendido (executado no modo CLI).
> **Resolvido (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (com os
> registradores emparelhados `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Resolvido (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (libera as travas de registro INDEXED) e `CANCEL programa`.
> **Resolvido (1.8.0):** `COMMIT` / `ROLLBACK` como transações de arquivos INDEXED
> controladas pelo programa (motores de memória e disco; log de desfazer real em
> disco).

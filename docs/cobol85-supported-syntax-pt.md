<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.0 -->

# Referência da sintaxe suportada do RustCOBOL‑85

**Para que serve este documento:** para dizer quanto do padrão COBOL‑85 o
RustCOBOL implementa realmente — e para o demonstrar contra a **suite oficial de
validação NIST COBOL‑85** em vez de simplesmente o afirmar. O
[painel](#-a-conformidade-é-medida-não-afirmada--nist-ccvs85) mais abaixo é o
título; tudo o que vem depois é o detalhe por trás desse número.

**Verdade de campo sobre o que o lexer/parser/runtime do RustCOBOL aceitam hoje
realmente**, derivada do código fonte (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`) e confrontada com `NIST/newcob.val,cbl`.
Escreve os testes contra as formas ✅; as formas ❌ não chegam a ser analisadas ou
são no‑ops, e as formas ⚠️ são analisadas mas comportam‑se apenas parcialmente.
Este é o documento companheiro de
[`cobol85-verb-test-matrix-pt.md`](cobol85-verb-test-matrix-pt.md): a matriz diz
*o que* testar, e este diz *que grafia o RustCOBOL entende*.

Legenda: ✅ suportado · ⚠️ é analisado mas parcial/simplificado · ❌ não
reconhecido (evita‑o, ou testa‑o apenas para confirmar a lacuna).

---

## Índice

1. [★ A conformidade é medida, não afirmada — NIST CCVS85](#-a-conformidade-é-medida-não-afirmada--nist-ccvs85)
2. [Parágrafos da IDENTIFICATION DIVISION](#parágrafos-da-identification-division)
3. [Formatos de fonte](#formatos-de-fonte)
4. [Instruções reconhecidas (verbos)](#instruções-reconhecidas-verbos)
5. [Formas suportadas por verbo](#formas-suportadas-por-verbo)
6. [Condições (IF / EVALUATE / PERFORM UNTIL)](#condições-if--evaluate--perform-until)
7. [Expressões, literais, USAGE](#expressões-literais-usage)
8. [Cláusulas da DATA DIVISION (sintaxe de declaração aceite)](#cláusulas-da-data-division-sintaxe-de-declaração-aceite)
9. [Ainda NÃO suportado — lista de evitação atual](#ainda-não-suportado--lista-de-evitação-atual)

---

## ★ A conformidade é medida, não afirmada — NIST CCVS85

**Este é o sentido do documento.** Cada afirmação abaixo é verificada contra a
**suite oficial de validação NIST COBOL‑85** — CCVS85 versão 4.0 (01 OCT 1992,
COBOL 85 versão 4.2, SSVG de abril de 1993), a suite que o National Institute of
Standards and Technology dos Estados Unidos usava para certificar compiladores
COBOL. Tem 28 MB, 348.271 linhas, **459 programas COBOL** e 51 membros de
copybook, e reside neste repositório em `NIST/newcob.val,cbl`.

É a fonte de verdade. Quando o RustCOBOL e o CCVS85 discordam, **o CCVS85 está
certo e o RustCOBOL está errado**.

O livro de registo legível por máquina é
[`NIST/progress.json`](../NIST/progress.json) — versionado, atualizado após cada
alteração verificada. Os números abaixo são retirados dele em vez de reescritos à
mão.

### O painel

Medido em **2026‑08‑31 na 1.62.132**, sobre a distribuição intacta. O censo de
compilação fechou na 1.62.129.

| Eixo | Resultado | Significado |
|---|---:|---|
| **Compilação** | **420 / 420** | o front end aceita todos os programas dentro do âmbito. FAIL 0. |
| **Execução** | **380 / 380** | todos os programas pontuados correm e reportam **zero falhas** no seu próprio relatório CCVS. |
| **Asserções** | **8.362 PASS / 0 FAIL** | as verificações que esses programas fazem sobre si mesmos. |

Reproduz qualquer um dos eixos:

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### Os dois eixos nunca são confundidos

A compilação é a afirmação estritamente mais fraca: diz que o front end aceita
todas as construções de um programa, não que o programa calcule a resposta certa.
A suite pontua‑se a si mesma — cada programa CCVS85 imprime a sua própria
contagem `PASS` / `FAIL*` — pelo que o eixo de execução é o que significa
«funciona». Ambos são reportados por módulo abaixo, com os seus próprios
denominadores, e nenhum é nunca citado como se fosse o outro.

A ilustração mais clara está na própria história deste repositório: 30 dos 35
programas de ficheiros RELATIVE compilavam limpos enquanto o runtime **não tinha
motor RELATIVE nenhum**. Corriam e produziam resultados errados em silêncio. O
motor chegou na 1.62.76 e o módulo terminou na 1.62.77.

#### Por módulo

A compilação e a execução têm denominadores diferentes, por duas razões
declaradas. Os membros `*301M` testam a *sinalização de subconjunto intermédio*
de funcionalidades que o RustCOBOL implementa como padrão, o que é inalcançável
por desenho e fica excluído da execução por decisão do operador (IX301M, RL301M,
ST301M, SM301M); continuam a contar no censo de compilação, onde passam. E a
maioria dos membros IC são **chamados** — subprogramas sem relatório próprio —
pelo que apenas os programas chamadores são pontuados.

| Módulo | O que testa | Compilação | Execução | Asserções | Estado |
|---|---|---:|---:|---:|---|
| **NC** | Núcleo | **95 / 95** | **95 / 95** | 4.614 | ✅ terminado |
| **SQ** | E/S sequencial | **85 / 85** | **85 / 85** | 624 | ✅ terminado |
| **IX** | E/S indexada | **42 / 42** | **41 / 41** | 574 | ✅ terminado |
| **IF** | Funções intrínsecas | **45 / 45** | **45 / 45** | 841 | ✅ terminado |
| **IC** | Comunicação entre programas | **47 / 47** | **25 / 25** | 309 | ✅ terminado |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ terminado |
| **SM** | Manipulação do texto fonte | **17 / 17** | **16 / 16** | 311 | ✅ terminado |
| **RL** | E/S relativa | **35 / 35** | **34 / 34** | 354 | ✅ terminado |
| **DB** | Depuração | **14 / 14** | — | — | apenas eixo de compilação (abaixo) |
| **No âmbito** | | **420 / 420** | **380 / 380** | **8.362** | |
| SG | Segmentação | 13 / 13 | — | — | ⬜ declarado fora do âmbito (abaixo) |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB (Depuração)** é pontuado apenas na compilação. Os seus 14 programas são
aceites; a semântica de *execução* do módulo de depuração não está implementada, e
o eixo de execução para ele não foi declarado dentro do âmbito. É listado aqui em
vez de escondido, para que a lacuna continue visível.

#### A contagem DELETED — 24, e o que significa

`***** ****TEST DELETED****` é o próprio marcador do CCVS para um caso que o
programa saltou por si mesmo. **Não** é uma passagem, e é registado em separado
por essa razão: a contagem caiu de 108 → 1 na 1.62.53 enquanto a contagem de
programas limpos quase não se movia, o que era progresso real que uma leitura
centrada apenas nas falhas teria deixado passar.

Nos módulos terminados há 24 casos DELETED: NC 5, SQ 6, IX 1, IC 4, SM 3, RL 5.
**Apenas os 3 do SM estão documentados como sendo da própria distribuição** —
SM206A PST‑TEST‑008 e PST‑TEST‑11, e SM208A REP‑TEST‑7, são distribuídos
comentados, pelo que uma execução conforme do fonte entregue reporta exatamente
esses três. Os outros 21 estão registados mas ainda não explicados
individualmente no livro de registo; não os cites como intencionais.

### ⬜ N/A — o que está fora do âmbito do RustCOBOL, e por quê

Estes módulos **não são contados como falhas** — 38 programas excluídos de toda a
pontuação. O raciocínio completo está em
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Módulo | Programas | Por que está fora do âmbito |
|---|---:|---|
| **CM** — Comunicação | 9 | `COMMUNICATION SECTION`, entradas `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Visa os monitores de teleprocessamento dos anos oitenta — filas de mensagens na posse de um gestor de transações. Não existe aqui tal runtime, e o módulo foi retirado dos padrões COBOL posteriores. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entradas `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, quebras de controlo. Uma sublinguagem declarativa extensa; a resposta do PowerRustCOBOL aos relatórios é o Form Designer e a exportação para PDF. Poderia tornar‑se mais tarde uma *funcionalidade* se se quiser — é a única exclusão com valor real para o utilizador. |
| **SG** — Segmentação | 13 | Decisão do operador, 2026‑08‑29. A segmentação existe para encaixar um programa numa máquina demasiado pequena para o conter: os cabeçalhos `SECTION` levam um número de segmento e o runtime sobrepõe segmentos independentes uns aos outros. O RustCOBOL é um runtime de 64 bits com mais espaço de endereçamento do que qualquer programa COBOL consegue esgotar, pelo que um número de segmento **compila e não tem efeito nenhum**. Não há comportamento que o módulo possa medir. Os seus 13 programas continuam a compilar, e são reportados como N‑A em vez de eliminados para que a exclusão continue visível. |
| **OBSQ / OBIC / OBNC** | 9 | Voltam a testar módulos anteriores e esperam que o compilador *sinalize* elementos obsoletos do COBOL‑85. O seu conteúdo de linguagem está coberto pelas especificações dentro do âmbito; o que está fora do âmbito é a **sinalização** de funcionalidades obsoletas. |
| **EXEC85** | 1 | Não é um teste. É o próprio executivo COBOL do NIST que divide a distribuição e comanda a suite — aqui substituído por um arnês em Rust, pelo que não precisa de compilar. |

O **COBOL orientado a objetos** também está fora do âmbito do RustCOBOL, mas o
CCVS85 é‑lhe inteiramente anterior — não há programas OO na suite.

### O que falta

Nos módulos pontuados, nada: ambos os eixos estão fechados e não falha nenhuma
asserção. O que resta não é uma lista de defeitos mas três decisões permanentes —
o eixo de execução do DB, o SG e os membros de sinalização `*301M` — cada uma
registada acima com a sua razão, mais os 21 casos DELETED que não foram
explicados individualmente.

O arnês imprime o detalhe da falha por trás de qualquer regressão, pronto a
agrupar por módulo:

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> Uma linha de detalhe `FAIL*` é escrita **duas vezes** de propósito — o
> `PRINT-DETAIL` do CCVS executa
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — enquanto `PASS ` é escrito
> uma só vez. Qualquer contagem bruta de marcadores retirada do ficheiro de
> impressão tem de dividir as falhas por dois antes de significar algo.

### História de conformidade

Eixo de compilação, contra o denominador dentro do âmbito de cada momento. O
próprio denominador moveu‑se quando o SG foi declarado fora do âmbito e quando o
DB205A foi repontuado sob o CM, pelo que as primeiras linhas são sobre 434 e a
linha de fecho sobre 420.

| Versão | Compilação | O que mudou |
|---|---:|---|
| 1.62.7 | **0** / 434 | Nada compilava. Faltavam duas regras do formato de referência clássico: as colunas 73‑80 eram lidas como fonte, e as linhas de continuação nunca eram juntadas. |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — o formato de referência clássico, incluindo a continuação. Ver [Formatos de fonte](#formatos-de-fonte). |
| 1.62.13 | 292 / 434 | A vírgula e o ponto e vírgula separadores são pontuação, não tokens; os subscritos podem ser separados apenas por espaços; um delimitador duplicado dentro de um literal é um único carácter. Esvaziaram‑se três baldes de diagnóstico completos. |
| 1.62.14 | 317 / 434 | Uma tabela inteira como argumento intrínseco; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`. **Funções intrínsecas 45 / 45 na compilação.** |
| 1.62.16 | 376 / 434 | O `AT` de `AT END` é opcional, pelo que uma frase `END` solta já não engole o cabeçalho de parágrafo seguinte (33 programas). **E/S indexada 42 / 42 na compilação.** |
| 1.62.21 | 417 / 434 | O passo do Núcleo — série `ALTER`, nomes‑condição com subscrito, relações combinadas abreviadas, categorias de `INSPECT` entre operandos. Núcleo de 76 → 92 a compilar. |
| **1.62.42** | 420 / 434 | **Núcleo terminado em ambos os eixos** — 95 / 95 a compilar *e* a executar limpo, 4.614 asserções sem nenhuma a falhar. |
| **1.62.43** | 422 / 434 | A E/S sequencial compila por completo, 85 / 85, e vai de 10 → 44 de 85 na execução. Os parágrafos de um declarativo conservam os nomes, pelo que um tratador `USE` lhes pode fazer `PERFORM` e `GO TO` — 20 programas deixaram de rebentar. |
| **1.62.47** | — | **E/S sequencial terminada** — 85 / 85 em ambos os eixos. A última lacuna era `XXXXD001`, um ficheiro de dados que a *instalação* do CCVS85 fornece e que nenhum membro escreve; agora o arnês planta‑o. |
| **1.62.76** | — | Chega o **motor RELATIVE** (`cobolt-runtime/src/relative.rs`, contentor `PRCREL1`). Os sete verbos de ficheiro despacham sobre `FileOrganization::Relative`. |
| **1.62.77** | — | **E/S relativa terminada** (34 / 34) desde uma linha de base de 14 / 35 numa única sessão, e **E/S indexada terminada** — o motor relativo fechou as últimas quatro falhas do IX106A, que eram o ficheiro relativo que ele exercita ao lado dos sequenciais e indexados. |
| **1.62.81** | — | **Funções intrínsecas terminadas** na execução, 45 / 45, desde uma linha de base de 24 / 45. Cinco causas, nenhuma na matéria própria do módulo: uma regra de separadores do lexer duas vezes, uma lacuna de sinalização, a gramática do argumento do `NUMVAL`, uma comparação de listas de argumentos e um caminho de transbordo na divisão. |
| **1.62.107** | — | **Comunicação entre programas terminada**, 25 / 25. |
| **1.62.119** | — | **Sort / Merge terminado**, 39 / 39, 735 PASS. O passo final foi `[COLLATING] SEQUENCE [IS] alphabet-name` no SORT/MERGE, ordenando chaves alfanuméricas pelo alfabeto nomeado em `SPECIAL-NAMES`. |
| **1.62.127** | — | **Manipulação do texto fonte terminada**, 16 / 16. Os operandos literais de cadeia conservam as aspas; os operandos identificador abrangem a sua cadeia `IN`/`OF` e o subscrito; os pares aplicam‑se numa só passagem sem reexaminar as substituições. |
| **1.62.129** | **420 / 420** | **O censo de compilação fecha a 100 %.** O DB205A é pontuado sob o CM por decisão, o que coloca a suite dentro do âmbito em 420. |

> **O resumo honesto.** Todos os programas dentro do âmbito compilam, e todos os
> programas pontuados correm limpos: **420 / 420 na compilação, 380 / 380 na
> execução, 8.362 asserções sem nenhuma a falhar.** Nove lançamentos antes da
> primeira dessas linhas o número de compilação era zero. O que continua aberto é
> declarado acima como decisões em vez de escondido dentro de uma percentagem —
> o eixo de execução do DB, o SG, os membros de sinalização `*301M`, e 21 casos
> DELETED que não foram explicados individualmente.

---

> **Atualização (passo de implementação de lacunas):** os seguintes foram
> implementados e são agora ✅ — **modificação de referência** `id(start:len)`,
> **`PERFORM n TIMES` em linha**, **`SET … UP/DOWN BY`**, **STRING/UNSTRING
> `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` consciente de
> categorias**, **condições abreviadas com operador anteposto** (`a > 1 AND < 9`),
> **`CALL … ON EXCEPTION`** (corre num CALL não resolvido), **múltiplos recetores
> em `COMPUTE` + `ROUNDED` por recetor**, e um conjunto de **funções
> intrínsecas** muito maior.
>
> **Atualização (passo de ambiente hierárquico / consciente de ocorrências —
> 1.5.0):** quatro funcionalidades bloqueadas pelo modelo de dados são agora ✅ —
> **subscrição de tabelas em tempo de execução** `t(i)` / `t(i, j)`
> (armazenamento por ocorrência), **desambiguação de nomes qualificados**
> `id OF/IN group` (os nomes folha duplicados resolvem para armazenamento
> independente), **`MOVE/ADD/SUBTRACT CORRESPONDING`**, e **`SEARCH` /
> `SEARCH ALL` funcionais**.
>
> **Atualização (passo de completude de verbos — 1.6.0):** agora também ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` com múltiplos recetores + `ROUNDED` por recetor**
> em `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** e o `EXIT` simples corrigido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado e regiões
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de data/finanças
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condições abreviadas com objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multissujeito) e **`WHEN NOT`**;
> **nomes‑condição de nível 88 reais** (`SET … TO TRUE/FALSE`, o anfitrião é
> testado contra os seus VALUE/intervalos); **`PERFORM para VARYING`**; e um
> runtime de **`SORT`/`MERGE`** funcional (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). A lista de evitação do fim está atualizada.
>
> **Atualização (passo de liquidação da lista de evitação — 1.7.0):** as lacunas
> restantes estão agora implementadas — **abreviação com objeto identificador**
> (`a = b OR c`, resolvida através de metadados de nível 88);
> **`INITIALIZE … REPLACING category DATA BY value`**; **`66 RENAMES`** (a leitura
> sintetiza / a escrita distribui pelos itens cobertos); **apontadores**
> (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`,
> `SET ADDRESS OF item TO …` com alias, `IF ptr = NULL`); **`ALTER`** /
> **`UNLOCK`**; **`NEXT SENTENCE`** fiel; as **intrínsecas** padrão restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`); e
> **`ACCEPT`/`DISPLAY`** de ecrã estendidos (`AT`/`WITH` através de ANSI em modo
> CLI — agora *executados*, e não apenas analisados).
>
> **Atualização (1.7.1):** as fontes de registo do `ACCEPT` são agora funcionais
> (eram no‑ops reconhecidos) — **`FROM COMMAND-LINE`**, **`ARGUMENT-NUMBER`** /
> **`ARGUMENT-VALUE`** (emparelhados com `DISPLAY n UPON ARGUMENT-NUMBER`),
> **`ENVIRONMENT-VALUE`** (emparelhado com
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Atualização (1.7.2):** frases de partilha / bloqueio de ficheiros e `CANCEL`
> (eram ❌ / no‑ops) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (liberta os bloqueios de registo
> INDEXED do ficheiro), e **`CANCEL program`** (reinicializa o armazenamento do
> programa).
>
> **Atualização (1.8.0):** **`COMMIT` / `ROLLBACK`** são agora verbos COBOL reais
> — transações controladas pelo programa sobre os ficheiros INDEXED abertos (tanto
> o motor de memória como o de disco). O motor de disco ganhou um registo de
> desfazer real durante a execução (antes era um no‑op). A lista de evitação do
> fim está atualizada.

---

## Parágrafos da IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Os parágrafos de **entrada‑comentário** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — em **qualquer ordem e qualquer
  subconjunto**.
- ✅ `REMARKS` também é aceite. Foi eliminado do COBOL em 1985, pelo que não é
  armazenado; é aceite para que o código herdado do COBOL‑74 continue a compilar.

Uma **entrada‑comentário** é texto livre, e o COBOL‑85 quer dizer isso
literalmente:

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Pode conter **palavras reservadas** — o `DATA` acima não inicia uma DATA
  DIVISION.
- Pode conter **pontos**, e não termina num deles.
- **Abrange tantas linhas** quantas escreveres.
- Termina no cabeçalho de parágrafo ou de divisão seguinte que **comece uma
  linha** na Área A — que é como a entrada acima termina em `DATE-WRITTEN`.

**Uma aspa nessa prosa fica contida na sua linha** (desde 1.62.12). Texto como
`THE COMPILER"S ABILITY` já não abre um literal que se prolongue pelo resto do
programa — ver [Formatos de fonte](#formatos-de-fonte). Continua a valer a pena
evitar uma aspa sem par numa entrada‑comentário, mas agora custa‑te essa linha, e
não o ficheiro.

⚠️ `INSTALLATION`, `SECURITY` e `REMARKS` **não são palavras reservadas** aqui.
São reconhecidos como nomes de parágrafo apenas dentro da IDENTIFICATION
DIVISION, pelo que um dado chamado `SECURITY` continua a funcionar.

---

## Formatos de fonte

O RustCOBOL lê três disposições de fonte. A escolha é explícita — **nunca** é
adivinhada a partir do conteúdo do ficheiro, porque aplicar regras de colunas a
fonte que não foi escrito para elas apaga código em silêncio.

| `--source-format` | O que significa |
|---|---|
| `free` | Sem regras de colunas nenhumas. `*>` inicia um comentário. **O valor por omissão**, e o que os próprios projetos do PowerRustCOBOL e os ficheiros `.cbl` de formulário gerados usam. |
| `fixed` | ✅ **Formato de referência clássico do COBOL-85** — a disposição que o padrão define e em que o fonte de imagem de cartão é escrito. Ver abaixo. |
| `fixed-relaxed` | A área de sequência e a coluna indicadora são respeitadas, mas a linha segue tão longe quanto a escreveste — sem limite de 72 colunas. |
| `auto` | Comportamento histórico: `free`, a não ser que `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` define o valor por omissão de uma sessão.

### `fixed` — o formato de referência clássico

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Colunas 1-6** — área de número de sequência, ignorada.
- **Coluna 7** — área indicadora:
  - `*` ou `/` → linha de comentário
  - `-` → **continuação** da linha anterior
  - `D` → linha de depuração; um comentário (o modo de depuração ainda não está
    implementado)
  - qualquer outra coisa → lida como fonte comum. O padrão reserva esta coluna,
    mas as suites de imagem de cartão usam‑na como seletor de linhas opcionais, e
    descartar essas linhas em silêncio apagaria código.
- **Colunas 8-72** — o fonte.
- **Colunas 73-80** — área de identificação, **descartada**.

### Linhas de continuação ✅

Um hífen na coluna 7 continua a linha anterior.

**Continuar uma palavra ou um literal numérico** — os espaços finais da linha
continuada são descartados e as duas metades juntam‑se sem nada entre elas:

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

declara um único item chamado `WRK-DS-18V00-CONTINUED`.

**Continuar um literal alfanumérico** — o literal da linha continuada não tem
aspa de fecho; a linha de continuação tem de reabrir com uma, e o literal retoma
no carácter seguinte:

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **O fragmento continuado segue até à coluna 72, espaços finais incluídos.** Uma
linha que fique curta da coluna 72 continua a contribuir com esses espaços para o
literal. É por isso que um literal continuado só é exato byte a byte sob `fixed`;
os outros formatos não têm coluna 72 onde parar.

### Um literal nunca abrange uma linha por acidente ✅

A continuação é a **única** forma de um literal atravessar linhas. Uma aspa que
não é fechada na sua própria linha é um erro, reportado onde está escrita:

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Isto importa mais do que parece. Antes de 1.62.12 uma aspa sem par seguia até à
*próxima* aspa em qualquer ponto do ficheiro, pelo que um único `"` perdido num
comentário engolia divisões inteiras e deslocava o emparelhamento de todas as
aspas seguintes — os programas do NIST onde isto foi encontrado têm um número
**par** de aspas, pelo que nada ficava por terminar; um único carácter havia
deslocado a paridade de todo o ficheiro. Agora o dano para na mudança de linha.

> **O formato livre não tem continuação de literais.** Nem `&` — esse é o
> *operador* de concatenação — nem um bloco delimitado. Um literal em formato
> livre tem de caber numa linha; para um longo, concatena:
> `"first part" & "second part"`.

> **Nota.** Escolher `fixed` para um ficheiro que foi escrito em formato livre
> irá danificá‑lo — tudo o que passe da coluna 72 desaparece, e o texto antes da
> coluna 8 é lido como número de sequência. Passa‑o apenas para fonte que
> realmente seja imagem de cartão.

---

## Instruções reconhecidas (verbos)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redireciona o `GO TO` de para-1) ·
`UNLOCK file` (liberta os bloqueios de registo do ficheiro) ·
`OPEN … SHARING/WITH LOCK` · `READ … WITH [NO] LOCK` (partilha/bloqueio de
ficheiros — indicativo dentro de uma única unidade de execução)
✅ `COMMIT` / `ROLLBACK` (transações de ficheiros INDEXED controladas pelo
programa — ver os verbos de ficheiro) · `CANCEL` (reinicializa o armazenamento do
programa) ·
✅ `INVOKE` — comanda objetos de GUI/runtime (janelas, formulários, métodos de
controlo); é um no‑op apenas para os objetos **COBOL**, já que as definições de
classe/método estão fora do âmbito
Extensões do projeto: `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Um bloco pode fazer `use` dos crates sempre ligados (std, egui, eframe e
o conjunto de runtime ligado) **mais qualquer crate que o projeto registe em
Crates do Projeto** (spec 044): os crates registados são fixados numa versão
exata, copiados para o `crates/` do projeto e compilados dentro do binário; os
crates não registados fazem falhar o Check/Build na linha do desenvolvedor,
nomeando o remédio.

✅ `SEARCH` (série) / `SEARCH ALL` (pesquisa binária sobre uma tabela com
`ASCENDING`/`DESCENDING KEY` — executa o primeiro `WHEN` que corresponda, senão
`AT END`).
✅ `SORT` / `MERGE` com `RELEASE` / `RETURN` (funcionais — ver abaixo).
✅ `DECLARATIVES … END DECLARATIVES` com `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — tratadores de erro de ficheiro
disparados por um `FILE STATUS` de erro não tratado. Um tratador **é entrado pelo
topo da sua secção e executa até ao fim da secção**, e os seus parágrafos
conservam os nomes, pelo que lhes pode fazer `PERFORM` e `GO TO` — incluindo um
parágrafo de *outra* secção declarativa. Os parágrafos declarativos vivem no seu
próprio espaço de nomes: o controlo nunca cai do corpo principal para dentro
deles, e um nome declarado em ambos resolve para a cópia do declarativo enquanto
um tratador está a correr e para a do corpo em todo o resto. Um declarativo
também pode fazer `PERFORM` de um parágrafo da porção não declarativa.
❌ **Não reconhecidos — não os uses:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas suportadas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (múltiplos recetores).
- ✅ **Um operando de grupo torna todo o movimento alfanumérico** (COBOL-85
  6.18.4). A PICTURE do outro operando contribui com o seu *tamanho* e nada mais:
  sem edição, sem des‑edição, sem conversão numérica.
  `MOVE <grupo que contém "123ABC">` deixa `"123ABC "` num `PIC 0XXXXX0` (não o
  editado `"0123AB0"`), os mesmos seis caracteres e um espaço num `PIC 9999V999`,
  e `"12"` num `PIC 99`. `JUSTIFIED RIGHT` continua a decidir que extremidade
  enche e que extremidade se perde. A mesma regra governa os próprios bytes de um
  grupo: cada filho toma a sua fatia literalmente, pelo que um filho
  alfanumérico‑editado **não** é reeditado.
- ✅ **Uma cláusula `VALUE` num grupo** inicializa os bytes do grupo e é
  distribuída pelos seus filhos — `01 G VALUE "$123.45". 02 E PIC $999.99.` deixa
  `E` com `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — move cada item subordinado que os dois grupos
  partilham por nome, recorrendo pelos subgrupos correspondentes.
- ✅ **`CORRESPONDING` exclui um item descrito com `REDEFINES` ou `RENAMES`**
  (COBOL-85 6.18.4 GR1), em qualquer dos lados, junto com tudo o que lhe é
  subordinado. A exclusão está na *declaração*, não no nome: um item comum que
  apenas partilha o nome com um nível 66 noutro lugar continua a corresponder.
- ✅ **Qualquer dos dois operandos de `CORRESPONDING` pode nomear uma ocorrência
  de uma tabela de grupos** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` escreve
  os espaços próprios dessa ocorrência, e o subscrito é arrastado pela recursão.
- ✅ **Um par precisa que apenas UM dos seus dois itens seja elementar.** Um grupo
  pode enfrentar um item elementar, e o movimento entre eles é alfanumérico: um
  `PIC XXX` elementar que envia para um grupo de `999` + `XXX` enche os seus seis
  caracteres, e um grupo de `XXX` + `99` que envia para um `X(5)` simples enche‑o.
  Dois grupos frente a frente continuam a **recorrer** — esse emparelhamento não é
  o caso elementar. *(Antes de 1.62.39 nenhuma das direções movia nada: um grupo
  não possui espaço de armazenamento, pelo que a escrita ia para onde nada a
  volta a ler e a leitura dava a cadeia vazia.)*
- ✅ **Modificação de referência `id(start:len)`** — emissor (subcadeia) e recetor
  (atribuição parcial emendada); funciona nos operandos de todos os verbos.
  `length` é opcional. Endereça **posições de carácter**, pelo que um operando
  numérico é tomado com toda a largura da sua `PIC` e os seus zeros à esquerda:
  `01 T PIC 9(8) VALUE 00224845` dá `T(1:2)` = `"00"`, não `"22"`.
- ✅ **Os itens de grupo são agregados alfanuméricos** — um grupo *é* os seus itens
  subordinados postos de ponta a ponta, e o seu tamanho é a soma dos deles. Ler um
  concatena os filhos (incluindo `FILLER`); mover para um distribui os bytes por
  eles conforme a largura. `MOVE 11 TO A` é visível através do grupo que contém
  `A`, e `MOVE "1234" TO G` define os filhos de `G`, não um espaço próprio.
- ✅ subscritos `t(i)`, `t(i, j)` — leem/escrevem o espaço de armazenamento por
  ocorrência; os subscritos variáveis `t(WS-I)` são avaliados em cada acesso.
- ✅ qualificação `id OF/IN group` (`… OF g1 OF g2`) — resolve para o item correto
  mesmo quando o nome folha está declarado sob mais do que um grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por recetor** — cada recetor leva a sua própria marca `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico
  correspondente, recorrendo pelos subgrupos correspondentes.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **múltiplos recetores `GIVING`**, cada um com o seu próprio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sem `GIVING`) guarda `a/b` de volta em `a` (uma comodidade
  do PowerRustCOBOL; o COBOL padrão exige aqui `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **múltiplos recetores, cada um com o seu próprio `ROUNDED`**.
- ✅ operadores de expressão `+ - * /` e `**` (potência, associativa à direita),
  parênteses, `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **multissujeito com `ALSO`** — cada coluna `WHEN` é comparada posicionalmente
  com o seu sujeito e combinada com AND.
- ✅ **`WHEN NOT value`** nega um objeto de seleção; **`WHEN condition`**
  (p. ex. `EVALUATE TRUE WHEN a > b`) avalia a condição booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal inteiro ou dado).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` em linha,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` em linha (sem parágrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — executa o parágrafo em
  cada iteração (fora de linha, sem `END-PERFORM`).
- ✅ **`WITH TEST AFTER` aplica‑se a `VARYING`**, escrito de qualquer dos lados da
  frase e em linha ou fora de linha. O corpo corre uma vez antes de se testar
  qualquer coisa, e as condições são então testadas **de dentro para fora**; o
  nível cuja condição é falsa é incrementado, cada nível interior a ele reinicia no
  seu valor `FROM`, e o corpo corre de novo. Uma variável só é incrementada quando
  o seu teste sai falso, pelo que o teste que termina o ciclo deixa‑a como o corpo
  a deixou.
- ✅ **Uma variável `AFTER` é reposta no seu valor `FROM` quando o seu ciclo
  termina**, antes de o nível seguinte para fora ser incrementado (COBOL-85 6.20.4
  GR10(d)). Após o `PERFORM` completo, as variáveis interiores leem os seus valores
  `FROM` e apenas a mais exterior guarda o valor que o terminou.
- ✅ **Um identificador `VARYING` com subscrito segue o seu subscrito.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` incrementa
  a ocorrência que `S1` selecionar nesse momento, pelo que um corpo que avança `S1`
  percorre a tabela.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **O qualificador `{OF|IN} section` escolhe qual cópia se pretende** quando um
  nome de parágrafo se repete entre secções, exatamente como acontece no `PERFORM`.
  Uma secção **desconhecida** recai na procura não qualificada em vez de perder o
  salto. `GO TO … DEPENDING ON` toma uma lista simples de nomes e nenhum
  qualificador, e um `GO TO` que um `ALTER` tenha redirecionado segue o
  redirecionamento — que nomeia o seu próprio destino sem ambiguidade. *(Antes de
  1.62.39 o qualificador era analisado e depois ignorado, pelo que o salto aterrava
  na primeira definição em qualquer ponto do programa.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ o `EXIT` simples é um ponto de retorno sem efeito; `EXIT PROGRAM` devolve o
  controlo ao chamador.
- ✅ `EXIT PERFORM [CYCLE]` (quebra / continua o PERFORM em linha mais próximo),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfere o controlo para além do limite de frase seguinte
  (o analisador insere marcadores de limite em cada ponto; fiel, e não apenas um
  `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lê do operador** quando `SPECIAL-NAMES` declara o
  mnemónico (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — esse é o Formato 1, idêntico a um `ACCEPT id` simples.
  Um nome que **nenhuma cláusula `SPECIAL-NAMES` declara** conserva a extensão do
  PowerRustCOBOL e lê a **variável de ambiente** desse nome. Qual das duas se
  aplica é decidido pela declaração, nunca pela grafia. *(Antes de 1.62.35 a
  cláusula comum `<implementor-name> IS <mnemonic>` era saltada por completo, pelo
  que todos os mnemónicos liam uma variável de ambiente que nunca era definida e o
  item recetor ficava vazio.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` posiciona o cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (toda a linha de comandos) · `FROM ARGUMENT-NUMBER`
  (número de argumentos) · `FROM ARGUMENT-VALUE` (o argumento no apontador
  definido por `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (a variável nomeada por
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` fecha a instrução (opcional).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` fecha a lista de operandos (opcional), pelo que
  `DISPLAY A END-DISPLAY DISPLAY B` são duas instruções em vez de uma.
- ✅ formas de ecrã `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — executadas através de
  posicionamento de cursor ANSI + SGR em **modo CLI** (`rcrun`); ignoradas em modo
  GUI (aí o Form Designer substitui a E/S de SCREEN). `ACCEPT id AT …` posiciona e
  depois lê.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Transbordo = a cadeia montada é mais larga do que o campo recetor.
- ✅ **Uma frase `DELIMITED BY` governa toda a série de emissores que a precede**,
  e não apenas aquele depois do qual é escrita:
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` delimita os três e constrói
  `"ABC"`. Uma instrução pode levar várias frases, cada uma governando os emissores
  desde a anterior; os emissores posteriores à última frase tomam cada um por
  inteiro. *(Antes de 1.62.40 apenas o emissor escrito imediatamente antes da frase
  era delimitado.)*
- ✅ **`INTO` um item de grupo** distribui pelos itens subordinados do grupo.
- ✅ **O resultado é montado byte a byte**, pelo que `STRING HIGH-VALUE` move o
  único byte `0xFF` e ocupa uma posição de carácter.
- ✅ **Extensão — `DELIMITED BY` inteligente por omissão** (quando nenhuma frase
  governa um operando): os itens alfanuméricos `PIC X`/`A` assumem `SPACES` por
  omissão (o enchimento final é descartado); os literais de cadeia, os itens
  numéricos, os numéricos‑editados, os resultados de `FUNCTION` e as expressões
  assumem `SIZE`. Os dados são movidos na sua forma de campo (numérico → dígitos
  com toda a largura da PIC; numérico‑editado → caracteres editados).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Transbordo = mais campos de origem do
  que recetores.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **ambas as metades são aplicadas**.
- ✅ `BEFORE/AFTER INITIAL` confina cada frase a uma sub‑região do campo.
  (TALLYING acumula sobre o contador, conforme o COBOL.)
- ✅ **Uma série de operandos TALLYING partilha UMA ÚNICA passagem da esquerda
  para a direita** (COBOL-85 6.17.3). Em cada posição de carácter os operandos são
  tentados na ordem em que foram escritos; o primeiro que corresponde toma a
  posição e a passagem retoma para além dos caracteres que consumiu. Assim
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` sobre `"AABA"` dá `t1 = 1, t2 = 1` —
  escrever os operandos pela ordem inversa dá `t1 = 3, t2 = 0`. `LEADING` tem de
  corresponder desde a extremidade esquerda da sua janela sem intervalo, pelo que
  um operando anterior que tome essa posição termina a sequência antes de ela
  começar, e `CHARACTERS` conta apenas as posições que nenhum operando anterior
  reclamou.
- ✅ **Uma série de operandos REPLACING partilha UMA ÚNICA passagem também**, pela
  mesma regra: o primeiro operando que corresponde numa posição substitui esses
  caracteres e a passagem retoma para além deles, pelo que nenhum operando
  posterior os consegue ver. A janela `BEFORE`/`AFTER` de cada operando é fixada
  **antes de qualquer substituição**, que é o que permite ancorar um operando em
  caracteres que outro anterior sobrescreve:

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Aplicado um operando de cada vez, a primeira frase apagaria o `"L "` em que a
  segunda está ancorada, e `"BAD"` sobreviveria.
- ✅ **Um item DISPLAY com sinal não tem um `-` entre as suas posições de
  carácter.** O sinal operacional é uma sobreperfuração num dígito, pelo que
  `INSPECT <PIC S9(5) que contém -12345> TALLYING c FOR ALL "-"` dá **0** enquanto
  `FOR ALL "5"` dá 1. O sinal é restaurado depois, pelo que um `REPLACING` sobre os
  dígitos deixa‑o em paz. `SIGN IS … SEPARATE CHARACTER` é o caso em que o sinal
  *é* uma posição, e é contado.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado para MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` define o item anfitrião para o primeiro VALUE da
  condição; `TO FALSE` define um valor fora do conjunto de VALUE (com o melhor
  esforço — não há cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` e
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — ver **Apontadores** abaixo.

### INITIALIZE
- ✅ `INITIALIZE id …` — consciente de categorias: numérico / numérico‑editado →
  ZERO, todo o resto → SPACES, recorrendo pelos itens de grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — define cada item
  subordinado dessa categoria para o valor; os outros ficam intactos.

### Apontadores (USAGE POINTER)
- ✅ `USAGE POINTER` declara um apontador (NULL inicialmente).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — põe `id` como alias do
  armazenamento do destino (as leituras **e** as escritas seguem o alias);
  tipicamente um registo de LINKAGE. `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ O corpo de `ON EXCEPTION` / `ON OVERFLOW` corre quando o programa chamado não
  é resolvido; o corpo de `NOT ON EXCEPTION` corre quando a chamada **é
  resolvida**.
- ✅ `CANCEL program …` reinicializa a WORKING-STORAGE do programa nomeado, de modo
  que o seu próximo `CALL` começa de novo.

### Verbos de ficheiro (as frases suportadas — a cobertura completa está na suite de E/S de ficheiros)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` são analisados e respeitados onde fazem sentido —
  indicativos no modelo de uma única unidade de execução.)
- ✅ **Um só `OPEN` pode levar vários grupos de modo**, cada um com os seus próprios
  ficheiros: `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Cada grupo é aberto no seu
  próprio modo; `SHARING` / `WITH LOCK` / `REGISTERED USER` aplicam‑se à instrução.
- ✅ **Um `OPEN` de um ficheiro que já está aberto é `41`**, e o ficheiro é deixado
  como estava — a instrução **não** o reabre. (Reabrir um ficheiro `OUTPUT`
  truncaria em silêncio o que o programa já tivesse escrito.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensão do
  PowerRustCOBOL) — registra o operador/utilizador no registo de observabilidade do
  INDEXED (campo `user=` em cada linha de evento da sessão desse ficheiro).
  Puramente observacional; sem autenticação/autorização. Ver
  [`observability-pt.md`](observability-pt.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` liberta o bloqueio de registo que o motor INDEXED toma sob I‑O.
- ✅ **`READ … INTO id` é o `READ` seguido de um `MOVE` de grupo.** O registo é
  distribuído pelos itens subordinados do recetor conforme a largura e cortado à
  largura própria do recetor, o recetor pode levar subscrito, e o movimento
  transporta bytes — um registo que contém um byte que não é um carácter chega
  intacto.
- ✅ **Cláusula `RECORD` da FD — registos de comprimento variável.** As três
  grafias: `RECORD CONTAINS n CHARACTERS` (fixo),
  `RECORD CONTAINS n TO m CHARACTERS` (variável; a descrição de registo que o
  `WRITE` nomeia dá o comprimento), e
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (o dado *é* o comprimento — definido antes de um `WRITE`, reposto por um `READ`, e
  limitado ao intervalo declarado). Uma FD cujos registos `01` diferem em tamanho é
  de comprimento variável, diga‑o ou não. Um ficheiro de comprimento variável
  guarda o comprimento de cada registo junto ao registo, pelo que os seus bytes
  **não** são intercambiáveis com os de um ficheiro de comprimento fixo; um
  ficheiro de comprimento fixo não muda.
- ✅ **Os registos `01` de uma FD descrevem uma única área de registo.** Um `READ`
  entrega os bytes através de todas as descrições de registo; um `WRITE` envia a
  área completa, pelo que o que outra descrição de registo colocou onde a escrita
  tem `FILLER` transparece.
- ✅ **`FILLER` ocupa os seus bytes num registo de FD**, e
  `SIGN IS SEPARATE CHARACTER` torna um item DISPLAY com sinal um carácter mais
  largo do que as suas posições de dígito.
- ✅ **O `LINAGE` de uma FD aceita nomes de dados além de inteiros** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. A página é
  medida a partir desses itens em cada `WRITE`, pelo que um programa pode
  redimensioná‑la enquanto corre. `LINAGE-COUNTER` vale um quando o ficheiro é
  aberto.
- ✅ **Um `READ` sequencial depois de `AT END` é `46`, e não um segundo `10`.** O
  `AT END` não deixou um registo seguinte válido, pelo que continuar a ler é um erro
  diferente de chegar ao fim. `46` é um estado de classe 4, pelo que nem `AT END`
  nem `NOT AT END` correm para ele — o declarativo `USE` do ficheiro é o que o
  trata. Um `OPEN` novo, ou um `START` com êxito, volta a estabelecer um registo.
- ✅ `UNLOCK f [RECORD[S]]` liberta os bloqueios de registo do ficheiro.
- ✅ **`COMMIT` / `ROLLBACK`** — transações controladas pelo programa sobre **todos**
  os ficheiros INDEXED abertos. `OPEN` inicia uma transação; `COMMIT` confirma os
  `WRITE`/`REWRITE`/`DELETE` pendentes (um `ROLLBACK` posterior já não os consegue
  desfazer) e inicia uma nova; `ROLLBACK` desfaz todas as alterações desde o último
  `COMMIT`/`OPEN`. O armazenamento **DISK** torna `COMMIT`/`CLOSE` duráveis em
  disco. O armazenamento **MEMORY** mantém `COMMIT`/`ROLLBACK` puramente em RAM
  (nunca escreve para disco); um ficheiro `STORAGE IS MEMORY` simples é efémero, e
  `STORAGE IS MEMORY WITH PERSISTENCE` guarda para disco apenas no `CLOSE`. (A
  recuperação de falhas através de um registo de escrita antecipada durável é
  trabalho futuro — isto é reversão a nível de programa, dentro da execução.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (ficheiros INDEXED; extensão do PowerRustCOBOL). O armazenamento
  por omissão é `DISK`. `WITH COMPRESSION` comprime o registo armazenado (as chaves
  são avaliadas sobre o registo não comprimido); `WITH PERSISTENCE` (apenas MEMORY)
  guarda o ficheiro em RAM no `CLOSE`. `OPEN OUTPUT` (re)cria sempre o contentor em
  disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` num ficheiro SEQUENTIAL de registos** substitui o registo que o
  último `READ` entregou, no lugar, e deixa a posição de leitura onde estava — o
  `READ` seguinte continua a dar o registo que vem depois. Os estados que deve:
  **`49`** quando o ficheiro não está aberto em `I-O`, **`43`** quando nenhum `READ`
  com êxito estabeleceu um registo (incluindo depois de `AT END`, e num segundo
  `REWRITE` sem `READ` de permeio), e **`44`** quando o novo registo não tem o mesmo
  comprimento do lido — num ficheiro com `DEPENDING ON` o valor do item é esse
  comprimento, que é como um programa pede outro.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ A partilha de ficheiros entre *processos* não é imposta (uma única unidade de
  execução); as frases `SHARING`/`LOCK` são analisadas e os bloqueios de registo por
  execução do motor INDEXED são respeitados.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcionais, buffer de trabalho em memória)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (num INPUT PROCEDURE) acrescenta à execução;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devolve os registos.
- Os registos são ordenados de forma estável pelas chaves declaradas
  (`ASCENDING`/`DESCENDING`); `USING` lê / `GIVING` escreve os ficheiros
  sequenciais nomeados.

---

## Condições (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionais: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relações por palavras: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Um item cuja PICTURE **não leva sinal operacional** é `NUMERIC` apenas quando
  todas as posições de carácter contêm um dígito — um `PIC X(5)` que contém
  `"+1234"`, `"1.234"` ou `"12 45"` **não** é numérico. *(Antes de 1.62.40 o teste
  analisava os caracteres como um número, pelo que um sinal, um ponto decimal, um
  expoente e os espaços em volta eram todos aceites.)*
- ✅ **Um operando de `CLASS` definido pelo utilizador pode ser uma posição
  ordinal** — `CLASS ORDINAL-A-ONLY IS 66` nomeia o 66.º carácter do conjunto
  nativo — e o operando pode ficar na sua própria linha de fonte. O mesmo vale para
  `ALPHABET`.
- ✅ Sinal: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nome‑condição de nível 88 (o nome solto como condição).
- ✅ **`TRUE` / `FALSE` como operandos** (extensão do PowerRustCOBOL) — açúcar para
  `1` e `0`, onde quer que um valor seja permitido: `IF x = TRUE`,
  `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (a forma com `NOT` solto, sem operador
  relacional), `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`,
  `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE`, e `WHEN TRUE` contra um
  sujeito de valor. Um `TRUE`/`FALSE` solto é também uma condição completa
  (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ Isto **não** altera os dois lugares onde essas palavras já significavam algo:
  `SET <88‑name> TO TRUE` continua a definir o item anfitrião para um valor que
  satisfaz a condição (não o número 1), e `EVALUATE TRUE`/`EVALUATE FALSE` abaixo
  continuam a ser a instrução de casos padrão.
- ✅ `AND` / `OR` / `NOT` combinados, parênteses (AND liga mais forte do que OR).
- ✅ **Condições abreviadas com operador anteposto** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (o sujeito de comparação precedente é reutilizado).
- ✅ **Abreviação com objeto literal** — `a = 1 OR 2 OR 3` (reutiliza tanto o
  sujeito como o operador; o objeto é um literal).
- ✅ **Abreviação com objeto identificador** — `a = b OR c` (onde `c` é um dado).
  Um identificador solto depois de AND/OR após uma comparação é resolvido em tempo
  de execução: um nome‑condição de nível 88 conhecido é avaliado como tal, caso
  contrário é o objeto `a = c`. (Um identificador seguido imediatamente de `AND`
  conserva a precedência de AND.)
- ✅ **Um `NOT` antes do *objeto* de uma abreviação nega a relação**, e não o
  objeto: `a > b OR NOT c` é `a > b OR NOT (a > c)`. A grafia
  `NOT <operador relacional>` (`AND NOT < x`) é a forma de operador e não muda, e um
  `NOT` que abre uma condição comum — `NOT (…)`, `NOT x = y`, `NOT x NUMERIC` —
  conserva o seu próprio significado. *(Antes de 1.62.42 a forma de objeto era lida
  como «o objeto é diferente de zero», o que dá a mesma resposta apenas quando o
  objeto por acaso contém zero.)*
- ✅ **Um nome‑condição declarado num grupo testa os bytes do grupo.** Um grupo não
  possui armazenamento próprio — *é* os seus filhos — pelo que
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compara contra os seis
  caracteres que o registo contém.
- ✅ **Uma constante figurativa é repetida até ao tamanho do outro operando**, e
  isso inclui uma escrita como o `VALUE` de um 88: `88 B VALUE QUOTE` num anfitrião
  `PIC X(4)` são quatro aspas, e `88 D VALUE ALL "BAC"` é `"BACB"`.
  `ALL literal` é dimensionado em **ambas** as direções — `IF X EQUAL TO ALL "BA"`
  numa `X` de dez caracteres compara contra `"BABABABABA"`, e não contra `"BA"`
  enchido com espaços.

---

## Expressões, literais, USAGE

- ✅ Operadores aritméticos `+ - * /` e `**`; parênteses; `+`/`-` unários.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — intrínsecas **implementadas**:
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (com semente opcional), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (As conversões de data usam a base padrão 1601‑01‑01 = dia 1.) O **conjunto
  completo de intrínsecas padrão do COBOL‑85** está implementado.
- ✅ **Os registos de data e hora leem o relógio LOCAL.** `ACCEPT … FROM DATE /
  TIME / DAY / DAY-OF-WEEK` e `FUNCTION CURRENT-DATE` reportam todos a hora própria
  da máquina, e não UTC — incluindo a data, que difere de um lado e do outro da
  meia‑noite. Os últimos cinco caracteres de `CURRENT-DATE` levam o desvio **real**
  em relação a GMT (`…-0300`), pelo que um programa consegue saber em que fuso está
  a correr.
  ✅ Um nome de `FUNCTION` não reconhecido é um **erro de compilação** que nomeia a
  função, com uma sugestão quando uma real está perto o suficiente para ser um erro
  de escrita provável. Antes era analisado e devolvia **0** em tempo de execução, o
  que transformava um erro ortográfico numa resposta errada com toda a confiança
  (1.62.15).
- ✅ Literais: inteiro, decimal, cadeia, todas as constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Uma constante figurativa enche todo o seu recetor**, incluindo
  `HIGH-VALUE` — `MOVE HIGH-VALUE TO <PIC X(10)>` são dez bytes `0xFF`, e para um
  grupo é distribuída pelos filhos. Um recetor alfanumérico‑editado continua a
  colocar os seus caracteres de inserção, pelo que um `PIC XX0XXBXXX` contém
  `FF FF '0' FF FF ' ' FF FF FF`. Sob uma `PROGRAM COLLATING SEQUENCE` a constante
  nomeia um carácter comum e é esse carácter que enche.
  ⚠️ `HIGH-VALUE` é o **byte** `0xFF`, e não um carácter. A leitura de um operando
  de grupo, a edição e todos os caminhos de movimento transportam‑no byte a byte,
  mas **a modificação de referência ainda não é exata ao byte** —
  `IF X (1:1) = HIGH-VALUE` é falso para um item que genuinamente contém `0xFF`.
- ✅ **Um literal numérico pode começar pelo ponto decimal** — `.5`, `-.5`,
  `.000000001`. O COBOL‑85 exige apenas que um literal não *termine* num, pelo que
  `5.` continua a ser o número 5 seguido de um terminador de frase.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Os zeros à esquerda são significativos e exatos: `.000000001` é um milésimo de
  milionésimo, e não um décimo. Sob `DECIMAL-POINT IS COMMA` o mesmo vale para `,5`.
  O que separa o literal de um ponto de fim de frase é a **ausência de um espaço** —
  o COBOL‑85 exige um depois de um terminador, pelo que `MOVE X TO Y.` nunca é lido
  como o início de uma fração, e `MOVE X TO Y.5` é um erro de compilação em vez de
  uma reinterpretação silenciosa.
- ✅ **Sinalização de conformidade** (`cobolt_semantic::flagging`) — o padrão pede
  que uma implementação conforme seja capaz de dizer a um programa quais das
  funcionalidades que usa ficam fora de um nível de conformidade escolhido. Duas
  análises respondem a isso:
  - `flag_obsolete` — o conjunto de **elementos obsoletos** do COBOL‑85: os cinco
    parágrafos opcionais da IDENTIFICATION DIVISION, `MEMORY SIZE`, `ALTER`, `STOP`
    com um literal, e `GO TO` sem nome de procedimento.
  - `flag_high_subset` — tudo o que está acima do **subconjunto alto**, desde
    `COMPUTE`, `EVALUATE` e `INITIALIZE` passando por `CORRESPONDING`, a modificação
    de referência, a qualificação, `SET … TO TRUE` e um quarto subscrito, até
    continuar uma *palavra* ou um *literal numérico* através do limite de um cartão.
    (Continuar um literal **alfanumérico** está dentro do subconjunto e não é
    reportado.)

  Nenhuma das duas é verificação de erros, e nenhuma corre numa compilação comum:
  cada construção que nomeiam é COBOL‑85 válido que o RustCOBOL implementa e
  executa. São pontos de entrada separados precisamente para que uma compilação
  normal nunca comece a avisar sobre `AUTHOR` ou sobre `COMPUTE`. Os NIST `NC302M`,
  `NC303M` e `NC401M` validam‑nas — 7, 4 e 40 sinalizações, todas coincidentes.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — o carácter que enche uma
  posição de moeda numa PICTURE editada. **Substitui** o `$` em vez de se juntar a
  ele, pelo que assim que um programa declara um, `$` já não é um carácter de
  picture ali:
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` lê‑se então como `      <.00`, e `MOVE 1234` lê‑se como
  ` <1,234.00` — a série flutuante comporta‑se exatamente como `$$$,$$$.99` se
  comporta. Um símbolo de moeda que seja uma **letra** funciona do mesmo modo:
  `CURRENCY SIGN IS "W"` torna `PICTURE WWWWW` uma cadeia de moeda flutuante de
  cinco posições, pelo que `MOVE 12` se lê como `  W12`. *(Antes de 1.62.40 uma
  série de um símbolo de letra era lida como uma só palavra e rejeitada, pelo que
  apenas o `$` flutuava.)* O literal tem de ter um carácter, e o COBOL‑85 proíbe um
  que colidisse com um carácter de picture ou um separador: não um dígito, não um de
  `A B C D E G N P R S V X Z`, e nenhum de `space * + - , . ; ( ) " / =`.
- ✅ **Literais hexadecimais** — `X"09"`, `x'0D0A'` (em qualquer caixa, com
  qualquer aspa). Um carácter por **par** de dígitos hexadecimais, pelo que o número
  de dígitos tem de ser par; um número ímpar ou um dígito não hexadecimal é um
  literal malformado e é reportado, e não relido calmamente como a palavra `X` ao
  lado de uma cadeia. Utilizáveis onde quer que um literal entre aspas o seja
  (`DELIMITED BY`, `MOVE`, `VALUE`, comparações).

---

## Cláusulas da DATA DIVISION (sintaxe de declaração aceite)

- ✅ Níveis `01`–`49`, `77`, `88`; `FILLER`; grupo/elementar. A palavra `FILLER` é
  **opcional** — `05 PIC X VALUE ":".` declara um tal como
  `05 FILLER PIC X VALUE ":".`, e de qualquer das formas contém os seus bytes e o
  seu `VALUE` dentro do grupo que o contém.
- ✅ `PIC/PICTURE` com `X A 9 S V P` e símbolos de edição (`Z * $ + - CR DB B 0 /
  , .`). O símbolo de moeda é `$` a não ser que `SPECIAL-NAMES. CURRENCY` tenha
  nomeado outro — ver **Expressões, literais, USAGE** acima. **`P` é uma posição de
  escalonamento decimal** — uma posição de dígito que o item abrange mas não
  armazena: `PIC S999PP` contém três dígitos que representam centenas
  (`MOVE 12300` armazena‑o exatamente; `MOVE 12345` armazena 12300), e `PIC PP99`
  contém dois que representam décimos de milésimo. As posições que os `P` ocupam
  leem‑se sempre como zero e não ocupam **nenhum byte** na disposição de um registo.
- ✅ **A proteção com asteriscos enche todo o item.** Um valor zero numa picture
  cujas posições de dígito são todas `*` enche todas as posições de carácter com
  asteriscos — os dígitos fracionários, as vírgulas de agrupamento, um `$` fixo, e
  um `CR` ou `DB` final por igual — deixando apenas o próprio ponto decimal: um
  `PIC $**.**CR` que contém zero lê‑se `***.****`, e um `PIC *,***.**` lê‑se
  `*****.**`. Um valor **diferente** de zero protege apenas os zeros à esquerda,
  pelo que o `$` fixo conserva a sua própria posição (`-2.34` → `$*2.34CR`).
  *(Antes de 1.62.37 `CR`/`DB` contribuía com um asterisco em vez das duas posições
  de carácter que ocupa, pelo que tal item voltava um carácter mais curto do que a
  sua própria largura.)*
- ✅ **Um literal numérico move os seus caracteres, tal como está escrito.** Para um
  recetor alfanumérico um literal contribui com os dígitos que o programa escreveu,
  justificados à esquerda e enchidos com espaços — `MOVE 2 TO <PIC X(4)>` é
  `"2   "`, e `MOVE 060820000200 TO <seis filhos PIC 99>` enche‑os
  `06 08 20 00 02 00`. A largura do **recetor** nunca enche o literal; apenas a sua
  própria largura escrita o faz. *(Antes de 1.62.38 o lexer guardava apenas o valor,
  pelo que um zero à esquerda era perdido e todos os caracteres seguintes se
  deslocavam um lugar para a esquerda.)*
- ✅ **Uma relação entre um operando numérico e um não numérico é não numérica**
  (COBOL‑85 VI‑89 6.15.4 GR2). O operando numérico é tratado como se tivesse sido
  movido para um item alfanumérico do **seu próprio tamanho**, o que transfere as
  suas posições de carácter e **não o seu sinal operacional**: um `PIC S9(18)` que
  contém `-123456789012345678` compara como **igual** a um `PIC X(18)` que contém
  `"123456789012345678"`. Três condições limitam a regra — o operando numérico tem
  de ser um **inteiro**; «não numérico» é decidido pela **declaração**, pelo que um
  filho `PIC 99` que contém caracteres depois de um `MOVE` de grupo continua a ser
  numérico — e um **grupo** é não numérico sejam os seus filhos quais forem, pelo que
  um `PIC 9(5)` que contém 12345 frente a um grupo de dez bytes que contém
  `"0000012345"` é `"12345     "` e desigual; e `ALL literal` toma o tamanho do outro
  operando. *(Antes de 1.62.38 a comparação era algébrica sempre que o lado de texto
  por acaso fosse analisável como número.)*
- ✅ **Truncamento pela esquerda num MOVE numérico.** Um recetor contém exatamente
  os seus dígitos declarados em ambas as extremidades:
  `01 M PIC 99V999.  MOVE 123.45 TO M.` deixa `23.450`. A aritmética testa primeiro
  a capacidade do recetor, pelo que uma instrução com `ON SIZE ERROR` conserva em vez
  disso o valor antigo.
- ✅ **Uma tabela de grupos é endereçada por ocorrência.** `MOVE VALUES-1 TO
  GRP-1 (2)` distribui pelos filhos próprios dessa ocorrência
  (`ELEM1 (2,1) … ELEM1 (2,4)`), e ler `GRP-1 (2)` concatena exatamente esses. O
  registo `01` que os engloba são os bytes de **todas** as ocorrências, pelo que
  `MOVE GRP-TAB1 TO GRP-TAB2` copia uma tabela completa.
- ✅ **Os nomes de índice, os literais e a indexação relativa misturam‑se como
  subscritos.** `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — um sinal
  colado aos seus dígitos é um literal com sinal que abre o subscrito seguinte — e
  `ELEM1 (IN1 - 1, 3)`, onde o operador tem espaços de ambos os lados, é indexação
  relativa.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (e `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/com sinal/alfanumérico/figurativo/`ALL`). **`VALUE ALL
  "literal"` repete a sua unidade por todo o item** — `PIC X(6) VALUE ALL "ABC"` é
  `"ABCABC"` e `PIC X(9) VALUE ALL "XY"` é `"XYXYXYXYX"`. *(Antes de 1.62.40 apenas
  as constantes figurativas de um carácter enchiam o seu item e `ALL "literal"`
  deixava‑o a conter espaços.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — uma segunda leitura **viva** dos mesmos bytes. Não acrescenta
  armazenamento (pelo que não alarga o grupo que o contém), e uma escrita através de
  qualquer das descrições é visível através da outra:
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` é depois relido através de `RESULT-A`.
  ⚠️ **Ressalva:** uma sobreposição maior do que 256 espaços de armazenamento
  expandidos (uma tabela 10×10×10 redefinida, por exemplo) conserva em vez disso
  armazenamento por descrição — refrescá‑la em cada escrita percorreria mil
  ocorrências duas vezes.
- ✅ **As sobreposições aninham‑se.** Um `REDEFINES` dentro de um registo que por sua
  vez está redefinido é alcançado em ambas as direções, por profundo que seja:
  escrever dois bytes através de uma redefinição de nível 01 alcança o registo
  redefinido, o `REDEFINES` de um grupo dentro dele, e o `REDEFINES` de um item
  dentro *desse* — incluindo um 88 declarado no mais interno. Cada descrição é
  rematerializada uma vez por escrita. *(Antes de 1.62.42 uma chave que pertencia a
  mais do que uma sobreposição conservava apenas a declarada em último lugar, e um
  único guarda parava a cadeia depois do seu primeiro salto.)*
- ✅ **Uma descrição sem nome continua a ser uma descrição.**
  `02 FILLER REDEFINES <item>.` volta a descrever os bytes do seu alvo sob nenhum
  nome próprio, e uma escrita no alvo é visível através dos seus filhos. Vários
  filhos repartem esses bytes entre si, na ordem de disposição — a sobreposição *não*
  é um alias do seu primeiro filho. Dois `FILLER REDEFINES` de um mesmo item são duas
  leituras independentes, cada uma começando no **primeiro** byte do alvo. *(Antes de
  1.62.36 a um grupo redefinidor sem nome não era dada chave de armazenamento
  nenhuma, pelo que os seus filhos liam‑se como espaços por muito que o alvo tivesse
  sido enchido.)*
- ✅ **Um nome duplicado dentro de uma sobreposição** resolve para o mesmo
  armazenamento que o resto do programa alcança: `TAB-A` declarado sob dois grupos
  diferentes conserva uma leitura por declaração. *(Antes de 1.62.36 a cópia inicial
  da sobreposição era indexada a partir de um caminho a que faltavam os seus
  qualificadores exteriores, o que só um nome duplicado consegue distinguir — pelo
  que precisamente o caso que precisa do qualificador perdia‑o.)*
- ✅ `JUSTIFIED [RIGHT]` — **armazena alinhado à direita**, num item *alfanumérico*
  ou *alfabético*. Um emissor mais estreito do que o recetor é enchido pela
  esquerda; um emissor mais largo conserva a sua extremidade **direita**, perdendo os
  caracteres mais à esquerda — o contrário da regra comum. *(Antes de 1.62.40 a
  cláusula era registada apenas para itens alfanuméricos, pelo que
  `PICTURE A(5) JUSTIFIED RIGHT` era analisada e depois alinhava à esquerda como
  qualquer outro item.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — aceites;
  `SIGN … SEPARATE` ainda não altera como o item é armazenado.
- ✅ **Um `REDEFINES` no nível 01 pode descrever mais armazenamento do que o item que
  redefine**, e os bytes além do fim desse item pertencem à descrição que for
  suficientemente longa para os nomear. Escrever através de uma descrição mais curta
  deixa intacta a cauda da mais longa.
- ✅ **Uma sobreposição `REDEFINES` transporta os bytes do item redefinido**,
  incluindo para um par numérico: uma sobreposição `PIC S9(18)` de uma `X(18)` que
  contém `"00ABCDEFGHI  4321 "` relê esses caracteres, e `IS NUMERIC` responde
  **não** para eles. Quando os bytes formam de facto dígitos a leitura numérica não
  muda.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — **nomes‑condição reais**: o nível
  88 liga‑se ao seu item anfitrião; o teste verifica o anfitrião contra os VALUE /
  intervalos, e `SET 88-name TO TRUE` guarda um valor que os satisfaz no anfitrião.
- ✅ **Um nome‑condição pode ser declarado sob mais do que um grupo, e `OF`/`IN`
  distingue‑os** — exatamente como acontece para um nome de dado, e os níveis
  intermédios podem ser omitidos:
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  O subscrito pertence ao item anfitrião, pelo que seleciona contra que ocorrência os
  VALUE são testados. Uma referência **não qualificada** a um nome‑condição
  duplicado é ambígua no COBOL‑85; o runtime toma a primeira declaração, a mesma
  regra que aplica a um nome de dado ambíguo.
- ✅ `USAGE INDEX` declara um registo de índice inteiro (`SET`/`SEARCH` usam‑no);
  `USAGE POINTER` — ver **Apontadores** acima.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — um alias de reagrupamento; a
  leitura concatena os itens cobertos, a escrita distribui por largura de campo.
  - ✅ **Um 66 é qualificado pelo registo que reagrupa**, exatamente como um dado é
    qualificado pelo grupo acima dele, pelo que o mesmo nome de 66 pode ser declarado
    uma vez por registo e distinguido com `OF`/`IN`:
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Isto funciona igualmente em
    leituras e em escritas, e um 66 ganha sobre um dado comum que por acaso partilhe
    o seu nome. Os operandos da cláusula `RENAMES` resolvem nesse mesmo registo, pelo
    que um `NAME-2` duplicado nomeia o deste registo.
  - ✅ **Uma tabela coberta contribui com todas as ocorrências**, e não apenas a
    primeira: `66 R RENAMES ITEM-1 THRU TABLE-2`, onde `TABLE-2` contém
    `03 T PIC XXX OCCURS 5`, tem 20 caracteres de largura.
  - ✅ **Um 66 sobre exatamente um item *é* esse item** — mesma PICTURE, mesma
    categoria, mesmo armazenamento. `66 R RENAMES W` onde `W` é `PIC 9(4)` é um item
    numérico de quatro dígitos, pelo que `ADD 3500 TO R` com 8000 dentro provoca
    `ON SIZE ERROR` e deixa‑o sem alteração.
- Secções: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` é
  analisada mas não executada.

---

## Ainda NÃO suportado — lista de evitação atual

> **Corrigido em 2026‑08‑25.** Esta secção começava por dizer «O conjunto de verbos
> / cláusulas do COBOL‑85 está **coberto por completo**». Executar a suite NIST
> CCVS85 desmentiu‑o: **102 dos 434 programas dentro do âmbito falharam naquele
> dia**, sobre construções que este documento não listava como lacunas — vírgulas e
> pontos e vírgulas separadores, `FUNCTION x(ALL)`, `CLOSE … WITH LOCK`, `COPY` na
> Área B, entradas de comentário de IDENTIFICATION, números de prioridade de secção,
> nomes de dados que começam por dígito e — até 1.62.10 — literais numéricos com
> ponto decimal inicial. É para isso que serve uma suite de validação. Cada lacuna
> está agora especificada em [`specs/nist/`](../specs/nist/README.md) e seguida no
> [painel](#-a-conformidade-é-medida-não-afirmada--nist-ccvs85) acima.

A lista abaixo é o que está fora do âmbito **por intenção**, em contraste com as
lacunas do NIST acima, que são defeitos em vias de resolução:

1. **Edição de entrada no `ACCEPT` de ecrã** — `DISPLAY … AT/WITH` e `ACCEPT … AT`
   são executados (ANSI) em modo CLI, mas a edição completa da SCREEN SECTION ao
   nível do campo (tabulação automática, validação de campos, mapas de cor) fica
   **substituída pelo desenhador de formulários** em modo GUI.
2. **Partilha de ficheiros entre *processos*** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK` e `UNLOCK` são analisados e comandam os bloqueios de
   registo por execução do motor INDEXED, mas os bloqueios não são impostos entre
   processos distintos do sistema operativo (modelo de uma única unidade de
   execução).
3. **COBOL orientado a objetos** (definições de classe/método) — `INVOKE` é um no‑op
   para os objetos COBOL (comanda apenas objetos de GUI/runtime).
4. ✅ **Resolvido (1.62.15).** Um nome de função intrínseca não reconhecido devolvia
   **0** em silêncio, pelo que um programa calculava com toda a confiança uma
   resposta errada a partir de um erro de escrita. É agora um **erro de compilação**
   que nomeia a função e sugere a real mais próxima quando há uma correspondência
   suficientemente próxima (`cobolt-semantic/src/resolver.rs`,
   `Expr::FunctionCall`). Mantém‑se aqui porque a forma do «zero silencioso» é a
   armadilha que os pontos 5 e 6 ainda levam.
5. ⚠️ **Um valor inválido de `ACCESS MODE` / `ORGANIZATION` é engolido sem
   diagnóstico** — a mesma armadilha outra vez, e esta é disparada por um erro de
   escrita comum do utilizador. `ACCESS MODE IS` aceita apenas `SEQUENTIAL`,
   `RANDOM` ou `DYNAMIC` (`INDEXED` é uma *organização*, e não um modo de acesso),
   mas o analisador da cláusula SELECT testa esses três e deixa qualquer outra coisa
   cair no ramo genérico de «saltar um token desconhecido», pelo que o ficheiro
   conserva em silêncio o `SEQUENTIAL` por omissão e comporta‑se mal em tempo de
   execução em vez de não compilar. `ORGANIZATION IS` tem a forma idêntica
   (`cobolt-parser/src/parser.rs`, o ramo `Token::Access` e o ramo de organização
   acima dele). Ambas deveriam levantar um erro claro em tempo de compilação
   nomeando a palavra ofensora. **Nenhum módulo do NIST apanhará isto jamais** — a
   suite escreve apenas cláusulas válidas, pelo que todos os módulos podem terminar
   a 100 % com a lacuna ainda aberta. É uma armadilha de erro de escrita do
   utilizador, e precisa de um teste próprio em vez de uma pontuação de módulo.
6. ⚠️ **`ALPHABET … IS EBCDIC` é aceite mas deixa em vigor a ordenação nativa
   (ASCII).** A frase literal (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` e `STANDARD‑2` estão todas implementadas e comandam de facto
   `PROGRAM COLLATING SEQUENCE`; apenas a tabela EBCDIC falta, e nomeá‑la dá
   calmamente a ordem ASCII. Mesma família de armadilhas que 4–6.
7. **O módulo de Comunicação e o Report Writer** — ver
   [N/A acima](#-na--o-que-está-fora-do-âmbito-do-rustcobol-e-por-quê).

> **Resolvido (1.5.0):** o modelo de dados plano passou a hierárquico / consciente de
> ocorrências, desbloqueando **CORRESPONDING**, os **nomes qualificados**, a
> **subscrição de tabelas** e **`SEARCH`**.
> **Resolvido (1.6.0):** `MULTIPLY`/`DIVIDE` com múltiplos recetores + `ROUNDED` por
> recetor; `EXIT PERFORM/PARAGRAPH/SECTION`; `CALL NOT ON EXCEPTION`;
> `INSPECT TALLYING REPLACING` combinado + `BEFORE/AFTER INITIAL`; intrínsecas de
> data/`ANNUITY`; abreviação com objeto literal; `EVALUATE ALSO`/`WHEN NOT`;
> nomes‑condição de nível 88 reais; `PERFORM para VARYING`; e o runtime de
> `SORT`/`MERGE` com `RELEASE`/`RETURN`.
> **Resolvido (1.7.0):** abreviação com objeto identificador;
> `INITIALIZE … REPLACING`; `66 RENAMES`; apontadores (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`); `ALTER` / `UNLOCK`;
> `NEXT SENTENCE` fiel; as intrínsecas padrão restantes; e `ACCEPT`/`DISPLAY` de ecrã
> estendidos (executados em modo CLI).
> **Resolvido (1.7.1):** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (com os registos
> emparelhados `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Resolvido (1.7.2):** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (liberta os bloqueios de registo INDEXED) e `CANCEL program`.
> **Resolvido (1.8.0):** `COMMIT` / `ROLLBACK` como transações de ficheiros INDEXED
> controladas pelo programa (motores de memória e disco; registo de desfazer real em
> disco).

.<<

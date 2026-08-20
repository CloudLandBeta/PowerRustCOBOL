<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Referência da sintaxe suportada do RustCOBOL-85

**Fonte de verdade sobre o que o lexer / parser / runtime do RustCOBOL realmente
aceitam hoje**, derivada do código-fonte (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`). Escreva os seus testes contra as formas ✅; as formas ❌ não
serão analisadas ou são operações nulas, e as formas ⚠️ são analisadas mas se
comportam parcialmente. Este é o companheiro de
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md): a matriz diz *o
que* testar, este documento diz *qual grafia o RustCOBOL entende*.

Legenda: ✅ suportado · ⚠️ analisado, porém parcial/simplificado · ❌ não
reconhecido (evite, ou teste apenas para confirmar a lacuna).

> **Atualização (passagem de implementação de lacunas):** os itens a seguir foram
> implementados e agora são ✅ — **modificação de referência** `id(início:tam)`,
> **`PERFORM n TIMES` em linha**, **`SET … UP/DOWN BY`**, **`ON OVERFLOW` de
> STRING/UNSTRING + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE` ciente de
> categorias**, **condições abreviadas prefixadas por operador**
> (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (executa em CALL não resolvido),
> **`COMPUTE` com múltiplos receptores + `ROUNDED` por receptor**, e um conjunto
> de **funções intrínsecas** bem maior.
>
> **Atualização (passagem de ambiente hierárquico / ciente de ocorrências —
> 1.5.0):** quatro recursos bloqueados pelo modelo de dados agora são ✅ —
> **subscrição de tabelas em tempo de execução** `t(i)` / `t(i, j)`
> (armazenamento por ocorrência), **desambiguação de nomes qualificados**
> `id OF/IN grupo` (nomes-folha duplicados resolvem para armazenamentos
> independentes), **`MOVE/ADD/SUBTRACT CORRESPONDING`** e **`SEARCH` /
> `SEARCH ALL` funcionais**.
>
> **Atualização (passagem de completude de verbos — 1.6.0):** agora também ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` com múltiplos receptores + `ROUNDED` por
> receptor** em `ADD`/`SUBTRACT`; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** e o `EXIT` simples corrigido; **`CALL … NOT ON EXCEPTION`**;
> **`INSPECT … TALLYING … REPLACING`** combinado e as regiões
> **`BEFORE/AFTER INITIAL`**; **intrínsecas** de data e financeiras
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`); **condições abreviadas com objeto literal**
> (`A = 1 OR 2 OR 3`); **`EVALUATE … ALSO`** (multi-sujeito) e **`WHEN NOT`**;
> **nomes-condição de nível 88 reais** (`SET … TO TRUE/FALSE`, o hospedeiro é
> testado contra os seus VALUE / faixas); **`PERFORM para VARYING`**; e um runtime
> funcional de **`SORT`/`MERGE`** (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). A lista de itens a evitar no final está atualizada.
>
> **Atualização (passagem de esvaziamento da lista a evitar — 1.7.0):** as
> lacunas restantes já foram implementadas — **abreviação com objeto
> identificador** (`a = b OR c`, resolvida via metadados de nível 88);
> **`INITIALIZE … REPLACING categoria DATA BY valor`**; **`66 RENAMES`** (a
> leitura sintetiza / a escrita distribui entre os itens cobertos);
> **ponteiros** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`, aliasing com
> `SET ADDRESS OF item TO …`, `IF ptr = NULL`); **`ALTER`** / **`UNLOCK`**;
> **`NEXT SENTENCE`** fiel; as **intrínsecas** padrão restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`); e
> o **`ACCEPT`/`DISPLAY` de tela** estendido (`AT`/`WITH` via ANSI no modo CLI —
> agora *executado*, não apenas analisado).
>
> **Atualização (1.7.1):** as fontes de registrador do `ACCEPT` agora são
> funcionais (antes eram operações nulas reconhecidas) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (emparelhadas com
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (emparelhada com
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Atualização (1.7.2):** cláusulas de compartilhamento / travamento de arquivos
> e `CANCEL` (antes ❌ / operação nula) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libera as travas de registro INDEXED
> do arquivo) e **`CANCEL programa`** (reinicializa o armazenamento do programa).
>
> **Atualização (1.8.0):** **`COMMIT` / `ROLLBACK`** agora são verbos COBOL
> reais — transações controladas pelo programa sobre os arquivos INDEXED abertos
> (tanto no motor de memória quanto no de disco). O motor de disco ganhou um log
> de desfazer real durante a execução (antes era uma operação nula). A lista de
> itens a evitar no final está atualizada.

---

## Comandos reconhecidos (verbos)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redireciona o `GO TO` de para-1) ·
`UNLOCK file` (libera as travas de registro do arquivo) ·
`OPEN … SHARING/WITH LOCK` · `READ … WITH [NO] LOCK` (compartilhamento/travamento
de arquivos — consultivo dentro da única unidade de execução)
✅ `COMMIT` / `ROLLBACK` (transações de arquivos INDEXED controladas pelo
programa — veja Verbos de arquivo) · `CANCEL` (reinicializa o armazenamento do
programa) · ⚠️ `INVOKE` (analisado como operação nula)
Extensões do projeto: `EXEC RUST … END-EXEC`,
`TRY/CATCH/FINALLY/END-TRY`, `THROW`. Um bloco pode fazer `use` das crates sempre
vinculadas (std, egui, eframe e o conjunto do runtime vinculado) **mais qualquer
crate que o project registre em Project's Crates** (spec 044): as crates
registradas são fixadas numa versão exata, incorporadas ao `crates/` do project e
compiladas dentro do binário; crates não registradas fazem o Check/Build falhar
na linha do desenvolvedor, com o remédio indicado.

✅ `SEARCH` (serial) / `SEARCH ALL` (busca binária sobre uma tabela com
`ASCENDING`/`DESCENDING KEY` — executa o primeiro `WHEN` que casar, senão
`AT END`).
✅ `SORT` / `MERGE` com `RELEASE` / `RETURN` (funcionais — veja abaixo).
✅ `DECLARATIVES … END DECLARATIVES` com `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — tratadores de erro de arquivo
disparados diante de um `FILE STATUS` de erro não tratado.
❌ **Não reconhecidos — não use:** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formas suportadas por verbo

### MOVE
- ✅ `MOVE {id|lit|figurativa} TO id1 [id2 …]` (vários receptores).
- ✅ `MOVE CORRESPONDING g1 TO g2` — move cada item subordinado que os dois grupos
  compartilham por nome, percorrendo recursivamente os subgrupos correspondentes.
- ✅ **Modificação de referência `id(início:tam)`** — como emissor (substring) e
  como receptor (atribuição parcial encaixada); funciona nos operandos de todos os
  verbos. `tam` é opcional.
- ✅ subscritos `t(i)`, `t(i, j)` — leem/escrevem o slot de armazenamento daquela
  ocorrência; subscritos variáveis `t(WS-I)` são avaliados a cada acesso.
- ✅ qualificação `id OF/IN grupo` (`… OF g1 OF g2`) — resolve para o item correto
  mesmo quando o nome-folha está declarado sob mais de um grupo.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` por receptor** — cada receptor carrega o seu próprio indicador
  `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combina cada par numérico
  correspondente, percorrendo recursivamente os subgrupos correspondentes.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **vários receptores `GIVING`**, cada um com o seu próprio `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sem `GIVING`) guarda `a/b` de volta em `a` (uma conveniência
  do PowerRustCOBOL; o COBOL padrão exige aqui `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **vários receptores, cada um com o seu próprio `ROUNDED`**.
- ✅ operadores de expressão `+ - * /` e `**` (potência, associativa à direita),
  parênteses, `FUNCTION nome(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] comandos [ELSE comandos] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO sujeito …]` … `WHEN {valor | valor THRU
  valor | NOT valor | condição | ANY} [ALSO …] comandos … [WHEN OTHER comandos]
  END-EVALUATE`.
- ✅ **`ALSO` multi-sujeito** — cada coluna `WHEN` é comparada posicionalmente com
  o seu sujeito e combinada com AND.
- ✅ **`WHEN NOT valor`** nega um objeto de seleção; **`WHEN condição`**
  (p. ex. `EVALUATE TRUE WHEN a > b`) avalia a condição booleana.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = literal inteiro ou data item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` em linha,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` em linha (sem parágrafo).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — executa o parágrafo a
  cada iteração (fora de linha, sem `END-PERFORM`).

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ o `EXIT` simples é um ponto de retorno sem efeito; `EXIT PROGRAM` retorna ao
  chamador.
- ✅ `EXIT PERFORM [CYCLE]` (interrompe / continua o PERFORM em linha mais
  próximo), `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfere o controle para além do próximo limite de
  sentença (o parser insere marcadores de limite em cada ponto final; fiel, não um
  mero `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemônico}`.
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` posiciona o cursor (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (a linha de comando inteira) · `FROM ARGUMENT-NUMBER`
  (quantidade de argumentos) · `FROM ARGUMENT-VALUE` (o argumento no ponteiro
  definido por `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (a variável nomeada por
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemônico] [[WITH] NO ADVANCING]`.
- ✅ formas de tela `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — executadas via
  posicionamento de cursor ANSI + SGR no **modo CLI** (`rcrun`); ignoradas no modo
  GUI (ali o form designer substitui a E/S de SCREEN). `ACCEPT id AT …` posiciona
  e então lê.

### STRING
- ✅ `STRING {origem [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO destino
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Estouro = a string montada é mais larga que o campo receptor.
- ✅ **Extensão — `DELIMITED BY` inteligente por padrão** (quando a cláusula é
  omitida num operando): itens alfanuméricos `PIC X`/`A` assumem `SPACES` (o
  preenchimento final é descartado); literais de string, numéricos, numéricos
  editados, resultados de `FUNCTION` e expressões assumem `SIZE`. Data items são
  movidos na sua forma de campo (numérico → dígitos com a largura completa do PIC;
  numérico editado → caracteres editados).

### UNSTRING
- ✅ `UNSTRING origem [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Estouro = mais campos de origem do que
  receptores.

### INSPECT
- ✅ `INSPECT id CONVERTING de TO para`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **ambas as metades são aplicadas**.
- ✅ `BEFORE/AFTER INITIAL` confina cada cláusula a uma sub-região do campo.
  (TALLYING acumula sobre o contador, conforme o COBOL.)

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilado para MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (codificado como ADD / SUBTRACT).
- ✅ `SET 88-nome TO TRUE` coloca no item hospedeiro o primeiro VALUE da condição;
  `TO FALSE` coloca um valor fora do conjunto de VALUE (melhor esforço — não há
  cláusula FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | outro-ptr}` e
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — veja **Ponteiros** abaixo.

### INITIALIZE
- ✅ `INITIALIZE id …` — ciente da categoria: numérico / numérico editado → ZERO,
  todo o resto → SPACES, percorrendo recursivamente os itens de grupo.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY valor …` — coloca cada item
  subordinado daquela categoria no valor; os demais ficam intactos.

### Ponteiros (USAGE POINTER)
- ✅ `USAGE POINTER` declara um ponteiro (NULL inicialmente).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — faz de `id` um alias do
  armazenamento do alvo (tanto leituras **quanto** escritas seguem o alias);
  tipicamente um registro de LINKAGE. `IF ptr = NULL` funciona.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ O corpo de `ON EXCEPTION` / `ON OVERFLOW` executa quando o programa chamado
  não é resolvido; o corpo de `NOT ON EXCEPTION` executa quando a chamada **é
  resolvida**.
- ✅ `CANCEL programa …` reinicializa a WORKING-STORAGE do programa nomeado, de
  modo que o próximo `CALL` comece do zero.

### Verbos de arquivo (as cláusulas suportadas — a cobertura completa está na suíte de E/S de arquivos)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]`; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` são analisados e respeitados onde fazem sentido —
  consultivos no modelo de unidade de execução única.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extensão do
  PowerRustCOBOL) — registra o operador/usuário no log de observabilidade INDEXED
  (campo `user=` em toda linha de evento da sessão daquele arquivo). Puramente
  observacional; sem autenticação/autorização. Veja
  [`observability.md`](observability.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libera a trava de registro que o motor INDEXED toma em I-O.
- ✅ `UNLOCK f [RECORD[S]]` libera as travas de registro do arquivo.
- ✅ **`COMMIT` / `ROLLBACK`** — transações controladas pelo programa sobre
  **todos** os arquivos INDEXED abertos. `OPEN` inicia uma transação; `COMMIT`
  confirma os `WRITE`/`REWRITE`/`DELETE` pendentes (um `ROLLBACK` posterior já não
  consegue desfazê-los) e inicia outra; `ROLLBACK` desfaz toda alteração desde o
  último `COMMIT`/`OPEN`. O armazenamento **DISK** torna `COMMIT`/`CLOSE`
  duráveis em disco. O armazenamento **MEMORY** mantém `COMMIT`/`ROLLBACK`
  puramente em RAM (nunca escreve em disco); um arquivo `STORAGE IS MEMORY` comum
  é efêmero, e `STORAGE IS MEMORY WITH PERSISTENCE` grava em disco apenas no
  `CLOSE`. (A recuperação de falhas via um log write-ahead durável é trabalho
  futuro — isto é rollback em nível de programa, dentro da execução.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (arquivos INDEXED; extensão do PowerRustCOBOL). O armazenamento
  padrão é `DISK`. `WITH COMPRESSION` comprime o registro armazenado (as chaves
  são avaliadas sobre o registro não comprimido); `WITH PERSISTENCE` (somente
  MEMORY) grava o arquivo em RAM no `CLOSE`. `OPEN OUTPUT` sempre (re)cria o
  contêiner em disco.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]`;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ O compartilhamento de arquivos entre *processos* não é imposto (unidade de
  execução única); as cláusulas `SHARING`/`LOCK` são analisadas e as travas de
  registro por execução do motor INDEXED são respeitadas.

### SORT / MERGE / RELEASE / RETURN  ✅ (funcionais, buffer de trabalho em memória)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dentro de um INPUT PROCEDURE) acrescenta à
  execução; `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` devolve os
  registros.
- Os registros são ordenados de forma estável pelas chaves declaradas
  (`ASCENDING`/`DESCENDING`); `USING` lê e `GIVING` escreve os arquivos
  sequenciais nomeados.

---

## Condições (IF / EVALUATE / PERFORM UNTIL)

- ✅ Símbolos relacionais: `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relações por palavra: `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe: `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
- ✅ Sinal: `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nome-condição de nível 88 (o nome sozinho como condição).
- ✅ `AND` / `OR` / `NOT` combinados, parênteses (AND liga mais forte que OR).
- ✅ **Condições abreviadas prefixadas por operador** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (o sujeito da comparação anterior é reaproveitado).
- ✅ **Abreviação com objeto literal** — `a = 1 OR 2 OR 3` (reaproveita tanto o
  sujeito quanto o operador; o objeto é um literal).
- ✅ **Abreviação com objeto identificador** — `a = b OR c` (onde `c` é um data
  item). Um identificador sozinho após AND/OR que segue uma comparação é resolvido
  em tempo de execução: se for um nome-condição de nível 88 conhecido, avalia como
  tal; caso contrário, é o objeto `a = c`. (Um identificador imediatamente seguido
  de `AND` mantém a precedência de AND.)

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
  (As conversões de data usam a base padrão 1601-01-01 = dia 1.) O **conjunto
  completo de intrínsecas do padrão COBOL-85** está implementado.
  ⚠️ Qualquer nome de `FUNCTION` não reconhecido ainda é analisado, mas retorna
  **0** em tempo de execução.
- ✅ Literais: inteiro, decimal, string, todas as constantes figurativas
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Literais hexadecimais** — `X"09"`, `x'0D0A'` (qualquer caixa, qualquer tipo
  de aspas). Um caractere por **par** de dígitos hexadecimais, portanto a
  quantidade de dígitos deve ser par; uma quantidade ímpar ou um dígito não
  hexadecimal é um literal malformado e é reportado, em vez de silenciosamente
  relido como a palavra `X` ao lado de uma string. Utilizáveis onde quer que um
  literal entre aspas seja válido (`DELIMITED BY`, `MOVE`, `VALUE`, comparações).

---

## Cláusulas da DATA DIVISION (sintaxe de declaração aceita)

- ✅ Níveis `01`–`49`, `77`, `88`; `FILLER`; de grupo / elementares.
- ✅ `PIC/PICTURE` com `X A 9 S V P` e símbolos de edição (`Z * $ + - CR DB B 0 /
  , .`).
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (e `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérico/com sinal/alfanumérico/figurativo/`ALL`).
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES`, `JUSTIFIED [RIGHT]`, `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL`.
- ✅ `88 nome VALUE v [v …]` / `VALUE a THRU b` — **nomes-condição reais**: o
  nível 88 se liga ao seu item hospedeiro; o teste confere o hospedeiro contra os
  VALUE / faixas, e `SET 88-nome TO TRUE` grava no hospedeiro um valor que a
  satisfaz.
- ✅ `USAGE INDEX` declara um registrador de índice inteiro (`SET`/`SEARCH` o
  usam); `USAGE POINTER` — veja **Ponteiros** acima.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — um alias de reagrupamento;
  a leitura concatena os itens cobertos, a escrita os distribui pela largura de
  campo.
- Seções: `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE`; `SCREEN` é
  analisada mas não executada.

---

## Ainda NÃO suportado — lista atual de itens a evitar

O conjunto de verbos e cláusulas do COBOL-85 está **totalmente coberto**. O que
resta fora do escopo é intencional ou posterior ao 85:

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
4. Organização de arquivos **RELATIVE** (SEQUENTIAL / LINE SEQUENTIAL / INDEXED
   estão prontas).
5. Nomes de função intrínseca não reconhecidos ainda retornam **0**.

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

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Matriz de testes de verbos e seções de dados do RustCOBOL‑85

Uma especificação de testes para concluir o COBOL‑85 dentro do escopo do projeto.
Ela enumera, **em profundidade**, o que *ainda não está coberto* pelas suítes
existentes, na forma de esqueletos de sintaxe + eixos de permutação + a mistura de
tipos de dados com que cada verbo precisa ser exercitado. O objetivo destes testes
é **exploratório**: executar cada variação, observar o comportamento atual e
decidir o que corrigir / ajustar / criar / remover.

> Já verificado — NUNCA especificar de novo aqui: aritmética numérica exata
> (valores de resultado de ADD/SUB/MUL/DIV/COMPUTE, ROUNDED, ON SIZE ERROR),
> PICTUREs numeric‑edited + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, toda a E/S de
> arquivos (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, chaves,
> START/REWRITE/DELETE/INVALID KEY, STORAGE MODE MEMORY/DISK, compressão,
> persistência de MEMORY), programas aninhados/CALL básico, comparação
> alfanumérica, lexer fixo/livre. (As permutações de *sintaxe* aritmética abaixo
> continuam no escopo — só a matemática dos valores está "pronta".)

## Notação

- `[ x ]` opcional, `{ a | b }` escolha, `…` repetição, `dn` = item de dados n.
- **Eixo de mistura de tipos (T):** cada posição de operando precisa ser exercitada
  com estas espécies de receptor/emissor, nos dois sentidos quando cabível:
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Valores-limite por espécie:** vazio, mínimo, máximo, estouro por um,
  tudo espaços, tudo zeros, sinal em LEADING/TRAILING [SEPARATE], escalado com P,
  ponto implícito por V.
- Para cada verbo, capturar: valor(es) de resultado, **FILE STATUS / registradores especiais**
  (`RETURN-CODE`, `TALLY`), ramo de estouro/exceção tomado e inalterado-em-erro.

---

## Parte A — Seções da DATA DIVISION (comportamentos não testados)

### WORKING-STORAGE SECTION
- **Níveis:** 01, aninhamento 02–49, 77 (independente), 66 `RENAMES a THRU b`, 88.
- **PIC:** `X A 9 S V P` com `(n)`; escalonamento com `P` (à esquerda/à direita); ponto
  implícito `V`; combinações editadas; grupo com `PIC` versus grupo sem PIC.
- **USAGE:** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2, COMP‑3/PACKED‑DECIMAL,
  COMP‑5, INDEX, POINTER — declaração + tamanho de armazenamento + ida e volta do valor.
- **VALUE:** numérico, com sinal, alfanumérico, figurativo, `ALL "x"`; VALUE em grupo;
  VALUE ilegal (tamanho > PIC).
- **OCCURS:** fixo; `DEPENDING ON`; `INDEXED BY`; `ASCENDING/DESCENDING KEY`;
  multidimensional (2–3); OCCURS em grupo.
- **Cláusulas:** REDEFINES (igual/menor/maior, encadeado), RENAMES, JUSTIFIED RIGHT,
  BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`, SYNCHRONIZED, FILLER.
- **Nomes de condição 88:** valor único, lista de valores, `VALUE a THRU b`, vários
  intervalos, sobre hospedeiro numérico / alfanumérico / editado; avaliação + `SET … TO TRUE`.
- **Inicialização:** padrão (espaços/zeros conforme a classe) versus VALUE; **persistência
  através de PERFORM e através de CALL** (a WS mantém o último valor).

### LOCAL-STORAGE SECTION
- **Reinicializada a cada entrada no programa** (em contraste com a persistência da WS).
- Cláusulas VALUE **reaplicadas a cada entrada**.
- **Recursão:** cada CALL (recursivo) recebe uma instância independente de LOCAL-STORAGE.
- A mesma cobertura de cláusulas da WS (OCCURS/REDEFINES/88/…), mas verificando a semântica de reinicialização.

### LINKAGE SECTION
- Os itens **não têm armazenamento até serem vinculados** pelo chamador; acesso a linkage não vinculada.
- Vinculados via `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (o chamador vê as mudanças) versus **BY CONTENT** (o chamado edita uma cópia)
  versus **BY VALUE** (escalar).
- Grupo + elementar, OCCURS, REDEFINES, 88 na linkage.
- Divergência de tamanho/USAGE entre o parâmetro real e o formal (comportamento a observar).
- `ADDRESS OF` / `SET ADDRESS OF … TO` e vinculação de POINTER (se houver suporte).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — vinculação posicional aos argumentos do CALL;
  divergência de quantidade (menos/mais argumentos); ordem.
- `BY REFERENCE | BY VALUE` por parâmetro na lista USING.
- `RETURNING dn` — valor devolvido a `CALL … RETURNING`; versus `GIVING`; versus
  `RETURN-CODE`.
- `USING` do programa principal vinculado a partir da linha de comando (se houver suporte).
- Mistura de tipos em cada posição de parâmetro (aplicar **T**).

---

## Parte B — Matriz de permutações de verbos

Exercite cada verbo ao longo de **T** em cada posição de operando. Abaixo estão as
permutações *estruturais* (cláusulas/frases) que se somam à mistura de tipos.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (vários receptores).
- `MOVE CORRESPONDING g1 TO g2` (casamento de elementares pelo nome).
- Origem/destino com modificação de referência: `MOVE a(s:l) TO b(s:l)`.
- Com subscritos: `MOVE t(i) TO u(j)`, `t(i,j)`.
- Conversões de tipo (aplicar **T** nos dois sentidos): num→edited, edited→num, alnum→num,
  num→alnum (justificar/preencher/truncar), group→group (cópia de bytes), tratamento do sinal,
  COMP‑3↔DISPLAY, float↔fixed, figurative→cada espécie.

### DISPLAY
- `DISPLAY {dn|literal} …` (operandos concatenados).
- `[WITH NO ADVANCING]`; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Forma de tela (observar/decidir): `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Mistura de tipos: numérico (largura PIC completa), editado, com sinal, grupo, figurativo.

### ACCEPT  *(especificar todas as formas; muitas são de tela/terminal — sinalizar para decisão de escopo)*
- `ACCEPT dn` (do console para alnum / numeric / edited / group).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Formas de tela: `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Recebimento em numérico versus numeric-edited versus alnum (des-edição / validação).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Vários receptores, cada um com seu próprio comportamento ROUNDED/de tamanho; operandos
  com USAGE misto (COMP‑3 + DISPLAY + editado); com sinal; operandos com modificação de referência.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- Divisão por zero → ON SIZE ERROR; sinal/escala do REMAINDER; USAGE misto.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Operadores `+ - * / **`, parênteses, precedência; funções intrínsecas na expressão;
  operandos com USAGE misto; vários receptores; truncamento versus ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — aninhamento, ramos vazios, `NEXT SENTENCE`.
- Condições: de relação (`= < > <= >= NOT`), de classe (`IS [NOT] {NUMERIC|ALPHABETIC|
  ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), de sinal (`POSITIVE|NEGATIVE|ZERO`),
  referência a condição 88, combinadas (`AND/OR/NOT`), **abreviadas** (`a = b OR c`),
  entre parênteses.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` com
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Mistura de tipos nas comparações (numérico versus alnum versus editado versus figurativo).

### PERFORM
- Fora de linha: `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = literal / item de dados).
- `PERFORM … UNTIL cond` com `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- Em linha: `PERFORM … END-PERFORM` (com TIMES/UNTIL/VARYING).
- PERFORM aninhado/recursivo; sobreposição de intervalos; variável de laço índice versus numérica.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`; `GO TO p1 p2 … DEPENDING ON dn` (dentro/fora do intervalo).
- `CONTINUE`; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (do principal versus de um subprograma).

### SET
- `SET index TO {n|index}`; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}`; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (onde houver suporte).

### INITIALIZE
- `INITIALIZE dn …` (grupo/elementar; padrão conforme a categoria).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]`; tabelas (todas as ocorrências).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (serial).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH` (binária;
  exige `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Encontrado/não encontrado; vários WHEN; mistura de tipos de chave; comportamento com tabela não ordenada.

### STRING  *(exercitar o estilo de permutação do usuário)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutações a cobrir:
  - origem única `DELIMITED BY SIZE` → destino alnum.
  - várias origens, **delimitadores mistos**: `STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`.
  - muitas origens/delimitadores: `STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` início/avanço; ponteiro fora do intervalo → estouro.
  - destino pequeno demais → `ON OVERFLOW`; `NOT ON OVERFLOW`.
  - **origens com mistura de tipos:** numérico, numeric-edited, com sinal, grupo, figurativo,
    com modificação de referência — observar como cada um é convertido em cadeia.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutações: um delimitador versus vários, `ALL` (colapsa repetições), `OR`,
  captura com `DELIMITER IN`/`COUNT IN`, POINTER, TALLYING, mais campos do que dados
  (estouro), destinos de tipo misto (os receptores numéricos são des-editados).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combinado).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- Escopo BEFORE/AFTER; correspondências sobrepostas; padrões de vários caracteres; hospedeiro com mistura de tipos.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Nome de programa estático (literal) versus dinâmico (nome de dado); não resolvido → ON EXCEPTION.
- Modos de passagem de argumentos (observar a visibilidade pelo chamador); divergência de quantidade/tipo de argumentos.
- `RETURNING` versus `RETURN-CODE`; recursão; dados compartilhados `EXTERNAL`.
  (✅ `CANCEL prog` implementado — reinicializa o armazenamento do programa;
  `NOT ON EXCEPTION` roda em um CALL resolvido.)

### Registradores especiais de ARITHMETIC e verbos diversos
- Supressão de zeros de `ADD/SUBTRACT … GIVING` versus a acumulação de `TO`.
- `MOVE`/aritmética de/para `RETURN-CODE`, `TALLY`.
- ✅ `ALTER` (GO TO legado) — implementado (redireciona o `GO TO` do parágrafo).
- Ida e volta de `ACCEPT/DISPLAY` através de campos editados.

### Verbos de arquivo — *(apenas as lacunas que não estão na suíte de E/S de arquivos)*
- ✅ **Implementado e testado** (`test_file_locking`): `OPEN … SHARING WITH …
  [WITH LOCK]`, `READ … WITH [NO] LOCK`, `UNLOCK` (consultivo dentro da única
  unidade de execução — veja a referência de sintaxe suportada).
- `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  com chaves com modificação de referência; vários FDs compartilhando uma área de registro.

### Verbos planejados (especificação para quando forem implementados)
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- Organização `RELATIVE`: `READ/WRITE/REWRITE/DELETE/START` por `RELATIVE KEY`.

---

## Parte C — Bancada de equivalência entre formas

Para um conjunto selecionado dos programas acima, afirmar que a saída observável é
**idêntica** (texto do DISPLAY, FILE STATUS, RETURN-CODE, conteúdo dos arquivos) nas
três formas de execução do mesmo fonte:

1. **Interpretador** (`Interpreter::run`).
2. **Ida e volta do AST** — serializar (`bincode`+`flate2`) → desserializar → executar;
   afirmar que o AST é idêntico byte a byte e que a saída é idêntica.
3. **Binário empacotado/compilado** — `cobolt_compiler::build_project` → executar o
   binário produzido; afirmar que a saída é idêntica.

Qualquer divergência entre as formas é um defeito a registrar (o invariante
"um compilador, um comportamento").

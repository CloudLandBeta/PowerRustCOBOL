<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Matriz de testes de verbos e secções de dados do RustCOBOL‑85

Uma especificação de testes para terminar o COBOL‑85 dentro do escopo do
projeto. Enumera, **em profundidade**, o que *ainda não está coberto* pelos
conjuntos de testes existentes, sob a forma de esqueletos de sintaxe + eixos de
permutação + a mistura de tipos de dados com que cada verbo tem de ser
exercitado. O objetivo destes testes é **exploratório**: correr todas as
variações, observar o comportamento atual e decidir o que corrigir, ajustar,
criar ou remover.

> Já verificado — NÃO voltar a especificar aqui: a aritmética numérica exata
> (valores de resultado de ADD/SUB/MUL/DIV/COMPUTE, ROUNDED, ON SIZE ERROR), as
> PICTURE numéricas editadas + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, toda a
> E/S de arquivos (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, chaves,
> START/REWRITE/DELETE/INVALID KEY, STORAGE MODE MEMORY/DISK, compressão,
> persistência em MEMORY), programas aninhados e CALL básica, comparação
> alfanumérica, lexer fixo/livre. (As permutações de *sintaxe* aritmética abaixo
> continuam dentro do escopo — o que está «feito» é apenas a matemática dos
> valores.)

## Notação

- `[ x ]` opcional, `{ a | b }` escolha, `…` repetição, `dn` = item de dados n.
- **Eixo de mistura de tipos (T):** cada posição de operando tem de ser
  exercitada com estas espécies de recetor e de emissor, em ambos os sentidos
  quando aplicável:
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Valores-limite por espécie:** vazio, mínimo, máximo, transbordo por um, tudo
  espaços, tudo zeros, sinal em LEADING/TRAILING [SEPARATE], escalado com P,
  ponto implícito com V.
- De cada verbo deve captar-se: o valor ou valores resultantes, **FILE STATUS e
  os registros especiais** (`RETURN-CODE`, `TALLY`), o ramo de
  transbordo/exceção tomado, e que nada muda em caso de erro.

---

## Parte A — Secções da DATA DIVISION (comportamentos por testar)

### WORKING-STORAGE SECTION
- **Níveis:** 01, aninhamento 02–49, 77 (independente), 66 `RENAMES a THRU b`,
  88.
- **PIC:** `X A 9 S V P` com `(n)`; escalonamento com `P` (esquerda/direita);
  ponto implícito com `V`; combinações editadas; grupo com `PIC` versus grupo sem
  `PIC`.
- **USAGE:** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2, COMP‑3/PACKED‑DECIMAL,
  COMP‑5, INDEX, POINTER — declaração, tamanho de armazenamento e ida e volta do
  valor.
- **VALUE:** numérico, com sinal, alfanumérico, figurativo, `ALL "x"`; VALUE num
  grupo; VALUE ilegal (tamanho > PIC).
- **OCCURS:** fixo; `DEPENDING ON`; `INDEXED BY`; `ASCENDING/DESCENDING KEY`;
  várias dimensões (2–3); OCCURS sobre um grupo.
- **Cláusulas:** REDEFINES (igual, menor, maior, encadeado), RENAMES, JUSTIFIED
  RIGHT, BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`, SYNCHRONIZED,
  FILLER.
- **Nomes de condição 88:** valor único, lista de valores, `VALUE a THRU b`,
  vários intervalos, sobre host numérico / alfanumérico / editado; avaliação
  e `SET … TO TRUE`.
- **Inicialização:** por padrão (espaços/zeros conforme a classe) versus VALUE;
  **persistência através de PERFORM e através de CALL** (a WS guarda o último
  valor).

### LOCAL-STORAGE SECTION
- **Reinicializada a cada entrada no programa** (por contraste com a persistência
  da WS).
- As cláusulas VALUE são **reaplicadas a cada entrada**.
- **Recursão:** cada CALL (recursiva) obtém uma instância independente de
  LOCAL-STORAGE.
- A mesma cobertura de cláusulas que a WS (OCCURS/REDEFINES/88/…), mas
  verificando a semântica de reinicialização.

### LINKAGE SECTION
- Os itens **não têm armazenamento enquanto não forem ligados** pelo chamador;
  acessar a uma ligação não ligada.
- Ligados via `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (o chamador vê as alterações) versus **BY CONTENT** (o
  chamado edita uma cópia) versus **BY VALUE** (escalar).
- Grupo e elementar, OCCURS, REDEFINES, 88 na secção de ligação.
- Divergência de tamanho ou USAGE entre o parâmetro efetivo e o formal
  (comportamento a observar).
- `ADDRESS OF` / `SET ADDRESS OF … TO` e ligação de POINTER (se suportado).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — ligação posicional aos argumentos da
  CALL; divergência no número (menos ou mais argumentos); ordem.
- `BY REFERENCE | BY VALUE` por parâmetro na lista USING.
- `RETURNING dn` — valor devolvido a `CALL … RETURNING`; versus `GIVING`; versus
  `RETURN-CODE`.
- O `USING` do programa principal ligado a partir da linha de comando (se
  suportado).
- Mistura de tipos em cada posição de parâmetro (aplicar **T**).

---

## Parte B — Matriz de permutações de verbos

Exercite cada verbo ao longo de **T** para cada posição de operando. O que se
segue lista as permutações *estruturais* (cláusulas e frases) que acrescem à
mistura de tipos.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (vários recetores).
- `MOVE CORRESPONDING g1 TO g2` (elementares que coincidem pelo nome).
- Origem e destino com modificação de referência: `MOVE a(s:l) TO b(s:l)`.
- Com índices: `MOVE t(i) TO u(j)`, `t(i,j)`.
- Conversões de tipo (aplicar **T** nos dois sentidos): num→editado,
  editado→num, alfanum→num, num→alfanum (justificar/preencher/truncar),
  grupo→grupo (cópia de bytes), tratamento do sinal, COMP‑3↔DISPLAY,
  flutuante↔fixo, figurativo→cada espécie.

### DISPLAY
- `DISPLAY {dn|literal} …` (operandos concatenados).
- `[WITH NO ADVANCING]`; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Forma de tela (observar e decidir): `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Mistura de tipos: numérico (largura completa da PIC), editado, com sinal,
  grupo, figurativo.

### ACCEPT  *(especificar todas as formas; muitas são de tela ou terminal — assinalar para decisão de escopo)*
- `ACCEPT dn` (da consola para alfanum / numérico / editado / grupo).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Formas de tela: `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Recepção em numérico versus numérico editado versus alfanumérico (des-edição e
  validação).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Vários recetores, cada um com o seu próprio comportamento de ROUNDED e de
  tamanho; operandos de USAGE misturada (COMP‑3 + DISPLAY + editado); com sinal;
  operandos com modificação de referência.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- Divisão por zero → ON SIZE ERROR; sinal e escala de REMAINDER; USAGE
  misturada.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Operadores `+ - * / **`, parênteses, precedência; funções intrínsecas dentro da
  expressão; operandos de USAGE misturada; vários recetores; truncatura versus
  ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — aninhamento, ramos vazios, `NEXT SENTENCE`.
- Condições: de relação (`= < > <= >= NOT`), de classe
  (`IS [NOT] {NUMERIC|ALPHABETIC|ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), de sinal
  (`POSITIVE|NEGATIVE|ZERO`), referência a uma condição 88, combinadas
  (`AND/OR/NOT`), **abreviadas** (`a = b OR c`), entre parênteses.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` com
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Mistura de tipos nas comparações (num versus alfanum versus editado versus
  figurativo).

### PERFORM
- Fora de linha: `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = literal ou item de dados).
- `PERFORM … UNTIL cond` com `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- Em linha: `PERFORM … END-PERFORM` (com TIMES/UNTIL/VARYING).
- PERFORM aninhada e recursiva; sobreposição de intervalos; índice versus
  variável de ciclo numérica.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p`; `GO TO p1 p2 … DEPENDING ON dn` (dentro e fora do intervalo).
- `CONTINUE`; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (a partir do principal e de um
  subprograma).

### SET
- `SET index TO {n|index}`; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}`; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (onde for suportado).

### INITIALIZE
- `INITIALIZE dn …` (grupo ou elementar; por padrão conforme a categoria).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]`; tabelas (todas as ocorrências).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (série).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH`
  (binária; exige `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Encontrado e não encontrado; vários WHEN; mistura de tipos de chave;
  comportamento com a tabela por ordenar.

### STRING  *(exercitar o estilo de permutação do usuário)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutações a cobrir:
  - uma única origem com `DELIMITED BY SIZE` → destino alfanumérico.
  - várias origens, **delimitadores misturados**: `STRING "lit" DELIMITED BY SIZE
    d1 DELIMITED BY SPACES INTO d3`.
  - muitas origens e delimitadores: `STRING "l1" DELIMITED BY SIZE "l2"
    DELIMITED BY SIZE d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` para começar e avançar; ponteiro fora do intervalo →
    transbordo.
  - destino pequeno demais → `ON OVERFLOW`; `NOT ON OVERFLOW`.
  - **origens com mistura de tipos:** numérico, numérico editado, com sinal,
    grupo, figurativo, com modificação de referência — observar como cada um é
    convertido em cadeia.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutações: um versus vários delimitadores, `ALL` (colapsar repetições),
  `OR`, captura com `DELIMITER IN`/`COUNT IN`, POINTER, TALLYING, mais campos do
  que dados (transbordo), destinos de tipos misturados (os recetores numéricos
  são des-editados).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combinado).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- Escopo de BEFORE/AFTER; correspondências sobrepostas; padrões de vários
  carateres; host com mistura de tipos.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Nome de programa estático (literal) versus dinâmico (nome de dado); não
  resolvido → ON EXCEPTION.
- Modos de passagem de argumentos (observar a visibilidade a partir do
  chamador); divergência no número ou tipo de argumentos.
- `RETURNING` versus `RETURN-CODE`; recursão; dados partilhados `EXTERNAL`.
  (✅ `CANCEL prog` implementado — reinicializa o armazenamento do programa; o
  `NOT ON EXCEPTION` corre numa CALL resolvida.)

### Registros especiais de aritmética e verbos diversos
- `ADD/SUBTRACT … GIVING` (supressão a zero) versus a acumulação do `TO`.
- `MOVE` e aritmética de e para `RETURN-CODE`, `TALLY`.
- ✅ `ALTER` (o GO TO herdado) — implementado (redireciona o `GO TO` do
  parágrafo).
- Ida e volta de `ACCEPT/DISPLAY` através de campos editados.

### Verbos de arquivo — *(apenas as lacunas que o conjunto de E/S de arquivos não cobre)*
- ✅ **Implementado e testado** (`test_file_locking`): `OPEN … SHARING WITH …
  [WITH LOCK]`, `READ … WITH [NO] LOCK`, `UNLOCK` (consultivo dentro da unidade
  de execução — ver a referência de sintaxe suportada).
- `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  com chaves modificadas por referência; vários FD a partilhar uma área de
  registro.

### Verbos especificados aqui antes de existirem

> **Todos estes estão implementados.** SORT/MERGE/RELEASE/RETURN chegaram na
> 1.62.119 e o motor RELATIVE na 1.62.76
> (`crates/cobolt-runtime/src/relative.rs`); o
> `docs/cobol85-supported-syntax-pt.md` marca-os com ✅. Os eixos de permutação
> abaixo mantêm-se como o plano de testes que sempre foram: descrevem o que falta
> *cobrir*, não o que falta construir.
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}`; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- Organização `RELATIVE`: `READ/WRITE/REWRITE/DELETE/START` por `RELATIVE KEY`.

---

## Parte C — Bancada de equivalência entre formas

Para um conjunto escolhido dos programas acima, verificar que a saída observável
é **idêntica** (texto de DISPLAY, FILE STATUS, RETURN-CODE, conteúdo dos
arquivos) nas três formas de execução do mesmo fonte:

1. **Interpretador** (`Interpreter::run`).
2. **Ida e volta da AST** — serializar (`bincode`+`flate2`) → desserializar →
   executar; verificar que a AST é idêntica byte a byte e que a saída coincide.
3. **Binário empacotado/compilado** — `cobolt_compiler::build_project` →
   executar o binário produzido; verificar que a saída é idêntica.

Qualquer divergência entre formas é um defeito a registrar (o invariante «um
compilador, um comportamento»).

.<<

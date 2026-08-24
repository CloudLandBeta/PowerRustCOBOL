<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Benchmarks

A linha de base da 1.37.0: o quão rápido o runtime é sob carga, e o quanto ele
se apoia no alocador para chegar lá.

```sh
cargo run --release -p cobolt-bench              # everything
cargo run --release -p cobolt-bench -- dispatch  # one workload, by substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # a twentieth, for a quick check
```

O `--release` não é opcional. Uma compilação debug mede a *ausência* de
otimização, e o harness diz isso no seu cabeçalho em vez de deixar que os
números sejam citados.

## O que é medido

Toda carga de trabalho COBOL percorre o **mesmo caminho que um binário
entregue** percorre — tokenizar, fazer o parse, analisar, `Interpreter::run` —
porque é isso que o `main.rs` gerado pelo `rcrun build` faz com a sua AST
embutida. Executar dentro do próprio processo é o que torna possíveis os
contadores do alocador: os números descrevem o interpretador que está dentro de
cada binário que você entrega.

A memória é reportada como comportamento de alocação, e não como uma curva de
resident set. O Rust não tem coletor de lixo, então não há pausas a medir; o que
importa sob carga é a **rotatividade** — quantas vezes uma carga de trabalho
entra no alocador, quantos bytes passam por ele e quanto está vivo no pico. Um
alocador global com contadores
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) fornece os
três exatamente, nas três plataformas, sem nenhum profiler externo.

Duas coisas que isto deliberadamente **não** mede: o tempo de inicialização do
processo e o tamanho do binário. Meça essas duas no artefato real produzido pelo
`rcrun build`.

## A linha de base da 1.37.0

Apple M3 Pro, 18 GB, macOS 15.5, rustc 1.95.0, perfil release, 2026-07-27.
Números absolutos viajam mal entre máquinas; **alocações por operação** viaja
bem e é a coluna a observar.

| Carga de trabalho | Ops | Tempo | Ops/s | Alocações | Aloc./op | MB rotacionados | Pico vivo (MB) |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1,049s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0,729s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 compute | 0,824s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 rows, write+read) | 400 000 record | 2,179s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (create/read/destroy) | 20 000 object | 0,092s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (bulk insert) | 100 000 record | 0,710s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (random read) | 50 000 read | 0,034s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## O que a linha de base diz

**O gargalo é o alocador, não a caminhada na árvore.** 5,7 M de instruções por
segundo é uma taxa de despacho respeitável — mas chegar lá custou **24 milhões
de alocações para 6 milhões de instruções**. Um `ADD 1 TO ACC` sobre dois campos
`COMP`, que não deveria tocar o heap de forma alguma, custa quatro idas ao
alocador. Isso reposiciona o trabalho de otimização: os primeiros ganhos estão
no sistema de valores e no caminho dos operandos, não em substituir o
interpretador que caminha na árvore por uma VM de bytecode. Uma VM tornaria o
despacho mais barato e deixaria intactas as quatro alocações por instrução.

**Chamadas de parágrafo são caras fora de proporção.** 18 alocações e cerca de
820 bytes por `PERFORM <paragraph>`, contra 4 por instrução inline. Meio milhão
de chamadas rotacionam 410 MB. Seja lá o que o caminho de chamada esteja
construindo a cada invocação, é o alvo de maior densidade da tabela.

**Registros alfanuméricos alocam por campo, como esperado.** 65 alocações por
registro para uma linha de 4 campos lida e escrita é o `CobolValue::String`
sendo dono de um `Vec<u8>` por campo, mais um novo a cada `MOVE`. Uma
representação inline para strings curtas, ou fatiar dentro do próprio buffer do
registro, apareceria aqui imediatamente.

**Leituras de propriedade de objeto alocam sem motivo.** 55 alocações por objeto
ao longo de 24 leituras de propriedade. `CoboltObject::get_property`, `get_str`,
`get_bool` e `get_i64` chamam cada um `name.to_ascii_uppercase()` — uma `String`
alocada e descartada **por leitura**, puramente para tornar a busca insensível a
maiúsculas. Um invólucro de chave insensível a maiúsculas elimina a coluna
inteira.

**O motor INDEXED não é o problema.** O redb insere a 141 mil registros por
segundo com 0,66 alocação por registro e serve 1,5 M de leituras aleatórias por
segundo com alocação essencialmente nula. O armazenamento está confortavelmente
à frente do interpretador que o alimenta.

Ordenada pelo retorno esperado, a ordem de otimização que a linha de base sugere
é: as alocações por instrução, depois o caminho de chamada de parágrafo, depois
o `CobolValue` para alfanuméricos, depois o *upper-casing* das propriedades de
objeto. O armazenamento só aparece bem abaixo dessas.

## Cargas de trabalho

| Carga de trabalho | O que ela isola |
|---|---|
| `dispatch (PERFORM VARYING)` | Custo da caminhada na árvore: teste do laço, incremento, uma instrução, trabalho mínimo por baixo |
| `dispatch (PERFORM paragraph)` | Custo da chamada de parágrafo, contra o caso inline acima |
| `decimal COMPUTE` | A aritmética escalada em i128 do `CobolNumeric` — a matemática monetária do COBOL |
| `record batch` | Tabela de 1000 linhas escrita e lida de volta com campos alfanuméricos; o sistema de valores sob carga em lote |
| `object churn` | Criar/ler/destruir do `ObjectRegistry` — o que custa um formulário com muitos controles |
| `indexed redb` | O motor de arquivos INDEXED: inserção em massa, depois leituras por chave aleatória |

As duas linhas de `indexed redb` são uma versão recuperada e generalizada do
micro-benchmark `open_table_cost`, que vivia marcado com `#[ignore]` dentro de
`cobolt-runtime::indexed_redb`. Ele só rodava quando alguém lembrava de uma
invocação exata com `--ignored`, de modo que o motor não tinha linha de base
permanente; agora tem. A sua conclusão original está preservada — o handle da
tabela é aberto uma vez para toda a transação de escrita, o que se mediu como
cerca de 16 % mais rápido do que abri-lo duas vezes por inserção.

## Acrescentando uma carga de trabalho

Acrescente uma função `bench_*` em
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) que
retorne `measure(name, unit, || { ...; ops_performed })`, e registre-a em `main`
atrás de um filtro `wanted(...)`. Os contadores envolvem a closure
automaticamente. Retorne o número de unidades de *trabalho*, não de iterações,
para que `ops/sec` e `allocs/op` continuem comparáveis entre cargas.

Mantenha novas cargas determinísticas. A sonda de leitura aleatória usa um passo
multiplicativo fixo em vez de um gerador de números aleatórios exatamente por
esse motivo: um benchmark que se reembaralha entre execuções não pode ser
comparado com o número de ontem.

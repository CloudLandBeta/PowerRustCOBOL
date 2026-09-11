<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Benchmarks

A linha de base da 1.37.0: a que velocidade o runtime opera sob carga e com que
intensidade se apoia no alocador para chegar lá.

```sh
cargo run --release -p cobolt-bench              # tudo
cargo run --release -p cobolt-bench -- dispatch  # uma carga, por substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # um vinte avos, para uma verificação rápida
```

`--release` não é opcional. Uma compilação de depuração mede a ausência de
otimização, e o arcabouço diz isso no seu cabeçalho em vez de deixar que os
números sejam citados.

## O que é medido

Cada carga COBOL percorre **o mesmo caminho que um binário entregue percorre** —
tokenizar, analisar sintaticamente, analisar semanticamente, `Interpreter::run`
— porque é isso que o `main.rs` gerado pelo `rcrun build` faz com a sua AST
embutida. Executar no mesmo processo é o que torna possíveis os contadores do
alocador: os números descrevem o interpretador que está dentro de cada binário
que você entrega.

A memória é reportada como comportamento de alocação, não como uma curva de
conjunto residente. Rust não tem coletor de lixo, portanto não há pausas a
medir; o que importa sob carga é a **rotatividade** — quantas vezes uma carga
entra no alocador, quantos bytes passam por ele e quanto está vivo no pico. Um
alocador global contador
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) fornece as
três medidas com exatidão, nas três plataformas, sem nenhum profiler externo.

Duas coisas que isto deliberadamente **não** mede: a inicialização do processo e
o tamanho do binário. Meça essas no artefato real do `rcrun build`.

## A linha de base da 1.37.0

Apple M3 Pro, 18 GB, macOS 15.5, rustc 1.95.0, perfil release, 2026-07-27.
Números absolutos viajam mal entre máquinas; **alocações por operação** viaja bem
e é a coluna a observar.

| Carga | Ops | Tempo | Ops/s | Aloc. | Aloc./op | MB rotacionados | Pico vivo MB |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 sent. | 1,049 s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 cham. | 0,729 s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 cálculos | 0,824 s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 linhas, escrita+leitura) | 400 000 registros | 2,179 s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (criar/ler/destruir) | 20 000 objetos | 0,092 s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (inserção em massa) | 100 000 registros | 0,710 s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (leitura aleatória) | 50 000 leituras | 0,034 s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## O que a linha de base diz

**O gargalo é o alocador, não o percurso da árvore.** 5,7 M de sentenças por
segundo é uma taxa de despacho respeitável — mas chegar lá custou **24 milhões
de alocações para 6 milhões de sentenças**. `ADD 1 TO ACC` sobre dois campos
`COMP`, que não deveria tocar o heap de forma alguma, custa quatro viagens pelo
alocador. Isso reenquadra o trabalho de otimização: as primeiras vitórias estão
no sistema de valores e no caminho dos operandos, não em substituir o
interpretador de árvore por uma máquina virtual de bytecode. Uma VM tornaria o
despacho mais barato deixando intactas as quatro alocações por sentença.

**Chamadas de parágrafo são caras de forma desproporcional.** 18 alocações e
cerca de 820 bytes por `PERFORM <paragraph>`, contra 4 por sentença em linha.
Meio milhão de chamadas rotacionam 410 MB. O que quer que o caminho de chamada
construa a cada invocação é o alvo de maior densidade da tabela.

**Registros alfanuméricos alocam por campo, como esperado.** 65 alocações por
registro para uma linha de 4 campos lida e escrita é `CobolValue::String`
possuindo um `Vec<u8>` por campo, mais um novo a cada `MOVE`. Uma representação
de string curta em linha, ou fatiar dentro do próprio buffer do registro,
apareceria aqui imediatamente.

**Leituras de propriedades de objeto alocam sem motivo.** 55 alocações por objeto
ao longo de 24 leituras de propriedade. `CoboltObject::get_property`, `get_str`,
`get_bool` e `get_i64` chamam cada um `name.to_ascii_uppercase()` — uma `String`
alocada e descartada **por leitura**, apenas para tornar a busca insensível a
maiúsculas. Um invólucro de chave insensível a maiúsculas remove a coluna
inteira.

**O motor INDEXED não é o problema.** O redb insere a 141 k registros/s com 0,66
alocações por registro e serve 1,5 M de leituras aleatórias por segundo
praticamente sem alocar. O armazenamento está confortavelmente à frente do
interpretador que o alimenta.

Ordenada pelo retorno esperado, a ordem de otimização que a linha de base sugere
é: as alocações por sentença, depois o caminho de chamada de parágrafo, depois
`CobolValue` para alfanuméricos, e depois a conversão para maiúsculas nas
propriedades de objeto. O armazenamento só aparece bem abaixo disso.

## Cargas de trabalho

| Carga | O que isola |
|---|---|
| `dispatch (PERFORM VARYING)` | Custo do percurso da árvore: teste do laço, incremento, uma sentença, trabalho mínimo por baixo |
| `dispatch (PERFORM paragraph)` | Custo da chamada de parágrafo, contra o caso em linha acima |
| `decimal COMPUTE` | A aritmética escalada em i128 de `CobolNumeric` — matemática monetária COBOL |
| `record batch` | Tabela de 1000 linhas escrita e relida com campos alfanuméricos; o sistema de valores sob carga em lote |
| `object churn` | `ObjectRegistry` criar/ler/destruir — o que custa um formulário com muitos controles |
| `indexed redb` | O motor de arquivos INDEXED: inserção em massa e depois leituras por chave aleatória |

As duas linhas de `indexed redb` são uma versão recuperada e generalizada do
micro-benchmark `open_table_cost`, que continua marcado `#[ignore]` dentro de
`cobolt-runtime::indexed_redb` (`indexed_redb.rs:1264`). Ele só roda quando
alguém lembra de uma invocação `--ignored` exata, de modo que o motor não tinha
uma linha de base permanente; agora tem uma aqui, e o original fica onde está.
A sua conclusão original é mantida: o manipulador da tabela é aberto uma única
vez para toda a transação de escrita, o que mediu ~16 % mais rápido do que
abri-lo duas vezes por inserção.

## Adicionar uma carga de trabalho

Adicione uma função `bench_*` a
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) que
retorne `measure(name, unit, || { ...; ops_performed })`, e registre-a em `main`
atrás de um filtro `wanted(...)`. Os contadores envolvem o closure
automaticamente. Retorne o número de unidades de *trabalho*, não de iterações,
para que `ops/sec` e `allocs/op` continuem comparáveis entre cargas.

Mantenha as cargas novas determinísticas. A sonda de leitura aleatória usa um
passo multiplicativo fixo em vez de um gerador de números aleatórios exatamente
por isso: um benchmark que se reembaralha entre execuções não pode ser comparado
com o número de ontem.

.<<

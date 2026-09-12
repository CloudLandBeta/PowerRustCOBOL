<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# Inventário de crates

Todas as crates de que o PowerRustCOBOL depende **diretamente**, com a versão
que é realmente ligada (não a cadeia do requisito, mas a resolvida no
`Cargo.lock`).

Gerado pela primeira vez com `cargo metadata` em **2026-07-27**, na versão de
produto **1.37.0**; as versões resolvidas abaixo foram reconciliadas pela última
vez com o `Cargo.lock` em **2026-09-11**, na **1.65.128**. Repare nos dois
esquemas de numeração: a versão de *produto* é a que está em
`crates/cobolt-ide/src/version.rs` e é mostrada no IDE; a versão de *crate* no
`Cargo.toml` é `0.2.0` e é partilhada por todas as crates do espaço de trabalho.
Regenere a coluna das versões com:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

O grafo completo de dependências são **944 pacotes**. As tabelas seguintes são
as **59** que o espaço de trabalho nomeia por si; tudo o resto chega
transitivamente através delas.

---

## Crates do espaço de trabalho

As 17 crates que *são* o PowerRustCOBOL — a autoridade aqui é
`cargo metadata --no-deps`, não um grep do `Cargo.toml`, onde dois membros
partilham uma linha e uma contagem ingénua devolve 16. Todas partilham a versão
de crate `0.2.0` do espaço de trabalho (ver a nota acima — a versão de produto
segue a sua própria sequência).

| Crate | Versão da crate | Camada | O que faz |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Tokenizador de COBOL Fujitsu — fonte em formato fixo e livre — e o pré-processador `COPY`/`REPLACE` |
| `cobolt-parser` | 0.2.0 | front end | Analisador descendente recursivo: fluxo de tokens → AST |
| `cobolt-ast` | 0.2.0 | front end | Tipos de nó da AST |
| `cobolt-semantic` | 0.2.0 | front end | Resolução de nomes, verificação de tipos, ligação de `EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | execução | Interpretador que percorre a árvore, sistema de valores, executor de `EXEC RUST`, ambientes de BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | execução | Funções intrínsecas, backend de E/S, utilitários de consola |
| `cobolt-indexed` | 0.2.0 | execução | Modelo de definição de arquivos indexados (`.cidx`) |
| `cobolt-forms` | 0.2.0 | motor de UI | Modelo de formulários/controles (`.cfrm`), o motor de renderização unificado, temas, animação |
| `cobolt-form-host` | 0.2.0 | motor de UI | O único host de formulários (spec 042) — partilhado pelo `rcrun run-form` e pelas aplicações compiladas |
| `cobolt-media` | 0.2.0 | motor de UI | Descodificação e reprodução de imagens animadas (GIF/WebP/APNG) para o widget Animator |
| `cobolt-codegen` | 0.2.0 | ferramentas | Gerador de fonte COBOL a partir de formulários |
| `cobolt-compiler` | 0.2.0 | ferramentas | Compilador de incorporação e empacotamento: projeto → um executável nativo |
| `cobolt-dap` | 0.2.0 | ferramentas | Protocolo Debug Adapter compatível ao nível do protocolo — enquadramento, tipos, cliente e servidor adaptador |
| `cobolt-agents` | 0.2.0 | IA | Malha de agentes, índice da base de conhecimento, embeddings, recuperação |
| `cobolt-cli` | 0.2.0 | binário | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binário | O próprio IDE |
| `cobolt-bench` | 0.2.0 | binário | Banco de referência de desempenho e alocações (ver [BENCHMARKS-pt.md](BENCHMARKS-pt.md)) |

---

## Dependências externas

`Used by` nomeia crates do espaço de trabalho sem o prefixo `cobolt-`.

### UI e renderização

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | Kit de interface em modo imediato — toda a UI |
| `eframe` | 0.36.0 | cli, ide | Host de janela e ciclo de eventos para o egui |
| `egui_extras` | 0.36.0 | cli, ide | Tabelas, carregadores de imagens, widgets adicionais |
| `egui_commonmark` | 0.25.0 | ide | Renderização de Markdown nos painéis de documentação e de chat |
| `egui_inspection` | 0.36.0 | ide | Inspetor de widgets e disposição em tempo real |
| `image` | 0.25.10 | cli, forms, ide, media | Descodificação PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rasterização de SVG |
| `fontdb` | 0.23.0 | forms, ide | Enumeração dos tipos de letra do sistema |
| `skrifa` | 0.42.1 | forms | Validação de tipos de letra com o mesmo analisador que o epaint usa |
| `rfd` | 0.14.1 | ide | Diálogos nativos de Abrir/Salvar |
| `syntect` | 5.3.0 | ide | Realce de sintaxe no editor |
| `pulldown-cmark` | 0.12.2 | ide | Análise de Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Renderização de diagramas mermaid |
| `genpdf` | 0.2.0 | ide | Exportação para PDF |
| `pollster` | 0.3.0 | ide | Bloqueia nas poucas chamadas assíncronas que o IDE faz |

### Front end da linguagem

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Gerador de analisadores léxicos |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Mapas que preservam a ordem de inserção — em COBOL a ordem de declaração é semântica |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Tipos de erro |

### Dados, armazenamento e E/S

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Armazém ACID embutido em Rust puro — arquivos INDEXED e o índice da base de conhecimento |
| `rusqlite` | 0.32.1 | runtime | SQLite para o ambiente de bancos de dados COBOL (incorporado; compila C) |
| `postgres` | 0.19.13 | runtime | Controlador de PostgreSQL (Rust puro, síncrono) |
| `mysql` | 28.0.0 | runtime | Controlador de MySQL (Rust puro, conjunto de funcionalidades `minimal-rust` — **sem TLS**; ver [database-runtime-pt.md](database-runtime-pt.md)) |
| `ureq` | 2.12.1 | runtime | Cliente HTTP bloqueante para o ambiente REST do COBOL |
| `native-tls` | 0.2.18 | runtime | TLS através da pilha do sistema operacional — sem criptografia incorporada para compilar |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Cliente HTTP para as chamadas a modelos e à web |
| `quick-xml` | 0.36.2 | forms, indexed | Serialização de `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Infraestrutura de serialização |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (descontinuado a montante; fixado) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifestos de temas |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Codificação binária compacta da AST compilada |
| `flate2` | 1.1.9 | compiler | Deflate — comprime a AST incorporada |
| `zip` | 2.4.2 | cli, ide | Importação/exportação de arquivos de projeto |
| `include_dir` | 0.7.4 | ide | Cozinha a documentação incluída dentro do binário |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Arquivos temporários (também dependência de desenvolvimento) |
| `dirs` | 5.0.1 | ide | Diretórios de configuração e dados por plataforma |

### IA e recuperação

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orquestração de agentes/LLM (native-tls, não rustls) |
| `candle-core` | 0.11.0 | agents | Ambiente de tensores em Rust puro |
| `candle-nn` | 0.11.0 | agents | Camadas de redes neuronais para o Candle |
| `candle-transformers` | 0.11.0 | agents | BERT e companhia — executa o `all-MiniLM-L6-v2` dentro do processo |
| `tokenizers` | 0.23.1 | agents | Tokenizador da HuggingFace (`esaxx_fast` desligado, `onig` ligado) |
| `schemars` | 1.2.1 | agents, ide | JSON Schema para as definições de ferramentas |
| `tokio` | 1.52.3 | agents, ide | Ambiente assíncrono para a camada de agentes |
| `futures` | 0.3.32 | agents | Combinadores assíncronos |

### Transversais

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Registro estruturado |
| `tracing-subscriber` | 0.3.23 | cli, ide | Filtragem e formatação de registros |
| `sysinfo` | 0.31.4 | ide | Estatísticas de processo e memória |
| `num_cpus` | 1.17.0 | agents | Dimensionamento do paralelismo |
| `rand` | 0.8.6 | ide | Valores aleatórios |
| `hmac` | 0.12.1 | forms | HMAC para a assinatura de ligação |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Diferenças legíveis nos testes (dependência de desenvolvimento) |

---

## Funcionalidades opcionais

A `cobolt-agents` declara exatamente uma funcionalidade opcional, e ela está
**desligada** numa compilação por padrão:

| Funcionalidade | O que traz | Porque é opcional |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | Embeddings em GPU NVIDIA no Linux e no Windows. Compilá-la exige o toolkit CUDA, e é por isso que é opcional; sem ela o gerador de embeddings corre na CPU |

> **Removido na 1.41.4.** Esta secção listava antes `tantivy`, `sqlite-vec`,
> `rig-sqlite`, `tokio-rusqlite`, `ort`, `ndarray` e `opentelemetry-otlp` por
> trás de `local-retrieval` e `otel`. Nenhuma dessas duas funcionalidades existe
> já e nenhuma dessas crates está declarada em parte alguma do espaço de
> trabalho — a recuperação é servida pelo armazém interno em
> `cobolt-agents/src/knowledge_store.rs`, que guarda os embeddings como
> `Vec<f32>` e os compara com um simples produto escalar.

---

## As duas crates que compilam C

Vale a pena saber ao preparar uma máquina (ver [BUILDING-pt.md](BUILDING-pt.md)):

| Crate | Alcançada por | O que compila |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (em `cobolt-runtime`) | A amálgama em C do SQLite, incorporada para que nenhum SQLite do sistema tenha de coincidir |
| `onig_sys` | `tokenizers` → `onig` | O motor de expressões regulares Oniguruma |

Nada na árvore compila **C++**, e nenhum script de compilação invoca CMake,
NASM, Python, Node ou uma JVM.

.<<

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Inventário de crates

Todo crate do qual o PowerRustCOBOL depende **diretamente**, com a versão
realmente ligada (não a string do requisito — a versão resolvida, vinda do
`Cargo.lock`).

Gerado a partir do `cargo metadata` em **2026-07-27**, na versão de produto
**1.37.0**. Repare nos dois esquemas de numeração: a versão do *produto* é a que
está em `crates/cobolt-ide/src/version.rs` e é exibida na IDE; a versão do
*crate* no `Cargo.toml` é `0.2.0` e é compartilhada por todos os crates do
workspace. Para regerar a coluna de versões:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

O grafo completo de dependências tem **906 pacotes**. As tabelas abaixo são os
~56 que o próprio workspace nomeia; todo o resto chega transitivamente através
deles.

---

## Crates do workspace

Os 14 crates que *são* o PowerRustCOBOL. Todos compartilham a versão de crate do
workspace, `0.2.0` (veja a nota acima — a versão de produto é 1.37.0).

| Crate | Versão do crate | Camada | O que faz |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Tokenizador COBOL Fujitsu — código em forma fixa e em forma livre |
| `cobolt-parser` | 0.2.0 | front end | Parser descendente recursivo: fluxo de tokens → AST |
| `cobolt-ast` | 0.2.0 | front end | Tipos de nós da AST |
| `cobolt-semantic` | 0.2.0 | front end | Resolução de nomes, verificação de tipos, ligação de `EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | execução | Interpretador que caminha na árvore, sistema de valores, executor de `EXEC RUST`, runtimes de BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | execução | Funções intrínsecas, backend de E/S, utilitários de console |
| `cobolt-indexed` | 0.2.0 | execução | Modelo de definição de arquivos indexados (`.cidx`) |
| `cobolt-forms` | 0.2.0 | motor de UI | Modelo de formulário/controle (`.cfrm`), o motor de renderização unificado, temas, animação |
| `cobolt-media` | 0.2.0 | motor de UI | Decodificação e reprodução de imagens animadas (GIF/WebP/APNG) para o widget Animator |
| `cobolt-codegen` | 0.2.0 | ferramental | Gerador de código-fonte COBOL a partir de formulários |
| `cobolt-compiler` | 0.2.0 | ferramental | Compilador que embute e empacota: projeto → um executável nativo |
| `cobolt-agents` | 0.2.0 | IA | Malha de agentes, índice da Base de Conhecimento, embeddings, recuperação |
| `cobolt-cli` | 0.2.0 | binário | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binário | A própria IDE |

---

## Dependências externas

A coluna `Usado por` nomeia crates do workspace com o prefixo `cobolt-` omitido.

### UI e renderização

| Crate | Versão | Usado por | O que faz |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | Biblioteca de GUI em modo imediato — toda a interface |
| `eframe` | 0.35.0 | cli, ide | Hospeda a janela e o laço de eventos do egui |
| `egui_extras` | 0.35.0 | cli, ide | Tabelas, carregadores de imagem, widgets extras |
| `egui_glow` | 0.35.0 | ide | Pintor OpenGL — o gancho de recorte de cantos arredondados precisa dele |
| `egui_commonmark` | 0.24.0 | ide | Renderização de Markdown nos painéis de documentação e chat |
| `egui_inspection` | 0.35.0 | ide | Inspetor de widgets/layout ao vivo |
| `image` | 0.25.10 | cli, forms, ide, media | Decodificação de PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rasterização de SVG |
| `fontdb` | 0.23.0 | forms, ide | Enumeração das fontes do sistema |
| `skrifa` | 0.42.1 | forms | Validação de fontes com o mesmo parser que o próprio epaint usa |
| `rfd` | 0.14.1 | ide | Caixas de diálogo nativas de abrir/salvar |
| `syntect` | 5.3.0 | ide | Realce de sintaxe no editor |
| `pulldown-cmark` | 0.12.2 | ide | Parsing de Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Renderização de diagramas mermaid |
| `genpdf` | 0.2.0 | ide | Exportação para PDF |
| `pollster` | 0.3.0 | ide | Bloqueia nas poucas chamadas assíncronas que a IDE faz |

### Front end de linguagem

| Crate | Versão | Usado por | O que faz |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Gerador de analisador léxico |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Mapas que preservam a ordem de inserção — no COBOL a ordem de declaração é semântica |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Tipos de erro |

### Dados, armazenamento e E/S

| Crate | Versão | Usado por | O que faz |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Armazenamento ACID embutido, puro Rust — arquivos INDEXED e o índice da Base de Conhecimento |
| `rusqlite` | 0.32.1 | runtime | SQLite para o runtime de banco de dados COBOL (embutido; compila C) |
| `postgres` | 0.19.13 | runtime | Driver PostgreSQL (puro Rust, síncrono) |
| `mysql` | 28.0.0 | runtime | Driver MySQL (puro Rust, conjunto de features rustls) |
| `ureq` | 2.12.1 | runtime | Cliente HTTP bloqueante para o runtime REST do COBOL |
| `native-tls` | 0.2.18 | runtime | TLS pela pilha do sistema operacional — nenhuma criptografia embutida para compilar |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Cliente HTTP para chamadas a modelos e à web |
| `quick-xml` | 0.36.2 | forms, indexed | Serialização de `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Framework de serialização |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (descontinuado a montante; versão fixada) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifestos de tema |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Codificação binária compacta da AST compilada |
| `flate2` | 1.1.9 | compiler | Deflate — comprime a AST embutida |
| `zip` | 2.4.2 | cli, ide | Importação/exportação de arquivos de projeto |
| `include_dir` | 0.7.4 | ide | Embute a documentação empacotada dentro do binário |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Arquivos temporários (também é dependência de desenvolvimento) |
| `dirs` | 5.0.1 | ide | Diretórios de configuração/dados por plataforma |

### IA e recuperação

| Crate | Versão | Usado por | O que faz |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orquestração de agentes/LLM (native-tls, não rustls) |
| `candle-core` | 0.11.0 | agents | Runtime de tensores puro Rust |
| `candle-nn` | 0.11.0 | agents | Camadas de rede neural para o Candle |
| `candle-transformers` | 0.11.0 | agents | BERT e afins — executa o `all-MiniLM-L6-v2` dentro do processo |
| `tokenizers` | 0.23.1 | agents | Tokenizador da HuggingFace (`esaxx_fast` desligado, `onig` ligado) |
| `embedvec` | 0.8.0 | agents | Armazenamento vetorial: quantização E8, similaridade de cosseno |
| `schemars` | 1.2.1 | agents, ide | JSON Schema para definições de ferramentas |
| `opentelemetry` | 0.32.0 | agents | API de tracing/métricas |
| `tokio` | 1.52.3 | agents, ide | Runtime assíncrono da camada de agentes |
| `futures` | 0.3.32 | agents | Combinadores assíncronos |

### Transversais

| Crate | Versão | Usado por | O que faz |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Registro estruturado |
| `tracing-subscriber` | 0.3.23 | cli, ide | Filtragem e formatação de logs |
| `sysinfo` | 0.31.4 | ide | Estatísticas de processo/memória |
| `num_cpus` | 1.17.0 | agents | Dimensionamento do paralelismo |
| `rand` | 0.8.6 | ide | Valores aleatórios |
| `hmac` | 0.12.1 | forms | HMAC para a assinatura de vínculo |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Diferenças legíveis nos testes (dependência de desenvolvimento) |

---

## Declarados, mas não ligados por padrão

Estes são nomeados em algum `Cargo.toml` atrás de uma feature que está
**desligada** numa compilação padrão, de modo que não contribuem em nada para o
tempo de compilação nem para o tamanho do binário, a menos que você ligue a
feature:

| Crate | Feature | Por que é opcional |
|---|---|---|
| `tantivy` | `local-retrieval` | Índice léxico — o caminho padrão é `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | Busca vetorial sobre SQLite; habilitá-la traz o SQLite embutido (e um toolchain C) para dentro do `cobolt-agents` |
| `ort`, `ndarray` | `local-retrieval` | Caminho de inferência do ONNX Runtime |
| `opentelemetry-otlp` | `otel` | Exportação OTLP |

---

## Os dois crates que compilam C

Vale saber na hora de preparar uma máquina (veja
[BUILDING-en.md](BUILDING-en.md)):

| Crate | Alcançado via | O que ele compila |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (em `cobolt-runtime`) | A amalgamação C do SQLite, embutida para que nenhum SQLite do sistema precise coincidir |
| `onig_sys` | `tokenizers` → `onig` | O motor de expressões regulares Oniguruma |

Nada na árvore compila **C++**, e nenhum script de build invoca CMake, NASM,
Python, Node ou uma JVM.

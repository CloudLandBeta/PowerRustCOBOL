<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# Inventario de crates

Todos los crates de los que PowerRustCOBOL depende **directamente**, con la
versión que realmente se enlaza (no la cadena del requisito, sino la resuelta en
`Cargo.lock`).

Generado por primera vez con `cargo metadata` el **2026-07-27**, en la versión
de producto **1.37.0**; las versiones resueltas de más abajo se reconciliaron por
última vez con `Cargo.lock` el **2026-09-11**, en la **1.65.128**. Repare en los
dos esquemas de numeración: la versión de *producto* es la que está en
`crates/cobolt-ide/src/version.rs` y se muestra en el IDE; la versión de *crate*
en `Cargo.toml` es `0.2.0` y la comparten todos los crates del espacio de
trabajo. Regenere la columna de versiones con:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

El grafo completo de dependencias son **944 paquetes**. Las tablas siguientes
son los **59** que el espacio de trabajo nombra por sí mismo; todo lo demás llega
transitivamente a través de ellos.

---

## Crates del espacio de trabajo

Los 17 crates que *son* PowerRustCOBOL — aquí la autoridad es
`cargo metadata --no-deps`, no un grep de `Cargo.toml`, donde dos miembros
comparten línea y un recuento ingenuo informa de 16. Todos comparten la versión
de crate `0.2.0` del espacio de trabajo (véase la nota de arriba: la versión de
producto va por su cuenta).

| Crate | Versión del crate | Capa | Qué hace |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Tokenizador de COBOL Fujitsu — fuente en formato fijo y libre — y el preprocesador `COPY`/`REPLACE` |
| `cobolt-parser` | 0.2.0 | front end | Analizador descendente recursivo: flujo de tokens → AST |
| `cobolt-ast` | 0.2.0 | front end | Tipos de nodo del AST |
| `cobolt-semantic` | 0.2.0 | front end | Resolución de nombres, comprobación de tipos, enlace de `EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | ejecución | Intérprete que recorre el árbol, sistema de valores, ejecutor de `EXEC RUST`, entornos de BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | ejecución | Funciones intrínsecas, backend de E/S, utilidades de consola |
| `cobolt-indexed` | 0.2.0 | ejecución | Modelo de definición de ficheros indexados (`.cidx`) |
| `cobolt-forms` | 0.2.0 | motor de UI | Modelo de formularios/controles (`.cfrm`), el motor de renderizado unificado, temas, animación |
| `cobolt-form-host` | 0.2.0 | motor de UI | El único anfitrión de formularios (spec 042) — compartido por `rcrun run-form` y las aplicaciones compiladas |
| `cobolt-media` | 0.2.0 | motor de UI | Decodificación y reproducción de imágenes animadas (GIF/WebP/APNG) para el widget Animator |
| `cobolt-codegen` | 0.2.0 | herramientas | Generador de fuente COBOL a partir de formularios |
| `cobolt-compiler` | 0.2.0 | herramientas | Compilador de incrustación y empaquetado: proyecto → un ejecutable nativo |
| `cobolt-dap` | 0.2.0 | herramientas | Protocolo Debug Adapter compatible a nivel de cable — tramado, tipos, cliente y servidor adaptador |
| `cobolt-agents` | 0.2.0 | IA | Malla de agentes, índice de la base de conocimiento, embeddings, recuperación |
| `cobolt-cli` | 0.2.0 | binario | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binario | El propio IDE |
| `cobolt-bench` | 0.2.0 | binario | Banco de pruebas de referencia de rendimiento y asignaciones (véase [BENCHMARKS-es.md](BENCHMARKS-es.md)) |

---

## Dependencias externas

`Used by` nombra crates del espacio de trabajo sin el prefijo `cobolt-`.

### UI y renderizado

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | Kit de interfaz en modo inmediato — toda la UI |
| `eframe` | 0.36.0 | cli, ide | Anfitrión de ventana y bucle de eventos para egui |
| `egui_extras` | 0.36.0 | cli, ide | Tablas, cargadores de imágenes, widgets adicionales |
| `egui_commonmark` | 0.25.0 | ide | Renderizado de Markdown en los paneles de documentación y chat |
| `egui_inspection` | 0.36.0 | ide | Inspector de widgets y disposición en vivo |
| `image` | 0.25.10 | cli, forms, ide, media | Decodificación PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rasterización de SVG |
| `fontdb` | 0.23.0 | forms, ide | Enumeración de fuentes del sistema |
| `skrifa` | 0.42.1 | forms | Validación de tipos de letra con el mismo analizador que usa epaint |
| `rfd` | 0.14.1 | ide | Diálogos nativos de Abrir/Guardar |
| `syntect` | 5.3.0 | ide | Resaltado de sintaxis en el editor |
| `pulldown-cmark` | 0.12.2 | ide | Análisis de Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Renderizado de diagramas mermaid |
| `genpdf` | 0.2.0 | ide | Exportación a PDF |
| `pollster` | 0.3.0 | ide | Bloquea en las pocas llamadas asíncronas que hace el IDE |

### Front end del lenguaje

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Generador de analizadores léxicos |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Mapas que conservan el orden de inserción — en COBOL el orden de declaración es semántico |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Tipos de error |

### Datos, almacenamiento y E/S

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Almacén ACID embebido en Rust puro — ficheros INDEXED y el índice de la base de conocimiento |
| `rusqlite` | 0.32.1 | runtime | SQLite para el entorno de bases de datos COBOL (incorporado; compila C) |
| `postgres` | 0.19.13 | runtime | Controlador de PostgreSQL (Rust puro, síncrono) |
| `mysql` | 28.0.0 | runtime | Controlador de MySQL (Rust puro, conjunto de características `minimal-rust` — **sin TLS**; véase [database-runtime-es.md](database-runtime-es.md)) |
| `ureq` | 2.12.1 | runtime | Cliente HTTP bloqueante para el entorno REST de COBOL |
| `native-tls` | 0.2.18 | runtime | TLS a través de la pila del sistema operativo — sin criptografía incorporada que compilar |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Cliente HTTP para las llamadas a modelos y a la web |
| `quick-xml` | 0.36.2 | forms, indexed | Serialización de `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Marco de serialización |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (obsoleto aguas arriba; fijado) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifiestos de temas |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Codificación binaria compacta del AST compilado |
| `flate2` | 1.1.9 | compiler | Deflate — comprime el AST incrustado |
| `zip` | 2.4.2 | cli, ide | Importación/exportación de archivos de proyecto |
| `include_dir` | 0.7.4 | ide | Incrusta la documentación incluida dentro del binario |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Ficheros temporales (también dependencia de desarrollo) |
| `dirs` | 5.0.1 | ide | Directorios de configuración y datos por plataforma |

### IA y recuperación

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orquestación de agentes/LLM (native-tls, no rustls) |
| `candle-core` | 0.11.0 | agents | Entorno de tensores en Rust puro |
| `candle-nn` | 0.11.0 | agents | Capas de redes neuronales para Candle |
| `candle-transformers` | 0.11.0 | agents | BERT y compañía — ejecuta `all-MiniLM-L6-v2` dentro del proceso |
| `tokenizers` | 0.23.1 | agents | Tokenizador de HuggingFace (`esaxx_fast` desactivado, `onig` activado) |
| `schemars` | 1.2.1 | agents, ide | JSON Schema para las definiciones de herramientas |
| `tokio` | 1.52.3 | agents, ide | Entorno asíncrono para la capa de agentes |
| `futures` | 0.3.32 | agents | Combinadores asíncronos |

### Transversales

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Registro estructurado |
| `tracing-subscriber` | 0.3.23 | cli, ide | Filtrado y formateo de registros |
| `sysinfo` | 0.31.4 | ide | Estadísticas de proceso y memoria |
| `num_cpus` | 1.17.0 | agents | Dimensionado del paralelismo |
| `rand` | 0.8.6 | ide | Valores aleatorios |
| `hmac` | 0.12.1 | forms | HMAC para la firma de enlace |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Diferencias legibles en las pruebas (dependencia de desarrollo) |

---

## Características opcionales

`cobolt-agents` declara exactamente una característica opcional, y está
**desactivada** en una compilación por defecto:

| Característica | Qué trae | Por qué es opcional |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | Embeddings en GPU NVIDIA sobre Linux y Windows. Compilarlo requiere el toolkit de CUDA, y por eso es opcional; sin él el generador de embeddings se ejecuta en la CPU |

> **Eliminado en 1.41.4.** Esta sección listaba antes `tantivy`, `sqlite-vec`,
> `rig-sqlite`, `tokio-rusqlite`, `ort`, `ndarray` y `opentelemetry-otlp` detrás
> de `local-retrieval` y `otel`. Ninguna de esas dos características existe ya y
> ninguno de esos crates está declarado en parte alguna del espacio de trabajo:
> la recuperación la sirve el almacén interno de
> `cobolt-agents/src/knowledge_store.rs`, que guarda los embeddings como
> `Vec<f32>` y los compara con un simple producto escalar.

---

## Los dos crates que compilan C

Conviene saberlo al preparar una máquina (véase [BUILDING-es.md](BUILDING-es.md)):

| Crate | Se alcanza por | Qué compila |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (en `cobolt-runtime`) | La amalgama en C de SQLite, incorporada para que no haya que hacer coincidir ningún SQLite del sistema |
| `onig_sys` | `tokenizers` → `onig` | El motor de expresiones regulares Oniguruma |

Nada en el árbol compila **C++**, y ningún script de compilación invoca CMake,
NASM, Python, Node ni una JVM.

.<<

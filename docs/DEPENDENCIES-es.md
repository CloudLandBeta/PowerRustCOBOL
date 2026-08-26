<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Inventario de crates

Todo crate del que PowerRustCOBOL depende **directamente**, con la versión
realmente enlazada (no la cadena del requisito, sino la resuelta desde
`Cargo.lock`).

Generado a partir de `cargo metadata` el **2026-07-27**, en la versión de
producto **1.37.0**. Fíjese en los dos esquemas de numeración: la versión del
*producto* es la que está en `crates/cobolt-ide/src/version.rs` y se muestra en
el IDE; la versión del *crate* en `Cargo.toml` es `0.2.0` y la comparten todos
los crates del workspace. Para regenerar la columna de versiones:

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

El grafo completo de dependencias son **906 paquetes**. Las tablas de abajo son
los ~56 que el propio workspace nombra; todo lo demás llega transitivamente a
través de ellos.

---

## Crates del workspace

Los 14 crates que *son* PowerRustCOBOL. Todos comparten la versión de crate del
workspace, `0.2.0` (véase la nota de arriba — la versión de producto es 1.37.0).

| Crate | Versión del crate | Capa | Qué hace |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Tokenizador COBOL Fujitsu — código en forma fija y en forma libre |
| `cobolt-parser` | 0.2.0 | front end | Analizador descendente recursivo: flujo de tokens → AST |
| `cobolt-ast` | 0.2.0 | front end | Tipos de nodo de la AST |
| `cobolt-semantic` | 0.2.0 | front end | Resolución de nombres, comprobación de tipos, enlace de `EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | ejecución | Intérprete que recorre el árbol, sistema de valores, ejecutor de `EXEC RUST`, runtimes de BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | ejecución | Funciones intrínsecas, backend de E/S, utilidades de consola |
| `cobolt-indexed` | 0.2.0 | ejecución | Modelo de definición de archivos indexados (`.cidx`) |
| `cobolt-forms` | 0.2.0 | motor de UI | Modelo de formulario/control (`.cfrm`), el motor de renderizado unificado, temas, animación |
| `cobolt-media` | 0.2.0 | motor de UI | Decodificación y reproducción de imágenes animadas (GIF/WebP/APNG) para el widget Animator |
| `cobolt-codegen` | 0.2.0 | herramientas | Generador de código fuente COBOL a partir de formularios |
| `cobolt-compiler` | 0.2.0 | herramientas | Compilador que incrusta y empaqueta: proyecto → un ejecutable nativo |
| `cobolt-agents` | 0.2.0 | IA | Malla de agentes, índice de la Base de Conocimiento, embeddings, recuperación |
| `cobolt-cli` | 0.2.0 | binario | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binario | El IDE en sí |

---

## Dependencias externas

La columna `Usado por` nombra crates del workspace omitiendo el prefijo
`cobolt-`.

### UI y renderizado

| Crate | Versión | Usado por | Qué hace |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | Biblioteca de GUI en modo inmediato — toda la interfaz |
| `eframe` | 0.35.0 | cli, ide | Aloja la ventana y el bucle de eventos de egui |
| `egui_extras` | 0.35.0 | cli, ide | Tablas, cargadores de imágenes, widgets adicionales |
| `egui_glow` | 0.35.0 | ide | Pintor OpenGL — el gancho de recorte de esquinas redondeadas lo necesita |
| `egui_commonmark` | 0.24.0 | ide | Renderizado de Markdown en los paneles de documentación y chat |
| `egui_inspection` | 0.35.0 | ide | Inspector de widgets/diseño en vivo |
| `image` | 0.25.10 | cli, forms, ide, media | Decodificación de PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rasterización de SVG |
| `fontdb` | 0.23.0 | forms, ide | Enumeración de las fuentes del sistema |
| `skrifa` | 0.42.1 | forms | Validación de tipografías con el mismo parser que usa epaint |
| `rfd` | 0.14.1 | ide | Diálogos nativos de abrir/guardar |
| `syntect` | 5.3.0 | ide | Resaltado de sintaxis en el editor |
| `pulldown-cmark` | 0.12.2 | ide | Análisis de Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Renderizado de diagramas mermaid |
| `genpdf` | 0.2.0 | ide | Exportación a PDF |
| `pollster` | 0.3.0 | ide | Bloquea en las pocas llamadas asíncronas que hace el IDE |

### Front end de lenguaje

| Crate | Versión | Usado por | Qué hace |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Generador de analizadores léxicos |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Mapas que conservan el orden de inserción — en COBOL el orden de declaración es semántico |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Tipos de error |

### Datos, almacenamiento y E/S

| Crate | Versión | Usado por | Qué hace |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Almacén ACID embebido, Rust puro — archivos INDEXED y el índice de la Base de Conocimiento |
| `rusqlite` | 0.32.1 | runtime | SQLite para el runtime de bases de datos COBOL (incorporado; compila C) |
| `postgres` | 0.19.13 | runtime | Controlador PostgreSQL (Rust puro, síncrono) |
| `mysql` | 28.0.0 | runtime | Controlador MySQL (Rust puro, conjunto de features rustls) |
| `ureq` | 2.12.1 | runtime | Cliente HTTP bloqueante para el runtime REST de COBOL |
| `native-tls` | 0.2.18 | runtime | TLS por la pila del sistema operativo — sin criptografía incorporada que compilar |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Cliente HTTP para llamadas a modelos y a la web |
| `quick-xml` | 0.36.2 | forms, indexed | Serialización de `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Framework de serialización |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (descontinuado aguas arriba; versión fijada) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifiestos de tema |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Codificación binaria compacta de la AST compilada |
| `flate2` | 1.1.9 | compiler | Deflate — comprime la AST incrustada |
| `zip` | 2.4.2 | cli, ide | Importación/exportación de archivos de proyecto |
| `include_dir` | 0.7.4 | ide | Hornea la documentación empaquetada dentro del binario |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Archivos temporales (también dependencia de desarrollo) |
| `dirs` | 5.0.1 | ide | Directorios de configuración/datos por plataforma |

### IA y recuperación

| Crate | Versión | Usado por | Qué hace |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orquestación de agentes/LLM (native-tls, no rustls) |
| `candle-core` | 0.11.0 | agents | Runtime de tensores en Rust puro |
| `candle-nn` | 0.11.0 | agents | Capas de red neuronal para Candle |
| `candle-transformers` | 0.11.0 | agents | BERT y compañía — ejecuta `all-MiniLM-L6-v2` dentro del proceso |
| `tokenizers` | 0.23.1 | agents | Tokenizador de HuggingFace (`esaxx_fast` apagado, `onig` encendido) |
| `embedvec` | 0.8.0 | agents | Almacén vectorial: cuantización E8, similitud del coseno |
| `schemars` | 1.2.1 | agents, ide | JSON Schema para definiciones de herramientas |
| `opentelemetry` | 0.32.0 | agents | API de trazas/métricas |
| `tokio` | 1.52.3 | agents, ide | Runtime asíncrono de la capa de agentes |
| `futures` | 0.3.32 | agents | Combinadores asíncronos |

### Transversales

| Crate | Versión | Usado por | Qué hace |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Registro estructurado |
| `tracing-subscriber` | 0.3.23 | cli, ide | Filtrado y formato de registros |
| `sysinfo` | 0.31.4 | ide | Estadísticas de proceso/memoria |
| `num_cpus` | 1.17.0 | agents | Dimensionado del paralelismo |
| `rand` | 0.8.6 | ide | Valores aleatorios |
| `hmac` | 0.12.1 | forms | HMAC para la firma de vinculación |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Diferencias legibles en las pruebas (dependencia de desarrollo) |

---

## Declarados, pero no enlazados por defecto

Estos se nombran en algún `Cargo.toml` detrás de una feature que está
**apagada** en una compilación por defecto, de modo que no aportan nada al
tiempo de compilación ni al tamaño del binario a menos que usted la encienda:

| Crate | Feature | Por qué es opcional |
|---|---|---|
| `tantivy` | `local-retrieval` | Índice léxico — el camino por defecto es `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | Búsqueda vectorial sobre SQLite; habilitarla mete el SQLite incorporado (y un toolchain de C) dentro de `cobolt-agents` |
| `ort`, `ndarray` | `local-retrieval` | Camino de inferencia de ONNX Runtime |
| `opentelemetry-otlp` | `otel` | Exportación OTLP |

---

## Los dos crates que compilan C

Conviene saberlo al preparar una máquina (véase
[BUILDING-en.md](BUILDING-en.md)):

| Crate | Se alcanza vía | Qué compila |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (en `cobolt-runtime`) | La amalgama en C de SQLite, incorporada para que ningún SQLite del sistema tenga que coincidir |
| `onig_sys` | `tokenizers` → `onig` | El motor de expresiones regulares Oniguruma |

Nada en el árbol compila **C++**, y ningún script de compilación invoca CMake,
NASM, Python, Node ni una JVM.

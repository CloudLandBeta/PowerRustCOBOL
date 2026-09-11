<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.128 -->

# Inventaire des crates

Toutes les crates dont PowerRustCOBOL dépend **directement**, avec la version
réellement liée (non pas la chaîne d'exigence, mais celle résolue dans
`Cargo.lock`).

Généré pour la première fois par `cargo metadata` le **2026-07-27**, à la
version produit **1.37.0** ; les versions résolues ci-dessous ont été
réconciliées avec `Cargo.lock` pour la dernière fois le **2026-09-11**, en
**1.65.128**. Notez les deux systèmes de numérotation : la version *produit* est
celle de `crates/cobolt-ide/src/version.rs`, affichée dans l'IDE ; la version de
*crate* dans `Cargo.toml` est `0.2.0` et est partagée par toutes les crates de
l'espace de travail. Régénérez la colonne des versions avec :

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

Le graphe de dépendances complet compte **944 paquets**. Les tableaux
ci-dessous sont les **59** que l'espace de travail nomme lui-même ; tout le reste
arrive transitivement au travers d'eux.

---

## Crates de l'espace de travail

Les 17 crates qui *sont* PowerRustCOBOL — l'autorité ici est
`cargo metadata --no-deps`, pas un grep de `Cargo.toml`, où deux membres
partagent une ligne et où un décompte naïf annonce 16. Toutes partagent la
version de crate `0.2.0` de l'espace de travail (voir la note ci-dessus : la
version produit suit sa propre série).

| Crate | Version de la crate | Couche | Ce qu'elle fait |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | front end | Tokeniseur COBOL Fujitsu — source en format fixe et libre — et le préprocesseur `COPY`/`REPLACE` |
| `cobolt-parser` | 0.2.0 | front end | Analyseur par descente récursive : flux de jetons → AST |
| `cobolt-ast` | 0.2.0 | front end | Types de nœuds de l'AST |
| `cobolt-semantic` | 0.2.0 | front end | Résolution des noms, vérification des types, liaison `EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | exécution | Interpréteur à parcours d'arbre, système de valeurs, exécuteur `EXEC RUST`, runtimes BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | exécution | Fonctions intrinsèques, backend d'E/S, utilitaires console |
| `cobolt-indexed` | 0.2.0 | exécution | Modèle de définition des fichiers indexés (`.cidx`) |
| `cobolt-forms` | 0.2.0 | moteur d'interface | Modèle formulaire/contrôle (`.cfrm`), le moteur de rendu unifié, thèmes, animation |
| `cobolt-form-host` | 0.2.0 | moteur d'interface | L'unique hôte de formulaires (spec 042) — partagé par `rcrun run-form` et les applications compilées |
| `cobolt-media` | 0.2.0 | moteur d'interface | Décodage et lecture d'images animées (GIF/WebP/APNG) pour le widget Animator |
| `cobolt-codegen` | 0.2.0 | outillage | Générateur de source COBOL à partir d'un formulaire |
| `cobolt-compiler` | 0.2.0 | outillage | Compilateur d'intégration et d'empaquetage : projet → un exécutable natif |
| `cobolt-dap` | 0.2.0 | outillage | Debug Adapter Protocol compatible au niveau du protocole — trame, types, client et serveur adaptateur |
| `cobolt-agents` | 0.2.0 | IA | Maillage d'agents, index de la base de connaissances, embeddings, recherche |
| `cobolt-cli` | 0.2.0 | binaire | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binaire | L'IDE lui-même |
| `cobolt-bench` | 0.2.0 | binaire | Banc de référence des performances et des allocations (voir [BENCHMARKS-fr.md](BENCHMARKS-fr.md)) |

---

## Dépendances externes

`Used by` nomme les crates de l'espace de travail sans le préfixe `cobolt-`.

### Interface et rendu

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `egui` | 0.36.1 | cli, forms, ide, media | Boîte à outils GUI en mode immédiat — toute l'interface |
| `eframe` | 0.36.0 | cli, ide | Hôte de fenêtre et boucle d'événements pour egui |
| `egui_extras` | 0.36.0 | cli, ide | Tableaux, chargeurs d'images, widgets supplémentaires |
| `egui_commonmark` | 0.25.0 | ide | Rendu Markdown dans les panneaux de documentation et de discussion |
| `egui_inspection` | 0.36.0 | ide | Inspecteur de widgets et de mise en page en direct |
| `image` | 0.25.10 | cli, forms, ide, media | Décodage PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rastérisation SVG |
| `fontdb` | 0.23.0 | forms, ide | Énumération des polices du système |
| `skrifa` | 0.42.1 | forms | Validation des fontes avec l'analyseur qu'epaint utilise lui-même |
| `rfd` | 0.14.1 | ide | Boîtes de dialogue natives Ouvrir/Enregistrer |
| `syntect` | 5.3.0 | ide | Coloration syntaxique dans l'éditeur |
| `pulldown-cmark` | 0.12.2 | ide | Analyse Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Rendu des diagrammes mermaid |
| `genpdf` | 0.2.0 | ide | Export PDF |
| `pollster` | 0.3.0 | ide | Bloque sur les quelques appels asynchrones que fait l'IDE |

### Front end du langage

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Générateur d'analyseurs lexicaux |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Tables préservant l'ordre d'insertion — en COBOL l'ordre de déclaration est sémantique |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Types d'erreur |

### Données, stockage et E/S

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Magasin ACID embarqué en Rust pur — fichiers INDEXED et index de la base de connaissances |
| `rusqlite` | 0.32.1 | runtime | SQLite pour le runtime de bases de données COBOL (intégré ; compile du C) |
| `postgres` | 0.19.13 | runtime | Pilote PostgreSQL (Rust pur, synchrone) |
| `mysql` | 28.0.0 | runtime | Pilote MySQL (Rust pur, jeu de fonctionnalités `minimal-rust` — **pas de TLS** ; voir [database-runtime-fr.md](database-runtime-fr.md)) |
| `ureq` | 2.12.1 | runtime | Client HTTP bloquant pour le runtime REST de COBOL |
| `native-tls` | 0.2.18 | runtime | TLS via la pile du système d'exploitation — aucune crypto embarquée à compiler |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Client HTTP pour les appels aux modèles et au web |
| `quick-xml` | 0.36.2 | forms, indexed | Sérialisation `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Cadre de sérialisation |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (abandonné en amont ; épinglé) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifestes de thèmes |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Encodage binaire compact de l'AST compilé |
| `flate2` | 1.1.9 | compiler | Deflate — compresse l'AST intégré |
| `zip` | 2.4.2 | cli, ide | Import/export des archives de projet |
| `include_dir` | 0.7.4 | ide | Intègre la documentation livrée dans le binaire |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Fichiers temporaires (également dépendance de développement) |
| `dirs` | 5.0.1 | ide | Répertoires de configuration et de données par plateforme |

### IA et recherche

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orchestration agents/LLM (native-tls, pas rustls) |
| `candle-core` | 0.11.0 | agents | Runtime de tenseurs en Rust pur |
| `candle-nn` | 0.11.0 | agents | Couches de réseaux de neurones pour Candle |
| `candle-transformers` | 0.11.0 | agents | BERT et consorts — exécute `all-MiniLM-L6-v2` dans le processus |
| `tokenizers` | 0.23.1 | agents | Tokeniseur HuggingFace (`esaxx_fast` désactivé, `onig` activé) |
| `schemars` | 1.2.1 | agents, ide | JSON Schema pour les définitions d'outils |
| `tokio` | 1.52.3 | agents, ide | Runtime asynchrone de la couche agents |
| `futures` | 0.3.32 | agents | Combinateurs asynchrones |

### Transverses

| Crate | Version | Used by | What it does |
|---|---|---|---|
| `tracing` | 0.1.44 | agents, cli, compiler, ide, runtime, stdlib | Journalisation structurée |
| `tracing-subscriber` | 0.3.23 | cli, ide | Filtrage et mise en forme des journaux |
| `sysinfo` | 0.31.4 | ide | Statistiques de processus et de mémoire |
| `num_cpus` | 1.17.0 | agents | Dimensionnement du parallélisme |
| `rand` | 0.8.6 | ide | Valeurs aléatoires |
| `hmac` | 0.12.1 | forms | HMAC pour la signature de liaison |
| `sha2` | 0.10.9 | forms | SHA-2 |
| `pretty_assertions` | 1.4.1 | ast, forms, indexed, lexer, parser, runtime, semantic, stdlib | Différences lisibles dans les tests (dépendance de développement) |

---

## Fonctionnalités optionnelles

`cobolt-agents` déclare exactement une fonctionnalité optionnelle, et elle est
**désactivée** dans une compilation par défaut :

| Fonctionnalité | Ce qu'elle apporte | Pourquoi elle est optionnelle |
|---|---|---|
| `embed-cuda` | `candle-transformers/cuda` | Embeddings sur GPU NVIDIA sous Linux et Windows. Sa compilation exige la boîte à outils CUDA, d'où son caractère optionnel ; sans elle, le générateur d'embeddings tourne sur le CPU |

> **Retiré en 1.41.4.** Cette section listait autrefois `tantivy`,
> `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite`, `ort`, `ndarray` et
> `opentelemetry-otlp` derrière `local-retrieval` et `otel`. Aucune de ces deux
> fonctionnalités n'existe plus et aucune de ces crates n'est déclarée nulle part
> dans l'espace de travail — la recherche est assurée par le magasin interne de
> `cobolt-agents/src/knowledge_store.rs`, qui conserve les embeddings en
> `Vec<f32>` et les compare par simple produit scalaire.

---

## Les deux crates qui compilent du C

Bon à savoir au moment de préparer une machine (voir [BUILDING-fr.md](BUILDING-fr.md)) :

| Crate | Atteinte par | Ce qu'elle compile |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (dans `cobolt-runtime`) | L'amalgame C de SQLite, intégré pour qu'aucun SQLite système n'ait à correspondre |
| `onig_sys` | `tokenizers` → `onig` | Le moteur d'expressions régulières Oniguruma |

Rien dans l'arborescence ne compile de **C++**, et aucun script de compilation
n'invoque CMake, NASM, Python, Node ou une JVM.

.<<

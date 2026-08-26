<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Inventaire des crates

Chaque crate dont PowerRustCOBOL dépend **directement**, avec la version
réellement liée (non pas la chaîne d'exigence, mais celle résolue depuis
`Cargo.lock`).

Généré à partir de `cargo metadata` le **2026-07-27**, en version produit
**1.37.0**. Notez les deux schémas de numérotation : la version du *produit* est
celle de `crates/cobolt-ide/src/version.rs`, affichée dans l'IDE ; la version du
*crate* dans `Cargo.toml` est `0.2.0` et elle est partagée par tous les crates
du workspace. Pour régénérer la colonne des versions :

```sh
cargo metadata --format-version 1 | \
  jq -r '.resolve.nodes[] | select(.id | contains("PowerRustCOBOL")) | .deps[].pkg'
```

Le graphe complet des dépendances compte **906 paquets**. Les tableaux ci-dessous
recensent les ~56 que le workspace nomme lui-même ; tout le reste arrive
transitivement par leur intermédiaire.

---

## Crates du workspace

Les 14 crates qui *constituent* PowerRustCOBOL. Tous partagent la version de
crate du workspace, `0.2.0` (voir la note ci-dessus — la version produit est
1.37.0).

| Crate | Version du crate | Couche | Rôle |
|---|---|---|---|
| `cobolt-lexer` | 0.2.0 | frontal | Tokeniseur COBOL Fujitsu — sources en format fixe et en format libre |
| `cobolt-parser` | 0.2.0 | frontal | Analyseur descendant récursif : flux de tokens → AST |
| `cobolt-ast` | 0.2.0 | frontal | Types de nœuds de l'AST |
| `cobolt-semantic` | 0.2.0 | frontal | Résolution des noms, vérification des types, liaison d'`EXEC RUST` |
| `cobolt-runtime` | 0.2.0 | exécution | Interpréteur à parcours d'arbre, système de valeurs, exécuteur d'`EXEC RUST`, runtimes BD/HTTP |
| `cobolt-stdlib` | 0.2.0 | exécution | Fonctions intrinsèques, backend d'E/S, utilitaires console |
| `cobolt-indexed` | 0.2.0 | exécution | Modèle de définition des fichiers indexés (`.cidx`) |
| `cobolt-forms` | 0.2.0 | moteur d'IU | Modèle formulaire/contrôle (`.cfrm`), moteur de rendu unifié, thèmes, animation |
| `cobolt-media` | 0.2.0 | moteur d'IU | Décodage et lecture d'images animées (GIF/WebP/APNG) pour le widget Animator |
| `cobolt-codegen` | 0.2.0 | outillage | Générateur de source COBOL à partir des formulaires |
| `cobolt-compiler` | 0.2.0 | outillage | Compilateur d'intégration et d'empaquetage : projet → un exécutable natif |
| `cobolt-agents` | 0.2.0 | IA | Maillage d'agents, index de la Base de Connaissances, embeddings, recherche |
| `cobolt-cli` | 0.2.0 | binaire | `rcrun` — run, check, build, run-form |
| `cobolt-ide` | 0.2.0 | binaire | L'IDE lui-même |

---

## Dépendances externes

La colonne `Utilisé par` nomme les crates du workspace sans le préfixe
`cobolt-`.

### Interface et rendu

| Crate | Version | Utilisé par | Rôle |
|---|---|---|---|
| `egui` | 0.35.0 | cli, forms, ide, media | Bibliothèque d'interface en mode immédiat — toute l'IU |
| `eframe` | 0.35.0 | cli, ide | Héberge la fenêtre et la boucle d'événements d'egui |
| `egui_extras` | 0.35.0 | cli, ide | Tableaux, chargeurs d'images, widgets supplémentaires |
| `egui_glow` | 0.35.0 | ide | Peintre OpenGL — le crochet de découpe des coins arrondis en dépend |
| `egui_commonmark` | 0.24.0 | ide | Rendu Markdown dans les panneaux documentation et discussion |
| `egui_inspection` | 0.35.0 | ide | Inspecteur de widgets et de mise en page en direct |
| `image` | 0.25.10 | cli, forms, ide, media | Décodage PNG/JPEG/GIF/WebP/BMP |
| `resvg` | 0.46.0 | forms, ide | Rastérisation SVG |
| `fontdb` | 0.23.0 | forms, ide | Énumération des polices du système |
| `skrifa` | 0.42.1 | forms | Validation des fontes avec l'analyseur qu'epaint utilise lui-même |
| `rfd` | 0.14.1 | ide | Boîtes de dialogue natives d'ouverture et d'enregistrement |
| `syntect` | 5.3.0 | ide | Coloration syntaxique dans l'éditeur |
| `pulldown-cmark` | 0.12.2 | ide | Analyse du Markdown |
| `mermaid-rs-renderer` | 0.2.2 | ide | Rendu des diagrammes mermaid |
| `genpdf` | 0.2.0 | ide | Export PDF |
| `pollster` | 0.3.0 | ide | Bloque sur les rares appels asynchrones que fait l'IDE |

### Frontal du langage

| Crate | Version | Utilisé par | Rôle |
|---|---|---|---|
| `logos` | 0.14.4 | lexer | Générateur d'analyseur lexical |
| `indexmap` | 2.14.0 | ast, codegen, forms, ide, runtime, semantic, stdlib | Tables conservant l'ordre d'insertion — en COBOL l'ordre de déclaration est sémantique |
| `thiserror` | 2.0.18 | agents, compiler, forms, indexed, lexer, runtime, semantic, stdlib | Types d'erreur |

### Données, stockage et E/S

| Crate | Version | Utilisé par | Rôle |
|---|---|---|---|
| `redb` | 2.6.3 | agents, runtime | Magasin ACID embarqué, Rust pur — fichiers INDEXED et index de la Base de Connaissances |
| `rusqlite` | 0.32.1 | runtime | SQLite pour le runtime de bases de données COBOL (intégré ; compile du C) |
| `postgres` | 0.19.13 | runtime | Pilote PostgreSQL (Rust pur, synchrone) |
| `mysql` | 28.0.0 | runtime | Pilote MySQL (Rust pur, jeu de features rustls) |
| `ureq` | 2.12.1 | runtime | Client HTTP bloquant pour le runtime REST de COBOL |
| `native-tls` | 0.2.18 | runtime | TLS via la pile du système — aucune bibliothèque cryptographique à compiler |
| `reqwest` | 0.12.28 / 0.13.4 | ide / agents | Client HTTP pour les appels aux modèles et au web |
| `quick-xml` | 0.36.2 | forms, indexed | Sérialisation des `.cfrm` / `.cidx` |
| `serde` | 1.0.228 | agents, ast, cli, compiler, forms, ide, lexer, runtime | Cadre de sérialisation |
| `serde_json` | 1.0.150 | agents, cli, forms, ide, runtime | JSON |
| `serde_yaml` | 0.9.34 | forms | YAML (abandonné en amont ; version figée) |
| `toml` | 0.8.23 | cli, compiler, forms, ide | `cobolt.toml`, manifestes de thèmes |
| `bincode` | 1.3.3 | agents, cli, compiler, ide | Encodage binaire compact de l'AST compilée |
| `flate2` | 1.1.9 | compiler | Deflate — compresse l'AST embarquée |
| `zip` | 2.4.2 | cli, ide | Import/export des archives de projet |
| `include_dir` | 0.7.4 | ide | Intègre la documentation livrée dans le binaire |
| `tempfile` | 3.27.0 | agents, forms, indexed, runtime | Fichiers temporaires (également dépendance de développement) |
| `dirs` | 5.0.1 | ide | Répertoires de configuration et de données par plateforme |

### IA et recherche

| Crate | Version | Utilisé par | Rôle |
|---|---|---|---|
| `rig-core` | 0.40.0 | agents | Orchestration agents/LLM (native-tls, pas rustls) |
| `candle-core` | 0.11.0 | agents | Runtime de tenseurs en Rust pur |
| `candle-nn` | 0.11.0 | agents | Couches de réseaux de neurones pour Candle |
| `candle-transformers` | 0.11.0 | agents | BERT et consorts — exécute `all-MiniLM-L6-v2` dans le processus |
| `tokenizers` | 0.23.1 | agents | Tokeniseur HuggingFace (`esaxx_fast` désactivé, `onig` activé) |
| `embedvec` | 0.8.0 | agents | Magasin vectoriel : quantification E8, similarité cosinus |
| `schemars` | 1.2.1 | agents, ide | JSON Schema pour les définitions d'outils |
| `opentelemetry` | 0.32.0 | agents | API de traçage et de métriques |
| `tokio` | 1.52.3 | agents, ide | Runtime asynchrone de la couche agents |
| `futures` | 0.3.32 | agents | Combinateurs asynchrones |

### Transversal

| Crate | Version | Utilisé par | Rôle |
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

## Déclarés mais non liés par défaut

Ceux-ci sont nommés dans un `Cargo.toml` derrière une feature **désactivée**
dans une compilation par défaut : ils n'ajoutent donc rien au temps de
compilation ni à la taille du binaire tant que vous ne l'activez pas.

| Crate | Feature | Pourquoi c'est optionnel |
|---|---|---|
| `tantivy` | `local-retrieval` | Index lexical — le chemin par défaut est `embedvec` + `redb` |
| `sqlite-vec`, `rig-sqlite`, `tokio-rusqlite` | `local-retrieval` | Recherche vectorielle sur SQLite ; l'activer fait entrer le SQLite intégré (et une chaîne d'outils C) dans `cobolt-agents` |
| `ort`, `ndarray` | `local-retrieval` | Chemin d'inférence ONNX Runtime |
| `opentelemetry-otlp` | `otel` | Export OTLP |

---

## Les deux crates qui compilent du C

Bon à savoir au moment de préparer une machine (voir
[BUILDING-en.md](BUILDING-en.md)) :

| Crate | Atteint via | Ce qu'il compile |
|---|---|---|
| `libsqlite3-sys` | `rusqlite` (dans `cobolt-runtime`) | L'amalgame C de SQLite, intégré pour qu'aucun SQLite système n'ait à correspondre |
| `onig_sys` | `tokenizers` → `onig` | Le moteur d'expressions régulières Oniguruma |

Rien dans l'arborescence ne compile de **C++**, et aucun script de compilation
n'invoque CMake, NASM, Python, Node ou une JVM.

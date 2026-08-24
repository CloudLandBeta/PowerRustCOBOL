<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Benchmarks

La référence 1.37.0 : à quelle vitesse le runtime tourne sous charge, et à quel
point il s'appuie sur l'allocateur pour y parvenir.

```sh
cargo run --release -p cobolt-bench              # everything
cargo run --release -p cobolt-bench -- dispatch  # one workload, by substring
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # a twentieth, for a quick check
```

`--release` n'est pas facultatif. Une compilation debug mesure l'*absence*
d'optimisation, et le harnais le dit dans son en-tête plutôt que de laisser
citer les chiffres.

## Ce qui est mesuré

Chaque charge de travail COBOL emprunte le **même chemin qu'un binaire livré** —
tokeniser, analyser syntaxiquement, analyser sémantiquement, `Interpreter::run`
— parce que c'est ce que fait le `main.rs` généré par `rcrun build` avec son AST
embarquée. Tourner dans le processus lui-même est ce qui rend possibles les
compteurs de l'allocateur : les chiffres décrivent l'interpréteur qui se trouve
à l'intérieur de chaque binaire que vous livrez.

La mémoire est rapportée comme un comportement d'allocation, et non comme une
courbe de resident set. Rust n'a pas de ramasse-miettes, il n'y a donc aucune
pause à mesurer ; ce qui compte sous charge, c'est le **brassage** — combien de
fois une charge entre dans l'allocateur, combien d'octets le traversent, et
combien reste vivant au pic. Un allocateur global à compteurs
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) fournit
les trois exactement, sur les trois plateformes, sans profileur externe.

Deux choses que ceci ne mesure délibérément **pas** : le démarrage du processus
et la taille du binaire. Mesurez-les sur l'artefact réel issu de
`rcrun build`.

## La référence 1.37.0

Apple M3 Pro, 18 Go, macOS 15.5, rustc 1.95.0, profil release, 2026-07-27. Les
chiffres absolus voyagent mal d'une machine à l'autre ; les **allocations par
opération** voyagent bien et constituent la colonne à surveiller.

| Charge de travail | Ops | Durée | Ops/s | Allocations | Alloc./op | Mo brassés | Pic vivant (Mo) |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 stmt | 1,049s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 call | 0,729s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 compute | 0,824s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 rows, write+read) | 400 000 record | 2,179s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (create/read/destroy) | 20 000 object | 0,092s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (bulk insert) | 100 000 record | 0,710s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (random read) | 50 000 read | 0,034s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## Ce que dit la référence

**Le goulot d'étranglement est l'allocateur, pas le parcours de l'arbre.**
5,7 M d'instructions par seconde est un débit de dispatch respectable — mais y
parvenir a coûté **24 millions d'allocations pour 6 millions d'instructions**.
Un `ADD 1 TO ACC` sur deux champs `COMP`, qui ne devrait toucher le tas en aucune
façon, coûte quatre passages par l'allocateur. Cela recadre le travail
d'optimisation : les premiers gains sont dans le système de valeurs et le chemin
des opérandes, pas dans le remplacement de l'interpréteur à parcours d'arbre par
une VM à bytecode. Une VM rendrait le dispatch moins cher tout en laissant
intactes les quatre allocations par instruction.

**Les appels de paragraphe coûtent hors de proportion.** 18 allocations et
environ 820 octets par `PERFORM <paragraph>`, contre 4 par instruction en ligne.
Un demi-million d'appels brassent 410 Mo. Quoi que le chemin d'appel construise
à chaque invocation, c'est la cible la plus dense du tableau.

**Les enregistrements alphanumériques allouent par champ, comme prévu.** 65
allocations par enregistrement pour une ligne de 4 champs lue et écrite, c'est
`CobolValue::String` possédant un `Vec<u8>` par champ, plus un neuf à chaque
`MOVE`. Une représentation en ligne pour les chaînes courtes, ou un découpage
dans le tampon même de l'enregistrement, se verrait ici immédiatement.

**Les lectures de propriétés d'objet allouent sans raison.** 55 allocations par
objet sur 24 lectures de propriété. `CoboltObject::get_property`, `get_str`,
`get_bool` et `get_i64` appellent chacun `name.to_ascii_uppercase()` — une
`String` allouée puis jetée **à chaque lecture**, uniquement pour rendre la
recherche insensible à la casse. Une enveloppe de clé insensible à la casse
supprime la colonne entière.

**Le moteur INDEXED n'est pas le problème.** redb insère à 141 000
enregistrements par seconde avec 0,66 allocation par enregistrement et sert
1,5 M de lectures aléatoires par seconde avec une allocation pour ainsi dire
nulle. Le stockage est confortablement en avance sur l'interpréteur qui
l'alimente.

Classé par retour attendu, l'ordre d'optimisation que suggère la référence est :
les allocations par instruction, puis le chemin d'appel de paragraphe, puis
`CobolValue` pour les alphanumériques, puis la mise en majuscules des propriétés
d'objet. Le stockage n'apparaît que bien en dessous de tout cela.

## Charges de travail

| Charge de travail | Ce qu'elle isole |
|---|---|
| `dispatch (PERFORM VARYING)` | Coût du parcours d'arbre : test de boucle, incrément, une instruction, travail minimal en dessous |
| `dispatch (PERFORM paragraph)` | Coût de l'appel de paragraphe, face au cas en ligne ci-dessus |
| `decimal COMPUTE` | L'arithmétique i128 mise à l'échelle de `CobolNumeric` — les calculs monétaires de COBOL |
| `record batch` | Table de 1000 lignes écrite puis relue avec des champs alphanumériques ; le système de valeurs sous charge par lots |
| `object churn` | Créer/lire/détruire d'`ObjectRegistry` — ce que coûte un formulaire à nombreux contrôles |
| `indexed redb` | Le moteur de fichiers INDEXED : insertion en masse, puis lectures par clé aléatoire |

Les deux lignes `indexed redb` sont une version récupérée et généralisée du
micro-benchmark `open_table_cost`, qui vivait marqué `#[ignore]` à l'intérieur
de `cobolt-runtime::indexed_redb`. Il ne s'exécutait que lorsque quelqu'un se
souvenait d'une invocation `--ignored` exacte : le moteur n'avait donc pas de
référence permanente ; il en a une désormais. Sa conclusion d'origine est
conservée — le handle de table est ouvert une seule fois pour toute la
transaction d'écriture, ce qui s'est mesuré environ 16 % plus rapide que de
l'ouvrir deux fois par insertion.

## Ajouter une charge de travail

Ajoutez une fonction `bench_*` dans
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) qui
renvoie `measure(name, unit, || { ...; ops_performed })`, et enregistrez-la dans
`main` derrière un filtre `wanted(...)`. Les compteurs enveloppent la closure
automatiquement. Renvoyez le nombre d'unités de *travail*, pas d'itérations,
pour que `ops/sec` et `allocs/op` restent comparables d'une charge à l'autre.

Gardez les nouvelles charges déterministes. La sonde de lecture aléatoire
utilise un pas multiplicatif fixe plutôt qu'un générateur de nombres aléatoires
exactement pour cette raison : un benchmark qui se rebat les cartes entre deux
exécutions ne peut pas être comparé au chiffre d'hier.

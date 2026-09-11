<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Tests de performance

La référence 1.37.0 : à quelle vitesse le runtime tourne sous charge, et avec
quelle intensité il s'appuie sur l'allocateur pour y parvenir.

```sh
cargo run --release -p cobolt-bench              # tout
cargo run --release -p cobolt-bench -- dispatch  # une charge, par sous-chaîne
PRC_BENCH_SCALE=0.05 cargo run --release -p cobolt-bench   # un vingtième, pour une vérification rapide
```

`--release` n'est pas facultatif. Une compilation de débogage mesure l'absence
d'optimisation, et le harnais le signale dans son en-tête plutôt que de laisser
citer ces chiffres.

## Ce qui est mesuré

Chaque charge COBOL emprunte **le même chemin qu'un binaire livré** — tokeniser,
analyser syntaxiquement, analyser sémantiquement, `Interpreter::run` — car c'est
ce que fait le `main.rs` généré par `rcrun build` avec son AST embarqué.
L'exécution dans le processus est ce qui rend possibles les compteurs de
l'allocateur : les chiffres décrivent l'interpréteur présent dans chaque binaire
que vous livrez.

La mémoire est rapportée comme un comportement d'allocation, non comme une courbe
d'ensemble résident. Rust n'a pas de ramasse-miettes, il n'y a donc aucune pause
à mesurer ; ce qui compte sous charge, c'est le **brassage** — combien de fois
une charge entre dans l'allocateur, combien d'octets le traversent, et combien
sont vivants au pic. Un allocateur global compteur
([`counting_alloc.rs`](../crates/cobolt-bench/src/counting_alloc.rs)) fournit ces
trois valeurs exactement, sur les trois plateformes, sans profileur externe.

Deux choses que ceci ne mesure délibérément **pas** : le démarrage du processus
et la taille du binaire. Mesurez-les sur l'artefact réel de `rcrun build`.

## La référence 1.37.0

Apple M3 Pro, 18 Go, macOS 15.5, rustc 1.95.0, profil release, 2026-07-27.
Les chiffres absolus voyagent mal d'une machine à l'autre ; **allocations par
opération** voyage bien et c'est la colonne à surveiller.

| Charge | Ops | Durée | Ops/s | Alloc. | Alloc./op | Mo brassés | Pic vivant Mo |
|---|---:|---:|---:|---:|---:|---:|---:|
| dispatch (PERFORM VARYING) | 6 000 000 instr. | 1,049 s | 5 721 961 | 24 000 334 | 4,00 | 72,5 | 0,0 |
| dispatch (PERFORM paragraph) | 500 000 appels | 0,729 s | 686 318 | 9 000 356 | 18,00 | 409,6 | 0,0 |
| decimal COMPUTE | 500 000 calculs | 0,824 s | 606 461 | 10 000 499 | 20,00 | 41,0 | 0,0 |
| record batch (1000 lignes, écriture+lecture) | 400 000 enregistr. | 2,179 s | 183 612 | 26 023 007 | 65,06 | 227,9 | 0,8 |
| object churn (créer/lire/détruire) | 20 000 objets | 0,092 s | 216 320 | 1 100 000 | 55,00 | 27,5 | 0,0 |
| indexed redb (insertion en masse) | 100 000 enregistr. | 0,710 s | 140 922 | 65 854 | 0,66 | 188,9 | 22,4 |
| indexed redb (lecture aléatoire) | 50 000 lectures | 0,034 s | 1 489 965 | 9 | 0,00 | 0,0 | 22,4 |

## Ce que dit la référence

**Le goulot d'étranglement est l'allocateur, pas le parcours d'arbre.** 5,7 M
d'instructions par seconde est un taux d'aiguillage respectable — mais y parvenir
a coûté **24 millions d'allocations pour 6 millions d'instructions**.
`ADD 1 TO ACC` sur deux champs `COMP`, qui ne devrait pas toucher le tas du tout,
coûte quatre passages par l'allocateur. Cela recadre le travail d'optimisation :
les premiers gains sont dans le système de valeurs et dans le chemin des
opérandes, pas dans le remplacement de l'interpréteur par une machine virtuelle à
bytecode. Une VM rendrait l'aiguillage moins cher en laissant intactes les quatre
allocations par instruction.

**Les appels de paragraphe coûtent cher de façon disproportionnée.** 18
allocations et environ 820 octets par `PERFORM <paragraph>`, contre 4 par
instruction en ligne. Un demi-million d'appels brassent 410 Mo. Ce que le chemin
d'appel construit à chaque invocation est la cible la plus dense du tableau.

**Les enregistrements alphanumériques allouent par champ, comme prévu.** 65
allocations par enregistrement pour une ligne de 4 champs lue et écrite, c'est
`CobolValue::String` possédant un `Vec<u8>` par champ, plus un nouveau à chaque
`MOVE`. Une représentation de chaîne courte en ligne, ou un découpage dans le
tampon propre de l'enregistrement, se verrait ici immédiatement.

**Les lectures de propriétés d'objet allouent sans raison.** 55 allocations par
objet sur 24 lectures de propriété. `CoboltObject::get_property`, `get_str`,
`get_bool` et `get_i64` appellent chacun `name.to_ascii_uppercase()` — une
`String` allouée puis abandonnée **à chaque lecture**, uniquement pour rendre la
recherche insensible à la casse. Une enveloppe de clé insensible à la casse
supprime la colonne entière.

**Le moteur INDEXED n'est pas le problème.** redb insère à 141 k
enregistrements/s avec 0,66 allocation par enregistrement et sert 1,5 M de
lectures aléatoires par seconde pratiquement sans allouer. Le stockage est
confortablement en avance sur l'interpréteur qui l'alimente.

Classé par retour attendu, l'ordre d'optimisation que suggère la référence est :
les allocations par instruction, puis le chemin d'appel de paragraphe, puis
`CobolValue` pour les alphanumériques, puis la mise en majuscules des propriétés
d'objet. Le stockage n'apparaît que bien en dessous de tout cela.

## Charges de travail

| Charge | Ce qu'elle isole |
|---|---|
| `dispatch (PERFORM VARYING)` | Surcoût du parcours d'arbre : test de boucle, incrément, une instruction, travail minimal en dessous |
| `dispatch (PERFORM paragraph)` | Surcoût de l'appel de paragraphe, face au cas en ligne ci-dessus |
| `decimal COMPUTE` | L'arithmétique à l'échelle i128 de `CobolNumeric` — le calcul monétaire COBOL |
| `record batch` | Table de 1000 lignes écrite puis relue avec des champs alphanumériques ; le système de valeurs sous charge par lots |
| `object churn` | `ObjectRegistry` créer/lire/détruire — ce que coûte un formulaire comportant de nombreux contrôles |
| `indexed redb` | Le moteur de fichiers INDEXED : insertion en masse, puis lectures par clé aléatoire |

Les deux lignes `indexed redb` sont une version récupérée et généralisée du
micro-benchmark `open_table_cost`, qui reste marqué `#[ignore]` à l'intérieur de
`cobolt-runtime::indexed_redb` (`indexed_redb.rs:1264`). Il ne s'exécute que
lorsque quelqu'un se souvient d'une invocation `--ignored` exacte ; le moteur
n'avait donc pas de référence permanente. Il en a une ici désormais, et
l'original reste où il est. Sa conclusion d'origine est conservée : le descripteur
de table est ouvert une seule fois pour toute la transaction d'écriture, ce qui
s'est mesuré ~16 % plus rapide que de l'ouvrir deux fois par insertion.

## Ajouter une charge de travail

Ajoutez une fonction `bench_*` à
[`crates/cobolt-bench/src/main.rs`](../crates/cobolt-bench/src/main.rs) qui
renvoie `measure(name, unit, || { ...; ops_performed })`, et enregistrez-la dans
`main` derrière un filtre `wanted(...)`. Les compteurs enveloppent la fermeture
automatiquement. Renvoyez le nombre d'unités de *travail*, non d'itérations, afin
que `ops/sec` et `allocs/op` restent comparables d'une charge à l'autre.

Gardez les nouvelles charges déterministes. La sonde de lecture aléatoire utilise
un pas multiplicatif fixe plutôt qu'un générateur de nombres aléatoires
précisément pour cette raison : un benchmark qui se rebat entre deux exécutions
ne peut pas être comparé au chiffre d'hier.

.<<

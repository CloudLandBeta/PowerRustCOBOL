<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Référence de la syntaxe prise en charge par RustCOBOL‑85

**À quoi sert ce document :** dire quelle part de la norme COBOL‑85 RustCOBOL
implémente réellement — et le prouver face à la **suite officielle de validation
NIST COBOL‑85** plutôt que de l'affirmer. Le
[tableau de bord](#-la-conformité-est-mesurée-pas-affirmée--nist-ccvs85) ci-dessous
est le chiffre à retenir ; tout ce qui le suit est le détail derrière ce chiffre.

**Vérité de terrain sur ce que le lexer/parser/runtime de RustCOBOL accepte
réellement aujourd'hui**, dérivée des sources (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`) et vérifiée face à `NIST/newcob.val,cbl`.
Écrivez les tests sur les formes ✅ ; les formes ❌ échouent à l'analyse ou sont
sans effet, et les formes ⚠️ s'analysent mais ne se comportent que
partiellement. Ce document est le compagnon de
[`cobol85-verb-test-matrix-fr.md`](cobol85-verb-test-matrix-fr.md) : la matrice dit
*quoi* tester, celui-ci dit *quelle écriture RustCOBOL comprend*.

Légende : ✅ pris en charge · ⚠️ analysé mais partiel/simplifié · ❌ non reconnu
(à éviter, ou à tester uniquement pour confirmer le manque).

---

## ★ La conformité est mesurée, pas affirmée — NIST CCVS85

**C'est là tout l'objet du document.** Chaque affirmation ci-dessous est vérifiée
face à la **suite officielle de validation NIST COBOL‑85** — CCVS85 version 4.0
(01 OCT 1992, COBOL 85 version 4.2, Apr 1993 SSVG), la suite que le National
Institute of Standards and Technology des États-Unis utilisait pour certifier les
compilateurs COBOL. Elle pèse 28 MB, 348,271 lignes, **459 programmes COBOL** et
51 membres de copybook, et elle réside dans ce dépôt sous
`NIST/newcob.val,cbl`.

C'est la source de vérité. Là où RustCOBOL et CCVS85 divergent, **CCVS85 a raison
et RustCOBOL a tort**, et l'écart est consigné comme un défaut dans
[`specs/nist/`](../specs/nist/README.md) — une spécification par correctif, avec
les programmes en échec nommément désignés.

### Le tableau de bord

Mesuré le 2026‑08‑28 en version 1.62.43, sur la distribution intacte :

| | Programmes | Part | Signification |
|---|---:|---:|---|
| ✅ **PASS** | **422** | **97.2 %** | des 434 programmes dans le périmètre |
| ❌ **FAIL** | **12** | 2.8 % | des 434 programmes dans le périmètre |
| ⬜ **N/A** | **25** | — | modules hors du périmètre de RustCOBOL (ci-dessous) |
| | **459** | | total des programmes de la suite |

Pour le reproduire :

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict
```

#### ⚠️ Compiler est l'affirmation la plus faible

Le tableau ci-dessus compte les programmes que le **front end accepte**. Il ne
dit pas qu'ils s'exécutent. La suite se note elle-même — chaque programme CCVS85
imprime son propre rapport `PASS` / `FAIL*` — d'où un second chiffre,
strictement plus fort : combien vont jusqu'au bout et ne signalent **aucun
échec**.

```bash
cargo build --release -p cobolt-cli          # always: the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

Les deux chiffres sont rapportés par module, et jamais confondus :

| Module | Compilation | Exécution (0 échec) |
|---|---:|---:|
| **NC (Noyau)** | **95 / 95** | **83 / 95** |

Le travail avance **un module à la fois** : NC n'est terminé que lorsque les deux
chiffres atteignent 95, et aucun autre module n'est traité avant. Un score de
compilation large réparti sur dix modules ne dit rien sur le fait que l'un
d'entre eux fonctionne.

##### Les cinq membres NC qui demandent plus qu'un fichier d'impression — tous notés

Le score d'exécution compte un programme comme propre lorsque **son propre
rapport CCVS** ne montre aucun échec. Cinq membres NC n'impriment pas ce rapport,
et ce n'est pas parce que quelque chose est cassé. Chacun demandait du travail
sur le banc d'essai plutôt que sur le compilateur, et chacun est désormais noté :

| Membre | Ce qu'il lui faut | Comment il est noté |
|---|---|---|
| **NC302M**, **NC303M**, **NC401M** | Tests de *signalement (flagging)*. Ils ne portent aucune machinerie `PASS`/`FAIL` — chacun se termine par `TOTAL NUMBER OF FLAGS EXPECTED = n`, et le résultat validé est l'ensemble des **diagnostics émis par le compilateur** pour les constructions obsolètes (NC302M/NC303M) ou pour les constructions au-dessus du sous-ensemble haut (NC401M). | Le banc d'essai compare les diagnostics à la liste d'attentes du membre lui-même, ligne à ligne. Les deux classes sont exécutées en **passes séparées** : `DATE-COMPILED` est à la fois obsolète *et* au-dessus du sous-ensemble haut, si bien qu'une passe combinée unique donnerait à chaque membre les signalements de l'autre en faux positifs. |
| **NC110M** | Écrit son rapport avec `DISPLAY`, vers la console de l'opérateur, et non vers le fichier d'impression CCVS que lit le banc d'essai. | La sortie console du processus fils est capturée dans un fichier et notée à partir de là. |
| **NC109M**, **NC204M** | Testent l'`ACCEPT` de Format 1, qui lit depuis l'opérateur — NC109M en l'écrivant tel quel, NC204M via un mnémonique que `SPECIAL-NAMES` associe au périphérique d'entrée. Le validateur est censé fournir l'entrée ; sans stdin, toutes les comparaisons échouent. | Le banc d'essai fournit un jeu d'entrées de l'opérateur sur le stdin du processus fils. Le jeu est **retrouvé dans le source, pas inventé** : chaque élément accepté est comparé à un élément apparié dont le programme fixe la valeur juste au-dessus de l'`ACCEPT`, si bien que chaque ligne du jeu est cette valeur. |

Il n'y a donc **aucun plafond structurel en dessous de 95** sur l'axe exécution :
tous les programmes NC du périmètre compilent, et chacun d'eux est noté sur ce
qu'il rapporte lui-même.

Le cas comparable qui, lui, **a** été tranché est celui du commutateur externe.
NC174A, NC253A et NC254A testent `ON STATUS` / `OFF STATUS` face à un commutateur
que l'opérateur positionne avant l'exécution — rien à l'intérieur de COBOL ne
peut en positionner un — aussi le banc d'essai passe-t-il désormais
`--switch XXXXX051=ON --switch XXXXX052=OFF` (ainsi que les graphies substituées
`SWITCH-1` / `SWITCH-2`) exactement comme l'exigent les instructions d'exécution
de CCVS85. C'est une configuration réclamée par la procédure de validation, pas
un pouce sur la balance : un programme qui ne déclare aucun commutateur n'est pas
affecté.

#### ⚠️ Ce que PASS veut vraiment dire — à lire avant de citer le chiffre

Un programme compte comme **PASS** lorsqu'il traverse le front end de RustCOBOL —
lexer, parser, analyseur sémantique — avec **zéro erreur**, en utilisant
`--source-format=fixed`.

C'est de la conformité de *compilation*. Ce **n'est pas** la preuve que le
programme calcule la bonne réponse. Un programme CCVS85 imprime aussi son propre
décompte `PASS`/`FAIL` quand il s'exécute, et noter cette sortie est l'**étape
suivante** de ce travail — elle n'est pas incluse dans les 332 — voir le tableau
de bord d'exécution ci-dessous. Deux cas mesurés montrent pourquoi la distinction
compte :

- 30 des 35 programmes de fichiers RELATIVE compilent proprement, et le runtime
  **n'a aucun moteur RELATIVE** — ils s'exécuteraient et produiraient des
  résultats faux silencieusement.
- Un littéral continué sur deux lignes peut être réassemblé de travers et
  s'analyser malgré tout, laissant le programme avec les mauvaises données.

Donc : **PASS = « RustCOBOL accepte toutes les constructions de ce programme. »**
Rien de plus, pour l'instant.

#### 🔴 Le tableau de bord d'exécution — le chiffre qui veut dire « ça marche »

Tout ce qui précède mesure la **compilation**. Un programme CCVS85 *s'exécute*
aussi et imprime son propre décompte `PASS`/`FAIL`, et c'est ce décompte que la
suite existe pour produire. Depuis 1.62.15 le banc d'essai les exécute :

```bash
cargo run -p cobolt-semantic --example nist_conformance -- run
```

Mesuré le 2026‑08‑28 en 1.62.43. Selon la RÈGLE D'OR nº 9, un module est terminé
avant que le suivant ne commence : **NC (Noyau) est complet sur les deux axes**,
si bien que **SQ (E/S séquentielles)** est le module en cours.

**NC — Noyau**

| | Programmes |
|---|---:|
| dans le périmètre | 95 |
| n'ont pas compilé | 0 |
| sont allés jusqu'au bout | 95 |
| **…signalant 0 échec** | **95** |
| …signalant des échecs | 0 |
| se sont exécutés sans imprimer de rapport | 0 |
| ont dépassé le délai (>20 s) | 0 |
| ont planté ou ont été refusés par le runtime | 0 |

Les assertions que les programmes rapportent eux-mêmes : **4 614 PASS / 0 FAIL**,
100 % des 4 614 notées. (5 de plus sont `DELETED` — le marqueur propre à CCVS
pour un test que le programme lui-même saute.)

Par contraste, le même tableau en 1.62.23 affichait 65 propres sur 95,
4 278 PASS / 226 FAIL. C'est l'écart entre « compile » et « fonctionne » qui
s'est refermé.

**SQ — E/S séquentielles (en cours)**

| | Programmes |
|---|---:|
| dans le périmètre | 85 |
| n'ont pas compilé | 0 |
| sont allés jusqu'au bout | 83 |
| **…signalant 0 échec** | **84** |
| …signalant des échecs | 1 |
| se sont exécutés sans imprimer de rapport | 0 |
| ont dépassé le délai (>20 s) | 0 |
| sortie emballée (>2 MB) | 0 |
| ont planté ou ont été refusés par le runtime | 0 |

Assertions : **623 PASS / 1 FAIL**, 99.8 % des 624 notées, et **tous les
programmes vont jusqu'au bout**. En 1.62.42 le même tableau affichait **10**
propres sur 85, 20 plantés, 1 hors délai et 215 PASS / 190 FAIL — la grappe de
plantages n'était qu'un seul défaut, les paragraphes déclaratifs perdant leurs
noms ; en 1.62.43 il affichait 44 propres et 471 PASS / 162 FAIL. Les
enregistrements de longueur variable, la zone d'enregistrement partagée, les
largeurs de `FILLER`, `READ … INTO` et le `REWRITE` séquentiel sont arrivés en
1.62.44 ; le `USE` qualifié par mode, `CLOSE REEL/UNIT`, `SELECT OPTIONAL`,
`LINAGE-COUNTER` à l'`OPEN` et les longueurs d'enregistrement hors plage en
1.62.45 ; les valeurs de `LINAGE` données par nom de donnée et les détecteurs de
signalement des E/S séquentielles en 1.62.46.

Un membre reste en deçà :

| Membre | Ce qu'il manque |
|---|---|
| SQ203A | Requiert `XXXXD001`, un fichier de données que l'**installation** CCVS85 fournit. Aucun membre de la suite ne l'écrit, si bien que la moitié « fichier présent » de son test `SELECT OPTIONAL` ne peut pas s'exécuter ici ; la moitié « fichier absent » passe. C'est une entrée d'installation manquante, pas un défaut de RustCOBOL. |

> Une ligne de détail `FAIL*` est écrite **deux fois** à dessein — le
> `PRINT-DETAIL` de CCVS exécute
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — tandis que `PASS ` n'est
> écrit qu'une fois. Tout décompte brut de marqueurs tiré du fichier d'impression
> doit diviser les échecs par deux avant de vouloir dire quoi que ce soit.

Pour lire *pourquoi* un programme échoue, une troisième passe imprime le détail
d'échec que porte son propre rapport, prêt à être regroupé sur tout un module :

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> C'est pourquoi le chiffre de compilation est toujours rapporté comme
> « RustCOBOL **accepte** ces constructions ». Le citer comme un niveau de
> conformité serait faux.

#### Par module

| Module | Ce qu'il teste | PASS / Total | |
|---|---|---:|---|
| NC | Noyau | **95 / 95** | ✅ complet — et complet en **exécution** aussi (voir le tableau de bord ci-dessus) |
| SQ | E/S séquentielles | **85 / 85** | ✅ complet à la compilation ; **44 / 85 à l'exécution** — le module en cours |
| IC | Communication inter‑programmes | 45 / 47 | `END-CALL` atteint le répartiteur d'instructions au lieu d'être consommé par son `CALL` ; un nom-condition indicé |
| IF | Fonctions intrinsèques | **45 / 45** | ✅ complet |
| IX | E/S indexées | **42 / 42** | ✅ complet |
| SG | Segmentation | **13 / 13** | ✅ complet |
| ST | Tri / Fusion | 38 / 40 | `COLLATING SEQUENCE` / `ALPHABET` |
| RL | E/S relatives | 34 / 35 | ⚠️ **compile seulement — pas de moteur d'exécution.** `ORGANIZATION IS RELATIVE` s'analyse et n'est jamais pris en charge à l'exécution, si bien que cette ligne surévalue la capacité réelle. L'unique échec est un `ELSE` pendant |
| SM | Manipulation du texte source (COPY/REPLACE) | 14 / 17 | un `$` à l'intérieur d'un nom de donnée ; du pseudo-texte qualifié/indicé ; une forme de `PERFORM … VARYING` |
| DB | Débogage | 11 / 15 | `GO-TO` employé comme mot défini par l'utilisateur, en collision avec la paire de mots-clés `GO TO` ; un programme utilise le verbe de Communication `DISABLE` |
| **Dans le périmètre** | | **422 / 434** | |
| CM | Communication | — | ⬜ N/A |
| RW | Report Writer | — | ⬜ N/A |
| OBSQ / OBIC / OBNC | Signalement des fonctions obsolètes | — | ⬜ N/A |
| EXEC85 | Le programme pilote COBOL propre au NIST | — | ⬜ N/A |

### ⬜ N/A — ce qui est hors du périmètre de RustCOBOL, et pourquoi

Ces 25 programmes **ne sont pas comptés comme des échecs**. Ce sont des
fonctionnalités que RustCOBOL n'implémente pas et n'a pas l'intention
d'implémenter. Le raisonnement complet est dans
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Module | Programmes | Pourquoi c'est hors périmètre |
|---|---:|---|
| **CM** — Communication | 9 | `COMMUNICATION SECTION`, entrées `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Vise les moniteurs de télétraitement des années 1980 — des files de messages détenues par un gestionnaire de transactions. Il n'existe pas de tel runtime ici, et le module a été retiré des normes COBOL ultérieures. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entrées `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, ruptures de contrôle. Un vaste sous-langage déclaratif ; la réponse de PowerRustCOBOL aux états est le Concepteur de formulaires et l'export PDF. Cela pourrait devenir une *fonctionnalité* plus tard si on le souhaite — c'est la seule exclusion à réelle valeur pour l'utilisateur. |
| **OBSQ / OBIC / OBNC** | 9 | Ceux-ci retestent des modules antérieurs et attendent du compilateur qu'il *signale* les éléments obsolètes de COBOL‑85. Leur contenu de langage est couvert par les spécifications du périmètre ; c'est le **signalement** des fonctions obsolètes qui est hors périmètre. |
| **EXEC85** | 1 | Ce n'est pas un test. C'est l'exécutif COBOL propre au NIST qui découpe la distribution et pilote la suite — remplacé ici par un banc d'essai en Rust, il n'a donc pas besoin de compiler. |

**Le COBOL orienté objet** est lui aussi hors du périmètre de RustCOBOL, mais
CCVS85 lui est entièrement antérieur — il n'y a aucun programme OO dans la suite.

### D'où viennent les 192 échecs restants

Chacun est un défaut spécifié, pas une inconnue. Classés par le nombre de
programmes dont il est la *première* erreur :

| Programmes | Cause racine | Spécification |
|---:|---|---|
| 31 | virgule séparatrice — `MOVE ZERO TO A, B, C` | [séparateurs](../specs/nist/NIST-spec-separators.md) |
| 15 | `FUNCTION MAX(TBL(ALL))` | [intrinsèques](../specs/nist/NIST-spec-intrinsic-function-gaps.md) |
| 12 | `WHEN -0.000020 THRU 0.000020` | [manques d'instructions](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 11 | indices séparés par des espaces — `TBL (1  2)` | [séparateurs](../specs/nist/NIST-spec-separators.md) |
| 10 | `SET SW-1 TO ON` (noms de commutateur) et `SET A, B, C TO 1` | [special‑names](../specs/nist/NIST-spec-special-names.md), [séparateurs](../specs/nist/NIST-spec-separators.md) |
| 9 | `CLOSE … WITH LOCK` / `WITH NO REWIND` | [manques d'instructions](../specs/nist/NIST-spec-statement-grammar-gaps.md) |
| 7 | `COPY` enfoui dans la zone B ou réparti sur plusieurs lignes | [COPY/REPLACE](../specs/nist/NIST-spec-copy-and-replace.md) |
| 5 | point-virgule séparateur — `START F ; INVALID KEY` | [séparateurs](../specs/nist/NIST-spec-separators.md) |
| 4 | entier d'`OCCURS` sur la ligne suivante | [séparateurs](../specs/nist/NIST-spec-separators.md) |
| 4 | `SECTION` avec un numéro de priorité — `SORT-PARA SECTION 69.` | [segmentation](../specs/nist/NIST-spec-segmentation.md) |

> **Le classement bouge après chaque correctif, et ces mouvements sont
> instructifs.** Trois lignes qui menaient ce tableau dans des versions
> antérieures ont disparu — les entrées de commentaire d'IDENTIFICATION, les
> littéraux numériques et le guillemet égaré. À chaque fois, la plupart des
> programmes de la ligne libérée ne se sont **pas** mis à passer ; ils ont glissé
> vers la ligne du dessous. Les quatre programmes SG libérés en 1.62.12 butent
> aujourd'hui sur `SORT-PARA SECTION 69.`, ce qui explique que la Segmentation
> affiche toujours 0 / 13. Remesurez plutôt que de vous fier à un classement
> précédent.

### Historique de conformité

| Version | PASS / 434 | Ce qui a changé |
|---|---:|---|
| 1.62.7 | **0** | Rien ne compilait. Deux règles du format de référence classique manquaient : les colonnes 73‑80 étaient lues comme du source, et les lignes de continuation n'étaient jamais raccordées. |
| 1.62.8 | **222** | `--source-format=fixed` — le format de référence classique, continuation comprise. Voir [Formats de source](#formats-de-source). |
| 1.62.10 | **237** | Les littéraux numériques peuvent commencer par un point décimal (`.999`). Fonctions intrinsèques 21 → 29, Noyau 25 → 29, Tri/Fusion 27 → 30. |
| 1.62.11 | 241 | Les paragraphes d'entrée de commentaire d'IDENTIFICATION. Débogage 5 → 9. Un gain plus modeste que ne le suggère le lot de 32 programmes : 9 d'entre eux sont des programmes de Communication (N/A), et la plupart des autres butaient sur un second blocage aussitôt après. |
| 1.62.12 | 242 | Un littéral est confiné à sa ligne, si bien qu'un guillemet égaré ne peut plus inverser la parité d'un fichier entier. Noyau 29 → 30. Le lot de 6 programmes s'est vidé : 4 sont passés aux numéros de priorité de segment, 1 passe désormais. |
| 1.62.13 | 292 | La virgule et le point-virgule séparateurs sont de la ponctuation, pas des jetons ; les indices peuvent être séparés par de simples espaces ; un indice peut suivre un nom qualifié complet ; un délimiteur doublé à l'intérieur d'un littéral vaut un caractère. Noyau 30 → 56, Inter-programmes 32 → 44, Indexées 31 → 38. Trois lots de diagnostics entiers se sont vidés. |
| 1.62.14 | 317 | `FUNCTION MAX(TBL(ALL))` — une table entière en argument d'intrinsèque ; `MOVE ALL "X"` remplit le champ ; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL` ; un littéral signé comme objet de `WHEN` ; `PERFORM … TIMES` avec un compte porté par un élément de données ; un compte entier écrit sur une ligne de continuation. **Fonctions intrinsèques 45 / 45 — module complet.** |
| 1.62.15 | 332 | Un nom de `FUNCTION` inconnu est une erreur de compilation au lieu de renvoyer 0 ; un mot défini par l'utilisateur peut commencer par un chiffre (`25COUNT`, `3-DEM-TBL`, `0 SECTION.`) ; une ligne `D` est un commentaire sauf avec `WITH DEBUGGING MODE`. Segmentation 0 → 10, Noyau 58 → 61. |
| 1.62.16 | 376 | L'`AT` d'`AT END` est facultatif, si bien qu'une clause `END` seule n'avale plus l'en-tête de paragraphe suivant (33 programmes). Le préprocesseur COPY/REPLACE confine un littéral à sa ligne, si bien que le mot COPY dans le bandeau de copyright n'est pas une directive. Un littéral numérique peut ouvrir par son point décimal une liste d'opérandes d'`ADD`/`SUBTRACT`. **E/S indexées complètes, 42 / 42.** |
| 1.62.17 | 380 | La mise en page `LINAGE`, le `LINAGE-COUNTER` et `WRITE … AT END-OF-PAGE` / `AT EOP` — implémentés, pas simulés. E/S séquentielles 77 → 81. |
| **1.62.19** | **396** | Un élément numeric-edited est un élément numérique. Le point décimal d'édition conserve le chiffre qui le suit (`PIC ZZ,ZZZ.9` ne tronque plus en `ZZ,ZZZ`), et une picture bâtie uniquement de caractères d'édition — `ZZZZ`, `$.**`, `$**.**CR` — est numeric-edited et non alphanumérique. Ces deux points faisaient paraître non numérique un récepteur `GIVING` arithmétique pourtant licite. |
| **1.62.18** | **391** | Un nombre ouvrant une ligne de continuation est un opérande là où une expression est attendue. L'`IS` est facultatif dans une condition de classe ou de signe, et une condition peut être sujet d'`EVALUATE`. Un nom de procédure peut s'écrire entièrement en chiffres, aussi bien dans les références que dans les en-têtes. |
| **1.62.21** | **417** | La passe Noyau. `ALTER` est une série et `GO TO.` est le GO TO altéré ; un nom de procédure tout en chiffres garde ses zéros de tête ; un nom-condition peut être indicé ou qualifié ; une expression arithmétique parenthésée est un opérande, pas une condition imbriquée ; `MULTIPLY`/`DIVIDE` en format 1 acceptent une série de récepteurs ; `WITH TEST` peut précéder `VARYING` et un compte de répétitions peut être indicé ; `PERFORM impératif … END-PERFORM` n'a besoin d'aucune clause ; un nom de paragraphe peut être qualifié par sa section ; l'`ELSE` n'est avalé ni par un impératif d'`ON SIZE ERROR` ni par une branche ELSE imbriquée ; les relations combinées abrégées acceptent des objets arithmétiques et de classe/signe ; `INSPECT` reporte sa catégorie ALL/LEADING d'un opérande à l'autre et `CONVERTING` accepte une région ; `UNSTRING TALLYING` vient après `WITH POINTER`. **Noyau 76 → 92 sur 95 à la compilation, 16 → 28 à l'exécution propre.** |
| **1.62.43** | **422** | **Le module E/S séquentielles compile intégralement — 85 sur 85 — et passe de 10 à 44 sur 85 à l'exécution.** Les paragraphes d'un déclaratif gardent leurs noms, si bien qu'un gestionnaire `USE` peut leur appliquer `PERFORM` et `GO TO` (20 programmes ont cessé de planter) ; un élément `FILE STATUS` déclaré comme *groupe* de deux caractères reçoit le code ; l'`OPEN` d'un fichier déjà ouvert vaut `41` et ne le rouvre pas ; un `READ` séquentiel après `AT END` vaut `46` ; et un même `OPEN` peut porter plusieurs groupes de mode (`OPEN INPUT f1 OUTPUT f2`), ce qui constitue tout le gain de compilation. |
| **1.62.42** | **420** | **Le module Noyau est terminé — 95 sur 95 à la compilation *et* 95 sur 95 à l'exécution propre, 4 614 assertions sans aucun échec.** Un `66 RENAMES` est qualifié par son enregistrement, couvre toutes les occurrences d'une table qu'il enjambe, et est l'élément qu'il renomme lorsqu'il n'en renomme qu'un ; un 88 déclaré sur un groupe teste les octets du groupe ; une constante figurative est dimensionnée d'après l'autre opérande, `VALUE` compris ; un opérande de groupe est de catégorie alphanumérique ; un `NOT` devant l'objet d'une abréviation nie la relation ; une série `INSPECT … REPLACING` partage un seul balayage et un élément DISPLAY signé n'a pas de `-` parmi ses caractères ; les recouvrements de `REDEFINES` s'imbriquent ; et `PERFORM … WITH TEST AFTER VARYING` est honoré, une variable `AFTER` est réinitialisée à la fin de sa boucle, et un identifiant `VARYING` indicé suit son indice. C'est ce dernier groupe qui explique que NC201A soit allé au bout. |

> **Le résumé honnête.** RustCOBOL accepte aujourd'hui **97.2 %** de la suite NIST
> du périmètre, contre rien du tout il y a neuf versions. Les 12 restants ne sont
> pas mystérieux — ce sont des défauts nommés, chacun spécifié avec les
> programmes qu'il bloque. Ce tableau est la mesure du progrès, et il est mis à
> jour à chaque version.
>
> **Et un module est terminé sur l'axe qui compte.** Le Noyau exécute 95
> programmes propres sur 95, il ne se contente pas de les compiler — voir le
> tableau de bord d'exécution ci-dessus. Selon la RÈGLE D'OR nº 9, c'est le seuil
> qui autorise à démarrer le module suivant, aussi **les E/S séquentielles sont
> désormais en cours** : complètes à la compilation, 44 sur 85 à l'exécution.

---

> **Mise à jour (passe d'implémentation des manques) :** les éléments suivants ont
> été implémentés et sont désormais ✅ — la **modification de référence**
> `id(start:len)`, le **`PERFORM n TIMES` en ligne**, **`SET … UP/DOWN BY`**,
> **STRING/UNSTRING `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**,
> l'**`INITIALIZE` sensible à la catégorie**, les **conditions abrégées préfixées
> par l'opérateur** (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (s'exécute sur un
> CALL non résolu), les **récepteurs multiples de `COMPUTE` + `ROUNDED` par
> récepteur**, et un **jeu de fonctions intrinsèques** bien plus large.
>
> **Mise à jour (passe d'environnement hiérarchique / sensible aux occurrences —
> 1.5.0) :** quatre fonctionnalités bloquées par le modèle de données sont
> désormais ✅ — l'**indiçage de table à l'exécution** `t(i)` / `t(i, j)`
> (stockage par occurrence), la **désambiguïsation par nom qualifié**
> `id OF/IN group` (des noms de feuille dupliqués se résolvent vers des stockages
> indépendants), **`MOVE/ADD/SUBTRACT CORRESPONDING`**, et des **`SEARCH` /
> `SEARCH ALL` fonctionnels**.
>
> **Mise à jour (passe de complétude des verbes — 1.6.0) :** sont aussi ✅
> désormais — **`MULTIPLY`/`DIVIDE GIVING` à récepteurs multiples + `ROUNDED` par
> récepteur** sur `ADD`/`SUBTRACT` ; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** et l'`EXIT` simple corrigé ; **`CALL … NOT ON EXCEPTION`** ;
> **`INSPECT … TALLYING … REPLACING`** combiné et les régions
> **`BEFORE/AFTER INITIAL`** ; les **intrinsèques** de date/finance
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`) ; les **conditions abrégées à objet littéral**
> (`A = 1 OR 2 OR 3`) ; **`EVALUATE … ALSO`** (multi-sujets) et **`WHEN NOT`** ;
> les **vrais noms-conditions de niveau 88** (`SET … TO TRUE/FALSE`, l'hôte est
> testé face à ses VALUE/plages) ; **`PERFORM para VARYING`** ; et un runtime
> **`SORT`/`MERGE`** fonctionnel (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). La liste des éléments à éviter, en bas, est à jour.
>
> **Mise à jour (passe de résorption de la liste à éviter — 1.7.0) :** les manques
> restants sont désormais implémentés — l'**abréviation à objet identifiant**
> (`a = b OR c`, résolue via les métadonnées de niveau 88) ;
> **`INITIALIZE … REPLACING category DATA BY value`** ; **`66 RENAMES`** (la
> lecture synthétise / l'écriture répartit sur les éléments couverts) ; les
> **pointeurs** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`, l'aliasing
> par `SET ADDRESS OF item TO …`, `IF ptr = NULL`) ; **`ALTER`** / **`UNLOCK`** ;
> un **`NEXT SENTENCE`** fidèle ; les **intrinsèques** standard restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`) ; et
> l'**`ACCEPT`/`DISPLAY` écran** étendu (`AT`/`WITH` via ANSI en mode CLI —
> désormais *exécuté*, et pas seulement analysé).
>
> **Mise à jour (1.7.1) :** les sources de registre d'`ACCEPT` sont désormais
> fonctionnelles (c'étaient des no-ops reconnus) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (appariés avec
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (apparié avec
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Mise à jour (1.7.2) :** les clauses de partage / verrouillage de fichiers et
> `CANCEL` (c'étaient des ❌ / des no-ops) —
> **`OPEN … SHARING WITH … [WITH LOCK]`**, **`READ … WITH [NO] LOCK`**,
> **`UNLOCK`** (libère les verrous d'enregistrement INDEXED du fichier), et
> **`CANCEL program`** (réinitialise le stockage du programme).
>
> **Mise à jour (1.8.0) :** **`COMMIT` / `ROLLBACK`** sont désormais de vrais
> verbes COBOL — des transactions pilotées par le programme sur les fichiers
> INDEXED ouverts (dans le moteur mémoire comme dans le moteur disque). Le moteur
> disque a gagné un véritable journal d'annulation en cours d'exécution (c'était
> un no-op auparavant). La liste des éléments à éviter, en bas, est à jour.

---

## Paragraphes de l'IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Les paragraphes d'**entrée de commentaire** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — dans **n'importe quel ordre et
  n'importe quel sous-ensemble**.
- ✅ `REMARKS` est accepté lui aussi. Il a été supprimé de COBOL en 1985, il
  n'est donc pas conservé ; il est admis pour qu'un source repris de COBOL‑74
  compile encore.

Une **entrée de commentaire** est du texte libre, et COBOL‑85 l'entend au pied
de la lettre :

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Elle peut contenir des **mots réservés** — le `DATA` ci-dessus n'ouvre pas de
  DATA DIVISION.
- Elle peut contenir des **points**, et ne s'arrête pas à l'un d'eux.
- Elle **s'étend sur autant de lignes** que vous en écrivez.
- Elle se termine au prochain en-tête de paragraphe ou de division qui
  **commence une ligne** en zone A — c'est ainsi que l'entrée ci-dessus se
  termine à `DATE-WRITTEN`.

**Un guillemet dans cette prose reste confiné à sa ligne** (depuis 1.62.12). Un
texte tel que `THE COMPILER"S ABILITY` n'ouvre plus un littéral qui court dans
tout le reste du programme — voir [Formats de source](#formats-de-source). Il vaut
toujours mieux éviter un guillemet non apparié dans une entrée de commentaire,
mais cela vous coûte désormais cette ligne, pas le fichier.

⚠️ `INSTALLATION`, `SECURITY` et `REMARKS` **ne sont pas des mots réservés**
ici. Ils ne sont reconnus comme noms de paragraphe qu'à l'intérieur de
l'IDENTIFICATION DIVISION, si bien qu'un élément de données nommé `SECURITY`
continue de fonctionner.

---

## Formats de source

RustCOBOL lit trois dispositions de source. Le choix est explicite — il n'est
**jamais** deviné d'après le contenu du fichier, car appliquer des règles de
colonnes à un source qui n'a pas été écrit pour elles supprime du code en
silence.

| `--source-format` | Ce que cela signifie |
|---|---|
| `free` | Aucune règle de colonnes. `*>` ouvre un commentaire. **La valeur par défaut**, et ce qu'utilisent les projets de PowerRustCOBOL eux-mêmes ainsi que les fichiers `.cbl` de formulaire générés. |
| `fixed` | ✅ **Format de référence classique de COBOL-85** — la disposition définie par la norme, celle dans laquelle est écrit le source en image de carte. Voir ci-dessous. |
| `fixed-relaxed` | La zone séquence et la colonne indicatrice sont respectées, mais la ligne va aussi loin que vous l'avez saisie — pas de limite à 72 colonnes. |
| `auto` | Comportement historique : `free`, sauf si `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` fixe la valeur par défaut d'une session.

### `fixed` — le format de référence classique

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Colonnes 1-6** — zone du numéro de séquence, ignorée.
- **Colonne 7** — zone indicatrice :
  - `*` ou `/` → ligne de commentaire
  - `-` → **continuation** de la ligne précédente
  - `D` → ligne de débogage ; un commentaire (le mode débogage n'est pas encore
    implémenté)
  - tout le reste → lu comme du source ordinaire. La norme réserve cette
    colonne, mais les suites en image de carte s'en servent comme sélecteur de
    lignes optionnelles, et supprimer ces lignes en silence supprimerait du
    code.
- **Colonnes 8-72** — le source.
- **Colonnes 73-80** — zone d'identification, **écartée**.

### Lignes de continuation ✅

Un tiret en colonne 7 continue la ligne précédente.

**Continuer un mot ou un littéral numérique** — les espaces de fin de la ligne
continuée sont écartés et les deux moitiés se rejoignent sans rien entre elles :

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

déclare un seul élément nommé `WRK-DS-18V00-CONTINUED`.

**Continuer un littéral alphanumérique** — le littéral de la ligne continuée n'a
pas de guillemet fermant ; la ligne de continuation doit en rouvrir un, et le
littéral reprend au caractère qui le suit :

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **Le fragment continué va jusqu'à la colonne 72, espaces de fin compris.** Une
ligne qui s'arrête avant la colonne 72 apporte tout de même ces espaces au
littéral. C'est pourquoi un littéral continué n'est exact octet pour octet que
sous `fixed` ; les autres formats n'ont pas de colonne 72 où s'arrêter.

### Un littéral ne franchit jamais une ligne par accident ✅

La continuation est le **seul** moyen pour un littéral de s'étendre sur
plusieurs lignes. Un guillemet qui n'est pas fermé sur sa propre ligne est une
erreur, signalée là où il est écrit :

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Cela compte plus qu'il n'y paraît. Avant 1.62.12, un guillemet non apparié
courait jusqu'au guillemet *suivant* n'importe où dans le fichier : un seul `"`
égaré dans un commentaire avalait des divisions entières et décalait
l'appariement de tous les guillemets suivants — les programmes NIST où cela a
été trouvé ont un nombre **pair** de guillemets, donc rien n'était non terminé ;
un seul caractère avait décalé la parité du fichier entier. Les dégâts
s'arrêtent désormais au saut de ligne.

> **Le format libre n'a pas de continuation de littéral.** Ni `&` — c'est
> l'*opérateur* de concaténation — ni un bloc délimité. Un littéral en format
> libre doit tenir sur une seule ligne ; pour un littéral long, concaténez :
> `"first part" & "second part"`.

> **Note.** Choisir `fixed` pour un fichier écrit en format libre l'abîmera —
> tout ce qui dépasse la colonne 72 disparaît, et le texte avant la colonne 8 est
> lu comme un numéro de séquence. Ne l'utilisez que pour du source qui est
> vraiment en image de carte.

---

## Instructions reconnues (verbes)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirige le `GO TO` de para-1) ·
`UNLOCK file` (libère les verrous d'enregistrement du fichier) ·
`OPEN … SHARING/WITH LOCK` ·
`READ … WITH [NO] LOCK` (partage/verrouillage de fichiers — indicatif au sein de
l'unique unité d'exécution)
✅ `COMMIT` / `ROLLBACK` (transactions sur fichier INDEXED pilotées par le
programme — voir Verbes fichier) · `CANCEL` (réinitialise la mémoire du
programme) ·
⚠️ `INVOKE` (analysé comme une opération sans effet)
Extensions du projet : `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Un bloc peut faire `use` des crates toujours liées (std, egui, eframe
et l'ensemble du runtime lié) **plus toute crate que le projet enregistre dans
Project's Crates** (spéc. 044) : les crates enregistrées sont figées à une
version exacte, recopiées (vendoring) dans le `crates/` du projet et compilées
dans le binaire ; les crates non enregistrées font échouer Check/Build à la
ligne du développeur, avec le remède indiqué.

✅ `SEARCH` (séquentiel) / `SEARCH ALL` (recherche binaire sur une table à
`ASCENDING`/`DESCENDING KEY` — exécute le premier `WHEN` qui correspond, sinon
`AT END`).
✅ `SORT` / `MERGE` avec `RELEASE` / `RETURN` (fonctionnels — voir ci-dessous).
✅ `DECLARATIVES … END DECLARATIVES` avec `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — gestionnaires d'erreur fichier
déclenchés par un `FILE STATUS` d'erreur non traité. **On entre dans un
gestionnaire par le début de sa section et il s'exécute jusqu'à la fin de cette
section**, et ses paragraphes gardent leurs noms : il peut donc leur faire
`PERFORM` et `GO TO` — y compris à un paragraphe d'une *autre* section
déclarative. Les paragraphes déclaratifs vivent dans leur propre espace de noms :
le contrôle ne tombe jamais du corps principal dans ces paragraphes, et un nom
déclaré des deux côtés se résout vers la copie de la déclarative tant qu'un
gestionnaire s'exécute, et vers celle du corps partout ailleurs. Une déclarative
peut aussi faire `PERFORM` d'un paragraphe de la partie non déclarative.
❌ **Non reconnus — à ne pas utiliser :** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formes prises en charge par verbe

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (plusieurs récepteurs).
- ✅ **Un opérande de groupe rend tout le déplacement alphanumérique** (COBOL-85 6.18.4).
  La PICTURE de l'autre opérande n'apporte que sa *taille* : pas d'édition,
  pas de dés-édition, pas de conversion numérique. `MOVE <group holding "123ABC">`
  laisse `"123ABC "` dans un `PIC 0XXXXX0` (et non l'édité `"0123AB0"`), les mêmes
  six caractères et une espace dans un `PIC 9999V999`, et `"12"` dans un `PIC 99`.
  `JUSTIFIED RIGHT` décide toujours quel bout est complété et quel bout est perdu.
  La même règle régit les octets propres d'un groupe : chaque enfant prend sa
  tranche telle quelle, si bien qu'un enfant alphanumérique édité n'est **pas** réédité.
- ✅ **Une clause `VALUE` sur un groupe** initialise les octets du groupe et se
  répartit entre ses enfants — `01 G VALUE "$123.45". 02 E PIC $999.99.`
  laisse `E` contenant `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — déplace chaque élément subordonné que les deux
  groupes partagent par le nom, en descendant récursivement dans les sous-groupes qui concordent.
- ✅ **`CORRESPONDING` exclut un élément décrit avec `REDEFINES` ou `RENAMES`**
  (COBOL-85 6.18.4 GR1), de l'un ou l'autre côté, ainsi que tout ce qui lui est
  subordonné. L'exclusion porte sur la *déclaration*, pas sur le nom : un élément ordinaire qui
  partage seulement son nom avec un niveau 66 déclaré ailleurs correspond toujours.
- ✅ **L'un ou l'autre opérande de `CORRESPONDING` peut nommer une occurrence d'une
  table de groupes** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` écrit dans les
  emplacements propres à cette occurrence, et l'indice est transporté tout au long de la récursion.
- ✅ **Il suffit qu'UN des deux éléments d'une paire soit élémentaire.** Un groupe peut
  faire face à un élément élémentaire, et le déplacement entre eux est alphanumérique : un
  élément élémentaire `PIC XXX` qui émet vers un groupe de `999` + `XXX` remplit ses six
  caractères, et un groupe de `XXX` + `99` qui émet vers un simple `X(5)` le remplit.
  Deux groupes face à face **récursent** toujours — cet appariement n'est pas le cas
  élémentaire. *(Avant 1.62.39 aucun des deux sens ne déplaçait quoi que ce soit : un
  groupe ne possède pas d'emplacement de stockage, l'écriture allait donc là où personne ne la relit et
  la lecture rendait la chaîne vide.)*
- ✅ **Modification de référence `id(start:len)`** — émetteur (sous-chaîne) et récepteur
  (affectation partielle insérée) ; fonctionne sur les opérandes de tous les verbes. `length` est facultatif.
  Elle adresse des **positions de caractère**, si bien qu'un opérande numérique est pris sur toute la
  largeur de sa `PIC` avec ses zéros de tête : `01 T PIC 9(8) VALUE 00224845` donne
  `T(1:2)` = `"00"`, et non `"22"`.
- ✅ **Les éléments de groupe sont des agrégats alphanumériques** — un groupe *est* ses éléments
  subordonnés mis bout à bout, et sa taille est la somme des leurs. En lire un
  concatène les enfants (y compris le `FILLER`) ; déplacer vers un groupe répartit les
  octets entre eux selon leur largeur. `MOVE 11 TO A` est visible à travers le groupe qui
  contient `A`, et `MOVE "1234" TO G` fixe les enfants de `G`, pas un emplacement qui lui serait propre.
- ✅ indices `t(i)`, `t(i, j)` — lisent/écrivent l'emplacement de stockage de chaque occurrence ;
  les indices variables `t(WS-I)` sont évalués à chaque accès.
- ✅ qualification `id OF/IN group` (`… OF g1 OF g2`) — se résout vers le bon
  élément même lorsque le nom de la feuille est déclaré sous plus d'un groupe.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` par récepteur** — chaque récepteur porte son propre indicateur `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine chaque paire numérique qui
  concorde, en descendant récursivement dans les sous-groupes qui concordent.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **plusieurs récepteurs `GIVING`**, chacun avec son propre `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sans `GIVING`) range `a/b` de nouveau dans `a` (une commodité de
  PowerRustCOBOL ; le COBOL standard exige ici `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **plusieurs récepteurs, chacun avec son propre `ROUNDED`**.
- ✅ opérateurs d'expression `+ - * /` et `**` (puissance, associative à droite), parenthèses,
  `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **`ALSO` à plusieurs sujets** — chaque colonne de `WHEN` est comparée par position
  à son sujet, et les résultats sont combinés par ET.
- ✅ **`WHEN NOT value`** nie un objet de sélection ; **`WHEN condition`**
  (p. ex. `EVALUATE TRUE WHEN a > b`) évalue la condition booléenne.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = littéral entier ou élément de données).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en ligne,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en ligne (sans paragraphe).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — exécute le paragraphe à chaque
  itération (hors ligne, sans `END-PERFORM`).
- ✅ **`WITH TEST AFTER` s'applique à `VARYING`**, écrit de part et d'autre de la
  phrase, en ligne comme hors ligne. Le corps s'exécute une fois avant que quoi que ce soit
  ne soit testé, puis les conditions sont testées **de la plus interne vers l'extérieur** ; le niveau dont
  la condition est fausse est augmenté, tous les niveaux intérieurs repartent de leur valeur `FROM`,
  et le corps s'exécute de nouveau. Une variable n'est augmentée que lorsque son test
  ressort faux, si bien que le test qui met fin à la boucle la laisse telle que le corps l'a laissée.
- ✅ **Une variable de `AFTER` est remise à sa valeur `FROM` quand sa boucle se termine**,
  avant que le niveau immédiatement supérieur ne soit augmenté (COBOL-85 6.20.4 GR10(d)). Après
  le `PERFORM` entier, les variables intérieures portent leurs valeurs `FROM` et seule la
  plus externe conserve la valeur qui y a mis fin.
- ✅ **Un identifiant de `VARYING` indicé suit son indice.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` augmente
  l'occurrence que `S1` sélectionne à cet instant, si bien qu'un corps qui fait avancer `S1`
  parcourt la table.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **Le qualificateur `{OF|IN} section` choisit de quelle copie il s'agit** lorsqu'un
  nom de paragraphe se répète dans plusieurs sections, exactement comme sur `PERFORM`. Une
  section **inconnue** se rabat sur la recherche non qualifiée plutôt que de perdre
  le saut. `GO TO … DEPENDING ON` prend une simple liste de noms et aucun qualificateur,
  et un `GO TO` qu'un `ALTER` a redirigé suit la redirection — laquelle nomme
  sa propre cible sans détour. *(Avant 1.62.39 le qualificateur était analysé puis
  ignoré, si bien que le saut tombait sur la première définition trouvée n'importe où dans le programme.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ le `EXIT` simple est un point de retour sans effet ; `EXIT PROGRAM` revient à l'appelant.
- ✅ `EXIT PERFORM [CYCLE]` (interrompre / poursuivre le PERFORM en ligne le plus proche),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfère le contrôle au-delà de la prochaine limite de phrase (l'
  analyseur insère des marques de limite à chaque point ; fidèle, pas un simple `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lit auprès de l'opérateur** quand `SPECIAL-NAMES` déclare
  le mnémonique (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — c'est le Format 1, identique à un `ACCEPT id` simple.
  Un nom qu'**aucune clause `SPECIAL-NAMES` ne déclare** garde l'extension
  PowerRustCOBOL et lit la **variable d'environnement** portant ce nom. Lequel des
  deux s'applique est décidé par la déclaration, jamais par l'orthographe.
  *(Avant 1.62.35 la clause ordinaire `<implementor-name> IS <mnemonic>` était
  sautée purement et simplement, si bien que chaque mnémonique lisait une variable d'environnement
  jamais définie et l'élément récepteur restait vide.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` positionne le curseur (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (toute la ligne de commande) · `FROM ARGUMENT-NUMBER` (nombre d'arguments)
  · `FROM ARGUMENT-VALUE` (l'argument au pointeur fixé par `DISPLAY n UPON
  ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` / `FROM ENVIRONMENT-VALUE` (la
  variable nommée par `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY`
  → `"00"` · `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` clôt l'instruction (facultatif).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` clôt la liste d'opérandes (facultatif), si bien que
  `DISPLAY A END-DISPLAY DISPLAY B` fait deux instructions et non une seule.
- ✅ formes écran `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — exécutées par positionnement de
  curseur ANSI + SGR en **mode CLI** (`rcrun`) ; ignorées en mode GUI (le concepteur
  de formulaires y remplace les E/S SCREEN). `ACCEPT id AT …` positionne puis lit.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Débordement = la chaîne assemblée est plus large que le champ récepteur.
- ✅ **Une phrase `DELIMITED BY` régit toute la série d'émetteurs qui la précède**,
  et pas seulement celui après lequel elle est écrite :
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` délimite les trois et
  construit `"ABC"`. Une instruction peut porter plusieurs phrases, chacune régissant les
  émetteurs depuis la précédente ; les émetteurs qui suivent la dernière phrase sont pris en entier.
  *(Avant 1.62.40 seul l'émetteur écrit immédiatement avant la phrase était
  délimité.)*
- ✅ **`INTO` un élément de groupe** répartit entre les éléments subordonnés du groupe.
- ✅ **Le résultat est assemblé octet par octet**, si bien que `STRING HIGH-VALUE` déplace
  l'unique octet `0xFF` et occupe une position de caractère.
- ✅ **Extension — `DELIMITED BY` par défaut intelligent** (lorsqu'aucune phrase ne régit un
  opérande) : les éléments alphanumériques `PIC X`/`A` prennent `SPACES` par défaut (le remplissage
  de fin est abandonné) ; les littéraux chaîne, les éléments numériques, numériques édités, les résultats de
  `FUNCTION` et les expressions prennent `SIZE`. Les éléments de données sont déplacés sous leur forme de champ
  (numérique → chiffres sur toute la largeur de la PIC ; numérique édité → caractères édités).

### UNSTRING
- ✅ `UNSTRING src [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Débordement = plus de champs source que de
  récepteurs.

### INSPECT
- ✅ `INSPECT id CONVERTING from TO to`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **les deux moitiés sont appliquées**.
- ✅ `BEFORE/AFTER INITIAL` confine chaque phrase à une sous-région du champ.
  (TALLYING cumule sur le compteur, comme le veut COBOL.)
- ✅ **Une série d'opérandes TALLYING partage UN SEUL balayage de gauche à droite** (COBOL-85
  6.17.3). À chaque position de caractère, les opérandes sont essayés dans l'ordre où
  ils ont été écrits ; le premier qui concorde prend la position et le balayage reprend
  au-delà des caractères qu'il a consommés. Ainsi `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"`
  sur `"AABA"` donne `t1 = 1, t2 = 1` — écrire les opérandes dans l'autre ordre
  donne `t1 = 3, t2 = 0`. `LEADING` doit concorder depuis le bord gauche de sa fenêtre sans
  interstice, si bien qu'un opérande antérieur qui prend cette position met fin à la série avant qu'elle ne commence,
  et `CHARACTERS` ne compte que les positions qu'aucun opérande antérieur n'a réclamées.
- ✅ **Une série d'opérandes REPLACING partage elle aussi UN SEUL balayage**, selon la même règle :
  le premier opérande qui concorde à une position remplace ces caractères et le
  balayage reprend au-delà d'eux, si bien qu'aucun opérande ultérieur ne peut les voir. La fenêtre
  `BEFORE`/`AFTER` de chaque opérande est fixée **avant tout remplacement**, ce qui est ce qui
  permet d'ancrer un opérande sur des caractères qu'un opérande antérieur écrase :

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Appliquée un opérande à la fois, la première phrase effacerait le `"L "` sur lequel la
  seconde est ancrée, et `"BAD"` survivrait.
- ✅ **Un élément DISPLAY signé n'a aucun `-` parmi ses positions de caractère.** Le
  signe opérationnel est une surperforation sur un chiffre, si bien que
  `INSPECT <PIC S9(5) holding -12345> TALLYING c FOR ALL "-"` donne **0** alors que
  `FOR ALL "5"` donne 1. Le signe est rétabli ensuite, si bien qu'un `REPLACING` sur
  les chiffres le laisse tranquille. `SIGN IS … SEPARATE CHARACTER` est le cas où le
  signe *est* une position, et là il est compté.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilé en MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encodé en ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` met dans l'élément hôte la première VALUE de la condition ;
  `TO FALSE` met une valeur hors de l'ensemble des VALUE (au mieux — il n'y a pas de clause FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` et
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — voir **Pointeurs** ci-dessous.

### INITIALIZE
- ✅ `INITIALIZE id …` — sensible à la catégorie : numérique / numérique édité → ZERO,
  tout le reste → SPACES, en descendant récursivement dans les éléments de groupe.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — met chaque élément
  subordonné de cette catégorie à la valeur ; les autres restent intacts.

### Pointeurs (USAGE POINTER)
- ✅ `USAGE POINTER` déclare un pointeur (NULL au départ).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — fait de `id` un alias du
  stockage de la cible (les lectures **et** les écritures suivent l'alias) ; typiquement un enregistrement
  de LINKAGE. `IF ptr = NULL` fonctionne.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ Le corps de `ON EXCEPTION` / `ON OVERFLOW` s'exécute quand le programme appelé n'est
  pas résolu ; le corps de `NOT ON EXCEPTION` s'exécute quand l'appel **se résout**.
- ✅ `CANCEL program …` réinitialise la WORKING-STORAGE du programme nommé, de sorte que son
  prochain `CALL` reparte à neuf.

### Verbes de fichier (les phrases prises en charge — la couverture complète est dans la suite d'E/S fichiers)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]` ; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` sont analysés et honorés là où cela a un sens — ils sont
  indicatifs dans le modèle à unité d'exécution unique.)
- ✅ **Un seul `OPEN` peut porter plusieurs groupes de mode**, chacun avec ses fichiers :
  `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Chaque groupe est ouvert dans son propre
  mode ; `SHARING` / `WITH LOCK` / `REGISTERED USER` s'appliquent à toute l'instruction.
- ✅ **Un `OPEN` d'un fichier déjà ouvert vaut `41`**, et le fichier reste tel
  qu'il était — l'instruction ne le rouvre **pas**. (Rouvrir un fichier `OUTPUT`
  tronquerait en silence ce que le programme avait déjà écrit.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extension
  PowerRustCOBOL) — consigne l'opérateur/utilisateur dans le journal d'observabilité INDEXED
  (champ `user=` sur chaque ligne d'événement de la session de ce fichier). Purement
  observationnel ; sans authentification ni autorisation. Voir
  [`observability-fr.md`](observability-fr.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libère le verrou d'enregistrement que le moteur INDEXED prend en I-O.
- ✅ **`READ … INTO id` est le `READ` suivi d'un `MOVE` de groupe.** L'enregistrement est
  réparti entre les éléments subordonnés du récepteur selon leur largeur et coupé à la
  largeur du récepteur lui-même, le récepteur peut être indicé, et le déplacement transporte des
  octets — un enregistrement contenant un octet qui n'est pas un caractère arrive intact.
- ✅ **Clause `RECORD` de la FD — enregistrements de longueur variable.** Les trois écritures :
  `RECORD CONTAINS n CHARACTERS` (fixe), `RECORD CONTAINS n TO m CHARACTERS`
  (variable ; la description d'enregistrement que le `WRITE` nomme donne la longueur), et
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]`
  (l'élément de données *est* la longueur — fixé avant un `WRITE`, remis par un `READ`,
  et ramené dans la plage déclarée). Une FD dont les enregistrements `01` diffèrent en taille est
  de longueur variable, qu'elle le dise ou non. Un fichier de longueur variable range la longueur de
  chaque enregistrement avec l'enregistrement, si bien que ses octets **ne** sont **pas** interchangeables avec
  ceux d'un fichier de longueur fixe ; un fichier de longueur fixe est inchangé.
- ✅ **Les enregistrements `01` d'une FD décrivent une seule zone d'enregistrement.** Un `READ` livre les
  octets à travers toutes les descriptions d'enregistrement ; un `WRITE` envoie toute la zone, si bien que ce
  qu'une autre description d'enregistrement a mis là où celui qui est écrit a du `FILLER`
  transparaît.
- ✅ **`FILLER` occupe ses octets dans un enregistrement de FD**, et
  `SIGN IS SEPARATE CHARACTER` rend un élément DISPLAY signé plus large d'un caractère
  que ses positions de chiffres.
- ✅ **Le `LINAGE` de la FD accepte des noms de données aussi bien que des entiers** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. La page est
  mesurée d'après ces éléments à chaque `WRITE`, si bien qu'un programme peut la redimensionner pendant
  son exécution. `LINAGE-COUNTER` vaut un à l'ouverture du fichier.
- ✅ **Un `READ` séquentiel après `AT END` vaut `46`, pas un second `10`.** Le
  `AT END` n'a laissé aucun enregistrement suivant valide, si bien que continuer à lire est une erreur différente de
  celle d'atteindre la fin. `46` est un statut de classe 4, donc ni `AT END` ni
  `NOT AT END` ne s'exécutent pour lui — c'est la déclarative `USE` du fichier qui le traite.
  Un nouvel `OPEN`, ou un `START` réussi, rétablit un enregistrement.
- ✅ `UNLOCK f [RECORD[S]]` libère les verrous d'enregistrement du fichier.
- ✅ **`COMMIT` / `ROLLBACK`** — transactions pilotées par le programme sur **tous** les
  fichiers INDEXED ouverts. `OPEN` commence une transaction ; `COMMIT` confirme les
  `WRITE`/`REWRITE`/`DELETE` en attente (un `ROLLBACK` ultérieur ne peut plus les défaire) et
  en commence une nouvelle ; `ROLLBACK` défait toute modification depuis le dernier `COMMIT`/`OPEN`.
  Le stockage **DISK** rend `COMMIT`/`CLOSE` durables sur disque. Le stockage **MEMORY**
  garde `COMMIT`/`ROLLBACK` purement en RAM (n'écrit jamais sur disque) ; un fichier
  `STORAGE IS MEMORY` simple est éphémère, et `STORAGE IS MEMORY WITH PERSISTENCE`
  n'enregistre sur disque qu'au `CLOSE`. (La reprise après panne par un journal d'écriture
  anticipée durable reste à faire — il s'agit ici d'un retour arrière au niveau du programme, en cours d'exécution.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (fichiers INDEXED ; extension PowerRustCOBOL). Le stockage par défaut est
  `DISK`. `WITH COMPRESSION` compresse l'enregistrement stocké (les clés sont évaluées sur
  l'enregistrement non compressé) ; `WITH PERSISTENCE` (MEMORY seulement) enregistre au `CLOSE` le fichier
  qui est en RAM. `OPEN OUTPUT` (re)crée toujours le conteneur sur disque.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]` ;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` sur un fichier SEQUENTIAL d'enregistrements** remplace sur place l'enregistrement que le
  dernier `READ` a livré, et laisse la position de lecture là où elle était — le
  `READ` suivant donne toujours l'enregistrement qui suit. Les statuts qu'il doit :
  **`49`** quand le fichier n'est pas ouvert en `I-O`, **`43`** quand aucun `READ` réussi
  n'a établi d'enregistrement (y compris après `AT END`, et lors d'un second `REWRITE` sans
  `READ` entre les deux), et **`44`** quand le nouvel enregistrement n'a pas la même longueur que
  celui qui a été lu — sur un fichier `DEPENDING ON` la valeur de l'élément est cette longueur, et c'est ainsi
  qu'un programme en demande une autre.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ Le partage de fichiers entre *processus* n'est pas imposé (unité d'exécution unique) ; les
  phrases `SHARING`/`LOCK` sont analysées et les verrous d'enregistrement par exécution du moteur
  INDEXED sont honorés.

### SORT / MERGE / RELEASE / RETURN  ✅ (fonctionnel, tampon de travail en mémoire)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dans une INPUT PROCEDURE) ajoute à l'exécution ;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` restitue les enregistrements.
- Les enregistrements sont triés de façon stable selon les clés déclarées (`ASCENDING`/`DESCENDING`) ;
  `USING` lit / `GIVING` écrit les fichiers séquentiels nommés.

---

## Conditions (IF / EVALUATE / PERFORM UNTIL)

- ✅ Symboles relationnels : `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relations en toutes lettres : `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN] [OR EQUAL
  TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe : `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Un élément dont la PICTURE ne porte **aucun signe opérationnel** n'est
  `NUMERIC` que si chaque position de caractère contient un chiffre — un
  `PIC X(5)` contenant `"+1234"`, `"1.234"` ou `"12 45"` n'est **pas** numérique.
  *(Avant 1.62.40, le test analysait les caractères comme un nombre : un signe,
  un point décimal, un exposant et les espaces alentour étaient donc tous
  acceptés.)*
- ✅ **L'opérande d'une `CLASS` définie par l'utilisateur peut être une position
  ordinale** — `CLASS ORDINAL-A-ONLY IS 66` désigne le 66e caractère du jeu
  natif — et cet opérande peut occuper sa propre ligne de source. Il en va de
  même pour `ALPHABET`.
- ✅ Signe : `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nom-condition de niveau 88 (le nom seul en guise de condition).
- ✅ **`TRUE` / `FALSE` comme opérandes** (extension PowerRustCOBOL) — du sucre
  syntaxique pour `1` et `0`, partout où une valeur est admise : `IF x = TRUE`,
  `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (la forme avec `NOT` seul, sans
  opérateur relationnel), `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`,
  `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE`, et `WHEN TRUE` face à un
  sujet qui est une valeur. Un `TRUE`/`FALSE` seul est aussi une condition
  complète (`IF TRUE`, `PERFORM UNTIL TRUE`).
  ⚠️ Cela ne change **pas** les deux endroits où ces mots avaient déjà un sens :
  `SET <88‑name> TO TRUE` donne toujours à l'élément hôte une valeur qui
  satisfait la condition (et non le nombre 1), et `EVALUATE TRUE`/`EVALUATE
  FALSE` ci-dessous restent l'instruction de sélection standard.
- ✅ `AND` / `OR` / `NOT` combinés, parenthèses (AND lie plus fort que OR).
- ✅ **Conditions abrégées préfixées par l'opérateur** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (le sujet de la comparaison précédente est réutilisé).
- ✅ **Abréviation à objet littéral** — `a = 1 OR 2 OR 3` (réutilise à la fois le
  sujet et l'opérateur ; l'objet est un littéral).
- ✅ **Abréviation à objet identificateur** — `a = b OR c` (où `c` est un élément
  de données). Un identificateur seul après AND/OR à la suite d'une comparaison
  est résolu à l'exécution : s'il s'agit d'un nom-condition de niveau 88 connu,
  il est évalué comme tel ; sinon, c'est l'objet de `a = c`. (Un identificateur
  immédiatement suivi de `AND` conserve la priorité du AND.)
- ✅ **Un `NOT` placé devant l'*objet* d'une abréviation nie la relation**, pas
  l'objet : `a > b OR NOT c` vaut `a > b OR NOT (a > c)`. L'écriture `NOT
  <relational operator>` (`AND NOT < x`) est la forme opérateur et reste
  inchangée, et un `NOT` qui ouvre une condition ordinaire — `NOT (…)`,
  `NOT x = y`, `NOT x NUMERIC` — garde son sens propre. *(Avant 1.62.42, la
  forme objet était lue comme « l'objet est non nul », ce qui ne donne la même
  réponse que si l'objet contient justement zéro.)*
- ✅ **Un nom-condition déclaré sur un groupe teste les octets du groupe.** Un
  groupe ne possède pas de mémoire propre — il *est* ses éléments subordonnés —,
  si bien que `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compare
  avec les six caractères que contient l'enregistrement.
- ✅ **Une constante figurative est répétée jusqu'à la taille de l'autre
  opérande**, y compris lorsqu'elle est écrite comme `VALUE` d'un 88 :
  `88 B VALUE QUOTE` sur un hôte `PIC X(4)` fait quatre guillemets, et
  `88 D VALUE ALL "BAC"` vaut `"BACB"`. `ALL literal` est dimensionné dans les
  **deux** sens — `IF X EQUAL TO ALL "BA"` sur un `X` de dix caractères compare
  avec `"BABABABABA"`, et non avec `"BA"` complété par des espaces.

---

## Expressions, littéraux, USAGE

- ✅ Opérateurs arithmétiques `+ - * /` et `**` ; parenthèses ; `+`/`-` unaires.
- ✅ `FUNCTION nom ( arg [ , arg … ] )` — intrinsèques **implémentées** :
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (avec germe facultatif), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Les conversions de date utilisent la base standard 1601‑01‑01 = jour 1.) Le
  **jeu complet d'intrinsèques du standard COBOL‑85** est implémenté.
- ✅ **Les registres de date et d'heure lisent l'horloge LOCALE.** `ACCEPT … FROM
  DATE / TIME / DAY / DAY-OF-WEEK` et `FUNCTION CURRENT-DATE` rapportent tous
  l'heure propre de la machine, et non UTC — y compris la date, qui diffère de
  part et d'autre de minuit. Les cinq derniers caractères de `CURRENT-DATE`
  portent le décalage **réel** par rapport à GMT (`…-0300`), de sorte qu'un
  programme peut savoir dans quel fuseau il s'exécute.
  ⚠️ Tout nom de `FUNCTION` non reconnu s'analyse quand même, mais renvoie **0** à
  l'exécution.
- ✅ Littéraux : entier, décimal, chaîne, toutes les constantes figuratives
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Une constante figurative remplit son récepteur tout entier**, y compris
  `HIGH-VALUE` — `MOVE HIGH-VALUE TO <PIC X(10)>` donne dix octets `0xFF`, et
  vers un groupe elle est répartie entre les enfants. Un récepteur
  alphanumérique édité place toujours ses caractères d'insertion, si bien que
  `PIC XX0XXBXXX` contient `FF FF '0' FF FF ' ' FF FF FF`. Sous une
  `PROGRAM COLLATING SEQUENCE`, la constante désigne un caractère ordinaire et
  c'est ce caractère qui remplit.
  ⚠️ `HIGH-VALUE` est l'**octet** `0xFF`, non un caractère. La lecture d'un
  opérande de groupe, l'édition et tous les chemins de déplacement le
  transportent octet par octet, mais **la modification par référence n'est pas
  encore exacte à l'octet** : `IF X (1:1) = HIGH-VALUE` est faux pour un élément
  qui contient réellement `0xFF`.
- ✅ **Un littéral numérique peut commencer par le point décimal** — `.5`, `-.5`,
  `.000000001`. COBOL‑85 exige seulement qu'un littéral ne se *termine* pas par
  un point, si bien que `5.` reste le nombre 5 suivi d'un terminateur de phrase.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Les zéros de tête sont significatifs et exacts : `.000000001` vaut un
  milliardième, pas un dixième. Sous `DECIMAL-POINT IS COMMA`, il en va de même
  pour `,5`.
  Ce qui sépare le littéral d'un point de fin de phrase, c'est l'**absence
  d'espace** — COBOL‑85 en exige un après un terminateur, donc `MOVE X TO Y.`
  n'est jamais lu comme le début d'une fraction, et `MOVE X TO Y.5` est une
  erreur de compilation plutôt qu'une réinterprétation silencieuse.
- ✅ **Signalement de conformité** (`cobolt_semantic::flagging`) — le standard
  demande qu'une implémentation conforme soit capable d'indiquer à un programme
  lesquelles des fonctionnalités qu'il emploie se situent en dehors d'un niveau
  de conformité choisi. Deux analyses y répondent :
  - `flag_obsolete` — l'ensemble des **éléments obsolètes** de COBOL‑85 : les
    cinq paragraphes facultatifs de l'IDENTIFICATION DIVISION, `MEMORY SIZE`,
    `ALTER`, `STOP` suivi d'un littéral, et `GO TO` sans nom de procédure.
  - `flag_high_subset` — tout ce qui dépasse le **sous-ensemble haut**, depuis
    `COMPUTE`, `EVALUATE` et `INITIALIZE` en passant par `CORRESPONDING`, la
    modification par référence, la qualification, `SET … TO TRUE` et un quatrième
    indice, jusqu'à la continuation d'un *mot* ou d'un *littéral numérique* par
    delà la limite de carte. (Continuer un littéral **alphanumérique** relève du
    sous-ensemble et n'est pas signalé.)

  Ni l'une ni l'autre n'est un contrôle d'erreurs, et aucune ne s'exécute lors
  d'une compilation ordinaire : chaque construction qu'elles nomment est du
  COBOL‑85 valide que RustCOBOL implémente et exécute. Ce sont des points
  d'entrée distincts précisément pour qu'une compilation normale ne se mette
  jamais à avertir au sujet d'`AUTHOR` ou de `COMPUTE`. Les programmes NIST
  `NC302M`, `NC303M` et `NC401M` les valident — 7, 4 et 40 signalements, tous
  concordants.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — le caractère qui
  remplit une position monétaire dans un PICTURE édité. Il **remplace** `$` au
  lieu de s'y ajouter : dès qu'un programme en déclare un, `$` cesse d'être un
  caractère de picture à cet endroit :
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` donne alors `      <.00`, et `MOVE 1234` donne
  ` <1,234.00` — la suite flottante se comporte exactement comme `$$$,$$$.99`.
  Un symbole monétaire **alphabétique** fonctionne de la même façon :
  `CURRENCY SIGN IS "W"` fait de `PICTURE WWWWW` une chaîne monétaire flottante
  de cinq positions, si bien que `MOVE 12` donne `  W12`. *(Avant la 1.62.40, une
  suite formée d'un symbole alphabétique était lue comme un seul mot et rejetée,
  de sorte que seul `$` flottait.)* Le
  littéral doit tenir en un caractère, et COBOL‑85 en interdit un qui entrerait
  en collision avec un caractère de picture ou un séparateur : pas de chiffre,
  aucun de `A B C D E G N P R S V X Z`, et aucun de
  `space * + - , . ; ( ) " / =`.
- ✅ **Littéraux hexadécimaux** — `X"09"`, `x'0D0A'` (indifféremment en majuscules
  ou minuscules, avec l'un ou l'autre type de guillemets). Un caractère par
  **paire** de chiffres hexadécimaux : le nombre de chiffres doit donc être pair ;
  un nombre impair ou un chiffre non hexadécimal constitue un littéral mal formé
  et est signalé, plutôt que relu silencieusement comme le mot `X` accolé à une
  chaîne. Utilisables partout où un littéral entre guillemets l'est
  (`DELIMITED BY`, `MOVE`, `VALUE`, comparaisons).

---

## Clauses de la DATA DIVISION (syntaxe de déclaration acceptée)

- ✅ Niveaux `01`–`49`, `77`, `88` ; `FILLER` ; groupe/élémentaire. Le mot
  `FILLER` est **facultatif** — `05 PIC X VALUE ":".` en déclare un tout comme
  le fait `05 FILLER PIC X VALUE ":".`, et dans les deux cas il occupe ses
  octets et conserve son `VALUE` à l'intérieur du groupe qui le contient.
- ✅ `PIC/PICTURE` avec `X A 9 S V P` et les symboles d'édition
  (`Z * $ + - CR DB B 0 / , .`). Le symbole monétaire est `$` sauf si
  `SPECIAL-NAMES. CURRENCY` en a nommé un autre — voir **Expressions,
  littéraux, USAGE** ci-dessus. **`P` est une position de cadrage décimal** —
  une position de chiffre que l'élément couvre mais ne stocke pas :
  `PIC S999PP` contient trois chiffres qui valent des centaines
  (`MOVE 12300` le stocke exactement ; `MOVE 12345` stocke 12300), et
  `PIC PP99` en contient deux qui valent des dix-millièmes. Les positions
  occupées par les `P` se relisent toujours comme des zéros et ne prennent
  **aucun octet** dans la description d'un enregistrement.
- ✅ **La protection par astérisques remplit l'élément entier.** Une valeur nulle
  dans une PICTURE dont toutes les positions de chiffre sont des `*` remplit
  d'astérisques chaque position de caractère — les décimales, les virgules de
  groupement, un `$` fixe et un `CR` ou `DB` final tout autant — en ne laissant
  que le point décimal lui-même : `PIC $**.**CR` contenant zéro se lit
  `***.****`, et `PIC *,***.**` se lit `*****.**`. Une valeur **non** nulle ne
  protège que les zéros de tête, si bien que le `$` fixe garde sa propre
  position (`-2.34` → `$*2.34CR`). *(Avant 1.62.37 `CR`/`DB` n'apportaient qu'un
  seul astérisque au lieu des deux positions de caractère qu'ils occupent, de
  sorte qu'un tel élément revenait plus court d'un caractère que sa propre
  largeur.)*
- ✅ **Un littéral numérique déplace ses caractères, tels qu'ils sont écrits.**
  Vers un récepteur alphanumérique, un littéral apporte les chiffres que le
  programme a saisis, cadrés à gauche et complétés par des espaces —
  `MOVE 2 TO <PIC X(4)>` donne `"2   "`, et
  `MOVE 060820000200 TO <six PIC 99 children>` les remplit ainsi :
  `06 08 20 00 02 00`. La largeur du **récepteur** ne complète jamais le
  littéral ; seule sa propre largeur écrite le fait. *(Avant 1.62.38 le lexer ne
  gardait que la valeur, donc un zéro de tête était perdu et chaque caractère
  suivant se décalait d'une place vers la gauche.)*
- ✅ **Une relation entre un opérande numérique et un opérande non numérique est
  non numérique** (COBOL‑85 VI‑89 6.15.4 GR2). L'opérande numérique est traité
  comme s'il avait été déplacé vers un élément alphanumérique de **sa propre
  taille**, ce qui transfère ses positions de caractère et **non son signe
  opérationnel** : un `PIC S9(18)` contenant `-123456789012345678` est jugé
  **égal** à un `PIC X(18)` contenant `"123456789012345678"`. Trois conditions
  bornent la règle — l'opérande numérique doit être un **entier** ; le caractère
  « non numérique » est décidé par la **déclaration**, donc un enfant `PIC 99`
  contenant des caractères après un `MOVE` de groupe reste numérique — et un
  **groupe** est non numérique quels que soient ses enfants, si bien qu'un
  `PIC 9(5)` contenant 12345 face à un groupe de dix octets contenant
  `"0000012345"` vaut `"12345     "` et diffère ; et `ALL literal` prend la
  taille de l'autre opérande. *(Avant 1.62.38 la comparaison était algébrique
  dès que le côté texte se laissait lire comme un nombre.)*
- ✅ **Troncature de poids fort lors d'un MOVE numérique.** Un récepteur ne
  garde exactement que les chiffres qu'il a déclarés, aux deux extrémités :
  `01 M PIC 99V999.  MOVE 123.45 TO M.` laisse `23.450`. L'arithmétique teste
  d'abord la capacité du récepteur, si bien qu'une instruction avec
  `ON SIZE ERROR` conserve plutôt son ancienne valeur.
- ✅ **Une table de groupes s'adresse par occurrence.** `MOVE VALUES-1 TO
  GRP-1 (2)` répartit la valeur entre les enfants propres à cette occurrence
  (`ELEM1 (2,1) … ELEM1 (2,4)`), et lire `GRP-1 (2)` concatène exactement
  ceux-là. L'enregistrement `01` englobant, ce sont les octets de **toutes** les
  occurrences, donc `MOVE GRP-TAB1 TO GRP-TAB2` copie une table entière.
- ✅ **Noms d'index, littéraux et indexation relative se mêlent comme indices.**
  `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — un signe collé à ses
  chiffres est un littéral signé qui ouvre l'indice suivant — et
  `ELEM1 (IN1 - 1, 3)`, où l'opérateur est espacé des deux côtés, est de
  l'indexation relative.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (et `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérique/signé/alphanumérique/figuratif/`ALL`). **`VALUE ALL
  "literal"` répète son motif sur tout l'élément** — `PIC X(6) VALUE ALL
  "ABC"` vaut `"ABCABC"` et `PIC X(9) VALUE ALL "XY"` vaut `"XYXYXYXYX"`.
  *(Avant 1.62.40 seules les constantes figuratives d'un seul caractère
  remplissaient leur élément et `ALL "literal"` le laissait rempli d'espaces.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — une seconde lecture **vivante** des mêmes octets. Elle
  n'ajoute aucune mémoire (elle n'élargit donc pas le groupe qui la contient),
  et une écriture faite par l'une ou l'autre description est visible par
  l'autre : `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` puis relecture par `RESULT-A`.
  ⚠️ **Réserve :** un recouvrement de plus de 256 emplacements de mémoire
  développés (une table 10×10×10 redéfinie, par exemple) conserve une mémoire
  par description — la rafraîchir à chaque écriture parcourrait mille
  occurrences deux fois.
- ✅ **Les recouvrements s'imbriquent.** Un `REDEFINES` situé dans un
  enregistrement lui-même redéfini est atteint dans les deux sens, aussi profond
  soit-il : écrire deux octets à travers une redéfinition de niveau 01 atteint
  l'enregistrement redéfini, le `REDEFINES` d'un groupe qui s'y trouve et le
  `REDEFINES` d'un élément qui se trouve dans *celui-ci* — y compris un 88
  déclaré sur le plus interne. Chaque description est régénérée une fois par
  écriture. *(Avant 1.62.42 une clé appartenant à plus d'un recouvrement ne
  gardait que la dernière déclarée, et une garde unique arrêtait la chaîne après
  son premier saut.)*
- ✅ **Une description sans nom reste une description.** `02 FILLER REDEFINES
  <item>.` redécrit les octets de sa cible sous aucun nom propre, et une
  écriture dans la cible est visible par ses enfants. Plusieurs enfants se
  partagent ces octets, dans l'ordre de la description — le recouvrement n'est
  *pas* un alias de son premier enfant. Deux `FILLER REDEFINES` d'un même
  élément sont deux lectures indépendantes, chacune commençant au **premier**
  octet de la cible. *(Avant 1.62.36 un groupe redéfinissant sans nom ne
  recevait aucune clé de mémoire, si bien que ses enfants se lisaient comme des
  espaces quel qu'ait été le remplissage de la cible.)*
- ✅ **Un nom dupliqué à l'intérieur d'un recouvrement** se résout vers la même
  mémoire que celle qu'atteint le reste du programme : `TAB-A` déclaré sous deux
  groupes différents garde une lecture par déclaration. *(Avant 1.62.36 la copie
  initiale du recouvrement était indexée par un chemin auquel manquaient ses
  qualificateurs extérieurs, ce que seul un nom dupliqué permet de distinguer —
  autrement dit, exactement le cas qui a besoin du qualificateur le perdait.)*
- ✅ `JUSTIFIED [RIGHT]` — **stocke cadré à droite**, sur un élément
  *alphanumérique* ou *alphabétique*. Un émetteur plus étroit que le récepteur
  est complété à gauche ; un émetteur plus large que lui garde son extrémité
  **droite** et perd ses caractères les plus à gauche — l'inverse de la règle
  ordinaire. *(Avant 1.62.40 la clause n'était enregistrée que pour les éléments
  alphanumériques, si bien que `PICTURE A(5) JUSTIFIED RIGHT` s'analysait puis
  cadrait à gauche comme n'importe quel autre élément.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — acceptées ;
  `SIGN … SEPARATE` ne change pas encore la façon dont l'élément est stocké.
- ✅ **Un `REDEFINES` au niveau 01 peut décrire plus de mémoire que l'élément
  qu'il redéfinit**, et les octets situés au-delà de la fin de cet élément
  appartiennent à la description qui est assez longue pour les nommer. Écrire à
  travers une description plus courte laisse intacte la queue de la plus longue.
- ✅ **Un recouvrement `REDEFINES` emporte les octets de l'élément redéfini**, y
  compris vers un homologue numérique : un recouvrement `PIC S9(18)` d'un
  `X(18)` contenant `"00ABCDEFGHI  4321 "` relit ces caractères, et
  `IS NUMERIC` répond **non** à leur sujet. Quand les octets forment bien des
  chiffres, la lecture numérique est inchangée.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — de **vrais noms-conditions** :
  le niveau 88 se lie à son élément hôte ; le test confronte l'hôte aux VALUE /
  aux plages, et `SET 88-name TO TRUE` range dans l'hôte une valeur qui le
  satisfait.
- ✅ **Un nom-condition peut être déclaré sous plus d'un groupe, et `OF`/`IN`
  les distingue** — exactement comme pour un nom de donnée, et les niveaux
  intermédiaires peuvent être omis :
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  L'indice appartient à l'élément hôte, il sélectionne donc l'occurrence contre
  laquelle les VALUE sont testés. Une référence **non qualifiée** à un
  nom-condition dupliqué est ambiguë en COBOL‑85 ; le runtime retient la
  première déclaration, la règle même qu'il applique à un nom de donnée ambigu.
- ✅ `USAGE INDEX` déclare un registre d'index entier (`SET`/`SEARCH`
  l'utilisent) ; `USAGE POINTER` — voir **Pointeurs** ci-dessus.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — un alias de
  regroupement ; la lecture concatène les éléments couverts, l'écriture répartit
  selon la largeur de chaque champ.
  - ✅ **Un 66 est qualifié par l'enregistrement qu'il regroupe**, exactement
    comme un élément de données est qualifié par le groupe au-dessus de lui ; le
    même nom de 66 peut donc être déclaré une fois par enregistrement et
    distingué avec `OF`/`IN` :
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Cela vaut aussi bien en
    lecture qu'en écriture, et un 66 l'emporte sur un élément de données
    ordinaire qui partagerait son nom. Les opérandes de la clause `RENAMES` se
    résolvent dans ce même enregistrement, donc un `NAME-2` dupliqué désigne
    celui de cet enregistrement.
  - ✅ **Une table couverte apporte toutes ses occurrences**, pas seulement la
    première : `66 R RENAMES ITEM-1 THRU TABLE-2`, où `TABLE-2` contient
    `03 T PIC XXX OCCURS 5`, fait 20 caractères de large.
  - ✅ **Un 66 portant sur exactement un élément *est* cet élément** — même
    PICTURE, même catégorie, même mémoire. `66 R RENAMES W`, où `W` est
    `PIC 9(4)`, est un élément numérique de quatre chiffres, si bien que
    `ADD 3500 TO R` avec 8000 dedans déclenche `ON SIZE ERROR` et le laisse
    inchangé.
- Sections : `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE` ; `SCREEN`
  est analysée mais non exécutée.

---

## Toujours PAS pris en charge — liste actuelle des éléments à éviter

> **Corrigé le 2026‑08‑25.** Cette section s'ouvrait autrefois sur « Le jeu de
> verbes et de clauses COBOL‑85 est **entièrement couvert**. » L'exécution de la
> suite NIST CCVS85 l'a démenti : **102 des 434 programmes dans le périmètre ont
> échoué ce jour-là**, sur des constructions que ce document ne listait pas comme
> des manques — virgules et points-virgules séparateurs, `FUNCTION x(ALL)`,
> `CLOSE … WITH LOCK`, `COPY` en zone B, entrées de commentaire de
> l'IDENTIFICATION, numéros de priorité de section, noms de données commençant par
> un chiffre et — jusqu'à la 1.62.10 — littéraux numériques débutant par un point
> décimal. C'est à cela que sert une suite de validation. Chaque manque est
> désormais spécifié dans [`specs/nist/`](../specs/nist/README.md) et suivi dans
> le [tableau de bord](#-la-conformité-est-mesurée-pas-affirmée--nist-ccvs85)
> ci-dessus.

La liste ci-dessous est ce qui reste hors périmètre **à dessein**, par opposition
aux manques NIST ci-dessus, qui sont des défauts en cours de traitement :

1. **Édition de saisie par `ACCEPT` écran** — `DISPLAY … AT/WITH` et
   `ACCEPT … AT` sont exécutés (ANSI) en mode CLI, mais l'édition complète au
   niveau champ de la SCREEN SECTION (tabulation automatique, validation de champ,
   cartes de couleurs) est **supplantée par le form designer** en mode GUI.
2. **Partage de fichiers entre *processus*** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK` et `UNLOCK` s'analysent et pilotent les verrous
   d'enregistrement par exécution du moteur INDEXED, mais les verrous ne sont pas
   imposés entre processus distincts du système (modèle à unité d'exécution
   unique).
3. **COBOL orienté objet** (définitions de classe/méthode) — `INVOKE` est sans
   effet pour les objets COBOL (il ne pilote que les objets GUI/runtime).
4. ⚠️ Organisation de fichiers **RELATIVE** (SEQUENTIAL / LINE SEQUENTIAL /
   INDEXED sont faites). **Celle-ci est un piège, pas un manque net :**
   `ORGANIZATION IS RELATIVE` *s'analyse*, et rien dans le runtime ne s'y réfère
   jamais pour aiguiller le traitement — si bien qu'un programme RELATIVE compile
   puis se comporte mal sans le moindre diagnostic. 30 des 35 programmes du module
   RL du NIST sont exactement dans cet état. Considérez-la comme non implémentée.
   Spécification :
   [organisation RELATIVE](../specs/nist/NIST-spec-relative-organization.md).
5. Les noms de fonction intrinsèque non reconnus renvoient toujours **0** — le
   même mode de défaillance silencieuse. Spécification :
   [intrinsèques](../specs/nist/NIST-spec-intrinsic-function-gaps.md).
6. ⚠️ **Une valeur invalide d'`ACCESS MODE` / d'`ORGANIZATION` est avalée sans
   diagnostic** — le même piège encore, et celui-ci se déclenche sur une simple
   faute de frappe de l'utilisateur. `ACCESS MODE IS` n'accepte que `SEQUENTIAL`,
   `RANDOM` ou `DYNAMIC` (`INDEXED` est une *organisation*, pas un mode d'accès),
   mais l'analyseur de la clause SELECT teste ces trois valeurs et laisse tout le
   reste tomber dans la branche générique « sauter un jeton inconnu » : le fichier
   conserve donc silencieusement le `SEQUENTIAL` par défaut et se comporte mal à
   l'exécution au lieu d'échouer à la compilation. `ORGANIZATION IS` a exactement
   la même forme. Les deux devraient lever une erreur de compilation claire nommant
   le mot fautif. **Ce n'est pas un problème du Noyau** — aucun programme NC ne
   porte de clause `ACCESS MODE` ; la clause n'apparaît que dans les modules DB,
   IC, IX, OBSQ, RL, RW, SQ et ST, si bien que, sous la RÈGLE D'OR nº 9, cela
   attend que NC soit terminé.
7. ⚠️ **`ALPHABET … IS EBCDIC` est accepté mais laisse en vigueur l'ordre natif
   (ASCII).** La phrase littérale (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` et `STANDARD‑2` sont tous implémentés et pilotent réellement
   `PROGRAM COLLATING SEQUENCE` ; seule la table EBCDIC manque, et la nommer donne
   silencieusement l'ordre ASCII. Même famille de pièges que 4–6.
8. **Le module Communication et le Report Writer** — voir
   [N/A ci-dessus](#-na--ce-qui-est-hors-du-périmètre-de-rustcobol-et-pourquoi).

> **Résolu (1.5.0) :** le modèle de données plat est devenu hiérarchique /
> sensible aux occurrences, débloquant **CORRESPONDING**, les **noms qualifiés**,
> l'**indiçage de tables** et **`SEARCH`**.
> **Résolu (1.6.0) :** `MULTIPLY`/`DIVIDE` à récepteurs multiples + `ROUNDED` par
> récepteur ; `EXIT PERFORM/PARAGRAPH/SECTION` ; `CALL NOT ON EXCEPTION` ;
> `INSPECT TALLYING REPLACING` combiné + `BEFORE/AFTER INITIAL` ; intrinsèques de
> date et `ANNUITY` ; abréviation à objet littéral ; `EVALUATE ALSO`/`WHEN NOT` ;
> noms-conditions de niveau 88 réels ; `PERFORM para VARYING` ; et le runtime
> `SORT`/`MERGE` avec `RELEASE`/`RETURN`.
> **Résolu (1.7.0) :** abréviation à objet identificateur ;
> `INITIALIZE … REPLACING` ; `66 RENAMES` ; pointeurs (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`) ; `ALTER` / `UNLOCK` ;
> `NEXT SENTENCE` fidèle ; les intrinsèques standard restantes ; et
> l'`ACCEPT`/`DISPLAY` écran étendu (exécuté en mode CLI).
> **Résolu (1.7.1) :** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (avec les
> registres appariés `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Résolu (1.7.2) :** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (libère les verrous d'enregistrement INDEXED) et `CANCEL programme`.
> **Résolu (1.8.0) :** `COMMIT` / `ROLLBACK` en tant que transactions de fichiers
> INDEXED pilotées par le programme (moteurs mémoire et disque ; véritable
> journal d'annulation sur disque).

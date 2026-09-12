<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.70.0 -->

# Référence de la syntaxe prise en charge par RustCOBOL‑85

**À quoi sert ce document :** à dire quelle part de la norme COBOL‑85 RustCOBOL
implémente réellement — et à le démontrer face à la **suite officielle de
validation NIST COBOL‑85** plutôt qu'à l'affirmer. Le
[tableau de bord](#-la-conformité-est-mesurée-non-affirmée--nist-ccvs85) ci‑dessous
est le titre ; tout ce qui suit est le détail derrière ce chiffre.

**Vérité de terrain sur ce que le lexeur/analyseur/runtime de RustCOBOL acceptent
réellement aujourd'hui**, dérivée du code source (`cobolt-lexer`, `cobolt-parser`,
`cobolt-runtime`) et confrontée à `NIST/newcob.val,cbl`.
Écrivez les tests contre les formes ✅ ; les formes ❌ n'arriveront pas à être
analysées ou sont sans effet, et les formes ⚠️ sont analysées mais ne se comportent
que partiellement. Ceci est le document compagnon de
[`cobol85-verb-test-matrix-fr.md`](cobol85-verb-test-matrix-fr.md) : la matrice dit
*quoi* tester, et celui‑ci dit *quelle graphie RustCOBOL comprend*.

Légende : ✅ pris en charge · ⚠️ analysé mais partiel/simplifié · ❌ non reconnu
(à éviter, ou à tester uniquement pour confirmer la lacune).

---

## Table des matières

1. [★ La conformité est mesurée, non affirmée — NIST CCVS85](#-la-conformité-est-mesurée-non-affirmée--nist-ccvs85)
2. [Paragraphes de l'IDENTIFICATION DIVISION](#paragraphes-de-lidentification-division)
3. [Formats de source](#formats-de-source)
4. [Instructions reconnues (verbes)](#instructions-reconnues-verbes)
5. [Formes prises en charge par verbe](#formes-prises-en-charge-par-verbe)
6. [Conditions (IF / EVALUATE / PERFORM UNTIL)](#conditions-if--evaluate--perform-until)
7. [Expressions, littéraux, USAGE](#expressions-littéraux-usage)
8. [Clauses de la DATA DIVISION (syntaxe de déclaration acceptée)](#clauses-de-la-data-division-syntaxe-de-déclaration-acceptée)
9. [Toujours PAS pris en charge — liste d'évitement actuelle](#toujours-pas-pris-en-charge--liste-dévitement-actuelle)

---

## ★ La conformité est mesurée, non affirmée — NIST CCVS85

**C'est là tout le sens du document.** Chaque affirmation ci‑dessous est vérifiée
face à la **suite officielle de validation NIST COBOL‑85** — CCVS85 version 4.0
(01 OCT 1992, COBOL 85 version 4.2, SSVG d'avril 1993), la suite que le National
Institute of Standards and Technology des États‑Unis utilisait pour certifier les
compilateurs COBOL. Elle pèse 28 Mo, 348 271 lignes, **459 programmes COBOL** et
51 membres de copybook, et elle réside dans ce dépôt à `NIST/newcob.val,cbl`.

C'est la source de vérité. Là où RustCOBOL et CCVS85 divergent, **CCVS85 a raison
et RustCOBOL a tort**.

Le registre lisible par machine est
[`NIST/progress.json`](../NIST/progress.json) — versionné, mis à jour après chaque
changement vérifié. Les chiffres ci‑dessous en sont tirés plutôt que retapés.

### Le tableau de bord

Mesuré le **2026‑08‑31 en 1.62.132**, sur la distribution intacte. Le recensement
de compilation s'est clos en 1.62.129.

| Axe | Résultat | Signification |
|---|---:|---|
| **Compilation** | **420 / 420** | tous les programmes dans le périmètre sont acceptés par le front end. FAIL 0. |
| **Exécution** | **380 / 380** | tous les programmes notés s'exécutent et signalent **zéro échec** dans leur propre rapport CCVS. |
| **Assertions** | **8 362 PASS / 0 FAIL** | les vérifications que ces programmes font sur eux‑mêmes. |

Reproduisez l'un ou l'autre axe :

```bash
cargo run -p cobolt-semantic --example nist_conformance -- strict     # compile
cargo build --release -p cobolt-cli                                   # the harness runs the real binary
cargo run --release -p cobolt-semantic --example nist_conformance -- run NC
```

#### Les deux axes ne sont jamais confondus

La compilation est l'affirmation strictement la plus faible : elle dit que le front
end accepte toutes les constructions d'un programme, non que le programme calcule
la bonne réponse. La suite se note elle‑même — chaque programme CCVS85 imprime son
propre décompte `PASS` / `FAIL*` — de sorte que l'axe d'exécution est celui qui
signifie « cela fonctionne ». Les deux sont rapportés par module ci‑dessous, avec
leurs propres dénominateurs, et aucun n'est jamais cité comme s'il était l'autre.

L'illustration la plus claire se trouve dans l'histoire même de ce dépôt : 30 des
35 programmes de fichiers RELATIVE compilaient proprement alors que le runtime
**n'avait aucun moteur RELATIVE**. Ils s'exécutaient et produisaient des résultats
faux en silence. Le moteur est arrivé en 1.62.76 et le module s'est achevé en
1.62.77.

#### Par module

La compilation et l'exécution portent des dénominateurs différents, pour deux
raisons déclarées. Les membres `*301M` testent le *signalement de sous‑ensemble
intermédiaire* de fonctionnalités que RustCOBOL implémente comme standard, ce qui
est inatteignable par conception et exclu de l'exécution par décision de
l'opérateur (IX301M, RL301M, ST301M, SM301M) ; ils comptent toujours dans le
recensement de compilation, où ils passent. Et la plupart des membres IC sont des
**appelés** — des sous‑programmes sans rapport propre — de sorte que seuls les
programmes appelants sont notés.

| Module | Ce qu'il teste | Compilation | Exécution | Assertions | État |
|---|---|---:|---:|---:|---|
| **NC** | Noyau | **95 / 95** | **95 / 95** | 4 614 | ✅ achevé |
| **SQ** | E/S séquentielles | **85 / 85** | **85 / 85** | 624 | ✅ achevé |
| **IX** | E/S indexées | **42 / 42** | **41 / 41** | 574 | ✅ achevé |
| **IF** | Fonctions intrinsèques | **45 / 45** | **45 / 45** | 841 | ✅ achevé |
| **IC** | Communication entre programmes | **47 / 47** | **25 / 25** | 309 | ✅ achevé |
| **ST** | Sort / Merge | **40 / 40** | **39 / 39** | 735 | ✅ achevé |
| **SM** | Manipulation du texte source | **17 / 17** | **16 / 16** | 311 | ✅ achevé |
| **RL** | E/S relatives | **35 / 35** | **34 / 34** | 354 | ✅ achevé |
| **DB** | Débogage | **14 / 14** | — | — | axe de compilation uniquement (ci‑dessous) |
| **Dans le périmètre** | | **420 / 420** | **380 / 380** | **8 362** | |
| SG | Segmentation | 13 / 13 | — | — | ⬜ déclaré hors périmètre (ci‑dessous) |
| CM · RW · OBSQ · OBIC · OBNC · EXEC85 | | — | — | — | ⬜ N/A |

**DB (Débogage)** est noté sur la compilation seulement. Ses 14 programmes sont
acceptés ; la sémantique d'*exécution* du module de débogage n'est pas implémentée,
et l'axe d'exécution pour lui n'a pas été déclaré dans le périmètre. Il est listé
ici plutôt que caché, pour que la lacune reste visible.

#### Le décompte DELETED — 24, et ce qu'il signifie

`***** ****TEST DELETED****` est le marqueur propre de CCVS pour un cas que le
programme a lui‑même sauté. Ce n'est **pas** une réussite, et il est suivi
séparément pour cette raison : le décompte est tombé de 108 → 1 en 1.62.53 alors
que le nombre de programmes propres bougeait à peine, ce qui était un progrès réel
qu'une lecture centrée sur les seuls échecs aurait manqué.

Dans les modules achevés, 24 cas sont DELETED : NC 5, SQ 6, IX 1, IC 4, SM 3,
RL 5. **Seuls les 3 de SM sont documentés comme propres à la distribution** —
SM206A PST‑TEST‑008 et PST‑TEST‑11, ainsi que SM208A REP‑TEST‑7, sont livrés en
commentaire, de sorte qu'une exécution conforme du source livré signale exactement
ces trois‑là. Les 21 autres sont enregistrés mais pas encore expliqués
individuellement dans le registre ; ne les citez pas comme voulus.

### ⬜ N/A — ce qui sort du périmètre de RustCOBOL, et pourquoi

Ces modules **ne sont pas comptés comme des échecs** — 38 programmes exclus de
toute note. Le raisonnement complet est dans
[`NIST-spec-out-of-scope-modules.md`](../specs/nist/NIST-spec-out-of-scope-modules.md).

| Module | Programmes | Pourquoi il sort du périmètre |
|---|---:|---|
| **CM** — Communication | 9 | `COMMUNICATION SECTION`, entrées `CD`, `SEND` / `RECEIVE` / `ENABLE` / `DISABLE`. Vise les moniteurs de télétraitement des années 1980 — des files de messages détenues par un gestionnaire de transactions. Il n'existe pas de tel runtime ici, et le module a été retiré des normes COBOL ultérieures. |
| **RW** — Report Writer | 6 | `REPORT SECTION`, entrées `RD`, `INITIATE` / `GENERATE` / `TERMINATE`, ruptures de contrôle. Un vaste sous‑langage déclaratif ; la réponse de PowerRustCOBOL au reporting est le Form Designer et l'export PDF. Pourrait devenir plus tard une *fonctionnalité* si on le souhaite — c'est la seule exclusion ayant une valeur réelle pour l'utilisateur. |
| **SG** — Segmentation | 13 | Décision de l'opérateur, 2026‑08‑29. La segmentation existe pour faire tenir un programme dans une machine trop petite pour le contenir : les en‑têtes `SECTION` portent un numéro de segment et le runtime superpose des segments indépendants les uns aux autres. RustCOBOL est un runtime 64 bits avec plus d'espace d'adressage qu'aucun programme COBOL ne peut épuiser, de sorte qu'un numéro de segment **compile et n'a aucun effet**. Il n'y a aucun comportement que le module puisse mesurer. Ses 13 programmes compilent toujours, et sont signalés N‑A plutôt que supprimés pour que l'exclusion reste visible. |
| **OBSQ / OBIC / OBNC** | 9 | Ils retestent des modules antérieurs et attendent du compilateur qu'il *signale* des éléments obsolètes de COBOL‑85. Leur contenu de langage est couvert par les spécifications dans le périmètre ; ce qui sort du périmètre, c'est le **signalement** des fonctionnalités obsolètes. |
| **EXEC85** | 1 | Ce n'est pas un test. C'est l'exécutif COBOL propre au NIST qui découpe la distribution et pilote la suite — remplacé ici par un harnais en Rust, il n'a donc pas besoin de compiler. |

Le **COBOL orienté objet** sort également du périmètre de RustCOBOL, mais CCVS85
lui est entièrement antérieur — il n'y a aucun programme OO dans la suite.

### Ce qu'il reste

Sur les modules notés, rien : les deux axes sont clos et aucune assertion n'échoue.
Ce qui reste n'est pas une liste de défauts mais trois décisions permanentes —
l'axe d'exécution de DB, SG et les membres de signalement `*301M` — chacune
consignée ci‑dessus avec sa raison, plus les 21 cas DELETED qui n'ont pas été
expliqués individuellement.

Le harnais imprime le détail de l'échec derrière n'importe quelle régression, prêt
à être regroupé par module :

```bash
cargo run --release -p cobolt-semantic --example nist_conformance -- fails NC
```

> Une ligne de détail `FAIL*` est écrite **deux fois** à dessein — le
> `PRINT-DETAIL` de CCVS exécute
> `IF P-OR-F EQUAL TO "FAIL*" PERFORM WRITE-LINE` — tandis que `PASS ` est écrit
> une seule fois. Tout décompte brut de marqueurs pris dans le fichier
> d'impression doit diviser les échecs par deux avant de signifier quelque chose.

### Historique de conformité

Axe de compilation, face au dénominateur dans le périmètre du moment. Le
dénominateur lui‑même a bougé lorsque SG a été déclaré hors périmètre et lorsque
DB205A a été renoté sous CM, de sorte que les premières lignes sont sur 434 et la
ligne de clôture sur 420.

| Version | Compilation | Ce qui a changé |
|---|---:|---|
| 1.62.7 | **0** / 434 | Rien ne compilait. Deux règles du format de référence classique manquaient : les colonnes 73‑80 étaient lues comme du source, et les lignes de continuation n'étaient jamais jointes. |
| 1.62.8 | 222 / 434 | `--source-format=fixed` — le format de référence classique, continuation comprise. Voir [Formats de source](#formats-de-source). |
| 1.62.13 | 292 / 434 | La virgule et le point‑virgule séparateurs sont de la ponctuation, non des jetons ; les indices peuvent être séparés par de simples espaces ; un délimiteur doublé à l'intérieur d'un littéral est un seul caractère. Trois seaux de diagnostics entiers se sont vidés. |
| 1.62.14 | 317 / 434 | Une table entière comme argument intrinsèque ; `CLOSE … WITH LOCK` / `NO REWIND` / `REEL`. **Fonctions intrinsèques 45 / 45 en compilation.** |
| 1.62.16 | 376 / 434 | Le `AT` de `AT END` est facultatif, de sorte qu'une clause `END` seule n'avale plus l'en‑tête de paragraphe suivant (33 programmes). **E/S indexées 42 / 42 en compilation.** |
| 1.62.21 | 417 / 434 | La passe du Noyau — série `ALTER`, noms‑conditions indicés, relations combinées abrégées, catégories d'`INSPECT` entre opérandes. Noyau de 76 → 92 qui compilent. |
| **1.62.42** | 420 / 434 | **Noyau achevé sur les deux axes** — 95 / 95 qui compilent *et* s'exécutent proprement, 4 614 assertions sans aucun échec. |
| **1.62.43** | 422 / 434 | Les E/S séquentielles compilent entièrement, 85 / 85, et passent de 10 → 44 sur 85 en exécution. Les paragraphes d'un déclaratif conservent leurs noms, un gestionnaire `USE` peut donc leur faire `PERFORM` et `GO TO` — 20 programmes ont cessé de planter. |
| **1.62.47** | — | **E/S séquentielles achevées** — 85 / 85 sur les deux axes. La dernière lacune était `XXXXD001`, un fichier de données que l'*installation* de CCVS85 fournit et qu'aucun membre n'écrit ; le harnais le dépose désormais. |
| **1.62.76** | — | Le **moteur RELATIVE** arrive (`cobolt-runtime/src/relative.rs`, conteneur `PRCREL1`). Les sept verbes de fichier s'aiguillent sur `FileOrganization::Relative`. |
| **1.62.77** | — | **E/S relatives achevées** (34 / 34) depuis une base de 14 / 35 en une seule session, et **E/S indexées achevées** — le moteur relatif a clos les quatre derniers échecs d'IX106A, qui portaient sur le fichier relatif qu'il exerce aux côtés des séquentiels et des indexés. |
| **1.62.81** | — | **Fonctions intrinsèques achevées** en exécution, 45 / 45, depuis une base de 24 / 45. Cinq causes, aucune dans la matière propre du module : une règle de séparateurs du lexeur à deux reprises, une lacune de signalement, la grammaire de l'argument de `NUMVAL`, une comparaison de listes d'arguments et un chemin de dépassement dans la division. |
| **1.62.107** | — | **Communication entre programmes achevée**, 25 / 25. |
| **1.62.119** | — | **Sort / Merge achevé**, 39 / 39, 735 PASS. L'étape finale a été `[COLLATING] SEQUENCE [IS] alphabet-name` sur SORT/MERGE, ordonnant les clés alphanumériques selon l'alphabet nommé dans `SPECIAL-NAMES`. |
| **1.62.127** | — | **Manipulation du texte source achevée**, 16 / 16. Les opérandes littéraux de chaîne conservent leurs guillemets ; les opérandes identificateurs couvrent leur chaîne `IN`/`OF` et leur indice ; les paires s'appliquent en une seule passe sans réexamen des remplacements. |
| **1.62.129** | **420 / 420** | **Le recensement de compilation se clôt à 100 %.** DB205A est noté sous CM par décision, ce qui porte la suite dans le périmètre à 420. |

> **Le résumé honnête.** Tous les programmes dans le périmètre compilent, et tous
> les programmes notés s'exécutent proprement : **420 / 420 en compilation,
> 380 / 380 en exécution, 8 362 assertions sans aucun échec.** Neuf versions avant
> la première de ces lignes, le chiffre de compilation était zéro. Ce qui reste
> ouvert est déclaré ci‑dessus comme des décisions plutôt que caché dans un
> pourcentage — l'axe d'exécution de DB, SG, les membres de signalement `*301M`, et
> 21 cas DELETED qui n'ont pas été expliqués individuellement.

---

> **Mise à jour (passe d'implémentation des lacunes) :** les éléments suivants ont
> été implémentés et sont désormais ✅ — **modification de référence**
> `id(start:len)`, **`PERFORM n TIMES` en ligne**, **`SET … UP/DOWN BY`**,
> **STRING/UNSTRING `ON OVERFLOW` + `END-STRING`/`END-UNSTRING`**, **`INITIALIZE`
> conscient des catégories**, **conditions abrégées à opérateur antéposé**
> (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (s'exécute sur un CALL non résolu),
> **plusieurs récepteurs dans `COMPUTE` + `ROUNDED` par récepteur**, et un ensemble
> de **fonctions intrinsèques** bien plus vaste.
>
> **Mise à jour (passe d'environnement hiérarchique / conscient des occurrences —
> 1.5.0) :** quatre fonctionnalités bloquées par le modèle de données sont
> désormais ✅ — **indiçage de tables à l'exécution** `t(i)` / `t(i, j)` (stockage
> par occurrence), **désambiguïsation des noms qualifiés** `id OF/IN group` (les
> noms feuilles dupliqués se résolvent vers un stockage indépendant),
> **`MOVE/ADD/SUBTRACT CORRESPONDING`**, et **`SEARCH` / `SEARCH ALL`
> fonctionnels**.
>
> **Mise à jour (passe de complétude des verbes — 1.6.0) :** désormais aussi ✅ —
> **`MULTIPLY`/`DIVIDE GIVING` à plusieurs récepteurs + `ROUNDED` par récepteur**
> sur `ADD`/`SUBTRACT` ; **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` /
> `EXIT SECTION`** et le `EXIT` simple corrigé ; **`CALL … NOT ON EXCEPTION`** ;
> **`INSPECT … TALLYING … REPLACING`** combiné et régions
> **`BEFORE/AFTER INITIAL`** ; **intrinsèques** de date/finance
> (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`,
> `ANNUITY`, `FRACTION-PART`) ; **conditions abrégées à objet littéral**
> (`A = 1 OR 2 OR 3`) ; **`EVALUATE … ALSO`** (multi‑sujet) et **`WHEN NOT`** ;
> **vrais noms‑conditions de niveau 88** (`SET … TO TRUE/FALSE`, l'hôte est testé
> face à ses VALUE/plages) ; **`PERFORM para VARYING`** ; et un runtime
> **`SORT`/`MERGE`** fonctionnel (`RELEASE`/`RETURN`, `USING`/`GIVING`,
> `INPUT`/`OUTPUT PROCEDURE`). La liste d'évitement en fin de document est à jour.
>
> **Mise à jour (passe de liquidation de la liste d'évitement — 1.7.0) :** les
> lacunes restantes sont désormais implémentées — **abréviation à objet
> identificateur** (`a = b OR c`, résolue via les métadonnées de niveau 88) ;
> **`INITIALIZE … REPLACING category DATA BY value`** ; **`66 RENAMES`** (la lecture
> synthétise / l'écriture répartit sur les éléments couverts) ; **pointeurs**
> (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`, `SET ADDRESS OF item TO …` en
> alias, `IF ptr = NULL`) ; **`ALTER`** / **`UNLOCK`** ; un **`NEXT SENTENCE`**
> fidèle ; les **intrinsèques** standard restantes (`PRESENT-VALUE`,
> `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`) ; et les
> **`ACCEPT`/`DISPLAY`** d'écran étendus (`AT`/`WITH` via ANSI en mode CLI —
> désormais *exécutés*, et pas seulement analysés).
>
> **Mise à jour (1.7.1) :** les sources de registre d'`ACCEPT` sont désormais
> fonctionnelles (c'étaient des no‑ops reconnus) — **`FROM COMMAND-LINE`**,
> **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`** (appariés avec
> `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`** (apparié avec
> `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** → `"00"`,
> **`CRT STATUS`** → `"0000"`.
>
> **Mise à jour (1.7.2) :** clauses de partage / verrouillage de fichiers et
> `CANCEL` (c'étaient ❌ / des no‑ops) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libère les verrous d'enregistrement
> INDEXED du fichier), et **`CANCEL program`** (réinitialise le stockage du
> programme).
>
> **Mise à jour (1.8.0) :** **`COMMIT` / `ROLLBACK`** sont désormais de vrais verbes
> COBOL — des transactions pilotées par le programme sur les fichiers INDEXED
> ouverts (moteurs mémoire et disque). Le moteur disque a gagné un vrai journal
> d'annulation pendant l'exécution (c'était un no‑op auparavant). La liste
> d'évitement en fin de document est à jour.

---

## Paragraphes de l'IDENTIFICATION DIVISION

- ✅ `PROGRAM-ID. name [IS] [COMMON] [INITIAL] [RECURSIVE] [PROGRAM].`
- ✅ Les paragraphes d'**entrée‑commentaire** — `AUTHOR`, `INSTALLATION`,
  `DATE‑WRITTEN`, `DATE‑COMPILED`, `SECURITY` — dans **n'importe quel ordre et
  n'importe quel sous‑ensemble**.
- ✅ `REMARKS` est également accepté. Il a été supprimé de COBOL en 1985, il n'est
  donc pas stocké ; il est accepté pour que du source repris de COBOL‑74 compile
  encore.

Une **entrée‑commentaire** est du texte libre, et COBOL‑85 l'entend littéralement :

```cobol
INSTALLATION.
    GENERAL SERVICES ADMINISTRATION
    AUTOMATED DATA AND TELECOMMUNICATION SERVICE.
    5203 LEESBURG PIKE  SUITE 1100
    FALLS CHURCH VIRGINIA 22041.
DATE-WRITTEN.
    CCVS-74 VERSION 4.0 - 1980 JULY 1.
```

- Elle peut contenir des **mots réservés** — le `DATA` ci‑dessus n'ouvre pas une
  DATA DIVISION.
- Elle peut contenir des **points**, et ne s'arrête pas à l'un d'eux.
- Elle **s'étend sur autant de lignes** que vous en écrivez.
- Elle se termine au prochain en‑tête de paragraphe ou de division qui **commence
  une ligne** en zone A — c'est ainsi que l'entrée ci‑dessus se termine à
  `DATE-WRITTEN`.

**Un guillemet dans cette prose reste contenu dans sa ligne** (depuis 1.62.12). Un
texte tel que `THE COMPILER"S ABILITY` n'ouvre plus un littéral qui se poursuit
dans tout le reste du programme — voir [Formats de source](#formats-de-source). Il
reste préférable d'éviter un guillemet non apparié dans une entrée‑commentaire,
mais cela vous coûte désormais cette ligne, et non le fichier.

⚠️ `INSTALLATION`, `SECURITY` et `REMARKS` **ne sont pas des mots réservés** ici.
Ils sont reconnus comme noms de paragraphe uniquement à l'intérieur de
l'IDENTIFICATION DIVISION, de sorte qu'une donnée nommée `SECURITY` continue de
fonctionner.

---

## Formats de source

RustCOBOL lit trois dispositions de source. Le choix est explicite — il n'est
**jamais** deviné d'après le contenu du fichier, car appliquer des règles de
colonnes à du source qui n'a pas été écrit pour elles supprime du code en silence.

| `--source-format` | Ce que cela signifie |
|---|---|
| `free` | Aucune règle de colonnes. `*>` ouvre un commentaire. **La valeur par défaut**, et ce qu'utilisent les projets de PowerRustCOBOL eux‑mêmes ainsi que les fichiers `.cbl` de formulaire générés. |
| `fixed` | ✅ **Format de référence classique de COBOL-85** — la disposition que la norme définit et dans laquelle le source en image de carte est écrit. Voir ci‑dessous. |
| `fixed-relaxed` | La zone de séquence et la colonne indicatrice sont respectées, mais la ligne va aussi loin que vous l'avez tapée — pas de limite à 72 colonnes. |
| `auto` | Comportement historique : `free`, sauf si `COBOLT_FIXED=1`. |

`COBOLT_SOURCE_FORMAT` fixe la valeur par défaut d'une session.

### `fixed` — le format de référence classique

```text
Col:  1     6 7  8   11  12                                      72 73    80
      |-----| |  |---|   |--------------------------------------- | |------|
      SeqNum  I  AreaA   Area B (active source)                    Ident
```

- **Colonnes 1-6** — zone de numéro de séquence, ignorée.
- **Colonne 7** — zone indicatrice :
  - `*` ou `/` → ligne de commentaire
  - `-` → **continuation** de la ligne précédente
  - `D` → ligne de débogage ; un commentaire (le mode de débogage n'est pas encore
    implémenté)
  - toute autre chose → lue comme du source ordinaire. La norme réserve cette
    colonne, mais les suites en image de carte l'utilisent comme sélecteur de
    lignes optionnelles, et écarter ces lignes en silence supprimerait du code.
- **Colonnes 8-72** — le source.
- **Colonnes 73-80** — zone d'identification, **écartée**.

### Lignes de continuation ✅

Un tiret en colonne 7 poursuit la ligne précédente.

**Poursuivre un mot ou un littéral numérique** — les espaces de fin de la ligne
poursuivie sont écartés et les deux moitiés se rejoignent sans rien entre elles :

```cobol
004700 01  WRK-DS-18V00-CONTIN
004800-    UED PICTURE X.
```

déclare un seul élément nommé `WRK-DS-18V00-CONTINUED`.

**Poursuivre un littéral alphanumérique** — le littéral de la ligne poursuivie n'a
pas de guillemet fermant ; la ligne de continuation doit rouvrir avec un, et le
littéral reprend au caractère suivant :

```cobol
011700     02 FILLER PICTURE IS X(54) VALUE IS "------------------------
011800-    "------------------------------".
```

⚠️ **Le fragment poursuivi va jusqu'à la colonne 72, espaces de fin inclus.** Une
ligne qui s'arrête avant la colonne 72 apporte tout de même ces espaces au
littéral. C'est pourquoi un littéral poursuivi n'est exact octet par octet que sous
`fixed` ; les autres formats n'ont pas de colonne 72 où s'arrêter.

### Un littéral ne franchit jamais une ligne par accident ✅

La continuation est le **seul** moyen pour un littéral de traverser des lignes. Un
guillemet qui n'est pas fermé sur sa propre ligne est une erreur, signalée là où
elle est écrite :

```text
unterminated alphanumeric literal — a literal cannot span source lines. In fixed
format, continue it on the next line with `-` in column 7 and reopen with the
same quotation mark; in free format there is no continuation, so the literal
must fit on one line.
```

Cela compte davantage qu'il n'y paraît. Avant 1.62.12 un guillemet non apparié
allait jusqu'au *prochain* guillemet n'importe où dans le fichier, de sorte qu'un
seul `"` égaré dans un commentaire avalait des divisions entières et décalait
l'appariement de tous les guillemets suivants — les programmes du NIST où cela a
été trouvé comportent un nombre **pair** de guillemets, donc rien n'était laissé
inachevé ; un seul caractère avait décalé la parité du fichier entier. Le dégât
s'arrête maintenant au passage à la ligne.

> **Le format libre n'a pas de continuation de littéraux.** Ni `&` — c'est
> l'*opérateur* de concaténation — ni un bloc délimité. Un littéral en format libre
> doit tenir sur une ligne ; pour un long, concaténez :
> `"first part" & "second part"`.

> **Note.** Choisir `fixed` pour un fichier écrit en format libre l'endommagera —
> tout ce qui dépasse la colonne 72 disparaît, et le texte avant la colonne 8 est lu
> comme un numéro de séquence. Ne le passez que pour du source qui est réellement en
> image de carte.

---

## Instructions reconnues (verbes)

✅ `MOVE` `ADD` `SUBTRACT` `MULTIPLY` `DIVIDE` `COMPUTE` `IF` `EVALUATE`
`PERFORM` `GO TO` `GOBACK`/`GO BACK` `CONTINUE` `EXIT` `STOP` `OPEN` `CLOSE`
`READ` `WRITE` `REWRITE` `DELETE` `START` `ACCEPT` `DISPLAY` `STRING` `UNSTRING`
`INSPECT` `CALL` `SET` `INITIALIZE` `SEARCH`/`SEARCH ALL` `SORT` `MERGE`
`RELEASE` `RETURN`
✅ `ALTER para-1 TO [PROCEED TO] para-2` (redirige le `GO TO` de para-1) ·
`UNLOCK file` (libère les verrous d'enregistrement du fichier) ·
`OPEN … SHARING/WITH LOCK` · `READ … WITH [NO] LOCK` (partage/verrouillage de
fichiers — indicatif au sein d'une unique unité d'exécution)
✅ `COMMIT` / `ROLLBACK` (transactions de fichiers INDEXED pilotées par le
programme — voir les verbes de fichier) · `CANCEL` (réinitialise le stockage du
programme) ·
✅ `INVOKE` — pilote les objets d'IHM/runtime (fenêtres, formulaires, méthodes de
contrôle) ; sans effet uniquement pour les objets **COBOL**, puisque les
définitions de classe/méthode sont hors périmètre
Extensions du projet : `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Un bloc peut faire `use` des crates toujours liés (std, egui, eframe et
l'ensemble de runtime lié) **plus n'importe quel crate que le projet enregistre
dans les Crates du projet** (spec 044) : les crates enregistrés sont figés à une
version exacte, copiés dans le `crates/` du projet et compilés dans le binaire ;
les crates non enregistrés font échouer Check/Build à la ligne du développeur, en
nommant le remède.

✅ `SEARCH` (série) / `SEARCH ALL` (recherche binaire sur une table avec
`ASCENDING`/`DESCENDING KEY` — exécute le premier `WHEN` qui correspond, sinon
`AT END`).
✅ `SORT` / `MERGE` avec `RELEASE` / `RETURN` (fonctionnels — voir ci‑dessous).
✅ `DECLARATIVES … END DECLARATIVES` avec `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — gestionnaires d'erreur de fichier
déclenchés par un `FILE STATUS` d'erreur non traité. Un gestionnaire **est entré
par le haut de sa section et s'exécute jusqu'à la fin de la section**, et ses
paragraphes conservent leurs noms, il peut donc leur faire `PERFORM` et `GO TO` —
y compris un paragraphe d'une *autre* section déclarative. Les paragraphes
déclaratifs vivent dans leur propre espace de noms : le contrôle ne tombe jamais du
corps principal à l'intérieur d'eux, et un nom déclaré dans les deux se résout vers
la copie du déclaratif pendant qu'un gestionnaire s'exécute et vers celle du corps
partout ailleurs. Un déclaratif peut aussi faire `PERFORM` d'un paragraphe de la
portion non déclarative.
❌ **Non reconnus — ne les utilisez pas :** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formes prises en charge par verbe

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (plusieurs récepteurs).
- ✅ **Un opérande de groupe rend tout le déplacement alphanumérique** (COBOL-85
  6.18.4). La PICTURE de l'autre opérande n'apporte que sa *taille* et rien de plus :
  pas d'édition, pas de dés‑édition, pas de conversion numérique.
  `MOVE <groupe contenant "123ABC">` laisse `"123ABC "` dans un `PIC 0XXXXX0` (et non
  l'édité `"0123AB0"`), les six mêmes caractères et une espace dans un
  `PIC 9999V999`, et `"12"` dans un `PIC 99`. `JUSTIFIED RIGHT` décide toujours quelle
  extrémité comble et quelle extrémité est perdue. La même règle régit les octets
  propres d'un groupe : chaque enfant prend sa tranche telle quelle, de sorte qu'un
  enfant alphanumérique‑édité **n'est pas** réédité.
- ✅ **Une clause `VALUE` sur un groupe** initialise les octets du groupe et est
  répartie sur ses enfants — `01 G VALUE "$123.45". 02 E PIC $999.99.` laisse `E`
  contenant `"$123.45"`.
- ✅ `MOVE CORRESPONDING g1 TO g2` — déplace chaque élément subordonné que les deux
  groupes partagent par nom, en descendant récursivement dans les sous‑groupes
  correspondants.
- ✅ **`CORRESPONDING` exclut un élément décrit avec `REDEFINES` ou `RENAMES`**
  (COBOL-85 6.18.4 GR1), de part et d'autre, ainsi que tout ce qui lui est
  subordonné. L'exclusion porte sur la *déclaration*, non sur le nom : un élément
  ordinaire qui partage simplement son nom avec un niveau 66 ailleurs correspond
  toujours.
- ✅ **L'un ou l'autre des opérandes de `CORRESPONDING` peut nommer une occurrence
  d'une table de groupes** — `MOVE CORRESPONDING C-LEVEL TO C-FLOCK (4)` écrit les
  emplacements propres de cette occurrence, et l'indice est répercuté dans la
  récursion.
- ✅ **Une paire n'a besoin que d'UN de ses deux éléments élémentaire.** Un groupe
  peut faire face à un élément élémentaire, et le déplacement entre eux est
  alphanumérique : un `PIC XXX` élémentaire envoyant vers un groupe de `999` + `XXX`
  remplit ses six caractères, et un groupe de `XXX` + `99` envoyant vers un `X(5)`
  simple le remplit. Deux groupes face à face **descendent** toujours
  récursivement — cet appariement n'est pas le cas élémentaire. *(Avant 1.62.39
  aucune des deux directions ne déplaçait rien du tout : un groupe ne possède pas
  d'emplacement de stockage, l'écriture allait donc là où rien ne la relit et la
  lecture donnait la chaîne vide.)*
- ✅ **Modification de référence `id(start:len)`** — émetteur (sous‑chaîne) et
  récepteur (affectation partielle raccordée) ; fonctionne sur les opérandes de tous
  les verbes. `length` est facultatif. Elle adresse des **positions de caractère**,
  de sorte qu'un opérande numérique est pris avec toute la largeur de sa `PIC` et ses
  zéros de tête : `01 T PIC 9(8) VALUE 00224845` donne `T(1:2)` = `"00"`, et non
  `"22"`.
- ✅ **Les éléments de groupe sont des agrégats alphanumériques** — un groupe *est*
  ses éléments subordonnés mis bout à bout, et sa taille est la somme des leurs. En
  lire un concatène les enfants (`FILLER` compris) ; déplacer vers un répartit les
  octets entre eux selon la largeur. `MOVE 11 TO A` est visible à travers le groupe
  qui contient `A`, et `MOVE "1234" TO G` fixe les enfants de `G`, non un emplacement
  propre.
- ✅ indices `t(i)`, `t(i, j)` — lisent/écrivent l'emplacement de stockage par
  occurrence ; les indices variables `t(WS-I)` sont évalués à chaque accès.
- ✅ qualification `id OF/IN group` (`… OF g1 OF g2`) — se résout vers le bon élément
  même lorsque le nom feuille est déclaré sous plus d'un groupe.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` par récepteur** — chaque récepteur porte son propre indicateur
  `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine chaque paire numérique
  correspondante, en descendant dans les sous‑groupes correspondants.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **plusieurs récepteurs `GIVING`**, chacun avec son propre `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sans `GIVING`) range `a/b` dans `a` (une commodité de
  PowerRustCOBOL ; le COBOL standard exige ici `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **plusieurs récepteurs, chacun avec son propre `ROUNDED`**.
- ✅ opérateurs d'expression `+ - * /` et `**` (puissance, associative à droite),
  parenthèses, `FUNCTION name(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] stmts [ELSE stmts] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO subject …]` … `WHEN {value | value THRU
  value | NOT value | condition | ANY} [ALSO …] stmts … [WHEN OTHER stmts]
  END-EVALUATE`.
- ✅ **multi‑sujet avec `ALSO`** — chaque colonne `WHEN` est comparée
  positionnellement à son sujet et combinée par AND.
- ✅ **`WHEN NOT value`** nie un objet de sélection ; **`WHEN condition`**
  (p. ex. `EVALUATE TRUE WHEN a > b`) évalue la condition booléenne.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = littéral entier ou donnée).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en ligne,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en ligne (sans paragraphe).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — exécute le paragraphe à
  chaque itération (hors ligne, sans `END-PERFORM`).
- ✅ **`WITH TEST AFTER` s'applique à `VARYING`**, écrit de part et d'autre de la
  clause et en ligne ou hors ligne. Le corps s'exécute une fois avant que quoi que ce
  soit ne soit testé, et les conditions sont alors testées **de l'intérieur vers
  l'extérieur** ; le niveau dont la condition est fausse est incrémenté, chaque
  niveau intérieur redémarre à sa valeur `FROM`, et le corps s'exécute de nouveau.
  Une variable n'est incrémentée que lorsque son test ressort faux, de sorte que le
  test qui termine la boucle la laisse telle que le corps l'a laissée.
- ✅ **Une variable `AFTER` est remise à sa valeur `FROM` lorsque sa boucle se
  termine**, avant que le niveau suivant vers l'extérieur ne soit incrémenté
  (COBOL-85 6.20.4 GR10(d)). Après le `PERFORM` complet, les variables intérieures
  lisent leurs valeurs `FROM` et seule la plus extérieure conserve la valeur qui l'a
  terminé.
- ✅ **Un identificateur `VARYING` indicé suit son indice.**
  `PERFORM p VARYING TBL (S1) FROM 10 BY INC (S2) UNTIL TBL (S1) > 70` incrémente
  l'occurrence que `S1` sélectionne à cet instant, de sorte qu'un corps qui fait
  avancer `S1` parcourt la table.

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p {OF|IN} section` · `GO TO p1 p2 … DEPENDING ON id` ·
  `GOBACK` / `GO BACK`.
- ✅ **Le qualificateur `{OF|IN} section` choisit quelle copie est visée** lorsqu'un
  nom de paragraphe se répète d'une section à l'autre, exactement comme sur
  `PERFORM`. Une section **inconnue** retombe sur la recherche non qualifiée plutôt
  que de perdre le saut. `GO TO … DEPENDING ON` prend une simple liste de noms et
  aucun qualificateur, et un `GO TO` qu'un `ALTER` a redirigé suit la
  redirection — laquelle nomme sa propre cible sans ambiguïté. *(Avant 1.62.39 le
  qualificateur était analysé puis ignoré, de sorte que le saut atterrissait sur la
  première définition n'importe où dans le programme.)*
- ✅ `CONTINUE` · `STOP RUN` · `STOP literal`.
- ✅ le `EXIT` simple est un point de retour sans effet ; `EXIT PROGRAM` rend le
  contrôle à l'appelant.
- ✅ `EXIT PERFORM [CYCLE]` (rompt / poursuit le PERFORM en ligne le plus proche),
  `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfère le contrôle au‑delà de la prochaine limite de phrase
  (l'analyseur insère des marqueurs de limite à chaque point ; fidèle, et non un
  simple `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnemonic}`.
- ✅ **`FROM mnemonic-name` lit l'opérateur** lorsque `SPECIAL-NAMES` déclare le
  mnémonique (`XXXXX057 IS ACCEPT-INPUT-DEVICE.` … `ACCEPT ACCEPT-D1 FROM
  ACCEPT-INPUT-DEVICE`) — c'est le Format 1, identique à un `ACCEPT id` simple. Un
  nom **qu'aucune clause `SPECIAL-NAMES` ne déclare** conserve l'extension de
  PowerRustCOBOL et lit la **variable d'environnement** de ce nom. Laquelle des deux
  s'applique est décidé par la déclaration, jamais par la graphie. *(Avant 1.62.35 la
  clause ordinaire `<implementor-name> IS <mnemonic>` était purement sautée, de sorte
  que chaque mnémonique lisait une variable d'environnement jamais définie et
  l'élément récepteur restait vide.)*
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` positionne le curseur (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (toute la ligne de commande) · `FROM ARGUMENT-NUMBER`
  (nombre d'arguments) · `FROM ARGUMENT-VALUE` (l'argument au pointeur fixé par
  `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (la variable nommée par
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.
- ✅ `END-ACCEPT` clôt l'instruction (facultatif).

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnemonic] [[WITH] NO ADVANCING] [END-DISPLAY]`.
- ✅ `END-DISPLAY` clôt la liste d'opérandes (facultatif), de sorte que
  `DISPLAY A END-DISPLAY DISPLAY B` fait deux instructions au lieu d'une.
- ✅ formes écran `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — exécutées via un positionnement
  de curseur ANSI + SGR en **mode CLI** (`rcrun`) ; ignorées en mode IHM (là, le Form
  Designer supplante les E/S de SCREEN). `ACCEPT id AT …` positionne puis lit.

### STRING
- ✅ `STRING {src [DELIMITED BY {SIZE | SPACE[S] | delim}]} … INTO target
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Débordement = la chaîne assemblée est plus large que le champ récepteur.
- ✅ **Une clause `DELIMITED BY` régit toute la série d'émetteurs qui la précède**,
  et pas seulement celui après lequel elle est écrite :
  `STRING "A0" "B0D" "C0X" DELIMITED BY ZERO INTO T` délimite les trois et construit
  `"ABC"`. Une instruction peut porter plusieurs clauses, chacune régissant les
  émetteurs depuis la précédente ; les émetteurs après la dernière clause sont pris
  en entier. *(Avant 1.62.40 seul l'émetteur écrit immédiatement avant la clause
  était délimité.)*
- ✅ **`INTO` un élément de groupe** répartit sur les éléments subordonnés du groupe.
- ✅ **Le résultat est assemblé octet par octet**, de sorte que `STRING HIGH-VALUE`
  déplace l'unique octet `0xFF` et occupe une position de caractère.
- ✅ **Extension — `DELIMITED BY` intelligent par défaut** (lorsqu'aucune clause ne
  régit un opérande) : les éléments alphanumériques `PIC X`/`A` prennent `SPACES` par
  défaut (le remplissage de fin est écarté) ; les littéraux de chaîne, les éléments
  numériques, les numériques‑édités, les résultats de `FUNCTION` et les expressions
  prennent `SIZE`. Les données sont déplacées sous leur forme de champ (numérique →
  chiffres sur toute la largeur de la PIC ; numérique‑édité → caractères édités).

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
- ✅ `BEFORE/AFTER INITIAL` confine chaque clause à une sous‑région du champ.
  (TALLYING accumule sur le compteur, conformément à COBOL.)
- ✅ **Une série d'opérandes TALLYING partage UN SEUL balayage de gauche à droite**
  (COBOL-85 6.17.3). À chaque position de caractère les opérandes sont essayés dans
  l'ordre où ils ont été écrits ; le premier qui correspond prend la position et le
  balayage reprend au‑delà des caractères qu'il a consommés. Ainsi
  `TALLYING t1 FOR ALL "AA" t2 FOR ALL "A"` sur `"AABA"` donne `t1 = 1, t2 = 1` —
  écrire les opérandes dans l'autre sens donne `t1 = 3, t2 = 0`. `LEADING` doit
  correspondre depuis le bord gauche de sa fenêtre sans écart, de sorte qu'un
  opérande antérieur prenant cette position met fin à la série avant qu'elle ne
  commence, et `CHARACTERS` ne compte que les positions qu'aucun opérande antérieur
  n'a réclamées.
- ✅ **Une série d'opérandes REPLACING partage UN SEUL balayage également**, par la
  même règle : le premier opérande qui correspond à une position remplace ces
  caractères et le balayage reprend au‑delà d'eux, de sorte qu'aucun opérande
  ultérieur ne peut les voir. La fenêtre `BEFORE`/`AFTER` de chaque opérande est
  fixée **avant tout remplacement**, ce qui permet d'ancrer un opérande sur des
  caractères qu'un opérande antérieur écrase :

  ```cobol
  MOVE "CAN NOT BE ALL BAD." TO SUBJ.
  INSPECT SUBJ REPLACING
      FIRST "L " BY "ZZ"  AFTER INITIAL "AL"
      FIRST "BAD" BY "ZZZ" AFTER "L "
      ALL   "." BY "Z"     AFTER "AL".
  *> SUBJ is now "CAN NOT BE ALZZZZZZ"
  ```

  Appliquées un opérande à la fois, la première clause effacerait le `"L "` sur
  lequel la seconde est ancrée, et `"BAD"` survivrait.
- ✅ **Un élément DISPLAY signé n'a pas de `-` parmi ses positions de caractère.** Le
  signe opérationnel est une surperforation sur un chiffre, de sorte que
  `INSPECT <PIC S9(5) contenant -12345> TALLYING c FOR ALL "-"` donne **0** tandis
  que `FOR ALL "5"` donne 1. Le signe est rétabli ensuite, de sorte qu'un `REPLACING`
  sur les chiffres le laisse tranquille. `SIGN IS … SEPARATE CHARACTER` est le cas où
  le signe *est* une position, et il est compté.

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilé en MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encodé en ADD / SUBTRACT).
- ✅ `SET 88-name TO TRUE` fixe l'élément hôte à la première VALUE de la condition ;
  `TO FALSE` fixe une valeur hors de l'ensemble des VALUE (au mieux — il n'y a pas de
  clause FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | other-ptr}` et
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — voir **Pointeurs**
  ci‑dessous.

### INITIALIZE
- ✅ `INITIALIZE id …` — conscient des catégories : numérique / numérique‑édité →
  ZERO, tout le reste → SPACES, en descendant dans les éléments de groupe.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY value …` — fixe chaque élément
  subordonné de cette catégorie à la valeur ; les autres restent intacts.

### Pointeurs (USAGE POINTER)
- ✅ `USAGE POINTER` déclare un pointeur (NULL au départ).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — fait de `id` un alias du
  stockage de la cible (les lectures **et** les écritures suivent l'alias) ;
  typiquement un enregistrement de LINKAGE. `IF ptr = NULL` fonctionne.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ Le corps de `ON EXCEPTION` / `ON OVERFLOW` s'exécute lorsque le programme appelé
  n'est pas résolu ; le corps de `NOT ON EXCEPTION` s'exécute lorsque l'appel **est
  résolu**.
- ✅ `CANCEL program …` réinitialise la WORKING-STORAGE du programme nommé, de sorte
  que son prochain `CALL` reparte de zéro.

### Verbes de fichier (les clauses prises en charge — la couverture complète est dans la suite d'E/S de fichiers)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]` ; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` sont analysés et respectés là où c'est pertinent —
  indicatifs dans le modèle à unique unité d'exécution.)
- ✅ **Un seul `OPEN` peut porter plusieurs groupes de mode**, chacun avec ses propres
  fichiers : `OPEN INPUT SQ-FS1, SQ-FS3 OUTPUT SQ-FS4.` Chaque groupe est ouvert dans
  son propre mode ; `SHARING` / `WITH LOCK` / `REGISTERED USER` s'appliquent à
  l'instruction.
- ✅ **Un `OPEN` d'un fichier déjà ouvert vaut `41`**, et le fichier est laissé tel
  qu'il était — l'instruction ne le **rouvre pas**. (Rouvrir un fichier `OUTPUT`
  tronquerait en silence ce que le programme avait déjà écrit.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extension
  PowerRustCOBOL) — consigne l'opérateur/utilisateur dans le journal d'observabilité
  INDEXED (champ `user=` sur chaque ligne d'événement de la session de ce fichier).
  Purement observationnel ; pas d'authentification/autorisation. Voir
  [`observability-fr.md`](observability-fr.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libère le verrou d'enregistrement que le moteur INDEXED prend sous
  I‑O.
- ✅ **`READ … INTO id` est le `READ` suivi d'un `MOVE` de groupe.** L'enregistrement
  est réparti sur les éléments subordonnés du récepteur selon la largeur et coupé à la
  largeur propre du récepteur, le récepteur peut être indicé, et le déplacement
  transporte des octets — un enregistrement contenant un octet qui n'est pas un
  caractère arrive intact.
- ✅ **Clause `RECORD` de la FD — enregistrements de longueur variable.** Les trois
  graphies : `RECORD CONTAINS n CHARACTERS` (fixe),
  `RECORD CONTAINS n TO m CHARACTERS` (variable ; la description d'enregistrement que
  le `WRITE` nomme donne la longueur), et
  `RECORD [IS] VARYING [IN SIZE] [FROM n] [TO m] [CHARACTERS] [DEPENDING ON id]` (la
  donnée *est* la longueur — fixée avant un `WRITE`, remise par un `READ`, et bornée à
  la plage déclarée). Une FD dont les enregistrements `01` diffèrent en taille est à
  longueur variable, qu'elle le dise ou non. Un fichier à longueur variable stocke la
  longueur de chaque enregistrement avec l'enregistrement, de sorte que ses octets
  **ne sont pas** interchangeables avec ceux d'un fichier à longueur fixe ; un fichier
  à longueur fixe est inchangé.
- ✅ **Les enregistrements `01` d'une FD décrivent une seule zone d'enregistrement.**
  Un `READ` livre les octets à travers toutes les descriptions d'enregistrement ; un
  `WRITE` envoie la zone entière, de sorte que ce qu'une autre description
  d'enregistrement a placé là où celle écrite a du `FILLER` se voit en transparence.
- ✅ **`FILLER` occupe ses octets dans un enregistrement de FD**, et
  `SIGN IS SEPARATE CHARACTER` rend un élément DISPLAY signé plus large d'un caractère
  que ses positions de chiffre.
- ✅ **Le `LINAGE` d'une FD accepte des noms de données autant que des entiers** —
  `LINAGE LINAGE-CTR FOOTING FOOT-CTR TOP TOP-CTR BOTTOM BOTTOM-CTR`. La page est
  mesurée à partir de ces éléments à chaque `WRITE`, de sorte qu'un programme peut la
  redimensionner en cours d'exécution. `LINAGE-COUNTER` vaut un lorsque le fichier est
  ouvert.
- ✅ **Un `READ` séquentiel après `AT END` vaut `46`, et non un second `10`.** Le
  `AT END` n'a laissé aucun enregistrement suivant valide, de sorte que continuer à
  lire est une erreur différente d'atteindre la fin. `46` est un état de classe 4, de
  sorte que ni `AT END` ni `NOT AT END` ne s'exécutent pour lui — c'est le déclaratif
  `USE` du fichier qui le traite. Un `OPEN` neuf, ou un `START` réussi, rétablit un
  enregistrement.
- ✅ `UNLOCK f [RECORD[S]]` libère les verrous d'enregistrement du fichier.
- ✅ **`COMMIT` / `ROLLBACK`** — transactions pilotées par le programme sur **tous**
  les fichiers INDEXED ouverts. `OPEN` ouvre une transaction ; `COMMIT` confirme les
  `WRITE`/`REWRITE`/`DELETE` en attente (un `ROLLBACK` ultérieur ne peut plus les
  annuler) et en ouvre une nouvelle ; `ROLLBACK` annule tous les changements depuis le
  dernier `COMMIT`/`OPEN`. Le stockage **DISK** rend `COMMIT`/`CLOSE` durables sur
  disque. Le stockage **MEMORY** garde `COMMIT`/`ROLLBACK` purement en RAM (n'écrit
  jamais sur disque) ; un fichier `STORAGE IS MEMORY` simple est éphémère, et
  `STORAGE IS MEMORY WITH PERSISTENCE` n'enregistre sur disque qu'au `CLOSE`. (La
  reprise après panne via un journal d'écriture anticipée durable reste à faire —
  ceci est une annulation au niveau du programme, en cours d'exécution.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (fichiers INDEXED ; extension PowerRustCOBOL). Le stockage par
  défaut est `DISK`. `WITH COMPRESSION` comprime l'enregistrement stocké (les clés
  sont évaluées sur l'enregistrement non comprimé) ; `WITH PERSISTENCE` (MEMORY
  seulement) enregistre le fichier en RAM au `CLOSE`. `OPEN OUTPUT` (re)crée toujours
  le conteneur sur disque.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]` ;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ **`REWRITE` sur un fichier SEQUENTIAL d'enregistrements** remplace
  l'enregistrement que le dernier `READ` a livré, sur place, et laisse la position de
  lecture où elle était — le `READ` suivant donne toujours l'enregistrement qui suit.
  Les états qu'il doit : **`49`** lorsque le fichier n'est pas ouvert en `I-O`,
  **`43`** lorsqu'aucun `READ` réussi n'a établi d'enregistrement (y compris après
  `AT END`, et sur un second `REWRITE` sans `READ` entre les deux), et **`44`** lorsque
  le nouvel enregistrement n'a pas la même longueur que celui lu — sur un fichier avec
  `DEPENDING ON` la valeur de l'élément est cette longueur, c'est ainsi qu'un programme
  en demande une autre.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ Le partage de fichiers entre *processus* n'est pas imposé (unique unité
  d'exécution) ; les clauses `SHARING`/`LOCK` sont analysées et les verrous
  d'enregistrement par exécution du moteur INDEXED sont respectés.

### SORT / MERGE / RELEASE / RETURN  ✅ (fonctionnels, tampon de travail en mémoire)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dans une INPUT PROCEDURE) ajoute à l'exécution ;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` restitue les
  enregistrements.
- Les enregistrements sont triés de façon stable selon les clés déclarées
  (`ASCENDING`/`DESCENDING`) ; `USING` lit / `GIVING` écrit les fichiers séquentiels
  nommés.

---

## Conditions (IF / EVALUATE / PERFORM UNTIL)

- ✅ Symboles relationnels : `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relations en mots : `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe : `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
  Un élément dont la PICTURE **ne porte pas de signe opérationnel** n'est `NUMERIC`
  que lorsque toutes ses positions de caractère contiennent un chiffre — un
  `PIC X(5)` contenant `"+1234"`, `"1.234"` ou `"12 45"` **n'est pas** numérique.
  *(Avant 1.62.40 le test analysait les caractères comme un nombre, de sorte qu'un
  signe, un point décimal, un exposant et les espaces autour étaient tous acceptés.)*
- ✅ **Un opérande de `CLASS` défini par l'utilisateur peut être une position
  ordinale** — `CLASS ORDINAL-A-ONLY IS 66` nomme le 66ᵉ caractère du jeu natif — et
  l'opérande peut occuper sa propre ligne de source. Il en va de même pour
  `ALPHABET`.
- ✅ Signe : `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nom‑condition de niveau 88 (le nom seul comme condition).
- ✅ **`TRUE` / `FALSE` comme opérandes** (extension PowerRustCOBOL) — du sucre pour
  `1` et `0`, partout où une valeur est permise : `IF x = TRUE`,
  `IF x IS [NOT] FALSE`, `IF x NOT TRUE` (la forme avec `NOT` seul, sans opérateur
  relationnel), `PERFORM UNTIL x = FALSE`, `MOVE TRUE TO x`,
  `COMPUTE n = n + TRUE`, `INVOKE obj "m" USING TRUE`, et `WHEN TRUE` face à un sujet
  de valeur. Un `TRUE`/`FALSE` seul est aussi une condition complète (`IF TRUE`,
  `PERFORM UNTIL TRUE`).
  ⚠️ Cela **ne change pas** les deux endroits où ces mots signifiaient déjà quelque
  chose : `SET <88‑name> TO TRUE` fixe toujours l'élément hôte à une valeur qui
  satisfait la condition (et non au nombre 1), et `EVALUATE TRUE`/`EVALUATE FALSE`
  ci‑dessous restent l'instruction à cas standard.
- ✅ `AND` / `OR` / `NOT` combinés, parenthèses (AND lie plus fort que OR).
- ✅ **Conditions abrégées à opérateur antéposé** — `a > 1 AND < 9`, `a = 5 OR = 7`
  (le sujet de comparaison précédent est réutilisé).
- ✅ **Abréviation à objet littéral** — `a = 1 OR 2 OR 3` (réutilise à la fois le
  sujet et l'opérateur ; l'objet est un littéral).
- ✅ **Abréviation à objet identificateur** — `a = b OR c` (où `c` est une donnée).
  Un identificateur seul après AND/OR à la suite d'une comparaison est résolu à
  l'exécution : un nom‑condition de niveau 88 connu s'évalue comme tel, sinon c'est
  l'objet `a = c`. (Un identificateur immédiatement suivi de `AND` conserve la
  précédence de AND.)
- ✅ **Un `NOT` devant l'*objet* d'une abréviation nie la relation**, et non l'objet :
  `a > b OR NOT c` vaut `a > b OR NOT (a > c)`. La graphie
  `NOT <opérateur relationnel>` (`AND NOT < x`) est la forme opérateur et ne change
  pas, et un `NOT` qui ouvre une condition ordinaire — `NOT (…)`, `NOT x = y`,
  `NOT x NUMERIC` — conserve son propre sens. *(Avant 1.62.42 la forme objet était lue
  comme « l'objet est non nul », ce qui donne la même réponse seulement lorsque
  l'objet contient justement zéro.)*
- ✅ **Un nom‑condition déclaré sur un groupe teste les octets du groupe.** Un groupe
  ne possède pas de stockage propre — il *est* ses enfants — de sorte que
  `01 T. 88 B VALUE "ABCABC". 02 A PIC XXX. 02 B2 PIC XXX.` compare face aux six
  caractères que l'enregistrement contient.
- ✅ **Une constante figurative est répétée jusqu'à la taille de l'autre opérande**,
  et cela inclut une constante écrite comme la `VALUE` d'un 88 : `88 B VALUE QUOTE`
  sur un hôte `PIC X(4)` fait quatre guillemets, et `88 D VALUE ALL "BAC"` fait
  `"BACB"`. `ALL literal` est dimensionné dans **les deux** directions —
  `IF X EQUAL TO ALL "BA"` sur un `X` de dix caractères compare face à
  `"BABABABABA"`, et non à `"BA"` complété d'espaces.

---

## Expressions, littéraux, USAGE

- ✅ Opérateurs arithmétiques `+ - * /` et `**` ; parenthèses ; `+`/`-` unaires.
- ✅ `FUNCTION name ( arg [ , arg … ] )` — intrinsèques **implémentées** :
  `LENGTH, UPPER-CASE, LOWER-CASE, NUMVAL, NUMVAL-C, MAX, MIN, SQRT, MOD, REM,
  ABS, INTEGER, INTEGER-PART, RANDOM (avec germe facultatif), CURRENT-DATE, TRIM, REVERSE, CONCATENATE,
  ORD, CHAR, ORD-MAX, ORD-MIN, SUM, MEAN, MEDIAN, MIDRANGE, RANGE, VARIANCE,
  STANDARD-DEVIATION, FACTORIAL, SIN, COS, TAN, ASIN, ACOS, ATAN, LOG, LOG10,
  EXP, EXP10, PI, STORED-CHAR-LENGTH, WHEN-COMPILED, INTEGER-OF-DATE,
  DATE-OF-INTEGER, INTEGER-OF-DAY, DAY-OF-INTEGER, FRACTION-PART, ANNUITY,
  PRESENT-VALUE, YEAR-TO-YYYY, BYTE-LENGTH, LENGTH-AN, NUMVAL-F, TEST-NUMVAL`.
  (Les conversions de date utilisent la base standard 1601‑01‑01 = jour 1.)
  L'**ensemble complet des intrinsèques standard de COBOL‑85** est implémenté.
- ✅ **Les registres de date et d'heure lisent l'horloge LOCALE.**
  `ACCEPT … FROM DATE / TIME / DAY / DAY-OF-WEEK` et `FUNCTION CURRENT-DATE`
  rapportent tous l'heure propre de la machine, et non UTC — y compris la date, qui
  diffère de part et d'autre de minuit. Les cinq derniers caractères de
  `CURRENT-DATE` portent le décalage **réel** par rapport à GMT (`…-0300`), de sorte
  qu'un programme peut savoir dans quel fuseau il s'exécute.
  ✅ Un nom de `FUNCTION` non reconnu est une **erreur de compilation** qui nomme la
  fonction, avec une suggestion lorsqu'une vraie est assez proche pour être une faute
  de frappe probable. Auparavant il était analysé et renvoyait **0** à
  l'exécution, ce qui transformait une faute d'orthographe en une réponse fausse
  énoncée avec assurance (1.62.15).
- ✅ Littéraux : entier, décimal, chaîne, toutes les constantes figuratives
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Une constante figurative remplit tout son récepteur**, y compris
  `HIGH-VALUE` — `MOVE HIGH-VALUE TO <PIC X(10)>` fait dix octets `0xFF`, et vers un
  groupe elle est répartie sur les enfants. Un récepteur alphanumérique‑édité place
  toujours ses caractères d'insertion, de sorte qu'un `PIC XX0XXBXXX` contient
  `FF FF '0' FF FF ' ' FF FF FF`. Sous une `PROGRAM COLLATING SEQUENCE` la constante
  nomme un caractère ordinaire et c'est ce caractère qui remplit.
  ⚠️ `HIGH-VALUE` est l'**octet** `0xFF`, et non un caractère. La lecture d'un
  opérande de groupe, l'édition et tous les chemins de déplacement le transportent
  octet par octet, mais **la modification de référence n'est pas encore exacte à
  l'octet** — `IF X (1:1) = HIGH-VALUE` est faux pour un élément qui contient
  véritablement `0xFF`.
- ✅ **Un littéral numérique peut commencer par le point décimal** — `.5`, `-.5`,
  `.000000001`. COBOL‑85 exige seulement qu'un littéral ne *finisse* pas par un, de
  sorte que `5.` reste le nombre 5 suivi d'un terminateur de phrase.
  ```cobol
  77  A05ONES  PICTURE SV9(5)  VALUE .11111.
      COMPUTE WS-NUM = FUNCTION ACOS(.999).
      IF WRK-DU-5V1-1 = .1  PERFORM PASS-PARA.
  ```
  Les zéros de tête sont significatifs et exacts : `.000000001` est un milliardième,
  et non un dixième. Sous `DECIMAL-POINT IS COMMA` la même chose vaut pour `,5`. Ce
  qui sépare le littéral d'un point de fin de phrase est l'**absence d'une espace** —
  COBOL‑85 en exige une après un terminateur, de sorte que `MOVE X TO Y.` n'est jamais
  lu comme le début d'une fraction, et `MOVE X TO Y.5` est une erreur de compilation
  plutôt qu'une réinterprétation silencieuse.
- ✅ **Signalement de conformité** (`cobolt_semantic::flagging`) — la norme demande
  qu'une implémentation conforme soit capable de dire à un programme lesquelles des
  fonctionnalités qu'il utilise sortent d'un niveau de conformité choisi. Deux
  analyses y répondent :
  - `flag_obsolete` — l'ensemble des **éléments obsolètes** de COBOL‑85 : les cinq
    paragraphes facultatifs de l'IDENTIFICATION DIVISION, `MEMORY SIZE`, `ALTER`,
    `STOP` avec un littéral, et `GO TO` sans nom de procédure.
  - `flag_high_subset` — tout ce qui est au‑dessus du **sous‑ensemble haut**, depuis
    `COMPUTE`, `EVALUATE` et `INITIALIZE` en passant par `CORRESPONDING`, la
    modification de référence, la qualification, `SET … TO TRUE` et un quatrième
    indice, jusqu'à la continuation d'un *mot* ou d'un *littéral numérique* par‑delà
    la limite d'une carte. (Continuer un littéral **alphanumérique** est dans le
    sous‑ensemble et n'est pas signalé.)

  Ni l'une ni l'autre n'est une vérification d'erreurs, et aucune ne s'exécute lors
  d'une compilation ordinaire : chaque construction qu'elles nomment est du COBOL‑85
  valide que RustCOBOL implémente et exécute. Ce sont des points d'entrée distincts
  précisément pour qu'une compilation normale ne se mette jamais à avertir au sujet
  d'`AUTHOR` ou de `COMPUTE`. Les NIST `NC302M`, `NC303M` et `NC401M` les
  valident — 7, 4 et 40 signalements, tous concordants.
- ✅ **`SPECIAL-NAMES. CURRENCY [SIGN] [IS] literal`** — le caractère qui remplit une
  position monétaire dans une PICTURE éditée. Il **remplace** `$` au lieu de s'y
  ajouter, de sorte que dès qu'un programme en déclare un, `$` n'est plus un caractère
  de picture à cet endroit :
  ```cobol
  SPECIAL-NAMES.
      CURRENCY "<".
  ...
  01  FL-LESS  PICTURE <(3),<<<.99  VALUE " <1,111.11".
  ```
  `MOVE ZERO TO FL-LESS` se lit alors `      <.00`, et `MOVE 1234` se lit
  ` <1,234.00` — la série flottante se comporte exactement comme `$$$,$$$.99`. Un
  symbole monétaire qui est une **lettre** fonctionne de la même façon :
  `CURRENCY SIGN IS "W"` fait de `PICTURE WWWWW` une chaîne monétaire flottante de
  cinq positions, de sorte que `MOVE 12` se lit `  W12`. *(Avant 1.62.40 une série
  d'un symbole‑lettre était lue comme un seul mot et rejetée, de sorte que seul `$`
  flottait.)* Le littéral doit faire un caractère, et COBOL‑85 en interdit un qui
  entrerait en collision avec un caractère de picture ou un séparateur : pas un
  chiffre, aucun de `A B C D E G N P R S V X Z`, et aucun de
  `space * + - , . ; ( ) " / =`.
- ✅ **Littéraux hexadécimaux** — `X"09"`, `x'0D0A'` (dans l'une ou l'autre casse,
  avec l'un ou l'autre guillemet). Un caractère par **paire** de chiffres
  hexadécimaux, le nombre de chiffres doit donc être pair ; un nombre impair ou un
  chiffre non hexadécimal est un littéral mal formé et est signalé, et non relu
  discrètement comme le mot `X` à côté d'une chaîne. Utilisables partout où un
  littéral entre guillemets l'est (`DELIMITED BY`, `MOVE`, `VALUE`, comparaisons).

---

## Clauses de la DATA DIVISION (syntaxe de déclaration acceptée)

- ✅ Niveaux `01`–`49`, `77`, `88` ; `FILLER` ; groupe/élémentaire. Le mot `FILLER`
  est **facultatif** — `05 PIC X VALUE ":".` en déclare un tout comme
  `05 FILLER PIC X VALUE ":".`, et dans les deux cas il contient ses octets et sa
  `VALUE` à l'intérieur du groupe qui le contient.
- ✅ `PIC/PICTURE` avec `X A 9 S V P` et les symboles d'édition (`Z * $ + - CR DB B 0 /
  , .`). Le symbole monétaire est `$` sauf si `SPECIAL-NAMES. CURRENCY` en a nommé un
  autre — voir **Expressions, littéraux, USAGE** ci‑dessus. **`P` est une position de
  cadrage décimal** — une position de chiffre que l'élément couvre mais ne stocke
  pas : `PIC S999PP` contient trois chiffres représentant des centaines
  (`MOVE 12300` le stocke exactement ; `MOVE 12345` stocke 12300), et `PIC PP99` en
  contient deux représentant des dix‑millièmes. Les positions que les `P` occupent se
  relisent toujours comme zéro et ne prennent **aucun octet** dans la disposition d'un
  enregistrement.
- ✅ **La protection par astérisques remplit tout l'élément.** Une valeur nulle dans
  une picture dont les positions de chiffre sont toutes `*` remplit chaque position de
  caractère d'astérisques — les chiffres fractionnaires, les virgules de groupement,
  un `$` fixe, et un `CR` ou `DB` final pareillement — ne laissant que le point
  décimal lui‑même : un `PIC $**.**CR` contenant zéro se lit `***.****`, et un
  `PIC *,***.**` se lit `*****.**`. Une valeur **non** nulle ne protège que les zéros
  de tête, de sorte que le `$` fixe garde sa propre position (`-2.34` → `$*2.34CR`).
  *(Avant 1.62.37 `CR`/`DB` apportait un astérisque au lieu des deux positions de
  caractère qu'il occupe, de sorte qu'un tel élément revenait plus court d'un caractère
  que sa propre largeur.)*
- ✅ **Un littéral numérique déplace ses caractères, tels qu'écrits.** Vers un
  récepteur alphanumérique un littéral apporte les chiffres que le programme a tapés,
  cadrés à gauche et complétés d'espaces — `MOVE 2 TO <PIC X(4)>` donne `"2   "`, et
  `MOVE 060820000200 TO <six enfants PIC 99>` les remplit `06 08 20 00 02 00`. La
  largeur du **récepteur** ne complète jamais le littéral ; seule sa propre largeur
  écrite le fait. *(Avant 1.62.38 le lexeur ne gardait que la valeur, de sorte qu'un
  zéro de tête était perdu et chaque caractère suivant se décalait d'une place vers la
  gauche.)*
- ✅ **Une relation entre un opérande numérique et un opérande non numérique est non
  numérique** (COBOL‑85 VI‑89 6.15.4 GR2). L'opérande numérique est traité comme s'il
  avait été déplacé vers un élément alphanumérique de **sa propre taille**, ce qui
  transfère ses positions de caractère et **non son signe opérationnel** : un
  `PIC S9(18)` contenant `-123456789012345678` compare comme **égal** à un `PIC X(18)`
  contenant `"123456789012345678"`. Trois conditions bornent la règle — l'opérande
  numérique doit être un **entier** ; « non numérique » est décidé par la
  **déclaration**, de sorte qu'un enfant `PIC 99` contenant des caractères après un
  `MOVE` de groupe est toujours numérique — et un **groupe** est non numérique quels
  que soient ses enfants, de sorte qu'un `PIC 9(5)` contenant 12345 face à un groupe de
  dix octets contenant `"0000012345"` donne `"12345     "` et l'inégalité ; et
  `ALL literal` prend la taille de l'autre opérande. *(Avant 1.62.38 la comparaison
  était algébrique dès que le côté texte se laissait analyser comme un nombre.)*
- ✅ **Troncature de poids fort sur un MOVE numérique.** Un récepteur contient
  exactement ses chiffres déclarés aux deux extrémités :
  `01 M PIC 99V999.  MOVE 123.45 TO M.` laisse `23.450`. L'arithmétique teste d'abord
  la capacité du récepteur, de sorte qu'une instruction avec `ON SIZE ERROR` conserve
  au contraire son ancienne valeur.
- ✅ **Une table de groupes est adressée par occurrence.** `MOVE VALUES-1 TO
  GRP-1 (2)` répartit sur les enfants propres de cette occurrence
  (`ELEM1 (2,1) … ELEM1 (2,4)`), et lire `GRP-1 (2)` concatène exactement ceux‑là.
  L'enregistrement `01` englobant représente les octets de **toutes** les occurrences,
  de sorte que `MOVE GRP-TAB1 TO GRP-TAB2` copie une table entière.
- ✅ **Les noms d'index, les littéraux et l'indexation relative se mélangent comme
  indices.** `ELEM1 (IN1, 1)`, `ELEM1 (1 IN2)`, `ELEM1 (IN1 +3)` — un signe collé à
  ses chiffres est un littéral signé qui ouvre l'indice suivant — et
  `ELEM1 (IN1 - 1, 3)`, où l'opérateur est espacé des deux côtés, est de l'indexation
  relative.
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (et `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérique/signé/alphanumérique/figuratif/`ALL`). **`VALUE ALL
  "literal"` répète son motif sur tout l'élément** — `PIC X(6) VALUE ALL "ABC"` donne
  `"ABCABC"` et `PIC X(9) VALUE ALL "XY"` donne `"XYXYXYXYX"`. *(Avant 1.62.40 seules
  les constantes figuratives d'un caractère remplissaient leur élément et
  `ALL "literal"` le laissait contenant des espaces.)*
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES` — une seconde lecture **vivante** des mêmes octets. Elle n'ajoute
  aucun stockage (elle n'élargit donc pas le groupe qui la contient), et une écriture à
  travers l'une ou l'autre description est visible à travers l'autre :
  `03 RESULT-A PIC X(6). 03 RESULT-N REDEFINES RESULT-A PIC 9(6).` —
  `MOVE 123456 TO RESULT-N` se relit ensuite à travers `RESULT-A`.
  ⚠️ **Réserve :** un recouvrement de plus de 256 emplacements de stockage développés
  (une table 10×10×10 redéfinie, par exemple) conserve au contraire un stockage par
  description — la rafraîchir à chaque écriture parcourrait mille occurrences deux
  fois.
- ✅ **Les recouvrements s'imbriquent.** Un `REDEFINES` à l'intérieur d'un
  enregistrement lui‑même redéfini est atteint dans les deux directions, aussi profond
  soit‑il : écrire deux octets à travers une redéfinition de niveau 01 atteint
  l'enregistrement redéfini, le `REDEFINES` d'un groupe à l'intérieur, et le
  `REDEFINES` d'un élément à l'intérieur de *celui‑ci* — y compris un 88 déclaré sur le
  plus interne. Chaque description est reconstituée une fois par écriture. *(Avant
  1.62.42 une clé appartenant à plus d'un recouvrement ne gardait que celle déclarée en
  dernier, et un unique garde arrêtait la chaîne après son premier saut.)*
- ✅ **Une description sans nom reste une description.**
  `02 FILLER REDEFINES <item>.` redécrit les octets de sa cible sous aucun nom propre,
  et une écriture dans la cible est visible à travers ses enfants. Plusieurs enfants se
  partagent ces octets, dans l'ordre de disposition — le recouvrement n'est *pas* un
  alias de son premier enfant. Deux `FILLER REDEFINES` d'un même élément font deux
  lectures indépendantes, chacune commençant au **premier** octet de la cible. *(Avant
  1.62.36 un groupe redéfinissant sans nom ne recevait aucune clé de stockage, de sorte
  que ses enfants se lisaient comme des espaces quoi qu'on eût mis dans la cible.)*
- ✅ **Un nom dupliqué à l'intérieur d'un recouvrement** se résout vers le même
  stockage que le reste du programme atteint : `TAB-A` déclaré sous deux groupes
  différents garde une lecture par déclaration. *(Avant 1.62.36 la copie initiale du
  recouvrement était indexée depuis un chemin auquel manquaient ses qualificateurs
  extérieurs, ce que seul un nom dupliqué peut distinguer — de sorte que précisément le
  cas qui a besoin du qualificateur le perdait.)*
- ✅ `JUSTIFIED [RIGHT]` — **stocke cadré à droite**, sur un élément *alphanumérique*
  ou *alphabétique*. Un émetteur plus étroit que le récepteur est complété à gauche ;
  un émetteur plus large garde son extrémité **droite**, perdant ses caractères les
  plus à gauche — l'inverse de la règle ordinaire. *(Avant 1.62.40 la clause n'était
  enregistrée que pour les éléments alphanumériques, de sorte que
  `PICTURE A(5) JUSTIFIED RIGHT` était analysée puis cadrait à gauche comme n'importe
  quel autre élément.)*
- ✅ `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL` — acceptés ;
  `SIGN … SEPARATE` ne change pas encore la façon dont l'élément est stocké.
- ✅ **Un `REDEFINES` au niveau 01 peut décrire plus de stockage que l'élément qu'il
  redéfinit**, et les octets au‑delà de la fin de cet élément appartiennent à celle des
  descriptions qui est assez longue pour les nommer. Écrire à travers une description
  plus courte laisse la queue de la plus longue tranquille.
- ✅ **Un recouvrement `REDEFINES` transporte les octets de l'élément redéfini**, y
  compris vers un pair numérique : un recouvrement `PIC S9(18)` d'un `X(18)` contenant
  `"00ABCDEFGHI  4321 "` relit ces caractères, et `IS NUMERIC` répond **non** pour eux.
  Lorsque les octets forment effectivement des chiffres, la lecture numérique est
  inchangée.
- ✅ `88 name VALUE v [v …]` / `VALUE a THRU b` — de **vrais noms‑conditions** : le
  niveau 88 se lie à son élément hôte ; le test vérifie l'hôte face aux VALUE /
  plages, et `SET 88-name TO TRUE` range dans l'hôte une valeur qui les satisfait.
- ✅ **Un nom‑condition peut être déclaré sous plus d'un groupe, et `OF`/`IN` les
  distingue** — exactement comme pour un nom de donnée, et les niveaux intermédiaires
  peuvent être omis :
  ```cobol
  IF EQUALS-M OF TABLE-LEVEL-5 OF TABLE-LEVEL-4
           IN TABLE-LEVEL-3 OF TABLE-LEVEL-2
           OF GROUP-1-TABLE (13)   *> occurrence 13 of THIS table's host
  ```
  L'indice appartient à l'élément hôte, il sélectionne donc face à quelle occurrence
  les VALUE sont testées. Une référence **non qualifiée** à un nom‑condition dupliqué
  est ambiguë en COBOL‑85 ; le runtime prend la première déclaration, la même règle
  qu'il applique à un nom de donnée ambigu.
- ✅ `USAGE INDEX` déclare un registre d'index entier (`SET`/`SEARCH` l'utilisent) ;
  `USAGE POINTER` — voir **Pointeurs** ci‑dessus.
- ✅ `66 NEW RENAMES item-1 [{THRU|THROUGH} item-2]` — un alias de regroupement ; la
  lecture concatène les éléments couverts, l'écriture répartit selon la largeur de
  champ.
  - ✅ **Un 66 est qualifié par l'enregistrement qu'il regroupe**, exactement comme une
    donnée est qualifiée par le groupe au‑dessus d'elle, de sorte que le même nom de 66
    peut être déclaré une fois par enregistrement et distingué avec `OF`/`IN` :
    `MOVE "CALIFORNIA" TO RENAME-5 OF T-RENAMES-DATA`. Cela fonctionne autant en
    lecture qu'en écriture, et un 66 l'emporte sur une donnée ordinaire qui partagerait
    son nom. Les opérandes de la clause `RENAMES` se résolvent dans ce même
    enregistrement, de sorte qu'un `NAME-2` dupliqué nomme celui de cet
    enregistrement.
  - ✅ **Une table couverte apporte toutes les occurrences**, et pas seulement la
    première : `66 R RENAMES ITEM-1 THRU TABLE-2`, où `TABLE-2` contient
    `03 T PIC XXX OCCURS 5`, fait 20 caractères de large.
  - ✅ **Un 66 sur exactement un élément *est* cet élément** — même PICTURE, même
    catégorie, même stockage. `66 R RENAMES W` où `W` est `PIC 9(4)` est un élément
    numérique à quatre chiffres, de sorte que `ADD 3500 TO R` avec 8000 dedans
    déclenche `ON SIZE ERROR` et le laisse inchangé.
- Sections : `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE` ; `SCREEN` est
  analysée mais non exécutée.

---

## Toujours PAS pris en charge — liste d'évitement actuelle

> **Corrigé le 2026‑08‑25.** Cette section commençait par « L'ensemble des verbes /
> clauses de COBOL‑85 est **entièrement couvert**. » Exécuter la suite NIST CCVS85 l'a
> démenti : **102 des 434 programmes dans le périmètre ont échoué ce jour‑là**, sur
> des constructions que ce document ne listait pas comme des lacunes — virgules et
> points‑virgules séparateurs, `FUNCTION x(ALL)`, `CLOSE … WITH LOCK`, `COPY` en
> zone B, entrées de commentaire d'IDENTIFICATION, numéros de priorité de section,
> noms de données commençant par un chiffre et — jusqu'à 1.62.10 — littéraux
> numériques à point décimal initial. C'est à cela que sert une suite de validation.
> Chaque lacune est désormais spécifiée dans
> [`specs/nist/`](../specs/nist/README.md) et suivie dans le
> [tableau de bord](#-la-conformité-est-mesurée-non-affirmée--nist-ccvs85) ci‑dessus.

La liste ci‑dessous est ce qui sort du périmètre **à dessein**, par opposition aux
lacunes du NIST ci‑dessus, qui sont des défauts en cours de traitement :

1. **Édition de saisie dans l'`ACCEPT` d'écran** — `DISPLAY … AT/WITH` et
   `ACCEPT … AT` sont exécutés (ANSI) en mode CLI, mais l'édition complète de la
   SCREEN SECTION au niveau du champ (tabulation automatique, validation de champs,
   cartes de couleurs) est **supplantée par le concepteur de formulaires** en mode
   IHM.
2. **Partage de fichiers entre *processus*** — `OPEN … SHARING/WITH LOCK`,
   `READ … WITH [NO] LOCK` et `UNLOCK` sont analysés et pilotent les verrous
   d'enregistrement par exécution du moteur INDEXED, mais les verrous ne sont pas
   imposés entre processus distincts du système d'exploitation (modèle à unique unité
   d'exécution).
3. **COBOL orienté objet** (définitions de classe/méthode) — `INVOKE` est sans effet
   pour les objets COBOL (il ne pilote que les objets d'IHM/runtime).
4. ✅ **Résolu (1.62.15).** Un nom de fonction intrinsèque non reconnu renvoyait **0**
   en silence, de sorte qu'un programme calculait avec assurance une réponse fausse à
   partir d'une faute de frappe. C'est désormais une **erreur de compilation** qui
   nomme la fonction et suggère la vraie la plus proche lorsqu'il y a une
   correspondance suffisamment proche (`cobolt-semantic/src/resolver.rs`,
   `Expr::FunctionCall`). Conservé ici parce que la forme du « zéro silencieux » est le
   piège que les points 5 et 6 portent encore.
5. ⚠️ **Une valeur invalide d'`ACCESS MODE` / `ORGANIZATION` est avalée sans
   diagnostic** — le même piège à nouveau, et celui‑ci est déclenché par une faute de
   frappe ordinaire de l'utilisateur. `ACCESS MODE IS` n'accepte que `SEQUENTIAL`,
   `RANDOM` ou `DYNAMIC` (`INDEXED` est une *organisation*, non un mode d'accès), mais
   l'analyseur de la clause SELECT teste ces trois‑là et laisse tout le reste tomber
   dans la branche générique « sauter un jeton inconnu », de sorte que le fichier
   conserve en silence le `SEQUENTIAL` par défaut et se comporte mal à l'exécution au
   lieu de ne pas compiler. `ORGANIZATION IS` a la forme identique
   (`cobolt-parser/src/parser.rs`, la branche `Token::Access` et la branche
   d'organisation au‑dessus d'elle). Les deux devraient lever une erreur claire à la
   compilation en nommant le mot fautif. **Aucun module du NIST n'attrapera jamais
   ceci** — la suite n'écrit que des clauses valides, de sorte que chaque module peut
   finir à 100 % avec la lacune toujours ouverte. C'est un piège à faute de frappe
   utilisateur, et il lui faut un test propre plutôt qu'une note de module.
6. ⚠️ **`ALPHABET … IS EBCDIC` est accepté mais laisse l'ordonnancement natif (ASCII)
   en vigueur.** La clause littérale (`"A" THRU "H" "I" ALSO "J" …`), `NATIVE`,
   `STANDARD‑1` et `STANDARD‑2` sont toutes implémentées et pilotent réellement
   `PROGRAM COLLATING SEQUENCE` ; seule la table EBCDIC manque, et la nommer donne
   discrètement l'ordre ASCII. Même famille de pièges que 4–6.
7. **Le module Communication et le Report Writer** — voir
   [N/A ci‑dessus](#-na--ce-qui-sort-du-périmètre-de-rustcobol-et-pourquoi).

> **Résolu (1.5.0) :** le modèle de données plat est devenu hiérarchique / conscient
> des occurrences, débloquant **CORRESPONDING**, les **noms qualifiés**,
> l'**indiçage de tables** et **`SEARCH`**.
> **Résolu (1.6.0) :** `MULTIPLY`/`DIVIDE` à plusieurs récepteurs + `ROUNDED` par
> récepteur ; `EXIT PERFORM/PARAGRAPH/SECTION` ; `CALL NOT ON EXCEPTION` ;
> `INSPECT TALLYING REPLACING` combiné + `BEFORE/AFTER INITIAL` ; intrinsèques de
> date/`ANNUITY` ; abréviation à objet littéral ; `EVALUATE ALSO`/`WHEN NOT` ; vrais
> noms‑conditions de niveau 88 ; `PERFORM para VARYING` ; et le runtime
> `SORT`/`MERGE` avec `RELEASE`/`RETURN`.
> **Résolu (1.7.0) :** abréviation à objet identificateur ;
> `INITIALIZE … REPLACING` ; `66 RENAMES` ; pointeurs (`USAGE POINTER`,
> `SET ADDRESS OF` / `TO ADDRESS OF` / `NULL`) ; `ALTER` / `UNLOCK` ;
> `NEXT SENTENCE` fidèle ; les intrinsèques standard restantes ; et les
> `ACCEPT`/`DISPLAY` d'écran étendus (exécutés en mode CLI).
> **Résolu (1.7.1) :** `ACCEPT FROM COMMAND-LINE / ARGUMENT-NUMBER /
> ARGUMENT-VALUE / ENVIRONMENT-VALUE / ESCAPE KEY / CRT STATUS` (avec les registres
> appariés `DISPLAY … UPON ARGUMENT-NUMBER / ENVIRONMENT-NAME`).
> **Résolu (1.7.2) :** `OPEN … SHARING/WITH LOCK`, `READ … WITH [NO] LOCK`,
> `UNLOCK` (libère les verrous d'enregistrement INDEXED) et `CANCEL program`.
> **Résolu (1.8.0) :** `COMMIT` / `ROLLBACK` comme transactions de fichiers INDEXED
> pilotées par le programme (moteurs mémoire et disque ; vrai journal d'annulation sur
> disque).

.<<

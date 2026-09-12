<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.135 -->

# Matrice de prise en charge PowerRustCOBOL

**À quoi sert ce document :** un seul endroit parcourable du regard, qui répond
à *« PowerRustCOBOL fait-il X, et X est-il du COBOL normalisé ou quelque chose
qu'ajoute cette plateforme ? »*. Chaque capacité est une ligne. Aucune liste en
prose — si une chose est prise en charge, elle a une ligne que l'on peut
montrer.

Ceci est la **vue d'ensemble**. Le détail est porté par deux documents compagnons :

| Document | Ce qu'il répond |
|---|---|
| [`cobol85-supported-syntax-en.md`](cobol85-supported-syntax-en.md) | **Quelle graphie** de chaque instruction le lexeur, l'analyseur et le runtime acceptent réellement, et le tableau de conformité NIST CCVS85 |
| [`cobol85-verb-test-matrix-fr.md`](cobol85-verb-test-matrix-fr.md) | **Quoi tester** pour chaque verbe |
| [`developers-guide-en.md`](developers-guide-en.md) | Comment construire des applications avec tout cela |

---

## Comment lire les tableaux

Chaque ligne de capacité est marquée contre trois origines, puis reçoit un état.

| Colonne | Signification |
|---|---|
| **85** | Défini par **COBOL-85** (ANSI X3.23-1985, y compris l'amendement de 1989 sur les fonctions intrinsèques là où c'est signalé) |
| **20xx** | Défini par une **norme ISO ultérieure** — COBOL 2002 / 2014 / 2023, et ce qui est actuellement rédigé pour 2026 |
| **PRC** | Une **extension PowerRustCOBOL** — absente de toute norme COBOL |
| **État** | Ce que cette implémentation en fait |

Une capacité peut être marquée dans plus d'une colonne d'origine : une
fonctionnalité COBOL-85 qu'une norme ultérieure a étendue porte `●` dans les deux,
et la colonne **Notes** dit ce que la norme ultérieure a ajouté.

**Marques d'origine :** `●` défini ici · `○` étendu ou clarifié ici · `—` absent
de cette norme.

**Marques d'état :** `✅` pris en charge · `🚧` partiel ou simplifié · `⛔`
prévu, pas encore implémenté · `🚫` hors périmètre par conception, ne sera jamais
implémenté.

> **Note d'honnêteté.** PowerRustCOBOL vise un sous-ensemble pratique, orienté
> applications, plus des extensions RAD visuelles. Ce n'est **pas** une
> implémentation COBOL-85 certifiée. La conformité est *mesurée* contre la suite
> officielle NIST CCVS85 plutôt qu'affirmée — voir le
> [tableau de bord](cobol85-supported-syntax-en.md).

---

## 1. Format de source et structure du programme

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Source en format fixe, **assoupli** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **La valeur par défaut.** La zone de séquence et la colonne indicatrice sont respectées, mais la ligne va jusqu'où le développeur a tapé — pas de coupure en colonne 72. Les `.cbl` générés des formulaires et les blocs `EXEC RUST` en ont besoin |
| Source en format fixe, **format de référence classique COBOL-85** (`--source-format=fixed`) | ● | ○ | — | ✅ | Toutes les règles de colonnes appliquées : 1–6 séquence, 7 indicateur (`*` `/` commentaire, `-` continuation, `D` ligne de débogage), 8–72 source, **73–80 abandonnées**, et le raccordement standard des continuations, y compris un littéral alphanumérique continué. C'est dans ce format qu'est écrite la suite d'images de cartes NIST CCVS85. **Choisi explicitement, jamais par détection** — appliquer ces règles à une source qui n'a pas été écrite pour elles supprime du code en silence |
| Source en format libre | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Sélecteur de format de source — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | Également `COBOLT_SOURCE_FORMAT` ; `auto` inspecte les premières lignes et ne choisit jamais le format strict |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ |  |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ |  |
| DATA DIVISION | ● | ○ | — | ✅ |  |
| PROCEDURE DIVISION | ● | ○ | — | ✅ |  |
| Programmes imbriqués | ● | ○ | — | ✅ |  |
| Plusieurs unités de programme séquentielles dans un même fichier | ● | ○ | — | ✅ |  |
| Copybooks `COPY` / `REPLACE` | ● | ○ | — | ✅ | Remplacement de pseudo-texte et de mots, `COPY` imbriqué, `REPLACE OFF` ; résout `.cpy`/`.cbl`/`.cob` à côté de la source, sans distinction de casse |
| Paragraphe `REPOSITORY` | — | ● | ○ | ✅ | COBOL 2002 pour les classes ; PowerRustCOBOL y lie aussi les types de **FFI Rust** |
| Rust en ligne avec `EXEC RUST … END-EXEC` | — | — | ● | ✅ | Compilé dans le binaire ; les erreurs sont signalées à la ligne et à la colonne COBOL du développeur lui-même |

## 2. La DATA DIVISION et la description des données

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ |  |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ |  |
| FILE SECTION | ● | ○ | — | ✅ |  |
| SCREEN SECTION | ● | ○ | — | 🚧 | Les `ACCEPT`/`DISPLAY` étendus avec `AT`/`WITH` s'exécutent via ANSI en mode CLI ; l'édition d'écran champ par champ est remplacée par le concepteur visuel de formulaires en mode GUI |
| COMMUNICATION SECTION (`CD`, contrôle de messages) | ● | — | — | 🚫 | Télétraitement ; obsolète dans les normes ultérieures |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Hors périmètre par conception |
| `PICTURE` X / A / 9 / S / V avec répétition `(n)` | ● | ○ | — | ✅ |  |
| PICTURE numérique éditée (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Suppression des zéros, protection par astérisques, `$` et signes fixes et flottants |
| `USAGE DISPLAY` | ● | ○ | — | ✅ |  |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ |  |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Virgule flottante ; une extension d'éditeur normalisée plus tard sous `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ |  |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Binaire natif ; extension d'éditeur |
| `USAGE INDEX` | ● | ○ | — | ✅ |  |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002 ; alias en lecture **et** en écriture |
| `OCCURS` fixe | ● | ○ | — | ✅ |  |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ |  |
| `INDEXED BY` | ● | ○ | — | ✅ |  |
| Numéros de niveau 01–49, 77 | ● | ○ | — | ✅ |  |
| Niveau 66 `RENAMES` | ● | ○ | — | ✅ |  |
| Noms-conditions de niveau 88 | ● | ○ | — | ✅ | Y compris `SET … TO TRUE` |
| Clause `VALUE` | ● | ○ | — | ✅ |  |
| Éléments de groupe, `FILLER` | ● | ○ | — | ✅ |  |
| `REDEFINES` | ● | ○ | — | ✅ |  |
| Constantes figuratives (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ |  |

## 3. La PROCEDURE DIVISION — les verbes

| Verbe | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Appariement des sous-champs de groupe |
| `DISPLAY` | ● | ○ | — | ✅ | Le numérique est rendu sur toute la largeur de la PIC |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ |  |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (incl. `CORRESPONDING`) | ● | ○ | — | ✅ | Plusieurs récepteurs, `ROUNDED` par récepteur |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Plusieurs récepteurs, `ROUNDED` par récepteur |
| `COMPUTE` | ● | ○ | — | ✅ | Plusieurs récepteurs, `ROUNDED` par récepteur |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ |  |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ |  |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ |  |
| `PERFORM` en ligne, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ |  |
| `PERFORM para VARYING` (hors ligne) | ● | ○ | — | ✅ |  |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ |  |
| `ALTER` | ● | ○ | — | ✅ | Élément obsolète en COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Sémantique fidèle ; obsolète en COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ |  |
| `EXIT` | ● | ○ | — | ✅ |  |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ |  |
| `GOBACK` | — | ● | — | ✅ | Extension d'éditeur normalisée en COBOL 2002 |
| `SET` (incl. `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ |  |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | Pointeurs COBOL 2002 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Sensible à la catégorie, descend dans les groupes |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ |  |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` combiné |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Pilote l'index de la table, exécute le premier `WHEN` qui correspond, sinon `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` et `RETURNING` relèvent de COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ |  |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ |  |
| `INVOKE` | — | ● | ○ | 🚧 | OO de COBOL 2002. Pris en charge pour les **objets d'interface et d'exécution et les greffons FFI Rust** ; les définitions de classe et de méthode par l'utilisateur ne sont pas implémentées |
| `UNLOCK` | — | ● | — | 🚧 | Pilote les verrous d'enregistrement au sein de l'exécution ; non imposé entre processus du système |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Transactions pilotées par le programme sur les fichiers INDEXED, avec un vrai journal d'annulation |
| Définitions OO `CLASS-ID` / `METHOD-ID` | — | ● | — | ⛔ | Prévu |

## 4. Conditions et expressions

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Conditions de relation, de classe, de signe et de nom-condition | ● | ○ | — | ✅ |  |
| Relations combinées abrégées, opérateur en tête (`a > 1 AND < 9`) | ● | ○ | — | ✅ |  |
| Relations combinées abrégées, objet littéral (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ |  |
| Relations combinées abrégées, objet identificateur (`a = b OR c`) | ● | ○ | — | ✅ |  |
| Modification de référence `item(start:length)` | ● | ○ | — | ✅ | Lecture **et** écriture par découpe, sur n'importe quel opérande |
| Indiçage de table à l'exécution `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Stockage par occurrence, indices variables |
| Noms qualifiés `id OF/IN group` | ● | ○ | — | ✅ | Une feuille déclarée sous plus d'un groupe se résout vers des stockages indépendants |
| Comparaison alphanumérique correcte au sens COBOL (complétée par des espaces) | ● | ○ | — | ✅ |  |
| **Arithmétique exacte en virgule fixe** | ● | ○ | ○ | ✅ | Mantisse entière `i128`, sans aller-retour par `f64` : la précision standard de 18 chiffres et l'**étendue à 31 chiffres** restent exactes |
| Expressions de propriété concises (`Output::Value`) | — | — | ● | ✅ | Lire ou fixer une propriété de contrôle à l'intérieur d'une formule, sans aucun élément temporaire de working-storage |

### 4.1 Méthodes de valeur sur un élément de données

`item::Method(args)` appelle une méthode sur la **valeur d'un élément de données
ordinaire** — un champ `PIC X`, un groupe, une occurrence de table, une tranche en
modification de référence ou une expression arithmétique — et pas seulement sur un
contrôle. Rien de tout cela n'est du COBOL normalisé.

Utilisable partout où une expression l'est : comme source d'un `MOVE`, dans un
`COMPUTE`, à l'intérieur d'une condition et en ligne dans un `DISPLAY`. Les
méthodes **se chaînent** : `WS-TEXT::Trim()::Len()`.

| Méthode | Renvoie | État | Notes |
|---|---|:--:|---|
| `Trim()` | texte | ✅ | Espaces de tête et de queue supprimés |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | texte | ✅ | Trois graphies acceptées d'une même méthode |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | texte | ✅ |  |
| `Replace(from, to)` | texte | ✅ | Toutes les occurrences |
| `Len()` · `Length()` | numérique | ✅ | La longueur **du champ** : un `PIC X(20)` contenant `hello` répond donc `20`. Chaînez `::Trim()::Len()` pour la longueur du contenu |
| `Split(sep)` | texte | ✅ | Le **premier** champ |
| `Split(sep)(n)` | texte | ✅ | Le *n*-ième champ, à partir de 1. L'indice n'est accepté que sur un récepteur qui est un élément de données |

| Récepteur | État | Notes |
|---|:--:|---|
| Élément de données (`PIC X`, groupe, `01`/`77`) | ✅ | Le cas ordinaire |
| Occurrence de table, modification de référence, nom qualifié, expression arithmétique | ✅ | Accepté par l'évaluateur |
| **Literal** (`"a-b-c"::Split("-")`) | ⛔ | L'interpréteur accepte un récepteur littéral, mais l'analyseur non : un `::` après un littéral est une erreur de syntaxe. Affectez d'abord le littéral à un élément de données |

### 4.2 Une expression là où COBOL-85 n'admet qu'un élément

COBOL-85 restreint la plupart des positions émettrices à un identificateur ou à
un littéral. RustCOBOL y évalue à la place une expression complète, et c'est ce qui
supprime l'élément de working-storage de fortune que la norme oblige à déclarer.

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. La norme n'autorise qu'un identificateur ou un littéral comme champ émetteur |
| `SET target TO <expression>` | — | — | ● | ✅ | Équivalent à la forme `COMPUTE` ; la cible peut être un élément de données ou une propriété de contrôle en position de lvalue |
| `STRING <expression> … INTO` | — | — | ● | ✅ | Un élément émetteur peut être une expression arithmétique (`STRING WS-N * 2 …`) ou un appel de méthode de valeur (`STRING WS-A::UpperCase() …`) ; `DELIMITED BY` et le reste restent normalisés |
| **Inférence de type** — lire `Ctrl::Property` donne une valeur typée de première classe | — | — | ● | ✅ | Le type numérique ou texte circule dans l'expression : une propriété entre donc directement dans une arithmétique, une condition ou une position émettrice **sans aucun élément `PIC` entre les deux** : `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. La valeur d'une propriété qui ressemble à un nombre est relue comme un nombre, si bien que comparaisons et arithmétique restent algébriques plutôt que caractère par caractère |

## 5. Fonctions intrinsèques

Le jeu d'intrinsèques COBOL-85 est arrivé avec l'**amendement de 1989** (ANSI
X3.23a-1989) ; les fonctions ajoutées par COBOL 2002 et au-delà sont marquées dans
la colonne `20xx`. Toutes celles ci-dessous sont implémentées.

| Groupe | Fonctions | 85 | 20xx | PRC | État |
|---|---|:--:|:--:|:--:|:--:|
| Longueur et caractères | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Longueur et caractères (ultérieures) | `BYTE-LENGTH`, `LENGTH-AN`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Casse et texte | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| Texte (ultérieures) | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| Conversion numérique | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| Conversion numérique (ultérieures) | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| Arithmétique | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| Ordonnancement | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| Statistiques | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| Trigonométrie et logarithmes | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| Combinatoire | `FACTORIAL` | ● | ○ | — | ✅ |
| Financières | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| Date et heure | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. E/S fichier — organisations et accès

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Enregistrements de longueur fixe |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Texte terminé par un saut de ligne ; les espaces de fin sont abandonnés à l'écriture |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Moteur ISAM intégré et sans dépendance |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Moteur propre (`cobolt-runtime/src/relative.rs`, conteneur `PRCREL1`, disque et MEMORY). `RELATIVE KEY IS` adresse les enregistrements par numéro entier à partir de 1 ; les trois modes d'accès ; les sept verbes fichier s'y aiguillent. **Le module RL du NIST est terminé sur les deux axes** — 35/35 en compilation, 34/34 en exécution, 354 assertions, 0 échec (moteur 1.62.76, module 1.62.77) |
| `RELATIVE KEY IS data-name` (y compris la graphie sans `KEY`) | ● | ○ | — | ✅ | Une clause `RELATIVE data-name` où le mot `KEY` est omis est bien la clé, et non une simple clause d'organisation |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | Les trois s'exécutent |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Ordre de clé ascendant sur le disque |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ |  |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ |  |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` relève de COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ |  |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Y compris `GREATER/LESS THAN` et `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ |  |
| Codes `FILE STATUS` | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Analysé et porté par l'instruction, mais **indicatif** — il n'y a qu'une unité d'exécution, donc rien n'entre en concurrence |
| `OPEN … WITH LOCK` (ouvrir le fichier en exclusivité) | — | ● | — | 🚧 | Idem : accepté et indicatif dans le modèle à unité d'exécution unique |
| `READ … WITH LOCK` | — | ● | — | ✅ | Le moteur détient déjà l'enregistrement sous `I-O` ; la phrase énonce l'intention |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | Relâche réellement le verrou que le moteur prend sous `I-O` — c'est aujourd'hui la seule phrase de verrouillage ayant un effet à l'exécution. `UNLOCK` est au §3 avec les autres verbes |
| Partage de fichiers entre processus et application des verrous d'enregistrement | — | ● | — | ⛔ | Prévu ; le modèle actuel est à unité d'exécution unique |

## 7. E/S fichier — le moteur INDEXED (PowerRustCOBOL)

Tout ce que contient cette section est une extension de la plateforme autour du
comportement normalisé d'`ORGANIZATION IS INDEXED` ci-dessus. Le détail est dans
[`indexed-file-format-fr.md`](indexed-file-format-fr.md),
[`indexed-file-internals-fr.md`](indexed-file-internals-fr.md) et
[`indexed-redb-engine-fr.md`](indexed-redb-engine-fr.md).

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **Le mode de stockage par défaut.** Enregistrements et index vivent dans le fichier de l'`ASSIGN` et sont lus à la demande, si bien que la RAM reste bornée même sur de très gros fichiers. Servi par le moteur redb résistant aux pannes depuis 1.62.73 ; on atteint encore l'ancien B+tree paginé avec `--indexed-engine rust` |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | Le fichier entier en RAM, persisté sur le chemin de l'`ASSIGN` à la fermeture |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | RLE sans dépendance ; écrase bien au-delà de 50 % les plages de remplissage typiques des enregistrements COBOL |
| `COMMIT` / `ROLLBACK` pilotés par le programme | — | — | ● | ✅ | Vrai journal d'annulation, sur les moteurs mémoire et disque |
| Verrouillage d'enregistrement au sein d'une unité d'exécution | — | ○ | ● | ✅ | Voir la réserve sur les processus ci-dessus |
| Moteur sélectionnable (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | Également `COBOL_INDEXED_ENGINE` ; tous compatibles en comportement. **`redb` est la valeur par défaut** depuis 1.62.73 (`cobolt-runtime/src/indexed.rs:126`) |
| Moteur ACID `redb` résistant aux pannes | — | — | ● | ✅ | OPEN en O(1) (~5 ms à 200 k enregistrements), RAM de l'ensemble de travail (≥250 M d'enregistrements), survit à une coupure de courant sans corruption d'index |
| Conteneur autodescriptif `PRCIDX1` | — | — | ● | ✅ | Intègre le format d'enregistrement et les descripteurs de clé ; la validation stricte à l'ouverture transforme une divergence de schéma en `39` et un fichier absent en `35`. Pas compatible octet à octet avec Fujitsu |
| Journal de transactions par fichier (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt ou NDJSON prêt pour Grafana/Loki — voir [`observability-fr.md`](observability-fr.md) |

## 8. Intégrations du runtime

Atteintes depuis COBOL par des `CALL` d'exécution et par `INVOKE`. Rien de tout
cela n'est du COBOL normalisé ; c'est ce qui rend le langage utilisable pour des
applications modernes.

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | Une seule et même surface de CALL pour les trois ; le backend est choisi d'après la chaîne de connexion. **Aucune bibliothèque système** — rien n'est lié depuis l'hôte — mais « Rust pur » n'est vrai que de deux des trois : `postgres` et `mysql` le sont, tandis que `rusqlite` est épinglé avec `features = ["bundled"]` et compile l'**amalgame C de SQLite** via `libsqlite3-sys`. (Cette compilation C est aussi la raison pour laquelle `test_external_crates_e2e` échoue par intermittence à l'intérieur d'un `cargo build` imbriqué.) Voir [`database-runtime-fr.md`](database-runtime-fr.md) |
| **Jeux de résultats SQL** — `Fetch()`, `ColumnNames()`, `ColumnCount()`, `ColumnName(n)` | — | — | ● | ✅ | `Fetch()` renvoie la ligne suivante séparée par des tabulations, et vide une fois épuisée : elle termine donc sa propre boucle ; `ColumnNames()` nomme le jeu de résultats dans l'ordre du SELECT, même s'il n'a rapporté aucune ligne. La surface `CALL` lit au contraire la ligne courante colonne par colonne, par indice — les deux parcours ne doivent pas être mélangés sur un même descripteur |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | En-têtes personnalisés |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ |  |
| **Graphiques** — barres / lignes / secteurs / aires / nuage / anneau | — | — | ● | ✅ | Liés à des tables COBOL |
| **Fichiers texte** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ |  |
| **Minuteries** | — | — | ● | ✅ |  |
| **Point d'accroche objet pour agent IA** | — | — | ● | ✅ |  |
| **Greffons FFI Rust** | — | — | ● | ✅ | Modules déclarés sous `REPOSITORY`, aiguillés par `INVOKE` ou par des correspondances directes de propriétés |
| **Procédures utilisateur** | — | — | ● | ✅ | Procédures COBOL partagées, éditables dans l'IDE et appelables par `CALL "PROCEDURE-NAME"` |

## 9. Explicitement hors périmètre

Ces éléments ne seront pas implémentés. Ils sont listés pour que la réponse soit
trouvable plutôt qu'absente.

| Capacité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, contrôle de messages / télétraitement) | ● | — | — | 🚫 | Obsolète dans les normes ultérieures ; aucun usage moderne |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Remplacé par les rapports et la liaison de données propres à la plateforme |
| Contrôles ActiveX / OLE / COM | — | — | — | 🚫 | Spécifiques à une plateforme et non portables |

---

## 10. La plateforme elle-même

Il ne s'agit pas de fonctionnalités du langage COBOL mais de l'IDE, du
compilateur et de l'outillage qui les entoure. Parcours complet dans le
[guide du développeur](developers-guide-en.md).

### 10.1 L'IDE

| Capacité | État | Notes |
|---|:--:|---|
| Concepteur visuel de formulaires | ✅ | Toile de conception avec plusieurs thèmes (**Liquid Glass**, **Cobalt Steel**), aimantation à la grille, redimensionnement par glissement des contrôles et de la toile, alignement en sélection multiple et ordre de profondeur |
| Moteur de rendu unifié | ✅ | Parité au pixel entre le concepteur, l'aperçu, l'application en cours d'exécution et le binaire compilé |
| Catalogue de contrôles | ✅ | **43 widgets** répartis entre Common, Container, Data, Graphics, Menu, Non-visual et Charts, plus un type `Custom` fourni par greffon |
| Rayon d'angle universel et détourage arrondi | ✅ | Les enfants imbriqués se détourent sur la bordure arrondie du parent par un masquage à encoches d'angle |
| `Transparency` par contrôle | ✅ | 0 = opaque … 100 = transparent ; atténue la face, le cadre et l'ombre tandis que le texte, les glyphes et la bordure restent lisibles. Les libellés qui passent sous le seuil WCAG AA face à ce qui est derrière basculent vers le pôle qui se lit |
| Widget Animator | ✅ | Rend nativement les **GIF / WebP / APNG** |
| Knob, Gauge, Switch, FileDropZone, Maps et Web Search | ✅ | Molette rotative à remplissage bipolaire ; KPI radial, linéaire ou en anneau avec zones d'alerte et critiques automatiques ; glisser-déposer ou sélecteur natif |
| Éditeur de menus avancé | ✅ | Éditeur visuel en arbre, **1112** icônes vectorielles intégrées réparties en 37 catégories, imbrication hiérarchique et signatures HMAC d'intégrité de la configuration |
| Liaison de données et tableaux de contrôles | ✅ | Liaison directe à des sources SQL et de données ; les **groupes répétitifs visuels** déploient des tableaux de GroupBox et de Panel à partir du nombre de lignes du `DataSource` à l'exécution |
| Validation visuelle et inspecteur de formulaires | ✅ | Badges d'erreur en temps réel pour les gestionnaires malformés, les liaisons incomplètes et les anomalies de mise en page ; le gestionnaire de processus de `rcrun` suit en direct le pourcentage de CPU, la RSS, les journaux et le nombre de fils |
| Débogueur de formulaires | ✅ | Fenêtre autonome toujours au premier plan : points d'arrêt, pas à pas entrant/sortant/principal, inspecteur de variables et relecture animée à 1–10 lignes par seconde |
| Maillage d'assistants IA agentiques | ✅ | Orchestrateur de LLM **rig-core** (Ollama, OpenAI, Groq, Alibaba Model Studio et autres API en nuage) exécutant le Dev Agent, l'Editor Assistant et le History Compactor, avec un journal d'observabilité en direct et des relevés de jetons `↑input ↓output` |
| Grace, l'orchestratrice | ✅ | Décompose une demande, achemine chaque tâche vers le spécialiste qui en a la charge et impose un **relecteur Pedantic** un pour un — aucun spécialiste n'approuve son propre travail |
| Base de connaissances découpée avec RAG | ✅ | Indexée à raison d'un enregistrement par sujet ; livrée déjà vectorisée, GPU avec repli CPU qui ne chauffe pas, et **File → Reindex Knowledge Bases** |
| Cycle de vie des formulaires et fenêtrage | ✅ | Un **formulaire principal** désigné démarre l'application ; l'habillage et l'état de chaque formulaire sont respectés ; `OpenFormSync`/`OpenFormAsync` ; la position de la fenêtre est une propriété de conception ; effets d'entrée et de sortie par projet |
| Exécution multifenêtre | ✅ | Écrans d'aperçu et d'exécution dans des viewports propres au système (multi-viewport egui) |
| Interface internationalisée | ✅ | 6 langues d'interface : anglais, espagnol, portugais, japonais, chinois et français |
| Sélecteur de polices système | ✅ | N'importe quelle police installée, rendue dans sa propre typographie et appliquée en direct au concepteur, aux aperçus et aux formulaires en cours d'exécution |
| Boîtes de dialogue de fichiers natives et non bloquantes | ✅ | Ouvrir, enregistrer et parcourir sans bloquer la boucle d'événements de l'interface |

### 10.2 Le compilateur

| Capacité | État | Notes |
|---|:--:|---|
| Sortie en un seul binaire natif | ✅ | Sérialise l'AST avec `bincode` + `flate2`, l'intègre avec tous les formulaires via `include_bytes!`, compile avec `cargo build --release` et émet un binaire dans `bin/` — **sans aucune source `.cbl` incluse** |
| Mentions de redistribution | ✅ | `bin/` reçoit automatiquement `LICENSE`, `NOTICE` et la mention du runtime, de sorte que les distributions portent les mentions exigées par Apache-2.0 |
| Vrais diagnostics `rustc` en cas d'échec de compilation | ✅ | Un échec de compilation rapporte les diagnostics du compilateur lui-même, pas une ligne de résumé |

.<<

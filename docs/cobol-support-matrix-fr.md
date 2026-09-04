<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Matrice de prise en charge de PowerRustCOBOL

**À quoi sert ce document :** un seul endroit lisible d'un coup d'œil qui répond
à la question *« PowerRustCOBOL fait-il X, et X relève-t-il du COBOL standard ou
est-ce quelque chose que cette plateforme ajoute ? »* Chaque fonctionnalité est
une ligne. Pas de listes en prose — si une chose est prise en charge, elle a une
ligne que l'on peut montrer du doigt.

Ceci est la **vue d'ensemble**. Deux compagnons en portent le détail :

| Document | Ce à quoi il répond |
|---|---|
| [`cobol85-supported-syntax-fr.md`](cobol85-supported-syntax-fr.md) | **Quelle écriture** de chaque instruction le lexer/parser/runtime acceptent réellement, et le tableau de bord de conformité NIST CCVS85 |
| [`cobol85-verb-test-matrix-fr.md`](cobol85-verb-test-matrix-fr.md) | **Quoi tester** pour chaque verbe |
| [`developers-guide-en.md`](developers-guide-en.md) | Comment construire des applications avec tout cela |

---

## Comment lire les tableaux

Chaque ligne de fonctionnalité est marquée face à trois origines, puis dotée d'un état.

| Colonne | Signification |
|---|---|
| **85** | Défini par **COBOL-85** (ANSI X3.23-1985, y compris l'amendement de 1989 sur les fonctions intrinsèques là où c'est signalé) |
| **20xx** | Défini par une **norme ISO ultérieure** — COBOL 2002 / 2014 / 2023, et ce qui est actuellement en projet pour 2026 |
| **PRC** | Une **extension PowerRustCOBOL** — absente de toute norme COBOL |
| **État** | Ce que cette implémentation en fait |

Une fonctionnalité peut être marquée dans plus d'une colonne d'origine : une
fonctionnalité COBOL-85 qu'une norme ultérieure a étendue porte `●` dans les
deux, et la colonne **Notes** dit ce que la norme ultérieure a ajouté.

**Marques d'origine :** `●` défini ici · `○` étendu/clarifié ici · `—` absent de
cette norme.

**Marques d'état :** `✅` pris en charge · `🚧` partiel ou simplifié · `⛔` prévu,
pas encore implémenté · `🚫` hors périmètre par conception, ne sera jamais implémenté.

> **Note d'honnêteté.** PowerRustCOBOL vise un sous-ensemble pratique, orienté
> applications, augmenté d'extensions RAD visuelles. Ce n'est **pas** une
> implémentation COBOL-85 certifiée. La conformité est *mesurée* face à la suite
> officielle NIST CCVS85 plutôt qu'affirmée — voir le
> [tableau de bord](cobol85-supported-syntax-fr.md).

---

## 1. Format source et structure de programme

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Source en format fixe, **assoupli** (`fixed-relaxed`) | ● | ○ | ○ | ✅ | **Le format par défaut.** La zone de séquence et la colonne indicatrice sont respectées, mais la ligne se poursuit aussi loin que le développeur a tapé — pas de coupure à la colonne 72. Les `.cbl` de formulaire générés et les blocs `EXEC RUST` en ont besoin |
| Source en format fixe, **format de référence COBOL-85 classique** (`--source-format=fixed`) | ● | ○ | — | ✅ | Toutes les règles de colonnes appliquées : 1–6 séquence, 7 indicateur (`*` `/` commentaire, `-` continuation, `D` ligne de débogage), 8–72 source, **73–80 rejetées**, jonction de continuation standard y compris un littéral alphanumérique continué. C'est dans ce format qu'est écrite la suite NIST CCVS85 en images de cartes. **Choisi explicitement, jamais par détection** — appliquer ces règles à un source qui n'a pas été écrit pour elles supprime du code en silence |
| Source en format libre | — | ● | — | ✅ | COBOL 2002 (`--source-format=free`) |
| Sélecteur de format source — `--source-format free\|fixed\|fixed-relaxed\|auto` | — | — | ● | ✅ | Également `COBOLT_SOURCE_FORMAT` ; `auto` inspecte les premières lignes et ne sélectionne jamais le format strict |
| IDENTIFICATION DIVISION | ● | ○ | — | ✅ | |
| ENVIRONMENT DIVISION (CONFIGURATION, INPUT-OUTPUT / FILE-CONTROL) | ● | ○ | — | ✅ | |
| DATA DIVISION | ● | ○ | — | ✅ | |
| PROCEDURE DIVISION | ● | ○ | — | ✅ | |
| Programmes imbriqués | ● | ○ | — | ✅ | |
| Plusieurs unités de programme séquentielles dans un même fichier | ● | ○ | — | ✅ | |
| Copybooks `COPY` / `REPLACE` | ● | ○ | — | ✅ | Remplacement de pseudo-texte et de mots, `COPY` imbriqué, `REPLACE OFF` ; résout `.cpy`/`.cbl`/`.cob` à côté du source, sans distinction de casse |
| Paragraphe `REPOSITORY` | — | ● | ○ | ✅ | COBOL 2002 pour les classes ; PowerRustCOBOL y lie aussi les types **Rust FFI** |
| Rust en ligne `EXEC RUST … END-EXEC` | — | — | ● | ✅ | Compilé dans le binaire ; les erreurs sont signalées à la ligne et à la colonne COBOL du développeur lui-même |

## 2. Division des données et description des données

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| WORKING-STORAGE SECTION | ● | ○ | — | ✅ | |
| LOCAL-STORAGE SECTION | — | ● | — | ✅ | COBOL 2002 |
| LINKAGE SECTION | ● | ○ | — | ✅ | |
| FILE SECTION | ● | ○ | — | ✅ | |
| SCREEN SECTION | ● | ○ | — | 🚧 | Les `ACCEPT`/`DISPLAY` étendus `AT`/`WITH` s'exécutent via ANSI en mode CLI ; la saisie écran champ par champ est remplacée par le concepteur visuel de formulaires en mode GUI |
| COMMUNICATION SECTION (`CD`, contrôle des messages) | ● | — | — | 🚫 | Téléinformatique ; obsolète dans les normes ultérieures |
| REPORT SECTION / REPORT WRITER (`RD`, `GENERATE`) | ● | ○ | — | 🚫 | Hors périmètre par conception |
| `PICTURE` X / A / 9 / S / V avec répétition `(n)` | ● | ○ | — | ✅ | |
| PICTURE numérique éditée (`Z` `*` `$` `+` `-` `,` `.` `B` `0` `/` `CR` `DB`) | ● | ○ | — | ✅ | Suppression des zéros, protection par astérisques, `$` et signes fixes et flottants |
| `USAGE DISPLAY` | ● | ○ | — | ✅ | |
| `USAGE COMP` / `BINARY` | ● | ○ | — | ✅ | |
| `USAGE COMP-1` / `COMP-2` | — | ○ | ● | ✅ | Virgule flottante ; une extension propriétaire normalisée plus tard sous `FLOAT-SHORT`/`FLOAT-LONG` |
| `USAGE COMP-3` / `PACKED-DECIMAL` | ● | ○ | — | ✅ | |
| `USAGE COMP-5` | — | ○ | ● | ✅ | Binaire natif ; extension propriétaire |
| `USAGE INDEX` | ● | ○ | — | ✅ | |
| `USAGE POINTER` | — | ● | — | ✅ | COBOL 2002 ; lecture **et** écriture par alias |
| `OCCURS` fixe | ● | ○ | — | ✅ | |
| `OCCURS DEPENDING ON` | ● | ○ | — | ✅ | |
| `INDEXED BY` | ● | ○ | — | ✅ | |
| Numéros de niveau 01–49, 77 | ● | ○ | — | ✅ | |
| Niveau 66 `RENAMES` | ● | ○ | — | ✅ | |
| Noms-conditions de niveau 88 | ● | ○ | — | ✅ | Y compris `SET … TO TRUE` |
| Clause `VALUE` | ● | ○ | — | ✅ | |
| Éléments de groupe, `FILLER` | ● | ○ | — | ✅ | |
| `REDEFINES` | ● | ○ | — | ✅ | |
| Constantes figuratives (`SPACES`, `ZEROS`, `HIGH-`/`LOW-VALUES`, `QUOTES`, `NULLS`) | ● | ○ | — | ✅ | |

## 3. Division procédurale — verbes

| Verbe | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE`, `MOVE CORRESPONDING` | ● | ○ | — | ✅ | Appariement des sous-champs de groupe |
| `DISPLAY` | ● | ○ | — | ✅ | Les numériques sont rendus sur toute la largeur du PIC |
| `ACCEPT` (`FROM DATE/TIME/DAY/DAY-OF-WEEK`) | ● | ○ | — | ✅ | |
| `ACCEPT … FROM ENVIRONMENT` | — | ● | — | ✅ | COBOL 2002 |
| `ADD` / `SUBTRACT` (y compris `CORRESPONDING`) | ● | ○ | — | ✅ | Récepteurs multiples, `ROUNDED` par récepteur |
| `MULTIPLY` / `DIVIDE` (`GIVING`, `REMAINDER`) | ● | ○ | — | ✅ | Récepteurs multiples, `ROUNDED` par récepteur |
| `COMPUTE` | ● | ○ | — | ✅ | Récepteurs multiples, `ROUNDED` par récepteur |
| `ON SIZE ERROR` / `NOT ON SIZE ERROR` | ● | ○ | — | ✅ | |
| `IF … ELSE … END-IF` | ● | ○ | — | ✅ | |
| `EVALUATE … WHEN` / `ALSO` / `WHEN NOT` / `WHEN OTHER` | ● | ○ | — | ✅ | |
| `PERFORM` en ligne, `TIMES`, `UNTIL`, `TEST BEFORE/AFTER`, `VARYING … AFTER`, `THRU` | ● | ○ | — | ✅ | |
| `PERFORM para VARYING` (hors ligne) | ● | ○ | — | ✅ | |
| `GO TO`, `GO TO … DEPENDING ON` | ● | ○ | — | ✅ | |
| `ALTER` | ● | ○ | — | ✅ | Élément obsolète en COBOL-85 |
| `NEXT SENTENCE` | ● | ○ | — | ✅ | Sémantique fidèle ; obsolète en COBOL 2002 |
| `CONTINUE` | ● | ○ | — | ✅ | |
| `EXIT` | ● | ○ | — | ✅ | |
| `EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION` | — | ● | — | ✅ | COBOL 2002 |
| `STOP RUN` | ● | ○ | — | ✅ | |
| `GOBACK` | — | ● | — | ✅ | Extension propriétaire normalisée dans COBOL 2002 |
| `SET` (y compris `UP/DOWN BY`, 88 `TO TRUE`) | ● | ○ | — | ✅ | |
| `SET ADDRESS OF` / `SET … TO ADDRESS OF` / `NULL` | — | ● | — | ✅ | Pointeurs COBOL 2002 |
| `INITIALIZE`, `INITIALIZE … REPLACING` | ● | ○ | — | ✅ | Sensible à la catégorie, parcourt les groupes récursivement |
| `STRING` / `UNSTRING` (`ON OVERFLOW`) | ● | ○ | — | ✅ | |
| `INSPECT` `TALLYING` / `REPLACING` / `CONVERTING`, `BEFORE/AFTER INITIAL` | ● | ○ | — | ✅ | `TALLYING REPLACING` combiné |
| `SEARCH` / `SEARCH ALL` | ● | ○ | — | ✅ | Pilote l'index de la table, exécute le premier `WHEN` correspondant, sinon `AT END` |
| `SORT` / `MERGE` / `RELEASE` / `RETURN` | ● | ○ | — | ✅ | `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE` |
| `CALL … USING BY REFERENCE/CONTENT/VALUE`, `RETURNING` | ● | ○ | — | ✅ | `BY VALUE` et `RETURNING` relèvent de COBOL 2002 |
| `CALL … ON OVERFLOW` | ● | — | — | ✅ | |
| `CALL … ON EXCEPTION` / `NOT ON EXCEPTION` | — | ● | — | ✅ | COBOL 2002 |
| `CANCEL` | ● | ○ | — | ✅ | |
| `INVOKE` | — | ● | ○ | 🚧 | OO de COBOL 2002. Pris en charge pour les **objets GUI et runtime ainsi que les greffons Rust FFI** ; les définitions de classes/méthodes écrites par l'utilisateur ne sont pas implémentées |
| `UNLOCK` | — | ● | — | 🚧 | Pilote les verrous d'enregistrement propres à une exécution ; non appliqué entre processus du système |
| `COMMIT` / `ROLLBACK` | — | — | ● | ✅ | Transactions pilotées par le programme sur les fichiers INDEXED, avec un véritable journal d'annulation |
| Définitions OO `CLASS-ID` / `METHOD-ID` | — | ● | — | ⛔ | Prévu |

## 4. Conditions et expressions

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| Conditions de relation, de classe, de signe et de nom-condition | ● | ○ | — | ✅ | |
| Relations combinées abrégées, préfixées par l'opérateur (`a > 1 AND < 9`) | ● | ○ | — | ✅ | |
| Relations combinées abrégées, objet littéral (`a = 1 OR 2 OR 3`) | ● | ○ | — | ✅ | |
| Relations combinées abrégées, objet identificateur (`a = b OR c`) | ● | ○ | — | ✅ | |
| Modification de référence `item(start:length)` | ● | ○ | — | ✅ | Lecture **et** écriture par insertion, sur n'importe quel opérande |
| Indiçage de table à l'exécution `t(i)` / `t(i, j)` | ● | ○ | — | ✅ | Stockage par occurrence, indices variables |
| Noms qualifiés `id OF/IN group` | ● | ○ | — | ✅ | Une feuille déclarée sous plus d'un groupe se résout vers un stockage indépendant |
| Comparaison alphanumérique conforme à COBOL (complétée par des espaces) | ● | ○ | — | ✅ | |
| **Arithmétique exacte en virgule fixe** | ● | ○ | ○ | ✅ | Mantisse entière `i128`, aucun aller-retour par `f64` : la précision standard à 18 chiffres et la précision **étendue à 31 chiffres** restent exactes |
| Expressions de propriété concises (`Output::Value`) | — | — | ● | ✅ | Lire/écrire une propriété de contrôle à l'intérieur d'une formule, sans élément temporaire en working-storage |

### 4.1 Méthodes de valeur sur un élément de données

`item::Method(args)` appelle une méthode sur la **valeur d'un élément de données
ordinaire** — un champ `PIC X`, un groupe, une occurrence de table, une tranche
issue d'une modification de référence ou une expression arithmétique — et pas
seulement sur un contrôle. Rien de tout cela n'est du COBOL standard.

Utilisable partout où une expression l'est : comme source d'un `MOVE`, dans un
`COMPUTE`, à l'intérieur d'une condition, et en ligne dans un `DISPLAY`. Les
méthodes se **chaînent** : `WS-TEXT::Trim()::Len()`.

| Méthode | Retourne | État | Notes |
|---|---|:--:|---|
| `Trim()` | texte | ✅ | Espaces de tête et de queue supprimés |
| `UpperCase()` · `ToUpperCase()` · `Upper()` | texte | ✅ | Trois écritures acceptées d'une même méthode |
| `LowerCase()` · `ToLowerCase()` · `Lower()` | texte | ✅ | |
| `Replace(from, to)` | texte | ✅ | Toutes les occurrences |
| `Len()` · `Length()` | numérique | ✅ | La longueur du **champ**, si bien qu'un `PIC X(20)` contenant `hello` répond `20`. Chaînez `::Trim()::Len()` pour obtenir la longueur du contenu |
| `Split(sep)` | texte | ✅ | Le **premier** champ |
| `Split(sep)(n)` | texte | ✅ | Le *n*-ième champ, numéroté à partir de 1. L'indice n'est accepté que sur un récepteur qui est un élément de données |

| Récepteur | État | Notes |
|---|:--:|---|
| Élément de données (`PIC X`, groupe, `01`/`77`) | ✅ | Le cas ordinaire |
| Occurrence de table, modification de référence, nom qualifié, expression arithmétique | ✅ | Acceptés par l'évaluateur |
| **Littéral** (`"a-b-c"::Split("-")`) | ⛔ | L'interpréteur accepte un récepteur littéral, mais pas le parser : `::` après un littéral est une erreur de syntaxe. Affectez d'abord le littéral à un élément de données |

### 4.2 Une expression partout où COBOL-85 n'admet qu'un élément

COBOL-85 limite la plupart des positions émettrices à un identificateur ou à un
littéral. RustCOBOL y évalue à la place une expression complète, et c'est ce qui
supprime l'élément de working-storage jetable que la norme oblige à déclarer.

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `MOVE <expression> TO target` | — | — | ● | ✅ | `MOVE WS-N * 2 TO WS-OUT`. La norme n'admet qu'un identificateur ou un littéral comme champ émetteur |
| `SET target TO <expression>` | — | — | ● | ✅ | Équivalent à la forme `COMPUTE` ; la cible peut être un élément de données ou une lvalue de propriété de contrôle |
| `STRING <expression> … INTO` | — | — | ● | ✅ | Un élément émetteur peut être une expression arithmétique (`STRING WS-N * 2 …`) ou un appel de méthode de valeur (`STRING WS-A::UpperCase() …`) ; `DELIMITED BY` et le reste demeurent standard |
| **Inférence de type** — une lecture `Ctrl::Property` est une valeur typée de première classe | — | — | ● | ✅ | Le type numérique/texte se propage à travers l'expression, si bien qu'une propriété entre directement dans un calcul, une condition ou une position émettrice **sans aucun élément `PIC` intermédiaire** : `IF Slider-1::Value > 50`, `COMPUTE Total-Lbl::Value = Qty-Box::Value * Price-Box::Value`. Une valeur de propriété d'aspect numérique est relue comme numérique, de sorte que les comparaisons et les calculs restent algébriques plutôt que caractère par caractère |

## 5. Fonctions intrinsèques

L'ensemble des fonctions intrinsèques de COBOL-85 est arrivé avec l'**amendement
de 1989** (ANSI X3.23a-1989) ; les fonctions ajoutées par COBOL 2002 et les
normes suivantes sont marquées dans la colonne `20xx`. Toutes celles ci-dessous
sont implémentées.

| Groupe | Fonctions | 85 | 20xx | PRC | État |
|---|---|:--:|:--:|:--:|:--:|
| Longueur et caractère | `LENGTH`, `ORD`, `CHAR` | ● | ○ | — | ✅ |
| Longueur et caractère (ultérieures) | `BYTE-LENGTH`, `STORED-CHAR-LENGTH` | — | ● | — | ✅ |
| Casse et texte | `UPPER-CASE`, `LOWER-CASE`, `REVERSE` | ● | ○ | — | ✅ |
| Texte (ultérieures) | `TRIM`, `CONCATENATE` | — | ● | — | ✅ |
| Conversion numérique | `NUMVAL`, `NUMVAL-C` | ● | ○ | — | ✅ |
| Conversion numérique (ultérieures) | `NUMVAL-F`, `TEST-NUMVAL` | — | ● | — | ✅ |
| Arithmétique | `MAX`, `MIN`, `SQRT`, `MOD`, `REM`, `ABS`, `INTEGER`, `INTEGER-PART`, `FRACTION-PART`, `RANDOM` | ● | ○ | — | ✅ |
| Ordonnancement | `ORD-MAX`, `ORD-MIN` | ● | ○ | — | ✅ |
| Statistiques | `SUM`, `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION` | ● | ○ | — | ✅ |
| Trigonométrie et logarithmes | `SIN`, `COS`, `TAN`, `ASIN`, `ACOS`, `ATAN`, `LOG`, `LOG10`, `EXP`, `EXP10`, `PI` | ● | ○ | — | ✅ |
| Combinatoire | `FACTORIAL` | ● | ○ | — | ✅ |
| Finance | `ANNUITY`, `PRESENT-VALUE` | ● | ○ | — | ✅ |
| Date et heure | `CURRENT-DATE`, `WHEN-COMPILED`, `INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `YEAR-TO-YYYY` | ● | ○ | — | ✅ |

## 6. E/S fichiers — organisations et accès

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `ORGANIZATION IS SEQUENTIAL` | ● | ○ | — | ✅ | Enregistrements de longueur fixe |
| `ORGANIZATION IS LINE SEQUENTIAL` | — | ● | — | ✅ | Texte terminé par un saut de ligne ; les espaces de fin sont supprimés à l'écriture |
| `ORGANIZATION IS INDEXED` | ● | ○ | — | ✅ | Moteur ISAM intégré, sans dépendances |
| `ORGANIZATION IS RELATIVE` | ● | ○ | — | ✅ | Moteur propre (`cobolt-runtime/src/relative.rs`, conteneur `PRCREL1`, disque et MEMORY). `RELATIVE KEY IS` adresse les enregistrements par numéro d'enregistrement entier à partir de 1 ; les trois modes d'accès ; les sept verbes fichier s'y aiguillent. NIST — **module RL terminé sur les deux axes** : 35/35 à la compilation, 34/34 à l'exécution, 354 assertions, 0 échec (moteur 1.62.76, module 1.62.77) |
| `RELATIVE KEY IS data-name` (y compris l'écriture sans `KEY`) | ● | ○ | — | ✅ | Une clause `RELATIVE data-name` dont le mot `KEY` est omis désigne la clé, et non une simple clause d'organisation |
| `ACCESS MODE SEQUENTIAL` / `RANDOM` / `DYNAMIC` | ● | ○ | — | ✅ | Les trois s'exécutent |
| `RECORD KEY`, `ALTERNATE RECORD KEY [WITH DUPLICATES]` | ● | ○ | — | ✅ | Ordre de clés croissant sur le disque |
| `OPEN INPUT` / `OUTPUT` / `EXTEND` / `I-O` | ● | ○ | — | ✅ | |
| `READ … [INTO] [AT END / NOT AT END]` | ● | ○ | — | ✅ | |
| `READ … NEXT` / `PREVIOUS` | ● | ○ | — | ✅ | `PREVIOUS` relève de COBOL 2002 |
| `WRITE … [FROM]`, `REWRITE`, `DELETE` | ● | ○ | — | ✅ | |
| `START … KEY IS = / > / >= / < / <=` | ● | ○ | — | ✅ | Y compris `GREATER/LESS THAN`, `NOT LESS THAN` |
| `INVALID KEY` / `NOT INVALID KEY` | ● | ○ | — | ✅ | |
| Codes `FILE STATUS` | ● | ○ | — | ✅ | 00/02/10/22/23/30/35/39/… |
| `OPEN … SHARING WITH ALL OTHER \| NO OTHER \| READ ONLY` | — | ● | — | 🚧 | Analysée et portée par l'instruction, **indicative** — il n'y a qu'une seule unité d'exécution, donc rien n'entre en concurrence |
| `OPEN … WITH LOCK` (ouvrir le fichier en exclusivité) | — | ● | — | 🚧 | Idem : acceptée et indicative dans le modèle à unité d'exécution unique |
| `READ … WITH LOCK` | — | ● | — | ✅ | Le moteur détient déjà l'enregistrement sous `I-O` ; la clause énonce l'intention |
| `READ … WITH NO LOCK` | — | ● | — | ✅ | Libère réellement le verrou que le moteur prend sous `I-O` — la seule clause de verrouillage qui ait aujourd'hui un effet à l'exécution. `UNLOCK` figure au §3 avec les autres verbes |
| Partage de fichiers entre processus / application des verrous d'enregistrement | — | ● | — | ⛔ | Prévu ; modèle à unité d'exécution unique aujourd'hui |

## 7. E/S fichiers — le moteur INDEXED (PowerRustCOBOL)

Tout ce que contient cette section est une extension de la plateforme autour du
comportement standard `ORGANIZATION IS INDEXED` ci-dessus. Détail :
[`indexed-file-format-fr.md`](indexed-file-format-fr.md),
[`indexed-file-internals-fr.md`](indexed-file-internals-fr.md),
[`indexed-redb-engine-fr.md`](indexed-redb-engine-fr.md).

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| `STORAGE [MODE] IS DISK` | — | — | ● | ✅ | **Le mode par défaut.** B+tree paginé persistant ; les enregistrements et les index résident dans le fichier `ASSIGN` et sont lus à la demande, de sorte que la RAM reste bornée sur de très gros fichiers |
| `STORAGE [MODE] IS MEMORY` | — | — | ● | ✅ | Fichier entier en RAM, persisté vers le chemin `ASSIGN` à la fermeture |
| `WITH [DATA] COMPRESSION` | — | — | ● | ✅ | RLE sans dépendances ; écrase les suites de remplissage des enregistrements COBOL typiques bien au-delà de 50 % |
| `COMMIT` / `ROLLBACK` pilotés par le programme | — | — | ● | ✅ | Véritable journal d'annulation, moteurs mémoire et disque |
| Verrouillage d'enregistrement au sein d'une unité d'exécution | — | ○ | ● | ✅ | Voir la réserve sur l'inter-processus ci-dessus |
| Moteur sélectionnable (`--indexed-engine rust\|rm-cobol85\|fujitsu\|redb`) | — | — | ● | ✅ | Également `COBOL_INDEXED_ENGINE` ; tous compatibles en comportement, `rust` est celui par défaut |
| Moteur ACID `redb` résistant aux plantages | — | — | ● | ✅ | OPEN en O(1) (~5 ms à 200 k enregistrements), RAM à l'échelle de l'ensemble de travail (≥250 M enregistrements), survit à une coupure de courant sans corruption d'index |
| Conteneur `PRCIDX1` autodescriptif | — | — | ● | ✅ | Intègre le format d'enregistrement et les descripteurs de clés ; une validation stricte à l'ouverture fait correspondre une divergence de schéma → `39`, un fichier absent → `35`. Pas compatible octet à octet avec Fujitsu |
| Journal de transactions par fichier (`--indexed-log basic\|full`) | — | — | ● | ✅ | logfmt ou NDJSON prêt pour Grafana/Loki — voir [`observability-fr.md`](observability-fr.md) |

## 8. Intégrations d'exécution

Atteintes depuis COBOL sous forme de `CALL` d'exécution et d'`INVOKE`. Rien de
tout cela n'est du COBOL standard ; c'est ce qui rend le langage utilisable pour
des applications modernes.

| Fonctionnalité | 85 | 20xx | PRC | État | Notes |
|---|:--:|:--:|:--:|:--:|---|
| **SQL** — SQLite, PostgreSQL, MySQL | — | — | ● | ✅ | Une surface d'appel CALL identique pour les trois ; le backend est choisi d'après la chaîne de connexion. **Aucune bibliothèque système** — rien n'est lié depuis l'hôte — mais « purement Rust » n'est vrai que pour deux des trois : `postgres` et `mysql` le sont, tandis que `rusqlite` est épinglé avec `features = ["bundled"]` et compile l'**amalgame C de SQLite** via `libsqlite3-sys`. (Cette compilation C explique aussi pourquoi `test_external_crates_e2e` échoue par intermittence à l'intérieur d'un `cargo build` imbriqué.) Voir [`database-runtime-fr.md`](database-runtime-fr.md) |
| **Jeux de résultats SQL** — `Fetch()`, `ColumnNames()`, `ColumnCount()`, `ColumnName(n)` | — | — | ● | ✅ | `Fetch()` renvoie la ligne suivante séparée par des TABULATIONS, et vide une fois les lignes épuisées, de sorte qu'elle termine sa propre boucle ; `ColumnNames()` nomme le jeu de résultats dans l'ordre du SELECT, même lorsqu'aucune ligne n'a été trouvée. La surface `CALL`, elle, lit la ligne courante colonne par colonne par indice — les deux parcours ne doivent pas être mêlés sur un même descripteur |
| **HTTP / REST** — GET / POST / PUT / DELETE | — | — | ● | ✅ | En-têtes personnalisés |
| **GUI** — `COBOL-WAIT-EVENT`, `COBOL-SET-PROPERTY`, `COBOL-GET-PROPERTY`, `COBOL-INIT-FORM` | — | — | ● | ✅ | |
| **Graphiques** — barres / courbes / secteurs / aires / nuages de points / anneau | — | — | ● | ✅ | Liés à des tables COBOL |
| **Fichiers texte** — `COBOL-APPEND-FILE`, `COBOL-WRITE-FILE` | — | — | ● | ✅ | |
| **Minuteurs** | — | — | ● | ✅ | |
| **Point d'accroche objet pour agent IA** | — | — | ● | ✅ | |
| **Greffons Rust FFI** | — | — | ● | ✅ | Modules déclarés sous `REPOSITORY`, aiguillés via `INVOKE` ou par des correspondances directes de propriétés |
| **Procédures utilisateur** | — | — | ● | ✅ | Procédures COBOL partagées, éditables dans l'IDE, appelables par `CALL "PROCEDURE-NAME"` |

## 9. Explicitement hors périmètre

Ces éléments ne seront pas implémentés. Ils sont listés pour que la réponse se
trouve plutôt qu'elle ne manque.

| Fonctionnalité | 85 | 20xx | PRC | État | Pourquoi |
|---|:--:|:--:|:--:|:--:|---|
| COMMUNICATION SECTION (`CD`, contrôle des messages / téléinformatique) | ● | — | — | 🚫 | Obsolète dans les normes ultérieures ; aucun usage moderne |
| REPORT WRITER SECTION (`RD`, `GENERATE` / `INITIATE` / `TERMINATE`) | ● | ○ | — | 🚫 | Remplacée par les rapports et la liaison de données propres à la plateforme |
| Contrôles ActiveX / OLE / COM | — | — | — | 🚫 | Spécifiques à une plateforme et non portables |

---

## 10. La plateforme elle-même

Ce ne sont pas des fonctionnalités du langage COBOL — il s'agit de l'IDE, du
compilateur et de l'outillage qui les entourent. Visite complète dans le
[guide du développeur](developers-guide-en.md).

### 10.1 L'IDE

| Fonctionnalité | État | Notes |
|---|:--:|---|
| Concepteur visuel de formulaires | ✅ | Canevas de conception à thèmes multiples (**Liquid Glass**, **Cobalt Steel**), magnétisme de grille, redimensionnement par glissement des contrôles et du canevas, alignement multi-sélection, ordonnancement en z |
| Moteur de rendu unifié | ✅ | Parité au pixel près entre le concepteur, l'aperçu, l'application en cours d'exécution et le binaire compilé |
| Catalogue de contrôles | ✅ | **42 widgets** répartis entre Common, Container, Data, Graphics, Menu, Non-visual et Charts |
| Rayon d'angle universel et découpe arrondie | ✅ | Les enfants imbriqués se découpent sur la bordure arrondie du parent par masquage à encoche d'angle |
| `Transparency` par contrôle | ✅ | 0 = opaque … 100 = translucide ; estompe la face, le cadre et l'ombre tandis que le texte, les glyphes et la bordure restent lisibles. Les libellés qui passent sous le seuil WCAG AA face à ce qui se trouve derrière eux basculent vers le pôle qui se lit |
| Widget Animator | ✅ | Rend nativement le **GIF / WebP / APNG** |
| Knob, Gauge, Switch, FileDropZone, Maps, Web Search | ✅ | Cadran rotatif à remplissage bipolaire ; KPI radial/linéaire/en anneau avec zones d'avertissement et critiques automatiques ; glisser-déposer ou sélecteur natif |
| Éditeur de menus avancé | ✅ | Éditeur d'arborescence visuel, 122 icônes vectorielles intégrées, imbrication hiérarchique, signatures HMAC d'intégrité de la configuration |
| Liaison de données et tableaux de contrôles | ✅ | Liaison directe à des sources SQL/de données ; les **Visual Repeating Groups** déploient des tableaux de GroupBox/Panel à partir du nombre de lignes `DataSource` à l'exécution |
| Validation visuelle et inspecteur de formulaires | ✅ | Badges d'erreur en temps réel pour les gestionnaires mal formés, les liaisons incomplètes, les anomalies de mise en page ; le gestionnaire de processus `rcrun` suit en direct le % CPU, la RSS, les journaux et le nombre de threads |
| Form Debugger | ✅ | Fenêtre indépendante toujours au premier plan : points d'arrêt, pas à pas In/Out/Over, inspecteur de variables, lecture animée à 1–10 lignes par seconde |
| Maillage d'assistants IA agentiques | ✅ | Orchestrateur LLM **rig-core** (Ollama, OpenAI, Groq, Alibaba Model Studio, autres API cloud) exécutant Dev Agent, Editor Assistant et History Compactor, avec un journal d'observabilité en direct et des relevés de jetons `↑input ↓output` |
| Grace, l'orchestratrice | ✅ | Décompose une requête, achemine chaque tâche vers le spécialiste qui en est propriétaire, et impose un **relecteur Pedantic** en tête-à-tête — aucun spécialiste n'approuve son propre travail |
| Base de connaissances découpée avec RAG | ✅ | Indexée à raison d'un enregistrement par sujet ; livrée pré-vectorisée, GPU avec repli CPU peu gourmand, **File → Reindex Knowledge Bases** |
| Cycle de vie des formulaires et fenêtrage | ✅ | Un **formulaire principal** désigné démarre une application ; l'habillage et l'état propres à chaque formulaire sont respectés ; `OpenFormSync`/`OpenFormAsync` ; la position de la fenêtre est une propriété de conception ; effets d'entrée et de sortie par projet |
| Exécution multifenêtre | ✅ | Aperçu et exécution des écrans dans des viewports dédiés du système (multi-viewport egui) |
| Interface internationalisée | ✅ | 6 langues d'interface : anglais, espagnol, portugais, japonais, chinois, français |
| Sélecteur de polices système | ✅ | N'importe quelle police installée, rendue dans son propre caractère, appliquée en direct au concepteur, aux aperçus et aux formulaires en cours d'exécution |
| Boîtes de dialogue de fichiers natives non bloquantes | ✅ | Ouvrir/enregistrer/parcourir sans bloquer la boucle d'événements de l'interface |

### 10.2 Le compilateur

| Fonctionnalité | État | Notes |
|---|:--:|---|
| Sortie en un seul binaire natif | ✅ | Sérialise l'AST avec `bincode` + `flate2`, l'intègre avec tous les formulaires via `include_bytes!`, construit avec `cargo build --release`, produit un binaire unique dans `bin/` — **sans aucun source `.cbl` inclus** |
| Mentions de redistribution | ✅ | `bin/` reçoit automatiquement `LICENSE`, `NOTICE` et la mention d'exécution, de sorte que les distributions portent les mentions Apache-2.0 requises |
| Véritables diagnostics `rustc` en cas d'échec de construction | ✅ | Un échec de construction rapporte les diagnostics propres au compilateur, pas une ligne de résumé |

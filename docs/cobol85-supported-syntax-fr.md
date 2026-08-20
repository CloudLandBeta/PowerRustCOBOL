<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Référence de la syntaxe prise en charge par RustCOBOL-85

**Source de vérité sur ce que le lexeur / l'analyseur / le runtime RustCOBOL
acceptent réellement aujourd'hui**, dérivée du code source (`cobolt-lexer`,
`cobolt-parser`, `cobolt-runtime`). Écrivez vos tests contre les formes ✅ ; les
formes ❌ ne s'analyseront pas ou sont sans effet, et les formes ⚠️ s'analysent
mais se comportent partiellement. Ce document est le compagnon de
[`cobol85-verb-test-matrix.md`](cobol85-verb-test-matrix.md) : la matrice dit
*quoi* tester, celui-ci dit *quelle orthographe RustCOBOL comprend*.

Légende : ✅ pris en charge · ⚠️ s'analyse mais partiel/simplifié · ❌ non
reconnu (à éviter, ou à tester uniquement pour confirmer la lacune).

> **Mise à jour (passe de comblement des lacunes) :** les éléments suivants ont
> été implémentés et sont désormais ✅ — **modification de référence**
> `id(début:long)`, **`PERFORM n TIMES` en ligne**, **`SET … UP/DOWN BY`**,
> **`ON OVERFLOW` de STRING/UNSTRING + `END-STRING`/`END-UNSTRING`**,
> **`INITIALIZE` sensible aux catégories**, **conditions abrégées préfixées par
> un opérateur** (`a > 1 AND < 9`), **`CALL … ON EXCEPTION`** (s'exécute sur un
> CALL non résolu), **`COMPUTE` à récepteurs multiples + `ROUNDED` par
> récepteur**, et un jeu de **fonctions intrinsèques** bien plus large.
>
> **Mise à jour (passe d'environnement hiérarchique / sensible aux occurrences —
> 1.5.0) :** quatre fonctionnalités que le modèle de données bloquait sont
> désormais ✅ — **indiçage de tables à l'exécution** `t(i)` / `t(i, j)`
> (stockage par occurrence), **levée d'ambiguïté des noms qualifiés**
> `id OF/IN groupe` (les noms feuilles dupliqués se résolvent vers des stockages
> indépendants), **`MOVE/ADD/SUBTRACT CORRESPONDING`** et **`SEARCH` /
> `SEARCH ALL` fonctionnels**.
>
> **Mise à jour (passe de complétude des verbes — 1.6.0) :** désormais également
> ✅ — **`MULTIPLY`/`DIVIDE GIVING` à récepteurs multiples + `ROUNDED` par
> récepteur** sur `ADD`/`SUBTRACT` ; **`EXIT PERFORM [CYCLE]` /
> `EXIT PARAGRAPH` / `EXIT SECTION`** et le `EXIT` simple corrigé ;
> **`CALL … NOT ON EXCEPTION`** ; **`INSPECT … TALLYING … REPLACING`** combiné et
> les régions **`BEFORE/AFTER INITIAL`** ; les **intrinsèques** de date et
> financières (`INTEGER-OF-DATE`, `DATE-OF-INTEGER`, `INTEGER-OF-DAY`,
> `DAY-OF-INTEGER`, `ANNUITY`, `FRACTION-PART`) ; les **conditions abrégées à
> objet littéral** (`A = 1 OR 2 OR 3`) ; **`EVALUATE … ALSO`** (multi-sujets) et
> **`WHEN NOT`** ; les **noms-conditions de niveau 88 réels**
> (`SET … TO TRUE/FALSE`, l'hôte étant testé contre ses VALUE / plages) ;
> **`PERFORM para VARYING`** ; et un runtime **`SORT`/`MERGE`** fonctionnel
> (`RELEASE`/`RETURN`, `USING`/`GIVING`, `INPUT`/`OUTPUT PROCEDURE`). La liste
> des éléments à éviter, en bas, est à jour.
>
> **Mise à jour (passe de vidage de la liste à éviter — 1.7.0) :** les lacunes
> restantes sont désormais implémentées — **abréviation à objet identificateur**
> (`a = b OR c`, résolue via les métadonnées de niveau 88) ;
> **`INITIALIZE … REPLACING catégorie DATA BY valeur`** ; **`66 RENAMES`** (la
> lecture synthétise / l'écriture répartit sur les éléments couverts) ;
> **pointeurs** (`USAGE POINTER`, `SET ptr TO ADDRESS OF x / NULL`, aliasing via
> `SET ADDRESS OF item TO …`, `IF ptr = NULL`) ; **`ALTER`** / **`UNLOCK`** ;
> **`NEXT SENTENCE`** fidèle ; les **intrinsèques** standard restantes
> (`PRESENT-VALUE`, `YEAR-TO-YYYY`, `BYTE-LENGTH`, `NUMVAL-F`, `TEST-NUMVAL`) ;
> et l'**`ACCEPT`/`DISPLAY` écran** étendu (`AT`/`WITH` via ANSI en mode CLI —
> désormais *exécuté*, et non plus seulement analysé).
>
> **Mise à jour (1.7.1) :** les sources de registre d'`ACCEPT` sont désormais
> fonctionnelles (elles étaient des non-opérations reconnues) —
> **`FROM COMMAND-LINE`**, **`ARGUMENT-NUMBER`** / **`ARGUMENT-VALUE`**
> (appariées avec `DISPLAY n UPON ARGUMENT-NUMBER`), **`ENVIRONMENT-VALUE`**
> (appariée avec `DISPLAY "name" UPON ENVIRONMENT-NAME`), **`ESCAPE KEY`** →
> `"00"`, **`CRT STATUS`** → `"0000"`.
>
> **Mise à jour (1.7.2) :** les clauses de partage / verrouillage de fichiers et
> `CANCEL` (auparavant ❌ / sans effet) — **`OPEN … SHARING WITH … [WITH LOCK]`**,
> **`READ … WITH [NO] LOCK`**, **`UNLOCK`** (libère les verrous
> d'enregistrement INDEXED du fichier) et **`CANCEL programme`** (réinitialise le
> stockage du programme).
>
> **Mise à jour (1.8.0) :** **`COMMIT` / `ROLLBACK`** sont désormais de vrais
> verbes COBOL — des transactions pilotées par le programme sur les fichiers
> INDEXED ouverts (moteur mémoire comme moteur disque). Le moteur disque a gagné
> un véritable journal d'annulation en cours d'exécution (c'était auparavant sans
> effet). La liste des éléments à éviter, en bas, est à jour.

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
fichiers — indicatif au sein de l'unité d'exécution unique)
✅ `COMMIT` / `ROLLBACK` (transactions de fichiers INDEXED pilotées par le
programme — voir Verbes de fichier) · `CANCEL` (réinitialise le stockage du
programme) · ⚠️ `INVOKE` (analysé comme une non-opération)
Extensions du projet : `EXEC RUST … END-EXEC`, `TRY/CATCH/FINALLY/END-TRY`,
`THROW`. Un bloc peut faire `use` des crates toujours liées (std, egui, eframe et
le jeu du runtime lié) **plus toute crate que le project enregistre sous
Project's Crates** (spec 044) : les crates enregistrées sont figées à une version
exacte, intégrées au `crates/` du project et compilées dans le binaire ; les
crates non enregistrées font échouer Check/Build à la ligne du développeur, en
nommant le remède.

✅ `SEARCH` (séquentiel) / `SEARCH ALL` (recherche binaire sur une table à
`ASCENDING`/`DESCENDING KEY` — exécute le premier `WHEN` qui correspond, sinon
`AT END`).
✅ `SORT` / `MERGE` avec `RELEASE` / `RETURN` (fonctionnels — voir plus bas).
✅ `DECLARATIVES … END DECLARATIVES` avec `USE AFTER STANDARD ERROR PROCEDURE ON
{file… | INPUT | OUTPUT | I-O | EXTEND}` — gestionnaires d'erreur de fichier
déclenchés sur un `FILE STATUS` d'erreur non traité.
❌ **Non reconnus — à ne pas utiliser :** `ENTRY`,
`GENERATE`/`INITIATE`/`TERMINATE`, `SEND`/`RECEIVE`, `ENABLE`/`DISABLE`.

---

## Formes prises en charge, verbe par verbe

### MOVE
- ✅ `MOVE {id|lit|figurative} TO id1 [id2 …]` (récepteurs multiples).
- ✅ `MOVE CORRESPONDING g1 TO g2` — déplace chaque élément subordonné que les
  deux groupes partagent par son nom, en descendant récursivement dans les
  sous-groupes correspondants.
- ✅ **Modification de référence `id(début:long)`** — en émetteur (sous-chaîne) et
  en récepteur (affectation partielle insérée) ; fonctionne sur les opérandes de
  tous les verbes. `long` est facultatif.
- ✅ indices `t(i)`, `t(i, j)` — lisent/écrivent l'emplacement de stockage de
  cette occurrence ; les indices variables `t(WS-I)` sont évalués à chaque accès.
- ✅ qualification `id OF/IN groupe` (`… OF g1 OF g2`) — se résout vers le bon
  élément même lorsque le nom feuille est déclaré sous plus d'un groupe.

### ADD / SUBTRACT
- ✅ `ADD a [b …] TO r1 [ROUNDED] [r2 [ROUNDED] …] [[ON] SIZE ERROR …][NOT …][END-ADD]`.
- ✅ `ADD a [b …] GIVING r1 [ROUNDED] [r2 …] …` · `SUBTRACT a … FROM r …` · `… GIVING …`.
- ✅ **`ROUNDED` par récepteur** — chaque récepteur porte son propre indicateur
  `ROUNDED`.
- ✅ `ADD CORRESPONDING g1 TO g2 [ROUNDED]` /
  `SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]` — combine chaque paire numérique
  correspondante, en descendant récursivement dans les sous-groupes qui
  correspondent.

### MULTIPLY / DIVIDE
- ✅ `MULTIPLY a BY b [ROUNDED] [GIVING r1 [ROUNDED] r2 …] [SIZE ERROR …][END-MULTIPLY]`.
- ✅ `DIVIDE a {INTO|BY} b [ROUNDED] [GIVING q1 [ROUNDED] q2 …] [REMAINDER r] [SIZE ERROR …][END-DIVIDE]`.
- ✅ **plusieurs récepteurs `GIVING`**, chacun avec son propre `ROUNDED`.
- ⚠️ `DIVIDE a BY b` (sans `GIVING`) range `a/b` de nouveau dans `a` (une
  commodité PowerRustCOBOL ; le COBOL standard exige ici `INTO` ou `GIVING`).

### COMPUTE
- ✅ `COMPUTE r1 [ROUNDED] [r2 [ROUNDED] …] = expr [[ON] SIZE ERROR …][NOT …]
  [END-COMPUTE]` — **récepteurs multiples, chacun avec son propre `ROUNDED`**.
- ✅ opérateurs d'expression `+ - * /` et `**` (puissance, associative à droite),
  parenthèses, `FUNCTION nom(args)`.

### IF / EVALUATE
- ✅ `IF cond [THEN] instructions [ELSE instructions] [END-IF]`.
- ✅ `EVALUATE {expr | TRUE | FALSE} [ALSO sujet …]` … `WHEN {valeur | valeur THRU
  valeur | NOT valeur | condition | ANY} [ALSO …] instructions … [WHEN OTHER
  instructions] END-EVALUATE`.
- ✅ **`ALSO` multi-sujets** — chaque colonne `WHEN` est comparée
  positionnellement à son sujet, puis combinée par AND.
- ✅ **`WHEN NOT valeur`** nie un objet de sélection ; **`WHEN condition`**
  (par ex. `EVALUATE TRUE WHEN a > b`) évalue la condition booléenne.

### PERFORM
- ✅ `PERFORM p [THRU p2]`.
- ✅ `PERFORM p [THRU p2] n TIMES` (n = littéral entier ou data item).
- ✅ `PERFORM p UNTIL cond [WITH TEST {BEFORE|AFTER}]`.
- ✅ `PERFORM UNTIL cond … END-PERFORM` en ligne,
  `PERFORM [WITH] TEST {BEFORE|AFTER} UNTIL cond … END-PERFORM`.
- ✅ `PERFORM VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …] …
  END-PERFORM`.
- ✅ `PERFORM n TIMES … END-PERFORM` en ligne (sans paragraphe).
- ✅ `PERFORM p [THRU p2] VARYING v FROM a BY b UNTIL c` — exécute le paragraphe à
  chaque itération (hors ligne, sans `END-PERFORM`).

### GO TO / CONTINUE / EXIT / STOP
- ✅ `GO TO p` · `GO TO p1 p2 … DEPENDING ON id` · `GOBACK` / `GO BACK`.
- ✅ `CONTINUE` · `STOP RUN` · `STOP littéral`.
- ✅ le `EXIT` simple est un point de retour sans effet ; `EXIT PROGRAM` rend la
  main à l'appelant.
- ✅ `EXIT PERFORM [CYCLE]` (rompre / poursuivre le PERFORM en ligne le plus
  proche), `EXIT PARAGRAPH`, `EXIT SECTION`.
- ✅ `NEXT SENTENCE` — transfère le contrôle au-delà de la prochaine limite de
  phrase (l'analyseur insère des marqueurs de limite à chaque point ; fidèle, et
  non un simple `CONTINUE`).

### ACCEPT
- ✅ `ACCEPT id`.
- ✅ `ACCEPT id FROM {DATE | TIME | DAY | DAY-OF-WEEK | COMMAND-LINE |
  ENVIRONMENT "name" | mnémonique}`.
- ✅ `ACCEPT id AT {nnnn | LINE n COLUMN n}` positionne le curseur (ANSI, CLI).
- ✅ `FROM COMMAND-LINE` (la ligne de commande entière) · `FROM ARGUMENT-NUMBER`
  (nombre d'arguments) · `FROM ARGUMENT-VALUE` (l'argument au pointeur défini par
  `DISPLAY n UPON ARGUMENT-NUMBER`) · `FROM ENVIRONMENT "name"` /
  `FROM ENVIRONMENT-VALUE` (la variable nommée par
  `DISPLAY "name" UPON ENVIRONMENT-NAME`) · `FROM ESCAPE KEY` → `"00"` ·
  `FROM CRT STATUS` → `"0000"`.

### DISPLAY
- ✅ `DISPLAY {id|lit} … [UPON mnémonique] [[WITH] NO ADVANCING]`.
- ✅ formes écran `DISPLAY id AT nnnn` / `AT LINE n COLUMN n`
  `[WITH {HIGHLIGHT | REVERSE-VIDEO | UNDERLINE}]` — exécutées par positionnement
  de curseur ANSI + SGR en **mode CLI** (`rcrun`) ; ignorées en mode GUI (le form
  designer y supplante les E/S SCREEN). `ACCEPT id AT …` positionne puis lit.

### STRING
- ✅ `STRING {source [DELIMITED BY {SIZE | SPACE[S] | délim}]} … INTO cible
  [WITH POINTER p] [[ON] OVERFLOW imp] [NOT [ON] OVERFLOW imp] [END-STRING]`.
  Débordement = la chaîne assemblée est plus large que le champ récepteur.
- ✅ **Extension — `DELIMITED BY` intelligent par défaut** (lorsque la clause est
  omise sur un opérande) : les éléments alphanumériques `PIC X`/`A` prennent
  `SPACES` par défaut (le remplissage de fin est supprimé) ; les littéraux
  chaîne, les numériques, les numériques édités, les résultats de `FUNCTION` et
  les expressions prennent `SIZE` par défaut. Les data items sont déplacés sous
  leur forme de champ (numérique → chiffres sur toute la largeur du PIC ;
  numérique édité → caractères édités).

### UNSTRING
- ✅ `UNSTRING source [DELIMITED BY [ALL] d [OR [ALL] d …]] INTO {t [DELIMITER IN d]
  [COUNT IN c]} … [TALLYING IN n] [WITH POINTER p] [[ON] OVERFLOW imp]
  [NOT [ON] OVERFLOW imp] [END-UNSTRING]`. Débordement = plus de champs sources
  que de récepteurs.

### INSPECT
- ✅ `INSPECT id CONVERTING de TO vers`.
- ✅ `INSPECT id TALLYING c FOR {CHARACTERS | ALL x | LEADING x | TRAILING x}
  [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT id REPLACING {CHARACTERS | ALL x | LEADING x | TRAILING x | FIRST x}
  BY y [{BEFORE|AFTER} INITIAL d] …`.
- ✅ `INSPECT … TALLYING … REPLACING …` — **les deux moitiés sont appliquées**.
- ✅ `BEFORE/AFTER INITIAL` confine chaque clause à une sous-région du champ.
  (TALLYING cumule sur le compteur, conformément à COBOL.)

### SET
- ✅ `SET t1 [t2 …] TO {TRUE | FALSE | expr}` (compilé en MOVE).
- ✅ `SET idx {UP|DOWN} BY n` (encodé en ADD / SUBTRACT).
- ✅ `SET 88-nom TO TRUE` place dans l'élément hôte la première VALUE de la
  condition ; `TO FALSE` place une valeur hors de l'ensemble des VALUE (au mieux —
  il n'existe pas de clause FALSE).
- ✅ `SET ptr TO {ADDRESS OF id | NULL | autre-ptr}` et
  `SET ADDRESS OF id TO {ADDRESS OF x | ptr | NULL}` — voir **Pointeurs** plus bas.

### INITIALIZE
- ✅ `INITIALIZE id …` — sensible à la catégorie : numérique / numérique édité →
  ZERO, tout le reste → SPACES, en descendant récursivement dans les éléments de
  groupe.
- ✅ `INITIALIZE id REPLACING {ALPHABETIC | ALPHANUMERIC | NUMERIC |
  ALPHANUMERIC-EDITED | NUMERIC-EDITED} [DATA] BY valeur …` — met chaque élément
  subordonné de cette catégorie à la valeur ; les autres restent intacts.

### Pointeurs (USAGE POINTER)
- ✅ `USAGE POINTER` déclare un pointeur (NULL au départ).
- ✅ `SET ptr TO ADDRESS OF id` / `SET ptr TO NULL` / `SET ptr2 TO ptr`.
- ✅ `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` — fait de `id` un alias du
  stockage de la cible (les lectures **et** les écritures suivent l'alias) ;
  typiquement un enregistrement LINKAGE. `IF ptr = NULL` fonctionne.

### CALL / CANCEL
- ✅ `CALL {lit|id} [USING [BY {REFERENCE|CONTENT|VALUE}] arg …] [RETURNING r]
  [[ON] {EXCEPTION|OVERFLOW} imp] [NOT [ON] {EXCEPTION|OVERFLOW} imp] [END-CALL]`.
- ✅ Le corps de `ON EXCEPTION` / `ON OVERFLOW` s'exécute lorsque le programme
  appelé n'est pas résolu ; celui de `NOT ON EXCEPTION` s'exécute lorsque l'appel
  **est résolu**.
- ✅ `CANCEL programme …` réinitialise la WORKING-STORAGE du programme nommé, de
  sorte que son prochain `CALL` reparte de zéro.

### Verbes de fichier (les clauses prises en charge — la couverture complète est dans la suite d'E/S fichiers)
- ✅ `OPEN {INPUT|OUTPUT|I-O|EXTEND} f … [SHARING WITH {ALL OTHER|NO OTHER|READ
  ONLY}] [WITH LOCK] [WITH REGISTERED [USER] {literal|data-item}]` ; `CLOSE f …`.
  (`SHARING` / `WITH LOCK` s'analysent et sont honorés là où cela a un sens —
  indicatifs dans le modèle à unité d'exécution unique.)
- ✅ **`OPEN … WITH REGISTERED [USER] {literal | data-item}`** (extension
  PowerRustCOBOL) — consigne l'opérateur/utilisateur dans le journal
  d'observabilité INDEXED (champ `user=` sur chaque ligne d'événement de la
  session de ce fichier). Purement observationnel ; aucune authentification ni
  autorisation. Voir [`observability.md`](observability.md) §1.3.1.
- ✅ `READ f [RECORD] [{NEXT|PREVIOUS}] [INTO id] [KEY IS k] [WITH [NO] LOCK]
  [AT END …][NOT AT END …][INVALID KEY …][NOT INVALID KEY …][END-READ]`.
  `WITH NO LOCK` libère le verrou d'enregistrement que le moteur INDEXED prend en
  I-O.
- ✅ `UNLOCK f [RECORD[S]]` libère les verrous d'enregistrement du fichier.
- ✅ **`COMMIT` / `ROLLBACK`** — transactions pilotées par le programme sur
  **tous** les fichiers INDEXED ouverts. `OPEN` démarre une transaction ;
  `COMMIT` valide les `WRITE`/`REWRITE`/`DELETE` en attente (un `ROLLBACK`
  ultérieur ne peut plus les annuler) et en démarre une nouvelle ; `ROLLBACK`
  annule toute modification depuis le dernier `COMMIT`/`OPEN`. Le stockage
  **DISK** rend `COMMIT`/`CLOSE` durables sur disque. Le stockage **MEMORY**
  garde `COMMIT`/`ROLLBACK` purement en RAM (aucune écriture disque) ; un fichier
  `STORAGE IS MEMORY` ordinaire est éphémère, et
  `STORAGE IS MEMORY WITH PERSISTENCE` enregistre sur disque au `CLOSE`
  seulement. (La reprise après panne via un journal write-ahead durable reste à
  faire — il s'agit ici d'une annulation au niveau du programme, en cours
  d'exécution.)
- ✅ **`SELECT … STORAGE [MODE] IS MEMORY | DISK [WITH COMPRESSION] [WITH
  PERSISTENCE]`** (fichiers INDEXED ; extension PowerRustCOBOL). Le stockage par
  défaut est `DISK`. `WITH COMPRESSION` compresse l'enregistrement stocké (les
  clés sont évaluées sur l'enregistrement non compressé) ; `WITH PERSISTENCE`
  (MEMORY uniquement) enregistre le fichier en RAM au `CLOSE`. `OPEN OUTPUT`
  (re)crée toujours le conteneur sur disque.
- ✅ `WRITE rec [FROM id] [{BEFORE|AFTER} ADVANCING n [LINE[S]]]
  [INVALID KEY …][NOT …][END-WRITE]`.
- ✅ `REWRITE rec [FROM id] [INVALID KEY …][END-REWRITE]` ;
  `DELETE f [RECORD] [INVALID KEY …][END-DELETE]`.
- ✅ `START f [KEY IS {= | > | >= | < | <= | NOT … | GREATER [THAN] [OR EQUAL TO]
  | LESS [THAN] [OR EQUAL TO]} k] [INVALID KEY …][END-START]`.
- ⚠️ Le partage de fichiers entre *processus* n'est pas imposé (unité d'exécution
  unique) ; les clauses `SHARING`/`LOCK` s'analysent et les verrous
  d'enregistrement par exécution du moteur INDEXED sont honorés.

### SORT / MERGE / RELEASE / RETURN  ✅ (fonctionnels, tampon de travail en mémoire)
- ✅ `SORT f [ON] {ASCENDING|DESCENDING} KEY k … {USING f1 … | INPUT PROCEDURE p}
  {GIVING f2 … | OUTPUT PROCEDURE p} [END-SORT]`.
- ✅ `MERGE f [ON] {ASCENDING|DESCENDING} KEY k … USING f1 f2 …
  {GIVING f3 … | OUTPUT PROCEDURE p} [END-MERGE]`.
- ✅ `RELEASE record [FROM id]` (dans une INPUT PROCEDURE) ajoute à l'exécution ;
  `RETURN f [INTO id] AT END … [NOT AT END …] [END-RETURN]` restitue les
  enregistrements.
- Les enregistrements sont triés de façon stable selon les clés déclarées
  (`ASCENDING`/`DESCENDING`) ; `USING` lit et `GIVING` écrit les fichiers
  séquentiels nommés.

---

## Conditions (IF / EVALUATE / PERFORM UNTIL)

- ✅ Symboles relationnels : `=` `<>` `<` `>` `<=` `>=`.
- ✅ Relations en mots : `[IS] [NOT] EQUAL TO`, `[IS] [NOT] GREATER [THAN]
  [OR EQUAL TO]`, `[IS] [NOT] LESS [THAN] [OR EQUAL TO]`.
- ✅ Classe : `id IS [NOT] {NUMERIC | ALPHABETIC | ALPHABETIC-LOWER | ALPHABETIC-UPPER}`.
- ✅ Signe : `id IS [NOT] {POSITIVE | NEGATIVE | ZERO}`.
- ✅ Nom-condition de niveau 88 (le nom seul comme condition).
- ✅ `AND` / `OR` / `NOT` combinés, parenthèses (AND lie plus fort que OR).
- ✅ **Conditions abrégées préfixées par un opérateur** — `a > 1 AND < 9`,
  `a = 5 OR = 7` (le sujet de la comparaison précédente est réutilisé).
- ✅ **Abréviation à objet littéral** — `a = 1 OR 2 OR 3` (réutilise à la fois le
  sujet et l'opérateur ; l'objet est un littéral).
- ✅ **Abréviation à objet identificateur** — `a = b OR c` (où `c` est un data
  item). Un identificateur seul après AND/OR qui suit une comparaison est résolu à
  l'exécution : s'il s'agit d'un nom-condition de niveau 88 connu, il s'évalue
  comme tel ; sinon, c'est l'objet `a = c`. (Un identificateur immédiatement suivi
  de `AND` conserve la précédence de AND.)

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
  (Les conversions de date utilisent la base standard 1601-01-01 = jour 1.) Le
  **jeu complet d'intrinsèques du standard COBOL-85** est implémenté.
  ⚠️ Tout nom de `FUNCTION` non reconnu s'analyse quand même, mais renvoie **0** à
  l'exécution.
- ✅ Littéraux : entier, décimal, chaîne, toutes les constantes figuratives
  (`SPACES/SPACE, ZEROS/ZERO/ZEROES, HIGH-VALUES, LOW-VALUES, QUOTES, NULLS`,
  `ALL "x"`).
- ✅ **Littéraux hexadécimaux** — `X"09"`, `x'0D0A'` (indifféremment en majuscules
  ou minuscules, avec l'un ou l'autre type de guillemets). Un caractère par
  **paire** de chiffres hexadécimaux : le nombre de chiffres doit donc être pair ;
  un nombre impair ou un chiffre non hexadécimal constitue un littéral mal formé
  et est signalé, plutôt que relu silencieusement comme le mot `X` accolé à une
  chaîne. Utilisables partout où un littéral entre guillemets l'est
  (`DELIMITED BY`, `MOVE`, `VALUE`, comparaisons).

---

## Clauses de la DATA DIVISION (syntaxe de déclaration acceptée)

- ✅ Niveaux `01`–`49`, `77`, `88` ; `FILLER` ; de groupe / élémentaires.
- ✅ `PIC/PICTURE` avec `X A 9 S V P` et les symboles d'édition (`Z * $ + - CR DB
  B 0 / , .`).
- ✅ `USAGE [IS] {DISPLAY | BINARY | COMP | COMP-1 | COMP-2 | COMP-3 |
  PACKED-DECIMAL | COMP-5}` (et `COMP-4`→COMP, `COMP-X`→COMP-5).
- ✅ `VALUE` (numérique/signé/alphanumérique/figuratif/`ALL`).
- ✅ `OCCURS n [TIMES] [DEPENDING ON id] [ASCENDING/DESCENDING KEY …] [INDEXED BY …]`.
- ✅ `REDEFINES`, `JUSTIFIED [RIGHT]`, `SYNCHRONIZED/SYNC`, `BLANK [WHEN] ZERO`,
  `SIGN [IS] {LEADING|TRAILING} [SEPARATE]`, `GLOBAL`, `EXTERNAL`.
- ✅ `88 nom VALUE v [v …]` / `VALUE a THRU b` — **noms-conditions réels** : le
  niveau 88 se lie à son élément hôte ; le test compare l'hôte aux VALUE / plages,
  et `SET 88-nom TO TRUE` range dans l'hôte une valeur qui la satisfait.
- ✅ `USAGE INDEX` déclare un registre d'index entier (`SET`/`SEARCH` s'en
  servent) ; `USAGE POINTER` — voir **Pointeurs** plus haut.
- ✅ `66 NEW RENAMES élément-1 [{THRU|THROUGH} élément-2]` — un alias de
  regroupement ; la lecture concatène les éléments couverts, l'écriture les
  répartit selon la largeur des champs.
- Sections : `WORKING-STORAGE`, `LOCAL-STORAGE`, `LINKAGE`, `FILE` ; `SCREEN` est
  analysée mais non exécutée.

---

## Toujours PAS pris en charge — liste actuelle des éléments à éviter

Le jeu de verbes et de clauses COBOL-85 est **entièrement couvert**. Ce qui reste
hors périmètre est intentionnel ou postérieur à 85 :

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
4. Organisation de fichiers **RELATIVE** (SEQUENTIAL / LINE SEQUENTIAL / INDEXED
   sont faites).
5. Les noms de fonction intrinsèque non reconnus renvoient toujours **0**.

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

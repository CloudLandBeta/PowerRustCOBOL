<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Matrice de tests des verbes et des sections de données RustCOBOL‑85

Une spécification de tests pour achever COBOL‑85 dans le périmètre du projet.
Elle énumère, **en profondeur**, ce qui n'est *pas encore couvert* par les suites
existantes, sous forme de squelettes de syntaxe + axes de permutation + le
mélange de types de données avec lequel chaque verbe doit être éprouvé.
L'objectif de ces tests est **exploratoire** : exécuter chaque variante, observer
le comportement actuel, et décider quoi corriger, ajuster, créer ou supprimer.

> Déjà vérifié — NE PAS respécifier ici : l'arithmétique numérique exacte
> (valeurs de résultat d'ADD/SUB/MUL/DIV/COMPUTE, ROUNDED, ON SIZE ERROR), les
> PICTURE numériques éditées + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, toute
> l'E/S fichier (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, clés,
> START/REWRITE/DELETE/INVALID KEY, STORAGE MODE MEMORY/DISK, compression,
> persistance MEMORY), programmes imbriqués et CALL de base, comparaison
> alphanumérique, analyseur lexical fixe/libre. (Les permutations de *syntaxe*
> arithmétique ci-dessous restent dans le périmètre — seul le calcul des valeurs
> est « fait ».)

## Notation

- `[ x ]` optionnel, `{ a | b }` choix, `…` répétition, `dn` = élément de
  données n.
- **Axe de mélange de types (T) :** chaque emplacement d'opérande doit être
  éprouvé avec ces espèces de récepteur et d'émetteur, dans les deux sens le cas
  échéant :
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Valeurs limites par espèce :** vide, minimum, maximum, débordement d'une
  unité, tout espaces, tout zéros, signe en LEADING/TRAILING [SEPARATE], mise à
  l'échelle par P, virgule implicite par V.
- Pour chaque verbe, relever : la ou les valeurs de résultat, **FILE STATUS et
  les registres spéciaux** (`RETURN-CODE`, `TALLY`), la branche de
  débordement/exception empruntée, et le fait que rien ne change en cas d'erreur.

---

## Partie A — Sections de la DATA DIVISION (comportements non testés)

### WORKING-STORAGE SECTION
- **Niveaux :** 01, imbrication 02–49, 77 (indépendant), 66
  `RENAMES a THRU b`, 88.
- **PIC :** `X A 9 S V P` avec `(n)` ; mise à l'échelle par `P` (gauche/droite) ;
  virgule implicite par `V` ; combinaisons éditées ; groupe avec `PIC` contre
  groupe sans `PIC`.
- **USAGE :** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2,
  COMP‑3/PACKED‑DECIMAL, COMP‑5, INDEX, POINTER — déclaration, taille de stockage
  et aller-retour de la valeur.
- **VALUE :** numérique, signé, alphanumérique, figuratif, `ALL "x"` ; VALUE sur
  un groupe ; VALUE illégale (taille > PIC).
- **OCCURS :** fixe ; `DEPENDING ON` ; `INDEXED BY` ;
  `ASCENDING/DESCENDING KEY` ; plusieurs dimensions (2–3) ; OCCURS sur un groupe.
- **Clauses :** REDEFINES (égal, plus petit, plus grand, chaîné), RENAMES,
  JUSTIFIED RIGHT, BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`,
  SYNCHRONIZED, FILLER.
- **Noms-conditions 88 :** valeur unique, liste de valeurs, `VALUE a THRU b`,
  plages multiples, sur hôte numérique / alphanumérique / édité ; évaluation et
  `SET … TO TRUE`.
- **Initialisation :** par défaut (espaces/zéros selon la classe) contre VALUE ;
  **persistance au travers de PERFORM et au travers de CALL** (la WS conserve la
  dernière valeur).

### LOCAL-STORAGE SECTION
- **Réinitialisée à chaque entrée dans le programme** (par contraste avec la
  persistance de la WS).
- Les clauses VALUE sont **réappliquées à chaque entrée**.
- **Récursion :** chaque CALL (récursive) obtient une instance indépendante de
  LOCAL-STORAGE.
- La même couverture de clauses que la WS (OCCURS/REDEFINES/88/…), mais en
  vérifiant la sémantique de réinitialisation.

### LINKAGE SECTION
- Les éléments **n'ont pas de stockage tant que l'appelant ne les a pas liés** ;
  accès à une liaison non liée.
- Liés via `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (l'appelant voit les modifications) contre **BY CONTENT**
  (l'appelé modifie une copie) contre **BY VALUE** (scalaire).
- Groupe et élémentaire, OCCURS, REDEFINES, 88 dans la section de liaison.
- Divergence de taille ou d'USAGE entre le paramètre effectif et le paramètre
  formel (comportement à observer).
- `ADDRESS OF` / `SET ADDRESS OF … TO` et liaison de POINTER (si pris en charge).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — liaison positionnelle aux arguments du
  CALL ; divergence de nombre (moins ou plus d'arguments) ; ordre.
- `BY REFERENCE | BY VALUE` par paramètre dans la liste USING.
- `RETURNING dn` — valeur rendue à `CALL … RETURNING` ; contre `GIVING` ; contre
  `RETURN-CODE`.
- Le `USING` du programme principal lié depuis la ligne de commande (si pris en
  charge).
- Mélange de types sur chaque emplacement de paramètre (appliquer **T**).

---

## Partie B — Matrice de permutation des verbes

Éprouvez chaque verbe le long de **T** pour chaque emplacement d'opérande. Ce qui
suit énumère les permutations *structurelles* (clauses et phrases) qui s'ajoutent
au mélange de types.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (plusieurs récepteurs).
- `MOVE CORRESPONDING g1 TO g2` (élémentaires correspondant par le nom).
- Source et cible en modification de référence : `MOVE a(s:l) TO b(s:l)`.
- Avec indices : `MOVE t(i) TO u(j)`, `t(i,j)`.
- Conversions de type (appliquer **T** dans les deux sens) : num→édité,
  édité→num, alphanum→num, num→alphanum (justification/remplissage/troncature),
  groupe→groupe (copie d'octets), gestion du signe, COMP‑3↔DISPLAY,
  flottant↔fixe, figuratif→chaque espèce.

### DISPLAY
- `DISPLAY {dn|literal} …` (opérandes concaténés).
- `[WITH NO ADVANCING]` ; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Forme écran (observer et décider) : `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Mélange de types : numérique (largeur complète de la PIC), édité, signé,
  groupe, figuratif.

### ACCEPT  *(spécifier toutes les formes ; beaucoup sont écran/terminal — à signaler pour une décision de périmètre)*
- `ACCEPT dn` (depuis la console vers alphanum / numérique / édité / groupe).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Formes écran : `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Réception dans un numérique contre un numérique édité contre un alphanumérique
  (dé-édition et validation).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Plusieurs récepteurs, chacun avec son propre comportement de ROUNDED et de
  taille ; opérandes d'USAGE mélangée (COMP‑3 + DISPLAY + édité) ; signés ;
  opérandes en modification de référence.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- Division par zéro → ON SIZE ERROR ; signe et échelle de REMAINDER ; USAGE
  mélangée.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Opérateurs `+ - * / **`, parenthèses, priorité ; fonctions intrinsèques dans
  l'expression ; opérandes d'USAGE mélangée ; plusieurs récepteurs ; troncature
  contre ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — imbrication, branches vides,
  `NEXT SENTENCE`.
- Conditions : de relation (`= < > <= >= NOT`), de classe
  (`IS [NOT] {NUMERIC|ALPHABETIC|ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), de signe
  (`POSITIVE|NEGATIVE|ZERO`), référence à une condition 88, combinées
  (`AND/OR/NOT`), **abrégées** (`a = b OR c`), parenthésées.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` avec
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Mélange de types dans les comparaisons (num contre alphanum contre édité contre
  figuratif).

### PERFORM
- Hors ligne : `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = littéral ou élément de données).
- `PERFORM … UNTIL cond` avec `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- En ligne : `PERFORM … END-PERFORM` (avec TIMES/UNTIL/VARYING).
- PERFORM imbriquée et récursive ; chevauchement de plages ; index contre
  variable de boucle numérique.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p` ; `GO TO p1 p2 … DEPENDING ON dn` (dans et hors plage).
- `CONTINUE` ; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (depuis le principal et depuis un
  sous-programme).

### SET
- `SET index TO {n|index}` ; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}` ; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (là où c'est pris en charge).

### INITIALIZE
- `INITIALIZE dn …` (groupe ou élémentaire ; par défaut selon la catégorie).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]` ; tables (toutes les occurrences).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (série).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH`
  (binaire ; exige `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Trouvé et non trouvé ; plusieurs WHEN ; mélange de types de clé ; comportement
  avec une table non triée.

### STRING  *(éprouver le style de permutation de l'utilisateur)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutations à couvrir :
  - une seule source en `DELIMITED BY SIZE` → cible alphanumérique.
  - plusieurs sources, **délimiteurs mélangés** : `STRING "lit" DELIMITED BY SIZE
    d1 DELIMITED BY SPACES INTO d3`.
  - de nombreuses sources et délimiteurs : `STRING "l1" DELIMITED BY SIZE "l2"
    DELIMITED BY SIZE d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` pour démarrer et avancer ; pointeur hors plage →
    débordement.
  - cible trop petite → `ON OVERFLOW` ; `NOT ON OVERFLOW`.
  - **sources de types mélangés :** numérique, numérique édité, signé, groupe,
    figuratif, en modification de référence — observer comment chacun est
    converti en chaîne.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutations : un contre plusieurs délimiteurs, `ALL` (fusion des répétitions),
  `OR`, capture par `DELIMITER IN`/`COUNT IN`, POINTER, TALLYING, plus de champs
  que de données (débordement), cibles de types mélangés (les récepteurs
  numériques sont dé-édités).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combiné).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- Portée de BEFORE/AFTER ; correspondances qui se chevauchent ; motifs de
  plusieurs caractères ; hôte de types mélangés.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Nom de programme statique (littéral) contre dynamique (nom de donnée) ; non
  résolu → ON EXCEPTION.
- Modes de passage des arguments (observer la visibilité côté appelant) ;
  divergence de nombre ou de type d'arguments.
- `RETURNING` contre `RETURN-CODE` ; récursion ; données partagées `EXTERNAL`.
  (✅ `CANCEL prog` implémenté — réinitialise le stockage du programme ;
  `NOT ON EXCEPTION` s'exécute sur un CALL résolu.)

### Registres spéciaux arithmétiques et verbes divers
- `ADD/SUBTRACT … GIVING` (suppression à zéro) contre l'accumulation de `TO`.
- `MOVE` et arithmétique vers et depuis `RETURN-CODE`, `TALLY`.
- ✅ `ALTER` (le GO TO hérité) — implémenté (redirige le `GO TO` du paragraphe).
- Aller-retour `ACCEPT/DISPLAY` au travers de champs édités.

### Verbes fichier — *(uniquement les manques que la suite d'E/S fichier ne couvre pas)*
- ✅ **Implémenté et testé** (`test_file_locking`) : `OPEN … SHARING WITH …
  [WITH LOCK]`, `READ … WITH [NO] LOCK`, `UNLOCK` (indicatif au sein de l'unité
  d'exécution — voir la référence de syntaxe prise en charge).
- `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  avec des clés en modification de référence ; plusieurs FD partageant une zone
  d'enregistrement.

### Verbes spécifiés ici avant d'exister

> **Tous sont implémentés.** SORT/MERGE/RELEASE/RETURN sont arrivés en 1.62.119
> et le moteur RELATIVE en 1.62.76
> (`crates/cobolt-runtime/src/relative.rs`) ;
> `docs/cobol85-supported-syntax-en.md` les marque ✅. Les axes de permutation
> ci-dessous restent le plan de tests qu'ils ont toujours été : ils décrivent ce
> qu'il reste à *couvrir*, non ce qu'il reste à construire.
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}` ; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- Organisation `RELATIVE` : `READ/WRITE/REWRITE/DELETE/START` par
  `RELATIVE KEY`.

---

## Partie C — Banc d'équivalence entre formes

Pour un ensemble choisi des programmes ci-dessus, vérifier que la sortie
observable est **identique** (texte des DISPLAY, FILE STATUS, RETURN-CODE,
contenu des fichiers) dans les trois formes d'exécution d'une même source :

1. **Interpréteur** (`Interpreter::run`).
2. **Aller-retour de l'AST** — sérialiser (`bincode`+`flate2`) → désérialiser →
   exécuter ; vérifier que l'AST est identique octet pour octet et que la sortie
   concorde.
3. **Binaire empaqueté/compilé** — `cobolt_compiler::build_project` → exécuter le
   binaire produit ; vérifier que la sortie est identique.

Toute divergence entre formes est un défaut à consigner (l'invariant « un
compilateur, un comportement »).

.<<

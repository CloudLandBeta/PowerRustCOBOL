<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Matrice de tests des verbes et des sections de données RustCOBOL‑85

Une spécification de tests pour achever COBOL‑85 dans le périmètre du projet. Elle
énumère, **en profondeur**, ce qui n'est *pas encore couvert* par les suites
existantes, sous forme de squelettes de syntaxe + axes de permutation + le mélange
de types de données avec lequel chaque verbe doit être exercé. L'objectif de ces
tests est **exploratoire** : exécuter chaque variante, observer le comportement
actuel, et décider quoi corriger / ajuster / créer / supprimer.

> Déjà vérifié — NE PAS respécifier ici : l'arithmétique numérique exacte
> (valeurs de résultat de ADD/SUB/MUL/DIV/COMPUTE, ROUNDED, ON SIZE ERROR), les
> PICTUREs numeric‑edited + `DECIMAL-POINT IS COMMA`, COPY/REPLACE, toutes les
> E/S fichiers (SEQUENTIAL/LINE SEQUENTIAL/INDEXED, clés,
> START/REWRITE/DELETE/INVALID KEY, STORAGE MODE MEMORY/DISK, compression,
> persistance de MEMORY), les programmes imbriqués/CALL de base, la comparaison
> alphanumérique, le lexer fixe/libre. (Les permutations de *syntaxe*
> arithmétique ci-dessous restent dans le périmètre — seul le calcul des valeurs
> est « terminé ».)

## Notation

- `[ x ]` facultatif, `{ a | b }` choix, `…` répétition, `dn` = élément de données n.
- **Axe de mélange de types (T) :** chaque emplacement d'opérande doit être exercé sur
  ces sortes de récepteur/émetteur, dans les deux sens le cas échéant :
  `T = { alphanumeric X(n) | alphabetic A(n) | unsigned-num 9(n) | signed-num S9(n)V9(n) | numeric-edited (Z,*,$,+,-,CR,DB,/,B,0,comma,period) | COMP/COMP-4 | COMP-3 | COMP-5 | COMP-1/2 float | group | 88-condition | INDEX | POINTER | literal | figurative (SPACES/ZEROS/HIGH/LOW/QUOTES/ALL) | reference-modified d(s:l) | subscripted t(i)/t(i,j) }`.
- **Valeurs limites par sorte :** vide, minimum, maximum, débordement d'une unité,
  tout espaces, tout zéros, signe en LEADING/TRAILING [SEPARATE], mise à l'échelle par P,
  virgule implicite par V.
- Pour chaque verbe, capturer : la ou les valeurs de résultat, **FILE STATUS / registres spéciaux**
  (`RETURN-CODE`, `TALLY`), la branche de débordement/exception empruntée, et l'inchangé-en-erreur.

---

## Partie A — Sections de la DATA DIVISION (comportements non testés)

### WORKING-STORAGE SECTION
- **Niveaux :** 01, imbrication 02–49, 77 (indépendant), 66 `RENAMES a THRU b`, 88.
- **PIC :** `X A 9 S V P` avec `(n)` ; mise à l'échelle par `P` (gauche/droite) ; virgule
  implicite `V` ; combinaisons éditées ; groupe avec `PIC` face au groupe sans PIC.
- **USAGE :** DISPLAY, COMP/COMP‑4/BINARY, COMP‑1, COMP‑2, COMP‑3/PACKED‑DECIMAL,
  COMP‑5, INDEX, POINTER — déclaration + taille de stockage + aller-retour de la valeur.
- **VALUE :** numérique, signé, alphanumérique, figuratif, `ALL "x"` ; VALUE sur un groupe ;
  VALUE illégal (taille > PIC).
- **OCCURS :** fixe ; `DEPENDING ON` ; `INDEXED BY` ; `ASCENDING/DESCENDING KEY` ;
  multidimensionnel (2–3) ; OCCURS sur un groupe.
- **Clauses :** REDEFINES (identique/plus petit/plus grand, chaîné), RENAMES, JUSTIFIED RIGHT,
  BLANK WHEN ZERO, `SIGN IS {LEADING|TRAILING} [SEPARATE]`, SYNCHRONIZED, FILLER.
- **Noms-conditions 88 :** valeur unique, liste de valeurs, `VALUE a THRU b`, plusieurs
  plages, sur un hôte numérique / alphanumérique / édité ; évaluation + `SET … TO TRUE`.
- **Initialisation :** par défaut (espaces/zéros selon la classe) face à VALUE ; **persistance
  à travers PERFORM et à travers CALL** (la WS conserve la dernière valeur).

### LOCAL-STORAGE SECTION
- **Réinitialisée à chaque entrée dans le programme** (par contraste avec la persistance de la WS).
- Clauses VALUE **réappliquées à chaque entrée**.
- **Récursion :** chaque CALL (récursif) obtient une instance indépendante de LOCAL-STORAGE.
- La même couverture de clauses que la WS (OCCURS/REDEFINES/88/…) mais en vérifiant la sémantique de réinitialisation.

### LINKAGE SECTION
- Les éléments **n'ont pas de stockage tant que l'appelant ne les a pas liés** ; accès à une linkage non liée.
- Liés via `CALL … USING` ↔ `PROCEDURE DIVISION USING`.
- **BY REFERENCE** (l'appelant voit les modifications) face à **BY CONTENT** (l'appelé modifie une copie)
  face à **BY VALUE** (scalaire).
- Groupe + élémentaire, OCCURS, REDEFINES, 88 dans la linkage.
- Écart de taille/USAGE entre le paramètre effectif et le paramètre formel (comportement à observer).
- `ADDRESS OF` / `SET ADDRESS OF … TO` et liaison de POINTER (si pris en charge).

### PROCEDURE DIVISION USING … RETURNING …
- `PROCEDURE DIVISION USING d1 d2 …` — liaison positionnelle aux arguments de CALL ;
  écart de nombre (moins/plus d'arguments) ; ordre.
- `BY REFERENCE | BY VALUE` par paramètre sur la liste USING.
- `RETURNING dn` — valeur rendue à `CALL … RETURNING` ; face à `GIVING` ; face à
  `RETURN-CODE`.
- `USING` du programme principal lié depuis la ligne de commande (si pris en charge).
- Mélange de types sur chaque emplacement de paramètre (appliquer **T**).

---

## Partie B — Matrice de permutations des verbes

Exercez chaque verbe sur toute l'étendue de **T** pour chaque emplacement d'opérande.
Ci-dessous sont listées les permutations *structurelles* (clauses/phrases) qui viennent
s'ajouter au mélange de types.

### MOVE
- `MOVE {dn|literal|figurative} TO d1 [d2 …]` (plusieurs récepteurs).
- `MOVE CORRESPONDING g1 TO g2` (appariement des élémentaires par le nom).
- Source/cible avec modification de référence : `MOVE a(s:l) TO b(s:l)`.
- Indicé : `MOVE t(i) TO u(j)`, `t(i,j)`.
- Conversions de type (appliquer **T** dans les deux sens) : num→edited, edited→num,
  alnum→num, num→alnum (justifier/compléter/tronquer), group→group (copie d'octets),
  traitement du signe, COMP‑3↔DISPLAY, float↔fixed, figurative→chaque sorte.

### DISPLAY
- `DISPLAY {dn|literal} …` (opérandes concaténés).
- `[WITH NO ADVANCING]` ; `UPON {CONSOLE|SYSOUT|mnemonic}`.
- Forme écran (observer/décider) : `DISPLAY dn AT {nnnn|LINE n COLUMN n}
  [WITH {FOREGROUND-COLOR n|BACKGROUND-COLOR n|HIGHLIGHT|REVERSE-VIDEO|BLINK|…}]`.
- Mélange de types : numérique (largeur PIC complète), édité, signé, groupe, figuratif.

### ACCEPT  *(spécifier toutes les formes ; beaucoup sont écran/terminal — à signaler pour une décision de périmètre)*
- `ACCEPT dn` (depuis la console vers alnum / numeric / edited / group).
- `ACCEPT dn FROM {DATE|DATE YYYYMMDD|DAY|DAY YYYYDDD|DAY-OF-WEEK|TIME}`.
- `ACCEPT dn FROM {ENVIRONMENT "NAME"|ENVIRONMENT-NAME|ENVIRONMENT-VALUE}`.
- `ACCEPT dn FROM {COMMAND-LINE|ARGUMENT-NUMBER|ARGUMENT-VALUE}`.
- `ACCEPT dn FROM {mnemonic|CONSOLE|SYSIN}`.
- Formes écran : `ACCEPT dn AT {nnnn|LINE n COL n}`,
  `ACCEPT dn AT 0101 WITH CONTROL screen-attrs`,
  `… WITH {AUTO|SECURE|REQUIRED|FULL|UPDATE|PROMPT|NO-ECHO|…}`,
  `ACCEPT dn FROM ESCAPE KEY` / `FROM CRT STATUS`.
- Réception dans un numérique face à un numeric-edited face à un alnum (dé-édition / validation).

### ADD / SUBTRACT
- `ADD {dn|lit} … TO d1 [d2 …] [ROUNDED] [ON SIZE ERROR …][NOT…][END-ADD]`.
- `ADD {dn|lit} … GIVING d1 [d2 …] [ROUNDED]…`.
- `ADD CORRESPONDING g1 TO g2 [ROUNDED][ON SIZE ERROR…]`.
- `SUBTRACT … FROM …`, `SUBTRACT … GIVING …`, `SUBTRACT CORRESPONDING …`.
- Plusieurs récepteurs, chacun avec son propre comportement ROUNDED/de taille ; opérandes
  d'USAGE mixtes (COMP‑3 + DISPLAY + édité) ; signés ; opérandes avec modification de référence.

### MULTIPLY / DIVIDE
- `MULTIPLY {dn|lit} BY d1 [d2…] [ROUNDED]…` / `… GIVING …`.
- `DIVIDE a INTO d1 [d2…] [ROUNDED]` / `DIVIDE a INTO b GIVING q [ROUNDED]
  [REMAINDER r]` / `DIVIDE a BY b GIVING q [REMAINDER r]`.
- Division par zéro → ON SIZE ERROR ; signe/échelle du REMAINDER ; USAGE mixtes.

### COMPUTE
- `COMPUTE d1 [d2…] [ROUNDED] = expr [ON SIZE ERROR…][NOT…][END-COMPUTE]`.
- Opérateurs `+ - * / **`, parenthèses, priorité ; fonctions intrinsèques dans l'expression ;
  opérandes d'USAGE mixtes ; plusieurs récepteurs ; troncature face à ROUNDED.

### IF / EVALUATE
- `IF cond THEN … [ELSE …] END-IF` — imbrication, branches vides, `NEXT SENTENCE`.
- Conditions : de relation (`= < > <= >= NOT`), de classe (`IS [NOT] {NUMERIC|ALPHABETIC|
  ALPHABETIC-UPPER|ALPHABETIC-LOWER}`), de signe (`POSITIVE|NEGATIVE|ZERO`),
  référence à une condition 88, combinées (`AND/OR/NOT`), **abrégées** (`a = b OR c`),
  parenthésées.
- `EVALUATE {subj1 [ALSO subj2 …] | TRUE | FALSE}` avec
  `WHEN {val | val THRU val | ANY | cond | TRUE} [ALSO …] … [WHEN OTHER] END-EVALUATE`.
- Mélange de types dans les comparaisons (numérique face à alnum face à édité face à figuratif).

### PERFORM
- Hors ligne `PERFORM p1 [THRU p2]`.
- `PERFORM p [THRU p2] n TIMES` (n = littéral / élément de données).
- `PERFORM … UNTIL cond` avec `[WITH TEST {BEFORE|AFTER}]`.
- `PERFORM … VARYING v FROM a BY b UNTIL c [AFTER v2 FROM … BY … UNTIL …]`.
- `PERFORM … END-PERFORM` en ligne (avec TIMES/UNTIL/VARYING).
- PERFORM imbriqué/récursif ; chevauchement de plages ; variable de boucle index face à numérique.

### GO TO / CONTINUE / EXIT / STOP
- `GO TO p` ; `GO TO p1 p2 … DEPENDING ON dn` (dans/hors de la plage).
- `CONTINUE` ; `NEXT SENTENCE`.
- `EXIT`, `EXIT PERFORM [CYCLE]`, `EXIT PROGRAM`, `EXIT PARAGRAPH/SECTION`.
- `STOP RUN`, `STOP literal`, `GOBACK` (depuis le principal face à un sous-programme).

### SET
- `SET index TO {n|index}` ; `SET index {UP|DOWN} BY n`.
- `SET 88-name TO TRUE`.
- `SET pointer TO {ADDRESS OF dn|NULL}` ; `SET ADDRESS OF linkage TO pointer`.
- `SET d1 TO {TRUE|FALSE}` (là où c'est pris en charge).

### INITIALIZE
- `INITIALIZE dn …` (groupe/élémentaire ; par défaut selon la catégorie).
- `INITIALIZE dn REPLACING {ALPHANUMERIC|NUMERIC|ALPHABETIC|NUMERIC-EDITED|
  ALPHANUMERIC-EDITED} DATA BY {lit|dn} …`.
- `[WITH FILLER]`, `[THEN TO DEFAULT]` ; tables (toutes les occurrences).

### SEARCH / SEARCH ALL
- `SEARCH t [VARYING idx] [AT END …] WHEN cond … [END-SEARCH]` (série).
- `SEARCH ALL t [AT END …] WHEN key = val [AND key2 = val2] END-SEARCH` (binaire ;
  exige `ASCENDING/DESCENDING KEY` + `INDEXED BY`).
- Trouvé/non trouvé ; plusieurs WHEN ; mélange de types de clé ; comportement avec une table non triée.

### STRING  *(exercer le style de permutation de l'utilisateur)*
- `STRING {dn|lit} … DELIMITED BY {SIZE|lit|dn} [ {dn|lit}… DELIMITED BY … ]…
   INTO target [WITH POINTER p] [ON OVERFLOW …][NOT…][END-STRING]`.
- Permutations à couvrir :
  - source unique `DELIMITED BY SIZE` → cible alnum.
  - plusieurs sources, **délimiteurs mixtes** : `STRING "lit" DELIMITED BY SIZE d1
    DELIMITED BY SPACES INTO d3`.
  - nombreuses sources/délimiteurs : `STRING "l1" DELIMITED BY SIZE "l2" DELIMITED BY SIZE
    d1 d2 d3 DELIMITED BY SPACES INTO d3`.
  - `WITH POINTER` début/avancement ; pointeur hors plage → débordement.
  - cible trop petite → `ON OVERFLOW` ; `NOT ON OVERFLOW`.
  - **sources de types mélangés :** numérique, numeric-edited, signé, groupe, figuratif,
    avec modification de référence — observer comment chacun est mis en chaîne.

### UNSTRING
- `UNSTRING src [DELIMITED BY [ALL] {lit|dn} [OR [ALL] …]]
   INTO {t1 [DELIMITER IN d] [COUNT IN c]} … [WITH POINTER p] [TALLYING IN n]
   [ON OVERFLOW …][NOT…][END-UNSTRING]`.
- Permutations : un seul délimiteur face à plusieurs, `ALL` (fusionne les répétitions), `OR`,
  capture par `DELIMITER IN`/`COUNT IN`, POINTER, TALLYING, plus de champs que de données
  (débordement), cibles de types mélangés (les récepteurs numériques sont dé-édités).

### INSPECT
- `INSPECT dn TALLYING c FOR {ALL|LEADING|CHARACTERS} {lit|dn}
   [{BEFORE|AFTER} INITIAL {lit|dn}] …`.
- `INSPECT dn REPLACING {ALL|LEADING|FIRST|CHARACTERS} {lit} BY {lit}
   [{BEFORE|AFTER} INITIAL …] …`.
- `INSPECT dn TALLYING … REPLACING …` (combiné).
- `INSPECT dn CONVERTING "abc" TO "xyz" [{BEFORE|AFTER} INITIAL …]`.
- Portée BEFORE/AFTER ; correspondances qui se chevauchent ; motifs multicaractères ; hôte de types mélangés.

### CALL / CANCEL
- `CALL {lit|dn} [USING {[BY REFERENCE|BY CONTENT|BY VALUE] {dn|lit|OMITTED}}…]
   [RETURNING dn] [ON {EXCEPTION|OVERFLOW} …][NOT…][END-CALL]`.
- Nom de programme statique (littéral) face à dynamique (nom de donnée) ; non résolu → ON EXCEPTION.
- Modes de passage des arguments (observer la visibilité côté appelant) ; écart de nombre/type d'arguments.
- `RETURNING` face à `RETURN-CODE` ; récursion ; données partagées `EXTERNAL`.
  (✅ `CANCEL prog` implémenté — réinitialise le stockage du programme ;
  `NOT ON EXCEPTION` s'exécute sur un CALL résolu.)

### Registres spéciaux d'ARITHMETIC et verbes divers
- Suppression des zéros de `ADD/SUBTRACT … GIVING` face à l'accumulation de `TO`.
- `MOVE`/arithmétique vers/depuis `RETURN-CODE`, `TALLY`.
- ✅ `ALTER` (GO TO hérité) — implémenté (redirige le `GO TO` du paragraphe).
- Aller-retour de `ACCEPT/DISPLAY` à travers des champs édités.

### Verbes fichiers — *(uniquement les lacunes absentes de la suite d'E/S fichiers)*
- ✅ **Implémenté et testé** (`test_file_locking`) : `OPEN … SHARING WITH …
  [WITH LOCK]`, `READ … WITH [NO] LOCK`, `UNLOCK` (indicatif au sein de l'unique
  unité d'exécution — voir la référence de syntaxe prise en charge).
- `READ … INTO`, `WRITE … FROM`, `REWRITE … FROM`, `START … KEY IS {= > >= < <=}`
  avec des clés à modification de référence ; plusieurs FD partageant une zone d'enregistrement.

### Verbes prévus (spécification pour le jour de leur implémentation)
- `SORT f ON {ASCENDING|DESCENDING} KEY k … {USING f…|INPUT PROCEDURE p}
   {GIVING f…|OUTPUT PROCEDURE p}` ; `RELEASE`, `RETURN`.
- `MERGE f ON … KEY … USING f1 f2 … GIVING f`.
- Organisation `RELATIVE` : `READ/WRITE/REWRITE/DELETE/START` par `RELATIVE KEY`.

---

## Partie C — Banc d'équivalence entre formes

Pour un ensemble choisi des programmes ci-dessus, affirmer que la sortie observable est
**identique** (texte des DISPLAY, FILE STATUS, RETURN-CODE, contenu des fichiers) sur les
trois formes d'exécution d'une même source :

1. **Interpréteur** (`Interpreter::run`).
2. **Aller-retour de l'AST** — sérialiser (`bincode`+`flate2`) → désérialiser → exécuter ;
   affirmer que l'AST est identique octet pour octet et que la sortie est identique.
3. **Binaire empaqueté/compilé** — `cobolt_compiler::build_project` → exécuter le binaire
   produit ; affirmer que la sortie est identique.

Toute divergence entre les formes est un défaut à consigner (l'invariant
« un compilateur, un comportement »).

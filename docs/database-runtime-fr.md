<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Runtime de bases de données RustCOBOL

Les programmes RustCOBOL dialoguent avec les bases de données SQL par un petit
ensemble de `CALL` intégrés. Les six mêmes verbes fonctionnent avec **trois
moteurs** — le moteur est choisi automatiquement d'après la chaîne de connexion,
si bien qu'un programme écrit pour SQLite s'exécute sans modification contre
PostgreSQL ou MySQL en changeant un seul littéral.

| Moteur | Pilote (Rust pur, sans bibliothèque système) | Chaîne de connexion |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite` (SQLite embarqué)          | `:memory:`, `sqlite:<chemin>`, ou un simple chemin de fichier |
| **PostgreSQL** | `postgres` (rust-postgres, synchrone) | `postgres://utilisateur:motdepasse@hôte:port/bd` |
| **MySQL**   | `mysql` (rustls, synchrone)           | `mysql://utilisateur:motdepasse@hôte:port/bd`      |

Les trois pilotes sont liés statiquement et n'exigent **aucune bibliothèque
cliente externe** (`libpq`, `libmysqlclient`) **ni OpenSSL** pour compiler — en
cohérence avec le reste de PowerRustCOBOL.

---

## 1. Chaînes de connexion

Le moteur est choisi uniquement d'après le schéma de la chaîne de connexion :

| Forme | Moteur | Remarques |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | Base en RAM, abandonnée à la fermeture. |
| `sqlite:/var/data/app.db`                  | SQLite        | Le fichier est créé s'il n'existe pas. |
| `/var/data/app.db`                         | SQLite        | Un simple chemin est traité comme SQLite. |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` est également accepté. |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

La comparaison est insensible à la casse sur le schéma et tolère les espaces
alentour. Tout ce qui n'est **pas** une URL `postgres(ql)://` ou `mysql://` est
traité comme une cible SQLite.

---

## 2. La surface CALL

Chaque CALL passe ses arguments `BY REFERENCE`. Les valeurs d'état et de
descripteur vivent dans des data items COBOL ordinaires, afin de pouvoir être
conservées et transmises d'un paragraphe à l'autre.

| Nom du CALL | Arguments (`BY REFERENCE`) |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | chaîne-connexion, var-descripteur `PIC 9(9)`, var-état  |
| `COBOL-EXEC-SQL`   | descripteur, requête, var-nb-lignes `PIC 9(9)`, var-état |
| `COBOL-FETCH-ROW`  | descripteur, indice-colonne `PIC 9(n)` (base 1), var-destination, état |
| `COBOL-NEXT-ROW`   | descripteur, var-indicateur-suite `PIC X` (`Y`/`N`)     |
| `COBOL-ROW-COUNT`  | descripteur, var-compteur `PIC 9(9)`                    |
| `COBOL-CLOSE-DB`   | descripteur                                             |

### Sémantique

- **`COBOL-OPEN-DB`** ouvre une connexion et écrit un descripteur entier positif
  dans *var-descripteur*. En cas de succès, *var-état* contient des espaces ; en
  cas d'échec, *var-descripteur* vaut `0` et *var-état* porte le message
  d'erreur du pilote.
- **`COBOL-EXEC-SQL`** exécute une instruction sur *descripteur*.
  - Pour les instructions qui renvoient des lignes (`SELECT`, CTE, …),
    l'ensemble du résultat est mis en cache et *var-nb-lignes* reçoit le
    **nombre de lignes**. Le curseur démarre sur la première ligne.
  - Pour `INSERT` / `UPDATE` / `DELETE` / DDL, *var-nb-lignes* reçoit le
    **nombre de lignes affectées** et le jeu de résultats est vide.
  - En cas d'erreur, *var-état* porte le message et *var-nb-lignes* vaut `0`.
- **`COBOL-FETCH-ROW`** copie la colonne *indice-colonne* (base 1) de la ligne
  **courante** dans *var-destination*, sous forme de texte. Les colonnes hors
  plage et un curseur épuisé donnent des espaces.
- **`COBOL-NEXT-ROW`** avance le curseur et met *var-indicateur-suite* à `Y` si
  une ligne est désormais disponible, ou à `N` une fois le jeu épuisé.
- **`COBOL-ROW-COUNT`** renvoie le nombre de lignes mis en cache pour la
  dernière requête.
- **`COBOL-CLOSE-DB`** ferme la connexion et libère son jeu de résultats. Les
  descripteurs inconnus sont ignorés. Toutes les connexions ouvertes sont
  fermées à la fin du programme.

### Normalisation des valeurs

Toute valeur de colonne — quel que soit le moteur ou le type SQL — est livrée à
COBOL sous forme de **texte**, de sorte qu'elle peut faire l'objet d'un `MOVE`
direct vers un champ `PIC X` (ou vers un champ numérique, qui réinterprète les
chiffres). La normalisation est uniforme :

| Valeur SQL | Texte livré à COBOL |
|----------------|----------------------------------------|
| `NULL`         | espaces (chaîne vide)                  |
| entier         | chiffres décimaux, par ex. `42`, `-7`  |
| réel / double  | la forme aller-retour la plus courte, par ex. `3.14` |
| text / varchar | la chaîne UTF-8                        |
| date           | `YYYY-MM-DD`                           |
| datetime       | `YYYY-MM-DD HH:MM:SS`                  |
| time (MySQL)   | `HH:MM:SS`                             |
| blob (SQLite)  | marqueur `<blob N bytes>`              |

---

## 3. Exemple — un CRUD portable

Ce programme s'exécute contre **n'importe lequel** des trois moteurs ; seul
`WS-CONN` change. C'est exactement le programme exercé par la suite de tests
(`crates/cobolt-runtime/tests/test_sql.rs`).

```cobol
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQL-CRUD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CONN     PIC X(64)  VALUE ":memory:".
      *>  PostgreSQL: VALUE "postgres://scott:tiger@localhost:5432/store".
      *>  MySQL:      VALUE "mysql://scott:tiger@localhost:3306/store".
       01 WS-HANDLE   PIC 9(9)   VALUE 0.
       01 WS-STATUS   PIC X(128) VALUE SPACES.
       01 WS-QUERY    PIC X(256) VALUE SPACES.
       01 WS-ROWCNT   PIC 9(9)   VALUE 0.
       01 WS-COL      PIC 9(4)   VALUE 1.
       01 WS-NAME     PIC X(16)  VALUE SPACES.
       01 WS-MORE     PIC X      VALUE "N".
       PROCEDURE DIVISION.
       MAIN.
           CALL "COBOL-OPEN-DB" USING WS-CONN WS-HANDLE WS-STATUS
           IF WS-STATUS NOT = SPACES
               DISPLAY "OPEN FAILED: " WS-STATUS
               STOP RUN
           END-IF

           MOVE "CREATE TABLE c (id INTEGER, name TEXT)" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS

           MOVE "INSERT INTO c VALUES (1,'ANA'),(2,'BRUNO'),(3,'CARLOS')"
               TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "INSERTED " WS-ROWCNT

           MOVE "SELECT name FROM c ORDER BY id" TO WS-QUERY
           CALL "COBOL-EXEC-SQL"
               USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           DISPLAY "ROWS " WS-ROWCNT

           MOVE "Y" TO WS-MORE
           PERFORM UNTIL WS-MORE = "N"
               MOVE 1 TO WS-COL
               CALL "COBOL-FETCH-ROW"
                   USING WS-HANDLE WS-COL WS-NAME WS-STATUS
               DISPLAY "NAME " WS-NAME
               CALL "COBOL-NEXT-ROW" USING WS-HANDLE WS-MORE
           END-PERFORM

           CALL "COBOL-CLOSE-DB" USING WS-HANDLE
           STOP RUN.
```

Sortie (SQLite en mémoire) :

```
INSERTED 000000003
ROWS 000000003
NAME ANA
NAME BRUNO
NAME CARLOS
```

### Lire plusieurs colonnes

`COBOL-FETCH-ROW` lit une colonne par appel ; changez `WS-COL` pour en lire
d'autres sur la même ligne avant d'avancer :

```cobol
           MOVE 1 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-ID  WS-STATUS
           MOVE 2 TO WS-COL
           CALL "COBOL-FETCH-ROW" USING WS-HANDLE WS-COL WS-NAME WS-STATUS
           CALL "COBOL-NEXT-ROW"  USING WS-HANDLE WS-MORE
```

---

## 4. Transactions

Les transactions se pilotent en SQL ordinaire via `COBOL-EXEC-SQL` : le
comportement est donc exactement celui de votre serveur.

```cobol
           MOVE "BEGIN"  TO WS-QUERY
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
           *>  … several INSERT/UPDATE/DELETE statements …
           MOVE "COMMIT" TO WS-QUERY      *>  or "ROLLBACK"
           CALL "COBOL-EXEC-SQL" USING WS-HANDLE WS-QUERY WS-ROWCNT WS-STATUS
```

> Les **verbes** COBOL `COMMIT` / `ROLLBACK` sont une fonctionnalité distincte,
> qui pilote les transactions de **fichiers INDEXED** de RustCOBOL (voir
> [`docs/indexed-file-format.md`](indexed-file-format.md)). Ils n'agissent
> **pas** sur les connexions SQL — pour la base de données, utilisez
> `COBOL-EXEC-SQL` avec `BEGIN`/`COMMIT`/`ROLLBACK`, comme ci-dessus.

PostgreSQL et MySQL sont en autocommit par défaut : une instruction isolée est
donc validée immédiatement. Enveloppez une unité de travail dans
`BEGIN … COMMIT` pour la rendre atomique.

---

## 5. Le contrôle de données de l'IDE

Dans le form designer de PowerRustCOBOL, un contrôle **SqlDatabase** génère
automatiquement les paragraphes d'infrastructure (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Deux properties comptent :

- **`ConnectionString`** — n'importe laquelle des chaînes de connexion
  ci-dessus. C'est elle qui sélectionne réellement le moteur à l'exécution.
- **`Driver`** — `sqlite` (par défaut), `postgres` ou `mysql`. Purement
  cosmétique : elle étiquette les commentaires générés ; l'aiguillage se fait
  par la chaîne de connexion.

---

## 6. Notes de sécurité et d'exploitation

- **TLS.** Le pilote MySQL est construit avec rustls et négocie TLS lorsque le
  serveur le demande. Le pilote PostgreSQL synchrone se connecte **sans TLS**
  (`NoTls`) — adapté aux sockets locaux et aux réseaux de confiance. Pour un
  serveur PostgreSQL qui exige TLS, terminez le TLS sur un proxy local (par
  exemple `stunnel`/`pgbouncer`) ou passez par un tunnel SSH.
- **Injection SQL.** Les instructions sont envoyées sous forme de texte.
  Construisez vos requêtes à partir d'entrées de confiance, ou validez/échappez
  au préalable toute valeur fournie par l'utilisateur avant de composer la
  chaîne SQL.
- **Durée de vie des connexions.** Chaque descripteur possède une connexion
  vivante. Fermez avec `COBOL-CLOSE-DB` les descripteurs dont vous n'avez plus
  besoin ; tout ce qui reste ouvert est fermé à la fin du programme.

---

## 7. Tests

- **Hors ligne (toujours exécutés) :** l'aiguillage de la chaîne de connexion,
  la normalisation des valeurs et un aller-retour CRUD complet sur SQLite en
  mémoire — `cargo test -p cobolt-runtime --lib db_runtime` et
  `cargo test -p cobolt-runtime --test test_sql`.
- **Serveurs réels (sur demande) :** deux tests aller-retour marqués `#[ignore]`
  se connectent à de vrais serveurs. Fournissez une URL et lancez-les
  explicitement :

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implémentation

`crates/cobolt-runtime/src/db_runtime.rs` contient le moteur. Un `DbConn`
enveloppe une énumération `Backend` (`Sqlite` / `Postgres` / `MySql`) ;
`BackendKind::classify` choisit le moteur d'après la chaîne de connexion. Chaque
moteur a son propre chemin `exec_*` qui normalise les lignes en
`Vec<Vec<String>>`, après quoi la logique de curseur partagée (`fetch_col` /
`next_row` / `row_count`) est indépendante du moteur. Le `exec_call` de
l'interpréteur (`crates/cobolt-runtime/src/interpreter.rs`) fait correspondre
les six CALL COBOL à `DbRegistry`, qui mutualise les connexions par descripteur
entier.

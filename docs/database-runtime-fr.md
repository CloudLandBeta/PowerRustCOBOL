<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Runtime de bases de données RustCOBOL

Les programmes RustCOBOL dialoguent avec les bases SQL au travers d'un petit jeu
de `CALL` intégrés. Les mêmes six verbes fonctionnent contre **trois backends** :
le moteur est choisi automatiquement d'après la chaîne de connexion, si bien
qu'un programme écrit pour SQLite tourne sans modification contre PostgreSQL ou
MySQL en changeant un seul littéral.

| Backend     | Pilote (aucune bibliothèque système requise)     | Chaîne de connexion                                |
|-------------|---------------------------------------|----------------------------------------------------|
| **SQLite**  | `rusqlite`, `features = ["bundled"]` — compile l'amalgame **C** de SQLite ; celui-ci n'est donc pas du Rust pur | `:memory:`, `sqlite:<chemin>`, ou un simple chemin de fichier |
| **PostgreSQL** | `postgres` (rust-postgres, synchrone) | `postgres://user:pass@host:port/db`             |
| **MySQL**   | `mysql` (`minimal-rust`, synchrone, sans TLS) | `mysql://user:pass@host:port/db`           |

Les trois pilotes sont liés statiquement et n'exigent **aucune bibliothèque
cliente externe** (`libpq`, `libmysqlclient`) ni **OpenSSL** pour compiler — en
cohérence avec le reste de PowerRustCOBOL.

---

## 1. Chaînes de connexion

Le backend est choisi uniquement d'après le schéma de la chaîne de connexion :

| Forme                                      | Backend       | Notes                                  |
|--------------------------------------------|---------------|----------------------------------------|
| `:memory:`                                 | SQLite        | Base en mémoire vive, jetée à la fermeture. |
| `sqlite:/var/data/app.db`                  | SQLite        | Le fichier est créé s'il n'existe pas. |
| `/var/data/app.db`                         | SQLite        | Un simple chemin est traité comme SQLite. |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` est également accepté. |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                        |

La comparaison du schéma ignore la casse et tolère les espaces alentour. Tout ce
qui n'est **pas** une URL `postgres(ql)://` ou `mysql://` est traité comme une
cible SQLite.

---

## 2. La surface d'appel CALL

Chaque CALL passe ses arguments `BY REFERENCE`. Les valeurs d'état et de
descripteur vivent dans des éléments de données COBOL ordinaires, afin de pouvoir
être conservées et transmises d'un paragraphe à l'autre.

| Nom du CALL        | Arguments (`BY REFERENCE`)                              |
|--------------------|---------------------------------------------------------|
| `COBOL-OPEN-DB`    | chaîne de connexion, variable de descripteur `PIC 9(9)`, variable d'état |
| `COBOL-EXEC-SQL`   | descripteur, requête, variable de nombre de lignes `PIC 9(9)`, variable d'état |
| `COBOL-FETCH-ROW`  | descripteur, indice de colonne `PIC 9(n)` (à partir de 1), variable de destination, état |
| `COBOL-NEXT-ROW`   | descripteur, variable d'indicateur de suite `PIC X` (`Y`/`N`) |
| `COBOL-ROW-COUNT`  | descripteur, variable de comptage `PIC 9(9)`            |
| `COBOL-CLOSE-DB`   | descripteur                                             |

### Sémantique

- **`COBOL-OPEN-DB`** ouvre une connexion et écrit un descripteur entier positif
  dans *handle-var*. En cas de succès, *status-var* reçoit des espaces ; en cas
  d'échec, *handle-var* vaut `0` et *status-var* contient le message d'erreur du
  pilote.
- **`COBOL-EXEC-SQL`** exécute une instruction sur *handle*.
  - Pour les instructions qui renvoient des lignes (`SELECT`, CTE, …), tout le
    jeu de résultats est mis en cache et *row-count-var* reçoit le **nombre de
    lignes**. Le curseur démarre sur la première ligne.
  - Pour `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* reçoit le **nombre
    de lignes affectées** et le jeu de résultats est vide.
  - En cas d'erreur, *status-var* contient le message et *row-count-var* vaut
    `0`.
- **`COBOL-FETCH-ROW`** copie la colonne *col-index* (à partir de 1) de la ligne
  **courante** dans *dest-var* sous forme de texte. Une colonne hors limites et
  un curseur épuisé donnent des espaces.
- **`COBOL-NEXT-ROW`** avance le curseur et met *more-flag-var* à `Y` si une
  ligne est désormais disponible, ou à `N` une fois le jeu épuisé.
- **`COBOL-ROW-COUNT`** renvoie le nombre de lignes mis en cache par la dernière
  requête.
- **`COBOL-CLOSE-DB`** ferme la connexion et libère son jeu de résultats. Les
  descripteurs inconnus sont ignorés. Toutes les connexions encore ouvertes sont
  fermées à la fin du programme.

### Normalisation des valeurs

Toute valeur de colonne — quel que soit le backend ou le type SQL — est remise à
COBOL sous forme de **texte**, afin de pouvoir être `MOVE`d directement dans un
champ `PIC X` (ou dans un champ numérique, qui réinterprète les chiffres). La
normalisation est uniforme :

| Valeur SQL     | Texte remis à COBOL                    |
|----------------|----------------------------------------|
| `NULL`         | espaces (chaîne vide)                  |
| entier         | chiffres décimaux, p. ex. `42`, `-7`   |
| réel / double  | la forme aller-retour la plus courte, p. ex. `3.14` |
| texte / varchar| la chaîne UTF-8                        |
| date           | `YYYY-MM-DD`                           |
| date-heure     | `YYYY-MM-DD HH:MM:SS`                  |
| heure (MySQL)  | `HH:MM:SS`                             |
| blob (SQLite)  | marqueur `<blob N bytes>`              |

---

## 3. Exemple — un CRUD portable

Ce programme tourne contre **n'importe lequel** des trois backends ; seul
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
d'autres de la même ligne avant d'avancer :

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

> Les **verbes** COBOL `COMMIT` / `ROLLBACK` sont une fonctionnalité distincte
> qui pilote les transactions sur les **fichiers INDEXED** de RustCOBOL (voir
> [`docs/indexed-file-format-fr.md`](indexed-file-format-fr.md)). Ils n'agissent
> **pas** sur les connexions SQL — pour la base, utilisez `COBOL-EXEC-SQL` avec
> `BEGIN`/`COMMIT`/`ROLLBACK`, comme ci-dessus.

PostgreSQL et MySQL sont en validation automatique par défaut : une instruction
isolée est donc validée immédiatement. Enveloppez une unité de travail dans
`BEGIN … COMMIT` pour la rendre atomique.

---

## 5. Le contrôle de données de l'IDE

Dans le concepteur de formulaires de PowerRustCOBOL, un contrôle **SqlDatabase**
génère automatiquement les paragraphes répétitifs (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Deux propriétés comptent :

- **`ConnectionString`** — n'importe laquelle des chaînes de connexion ci-dessus.
  C'est elle qui choisit réellement le backend à l'exécution.
- **`Driver`** — `sqlite` (par défaut), `postgres` ou `mysql`. Purement
  cosmétique : elle étiquette les commentaires générés ; l'aiguillage se fait par
  la chaîne de connexion.

---

## 6. Notes de sécurité et d'exploitation

- **TLS.** ⚠️ **Aucun des deux pilotes SQL ne parle TLS aujourd'hui.** Le pilote
  MySQL est compilé avec
  `default-features = false, features = ["minimal-rust"]`, et le `mysql 28`
  résolu n'entraîne aucune crate TLS — il ne peut pas négocier de connexion
  sécurisée, quoi que demande le serveur. Le pilote PostgreSQL synchrone se
  connecte en `NoTls` par construction. Les deux conviennent aux sockets locaux
  et aux réseaux de confiance. Pour un serveur qui exige TLS, terminez-le sur un
  mandataire local (`stunnel`/`pgbouncer` par exemple) ou passez par un tunnel
  SSH.
- **Injection SQL.** Les instructions sont envoyées sous forme de texte.
  Construisez les requêtes à partir d'entrées de confiance, ou validez et
  échappez au préalable toute valeur fournie par l'utilisateur avant de composer
  la chaîne SQL.
- **Durée de vie des connexions.** Chaque descripteur possède une connexion
  vivante. Fermez avec `COBOL-CLOSE-DB` ceux dont vous n'avez plus besoin ; tout
  ce qui reste ouvert est fermé à la fin du programme.

---

## 7. Tests

- **Hors ligne (toujours exécutés) :** l'aiguillage par chaîne de connexion, la
  normalisation des valeurs et un aller-retour CRUD complet sur SQLite en mémoire
  — `cargo test -p cobolt-runtime --lib db_runtime` et
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

`crates/cobolt-runtime/src/db_runtime.rs` contient le moteur. Un `DbConn` enveloppe
une énumération `Backend` (`Sqlite` / `Postgres` / `MySql`) ;
`BackendKind::classify` choisit le backend d'après la chaîne de connexion. Chaque
backend a son propre chemin `exec_*` qui normalise les lignes en
`Vec<Vec<String>>`, après quoi la logique de curseur partagée (`fetch_col` /
`next_row` / `row_count`) est indépendante du backend. Le `exec_call` de
l'interpréteur (`crates/cobolt-runtime/src/interpreter.rs`) fait correspondre les
six CALL COBOL à `DbRegistry`, qui met en commun les connexions par descripteur
entier.

.<<

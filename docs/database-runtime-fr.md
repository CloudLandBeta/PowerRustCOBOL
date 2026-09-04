<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Runtime de bases de données RustCOBOL

Les programmes RustCOBOL dialoguent avec les bases de données SQL au moyen d'un
petit ensemble de `CALL`s intégrés. Les mêmes six verbes fonctionnent avec
**trois moteurs** — le backend est choisi automatiquement d'après la chaîne de
connexion, si bien qu'un programme écrit pour SQLite s'exécute tel quel avec
PostgreSQL ou MySQL en changeant un seul littéral.

| Backend     | Pilote (Rust pur, sans bibliothèque système) | Chaîne de connexion                                     |
|-------------|----------------------------------------------|----------------------------------------------------------|
| **SQLite**  | `rusqlite` (SQLite embarqué)                 | `:memory:`, `sqlite:<path>`, ou un simple chemin de fichier |
| **PostgreSQL** | `postgres` (rust-postgres, synchrone)     | `postgres://user:pass@host:port/db`                      |
| **MySQL**   | `mysql` (rustls, synchrone)                  | `mysql://user:pass@host:port/db`                         |

Les trois pilotes sont liés statiquement et ne réclament **aucune bibliothèque
cliente externe** (`libpq`, `libmysqlclient`) **ni OpenSSL** pour compiler — dans
la ligne du reste de PowerRustCOBOL.

---

## 1. Chaînes de connexion

Le backend est déterminé uniquement par le schéma de la chaîne de connexion :

| Forme                                      | Backend       | Remarques                                   |
|--------------------------------------------|---------------|---------------------------------------------|
| `:memory:`                                 | SQLite        | Base en RAM, abandonnée à la fermeture.     |
| `sqlite:/var/data/app.db`                  | SQLite        | Le fichier est créé s'il n'existe pas.      |
| `/var/data/app.db`                         | SQLite        | Un simple chemin est traité comme SQLite.   |
| `postgres://scott:tiger@localhost:5432/store`    | PostgreSQL | `postgresql://` est également accepté.   |
| `mysql://scott:tiger@localhost:3306/store` | MySQL         |                                             |

La comparaison du schéma ne tient pas compte de la casse et tolère les espaces
alentour. Tout ce qui n'est **pas** une URL `postgres(ql)://` ou `mysql://` est
traité comme une cible SQLite.

---

## 2. La surface CALL

Chaque CALL passe ses arguments `BY REFERENCE`. Les valeurs de statut et de
descripteur résident dans des éléments de données COBOL ordinaires, afin de
pouvoir être conservées et transmises d'un paragraphe à l'autre.

| Nom du CALL        | Arguments (`BY REFERENCE`)                                |
|--------------------|-----------------------------------------------------------|
| `COBOL-OPEN-DB`    | conn-string, handle-var `PIC 9(9)`, status-var            |
| `COBOL-EXEC-SQL`   | handle, query, row-count-var `PIC 9(9)`, status-var       |
| `COBOL-FETCH-ROW`  | handle, col-index `PIC 9(n)` (base 1), dest-var, status   |
| `COBOL-NEXT-ROW`   | handle, more-flag-var `PIC X` (`Y`/`N`)                   |
| `COBOL-ROW-COUNT`  | handle, count-var `PIC 9(9)`                              |
| `COBOL-CLOSE-DB`   | handle                                                    |

### Sémantique

- **`COBOL-OPEN-DB`** ouvre une connexion et écrit un descripteur entier positif
  dans *handle-var*. En cas de succès, *status-var* est mis à blanc ; en cas
  d'échec, *handle-var* vaut `0` et *status-var* contient le message d'erreur du
  pilote.
- **`COBOL-EXEC-SQL`** exécute une instruction sur *handle*.
  - Pour les instructions qui renvoient des lignes (`SELECT`, CTE, …) le jeu de
    résultats complet est mis en cache et *row-count-var* reçoit le **nombre de
    lignes**. Le curseur démarre sur la première ligne.
  - Pour `INSERT` / `UPDATE` / `DELETE` / DDL, *row-count-var* reçoit le **nombre
    de lignes affectées** et le jeu de résultats est vide.
  - En cas d'erreur, *status-var* contient le message et *row-count-var* vaut `0`.
- **`COBOL-FETCH-ROW`** copie la colonne *col-index* (base 1) de la ligne
  **courante** dans *dest-var* sous forme de texte. Une colonne hors limites ou
  un curseur épuisé donnent des blancs.
- **`COBOL-NEXT-ROW`** avance le curseur et met *more-flag-var* à `Y` si une
  ligne est désormais disponible, ou à `N` une fois le jeu épuisé.
- **`COBOL-ROW-COUNT`** renvoie le nombre de lignes mis en cache pour la dernière
  requête.
- **`COBOL-CLOSE-DB`** ferme la connexion et libère son jeu de résultats. Les
  descripteurs inconnus sont ignorés. Toutes les connexions encore ouvertes sont
  fermées à la fin du programme.

### Normalisation des valeurs

Chaque valeur de colonne — quel que soit le backend ou le type SQL — est livrée à
COBOL sous forme de **texte**, de sorte qu'un `MOVE` la place directement dans un
champ `PIC X` (ou dans un champ numérique, qui en réinterprète les chiffres). La
normalisation est uniforme :

| Valeur SQL     | Texte livré à COBOL                              |
|----------------|--------------------------------------------------|
| `NULL`         | blancs (chaîne vide)                             |
| integer        | chiffres décimaux, p. ex. `42`, `-7`             |
| real / double  | la forme d'aller-retour la plus courte, p. ex. `3.14` |
| text / varchar | la chaîne UTF-8                                  |
| date           | `YYYY-MM-DD`                                     |
| datetime       | `YYYY-MM-DD HH:MM:SS`                            |
| time (MySQL)   | `HH:MM:SS`                                       |
| blob (SQLite)  | l'indicateur `<blob N bytes>`                    |

---

## 3. Exemple — un CRUD portable

Ce programme s'exécute avec **n'importe lequel** des trois backends ; seul
`WS-CONN` change. C'est exactement le programme éprouvé par la suite de tests
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

`COBOL-FETCH-ROW` lit une colonne par appel ; modifiez `WS-COL` pour en lire
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

> Les **verbes** COBOL `COMMIT` / `ROLLBACK` constituent une fonctionnalité
> distincte, qui pilote les transactions des **fichiers INDEXED** de RustCOBOL
> (voir [`docs/indexed-file-format-fr.md`](indexed-file-format-fr.md)). Ils
> n'agissent **pas** sur les connexions SQL — pour la base de données, employez
> `COBOL-EXEC-SQL` avec `BEGIN`/`COMMIT`/`ROLLBACK`, comme ci-dessus.

PostgreSQL et MySQL sont en autocommit par défaut : une instruction isolée est
donc validée immédiatement. Encadrez une unité de travail par `BEGIN … COMMIT`
pour la rendre atomique.

---

## 5. Le contrôle de données de l'IDE

Dans le concepteur de formulaires de PowerRustCOBOL, un contrôle **SqlDatabase**
génère automatiquement les paragraphes répétitifs (`<id>-CONNECT`, `<id>-EXEC`,
`<id>-FETCH-ALL`, `<id>-CLOSE`). Deux propriétés comptent :

- **`ConnectionString`** — n'importe laquelle des chaînes de connexion
  ci-dessus. C'est elle qui sélectionne réellement le backend à l'exécution.
- **`Driver`** — `sqlite` (par défaut), `postgres` ou `mysql`. Purement
  cosmétique : il libelle les commentaires générés ; l'aiguillage, lui, dépend de
  la chaîne de connexion.

---

## 6. Notes de sécurité et d'exploitation

- **TLS.** Le pilote MySQL est construit avec rustls et négocie TLS lorsque le
  serveur le demande. Le pilote PostgreSQL synchrone se connecte **sans TLS**
  (`NoTls`) — ce qui convient aux sockets locales et aux réseaux de confiance.
  Pour un serveur PostgreSQL qui exige TLS, terminez le TLS sur un proxy local
  (par exemple `stunnel`/`pgbouncer`) ou passez par un tunnel SSH.
- **Injection SQL.** Les instructions sont envoyées sous forme de texte.
  Construisez les requêtes à partir d'entrées fiables, ou validez et échappez au
  préalable toute valeur fournie par l'utilisateur avant de composer la chaîne
  SQL.
- **Durée de vie des connexions.** Chaque descripteur possède une connexion
  vivante. Fermez avec `COBOL-CLOSE-DB` les descripteurs dont vous n'avez plus
  besoin ; tout ce qui reste ouvert est fermé à la fin du programme.

---

## 7. Tests

- **Hors ligne (toujours exécutés) :** aiguillage de la chaîne de connexion,
  normalisation des valeurs et un aller-retour CRUD complet sur SQLite en
  mémoire — `cargo test -p cobolt-runtime --lib db_runtime` et
  `cargo test -p cobolt-runtime --test test_sql`.
- **Serveurs réels (sur demande) :** deux tests d'aller-retour marqués
  `#[ignore]` se connectent à de vrais serveurs. Fournissez une URL et
  lancez-les explicitement :

  ```bash
  PRC_TEST_PG_URL="postgres://postgres:postgres@localhost:5432/postgres" \
      cargo test -p cobolt-runtime --lib -- --ignored pg_live

  PRC_TEST_MYSQL_URL="mysql://root:root@localhost:3306/test" \
      cargo test -p cobolt-runtime --lib -- --ignored mysql_live
  ```

---

## 8. Implémentation

`crates/cobolt-runtime/src/db_runtime.rs` abrite le moteur. Un `DbConn`
enveloppe une énumération `Backend` (`Sqlite` / `Postgres` / `MySql`) ;
`BackendKind::classify` choisit le backend d'après la chaîne de connexion. Chaque
backend possède son propre chemin `exec_*`, qui normalise les lignes en
`Vec<Vec<String>>` ; au-delà, la logique de curseur partagée (`fetch_col` /
`next_row` / `row_count`) est indépendante du backend. L'`exec_call` de
l'interpréteur (`crates/cobolt-runtime/src/interpreter.rs`) projette les six
CALLs COBOL sur `DbRegistry`, qui met les connexions en pool par descripteur
entier.

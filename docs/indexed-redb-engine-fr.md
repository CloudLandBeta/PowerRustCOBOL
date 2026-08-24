<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Moteur INDEXED résistant aux pannes (redb)

PowerRustCOBOL livre un second moteur `STORAGE IS DISK` pour les fichiers
`ORGANIZATION IS INDEXED`, bâti sur **redb** — un magasin clé-valeur ACID
embarqué et écrit en Rust pur (arbre B+ en copie sur écriture, doubles pages de
métadonnées, sommes de contrôle par page). Il présente un comportement COBOL
observable *identique* à celui du moteur par défaut `PRCIDXD1`, mais il est conçu
autour de quatre objectifs opérationnels que le moteur sur mesure ne pouvait pas
tenir à grande échelle.

Il est **optionnel** aujourd'hui (le moteur disque par défaut reste
`PRCIDXD1`) :

```bash
rcrun run program.cbl --indexed-engine redb
# or
COBOL_INDEXED_ENGINE=redb rcrun run program.cbl
```

Implémentation :
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Pourquoi — les quatre objectifs

| Objectif | Comment le moteur redb y répond |
|------|------------------------------|
| **OPEN est instantané, toujours** | redb ne lit que sa page de métadonnées à l'ouverture. Il n'y a **aucun répertoire d'enregistrements en RAM à charger, ni balayage de reprise**, même après une panne. Mesuré : environ 5 ms pour ouvrir un fichier de 200 000 enregistrements (indépendamment de leur nombre). |
| **READ RANDOM / NEXT à la vitesse de la lumière** | RANDOM est une descente dans l'arbre B+ ; NEXT est un itérateur séquentiel sur intervalle. Les deux s'exécutent au-dessus du cache de pages de redb. Mesuré : environ 21 µs par lecture aléatoire à 200 000 enregistrements. |
| **Jusqu'à 250 M d'enregistrements (données non bornées)** | La RAM résidente correspond à l'ensemble de travail (le cache de redb), et **non** au nombre d'enregistrements. Aucune structure en `O(enregistrements)` n'est conservée en mémoire. |
| **La sûreté prime** | redb est pleinement ACID. `COMMIT` est une validation de transaction durable (fsync) ; `ROLLBACK` est un abandon de transaction. Une coupure de courant ne peut jamais exposer un index déchiré — redb revient au dernier commit valide grâce à ses doubles pages de métadonnées. Aucune perte de données, aucune corruption d'index. |

À comparer au moteur `PRCIDXD1`, dont le répertoire de RecordId est chargé
intégralement en RAM à l'OPEN (≈16 octets × chaque RecordId jamais alloué) et
dont les transactions étaient un journal d'annulation en RAM, persisté seulement
au CLOSE — il ne pouvait donc ni ouvrir instantanément à grande échelle, ni
survivre à une coupure de courant en cours d'exécution.

---

## Disposition sur disque (tables redb)

| Table redb | Type     | clé → valeur                                   |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | octets de la clé primaire → enregistrement (éventuellement compressé) |
| `alt`      | multimap | `[u16 idx][alt-key bytes]` → `[u64 seq][primary key]` |
| `seq`      | table    | octets de la clé primaire → numéro d'insertion `u64`  |
| `meta`     | table    | descripteurs `schema`, `compress`, `nextseq`   |

- Un **unique multimap `alt`** contient toutes les clés alternatives, cloisonnées
  par un index de clé de 2 octets en gros-boutiste. L'ordre des octets est donc
  `(index de clé, valeur alternative, numéro d'insertion)` — ce qui fait que les
  alternatives dupliquées se parcourent dans l'**ordre de création**, exactement
  comme l'ordonnancement par RecordId du moteur disque et comme la règle COBOL
  sur les clés alternatives dupliquées.
- La mécanique `seq` / `meta:nextseq` n'existe **que** pour ordonner les
  doublons de clé alternative. Les fichiers sans clé alternative la contournent
  entièrement et ne paient qu'une insertion dans l'arbre B+ par `WRITE`.
- Les enregistrements sont stockés sous forme d'images positionnelles à largeur
  fixe (voir [`indexed-file-internals-fr.md`](indexed-file-internals-fr.md) §6) ;
  `WITH COMPRESSION` applique le même RLE PackBits que les autres moteurs.

---

## Modèle transactionnel

Une ouverture en écriture (`OUTPUT` / `I-O` / `EXTEND`) garde une
`WriteTransaction` redb ouverte depuis l'OPEN. Les lectures effectuées dans cette
transaction voient les écritures non encore validées du programme lui-même (le
« lire ses propres écritures » de COBOL). Les verbes COBOL se transposent
directement :

| COBOL | redb |
|-------|------|
| `OPEN`     | ouvre une transaction d'écriture (modes en écriture) |
| `COMMIT`   | `commit()` de la transaction (durable), puis en ouvre une neuve |
| `ROLLBACK` | `abort()` de la transaction (jette tout depuis le dernier `COMMIT`/`OPEN`), puis en ouvre une neuve |
| `CLOSE`    | `commit()` (validation implicite) |

Les ouvertures en `INPUT` utilisent de courtes transactions de lecture. Comme
`ROLLBACK` est un véritable abandon redb, **aucun journal d'annulation n'est
nécessaire** — la durabilité et le retour arrière sont les garanties propres du
magasin.

> Les verbes COBOL `COMMIT` / `ROLLBACK` agissent sur les **fichiers INDEXED**,
> pas sur les connexions SQL (celles-ci passent par `COBOL-EXEC-SQL` avec
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Parité de comportement

Le moteur est tenu au comportement exact du moteur par défaut : les mêmes
fixtures versionnées (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) s'exécutent sous `--indexed-engine redb` et doivent produire une
sortie DISPLAY identique — CRUD avec clé primaire plus alternative `WITH
DUPLICATES`, persistance à travers une réouverture, et `COMMIT`/`ROLLBACK`. Les
codes d'état de fichier (`00/02/10/22/23/35/39/46/47/48/49/90/...`), la
résolution de la clé de référence, la sémantique de `START` et la règle selon
laquelle « REWRITE/DELETE exigent un enregistrement courant » concordent toutes.

Tests : `crates/cobolt-runtime/tests/test_indexed_redb.rs` (les fixtures sous
redb + vérifications directes d'`IndexedStore` + un test de fumée à l'échelle
marqué `#[ignore]`).

---

## Limites

Le moteur étant paginé à la demande, les limites pratiques sont fixées par redb
et le système de fichiers, non par la RAM résidente :

| Dimension | Limite |
|-----------|-------|
| Taille du fichier | limite de redb / du système de fichiers (téraoctets) |
| Enregistrements | borné par la RAM de l'ensemble de travail, pas par leur nombre (≥250 M avec un petit cache) |
| Taille d'enregistrement | image à largeur fixe ; les gros enregistrements sont stockés comme valeurs redb |
| Taille de clé | octets de la clé composite (clés en plusieurs parties prises en charge par la couche COBOL) |
| Clés alternatives | jusqu'à 65 535 (espace d'index de 2 octets) |

---

## Notes de performance

- Le **`READ NEXT` séquentiel** par la clé primaire de référence renvoie
  l'enregistrement directement depuis le curseur d'intervalle — une descente dans
  l'arbre B+ par enregistrement, et non deux (environ 17 µs par enregistrement à
  200 000). Les balayages par clé alternative font toujours une descente dans
  l'alternative plus une lecture dans la primaire.
- Le **`WRITE`** ouvre les tables `primary`/`alt` une fois par opération (le
  contrôle de doublon et l'insertion partagent le handle). Un micro-benchmark a
  montré que mettre le handle en cache *entre* les appels n'apporte qu'environ
  8 % de mieux qu'une ouverture par opération : le moteur conserve donc le chemin
  simple et sans `unsafe`. Le coût d'écriture (environ 44 µs par enregistrement)
  est dominé par l'insertion ACID dans l'arbre B+ de redb, qui constitue le
  plancher sûr — aucune des optimisations d'écriture ne change les points de
  validation ni la durabilité.
- Le **`WRITE` en masse** tourne donc autour de 20 000 enregistrements/s dans une
  transaction unique (un coût de chargement payé une seule fois). L'OPEN, les
  lectures et la résistance aux pannes n'en sont pas affectés.

---

## Journal d'observabilité (`--indexed-log`)

Le moteur redb peut écrire un journal de transactions facultatif, par fichier
(désactivé par défaut), dans **`<assign-path>.log`** (par exemple
`customers.idx` → `customers.idx.log`), avec une ligne par
`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE` consignant l'horodatage, les décomptes
d'enregistrements et d'octets, le débit, la qualité de l'ordre des clés en
écriture et — au niveau `full` — les statistiques de pages d'index redb.

```bash
rcrun run app.cbl --indexed-engine redb --indexed-log full --indexed-log-format json
```

Le format de ligne est `text` (logfmt) ou `json` (NDJSON, prêt pour
Grafana/Loki).

**La référence complète** — options, tableau des champs, formats, chaîne
Grafana/Loki (Promtail + LogQL) et notes de coût et de sûreté — se trouve dans
[`observability-fr.md`](observability-fr.md) §1.

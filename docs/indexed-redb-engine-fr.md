<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Moteur INDEXED résistant aux pannes (redb)

PowerRustCOBOL livre un second moteur `STORAGE IS DISK` pour les fichiers
`ORGANIZATION IS INDEXED`, bâti sur **redb** — un magasin clé-valeur ACID
embarqué et écrit intégralement en Rust (B+tree en copie sur écriture, pages
méta en double, sommes de contrôle par page). Il présente un comportement COBOL
observable *identique* à celui de l'ancien moteur `PRCIDXD1`, mais il est conçu
autour de quatre objectifs opérationnels que le moteur maison ne pouvait pas
tenir à grande échelle.

**C'est le moteur par défaut, et il l'est depuis la 1.62.73** (décision de
l'opérateur, 2026-08-29). `IndexedEngine` dérive `Default` avec `#[default]` sur
`Redb` (`crates/cobolt-runtime/src/indexed.rs:126`), et un test l'y maintient
(`indexed.rs:1643`). Rien n'a besoin d'être sélectionné pour l'obtenir.

L'ancien moteur paginé reste disponible par son nom, tout comme les deux alias
qui délèguent au conteneur Rust intégré :

```bash
rcrun run program.cbl --indexed-engine rust    # le moteur paginé PRCIDXD1
# ou
COBOL_INDEXED_ENGINE=rust rcrun run program.cbl
```

Implémentation :
[`crates/cobolt-runtime/src/indexed_redb.rs`](../crates/cobolt-runtime/src/indexed_redb.rs).

---

## Pourquoi — les quatre objectifs

| Objectif | Comment le moteur redb le tient |
|------|------------------------------|
| **OPEN est instantané, toujours** | redb ne lit que sa page méta à l'ouverture. Il n'y a **aucun répertoire d'enregistrements en RAM à charger et aucun parcours de récupération**, même après un plantage. Mesuré : ~5 ms pour un OPEN sur un fichier de 200 000 enregistrements (indépendamment du nombre d'enregistrements). |
| **READ RANDOM / NEXT à la vitesse de la lumière** | RANDOM est une descente de B+tree ; NEXT est un itérateur de plage séquentiel. Les deux passent par le cache de pages de redb. Mesuré : ~21 µs par lecture aléatoire à 200 000 enregistrements. |
| **Jusqu'à 250 M d'enregistrements (données sans limite)** | La RAM résidente correspond à l'ensemble de travail (le cache de redb), **et non** au nombre d'enregistrements. Aucune structure en `O(enregistrements)` n'est gardée en mémoire. |
| **La sûreté prime sur tout** | redb est pleinement ACID. `COMMIT` est une validation de transaction durable (fsync) ; `ROLLBACK` est un abandon de transaction. Une coupure de courant ne peut jamais laisser apparaître un index déchiré : redb revient au dernier commit valide grâce à ses pages méta en double. Aucune perte de données, aucune corruption d'index. |

À comparer avec le moteur `PRCIDXD1`, dont le répertoire de RecordId est chargé
entièrement en RAM à l'OPEN (≈16 octets × chaque RecordId jamais alloué) et dont
les transactions étaient un journal d'annulation en RAM persisté seulement au
CLOSE — de sorte qu'il ne pouvait ni ouvrir instantanément à grande échelle ni
survivre à une coupure de courant en cours d'exécution.

---

## Disposition sur disque (tables redb)

| Table redb | Type     | clé → valeur                                  |
|------------|----------|-----------------------------------------------|
| `primary`  | table    | octets de la clé primaire → enregistrement (éventuellement compressé) |
| `alt`      | multimap | `[u16 idx][octets de la clé alternative]` → `[u64 seq][clé primaire]` |
| `seq`      | table    | octets de la clé primaire → séquence d'insertion `u64` |
| `meta`     | table    | descripteurs `schema`, `compress`, `nextseq`  |

- Un **unique multimap `alt`** contient toutes les clés alternatives, cloisonnées
  par un index de clé de 2 octets en big-endian. L'ordre des octets est donc
  `(index de clé, valeur alternative, séquence d'insertion)` — ce qui fait
  parcourir les alternatives en double dans leur **ordre de création**, exactement
  comme l'ordre des RecordId du moteur disque et comme la règle COBOL pour les
  clés alternatives en double.
- La mécanique `seq` / `meta:nextseq` n'existe **que** pour ordonner les doublons
  de clés alternatives. Les fichiers sans clé alternative l'ignorent entièrement
  et ne paient qu'une insertion de B+tree par `WRITE`.
- Les enregistrements sont stockés comme des images positionnelles à largeur fixe
  (voir [`indexed-file-internals-fr.md`](indexed-file-internals-fr.md) §6) ;
  `WITH COMPRESSION` applique le même RLE PackBits que les autres moteurs.

---

## Modèle transactionnel

Une ouverture en écriture (`OUTPUT` / `I-O` / `EXTEND`) garde une
`WriteTransaction` redb ouverte depuis l'OPEN. Les lectures faites au travers de
cette transaction voient les écritures non validées du programme lui-même (le
« lire ses propres écritures » de COBOL). Les verbes COBOL se traduisent
directement :

| COBOL | redb |
|-------|------|
| `OPEN`     | ouvre une transaction d'écriture (modes en écriture) |
| `COMMIT`   | `commit()` de la transaction (durable), puis en ouvre une nouvelle |
| `ROLLBACK` | `abort()` de la transaction (abandonne tout depuis le dernier `COMMIT`/`OPEN`), puis en ouvre une nouvelle |
| `CLOSE`    | `commit()` (validation implicite) |

Les ouvertures `INPUT` utilisent de courtes transactions de lecture. Comme
`ROLLBACK` est un véritable abandon redb, **aucun journal d'annulation n'est
nécessaire** — la durabilité et le retour arrière sont les garanties propres du
magasin.

> Les verbes COBOL `COMMIT` / `ROLLBACK` agissent sur les **fichiers INDEXED**,
> pas sur les connexions SQL (celles-ci passent par `COBOL-EXEC-SQL` avec
> `BEGIN`/`COMMIT`/`ROLLBACK`).

---

## Parité de comportement

Le moteur est tenu au comportement exact du moteur par défaut : les mêmes jeux
d'essai versionnés (`tests/cobol/fileio/idx_crud.cbl`, `idx_persist.cbl`,
`idx_tx.cbl`) s'exécutent sous `--indexed-engine redb` et doivent produire une
sortie DISPLAY identique — CRUD avec clé primaire et alternative
`WITH DUPLICATES`, persistance à travers une réouverture, et
`COMMIT`/`ROLLBACK`. Les codes d'état de fichier
(`00/02/10/22/23/35/39/46/47/48/49/90/...`), la résolution de la clé de
référence, la sémantique de `START` et la règle « REWRITE/DELETE exigent un
enregistrement courant » concordent toutes.

Tests : `crates/cobolt-runtime/tests/test_indexed_redb.rs` (les jeux d'essai sous
redb + des vérifications directes de `IndexedStore` + un test de fumée à grande
échelle marqué `#[ignore]`).

---

## Limites

Comme le moteur est paginé à la demande, les limites pratiques sont fixées par
redb et le système de fichiers, non par la RAM résidente :

| Dimension | Limite |
|-----------|-------|
| Taille de fichier | limite de redb / du système de fichiers (téraoctets) |
| Enregistrements | borné par la RAM de l'ensemble de travail, pas par le nombre d'enregistrements (≥250 M avec un petit cache) |
| Taille d'enregistrement | image à largeur fixe ; les grands enregistrements sont stockés comme valeurs redb |
| Taille de clé | octets de la clé composite (les clés en plusieurs parties sont prises en charge par la couche COBOL) |
| Clés alternatives | jusqu'à 65 535 (espace d'index de 2 octets) |

---

## Notes de performance

- Le **`READ NEXT` séquentiel** par la clé primaire de référence renvoie
  l'enregistrement directement depuis le curseur de plage — une descente de
  B+tree par enregistrement, pas deux (~17 µs/enregistrement à 200 000). Les
  parcours par clé alternative font toujours une descente alternative plus une
  récupération primaire.
- **`WRITE`** ouvre les tables `primary`/`alt` une fois par opération (le contrôle
  de doublon et l'insertion partagent le descripteur). Un micro-benchmark a montré
  que mettre le descripteur en cache *entre* les appels n'ajoute qu'environ 8 %
  par rapport à une ouverture par opération : le moteur garde donc le chemin
  simple et sans `unsafe`. Le coût d'écriture (~44 µs/enregistrement) est dominé
  par l'insertion ACID dans le B+tree de redb, qui est le plancher sûr — aucune
  des optimisations d'écriture ne change les points de validation ni la
  durabilité.
- Le **`WRITE` en masse** tourne donc autour de 20 k enregistrements/s dans une
  transaction unique (un coût de chargement payé une fois). L'OPEN, les lectures
  et la résistance aux pannes n'en sont pas affectés.

---

## Journal d'observabilité (`--indexed-log`)

Le moteur redb peut écrire un journal de transactions facultatif par fichier
(désactivé par défaut) dans **`<chemin-assign>.log`** (p. ex. `customers.idx` →
`customers.idx.log`), avec une ligne par `OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE`
consignant l'horodatage, les compteurs d'enregistrements et d'octets, le débit,
la qualité de l'ordre des clés en écriture et — au niveau `full` — les
statistiques de pages de l'index redb.

```bash
rcrun run app.cbl --indexed-log full --indexed-log-format json
```

Le format de ligne est `text` (logfmt) ou `json` (NDJSON, prêt pour
Grafana/Loki).

**La référence complète** — les options, le tableau des champs, les formats, la
chaîne Grafana/Loki (Promtail + LogQL) et les notes de coût et de sûreté — se
trouve dans [`observability-fr.md`](observability-fr.md) §1.

.<<

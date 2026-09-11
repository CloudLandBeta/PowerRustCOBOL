<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.134 -->

# Les entrailles des fichiers indexés PowerRustCOBOL (moteur paginé `PRCIDXD1`)

Ce document est un schéma conceptuel du moteur **persistant, paginé sur disque**
qui sous-tend les fichiers `ORGANIZATION IS INDEXED` déclarés avec
`STORAGE IS DISK` (la valeur par défaut). C'est une conception à B+tree et pages
à emplacements qui lit les enregistrements à la demande, de sorte que la RAM
reste bornée quelle que soit la taille du fichier.

> ⚠️ **Ce n'est plus le moteur par défaut.** `STORAGE IS DISK` reste le *mode de
> stockage* par défaut, mais depuis **1.62.73** le moteur qui le sert est **redb**
> (`crates/cobolt-runtime/src/indexed.rs:126`) — voir
> [`indexed-redb-engine-fr.md`](indexed-redb-engine-fr.md). Tout ce qui suit reste
> exact pour le moteur paginé, encore accessible avec `--indexed-engine rust` ;
> ce n'est simplement plus ce qu'un programme obtient par défaut.
>
> **Périmètre.** Ceci décrit le *moteur physique* (`DiskIndexedFile`, nombre
> magique de conteneur `PRCIDXD1`). C'est un artefact différent du conteneur
> `PRCIDX1`, monobloc et autodescriptif, documenté dans
> [`indexed-file-format-fr.md`](indexed-file-format-fr.md), qui modélise les
> métadonnées dont un futur importateur Fujitsu aura besoin. Le moteur en mémoire
> (`STORAGE IS MEMORY`, `IndexedFile`) est un sous-ensemble simplifié du même
> modèle logique (des BTreeMap au lieu de B+trees sur disque).
>
> Un second moteur `STORAGE IS DISK`, **résistant aux pannes** (optionnel, sur le
> magasin ACID redb en Rust pur), corrige l'annuaire borné par la RAM et la
> persistance au seul CLOSE de ce moteur — voir
> [`indexed-redb-engine-fr.md`](indexed-redb-engine-fr.md).

Implémentation :
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
et la (dé)matérialisation des enregistrements dans
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. La conception en une phrase

Un fichier paginé fait **d'une page d'en-tête + N B+trees (un par clé) → d'un
annuaire de RecordId → de pages de données à emplacements contenant des images
d'enregistrement positionnelles et à largeur fixe**, avec une liste de pages
libres, des chaînes de débordement, une compression RLE optionnelle et un journal
d'annulation en mémoire pour les transactions.

---

## 2. Le fichier est un tableau de pages fixes de 4 Kio

```
 byte 0                                                        end of file
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 bytes (fixed).   page id = byte offset / 4096.
```

Toute page **après** la page 0 s'identifie par son premier octet (l'étiquette de
type de page). Les pages libérées sont recyclées via une liste de pages libres :
l'ordre physique des pages sur le disque ne suit donc **pas** l'ordre logique des
enregistrements.

| Étiquette | Constante   | La page contient                              |
|-----|---------------|-----------------------------------------------|
| `1` | `PT_INTERNAL` | nœud interne (de routage) du B+tree           |
| `2` | `PT_LEAF`     | nœud feuille du B+tree (doublement chaîné à ses voisins) |
| `3` | `PT_DATA`     | page à emplacements regroupant plusieurs images d'enregistrement |
| `4` | `PT_OVERFLOW` | suite d'un enregistrement trop gros pour tenir en ligne |
| `5` | `PT_DIR`      | une tranche de l'annuaire de RecordId         |

---

## 3. Page 0 — l'en-tête

La page 0 est le seul endroit où un *schéma* est stocké, et elle est écrite une
seule fois. Les champs sont en petit-boutiste, dans cet ordre :

```
 PRCIDXD1  version  page_size  rec_fmt  compressing  record_len
 (8 bytes) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (each u64)
 primary_root   dir_head         directory_len                 (each u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (one B+tree root per alt key)
 ──────────────────────────────────────────────────────────────────────
 KEY SCHEMA:  key_count (u16) → for each key (primary first, then alternates):
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × parts   (composite-key parts)
```

| Champ d'en-tête   | Signification                                                 |
|-------------------|---------------------------------------------------------------|
| `version`         | Version du format (actuellement `1`).                         |
| `page_size`       | Taille de page en octets (4096).                              |
| `rec_fmt`         | Format d'enregistrement : `1` = longueur fixe.                |
| `compressing`     | `1` si les charges utiles sont compressées en RLE sur le disque. |
| `record_len`      | Longueur logique (non compressée) de l'enregistrement, en octets. |
| `next_page_id`    | Prochain identifiant de page à allouer quand la liste libre est vide. |
| `free_list_head`  | Première page de la liste des pages récupérées (`0` = aucune). |
| `record_count`    | Nombre d'enregistrements vivants.                             |
| `data_tail`       | Page `PT_DATA` courante acceptant les écritures en ligne (`0` = aucune). |
| `primary_root`    | Page racine du B+tree de la clé primaire.                     |
| `dir_head`        | Première page `PT_DIR` de l'annuaire de RecordId (`0` = aucune). |
| `directory_len`   | Nombre d'entrées de l'annuaire (RecordId jamais alloués).     |
| `alt_root[k]`     | Page racine du B+tree de la clé alternative *k*.              |
| SCHÉMA DES CLÉS   | Politique de doublons par clé + plages d'octets des parties composites. |

**Ce qui n'est délibérément *pas* dans l'en-tête :** il n'y a **aucun nom de
champ de données** et **aucune métadonnée par enregistrement**. Le schéma est
purement une *géométrie de clés* (des plages d'octets). Tout le reste d'un
enregistrement est positionnel — voir le §6.

---

## 4. Le chemin d'accès (comment un `READ` par clé se résout)

```
  COBOL key value (bytes)
        │
        ▼
  ┌──────────────┐   Start at primary_root (random READ by RECORD KEY) or
  │  B+tree      │   alt_roots[k] (READ KEY IS <alt>). Internal nodes route by
  │  (one per    │   key; leaves hold (key_bytes → RecordId) and are doubly
  │  key)        │   linked (next/prev) for READ NEXT / READ PREVIOUS / START.
  └──────┬───────┘
         │  RecordId (a stable integer, independent of physical location)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = free/tombstone, 1 = inline, 2 = overflow head
  │  directory   │     len : stored (possibly compressed) byte length
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Slotted DATA page → slot directory → (offset, len) →
  │  DATA page   │   raw record image (decompressed if `compressing`).
  └──────┬───────┘
         ▼
  the fixed-width record bytes
        │  RecordLayout.distribute()
        ▼
  scattered into the FD's elementary items in working memory
```

**Un enregistrement, plusieurs clés.** La clé primaire et chaque clé alternative
pointent vers le *même* RecordId : il n'existe donc exactement qu'une copie
stockée de chaque enregistrement. Les index alternatifs ne sont que des B+trees
supplémentaires posés sur l'annuaire de RecordId partagé ; une valeur alternative
en double est permise lorsque cette clé a été déclarée `WITH DUPLICATES`.

---

## 5. Intérieur des pages

### 5.1 Nœud de B+tree (`PT_INTERNAL` / `PT_LEAF`)

Un nœud est chargé en mémoire pour une opération, modifié, éclaté si nécessaire,
puis réécrit.

```
 Leaf:      type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Internal:  type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- Les feuilles sont **doublement chaînées** (`next`/`prev`), si bien qu'un
  parcours ordonné après un `START` passe directement d'une voisine à l'autre —
  c'est le `READ NEXT` à clé ascendante de RustCOBOL.
- L'insertion **éclate en cas de débordement** lorsque le nœud sérialisé
  dépasserait `PAGE_SIZE` ; la clé médiane remonte au parent.
- Les nœuds internes contiennent `child0` plus des paires *(clé séparatrice,
  enfant)*.

### 5.2 Page de données à emplacements (`PT_DATA`)

```
 ┌─ byte 0 ─┬─ 1..3 ──┬─ 3..5 ──┬─ slot directory ──────┬─ free ─┬─ record data ─┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  packed       │
 │          │ count   │ top     │ grows  →              │        │  ←  grows     │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- En-tête de page de 5 octets, puis un **répertoire d'emplacements** qui croît
  depuis l'avant tandis que les **charges utiles** croissent depuis l'arrière ; un
  enregistrement tient en ligne tant que les deux régions ne se sont pas
  rejointes.
- Un emplacement est `(offset, len)` ; supprimer un enregistrement met son
  emplacement à `len = 0` (pierre tombale). Lorsque tous les emplacements d'une
  page sont libres, la page entière retourne à la liste libre.
- Le champ `slot` d'un `RecLoc` indexe ce répertoire d'emplacements.

### 5.3 Chaîne de débordement (`PT_OVERFLOW`)

Un enregistrement plus grand que la limite en ligne
(`PAGE_SIZE − en-tête − un emplacement`) est stocké comme une chaîne de pages de
débordement ; son `RecLoc.kind = 2` et `page` pointe vers la tête de chaîne.

### 5.4 Annuaire de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 bytes/entry)
```

L'annuaire est conservé en RAM sous forme de `Vec<RecLoc>` tant que le fichier
est ouvert (la recherche d'un RecordId est donc un accès indexé en O(1)) et il
est persisté à la fermeture sous forme de chaîne de pages `PT_DIR` (commençant à
`dir_head`). Les B+trees stockent des RecordId, jamais des adresses physiques :
un enregistrement peut donc être déplacé sur le disque sans toucher à aucun
index.

---

## 6. L'image de l'enregistrement elle-même (positionnelle, sans noms)

Un enregistrement sur disque est un unique **tampon d'octets à largeur fixe**
disposé par *décalage* de champ — il n'y a ni nom de champ, ni étiquette, ni
délimiteur dans la charge utile. Pour :

```cobol
01 CUST.
   05 CUST-ID    PIC 9(5).
   05 CUST-NAME  PIC X(10).
   05 CUST-CITY  PIC X(8).
```

l'image stockée fait 23 octets :

```
 offset:  0        5                     15              23
          ┌────────┬─────────────────────┬───────────────┐
 payload: │ 00001  │ John Doe░░          │ Sao Paulo     │
          └────────┴─────────────────────┴───────────────┘
            ID(5)     NAME(10)              CITY(8)
            (░ = space padding)
```

- `RecordLayout::materialize()` tasse les éléments élémentaires du `FD` dans ce
  tampon, par décalage, pour `WRITE`/`REWRITE` ; `RecordLayout::distribute()`
  fait l'inverse au `READ`. La table champ → décalage ne vit que dans le
  `RecordLayout` du programme (dérivé du `FD`), **jamais** dans le fichier.
- **L'identité, c'est la position.** C'est le cas limite du « ne pas répéter les
  clés à chaque enregistrement » : l'identité d'un champ coûte *zéro* octet par
  enregistrement, et l'accès à un champ est en O(1) par décalage précalculé (sans
  analyse). Renommer un champ non-clé ne change rien sur le disque ; renommer un
  champ clé ne réécrit que le schéma de clés de l'en-tête, ni les enregistrements
  ni les index. Changer le décalage ou la largeur d'un champ est le seul
  changement qui oblige à réécrire les données — c'est inhérent aux
  enregistrements de longueur fixe (et aux vrais ISAM/VSAM).

### Compression

Avec `STORAGE IS DISK WITH COMPRESSION`, la charge utile **stockée** est
compressée en RLE PackBits (`compress.rs`), et `RecLoc.len` est la longueur
*stockée* ; le tampon est redéployé à `record_len` à la lecture. La compression
est transparente pour la géométrie des clés et pour le chemin d'accès.

---

## 7. Espace libre et réemploi

- **Liste libre.** `free_list_head` chaîne les pages récupérées sur des pages de
  données vidées, des nœuds orphelins après éclatement, etc. ; `allocate` y
  puise avant d'incrémenter `next_page_id`, si bien que l'espace est réemployé et
  que le fichier ne croît pas de façon monotone.
- **Pierres tombales.** Un `DELETE` libère l'emplacement (et, paresseusement, la
  page de données) et marque l'entrée d'annuaire `RecLoc::FREE` ; le RecordId est
  retiré du service.

---

## 8. Transactions (journal d'annulation en mémoire)

Le moteur disque tient un **journal d'annulation** des opérations inverses de
chaque modification depuis le dernier `COMMIT`/`OPEN` :

```
 DiskUndo::Insert(key)        ← a WRITE   → undone by deleting that key
 DiskUndo::Update(prev_image) ← a REWRITE → undone by rewriting the prior image
 DiskUndo::Delete(prev_image) ← a DELETE  → undone by writing the image back
```

- `OPEN` ouvre une transaction (vide le journal) ; `COMMIT` rend les
  modifications durables et en ouvre une nouvelle ; `ROLLBACK` rejoue les
  inverses en ordre inverse ; `CLOSE` vide les tampons (validation implicite). Un
  garde `tx_replay` empêche les opérations inverses de se journaliser à leur
  tour.
- Il s'agit d'un retour arrière **au niveau du programme**. La reprise après
  panne au moyen d'un journal d'écriture anticipée durable reste un travail
  futur. Voir les verbes COBOL `COMMIT`/`ROLLBACK` dans la référence du langage ;
  notez que ces verbes agissent sur les **fichiers INDEXED**, pas sur les
  connexions SQL.

---

## 9. Validation à l'OPEN

À l'`OPEN`, le schéma de clés stocké dans l'en-tête est comparé au `SELECT` du
programme (longueur d'enregistrement, nombre de clés, parties de chaque clé et
politique de doublons). Une divergence renvoie le file status COBOL `39` ; un
fichier absent ouvert en `INPUT` renvoie `35` ; un en-tête corrompu ou tronqué
renvoie `90`. (La validation stricte peut être assouplie par le drapeau
`strict_metadata` du moteur.)

---

## 10. Aide-mémoire — qui stocke quoi

| Chose                         | Où elle vit                            | Copies      |
|-------------------------------|----------------------------------------|-------------|
| Géométrie des clés (décalages/largeurs) | Schéma de clés de l'en-tête (page 0) | une   |
| Noms des champs de données    | Uniquement le `FD` du programme        | pas dans le fichier |
| Octets des enregistrements    | Pages `PT_DATA` / `PT_OVERFLOW`         | une par enregistrement |
| clé → RecordId                | un B+tree par clé                      | un par clé  |
| RecordId → emplacement physique | Annuaire de RecordId (chaîne `PT_DIR`) | une par enregistrement |
| Pages libres                  | Liste libre (`free_list_head`)         | —           |
| Inverses des changements non validés | Journal d'annulation en RAM      | par transaction |

.<<

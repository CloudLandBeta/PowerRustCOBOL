<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Fonctionnement interne du fichier indexé PowerRustCOBOL (moteur paginé `PRCIDXD1`)

Ce document est un schéma conceptuel du moteur **persistant et paginé sur
disque** qui sous-tend les fichiers `ORGANIZATION IS INDEXED` déclarés avec
`STORAGE IS DISK` (la valeur par défaut). C'est une conception à arbre B+ /
pages à créneaux qui lit les enregistrements à la demande, de sorte que la RAM
reste bornée quelle que soit la taille du fichier.

> **Portée.** Ce document décrit le *moteur physique* (`DiskIndexedFile`, nombre
> magique de conteneur `PRCIDXD1`). C'est un artefact différent du conteneur
> `PRCIDX1`, autodescriptif et en blob unique, documenté dans
> [`indexed-file-format-en.md`](indexed-file-format-fr.md), qui modélise les métadonnées dont
> un futur importateur Fujitsu aura besoin. Le moteur en mémoire
> (`STORAGE IS MEMORY`, `IndexedFile`) est un sous-ensemble simplifié du même
> modèle logique (des BTreeMaps au lieu d'arbres B+ sur disque).
>
> Un second moteur `STORAGE IS DISK`, **résistant aux plantages** (optionnel,
> bâti sur le magasin ACID redb en Rust pur), corrige le répertoire borné par la
> RAM et la persistance uniquement-au-CLOSE de ce moteur — voir
> [`indexed-redb-engine-fr.md`](indexed-redb-engine-fr.md).

Implémentation :
[`crates/cobolt-runtime/src/indexed_disk.rs`](../crates/cobolt-runtime/src/indexed_disk.rs),
(dé)matérialisation des enregistrements dans
[`crates/cobolt-runtime/src/files.rs`](../crates/cobolt-runtime/src/files.rs).

---

## 1. La conception en une phrase

Un fichier paginé composé d'**une page d'en-tête + N arbres B+ (un par clé) → un
répertoire de RecordId → des pages de données à créneaux contenant des images
d'enregistrement positionnelles et de largeur fixe**, avec une liste des pages
libres, des chaînes de débordement, une compression RLE optionnelle et un
journal d'annulation valable le temps de l'exécution pour les transactions.

---

## 2. Le fichier est un tableau de pages fixes de 4 Kio

```
 octet 0                                                 fin du fichier
 ┌────────┬────────┬────────┬────────┬────────┬────────┬───── ... ─────┐
 │ Page 0 │ Page 1 │ Page 2 │ Page 3 │ Page 4 │ Page 5 │     ...       │
 │ HEADER │ B+tree │ B+tree │  DATA  │ DATA   │  DIR   │               │
 └────────┴────────┴────────┴────────┴────────┴────────┴───────────────┘
   PAGE_SIZE = 4096 octets (fixe).   id de page = décalage en octets / 4096.
```

Toute page **postérieure** à la page 0 s'identifie elle-même par son premier
octet (l'étiquette de type de page). Les pages libérées sont recyclées via une
liste des pages libres : l'ordre physique des pages sur le disque ne suit donc
**pas** l'ordre logique des enregistrements.

| Étiq. | Constante   | La page contient                                      |
|-----|---------------|--------------------------------------------------------|
| `1` | `PT_INTERNAL` | nœud interne (de routage) de l'arbre B+                 |
| `2` | `PT_LEAF`     | nœud feuille de l'arbre B+ (doublement lié aux frères)  |
| `3` | `PT_DATA`     | page à créneaux regroupant plusieurs images d'enregistrement |
| `4` | `PT_OVERFLOW` | suite d'un enregistrement trop gros pour tenir en ligne |
| `5` | `PT_DIR`      | une tranche du répertoire de RecordId                   |

---

## 3. Page 0 — l'en-tête

La page 0 est le seul endroit où un *schéma* est stocké, et elle n'est écrite
qu'une fois. Les champs sont en petit-boutiste, dans cet ordre :

```
 PRCIDXD1   version  page_size  rec_fmt  compressing  record_len
 (8 octets) (u16)    (u32)      (u8 = 1) (u8 0/1)     (u32)
 ──────────────────────────────────────────────────────────────────────
 next_page_id   free_list_head   record_count   data_tail      (chacun u64)
 primary_root   dir_head         directory_len                 (chacun u64)
 ──────────────────────────────────────────────────────────────────────
 alt_root_count (u16) → [ alt_root : u64 ] × N     (une racine B+ par clé alt.)
 ──────────────────────────────────────────────────────────────────────
 SCHÉMA DES CLÉS:  key_count (u16) → pour chaque clé (la primaire d'abord, puis les alternatives) :
     duplicates_allowed (u8)
     part_count (u16) → [ offset:u32, length:u32 ] × parties   (parties de clé composée)
```

| Champ d'en-tête   | Signification                                           |
|-------------------|---------------------------------------------------------|
| `version`         | Version du format (actuellement `1`).                   |
| `page_size`       | Taille de page en octets (4096).                        |
| `rec_fmt`         | Format d'enregistrement : `1` = longueur fixe.          |
| `compressing`     | `1` si les charges utiles des enregistrements sont compressées en RLE sur disque. |
| `record_len`      | Longueur logique (non compressée) de l'enregistrement, en octets. |
| `next_page_id`    | Prochain id de page à allouer quand la liste des pages libres est vide. |
| `free_list_head`  | Première page de la liste des pages récupérées (`0` = aucune). |
| `record_count`    | Nombre d'enregistrements vivants.                       |
| `data_tail`       | Page `PT_DATA` courante acceptant les écritures en ligne (`0` = aucune). |
| `primary_root`    | Page racine de l'arbre B+ de la clé primaire.           |
| `dir_head`        | Première page `PT_DIR` du répertoire de RecordId (`0` = aucune). |
| `directory_len`   | Nombre d'entrées du répertoire (RecordId alloués depuis toujours). |
| `alt_root[k]`     | Page racine de l'arbre B+ de la clé alternative *k*.    |
| SCHÉMA DES CLÉS   | Politique de doublons par clé + plages d'octets des parties composées. |

**Ce qui est délibérément *absent* de l'en-tête :** il n'y a **aucun nom de champ
de données** et **aucune métadonnée par enregistrement**. Le schéma se réduit à
la *géométrie des clés* (des plages d'octets). Tout le reste d'un enregistrement
est positionnel — voir §6.

---

## 4. Le chemin d'accès (comment un `READ` par clé se résout)

```
  valeur de clé COBOL (octets)
        │
        ▼
  ┌──────────────┐   Départ à primary_root (READ aléatoire par RECORD KEY) ou
  │  B+tree      │   à alt_roots[k] (READ KEY IS <alt>). Les nœuds internes
  │  (une par    │   routent par clé ; les feuilles portent (key_bytes →
  │  clé)        │   RecordId) et sont doublement liées (next/prev) pour
  └──────┬───────┘   READ NEXT / READ PREVIOUS / START.
         │  RecordId (un entier stable, indépendant de l'emplacement physique)
         ▼
  ┌──────────────┐   directory[RecordId] = RecLoc { kind, page, slot, len }
  │  RecordId    │     kind: 0 = libre/pierre tombale, 1 = en ligne, 2 = tête de débordement
  │  répertoire  │     len : longueur en octets stockée (peut-être compressée)
  └──────┬───────┘
         │  (page, slot)
         ▼
  ┌──────────────┐   Page DATA à créneaux → répertoire de créneaux →
  │  page DATA   │   (offset, len) → image brute de l'enregistrement
  └──────┬───────┘   (décompressée si `compressing`).
         ▼
  les octets de l'enregistrement de largeur fixe
        │  RecordLayout.distribute()
        ▼
  répartis dans les éléments élémentaires du FD en mémoire de travail
```

**Un enregistrement, plusieurs clés.** La clé primaire et chaque clé alternative
pointent vers le *même* RecordId : il n'existe donc qu'une seule copie stockée de
chaque enregistrement. Les index alternatifs ne sont que des arbres B+
supplémentaires posés sur le répertoire de RecordId partagé ; une valeur
alternative en double est admise lorsque cette clé a été déclarée
`WITH DUPLICATES`.

---

## 5. Intérieur des pages

### 5.1 Nœud d'arbre B+ (`PT_INTERNAL` / `PT_LEAF`)

Un nœud est chargé en mémoire pour une opération, modifié, scindé si nécessaire,
puis réécrit.

```
 Feuille:   type=2 | next:u64 | prev:u64 | count:u16 | [ klen:u16, key, RecordId:u64 ] × count
 Interne:   type=1 | child0:u64           | count:u16 | [ klen:u16, key, child:u64  ] × count
```

- Les feuilles sont **doublement liées** (`next`/`prev`) : un parcours ordonné
  après un `START` marche donc directement de frère en frère — c'est le
  `READ NEXT` à clé ascendante de RustCOBOL.
- L'insertion **scinde en cas de débordement** lorsque le nœud sérialisé
  dépasserait `PAGE_SIZE` ; la clé médiane est promue au parent.
- Les nœuds internes portent `child0` plus des paires *(clé séparatrice, fils)*.

### 5.2 Page de données à créneaux (`PT_DATA`)

```
 ┌─ octet 0 ┬─ 1..3 ──┬─ 3..5 ──┬─ rép. des créneaux ───┬─ libre ┬─ données ─────┐
 │ type=3   │ slot_   │ free_   │ (off:u16, len:u16) ×N │        │  compactées   │
 │          │ count   │ top     │ croît  →              │        │  ←  croissent │
 └──────────┴─────────┴─────────┴───────────────────────┴────────┴───────────────┘
```

- En-tête de page de 5 octets, puis un **répertoire de créneaux** qui croît
  depuis l'avant tandis que les **charges utiles des enregistrements** croissent
  depuis l'arrière ; un enregistrement tient en ligne tant que les deux régions
  ne se sont pas rejointes.
- Un créneau est `(offset, len)` ; supprimer un enregistrement met le `len = 0`
  de son créneau (pierre tombale). Lorsque tous les créneaux d'une page sont
  libres, la page entière retourne à la liste des pages libres.
- Le champ `slot` d'un `RecLoc` indexe dans ce répertoire de créneaux.

### 5.3 Chaîne de débordement (`PT_OVERFLOW`)

Un enregistrement plus grand que la limite en ligne
(`PAGE_SIZE − en-tête − un créneau`) est stocké comme une chaîne liée de pages de
débordement ; son `RecLoc.kind = 2` et `page` pointe vers la tête de la chaîne.

### 5.4 Répertoire de RecordId (`PT_DIR`)

```
 directory[RecordId]  →  RecLoc { kind:u8, page:u64, slot:u16, len:u32 }   (15 octets/entrée)
```

Le répertoire est conservé en RAM sous forme de `Vec<RecLoc>` tant que le fichier
est ouvert (une recherche de RecordId est donc une indexation en O(1)) et il est
persisté à la fermeture sous forme de chaîne de pages `PT_DIR` (à partir de
`dir_head`). Les arbres B+ stockent des RecordId, jamais des adresses physiques :
un enregistrement peut donc être déplacé sur le disque sans toucher au moindre
index.

---

## 6. L'image de l'enregistrement elle-même (positionnelle, sans noms)

Un enregistrement sur disque est un unique **tampon d'octets de largeur fixe**
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
 décalage :       0        5                     15              23
                  ┌────────┬─────────────────────┬───────────────┐
 charge utile :   │ 00001  │ John Doe░░          │ Sao Paulo     │
                  └────────┴─────────────────────┴───────────────┘
                    ID(5)     NAME(10)              CITY(8)
                    (░ = remplissage par des espaces)
```

- `RecordLayout::materialize()` tasse les éléments élémentaires du FD dans ce
  tampon par décalage pour `WRITE`/`REWRITE` ; `RecordLayout::distribute()`
  effectue l'inverse au `READ`. La table champ → décalage n'existe que dans le
  `RecordLayout` du programme (dérivé du `FD`), **jamais** dans le fichier.
- **L'identité, c'est la position.** C'est le cas limite du « ne pas répéter les
  clés dans chaque enregistrement » : l'identité d'un champ coûte *zéro* octet
  par enregistrement, et l'accès au champ est en O(1) par décalage précalculé
  (aucune analyse). Renommer un champ non clé ne change rien sur le disque ;
  renommer un champ clé ne réécrit que le schéma des clés de l'en-tête, pas les
  enregistrements ni les index. Changer le décalage ou la largeur d'un champ est
  le seul changement qui impose de réécrire les données — c'est inhérent aux
  enregistrements de longueur fixe (et aux vrais ISAM/VSAM).

### Compression

Avec `STORAGE IS DISK WITH COMPRESSION`, la charge utile **stockée** est
compressée en PackBits-RLE (`compress.rs`), et `RecLoc.len` est la longueur
*stockée* ; le tampon est réétendu à `record_len` à la lecture. La compression
est transparente pour la géométrie des clés et pour le chemin d'accès.

---

## 7. Espace libre et réutilisation

- **Liste des pages libres.** `free_list_head` chaîne les pages récupérées sur
  des pages de données vidées, des nœuds orphelins après une scission, etc. ;
  `allocate` y puise avant d'incrémenter `next_page_id`, de sorte que l'espace
  est réutilisé et que le fichier ne grossit pas de façon monotone.
- **Pierres tombales.** Un `DELETE` libère le créneau (et, paresseusement, la
  page de données) et marque l'entrée du répertoire `RecLoc::FREE` ; le RecordId
  est mis à la retraite.

---

## 8. Transactions (journal d'annulation en cours d'exécution)

Le moteur disque tient un **journal d'annulation** des inverses de chaque
mutation depuis le dernier `COMMIT`/`OPEN` :

```
 DiskUndo::Insert(key)        ← un WRITE   → annulé en supprimant cette clé
 DiskUndo::Update(prev_image) ← un REWRITE → annulé en réécrivant l'image précédente
 DiskUndo::Delete(prev_image) ← un DELETE  → annulé en réécrivant l'image d'origine
```

- `OPEN` démarre une transaction (vide le journal) ; `COMMIT` rend les
  changements durables et en démarre une nouvelle ; `ROLLBACK` rejoue les
  inverses en ordre inverse ; `CLOSE` vide les tampons (commit implicite). Un
  garde `tx_replay` empêche les opérations inverses de se journaliser
  elles-mêmes.
- Il s'agit d'une annulation **au niveau du programme**. La reprise après
  plantage via un journal d'écriture anticipée durable reste à faire. Voir les
  verbes COBOL `COMMIT`/`ROLLBACK` dans la référence du langage ; notez que ces
  verbes agissent sur les **fichiers INDEXED**, pas sur les connexions SQL.

---

## 9. Validation à l'OPEN

À l'`OPEN`, le schéma des clés stocké dans l'en-tête est comparé au `SELECT` du
programme (longueur d'enregistrement, nombre de clés, parties et politique de
doublons de chaque clé). Une divergence renvoie le file status COBOL `39` ; un
fichier absent ouvert en `INPUT` renvoie `35` ; un en-tête corrompu ou tronqué
renvoie `90`. (La validation stricte peut être assouplie via l'indicateur
`strict_metadata` du moteur.)

---

## 10. Référence rapide — qui stocke quoi

| Élément                           | Où il réside                           | Copies        |
|-------------------------------|----------------------------------------|-------------|
| Géométrie des clés (décalages/largeurs) | Schéma des clés de l'en-tête (page 0) | une fois |
| Noms des champs de données    | Uniquement dans le `FD` du programme   | pas dans le fichier |
| Octets de l'enregistrement    | Pages `PT_DATA` / `PT_OVERFLOW`        | un/enregistrement |
| clé → RecordId                | un arbre B+ par clé                    | un/clé      |
| RecordId → emplacement physique | Répertoire de RecordId (chaîne `PT_DIR`) | un/enregistrement |
| Pages libres                  | Liste des pages libres (`free_list_head`) | —        |
| Inverses des changements non validés | Journal d'annulation en RAM      | par tx      |
```

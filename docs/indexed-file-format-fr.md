<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Format de fichier indexé PowerRustCOBOL (`PRCIDX1`)

Ce document décrit le conteneur sur disque qui sous-tend les fichiers
`ORGANIZATION IS INDEXED` dans PowerRustCOBOL, et la façon dont il correspond aux
métadonnées dont aura besoin un futur **importateur Fujitsu COBOL-85 →
PowerRustCOBOL**.

> **Pas compatible au niveau binaire avec Fujitsu.** `PRCIDX1` est le conteneur
> autodescriptif propre à PowerRustCOBOL. Il est modelé *sémantiquement* sur les
> métadonnées que les File Access Subroutines de Fujitsu exposent via
> `cobfa_indexinfo()` (format d'enregistrement, longueur d'enregistrement, nombre
> et longueur totale des clés, clé primaire, clés alternatives), mais il
> n'analyse **pas** et ne reproduit pas les octets `cobidx`/`cobi64` de Fujitsu.
> L'importateur est un travail futur et vit hors de PowerRustCOBOL.

Implémentation : [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Pourquoi le format est autodescriptif

Le conteneur d'origine (`PRCISAM1`) ne stockait qu'un nombre magique, la longueur
d'enregistrement et les octets des enregistrements : il ne portait **aucun schéma
de clés**. Un convertisseur (ou tout outil externe) ne pouvait pas savoir quelles
étaient les clés sans le `FD` COBOL.

`PRCIDX1` intègre le schéma complet dans le fichier : le format
d'enregistrement et, pour chaque clé, sa disposition en octets, son ordre, sa
politique de doublons et (éventuellement) son nom de champ COBOL. Le fichier
devient ainsi **explorable** — voir [`inspect_path`](#api-dexploration) — et un
importateur Fujitsu peut écrire un fichier PowerRustCOBOL fidèle à partir des
métadonnées qu'il lit dans un fichier Fujitsu, sans disposer d'un `FD`
correspondant.

---

## Modèle de métadonnées

Ces types Rust (réexportés depuis `cobolt_runtime`) sont le schéma. Ils reflètent
les concepts de `cobfa_indexinfo()` ; tous les décalages et longueurs sont **en
octets** (jamais des comptes de caractères — conformément à la règle Fujitsu en
mode Unicode).

```rust
pub enum RecordFormat {
    Fixed { length: u32 },
    Variable { min_length: u32, max_length: u32 },
}

pub enum KeyEncoding {
    Bytes, DisplayAscii, DisplayUtf8,
    Ucs2Le, Ucs2Be, Utf32Le, Utf32Be,
    PackedDecimal, BinaryBigEndian, BinaryLittleEndian,
}

pub enum KeyOrdering { Ascending, Descending }

pub struct KeyPart { pub offset: u32, pub length: u32, pub encoding: KeyEncoding }

pub struct KeyDescriptor {
    pub key_number: u16,          // 1 = primary, 2.. = alternates (declaration order)
    pub name: Option<String>,     // descriptive COBOL field name (optional)
    pub parts: Vec<KeyPart>,      // concatenated → composite key value
    pub duplicates_allowed: bool,
    pub ordering: KeyOrdering,
}

pub struct IndexedFileInfo {
    pub record_format: RecordFormat,
    pub key_count: u16,           // primary + alternates
    pub total_key_length: u32,
    pub primary: KeyDescriptor,
    pub alternates: Vec<KeyDescriptor>,
}
```

Le runtime actuel émet des clés **à une seule partie, encodées en `Bytes` et
`Ascending`** (c'est ce à quoi se résout un `RECORD KEY` /
`ALTERNATE RECORD KEY` d'un `FD` COBOL). Les clés composites, les encodages
alternatifs et l'ordre descendant sont **représentables dans le format**, afin
qu'un importateur puisse les consigner sans perte ; leur prise en charge complète
par le runtime est un travail futur.

---

## Disposition du conteneur

Tous les entiers sont en **petit-boutiste**. Le fichier se présente ainsi :

```text
┌────────────────────────────────────────────────────────────┐
│ Header                                                      │
│ Key schema  (key_count descriptors: primary, then alts)     │
│ Records                                                     │
│ CRC-32 trailer (over all preceding bytes)                   │
└────────────────────────────────────────────────────────────┘
```

### En-tête

| Champ            | Type      | Notes                                   |
|------------------|-----------|-----------------------------------------|
| `magic`          | `[u8; 8]` | `b"PRCIDX1\0"`                          |
| `version`        | `u16`     | `1`                                     |
| `flags`          | `u16`     | réservé (`0`)                           |
| `record_format`  | `u8`      | `1` = fixe, `2` = variable              |
| `reserved`       | `u8`      | `0`                                     |
| `fixed_length`   | `u32`     | longueur d'enregistrement si fixe       |
| `min_length`     | `u32`     | charge utile minimale si variable       |
| `max_length`     | `u32`     | charge utile maximale si variable       |
| `key_count`      | `u16`     | primaire + alternatives                 |
| `created_unix_ms`| `u64`     | date de création, préservée lors des réécritures|
| `updated_unix_ms`| `u64`     | date de dernière écriture               |

### Schéma de clés — répété `key_count` fois (la primaire d'abord)

| Champ          | Type      | Notes                                   |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primaire, `2..` alternatives        |
| `duplicates`   | `u8`      | `0`/`1`                                 |
| `ordering`     | `u8`      | `0` ascendant, `1` descendant           |
| `part_count`   | `u16`     | nombre de `KeyPart`                     |
| `name_len`     | `u16`     | longueur du nom UTF-8 (`0` = aucun)     |
| `name`         | `[u8]`    | `name_len` octets                       |
| `parts`        | répété    | `part_count` × KeyPart (ci-dessous)     |

Chaque **KeyPart** :

| Champ      | Type  | Notes                                   |
|------------|-------|-----------------------------------------|
| `offset`   | `u32` | décalage en octets dans la charge utile de l'enregistrement|
| `length`   | `u32` | longueur en octets                      |
| `encoding` | `u8`  | discriminant de `KeyEncoding`           |
| `reserved` | `u8`  | `0`                                     |

### Enregistrements

| Champ          | Type   | Notes                                   |
|----------------|--------|-----------------------------------------|
| `record_count` | `u64`  | nombre d'enregistrements vivants        |
| par enregistrement | répété | `length: u32` puis `length` octets  |

Les enregistrements sont écrits par ordre ascendant de **clé primaire**.

### Fin de fichier

| Champ   | Type  | Notes                                            |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, réfléchi) sur tous les octets précédant la fin de fichier |

Le CRC est validé au chargement ; une divergence donne FILE STATUS `90` (erreur
d'E/S).

---

## API d'exploration

```rust
use cobolt_runtime::indexed::IndexedFile; // (engine type — not re-exported at the crate root)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Renvoie `Some(IndexedFileInfo)` pour un fichier `PRCIDX1` et `None` pour
l'ancien conteneur `PRCISAM1` (qui ne porte aucun schéma). C'est l'équivalent de
`cobfa_indexinfo()` qu'un convertisseur ou un outil d'inspection peut appeler.

---

## Validation à l'ouverture (FILE STATUS)

À l'ouverture d'un fichier indexé **existant** en `INPUT` / `I-O`, le runtime
valide les clés et le format d'enregistrement déclarés dans le `SELECT`/`FD`
contre le schéma stocké (mode strict, actif par défaut). Statuts pertinents :

| Statut | Condition                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` d'un fichier inexistant                  |
| `39`   | schéma du fichier existant ≠ clés ou format d'enregistrement déclarés |
| `90`   | conteneur corrompu (CRC divergent) ou autre erreur d'E/S |

L'ancien conteneur `PRCISAM1` n'a pas de schéma : la validation stricte est donc
omise pour lui (il se charge toujours de façon permissive).

---

## Modes de stockage (`STORAGE IS MEMORY | DISK`)

La clause `STORAGE MODE` choisit quel moteur — et donc quel conteneur sur disque
— sous-tend un fichier INDEXED. **Le mode de stockage par défaut est `DISK`**
(en l'absence de clause `STORAGE`). `WITH COMPRESSION` s'applique aux deux
modes ; `WITH PERSISTENCE` ne s'applique qu'à `MEMORY`.

| Mode | Moteur | Conteneur | Notes |
|------|--------|-----------|-------|
| `MEMORY` | `BTreeMap` en RAM (`indexed.rs`) | `PRCIDX1` (ce document) | fichier entier en mémoire ; **éphémère par défaut** — `COMMIT` n'écrit jamais sur le disque. Avec `WITH PERSISTENCE`, enregistré en `PRCIDX1` au seul `CLOSE`. `OPEN OUTPUT` (re)crée toujours le conteneur. |
| `DISK` (par défaut) | magasin redb résistant aux pannes (`indexed_redb.rs`) depuis 1.62.73 ; le B+tree paginé (`indexed_disk.rs`) avec `--indexed-engine rust` | celui de redb, ou `PRCIDXD1` pour le moteur paginé | enregistrements et index lus à la demande ; RAM bornée ; toujours persistant (écritures par opération, `fsync` au `COMMIT`/`CLOSE`) |

Le conteneur disque **`PRCIDXD1`** est un fichier paginé unique (pages de
4 Kio) :

* **page 0** — en-tête : racines (un B+tree par clé), tête de la liste des pages
  libres, prochain identifiant de page, compteur de `RecordId`, nombre
  d'enregistrements, le schéma de clés et l'indicateur de compression.
* **pages de B+tree** — nœuds internes et feuilles (empaquetage en octets de
  taille variable, division à l'insertion, feuilles doublement chaînées pour les
  parcours ordonnés).
* **pages de données** — cellules d'enregistrement à emplacements (plusieurs
  enregistrements par page), plus une chaîne de pages de débordement pour les
  enregistrements plus grands qu'une page.
* **pages d'annuaire** — la table `RecordId` → emplacement physique.
* une **liste libre** enfile les pages libérées en vue de leur réemploi.

`WITH COMPRESSION` (`compress.rs`) est un RLE de style PackBits sans dépendance,
appliqué à chaque enregistrement stocké (`PRCIDXD1`) ou à chaque enregistrement
de la section des enregistrements (`PRCIDX1`) ; une étiquette d'un octet garantit
que l'encodage ne grossit jamais, et l'en-tête du conteneur note que la
compression est active.

> `PRCIDXD1` est destiné au stockage natif en mode DISK. Les métadonnées
> explorables et orientées import Fujitsu décrites plus haut sont celles du
> conteneur `PRCIDX1` (mode MEMORY) ; un importateur devrait viser `PRCIDX1`, à
> moins d'avoir spécifiquement besoin de la disposition paginée sur disque.

## Compatibilité ascendante

* `PRCIDX1` (nombre magique `PRCIDX1\0`) — le format autodescriptif actuel du
  mode MEMORY (lecture + écriture).
* `PRCIDXD1` (nombre magique `PRCIDXD1`) — conteneur paginé de B+tree en mode
  DISK.
* `PRCISAM1` (nombre magique `PRCISAM1`) — ancien conteneur ne contenant que des
  enregistrements (lecture seule ; réenregistré en `PRCIDX1` au `CLOSE` suivant
  d'une ouverture en écriture).
* Tout autre contenu — traité comme un fichier vide.

---

## Future voie d'import depuis Fujitsu

Le flux de migration envisagé (entièrement hors du périmètre actuel de
PowerRustCOBOL) :

```text
Fujitsu runtime
  └─ cobfa_indexinfo()  → record format, record length, key list (primary + alternates)
  └─ sequential export  → record payloads
        │
        ▼
  converter (future, external)
        │  builds IndexedFileInfo + records
        ▼
  PRCIDX1 file  → opened natively by PowerRustCOBOL
```

Comme `PRCIDX1` sait déjà *représenter* les clés composites, les encodages de
clé, l'ordre des clés, la politique de doublons, les bornes des enregistrements
de longueur variable et les noms des champs clés, il ne reste au convertisseur
qu'à traduire les métadonnées Fujitsu en `IndexedFileInfo` et à débiter les
enregistrements — aucun changement du format PowerRustCOBOL n'est requis.

**N'essayez pas** d'analyser les octets bruts `cobidx`/`cobi64` de Fujitsu. La
documentation publique de Fujitsu expose les métadonnées via les File Access
Subroutines mais ne publie pas la disposition physique des octets.

.<<

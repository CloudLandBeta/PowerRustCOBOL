<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Format de fichier indexé de PowerRustCOBOL (`PRCIDX1`)

Ce document décrit le conteneur sur disque qui sous-tend les fichiers
`ORGANIZATION IS INDEXED` dans PowerRustCOBOL, ainsi que sa correspondance avec
les métadonnées dont aura besoin un futur **importateur Fujitsu COBOL-85 →
PowerRustCOBOL**.

> **Pas de compatibilité binaire avec Fujitsu.** `PRCIDX1` est le conteneur
> autodescriptif propre à PowerRustCOBOL. Il est modelé *sémantiquement* sur les
> métadonnées que les File Access Subroutines de Fujitsu exposent via
> `cobfa_indexinfo()` (format d'enregistrement, longueur d'enregistrement,
> nombre et longueur totale des clés, clé primaire, clés alternatives), mais il
> n'analyse ni ne reproduit **pas** les octets `cobidx`/`cobi64` de Fujitsu.
> L'importateur relève de travaux futurs et vit en dehors de PowerRustCOBOL.

Implémentation : [`crates/cobolt-runtime/src/indexed.rs`](../crates/cobolt-runtime/src/indexed.rs).

---

## Pourquoi le format est autodescriptif

Le conteneur d'origine (`PRCISAM1`) ne stockait qu'un nombre magique, la
longueur d'enregistrement et les octets de l'enregistrement : il ne portait
**aucun schéma de clés**. Un convertisseur (ou tout outil externe) ne pouvait
pas savoir quelles étaient les clés sans le `FD` COBOL.

`PRCIDX1` intègre le schéma complet dans le fichier : le format
d'enregistrement et, pour chaque clé, sa disposition d'octets, son ordre de tri,
sa politique de doublons et (facultativement) son nom de champ COBOL. Le fichier
devient ainsi **explorable** — voir [`inspect_path`](#api-de-découverte) — et
un importateur Fujitsu peut écrire un fichier PowerRustCOBOL fidèle à partir des
métadonnées qu'il lit dans un fichier Fujitsu, sans disposer du `FD`
correspondant.

---

## Modèle de métadonnées

Ces types Rust (réexportés depuis `cobolt_runtime`) constituent le schéma. Ils
reprennent les concepts de `cobfa_indexinfo()` ; tous les décalages et longueurs
sont exprimés **en octets** (jamais en nombre de caractères — conformément à la
règle de Fujitsu en mode Unicode).

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
`Ascending`** (c'est ce à quoi se résout un `RECORD KEY` / `ALTERNATE RECORD
KEY` d'un `FD` COBOL). Les clés composites, les autres encodages et l'ordre
décroissant sont **représentables dans le format**, afin qu'un importateur
puisse les consigner sans perte ; leur prise en charge complète par le runtime
relève de travaux futurs.

---

## Disposition du conteneur

Tous les entiers sont en **little-endian**. Le fichier se présente ainsi :

```text
┌────────────────────────────────────────────────────────────┐
│ En-tête                                                    │
│ Schéma de clés (key_count descripteurs : primaire, alt.)   │
│ Enregistrements                                            │
│ Bloc de fin CRC-32 (sur tous les octets précédents)        │
└────────────────────────────────────────────────────────────┘
```

### En-tête

| Champ            | Type      | Remarques                               |
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
| `created_unix_ms`| `u64`     | date de création, conservée d'une réécriture à l'autre|
| `updated_unix_ms`| `u64`     | date de dernière écriture               |

### Schéma de clés — répété `key_count` fois (la primaire d'abord)

| Champ          | Type      | Remarques                               |
|----------------|-----------|-----------------------------------------|
| `key_number`   | `u16`     | `1` primaire, `2..` alternatives        |
| `duplicates`   | `u8`      | `0`/`1`                                  |
| `ordering`     | `u8`      | `0` croissant, `1` décroissant          |
| `part_count`   | `u16`     | nombre de `KeyPart`                     |
| `name_len`     | `u16`     | longueur du nom UTF-8 (`0` = aucun)     |
| `name`         | `[u8]`    | `name_len` octets                       |
| `parts`        | répété    | `part_count` × KeyPart (ci-dessous)     |

Chaque **KeyPart** :

| Champ      | Type  | Remarques                      |
|------------|-------|--------------------------------|
| `offset`   | `u32` | décalage en octets dans la charge utile|
| `length`   | `u32` | longueur en octets             |
| `encoding` | `u8`  | discriminant de `KeyEncoding`  |
| `reserved` | `u8`  | `0`                            |

### Enregistrements

| Champ             | Type   | Remarques                              |
|-------------------|--------|----------------------------------------|
| `record_count`    | `u64`  | nombre d'enregistrements vivants       |
| par enregistrement| répété | `length: u32` puis `length` octets     |

Les enregistrements sont écrits par ordre croissant de **clé primaire**.

### Bloc de fin

| Champ   | Type  | Remarques                                        |
|---------|-------|--------------------------------------------------|
| `crc32` | `u32` | CRC-32 (IEEE 802.3, réfléchi) sur tous les octets précédant le bloc de fin |

Le CRC est vérifié au chargement ; une divergence donne le FILE STATUS `90`
(erreur d'E/S).

---

## API de découverte

```rust
use cobolt_runtime::IndexedFile; // (engine type)

// Read just the schema, without opening the file for I/O:
let info: Option<IndexedFileInfo> = IndexedFile::inspect_path("customers.idx")?;
```

Renvoie `Some(IndexedFileInfo)` pour un fichier `PRCIDX1` et `None` pour
l'ancien conteneur `PRCISAM1` (qui ne porte aucun schéma). C'est l'équivalent de
`cobfa_indexinfo()` qu'un convertisseur ou un outil d'inspection peut appeler.

---

## Validation à l'ouverture (FILE STATUS)

À l'ouverture d'un fichier indexé **existant** en `INPUT` / `I-O`, le runtime
vérifie les clés déclarées dans `SELECT`/`FD` ainsi que le format
d'enregistrement par rapport au schéma stocké (mode strict, actif par défaut).
Statuts concernés :

| Statut | Condition                                              |
|-------:|-------------------------------------------------------|
| `35`   | `OPEN INPUT` d'un fichier inexistant                  |
| `39`   | schéma du fichier existant ≠ clés/format d'enregistrement déclarés |
| `90`   | conteneur corrompu (CRC non concordant) ou autre erreur d'E/S |

L'ancien conteneur `PRCISAM1` n'a pas de schéma : la validation stricte est donc
ignorée pour lui (il se charge toujours de façon permissive).

---

## Modes de stockage (`STORAGE IS MEMORY | DISK`)

La clause `STORAGE MODE` choisit quel moteur — et donc quel conteneur sur disque
— sous-tend un fichier INDEXED. **Le mode de stockage par défaut est `DISK`**
(en l'absence de clause `STORAGE`). `WITH COMPRESSION` s'applique aux deux
modes ; `WITH PERSISTENCE` ne s'applique qu'à `MEMORY`.

| Mode | Moteur | Conteneur | Remarques |
|------|--------|-----------|-----------|
| `MEMORY` | `BTreeMap` en RAM (`indexed.rs`) | `PRCIDX1` (ce document) | fichier entier en mémoire ; **éphémère par défaut** — `COMMIT` n'écrit jamais sur disque. Avec `WITH PERSISTENCE`, l'enregistrement dans `PRCIDX1` n'a lieu qu'au `CLOSE`. `OPEN OUTPUT` (re)crée toujours le conteneur. |
| `DISK` (par défaut) | arbre B+ paginé et persistant (`indexed_disk.rs`) | `PRCIDXD1` | enregistrements et index lus à la demande ; RAM bornée ; toujours persistant (écritures par opération, `fsync` au `COMMIT`/`CLOSE`) |

Le conteneur disque **`PRCIDXD1`** est un fichier paginé unique (pages de
4 Kio) :

* **page 0** — en-tête : les racines (un arbre B+ par clé), la tête de la liste
  des pages libres, l'identifiant de la page suivante, le compteur `RecordId`,
  le nombre d'enregistrements, le schéma de clés et l'indicateur de compression.
* **pages d'arbre B+** — nœuds internes / feuilles (octets empaquetés de taille
  variable, éclatement à l'insertion, feuilles doublement chaînées pour les
  parcours ordonnés).
* **pages de données** — cellules d'enregistrement à emplacements (plusieurs
  enregistrements par page), plus une chaîne de pages de débordement pour les
  enregistrements plus grands qu'une page.
* **pages de répertoire** — la table de correspondance `RecordId` →
  emplacement physique.
* une **liste des pages libres** chaîne les pages libérées en vue de leur
  réutilisation.

`WITH COMPRESSION` (`compress.rs`) est un RLE de type PackBits, sans dépendance,
appliqué à chaque enregistrement stocké (`PRCIDXD1`) ou à chaque enregistrement
de la section des enregistrements (`PRCIDX1`) ; un marqueur d'un octet garantit
que l'encodage ne grossit jamais, et l'en-tête du conteneur consigne que la
compression est active.

> `PRCIDXD1` est le stockage natif du mode DISK. Les métadonnées explorables et
> orientées import Fujitsu décrites ci-dessus concernent le conteneur `PRCIDX1`
> (mode MEMORY) ; un importateur doit viser `PRCIDX1`, sauf s'il lui faut
> spécifiquement la disposition paginée sur disque.

## Rétrocompatibilité

* `PRCIDX1` (nombre magique `PRCIDX1\0`) — format autodescriptif actuel du mode
  MEMORY (lecture + écriture).
* `PRCIDXD1` (nombre magique `PRCIDXD1`) — conteneur à arbre B+ paginé du mode
  DISK.
* `PRCISAM1` (nombre magique `PRCISAM1`) — ancien conteneur ne contenant que des
  enregistrements (lecture seule ; réenregistré au format `PRCIDX1` au `CLOSE`
  suivant d'une ouverture en écriture).
* Tout autre contenu — traité comme un fichier vide.

---

## Futur chemin d'importation depuis Fujitsu

Le flux de migration envisagé (aujourd'hui entièrement hors du périmètre de
PowerRustCOBOL) :

```text
runtime Fujitsu
  └─ cobfa_indexinfo()  → format d'enregistrement, longueur d'enregistrement, liste des clés (primaire + alternatives)
  └─ export séquentiel  → charges utiles des enregistrements
        │
        ▼
  convertisseur (futur, externe)
        │  construit IndexedFileInfo + enregistrements
        ▼
  fichier PRCIDX1  → ouvert nativement par PowerRustCOBOL
```

Comme `PRCIDX1` sait déjà *représenter* les clés composites, les encodages de
clés, l'ordre de tri des clés, la politique de doublons, les bornes des
enregistrements de longueur variable et les noms des champs clés, le
convertisseur n'a qu'à traduire les métadonnées Fujitsu en `IndexedFileInfo` et
à diffuser les enregistrements — aucun changement de format PowerRustCOBOL n'est
nécessaire.

**N'essayez pas** d'analyser les octets bruts `cobidx`/`cobi64` de Fujitsu. La
documentation publique de Fujitsu expose les métadonnées via les File Access
Subroutines, mais ne publie pas la disposition physique des octets.

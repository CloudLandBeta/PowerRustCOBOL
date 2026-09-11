<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# IDE PowerRustCOBOL — Collaboration (phase B) — Conception

> **État : conception seulement.** Rien de ce qui suit n'est encore implémenté.
> La phase A (l'arborescence de projet contrôlée, le code généré en bleu et en
> lecture seule, compiler/exécuter/déboguer depuis la barre d'outils et le
> filtre de compilabilité) est construite ; ce document conçoit la couche de
> *collaboration multi-développeur* derrière un **backend enfichable**, afin de
> pouvoir démarrer avec un backend local trivial et grandir vers Google Drive /
> GitHub / git sans réécrire l'IDE.

## 1. Objectifs et non-objectifs

**Objectifs**
- Plusieurs développeurs éditent le même projet, chacun sur sa machine.
- Un fichier en cours d'édition par un développeur est **verrouillé** pour les
  autres : le deuxième développeur est **averti une fois** à l'ouverture et
  obtient le fichier **en lecture seule**.
- Lorsque le premier développeur **libère** un fichier (fermeture de l'éditeur /
  perte du verrou), l'IDE **propose** aux développeurs en attente de le rouvrir
  en lecture/écriture.
- Les modifications qu'un développeur valide sont **propagées** aux autres
  instances de l'IDE avec une promptitude raisonnable.
- Le transport est **enfichable** — local seul, git local, GitHub, Google Drive,
  … — choisi par projet, avec le même comportement d'IDE par-dessus.

**Non-objectifs (explicitement hors périmètre)**
- **La co-édition concurrente au caractère** (façon Google Docs / CRDT). Nous
  utilisons un **verrouillage pessimiste au niveau du fichier** : un seul
  rédacteur par fichier à la fois. Cela correspond exactement à l'exigence
  (« avertir et ne pas autoriser … lecture seule ») et garde la source COBOL
  faisant autorité et compatible avec les diffs.
- Un serveur maison toujours allumé (à moins qu'un futur backend ne choisisse
  d'en ajouter un).

---

## 2. Le backend enfichable — `SyncBackend`

Toute la collaboration passe par un unique trait. Le cœur de l'IDE ne nomme
jamais un service particulier ; le backend est choisi par projet (et rangé dans
`cobolt.toml`).

```rust
/// Identity of a developer in a collaboration session.
pub struct Peer { pub id: String, pub display_name: String }

/// A file lock held by a peer.
pub struct Lock { pub rel_path: String, pub holder: Peer, pub since: SystemTime }

/// Events a backend pushes up to the IDE (lock changes, remote edits, presence).
pub enum SyncEvent {
    LockAcquired(Lock),
    LockReleased { rel_path: String },
    FileChanged  { rel_path: String, by: Peer }, // remote saved a new version
    PeerJoined(Peer),
    PeerLeft(Peer),
    Error(String),
}

pub trait SyncBackend: Send {
    /// Human label + capabilities (does it support real-time push? locking?).
    fn capabilities(&self) -> Capabilities;

    /// Connect / open the shared project. Returns the initial lock table.
    fn connect(&mut self, project: &ProjectRef, me: &Peer) -> Result<Vec<Lock>, SyncError>;

    /// Try to take the write lock for `rel_path`. `Ok(None)` = granted;
    /// `Ok(Some(lock))` = already held by someone else (open read-only).
    fn try_lock(&mut self, rel_path: &str) -> Result<Option<Lock>, SyncError>;

    /// Release a lock we hold (on editor close / explicit unlock / app exit).
    fn release(&mut self, rel_path: &str) -> Result<(), SyncError>;

    /// Publish a new version of a file we hold the lock on.
    fn push_change(&mut self, rel_path: &str, bytes: &[u8]) -> Result<(), SyncError>;

    /// Fetch the latest bytes of a file (to refresh a read-only view).
    fn fetch(&mut self, rel_path: &str) -> Result<Vec<u8>, SyncError>;

    /// Drain backend events since the last poll (non-blocking). Backends that
    /// support push deliver promptly; polling backends synthesise these.
    fn poll(&mut self) -> Vec<SyncEvent>;
}

pub struct Capabilities {
    pub realtime: bool,      // true = push; false = the IDE must poll
    pub locking:  LockKind,  // Native | Advisory | None
    pub auth:     AuthKind,  // None | OAuth | Token | FsPermissions
}
```

- L'IDE ne parle qu'à `SyncBackend` et vide `poll()` à chaque frame dans l'état
  de l'interface.
- Les backends incapables de pousser (git, Drive) implémentent `poll()` en
  interrogeant le distant à intervalle régulier (2–5 s par exemple) et en
  émettant des événements synthétiques.
- `Capabilities` permet à l'interface de s'adapter (afficher par exemple des
  badges « verrouillage indicatif » ou « quasi temps réel ») et de **se dégrader
  proprement** lorsqu'un backend n'offre pas une fonction.

---

## 3. Le modèle de verrouillage et de propagation (indépendant du backend)

Voici le comportement que l'IDE impose par-dessus n'importe quel backend.

### Ouvrir un fichier
1. L'IDE appelle `try_lock(rel)`.
2. `Ok(None)` → ouvrir en **lecture/écriture** ; marquer l'onglet
   « verrouillé par moi ».
3. `Ok(Some(lock))` → **avertir une fois** (« `{file}` est en cours d'édition par
   `{holder}` — ouverture en lecture seule »), ouvrir l'onglet **en lecture
   seule** et retenir que l'on *attend* `rel`.

### Éditer et enregistrer
- Enregistrer un fichier dont on détient le verrou en écriture appelle
  `push_change(rel, bytes)`.
- Le backend propage ; les autres IDE reçoivent `FileChanged` et, s'ils ont le
  fichier ouvert en lecture seule, rafraîchissent la vue (et l'arborescence le
  marque comme mis à jour).

### Libérer
- À la fermeture de l'éditeur, à la sortie de l'application ou sur déverrouillage
  explicite, l'IDE appelle `release(rel)`.
- Les autres IDE reçoivent `LockReleased`. Pour tout développeur *en attente* de
  `rel`, l'IDE affiche une invite : **« `{file}` est maintenant libre —
  l'éditer ? »** → Oui reprend le verrou et bascule l'onglet en lecture/écriture.

### Sûreté en cas de plantage ou de déconnexion
- Les verrous portent un **détenteur et un horodatage** ainsi qu'un **TTL de
  bail**. Le backend (ou l'IDE) expire un verrou périmé passé le TTL, afin qu'un
  éditeur qui a planté ne puisse pas bloquer un fichier pour toujours. (Le code
  généré n'est jamais verrouillable — il est en lecture seule pour tout le
  monde.)

> Le COBOL généré et les actifs sont en lecture seule ou binaires ; seuls
> **Common Code**, **Forms** et **Documentation** participent au verrouillage.

---

## 4. Les quatre backends

Les quatre implémentent le même trait ; ils ne diffèrent que par *l'endroit où
vit le projet de référence* et par *la façon dont voyagent verrous et
modifications*.

| Backend | Projet de référence | Verrouillage | Propagation | Authentification | Notes |
|---------|-------------------|---------|-------------|------|-------|
| **Local seul** | le dossier local | dans le processus uniquement (une machine, plusieurs fenêtres) | directe | aucune | Le choix par défaut trivial. Valide toute l'expérience sans aucune infrastructure ; pas de synchronisation entre machines. |
| **git local** | un dépôt git (éventuellement sur un chemin partagé ou un distant du réseau local) | **refs de verrou indicatives** (une `refs/locks/<path>` ou un fichier `.cobolt/locks/` validé et poussé) | commit + push à l'enregistrement ; fetch au sondage | identifiants ssh/https | Historique familier et auditable ; l'« immédiateté » vaut l'intervalle de sondage. |
| **GitHub** | un dépôt GitHub | une branche ou un fichier de verrou via l'API (ou un registre de verrous fondé sur **GraphQL/Issues**) ; webhooks d'application GitHub optionnels pour la poussée | commits via l'API ; webhook → quasi temps réel, sinon sondage | **OAuth / PAT** | Hébergé, aucune infrastructure à tenir ; soumis à des quotas ; les webhooks exigent un petit relais pour une vraie poussée. |
| **Google Drive** | un dossier Drive | un fichier de verrou (document `<path>.lock`) ou l'API de **restriction de contenu / verrouillage de fichiers** de Drive | envoi d'une nouvelle révision à l'enregistrement ; le **flux de modifications** de Drive au sondage (ou des notifications push) | **OAuth** | Partage facile avec des non-développeurs ; les notifications de Drive donnent un quasi temps réel. |

Conséquences de conception déjà inscrites dans le trait :
- **Le verrouillage est un `LockKind`** parce que git/Drive/GitHub fournissent des
  verrous *indicatifs* (une convention que chacun respecte), non imposés par le
  système. L'IDE traite les verrous indicatifs comme faisant autorité *tant que
  tous les clients sont un IDE PowerRustCOBOL*.
- **La propagation est `realtime` ou sondée** — git est sondé ; Drive et GitHub
  peuvent approcher le temps réel grâce à leurs flux de modifications et à leurs
  webhooks ; local seul est instantané.
- Chaque backend sérialise la table des verrous de la même façon (un petit
  document `locks` en JSON/TOML), si bien que changer de backend ne change pas
  l'IDE.

---

## 5. Où vit l'état

- **`cobolt.toml`** gagne une section `[collaboration]` :
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **Registre des verrous** : un unique petit document que le backend possède
  (`.cobolt/locks.toml` dans le dépôt ou le dossier, ou un enregistrement côté
  API), de la forme :
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identité** : un `Peer { id, display_name }` issu des réglages de l'IDE (et,
  pour les backends OAuth, du compte authentifié).

---

## 6. Points d'intégration côté IDE (la phase A les a déjà préparés)

- Les catégories de l'**arborescence** qui participent au verrouillage sont déjà
  isolées (Forms / Common Code / Documentation), et le **code généré est en
  lecture seule** pour tout le monde — aucun verrou nécessaire.
- L'**éditeur** gère déjà un indicateur `read_only` par onglet (utilisé
  aujourd'hui pour le code généré) ; la couche de collaboration le réemploie pour
  « verrouillé par quelqu'un d'autre », plus un avertissement unique et un badge
  d'onglet (`🔒 by {name}`).
- Un nouveau **`SyncManager`** (qui détient un `Box<dyn SyncBackend>`) appartient
  à l'application et est vidé à chaque frame dans : les états de lecture seule des
  onglets, l'ensemble des avertissements déjà donnés, l'ensemble « en attente »
  (pour l'invite de reproposition) et une liste de présence.

---

## 7. Déploiement par phases

1. **B0 — Backend local seul et toute l'expérience d'usage.** Implémenter
   `SyncBackend`, `SyncManager`, le flux avertir-une-fois / lecture seule /
   reproposition et les badges d'onglet — le tout contre un backend trivial dans
   le processus (plusieurs fenêtres d'IDE sur une machine). Cela démontre le
   modèle sans la moindre infrastructure.
2. **B1 — Backend git local.** Refs de verrou indicatives, commit et push à
   l'enregistrement, fetch au sondage. La première vraie collaboration entre
   machines.
3. **B2 — Backend GitHub.** Dépôt et registre de verrous via l'API ; relais de
   webhooks optionnel pour le quasi temps réel.
4. **B3 — Backend Google Drive.** OAuth, fichiers de verrou et flux de
   modifications de Drive.

Chaque phase est livrable seule ; le comportement de l'IDE est identique dans
toutes.

---

## 8. Questions ouvertes (à trancher avant B1)

- **Expérience d'identité et d'authentification** : comment un développeur
  s'authentifie-t-il sur chaque backend (coller un PAT contre un parcours OAuth
  dans le navigateur), et comment `Peer.id` reste-t-il stable ?
- **Granularité** : uniquement des verrous par fichier, ou verrouiller aussi
  implicitement la sortie générée d'un formulaire lorsque son `.cfrm` est
  verrouillé ? (Recommandation : verrouiller le `.cfrm` ; son `.cbl` généré est
  déjà en lecture seule.)
- **Politique de conflit** lorsque quelqu'un contourne les verrous indicatifs
  (édition hors de l'IDE) : le dernier qui écrit gagne, avec un bandeau visible
  « modifié sur le disque / sur le distant ».
- **Édition hors ligne** : mettre `push_change` en file d'attente et réconcilier
  à la reconnexion, ou bloquer les enregistrements pendant la déconnexion ?

---

## 9. Pourquoi un verrouillage pessimiste (et non un CRDT)

L'exigence est explicite : un second développeur doit être **averti et bloqué**
(lecture seule), non fusionné en direct. Le verrouillage pessimiste au niveau du
fichier :
- correspond exactement à cette exigence,
- garde la source COBOL comme un artefact propre et relisible (de vrais diffs,
  aucune métadonnée de CRDT),
- fonctionne sur *n'importe lequel* des quatre backends avec la même sémantique,
  et
- est radicalement moins complexe et moins risqué que la convergence CRDT en
  temps réel.

Si une véritable co-édition concurrente est un jour souhaitée, ce serait un mode
distinct et additif — elle ne bloque pas cette conception.

.<<

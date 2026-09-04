<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL IDE — Collaboration (phase B) — Conception

> **Statut : conception uniquement.** Rien de tout cela n'est encore implémenté.
> La phase A (l'arborescence de projet contrôlée, le code généré en bleu et en
> lecture seule, les boutons compiler/exécuter/déboguer de la barre d'outils et
> le verrouillage des actions tant que le projet ne compile pas) est construite ;
> ce document conçoit la couche de *collaboration entre plusieurs développeurs*
> derrière un **backend enfichable**, afin de pouvoir démarrer avec un backend
> local trivial et évoluer vers Google Drive / GitHub / git sans réécrire l'IDE.

## 1. Objectifs et non-objectifs

**Objectifs**
- Plusieurs développeurs modifient le même projet, chacun sur sa propre machine.
- Un fichier en cours de modification par un développeur est **verrouillé** pour
  les autres : le second développeur est **averti une seule fois** à l'ouverture
  et obtient le fichier en **lecture seule**.
- Lorsque le premier développeur **libère** un fichier (fermeture de l'éditeur /
  perte du verrou), l'IDE **propose** aux développeurs en attente de le rouvrir
  en lecture/écriture.
- Les modifications qu'un développeur valide sont **propagées** aux autres
  instances de l'IDE dans un délai raisonnable.
- Le transport est **enfichable** — local uniquement, git local, GitHub, Google
  Drive, … choisi projet par projet, avec le même comportement de l'IDE par
  dessus.

**Non-objectifs (explicitement hors périmètre)**
- **La co-édition concurrente au niveau du caractère** (façon Google Docs / CRDT).
  Nous utilisons un **verrouillage pessimiste au niveau du fichier** — un seul
  rédacteur par fichier à la fois. Cela correspond à l'exigence (« avertir et ne
  pas autoriser … lecture seule ») et garde le source COBOL faisant autorité et
  compatible avec les diffs.
- Un serveur dédié toujours actif (à moins qu'un backend futur ne choisisse d'en
  ajouter un).

---

## 2. Le backend enfichable — `SyncBackend`

Toute la collaboration passe par un seul trait. Le cœur de l'IDE ne nomme jamais
un service précis ; le backend est choisi projet par projet (stocké dans
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

- L'IDE ne dialogue qu'avec `SyncBackend` et vide `poll()` à chaque frame dans
  l'état de l'interface.
- Les backends incapables de pousser (git, Drive) implémentent `poll()` en
  interrogeant le dépôt distant à intervalles réguliers (p. ex. 2–5 s) et en
  émettant des événements synthétiques.
- `Capabilities` permet à l'interface de s'adapter (p. ex. afficher les badges
  « verrouillage indicatif » ou « quasi temps réel ») et nous permet de
  **dégrader en douceur** lorsqu'une fonctionnalité manque à un backend.

---

## 3. Le modèle de verrouillage et de propagation (indépendant du backend)

Voici le comportement que l'IDE impose au-dessus de n'importe quel backend.

### Ouvrir un fichier
1. L'IDE appelle `try_lock(rel)`.
2. `Ok(None)` → ouverture en **lecture/écriture** ; l'onglet est marqué
   « verrouillé par moi ».
3. `Ok(Some(lock))` → **avertissement unique** (« `{file}` est en cours de
   modification par `{holder}` — ouverture en lecture seule »), l'onglet s'ouvre
   en **lecture seule**, et l'on retient que l'on est *en attente* de `rel`.

### Modifier et enregistrer
- L'enregistrement d'un fichier verrouillé en écriture appelle
  `push_change(rel, bytes)`.
- Le backend propage ; les autres IDE reçoivent `FileChanged` et, s'ils ont le
  fichier ouvert en lecture seule, rafraîchissent la vue (et l'arborescence le
  signale comme mis à jour).

### Libérer
- À la fermeture de l'éditeur, à la sortie de l'application ou lors d'un
  déverrouillage explicite, l'IDE appelle `release(rel)`.
- Les autres IDE reçoivent `LockReleased`. Pour tout développeur *en attente* de
  `rel`, l'IDE affiche une invite : **« `{file}` est maintenant libre — le
  modifier ? »** → Oui réacquiert le verrou et bascule l'onglet en
  lecture/écriture.

### Sûreté en cas de plantage ou de déconnexion
- Les verrous portent un **détenteur et un horodatage** ainsi qu'une **durée de
  bail (TTL)**. Un backend (ou l'IDE lui-même) fait expirer un verrou périmé
  passé le TTL, afin qu'un éditeur planté ne puisse pas bloquer un fichier
  indéfiniment. (Le code généré n'est jamais verrouillable — il est en lecture
  seule pour tout le monde.)

> Le COBOL généré et les Assets sont en lecture seule ou binaires ; seuls
> **Common Code**, **Forms** et **Documentation** participent au verrouillage.

---

## 4. Les quatre backends

Les quatre implémentent le même trait ; ils ne diffèrent que par *l'endroit où
vit le projet de référence* et *la manière dont voyagent les verrous et les
modifications*.

| Backend | Projet de référence | Verrouillage | Propagation | Authentification | Remarques |
|---------|---------------------|--------------|-------------|------------------|-----------|
| **Local uniquement** | le dossier local | dans le processus seulement (une machine, plusieurs fenêtres) | directe | aucune | Le choix par défaut, trivial. Il valide toute l'expérience sans la moindre infrastructure ; aucune synchronisation entre machines. |
| **git local** | un dépôt git (éventuellement sur un chemin partagé ou un dépôt distant en LAN) | **refs de verrou indicatives** (un `refs/locks/<path>` ou un fichier `.cobolt/locks/` commité et poussé) | commit + push à l'enregistrement ; fetch à chaque sondage | identifiants ssh/https | Historique familier et auditable ; l'« immédiateté » vaut l'intervalle de sondage. |
| **GitHub** | un dépôt GitHub | une branche ou un fichier de verrou via l'API (ou un registre de verrous fondé sur **GraphQL/Issues**) ; webhooks facultatifs d'une GitHub App pour le push | commits via l'API ; webhook → quasi temps réel, sinon sondage | **OAuth / PAT** | Hébergé, aucune infrastructure à exploiter ; soumis à des quotas ; les webhooks exigent un petit relais pour un vrai push. |
| **Google Drive** | un dossier Drive | un fichier de verrou (document `<path>.lock`) ou l'API de **restriction de contenu / verrouillage de fichiers** de Drive | envoi d'une nouvelle révision à l'enregistrement ; **flux de modifications** de Drive à chaque sondage (ou notifications push) | **OAuth** | Partage facile pour les non-développeurs ; les notifications de modification de Drive donnent du quasi temps réel. |

Les implications de conception intégrées au trait :
- **Le verrouillage est un `LockKind`** parce que git, Drive et GitHub offrent
  des verrous *indicatifs* (une convention que tout le monde respecte) et non des
  verrous imposés par le système d'exploitation. L'IDE traite les verrous
  indicatifs comme faisant autorité *tant que tous les clients sont l'IDE
  PowerRustCOBOL*.
- **La propagation est `realtime` ou sondée** — git est sondé ; Drive et GitHub
  peuvent approcher le temps réel avec leurs flux de modifications et leurs
  webhooks ; le mode local uniquement est instantané.
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
- **Identité** : un `Peer { id, display_name }` issu des paramètres de l'IDE
  (et, pour les backends OAuth, du compte authentifié).

---

## 6. Points d'intégration côté IDE (déjà préparés par la phase A)

- Les catégories de l'**arborescence** qui participent au verrouillage sont déjà
  isolées (Forms / Common Code / Documentation), et **le code généré est en
  lecture seule** pour tout le monde — aucun verrou nécessaire.
- L'**éditeur** prend déjà en charge un indicateur `read_only` par onglet
  (utilisé aujourd'hui pour le code généré) ; la couche de collaboration le
  réutilise pour « verrouillé par quelqu'un d'autre », plus un avertissement
  unique et un badge d'onglet (`🔒 by {name}`).
- Un nouveau **`SyncManager`** (qui détient une `Box<dyn SyncBackend>`)
  appartient à l'application et est vidé à chaque frame dans : les états de
  lecture seule des onglets, l'ensemble des avertissements déjà émis, l'ensemble
  « en attente » (pour l'invite de nouvelle proposition) et une liste de
  présence.

---

## 7. Déploiement par phases

1. **B0 — Backend local uniquement et toute l'expérience utilisateur.**
   Implémenter `SyncBackend`, `SyncManager`, le flux avertissement unique /
   lecture seule / nouvelle proposition, les badges d'onglet — le tout face à un
   backend trivial dans le processus (plusieurs fenêtres de l'IDE sur une même
   machine). Cela valide le modèle sans aucune infrastructure.
2. **B1 — Backend git local.** Refs de verrou indicatives + commit et push à
   l'enregistrement + fetch au sondage. La première vraie collaboration entre
   machines.
3. **B2 — Backend GitHub.** Dépôt et registre de verrous via l'API ; relais de
   webhooks facultatif pour du quasi temps réel.
4. **B3 — Backend Google Drive.** OAuth + fichiers de verrou + flux de
   modifications de Drive.

Chaque phase est livrable seule ; le comportement de l'IDE est identique dans
toutes.

---

## 8. Questions ouvertes (à trancher avant B1)

- **Expérience d'identité et d'authentification** : comment les développeurs se
  connectent-ils à chaque backend (coller un PAT face à un parcours OAuth dans le
  navigateur), et comment `Peer.id` reste-t-il stable ?
- **Granularité** : uniquement des verrous au niveau du fichier, ou verrouiller
  aussi implicitement la sortie générée d'un formulaire lorsque son `.cfrm` est
  verrouillé ? (Recommandation : verrouiller le `.cfrm` ; son `.cbl` généré est
  déjà en lecture seule.)
- **Politique de conflit** lorsque les verrous indicatifs sont contournés
  (quelqu'un modifie en dehors de l'IDE) : le dernier qui écrit gagne, avec une
  bannière visible « modifié sur le disque ou à distance ».
- **Édition hors connexion** : mettre `push_change` en file d'attente et
  réconcilier à la reconnexion, ou bloquer les enregistrements tant que la
  connexion est coupée ?

---

## 9. Pourquoi un verrouillage pessimiste (et non un CRDT)

L'exigence est explicite : un second développeur doit être **averti et bloqué**
(lecture seule), et non fusionné en direct. Le verrouillage pessimiste au niveau
du fichier :
- répond exactement à cette exigence,
- garde le source COBOL comme un artefact propre et relisible (de vrais diffs,
  aucune métadonnée CRDT),
- fonctionne sur *n'importe lequel* des quatre backends avec la même sémantique,
  et
- est radicalement moins complexe et moins risqué que la convergence CRDT en
  temps réel.

Si une véritable co-édition concurrente devenait souhaitable un jour, ce serait un
mode distinct et additif — cela ne bloque pas cette conception.

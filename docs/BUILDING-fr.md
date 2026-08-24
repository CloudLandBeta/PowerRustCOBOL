<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compiler PowerRustCOBOL

D'une machine vierge à un IDE qui tourne, sous **Windows**, **Linux** et
**macOS**.

Tout ce qui suit tient dans les mêmes trois étapes sur toutes les plateformes —
installer une chaîne d'outils, cloner, `cargo build`. Seule la première étape
diffère selon le système d'exploitation.

---

## Ce que la compilation exige

| Prérequis | Pourquoi |
|---|---|
| **Rust**, canal stable, **1.92 ou plus récent** | compile l'ensemble du workspace |
| **Git** | clone le dépôt |
| **Un compilateur C et un éditeur de liens** | l'éditeur de liens dont Rust a besoin pour *n'importe quel* binaire, plus deux dépendances en C |
| **Bibliothèques graphiques natives** (Linux uniquement) | création des fenêtres et boîtes de dialogue de fichiers natives |

> **L'IDE empaqueté vérifie lui-même le prérequis Rust.** Celui qui *utilise*
> PowerRustCOBOL au lieu de le compiler ne lit jamais cette page : l'IDE cherche
> donc Rust au premier lancement et propose de l'installer lorsque ce même
> minimum de **1.92** n'est pas atteint. Il lit le numéro dans le manifeste de
> ce workspace, si bien que les deux ne peuvent pas diverger. Voir le §3 du
> Guide du Développeur.

### À propos du compilateur C

Deux crates de l'arborescence compilent du code C ; un compilateur C est donc
réellement nécessaire :

- **`libsqlite3-sys`** — SQLite, intégré depuis son amalgame en C. C'est le
  support SQLite du runtime de bases de données COBOL : aucun SQLite système ne
  doit être installé ni aligné en version sur la machine de l'utilisateur final.
- **`onig_sys`** — le moteur d'expressions régulières Oniguruma, qu'utilise le
  tokeniseur derrière la recherche sémantique.

Ce dont la compilation **n'a pas** besoin, et qu'elle n'invoque jamais :

> **aucun compilateur C++ · aucun CMake · aucun NASM · aucun Python · aucun
> Node · aucune JVM**

C'est délibéré et cela le reste. TLS passe par la pile du système d'exploitation
lui-même (schannel sous Windows, Security.framework sous macOS, OpenSSL sous
Linux) via des liaisons purement Rust, plutôt que par une bibliothèque
cryptographique embarquée qui réclamerait C, de l'assembleur et CMake sur chaque
machine ; le suffix-array C++ du tokeniseur (`esaxx_fast`) est désactivé parce
que rien ici n'entraîne de modèle ; et l'index de la Base de Connaissances est
`redb`, du Rust pur.

Sur toutes les plateformes, le compilateur C arrive dans le même paquet que
l'éditeur de liens déjà exigé par Rust : en pratique, cela n'ajoute donc rien à
installer.

---

## 1. Installez la chaîne d'outils

### Windows

1. Installez les **Visual Studio Build Tools** avec la charge de travail
   **« Desktop development with C++ »** —
   [téléchargement](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La charge de travail porte le nom de C++, mais ce qu'elle livre est ce dont
   toute compilation Rust sous Windows a besoin de toute façon : `link.exe`, le
   Windows SDK et `cl.exe` pour les deux dépendances en C ci-dessus. Il n'y a
   rien d'autre à télécharger.

2. Installez Rust depuis [rustup.rs](https://rustup.rs). Il sélectionne
   automatiquement la chaîne d'outils MSVC.

3. Vérifiez, depuis une invite PowerShell ordinaire :

   ```powershell
   rustc --version
   cargo --version
   ```

Aucun paramètre d'édition de liens à régler à la main : le
`.cargo/config.toml` du dépôt place déjà tous les objets sur le CRT dynamique,
ce qui empêche les dépendances en C et le runtime de Rust d'entrer en collision
à l'édition de liens.

### macOS

Installez les Xcode Command Line Tools — c'est tout :

```sh
xcode-select --install
```

Puis Rust :

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon et Intel sont tous deux pris en charge ; rustup choisit la bonne
cible pour l'hôte.

### Linux

**Debian / Ubuntu :**

```sh
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libssl-dev
```

**Fedora / RHEL :**

```sh
sudo dnf install -y @development-tools pkgconf-pkg-config \
    gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel
```

**Arch :**

```sh
sudo pacman -S --needed base-devel pkgconf gtk3 libxcb libxkbcommon openssl
```

Puis Rust :

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Deux de ces paquets sont porteurs et méritent d'être nommés :

- **`libssl-dev` / `openssl-devel`** — sous Linux, HTTPS utilise le TLS du
  système, et c'est celui-ci.
- **`libgtk-3-dev` / `gtk3-devel`** — les boîtes de dialogue natives
  d'ouverture et d'enregistrement.

X11 et Wayland sont tous deux pris en charge ; la couche fenêtrage retient la
session en cours, aucun des deux n'est donc une installation séparée.

---

## 2. Récupérez le code

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compilez

```sh
cargo build
```

> La première compilation télécharge tous les crates et compile le workspace :
> comptez quelques minutes et un cache `target/` d'environ 1,5 Go. Les
> compilations suivantes sont incrémentales. `cargo clean` récupère l'espace dès
> que vous le voulez.

Pour ne compiler que les deux choses que l'on exécute :

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Lancez l'IDE

```sh
cargo run -p cobolt-ide
```

Au quotidien, préférez une compilation release — plus lente à compiler une fois,
bien plus fluide à l'usage :

```sh
cargo run --release -p cobolt-ide
```

---

## Exécuter les tests

```sh
cargo test --workspace
```

Le moteur de formulaires a besoin de sa feature `render` pour tester les chemins
de rendu :

```sh
cargo test -p cobolt-forms --features render
```

---

## Où atterrissent les artefacts

| Artefact | Chemin |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` sous Windows) |
| Runtime / compilateur en ligne de commande | `target/release/rcrun` (`.exe` sous Windows) |
| Une application que **vous** compilez à partir d'un projet | `<project>/bin/` et le dossier de destination du projet |

Une application compilée avec `rcrun build` est un exécutable autonome unique :
elle embarque son programme compilé, ses formulaires et tout thème d'asset pack
qu'ils utilisent, de sorte qu'il n'y a rien à installer à côté d'elle sur la
machine à laquelle vous la remettez.

---

## Dépannage

**`linker 'cc' not found` (Linux)** — `build-essential` (ou
`@development-tools`) est absent.

**`link.exe not found` (Windows)** — les Build Tools ont été installés sans la
charge de travail « Desktop development with C++ ». Relancez l'installeur et
cochez-la.

**`Could not find directory of OpenSSL installation` (Linux)** — installez
`libssl-dev` / `openssl-devel` et `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**L'IDE compile mais aucune fenêtre ne s'ouvre (Linux)** — vérifiez que
`libxkbcommon-dev` est installé et que `$DISPLAY` ou `$WAYLAND_DISPLAY` est
défini ; un TTY nu ou une session SSH sans redirection X n'a aucun affichage sur
lequel s'ouvrir.

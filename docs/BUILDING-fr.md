<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compiler PowerRustCOBOL

D'une machine vierge à un IDE qui tourne, sous **Windows**, **Linux** et
**macOS**.

Tout ce qui suit tient dans les trois mêmes étapes sur chaque plateforme :
installer une chaîne d'outils, cloner, `cargo build`. Seule la première étape
diffère selon le système.

---

## Ce dont la compilation a besoin

| Prérequis | Pourquoi |
|---|---|
| **Rust**, canal stable, **1.92 ou plus récent** | compile l'ensemble du workspace |
| **Git** | clone le dépôt |
| **Un compilateur C et un éditeur de liens** | l'éditeur de liens dont Rust a besoin pour *tout* binaire, plus deux dépendances en C |
| **Bibliothèques GUI natives** (Linux uniquement) | création des fenêtres et boîtes de dialogue de fichiers natives |

### À propos du compilateur C

Deux crates de l'arborescence compilent du code C, un compilateur C est donc
réellement nécessaire :

- **`libsqlite3-sys`** — SQLite, intégré à partir de son amalgame en C. C'est le
  support SQLite du runtime de bases de données COBOL, de sorte qu'aucun SQLite
  système n'a à être installé ni mis en correspondance de version sur la machine
  de l'utilisateur final.
- **`onig_sys`** — le moteur d'expressions régulières Oniguruma, qu'utilise le
  tokeniseur situé derrière la recherche sémantique.

Ce dont la compilation n'a **pas** besoin, et qu'elle n'invoque jamais :

> **aucun compilateur C++ · aucun CMake · aucun NASM · aucun Python · aucun Node · aucune JVM**

C'est délibéré et c'est maintenu ainsi. TLS passe par la pile propre du système
d'exploitation (schannel sous Windows, Security.framework sous macOS, OpenSSL
sous Linux) via des liaisons en Rust pur, plutôt que par une bibliothèque
cryptographique embarquée qui exigerait C, de l'assembleur et CMake sur chaque
machine ; le tableau de suffixes en C++ du tokeniseur (`esaxx_fast`) est
désactivé parce que rien ici n'entraîne de modèle ; et l'index de la Knowledge
Base, c'est `redb`, en Rust pur.

Sur chaque plateforme, le compilateur C arrive dans le même paquet que celui qui
fournit l'éditeur de liens que Rust exige déjà : en pratique, cela n'ajoute donc
rien à installer.

---

## 1. Installer la chaîne d'outils

### Windows

1. Installez les **Visual Studio Build Tools** avec la charge de travail
   **« Desktop development with C++ »** —
   [téléchargement](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La charge de travail porte le nom de C++, mais ce qu'elle livre est
   exactement ce dont toute compilation Rust sous Windows a besoin de toute
   façon : `link.exe`, le SDK Windows et `cl.exe` pour les deux dépendances en C
   ci-dessus. Il n'y a rien d'autre à télécharger.

2. Installez Rust depuis [rustup.rs](https://rustup.rs). Il sélectionne la
   chaîne d'outils MSVC automatiquement.

3. Vérifiez, depuis une invite PowerShell ordinaire :

   ```powershell
   rustc --version
   cargo --version
   ```

Aucun drapeau d'édition de liens à régler à la main : le `.cargo/config.toml` du
dépôt place déjà chaque objet sur le CRT dynamique, ce qui empêche les
dépendances en C et le runtime propre de Rust d'entrer en collision à l'édition
de liens.

### macOS

Installez les Xcode Command Line Tools — c'est tout :

```sh
xcode-select --install
```

Puis Rust :

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon et Intel sont tous deux pris en charge ; rustup choisit la cible
hôte adéquate.

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
  système, et le voici.
- **`libgtk-3-dev` / `gtk3-devel`** — les boîtes de dialogue natives Ouvrir et
  Enregistrer.

X11 et Wayland sont tous deux pris en charge ; la couche fenêtrage choisit la
session en cours, aucune des deux n'est donc une installation distincte.

---

## 2. Récupérer le code

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compiler

```sh
cargo build
```

> La première compilation récupère chaque crate et compile le workspace :
> comptez quelques minutes et un cache `target/` d'environ 1,5 Go. Les
> compilations suivantes sont incrémentales. `cargo clean` récupère l'espace dès
> que vous le souhaitez.

Pour ne compiler que les deux choses que vous exécutez :

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Lancer l'IDE

```sh
cargo run -p cobolt-ide
```

Au quotidien, préférez une compilation release — plus longue à compiler une
fois, bien plus agréable à utiliser :

```sh
cargo run --release -p cobolt-ide
```

---

## Exécuter les tests

```sh
cargo test --workspace
```

Le moteur de forms a besoin de sa fonctionnalité `render` pour tester les
chemins de rendu :

```sh
cargo test -p cobolt-forms --features render
```

---

## Où atterrissent les artefacts

| Artefact | Chemin |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` sous Windows) |
| Runtime / compilateur en ligne de commande | `target/release/rcrun` (`.exe` sous Windows) |
| Une application que **vous** compilez depuis un project | `<project>/bin/` et le dossier de destination du project |

Une application compilée avec `rcrun build` est un exécutable unique et
autonome : il embarque son programme compilé, ses forms et tout thème
d'asset-pack qu'ils utilisent, si bien qu'il n'y a rien à installer à côté de
lui sur la machine à laquelle vous le remettez.

---

## Dépannage

**`linker 'cc' not found` (Linux)** — `build-essential` (ou
`@development-tools`) est absent.

**`link.exe not found` (Windows)** — les Build Tools ont été installés sans la
charge de travail « Desktop development with C++ ». Relancez l'installateur et
cochez-la.

**`Could not find directory of OpenSSL installation` (Linux)** — installez
`libssl-dev` / `openssl-devel` et `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**L'IDE se compile mais aucune fenêtre ne s'ouvre (Linux)** — vérifiez que
`libxkbcommon-dev` est installé et que `$DISPLAY` ou `$WAYLAND_DISPLAY` est
défini ; un simple TTY ou une session SSH sans redirection X n'a aucun écran sur
lequel s'ouvrir.

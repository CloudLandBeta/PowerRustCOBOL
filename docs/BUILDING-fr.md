<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compiler PowerRustCOBOL

D'une machine vierge à un IDE qui tourne, sous **Windows**, **Linux** et
**macOS**.

Tout ce qui suit tient dans les mêmes trois étapes sur chaque plateforme —
installer une chaîne d'outils, cloner, `cargo build`. Seule la première étape
change selon le système d'exploitation.

---

## Ce dont la compilation a besoin

| Prérequis | Pourquoi |
|---|---|
| **Rust**, canal stable, **1.92 ou plus récent** | compile tout le workspace |
| **Git** | clone le dépôt |
| **Un compilateur C et un éditeur de liens** | l'éditeur de liens dont Rust a besoin pour *n'importe quel* binaire, plus deux dépendances en C |
| **Bibliothèques GUI natives** (Linux seulement) | la création de fenêtres et les boîtes de dialogue de fichiers natives |

> **L'IDE empaqueté vérifie lui-même le prérequis Rust.** Celui qui *utilise*
> PowerRustCOBOL au lieu de le compiler ne lit jamais cette page : l'IDE cherche
> donc Rust à son premier lancement et propose de l'installer quand ce même
> minimum de **1.92** n'est pas atteint. Il lit le numéro dans le manifeste de ce
> workspace, si bien que les deux ne peuvent pas diverger. Voir le §3 du Guide du
> développeur.

### À propos du compilateur C

Deux crates de l'arborescence compilent du code C, un compilateur C est donc
réellement indispensable :

- **`libsqlite3-sys`** — SQLite, embarqué depuis son amalgame C. C'est le
  support SQLite du runtime de bases de données COBOL, de sorte qu'aucun SQLite
  système n'a besoin d'être installé ni accordé en version sur la machine de
  l'utilisateur final.
- **`onig_sys`** — le moteur d'expressions régulières Oniguruma, qu'utilise le
  tokeniseur derrière la recherche sémantique.

Ce dont la compilation n'a **pas** besoin, et qu'elle n'invoque jamais :

> **pas de compilateur C++ · pas de CMake · pas de NASM · pas de Python · pas de Node · pas de JVM**

C'est délibéré et cela reste ainsi. TLS passe par la pile du système
d'exploitation lui-même (schannel sous Windows, Security.framework sous macOS,
OpenSSL sous Linux) via des liaisons en Rust pur, plutôt que par une
bibliothèque cryptographique embarquée qui réclamerait C, de l'assembleur et
CMake sur chaque machine ; le tableau de suffixes C++ du tokeniseur
(`esaxx_fast`) est désactivé parce que rien ici n'entraîne de modèle ; et
l'index de la base de connaissances est `redb`, en Rust pur.

Sur chaque plateforme, le compilateur C arrive dans le paquet même qui fournit
l'éditeur de liens que Rust exige déjà : en pratique, cela n'ajoute donc rien à
installer.

---

## 1. Installer la chaîne d'outils

### Windows

1. Installez les **Visual Studio Build Tools** avec la charge de travail
   **« Desktop development with C++ »** —
   [téléchargement](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La charge de travail porte le nom du C++, mais ce qu'elle livre est ce dont
   toute compilation Rust sous Windows a de toute façon besoin : `link.exe`, le
   Windows SDK et `cl.exe` pour les deux dépendances en C ci-dessus. Il n'y a
   rien d'autre à télécharger.

2. Installez Rust depuis [rustup.rs](https://rustup.rs). Il sélectionne
   automatiquement la chaîne d'outils MSVC.

3. Vérifiez, depuis une invite PowerShell ordinaire :

   ```powershell
   rustc --version
   cargo --version
   ```

Aucune option d'édition de liens à poser à la main : le `.cargo/config.toml` du
dépôt place déjà chaque objet sur le CRT dynamique, et c'est ce qui empêche les
dépendances en C et le runtime de Rust lui-même d'entrer en collision à l'édition
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

Apple Silicon et Intel sont tous deux pris en charge ; rustup choisit la bonne
cible hôte.

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

- **`libssl-dev` / `openssl-devel`** — HTTPS utilise le TLS du système sous
  Linux, et le voici.
- **`libgtk-3-dev` / `gtk3-devel`** — les boîtes de dialogue natives
  Ouvrir/Enregistrer.

X11 et Wayland sont tous deux pris en charge ; la couche fenêtrage retient la
session qui tourne, aucun des deux n'est donc une installation séparée.

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
> que vous le voulez.

Pour ne compiler que les deux choses que vous exécutez :

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Lancer l'IDE

```sh
cargo run -p cobolt-ide
```

Au quotidien, préférez une compilation release — plus lente à compiler une fois,
bien plus fluide à l'usage :

```sh
cargo run --release -p cobolt-ide
```

---

## Lancer les tests

```sh
cargo test --workspace
```

Le moteur de formulaires a besoin de sa fonctionnalité `render` pour tester les
chemins de rendu :

```sh
cargo test -p cobolt-forms --features render
```

---

## Où atterrissent les artefacts

| Artefact | Chemin |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` sous Windows) |
| Runtime / constructeur en ligne de commande | `target/release/rcrun` (`.exe` sous Windows) |
| Une application que **vous** compilez à partir d'un projet | `<project>/bin/` et le dossier de destination du projet |

Une application compilée avec `rcrun build` est un exécutable unique et
autonome : elle embarque son programme compilé, ses formulaires et le thème de
pack d'assets qu'ils utilisent éventuellement, si bien qu'il n'y a rien à
installer à côté d'elle sur la machine à laquelle vous la remettez.

---

## Installer l'IDE ailleurs — emportez le SDK de la plateforme

L'exécutable de l'IDE n'est **pas** autonome comme l'est une application que
vous compilez. Compiler une application lance un vrai `cargo build` contre les
sources Rust de la plateforme : ces sources doivent donc exister sur la machine
qui compile. Copiez `cobolt-ide` tout seul quelque part et Build échoue, en
nommant chaque dossier où il a regardé — la chaîne d'outils va bien, ce sont les
sources qui sont simplement absentes.

Déposez-les à côté de l'exécutable. Depuis l'arborescence source :

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

Cela écrit `Cargo.toml` et `crates/` dans `<install-dir>` — 6,0 Mo, les dix
crates contre lesquels une application compilée se construit. Passez `--sdk`
pour les mettre dans `<install-dir>/sdk/` quand le dossier d'installation
contient autre chose. L'IDE trouve l'une comme l'autre disposition sans aucune
configuration, et regarde aussi un niveau au-dessus et, sous macOS, dans les
`Resources` du bundle.

La machine a toujours besoin de la chaîne d'outils Rust — Build est une vraie
compilation — et sa première compilation télécharge les crates de dépendances
depuis le registre : il lui faut donc un accès réseau une fois.

> **Note.** Pour une copie de travail qui vit entièrement ailleurs, réglez le
> dossier à la main sous **Help → Platform SDK Location**. Il est mémorisé par
> machine et non par projet, il ne voyage donc jamais jusqu'à un collègue dans
> `cobolt.toml`. Laissez-le vide pour revenir à la recherche automatique.

---

## Dépannage

**`linker 'cc' not found` (Linux)** — `build-essential` (ou
`@development-tools`) manque.

**`link.exe not found` (Windows)** — les Build Tools ont été installés sans la
charge de travail « Desktop development with C++ ». Relancez l'installateur et
cochez-la.

**`Could not find directory of OpenSSL installation` (Linux)** — installez
`libssl-dev` / `openssl-devel` et `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**L'IDE compile mais aucune fenêtre ne s'ouvre (Linux)** — vérifiez que
`libxkbcommon-dev` est installé et que `$DISPLAY` ou `$WAYLAND_DISPLAY` est
défini ; un TTY nu ou une session SSH sans redirection X n'a aucun affichage sur
lequel s'ouvrir.

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilar PowerRustCOBOL

De una máquina limpia a un IDE en marcha, en **Windows**, **Linux** y **macOS**.

Todo lo que hay aquí son los mismos tres pasos en cualquier plataforma —
instalar un toolchain, clonar, `cargo build`. Solo el primer paso cambia según
el sistema operativo.

---

## Qué necesita la compilación

| Requisito | Por qué |
|---|---|
| **Rust**, canal stable, **1.92 o posterior** | compila todo el workspace |
| **Git** | clona el repositorio |
| **Un compilador de C y un enlazador** | el enlazador que Rust necesita para *cualquier* binario, más dos dependencias en C |
| **Bibliotecas gráficas nativas** (solo Linux) | creación de ventanas y los diálogos de archivo nativos |

> **El IDE empaquetado comprueba por sí mismo el requisito de Rust.** Quien
> *usa* PowerRustCOBOL en lugar de compilarlo nunca lee esta página, así que el
> IDE busca Rust en su primera ejecución y se ofrece a instalarlo cuando no se
> cumple este mismo mínimo de **1.92**. Lee el número del propio manifiesto de
> este workspace, de modo que ambos no pueden discrepar. Véase la §3 de la Guía
> del Desarrollador.

### Sobre el compilador de C

Dos crates del árbol compilan código C, así que un compilador de C es realmente
necesario:

- **`libsqlite3-sys`** — SQLite, incorporado desde su amalgama en C. Es el
  soporte de SQLite del runtime de bases de datos COBOL, de manera que no hace
  falta instalar ni hacer coincidir la versión de ningún SQLite del sistema en
  la máquina del usuario final.
- **`onig_sys`** — el motor de expresiones regulares Oniguruma, que usa el
  tokenizador que hay detrás de la búsqueda semántica.

Lo que la compilación **no** necesita, y nunca invoca:

> **ningún compilador C++ · ningún CMake · ningún NASM · ningún Python · ningún
> Node · ninguna JVM**

Es deliberado y se mantiene así. TLS pasa por la pila del propio sistema
operativo (schannel en Windows, Security.framework en macOS, OpenSSL en Linux)
mediante bindings puramente en Rust, en lugar de una biblioteca criptográfica
incorporada que exigiría C, ensamblador y CMake en cada máquina; el suffix-array
en C++ del tokenizador (`esaxx_fast`) está desactivado porque aquí no se entrena
ningún modelo; y el índice de la Base de Conocimiento es `redb`, Rust puro.

En todas las plataformas el compilador de C llega dentro del mismo paquete que
proporciona el enlazador que Rust ya exige, así que en la práctica esto no añade
nada que instalar.

---

## 1. Instale el toolchain

### Windows

1. Instale las **Visual Studio Build Tools** con la carga de trabajo
   **"Desktop development with C++"** —
   [descarga](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La carga de trabajo lleva el nombre de C++, pero lo que entrega es lo que
   toda compilación de Rust en Windows necesita de todos modos: `link.exe`, el
   Windows SDK y `cl.exe` para las dos dependencias en C citadas arriba. No hay
   nada más que descargar.

2. Instale Rust desde [rustup.rs](https://rustup.rs). Selecciona el toolchain
   MSVC automáticamente.

3. Verifique, desde un símbolo del sistema normal de PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

No hay flags de enlazado que ajustar a mano: el `.cargo/config.toml` del
repositorio ya coloca todos los objetos sobre el CRT dinámico, que es lo que
impide que las dependencias en C y el propio runtime de Rust choquen al enlazar.

### macOS

Instale las Xcode Command Line Tools — eso es todo:

```sh
xcode-select --install
```

Después, Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel están ambos soportados; rustup elige el target correcto
para el host.

### Linux

**Debian / Ubuntu:**

```sh
sudo apt update && sudo apt install -y \
    build-essential pkg-config \
    libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libssl-dev
```

**Fedora / RHEL:**

```sh
sudo dnf install -y @development-tools pkgconf-pkg-config \
    gtk3-devel libxcb-devel libxkbcommon-devel openssl-devel
```

**Arch:**

```sh
sudo pacman -S --needed base-devel pkgconf gtk3 libxcb libxkbcommon openssl
```

Después, Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Dos de esos paquetes son estructurales y merece la pena nombrarlos:

- **`libssl-dev` / `openssl-devel`** — en Linux HTTPS usa el TLS del sistema, y
  es esto.
- **`libgtk-3-dev` / `gtk3-devel`** — los diálogos nativos de abrir/guardar.

X11 y Wayland están ambos soportados; la capa de ventanas elige la sesión que
esté en marcha, así que ninguno de los dos es una instalación aparte.

---

## 2. Obtenga el código

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compile

```sh
cargo build
```

> La primera compilación descarga todos los crates y compila el workspace, así
> que cuente con unos minutos y una caché `target/` de alrededor de 1,5 GB. Las
> compilaciones posteriores son incrementales. `cargo clean` recupera el espacio
> cuando quiera tenerlo de vuelta.

Para compilar solo las dos cosas que se ejecutan:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Inicie el IDE

```sh
cargo run -p cobolt-ide
```

Para el uso diario prefiera una compilación release — más lenta de compilar una
vez, mucho más fluida de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Ejecutar las pruebas

```sh
cargo test --workspace
```

El motor de formularios necesita su feature `render` para probar los caminos de
renderizado:

```sh
cargo test -p cobolt-forms --features render
```

---

## Dónde quedan los artefactos

| Artefacto | Ruta |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` en Windows) |
| Runtime / compilador de línea de comandos | `target/release/rcrun` (`.exe` en Windows) |
| Una aplicación que **usted** compila a partir de un proyecto | `<project>/bin/` y la carpeta de destino del proyecto |

Una aplicación compilada con `rcrun build` es un único ejecutable autónomo:
incorpora su programa compilado, sus formularios y cualquier tema de asset pack
que usen, de modo que no hay nada que instalar junto a él en la máquina a la que
se lo entregue.

---

## Resolución de problemas

**`linker 'cc' not found` (Linux)** — falta `build-essential` (o
`@development-tools`).

**`link.exe not found` (Windows)** — las Build Tools se instalaron sin la carga
de trabajo "Desktop development with C++". Vuelva a ejecutar el instalador y
márquela.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` y `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**El IDE compila pero no se abre ninguna ventana (Linux)** — compruebe que
`libxkbcommon-dev` está instalado y que `$DISPLAY` o `$WAYLAND_DISPLAY` está
definido; un TTY pelado o una sesión SSH sin reenvío de X no tiene display donde
abrirse.

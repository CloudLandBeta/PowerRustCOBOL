<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilar PowerRustCOBOL

De una máquina limpia a una IDE en marcha, en **Windows**, **Linux** y **macOS**.

Todo lo que hay aquí son los mismos tres pasos en cualquier plataforma: instalar
un toolchain, clonar y `cargo build`. Sólo el primer paso cambia según el
sistema operativo.

---

## Qué necesita la compilación

| Requisito | Para qué |
|---|---|
| **Rust**, canal stable, **1.92 o posterior** | compila todo el workspace |
| **Git** | clona el repositorio |
| **Un compilador de C y un enlazador** | el enlazador que Rust necesita para *cualquier* binario, más dos dependencias en C |
| **Bibliotecas GUI nativas** (sólo Linux) | creación de ventanas y diálogos de fichero nativos |

### Sobre el compilador de C

Dos crates del árbol compilan código C, así que un compilador de C es realmente
imprescindible:

- **`libsqlite3-sys`** — SQLite, incluido a partir de su amalgama en C. Éste es
  el soporte de SQLite del runtime de bases de datos de COBOL, de modo que no hay
  que instalar ni hacer coincidir versiones de ningún SQLite del sistema en la
  máquina del usuario final.
- **`onig_sys`** — el motor de expresiones regulares Oniguruma, que usa el
  tokenizador que hay detrás de la búsqueda semántica.

Lo que la compilación **no** necesita, y nunca invoca:

> **ningún compilador de C++ · ningún CMake · ningún NASM · ningún Python · ningún Node · ninguna JVM**

Eso es deliberado y se mantiene así. TLS pasa por la pila propia del sistema
operativo (schannel en Windows, Security.framework en macOS, OpenSSL en Linux)
mediante enlaces en Rust puro, en lugar de una biblioteca criptográfica incluida
que exigiría C, ensamblador y CMake en cada máquina; el array de sufijos en C++
del tokenizador (`esaxx_fast`) está desactivado porque aquí nada entrena un
modelo; y el índice de la Knowledge Base es `redb`, Rust puro.

En todas las plataformas el compilador de C llega dentro del mismo paquete que
proporciona el enlazador que Rust ya exige, así que en la práctica esto no añade
nada que instalar.

---

## 1. Instalar el toolchain

### Windows

1. Instale las **Visual Studio Build Tools** con la carga de trabajo
   **«Desktop development with C++»** —
   [descarga](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La carga de trabajo lleva el nombre de C++, pero lo que entrega es
   exactamente lo que toda compilación de Rust en Windows necesita de todos
   modos: `link.exe`, el SDK de Windows y `cl.exe` para las dos dependencias en
   C mencionadas arriba. No hay nada más que descargar.

2. Instale Rust desde [rustup.rs](https://rustup.rs). Selecciona el toolchain
   MSVC automáticamente.

3. Verifique, desde un símbolo del sistema normal de PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

No hay que fijar flags del enlazador a mano: el `.cargo/config.toml` del
repositorio ya coloca todos los objetos sobre el CRT dinámico, que es lo que
evita que las dependencias en C y el propio runtime de Rust choquen al enlazar.

### macOS

Instale las Xcode Command Line Tools — eso es todo:

```sh
xcode-select --install
```

Después Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel están ambos soportados; rustup elige el target de host
correcto.

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

Después Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Dos de esos paquetes son estructurales y conviene nombrarlos:

- **`libssl-dev` / `openssl-devel`** — en Linux HTTPS usa el TLS del sistema, y
  esto es justamente eso.
- **`libgtk-3-dev` / `gtk3-devel`** — los diálogos nativos de Abrir y Guardar.

X11 y Wayland están ambos soportados; la capa de ventanas elige la sesión que
esté en marcha, así que ninguna de las dos es una instalación aparte.

---

## 2. Obtener el código

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compilar

```sh
cargo build
```

> La primera compilación descarga todos los crates y compila el workspace, así
> que cuente con unos minutos y una caché en `target/` de alrededor de 1,5 GB.
> Las compilaciones siguientes son incrementales. `cargo clean` recupera el
> espacio cuando lo quiera de vuelta.

Para compilar sólo las dos cosas que se ejecutan:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Lanzar la IDE

```sh
cargo run -p cobolt-ide
```

Para el día a día prefiera una compilación release — más lenta de compilar una
vez, mucho más fluida de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Ejecutar los tests

```sh
cargo test --workspace
```

El motor de forms necesita su feature `render` para probar los caminos de
renderizado:

```sh
cargo test -p cobolt-forms --features render
```

---

## Dónde acaban los artefactos

| Artefacto | Ruta |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` en Windows) |
| Runtime / compilador de línea de órdenes | `target/release/rcrun` (`.exe` en Windows) |
| Una aplicación que compila **usted** desde un project | `<project>/bin/` y la carpeta de destino del project |

Una aplicación compilada con `rcrun build` es un único ejecutable autocontenido:
embebe su programa compilado, sus forms y cualquier tema de asset-pack que usen,
de modo que no hay nada que instalar junto a él en la máquina a la que se lo
entrega.

---

## Resolución de problemas

**`linker 'cc' not found` (Linux)** — falta `build-essential` (o
`@development-tools`).

**`link.exe not found` (Windows)** — las Build Tools se instalaron sin la carga
de trabajo «Desktop development with C++». Vuelva a ejecutar el instalador y
márquela.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` y `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**La IDE compila pero no se abre ninguna ventana (Linux)** — compruebe que
`libxkbcommon-dev` está instalado y que `$DISPLAY` o `$WAYLAND_DISPLAY` está
definido; una TTY pelada o una sesión SSH sin reenvío de X no tiene ninguna
pantalla sobre la que abrirse.

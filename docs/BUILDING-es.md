<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Compilar PowerRustCOBOL

De una máquina limpia a un IDE en marcha, en **Windows**, **Linux** y **macOS**.

Todo lo de aquí son los mismos tres pasos en cualquier plataforma: instalar una
cadena de herramientas, clonar y `cargo build`. Solo el primer paso cambia según
el sistema operativo.

---

## Qué necesita la compilación

| Requisito | Para qué |
|---|---|
| **Rust**, canal estable, **1.92 o posterior** | compila todo el espacio de trabajo |
| **Git** | clona el repositorio |
| **Un compilador de C y un enlazador** | el enlazador que Rust necesita para *cualquier* binario, más dos dependencias en C |
| **Bibliotecas GUI nativas** (solo Linux) | creación de ventanas y los diálogos de fichero nativos |

> **El IDE empaquetado comprueba por sí mismo el requisito de Rust.** Quien *usa*
> PowerRustCOBOL en lugar de compilarlo nunca lee esta página, así que el IDE
> busca Rust en su primer arranque y ofrece instalarlo cuando no se cumple este
> mismo mínimo de **1.92**. Lee el número del propio manifiesto de este espacio de
> trabajo, de modo que los dos no pueden discrepar. Véase §3 de la Guía del
> desarrollador.

### Sobre el compilador de C

Dos crates del árbol compilan código C, así que un compilador de C es realmente
imprescindible:

- **`libsqlite3-sys`** — SQLite, incorporado desde su amalgama en C. Es el soporte
  de SQLite del entorno de ejecución de bases de datos COBOL, de modo que no hay
  que instalar ni hacer coincidir versiones de ningún SQLite del sistema en la
  máquina del usuario final.
- **`onig_sys`** — el motor de expresiones regulares Oniguruma, que usa el
  tokenizador que hay detrás de la búsqueda semántica.

Lo que la compilación **no** necesita, y nunca invoca:

> **ningún compilador de C++ · ni CMake · ni NASM · ni Python · ni Node · ni JVM**

Eso es deliberado y se mantiene así. TLS pasa por la pila propia del sistema
operativo (schannel en Windows, Security.framework en macOS, OpenSSL en Linux)
mediante enlaces escritos íntegramente en Rust, en lugar de una biblioteca
criptográfica incorporada que exigiría C, ensamblador y CMake en cada máquina; el
array de sufijos en C++ del tokenizador (`esaxx_fast`) está desactivado porque
aquí no se entrena ningún modelo; y el índice de la base de conocimiento es
`redb`, Rust puro.

En todas las plataformas el compilador de C llega dentro del mismo paquete que
proporciona el enlazador que Rust ya exige, así que en la práctica esto no añade
nada que instalar.

---

## 1. Instalar la cadena de herramientas

### Windows

1. Instale las **Visual Studio Build Tools** con la carga de trabajo **«Desktop
   development with C++»** —
   [descarga](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La carga de trabajo lleva el nombre de C++, pero lo que entrega es lo que
   cualquier compilación de Rust en Windows necesita de todos modos: `link.exe`,
   el SDK de Windows y `cl.exe` para las dos dependencias en C de arriba. No hay
   nada más que descargar.

2. Instale Rust desde [rustup.rs](https://rustup.rs). Selecciona automáticamente
   la cadena de herramientas MSVC.

3. Verifique, desde un símbolo del sistema normal de PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

No hay opciones de enlazado que poner a mano: el `.cargo/config.toml` del
repositorio ya sitúa cada objeto sobre el CRT dinámico, que es lo que evita que
las dependencias en C y el propio runtime de Rust choquen al enlazar.

### macOS

Instale las Xcode Command Line Tools — eso es todo:

```sh
xcode-select --install
```

Y después Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel están ambos soportados; rustup elige el destino de host
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

Y después Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Dos de esos paquetes son estructurales y merecen nombrarse:

- **`libssl-dev` / `openssl-devel`** — en Linux HTTPS usa el TLS del sistema, y es
  este.
- **`libgtk-3-dev` / `gtk3-devel`** — los diálogos nativos de Abrir/Guardar.

X11 y Wayland están ambos soportados; la capa de ventanas elige la sesión que
esté en marcha, así que ninguno es una instalación aparte.

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

> La primera compilación descarga todos los crates y compila el espacio de
> trabajo, así que cuente con unos minutos y una caché `target/` de unos 1,5 GB.
> Las siguientes son incrementales. `cargo clean` recupera el espacio cuando lo
> quiera de vuelta.

Para compilar solo las dos cosas que se ejecutan:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Arrancar el IDE

```sh
cargo run -p cobolt-ide
```

Para el uso diario es preferible una compilación de release: más lenta de
compilar una vez, mucho más suave de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Ejecutar las pruebas

```sh
cargo test --workspace
```

El motor de formularios necesita su característica `render` para probar los
caminos de renderizado:

```sh
cargo test -p cobolt-forms --features render
```

---

## Dónde acaban los artefactos

| Artefacto | Ruta |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` en Windows) |
| Runtime / compilador de la CLI | `target/release/rcrun` (`.exe` en Windows) |
| Una aplicación que **usted** compila desde un proyecto | `<proyecto>/bin/` y la carpeta de destino del proyecto |

Una aplicación compilada con `rcrun build` es un único ejecutable autocontenido:
incorpora su programa compilado, sus formularios y cualquier tema de paquete de
recursos que usen, de modo que no hay nada que instalar junto a él en la máquina
a la que se lo entregue.

---

## Instalar el IDE en otra parte — lleve consigo el SDK de la plataforma

El ejecutable del IDE **no** es autocontenido como sí lo es una aplicación que
usted compile. Compilar una aplicación ejecuta un `cargo build` real contra las
fuentes Rust de la plataforma, así que esas fuentes deben existir en la máquina
que compila. Copie `cobolt-ide` a otro sitio por sí solo y Build falla, nombrando
todas las carpetas en las que buscó: la cadena de herramientas está bien, las
fuentes sencillamente no están.

Prepárelas junto al ejecutable. Desde el árbol de fuentes:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

Eso escribe `Cargo.toml`, `Cargo.lock` y `crates/` en `<install-dir>`, junto con
los recursos que necesita una aplicación de formularios: el árbol de temas y el
icono de ventana. Los diez crates contra los que compila una aplicación
construida ocupan **8,6 MiB**; con `assets/themes` el árbol preparado ronda los
**21 MiB**. El icono no es opcional: omítalo y no compila ninguna aplicación de
formularios. Pase `--sdk` para ponerlos en `<install-dir>/sdk/` cuando la carpeta
de instalación contenga otras cosas. El IDE encuentra cualquiera de las dos
disposiciones sin configuración alguna, y además mira un nivel por encima y, en
macOS, dentro de `Resources` del paquete.

La máquina sigue necesitando la cadena de herramientas de Rust —Build es una
compilación de verdad— y su primera compilación descarga los crates de
dependencia del registro, así que necesita acceso a la red una vez.

> **Nota.** Para un checkout que viva en un lugar completamente distinto, indique
> la carpeta a mano en **Help → Platform SDK Location**. Se recuerda por máquina y
> no por proyecto, de modo que nunca viaja a un colega dentro de `cobolt.toml`.
> Déjelo en blanco para volver a la búsqueda automática.

---

## Resolución de problemas

**`linker 'cc' not found` (Linux)** — falta `build-essential` (o
`@development-tools`).

**`link.exe not found` (Windows)** — las Build Tools se instalaron sin la carga de
trabajo «Desktop development with C++». Vuelva a ejecutar el instalador y
márquela.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` y `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**El IDE compila pero no se abre ninguna ventana (Linux)** — compruebe que
`libxkbcommon-dev` está instalado y que `$DISPLAY` o `$WAYLAND_DISPLAY` tiene
valor; una TTY pelada o una sesión SSH sin reenvío de X no tiene pantalla sobre la
que abrirse.

.<<

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilar PowerRustCOBOL

De una máquina limpia a un IDE en marcha, en **Windows**, **Linux** y **macOS**.

Todo lo que hay aquí son los mismos tres pasos en cada plataforma — instalar una
cadena de herramientas, clonar, `cargo build`. Solo el primer paso cambia según
el sistema operativo.

---

## Qué necesita la compilación

| Requisito | Por qué |
|---|---|
| **Rust**, canal stable, **1.92 o posterior** | compila todo el workspace |
| **Git** | clona el repositorio |
| **Un compilador de C y un enlazador** | el enlazador que Rust necesita para *cualquier* binario, más dos dependencias en C |
| **Bibliotecas GUI nativas** (solo Linux) | la creación de ventanas y los diálogos de archivo nativos |

> **El IDE empaquetado comprueba el requisito de Rust por su cuenta.** Quien
> *usa* PowerRustCOBOL en lugar de compilarlo nunca lee esta página, así que el
> IDE busca Rust en su primer arranque y ofrece instalarlo cuando no se cumple
> este mismo mínimo de **1.92**. Lee el número del propio manifiesto de este
> workspace, de modo que los dos no pueden discrepar. Consulta el §3 de la Guía
> del desarrollador.

### Sobre el compilador de C

Dos crates del árbol compilan código C, así que un compilador de C hace falta de
verdad:

- **`libsqlite3-sys`** — SQLite, empaquetado desde su amalgama en C. Este es el
  soporte de SQLite del runtime de bases de datos de COBOL, así que no hay que
  instalar ningún SQLite del sistema ni casar versiones en la máquina del
  usuario final.
- **`onig_sys`** — el motor de expresiones regulares Oniguruma, que usa el
  tokenizador que hay detrás de la búsqueda semántica.

Lo que la compilación **no** necesita, y nunca invoca:

> **sin compilador de C++ · sin CMake · sin NASM · sin Python · sin Node · sin JVM**

Es deliberado y se mantiene así. TLS pasa por la pila propia del sistema
operativo (schannel en Windows, Security.framework en macOS, OpenSSL en Linux)
mediante enlaces en Rust puro, en lugar de una biblioteca criptográfica
empaquetada que exigiría C, ensamblador y CMake en cada máquina; el array de
sufijos en C++ del tokenizador (`esaxx_fast`) está apagado porque aquí nada
entrena un modelo; y el índice de la base de conocimiento es `redb`, Rust puro.

En todas las plataformas el compilador de C llega dentro del mismo paquete que
provee el enlazador que Rust ya requiere, así que en la práctica esto no añade
nada que instalar.

---

## 1. Instalar la cadena de herramientas

### Windows

1. Instala las **Visual Studio Build Tools** con la carga de trabajo **"Desktop
   development with C++"** —
   [descarga](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   La carga de trabajo lleva el nombre de C++, pero lo que entrega es lo que
   toda compilación de Rust en Windows necesita de todos modos: `link.exe`, el
   Windows SDK y `cl.exe` para las dos dependencias en C de arriba. No hay nada
   más que descargar.

2. Instala Rust desde [rustup.rs](https://rustup.rs). Selecciona la cadena de
   herramientas MSVC automáticamente.

3. Verifica, desde un símbolo del sistema normal de PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

No hay flags de enlazado que poner a mano: el `.cargo/config.toml` del
repositorio ya pone cada objeto sobre el CRT dinámico, que es lo que evita que
las dependencias en C y el propio runtime de Rust choquen al enlazar.

### macOS

Instala las Xcode Command Line Tools — eso es todo:

```sh
xcode-select --install
```

Después Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel están soportados los dos; rustup elige el target host
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

Dos de esos paquetes son estructurales y merecen que se los nombre:

- **`libssl-dev` / `openssl-devel`** — HTTPS usa el TLS del sistema en Linux, y
  este es.
- **`libgtk-3-dev` / `gtk3-devel`** — los diálogos nativos de Abrir/Guardar.

X11 y Wayland están soportados los dos; la capa de ventanas elige la sesión que
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

> La primera compilación descarga cada crate y compila el workspace, así que
> cuenta con unos minutos y una caché `target/` de alrededor de 1,5 GB. Las
> siguientes son incrementales. `cargo clean` recupera el espacio cuando lo
> quieras de vuelta.

Para compilar solo las dos cosas que ejecutas:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Arrancar el IDE

```sh
cargo run -p cobolt-ide
```

Para el día a día prefiere una compilación release — más lenta de compilar una
vez, mucho más fluida de usar:

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
| Runtime / constructor de línea de comandos | `target/release/rcrun` (`.exe` en Windows) |
| Una aplicación que **tú** compilas a partir de un proyecto | `<project>/bin/` y la carpeta de destino del proyecto |

Una aplicación compilada con `rcrun build` es un único ejecutable
autocontenido: lleva incrustados su programa compilado, sus formularios y
cualquier tema de paquete de recursos que usen, así que no hay nada que instalar
junto a él en la máquina a la que se lo entregas.

---

## Instalar el IDE en otro sitio — lleva el SDK de la plataforma

El ejecutable del IDE **no** es autocontenido como sí lo es una aplicación que
compilas tú. Compilar una aplicación ejecuta un `cargo build` real contra las
fuentes Rust de la plataforma, así que esas fuentes tienen que existir en la
máquina que compila. Copia `cobolt-ide` a un sitio por su cuenta y Build falla,
nombrando cada carpeta en la que ha mirado — la cadena de herramientas está
bien, las fuentes sencillamente no están.

Colócalas junto al ejecutable. Desde el árbol de fuentes:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

Eso escribe `Cargo.toml` y `crates/` en `<install-dir>` — 6,0 MB, los diez
crates contra los que compila una aplicación construida. Pasa `--sdk` para
ponerlos en `<install-dir>/sdk/` cuando la carpeta de instalación contenga otras
cosas. El IDE encuentra cualquiera de las dos disposiciones sin configuración
alguna, y también mira un nivel más arriba y, en macOS, dentro de los
`Resources` del bundle.

La máquina sigue necesitando la cadena de herramientas de Rust — Build es una
compilación de verdad — y su primera compilación descarga los crates de las
dependencias desde el registro, así que necesita acceso a la red una vez.

> **Nota.** Para un checkout que viva en un sitio completamente distinto, fija
> la carpeta a mano en **Help → Platform SDK Location**. Se recuerda por máquina
> y no por proyecto, así que nunca viaja hasta un colega dentro de `cobolt.toml`.
> Déjalo en blanco para volver a la búsqueda automática.

---

## Resolución de problemas

**`linker 'cc' not found` (Linux)** — falta `build-essential` (o
`@development-tools`).

**`link.exe not found` (Windows)** — las Build Tools se instalaron sin la carga
de trabajo "Desktop development with C++". Vuelve a ejecutar el instalador y
márcala.

**`Could not find directory of OpenSSL installation` (Linux)** — instala
`libssl-dev` / `openssl-devel` y `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**El IDE compila pero no se abre ninguna ventana (Linux)** — comprueba que
`libxkbcommon-dev` está instalado y que `$DISPLAY` o `$WAYLAND_DISPLAY` está
definido; una TTY pelada o una sesión SSH sin reenvío de X no tiene pantalla
sobre la que abrirse.

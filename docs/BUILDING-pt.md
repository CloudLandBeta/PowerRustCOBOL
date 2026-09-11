<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# Compilar o PowerRustCOBOL

De uma máquina limpa até um IDE a correr, no **Windows**, no **Linux** e no
**macOS**.

Tudo o que está aqui são os mesmos três passos em qualquer plataforma: instalar
uma cadeia de ferramentas, clonar e `cargo build`. Só o primeiro passo difere
conforme o sistema operativo.

---

## Do que a compilação precisa

| Requisito | Para quê |
|---|---|
| **Rust**, canal estável, **1.92 ou mais recente** | compila todo o espaço de trabalho |
| **Git** | clona o repositório |
| **Um compilador de C e um linker** | o linker de que o Rust precisa para *qualquer* binário, mais duas dependências em C |
| **Bibliotecas GUI nativas** (só Linux) | criação de janelas e os diálogos de ficheiro nativos |

> **O IDE empacotado verifica ele próprio o requisito do Rust.** Quem *usa* o
> PowerRustCOBOL em vez de o compilar nunca lê esta página, por isso o IDE procura
> o Rust no primeiro arranque e oferece-se para o instalar quando este mesmo
> mínimo de **1.92** não é cumprido. Lê o número do manifesto do próprio espaço de
> trabalho, pelo que os dois não podem discordar. Ver §3 do Guia do programador.

### Sobre o compilador de C

Duas crates da árvore compilam código C, por isso um compilador de C é mesmo
indispensável:

- **`libsqlite3-sys`** — SQLite, incorporado a partir da sua amálgama em C. É o
  suporte de SQLite do ambiente de execução de bases de dados COBOL, para que não
  seja preciso instalar nem fazer coincidir versões de um SQLite do sistema na
  máquina do utilizador final.
- **`onig_sys`** — o motor de expressões regulares Oniguruma, usado pelo
  tokenizador que está por trás da pesquisa semântica.

O que a compilação **não** precisa, e nunca invoca:

> **nenhum compilador de C++ · nenhum CMake · nenhum NASM · nenhum Python ·
> nenhum Node · nenhuma JVM**

Isso é deliberado e assim se mantém. O TLS passa pela pilha do próprio sistema
operativo (schannel no Windows, Security.framework no macOS, OpenSSL no Linux)
através de ligações escritas inteiramente em Rust, em vez de uma biblioteca
criptográfica incorporada que exigiria C, assembly e CMake em cada máquina; o
array de sufixos em C++ do tokenizador (`esaxx_fast`) está desligado porque aqui
não se treina modelo nenhum; e o índice da base de conhecimento é `redb`, Rust
puro.

Em todas as plataformas o compilador de C vem dentro do mesmo pacote que fornece
o linker que o Rust já exige, pelo que na prática isto não acrescenta nada para
instalar.

---

## 1. Instalar a cadeia de ferramentas

### Windows

1. Instale as **Visual Studio Build Tools** com a carga de trabalho **"Desktop
   development with C++"** —
   [descarregar](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   A carga de trabalho tem o nome de C++, mas o que entrega é aquilo de que
   qualquer compilação de Rust no Windows precisa de qualquer forma: `link.exe`, o
   SDK do Windows e `cl.exe` para as duas dependências em C acima. Não há mais
   nada para descarregar.

2. Instale o Rust a partir de [rustup.rs](https://rustup.rs). Seleciona
   automaticamente a cadeia de ferramentas MSVC.

3. Verifique, a partir de uma linha de comandos normal do PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

Não há opções de linkagem para definir à mão: o `.cargo/config.toml` do
repositório já coloca cada objeto sobre o CRT dinâmico, que é o que impede as
dependências em C e o próprio runtime do Rust de colidirem na linkagem.

### macOS

Instale as Xcode Command Line Tools — é tudo:

```sh
xcode-select --install
```

Depois o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel são ambos suportados; o rustup escolhe o alvo de anfitrião
correto.

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

Depois o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Dois desses pacotes são estruturais e merecem ser nomeados:

- **`libssl-dev` / `openssl-devel`** — no Linux o HTTPS usa o TLS do sistema, e é
  este.
- **`libgtk-3-dev` / `gtk3-devel`** — os diálogos nativos de Abrir/Guardar.

X11 e Wayland são ambos suportados; a camada de janelas escolhe a sessão que
estiver a correr, pelo que nenhum é uma instalação à parte.

---

## 2. Obter o código

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compilar

```sh
cargo build
```

> A primeira compilação descarrega todas as crates e compila o espaço de
> trabalho, por isso conte com alguns minutos e uma cache `target/` à volta de
> 1,5 GB. As seguintes são incrementais. O `cargo clean` recupera o espaço sempre
> que o quiser de volta.

Para compilar apenas as duas coisas que se executam:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Arrancar o IDE

```sh
cargo run -p cobolt-ide
```

Para o dia a dia prefira uma compilação de release: mais lenta a compilar uma
vez, muito mais suave de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Correr os testes

```sh
cargo test --workspace
```

O motor de formulários precisa da sua funcionalidade `render` para testar os
caminhos de renderização:

```sh
cargo test -p cobolt-forms --features render
```

---

## Onde ficam os artefactos

| Artefacto | Caminho |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` no Windows) |
| Runtime / compilador da CLI | `target/release/rcrun` (`.exe` no Windows) |
| Uma aplicação que **você** compila a partir de um projeto | `<projeto>/bin/` e a pasta de destino do projeto |

Uma aplicação compilada com `rcrun build` é um único executável autossuficiente:
incorpora o seu programa compilado, os seus formulários e qualquer tema de pacote
de recursos que usem, pelo que não há nada para instalar ao lado dele na máquina
a quem o entregar.

---

## Instalar o IDE noutro sítio — leve o SDK da plataforma

O executável do IDE **não** é autossuficiente como é uma aplicação que você
compile. Compilar uma aplicação corre um `cargo build` a sério contra as fontes
Rust da plataforma, por isso essas fontes têm de existir na máquina que compila.
Copie o `cobolt-ide` sozinho para outro sítio e o Build falha, nomeando todas as
pastas onde procurou — a cadeia de ferramentas está bem, as fontes é que
simplesmente não estão lá.

Prepare-as ao lado do executável. A partir da árvore de fontes:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

Isso escreve `Cargo.toml`, `Cargo.lock` e `crates/` em `<install-dir>`, juntamente
com os recursos de que uma aplicação de formulários precisa — a árvore de temas e
o ícone de janela. As dez crates contra as quais uma aplicação compilada é
construída ocupam **8,6 MiB**; com `assets/themes` a árvore preparada ronda os
**21 MiB**. O ícone não é opcional: omita-o e nenhuma aplicação de formulários
compila sequer. Passe `--sdk` para os colocar em `<install-dir>/sdk/` quando a
pasta de instalação contiver outras coisas. O IDE encontra qualquer uma das
disposições sem configuração nenhuma, e olha também um nível acima e, no macOS,
dentro de `Resources` do pacote.

A máquina continua a precisar da cadeia de ferramentas Rust — o Build é uma
compilação a sério — e a sua primeira compilação descarrega as crates de
dependência do registo, pelo que precisa de acesso à rede uma vez.

> **Nota.** Para um checkout que viva noutro sítio completamente diferente,
> indique a pasta à mão em **Help → Platform SDK Location**. É lembrada por
> máquina e não por projeto, pelo que nunca viaja para um colega dentro do
> `cobolt.toml`. Deixe em branco para voltar à procura automática.

---

## Resolução de problemas

**`linker 'cc' not found` (Linux)** — falta o `build-essential` (ou
`@development-tools`).

**`link.exe not found` (Windows)** — as Build Tools foram instaladas sem a carga
de trabalho "Desktop development with C++". Volte a correr o instalador e
assinale-a.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` e `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**O IDE compila mas não abre nenhuma janela (Linux)** — verifique se o
`libxkbcommon-dev` está instalado e se `$DISPLAY` ou `$WAYLAND_DISPLAY` tem valor;
uma TTY nua ou uma sessão SSH sem reencaminhamento de X não tem ecrã onde
abrir.

.<<

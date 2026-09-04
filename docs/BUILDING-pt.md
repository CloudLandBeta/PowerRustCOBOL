<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilando o PowerRustCOBOL

De uma máquina limpa até um IDE rodando, no **Windows**, no **Linux** e no
**macOS**.

Tudo aqui são os mesmos três passos em qualquer plataforma — instalar um
toolchain, clonar, `cargo build`. Só o primeiro passo muda de acordo com o
sistema operacional.

---

## O que a compilação exige

| Requisito | Por quê |
|---|---|
| **Rust**, canal stable, **1.92 ou mais recente** | compila o workspace inteiro |
| **Git** | clona o repositório |
| **Um compilador C e um linker** | o linker que o Rust precisa para *qualquer* binário, mais duas dependências em C |
| **Bibliotecas GUI nativas** (só no Linux) | a criação de janelas e os diálogos de arquivo nativos |

> **O IDE empacotado confere o requisito do Rust sozinho.** Quem *usa* o
> PowerRustCOBOL em vez de compilá-lo nunca lê esta página, então o IDE procura
> o Rust na primeira execução e oferece instalá-lo quando esse mesmo mínimo de
> **1.92** não é atendido. Ele lê o número do próprio manifesto deste workspace,
> de modo que os dois não têm como divergir. Veja o §3 do Guia do desenvolvedor.

### Sobre o compilador C

Dois crates da árvore compilam código C, então um compilador C é mesmo
necessário:

- **`libsqlite3-sys`** — SQLite, embutido a partir da sua amálgama em C. Este é
  o suporte a SQLite do runtime de banco de dados do COBOL, então nenhum SQLite
  do sistema precisa ser instalado nem ter a versão casada na máquina do usuário
  final.
- **`onig_sys`** — o motor de expressões regulares Oniguruma, usado pelo
  tokenizador que fica por trás da busca semântica.

O que a compilação **não** exige, e nunca invoca:

> **sem compilador C++ · sem CMake · sem NASM · sem Python · sem Node · sem JVM**

Isso é deliberado e continua assim. O TLS passa pela pilha do próprio sistema
operacional (schannel no Windows, Security.framework no macOS, OpenSSL no Linux)
por meio de bindings em Rust puro, em vez de uma biblioteca de criptografia
embutida que exigiria C, assembly e CMake em toda máquina; o array de sufixos em
C++ do tokenizador (`esaxx_fast`) está desligado porque nada aqui treina um
modelo; e o índice da base de conhecimento é o `redb`, Rust puro.

Em todas as plataformas o compilador C chega dentro do mesmo pacote que fornece
o linker que o Rust já exige, então na prática isso não acrescenta nada para
instalar.

---

## 1. Instalar o toolchain

### Windows

1. Instale as **Visual Studio Build Tools** com a carga de trabalho **"Desktop
   development with C++"** —
   [download](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   A carga de trabalho leva o nome de C++, mas o que ela entrega é o que toda
   compilação de Rust no Windows precisa de qualquer jeito: `link.exe`, o
   Windows SDK e `cl.exe` para as duas dependências em C acima. Não há mais nada
   a baixar.

2. Instale o Rust em [rustup.rs](https://rustup.rs). Ele seleciona o toolchain
   MSVC automaticamente.

3. Verifique, a partir de um prompt normal do PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

Não há flags de linkagem para definir na mão: o `.cargo/config.toml` do
repositório já coloca cada objeto sobre o CRT dinâmico, e é isso que impede as
dependências em C e o próprio runtime do Rust de colidirem na hora de linkar.

### macOS

Instale as Xcode Command Line Tools — é só isso:

```sh
xcode-select --install
```

Depois o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel são ambos suportados; o rustup escolhe o target host
certo.

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

- **`libssl-dev` / `openssl-devel`** — o HTTPS usa o TLS do sistema no Linux, e
  é este.
- **`libgtk-3-dev` / `gtk3-devel`** — os diálogos nativos de Abrir/Salvar.

X11 e Wayland são ambos suportados; a camada de janelas escolhe a sessão que
estiver rodando, então nenhum dos dois é uma instalação à parte.

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

> A primeira compilação baixa cada crate e compila o workspace, então conte com
> alguns minutos e um cache `target/` de uns 1,5 GB. As compilações seguintes
> são incrementais. `cargo clean` devolve o espaço sempre que você quiser.

Para compilar só as duas coisas que você executa:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Iniciar o IDE

```sh
cargo run -p cobolt-ide
```

No dia a dia, prefira uma compilação release — mais lenta de compilar uma vez,
bem mais fluida de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Rodando os testes

```sh
cargo test --workspace
```

O motor de formulários precisa da feature `render` para testar os caminhos de
renderização:

```sh
cargo test -p cobolt-forms --features render
```

---

## Onde os artefatos vão parar

| Artefato | Caminho |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` no Windows) |
| Runtime / construtor de linha de comando | `target/release/rcrun` (`.exe` no Windows) |
| Um aplicativo que **você** compila a partir de um projeto | `<project>/bin/` e a pasta de destino do projeto |

Um aplicativo compilado com `rcrun build` é um único executável autocontido: ele
embute o programa compilado, seus formulários e qualquer tema de asset pack que
eles usem, então não há nada para instalar ao lado dele na máquina para a qual
você o entrega.

---

## Instalando o IDE em outro lugar — leve o SDK da plataforma

O executável do IDE **não** é autocontido do jeito que um aplicativo compilado
por você é. Compilar um aplicativo executa um `cargo build` de verdade contra os
fontes Rust da plataforma, então esses fontes precisam existir na máquina que
está compilando. Copie o `cobolt-ide` sozinho para algum lugar e o Build falha,
nomeando cada pasta em que procurou — o toolchain está em ordem, os fontes é que
simplesmente não estão lá.

Coloque-os ao lado do executável. A partir da árvore de fontes:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

Isso escreve `Cargo.toml` e `crates/` em `<install-dir>` — 6,0 MB, os dez crates
contra os quais um aplicativo compilado é construído. Passe `--sdk` para
colocá-los em `<install-dir>/sdk/` quando a pasta de instalação guardar outras
coisas. O IDE encontra qualquer um dos dois arranjos sem configuração alguma, e
também olha um nível acima e, no macOS, dentro dos `Resources` do bundle.

A máquina ainda precisa do toolchain do Rust — o Build é uma compilação de
verdade — e a primeira compilação dela baixa os crates das dependências do
registro, então ela precisa de acesso à rede uma vez.

> **Nota.** Para um checkout que mora em outro lugar completamente diferente,
> defina a pasta na mão em **Help → Platform SDK Location**. Ela é lembrada por
> máquina e não por projeto, então nunca viaja até um colega dentro do
> `cobolt.toml`. Deixe em branco para voltar à busca automática.

---

## Resolução de problemas

**`linker 'cc' not found` (Linux)** — falta o `build-essential` (ou o
`@development-tools`).

**`link.exe not found` (Windows)** — as Build Tools foram instaladas sem a carga
de trabalho "Desktop development with C++". Rode o instalador de novo e marque-a.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` e `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**O IDE compila mas nenhuma janela abre (Linux)** — verifique se o
`libxkbcommon-dev` está instalado e se `$DISPLAY` ou `$WAYLAND_DISPLAY` está
definido; um TTY puro ou uma sessão SSH sem encaminhamento de X não tem tela
nenhuma para abrir.

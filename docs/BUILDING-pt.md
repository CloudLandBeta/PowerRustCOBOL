<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilando o PowerRustCOBOL

De uma máquina limpa até uma IDE em execução, no **Windows**, no **Linux** e no
**macOS**.

Tudo aqui são os mesmos três passos em qualquer plataforma: instalar um
toolchain, clonar e `cargo build`. Somente o primeiro passo muda conforme o
sistema operacional.

---

## O que a compilação exige

| Requisito | Para quê |
|---|---|
| **Rust**, canal stable, **1.92 ou mais recente** | compila todo o workspace |
| **Git** | clona o repositório |
| **Um compilador C e um linkeditor** | o linkeditor que o Rust exige para *qualquer* binário, mais duas dependências em C |
| **Bibliotecas GUI nativas** (somente Linux) | criação de janelas e diálogos de arquivo nativos |

### Sobre o compilador C

Duas crates da árvore compilam código C, portanto um compilador C é realmente
necessário:

- **`libsqlite3-sys`** — SQLite, embutido a partir da sua amálgama em C. Este é
  o suporte a SQLite do runtime de bancos de dados do COBOL, de modo que nenhum
  SQLite do sistema precisa ser instalado nem ter a versão conferida na máquina
  do usuário final.
- **`onig_sys`** — o motor de expressões regulares Oniguruma, usado pelo
  tokenizador que está por trás da busca semântica.

O que a compilação **não** exige, e nunca invoca:

> **nenhum compilador C++ · nenhum CMake · nenhum NASM · nenhum Python · nenhum Node · nenhuma JVM**

Isso é deliberado e é mantido assim. O TLS passa pela pilha do próprio sistema
operacional (schannel no Windows, Security.framework no macOS, OpenSSL no Linux)
por meio de bindings em Rust puro, em vez de uma biblioteca criptográfica
embutida que exigiria C, assembly e CMake em cada máquina; o array de sufixos em
C++ do tokenizador (`esaxx_fast`) está desligado porque nada aqui treina um
modelo; e o índice da Knowledge Base é `redb`, Rust puro.

Em todas as plataformas o compilador C chega dentro do mesmo pacote que fornece
o linkeditor que o Rust já exige, então na prática isso não acrescenta nada a
instalar.

---

## 1. Instalar o toolchain

### Windows

1. Instale as **Visual Studio Build Tools** com a carga de trabalho
   **"Desktop development with C++"** —
   [download](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   A carga de trabalho leva o nome de C++, mas o que ela entrega é exatamente o
   que toda compilação Rust no Windows precisa de qualquer forma: `link.exe`, o
   SDK do Windows e `cl.exe` para as duas dependências em C acima. Não há mais
   nada para baixar.

2. Instale o Rust em [rustup.rs](https://rustup.rs). Ele seleciona o toolchain
   MSVC automaticamente.

3. Verifique, a partir de um prompt normal do PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

Não há flags de linkedição para definir à mão: o `.cargo/config.toml` do
repositório já coloca todos os objetos sobre o CRT dinâmico, que é o que impede
as dependências em C e o próprio runtime do Rust de colidirem na linkedição.

### macOS

Instale as Xcode Command Line Tools — é só isso:

```sh
xcode-select --install
```

Depois o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel são ambos suportados; o rustup escolhe o target de host
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

Dois desses pacotes são estruturais e vale a pena nomeá-los:

- **`libssl-dev` / `openssl-devel`** — no Linux o HTTPS usa o TLS do sistema, e
  é exatamente isto.
- **`libgtk-3-dev` / `gtk3-devel`** — os diálogos nativos de Abrir e Salvar.

X11 e Wayland são ambos suportados; a camada de janelas escolhe a sessão que
estiver em execução, de modo que nenhuma das duas é uma instalação à parte.

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

> A primeira compilação baixa todas as crates e compila o workspace, portanto
> conte com alguns minutos e um cache em `target/` em torno de 1,5 GB. As
> compilações seguintes são incrementais. `cargo clean` devolve o espaço sempre
> que você quiser.

Para compilar apenas as duas coisas que você executa:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Iniciar a IDE

```sh
cargo run -p cobolt-ide
```

Para o dia a dia prefira uma compilação release — mais lenta de compilar uma
vez, muito mais fluida de usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Rodando os testes

```sh
cargo test --workspace
```

O motor de forms precisa da sua feature `render` para testar os caminhos de
renderização:

```sh
cargo test -p cobolt-forms --features render
```

---

## Onde os artefatos vão parar

| Artefato | Caminho |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` no Windows) |
| Runtime / compilador de linha de comando | `target/release/rcrun` (`.exe` no Windows) |
| Uma aplicação que **você** compila a partir de um project | `<project>/bin/` e a pasta de destino do project |

Uma aplicação compilada com `rcrun build` é um único executável autocontido: ele
embute o seu programa compilado, os seus forms e qualquer tema de asset-pack que
eles usem, de modo que não há nada a instalar ao lado dele na máquina para a
qual você o entrega.

---

## Solução de problemas

**`linker 'cc' not found` (Linux)** — falta o `build-essential` (ou
`@development-tools`).

**`link.exe not found` (Windows)** — as Build Tools foram instaladas sem a carga
de trabalho "Desktop development with C++". Execute o instalador novamente e
marque-a.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` e `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**A IDE compila mas nenhuma janela abre (Linux)** — verifique se o
`libxkbcommon-dev` está instalado e se `$DISPLAY` ou `$WAYLAND_DISPLAY` está
definido; um TTY puro ou uma sessão SSH sem encaminhamento de X não tem tela
alguma sobre a qual abrir.

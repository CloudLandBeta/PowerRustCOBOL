<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# Compilando o PowerRustCOBOL

De uma máquina limpa até a IDE em execução, no **Windows**, no **Linux** e no
**macOS**.

Tudo aqui são os mesmos três passos em qualquer plataforma — instalar um
toolchain, clonar, `cargo build`. Só o primeiro passo muda conforme o sistema
operacional.

---

## O que a compilação exige

| Requisito | Por quê |
|---|---|
| **Rust**, canal stable, **1.92 ou mais recente** | compila todo o workspace |
| **Git** | clona o repositório |
| **Um compilador C e um linkeditor** | o linkeditor que o Rust exige para *qualquer* binário, mais duas dependências em C |
| **Bibliotecas gráficas nativas** (só no Linux) | criação de janelas e as caixas de diálogo de arquivo nativas |

> **A IDE empacotada verifica o requisito do Rust sozinha.** Quem *usa* o
> PowerRustCOBOL em vez de compilá-lo nunca lê esta página, então a IDE procura
> o Rust na primeira execução e se oferece para instalá-lo quando este mesmo
> mínimo de **1.92** não é atendido. Ela lê o número do próprio manifesto deste
> workspace, de modo que os dois não podem divergir. Veja a §3 do Guia do
> Desenvolvedor.

### Sobre o compilador C

Dois crates da árvore compilam código C, então um compilador C é realmente
necessário:

- **`libsqlite3-sys`** — o SQLite, embutido a partir da sua amalgamação em C.
  É o suporte a SQLite do runtime de banco de dados COBOL, de modo que nenhum
  SQLite do sistema precisa ser instalado nem ter a versão conferida na máquina
  do usuário final.
- **`onig_sys`** — o motor de expressões regulares Oniguruma, usado pelo
  tokenizador que está por trás da busca semântica.

O que a compilação **não** exige, e nunca invoca:

> **nenhum compilador C++ · nenhum CMake · nenhum NASM · nenhum Python · nenhum
> Node · nenhuma JVM**

Isso é deliberado e é mantido assim. O TLS passa pela pilha do próprio sistema
operacional (schannel no Windows, Security.framework no macOS, OpenSSL no Linux)
por meio de bindings puramente em Rust, em vez de uma biblioteca de criptografia
embutida que exigiria C, assembly e CMake em cada máquina; o suffix-array em C++
do tokenizador (`esaxx_fast`) fica desligado porque nada aqui treina modelo; e o
índice da Base de Conhecimento é o `redb`, puro Rust.

Em todas as plataformas o compilador C chega dentro do mesmo pacote que fornece
o linkeditor que o Rust já exige, então na prática isso não acrescenta nada a
instalar.

---

## 1. Instale o toolchain

### Windows

1. Instale as **Visual Studio Build Tools** com a carga de trabalho
   **"Desktop development with C++"** —
   [download](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).

   A carga de trabalho tem o nome de C++, mas o que ela entrega é o que toda
   compilação Rust no Windows precisa de qualquer forma: `link.exe`, o Windows
   SDK e o `cl.exe` para as duas dependências em C citadas acima. Não há mais
   nada para baixar.

2. Instale o Rust a partir de [rustup.rs](https://rustup.rs). Ele seleciona o
   toolchain MSVC automaticamente.

3. Verifique, num prompt normal do PowerShell:

   ```powershell
   rustc --version
   cargo --version
   ```

Não há flags de linkedição para ajustar à mão: o `.cargo/config.toml` do
repositório já coloca todos os objetos sobre o CRT dinâmico, que é o que impede
as dependências em C e o runtime do próprio Rust de colidirem na linkedição.

### macOS

Instale as Xcode Command Line Tools — é só isso:

```sh
xcode-select --install
```

Depois, o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon e Intel são ambos suportados; o rustup escolhe o target correto
para o host.

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

Depois, o Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Dois desses pacotes são estruturais e merecem ser citados:

- **`libssl-dev` / `openssl-devel`** — no Linux o HTTPS usa o TLS do sistema, e
  é isto aqui.
- **`libgtk-3-dev` / `gtk3-devel`** — as caixas de diálogo nativas de
  abrir/salvar.

X11 e Wayland são ambos suportados; a camada de janelas escolhe a sessão que
estiver em execução, então nenhum dos dois é uma instalação à parte.

---

## 2. Obtenha o código

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. Compile

```sh
cargo build
```

> A primeira compilação baixa todos os crates e compila o workspace, então
> espere alguns minutos e um cache `target/` de cerca de 1,5 GB. As compilações
> seguintes são incrementais. O `cargo clean` recupera o espaço sempre que você
> quiser tê-lo de volta.

Para compilar apenas as duas coisas que você executa:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. Inicie a IDE

```sh
cargo run -p cobolt-ide
```

No dia a dia prefira uma compilação release — mais lenta para compilar uma vez,
muito mais suave para usar:

```sh
cargo run --release -p cobolt-ide
```

---

## Executando os testes

```sh
cargo test --workspace
```

O motor de formulários precisa da sua feature `render` para testar os caminhos
de renderização:

```sh
cargo test -p cobolt-forms --features render
```

---

## Onde os artefatos ficam

| Artefato | Caminho |
|---|---|
| IDE | `target/release/cobolt-ide` (`.exe` no Windows) |
| Runtime / compilador de linha de comando | `target/release/rcrun` (`.exe` no Windows) |
| Uma aplicação que **você** compila a partir de um projeto | `<project>/bin/` e a pasta de destino do projeto |

Uma aplicação compilada com `rcrun build` é um único executável autossuficiente:
ele embute o programa compilado, seus formulários e qualquer tema de asset pack
que usem, de modo que não há nada para instalar ao lado dele na máquina para a
qual você o entrega.

---

## Solução de problemas

**`linker 'cc' not found` (Linux)** — falta o `build-essential` (ou
`@development-tools`).

**`link.exe not found` (Windows)** — as Build Tools foram instaladas sem a carga
de trabalho "Desktop development with C++". Rode o instalador de novo e marque-a.

**`Could not find directory of OpenSSL installation` (Linux)** — instale
`libssl-dev` / `openssl-devel` e `pkg-config`.

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`.

**A IDE compila mas nenhuma janela abre (Linux)** — verifique se o
`libxkbcommon-dev` está instalado e se `$DISPLAY` ou `$WAYLAND_DISPLAY` está
definido; um TTY puro ou uma sessão SSH sem encaminhamento de X não tem display
onde abrir.

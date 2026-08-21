<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL のビルド

まっさらなマシンから IDE が動くところまで。**Windows**、**Linux**、**macOS**
に対応します。

ここに書かれていることは、どのプラットフォームでも同じ 3 ステップです。
ツールチェーンをインストールし、クローンし、`cargo build` する。OS によって
異なるのは最初のステップだけです。

---

## ビルドに必要なもの

| 必要なもの | 用途 |
|---|---|
| **Rust**、stable チャンネル、**1.92 以降** | ワークスペース全体をビルドする |
| **Git** | リポジトリをクローンする |
| **C コンパイラとリンカ** | Rust が*あらゆる*バイナリに必要とするリンカ、および 2 つの C 依存関係 |
| **ネイティブ GUI ライブラリ**（Linux のみ） | ウィンドウの生成とネイティブのファイルダイアログ |

> **パッケージ版 IDE はこの Rust 要件を自分で確認します。** PowerRustCOBOL を
> ビルドせず*使う*人はこのページを読まないため、IDE は初回起動時に Rust を探し、
> ここと同じ **1.92** の下限を満たしていない場合はインストールを申し出ます。その
> 番号はこのワークスペース自身のマニフェストから読み取るので、両者が食い違うこと
> はありません。開発者ガイドの §3 を参照してください。

### C コンパイラについて

ツリー内の 2 つの crate が C のソースをコンパイルするため、C コンパイラは実際に
必要です。

- **`libsqlite3-sys`** — SQLite。C の amalgamation から同梱されます。これは
  COBOL のデータベースランタイムにおける SQLite サポートであり、エンドユーザーの
  マシンにシステムの SQLite をインストールしたりバージョンを合わせたりする必要は
  ありません。
- **`onig_sys`** — 鬼車（Oniguruma）正規表現エンジン。セマンティック検索の背後に
  あるトークナイザが使用します。

ビルドが**必要としない**もの、そして決して呼び出さないもの：

> **C++ コンパイラ不要 · CMake 不要 · NASM 不要 · Python 不要 · Node 不要 · JVM 不要**

これは意図的であり、今後もそのように保たれます。TLS は OS 自身のスタック
（Windows では schannel、macOS では Security.framework、Linux では OpenSSL）を、
純粋な Rust のバインディング経由で使います。各マシンに C とアセンブリと CMake を
要求するような暗号ライブラリを同梱することはしません。トークナイザの C++ 製
接尾辞配列（`esaxx_fast`）は、ここではモデルの学習を行わないため無効化されて
います。そして Knowledge Base のインデックスは純粋な Rust の `redb` です。

どのプラットフォームでも、C コンパイラは Rust がすでに必要としているリンカを
提供するのと同じパッケージに入っています。したがって実際には、これによって
追加でインストールするものは何もありません。

---

## 1. ツールチェーンをインストールする

### Windows

1. **Visual Studio Build Tools** を **「Desktop development with C++」**
   ワークロード付きでインストールします —
   [ダウンロード](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   このワークロードは C++ の名を冠していますが、実際に提供されるものは、Windows
   上のあらゆる Rust ビルドがいずれにせよ必要とするもの、すなわち `link.exe`、
   Windows SDK、そして上記 2 つの C 依存関係のための `cl.exe` です。他に
   ダウンロードするものはありません。

2. [rustup.rs](https://rustup.rs) から Rust をインストールします。MSVC
   ツールチェーンは自動的に選択されます。

3. 通常の PowerShell プロンプトから確認します。

   ```powershell
   rustc --version
   cargo --version
   ```

リンカのフラグを手で設定する必要はありません。リポジトリの
`.cargo/config.toml` がすでに、すべてのオブジェクトを動的 CRT の上に載せて
います。これが、C 依存関係と Rust 自身のランタイムがリンク時に衝突するのを防いで
います。

### macOS

Xcode Command Line Tools をインストールします。必要なのはそれだけです。

```sh
xcode-select --install
```

続いて Rust を入れます。

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon と Intel の両方に対応しています。rustup が正しいホストターゲットを
選びます。

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

続いて Rust を入れます。

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

このうち 2 つのパッケージは要となるもので、名前を挙げておく価値があります。

- **`libssl-dev` / `openssl-devel`** — Linux では HTTPS はシステムの TLS を
  使います。それがこれです。
- **`libgtk-3-dev` / `gtk3-devel`** — ネイティブの「開く」「保存」ダイアログ。

X11 と Wayland の両方に対応しています。ウィンドウ層は実行中のセッションを自分で
選ぶため、どちらも別途インストールする必要はありません。

---

## 2. コードを取得する

```sh
git clone https://github.com/CloudLandBeta/PowerRustCOBOL.git
cd PowerRustCOBOL
```

## 3. ビルドする

```sh
cargo build
```

> 初回のビルドはすべての crate を取得してワークスペースをコンパイルするため、
> 数分と、1.5 GB 程度の `target/` キャッシュを見込んでください。以降のビルドは
> 差分ビルドです。`cargo clean` すれば、いつでも領域を取り戻せます。

実際に実行する 2 つだけをビルドするには次のようにします。

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. IDE を起動する

```sh
cargo run -p cobolt-ide
```

日常的な利用では release ビルドをお勧めします。一度のコンパイルは遅くなります
が、使い心地は格段に滑らかです。

```sh
cargo run --release -p cobolt-ide
```

---

## テストを実行する

```sh
cargo test --workspace
```

forms エンジンでレンダリング経路をテストするには `render` フィーチャが必要です。

```sh
cargo test -p cobolt-forms --features render
```

---

## 成果物の出力先

| 成果物 | パス |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows では `.exe`） |
| CLI ランタイム／ビルダー | `target/release/rcrun`（Windows では `.exe`） |
| **あなたが** project からビルドしたアプリケーション | `<project>/bin/` と project の出力先フォルダ |

`rcrun build` でビルドしたアプリケーションは、単一の自己完結型実行ファイルです。
コンパイル済みのプログラム、その forms、そしてそれらが使用する asset-pack の
テーマを内部に埋め込むため、引き渡す先のマシンでその横に別途インストールする
ものは何もありません。

---

## トラブルシューティング

**`linker 'cc' not found`（Linux）** — `build-essential`（または
`@development-tools`）が入っていません。

**`link.exe not found`（Windows）** — Build Tools が「Desktop development with
C++」ワークロードなしでインストールされています。インストーラーを再実行して
チェックを入れてください。

**`Could not find directory of OpenSSL installation`（Linux）** —
`libssl-dev` / `openssl-devel` と `pkg-config` をインストールしてください。

**`error: package requires rustc 1.92 or newer`** — `rustup update stable` を
実行してください。

**IDE はビルドできるがウィンドウが開かない（Linux）** — `libxkbcommon-dev` が
インストールされているか、そして `$DISPLAY` か `$WAYLAND_DISPLAY` が設定されて
いるかを確認してください。素の TTY や X 転送のない SSH セッションには、開くべき
ディスプレイがありません。

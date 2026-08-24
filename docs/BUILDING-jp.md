<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL のビルド

まっさらなマシンから IDE が動くところまで、**Windows**・**Linux**・**macOS**
のそれぞれで。

ここに書かれていることは、どのプラットフォームでも同じ 3 ステップです —
ツールチェーンを入れ、クローンし、`cargo build`。OS によって変わるのは最初の
ステップだけです。

---

## ビルドに必要なもの

| 必要なもの | 理由 |
|---|---|
| **Rust**、stable チャンネル、**1.92 以降** | ワークスペース全体をビルドする |
| **Git** | リポジトリをクローンする |
| **C コンパイラとリンカ** | Rust が*どんな*バイナリにも必要とするリンカと、2 つの C 依存関係のため |
| **ネイティブ GUI ライブラリ**（Linux のみ） | ウィンドウの生成とネイティブのファイルダイアログ |

> **パッケージ版の IDE は Rust の要件を自分で確認します。** PowerRustCOBOL を
> ビルドするのではなく*使う*人はこのページを読みません。そのため IDE は初回
> 起動時に Rust を探し、この同じ **1.92** の下限を満たしていなければインス
> トールを提案します。バージョン番号はこのワークスペース自身のマニフェストから
> 読み取るので、両者が食い違うことはありません。開発者ガイドの §3 を参照して
> ください。

### C コンパイラについて

ツリー内の 2 つの crate が C のソースをコンパイルするため、C コンパイラは
実際に必要です。

- **`libsqlite3-sys`** — SQLite。C の amalgamation から同梱されています。これが
  COBOL データベースランタイムの SQLite サポートであり、エンドユーザーのマシンに
  システム側の SQLite をインストールしたりバージョンを合わせたりする必要は
  ありません。
- **`onig_sys`** — Oniguruma 正規表現エンジン。セマンティック検索の背後にある
  トークナイザが使用します。

ビルドが**必要としない**もの、そして決して呼び出さないもの:

> **C++ コンパイラなし · CMake なし · NASM なし · Python なし · Node なし ·
> JVM なし**

これは意図的であり、今後もそう保たれます。TLS は OS 自身のスタック（Windows は
schannel、macOS は Security.framework、Linux は OpenSSL）を純 Rust のバインディ
ング経由で使います。すべてのマシンで C・アセンブラ・CMake を要求するような暗号
ライブラリを同梱しないためです。トークナイザの C++ 版 suffix-array
（`esaxx_fast`）は、ここでモデルを学習させることがないので無効にしてあります。
そしてナレッジベースのインデックスは純 Rust の `redb` です。

どのプラットフォームでも、C コンパイラは Rust がすでに要求しているリンカと同じ
パッケージに含まれて届きます。したがって実際にはインストールするものは増えません。

---

## 1. ツールチェーンをインストールする

### Windows

1. **Visual Studio Build Tools** を **"Desktop development with C++"**
   ワークロード付きでインストールします —
   [ダウンロード](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   ワークロードの名前は C++ ですが、実際に入るのは Windows 上のあらゆる Rust
   ビルドがどのみち必要とするもの、すなわち `link.exe`、Windows SDK、そして
   上記 2 つの C 依存関係のための `cl.exe` です。ほかにダウンロードするものは
   ありません。

2. [rustup.rs](https://rustup.rs) から Rust をインストールします。MSVC
   ツールチェーンは自動的に選ばれます。

3. 通常の PowerShell プロンプトで確認します:

   ```powershell
   rustc --version
   cargo --version
   ```

リンカのフラグを手で設定する必要はありません。リポジトリの
`.cargo/config.toml` がすべてのオブジェクトを動的 CRT 上に配置済みで、これが
C 依存関係と Rust 自身のランタイムがリンク時に衝突するのを防いでいます。

### macOS

Xcode Command Line Tools を入れるだけです:

```sh
xcode-select --install
```

続いて Rust:

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

続いて Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

このうち 2 つのパッケージは要となるもので、名前を挙げておく価値があります。

- **`libssl-dev` / `openssl-devel`** — Linux では HTTPS がシステムの TLS を
  使います。それがこれです。
- **`libgtk-3-dev` / `gtk3-devel`** — ネイティブの開く／保存ダイアログ。

X11 と Wayland の両方に対応しています。ウィンドウ層が実行中のセッションを自分で
選ぶため、どちらも別途インストールするものではありません。

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

> 最初のビルドはすべての crate を取得してワークスペースをコンパイルするので、
> 数分と 1.5 GB 前後の `target/` キャッシュを見込んでください。以降のビルドは
> インクリメンタルです。領域を取り戻したくなったら `cargo clean` で回収できます。

実際に実行する 2 つだけをビルドするには:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. IDE を起動する

```sh
cargo run -p cobolt-ide
```

日常的に使うならリリースビルドを選んでください。コンパイルは一度だけ遅くなり
ますが、使い心地ははるかに滑らかです:

```sh
cargo run --release -p cobolt-ide
```

---

## テストを実行する

```sh
cargo test --workspace
```

フォームエンジンは、描画パスをテストするために `render` feature を必要とします:

```sh
cargo test -p cobolt-forms --features render
```

---

## 成果物の出力先

| 成果物 | パス |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows では `.exe`） |
| CLI ランタイム／ビルダー | `target/release/rcrun`（Windows では `.exe`） |
| **あなた**がプロジェクトからビルドしたアプリケーション | `<project>/bin/` とプロジェクトの出力先フォルダ |

`rcrun build` でビルドしたアプリケーションは、単体で完結する 1 つの実行ファイル
です。コンパイル済みのプログラム、フォーム、そしてそれらが使う asset pack の
テーマまで内包するので、渡した先のマシンにそれ以外へ入れるものはありません。

---

## トラブルシューティング

**`linker 'cc' not found`（Linux）** — `build-essential`（または
`@development-tools`）が入っていません。

**`link.exe not found`（Windows）** — Build Tools が "Desktop development with
C++" ワークロードなしでインストールされています。インストーラを再実行して
チェックを入れてください。

**`Could not find directory of OpenSSL installation`（Linux）** —
`libssl-dev` / `openssl-devel` と `pkg-config` をインストールしてください。

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`。

**IDE はビルドできるがウィンドウが開かない（Linux）** — `libxkbcommon-dev` が
インストールされていること、`$DISPLAY` または `$WAYLAND_DISPLAY` が設定されて
いることを確認してください。素の TTY や X 転送のない SSH セッションには、開く
ためのディスプレイがありません。

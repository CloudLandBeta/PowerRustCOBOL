<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL のビルド

まっさらなマシンから動く IDE まで — **Windows**、**Linux**、**macOS** で。

ここに書かれていることは、どのプラットフォームでも同じ 3 ステップです。ツールチェ
ーンを入れ、クローンし、`cargo build`。OS ごとに違うのは最初のステップだけです。

---

## ビルドに必要なもの

| 要件 | 理由 |
|---|---|
| **Rust** の stable チャンネル、**1.92 以降** | ワークスペース全体をビルドする |
| **Git** | リポジトリをクローンする |
| **C コンパイラーとリンカー** | *あらゆる*バイナリに Rust が必要とするリンカー、加えて 2 つの C 依存 |
| **ネイティブ GUI ライブラリ**（Linux のみ） | ウィンドウの生成とネイティブのファイルダイアログ |

> **パッケージ版の IDE は Rust の要件を自分で確認します。** PowerRustCOBOL を
> ビルドするのではなく*使う*人はこのページを読みません。そのため IDE は初回起動時に
> Rust を探し、この同じ **1.92** の下限を満たしていなければインストールを提案しま
> す。番号はこのワークスペース自身のマニフェストから読むので、両者が食い違うことは
> ありません。開発者ガイドの §3 を参照してください。

### C コンパイラーについて

ツリー内の 2 つの crate が C のソースをコンパイルするため、C コンパイラーは本当に
必要です。

- **`libsqlite3-sys`** — C のアマルガメーションから同梱される SQLite。COBOL の
  データベースランタイムの SQLite サポートであり、エンドユーザーのマシンにシステム
  の SQLite を入れたりバージョンを合わせたりする必要はありません。
- **`onig_sys`** — 鬼車（Oniguruma）正規表現エンジン。セマンティック検索の背後にあ
  るトークナイザーが使います。

ビルドが**必要としない**もの、そして決して呼び出さないもの:

> **C++ コンパイラー不要 · CMake 不要 · NASM 不要 · Python 不要 · Node 不要 ·
> JVM 不要**

これは意図的であり、今後もそう保たれます。TLS は OS 自身のスタック（Windows では
schannel、macOS では Security.framework、Linux では OpenSSL）を純 Rust のバイン
ディング経由で使います。すべてのマシンに C とアセンブラーと CMake を要求する同梱の
暗号ライブラリは使いません。トークナイザーの C++ 接尾辞配列（`esaxx_fast`）は、ここ
ではモデルを学習しないので無効にしてあります。そしてナレッジベースの索引は純 Rust の
`redb` です。

どのプラットフォームでも、C コンパイラーは Rust がもともと必要とするリンカーを提供
するのと同じパッケージに入っています。つまり実際には、追加で入れるものはありません。

---

## 1. ツールチェーンを入れる

### Windows

1. **Visual Studio Build Tools** を **"Desktop development with C++"**
   ワークロード付きでインストールします —
   [ダウンロード](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   ワークロードの名前は C++ ですが、届くものは Windows 上のあらゆる Rust ビルドが
   どのみち必要とするもの、すなわち `link.exe`、Windows SDK、そして上記 2 つの C
   依存のための `cl.exe` です。ほかにダウンロードするものはありません。

2. [rustup.rs](https://rustup.rs) から Rust をインストールします。MSVC ツールチェ
   ーンが自動的に選ばれます。

3. 通常の PowerShell プロンプトから確認します。

   ```powershell
   rustc --version
   cargo --version
   ```

手で設定するリンカーフラグはありません。リポジトリの `.cargo/config.toml` が、
すべてのオブジェクトを動的 CRT 上に置くよう既に指定しており、それが C 依存と Rust
自身のランタイムがリンク時に衝突するのを防いでいます。

### macOS

Xcode Command Line Tools を入れる — それだけです。

```sh
xcode-select --install
```

続いて Rust:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Apple Silicon と Intel のどちらも対応しています。rustup が正しいホストターゲットを
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

- **`libssl-dev` / `openssl-devel`** — Linux では HTTPS がシステムの TLS を使い、
  それがこれです。
- **`libgtk-3-dev` / `gtk3-devel`** — ネイティブの「開く」「保存」ダイアログ。

X11 と Wayland のどちらにも対応しており、ウィンドウ層が実際に動いているセッション
を選ぶので、どちらも別途インストールする必要はありません。

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

> 初回ビルドはすべての crate を取得してワークスペースをコンパイルするので、数分と
> 約 1.5 GB の `target/` キャッシュを見込んでください。以降のビルドは差分です。
> 容量を取り戻したくなったら `cargo clean` で回収できます。

実行する 2 つだけをビルドするには:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. IDE を起動する

```sh
cargo run -p cobolt-ide
```

日常的に使うなら release ビルドをおすすめします。一度のコンパイルは遅いものの、
使い心地ははるかに滑らかです。

```sh
cargo run --release -p cobolt-ide
```

---

## テストを走らせる

```sh
cargo test --workspace
```

フォームエンジンは描画経路をテストするために `render` フィーチャーを必要とします。

```sh
cargo test -p cobolt-forms --features render
```

---

## 成果物の置き場所

| 成果物 | パス |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows では `.exe`） |
| CLI ランタイム / ビルダー | `target/release/rcrun`（Windows では `.exe`） |
| **あなた**がプロジェクトからビルドしたアプリケーション | `<project>/bin/` とプロジェクトの出力先フォルダー |

`rcrun build` でビルドされたアプリケーションは、単体で完結する 1 つの実行ファイルで
す。コンパイル済みのプログラム、フォーム、そしてそれらが使うアセットパックのテーマ
を埋め込んでいるので、渡す相手のマシンに一緒に入れるものは何もありません。

---

## IDE を別の場所にインストールする — プラットフォーム SDK を同梱する

IDE の実行ファイルは、あなたがビルドするアプリケーションのようには自己完結して
**いません**。アプリケーションのビルドはプラットフォームの Rust ソースに対して本物
の `cargo build` を走らせるため、そのソースがビルドを行うマシンに存在していなければ
なりません。`cobolt-ide` だけをどこかにコピーすると Build は失敗し、探したフォルダ
ーをすべて挙げます — ツールチェーンは問題なく、単にソースが無いのです。

実行ファイルの隣に配置してください。ソースツリーから:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

これは `Cargo.toml`、`Cargo.lock`、`crates/` を `<install-dir>` に書き出し、あわせ
てフォームアプリケーションが必要とするアセット — テーマツリーとウィンドウアイコン
— も書き出します。ビルドされたアプリケーションがコンパイル対象とする 10 個の crate
は **8.6 MiB**、`assets/themes` を含めると配置後のツリーは約 **21 MiB** です。アイ
コンは省略可能ではありません。省くとフォームアプリケーションは一切コンパイルできま
せん。インストール先フォルダーに他のものが入っている場合は、`--sdk` を渡して
`<install-dir>/sdk/` に置いてください。IDE はどちらの配置も設定なしで見つけ、さらに
1 階層上と、macOS ではバンドルの `Resources` の中も探します。

そのマシンには依然として Rust のツールチェーンが必要で（Build は本物のコンパイルで
す）、最初のビルドでは依存 crate をレジストリからダウンロードするため、一度はネット
ワークアクセスが要ります。

> **注意。** まったく別の場所にあるチェックアウトを使う場合は、**Help → Platform
> SDK Location** でフォルダーを手動で指定してください。プロジェクトごとではなく
> マシンごとに記憶されるので、`cobolt.toml` に入って同僚に渡ってしまうことはありま
> せん。空欄にすれば自動探索に戻ります。

---

## トラブルシューティング

**`linker 'cc' not found`（Linux）** — `build-essential`（または
`@development-tools`）が入っていません。

**`link.exe not found`（Windows）** — Build Tools が "Desktop development with
C++" ワークロード無しでインストールされています。インストーラーを再実行してチェック
を入れてください。

**`Could not find directory of OpenSSL installation`（Linux）** —
`libssl-dev` / `openssl-devel` と `pkg-config` を入れてください。

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`。

**IDE はビルドできるがウィンドウが開かない（Linux）** — `libxkbcommon-dev` が入って
いること、`$DISPLAY` か `$WAYLAND_DISPLAY` が設定されていることを確認してください。
素の TTY や X 転送なしの SSH セッションには、開くべきディスプレイがありません。

.<<

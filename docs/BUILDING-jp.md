<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL をビルドする

まっさらなマシンから動く IDE まで。**Windows**、**Linux**、**macOS** のいずれでも。

ここに書かれていることは、どのプラットフォームでも同じ 3 ステップです —
ツールチェインを入れ、クローンし、`cargo build`。OS ごとに違うのは最初のステップ
だけです。

---

## ビルドに必要なもの

| 要件 | 理由 |
|---|---|
| **Rust**、stable チャンネル、**1.92 以降** | ワークスペース全体をビルドする |
| **Git** | リポジトリをクローンする |
| **C コンパイラとリンカ** | *あらゆる*バイナリに Rust が必要とするリンカ、および 2 つの C 依存 |
| **ネイティブ GUI ライブラリ**（Linux のみ） | ウィンドウの生成とネイティブのファイルダイアログ |

> **パッケージされた IDE は Rust の要件を自分で確認します。** PowerRustCOBOL を
> ビルドするのではなく*使う*人がこのページを読むことはありません。そこで IDE は
> 初回起動時に Rust を探し、この同じ **1.92** という下限を満たしていなければ
> インストールを提案します。その数値はこのワークスペース自身のマニフェストから
> 読み取るので、両者が食い違うことはありません。開発者ガイドの §3 を参照して
> ください。

### C コンパイラについて

ツリー内の 2 つのクレートが C のソースをコンパイルするため、C コンパイラは本当に
必要です:

- **`libsqlite3-sys`** — SQLite。その C アマルガメーションから同梱されています。
  これは COBOL データベースランタイムの SQLite サポートであり、エンドユーザーの
  マシンにシステムの SQLite をインストールしたりバージョンを合わせたりする必要は
  ありません。
- **`onig_sys`** — 正規表現エンジン Oniguruma。セマンティック検索の裏側にある
  トークナイザが使います。

ビルドが必要と**しない**もの、そして決して呼び出さないもの:

> **C++ コンパイラなし · CMake なし · NASM なし · Python なし · Node なし · JVM なし**

これは意図的であり、今後もそのまま維持します。TLS はバンドルされた暗号ライブラリ
ではなく、純 Rust のバインディング経由で OS 自身のスタック（Windows では
schannel、macOS では Security.framework、Linux では OpenSSL）を通ります。
バンドル方式なら、どのマシンでも C とアセンブラと CMake が要るからです。また
トークナイザの C++ 接尾辞配列（`esaxx_fast`）は、ここでモデルを学習させるものが
何もないため無効にしてあります。ナレッジベースのインデックスは純 Rust の `redb`
です。

どのプラットフォームでも、C コンパイラは Rust がすでに要求しているリンカを提供
するのと同じパッケージに入って届きます。したがって実際には、インストールするもの
は何も増えません。

---

## 1. ツールチェインをインストールする

### Windows

1. **Visual Studio Build Tools** を **"Desktop development with C++"**
   ワークロード付きでインストールします —
   [ダウンロード](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)。

   ワークロードの名前は C++ ですが、そこで手に入るのは Windows 上のあらゆる Rust
   ビルドがどのみち必要とするもの、すなわち `link.exe`、Windows SDK、そして
   上記 2 つの C 依存のための `cl.exe` です。ほかにダウンロードするものは
   ありません。

2. [rustup.rs](https://rustup.rs) から Rust をインストールします。MSVC
   ツールチェインは自動的に選ばれます。

3. 通常の PowerShell プロンプトから確認します:

   ```powershell
   rustc --version
   cargo --version
   ```

手作業で設定するリンカフラグはありません。リポジトリの `.cargo/config.toml` が
すでにすべてのオブジェクトを動的 CRT に載せており、これが C 依存と Rust 自身の
ランタイムがリンク時に衝突するのを防いでいます。

### macOS

Xcode Command Line Tools をインストールします — これで全部です:

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

このうち 2 つのパッケージは要となるもので、名前を挙げておく価値があります:

- **`libssl-dev` / `openssl-devel`** — Linux では HTTPS がシステムの TLS を使い
  ますが、その実体がこれです。
- **`libgtk-3-dev` / `gtk3-devel`** — ネイティブの「開く / 保存」ダイアログ。

X11 と Wayland の両方に対応しています。ウィンドウ層は動いているセッションの方を
選ぶので、どちらも別途インストールするものではありません。

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

> 最初のビルドはすべてのクレートを取得してワークスペースをコンパイルするので、
> 数分と 1.5 GB 程度の `target/` キャッシュを見込んでください。以降のビルドは
> 差分ビルドです。領域を取り戻したくなったら、いつでも `cargo clean` で回収でき
> ます。

実際に実行する 2 つだけをビルドするには:

```sh
cargo build --release -p cobolt-ide -p cobolt-cli
```

## 4. IDE を起動する

```sh
cargo run -p cobolt-ide
```

日常的に使うなら release ビルドをお勧めします — 一度のコンパイルは遅くなります
が、使い心地ははるかに滑らかです:

```sh
cargo run --release -p cobolt-ide
```

---

## テストを実行する

```sh
cargo test --workspace
```

フォームエンジンで描画経路をテストするには `render` フィーチャが必要です:

```sh
cargo test -p cobolt-forms --features render
```

---

## 成果物が置かれる場所

| 成果物 | パス |
|---|---|
| IDE | `target/release/cobolt-ide`（Windows では `.exe`） |
| CLI ランタイム / ビルダー | `target/release/rcrun`（Windows では `.exe`） |
| **あなた**がプロジェクトからビルドしたアプリケーション | `<project>/bin/` とプロジェクトの出力先フォルダ |

`rcrun build` でビルドしたアプリケーションは、単体で完結する 1 つの実行ファイル
です。コンパイル済みのプログラム、フォーム、そしてそれらが使うアセットパックの
テーマまで埋め込むので、渡した先のマシンで一緒にインストールするものは何もあり
ません。

---

## IDE を別の場所へインストールする — プラットフォーム SDK を同梱する

IDE の実行ファイルは、あなたがビルドしたアプリケーションのようには単体で完結して
**いません**。アプリケーションのビルドは、プラットフォームの Rust ソースに対して
本物の `cargo build` を走らせるため、ビルドするマシンにそのソースが存在していな
ければなりません。`cobolt-ide` だけをどこかにコピーすると Build は失敗し、探した
フォルダをすべて列挙します — ツールチェインは問題なく、単にソースが無いのです。

ソースは実行ファイルの隣に配置します。ソースツリーから:

```sh
cargo run -p cobolt-compiler --example stage_sdk -- <install-dir>
```

これは `Cargo.toml` と `crates/` を `<install-dir>` に書き出します — 6.0 MB、
ビルドされたアプリケーションがコンパイル対象とする 10 個のクレートです。
インストール先フォルダに他のものが入っている場合は、`--sdk` を渡して
`<install-dir>/sdk/` に置いてください。IDE はどちらの配置も設定なしで見つけ、
さらに 1 階層上と、macOS ではバンドルの `Resources` の中も探します。

そのマシンにはやはり Rust ツールチェインが必要で（Build は本物のコンパイルです）、
最初のビルドは依存クレートをレジストリからダウンロードするため、一度だけネット
ワークアクセスが必要です。

> **注記.** まったく別の場所にあるチェックアウトを使う場合は、
> **Help → Platform SDK Location** でフォルダを手動指定してください。これは
> プロジェクト単位ではなくマシン単位で記憶されるので、`cobolt.toml` に入って
> 同僚のところへ渡ることはありません。空欄にすれば自動検索に戻ります。

---

## トラブルシューティング

**`linker 'cc' not found`（Linux）** — `build-essential`（あるいは
`@development-tools`）がありません。

**`link.exe not found`（Windows）** — Build Tools が "Desktop development with
C++" ワークロードなしでインストールされています。インストーラを再実行して
チェックを入れてください。

**`Could not find directory of OpenSSL installation`（Linux）** —
`libssl-dev` / `openssl-devel` と `pkg-config` をインストールしてください。

**`error: package requires rustc 1.92 or newer`** — `rustup update stable`。

**IDE はビルドできるがウィンドウが開かない（Linux）** — `libxkbcommon-dev` が
インストールされているか、`$DISPLAY` または `$WAYLAND_DISPLAY` が設定されているか
を確認してください。素の TTY や X 転送のない SSH セッションには、開くべきディス
プレイがありません。

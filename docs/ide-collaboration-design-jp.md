<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL IDE — 共同作業（フェーズ B）— 設計

> **状態: 設計のみ。** ここに書かれていることはまだ何も実装されていません。フェーズ
> A（統制されたプロジェクトツリー、読み取り専用で青く表示される生成コード、ツール
> バーからのビルド/実行/デバッグ、コンパイル可否のゲート）は構築済みです。この文書
> は、**差し替え可能なバックエンド**の背後にある*複数開発者による共同作業*の層を設計
> します。ごく単純なローカルバックエンドから始めて、IDE を書き直すことなく Google
> Drive / GitHub / git へ育てられるようにするためです。

## 1. 目標と非目標

**目標**
- 複数の開発者が、それぞれ自分のマシンで同じプロジェクトを編集する。
- ある開発者が編集中のファイルは、ほかの開発者に対して**ロック**される。2 人目の
  開発者は開いた時点で**一度だけ警告**され、そのファイルを**読み取り専用**で受け取
  る。
- 最初の開発者がファイルを**解放**すると（エディターを閉じる／ロックを失う）、IDE は
  待っている開発者に読み書きでの再オープンを**提案**する。
- 開発者が確定した変更は、ほかの IDE インスタンスへ**そこそこ速やかに伝播**する。
- 転送手段は**差し替え可能**（ローカルのみ、ローカル git、GitHub、Google Drive、
  …）であり、プロジェクトごとに選べて、その上の IDE の振る舞いは同じ。

**非目標（明示的に対象外）**
- **文字単位の同時共同編集**（Google Docs / CRDT 方式）。採用するのは**ファイル単位
  の悲観的ロック**で、同時に書けるのは 1 人だけです。これは要件（「警告し、許可しな
  い……読み取り専用」）に一致し、COBOL のソースを正典のまま、差分の取りやすい形に保
  ちます。
- 常時稼働の専用サーバー（将来のバックエンドが自分で追加すると決めた場合を除く）。

---

## 2. 差し替え可能なバックエンド — `SyncBackend`

共同作業はすべて 1 つのトレイトを通ります。IDE の中核が特定のサービス名を口にする
ことはありません。バックエンドはプロジェクトごとに選ばれ、`cobolt.toml` に保存され
ます。

```rust
/// Identity of a developer in a collaboration session.
pub struct Peer { pub id: String, pub display_name: String }

/// A file lock held by a peer.
pub struct Lock { pub rel_path: String, pub holder: Peer, pub since: SystemTime }

/// Events a backend pushes up to the IDE (lock changes, remote edits, presence).
pub enum SyncEvent {
    LockAcquired(Lock),
    LockReleased { rel_path: String },
    FileChanged  { rel_path: String, by: Peer }, // remote saved a new version
    PeerJoined(Peer),
    PeerLeft(Peer),
    Error(String),
}

pub trait SyncBackend: Send {
    /// Human label + capabilities (does it support real-time push? locking?).
    fn capabilities(&self) -> Capabilities;

    /// Connect / open the shared project. Returns the initial lock table.
    fn connect(&mut self, project: &ProjectRef, me: &Peer) -> Result<Vec<Lock>, SyncError>;

    /// Try to take the write lock for `rel_path`. `Ok(None)` = granted;
    /// `Ok(Some(lock))` = already held by someone else (open read-only).
    fn try_lock(&mut self, rel_path: &str) -> Result<Option<Lock>, SyncError>;

    /// Release a lock we hold (on editor close / explicit unlock / app exit).
    fn release(&mut self, rel_path: &str) -> Result<(), SyncError>;

    /// Publish a new version of a file we hold the lock on.
    fn push_change(&mut self, rel_path: &str, bytes: &[u8]) -> Result<(), SyncError>;

    /// Fetch the latest bytes of a file (to refresh a read-only view).
    fn fetch(&mut self, rel_path: &str) -> Result<Vec<u8>, SyncError>;

    /// Drain backend events since the last poll (non-blocking). Backends that
    /// support push deliver promptly; polling backends synthesise these.
    fn poll(&mut self) -> Vec<SyncEvent>;
}

pub struct Capabilities {
    pub realtime: bool,      // true = push; false = the IDE must poll
    pub locking:  LockKind,  // Native | Advisory | None
    pub auth:     AuthKind,  // None | OAuth | Token | FsPermissions
}
```

- IDE が話す相手は `SyncBackend` だけで、毎フレーム `poll()` を吸い出して UI の状態
  に流し込みます。
- プッシュできないバックエンド（git、Drive）は、一定間隔（たとえば 2〜5 秒）でリモー
  トを確認し、合成したイベントを発行することで `poll()` を実装します。
- `Capabilities` によって UI が適応でき（たとえば「ロックは勧告的」「ほぼリアルタイ
  ム」といったバッジの表示）、バックエンドに機能が欠けている場合も**優雅に劣化**でき
  ます。

---

## 3. ロックと伝播のモデル（バックエンド非依存）

これは、どのバックエンドの上でも IDE が課す振る舞いです。

### ファイルを開く
1. IDE が `try_lock(rel)` を呼ぶ。
2. `Ok(None)` → **読み書き**で開き、タブに「自分がロック中」と印を付ける。
3. `Ok(Some(lock))` → **一度だけ警告**し（「`{file}` は `{holder}` が編集中です。
   読み取り専用で開きます」）、タブを**読み取り専用**で開き、`rel` を*待っている*こと
   を覚えておく。

### 編集と保存
- 書き込みロックを持つファイルを保存すると `push_change(rel, bytes)` を呼ぶ。
- バックエンドが伝播し、ほかの IDE は `FileChanged` を受け取る。そのファイルを読み
  取り専用で開いていれば表示を更新する（ツリーにも更新の印が付く）。

### 解放
- エディターを閉じたとき、アプリを終了したとき、明示的にロックを外したときに、IDE は
  `release(rel)` を呼ぶ。
- ほかの IDE は `LockReleased` を受け取る。`rel` を*待っていた*開発者には、IDE が
  **「`{file}` が空きました。編集しますか？」**と尋ね、「はい」ならロックを取り直して
  タブを読み書きに切り替える。

### クラッシュ・切断への備え
- ロックは**保持者とタイムスタンプ**、そして**リース TTL** を持ちます。バックエンド
  （または IDE）は TTL を過ぎた古いロックを失効させるので、落ちたエディターがファイル
  を永久に塞ぐことはありません。（生成コードはそもそもロック対象外です — 誰にとって
  も読み取り専用だからです。）

> 生成された COBOL とアセットは読み取り専用ないしバイナリです。ロックに参加するのは
> **Common Code**、**Forms**、**Documentation** だけです。

---

## 4. 4 つのバックエンド

4 つとも同じトレイトを実装します。違うのは*正典のプロジェクトがどこにあるか*と、
*ロックと変更がどう伝わるか*だけです。

| バックエンド | 正典のプロジェクト | ロック | 伝播 | 認証 | 備考 |
|---------|-------------------|---------|-------------|------|-------|
| **ローカルのみ** | ローカルのフォルダー | プロセス内のみ（1 台のマシン、複数ウィンドウ） | 直接 | なし | ごく単純な既定値。インフラ無しで体験全体を検証できる。マシン間の同期はない。 |
| **ローカル git** | git リポジトリ（共有パスや LAN 上のリモートでも可） | **勧告的なロック ref**（`refs/locks/<path>`、またはコミットして push する `.cobolt/locks/` ファイル） | 保存時に commit + push、ポーリング時に fetch | ssh/https の資格情報 | 慣れ親しんだ、監査できる履歴。「即時性」はポーリング間隔そのもの。 |
| **GitHub** | GitHub のリポジトリ | API 経由のロック用ブランチ／ファイル（あるいは **GraphQL/Issues** ベースのロック台帳）。プッシュ用に GitHub App の webhook を任意で | API 経由のコミット。webhook があればほぼリアルタイム、なければポーリング | **OAuth / PAT** | ホスティング済みで運用インフラ不要。レート制限あり。真のプッシュには小さな中継が要る。 |
| **Google Drive** | Drive のフォルダー | ロックファイル（`<path>.lock` のドキュメント）、または Drive の**コンテンツ制限／ファイルロック** API | 保存時に新しいリビジョンをアップロード。ポーリング時に Drive の**変更フィード**（またはプッシュ通知） | **OAuth** | 開発者以外との共有が容易。Drive の変更通知でほぼリアルタイムになる。 |

トレイトに織り込み済みの設計上の含意:
- **ロックが `LockKind` である**のは、git/Drive/GitHub が与えるのが*勧告的*なロック
  （全員が守る取り決め）であって、OS が強制するものではないからです。IDE は、
  *すべてのクライアントが PowerRustCOBOL IDE である限り*、勧告的ロックを正規のものと
  して扱います。
- **伝播は `realtime` かポーリングか**です。git はポーリング、Drive と GitHub は変更
  フィードや webhook でほぼリアルタイムにできます。ローカルのみは即時です。
- どのバックエンドもロック表を同じ形（小さな JSON/TOML の `locks` ドキュメント）で
  直列化するので、バックエンドを変えても IDE は変わりません。

---

## 5. 状態の置き場所

- **`cobolt.toml`** に `[collaboration]` セクションが加わります。
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **ロック台帳**: バックエンドが所有する小さなドキュメント 1 つ（リポジトリやフォル
  ダー内の `.cobolt/locks.toml`、あるいは API 側のレコード）で、形は次のとおりです。
  `[{ path, holder_id, holder_name, since, ttl }]`。
- **識別情報**: IDE の設定から得る `Peer { id, display_name }`（OAuth バックエンドの
  場合は認証済みアカウントから）。

---

## 6. IDE 側の統合点（フェーズ A で準備済み）

- ロックに参加する**ツリー**のカテゴリーはすでに分離されており（Forms / Common Code
  / Documentation）、**生成コードは誰にとっても読み取り専用**なので、ロックは不要で
  す。
- **エディター**はすでにタブごとの `read_only` フラグを備えています（今日は生成コード
  に使用）。共同作業の層はこれを「他人がロック中」に再利用し、加えて一度きりの警告と
  タブのバッジ（`🔒 by {name}`）を出します。
- 新しい **`SyncManager`**（`Box<dyn SyncBackend>` を保持）がアプリケーションに属し、
  毎フレーム吸い出されて、タブの読み取り専用状態、警告済みの集合、「待機中」の集合
  （再提案のため）、在席リストへ流れ込みます。

---

## 7. 段階的な展開

1. **B0 — ローカルのみのバックエンドと体験の全体。** `SyncBackend`、
   `SyncManager`、一度だけ警告／読み取り専用／再提案の流れ、タブのバッジを、プロセス
   内のごく単純なバックエンド（1 台のマシン上の複数の IDE ウィンドウ）に対して実装し
   ます。これでインフラ無しにモデルを実証できます。
2. **B1 — ローカル git のバックエンド。** 勧告的なロック ref、保存時の commit と
   push、ポーリング時の fetch。マシンをまたぐ最初の本物の共同作業です。
3. **B2 — GitHub のバックエンド。** API ベースのリポジトリとロック台帳。ほぼリアル
   タイムのための webhook 中継は任意。
4. **B3 — Google Drive のバックエンド。** OAuth、ロックファイル、Drive の変更フィー
   ド。

各フェーズはそれ単体で出荷できます。IDE の振る舞いはどのフェーズでも同一です。

---

## 8. 未解決の問い（B1 の前に決めること）

- **識別と認証の使い勝手**: 開発者は各バックエンドにどうサインインするのか（PAT を
  貼り付けるのか、ブラウザーでの OAuth フローか）、そして `Peer.id` はどうやって安定
  に保つのか。
- **粒度**: ファイル単位のロックだけにするか、それともフォームの `.cfrm` がロックさ
  れたときにその生成物も暗黙にロックするか。（推奨: `.cfrm` をロックする。生成された
  `.cbl` はすでに読み取り専用である。）
- 勧告的ロックが迂回されたとき（IDE の外で編集されたとき）の**衝突方針**: 最後に書い
  た人が勝ち、「ディスク上／リモートで変更されました」という目に見える帯を出す。
- **オフライン編集**: `push_change` をキューに積んで再接続時に突き合わせるのか、切断
  中は保存そのものを止めるのか。

---

## 9. なぜ悲観的ロックなのか（CRDT ではなく）

要件は明確です。2 人目の開発者は**警告されて阻止**され（読み取り専用）、ライブに統合
されてはならない、というものです。ファイル単位の悲観的ロックは、

- その要件にちょうど一致し、
- COBOL のソースを、きれいで査読できる成果物のまま保ち（本物の差分、CRDT のメタデー
  タなし）、
- 4 つのバックエンドの*どれ*の上でも同じ意味論で動き、
- リアルタイムの CRDT 収束よりはるかに複雑さもリスクも小さい。

本当の同時共同編集がいつか望まれたとしても、それは別の追加的なモードになります。この
設計を妨げるものではありません。

.<<

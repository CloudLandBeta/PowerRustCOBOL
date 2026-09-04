<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL IDE — 共同編集（フェーズ B）— 設計

> **ステータス: 設計のみ。** ここに書かれたものはまだ何も実装されていません。
> フェーズ A（管理されたプロジェクトツリー、青色で読み取り専用の生成コード、
> ツールバーのビルド／実行／デバッグ、そしてコンパイルが通るまで操作を止める
> 仕組み）はすでに構築済みです。本書は、**プラガブルなバックエンド**の裏側に
> 置く*複数開発者による共同編集*レイヤーを設計します。これにより、まずは単純な
> ローカルバックエンドから始めて、IDE を書き直すことなく Google Drive /
> GitHub / git へ育てていけます。

## 1. 目標と非目標

**目標**
- 複数の開発者が、それぞれ自分のマシンで同じプロジェクトを編集する。
- ある開発者が編集中のファイルは、他の開発者に対して**ロック**される。2 人目の
  開発者は開いたときに**一度だけ警告**され、そのファイルを**読み取り専用**で
  受け取る。
- 最初の開発者がファイルを**解放**したとき（エディタを閉じる／ロックを失う）、
  IDE は待機中の開発者に読み書きでの開き直しを**提案**する。
- ある開発者がコミットした変更は、他の IDE インスタンスへ十分に速やかに
  **伝播**される。
- トランスポートは**プラガブル**である — ローカルのみ、ローカル git、GitHub、
  Google Drive、… をプロジェクトごとに選択でき、その上に載る IDE の振る舞いは
  同じ。

**非目標（明示的に対象外）**
- **文字単位の同時共同編集**（Google ドキュメント／CRDT 方式）。採用するのは
  **ファイル単位の悲観的ロック** — 同時に書き込めるのは 1 ファイルにつき 1 人
  だけです。これは要件（「警告して許可しない … 読み取り専用」）にそのまま合致し、
  COBOL ソースを正本として保ち、差分も見やすく保ちます。
- 常時稼働の専用サーバー（将来のバックエンドが自ら追加を選ぶ場合を除く）。

---

## 2. プラガブルなバックエンド — `SyncBackend`

共同編集はすべて 1 つのトレイトを経由します。IDE の中核が特定のサービス名を
持つことはありません。バックエンドはプロジェクトごとに選択されます
（`cobolt.toml` に保存）。

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

- IDE が話しかける相手は `SyncBackend` だけで、毎フレーム `poll()` を汲み出して
  UI の状態へ流し込みます。
- プッシュできないバックエンド（git、Drive）は、一定間隔（例: 2〜5 秒）で
  リモートを確認し、合成したイベントを発行することで `poll()` を実装します。
- `Capabilities` によって UI は適応でき（例:「ロックは勧告的」「ほぼリアルタイム」
  といったバッジの表示）、バックエンドに機能が欠けていても**穏やかに機能を
  落とす**ことができます。

---

## 3. ロックと伝播のモデル（バックエンド非依存）

これは、どのバックエンドの上でも IDE が強制する振る舞いです。

### ファイルを開く
1. IDE が `try_lock(rel)` を呼ぶ。
2. `Ok(None)` → **読み書き**で開き、タブに「自分がロック中」と印を付ける。
3. `Ok(Some(lock))` → **一度だけ警告**し（「`{file}` は `{holder}` が編集中です
   — 読み取り専用で開きます」）、タブを**読み取り専用**で開いて、`rel` を
   *待機中*として覚えておく。

### 編集と保存
- 書き込みロックを持つファイルを保存すると `push_change(rel, bytes)` が
  呼ばれます。
- バックエンドが伝播し、他の IDE は `FileChanged` を受け取ります。そのファイルを
  読み取り専用で開いていれば表示を更新します（ツリーにも更新の印が付きます）。

### 解放
- エディタを閉じたとき、アプリを終了したとき、明示的にロックを解除したとき、
  IDE は `release(rel)` を呼びます。
- 他の IDE は `LockReleased` を受け取ります。`rel` を*待機中*の開発者には、IDE が
  プロンプトを出します: **「`{file}` が空きました — 編集しますか?」** → はい を
  選ぶとロックを取り直し、タブを読み書きに切り替えます。

### クラッシュ・切断時の安全策
- ロックは**保持者とタイムスタンプ**、そして**リース TTL** を持ちます。TTL を
  過ぎた古いロックはバックエンド（あるいは IDE 自身）が失効させるので、
  クラッシュしたエディタがファイルを永久に塞ぐことはありません。（生成コードは
  そもそもロック対象外です — 全員にとって読み取り専用だからです。）

> 生成された COBOL と Assets は読み取り専用かバイナリです。ロックに参加するのは
> **Common Code**、**Forms**、**Documentation** だけです。

---

## 4. 4 つのバックエンド

4 つとも同じトレイトを実装します。違うのは*正本のプロジェクトがどこにあるか*と、
*ロックと変更がどう運ばれるか*だけです。

| バックエンド | 正本のプロジェクト | ロック | 伝播 | 認証 | 備考 |
|--------------|--------------------|--------|------|------|------|
| **ローカルのみ** | ローカルフォルダー | プロセス内のみ（1 台のマシン、複数ウィンドウ） | 直接 | なし | 単純な既定値。インフラ ゼロで UX 全体を検証できる。マシンをまたぐ同期はなし。 |
| **ローカル git** | git リポジトリ（共有パスや LAN 上のリモートでも可） | **勧告的なロック ref**（`refs/locks/<path>`、またはコミットしてプッシュする `.cobolt/locks/` ファイル） | 保存時に commit + push、ポーリング時に fetch | ssh/https の認証情報 | 馴染みがあり監査もできる履歴。「即時性」はポーリング間隔しだい。 |
| **GitHub** | GitHub リポジトリ | API 経由のロック用ブランチ／ファイル（あるいは **GraphQL/Issues** ベースのロック台帳）、プッシュ用に GitHub App の webhook も任意で利用 | API 経由のコミット。webhook があればほぼリアルタイム、なければポーリング | **OAuth / PAT** | ホスティング済みで運用するインフラが不要。レート制限あり。真のプッシュには webhook 用の小さな中継が必要。 |
| **Google Drive** | Drive のフォルダー | ロックファイル（`<path>.lock` ドキュメント）、または Drive の**コンテンツ制限／ファイルロック** API | 保存時に新しいリビジョンをアップロード。ポーリング時に Drive の**変更フィード**（またはプッシュ通知） | **OAuth** | 開発者以外とも簡単に共有できる。Drive の変更通知でほぼリアルタイムになる。 |

トレイトに織り込まれた設計上の含意:
- **ロックが `LockKind` である**のは、git／Drive／GitHub が提供するのが
  *勧告的*なロック（全員が守る取り決め）であって、OS が強制するものではないから
  です。IDE は、*すべてのクライアントが PowerRustCOBOL の IDE である限り*、
  勧告的ロックを正式なものとして扱います。
- **伝播は `realtime` かポーリングか** — git はポーリング、Drive と GitHub は
  変更フィードや webhook でほぼリアルタイムにできます。ローカルのみは即時です。
- どのバックエンドもロック表を同じ形式（小さな JSON/TOML の `locks` ドキュメント）
  でシリアライズするので、バックエンドを切り替えても IDE は変わりません。

---

## 5. 状態はどこに置かれるか

- **`cobolt.toml`** に `[collaboration]` セクションが加わります:
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **ロック台帳**: バックエンドが所有する 1 つの小さなドキュメント（リポジトリや
  フォルダー内の `.cobolt/locks.toml`、あるいは API 側のレコード）。形式は次の
  とおりです:
  `[{ path, holder_id, holder_name, since, ttl }]`。
- **アイデンティティ**: IDE の設定から得られる `Peer { id, display_name }`
  （OAuth バックエンドでは、認証済みアカウントから）。

---

## 6. IDE 側の統合ポイント（フェーズ A ですでに準備済み）

- ロックに参加する**ツリー**のカテゴリーはすでに分離されており
  （Forms / Common Code / Documentation）、**生成コードは全員にとって読み取り
  専用**です — ロックは不要です。
- **エディタ**にはすでにタブ単位の `read_only` フラグがあります（現在は生成
  コードに使用）。共同編集レイヤーはこれを「他の人がロック中」にも再利用し、
  さらに一度だけの警告とタブのバッジ（`🔒 by {name}`）を加えます。
- 新しい **`SyncManager`**（`Box<dyn SyncBackend>` を保持）はアプリが所有し、
  毎フレーム、タブの読み取り専用状態、警告済み集合、「待機中」集合（再提案の
  プロンプト用）、在席リストへと汲み出されます。

---

## 7. 段階的な展開

1. **B0 — ローカルのみのバックエンドと UX 一式。** `SyncBackend`、
   `SyncManager`、一度だけの警告／読み取り専用／再提案の流れ、タブのバッジを
   実装します。相手はプロセス内の単純なバックエンドだけです（1 台のマシン上の
   複数 IDE ウィンドウ）。これでインフラ ゼロのままモデルを実証できます。
2. **B1 — ローカル git バックエンド。** 勧告的なロック ref + 保存時の
   commit/push + ポーリングでの fetch。マシンをまたぐ最初の本物の共同編集です。
3. **B2 — GitHub バックエンド。** API ベースのリポジトリとロック台帳。ほぼ
   リアルタイム化のための webhook 中継は任意。
4. **B3 — Google Drive バックエンド。** OAuth + ロックファイル + Drive の変更
   フィード。

各フェーズは単独で出荷できます。IDE の振る舞いはどのフェーズでも同一です。

---

## 8. 未解決の論点（B1 までに決めること）

- **アイデンティティ／認証の UX**: 各バックエンドで開発者はどうサインインするか
  （PAT を貼り付けるのか、ブラウザーでの OAuth フローか）、そして `Peer.id` は
  どうやって安定させるか。
- **粒度**: ロックはファイル単位だけにするか、それともフォームの `.cfrm` が
  ロックされたときにその生成物も暗黙にロックするか。（推奨: `.cfrm` をロックする。
  生成された `.cbl` はすでに読み取り専用です。）
- **競合ポリシー**: 勧告的ロックが迂回されたとき（IDE の外で誰かが編集したとき）
  は、最後に書いた人が勝ち、「ディスク上／リモートで変更されました」というバナー
  を目に見える形で出す。
- **オフライン編集**: `push_change` をキューに入れて再接続時に調停するか、切断中は
  保存自体を止めるか。

---

## 9. なぜ悲観的ロックなのか（CRDT ではなく）

要件は明確です。2 人目の開発者は**警告のうえ阻止**（読み取り専用）されるべきで
あり、その場でマージされるべきではありません。ファイル単位の悲観的ロックは、
- その要件にそのまま合致し、
- COBOL ソースを、きれいでレビューしやすい成果物のまま保ち（本物の差分、CRDT の
  メタデータなし）、
- 4 つのバックエンドの*どれ*の上でも同じ意味論で動き、
- リアルタイムな CRDT 収束に比べて、複雑さもリスクも桁違いに小さい。

本当の同時共同編集がいつか望まれるとしても、それは別建ての追加モードになります
— 本設計を妨げるものではありません。

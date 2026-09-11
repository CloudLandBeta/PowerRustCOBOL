<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# PowerRustCOBOL IDE —— 协作（阶段 B）—— 设计

> **状态：仅为设计。** 这里的内容尚未实现。阶段 A（受控的项目树、只读并以蓝色显示的
> 生成代码、工具栏上的构建/运行/调试，以及可编译性门控）已经建成；本文档设计的是位于
> **可插拔后端**之后的*多开发者协作*层，好让我们能从一个极简的本地后端起步，并成长到
> Google Drive / GitHub / git，而无需重写 IDE。

## 1. 目标与非目标

**目标**
- 多位开发者编辑同一个项目，各自在自己的机器上。
- 某个开发者正在编辑的文件对其他人**加锁**：第二位开发者在打开时会被**警告一次**，
  并拿到**只读**的文件。
- 当第一位开发者**释放**某个文件（关闭编辑器／失去锁）时，IDE 会**提议**等待中的
  开发者以读写方式重新打开它。
- 一位开发者提交的改动会以合理的及时程度**传播**到其他 IDE 实例。
- 传输方式是**可插拔的**——仅本地、本地 git、GitHub、Google Drive……按项目选择，
  其上的 IDE 行为完全一致。

**非目标（明确不在范围内）**
- **字符级的并发协同编辑**（Google Docs / CRDT 那一类）。我们采用**文件级的悲观
  锁**——同一时刻每个文件只有一个写者。这恰好符合需求（"警告并且不允许……只读"），
  也让 COBOL 源码保持权威且便于做差异比较。
- 一台自建的常驻服务器（除非将来某个后端自己决定加一个）。

---

## 2. 可插拔后端 —— `SyncBackend`

所有协作都经由一个 trait。IDE 的核心从不点名任何具体服务；后端按项目选择，并记录在
`cobolt.toml` 中。

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

- IDE 只与 `SyncBackend` 对话，并在每一帧把 `poll()` 排空到界面状态里。
- 无法主动推送的后端（git、Drive）通过按间隔（例如 2–5 秒）查询远端并发出合成事件来
  实现 `poll()`。
- `Capabilities` 让界面得以自我调整（例如显示"锁是建议性的"或"接近实时"的标记），也
  让我们在某个后端缺少某项能力时能够**平滑降级**。

---

## 3. 加锁与传播模型（与后端无关）

这是 IDE 在任何后端之上都强制执行的行为。

### 打开一个文件
1. IDE 调用 `try_lock(rel)`。
2. `Ok(None)` → 以**读写**方式打开；把标签页标记为"由我持锁"。
3. `Ok(Some(lock))` → **警告一次**（"`{file}` 正由 `{holder}` 编辑——将以只读方式
   打开"），以**只读**方式打开标签页，并记住我们正在*等待* `rel`。

### 编辑与保存
- 对持有写锁的文件执行保存会调用 `push_change(rel, bytes)`。
- 后端负责传播；其他 IDE 收到 `FileChanged`，若它们以只读方式打开着该文件，就刷新
  视图（项目树也会标记为已更新）。

### 释放
- 在关闭编辑器、退出应用或显式解锁时，IDE 调用 `release(rel)`。
- 其他 IDE 收到 `LockReleased`。对任何正在*等待* `rel` 的开发者，IDE 会弹出提示：
  **"`{file}` 现在空闲了——要编辑吗？"** → 选"是"就重新取得锁，并把标签页切换为
  读写。

### 崩溃与断线的安全性
- 锁带有**持有者与时间戳**以及一个**租约 TTL**。后端（或 IDE）会在 TTL 之后让过期的
  锁失效，这样崩溃的编辑器就不会永远占住一个文件。（生成代码从来不可加锁——它对所有
  人都是只读的。）

> 生成的 COBOL 和资源是只读或二进制的；只有 **Common Code**、**Forms** 和
> **Documentation** 参与加锁。

---

## 4. 四个后端

四者实现同一个 trait；它们的区别只在于*权威项目存放在哪里*，以及*锁与改动如何
传递*。

| 后端 | 权威项目所在 | 加锁 | 传播 | 认证 | 说明 |
|---------|-------------------|---------|-------------|------|-------|
| **仅本地** | 本地文件夹 | 仅进程内（一台机器、多个窗口） | 直接 | 无 | 极简的默认项。零基础设施即可验证完整体验；没有跨机器同步。 |
| **本地 git** | 一个 git 仓库（可以位于共享路径或局域网远端） | **建议性的锁 ref**（一个 `refs/locks/<path>`，或一个被提交并推送的 `.cobolt/locks/` 文件） | 保存时 commit + push；轮询时 fetch | ssh/https 凭据 | 熟悉且可审计的历史；"即时性"就等于轮询间隔。 |
| **GitHub** | 一个 GitHub 仓库 | 通过 API 使用锁分支或锁文件（或基于 **GraphQL/Issues** 的锁登记表）；可选用 GitHub App 的 webhook 实现推送 | 通过 API 提交；有 webhook 则接近实时，否则轮询 | **OAuth / PAT** | 托管式，无需自行运维；有速率限制；真正的推送需要一个小型中继。 |
| **Google Drive** | 一个 Drive 文件夹 | 一个锁文件（`<path>.lock` 文档），或 Drive 的**内容限制/文件锁定** API | 保存时上传一个新修订版；轮询时读取 Drive 的**变更订阅流**（或推送通知） | **OAuth** | 便于与非开发者共享；Drive 的变更通知能做到接近实时。 |

已经写进这个 trait 的设计含义：
- **锁是一个 `LockKind`**，因为 git/Drive/GitHub 给的是*建议性*的锁（一种大家共同
  遵守的约定），而不是由操作系统强制的。*只要所有客户端都是 PowerRustCOBOL IDE*，
  IDE 就把建议性的锁当作权威。
- **传播要么是 `realtime`，要么是轮询**——git 靠轮询；Drive 和 GitHub 借助变更订阅
  流和 webhook 可以接近实时；仅本地则是瞬时的。
- 每个后端都以同样的方式序列化锁表（一个小小的 JSON/TOML `locks` 文档），因此更换
  后端不会改变 IDE。

---

## 5. 状态存放在哪里

- **`cobolt.toml`** 新增一个 `[collaboration]` 小节：
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **锁登记表**：一个由后端拥有的小文档（仓库或文件夹中的 `.cobolt/locks.toml`，或
  一条 API 侧的记录），形状为：
  `[{ path, holder_id, holder_name, since, ttl }]`。
- **身份**：来自 IDE 设置的 `Peer { id, display_name }`（对 OAuth 后端，则来自已认证
  的账号）。

---

## 6. IDE 侧的集成点（阶段 A 已经铺好）

- 参与加锁的**项目树**类别已经隔离好（Forms / Common Code / Documentation），而
  **生成代码对所有人只读**——不需要锁。
- **编辑器**已经支持按标签页的 `read_only` 标志（今天用于生成代码）；协作层复用它来
  表示"被别人锁住"，再加上一次性的警告和一个标签页徽标（`🔒 by {name}`）。
- 一个新的 **`SyncManager`**（持有一个 `Box<dyn SyncBackend>`）归应用所有，每帧排空
  到：各标签页的只读状态、已警告集合、"等待中"集合（用于再次提议的提示），以及一份
  在线者列表。

---

## 7. 分阶段推出

1. **B0 —— 仅本地后端加上完整的使用体验。** 实现 `SyncBackend`、`SyncManager`、
   只警告一次／只读／再次提议的流程，以及标签页徽标——全部基于一个进程内的极简后端
   （一台机器上的多个 IDE 窗口）。这在零基础设施的情况下验证整个模型。
2. **B1 —— 本地 git 后端。** 建议性的锁 ref、保存时 commit 与 push、轮询时 fetch。
   第一次真正的跨机器协作。
3. **B2 —— GitHub 后端。** 基于 API 的仓库与锁登记表；可选的 webhook 中继以接近
   实时。
4. **B3 —— Google Drive 后端。** OAuth、锁文件与 Drive 的变更订阅流。

每个阶段都可以单独交付；IDE 的行为在各阶段完全一致。

---

## 8. 待解的问题（在 B1 之前定下来）

- **身份与认证的使用体验**：开发者如何在每个后端登录（粘贴 PAT，还是浏览器里的
  OAuth 流程），以及 `Peer.id` 如何保持稳定？
- **粒度**：只做文件级的锁，还是在某个窗体的 `.cfrm` 被锁住时，也隐式锁住它生成的
  输出？（建议：锁 `.cfrm`；它生成的 `.cbl` 本来就是只读的。）
- 当有人绕开建议性的锁（在 IDE 之外编辑）时的**冲突策略**：后写者胜出，并显示一条
  醒目的"磁盘上／远端已变更"横幅。
- **离线编辑**：把 `push_change` 排入队列并在重新连接时对账，还是在断开期间直接禁止
  保存？

---

## 9. 为什么用悲观锁（而不是 CRDT）

需求写得很明白：第二位开发者必须被**警告并挡住**（只读），而不是实时合并进来。文件级
的悲观锁：
- 与该需求严丝合缝，
- 让 COBOL 源码保持为一件干净、可评审的产物（真正的差异，没有 CRDT 元数据），
- 在四个后端中的*任何一个*之上都具有相同语义，并且
- 比实时 CRDT 收敛在复杂度和风险上都小得多。

如果将来真的想要并发协同编辑，那会是一个独立的、附加的模式——它并不妨碍当前这个
设计。

.<<

<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL IDE — Colaboração (Fase B) — Projeto

> **Status: apenas projeto.** Nada aqui foi implementado ainda. A Fase A (a
> árvore de projeto controlada, o código gerado em azul e somente leitura, os
> botões de compilar/executar/depurar da barra de ferramentas e o bloqueio das
> ações até que o projeto compile) já está construída; este documento projeta a
> camada de *colaboração entre vários desenvolvedores* atrás de um **backend
> plugável**, para que possamos começar com um backend local trivial e crescer
> rumo ao Google Drive / GitHub / git sem reescrever a IDE.

## 1. Objetivos e não objetivos

**Objetivos**
- Vários desenvolvedores editam o mesmo projeto, cada um na sua própria máquina.
- Um arquivo que está sendo editado por um desenvolvedor fica **bloqueado** para
  os demais: o segundo desenvolvedor é **avisado uma única vez** ao abrir e
  recebe o arquivo em **somente leitura**.
- Quando o primeiro desenvolvedor **libera** um arquivo (fecha o editor / perde o
  bloqueio), a IDE **oferece** aos desenvolvedores em espera reabri-lo em
  leitura/escrita.
- As alterações que um desenvolvedor confirma são **propagadas** às demais
  instâncias da IDE com razoável rapidez.
- O transporte é **plugável** — somente local, git local, GitHub, Google Drive,
  … escolhido por projeto, com o mesmo comportamento da IDE por cima.

**Não objetivos (explicitamente fora de escopo)**
- **Coedição concorrente no nível do caractere** (estilo Google Docs / CRDT).
  Usamos **bloqueio pessimista no nível do arquivo** — um único escritor por
  arquivo de cada vez. Isso atende ao requisito ("avisar e não permitir … somente
  leitura") e mantém o fonte COBOL como fonte da verdade e amigável a diffs.
- Um servidor próprio sempre ligado (a menos que um backend futuro decida
  acrescentar um).

---

## 2. O backend plugável — `SyncBackend`

Toda a colaboração passa por um único trait. O núcleo da IDE nunca nomeia um
serviço específico; o backend é escolhido por projeto (guardado em `cobolt.toml`).

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

- A IDE conversa apenas com `SyncBackend` e despeja `poll()` a cada frame no
  estado da interface.
- Os backends que não sabem fazer push (git, Drive) implementam `poll()`
  consultando o remoto em intervalos (por exemplo, 2–5 s) e emitindo eventos
  sintéticos.
- `Capabilities` permite que a interface se adapte (por exemplo, exibir emblemas
  de "bloqueio consultivo" ou "quase em tempo real") e nos deixa **degradar com
  elegância** quando falta um recurso a um backend.

---

## 3. O modelo de bloqueio e propagação (independente do backend)

Este é o comportamento que a IDE impõe sobre qualquer backend.

### Abrir um arquivo
1. A IDE chama `try_lock(rel)`.
2. `Ok(None)` → abre em **leitura/escrita**; a aba é marcada como "bloqueada por
   mim".
3. `Ok(Some(lock))` → **avisa uma única vez** ("`{file}` está sendo editado por
   `{holder}` — abrindo em somente leitura"), abre a aba em **somente leitura** e
   lembra que estamos *aguardando* por `rel`.

### Editar e salvar
- Salvar um arquivo com bloqueio de escrita chama `push_change(rel, bytes)`.
- O backend propaga; as outras IDEs recebem `FileChanged` e, se tiverem o arquivo
  aberto em somente leitura, atualizam a visualização (e a árvore o marca como
  atualizado).

### Liberar
- Ao fechar o editor, ao sair do aplicativo ou ao desbloquear explicitamente, a
  IDE chama `release(rel)`.
- As outras IDEs recebem `LockReleased`. Para qualquer desenvolvedor
  *aguardando* por `rel`, a IDE exibe um aviso: **"`{file}` está livre agora —
  editar?"** → Sim readquire o bloqueio e muda a aba para leitura/escrita.

### Segurança contra travamentos e desconexões
- Os bloqueios carregam **detentor e carimbo de tempo** e um **TTL de concessão**.
  Um backend (ou a própria IDE) expira um bloqueio obsoleto depois do TTL, para
  que um editor que travou não bloqueie um arquivo para sempre. (O código gerado
  nunca é bloqueável — ele é somente leitura para todos.)

> O COBOL gerado e os Assets são somente leitura ou binários; apenas **Common
> Code**, **Forms** e **Documentation** participam do bloqueio.

---

## 4. Os quatro backends

Todos os quatro implementam o mesmo trait; eles diferem apenas em *onde vive o
projeto de referência* e em *como os bloqueios e as mudanças trafegam*.

| Backend | Projeto de referência | Bloqueio | Propagação | Autenticação | Observações |
|---------|-----------------------|----------|------------|--------------|-------------|
| **Somente local** | a pasta local | apenas no processo (uma máquina, várias janelas) | direta | nenhuma | O padrão trivial. Valida toda a experiência sem nenhuma infraestrutura; sem sincronização entre máquinas. |
| **git local** | um repositório git (possivelmente num caminho compartilhado ou num remoto na LAN) | **refs de bloqueio consultivas** (um `refs/locks/<path>` ou um arquivo `.cobolt/locks/` commitado e enviado) | commit + push ao salvar; fetch a cada sondagem | credenciais ssh/https | Histórico familiar e auditável; a "imediatez" é o intervalo de sondagem. |
| **GitHub** | um repositório do GitHub | um branch ou arquivo de bloqueio via API (ou um registro de bloqueios baseado em **GraphQL/Issues**); webhooks opcionais de um GitHub App para o push | commits via API; webhook → quase tempo real, senão sondagem | **OAuth / PAT** | Hospedado, sem infraestrutura para manter; com limite de requisições; os webhooks precisam de um pequeno relé para push de verdade. |
| **Google Drive** | uma pasta do Drive | um arquivo de bloqueio (documento `<path>.lock`) ou a API de **restrição de conteúdo / bloqueio de arquivos** do Drive | enviar uma nova revisão ao salvar; **feed de mudanças** do Drive a cada sondagem (ou notificações push) | **OAuth** | Compartilhamento fácil para quem não é desenvolvedor; as notificações de mudança do Drive dão quase tempo real. |

Implicações de projeto já embutidas no trait:
- **O bloqueio é um `LockKind`** porque git, Drive e GitHub oferecem bloqueios
  *consultivos* (uma convenção que todos respeitam), não impostos pelo sistema
  operacional. A IDE trata bloqueios consultivos como autoritativos *enquanto
  todos os clientes forem a IDE do PowerRustCOBOL*.
- **A propagação é `realtime` ou por sondagem** — git é sondado; Drive e GitHub
  podem chegar quase ao tempo real com seus feeds de mudança e webhooks; somente
  local é instantâneo.
- Cada backend serializa a tabela de bloqueios do mesmo jeito (um pequeno
  documento `locks` em JSON/TOML), de modo que trocar de backend não muda a IDE.

---

## 5. Onde o estado vive

- **`cobolt.toml`** ganha uma seção `[collaboration]`:
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **Registro de bloqueios**: um único documento pequeno que pertence ao backend
  (`.cobolt/locks.toml` no repositório ou na pasta, ou um registro do lado da
  API), com este formato:
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identidade**: um `Peer { id, display_name }` vindo das configurações da IDE
  (e, nos backends OAuth, da conta autenticada).

---

## 6. Pontos de integração no lado da IDE (a Fase A já preparou)

- As categorias da **árvore** que participam do bloqueio já estão isoladas
  (Forms / Common Code / Documentation), e **o código gerado é somente leitura**
  para todos — nenhum bloqueio é necessário.
- O **editor** já suporta um sinalizador `read_only` por aba (usado hoje para o
  código gerado); a camada de colaboração o reaproveita para "bloqueado por outra
  pessoa", mais um aviso único e um emblema na aba (`🔒 by {name}`).
- Um novo **`SyncManager`** (que guarda um `Box<dyn SyncBackend>`) pertence ao
  aplicativo e é despejado a cada frame em: os estados de somente leitura das
  abas, o conjunto de avisos já emitidos, o conjunto "aguardando" (para o aviso
  de reoferta) e uma lista de presença.

---

## 7. Implantação em fases

1. **B0 — Backend somente local e toda a experiência de uso.** Implementar
   `SyncBackend`, `SyncManager`, o fluxo de aviso único / somente leitura /
   reoferta e os emblemas de aba — tudo contra um backend trivial dentro do
   processo (várias janelas da IDE numa mesma máquina). Isso prova o modelo sem
   nenhuma infraestrutura.
2. **B1 — Backend de git local.** Refs de bloqueio consultivas + commit e push ao
   salvar + fetch por sondagem. A primeira colaboração real entre máquinas.
3. **B2 — Backend do GitHub.** Repositório e registro de bloqueios via API; relé
   de webhooks opcional para quase tempo real.
4. **B3 — Backend do Google Drive.** OAuth + arquivos de bloqueio + feed de
   mudanças do Drive.

Cada fase é publicável por conta própria; o comportamento da IDE é idêntico em
todas elas.

---

## 8. Questões em aberto (a resolver antes da B1)

- **Experiência de identidade e autenticação**: como os desenvolvedores fazem
  login em cada backend (colar um PAT versus um fluxo OAuth no navegador), e como
  `Peer.id` se mantém estável?
- **Granularidade**: apenas bloqueios no nível do arquivo, ou também bloquear
  implicitamente a saída gerada de um formulário quando o seu `.cfrm` está
  bloqueado? (Recomendação: bloquear o `.cfrm`; o seu `.cbl` gerado já é somente
  leitura.)
- **Política de conflitos** quando os bloqueios consultivos são contornados
  (alguém edita fora da IDE): vence quem escreve por último, com um banner
  visível de "alterado em disco ou no remoto".
- **Edição offline**: enfileirar `push_change` e reconciliar ao reconectar, ou
  bloquear os salvamentos enquanto estiver desconectado?

---

## 9. Por que bloqueio pessimista (e não CRDT)

O requisito é explícito: um segundo desenvolvedor precisa ser **avisado e
bloqueado** (somente leitura), não mesclado ao vivo. O bloqueio pessimista no
nível do arquivo:
- atende exatamente a esse requisito,
- mantém o fonte COBOL como um artefato limpo e revisável (diffs reais, sem
  metadados de CRDT),
- funciona sobre *qualquer* um dos quatro backends com a mesma semântica, e
- é dramaticamente menos complexo e arriscado do que a convergência CRDT em tempo
  real.

Se algum dia se quiser coedição concorrente de verdade, ela seria um modo
separado e aditivo — não bloqueia este projeto.

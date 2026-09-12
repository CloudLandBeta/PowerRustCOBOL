<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# IDE PowerRustCOBOL — Colaboração (fase B) — Desenho

> **Estado: apenas desenho.** Nada do que está aqui está implementado ainda. A
> fase A (a árvore de projeto controlada, o código gerado a azul e só de leitura,
> compilar/executar/depurar na barra de ferramentas e o filtro de
> compilabilidade) já está construída; este documento desenha a camada de
> *colaboração multi-programador* por trás de um **backend encaixável**, para que
> possamos começar com um backend local trivial e crescer para Google Drive /
> GitHub / git sem reescrever o IDE.

## 1. Objetivos e não-objetivos

**Objetivos**
- Vários programadores editam o mesmo projeto, cada um na sua máquina.
- Um arquivo que está sendo editado por um programador fica **bloqueado** para
  os outros: o segundo programador é **avisado uma vez** ao abrir e recebe o
  arquivo **só de leitura**.
- Quando o primeiro programador **liberta** um arquivo (fecha o editor / perde o
  bloqueio), o IDE **oferece** aos programadores em espera uma reabertura em
  leitura/escrita.
- As alterações que um programador confirma são **propagadas** às outras
  instâncias do IDE com razoável prontidão.
- O transporte é **encaixável** — apenas local, git local, GitHub, Google Drive,
  … — selecionado por projeto, com o mesmo comportamento do IDE por cima.

**Não-objetivos (explicitamente fora de escopo)**
- **Co-edição concorrente ao nível do caráter** (estilo Google Docs / CRDT).
  Usamos **bloqueio pessimista ao nível do arquivo** — um escritor por arquivo
  de cada vez. Isto corresponde ao requisito («avisar e não permitir … só de
  leitura») e mantém a fonte COBOL autoritativa e amiga dos diffs.
- Um servidor próprio sempre ligado (a menos que um backend futuro decida
  acrescentar um).

---

## 2. O backend encaixável — `SyncBackend`

Toda a colaboração passa por um único trait. O núcleo do IDE nunca nomeia um
serviço concreto; o backend é escolhido por projeto (e guardado no
`cobolt.toml`).

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

- O IDE fala apenas com o `SyncBackend` e esvazia o `poll()` em cada frame para o
  estado da interface.
- Os backends que não conseguem empurrar alterações (git, Drive) implementam o
  `poll()` consultando o remoto num intervalo (p. ex. 2–5 s) e emitindo eventos
  sintéticos.
- As `Capabilities` permitem que a interface se adapte (p. ex. mostrar emblemas
  de «bloqueio consultivo» ou «quase tempo real») e que **degrademos com
  elegância** quando falta uma funcionalidade a um backend.

---

## 3. O modelo de bloqueio e propagação (independente do backend)

Este é o comportamento que o IDE impõe sobre qualquer backend.

### Abrir um arquivo
1. O IDE chama `try_lock(rel)`.
2. `Ok(None)` → abrir em **leitura/escrita**; marcar a aba como «bloqueado
   por mim».
3. `Ok(Some(lock))` → **avisar uma vez** («`{file}` está sendo editado por
   `{holder}` — abrindo só de leitura»), abrir a aba **só de leitura** e
   lembrar que estamos *à espera* de `rel`.

### Editar e salvar
- Salvar um arquivo com bloqueio de escrita chama `push_change(rel, bytes)`.
- O backend propaga; os outros IDE recebem `FileChanged` e, se tiverem o arquivo
  aberto só de leitura, atualizam a vista (e a árvore marca-o como atualizado).

### Libertar
- Ao fechar o editor, ao sair da aplicação ou ao desbloquear explicitamente, o
  IDE chama `release(rel)`.
- Os outros IDE recebem `LockReleased`. A qualquer programador que estivesse *à
  espera* de `rel`, o IDE mostra um aviso: **«`{file}` está agora livre —
  editar?»** → Sim volta a adquirir o bloqueio e passa a aba para
  leitura/escrita.

### Segurança perante falhas e desligamentos
- Os bloqueios trazem um **detentor e uma marca temporal** e um **TTL de
  arrendamento**. O backend (ou o IDE) expira um bloqueio obsoleto findo o TTL,
  para que um editor que travamentou não possa bloquear um arquivo para sempre. (O
  código gerado nunca é bloqueável — é só de leitura para toda a gente.)

> O COBOL gerado e os recursos são só de leitura ou binários; só participam no
> bloqueio **Common Code**, **Forms** e **Documentation**.

---

## 4. Os quatro backends

Os quatro implementam o mesmo trait; diferem apenas em *onde vive o projeto de
referência* e em *como viajam os bloqueios e as alterações*.

| Backend | Projeto de referência | Bloqueio | Propagação | Autenticação | Notas |
|---------|-------------------|---------|-------------|------|-------|
| **Apenas local** | a pasta local | apenas dentro do processo (uma máquina, várias janelas) | direta | nenhuma | O valor por padrão trivial. Valida toda a experiência sem infraestrutura; sem sincronização entre máquinas. |
| **git local** | um repositório git (possivelmente num caminho partilhado ou num remoto da LAN) | **refs de bloqueio consultivas** (uma `refs/locks/<path>` ou um arquivo `.cobolt/locks/` confirmado e enviado) | commit + push ao salvar; fetch ao sondar | credenciais ssh/https | Histórico familiar e auditável; a «imediatez» é o intervalo de sondagem. |
| **GitHub** | um repositório do GitHub | um ramo ou arquivo de bloqueio via a API (ou um registro de bloqueios baseado em **GraphQL/Issues**); webhooks opcionais de GitHub App para o empurrão | commits via a API; webhook → quase tempo real, caso contrário sondagem | **OAuth / PAT** | Alojado, sem infraestrutura para manter; com limites de taxa; os webhooks precisam de um pequeno relé para empurrão verdadeiro. |
| **Google Drive** | uma pasta do Drive | um arquivo de bloqueio (documento `<path>.lock`) ou a API de **restrição de conteúdo / bloqueio de arquivos** do Drive | enviar uma nova revisão ao salvar; o **feed de alterações** do Drive ao sondar (ou notificações push) | **OAuth** | Fácil de partilhar com quem não programa; as notificações de alteração do Drive dão quase tempo real. |

Implicações de desenho já cozidas no trait:
- **O bloqueio é `LockKind`** porque git/Drive/GitHub dão bloqueios *consultivos*
  (uma convenção que todos respeitam), não impostos pelo sistema operacional. O IDE
  trata os bloqueios consultivos como autoritativos *enquanto todos os clientes
  forem um IDE PowerRustCOBOL*.
- **A propagação é `realtime` ou sondada** — o git é sondado; o Drive e o GitHub
  podem ir quase em tempo real com os seus feeds de alterações e webhooks; apenas
  local é instantâneo.
- Cada backend serializa a tabela de bloqueios da mesma forma (um pequeno
  documento `locks` em JSON/TOML), de modo que mudar de backend não muda o IDE.

---

## 5. Onde vive o estado

- O **`cobolt.toml`** ganha uma secção `[collaboration]`:
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **Registro de bloqueios**: um único documento pequeno de que o backend é dono
  (`.cobolt/locks.toml` no repositório ou na pasta, ou um registro do lado da
  API), com esta forma:
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identidade**: um `Peer { id, display_name }` vindo das definições do IDE (e,
  para os backends OAuth, da conta autenticada).

---

## 6. Pontos de integração no IDE (a fase A já os preparou)

- As categorias da **árvore** que participam no bloqueio já estão isoladas
  (Forms / Common Code / Documentation), e o **código gerado é só de leitura**
  para toda a gente — não precisa de bloqueio.
- O **editor** já suporta uma marca `read_only` por aba (usada hoje para o
  código gerado); a camada de colaboração reaproveita-a para «bloqueado por
  outra pessoa», mais um aviso único e um emblema na aba (`🔒 by {name}`).
- Um novo **`SyncManager`** (que contém um `Box<dyn SyncBackend>`) pertence à
  aplicação e é esvaziado em cada frame para: os estados de só-leitura dos
  abas, o conjunto de avisos já dados, o conjunto «à espera» (para o aviso
  de reoferta) e uma lista de presença.

---

## 7. Lançamento por fases

1. **B0 — Backend apenas local e toda a experiência de utilização.** Implementar
   `SyncBackend`, `SyncManager`, o fluxo de aviso único / só leitura / reoferta e
   os emblemas de aba — tudo contra um backend trivial dentro do processo
   (várias janelas do IDE numa máquina). Isto prova o modelo sem infraestrutura
   nenhuma.
2. **B1 — Backend de git local.** Refs de bloqueio consultivas, commit e push ao
   salvar, e fetch ao sondar. A primeira colaboração real entre máquinas.
3. **B2 — Backend do GitHub.** Repositório e registro de bloqueios via a API; relé
   de webhooks opcional para quase tempo real.
4. **B3 — Backend do Google Drive.** OAuth, arquivos de bloqueio e feed de
   alterações do Drive.

Cada fase pode ser entregue por si só; o comportamento do IDE é idêntico em todas
elas.

---

## 8. Perguntas em aberto (a resolver antes de B1)

- **Experiência de identidade e autenticação**: como é que um programador inicia
  sessão em cada backend (colar um PAT versus um fluxo OAuth no navegador), e
  como se mantém estável o `Peer.id`?
- **Granularidade**: apenas bloqueios ao arquivo, ou também bloquear
  implicitamente a saída gerada de um formulário quando o seu `.cfrm` está
  bloqueado? (Recomendação: bloquear o `.cfrm`; o seu `.cbl` gerado já é só de
  leitura.)
- **Política de conflitos** quando alguém contorna os bloqueios consultivos
  (edita fora do IDE): ganha quem escreve por último, com um aviso visível de
  «alterado em disco/no remoto».
- **Edição offline**: pôr `push_change` numa fila e reconciliar ao reconectar, ou
  impedir gravações enquanto se está desligado?

---

## 9. Porquê bloqueio pessimista (e não CRDT)

O requisito é explícito: um segundo programador tem de ser **avisado e
bloqueado** (só de leitura), não fundido ao vivo. O bloqueio pessimista ao nível
do arquivo:
- corresponde exatamente a esse requisito,
- mantém a fonte COBOL um artefato limpo e revisível (diffs a sério, sem
  metadados de CRDT),
- funciona sobre *qualquer* um dos quatro backends com a mesma semântica, e
- é dramaticamente menos complexo e arriscado do que a convergência CRDT em tempo
  real.

Se alguma vez se quiser co-edição concorrente a sério, seria um modo separado e
aditivo — não bloqueia este desenho.

.<<

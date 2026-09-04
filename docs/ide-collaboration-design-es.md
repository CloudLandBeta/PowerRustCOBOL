<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL IDE — Colaboración (Fase B) — Diseño

> **Estado: solo diseño.** Todavía no hay nada implementado. La Fase A (el árbol
> de proyecto controlado, el código generado en azul y de solo lectura, los
> botones de compilar/ejecutar/depurar de la barra de herramientas y el bloqueo
> de acciones hasta que el proyecto compile) ya está construida; este documento
> diseña la capa de *colaboración multidesarrollador* detrás de un **backend
> conectable**, para poder empezar con un backend local trivial y crecer hacia
> Google Drive / GitHub / git sin reescribir el IDE.

## 1. Objetivos y no objetivos

**Objetivos**
- Varios desarrolladores editan el mismo proyecto, cada uno en su propia máquina.
- Un archivo que está editando un desarrollador queda **bloqueado** para los
  demás: al segundo desarrollador se le **avisa una sola vez** al abrirlo y
  recibe el archivo en **solo lectura**.
- Cuando el primer desarrollador **libera** un archivo (cierra el editor / pierde
  el bloqueo), el IDE **ofrece** a los desarrolladores en espera volver a abrirlo
  en lectura/escritura.
- Los cambios que confirma un desarrollador se **propagan** a las demás
  instancias del IDE con razonable prontitud.
- El transporte es **conectable** — solo local, git local, GitHub, Google Drive,
  … se elige por proyecto, con el mismo comportamiento del IDE por encima.

**No objetivos (explícitamente fuera de alcance)**
- **Coedición concurrente a nivel de carácter** (al estilo de Google Docs / CRDT).
  Usamos **bloqueo pesimista a nivel de archivo** — un único escritor por archivo
  cada vez. Esto se ajusta al requisito («avisar y no permitir … solo lectura») y
  mantiene el fuente COBOL como fuente de verdad y apto para diffs.
- Un servidor propio siempre activo (salvo que un backend futuro decida añadir
  uno).

---

## 2. El backend conectable — `SyncBackend`

Toda la colaboración pasa por un único trait. El núcleo del IDE nunca nombra un
servicio concreto; el backend se elige por proyecto (se guarda en `cobolt.toml`).

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

- El IDE habla únicamente con `SyncBackend` y vuelca `poll()` en cada frame al
  estado de la interfaz.
- Los backends que no saben hacer push (git, Drive) implementan `poll()`
  consultando el remoto a intervalos (p. ej. 2–5 s) y emitiendo eventos
  sintéticos.
- `Capabilities` permite que la interfaz se adapte (p. ej. mostrar insignias de
  «bloqueo consultivo» o «casi en tiempo real») y nos deja **degradar con
  elegancia** cuando a un backend le falta una prestación.

---

## 3. El modelo de bloqueo y propagación (independiente del backend)

Este es el comportamiento que el IDE impone sobre cualquier backend.

### Abrir un archivo
1. El IDE llama a `try_lock(rel)`.
2. `Ok(None)` → se abre en **lectura/escritura**; la pestaña se marca como
   «bloqueada por mí».
3. `Ok(Some(lock))` → se **avisa una sola vez** («`{file}` lo está editando
   `{holder}` — se abre en solo lectura»), la pestaña se abre en **solo lectura**
   y se recuerda que estamos *en espera* de `rel`.

### Editar y guardar
- Al guardar un archivo con bloqueo de escritura se llama a
  `push_change(rel, bytes)`.
- El backend lo propaga; los demás IDE reciben `FileChanged` y, si tienen el
  archivo abierto en solo lectura, refrescan la vista (y el árbol lo marca como
  actualizado).

### Liberar
- Al cerrar el editor, al salir de la aplicación o al desbloquear explícitamente,
  el IDE llama a `release(rel)`.
- Los demás IDE reciben `LockReleased`. A cualquier desarrollador *en espera* de
  `rel`, el IDE le muestra un aviso: **«`{file}` ya está libre — ¿editarlo?»** →
  Sí vuelve a adquirir el bloqueo y cambia la pestaña a lectura/escritura.

### Seguridad ante caídas y desconexiones
- Los bloqueos llevan **titular y marca de tiempo** y un **TTL de arrendamiento**.
  El backend (o el propio IDE) caduca un bloqueo obsoleto pasado el TTL, de modo
  que un editor que se ha caído no pueda bloquear un archivo para siempre. (El
  código generado nunca es bloqueable — es de solo lectura para todo el mundo.)

> El COBOL generado y los Assets son de solo lectura o binarios; solo **Common
> Code**, **Forms** y **Documentation** participan en el bloqueo.

---

## 4. Los cuatro backends

Los cuatro implementan el mismo trait; solo se diferencian en *dónde vive el
proyecto de referencia* y en *cómo viajan los bloqueos y los cambios*.

| Backend | Proyecto de referencia | Bloqueo | Propagación | Autenticación | Notas |
|---------|------------------------|---------|-------------|---------------|-------|
| **Solo local** | la carpeta local | solo en proceso (una máquina, varias ventanas) | directa | ninguna | El predeterminado trivial. Valida toda la experiencia sin infraestructura alguna; sin sincronización entre máquinas. |
| **git local** | un repositorio git (quizá en una ruta compartida o un remoto en la LAN) | **refs de bloqueo consultivas** (un `refs/locks/<path>` o un archivo `.cobolt/locks/` confirmado y enviado) | commit + push al guardar; fetch en cada sondeo | credenciales ssh/https | Historial familiar y auditable; la «inmediatez» es el intervalo de sondeo. |
| **GitHub** | un repositorio de GitHub | una rama o un archivo de bloqueo vía la API (o un registro de bloqueos basado en **GraphQL/Issues**); webhooks opcionales de una GitHub App para el push | commits vía la API; webhook → casi tiempo real; si no, sondeo | **OAuth / PAT** | Alojado, sin infraestructura que mantener; con límite de peticiones; los webhooks necesitan un pequeño relé para lograr push de verdad. |
| **Google Drive** | una carpeta de Drive | un archivo de bloqueo (documento `<path>.lock`) o la API de **restricción de contenido / bloqueo de archivos** de Drive | subir una revisión nueva al guardar; **feed de cambios** de Drive en cada sondeo (o notificaciones push) | **OAuth** | Fácil de compartir con quien no programa; las notificaciones de cambios de Drive dan casi tiempo real. |

Implicaciones de diseño ya incorporadas al trait:
- **El bloqueo es un `LockKind`** porque git, Drive y GitHub ofrecen bloqueos
  *consultivos* (una convención que todos respetan), no impuestos por el sistema
  operativo. El IDE trata los bloqueos consultivos como autoritativos *mientras
  todos los clientes sean el IDE de PowerRustCOBOL*.
- **La propagación es `realtime` o por sondeo** — git se sondea; Drive y GitHub
  pueden ir casi en tiempo real con sus feeds de cambios y sus webhooks; solo
  local es instantáneo.
- Cada backend serializa la tabla de bloqueos igual (un pequeño documento `locks`
  en JSON/TOML), de modo que cambiar de backend no cambia el IDE.

---

## 5. Dónde vive el estado

- **`cobolt.toml`** gana una sección `[collaboration]`:
  ```toml
  [collaboration]
  backend = "local" | "git" | "github" | "gdrive"
  # backend-specific:
  remote  = "git@github.com:team/app.git"   # git/github
  folder  = "0B...drive-folder-id"           # gdrive
  poll_ms = 3000                              # for polled backends
  ```
- **Registro de bloqueos**: un único documento pequeño que posee el backend
  (`.cobolt/locks.toml` en el repositorio o la carpeta, o un registro del lado de
  la API), con esta forma:
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identidad**: un `Peer { id, display_name }` tomado de la configuración del
  IDE (y, en los backends OAuth, de la cuenta autenticada).

---

## 6. Puntos de integración en el IDE (la Fase A ya los dejó preparados)

- Las categorías del **árbol** que participan en el bloqueo ya están aisladas
  (Forms / Common Code / Documentation), y **el código generado es de solo
  lectura** para todo el mundo — no necesita bloqueo.
- El **editor** ya admite un indicador `read_only` por pestaña (usado hoy para el
  código generado); la capa de colaboración lo reutiliza para «bloqueado por otra
  persona», más un aviso único y una insignia en la pestaña (`🔒 by {name}`).
- Un nuevo **`SyncManager`** (que contiene un `Box<dyn SyncBackend>`) pertenece a
  la aplicación y se vuelca en cada frame a: los estados de solo lectura de las
  pestañas, el conjunto de avisos ya emitidos, el conjunto «en espera» (para el
  aviso de reoferta) y una lista de presencia.

---

## 7. Despliegue por fases

1. **B0 — Backend solo local y toda la experiencia de usuario.** Implementar
   `SyncBackend`, `SyncManager`, el flujo de aviso único / solo lectura /
   reoferta y las insignias de pestaña — todo contra un backend trivial en
   proceso (varias ventanas del IDE en una misma máquina). Esto demuestra el
   modelo sin infraestructura alguna.
2. **B1 — Backend de git local.** Refs de bloqueo consultivas + commit y push al
   guardar + fetch por sondeo. La primera colaboración real entre máquinas.
3. **B2 — Backend de GitHub.** Repositorio y registro de bloqueos vía la API;
   relé de webhooks opcional para casi tiempo real.
4. **B3 — Backend de Google Drive.** OAuth + archivos de bloqueo + feed de
   cambios de Drive.

Cada fase se puede publicar por sí sola; el comportamiento del IDE es idéntico en
todas ellas.

---

## 8. Preguntas abiertas (a resolver antes de B1)

- **Experiencia de identidad y autenticación**: ¿cómo inician sesión los
  desarrolladores en cada backend (pegar un PAT frente a un flujo OAuth en el
  navegador) y cómo se mantiene estable `Peer.id`?
- **Granularidad**: ¿solo bloqueos a nivel de archivo, o también bloquear
  implícitamente la salida generada de un formulario cuando su `.cfrm` está
  bloqueado? (Recomendación: bloquear el `.cfrm`; su `.cbl` generado ya es de
  solo lectura.)
- **Política de conflictos** cuando se saltan los bloqueos consultivos (alguien
  edita fuera del IDE): gana quien escribe último, con un banner visible de
  «cambiado en disco o en el remoto».
- **Edición sin conexión**: ¿encolar `push_change` y reconciliar al reconectar, o
  bloquear el guardado mientras no haya conexión?

---

## 9. Por qué bloqueo pesimista (y no CRDT)

El requisito es explícito: al segundo desarrollador hay que **avisarlo y
bloquearlo** (solo lectura), no fusionar sus cambios en vivo. El bloqueo
pesimista a nivel de archivo:
- cumple ese requisito exactamente,
- mantiene el fuente COBOL como un artefacto limpio y revisable (diffs reales,
  sin metadatos de CRDT),
- funciona sobre *cualquiera* de los cuatro backends con la misma semántica, y
- es muchísimo menos complejo y arriesgado que la convergencia CRDT en tiempo
  real.

Si algún día se quiere coedición concurrente de verdad, sería un modo aparte y
aditivo — no bloquea este diseño.

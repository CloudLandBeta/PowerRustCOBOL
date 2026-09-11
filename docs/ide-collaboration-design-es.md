<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

<!-- powerrustcobol: 1.65.124 -->

# IDE PowerRustCOBOL — Colaboración (fase B) — Diseño

> **Estado: solo diseño.** Nada de lo que hay aquí está implementado todavía. La
> fase A (el árbol de proyecto controlado, el código generado en azul y de solo
> lectura, compilar/ejecutar/depurar desde la barra de herramientas y el filtro
> de compilabilidad) ya está construida; este documento diseña la capa de
> *colaboración multidesarrollador* detrás de un **backend enchufable**, de modo
> que podamos empezar con un backend local trivial y crecer hacia Google Drive /
> GitHub / git sin reescribir el IDE.

## 1. Objetivos y no-objetivos

**Objetivos**
- Varios desarrolladores editan el mismo proyecto, cada uno en su máquina.
- Un fichero que un desarrollador está editando queda **bloqueado** para los
  demás: al segundo desarrollador se le **avisa una vez** al abrirlo y lo recibe
  en **solo lectura**.
- Cuando el primer desarrollador **libera** un fichero (cierra el editor / pierde
  el bloqueo), el IDE **ofrece** a los desarrolladores en espera reabrirlo en
  lectura/escritura.
- Los cambios que un desarrollador confirma se **propagan** a las demás
  instancias del IDE con razonable prontitud.
- El transporte es **enchufable** —solo local, git local, GitHub, Google Drive,
  …— seleccionable por proyecto, con el mismo comportamiento del IDE encima.

**No-objetivos (explícitamente fuera de alcance)**
- **Coedición concurrente a nivel de carácter** (estilo Google Docs / CRDT).
  Usamos **bloqueo pesimista a nivel de fichero**: un escritor por fichero a la
  vez. Esto encaja con el requisito («avisar y no permitir … solo lectura») y
  mantiene la fuente COBOL autorizada y amigable con los diffs.
- Un servidor propio siempre activo (salvo que un backend futuro decida añadir
  uno).

---

## 2. El backend enchufable — `SyncBackend`

Toda la colaboración pasa por un único trait. El núcleo del IDE nunca nombra un
servicio concreto; el backend se elige por proyecto (y se guarda en
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

- El IDE habla únicamente con `SyncBackend` y vacía `poll()` en cada frame hacia
  el estado de la interfaz.
- Los backends que no pueden empujar cambios (git, Drive) implementan `poll()`
  consultando el remoto en un intervalo (p. ej. 2–5 s) y emitiendo eventos
  sintéticos.
- `Capabilities` permite que la interfaz se adapte (p. ej. mostrar distintivos de
  «bloqueo consultivo» o «casi en tiempo real») y que **degrademos con
  elegancia** cuando a un backend le falte una función.

---

## 3. El modelo de bloqueo y propagación (independiente del backend)

Este es el comportamiento que el IDE impone sobre cualquier backend.

### Abrir un fichero
1. El IDE llama a `try_lock(rel)`.
2. `Ok(None)` → abrir en **lectura/escritura**; marcar la pestaña como
   «bloqueada por mí».
3. `Ok(Some(lock))` → **avisar una vez** («`{file}` lo está editando
   `{holder}`: se abre en solo lectura»), abrir la pestaña en **solo lectura** y
   recordar que estamos *esperando* por `rel`.

### Editar y guardar
- Guardar un fichero con bloqueo de escritura llama a `push_change(rel, bytes)`.
- El backend lo propaga; los demás IDE reciben `FileChanged` y, si tienen el
  fichero abierto en solo lectura, refrescan la vista (y el árbol lo marca como
  actualizado).

### Liberar
- Al cerrar el editor, salir de la aplicación o desbloquear explícitamente, el
  IDE llama a `release(rel)`.
- Los demás IDE reciben `LockReleased`. A cualquier desarrollador que estuviera
  *esperando* por `rel`, el IDE le muestra un aviso: **«`{file}` ya está libre,
  ¿editarlo?»** → Sí vuelve a tomar el bloqueo y cambia la pestaña a
  lectura/escritura.

### Seguridad ante caídas y desconexiones
- Los bloqueos llevan un **titular y una marca de tiempo** y un **TTL de
  arrendamiento**. El backend (o el IDE) caduca un bloqueo obsoleto pasado el TTL
  para que un editor que se ha caído no pueda bloquear un fichero para siempre.
  (El código generado nunca es bloqueable: es de solo lectura para todo el
  mundo.)

> El COBOL generado y los recursos son de solo lectura o binarios; solo
> participan en el bloqueo **Common Code**, **Forms** y **Documentation**.

---

## 4. Los cuatro backends

Los cuatro implementan el mismo trait; solo se diferencian en *dónde vive el
proyecto de referencia* y en *cómo viajan los bloqueos y los cambios*.

| Backend | Proyecto de referencia | Bloqueo | Propagación | Autenticación | Notas |
|---------|-------------------|---------|-------------|------|-------|
| **Solo local** | la carpeta local | solo dentro del proceso (una máquina, varias ventanas) | directa | ninguna | El valor por defecto trivial. Valida toda la experiencia sin infraestructura; sin sincronización entre máquinas. |
| **git local** | un repositorio git (posiblemente en una ruta compartida o un remoto de la LAN) | **refs de bloqueo consultivas** (una `refs/locks/<path>` o un fichero `.cobolt/locks/` confirmado y empujado) | commit + push al guardar; fetch al sondear | credenciales ssh/https | Historial familiar y auditable; la «inmediatez» es el intervalo de sondeo. |
| **GitHub** | un repositorio de GitHub | una rama o fichero de bloqueo vía la API (o un registro de bloqueos basado en **GraphQL/Issues**); webhooks opcionales de GitHub App para el empuje | commits vía la API; webhook → casi tiempo real, si no, sondeo | **OAuth / PAT** | Alojado, sin infraestructura que mantener; con límites de tasa; los webhooks necesitan un pequeño relé para empuje real. |
| **Google Drive** | una carpeta de Drive | un fichero de bloqueo (documento `<path>.lock`) o la API de **restricción de contenido / bloqueo de ficheros** de Drive | subir una revisión nueva al guardar; el **feed de cambios** de Drive al sondear (o notificaciones push) | **OAuth** | Fácil de compartir con quien no programa; las notificaciones de cambio de Drive dan casi tiempo real. |

Implicaciones de diseño ya horneadas en el trait:
- **El bloqueo es `LockKind`** porque git/Drive/GitHub dan bloqueos *consultivos*
  (una convención que todos respetan), no impuestos por el sistema operativo. El
  IDE trata los bloqueos consultivos como autorizados *mientras todos los
  clientes sean un IDE PowerRustCOBOL*.
- **La propagación es `realtime` o por sondeo**: git se sondea; Drive y GitHub
  pueden ir casi en tiempo real con sus feeds de cambios y webhooks; solo local
  es instantáneo.
- Cada backend serializa la tabla de bloqueos de la misma forma (un pequeño
  documento `locks` en JSON/TOML), de modo que cambiar de backend no cambia el
  IDE.

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
- **Registro de bloqueos**: un único documento pequeño del que es dueño el
  backend (`.cobolt/locks.toml` en el repositorio o la carpeta, o un registro del
  lado de la API), con esta forma:
  `[{ path, holder_id, holder_name, since, ttl }]`.
- **Identidad**: un `Peer { id, display_name }` tomado de los ajustes del IDE (y,
  para los backends OAuth, de la cuenta autenticada).

---

## 6. Puntos de integración en el IDE (la fase A ya los preparó)

- Las categorías del **árbol** que participan en el bloqueo ya están aisladas
  (Forms / Common Code / Documentation), y el **código generado es de solo
  lectura** para todo el mundo, así que no necesita bloqueo.
- El **editor** ya admite un indicador `read_only` por pestaña (hoy se usa para
  el código generado); la capa de colaboración lo reutiliza para «bloqueado por
  otra persona», además de un aviso único y un distintivo en la pestaña
  (`🔒 by {name}`).
- Un nuevo **`SyncManager`** (que contiene un `Box<dyn SyncBackend>`) pertenece a
  la aplicación y se vacía en cada frame hacia: los estados de solo lectura de
  las pestañas, el conjunto de avisos ya dados, el conjunto «en espera» (para el
  aviso de reoferta) y una lista de presencia.

---

## 7. Despliegue por fases

1. **B0 — Backend solo local y toda la experiencia de uso.** Implementar
   `SyncBackend`, `SyncManager`, el flujo de aviso único / solo lectura /
   reoferta y los distintivos de pestaña, todo contra un backend trivial dentro
   del proceso (varias ventanas del IDE en una máquina). Esto prueba el modelo
   sin infraestructura alguna.
2. **B1 — Backend de git local.** Refs de bloqueo consultivas, commit y push al
   guardar, y fetch al sondear. La primera colaboración real entre máquinas.
3. **B2 — Backend de GitHub.** Repositorio y registro de bloqueos vía la API;
   relé de webhooks opcional para casi tiempo real.
4. **B3 — Backend de Google Drive.** OAuth, ficheros de bloqueo y feed de cambios
   de Drive.

Cada fase se puede entregar por sí sola; el comportamiento del IDE es idéntico en
todas ellas.

---

## 8. Preguntas abiertas (a resolver antes de B1)

- **Experiencia de identidad y autenticación**: ¿cómo inicia sesión un
  desarrollador en cada backend (pegar un PAT frente a un flujo OAuth en el
  navegador) y cómo se mantiene estable `Peer.id`?
- **Granularidad**: ¿solo bloqueos por fichero, o también bloquear implícitamente
  la salida generada de un formulario cuando su `.cfrm` está bloqueado?
  (Recomendación: bloquear el `.cfrm`; su `.cbl` generado ya es de solo lectura.)
- **Política de conflictos** cuando alguien esquiva los bloqueos consultivos
  (edita fuera del IDE): gana quien escribe último, con un cartel visible de
  «cambiado en disco/en el remoto».
- **Edición sin conexión**: ¿encolar `push_change` y reconciliar al reconectar, o
  impedir guardar mientras se está desconectado?

---

## 9. Por qué bloqueo pesimista (y no CRDT)

El requisito es explícito: al segundo desarrollador hay que **avisarle y
bloquearle** (solo lectura), no fusionarle en vivo. El bloqueo pesimista a nivel
de fichero:
- encaja exactamente con ese requisito,
- mantiene la fuente COBOL como un artefacto limpio y revisable (diffs de verdad,
  sin metadatos de CRDT),
- funciona sobre *cualquiera* de los cuatro backends con la misma semántica, y
- es drásticamente menos complejo y arriesgado que la convergencia CRDT en tiempo
  real.

Si alguna vez se quisiera coedición concurrente de verdad, sería un modo
separado y aditivo: no bloquea este diseño.

.<<

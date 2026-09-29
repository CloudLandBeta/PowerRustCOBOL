      *> ───────────────────────────────────────────────────────────
      *>  This code was generated automatically by PowerRustCOBOL RAD.
      *>
      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time
      *>  you interact with the Form Designer, so manual edits are lost.
      *>  Edit the form and its event handlers in the Form Designer
      *>  instead.
      *>
      *>  PowerRustCOBOL may change the structure of this generated code
      *>  at any time — without breaking your code's functionality — for
      *>  reasons such as performance improvements, new observability
      *>  features, and bug fixes.
      *>
      *>  PowerRustCOBOL and its components are distributed under the
      *>  Apache 2.0 License.
      *> ───────────────────────────────────────────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROMPTS-FORM.

       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       REPOSITORY.
           CLASS RUST-BOOL IS "Rust.bool"
           CLASS RUST-CHAR IS "Rust.char"
           CLASS RUST-I8 IS "Rust.i8"
           CLASS RUST-I16 IS "Rust.i16"
           CLASS RUST-I32 IS "Rust.i32"
           CLASS RUST-I64 IS "Rust.i64"
           CLASS RUST-I128 IS "Rust.i128"
           CLASS RUST-ISIZE IS "Rust.isize"
           CLASS RUST-U8 IS "Rust.u8"
           CLASS RUST-U16 IS "Rust.u16"
           CLASS RUST-U32 IS "Rust.u32"
           CLASS RUST-U64 IS "Rust.u64"
           CLASS RUST-U128 IS "Rust.u128"
           CLASS RUST-USIZE IS "Rust.usize"
           CLASS RUST-F32 IS "Rust.f32"
           CLASS RUST-F64 IS "Rust.f64"
           CLASS RUST-STR IS "Rust.str"
           CLASS RUST-UNIT IS "Rust.unit"
           CLASS RUST-STRING IS "Rust.String"
           CLASS RUST-OSSTRING IS "Rust.OsString"
           CLASS RUST-OSSTR IS "Rust.OsStr"
           CLASS RUST-CSTRING IS "Rust.CString"
           CLASS RUST-CSTR IS "Rust.CStr"
           CLASS RUST-PATH IS "Rust.Path"
           CLASS RUST-PATHBUF IS "Rust.PathBuf"
           CLASS RUST-VEC IS "Rust.Vec"
           CLASS RUST-VECDEQUE IS "Rust.VecDeque"
           CLASS RUST-LINKEDLIST IS "Rust.LinkedList"
           CLASS RUST-HASHMAP IS "Rust.HashMap"
           CLASS RUST-BTREEMAP IS "Rust.BTreeMap"
           CLASS RUST-HASHSET IS "Rust.HashSet"
           CLASS RUST-BTREESET IS "Rust.BTreeSet"
           CLASS RUST-BINARYHEAP IS "Rust.BinaryHeap"
           CLASS RUST-OPTION IS "Rust.Option"
           CLASS RUST-RESULT IS "Rust.Result"
           CLASS RUST-BOX IS "Rust.Box"
           CLASS RUST-RC IS "Rust.Rc"
           CLASS RUST-ARC IS "Rust.Arc"
           CLASS RUST-WEAK IS "Rust.Weak"
           CLASS RUST-CELL IS "Rust.Cell"
           CLASS RUST-REFCELL IS "Rust.RefCell"
           CLASS RUST-MUTEX IS "Rust.Mutex"
           CLASS RUST-RWLOCK IS "Rust.RwLock"
           CLASS RUST-COW IS "Rust.Cow"
           CLASS RUST-DURATION IS "Rust.Duration"
           CLASS RUST-INSTANT IS "Rust.Instant"
           CLASS RUST-SYSTEMTIME IS "Rust.SystemTime"
           CLASS RUST-RANGE IS "Rust.Range".
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SETTINGS-FILE ASSIGN TO WS-SETTINGS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS SET-NAME
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT TOPICS-FILE ASSIGN TO WS-TOPICS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TOP-ID
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT PROMPTS-FILE ASSIGN TO WS-PROMPTS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PRM-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT OLDPRM-FILE ASSIGN TO WS-OLDPRM-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS OLD-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT MPSEED-FILE ASSIGN TO WS-MPSEED-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.

       DATA DIVISION.
       FILE SECTION.
       FD  SETTINGS-FILE IS GLOBAL.
       01  SETTINGS-REC.
           05 SET-NAME          PIC X(20).
           05 SET-VALUE         PIC X(200).
       FD  TOPICS-FILE IS GLOBAL.
       01  TOPIC-REC.
           05 TOP-ID            PIC X(16).
           05 TOP-NAME          PIC X(40).
           05 TOP-PROMPT        PIC X(1000).
           05 TOP-CREATED       PIC X(14).
      *>   "Y" for a topic the sample installer made, so it can be removed.
           05 TOP-SAMPLE        PIC X.
       FD  PROMPTS-FILE IS GLOBAL.
      *>   Every version of each topic's system prompt (R48), and of the main
      *>   prompt under "*MAIN": data/prompt-versions.idx. A topic's active
      *>   text is also its TOP-PROMPT, which is what the chat reads.
       01  PROMPT-REC.
           05 PRM-KEY.
              10 PRM-TOPIC      PIC X(16).
              10 PRM-VERSION    PIC 9(4).
           05 PRM-CREATED       PIC X(14).
           05 PRM-ACTIVE        PIC X.
           05 PRM-TEXT          PIC X(32000).
      *>   The versions' file before 1.70.332, read once to move them over.
       FD  OLDPRM-FILE IS GLOBAL.
       01  OLD-PROMPT-REC.
           05 OLD-KEY.
              10 OLD-TOPIC      PIC X(16).
              10 OLD-VERSION    PIC 9(4).
           05 OLD-CREATED       PIC X(14).
           05 OLD-ACTIVE        PIC X.
           05 OLD-TEXT          PIC X(1000).
       FD  MPSEED-FILE IS GLOBAL.
       01  MPSEED-REC           PIC X(1000).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'PROMPTS-FORM'.

      *>── DataGrid Dg-List CSV export ──────────────────────────
       01 WS-Dg-List-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-Dg-List-CSV-STATUS  PIC 9       VALUE 0.

      *>── Timer: Tmr-Lang ──────────────────────────────────────────
       01 WS-Tmr-Lang-INTERVAL   PIC 9(8) VALUE 1000.
       01 WS-Tmr-Lang-ENABLED    PIC 9    VALUE 1.
       01 WS-Tmr-Lang-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS              GLOBAL PIC XX VALUE "00".
       01 WS-DATA-DIR        GLOBAL PIC X(200).
       01 WS-SET-NAME        GLOBAL PIC X(20).
       01 WS-SET-VALUE       GLOBAL PIC X(200).
       01 WS-NOW             GLOBAL PIC X(21).
       01 WS-EOF             GLOBAL PIC X.
       01 WS-OK              GLOBAL PIC X(4).
       01 WS-SETTINGS-PATH  GLOBAL PIC X(240).
       01 WS-TOPICS-PATH  GLOBAL PIC X(240).
       01 WS-PROMPTS-PATH  GLOBAL PIC X(240).
       01 WS-CUR-TOPIC       GLOBAL PIC X(16).
       01 WS-VER-COUNT       GLOBAL PIC 9(4) VALUE 0.
       01 WS-VERSIONS        GLOBAL.
          05 WS-VER          PIC 9(4) OCCURS 200.
       01 WS-LAST-VER        GLOBAL PIC 9(4) VALUE 0.
       01 WS-ACTIVE-VER      GLOBAL PIC 9(4) VALUE 0.
       01 WS-PICKED          GLOBAL PIC 9(4) VALUE 0.
       01 WS-CONFIRM-VER     GLOBAL PIC 9(4) VALUE 0.
       01 WS-INDEX           GLOBAL PIC S9(4).
       01 WS-I               GLOBAL PIC 9(4).
       01 WS-TEXT            GLOBAL PIC X(32000).
      *>   Which prompt the screen edits: "T" the open topic's, "M" the main
      *>   prompt - every instruction, for every topic - kept in the same
      *>   version file under the topic id "*MAIN" (operator, 2026-09-28).
       01 WS-SCOPE           GLOBAL PIC X VALUE "T".
       01 WS-TOPIC-ID        GLOBAL PIC X(16).
       01 WS-OLDPRM-PATH     GLOBAL PIC X(240).
       01 WS-MPSEED-PATH     GLOBAL PIC X(240).
       01 WS-SAMPLES-DIR     GLOBAL PIC X(240).
       01 WS-EMPTY           GLOBAL PIC X.
       01 WS-SX-B            GLOBAL PIC 9(6).
       01 WS-LN-BUF          GLOBAL PIC X(1000).
       01 WS-LN-REV          GLOBAL PIC X(1000).
       01 WS-LN-LEN          GLOBAL PIC 9(4).
       01 WS-ROW             GLOBAL PIC X(300).
       01 WS-LINE            GLOBAL PIC X(200).
       01 WS-VNUM            GLOBAL PIC Z(3)9.
      *>   The CRUD tabs: the version being edited (0 = a new one), the
      *>   grid cell a click landed on, and the confirmation dialog's text.
       01 WS-EDIT-VER        GLOBAL PIC 9(4) VALUE 0.
       01 WS-SAVE-VER        GLOBAL PIC 9(4) VALUE 0.
       01 WS-SAVE-OK         GLOBAL PIC X VALUE "Y".
       01 WS-ROW-NO          GLOBAL PIC 9(4) VALUE 0.
       01 WS-COL-NO          GLOBAL PIC 9(4) VALUE 0.
       01 WS-KEY             GLOBAL PIC X(10).
       01 WS-PREVIEW         GLOBAL PIC X(60).
       01 WS-TABS            GLOBAL PIC X(270).
       01 WS-QUESTION        GLOBAL PIC X(300).
       01 WS-ANSWER          GLOBAL PIC X.
       01 WS-FS-ERR          GLOBAL PIC XX.
      *>   The interface texts (spec 071 R44): one row per text, one column
      *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
      *>   only the values are translated.
       01 WS-LANG            GLOBAL PIC XX VALUE "en".
       01 WS-LANG-NOW        GLOBAL PIC XX VALUE "en".
       01 WS-LANG-IX         GLOBAL PIC 9 VALUE 1.
       01 WS-TX-I            GLOBAL PIC 9(4).
       01 PC-TEXT-DATA       GLOBAL.
      *>   OPEN-TOPIC-FIRST
          05 FILLER PIC X(130) VALUE "Open a topic first.".
          05 FILLER PIC X(130) VALUE "Abra um tópico primeiro.".
          05 FILLER PIC X(130) VALUE "Primero abre un tema.".
          05 FILLER PIC X(130) VALUE "Ouvrez d'abord un sujet.".
          05 FILLER PIC X(130) VALUE "先にトピックを開いてください。".
          05 FILLER PIC X(130) VALUE "请先打开一个主题。".
      *>   PROMPT-OF
          05 FILLER PIC X(130) VALUE "Prompt - &1".
          05 FILLER PIC X(130) VALUE "Prompt - &1".
          05 FILLER PIC X(130) VALUE "Prompt - &1".
          05 FILLER PIC X(130) VALUE "Prompt - &1".
          05 FILLER PIC X(130) VALUE "プロンプト - &1".
          05 FILLER PIC X(130) VALUE "提示词 - &1".
      *>   PICK-VERSION-FIRST
          05 FILLER PIC X(130) VALUE "Pick a version in the list first.".
          05 FILLER PIC X(130) VALUE "Escolha uma versão na lista primeiro.".
          05 FILLER PIC X(130) VALUE "Primero elige una versión de la lista.".
          05 FILLER PIC X(130) VALUE "Choisissez d'abord une version dans la liste.".
          05 FILLER PIC X(130) VALUE "先に一覧からバージョンを選んでください。".
          05 FILLER PIC X(130) VALUE "请先在列表中选择一个版本。".
      *>   EDITING-V
          05 FILLER PIC X(130) VALUE "Editing v&1".
          05 FILLER PIC X(130) VALUE "Editando a v&1".
          05 FILLER PIC X(130) VALUE "Editando la v&1".
          05 FILLER PIC X(130) VALUE "Modification de la v&1".
          05 FILLER PIC X(130) VALUE "v&1 を編集中".
          05 FILLER PIC X(130) VALUE "正在编辑 v&1".
      *>   ALREADY-ACTIVE
          05 FILLER PIC X(130) VALUE "That version is already the active one.".
          05 FILLER PIC X(130) VALUE "Essa versão já é a ativa.".
          05 FILLER PIC X(130) VALUE "Esa versión ya es la activa.".
          05 FILLER PIC X(130) VALUE "Cette version est déjà la version active.".
          05 FILLER PIC X(130) VALUE "そのバージョンはすでに有効です。".
          05 FILLER PIC X(130) VALUE "该版本已经是当前版本。".
      *>   CONFIRM-ACTIVATE
          05 FILLER PIC X(130) VALUE "Make v&1 the active prompt?".
          05 FILLER PIC X(130) VALUE "Tornar a v&1 o prompt ativo?".
          05 FILLER PIC X(130) VALUE "¿Hacer de la v&1 el prompt activo?".
          05 FILLER PIC X(130) VALUE "Faire de la v&1 le prompt actif ?".
          05 FILLER PIC X(130) VALUE "v&1 を有効なプロンプトにしますか？".
          05 FILLER PIC X(130) VALUE "将 v&1 设为当前提示词？".
      *>   NOW-ACTIVE
          05 FILLER PIC X(130) VALUE "v&1 is the active prompt.".
          05 FILLER PIC X(130) VALUE "A v&1 é o prompt ativo.".
          05 FILLER PIC X(130) VALUE "La v&1 es el prompt activo.".
          05 FILLER PIC X(130) VALUE "La v&1 est le prompt actif.".
          05 FILLER PIC X(130) VALUE "v&1 が有効なプロンプトになりました。".
          05 FILLER PIC X(130) VALUE "v&1 现在是当前提示词。".
      *>   PROMPT-EMPTY
          05 FILLER PIC X(130) VALUE "The prompt is empty.".
          05 FILLER PIC X(130) VALUE "O prompt está vazio.".
          05 FILLER PIC X(130) VALUE "El prompt está vacío.".
          05 FILLER PIC X(130) VALUE "Le prompt est vide.".
          05 FILLER PIC X(130) VALUE "プロンプトが空です。".
          05 FILLER PIC X(130) VALUE "提示词为空。".
      *>   SAVED-AS
          05 FILLER PIC X(130) VALUE "Saved as v&1, now active.".
          05 FILLER PIC X(130) VALUE "Salvo como v&1, agora ativo.".
          05 FILLER PIC X(130) VALUE "Guardado como v&1, ahora activo.".
          05 FILLER PIC X(130) VALUE "Enregistré comme v&1, désormais actif.".
          05 FILLER PIC X(130) VALUE "v&1 として保存し、有効にしました。".
          05 FILLER PIC X(130) VALUE "已保存为 v&1，现已启用。".
      *>   ACTIVE-MARK
          05 FILLER PIC X(130) VALUE "[active]".
          05 FILLER PIC X(130) VALUE "[ativa]".
          05 FILLER PIC X(130) VALUE "[activa]".
          05 FILLER PIC X(130) VALUE "[active]".
          05 FILLER PIC X(130) VALUE "[有効]".
          05 FILLER PIC X(130) VALUE "[当前]".
      *>   PROMPT-TITLE
          05 FILLER PIC X(130) VALUE "Prompt".
          05 FILLER PIC X(130) VALUE "Prompt".
          05 FILLER PIC X(130) VALUE "Prompt".
          05 FILLER PIC X(130) VALUE "Prompt".
          05 FILLER PIC X(130) VALUE "プロンプト".
          05 FILLER PIC X(130) VALUE "提示词".
      *>   HINT-PROMPT-EDITOR
          05 FILLER PIC X(130) VALUE "What the assistant is for and how it answers - the topic's system prompt".
          05 FILLER PIC X(130) VALUE "Para que serve o assistente e como ele responde - o prompt de sistema do tópico".
          05 FILLER PIC X(130) VALUE "Para qué sirve el asistente y cómo responde: el prompt de sistema del tema".
          05 FILLER PIC X(130) VALUE "À quoi sert l'assistant et comment il répond : le prompt système du sujet".
          05 FILLER PIC X(130) VALUE "アシスタントの役割と回答の仕方 - トピックのシステムプロンプト".
          05 FILLER PIC X(130) VALUE "助手的用途及回答方式——该主题的系统提示词".
      *>   NEW-VERSION
          05 FILLER PIC X(130) VALUE "New version".
          05 FILLER PIC X(130) VALUE "Nova versão".
          05 FILLER PIC X(130) VALUE "Nueva versión".
          05 FILLER PIC X(130) VALUE "Nouvelle version".
          05 FILLER PIC X(130) VALUE "新しいバージョン".
          05 FILLER PIC X(130) VALUE "新版本".
      *>   ACTIVATE
          05 FILLER PIC X(130) VALUE "Activate".
          05 FILLER PIC X(130) VALUE "Ativar".
          05 FILLER PIC X(130) VALUE "Activar".
          05 FILLER PIC X(130) VALUE "Activer".
          05 FILLER PIC X(130) VALUE "有効にする".
          05 FILLER PIC X(130) VALUE "启用".
      *>   SAVE
          05 FILLER PIC X(130) VALUE "Save".
          05 FILLER PIC X(130) VALUE "Salvar".
          05 FILLER PIC X(130) VALUE "Guardar".
          05 FILLER PIC X(130) VALUE "Enregistrer".
          05 FILLER PIC X(130) VALUE "保存".
          05 FILLER PIC X(130) VALUE "保存".
      *>   STATUS
          05 FILLER PIC X(130) VALUE "Status".
          05 FILLER PIC X(130) VALUE "Status".
          05 FILLER PIC X(130) VALUE "Estado".
          05 FILLER PIC X(130) VALUE "État".
          05 FILLER PIC X(130) VALUE "ステータス".
          05 FILLER PIC X(130) VALUE "状态".
      *>   TAB-BROWSE
          05 FILLER PIC X(130) VALUE "Browse".
          05 FILLER PIC X(130) VALUE "Consultar".
          05 FILLER PIC X(130) VALUE "Consultar".
          05 FILLER PIC X(130) VALUE "Parcourir".
          05 FILLER PIC X(130) VALUE "一覧".
          05 FILLER PIC X(130) VALUE "浏览".
      *>   TAB-EDIT
          05 FILLER PIC X(130) VALUE "Create/Update".
          05 FILLER PIC X(130) VALUE "Criar/Atualizar".
          05 FILLER PIC X(130) VALUE "Crear/Actualizar".
          05 FILLER PIC X(130) VALUE "Créer/Modifier".
          05 FILLER PIC X(130) VALUE "作成/更新".
          05 FILLER PIC X(130) VALUE "新建/更新".
      *>   NEW
          05 FILLER PIC X(130) VALUE "New".
          05 FILLER PIC X(130) VALUE "Novo".
          05 FILLER PIC X(130) VALUE "Nuevo".
          05 FILLER PIC X(130) VALUE "Nouveau".
          05 FILLER PIC X(130) VALUE "新規".
          05 FILLER PIC X(130) VALUE "新建".
      *>   CANCEL
          05 FILLER PIC X(130) VALUE "Cancel".
          05 FILLER PIC X(130) VALUE "Cancelar".
          05 FILLER PIC X(130) VALUE "Cancelar".
          05 FILLER PIC X(130) VALUE "Annuler".
          05 FILLER PIC X(130) VALUE "キャンセル".
          05 FILLER PIC X(130) VALUE "取消".
      *>   YES
          05 FILLER PIC X(130) VALUE "Yes".
          05 FILLER PIC X(130) VALUE "Sim".
          05 FILLER PIC X(130) VALUE "Sí".
          05 FILLER PIC X(130) VALUE "Oui".
          05 FILLER PIC X(130) VALUE "はい".
          05 FILLER PIC X(130) VALUE "是".
      *>   NO
          05 FILLER PIC X(130) VALUE "No".
          05 FILLER PIC X(130) VALUE "Não".
          05 FILLER PIC X(130) VALUE "No".
          05 FILLER PIC X(130) VALUE "Non".
          05 FILLER PIC X(130) VALUE "いいえ".
          05 FILLER PIC X(130) VALUE "否".
      *>   CONFIRM-DELETE
          05 FILLER PIC X(130) VALUE "Delete v&1? This cannot be undone.".
          05 FILLER PIC X(130) VALUE "Excluir a v&1? Isso não pode ser desfeito.".
          05 FILLER PIC X(130) VALUE "¿Eliminar la v&1? No se puede deshacer.".
          05 FILLER PIC X(130) VALUE "Supprimer la v&1 ? Cette action est irréversible.".
          05 FILLER PIC X(130) VALUE "v&1 を削除しますか？元に戻せません。".
          05 FILLER PIC X(130) VALUE "删除 v&1？此操作无法撤销。".
      *>   ACTIVE-NO-DELETE
          05 FILLER PIC X(130) VALUE "v&1 is the active prompt and cannot be deleted. Activate another version first.".
          05 FILLER PIC X(130) VALUE "A v&1 é o prompt ativo e não pode ser excluída. Ative outra versão antes.".
          05 FILLER PIC X(130) VALUE "La v&1 es el prompt activo y no se puede eliminar. Activa antes otra versión.".
          05 FILLER PIC X(130) VALUE "La v&1 est le prompt actif : impossible de la supprimer. Activez d'abord une autre version.".
          05 FILLER PIC X(130) VALUE "有効な v&1 は削除できません。先に別のバージョンを有効にしてください。".
          05 FILLER PIC X(130) VALUE "v&1 是当前提示词，无法删除。请先启用其他版本。".
      *>   DELETED
          05 FILLER PIC X(130) VALUE "v&1 deleted.".
          05 FILLER PIC X(130) VALUE "A v&1 foi excluída.".
          05 FILLER PIC X(130) VALUE "Se eliminó la v&1.".
          05 FILLER PIC X(130) VALUE "La v&1 a été supprimée.".
          05 FILLER PIC X(130) VALUE "v&1 を削除しました。".
          05 FILLER PIC X(130) VALUE "已删除 v&1。".
      *>   SAVED-V
          05 FILLER PIC X(130) VALUE "v&1 saved.".
          05 FILLER PIC X(130) VALUE "A v&1 foi salva.".
          05 FILLER PIC X(130) VALUE "Se guardó la v&1.".
          05 FILLER PIC X(130) VALUE "La v&1 a été enregistrée.".
          05 FILLER PIC X(130) VALUE "v&1 を保存しました。".
          05 FILLER PIC X(130) VALUE "已保存 v&1。".
      *>   SAVE-FAILED
          05 FILLER PIC X(130) VALUE "Could not save v&1 (file status &2).".
          05 FILLER PIC X(130) VALUE "Não foi possível salvar a v&1 (status &2).".
          05 FILLER PIC X(130) VALUE "No se pudo guardar la v&1 (estado &2).".
          05 FILLER PIC X(130) VALUE "Impossible d'enregistrer la v&1 (état &2).".
          05 FILLER PIC X(130) VALUE "v&1 を保存できませんでした (状態 &2)。".
          05 FILLER PIC X(130) VALUE "无法保存 v&1（状态 &2）。".
      *>   DELETE-FAILED
          05 FILLER PIC X(130) VALUE "Could not delete v&1 (file status &2).".
          05 FILLER PIC X(130) VALUE "Não foi possível excluir a v&1 (status &2).".
          05 FILLER PIC X(130) VALUE "No se pudo eliminar la v&1 (estado &2).".
          05 FILLER PIC X(130) VALUE "Impossible de supprimer la v&1 (état &2).".
          05 FILLER PIC X(130) VALUE "v&1 を削除できませんでした (状態 &2)。".
          05 FILLER PIC X(130) VALUE "无法删除 v&1（状态 &2）。".
      *>   COL-VERSION
          05 FILLER PIC X(130) VALUE "Version".
          05 FILLER PIC X(130) VALUE "Versão".
          05 FILLER PIC X(130) VALUE "Versión".
          05 FILLER PIC X(130) VALUE "Version".
          05 FILLER PIC X(130) VALUE "バージョン".
          05 FILLER PIC X(130) VALUE "版本".
      *>   COL-CREATED
          05 FILLER PIC X(130) VALUE "Created".
          05 FILLER PIC X(130) VALUE "Criada em".
          05 FILLER PIC X(130) VALUE "Creada".
          05 FILLER PIC X(130) VALUE "Créée le".
          05 FILLER PIC X(130) VALUE "作成日時".
          05 FILLER PIC X(130) VALUE "创建时间".
      *>   COL-ACTIVE
          05 FILLER PIC X(130) VALUE "Active".
          05 FILLER PIC X(130) VALUE "Ativa".
          05 FILLER PIC X(130) VALUE "Activa".
          05 FILLER PIC X(130) VALUE "Active".
          05 FILLER PIC X(130) VALUE "有効".
          05 FILLER PIC X(130) VALUE "当前".
      *>   COL-TEXT
          05 FILLER PIC X(130) VALUE "Text".
          05 FILLER PIC X(130) VALUE "Texto".
          05 FILLER PIC X(130) VALUE "Texto".
          05 FILLER PIC X(130) VALUE "Texte".
          05 FILLER PIC X(130) VALUE "テキスト".
          05 FILLER PIC X(130) VALUE "文本".
      *>   MAIN-PROMPT
          05 FILLER PIC X(130) VALUE "Main prompt".
          05 FILLER PIC X(130) VALUE "Prompt principal".
          05 FILLER PIC X(130) VALUE "Prompt principal".
          05 FILLER PIC X(130) VALUE "Prompt principal".
          05 FILLER PIC X(130) VALUE "メインプロンプト".
          05 FILLER PIC X(130) VALUE "主提示词".
      *>   TOPIC-PROMPT
          05 FILLER PIC X(130) VALUE "Topic prompt".
          05 FILLER PIC X(130) VALUE "Prompt do tópico".
          05 FILLER PIC X(130) VALUE "Prompt del tema".
          05 FILLER PIC X(130) VALUE "Prompt du sujet".
          05 FILLER PIC X(130) VALUE "トピックのプロンプト".
          05 FILLER PIC X(130) VALUE "主题提示词".
      *>   RESTORE-DEFAULT
          05 FILLER PIC X(130) VALUE "Restore default".
          05 FILLER PIC X(130) VALUE "Restaurar padrão".
          05 FILLER PIC X(130) VALUE "Restaurar predeterminado".
          05 FILLER PIC X(130) VALUE "Rétablir par défaut".
          05 FILLER PIC X(130) VALUE "既定に戻す".
          05 FILLER PIC X(130) VALUE "恢复默认".
      *>   MAIN-TITLE
          05 FILLER PIC X(130) VALUE "Main prompt - every instruction, for every topic".
          05 FILLER PIC X(130) VALUE "Prompt principal - todas as instruções, para todos os tópicos".
          05 FILLER PIC X(130) VALUE "Prompt principal: todas las instrucciones, para todos los temas".
          05 FILLER PIC X(130) VALUE "Prompt principal : toutes les instructions, pour tous les sujets".
          05 FILLER PIC X(130) VALUE "メインプロンプト - すべてのトピックへのすべての指示".
          05 FILLER PIC X(130) VALUE "主提示词——适用于所有主题的全部指令".
      *>   HINT-MAIN-EDITOR
          05 FILLER PIC X(130) VALUE "Every instruction the models get, in English, section by section (=== NAME ===)".
          05 FILLER PIC X(130) VALUE "Todas as instruções dadas aos modelos, em inglês, seção por seção (=== NAME ===)".
          05 FILLER PIC X(130) VALUE "Todas las instrucciones que reciben los modelos, en inglés, sección por sección (=== NAME ===)".
          05 FILLER PIC X(130) VALUE "Toutes les instructions données aux modèles, en anglais, section par section (=== NAME ===)".
          05 FILLER PIC X(130) VALUE "モデルへのすべての指示（英語、セクションごと: === NAME ===）".
          05 FILLER PIC X(130) VALUE "模型收到的全部指令（英文，按节：=== NAME ===）".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
          05 PC-TEXT-ROW     OCCURS 37.
             10 PC-TEXT      PIC X(130) OCCURS 6.
      *>   The texts in the current language, by name.
       01 PC-TEXTS-NOW       GLOBAL.
          05 T-OPEN-TOPIC-FIRST PIC X(130).
          05 T-PROMPT-OF PIC X(130).
          05 T-PICK-VERSION-FIRST PIC X(130).
          05 T-EDITING-V PIC X(130).
          05 T-ALREADY-ACTIVE PIC X(130).
          05 T-CONFIRM-ACTIVATE PIC X(130).
          05 T-NOW-ACTIVE PIC X(130).
          05 T-PROMPT-EMPTY PIC X(130).
          05 T-SAVED-AS PIC X(130).
          05 T-ACTIVE-MARK PIC X(130).
          05 T-PROMPT-TITLE PIC X(130).
          05 T-HINT-PROMPT-EDITOR PIC X(130).
          05 T-NEW-VERSION PIC X(130).
          05 T-ACTIVATE PIC X(130).
          05 T-SAVE PIC X(130).
          05 T-STATUS PIC X(130).
          05 T-TAB-BROWSE PIC X(130).
          05 T-TAB-EDIT PIC X(130).
          05 T-NEW PIC X(130).
          05 T-CANCEL PIC X(130).
          05 T-YES PIC X(130).
          05 T-NO PIC X(130).
          05 T-CONFIRM-DELETE PIC X(130).
          05 T-ACTIVE-NO-DELETE PIC X(130).
          05 T-DELETED PIC X(130).
          05 T-SAVED-V PIC X(130).
          05 T-SAVE-FAILED PIC X(130).
          05 T-DELETE-FAILED PIC X(130).
          05 T-COL-VERSION PIC X(130).
          05 T-COL-CREATED PIC X(130).
          05 T-COL-ACTIVE PIC X(130).
          05 T-COL-TEXT PIC X(130).
          05 T-MAIN-PROMPT PIC X(130).
          05 T-TOPIC-PROMPT PIC X(130).
          05 T-RESTORE-DEFAULT PIC X(130).
          05 T-MAIN-TITLE PIC X(130).
          05 T-HINT-MAIN-EDITOR PIC X(130).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
          05 PC-TEXT-NOW     PIC X(130) OCCURS 37.
      *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.
       01 WS-FMT             GLOBAL PIC X(130).
       01 WS-ARG1            GLOBAL PIC X(300).
       01 WS-ARG2            GLOBAL PIC X(300).
       01 WS-ARG3            GLOBAL PIC X(300).
       01 WS-ARG4            GLOBAL PIC X(300).
       01 WS-FMT-OUT         GLOBAL PIC X(1200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Prompt'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Prompt.
          05 WS-Txt-Prompt-TEXT       PIC X(32000) VALUE SPACES.
          05 WS-Txt-Prompt-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Prompt-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Prompt-VALUE      PIC X(32000) VALUE SPACES.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Lang.
          05 WS-Tmr-Lang-TEXT       PIC X(256) VALUE 'Tmr-Lang'.
          05 WS-Tmr-Lang-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Lang-ENABLED    PIC 9      VALUE 1.

       01 WS-Tab-Crud.
          05 WS-Tab-Crud-TEXT       PIC X(256) VALUE 'Tab-Crud'.
          05 WS-Tab-Crud-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tab-Crud-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-New.
          05 WS-Btn-New-TEXT       PIC X(256) VALUE 'New'.
          05 WS-Btn-New-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-New-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Main.
          05 WS-Btn-Main-TEXT       PIC X(256) VALUE 'Main prompt'.
          05 WS-Btn-Main-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Main-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Topic.
          05 WS-Btn-Topic-TEXT       PIC X(256) VALUE 'Topic prompt'.
          05 WS-Btn-Topic-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Topic-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Default.
          05 WS-Btn-Default-TEXT       PIC X(256) VALUE 'Restore default'.
          05 WS-Btn-Default-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Default-ENABLED    PIC 9      VALUE 1.

       01 WS-Dg-List.
          05 WS-Dg-List-TEXT       PIC X(256) VALUE 'Dg-List'.
          05 WS-Dg-List-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dg-List-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Edit-Ver.
          05 WS-Lbl-Edit-Ver-TEXT       PIC X(256) VALUE 'New version'.
          05 WS-Lbl-Edit-Ver-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Edit-Ver-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save-Bottom.
          05 WS-Btn-Save-Bottom-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-Bottom-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-Bottom-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel-Bottom.
          05 WS-Btn-Cancel-Bottom-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-Bottom-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-Bottom-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "PROMPTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PROMPTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "PROMPTS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onActivate"
                               CALL "PROMPTS-FORM--ONACTIVATE"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Tmr-Lang"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-LANG--ONTICK"
                       END-EVALUATE
                   WHEN "Btn-New"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEW--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Main"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MAIN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Topic"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-TOPIC--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Default"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-DEFAULT--ONCLICK"
                       END-EVALUATE
                   WHEN "Dg-List"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCellClick"
                               CALL "DG-LIST--ONCELLCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save-Bottom"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE-BOTTOM--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel-Bottom"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL-BOTTOM--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Tmr-Lang 'SetInterval' USING BY VALUE 1000
           CONTINUE.

      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
       Dg-List-EXPORT-CSV.
      *>    Export Dg-List data to CSV file.  Delimiter: ",". Mode: Filtered.
      *>    Column order and filtered/all rows follow the DataGrid settings.
      *>    Set WS-Dg-List-CSV-PATH to the desired output file path before calling.
           INVOKE Dg-List 'ExportCSV'
               USING BY REFERENCE WS-Dg-List-CSV-PATH
               RETURNING WS-Dg-List-CSV-STATUS
           IF WS-Dg-List-CSV-STATUS NOT = 0
               DISPLAY "CSV export error: " WS-Dg-List-CSV-STATUS
           END-IF.

      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROMPTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM PROMPTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROMPTS-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back on the pane: the topic or the language may have changed.
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM PROMPTS-FORM--ONACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROMPTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PROMPTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-LANG--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   A flag in the chat's menu changes the language while this form is
      *>   on the pane; nothing tells a pane occupant, so it looks (R46).
           CALL "PC-LANG-NOW"
           IF WS-LANG-NOW NOT = WS-LANG
               CALL "PROMPTS-FORM--ONACTIVATE"
           END-IF

           GOBACK.

       END PROGRAM TMR-LANG--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-NEW"

           GOBACK.

       END PROGRAM BTN-NEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MAIN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The main prompt: every instruction the models get, every topic.
           MOVE "M" TO WS-SCOPE
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM BTN-MAIN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-TOPIC--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back to the open topic's own prompt.
           MOVE "T" TO WS-SCOPE
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM BTN-TOPIC--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-DEFAULT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The main prompt as shipped, saved as a new version and made the
      *>   active one - the versions before it stay in the list.
           CALL "PC-MAIN-DEFAULT"
           MOVE 0 TO WS-EDIT-VER
           MOVE WS-TEXT TO Txt-Prompt::Text
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-DEFAULT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DG-LIST--ONCELLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Column 1 holds the version number (the key); 5 promotes, 6 edits
      *>   and 7 deletes. Promoting and deleting ask first (R48).
           MOVE Dg-List::ClickedRow TO WS-ROW-NO
           MOVE Dg-List::ClickedColumn TO WS-COL-NO
           IF WS-ROW-NO < 1 OR WS-COL-NO < 5
               EXIT PROGRAM
           END-IF
           MOVE Dg-List::GetCellValue(WS-ROW-NO, 1) TO WS-KEY
           MOVE 0 TO WS-PICKED
           IF FUNCTION TRIM(WS-KEY) NOT = SPACES
               COMPUTE WS-PICKED = FUNCTION NUMVAL(WS-KEY)
           END-IF
           IF WS-PICKED = 0
               MOVE FUNCTION TRIM(T-PICK-VERSION-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           MOVE WS-PICKED TO WS-VNUM
           MOVE WS-VNUM TO WS-ARG1
           EVALUATE WS-COL-NO
               WHEN 5
                   IF WS-PICKED = WS-ACTIVE-VER
                       MOVE FUNCTION TRIM(T-ALREADY-ACTIVE) TO Lbl-Status::Caption
                       EXIT PROGRAM
                   END-IF
                   MOVE T-CONFIRM-ACTIVATE TO WS-FMT
                   CALL "PC-FMT"
                   MOVE FUNCTION TRIM(WS-FMT-OUT) TO WS-QUESTION
                   INVOKE ME::"SetProperty"("ConfirmText", WS-QUESTION)
                   INVOKE ME::"SetProperty"("ConfirmYes", T-ACTIVATE)
                   INVOKE ME::"SetProperty"("ConfirmNo", T-NO)
                   INVOKE ME::"SetProperty"("ConfirmAnswer", "N")
                   INVOKE ME::"OpenFormSync"("CONFIRM-FORM")
                   INVOKE ME::"GetProperty"("ConfirmAnswer") RETURNING WS-ANSWER
                   IF WS-ANSWER = "Y"
                       CALL "PC-ACTIVATE"
                       MOVE T-NOW-ACTIVE TO WS-FMT
                       MOVE WS-VNUM TO WS-ARG1
                       CALL "PC-FMT"
                       MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
                       CALL "PC-LOAD-VERSIONS"
                   END-IF
               WHEN 6
                   CALL "PC-EDIT"
               WHEN 7
                   IF WS-PICKED = WS-ACTIVE-VER
                       MOVE T-ACTIVE-NO-DELETE TO WS-FMT
                       CALL "PC-FMT"
                       MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
                       EXIT PROGRAM
                   END-IF
                   MOVE T-CONFIRM-DELETE TO WS-FMT
                   CALL "PC-FMT"
                   MOVE FUNCTION TRIM(WS-FMT-OUT) TO WS-QUESTION
                   INVOKE ME::"SetProperty"("ConfirmText", WS-QUESTION)
                   INVOKE ME::"SetProperty"("ConfirmYes", T-YES)
                   INVOKE ME::"SetProperty"("ConfirmNo", T-NO)
                   INVOKE ME::"SetProperty"("ConfirmAnswer", "N")
                   INVOKE ME::"OpenFormSync"("CONFIRM-FORM")
                   INVOKE ME::"GetProperty"("ConfirmAnswer") RETURNING WS-ANSWER
                   IF WS-ANSWER = "Y"
                       CALL "PC-DELETE"
                   END-IF
           END-EVALUATE

           GOBACK.

       END PROGRAM DG-LIST--ONCELLCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back to the list as it was: nothing saved, nothing re-read.
           MOVE 0 TO WS-EDIT-VER
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE-BOTTOM--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE-BOTTOM--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL-BOTTOM--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back to the list as it was: nothing saved, nothing re-read.
           MOVE 0 TO WS-EDIT-VER
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM BTN-CANCEL-BOTTOM--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PATHS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Where PowerChat keeps its own files: POWERCHAT_DATA, else "data".
           DISPLAY "POWERCHAT_DATA" UPON ENVIRONMENT-NAME
           ACCEPT WS-DATA-DIR FROM ENVIRONMENT-VALUE
           IF WS-DATA-DIR = SPACES
               MOVE "data" TO WS-DATA-DIR
           END-IF
           MOVE SPACES TO WS-SETTINGS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/settings.idx"
               DELIMITED BY SIZE INTO WS-SETTINGS-PATH
           MOVE SPACES TO WS-TOPICS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/topics.idx"
               DELIMITED BY SIZE INTO WS-TOPICS-PATH
           MOVE SPACES TO WS-PROMPTS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/prompt-versions.idx"
               DELIMITED BY SIZE INTO WS-PROMPTS-PATH
           MOVE SPACES TO WS-OLDPRM-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/prompts.idx"
               DELIMITED BY SIZE INTO WS-OLDPRM-PATH
           DISPLAY "POWERCHAT_SAMPLES" UPON ENVIRONMENT-NAME
           ACCEPT WS-SAMPLES-DIR FROM ENVIRONMENT-VALUE
           IF WS-SAMPLES-DIR = SPACES
               MOVE "samples" TO WS-SAMPLES-DIR
           END-IF
           MOVE SPACES TO WS-MPSEED-PATH
           STRING FUNCTION TRIM(WS-SAMPLES-DIR) "/main-prompt.md"
               DELIMITED BY SIZE INTO WS-MPSEED-PATH

           GOBACK.

       END PROGRAM PC-PATHS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-GET IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-SET-NAME in, WS-SET-VALUE out (spaces when the setting is unset).
           MOVE SPACES TO WS-SET-VALUE
           OPEN I-O SETTINGS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT SETTINGS-FILE
               CLOSE SETTINGS-FILE
               OPEN I-O SETTINGS-FILE
           END-IF
           MOVE WS-SET-NAME TO SET-NAME
           READ SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE SET-VALUE TO WS-SET-VALUE
           END-READ
           CLOSE SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-GET.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-PUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-SET-NAME, WS-SET-VALUE in. Written and committed at once (R10f).
           OPEN I-O SETTINGS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT SETTINGS-FILE
               CLOSE SETTINGS-FILE
               OPEN I-O SETTINGS-FILE
           END-IF
           MOVE WS-SET-NAME TO SET-NAME
           MOVE WS-SET-VALUE TO SET-VALUE
           WRITE SETTINGS-REC
               INVALID KEY REWRITE SETTINGS-REC
           END-WRITE
           COMMIT
           CLOSE SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-PUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LOAD-VERSIONS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The topic's versions, newest first, the active one marked (R48).
      *>   A topic that has none yet gets version 1 from its current prompt.
      *>   One grid row per version: number (the key), date, the active
      *>   marker, a preview of the text, then the promote, edit and delete
      *>   icon buttons.
           MOVE Dg-List::ClearRows() TO WS-OK
           MOVE 0 TO WS-VER-COUNT WS-LAST-VER WS-ACTIVE-VER
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE WS-CUR-TOPIC TO PRM-TOPIC
           MOVE 0 TO PRM-VERSION
           MOVE "N" TO WS-EOF
           START PROMPTS-FILE KEY IS >= PRM-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ PROMPTS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF PRM-TOPIC NOT = WS-CUR-TOPIC
                           MOVE "Y" TO WS-EOF
                       ELSE
                           MOVE PRM-VERSION TO WS-LAST-VER
                           IF PRM-ACTIVE = "Y"
                               MOVE PRM-VERSION TO WS-ACTIVE-VER
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           IF WS-LAST-VER = 0
               MOVE WS-CUR-TOPIC TO PRM-TOPIC
               MOVE 1 TO PRM-VERSION WS-LAST-VER WS-ACTIVE-VER
               MOVE FUNCTION CURRENT-DATE TO WS-NOW
               MOVE WS-NOW(1:14) TO PRM-CREATED
               MOVE "Y" TO PRM-ACTIVE
               IF WS-CUR-TOPIC = "*MAIN"
                   CALL "PC-MAIN-DEFAULT"
                   MOVE WS-CUR-TOPIC TO PRM-TOPIC
                   MOVE 1 TO PRM-VERSION
                   MOVE "Y" TO PRM-ACTIVE
                   MOVE WS-TEXT TO PRM-TEXT
               ELSE
                   MOVE TOP-PROMPT TO PRM-TEXT
               END-IF
               WRITE PROMPT-REC
                   INVALID KEY CONTINUE
               END-WRITE
               COMMIT
           END-IF
      *>   Newest first: walk down from the last version.
           PERFORM VARYING WS-I FROM WS-LAST-VER BY -1 UNTIL WS-I < 1
               MOVE WS-CUR-TOPIC TO PRM-TOPIC
               MOVE WS-I TO PRM-VERSION
               READ PROMPTS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY
                       IF WS-VER-COUNT < 200
                           ADD 1 TO WS-VER-COUNT
                           MOVE PRM-VERSION TO WS-VER(WS-VER-COUNT)
                           MOVE PRM-VERSION TO WS-VNUM
      *>   A tab or a line break inside the text would split the row.
                           MOVE PRM-TEXT(1:57) TO WS-PREVIEW
                           INSPECT WS-PREVIEW REPLACING ALL X"09" BY SPACE
                                                        ALL X"0A" BY SPACE
                                                        ALL X"0D" BY SPACE
                           IF PRM-TEXT(58:) NOT = SPACES
                               MOVE SPACES TO WS-LINE
                               STRING FUNCTION TRIM(WS-PREVIEW) "..."
                                   DELIMITED BY SIZE INTO WS-LINE
                               MOVE WS-LINE TO WS-PREVIEW
                           END-IF
                           MOVE SPACES TO WS-LINE
                           IF PRM-ACTIVE = "Y"
                               MOVE T-ACTIVE-MARK TO WS-LINE
                           END-IF
                           MOVE SPACES TO WS-ROW
                           STRING FUNCTION TRIM(WS-VNUM) X"09"
                                  PRM-CREATED(1:4) "-" PRM-CREATED(5:2) "-"
                                  PRM-CREATED(7:2) " " PRM-CREATED(9:2) ":"
                                  PRM-CREATED(11:2) X"09"
                                  FUNCTION TRIM(WS-LINE) X"09"
                                  FUNCTION TRIM(WS-PREVIEW) X"09"
                                  "icon:check-circle" X"09"
                                  "icon:pencil" X"09"
                                  "icon:trash"
                               DELIMITED BY SIZE INTO WS-ROW
                           MOVE Dg-List::AddRow(WS-ROW) TO WS-OK
                       END-IF
               END-READ
           END-PERFORM
           CLOSE PROMPTS-FILE

           GOBACK.

       END PROGRAM PC-LOAD-VERSIONS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-PICKED in: that version becomes the active one, and its text the
      *>   topic's prompt, which the chat's orchestrator is given (R63).
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LAST-VER
               MOVE WS-CUR-TOPIC TO PRM-TOPIC
               MOVE WS-I TO PRM-VERSION
               READ PROMPTS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY
                       IF WS-I = WS-PICKED
                           MOVE "Y" TO PRM-ACTIVE
                           MOVE PRM-TEXT TO WS-TEXT
                       ELSE
                           MOVE "N" TO PRM-ACTIVE
                       END-IF
                       REWRITE PROMPT-REC
                       END-REWRITE
               END-READ
           END-PERFORM
           COMMIT
           CLOSE PROMPTS-FILE
           MOVE WS-PICKED TO WS-ACTIVE-VER
           IF WS-CUR-TOPIC = "*MAIN"
               EXIT PROGRAM
           END-IF
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE WS-CUR-TOPIC TO TOP-ID
           READ TOPICS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   MOVE WS-TEXT TO TOP-PROMPT
                   REWRITE TOPIC-REC
                   END-REWRITE
                   COMMIT
           END-READ
           CLOSE TOPICS-FILE
           MOVE WS-PICKED TO WS-ACTIVE-VER

           GOBACK.

       END PROGRAM PC-ACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The language the user picked (LANG), the texts in it, and every
      *>   designed caption and hint that shows one. Called on load and again
      *>   whenever the language changes (R46).
           MOVE "LANG" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           MOVE WS-SET-VALUE(1:2) TO WS-LANG
           EVALUATE WS-LANG
               WHEN "en" MOVE 1 TO WS-LANG-IX
               WHEN "pt" MOVE 2 TO WS-LANG-IX
               WHEN "es" MOVE 3 TO WS-LANG-IX
               WHEN "fr" MOVE 4 TO WS-LANG-IX
               WHEN "jp" MOVE 5 TO WS-LANG-IX
               WHEN "cn" MOVE 6 TO WS-LANG-IX
               WHEN OTHER MOVE "en" TO WS-LANG
                          MOVE 1 TO WS-LANG-IX
           END-EVALUATE
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 37
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-PROMPT-TITLE) TO Lbl-Title::Caption
           MOVE FUNCTION TRIM(T-HINT-PROMPT-EDITOR) TO Txt-Prompt::HintText
           MOVE FUNCTION TRIM(T-NEW) TO Btn-New::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save-Bottom::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel-Bottom::Caption
           MOVE FUNCTION TRIM(T-MAIN-PROMPT) TO Btn-Main::Caption
           MOVE FUNCTION TRIM(T-TOPIC-PROMPT) TO Btn-Topic::Caption
           MOVE FUNCTION TRIM(T-RESTORE-DEFAULT) TO Btn-Default::Caption
           MOVE FUNCTION TRIM(T-STATUS) TO Lbl-Status::Caption
           MOVE SPACES TO WS-TABS
           STRING FUNCTION TRIM(T-TAB-BROWSE) X"0A" FUNCTION TRIM(T-TAB-EDIT)
               DELIMITED BY SIZE INTO WS-TABS
           MOVE WS-TABS TO Tab-Crud::Tabs
           INVOKE Dg-List::SetColumnTitle("Ver", T-COL-VERSION)
           INVOKE Dg-List::SetColumnTitle("Created", T-COL-CREATED)
           INVOKE Dg-List::SetColumnTitle("Active", T-COL-ACTIVE)
           INVOKE Dg-List::SetColumnTitle("Text", T-COL-TEXT)

           GOBACK.

       END PROGRAM PC-TEXTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FMT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PT              PIC 9(4).
       01 WS-FROM            PIC 9(4).
       01 WS-CNT             PIC 9(4).
       01 WS-PART            PIC X(130).
       01 WS-DELIM           PIC XX.
       PROCEDURE DIVISION.
      *>   A message whose words go round numbers and names differently in each
      *>   language: WS-FMT holds "&1" .. "&4" where WS-ARG1..4 go.
           MOVE SPACES TO WS-FMT-OUT
           MOVE 1 TO WS-PT
           MOVE 1 TO WS-FROM
           PERFORM UNTIL WS-FROM > FUNCTION LENGTH(WS-FMT)
               MOVE SPACES TO WS-PART WS-DELIM
               MOVE 0 TO WS-CNT
               UNSTRING WS-FMT DELIMITED BY "&1" OR "&2" OR "&3" OR "&4"
                   INTO WS-PART DELIMITER IN WS-DELIM COUNT IN WS-CNT
                   WITH POINTER WS-FROM
               END-UNSTRING
               IF WS-CNT > 0
                   STRING WS-PART(1:WS-CNT) DELIMITED BY SIZE
                       INTO WS-FMT-OUT WITH POINTER WS-PT
               END-IF
               EVALUATE WS-DELIM
                   WHEN "&1" STRING FUNCTION TRIM(WS-ARG1) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&2" STRING FUNCTION TRIM(WS-ARG2) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&3" STRING FUNCTION TRIM(WS-ARG3) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&4" STRING FUNCTION TRIM(WS-ARG4) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN OTHER COMPUTE WS-FROM = FUNCTION LENGTH(WS-FMT) + 1
               END-EVALUATE
           END-PERFORM

           GOBACK.

       END PROGRAM PC-FMT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-OPEN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   What the form shows: run on load, and again when it comes back.
           CALL "PC-PATHS"
           CALL "PC-MIGRATE"
           CALL "PC-TEXTS"
           MOVE "CUR-TOPIC" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           MOVE WS-SET-VALUE(1:16) TO WS-TOPIC-ID
      *>   The main prompt: every topic's, whether one is open or not.
           IF WS-SCOPE = "M"
               MOVE "*MAIN" TO WS-CUR-TOPIC
               MOVE FUNCTION TRIM(T-MAIN-TITLE) TO Lbl-Title::Caption
               MOVE FUNCTION TRIM(T-HINT-MAIN-EDITOR) TO Txt-Prompt::HintText
               SET Btn-Default::Visible TO TRUE
               CALL "PC-LOAD-VERSIONS"
               MOVE 0 TO Tab-Crud::SelectedTab
               EXIT PROGRAM
           END-IF
           SET Btn-Default::Visible TO FALSE
           MOVE WS-TOPIC-ID TO WS-CUR-TOPIC
           IF WS-CUR-TOPIC = SPACES
               MOVE FUNCTION TRIM(T-OPEN-TOPIC-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE WS-CUR-TOPIC TO TOP-ID
           READ TOPICS-FILE
               INVALID KEY MOVE "?" TO TOP-NAME
           END-READ
           CLOSE TOPICS-FILE
           MOVE T-PROMPT-OF TO WS-FMT
           MOVE TOP-NAME TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Title::Caption
           CALL "PC-LOAD-VERSIONS"
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-OPEN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   An empty editor on the Create/Update tab: Save adds a new version.
           IF WS-CUR-TOPIC = SPACES
               MOVE FUNCTION TRIM(T-OPEN-TOPIC-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           MOVE 0 TO WS-EDIT-VER
           MOVE SPACES TO WS-TEXT
           MOVE WS-TEXT TO Txt-Prompt::Text
           MOVE FUNCTION TRIM(T-NEW-VERSION) TO Lbl-Edit-Ver::Caption
           MOVE 1 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-NEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EDIT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-PICKED in: that version's text on the Create/Update tab.
           MOVE WS-PICKED TO WS-VNUM
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE WS-CUR-TOPIC TO PRM-TOPIC
           MOVE WS-PICKED TO PRM-VERSION
           MOVE "Y" TO WS-SAVE-OK
           READ PROMPTS-FILE
               INVALID KEY MOVE "N" TO WS-SAVE-OK
           END-READ
           CLOSE PROMPTS-FILE
           IF WS-SAVE-OK NOT = "Y"
               MOVE FUNCTION TRIM(T-PICK-VERSION-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           MOVE WS-PICKED TO WS-EDIT-VER
           MOVE PRM-TEXT TO WS-TEXT
           MOVE WS-TEXT TO Txt-Prompt::Text
           MOVE T-EDITING-V TO WS-FMT
           MOVE WS-VNUM TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Edit-Ver::Caption
           MOVE 1 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-EDIT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Both Save buttons. A new version (WS-EDIT-VER = 0) is written as
      *>   the last one and becomes the active prompt; an edited one keeps its
      *>   date and its active flag, and when it is the active one the topic's
      *>   prompt follows. WRITE first; an existing key is REWRITTEN.
           MOVE Txt-Prompt::Text TO WS-TEXT
           IF WS-TEXT = SPACES
               MOVE FUNCTION TRIM(T-PROMPT-EMPTY) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           IF WS-CUR-TOPIC = SPACES
               MOVE FUNCTION TRIM(T-OPEN-TOPIC-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           MOVE WS-CUR-TOPIC TO PRM-TOPIC
           IF WS-EDIT-VER = 0
               COMPUTE WS-SAVE-VER = WS-LAST-VER + 1
               MOVE WS-SAVE-VER TO PRM-VERSION
               MOVE WS-NOW(1:14) TO PRM-CREATED
               MOVE "N" TO PRM-ACTIVE
           ELSE
               MOVE WS-EDIT-VER TO WS-SAVE-VER
               MOVE WS-SAVE-VER TO PRM-VERSION
               READ PROMPTS-FILE
                   INVALID KEY
                       MOVE WS-NOW(1:14) TO PRM-CREATED
                       MOVE "N" TO PRM-ACTIVE
               END-READ
               MOVE WS-CUR-TOPIC TO PRM-TOPIC
               MOVE WS-SAVE-VER TO PRM-VERSION
           END-IF
           MOVE WS-TEXT TO PRM-TEXT
           MOVE "Y" TO WS-SAVE-OK
           WRITE PROMPT-REC
               INVALID KEY
                   REWRITE PROMPT-REC
                       INVALID KEY MOVE "N" TO WS-SAVE-OK
                                   MOVE WS-FS TO WS-FS-ERR
                   END-REWRITE
           END-WRITE
           IF WS-SAVE-OK = "Y"
               COMMIT
           ELSE
               ROLLBACK
           END-IF
           CLOSE PROMPTS-FILE
           MOVE WS-SAVE-VER TO WS-VNUM
           IF WS-SAVE-OK NOT = "Y"
               MOVE T-SAVE-FAILED TO WS-FMT
               MOVE WS-VNUM TO WS-ARG1
               MOVE WS-FS-ERR TO WS-ARG2
               CALL "PC-FMT"
               MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           IF WS-EDIT-VER = 0
               MOVE WS-SAVE-VER TO WS-LAST-VER
               MOVE WS-SAVE-VER TO WS-PICKED
               CALL "PC-ACTIVATE"
               MOVE T-SAVED-AS TO WS-FMT
           ELSE
               IF WS-SAVE-VER = WS-ACTIVE-VER
                   MOVE WS-SAVE-VER TO WS-PICKED
                   CALL "PC-ACTIVATE"
               END-IF
               MOVE T-SAVED-V TO WS-FMT
           END-IF
           MOVE WS-VNUM TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
           MOVE 0 TO WS-EDIT-VER
           CALL "PC-LOAD-VERSIONS"
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DELETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-PICKED in, already confirmed: that version leaves the history.
      *>   The active version is refused before the question is asked.
           MOVE WS-PICKED TO WS-VNUM
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE WS-CUR-TOPIC TO PRM-TOPIC
           MOVE WS-PICKED TO PRM-VERSION
           MOVE "Y" TO WS-SAVE-OK
           DELETE PROMPTS-FILE RECORD
               INVALID KEY MOVE "N" TO WS-SAVE-OK
                           MOVE WS-FS TO WS-FS-ERR
           END-DELETE
           IF WS-SAVE-OK = "Y"
               COMMIT
           ELSE
               ROLLBACK
           END-IF
           CLOSE PROMPTS-FILE
           IF WS-SAVE-OK = "Y"
               MOVE T-DELETED TO WS-FMT
           ELSE
               MOVE T-DELETE-FAILED TO WS-FMT
               MOVE WS-FS-ERR TO WS-ARG2
           END-IF
           MOVE WS-VNUM TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
           CALL "PC-LOAD-VERSIONS"

           GOBACK.

       END PROGRAM PC-DELETE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MIGRATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The versions moved to prompt-versions.idx (1.70.332), where a text
      *>   holds 32,000 characters for the main prompt: while the new file is
      *>   empty, every version of the old prompts.idx is copied over. The old
      *>   file is only read, and left as it was.
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE SPACES TO PRM-TOPIC
           MOVE 0 TO PRM-VERSION
           MOVE "N" TO WS-EMPTY
           START PROMPTS-FILE KEY IS >= PRM-KEY
               INVALID KEY MOVE "Y" TO WS-EMPTY
           END-START
           IF WS-EMPTY = "Y"
               OPEN INPUT OLDPRM-FILE
               IF WS-FS = "00"
                   MOVE "N" TO WS-EOF
                   PERFORM UNTIL WS-EOF = "Y"
                       READ OLDPRM-FILE NEXT RECORD
                           AT END MOVE "Y" TO WS-EOF
                           NOT AT END
                               MOVE OLD-TOPIC TO PRM-TOPIC
                               MOVE OLD-VERSION TO PRM-VERSION
                               MOVE OLD-CREATED TO PRM-CREATED
                               MOVE OLD-ACTIVE TO PRM-ACTIVE
                               MOVE OLD-TEXT TO PRM-TEXT
                               WRITE PROMPT-REC
                                   INVALID KEY CONTINUE
                               END-WRITE
                       END-READ
                   END-PERFORM
                   CLOSE OLDPRM-FILE
                   COMMIT
               END-IF
           END-IF
           CLOSE PROMPTS-FILE
           MOVE "N" TO WS-EOF

           GOBACK.

       END PROGRAM PC-MIGRATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MAIN-DEFAULT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-TEXT out: the main prompt as shipped - samples/main-prompt.md,
      *>   each line's indentation kept.
           MOVE SPACES TO WS-TEXT
           MOVE 1 TO WS-SX-B
           OPEN INPUT MPSEED-FILE
           IF WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF
           MOVE "N" TO WS-EOF
           PERFORM UNTIL WS-EOF = "Y"
               READ MPSEED-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       MOVE MPSEED-REC TO WS-LN-BUF
                       MOVE FUNCTION REVERSE(WS-LN-BUF) TO WS-LN-REV
                       MOVE 0 TO WS-LN-LEN
                       INSPECT WS-LN-REV TALLYING WS-LN-LEN FOR LEADING SPACES
                       COMPUTE WS-LN-LEN = 1000 - WS-LN-LEN
                       IF WS-LN-LEN > 0
                           STRING WS-LN-BUF(1:WS-LN-LEN) X"0A"
                               DELIMITED BY SIZE INTO WS-TEXT WITH POINTER WS-SX-B
                           END-STRING
                       ELSE
                           STRING X"0A" DELIMITED BY SIZE INTO WS-TEXT
                               WITH POINTER WS-SX-B
                           END-STRING
                       END-IF
               END-READ
           END-PERFORM
           CLOSE MPSEED-FILE
           MOVE "N" TO WS-EOF

           GOBACK.

       END PROGRAM PC-MAIN-DEFAULT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LANG-NOW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The language saved now (LANG), into WS-LANG-NOW; "en" when unset.
           MOVE "en" TO WS-LANG-NOW
           OPEN INPUT SETTINGS-FILE
           IF WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF
           MOVE "LANG" TO SET-NAME
           READ SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   EVALUATE SET-VALUE(1:2)
                       WHEN "pt" WHEN "es" WHEN "fr" WHEN "jp" WHEN "cn"
                           MOVE SET-VALUE(1:2) TO WS-LANG-NOW
                   END-EVALUATE
           END-READ
           CLOSE SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-LANG-NOW.

       END PROGRAM PROMPTS-FORM.

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
       FD SETTINGS-FILE IS GLOBAL.
       01 SETTINGS-REC.
           05 SET-NAME                           PIC    X(20).
           05 SET-VALUE                          PIC    X(200).
       FD TOPICS-FILE IS GLOBAL.
       01 TOPIC-REC.
           05 TOP-ID                             PIC    X(16).
           05 TOP-NAME                           PIC    X(40).
           05 TOP-PROMPT                         PIC    X(1000).
           05 TOP-CREATED                        PIC    X(14).
           *>   "Y" for a topic the sample installer made, so it can be removed.

           05 TOP-SAMPLE                         PIC    X.
       FD PROMPTS-FILE IS GLOBAL.
       *>   Every version of each topic's system prompt (R48), and of the main
       *>   prompt under "*MAIN": data/prompt-versions.idx. A topic's active
       *>   text is also its TOP-PROMPT, which is what the chat reads.

       01 PROMPT-REC.
           05 PRM-KEY.
               10 PRM-TOPIC                      PIC    X(16).
               10 PRM-VERSION                    PIC    9(4).
           05 PRM-CREATED                        PIC    X(14).
           05 PRM-ACTIVE                         PIC    X.
           05 PRM-TEXT                           PIC    X(32000).
       *>   The versions' file before 1.70.332, read once to move them over.

       FD OLDPRM-FILE IS GLOBAL.
       01 OLD-PROMPT-REC.
           05 OLD-KEY.
               10 OLD-TOPIC                      PIC    X(16).
               10 OLD-VERSION                    PIC    9(4).
           05 OLD-CREATED                        PIC    X(14).
           05 OLD-ACTIVE                         PIC    X.
           05 OLD-TEXT                           PIC    X(1000).
       FD MPSEED-FILE IS GLOBAL.
       01 MPSEED-REC                             PIC    X(1000).
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
       01 WS-FS                                  GLOBAL    PIC XX    VALUE "00".
       01 WS-DATA-DIR                            GLOBAL    PIC X(200).
       01 WS-SET-NAME                            GLOBAL    PIC X(20).
       01 WS-SET-VALUE                           GLOBAL    PIC X(200).
       01 WS-NOW                                 GLOBAL    PIC X(21).
       01 WS-EOF                                 GLOBAL    PIC X.
       01 WS-OK                                  GLOBAL    PIC X(4).
       01 WS-SETTINGS-PATH                       GLOBAL    PIC X(240).
       01 WS-TOPICS-PATH                         GLOBAL    PIC X(240).
       01 WS-PROMPTS-PATH                        GLOBAL    PIC X(240).
       01 WS-CUR-TOPIC                           GLOBAL    PIC X(16).
       01 WS-VER-COUNT                           GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-VERSIONS                            GLOBAL.
           05 WS-VER                             PIC    9(4)    OCCURS 200.
       01 WS-LAST-VER                            GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-ACTIVE-VER                          GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-PICKED                              GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-CONFIRM-VER                         GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-INDEX                               GLOBAL    PIC S9(4).
       01 WS-I                                   GLOBAL    PIC 9(4).
       01 WS-TEXT                                GLOBAL    PIC X(32000).
       *>   Which prompt the screen edits: "T" the open topic's, "M" the main
       *>   prompt - every instruction, for every topic - kept in the same
       *>   version file under the topic id "*MAIN" (operator, 2026-09-28).

       01 WS-SCOPE                               GLOBAL    PIC X    VALUE "T".
       01 WS-TOPIC-ID                            GLOBAL    PIC X(16).
       01 WS-OLDPRM-PATH                         GLOBAL    PIC X(240).
       01 WS-MPSEED-PATH                         GLOBAL    PIC X(240).
       01 WS-SAMPLES-DIR                         GLOBAL    PIC X(240).
       01 WS-EMPTY                               GLOBAL    PIC X.
       01 WS-SX-B                                GLOBAL    PIC 9(6).
       01 WS-LN-BUF                              GLOBAL    PIC X(1000).
       01 WS-LN-REV                              GLOBAL    PIC X(1000).
       01 WS-LN-LEN                              GLOBAL    PIC 9(4).
       01 WS-ROW                                 GLOBAL    PIC X(300).
       01 WS-LINE                                GLOBAL    PIC X(200).
       01 WS-VNUM                                GLOBAL    PIC Z(3)9.
       *>   The CRUD tabs: the version being edited (0 = a new one), the
       *>   grid cell a click landed on, and the confirmation dialog's text.

       01 WS-EDIT-VER                            GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-SAVE-VER                            GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-SAVE-OK                             GLOBAL    PIC X    VALUE "Y".
       01 WS-ROW-NO                              GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-COL-NO                              GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-KEY                                 GLOBAL    PIC X(10).
       01 WS-PREVIEW                             GLOBAL    PIC X(60).
       01 WS-TABS                                GLOBAL    PIC X(270).
       01 WS-QUESTION                            GLOBAL    PIC X(300).
       01 WS-ANSWER                              GLOBAL    PIC X.
       01 WS-FS-ERR                              GLOBAL    PIC XX.
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-NOW                            GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   OPEN-TOPIC-FIRST

           05 FILLER                             PIC    X(130)    VALUE "Open a topic first.".
           05 FILLER                             PIC    X(130)    VALUE "Abra um tópico primeiro.".
           05 FILLER                             PIC    X(130)    VALUE "Primero abre un tema.".
           05 FILLER                             PIC    X(130)    VALUE "Ouvrez d'abord un sujet.".
           05 FILLER                             PIC    X(130)    VALUE "先にトピックを開いてください。".
           05 FILLER                             PIC    X(130)    VALUE "请先打开一个主题。".
           *>   PROMPT-OF

           05 FILLER                             PIC    X(130)    VALUE "Prompt - &1".
           05 FILLER                             PIC    X(130)    VALUE "Prompt - &1".
           05 FILLER                             PIC    X(130)    VALUE "Prompt - &1".
           05 FILLER                             PIC    X(130)    VALUE "Prompt - &1".
           05 FILLER                             PIC    X(130)    VALUE "プロンプト - &1".
           05 FILLER                             PIC    X(130)    VALUE "提示词 - &1".
           *>   PICK-VERSION-FIRST

           05 FILLER                             PIC    X(130)    VALUE "Pick a version in the list first.".
           05 FILLER                             PIC    X(130)    VALUE "Escolha uma versão na lista primeiro.".
           05 FILLER                             PIC    X(130)    VALUE "Primero elige una versión de la lista.".
           05 FILLER                             PIC    X(130)    VALUE "Choisissez d'abord une version dans la liste.".
           05 FILLER                             PIC    X(130)    VALUE "先に一覧からバージョンを選んでください。".
           05 FILLER                             PIC    X(130)    VALUE "请先在列表中选择一个版本。".
           *>   EDITING-V

           05 FILLER                             PIC    X(130)    VALUE "Editing v&1".
           05 FILLER                             PIC    X(130)    VALUE "Editando a v&1".
           05 FILLER                             PIC    X(130)    VALUE "Editando la v&1".
           05 FILLER                             PIC    X(130)    VALUE "Modification de la v&1".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を編集中".
           05 FILLER                             PIC    X(130)    VALUE "正在编辑 v&1".
           *>   ALREADY-ACTIVE

           05 FILLER                             PIC    X(130)    VALUE "That version is already the active one.".
           05 FILLER                             PIC    X(130)    VALUE "Essa versão já é a ativa.".
           05 FILLER                             PIC    X(130)    VALUE "Esa versión ya es la activa.".
           05 FILLER                             PIC    X(130)    VALUE "Cette version est déjà la version active.".
           05 FILLER                             PIC    X(130)    VALUE "そのバージョンはすでに有効です。".
           05 FILLER                             PIC    X(130)    VALUE "该版本已经是当前版本。".
           *>   CONFIRM-ACTIVATE

           05 FILLER                             PIC    X(130)    VALUE "Make v&1 the active prompt?".
           05 FILLER                             PIC    X(130)    VALUE "Tornar a v&1 o prompt ativo?".
           05 FILLER                             PIC    X(130)    VALUE "¿Hacer de la v&1 el prompt activo?".
           05 FILLER                             PIC    X(130)    VALUE "Faire de la v&1 le prompt actif ?".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を有効なプロンプトにしますか？".
           05 FILLER                             PIC    X(130)    VALUE "将 v&1 设为当前提示词？".
           *>   NOW-ACTIVE

           05 FILLER                             PIC    X(130)    VALUE "v&1 is the active prompt.".
           05 FILLER                             PIC    X(130)    VALUE "A v&1 é o prompt ativo.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 es el prompt activo.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 est le prompt actif.".
           05 FILLER                             PIC    X(130)    VALUE "v&1 が有効なプロンプトになりました。".
           05 FILLER                             PIC    X(130)    VALUE "v&1 现在是当前提示词。".
           *>   PROMPT-EMPTY

           05 FILLER                             PIC    X(130)    VALUE "The prompt is empty.".
           05 FILLER                             PIC    X(130)    VALUE "O prompt está vazio.".
           05 FILLER                             PIC    X(130)    VALUE "El prompt está vacío.".
           05 FILLER                             PIC    X(130)    VALUE "Le prompt est vide.".
           05 FILLER                             PIC    X(130)    VALUE "プロンプトが空です。".
           05 FILLER                             PIC    X(130)    VALUE "提示词为空。".
           *>   SAVED-AS

           05 FILLER                             PIC    X(130)    VALUE "Saved as v&1, now active.".
           05 FILLER                             PIC    X(130)    VALUE "Salvo como v&1, agora ativo.".
           05 FILLER                             PIC    X(130)    VALUE "Guardado como v&1, ahora activo.".
           05 FILLER                             PIC    X(130)    VALUE "Enregistré comme v&1, désormais actif.".
           05 FILLER                             PIC    X(130)    VALUE "v&1 として保存し、有効にしました。".
           05 FILLER                             PIC    X(130)    VALUE "已保存为 v&1，现已启用。".
           *>   ACTIVE-MARK

           05 FILLER                             PIC    X(130)    VALUE "[active]".
           05 FILLER                             PIC    X(130)    VALUE "[ativa]".
           05 FILLER                             PIC    X(130)    VALUE "[activa]".
           05 FILLER                             PIC    X(130)    VALUE "[active]".
           05 FILLER                             PIC    X(130)    VALUE "[有効]".
           05 FILLER                             PIC    X(130)    VALUE "[当前]".
           *>   PROMPT-TITLE

           05 FILLER                             PIC    X(130)    VALUE "Prompt".
           05 FILLER                             PIC    X(130)    VALUE "Prompt".
           05 FILLER                             PIC    X(130)    VALUE "Prompt".
           05 FILLER                             PIC    X(130)    VALUE "Prompt".
           05 FILLER                             PIC    X(130)    VALUE "プロンプト".
           05 FILLER                             PIC    X(130)    VALUE "提示词".
           *>   HINT-PROMPT-EDITOR

           05 FILLER                             PIC    X(130)    VALUE "What the assistant is for and how it answers - the topic's system prompt".
           05 FILLER                             PIC    X(130)    VALUE "Para que serve o assistente e como ele responde - o prompt de sistema do tópico".
           05 FILLER                             PIC    X(130)    VALUE "Para qué sirve el asistente y cómo responde: el prompt de sistema del tema".
           05 FILLER                             PIC    X(130)    VALUE "À quoi sert l'assistant et comment il répond : le prompt système du sujet".
           05 FILLER                             PIC    X(130)    VALUE "アシスタントの役割と回答の仕方 - トピックのシステムプロンプト".
           05 FILLER                             PIC    X(130)    VALUE "助手的用途及回答方式——该主题的系统提示词".
           *>   NEW-VERSION

           05 FILLER                             PIC    X(130)    VALUE "New version".
           05 FILLER                             PIC    X(130)    VALUE "Nova versão".
           05 FILLER                             PIC    X(130)    VALUE "Nueva versión".
           05 FILLER                             PIC    X(130)    VALUE "Nouvelle version".
           05 FILLER                             PIC    X(130)    VALUE "新しいバージョン".
           05 FILLER                             PIC    X(130)    VALUE "新版本".
           *>   ACTIVATE

           05 FILLER                             PIC    X(130)    VALUE "Activate".
           05 FILLER                             PIC    X(130)    VALUE "Ativar".
           05 FILLER                             PIC    X(130)    VALUE "Activar".
           05 FILLER                             PIC    X(130)    VALUE "Activer".
           05 FILLER                             PIC    X(130)    VALUE "有効にする".
           05 FILLER                             PIC    X(130)    VALUE "启用".
           *>   SAVE

           05 FILLER                             PIC    X(130)    VALUE "Save".
           05 FILLER                             PIC    X(130)    VALUE "Salvar".
           05 FILLER                             PIC    X(130)    VALUE "Guardar".
           05 FILLER                             PIC    X(130)    VALUE "Enregistrer".
           05 FILLER                             PIC    X(130)    VALUE "保存".
           05 FILLER                             PIC    X(130)    VALUE "保存".
           *>   STATUS

           05 FILLER                             PIC    X(130)    VALUE "Status".
           05 FILLER                             PIC    X(130)    VALUE "Status".
           05 FILLER                             PIC    X(130)    VALUE "Estado".
           05 FILLER                             PIC    X(130)    VALUE "État".
           05 FILLER                             PIC    X(130)    VALUE "ステータス".
           05 FILLER                             PIC    X(130)    VALUE "状态".
           *>   TAB-BROWSE

           05 FILLER                             PIC    X(130)    VALUE "Browse".
           05 FILLER                             PIC    X(130)    VALUE "Consultar".
           05 FILLER                             PIC    X(130)    VALUE "Consultar".
           05 FILLER                             PIC    X(130)    VALUE "Parcourir".
           05 FILLER                             PIC    X(130)    VALUE "一覧".
           05 FILLER                             PIC    X(130)    VALUE "浏览".
           *>   TAB-EDIT

           05 FILLER                             PIC    X(130)    VALUE "Create/Update".
           05 FILLER                             PIC    X(130)    VALUE "Criar/Atualizar".
           05 FILLER                             PIC    X(130)    VALUE "Crear/Actualizar".
           05 FILLER                             PIC    X(130)    VALUE "Créer/Modifier".
           05 FILLER                             PIC    X(130)    VALUE "作成/更新".
           05 FILLER                             PIC    X(130)    VALUE "新建/更新".
           *>   NEW

           05 FILLER                             PIC    X(130)    VALUE "New".
           05 FILLER                             PIC    X(130)    VALUE "Novo".
           05 FILLER                             PIC    X(130)    VALUE "Nuevo".
           05 FILLER                             PIC    X(130)    VALUE "Nouveau".
           05 FILLER                             PIC    X(130)    VALUE "新規".
           05 FILLER                             PIC    X(130)    VALUE "新建".
           *>   CANCEL

           05 FILLER                             PIC    X(130)    VALUE "Cancel".
           05 FILLER                             PIC    X(130)    VALUE "Cancelar".
           05 FILLER                             PIC    X(130)    VALUE "Cancelar".
           05 FILLER                             PIC    X(130)    VALUE "Annuler".
           05 FILLER                             PIC    X(130)    VALUE "キャンセル".
           05 FILLER                             PIC    X(130)    VALUE "取消".
           *>   YES

           05 FILLER                             PIC    X(130)    VALUE "Yes".
           05 FILLER                             PIC    X(130)    VALUE "Sim".
           05 FILLER                             PIC    X(130)    VALUE "Sí".
           05 FILLER                             PIC    X(130)    VALUE "Oui".
           05 FILLER                             PIC    X(130)    VALUE "はい".
           05 FILLER                             PIC    X(130)    VALUE "是".
           *>   NO

           05 FILLER                             PIC    X(130)    VALUE "No".
           05 FILLER                             PIC    X(130)    VALUE "Não".
           05 FILLER                             PIC    X(130)    VALUE "No".
           05 FILLER                             PIC    X(130)    VALUE "Non".
           05 FILLER                             PIC    X(130)    VALUE "いいえ".
           05 FILLER                             PIC    X(130)    VALUE "否".
           *>   CONFIRM-DELETE

           05 FILLER                             PIC    X(130)    VALUE "Delete v&1? This cannot be undone.".
           05 FILLER                             PIC    X(130)    VALUE "Excluir a v&1? Isso não pode ser desfeito.".
           05 FILLER                             PIC    X(130)    VALUE "¿Eliminar la v&1? No se puede deshacer.".
           05 FILLER                             PIC    X(130)    VALUE "Supprimer la v&1 ? Cette action est irréversible.".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を削除しますか？元に戻せません。".
           05 FILLER                             PIC    X(130)    VALUE "删除 v&1？此操作无法撤销。".
           *>   ACTIVE-NO-DELETE

           05 FILLER                             PIC    X(130)    VALUE "v&1 is the active prompt and cannot be deleted. Activate another version first.".
           05 FILLER                             PIC    X(130)    VALUE "A v&1 é o prompt ativo e não pode ser excluída. Ative outra versão antes.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 es el prompt activo y no se puede eliminar. Activa antes otra versión.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 est le prompt actif : impossible de la supprimer. Activez d'abord une autre version.".
           05 FILLER                             PIC    X(130)    VALUE "有効な v&1 は削除できません。先に別のバージョンを有効にしてください。".
           05 FILLER                             PIC    X(130)    VALUE "v&1 是当前提示词，无法删除。请先启用其他版本。".
           *>   DELETED

           05 FILLER                             PIC    X(130)    VALUE "v&1 deleted.".
           05 FILLER                             PIC    X(130)    VALUE "A v&1 foi excluída.".
           05 FILLER                             PIC    X(130)    VALUE "Se eliminó la v&1.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 a été supprimée.".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を削除しました。".
           05 FILLER                             PIC    X(130)    VALUE "已删除 v&1。".
           *>   SAVED-V

           05 FILLER                             PIC    X(130)    VALUE "v&1 saved.".
           05 FILLER                             PIC    X(130)    VALUE "A v&1 foi salva.".
           05 FILLER                             PIC    X(130)    VALUE "Se guardó la v&1.".
           05 FILLER                             PIC    X(130)    VALUE "La v&1 a été enregistrée.".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を保存しました。".
           05 FILLER                             PIC    X(130)    VALUE "已保存 v&1。".
           *>   SAVE-FAILED

           05 FILLER                             PIC    X(130)    VALUE "Could not save v&1 (file status &2).".
           05 FILLER                             PIC    X(130)    VALUE "Não foi possível salvar a v&1 (status &2).".
           05 FILLER                             PIC    X(130)    VALUE "No se pudo guardar la v&1 (estado &2).".
           05 FILLER                             PIC    X(130)    VALUE "Impossible d'enregistrer la v&1 (état &2).".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を保存できませんでした (状態 &2)。".
           05 FILLER                             PIC    X(130)    VALUE "无法保存 v&1（状态 &2）。".
           *>   DELETE-FAILED

           05 FILLER                             PIC    X(130)    VALUE "Could not delete v&1 (file status &2).".
           05 FILLER                             PIC    X(130)    VALUE "Não foi possível excluir a v&1 (status &2).".
           05 FILLER                             PIC    X(130)    VALUE "No se pudo eliminar la v&1 (estado &2).".
           05 FILLER                             PIC    X(130)    VALUE "Impossible de supprimer la v&1 (état &2).".
           05 FILLER                             PIC    X(130)    VALUE "v&1 を削除できませんでした (状態 &2)。".
           05 FILLER                             PIC    X(130)    VALUE "无法删除 v&1（状态 &2）。".
           *>   COL-VERSION

           05 FILLER                             PIC    X(130)    VALUE "Version".
           05 FILLER                             PIC    X(130)    VALUE "Versão".
           05 FILLER                             PIC    X(130)    VALUE "Versión".
           05 FILLER                             PIC    X(130)    VALUE "Version".
           05 FILLER                             PIC    X(130)    VALUE "バージョン".
           05 FILLER                             PIC    X(130)    VALUE "版本".
           *>   COL-CREATED

           05 FILLER                             PIC    X(130)    VALUE "Created".
           05 FILLER                             PIC    X(130)    VALUE "Criada em".
           05 FILLER                             PIC    X(130)    VALUE "Creada".
           05 FILLER                             PIC    X(130)    VALUE "Créée le".
           05 FILLER                             PIC    X(130)    VALUE "作成日時".
           05 FILLER                             PIC    X(130)    VALUE "创建时间".
           *>   COL-ACTIVE

           05 FILLER                             PIC    X(130)    VALUE "Active".
           05 FILLER                             PIC    X(130)    VALUE "Ativa".
           05 FILLER                             PIC    X(130)    VALUE "Activa".
           05 FILLER                             PIC    X(130)    VALUE "Active".
           05 FILLER                             PIC    X(130)    VALUE "有効".
           05 FILLER                             PIC    X(130)    VALUE "当前".
           *>   COL-TEXT

           05 FILLER                             PIC    X(130)    VALUE "Text".
           05 FILLER                             PIC    X(130)    VALUE "Texto".
           05 FILLER                             PIC    X(130)    VALUE "Texto".
           05 FILLER                             PIC    X(130)    VALUE "Texte".
           05 FILLER                             PIC    X(130)    VALUE "テキスト".
           05 FILLER                             PIC    X(130)    VALUE "文本".
           *>   MAIN-PROMPT

           05 FILLER                             PIC    X(130)    VALUE "Main prompt".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal".
           05 FILLER                             PIC    X(130)    VALUE "メインプロンプト".
           05 FILLER                             PIC    X(130)    VALUE "主提示词".
           *>   TOPIC-PROMPT

           05 FILLER                             PIC    X(130)    VALUE "Topic prompt".
           05 FILLER                             PIC    X(130)    VALUE "Prompt do tópico".
           05 FILLER                             PIC    X(130)    VALUE "Prompt del tema".
           05 FILLER                             PIC    X(130)    VALUE "Prompt du sujet".
           05 FILLER                             PIC    X(130)    VALUE "トピックのプロンプト".
           05 FILLER                             PIC    X(130)    VALUE "主题提示词".
           *>   RESTORE-DEFAULT

           05 FILLER                             PIC    X(130)    VALUE "Restore default".
           05 FILLER                             PIC    X(130)    VALUE "Restaurar padrão".
           05 FILLER                             PIC    X(130)    VALUE "Restaurar predeterminado".
           05 FILLER                             PIC    X(130)    VALUE "Rétablir par défaut".
           05 FILLER                             PIC    X(130)    VALUE "既定に戻す".
           05 FILLER                             PIC    X(130)    VALUE "恢复默认".
           *>   MAIN-TITLE

           05 FILLER                             PIC    X(130)    VALUE "Main prompt - every instruction, for every topic".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal - todas as instruções, para todos os tópicos".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal: todas las instrucciones, para todos los temas".
           05 FILLER                             PIC    X(130)    VALUE "Prompt principal : toutes les instructions, pour tous les sujets".
           05 FILLER                             PIC    X(130)    VALUE "メインプロンプト - すべてのトピックへのすべての指示".
           05 FILLER                             PIC    X(130)    VALUE "主提示词——适用于所有主题的全部指令".
           *>   HINT-MAIN-EDITOR

           05 FILLER                             PIC    X(130)    VALUE "Every instruction the models get, in English, section by section (=== NAME ===)".
           05 FILLER                             PIC    X(130)    VALUE "Todas as instruções dadas aos modelos, em inglês, seção por seção (=== NAME ===)".
           05 FILLER                             PIC    X(130)    VALUE "Todas las instrucciones que reciben los modelos, en inglés, sección por sección (=== NAME ===)".
           05 FILLER                             PIC    X(130)    VALUE "Toutes les instructions données aux modèles, en anglais, section par section (=== NAME ===)".
           05 FILLER                             PIC    X(130)    VALUE "モデルへのすべての指示（英語、セクションごと: === NAME ===）".
           05 FILLER                             PIC    X(130)    VALUE "模型收到的全部指令（英文，按节：=== NAME ===）".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    37.
               10 PC-TEXT                        PIC    X(130)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-OPEN-TOPIC-FIRST                 PIC    X(130).
           05 T-PROMPT-OF                        PIC    X(130).
           05 T-PICK-VERSION-FIRST               PIC    X(130).
           05 T-EDITING-V                        PIC    X(130).
           05 T-ALREADY-ACTIVE                   PIC    X(130).
           05 T-CONFIRM-ACTIVATE                 PIC    X(130).
           05 T-NOW-ACTIVE                       PIC    X(130).
           05 T-PROMPT-EMPTY                     PIC    X(130).
           05 T-SAVED-AS                         PIC    X(130).
           05 T-ACTIVE-MARK                      PIC    X(130).
           05 T-PROMPT-TITLE                     PIC    X(130).
           05 T-HINT-PROMPT-EDITOR               PIC    X(130).
           05 T-NEW-VERSION                      PIC    X(130).
           05 T-ACTIVATE                         PIC    X(130).
           05 T-SAVE                             PIC    X(130).
           05 T-STATUS                           PIC    X(130).
           05 T-TAB-BROWSE                       PIC    X(130).
           05 T-TAB-EDIT                         PIC    X(130).
           05 T-NEW                              PIC    X(130).
           05 T-CANCEL                           PIC    X(130).
           05 T-YES                              PIC    X(130).
           05 T-NO                               PIC    X(130).
           05 T-CONFIRM-DELETE                   PIC    X(130).
           05 T-ACTIVE-NO-DELETE                 PIC    X(130).
           05 T-DELETED                          PIC    X(130).
           05 T-SAVED-V                          PIC    X(130).
           05 T-SAVE-FAILED                      PIC    X(130).
           05 T-DELETE-FAILED                    PIC    X(130).
           05 T-COL-VERSION                      PIC    X(130).
           05 T-COL-CREATED                      PIC    X(130).
           05 T-COL-ACTIVE                       PIC    X(130).
           05 T-COL-TEXT                         PIC    X(130).
           05 T-MAIN-PROMPT                      PIC    X(130).
           05 T-TOPIC-PROMPT                     PIC    X(130).
           05 T-RESTORE-DEFAULT                  PIC    X(130).
           05 T-MAIN-TITLE                       PIC    X(130).
           05 T-HINT-MAIN-EDITOR                 PIC    X(130).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(130)    OCCURS 37.
       *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.

       01 WS-FMT                                 GLOBAL    PIC X(130).
       01 WS-ARG1                                GLOBAL    PIC X(300).
       01 WS-ARG2                                GLOBAL    PIC X(300).
       01 WS-ARG3                                GLOBAL    PIC X(300).
       01 WS-ARG4                                GLOBAL    PIC X(300).
       01 WS-FMT-OUT                             GLOBAL    PIC X(1200).

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
           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM PROMPTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROMPTS-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back on the pane: the topic or the language may have changed.

           CALL    "PC-OPEN"

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
           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-LANG--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the chat's menu changes the language while this form is
           *>   on the pane; nothing tells a pane occupant, so it looks (R46).

           CALL    "PC-LANG-NOW"
           IF    WS-LANG-NOW NOT = WS-LANG
               CALL    "PROMPTS-FORM--ONACTIVATE"
           END-IF

           GOBACK.

       END PROGRAM TMR-LANG--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-NEW"

           GOBACK.

       END PROGRAM BTN-NEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MAIN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The main prompt: every instruction the models get, every topic.

           SET     WS-SCOPE                      TO    "M"
           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM BTN-MAIN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-TOPIC--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back to the open topic's own prompt.

           SET     WS-SCOPE                      TO    "T"
           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM BTN-TOPIC--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-DEFAULT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The main prompt as shipped, saved as a new version and made the
           *>   active one - the versions before it stay in the list.

           CALL    "PC-MAIN-DEFAULT"

           SET     WS-EDIT-VER                   TO    0
           SET     Txt-Prompt::Text              TO    WS-TEXT

           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-DEFAULT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DG-LIST--ONCELLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Column 1 holds the version number (the key); 5 promotes, 6 edits
           *>   and 7 deletes. Promoting and deleting ask first (R48).

           SET     WS-ROW-NO                     TO    Dg-List::ClickedRow
           SET     WS-COL-NO                     TO    Dg-List::ClickedColumn

           IF    WS-ROW-NO < 1 OR WS-COL-NO < 5
               EXIT PROGRAM
           END-IF

           SET     WS-KEY                        TO    Dg-List::GetCellValue(WS-ROW-NO, 1)
           SET     WS-PICKED                     TO    0

           IF    WS-KEY::Trim() NOT = SPACES
               COMPUTE    WS-PICKED = FUNCTION NUMVAL(WS-KEY)
           END-IF
           IF    WS-PICKED = 0
               SET     Lbl-Status::Caption       TO    T-PICK-VERSION-FIRST::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-VNUM                       TO    WS-PICKED
           SET     WS-ARG1                       TO    WS-VNUM

           EVALUATE    WS-COL-NO
               WHEN 5
                   IF    WS-PICKED = WS-ACTIVE-VER
                       SET     Lbl-Status::Caption
                                                 TO    T-ALREADY-ACTIVE::Trim()

                       EXIT PROGRAM
                   END-IF
                   SET     WS-FMT                TO    T-CONFIRM-ACTIVATE
                   CALL    "PC-FMT"
                   SET     WS-QUESTION           TO    WS-FMT-OUT::Trim()

                   INVOKE    ME::SetProperty("ConfirmText", WS-QUESTION)
                   INVOKE    ME::SetProperty("ConfirmYes", T-ACTIVATE)
                   INVOKE    ME::SetProperty("ConfirmNo", T-NO)
                   INVOKE    ME::SetProperty("ConfirmAnswer", "N")
                   INVOKE    ME::OpenFormSync("CONFIRM-FORM")
                   INVOKE    ME::GetProperty("ConfirmAnswer") RETURNING WS-ANSWER

                   IF    WS-ANSWER = "Y"
                       CALL    "PC-ACTIVATE"

                       SET     WS-FMT            TO    T-NOW-ACTIVE
                       SET     WS-ARG1           TO    WS-VNUM

                       CALL    "PC-FMT"
                       SET     Lbl-Status::Caption
                                                 TO    WS-FMT-OUT::Trim()

                       CALL    "PC-LOAD-VERSIONS"
                   END-IF
               WHEN 6
                   CALL    "PC-EDIT"
               WHEN 7
                   IF    WS-PICKED = WS-ACTIVE-VER
                       SET     WS-FMT            TO    T-ACTIVE-NO-DELETE
                       CALL    "PC-FMT"
                       SET     Lbl-Status::Caption
                                                 TO    WS-FMT-OUT::Trim()

                       EXIT PROGRAM
                   END-IF
                   SET     WS-FMT                TO    T-CONFIRM-DELETE
                   CALL    "PC-FMT"
                   SET     WS-QUESTION           TO    WS-FMT-OUT::Trim()

                   INVOKE    ME::SetProperty("ConfirmText", WS-QUESTION)
                   INVOKE    ME::SetProperty("ConfirmYes", T-YES)
                   INVOKE    ME::SetProperty("ConfirmNo", T-NO)
                   INVOKE    ME::SetProperty("ConfirmAnswer", "N")
                   INVOKE    ME::OpenFormSync("CONFIRM-FORM")
                   INVOKE    ME::GetProperty("ConfirmAnswer") RETURNING WS-ANSWER

                   IF    WS-ANSWER = "Y"
                       CALL    "PC-DELETE"
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

           SET     WS-EDIT-VER                   TO    0
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE-BOTTOM--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE-BOTTOM--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL-BOTTOM--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back to the list as it was: nothing saved, nothing re-read.

           SET     WS-EDIT-VER                   TO    0
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM BTN-CANCEL-BOTTOM--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PATHS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Where PowerChat keeps its own files: POWERCHAT_DATA, else "data".

           DISPLAY    "POWERCHAT_DATA" UPON ENVIRONMENT-NAME
           ACCEPT    WS-DATA-DIR FROM ENVIRONMENT-VALUE
           IF    WS-DATA-DIR = SPACES
               SET     WS-DATA-DIR               TO    "data"
           END-IF
           SET     WS-SETTINGS-PATH              TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/settings.idx" INTO WS-SETTINGS-PATH
           SET     WS-TOPICS-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/topics.idx" INTO WS-TOPICS-PATH
           SET     WS-PROMPTS-PATH               TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/prompt-versions.idx" INTO WS-PROMPTS-PATH
           SET     WS-OLDPRM-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/prompts.idx" INTO WS-OLDPRM-PATH
           DISPLAY    "POWERCHAT_SAMPLES" UPON ENVIRONMENT-NAME
           ACCEPT    WS-SAMPLES-DIR FROM ENVIRONMENT-VALUE
           IF    WS-SAMPLES-DIR = SPACES
               SET     WS-SAMPLES-DIR            TO    "samples"
           END-IF
           SET     WS-MPSEED-PATH                TO    SPACES
           STRING    WS-SAMPLES-DIR::Trim() "/main-prompt.md" INTO WS-MPSEED-PATH

           GOBACK.

       END PROGRAM PC-PATHS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-GET IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-SET-NAME in, WS-SET-VALUE out (spaces when the setting is unset).

           SET     WS-SET-VALUE                  TO    SPACES
           *> Status 35 is a missing file (first run): create it empty and reopen
           *> I-O, so the read below simply finds no record.

           OPEN    I-O SETTINGS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT SETTINGS-FILE
               CLOSE    SETTINGS-FILE
               OPEN    I-O SETTINGS-FILE
           END-IF
           SET     SET-NAME                      TO    WS-SET-NAME
           READ    SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE SET-VALUE TO WS-SET-VALUE
           END-READ
           CLOSE    SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-GET.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-PUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-SET-NAME, WS-SET-VALUE in. Written and committed at once (R10f).

           OPEN    I-O SETTINGS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT SETTINGS-FILE
               CLOSE    SETTINGS-FILE
               OPEN    I-O SETTINGS-FILE
           END-IF

           SET     SET-NAME                      TO    WS-SET-NAME
           SET     SET-VALUE                     TO    WS-SET-VALUE

           WRITE    SETTINGS-REC
               INVALID KEY REWRITE SETTINGS-REC
           END-WRITE
           COMMIT
           CLOSE    SETTINGS-FILE

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

           SET     WS-OK                         TO    Dg-List::ClearRows()
           SET     WS-VER-COUNT WS-LAST-VER WS-ACTIVE-VER
                                                 TO    0

           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF

           *> PRM-KEY is topic then version, so from version 0 the topic's records
           *> come oldest first: WS-LAST-VER ends as the newest, and the first
           *> record of another topic ends the scan.

           SET     PRM-TOPIC                     TO    WS-CUR-TOPIC
           SET     PRM-VERSION                   TO    0
           SET     WS-EOF                        TO    "N"

           START    PROMPTS-FILE KEY IS >= PRM-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    PROMPTS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    PRM-TOPIC NOT = WS-CUR-TOPIC
                           SET     WS-EOF        TO    "Y"
                       ELSE
                           SET     WS-LAST-VER   TO    PRM-VERSION
                           IF    PRM-ACTIVE = "Y"
                               SET     WS-ACTIVE-VER
                                                 TO    PRM-VERSION
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           IF    WS-LAST-VER = 0
               SET     PRM-TOPIC                 TO    WS-CUR-TOPIC
               SET     PRM-VERSION WS-LAST-VER WS-ACTIVE-VER
                                                 TO    1

               SET     WS-NOW                    TO    FUNCTION CURRENT-DATE
               SET     PRM-CREATED               TO    WS-NOW(1:14)
               SET     PRM-ACTIVE                TO    "Y"

               *> The main prompt is not a topic: its first version is the shipped
               *> default (samples/main-prompt.md), not a topic's prompt.

               IF    WS-CUR-TOPIC = "*MAIN"
                   CALL    "PC-MAIN-DEFAULT"

                   SET     PRM-TOPIC             TO    WS-CUR-TOPIC
                   SET     PRM-VERSION           TO    1
                   SET     PRM-ACTIVE            TO    "Y"
                   SET     PRM-TEXT              TO    WS-TEXT
               ELSE
                   SET     PRM-TEXT              TO    TOP-PROMPT
               END-IF
               WRITE    PROMPT-REC
                   INVALID KEY CONTINUE
               END-WRITE
               COMMIT
           END-IF
           *>   Newest first: walk down from the last version.

           PERFORM    VARYING WS-I FROM WS-LAST-VER BY -1 UNTIL WS-I < 1
               SET     PRM-TOPIC                 TO    WS-CUR-TOPIC
               SET     PRM-VERSION               TO    WS-I

               READ    PROMPTS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY
                       IF    WS-VER-COUNT < 200
                           ADD    1 TO WS-VER-COUNT
                           SET     WS-VER(WS-VER-COUNT)
                                                 TO    PRM-VERSION

                           SET     WS-VNUM       TO    PRM-VERSION
                           *>   A tab or a line break inside the text would split the row.

                           SET     WS-PREVIEW    TO    PRM-TEXT(1:57)
                           INSPECT    WS-PREVIEW REPLACING ALL X"09" BY SPACE
                               ALL X"0A" BY SPACE
                               ALL X"0D" BY SPACE
                           IF    PRM-TEXT(58:) NOT = SPACES
                               SET     WS-LINE   TO    SPACES
                               STRING    WS-PREVIEW::Trim() "..." INTO WS-LINE
                               SET     WS-PREVIEW
                                                 TO    WS-LINE
                           END-IF
                           SET     WS-LINE       TO    SPACES
                           IF    PRM-ACTIVE = "Y"
                               SET     WS-LINE   TO    T-ACTIVE-MARK
                           END-IF
                           SET     WS-ROW        TO    SPACES
                           *> One tab-separated row, in the grid's column order. WS-VNUM is
                           *> zero-suppressed (3, not 0003); PRM-CREATED is YYYYMMDDHHMMSS and
                           *> shows as YYYY-MM-DD HH:MM.

                           STRING    WS-VNUM::Trim() X"09"
                               PRM-CREATED(1:4) "-" PRM-CREATED(5:2) "-"
                               PRM-CREATED(7:2) " " PRM-CREATED(9:2) ":"
                               PRM-CREATED(11:2) X"09"
                           WS-LINE::Trim() X"09"
                           WS-PREVIEW::Trim() X"09"
                               "icon:check-circle" X"09"
                               "icon:pencil" X"09"
                               "icon:trash"
                               INTO WS-ROW

                           SET     WS-OK         TO    Dg-List::AddRow(WS-ROW)
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    PROMPTS-FILE

           GOBACK.

       END PROGRAM PC-LOAD-VERSIONS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-PICKED in: that version becomes the active one, and its text the
           *>   topic's prompt, which the chat's orchestrator is given (R63).

           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF
           *> Exactly one version stays active: the picked one is switched on and
           *> every other version of the topic is switched off in the same pass.

           PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LAST-VER
               SET     PRM-TOPIC                 TO    WS-CUR-TOPIC
               SET     PRM-VERSION               TO    WS-I

               READ    PROMPTS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY
                       IF    WS-I = WS-PICKED
                           SET     PRM-ACTIVE    TO    "Y"
                           SET     WS-TEXT       TO    PRM-TEXT
                       ELSE
                           SET     PRM-ACTIVE    TO    "N"
                       END-IF
                       REWRITE    PROMPT-REC
                       END-REWRITE
               END-READ
           END-PERFORM
           COMMIT
           CLOSE    PROMPTS-FILE
           SET     WS-ACTIVE-VER                 TO    WS-PICKED
           *> The main prompt is no topic: there is no topic record to copy it to.

           IF    WS-CUR-TOPIC = "*MAIN"
               EXIT PROGRAM
           END-IF
           OPEN    I-O TOPICS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TOPICS-FILE
               CLOSE    TOPICS-FILE
               OPEN    I-O TOPICS-FILE
           END-IF
           SET     TOP-ID                        TO    WS-CUR-TOPIC
           READ    TOPICS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   SET     TOP-PROMPT            TO    WS-TEXT
                   REWRITE    TOPIC-REC
                   END-REWRITE
                   COMMIT
           END-READ
           CLOSE    TOPICS-FILE
           SET     WS-ACTIVE-VER                 TO    WS-PICKED

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

           SET     WS-SET-NAME                   TO    "LANG"
           CALL    "PC-SETTING-GET"
           SET     WS-LANG                       TO    WS-SET-VALUE(1:2)
           EVALUATE    WS-LANG
               WHEN "en" MOVE 1 TO WS-LANG-IX
               WHEN "pt" MOVE 2 TO WS-LANG-IX
               WHEN "es" MOVE 3 TO WS-LANG-IX
               WHEN "fr" MOVE 4 TO WS-LANG-IX
               WHEN "jp" MOVE 5 TO WS-LANG-IX
               WHEN "cn" MOVE 6 TO WS-LANG-IX
               WHEN OTHER MOVE "en" TO WS-LANG
                   SET     WS-LANG-IX            TO    1
           END-EVALUATE
           *> PC-TEXT-NOW overlays the T- names, so row N of the table becomes the
           *> Nth name. The bound is the number of texts (the OCCURS of the table).

           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 37
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Lbl-Title::Caption            TO    T-PROMPT-TITLE::Trim()
           SET     Txt-Prompt::HintText          TO    T-HINT-PROMPT-EDITOR::Trim()
           SET     Btn-New::Caption              TO    T-NEW::Trim()
           SET     Btn-Save::Caption             TO    T-SAVE::Trim()
           SET     Btn-Save-Bottom::Caption      TO    T-SAVE::Trim()
           SET     Btn-Cancel::Caption           TO    T-CANCEL::Trim()
           SET     Btn-Cancel-Bottom::Caption    TO    T-CANCEL::Trim()
           SET     Btn-Main::Caption             TO    T-MAIN-PROMPT::Trim()
           SET     Btn-Topic::Caption            TO    T-TOPIC-PROMPT::Trim()
           SET     Btn-Default::Caption          TO    T-RESTORE-DEFAULT::Trim()
           SET     Lbl-Status::Caption           TO    T-STATUS::Trim()
           SET     WS-TABS                       TO    SPACES

           STRING    T-TAB-BROWSE::Trim() X"0A" T-TAB-EDIT::Trim() INTO WS-TABS
           SET     Tab-Crud::Tabs                TO    WS-TABS

           Dg-List::SetColumnTitle("Ver", T-COL-VERSION)
           Dg-List::SetColumnTitle("Created", T-COL-CREATED)
           Dg-List::SetColumnTitle("Active", T-COL-ACTIVE)
           Dg-List::SetColumnTitle("Text", T-COL-TEXT)

           GOBACK.

       END PROGRAM PC-TEXTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FMT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-PT                                  PIC    9(4).
       01 WS-FROM                                PIC    9(4).
       01 WS-CNT                                 PIC    9(4).
       01 WS-PART                                PIC    X(130).
       01 WS-DELIM                               PIC    XX.

       PROCEDURE DIVISION.
           *>   A message whose words go round numbers and names differently in each
           *>   language: WS-FMT holds "&1" .. "&4" where WS-ARG1..4 go.

           SET     WS-FMT-OUT                    TO    SPACES
           SET     WS-PT                         TO    1
           SET     WS-FROM                       TO    1

           PERFORM    UNTIL WS-FROM > FUNCTION LENGTH(WS-FMT)
               SET     WS-PART WS-DELIM          TO    SPACES
               SET     WS-CNT                    TO    0

               UNSTRING    WS-FMT DELIMITED BY "&1" OR "&2" OR "&3" OR "&4"
                   INTO WS-PART DELIMITER IN WS-DELIM COUNT IN WS-CNT
                   WITH POINTER WS-FROM
               END-UNSTRING
               IF    WS-CNT > 0
                   STRING    WS-PART(1:WS-CNT) INTO WS-FMT-OUT WITH POINTER WS-PT
               END-IF
               EVALUATE    WS-DELIM
                   WHEN "&1" STRING WS-ARG1::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&2" STRING WS-ARG2::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&3" STRING WS-ARG3::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&4" STRING WS-ARG4::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   *> No &n left: the tail was copied above, so move the pointer past the
                   *> end to stop the loop.
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

           CALL    "PC-PATHS"
           CALL    "PC-MIGRATE"
           CALL    "PC-TEXTS"

           SET     WS-SET-NAME                   TO    "CUR-TOPIC"
           CALL    "PC-SETTING-GET"
           SET     WS-TOPIC-ID                   TO    WS-SET-VALUE(1:16)
           *>   The main prompt: every topic's, whether one is open or not.

           IF    WS-SCOPE = "M"
               SET     WS-CUR-TOPIC              TO    "*MAIN"
               SET     Lbl-Title::Caption        TO    T-MAIN-TITLE::Trim()
               SET     Txt-Prompt::HintText      TO    T-HINT-MAIN-EDITOR::Trim()
               SET     Btn-Default::Visible      TO    TRUE

               CALL    "PC-LOAD-VERSIONS"
               SET     Tab-Crud::SelectedTab     TO    1
               EXIT PROGRAM
           END-IF

           SET     Btn-Default::Visible          TO    FALSE
           SET     WS-CUR-TOPIC                  TO    WS-TOPIC-ID

           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Status::Caption       TO    T-OPEN-TOPIC-FIRST::Trim()
               EXIT PROGRAM
           END-IF
           OPEN    I-O TOPICS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TOPICS-FILE
               CLOSE    TOPICS-FILE
               OPEN    I-O TOPICS-FILE
           END-IF
           SET     TOP-ID                        TO    WS-CUR-TOPIC
           READ    TOPICS-FILE
               INVALID KEY MOVE "?" TO TOP-NAME
           END-READ
           CLOSE    TOPICS-FILE

           SET     WS-FMT                        TO    T-PROMPT-OF
           SET     WS-ARG1                       TO    TOP-NAME

           CALL    "PC-FMT"
           SET     Lbl-Title::Caption            TO    WS-FMT-OUT::Trim()
           CALL    "PC-LOAD-VERSIONS"
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM PC-OPEN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   An empty editor on the Create/Update tab: Save adds a new version.

           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Status::Caption       TO    T-OPEN-TOPIC-FIRST::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-EDIT-VER                   TO    0
           SET     WS-TEXT                       TO    SPACES
           SET     Txt-Prompt::Text              TO    WS-TEXT
           SET     Lbl-Edit-Ver::Caption         TO    T-NEW-VERSION::Trim()
           SET     Tab-Crud::SelectedTab         TO    2

           GOBACK.

       END PROGRAM PC-NEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EDIT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-PICKED in: that version's text on the Create/Update tab.

           SET     WS-VNUM                       TO    WS-PICKED
           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF

           SET     PRM-TOPIC                     TO    WS-CUR-TOPIC
           SET     PRM-VERSION                   TO    WS-PICKED
           SET     WS-SAVE-OK                    TO    "Y"

           READ    PROMPTS-FILE
               INVALID KEY MOVE "N" TO WS-SAVE-OK
           END-READ
           CLOSE    PROMPTS-FILE
           IF    WS-SAVE-OK NOT = "Y"
               SET     Lbl-Status::Caption       TO    T-PICK-VERSION-FIRST::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-EDIT-VER                   TO    WS-PICKED
           SET     WS-TEXT                       TO    PRM-TEXT
           SET     Txt-Prompt::Text              TO    WS-TEXT
           SET     WS-FMT                        TO    T-EDITING-V
           SET     WS-ARG1                       TO    WS-VNUM

           CALL    "PC-FMT"

           SET     Lbl-Edit-Ver::Caption         TO    WS-FMT-OUT::Trim()
           SET     Tab-Crud::SelectedTab         TO    2

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

           SET     WS-TEXT                       TO    Txt-Prompt::Text
           IF    WS-TEXT = SPACES
               SET     Lbl-Status::Caption       TO    T-PROMPT-EMPTY::Trim()
               EXIT PROGRAM
           END-IF
           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Status::Caption       TO    T-OPEN-TOPIC-FIRST::Trim()
               EXIT PROGRAM
           END-IF
           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF

           SET     WS-NOW                        TO    FUNCTION CURRENT-DATE
           SET     PRM-TOPIC                     TO    WS-CUR-TOPIC

           IF    WS-EDIT-VER = 0
               COMPUTE    WS-SAVE-VER = WS-LAST-VER + 1

               SET     PRM-VERSION               TO    WS-SAVE-VER
               SET     PRM-CREATED               TO    WS-NOW(1:14)
               SET     PRM-ACTIVE                TO    "N"
           ELSE
               SET     WS-SAVE-VER               TO    WS-EDIT-VER
               SET     PRM-VERSION               TO    WS-SAVE-VER

               READ    PROMPTS-FILE
                   INVALID KEY
                       SET     PRM-CREATED       TO    WS-NOW(1:14)
                       SET     PRM-ACTIVE        TO    "N"
               END-READ

               SET     PRM-TOPIC                 TO    WS-CUR-TOPIC
               SET     PRM-VERSION               TO    WS-SAVE-VER
           END-IF

           SET     PRM-TEXT                      TO    WS-TEXT
           SET     WS-SAVE-OK                    TO    "Y"

           WRITE    PROMPT-REC
               INVALID KEY
                   REWRITE    PROMPT-REC
                       INVALID KEY MOVE "N" TO WS-SAVE-OK
                       SET     WS-FS-ERR         TO    WS-FS
                   END-REWRITE
           END-WRITE
           IF    WS-SAVE-OK = "Y"
               COMMIT
           ELSE
               ROLLBACK
           END-IF
           CLOSE    PROMPTS-FILE
           SET     WS-VNUM                       TO    WS-SAVE-VER
           IF    WS-SAVE-OK NOT = "Y"
               SET     WS-FMT                    TO    T-SAVE-FAILED
               SET     WS-ARG1                   TO    WS-VNUM
               SET     WS-ARG2                   TO    WS-FS-ERR

               CALL    "PC-FMT"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           IF    WS-EDIT-VER = 0
               *> PC-ACTIVATE walks the versions up to WS-LAST-VER, so the new one
               *> must be counted before it runs.

               SET     WS-LAST-VER               TO    WS-SAVE-VER
               SET     WS-PICKED                 TO    WS-SAVE-VER

               CALL    "PC-ACTIVATE"
               SET     WS-FMT                    TO    T-SAVED-AS
           ELSE
               IF    WS-SAVE-VER = WS-ACTIVE-VER
                   SET     WS-PICKED             TO    WS-SAVE-VER
                   CALL    "PC-ACTIVATE"
               END-IF
               SET     WS-FMT                    TO    T-SAVED-V
           END-IF
           SET     WS-ARG1                       TO    WS-VNUM
           CALL    "PC-FMT"

           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()
           SET     WS-EDIT-VER                   TO    0

           CALL    "PC-LOAD-VERSIONS"
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DELETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-PICKED in, already confirmed: that version leaves the history.
           *>   The active version is refused before the question is asked.

           SET     WS-VNUM                       TO    WS-PICKED
           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF

           SET     PRM-TOPIC                     TO    WS-CUR-TOPIC
           SET     PRM-VERSION                   TO    WS-PICKED
           SET     WS-SAVE-OK                    TO    "Y"

           DELETE    PROMPTS-FILE RECORD
               INVALID KEY MOVE "N" TO WS-SAVE-OK
               SET     WS-FS-ERR                 TO    WS-FS
           END-DELETE
           IF    WS-SAVE-OK = "Y"
               COMMIT
           ELSE
               ROLLBACK
           END-IF
           CLOSE    PROMPTS-FILE
           IF    WS-SAVE-OK = "Y"
               SET     WS-FMT                    TO    T-DELETED
           ELSE
               SET     WS-FMT                    TO    T-DELETE-FAILED
               SET     WS-ARG2                   TO    WS-FS-ERR
           END-IF
           SET     WS-ARG1                       TO    WS-VNUM
           CALL    "PC-FMT"
           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()
           CALL    "PC-LOAD-VERSIONS"

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

           OPEN    I-O PROMPTS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT PROMPTS-FILE
               CLOSE    PROMPTS-FILE
               OPEN    I-O PROMPTS-FILE
           END-IF

           *> Lowest possible key: START finds no record from here only when the
           *> new file is empty, the one time the old file is copied over.

           SET     PRM-TOPIC                     TO    SPACES
           SET     PRM-VERSION                   TO    0
           SET     WS-EMPTY                      TO    "N"

           START    PROMPTS-FILE KEY IS >= PRM-KEY
               INVALID KEY MOVE "Y" TO WS-EMPTY
           END-START
           IF    WS-EMPTY = "Y"
               OPEN    INPUT OLDPRM-FILE
               IF    WS-FS = "00"
                   SET     WS-EOF                TO    "N"
                   PERFORM    UNTIL WS-EOF = "Y"
                       READ    OLDPRM-FILE NEXT RECORD
                           AT END MOVE "Y" TO WS-EOF
                           NOT AT END
                               SET     PRM-TOPIC TO    OLD-TOPIC
                               SET     PRM-VERSION
                                                 TO    OLD-VERSION

                               SET     PRM-CREATED
                                                 TO    OLD-CREATED

                               SET     PRM-ACTIVE
                                                 TO    OLD-ACTIVE

                               SET     PRM-TEXT  TO    OLD-TEXT
                               WRITE    PROMPT-REC
                                   INVALID KEY CONTINUE
                               END-WRITE
                       END-READ
                   END-PERFORM
                   CLOSE    OLDPRM-FILE
                   COMMIT
               END-IF
           END-IF
           CLOSE    PROMPTS-FILE
           SET     WS-EOF                        TO    "N"

           GOBACK.

       END PROGRAM PC-MIGRATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MAIN-DEFAULT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-TEXT out: the main prompt as shipped - samples/main-prompt.md,
           *>   each line's indentation kept.

           SET     WS-TEXT                       TO    SPACES
           SET     WS-SX-B                       TO    1

           OPEN    INPUT MPSEED-FILE
           IF    WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF
           SET     WS-EOF                        TO    "N"
           PERFORM    UNTIL WS-EOF = "Y"
               READ    MPSEED-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       SET     WS-LN-BUF         TO    MPSEED-REC
                       *> The line's real length, trailing blanks off but indentation kept (TRIM
                       *> would strip it): leading blanks of the reversed line, out of 1000.

                       SET     WS-LN-REV         TO    FUNCTION REVERSE(WS-LN-BUF)
                       SET     WS-LN-LEN         TO    0

                       INSPECT    WS-LN-REV TALLYING WS-LN-LEN FOR LEADING SPACES
                       COMPUTE    WS-LN-LEN = 1000 - WS-LN-LEN
                       *> A blank line has nothing to copy (a zero-length piece is not valid),
                       *> only its line break.

                       IF    WS-LN-LEN > 0
                           STRING    WS-LN-BUF(1:WS-LN-LEN) X"0A" INTO WS-TEXT WITH POINTER WS-SX-B
                           END-STRING
                       ELSE
                           STRING    X"0A" INTO WS-TEXT
                               WITH POINTER WS-SX-B
                           END-STRING
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    MPSEED-FILE
           SET     WS-EOF                        TO    "N"

           GOBACK.

       END PROGRAM PC-MAIN-DEFAULT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LANG-NOW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The language saved now (LANG), into WS-LANG-NOW; "en" when unset.

           SET     WS-LANG-NOW                   TO    "en"
           OPEN    INPUT SETTINGS-FILE
           IF    WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF
           SET     SET-NAME                      TO    "LANG"
           READ    SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   EVALUATE    SET-VALUE(1:2)
                       WHEN "pt" WHEN "es" WHEN "fr" WHEN "jp" WHEN "cn"
                           SET     WS-LANG-NOW   TO    SET-VALUE(1:2)
                   END-EVALUATE
           END-READ
           CLOSE    SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-LANG-NOW.

       END PROGRAM PROMPTS-FORM.

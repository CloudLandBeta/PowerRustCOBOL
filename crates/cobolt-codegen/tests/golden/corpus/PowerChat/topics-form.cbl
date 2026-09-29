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
       PROGRAM-ID. TOPICS-FORM.

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
           SELECT TFILES-FILE ASSIGN TO WS-TFILES-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TF-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT PROMPTS-FILE ASSIGN TO WS-PROMPTS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PRM-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT SAMPLES-FILE ASSIGN TO WS-SAMPLES-LIST
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
       FD  TFILES-FILE IS GLOBAL.
      *>   The users' indexed files each topic registers by path (R20-R25):
      *>   the data file and its .cidx. Nothing is copied or attached.
       01  TFILE-REC.
           05 TF-KEY.
              10 TF-TOPIC       PIC X(16).
              10 TF-SEQ         PIC 9(3).
           05 TF-NAME           PIC X(30).
           05 TF-DATA           PIC X(250).
           05 TF-CIDX           PIC X(250).
       FD  PROMPTS-FILE IS GLOBAL.
      *>   Every version of each topic's system prompt (R48). The active one's
      *>   text is also the topic's TOP-PROMPT, which is what the chat reads.
       01  PROMPT-REC.
           05 PRM-KEY.
              10 PRM-TOPIC      PIC X(16).
              10 PRM-VERSION    PIC 9(4).
           05 PRM-CREATED       PIC X(14).
           05 PRM-ACTIVE        PIC X.
           05 PRM-TEXT          PIC X(32000).
       FD  SAMPLES-FILE IS GLOBAL.
       01  SAMPLE-LINE          PIC X(1400).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'TOPICS-FORM'.

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
       01 WS-TFILES-PATH  GLOBAL PIC X(240).
       01 WS-PROMPTS-PATH  GLOBAL PIC X(240).
       01 WS-TOPIC-COUNT     GLOBAL PIC 9(4) VALUE 0.
       01 WS-TOPIC-IDS       GLOBAL.
          05 WS-TOPIC-ID     PIC X(16) OCCURS 500.
       01 WS-INDEX           GLOBAL PIC S9(4).
       01 WS-NEW-NAME        GLOBAL PIC X(40).
       01 WS-NEW-PROMPT      GLOBAL PIC X(1000).
       01 WS-NEW-ID          GLOBAL PIC X(16).
      *>   The sample topics (spec 071 Q1): shipped in samples/, installed and
      *>   removed from here. POWERCHAT_SAMPLES moves the folder.
       01 WS-SAMPLES-DIR     GLOBAL PIC X(200).
       01 WS-SAMPLES-LIST    GLOBAL PIC X(240).
       01 WS-KIND            GLOBAL PIC X(8).
       01 WS-FOLDER          GLOBAL PIC X(30).
       01 WS-F3              GLOBAL PIC X(1000).
       01 WS-F4              GLOBAL PIC X(1000).
       01 WS-SEQ-N           GLOBAL PIC 9 VALUE 0.
       01 WS-TF-SEQ          GLOBAL PIC 9(3) VALUE 0.
       01 WS-MAP-COUNT       GLOBAL PIC 99 VALUE 0.
       01 WS-MAP             GLOBAL.
          05 WS-MAP-ROW      OCCURS 10.
             10 WS-MAP-FOLDER PIC X(30).
             10 WS-MAP-ID    PIC X(16).
       01 WS-Q-COUNT         GLOBAL PIC 99 VALUE 0.
       01 WS-Q-NEXT          GLOBAL PIC 99 VALUE 0.
       01 WS-QUEUE           GLOBAL.
          05 WS-Q            OCCURS 60.
             10 WS-Q-TOPIC   PIC X(16).
             10 WS-Q-PATH    PIC X(300).
       01 WS-J               GLOBAL PIC 99.
       01 WS-TOPIC-ID-OF     GLOBAL PIC X(16).
       01 WS-INSTALLING      GLOBAL PIC X VALUE "N".
       01 WS-N1              GLOBAL PIC Z9.
       01 WS-N2              GLOBAL PIC Z9.
       01 WS-LINE            GLOBAL PIC X(300).
       01 WS-REMOVED         GLOBAL PIC 99 VALUE 0.
       01 WS-SEQS            GLOBAL.
          05 WS-SEQ-OF       PIC 9(4) OCCURS 200.
       01 WS-SEQ-COUNT       GLOBAL PIC 9(4).
       01 WS-K               GLOBAL PIC 9(4).
      *>   The CRUD (Browse / Create-Update): a grid row, the clicked cell,
      *>   the topic being edited (spaces = a new one) and the confirmation.
       01 WS-ROW             GLOBAL PIC X(1200).
       01 WS-PREVIEW         GLOBAL PIC X(1000).
       01 WS-ROW-NO          GLOBAL PIC S9(4).
       01 WS-COL-NO          GLOBAL PIC S9(4).
       01 WS-EDIT-ID         GLOBAL PIC X(16).
       01 WS-TABS            GLOBAL PIC X(240).
       01 WS-QUESTION        GLOBAL PIC X(300).
       01 WS-ANSWER          GLOBAL PIC X.
       01 WS-SAVE-OK         GLOBAL PIC X.
      *>   The interface texts (spec 071 R44): one row per text, one column
      *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
      *>   only the values are translated.
       01 WS-LANG            GLOBAL PIC XX VALUE "en".
       01 WS-LANG-NOW        GLOBAL PIC XX VALUE "en".
       01 WS-LANG-IX         GLOBAL PIC 9 VALUE 1.
       01 WS-TX-I            GLOBAL PIC 9(4).
       01 PC-TEXT-DATA       GLOBAL.
      *>   NO-TOPICS
          05 FILLER PIC X(110) VALUE "No topics yet - give the first one a name and create it.".
          05 FILLER PIC X(110) VALUE "Ainda não há tópicos - dê um nome ao primeiro e crie-o.".
          05 FILLER PIC X(110) VALUE "Aún no hay temas: ponle nombre al primero y créalo.".
          05 FILLER PIC X(110) VALUE "Aucun sujet pour l'instant : donnez un nom au premier et créez-le.".
          05 FILLER PIC X(110) VALUE "トピックはまだありません。最初のトピックに名前を付けて作成してください。".
          05 FILLER PIC X(110) VALUE "还没有主题——给第一个主题起个名字并创建它。".
      *>   PICK-OR-CREATE
          05 FILLER PIC X(110) VALUE "Pick a topic and press Open, or create a new one.".
          05 FILLER PIC X(110) VALUE "Escolha um tópico e clique em Abrir, ou crie um novo.".
          05 FILLER PIC X(110) VALUE "Elige un tema y pulsa Abrir, o crea uno nuevo.".
          05 FILLER PIC X(110) VALUE "Choisissez un sujet et cliquez sur Ouvrir, ou créez-en un nouveau.".
          05 FILLER PIC X(110) VALUE "トピックを選んで「開く」を押すか、新しいトピックを作成してください。".
          05 FILLER PIC X(110) VALUE "选择一个主题并点击“打开”，或创建一个新主题。".
      *>   PICK-TOPIC-FIRST
          05 FILLER PIC X(110) VALUE "Pick a topic in the list first.".
          05 FILLER PIC X(110) VALUE "Escolha um tópico na lista primeiro.".
          05 FILLER PIC X(110) VALUE "Primero elige un tema de la lista.".
          05 FILLER PIC X(110) VALUE "Choisissez d'abord un sujet dans la liste.".
          05 FILLER PIC X(110) VALUE "先に一覧からトピックを選んでください。".
          05 FILLER PIC X(110) VALUE "请先在列表中选择一个主题。".
      *>   TOPIC-OPENED
          05 FILLER PIC X(110) VALUE "Topic opened.".
          05 FILLER PIC X(110) VALUE "Tópico aberto.".
          05 FILLER PIC X(110) VALUE "Tema abierto.".
          05 FILLER PIC X(110) VALUE "Sujet ouvert.".
          05 FILLER PIC X(110) VALUE "トピックを開きました。".
          05 FILLER PIC X(110) VALUE "主题已打开。".
      *>   NAME-FIRST
          05 FILLER PIC X(110) VALUE "Give the topic a name first.".
          05 FILLER PIC X(110) VALUE "Dê um nome ao tópico primeiro.".
          05 FILLER PIC X(110) VALUE "Primero ponle un nombre al tema.".
          05 FILLER PIC X(110) VALUE "Donnez d'abord un nom au sujet.".
          05 FILLER PIC X(110) VALUE "先にトピックの名前を入力してください。".
          05 FILLER PIC X(110) VALUE "请先为主题命名。".
      *>   SAME-INSTANT
          05 FILLER PIC X(110) VALUE "A topic was created this same instant - try again.".
          05 FILLER PIC X(110) VALUE "Um tópico foi criado neste mesmo instante - tente de novo.".
          05 FILLER PIC X(110) VALUE "Se creó un tema en este mismo instante: inténtalo de nuevo.".
          05 FILLER PIC X(110) VALUE "Un sujet vient d'être créé au même instant : réessayez.".
          05 FILLER PIC X(110) VALUE "ちょうど同じ瞬間に別のトピックが作成されました。もう一度お試しください。".
          05 FILLER PIC X(110) VALUE "同一时刻已创建了一个主题——请重试。".
      *>   TOPIC-CREATED
          05 FILLER PIC X(110) VALUE "Topic created.".
          05 FILLER PIC X(110) VALUE "Tópico criado.".
          05 FILLER PIC X(110) VALUE "Tema creado.".
          05 FILLER PIC X(110) VALUE "Sujet créé.".
          05 FILLER PIC X(110) VALUE "トピックを作成しました。".
          05 FILLER PIC X(110) VALUE "主题已创建。".
      *>   SAMPLES-ALREADY
          05 FILLER PIC X(110) VALUE "The sample topics are already installed.".
          05 FILLER PIC X(110) VALUE "Os tópicos de exemplo já estão instalados.".
          05 FILLER PIC X(110) VALUE "Los temas de ejemplo ya están instalados.".
          05 FILLER PIC X(110) VALUE "Les sujets d'exemple sont déjà installés.".
          05 FILLER PIC X(110) VALUE "サンプルトピックはすでにインストールされています。".
          05 FILLER PIC X(110) VALUE "示例主题已经安装。".
      *>   SAMPLES-NONE
          05 FILLER PIC X(110) VALUE "No sample topics found (&1).".
          05 FILLER PIC X(110) VALUE "Nenhum tópico de exemplo encontrado (&1).".
          05 FILLER PIC X(110) VALUE "No se encontraron temas de ejemplo (&1).".
          05 FILLER PIC X(110) VALUE "Aucun sujet d'exemple trouvé (&1).".
          05 FILLER PIC X(110) VALUE "サンプルトピックが見つかりません（&1）。".
          05 FILLER PIC X(110) VALUE "未找到示例主题（&1）。".
      *>   SAMPLES-REMOVED
          05 FILLER PIC X(110) VALUE "Removed &1 sample topic(s).".
          05 FILLER PIC X(110) VALUE "&1 tópico(s) de exemplo removido(s).".
          05 FILLER PIC X(110) VALUE "Se quitaron &1 tema(s) de ejemplo.".
          05 FILLER PIC X(110) VALUE "&1 sujet(s) d'exemple retiré(s).".
          05 FILLER PIC X(110) VALUE "サンプルトピックを &1 件削除しました。".
          05 FILLER PIC X(110) VALUE "已移除 &1 个示例主题。".
      *>   SAMPLES-INSTALLED
          05 FILLER PIC X(110) VALUE "Sample topics installed: &1 topics, &2 documents.".
          05 FILLER PIC X(110) VALUE "Tópicos de exemplo instalados: &1 tópicos, &2 documentos.".
          05 FILLER PIC X(110) VALUE "Temas de ejemplo instalados: &1 temas, &2 documentos.".
          05 FILLER PIC X(110) VALUE "Sujets d'exemple installés : &1 sujets, &2 documents.".
          05 FILLER PIC X(110) VALUE "サンプルトピックをインストールしました：トピック &1 件、文書 &2 件。".
          05 FILLER PIC X(110) VALUE "示例主题已安装：&1 个主题，&2 个文档。".
      *>   SAMPLES-PROGRESS
          05 FILLER PIC X(110) VALUE "Installing the sample topics: document &1 of &2...".
          05 FILLER PIC X(110) VALUE "Instalando os tópicos de exemplo: documento &1 de &2...".
          05 FILLER PIC X(110) VALUE "Instalando los temas de ejemplo: documento &1 de &2...".
          05 FILLER PIC X(110) VALUE "Installation des sujets d'exemple : document &1 sur &2...".
          05 FILLER PIC X(110) VALUE "サンプルトピックをインストール中：文書 &1 / &2...".
          05 FILLER PIC X(110) VALUE "正在安装示例主题：第 &1 个文档，共 &2 个...".
      *>   TOPICS-TITLE
          05 FILLER PIC X(110) VALUE "Topics".
          05 FILLER PIC X(110) VALUE "Tópicos".
          05 FILLER PIC X(110) VALUE "Temas".
          05 FILLER PIC X(110) VALUE "Sujets".
          05 FILLER PIC X(110) VALUE "トピック".
          05 FILLER PIC X(110) VALUE "主题".
      *>   OPEN-TOPIC
          05 FILLER PIC X(110) VALUE "Open topic".
          05 FILLER PIC X(110) VALUE "Abrir tópico".
          05 FILLER PIC X(110) VALUE "Abrir tema".
          05 FILLER PIC X(110) VALUE "Ouvrir le sujet".
          05 FILLER PIC X(110) VALUE "トピックを開く".
          05 FILLER PIC X(110) VALUE "打开主题".
      *>   NEW-TOPIC
          05 FILLER PIC X(110) VALUE "New topic".
          05 FILLER PIC X(110) VALUE "Novo tópico".
          05 FILLER PIC X(110) VALUE "Nuevo tema".
          05 FILLER PIC X(110) VALUE "Nouveau sujet".
          05 FILLER PIC X(110) VALUE "新しいトピック".
          05 FILLER PIC X(110) VALUE "新主题".
      *>   HINT-TOPIC-NAME
          05 FILLER PIC X(110) VALUE "Name, e.g. Human Resources".
          05 FILLER PIC X(110) VALUE "Nome, por exemplo Recursos Humanos".
          05 FILLER PIC X(110) VALUE "Nombre, por ejemplo Recursos Humanos".
          05 FILLER PIC X(110) VALUE "Nom, par exemple Ressources humaines".
          05 FILLER PIC X(110) VALUE "名前（例：人事）".
          05 FILLER PIC X(110) VALUE "名称，例如：人力资源".
      *>   HINT-TOPIC-PROMPT
          05 FILLER PIC X(110) VALUE "What the assistant is for, how it should answer - the topic's system prompt".
          05 FILLER PIC X(110) VALUE "Para que serve o assistente e como ele deve responder - o prompt de sistema do tópico".
          05 FILLER PIC X(110) VALUE "Para qué sirve el asistente y cómo debe responder: el prompt de sistema del tema".
          05 FILLER PIC X(110) VALUE "À quoi sert l'assistant et comment il doit répondre : le prompt système du sujet".
          05 FILLER PIC X(110) VALUE "アシスタントの役割と回答の仕方 - トピックのシステムプロンプト".
          05 FILLER PIC X(110) VALUE "助手的用途以及回答方式——该主题的系统提示词".
      *>   CREATE-TOPIC
          05 FILLER PIC X(110) VALUE "Create topic".
          05 FILLER PIC X(110) VALUE "Criar tópico".
          05 FILLER PIC X(110) VALUE "Crear tema".
          05 FILLER PIC X(110) VALUE "Créer le sujet".
          05 FILLER PIC X(110) VALUE "トピックを作成".
          05 FILLER PIC X(110) VALUE "创建主题".
      *>   INSTALL-SAMPLES
          05 FILLER PIC X(110) VALUE "Install sample topics".
          05 FILLER PIC X(110) VALUE "Instalar tópicos de exemplo".
          05 FILLER PIC X(110) VALUE "Instalar temas de ejemplo".
          05 FILLER PIC X(110) VALUE "Installer les sujets d'exemple".
          05 FILLER PIC X(110) VALUE "サンプルトピックをインストール".
          05 FILLER PIC X(110) VALUE "安装示例主题".
      *>   REMOVE-SAMPLES
          05 FILLER PIC X(110) VALUE "Remove sample topics".
          05 FILLER PIC X(110) VALUE "Remover tópicos de exemplo".
          05 FILLER PIC X(110) VALUE "Quitar temas de ejemplo".
          05 FILLER PIC X(110) VALUE "Retirer les sujets d'exemple".
          05 FILLER PIC X(110) VALUE "サンプルトピックを削除".
          05 FILLER PIC X(110) VALUE "移除示例主题".
      *>   STATUS
          05 FILLER PIC X(110) VALUE "Status".
          05 FILLER PIC X(110) VALUE "Status".
          05 FILLER PIC X(110) VALUE "Estado".
          05 FILLER PIC X(110) VALUE "État".
          05 FILLER PIC X(110) VALUE "ステータス".
          05 FILLER PIC X(110) VALUE "状态".
      *>   TAB-BROWSE
          05 FILLER PIC X(110) VALUE "Browse".
          05 FILLER PIC X(110) VALUE "Consultar".
          05 FILLER PIC X(110) VALUE "Consultar".
          05 FILLER PIC X(110) VALUE "Parcourir".
          05 FILLER PIC X(110) VALUE "一覧".
          05 FILLER PIC X(110) VALUE "浏览".
      *>   TAB-EDIT
          05 FILLER PIC X(110) VALUE "Create/Update".
          05 FILLER PIC X(110) VALUE "Criar/Atualizar".
          05 FILLER PIC X(110) VALUE "Crear/Actualizar".
          05 FILLER PIC X(110) VALUE "Créer/Modifier".
          05 FILLER PIC X(110) VALUE "作成/更新".
          05 FILLER PIC X(110) VALUE "创建/更新".
      *>   NEW
          05 FILLER PIC X(110) VALUE "New".
          05 FILLER PIC X(110) VALUE "Novo".
          05 FILLER PIC X(110) VALUE "Nuevo".
          05 FILLER PIC X(110) VALUE "Nouveau".
          05 FILLER PIC X(110) VALUE "新規".
          05 FILLER PIC X(110) VALUE "新建".
      *>   SAVE
          05 FILLER PIC X(110) VALUE "Save".
          05 FILLER PIC X(110) VALUE "Salvar".
          05 FILLER PIC X(110) VALUE "Guardar".
          05 FILLER PIC X(110) VALUE "Enregistrer".
          05 FILLER PIC X(110) VALUE "保存".
          05 FILLER PIC X(110) VALUE "保存".
      *>   CANCEL
          05 FILLER PIC X(110) VALUE "Cancel".
          05 FILLER PIC X(110) VALUE "Cancelar".
          05 FILLER PIC X(110) VALUE "Cancelar".
          05 FILLER PIC X(110) VALUE "Annuler".
          05 FILLER PIC X(110) VALUE "キャンセル".
          05 FILLER PIC X(110) VALUE "取消".
      *>   EDIT-TOPIC
          05 FILLER PIC X(110) VALUE "Edit topic".
          05 FILLER PIC X(110) VALUE "Editar tópico".
          05 FILLER PIC X(110) VALUE "Editar tema".
          05 FILLER PIC X(110) VALUE "Modifier le sujet".
          05 FILLER PIC X(110) VALUE "トピックを編集".
          05 FILLER PIC X(110) VALUE "编辑主题".
      *>   CONFIRM-DELETE
          05 FILLER PIC X(110) VALUE "Delete the topic ""&1"" and its Knowledge Base?".
          05 FILLER PIC X(110) VALUE "Excluir o tópico ""&1"" e sua base de conhecimento?".
          05 FILLER PIC X(110) VALUE "¿Eliminar el tema ""&1"" y su base de conocimiento?".
          05 FILLER PIC X(110) VALUE "Supprimer le sujet « &1 » et sa base de connaissances ?".
          05 FILLER PIC X(110) VALUE "トピック「&1」とそのナレッジベースを削除しますか？".
          05 FILLER PIC X(110) VALUE "删除主题“&1”及其知识库？".
      *>   TOPIC-SAVED
          05 FILLER PIC X(110) VALUE "Topic saved.".
          05 FILLER PIC X(110) VALUE "Tópico salvo.".
          05 FILLER PIC X(110) VALUE "Tema guardado.".
          05 FILLER PIC X(110) VALUE "Sujet enregistré.".
          05 FILLER PIC X(110) VALUE "トピックを保存しました。".
          05 FILLER PIC X(110) VALUE "主题已保存。".
      *>   TOPIC-DELETED
          05 FILLER PIC X(110) VALUE "Topic deleted.".
          05 FILLER PIC X(110) VALUE "Tópico excluído.".
          05 FILLER PIC X(110) VALUE "Tema eliminado.".
          05 FILLER PIC X(110) VALUE "Sujet supprimé.".
          05 FILLER PIC X(110) VALUE "トピックを削除しました。".
          05 FILLER PIC X(110) VALUE "主题已删除。".
      *>   SAVE-FAILED
          05 FILLER PIC X(110) VALUE "The topic could not be saved (status &1).".
          05 FILLER PIC X(110) VALUE "Não foi possível salvar o tópico (status &1).".
          05 FILLER PIC X(110) VALUE "No se pudo guardar el tema (estado &1).".
          05 FILLER PIC X(110) VALUE "Impossible d'enregistrer le sujet (état &1).".
          05 FILLER PIC X(110) VALUE "トピックを保存できませんでした（ステータス &1）。".
          05 FILLER PIC X(110) VALUE "无法保存主题（状态 &1）。".
      *>   YES
          05 FILLER PIC X(110) VALUE "Yes".
          05 FILLER PIC X(110) VALUE "Sim".
          05 FILLER PIC X(110) VALUE "Sí".
          05 FILLER PIC X(110) VALUE "Oui".
          05 FILLER PIC X(110) VALUE "はい".
          05 FILLER PIC X(110) VALUE "是".
      *>   NO
          05 FILLER PIC X(110) VALUE "No".
          05 FILLER PIC X(110) VALUE "Não".
          05 FILLER PIC X(110) VALUE "No".
          05 FILLER PIC X(110) VALUE "Non".
          05 FILLER PIC X(110) VALUE "いいえ".
          05 FILLER PIC X(110) VALUE "否".
      *>   COL-NAME
          05 FILLER PIC X(110) VALUE "Name".
          05 FILLER PIC X(110) VALUE "Nome".
          05 FILLER PIC X(110) VALUE "Nombre".
          05 FILLER PIC X(110) VALUE "Nom".
          05 FILLER PIC X(110) VALUE "名前".
          05 FILLER PIC X(110) VALUE "名称".
      *>   COL-PROMPT
          05 FILLER PIC X(110) VALUE "System prompt".
          05 FILLER PIC X(110) VALUE "Prompt de sistema".
          05 FILLER PIC X(110) VALUE "Prompt de sistema".
          05 FILLER PIC X(110) VALUE "Prompt système".
          05 FILLER PIC X(110) VALUE "システムプロンプト".
          05 FILLER PIC X(110) VALUE "系统提示词".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
          05 PC-TEXT-ROW     OCCURS 35.
             10 PC-TEXT      PIC X(110) OCCURS 6.
      *>   The texts in the current language, by name.
       01 PC-TEXTS-NOW       GLOBAL.
          05 T-NO-TOPICS PIC X(110).
          05 T-PICK-OR-CREATE PIC X(110).
          05 T-PICK-TOPIC-FIRST PIC X(110).
          05 T-TOPIC-OPENED PIC X(110).
          05 T-NAME-FIRST PIC X(110).
          05 T-SAME-INSTANT PIC X(110).
          05 T-TOPIC-CREATED PIC X(110).
          05 T-SAMPLES-ALREADY PIC X(110).
          05 T-SAMPLES-NONE PIC X(110).
          05 T-SAMPLES-REMOVED PIC X(110).
          05 T-SAMPLES-INSTALLED PIC X(110).
          05 T-SAMPLES-PROGRESS PIC X(110).
          05 T-TOPICS-TITLE PIC X(110).
          05 T-OPEN-TOPIC PIC X(110).
          05 T-NEW-TOPIC PIC X(110).
          05 T-HINT-TOPIC-NAME PIC X(110).
          05 T-HINT-TOPIC-PROMPT PIC X(110).
          05 T-CREATE-TOPIC PIC X(110).
          05 T-INSTALL-SAMPLES PIC X(110).
          05 T-REMOVE-SAMPLES PIC X(110).
          05 T-STATUS PIC X(110).
          05 T-TAB-BROWSE PIC X(110).
          05 T-TAB-EDIT PIC X(110).
          05 T-NEW PIC X(110).
          05 T-SAVE PIC X(110).
          05 T-CANCEL PIC X(110).
          05 T-EDIT-TOPIC PIC X(110).
          05 T-CONFIRM-DELETE PIC X(110).
          05 T-TOPIC-SAVED PIC X(110).
          05 T-TOPIC-DELETED PIC X(110).
          05 T-SAVE-FAILED PIC X(110).
          05 T-YES PIC X(110).
          05 T-NO PIC X(110).
          05 T-COL-NAME PIC X(110).
          05 T-COL-PROMPT PIC X(110).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
          05 PC-TEXT-NOW     PIC X(110) OCCURS 35.
      *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.
       01 WS-FMT             GLOBAL PIC X(110).
       01 WS-ARG1            GLOBAL PIC X(300).
       01 WS-ARG2            GLOBAL PIC X(300).
       01 WS-ARG3            GLOBAL PIC X(300).
       01 WS-ARG4            GLOBAL PIC X(300).
       01 WS-FMT-OUT         GLOBAL PIC X(1200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Topics'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-New.
          05 WS-Lbl-New-TEXT       PIC X(256) VALUE 'New topic'.
          05 WS-Lbl-New-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-New-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Name.
          05 WS-Txt-Name-TEXT       PIC X(40) VALUE SPACES.
          05 WS-Txt-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Name-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Name-VALUE      PIC X(40) VALUE SPACES.

       01 WS-Txt-Prompt.
          05 WS-Txt-Prompt-TEXT       PIC X(1000) VALUE SPACES.
          05 WS-Txt-Prompt-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Prompt-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Prompt-VALUE      PIC X(1000) VALUE SPACES.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Install.
          05 WS-Btn-Install-TEXT       PIC X(256) VALUE 'Install sample topics'.
          05 WS-Btn-Install-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Install-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-RemoveSamples.
          05 WS-Btn-RemoveSamples-TEXT       PIC X(256) VALUE 'Remove sample topics'.
          05 WS-Btn-RemoveSamples-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-RemoveSamples-ENABLED    PIC 9      VALUE 1.

       01 WS-KB-T.
          05 WS-KB-T-TEXT       PIC X(256) VALUE 'KB-T'.
          05 WS-KB-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-KB-T-ENABLED    PIC 9      VALUE 1.

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

       01 WS-Dg-List.
          05 WS-Dg-List-TEXT       PIC X(256) VALUE 'Dg-List'.
          05 WS-Dg-List-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dg-List-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save2.
          05 WS-Btn-Save2-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save2-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel2.
          05 WS-Btn-Cancel2-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel2-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "TOPICS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "TOPICS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "TOPICS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onActivate"
                               CALL "TOPICS-FORM--ONACTIVATE"
                       END-EVALUATE
                   WHEN "Btn-Install"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-INSTALL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-RemoveSamples"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-REMOVESAMPLES--ONCLICK"
                       END-EVALUATE
                   WHEN "KB-T"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onIndexed"
                               CALL "KB-T--ONINDEXED"
                           WHEN "onError"
                               CALL "KB-T--ONERROR"
                           WHEN "onBusy"
                               CALL "KB-T--ONBUSY"
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
                   WHEN "Dg-List"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCellClick"
                               CALL "DG-LIST--ONCELLCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE2--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL2--ONCLICK"
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
       PROGRAM-ID. TOPICS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM TOPICS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TOPICS-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back on the pane: the topic or the language may have changed.
           CALL "PC-OPEN"

           GOBACK.

       END PROGRAM TOPICS-FORM--ONACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TOPICS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TOPICS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-INSTALL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Every topic in samples/samples.txt: its topic, its Knowledge Base
      *>   with the listed documents, and its data files registered by path.
      *>   Nothing about any sample is written in this code (R10).
           CALL "PC-HAS-SAMPLES"
           IF WS-OK = "Y"
               MOVE FUNCTION TRIM(T-SAMPLES-ALREADY) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           OPEN INPUT SAMPLES-FILE
           IF WS-FS NOT = "00"
               MOVE T-SAMPLES-NONE TO WS-FMT
               MOVE WS-SAMPLES-LIST TO WS-ARG1
               CALL "PC-FMT"
               MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           MOVE 0 TO WS-MAP-COUNT WS-Q-COUNT WS-SEQ-N WS-TF-SEQ
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           MOVE "N" TO WS-EOF
           PERFORM UNTIL WS-EOF = "Y"
               READ SAMPLES-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       MOVE SPACES TO WS-KIND WS-FOLDER WS-F3 WS-F4
                       UNSTRING SAMPLE-LINE DELIMITED BY "|"
                           INTO WS-KIND WS-FOLDER WS-F3 WS-F4
                       END-UNSTRING
                       EVALUATE WS-KIND
                           WHEN "TOPIC"
                               CALL "PC-ADD-SAMPLE-TOPIC"
                           WHEN "DOC"
                               CALL "PC-TOPIC-OF"
                               IF WS-TOPIC-ID-OF NOT = SPACES AND WS-Q-COUNT < 60
                                   ADD 1 TO WS-Q-COUNT
                                   MOVE WS-TOPIC-ID-OF TO WS-Q-TOPIC(WS-Q-COUNT)
                                   MOVE SPACES TO WS-Q-PATH(WS-Q-COUNT)
                                   STRING FUNCTION TRIM(WS-SAMPLES-DIR) "/"
                                          FUNCTION TRIM(WS-FOLDER) "/documents/"
                                          FUNCTION TRIM(WS-F3)
                                       DELIMITED BY SIZE INTO WS-Q-PATH(WS-Q-COUNT)
                               END-IF
                           WHEN "FILE"
                               CALL "PC-ADD-SAMPLE-FILE"
                           WHEN OTHER
                               CONTINUE
                       END-EVALUATE
               END-READ
           END-PERFORM
           CLOSE SAMPLES-FILE
           CALL "PC-LOAD-TOPICS"
           MOVE "Y" TO WS-INSTALLING
           MOVE 1 TO WS-Q-NEXT
           CALL "PC-NEXT-IMPORT"

           GOBACK.

       END PROGRAM BTN-INSTALL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-REMOVESAMPLES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Every sample topic, with its Knowledge Base (moved aside, never
      *>   deleted), its registered files and its prompt versions. The files
      *>   in samples/ stay where they are.
           MOVE 0 TO WS-REMOVED WS-MAP-COUNT
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE LOW-VALUES TO TOP-ID
           MOVE "N" TO WS-EOF
           START TOPICS-FILE KEY IS >= TOP-ID
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ TOPICS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF TOP-SAMPLE = "Y" AND WS-MAP-COUNT < 10
                           ADD 1 TO WS-MAP-COUNT
                           MOVE TOP-ID TO WS-MAP-ID(WS-MAP-COUNT)
                       END-IF
               END-READ
           END-PERFORM
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-MAP-COUNT
               MOVE WS-MAP-ID(WS-J) TO TOP-ID
               DELETE TOPICS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY ADD 1 TO WS-REMOVED
               END-DELETE
           END-PERFORM
           COMMIT
           CLOSE TOPICS-FILE
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-MAP-COUNT
               MOVE KB-T::RemoveCollection(WS-MAP-ID(WS-J)) TO WS-OK
               MOVE WS-MAP-ID(WS-J) TO WS-TOPIC-ID-OF
               CALL "PC-DROP-TOPIC-ROWS"
           END-PERFORM
           MOVE "CUR-TOPIC" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-MAP-COUNT
               IF WS-SET-VALUE(1:16) = WS-MAP-ID(WS-J)
                   MOVE SPACES TO WS-SET-VALUE
                   CALL "PC-SETTING-PUT"
               END-IF
           END-PERFORM
           MOVE WS-REMOVED TO WS-N1
           MOVE T-SAMPLES-REMOVED TO WS-FMT
           MOVE WS-N1 TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
           CALL "PC-LOAD-TOPICS"

           GOBACK.

       END PROGRAM BTN-REMOVESAMPLES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-T--ONINDEXED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           IF WS-INSTALLING = "Y"
               CALL "PC-NEXT-IMPORT"
           END-IF

           GOBACK.

       END PROGRAM KB-T--ONINDEXED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-T--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE KB-T::LastError TO Lbl-Status::Caption
           IF WS-INSTALLING = "Y"
               CALL "PC-NEXT-IMPORT"
           END-IF

           GOBACK.

       END PROGRAM KB-T--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-T--ONBUSY IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE KB-T::LastError TO Lbl-Status::Caption
           IF WS-INSTALLING = "Y"
               CALL "PC-NEXT-IMPORT"
           END-IF

           GOBACK.

       END PROGRAM KB-T--ONBUSY.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-LANG--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   A flag in the chat's menu changes the language while this form is
      *>   on the pane; nothing tells a pane occupant, so it looks (R46).
           CALL "PC-LANG-NOW"
           IF WS-LANG-NOW NOT = WS-LANG
               CALL "TOPICS-FORM--ONACTIVATE"
           END-IF

           GOBACK.

       END PROGRAM TMR-LANG--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-NEW-TOPIC"

           GOBACK.

       END PROGRAM BTN-NEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DG-LIST--ONCELLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Grid row n is WS-TOPIC-ID(n); columns 3, 4 and 5 are the open,
      *>   edit and delete buttons.
           MOVE Dg-List::ClickedRow TO WS-ROW-NO
           MOVE Dg-List::ClickedColumn TO WS-COL-NO
           IF WS-ROW-NO < 1 OR WS-ROW-NO > WS-TOPIC-COUNT
               EXIT PROGRAM
           END-IF
           MOVE WS-TOPIC-ID(WS-ROW-NO) TO WS-TOPIC-ID-OF
           EVALUATE WS-COL-NO
               WHEN 3 CALL "PC-OPEN-TOPIC"
               WHEN 4 CALL "PC-EDIT-TOPIC"
               WHEN 5 CALL "PC-DELETE-TOPIC"
               WHEN OTHER CONTINUE
           END-EVALUATE

           GOBACK.

       END PROGRAM DG-LIST--ONCELLCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE-TOPIC"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-CANCEL-EDIT"

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE-TOPIC"

           GOBACK.

       END PROGRAM BTN-SAVE2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-CANCEL-EDIT"

           GOBACK.

       END PROGRAM BTN-CANCEL2--ONCLICK.

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
           MOVE SPACES TO WS-TFILES-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/topic-files.idx"
               DELIMITED BY SIZE INTO WS-TFILES-PATH
           MOVE SPACES TO WS-PROMPTS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/prompt-versions.idx"
               DELIMITED BY SIZE INTO WS-PROMPTS-PATH

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
       PROGRAM-ID. PC-LOAD-TOPICS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Every topic, in the order they were created (the key is a timestamp),
      *>   one grid row each: name, prompt, then the open / edit / delete
      *>   buttons. Grid row n is WS-TOPIC-ID(n).
           MOVE Dg-List::ClearRows() TO WS-OK
           MOVE 0 TO WS-TOPIC-COUNT
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE LOW-VALUES TO TOP-ID
           MOVE "N" TO WS-EOF
           START TOPICS-FILE KEY IS >= TOP-ID
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ TOPICS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF WS-TOPIC-COUNT < 500
                           ADD 1 TO WS-TOPIC-COUNT
                           MOVE TOP-ID TO WS-TOPIC-ID(WS-TOPIC-COUNT)
                           MOVE TOP-PROMPT TO WS-PREVIEW
                           INSPECT WS-PREVIEW REPLACING ALL X"0A" BY SPACE
                               ALL X"0D" BY SPACE ALL X"09" BY SPACE
                           MOVE SPACES TO WS-ROW
                           STRING FUNCTION TRIM(TOP-NAME) X"09"
                                  FUNCTION TRIM(WS-PREVIEW) X"09"
                                  "icon:chat" X"09" "icon:pencil" X"09"
                                  "icon:trash"
                               DELIMITED BY SIZE INTO WS-ROW
                           MOVE Dg-List::AddRow(WS-ROW) TO WS-OK
                       END-IF
               END-READ
           END-PERFORM
           CLOSE TOPICS-FILE
      *>   The chat's menu turns Chat and New conversation on only while a
      *>   topic exists: tell it the list may have changed (the first topic
      *>   saved, the last one deleted), or it waits until it is shown again.
           INVOKE super::"PC-REFRESH"()

           GOBACK.

       END PROGRAM PC-LOAD-TOPICS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEXT-IMPORT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   One sample document at a time: a Knowledge Base import runs in the
      *>   background and reports through onIndexed, which calls back here.
           IF WS-Q-NEXT > WS-Q-COUNT
               MOVE "N" TO WS-INSTALLING
               MOVE WS-MAP-COUNT TO WS-N1
               MOVE WS-Q-COUNT TO WS-N2
               MOVE T-SAMPLES-INSTALLED TO WS-FMT
               MOVE WS-N1 TO WS-ARG1
               MOVE WS-N2 TO WS-ARG2
               CALL "PC-FMT"
               MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
               CALL "PC-LOAD-TOPICS"
               EXIT PROGRAM
           END-IF
           MOVE WS-Q-TOPIC(WS-Q-NEXT) TO KB-T::Collection
           MOVE WS-Q-NEXT TO WS-N1
           MOVE WS-Q-COUNT TO WS-N2
           MOVE T-SAMPLES-PROGRESS TO WS-FMT
           MOVE WS-N1 TO WS-ARG1
           MOVE WS-N2 TO WS-ARG2
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Status::Caption
           MOVE KB-T::ImportDocument(FUNCTION TRIM(WS-Q-PATH(WS-Q-NEXT))) TO WS-OK
           ADD 1 TO WS-Q-NEXT

           GOBACK.

       END PROGRAM PC-NEXT-IMPORT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TOPIC-OF IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-FOLDER in: the id the installer gave that sample folder's topic.
           MOVE SPACES TO WS-TOPIC-ID-OF
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-MAP-COUNT
               IF WS-MAP-FOLDER(WS-J) = WS-FOLDER
                   MOVE WS-MAP-ID(WS-J) TO WS-TOPIC-ID-OF
               END-IF
           END-PERFORM

           GOBACK.

       END PROGRAM PC-TOPIC-OF.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-HAS-SAMPLES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-OK out: "Y" when sample topics are installed.
           MOVE "N" TO WS-OK
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE LOW-VALUES TO TOP-ID
           MOVE "N" TO WS-EOF
           START TOPICS-FILE KEY IS >= TOP-ID
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ TOPICS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF TOP-SAMPLE = "Y"
                           MOVE "Y" TO WS-OK
                       END-IF
               END-READ
           END-PERFORM
           CLOSE TOPICS-FILE

           GOBACK.

       END PROGRAM PC-HAS-SAMPLES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DROP-TOPIC-ROWS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-TOPIC-ID-OF in: its registered files and prompt versions go.
           MOVE 0 TO WS-SEQ-COUNT
           OPEN I-O TFILES-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TFILES-FILE
               CLOSE TFILES-FILE
               OPEN I-O TFILES-FILE
           END-IF
           MOVE WS-TOPIC-ID-OF TO TF-TOPIC
           MOVE 0 TO TF-SEQ
           MOVE "N" TO WS-EOF
           START TFILES-FILE KEY IS >= TF-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ TFILES-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF TF-TOPIC NOT = WS-TOPIC-ID-OF
                           MOVE "Y" TO WS-EOF
                       ELSE
                           IF WS-SEQ-COUNT < 200
                               ADD 1 TO WS-SEQ-COUNT
                               MOVE TF-SEQ TO WS-SEQ-OF(WS-SEQ-COUNT)
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-SEQ-COUNT
               MOVE WS-TOPIC-ID-OF TO TF-TOPIC
               MOVE WS-SEQ-OF(WS-K) TO TF-SEQ
               DELETE TFILES-FILE
                   INVALID KEY CONTINUE
               END-DELETE
           END-PERFORM
           COMMIT
           CLOSE TFILES-FILE
           MOVE 0 TO WS-SEQ-COUNT
           OPEN I-O PROMPTS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT PROMPTS-FILE
               CLOSE PROMPTS-FILE
               OPEN I-O PROMPTS-FILE
           END-IF
           MOVE WS-TOPIC-ID-OF TO PRM-TOPIC
           MOVE 0 TO PRM-VERSION
           MOVE "N" TO WS-EOF
           START PROMPTS-FILE KEY IS >= PRM-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ PROMPTS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF PRM-TOPIC NOT = WS-TOPIC-ID-OF
                           MOVE "Y" TO WS-EOF
                       ELSE
                           IF WS-SEQ-COUNT < 200
                               ADD 1 TO WS-SEQ-COUNT
                               MOVE PRM-VERSION TO WS-SEQ-OF(WS-SEQ-COUNT)
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-SEQ-COUNT
               MOVE WS-TOPIC-ID-OF TO PRM-TOPIC
               MOVE WS-SEQ-OF(WS-K) TO PRM-VERSION
               DELETE PROMPTS-FILE
                   INVALID KEY CONTINUE
               END-DELETE
           END-PERFORM
           COMMIT
           CLOSE PROMPTS-FILE

           GOBACK.

       END PROGRAM PC-DROP-TOPIC-ROWS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ADD-SAMPLE-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   A TOPIC line: the topic, marked as a sample, and its collection.
           IF WS-MAP-COUNT >= 10
               EXIT PROGRAM
           END-IF
           ADD 1 TO WS-SEQ-N WS-MAP-COUNT
           MOVE SPACES TO WS-NEW-ID
           STRING WS-NOW(1:15) WS-SEQ-N DELIMITED BY SIZE INTO WS-NEW-ID
           MOVE WS-FOLDER TO WS-MAP-FOLDER(WS-MAP-COUNT)
           MOVE WS-NEW-ID TO WS-MAP-ID(WS-MAP-COUNT)
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE WS-NEW-ID TO TOP-ID
           MOVE WS-F3 TO TOP-NAME
           MOVE WS-F4 TO TOP-PROMPT
           MOVE WS-NOW(1:14) TO TOP-CREATED
           MOVE "Y" TO TOP-SAMPLE
           WRITE TOPIC-REC
               INVALID KEY CONTINUE
           END-WRITE
           COMMIT
           CLOSE TOPICS-FILE
           MOVE KB-T::CreateCollection(WS-NEW-ID) TO WS-OK

           GOBACK.

       END PROGRAM PC-ADD-SAMPLE-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ADD-SAMPLE-FILE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   A FILE line: the data file and its .cidx, registered by path for the
      *>   topic. They stay in samples/ and are only ever read.
           CALL "PC-TOPIC-OF"
           IF WS-TOPIC-ID-OF = SPACES
               EXIT PROGRAM
           END-IF
           ADD 1 TO WS-TF-SEQ
           OPEN I-O TFILES-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TFILES-FILE
               CLOSE TFILES-FILE
               OPEN I-O TFILES-FILE
           END-IF
           MOVE WS-TOPIC-ID-OF TO TF-TOPIC
           MOVE WS-TF-SEQ TO TF-SEQ
           MOVE WS-F3 TO TF-NAME
           MOVE SPACES TO TF-DATA TF-CIDX
           STRING FUNCTION TRIM(WS-SAMPLES-DIR) "/"
                  FUNCTION TRIM(WS-FOLDER) "/" FUNCTION TRIM(WS-F3)
               DELIMITED BY SIZE INTO TF-DATA
           STRING FUNCTION TRIM(WS-SAMPLES-DIR) "/"
                  FUNCTION TRIM(WS-FOLDER) "/" FUNCTION TRIM(WS-F4)
               DELIMITED BY SIZE INTO TF-CIDX
           WRITE TFILE-REC
               INVALID KEY REWRITE TFILE-REC
           END-WRITE
           COMMIT
           CLOSE TFILES-FILE

           GOBACK.

       END PROGRAM PC-ADD-SAMPLE-FILE.

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
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 35
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-TOPICS-TITLE) TO Lbl-Title::Caption
           MOVE SPACES TO WS-TABS
           STRING FUNCTION TRIM(T-TAB-BROWSE) X"0A" FUNCTION TRIM(T-TAB-EDIT)
               DELIMITED BY SIZE INTO WS-TABS
           MOVE WS-TABS TO Tab-Crud::Tabs
           INVOKE Dg-List::SetColumnTitle("name", T-COL-NAME)
           INVOKE Dg-List::SetColumnTitle("prompt", T-COL-PROMPT)
           MOVE FUNCTION TRIM(T-NEW) TO Btn-New::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save2::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel2::Caption
           IF WS-EDIT-ID = SPACES
               MOVE FUNCTION TRIM(T-NEW-TOPIC) TO Lbl-New::Caption
           ELSE
               MOVE FUNCTION TRIM(T-EDIT-TOPIC) TO Lbl-New::Caption
           END-IF
           MOVE FUNCTION TRIM(T-HINT-TOPIC-NAME) TO Txt-Name::HintText
           MOVE FUNCTION TRIM(T-HINT-TOPIC-PROMPT) TO Txt-Prompt::HintText
           MOVE FUNCTION TRIM(T-INSTALL-SAMPLES) TO Btn-Install::Caption
           MOVE FUNCTION TRIM(T-REMOVE-SAMPLES) TO Btn-RemoveSamples::Caption
           MOVE FUNCTION TRIM(T-STATUS) TO Lbl-Status::Caption

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
       01 WS-PART            PIC X(110).
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
           CALL "PC-TEXTS"
           DISPLAY "POWERCHAT_SAMPLES" UPON ENVIRONMENT-NAME
           ACCEPT WS-SAMPLES-DIR FROM ENVIRONMENT-VALUE
           IF WS-SAMPLES-DIR = SPACES
               MOVE "samples" TO WS-SAMPLES-DIR
           END-IF
           MOVE SPACES TO WS-SAMPLES-LIST
           STRING FUNCTION TRIM(WS-SAMPLES-DIR) "/samples.txt"
               DELIMITED BY SIZE INTO WS-SAMPLES-LIST
           MOVE "KB-LOCATION" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           IF WS-SET-VALUE NOT = SPACES
               MOVE WS-SET-VALUE TO KB-T::Location
           END-IF
           CALL "PC-LOAD-TOPICS"
           IF WS-TOPIC-COUNT = 0
               MOVE FUNCTION TRIM(T-NO-TOPICS) TO Lbl-Status::Caption
           ELSE
               MOVE FUNCTION TRIM(T-PICK-OR-CREATE) TO Lbl-Status::Caption
           END-IF

           GOBACK.

       END PROGRAM PC-OPEN.

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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-OPEN-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-TOPIC-ID-OF in: that topic becomes the current one (CUR-TOPIC).
           MOVE "CUR-TOPIC" TO WS-SET-NAME
           MOVE WS-TOPIC-ID-OF TO WS-SET-VALUE
           CALL "PC-SETTING-PUT"
           MOVE FUNCTION TRIM(T-TOPIC-OPENED) TO Lbl-Status::Caption

           GOBACK.

       END PROGRAM PC-OPEN-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEW-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The Create/Update page, empty: Save writes a new topic.
           MOVE SPACES TO WS-EDIT-ID
           MOVE SPACES TO Txt-Name::Text
           MOVE SPACES TO Txt-Prompt::Text
           MOVE FUNCTION TRIM(T-NEW-TOPIC) TO Lbl-New::Caption
           MOVE 1 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-NEW-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EDIT-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-TOPIC-ID-OF in: its name and prompt on the Create/Update page.
      *>   The key is the topic's id, so the name stays editable (a rename).
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE "N" TO WS-SAVE-OK
           MOVE WS-TOPIC-ID-OF TO TOP-ID
           READ TOPICS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE "Y" TO WS-SAVE-OK
           END-READ
           CLOSE TOPICS-FILE
           IF WS-SAVE-OK NOT = "Y"
               MOVE FUNCTION TRIM(T-PICK-TOPIC-FIRST) TO Lbl-Status::Caption
               CALL "PC-LOAD-TOPICS"
               EXIT PROGRAM
           END-IF
           MOVE WS-TOPIC-ID-OF TO WS-EDIT-ID
           MOVE FUNCTION TRIM(TOP-NAME) TO Txt-Name::Text
           MOVE FUNCTION TRIM(TOP-PROMPT) TO Txt-Prompt::Text
           MOVE FUNCTION TRIM(T-EDIT-TOPIC) TO Lbl-New::Caption
           MOVE 1 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-EDIT-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   A topic is data: its own id, name, prompt, documents and Knowledge
      *>   Base collection. Nothing about any topic is written in this code.
      *>   WS-EDIT-ID spaces = a new topic (a fresh id and its collection);
      *>   otherwise that topic, keeping its creation stamp and sample mark.
           MOVE Txt-Name::Text TO WS-NEW-NAME
           MOVE Txt-Prompt::Text TO WS-NEW-PROMPT
           IF WS-NEW-NAME = SPACES
               MOVE FUNCTION TRIM(T-NAME-FIRST) TO Lbl-Status::Caption
               EXIT PROGRAM
           END-IF
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           IF WS-EDIT-ID = SPACES
               MOVE WS-NOW(1:16) TO WS-NEW-ID
               MOVE WS-NEW-ID TO TOP-ID
               MOVE WS-NOW(1:14) TO TOP-CREATED
               MOVE "N" TO TOP-SAMPLE
           ELSE
               MOVE WS-EDIT-ID TO WS-NEW-ID
               MOVE WS-EDIT-ID TO TOP-ID
               READ TOPICS-FILE
                   INVALID KEY
                       MOVE WS-EDIT-ID TO TOP-ID
                       MOVE WS-NOW(1:14) TO TOP-CREATED
                       MOVE "N" TO TOP-SAMPLE
               END-READ
           END-IF
           MOVE WS-NEW-NAME TO TOP-NAME
           MOVE WS-NEW-PROMPT TO TOP-PROMPT
           MOVE "Y" TO WS-SAVE-OK
           WRITE TOPIC-REC
               INVALID KEY
                   IF WS-EDIT-ID = SPACES
      *>               A fresh id already taken: another topic, this instant.
                       MOVE FUNCTION TRIM(T-SAME-INSTANT) TO Lbl-Status::Caption
                       MOVE "N" TO WS-SAVE-OK
                   ELSE
                       REWRITE TOPIC-REC
                           INVALID KEY
                               MOVE T-SAVE-FAILED TO WS-FMT
                               MOVE WS-FS TO WS-ARG1
                               CALL "PC-FMT"
                               MOVE FUNCTION TRIM(WS-FMT-OUT)
                                   TO Lbl-Status::Caption
                               MOVE "N" TO WS-SAVE-OK
                       END-REWRITE
                   END-IF
           END-WRITE
           IF WS-SAVE-OK NOT = "Y"
               CLOSE TOPICS-FILE
               EXIT PROGRAM
           END-IF
           COMMIT
           CLOSE TOPICS-FILE
           IF WS-EDIT-ID = SPACES
      *>       Its Knowledge Base collection: an empty documents folder and index.
               MOVE KB-T::CreateCollection(WS-NEW-ID) TO WS-OK
               IF WS-OK NOT = "1"
                   MOVE KB-T::LastError TO Lbl-Status::Caption
               ELSE
                   MOVE FUNCTION TRIM(T-TOPIC-CREATED) TO Lbl-Status::Caption
               END-IF
           ELSE
               MOVE FUNCTION TRIM(T-TOPIC-SAVED) TO Lbl-Status::Caption
           END-IF
           MOVE SPACES TO WS-EDIT-ID
           MOVE SPACES TO Txt-Name::Text
           MOVE SPACES TO Txt-Prompt::Text
           CALL "PC-LOAD-TOPICS"
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-SAVE-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-CANCEL-EDIT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Back to Browse, nothing written and the list not reloaded.
           MOVE SPACES TO WS-EDIT-ID
           MOVE 0 TO Tab-Crud::SelectedTab

           GOBACK.

       END PROGRAM PC-CANCEL-EDIT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DELETE-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-TOPIC-ID-OF in, after a confirmation: the topic goes the way the
      *>   sample topics go - its Knowledge Base (moved aside, never deleted),
      *>   its registered files and its prompt versions, and CUR-TOPIC when
      *>   it was the current one.
           OPEN I-O TOPICS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT TOPICS-FILE
               CLOSE TOPICS-FILE
               OPEN I-O TOPICS-FILE
           END-IF
           MOVE "N" TO WS-SAVE-OK
           MOVE WS-TOPIC-ID-OF TO TOP-ID
           READ TOPICS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE "Y" TO WS-SAVE-OK
           END-READ
           CLOSE TOPICS-FILE
           IF WS-SAVE-OK NOT = "Y"
               MOVE FUNCTION TRIM(T-PICK-TOPIC-FIRST) TO Lbl-Status::Caption
               CALL "PC-LOAD-TOPICS"
               EXIT PROGRAM
           END-IF
           MOVE T-CONFIRM-DELETE TO WS-FMT
           MOVE TOP-NAME TO WS-ARG1
           CALL "PC-FMT"
           MOVE WS-FMT-OUT TO WS-QUESTION
           INVOKE ME::"SetProperty"("ConfirmText", WS-QUESTION)
           INVOKE ME::"SetProperty"("ConfirmYes", T-YES)
           INVOKE ME::"SetProperty"("ConfirmNo", T-NO)
           INVOKE ME::"SetProperty"("ConfirmAnswer", "N")
           INVOKE ME::"OpenFormSync"("CONFIRM-FORM")
           INVOKE ME::"GetProperty"("ConfirmAnswer") RETURNING WS-ANSWER
           IF WS-ANSWER NOT = "Y"
               EXIT PROGRAM
           END-IF
           OPEN I-O TOPICS-FILE
           MOVE WS-TOPIC-ID-OF TO TOP-ID
           DELETE TOPICS-FILE
               INVALID KEY CONTINUE
           END-DELETE
           COMMIT
           CLOSE TOPICS-FILE
           MOVE KB-T::RemoveCollection(WS-TOPIC-ID-OF) TO WS-OK
           CALL "PC-DROP-TOPIC-ROWS"
           MOVE "CUR-TOPIC" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           IF WS-SET-VALUE(1:16) = WS-TOPIC-ID-OF
               MOVE SPACES TO WS-SET-VALUE
               CALL "PC-SETTING-PUT"
           END-IF
           MOVE FUNCTION TRIM(T-TOPIC-DELETED) TO Lbl-Status::Caption
           CALL "PC-LOAD-TOPICS"

           GOBACK.

       END PROGRAM PC-DELETE-TOPIC.

       END PROGRAM TOPICS-FORM.

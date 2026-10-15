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
       PROGRAM-ID. CHAT-FORM.

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
           SELECT CONVS-FILE ASSIGN TO WS-CONVS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CNV-ID
               ALTERNATE RECORD KEY IS CNV-TOPIC WITH DUPLICATES
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT TURNS-FILE ASSIGN TO WS-TURNS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TRN-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           *>   Each task's result, beside the answer it went into - what each
           *>   assistant actually wrote, kept to explain an answer afterwards.

           SELECT TASKRES-FILE ASSIGN TO WS-TR-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TR-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT MODELS-FILE ASSIGN TO WS-MODELS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS MDL-NAME
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT TFILES-FILE ASSIGN TO WS-TFILES-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TF-KEY
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT RTPL-FILE ASSIGN TO WS-RTPL-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS RT-NAME
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT RTSEED-FILE ASSIGN TO WS-RTSEED-PATH
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FS.
           SELECT MAINP-FILE ASSIGN TO WS-MAINP-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PRM-KEY
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
       FD CONVS-FILE IS GLOBAL.
       01 CONV-REC.
           05 CNV-ID                             PIC    X(16).
           05 CNV-TOPIC                          PIC    X(16).
           05 CNV-TITLE                          PIC    X(60).
           05 CNV-CREATED                        PIC    X(14).
           05 CNV-IN-TOKENS                      PIC    9(9).
           05 CNV-OUT-TOKENS                     PIC    9(9).
           05 CNV-TURNS                          PIC    9(5).
       FD TURNS-FILE IS GLOBAL.
       01 TURN-REC.
           05 TRN-KEY.
               10 TRN-CONV                       PIC    X(16).
               10 TRN-SEQ                        PIC    9(5).
           05 TRN-ROLE                           PIC    X.
           05 TRN-TEXT                           PIC    X(2000).
       FD TASKRES-FILE IS GLOBAL.
       01 TASKRES-REC.
           05 TR-KEY.
               10 TR-CONV                        PIC    X(16).
               10 TR-ANSWER                      PIC    9(5).
               10 TR-TASK                        PIC    99.
               10 TR-PIECE                       PIC    99.
           05 TR-STATE                           PIC    X.
           05 TR-TASK-TEXT                       PIC    X(500).
           05 TR-TEXT                            PIC    X(2000).
       FD MODELS-FILE IS GLOBAL.
       01 MODEL-REC.
           05 MDL-NAME                           PIC    X(30).
           05 MDL-API                            PIC    X(12).
           05 MDL-URL                            PIC    X(200).
           05 MDL-MODEL                          PIC    X(80).
           *>   The capability table (spec 071 R31, 063 R72): whether the model
           *>   can call tools, and how well it orchestrates, 1 (poorly) to 9.

           05 MDL-TOOLS                          PIC    X.
           05 MDL-RANK                           PIC    9.
       FD TFILES-FILE IS GLOBAL.
       *>   The users' indexed files each topic registers by path (R20-R25):
       *>   the data file and its .cidx. Nothing is copied or attached.

       01 TFILE-REC.
           05 TF-KEY.
               10 TF-TOPIC                       PIC    X(16).
               10 TF-SEQ                         PIC    9(3).
           05 TF-NAME                            PIC    X(30).
           05 TF-DATA                            PIC    X(250).
           05 TF-CIDX                            PIC    X(250).
       FD RTPL-FILE IS GLOBAL.
       *>   One report template: its name and what it says - what it suits,
       *>   its structure, its look - as the model reads it.

       01 RTPL-REC.
           05 RT-NAME                            PIC    X(40).
           05 RT-SUITS                           PIC    X(300).
           05 RT-SKEL                            PIC    X(12000).
       FD RTSEED-FILE IS GLOBAL.
       01 RTSEED-REC                             PIC    X(1000).
       *>   The Prompt screen's versions - the same file and layout
       *>   (prompts-form); the chat reads the main prompt's active one.

       FD MAINP-FILE IS GLOBAL.
       01 PROMPT-REC.
           05 PRM-KEY.
               10 PRM-TOPIC                      PIC    X(16).
               10 PRM-VERSION                    PIC    9(4).
           05 PRM-CREATED                        PIC    X(14).
           05 PRM-ACTIVE                         PIC    X.
           05 PRM-TEXT                           PIC    X(32000).
       FD MPSEED-FILE IS GLOBAL.
       01 MPSEED-REC                             PIC    X(1000).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'CHAT-FORM'.

      *>── AI Agent infrastructure ────────────────────────────────────
      *>   INVOKE agent-id 'Ask' USING BY VALUE WS-AGENT-PROMPT
      *>   returns at once; the reply arrives as onResponse (LastReply)
      *>   or onError (LastError).
       01 WS-AGENT-PROMPT        PIC X(4096)  VALUE SPACES.
       01 WS-AGENT-RESPONSE      PIC X(32767) VALUE SPACES.
       01 WS-AGENT-ERROR         PIC X(512)   VALUE SPACES.

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
       01 WS-CONVS-PATH                          GLOBAL    PIC X(240).
       01 WS-TURNS-PATH                          GLOBAL    PIC X(240).
       01 WS-MODELS-PATH                         GLOBAL    PIC X(240).
       01 WS-TFILES-PATH                         GLOBAL    PIC X(240).
       01 WS-CUR-TOPIC                           GLOBAL    PIC X(16).
       01 WS-ALLOWED-TOPIC                       GLOBAL    PIC X(16).
       01 WS-CUR-CONV                            GLOBAL    PIC X(16).
       01 WS-SEQ                                 GLOBAL    PIC 9(5)    VALUE 0.
       01 WS-ITEM                                GLOBAL    PIC X(40).
       01 WS-ROW-ID                              GLOBAL    PIC X(17).
       *>   A question - and an answer - is not cut short (operator,
       *>   2026-09-27: "do not limit the size of the question"): 64,000
       *>   characters each, the prompt built from them 256,000. A turn longer
       *>   than a TURNS-FILE record is saved over several records (see
       *>   PC-SAVE-TURN), so the file's layout does not change.

       01 WS-QUESTION                            GLOBAL    PIC X(64000).
       01 WS-REPLY                               GLOBAL    PIC X(64000).
       01 WS-ROLE                                GLOBAL    PIC X.
       01 WS-TEXT                                GLOBAL    PIC X(64000).
       01 WS-MSG                                 GLOBAL    PIC X(64100).
       *>   The conversation so far, sent with each question (R41). An
       *>   AgentObject remembers nothing between questions.

       01 WS-PROMPT                              GLOBAL    PIC X(256000).
       01 WS-PTR                                 GLOBAL    PIC 9(6).
       01 WS-HISTORY-TURNS                       GLOBAL    PIC 9(3)    VALUE 10.
       01 WS-FIRST-SEQ                           GLOBAL    PIC 9(5).
       01 WS-LINE                                GLOBAL    PIC X(300).
       01 WS-MONTH                               GLOBAL    PIC X(6).
       01 WS-SUM-IN                              GLOBAL    PIC 9(12).
       01 WS-SUM-OUT                             GLOBAL    PIC 9(12).
       01 WS-SUM-CONVS                           GLOBAL    PIC 9(6).
       01 WS-E1                                  GLOBAL    PIC Z(11)9.
       01 WS-E2                                  GLOBAL    PIC Z(11)9.
       01 WS-E3                                  GLOBAL    PIC Z(5)9.
       01 WS-TOKENS                              GLOBAL    PIC 9(9).
       01 WS-CONV-COUNT                          GLOBAL    PIC 9(4).
       01 WS-CONV-LIST                           GLOBAL.
           05 WS-CONV-ROW                        OCCURS    50.
               10 WS-CL-ID                       PIC    X(16).
               10 WS-CL-TITLE                    PIC    X(60).
       01 WS-I                                   GLOBAL    PIC 9(4).
       *>   The mesh (spec 071 R26-R32; 063 R51-R71): three agents, each with
       *>   the model-list entry the settings assign it, its capabilities from
       *>   that entry, and the role prompt designed on it.

       01 WS-AGENTS                              GLOBAL.
           05 WS-AG                              OCCURS    3.
               10 WS-AG-ENTRY                    PIC    X(30).
               10 WS-AG-MODEL                    PIC    X(80).
               10 WS-AG-TOOLS                    PIC    X.
               10 WS-AG-RANK                     PIC    9.
               10 WS-AG-ROLE                     PIC    X(1000).
               10 WS-AG-TASK                     PIC    X(2000).
               10 WS-AG-RESULT                   PIC    X(64000).
               10 WS-AG-WAIT                     PIC    X.
               *>       The task it is answering now (WS-TASKS), 0 when none.

               10 WS-AG-TASKNO                   PIC    99.
       01 WS-N-AGENTS                            GLOBAL    PIC 9    VALUE 0.
       01 WS-N-TOOLS                             GLOBAL    PIC 9    VALUE 0.
       01 WS-ORCH                                GLOBAL    PIC 9    VALUE 0.
       01 WS-WORKER                              GLOBAL    PIC 9    VALUE 0.
       01 WS-FROM                                GLOBAL    PIC 9.
       01 WS-A                                   GLOBAL    PIC 9.
       01 WS-B                                   GLOBAL    PIC 9.
       01 WS-BEST                                GLOBAL    PIC 9.
       01 WS-SAME-MODEL                          GLOBAL    PIC X.
       01 WS-STAGE                               GLOBAL    PIC X(8)    VALUE "IDLE".
       01 WS-PENDING                             GLOBAL    PIC 99    VALUE 0.
       01 WS-TOPIC-PROMPT                        GLOBAL    PIC X(1000).
       01 WS-HISTORY                             GLOBAL    PIC X(256000).
       *>   The plan's tasks (operator, 2026-10-01): up to 12 small ones,
       *>   queued - each working agent takes the next as soon as it has
       *>   answered the last - so a long question is answered in parts that
       *>   each fit one answer. Every result is kept, in task order: "D" done,
       *>   "F" failed or empty, "Q" queued, "R" running.

       01 WS-TASK-COUNT                          GLOBAL    PIC 99    VALUE 0.
       01 WS-TASKS                               GLOBAL.
           05 WS-TASK-ENTRY                      OCCURS    12.
               10 WS-TASK                        PIC    X(500).
               10 WS-TASK-ST                     PIC    X.
               10 WS-TASK-RES                    PIC    X(32000).
       01 WS-LINES                               GLOBAL.
           05 WS-L                               PIC    X(500)    OCCURS 16.
       01 WS-NEXT-TASK                           GLOBAL    PIC 99.
       01 WS-TK                                  GLOBAL    PIC 99.
       01 WS-TASK-OK                             GLOBAL    PIC X.
       01 WS-TASKS-RAN                           GLOBAL    PIC X    VALUE "N".
       *>   "JOIN:" in the plan: the user asked for every part in detail, so
       *>   the parts are given as the assistants wrote them, in task order,
       *>   not rewritten into one shorter answer.

       01 WS-JOIN-MODE                           GLOBAL    PIC X    VALUE "N".
       01 WS-NUM-ED                              GLOBAL    PIC Z9.
       01 WS-ANSWER-SEQ                          GLOBAL    PIC 9(5).
       01 WS-TR-PATH                             GLOBAL    PIC X(240).
       01 WS-T-IN                                GLOBAL    PIC 9(9).
       01 WS-T-OUT                               GLOBAL    PIC 9(9).
       01 WS-RANDOM                              GLOBAL    PIC 9V9(6).
       01 WS-ROLES-SEEN                          GLOBAL    PIC X    VALUE "N".
       *>   The topic's registered data files (R20-R25), by the name each was
       *>   registered under, and what went wrong with any that could not be.

       01 WS-REG-COUNT                           GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-REG-NAMES                           GLOBAL.
           05 WS-REG-NAME                        PIC    X(30)    OCCURS 100.
       01 WS-FILE-NOTE                           GLOBAL    PIC X(300).
       01 WS-FAILED                              GLOBAL    PIC 9(3).
       *>   New conversation's topic picker (PICK-FORM): the ids behind the
       *>   names it shows, the preselected one and its answer.

       01 WS-PICK-COUNT                          GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-PICK-IDS                            GLOBAL.
           05 WS-PICK-ID                         PIC    X(16)    OCCURS 200.
       01 WS-PICK-LIST                           GLOBAL    PIC X(8400).
       01 WS-PICK-PTR                            GLOBAL    PIC 9(5).
       01 WS-PICK-SEL                            GLOBAL    PIC 9(4).
       01 WS-PICK-ANSWER                         GLOBAL    PIC X(10).
       01 WS-PICK-N                              GLOBAL    PIC S9(4).
       *>   Whether any topic exists (the menu), and how many sources - KB
       *>   documents plus data files - the current topic has (the prompt).

       01 WS-HAS-TOPIC                           GLOBAL    PIC X    VALUE "N".
       01 WS-SOURCES                             GLOBAL    PIC 9(5)    VALUE 0.
       01 WS-SOURCES-WAS                         GLOBAL    PIC 9(5)    VALUE 0.
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   OPEN-TOPIC-MENU

           05 FILLER                             PIC    X(120)    VALUE "Open a topic first - **Topics** in the menu.".
           05 FILLER                             PIC    X(120)    VALUE "Abra um tópico primeiro - **Tópicos** no menu.".
           05 FILLER                             PIC    X(120)    VALUE "Primero abre un tema: **Temas** en el menú.".
           05 FILLER                             PIC    X(120)    VALUE "Ouvrez d'abord un sujet : **Sujets** dans le menu.".
           05 FILLER                             PIC    X(120)    VALUE "先にトピックを開いてください - メニューの **トピック**。".
           05 FILLER                             PIC    X(120)    VALUE "请先打开一个主题——菜单中的 **主题**。".
           *>   ASSIGN-MODEL-MENU

           05 FILLER                             PIC    X(120)    VALUE "Assign a model to an agent first - **RAG settings** in the menu.".
           05 FILLER                             PIC    X(120)    VALUE "Atribua um modelo a um agente primeiro - **Configurações de RAG** no menu.".
           05 FILLER                             PIC    X(120)    VALUE "Primero asigna un modelo a un agente: **Configuración de RAG** en el menú.".
           05 FILLER                             PIC    X(120)    VALUE "Attribuez d'abord un modèle à un agent : **Paramètres RAG** dans le menu.".
           05 FILLER                             PIC    X(120)    VALUE "先にエージェントにモデルを割り当ててください - メニューの **RAG 設定**。".
           05 FILLER                             PIC    X(120)    VALUE "请先为代理分配模型——菜单中的 **RAG 设置**。".
           *>   YOU

           05 FILLER                             PIC    X(120)    VALUE "You".
           05 FILLER                             PIC    X(120)    VALUE "Você".
           05 FILLER                             PIC    X(120)    VALUE "Tú".
           05 FILLER                             PIC    X(120)    VALUE "Vous".
           05 FILLER                             PIC    X(120)    VALUE "あなた".
           05 FILLER                             PIC    X(120)    VALUE "你".
           *>   THINKING

           05 FILLER                             PIC    X(120)    VALUE "Thinking...".
           05 FILLER                             PIC    X(120)    VALUE "Pensando...".
           05 FILLER                             PIC    X(120)    VALUE "Pensando...".
           05 FILLER                             PIC    X(120)    VALUE "Réflexion...".
           05 FILLER                             PIC    X(120)    VALUE "考え中...".
           05 FILLER                             PIC    X(120)    VALUE "思考中...".
           *>   AGENT-FAILED

           05 FILLER                             PIC    X(120)    VALUE "Agent &1 could not answer:".
           05 FILLER                             PIC    X(120)    VALUE "O agente &1 não conseguiu responder:".
           05 FILLER                             PIC    X(120)    VALUE "El agente &1 no pudo responder:".
           05 FILLER                             PIC    X(120)    VALUE "L'agent &1 n'a pas pu répondre :".
           05 FILLER                             PIC    X(120)    VALUE "エージェント &1 は回答できませんでした：".
           05 FILLER                             PIC    X(120)    VALUE "代理 &1 无法回答：".
           *>   LAST-FAILED

           05 FILLER                             PIC    X(120)    VALUE "The last question failed.".
           05 FILLER                             PIC    X(120)    VALUE "A última pergunta falhou.".
           05 FILLER                             PIC    X(120)    VALUE "La última pregunta falló.".
           05 FILLER                             PIC    X(120)    VALUE "La dernière question a échoué.".
           05 FILLER                             PIC    X(120)    VALUE "直前の質問は失敗しました。".
           05 FILLER                             PIC    X(120)    VALUE "上一个问题失败了。".
           *>   NO-MODEL

           05 FILLER                             PIC    X(120)    VALUE "No model is assigned to an agent - open RAG settings.".
           05 FILLER                             PIC    X(120)    VALUE "Nenhum modelo foi atribuído a um agente - abra as Configurações de RAG.".
           05 FILLER                             PIC    X(120)    VALUE "No hay ningún modelo asignado a un agente: abre la Configuración de RAG.".
           05 FILLER                             PIC    X(120)    VALUE "Aucun modèle n'est attribué à un agent : ouvrez les Paramètres RAG.".
           05 FILLER                             PIC    X(120)    VALUE "どのエージェントにもモデルが割り当てられていません。RAG 設定を開いてください。".
           05 FILLER                             PIC    X(120)    VALUE "没有为任何代理分配模型——请打开 RAG 设置。".
           *>   NO-TOPIC

           05 FILLER                             PIC    X(120)    VALUE "No topic - open Topics to create one.".
           05 FILLER                             PIC    X(120)    VALUE "Nenhum tópico - abra Tópicos para criar um.".
           05 FILLER                             PIC    X(120)    VALUE "Ningún tema: abre Temas para crear uno.".
           05 FILLER                             PIC    X(120)    VALUE "Aucun sujet : ouvrez Sujets pour en créer un.".
           05 FILLER                             PIC    X(120)    VALUE "トピックがありません。「トピック」を開いて作成してください。".
           05 FILLER                             PIC    X(120)    VALUE "没有主题——打开“主题”创建一个。".
           *>   TOPIC-GONE

           05 FILLER                             PIC    X(120)    VALUE "That topic no longer exists - open Topics.".
           05 FILLER                             PIC    X(120)    VALUE "Esse tópico não existe mais - abra Tópicos.".
           05 FILLER                             PIC    X(120)    VALUE "Ese tema ya no existe: abre Temas.".
           05 FILLER                             PIC    X(120)    VALUE "Ce sujet n'existe plus : ouvrez Sujets.".
           05 FILLER                             PIC    X(120)    VALUE "そのトピックはもう存在しません。「トピック」を開いてください。".
           05 FILLER                             PIC    X(120)    VALUE "该主题已不存在——请打开“主题”。".
           *>   WORKING

           05 FILLER                             PIC    X(120)    VALUE "Working: &1 task(s) across &2 agent(s)...".
           05 FILLER                             PIC    X(120)    VALUE "Trabalhando: &1 tarefa(s) em &2 agente(s)...".
           05 FILLER                             PIC    X(120)    VALUE "Trabajando: &1 tarea(s) entre &2 agente(s)...".
           05 FILLER                             PIC    X(120)    VALUE "En cours : &1 tâche(s) sur &2 agent(s)...".
           05 FILLER                             PIC    X(120)    VALUE "処理中：&1 件のタスクを &2 体のエージェントで実行中...".
           05 FILLER                             PIC    X(120)    VALUE "处理中：&1 个任务，由 &2 个代理执行...".
           *>   MENU-CHAT

           05 FILLER                             PIC    X(120)    VALUE "Chat".
           05 FILLER                             PIC    X(120)    VALUE "Chat".
           05 FILLER                             PIC    X(120)    VALUE "Chat".
           05 FILLER                             PIC    X(120)    VALUE "Discussion".
           05 FILLER                             PIC    X(120)    VALUE "チャット".
           05 FILLER                             PIC    X(120)    VALUE "聊天".
           *>   MENU-NEWC

           05 FILLER                             PIC    X(120)    VALUE "New conversation".
           05 FILLER                             PIC    X(120)    VALUE "Nova conversa".
           05 FILLER                             PIC    X(120)    VALUE "Nueva conversación".
           05 FILLER                             PIC    X(120)    VALUE "Nouvelle conversation".
           05 FILLER                             PIC    X(120)    VALUE "新しい会話".
           05 FILLER                             PIC    X(120)    VALUE "新对话".
           *>   TOPICS-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Topics".
           05 FILLER                             PIC    X(120)    VALUE "Tópicos".
           05 FILLER                             PIC    X(120)    VALUE "Temas".
           05 FILLER                             PIC    X(120)    VALUE "Sujets".
           05 FILLER                             PIC    X(120)    VALUE "トピック".
           05 FILLER                             PIC    X(120)    VALUE "主题".
           *>   DOCS-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Documents".
           05 FILLER                             PIC    X(120)    VALUE "Documentos".
           05 FILLER                             PIC    X(120)    VALUE "Documentos".
           05 FILLER                             PIC    X(120)    VALUE "Documents".
           05 FILLER                             PIC    X(120)    VALUE "文書".
           05 FILLER                             PIC    X(120)    VALUE "文档".
           *>   FILES-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Data files".
           05 FILLER                             PIC    X(120)    VALUE "Arquivos de dados".
           05 FILLER                             PIC    X(120)    VALUE "Archivos de datos".
           05 FILLER                             PIC    X(120)    VALUE "Fichiers de données".
           05 FILLER                             PIC    X(120)    VALUE "データファイル".
           05 FILLER                             PIC    X(120)    VALUE "数据文件".
           *>   PROMPT-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Prompt".
           05 FILLER                             PIC    X(120)    VALUE "Prompt".
           05 FILLER                             PIC    X(120)    VALUE "Prompt".
           05 FILLER                             PIC    X(120)    VALUE "Prompt".
           05 FILLER                             PIC    X(120)    VALUE "プロンプト".
           05 FILLER                             PIC    X(120)    VALUE "提示词".
           *>   SETTINGS-TITLE

           05 FILLER                             PIC    X(120)    VALUE "RAG settings".
           05 FILLER                             PIC    X(120)    VALUE "Configurações de RAG".
           05 FILLER                             PIC    X(120)    VALUE "Configuración de RAG".
           05 FILLER                             PIC    X(120)    VALUE "Paramètres RAG".
           05 FILLER                             PIC    X(120)    VALUE "RAG 設定".
           05 FILLER                             PIC    X(120)    VALUE "RAG 设置".
           *>   CONVERSATIONS

           05 FILLER                             PIC    X(120)    VALUE "Conversations".
           05 FILLER                             PIC    X(120)    VALUE "Conversas".
           05 FILLER                             PIC    X(120)    VALUE "Conversaciones".
           05 FILLER                             PIC    X(120)    VALUE "Conversations".
           05 FILLER                             PIC    X(120)    VALUE "会話".
           05 FILLER                             PIC    X(120)    VALUE "对话".
           *>   THIS-MONTH

           05 FILLER                             PIC    X(120)    VALUE "This month: &1 conversation(s), &2 input and &3 output tokens.".
           05 FILLER                             PIC    X(120)    VALUE "Este mês: &1 conversa(s), &2 tokens de entrada e &3 de saída.".
           05 FILLER                             PIC    X(120)    VALUE "Este mes: &1 conversación(es), &2 tokens de entrada y &3 de salida.".
           05 FILLER                             PIC    X(120)    VALUE "Ce mois-ci : &1 conversation(s), &2 jetons en entrée et &3 en sortie.".
           05 FILLER                             PIC    X(120)    VALUE "今月：会話 &1 件、入力トークン &2、出力トークン &3。".
           05 FILLER                             PIC    X(120)    VALUE "本月：&1 次对话，输入 &2 个令牌，输出 &3 个令牌。".
           *>   FILES-UNUSABLE

           05 FILLER                             PIC    X(120)    VALUE "Data files not usable: &1.".
           05 FILLER                             PIC    X(120)    VALUE "Arquivos de dados inutilizáveis: &1.".
           05 FILLER                             PIC    X(120)    VALUE "Archivos de datos no utilizables: &1.".
           05 FILLER                             PIC    X(120)    VALUE "Fichiers de données inutilisables : &1.".
           05 FILLER                             PIC    X(120)    VALUE "使用できないデータファイル：&1。".
           05 FILLER                             PIC    X(120)    VALUE "无法使用的数据文件：&1。".
           *>   ORCH-LINE

           05 FILLER                             PIC    X(120)    VALUE "Orchestrator: agent &1, tools: agent &2.".
           05 FILLER                             PIC    X(120)    VALUE "Orquestrador: agente &1, ferramentas: agente &2.".
           05 FILLER                             PIC    X(120)    VALUE "Orquestador: agente &1, herramientas: agente &2.".
           05 FILLER                             PIC    X(120)    VALUE "Orchestrateur : agent &1, outils : agent &2.".
           05 FILLER                             PIC    X(120)    VALUE "オーケストレーター：エージェント &1、ツール：エージェント &2。".
           05 FILLER                             PIC    X(120)    VALUE "编排：代理 &1，工具：代理 &2。".
           *>   HINT-ASK

           05 FILLER                             PIC    X(120)    VALUE "Ask about this topic".
           05 FILLER                             PIC    X(120)    VALUE "Pergunte sobre este tópico".
           05 FILLER                             PIC    X(120)    VALUE "Pregunta sobre este tema".
           05 FILLER                             PIC    X(120)    VALUE "Posez une question sur ce sujet".
           05 FILLER                             PIC    X(120)    VALUE "このトピックについて質問".
           05 FILLER                             PIC    X(120)    VALUE "就此主题提问".
           *>   SEND

           05 FILLER                             PIC    X(120)    VALUE "Send".
           05 FILLER                             PIC    X(120)    VALUE "Enviar".
           05 FILLER                             PIC    X(120)    VALUE "Enviar".
           05 FILLER                             PIC    X(120)    VALUE "Envoyer".
           05 FILLER                             PIC    X(120)    VALUE "送信".
           05 FILLER                             PIC    X(120)    VALUE "发送".
           *>   STATUS

           05 FILLER                             PIC    X(120)    VALUE "Status".
           05 FILLER                             PIC    X(120)    VALUE "Status".
           05 FILLER                             PIC    X(120)    VALUE "Estado".
           05 FILLER                             PIC    X(120)    VALUE "État".
           05 FILLER                             PIC    X(120)    VALUE "ステータス".
           05 FILLER                             PIC    X(120)    VALUE "状态".
           *>   MENU-WELC

           05 FILLER                             PIC    X(120)    VALUE "Getting started".
           05 FILLER                             PIC    X(120)    VALUE "Primeiros passos".
           05 FILLER                             PIC    X(120)    VALUE "Primeros pasos".
           05 FILLER                             PIC    X(120)    VALUE "Premiers pas".
           05 FILLER                             PIC    X(120)    VALUE "はじめに".
           05 FILLER                             PIC    X(120)    VALUE "入门".
           *>   PICK-TOPIC

           05 FILLER                             PIC    X(120)    VALUE "Choose the topic for the new conversation".
           05 FILLER                             PIC    X(120)    VALUE "Escolha o tópico da nova conversa".
           05 FILLER                             PIC    X(120)    VALUE "Elige el tema de la nueva conversación".
           05 FILLER                             PIC    X(120)    VALUE "Choisissez le sujet de la nouvelle conversation".
           05 FILLER                             PIC    X(120)    VALUE "新しい会話のトピックを選んでください".
           05 FILLER                             PIC    X(120)    VALUE "请选择新对话的主题".
           *>   PICK-START

           05 FILLER                             PIC    X(120)    VALUE "Start".
           05 FILLER                             PIC    X(120)    VALUE "Iniciar".
           05 FILLER                             PIC    X(120)    VALUE "Iniciar".
           05 FILLER                             PIC    X(120)    VALUE "Commencer".
           05 FILLER                             PIC    X(120)    VALUE "開始".
           05 FILLER                             PIC    X(120)    VALUE "开始".
           *>   PICK-CANCEL

           05 FILLER                             PIC    X(120)    VALUE "Cancel".
           05 FILLER                             PIC    X(120)    VALUE "Cancelar".
           05 FILLER                             PIC    X(120)    VALUE "Cancelar".
           05 FILLER                             PIC    X(120)    VALUE "Annuler".
           05 FILLER                             PIC    X(120)    VALUE "キャンセル".
           05 FILLER                             PIC    X(120)    VALUE "取消".
           *>   SAVE-PDF

           05 FILLER                             PIC    X(120)    VALUE "Save as PDF".
           05 FILLER                             PIC    X(120)    VALUE "Salvar como PDF".
           05 FILLER                             PIC    X(120)    VALUE "Guardar como PDF".
           05 FILLER                             PIC    X(120)    VALUE "Enregistrer en PDF".
           05 FILLER                             PIC    X(120)    VALUE "PDF として保存".
           05 FILLER                             PIC    X(120)    VALUE "另存为 PDF".
           *>   PDF-SAVED

           05 FILLER                             PIC    X(120)    VALUE "The conversation was saved as a PDF.".
           05 FILLER                             PIC    X(120)    VALUE "A conversa foi salva como PDF.".
           05 FILLER                             PIC    X(120)    VALUE "La conversación se guardó como PDF.".
           05 FILLER                             PIC    X(120)    VALUE "La conversation a été enregistrée en PDF.".
           05 FILLER                             PIC    X(120)    VALUE "会話を PDF として保存しました。".
           05 FILLER                             PIC    X(120)    VALUE "对话已保存为 PDF。".
           *>   PDF-EMPTY

           05 FILLER                             PIC    X(120)    VALUE "There is no conversation to save yet.".
           05 FILLER                             PIC    X(120)    VALUE "Ainda não há conversa para salvar.".
           05 FILLER                             PIC    X(120)    VALUE "Todavía no hay ninguna conversación que guardar.".
           05 FILLER                             PIC    X(120)    VALUE "Il n'y a pas encore de conversation à enregistrer.".
           05 FILLER                             PIC    X(120)    VALUE "保存する会話はまだありません。".
           05 FILLER                             PIC    X(120)    VALUE "还没有可保存的对话。".
           *>   ST-COORD

           05 FILLER                             PIC    X(120)    VALUE "Coordinating the agents...".
           05 FILLER                             PIC    X(120)    VALUE "Coordenando os agentes...".
           05 FILLER                             PIC    X(120)    VALUE "Coordinando los agentes...".
           05 FILLER                             PIC    X(120)    VALUE "Coordination des agents...".
           05 FILLER                             PIC    X(120)    VALUE "エージェントを調整中...".
           05 FILLER                             PIC    X(120)    VALUE "正在协调各个智能体...".
           *>   ST-SEARCH-KB

           05 FILLER                             PIC    X(120)    VALUE "Searching the Knowledge Base...".
           05 FILLER                             PIC    X(120)    VALUE "Consultando a base de conhecimento...".
           05 FILLER                             PIC    X(120)    VALUE "Consultando la base de conocimiento...".
           05 FILLER                             PIC    X(120)    VALUE "Recherche dans la base de connaissances...".
           05 FILLER                             PIC    X(120)    VALUE "ナレッジベースを検索中...".
           05 FILLER                             PIC    X(120)    VALUE "正在检索知识库...".
           *>   ST-READ-DATA

           05 FILLER                             PIC    X(120)    VALUE "Reading the data files...".
           05 FILLER                             PIC    X(120)    VALUE "Lendo os arquivos de dados...".
           05 FILLER                             PIC    X(120)    VALUE "Leyendo los archivos de datos...".
           05 FILLER                             PIC    X(120)    VALUE "Lecture des fichiers de données...".
           05 FILLER                             PIC    X(120)    VALUE "データファイルを読み込み中...".
           05 FILLER                             PIC    X(120)    VALUE "正在读取数据文件...".
           *>   ST-COMPOSING

           05 FILLER                             PIC    X(120)    VALUE "Preparing the answer...".
           05 FILLER                             PIC    X(120)    VALUE "Preparando a resposta...".
           05 FILLER                             PIC    X(120)    VALUE "Preparando la respuesta...".
           05 FILLER                             PIC    X(120)    VALUE "Préparation de la réponse...".
           05 FILLER                             PIC    X(120)    VALUE "回答を準備中...".
           05 FILLER                             PIC    X(120)    VALUE "正在准备回答...".
           *>   PART-MISSING

           05 FILLER                             PIC    X(120)    VALUE "Part not answered:".
           05 FILLER                             PIC    X(120)    VALUE "Parte não respondida:".
           05 FILLER                             PIC    X(120)    VALUE "Parte sin respuesta:".
           05 FILLER                             PIC    X(120)    VALUE "Partie sans réponse :".
           05 FILLER                             PIC    X(120)    VALUE "回答できなかった部分:".
           05 FILLER                             PIC    X(120)    VALUE "未能回答的部分：".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    36.
               10 PC-TEXT                        PIC    X(120)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-OPEN-TOPIC-MENU                  PIC    X(120).
           05 T-ASSIGN-MODEL-MENU                PIC    X(120).
           05 T-YOU                              PIC    X(120).
           05 T-THINKING                         PIC    X(120).
           05 T-AGENT-FAILED                     PIC    X(120).
           05 T-LAST-FAILED                      PIC    X(120).
           05 T-NO-MODEL                         PIC    X(120).
           05 T-NO-TOPIC                         PIC    X(120).
           05 T-TOPIC-GONE                       PIC    X(120).
           05 T-WORKING                          PIC    X(120).
           05 T-MENU-CHAT                        PIC    X(120).
           05 T-MENU-NEWC                        PIC    X(120).
           05 T-TOPICS-TITLE                     PIC    X(120).
           05 T-DOCS-TITLE                       PIC    X(120).
           05 T-FILES-TITLE                      PIC    X(120).
           05 T-PROMPT-TITLE                     PIC    X(120).
           05 T-SETTINGS-TITLE                   PIC    X(120).
           05 T-CONVERSATIONS                    PIC    X(120).
           05 T-THIS-MONTH                       PIC    X(120).
           05 T-FILES-UNUSABLE                   PIC    X(120).
           05 T-ORCH-LINE                        PIC    X(120).
           05 T-HINT-ASK                         PIC    X(120).
           05 T-SEND                             PIC    X(120).
           05 T-STATUS                           PIC    X(120).
           05 T-MENU-WELC                        PIC    X(120).
           05 T-PICK-TOPIC                       PIC    X(120).
           05 T-PICK-START                       PIC    X(120).
           05 T-PICK-CANCEL                      PIC    X(120).
           05 T-SAVE-PDF                         PIC    X(120).
           05 T-PDF-SAVED                        PIC    X(120).
           05 T-PDF-EMPTY                        PIC    X(120).
           05 T-ST-COORD                         PIC    X(120).
           05 T-ST-SEARCH-KB                     PIC    X(120).
           05 T-ST-READ-DATA                     PIC    X(120).
           05 T-ST-COMPOSING                     PIC    X(120).
           05 T-PART-MISSING                     PIC    X(120).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(120)    OCCURS 36.
       *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.

       01 WS-FMT                                 GLOBAL    PIC X(120).
       01 WS-ARG1                                GLOBAL    PIC X(300).
       01 WS-ARG2                                GLOBAL    PIC X(300).
       01 WS-ARG3                                GLOBAL    PIC X(300).
       01 WS-ARG4                                GLOBAL    PIC X(300).
       01 WS-FMT-OUT                             GLOBAL    PIC X(1200).
       01 WS-CONFIGURED                          GLOBAL    PIC X    VALUE "N".
       01 WS-SHOWN-TOPIC                         GLOBAL    PIC X(16)    VALUE SPACES.
       *>   The status bubble (operator, 2026-09-27): ONE agent message that
       *>   says what is being done - thinking, coordinating the agents,
       *>   searching the Knowledge Base, preparing the answer - and then
       *>   BECOMES the answer. Its id, while it is on screen.

       01 WS-STATUS-ID                           GLOBAL    PIC X(20)    VALUE SPACES.
       01 WS-STATUS-TEXT                         GLOBAL    PIC X(300).
       01 WS-TOOL-KIND                           GLOBAL    PIC X(20).
       *>   Saving and reading a turn in 2,000-character pieces.

       01 WS-PIECE-AT                            GLOBAL    PIC 9(6).
       01 WS-PIECE-ROLE                          GLOBAL    PIC X.
       01 WS-JOIN-PT                             GLOBAL    PIC 9(6).
       01 WS-JOIN-ROLE                           GLOBAL    PIC X    VALUE SPACE.
       01 WS-JOIN                                GLOBAL    PIC X(64000).
       *>   Where a joined turn goes: "P" the prompt, "V" the Viewer.

       01 WS-JOIN-TO                             GLOBAL    PIC X    VALUE SPACE.
       *>   A formatted answer - a table, a Markdown or HTML presentation - goes
       *>   straight into the conversation, not into a bubble (operator,
       *>   2026-09-27). PC-IS-FORMATTED decides, from WS-MSG and, for a live
       *>   answer, the question that asked for it.

       01 WS-FORMATTED                           GLOBAL    PIC X    VALUE "N".
       01 WS-FMT-HTML                            GLOBAL    PIC X    VALUE "N".
       01 WS-SHOW-ANSWER                         GLOBAL    PIC X    VALUE "N".
       01 WS-FMT-T                               GLOBAL    PIC 9(6).
       *>   PC-APPEND-FORMATTED's place in WS-MSG, the fence it found, and
       *>   the page inside it.

       01 WS-FP                                  GLOBAL    PIC 9(6).
       01 WS-FA                                  GLOBAL    PIC 9(6).
       01 WS-FB                                  GLOBAL    PIC 9(6).
       01 WS-BS                                  GLOBAL    PIC 9(6).
       01 WS-FC                                  GLOBAL    PIC 9(6).
       01 WS-REST                                GLOBAL    PIC 9(6).
       *>   The kind of page found: "F" a ```html fence, "R" a page written
       *>   straight into the answer, space none.

       01 WS-FKIND                               GLOBAL    PIC X.
       *>   The Markdown before a page, and whether a fence the page sat in
       *>   ("```" or "```HTML", not "```html") was cut off with it.

       01 WS-FQ                                  GLOBAL    PIC 9(6).
       01 WS-FSTRIP                              GLOBAL    PIC X.
       *>   Where the orchestrator's prompt ends, for what is added to it.

       01 WS-SYS-PT                              GLOBAL    PIC 9(6).
       01 WS-LOWQ                                GLOBAL    PIC X(64000).
       *>   The report templates (operator, 2026-09-28): data/templates.idx,
       *>   each a name, what it suits and its HTML skeleton, seeded from
       *>   samples/report-templates.txt the first time.

       01 WS-SAMPLES-DIR                         GLOBAL    PIC X(240).
       01 WS-RTPL-PATH                           GLOBAL    PIC X(240).
       01 WS-RTSEED-PATH                         GLOBAL    PIC X(240).
       *>   An upgrade of the shipped templates (operator, 2026-09-30: a
       *>   report is 80 % of the viewer, up to 1600 px). The templates shipped
       *>   before are samples/report-templates-previous.txt: a stored template
       *>   still equal to one of them was never changed, and takes the shipped
       *>   template of its name; one the user changed is left as it is.
       *>   WS-RT-MODE: "S" seed, "O" read the previous ones, "U" upgrade.

       01 WS-RT-MODE                             GLOBAL    PIC X    VALUE "S".
       01 WS-RT-UPGRADED                         GLOBAL    PIC X    VALUE "N".
       01 WS-RTPREV-PATH                         GLOBAL    PIC X(240).
       01 WS-RTNEW-PATH                          GLOBAL    PIC X(240).
       01 WS-OLD-TPLS                            GLOBAL.
           05 WS-OLD-N                           PIC    9(3)    VALUE 0.
           05 WS-OLD-TPL                         OCCURS    40 TIMES.
               10 WS-OLD-NAME                    PIC    X(40).
               10 WS-OLD-SKEL                    PIC    X(12000).
       01 WS-OLD-I                               GLOBAL    PIC 9(3).
       01 WS-RT-NAME                             GLOBAL    PIC X(40).
       01 WS-RT-SUITS                            GLOBAL    PIC X(300).
       01 WS-RT-SKEL                             GLOBAL    PIC X(12000).
       *>   The template the user chose for this question, once per question.

       01 WS-TPL-NAME                            GLOBAL    PIC X(40).
       01 WS-TPL-SKEL                            GLOBAL    PIC X(12000).
       01 WS-TPL-DONE                            GLOBAL    PIC X    VALUE "N".
       01 WS-CATALOG                             GLOBAL    PIC X(8000).
       *>   The main prompt (operator, 2026-09-28: every instruction in English,
       *>   in one prompt the user edits). Its versions live in
       *>   data/prompt-versions.idx under the topic id "*MAIN"; before it has
       *>   one, samples/main-prompt.md is the text.

       01 WS-MAINP-PATH                          GLOBAL    PIC X(240).
       01 WS-MPSEED-PATH                         GLOBAL    PIC X(240).
       01 WS-MAIN                                GLOBAL    PIC X(32000).
       01 WS-MAIN-ELECTED                        GLOBAL    PIC X(32000).
       *>   One section of it (PC-SECTION), and its {PLACEHOLDERS} filled
       *>   (PC-FILL).

       01 WS-SEC-NAME                            GLOBAL    PIC X(20).
       01 WS-SEC-MARK                            GLOBAL    PIC X(5)    VALUE X"0A3D3D3D20".
       01 WS-SEC                                 GLOBAL    PIC X(64000).
       01 WS-SPL                                 GLOBAL    PIC X(64000).
       01 WS-INS                                 GLOBAL    PIC X(64000).
       01 WS-TOKEN                               GLOBAL    PIC X(20).
       01 WS-NOTE                                GLOBAL    PIC X(4000).
       01 WS-SX-A                                GLOBAL    PIC 9(6).
       01 WS-SX-B                                GLOBAL    PIC 9(6).
       01 WS-SX-E                                GLOBAL    PIC 9(6).
       01 WS-SX-L                                GLOBAL    PIC 9(6).
       *>   A seed file's line and its length without the trailing spaces.

       01 WS-LN-BUF                              GLOBAL    PIC X(1000).
       01 WS-LN-REV                              GLOBAL    PIC X(1000).
       01 WS-LN-LEN                              GLOBAL    PIC 9(4).
       01 WS-RT-PT                               GLOBAL    PIC 9(6).
       01 WS-RT-AT                               GLOBAL    PIC 9(6).
       01 WS-RT-LEN                              GLOBAL    PIC 9(6).
       01 WS-RT-END                              GLOBAL    PIC 9(6).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-SideMenu-1.
          05 WS-SideMenu-1-TEXT       PIC X(256) VALUE 'SideMenu-1'.
          05 WS-SideMenu-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-SideMenu-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Topic.
          05 WS-Lbl-Topic-TEXT       PIC X(256) VALUE 'Lbl-Topic'.
          05 WS-Lbl-Topic-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Topic-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Vwr-Chat.
          05 WS-Vwr-Chat-TEXT       PIC X(256) VALUE 'Vwr-Chat'.
          05 WS-Vwr-Chat-VISIBLE    PIC 9      VALUE 1.
          05 WS-Vwr-Chat-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Input.
          05 WS-Txt-Input-TEXT       PIC X(8192) VALUE SPACES.
          05 WS-Txt-Input-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Input-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Input-VALUE      PIC X(8192) VALUE SPACES.

       01 WS-Btn-Send.
          05 WS-Btn-Send-TEXT       PIC X(256) VALUE 'Send'.
          05 WS-Btn-Send-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Send-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Pdf.
          05 WS-Btn-Pdf-TEXT       PIC X(256) VALUE 'Save as PDF'.
          05 WS-Btn-Pdf-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Pdf-ENABLED    PIC 9      VALUE 1.

       01 WS-AGENT-1.
          05 WS-AGENT-1-TEXT       PIC X(256) VALUE 'AGENT-1'.
          05 WS-AGENT-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AGENT-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AGENT-2.
          05 WS-AGENT-2-TEXT       PIC X(256) VALUE 'AGENT-2'.
          05 WS-AGENT-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AGENT-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AGENT-3.
          05 WS-AGENT-3-TEXT       PIC X(256) VALUE 'AGENT-3'.
          05 WS-AGENT-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AGENT-3-ENABLED    PIC 9      VALUE 1.

       01 WS-KB-1.
          05 WS-KB-1-TEXT       PIC X(256) VALUE 'KB-1'.
          05 WS-KB-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-KB-1-ENABLED    PIC 9      VALUE 1.

       01 WS-SideMenu-1-Footer.
          05 WS-SideMenu-1-Footer-TEXT       PIC X(256) VALUE 'SideMenu-1-Footer'.
          05 WS-SideMenu-1-Footer-VISIBLE    PIC 9      VALUE 1.
          05 WS-SideMenu-1-Footer-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-en.
          05 WS-Flag-en-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-en-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-en-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-pt.
          05 WS-Flag-pt-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-pt-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-pt-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-es.
          05 WS-Flag-es-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-es-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-es-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-fr.
          05 WS-Flag-fr-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-fr-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-fr-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-jp.
          05 WS-Flag-jp-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-jp-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-jp-ENABLED    PIC 9      VALUE 1.

       01 WS-Flag-cn.
          05 WS-Flag-cn-TEXT       PIC X(256) VALUE ''.
          05 WS-Flag-cn-VISIBLE    PIC 9      VALUE 1.
          05 WS-Flag-cn-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "CHAT-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CHAT-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "CHAT-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onActivate"
                               CALL "CHAT-FORM--ONACTIVATE"
                           WHEN "onResize"
                               CALL "CHAT-FORM--ONRESIZE"
                       END-EVALUATE
                   WHEN "SideMenu-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMenuItemClick"
                               CALL "SIDEMENU-1--ONMENUITEMCLICK"
                       END-EVALUATE
                   WHEN "Vwr-Chat"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onSaveComplete"
                               CALL "VWR-CHAT--ONSAVECOMPLETE"
                           WHEN "onError"
                               CALL "VWR-CHAT--ONERROR"
                       END-EVALUATE
                   WHEN "Btn-Send"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SEND--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Pdf"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PDF--ONCLICK"
                       END-EVALUATE
                   WHEN "AGENT-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResponse"
                               CALL "AGENT-1--ONRESPONSE"
                           WHEN "onError"
                               CALL "AGENT-1--ONERROR"
                           WHEN "onTimeout"
                               CALL "AGENT-1--ONTIMEOUT"
                           WHEN "onModelChanged"
                               CALL "AGENT-1--ONMODELCHANGED"
                           WHEN "onToolUse"
                               CALL "AGENT-1--ONTOOLUSE"
                       END-EVALUATE
                   WHEN "AGENT-2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResponse"
                               CALL "AGENT-2--ONRESPONSE"
                           WHEN "onError"
                               CALL "AGENT-2--ONERROR"
                           WHEN "onTimeout"
                               CALL "AGENT-2--ONTIMEOUT"
                           WHEN "onModelChanged"
                               CALL "AGENT-2--ONMODELCHANGED"
                           WHEN "onToolUse"
                               CALL "AGENT-2--ONTOOLUSE"
                       END-EVALUATE
                   WHEN "AGENT-3"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResponse"
                               CALL "AGENT-3--ONRESPONSE"
                           WHEN "onError"
                               CALL "AGENT-3--ONERROR"
                           WHEN "onTimeout"
                               CALL "AGENT-3--ONTIMEOUT"
                           WHEN "onModelChanged"
                               CALL "AGENT-3--ONMODELCHANGED"
                           WHEN "onToolUse"
                               CALL "AGENT-3--ONTOOLUSE"
                       END-EVALUATE
                   WHEN "Flag-en"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-EN--ONCLICK"
                       END-EVALUATE
                   WHEN "Flag-pt"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-PT--ONCLICK"
                       END-EVALUATE
                   WHEN "Flag-es"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-ES--ONCLICK"
                       END-EVALUATE
                   WHEN "Flag-fr"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-FR--ONCLICK"
                       END-EVALUATE
                   WHEN "Flag-jp"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-JP--ONCLICK"
                       END-EVALUATE
                   WHEN "Flag-cn"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FLAG-CN--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>
       AGENT-1-ASK.
      *>    Ask the AI agent AGENT-1 (model: llama3.2, endpoint: http://localhost:11434)
      *>    Set WS-AGENT-PROMPT before calling.
      *>    Returns at once. The reply arrives as AGENT-1--ONRESPONSE
      *>    (read LastReply) or --ONERROR (read LastError).
           INVOKE AGENT-1 'Ask'
               USING BY VALUE WS-AGENT-PROMPT.

       AGENT-1-ON-RESPONSE.
      *>    TODO: AGENT-1 — not called by the runtime; bind onResponse and read LastReply
           CONTINUE.

       AGENT-1-ON-ERROR.
      *>    TODO: AGENT-1 — not called by the runtime; bind onError and read LastError
           CONTINUE.

       AGENT-2-ASK.
      *>    Ask the AI agent AGENT-2 (model: llama3.2, endpoint: http://localhost:11434)
      *>    Set WS-AGENT-PROMPT before calling.
      *>    Returns at once. The reply arrives as AGENT-2--ONRESPONSE
      *>    (read LastReply) or --ONERROR (read LastError).
           INVOKE AGENT-2 'Ask'
               USING BY VALUE WS-AGENT-PROMPT.

       AGENT-2-ON-RESPONSE.
      *>    TODO: AGENT-2 — not called by the runtime; bind onResponse and read LastReply
           CONTINUE.

       AGENT-2-ON-ERROR.
      *>    TODO: AGENT-2 — not called by the runtime; bind onError and read LastError
           CONTINUE.

       AGENT-3-ASK.
      *>    Ask the AI agent AGENT-3 (model: llama3.2, endpoint: http://localhost:11434)
      *>    Set WS-AGENT-PROMPT before calling.
      *>    Returns at once. The reply arrives as AGENT-3--ONRESPONSE
      *>    (read LastReply) or --ONERROR (read LastError).
           INVOKE AGENT-3 'Ask'
               USING BY VALUE WS-AGENT-PROMPT.

       AGENT-3-ON-RESPONSE.
      *>    TODO: AGENT-3 — not called by the runtime; bind onResponse and read LastReply
           CONTINUE.

       AGENT-3-ON-ERROR.
      *>    TODO: AGENT-3 — not called by the runtime; bind onError and read LastError
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHAT-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-PATHS"
           CALL    "PC-TEXTS"
           CALL    "PC-READ-ROLES"
           CALL    "PC-HAND-MODELS"

           SET     WS-ALLOWED-TOPIC              TO    SPACES
           CALL    "PC-LOAD-TOPIC"
           IF    WS-CUR-TOPIC NOT = SPACES
               CALL    "PC-NEW-CONV"
           END-IF
           *>   No topic is forced open any more: with no model yet the menu is
           *>   shut and the welcome form explains; with one, Topics is a click.

           IF    WS-CUR-TOPIC = SPACES
               CALL    "PC-ELECT"
           END-IF

           CALL    "PC-LIST-CONVS"
           CALL    "PC-MENU-STATE"
           CALL    "PC-STATUS"

           *>   Nothing configured yet: the ContentPane opens on the welcome form
           *>   instead of an empty chat.

           IF    WS-CONFIGURED = "N"
               SET     WS-ITEM                   TO    SideMenu-1::ActivateItem("welc")
           END-IF

           Txt-Input::SetFocus()

           EXIT PROGRAM.

           GOBACK.

       END PROGRAM CHAT-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHAT-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Home again (the Chat row): pick up what the other forms changed.

           CALL    "PC-REFRESH"

           GOBACK.

       END PROGRAM CHAT-FORM--ONACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHAT-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CHAT-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHAT-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           GOBACK.

       END PROGRAM CHAT-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SIDEMENU-1--ONMENUITEMCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-ITEM                       TO    SideMenu-1::SelectedItemId
           *> Conversation rows have the id "c" + conversation id, so they are tested
           *> last: the menu's own "chat" row also starts with "c" and has to be
           *> matched above.

           EVALUATE    WS-ITEM
               WHEN "chat"
                   CONTINUE
               WHEN "newc"
                   CALL    "PC-PICK-TOPIC"
                   CALL    "PC-SHOW-CHAT"
               WHEN OTHER
                   IF    WS-ITEM(1:1) = "c"
                       CALL    "PC-OPEN-CONV"
                       CALL    "PC-SHOW-CHAT"
                   END-IF
           END-EVALUATE
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM SIDEMENU-1--ONMENUITEMCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VWR-CHAT--ONSAVECOMPLETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     Lbl-Status::Caption           TO    T-PDF-SAVED::Trim()

           GOBACK.

       END PROGRAM VWR-CHAT--ONSAVECOMPLETE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VWR-CHAT--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Saving as PDF is the chat's only way to fail: say why - in the
           *>   interface's language when it is only that there is nothing yet.

           SET     WS-LINE                       TO    Vwr-Chat::LastError
           IF    WS-LINE = "the conversation is empty"
               SET     Lbl-Status::Caption       TO    T-PDF-EMPTY::Trim()
           ELSE
               SET     Lbl-Status::Caption       TO    WS-LINE
           END-IF

           GOBACK.

       END PROGRAM VWR-CHAT--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SEND--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-QUESTION                   TO    Txt-Input::Text
           IF    WS-QUESTION = SPACES
               EXIT PROGRAM
           END-IF
           IF    WS-CUR-TOPIC = SPACES
               SET     WS-OK                     TO    Vwr-Chat::AppendMarkdown(T-OPEN-TOPIC-MENU::Trim())
               EXIT PROGRAM
           END-IF
           IF    WS-ORCH = 0
               SET     WS-OK                     TO    Vwr-Chat::AppendMarkdown(T-ASSIGN-MODEL-MENU::Trim())
               EXIT PROGRAM
           END-IF
           *> One question at a time: WS-STAGE leaves IDLE as the question goes out
           *> and comes back only when the answer (or a failure) arrives.

           IF    WS-STAGE NOT = "IDLE"
               EXIT PROGRAM
           END-IF
           *>   The main prompt as it stands now: an edit in the Prompt screen
           *>   reaches the next question, the agents' instructions included.

           CALL    "PC-MAIN-LOAD"
           IF    WS-MAIN NOT = WS-MAIN-ELECTED
               CALL    "PC-ELECT"
           END-IF

           SET     WS-TPL-NAME WS-TPL-SKEL       TO    SPACES
           SET     WS-TPL-DONE                   TO    "N"

           CALL    "PC-COUNT-SOURCES"
           CALL    "PC-START-CONV"

           SET     WS-ROLE                       TO    "U"
           SET     WS-TEXT                       TO    WS-QUESTION

           CALL    "PC-SAVE-TURN"

           SET     WS-MSG                        TO    WS-QUESTION
           SET     WS-OK                         TO    Vwr-Chat::AppendMarkdown(WS-MSG::Trim(), "user")
           SET     Txt-Input::Text               TO    SPACES

           *> WS-HISTORY keeps the history alone: the PLAN section below is appended
           *> to WS-PROMPT, while the compose and template stages rebuild their own
           *> prompts from WS-HISTORY.

           CALL    "PC-BUILD-PROMPT"
           SET     WS-HISTORY                    TO    WS-PROMPT
           IF    WS-N-AGENTS = 1
               *>       One agent answers by itself (R52).

               SET     WS-STAGE                  TO    "SINGLE"
           ELSE
               *>       Several: the orchestrator plans first (R58).

               SET     WS-STAGE                  TO    "PLAN"
               *>       The main prompt's PLAN section says how.

               SET     WS-SEC-NAME               TO    "PLAN"
               CALL    "PC-SECTION"
               STRING    X"0A" X"0A" WS-SEC::Trim() INTO WS-PROMPT WITH POINTER WS-PTR
               END-STRING
           END-IF
           SET     WS-FROM                       TO    WS-ORCH
           CALL    "PC-ASK"

           SET     Lbl-Status::Caption           TO    T-THINKING::Trim()
           SET     WS-STATUS-ID                  TO    SPACES

           IF    WS-STAGE = "PLAN"
               SET     WS-STATUS-TEXT            TO    T-ST-COORD
           ELSE
               SET     WS-STATUS-TEXT            TO    T-THINKING
           END-IF
           CALL    "PC-SAY-STATUS"

           Txt-Input::SetFocus()

           GOBACK.

       END PROGRAM BTN-SEND--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PDF--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The conversation as a PDF, with its formatting; the platform's
           *>   own Save panel asks where (the Viewer's SaveAsPdf).

           SET     WS-OK                         TO    Vwr-Chat::SaveAsPdf()

           GOBACK.

       END PROGRAM BTN-PDF--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-1--ONRESPONSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-FROM                       TO    1
           SET     WS-REPLY                      TO    AGENT-1::LastReply
           SET     WS-T-IN                       TO    AGENT-1::LastInputTokens
           SET     WS-T-OUT                      TO    AGENT-1::LastOutputTokens

           CALL    "PC-ON-REPLY"

           GOBACK.

       END PROGRAM AGENT-1--ONRESPONSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-1--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(1) = "Y"
               SET     WS-FROM                   TO    1
               SET     WS-TEXT                   TO    AGENT-1::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-1::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "1"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-1--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-1--ONTIMEOUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The model never answered within TimeoutSeconds, and the runtime
           *>   cancelled the call: the same as a failure - the chat goes back to
           *>   idle and says why (LastError). Unhandled, the chat stayed stuck
           *>   waiting for an answer that was never coming (operator,
           *>   2026-09-28: "the interface froze").
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(1) = "Y"
               SET     WS-FROM                   TO    1
               SET     WS-TEXT                   TO    AGENT-1::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-1::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "1"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-1--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-1--ONMODELCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A model an agent uses changed: a new election (R36; 063 R71).

           CALL    "PC-HAND-MODELS"
           CALL    "PC-ELECT"
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM AGENT-1--ONMODELCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-1--ONTOOLUSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The agent is searching the Knowledge Base or a data file: the
           *>   status bubble says so while it happens.

           SET     WS-TOOL-KIND                  TO    AGENT-1::ToolKind
           CALL    "PC-TOOL-USE"

           GOBACK.

       END PROGRAM AGENT-1--ONTOOLUSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-2--ONRESPONSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-FROM                       TO    2
           SET     WS-REPLY                      TO    AGENT-2::LastReply
           SET     WS-T-IN                       TO    AGENT-2::LastInputTokens
           SET     WS-T-OUT                      TO    AGENT-2::LastOutputTokens

           CALL    "PC-ON-REPLY"

           GOBACK.

       END PROGRAM AGENT-2--ONRESPONSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-2--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(2) = "Y"
               SET     WS-FROM                   TO    2
               SET     WS-TEXT                   TO    AGENT-2::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-2::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "2"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-2--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-2--ONTIMEOUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The model never answered within TimeoutSeconds, and the runtime
           *>   cancelled the call: the same as a failure - the chat goes back to
           *>   idle and says why (LastError). Unhandled, the chat stayed stuck
           *>   waiting for an answer that was never coming (operator,
           *>   2026-09-28: "the interface froze").
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(2) = "Y"
               SET     WS-FROM                   TO    2
               SET     WS-TEXT                   TO    AGENT-2::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-2::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "2"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-2--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-2--ONMODELCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A model an agent uses changed: a new election (R36; 063 R71).

           CALL    "PC-HAND-MODELS"
           CALL    "PC-ELECT"
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM AGENT-2--ONMODELCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-2--ONTOOLUSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The agent is searching the Knowledge Base or a data file: the
           *>   status bubble says so while it happens.

           SET     WS-TOOL-KIND                  TO    AGENT-2::ToolKind
           CALL    "PC-TOOL-USE"

           GOBACK.

       END PROGRAM AGENT-2--ONTOOLUSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-3--ONRESPONSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-FROM                       TO    3
           SET     WS-REPLY                      TO    AGENT-3::LastReply
           SET     WS-T-IN                       TO    AGENT-3::LastInputTokens
           SET     WS-T-OUT                      TO    AGENT-3::LastOutputTokens

           CALL    "PC-ON-REPLY"

           GOBACK.

       END PROGRAM AGENT-3--ONRESPONSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-3--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(3) = "Y"
               SET     WS-FROM                   TO    3
               SET     WS-TEXT                   TO    AGENT-3::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-3::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "3"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-3--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-3--ONTIMEOUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The model never answered within TimeoutSeconds, and the runtime
           *>   cancelled the call: the same as a failure - the chat goes back to
           *>   idle and says why (LastError). Unhandled, the chat stayed stuck
           *>   waiting for an answer that was never coming (operator,
           *>   2026-09-28: "the interface froze").
           *>   While the assistants work, a failure fails that task only: the
           *>   others go on, and the answer says which part is missing.

           IF    WS-STAGE = "WORK" AND WS-AG-WAIT(3) = "Y"
               SET     WS-FROM                   TO    3
               SET     WS-TEXT                   TO    AGENT-3::LastError
               SET     WS-TASK-OK                TO    "N"

               CALL    "PC-TASK-DONE"
               EXIT PROGRAM
           END-IF

           SET     WS-STAGE                      TO    "IDLE"
           SET     WS-TEXT                       TO    AGENT-3::LastError
           SET     WS-FMT                        TO    T-AGENT-FAILED
           SET     WS-ARG1                       TO    "3"

           CALL    "PC-FMT"
           SET     WS-MSG                        TO    SPACES
           STRING    "**" WS-FMT-OUT::Trim() "** " WS-TEXT::Trim() INTO WS-MSG
           CALL    "PC-SHOW-AGENT"
           SET     Lbl-Status::Caption           TO    T-LAST-FAILED::Trim()

           GOBACK.

       END PROGRAM AGENT-3--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-3--ONMODELCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A model an agent uses changed: a new election (R36; 063 R71).

           CALL    "PC-HAND-MODELS"
           CALL    "PC-ELECT"
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM AGENT-3--ONMODELCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-3--ONTOOLUSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The agent is searching the Knowledge Base or a data file: the
           *>   status bubble says so while it happens.

           SET     WS-TOOL-KIND                  TO    AGENT-3::ToolKind
           CALL    "PC-TOOL-USE"

           GOBACK.

       END PROGRAM AGENT-3--ONTOOLUSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-EN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "en"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-EN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-PT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "pt"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-PT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-ES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "es"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-ES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-FR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "fr"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-FR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-JP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "jp"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-JP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLAG-CN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the menu's footer: the language changes at once (R46).

           SET     WS-SET-NAME                   TO    "LANG"
           SET     WS-SET-VALUE                  TO    "cn"

           CALL    "PC-SETTING-PUT"
           CALL    "PC-RELABEL"

           GOBACK.

       END PROGRAM FLAG-CN--ONCLICK.

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
           SET     WS-CONVS-PATH                 TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/convs.idx" INTO WS-CONVS-PATH
           SET     WS-TURNS-PATH                 TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/turns.idx" INTO WS-TURNS-PATH
           SET     WS-TR-PATH                    TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/taskresults.idx" INTO WS-TR-PATH
           SET     WS-MODELS-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/models.idx" INTO WS-MODELS-PATH
           SET     WS-TFILES-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/topic-files.idx" INTO WS-TFILES-PATH
           SET     WS-RTPL-PATH                  TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/templates.idx" INTO WS-RTPL-PATH
           SET     WS-MAINP-PATH                 TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/prompt-versions.idx" INTO WS-MAINP-PATH
           *>   What ships beside the sample topics - the templates and the main
           *>   prompt's default: POWERCHAT_SAMPLES, else "samples".

           DISPLAY    "POWERCHAT_SAMPLES" UPON ENVIRONMENT-NAME
           ACCEPT    WS-SAMPLES-DIR FROM ENVIRONMENT-VALUE
           IF    WS-SAMPLES-DIR = SPACES
               SET     WS-SAMPLES-DIR            TO    "samples"
           END-IF
           SET     WS-RTSEED-PATH                TO    SPACES
           STRING    WS-SAMPLES-DIR::Trim() "/report-templates.txt" INTO WS-RTSEED-PATH

           *> WS-RTSEED-PATH is pointed at the previous file and back during the
           *> upgrade (PC-UPGRADE-TEMPLATES); WS-RTNEW-PATH keeps the shipped one.

           SET     WS-RTNEW-PATH                 TO    WS-RTSEED-PATH
           SET     WS-RTPREV-PATH                TO    SPACES

           STRING    WS-SAMPLES-DIR::Trim() "/report-templates-previous.txt" INTO WS-RTPREV-PATH
           SET     WS-MPSEED-PATH                TO    SPACES
           STRING    WS-SAMPLES-DIR::Trim() "/main-prompt.md" INTO WS-MPSEED-PATH

           GOBACK.

       END PROGRAM PC-PATHS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SYNC-MODELS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 SY-N                                   PIC    9(3).
       01 SY-I                                   PIC    9(3).
       01 SY-J                                   PIC    9(3).
       01 SY-NAME                                PIC    X(30).
       01 SY-API                                 PIC    X(12).
       01 SY-URL                                 PIC    X(200).
       01 SY-MODEL                               PIC    X(80).
       01 SY-FOUND                               PIC    X.
       01 SY-EOF                                 PIC    X.
       01 SY-GONE-N                              PIC    9(3).
       01 SY-GONE-TABLE.
           05 SY-GONE                            PIC    X(30)    OCCURS 200 TIMES.

       PROCEDURE DIVISION.
           *>   The models are the APPLICATION's (spec 085): the runtime keeps the
           *>   list for every form of it, the application's own agents included.
           *>   MODELS mirrors that list and adds what only PowerChat knows of each
           *>   model - whether it calls tools, how well it orchestrates. Once, the
           *>   models PowerChat kept before are handed to the application's list.

           OPEN    I-O MODELS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT MODELS-FILE
               CLOSE    MODELS-FILE
               OPEN    I-O MODELS-FILE
           END-IF
           SET     WS-SET-NAME                   TO    "MODELS-ADOPTED"
           CALL    "PC-SETTING-GET"
           IF    WS-SET-VALUE NOT = "Y"
               SET     MDL-NAME                  TO    LOW-VALUES
               SET     SY-EOF                    TO    "N"

               START    MODELS-FILE KEY IS >= MDL-NAME
                   INVALID KEY MOVE "Y" TO SY-EOF
               END-START
               PERFORM    UNTIL SY-EOF = "Y"
                   READ    MODELS-FILE NEXT RECORD
                       AT END MOVE "Y" TO SY-EOF
                       NOT AT END
                           COBOL::"MODEL-SET" ( MDL-NAME MDL-API MDL-URL MDL-MODEL )
                   END-READ
               END-PERFORM

               SET     WS-SET-NAME               TO    "MODELS-ADOPTED"
               SET     WS-SET-VALUE              TO    "Y"

               CALL    "PC-SETTING-PUT"
           END-IF
           *>   Every model of the application's is here, with its own metadata.

           COBOL::"MODEL-COUNT" ( SY-N )
           PERFORM    VARYING SY-I FROM 1 BY 1 UNTIL SY-I > SY-N
               SET     SY-NAME SY-API SY-URL SY-MODEL
                                                 TO    SPACES

               COBOL::"MODEL-GET" ( SY-I SY-NAME SY-API SY-URL SY-MODEL )
               SET     MDL-NAME                  TO    SY-NAME
               READ    MODELS-FILE
                   INVALID KEY
                       *> A model new to PowerChat starts without tool support and at the
                       *> middle rank, 5 (the scale is 1-9); Model selection sets both.

                       SET     MDL-API           TO    SY-API
                       SET     MDL-URL           TO    SY-URL
                       SET     MDL-MODEL         TO    SY-MODEL
                       SET     MDL-TOOLS         TO    "N"
                       SET     MDL-RANK          TO    5

                       WRITE    MODEL-REC
                           INVALID KEY CONTINUE
                       END-WRITE
                   NOT INVALID KEY
                       IF    MDL-API NOT = SY-API OR MDL-URL NOT = SY-URL
                               OR MDL-MODEL NOT = SY-MODEL

                           SET     MDL-API       TO    SY-API
                           SET     MDL-URL       TO    SY-URL
                           SET     MDL-MODEL     TO    SY-MODEL

                           REWRITE    MODEL-REC
                               INVALID KEY CONTINUE
                           END-REWRITE
                       END-IF
               END-READ
           END-PERFORM
           *>   ...and none the application has withdrawn.

           SET     SY-GONE-N                     TO    0
           SET     MDL-NAME                      TO    LOW-VALUES
           SET     SY-EOF                        TO    "N"

           START    MODELS-FILE KEY IS >= MDL-NAME
               INVALID KEY MOVE "Y" TO SY-EOF
           END-START
           PERFORM    UNTIL SY-EOF = "Y"
               READ    MODELS-FILE NEXT RECORD
                   AT END MOVE "Y" TO SY-EOF
                   NOT AT END
                       SET     SY-FOUND          TO    "N"
                       PERFORM    VARYING SY-J FROM 1 BY 1
                               UNTIL SY-J > SY-N OR SY-FOUND = "Y"
                           SET     SY-NAME       TO    SPACES
                           COBOL::"MODEL-GET" ( SY-J SY-NAME )
                           IF    FUNCTION UPPER-CASE(SY-NAME)
                                   = FUNCTION UPPER-CASE(MDL-NAME)
                               SET     SY-FOUND  TO    "Y"
                           END-IF
                       END-PERFORM
                       IF    SY-FOUND = "N" AND SY-GONE-N < 200
                           ADD    1 TO SY-GONE-N
                           SET     SY-GONE(SY-GONE-N)
                                                 TO    MDL-NAME
                       END-IF
               END-READ
           END-PERFORM
           *> Withdrawn models are deleted after the scan, from the names noted in
           *> SY-GONE: no DELETE happens in the middle of the READ NEXT loop.

           PERFORM    VARYING SY-J FROM 1 BY 1 UNTIL SY-J > SY-GONE-N
               SET     MDL-NAME                  TO    SY-GONE(SY-J)
               DELETE    MODELS-FILE
                   INVALID KEY CONTINUE
               END-DELETE
           END-PERFORM
           COMMIT
           CLOSE    MODELS-FILE

           GOBACK.

       END PROGRAM PC-SYNC-MODELS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-GET IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-SET-NAME in, WS-SET-VALUE out (spaces when the setting is unset).

           SET     WS-SET-VALUE                  TO    SPACES
           *> Status 35 means the file does not exist yet: create it empty, then
           *> open it I-O again, so a first run needs no setup.

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
       PROGRAM-ID. PC-HAND-MODELS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The models are the application's, kept by the runtime for every form
           *>   of it (spec 085); MODELS mirrors them with PowerChat's own metadata.

           CALL    "PC-SYNC-MODELS"
           OPEN    I-O MODELS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT MODELS-FILE
               CLOSE    MODELS-FILE
               OPEN    I-O MODELS-FILE
           END-IF
           *>   Each agent's entry, and what the entry says the model can do.

           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               SET     WS-SET-NAME               TO    SPACES
               STRING    "AGENT-" WS-A "-ENTRY" DELIMITED BY SIZE INTO WS-SET-NAME
               CALL    "PC-SETTING-GET"
               *> Agent 1 falls back to MODEL-ENTRY, the single setting PowerChat 1
               *> kept, when it has no AGENT-1-ENTRY of its own.

               IF    WS-A = 1 AND WS-SET-VALUE = SPACES
                   SET     WS-SET-NAME           TO    "MODEL-ENTRY"
                   CALL    "PC-SETTING-GET"
               END-IF

               SET     WS-AG-ENTRY(WS-A)         TO    WS-SET-VALUE
               SET     WS-AG-MODEL(WS-A)         TO    SPACES
               SET     WS-AG-TOOLS(WS-A)         TO    "N"
               SET     WS-AG-RANK(WS-A)          TO    0

               *> An entry whose connection is no longer in MODELS is blanked, so that
               *> agent counts as unassigned: PC-ELECT skips blank entries.

               IF    WS-AG-ENTRY(WS-A) NOT = SPACES
                   SET     MDL-NAME              TO    WS-AG-ENTRY(WS-A)
                   READ    MODELS-FILE
                       INVALID KEY MOVE SPACES TO WS-AG-ENTRY(WS-A)
                       NOT INVALID KEY
                           SET     WS-AG-MODEL(WS-A)
                                                 TO    MDL-MODEL

                           SET     WS-AG-TOOLS(WS-A)
                                                 TO    MDL-TOOLS

                           IF    MDL-RANK IS NUMERIC
                               SET     WS-AG-RANK(WS-A)
                                                 TO    MDL-RANK
                           END-IF
                   END-READ
               END-IF
               SET     WS-FROM                   TO    WS-A
               EVALUATE    WS-FROM
                   WHEN 1
                       SET     AGENT-1::ModelEntry
                                                 TO    WS-AG-ENTRY(1)
                   WHEN 2
                       SET     AGENT-2::ModelEntry
                                                 TO    WS-AG-ENTRY(2)
                   WHEN 3
                       SET     AGENT-3::ModelEntry
                                                 TO    WS-AG-ENTRY(3)
               END-EVALUATE
           END-PERFORM
           CLOSE    MODELS-FILE
           SET     WS-SET-NAME                   TO    "KB-LOCATION"
           CALL    "PC-SETTING-GET"
           IF    WS-SET-VALUE NOT = SPACES
               SET     KB-1::Location            TO    WS-SET-VALUE
           END-IF

           GOBACK.

       END PROGRAM PC-HAND-MODELS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-READ-ROLES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The role prompt designed on each agent (R64), kept before the
           *>   election gives the orchestrator the topic's prompt instead (R63).

           IF    WS-ROLES-SEEN = "Y"
               EXIT PROGRAM
           END-IF

           *> Only the first call may read them: later the agents hold the elected
           *> prompts, which are not the roles that were designed.

           SET     WS-ROLES-SEEN                 TO    "Y"
           SET     WS-AG-ROLE(1)                 TO    AGENT-1::SystemPrompt
           SET     WS-AG-ROLE(2)                 TO    AGENT-2::SystemPrompt
           SET     WS-AG-ROLE(3)                 TO    AGENT-3::SystemPrompt

           GOBACK.

       END PROGRAM PC-READ-ROLES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ELECT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Who orchestrates and who does the tool work (063 R52-R57, R66).
           *>   Run once per session, and again when a model changes (R70, R71).

           SET     WS-N-AGENTS WS-N-TOOLS WS-ORCH WS-WORKER
                                                 TO    0

           SET     WS-SAME-MODEL                 TO    "Y"
           *> The best-ranked agent orchestrates, the first one on a tie. Also
           *> counted: assigned agents, tool-capable ones, and whether all of them
           *> share one model.

           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               IF    WS-AG-ENTRY(WS-A) NOT = SPACES
                   ADD    1 TO WS-N-AGENTS
                   IF    WS-AG-TOOLS(WS-A) = "Y"
                       ADD    1 TO WS-N-TOOLS
                   END-IF
                   IF    WS-ORCH = 0
                       SET     WS-ORCH           TO    WS-A
                   ELSE
                       IF    WS-AG-MODEL(WS-A) NOT = WS-AG-MODEL(WS-ORCH)
                           SET     WS-SAME-MODEL TO    "N"
                       END-IF
                       IF    WS-AG-RANK(WS-A) > WS-AG-RANK(WS-ORCH)
                           SET     WS-ORCH       TO    WS-A
                       END-IF
                   END-IF
               END-IF
           END-PERFORM
           EVALUATE    TRUE
               WHEN WS-N-AGENTS = 0
                   SET     Lbl-Status::Caption   TO    T-NO-MODEL::Trim()
               WHEN WS-N-AGENTS = 1
                   *>           One agent answers by itself (R52).

                   IF    WS-AG-TOOLS(WS-ORCH) = "Y"
                       SET     WS-WORKER         TO    WS-ORCH
                   END-IF
               WHEN OTHER
                   *>           Every agent on the same model: any of them, at random (R57).

                   IF    WS-SAME-MODEL = "Y"
                       *> RANDOM is below 1, so WS-B runs from 1 to the number of agents: the
                       *> WS-B-th assigned agent, counted in WS-A past any empty slot.

                       SET     WS-RANDOM         TO    FUNCTION RANDOM
                       COMPUTE    WS-B = WS-RANDOM * WS-N-AGENTS + 1
                       SET     WS-A              TO    0
                       PERFORM    VARYING WS-BEST FROM 1 BY 1 UNTIL WS-BEST > 3
                           IF    WS-AG-ENTRY(WS-BEST) NOT = SPACES
                               ADD    1 TO WS-A
                               IF    WS-A = WS-B
                                   SET     WS-ORCH
                                                 TO    WS-BEST
                               END-IF
                           END-IF
                       END-PERFORM
                   END-IF
                   *>           The only tool-capable model does the tool work; the next
                   *>           best orchestrates (R55, R56).

                   IF    WS-N-TOOLS = 1 AND WS-AG-TOOLS(WS-ORCH) = "Y"
                       SET     WS-WORKER         TO    WS-ORCH
                       SET     WS-ORCH           TO    0

                       PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
                           IF    WS-AG-ENTRY(WS-A) NOT = SPACES AND WS-A NOT = WS-WORKER
                               IF    WS-ORCH = 0
                                   SET     WS-ORCH
                                                 TO    WS-A
                               ELSE
                                   IF    WS-AG-RANK(WS-A) > WS-AG-RANK(WS-ORCH)
                                       SET     WS-ORCH
                                                 TO    WS-A
                                   END-IF
                               END-IF
                           END-IF
                       END-PERFORM
                   ELSE
                       *>               The best-ranked tool-capable agent that is not orchestrating.

                       PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
                           IF    WS-AG-ENTRY(WS-A) NOT = SPACES AND WS-A NOT = WS-ORCH
                                   AND WS-AG-TOOLS(WS-A) = "Y"
                               IF    WS-WORKER = 0
                                   SET     WS-WORKER
                                                 TO    WS-A
                               ELSE
                                   IF    WS-AG-RANK(WS-A) > WS-AG-RANK(WS-WORKER)
                                       SET     WS-WORKER
                                                 TO    WS-A
                                   END-IF
                               END-IF
                           END-IF
                       END-PERFORM
                   END-IF
           END-EVALUATE
           *>   Prompts: the topic's to the orchestrator only (R63); the others
           *>   keep their role prompts (R64). The Knowledge Base goes to the tool
           *>   worker alone; with none, the chat runs without tools (R66).

           CALL    "PC-MAIN-LOAD"
           *> The main prompt these instructions were built from: each question
           *> loads it again and elects anew when it differs, i.e. after an edit.

           SET     WS-MAIN-ELECTED               TO    WS-MAIN
           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               SET     WS-FROM                   TO    WS-A
               IF    WS-A = WS-ORCH
                   *>           Every instruction is the main prompt's (operator,
                   *>           2026-09-28): its SYSTEM section, with the topic's prompt,
                   *>           the NO SOURCES section while the topic has no documents
                   *>           and no data files, and the template list, each where the
                   *>           text puts it.

                   SET     WS-NOTE               TO    SPACES
                   IF    WS-SOURCES = 0 AND WS-CUR-TOPIC NOT = SPACES
                       SET     WS-SEC-NAME       TO    "NO SOURCES"
                       CALL    "PC-SECTION"

                       SET     WS-TOKEN          TO    "{DOCUMENTS MENU}"
                       SET     WS-INS            TO    T-DOCS-TITLE

                       CALL    "PC-FILL"

                       SET     WS-TOKEN          TO    "{DATA FILES MENU}"
                       SET     WS-INS            TO    T-FILES-TITLE

                       CALL    "PC-FILL"
                       SET     WS-NOTE           TO    WS-SEC
                   END-IF
                   CALL    "PC-TEMPLATES"
                   SET     WS-SEC-NAME           TO    "SYSTEM"
                   CALL    "PC-SECTION"

                   SET     WS-TOKEN              TO    "{TOPIC}"
                   SET     WS-INS                TO    WS-TOPIC-PROMPT

                   CALL    "PC-FILL"

                   SET     WS-TOKEN              TO    "{SOURCES NOTE}"
                   SET     WS-INS                TO    WS-NOTE

                   CALL    "PC-FILL"

                   SET     WS-TOKEN              TO    "{TEMPLATES}"
                   SET     WS-INS                TO    WS-CATALOG

                   CALL    "PC-FILL"
                   SET     WS-TEXT               TO    WS-SEC
                   *>           No main prompt at all - no version and no shipped file -
                   *>           or one without a SYSTEM section: the topic's own prompt,
                   *>           which the user wrote, rather than no instructions.

                   IF    WS-TEXT = SPACES
                       SET     WS-TEXT           TO    WS-TOPIC-PROMPT
                   END-IF
               ELSE
                   *>           The assistants: the main prompt's ASSISTANT section.

                   SET     WS-SEC-NAME           TO    "ASSISTANT"
                   CALL    "PC-SECTION"
                   SET     WS-TEXT               TO    WS-SEC
               END-IF
               EVALUATE    WS-FROM
                   WHEN 1
                       SET     AGENT-1::SystemPrompt
                                                 TO    WS-TEXT

                       *>           Tools only for the tool worker; the others get none (R66).

                       IF    1 = WS-WORKER
                           SET     AGENT-1::ToolProtocol
                                                 TO    "Native"
                       ELSE
                           SET     AGENT-1::ToolProtocol
                                                 TO    "None"
                       END-IF
                       *> Withdraw the topic allowed at the last election from this agent, then
                       *> grant the current one to the tool worker alone: a grant is per agent,
                       *> so no agent keeps a former topic's documents (R7).

                       IF    WS-ALLOWED-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-1::DenyKnowledgeBase("KB-1", WS-ALLOWED-TOPIC)
                       END-IF
                       IF    1 = WS-WORKER AND WS-CUR-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-1::AllowKnowledgeBase("KB-1", WS-CUR-TOPIC)
                       END-IF
                   WHEN 2
                       SET     AGENT-2::SystemPrompt
                                                 TO    WS-TEXT

                       *>           Tools only for the tool worker; the others get none (R66).

                       IF    2 = WS-WORKER
                           SET     AGENT-2::ToolProtocol
                                                 TO    "Native"
                       ELSE
                           SET     AGENT-2::ToolProtocol
                                                 TO    "None"
                       END-IF
                       IF    WS-ALLOWED-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-2::DenyKnowledgeBase("KB-1", WS-ALLOWED-TOPIC)
                       END-IF
                       IF    2 = WS-WORKER AND WS-CUR-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-2::AllowKnowledgeBase("KB-1", WS-CUR-TOPIC)
                       END-IF
                   WHEN 3
                       SET     AGENT-3::SystemPrompt
                                                 TO    WS-TEXT

                       *>           Tools only for the tool worker; the others get none (R66).

                       IF    3 = WS-WORKER
                           SET     AGENT-3::ToolProtocol
                                                 TO    "Native"
                       ELSE
                           SET     AGENT-3::ToolProtocol
                                                 TO    "None"
                       END-IF
                       IF    WS-ALLOWED-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-3::DenyKnowledgeBase("KB-1", WS-ALLOWED-TOPIC)
                       END-IF
                       IF    3 = WS-WORKER AND WS-CUR-TOPIC NOT = SPACES
                           SET     WS-OK         TO    AGENT-3::AllowKnowledgeBase("KB-1", WS-CUR-TOPIC)
                       END-IF
               END-EVALUATE
           END-PERFORM

           SET     WS-ALLOWED-TOPIC              TO    WS-CUR-TOPIC
           SET     WS-STAGE                      TO    "IDLE"

           GOBACK.

       END PROGRAM PC-ELECT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MAIN-LOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-MAIN out: the main prompt - the active version the Prompt
           *>   screen keeps under the topic id "*MAIN", or, before there is one,
           *>   the shipped samples/main-prompt.md.

           SET     WS-MAIN                       TO    SPACES
           OPEN    INPUT MAINP-FILE
           IF    WS-FS = "00"
               *> Keyed by topic, then version: START at *MAIN version 0 and read on
               *> until the topic changes. The last version flagged active wins.

               SET     PRM-TOPIC                 TO    "*MAIN"
               SET     PRM-VERSION               TO    0
               SET     WS-EOF                    TO    "N"

               START    MAINP-FILE KEY IS >= PRM-KEY
                   INVALID KEY MOVE "Y" TO WS-EOF
               END-START
               PERFORM    UNTIL WS-EOF = "Y"
                   READ    MAINP-FILE NEXT RECORD
                       AT END MOVE "Y" TO WS-EOF
                       NOT AT END
                           IF    PRM-TOPIC NOT = "*MAIN"
                               SET     WS-EOF    TO    "Y"
                           ELSE
                               IF    PRM-ACTIVE = "Y"
                                   SET     WS-MAIN
                                                 TO    PRM-TEXT
                               END-IF
                           END-IF
                   END-READ
               END-PERFORM
               CLOSE    MAINP-FILE
           END-IF
           *> No saved active version: WS-MAIN is built from the shipped file, a
           *> line feed after each line, trailing blanks cut, blank lines kept.

           IF    WS-MAIN = SPACES
               SET     WS-SX-B                   TO    1
               OPEN    INPUT MPSEED-FILE
               IF    WS-FS = "00"
                   SET     WS-EOF                TO    "N"
                   PERFORM    UNTIL WS-EOF = "Y"
                       READ    MPSEED-FILE
                           AT END MOVE "Y" TO WS-EOF
                           NOT AT END
                               SET     WS-LN-BUF TO    MPSEED-REC
                               CALL    "PC-LINE-LEN"
                               IF    WS-LN-LEN > 0
                                   STRING    WS-LN-BUF(1:WS-LN-LEN) X"0A" INTO WS-MAIN
                                       WITH POINTER WS-SX-B
                                   END-STRING
                               ELSE
                                   STRING    X"0A" INTO WS-MAIN
                                       WITH POINTER WS-SX-B
                                   END-STRING
                               END-IF
                       END-READ
                   END-PERFORM
                   CLOSE    MPSEED-FILE
               END-IF
           END-IF
           SET     WS-EOF                        TO    "N"

           GOBACK.

       END PROGRAM PC-MAIN-LOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SECTION IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-SEC-NAME in: WS-SEC out - that section of the main prompt, from
           *>   the line after its "=== NAME ===" to the next "=== " line or the
           *>   end. A section the text does not have is empty: nothing is sent.

           SET     WS-SEC WS-SPL                 TO    SPACES
           STRING    X"0A" "=== " WS-SEC-NAME::Trim() " ===" INTO WS-SPL
           END-STRING
           *> WS-SX-L is the header's length with its line feed: "=== " (4), the
           *> name, " ===" (4) and the line feed (1).

           COMPUTE    WS-SX-L = FUNCTION LENGTH(WS-SEC-NAME::Trim()) + 9
           *>   WS-SX-A: where the header's "===" starts - the very first line
           *>   counts too (a text whose first section has nothing above it).

           IF    WS-MAIN(1:WS-SX-L - 1) = WS-SPL(2:WS-SX-L - 1)
               SET     WS-SX-A                   TO    1
           ELSE
               SET     WS-SX-A                   TO    0
               *> INSPECT counts the whole field when it finds nothing, so 32000,
               *> WS-MAIN's size, means the text has no such section.

               INSPECT    WS-MAIN TALLYING WS-SX-A
                   FOR CHARACTERS BEFORE INITIAL WS-SPL(1:WS-SX-L)
               IF    WS-SX-A NOT < 32000
                   SET     WS-SPL                TO    SPACES
                   EXIT PROGRAM
               END-IF
               ADD    2 TO WS-SX-A
           END-IF
           *>   Past the header line.

           COMPUTE    WS-SX-A = WS-SX-A + WS-SX-L - 1
           SET     WS-SX-B                       TO    0
           INSPECT    WS-MAIN(WS-SX-A:) TALLYING WS-SX-B
               FOR CHARACTERS BEFORE INITIAL X"0A"
           COMPUTE    WS-SX-A = WS-SX-A + WS-SX-B + 1
           SET     WS-SPL                        TO    SPACES
           IF    WS-SX-A > 32000
               EXIT PROGRAM
           END-IF
           SET     WS-SX-B                       TO    0
           INSPECT    WS-MAIN(WS-SX-A:) TALLYING WS-SX-B
               FOR CHARACTERS BEFORE INITIAL WS-SEC-MARK
           IF    WS-SX-B > 0
               SET     WS-SEC                    TO    WS-MAIN(WS-SX-A:WS-SX-B)
           END-IF

           GOBACK.

       END PROGRAM PC-SECTION.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FILL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-TOKEN and WS-INS in: every WS-TOKEN in WS-SEC - "{TOPIC}" and
           *>   the like - replaced by WS-INS (nothing, when it is spaces).

           COMPUTE    WS-SX-L = FUNCTION LENGTH(WS-TOKEN::Trim())
           *> One token per pass, at most 20 passes. INSPECT counts the whole field
           *> when no token is left, so 64000 (WS-SEC's size) ends the loop.

           PERFORM    20 TIMES
               SET     WS-SX-A                   TO    0
               INSPECT    WS-SEC TALLYING WS-SX-A
                   FOR CHARACTERS BEFORE INITIAL WS-TOKEN(1:WS-SX-L)
               IF    WS-SX-A NOT < 64000
                   EXIT PERFORM
               END-IF

               SET     WS-SPL                    TO    SPACES
               SET     WS-SX-B                   TO    1

               IF    WS-SX-A > 0
                   STRING    WS-SEC(1:WS-SX-A) INTO WS-SPL WITH POINTER WS-SX-B
                   END-STRING
               END-IF
               IF    WS-INS NOT = SPACES
                   STRING    WS-INS::Trim() INTO WS-SPL WITH POINTER WS-SX-B
                   END-STRING
               END-IF
               COMPUTE    WS-SX-E = WS-SX-A + WS-SX-L + 1
               IF    WS-SX-E NOT > 64000
                   STRING    WS-SEC(WS-SX-E:) INTO WS-SPL WITH POINTER WS-SX-B
                   END-STRING
               END-IF
               SET     WS-SEC                    TO    WS-SPL
           END-PERFORM
           SET     WS-SPL                        TO    SPACES

           GOBACK.

       END PROGRAM PC-FILL.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LINE-LEN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-LN-BUF in: WS-LN-LEN out - its length without the trailing
           *>   spaces, so a line's indentation is kept and its padding is not.

           SET     WS-LN-REV                     TO    FUNCTION REVERSE(WS-LN-BUF)
           SET     WS-LN-LEN                     TO    0

           *> The reversed line's leading spaces are the original's trailing ones;
           *> their count off the 1000-character buffer is the length.

           INSPECT    WS-LN-REV TALLYING WS-LN-LEN FOR LEADING SPACES
           COMPUTE    WS-LN-LEN = 1000 - WS-LN-LEN

           GOBACK.

       END PROGRAM PC-LINE-LEN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TPL-FIND IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-TPL-NAME in (any case): WS-TPL-SKEL out, and WS-TPL-NAME as
           *>   the catalog spells it; spaces when there is no such template.

           SET     WS-TPL-SKEL                   TO    SPACES
           SET     WS-RT-NAME                    TO    FUNCTION UPPER-CASE(WS-TPL-NAME::Trim())

           OPEN    INPUT RTPL-FILE
           IF    WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF

           *> A scan, not a keyed READ: the key is case-sensitive and the name may
           *> come in any case, so each stored name is compared in upper case.

           SET     RT-NAME                       TO    LOW-VALUES
           SET     WS-EOF                        TO    "N"

           START    RTPL-FILE KEY IS >= RT-NAME
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    RTPL-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    FUNCTION UPPER-CASE(FUNCTION TRIM(RT-NAME)) = WS-RT-NAME
                           SET     WS-TPL-NAME   TO    RT-NAME
                           SET     WS-TPL-SKEL   TO    RT-SKEL
                           SET     WS-EOF        TO    "Y"
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    RTPL-FILE
           SET     WS-EOF                        TO    "N"

           GOBACK.

       END PROGRAM PC-TPL-FIND.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEMPLATE-TEXT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-TPL-NAME and WS-TPL-SKEL in: WS-SEC out - the main prompt's
           *>   TEMPLATE section with the chosen template filled in.

           SET     WS-SEC-NAME                   TO    "TEMPLATE"
           CALL    "PC-SECTION"

           SET     WS-TOKEN                      TO    "{NAME}"
           SET     WS-INS                        TO    WS-TPL-NAME

           CALL    "PC-FILL"

           SET     WS-TOKEN                      TO    "{SKELETON}"
           SET     WS-INS                        TO    WS-TPL-SKEL

           CALL    "PC-FILL"

           GOBACK.

       END PROGRAM PC-TEMPLATE-TEXT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TPL-LINE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-REPLY in: when it opens with "TEMPLATE: name" - the user chose
           *>   a report template - WS-TPL-NAME and WS-TPL-SKEL are that template.

           SET     WS-FP                         TO    1
           PERFORM    UNTIL WS-FP > 200
                   OR (WS-REPLY(WS-FP:1) NOT = SPACE
                   AND WS-REPLY(WS-FP:1) NOT = X"0A"
                   AND WS-REPLY(WS-FP:1) NOT = X"0D")
               ADD    1 TO WS-FP
           END-PERFORM
           IF    WS-FP > 200
               EXIT PROGRAM
           END-IF
           IF    FUNCTION UPPER-CASE(WS-REPLY(WS-FP:9)) NOT = "TEMPLATE:"
               EXIT PROGRAM
           END-IF
           COMPUTE    WS-FA = WS-FP + 9
           SET     WS-FB                         TO    0
           INSPECT    WS-REPLY(WS-FA:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL X"0A"
           IF    WS-FB > 80
               SET     WS-FB                     TO    80
           END-IF
           IF    WS-FB > 0
               SET     WS-TPL-NAME               TO    WS-REPLY(WS-FA:WS-FB)
               CALL    "PC-TPL-FIND"
           END-IF

           GOBACK.

       END PROGRAM PC-TPL-LINE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEMPLATES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-CATALOG out: one line per report template - its name and what
           *>   it suits - for the main prompt's {TEMPLATES}. An empty file - the
           *>   first time - is filled from samples/report-templates.txt.

           SET     WS-CATALOG                    TO    SPACES
           SET     WS-SX-B                       TO    1

           OPEN    I-O RTPL-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT RTPL-FILE
               CLOSE    RTPL-FILE
               OPEN    I-O RTPL-FILE
           END-IF
           IF    WS-RT-UPGRADED = "N"
               SET     WS-RT-UPGRADED            TO    "Y"
               CALL    "PC-UPGRADE-TEMPLATES"
           END-IF

           SET     RT-NAME                       TO    LOW-VALUES
           SET     WS-EOF                        TO    "N"

           START    RTPL-FILE KEY IS >= RT-NAME
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           IF    WS-EOF = "Y"
               SET     WS-RT-MODE                TO    "S"
               CALL    "PC-SEED-TEMPLATES"

               SET     RT-NAME                   TO    LOW-VALUES
               SET     WS-EOF                    TO    "N"

               START    RTPL-FILE KEY IS >= RT-NAME
                   INVALID KEY MOVE "Y" TO WS-EOF
               END-START
           END-IF
           PERFORM    UNTIL WS-EOF = "Y"
               READ    RTPL-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       STRING    "- " FUNCTION TRIM(RT-NAME) ": "
                           FUNCTION TRIM(RT-SUITS) X"0A"
                           INTO WS-CATALOG WITH POINTER WS-SX-B
                       END-STRING
               END-READ
           END-PERFORM
           CLOSE    RTPL-FILE

           GOBACK.

       END PROGRAM PC-TEMPLATES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SEED-TEMPLATES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   RTPL-FILE open I-O and empty: the shipped templates written in
           *>   (WS-RT-MODE "S"). The same reading serves the upgrade: "O" keeps
           *>   the previous templates, "U" offers each shipped one to the store.
           *>   "=== name" opens each; its "Suits:" line says what it suits, and
           *>   every other line is its HTML skeleton, indentation kept. A missing
           *>   samples file leaves it empty - the chat still answers.

           OPEN    INPUT RTSEED-FILE
           IF    WS-FS NOT = "00"
               EXIT PROGRAM
           END-IF
           SET     WS-RT-NAME WS-RT-SUITS WS-RT-SKEL
                                                 TO    SPACES

           SET     WS-RT-PT                      TO    1
           SET     WS-EOF                        TO    "N"

           PERFORM    UNTIL WS-EOF = "Y"
               READ    RTSEED-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       EVALUATE    TRUE
                           WHEN RTSEED-REC(1:4) = "=== "
                               CALL    "PC-TAKE-TEMPLATE"
                               SET     WS-RT-NAME
                                                 TO    FUNCTION TRIM(RTSEED-REC(5:))

                               SET     WS-RT-SUITS WS-RT-SKEL
                                                 TO    SPACES

                               SET     WS-RT-PT  TO    1
                           WHEN RTSEED-REC(1:6) = "Suits:"
                               SET     WS-RT-SUITS
                                                 TO    FUNCTION TRIM(RTSEED-REC(7:))
                           WHEN WS-RT-NAME NOT = SPACES
                               SET     WS-LN-BUF TO    RTSEED-REC
                               CALL    "PC-LINE-LEN"
                               IF    WS-LN-LEN > 0
                                   STRING    WS-LN-BUF(1:WS-LN-LEN) X"0A" INTO WS-RT-SKEL
                                       WITH POINTER WS-RT-PT
                                   END-STRING
                               ELSE
                                   STRING    X"0A" INTO WS-RT-SKEL
                                       WITH POINTER WS-RT-PT
                                   END-STRING
                               END-IF
                       END-EVALUATE
               END-READ
           END-PERFORM
           *> The last template has no "=== " line after it to hand it over, so
           *> it is handed over here.

           CALL    "PC-TAKE-TEMPLATE"
           CLOSE    RTSEED-FILE
           SET     WS-EOF                        TO    "N"

           GOBACK.

       END PROGRAM PC-SEED-TEMPLATES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TAKE-TEMPLATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   One template read from a samples file - WS-RT-NAME and
           *>   WS-RT-SKEL - goes where WS-RT-MODE says.

           IF    WS-RT-NAME = SPACES OR WS-RT-SKEL = SPACES
               EXIT PROGRAM
           END-IF
           *> WS-RT-MODE (set by PC-UPGRADE-TEMPLATES): O keeps the previous
           *> shipped versions (up to 40), U offers the shipped one to the store,
           *> S - or any other - writes it.

           EVALUATE    WS-RT-MODE
               WHEN "O"
                   IF    WS-OLD-N < 40
                       ADD    1 TO WS-OLD-N
                       SET     WS-OLD-NAME(WS-OLD-N)
                                                 TO    WS-RT-NAME

                       SET     WS-OLD-SKEL(WS-OLD-N)
                                                 TO    WS-RT-SKEL
                   END-IF
               WHEN "U"
                   CALL    "PC-UPGRADE-ONE"
               WHEN OTHER
                   CALL    "PC-PUT-TEMPLATE"
           END-EVALUATE

           GOBACK.

       END PROGRAM PC-TAKE-TEMPLATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-UPGRADE-TEMPLATES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   RTPL-FILE open I-O: every stored template still exactly as an
           *>   earlier version shipped it takes the shipped template of its
           *>   name. Once per run; without a previous file, nothing changes.

           SET     WS-OLD-N                      TO    0
           SET     WS-RTSEED-PATH                TO    WS-RTPREV-PATH
           SET     WS-RT-MODE                    TO    "O"

           CALL    "PC-SEED-TEMPLATES"
           IF    WS-OLD-N > 0
               SET     WS-RTSEED-PATH            TO    WS-RTNEW-PATH
               SET     WS-RT-MODE                TO    "U"

               CALL    "PC-SEED-TEMPLATES"
           END-IF

           *> Leave the seed path and mode as a plain seeding expects them: the
           *> shipped file, mode S (PC-TEMPLATES seeds an empty store that way).

           SET     WS-RTSEED-PATH                TO    WS-RTNEW-PATH
           SET     WS-RT-MODE                    TO    "S"

           GOBACK.

       END PROGRAM PC-UPGRADE-TEMPLATES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-UPGRADE-ONE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-RT-NAME, WS-RT-SUITS and WS-RT-SKEL in - a shipped template:
           *>   the stored one of that name is replaced only when it equals a
           *>   previous shipped version of it, so a user's change is never lost.

           SET     RT-NAME                       TO    WS-RT-NAME
           READ    RTPL-FILE
               INVALID KEY EXIT PROGRAM
           END-READ
           IF    RT-SKEL = WS-RT-SKEL
               EXIT PROGRAM
           END-IF
           PERFORM    VARYING WS-OLD-I FROM 1 BY 1 UNTIL WS-OLD-I > WS-OLD-N
               IF    WS-OLD-NAME(WS-OLD-I) = WS-RT-NAME
                   IF    WS-OLD-SKEL(WS-OLD-I) = RT-SKEL
                       IF    WS-RT-SUITS NOT = SPACES
                           SET     RT-SUITS      TO    WS-RT-SUITS
                       END-IF
                       SET     RT-SKEL           TO    WS-RT-SKEL
                       REWRITE    RTPL-REC
                       END-REWRITE
                       COMMIT
                       EXIT PROGRAM
                   END-IF
               END-IF
           END-PERFORM

           GOBACK.

       END PROGRAM PC-UPGRADE-ONE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PUT-TEMPLATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-RT-NAME, WS-RT-SUITS and WS-RT-SKEL in, RTPL-FILE open I-O: the
           *>   template written, or rewritten when one of that name exists.

           IF    WS-RT-NAME = SPACES OR WS-RT-SKEL = SPACES
               EXIT PROGRAM
           END-IF
           SET     RT-NAME                       TO    WS-RT-NAME
           READ    RTPL-FILE
               INVALID KEY
                   SET     RT-SUITS              TO    WS-RT-SUITS
                   SET     RT-SKEL               TO    WS-RT-SKEL

                   WRITE    RTPL-REC
                   END-WRITE
               NOT INVALID KEY
                   IF    WS-RT-SUITS NOT = SPACES
                       SET     RT-SUITS          TO    WS-RT-SUITS
                   END-IF
                   SET     RT-SKEL               TO    WS-RT-SKEL
                   REWRITE    RTPL-REC
                   END-REWRITE
           END-READ
           COMMIT

           GOBACK.

       END PROGRAM PC-PUT-TEMPLATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE-TEMPLATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-REPLY in: a template the model defined - the
           *>   "<!--REPORT-TEMPLATE" block, with NAME: and SUITS: - is saved with
           *>   the page of this same answer as its skeleton: a new one added, a
           *>   changed one replaced. The block is cut out of what the user sees
           *>   and of what is kept (operator, 2026-09-28: "always save").

           SET     WS-RT-AT                      TO    0
           INSPECT    WS-REPLY TALLYING WS-RT-AT
               FOR CHARACTERS BEFORE INITIAL "<!--REPORT-TEMPLATE"
           *> INSPECT counts the whole field when the marker is absent, so 64000,
           *> WS-REPLY's size, means this answer defines no template.

           IF    WS-RT-AT NOT < 64000
               EXIT PROGRAM
           END-IF
           ADD    1 TO WS-RT-AT
           COMPUTE    WS-RT-PT = WS-RT-AT + 19
           SET     WS-RT-LEN                     TO    0
           INSPECT    WS-REPLY(WS-RT-PT:) TALLYING WS-RT-LEN
               FOR CHARACTERS BEFORE INITIAL "-->"
           COMPUTE    WS-RT-END = WS-RT-PT + WS-RT-LEN + 3
           IF    WS-RT-END > 64001
               SET     WS-RT-END                 TO    64001
           END-IF
           SET     WS-RT-NAME WS-RT-SUITS WS-RT-SKEL WS-SEC
                                                 TO    SPACES

           IF    WS-RT-LEN > 0
               SET     WS-SEC                    TO    WS-REPLY(WS-RT-PT:WS-RT-LEN)
           END-IF
           *>   NAME: and SUITS: - each to the end of its line.

           SET     WS-FC                         TO    0
           INSPECT    WS-SEC TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "NAME:"
           IF    WS-FC < 64000
               COMPUTE    WS-FA = WS-FC + 6
               SET     WS-FB                     TO    0
               INSPECT    WS-SEC(WS-FA:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL X"0A"
               IF    WS-FB > 0
                   SET     WS-RT-NAME            TO    WS-SEC(WS-FA:WS-FB)::Trim()
               END-IF
           END-IF
           *> The model may write SUITS: or Suits:, and INSPECT is case-sensitive,
           *> so both are tried.

           SET     WS-FC                         TO    0
           INSPECT    WS-SEC TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "SUITS:"
           IF    WS-FC NOT < 64000
               SET     WS-FC                     TO    0
               INSPECT    WS-SEC TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "Suits:"
           END-IF
           IF    WS-FC < 64000
               COMPUTE    WS-FA = WS-FC + 7
               SET     WS-FB                     TO    0
               INSPECT    WS-SEC(WS-FA:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL X"0A"
               IF    WS-FB > 0
                   SET     WS-RT-SUITS           TO    WS-SEC(WS-FA:WS-FB)::Trim()
               END-IF
           END-IF
           *>   The skeleton: this answer's page - a ```html fence's content, or
           *>   a page written straight in, to its </html>.

           SET     WS-FC                         TO    0
           INSPECT    WS-REPLY TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "```html"
           IF    WS-FC < 64000
               COMPUTE    WS-FA = WS-FC + 8
               SET     WS-FB                     TO    0
               INSPECT    WS-REPLY(WS-FA:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL "```"
           ELSE
               SET     WS-FC                     TO    0
               INSPECT    WS-REPLY TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "<!DOCTYPE"
               IF    WS-FC NOT < 64000
                   SET     WS-FC                 TO    0
                   INSPECT    WS-REPLY TALLYING WS-FC FOR CHARACTERS BEFORE INITIAL "<html"
               END-IF
               COMPUTE    WS-FA = WS-FC + 1
               SET     WS-FB                     TO    0
               IF    WS-FC < 64000
                   INSPECT    WS-REPLY(WS-FA:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL "</html>"
                   ADD    7 TO WS-FB
               END-IF
           END-IF
           COMPUTE    WS-SX-E = WS-FA + WS-FB
           IF    WS-FC < 64000 AND WS-FB > 0 AND WS-SX-E NOT > 64001
               SET     WS-RT-SKEL                TO    WS-REPLY(WS-FA:WS-FB)
           END-IF
           IF    WS-RT-NAME NOT = SPACES AND WS-RT-SKEL NOT = SPACES
               OPEN    I-O RTPL-FILE
               IF    WS-FS = "35"
                   OPEN    OUTPUT RTPL-FILE
                   CLOSE    RTPL-FILE
                   OPEN    I-O RTPL-FILE
               END-IF
               CALL    "PC-PUT-TEMPLATE"
               CLOSE    RTPL-FILE
           END-IF
           *>   The block cut out of what the user sees and what is kept.

           SET     WS-TEXT                       TO    SPACES
           IF    WS-RT-AT > 1
               SET     WS-TEXT                   TO    WS-REPLY(1:WS-RT-AT - 1)
           END-IF
           IF    WS-RT-END NOT > 64000
               SET     WS-TEXT(WS-RT-AT:)        TO    WS-REPLY(WS-RT-END:)
           END-IF
           SET     WS-REPLY                      TO    WS-TEXT

           GOBACK.

       END PROGRAM PC-SAVE-TEMPLATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-REGISTER-FILES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The topic's data files, registered for the agents to search. A file
           *>   that cannot be reached or described is named with its reason, and
           *>   the chat answers without it (R22). Registration is program-wide;
           *>   only the tool worker is offered them (ToolProtocol, PC-ELECT).

           PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-REG-COUNT
               SET     WS-OK                     TO    AGENT-1::UnregisterFile(WS-REG-NAME(WS-I)::Trim())
           END-PERFORM

           SET     WS-REG-COUNT WS-FAILED        TO    0
           SET     WS-FILE-NOTE                  TO    SPACES

           IF    WS-CUR-TOPIC = SPACES
               EXIT PROGRAM
           END-IF
           OPEN    I-O TFILES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TFILES-FILE
               CLOSE    TFILES-FILE
               OPEN    I-O TFILES-FILE
           END-IF

           SET     TF-TOPIC                      TO    WS-CUR-TOPIC
           SET     TF-SEQ                        TO    0
           SET     WS-EOF                        TO    "N"

           START    TFILES-FILE KEY IS >= TF-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    TFILES-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    TF-TOPIC NOT = WS-CUR-TOPIC
                           SET     WS-EOF        TO    "Y"
                       ELSE
                           MOVE    AGENT-1::RegisterFile(FUNCTION TRIM(TF-DATA),
                               FUNCTION TRIM(TF-CIDX)) TO WS-OK
                           IF    WS-OK = "1" AND WS-REG-COUNT < 100
                               ADD    1 TO WS-REG-COUNT
                               SET     WS-REG-NAME(WS-REG-COUNT)
                                                 TO    AGENT-1::RegisteredName
                           ELSE
                               ADD    1 TO WS-FAILED
                               STRING    WS-FILE-NOTE::Trim() " " FUNCTION TRIM(TF-NAME)
                                   " (" AGENT-1::RegisterResult::Trim() ")"
                                   INTO WS-FILE-NOTE
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    TFILES-FILE

           GOBACK.

       END PROGRAM PC-REGISTER-FILES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LOAD-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The selected topic decides everything the agents may use: its system
           *>   prompt and its Knowledge Base collection, and nothing else (R7).

           SET     WS-SET-NAME                   TO    "CUR-TOPIC"
           CALL    "PC-SETTING-GET"
           SET     WS-CUR-TOPIC                  TO    WS-SET-VALUE(1:16)
           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Topic::Caption        TO    T-NO-TOPIC::Trim()
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
               INVALID KEY
                   SET     WS-CUR-TOPIC          TO    SPACES
                   SET     Lbl-Topic::Caption    TO    T-TOPIC-GONE::Trim()
           END-READ
           CLOSE    TOPICS-FILE
           IF    WS-CUR-TOPIC = SPACES
               EXIT PROGRAM
           END-IF

           SET     Lbl-Topic::Caption            TO    TOP-NAME
           SET     WS-TOPIC-PROMPT               TO    TOP-PROMPT
           SET     KB-1::Collection              TO    WS-CUR-TOPIC

           CALL    "PC-REGISTER-FILES"
           *>   What the agents can draw on for this topic: its documents and the
           *>   data files that registered. None, and PC-ELECT tells the
           *>   orchestrator to say so rather than guess.

           SET     WS-SOURCES                    TO    KB-1::ListDocuments()
           ADD    WS-REG-COUNT TO WS-SOURCES
           CALL    "PC-ELECT"

           GOBACK.

       END PROGRAM PC-LOAD-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-COUNT-SOURCES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The topic's sources counted again, before each question. They were
           *>   counted only when the topic loaded, so a document uploaded while
           *>   the chat was open left the orchestrator told "this topic has no
           *>   documents - say so", and it did (operator, 2026-09-27: "not
           *>   provided the internal documents", with the policy in the index).
           *>   When the count crosses zero, either way, the agents' instructions
           *>   are set again.

           SET     WS-SOURCES-WAS                TO    WS-SOURCES
           SET     WS-SOURCES                    TO    KB-1::ListDocuments()

           ADD    WS-REG-COUNT TO WS-SOURCES
           IF    WS-SOURCES-WAS = 0
               IF    WS-SOURCES > 0
                   CALL    "PC-ELECT"
               END-IF
           ELSE
               IF    WS-SOURCES = 0
                   CALL    "PC-ELECT"
               END-IF
           END-IF

           GOBACK.

       END PROGRAM PC-COUNT-SOURCES.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ASK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-FROM (1-3) and WS-PROMPT in: that agent's question goes out.

           EVALUATE    WS-FROM
               WHEN 1
                   SET     WS-OK                 TO    AGENT-1::Ask(WS-PROMPT::Trim())
               WHEN 2
                   SET     WS-OK                 TO    AGENT-2::Ask(WS-PROMPT::Trim())
               WHEN 3
                   SET     WS-OK                 TO    AGENT-3::Ask(WS-PROMPT::Trim())
           END-EVALUATE

           GOBACK.

       END PROGRAM PC-ASK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ADD-TOKENS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-T-IN, WS-T-OUT in: one request's tokens, added to the
           *>   conversation - every agent's requests count (R42).

           OPEN    I-O CONVS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT CONVS-FILE
               CLOSE    CONVS-FILE
               OPEN    I-O CONVS-FILE
           END-IF
           SET     CNV-ID                        TO    WS-CUR-CONV
           READ    CONVS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   ADD    WS-T-IN TO CNV-IN-TOKENS
                   ADD    WS-T-OUT TO CNV-OUT-TOKENS

                   SET     CNV-TURNS             TO    WS-SEQ
                   REWRITE    CONV-REC
                   END-REWRITE
                   COMMIT
           END-READ
           CLOSE    CONVS-FILE

           GOBACK.

       END PROGRAM PC-ADD-TOKENS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-IS-FORMATTED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-MSG in: is it a presentation rather than a reply? HTML blocks
           *>   (WS-FMT-HTML too), a Markdown table or a fenced block - or, when
           *>   WS-SHOW-ANSWER says it answers the question in hand, a question
           *>   that asked for a table, Markdown or HTML.

           SET     WS-FORMATTED WS-FMT-HTML      TO    "N"
           SET     WS-FMT-T                      TO    0

           INSPECT    WS-MSG TALLYING WS-FMT-T FOR ALL "<table" ALL "<TABLE"
               ALL "<html" ALL "<HTML" ALL "<div" ALL "<h1" ALL "<h2" ALL "<ul"
           IF    WS-FMT-T > 0
               SET     WS-FORMATTED              TO    "Y"
               *>       HTML only when the answer IS HTML - it starts with a tag. A
               *>       Markdown answer that shows a page inside a fenced block stays
               *>       Markdown, or the HTML reader takes the page apart (operator,
               *>       2026-09-27: "fenced blocks break the conversation").

               PERFORM    VARYING WS-FMT-T FROM 1 BY 1
                       UNTIL WS-FMT-T > 200
                       OR (WS-MSG(WS-FMT-T:1) NOT = SPACE
                       AND WS-MSG(WS-FMT-T:1) NOT = X"0A"
                       AND WS-MSG(WS-FMT-T:1) NOT = X"0D"
                       AND WS-MSG(WS-FMT-T:1) NOT = X"09")
               END-PERFORM
               IF    WS-FMT-T NOT > 200
                   IF    WS-MSG(WS-FMT-T:1) = "<"
                       SET     WS-FMT-HTML       TO    "Y"
                   END-IF
               END-IF
               EXIT PROGRAM
           END-IF
           *> WS-FMT-T is still 0 here - the HTML case above has already left -
           *> so these tallies need no reset.

           INSPECT    WS-MSG TALLYING WS-FMT-T FOR ALL "|---" ALL "| ---"
               ALL "|:--" ALL "```"
           IF    WS-FMT-T > 0
               SET     WS-FORMATTED              TO    "Y"
               EXIT PROGRAM
           END-IF
           IF    WS-SHOW-ANSWER = "Y"
               SET     WS-LOWQ                   TO    FUNCTION LOWER-CASE(WS-QUESTION)
               *>       " table" with its space: "acceptable" and "notable" are not
               *>       asking for one; the same for the CJK words, not a bare 表.

               INSPECT    WS-LOWQ TALLYING WS-FMT-T FOR ALL "tabela" ALL " table"
                   ALL "tabla" ALL "markdown" ALL "html" ALL "表格" ALL "テーブル"
                   ALL "表を"
               IF    WS-FMT-T > 0
                   SET     WS-FORMATTED          TO    "Y"
               END-IF
           END-IF

           GOBACK.

       END PROGRAM PC-IS-FORMATTED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-APPEND-FORMATTED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-MSG in, a presentation (PC-IS-FORMATTED): straight into the
           *>   conversation, no bubble. HTML is rendered as HTML. Markdown is
           *>   rendered as Markdown - except a page inside it, fenced (```html)
           *>   or written straight in (<!DOCTYPE, <html, <style), which is
           *>   rendered as the page it is instead of shown as code (operator,
           *>   2026-09-27 and 09-28). The text around each page stays Markdown,
           *>   in order.
           *>   An answer that starts with a tag is HTML - unless it goes on after
           *>   its page's </html> (a closing fence, an explanation): then the page
           *>   is cut out below like any other, or everything after </html> is
           *>   shown as text (operator, 2026-09-28: "/html>" and "```" leaked).

           IF    WS-FMT-HTML = "Y"
               SET     WS-FB                     TO    0
               INSPECT    WS-MSG TALLYING WS-FB
                   FOR CHARACTERS BEFORE INITIAL "</html>"
               IF    WS-FB NOT < 64100
                   SET     WS-FB                 TO    0
                   INSPECT    WS-MSG TALLYING WS-FB
                       FOR CHARACTERS BEFORE INITIAL "</HTML>"
               END-IF
               *> WS-BS is the first character past </html>. A tally of 64093 or more
               *> means no </html>, or no room after it: the whole answer is the page.

               COMPUTE    WS-BS = WS-FB + 8
               IF    WS-FB NOT < 64093
                   SET     WS-OK                 TO    Vwr-Chat::AppendHtml(WS-MSG::Trim())
                   EXIT PROGRAM
               END-IF
               IF    WS-MSG(WS-BS:) = SPACES
                   SET     WS-OK                 TO    Vwr-Chat::AppendHtml(WS-MSG::Trim())
                   EXIT PROGRAM
               END-IF
           END-IF
           SET     WS-FP                         TO    1
           PERFORM    UNTIL WS-FP > 64100
               *>       The next page in what is left: a ```html fence, or a page
               *>       written straight into the answer - <!DOCTYPE, <html or
               *>       <style. Markdown reads such a page only up to its first blank
               *>       line and shows the indented rest as code (operator,
               *>       2026-09-28: "it showed the generated code"), so it is cut out
               *>       and rendered as HTML here. The nearest one wins.

               COMPUTE    WS-REST = 64100 - WS-FP + 1

               *> WS-FA starts at the length of what is left. INSPECT counts exactly
               *> that when a marker is absent, so only a real hit is smaller.

               SET     WS-FA                     TO    WS-REST
               SET     WS-FKIND                  TO    SPACE
               SET     WS-FC                     TO    0

               INSPECT    WS-MSG(WS-FP:) TALLYING WS-FC
                   FOR CHARACTERS BEFORE INITIAL "```html"
               IF    WS-FC < WS-FA
                   SET     WS-FA                 TO    WS-FC
                   SET     WS-FKIND              TO    "F"
               END-IF
               SET     WS-FC                     TO    0
               INSPECT    WS-MSG(WS-FP:) TALLYING WS-FC
                   FOR CHARACTERS BEFORE INITIAL "<!DOCTYPE"
               IF    WS-FC < WS-FA
                   SET     WS-FA                 TO    WS-FC
                   SET     WS-FKIND              TO    "R"
               END-IF
               SET     WS-FC                     TO    0
               INSPECT    WS-MSG(WS-FP:) TALLYING WS-FC
                   FOR CHARACTERS BEFORE INITIAL "<!doctype"
               IF    WS-FC < WS-FA
                   SET     WS-FA                 TO    WS-FC
                   SET     WS-FKIND              TO    "R"
               END-IF
               SET     WS-FC                     TO    0
               INSPECT    WS-MSG(WS-FP:) TALLYING WS-FC
                   FOR CHARACTERS BEFORE INITIAL "<html"
               IF    WS-FC < WS-FA
                   SET     WS-FA                 TO    WS-FC
                   SET     WS-FKIND              TO    "R"
               END-IF
               SET     WS-FC                     TO    0
               INSPECT    WS-MSG(WS-FP:) TALLYING WS-FC
                   FOR CHARACTERS BEFORE INITIAL "<style"
               IF    WS-FC < WS-FA
                   SET     WS-FA                 TO    WS-FC
                   SET     WS-FKIND              TO    "R"
               END-IF
               *>       No page left: the rest is Markdown.

               IF    WS-FKIND = SPACE
                   IF    WS-MSG(WS-FP:) NOT = SPACES
                       SET     WS-OK             TO    Vwr-Chat::AppendMarkdown(WS-MSG(WS-FP:)::Trim())
                   END-IF
                   EXIT PERFORM
               END-IF
               *>       A page written straight in may still sit inside a fence the
               *>       model opened as "```" or "```HTML": an odd count of "```" in
               *>       the Markdown before it. That opening fence is cut off here and
               *>       its closing one skipped after the page, or Markdown shows them.

               SET     WS-FQ                     TO    WS-FA
               SET     WS-FSTRIP                 TO    "N"

               IF    WS-FKIND = "R" AND WS-FA > 2
                   SET     WS-FC                 TO    0
                   INSPECT    WS-MSG(WS-FP:WS-FA) TALLYING WS-FC FOR ALL "```"
                   IF    FUNCTION MOD(WS-FC, 2) = 1
                       COMPUTE    WS-FQ = WS-FA - 3
                       PERFORM    UNTIL WS-FQ = 0
                               OR WS-MSG(WS-FP + WS-FQ:3) = "```"
                           SUBTRACT    1 FROM WS-FQ
                       END-PERFORM
                       IF    WS-MSG(WS-FP + WS-FQ:3) = "```"
                           SET     WS-FSTRIP     TO    "Y"
                       ELSE
                           SET     WS-FQ         TO    WS-FA
                       END-IF
                   END-IF
               END-IF
               IF    WS-FQ > 0
                   IF    WS-MSG(WS-FP:WS-FQ) NOT = SPACES
                       SET     WS-OK             TO    Vwr-Chat::AppendMarkdown(WS-MSG(WS-FP:WS-FQ)::Trim())
                   END-IF
               END-IF
               IF    WS-FKIND = "F"
                   *>           A fence: from past "```html" to the closing fence (or the
                   *>           end of the answer, when the model left it open).

                   COMPUTE    WS-BS = WS-FP + WS-FA + 7
                   IF    WS-BS > 64100
                       EXIT PERFORM
                   END-IF
                   SET     WS-FB                 TO    0
                   INSPECT    WS-MSG(WS-BS:) TALLYING WS-FB
                       FOR CHARACTERS BEFORE INITIAL "```"
                   SET     WS-FC                 TO    3
               ELSE
                   *>           A page in the answer: to its </html>, or to the end.

                   COMPUTE    WS-BS = WS-FP + WS-FA
                   COMPUTE    WS-REST = 64100 - WS-BS + 1

                   SET     WS-FB                 TO    0
                   INSPECT    WS-MSG(WS-BS:) TALLYING WS-FB
                       FOR CHARACTERS BEFORE INITIAL "</html>"
                   IF    WS-FB NOT < WS-REST
                       SET     WS-FB             TO    0
                       INSPECT    WS-MSG(WS-BS:) TALLYING WS-FB
                           FOR CHARACTERS BEFORE INITIAL "</HTML>"
                   END-IF
                   IF    WS-FB < WS-REST
                       ADD    7 TO WS-FB
                   END-IF
                   SET     WS-FC                 TO    0
               END-IF
               IF    WS-FB > 0
                   IF    WS-MSG(WS-BS:WS-FB) NOT = SPACES
                       SET     WS-OK             TO    Vwr-Chat::AppendHtml(WS-MSG(WS-BS:WS-FB)::Trim())
                   END-IF
               END-IF
               COMPUTE    WS-FP = WS-BS + WS-FB + WS-FC
               *>       ...and the fence the page sat in is closed right after it: a
               *>       bare "```" straight after a page written in is that fence's
               *>       end, whether or not its opening was in this answer.

               IF    WS-FKIND = "R"
                   SET     WS-FQ                 TO    WS-FP
                   PERFORM    UNTIL WS-FQ > 64097
                           OR (WS-MSG(WS-FQ:1) NOT = SPACE
                           AND WS-MSG(WS-FQ:1) NOT = X"0A"
                           AND WS-MSG(WS-FQ:1) NOT = X"0D")
                       ADD    1 TO WS-FQ
                   END-PERFORM
                   IF    WS-FQ NOT > 64097
                       IF    WS-MSG(WS-FQ:3) = "```"
                               AND (WS-FSTRIP = "Y"
                               OR WS-MSG(WS-FQ + 3:1) = SPACE
                               OR WS-MSG(WS-FQ + 3:1) = X"0A"
                               OR WS-MSG(WS-FQ + 3:1) = X"0D")
                           COMPUTE    WS-FP = WS-FQ + 3
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           GOBACK.

       END PROGRAM PC-APPEND-FORMATTED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAY-STATUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-STATUS-TEXT in: shown in the status bubble, in italics - made
           *>   the first time, changed in place after that.

           SET     WS-MSG                        TO    SPACES
           STRING    "*" WS-STATUS-TEXT::Trim() "*" INTO WS-MSG
           IF    WS-STATUS-ID = SPACES
               SET     WS-STATUS-ID              TO    Vwr-Chat::AppendMarkdown(WS-MSG::Trim(), "agent")
           ELSE
               MOVE    Vwr-Chat::ReplaceMessage(WS-STATUS-ID::Trim(),
               WS-MSG::Trim(), "Markdown") TO WS-OK
           END-IF
           SET     WS-OK                         TO    Vwr-Chat::JumpToLatest()

           GOBACK.

       END PROGRAM PC-SAY-STATUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SHOW-AGENT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-MSG in: the agent's message - the answer, or what went wrong.
           *>   It takes the status bubble's place, so the bubble becomes it -
           *>   unless it is a presentation, which goes straight into the
           *>   conversation, full width, with no bubble.

           CALL    "PC-IS-FORMATTED"
           SET     WS-SHOW-ANSWER                TO    "N"
           IF    WS-FORMATTED = "Y"
               IF    WS-STATUS-ID NOT = SPACES
                   SET     WS-OK                 TO    Vwr-Chat::RemoveMessage(WS-STATUS-ID::Trim())
                   SET     WS-STATUS-ID          TO    SPACES
               END-IF
               CALL    "PC-APPEND-FORMATTED"
               SET     WS-OK                     TO    Vwr-Chat::JumpToLatest()
               EXIT PROGRAM
           END-IF
           IF    WS-STATUS-ID = SPACES
               SET     WS-OK                     TO    Vwr-Chat::AppendMarkdown(WS-MSG::Trim(), "agent")
           ELSE
               MOVE    Vwr-Chat::ReplaceMessage(WS-STATUS-ID::Trim(),
               WS-MSG::Trim(), "Markdown") TO WS-OK
               SET     WS-STATUS-ID              TO    SPACES
           END-IF
           SET     WS-OK                         TO    Vwr-Chat::JumpToLatest()

           GOBACK.

       END PROGRAM PC-SHOW-AGENT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TOOL-USE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-TOOL-KIND in: an agent is using a tool the runtime answers -
           *>   the Knowledge Base, or a registered data file.

           IF    WS-STAGE = "IDLE"
               EXIT PROGRAM
           END-IF
           EVALUATE    WS-TOOL-KIND
               WHEN "KnowledgeBase" MOVE T-ST-SEARCH-KB TO WS-STATUS-TEXT
               WHEN "IndexedFile" MOVE T-ST-READ-DATA TO WS-STATUS-TEXT
               WHEN OTHER EXIT PROGRAM
           END-EVALUATE
           CALL    "PC-SAY-STATUS"

           GOBACK.

       END PROGRAM PC-TOOL-USE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FINISH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-REPLY in: the answer the user sees, kept as the next turn.

           SET     WS-STAGE                      TO    "IDLE"
           *> Before the answer is shown or kept: its template block is cut out of
           *> WS-REPLY here.

           CALL    "PC-SAVE-TEMPLATE"
           SET     WS-MSG                        TO    WS-REPLY
           CALL    "PC-DROP-MARKER"

           SET     WS-REPLY                      TO    WS-MSG
           SET     WS-SHOW-ANSWER                TO    "Y"

           CALL    "PC-SHOW-AGENT"

           SET     WS-ROLE                       TO    "A"
           SET     WS-TEXT                       TO    WS-REPLY

           *> PC-SAVE-TURN adds 1 to WS-SEQ before it writes, so this is the number
           *> the answer is stored under; PC-SAVE-TASKS files its tasks by it.

           COMPUTE    WS-ANSWER-SEQ = WS-SEQ + 1

           CALL    "PC-SAVE-TURN"
           CALL    "PC-SAVE-TASKS"
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM PC-FINISH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DROP-MARKER IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-MSG in and out: an answer that opens with a "TEMPLATE: name"
           *>   line, and goes on after it, loses that line. The line is how the
           *>   model chooses a template; a model that repeats it over the report
           *>   it then writes had it shown, and printed in the PDF, above the page
           *>   (operator, 2026-09-29).

           SET     WS-FP                         TO    1
           PERFORM    UNTIL WS-FP > 200
                   OR (WS-MSG(WS-FP:1) NOT = SPACE
                   AND WS-MSG(WS-FP:1) NOT = X"0A"
                   AND WS-MSG(WS-FP:1) NOT = X"0D")
               ADD    1 TO WS-FP
           END-PERFORM
           IF    WS-FP > 200
               EXIT PROGRAM
           END-IF
           IF    FUNCTION UPPER-CASE(WS-MSG(WS-FP:9)) NOT = "TEMPLATE:"
               EXIT PROGRAM
           END-IF
           SET     WS-FB                         TO    0
           INSPECT    WS-MSG(WS-FP:) TALLYING WS-FB FOR CHARACTERS BEFORE INITIAL X"0A"
           COMPUTE    WS-FA = WS-FP + WS-FB + 1
           IF    WS-FA > 64000
               EXIT PROGRAM
           END-IF
           IF    WS-MSG(WS-FA:) = SPACES
               EXIT PROGRAM
           END-IF

           SET     WS-TEXT                       TO    WS-MSG(WS-FA:)
           SET     WS-MSG                        TO    WS-TEXT::Trim()

           GOBACK.

       END PROGRAM PC-DROP-MARKER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DISPATCH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The orchestrator's plan in WS-REPLY: its TASK: lines go to the other
           *>   agents, all at once (R58, R59). The tool worker takes the first.
           *>   A plan that starts ASK: is a question for the user instead - which
           *>   report template to use - shown as the answer.

           SET     WS-FP                         TO    1
           PERFORM    UNTIL WS-FP > 200
                   OR (WS-REPLY(WS-FP:1) NOT = SPACE
                   AND WS-REPLY(WS-FP:1) NOT = X"0A"
                   AND WS-REPLY(WS-FP:1) NOT = X"0D")
               ADD    1 TO WS-FP
           END-PERFORM
           IF    WS-FP NOT > 200
               IF    FUNCTION UPPER-CASE(WS-REPLY(WS-FP:4)) = "ASK:"
                   SET     WS-TEXT               TO    WS-REPLY(WS-FP + 4:)
                   SET     WS-REPLY              TO    WS-TEXT::Trim()

                   CALL    "PC-FINISH"
                   EXIT PROGRAM
               END-IF
           END-IF

           SET     WS-TASK-COUNT                 TO    0
           SET     WS-JOIN-MODE                  TO    "N"
           SET     WS-LINES                      TO    SPACES

           *> Only the first 16 lines of the plan are read, and at most 12 TASK:
           *> lines are kept - the sizes of WS-LINES and WS-TASKS.

           UNSTRING    WS-REPLY DELIMITED BY X"0A"
               INTO WS-L(1) WS-L(2) WS-L(3) WS-L(4) WS-L(5) WS-L(6)
               WS-L(7) WS-L(8) WS-L(9) WS-L(10) WS-L(11) WS-L(12)
               WS-L(13) WS-L(14) WS-L(15) WS-L(16)
           END-UNSTRING
           PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 16
               SET     WS-L(WS-I)                TO    WS-L(WS-I)::Trim()
               *>       A "TEMPLATE: name" line: the template the report takes.

               IF    FUNCTION UPPER-CASE(WS-L(WS-I)(1:9)) = "TEMPLATE:"
                   SET     WS-TPL-NAME           TO    WS-L(WS-I)(10:)
                   CALL    "PC-TPL-FIND"
                   SET     WS-TPL-DONE           TO    "Y"
               END-IF
               IF    FUNCTION UPPER-CASE(WS-L(WS-I)(1:5)) = "JOIN:"
                   SET     WS-JOIN-MODE          TO    "Y"
               END-IF
               IF    FUNCTION UPPER-CASE(WS-L(WS-I)(1:5)) = "TASK:"
                       AND WS-TASK-COUNT < 12
                   ADD    1 TO WS-TASK-COUNT
                   SET     WS-TASK(WS-TASK-COUNT)
                                                 TO    FUNCTION TRIM(WS-L(WS-I)(6:495))
               END-IF
           END-PERFORM
           *> No TASK: line in the plan: the user's own question is the one task,
           *> so the chat still answers.

           IF    WS-TASK-COUNT = 0
               SET     WS-TASK-COUNT             TO    1
               SET     WS-TASK(1)                TO    WS-QUESTION
           END-IF
           PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
               SET     WS-TASK-ST(WS-TK)         TO    "Q"
               SET     WS-TASK-RES(WS-TK)        TO    SPACES
           END-PERFORM
           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               SET     WS-AG-TASK(WS-A) WS-AG-RESULT(WS-A)
                                                 TO    SPACES

               SET     WS-AG-WAIT(WS-A)          TO    "N"
               SET     WS-AG-TASKNO(WS-A)        TO    0
           END-PERFORM

           SET     WS-PENDING                    TO    WS-TASK-COUNT
           SET     WS-NEXT-TASK                  TO    1
           SET     WS-TASKS-RAN                  TO    "Y"
           SET     WS-STAGE                      TO    "WORK"

           *>   Every agent that is not orchestrating takes a task, the tool
           *>   worker first; the rest wait in the queue (PC-NEXT-TASK).

           SET     WS-B                          TO    WS-WORKER
           IF    WS-B = 0
               SET     WS-B                      TO    1
           END-IF
           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               IF    WS-B NOT = WS-ORCH AND WS-AG-ENTRY(WS-B) NOT = SPACES
                   SET     WS-FROM               TO    WS-B
                   CALL    "PC-NEXT-TASK"
               END-IF
               ADD    1 TO WS-B
               IF    WS-B > 3
                   SET     WS-B                  TO    1
               END-IF
           END-PERFORM
           CALL    "PC-WORK-STATUS"

           GOBACK.

       END PROGRAM PC-DISPATCH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEXT-TASK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-FROM in, an agent with nothing to do: it takes the next queued
           *>   task, with the main prompt's TASK section; none left, it rests.

           IF    WS-NEXT-TASK > WS-TASK-COUNT
               SET     WS-AG-WAIT(WS-FROM)       TO    "N"
               SET     WS-AG-TASKNO(WS-FROM)     TO    0

               EXIT PROGRAM
           END-IF

           SET     WS-AG-TASKNO(WS-FROM)         TO    WS-NEXT-TASK
           SET     WS-TASK-ST(WS-NEXT-TASK)      TO    "R"
           SET     WS-AG-TASK(WS-FROM)           TO    WS-TASK(WS-NEXT-TASK)

           ADD    1 TO WS-NEXT-TASK

           SET     WS-AG-WAIT(WS-FROM)           TO    "Y"
           SET     WS-SEC-NAME                   TO    "TASK"

           CALL    "PC-SECTION"

           SET     WS-TOKEN                      TO    "{TASK}"
           SET     WS-INS                        TO    WS-AG-TASK(WS-FROM)

           CALL    "PC-FILL"

           SET     WS-PROMPT                     TO    SPACES
           SET     WS-PROMPT                     TO    WS-SEC

           CALL    "PC-ASK"

           GOBACK.

       END PROGRAM PC-NEXT-TASK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TASK-DONE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-FROM, WS-TASK-OK and WS-TEXT (the answer, or why there is none)
           *>   in: that agent's task is over. An empty answer counts as failed.
           *>   The agent takes the next task; when none is left running or
           *>   queued, the answer is put together (PC-COMPOSE).

           IF    WS-AG-WAIT(WS-FROM) NOT = "Y"
               EXIT PROGRAM
           END-IF

           SET     WS-AG-WAIT(WS-FROM)           TO    "N"
           SET     WS-TK                         TO    WS-AG-TASKNO(WS-FROM)

           IF    WS-TK > 0
               SET     WS-TASK-RES(WS-TK)        TO    WS-TEXT
               IF    WS-TASK-OK = "Y" AND WS-TEXT NOT = SPACES
                   SET     WS-TASK-ST(WS-TK)     TO    "D"
               ELSE
                   SET     WS-TASK-ST(WS-TK)     TO    "F"
               END-IF
               SUBTRACT    1 FROM WS-PENDING
           END-IF
           IF    WS-PENDING = 0
               CALL    "PC-COMPOSE"
           ELSE
               CALL    "PC-NEXT-TASK"
               CALL    "PC-WORK-STATUS"
           END-IF

           GOBACK.

       END PROGRAM PC-TASK-DONE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-WORK-STATUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   "Working: N task(s) across M agent(s)" - the tasks left.

           SET     WS-B                          TO    0
           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               IF    WS-AG-WAIT(WS-A) = "Y"
                   ADD    1 TO WS-B
               END-IF
           END-PERFORM

           SET     WS-FMT                        TO    T-WORKING
           SET     WS-NUM-ED                     TO    WS-PENDING
           SET     WS-ARG1                       TO    WS-NUM-ED::Trim()
           SET     WS-ARG2                       TO    WS-B

           CALL    "PC-FMT"

           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()
           SET     WS-STATUS-TEXT                TO    WS-FMT-OUT

           CALL    "PC-SAY-STATUS"

           GOBACK.

       END PROGRAM PC-WORK-STATUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NOTE-MISSING IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-REPLY in and out: a task that failed or came back empty is
           *>   SAID, at the end of the answer - never left out in silence.

           SET     WS-TASK-OK                    TO    "N"
           *> WS-TASK-OK is borrowed as a scratch flag here: "Y" once any task has
           *> failed.

           PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
               IF    WS-TASK-ST(WS-TK) = "F"
                   SET     WS-TASK-OK            TO    "Y"
               END-IF
           END-PERFORM
           IF    WS-TASK-OK = "N"
               EXIT PROGRAM
           END-IF
           *> Walk back from the end of WS-REPLY to its last non-blank character,
           *> so the notes are appended right after the answer.

           PERFORM    VARYING WS-SX-B FROM 64000 BY -1
                   UNTIL WS-SX-B < 2 OR WS-REPLY(WS-SX-B:1) NOT = SPACE
               CONTINUE
           END-PERFORM
           ADD    1 TO WS-SX-B
           PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
               IF    WS-TASK-ST(WS-TK) = "F"
                   STRING    X"0A" X"0A" "> **" T-PART-MISSING::Trim()
                       "** " WS-TASK(WS-TK)::Trim()
                       INTO WS-REPLY WITH POINTER WS-SX-B
                   END-STRING
               END-IF
           END-PERFORM

           GOBACK.

       END PROGRAM PC-NOTE-MISSING.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE-TASKS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The tasks of the answer just saved (WS-ANSWER-SEQ), each with what
           *>   its assistant wrote, into data/taskresults.idx: 2000 characters a
           *>   record, in order. Nothing reads them back into the chat.

           IF    WS-TASKS-RAN NOT = "Y"
               EXIT PROGRAM
           END-IF
           SET     WS-TASKS-RAN                  TO    "N"
           OPEN    I-O TASKRES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TASKRES-FILE
               CLOSE    TASKRES-FILE
               OPEN    I-O TASKRES-FILE
           END-IF
           PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
               *> A result (up to 32000 characters) is stored in 2000-character pieces.
               *> Piece 1 is always written, even when empty; later ones only while
               *> text is left.

               SET     WS-PIECE-AT               TO    1
               PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 16
                   IF    WS-I > 1
                       IF    WS-TASK-RES(WS-TK)(WS-PIECE-AT:2000) = SPACES
                           EXIT PERFORM
                       END-IF
                   END-IF

                   SET     TR-CONV               TO    WS-CUR-CONV
                   SET     TR-ANSWER             TO    WS-ANSWER-SEQ
                   SET     TR-TASK               TO    WS-TK
                   SET     TR-PIECE              TO    WS-I
                   SET     TR-STATE              TO    WS-TASK-ST(WS-TK)
                   SET     TR-TASK-TEXT          TO    WS-TASK(WS-TK)
                   SET     TR-TEXT               TO    WS-TASK-RES(WS-TK)(WS-PIECE-AT:2000)

                   WRITE    TASKRES-REC
                       INVALID KEY REWRITE TASKRES-REC
                   END-WRITE
                   ADD    2000 TO WS-PIECE-AT
               END-PERFORM
           END-PERFORM
           COMMIT
           CLOSE    TASKRES-FILE

           GOBACK.

       END PROGRAM PC-SAVE-TASKS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-COMPOSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Every task has reported (R60): the orchestrator answers from the
           *>   results, as the main prompt's COMPOSE section says - and from the
           *>   chosen template, with its TEMPLATE section. No agent answers the
           *>   user directly (R61).
           *>   "JOIN:" and no report template: the parts, as written, in task
           *>   order - nothing condensed, nothing re-said.

           IF    WS-JOIN-MODE = "Y" AND WS-TPL-SKEL = SPACES
               SET     WS-REPLY                  TO    SPACES
               SET     WS-SX-B                   TO    1

               PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
                   IF    WS-TASK-ST(WS-TK) = "D"
                       STRING    WS-TASK-RES(WS-TK)::Trim() X"0A" X"0A"
                           INTO WS-REPLY WITH POINTER WS-SX-B
                       END-STRING
                   END-IF
               END-PERFORM

               CALL    "PC-NOTE-MISSING"
               CALL    "PC-FINISH"

               EXIT PROGRAM
           END-IF

           SET     WS-INS                        TO    SPACES
           SET     WS-SX-B                       TO    1

           PERFORM    VARYING WS-TK FROM 1 BY 1 UNTIL WS-TK > WS-TASK-COUNT
               IF    WS-TASK-ST(WS-TK) = "D"
                   STRING    "- On " WS-TASK(WS-TK)::Trim() ": "
                       WS-TASK-RES(WS-TK)::Trim() X"0A"
                       INTO WS-INS WITH POINTER WS-SX-B
                   END-STRING
               ELSE
                   STRING    "- On " WS-TASK(WS-TK)::Trim()
                       ": NO RESULT - this part could not be answered." X"0A"
                       INTO WS-INS WITH POINTER WS-SX-B
                   END-STRING
               END-IF
           END-PERFORM
           SET     WS-SEC-NAME                   TO    "COMPOSE"
           CALL    "PC-SECTION"
           SET     WS-TOKEN                      TO    "{RESULTS}"
           CALL    "PC-FILL"

           SET     WS-PROMPT                     TO    SPACES
           SET     WS-PTR                        TO    1

           STRING    WS-HISTORY::Trim() X"0A" X"0A" WS-SEC::Trim() INTO WS-PROMPT WITH POINTER WS-PTR
           END-STRING
           IF    WS-TPL-SKEL NOT = SPACES
               CALL    "PC-TEMPLATE-TEXT"
               STRING    X"0A" X"0A" WS-SEC::Trim() INTO WS-PROMPT WITH POINTER WS-PTR
               END-STRING
           END-IF

           SET     WS-STAGE                      TO    "COMPOSE"
           SET     WS-FROM                       TO    WS-ORCH

           CALL    "PC-ASK"
           SET     WS-STATUS-TEXT                TO    T-ST-COMPOSING
           CALL    "PC-SAY-STATUS"

           GOBACK.

       END PROGRAM PC-COMPOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-ON-REPLY IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-FROM, WS-REPLY, WS-T-IN, WS-T-OUT in: an agent answered.

           CALL    "PC-ADD-TOKENS"
           *>   "TEMPLATE: name" from the orchestrator: the user chose a report
           *>   template. Its skeleton goes back with the main prompt's TEMPLATE
           *>   section, and the same stage answers again - once per question.

           IF    (WS-STAGE = "SINGLE" OR WS-STAGE = "COMPOSE")
                   AND WS-TPL-DONE NOT = "Y"
               CALL    "PC-TPL-LINE"
               IF    WS-TPL-SKEL NOT = SPACES
                   SET     WS-TPL-DONE           TO    "Y"
                   IF    WS-STAGE = "COMPOSE"
                       CALL    "PC-COMPOSE"
                   ELSE
                       CALL    "PC-TEMPLATE-TEXT"

                       SET     WS-PROMPT         TO    SPACES
                       SET     WS-PTR            TO    1

                       STRING    WS-HISTORY::Trim() X"0A" X"0A"
                           WS-SEC::Trim()
                           INTO WS-PROMPT WITH POINTER WS-PTR
                       END-STRING
                       CALL    "PC-ASK"
                   END-IF
                   EXIT PROGRAM
               END-IF
           END-IF
           IF    WS-STAGE = "COMPOSE"
               CALL    "PC-NOTE-MISSING"
           END-IF
           EVALUATE    WS-STAGE
               WHEN "SINGLE"
               WHEN "COMPOSE"
                   CALL    "PC-FINISH"
               WHEN "PLAN"
                   CALL    "PC-DISPATCH"
               WHEN "WORK"
                   SET     WS-TEXT               TO    WS-REPLY
                   SET     WS-TASK-OK            TO    "Y"

                   CALL    "PC-TASK-DONE"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE

           GOBACK.

       END PROGRAM PC-ON-REPLY.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PICK-TOPIC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   New conversation: the user chooses its topic first (operator,
           *>   2026-09-27). The topics go to PICK-FORM by name, the current one
           *>   preselected; the one chosen becomes the current topic and a fresh
           *>   conversation starts in it. Cancel changes nothing.

           SET     WS-PICK-COUNT                 TO    0
           SET     WS-PICK-SEL                   TO    1
           SET     WS-PICK-LIST                  TO    SPACES
           SET     WS-PICK-PTR                   TO    1

           OPEN    I-O TOPICS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TOPICS-FILE
               CLOSE    TOPICS-FILE
               OPEN    I-O TOPICS-FILE
           END-IF

           SET     TOP-ID                        TO    LOW-VALUES
           SET     WS-EOF                        TO    "N"

           START    TOPICS-FILE KEY IS >= TOP-ID
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y" OR WS-PICK-COUNT = 200
               READ    TOPICS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       ADD    1 TO WS-PICK-COUNT
                       SET     WS-PICK-ID(WS-PICK-COUNT)
                                                 TO    TOP-ID

                       IF    TOP-ID = WS-CUR-TOPIC
                           SET     WS-PICK-SEL   TO    WS-PICK-COUNT
                       END-IF
                       IF    WS-PICK-COUNT > 1
                           STRING    X"0A" INTO WS-PICK-LIST WITH POINTER WS-PICK-PTR
                           END-STRING
                       END-IF
                       STRING    FUNCTION TRIM(TOP-NAME) INTO WS-PICK-LIST WITH POINTER WS-PICK-PTR
                       END-STRING
               END-READ
           END-PERFORM
           CLOSE    TOPICS-FILE
           IF    WS-PICK-COUNT = 0
               SET     WS-OK                     TO    Vwr-Chat::AppendMarkdown(T-OPEN-TOPIC-MENU::Trim())
               EXIT PROGRAM
           END-IF

           INVOKE    ME::SetProperty("PickTitle", T-PICK-TOPIC)
           INVOKE    ME::SetProperty("PickOk", T-PICK-START)
           INVOKE    ME::SetProperty("PickCancel", T-PICK-CANCEL)
           INVOKE    ME::SetProperty("PickItems", WS-PICK-LIST)
           INVOKE    ME::SetProperty("PickSelected", WS-PICK-SEL)

           *> PICK-FORM overwrites PickAnswer with the item's number; Cancel leaves
           *> the 0, so a number outside 1..WS-PICK-COUNT means no choice.

           INVOKE    ME::SetProperty("PickAnswer", "0")
           INVOKE    ME::OpenFormSync("PICK-FORM")
           INVOKE    ME::GetProperty("PickAnswer") RETURNING WS-PICK-ANSWER

           SET     WS-PICK-N                     TO    FUNCTION NUMVAL(WS-PICK-ANSWER)
           IF    WS-PICK-N < 1 OR WS-PICK-N > WS-PICK-COUNT
               EXIT PROGRAM
           END-IF

           SET     WS-SET-NAME                   TO    "CUR-TOPIC"
           SET     WS-SET-VALUE                  TO    WS-PICK-ID(WS-PICK-N)

           CALL    "PC-SETTING-PUT"
           CALL    "PC-LOAD-TOPIC"
           CALL    "PC-NEW-CONV"
           CALL    "PC-LIST-CONVS"

           *>   The topic is chosen: show the chat, wherever the user was - what a
           *>   click on the Chat row does (operator, 2026-09-27).

           SET     WS-ITEM                       TO    SideMenu-1::ActivateItem("chat")

           GOBACK.

       END PROGRAM PC-PICK-TOPIC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEW-CONV IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-CUR-CONV                   TO    SPACES
           SET     WS-SEQ                        TO    0
           SET     WS-OK                         TO    Vwr-Chat::NewConversation()

           GOBACK.

       END PROGRAM PC-NEW-CONV.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LIST-CONVS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The topic's conversations as run-time rows, newest first (R39).

           SET     WS-CONV-COUNT                 TO    0
           OPEN    I-O CONVS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT CONVS-FILE
               CLOSE    CONVS-FILE
               OPEN    I-O CONVS-FILE
           END-IF

           SET     CNV-TOPIC                     TO    WS-CUR-TOPIC
           SET     WS-EOF                        TO    "N"

           *> CNV-TOPIC is an alternate key with duplicates: after START the reads
           *> run on into the next topic's records, so the loop stops at the first
           *> record that is not this topic's.

           START    CONVS-FILE KEY IS = CNV-TOPIC
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    CONVS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    CNV-TOPIC NOT = WS-CUR-TOPIC
                           SET     WS-EOF        TO    "Y"
                       ELSE
                           *> Records come oldest first (ids are time stamps), and only the newest
                           *> 50 rows fit: when full, shift every row up one (dropping the oldest)
                           *> and reuse the last slot for this one.

                           IF    WS-CONV-COUNT < 50
                               ADD    1 TO WS-CONV-COUNT
                           ELSE
                               PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 49
                                   SET     WS-CONV-ROW(WS-I)
                                                 TO    WS-CONV-ROW(WS-I + 1)
                               END-PERFORM
                           END-IF
                           SET     WS-CL-ID(WS-CONV-COUNT)
                                                 TO    CNV-ID

                           SET     WS-CL-TITLE(WS-CONV-COUNT)
                                                 TO    CNV-TITLE
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    CONVS-FILE
           SET     WS-OK                         TO    SideMenu-1::Clear()
           *>   The menu's own rows are designed (SideMenu-1.menu.yaml), so the
           *>   designer and the preview show them; their labels follow the
           *>   language here. Clear() keeps these labels - it takes only the
           *>   conversation rows added below.

           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("chat", T-MENU-CHAT::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("newc", T-MENU-NEWC::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("tpcs", T-TOPICS-TITLE::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("docs", T-DOCS-TITLE::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("fils", T-FILES-TITLE::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("prmt", T-PROMPT-TITLE::Trim())
           SET     WS-OK                         TO    SideMenu-1::SetItemLabel("sett", T-SETTINGS-TITLE::Trim())

           *>   The welcome form's row is a run-time one, so Clear() took it:
           *>   it comes back here, in the current language, and stays enabled.

           MOVE    SideMenu-1::AddItem("welc", T-MENU-WELC::Trim(),
               "rocket", "", "open-form:welcome-form") TO WS-OK
           IF    WS-CONV-COUNT > 0
               SET     WS-ITEM                   TO    SideMenu-1::AddSection(T-CONVERSATIONS::Trim())
               *>       Row ids are "c" + the conversation id, apart from the menu's own.

               PERFORM    VARYING WS-I FROM WS-CONV-COUNT BY -1 UNTIL WS-I < 1
                   SET     WS-ROW-ID             TO    SPACES
                   STRING    "c" WS-CL-ID(WS-I) DELIMITED BY SIZE INTO WS-ROW-ID
                   MOVE    SideMenu-1::AddItem(WS-ROW-ID,
                   WS-CL-TITLE(WS-I)::Trim(), "chat") TO WS-OK
               END-PERFORM
           END-IF

           GOBACK.

       END PROGRAM PC-LIST-CONVS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-STATUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   This month's input and output tokens and conversations (R42).

           SET     WS-NOW                        TO    FUNCTION CURRENT-DATE
           SET     WS-MONTH                      TO    WS-NOW(1:6)
           SET     WS-SUM-IN WS-SUM-OUT WS-SUM-CONVS
                                                 TO    0

           OPEN    I-O CONVS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT CONVS-FILE
               CLOSE    CONVS-FILE
               OPEN    I-O CONVS-FILE
           END-IF

           SET     CNV-ID                        TO    LOW-VALUES
           SET     WS-EOF                        TO    "N"

           START    CONVS-FILE KEY IS >= CNV-ID
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    CONVS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       *> CNV-CREATED is YYYYMMDDHHMMSS and WS-MONTH the YYYYMM of today, so the
                       *> first six characters decide the month. Every topic counts, not only
                       *> the open one: the scan walks the whole file.

                       IF    CNV-CREATED(1:6) = WS-MONTH
                           ADD    1 TO WS-SUM-CONVS
                           ADD    CNV-IN-TOKENS TO WS-SUM-IN
                           ADD    CNV-OUT-TOKENS TO WS-SUM-OUT
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    CONVS-FILE

           SET     WS-E1                         TO    WS-SUM-IN
           SET     WS-E2                         TO    WS-SUM-OUT
           SET     WS-E3                         TO    WS-SUM-CONVS
           SET     WS-FMT                        TO    T-THIS-MONTH
           SET     WS-ARG1                       TO    WS-E3
           SET     WS-ARG2                       TO    WS-E1
           SET     WS-ARG3                       TO    WS-E2

           CALL    "PC-FMT"
           SET     WS-LINE                       TO    WS-FMT-OUT
           IF    WS-FAILED > 0
               SET     WS-FMT                    TO    T-FILES-UNUSABLE
               SET     WS-ARG1                   TO    WS-FILE-NOTE

               CALL    "PC-FMT"

               SET     WS-ARG4                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG4::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
           END-IF
           IF    WS-N-AGENTS > 1
               SET     WS-FMT                    TO    T-ORCH-LINE
               SET     WS-ARG1                   TO    WS-ORCH
               SET     WS-ARG2                   TO    WS-WORKER

               CALL    "PC-FMT"

               SET     WS-ARG4                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG4::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
           END-IF
           SET     Lbl-Status::Caption           TO    WS-LINE

           GOBACK.

       END PROGRAM PC-STATUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE-TURN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-ROLE ("U" or "A") and WS-TEXT in: the next turn of the current
           *>   conversation, written and committed at once (R38, R10f).

           ADD    1 TO WS-SEQ
           OPEN    I-O TURNS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TURNS-FILE
               CLOSE    TURNS-FILE
               OPEN    I-O TURNS-FILE
           END-IF
           *>   A turn longer than one record goes over several, in order: the
           *>   first under its role ("U"/"A"), the rest under the same letter in
           *>   lower case - "this continues the turn before it". A file written
           *>   before holds no lower-case role, and reads exactly as it did.

           SET     WS-PIECE-ROLE                 TO    WS-ROLE
           SET     WS-PIECE-AT                   TO    1

           PERFORM    UNTIL WS-PIECE-AT > 64000
               *> The first piece is always written; a later one only if it holds text,
               *> WS-TEXT being 64000 characters padded with spaces.

               IF    WS-PIECE-AT > 1
                   IF    WS-TEXT(WS-PIECE-AT:2000) = SPACES
                       EXIT PERFORM
                   END-IF
                   ADD    1 TO WS-SEQ
                   SET     WS-PIECE-ROLE         TO    FUNCTION LOWER-CASE(WS-ROLE)
               END-IF

               SET     TRN-CONV                  TO    WS-CUR-CONV
               SET     TRN-SEQ                   TO    WS-SEQ
               SET     TRN-ROLE                  TO    WS-PIECE-ROLE
               SET     TRN-TEXT                  TO    WS-TEXT(WS-PIECE-AT:2000)

               WRITE    TURN-REC
                   INVALID KEY REWRITE TURN-REC
               END-WRITE
               ADD    2000 TO WS-PIECE-AT
           END-PERFORM
           COMMIT
           CLOSE    TURNS-FILE

           GOBACK.

       END PROGRAM PC-SAVE-TURN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-START-CONV IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A conversation is created by its first question, titled by it.

           IF    WS-CUR-CONV NOT = SPACES
               EXIT PROGRAM
           END-IF

           SET     WS-NOW                        TO    FUNCTION CURRENT-DATE
           *> 16 digits: CURRENT-DATE down to the hundredth of a second, where
           *> CNV-CREATED stops at the second. The id is unique in practice and
           *> sorts by creation time, which the newest-first list relies on.

           SET     WS-CUR-CONV                   TO    WS-NOW(1:16)
           SET     WS-SEQ                        TO    0

           OPEN    I-O CONVS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT CONVS-FILE
               CLOSE    CONVS-FILE
               OPEN    I-O CONVS-FILE
           END-IF

           SET     CNV-ID                        TO    WS-CUR-CONV
           SET     CNV-TOPIC                     TO    WS-CUR-TOPIC
           SET     CNV-TITLE                     TO    WS-QUESTION
           SET     CNV-CREATED                   TO    WS-NOW(1:14)
           SET     CNV-IN-TOKENS CNV-OUT-TOKENS CNV-TURNS
                                                 TO    0

           WRITE    CONV-REC
               INVALID KEY CONTINUE
           END-WRITE
           COMMIT
           CLOSE    CONVS-FILE
           CALL    "PC-LIST-CONVS"

           GOBACK.

       END PROGRAM PC-START-CONV.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-BUILD-PROMPT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The last WS-HISTORY-TURNS turns of this conversation, oldest first,
           *>   the current question last. Trimmed by turns, not tokens.

           SET     WS-PROMPT                     TO    SPACES
           SET     WS-PTR                        TO    1

           *> WS-SEQ is the record just saved (the current question), so the window
           *> of WS-HISTORY-TURNS records ends on it. It counts records, not turns:
           *> a long turn spans several, so the window may start mid-turn
           *> (PC-JOIN-PIECE skips such a leftover continuation).

           IF    WS-SEQ > WS-HISTORY-TURNS
               COMPUTE    WS-FIRST-SEQ = WS-SEQ - WS-HISTORY-TURNS + 1
           ELSE
               SET     WS-FIRST-SEQ              TO    1
           END-IF
           OPEN    I-O TURNS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TURNS-FILE
               CLOSE    TURNS-FILE
               OPEN    I-O TURNS-FILE
           END-IF

           SET     TRN-CONV                      TO    WS-CUR-CONV
           SET     TRN-SEQ                       TO    WS-FIRST-SEQ
           SET     WS-JOIN-ROLE                  TO    SPACE
           SET     WS-JOIN-TO                    TO    "P"
           SET     WS-EOF                        TO    "N"

           START    TURNS-FILE KEY IS >= TRN-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    TURNS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    TRN-CONV NOT = WS-CUR-CONV
                           SET     WS-EOF        TO    "Y"
                       ELSE
                           CALL    "PC-JOIN-PIECE"
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    TURNS-FILE
           SET     WS-JOIN-TO                    TO    "P"
           CALL    "PC-JOIN-FLUSH"

           GOBACK.

       END PROGRAM PC-BUILD-PROMPT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-JOIN-PIECE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   One TURNS-FILE record read: a continuation ("u"/"a") extends the
           *>   turn in hand - one with no turn before it, left by a history
           *>   window that starts mid-turn, is skipped; anything else ends the
           *>   turn in hand (PC-JOIN-FLUSH) and starts the next.

           IF    TRN-ROLE = "u" OR TRN-ROLE = "a"
               IF    WS-JOIN-ROLE NOT = SPACE
                   STRING    TRN-TEXT DELIMITED BY SIZE
                       INTO WS-JOIN WITH POINTER WS-JOIN-PT
                   END-STRING
               END-IF
               EXIT PROGRAM
           END-IF
           CALL    "PC-JOIN-FLUSH"

           SET     WS-JOIN                       TO    SPACES
           SET     WS-JOIN-PT                    TO    1

           STRING    TRN-TEXT DELIMITED BY SIZE
               INTO WS-JOIN WITH POINTER WS-JOIN-PT
           END-STRING
           SET     WS-JOIN-ROLE                  TO    TRN-ROLE

           GOBACK.

       END PROGRAM PC-JOIN-PIECE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-JOIN-FLUSH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The turn in hand, whole: into the prompt ("P") or the chat ("V").

           IF    WS-JOIN-ROLE = SPACE
               EXIT PROGRAM
           END-IF
           IF    WS-JOIN-TO = "P"
               IF    WS-JOIN-ROLE = "U"
                   STRING    "User: " WS-JOIN::Trim() X"0A" INTO WS-PROMPT WITH POINTER WS-PTR
                   END-STRING
               ELSE
                   STRING    "Assistant: " WS-JOIN::Trim() X"0A" INTO WS-PROMPT WITH POINTER WS-PTR
                   END-STRING
               END-IF
           ELSE
               IF    WS-JOIN-ROLE = "U"
                   SET     WS-OK                 TO    Vwr-Chat::AppendMarkdown(WS-JOIN::Trim(), "user")
               ELSE
                   *>           A presentation reopens as it was shown: in the stream.

                   SET     WS-MSG                TO    WS-JOIN
                   CALL    "PC-DROP-MARKER"
                   SET     WS-SHOW-ANSWER        TO    "N"
                   CALL    "PC-IS-FORMATTED"
                   IF    WS-FORMATTED = "Y"
                       CALL    "PC-APPEND-FORMATTED"
                   ELSE
                       SET     WS-OK             TO    Vwr-Chat::AppendMarkdown(WS-JOIN::Trim(), "agent")
                   END-IF
               END-IF
           END-IF
           SET     WS-JOIN-ROLE                  TO    SPACE

           GOBACK.

       END PROGRAM PC-JOIN-FLUSH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-OPEN-CONV IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-ITEM in: a conversation row from the sidebar ("c" + its id). Its turns go back
           *>   into the chat, and the next question continues it (R40).

           SET     WS-CUR-CONV                   TO    WS-ITEM(2:16)
           SET     WS-SEQ                        TO    0
           SET     WS-OK                         TO    Vwr-Chat::NewConversation()

           OPEN    I-O TURNS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TURNS-FILE
               CLOSE    TURNS-FILE
               OPEN    I-O TURNS-FILE
           END-IF

           SET     TRN-CONV                      TO    WS-CUR-CONV
           SET     TRN-SEQ                       TO    0
           SET     WS-JOIN-ROLE                  TO    SPACE
           SET     WS-JOIN-TO                    TO    "V"
           SET     WS-EOF                        TO    "N"

           START    TURNS-FILE KEY IS >= TRN-KEY
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM    UNTIL WS-EOF = "Y"
               READ    TURNS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF    TRN-CONV NOT = WS-CUR-CONV
                           SET     WS-EOF        TO    "Y"
                       ELSE
                           *> Ends on the number of the conversation's last record, so the next
                           *> turn PC-SAVE-TURN writes (WS-SEQ + 1) follows it.

                           SET     WS-SEQ        TO    TRN-SEQ
                           CALL    "PC-JOIN-PIECE"
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    TURNS-FILE
           SET     WS-JOIN-TO                    TO    "V"
           CALL    "PC-JOIN-FLUSH"

           GOBACK.

       END PROGRAM PC-OPEN-CONV.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SHOW-CHAT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Bring the chat into view. A conversation row and New conversation
           *>   only raise onMenuItemClick: with Topics, Documents or the settings
           *>   on the pane, the conversation loaded behind it and nothing seemed
           *>   to happen (operator, 2026-09-28: "the history is listed, but the
           *>   conversation does not load"). The Chat row's own action goes home.

           SET     WS-OK                         TO    SideMenu-1::ActivateItem("chat")

           GOBACK.

       END PROGRAM PC-SHOW-CHAT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MENU-STATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Until one agent has a model, PowerChat can do nothing useful: the
           *>   menu stays shut but for RAG settings, Getting started (the welcome
           *>   form, which says what to do) and Chat, the way back home that
           *>   opens it (item 1).

           SET     WS-CONFIGURED                 TO    "N"
           PERFORM    VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               IF    WS-AG-ENTRY(WS-A) NOT = SPACES
                   SET     WS-CONFIGURED         TO    "Y"
               END-IF
           END-PERFORM
           IF    WS-CONFIGURED = "Y"
               SET     WS-OK                     TO    "1"
           ELSE
               SET     WS-OK                     TO    "0"
           END-IF

           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("tpcs", WS-OK)
           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("docs", WS-OK)
           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("fils", WS-OK)
           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("prmt", WS-OK)

           *>   Chat and New conversation need a topic to talk about as well: with
           *>   none created they stay off, and Topics is where to make one. The
           *>   Topics form calls PC-REFRESH after a save, so they come on at once.

           SET     WS-HAS-TOPIC                  TO    "N"
           OPEN    I-O TOPICS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TOPICS-FILE
               CLOSE    TOPICS-FILE
               OPEN    I-O TOPICS-FILE
           END-IF
           SET     TOP-ID                        TO    LOW-VALUES
           START    TOPICS-FILE KEY IS >= TOP-ID
               INVALID KEY CONTINUE
               NOT INVALID KEY
                   READ    TOPICS-FILE NEXT RECORD
                       AT END CONTINUE
                       NOT AT END MOVE "Y" TO WS-HAS-TOPIC
                   END-READ
           END-START
           CLOSE    TOPICS-FILE
           IF    WS-HAS-TOPIC = "Y"
               SET     WS-OK                     TO    "1"
           ELSE
               SET     WS-OK                     TO    "0"
           END-IF
           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("chat", WS-OK)
           *>   A new conversation needs a model to answer in it, too.

           IF    WS-CONFIGURED NOT = "Y"
               SET     WS-OK                     TO    "0"
           END-IF
           SET     WS-ITEM                       TO    SideMenu-1::SetItemEnabled("newc", WS-OK)

           GOBACK.

       END PROGRAM PC-MENU-STATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-REFRESH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Everything the other forms may have changed while the chat was out
           *>   of sight: the models and agents, the topic, its files, the menu.

           CALL    "PC-HAND-MODELS"
           SET     WS-SHOWN-TOPIC                TO    WS-CUR-TOPIC
           CALL    "PC-LOAD-TOPIC"
           *> A conversation belongs to one topic: if the topic changed meanwhile,
           *> the open conversation is not this topic's, so start a blank one.

           IF    WS-CUR-TOPIC NOT = WS-SHOWN-TOPIC AND WS-CUR-TOPIC NOT = SPACES
               CALL    "PC-NEW-CONV"
           END-IF
           *> PC-LOAD-TOPIC elects the agents only when it finds a topic; with none,
           *> redo the election here so changed models still take effect.

           IF    WS-CUR-TOPIC = SPACES
               CALL    "PC-ELECT"
           END-IF

           CALL    "PC-LIST-CONVS"
           CALL    "PC-MENU-STATE"
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM PC-REFRESH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-RELABEL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Every text on this form in the language just picked: the designed
           *>   ones, the menu, the status line, the topic heading when it is a
           *>   message, and the open conversation's labels.

           CALL    "PC-TEXTS"
           CALL    "PC-LIST-CONVS"

           IF    WS-CUR-TOPIC = SPACES
               CALL    "PC-LOAD-TOPIC"
           END-IF
           IF    WS-CUR-CONV NOT = SPACES
               SET     WS-ITEM                   TO    SPACES
               STRING    "c" WS-CUR-CONV DELIMITED BY SIZE INTO WS-ITEM
               CALL    "PC-OPEN-CONV"
           END-IF
           CALL    "PC-STATUS"

           GOBACK.

       END PROGRAM PC-RELABEL.

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
           *> WS-LANG-IX is the language's column in PC-TEXT (en pt es fr jp cn);
           *> an unset or unknown code means English. The copy loop below must run
           *> over every row of PC-TEXT, so its bound has to match the OCCURS.

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
           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 36
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Txt-Input::HintText           TO    T-HINT-ASK::Trim()
           SET     Btn-Send::Caption             TO    T-SEND::Trim()
           SET     Btn-Pdf::Caption              TO    T-SAVE-PDF::Trim()
           SET     Lbl-Status::Caption           TO    T-STATUS::Trim()

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
       01 WS-PART                                PIC    X(120).
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
               *> No placeholder left (WS-DELIM stayed spaces): WS-PART already holds
               *> the tail of the text, so push WS-FROM past the end to leave the loop.

               EVALUATE    WS-DELIM
                   WHEN "&1" STRING WS-ARG1::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&2" STRING WS-ARG2::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&3" STRING WS-ARG3::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&4" STRING WS-ARG4::Trim() INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN OTHER COMPUTE WS-FROM = FUNCTION LENGTH(WS-FMT) + 1
               END-EVALUATE
           END-PERFORM

           GOBACK.

       END PROGRAM PC-FMT.

       END PROGRAM CHAT-FORM.

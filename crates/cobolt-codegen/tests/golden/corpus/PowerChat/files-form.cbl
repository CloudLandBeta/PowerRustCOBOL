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
       PROGRAM-ID. FILES-FORM.

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
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'FILES-FORM'.

      *>── AI Agent infrastructure ────────────────────────────────────
      *>   INVOKE agent-id 'Ask' USING BY VALUE WS-AGENT-PROMPT
      *>   returns at once; the reply arrives as onResponse (LastReply)
      *>   or onError (LastError).
       01 WS-AGENT-PROMPT        PIC X(4096)  VALUE SPACES.
       01 WS-AGENT-RESPONSE      PIC X(32767) VALUE SPACES.
       01 WS-AGENT-ERROR         PIC X(512)   VALUE SPACES.

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
       01 WS-TFILES-PATH                         GLOBAL    PIC X(240).
       01 WS-CUR-TOPIC                           GLOBAL    PIC X(16).
       01 WS-FILE-COUNT                          GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-FILE-SEQS                           GLOBAL.
           05 WS-FILE-SEQ                        PIC    9(3)    OCCURS 100.
       01 WS-NEXT-SEQ                            GLOBAL    PIC 9(3).
       01 WS-INDEX                               GLOBAL    PIC S9(4).
       01 WS-DATA                                GLOBAL    PIC X(250).
       01 WS-CIDX                                GLOBAL    PIC X(250).
       01 WS-NAME                                GLOBAL    PIC X(30).
       01 WS-RESULT                              GLOBAL    PIC X(30).
       01 WS-MESSAGE                             GLOBAL    PIC X(300).
       01 WS-ROW                                 GLOBAL    PIC X(620).
       01 WS-LINE                                GLOBAL    PIC X(400).
       01 WS-DISCARD                             GLOBAL    PIC X(4).
       *>   The CRUD tabs: which grid row/column was clicked, and whether the
       *>   Create/Update page holds a new file ("N") or an existing one ("E",
       *>   whose key sequence is WS-EDIT-SEQ).

       01 WS-TABS                                GLOBAL    PIC X(300).
       01 WS-ROW-NO                              GLOBAL    PIC 9(4).
       01 WS-COL-NO                              GLOBAL    PIC 9(4).
       01 WS-EDIT-MODE                           GLOBAL    PIC X    VALUE "N".
       01 WS-EDIT-SEQ                            GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-SAVE-OK                             GLOBAL    PIC X.
       01 WS-QUESTION                            GLOBAL    PIC X(300).
       01 WS-ANSWER                              GLOBAL    PIC X(4).
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-NOW                            GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   OPEN-TOPIC-FIRST

           05 FILLER                             PIC    X(140)    VALUE "Open a topic first.".
           05 FILLER                             PIC    X(140)    VALUE "Abra um tópico primeiro.".
           05 FILLER                             PIC    X(140)    VALUE "Primero abre un tema.".
           05 FILLER                             PIC    X(140)    VALUE "Ouvrez d'abord un sujet.".
           05 FILLER                             PIC    X(140)    VALUE "先にトピックを開いてください。".
           05 FILLER                             PIC    X(140)    VALUE "请先打开一个主题。".
           *>   FILES-OF

           05 FILLER                             PIC    X(140)    VALUE "Data files - &1".
           05 FILLER                             PIC    X(140)    VALUE "Arquivos de dados - &1".
           05 FILLER                             PIC    X(140)    VALUE "Archivos de datos - &1".
           05 FILLER                             PIC    X(140)    VALUE "Fichiers de données - &1".
           05 FILLER                             PIC    X(140)    VALUE "データファイル - &1".
           05 FILLER                             PIC    X(140)    VALUE "数据文件 - &1".
           *>   PICK-FILE-FIRST

           05 FILLER                             PIC    X(140)    VALUE "Pick a file in the list first.".
           05 FILLER                             PIC    X(140)    VALUE "Escolha um arquivo na lista primeiro.".
           05 FILLER                             PIC    X(140)    VALUE "Primero elige un archivo de la lista.".
           05 FILLER                             PIC    X(140)    VALUE "Choisissez d'abord un fichier dans la liste.".
           05 FILLER                             PIC    X(140)    VALUE "先に一覧からファイルを選んでください。".
           05 FILLER                             PIC    X(140)    VALUE "请先在列表中选择一个文件。".
           *>   FILE-REMOVED

           05 FILLER                             PIC    X(140)    VALUE "Removed. The file itself is untouched.".
           05 FILLER                             PIC    X(140)    VALUE "Removido. O arquivo em si não foi alterado.".
           05 FILLER                             PIC    X(140)    VALUE "Quitado. El archivo en sí no se ha modificado.".
           05 FILLER                             PIC    X(140)    VALUE "Retiré. Le fichier lui-même n'a pas été modifié.".
           05 FILLER                             PIC    X(140)    VALUE "削除しました。ファイル自体は変更されていません。".
           05 FILLER                             PIC    X(140)    VALUE "已移除。文件本身未被改动。".
           *>   GIVE-BOTH

           05 FILLER                             PIC    X(140)    VALUE "Give both the data file and its .cidx description.".
           05 FILLER                             PIC    X(140)    VALUE "Informe o arquivo de dados e a descrição .cidx dele.".
           05 FILLER                             PIC    X(140)    VALUE "Indica el archivo de datos y su descripción .cidx.".
           05 FILLER                             PIC    X(140)    VALUE "Indiquez le fichier de données et sa description .cidx.".
           05 FILLER                             PIC    X(140)    VALUE "データファイルとその .cidx 定義の両方を指定してください。".
           05 FILLER                             PIC    X(140)    VALUE "请同时提供数据文件及其 .cidx 描述。".
           *>   NOT-ADDED

           05 FILLER                             PIC    X(140)    VALUE "Not added - &1: &2".
           05 FILLER                             PIC    X(140)    VALUE "Não adicionado - &1: &2".
           05 FILLER                             PIC    X(140)    VALUE "No se añadió - &1: &2".
           05 FILLER                             PIC    X(140)    VALUE "Non ajouté - &1 : &2".
           05 FILLER                             PIC    X(140)    VALUE "追加されませんでした - &1: &2".
           05 FILLER                             PIC    X(140)    VALUE "未添加 - &1：&2".
           *>   FILE-ADDED

           05 FILLER                             PIC    X(140)    VALUE "Added &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Adicionado &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Añadido &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Ajouté &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "&1 を追加しました - &2".
           05 FILLER                             PIC    X(140)    VALUE "已添加 &1 - &2".
           *>   FILES-TITLE

           05 FILLER                             PIC    X(140)    VALUE "Data files".
           05 FILLER                             PIC    X(140)    VALUE "Arquivos de dados".
           05 FILLER                             PIC    X(140)    VALUE "Archivos de datos".
           05 FILLER                             PIC    X(140)    VALUE "Fichiers de données".
           05 FILLER                             PIC    X(140)    VALUE "データファイル".
           05 FILLER                             PIC    X(140)    VALUE "数据文件".
           *>   FILES-HELP

           05 FILLER                             PIC    X(140)    VALUE "Indexed files the assistant may search for this topic. They are only ever read.".
           05 FILLER                             PIC    X(140)    VALUE "Arquivos indexados que o assistente pode consultar neste tópico. Eles são apenas lidos, nunca alterados.".
           05 FILLER                             PIC    X(140)    VALUE "Archivos indexados que el asistente puede consultar en este tema. Solo se leen, nunca se modifican.".
           05 FILLER                             PIC    X(140)    VALUE "Fichiers indexés que l'assistant peut consulter pour ce sujet. Ils sont seulement lus, jamais modifiés.".
           05 FILLER                             PIC    X(140)    VALUE "このトピックでアシスタントが検索できる索引ファイルです。読み取るだけで、変更はしません。".
           05 FILLER                             PIC    X(140)    VALUE "助手在此主题中可以搜索的索引文件。它们只会被读取，从不修改。".
           *>   REMOVE

           05 FILLER                             PIC    X(140)    VALUE "Remove".
           05 FILLER                             PIC    X(140)    VALUE "Remover".
           05 FILLER                             PIC    X(140)    VALUE "Quitar".
           05 FILLER                             PIC    X(140)    VALUE "Retirer".
           05 FILLER                             PIC    X(140)    VALUE "削除".
           05 FILLER                             PIC    X(140)    VALUE "移除".
           *>   DATA-FILE

           05 FILLER                             PIC    X(140)    VALUE "Data file".
           05 FILLER                             PIC    X(140)    VALUE "Arquivo de dados".
           05 FILLER                             PIC    X(140)    VALUE "Archivo de datos".
           05 FILLER                             PIC    X(140)    VALUE "Fichier de données".
           05 FILLER                             PIC    X(140)    VALUE "データファイル".
           05 FILLER                             PIC    X(140)    VALUE "数据文件".
           *>   HINT-DATA-PATH

           05 FILLER                             PIC    X(140)    VALUE "A path, a network path, or smb://server/share/file".
           05 FILLER                             PIC    X(140)    VALUE "Um caminho, um caminho de rede ou smb://servidor/compartilhamento/arquivo".
           05 FILLER                             PIC    X(140)    VALUE "Una ruta, una ruta de red o smb://servidor/recurso/archivo".
           05 FILLER                             PIC    X(140)    VALUE "Un chemin, un chemin réseau ou smb://serveur/partage/fichier".
           05 FILLER                             PIC    X(140)    VALUE "パス、ネットワークパス、または smb://サーバー/共有/ファイル".
           05 FILLER                             PIC    X(140)    VALUE "路径、网络路径或 smb://服务器/共享/文件".
           *>   DESCRIPTION

           05 FILLER                             PIC    X(140)    VALUE "Description".
           05 FILLER                             PIC    X(140)    VALUE "Descrição".
           05 FILLER                             PIC    X(140)    VALUE "Descripción".
           05 FILLER                             PIC    X(140)    VALUE "Description".
           05 FILLER                             PIC    X(140)    VALUE "定義".
           05 FILLER                             PIC    X(140)    VALUE "描述".
           *>   HINT-CIDX

           05 FILLER                             PIC    X(140)    VALUE "Its .cidx".
           05 FILLER                             PIC    X(140)    VALUE "O .cidx dele".
           05 FILLER                             PIC    X(140)    VALUE "Su .cidx".
           05 FILLER                             PIC    X(140)    VALUE "Son .cidx".
           05 FILLER                             PIC    X(140)    VALUE "その .cidx".
           05 FILLER                             PIC    X(140)    VALUE "对应的 .cidx".
           *>   ADD-FILE

           05 FILLER                             PIC    X(140)    VALUE "Add file".
           05 FILLER                             PIC    X(140)    VALUE "Adicionar arquivo".
           05 FILLER                             PIC    X(140)    VALUE "Añadir archivo".
           05 FILLER                             PIC    X(140)    VALUE "Ajouter le fichier".
           05 FILLER                             PIC    X(140)    VALUE "ファイルを追加".
           05 FILLER                             PIC    X(140)    VALUE "添加文件".
           *>   STATUS

           05 FILLER                             PIC    X(140)    VALUE "Status".
           05 FILLER                             PIC    X(140)    VALUE "Status".
           05 FILLER                             PIC    X(140)    VALUE "Estado".
           05 FILLER                             PIC    X(140)    VALUE "État".
           05 FILLER                             PIC    X(140)    VALUE "ステータス".
           05 FILLER                             PIC    X(140)    VALUE "状态".
           *>   TAB-BROWSE

           05 FILLER                             PIC    X(140)    VALUE "Browse".
           05 FILLER                             PIC    X(140)    VALUE "Consultar".
           05 FILLER                             PIC    X(140)    VALUE "Consultar".
           05 FILLER                             PIC    X(140)    VALUE "Consulter".
           05 FILLER                             PIC    X(140)    VALUE "一覧".
           05 FILLER                             PIC    X(140)    VALUE "浏览".
           *>   TAB-EDIT

           05 FILLER                             PIC    X(140)    VALUE "Create/Update".
           05 FILLER                             PIC    X(140)    VALUE "Criar/Atualizar".
           05 FILLER                             PIC    X(140)    VALUE "Crear/Actualizar".
           05 FILLER                             PIC    X(140)    VALUE "Créer/Modifier".
           05 FILLER                             PIC    X(140)    VALUE "作成/更新".
           05 FILLER                             PIC    X(140)    VALUE "创建/更新".
           *>   NEW

           05 FILLER                             PIC    X(140)    VALUE "New".
           05 FILLER                             PIC    X(140)    VALUE "Novo".
           05 FILLER                             PIC    X(140)    VALUE "Nuevo".
           05 FILLER                             PIC    X(140)    VALUE "Nouveau".
           05 FILLER                             PIC    X(140)    VALUE "新規".
           05 FILLER                             PIC    X(140)    VALUE "新建".
           *>   SAVE

           05 FILLER                             PIC    X(140)    VALUE "Save".
           05 FILLER                             PIC    X(140)    VALUE "Salvar".
           05 FILLER                             PIC    X(140)    VALUE "Guardar".
           05 FILLER                             PIC    X(140)    VALUE "Enregistrer".
           05 FILLER                             PIC    X(140)    VALUE "保存".
           05 FILLER                             PIC    X(140)    VALUE "保存".
           *>   CANCEL

           05 FILLER                             PIC    X(140)    VALUE "Cancel".
           05 FILLER                             PIC    X(140)    VALUE "Cancelar".
           05 FILLER                             PIC    X(140)    VALUE "Cancelar".
           05 FILLER                             PIC    X(140)    VALUE "Annuler".
           05 FILLER                             PIC    X(140)    VALUE "キャンセル".
           05 FILLER                             PIC    X(140)    VALUE "取消".
           *>   YES

           05 FILLER                             PIC    X(140)    VALUE "Yes".
           05 FILLER                             PIC    X(140)    VALUE "Sim".
           05 FILLER                             PIC    X(140)    VALUE "Sí".
           05 FILLER                             PIC    X(140)    VALUE "Oui".
           05 FILLER                             PIC    X(140)    VALUE "はい".
           05 FILLER                             PIC    X(140)    VALUE "是".
           *>   NO

           05 FILLER                             PIC    X(140)    VALUE "No".
           05 FILLER                             PIC    X(140)    VALUE "Não".
           05 FILLER                             PIC    X(140)    VALUE "No".
           05 FILLER                             PIC    X(140)    VALUE "Non".
           05 FILLER                             PIC    X(140)    VALUE "いいえ".
           05 FILLER                             PIC    X(140)    VALUE "否".
           *>   ASK-REMOVE

           05 FILLER                             PIC    X(140)    VALUE "Remove &1 from this topic? The file itself is not touched.".
           05 FILLER                             PIC    X(140)    VALUE "Remover &1 deste tópico? O arquivo em si não é alterado.".
           05 FILLER                             PIC    X(140)    VALUE "¿Quitar &1 de este tema? El archivo en sí no se modifica.".
           05 FILLER                             PIC    X(140)    VALUE "Retirer &1 de ce sujet ? Le fichier lui-même n'est pas modifié.".
           05 FILLER                             PIC    X(140)    VALUE "&1 をこのトピックから外しますか？ファイル自体は変更されません。".
           05 FILLER                             PIC    X(140)    VALUE "从此主题中移除 &1？文件本身不会被改动。".
           *>   FILE-UPDATED

           05 FILLER                             PIC    X(140)    VALUE "Updated &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Atualizado &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Actualizado &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "Mis à jour &1 - &2".
           05 FILLER                             PIC    X(140)    VALUE "&1 を更新しました - &2".
           05 FILLER                             PIC    X(140)    VALUE "已更新 &1 - &2".
           *>   NOT-SAVED

           05 FILLER                             PIC    X(140)    VALUE "Not saved - file status &1.".
           05 FILLER                             PIC    X(140)    VALUE "Não salvo - status do arquivo &1.".
           05 FILLER                             PIC    X(140)    VALUE "No se guardó - estado del archivo &1.".
           05 FILLER                             PIC    X(140)    VALUE "Non enregistré - état du fichier &1.".
           05 FILLER                             PIC    X(140)    VALUE "保存されませんでした - ファイル状態 &1。".
           05 FILLER                             PIC    X(140)    VALUE "未保存 - 文件状态 &1。".
           *>   COL-FILE

           05 FILLER                             PIC    X(140)    VALUE "File".
           05 FILLER                             PIC    X(140)    VALUE "Arquivo".
           05 FILLER                             PIC    X(140)    VALUE "Archivo".
           05 FILLER                             PIC    X(140)    VALUE "Fichier".
           05 FILLER                             PIC    X(140)    VALUE "ファイル".
           05 FILLER                             PIC    X(140)    VALUE "文件".
           *>   BROWSE

           05 FILLER                             PIC    X(140)    VALUE "Browse...".
           05 FILLER                             PIC    X(140)    VALUE "Procurar...".
           05 FILLER                             PIC    X(140)    VALUE "Examinar...".
           05 FILLER                             PIC    X(140)    VALUE "Parcourir...".
           05 FILLER                             PIC    X(140)    VALUE "参照...".
           05 FILLER                             PIC    X(140)    VALUE "浏览...".
           *>   PICK-DATA

           05 FILLER                             PIC    X(140)    VALUE "Choose the indexed data file".
           05 FILLER                             PIC    X(140)    VALUE "Escolha o arquivo de dados indexado".
           05 FILLER                             PIC    X(140)    VALUE "Elige el archivo de datos indexado".
           05 FILLER                             PIC    X(140)    VALUE "Choisissez le fichier de données indexé".
           05 FILLER                             PIC    X(140)    VALUE "索引データファイルを選択".
           05 FILLER                             PIC    X(140)    VALUE "选择索引数据文件".
           *>   PICK-CIDX

           05 FILLER                             PIC    X(140)    VALUE "Choose its description (.cidx)".
           05 FILLER                             PIC    X(140)    VALUE "Escolha a descrição dele (.cidx)".
           05 FILLER                             PIC    X(140)    VALUE "Elige su descripción (.cidx)".
           05 FILLER                             PIC    X(140)    VALUE "Choisissez sa description (.cidx)".
           05 FILLER                             PIC    X(140)    VALUE "その記述ファイル (.cidx) を選択".
           05 FILLER                             PIC    X(140)    VALUE "选择它的描述文件 (.cidx)".
           *>   FILTER-IDX

           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL indexed files".
           05 FILLER                             PIC    X(140)    VALUE "Arquivos indexados RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "Archivos indexados RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "Fichiers indexés RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL 索引ファイル".
           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL 索引文件".
           *>   FILTER-CIDX

           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL file descriptions".
           05 FILLER                             PIC    X(140)    VALUE "Descrições de arquivo RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "Descripciones de archivo RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "Descriptions de fichier RustCOBOL".
           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL ファイル記述".
           05 FILLER                             PIC    X(140)    VALUE "RustCOBOL 文件描述".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    32.
               10 PC-TEXT                        PIC    X(140)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-OPEN-TOPIC-FIRST                 PIC    X(140).
           05 T-FILES-OF                         PIC    X(140).
           05 T-PICK-FILE-FIRST                  PIC    X(140).
           05 T-FILE-REMOVED                     PIC    X(140).
           05 T-GIVE-BOTH                        PIC    X(140).
           05 T-NOT-ADDED                        PIC    X(140).
           05 T-FILE-ADDED                       PIC    X(140).
           05 T-FILES-TITLE                      PIC    X(140).
           05 T-FILES-HELP                       PIC    X(140).
           05 T-REMOVE                           PIC    X(140).
           05 T-DATA-FILE                        PIC    X(140).
           05 T-HINT-DATA-PATH                   PIC    X(140).
           05 T-DESCRIPTION                      PIC    X(140).
           05 T-HINT-CIDX                        PIC    X(140).
           05 T-ADD-FILE                         PIC    X(140).
           05 T-STATUS                           PIC    X(140).
           05 T-TAB-BROWSE                       PIC    X(140).
           05 T-TAB-EDIT                         PIC    X(140).
           05 T-NEW                              PIC    X(140).
           05 T-SAVE                             PIC    X(140).
           05 T-CANCEL                           PIC    X(140).
           05 T-YES                              PIC    X(140).
           05 T-NO                               PIC    X(140).
           05 T-ASK-REMOVE                       PIC    X(140).
           05 T-FILE-UPDATED                     PIC    X(140).
           05 T-NOT-SAVED                        PIC    X(140).
           05 T-COL-FILE                         PIC    X(140).
           05 T-BROWSE                           PIC    X(140).
           05 T-PICK-DATA                        PIC    X(140).
           05 T-PICK-CIDX                        PIC    X(140).
           05 T-FILTER-IDX                       PIC    X(140).
           05 T-FILTER-CIDX                      PIC    X(140).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(140)    OCCURS 32.
       *>   The platform's Open panel: its "Description|ext" filter and the
       *>   path it answers with (spaces when the operator cancels).

       01 WS-FILTER                              GLOBAL    PIC X(200).
       01 WS-PICKED                              GLOBAL    PIC X(300).
       *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.

       01 WS-FMT                                 GLOBAL    PIC X(140).
       01 WS-ARG1                                GLOBAL    PIC X(300).
       01 WS-ARG2                                GLOBAL    PIC X(300).
       01 WS-ARG3                                GLOBAL    PIC X(300).
       01 WS-ARG4                                GLOBAL    PIC X(300).
       01 WS-FMT-OUT                             GLOBAL    PIC X(1200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Data files'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Help.
          05 WS-Lbl-Help-TEXT       PIC X(256) VALUE 'Indexed files the assistant may search for this topic. They are only ever read.'.
          05 WS-Lbl-Help-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Help-ENABLED    PIC 9      VALUE 1.

       01 WS-Tab-Crud.
          05 WS-Tab-Crud-TEXT       PIC X(256) VALUE 'Tab-Crud'.
          05 WS-Tab-Crud-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tab-Crud-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Data.
          05 WS-Lbl-Data-TEXT       PIC X(256) VALUE 'Data file'.
          05 WS-Lbl-Data-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Data-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Data.
          05 WS-Txt-Data-TEXT       PIC X(250) VALUE SPACES.
          05 WS-Txt-Data-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Data-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Data-VALUE      PIC X(250) VALUE SPACES.

       01 WS-Lbl-Cidx.
          05 WS-Lbl-Cidx-TEXT       PIC X(256) VALUE 'Description'.
          05 WS-Lbl-Cidx-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Cidx-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Cidx.
          05 WS-Txt-Cidx-TEXT       PIC X(250) VALUE SPACES.
          05 WS-Txt-Cidx-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Cidx-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Cidx-VALUE      PIC X(250) VALUE SPACES.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-AGENT-F.
          05 WS-AGENT-F-TEXT       PIC X(256) VALUE 'AGENT-F'.
          05 WS-AGENT-F-VISIBLE    PIC 9      VALUE 1.
          05 WS-AGENT-F-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Lang.
          05 WS-Tmr-Lang-TEXT       PIC X(256) VALUE 'Tmr-Lang'.
          05 WS-Tmr-Lang-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Lang-ENABLED    PIC 9      VALUE 1.

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

       01 WS-Btn-BrowseData.
          05 WS-Btn-BrowseData-TEXT       PIC X(256) VALUE 'Browse...'.
          05 WS-Btn-BrowseData-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-BrowseData-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-BrowseCidx.
          05 WS-Btn-BrowseCidx-TEXT       PIC X(256) VALUE 'Browse...'.
          05 WS-Btn-BrowseCidx-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-BrowseCidx-ENABLED    PIC 9      VALUE 1.

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
           CALL "FILES-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "FILES-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "FILES-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onActivate"
                               CALL "FILES-FORM--ONACTIVATE"
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
                   WHEN "Btn-BrowseData"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-BROWSEDATA--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-BrowseCidx"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-BROWSECIDX--ONCLICK"
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
       AGENT-F-ASK.
      *>    Ask the AI agent AGENT-F (model: llama3.2, endpoint: http://localhost:11434)
      *>    Set WS-AGENT-PROMPT before calling.
      *>    Returns at once. The reply arrives as AGENT-F--ONRESPONSE
      *>    (read LastReply) or --ONERROR (read LastError).
           INVOKE AGENT-F 'Ask'
               USING BY VALUE WS-AGENT-PROMPT.

       AGENT-F-ON-RESPONSE.
      *>    TODO: AGENT-F — not called by the runtime; bind onResponse and read LastReply
           CONTINUE.

       AGENT-F-ON-ERROR.
      *>    TODO: AGENT-F — not called by the runtime; bind onError and read LastError
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILES-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM FILES-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILES-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back on the pane: the topic or the language may have changed.

           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM FILES-FORM--ONACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILES-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM FILES-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-LANG--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the chat's menu changes the language while this form is
           *>   on the pane; nothing tells a pane occupant, so it looks (R46).

           CALL    "PC-LANG-NOW"
           IF    WS-LANG-NOW NOT = WS-LANG
               CALL    "FILES-FORM--ONACTIVATE"
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
       PROGRAM-ID. DG-LIST--ONCELLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           SET     WS-ROW-NO                     TO    Dg-List::ClickedRow
           SET     WS-COL-NO                     TO    Dg-List::ClickedColumn

           IF    WS-ROW-NO < 1 OR WS-ROW-NO > WS-FILE-COUNT
               EXIT PROGRAM
           END-IF
           *>   Column 4 is the pencil, column 5 the trash can.

           EVALUATE    WS-COL-NO
               WHEN 4 CALL "PC-EDIT"
               WHEN 5 CALL "PC-DELETE"
           END-EVALUATE

           GOBACK.

       END PROGRAM DG-LIST--ONCELLCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-BROWSEDATA--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The platform's own Open panel, showing RustCOBOL indexed files
           *>   only; the choice lands in the field and waits for Save.

           SET     WS-FILTER                     TO    SPACES
           STRING    T-FILTER-IDX::Trim() "|idx" INTO WS-FILTER
           SET     WS-PICKED                     TO    SPACES
           COBOL::"OPEN-FILE-DIALOG" ( T-PICK-DATA WS-FILTER WS-PICKED )
           IF    WS-PICKED NOT = SPACES
               SET     Txt-Data::Text            TO    WS-PICKED
           END-IF

           GOBACK.

       END PROGRAM BTN-BROWSEDATA--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-BROWSECIDX--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The same, for the file's description: .cidx files only.

           SET     WS-FILTER                     TO    SPACES
           STRING    T-FILTER-CIDX::Trim() "|cidx" INTO WS-FILTER
           SET     WS-PICKED                     TO    SPACES
           COBOL::"OPEN-FILE-DIALOG" ( T-PICK-CIDX WS-FILTER WS-PICKED )
           IF    WS-PICKED NOT = SPACES
               SET     Txt-Cidx::Text            TO    WS-PICKED
           END-IF

           GOBACK.

       END PROGRAM BTN-BROWSECIDX--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCEL2--ONCLICK.

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
           SET     WS-TFILES-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/topic-files.idx" INTO WS-TFILES-PATH

           GOBACK.

       END PROGRAM PC-PATHS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-GET IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-SET-NAME in, WS-SET-VALUE out (spaces when the setting is unset).

           SET     WS-SET-VALUE                  TO    SPACES
           *> Status 35 = the file is not there yet (first run): create it empty and
           *> reopen it, so the read below finds a file without the setting rather
           *> than failing.

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

           *> One record per setting name: WRITE adds a new name, INVALID KEY (the
           *> name is already there) turns it into an update in place.

           WRITE    SETTINGS-REC
               INVALID KEY REWRITE SETTINGS-REC
           END-WRITE
           COMMIT
           CLOSE    SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-PUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TRY IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-DATA, WS-CIDX in: register the file on a scratch agent to learn
           *>   whether it can be used and how - WS-OK, WS-NAME, WS-RESULT and
           *>   WS-MESSAGE out - then withdraw it again. The chat form registers
           *>   the topic's files for real when the topic opens.

           SET     WS-OK                         TO    AGENT-F::RegisterFile(WS-DATA::Trim(), WS-CIDX::Trim())
           SET     WS-RESULT                     TO    AGENT-F::RegisterResult
           SET     WS-MESSAGE                    TO    AGENT-F::RegisterMessage
           SET     WS-NAME                       TO    AGENT-F::RegisteredName

           IF    WS-OK = "1"
               SET     WS-DISCARD                TO    AGENT-F::UnregisterFile(WS-NAME::Trim())
           END-IF

           GOBACK.

       END PROGRAM PC-TRY.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LOAD-FILES IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The topic's registered files, each checked as it is listed. Grid
           *>   row N is the file whose key sequence is WS-FILE-SEQ(N).

           SET     WS-DISCARD                    TO    Dg-List::ClearRows()
           SET     WS-FILE-COUNT WS-NEXT-SEQ     TO    0

           OPEN    I-O TFILES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TFILES-FILE
               CLOSE    TFILES-FILE
               OPEN    I-O TFILES-FILE
           END-IF

           *> TF-KEY is topic then sequence: from sequence 0 the topic's files come
           *> in order, and the first record of another topic ends the scan.

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
                           IF    WS-FILE-COUNT < 100
                               ADD    1 TO WS-FILE-COUNT
                               SET     WS-FILE-SEQ(WS-FILE-COUNT)
                                                 TO    TF-SEQ

                               SET     WS-NEXT-SEQ
                                                 TO    TF-SEQ

                               SET     WS-DATA   TO    TF-DATA
                               SET     WS-CIDX   TO    TF-CIDX

                               CALL    "PC-TRY"
                               SET     WS-ROW    TO    SPACES
                               STRING    FUNCTION TRIM(TF-NAME)
                                   "  [" WS-RESULT::Trim() "]" X"09"
                                   FUNCTION TRIM(TF-DATA) X"09"
                                   FUNCTION TRIM(TF-CIDX) X"09"
                                   "icon:pencil" X"09" "icon:trash"
                                   INTO WS-ROW
                               SET     WS-DISCARD
                                                 TO    Dg-List::AddRow(WS-ROW)
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           CLOSE    TFILES-FILE
           *> Keys came in order, so WS-NEXT-SEQ holds the last sequence read:
           *> one more is the key PC-SAVE gives a new file.

           ADD    1 TO WS-NEXT-SEQ

           GOBACK.

       END PROGRAM PC-LOAD-FILES.

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

           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 32
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Lbl-Title::Caption            TO    T-FILES-TITLE::Trim()
           SET     Lbl-Help::Caption             TO    T-FILES-HELP::Trim()
           SET     WS-TABS                       TO    SPACES

           STRING    T-TAB-BROWSE::Trim() X"0A" T-TAB-EDIT::Trim() INTO WS-TABS
           SET     Tab-Crud::Tabs                TO    WS-TABS
           *>   The grid's headings: the data columns, by their designed id.

           Dg-List::SetColumnTitle("Col-Name", T-COL-FILE::Trim())
           Dg-List::SetColumnTitle("Col-Data", T-DATA-FILE::Trim())
           Dg-List::SetColumnTitle("Col-Cidx", T-DESCRIPTION::Trim())

           SET     Btn-New::Caption              TO    T-NEW::Trim()
           SET     Btn-Save::Caption             TO    T-SAVE::Trim()
           SET     Btn-Save2::Caption            TO    T-SAVE::Trim()
           SET     Btn-Cancel::Caption           TO    T-CANCEL::Trim()
           SET     Btn-Cancel2::Caption          TO    T-CANCEL::Trim()
           SET     Lbl-Data::Caption             TO    T-DATA-FILE::Trim()
           SET     Txt-Data::HintText            TO    T-HINT-DATA-PATH::Trim()
           SET     Lbl-Cidx::Caption             TO    T-DESCRIPTION::Trim()
           SET     Txt-Cidx::HintText            TO    T-HINT-CIDX::Trim()
           SET     Btn-BrowseData::Caption       TO    T-BROWSE::Trim()
           SET     Btn-BrowseCidx::Caption       TO    T-BROWSE::Trim()
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
       01 WS-PART                                PIC    X(140).
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
           CALL    "PC-TEXTS"

           SET     WS-SET-NAME                   TO    "CUR-TOPIC"
           CALL    "PC-SETTING-GET"
           IF    WS-SET-VALUE(1:16) NOT = WS-CUR-TOPIC
               *>       Another topic: a half-edited file of the old one is dropped.

               CALL    "PC-CANCEL"
           END-IF
           SET     WS-CUR-TOPIC                  TO    WS-SET-VALUE(1:16)
           IF    WS-CUR-TOPIC = SPACES
               SET     WS-DISCARD                TO    Dg-List::ClearRows()
               SET     WS-FILE-COUNT             TO    0
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

           SET     WS-FMT                        TO    T-FILES-OF
           SET     WS-ARG1                       TO    TOP-NAME

           CALL    "PC-FMT"
           SET     Lbl-Title::Caption            TO    WS-FMT-OUT::Trim()
           CALL    "PC-LOAD-FILES"

           GOBACK.

       END PROGRAM PC-OPEN.

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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-NEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   An empty Create/Update page for a new file of the current topic.

           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Status::Caption       TO    T-OPEN-TOPIC-FIRST::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-EDIT-MODE                  TO    "N"
           SET     WS-EDIT-SEQ                   TO    0
           SET     Txt-Data::Text                TO    SPACES
           SET     Txt-Cidx::Text                TO    SPACES
           SET     Tab-Crud::SelectedTab         TO    2

           GOBACK.

       END PROGRAM PC-NEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-EDIT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-ROW-NO in: that grid row's file, loaded into Create/Update.

           SET     WS-SAVE-OK                    TO    "N"
           OPEN    I-O TFILES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TFILES-FILE
               CLOSE    TFILES-FILE
               OPEN    I-O TFILES-FILE
           END-IF

           SET     TF-TOPIC                      TO    WS-CUR-TOPIC
           SET     TF-SEQ                        TO    WS-FILE-SEQ(WS-ROW-NO)

           READ    TFILES-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE "Y" TO WS-SAVE-OK
           END-READ
           CLOSE    TFILES-FILE
           IF    WS-SAVE-OK NOT = "Y"
               SET     Lbl-Status::Caption       TO    T-PICK-FILE-FIRST::Trim()
               CALL    "PC-LOAD-FILES"
               EXIT PROGRAM
           END-IF

           SET     WS-EDIT-MODE                  TO    "E"
           SET     WS-EDIT-SEQ                   TO    TF-SEQ
           SET     Txt-Data::Text                TO    FUNCTION TRIM(TF-DATA)
           SET     Txt-Cidx::Text                TO    FUNCTION TRIM(TF-CIDX)
           SET     Tab-Crud::SelectedTab         TO    2

           GOBACK.

       END PROGRAM PC-EDIT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A file is only kept when it can be registered: its .cidx describes
           *>   it, and it can be read (R23-R25). Otherwise the reason is shown and
           *>   the Create/Update page stays open. A new file takes the next key
           *>   sequence; an edited one keeps its own, so WRITE finds it taken and
           *>   REWRITE replaces it.

           IF    WS-CUR-TOPIC = SPACES
               SET     Lbl-Status::Caption       TO    T-OPEN-TOPIC-FIRST::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-DATA                       TO    Txt-Data::Text
           SET     WS-CIDX                       TO    Txt-Cidx::Text

           IF    WS-DATA = SPACES OR WS-CIDX = SPACES
               SET     Lbl-Status::Caption       TO    T-GIVE-BOTH::Trim()
               EXIT PROGRAM
           END-IF
           CALL    "PC-TRY"
           IF    WS-OK NOT = "1"
               SET     WS-FMT                    TO    T-NOT-ADDED
               SET     WS-ARG1                   TO    WS-RESULT
               SET     WS-ARG2                   TO    WS-MESSAGE

               CALL    "PC-FMT"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           OPEN    I-O TFILES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TFILES-FILE
               CLOSE    TFILES-FILE
               OPEN    I-O TFILES-FILE
           END-IF
           SET     TF-TOPIC                      TO    WS-CUR-TOPIC
           IF    WS-EDIT-MODE = "E"
               SET     TF-SEQ                    TO    WS-EDIT-SEQ
           ELSE
               SET     TF-SEQ                    TO    WS-NEXT-SEQ
           END-IF

           SET     TF-NAME                       TO    WS-NAME
           SET     TF-DATA                       TO    WS-DATA
           SET     TF-CIDX                       TO    WS-CIDX
           SET     WS-SAVE-OK                    TO    "Y"

           WRITE    TFILE-REC
               INVALID KEY
                   REWRITE    TFILE-REC
                       INVALID KEY MOVE "N" TO WS-SAVE-OK
                   END-REWRITE
           END-WRITE
           COMMIT
           CLOSE    TFILES-FILE
           IF    WS-SAVE-OK NOT = "Y"
               SET     WS-FMT                    TO    T-NOT-SAVED
               SET     WS-ARG1                   TO    WS-FS

               CALL    "PC-FMT"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           IF    WS-EDIT-MODE = "E"
               SET     WS-FMT                    TO    T-FILE-UPDATED
           ELSE
               SET     WS-FMT                    TO    T-FILE-ADDED
           END-IF

           SET     WS-ARG1                       TO    WS-NAME
           SET     WS-ARG2                       TO    WS-MESSAGE

           CALL    "PC-FMT"

           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()
           SET     WS-EDIT-MODE                  TO    "N"
           SET     Txt-Data::Text                TO    SPACES
           SET     Txt-Cidx::Text                TO    SPACES

           CALL    "PC-LOAD-FILES"
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-CANCEL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back to Browse as it was: nothing saved, nothing reloaded.

           SET     WS-EDIT-MODE                  TO    "N"
           SET     Txt-Data::Text                TO    SPACES
           SET     Txt-Cidx::Text                TO    SPACES
           SET     Tab-Crud::SelectedTab         TO    1

           GOBACK.

       END PROGRAM PC-CANCEL.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-DELETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   WS-ROW-NO in: after a yes, that file leaves the topic's list. The
           *>   file itself is untouched; the chat registers only what the list
           *>   holds when the topic opens, so the record is all there is to drop.

           OPEN    I-O TFILES-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT TFILES-FILE
               CLOSE    TFILES-FILE
               OPEN    I-O TFILES-FILE
           END-IF

           SET     TF-TOPIC                      TO    WS-CUR-TOPIC
           SET     TF-SEQ                        TO    WS-FILE-SEQ(WS-ROW-NO)
           SET     WS-SAVE-OK                    TO    "N"

           READ    TFILES-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE "Y" TO WS-SAVE-OK
           END-READ
           CLOSE    TFILES-FILE
           IF    WS-SAVE-OK NOT = "Y"
               SET     Lbl-Status::Caption       TO    T-PICK-FILE-FIRST::Trim()
               CALL    "PC-LOAD-FILES"
               EXIT PROGRAM
           END-IF
           SET     WS-FMT                        TO    T-ASK-REMOVE
           IF    TF-NAME = SPACES
               SET     WS-ARG1                   TO    TF-DATA
           ELSE
               SET     WS-ARG1                   TO    TF-NAME
           END-IF
           CALL    "PC-FMT"
           SET     WS-QUESTION                   TO    WS-FMT-OUT

           INVOKE    ME::SetProperty("ConfirmText", WS-QUESTION)
           INVOKE    ME::SetProperty("ConfirmYes", T-YES)
           INVOKE    ME::SetProperty("ConfirmNo", T-NO)
           INVOKE    ME::SetProperty("ConfirmAnswer", "N")
           INVOKE    ME::OpenFormSync("CONFIRM-FORM")
           INVOKE    ME::GetProperty("ConfirmAnswer") RETURNING WS-ANSWER

           IF    WS-ANSWER NOT = "Y"
               EXIT PROGRAM
           END-IF
           OPEN    I-O TFILES-FILE

           SET     TF-TOPIC                      TO    WS-CUR-TOPIC
           SET     TF-SEQ                        TO    WS-FILE-SEQ(WS-ROW-NO)

           DELETE    TFILES-FILE
               INVALID KEY CONTINUE
           END-DELETE
           COMMIT
           CLOSE    TFILES-FILE
           SET     Lbl-Status::Caption           TO    T-FILE-REMOVED::Trim()
           CALL    "PC-LOAD-FILES"

           GOBACK.

       END PROGRAM PC-DELETE.

       END PROGRAM FILES-FORM.

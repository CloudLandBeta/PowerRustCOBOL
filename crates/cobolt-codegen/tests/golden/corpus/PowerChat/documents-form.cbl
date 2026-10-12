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
       PROGRAM-ID. DOCUMENTS-FORM.

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
           SELECT FOLDERS-FILE ASSIGN TO WS-FOLDERS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS FLD-KEY
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
       FD FOLDERS-FILE IS GLOBAL.
       *>   A topic's document folders (R49), so a folder exists before its
       *>   first document does.

       01 FOLDER-REC.
           05 FLD-KEY.
               10 FLD-TOPIC                      PIC    X(16).
               10 FLD-PATH                       PIC    X(200).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'DOCUMENTS-FORM'.

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
       01 WS-CUR-TOPIC                           GLOBAL    PIC X(16).
       01 WS-KB-LOCATION                         GLOBAL    PIC X(200).
       01 WS-DOC-COUNT                           GLOBAL    PIC 9(5)    VALUE 0.
       01 WS-I                                   GLOBAL    PIC 9(5).
       01 WS-INDEX                               GLOBAL    PIC S9(5).
       01 WS-DOC                                 GLOBAL    PIC X(260).
       01 WS-LINE                                GLOBAL    PIC X(400).
       01 WS-N1                                  GLOBAL    PIC Z(4)9.
       01 WS-N2                                  GLOBAL    PIC Z(4)9.
       01 WS-N3                                  GLOBAL    PIC Z(4)9.
       01 WS-N4                                  GLOBAL    PIC Z(4)9.
       01 WS-SKIPPED                             GLOBAL    PIC X(400).
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-NOW                            GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   OPEN-TOPIC-FIRST

           05 FILLER                             PIC    X(70)    VALUE "Open a topic first.".
           05 FILLER                             PIC    X(70)    VALUE "Abra um tópico primeiro.".
           05 FILLER                             PIC    X(70)    VALUE "Primero abre un tema.".
           05 FILLER                             PIC    X(70)    VALUE "Ouvrez d'abord un sujet.".
           05 FILLER                             PIC    X(70)    VALUE "先にトピックを開いてください。".
           05 FILLER                             PIC    X(70)    VALUE "请先打开一个主题。".
           *>   DOCS-OF

           05 FILLER                             PIC    X(70)    VALUE "Documents - &1".
           05 FILLER                             PIC    X(70)    VALUE "Documentos - &1".
           05 FILLER                             PIC    X(70)    VALUE "Documentos - &1".
           05 FILLER                             PIC    X(70)    VALUE "Documents - &1".
           05 FILLER                             PIC    X(70)    VALUE "文書 - &1".
           05 FILLER                             PIC    X(70)    VALUE "文档 - &1".
           *>   PICK-DOC-FIRST

           05 FILLER                             PIC    X(70)    VALUE "Pick a document or a folder in the tree first.".
           05 FILLER                             PIC    X(70)    VALUE "Escolha um documento ou uma pasta na árvore primeiro.".
           05 FILLER                             PIC    X(70)    VALUE "Primero elige un documento o una carpeta del árbol.".
           05 FILLER                             PIC    X(70)    VALUE "Choisissez d'abord un document ou un dossier.".
           05 FILLER                             PIC    X(70)    VALUE "先に文書かフォルダーを選んでください。".
           05 FILLER                             PIC    X(70)    VALUE "请先在树中选择文档或文件夹。".
           *>   KB-PROGRESS

           05 FILLER                             PIC    X(70)    VALUE "Updating the Knowledge Base: &1 of &2 - &3".
           05 FILLER                             PIC    X(70)    VALUE "Atualizando a base de conhecimento: &1 de &2 - &3".
           05 FILLER                             PIC    X(70)    VALUE "Actualizando la base de conocimiento: &1 de &2 - &3".
           05 FILLER                             PIC    X(70)    VALUE "Mise à jour de la base de connaissances : &1 sur &2 - &3".
           05 FILLER                             PIC    X(70)    VALUE "ナレッジベースを更新中：&1 / &2 - &3".
           05 FILLER                             PIC    X(70)    VALUE "正在更新知识库：第 &1 个，共 &2 个 - &3".
           *>   KB-COUNTS

           05 FILLER                             PIC    X(70)    VALUE "Added &1, updated &2, removed &3, skipped &4.".
           05 FILLER                             PIC    X(70)    VALUE "Adicionados &1, atualizados &2, removidos &3, ignorados &4.".
           05 FILLER                             PIC    X(70)    VALUE "Añadidos &1, actualizados &2, quitados &3, omitidos &4.".
           05 FILLER                             PIC    X(70)    VALUE "Ajoutés &1, mis à jour &2, retirés &3, ignorés &4.".
           05 FILLER                             PIC    X(70)    VALUE "追加 &1、更新 &2、削除 &3、スキップ &4。".
           05 FILLER                             PIC    X(70)    VALUE "新增 &1，更新 &2，移除 &3，跳过 &4。".
           *>   KB-SKIPPED

           05 FILLER                             PIC    X(70)    VALUE "Skipped: &1".
           05 FILLER                             PIC    X(70)    VALUE "Ignorados: &1".
           05 FILLER                             PIC    X(70)    VALUE "Omitidos: &1".
           05 FILLER                             PIC    X(70)    VALUE "Ignorés : &1".
           05 FILLER                             PIC    X(70)    VALUE "スキップ：&1".
           05 FILLER                             PIC    X(70)    VALUE "已跳过：&1".
           *>   DOC-COUNT

           05 FILLER                             PIC    X(70)    VALUE "&1 document(s).".
           05 FILLER                             PIC    X(70)    VALUE "&1 documento(s).".
           05 FILLER                             PIC    X(70)    VALUE "&1 documento(s).".
           05 FILLER                             PIC    X(70)    VALUE "&1 document(s).".
           05 FILLER                             PIC    X(70)    VALUE "文書 &1 件。".
           05 FILLER                             PIC    X(70)    VALUE "共 &1 个文档。".
           *>   DOCS-TITLE

           05 FILLER                             PIC    X(70)    VALUE "Documents".
           05 FILLER                             PIC    X(70)    VALUE "Documentos".
           05 FILLER                             PIC    X(70)    VALUE "Documentos".
           05 FILLER                             PIC    X(70)    VALUE "Documents".
           05 FILLER                             PIC    X(70)    VALUE "文書".
           05 FILLER                             PIC    X(70)    VALUE "文档".
           *>   DROP-HINT

           05 FILLER                             PIC    X(70)    VALUE "Drop documents here, or click to choose".
           05 FILLER                             PIC    X(70)    VALUE "Solte documentos aqui ou clique para escolher".
           05 FILLER                             PIC    X(70)    VALUE "Suelta documentos aquí o haz clic para elegir".
           05 FILLER                             PIC    X(70)    VALUE "Déposez des documents ici, ou cliquez pour choisir".
           05 FILLER                             PIC    X(70)    VALUE "ここに文書をドロップするか、クリックして選択".
           05 FILLER                             PIC    X(70)    VALUE "将文档拖放到此处，或点击选择".
           *>   DELETE

           05 FILLER                             PIC    X(70)    VALUE "Delete".
           05 FILLER                             PIC    X(70)    VALUE "Excluir".
           05 FILLER                             PIC    X(70)    VALUE "Eliminar".
           05 FILLER                             PIC    X(70)    VALUE "Supprimer".
           05 FILLER                             PIC    X(70)    VALUE "削除".
           05 FILLER                             PIC    X(70)    VALUE "删除".
           *>   REFRESH

           05 FILLER                             PIC    X(70)    VALUE "Refresh".
           05 FILLER                             PIC    X(70)    VALUE "Atualizar".
           05 FILLER                             PIC    X(70)    VALUE "Actualizar".
           05 FILLER                             PIC    X(70)    VALUE "Actualiser".
           05 FILLER                             PIC    X(70)    VALUE "更新".
           05 FILLER                             PIC    X(70)    VALUE "刷新".
           *>   KB-UPDATING

           05 FILLER                             PIC    X(70)    VALUE "Updating the Knowledge Base...".
           05 FILLER                             PIC    X(70)    VALUE "Atualizando a base de conhecimento...".
           05 FILLER                             PIC    X(70)    VALUE "Actualizando la base de conocimiento...".
           05 FILLER                             PIC    X(70)    VALUE "Mise à jour de la base de connaissances...".
           05 FILLER                             PIC    X(70)    VALUE "ナレッジベースを更新中...".
           05 FILLER                             PIC    X(70)    VALUE "正在更新知识库...".
           *>   NEW-FOLDER

           05 FILLER                             PIC    X(70)    VALUE "New folder".
           05 FILLER                             PIC    X(70)    VALUE "Nova pasta".
           05 FILLER                             PIC    X(70)    VALUE "Nueva carpeta".
           05 FILLER                             PIC    X(70)    VALUE "Nouveau dossier".
           05 FILLER                             PIC    X(70)    VALUE "新しいフォルダー".
           05 FILLER                             PIC    X(70)    VALUE "新建文件夹".
           *>   FOLDER-HINT

           05 FILLER                             PIC    X(70)    VALUE "Folder name".
           05 FILLER                             PIC    X(70)    VALUE "Nome da pasta".
           05 FILLER                             PIC    X(70)    VALUE "Nombre de la carpeta".
           05 FILLER                             PIC    X(70)    VALUE "Nom du dossier".
           05 FILLER                             PIC    X(70)    VALUE "フォルダー名".
           05 FILLER                             PIC    X(70)    VALUE "文件夹名称".
           *>   FOLDER-CREATED

           05 FILLER                             PIC    X(70)    VALUE "Folder created: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Pasta criada: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Carpeta creada: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Dossier créé : &1.".
           05 FILLER                             PIC    X(70)    VALUE "フォルダーを作成しました: &1".
           05 FILLER                             PIC    X(70)    VALUE "已创建文件夹：&1".
           *>   FOLDER-EXISTS

           05 FILLER                             PIC    X(70)    VALUE "That folder already exists.".
           05 FILLER                             PIC    X(70)    VALUE "Essa pasta já existe.".
           05 FILLER                             PIC    X(70)    VALUE "Esa carpeta ya existe.".
           05 FILLER                             PIC    X(70)    VALUE "Ce dossier existe déjà.".
           05 FILLER                             PIC    X(70)    VALUE "そのフォルダーは既にあります。".
           05 FILLER                             PIC    X(70)    VALUE "该文件夹已存在。".
           *>   FOLDER-NAME-BAD

           05 FILLER                             PIC    X(70)    VALUE "Type a folder name, without / or \.".
           05 FILLER                             PIC    X(70)    VALUE "Digite um nome de pasta, sem / nem \.".
           05 FILLER                             PIC    X(70)    VALUE "Escribe un nombre de carpeta, sin / ni \.".
           05 FILLER                             PIC    X(70)    VALUE "Saisissez un nom de dossier, sans / ni \.".
           05 FILLER                             PIC    X(70)    VALUE "フォルダー名を入力（/ と \ は不可）。".
           05 FILLER                             PIC    X(70)    VALUE "请输入文件夹名称（不含 / 或 \）。".
           *>   FOLDER-NOT-EMPTY

           05 FILLER                             PIC    X(70)    VALUE "The folder is not empty: delete its documents first.".
           05 FILLER                             PIC    X(70)    VALUE "A pasta não está vazia: exclua os documentos dela primeiro.".
           05 FILLER                             PIC    X(70)    VALUE "La carpeta no está vacía: borra primero sus documentos.".
           05 FILLER                             PIC    X(70)    VALUE "Le dossier n'est pas vide : supprimez d'abord ses documents.".
           05 FILLER                             PIC    X(70)    VALUE "空でないフォルダーは削除できません。".
           05 FILLER                             PIC    X(70)    VALUE "文件夹不为空，请先删除其中的文档。".
           *>   FOLDER-DELETED

           05 FILLER                             PIC    X(70)    VALUE "Folder deleted: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Pasta excluída: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Carpeta eliminada: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Dossier supprimé : &1.".
           05 FILLER                             PIC    X(70)    VALUE "フォルダーを削除しました: &1".
           05 FILLER                             PIC    X(70)    VALUE "已删除文件夹：&1".
           *>   DROP-INTO

           05 FILLER                             PIC    X(70)    VALUE "New documents go into &1.".
           05 FILLER                             PIC    X(70)    VALUE "Novos documentos vão para &1.".
           05 FILLER                             PIC    X(70)    VALUE "Los documentos nuevos van a &1.".
           05 FILLER                             PIC    X(70)    VALUE "Les nouveaux documents vont dans &1.".
           05 FILLER                             PIC    X(70)    VALUE "新しい文書の保存先: &1".
           05 FILLER                             PIC    X(70)    VALUE "新文档将放入 &1。".
           *>   TOP-LEVEL

           05 FILLER                             PIC    X(70)    VALUE "the top level".
           05 FILLER                             PIC    X(70)    VALUE "o nível principal".
           05 FILLER                             PIC    X(70)    VALUE "el nivel superior".
           05 FILLER                             PIC    X(70)    VALUE "le niveau supérieur".
           05 FILLER                             PIC    X(70)    VALUE "最上位".
           05 FILLER                             PIC    X(70)    VALUE "顶层".
           *>   STATUS

           05 FILLER                             PIC    X(70)    VALUE "Status".
           05 FILLER                             PIC    X(70)    VALUE "Status".
           05 FILLER                             PIC    X(70)    VALUE "Estado".
           05 FILLER                             PIC    X(70)    VALUE "État".
           05 FILLER                             PIC    X(70)    VALUE "ステータス".
           05 FILLER                             PIC    X(70)    VALUE "状态".
           *>   MOVE

           05 FILLER                             PIC    X(70)    VALUE "Move".
           05 FILLER                             PIC    X(70)    VALUE "Mover".
           05 FILLER                             PIC    X(70)    VALUE "Mover".
           05 FILLER                             PIC    X(70)    VALUE "Déplacer".
           05 FILLER                             PIC    X(70)    VALUE "移動".
           05 FILLER                             PIC    X(70)    VALUE "移动".
           *>   MOVE-HERE

           05 FILLER                             PIC    X(70)    VALUE "Move here".
           05 FILLER                             PIC    X(70)    VALUE "Mover para cá".
           05 FILLER                             PIC    X(70)    VALUE "Mover aquí".
           05 FILLER                             PIC    X(70)    VALUE "Déplacer ici".
           05 FILLER                             PIC    X(70)    VALUE "ここへ移動".
           05 FILLER                             PIC    X(70)    VALUE "移到此处".
           *>   MOVE-TOP

           05 FILLER                             PIC    X(70)    VALUE "To the top level".
           05 FILLER                             PIC    X(70)    VALUE "Para o nível principal".
           05 FILLER                             PIC    X(70)    VALUE "Al nivel superior".
           05 FILLER                             PIC    X(70)    VALUE "Au niveau supérieur".
           05 FILLER                             PIC    X(70)    VALUE "最上位へ".
           05 FILLER                             PIC    X(70)    VALUE "移到顶层".
           *>   PICK-DOC-TO-MOVE

           05 FILLER                             PIC    X(70)    VALUE "Pick the document to move in the tree first.".
           05 FILLER                             PIC    X(70)    VALUE "Escolha primeiro na árvore o documento a mover.".
           05 FILLER                             PIC    X(70)    VALUE "Primero elige en el árbol el documento a mover.".
           05 FILLER                             PIC    X(70)    VALUE "Choisissez d'abord le document à déplacer.".
           05 FILLER                             PIC    X(70)    VALUE "先に移動する文書を選んでください。".
           05 FILLER                             PIC    X(70)    VALUE "请先在树中选择要移动的文档。".
           *>   MOVE-PICK

           05 FILLER                             PIC    X(70)    VALUE "Moving &1: pick a folder, then press Move here.".
           05 FILLER                             PIC    X(70)    VALUE "Movendo &1: escolha uma pasta e clique em Mover para cá.".
           05 FILLER                             PIC    X(70)    VALUE "Moviendo &1: elige una carpeta y pulsa Mover aquí.".
           05 FILLER                             PIC    X(70)    VALUE "Déplacement de &1 : choisissez un dossier, puis Déplacer ici.".
           05 FILLER                             PIC    X(70)    VALUE "&1 の移動先フォルダーを選んでください。".
           05 FILLER                             PIC    X(70)    VALUE "移动 &1：请选择目标文件夹。".
           *>   MOVE-INTO

           05 FILLER                             PIC    X(70)    VALUE "Move here puts &1 into &2.".
           05 FILLER                             PIC    X(70)    VALUE "Mover para cá coloca &1 em &2.".
           05 FILLER                             PIC    X(70)    VALUE "Mover aquí lleva &1 a &2.".
           05 FILLER                             PIC    X(70)    VALUE "Déplacer ici met &1 dans &2.".
           05 FILLER                             PIC    X(70)    VALUE "&1 を &2 へ移動します。".
           05 FILLER                             PIC    X(70)    VALUE "将把 &1 移到 &2。".
           *>   MOVE-SAME

           05 FILLER                             PIC    X(70)    VALUE "The document is already in that folder.".
           05 FILLER                             PIC    X(70)    VALUE "O documento já está nessa pasta.".
           05 FILLER                             PIC    X(70)    VALUE "El documento ya está en esa carpeta.".
           05 FILLER                             PIC    X(70)    VALUE "Le document est déjà dans ce dossier.".
           05 FILLER                             PIC    X(70)    VALUE "文書は既にそのフォルダーにあります。".
           05 FILLER                             PIC    X(70)    VALUE "文档已在该文件夹中。".
           *>   MOVE-EXISTS

           05 FILLER                             PIC    X(70)    VALUE "A document of that name is already there: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Já existe um documento com esse nome lá: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Ya hay un documento con ese nombre allí: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Un document de ce nom s'y trouve déjà : &1.".
           05 FILLER                             PIC    X(70)    VALUE "同名の文書が既にあります: &1".
           05 FILLER                             PIC    X(70)    VALUE "该处已有同名文档：&1".
           *>   MOVED

           05 FILLER                             PIC    X(70)    VALUE "Moved &1 to &2.".
           05 FILLER                             PIC    X(70)    VALUE "&1 movido para &2.".
           05 FILLER                             PIC    X(70)    VALUE "&1 movido a &2.".
           05 FILLER                             PIC    X(70)    VALUE "&1 déplacé vers &2.".
           05 FILLER                             PIC    X(70)    VALUE "&1 を &2 へ移動しました。".
           05 FILLER                             PIC    X(70)    VALUE "已将 &1 移到 &2。".
           *>   ACCEPTED-TYPES

           05 FILLER                             PIC    X(70)    VALUE "Accepted types: &1".
           05 FILLER                             PIC    X(70)    VALUE "Tipos aceitos: &1".
           05 FILLER                             PIC    X(70)    VALUE "Tipos aceptados: &1".
           05 FILLER                             PIC    X(70)    VALUE "Types acceptés : &1".
           05 FILLER                             PIC    X(70)    VALUE "対応する形式: &1".
           05 FILLER                             PIC    X(70)    VALUE "支持的类型：&1".
           *>   REJECTED-TYPE

           05 FILLER                             PIC    X(70)    VALUE "Not added - this type of file cannot be read: &1".
           05 FILLER                             PIC    X(70)    VALUE "Não adicionado - este tipo de arquivo não pode ser lido: &1".
           05 FILLER                             PIC    X(70)    VALUE "No añadido: este tipo de archivo no se puede leer: &1".
           05 FILLER                             PIC    X(70)    VALUE "Non ajouté : ce type de fichier ne peut pas être lu : &1".
           05 FILLER                             PIC    X(70)    VALUE "追加しません（読めない形式）: &1".
           05 FILLER                             PIC    X(70)    VALUE "未添加：无法读取此类文件：&1".
           *>   MODEL-FETCH

           05 FILLER                             PIC    X(70)    VALUE "Downloading the semantic model: &1 - &2 of &3 KB".
           05 FILLER                             PIC    X(70)    VALUE "Baixando o modelo semântico: &1 - &2 de &3 KB".
           05 FILLER                             PIC    X(70)    VALUE "Descargando el modelo semántico: &1 - &2 de &3 KB".
           05 FILLER                             PIC    X(70)    VALUE "Téléchargement du modèle sémantique : &1 - &2 sur &3 Ko".
           05 FILLER                             PIC    X(70)    VALUE "意味モデルをダウンロード中: &1 - &2 / &3 KB".
           05 FILLER                             PIC    X(70)    VALUE "正在下载语义模型：&1 - &2 / &3 KB".
           *>   REINDEXING

           05 FILLER                             PIC    X(70)    VALUE "Rebuilding the Knowledge Base with the semantic model...".
           05 FILLER                             PIC    X(70)    VALUE "Reconstruindo a base de conhecimento com o modelo semântico...".
           05 FILLER                             PIC    X(70)    VALUE "Reconstruyendo la base de conocimiento con el modelo semántico...".
           05 FILLER                             PIC    X(70)    VALUE "Reconstruction de la base avec le modèle sémantique...".
           05 FILLER                             PIC    X(70)    VALUE "意味モデルでナレッジベースを再構築中...".
           05 FILLER                             PIC    X(70)    VALUE "正在用语义模型重建知识库...".
           *>   WORDS-ONLY

           05 FILLER                             PIC    X(70)    VALUE "Search is by words only: &1".
           05 FILLER                             PIC    X(70)    VALUE "A busca é só por palavras: &1".
           05 FILLER                             PIC    X(70)    VALUE "La búsqueda es solo por palabras: &1".
           05 FILLER                             PIC    X(70)    VALUE "Recherche par mots seulement : &1".
           05 FILLER                             PIC    X(70)    VALUE "語句のみで検索します: &1".
           05 FILLER                             PIC    X(70)    VALUE "仅按词语搜索：&1".
           *>   DRAG-DOCS-ONLY

           05 FILLER                             PIC    X(70)    VALUE "Only documents move: drag a document onto a folder.".
           05 FILLER                             PIC    X(70)    VALUE "Só documentos se movem: arraste um até uma pasta.".
           05 FILLER                             PIC    X(70)    VALUE "Solo se mueven documentos: arrastra uno a una carpeta.".
           05 FILLER                             PIC    X(70)    VALUE "Seuls les documents se déplacent : glissez-en un sur un dossier.".
           05 FILLER                             PIC    X(70)    VALUE "移動できるのは文書だけです。".
           05 FILLER                             PIC    X(70)    VALUE "只能移动文档，请拖到文件夹上。".
           *>   PREVIEW

           05 FILLER                             PIC    X(70)    VALUE "Preview".
           05 FILLER                             PIC    X(70)    VALUE "Visualizar".
           05 FILLER                             PIC    X(70)    VALUE "Vista previa".
           05 FILLER                             PIC    X(70)    VALUE "Aperçu".
           05 FILLER                             PIC    X(70)    VALUE "プレビュー".
           05 FILLER                             PIC    X(70)    VALUE "预览".
           *>   PICK-DOC-TO-PREVIEW

           05 FILLER                             PIC    X(70)    VALUE "Pick a document in the tree to preview it.".
           05 FILLER                             PIC    X(70)    VALUE "Escolha um documento na árvore para visualizá-lo.".
           05 FILLER                             PIC    X(70)    VALUE "Elige un documento del árbol para verlo.".
           05 FILLER                             PIC    X(70)    VALUE "Choisissez un document dans l'arbre pour l'afficher.".
           05 FILLER                             PIC    X(70)    VALUE "プレビューする文書を選んでください。".
           05 FILLER                             PIC    X(70)    VALUE "请在树中选择要预览的文档。".
           *>   CLOSE

           05 FILLER                             PIC    X(70)    VALUE "Close".
           05 FILLER                             PIC    X(70)    VALUE "Fechar".
           05 FILLER                             PIC    X(70)    VALUE "Cerrar".
           05 FILLER                             PIC    X(70)    VALUE "Fermer".
           05 FILLER                             PIC    X(70)    VALUE "閉じる".
           05 FILLER                             PIC    X(70)    VALUE "关闭".
           *>   CHUNK-BAR

           05 FILLER                             PIC    X(70)    VALUE "Chunking: &1 of &2 documents".
           05 FILLER                             PIC    X(70)    VALUE "Divisão em trechos: &1 de &2 documentos".
           05 FILLER                             PIC    X(70)    VALUE "División en fragmentos: &1 de &2 documentos".
           05 FILLER                             PIC    X(70)    VALUE "Découpage : &1 sur &2 documents".
           05 FILLER                             PIC    X(70)    VALUE "断片化: &1 / &2 文書".
           05 FILLER                             PIC    X(70)    VALUE "分块：&1 / &2 个文档".
           *>   EMBED-BAR

           05 FILLER                             PIC    X(70)    VALUE "Embeddings: &1 of &2 passages".
           05 FILLER                             PIC    X(70)    VALUE "Embeddings: &1 de &2 trechos".
           05 FILLER                             PIC    X(70)    VALUE "Embeddings: &1 de &2 fragmentos".
           05 FILLER                             PIC    X(70)    VALUE "Embeddings : &1 sur &2 passages".
           05 FILLER                             PIC    X(70)    VALUE "埋め込み: &1 / &2 断片".
           05 FILLER                             PIC    X(70)    VALUE "嵌入：&1 / &2 个片段".
           *>   KB-EMBEDDED

           05 FILLER                             PIC    X(70)    VALUE "&1 passages embedded: semantic search.".
           05 FILLER                             PIC    X(70)    VALUE "&1 trechos com embedding: busca semântica.".
           05 FILLER                             PIC    X(70)    VALUE "&1 fragmentos con embedding: búsqueda semántica.".
           05 FILLER                             PIC    X(70)    VALUE "&1 passages vectorisés : recherche sémantique.".
           05 FILLER                             PIC    X(70)    VALUE "&1 件の断片を埋め込み済み（意味検索）。".
           05 FILLER                             PIC    X(70)    VALUE "已嵌入 &1 个片段（语义搜索）。".
           *>   KB-PASSAGES

           05 FILLER                             PIC    X(70)    VALUE "&1 passages indexed.".
           05 FILLER                             PIC    X(70)    VALUE "&1 trechos indexados.".
           05 FILLER                             PIC    X(70)    VALUE "&1 fragmentos indexados.".
           05 FILLER                             PIC    X(70)    VALUE "&1 passages indexés.".
           05 FILLER                             PIC    X(70)    VALUE "&1 件の断片を索引化しました。".
           05 FILLER                             PIC    X(70)    VALUE "已索引 &1 个片段。".
           *>   REMOVING

           05 FILLER                             PIC    X(70)    VALUE "Removing &1 from the Knowledge Base...".
           05 FILLER                             PIC    X(70)    VALUE "Removendo &1 da base de conhecimento...".
           05 FILLER                             PIC    X(70)    VALUE "Quitando &1 de la base de conocimiento...".
           05 FILLER                             PIC    X(70)    VALUE "Retrait de &1 de la base de connaissances...".
           05 FILLER                             PIC    X(70)    VALUE "&1 をナレッジベースから削除中...".
           05 FILLER                             PIC    X(70)    VALUE "正在从知识库移除 &1...".
           *>   DOC-REMOVED

           05 FILLER                             PIC    X(70)    VALUE "Deleted: &1 - removed from the Knowledge Base.".
           05 FILLER                             PIC    X(70)    VALUE "Excluído: &1 - removido da base de conhecimento.".
           05 FILLER                             PIC    X(70)    VALUE "Eliminado: &1 - quitado de la base de conocimiento.".
           05 FILLER                             PIC    X(70)    VALUE "Supprimé : &1 - retiré de la base de connaissances.".
           05 FILLER                             PIC    X(70)    VALUE "削除しました: &1（ナレッジベースから除外）".
           05 FILLER                             PIC    X(70)    VALUE "已删除：&1（已从知识库移除）".
           *>   REPLACE-Q

           05 FILLER                             PIC    X(70)    VALUE "&1 already exists in this folder. Replace it?".
           05 FILLER                             PIC    X(70)    VALUE "&1 já existe nesta pasta. Substituir?".
           05 FILLER                             PIC    X(70)    VALUE "&1 ya existe en esta carpeta. ¿Reemplazarlo?".
           05 FILLER                             PIC    X(70)    VALUE "&1 existe déjà dans ce dossier. Le remplacer ?".
           05 FILLER                             PIC    X(70)    VALUE "&1 は既にあります。置き換えますか？".
           05 FILLER                             PIC    X(70)    VALUE "&1 已存在于此文件夹。要替换吗？".
           *>   REPLACE-YES

           05 FILLER                             PIC    X(70)    VALUE "Replace".
           05 FILLER                             PIC    X(70)    VALUE "Substituir".
           05 FILLER                             PIC    X(70)    VALUE "Reemplazar".
           05 FILLER                             PIC    X(70)    VALUE "Remplacer".
           05 FILLER                             PIC    X(70)    VALUE "置き換える".
           05 FILLER                             PIC    X(70)    VALUE "替换".
           *>   REPLACE-NO

           05 FILLER                             PIC    X(70)    VALUE "Keep the old one".
           05 FILLER                             PIC    X(70)    VALUE "Manter o antigo".
           05 FILLER                             PIC    X(70)    VALUE "Conservar el anterior".
           05 FILLER                             PIC    X(70)    VALUE "Garder l'ancien".
           05 FILLER                             PIC    X(70)    VALUE "古いものを残す".
           05 FILLER                             PIC    X(70)    VALUE "保留旧文件".
           *>   UP-SENDING

           05 FILLER                             PIC    X(70)    VALUE "Adding &1 to the Knowledge Base...".
           05 FILLER                             PIC    X(70)    VALUE "Adicionando &1 à base de conhecimento...".
           05 FILLER                             PIC    X(70)    VALUE "Añadiendo &1 a la base de conocimiento...".
           05 FILLER                             PIC    X(70)    VALUE "Ajout de &1 à la base de connaissances...".
           05 FILLER                             PIC    X(70)    VALUE "&1 をナレッジベースに追加中...".
           05 FILLER                             PIC    X(70)    VALUE "正在将 &1 加入知识库...".
           *>   UP-ADDED

           05 FILLER                             PIC    X(70)    VALUE "Added: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Adicionado: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Añadido: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Ajouté : &1.".
           05 FILLER                             PIC    X(70)    VALUE "追加しました: &1".
           05 FILLER                             PIC    X(70)    VALUE "已添加：&1。".
           *>   UP-UPDATED

           05 FILLER                             PIC    X(70)    VALUE "Updated: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Atualizado: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Actualizado: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Mis à jour : &1.".
           05 FILLER                             PIC    X(70)    VALUE "更新しました: &1".
           05 FILLER                             PIC    X(70)    VALUE "已更新：&1。".
           *>   UP-KEPT

           05 FILLER                             PIC    X(70)    VALUE "Not replaced: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Não substituído: &1.".
           05 FILLER                             PIC    X(70)    VALUE "No reemplazado: &1.".
           05 FILLER                             PIC    X(70)    VALUE "Non remplacé : &1.".
           05 FILLER                             PIC    X(70)    VALUE "置き換えませんでした: &1".
           05 FILLER                             PIC    X(70)    VALUE "未替换：&1。".
           *>   UP-SUMMARY

           05 FILLER                             PIC    X(70)    VALUE "&1 added, &2 updated, &3 not replaced.".
           05 FILLER                             PIC    X(70)    VALUE "&1 adicionado(s), &2 atualizado(s), &3 não substituído(s).".
           05 FILLER                             PIC    X(70)    VALUE "&1 añadido(s), &2 actualizado(s), &3 no reemplazado(s).".
           05 FILLER                             PIC    X(70)    VALUE "&1 ajouté(s), &2 mis à jour, &3 non remplacé(s).".
           05 FILLER                             PIC    X(70)    VALUE "追加 &1、更新 &2、置き換えなし &3。".
           05 FILLER                             PIC    X(70)    VALUE "新增 &1，更新 &2，未替换 &3。".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    54.
               10 PC-TEXT                        PIC    X(70)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-OPEN-TOPIC-FIRST                 PIC    X(70).
           05 T-DOCS-OF                          PIC    X(70).
           05 T-PICK-DOC-FIRST                   PIC    X(70).
           05 T-KB-PROGRESS                      PIC    X(70).
           05 T-KB-COUNTS                        PIC    X(70).
           05 T-KB-SKIPPED                       PIC    X(70).
           05 T-DOC-COUNT                        PIC    X(70).
           05 T-DOCS-TITLE                       PIC    X(70).
           05 T-DROP-HINT                        PIC    X(70).
           05 T-DELETE                           PIC    X(70).
           05 T-REFRESH                          PIC    X(70).
           05 T-KB-UPDATING                      PIC    X(70).
           05 T-NEW-FOLDER                       PIC    X(70).
           05 T-FOLDER-HINT                      PIC    X(70).
           05 T-FOLDER-CREATED                   PIC    X(70).
           05 T-FOLDER-EXISTS                    PIC    X(70).
           05 T-FOLDER-NAME-BAD                  PIC    X(70).
           05 T-FOLDER-NOT-EMPTY                 PIC    X(70).
           05 T-FOLDER-DELETED                   PIC    X(70).
           05 T-DROP-INTO                        PIC    X(70).
           05 T-TOP-LEVEL                        PIC    X(70).
           05 T-STATUS                           PIC    X(70).
           05 T-MOVE                             PIC    X(70).
           05 T-MOVE-HERE                        PIC    X(70).
           05 T-MOVE-TOP                         PIC    X(70).
           05 T-PICK-DOC-TO-MOVE                 PIC    X(70).
           05 T-MOVE-PICK                        PIC    X(70).
           05 T-MOVE-INTO                        PIC    X(70).
           05 T-MOVE-SAME                        PIC    X(70).
           05 T-MOVE-EXISTS                      PIC    X(70).
           05 T-MOVED                            PIC    X(70).
           05 T-ACCEPTED-TYPES                   PIC    X(70).
           05 T-REJECTED-TYPE                    PIC    X(70).
           05 T-MODEL-FETCH                      PIC    X(70).
           05 T-REINDEXING                       PIC    X(70).
           05 T-WORDS-ONLY                       PIC    X(70).
           05 T-DRAG-DOCS-ONLY                   PIC    X(70).
           05 T-PREVIEW                          PIC    X(70).
           05 T-PICK-DOC-TO-PREVIEW              PIC    X(70).
           05 T-CLOSE                            PIC    X(70).
           05 T-CHUNK-BAR                        PIC    X(70).
           05 T-EMBED-BAR                        PIC    X(70).
           05 T-KB-EMBEDDED                      PIC    X(70).
           05 T-KB-PASSAGES                      PIC    X(70).
           05 T-REMOVING                         PIC    X(70).
           05 T-DOC-REMOVED                      PIC    X(70).
           05 T-REPLACE-Q                        PIC    X(70).
           05 T-REPLACE-YES                      PIC    X(70).
           05 T-REPLACE-NO                       PIC    X(70).
           05 T-UP-SENDING                       PIC    X(70).
           05 T-UP-ADDED                         PIC    X(70).
           05 T-UP-UPDATED                       PIC    X(70).
           05 T-UP-KEPT                          PIC    X(70).
           05 T-UP-SUMMARY                       PIC    X(70).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(70)    OCCURS 54.
       *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.

       01 WS-FMT                                 GLOBAL    PIC X(70).
       01 WS-ARG1                                GLOBAL    PIC X(300).
       01 WS-ARG2                                GLOBAL    PIC X(300).
       01 WS-ARG3                                GLOBAL    PIC X(300).
       01 WS-ARG4                                GLOBAL    PIC X(300).
       01 WS-FMT-OUT                             GLOBAL    PIC X(1200).
       01 WS-FOLDERS-PATH                        GLOBAL    PIC X(240).
       01 WS-DOCS-DIR                            GLOBAL    PIC X(400).
       *>   The documents tree (R49): one row per node, in the order the nodes
       *>   are added, so a node event's CONTROL-NODE-INDEX names its row.

       01 WS-TREE                                GLOBAL.
           05 WS-NODE-N                          PIC    9(4)    VALUE 0.
           05 WS-NODE                            OCCURS    1000.
               10 WS-NODE-KIND                   PIC    X.
               10 WS-NODE-PATH                   PIC    X(260).
               10 WS-NODE-KEY                    PIC    X(260).
       01 WS-NODE-TMP                            GLOBAL.
           05 TMP-KIND                           PIC    X.
           05 TMP-PATH                           PIC    X(260).
           05 TMP-KEY                            PIC    X(260).
       01 WS-KEY-TMP                             GLOBAL    PIC X(260).
       01 WS-ONE-KIND                            GLOBAL    PIC X.
       01 WS-ONE-PATH                            GLOBAL    PIC X(260).
       *>   The selection, by path: node numbers change on every rebuild.

       01 WS-SEL-KIND                            GLOBAL    PIC X    VALUE SPACE.
       01 WS-SEL-PATH                            GLOBAL    PIC X(260)    VALUE SPACES.
       01 WS-SEL-IX                              GLOBAL    PIC 9(4)    VALUE 0.
       01 WS-FOLDER                              GLOBAL    PIC X(260)    VALUE SPACES.
       01 WS-NAME                                GLOBAL    PIC X(260).
       01 WS-GONE                                GLOBAL    PIC X(260).
       01 WS-ICON                                GLOBAL    PIC X(10).
       01 WS-J                                   GLOBAL    PIC 9(4).
       01 WS-K                                   GLOBAL    PIC 9(4).
       01 WS-K1                                  GLOBAL    PIC 9(4).
       01 WS-L                                   GLOBAL    PIC 9(4).
       01 WS-P                                   GLOBAL    PIC 9(4).
       01 WS-DEPTH                               GLOBAL    PIC 9(4).
       01 WS-FOUND                               GLOBAL    PIC X.
       *>   A document being moved: picked with Move, placed with Move here.
       *>   The copy goes first; the original is deleted only once the copy
       *>   is indexed (WS-MOVE-STEP "I", then "D"), so a failure never loses it.

       01 WS-MOVE-SRC                            GLOBAL    PIC X(260)    VALUE SPACES.
       01 WS-MOVE-DEST                           GLOBAL    PIC X(260)    VALUE SPACES.
       01 WS-MOVE-TO                             GLOBAL    PIC X(260)    VALUE SPACES.
       01 WS-MOVE-STEP                           GLOBAL    PIC X    VALUE SPACE.
       *>   What the Knowledge Base can read, and so all the zone accepts: the
       *>   zone's filter AND the list the form shows. One list, both uses.

       01 WS-DOC-TYPES                           GLOBAL    PIC X(300)    VALUE
           ".md .markdown .txt .text .log .html .htm .xhtml .csv .tsv .tab .pdf .docx .docm .dotx .dotm .pptx .pptm .potx .potm .ppsx .ppsm .xlsx .xlsm .xltx .xltm .odt .ott .odm .oth .ods .ots .odp .otp .zip .tar .gz .tgz".
       *>   Files refused by the last drop, named for the status line; kept
       *>   until the indexing summary that follows has shown them too.

       01 WS-REJECT-MSG                          GLOBAL    PIC X(400)    VALUE SPACES.
       *>   The Knowledge Base's own work in flight: "M" fetching the semantic
       *>   model, "R" rebuilding a collection indexed with another embedder.

       01 WS-KB-STEP                             GLOBAL    PIC X    VALUE SPACE.
       01 WS-MODE-WHY                            GLOBAL    PIC X(400)    VALUE SPACES.
       01 WS-TALLY                               GLOBAL    PIC 9(4).
       01 WS-KB1                                 GLOBAL    PIC Z(8)9.
       01 WS-KB2                                 GLOBAL    PIC Z(8)9.
       *>   A number the Knowledge Base reports, as text and as a number.

       01 WS-NUM-TEXT                            GLOBAL    PIC X(20).
       01 WS-PASSAGES                            GLOBAL    PIC 9(9).
       01 WS-PASSAGE                             GLOBAL    PIC 9(9).
       01 WS-DOC-CUR                             GLOBAL    PIC 9(9).
       01 WS-DOC-TOT                             GLOBAL    PIC 9(9).
       *>   A document being deleted, named in the confirmation.

       01 WS-DELETING                            GLOBAL    PIC X(260)    VALUE SPACES.
       *>   Uploads (operator, 2026-09-27: ask, then replace). The zone copies
       *>   nothing; each dropped file is queued with where it goes, asked
       *>   about when that name is taken, and imported in turn.

       01 WS-UP-N                                GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-I                                GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-CUR                              GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-STEP                             GLOBAL    PIC X    VALUE SPACE.
       01 WS-UP-ADDED                            GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-UPDATED                          GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-KEPT                             GLOBAL    PIC 9(3)    VALUE 0.
       01 WS-UP-QUEUE                            GLOBAL.
           05 WS-UP-ITEM                         OCCURS    50.
               10 WS-UP-SRC                      PIC    X(400).
               10 WS-UP-DEST                     PIC    X(260).
               *>      "Q" queued, "K" kept (not replaced), "D" done.

               10 WS-UP-STATE                    PIC    X.
       01 WS-UP-ALL                              GLOBAL    PIC X(8000).
       01 WS-UP-LEN                              GLOBAL    PIC 9(5).
       01 WS-UP-PT                               GLOBAL    PIC 9(5).
       01 WS-UP-ONE                              GLOBAL    PIC X(400).
       01 WS-UP-NAME                             GLOBAL    PIC X(260).
       01 WS-UP-ANSWER                           GLOBAL    PIC X(4).
       01 WS-UP-QUESTION                         GLOBAL    PIC X(400).
       01 WS-UP-COUNT                            GLOBAL    PIC 9(9).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Documents'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Trv-Docs.
          05 WS-Trv-Docs-TEXT       PIC X(256) VALUE 'Trv-Docs'.
          05 WS-Trv-Docs-VISIBLE    PIC 9      VALUE 1.
          05 WS-Trv-Docs-ENABLED    PIC 9      VALUE 1.

       01 WS-Drop-Docs.
          05 WS-Drop-Docs-TEXT       PIC X(256) VALUE 'Drop-Docs'.
          05 WS-Drop-Docs-VISIBLE    PIC 9      VALUE 1.
          05 WS-Drop-Docs-ENABLED    PIC 9      VALUE 1.
          05 WS-Drop-Docs-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-Drop-Docs-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-Btn-Delete.
          05 WS-Btn-Delete-TEXT       PIC X(256) VALUE 'Delete'.
          05 WS-Btn-Delete-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Delete-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Preview.
          05 WS-Btn-Preview-TEXT       PIC X(256) VALUE 'Preview'.
          05 WS-Btn-Preview-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Preview-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Refresh.
          05 WS-Btn-Refresh-TEXT       PIC X(256) VALUE 'Refresh'.
          05 WS-Btn-Refresh-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Refresh-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Count.
          05 WS-Lbl-Count-TEXT       PIC X(256) VALUE 'Lbl-Count'.
          05 WS-Lbl-Count-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Count-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Folder.
          05 WS-Txt-Folder-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Folder-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Folder-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Folder-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-NewFolder.
          05 WS-Btn-NewFolder-TEXT       PIC X(256) VALUE 'New folder'.
          05 WS-Btn-NewFolder-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-NewFolder-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Move.
          05 WS-Btn-Move-TEXT       PIC X(256) VALUE 'Move'.
          05 WS-Btn-Move-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Move-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-MoveTop.
          05 WS-Btn-MoveTop-TEXT       PIC X(256) VALUE 'To the top level'.
          05 WS-Btn-MoveTop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-MoveTop-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Progress.
          05 WS-Lbl-Progress-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Progress-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Progress-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Index.
          05 WS-Prg-Index-TEXT       PIC X(256) VALUE 'Prg-Index'.
          05 WS-Prg-Index-VISIBLE    PIC 9      VALUE 0.
          05 WS-Prg-Index-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Chunk.
          05 WS-Lbl-Chunk-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Chunk-VISIBLE    PIC 9      VALUE 0.
          05 WS-Lbl-Chunk-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Chunk.
          05 WS-Prg-Chunk-TEXT       PIC X(256) VALUE 'Prg-Chunk'.
          05 WS-Prg-Chunk-VISIBLE    PIC 9      VALUE 0.
          05 WS-Prg-Chunk-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Embed.
          05 WS-Lbl-Embed-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Embed-VISIBLE    PIC 9      VALUE 0.
          05 WS-Lbl-Embed-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Embed.
          05 WS-Prg-Embed-TEXT       PIC X(256) VALUE 'Prg-Embed'.
          05 WS-Prg-Embed-VISIBLE    PIC 9      VALUE 0.
          05 WS-Prg-Embed-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Types.
          05 WS-Lbl-Types-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Types-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Types-ENABLED    PIC 9      VALUE 1.

       01 WS-KB-D.
          05 WS-KB-D-TEXT       PIC X(256) VALUE 'KB-D'.
          05 WS-KB-D-VISIBLE    PIC 9      VALUE 1.
          05 WS-KB-D-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Lang.
          05 WS-Tmr-Lang-TEXT       PIC X(256) VALUE 'Tmr-Lang'.
          05 WS-Tmr-Lang-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Lang-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "DOCUMENTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "DOCUMENTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "DOCUMENTS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onActivate"
                               CALL "DOCUMENTS-FORM--ONACTIVATE"
                       END-EVALUATE
                   WHEN "Trv-Docs"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onNodeSelect"
                               CALL "TRV-DOCS--ONNODESELECT"
                           WHEN "onNodeDblClick"
                               CALL "TRV-DOCS--ONNODEDBLCLICK"
                           WHEN "onNodeDrop"
                               CALL "TRV-DOCS--ONNODEDROP"
                       END-EVALUATE
                   WHEN "Drop-Docs"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onFilesDropped"
                               CALL "DROP-DOCS--ONFILESDROPPED"
                           WHEN "onFilesRejected"
                               CALL "DROP-DOCS--ONFILESREJECTED"
                       END-EVALUATE
                   WHEN "Btn-Delete"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-DELETE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Preview"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PREVIEW--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Refresh"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-REFRESH--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-NewFolder"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEWFOLDER--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Move"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MOVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-MoveTop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MOVETOP--ONCLICK"
                       END-EVALUATE
                   WHEN "KB-D"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onProgress"
                               CALL "KB-D--ONPROGRESS"
                           WHEN "onIndexed"
                               CALL "KB-D--ONINDEXED"
                           WHEN "onBusy"
                               CALL "KB-D--ONBUSY"
                           WHEN "onError"
                               CALL "KB-D--ONERROR"
                       END-EVALUATE
                   WHEN "Tmr-Lang"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-LANG--ONTICK"
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
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOCUMENTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM DOCUMENTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOCUMENTS-FORM--ONACTIVATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Back on the pane: the topic or the language may have changed.

           CALL    "PC-OPEN"

           GOBACK.

       END PROGRAM DOCUMENTS-FORM--ONACTIVATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOCUMENTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM DOCUMENTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TRV-DOCS--ONNODESELECT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       LINKAGE SECTION.
       01 CONTROL-NODE-DATA.
           05 CONTROL-NODE                       PIC    X(256).
           05 CONTROL-NODE-INDEX                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-LEVEL                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-CHECKED               PIC    9.

       PROCEDURE DIVISION USING CONTROL-NODE-DATA.
           *>   The node's index is its row in WS-TREE; the selection is kept by
           *>   path, and new documents dropped go into its folder.

           IF    CONTROL-NODE-INDEX > 0 AND CONTROL-NODE-INDEX NOT > WS-NODE-N
               SET     WS-J                      TO    CONTROL-NODE-INDEX
               SET     WS-SEL-KIND               TO    WS-NODE-KIND(WS-J)
               SET     WS-SEL-PATH               TO    WS-NODE-PATH(WS-J)
           END-IF
           CALL    "PC-SELECT-SYNC"
           *>   While a document waits to be moved, say where Move here puts it.

           IF    WS-MOVE-SRC NOT = SPACES AND WS-MOVE-STEP = SPACE
               SET     WS-FMT                    TO    T-MOVE-INTO
               SET     WS-ARG1                   TO    WS-MOVE-SRC

               IF    WS-FOLDER = SPACES
                   SET     WS-ARG2               TO    T-TOP-LEVEL
               ELSE
                   SET     WS-ARG2               TO    WS-FOLDER
               END-IF
               CALL    "PC-FMT"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-FMT                        TO    T-DROP-INTO
           IF    WS-FOLDER = SPACES
               SET     WS-ARG1                   TO    T-TOP-LEVEL
           ELSE
               SET     WS-ARG1                   TO    WS-FOLDER
           END-IF
           CALL    "PC-FMT"
           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM TRV-DOCS--ONNODESELECT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TRV-DOCS--ONNODEDBLCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       LINKAGE SECTION.
       01 CONTROL-NODE-DATA.
           05 CONTROL-NODE                       PIC    X(256).
           05 CONTROL-NODE-INDEX                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-LEVEL                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-CHECKED               PIC    9.

       PROCEDURE DIVISION USING CONTROL-NODE-DATA.
           *>   A double-click on a document previews it.

           IF    CONTROL-NODE-INDEX > 0 AND CONTROL-NODE-INDEX NOT > WS-NODE-N
               SET     WS-J                      TO    CONTROL-NODE-INDEX
               SET     WS-SEL-KIND               TO    WS-NODE-KIND(WS-J)
               SET     WS-SEL-PATH               TO    WS-NODE-PATH(WS-J)

               CALL    "PC-SELECT-SYNC"
               IF    WS-SEL-KIND = "D"
                   CALL    "PC-PREVIEW"
               END-IF
           END-IF

           GOBACK.

       END PROGRAM TRV-DOCS--ONNODEDBLCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TRV-DOCS--ONNODEDROP IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       LINKAGE SECTION.
       01 CONTROL-NODE-DATA.
           05 CONTROL-NODE                       PIC    X(256).
           05 CONTROL-NODE-INDEX                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-LEVEL                 PIC    S9(4)    COMP-5.
           05 CONTROL-NODE-CHECKED               PIC    9.
           05 CONTROL-TARGET-INDEX               PIC    S9(4)    COMP-5.
           05 CONTROL-TARGET-NODE                PIC    X(256).

       PROCEDURE DIVISION USING CONTROL-NODE-DATA.
           *>   A document dragged onto a folder moves into it; onto a document,
           *>   into that document's folder; onto empty space, to the top level.
           *>   The move itself is the Move button's (PC-MOVE-GO): copy, index,
           *>   then delete the original, never over a document of the same name.

           IF    CONTROL-NODE-INDEX < 1 OR CONTROL-NODE-INDEX > WS-NODE-N
               EXIT PROGRAM
           END-IF
           IF    WS-MOVE-STEP NOT = SPACE
               SET     Lbl-Status::Caption       TO    T-KB-UPDATING::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-J                          TO    CONTROL-NODE-INDEX
           IF    WS-NODE-KIND(WS-J) NOT = "D"
               SET     Lbl-Status::Caption       TO    T-DRAG-DOCS-ONLY::Trim()
               EXIT PROGRAM
           END-IF

           SET     WS-MOVE-SRC                   TO    WS-NODE-PATH(WS-J)
           SET     WS-MOVE-TO                    TO    SPACES

           IF    CONTROL-TARGET-INDEX > 0 AND CONTROL-TARGET-INDEX NOT > WS-NODE-N
               SET     WS-K1                     TO    CONTROL-TARGET-INDEX
               IF    WS-NODE-KIND(WS-K1) = "F"
                   SET     WS-MOVE-TO            TO    WS-NODE-PATH(WS-K1)
               ELSE
                   SET     WS-L                  TO    FUNCTION LENGTH(WS-NODE-PATH(WS-K1)::Trim())
                   SET     WS-P                  TO    0

                   PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
                       IF    WS-NODE-PATH(WS-K1)(WS-K:1) = "/"
                           SET     WS-P          TO    WS-K
                       END-IF
                   END-PERFORM
                   IF    WS-P > 1
                       SET     WS-MOVE-TO        TO    WS-NODE-PATH(WS-K1)(1:WS-P - 1)
                   END-IF
               END-IF
           END-IF
           CALL    "PC-MOVE-GO"

           GOBACK.

       END PROGRAM TRV-DOCS--ONNODEDROP.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DROP-DOCS--ONFILESDROPPED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Each accepted file (DroppedFiles, at its own path - the zone
           *>   copies nothing) goes into the selected folder under its own name.
           *>   A name already there is asked about first: replace, or keep.

           SET     WS-UP-ALL                     TO    Drop-Docs::DroppedFiles
           SET     WS-UP-LEN                     TO    FUNCTION LENGTH(WS-UP-ALL::Trim())
           SET     WS-UP-PT                      TO    1

           *> The queue (WS-UP-N, up to 50) outlives one drop: files dropped while
           *> an earlier batch is still importing join it, so the counters restart
           *> only when the queue was empty.

           IF    WS-UP-N = 0
               SET     WS-UP-ADDED WS-UP-UPDATED WS-UP-KEPT
                                                 TO    0
           END-IF
           PERFORM    UNTIL WS-UP-PT > WS-UP-LEN OR WS-UP-N = 50
               SET     WS-UP-ONE                 TO    SPACES
               UNSTRING    WS-UP-ALL DELIMITED BY X"0A" INTO WS-UP-ONE
                   WITH POINTER WS-UP-PT
               END-UNSTRING
               IF    WS-UP-ONE NOT = SPACES
                   *> Dropped paths may use either separator (\ on Windows, / elsewhere):
                   *> the file name is what follows the last one.

                   SET     WS-L                  TO    FUNCTION LENGTH(WS-UP-ONE::Trim())
                   SET     WS-P                  TO    0

                   PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
                       IF    WS-UP-ONE(WS-K:1) = "/" OR WS-UP-ONE(WS-K:1) = "\"
                           SET     WS-P          TO    WS-K
                       END-IF
                   END-PERFORM

                   SET     WS-UP-NAME            TO    SPACES
                   SET     WS-UP-NAME            TO    WS-UP-ONE(WS-P + 1:WS-L - WS-P)

                   ADD    1 TO WS-UP-N

                   SET     WS-UP-SRC(WS-UP-N)    TO    WS-UP-ONE
                   SET     WS-UP-DEST(WS-UP-N)   TO    SPACES

                   IF    WS-FOLDER = SPACES
                       SET     WS-UP-DEST(WS-UP-N)
                                                 TO    WS-UP-NAME::Trim()
                   ELSE
                       STRING    WS-FOLDER::Trim() "/" WS-UP-NAME::Trim() INTO WS-UP-DEST(WS-UP-N)
                   END-IF

                   SET     WS-UP-STATE(WS-UP-N)  TO    "Q"
                   SET     WS-FOUND              TO    "N"

                   PERFORM    VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NODE-N
                       IF    WS-NODE-KIND(WS-J) = "D"
                               AND WS-NODE-PATH(WS-J) = WS-UP-DEST(WS-UP-N)
                           SET     WS-FOUND      TO    "Y"
                       END-IF
                   END-PERFORM
                   IF    WS-FOUND = "Y"
                       SET     WS-FMT            TO    T-REPLACE-Q
                       SET     WS-ARG1           TO    WS-UP-DEST(WS-UP-N)

                       CALL    "PC-FMT"
                       SET     WS-UP-QUESTION    TO    WS-FMT-OUT

                       INVOKE    ME::SetProperty("ConfirmText", WS-UP-QUESTION)
                       INVOKE    ME::SetProperty("ConfirmYes", T-REPLACE-YES)
                       INVOKE    ME::SetProperty("ConfirmNo", T-REPLACE-NO)
                       INVOKE    ME::SetProperty("ConfirmAnswer", "N")
                       INVOKE    ME::OpenFormSync("CONFIRM-FORM")
                       INVOKE    ME::GetProperty("ConfirmAnswer") RETURNING WS-UP-ANSWER

                       IF    WS-UP-ANSWER(1:1) NOT = "Y"
                           SET     WS-UP-STATE(WS-UP-N)
                                                 TO    "K"

                           ADD    1 TO WS-UP-KEPT

                           SET     WS-FMT        TO    T-UP-KEPT
                           SET     WS-ARG1       TO    WS-UP-DEST(WS-UP-N)

                           CALL    "PC-FMT"
                           SET     Lbl-Status::Caption
                                                 TO    WS-FMT-OUT::Trim()
                       END-IF
                   END-IF
               END-IF
           END-PERFORM
           CALL    "PC-UP-NEXT"

           GOBACK.

       END PROGRAM DROP-DOCS--ONFILESDROPPED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DROP-DOCS--ONFILESREJECTED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 LK-ALL                                 PIC    X(4000).
       01 LK-ONE                                 PIC    X(400).
       01 LK-PATH                                PIC    X(400).
       01 LK-NAMES                               PIC    X(300).
       01 LK-LEN                                 PIC    9(4).
       01 LK-PT                                  PIC    9(4).
       01 LK-OUT                                 PIC    9(4).

       PROCEDURE DIVISION.
           *>   RejectedFiles: one "path<TAB>reason" line per refused file. Name
           *>   each one and say why, so nothing is refused without a word.

           SET     LK-ALL                        TO    Drop-Docs::RejectedFiles
           SET     LK-LEN                        TO    FUNCTION LENGTH(FUNCTION TRIM(LK-ALL))
           SET     LK-NAMES                      TO    SPACES
           SET     LK-PT                         TO    1
           SET     LK-OUT                        TO    1

           PERFORM    UNTIL LK-PT > LK-LEN
               SET     LK-ONE LK-PATH            TO    SPACES
               UNSTRING    LK-ALL DELIMITED BY X"0A" INTO LK-ONE
                   WITH POINTER LK-PT
               END-UNSTRING
               UNSTRING    LK-ONE DELIMITED BY X"09" INTO LK-PATH
               END-UNSTRING
               IF    LK-PATH NOT = SPACES
                   SET     WS-L                  TO    FUNCTION LENGTH(FUNCTION TRIM(LK-PATH))
                   SET     WS-P                  TO    0

                   PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
                       IF    LK-PATH(WS-K:1) = "/" OR LK-PATH(WS-K:1) = "\"
                           SET     WS-P          TO    WS-K
                       END-IF
                   END-PERFORM
                   IF    LK-OUT > 1
                       STRING    ", " INTO LK-NAMES WITH POINTER LK-OUT
                   END-IF
                   STRING    LK-PATH(WS-P + 1:WS-L - WS-P) INTO LK-NAMES WITH POINTER LK-OUT
               END-IF
           END-PERFORM

           SET     WS-FMT                        TO    T-REJECTED-TYPE
           SET     WS-ARG1                       TO    LK-NAMES

           CALL    "PC-FMT"

           SET     WS-REJECT-MSG                 TO    WS-FMT-OUT
           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM DROP-DOCS--ONFILESREJECTED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-DELETE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           IF    WS-SEL-IX = 0
               SET     Lbl-Status::Caption       TO    T-PICK-DOC-FIRST::Trim()
               EXIT PROGRAM
           END-IF
           IF    WS-SEL-KIND = "D"
               SET     WS-OK                     TO    KB-D::DeleteDocument(WS-SEL-PATH::Trim())
               IF    WS-OK NOT = "1"
                   SET     Lbl-Status::Caption   TO    KB-D::LastError
               ELSE
                   *>           Said while it happens, and confirmed by name when done.

                   SET     WS-DELETING           TO    WS-SEL-PATH
                   SET     WS-FMT                TO    T-REMOVING
                   SET     WS-ARG1               TO    WS-SEL-PATH

                   CALL    "PC-FMT"
                   SET     Lbl-Progress::Caption TO    WS-FMT-OUT::Trim()
               END-IF
               EXIT PROGRAM
           END-IF
           *>   A folder goes only when nothing is under it: deleting a folder
           *>   never deletes documents as a side effect.

           SET     WS-KEY-TMP                    TO    SPACES
           STRING    WS-SEL-PATH::Trim() "/" INTO WS-KEY-TMP

           SET     WS-L                          TO    FUNCTION LENGTH(WS-KEY-TMP::Trim())
           SET     WS-FOUND                      TO    "N"

           PERFORM    VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NODE-N
               IF    WS-NODE-PATH(WS-J)(1:WS-L) = WS-KEY-TMP(1:WS-L)
                   SET     WS-FOUND              TO    "Y"
               END-IF
           END-PERFORM
           IF    WS-FOUND = "Y"
               SET     Lbl-Status::Caption       TO    T-FOLDER-NOT-EMPTY::Trim()
               EXIT PROGRAM
           END-IF
           OPEN    I-O FOLDERS-FILE
           IF    WS-FS = "00"
               SET     FLD-TOPIC                 TO    WS-CUR-TOPIC
               SET     FLD-PATH                  TO    WS-SEL-PATH

               DELETE    FOLDERS-FILE
                   INVALID KEY CONTINUE
               END-DELETE
               COMMIT
               CLOSE    FOLDERS-FILE
           END-IF
           *>   (Not WS-NAME: PC-LIST-DOCS writes every node's label there.)

           SET     WS-GONE                       TO    WS-SEL-PATH
           SET     WS-SEL-KIND WS-SEL-PATH       TO    SPACES

           CALL    "PC-LIST-DOCS"

           SET     WS-FMT                        TO    T-FOLDER-DELETED
           SET     WS-ARG1                       TO    WS-GONE

           CALL    "PC-FMT"
           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM BTN-DELETE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PREVIEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-PREVIEW"

           GOBACK.

       END PROGRAM BTN-PREVIEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-REFRESH--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-KB-REFRESH"

           GOBACK.

       END PROGRAM BTN-REFRESH--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEWFOLDER--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A folder inside the selected one (or a document's folder), else at
           *>   the top. Recorded in FOLDERS-FILE, so it shows while still empty.

           SET     WS-NAME                       TO    Txt-Folder::Text
           SET     WS-K                          TO    0

           *> A name is one path segment: any "/" or "\" in it (WS-K counts them)
           *> would make a path, and "." / ".." are not new folders at all.

           INSPECT    WS-NAME TALLYING WS-K FOR ALL "/"
           INSPECT    WS-NAME TALLYING WS-K FOR ALL "\"

           IF    WS-NAME = SPACES OR WS-K > 0
                   OR FUNCTION TRIM(WS-NAME) = "."
                   OR FUNCTION TRIM(WS-NAME) = ".."
               SET     Lbl-Status::Caption       TO    T-FOLDER-NAME-BAD::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-ONE-PATH                   TO    SPACES
           IF    WS-FOLDER = SPACES
               SET     WS-ONE-PATH               TO    WS-NAME::Trim()
           ELSE
               STRING    WS-FOLDER::Trim() "/" WS-NAME::Trim() INTO WS-ONE-PATH
           END-IF
           SET     WS-FOUND                      TO    "N"
           PERFORM    VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NODE-N
               IF    WS-NODE-PATH(WS-J) = WS-ONE-PATH
                   SET     WS-FOUND              TO    "Y"
               END-IF
           END-PERFORM
           IF    WS-FOUND = "Y"
               SET     Lbl-Status::Caption       TO    T-FOLDER-EXISTS::Trim()
               EXIT PROGRAM
           END-IF
           OPEN    I-O FOLDERS-FILE
           IF    WS-FS = "35"
               OPEN    OUTPUT FOLDERS-FILE
               CLOSE    FOLDERS-FILE
               OPEN    I-O FOLDERS-FILE
           END-IF

           SET     FLD-TOPIC                     TO    WS-CUR-TOPIC
           SET     FLD-PATH                      TO    WS-ONE-PATH

           WRITE    FOLDER-REC
               INVALID KEY CONTINUE
           END-WRITE
           COMMIT
           CLOSE    FOLDERS-FILE
           *>   The new folder becomes the selection, so a drop goes into it.

           SET     WS-SEL-KIND                   TO    "F"
           SET     WS-SEL-PATH                   TO    WS-ONE-PATH
           SET     Txt-Folder::Text              TO    SPACES

           CALL    "PC-LIST-DOCS"

           SET     WS-FMT                        TO    T-FOLDER-CREATED
           SET     WS-ARG1                       TO    WS-SEL-PATH

           CALL    "PC-FMT"
           SET     Lbl-Status::Caption           TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM BTN-NEWFOLDER--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MOVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   First press: the selected document is the one to move. Second
           *>   press: it goes into the selected folder (or a document's folder).

           IF    WS-MOVE-STEP NOT = SPACE
               SET     Lbl-Status::Caption       TO    T-KB-UPDATING::Trim()
               EXIT PROGRAM
           END-IF
           IF    WS-MOVE-SRC = SPACES
               IF    WS-SEL-IX = 0 OR WS-SEL-KIND NOT = "D"
                   SET     Lbl-Status::Caption   TO    T-PICK-DOC-TO-MOVE::Trim()
                   EXIT PROGRAM
               END-IF
               SET     WS-MOVE-SRC               TO    WS-SEL-PATH
               CALL    "PC-MOVE-UI"

               SET     WS-FMT                    TO    T-MOVE-PICK
               SET     WS-ARG1                   TO    WS-MOVE-SRC

               CALL    "PC-FMT"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-MOVE-TO                    TO    WS-FOLDER
           CALL    "PC-MOVE-GO"

           GOBACK.

       END PROGRAM BTN-MOVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MOVETOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Place the document picked with Move at the top level.

           IF    WS-MOVE-SRC = SPACES OR WS-MOVE-STEP NOT = SPACE
               EXIT PROGRAM
           END-IF
           SET     WS-MOVE-TO                    TO    SPACES
           CALL    "PC-MOVE-GO"

           GOBACK.

       END PROGRAM BTN-MOVETOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-D--ONPROGRESS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The progress strip (R16a): which document, how many of how many.
           *>   The strip is always on the form; it is empty while nothing indexes.
           *>   Fetching the model counts kilobytes of one file, not documents.

           IF    WS-KB-STEP = "M"
               SET     WS-KB1                    TO    KB-D::ProgressCurrent
               SET     WS-KB2                    TO    KB-D::ProgressTotal
               SET     Prg-Index::Maximum        TO    KB-D::ProgressTotal
               SET     Prg-Index::Value          TO    KB-D::ProgressCurrent
               SET     Prg-Index::Visible        TO    TRUE
               SET     WS-FMT                    TO    T-MODEL-FETCH
               SET     WS-ARG1                   TO    KB-D::ProgressDocument
               SET     WS-ARG2                   TO    WS-KB1
               SET     WS-ARG3                   TO    WS-KB2

               CALL    "PC-FMT"
               SET     Lbl-Progress::Caption     TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           *>   Two bars (operator, 2026-09-27): documents split into passages,
           *>   and the passages of the document in hand given their embeddings.
           *>   Each shows while its stage runs and goes when it reaches 100 %.

           SET     WS-NUM-TEXT                   TO    KB-D::ProgressCurrent
           COMPUTE    WS-DOC-CUR = FUNCTION NUMVAL(WS-NUM-TEXT)
           SET     WS-NUM-TEXT                   TO    KB-D::ProgressTotal
           COMPUTE    WS-DOC-TOT = FUNCTION NUMVAL(WS-NUM-TEXT)
           SET     WS-NUM-TEXT                   TO    KB-D::ProgressPassage
           COMPUTE    WS-PASSAGE = FUNCTION NUMVAL(WS-NUM-TEXT)
           SET     WS-NUM-TEXT                   TO    KB-D::ProgressPassages
           COMPUTE    WS-PASSAGES = FUNCTION NUMVAL(WS-NUM-TEXT)
           *>   Chunking is done once the last document is split: when its
           *>   embedding has begun, or when it is finished.

           IF    WS-DOC-TOT > 0 AND (WS-DOC-CUR < WS-DOC-TOT
                   OR (WS-PASSAGES > 0 AND WS-PASSAGE = 0))

               SET     WS-KB1                    TO    WS-DOC-CUR
               SET     WS-KB2                    TO    WS-DOC-TOT
               SET     WS-FMT                    TO    T-CHUNK-BAR
               SET     WS-ARG1                   TO    WS-KB1
               SET     WS-ARG2                   TO    WS-KB2

               CALL    "PC-FMT"

               SET     Lbl-Chunk::Caption        TO    WS-FMT-OUT::Trim()
               SET     Prg-Chunk::Maximum        TO    WS-DOC-TOT
               SET     Prg-Chunk::Value          TO    WS-DOC-CUR
               SET     Lbl-Chunk::Visible        TO    TRUE
               SET     Prg-Chunk::Visible        TO    TRUE
           ELSE
               SET     Lbl-Chunk::Visible        TO    FALSE
               SET     Prg-Chunk::Visible        TO    FALSE
           END-IF
           IF    WS-PASSAGES > 0 AND WS-PASSAGE < WS-PASSAGES
               SET     WS-KB1                    TO    WS-PASSAGE
               SET     WS-KB2                    TO    WS-PASSAGES
               SET     WS-FMT                    TO    T-EMBED-BAR
               SET     WS-ARG1                   TO    WS-KB1
               SET     WS-ARG2                   TO    WS-KB2

               CALL    "PC-FMT"

               SET     Lbl-Embed::Caption        TO    WS-FMT-OUT::Trim()
               SET     Prg-Embed::Maximum        TO    WS-PASSAGES
               SET     Prg-Embed::Value          TO    WS-PASSAGE
               SET     Lbl-Embed::Visible        TO    TRUE
               SET     Prg-Embed::Visible        TO    TRUE
           ELSE
               SET     Lbl-Embed::Visible        TO    FALSE
               SET     Prg-Embed::Visible        TO    FALSE
           END-IF

           SET     WS-N1                         TO    KB-D::ProgressCurrent
           SET     WS-N2                         TO    KB-D::ProgressTotal
           SET     WS-FMT                        TO    T-KB-PROGRESS
           SET     WS-ARG1                       TO    WS-N1
           SET     WS-ARG2                       TO    WS-N2
           SET     WS-ARG3                       TO    KB-D::ProgressDocument

           CALL    "PC-FMT"
           SET     Lbl-Progress::Caption         TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM KB-D--ONPROGRESS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-D--ONINDEXED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The model is here (or was already): now the folder.

           *> One handler ends every job; the markers say which one: WS-KB-STEP M
           *> (model fetched), WS-UP-STEP U (upload), WS-MOVE-STEP I then D (move),
           *> WS-DELETING (delete). None of them, or R: the refresh summary below.

           IF    WS-KB-STEP = "M"
               SET     WS-KB-STEP                TO    SPACE

               CALL    "PC-PROGRESS-IDLE"
               CALL    "PC-KB-REFRESH"

               EXIT PROGRAM
           END-IF
           *>   An upload imported: say which, and whether it was new; then the
           *>   next one.

           IF    WS-UP-STEP = "U"
               SET     WS-UP-STEP                TO    SPACE
               SET     WS-UP-STATE(WS-UP-CUR)    TO    "D"

               CALL    "PC-PROGRESS-IDLE"
               CALL    "PC-LIST-DOCS"

               SET     WS-NUM-TEXT               TO    KB-D::UpdatedCount
               COMPUTE    WS-UP-COUNT = FUNCTION NUMVAL(WS-NUM-TEXT)
               IF    WS-UP-COUNT > 0
                   SET     WS-FMT                TO    T-UP-UPDATED
                   ADD    1 TO WS-UP-UPDATED
               ELSE
                   SET     WS-FMT                TO    T-UP-ADDED
                   ADD    1 TO WS-UP-ADDED
               END-IF
               SET     WS-ARG1                   TO    WS-UP-DEST(WS-UP-CUR)
               CALL    "PC-FMT"

               SET     WS-LINE                   TO    WS-FMT-OUT
               SET     WS-NUM-TEXT               TO    KB-D::PassageCount

               COMPUTE    WS-PASSAGES = FUNCTION NUMVAL(WS-NUM-TEXT)

               SET     WS-KB1                    TO    WS-PASSAGES
               SET     WS-NUM-TEXT               TO    KB-D::SearchMode

               IF    WS-NUM-TEXT = "Semantic"
                   SET     WS-FMT                TO    T-KB-EMBEDDED
               ELSE
                   SET     WS-FMT                TO    T-KB-PASSAGES
               END-IF
               SET     WS-ARG1                   TO    WS-KB1
               CALL    "PC-FMT"

               SET     WS-ARG2                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG2::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
               IF    WS-REJECT-MSG NOT = SPACES
                   SET     WS-ARG2               TO    WS-LINE
                   SET     WS-LINE               TO    SPACES

                   STRING    WS-ARG2::Trim() " " WS-REJECT-MSG::Trim() INTO WS-LINE
                   SET     WS-REJECT-MSG         TO    SPACES
               END-IF
               SET     Lbl-Status::Caption       TO    WS-LINE
               CALL    "PC-UP-NEXT"
               EXIT PROGRAM
           END-IF
           *>   A move's copy is indexed: now the original goes. Its own onIndexed
           *>   lists the tree and reports the move.

           IF    WS-MOVE-STEP = "I"
               SET     WS-OK                     TO    KB-D::DeleteDocument(WS-MOVE-SRC::Trim())
               IF    WS-OK = "1"
                   SET     WS-MOVE-STEP          TO    "D"
                   EXIT PROGRAM
               END-IF

               SET     WS-MOVE-STEP              TO    SPACE
               SET     WS-MOVE-SRC               TO    SPACES

               CALL    "PC-MOVE-UI"
               CALL    "PC-PROGRESS-IDLE"

               SET     Lbl-Status::Caption       TO    KB-D::LastError
               CALL    "PC-LIST-DOCS"
               EXIT PROGRAM
           END-IF
           CALL    "PC-PROGRESS-IDLE"
           *>   A document deleted: say which, and that it left the index.

           IF    WS-DELETING NOT = SPACES
               *>       The tree first: PC-LIST-DOCS formats its own count line.

               SET     WS-SEL-KIND WS-SEL-PATH   TO    SPACES
               CALL    "PC-LIST-DOCS"

               SET     WS-FMT                    TO    T-DOC-REMOVED
               SET     WS-ARG1                   TO    WS-DELETING

               CALL    "PC-FMT"

               SET     WS-DELETING               TO    SPACES
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()

               EXIT PROGRAM
           END-IF
           IF    WS-MOVE-STEP = "D"
               SET     WS-MOVE-STEP              TO    SPACE
               SET     WS-SEL-KIND               TO    "D"
               SET     WS-SEL-PATH               TO    WS-MOVE-DEST

               CALL    "PC-LIST-DOCS"

               SET     WS-FMT                    TO    T-MOVED
               SET     WS-ARG1                   TO    WS-MOVE-SRC

               IF    WS-MOVE-TO = SPACES
                   SET     WS-ARG2               TO    T-TOP-LEVEL
               ELSE
                   SET     WS-ARG2               TO    WS-MOVE-TO
               END-IF
               CALL    "PC-FMT"

               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               SET     WS-MOVE-SRC               TO    SPACES

               CALL    "PC-MOVE-UI"
               EXIT PROGRAM
           END-IF
           *>   A collection indexed with another embedder (the lexical one, before
           *>   PowerChat used the semantic model) is only stored as text: rebuild
           *>   it once, so its documents get their vectors.

           SET     WS-MODE-WHY                   TO    KB-D::SearchModeReason
           SET     WS-TALLY                      TO    0

           INSPECT    WS-MODE-WHY TALLYING WS-TALLY FOR ALL "another embedder"
           *> The WS-KB-STEP test keeps a rebuild from repeating: it is "R" while
           *> one runs, so the onIndexed that ends it does not start another.

           IF    WS-TALLY > 0 AND WS-KB-STEP = SPACE
               SET     KB-D::SearchModeReason    TO    SPACES
               SET     WS-OK                     TO    KB-D::Reindex()

               IF    WS-OK = "1"
                   SET     WS-KB-STEP            TO    "R"
                   SET     Lbl-Progress::Caption TO    T-REINDEXING::Trim()

                   EXIT PROGRAM
               END-IF
           END-IF

           SET     WS-KB-STEP                    TO    SPACE
           SET     WS-N1                         TO    KB-D::AddedCount
           SET     WS-N2                         TO    KB-D::UpdatedCount
           SET     WS-N3                         TO    KB-D::RemovedCount
           SET     WS-N4                         TO    KB-D::SkippedCount
           SET     WS-FMT                        TO    T-KB-COUNTS
           SET     WS-ARG1                       TO    WS-N1
           SET     WS-ARG2                       TO    WS-N2
           SET     WS-ARG3                       TO    WS-N3
           SET     WS-ARG4                       TO    WS-N4

           CALL    "PC-FMT"

           SET     WS-LINE                       TO    WS-FMT-OUT
           SET     WS-SKIPPED                    TO    KB-D::SkippedDocuments

           IF    WS-SKIPPED NOT = SPACES
               SET     WS-FMT                    TO    T-KB-SKIPPED
               SET     WS-ARG1                   TO    WS-SKIPPED

               CALL    "PC-FMT"

               SET     WS-ARG2                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG2::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
           END-IF
           *>   What was stored, said outright: how many passages, and whether
           *>   they carry embeddings (operator, 2026-09-27: no confirmation that
           *>   the chunking and the embedding were done).

           SET     WS-NUM-TEXT                   TO    KB-D::PassageCount
           COMPUTE    WS-PASSAGES = FUNCTION NUMVAL(WS-NUM-TEXT)
           IF    WS-PASSAGES > 0
               SET     WS-KB1                    TO    WS-PASSAGES
               SET     WS-NUM-TEXT               TO    KB-D::SearchMode

               IF    WS-NUM-TEXT = "Semantic"
                   SET     WS-FMT                TO    T-KB-EMBEDDED
               ELSE
                   SET     WS-FMT                TO    T-KB-PASSAGES
               END-IF
               SET     WS-ARG1                   TO    WS-KB1
               CALL    "PC-FMT"

               SET     WS-ARG2                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG2::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
           END-IF
           *>   R18: never degrade silently - say when search is by words only.

           IF    WS-MODE-WHY NOT = SPACES
               SET     WS-FMT                    TO    T-WORDS-ONLY
               SET     WS-ARG1                   TO    WS-MODE-WHY

               CALL    "PC-FMT"

               SET     WS-ARG2                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG2::Trim() " " WS-FMT-OUT::Trim() INTO WS-LINE
           END-IF
           IF    WS-REJECT-MSG NOT = SPACES
               SET     WS-ARG2                   TO    WS-LINE
               SET     WS-LINE                   TO    SPACES

               STRING    WS-ARG2::Trim() " " WS-REJECT-MSG::Trim() INTO WS-LINE
               SET     WS-REJECT-MSG             TO    SPACES
           END-IF
           SET     Lbl-Status::Caption           TO    WS-LINE
           CALL    "PC-LIST-DOCS"
           *>   Uploads that arrived while this update ran go now.

           IF    WS-UP-N > 0
               CALL    "PC-UP-NEXT"
           END-IF

           GOBACK.

       END PROGRAM KB-D--ONINDEXED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-D--ONBUSY IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-PROGRESS-IDLE"

           SET     Lbl-Status::Caption           TO    KB-D::LastError
           SET     WS-DELETING                   TO    SPACES

           IF    WS-UP-STEP = "U"
               SET     WS-UP-STEP                TO    SPACE
               SET     WS-UP-STATE(WS-UP-CUR)    TO    "D"

               CALL    "PC-UP-NEXT"
           END-IF
           *>   No model (no network, say): index anyway - by words, and the
           *>   summary says so. A failed rebuild leaves the text-only index.

           IF    WS-KB-STEP = "M"
               SET     WS-KB-STEP                TO    SPACE
               CALL    "PC-KB-REFRESH"
           END-IF
           SET     WS-KB-STEP                    TO    SPACE
           *>   A move stops where it failed: before the delete, the original
           *>   is untouched; the tree shows what is really there.

           IF    WS-MOVE-STEP NOT = SPACE
               SET     WS-MOVE-STEP              TO    SPACE
               SET     WS-MOVE-SRC               TO    SPACES

               CALL    "PC-MOVE-UI"
               CALL    "PC-LIST-DOCS"
           END-IF

           GOBACK.

       END PROGRAM KB-D--ONBUSY.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-D--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           CALL    "PC-PROGRESS-IDLE"

           SET     Lbl-Status::Caption           TO    KB-D::LastError
           SET     WS-DELETING                   TO    SPACES

           IF    WS-UP-STEP = "U"
               SET     WS-UP-STEP                TO    SPACE
               SET     WS-UP-STATE(WS-UP-CUR)    TO    "D"

               CALL    "PC-UP-NEXT"
           END-IF
           *>   No model (no network, say): index anyway - by words, and the
           *>   summary says so. A failed rebuild leaves the text-only index.

           IF    WS-KB-STEP = "M"
               SET     WS-KB-STEP                TO    SPACE
               CALL    "PC-KB-REFRESH"
           END-IF
           SET     WS-KB-STEP                    TO    SPACE
           *>   A move stops where it failed: before the delete, the original
           *>   is untouched; the tree shows what is really there.

           IF    WS-MOVE-STEP NOT = SPACE
               SET     WS-MOVE-STEP              TO    SPACE
               SET     WS-MOVE-SRC               TO    SPACES

               CALL    "PC-MOVE-UI"
               CALL    "PC-LIST-DOCS"
           END-IF

           GOBACK.

       END PROGRAM KB-D--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-LANG--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A flag in the chat's menu changes the language while this form is
           *>   on the pane; nothing tells a pane occupant, so it looks (R46).

           CALL    "PC-LANG-NOW"
           IF    WS-LANG-NOW NOT = WS-LANG
               CALL    "DOCUMENTS-FORM--ONACTIVATE"
           END-IF

           GOBACK.

       END PROGRAM TMR-LANG--ONTICK.

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
           SET     WS-FOLDERS-PATH               TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/folders.idx" INTO WS-FOLDERS-PATH

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
       PROGRAM-ID. PC-LIST-DOCS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The topic's documents and folders as a tree (R49): gathered into
           *>   WS-TREE, sorted so each folder comes right before what is in it,
           *>   then added node by node.

           SET     WS-NODE-N                     TO    0
           SET     WS-DOC-COUNT                  TO    KB-D::ListDocuments()

           PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-DOC-COUNT
               SET     WS-DOC                    TO    KB-D::GetDocument(WS-I)
               SET     WS-ONE-KIND               TO    "D"
               SET     WS-ONE-PATH               TO    WS-DOC::Trim()

               CALL    "PC-TREE-ADD"
           END-PERFORM
           *>   The folders made here, including the empty ones.

           *> WS-FS doubles as the loop flag: anything but "00" ends the scan - no
           *> folders file yet, a failed START, the end of the file, or the first
           *> record of another topic (the last three set it to "10" by hand).

           OPEN    INPUT FOLDERS-FILE
           IF    WS-FS = "00"
               SET     FLD-TOPIC                 TO    WS-CUR-TOPIC
               SET     FLD-PATH                  TO    LOW-VALUES

               START    FOLDERS-FILE KEY IS NOT LESS THAN FLD-KEY
                   INVALID KEY MOVE "10" TO WS-FS
               END-START
               PERFORM    UNTIL WS-FS NOT = "00"
                   READ    FOLDERS-FILE NEXT
                       AT END MOVE "10" TO WS-FS
                   END-READ
                   IF    WS-FS = "00"
                       IF    FLD-TOPIC NOT = WS-CUR-TOPIC
                           SET     WS-FS         TO    "10"
                       ELSE
                           SET     WS-ONE-KIND   TO    "F"
                           SET     WS-ONE-PATH   TO    FLD-PATH

                           CALL    "PC-TREE-ADD"
                       END-IF
                   END-IF
               END-PERFORM
               CLOSE    FOLDERS-FILE
           END-IF
           *>   Insertion sort on the key; a topic holds tens of entries.

           PERFORM    VARYING WS-J FROM 2 BY 1 UNTIL WS-J > WS-NODE-N
               SET     WS-NODE-TMP               TO    WS-NODE(WS-J)
               SET     WS-K                      TO    WS-J
               SET     WS-FOUND                  TO    "N"

               PERFORM    UNTIL WS-K < 2 OR WS-FOUND = "Y"
                   COMPUTE    WS-K1 = WS-K - 1
                   IF    WS-NODE-KEY(WS-K1) > TMP-KEY
                       SET     WS-NODE(WS-K)     TO    WS-NODE(WS-K1)
                       SET     WS-K              TO    WS-K1
                   ELSE
                       SET     WS-FOUND          TO    "Y"
                   END-IF
               END-PERFORM
               SET     WS-NODE(WS-K)             TO    WS-NODE-TMP
           END-PERFORM
           SET     WS-OK                         TO    Trv-Docs::Clear()
           PERFORM    VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NODE-N
               *>       Depth from 1 (a top-level entry), one more per "/".

               SET     WS-DEPTH                  TO    1
               INSPECT    WS-NODE-PATH(WS-J) TALLYING WS-DEPTH FOR ALL "/"

               *> The label is what follows the last "/" (WS-P stays 0 for a top-level
               *> entry, so the whole path is the label).

               SET     WS-L                      TO    FUNCTION LENGTH(WS-NODE-PATH(WS-J)::Trim())
               SET     WS-P                      TO    0

               PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
                   IF    WS-NODE-PATH(WS-J)(WS-K:1) = "/"
                       SET     WS-P              TO    WS-K
                   END-IF
               END-PERFORM

               SET     WS-NAME                   TO    SPACES
               SET     WS-NAME                   TO    WS-NODE-PATH(WS-J)(WS-P + 1:WS-L - WS-P)

               *>       A folder with something under it takes the tree's own
               *>       open/shut folder icon; an empty one has to be told.

               SET     WS-ICON                   TO    SPACES
               IF    WS-NODE-KIND(WS-J) = "F"
                   SET     WS-ICON               TO    "folder"
                   *> The sort puts a folder's contents right after it, so the next row is
                   *> enough: it starts with "path/" only when the folder is not empty.

                   IF    WS-J < WS-NODE-N
                       COMPUTE    WS-K1 = WS-J + 1
                       SET     WS-KEY-TMP        TO    SPACES
                       STRING    WS-NODE-PATH(WS-J)::Trim() "/" INTO WS-KEY-TMP
                       IF    WS-NODE-PATH(WS-K1)(1:WS-L + 1) = WS-KEY-TMP(1:WS-L + 1)
                           SET     WS-ICON       TO    SPACES
                       END-IF
                   END-IF
               END-IF
               IF    WS-ICON = SPACES
                   SET     WS-OK                 TO    Trv-Docs::AddNode(WS-DEPTH, WS-NAME::Trim())
               ELSE
                   MOVE    Trv-Docs::AddNode(WS-DEPTH, WS-NAME::Trim(),
                   WS-ICON::Trim()) TO WS-OK
               END-IF
           END-PERFORM

           SET     WS-N1                         TO    WS-DOC-COUNT
           SET     WS-FMT                        TO    T-DOC-COUNT
           SET     WS-ARG1                       TO    WS-N1

           CALL    "PC-FMT"
           SET     Lbl-Count::Caption            TO    WS-FMT-OUT::Trim()
           CALL    "PC-SELECT-SYNC"

           GOBACK.

       END PROGRAM PC-LIST-DOCS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TREE-ADD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 LK-KIND                                PIC    X.
       01 LK-PATH                                PIC    X(260).
       01 LK-L                                   PIC    9(4).
       01 LK-P                                   PIC    9(4).

       PROCEDURE DIVISION.
           *>   WS-ONE-PATH (of kind WS-ONE-KIND) and every folder above it go
           *>   into WS-TREE, each once.

           *> WS-ONE-KIND and WS-ONE-PATH are reused for each parent folder, so the
           *> caller's values wait in LK-* and go back for the last row. Every "/"
           *> past the first character closes a parent folder that needs its row.

           SET     LK-KIND                       TO    WS-ONE-KIND
           SET     LK-PATH                       TO    WS-ONE-PATH
           SET     LK-L                          TO    FUNCTION LENGTH(FUNCTION TRIM(LK-PATH))

           PERFORM    VARYING LK-P FROM 2 BY 1 UNTIL LK-P > LK-L
               IF    LK-PATH(LK-P:1) = "/"
                   SET     WS-ONE-KIND           TO    "F"
                   SET     WS-ONE-PATH           TO    SPACES
                   SET     WS-ONE-PATH           TO    LK-PATH(1:LK-P - 1)

                   CALL    "PC-TREE-ONE"
               END-IF
           END-PERFORM

           SET     WS-ONE-KIND                   TO    LK-KIND
           SET     WS-ONE-PATH                   TO    LK-PATH

           CALL    "PC-TREE-ONE"

           GOBACK.

       END PROGRAM PC-TREE-ADD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TREE-ONE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   One row, unless the path is there already. The sort key is the
           *>   path with "/" below every other character and LOW-VALUES after it,
           *>   so a folder sorts right before its own contents.

           SET     WS-FOUND                      TO    "N"
           PERFORM    VARYING WS-J FROM 1 BY 1
                   UNTIL WS-J > WS-NODE-N OR WS-FOUND = "Y"
               IF    WS-NODE-PATH(WS-J) = WS-ONE-PATH
                   SET     WS-FOUND              TO    "Y"
               END-IF
           END-PERFORM
           *> WS-TREE holds 1000 rows; once it is full, further entries are left
           *> out without any message.

           IF    WS-FOUND = "N" AND WS-NODE-N < 1000
               ADD    1 TO WS-NODE-N

               SET     WS-NODE-KIND(WS-NODE-N)   TO    WS-ONE-KIND
               SET     WS-NODE-PATH(WS-NODE-N)   TO    WS-ONE-PATH
               SET     WS-KEY-TMP                TO    LOW-VALUES
               SET     WS-L                      TO    FUNCTION LENGTH(WS-ONE-PATH::Trim())
               SET     WS-KEY-TMP(1:WS-L)        TO    WS-ONE-PATH(1:WS-L)

               INSPECT    WS-KEY-TMP(1:WS-L) CONVERTING "/" TO X"01"
               SET     WS-NODE-KEY(WS-NODE-N)    TO    WS-KEY-TMP
           END-IF

           GOBACK.

       END PROGRAM PC-TREE-ONE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SELECT-SYNC IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Find the selection again after a rebuild, and point the drop zone
           *>   at the folder it names - a document's own folder - or at the top.

           SET     WS-SEL-IX                     TO    0
           IF    WS-SEL-PATH NOT = SPACES
               PERFORM    VARYING WS-J FROM 1 BY 1
                       UNTIL WS-J > WS-NODE-N OR WS-SEL-IX > 0
                   IF    WS-NODE-PATH(WS-J) = WS-SEL-PATH
                           AND WS-NODE-KIND(WS-J) = WS-SEL-KIND
                       SET     WS-SEL-IX         TO    WS-J
                   END-IF
               END-PERFORM
           END-IF
           SET     WS-FOLDER                     TO    SPACES
           IF    WS-SEL-IX = 0
               SET     WS-SEL-KIND WS-SEL-PATH   TO    SPACES
           ELSE
               IF    WS-SEL-KIND = "F"
                   SET     WS-FOLDER             TO    WS-SEL-PATH
               ELSE
                   SET     WS-L                  TO    FUNCTION LENGTH(WS-SEL-PATH::Trim())
                   SET     WS-P                  TO    0

                   PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
                       IF    WS-SEL-PATH(WS-K:1) = "/"
                           SET     WS-P          TO    WS-K
                       END-IF
                   END-PERFORM
                   IF    WS-P > 1
                       SET     WS-FOLDER         TO    WS-SEL-PATH(1:WS-P - 1)
                   END-IF
               END-IF
           END-IF
           SET     WS-LINE                       TO    SPACES
           IF    WS-FOLDER = SPACES
               SET     WS-LINE                   TO    WS-DOCS-DIR
           ELSE
               STRING    WS-DOCS-DIR::Trim() "/" WS-FOLDER::Trim() INTO WS-LINE
           END-IF
           *>   The zone copies nothing itself (uploads are imported, asking
           *>   first when a name is taken): WS-FOLDER is where they go.

           GOBACK.

       END PROGRAM PC-SELECT-SYNC.

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
           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 54
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Lbl-Title::Caption            TO    T-DOCS-TITLE::Trim()
           SET     Drop-Docs::Hint               TO    T-DROP-HINT::Trim()
           SET     Btn-Delete::Caption           TO    T-DELETE::Trim()
           SET     Btn-Preview::Caption          TO    T-PREVIEW::Trim()
           SET     Btn-Refresh::Caption          TO    T-REFRESH::Trim()
           SET     Btn-NewFolder::Caption        TO    T-NEW-FOLDER::Trim()
           SET     Txt-Folder::HintText          TO    T-FOLDER-HINT::Trim()
           SET     Lbl-Status::Caption           TO    T-STATUS::Trim()
           SET     Btn-MoveTop::Caption          TO    T-MOVE-TOP::Trim()

           CALL    "PC-MOVE-UI"

           SET     WS-FMT                        TO    T-ACCEPTED-TYPES
           SET     WS-ARG1                       TO    WS-DOC-TYPES

           CALL    "PC-FMT"
           SET     Lbl-Types::Caption            TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM PC-TEXTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-UP-NEXT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Import the next queued upload; when none is left, the summary.
           *>   Busy with something else: stay queued - onIndexed calls back.

           IF    WS-UP-STEP NOT = SPACE
               EXIT PROGRAM
           END-IF
           SET     WS-UP-CUR                     TO    0
           PERFORM    VARYING WS-UP-I FROM 1 BY 1
                   UNTIL WS-UP-I > WS-UP-N OR WS-UP-CUR > 0
               IF    WS-UP-STATE(WS-UP-I) = "Q"
                   SET     WS-UP-CUR             TO    WS-UP-I
               END-IF
           END-PERFORM
           IF    WS-UP-CUR = 0
               IF    WS-UP-N > 1
                   SET     WS-FMT                TO    T-UP-SUMMARY
                   SET     WS-KB1                TO    WS-UP-ADDED
                   SET     WS-ARG1               TO    WS-KB1
                   SET     WS-KB1                TO    WS-UP-UPDATED
                   SET     WS-ARG2               TO    WS-KB1
                   SET     WS-KB1                TO    WS-UP-KEPT
                   SET     WS-ARG3               TO    WS-KB1

                   CALL    "PC-FMT"
                   SET     Lbl-Status::Caption   TO    WS-FMT-OUT::Trim()
               END-IF
               SET     WS-UP-N                   TO    0
               EXIT PROGRAM
           END-IF
           SET     KB-D::SearchModeReason        TO    SPACES
           MOVE    KB-D::ImportDocument(WS-UP-SRC(WS-UP-CUR)::Trim(),
           WS-UP-DEST(WS-UP-CUR)::Trim()) TO WS-OK
           IF    WS-OK NOT = "1"
               *>       Another update is running: this one waits for its end.

               EXIT PROGRAM
           END-IF

           SET     WS-UP-STEP                    TO    "U"
           SET     WS-FMT                        TO    T-UP-SENDING
           SET     WS-ARG1                       TO    WS-UP-DEST(WS-UP-CUR)

           CALL    "PC-FMT"
           SET     Lbl-Progress::Caption         TO    WS-FMT-OUT::Trim()

           GOBACK.

       END PROGRAM PC-UP-NEXT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PREVIEW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The selected document, shown in PREVIEW-FORM (a modal window).

           IF    WS-SEL-IX = 0 OR WS-SEL-KIND NOT = "D"
               SET     Lbl-Status::Caption       TO    T-PICK-DOC-TO-PREVIEW::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-LINE                       TO    SPACES
           STRING    WS-DOCS-DIR::Trim() "/" WS-SEL-PATH::Trim() INTO WS-LINE

           INVOKE    ME::SetProperty("PreviewPath", WS-LINE)
           INVOKE    ME::SetProperty("PreviewTitle", WS-SEL-PATH)
           INVOKE    ME::SetProperty("PreviewClose", T-CLOSE)
           INVOKE    ME::OpenFormSync("PREVIEW-FORM")

           GOBACK.

       END PROGRAM PC-PREVIEW.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-KB-REFRESH IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Bring the index up to date with the folder. SearchModeReason is
           *>   only written when there is something to say, so it is cleared
           *>   first: an old warning must not outlive a clean update.

           SET     KB-D::SearchModeReason        TO    SPACES
           SET     WS-OK                         TO    KB-D::Refresh()

           IF    WS-OK NOT = "1"
               SET     Lbl-Status::Caption       TO    KB-D::LastError
           ELSE
               SET     Lbl-Progress::Caption     TO    T-KB-UPDATING::Trim()
           END-IF

           GOBACK.

       END PROGRAM PC-KB-REFRESH.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MOVE-UI IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The Move button says what the next press does: pick the document,
           *>   or place the one picked. "To the top level" only while one is.

           IF    WS-MOVE-SRC = SPACES OR WS-MOVE-STEP NOT = SPACE
               SET     Btn-Move::Caption         TO    T-MOVE::Trim()
               SET     Btn-MoveTop::Enabled      TO    "false"
           ELSE
               SET     Btn-Move::Caption         TO    T-MOVE-HERE::Trim()
               SET     Btn-MoveTop::Enabled      TO    "true"
           END-IF

           GOBACK.

       END PROGRAM PC-MOVE-UI.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-MOVE-GO IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 LK-FILE                                PIC    X(260).

       PROCEDURE DIVISION.
           *>   WS-MOVE-SRC goes into folder WS-MOVE-TO (spaces: the top level).
           *>   Step one copies it there through the Knowledge Base, which indexes
           *>   the copy; onIndexed then deletes the original (step two).

           SET     WS-L                          TO    FUNCTION LENGTH(WS-MOVE-SRC::Trim())
           SET     WS-P                          TO    0

           PERFORM    VARYING WS-K FROM 1 BY 1 UNTIL WS-K > WS-L
               IF    WS-MOVE-SRC(WS-K:1) = "/"
                   SET     WS-P                  TO    WS-K
               END-IF
           END-PERFORM

           SET     LK-FILE                       TO    SPACES
           SET     LK-FILE                       TO    WS-MOVE-SRC(WS-P + 1:WS-L - WS-P)
           SET     WS-MOVE-DEST                  TO    SPACES

           IF    WS-MOVE-TO = SPACES
               SET     WS-MOVE-DEST              TO    FUNCTION TRIM(LK-FILE)
           ELSE
               STRING    WS-MOVE-TO::Trim() "/" FUNCTION TRIM(LK-FILE) INTO WS-MOVE-DEST
           END-IF
           IF    WS-MOVE-DEST = WS-MOVE-SRC
               SET     WS-MOVE-SRC               TO    SPACES
               CALL    "PC-MOVE-UI"
               SET     Lbl-Status::Caption       TO    T-MOVE-SAME::Trim()
               EXIT PROGRAM
           END-IF
           *>   Never over another document: the copy would replace it.

           SET     WS-FOUND                      TO    "N"
           PERFORM    VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NODE-N
               IF    WS-NODE-PATH(WS-J) = WS-MOVE-DEST
                   SET     WS-FOUND              TO    "Y"
               END-IF
           END-PERFORM
           IF    WS-FOUND = "Y"
               SET     WS-FMT                    TO    T-MOVE-EXISTS
               SET     WS-ARG1                   TO    WS-MOVE-DEST

               CALL    "PC-FMT"
               SET     WS-MOVE-SRC               TO    SPACES
               CALL    "PC-MOVE-UI"
               SET     Lbl-Status::Caption       TO    WS-FMT-OUT::Trim()
               EXIT PROGRAM
           END-IF
           SET     WS-LINE                       TO    SPACES
           STRING    WS-DOCS-DIR::Trim() "/" WS-MOVE-SRC::Trim() INTO WS-LINE
           MOVE    KB-D::ImportDocument(WS-LINE::Trim(),
           WS-MOVE-DEST::Trim()) TO WS-OK
           IF    WS-OK NOT = "1"
               SET     WS-MOVE-SRC               TO    SPACES
               CALL    "PC-MOVE-UI"
               SET     Lbl-Status::Caption       TO    KB-D::LastError
               EXIT PROGRAM
           END-IF
           SET     WS-MOVE-STEP                  TO    "I"
           CALL    "PC-MOVE-UI"
           SET     Lbl-Progress::Caption         TO    T-KB-UPDATING::Trim()

           GOBACK.

       END PROGRAM PC-MOVE-GO.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FMT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-PT                                  PIC    9(4).
       01 WS-FROM                                PIC    9(4).
       01 WS-CNT                                 PIC    9(4).
       01 WS-PART                                PIC    X(70).
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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-OPEN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   What the form shows: run on load, and again when it comes back.

           CALL    "PC-PATHS"
           CALL    "PC-TEXTS"

           *>   The zone refuses whatever the Knowledge Base could not read.

           SET     Drop-Docs::AllowedExtensions  TO    WS-DOC-TYPES
           SET     WS-SET-NAME                   TO    "CUR-TOPIC"

           CALL    "PC-SETTING-GET"

           SET     WS-CUR-TOPIC                  TO    WS-SET-VALUE(1:16)
           SET     WS-SET-NAME                   TO    "KB-LOCATION"

           CALL    "PC-SETTING-GET"
           SET     WS-KB-LOCATION                TO    WS-SET-VALUE
           IF    WS-KB-LOCATION = SPACES
               SET     WS-KB-LOCATION            TO    "assets/KB"
           END-IF
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

           SET     WS-FMT                        TO    T-DOCS-OF
           SET     WS-ARG1                       TO    TOP-NAME

           CALL    "PC-FMT"

           SET     Lbl-Title::Caption            TO    WS-FMT-OUT::Trim()
           SET     KB-D::Location                TO    WS-KB-LOCATION
           SET     KB-D::Collection              TO    WS-CUR-TOPIC

           *>   Dropped files land in the collection's documents folder; a refresh
           *>   then indexes them. So does anything copied there by other means.
           *>   PC-SELECT-SYNC points the zone at the selected folder inside it.

           SET     WS-DOCS-DIR                   TO    SPACES
           STRING    WS-KB-LOCATION::Trim() "/" WS-CUR-TOPIC::Trim() "/documents" INTO WS-DOCS-DIR
           CALL    "PC-LIST-DOCS"
           *>   The semantic model first - fetched once per installation, a quick
           *>   no-op after that - then pick up whatever changed in the folder
           *>   since the last run (R16). onIndexed runs the refresh after it.

           SET     WS-OK                         TO    KB-D::FetchModel()
           IF    WS-OK = "1"
               SET     WS-KB-STEP                TO    "M"
           END-IF

           GOBACK.

       END PROGRAM PC-OPEN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LANG-NOW IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The language saved now (LANG), into WS-LANG-NOW; "en" when unset.

           SET     WS-LANG-NOW                   TO    "en"
           *> Read only, unlike the other readers: a missing file (status 35) is
           *> not created here, it just means English.

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
       PROGRAM-ID. PC-PROGRESS-IDLE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Indexing ended (done, busy or failed): the progress strip goes
           *>   back to empty. It stays in its place on the form.

           SET     Lbl-Progress::Caption         TO    SPACES
           SET     Prg-Index::Value              TO    0
           SET     Prg-Index::Visible            TO    FALSE
           SET     Lbl-Chunk::Visible            TO    FALSE
           SET     Prg-Chunk::Visible            TO    FALSE
           SET     Lbl-Embed::Visible            TO    FALSE
           SET     Prg-Embed::Visible            TO    FALSE

           GOBACK.

       END PROGRAM PC-PROGRESS-IDLE.

       END PROGRAM DOCUMENTS-FORM.

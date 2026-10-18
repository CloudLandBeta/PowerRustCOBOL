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
       PROGRAM-ID. KB-FOLDER-FORM.

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

       DATA DIVISION.
       FILE SECTION.
       FD SETTINGS-FILE IS GLOBAL.
       01 SETTINGS-REC.
           05 SET-NAME                           PIC    X(20).
           05 SET-VALUE                          PIC    X(200).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'KB-FOLDER-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS                                  GLOBAL    PIC XX    VALUE "00".
       01 WS-DATA-DIR                            GLOBAL    PIC X(200).
       01 WS-SET-NAME                            GLOBAL    PIC X(20).
       01 WS-SET-VALUE                           GLOBAL    PIC X(200).
       01 WS-EOF                                 GLOBAL    PIC X.
       01 WS-OK                                  GLOBAL    PIC X(4).
       01 WS-SETTINGS-PATH                       GLOBAL    PIC X(240).
       01 WS-MODELS-PATH                         GLOBAL    PIC X(240).
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   KB-FOLDER-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Knowledge Base folder".
           05 FILLER                             PIC    X(120)    VALUE "Pasta da base de conhecimento".
           05 FILLER                             PIC    X(120)    VALUE "Carpeta de la base de conocimiento".
           05 FILLER                             PIC    X(120)    VALUE "Dossier de la base de connaissances".
           05 FILLER                             PIC    X(120)    VALUE "ナレッジベースのフォルダー".
           05 FILLER                             PIC    X(120)    VALUE "知识库文件夹".
           *>   KB-FOLDER-LABEL

           05 FILLER                             PIC    X(120)    VALUE "Knowledge Base folder (may be a folder on another machine)".
           05 FILLER                             PIC    X(120)    VALUE "Pasta da base de conhecimento (pode ser uma pasta em outra máquina)".
           05 FILLER                             PIC    X(120)    VALUE "Carpeta de la base de conocimiento (puede estar en otra máquina)".
           05 FILLER                             PIC    X(120)    VALUE "Dossier de la base de connaissances (il peut se trouver sur une autre machine)".
           05 FILLER                             PIC    X(120)    VALUE "ナレッジベースのフォルダー（別のマシン上のフォルダーでも可）".
           05 FILLER                             PIC    X(120)    VALUE "知识库文件夹（可以位于另一台机器上）".
           *>   KB-BROWSE-TITLE

           05 FILLER                             PIC    X(120)    VALUE "Choose the Knowledge Base folder".
           05 FILLER                             PIC    X(120)    VALUE "Escolha a pasta da base de conhecimento".
           05 FILLER                             PIC    X(120)    VALUE "Elige la carpeta de la base de conocimiento".
           05 FILLER                             PIC    X(120)    VALUE "Choisissez le dossier de la base de connaissances".
           05 FILLER                             PIC    X(120)    VALUE "ナレッジベースのフォルダーを選択".
           05 FILLER                             PIC    X(120)    VALUE "选择知识库文件夹".
           *>   KB-FOLDER-SAVED

           05 FILLER                             PIC    X(120)    VALUE "Knowledge Base folder saved.".
           05 FILLER                             PIC    X(120)    VALUE "Pasta da base de conhecimento salva.".
           05 FILLER                             PIC    X(120)    VALUE "Carpeta de la base de conocimiento guardada.".
           05 FILLER                             PIC    X(120)    VALUE "Dossier de la base de connaissances enregistré.".
           05 FILLER                             PIC    X(120)    VALUE "ナレッジベースのフォルダーを保存しました。".
           05 FILLER                             PIC    X(120)    VALUE "知识库文件夹已保存。".
           *>   SAVE

           05 FILLER                             PIC    X(120)    VALUE "Save".
           05 FILLER                             PIC    X(120)    VALUE "Salvar".
           05 FILLER                             PIC    X(120)    VALUE "Guardar".
           05 FILLER                             PIC    X(120)    VALUE "Enregistrer".
           05 FILLER                             PIC    X(120)    VALUE "保存".
           05 FILLER                             PIC    X(120)    VALUE "保存".
           *>   CANCEL

           05 FILLER                             PIC    X(120)    VALUE "Cancel".
           05 FILLER                             PIC    X(120)    VALUE "Cancelar".
           05 FILLER                             PIC    X(120)    VALUE "Cancelar".
           05 FILLER                             PIC    X(120)    VALUE "Annuler".
           05 FILLER                             PIC    X(120)    VALUE "キャンセル".
           05 FILLER                             PIC    X(120)    VALUE "取消".
           *>   STATUS

           05 FILLER                             PIC    X(120)    VALUE "Status".
           05 FILLER                             PIC    X(120)    VALUE "Status".
           05 FILLER                             PIC    X(120)    VALUE "Estado".
           05 FILLER                             PIC    X(120)    VALUE "État".
           05 FILLER                             PIC    X(120)    VALUE "ステータス".
           05 FILLER                             PIC    X(120)    VALUE "状态".
           *>   TIP-KB

           05 FILLER                             PIC    X(120)    VALUE "The folder where each topic keeps its documents and search index. It may be on another machine.".
           05 FILLER                             PIC    X(120)    VALUE "Pasta onde cada tópico guarda seus documentos e o índice de busca. Pode ficar em outra máquina.".
           05 FILLER                             PIC    X(120)    VALUE "Carpeta donde cada tema guarda sus documentos y su índice. Puede estar en otra máquina.".
           05 FILLER                             PIC    X(120)    VALUE "Dossier où chaque sujet garde ses documents et son index. Il peut être sur une autre machine.".
           05 FILLER                             PIC    X(120)    VALUE "各トピックの文書と検索索引を置くフォルダー。別のマシンにあってもかまいません。".
           05 FILLER                             PIC    X(120)    VALUE "每个主题存放文档和搜索索引的文件夹，可以位于另一台机器上。".
           *>   TIP-BROWSE

           05 FILLER                             PIC    X(120)    VALUE "Choose the folder in a window instead of typing it.".
           05 FILLER                             PIC    X(120)    VALUE "Escolher a pasta numa janela em vez de digitá-la.".
           05 FILLER                             PIC    X(120)    VALUE "Elegir la carpeta en una ventana en lugar de escribirla.".
           05 FILLER                             PIC    X(120)    VALUE "Choisir le dossier dans une fenêtre plutôt que le taper.".
           05 FILLER                             PIC    X(120)    VALUE "入力する代わりにウィンドウでフォルダーを選びます。".
           05 FILLER                             PIC    X(120)    VALUE "在窗口中选择文件夹，而不必手动输入。".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    9.
               10 PC-TEXT                        PIC    X(120)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-KB-FOLDER-TITLE                  PIC    X(120).
           05 T-KB-FOLDER-LABEL                  PIC    X(120).
           05 T-KB-BROWSE-TITLE                  PIC    X(120).
           05 T-KB-FOLDER-SAVED                  PIC    X(120).
           05 T-SAVE                             PIC    X(120).
           05 T-CANCEL                           PIC    X(120).
           05 T-STATUS                           PIC    X(120).
           05 T-TIP-KB                           PIC    X(120).
           05 T-TIP-BROWSE                       PIC    X(120).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(120)    OCCURS 9.
       *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.

       01 WS-FMT                                 GLOBAL    PIC X(120).
       01 WS-ARG1                                GLOBAL    PIC X(300).
       01 WS-ARG2                                GLOBAL    PIC X(300).
       01 WS-ARG3                                GLOBAL    PIC X(300).
       01 WS-ARG4                                GLOBAL    PIC X(300).
       01 WS-FMT-OUT                             GLOBAL    PIC X(1200).
       01 WS-PATH                                GLOBAL    PIC X(400).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Knowledge Base folder'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-KB.
          05 WS-Lbl-KB-TEXT       PIC X(256) VALUE 'Knowledge Base folder (may be a folder on another machine)'.
          05 WS-Lbl-KB-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-KB-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-KbLocation.
          05 WS-Txt-KbLocation-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-KbLocation-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-KbLocation-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-KbLocation-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-BrowseKb.
          05 WS-Btn-BrowseKb-TEXT       PIC X(256) VALUE '...'.
          05 WS-Btn-BrowseKb-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-BrowseKb-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-CancelTop.
          05 WS-Btn-CancelTop-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-CancelTop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-CancelTop-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-SaveTop.
          05 WS-Btn-SaveTop-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-SaveTop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-SaveTop-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "KB-FOLDER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "KB-FOLDER-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-BrowseKb"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-BROWSEKB--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-CancelTop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCELTOP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-SaveTop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVETOP--ONCLICK"
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

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-FOLDER-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-PATHS: sets the paths of the two indexed files PowerChat reads here.
           *> Uses:   The environment variable POWERCHAT_DATA.
           *> Result: WS-DATA-DIR = $POWERCHAT_DATA, or "data" when that is empty; WS-SETTINGS-PATH
           *>         = <WS-DATA-DIR>/settings.idx and WS-MODELS-PATH = <WS-DATA-DIR>/models.idx. No
           *>         file is opened.

           CALL    "PC-PATHS"

           *> PC-TEXTS: loads the form's texts in the language the user picked and sets every
           *> designed caption and tooltip that shows one.
           *> Uses:   The LANG setting in the settings file whose path PC-PATHS just set
           *>         (WS-SETTINGS-PATH), and PC-TEXT (9 texts x 6 languages en pt es fr jp cn).
           *> Result: WS-LANG = the saved code ("en" when unset or unknown), WS-LANG-IX = its column
           *>         1-6, PC-TEXT-NOW(1..9) (the T-... names) = that column; the title, label, Save
           *>         / Cancel captions of both button rows, Lbl-Status::Caption = T-STATUS and the
           *>         two tooltips are set in that language. WS-SET-NAME = "LANG" and WS-SET-VALUE
           *>         hold the lookup. WS-TX-I and WS-FS are overwritten.

           CALL    "PC-TEXTS"

           SET     WS-SET-NAME                   TO    "KB-LOCATION"

           *> PC-SETTING-GET: reads one setting from SETTINGS-FILE (indexed,
           *> <WS-DATA-DIR>/settings.idx) by its name; it never writes a setting (a missing file is
           *> only created, empty).
           *> Uses:   WS-SET-NAME = "KB-LOCATION" (set just above), the record key.
           *> Result: WS-SET-VALUE = the Knowledge Base folder saved before, or SPACES; the next
           *>         lines fall back to "assets/KB" and show it in Txt-KbLocation::Text.
           *>         WS-SET-NAME is unchanged; WS-FS and the record area SETTINGS-REC are
           *>         overwritten.

           CALL    "PC-SETTING-GET"
           IF    WS-SET-VALUE = SPACES
               SET     WS-SET-VALUE              TO    "assets/KB"
           END-IF
           SET     Txt-KbLocation::Text          TO    WS-SET-VALUE

           GOBACK.

       END PROGRAM KB-FOLDER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KB-FOLDER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM KB-FOLDER-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-BROWSEKB--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The operating system's own folder window (no typing, no typos).
           *>   The choice waits in the field until Save.

           SET     WS-SET-VALUE                  TO    Txt-KbLocation::Text
           COBOL::"FOLDER-DIALOG" ( T-KB-BROWSE-TITLE WS-SET-VALUE WS-PATH )
           IF    WS-PATH NOT = SPACES
               SET     Txt-KbLocation::Text      TO    WS-PATH
           END-IF

           GOBACK.

       END PROGRAM BTN-BROWSEKB--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The same code as the Cancel button at the top.

           *> PC-CANCEL: closes the dialog without saving anything (here: the Cancel button at the
           *> bottom was pressed).
           *> Uses:   No data item is read.
           *> Result: The dialog closes (ME::Close()); the settings file is not touched. No data
           *>         item changes.

           CALL    "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The same code as the Save button at the top.

           *> PC-SAVE: saves the Knowledge Base folder typed in Txt-KbLocation as the KB-LOCATION
           *> setting and closes the dialog.
           *> Uses:   Txt-KbLocation::Text, the folder the user typed (or left as loaded).
           *> Result: SETTINGS-FILE holds KB-LOCATION = the typed folder, or "assets/KB" when the
           *>         field was blank (WS-SET-NAME and WS-SET-VALUE carry it to PC-SETTING-PUT);
           *>         Lbl-Status::Caption = T-KB-FOLDER-SAVED; the dialog closes.

           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCELTOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-CANCEL: closes the dialog without saving anything (here: the Cancel button at the
           *> top was pressed).
           *> Uses:   No data item is read.
           *> Result: The dialog closes (ME::Close()); the settings file is not touched. No data
           *>         item changes.

           CALL    "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCELTOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVETOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> PC-SAVE: saves the Knowledge Base folder typed in Txt-KbLocation as the KB-LOCATION
           *> setting and closes the dialog.
           *> Uses:   Txt-KbLocation::Text, the folder the user typed (or left as loaded).
           *> Result: SETTINGS-FILE holds KB-LOCATION = the typed folder, or "assets/KB" when the
           *>         field was blank (WS-SET-NAME and WS-SET-VALUE carry it to PC-SETTING-PUT);
           *>         Lbl-Status::Caption = T-KB-FOLDER-SAVED; the dialog closes.

           CALL    "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVETOP--ONCLICK.

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
           SET     WS-MODELS-PATH                TO    SPACES
           STRING    WS-DATA-DIR::Trim() "/models.idx" INTO WS-MODELS-PATH

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
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The language the user picked (LANG), the texts in it, and every
           *>   designed caption and hint that shows one (R46).

           SET     WS-SET-NAME                   TO    "LANG"

           *> PC-SETTING-GET: reads one setting from SETTINGS-FILE (indexed,
           *> <WS-DATA-DIR>/settings.idx) by its name; it never writes a setting (a missing file is
           *> only created, empty).
           *> Uses:   WS-SET-NAME = "LANG" (set just above), the record key.
           *> Result: WS-SET-VALUE = the saved language code (PC-TEXTS reads its first two
           *>         characters), or SPACES when it was never saved. WS-SET-NAME is unchanged;
           *>         WS-FS and the record area SETTINGS-REC are overwritten.

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

           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 9
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Lbl-Title::Caption            TO    T-KB-FOLDER-TITLE::Trim()
           SET     Lbl-KB::Caption               TO    T-KB-FOLDER-LABEL::Trim()
           SET     Btn-Save::Caption             TO    T-SAVE::Trim()
           SET     Btn-Cancel::Caption           TO    T-CANCEL::Trim()
           SET     Btn-SaveTop::Caption          TO    T-SAVE::Trim()
           SET     Btn-CancelTop::Caption        TO    T-CANCEL::Trim()
           SET     Lbl-Status::Caption           TO    T-STATUS::Trim()
           SET     Txt-KbLocation::Tooltip       TO    T-TIP-KB::Trim()
           SET     Btn-BrowseKb::Tooltip         TO    T-TIP-BROWSE::Trim()

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
       PROGRAM-ID. PC-SAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Save - shared by the Save buttons at the top and at the bottom.

           SET     WS-SET-NAME                   TO    "KB-LOCATION"
           SET     WS-SET-VALUE                  TO    Txt-KbLocation::Text

           IF    WS-SET-VALUE = SPACES
               SET     WS-SET-VALUE              TO    "assets/KB"
           END-IF

           *> PC-SETTING-PUT: writes one setting to SETTINGS-FILE (indexed,
           *> <WS-DATA-DIR>/settings.idx): the record is added, or its value replaced when the name
           *> is already there, and committed at once.
           *> Uses:   WS-SET-NAME = "KB-LOCATION" and WS-SET-VALUE = Txt-KbLocation::Text, the
           *>         folder the user typed, or "assets/KB" when it was blank (both set just above).
           *> Result: The record KB-LOCATION of SETTINGS-FILE = WS-SET-VALUE: written, or rewritten
           *>         when it existed, and committed (the file is created first when missing).
           *>         DOCUMENTS-FORM reads it back as the Knowledge Base folder. WS-SET-NAME and
           *>         WS-SET-VALUE keep their values; WS-FS and the record area SETTINGS-REC are
           *>         overwritten.

           CALL    "PC-SETTING-PUT"
           SET     Lbl-Status::Caption           TO    T-KB-FOLDER-SAVED::Trim()
           INVOKE    ME::Close()

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-CANCEL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   Cancel - shared by the Cancel buttons at the top and at the bottom.
           *>   Nothing is written: the dialog closes as it was opened.

           INVOKE    ME::Close()

           GOBACK.

       END PROGRAM PC-CANCEL.

       END PROGRAM KB-FOLDER-FORM.

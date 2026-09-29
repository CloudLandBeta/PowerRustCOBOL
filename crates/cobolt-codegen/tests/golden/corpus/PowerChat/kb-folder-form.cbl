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
       FD  SETTINGS-FILE IS GLOBAL.
       01  SETTINGS-REC.
           05 SET-NAME          PIC X(20).
           05 SET-VALUE         PIC X(200).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'KB-FOLDER-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS              GLOBAL PIC XX VALUE "00".
       01 WS-DATA-DIR        GLOBAL PIC X(200).
       01 WS-SET-NAME        GLOBAL PIC X(20).
       01 WS-SET-VALUE       GLOBAL PIC X(200).
       01 WS-EOF             GLOBAL PIC X.
       01 WS-OK              GLOBAL PIC X(4).
       01 WS-SETTINGS-PATH   GLOBAL PIC X(240).
       01 WS-MODELS-PATH     GLOBAL PIC X(240).
      *>   The interface texts (spec 071 R44): one row per text, one column
      *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
      *>   only the values are translated.
       01 WS-LANG            GLOBAL PIC XX VALUE "en".
       01 WS-LANG-IX         GLOBAL PIC 9 VALUE 1.
       01 WS-TX-I            GLOBAL PIC 9(4).
       01 PC-TEXT-DATA       GLOBAL.
      *>   KB-FOLDER-TITLE
          05 FILLER PIC X(120) VALUE "Knowledge Base folder".
          05 FILLER PIC X(120) VALUE "Pasta da base de conhecimento".
          05 FILLER PIC X(120) VALUE "Carpeta de la base de conocimiento".
          05 FILLER PIC X(120) VALUE "Dossier de la base de connaissances".
          05 FILLER PIC X(120) VALUE "ナレッジベースのフォルダー".
          05 FILLER PIC X(120) VALUE "知识库文件夹".
      *>   KB-FOLDER-LABEL
          05 FILLER PIC X(120) VALUE "Knowledge Base folder (may be a folder on another machine)".
          05 FILLER PIC X(120) VALUE "Pasta da base de conhecimento (pode ser uma pasta em outra máquina)".
          05 FILLER PIC X(120) VALUE "Carpeta de la base de conocimiento (puede estar en otra máquina)".
          05 FILLER PIC X(120) VALUE "Dossier de la base de connaissances (il peut se trouver sur une autre machine)".
          05 FILLER PIC X(120) VALUE "ナレッジベースのフォルダー（別のマシン上のフォルダーでも可）".
          05 FILLER PIC X(120) VALUE "知识库文件夹（可以位于另一台机器上）".
      *>   KB-BROWSE-TITLE
          05 FILLER PIC X(120) VALUE "Choose the Knowledge Base folder".
          05 FILLER PIC X(120) VALUE "Escolha a pasta da base de conhecimento".
          05 FILLER PIC X(120) VALUE "Elige la carpeta de la base de conocimiento".
          05 FILLER PIC X(120) VALUE "Choisissez le dossier de la base de connaissances".
          05 FILLER PIC X(120) VALUE "ナレッジベースのフォルダーを選択".
          05 FILLER PIC X(120) VALUE "选择知识库文件夹".
      *>   KB-FOLDER-SAVED
          05 FILLER PIC X(120) VALUE "Knowledge Base folder saved.".
          05 FILLER PIC X(120) VALUE "Pasta da base de conhecimento salva.".
          05 FILLER PIC X(120) VALUE "Carpeta de la base de conocimiento guardada.".
          05 FILLER PIC X(120) VALUE "Dossier de la base de connaissances enregistré.".
          05 FILLER PIC X(120) VALUE "ナレッジベースのフォルダーを保存しました。".
          05 FILLER PIC X(120) VALUE "知识库文件夹已保存。".
      *>   SAVE
          05 FILLER PIC X(120) VALUE "Save".
          05 FILLER PIC X(120) VALUE "Salvar".
          05 FILLER PIC X(120) VALUE "Guardar".
          05 FILLER PIC X(120) VALUE "Enregistrer".
          05 FILLER PIC X(120) VALUE "保存".
          05 FILLER PIC X(120) VALUE "保存".
      *>   CANCEL
          05 FILLER PIC X(120) VALUE "Cancel".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Annuler".
          05 FILLER PIC X(120) VALUE "キャンセル".
          05 FILLER PIC X(120) VALUE "取消".
      *>   STATUS
          05 FILLER PIC X(120) VALUE "Status".
          05 FILLER PIC X(120) VALUE "Status".
          05 FILLER PIC X(120) VALUE "Estado".
          05 FILLER PIC X(120) VALUE "État".
          05 FILLER PIC X(120) VALUE "ステータス".
          05 FILLER PIC X(120) VALUE "状态".
      *>   TIP-KB
          05 FILLER PIC X(120) VALUE "The folder where each topic keeps its documents and search index. It may be on another machine.".
          05 FILLER PIC X(120) VALUE "Pasta onde cada tópico guarda seus documentos e o índice de busca. Pode ficar em outra máquina.".
          05 FILLER PIC X(120) VALUE "Carpeta donde cada tema guarda sus documentos y su índice. Puede estar en otra máquina.".
          05 FILLER PIC X(120) VALUE "Dossier où chaque sujet garde ses documents et son index. Il peut être sur une autre machine.".
          05 FILLER PIC X(120) VALUE "各トピックの文書と検索索引を置くフォルダー。別のマシンにあってもかまいません。".
          05 FILLER PIC X(120) VALUE "每个主题存放文档和搜索索引的文件夹，可以位于另一台机器上。".
      *>   TIP-BROWSE
          05 FILLER PIC X(120) VALUE "Choose the folder in a window instead of typing it.".
          05 FILLER PIC X(120) VALUE "Escolher a pasta numa janela em vez de digitá-la.".
          05 FILLER PIC X(120) VALUE "Elegir la carpeta en una ventana en lugar de escribirla.".
          05 FILLER PIC X(120) VALUE "Choisir le dossier dans une fenêtre plutôt que le taper.".
          05 FILLER PIC X(120) VALUE "入力する代わりにウィンドウでフォルダーを選びます。".
          05 FILLER PIC X(120) VALUE "在窗口中选择文件夹，而不必手动输入。".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
          05 PC-TEXT-ROW     OCCURS 9.
             10 PC-TEXT      PIC X(120) OCCURS 6.
      *>   The texts in the current language, by name.
       01 PC-TEXTS-NOW       GLOBAL.
          05 T-KB-FOLDER-TITLE PIC X(120).
          05 T-KB-FOLDER-LABEL PIC X(120).
          05 T-KB-BROWSE-TITLE PIC X(120).
          05 T-KB-FOLDER-SAVED PIC X(120).
          05 T-SAVE PIC X(120).
          05 T-CANCEL PIC X(120).
          05 T-STATUS PIC X(120).
          05 T-TIP-KB PIC X(120).
          05 T-TIP-BROWSE PIC X(120).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
          05 PC-TEXT-NOW     PIC X(120) OCCURS 9.
      *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.
       01 WS-FMT             GLOBAL PIC X(120).
       01 WS-ARG1            GLOBAL PIC X(300).
       01 WS-ARG2            GLOBAL PIC X(300).
       01 WS-ARG3            GLOBAL PIC X(300).
       01 WS-ARG4            GLOBAL PIC X(300).
       01 WS-FMT-OUT         GLOBAL PIC X(1200).
       01 WS-PATH            GLOBAL PIC X(400).

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
           CALL "PC-PATHS"
           CALL "PC-TEXTS"
           MOVE "KB-LOCATION" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           IF WS-SET-VALUE = SPACES
               MOVE "assets/KB" TO WS-SET-VALUE
           END-IF
           MOVE WS-SET-VALUE TO Txt-KbLocation::Text

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
           MOVE Txt-KbLocation::Text TO WS-SET-VALUE
           COBOL::"FOLDER-DIALOG" ( T-KB-BROWSE-TITLE WS-SET-VALUE WS-PATH )
           IF WS-PATH NOT = SPACES
               MOVE WS-PATH TO Txt-KbLocation::Text
           END-IF

           GOBACK.

       END PROGRAM BTN-BROWSEKB--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The same code as the Cancel button at the top.
           CALL "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The same code as the Save button at the top.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCELTOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCELTOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVETOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVETOP--ONCLICK.

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
           MOVE SPACES TO WS-MODELS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/models.idx"
               DELIMITED BY SIZE INTO WS-MODELS-PATH

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
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The language the user picked (LANG), the texts in it, and every
      *>   designed caption and hint that shows one (R46).
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
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 9
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-KB-FOLDER-TITLE) TO Lbl-Title::Caption
           MOVE FUNCTION TRIM(T-KB-FOLDER-LABEL) TO Lbl-KB::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-SaveTop::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-CancelTop::Caption
           MOVE FUNCTION TRIM(T-STATUS) TO Lbl-Status::Caption
           MOVE FUNCTION TRIM(T-TIP-KB) TO Txt-KbLocation::Tooltip
           MOVE FUNCTION TRIM(T-TIP-BROWSE) TO Btn-BrowseKb::Tooltip

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
       01 WS-PART            PIC X(120).
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
       PROGRAM-ID. PC-SAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Save - shared by the Save buttons at the top and at the bottom.
           MOVE "KB-LOCATION" TO WS-SET-NAME
           MOVE Txt-KbLocation::Text TO WS-SET-VALUE
           IF WS-SET-VALUE = SPACES
               MOVE "assets/KB" TO WS-SET-VALUE
           END-IF
           CALL "PC-SETTING-PUT"
           MOVE FUNCTION TRIM(T-KB-FOLDER-SAVED) TO Lbl-Status::Caption
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-CANCEL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Cancel - shared by the Cancel buttons at the top and at the bottom.
      *>   Nothing is written: the dialog closes as it was opened.
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM PC-CANCEL.

       END PROGRAM KB-FOLDER-FORM.

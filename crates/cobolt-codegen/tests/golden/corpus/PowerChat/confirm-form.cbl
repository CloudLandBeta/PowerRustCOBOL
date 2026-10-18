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
       PROGRAM-ID. CONFIRM-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'CONFIRM-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS                                  GLOBAL    PIC XX    VALUE "00".
       01 WS-DATA-DIR                            GLOBAL    PIC X(200).
       01 WS-SET-NAME                            GLOBAL    PIC X(20).
       01 WS-SET-VALUE                           GLOBAL    PIC X(200).
       01 WS-SETTINGS-PATH                       GLOBAL    PIC X(240).
       *>   The interface texts (spec 071 R44): one row per text, one column
       *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
       *>   only the values are translated. A caller's own text wins.

       01 WS-LANG                                GLOBAL    PIC XX    VALUE "en".
       01 WS-LANG-IX                             GLOBAL    PIC 9    VALUE 1.
       01 WS-TX-I                                GLOBAL    PIC 9(4).
       01 PC-TEXT-DATA                           GLOBAL.
           *>   ARE-YOU-SURE

           05 FILLER                             PIC    X(120)    VALUE "Are you sure?".
           05 FILLER                             PIC    X(120)    VALUE "Tem certeza?".
           05 FILLER                             PIC    X(120)    VALUE "¿Está seguro?".
           05 FILLER                             PIC    X(120)    VALUE "Êtes-vous sûr ?".
           05 FILLER                             PIC    X(120)    VALUE "よろしいですか?".
           05 FILLER                             PIC    X(120)    VALUE "确定吗?".
           *>   YES

           05 FILLER                             PIC    X(120)    VALUE "Yes".
           05 FILLER                             PIC    X(120)    VALUE "Sim".
           05 FILLER                             PIC    X(120)    VALUE "Sí".
           05 FILLER                             PIC    X(120)    VALUE "Oui".
           05 FILLER                             PIC    X(120)    VALUE "はい".
           05 FILLER                             PIC    X(120)    VALUE "是".
           *>   NO

           05 FILLER                             PIC    X(120)    VALUE "No".
           05 FILLER                             PIC    X(120)    VALUE "Não".
           05 FILLER                             PIC    X(120)    VALUE "No".
           05 FILLER                             PIC    X(120)    VALUE "Non".
           05 FILLER                             PIC    X(120)    VALUE "いいえ".
           05 FILLER                             PIC    X(120)    VALUE "否".
       01 PC-TEXT-TABLE                          REDEFINES    PC-TEXT-DATA    GLOBAL.
           05 PC-TEXT-ROW                        OCCURS    3.
               10 PC-TEXT                        PIC    X(120)    OCCURS 6.
       *>   The texts in the current language, by name.

       01 PC-TEXTS-NOW                           GLOBAL.
           05 T-ARE-YOU-SURE                     PIC    X(120).
           05 T-YES                              PIC    X(120).
           05 T-NO                               PIC    X(120).
       01 PC-TEXTS-NOW-R                         REDEFINES    PC-TEXTS-NOW    GLOBAL.
           05 PC-TEXT-NOW                        PIC    X(120)    OCCURS 3.
       01 WS-TEXT                                GLOBAL    PIC X(300).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Question.
          05 WS-Lbl-Question-TEXT       PIC X(256) VALUE 'Are you sure?'.
          05 WS-Lbl-Question-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Question-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-No.
          05 WS-Btn-No-TEXT       PIC X(256) VALUE 'No'.
          05 WS-Btn-No-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-No-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Yes.
          05 WS-Btn-Yes-TEXT       PIC X(256) VALUE 'Yes'.
          05 WS-Btn-Yes-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Yes-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "CONFIRM-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CONFIRM-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-No"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NO--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Yes"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-YES--ONCLICK"
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
       PROGRAM-ID. CONFIRM-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   A yes/no question for any form (operator, 2026-09-26): the
           *>   caller sets ConfirmText (and, translated, ConfirmYes /
           *>   ConfirmNo) on itself, opens this with OpenFormSync, and reads
           *>   ConfirmAnswer - Y or N - when it returns. Closing it any other
           *>   way leaves the caller's own default, which should be N.
           *>   The designed texts in the user's language first; what the
           *>   caller hands over below replaces them.

           *> PC-PATHS: sets the path of the settings file.
           *> Uses:   The environment variable POWERCHAT_DATA.
           *> Result: WS-DATA-DIR = $POWERCHAT_DATA, or "data" when that is empty; WS-SETTINGS-PATH
           *>         = <WS-DATA-DIR>/settings.idx. No file is opened.

           CALL    "PC-PATHS"

           *> PC-TEXTS: loads the form's texts in the language the user picked and sets the three
           *> captions that show them.
           *> Uses:   The LANG setting in the settings file whose path PC-PATHS just set
           *>         (WS-SETTINGS-PATH), and PC-TEXT (3 texts x 6 languages en pt es fr jp cn).
           *> Result: WS-LANG = the saved code ("en" when unset or unknown), WS-LANG-IX = its column
           *>         1-6, PC-TEXT-NOW(1..3) = T-ARE-YOU-SURE, T-YES and T-NO in that language;
           *>         Lbl-Question::Caption, Btn-Yes::Caption and Btn-No::Caption show them; a text
           *>         the caller hands over replaces its caption below. WS-SET-NAME = "LANG" and
           *>         WS-SET-VALUE hold the lookup. WS-TX-I and WS-FS are overwritten.

           CALL    "PC-TEXTS"

           INVOKE    super::GetProperty("ConfirmText") RETURNING WS-TEXT
           *>   No question given: the translated "Are you sure?" stays.

           IF    WS-TEXT NOT = SPACES
               SET     Lbl-Question::Caption     TO    WS-TEXT::Trim()
           END-IF
           INVOKE    super::GetProperty("ConfirmYes") RETURNING WS-TEXT
           IF    WS-TEXT NOT = SPACES
               SET     Btn-Yes::Caption          TO    WS-TEXT::Trim()
           END-IF
           INVOKE    super::GetProperty("ConfirmNo") RETURNING WS-TEXT
           IF    WS-TEXT NOT = SPACES
               SET     Btn-No::Caption           TO    WS-TEXT::Trim()
           END-IF

           GOBACK.

       END PROGRAM CONFIRM-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CONFIRM-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CONFIRM-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The caller asked; this is its answer, read after OpenFormSync.

           INVOKE    super::SetProperty("ConfirmAnswer", "N")
           INVOKE    ME::Close()

           GOBACK.

       END PROGRAM BTN-NO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-YES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           INVOKE    super::SetProperty("ConfirmAnswer", "Y")
           INVOKE    ME::Close()

           GOBACK.

       END PROGRAM BTN-YES--ONCLICK.

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
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.
           *>   The language the user picked (LANG), the texts in it, and every
           *>   designed caption that shows one (R46).

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
           PERFORM    VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 3
               SET     PC-TEXT-NOW(WS-TX-I)      TO    PC-TEXT(WS-TX-I, WS-LANG-IX)
           END-PERFORM

           SET     Lbl-Question::Caption         TO    T-ARE-YOU-SURE::Trim()
           SET     Btn-Yes::Caption              TO    T-YES::Trim()
           SET     Btn-No::Caption               TO    T-NO::Trim()

           GOBACK.

       END PROGRAM PC-TEXTS.

       END PROGRAM CONFIRM-FORM.

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
       PROGRAM-ID. PICK-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PICK-FORM'.

      *>── User Working Storage ────────────────────────────────────────
01 WS-FS              GLOBAL PIC XX VALUE "00".
       01 WS-DATA-DIR        GLOBAL PIC X(200).
       01 WS-SET-NAME        GLOBAL PIC X(20).
       01 WS-SET-VALUE       GLOBAL PIC X(200).
       01 WS-SETTINGS-PATH   GLOBAL PIC X(240).
      *>   The interface texts (spec 071 R44): one row per text, one column
      *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
      *>   only the values are translated. A caller's own text wins.
       01 WS-LANG            GLOBAL PIC XX VALUE "en".
       01 WS-LANG-IX         GLOBAL PIC 9 VALUE 1.
       01 WS-TX-I            GLOBAL PIC 9(4).
       01 PC-TEXT-DATA       GLOBAL.
      *>   CHOOSE-ONE
          05 FILLER PIC X(120) VALUE "Choose one".
          05 FILLER PIC X(120) VALUE "Escolha um".
          05 FILLER PIC X(120) VALUE "Elija uno".
          05 FILLER PIC X(120) VALUE "Choisissez-en un".
          05 FILLER PIC X(120) VALUE "1 つ選んでください".
          05 FILLER PIC X(120) VALUE "请选择一项".
      *>   OK
          05 FILLER PIC X(120) VALUE "OK".
          05 FILLER PIC X(120) VALUE "OK".
          05 FILLER PIC X(120) VALUE "Aceptar".
          05 FILLER PIC X(120) VALUE "OK".
          05 FILLER PIC X(120) VALUE "OK".
          05 FILLER PIC X(120) VALUE "确定".
      *>   CANCEL
          05 FILLER PIC X(120) VALUE "Cancel".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Annuler".
          05 FILLER PIC X(120) VALUE "キャンセル".
          05 FILLER PIC X(120) VALUE "取消".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
          05 PC-TEXT-ROW     OCCURS 3.
             10 PC-TEXT      PIC X(120) OCCURS 6.
      *>   The texts in the current language, by name.
       01 PC-TEXTS-NOW       GLOBAL.
          05 T-CHOOSE-ONE PIC X(120).
          05 T-OK PIC X(120).
          05 T-CANCEL PIC X(120).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
          05 PC-TEXT-NOW     PIC X(120) OCCURS 3.
       01 WS-TEXT            GLOBAL PIC X(300).
       01 WS-ITEMS           GLOBAL PIC X(8400).
       01 WS-ITEM            GLOBAL PIC X(120).
       01 WS-PTR             GLOBAL PIC 9(5).
       01 WS-OK              GLOBAL PIC X(4).
       01 WS-IDX             GLOBAL PIC S9(4).
       01 WS-ANSWER          GLOBAL PIC 9(4).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Question.
          05 WS-Lbl-Question-TEXT       PIC X(256) VALUE 'Choose one'.
          05 WS-Lbl-Question-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Question-ENABLED    PIC 9      VALUE 1.

       01 WS-Lst-Items.
          05 WS-Lst-Items-TEXT       PIC X(256) VALUE 'Lst-Items'.
          05 WS-Lst-Items-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Items-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Items-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Ok.
          05 WS-Btn-Ok-TEXT       PIC X(256) VALUE 'OK'.
          05 WS-Btn-Ok-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Ok-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "PICK-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PICK-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Ok"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-OK--ONCLICK"
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
       PROGRAM-ID. PICK-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Choose one of a list, for any form (operator, 2026-09-27):
      *>   the caller sets PickItems - one item per line - and, translated,
      *>   PickTitle / PickOk / PickCancel on itself, PickSelected (the
      *>   item preselected, 1 = the first) and PickAnswer = 0; opens this
      *>   with OpenFormSync; and reads PickAnswer when it returns: the
      *>   chosen item, 1 = the first, or 0 when nothing was chosen.
      *>   The designed texts in the user's language first; what the
      *>   caller hands over below replaces them.
           CALL "PC-PATHS"
           CALL "PC-TEXTS"
           INVOKE super::"GetProperty"("PickTitle") RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               MOVE FUNCTION TRIM(WS-TEXT) TO Lbl-Question::Caption
           END-IF
           INVOKE super::"GetProperty"("PickOk") RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               MOVE FUNCTION TRIM(WS-TEXT) TO Btn-Ok::Caption
           END-IF
           INVOKE super::"GetProperty"("PickCancel") RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               MOVE FUNCTION TRIM(WS-TEXT) TO Btn-Cancel::Caption
           END-IF
           INVOKE super::"GetProperty"("PickItems") RETURNING WS-ITEMS
           MOVE Lst-Items::Clear() TO WS-OK
           MOVE 1 TO WS-PTR
           PERFORM UNTIL WS-PTR > 8400
               MOVE SPACES TO WS-ITEM
               UNSTRING WS-ITEMS DELIMITED BY X"0A"
                   INTO WS-ITEM WITH POINTER WS-PTR
               END-UNSTRING
               IF WS-ITEM = SPACES
                   EXIT PERFORM
               END-IF
               MOVE Lst-Items::AddItem(FUNCTION TRIM(WS-ITEM)) TO WS-OK
           END-PERFORM
           INVOKE super::"GetProperty"("PickSelected") RETURNING WS-TEXT
           IF WS-TEXT = SPACES
               MOVE 1 TO WS-IDX
           ELSE
               MOVE FUNCTION NUMVAL(WS-TEXT) TO WS-IDX
           END-IF
           MOVE WS-IDX TO Lst-Items::SelectedIndex

           GOBACK.

       END PROGRAM PICK-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PICK-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PICK-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   No choice: PickAnswer keeps the caller's own 0.
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-OK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The chosen item, 1 = the first; nothing chosen, nothing to do.
           MOVE Lst-Items::SelectedIndex TO WS-IDX
           IF WS-IDX < 1
               EXIT PROGRAM
           END-IF
           MOVE WS-IDX TO WS-ANSWER
           INVOKE super::"SetProperty"("PickAnswer", WS-ANSWER)
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM BTN-OK--ONCLICK.

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
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The language the user picked (LANG), the texts in it, and every
      *>   designed caption that shows one (R46).
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
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 3
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-CHOOSE-ONE) TO Lbl-Question::Caption
           MOVE FUNCTION TRIM(T-OK) TO Btn-Ok::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel::Caption

           GOBACK.

       END PROGRAM PC-TEXTS.

       END PROGRAM PICK-FORM.

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

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'CONFIRM-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-TEXT   PIC X(300) GLOBAL.
       *>   The interface language, as the form that opened this one has it (property UiLanguage):
       *>   1 en, 2 pt, 3 es, 4 fr, 5 jp, 6 cn. The texts: one row each, one column per language.

       01 WS-LANG-IX PIC 9 GLOBAL VALUE 1.
       01 PC-TEXT-DATA GLOBAL.
           *>   YES
           05 FILLER PIC X(60) VALUE "Yes".
           05 FILLER PIC X(60) VALUE "Sim".
           05 FILLER PIC X(60) VALUE "Sí".
           05 FILLER PIC X(60) VALUE "Oui".
           05 FILLER PIC X(60) VALUE "はい".
           05 FILLER PIC X(60) VALUE "是".
           *>   NO
           05 FILLER PIC X(60) VALUE "No".
           05 FILLER PIC X(60) VALUE "Não".
           05 FILLER PIC X(60) VALUE "No".
           05 FILLER PIC X(60) VALUE "Non".
           05 FILLER PIC X(60) VALUE "いいえ".
           05 FILLER PIC X(60) VALUE "否".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
           05 PC-TEXT-ROW OCCURS 2.
               10 PC-TEXT PIC X(60) OCCURS 6.
       01 M-YES         PIC 99 GLOBAL VALUE 1.
       01 M-NO          PIC 99 GLOBAL VALUE 2.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Question.
          05 WS-Lbl-Question-TEXT       PIC X(256) VALUE 'Are you sure?'.
          05 WS-Lbl-Question-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Question-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Yes.
          05 WS-Btn-Yes-TEXT       PIC X(256) VALUE 'Yes'.
          05 WS-Btn-Yes-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Yes-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-No.
          05 WS-Btn-No-TEXT       PIC X(256) VALUE 'No'.
          05 WS-Btn-No-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-No-ENABLED    PIC 9      VALUE 1.

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
                   WHEN "Btn-Yes"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-YES--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-No"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NO--ONCLICK"
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

           *> The language of the window that opened this one, and the designed texts in it.

           INVOKE    super::GetProperty( "UiLanguage" ) RETURNING WS-TEXT
           SET     WS-LANG-IX                    TO    FUNCTION NUMVAL( WS-TEXT )
           IF WS-LANG-IX < 1 OR WS-LANG-IX > 6
               SET     WS-LANG-IX                    TO    1
           END-IF
           SET     BTN-YES::Caption              TO    PC-TEXT( M-YES, WS-LANG-IX )::Trim()
           SET     BTN-NO::Caption               TO    PC-TEXT( M-NO, WS-LANG-IX )::Trim()

           INVOKE    super::GetProperty( "ConfirmText" ) RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               SET     LBL-QUESTION::Caption         TO    WS-TEXT::Trim()
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
       PROGRAM-ID. BTN-YES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    super::SetProperty( "ConfirmAnswer", "Y" )
           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-YES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    super::SetProperty( "ConfirmAnswer", "N" )
           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-NO--ONCLICK.

       END PROGRAM CONFIRM-FORM.

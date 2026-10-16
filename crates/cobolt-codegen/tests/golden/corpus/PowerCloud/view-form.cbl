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
       PROGRAM-ID. VIEW-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'VIEW-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-TEXT   PIC X(300) GLOBAL.
       *>   The interface language, as the form that opened this one has it (property UiLanguage):
       *>   1 en, 2 pt, 3 es, 4 fr, 5 jp, 6 cn. The texts: one row each, one column per language.

       01 WS-LANG-IX PIC 9 GLOBAL VALUE 1.
       01 PC-TEXT-DATA GLOBAL.
           *>   CLOSE
           05 FILLER PIC X(60) VALUE "Close".
           05 FILLER PIC X(60) VALUE "Fechar".
           05 FILLER PIC X(60) VALUE "Cerrar".
           05 FILLER PIC X(60) VALUE "Fermer".
           05 FILLER PIC X(60) VALUE "閉じる".
           05 FILLER PIC X(60) VALUE "关闭".
           *>   AGE-OF
           05 FILLER PIC X(60) VALUE "Age &1".
           05 FILLER PIC X(60) VALUE "Idade &1".
           05 FILLER PIC X(60) VALUE "Edad &1".
           05 FILLER PIC X(60) VALUE "Âge &1".
           05 FILLER PIC X(60) VALUE "年齢 &1".
           05 FILLER PIC X(60) VALUE "年龄 &1".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
           05 PC-TEXT-ROW OCCURS 2.
               10 PC-TEXT PIC X(60) OCCURS 6.
       01 M-CLOSE       PIC 99 GLOBAL VALUE 1.
       01 M-AGE-OF      PIC 99 GLOBAL VALUE 2.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Name.
          05 WS-Lbl-Name-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Name-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Race.
          05 WS-Lbl-Race-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Race-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Race-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Age.
          05 WS-Lbl-Age-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Age-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Age-ENABLED    PIC 9      VALUE 1.

       01 WS-Pic-Photo.
          05 WS-Pic-Photo-TEXT       PIC X(256) VALUE 'Pic-Photo'.
          05 WS-Pic-Photo-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pic-Photo-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Close.
          05 WS-Btn-Close-TEXT       PIC X(256) VALUE 'Close'.
          05 WS-Btn-Close-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Close-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "VIEW-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "VIEW-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Close"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLOSE--ONCLICK"
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
       PROGRAM-ID. VIEW-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The language of the window that opened this one, and the designed texts in it.

           INVOKE    super::GetProperty( "UiLanguage" ) RETURNING WS-TEXT
           SET     WS-LANG-IX                    TO    FUNCTION NUMVAL( WS-TEXT )
           IF WS-LANG-IX < 1 OR WS-LANG-IX > 6
               SET     WS-LANG-IX                    TO    1
           END-IF
           SET     BTN-CLOSE::Caption            TO    PC-TEXT( M-CLOSE, WS-LANG-IX )::Trim()

           INVOKE    super::GetProperty( "PetName" ) RETURNING WS-TEXT
           SET     LBL-NAME::Caption             TO    WS-TEXT::Trim()
           INVOKE    super::GetProperty( "PetRace" ) RETURNING WS-TEXT
           SET     LBL-RACE::Caption             TO    WS-TEXT::Trim()
           INVOKE    super::GetProperty( "PetAge" ) RETURNING WS-TEXT
           SET     LBL-AGE::Caption              TO    PC-TEXT( M-AGE-OF, WS-LANG-IX )::Trim()::Replace( "&1", WS-TEXT::Trim() )
           INVOKE    super::GetProperty( "ViewPhoto" ) RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               SET     PIC-PHOTO::ImagePath          TO    WS-TEXT::Trim()
           END-IF

           GOBACK.

       END PROGRAM VIEW-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VIEW-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM VIEW-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-CLOSE--ONCLICK.

       END PROGRAM VIEW-FORM.

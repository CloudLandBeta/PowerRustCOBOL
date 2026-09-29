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
       PROGRAM-ID. CALL-FORM-DEMO.

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
       01 FORM-NAME               PIC X(64)   VALUE 'CALL-FORM-DEMO'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Call Form Demo (Sync)'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-2.
          05 WS-Label-2-TEXT       PIC X(256) VALUE 'Click the button to open CALLED-FORM-DEMO as a modal window. Pick OK or Cancel there — the called form closes itself and sends back the text it chose; this form picks it up and shows it below.'.
          05 WS-Label-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-1.
          05 WS-Button-1-TEXT       PIC X(256) VALUE 'Open Called Form'.
          05 WS-Button-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-3.
          05 WS-Label-3-TEXT       PIC X(256) VALUE 'Result from the called form:'.
          05 WS-Label-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Result.
          05 WS-Lbl-Result-TEXT       PIC X(256) VALUE '(not called yet)'.
          05 WS-Lbl-Result-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Result-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "CALL-FORM-DEMO--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CALL-FORM-DEMO--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Button-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-1--ONCLICK"
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
       PROGRAM-ID. CALL-FORM-DEMO--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CALL-FORM-DEMO--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CALL-FORM-DEMO--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CALL-FORM-DEMO--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.

           *> Call the form in modal (Sync)

           INVOKE ME::"OpenFormSync"("CALLED-FORM-DEMO")

           *> Retrieve data set by the modal window

           INVOKE ME::"GetProperty"("ResultText") RETURNING Lbl-Result::Caption.

           GOBACK.

       END PROGRAM BUTTON-1--ONCLICK.

       END PROGRAM CALL-FORM-DEMO.

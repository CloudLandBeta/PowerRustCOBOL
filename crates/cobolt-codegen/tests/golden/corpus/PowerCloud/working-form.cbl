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
       PROGRAM-ID. WORKING-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'WORKING-FORM'.

      *>── Timer: Tmr-Working ──────────────────────────────────────────
       01 WS-Tmr-Working-INTERVAL   PIC 9(8) VALUE 120.
       01 WS-Tmr-Working-ENABLED    PIC 9    VALUE 1.
       01 WS-Tmr-Working-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-TEXT   PIC X(300) GLOBAL.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Working.
          05 WS-Lbl-Working-TEXT       PIC X(256) VALUE 'Working…'.
          05 WS-Lbl-Working-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Working-ENABLED    PIC 9      VALUE 1.

       01 WS-Pgb-Working.
          05 WS-Pgb-Working-TEXT       PIC X(256) VALUE 'Pgb-Working'.
          05 WS-Pgb-Working-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pgb-Working-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Working.
          05 WS-Tmr-Working-TEXT       PIC X(256) VALUE 'Tmr-Working'.
          05 WS-Tmr-Working-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Working-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "WORKING-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "WORKING-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Tmr-Working"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-WORKING--ONTICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Tmr-Working 'SetInterval' USING BY VALUE 120
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
       PROGRAM-ID. WORKING-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    super::GetProperty( "WorkingText" ) RETURNING WS-TEXT
           SET     LBL-WORKING::Caption          TO    WS-TEXT::Trim()

           GOBACK.

       END PROGRAM WORKING-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WORKING-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM WORKING-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-WORKING--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The bar only shows that something is going on; it does not measure anything.

           ADD 8 TO PGB-WORKING::Value
           IF PGB-WORKING::Value >= 100
               SET     PGB-WORKING::Value            TO    0
           END-IF

           GOBACK.

       END PROGRAM TMR-WORKING--ONTICK.

       END PROGRAM WORKING-FORM.

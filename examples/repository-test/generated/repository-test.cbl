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
       PROGRAM-ID. REPO-TEST.

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
       01 FORM-NAME               PIC X(64)   VALUE 'REPO-TEST'.

      *>── User Working Storage ────────────────────────────────────────
       01 RSTR USAGE OBJECT REFERENCE RUST-STRING VALUE "Hello".
       01 RINT USAGE OBJECT REFERENCE RUST-I64 VALUE "10".
       01 RFLT USAGE OBJECT REFERENCE RUST-F64 VALUE "16".
       01 RBOO USAGE OBJECT REFERENCE RUST-BOOL VALUE "1".
       01 RVEC USAGE OBJECT REFERENCE RUST-VEC.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-RUN-TESTS.
          05 WS-RUN-TESTS-TEXT       PIC X(256) VALUE 'Run REPOSITORY tests'.
          05 WS-RUN-TESTS-VISIBLE    PIC 9      VALUE 1.
          05 WS-RUN-TESTS-ENABLED    PIC 9      VALUE 1.

       01 WS-RESULTS.
          05 WS-RESULTS-TEXT       PIC X(256) VALUE 'RESULTS'.
          05 WS-RESULTS-VISIBLE    PIC 9      VALUE 1.
          05 WS-RESULTS-ENABLED    PIC 9      VALUE 1.
          05 WS-RESULTS-VALUE      PIC X(512) VALUE SPACES.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           CALL "COBOL-INIT-FORM" USING FORM-NAME
           CALL "REPO-TEST--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "REPO-TEST--ONCLOSE"
           STOP RUN.

       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RUN-TESTS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RUN-TESTS--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. REPO-TEST--ONLOAD.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM REPO-TEST--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. REPO-TEST--ONCLOSE.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM REPO-TEST--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RUN-TESTS--ONCLICK.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-N    PIC 9(4).
       01 WS-T    PIC X(20).
       01 WS-LINE PIC X(50).

       PROCEDURE DIVISION.
           INVOKE RESULTS "Clear"
           INVOKE RESULTS "AddItem" USING "== Rust-FFI bridge tests =="

           INVOKE RSTR "len" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "String.len()       = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RSTR "to_uppercase" RETURNING WS-T
           MOVE SPACES TO WS-LINE
           STRING "String.to_uppercase = " DELIMITED BY SIZE
                  WS-T DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RSTR "contains" USING "ell" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "String.contains ell = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RSTR "push_str" USING "!!"
           INVOKE RSTR "len" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "String.len after +!! = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RINT "add" USING 5 RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "i64 10 add 5        = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RINT "pow" USING 2 RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "i64 (now 15) pow 2  = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RFLT "sqrt" RETURNING WS-T
           MOVE SPACES TO WS-LINE
           STRING "f64 16 sqrt         = " DELIMITED BY SIZE
                  WS-T DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RBOO "not" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "bool true not       = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RVEC "push" USING 7
           INVOKE RVEC "push" USING 8
           INVOKE RVEC "len" RETURNING WS-N
           MOVE SPACES TO WS-LINE
           STRING "Vec push x2, len    = " DELIMITED BY SIZE
                  WS-N DELIMITED BY SIZE INTO WS-LINE
           INVOKE RESULTS "AddItem" USING WS-LINE

           INVOKE RESULTS "AddItem" USING "== done ==".

           GOBACK.

       END PROGRAM RUN-TESTS--ONCLICK.

       END PROGRAM REPO-TEST.

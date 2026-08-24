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
       PROGRAM-ID. CUSTOMER-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'CUSTOMER-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-CustomerFile.
          05 WS-CustomerFile-TEXT       PIC X(256) VALUE 'CustomerFile'.
          05 WS-CustomerFile-VISIBLE    PIC 9      VALUE 1.
          05 WS-CustomerFile-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           CALL "COBOL-INIT-FORM" USING FORM-NAME
           PERFORM CustomerFile-OPEN
           CALL "CUSTOMER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CUSTOMER-FORM--ONCLOSE"
           PERFORM CustomerFile-CLOSE
           STOP RUN.

       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               *> No event handlers defined yet.
               CONTINUE
           END-PERFORM.

       CustomerFile-OPEN.
      *>  Opens indexed file CUSTOMERS for I-O.
           IF WS-CustomerFile-IS-OPEN = 0
               OPEN I-O CUSTOMERS
               MOVE '00' TO WS-CustomerFile-STATUS
               MOVE 1 TO WS-CustomerFile-IS-OPEN
               MOVE 0 TO WS-CustomerFile-AT-END
               MOVE 0 TO WS-CustomerFile-HAS-RECORD
           END-IF.

       CustomerFile-START.
      *>  Set CUSTOMER-ID, then PERFORM CustomerFile-START to position the current pointer.
           START CUSTOMERS KEY IS GREATER THAN OR EQUAL TO CUSTOMER-ID
               INVALID KEY
                   MOVE '23' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT INVALID KEY
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-AT-END
           END-START.

       CustomerFile-READ-NEXT.
           READ CUSTOMERS NEXT
               AT END
                   MOVE '10' TO WS-CustomerFile-STATUS
                   MOVE 1 TO WS-CustomerFile-AT-END
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT AT END
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-AT-END
                   MOVE 1 TO WS-CustomerFile-HAS-RECORD
           END-READ.

       CustomerFile-READ-PREVIOUS.
           READ CUSTOMERS PREVIOUS
               AT END
                   MOVE '10' TO WS-CustomerFile-STATUS
                   MOVE 1 TO WS-CustomerFile-AT-END
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT AT END
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-AT-END
                   MOVE 1 TO WS-CustomerFile-HAS-RECORD
           END-READ.

       CustomerFile-READ-FIRST.
      *>  Set CUSTOMER-ID to the lowest desired value, position, then read NEXT.
           START CUSTOMERS KEY IS GREATER THAN OR EQUAL TO CUSTOMER-ID
               INVALID KEY CONTINUE
           END-START
           READ CUSTOMERS NEXT
               AT END
                   MOVE '10' TO WS-CustomerFile-STATUS
                   MOVE 1 TO WS-CustomerFile-AT-END
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT AT END
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-AT-END
                   MOVE 1 TO WS-CustomerFile-HAS-RECORD
           END-READ.

       CustomerFile-READ-LAST.
      *>  Set CUSTOMER-ID to the highest desired value, position, then read PREVIOUS.
           START CUSTOMERS KEY IS LESS THAN OR EQUAL TO CUSTOMER-ID
               INVALID KEY CONTINUE
           END-START
           READ CUSTOMERS PREVIOUS
               AT END
                   MOVE '10' TO WS-CustomerFile-STATUS
                   MOVE 1 TO WS-CustomerFile-AT-END
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT AT END
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-AT-END
                   MOVE 1 TO WS-CustomerFile-HAS-RECORD
           END-READ.

       CustomerFile-READ-INVALID.
      *>  Direct keyed read. Set CUSTOMER-ID before calling this paragraph.
           READ CUSTOMERS
               INVALID KEY
                   MOVE '23' TO WS-CustomerFile-STATUS
                   MOVE 0 TO WS-CustomerFile-HAS-RECORD
               NOT INVALID KEY
                   MOVE '00' TO WS-CustomerFile-STATUS
                   MOVE 1 TO WS-CustomerFile-HAS-RECORD
           END-READ.

       CustomerFile-WRITE.
      *>  Requires CUSTOMERS opened I-O. Data comes from bound/set record fields.
           WRITE CUSTOMER-REC
               INVALID KEY
                   MOVE '23' TO WS-CustomerFile-STATUS
               NOT INVALID KEY
                   MOVE '00' TO WS-CustomerFile-STATUS
           END-WRITE.

       CustomerFile-REWRITE.
      *>  Requires CUSTOMERS opened I-O. Data comes from bound/set record fields.
           REWRITE CUSTOMER-REC
               INVALID KEY
                   MOVE '23' TO WS-CustomerFile-STATUS
               NOT INVALID KEY
                   MOVE '00' TO WS-CustomerFile-STATUS
           END-REWRITE.

       CustomerFile-DELETE.
      *>  Requires CUSTOMERS opened I-O. Data comes from bound/set record fields.
           DELETE CUSTOMERS
               INVALID KEY
                   MOVE '23' TO WS-CustomerFile-STATUS
               NOT INVALID KEY
                   MOVE '00' TO WS-CustomerFile-STATUS
           END-DELETE.

       CustomerFile-COMMIT.
      *>  Flushes pending indexed-file changes for CUSTOMERS.
           CLOSE CUSTOMERS
           OPEN I-O CUSTOMERS
           MOVE '00' TO WS-CustomerFile-STATUS.

       CustomerFile-ROLLBACK.
      *>  Transaction rollback is storage-engine dependent; reopen to discard pending cursor state.
           CLOSE CUSTOMERS
           OPEN I-O CUSTOMERS
           MOVE '00' TO WS-CustomerFile-STATUS.

       CustomerFile-CLOSE.
      *>  No-op when already closed. I-O close commits automatically.
           IF WS-CustomerFile-IS-OPEN = 1
               PERFORM CustomerFile-COMMIT
               CLOSE CUSTOMERS
               MOVE 0 TO WS-CustomerFile-IS-OPEN
               MOVE '00' TO WS-CustomerFile-STATUS
           END-IF.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTOMER-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CUSTOMER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTOMER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CUSTOMER-FORM--ONCLOSE.

       END PROGRAM CUSTOMER-FORM.

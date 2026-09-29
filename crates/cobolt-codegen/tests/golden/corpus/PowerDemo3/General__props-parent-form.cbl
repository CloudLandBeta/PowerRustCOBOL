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
       PROGRAM-ID. PROPS-PARENT-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PROPS-PARENT-FORM'.

      *>── User Working Storage ────────────────────────────────────────
*>   The order both forms share - a group item with a table inside.
      *>   It travels between them as ONE property, its fixed layout intact,
      *>   so both sides must declare it the same way (a real application
      *>   would COPY it from one copybook).
       01 WS-ORDER           GLOBAL.
          05 ORD-NUMBER      PIC 9(6).
          05 ORD-DATE        PIC X(10).
          05 ORD-LINE        OCCURS 3 TIMES.
             10 ORD-ITEM     PIC X(20).
             10 ORD-QTY      PIC 9(3).
             10 ORD-PRICE    PIC 9(5)V99.
       01 WS-TOTAL           GLOBAL PIC 9(7)V99.
       01 WS-I               GLOBAL PIC 9.
       01 WS-LINE            GLOBAL PIC X(80).
       01 WS-QTY-ED          GLOBAL PIC ZZ9.
       01 WS-AMT-ED          GLOBAL PIC ZZ,ZZ9.99.
       01 WS-TOTAL-ED        GLOBAL PIC Z,ZZZ,ZZ9.99.
       01 WS-VIP             GLOBAL PIC X.
       01 WS-RESULT          GLOBAL PIC X(10).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Passing Data to a Child Form'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Intro.
          05 WS-Lbl-Intro-TEXT       PIC X(256) VALUE 'Simple values and a whole record go to the child form as properties; the child edits them and sends them back.'.
          05 WS-Lbl-Intro-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Intro-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Simple.
          05 WS-Lbl-Simple-TEXT       PIC X(256) VALUE 'Simple values'.
          05 WS-Lbl-Simple-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Simple-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Name.
          05 WS-Lbl-Name-TEXT       PIC X(256) VALUE 'Customer'.
          05 WS-Lbl-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Name-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Name.
          05 WS-Txt-Name-TEXT       PIC X(256) VALUE 'Maria Silva'.
          05 WS-Txt-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Name-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Name-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Limit.
          05 WS-Lbl-Limit-TEXT       PIC X(256) VALUE 'Credit limit'.
          05 WS-Lbl-Limit-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Limit-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Limit.
          05 WS-Txt-Limit-TEXT       PIC X(256) VALUE '50000'.
          05 WS-Txt-Limit-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Limit-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Limit-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Chk-Vip.
          05 WS-Chk-Vip-TEXT       PIC X(256) VALUE 'VIP customer'.
          05 WS-Chk-Vip-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Vip-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Vip-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Complex.
          05 WS-Lbl-Complex-TEXT       PIC X(256) VALUE 'Complex data - an order record with a table'.
          05 WS-Lbl-Complex-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Complex-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Order.
          05 WS-Lbl-Order-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Order-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Order-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Line-1.
          05 WS-Lbl-Line-1-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Line-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Line-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Line-2.
          05 WS-Lbl-Line-2-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Line-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Line-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Line-3.
          05 WS-Lbl-Line-3-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Line-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Line-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Edit.
          05 WS-Btn-Edit-TEXT       PIC X(256) VALUE 'Edit in the child form'.
          05 WS-Btn-Edit-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Edit-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'The child form has not been opened yet.'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "PROPS-PARENT-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PROPS-PARENT-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Edit"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-EDIT--ONCLICK"
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
       PROGRAM-ID. PROPS-PARENT-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   A sample order to send to the child form.
           MOVE 104217       TO ORD-NUMBER
           MOVE "2026-09-28" TO ORD-DATE
           MOVE "Office chair"   TO ORD-ITEM(1)
           MOVE 2              TO ORD-QTY(1)
           MOVE 649.90         TO ORD-PRICE(1)
           MOVE "Standing desk"  TO ORD-ITEM(2)
           MOVE 1              TO ORD-QTY(2)
           MOVE 1890.00        TO ORD-PRICE(2)
           MOVE "Monitor arm"    TO ORD-ITEM(3)
           MOVE 3              TO ORD-QTY(3)
           MOVE 219.50         TO ORD-PRICE(3)
           CALL "PP-SHOW-ORDER"

           GOBACK.

       END PROGRAM PROPS-PARENT-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROPS-PARENT-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PROPS-PARENT-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-EDIT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   1. Publish the data on THIS form's property surface. Any name
      *>      will do - SetProperty/GetProperty are the any-name pair.
      *>      Simple values: one property each.
           INVOKE ME::"SetProperty"("CustomerName", Txt-Name::Text)
           INVOKE ME::"SetProperty"("CreditLimit", Txt-Limit::Text)
           IF Chk-Vip::Checked = 1
               MOVE "Y" TO WS-VIP
           ELSE
               MOVE "N" TO WS-VIP
           END-IF
           INVOKE ME::"SetProperty"("IsVip", WS-VIP)
      *>      Complex data: the whole group item - header and table -
      *>      as ONE property.
           INVOKE ME::"SetProperty"("OrderRecord", WS-ORDER)
           INVOKE ME::"SetProperty"("ChildResult", "NONE")
      *>   2. Open the child modally: it reads all of it with
      *>      super::"GetProperty", and answers with super::"SetProperty".
           INVOKE ME::"OpenFormSync"("PROPS-CHILD-FORM")
      *>   3. Back here: read the answer.
           INVOKE ME::"GetProperty"("ChildResult") RETURNING WS-RESULT
           IF WS-RESULT = "OK"
               INVOKE ME::"GetProperty"("CustomerName") RETURNING Txt-Name::Text
               INVOKE ME::"GetProperty"("OrderRecord") RETURNING WS-ORDER
               CALL "PP-SHOW-ORDER"
               MOVE "The child form sent the customer and the order back."
                   TO Lbl-Status::Caption
           ELSE
               MOVE "The child form was cancelled - nothing changed."
                   TO Lbl-Status::Caption
           END-IF

           GOBACK.

       END PROGRAM BTN-EDIT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PP-SHOW-ORDER IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   WS-ORDER in: one line of the order per label, and the total.
           MOVE 0 TO WS-TOTAL
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               MOVE ORD-QTY(WS-I) TO WS-QTY-ED
               COMPUTE WS-AMT-ED = ORD-QTY(WS-I) * ORD-PRICE(WS-I)
               COMPUTE WS-TOTAL = WS-TOTAL + ORD-QTY(WS-I) * ORD-PRICE(WS-I)
               MOVE SPACES TO WS-LINE
               STRING WS-QTY-ED " x " ORD-ITEM(WS-I) "  = " WS-AMT-ED
                   DELIMITED BY SIZE INTO WS-LINE
               EVALUATE WS-I
                   WHEN 1 MOVE WS-LINE TO Lbl-Line-1::Caption
                   WHEN 2 MOVE WS-LINE TO Lbl-Line-2::Caption
                   WHEN 3 MOVE WS-LINE TO Lbl-Line-3::Caption
               END-EVALUATE
           END-PERFORM
           MOVE WS-TOTAL TO WS-TOTAL-ED
           MOVE SPACES TO WS-LINE
           STRING "Order " ORD-NUMBER " of " ORD-DATE " - total " WS-TOTAL-ED
               DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO Lbl-Order::Caption

           GOBACK.

       END PROGRAM PP-SHOW-ORDER.

       END PROGRAM PROPS-PARENT-FORM.

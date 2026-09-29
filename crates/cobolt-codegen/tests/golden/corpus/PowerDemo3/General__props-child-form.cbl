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
       PROGRAM-ID. PROPS-CHILD-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PROPS-CHILD-FORM'.

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
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Order Editor'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-From.
          05 WS-Lbl-From-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-From-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-From-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Name.
          05 WS-Lbl-Name-TEXT       PIC X(256) VALUE 'Customer'.
          05 WS-Lbl-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Name-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Name.
          05 WS-Txt-Name-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Name-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Name-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Limit.
          05 WS-Lbl-Limit-TEXT       PIC X(256) VALUE 'Credit limit'.
          05 WS-Lbl-Limit-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Limit-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Limit-Value.
          05 WS-Lbl-Limit-Value-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Limit-Value-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Limit-Value-ENABLED    PIC 9      VALUE 1.

       01 WS-Chk-Vip.
          05 WS-Chk-Vip-TEXT       PIC X(256) VALUE 'VIP customer'.
          05 WS-Chk-Vip-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Vip-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Vip-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Order.
          05 WS-Lbl-Order-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Order-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Order-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Item-1.
          05 WS-Lbl-Item-1-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Item-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Item-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Qty-1.
          05 WS-Txt-Qty-1-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Qty-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Qty-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Qty-1-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Item-2.
          05 WS-Lbl-Item-2-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Item-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Item-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Qty-2.
          05 WS-Txt-Qty-2-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Qty-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Qty-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Qty-2-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Item-3.
          05 WS-Lbl-Item-3-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Item-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Item-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Qty-3.
          05 WS-Txt-Qty-3-TEXT       PIC X(256) VALUE SPACES.
          05 WS-Txt-Qty-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Qty-3-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Qty-3-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Total.
          05 WS-Lbl-Total-TEXT       PIC X(256) VALUE ''.
          05 WS-Lbl-Total-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Total-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Recalc.
          05 WS-Btn-Recalc-TEXT       PIC X(256) VALUE 'Recalculate'.
          05 WS-Btn-Recalc-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Recalc-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Ok.
          05 WS-Btn-Ok-TEXT       PIC X(256) VALUE 'OK - send back'.
          05 WS-Btn-Ok-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Ok-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "PROPS-CHILD-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PROPS-CHILD-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Recalc"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RECALC--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Ok"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-OK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
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
       PROGRAM-ID. PROPS-CHILD-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   Read what the parent published - super is the form that opened
      *>   this one. A form property of the parent's own surface reads
      *>   bare (super::Title); a property of any name, through GetProperty.
           MOVE SPACES TO WS-LINE
           STRING "Opened by: " super::Title DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO Lbl-From::Caption
      *>   Simple values.
           INVOKE super::"GetProperty"("CustomerName") RETURNING Txt-Name::Text
           INVOKE super::"GetProperty"("CreditLimit") RETURNING Lbl-Limit-Value::Caption
           INVOKE super::"GetProperty"("IsVip") RETURNING WS-VIP
           IF WS-VIP = "Y"
               SET Chk-Vip::Checked TO TRUE
           ELSE
               SET Chk-Vip::Checked TO FALSE
           END-IF
      *>   Complex data: the record lands in the same layout, field by field.
           INVOKE super::"GetProperty"("OrderRecord") RETURNING WS-ORDER
           MOVE ORD-ITEM(1) TO Lbl-Item-1::Caption
           MOVE ORD-ITEM(2) TO Lbl-Item-2::Caption
           MOVE ORD-ITEM(3) TO Lbl-Item-3::Caption
           MOVE ORD-QTY(1) TO WS-QTY-ED
           MOVE WS-QTY-ED TO Txt-Qty-1::Text
           MOVE ORD-QTY(2) TO WS-QTY-ED
           MOVE WS-QTY-ED TO Txt-Qty-2::Text
           MOVE ORD-QTY(3) TO WS-QTY-ED
           MOVE WS-QTY-ED TO Txt-Qty-3::Text
           MOVE SPACES TO WS-LINE
           STRING "Order " ORD-NUMBER " of " ORD-DATE DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO Lbl-Order::Caption
           CALL "PC-TOTAL"

           GOBACK.

       END PROGRAM PROPS-CHILD-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROPS-CHILD-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PROPS-CHILD-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RECALC--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-TOTAL"

           GOBACK.

       END PROGRAM BTN-RECALC--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-OK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   Send the edited data back to the parent - it reads it after
      *>   OpenFormSync returns - and close.
           CALL "PC-TOTAL"
           INVOKE super::"SetProperty"("CustomerName", Txt-Name::Text)
           INVOKE super::"SetProperty"("OrderRecord", WS-ORDER)
           INVOKE super::"SetProperty"("ChildResult", "OK")
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM BTN-OK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   Nothing is sent back but the answer itself.
           INVOKE super::"SetProperty"("ChildResult", "CANCEL")
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TOTAL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
*>   The quantities typed, back into the record, and its total.
           MOVE FUNCTION NUMVAL(Txt-Qty-1::Text) TO ORD-QTY(1)
           MOVE FUNCTION NUMVAL(Txt-Qty-2::Text) TO ORD-QTY(2)
           MOVE FUNCTION NUMVAL(Txt-Qty-3::Text) TO ORD-QTY(3)
           MOVE 0 TO WS-TOTAL
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               COMPUTE WS-TOTAL = WS-TOTAL + ORD-QTY(WS-I) * ORD-PRICE(WS-I)
           END-PERFORM
           MOVE WS-TOTAL TO WS-TOTAL-ED
           MOVE SPACES TO WS-LINE
           STRING "Total " WS-TOTAL-ED DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO Lbl-Total::Caption

           GOBACK.

       END PROGRAM PC-TOTAL.

       END PROGRAM PROPS-CHILD-FORM.

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
       PROGRAM-ID. GROUPBOX-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'GROUPBOX-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'GroupBox'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A GroupBox is a real container: a control dropped inside becomes its CHILD and moves, hides and disables with it. Membership is the child''s parent link, never geometry - an overlapping neighbour is not a member.'.
          05 WS-Lbl-Sub-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sub-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Log-Cap.
          05 WS-Lbl-Log-Cap-TEXT       PIC X(256) VALUE 'Event log'.
          05 WS-Lbl-Log-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Log-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Log.
          05 WS-Txt-Log-TEXT       PIC X(2048) VALUE 'Txt-Log'.
          05 WS-Txt-Log-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Log-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Log-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Grp-Address.
          05 WS-Grp-Address-TEXT       PIC X(256) VALUE 'Delivery address'.
          05 WS-Grp-Address-VISIBLE    PIC 9      VALUE 1.
          05 WS-Grp-Address-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Street.
          05 WS-Lbl-Street-TEXT       PIC X(256) VALUE 'Street'.
          05 WS-Lbl-Street-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Street-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Street.
          05 WS-Txt-Street-TEXT       PIC X(256) VALUE 'Rua das Laranjeiras, 120'.
          05 WS-Txt-Street-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Street-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Street-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-City.
          05 WS-Lbl-City-TEXT       PIC X(256) VALUE 'City'.
          05 WS-Lbl-City-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-City-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-City.
          05 WS-Txt-City-TEXT       PIC X(256) VALUE 'Rio de Janeiro'.
          05 WS-Txt-City-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-City-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-City-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Chk-Invoice.
          05 WS-Chk-Invoice-TEXT       PIC X(256) VALUE 'Same as the invoice address'.
          05 WS-Chk-Invoice-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Invoice-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Invoice-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Grp-Speed.
          05 WS-Grp-Speed-TEXT       PIC X(256) VALUE 'Delivery speed'.
          05 WS-Grp-Speed-VISIBLE    PIC 9      VALUE 1.
          05 WS-Grp-Speed-ENABLED    PIC 9      VALUE 1.

       01 WS-Rad-Slow.
          05 WS-Rad-Slow-TEXT       PIC X(256) VALUE 'Economy'.
          05 WS-Rad-Slow-VISIBLE    PIC 9      VALUE 1.
          05 WS-Rad-Slow-ENABLED    PIC 9      VALUE 1.

       01 WS-Rad-Fast.
          05 WS-Rad-Fast-TEXT       PIC X(256) VALUE 'Express'.
          05 WS-Rad-Fast-VISIBLE    PIC 9      VALUE 1.
          05 WS-Rad-Fast-ENABLED    PIC 9      VALUE 1.

       01 WS-Rad-Same.
          05 WS-Rad-Same-TEXT       PIC X(256) VALUE 'Same day'.
          05 WS-Rad-Same-VISIBLE    PIC 9      VALUE 1.
          05 WS-Rad-Same-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Note.
          05 WS-Lbl-Note-TEXT       PIC X(256) VALUE 'Hiding the group hides all of this at once.'.
          05 WS-Lbl-Note-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Note-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-HideGrp.
          05 WS-Btn-HideGrp-TEXT       PIC X(256) VALUE 'Hide the group'.
          05 WS-Btn-HideGrp-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-HideGrp-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-ShowGrp.
          05 WS-Btn-ShowGrp-TEXT       PIC X(256) VALUE 'Show the group'.
          05 WS-Btn-ShowGrp-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-ShowGrp-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lock.
          05 WS-Btn-Lock-TEXT       PIC X(256) VALUE 'Disable address'.
          05 WS-Btn-Lock-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lock-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Unlock.
          05 WS-Btn-Unlock-TEXT       PIC X(256) VALUE 'Enable address'.
          05 WS-Btn-Unlock-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Unlock-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Read.
          05 WS-Btn-Read-TEXT       PIC X(256) VALUE 'Read the group'.
          05 WS-Btn-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Read-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "GROUPBOX-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "GROUPBOX-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-HideGrp"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HIDEGRP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-ShowGrp"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SHOWGRP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lock"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LOCK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Unlock"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-UNLOCK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Read"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-READ--ONCLICK"
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
       PROGRAM-ID. GROUPBOX-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Rad-Fast::Select().
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "ready - two groups: " Grp-Address::Caption " and " Grp-Speed::Caption INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM GROUPBOX-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GROUPBOX-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM GROUPBOX-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HIDEGRP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> One call takes the caption, the frame and every child with it.
      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Grp-Speed::Hide().
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING Grp-Speed::Caption " hidden - its three radios went with it" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-HIDEGRP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SHOWGRP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Grp-Speed::Show().

           GOBACK.

       END PROGRAM BTN-SHOWGRP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LOCK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Disabling the container greys every field inside it.
      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Grp-Address::Disable().

           GOBACK.

       END PROGRAM BTN-LOCK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-UNLOCK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Grp-Address::Enable().

           GOBACK.

       END PROGRAM BTN-UNLOCK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-READ--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING Txt-Street::Text ", " Txt-City::Text INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-READ--ONCLICK.

       END PROGRAM GROUPBOX-FORM.

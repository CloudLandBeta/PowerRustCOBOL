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
       PROGRAM-ID. SPLITTER-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SPLITTER-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Splitter'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(259) VALUE 'A Splitter is a container divided by a draggable line. Dropping one gives you THREE controls: the splitter and its two panes, which are ordinary Panels. Orientation names how the PANES sit, and SplitPosition is a percentage, so the division survives a resize.'.
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

       01 WS-Spl-Work.
          05 WS-Spl-Work-TEXT       PIC X(256) VALUE 'Spl-Work'.
          05 WS-Spl-Work-VISIBLE    PIC 9      VALUE 1.
          05 WS-Spl-Work-ENABLED    PIC 9      VALUE 1.

       01 WS-Spl-Work-Pane1.
          05 WS-Spl-Work-Pane1-TEXT       PIC X(256) VALUE 'Spl-Work-Pane1'.
          05 WS-Spl-Work-Pane1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Spl-Work-Pane1-ENABLED    PIC 9      VALUE 1.

       01 WS-Spl-Work-Pane2.
          05 WS-Spl-Work-Pane2-TEXT       PIC X(256) VALUE 'Spl-Work-Pane2'.
          05 WS-Spl-Work-Pane2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Spl-Work-Pane2-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Pane1.
          05 WS-Lbl-Pane1-TEXT       PIC X(256) VALUE 'Pane 1 - navigation'.
          05 WS-Lbl-Pane1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Pane1-ENABLED    PIC 9      VALUE 1.

       01 WS-Lst-Nav.
          05 WS-Lst-Nav-TEXT       PIC X(256) VALUE 'Lst-Nav'.
          05 WS-Lst-Nav-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Nav-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Nav-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Pane2.
          05 WS-Lbl-Pane2-TEXT       PIC X(256) VALUE 'Pane 2 - detail'.
          05 WS-Lbl-Pane2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Pane2-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Detail.
          05 WS-Txt-Detail-TEXT       PIC X(2048) VALUE 'Drag the division line - anywhere along it, not only on the grip. Double-click it to put the split back at 50%.'.
          05 WS-Txt-Detail-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Detail-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Detail-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Btn-Narrow.
          05 WS-Btn-Narrow-TEXT       PIC X(256) VALUE 'Split at 20%'.
          05 WS-Btn-Narrow-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Narrow-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Even.
          05 WS-Btn-Even-TEXT       PIC X(256) VALUE 'Split at 50%'.
          05 WS-Btn-Even-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Even-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Wide.
          05 WS-Btn-Wide-TEXT       PIC X(256) VALUE 'Split at 80%'.
          05 WS-Btn-Wide-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Wide-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Flip.
          05 WS-Btn-Flip-TEXT       PIC X(256) VALUE 'Flip orientation'.
          05 WS-Btn-Flip-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Flip-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SPLITTER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SPLITTER-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Narrow"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NARROW--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Even"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-EVEN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Wide"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-WIDE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Flip"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FLIP--ONCLICK"
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
       PROGRAM-ID. SPLITTER-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - " Spl-Work::Orientation " at " Spl-Work::SplitPosition "%" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SPLITTER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SPLITTER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SPLITTER-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NARROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> SplitPosition is a percentage, so it holds when the form is resized.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE 20 TO Spl-Work::SplitPosition.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "split -> " Spl-Work::SplitPosition "%" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-NARROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-EVEN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE 50 TO Spl-Work::SplitPosition.

           GOBACK.

       END PROGRAM BTN-EVEN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-WIDE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE 80 TO Spl-Work::SplitPosition.

           GOBACK.

       END PROGRAM BTN-WIDE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FLIP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> Horizontal puts pane 1 on the left; Vertical puts it on top.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           IF Spl-Work::Orientation = "Horizontal"
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               MOVE "Vertical" TO Spl-Work::Orientation
           ELSE
               MOVE "Horizontal" TO Spl-Work::Orientation
           END-IF.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "orientation -> " Spl-Work::Orientation INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FLIP--ONCLICK.

       END PROGRAM SPLITTER-FORM.

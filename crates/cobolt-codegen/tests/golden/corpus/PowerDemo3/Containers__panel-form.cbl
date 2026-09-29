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
       PROGRAM-ID. PANEL-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PANEL-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Panel'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Panel is a GroupBox without the caption: a plain rectangle that owns its children. CornerRadius rounds it, HideBackground makes it a pure grouping device, and moving it with MoveTo carries everything inside.'.
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

       01 WS-Pnl-Card.
          05 WS-Pnl-Card-TEXT       PIC X(256) VALUE 'Pnl-Card'.
          05 WS-Pnl-Card-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pnl-Card-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-CardTitle.
          05 WS-Lbl-CardTitle-TEXT       PIC X(256) VALUE 'Titan Voyages'.
          05 WS-Lbl-CardTitle-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-CardTitle-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-CardBody.
          05 WS-Lbl-CardBody-TEXT       PIC X(256) VALUE 'Everything on this card belongs to the panel. Move the panel and the card moves whole.'.
          05 WS-Lbl-CardBody-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-CardBody-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-CardGo.
          05 WS-Btn-CardGo-TEXT       PIC X(256) VALUE 'Book now'.
          05 WS-Btn-CardGo-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-CardGo-ENABLED    PIC 9      VALUE 1.

       01 WS-Pnl-Scroll.
          05 WS-Pnl-Scroll-TEXT       PIC X(256) VALUE 'Pnl-Scroll'.
          05 WS-Pnl-Scroll-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pnl-Scroll-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row0.
          05 WS-Lbl-Row0-TEXT       PIC X(256) VALUE 'Scrollable row 1'.
          05 WS-Lbl-Row0-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row0-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row1.
          05 WS-Lbl-Row1-TEXT       PIC X(256) VALUE 'Scrollable row 2'.
          05 WS-Lbl-Row1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row1-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row2.
          05 WS-Lbl-Row2-TEXT       PIC X(256) VALUE 'Scrollable row 3'.
          05 WS-Lbl-Row2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row2-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row3.
          05 WS-Lbl-Row3-TEXT       PIC X(256) VALUE 'Scrollable row 4'.
          05 WS-Lbl-Row3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row3-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row4.
          05 WS-Lbl-Row4-TEXT       PIC X(256) VALUE 'Scrollable row 5'.
          05 WS-Lbl-Row4-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row4-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row5.
          05 WS-Lbl-Row5-TEXT       PIC X(256) VALUE 'Scrollable row 6'.
          05 WS-Lbl-Row5-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row5-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row6.
          05 WS-Lbl-Row6-TEXT       PIC X(256) VALUE 'Scrollable row 7'.
          05 WS-Lbl-Row6-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row6-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Row7.
          05 WS-Lbl-Row7-TEXT       PIC X(256) VALUE 'Scrollable row 8'.
          05 WS-Lbl-Row7-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Row7-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Move.
          05 WS-Btn-Move-TEXT       PIC X(256) VALUE 'Nudge the card'.
          05 WS-Btn-Move-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Move-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Home.
          05 WS-Btn-Home-TEXT       PIC X(256) VALUE 'Put it back'.
          05 WS-Btn-Home-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Home-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Grow.
          05 WS-Btn-Grow-TEXT       PIC X(256) VALUE 'Resize the card'.
          05 WS-Btn-Grow-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Grow-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Flat.
          05 WS-Btn-Flat-TEXT       PIC X(256) VALUE 'Flatten it'.
          05 WS-Btn-Flat-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Flat-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Round.
          05 WS-Btn-Round-TEXT       PIC X(256) VALUE 'Round it again'.
          05 WS-Btn-Round-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Round-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "PANEL-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PANEL-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Move"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MOVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Home"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HOME--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Grow"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-GROW--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Flat"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FLAT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Round"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ROUND--ONCLICK"
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
       PROGRAM-ID. PANEL-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - card at " Pnl-Card::X "," Pnl-Card::Y
                  " sized " Pnl-Card::Width "x" Pnl-Card::Height INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM PANEL-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PANEL-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PANEL-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MOVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> MoveTo writes X and Y; the children travel with the container.
      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Pnl-Card::MoveTo(80, 150).
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "card now at " Pnl-Card::X "," Pnl-Card::Y INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-MOVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HOME--ONCLICK IS COMMON PROGRAM.

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
           Pnl-Card::MoveTo(48, 130).

           GOBACK.

       END PROGRAM BTN-HOME--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-GROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Resize writes Width and Height - the panel's own rectangle, not its children's.
      *> EN: EXTENSION - universal control method: every visible control understands it.
      *> PT: EXTENSAO - metodo universal de controle: todo controle visivel o entende.
      *> ES: EXTENSION - metodo universal de control: todo control visible lo entiende.
      *> FR: EXTENSION - methode universelle de controle: tout controle visible la comprend.
      *> JP: EXTENSION - 共通コントロール メソッド。すべての可視コントロールが理解する。
      *> CN: EXTENSION - 通用控件方法：所有可见控件都支持。
           Pnl-Card::Resize(440, 220).

           GOBACK.

       END PROGRAM BTN-GROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FLAT--ONCLICK IS COMMON PROGRAM.

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
           MOVE 0 TO Pnl-Card::CornerRadius.
           MOVE 0 TO Pnl-Card::ShadowEnabled.

           GOBACK.

       END PROGRAM BTN-FLAT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ROUND--ONCLICK IS COMMON PROGRAM.

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
           MOVE 18 TO Pnl-Card::CornerRadius.
           MOVE 1  TO Pnl-Card::ShadowEnabled.

           GOBACK.

       END PROGRAM BTN-ROUND--ONCLICK.

       END PROGRAM PANEL-FORM.

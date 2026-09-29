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
       PROGRAM-ID. TABCONTROL-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'TABCONTROL-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'TabControl'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A TabControl stacks several pages in one place. Tabs holds the captions one per line, SelectedTab is the page on show, and every control carries the page number it belongs to. Switching pages raises onTabChanged.'.
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

       01 WS-Tab-Order.
          05 WS-Tab-Order-TEXT       PIC X(256) VALUE 'Tab-Order'.
          05 WS-Tab-Order-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tab-Order-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-P0.
          05 WS-Lbl-P0-TEXT       PIC X(256) VALUE 'Customer name'.
          05 WS-Lbl-P0-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-P0-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Name.
          05 WS-Txt-Name-TEXT       PIC X(256) VALUE 'Emerson Lopes'.
          05 WS-Txt-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Name-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Name-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-P0b.
          05 WS-Lbl-P0b-TEXT       PIC X(256) VALUE 'Loyalty tier'.
          05 WS-Lbl-P0b-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-P0b-ENABLED    PIC 9      VALUE 1.

       01 WS-Cbo-Tier.
          05 WS-Cbo-Tier-TEXT       PIC X(256) VALUE 'Cbo-Tier'.
          05 WS-Cbo-Tier-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cbo-Tier-ENABLED    PIC 9      VALUE 1.
          05 WS-Cbo-Tier-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lst-Items.
          05 WS-Lst-Items-TEXT       PIC X(256) VALUE 'Lst-Items'.
          05 WS-Lst-Items-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Items-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Items-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-P1.
          05 WS-Lbl-P1-TEXT       PIC X(256) VALUE 'Controls on this page exist all the time; only the visible page is painted.'.
          05 WS-Lbl-P1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-P1-ENABLED    PIC 9      VALUE 1.

       01 WS-Chk-Gift.
          05 WS-Chk-Gift-TEXT       PIC X(256) VALUE 'Gift wrap'.
          05 WS-Chk-Gift-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Gift-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Gift-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Dtp-When.
          05 WS-Dtp-When-TEXT       PIC X(256) VALUE 'Dtp-When'.
          05 WS-Dtp-When-VISIBLE    PIC 9      VALUE 1.
          05 WS-Dtp-When-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Next.
          05 WS-Btn-Next-TEXT       PIC X(256) VALUE 'Next page'.
          05 WS-Btn-Next-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Next-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Ship.
          05 WS-Btn-Ship-TEXT       PIC X(256) VALUE 'Jump to Shipping'.
          05 WS-Btn-Ship-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Ship-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Read.
          05 WS-Btn-Read-TEXT       PIC X(256) VALUE 'Read every page'.
          05 WS-Btn-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Read-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Bottom.
          05 WS-Btn-Bottom-TEXT       PIC X(256) VALUE 'Tabs at the bottom'.
          05 WS-Btn-Bottom-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Bottom-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "TABCONTROL-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "TABCONTROL-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Tab-Order"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTabChanged"
                               CALL "TAB-ORDER--ONTABCHANGED"
                       END-EVALUATE
                   WHEN "Btn-Next"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEXT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Ship"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SHIP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Read"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-READ--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Bottom"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-BOTTOM--ONCLICK"
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
       PROGRAM-ID. TABCONTROL-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - page " Tab-Order::SelectedTab " of 3" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM TABCONTROL-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TABCONTROL-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TABCONTROL-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TAB-ORDER--ONTABCHANGED IS COMMON PROGRAM.

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
           STRING "page -> " Tab-Order::SelectedTab INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM TAB-ORDER--ONTABCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEXT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Three pages, numbered from zero - wrap round at the end.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Tab-Order::SelectedTab < 2
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
               ADD 1 TO Tab-Order::SelectedTab
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           ELSE
               MOVE 0 TO Tab-Order::SelectedTab
           END-IF.

           GOBACK.

       END PROGRAM BTN-NEXT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SHIP--ONCLICK IS COMMON PROGRAM.

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
           MOVE 2 TO Tab-Order::SelectedTab.

           GOBACK.

       END PROGRAM BTN-SHIP--ONCLICK.

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
      *> A control on a hidden page is still live and still readable.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING Txt-Name::Text " / " Cbo-Tier::GetSelected()
                  " / gift=" Chk-Gift::IsChecked() INTO WS-LINE.
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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-BOTTOM--ONCLICK IS COMMON PROGRAM.

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
           MOVE "Bottom" TO Tab-Order::TabPosition.

           GOBACK.

       END PROGRAM BTN-BOTTOM--ONCLICK.

       END PROGRAM TABCONTROL-FORM.

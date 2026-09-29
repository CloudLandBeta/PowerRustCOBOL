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
       PROGRAM-ID. SWITCH-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SWITCH-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Switch'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Switch is a CheckBox with a different gesture: it reads as a live setting rather than a form field, so its change is expected to act immediately. Checked holds its state and Accent picks its colour family.'.
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

       01 WS-Lbl-Sw-Dark.
          05 WS-Lbl-Sw-Dark-TEXT       PIC X(256) VALUE 'Dark interface'.
          05 WS-Lbl-Sw-Dark-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sw-Dark-ENABLED    PIC 9      VALUE 1.

       01 WS-Sw-Dark.
          05 WS-Sw-Dark-TEXT       PIC X(256) VALUE 'Sw-Dark'.
          05 WS-Sw-Dark-VISIBLE    PIC 9      VALUE 1.
          05 WS-Sw-Dark-ENABLED    PIC 9      VALUE 1.
          05 WS-Sw-Dark-CHECKED    PIC 9      VALUE 0.

       01 WS-Lbl-Sw-Alerts.
          05 WS-Lbl-Sw-Alerts-TEXT       PIC X(256) VALUE 'Desktop alerts'.
          05 WS-Lbl-Sw-Alerts-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sw-Alerts-ENABLED    PIC 9      VALUE 1.

       01 WS-Sw-Alerts.
          05 WS-Sw-Alerts-TEXT       PIC X(256) VALUE 'Sw-Alerts'.
          05 WS-Sw-Alerts-VISIBLE    PIC 9      VALUE 1.
          05 WS-Sw-Alerts-ENABLED    PIC 9      VALUE 1.
          05 WS-Sw-Alerts-CHECKED    PIC 9      VALUE 0.

       01 WS-Lbl-Sw-Beta.
          05 WS-Lbl-Sw-Beta-TEXT       PIC X(256) VALUE 'Join the beta channel'.
          05 WS-Lbl-Sw-Beta-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sw-Beta-ENABLED    PIC 9      VALUE 1.

       01 WS-Sw-Beta.
          05 WS-Sw-Beta-TEXT       PIC X(256) VALUE 'Sw-Beta'.
          05 WS-Sw-Beta-VISIBLE    PIC 9      VALUE 1.
          05 WS-Sw-Beta-ENABLED    PIC 9      VALUE 1.
          05 WS-Sw-Beta-CHECKED    PIC 9      VALUE 0.

       01 WS-Lbl-State.
          05 WS-Lbl-State-TEXT       PIC X(256) VALUE '-'.
          05 WS-Lbl-State-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-State-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-AllOn.
          05 WS-Btn-AllOn-TEXT       PIC X(256) VALUE 'Everything on'.
          05 WS-Btn-AllOn-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-AllOn-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-AllOff.
          05 WS-Btn-AllOff-TEXT       PIC X(256) VALUE 'Everything off'.
          05 WS-Btn-AllOff-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-AllOff-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Read.
          05 WS-Btn-Read-TEXT       PIC X(256) VALUE 'Read settings'.
          05 WS-Btn-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Read-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SWITCH-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SWITCH-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Sw-Dark"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "SW-DARK--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "Sw-Alerts"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "SW-ALERTS--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "Sw-Beta"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "SW-BETA--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "Btn-AllOn"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ALLON--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-AllOff"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ALLOFF--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Read"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-READ--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Clear"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLEAR--ONCLICK"
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
       PROGRAM-ID. SWITCH-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - TRUE and FALSE are usable as operands; they are sugar for 1 and 0.
      *> PT: EXTENSAO - TRUE e FALSE servem como operandos; sao acucar sintatico para 1 e 0.
      *> ES: EXTENSION - TRUE y FALSE sirven como operandos; son azucar sintactico para 1 y 0.
      *> FR: EXTENSION - TRUE et FALSE s'emploient comme operandes; sucre syntaxique pour 1 et 0.
      *> JP: EXTENSION - TRUE と FALSE はオペランドとして使える。1 と 0 の糖衣構文である。
      *> CN: EXTENSION - TRUE 与 FALSE 可作为操作数使用；它们是 1 和 0 的语法糖。
           SET Sw-Alerts::Checked TO TRUE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "Flip a switch - the change is meant to take effect at once." TO Lbl-State::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "ready - alerts start " Sw-Alerts::IsChecked() INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SWITCH-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SWITCH-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SWITCH-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SW-DARK--ONCHECKEDCHANGED IS COMMON PROGRAM.

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
           STRING Lbl-Sw-Dark::Caption " -> " Sw-Dark::IsChecked() INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SW-DARK--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SW-ALERTS--ONCHECKEDCHANGED IS COMMON PROGRAM.

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
           STRING Lbl-Sw-Alerts::Caption " -> " Sw-Alerts::IsChecked() INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SW-ALERTS--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SW-BETA--ONCHECKEDCHANGED IS COMMON PROGRAM.

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
           STRING Lbl-Sw-Beta::Caption " -> " Sw-Beta::IsChecked() INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SW-BETA--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ALLON--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - TRUE and FALSE are usable as operands; they are sugar for 1 and 0.
      *> PT: EXTENSAO - TRUE e FALSE servem como operandos; sao acucar sintatico para 1 e 0.
      *> ES: EXTENSION - TRUE y FALSE sirven como operandos; son azucar sintactico para 1 y 0.
      *> FR: EXTENSION - TRUE et FALSE s'emploient comme operandes; sucre syntaxique pour 1 et 0.
      *> JP: EXTENSION - TRUE と FALSE はオペランドとして使える。1 と 0 の糖衣構文である。
      *> CN: EXTENSION - TRUE 与 FALSE 可作为操作数使用；它们是 1 和 0 的语法糖。
           SET Sw-Dark::Checked   TO TRUE.
           SET Sw-Alerts::Checked TO TRUE.
           SET Sw-Beta::Checked   TO TRUE.

           GOBACK.

       END PROGRAM BTN-ALLON--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ALLOFF--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - TRUE and FALSE are usable as operands; they are sugar for 1 and 0.
      *> PT: EXTENSAO - TRUE e FALSE servem como operandos; sao acucar sintatico para 1 e 0.
      *> ES: EXTENSION - TRUE y FALSE sirven como operandos; son azucar sintactico para 1 y 0.
      *> FR: EXTENSION - TRUE et FALSE s'emploient comme operandes; sucre syntaxique pour 1 et 0.
      *> JP: EXTENSION - TRUE と FALSE はオペランドとして使える。1 と 0 の糖衣構文である。
      *> CN: EXTENSION - TRUE 与 FALSE 可作为操作数使用；它们是 1 和 0 的语法糖。
           SET Sw-Dark::Checked   TO FALSE.
           SET Sw-Alerts::Checked TO FALSE.
           SET Sw-Beta::Checked   TO FALSE.

           GOBACK.

       END PROGRAM BTN-ALLOFF--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-READ--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT PIC X(160).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "dark=" Sw-Dark::Checked
                  "  alerts=" Sw-Alerts::Checked
                  "  beta=" Sw-Beta::Checked INTO WS-TEXT.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-TEXT TO Lbl-State::Caption.

           GOBACK.

       END PROGRAM BTN-READ--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLEAR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::Clear().

           GOBACK.

       END PROGRAM BTN-CLEAR--ONCLICK.

       END PROGRAM SWITCH-FORM.

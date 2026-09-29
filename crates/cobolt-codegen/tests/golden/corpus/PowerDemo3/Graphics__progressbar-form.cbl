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
       PROGRAM-ID. PROGRESSBAR-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PROGRESSBAR-FORM'.

      *>── Timer: Tmr-Copy ──────────────────────────────────────────
       01 WS-Tmr-Copy-INTERVAL   PIC 9(8) VALUE 120.
       01 WS-Tmr-Copy-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Copy-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'ProgressBar'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A ProgressBar shows a Value between Minimum and Maximum. It has no gesture of its own: your COBOL writes the number and the bar follows, raising onValueChanged as it moves and onCompleted when it reaches Maximum. A Timer drives it here.'.
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

       01 WS-Lbl-Copy-Cap.
          05 WS-Lbl-Copy-Cap-TEXT       PIC X(256) VALUE 'Continuous'.
          05 WS-Lbl-Copy-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Copy-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Copy.
          05 WS-Prg-Copy-TEXT       PIC X(256) VALUE 'Prg-Copy'.
          05 WS-Prg-Copy-VISIBLE    PIC 9      VALUE 1.
          05 WS-Prg-Copy-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Blocks-Cap.
          05 WS-Lbl-Blocks-Cap-TEXT       PIC X(256) VALUE 'Blocks'.
          05 WS-Lbl-Blocks-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Blocks-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Blocks.
          05 WS-Prg-Blocks-TEXT       PIC X(256) VALUE 'Prg-Blocks'.
          05 WS-Prg-Blocks-VISIBLE    PIC 9      VALUE 1.
          05 WS-Prg-Blocks-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Vert.
          05 WS-Prg-Vert-TEXT       PIC X(256) VALUE 'Prg-Vert'.
          05 WS-Prg-Vert-VISIBLE    PIC 9      VALUE 1.
          05 WS-Prg-Vert-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-State.
          05 WS-Lbl-State-TEXT       PIC X(256) VALUE 'idle'.
          05 WS-Lbl-State-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-State-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Copy.
          05 WS-Tmr-Copy-TEXT       PIC X(256) VALUE 'Tmr-Copy'.
          05 WS-Tmr-Copy-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Copy-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Start.
          05 WS-Btn-Start-TEXT       PIC X(256) VALUE 'Start copying'.
          05 WS-Btn-Start-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Start-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Stop.
          05 WS-Btn-Stop-TEXT       PIC X(256) VALUE 'Stop'.
          05 WS-Btn-Stop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Stop-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Half.
          05 WS-Btn-Half-TEXT       PIC X(256) VALUE 'Jump to half'.
          05 WS-Btn-Half-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Half-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Reset.
          05 WS-Btn-Reset-TEXT       PIC X(256) VALUE 'Back to zero'.
          05 WS-Btn-Reset-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Reset-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "PROGRESSBAR-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PROGRESSBAR-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Prg-Copy"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCompleted"
                               CALL "PRG-COPY--ONCOMPLETED"
                       END-EVALUATE
                   WHEN "Tmr-Copy"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-COPY--ONTICK"
                       END-EVALUATE
                   WHEN "Btn-Start"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-START--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Stop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-STOP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Half"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HALF--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Reset"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RESET--ONCLICK"
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
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Tmr-Copy 'SetInterval' USING BY VALUE 120
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
       PROGRAM-ID. PROGRESSBAR-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - scale " Prg-Copy::Minimum " .. " Prg-Copy::Maximum
                  ", tick every " Tmr-Copy::Interval " ms" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM PROGRESSBAR-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PROGRESSBAR-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PROGRESSBAR-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PRG-COPY--ONCOMPLETED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> Completion is an event, so nothing has to poll for the end.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Tmr-Copy::Stop().
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "Finished." TO Lbl-State::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "completed at " Prg-Copy::Value INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM PRG-COPY--ONCOMPLETED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-COPY--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> One tick, one step - the bar is written to, never clicked.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           ADD 4 TO Prg-Copy::Value.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Prg-Copy::Value TO Prg-Blocks::Value.
           MOVE Prg-Copy::Value TO Prg-Vert::Value.

           GOBACK.

       END PROGRAM TMR-COPY--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-START--ONCLICK IS COMMON PROGRAM.

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
           MOVE 0 TO Prg-Copy::Value.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "copying..." TO Lbl-State::Caption.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Tmr-Copy::Start().

           GOBACK.

       END PROGRAM BTN-START--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-STOP--ONCLICK IS COMMON PROGRAM.

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
           Tmr-Copy::Stop().
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "paused" TO Lbl-State::Caption.

           GOBACK.

       END PROGRAM BTN-STOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HALF--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Half of whatever the scale happens to be, not a hard-coded 50.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           COMPUTE Prg-Copy::Value = (Prg-Copy::Maximum - Prg-Copy::Minimum) / 2.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Prg-Copy::Value TO Prg-Blocks::Value.
           MOVE Prg-Copy::Value TO Prg-Vert::Value.

           GOBACK.

       END PROGRAM BTN-HALF--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RESET--ONCLICK IS COMMON PROGRAM.

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
           Tmr-Copy::Stop().
      *> EN: EXTENSION - INITIALIZE on a control resets its Value property.
      *> PT: EXTENSAO - INITIALIZE em um controle redefine a propriedade Value dele.
      *> ES: EXTENSION - INITIALIZE sobre un control reinicia su propiedad Value.
      *> FR: EXTENSION - INITIALIZE sur un controle reinitialise sa propriete Value.
      *> JP: EXTENSION - コントロールに対する INITIALIZE は Value プロパティを初期化する。
      *> CN: EXTENSION - 对控件执行 INITIALIZE 会重置其 Value 属性。
           INITIALIZE Prg-Copy.
           INITIALIZE Prg-Blocks.
           INITIALIZE Prg-Vert.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "idle" TO Lbl-State::Caption.

           GOBACK.

       END PROGRAM BTN-RESET--ONCLICK.

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

       END PROGRAM PROGRESSBAR-FORM.

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
       PROGRAM-ID. TIMER-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'TIMER-FORM'.

      *>── Timer: Tmr-Fast ──────────────────────────────────────────
       01 WS-Tmr-Fast-INTERVAL   PIC 9(8) VALUE 250.
       01 WS-Tmr-Fast-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Fast-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Timer: Tmr-Slow ──────────────────────────────────────────
       01 WS-Tmr-Slow-INTERVAL   PIC 9(8) VALUE 2000.
       01 WS-Tmr-Slow-ENABLED    PIC 9    VALUE 1.
       01 WS-Tmr-Slow-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Timer'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(258) VALUE 'A Timer fires onTick every Interval milliseconds while it is enabled. Enabled is the TIMER''s own switch - whether it runs - not whether a control is greyed out. The cadence is steady and missed time is never repaid: a stalled form gets one tick, not a burst.'.
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

       01 WS-Tmr-Fast.
          05 WS-Tmr-Fast-TEXT       PIC X(256) VALUE 'Tmr-Fast'.
          05 WS-Tmr-Fast-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Fast-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-TmrFast.
          05 WS-Lbl-TmrFast-TEXT       PIC X(256) VALUE 'Tmr-Fast  250 ms'.
          05 WS-Lbl-TmrFast-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-TmrFast-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Slow.
          05 WS-Tmr-Slow-TEXT       PIC X(256) VALUE 'Tmr-Slow'.
          05 WS-Tmr-Slow-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Slow-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-TmrSlow.
          05 WS-Lbl-TmrSlow-TEXT       PIC X(256) VALUE 'Tmr-Slow  2000 ms'.
          05 WS-Lbl-TmrSlow-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-TmrSlow-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Ticks-Cap.
          05 WS-Lbl-Ticks-Cap-TEXT       PIC X(256) VALUE 'Ticks counted'.
          05 WS-Lbl-Ticks-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Ticks-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Num-Ticks.
          05 WS-Num-Ticks-TEXT       PIC X(256) VALUE 'Num-Ticks'.
          05 WS-Num-Ticks-VISIBLE    PIC 9      VALUE 1.
          05 WS-Num-Ticks-ENABLED    PIC 9      VALUE 1.

       01 WS-Prg-Sweep.
          05 WS-Prg-Sweep-TEXT       PIC X(256) VALUE 'Prg-Sweep'.
          05 WS-Prg-Sweep-VISIBLE    PIC 9      VALUE 1.
          05 WS-Prg-Sweep-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Rule.
          05 WS-Lbl-Rule-TEXT       PIC X(256) VALUE 'A Timer is non-visual: the square in the tray is the designer showing you the object, not something the running form paints.'.
          05 WS-Lbl-Rule-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Rule-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Start.
          05 WS-Btn-Start-TEXT       PIC X(256) VALUE 'Start the fast one'.
          05 WS-Btn-Start-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Start-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Stop.
          05 WS-Btn-Stop-TEXT       PIC X(256) VALUE 'Stop it'.
          05 WS-Btn-Stop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Stop-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Faster.
          05 WS-Btn-Faster-TEXT       PIC X(256) VALUE 'Twice as fast'.
          05 WS-Btn-Faster-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Faster-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Enabled.
          05 WS-Btn-Enabled-TEXT       PIC X(256) VALUE 'Is it running?'.
          05 WS-Btn-Enabled-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Enabled-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "TIMER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "TIMER-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Tmr-Fast"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-FAST--ONTICK"
                       END-EVALUATE
                   WHEN "Tmr-Slow"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-SLOW--ONTICK"
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
                   WHEN "Btn-Faster"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FASTER--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Enabled"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ENABLED--ONCLICK"
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
           INVOKE Tmr-Fast 'SetInterval' USING BY VALUE 250
           INVOKE Tmr-Slow 'SetInterval' USING BY VALUE 2000
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
       PROGRAM-ID. TIMER-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> SetInterval is the method form of writing the property.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Tmr-Fast::SetInterval(250).
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "ready - slow timer already running at " Tmr-Slow::Interval " ms" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM TIMER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TIMER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TIMER-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-FAST--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           ADD 1 TO Num-Ticks::Value.
      *> Sweep the second hand round its own scale.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           ADD 4 TO Prg-Sweep::Value.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Prg-Sweep::Value >= Prg-Sweep::Maximum
      *> EN: EXTENSION - INITIALIZE on a control resets its Value property.
      *> PT: EXTENSAO - INITIALIZE em um controle redefine a propriedade Value dele.
      *> ES: EXTENSION - INITIALIZE sobre un control reinicia su propiedad Value.
      *> FR: EXTENSION - INITIALIZE sur un controle reinitialise sa propriete Value.
      *> JP: EXTENSION - コントロールに対する INITIALIZE は Value プロパティを初期化する。
      *> CN: EXTENSION - 对控件执行 INITIALIZE 会重置其 Value 属性。
               INITIALIZE Prg-Sweep
           END-IF.

           GOBACK.

       END PROGRAM TMR-FAST--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-SLOW--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> A slow heartbeat that only reports; it does not drive anything.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "heartbeat - fast timer has ticked " Num-Ticks::Value " times" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM TMR-SLOW--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-START--ONCLICK IS COMMON PROGRAM.

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
           Tmr-Fast::Start().

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
           Tmr-Fast::Stop().

           GOBACK.

       END PROGRAM BTN-STOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FASTER--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Tmr-Fast::Interval > 60
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
               COMPUTE Tmr-Fast::Interval = Tmr-Fast::Interval / 2
           END-IF.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "interval -> " Tmr-Fast::Interval " ms" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FASTER--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ENABLED--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> Enabled is the timer's own switch, and IsEnabled() reads it.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "fast timer enabled = " Tmr-Fast::IsEnabled() INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-ENABLED--ONCLICK.

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

       END PROGRAM TIMER-FORM.

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
       PROGRAM-ID. SLIDER-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SLIDER-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Slider'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Slider is the input a ProgressBar is the output of: the user drags it, Value follows, and onValueChanged fires at the end of the drag. TickFrequency and TickStyle draw the scale; Orientation stands it upright.'.
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

       01 WS-Lbl-Vol-Cap.
          05 WS-Lbl-Vol-Cap-TEXT       PIC X(256) VALUE 'Volume  (0-100, ticks every 10)'.
          05 WS-Lbl-Vol-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Vol-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Sld-Volume.
          05 WS-Sld-Volume-TEXT       PIC X(256) VALUE 'Sld-Volume'.
          05 WS-Sld-Volume-VISIBLE    PIC 9      VALUE 1.
          05 WS-Sld-Volume-ENABLED    PIC 9      VALUE 1.
          05 WS-Sld-Volume-VALUE      PIC S9(9) VALUE 35.
          05 WS-Sld-Volume-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Sld-Volume-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Sld-Volume-STEP       PIC S9(9) VALUE 1.

       01 WS-Prg-Meter.
          05 WS-Prg-Meter-TEXT       PIC X(256) VALUE 'Prg-Meter'.
          05 WS-Prg-Meter-VISIBLE    PIC 9      VALUE 1.
          05 WS-Prg-Meter-ENABLED    PIC 9      VALUE 1.

       01 WS-Sld-Bass.
          05 WS-Sld-Bass-TEXT       PIC X(256) VALUE 'Sld-Bass'.
          05 WS-Sld-Bass-VISIBLE    PIC 9      VALUE 1.
          05 WS-Sld-Bass-ENABLED    PIC 9      VALUE 1.
          05 WS-Sld-Bass-VALUE      PIC S9(9) VALUE 0.
          05 WS-Sld-Bass-MINIMUM    PIC S9(9) VALUE -12.
          05 WS-Sld-Bass-MAXIMUM    PIC S9(9) VALUE 12.
          05 WS-Sld-Bass-STEP       PIC S9(9) VALUE 10.

       01 WS-Lbl-Read.
          05 WS-Lbl-Read-TEXT       PIC X(256) VALUE '35'.
          05 WS-Lbl-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Read-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Mute.
          05 WS-Btn-Mute-TEXT       PIC X(256) VALUE 'Mute'.
          05 WS-Btn-Mute-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Mute-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Full.
          05 WS-Btn-Full-TEXT       PIC X(256) VALUE 'Full'.
          05 WS-Btn-Full-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Full-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Nudge.
          05 WS-Btn-Nudge-TEXT       PIC X(256) VALUE 'Nudge up'.
          05 WS-Btn-Nudge-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Nudge-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Flat.
          05 WS-Btn-Flat-TEXT       PIC X(256) VALUE 'Flat bass'.
          05 WS-Btn-Flat-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Flat-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SLIDER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SLIDER-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Sld-Volume"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "SLD-VOLUME--ONVALUECHANGED"
                       END-EVALUATE
                   WHEN "Sld-Bass"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "SLD-BASS--ONVALUECHANGED"
                       END-EVALUATE
                   WHEN "Btn-Mute"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MUTE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Full"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FULL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Nudge"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NUDGE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Flat"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FLAT--ONCLICK"
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
       PROGRAM-ID. SLIDER-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - volume " Sld-Volume::Value ", bass " Sld-Bass::Value " dB" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SLIDER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SLIDER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SLIDER-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SLD-VOLUME--ONVALUECHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> The slider drives the meter directly - no data item in between.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Sld-Volume::Value TO Prg-Meter::Value.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Sld-Volume::Value TO Lbl-Read::Caption.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Sld-Volume::Value > 85
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               MOVE "#C4341C" TO Prg-Meter::BarColor
           ELSE
               MOVE "#2C7BE5" TO Prg-Meter::BarColor
           END-IF.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "volume -> " Sld-Volume::Value INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SLD-VOLUME--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SLD-BASS--ONVALUECHANGED IS COMMON PROGRAM.

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
           STRING "bass -> " Sld-Bass::Value " dB" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SLD-BASS--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MUTE--ONCLICK IS COMMON PROGRAM.

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
           MOVE Sld-Volume::Minimum TO Sld-Volume::Value.

           GOBACK.

       END PROGRAM BTN-MUTE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FULL--ONCLICK IS COMMON PROGRAM.

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
           MOVE Sld-Volume::Maximum TO Sld-Volume::Value.

           GOBACK.

       END PROGRAM BTN-FULL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NUDGE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Increment moves by Step; LargeChange is what a page key moves by.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Sld-Volume::Increment().

           GOBACK.

       END PROGRAM BTN-NUDGE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FLAT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - INITIALIZE on a control resets its Value property.
      *> PT: EXTENSAO - INITIALIZE em um controle redefine a propriedade Value dele.
      *> ES: EXTENSION - INITIALIZE sobre un control reinicia su propiedad Value.
      *> FR: EXTENSION - INITIALIZE sur un controle reinitialise sa propriete Value.
      *> JP: EXTENSION - コントロールに対する INITIALIZE は Value プロパティを初期化する。
      *> CN: EXTENSION - 对控件执行 INITIALIZE 会重置其 Value 属性。
           INITIALIZE Sld-Bass.

           GOBACK.

       END PROGRAM BTN-FLAT--ONCLICK.

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

       END PROGRAM SLIDER-FORM.

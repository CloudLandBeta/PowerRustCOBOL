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
       PROGRAM-ID. KNOB-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'KNOB-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Knob'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Knob is a rotary value: drag it round and Value follows, between Minimum and Maximum. Bipolar draws the travel out from the centre rather than from the left, which is what a trim or a pan control wants. DefaultValue is where a reset puts it.'.
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

       01 WS-Knb-Gain.
          05 WS-Knb-Gain-TEXT       PIC X(256) VALUE 'Knb-Gain'.
          05 WS-Knb-Gain-VISIBLE    PIC 9      VALUE 1.
          05 WS-Knb-Gain-ENABLED    PIC 9      VALUE 1.
          05 WS-Knb-Gain-VALUE      PIC S9(9) VALUE 0.
          05 WS-Knb-Gain-MINIMUM    PIC S9(9) VALUE -12.
          05 WS-Knb-Gain-MAXIMUM    PIC S9(9) VALUE 12.
          05 WS-Knb-Gain-STEP       PIC S9(9) VALUE 1.
          05 WS-Knb-Gain-DEFAULT    PIC S9(9) VALUE 0.

       01 WS-Knb-Mix.
          05 WS-Knb-Mix-TEXT       PIC X(256) VALUE 'Knb-Mix'.
          05 WS-Knb-Mix-VISIBLE    PIC 9      VALUE 1.
          05 WS-Knb-Mix-ENABLED    PIC 9      VALUE 1.
          05 WS-Knb-Mix-VALUE      PIC S9(9) VALUE 50.
          05 WS-Knb-Mix-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Knb-Mix-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Knb-Mix-STEP       PIC S9(9) VALUE 1.
          05 WS-Knb-Mix-DEFAULT    PIC S9(9) VALUE 50.

       01 WS-Knb-Tone.
          05 WS-Knb-Tone-TEXT       PIC X(256) VALUE 'Knb-Tone'.
          05 WS-Knb-Tone-VISIBLE    PIC 9      VALUE 1.
          05 WS-Knb-Tone-ENABLED    PIC 9      VALUE 1.
          05 WS-Knb-Tone-VALUE      PIC S9(9) VALUE 70.
          05 WS-Knb-Tone-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Knb-Tone-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Knb-Tone-STEP       PIC S9(9) VALUE 1.
          05 WS-Knb-Tone-DEFAULT    PIC S9(9) VALUE 70.

       01 WS-Lbl-Live-Cap.
          05 WS-Lbl-Live-Cap-TEXT       PIC X(256) VALUE 'Last value moved'.
          05 WS-Lbl-Live-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Live-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Live.
          05 WS-Lbl-Live-TEXT       PIC X(256) VALUE '0'.
          05 WS-Lbl-Live-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Live-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sum.
          05 WS-Lbl-Sum-TEXT       PIC X(256) VALUE '-'.
          05 WS-Lbl-Sum-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sum-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Flat.
          05 WS-Btn-Flat-TEXT       PIC X(256) VALUE 'Flat response'.
          05 WS-Btn-Flat-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Flat-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Boost.
          05 WS-Btn-Boost-TEXT       PIC X(256) VALUE 'Boost gain'.
          05 WS-Btn-Boost-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Boost-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Read.
          05 WS-Btn-Read-TEXT       PIC X(256) VALUE 'Read all three'.
          05 WS-Btn-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Read-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "KNOB-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "KNOB-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Knb-Gain"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "KNB-GAIN--ONVALUECHANGED"
                       END-EVALUATE
                   WHEN "Knb-Mix"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "KNB-MIX--ONVALUECHANGED"
                       END-EVALUATE
                   WHEN "Knb-Tone"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "KNB-TONE--ONVALUECHANGED"
                       END-EVALUATE
                   WHEN "Btn-Flat"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FLAT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Boost"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-BOOST--ONCLICK"
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
       PROGRAM-ID. KNOB-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - gain runs " Knb-Gain::Minimum " .. " Knb-Gain::Maximum
                  " and is bipolar" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM KNOB-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNOB-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM KNOB-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNB-GAIN--ONVALUECHANGED IS COMMON PROGRAM.

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
           STRING Knb-Gain::Label " -> " Knb-Gain::Value INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

      *> Mirror the dial onto the meter so both readings agree.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Knb-Gain::Value TO Lbl-Live::Caption.

           GOBACK.

       END PROGRAM KNB-GAIN--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNB-MIX--ONVALUECHANGED IS COMMON PROGRAM.

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
           STRING Knb-Mix::Label " -> " Knb-Mix::Value INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

      *> Mirror the dial onto the meter so both readings agree.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Knb-Mix::Value TO Lbl-Live::Caption.

           GOBACK.

       END PROGRAM KNB-MIX--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNB-TONE--ONVALUECHANGED IS COMMON PROGRAM.

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
           STRING Knb-Tone::Label " -> " Knb-Tone::Value INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

      *> Mirror the dial onto the meter so both readings agree.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Knb-Tone::Value TO Lbl-Live::Caption.

           GOBACK.

       END PROGRAM KNB-TONE--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FLAT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Put every dial back where the design says it belongs.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Knb-Gain::DefaultValue TO Knb-Gain::Value.
           MOVE Knb-Mix::DefaultValue  TO Knb-Mix::Value.
           MOVE Knb-Tone::DefaultValue TO Knb-Tone::Value.

           GOBACK.

       END PROGRAM BTN-FLAT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-BOOST--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Knb-Gain::Value < Knb-Gain::Maximum
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
               ADD 3 TO Knb-Gain::Value
           END-IF.

           GOBACK.

       END PROGRAM BTN-BOOST--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-READ--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT PIC X(120).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "gain=" Knb-Gain::Value
                  "  mix=" Knb-Mix::Value
                  "  tone=" Knb-Tone::Value INTO WS-TEXT.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-TEXT TO Lbl-Sum::Caption.

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

       END PROGRAM KNOB-FORM.

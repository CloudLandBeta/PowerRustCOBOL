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
       PROGRAM-ID. GAUGE-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'GAUGE-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Gauge'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Gauge displays a measurement. It has NO events of its own by design - nothing about a reading is a user gesture - so your COBOL writes Value and the dial follows. WarningThreshold and CriticalThreshold recolour it as the number crosses them.'.
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

       01 WS-Gau-Cpu.
          05 WS-Gau-Cpu-TEXT       PIC X(256) VALUE 'Gau-Cpu'.
          05 WS-Gau-Cpu-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gau-Cpu-ENABLED    PIC 9      VALUE 1.
          05 WS-Gau-Cpu-VALUE      PIC S9(9) VALUE 22.
          05 WS-Gau-Cpu-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gau-Cpu-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gau-Cpu-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Gau-Mem.
          05 WS-Gau-Mem-TEXT       PIC X(256) VALUE 'Gau-Mem'.
          05 WS-Gau-Mem-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gau-Mem-ENABLED    PIC 9      VALUE 1.
          05 WS-Gau-Mem-VALUE      PIC S9(9) VALUE 18.
          05 WS-Gau-Mem-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gau-Mem-MAXIMUM    PIC S9(9) VALUE 64.
          05 WS-Gau-Mem-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Gau-Disk.
          05 WS-Gau-Disk-TEXT       PIC X(256) VALUE 'Gau-Disk'.
          05 WS-Gau-Disk-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gau-Disk-ENABLED    PIC 9      VALUE 1.
          05 WS-Gau-Disk-VALUE      PIC S9(9) VALUE 41.
          05 WS-Gau-Disk-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gau-Disk-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gau-Disk-STYLE      PIC X(10)  VALUE 'Linear'.

       01 WS-Lbl-Reading.
          05 WS-Lbl-Reading-TEXT       PIC X(256) VALUE '-'.
          05 WS-Lbl-Reading-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Reading-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Load.
          05 WS-Btn-Load-TEXT       PIC X(256) VALUE 'Simulate load'.
          05 WS-Btn-Load-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Load-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Idle.
          05 WS-Btn-Idle-TEXT       PIC X(256) VALUE 'Back to idle'.
          05 WS-Btn-Idle-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Idle-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Crit.
          05 WS-Btn-Crit-TEXT       PIC X(256) VALUE 'Cross critical'.
          05 WS-Btn-Crit-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Crit-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "GAUGE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "GAUGE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Load"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LOAD--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Idle"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-IDLE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Crit"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CRIT--ONCLICK"
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
       PROGRAM-ID. GAUGE-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - " Gau-Cpu::Text " warns at " Gau-Cpu::WarningThreshold
                  " and is critical at " Gau-Cpu::CriticalThreshold INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM GAUGE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GAUGE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM GAUGE-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LOAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> A gauge is written to, never clicked - this is the whole of its API.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           ADD 17 TO Gau-Cpu::Value.
           ADD  6 TO Gau-Mem::Value.
           ADD 11 TO Gau-Disk::Value.
      *> Keep every dial inside its own scale.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Gau-Cpu::Value > Gau-Cpu::Maximum
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               MOVE Gau-Cpu::Maximum TO Gau-Cpu::Value
           END-IF.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Gau-Mem::Value > Gau-Mem::Maximum
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               MOVE Gau-Mem::Maximum TO Gau-Mem::Value
           END-IF.
      *> EN: EXTENSION - a numeric property compares and computes algebraically; no PIC item needed.
      *> PT: EXTENSAO - propriedade numerica compara e calcula algebricamente; sem item PIC.
      *> ES: EXTENSION - la propiedad numerica compara y calcula algebraicamente; sin item PIC.
      *> FR: EXTENSION - une propriete numerique compare et calcule algebriquement; sans item PIC.
      *> JP: EXTENSION - 数値プロパティは代数的に比較・計算できる。中間の PIC 項目は不要。
      *> CN: EXTENSION - 数值属性按代数方式比较与计算；无需中间 PIC 数据项。
           IF Gau-Disk::Value > Gau-Disk::Maximum
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               MOVE Gau-Disk::Maximum TO Gau-Disk::Value
           END-IF.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "cpu=" Gau-Cpu::Value Gau-Cpu::Unit
                  "  mem=" Gau-Mem::Value Gau-Mem::Unit
                  "  disk=" Gau-Disk::Value Gau-Disk::Unit INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-LINE TO Lbl-Reading::Caption.

           GOBACK.

       END PROGRAM BTN-LOAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-IDLE--ONCLICK IS COMMON PROGRAM.

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
           MOVE 22 TO Gau-Cpu::Value.
           MOVE 18 TO Gau-Mem::Value.
           MOVE 41 TO Gau-Disk::Value.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "idle" TO Lbl-Reading::Caption.

           GOBACK.

       END PROGRAM BTN-IDLE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CRIT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Push each dial past its critical mark to see the colour change.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           COMPUTE Gau-Cpu::Value  = Gau-Cpu::CriticalThreshold  + 5.
           COMPUTE Gau-Disk::Value = Gau-Disk::CriticalThreshold + 3.

           GOBACK.

       END PROGRAM BTN-CRIT--ONCLICK.

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

       END PROGRAM GAUGE-FORM.

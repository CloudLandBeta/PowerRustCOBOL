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
       PROGRAM-ID. LINE-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'LINE-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Line'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Line is pure decoration: a rule with a colour, a thickness, a direction and a dash style. It has no events - nothing about a rule is a gesture - so everything about it is set from the properties pane or written from COBOL.'.
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

       01 WS-Lbl-Lin-Solid.
          05 WS-Lbl-Lin-Solid-TEXT       PIC X(256) VALUE 'Solid'.
          05 WS-Lbl-Lin-Solid-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Lin-Solid-ENABLED    PIC 9      VALUE 1.

       01 WS-Lin-Solid.
          05 WS-Lin-Solid-TEXT       PIC X(256) VALUE 'Lin-Solid'.
          05 WS-Lin-Solid-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lin-Solid-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Lin-Dash.
          05 WS-Lbl-Lin-Dash-TEXT       PIC X(256) VALUE 'Dash'.
          05 WS-Lbl-Lin-Dash-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Lin-Dash-ENABLED    PIC 9      VALUE 1.

       01 WS-Lin-Dash.
          05 WS-Lin-Dash-TEXT       PIC X(256) VALUE 'Lin-Dash'.
          05 WS-Lin-Dash-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lin-Dash-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Lin-Dot.
          05 WS-Lbl-Lin-Dot-TEXT       PIC X(256) VALUE 'Dot'.
          05 WS-Lbl-Lin-Dot-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Lin-Dot-ENABLED    PIC 9      VALUE 1.

       01 WS-Lin-Dot.
          05 WS-Lin-Dot-TEXT       PIC X(256) VALUE 'Lin-Dot'.
          05 WS-Lin-Dot-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lin-Dot-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Lin-DashDot.
          05 WS-Lbl-Lin-DashDot-TEXT       PIC X(256) VALUE 'DashDot'.
          05 WS-Lbl-Lin-DashDot-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Lin-DashDot-ENABLED    PIC 9      VALUE 1.

       01 WS-Lin-DashDot.
          05 WS-Lin-DashDot-TEXT       PIC X(256) VALUE 'Lin-DashDot'.
          05 WS-Lin-DashDot-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lin-DashDot-ENABLED    PIC 9      VALUE 1.

       01 WS-Lin-Vert.
          05 WS-Lin-Vert-TEXT       PIC X(256) VALUE 'Lin-Vert'.
          05 WS-Lin-Vert-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lin-Vert-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Vert.
          05 WS-Lbl-Vert-TEXT       PIC X(256) VALUE 'A vertical rule separates two columns.'.
          05 WS-Lbl-Vert-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Vert-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Thick.
          05 WS-Btn-Thick-TEXT       PIC X(256) VALUE 'Thicker'.
          05 WS-Btn-Thick-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Thick-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Thin.
          05 WS-Btn-Thin-TEXT       PIC X(256) VALUE 'Thinner'.
          05 WS-Btn-Thin-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Thin-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Red.
          05 WS-Btn-Red-TEXT       PIC X(256) VALUE 'Recolour'.
          05 WS-Btn-Red-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Red-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Hide.
          05 WS-Btn-Hide-TEXT       PIC X(256) VALUE 'Hide the rules'.
          05 WS-Btn-Hide-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Hide-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Show.
          05 WS-Btn-Show-TEXT       PIC X(256) VALUE 'Show them'.
          05 WS-Btn-Show-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Show-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "LINE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "LINE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Thick"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-THICK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Thin"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-THIN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Red"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RED--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Hide"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HIDE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Show"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SHOW--ONCLICK"
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
       PROGRAM-ID. LINE-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - four dash styles, " Lin-Solid::LineThickness " px to start" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM LINE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LINE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM LINE-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-THICK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> A rule has no events, so a button next to it is how it changes.
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
           ADD 1 TO Lin-Solid::LineThickness.
           ADD 1 TO Lin-Dash::LineThickness.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "solid rule is now " Lin-Solid::LineThickness " px" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-THICK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-THIN--ONCLICK IS COMMON PROGRAM.

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
           IF Lin-Solid::LineThickness > 1
      *> EN: EXTENSION - arithmetic writes straight into the property, which is a receiving field.
      *> PT: EXTENSAO - a aritmetica escreve direto na propriedade, que e um campo receptor.
      *> ES: EXTENSION - la aritmetica escribe directo en la propiedad, que es campo receptor.
      *> FR: EXTENSION - l'arithmetique ecrit directement dans la propriete, champ recepteur.
      *> JP: EXTENSION - 算術演算はプロパティへ直接書き込む。プロパティは受取項目である。
      *> CN: EXTENSION - 算术运算直接写入属性，因为属性是接收项。
               SUBTRACT 1 FROM Lin-Solid::LineThickness
           END-IF.

           GOBACK.

       END PROGRAM BTN-THIN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RED--ONCLICK IS COMMON PROGRAM.

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
           MOVE "#C4341C" TO Lin-Solid::LineColor.
           MOVE "#C4341C" TO Lin-Vert::LineColor.

           GOBACK.

       END PROGRAM BTN-RED--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HIDE--ONCLICK IS COMMON PROGRAM.

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
           Lin-Dot::Hide().
           Lin-DashDot::Hide().

           GOBACK.

       END PROGRAM BTN-HIDE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SHOW--ONCLICK IS COMMON PROGRAM.

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
           Lin-Dot::Show().
           Lin-DashDot::Show().

           GOBACK.

       END PROGRAM BTN-SHOW--ONCLICK.

       END PROGRAM LINE-FORM.

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
       PROGRAM-ID. SHAPE-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SHAPE-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Shape'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(256) VALUE 'A Shape draws one of three silhouettes with its own FillColor, FillStyle, LineColor, LineStyle and LineThickness. Tick the Appearance background gradient and the shading follows the shape itself, not a box drawn round it. Like Line, it has no events.'.
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

       01 WS-Lbl-Shp-Rect.
          05 WS-Lbl-Shp-Rect-TEXT       PIC X(256) VALUE 'Rectangle'.
          05 WS-Lbl-Shp-Rect-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Shp-Rect-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Rect.
          05 WS-Shp-Rect-TEXT       PIC X(256) VALUE 'Shp-Rect'.
          05 WS-Shp-Rect-VISIBLE    PIC 9      VALUE 1.
          05 WS-Shp-Rect-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Shp-Circle.
          05 WS-Lbl-Shp-Circle-TEXT       PIC X(256) VALUE 'Circle'.
          05 WS-Lbl-Shp-Circle-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Shp-Circle-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Circle.
          05 WS-Shp-Circle-TEXT       PIC X(256) VALUE 'Shp-Circle'.
          05 WS-Shp-Circle-VISIBLE    PIC 9      VALUE 1.
          05 WS-Shp-Circle-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Shp-Tri.
          05 WS-Lbl-Shp-Tri-TEXT       PIC X(256) VALUE 'Triangle'.
          05 WS-Lbl-Shp-Tri-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Shp-Tri-ENABLED    PIC 9      VALUE 1.

       01 WS-Shp-Tri.
          05 WS-Shp-Tri-TEXT       PIC X(256) VALUE 'Shp-Tri'.
          05 WS-Shp-Tri-VISIBLE    PIC 9      VALUE 1.
          05 WS-Shp-Tri-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Note.
          05 WS-Lbl-Note-TEXT       PIC X(256) VALUE 'Every shape is one control. Cycling ShapeType from COBOL turns a rectangle into a circle without touching the layout.'.
          05 WS-Lbl-Note-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Note-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cycle.
          05 WS-Btn-Cycle-TEXT       PIC X(256) VALUE 'Cycle the first'.
          05 WS-Btn-Cycle-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cycle-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Gradient.
          05 WS-Btn-Gradient-TEXT       PIC X(256) VALUE 'Gradient fill'.
          05 WS-Btn-Gradient-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Gradient-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Outline.
          05 WS-Btn-Outline-TEXT       PIC X(256) VALUE 'Fat outline'.
          05 WS-Btn-Outline-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Outline-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Plain.
          05 WS-Btn-Plain-TEXT       PIC X(256) VALUE 'Back to plain'.
          05 WS-Btn-Plain-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Plain-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SHAPE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SHAPE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Cycle"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CYCLE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Gradient"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-GRADIENT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Outline"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-OUTLINE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Plain"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PLAIN--ONCLICK"
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
       PROGRAM-ID. SHAPE-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - " Shp-Rect::ShapeType ", " Shp-Circle::ShapeType
                  " and " Shp-Tri::ShapeType INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SHAPE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHAPE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SHAPE-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CYCLE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           EVALUATE Shp-Rect::ShapeType
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
               WHEN "Rectangle" MOVE "Circle"    TO Shp-Rect::ShapeType
               WHEN "Circle"    MOVE "Triangle"  TO Shp-Rect::ShapeType
               WHEN OTHER       MOVE "Rectangle" TO Shp-Rect::ShapeType
           END-EVALUATE.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "first shape is now a " Shp-Rect::ShapeType INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-CYCLE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-GRADIENT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> The Appearance gradient leads over the fill, on all three silhouettes.
      *> EN: EXTENSION - TRUE and FALSE are usable as operands; they are sugar for 1 and 0.
      *> PT: EXTENSAO - TRUE e FALSE servem como operandos; sao acucar sintatico para 1 e 0.
      *> ES: EXTENSION - TRUE y FALSE sirven como operandos; son azucar sintactico para 1 y 0.
      *> FR: EXTENSION - TRUE et FALSE s'emploient comme operandes; sucre syntaxique pour 1 et 0.
      *> JP: EXTENSION - TRUE と FALSE はオペランドとして使える。1 と 0 の糖衣構文である。
      *> CN: EXTENSION - TRUE 与 FALSE 可作为操作数使用；它们是 1 和 0 的语法糖。
           SET Shp-Rect::BackgroundGradientEnabled   TO TRUE.
           SET Shp-Circle::BackgroundGradientEnabled TO TRUE.
           SET Shp-Tri::BackgroundGradientEnabled    TO TRUE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "#7FB2FF" TO Shp-Rect::BackgroundGradientStartColor.
           MOVE "#123A73" TO Shp-Rect::BackgroundGradientEndColor.

           GOBACK.

       END PROGRAM BTN-GRADIENT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-OUTLINE--ONCLICK IS COMMON PROGRAM.

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
           MOVE 6 TO Shp-Circle::LineThickness.
           MOVE "Dash" TO Shp-Circle::LineStyle.

           GOBACK.

       END PROGRAM BTN-OUTLINE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PLAIN--ONCLICK IS COMMON PROGRAM.

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
           SET Shp-Rect::BackgroundGradientEnabled   TO FALSE.
           SET Shp-Circle::BackgroundGradientEnabled TO FALSE.
           SET Shp-Tri::BackgroundGradientEnabled    TO FALSE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE 2 TO Shp-Circle::LineThickness.
           MOVE "Solid" TO Shp-Circle::LineStyle.
           MOVE "Rectangle" TO Shp-Rect::ShapeType.

           GOBACK.

       END PROGRAM BTN-PLAIN--ONCLICK.

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

       END PROGRAM SHAPE-FORM.

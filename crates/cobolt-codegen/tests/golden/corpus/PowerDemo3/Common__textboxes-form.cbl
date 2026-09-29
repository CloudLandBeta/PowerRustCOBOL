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
       PROGRAM-ID. TEXTBOXES-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'TEXTBOXES-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-txt1.
          05 WS-txt1-TEXT       PIC X(256) VALUE 'Field 1'.
          05 WS-txt1-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt1-ENABLED    PIC 9      VALUE 1.
          05 WS-txt1-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt2.
          05 WS-txt2-TEXT       PIC X(256) VALUE 'Field 2'.
          05 WS-txt2-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt2-ENABLED    PIC 9      VALUE 1.
          05 WS-txt2-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt3.
          05 WS-txt3-TEXT       PIC X(256) VALUE 'Field 3'.
          05 WS-txt3-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt3-ENABLED    PIC 9      VALUE 1.
          05 WS-txt3-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt4.
          05 WS-txt4-TEXT       PIC X(256) VALUE 'Field 4'.
          05 WS-txt4-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt4-ENABLED    PIC 9      VALUE 1.
          05 WS-txt4-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt5.
          05 WS-txt5-TEXT       PIC X(256) VALUE 'Field 5'.
          05 WS-txt5-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt5-ENABLED    PIC 9      VALUE 1.
          05 WS-txt5-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt6.
          05 WS-txt6-TEXT       PIC X(256) VALUE 'Field 6'.
          05 WS-txt6-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt6-ENABLED    PIC 9      VALUE 1.
          05 WS-txt6-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt7.
          05 WS-txt7-TEXT       PIC X(256) VALUE 'Field 7'.
          05 WS-txt7-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt7-ENABLED    PIC 9      VALUE 1.
          05 WS-txt7-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt8.
          05 WS-txt8-TEXT       PIC X(256) VALUE 'Field 8'.
          05 WS-txt8-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt8-ENABLED    PIC 9      VALUE 1.
          05 WS-txt8-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt9.
          05 WS-txt9-TEXT       PIC X(256) VALUE 'Field 9'.
          05 WS-txt9-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt9-ENABLED    PIC 9      VALUE 1.
          05 WS-txt9-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt10.
          05 WS-txt10-TEXT       PIC X(256) VALUE 'Field 10'.
          05 WS-txt10-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt10-ENABLED    PIC 9      VALUE 1.
          05 WS-txt10-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt11.
          05 WS-txt11-TEXT       PIC X(256) VALUE 'Field 11'.
          05 WS-txt11-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt11-ENABLED    PIC 9      VALUE 1.
          05 WS-txt11-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt12.
          05 WS-txt12-TEXT       PIC X(256) VALUE 'Field 12'.
          05 WS-txt12-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt12-ENABLED    PIC 9      VALUE 1.
          05 WS-txt12-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt13.
          05 WS-txt13-TEXT       PIC X(256) VALUE 'Field 13'.
          05 WS-txt13-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt13-ENABLED    PIC 9      VALUE 1.
          05 WS-txt13-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt14.
          05 WS-txt14-TEXT       PIC X(256) VALUE 'Field 14'.
          05 WS-txt14-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt14-ENABLED    PIC 9      VALUE 1.
          05 WS-txt14-VALUE      PIC X(256) VALUE SPACES.

       01 WS-txt15.
          05 WS-txt15-TEXT       PIC X(2048) VALUE 'General notes...'.
          05 WS-txt15-VISIBLE    PIC 9      VALUE 1.
          05 WS-txt15-ENABLED    PIC 9      VALUE 1.
          05 WS-txt15-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-lblSummary.
          05 WS-lblSummary-TEXT       PIC X(256) VALUE 'Summary:'.
          05 WS-lblSummary-VISIBLE    PIC 9      VALUE 1.
          05 WS-lblSummary-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Label-1'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-1.
          05 WS-Button-1-TEXT       PIC X(256) VALUE 'Summarize'.
          05 WS-Button-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "TEXTBOXES-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "TEXTBOXES-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "txt1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTextChanged"
                               CALL "TXT1--ONTEXTCHANGED"
                       END-EVALUATE
                   WHEN "txt2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onGotFocus"
                               CALL "TXT2--ONGOTFOCUS"
                       END-EVALUATE
                   WHEN "txt3"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "TXT3--ONCLICK"
                       END-EVALUATE
                   WHEN "txt4"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onEnterPressed"
                               CALL "TXT4--ONENTERPRESSED"
                       END-EVALUATE
                   WHEN "txt5"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onLostFocus"
                               CALL "TXT5--ONLOSTFOCUS"
                       END-EVALUATE
                   WHEN "txt6"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTextChanged"
                               CALL "TXT6--ONTEXTCHANGED"
                       END-EVALUATE
                   WHEN "txt7"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "TXT7--ONCLICK"
                       END-EVALUATE
                   WHEN "txt8"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onEnterPressed"
                               CALL "TXT8--ONENTERPRESSED"
                       END-EVALUATE
                   WHEN "txt9"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onGotFocus"
                               CALL "TXT9--ONGOTFOCUS"
                       END-EVALUATE
                   WHEN "txt10"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTextChanged"
                               CALL "TXT10--ONTEXTCHANGED"
                       END-EVALUATE
                   WHEN "txt11"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "TXT11--ONCLICK"
                       END-EVALUATE
                   WHEN "txt12"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onEnterPressed"
                               CALL "TXT12--ONENTERPRESSED"
                       END-EVALUATE
                   WHEN "txt13"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onGotFocus"
                               CALL "TXT13--ONGOTFOCUS"
                       END-EVALUATE
                   WHEN "txt14"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTextChanged"
                               CALL "TXT14--ONTEXTCHANGED"
                       END-EVALUATE
                   WHEN "txt15"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTextChanged"
                               CALL "TXT15--ONTEXTCHANGED"
                       END-EVALUATE
                   WHEN "Button-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-1--ONCLICK"
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
       PROGRAM-ID. TEXTBOXES-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TEXTBOXES-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TEXTBOXES-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TEXTBOXES-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT1--ONTEXTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-TEXT  PIC X(255).

       PROCEDURE DIVISION.
           MOVE txt1::Text TO WS-TEXT
           MOVE FUNCTION UPPER-CASE(WS-TEXT) TO txt1::Text
           MOVE "Converte o texto para maiúsculas" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT1--ONTEXTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT2--ONGOTFOCUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "#FF0000" TO txt2::ForegroundColor
           MOVE "Altera a cor da fonte para vermelho" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT2--ONGOTFOCUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT3--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-SIZE  PIC S9(4) COMP-5.

       PROCEDURE DIVISION.
           MOVE txt3::FontSize TO WS-SIZE
           SUBTRACT 2 FROM WS-SIZE
           MOVE WS-SIZE TO txt3::FontSize
           MOVE "Diminui o tamanho da fonte em 2 unidades" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT3--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT4--ONENTERPRESSED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "#00FF00" TO txt4::BackgroundColor
           SET  txt4::BackgroundGradientEnabled TO 0
           MOVE "Altera a cor do fundo para verde" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT4--ONENTERPRESSED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT5--ONLOSTFOCUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "#333333" TO txt5::BackgroundColor
           SET  txt5::BackgroundGradientEnabled TO 0
           MOVE "Restaura a cor do fundo para cinza escuro" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT5--ONLOSTFOCUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT6--ONTEXTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-TEXT  PIC X(255).

       PROCEDURE DIVISION.
           MOVE txt6::Text TO WS-TEXT
           IF WS-TEXT NOT = SPACES
               MOVE "#FFFF00" TO txt6::ForegroundColor
           END-IF
           MOVE "Altera a cor da fonte para amarelo se não estiver vazio" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT6--ONTEXTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT7--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "Center" TO txt7::TextAlignment
           MOVE "Centraliza o alinhamento do texto" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT7--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT8--ONENTERPRESSED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET txt8::ShadowEnabled TO 1
           MOVE "Ativa a sombra do controle" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT8--ONENTERPRESSED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT9--ONGOTFOCUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "#FFFF00" TO txt9::BackgroundColor
           SET  txt9::BackgroundGradientEnabled TO 0
           MOVE "Altera a cor do fundo para amarelo" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT9--ONGOTFOCUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT10--ONTEXTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-LEN  PIC S9(4) COMP-5.

       PROCEDURE DIVISION.
           COMPUTE WS-LEN = FUNCTION LENGTH(txt10::Text)
           IF WS-LEN > 10
               SET txt10::Bold TO 1
           ELSE
               SET txt10::Bold TO 0
           END-IF
           MOVE "Ativa o negrito se o texto tiver mais de 10 caracteres" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT10--ONTEXTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT11--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE "Clicado!" TO txt11::Text
           MOVE "Altera o texto para 'Clicado!'" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT11--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT12--ONENTERPRESSED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET txt12::BackgroundGradientEnabled TO 1
           MOVE "Ativa o gradiente de fundo" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT12--ONENTERPRESSED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT13--ONGOTFOCUS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 14 TO txt13::FontSize
           MOVE "Aumenta o tamanho da fonte para 14" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT13--ONGOTFOCUS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT14--ONTEXTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           IF txt14::Text = "OK"
               MOVE "#0000FF" TO txt14::ForegroundColor
           END-IF
           MOVE "Altera a cor da fonte para azul se o texto for 'OK'" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT14--ONTEXTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TXT15--ONTEXTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-FULL-TEXT  PIC X(2000).

       PROCEDURE DIVISION.
           INITIALIZE WS-FULL-TEXT
           STRING txt1::Text " "
                  txt2::Text " "
                  txt3::Text " "
                  txt4::Text " "
                  txt5::Text " "
                  txt6::Text " "
                  txt7::Text " "
                  txt8::Text " "
                  txt9::Text " "
                  txt10::Text " "
                  txt11::Text " "
                  txt12::Text " "
                  txt13::Text " "
                  txt14::Text " "
                  txt15::Text
             DELIMITED BY SIZE INTO WS-FULL-TEXT
           MOVE WS-FULL-TEXT TO lblSummary::Caption
           MOVE "Concatena todos os campos no lblSummary" TO Label-1::Caption.

           GOBACK.

       END PROGRAM TXT15--ONTEXTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-SUMMARY      PIC X(2000).
       01  WS-TEMP-VAL     PIC X(255).

       PROCEDURE DIVISION.
           INITIALIZE WS-SUMMARY.

           *> Collect and concatenate txt1 through txt15
           STRING "txt1=" txt1::Text " "
                  "txt2=" txt2::Text " "
                  "txt3=" txt3::Text " "
                  "txt4=" txt4::Text " "
                  "txt5=" txt5::Text " "
                  "txt6=" txt6::Text " "
                  "txt7=" txt7::Text " "
                  "txt8=" txt8::Text " "
                  "txt9=" txt9::Text " "
                  "txt10=" txt10::Text " "
                  "txt11=" txt11::Text " "
                  "txt12=" txt12::Text " "
                  "txt13=" txt13::Text " "
                  "txt14=" txt14::Text " "
                  "txt15=" txt15::Text
             DELIMITED BY SIZE INTO WS-SUMMARY

           MOVE WS-SUMMARY TO lblSummary::Caption
           MOVE "Summary generated successfully" TO Label-1::Caption.

           GOBACK.

       END PROGRAM BUTTON-1--ONCLICK.

       END PROGRAM TEXTBOXES-FORM.

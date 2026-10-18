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
       PROGRAM-ID. LAYERS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'LAYERS-FORM'.

      *>── Timer: TMR-WORK ──────────────────────────────────────────
       01 WS-TMR-WORK-INTERVAL   PIC 9(8) VALUE 400.
       01 WS-TMR-WORK-ENABLED    PIC 9    VALUE 0.
       01 WS-TMR-WORK-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Layers'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'A layer is a whole sheet above the form. Your COBOL shows and hides it by name, SET HELP-LAYER::Visible TO TRUE, and reads and writes its properties. Every layer starts hidden when the program runs.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-NAME.
          05 WS-LBL-NAME-TEXT       PIC X(256) VALUE 'Customer name'.
          05 WS-LBL-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-NAME-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-NAME.
          05 WS-TXT-NAME-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TXT-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-NAME-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-NAME-VALUE      PIC X(256) VALUE SPACES.

       01 WS-BTN-SAVE.
          05 WS-BTN-SAVE-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-BTN-SAVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-SAVE-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-HELP.
          05 WS-BTN-HELP-TEXT       PIC X(256) VALUE 'Help (a translucent layer)'.
          05 WS-BTN-HELP-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-HELP-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-COUNT.
          05 WS-BTN-COUNT-TEXT       PIC X(256) VALUE 'Button on the form'.
          05 WS-BTN-COUNT-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-COUNT-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-RESULT.
          05 WS-LBL-RESULT-TEXT       PIC X(256) VALUE 'Nothing saved yet.'.
          05 WS-LBL-RESULT-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-RESULT-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-CLICKS-TEXT.
          05 WS-LBL-CLICKS-TEXT-TEXT       PIC X(256) VALUE 'Clicks on that button:'.
          05 WS-LBL-CLICKS-TEXT-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-CLICKS-TEXT-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-CLICKS.
          05 WS-LBL-CLICKS-TEXT       PIC X(256) VALUE '0'.
          05 WS-LBL-CLICKS-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-CLICKS-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-STATE.
          05 WS-LBL-STATE-TEXT       PIC X(256) VALUE ''.
          05 WS-LBL-STATE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-STATE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-TRY1.
          05 WS-LBL-TRY1-TEXT       PIC X(256) VALUE '1. Save with the name empty. ERROR-LAYER shows a red banner. The layer has no background of its own, so only the banner paints and takes clicks: the form stays usable under it.'.
          05 WS-LBL-TRY1-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TRY1-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-TRY2.
          05 WS-LBL-TRY2-TEXT       PIC X(256) VALUE '2. Save with a name. BUSY-LAYER puts a curtain over the form. Its background is opaque (Transparency 0, full alpha), so nothing beneath it can be clicked until the Timer takes it down.'.
          05 WS-LBL-TRY2-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TRY2-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-TRY3.
          05 WS-LBL-TRY3-TEXT       PIC X(256) VALUE '3. Press Help. HELP-LAYER dims the form with a translucent background, so a click beside the card still reaches the button on the form. Drag the slider on the card to 0: the layer turns opaque and the button stops answering.'.
          05 WS-LBL-TRY3-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TRY3-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-TRY4.
          05 WS-LBL-TRY4-TEXT       PIC X(256) VALUE 'In the designer, the tab strip under the canvas holds Form and the three layers. Select a tab to edit that layer alone; its eye shows or hides it while you design. At run time the program decides, as above.'.
          05 WS-LBL-TRY4-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TRY4-ENABLED    PIC 9      VALUE 1.

       01 WS-TMR-WORK.
          05 WS-TMR-WORK-TEXT       PIC X(256) VALUE 'TMR-WORK'.
          05 WS-TMR-WORK-VISIBLE    PIC 9      VALUE 1.
          05 WS-TMR-WORK-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-HELP.
          05 WS-PNL-HELP-TEXT       PIC X(256) VALUE 'PNL-HELP'.
          05 WS-PNL-HELP-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-HELP-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-HELP-HEAD.
          05 WS-LBL-HELP-HEAD-TEXT       PIC X(256) VALUE 'Help'.
          05 WS-LBL-HELP-HEAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-HELP-HEAD-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-HELP-BODY.
          05 WS-LBL-HELP-BODY-TEXT       PIC X(256) VALUE 'This card is a Panel on HELP-LAYER. The layer''s own background dims the form below it, and it takes no clicks itself: while the layer is translucent, a click outside the card passes through to the form. The card, and anything on it, takes its own clicks.'.
          05 WS-LBL-HELP-BODY-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-HELP-BODY-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-DIM.
          05 WS-LBL-DIM-TEXT       PIC X(256) VALUE 'HELP-LAYER::Transparency = 55'.
          05 WS-LBL-DIM-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-DIM-ENABLED    PIC 9      VALUE 1.

       01 WS-SLD-DIM.
          05 WS-SLD-DIM-TEXT       PIC X(256) VALUE 'SLD-DIM'.
          05 WS-SLD-DIM-VISIBLE    PIC 9      VALUE 1.
          05 WS-SLD-DIM-ENABLED    PIC 9      VALUE 1.
          05 WS-SLD-DIM-VALUE      PIC S9(9) VALUE 55.
          05 WS-SLD-DIM-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-SLD-DIM-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-SLD-DIM-STEP       PIC S9(9) VALUE 1.

       01 WS-BTN-HELP-CLOSE.
          05 WS-BTN-HELP-CLOSE-TEXT       PIC X(256) VALUE 'Close'.
          05 WS-BTN-HELP-CLOSE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-HELP-CLOSE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-BUSY.
          05 WS-PNL-BUSY-TEXT       PIC X(256) VALUE 'PNL-BUSY'.
          05 WS-PNL-BUSY-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-BUSY-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-BUSY.
          05 WS-LBL-BUSY-TEXT       PIC X(256) VALUE 'Saving the customer...'.
          05 WS-LBL-BUSY-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-BUSY-ENABLED    PIC 9      VALUE 1.

       01 WS-PBR-WORK.
          05 WS-PBR-WORK-TEXT       PIC X(256) VALUE 'PBR-WORK'.
          05 WS-PBR-WORK-VISIBLE    PIC 9      VALUE 1.
          05 WS-PBR-WORK-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-BUSY-NOTE.
          05 WS-LBL-BUSY-NOTE-TEXT       PIC X(256) VALUE 'BUSY-LAYER is opaque: the form beneath it cannot be clicked while this shows.'.
          05 WS-LBL-BUSY-NOTE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-BUSY-NOTE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-ERROR.
          05 WS-PNL-ERROR-TEXT       PIC X(256) VALUE 'PNL-ERROR'.
          05 WS-PNL-ERROR-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-ERROR-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-ERROR-TEXT.
          05 WS-LBL-ERROR-TEXT-TEXT       PIC X(256) VALUE ''.
          05 WS-LBL-ERROR-TEXT-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-ERROR-TEXT-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-ERROR-OK.
          05 WS-BTN-ERROR-OK-TEXT       PIC X(256) VALUE 'Dismiss'.
          05 WS-BTN-ERROR-OK-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-ERROR-OK-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "LAYERS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "LAYERS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "BTN-SAVE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-HELP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HELP--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-COUNT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-COUNT--ONCLICK"
                       END-EVALUATE
                   WHEN "TMR-WORK"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-WORK--ONTICK"
                       END-EVALUATE
                   WHEN "SLD-DIM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onChange"
                               CALL "SLD-DIM--ONCHANGE"
                       END-EVALUATE
                   WHEN "BTN-HELP-CLOSE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HELP-CLOSE--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-ERROR-OK"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ERROR-OK--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE TMR-WORK 'SetInterval' USING BY VALUE 400
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
       PROGRAM-ID. LAYERS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> LAYERS-SHOW-STATE: writes into LBL-STATE which of the three layers is shown.
           *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible. Every layer
           *>         starts hidden when the program runs, so all three read false here.
           *> Result: LBL-STATE::Caption = "HELP-LAYER false   BUSY-LAYER false   ERROR-LAYER
           *>         false". No data item changes.

           CALL    "LAYERS-SHOW-STATE"

           GOBACK.

       END PROGRAM LAYERS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LAYERS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM LAYERS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           IF    TXT-NAME::Text::Trim()::Len() = 0
               SET     LBL-ERROR-TEXT::Caption   TO    "Type a customer name before saving."
               SET     ERROR-LAYER::Visible      TO    TRUE
           ELSE

               *> The curtain goes up and the Timer starts; the Timer takes it down.

               SET     PBR-WORK::Value           TO    0
               SET     BUSY-LAYER::Visible       TO    TRUE
               SET     TMR-WORK::Enabled         TO    TRUE
           END-IF

           *> LAYERS-SHOW-STATE: refreshes the line that lists which layers are shown.
           *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible.
           *>         the branch above just set ERROR-LAYER or BUSY-LAYER to true.
           *> Result: LBL-STATE::Caption = "HELP-LAYER x   BUSY-LAYER y   ERROR-LAYER z"
           *>         (x, y and z are true or false). No data item changes.

           CALL    "LAYERS-SHOW-STATE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HELP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The layer is shown by name; it stays shown until the program hides it.

           SET     HELP-LAYER::Visible           TO    TRUE

           *> LAYERS-SHOW-STATE: refreshes the line that lists which layers are shown.
           *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible.
           *>         HELP-LAYER was just set to true; the other two keep what they had.
           *> Result: LBL-STATE::Caption = "HELP-LAYER x   BUSY-LAYER y   ERROR-LAYER z"
           *>         (x, y and z are true or false). No data item changes.

           CALL    "LAYERS-SHOW-STATE"

           GOBACK.

       END PROGRAM BTN-HELP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-COUNT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The counter is the label's own caption: a property is a data item.

           ADD     1                             TO    LBL-CLICKS::Caption

           GOBACK.

       END PROGRAM BTN-COUNT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-WORK--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           ADD     25                            TO    PBR-WORK::Value

           IF    PBR-WORK::Value >= 100
               SET     TMR-WORK::Enabled         TO    FALSE
               SET     BUSY-LAYER::Visible       TO    FALSE
               MOVE    SPACES                    TO    LBL-RESULT::Caption
               STRING    "Saved: " TXT-NAME::Text::Trim() DELIMITED BY SIZE INTO LBL-RESULT::Caption

               *> LAYERS-SHOW-STATE: refreshes the line that lists which layers are shown.
               *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible.
               *>         BUSY-LAYER was just set to false.
               *> Result: LBL-STATE::Caption = "HELP-LAYER x   BUSY-LAYER y   ERROR-LAYER z"
               *>         (x, y and z are true or false). No data item changes.

               CALL    "LAYERS-SHOW-STATE"
           END-IF

           GOBACK.

       END PROGRAM TMR-WORK--ONTICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SLD-DIM--ONCHANGE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> A layer has properties like a control. Transparency 0 with a fully opaque colour makes
           *> the layer opaque: it then shields everything below it. Anything above 0 does not.

           MOVE    SLD-DIM::Value                TO    HELP-LAYER::Transparency
           MOVE    SPACES                        TO    LBL-DIM::Caption
           STRING    "HELP-LAYER::Transparency = " SLD-DIM::Value
               DELIMITED BY SIZE INTO LBL-DIM::Caption

           GOBACK.

       END PROGRAM SLD-DIM--ONCHANGE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HELP-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> This button lives ON the layer it hides.

           SET     HELP-LAYER::Visible           TO    FALSE

           *> LAYERS-SHOW-STATE: refreshes the line that lists which layers are shown.
           *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible.
           *>         HELP-LAYER was just set to false; the other two keep what they had.
           *> Result: LBL-STATE::Caption = "HELP-LAYER x   BUSY-LAYER y   ERROR-LAYER z"
           *>         (x, y and z are true or false). No data item changes.

           CALL    "LAYERS-SHOW-STATE"

           GOBACK.

       END PROGRAM BTN-HELP-CLOSE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ERROR-OK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     ERROR-LAYER::Visible          TO    FALSE

           *> LAYERS-SHOW-STATE: refreshes the line that lists which layers are shown.
           *> Uses:   HELP-LAYER::Visible, BUSY-LAYER::Visible and ERROR-LAYER::Visible.
           *>         ERROR-LAYER was just set to false; the other two keep what they had.
           *> Result: LBL-STATE::Caption = "HELP-LAYER x   BUSY-LAYER y   ERROR-LAYER z"
           *>         (x, y and z are true or false). No data item changes.

           CALL    "LAYERS-SHOW-STATE"

           GOBACK.

       END PROGRAM BTN-ERROR-OK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LAYERS-SHOW-STATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> A layer is read by name like a control: Visible reads "true" or "false".
           *> The caption is cleared first, because STRING writes only the characters it
           *> has: a shorter text would otherwise keep the tail of the longer one before it.

           MOVE    SPACES                        TO    LBL-STATE::Caption
           STRING    "HELP-LAYER " HELP-LAYER::Visible "   BUSY-LAYER " BUSY-LAYER::Visible
                     "   ERROR-LAYER " ERROR-LAYER::Visible
               DELIMITED BY SIZE INTO LBL-STATE::Caption

           GOBACK.

       END PROGRAM LAYERS-SHOW-STATE.

       END PROGRAM LAYERS-FORM.

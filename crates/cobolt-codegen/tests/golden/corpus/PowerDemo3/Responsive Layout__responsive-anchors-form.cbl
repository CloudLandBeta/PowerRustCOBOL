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
       PROGRAM-ID. RESPONSIVE-ANCHORS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-ANCHORS-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).
       01 WS-ANCHOR GLOBAL PIC X(40).
       01 WS-PTR GLOBAL PIC 99.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Anchors: every edge, every combination'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'Resize the window. A block keeps its distance to each edge named in its caption. With both edges of an axis it stretches; with neither it keeps its centre at the same fraction of its parent. The window stops before two blocks would touch.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-STAGE.
          05 WS-PNL-STAGE-TEXT       PIC X(256) VALUE 'PNL-STAGE'.
          05 WS-PNL-STAGE-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-STAGE-ENABLED    PIC 9      VALUE 1.

       01 WS-STG-CAP.
          05 WS-STG-CAP-TEXT       PIC X(256) VALUE 'Stage - anchored Top,Bottom,Left,Right, MinWidth 440, MinHeight 300'.
          05 WS-STG-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-STG-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-A-TL.
          05 WS-A-TL-TEXT       PIC X(256) VALUE 'Top,Left'.
          05 WS-A-TL-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-TL-ENABLED    PIC 9      VALUE 1.

       01 WS-A-T.
          05 WS-A-T-TEXT       PIC X(256) VALUE 'Top'.
          05 WS-A-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-T-ENABLED    PIC 9      VALUE 1.

       01 WS-A-TR.
          05 WS-A-TR-TEXT       PIC X(256) VALUE 'Top,Right'.
          05 WS-A-TR-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-TR-ENABLED    PIC 9      VALUE 1.

       01 WS-A-L.
          05 WS-A-L-TEXT       PIC X(256) VALUE 'Left'.
          05 WS-A-L-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-L-ENABLED    PIC 9      VALUE 1.

       01 WS-A-NONE.
          05 WS-A-NONE-TEXT       PIC X(256) VALUE '(none)'.
          05 WS-A-NONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-NONE-ENABLED    PIC 9      VALUE 1.

       01 WS-A-R.
          05 WS-A-R-TEXT       PIC X(256) VALUE 'Right'.
          05 WS-A-R-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-R-ENABLED    PIC 9      VALUE 1.

       01 WS-A-BL.
          05 WS-A-BL-TEXT       PIC X(256) VALUE 'Bottom,Left'.
          05 WS-A-BL-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-BL-ENABLED    PIC 9      VALUE 1.

       01 WS-A-B.
          05 WS-A-B-TEXT       PIC X(256) VALUE 'Bottom'.
          05 WS-A-B-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-B-ENABLED    PIC 9      VALUE 1.

       01 WS-A-BR.
          05 WS-A-BR-TEXT       PIC X(256) VALUE 'Bottom,Right'.
          05 WS-A-BR-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-BR-ENABLED    PIC 9      VALUE 1.

       01 WS-A-LIVE.
          05 WS-A-LIVE-TEXT       PIC X(256) VALUE 'Live: Top,Left'.
          05 WS-A-LIVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-A-LIVE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-LIVE.
          05 WS-PNL-LIVE-TEXT       PIC X(256) VALUE 'PNL-LIVE'.
          05 WS-PNL-LIVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-LIVE-ENABLED    PIC 9      VALUE 1.

       01 WS-LIV-CAP.
          05 WS-LIV-CAP-TEXT       PIC X(256) VALUE 'Pick the live block''s anchors'.
          05 WS-LIV-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-LIV-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-CHK-TOP.
          05 WS-CHK-TOP-TEXT       PIC X(256) VALUE 'Top'.
          05 WS-CHK-TOP-VISIBLE    PIC 9      VALUE 1.
          05 WS-CHK-TOP-ENABLED    PIC 9      VALUE 1.
          05 WS-CHK-TOP-VALUE      PIC X(512) VALUE SPACES.

       01 WS-CHK-BOTTOM.
          05 WS-CHK-BOTTOM-TEXT       PIC X(256) VALUE 'Bottom'.
          05 WS-CHK-BOTTOM-VISIBLE    PIC 9      VALUE 1.
          05 WS-CHK-BOTTOM-ENABLED    PIC 9      VALUE 1.
          05 WS-CHK-BOTTOM-VALUE      PIC X(512) VALUE SPACES.

       01 WS-CHK-LEFT.
          05 WS-CHK-LEFT-TEXT       PIC X(256) VALUE 'Left'.
          05 WS-CHK-LEFT-VISIBLE    PIC 9      VALUE 1.
          05 WS-CHK-LEFT-ENABLED    PIC 9      VALUE 1.
          05 WS-CHK-LEFT-VALUE      PIC X(512) VALUE SPACES.

       01 WS-CHK-RIGHT.
          05 WS-CHK-RIGHT-TEXT       PIC X(256) VALUE 'Right'.
          05 WS-CHK-RIGHT-VISIBLE    PIC 9      VALUE 1.
          05 WS-CHK-RIGHT-ENABLED    PIC 9      VALUE 1.
          05 WS-CHK-RIGHT-VALUE      PIC X(512) VALUE SPACES.

       01 WS-BTN-RESET.
          05 WS-BTN-RESET-TEXT       PIC X(256) VALUE 'Back to Top,Left'.
          05 WS-BTN-RESET-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-RESET-ENABLED    PIC 9      VALUE 1.

       01 WS-LIV-NOTE.
          05 WS-LIV-NOTE-TEXT       PIC X(256) VALUE 'Tick both Left and Right and the live block stretches across the stage. Untick both and it floats at the same fraction of the width. A program changes anchors the same way: MOVE "Top,Right" TO A-LIVE::Anchor.'.
          05 WS-LIV-NOTE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LIV-NOTE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-HORZ.
          05 WS-PNL-HORZ-TEXT       PIC X(256) VALUE 'PNL-HORZ'.
          05 WS-PNL-HORZ-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-HORZ-ENABLED    PIC 9      VALUE 1.

       01 WS-HZ-CAP.
          05 WS-HZ-CAP-TEXT       PIC X(256) VALUE 'Stretch and size limits - this panel is anchored Bottom,Left,Right'.
          05 WS-HZ-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-HZ-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-H-LR.
          05 WS-H-LR-TEXT       PIC X(256) VALUE 'Top,Left,Right - stretches with the window'.
          05 WS-H-LR-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-LR-ENABLED    PIC 9      VALUE 1.
          05 WS-H-LR-VALUE      PIC X(256) VALUE SPACES.

       01 WS-H-MAX.
          05 WS-H-MAX-TEXT       PIC X(256) VALUE 'MaxWidth 560 - stops growing and stays left'.
          05 WS-H-MAX-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-MAX-ENABLED    PIC 9      VALUE 1.
          05 WS-H-MAX-VALUE      PIC X(256) VALUE SPACES.

       01 WS-H-MIN.
          05 WS-H-MIN-TEXT       PIC X(256) VALUE 'MinWidth 240 - the window will not squeeze it below'.
          05 WS-H-MIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-MIN-ENABLED    PIC 9      VALUE 1.
          05 WS-H-MIN-VALUE      PIC X(256) VALUE SPACES.

       01 WS-H-PROP.
          05 WS-H-PROP-TEXT       PIC X(256) VALUE 'Top - proportional'.
          05 WS-H-PROP-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-PROP-ENABLED    PIC 9      VALUE 1.

       01 WS-H-RIGHT.
          05 WS-H-RIGHT-TEXT       PIC X(256) VALUE 'Top,Right'.
          05 WS-H-RIGHT-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-RIGHT-ENABLED    PIC 9      VALUE 1.

       01 WS-H-BR.
          05 WS-H-BR-TEXT       PIC X(256) VALUE 'Bottom,Right'.
          05 WS-H-BR-VISIBLE    PIC 9      VALUE 1.
          05 WS-H-BR-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-ANCHORS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-ANCHORS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-ANCHORS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-ANCHORS-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-ANCHORS-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "CHK-TOP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "CHK-TOP--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "CHK-BOTTOM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "CHK-BOTTOM--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "CHK-LEFT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "CHK-LEFT--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "CHK-RIGHT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onCheckedChanged"
                               CALL "CHK-RIGHT--ONCHECKEDCHANGED"
                       END-EVALUATE
                   WHEN "BTN-RESET"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-RESET--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-ANCHORS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "ANC-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-ANCHORS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-ANCHORS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-ANCHORS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-ANCHORS-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "ANC-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-ANCHORS-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-ANCHORS-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "ANC-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-ANCHORS-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-TOP--ONCHECKEDCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "ANC-APPLY-LIVE"

           GOBACK.

       END PROGRAM CHK-TOP--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-BOTTOM--ONCHECKEDCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "ANC-APPLY-LIVE"

           GOBACK.

       END PROGRAM CHK-BOTTOM--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-LEFT--ONCHECKEDCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "ANC-APPLY-LIVE"

           GOBACK.

       END PROGRAM CHK-LEFT--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-RIGHT--ONCHECKEDCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "ANC-APPLY-LIVE"

           GOBACK.

       END PROGRAM CHK-RIGHT--ONCHECKEDCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-RESET--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO CHK-TOP::Checked
           MOVE 0 TO CHK-BOTTOM::Checked
           MOVE 1 TO CHK-LEFT::Checked
           MOVE 0 TO CHK-RIGHT::Checked
           CALL "ANC-APPLY-LIVE"

           GOBACK.

       END PROGRAM BTN-RESET--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ANC-SHOW-INFO IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Window size, active breakpoint and font scale, as the
      *> layout engine reports them after laying the form out.
           MOVE SPACES TO WS-LINE
           STRING "Window " FUNCTION TRIM(WS-WIN-W)
               " x " FUNCTION TRIM(WS-WIN-H)
               "   |   breakpoint " FUNCTION TRIM(WS-BP)
               "   |   font scale " FUNCTION TRIM(WS-FSC)
               DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO SB-INFO::Items

           GOBACK.

       END PROGRAM ANC-SHOW-INFO.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ANC-APPLY-LIVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Build the anchor text from the four check boxes and give it
      *> to the live block. An empty text means no edge on either axis.
           MOVE SPACES TO WS-ANCHOR
           MOVE 1 TO WS-PTR
           IF CHK-TOP::Checked = 1
               STRING "Top," DELIMITED BY SIZE INTO WS-ANCHOR
                   WITH POINTER WS-PTR
           END-IF
           IF CHK-BOTTOM::Checked = 1
               STRING "Bottom," DELIMITED BY SIZE INTO WS-ANCHOR
                   WITH POINTER WS-PTR
           END-IF
           IF CHK-LEFT::Checked = 1
               STRING "Left," DELIMITED BY SIZE INTO WS-ANCHOR
                   WITH POINTER WS-PTR
           END-IF
           IF CHK-RIGHT::Checked = 1
               STRING "Right," DELIMITED BY SIZE INTO WS-ANCHOR
                   WITH POINTER WS-PTR
           END-IF
           MOVE WS-ANCHOR TO A-LIVE::Anchor
           IF WS-ANCHOR = SPACES
               MOVE "Live: (none)" TO A-LIVE::Caption
           ELSE
               MOVE SPACES TO WS-LINE
               STRING "Live: " WS-ANCHOR DELIMITED BY SIZE INTO WS-LINE
               MOVE WS-LINE TO A-LIVE::Caption
           END-IF

           GOBACK.

       END PROGRAM ANC-APPLY-LIVE.

       END PROGRAM RESPONSIVE-ANCHORS-FORM.

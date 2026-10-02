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
       PROGRAM-ID. RESPONSIVE-FONTS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-FONTS-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Font scaling: the text follows the window'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'FontScaling is Fluid: every font is multiplied by window width / 1000, between 0.8 and 1.6. Use the buttons to switch to Stepped or None, or to pin the factor.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-SAMPLES.
          05 WS-PNL-SAMPLES-TEXT       PIC X(256) VALUE 'PNL-SAMPLES'.
          05 WS-PNL-SAMPLES-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-SAMPLES-ENABLED    PIC 9      VALUE 1.

       01 WS-F-BODY.
          05 WS-F-BODY-TEXT       PIC X(256) VALUE 'Body text at 13 pt - scales with the window'.
          05 WS-F-BODY-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-BODY-ENABLED    PIC 9      VALUE 1.

       01 WS-F-HEAD.
          05 WS-F-HEAD-TEXT       PIC X(256) VALUE 'A heading at 24 pt'.
          05 WS-F-HEAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-HEAD-ENABLED    PIC 9      VALUE 1.

       01 WS-F-FIXED.
          05 WS-F-FIXED-TEXT       PIC X(256) VALUE 'ScaleFont off - always 16 pt'.
          05 WS-F-FIXED-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-FIXED-ENABLED    PIC 9      VALUE 1.

       01 WS-F-MIN.
          05 WS-F-MIN-TEXT       PIC X(256) VALUE 'MinFontSize 14 - never smaller than 14'.
          05 WS-F-MIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-MIN-ENABLED    PIC 9      VALUE 1.

       01 WS-F-MAX.
          05 WS-F-MAX-TEXT       PIC X(256) VALUE 'MaxFontSize 18 - never larger than 18'.
          05 WS-F-MAX-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-MAX-ENABLED    PIC 9      VALUE 1.

       01 WS-F-AUTO.
          05 WS-F-AUTO-TEXT       PIC X(256) VALUE 'AutoSize - measured at the size it paints'.
          05 WS-F-AUTO-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-AUTO-ENABLED    PIC 9      VALUE 1.

       01 WS-F-FIELD.
          05 WS-F-FIELD-TEXT       PIC X(256) VALUE 'Fields scale too'.
          05 WS-F-FIELD-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-FIELD-ENABLED    PIC 9      VALUE 1.
          05 WS-F-FIELD-VALUE      PIC X(256) VALUE SPACES.

       01 WS-F-BUTTON.
          05 WS-F-BUTTON-TEXT       PIC X(256) VALUE 'So do buttons'.
          05 WS-F-BUTTON-VISIBLE    PIC 9      VALUE 1.
          05 WS-F-BUTTON-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-CTRL.
          05 WS-PNL-CTRL-TEXT       PIC X(256) VALUE 'PNL-CTRL'.
          05 WS-PNL-CTRL-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-CTRL-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-CAP.
          05 WS-FC-CAP-TEXT       PIC X(256) VALUE 'From COBOL'.
          05 WS-FC-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-NONE.
          05 WS-FC-NONE-TEXT       PIC X(256) VALUE 'FontScaling None'.
          05 WS-FC-NONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-NONE-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-FLUID.
          05 WS-FC-FLUID-TEXT       PIC X(256) VALUE 'FontScaling Fluid'.
          05 WS-FC-FLUID-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-FLUID-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-STEP.
          05 WS-FC-STEP-TEXT       PIC X(256) VALUE 'FontScaling Stepped'.
          05 WS-FC-STEP-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-STEP-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-PIN14.
          05 WS-FC-PIN14-TEXT       PIC X(256) VALUE 'Pin FontScale 1.4'.
          05 WS-FC-PIN14-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-PIN14-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-PIN08.
          05 WS-FC-PIN08-TEXT       PIC X(256) VALUE 'Pin FontScale 0.8'.
          05 WS-FC-PIN08-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-PIN08-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-AUTO.
          05 WS-FC-AUTO-TEXT       PIC X(256) VALUE 'Unpin (FontScale 0)'.
          05 WS-FC-AUTO-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-AUTO-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-WIDE.
          05 WS-FC-WIDE-TEXT       PIC X(256) VALUE 'Limits 0.5 - 2.0'.
          05 WS-FC-WIDE-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-WIDE-ENABLED    PIC 9      VALUE 1.

       01 WS-FC-NARROW.
          05 WS-FC-NARROW-TEXT       PIC X(256) VALUE 'Limits 0.8 - 1.6'.
          05 WS-FC-NARROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-FC-NARROW-ENABLED    PIC 9      VALUE 1.

       01 WS-FS-NOW.
          05 WS-FS-NOW-TEXT       PIC X(256) VALUE 'Font scale: 1'.
          05 WS-FS-NOW-VISIBLE    PIC 9      VALUE 1.
          05 WS-FS-NOW-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-FONTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-FONTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-FONTS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-FONTS-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-FONTS-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "FC-NONE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-NONE--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-FLUID"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-FLUID--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-STEP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-STEP--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-PIN14"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-PIN14--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-PIN08"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-PIN08--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-AUTO"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-AUTO--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-WIDE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-WIDE--ONCLICK"
                       END-EVALUATE
                   WHEN "FC-NARROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FC-NARROW--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-FONTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FNT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Font scale: " WS-FSC DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO FS-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-FONTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FONTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-FONTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FONTS-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FNT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Font scale: " WS-FSC DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO FS-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-FONTS-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FONTS-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FNT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Font scale: " WS-FSC DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO FS-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-FONTS-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-NONE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "None" TO me::FontScaling

           GOBACK.

       END PROGRAM FC-NONE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-FLUID--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Fluid" TO me::FontScaling

           GOBACK.

       END PROGRAM FC-FLUID--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-STEP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Each breakpoint's factor: Compact 0.85, Medium 1,
      *> Expanded 1.25.
           MOVE "Stepped" TO me::FontScaling

           GOBACK.

       END PROGRAM FC-STEP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-PIN14--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Pins the TOTAL factor: it reads back as written.
           MOVE 1.4 TO me::FontScale

           GOBACK.

       END PROGRAM FC-PIN14--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-PIN08--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0.8 TO me::FontScale

           GOBACK.

       END PROGRAM FC-PIN08--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-AUTO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO me::FontScale

           GOBACK.

       END PROGRAM FC-AUTO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-WIDE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0.5 TO me::MinFontScale
           MOVE 2.0 TO me::MaxFontScale

           GOBACK.

       END PROGRAM FC-WIDE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FC-NARROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0.8 TO me::MinFontScale
           MOVE 1.6 TO me::MaxFontScale

           GOBACK.

       END PROGRAM FC-NARROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FNT-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM FNT-SHOW-INFO.

       END PROGRAM RESPONSIVE-FONTS-FORM.

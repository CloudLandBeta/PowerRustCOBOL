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
       PROGRAM-ID. RESPONSIVE-OBSOLETE-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-OBSOLETE-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-OB-TOOLBAR.
          05 WS-OB-TOOLBAR-TEXT       PIC X(256) VALUE 'OB-TOOLBAR'.
          05 WS-OB-TOOLBAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-TOOLBAR-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-TB-CAP.
          05 WS-OB-TB-CAP-TEXT       PIC X(256) VALUE 'ObsoleteScalingStyle:'.
          05 WS-OB-TB-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-TB-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S0.
          05 WS-OB-S0-TEXT       PIC X(256) VALUE '0'.
          05 WS-OB-S0-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S0-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S1.
          05 WS-OB-S1-TEXT       PIC X(256) VALUE '1'.
          05 WS-OB-S1-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S1-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S2.
          05 WS-OB-S2-TEXT       PIC X(256) VALUE '2'.
          05 WS-OB-S2-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S2-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S3.
          05 WS-OB-S3-TEXT       PIC X(256) VALUE '3'.
          05 WS-OB-S3-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S3-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S4.
          05 WS-OB-S4-TEXT       PIC X(256) VALUE '4'.
          05 WS-OB-S4-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S4-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S5.
          05 WS-OB-S5-TEXT       PIC X(256) VALUE '5'.
          05 WS-OB-S5-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S5-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S6.
          05 WS-OB-S6-TEXT       PIC X(256) VALUE '6'.
          05 WS-OB-S6-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S6-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-S7.
          05 WS-OB-S7-TEXT       PIC X(256) VALUE '7'.
          05 WS-OB-S7-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-S7-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-STYLE.
          05 WS-OB-STYLE-TEXT       PIC X(256) VALUE 'Style 7: Resize, reposition and font'.
          05 WS-OB-STYLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-STYLE-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-L-CODE.
          05 WS-OB-L-CODE-TEXT       PIC X(256) VALUE 'Code'.
          05 WS-OB-L-CODE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-L-CODE-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-CODE.
          05 WS-OB-CODE-TEXT       PIC X(256) VALUE SPACES.
          05 WS-OB-CODE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-CODE-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-CODE-VALUE      PIC X(256) VALUE SPACES.

       01 WS-OB-L-NAME.
          05 WS-OB-L-NAME-TEXT       PIC X(256) VALUE 'Name'.
          05 WS-OB-L-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-L-NAME-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-NAME.
          05 WS-OB-NAME-TEXT       PIC X(256) VALUE SPACES.
          05 WS-OB-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-NAME-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-NAME-VALUE      PIC X(256) VALUE SPACES.

       01 WS-OB-L-ADDRESS.
          05 WS-OB-L-ADDRESS-TEXT       PIC X(256) VALUE 'Address'.
          05 WS-OB-L-ADDRESS-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-L-ADDRESS-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-ADDRESS.
          05 WS-OB-ADDRESS-TEXT       PIC X(256) VALUE SPACES.
          05 WS-OB-ADDRESS-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-ADDRESS-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-ADDRESS-VALUE      PIC X(256) VALUE SPACES.

       01 WS-OB-L-CITY.
          05 WS-OB-L-CITY-TEXT       PIC X(256) VALUE 'City'.
          05 WS-OB-L-CITY-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-L-CITY-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-CITY.
          05 WS-OB-CITY-TEXT       PIC X(256) VALUE SPACES.
          05 WS-OB-CITY-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-CITY-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-CITY-VALUE      PIC X(256) VALUE SPACES.

       01 WS-OB-L-PHONE.
          05 WS-OB-L-PHONE-TEXT       PIC X(256) VALUE 'Phone'.
          05 WS-OB-L-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-L-PHONE-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-PHONE.
          05 WS-OB-PHONE-TEXT       PIC X(256) VALUE SPACES.
          05 WS-OB-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-PHONE-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-PHONE-VALUE      PIC X(256) VALUE SPACES.

       01 WS-OB-TYPE.
          05 WS-OB-TYPE-TEXT       PIC X(256) VALUE 'Customer type'.
          05 WS-OB-TYPE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-TYPE-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-T1.
          05 WS-OB-T1-TEXT       PIC X(256) VALUE 'Retail'.
          05 WS-OB-T1-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-T1-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-T1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-OB-T2.
          05 WS-OB-T2-TEXT       PIC X(256) VALUE 'Wholesale'.
          05 WS-OB-T2-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-T2-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-T2-VALUE      PIC X(512) VALUE SPACES.

       01 WS-OB-T3.
          05 WS-OB-T3-TEXT       PIC X(256) VALUE 'Government'.
          05 WS-OB-T3-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-T3-ENABLED    PIC 9      VALUE 1.
          05 WS-OB-T3-VALUE      PIC X(512) VALUE SPACES.

       01 WS-OB-FIXED.
          05 WS-OB-FIXED-TEXT       PIC X(256) VALUE 'ScaleFont off - this text keeps 13 pt at any style'.
          05 WS-OB-FIXED-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-FIXED-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-NOTE.
          05 WS-OB-NOTE-TEXT       PIC X(256) VALUE 'Drawn at 800 x 560. Make the window 1600 wide and with style 3 every control is twice as wide and twice as far from the left. A font flag scales the text by the smaller of the two ratios.'.
          05 WS-OB-NOTE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-NOTE-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-CANCEL.
          05 WS-OB-CANCEL-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-OB-CANCEL-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-CANCEL-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-OK.
          05 WS-OB-OK-TEXT       PIC X(256) VALUE 'OK'.
          05 WS-OB-OK-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-OK-ENABLED    PIC 9      VALUE 1.

       01 WS-OB-STATUS.
          05 WS-OB-STATUS-TEXT       PIC X(256) VALUE 'Resize the window'.
          05 WS-OB-STATUS-VISIBLE    PIC 9      VALUE 1.
          05 WS-OB-STATUS-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-OBSOLETE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-OBSOLETE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-OBSOLETE-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-OBSOLETE-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-OBSOLETE-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "OB-S0"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S0--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S1--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S2--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S3"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S3--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S4"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S4--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S5"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S5--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S6"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S6--ONCLICK"
                       END-EVALUATE
                   WHEN "OB-S7"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OB-S7--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-OBSOLETE-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "OBS-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-OBSOLETE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-OBSOLETE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-OBSOLETE-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-OBSOLETE-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "OBS-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-OBSOLETE-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-OBSOLETE-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "OBS-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-OBSOLETE-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S0--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO me::ObsoleteScalingStyle
           MOVE "Style 0: None - anchors as usual"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S0--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO me::ObsoleteScalingStyle
           MOVE "Style 1: Resize only"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 2 TO me::ObsoleteScalingStyle
           MOVE "Style 2: Reposition only"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S3--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 3 TO me::ObsoleteScalingStyle
           MOVE "Style 3: Resize and reposition"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S3--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S4--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 4 TO me::ObsoleteScalingStyle
           MOVE "Style 4: Font only"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S4--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S5--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 5 TO me::ObsoleteScalingStyle
           MOVE "Style 5: Resize and font"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S5--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S6--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 6 TO me::ObsoleteScalingStyle
           MOVE "Style 6: Reposition and font"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S6--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OB-S7--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 7 TO me::ObsoleteScalingStyle
           MOVE "Style 7: Resize, reposition and font"
               TO OB-STYLE::Caption

           GOBACK.

       END PROGRAM OB-S7--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OBS-SHOW-INFO IS COMMON PROGRAM.

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
           MOVE WS-LINE TO OB-STATUS::Caption

           GOBACK.

       END PROGRAM OBS-SHOW-INFO.

       END PROGRAM RESPONSIVE-OBSOLETE-FORM.

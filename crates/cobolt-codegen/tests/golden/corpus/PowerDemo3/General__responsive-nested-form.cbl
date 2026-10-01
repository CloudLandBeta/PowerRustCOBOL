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
       PROGRAM-ID. RESPONSIVE-NESTED-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-NESTED-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).
       01 WS-LEFT GLOBAL PIC X VALUE "Y".
       01 WS-PAD GLOBAL PIC X VALUE "Y".
       01 WS-COLS GLOBAL PIC X VALUE "2".

      *>── Form controls ───────────────────────────────────────────────
       01 WS-NS-HEAD.
          05 WS-NS-HEAD-TEXT       PIC X(256) VALUE 'NS-HEAD'.
          05 WS-NS-HEAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-HEAD-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-TITLE.
          05 WS-NS-TITLE-TEXT       PIC X(256) VALUE 'Nested: a Grid form holding Flow, Flex, Tabs and Grid'.
          05 WS-NS-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-T-LEFT.
          05 WS-NS-T-LEFT-TEXT       PIC X(256) VALUE 'Hide the Flow box'.
          05 WS-NS-T-LEFT-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-T-LEFT-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-T-PAD.
          05 WS-NS-T-PAD-TEXT       PIC X(256) VALUE 'Padding 0'.
          05 WS-NS-T-PAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-T-PAD-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-T-COLS.
          05 WS-NS-T-COLS-TEXT       PIC X(256) VALUE 'Inner grid: 1 column'.
          05 WS-NS-T-COLS-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-T-COLS-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-LEFT.
          05 WS-NS-LEFT-TEXT       PIC X(256) VALUE 'Flow inside a Grid cell'.
          05 WS-NS-LEFT-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-LEFT-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-1.
          05 WS-NS-W-1-TEXT       PIC X(256) VALUE 'Grid'.
          05 WS-NS-W-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-2.
          05 WS-NS-W-2-TEXT       PIC X(256) VALUE 'Flow'.
          05 WS-NS-W-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-3.
          05 WS-NS-W-3-TEXT       PIC X(256) VALUE 'Flex'.
          05 WS-NS-W-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-4.
          05 WS-NS-W-4-TEXT       PIC X(256) VALUE 'Tabs'.
          05 WS-NS-W-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-4-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-5.
          05 WS-NS-W-5-TEXT       PIC X(256) VALUE 'Anchors'.
          05 WS-NS-W-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-5-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-6.
          05 WS-NS-W-6-TEXT       PIC X(256) VALUE 'Dock'.
          05 WS-NS-W-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-6-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-7.
          05 WS-NS-W-7-TEXT       PIC X(256) VALUE 'Padding'.
          05 WS-NS-W-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-7-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-8.
          05 WS-NS-W-8-TEXT       PIC X(256) VALUE 'Gap'.
          05 WS-NS-W-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-8-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-9.
          05 WS-NS-W-9-TEXT       PIC X(256) VALUE 'Order'.
          05 WS-NS-W-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-9-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-10.
          05 WS-NS-W-10-TEXT       PIC X(256) VALUE 'Span'.
          05 WS-NS-W-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-10-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-11.
          05 WS-NS-W-11-TEXT       PIC X(256) VALUE 'Fonts'.
          05 WS-NS-W-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-11-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-W-12.
          05 WS-NS-W-12-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-NS-W-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-W-12-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-RIGHT.
          05 WS-NS-RIGHT-TEXT       PIC X(256) VALUE 'NS-RIGHT'.
          05 WS-NS-RIGHT-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-RIGHT-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-TABS.
          05 WS-NS-TABS-TEXT       PIC X(256) VALUE 'NS-TABS'.
          05 WS-NS-TABS-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-TABS-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-P0-TXT.
          05 WS-NS-P0-TXT-TEXT       PIC X(256) VALUE SPACES.
          05 WS-NS-P0-TXT-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-P0-TXT-ENABLED    PIC 9      VALUE 1.
          05 WS-NS-P0-TXT-VALUE      PIC X(256) VALUE SPACES.

       01 WS-NS-P0-LST.
          05 WS-NS-P0-LST-TEXT       PIC X(256) VALUE 'NS-P0-LST'.
          05 WS-NS-P0-LST-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-P0-LST-ENABLED    PIC 9      VALUE 1.
          05 WS-NS-P0-LST-VALUE      PIC X(512) VALUE SPACES.

       01 WS-NS-P0-BTN.
          05 WS-NS-P0-BTN-TEXT       PIC X(256) VALUE 'Bottom,Right'.
          05 WS-NS-P0-BTN-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-P0-BTN-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-PG.
          05 WS-NS-PG-TEXT       PIC X(256) VALUE 'NS-PG'.
          05 WS-NS-PG-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-PG-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G1.
          05 WS-NS-G1-TEXT       PIC X(256) VALUE 'Level 5: Row, Start'.
          05 WS-NS-G1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G1-C-1.
          05 WS-NS-G1-C-1-TEXT       PIC X(256) VALUE 'a'.
          05 WS-NS-G1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G1-C-2.
          05 WS-NS-G1-C-2-TEXT       PIC X(256) VALUE 'b'.
          05 WS-NS-G1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G1-C-3.
          05 WS-NS-G1-C-3-TEXT       PIC X(256) VALUE 'c'.
          05 WS-NS-G1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G2.
          05 WS-NS-G2-TEXT       PIC X(256) VALUE 'Level 5: Row, Center'.
          05 WS-NS-G2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G2-C-1.
          05 WS-NS-G2-C-1-TEXT       PIC X(256) VALUE 'a'.
          05 WS-NS-G2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G2-C-2.
          05 WS-NS-G2-C-2-TEXT       PIC X(256) VALUE 'b'.
          05 WS-NS-G2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G2-C-3.
          05 WS-NS-G2-C-3-TEXT       PIC X(256) VALUE 'c'.
          05 WS-NS-G2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G3.
          05 WS-NS-G3-TEXT       PIC X(256) VALUE 'Level 5: Row, End'.
          05 WS-NS-G3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G3-C-1.
          05 WS-NS-G3-C-1-TEXT       PIC X(256) VALUE 'a'.
          05 WS-NS-G3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G3-C-2.
          05 WS-NS-G3-C-2-TEXT       PIC X(256) VALUE 'b'.
          05 WS-NS-G3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G3-C-3.
          05 WS-NS-G3-C-3-TEXT       PIC X(256) VALUE 'c'.
          05 WS-NS-G3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G4.
          05 WS-NS-G4-TEXT       PIC X(256) VALUE 'Level 5: Row, SpaceEvenly'.
          05 WS-NS-G4-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G4-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G4-C-1.
          05 WS-NS-G4-C-1-TEXT       PIC X(256) VALUE 'a'.
          05 WS-NS-G4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G4-C-2.
          05 WS-NS-G4-C-2-TEXT       PIC X(256) VALUE 'b'.
          05 WS-NS-G4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-G4-C-3.
          05 WS-NS-G4-C-3-TEXT       PIC X(256) VALUE 'c'.
          05 WS-NS-G4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-G4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-FOOT.
          05 WS-NS-FOOT-TEXT       PIC X(256) VALUE 'NS-FOOT'.
          05 WS-NS-FOOT-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-FOOT-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-F1.
          05 WS-NS-F1-TEXT       PIC X(256) VALUE 'Left'.
          05 WS-NS-F1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-F1-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-F2.
          05 WS-NS-F2-TEXT       PIC X(256) VALUE 'JustifyContent = SpaceBetween'.
          05 WS-NS-F2-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-F2-ENABLED    PIC 9      VALUE 1.

       01 WS-NS-F3.
          05 WS-NS-F3-TEXT       PIC X(256) VALUE 'Right'.
          05 WS-NS-F3-VISIBLE    PIC 9      VALUE 1.
          05 WS-NS-F3-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-NESTED-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-NESTED-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-NESTED-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-NESTED-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-NESTED-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "NS-T-LEFT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "NS-T-LEFT--ONCLICK"
                       END-EVALUATE
                   WHEN "NS-T-PAD"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "NS-T-PAD--ONCLICK"
                       END-EVALUATE
                   WHEN "NS-T-COLS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "NS-T-COLS--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-NESTED-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "NST-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-NESTED-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-NESTED-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-NESTED-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-NESTED-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "NST-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-NESTED-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-NESTED-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "NST-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-NESTED-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NS-T-LEFT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> A COBOL write wins over the breakpoint's override. Hidden,
      *> the box is laid out as absent; the right column spans both.
           IF WS-LEFT = "Y"
               MOVE 0 TO NS-LEFT::Visible
               MOVE 1 TO NS-RIGHT::GridColumn
               MOVE 2 TO NS-RIGHT::ColumnSpan
               MOVE "Show the Flow box" TO NS-T-LEFT::Caption
               MOVE "N" TO WS-LEFT
           ELSE
               MOVE 1 TO NS-LEFT::Visible
               MOVE 2 TO NS-RIGHT::GridColumn
               MOVE 1 TO NS-RIGHT::ColumnSpan
               MOVE "Hide the Flow box" TO NS-T-LEFT::Caption
               MOVE "Y" TO WS-LEFT
           END-IF

           GOBACK.

       END PROGRAM NS-T-LEFT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NS-T-PAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           IF WS-PAD = "Y"
               MOVE 0 TO NS-RIGHT::Padding
               MOVE "0" TO NS-RIGHT::PaddingLeft
               MOVE "Padding 12 / 24" TO NS-T-PAD::Caption
               MOVE "N" TO WS-PAD
           ELSE
               MOVE 12 TO NS-RIGHT::Padding
               MOVE "24" TO NS-RIGHT::PaddingLeft
               MOVE "Padding 0" TO NS-T-PAD::Caption
               MOVE "Y" TO WS-PAD
           END-IF

           GOBACK.

       END PROGRAM NS-T-PAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NS-T-COLS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           IF WS-COLS = "2"
               MOVE "1fr" TO NS-PG::GridColumns
               MOVE "Inner grid: 2 columns" TO NS-T-COLS::Caption
               MOVE "1" TO WS-COLS
           ELSE
               MOVE "MinMax(150px, 1fr) MinMax(150px, 1fr)"
                   TO NS-PG::GridColumns
               MOVE "Inner grid: 1 column" TO NS-T-COLS::Caption
               MOVE "2" TO WS-COLS
           END-IF

           GOBACK.

       END PROGRAM NS-T-COLS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NST-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM NST-SHOW-INFO.

       END PROGRAM RESPONSIVE-NESTED-FORM.

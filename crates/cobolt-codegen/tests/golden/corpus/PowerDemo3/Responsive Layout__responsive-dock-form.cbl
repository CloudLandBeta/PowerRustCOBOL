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
       PROGRAM-ID. RESPONSIVE-DOCK-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-DOCK-FORM'.

      *>── DataGrid MAIN-GRID CSV export ──────────────────────────
       01 WS-MAIN-GRID-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-MAIN-GRID-CSV-STATUS  PIC 9       VALUE 0.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).
       01 WS-HDR GLOBAL PIC X VALUE "T".
       01 WS-NAV GLOBAL PIC X VALUE "L".
       01 WS-INS GLOBAL PIC X VALUE "Y".

      *>── Form controls ───────────────────────────────────────────────
       01 WS-PNL-HEADER.
          05 WS-PNL-HEADER-TEXT       PIC X(256) VALUE 'PNL-HEADER'.
          05 WS-PNL-HEADER-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-HEADER-ENABLED    PIC 9      VALUE 1.

       01 WS-HDR-TITLE.
          05 WS-HDR-TITLE-TEXT       PIC X(256) VALUE 'Docking: Top, Bottom, Left, Right, Fill'.
          05 WS-HDR-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-HDR-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-HDR.
          05 WS-BTN-HDR-TEXT       PIC X(256) VALUE 'Header to bottom'.
          05 WS-BTN-HDR-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-HDR-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-NAV.
          05 WS-BTN-NAV-TEXT       PIC X(256) VALUE 'Navigation right'.
          05 WS-BTN-NAV-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-NAV-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-INS.
          05 WS-BTN-INS-TEXT       PIC X(256) VALUE 'Hide inspector'.
          05 WS-BTN-INS-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-INS-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-NAV.
          05 WS-PNL-NAV-TEXT       PIC X(256) VALUE 'PNL-NAV'.
          05 WS-PNL-NAV-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-NAV-ENABLED    PIC 9      VALUE 1.

       01 WS-NAV-CAP.
          05 WS-NAV-CAP-TEXT       PIC X(256) VALUE 'Navigation - Dock Left'.
          05 WS-NAV-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-NAV-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-NAV-LIST.
          05 WS-NAV-LIST-TEXT       PIC X(256) VALUE 'NAV-LIST'.
          05 WS-NAV-LIST-VISIBLE    PIC 9      VALUE 1.
          05 WS-NAV-LIST-ENABLED    PIC 9      VALUE 1.
          05 WS-NAV-LIST-VALUE      PIC X(512) VALUE SPACES.

       01 WS-PNL-INSPECT.
          05 WS-PNL-INSPECT-TEXT       PIC X(256) VALUE 'PNL-INSPECT'.
          05 WS-PNL-INSPECT-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-INSPECT-ENABLED    PIC 9      VALUE 1.

       01 WS-INS-CAP.
          05 WS-INS-CAP-TEXT       PIC X(256) VALUE 'Inspector - Dock Right'.
          05 WS-INS-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-INS-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-INS-APPLY.
          05 WS-INS-APPLY-TEXT       PIC X(256) VALUE 'Apply'.
          05 WS-INS-APPLY-VISIBLE    PIC 9      VALUE 1.
          05 WS-INS-APPLY-ENABLED    PIC 9      VALUE 1.

       01 WS-INS-NOTE.
          05 WS-INS-NOTE-TEXT       PIC X(256) VALUE 'Inside this panel: a caption docked Top, a button docked Bottom, and this text docked Fill. Docking is recursive - every container docks its own children in its own client area, after its Padding.'.
          05 WS-INS-NOTE-VISIBLE    PIC 9      VALUE 1.
          05 WS-INS-NOTE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-MAIN.
          05 WS-PNL-MAIN-TEXT       PIC X(256) VALUE 'PNL-MAIN'.
          05 WS-PNL-MAIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-MAIN-ENABLED    PIC 9      VALUE 1.

       01 WS-MAIN-TOOLS.
          05 WS-MAIN-TOOLS-TEXT       PIC X(256) VALUE 'MAIN-TOOLS'.
          05 WS-MAIN-TOOLS-VISIBLE    PIC 9      VALUE 1.
          05 WS-MAIN-TOOLS-ENABLED    PIC 9      VALUE 1.

       01 WS-TB-NEW.
          05 WS-TB-NEW-TEXT       PIC X(256) VALUE 'New'.
          05 WS-TB-NEW-VISIBLE    PIC 9      VALUE 1.
          05 WS-TB-NEW-ENABLED    PIC 9      VALUE 1.

       01 WS-TB-OPEN.
          05 WS-TB-OPEN-TEXT       PIC X(256) VALUE 'Open'.
          05 WS-TB-OPEN-VISIBLE    PIC 9      VALUE 1.
          05 WS-TB-OPEN-ENABLED    PIC 9      VALUE 1.

       01 WS-TB-SAVE.
          05 WS-TB-SAVE-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-TB-SAVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-TB-SAVE-ENABLED    PIC 9      VALUE 1.

       01 WS-TB-SEARCH.
          05 WS-TB-SEARCH-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TB-SEARCH-VISIBLE    PIC 9      VALUE 1.
          05 WS-TB-SEARCH-ENABLED    PIC 9      VALUE 1.
          05 WS-TB-SEARCH-VALUE      PIC X(256) VALUE SPACES.

       01 WS-MAIN-FOOT.
          05 WS-MAIN-FOOT-TEXT       PIC X(256) VALUE 'Footer - Dock Bottom inside the Fill panel'.
          05 WS-MAIN-FOOT-VISIBLE    PIC 9      VALUE 1.
          05 WS-MAIN-FOOT-ENABLED    PIC 9      VALUE 1.

       01 WS-MAIN-GRID.
          05 WS-MAIN-GRID-TEXT       PIC X(256) VALUE 'MAIN-GRID'.
          05 WS-MAIN-GRID-VISIBLE    PIC 9      VALUE 1.
          05 WS-MAIN-GRID-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-FLOAT.
          05 WS-BTN-FLOAT-TEXT       PIC X(256) VALUE 'Not docked: Bottom,Right'.
          05 WS-BTN-FLOAT-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-FLOAT-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-DOCK-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-DOCK-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-DOCK-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-DOCK-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-DOCK-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "BTN-HDR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-HDR--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-NAV"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NAV--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-INS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-INS--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
       MAIN-GRID-EXPORT-CSV.
      *>    Export MAIN-GRID data to CSV file.  Delimiter: ",". Mode: Filtered.
      *>    Column order and filtered/all rows follow the DataGrid settings.
      *>    Set WS-MAIN-GRID-CSV-PATH to the desired output file path before calling.
           INVOKE MAIN-GRID 'ExportCSV'
               USING BY REFERENCE WS-MAIN-GRID-CSV-PATH
               RETURNING WS-MAIN-GRID-CSV-STATUS
           IF WS-MAIN-GRID-CSV-STATUS NOT = 0
               DISPLAY "CSV export error: " WS-MAIN-GRID-CSV-STATUS
           END-IF.

      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DOCK-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DCK-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-DOCK-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DOCK-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-DOCK-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DOCK-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DCK-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-DOCK-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DOCK-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DCK-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-DOCK-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-HDR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Re-dock the header. It docks before the status bar (it is
      *> earlier in render order), so at the bottom it sits UNDER it.
           IF WS-HDR = "T"
               MOVE "Bottom" TO PNL-HEADER::Dock
               MOVE "Header to top" TO BTN-HDR::Caption
               MOVE "B" TO WS-HDR
           ELSE
               MOVE "Top" TO PNL-HEADER::Dock
               MOVE "Header to bottom" TO BTN-HDR::Caption
               MOVE "T" TO WS-HDR
           END-IF

           GOBACK.

       END PROGRAM BTN-HDR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NAV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Both side panels docked Right: the first in render order
      *> takes the outer edge.
           IF WS-NAV = "L"
               MOVE "Right" TO PNL-NAV::Dock
               MOVE "Navigation left" TO BTN-NAV::Caption
               MOVE "R" TO WS-NAV
           ELSE
               MOVE "Left" TO PNL-NAV::Dock
               MOVE "Navigation right" TO BTN-NAV::Caption
               MOVE "L" TO WS-NAV
           END-IF

           GOBACK.

       END PROGRAM BTN-NAV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-INS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> A hidden docked panel takes no edge: the Fill panel grows.
           IF WS-INS = "Y"
               MOVE 0 TO PNL-INSPECT::Visible
               MOVE "Show inspector" TO BTN-INS::Caption
               MOVE "N" TO WS-INS
           ELSE
               MOVE 1 TO PNL-INSPECT::Visible
               MOVE "Hide inspector" TO BTN-INS::Caption
               MOVE "Y" TO WS-INS
           END-IF

           GOBACK.

       END PROGRAM BTN-INS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DCK-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM DCK-SHOW-INFO.

       END PROGRAM RESPONSIVE-DOCK-FORM.

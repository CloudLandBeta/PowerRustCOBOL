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
       PROGRAM-ID. RESPONSIVE-RUNTIME-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-RUNTIME-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).
       01 WS-E-X GLOBAL PIC -(4)9.
       01 WS-E-Y GLOBAL PIC -(4)9.
       01 WS-E-W GLOBAL PIC Z(4)9.
       01 WS-E-H GLOBAL PIC Z(4)9.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'From COBOL: every layout property is writable while the form runs'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'The stage starts as a Flow. Each button on the right writes one property of the stage or of chip 1, and the layout runs again on the same frame.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-STAGE.
          05 WS-RT-STAGE-TEXT       PIC X(256) VALUE 'RT-STAGE'.
          05 WS-RT-STAGE-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-STAGE-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-1.
          05 WS-RT-1-TEXT       PIC X(256) VALUE 'chip 1'.
          05 WS-RT-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-1-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-2.
          05 WS-RT-2-TEXT       PIC X(256) VALUE 'chip 2'.
          05 WS-RT-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-2-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-3.
          05 WS-RT-3-TEXT       PIC X(256) VALUE 'chip 3'.
          05 WS-RT-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-3-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-4.
          05 WS-RT-4-TEXT       PIC X(256) VALUE 'chip 4'.
          05 WS-RT-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-4-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-5.
          05 WS-RT-5-TEXT       PIC X(256) VALUE 'chip 5'.
          05 WS-RT-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-5-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-CMDS.
          05 WS-RT-CMDS-TEXT       PIC X(256) VALUE 'RT-CMDS'.
          05 WS-RT-CMDS-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-CMDS-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-ABS.
          05 WS-RC-ABS-TEXT       PIC X(256) VALUE 'Absolute'.
          05 WS-RC-ABS-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-ABS-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-ROW.
          05 WS-RC-ROW-TEXT       PIC X(256) VALUE 'Flex row'.
          05 WS-RC-ROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-ROW-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-COL.
          05 WS-RC-COL-TEXT       PIC X(256) VALUE 'Flex column'.
          05 WS-RC-COL-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-COL-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-GRID.
          05 WS-RC-GRID-TEXT       PIC X(256) VALUE 'Grid 3 x 2'.
          05 WS-RC-GRID-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-GRID-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-FLOW.
          05 WS-RC-FLOW-TEXT       PIC X(256) VALUE 'Flow'.
          05 WS-RC-FLOW-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-FLOW-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-CENTER.
          05 WS-RC-CENTER-TEXT       PIC X(256) VALUE 'Justify center'.
          05 WS-RC-CENTER-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-CENTER-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-GAP24.
          05 WS-RC-GAP24-TEXT       PIC X(256) VALUE 'Gap 24'.
          05 WS-RC-GAP24-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-GAP24-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-GAP0.
          05 WS-RC-GAP0-TEXT       PIC X(256) VALUE 'Gap 0'.
          05 WS-RC-GAP0-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-GAP0-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-GROW.
          05 WS-RC-GROW-TEXT       PIC X(256) VALUE 'Chip 1 grows'.
          05 WS-RC-GROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-GROW-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-MAXW.
          05 WS-RC-MAXW-TEXT       PIC X(256) VALUE 'Chip 1 MaxWidth 120'.
          05 WS-RC-MAXW-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-MAXW-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-MOVE.
          05 WS-RC-MOVE-TEXT       PIC X(256) VALUE 'Move chip 1 by 20'.
          05 WS-RC-MOVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-MOVE-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-READ.
          05 WS-RC-READ-TEXT       PIC X(256) VALUE 'Read chip 1'.
          05 WS-RC-READ-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-READ-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-OFF.
          05 WS-RC-OFF-TEXT       PIC X(256) VALUE 'Responsive off'.
          05 WS-RC-OFF-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-OFF-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-ON.
          05 WS-RC-ON-TEXT       PIC X(256) VALUE 'Responsive on'.
          05 WS-RC-ON-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-ON-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-MIN900.
          05 WS-RC-MIN900-TEXT       PIC X(256) VALUE 'MinFormWidth 900'.
          05 WS-RC-MIN900-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-MIN900-ENABLED    PIC 9      VALUE 1.

       01 WS-RC-MIN64.
          05 WS-RC-MIN64-TEXT       PIC X(256) VALUE 'MinFormWidth 64'.
          05 WS-RC-MIN64-VISIBLE    PIC 9      VALUE 1.
          05 WS-RC-MIN64-ENABLED    PIC 9      VALUE 1.

       01 WS-RT-LOG.
          05 WS-RT-LOG-TEXT       PIC X(256) VALUE 'Press a button.'.
          05 WS-RT-LOG-VISIBLE    PIC 9      VALUE 1.
          05 WS-RT-LOG-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-RUNTIME-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-RUNTIME-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-RUNTIME-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-RUNTIME-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-RUNTIME-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "RC-ABS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-ABS--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-ROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-ROW--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-COL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-COL--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-GRID"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-GRID--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-FLOW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-FLOW--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-CENTER"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-CENTER--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-GAP24"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-GAP24--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-GAP0"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-GAP0--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-GROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-GROW--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-MAXW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-MAXW--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-MOVE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-MOVE--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-READ"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-READ--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-OFF"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-OFF--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-ON"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-ON--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-MIN900"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-MIN900--ONCLICK"
                       END-EVALUATE
                   WHEN "RC-MIN64"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RC-MIN64--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-RUNTIME-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "RTM-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-RUNTIME-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-RUNTIME-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-RUNTIME-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-RUNTIME-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "RTM-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-RUNTIME-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-RUNTIME-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "RTM-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-RUNTIME-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-ABS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Absolute" TO RT-STAGE::LayoutMode
           MOVE "Last: Absolute" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-ABS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-ROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Flex" TO RT-STAGE::LayoutMode
           MOVE "Row" TO RT-STAGE::FlexDirection
           MOVE "Last: Flex row" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-ROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-COL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Flex" TO RT-STAGE::LayoutMode
           MOVE "Column" TO RT-STAGE::FlexDirection
           MOVE "Last: Flex column" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-COL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-GRID--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Grid" TO RT-STAGE::LayoutMode
           MOVE "Repeat(3, 1fr)" TO RT-STAGE::GridColumns
           MOVE "1fr 1fr" TO RT-STAGE::GridRows
           MOVE "Last: Grid 3 x 2" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-GRID--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-FLOW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Flow" TO RT-STAGE::LayoutMode
           MOVE "Last: Flow" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-FLOW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-CENTER--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Center" TO RT-STAGE::JustifyContent
           MOVE "Last: Justify center" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-CENTER--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-GAP24--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 24 TO RT-STAGE::Gap
           MOVE "Last: Gap 24" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-GAP24--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-GAP0--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO RT-STAGE::Gap
           MOVE "Last: Gap 0" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-GAP0--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-GROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO RT-1::FlexGrow
           MOVE "Last: Chip 1 grows" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-GROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-MAXW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 120 TO RT-1::MaxWidth
           MOVE "Last: Chip 1 MaxWidth 120" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-MAXW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-MOVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> On a responsive form a geometry write is ON-SCREEN:
      *> what COBOL writes is what it reads back (R38).
           MOVE RT-1::X TO WS-NUM
           ADD 20 TO WS-NUM
           MOVE WS-NUM TO RT-1::X
           MOVE "Last: Move chip 1 by 20" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-MOVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-READ--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Reads the LAID-OUT rectangle, not the designed one.
           MOVE RT-1::X TO WS-NUM
           MOVE WS-NUM TO WS-E-X
           MOVE RT-1::Y TO WS-NUM
           MOVE WS-NUM TO WS-E-Y
           MOVE RT-1::Width TO WS-NUM
           MOVE WS-NUM TO WS-E-W
           MOVE RT-1::Height TO WS-NUM
           MOVE WS-NUM TO WS-E-H
           MOVE SPACES TO WS-LINE
           STRING "Chip 1 is at " FUNCTION TRIM(WS-E-X)
               ", " FUNCTION TRIM(WS-E-Y) ", size "
               FUNCTION TRIM(WS-E-W) " x " FUNCTION TRIM(WS-E-H)
               DELIMITED BY SIZE INTO WS-LINE
           MOVE WS-LINE TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-READ--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-OFF--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Off: the designed size, scrolling - as before 1.80.
           MOVE 0 TO me::Responsive
           MOVE "Last: Responsive off" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-OFF--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-ON--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO me::Responsive
           MOVE "Last: Responsive on" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-ON--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-MIN900--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 900 TO me::MinFormWidth
           MOVE "Last: MinFormWidth 900" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-MIN900--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-MIN64--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 64 TO me::MinFormWidth
           MOVE "Last: MinFormWidth 64" TO RT-LOG::Caption

           GOBACK.

       END PROGRAM RC-MIN64--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RTM-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM RTM-SHOW-INFO.

       END PROGRAM RESPONSIVE-RUNTIME-FORM.

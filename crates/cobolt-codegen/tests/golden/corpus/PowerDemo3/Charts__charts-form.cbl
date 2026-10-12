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
       PROGRAM-ID. CHARTS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'CHARTS-FORM'.

      *>── Chart: BarChart-1 (type: BarChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-BarChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-BarChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-BarChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: LineChart-1 (type: LineChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-LineChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-LineChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-LineChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: PieChart-1 (type: PieChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-PieChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-PieChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-PieChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: AreaChart-1 (type: AreaChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-AreaChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-AreaChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-AreaChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: ScatterChart-1 (type: ScatterChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-ScatterChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-ScatterChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-ScatterChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: DonutChart-1 (type: DonutChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-DonutChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-DonutChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-DonutChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Label-TITLE.
          05 WS-Label-TITLE-TEXT       PIC X(256) VALUE 'Charts - the six types (value changes are animated)'.
          05 WS-Label-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-BarChart-1.
          05 WS-BarChart-1-TEXT       PIC X(256) VALUE 'BarChart-1'.
          05 WS-BarChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-BarChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-LineChart-1.
          05 WS-LineChart-1-TEXT       PIC X(256) VALUE 'LineChart-1'.
          05 WS-LineChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-LineChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-PieChart-1.
          05 WS-PieChart-1-TEXT       PIC X(256) VALUE 'PieChart-1'.
          05 WS-PieChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PieChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AreaChart-1.
          05 WS-AreaChart-1-TEXT       PIC X(256) VALUE 'AreaChart-1'.
          05 WS-AreaChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AreaChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-ScatterChart-1.
          05 WS-ScatterChart-1-TEXT       PIC X(256) VALUE 'ScatterChart-1'.
          05 WS-ScatterChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ScatterChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-DonutChart-1.
          05 WS-DonutChart-1-TEXT       PIC X(256) VALUE 'DonutChart-1'.
          05 WS-DonutChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-DonutChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-LOAD.
          05 WS-Button-CHART-LOAD-TEXT       PIC X(256) VALUE 'Load the first data set'.
          05 WS-Button-CHART-LOAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-LOAD-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-LOAD2.
          05 WS-Button-CHART-LOAD2-TEXT       PIC X(256) VALUE 'Load a second data set'.
          05 WS-Button-CHART-LOAD2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-LOAD2-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-CLEAR.
          05 WS-Button-CHART-CLEAR-TEXT       PIC X(256) VALUE 'Clear every chart'.
          05 WS-Button-CHART-CLEAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-CLEAR-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-MONO.
          05 WS-Button-CHART-MONO-TEXT       PIC X(256) VALUE 'Monochrome on / off'.
          05 WS-Button-CHART-MONO-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-MONO-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-GRADIENT.
          05 WS-Button-CHART-GRADIENT-TEXT       PIC X(256) VALUE 'Monochrome gradient'.
          05 WS-Button-CHART-GRADIENT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-GRADIENT-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-GRID.
          05 WS-Button-CHART-GRID-TEXT       PIC X(256) VALUE 'Grid lines on / off'.
          05 WS-Button-CHART-GRID-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-GRID-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-LEGEND.
          05 WS-Button-CHART-LEGEND-TEXT       PIC X(256) VALUE 'Legend on / off'.
          05 WS-Button-CHART-LEGEND-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-LEGEND-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-HORIZONTAL.
          05 WS-Button-CHART-HORIZONTAL-TEXT       PIC X(256) VALUE 'Bar: horizontal on / off'.
          05 WS-Button-CHART-HORIZONTAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-HORIZONTAL-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-SMOOTH.
          05 WS-Button-CHART-SMOOTH-TEXT       PIC X(256) VALUE 'Line / Area: smooth on / off'.
          05 WS-Button-CHART-SMOOTH-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-SMOOTH-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-SLICES.
          05 WS-Button-CHART-SLICES-TEXT       PIC X(256) VALUE 'Pie / Donut: labels and hole'.
          05 WS-Button-CHART-SLICES-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-SLICES-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-CHART-ANIMMS.
          05 WS-Button-CHART-ANIMMS-TEXT       PIC X(256) VALUE 'Animation: off / 400 / 2000 / 5000 ms'.
          05 WS-Button-CHART-ANIMMS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-CHART-ANIMMS-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-STATUS.
          05 WS-Label-STATUS-TEXT       PIC X(256) VALUE 'Clear() then AddPoint(label, value). AnimateValues is ON at 2000 ms - press the two data buttons alternately to watch the bars travel.'.
          05 WS-Label-STATUS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-STATUS-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "CHARTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CHARTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Button-CHART-LOAD"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-LOAD--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-LOAD2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-LOAD2--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-CLEAR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-CLEAR--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-MONO"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-MONO--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-GRADIENT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-GRADIENT--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-GRID"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-GRID--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-LEGEND"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-LEGEND--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-HORIZONTAL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-HORIZONTAL--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-SMOOTH"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-SMOOTH--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-SLICES"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-SLICES--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-CHART-ANIMMS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-CHART-ANIMMS--ONCLICK"
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
      *> ── Chart INVOKE verb paragraphs ─────────────────────────────────

       BarChart-1-SET-TABLE.
      *>    Bind a COBOL table to BarChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use BarChart-1-ADD-POINT.
           CONTINUE.

       BarChart-1-ADD-POINT.
      *>    Append a single data point to BarChart-1.
      *>    Usage: INVOKE BarChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "BarChart-1" WS-BarChart-1-SELECTED-LBL WS-BarChart-1-SELECTED-VAL )
           CONTINUE.

       BarChart-1-CLEAR.
      *>    Remove all data series from BarChart-1.
      *>    Usage: INVOKE BarChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "BarChart-1" )
           CONTINUE.

       BarChart-1-REFRESH.
      *>    Force BarChart-1 to redraw with current data.
      *>    Usage: INVOKE BarChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "BarChart-1" )
           CONTINUE.

       LineChart-1-SET-TABLE.
      *>    Bind a COBOL table to LineChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use LineChart-1-ADD-POINT.
           CONTINUE.

       LineChart-1-ADD-POINT.
      *>    Append a single data point to LineChart-1.
      *>    Usage: INVOKE LineChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "LineChart-1" WS-LineChart-1-SELECTED-LBL WS-LineChart-1-SELECTED-VAL )
           CONTINUE.

       LineChart-1-CLEAR.
      *>    Remove all data series from LineChart-1.
      *>    Usage: INVOKE LineChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "LineChart-1" )
           CONTINUE.

       LineChart-1-REFRESH.
      *>    Force LineChart-1 to redraw with current data.
      *>    Usage: INVOKE LineChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "LineChart-1" )
           CONTINUE.

       PieChart-1-SET-TABLE.
      *>    Bind a COBOL table to PieChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use PieChart-1-ADD-POINT.
           CONTINUE.

       PieChart-1-ADD-POINT.
      *>    Append a single data point to PieChart-1.
      *>    Usage: INVOKE PieChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "PieChart-1" WS-PieChart-1-SELECTED-LBL WS-PieChart-1-SELECTED-VAL )
           CONTINUE.

       PieChart-1-CLEAR.
      *>    Remove all data series from PieChart-1.
      *>    Usage: INVOKE PieChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "PieChart-1" )
           CONTINUE.

       PieChart-1-REFRESH.
      *>    Force PieChart-1 to redraw with current data.
      *>    Usage: INVOKE PieChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "PieChart-1" )
           CONTINUE.

       AreaChart-1-SET-TABLE.
      *>    Bind a COBOL table to AreaChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use AreaChart-1-ADD-POINT.
           CONTINUE.

       AreaChart-1-ADD-POINT.
      *>    Append a single data point to AreaChart-1.
      *>    Usage: INVOKE AreaChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "AreaChart-1" WS-AreaChart-1-SELECTED-LBL WS-AreaChart-1-SELECTED-VAL )
           CONTINUE.

       AreaChart-1-CLEAR.
      *>    Remove all data series from AreaChart-1.
      *>    Usage: INVOKE AreaChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "AreaChart-1" )
           CONTINUE.

       AreaChart-1-REFRESH.
      *>    Force AreaChart-1 to redraw with current data.
      *>    Usage: INVOKE AreaChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "AreaChart-1" )
           CONTINUE.

       ScatterChart-1-SET-TABLE.
      *>    Bind a COBOL table to ScatterChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use ScatterChart-1-ADD-POINT.
           CONTINUE.

       ScatterChart-1-ADD-POINT.
      *>    Append a single data point to ScatterChart-1.
      *>    Usage: INVOKE ScatterChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "ScatterChart-1" WS-ScatterChart-1-SELECTED-LBL WS-ScatterChart-1-SELECTED-VAL )
           CONTINUE.

       ScatterChart-1-CLEAR.
      *>    Remove all data series from ScatterChart-1.
      *>    Usage: INVOKE ScatterChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "ScatterChart-1" )
           CONTINUE.

       ScatterChart-1-REFRESH.
      *>    Force ScatterChart-1 to redraw with current data.
      *>    Usage: INVOKE ScatterChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "ScatterChart-1" )
           CONTINUE.

       DonutChart-1-SET-TABLE.
      *>    Bind a COBOL table to DonutChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use DonutChart-1-ADD-POINT.
           CONTINUE.

       DonutChart-1-ADD-POINT.
      *>    Append a single data point to DonutChart-1.
      *>    Usage: INVOKE DonutChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "DonutChart-1" WS-DonutChart-1-SELECTED-LBL WS-DonutChart-1-SELECTED-VAL )
           CONTINUE.

       DonutChart-1-CLEAR.
      *>    Remove all data series from DonutChart-1.
      *>    Usage: INVOKE DonutChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "DonutChart-1" )
           CONTINUE.

       DonutChart-1-REFRESH.
      *>    Force DonutChart-1 to redraw with current data.
      *>    Usage: INVOKE DonutChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "DonutChart-1" )
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHARTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Fill every chart as the form opens, so it never shows the sample
           *> preview before the first click.

           BarChart-1::Clear()
           BarChart-1::AddPoint("Q1", 128)
           BarChart-1::AddPoint("Q2", 174)
           BarChart-1::AddPoint("Q3", 151)
           BarChart-1::AddPoint("Q4", 209)

           LineChart-1::Clear()
           LineChart-1::AddPoint("Jan", 42)
           LineChart-1::AddPoint("Feb", 55)
           LineChart-1::AddPoint("Mar", 48)
           LineChart-1::AddPoint("Apr", 71)
           LineChart-1::AddPoint("May", 66)
           LineChart-1::AddPoint("Jun", 88)

           PieChart-1::Clear()
           PieChart-1::AddPoint("North", 34)
           PieChart-1::AddPoint("South", 21)
           PieChart-1::AddPoint("East", 28)
           PieChart-1::AddPoint("West", 17)

           AreaChart-1::Clear()
           AreaChart-1::AddPoint("Wk1", 120)
           AreaChart-1::AddPoint("Wk2", 260)
           AreaChart-1::AddPoint("Wk3", 415)
           AreaChart-1::AddPoint("Wk4", 590)
           AreaChart-1::AddPoint("Wk5", 720)

           ScatterChart-1::Clear()
           ScatterChart-1::AddPoint("0.4", 12)
           ScatterChart-1::AddPoint("0.9", 19)
           ScatterChart-1::AddPoint("1.3", 26)
           ScatterChart-1::AddPoint("1.8", 24)
           ScatterChart-1::AddPoint("2.4", 37)
           ScatterChart-1::AddPoint("3.1", 41)

           DonutChart-1::Clear()
           DonutChart-1::AddPoint("Used", 68)
           DonutChart-1::AddPoint("Free", 32)

           DISPLAY    "charts loaded".

           GOBACK.

       END PROGRAM CHARTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHARTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CHARTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-LOAD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Every chart takes its data the same way: Clear() drops what it
           *> holds, then one AddPoint(label, value) per point. Each AddPoint
           *> pushes the WHOLE series to the renderer, so the chart redraws as it
           *> fills - there is no separate "commit" step.
           *> A chart with NO data draws a representative sample instead, which is
           *> why the designer canvas never looks empty.

           BarChart-1::Clear()
           BarChart-1::AddPoint("Q1", 128)
           BarChart-1::AddPoint("Q2", 174)
           BarChart-1::AddPoint("Q3", 151)
           BarChart-1::AddPoint("Q4", 209)

           LineChart-1::Clear()
           LineChart-1::AddPoint("Jan", 42)
           LineChart-1::AddPoint("Feb", 55)
           LineChart-1::AddPoint("Mar", 48)
           LineChart-1::AddPoint("Apr", 71)
           LineChart-1::AddPoint("May", 66)
           LineChart-1::AddPoint("Jun", 88)

           PieChart-1::Clear()
           PieChart-1::AddPoint("North", 34)
           PieChart-1::AddPoint("South", 21)
           PieChart-1::AddPoint("East", 28)
           PieChart-1::AddPoint("West", 17)

           AreaChart-1::Clear()
           AreaChart-1::AddPoint("Wk1", 120)
           AreaChart-1::AddPoint("Wk2", 260)
           AreaChart-1::AddPoint("Wk3", 415)
           AreaChart-1::AddPoint("Wk4", 590)
           AreaChart-1::AddPoint("Wk5", 720)

           ScatterChart-1::Clear()
           ScatterChart-1::AddPoint("0.4", 12)
           ScatterChart-1::AddPoint("0.9", 19)
           ScatterChart-1::AddPoint("1.3", 26)
           ScatterChart-1::AddPoint("1.8", 24)
           ScatterChart-1::AddPoint("2.4", 37)
           ScatterChart-1::AddPoint("3.1", 41)

           DonutChart-1::Clear()
           DonutChart-1::AddPoint("Used", 68)
           DonutChart-1::AddPoint("Free", 32)

           DISPLAY    "charts loaded".

           GOBACK.

       END PROGRAM BUTTON-CHART-LOAD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-LOAD2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The same six charts, different numbers. Values are auto-scaled into
           *> the plot, so nothing here sets a maximum: the tallest point defines
           *> the top of the band.

           BarChart-1::Clear()
           BarChart-1::AddPoint("Q1", 96)
           BarChart-1::AddPoint("Q2", 88)
           BarChart-1::AddPoint("Q3", 143)
           BarChart-1::AddPoint("Q4", 261)

           LineChart-1::Clear()
           LineChart-1::AddPoint("Jan", 91)
           LineChart-1::AddPoint("Feb", 77)
           LineChart-1::AddPoint("Mar", 64)
           LineChart-1::AddPoint("Apr", 52)
           LineChart-1::AddPoint("May", 58)
           LineChart-1::AddPoint("Jun", 40)

           PieChart-1::Clear()
           PieChart-1::AddPoint("North", 12)
           PieChart-1::AddPoint("South", 44)
           PieChart-1::AddPoint("East", 19)
           PieChart-1::AddPoint("West", 25)

           AreaChart-1::Clear()
           AreaChart-1::AddPoint("Wk1", 80)
           AreaChart-1::AddPoint("Wk2", 140)
           AreaChart-1::AddPoint("Wk3", 190)
           AreaChart-1::AddPoint("Wk4", 205)
           AreaChart-1::AddPoint("Wk5", 212)

           ScatterChart-1::Clear()
           ScatterChart-1::AddPoint("0.4", 38)
           ScatterChart-1::AddPoint("0.9", 31)
           ScatterChart-1::AddPoint("1.3", 33)
           ScatterChart-1::AddPoint("1.8", 18)
           ScatterChart-1::AddPoint("2.4", 14)
           ScatterChart-1::AddPoint("3.1", 9)

           DonutChart-1::Clear()
           DonutChart-1::AddPoint("Used", 23)
           DonutChart-1::AddPoint("Free", 77)

           DISPLAY    "charts loaded".

           GOBACK.

       END PROGRAM BUTTON-CHART-LOAD2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-CLEAR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Clear() drops the pushed series. The renderer falls back to its
           *> representative sample until new points arrive - an empty chart and an
           *> unpopulated one look the same on purpose.

           BarChart-1::Clear()
           LineChart-1::Clear()
           PieChart-1::Clear()
           AreaChart-1::Clear()
           ScatterChart-1::Clear()
           DonutChart-1::Clear()

           SET     Label-STATUS::Caption         TO    "cleared - the sample preview is back"
           DISPLAY    "charts cleared".

           GOBACK.

       END PROGRAM BUTTON-CHART-CLEAR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-MONO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Monochrome draws every element in tonal variations of one base
           *> colour (MonochromeColor) instead of the SeriesColors palette.

           SET     WS-FLAG                       TO    BarChart-1::Monochrome
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF

           SET     BarChart-1::Monochrome        TO    FUNCTION TRIM(WS-FLAG)
           SET     LineChart-1::Monochrome       TO    FUNCTION TRIM(WS-FLAG)
           SET     PieChart-1::Monochrome        TO    FUNCTION TRIM(WS-FLAG)
           SET     AreaChart-1::Monochrome       TO    FUNCTION TRIM(WS-FLAG)
           SET     ScatterChart-1::Monochrome    TO    FUNCTION TRIM(WS-FLAG)
           SET     DonutChart-1::Monochrome      TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "Monochrome = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-MONO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-GRADIENT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> With the gradient on, each element shades from about 20 per cent
           *> lighter at the top left to 20 per cent darker at the bottom right.
           *> It only shows while Monochrome is on.

           SET     WS-FLAG                       TO    BarChart-1::MonochromeGradient
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF

           SET     BarChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     LineChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     PieChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     AreaChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     ScatterChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     DonutChart-1::MonochromeGradient
                                                 TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "MonochromeGradient = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-GRADIENT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-GRID--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The grid, the legend, the two axis lines and the tooltips are four
           *> independent switches - turning the grid off leaves the axes.

           SET     WS-FLAG                       TO    BarChart-1::ShowGridLines
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF

           SET     BarChart-1::ShowGridLines     TO    FUNCTION TRIM(WS-FLAG)
           SET     LineChart-1::ShowGridLines    TO    FUNCTION TRIM(WS-FLAG)
           SET     PieChart-1::ShowGridLines     TO    FUNCTION TRIM(WS-FLAG)
           SET     AreaChart-1::ShowGridLines    TO    FUNCTION TRIM(WS-FLAG)
           SET     ScatterChart-1::ShowGridLines TO    FUNCTION TRIM(WS-FLAG)
           SET     DonutChart-1::ShowGridLines   TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "ShowGridLines = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-GRID--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-LEGEND--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> ShowLegend is independent of ShowLabels on the pie and the donut:
           *> one names the series, the other writes on the slices.

           SET     WS-FLAG                       TO    BarChart-1::ShowLegend
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF

           SET     BarChart-1::ShowLegend        TO    FUNCTION TRIM(WS-FLAG)
           SET     LineChart-1::ShowLegend       TO    FUNCTION TRIM(WS-FLAG)
           SET     PieChart-1::ShowLegend        TO    FUNCTION TRIM(WS-FLAG)
           SET     AreaChart-1::ShowLegend       TO    FUNCTION TRIM(WS-FLAG)
           SET     ScatterChart-1::ShowLegend    TO    FUNCTION TRIM(WS-FLAG)
           SET     DonutChart-1::ShowLegend      TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "ShowLegend = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-LEGEND--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-HORIZONTAL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Horizontal is a BAR chart property - the other five do not have it.
           *> Stacked and BarCornerRadius are its companions.

           SET     WS-FLAG                       TO    BarChart-1::Horizontal
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF
           SET     BarChart-1::Horizontal        TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "BarChart-1 Horizontal = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-HORIZONTAL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-SMOOTH--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FLAG                                PIC    X(8).
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(64).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Smooth, ShowPoints and PointRadius belong to the LINE and AREA
           *> charts. Off, the line is drawn as straight segments between points.

           SET     WS-FLAG                       TO    LineChart-1::Smooth
           IF    FUNCTION TRIM(WS-FLAG) = "true"
               SET     WS-FLAG                   TO    "false"
           ELSE
               SET     WS-FLAG                   TO    "true"
           END-IF

           SET     LineChart-1::Smooth           TO    FUNCTION TRIM(WS-FLAG)
           SET     AreaChart-1::Smooth           TO    FUNCTION TRIM(WS-FLAG)

           SET     WS-MSG                        TO    "Smooth = "
           STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-FLAG) INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-SMOOTH--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-SLICES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-FMT                                 PIC    X(12).
       01 WS-HOLE                                PIC    9(3).
       01 WS-HOLE-TEXT                           PIC    ZZ9.
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(80).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> LabelFormat is percent | value | label, and InnerRadius is the
           *> donut's hole as a percentage of the outer radius. Each press moves
           *> both on one step.

           SET     WS-FMT                        TO    PieChart-1::LabelFormat
           EVALUATE    FUNCTION TRIM(WS-FMT)
               WHEN "percent" MOVE "value" TO WS-FMT
               WHEN "value" MOVE "label" TO WS-FMT
               WHEN OTHER MOVE "percent" TO WS-FMT
           END-EVALUATE

           SET     PieChart-1::LabelFormat       TO    FUNCTION TRIM(WS-FMT)
           SET     DonutChart-1::LabelFormat     TO    FUNCTION TRIM(WS-FMT)

           SET     WS-HOLE                       TO    DonutChart-1::InnerRadius
           EVALUATE    WS-HOLE
               WHEN 40 MOVE 60 TO WS-HOLE
               WHEN 60 MOVE 20 TO WS-HOLE
               WHEN OTHER MOVE 40 TO WS-HOLE
           END-EVALUATE
           SET     DonutChart-1::InnerRadius     TO    WS-HOLE

           SET     WS-HOLE-TEXT                  TO    WS-HOLE
           SET     WS-MSG                        TO    "LabelFormat "

           STRING    FUNCTION TRIM(WS-MSG)
               FUNCTION TRIM(WS-FMT)
               ", InnerRadius "
               FUNCTION TRIM(WS-HOLE-TEXT)
               INTO WS-OUT
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-SLICES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-CHART-ANIMMS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-ON                                  PIC    X(8).
       01 WS-MS                                  PIC    9(5).
       01 WS-MS-TEXT                             PIC    ZZZZ9.
       01 WS-MSG                                 PIC    X(32).
       01 WS-OUT                                 PIC    X(80).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> AnimateValues travels from the values the chart IS showing to the
           *> ones just pushed, over AnimationDuration, for the WHOLE series at once
           *> - so a chart settles in the same time with four points or forty.
           *> The floor is 250 ms: below that the eye reads a jump, so a smaller
           *> number is raised rather than honoured as written.
           *> The FIRST fill is never animated, and could not be. A plot auto-scales
           *> to its own largest value, so a series rising uniformly from zero would
           *> paint the same bars the whole way up. Only a change in how the values
           *> relate to EACH OTHER can be seen - press the two data buttons
           *> alternately to see it.

           SET     WS-MS                         TO    BarChart-1::AnimationDuration
           SET     WS-ON                         TO    BarChart-1::AnimateValues

           *> One button steps the animation: 400 ms, 2000 ms, 5000 ms, then off,
           *> then 400 ms again. WHEN OTHER is the 5000 ms step (or any duration
           *> set elsewhere) and is the one that switches animation off.

           IF    FUNCTION TRIM(WS-ON) NOT = "true"
               SET     WS-ON                     TO    "true"
               SET     WS-MS                     TO    400
           ELSE
               EVALUATE    WS-MS
                   WHEN 400 MOVE 2000 TO WS-MS
                   WHEN 2000 MOVE 5000 TO WS-MS
                   WHEN OTHER
                       SET     WS-ON             TO    "false"
                       SET     WS-MS             TO    2000
               END-EVALUATE
           END-IF

           SET     BarChart-1::AnimateValues     TO    FUNCTION TRIM(WS-ON)
           SET     BarChart-1::AnimationDuration TO    WS-MS
           SET     LineChart-1::AnimateValues    TO    FUNCTION TRIM(WS-ON)
           SET     LineChart-1::AnimationDuration
                                                 TO    WS-MS

           SET     PieChart-1::AnimateValues     TO    FUNCTION TRIM(WS-ON)
           SET     PieChart-1::AnimationDuration TO    WS-MS
           SET     AreaChart-1::AnimateValues    TO    FUNCTION TRIM(WS-ON)
           SET     AreaChart-1::AnimationDuration
                                                 TO    WS-MS

           SET     ScatterChart-1::AnimateValues TO    FUNCTION TRIM(WS-ON)
           SET     ScatterChart-1::AnimationDuration
                                                 TO    WS-MS

           SET     DonutChart-1::AnimateValues   TO    FUNCTION TRIM(WS-ON)
           SET     DonutChart-1::AnimationDuration
                                                 TO    WS-MS

           SET     WS-MS-TEXT                    TO    WS-MS
           IF    FUNCTION TRIM(WS-ON) = "true"
               SET     WS-MSG                    TO    "AnimateValues on, "
               STRING    FUNCTION TRIM(WS-MSG) FUNCTION TRIM(WS-MS-TEXT) " ms" INTO WS-OUT
           ELSE
               SET     WS-OUT                    TO    "AnimateValues off - data changes are instant"
           END-IF
           SET     Label-STATUS::Caption         TO    FUNCTION TRIM(WS-OUT)
           DISPLAY    FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-CHART-ANIMMS--ONCLICK.

       END PROGRAM CHARTS-FORM.

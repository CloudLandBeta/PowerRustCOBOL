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
       PROGRAM-ID. CHART-3D.

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
       01 FORM-NAME               PIC X(64)   VALUE 'CHART-3D'.

      *>── Animation runtime fields ──────────────────────────────────
      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME
       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.
       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.

      *>── Timer: Timer-1 ──────────────────────────────────────────
       01 WS-Timer-1-INTERVAL   PIC 9(8) VALUE 3000.
       01 WS-Timer-1-ENABLED    PIC 9    VALUE 1.
       01 WS-Timer-1-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Chart: BarChart-1 (type: BarChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-BarChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-BarChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-BarChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: PieChart-1 (type: PieChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-PieChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-PieChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-PieChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: RadarChart-1 (type: RadarChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-RadarChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-RadarChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-RadarChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: LineChart-1 (type: LineChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-LineChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-LineChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-LineChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-CYCLE                               GLOBAL    PIC 9(9)    VALUE 0.
       01 WS-PAL                                 GLOBAL    PIC 9    VALUE 1.
       01 WS-OWED-TAB                            GLOBAL.
           05 WS-OWED                            PIC    9    OCCURS 4.
       01 WS-VAL-TAB                             GLOBAL.
           05 WS-VAL                             PIC    9(3)    OCCURS 37.
       01 WS-LABEL-DATA                          GLOBAL.
           05 FILLER                             PIC    X(9)    VALUE "Q1".
           05 FILLER                             PIC    X(9)    VALUE "Q2".
           05 FILLER                             PIC    X(9)    VALUE "Q3".
           05 FILLER                             PIC    X(9)    VALUE "Q4".
           05 FILLER                             PIC    X(9)    VALUE "Retail".
           05 FILLER                             PIC    X(9)    VALUE "Online".
           05 FILLER                             PIC    X(9)    VALUE "Wholesale".
           05 FILLER                             PIC    X(9)    VALUE "Export".
           05 FILLER                             PIC    X(9)    VALUE "Other".
           05 FILLER                             PIC    X(9)    VALUE "Quality".
           05 FILLER                             PIC    X(9)    VALUE "Speed".
           05 FILLER                             PIC    X(9)    VALUE "Cost".
           05 FILLER                             PIC    X(9)    VALUE "Support".
           05 FILLER                             PIC    X(9)    VALUE "Design".
           05 FILLER                             PIC    X(9)    VALUE "Safety".
           05 FILLER                             PIC    X(9)    VALUE "Jan".
           05 FILLER                             PIC    X(9)    VALUE "Feb".
           05 FILLER                             PIC    X(9)    VALUE "Mar".
           05 FILLER                             PIC    X(9)    VALUE "Apr".
           05 FILLER                             PIC    X(9)    VALUE "May".
           05 FILLER                             PIC    X(9)    VALUE "Jun".
           05 FILLER                             PIC    X(9)    VALUE "Jul".
           05 FILLER                             PIC    X(9)    VALUE "Aug".
           05 FILLER                             PIC    X(9)    VALUE "Sep".
           05 FILLER                             PIC    X(9)    VALUE "Oct".
           05 FILLER                             PIC    X(9)    VALUE "Nov".
           05 FILLER                             PIC    X(9)    VALUE "Dec".
       01 WS-LABEL-TAB                           REDEFINES    WS-LABEL-DATA    GLOBAL.
           05 WS-LABEL                           PIC    X(9)    OCCURS 27.
       01 WS-PALETTE-DATA                        GLOBAL.
           05 FILLER                             PIC    X(47)    VALUE "#4C9BE8,#E8C84C,#4CE87A,#E84C9B,#9B4CE8,#E87A4C".
           05 FILLER                             PIC    X(47)    VALUE "#22C3D6,#F0527A,#B5D334,#8B5CF6,#F5A524,#2FBF71".
           05 FILLER                             PIC    X(47)    VALUE "#FF7A45,#F5C242,#E8436F,#A64CE8,#4CC9E8,#7AE84C".
           05 FILLER                             PIC    X(47)    VALUE "#2FBF71,#E8C84C,#4C9BE8,#E87A4C,#B5D334,#E84C9B".
           05 FILLER                             PIC    X(47)    VALUE "#F0527A,#4CE8D0,#F5A524,#8B5CF6,#3E7BE8,#B5D334".
           05 FILLER                             PIC    X(47)    VALUE "#5072D6,#F2A65A,#51FB7C,#D65A9E,#62D0E8,#E8D44C".
       01 WS-PALETTE-TAB                         REDEFINES    WS-PALETTE-DATA    GLOBAL.
           05 WS-PALETTE                         PIC    X(47)    OCCURS 6.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-PictureBox-Desk.
          05 WS-PictureBox-Desk-TEXT       PIC X(256) VALUE 'PictureBox-Desk'.
          05 WS-PictureBox-Desk-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-Desk-ENABLED    PIC 9      VALUE 1.

       01 WS-BarChart-1.
          05 WS-BarChart-1-TEXT       PIC X(256) VALUE 'BarChart-1'.
          05 WS-BarChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-BarChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-PictureBox-2.
          05 WS-PictureBox-2-TEXT       PIC X(256) VALUE 'PictureBox-2'.
          05 WS-PictureBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-2-ENABLED    PIC 9      VALUE 1.

       01 WS-PieChart-1.
          05 WS-PieChart-1-TEXT       PIC X(256) VALUE 'PieChart-1'.
          05 WS-PieChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PieChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-RadarChart-1.
          05 WS-RadarChart-1-TEXT       PIC X(256) VALUE 'RadarChart-1'.
          05 WS-RadarChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadarChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-LineChart-1.
          05 WS-LineChart-1-TEXT       PIC X(256) VALUE 'LineChart-1'.
          05 WS-LineChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-LineChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Timer-1.
          05 WS-Timer-1-TEXT       PIC X(256) VALUE 'Timer-1'.
          05 WS-Timer-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Timer-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "CHART-3D--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "CHART-3D--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Timer-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TIMER-1--ONTICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Timer-1 'SetInterval' USING BY VALUE 3000
           CONTINUE.

      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>
       COBOL-PLAY-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "anim1".

       COBOL-STOP-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           INVOKE PictureBox-2 'StopAnimation' USING BY VALUE "anim1".

       PictureBox-2-PLAY-ANIM1.
           INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "anim1".

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

       RadarChart-1-SET-TABLE.
      *>    Bind a COBOL table to RadarChart-1.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use RadarChart-1-ADD-POINT.
           CONTINUE.

       RadarChart-1-ADD-POINT.
      *>    Append a single data point to RadarChart-1.
      *>    Usage: INVOKE RadarChart-1 ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "RadarChart-1" WS-RadarChart-1-SELECTED-LBL WS-RadarChart-1-SELECTED-VAL )
           CONTINUE.

       RadarChart-1-CLEAR.
      *>    Remove all data series from RadarChart-1.
      *>    Usage: INVOKE RadarChart-1 CLEAR
           COBOL::"CHART-CLEAR" ( "RadarChart-1" )
           CONTINUE.

       RadarChart-1-REFRESH.
      *>    Force RadarChart-1 to redraw with current data.
      *>    Usage: INVOKE RadarChart-1 REFRESH
           COBOL::"CHART-REFRESH" ( "RadarChart-1" )
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


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHART-3D--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Every layer starts hidden in a running form, so the picture and the four charts,
           *> which live in the layers Woman and charts, are shown here.

           SET     Woman::Visible                TO    TRUE
           SET     charts::Visible               TO    TRUE.

           GOBACK.

       END PROGRAM CHART-3D--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHART-3D--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM CHART-3D--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TIMER-1--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-I                                   PIC    9(3).
       01 WS-C                                   PIC    9(3).
       01 WS-K                                   PIC    9(3).
       01 WS-STEP                                PIC    S9(4).
       01 WS-NEW                                 PIC    S9(4).
       01 WS-SEED                                PIC    9(8).
       01 WS-MS-TAB.
           05 WS-MS                              PIC    9(5)    OCCURS 4.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> One tick every 3 seconds is one cycle. Every cycle gives each chart a new series, and
           *> every 25th cycle gives all four charts new colours.
           *>
           *> The values live in WS-VAL (form-level, so they last from tick to tick) and drift: each
           *> one starts from where it was and moves a little, which reads as data changing rather
           *> than as dice. Where each chart's values sit in WS-VAL:
           *>    1 ..  8  BarChart-1    4 quarters x 2 series (quarter q: 2q - 1 and 2q)
           *>    9 .. 13  PieChart-1    5 slices
           *>   14 .. 25  RadarChart-1  6 axes x 2 series (axis a: 2a + 12 and 2a + 13)
           *>   26 .. 37  LineChart-1   12 months
           *> WS-LABEL holds their names in the same order: 1 .. 4, 5 .. 9, 10 .. 15 and 16 .. 27.

           ADD    1 TO WS-CYCLE

           IF    WS-CYCLE = 1

               *> First tick. An unseeded RANDOM gives the same numbers on every run, so seed it
               *> from the clock (the first number it returns is thrown away), then start every
               *> value somewhere between 30 and 79.

               ACCEPT    WS-SEED FROM TIME
               COMPUTE    WS-STEP = FUNCTION RANDOM(WS-SEED)

               PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 37
                   COMPUTE    WS-VAL(WS-I) = FUNCTION RANDOM * 50 + 30
               END-PERFORM

               PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 4
                   SET     WS-OWED(WS-C)         TO    0
               END-PERFORM
           ELSE

               *> Every other tick: each value moves by -18 to +18, with a pull of one sixth of the
               *> way back towards the middle (52) - without it the values wander into 8 or 96 and
               *> stay there - and stays between 8 and 96.

               PERFORM    VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 37
                   COMPUTE    WS-STEP = FUNCTION RANDOM * 37
                   COMPUTE    WS-NEW = WS-VAL(WS-I) + WS-STEP - 18 + (52 - WS-VAL(WS-I)) / 6

                   IF    WS-NEW < 8
                       SET     WS-NEW            TO    8
                   END-IF

                   IF    WS-NEW > 96
                       SET     WS-NEW            TO    96
                   END-IF

                   SET     WS-VAL(WS-I)          TO    WS-NEW
               END-PERFORM
           END-IF

           *> How each chart arrives this cycle, chosen chart by chart. Most travel to their new
           *> values over 1 to 2.6 seconds, a different length each time and always shorter than the
           *> 3 second tick, so a move has ended before the next one starts. About one in three is
           *> cut instead: it shows its new values at once (250 ms is the shortest move a chart
           *> makes, the nearest AnimateValues gets to a cut) and WS-OWED(chart) = 1 makes sure that
           *> chart travels on the next cycle.

           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 4
               IF    WS-OWED(WS-C) = 1 OR FUNCTION RANDOM >= 0.35
                   COMPUTE    WS-MS(WS-C) = FUNCTION RANDOM * 1600 + 1000

                   SET     WS-OWED(WS-C)         TO    0
               ELSE
                   SET     WS-MS(WS-C)           TO    250
                   SET     WS-OWED(WS-C)         TO    1
               END-IF
           END-PERFORM

           *> Every 25th cycle: new colours. The step is 1 to 5, so the palette is never the one it
           *> was. The bar chart colours its series from SeriesColors; the pie, the radar and the
           *> line are Monochrome, so they take one colour each - the 1st, 2nd and 3rd of the
           *> palette.

           IF    FUNCTION MOD(WS-CYCLE, 25) = 0
               COMPUTE    WS-STEP = FUNCTION RANDOM * 5 + 1
               COMPUTE    WS-PAL = FUNCTION MOD(WS-PAL - 1 + WS-STEP, 6) + 1

               SET     BarChart-1::SeriesColors  TO    WS-PALETTE(WS-PAL)

               SET     PieChart-1::MonochromeColor
                                                 TO    WS-PALETTE(WS-PAL)(1:7)

               SET     RadarChart-1::MonochromeColor
                                                 TO    WS-PALETTE(WS-PAL)(9:7)

               SET     LineChart-1::MonochromeColor
                                                 TO    WS-PALETTE(WS-PAL)(17:7)
           END-IF

           *> BarChart-1: four quarters, two series.

           SET     BarChart-1::AnimationDuration TO    WS-MS(1)
           BarChart-1::Clear()

           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 4
               COMPUTE    WS-K = WS-C * 2 - 1
               BarChart-1::AddPoint(WS-LABEL(WS-C)::Trim(), WS-VAL(WS-K), WS-VAL(WS-K + 1))
           END-PERFORM

           *> PieChart-1: five slices.

           SET     PieChart-1::AnimationDuration TO    WS-MS(2)
           PieChart-1::Clear()

           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 5
               PieChart-1::AddPoint(WS-LABEL(WS-C + 4)::Trim(), WS-VAL(WS-C + 8))
           END-PERFORM

           *> RadarChart-1: six axes, two series.

           SET     RadarChart-1::AnimationDuration
                                                 TO    WS-MS(3)

           RadarChart-1::Clear()

           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 6
               COMPUTE    WS-K = WS-C * 2 + 12
               RadarChart-1::AddPoint(WS-LABEL(WS-C + 9)::Trim(), WS-VAL(WS-K), WS-VAL(WS-K + 1))
           END-PERFORM

           *> LineChart-1: twelve months.

           SET     LineChart-1::AnimationDuration
                                                 TO    WS-MS(4)

           LineChart-1::Clear()

           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 12
               LineChart-1::AddPoint(WS-LABEL(WS-C + 15)::Trim(), WS-VAL(WS-C + 25))
           END-PERFORM.

           GOBACK.

       END PROGRAM TIMER-1--ONTICK.

       END PROGRAM CHART-3D.

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
       PROGRAM-ID. INNER-FORM2.

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
       01 FORM-NAME               PIC X(64)   VALUE 'INNER-FORM2'.

      *>── Animation runtime fields ──────────────────────────────────
      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME
       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.
       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.

      *>── Timer: Timer-1 ──────────────────────────────────────────
       01 WS-Timer-1-INTERVAL   PIC 9(8) VALUE 50.
       01 WS-Timer-1-ENABLED    PIC 9    VALUE 1.
       01 WS-Timer-1-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Chart: LineChart-1 (type: LineChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-LineChart-1-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-LineChart-1-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-LineChart-1-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-PictureBox-1.
          05 WS-PictureBox-1-TEXT       PIC X(256) VALUE 'PictureBox-1'.
          05 WS-PictureBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-1-ENABLED    PIC 9      VALUE 1.

       01 WS-PictureBox-2.
          05 WS-PictureBox-2-TEXT       PIC X(256) VALUE 'PictureBox-2'.
          05 WS-PictureBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-1.
          05 WS-Panel-1-TEXT       PIC X(256) VALUE 'Panel-1'.
          05 WS-Panel-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Gauge-1.
          05 WS-Gauge-1-TEXT       PIC X(256) VALUE 'Gauge-1'.
          05 WS-Gauge-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-1-VALUE      PIC S9(9) VALUE 0.
          05 WS-Gauge-1-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-1-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-1-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-LineChart-1.
          05 WS-LineChart-1-TEXT       PIC X(256) VALUE 'LineChart-1'.
          05 WS-LineChart-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-LineChart-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Gauge-2.
          05 WS-Gauge-2-TEXT       PIC X(256) VALUE 'Gauge-2'.
          05 WS-Gauge-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-2-VALUE      PIC S9(9) VALUE 0.
          05 WS-Gauge-2-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-2-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-2-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Gauge-3.
          05 WS-Gauge-3-TEXT       PIC X(256) VALUE 'Gauge-3'.
          05 WS-Gauge-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-3-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-3-VALUE      PIC S9(9) VALUE 0.
          05 WS-Gauge-3-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-3-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-3-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Timer-1.
          05 WS-Timer-1-TEXT       PIC X(256) VALUE 'Timer-1'.
          05 WS-Timer-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Timer-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "INNER-FORM2--ONCREATE"
           CALL "INNER-FORM2--ONINITIALIZE"
           CALL "INNER-FORM2--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "INNER-FORM2--ONCLOSE"
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
           INVOKE Timer-1 'SetInterval' USING BY VALUE 50
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
           INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "1".

       COBOL-STOP-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           INVOKE PictureBox-2 'StopAnimation' USING BY VALUE "1".

       PictureBox-2-PLAY-1.
           INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "1".

      *> ── Chart INVOKE verb paragraphs ─────────────────────────────────

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
       PROGRAM-ID. INNER-FORM2--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           move 0 to Gauge-1::Value Gauge-2::Value Gauge-3::Value

           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM2--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER-FORM2--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM2--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER-FORM2--ONCREATE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM2--ONCREATE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER-FORM2--ONINITIALIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 712 TO Panel-1::Y
           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM2--ONINITIALIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TIMER-1--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 incdec  PIC s9(3)V99 VALUE 1.

       PROCEDURE DIVISION.
           IF Panel-1::Y > 50
               SUBTRACT 1 FROM Panel-1::Y
           END-IF

           IF Panel-1::Y < 712
               MOVE 5 TO Panel-1::Transparency
           END-IF

           COMPUTE Gauge-1::VALUE = Gauge-1::VALUE + incdec
           COMPUTE Gauge-2::VALUE = Gauge-2::VALUE + incdec
           COMPUTE Gauge-3::VALUE = Gauge-3::VALUE + (incdec / 2)

           IF (Gauge-1::VALUE < 0 or > 100) OR (Gauge-2::VALUE < 0 or > 100) OR (Gauge-3::VALUE < 0 or > 100)
               COMPUTE incdec = incdec * -1
           END-IF

           DISPLAY "1-" Gauge-1::VALUE "2-" Gauge-1::VALUE "3-" Gauge-1::VALUE

           CONTINUE.

           GOBACK.

       END PROGRAM TIMER-1--ONTICK.

       END PROGRAM INNER-FORM2.

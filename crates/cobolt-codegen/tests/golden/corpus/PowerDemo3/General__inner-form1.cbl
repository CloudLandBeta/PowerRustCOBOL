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
       PROGRAM-ID. INNER-FORM1.

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
       01 FORM-NAME               PIC X(64)   VALUE 'INNER-FORM1'.

      *>── Timer: Timer-1 ──────────────────────────────────────────
       01 WS-Timer-1-INTERVAL   PIC 9(8) VALUE 25.
       01 WS-Timer-1-ENABLED    PIC 9    VALUE 1.
       01 WS-Timer-1-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-CheckBox-1.
          05 WS-CheckBox-1-TEXT       PIC X(256) VALUE 'CheckBox-1'.
          05 WS-CheckBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-CheckBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-CheckBox-1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-RadioButton-1.
          05 WS-RadioButton-1-TEXT       PIC X(256) VALUE 'RadioButton-1'.
          05 WS-RadioButton-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-1-ENABLED    PIC 9      VALUE 1.

       01 WS-ListBox-1.
          05 WS-ListBox-1-TEXT       PIC X(256) VALUE 'ListBox-1'.
          05 WS-ListBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ListBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-ListBox-1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-NumericUpDown-1.
          05 WS-NumericUpDown-1-TEXT       PIC X(256) VALUE 'NumericUpDown-1'.
          05 WS-NumericUpDown-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-NumericUpDown-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Switch-1.
          05 WS-Switch-1-TEXT       PIC X(256) VALUE 'Switch-1'.
          05 WS-Switch-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Switch-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Switch-1-CHECKED    PIC 9      VALUE 0.

       01 WS-RadioButton-2.
          05 WS-RadioButton-2-TEXT       PIC X(256) VALUE 'RadioButton-2'.
          05 WS-RadioButton-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-2-ENABLED    PIC 9      VALUE 1.

       01 WS-DateTimePicker-2.
          05 WS-DateTimePicker-2-TEXT       PIC X(256) VALUE 'DateTimePicker-2'.
          05 WS-DateTimePicker-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-DateTimePicker-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Knob-1.
          05 WS-Knob-1-TEXT       PIC X(256) VALUE 'Knob-1'.
          05 WS-Knob-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Knob-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Knob-1-VALUE      PIC S9(9) VALUE 28.
          05 WS-Knob-1-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Knob-1-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Knob-1-STEP       PIC S9(9) VALUE 1.
          05 WS-Knob-1-DEFAULT    PIC S9(9) VALUE 0.

       01 WS-Gauge-1.
          05 WS-Gauge-1-TEXT       PIC X(256) VALUE 'Gauge-1'.
          05 WS-Gauge-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-1-VALUE      PIC S9(9) VALUE 80.
          05 WS-Gauge-1-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-1-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-1-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Maps-1.
          05 WS-Maps-1-TEXT       PIC X(256) VALUE 'Maps-1'.
          05 WS-Maps-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Maps-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Maps-1-CENTER-LAT PIC X(32)  VALUE ''.
          05 WS-Maps-1-CENTER-LNG PIC X(32)  VALUE ''.
          05 WS-Maps-1-ZOOM       PIC S9(4)  VALUE 2.

       01 WS-ProgressBar-1.
          05 WS-ProgressBar-1-TEXT       PIC X(256) VALUE 'ProgressBar-1'.
          05 WS-ProgressBar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ProgressBar-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Slider-2.
          05 WS-Slider-2-TEXT       PIC X(256) VALUE 'Slider-2'.
          05 WS-Slider-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Slider-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Slider-2-VALUE      PIC S9(9) VALUE 45.
          05 WS-Slider-2-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Slider-2-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Slider-2-STEP       PIC S9(9) VALUE 10.

       01 WS-Timer-1.
          05 WS-Timer-1-TEXT       PIC X(256) VALUE 'Timer-1'.
          05 WS-Timer-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Timer-1-ENABLED    PIC 9      VALUE 1.

       01 WS-ToolBar-1.
          05 WS-ToolBar-1-TEXT       PIC X(256) VALUE 'ToolBar-1'.
          05 WS-ToolBar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ToolBar-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Line-1.
          05 WS-Line-1-TEXT       PIC X(256) VALUE 'Line-1'.
          05 WS-Line-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Line-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FileDropZone-1.
          05 WS-FileDropZone-1-TEXT       PIC X(256) VALUE 'FileDropZone-1'.
          05 WS-FileDropZone-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FileDropZone-1-ENABLED    PIC 9      VALUE 1.
          05 WS-FileDropZone-1-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-FileDropZone-1-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-ListBox-2.
          05 WS-ListBox-2-TEXT       PIC X(256) VALUE 'ListBox-2'.
          05 WS-ListBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-ListBox-2-ENABLED    PIC 9      VALUE 1.
          05 WS-ListBox-2-VALUE      PIC X(512) VALUE SPACES.

       01 WS-ComboBox-1.
          05 WS-ComboBox-1-TEXT       PIC X(256) VALUE 'ComboBox-1'.
          05 WS-ComboBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ComboBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-ComboBox-1-VALUE      PIC X(512) VALUE SPACES.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "INNER-FORM1--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "INNER-FORM1--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Knob-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onValueChanged"
                               CALL "KNOB-1--ONVALUECHANGED"
                       END-EVALUATE
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
           INVOKE Timer-1 'SetInterval' USING BY VALUE 25
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
       PROGRAM-ID. INNER-FORM1--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "-23.5614" TO MAPS-1::CenterLat
           MOVE "-46.6558" TO MAPS-1::CenterLng

           MOVE 8 TO MAPS-1::Zoom

           INVOKE MAPS-1 "AddMarker" USING "M1" "-23.5614" "-46.6558" "Av. Paulista" "1578"

           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM1--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER-FORM1--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM INNER-FORM1--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNOB-1--ONVALUECHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           IF Knob-1::Value > 100
               SET Knob-1::Value  TO 0
           END-IF

           MOVE Knob-1::Value TO ProgressBar-1::Value
                                 Gauge-1::Value
                                 Slider-2::Value

           CONTINUE.

           GOBACK.

       END PROGRAM KNOB-1--ONVALUECHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TIMER-1--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           COMPUTE Knob-1::Value = Knob-1::Value + 1

           CONTINUE.

           GOBACK.

       END PROGRAM TIMER-1--ONTICK.

       END PROGRAM INNER-FORM1.

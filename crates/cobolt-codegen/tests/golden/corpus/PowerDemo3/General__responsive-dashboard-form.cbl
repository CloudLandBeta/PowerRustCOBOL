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
       PROGRAM-ID. RESPONSIVE-DASHBOARD-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-DASHBOARD-FORM'.

      *>── DataGrid DB-ORDERS CSV export ──────────────────────────
       01 WS-DB-ORDERS-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-DB-ORDERS-CSV-STATUS  PIC 9       VALUE 0.

      *>── Chart: DB-BAR (type: BarChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-DB-BAR-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-DB-BAR-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-DB-BAR-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: DB-LINE (type: LineChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-DB-LINE-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-DB-LINE-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-DB-LINE-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── Chart: DB-PIE (type: PieChart) ─────────────────────────────────────
      *>   Data source : (none — use INVOKE SET-TABLE or ADD-POINT)
      *>   Row count   : (not set)
       01 WS-DB-PIE-SELECTED-IDX PIC 9(6) VALUE 0.
       01 WS-DB-PIE-SELECTED-LBL PIC X(64) VALUE SPACES.
       01 WS-DB-PIE-SELECTED-VAL PIC 9(18)V9(6) VALUE ZEROES.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-DB-HEAD.
          05 WS-DB-HEAD-TEXT       PIC X(256) VALUE 'DB-HEAD'.
          05 WS-DB-HEAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-HEAD-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-LOGO.
          05 WS-DB-LOGO-TEXT       PIC X(256) VALUE 'PRC'.
          05 WS-DB-LOGO-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-LOGO-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-TITLE.
          05 WS-DB-TITLE-TEXT       PIC X(256) VALUE 'Operations dashboard'.
          05 WS-DB-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-SEARCH.
          05 WS-DB-SEARCH-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DB-SEARCH-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-SEARCH-ENABLED    PIC 9      VALUE 1.
          05 WS-DB-SEARCH-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DB-NEW.
          05 WS-DB-NEW-TEXT       PIC X(256) VALUE 'New order'.
          05 WS-DB-NEW-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-NEW-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-EXPORT.
          05 WS-DB-EXPORT-TEXT       PIC X(256) VALUE 'Export'.
          05 WS-DB-EXPORT-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-EXPORT-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-NAV.
          05 WS-DB-NAV-TEXT       PIC X(256) VALUE 'DB-NAV'.
          05 WS-DB-NAV-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-NAV-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N1.
          05 WS-DB-N1-TEXT       PIC X(256) VALUE 'Overview'.
          05 WS-DB-N1-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N1-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N2.
          05 WS-DB-N2-TEXT       PIC X(256) VALUE 'Orders'.
          05 WS-DB-N2-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N2-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N3.
          05 WS-DB-N3-TEXT       PIC X(256) VALUE 'Customers'.
          05 WS-DB-N3-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N3-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N4.
          05 WS-DB-N4-TEXT       PIC X(256) VALUE 'Products'.
          05 WS-DB-N4-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N4-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N5.
          05 WS-DB-N5-TEXT       PIC X(256) VALUE 'Stock'.
          05 WS-DB-N5-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N5-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N6.
          05 WS-DB-N6-TEXT       PIC X(256) VALUE 'Deliveries'.
          05 WS-DB-N6-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N6-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N7.
          05 WS-DB-N7-TEXT       PIC X(256) VALUE 'Invoices'.
          05 WS-DB-N7-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N7-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-N8.
          05 WS-DB-N8-TEXT       PIC X(256) VALUE 'Reports'.
          05 WS-DB-N8-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-N8-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-NAV-SPACE.
          05 WS-DB-NAV-SPACE-TEXT       PIC X(256) VALUE ''.
          05 WS-DB-NAV-SPACE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-NAV-SPACE-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-SETTINGS.
          05 WS-DB-SETTINGS-TEXT       PIC X(256) VALUE 'Settings'.
          05 WS-DB-SETTINGS-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-SETTINGS-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-MAIN.
          05 WS-DB-MAIN-TEXT       PIC X(256) VALUE 'DB-MAIN'.
          05 WS-DB-MAIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-MAIN-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K1.
          05 WS-DB-K1-TEXT       PIC X(256) VALUE 'DB-K1'.
          05 WS-DB-K1-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K1-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K1-CAP.
          05 WS-DB-K1-CAP-TEXT       PIC X(256) VALUE 'Revenue today'.
          05 WS-DB-K1-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K1-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K1-VAL.
          05 WS-DB-K1-VAL-TEXT       PIC X(256) VALUE 'R$ 48.210'.
          05 WS-DB-K1-VAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K1-VAL-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K1-BAR.
          05 WS-DB-K1-BAR-TEXT       PIC X(256) VALUE 'DB-K1-BAR'.
          05 WS-DB-K1-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K1-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K2.
          05 WS-DB-K2-TEXT       PIC X(256) VALUE 'DB-K2'.
          05 WS-DB-K2-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K2-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K2-CAP.
          05 WS-DB-K2-CAP-TEXT       PIC X(256) VALUE 'Open orders'.
          05 WS-DB-K2-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K2-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K2-VAL.
          05 WS-DB-K2-VAL-TEXT       PIC X(256) VALUE '1.284'.
          05 WS-DB-K2-VAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K2-VAL-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K2-BAR.
          05 WS-DB-K2-BAR-TEXT       PIC X(256) VALUE 'DB-K2-BAR'.
          05 WS-DB-K2-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K2-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K3.
          05 WS-DB-K3-TEXT       PIC X(256) VALUE 'DB-K3'.
          05 WS-DB-K3-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K3-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K3-CAP.
          05 WS-DB-K3-CAP-TEXT       PIC X(256) VALUE 'Late deliveries'.
          05 WS-DB-K3-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K3-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K3-VAL.
          05 WS-DB-K3-VAL-TEXT       PIC X(256) VALUE '3'.
          05 WS-DB-K3-VAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K3-VAL-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K3-BAR.
          05 WS-DB-K3-BAR-TEXT       PIC X(256) VALUE 'DB-K3-BAR'.
          05 WS-DB-K3-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K3-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K4.
          05 WS-DB-K4-TEXT       PIC X(256) VALUE 'DB-K4'.
          05 WS-DB-K4-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K4-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K4-CAP.
          05 WS-DB-K4-CAP-TEXT       PIC X(256) VALUE 'Stock alerts'.
          05 WS-DB-K4-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K4-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K4-VAL.
          05 WS-DB-K4-VAL-TEXT       PIC X(256) VALUE '37'.
          05 WS-DB-K4-VAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K4-VAL-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-K4-BAR.
          05 WS-DB-K4-BAR-TEXT       PIC X(256) VALUE 'DB-K4-BAR'.
          05 WS-DB-K4-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-K4-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-BAR.
          05 WS-DB-BAR-TEXT       PIC X(256) VALUE 'DB-BAR'.
          05 WS-DB-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-LINE.
          05 WS-DB-LINE-TEXT       PIC X(256) VALUE 'DB-LINE'.
          05 WS-DB-LINE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-LINE-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-CUSTOMERS.
          05 WS-DB-CUSTOMERS-TEXT       PIC X(256) VALUE 'DB-CUSTOMERS'.
          05 WS-DB-CUSTOMERS-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-CUSTOMERS-ENABLED    PIC 9      VALUE 1.
          05 WS-DB-CUSTOMERS-VALUE      PIC X(512) VALUE SPACES.

       01 WS-DB-ORDERS.
          05 WS-DB-ORDERS-TEXT       PIC X(256) VALUE 'DB-ORDERS'.
          05 WS-DB-ORDERS-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-ORDERS-ENABLED    PIC 9      VALUE 1.

       01 WS-DB-PIE.
          05 WS-DB-PIE-TEXT       PIC X(256) VALUE 'DB-PIE'.
          05 WS-DB-PIE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DB-PIE-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-DASHBOARD-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-DASHBOARD-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-DASHBOARD-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-DASHBOARD-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-DASHBOARD-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
       DB-ORDERS-EXPORT-CSV.
      *>    Export DB-ORDERS data to CSV file.  Delimiter: ",". Mode: Filtered.
      *>    Column order and filtered/all rows follow the DataGrid settings.
      *>    Set WS-DB-ORDERS-CSV-PATH to the desired output file path before calling.
           INVOKE DB-ORDERS 'ExportCSV'
               USING BY REFERENCE WS-DB-ORDERS-CSV-PATH
               RETURNING WS-DB-ORDERS-CSV-STATUS
           IF WS-DB-ORDERS-CSV-STATUS NOT = 0
               DISPLAY "CSV export error: " WS-DB-ORDERS-CSV-STATUS
           END-IF.

      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>
      *> ── Chart INVOKE verb paragraphs ─────────────────────────────────

       DB-BAR-SET-TABLE.
      *>    Bind a COBOL table to DB-BAR.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use DB-BAR-ADD-POINT.
           CONTINUE.

       DB-BAR-ADD-POINT.
      *>    Append a single data point to DB-BAR.
      *>    Usage: INVOKE DB-BAR ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "DB-BAR" WS-DB-BAR-SELECTED-LBL WS-DB-BAR-SELECTED-VAL )
           CONTINUE.

       DB-BAR-CLEAR.
      *>    Remove all data series from DB-BAR.
      *>    Usage: INVOKE DB-BAR CLEAR
           COBOL::"CHART-CLEAR" ( "DB-BAR" )
           CONTINUE.

       DB-BAR-REFRESH.
      *>    Force DB-BAR to redraw with current data.
      *>    Usage: INVOKE DB-BAR REFRESH
           COBOL::"CHART-REFRESH" ( "DB-BAR" )
           CONTINUE.

       DB-LINE-SET-TABLE.
      *>    Bind a COBOL table to DB-LINE.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use DB-LINE-ADD-POINT.
           CONTINUE.

       DB-LINE-ADD-POINT.
      *>    Append a single data point to DB-LINE.
      *>    Usage: INVOKE DB-LINE ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "DB-LINE" WS-DB-LINE-SELECTED-LBL WS-DB-LINE-SELECTED-VAL )
           CONTINUE.

       DB-LINE-CLEAR.
      *>    Remove all data series from DB-LINE.
      *>    Usage: INVOKE DB-LINE CLEAR
           COBOL::"CHART-CLEAR" ( "DB-LINE" )
           CONTINUE.

       DB-LINE-REFRESH.
      *>    Force DB-LINE to redraw with current data.
      *>    Usage: INVOKE DB-LINE REFRESH
           COBOL::"CHART-REFRESH" ( "DB-LINE" )
           CONTINUE.

       DB-PIE-SET-TABLE.
      *>    Bind a COBOL table to DB-PIE.
      *>    Nothing to bind: set DataSource and DataCount on the
      *>    control to data items this program declares, or use DB-PIE-ADD-POINT.
           CONTINUE.

       DB-PIE-ADD-POINT.
      *>    Append a single data point to DB-PIE.
      *>    Usage: INVOKE DB-PIE ADD-POINT USING WS-LABEL WS-VALUE
           COBOL::"CHART-ADD-POINT" ( "DB-PIE" WS-DB-PIE-SELECTED-LBL WS-DB-PIE-SELECTED-VAL )
           CONTINUE.

       DB-PIE-CLEAR.
      *>    Remove all data series from DB-PIE.
      *>    Usage: INVOKE DB-PIE CLEAR
           COBOL::"CHART-CLEAR" ( "DB-PIE" )
           CONTINUE.

       DB-PIE-REFRESH.
      *>    Force DB-PIE to redraw with current data.
      *>    Usage: INVOKE DB-PIE REFRESH
           COBOL::"CHART-REFRESH" ( "DB-PIE" )
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DASHBOARD-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DSH-SHOW-INFO"
           CALL "DSH-CHARTS"

           GOBACK.

       END PROGRAM RESPONSIVE-DASHBOARD-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DASHBOARD-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-DASHBOARD-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DASHBOARD-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DSH-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-DASHBOARD-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-DASHBOARD-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "DSH-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-DASHBOARD-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DSH-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM DSH-SHOW-INFO.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DSH-CHARTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> Sample data for the three charts.
           INVOKE DB-BAR::Clear()
           INVOKE DB-BAR::AddPoint("Mon", 182)
           INVOKE DB-BAR::AddPoint("Tue", 214)
           INVOKE DB-BAR::AddPoint("Wed", 199)
           INVOKE DB-BAR::AddPoint("Thu", 241)
           INVOKE DB-BAR::AddPoint("Fri", 276)
           INVOKE DB-BAR::AddPoint("Sat", 133)
           INVOKE DB-LINE::Clear()
           INVOKE DB-LINE::AddPoint("W1", 31)
           INVOKE DB-LINE::AddPoint("W2", 35)
           INVOKE DB-LINE::AddPoint("W3", 33)
           INVOKE DB-LINE::AddPoint("W4", 41)
           INVOKE DB-LINE::AddPoint("W5", 44)
           INVOKE DB-LINE::AddPoint("W6", 40)
           INVOKE DB-LINE::AddPoint("W7", 47)
           INVOKE DB-LINE::AddPoint("W8", 52)
           INVOKE DB-PIE::Clear()
           INVOKE DB-PIE::AddPoint("Shop", 46)
           INVOKE DB-PIE::AddPoint("Web", 38)
           INVOKE DB-PIE::AddPoint("Phone", 16)

           GOBACK.

       END PROGRAM DSH-CHARTS.

       END PROGRAM RESPONSIVE-DASHBOARD-FORM.

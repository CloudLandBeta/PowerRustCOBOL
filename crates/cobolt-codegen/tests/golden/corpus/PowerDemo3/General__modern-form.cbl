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
       PROGRAM-ID. MODERN-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'MODERN-FORM'.

      *>── DataGrid GRD-LIST CSV export ──────────────────────────
       01 WS-GRD-LIST-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-GRD-LIST-CSV-STATUS  PIC 9       VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'My Modern Form'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-SEARCH.
          05 WS-TXT-SEARCH-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TXT-SEARCH-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-SEARCH-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-SEARCH-VALUE      PIC X(256) VALUE SPACES.

       01 WS-BTN-NEW.
          05 WS-BTN-NEW-TEXT       PIC X(256) VALUE 'New'.
          05 WS-BTN-NEW-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-NEW-ENABLED    PIC 9      VALUE 1.

       01 WS-GRD-LIST.
          05 WS-GRD-LIST-TEXT       PIC X(256) VALUE 'GRD-LIST'.
          05 WS-GRD-LIST-VISIBLE    PIC 9      VALUE 1.
          05 WS-GRD-LIST-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-DETAILS.
          05 WS-PNL-DETAILS-TEXT       PIC X(256) VALUE 'PNL-DETAILS'.
          05 WS-PNL-DETAILS-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-DETAILS-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-DETAILS.
          05 WS-LBL-DETAILS-TEXT       PIC X(256) VALUE 'Details'.
          05 WS-LBL-DETAILS-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-DETAILS-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-NAME.
          05 WS-LBL-NAME-TEXT       PIC X(256) VALUE 'Name'.
          05 WS-LBL-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-NAME-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-NAME.
          05 WS-TXT-NAME-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TXT-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-NAME-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-NAME-VALUE      PIC X(256) VALUE SPACES.

       01 WS-LBL-EMAIL.
          05 WS-LBL-EMAIL-TEXT       PIC X(256) VALUE 'E-mail'.
          05 WS-LBL-EMAIL-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-EMAIL-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-EMAIL.
          05 WS-TXT-EMAIL-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TXT-EMAIL-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-EMAIL-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-EMAIL-VALUE      PIC X(256) VALUE SPACES.

       01 WS-LBL-PHONE.
          05 WS-LBL-PHONE-TEXT       PIC X(256) VALUE 'Phone'.
          05 WS-LBL-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-PHONE-ENABLED    PIC 9      VALUE 1.

       01 WS-TXT-PHONE.
          05 WS-TXT-PHONE-TEXT       PIC X(256) VALUE SPACES.
          05 WS-TXT-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-TXT-PHONE-ENABLED    PIC 9      VALUE 1.
          05 WS-TXT-PHONE-VALUE      PIC X(256) VALUE SPACES.

       01 WS-BTN-DELETE.
          05 WS-BTN-DELETE-TEXT       PIC X(256) VALUE 'Delete'.
          05 WS-BTN-DELETE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-DELETE-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-SAVE.
          05 WS-BTN-SAVE-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-BTN-SAVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-SAVE-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "MODERN-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "MODERN-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               *> No event handlers defined yet.
               CONTINUE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
       GRD-LIST-EXPORT-CSV.
      *>    Export GRD-LIST data to CSV file.  Delimiter: ",". Mode: Filtered.
      *>    Column order and filtered/all rows follow the DataGrid settings.
      *>    Set WS-GRD-LIST-CSV-PATH to the desired output file path before calling.
           INVOKE GRD-LIST 'ExportCSV'
               USING BY REFERENCE WS-GRD-LIST-CSV-PATH
               RETURNING WS-GRD-LIST-CSV-STATUS
           IF WS-GRD-LIST-CSV-STATUS NOT = 0
               DISPLAY "CSV export error: " WS-GRD-LIST-CSV-STATUS
           END-IF.

      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MODERN-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM MODERN-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MODERN-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM MODERN-FORM--ONCLOSE.

       END PROGRAM MODERN-FORM.

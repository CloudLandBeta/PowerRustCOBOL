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
       PROGRAM-ID. VIEWER-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'VIEWER-FORM'.

      *>── Timer: Tmr-Stream ──────────────────────────────────────────
       01 WS-Tmr-Stream-INTERVAL   PIC 9(8) VALUE 120.
       01 WS-Tmr-Stream-ENABLED    PIC 9    VALUE 0.
       01 WS-Tmr-Stream-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Viewer'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Ready.'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Fdz-View1.
          05 WS-Fdz-View1-TEXT       PIC X(256) VALUE 'Fdz-View1'.
          05 WS-Fdz-View1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Fdz-View1-ENABLED    PIC 9      VALUE 1.
          05 WS-Fdz-View1-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-Fdz-View1-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-Fdz-View2.
          05 WS-Fdz-View2-TEXT       PIC X(256) VALUE 'Fdz-View2'.
          05 WS-Fdz-View2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Fdz-View2-ENABLED    PIC 9      VALUE 1.
          05 WS-Fdz-View2-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-Fdz-View2-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-Cap-Layout.
          05 WS-Cap-Layout-TEXT       PIC X(256) VALUE 'Layout - how the document is presented'.
          05 WS-Cap-Layout-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cap-Layout-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Layout-Raw.
          05 WS-Btn-Layout-Raw-TEXT       PIC X(256) VALUE 'Raw'.
          05 WS-Btn-Layout-Raw-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Layout-Raw-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Layout-Web.
          05 WS-Btn-Layout-Web-TEXT       PIC X(256) VALUE 'Web'.
          05 WS-Btn-Layout-Web-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Layout-Web-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Layout-Print.
          05 WS-Btn-Layout-Print-TEXT       PIC X(256) VALUE 'Print'.
          05 WS-Btn-Layout-Print-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Layout-Print-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Layout-Page.
          05 WS-Btn-Layout-Page-TEXT       PIC X(256) VALUE 'Page'.
          05 WS-Btn-Layout-Page-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Layout-Page-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Layout-Streamed.
          05 WS-Btn-Layout-Streamed-TEXT       PIC X(256) VALUE 'Streamed'.
          05 WS-Btn-Layout-Streamed-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Layout-Streamed-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Fullscreen.
          05 WS-Btn-Fullscreen-TEXT       PIC X(256) VALUE 'Fullscreen'.
          05 WS-Btn-Fullscreen-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Fullscreen-ENABLED    PIC 9      VALUE 1.

       01 WS-Cap-Split.
          05 WS-Cap-Split-TEXT       PIC X(256) VALUE 'Split view, card grid and filmstrip'.
          05 WS-Cap-Split-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cap-Split-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Split-None.
          05 WS-Btn-Split-None-TEXT       PIC X(256) VALUE 'Split: none'.
          05 WS-Btn-Split-None-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Split-None-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Split-LR.
          05 WS-Btn-Split-LR-TEXT       PIC X(256) VALUE 'Split: side by side'.
          05 WS-Btn-Split-LR-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Split-LR-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Split-TB.
          05 WS-Btn-Split-TB-TEXT       PIC X(256) VALUE 'Split: stacked'.
          05 WS-Btn-Split-TB-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Split-TB-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Filmstrip.
          05 WS-Btn-Filmstrip-TEXT       PIC X(256) VALUE 'Filmstrip'.
          05 WS-Btn-Filmstrip-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Filmstrip-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Full.
          05 WS-Btn-Full-TEXT       PIC X(256) VALUE 'Mode: full'.
          05 WS-Btn-Full-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Full-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cards.
          05 WS-Btn-Cards-TEXT       PIC X(256) VALUE 'Mode: cards'.
          05 WS-Btn-Cards-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cards-ENABLED    PIC 9      VALUE 1.

       01 WS-Cap-Zoom.
          05 WS-Cap-Zoom-TEXT       PIC X(256) VALUE 'Zoom, font size and pages'.
          05 WS-Cap-Zoom-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cap-Zoom-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Zoom-Out.
          05 WS-Btn-Zoom-Out-TEXT       PIC X(256) VALUE 'Zoom out'.
          05 WS-Btn-Zoom-Out-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Zoom-Out-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Zoom-In.
          05 WS-Btn-Zoom-In-TEXT       PIC X(256) VALUE 'Zoom in'.
          05 WS-Btn-Zoom-In-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Zoom-In-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Zoom-100.
          05 WS-Btn-Zoom-100-TEXT       PIC X(256) VALUE 'Zoom 100%'.
          05 WS-Btn-Zoom-100-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Zoom-100-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Font-Up.
          05 WS-Btn-Font-Up-TEXT       PIC X(256) VALUE 'Font +'.
          05 WS-Btn-Font-Up-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Font-Up-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Prev-Page.
          05 WS-Btn-Prev-Page-TEXT       PIC X(256) VALUE 'Previous page'.
          05 WS-Btn-Prev-Page-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Prev-Page-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Next-Page.
          05 WS-Btn-Next-Page-TEXT       PIC X(256) VALUE 'Next page'.
          05 WS-Btn-Next-Page-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Next-Page-ENABLED    PIC 9      VALUE 1.

       01 WS-Cap-Find.
          05 WS-Cap-Find-TEXT       PIC X(256) VALUE 'Find - search the document that is open'.
          05 WS-Cap-Find-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cap-Find-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Find.
          05 WS-Txt-Find-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-Txt-Find-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Find-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Find-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Btn-Find.
          05 WS-Btn-Find-TEXT       PIC X(256) VALUE 'Find'.
          05 WS-Btn-Find-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Find-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Find-Next.
          05 WS-Btn-Find-Next-TEXT       PIC X(256) VALUE 'Next match'.
          05 WS-Btn-Find-Next-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Find-Next-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Find-Prev.
          05 WS-Btn-Find-Prev-TEXT       PIC X(256) VALUE 'Previous match'.
          05 WS-Btn-Find-Prev-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Find-Prev-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Find-Close.
          05 WS-Btn-Find-Close-TEXT       PIC X(256) VALUE 'Close find'.
          05 WS-Btn-Find-Close-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Find-Close-ENABLED    PIC 9      VALUE 1.

       01 WS-Chk-Case.
          05 WS-Chk-Case-TEXT       PIC X(256) VALUE 'Case sensitive'.
          05 WS-Chk-Case-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Case-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Case-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Chk-Highlight.
          05 WS-Chk-Highlight-TEXT       PIC X(256) VALUE 'Highlight'.
          05 WS-Chk-Highlight-VISIBLE    PIC 9      VALUE 1.
          05 WS-Chk-Highlight-ENABLED    PIC 9      VALUE 1.
          05 WS-Chk-Highlight-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Matches.
          05 WS-Lbl-Matches-TEXT       PIC X(256) VALUE 'matches: -'.
          05 WS-Lbl-Matches-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Matches-ENABLED    PIC 9      VALUE 1.

       01 WS-Cap-Actions.
          05 WS-Cap-Actions-TEXT       PIC X(256) VALUE 'Save, print, share - and the live stream'.
          05 WS-Cap-Actions-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cap-Actions-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save as...'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Print.
          05 WS-Btn-Print-TEXT       PIC X(256) VALUE 'Print'.
          05 WS-Btn-Print-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Print-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Share.
          05 WS-Btn-Share-TEXT       PIC X(256) VALUE 'Share'.
          05 WS-Btn-Share-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Share-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Jump.
          05 WS-Btn-Jump-TEXT       PIC X(256) VALUE 'Jump to latest'.
          05 WS-Btn-Jump-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Jump-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Stream-Start.
          05 WS-Btn-Stream-Start-TEXT       PIC X(256) VALUE 'Start stream'.
          05 WS-Btn-Stream-Start-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Stream-Start-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Stream-Stop.
          05 WS-Btn-Stream-Stop-TEXT       PIC X(256) VALUE 'Stop stream'.
          05 WS-Btn-Stream-Stop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Stream-Stop-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-New-Conv.
          05 WS-Btn-New-Conv-TEXT       PIC X(256) VALUE 'New conversation'.
          05 WS-Btn-New-Conv-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-New-Conv-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Reg-Conv.
          05 WS-Btn-Reg-Conv-TEXT       PIC X(256) VALUE 'Register past one'.
          05 WS-Btn-Reg-Conv-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Reg-Conv-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Sel-Conv.
          05 WS-Btn-Sel-Conv-TEXT       PIC X(256) VALUE 'Select past one'.
          05 WS-Btn-Sel-Conv-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Sel-Conv-ENABLED    PIC 9      VALUE 1.

       01 WS-Num-Chunks.
          05 WS-Num-Chunks-TEXT       PIC X(256) VALUE 'Num-Chunks'.
          05 WS-Num-Chunks-VISIBLE    PIC 9      VALUE 1.
          05 WS-Num-Chunks-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Sample-Md.
          05 WS-Btn-Sample-Md-TEXT       PIC X(256) VALUE 'Sample: Markdown'.
          05 WS-Btn-Sample-Md-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Sample-Md-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Sample-Txt.
          05 WS-Btn-Sample-Txt-TEXT       PIC X(256) VALUE 'Sample: paged text'.
          05 WS-Btn-Sample-Txt-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Sample-Txt-ENABLED    PIC 9      VALUE 1.

       01 WS-VWR-1.
          05 WS-VWR-1-TEXT       PIC X(256) VALUE 'VWR-1'.
          05 WS-VWR-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-VWR-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Tmr-Stream.
          05 WS-Tmr-Stream-TEXT       PIC X(256) VALUE 'Tmr-Stream'.
          05 WS-Tmr-Stream-VISIBLE    PIC 9      VALUE 1.
          05 WS-Tmr-Stream-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Log.
          05 WS-Txt-Log-TEXT       PIC X(2048) VALUE 'Txt-Log'.
          05 WS-Txt-Log-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Log-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Log-VALUE      PIC X(2048) VALUE SPACES.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "VIEWER-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "VIEWER-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Fdz-View1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onFilesDropped"
                               CALL "FDZ-VIEW1--ONFILESDROPPED"
                       END-EVALUATE
                   WHEN "Fdz-View2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onFilesDropped"
                               CALL "FDZ-VIEW2--ONFILESDROPPED"
                       END-EVALUATE
                   WHEN "Btn-Layout-Raw"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAYOUT-RAW--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Layout-Web"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAYOUT-WEB--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Layout-Print"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAYOUT-PRINT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Layout-Page"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAYOUT-PAGE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Layout-Streamed"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LAYOUT-STREAMED--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Fullscreen"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FULLSCREEN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Split-None"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SPLIT-NONE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Split-LR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SPLIT-LR--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Split-TB"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SPLIT-TB--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Filmstrip"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FILMSTRIP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Full"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FULL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cards"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CARDS--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Zoom-Out"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ZOOM-OUT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Zoom-In"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ZOOM-IN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Zoom-100"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ZOOM-100--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Font-Up"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FONT-UP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Prev-Page"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PREV-PAGE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Next-Page"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEXT-PAGE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Find"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FIND--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Find-Next"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FIND-NEXT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Find-Prev"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FIND-PREV--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Find-Close"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-FIND-CLOSE--ONCLICK"
                       END-EVALUATE
                   WHEN "Chk-Case"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "CHK-CASE--ONCLICK"
                       END-EVALUATE
                   WHEN "Chk-Highlight"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "CHK-HIGHLIGHT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Print"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PRINT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Share"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SHARE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Jump"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-JUMP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Stream-Start"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-STREAM-START--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Stream-Stop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-STREAM-STOP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-New-Conv"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-NEW-CONV--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Reg-Conv"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-REG-CONV--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Sel-Conv"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SEL-CONV--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Sample-Md"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAMPLE-MD--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Sample-Txt"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAMPLE-TXT--ONCLICK"
                       END-EVALUATE
                   WHEN "VWR-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onLoaded"
                               CALL "VWR-1--ONLOADED"
                           WHEN "onError"
                               CALL "VWR-1--ONERROR"
                           WHEN "onConversationSelected"
                               CALL "VWR-1--ONCONVERSATIONSELECTED"
                       END-EVALUATE
                   WHEN "Tmr-Stream"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TMR-STREAM--ONTICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Tmr-Stream 'SetInterval' USING BY VALUE 120
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
       PROGRAM-ID. VIEWER-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Page" TO VWR-1::Layout.
           MOVE "None" TO VWR-1::SplitMode.
           MOVE "Full" TO VWR-1::View1ViewMode.
           MOVE 100 TO VWR-1::View1Zoom.
           MOVE 14 TO VWR-1::FontSize.
      *> Something to look at before anything is picked. The sample
      *> ships with the project, so the demo does not depend on where
      *> the repository happens to sit, and it exercises headings,
      *> tables, lists, quotes and fenced code all at once.
           MOVE "assets/docs/viewer-sample.md" TO VWR-1::View1Source.
           MOVE "Ready. Pick a file, or press Start stream." TO Lbl-Status::Caption.
           Txt-Log::AppendText("Viewer demo ready.").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM VIEWER-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VIEWER-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM VIEWER-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FDZ-VIEW1--ONFILESDROPPED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-ALL PIC X(2048).
       01 WS-PATH PIC X(512).
       01 WS-LINE PIC X(600).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> A click on a FileDropZone opens the native OS picker; a drag
      *> and drop lands here through the same event. DroppedFiles is
      *> what THIS drop brought, BEFORE any copying, so the Viewer opens
      *> the operator's own file and nothing is duplicated anywhere.
      *> StagedFiles is the running basket: its first line stays the
      *> first file ever dropped, which is not what a drop means here.
           MOVE Fdz-View1::DroppedFiles TO WS-ALL.
           UNSTRING WS-ALL DELIMITED BY X"0A" INTO WS-PATH.
           IF FUNCTION TRIM(WS-PATH) NOT = SPACES
               MOVE FUNCTION TRIM(WS-PATH) TO VWR-1::View1Source
               STRING "view 1: " FUNCTION TRIM(WS-PATH) INTO WS-LINE
               MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Status::Caption
           END-IF.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM FDZ-VIEW1--ONFILESDROPPED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FDZ-VIEW2--ONFILESDROPPED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-ALL PIC X(2048).
       01 WS-PATH PIC X(512).
       01 WS-LINE PIC X(600).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> A click on a FileDropZone opens the native OS picker; a drag
      *> and drop lands here through the same event. DroppedFiles is
      *> what THIS drop brought, BEFORE any copying, so the Viewer opens
      *> the operator's own file and nothing is duplicated anywhere.
      *> StagedFiles is the running basket: its first line stays the
      *> first file ever dropped, which is not what a drop means here.
           MOVE Fdz-View2::DroppedFiles TO WS-ALL.
           UNSTRING WS-ALL DELIMITED BY X"0A" INTO WS-PATH.
           IF FUNCTION TRIM(WS-PATH) NOT = SPACES
               MOVE FUNCTION TRIM(WS-PATH) TO VWR-1::View2Source
               MOVE "LeftRight" TO VWR-1::SplitMode
               STRING "view 2: " FUNCTION TRIM(WS-PATH) INTO WS-LINE
               MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Status::Caption
           END-IF.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM FDZ-VIEW2--ONFILESDROPPED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAYOUT-RAW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Raw" TO VWR-1::Layout.
           Txt-Log::AppendText("Layout -> Raw").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-LAYOUT-RAW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAYOUT-WEB--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Web" TO VWR-1::Layout.
           Txt-Log::AppendText("Layout -> Web").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-LAYOUT-WEB--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAYOUT-PRINT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Print" TO VWR-1::Layout.
           Txt-Log::AppendText("Layout -> Print").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-LAYOUT-PRINT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAYOUT-PAGE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Page" TO VWR-1::Layout.
           Txt-Log::AppendText("Layout -> Page").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-LAYOUT-PAGE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LAYOUT-STREAMED--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Streamed" TO VWR-1::Layout.
           Txt-Log::AppendText("Layout -> Streamed").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-LAYOUT-STREAMED--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FULLSCREEN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           IF VWR-1::Fullscreen = 1
               MOVE 0 TO VWR-1::Fullscreen
           ELSE
               MOVE 1 TO VWR-1::Fullscreen
           END-IF.
           Txt-Log::AppendText("Fullscreen toggled").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FULLSCREEN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SPLIT-NONE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "None" TO VWR-1::SplitMode.
           Txt-Log::AppendText("SplitMode -> None").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SPLIT-NONE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SPLIT-LR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "LeftRight" TO VWR-1::SplitMode.
           Txt-Log::AppendText("SplitMode -> LeftRight").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SPLIT-LR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SPLIT-TB--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "TopBottom" TO VWR-1::SplitMode.
           Txt-Log::AppendText("SplitMode -> TopBottom").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SPLIT-TB--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FILMSTRIP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           IF VWR-1::View1ShowFilmstrip = 1
               MOVE 0 TO VWR-1::View1ShowFilmstrip
           ELSE
               MOVE 1 TO VWR-1::View1ShowFilmstrip
           END-IF.
           Txt-Log::AppendText("Filmstrip toggled").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FILMSTRIP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FULL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Full" TO VWR-1::View1ViewMode.
           Txt-Log::AppendText("ViewMode -> Full").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FULL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CARDS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "Cards" TO VWR-1::View1ViewMode.
           Txt-Log::AppendText("ViewMode -> Cards").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-CARDS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ZOOM-OUT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(5).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           COMPUTE WS-N = VWR-1::View1Zoom - 25.
           IF WS-N < 25 MOVE 25 TO WS-N END-IF.
           MOVE WS-N TO VWR-1::View1Zoom.
           Txt-Log::AppendText("Zoom out").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-ZOOM-OUT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ZOOM-IN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(5).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> R12 caps magnification at 16x; the control clamps, so a
      *> runaway loop here can never ask for more than that.
           COMPUTE WS-N = VWR-1::View1Zoom + 25.
           MOVE WS-N TO VWR-1::View1Zoom.
           Txt-Log::AppendText("Zoom in").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-ZOOM-IN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ZOOM-100--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE 100 TO VWR-1::View1Zoom.
           Txt-Log::AppendText("Zoom reset").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-ZOOM-100--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FONT-UP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(5).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> R10: FontSize scales the text independently of Zoom.
           COMPUTE WS-N = VWR-1::FontSize + 2.
           IF WS-N > 40 MOVE 12 TO WS-N END-IF.
           MOVE WS-N TO VWR-1::FontSize.
           Txt-Log::AppendText("Font size changed").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FONT-UP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PREV-PAGE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(5).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           COMPUTE WS-N = VWR-1::View1Page - 1.
           IF WS-N < 1 MOVE 1 TO WS-N END-IF.
           MOVE WS-N TO VWR-1::View1Page.
           Txt-Log::AppendText("Page back").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-PREV-PAGE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEXT-PAGE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(5).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           COMPUTE WS-N = VWR-1::View1Page + 1.
           MOVE WS-N TO VWR-1::View1Page.
           Txt-Log::AppendText("Page forward").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-NEXT-PAGE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FIND--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-LINE PIC X(120).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE Txt-Find::Text TO VWR-1::View1SearchText.
           VWR-1::Find().
           STRING "matches: " VWR-1::View1SearchMatchCount INTO WS-LINE.
           MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Matches::Caption.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FIND--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FIND-NEXT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-LINE PIC X(120).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::FindNext().
           STRING VWR-1::View1SearchCurrentMatch " of "
                  VWR-1::View1SearchMatchCount INTO WS-LINE.
           MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Matches::Caption.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FIND-NEXT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FIND-PREV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-LINE PIC X(120).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::FindPrevious().
           STRING VWR-1::View1SearchCurrentMatch " of "
                  VWR-1::View1SearchMatchCount INTO WS-LINE.
           MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Matches::Caption.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FIND-PREV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-FIND-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::FindClose().
           Txt-Log::AppendText("Find closed").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-FIND-CLOSE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-CASE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           IF Chk-Case::Checked = 1
               MOVE 1 TO VWR-1::View1SearchCaseSensitive
           ELSE
               MOVE 0 TO VWR-1::View1SearchCaseSensitive
           END-IF.
           VWR-1::Find().
           Txt-Log::AppendText("Case sensitivity changed").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM CHK-CASE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CHK-HIGHLIGHT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           IF Chk-Highlight::Checked = 1
               MOVE 1 TO VWR-1::View1SearchHighlightEnabled
           ELSE
               MOVE 0 TO VWR-1::View1SearchHighlightEnabled
           END-IF.
           Txt-Log::AppendText("Highlight changed").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM CHK-HIGHLIGHT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> R18: Save As writes the ORIGINAL bytes, untouched. What is
      *> on screen is a derived reading of the stored form, never a
      *> replacement for it, so the copy is byte-identical.
           VWR-1::SaveAs().
           Txt-Log::AppendText("Save As requested").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PRINT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::Print().
           Txt-Log::AppendText("Print requested").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-PRINT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SHARE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::Share().
           Txt-Log::AppendText("Share requested").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SHARE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-JUMP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::JumpToLatest().
           Txt-Log::AppendText("Jumped to latest").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-JUMP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-STREAM-START--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> Streamed layout is one content pane and no chrome at all:
      *> the conversation IS the document. Each tick appends another
      *> chunk, and the Viewer keeps every chunk in its own native
      *> form, assembling the stream on demand.
           MOVE "Streamed" TO VWR-1::Layout.
           MOVE 0 TO Num-Chunks::Value.
           MOVE 1 TO Tmr-Stream::Enabled.
           MOVE "Streaming..." TO Lbl-Status::Caption.
           Txt-Log::AppendText("Stream started").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-STREAM-START--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-STREAM-STOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE 0 TO Tmr-Stream::Enabled.
           MOVE "Stream stopped." TO Lbl-Status::Caption.
           Txt-Log::AppendText("Stream stopped").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-STREAM-STOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-NEW-CONV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::NewConversation().
           MOVE 0 TO Num-Chunks::Value.
           Txt-Log::AppendText("New conversation").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-NEW-CONV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-REG-CONV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> History holds an id and a title, never the content. The
      *> host is asked for the text only when one is selected, which
      *> is what keeps a long session bounded.
           VWR-1::RegisterConversation("conv-archive", "Archived session").
           Txt-Log::AppendText("Registered conv-archive").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-REG-CONV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SEL-CONV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           VWR-1::SelectConversation("conv-archive").
           Txt-Log::AppendText("Selected conv-archive").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SEL-CONV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAMPLE-MD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           MOVE "assets/docs/viewer-sample.md" TO VWR-1::View1Source.
           MOVE "Page" TO VWR-1::Layout.
           Txt-Log::AppendText("Opened the Markdown sample").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SAMPLE-MD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAMPLE-TXT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> Plain text with real form-feed page breaks, so Previous and
      *> Next page move between pages the file itself declares (R9)
      *> rather than scrolling one continuous sheet.
           MOVE "assets/docs/viewer-sample.txt" TO VWR-1::View1Source.
           MOVE "Page" TO VWR-1::Layout.
           MOVE 1 TO VWR-1::View1Page.
           Txt-Log::AppendText("Opened the paged text sample").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SAMPLE-TXT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VWR-1--ONLOADED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-LINE PIC X(200).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           STRING "loaded, format: " VWR-1::Format INTO WS-LINE.
           MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Status::Caption.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM VWR-1--ONLOADED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VWR-1--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-LINE PIC X(400).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           STRING "error: " VWR-1::LastError INTO WS-LINE.
           MOVE FUNCTION TRIM(WS-LINE) TO Lbl-Status::Caption.
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM VWR-1--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VWR-1--ONCONVERSATIONSELECTED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> The control does not keep a past conversation's text. It
      *> asks, and the program answers - exactly the round trip the
      *> design calls for. Without this handler the pane would open
      *> empty, which is the documented behaviour, not a fault.
           VWR-1::AppendMarkdown("## Archived session\n\nRestored by the program when the entry was chosen. The Viewer stored only an id and a title.\n\n").
           Txt-Log::AppendText("Supplied the archived conversation").
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM VWR-1--ONCONVERSATIONSELECTED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TMR-STREAM--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-N PIC S9(6).
       01 WS-Q PIC S9(6).
       01 WS-KIND PIC S9(2).
       01 WS-NUM PIC 9(4).
       01 WS-CHUNK PIC X(900).
       PROCEDURE DIVISION.
           MOVE FUNCTION CHAR(11) TO WS-NL.
           COMPUTE WS-N = Num-Chunks::Value + 1.
           MOVE WS-N TO WS-NUM.
      *> Every chunk is Markdown, and the shape rotates so the stream
      *> exercises headings, prose, lists, a table and fenced code
      *> rather than one paragraph repeated.
           DIVIDE WS-N BY 4 GIVING WS-Q REMAINDER WS-KIND.
           EVALUATE WS-KIND
               WHEN 0
                   STRING "## Message " WS-NUM X"0A" X"0A"
                          "A streamed append. The Viewer keeps every "
                          "chunk in its own native form and assembles "
                          "the conversation on demand, so a session "
                          "that runs for hours stays bounded." X"0A" X"0A"
                       INTO WS-CHUNK
               WHEN 1
                   STRING "### Checklist " WS-NUM X"0A" X"0A"
                          "- [x] chunk stored in its own form" X"0A"
                          "- [x] rendered from that form" X"0A"
                          "- [ ] never converted on the way in" X"0A" X"0A"
                       INTO WS-CHUNK
               WHEN 2
                   STRING "| field | value |" X"0A"
                          "|---|---:|" X"0A"
                          "| message | " WS-NUM " |" X"0A"
                          "| layout | Streamed |" X"0A" X"0A"
                       INTO WS-CHUNK
               WHEN OTHER
                   STRING "```cobol" X"0A"
                          "           VWR-1::AppendMarkdown(WS-CHUNK)."
                          X"0A" "```" X"0A" X"0A"
                       INTO WS-CHUNK
           END-EVALUATE.
           VWR-1::AppendMarkdown(FUNCTION TRIM(WS-CHUNK)).
           MOVE WS-N TO Num-Chunks::Value.
      *> A long stream, on purpose: 150 chunks of mixed Markdown is a
      *> document of real size, which is the point of the exercise.
           IF WS-N >= 150
               MOVE 0 TO Tmr-Stream::Enabled
               MOVE "Stream complete - 150 messages." TO Lbl-Status::Caption
           END-IF.

           GOBACK.

       END PROGRAM TMR-STREAM--ONTICK.

       END PROGRAM VIEWER-FORM.

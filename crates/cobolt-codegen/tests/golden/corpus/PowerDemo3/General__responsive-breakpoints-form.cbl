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
       PROGRAM-ID. RESPONSIVE-BREAKPOINTS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-BREAKPOINTS-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).
       01 WS-TA-1 GLOBAL PIC X(44)
           VALUE "Phone:0:0.9;Tablet:600:0.95;Laptop:900:0.95;".
       01 WS-TA-2 GLOBAL PIC X(28)
           VALUE "Desktop:1200:1;Wide:1600:1.2".
       01 WS-TABLE GLOBAL PIC X(80).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-PNL-TOP.
          05 WS-PNL-TOP-TEXT       PIC X(256) VALUE 'PNL-TOP'.
          05 WS-PNL-TOP-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-TOP-ENABLED    PIC 9      VALUE 1.

       01 WS-BP-MENU.
          05 WS-BP-MENU-TEXT       PIC X(256) VALUE 'Menu'.
          05 WS-BP-MENU-VISIBLE    PIC 9      VALUE 0.
          05 WS-BP-MENU-ENABLED    PIC 9      VALUE 1.

       01 WS-BP-TITLE.
          05 WS-BP-TITLE-TEXT       PIC X(256) VALUE 'Breakpoints: one form, five layouts'.
          05 WS-BP-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-BP-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-BP-NOW.
          05 WS-BP-NOW-TEXT       PIC X(256) VALUE 'Now: Desktop'.
          05 WS-BP-NOW-VISIBLE    PIC 9      VALUE 1.
          05 WS-BP-NOW-ENABLED    PIC 9      VALUE 1.

       01 WS-BP-HELP.
          05 WS-BP-HELP-TEXT       PIC X(256) VALUE 'Help'.
          05 WS-BP-HELP-VISIBLE    PIC 9      VALUE 1.
          05 WS-BP-HELP-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-SIDE.
          05 WS-PNL-SIDE-TEXT       PIC X(256) VALUE 'PNL-SIDE'.
          05 WS-PNL-SIDE-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-SIDE-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-CAP.
          05 WS-SD-CAP-TEXT       PIC X(256) VALUE 'Pin a breakpoint from COBOL'.
          05 WS-SD-CAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-CAP-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-PHONE.
          05 WS-SD-PHONE-TEXT       PIC X(256) VALUE 'Phone'.
          05 WS-SD-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-PHONE-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-TABLET.
          05 WS-SD-TABLET-TEXT       PIC X(256) VALUE 'Tablet'.
          05 WS-SD-TABLET-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-TABLET-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-LAPTOP.
          05 WS-SD-LAPTOP-TEXT       PIC X(256) VALUE 'Laptop'.
          05 WS-SD-LAPTOP-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-LAPTOP-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-DESKTOP.
          05 WS-SD-DESKTOP-TEXT       PIC X(256) VALUE 'Desktop'.
          05 WS-SD-DESKTOP-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-DESKTOP-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-WIDE.
          05 WS-SD-WIDE-TEXT       PIC X(256) VALUE 'Wide'.
          05 WS-SD-WIDE-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-WIDE-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-AUTO.
          05 WS-SD-AUTO-TEXT       PIC X(256) VALUE 'Auto'.
          05 WS-SD-AUTO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-AUTO-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-SWAP.
          05 WS-SD-SWAP-TEXT       PIC X(256) VALUE 'Table B'.
          05 WS-SD-SWAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-SWAP-ENABLED    PIC 9      VALUE 1.

       01 WS-SD-RESTORE.
          05 WS-SD-RESTORE-TEXT       PIC X(256) VALUE 'Table A'.
          05 WS-SD-RESTORE-VISIBLE    PIC 9      VALUE 1.
          05 WS-SD-RESTORE-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-CONTENT.
          05 WS-PNL-CONTENT-TEXT       PIC X(256) VALUE 'PNL-CONTENT'.
          05 WS-PNL-CONTENT-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-CONTENT-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1.
          05 WS-CARD-1-TEXT       PIC X(256) VALUE 'CARD-1'.
          05 WS-CARD-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1-T.
          05 WS-CARD-1-T-TEXT       PIC X(256) VALUE 'Feature'.
          05 WS-CARD-1-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1-N.
          05 WS-CARD-1-N-TEXT       PIC X(256) VALUE 'This card spans two columns on Desktop, Tablet and Wide, and one on a Phone.'.
          05 WS-CARD-1-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2.
          05 WS-CARD-2-TEXT       PIC X(256) VALUE 'CARD-2'.
          05 WS-CARD-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2-T.
          05 WS-CARD-2-T-TEXT       PIC X(256) VALUE 'Sales'.
          05 WS-CARD-2-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2-N.
          05 WS-CARD-2-N-TEXT       PIC X(256) VALUE '1,284 orders this week.'.
          05 WS-CARD-2-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3.
          05 WS-CARD-3-TEXT       PIC X(256) VALUE 'CARD-3'.
          05 WS-CARD-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3-T.
          05 WS-CARD-3-T-TEXT       PIC X(256) VALUE 'Stock'.
          05 WS-CARD-3-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3-N.
          05 WS-CARD-3-N-TEXT       PIC X(256) VALUE '37 products below their minimum.'.
          05 WS-CARD-3-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4.
          05 WS-CARD-4-TEXT       PIC X(256) VALUE 'CARD-4'.
          05 WS-CARD-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4-T.
          05 WS-CARD-4-T-TEXT       PIC X(256) VALUE 'Deliveries'.
          05 WS-CARD-4-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4-N.
          05 WS-CARD-4-N-TEXT       PIC X(256) VALUE '12 on the road, 3 late.'.
          05 WS-CARD-4-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5.
          05 WS-CARD-5-TEXT       PIC X(256) VALUE 'CARD-5'.
          05 WS-CARD-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5-T.
          05 WS-CARD-5-T-TEXT       PIC X(256) VALUE 'Returns'.
          05 WS-CARD-5-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5-N.
          05 WS-CARD-5-N-TEXT       PIC X(256) VALUE 'Hidden on a Phone - laid out as absent.'.
          05 WS-CARD-5-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6.
          05 WS-CARD-6-TEXT       PIC X(256) VALUE 'CARD-6'.
          05 WS-CARD-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6-T.
          05 WS-CARD-6-T-TEXT       PIC X(256) VALUE 'Reviews'.
          05 WS-CARD-6-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6-N.
          05 WS-CARD-6-N-TEXT       PIC X(256) VALUE 'Hidden on a Phone too.'.
          05 WS-CARD-6-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-N-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-CONTACT.
          05 WS-BTN-CONTACT-TEXT       PIC X(256) VALUE 'Contact us'.
          05 WS-BTN-CONTACT-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-CONTACT-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-BREAKPOINTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-BREAKPOINTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-BREAKPOINTS-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-BREAKPOINTS-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-BREAKPOINTS-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "SD-PHONE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-PHONE--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-TABLET"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-TABLET--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-LAPTOP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-LAPTOP--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-DESKTOP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-DESKTOP--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-WIDE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-WIDE--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-AUTO"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-AUTO--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-SWAP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-SWAP--ONCLICK"
                       END-EVALUATE
                   WHEN "SD-RESTORE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SD-RESTORE--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-BREAKPOINTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "BPT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Now: " WS-BP DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO BP-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-BREAKPOINTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-BREAKPOINTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-BREAKPOINTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-BREAKPOINTS-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "BPT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Now: " WS-BP DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO BP-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-BREAKPOINTS-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-BREAKPOINTS-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "BPT-SHOW-INFO"
           MOVE SPACES TO WS-LINE
           STRING "Now: " WS-BP DELIMITED BY "  " INTO WS-LINE
           MOVE WS-LINE TO BP-NOW::Caption

           GOBACK.

       END PROGRAM RESPONSIVE-BREAKPOINTS-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-PHONE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Phone" TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-PHONE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-TABLET--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Tablet" TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-TABLET--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-LAPTOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Laptop" TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-LAPTOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-DESKTOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Desktop" TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-DESKTOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-WIDE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Wide" TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-WIDE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-AUTO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> SPACES unpins: the width chooses again.
           MOVE SPACES TO me::Breakpoint

           GOBACK.

       END PROGRAM SD-AUTO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-SWAP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *> A different table. The overrides of names it does not
      *> have are kept, but inactive until the names come back.
           MOVE "Narrow:0:1;Broad:800:1.15" TO me::Breakpoints

           GOBACK.

       END PROGRAM SD-SWAP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SD-RESTORE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           STRING WS-TA-1 WS-TA-2 DELIMITED BY SIZE INTO WS-TABLE
           MOVE WS-TABLE TO me::Breakpoints

           GOBACK.

       END PROGRAM SD-RESTORE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BPT-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM BPT-SHOW-INFO.

       END PROGRAM RESPONSIVE-BREAKPOINTS-FORM.

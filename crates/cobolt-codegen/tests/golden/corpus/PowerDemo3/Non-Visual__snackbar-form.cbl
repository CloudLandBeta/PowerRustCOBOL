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
       PROGRAM-ID. SNACKBAR-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SNACKBAR-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Label-TITLE.
          05 WS-Label-TITLE-TEXT       PIC X(256) VALUE 'Snackbar - every capability'.
          05 WS-Label-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-HINT.
          05 WS-Label-HINT-TEXT       PIC X(256) VALUE 'One control, one template: every Show() mints a NEW notification from the values current at that moment. They arrive one at a time. DISPLAY output goes to the IDE Output panel.'.
          05 WS-Label-HINT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-HINT-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-CATEGORIES.
          05 WS-Button-SNACK-CATEGORIES-TEXT       PIC X(256) VALUE '1  Five categories, one after another'.
          05 WS-Button-SNACK-CATEGORIES-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-CATEGORIES-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-TWOBUTTONS.
          05 WS-Button-SNACK-TWOBUTTONS-TEXT       PIC X(256) VALUE '2  Clear() + AddButton() - two buttons'.
          05 WS-Button-SNACK-TWOBUTTONS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-TWOBUTTONS-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-THREEBUTTONS.
          05 WS-Button-SNACK-THREEBUTTONS-TEXT       PIC X(256) VALUE '3  Three buttons - icon left, right, none'.
          05 WS-Button-SNACK-THREEBUTTONS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-THREEBUTTONS-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-SIZES.
          05 WS-Button-SNACK-SIZES-TEXT       PIC X(256) VALUE '4  Small . Medium . Large'.
          05 WS-Button-SNACK-SIZES-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-SIZES-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-ANCHOR.
          05 WS-Button-SNACK-ANCHOR-TEXT       PIC X(256) VALUE '5  Cycle the nine StackAnchor positions'.
          05 WS-Button-SNACK-ANCHOR-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-ANCHOR-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-ORDER.
          05 WS-Button-SNACK-ORDER-TEXT       PIC X(256) VALUE '6  StackOrder - Auto / NewestFirst / NewestLast'.
          05 WS-Button-SNACK-ORDER-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-ORDER-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-OVERFLOW.
          05 WS-Button-SNACK-OVERFLOW-TEXT       PIC X(256) VALUE '7  Overflow - Queue / DiscardOldest / DiscardNewest'.
          05 WS-Button-SNACK-OVERFLOW-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-OVERFLOW-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-STYLE.
          05 WS-Button-SNACK-STYLE-TEXT       PIC X(256) VALUE '8  Colours, corners, border and shadow by hand'.
          05 WS-Button-SNACK-STYLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-STYLE-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-CRITICAL.
          05 WS-Button-SNACK-CRITICAL-TEXT       PIC X(256) VALUE '9  Critical - 200 ms entrance, never expires'.
          05 WS-Button-SNACK-CRITICAL-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-CRITICAL-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-PIN.
          05 WS-Button-SNACK-PIN-TEXT       PIC X(256) VALUE '10  Pin three (Timeout 0) - dismiss the middle'.
          05 WS-Button-SNACK-PIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-PIN-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-WRAP.
          05 WS-Button-SNACK-WRAP-TEXT       PIC X(256) VALUE '11  Long text - wrap and the line budget'.
          05 WS-Button-SNACK-WRAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-WRAP-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-DISMISS.
          05 WS-Button-SNACK-DISMISS-TEXT       PIC X(256) VALUE '12  DismissAll()'.
          05 WS-Button-SNACK-DISMISS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-DISMISS-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-SNACK-RESET.
          05 WS-Button-SNACK-RESET-TEXT       PIC X(256) VALUE 'Reset the template to its designed defaults'.
          05 WS-Button-SNACK-RESET-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-SNACK-RESET-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-LASTCAP.
          05 WS-Label-LASTCAP-TEXT       PIC X(256) VALUE 'Last button click:'.
          05 WS-Label-LASTCAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-LASTCAP-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-LAST.
          05 WS-Label-LAST-TEXT       PIC X(256) VALUE '(nothing yet)'.
          05 WS-Label-LAST-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-LAST-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-FOOT.
          05 WS-Label-FOOT-TEXT       PIC X(256) VALUE 'Notifications anchor to THIS form''s own surface. The entrance grows from one pixel over 600 ms - 200 ms for Critical; movement and fade-out take 300 ms.'.
          05 WS-Label-FOOT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-FOOT-ENABLED    PIC 9      VALUE 1.

       01 WS-Snackbar-1.
          05 WS-Snackbar-1-TEXT       PIC X(256) VALUE 'Snackbar-1'.
          05 WS-Snackbar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Snackbar-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SNACKBAR-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SNACKBAR-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Button-SNACK-CATEGORIES"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-CATEGORIES--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-TWOBUTTONS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-TWOBUTTONS--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-THREEBUTTONS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-THREEBUTTONS--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-SIZES"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-SIZES--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-ANCHOR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-ANCHOR--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-ORDER"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-ORDER--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-OVERFLOW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-OVERFLOW--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-STYLE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-STYLE--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-CRITICAL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-CRITICAL--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-PIN"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-PIN--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-WRAP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-WRAP--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-DISMISS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-DISMISS--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-SNACK-RESET"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-SNACK-RESET--ONCLICK"
                       END-EVALUATE
                   WHEN "Snackbar-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onShown"
                               CALL "SNACKBAR-1--ONSHOWN"
                           WHEN "onTimeout"
                               CALL "SNACKBAR-1--ONTIMEOUT"
                           WHEN "onClosing"
                               CALL "SNACKBAR-1--ONCLOSING"
                           WHEN "onClosed"
                               CALL "SNACKBAR-1--ONCLOSED"
                           WHEN "onButtonClick"
                               CALL "SNACKBAR-1--ONBUTTONCLICK"
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
       PROGRAM-ID. SNACKBAR-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Nothing to set up: the Snackbar is NON-VISUAL and paints nothing
      *> where it sits. Every button below mints its own notification from
      *> the template's values at the moment it calls Show().

           DISPLAY "snackbar-form ready - 12 demonstrations".

           GOBACK.

       END PROGRAM SNACKBAR-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SNACKBAR-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-CATEGORIES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 1 - THE FIVE CATEGORIES.
      *> Category supplies the background, the ink, the icon AND the timeout
      *> for everything you did not set yourself. Timeout -1 means "take the
      *> category's own": 4000 / 6000 / 6000 / 8000, and Critical's 0, which
      *> is why the last one stays until it is dismissed.
      *> They arrive ONE AT A TIME - the second waits for the first to land.

           INVOKE SNACKBAR-1::Clear()
           MOVE -1 TO SNACKBAR-1::Timeout

           MOVE "Info" TO SNACKBAR-1::Category
           MOVE "Record saved." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Question" TO SNACKBAR-1::Category
           MOVE "Overwrite the existing file?" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Warning" TO SNACKBAR-1::Category
           MOVE "Index rebuilt with warnings." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Error" TO SNACKBAR-1::Category
           MOVE "Could not reach the server." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Critical" TO SNACKBAR-1::Category
           MOVE "Disk almost full - 2 per cent free." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           DISPLAY "1 - five categories raised; Critical stays until dismissed".

           GOBACK.

       END PROGRAM BUTTON-SNACK-CATEGORIES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-TWOBUTTONS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 2 - CLEAR() AND ADDBUTTON().
      *> Buttons is ONE LINE PER BUTTON and a COBOL literal cannot carry a
      *> newline, so a MOVE into Buttons can only ever declare ONE. Declare
      *> them a call at a time instead. Clear() empties the row and NOTHING
      *> else - the Text and Category below still apply.
      *> position is the ordinal, 1-based, left to right.
      *> dismiss=false leaves the notification up when that button is clicked.

           INVOKE SNACKBAR-1::Clear()
           INVOKE SNACKBAR-1::AddButton("id=undo,caption=Undo,icon=undo,position=1")
           INVOKE SNACKBAR-1::AddButton("id=later,caption=Later,position=2,dismiss=false")

           MOVE "Warning" TO SNACKBAR-1::Category
           MOVE "Saved. Undo?" TO SNACKBAR-1::Text
           MOVE 0 TO SNACKBAR-1::Timeout
           INVOKE SNACKBAR-1::Show()

           DISPLAY "2 - Undo dismisses, Later stays up; watch onButtonClick".

           GOBACK.

       END PROGRAM BUTTON-SNACK-TWOBUTTONS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-THREEBUTTONS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 3 - THREE BUTTONS, AND THE ICON POSITIONS.
      *> Three is the limit; a fourth is reported, never silently dropped.
      *> iconposition is None | Left | Right, and defaults to Left when an
      *> icon is given. A button with an icon and NO caption is icon-only.

           INVOKE SNACKBAR-1::Clear()
           INVOKE SNACKBAR-1::AddButton("id=retry,caption=Retry,icon=refresh,iconposition=Left,position=1")
           INVOKE SNACKBAR-1::AddButton("id=help,caption=Help,icon=check,iconposition=Right,position=2")
           INVOKE SNACKBAR-1::AddButton("id=close,icon=x-mark,position=3")

           MOVE "Error" TO SNACKBAR-1::Category
           MOVE "Could not reach the server." TO SNACKBAR-1::Text
           MOVE 0 TO SNACKBAR-1::Timeout
           INVOKE SNACKBAR-1::Show()

           DISPLAY "3 - icon Left, icon Right, icon-only".

           GOBACK.

       END PROGRAM BUTTON-SNACK-THREEBUTTONS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-SIZES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 4 - THE THREE SIZE CLASSES.
      *> Size fixes the padding, the font, the button height and how many
      *> lines the text may use before it is ellipsized.

           INVOKE SNACKBAR-1::Clear()
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Info" TO SNACKBAR-1::Category

           MOVE "Small" TO SNACKBAR-1::Size
           MOVE "Small - compact, one line." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Medium" TO SNACKBAR-1::Size
           MOVE "Medium - the default size class." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Large" TO SNACKBAR-1::Size
           MOVE "Large - room for a longer explanation." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Medium" TO SNACKBAR-1::Size
           DISPLAY "4 - Small, Medium, Large; template left on Medium".

           GOBACK.

       END PROGRAM BUTTON-SNACK-SIZES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-ANCHOR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ANCHOR PIC X(16).
       01 WS-MSG    PIC X(32).
       01 WS-OUT    PIC X(64).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 5 - THE NINE ANCHOR POSITIONS.
      *> Each press moves to the next. A TOP anchor grows DOWNWARD with the
      *> newest at the top; a BOTTOM anchor grows UPWARD. The anchor is
      *> resolved against THIS FORM'S OWN SURFACE, never the desktop.

           MOVE SNACKBAR-1::StackAnchor TO WS-ANCHOR
           EVALUATE FUNCTION TRIM(WS-ANCHOR)
               WHEN "BottomRight"  MOVE "BottomCenter" TO WS-ANCHOR
               WHEN "BottomCenter" MOVE "BottomLeft"   TO WS-ANCHOR
               WHEN "BottomLeft"   MOVE "CenterLeft"   TO WS-ANCHOR
               WHEN "CenterLeft"   MOVE "Center"       TO WS-ANCHOR
               WHEN "Center"       MOVE "CenterRight"  TO WS-ANCHOR
               WHEN "CenterRight"  MOVE "TopRight"     TO WS-ANCHOR
               WHEN "TopRight"     MOVE "TopCenter"    TO WS-ANCHOR
               WHEN "TopCenter"    MOVE "TopLeft"      TO WS-ANCHOR
               WHEN OTHER          MOVE "BottomRight"  TO WS-ANCHOR
           END-EVALUATE
           MOVE FUNCTION TRIM(WS-ANCHOR) TO SNACKBAR-1::StackAnchor

           INVOKE SNACKBAR-1::Clear()
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Info" TO SNACKBAR-1::Category
           MOVE "Oldest of the run" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "Newest of the run" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "5 - StackAnchor = " TO WS-MSG
           STRING FUNCTION TRIM(WS-MSG) DELIMITED BY SIZE
                  FUNCTION TRIM(WS-ANCHOR) DELIMITED BY SIZE
                  INTO WS-OUT
           MOVE FUNCTION TRIM(WS-OUT) TO LABEL-LAST::Caption
           DISPLAY FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-SNACK-ANCHOR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-ORDER--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ORDER PIC X(16).
       01 WS-MSG   PIC X(32).
       01 WS-OUT   PIC X(64).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 6 - STACKORDER.
      *> Auto follows the anchor. NewestFirst and NewestLast override it, so
      *> you can put the newest at the top of a bottom-anchored run.

           MOVE SNACKBAR-1::StackOrder TO WS-ORDER
           EVALUATE FUNCTION TRIM(WS-ORDER)
               WHEN "Auto"        MOVE "NewestFirst" TO WS-ORDER
               WHEN "NewestFirst" MOVE "NewestLast"  TO WS-ORDER
               WHEN OTHER         MOVE "Auto"        TO WS-ORDER
           END-EVALUATE
           MOVE FUNCTION TRIM(WS-ORDER) TO SNACKBAR-1::StackOrder

           INVOKE SNACKBAR-1::Clear()
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Info" TO SNACKBAR-1::Category
           MOVE "1 - raised first" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "2 - raised second" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "3 - raised last" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "6 - StackOrder = " TO WS-MSG
           STRING FUNCTION TRIM(WS-MSG) DELIMITED BY SIZE
                  FUNCTION TRIM(WS-ORDER) DELIMITED BY SIZE
                  INTO WS-OUT
           MOVE FUNCTION TRIM(WS-OUT) TO LABEL-LAST::Caption
           DISPLAY FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-SNACK-ORDER--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-OVERFLOW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-OVF PIC X(16).
       01 WS-MSG PIC X(48).
       01 WS-OUT PIC X(80).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 7 - MAXIMUMVISIBLE AND OVERFLOWBEHAVIOR.
      *> Two at a time, four raised. Queue holds the surplus and lets it in as
      *> room frees; DiscardOldest closes the oldest to make room and reports
      *> it as Overflow; DiscardNewest drops the arrival, which never appears.

           MOVE SNACKBAR-1::OverflowBehavior TO WS-OVF
           EVALUATE FUNCTION TRIM(WS-OVF)
               WHEN "Queue"         MOVE "DiscardOldest" TO WS-OVF
               WHEN "DiscardOldest" MOVE "DiscardNewest" TO WS-OVF
               WHEN OTHER           MOVE "Queue"         TO WS-OVF
           END-EVALUATE
           MOVE FUNCTION TRIM(WS-OVF) TO SNACKBAR-1::OverflowBehavior
           MOVE 2 TO SNACKBAR-1::MaximumVisible

           INVOKE SNACKBAR-1::Clear()
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Warning" TO SNACKBAR-1::Category
           MOVE "1 of 4" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "2 of 4" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "3 of 4" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "4 of 4" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "7 - MaximumVisible 2, OverflowBehavior " TO WS-MSG
           STRING FUNCTION TRIM(WS-MSG) DELIMITED BY SIZE
                  FUNCTION TRIM(WS-OVF) DELIMITED BY SIZE
                  INTO WS-OUT
           MOVE FUNCTION TRIM(WS-OUT) TO LABEL-LAST::Caption
           DISPLAY FUNCTION TRIM(WS-OUT).

           GOBACK.

       END PROGRAM BUTTON-SNACK-OVERFLOW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-STYLE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 8 - EVERY COLOUR AND SHAPE SET BY HAND.
      *> Leave a colour EMPTY to mean "the category decides"; set one and it
      *> wins ALONE, so the category's icon and ink stay put unless you also
      *> set those. On a Snackbar, ShadowBlur is a RADIUS IN PIXELS and
      *> ShadowDirection is DEGREES clockwise from up - not the compass string
      *> the rest of the catalogue uses.

           INVOKE SNACKBAR-1::Clear()
           INVOKE SNACKBAR-1::AddButton("id=ok,caption=Got it,icon=check")

           MOVE "#2B3B55FF" TO SNACKBAR-1::BackgroundColor
           MOVE "#E8F0FFFF" TO SNACKBAR-1::ForegroundColor
           MOVE "#70F3FCFF" TO SNACKBAR-1::CategoryIconColor
           MOVE 24 TO SNACKBAR-1::CornerRadius
           MOVE "Single" TO SNACKBAR-1::BorderStyle
           MOVE "#70F3FCFF" TO SNACKBAR-1::BorderColor
           MOVE 2 TO SNACKBAR-1::BorderWidth
           MOVE 45 TO SNACKBAR-1::ShadowOpacity
           MOVE 18 TO SNACKBAR-1::ShadowBlur
           MOVE 6 TO SNACKBAR-1::ShadowDistance
           MOVE 270 TO SNACKBAR-1::ShadowDirection

           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Brand colours, 24pt corners, a cyan rim." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           DISPLAY "8 - styled by hand; press Reset to go back to the design".

           GOBACK.

       END PROGRAM BUTTON-SNACK-STYLE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-CRITICAL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 9 - CRITICAL.
      *> The only category that changes an EFFECT: it enters in 200 ms rather
      *> than 600, because the most urgent message should already be there
      *> when the operator looks up. Its own default Timeout is 0, so -1
      *> ("take the category's") leaves it up until it is dismissed.

           INVOKE SNACKBAR-1::Clear()
           INVOKE SNACKBAR-1::AddButton("id=ack,caption=Acknowledge,icon=check")

           MOVE "Critical" TO SNACKBAR-1::Category
           MOVE -1 TO SNACKBAR-1::Timeout
           MOVE "Disk almost full - 2 per cent free." TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           DISPLAY "9 - Critical: 200 ms entrance, never expires by itself".

           GOBACK.

       END PROGRAM BUTTON-SNACK-CRITICAL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-PIN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 10 - PINNED, AND THE REFLOW.
      *> Timeout 0 never expires. Dismiss the MIDDLE one and watch the two
      *> survivors GLIDE into the gap over 300 ms rather than snapping.
      *> Hold the pointer over one and its timeout pauses - with Timeout 0
      *> there is nothing to pause, so raise these with a timeout to see it.

           INVOKE SNACKBAR-1::Clear()
           INVOKE SNACKBAR-1::AddButton("id=close,caption=Close,icon=x-mark")
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Info" TO SNACKBAR-1::Category

           MOVE "Pinned 1 - dismiss the MIDDLE one" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "Pinned 2 - this is the middle" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()
           MOVE "Pinned 3 - the newest" TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           DISPLAY "10 - three pinned; close the middle and watch the glide".

           GOBACK.

       END PROGRAM BUTTON-SNACK-PIN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-WRAP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 11 - LONG TEXT: WRAP, AND THE LINE BUDGET.
      *> Each size class allows a different number of lines before the text is
      *> ellipsized. The same sentence is shown Large then Small.
      *> Text is DATA, never a format string - build it with STRING or MOVE.

           INVOKE SNACKBAR-1::Clear()
           MOVE 0 TO SNACKBAR-1::Timeout
           MOVE "Warning" TO SNACKBAR-1::Category

           MOVE "Large" TO SNACKBAR-1::Size
           MOVE "The nightly rebuild finished with 3 warnings: two indexes we"
             TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Small" TO SNACKBAR-1::Size
           MOVE "The nightly rebuild finished with 3 warnings: two indexes we"
             TO SNACKBAR-1::Text
           INVOKE SNACKBAR-1::Show()

           MOVE "Medium" TO SNACKBAR-1::Size
           DISPLAY "11 - same text, Large then Small; the budget cuts it".

           GOBACK.

       END PROGRAM BUTTON-SNACK-WRAP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-DISMISS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> 12 - DISMISSALL().
      *> Every notification THIS control raised, with reason Programmatic, and
      *> anything it had queued is discarded too. Other Snackbar controls on
      *> the form are untouched. There is no Hide(): with Show() as a factory
      *> it could not say WHICH notification it meant.

           INVOKE SNACKBAR-1::DismissAll()
           MOVE "12 - DismissAll(): all cleared, reason Programmatic"
             TO LABEL-LAST::Caption
           DISPLAY "12 - DismissAll(): reason Programmatic".

           GOBACK.

       END PROGRAM BUTTON-SNACK-DISMISS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-SNACK-RESET--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> RESET THE TEMPLATE.
      *> Show() is a snapshot, so this changes nothing already on screen - it
      *> puts the TEMPLATE back where the Form Designer left it.

           INVOKE SNACKBAR-1::DismissAll()
           INVOKE SNACKBAR-1::Clear()

           MOVE "Info" TO SNACKBAR-1::Category
           MOVE "Medium" TO SNACKBAR-1::Size
           MOVE -1 TO SNACKBAR-1::Timeout
           MOVE 5 TO SNACKBAR-1::MaximumVisible
           MOVE "Queue" TO SNACKBAR-1::OverflowBehavior
           MOVE "BottomRight" TO SNACKBAR-1::StackAnchor
           MOVE "Auto" TO SNACKBAR-1::StackOrder
           MOVE SPACES TO SNACKBAR-1::BackgroundColor
           MOVE SPACES TO SNACKBAR-1::ForegroundColor
           MOVE SPACES TO SNACKBAR-1::CategoryIconColor
           MOVE 12 TO SNACKBAR-1::CornerRadius
           MOVE "None" TO SNACKBAR-1::BorderStyle
           MOVE 12 TO SNACKBAR-1::ShadowBlur
           MOVE 25 TO SNACKBAR-1::ShadowOpacity
           MOVE 4 TO SNACKBAR-1::ShadowDistance
           MOVE 270 TO SNACKBAR-1::ShadowDirection
           MOVE "Record saved" TO SNACKBAR-1::Text

           MOVE "Template reset to its designed defaults"
             TO LABEL-LAST::Caption
           DISPLAY "reset - template back to the designed defaults".

           GOBACK.

       END PROGRAM BUTTON-SNACK-RESET--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONSHOWN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Fires when the message joins the stack - which is BEFORE it has
      *> waited its turn in the arrival queue and before it has finished
      *> growing in. No event ever waits on an animation.
           DISPLAY "onShown".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONSHOWN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONTIMEOUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Its time ran out. Fires BEFORE onClosing.
           DISPLAY "onTimeout".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONCLOSING IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> About to leave. The reason (Timeout / User / Action / Programmatic /
      *> Overflow) travels with the event.
           DISPLAY "onClosing".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONCLOSING.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONCLOSED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Gone. Both this and onClosing fire the MOMENT it closes - the slot it
      *> held is free for the next Show() immediately, and only the picture
      *> lingers for 300 ms.
           DISPLAY "onClosed".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONCLOSED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONBUTTONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ID  PIC X(32).
       01 WS-IX  PIC X(8).
       01 WS-MSG PIC X(16).
       01 WS-OUT PIC X(80).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> WHICH button was pressed: LastButtonId is your own id from the
      *> AddButton spec, LastButtonIndex is its 0-based place in the row.
      *> Both are written BEFORE this handler runs.
      *> A button whose dismiss is true closes the notification AFTER this
      *> fires, so you can still read the message it was clicked on.

           MOVE SNACKBAR-1::LastButtonId TO WS-ID
           MOVE SNACKBAR-1::LastButtonIndex TO WS-IX

           EVALUATE FUNCTION TRIM(WS-ID)
               WHEN "undo"  DISPLAY "onButtonClick: undo - undoing"
               WHEN "later" DISPLAY "onButtonClick: later - left up"
               WHEN "retry" DISPLAY "onButtonClick: retry - retrying"
               WHEN "close" DISPLAY "onButtonClick: close"
               WHEN "ack"   DISPLAY "onButtonClick: acknowledged"
               WHEN OTHER   DISPLAY "onButtonClick: " FUNCTION TRIM(WS-ID)
           END-EVALUATE

           MOVE "button " TO WS-MSG
           STRING FUNCTION TRIM(WS-MSG) DELIMITED BY SIZE
                  FUNCTION TRIM(WS-ID)  DELIMITED BY SIZE
                  " at index "          DELIMITED BY SIZE
                  FUNCTION TRIM(WS-IX)  DELIMITED BY SIZE
                  INTO WS-OUT
           MOVE FUNCTION TRIM(WS-OUT) TO LABEL-LAST::Caption.

           GOBACK.

       END PROGRAM SNACKBAR-1--ONBUTTONCLICK.

       END PROGRAM SNACKBAR-FORM.

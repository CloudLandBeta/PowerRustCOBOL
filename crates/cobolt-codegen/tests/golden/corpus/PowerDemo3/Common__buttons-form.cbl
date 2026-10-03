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
       PROGRAM-ID. BUTTONS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'BUTTONS-FORM'.

      *>── Animation runtime fields ──────────────────────────────────
      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME
       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.
       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Button-1.
          05 WS-Button-1-TEXT       PIC X(256) VALUE 'Button-1'.
          05 WS-Button-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-2.
          05 WS-Button-2-TEXT       PIC X(256) VALUE 'Button-2'.
          05 WS-Button-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-3.
          05 WS-Button-3-TEXT       PIC X(256) VALUE 'Button-3'.
          05 WS-Button-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-4.
          05 WS-Button-4-TEXT       PIC X(256) VALUE 'Button-4'.
          05 WS-Button-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-4-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Label-1'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-5.
          05 WS-Button-5-TEXT       PIC X(256) VALUE 'Button with icon 1'.
          05 WS-Button-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-5-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-6.
          05 WS-Button-6-TEXT       PIC X(256) VALUE 'Button with icon 2'.
          05 WS-Button-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-6-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-7.
          05 WS-Button-7-TEXT       PIC X(256) VALUE 'Button with icon 3'.
          05 WS-Button-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-7-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-8.
          05 WS-Button-8-TEXT       PIC X(256) VALUE 'Button with icon 4'.
          05 WS-Button-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-8-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-9.
          05 WS-Button-9-TEXT       PIC X(256) VALUE 'Button with icon 5'.
          05 WS-Button-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-9-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-10.
          05 WS-Button-10-TEXT       PIC X(256) VALUE 'Button with icon 6'.
          05 WS-Button-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-10-ENABLED    PIC 9      VALUE 1.

       01 WS-PictureBox-1.
          05 WS-PictureBox-1-TEXT       PIC X(256) VALUE 'PictureBox-1'.
          05 WS-PictureBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "BUTTONS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "BUTTONS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Button-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-1--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-2--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-3"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-3--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-4"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-4--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-5"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-5--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-6"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-6--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-7"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onHoverEnter"
                               CALL "BUTTON-7--ONHOVERENTER"
                           WHEN "onHoverLeave"
                               CALL "BUTTON-7--ONHOVERLEAVE"
                       END-EVALUATE
                   WHEN "Button-8"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-8--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-9"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-9--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-10"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-10--ONCLICK"
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
       COBOL-PLAY-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           EVALUATE WS-ANIM-NAME
               WHEN "bt1"
                   INVOKE Button-1 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-2 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "anim2"
                   INVOKE Button-3 'PlayAnimation'
                       USING BY VALUE "anim2"
               WHEN "1"
                   INVOKE Button-4 'PlayAnimation'
                       USING BY VALUE "1"
               WHEN "bt1"
                   INVOKE Button-5 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-6 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-7 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-8 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-9 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-10 'PlayAnimation'
                       USING BY VALUE "bt1"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       COBOL-STOP-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           EVALUATE WS-ANIM-NAME
               WHEN "bt1"
                   INVOKE Button-1 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-2 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "anim2"
                   INVOKE Button-3 'StopAnimation'
                       USING BY VALUE "anim2"
               WHEN "1"
                   INVOKE Button-4 'StopAnimation'
                       USING BY VALUE "1"
               WHEN "bt1"
                   INVOKE Button-5 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-6 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-7 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-8 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-9 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN "bt1"
                   INVOKE Button-10 'StopAnimation'
                       USING BY VALUE "bt1"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       Button-1-PLAY-BT1.
           INVOKE Button-1 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-2-PLAY-BT1.
           INVOKE Button-2 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-3-PLAY-ANIM2.
           INVOKE Button-3 'PlayAnimation'
               USING BY VALUE "anim2".

       Button-4-PLAY-1.
           INVOKE Button-4 'PlayAnimation'
               USING BY VALUE "1".

       Button-5-PLAY-BT1.
           INVOKE Button-5 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-6-PLAY-BT1.
           INVOKE Button-6 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-7-PLAY-BT1.
           INVOKE Button-7 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-8-PLAY-BT1.
           INVOKE Button-8 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-9-PLAY-BT1.
           INVOKE Button-9 'PlayAnimation'
               USING BY VALUE "bt1".

       Button-10-PLAY-BT1.
           INVOKE Button-10 'PlayAnimation'
               USING BY VALUE "bt1".


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTONS-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTONS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTONS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTONS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           Label-1::SetCaption("Button-1 clicked!").

           GOBACK.

       END PROGRAM BUTTON-1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           Label-1::SetCaption("Button-2 clicked!").

           GOBACK.

       END PROGRAM BUTTON-2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-3--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           *> Deliberately uses this control's own identifier, Button-3,
           *> and NOT its Caption property (Caption is mismatched to
           *> 'Button-2' on this control and must not be propagated).
           Label-1::SetCaption("Button-3 with wait cursor (don't) clicked!").

           GOBACK.

       END PROGRAM BUTTON-3--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-4--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           Label-1::SetCaption("Button-4 clicked!").

           GOBACK.

       END PROGRAM BUTTON-4--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-5--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Button with icon 1 clicked!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-5--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-6--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Button with icon 2 clicked!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-6--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-7--ONHOVERENTER IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Hover Button with icon 3!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-7--ONHOVERENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-7--ONHOVERLEAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Label 1"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-7--ONHOVERLEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-8--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Button with icon 4 clicked!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-8--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-9--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Button with icon 5 clicked!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-9--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-10--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           set Label-1::Caption to "Button with icon 6 clicked!"
           CONTINUE.

           GOBACK.

       END PROGRAM BUTTON-10--ONCLICK.

       END PROGRAM BUTTONS-FORM.

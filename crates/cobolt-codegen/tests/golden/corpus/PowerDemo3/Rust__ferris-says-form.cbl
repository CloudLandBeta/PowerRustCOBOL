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
       PROGRAM-ID. FERRIS-SAYS-FORM.

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
           CLASS RUST-RANGE IS "Rust.Range"

           EXEC RUST

				use ferris_says::say;

				pub fn ferris_say(out: &str) -> String {
				    let width = 24;
				    let mut buffer = Vec::new();

				    say(out, width, &mut buffer).unwrap();

    				String::from_utf8(buffer).unwrap()
				}

		   END-EXEC.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'FERRIS-SAYS-FORM'.

      *>── Animation runtime fields ──────────────────────────────────
      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME
       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.
       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Button-1.
          05 WS-Button-1-TEXT       PIC X(256) VALUE 'Send to Rust'.
          05 WS-Button-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-1.
          05 WS-TextBox-1-TEXT       PIC X(40) VALUE 'TextBox-1'.
          05 WS-TextBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-1-VALUE      PIC X(40) VALUE SPACES.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE '...'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-2.
          05 WS-Label-2-TEXT       PIC X(256) VALUE 'Type your message'.
          05 WS-Label-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-2-ENABLED    PIC 9      VALUE 1.

       01 WS-PictureBox-1.
          05 WS-PictureBox-1-TEXT       PIC X(256) VALUE 'PictureBox-1'.
          05 WS-PictureBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-1-ENABLED    PIC 9      VALUE 1.

       01 WS-PictureBox-2.
          05 WS-PictureBox-2-TEXT       PIC X(256) VALUE 'PictureBox-2'.
          05 WS-PictureBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-3.
          05 WS-Label-3-TEXT       PIC X(256) VALUE '+'.
          05 WS-Label-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-4.
          05 WS-Label-4-TEXT       PIC X(256) VALUE 'Rust result'.
          05 WS-Label-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-4-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "FERRIS-SAYS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "FERRIS-SAYS-FORM--ONCLOSE"
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
               WHEN "rust"
                   INVOKE PictureBox-1 'PlayAnimation' USING BY VALUE "rust"
               WHEN "chibiin"
                   INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "chibiin"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       COBOL-STOP-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           EVALUATE WS-ANIM-NAME
               WHEN "rust"
                   INVOKE PictureBox-1 'StopAnimation' USING BY VALUE "rust"
               WHEN "chibiin"
                   INVOKE PictureBox-2 'StopAnimation' USING BY VALUE "chibiin"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       PictureBox-1-PLAY-RUST.
           INVOKE PictureBox-1 'PlayAnimation' USING BY VALUE "rust".

       PictureBox-2-PLAY-CHIBIIN.
           INVOKE PictureBox-2 'PlayAnimation' USING BY VALUE "chibiin".


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FERRIS-SAYS-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM FERRIS-SAYS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FERRIS-SAYS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM FERRIS-SAYS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 cobol-text  USAGE IS OBJECT REFERENCE RUST-STRING.
       01 rust-result USAGE IS OBJECT REFERENCE RUST-STRING.

       LINKAGE SECTION.

       PROCEDURE DIVISION.
       MAIN.
           SET cobol-text TO TextBox-1::Text

           TRY
               EXEC RUST
                   *rust_result = ferris_say(cobol_text);
               END-EXEC

               SET Label-1::Caption TO rust-result


           CATCH RUST-EXCEPTION ws-error
               DISPLAY "FFI failed: " ws-error
           END-TRY

           GOBACK.

           GOBACK.

       END PROGRAM BUTTON-1--ONCLICK.

       END PROGRAM FERRIS-SAYS-FORM.

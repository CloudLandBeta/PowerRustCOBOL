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
       PROGRAM-ID. LABELS-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'LABELS-FORM'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-01.
          05 WS-LBL-01-TEXT       PIC X(256) VALUE 'Lorem'.
          05 WS-LBL-01-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-01-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-02.
          05 WS-LBL-02-TEXT       PIC X(256) VALUE 'ipsum'.
          05 WS-LBL-02-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-02-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-03.
          05 WS-LBL-03-TEXT       PIC X(256) VALUE 'dolor'.
          05 WS-LBL-03-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-03-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-04.
          05 WS-LBL-04-TEXT       PIC X(256) VALUE 'sit'.
          05 WS-LBL-04-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-04-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-05.
          05 WS-LBL-05-TEXT       PIC X(256) VALUE 'amet'.
          05 WS-LBL-05-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-05-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-06.
          05 WS-LBL-06-TEXT       PIC X(256) VALUE 'consectetur'.
          05 WS-LBL-06-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-06-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-07.
          05 WS-LBL-07-TEXT       PIC X(256) VALUE 'adipiscing'.
          05 WS-LBL-07-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-07-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-08.
          05 WS-LBL-08-TEXT       PIC X(256) VALUE 'elit'.
          05 WS-LBL-08-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-08-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-09.
          05 WS-LBL-09-TEXT       PIC X(256) VALUE 'sed'.
          05 WS-LBL-09-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-09-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-10.
          05 WS-LBL-10-TEXT       PIC X(256) VALUE 'do'.
          05 WS-LBL-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-10-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-11.
          05 WS-LBL-11-TEXT       PIC X(256) VALUE 'eiusmod'.
          05 WS-LBL-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-11-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-12.
          05 WS-LBL-12-TEXT       PIC X(256) VALUE 'tempor'.
          05 WS-LBL-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-12-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-13.
          05 WS-LBL-13-TEXT       PIC X(256) VALUE 'incididunt'.
          05 WS-LBL-13-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-13-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-14.
          05 WS-LBL-14-TEXT       PIC X(256) VALUE 'ut'.
          05 WS-LBL-14-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-14-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-15.
          05 WS-LBL-15-TEXT       PIC X(256) VALUE 'labore'.
          05 WS-LBL-15-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-15-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-16.
          05 WS-LBL-16-TEXT       PIC X(256) VALUE 'et'.
          05 WS-LBL-16-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-16-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-17.
          05 WS-LBL-17-TEXT       PIC X(256) VALUE 'dolore'.
          05 WS-LBL-17-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-17-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-18.
          05 WS-LBL-18-TEXT       PIC X(256) VALUE 'magna'.
          05 WS-LBL-18-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-18-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-19.
          05 WS-LBL-19-TEXT       PIC X(256) VALUE 'aliqua'.
          05 WS-LBL-19-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-19-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-20.
          05 WS-LBL-20-TEXT       PIC X(256) VALUE 'Ut'.
          05 WS-LBL-20-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-20-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-21.
          05 WS-LBL-21-TEXT       PIC X(256) VALUE 'enim'.
          05 WS-LBL-21-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-21-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-22.
          05 WS-LBL-22-TEXT       PIC X(256) VALUE 'ad'.
          05 WS-LBL-22-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-22-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-23.
          05 WS-LBL-23-TEXT       PIC X(256) VALUE 'minim'.
          05 WS-LBL-23-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-23-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-24.
          05 WS-LBL-24-TEXT       PIC X(256) VALUE 'veniam'.
          05 WS-LBL-24-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-24-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-25.
          05 WS-LBL-25-TEXT       PIC X(256) VALUE 'quis'.
          05 WS-LBL-25-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-25-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-26.
          05 WS-LBL-26-TEXT       PIC X(256) VALUE 'nostrud'.
          05 WS-LBL-26-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-26-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-27.
          05 WS-LBL-27-TEXT       PIC X(256) VALUE 'exercitation'.
          05 WS-LBL-27-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-27-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-28.
          05 WS-LBL-28-TEXT       PIC X(256) VALUE 'ullamco'.
          05 WS-LBL-28-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-28-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-29.
          05 WS-LBL-29-TEXT       PIC X(256) VALUE 'laboris'.
          05 WS-LBL-29-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-29-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-30.
          05 WS-LBL-30-TEXT       PIC X(256) VALUE 'nisi'.
          05 WS-LBL-30-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-30-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Labels DEMO'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "LABELS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "LABELS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "LBL-01"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-01--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-01--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-02"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-02--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-02--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-03"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-03--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-03--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-04"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-04--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-04--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-05"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-05--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-05--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-06"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-06--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-06--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-07"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-07--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-07--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-08"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-08--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-08--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-09"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-09--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-09--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-10"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-10--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-10--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-11"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-11--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-11--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-12"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-12--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-12--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-13"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-13--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-13--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-14"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-14--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-14--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-15"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-15--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-15--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-16"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-16--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-16--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-17"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-17--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-17--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-18"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-18--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-18--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-19"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-19--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-19--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-20"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-20--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-20--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-21"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-21--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-21--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-22"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-22--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-22--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-23"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-23--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-23--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-24"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-24--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-24--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-25"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-25--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-25--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-26"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-26--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-26--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-27"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-27--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-27--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-28"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-28--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-28--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-29"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-29--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-29--ONMOUSELEAVE"
                       END-EVALUATE
                   WHEN "LBL-30"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onMouseEnter"
                               CALL "LBL-30--ONMOUSEENTER"
                           WHEN "onMouseLeave"
                               CALL "LBL-30--ONMOUSELEAVE"
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
       PROGRAM-ID. LABELS-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM LABELS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LABELS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM LABELS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-01--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-01::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-01--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-01--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-01::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-01--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-02--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-02::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-02--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-02--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-02::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-02--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-03--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-03::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-03--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-03--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-03::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-03--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-04--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-04::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-04--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-04--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-04::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-04--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-05--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-05::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-05--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-05--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-05::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-05--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-06--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-06::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-06--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-06--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-06::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-06--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-07--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-07::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-07--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-07--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-07::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-07--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-08--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-08::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-08--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-08--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-08::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-08--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-09--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-09::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-09--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-09--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-09::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-09--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-10--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-10::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-10--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-10--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-10::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-10--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-11--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-11::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-11--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-11--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-11::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-11--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-12--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-12::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-12--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-12--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-12::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-12--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-13--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-13::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-13--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-13--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-13::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-13--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-14--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-14::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-14--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-14--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-14::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-14--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-15--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-15::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-15--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-15--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-15::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-15--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-16--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-16::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-16--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-16--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-16::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-16--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-17--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-17::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-17--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-17--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-17::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-17--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-18--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-18::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-18--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-18--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-18::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-18--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-19--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-19::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-19--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-19--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-19::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-19--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-20--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-20::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-20--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-20--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-20::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-20--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-21--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-21::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-21--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-21--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-21::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-21--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-22--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-22::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-22--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-22--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-22::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-22--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-23--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-23::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-23--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-23--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-23::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-23--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-24--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-24::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-24--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-24--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-24::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-24--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-25--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-25::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-25--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-25--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-25::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-25--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-26--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-26::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-26--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-26--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-26::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-26--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-27--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-27::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-27--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-27--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-27::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-27--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-28--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-28::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-28--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-28--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-28::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-28--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-29--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-29::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-29--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-29--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-29::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-29--ONMOUSELEAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-30--ONMOUSEENTER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-30::ShadowEnabled TO 0.

           GOBACK.

       END PROGRAM LBL-30--ONMOUSEENTER.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. LBL-30--ONMOUSELEAVE IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
           SET LBL-30::ShadowEnabled TO 1.

           GOBACK.

       END PROGRAM LBL-30--ONMOUSELEAVE.

       END PROGRAM LABELS-FORM.

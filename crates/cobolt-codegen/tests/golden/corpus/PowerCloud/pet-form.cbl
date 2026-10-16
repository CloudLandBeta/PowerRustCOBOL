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
       PROGRAM-ID. PET-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'PET-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-TEXT   PIC X(300) GLOBAL.
       01 WS-PHOTO  PIC X(300) GLOBAL.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Pet'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Name.
          05 WS-Lbl-Name-TEXT       PIC X(256) VALUE 'Name'.
          05 WS-Lbl-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Name-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Name.
          05 WS-Txt-Name-TEXT       PIC X(60) VALUE SPACES.
          05 WS-Txt-Name-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Name-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Name-VALUE      PIC X(60) VALUE SPACES.

       01 WS-Lbl-Race.
          05 WS-Lbl-Race-TEXT       PIC X(256) VALUE 'Race'.
          05 WS-Lbl-Race-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Race-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Race.
          05 WS-Txt-Race-TEXT       PIC X(40) VALUE SPACES.
          05 WS-Txt-Race-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Race-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Race-VALUE      PIC X(40) VALUE SPACES.

       01 WS-Lbl-Age.
          05 WS-Lbl-Age-TEXT       PIC X(256) VALUE 'Age'.
          05 WS-Lbl-Age-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Age-ENABLED    PIC 9      VALUE 1.

       01 WS-Nud-Age.
          05 WS-Nud-Age-TEXT       PIC X(256) VALUE 'Nud-Age'.
          05 WS-Nud-Age-VISIBLE    PIC 9      VALUE 1.
          05 WS-Nud-Age-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Photo.
          05 WS-Lbl-Photo-TEXT       PIC X(256) VALUE 'Photo'.
          05 WS-Lbl-Photo-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Photo-ENABLED    PIC 9      VALUE 1.

       01 WS-Drp-Photo.
          05 WS-Drp-Photo-TEXT       PIC X(256) VALUE 'Drp-Photo'.
          05 WS-Drp-Photo-VISIBLE    PIC 9      VALUE 1.
          05 WS-Drp-Photo-ENABLED    PIC 9      VALUE 1.
          05 WS-Drp-Photo-FILE-COUNT PIC S9(4) VALUE 0.
          05 WS-Drp-Photo-FILE-PATH  PIC X(1024) OCCURS 20 TIMES
                                      VALUE SPACES.

       01 WS-Pic-Preview.
          05 WS-Pic-Preview-TEXT       PIC X(256) VALUE 'Pic-Preview'.
          05 WS-Pic-Preview-VISIBLE    PIC 9      VALUE 1.
          05 WS-Pic-Preview-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Note.
          05 WS-Lbl-Note-TEXT       PIC X(256) VALUE 'The photo is cropped to a square and kept as 400 x 400 in the photo bucket.'.
          05 WS-Lbl-Note-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Note-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "PET-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "PET-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Drp-Photo"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onFilesDropped"
                               CALL "DRP-PHOTO--ONFILESDROPPED"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
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
       PROGRAM-ID. PET-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           *> The pet to edit arrives as properties of the form that opened this one.

           INVOKE    super::GetProperty( "PetName" ) RETURNING WS-TEXT
           SET     TXT-NAME::Text                TO    WS-TEXT::Trim()
           INVOKE    super::GetProperty( "PetRace" ) RETURNING WS-TEXT
           SET     TXT-RACE::Text                TO    WS-TEXT::Trim()
           INVOKE    super::GetProperty( "PetAge" ) RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               SET     NUD-AGE::Value                TO    FUNCTION NUMVAL( WS-TEXT )
           END-IF
           INVOKE    super::GetProperty( "PetThumb" ) RETURNING WS-TEXT
           IF WS-TEXT NOT = SPACES
               SET     PIC-PREVIEW::ImagePath        TO    WS-TEXT::Trim()
           END-IF
           SET     WS-PHOTO                      TO    SPACES

           GOBACK.

       END PROGRAM PET-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PET-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM PET-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DRP-PHOTO--ONFILESDROPPED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           SET     WS-PHOTO                      TO    DRP-PHOTO::DroppedFiles
           SET     PIC-PREVIEW::ImagePath        TO    WS-PHOTO::Trim()

           GOBACK.

       END PROGRAM DRP-PHOTO--ONFILESDROPPED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           IF TXT-NAME::Text::Trim()::Len() = 0
               SET     LBL-NOTE::Caption             TO    "A pet needs a name."
               GOBACK
           END-IF
           INVOKE    super::SetProperty( "PetName", TXT-NAME::Text::Trim() )
           INVOKE    super::SetProperty( "PetRace", TXT-RACE::Text::Trim() )
           INVOKE    super::SetProperty( "PetAge", NUD-AGE::Value )
           INVOKE    super::SetProperty( "PetPhoto", WS-PHOTO::Trim() )
           INVOKE    super::SetProperty( "PetSaved", "Y" )
           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       PROCEDURE DIVISION.

           INVOKE    super::SetProperty( "PetSaved", "N" )
           INVOKE    me::Close()

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       END PROGRAM PET-FORM.

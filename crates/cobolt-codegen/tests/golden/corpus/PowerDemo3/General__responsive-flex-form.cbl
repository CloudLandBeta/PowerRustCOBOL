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
       PROGRAM-ID. RESPONSIVE-FLEX-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-FLEX-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Flex: one line (or several) of items that share the space'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'Every property of a Flex container and of its items, one tab each. Make the window narrow and wide: every box is laid out again on each frame.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-TAB-FLEX.
          05 WS-TAB-FLEX-TEXT       PIC X(256) VALUE 'TAB-FLEX'.
          05 WS-TAB-FLEX-VISIBLE    PIC 9      VALUE 1.
          05 WS-TAB-FLEX-ENABLED    PIC 9      VALUE 1.

       01 WS-P0-HOST.
          05 WS-P0-HOST-TEXT       PIC X(256) VALUE 'P0-HOST'.
          05 WS-P0-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P0-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-1.
          05 WS-FD-1-TEXT       PIC X(256) VALUE 'FlexDirection = Row'.
          05 WS-FD-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-1-C-1.
          05 WS-FD-1-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-FD-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-1-C-2.
          05 WS-FD-1-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-FD-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-1-C-3.
          05 WS-FD-1-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-FD-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-1-C-4.
          05 WS-FD-1-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-FD-1-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-1-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-2.
          05 WS-FD-2-TEXT       PIC X(256) VALUE 'FlexDirection = RowReverse'.
          05 WS-FD-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-2-C-1.
          05 WS-FD-2-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-FD-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-2-C-2.
          05 WS-FD-2-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-FD-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-2-C-3.
          05 WS-FD-2-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-FD-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-2-C-4.
          05 WS-FD-2-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-FD-2-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-2-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-3.
          05 WS-FD-3-TEXT       PIC X(256) VALUE 'FlexDirection = Column'.
          05 WS-FD-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-3-C-1.
          05 WS-FD-3-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-FD-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-3-C-2.
          05 WS-FD-3-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-FD-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-3-C-3.
          05 WS-FD-3-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-FD-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-3-C-4.
          05 WS-FD-3-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-FD-3-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-3-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-4.
          05 WS-FD-4-TEXT       PIC X(256) VALUE 'FlexDirection = ColumnReverse'.
          05 WS-FD-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-4-C-1.
          05 WS-FD-4-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-FD-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-4-C-2.
          05 WS-FD-4-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-FD-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-4-C-3.
          05 WS-FD-4-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-FD-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FD-4-C-4.
          05 WS-FD-4-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-FD-4-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FD-4-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-P1-HOST.
          05 WS-P1-HOST-TEXT       PIC X(256) VALUE 'P1-HOST'.
          05 WS-P1-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P1-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-1.
          05 WS-JC-1-TEXT       PIC X(256) VALUE 'JustifyContent = Start'.
          05 WS-JC-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-1-C-1.
          05 WS-JC-1-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-1-C-2.
          05 WS-JC-1-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-1-C-3.
          05 WS-JC-1-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-2.
          05 WS-JC-2-TEXT       PIC X(256) VALUE 'JustifyContent = Center'.
          05 WS-JC-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-2-C-1.
          05 WS-JC-2-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-2-C-2.
          05 WS-JC-2-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-2-C-3.
          05 WS-JC-2-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-3.
          05 WS-JC-3-TEXT       PIC X(256) VALUE 'JustifyContent = End'.
          05 WS-JC-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-3-C-1.
          05 WS-JC-3-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-3-C-2.
          05 WS-JC-3-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-3-C-3.
          05 WS-JC-3-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-4.
          05 WS-JC-4-TEXT       PIC X(256) VALUE 'JustifyContent = SpaceBetween'.
          05 WS-JC-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-4-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-4-C-1.
          05 WS-JC-4-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-4-C-2.
          05 WS-JC-4-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-4-C-3.
          05 WS-JC-4-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-5.
          05 WS-JC-5-TEXT       PIC X(256) VALUE 'JustifyContent = SpaceAround'.
          05 WS-JC-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-5-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-5-C-1.
          05 WS-JC-5-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-5-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-5-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-5-C-2.
          05 WS-JC-5-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-5-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-5-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-5-C-3.
          05 WS-JC-5-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-5-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-5-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-6.
          05 WS-JC-6-TEXT       PIC X(256) VALUE 'JustifyContent = SpaceEvenly'.
          05 WS-JC-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-6-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-6-C-1.
          05 WS-JC-6-C-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-JC-6-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-6-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-6-C-2.
          05 WS-JC-6-C-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-JC-6-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-6-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-JC-6-C-3.
          05 WS-JC-6-C-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-JC-6-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-JC-6-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-P2-HOST.
          05 WS-P2-HOST-TEXT       PIC X(256) VALUE 'P2-HOST'.
          05 WS-P2-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P2-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-1.
          05 WS-AI-1-TEXT       PIC X(256) VALUE 'AlignItems = Stretch'.
          05 WS-AI-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-1-C-1.
          05 WS-AI-1-C-1-TEXT       PIC X(256) VALUE 'h 30'.
          05 WS-AI-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-1-C-2.
          05 WS-AI-1-C-2-TEXT       PIC X(256) VALUE 'h 50'.
          05 WS-AI-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-1-C-3.
          05 WS-AI-1-C-3-TEXT       PIC X(256) VALUE 'h 70'.
          05 WS-AI-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-2.
          05 WS-AI-2-TEXT       PIC X(256) VALUE 'AlignItems = Start'.
          05 WS-AI-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-2-C-1.
          05 WS-AI-2-C-1-TEXT       PIC X(256) VALUE 'h 30'.
          05 WS-AI-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-2-C-2.
          05 WS-AI-2-C-2-TEXT       PIC X(256) VALUE 'h 50'.
          05 WS-AI-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-2-C-3.
          05 WS-AI-2-C-3-TEXT       PIC X(256) VALUE 'h 70'.
          05 WS-AI-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-3.
          05 WS-AI-3-TEXT       PIC X(256) VALUE 'AlignItems = Center'.
          05 WS-AI-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-3-C-1.
          05 WS-AI-3-C-1-TEXT       PIC X(256) VALUE 'h 30'.
          05 WS-AI-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-3-C-2.
          05 WS-AI-3-C-2-TEXT       PIC X(256) VALUE 'h 50'.
          05 WS-AI-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-3-C-3.
          05 WS-AI-3-C-3-TEXT       PIC X(256) VALUE 'h 70'.
          05 WS-AI-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-4.
          05 WS-AI-4-TEXT       PIC X(256) VALUE 'AlignItems = End'.
          05 WS-AI-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-4-C-1.
          05 WS-AI-4-C-1-TEXT       PIC X(256) VALUE 'h 30'.
          05 WS-AI-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-4-C-2.
          05 WS-AI-4-C-2-TEXT       PIC X(256) VALUE 'h 50'.
          05 WS-AI-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-4-C-3.
          05 WS-AI-4-C-3-TEXT       PIC X(256) VALUE 'h 70'.
          05 WS-AI-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-5.
          05 WS-AI-5-TEXT       PIC X(256) VALUE 'AlignSelf per item (container: Start)'.
          05 WS-AI-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-5-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-5-C-1.
          05 WS-AI-5-C-1-TEXT       PIC X(256) VALUE 'End'.
          05 WS-AI-5-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-5-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-5-C-2.
          05 WS-AI-5-C-2-TEXT       PIC X(256) VALUE 'Center'.
          05 WS-AI-5-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-5-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-5-C-3.
          05 WS-AI-5-C-3-TEXT       PIC X(256) VALUE 'Stretch'.
          05 WS-AI-5-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-5-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-5-C-4.
          05 WS-AI-5-C-4-TEXT       PIC X(256) VALUE 'Auto'.
          05 WS-AI-5-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-5-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-6.
          05 WS-AI-6-TEXT       PIC X(256) VALUE 'Column + AlignItems = Center'.
          05 WS-AI-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-6-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-6-C-1.
          05 WS-AI-6-C-1-TEXT       PIC X(256) VALUE 'w 60'.
          05 WS-AI-6-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-6-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-6-C-2.
          05 WS-AI-6-C-2-TEXT       PIC X(256) VALUE 'w 120'.
          05 WS-AI-6-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-6-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AI-6-C-3.
          05 WS-AI-6-C-3-TEXT       PIC X(256) VALUE 'w 180'.
          05 WS-AI-6-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AI-6-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-P3-HOST.
          05 WS-P3-HOST-TEXT       PIC X(256) VALUE 'P3-HOST'.
          05 WS-P3-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P3-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1.
          05 WS-WR-1-TEXT       PIC X(256) VALUE 'NoWrap - the items shrink'.
          05 WS-WR-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-1.
          05 WS-WR-1-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-2.
          05 WS-WR-1-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-3.
          05 WS-WR-1-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-4.
          05 WS-WR-1-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-1-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-5.
          05 WS-WR-1-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-1-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-1-C-6.
          05 WS-WR-1-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-1-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-1-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2.
          05 WS-WR-2-TEXT       PIC X(256) VALUE 'Wrap + AlignContent Start'.
          05 WS-WR-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-1.
          05 WS-WR-2-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-2.
          05 WS-WR-2-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-3.
          05 WS-WR-2-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-4.
          05 WS-WR-2-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-2-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-5.
          05 WS-WR-2-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-2-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-6.
          05 WS-WR-2-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-2-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-7.
          05 WS-WR-2-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-2-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-2-C-8.
          05 WS-WR-2-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-2-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-2-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3.
          05 WS-WR-3-TEXT       PIC X(256) VALUE 'WrapReverse'.
          05 WS-WR-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-1.
          05 WS-WR-3-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-2.
          05 WS-WR-3-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-3.
          05 WS-WR-3-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-4.
          05 WS-WR-3-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-3-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-5.
          05 WS-WR-3-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-3-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-6.
          05 WS-WR-3-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-3-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-7.
          05 WS-WR-3-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-3-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-3-C-8.
          05 WS-WR-3-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-3-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-3-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4.
          05 WS-WR-4-TEXT       PIC X(256) VALUE 'Wrap + AlignContent Center'.
          05 WS-WR-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-1.
          05 WS-WR-4-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-2.
          05 WS-WR-4-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-3.
          05 WS-WR-4-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-4.
          05 WS-WR-4-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-4-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-5.
          05 WS-WR-4-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-4-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-6.
          05 WS-WR-4-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-4-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-7.
          05 WS-WR-4-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-4-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-4-C-8.
          05 WS-WR-4-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-4-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-4-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5.
          05 WS-WR-5-TEXT       PIC X(256) VALUE 'Wrap + AlignContent End'.
          05 WS-WR-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-1.
          05 WS-WR-5-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-5-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-2.
          05 WS-WR-5-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-5-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-3.
          05 WS-WR-5-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-5-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-4.
          05 WS-WR-5-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-5-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-5.
          05 WS-WR-5-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-5-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-6.
          05 WS-WR-5-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-5-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-7.
          05 WS-WR-5-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-5-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-5-C-8.
          05 WS-WR-5-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-5-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-5-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6.
          05 WS-WR-6-TEXT       PIC X(256) VALUE 'Wrap + AlignContent SpaceBetween'.
          05 WS-WR-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-1.
          05 WS-WR-6-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-6-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-2.
          05 WS-WR-6-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-6-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-3.
          05 WS-WR-6-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-6-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-4.
          05 WS-WR-6-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-6-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-5.
          05 WS-WR-6-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-6-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-6.
          05 WS-WR-6-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-6-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-7.
          05 WS-WR-6-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-6-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-6-C-8.
          05 WS-WR-6-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-6-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-6-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7.
          05 WS-WR-7-TEXT       PIC X(256) VALUE 'Wrap + AlignContent SpaceAround'.
          05 WS-WR-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-1.
          05 WS-WR-7-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-7-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-2.
          05 WS-WR-7-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-7-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-3.
          05 WS-WR-7-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-7-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-4.
          05 WS-WR-7-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-7-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-5.
          05 WS-WR-7-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-7-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-6.
          05 WS-WR-7-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-7-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-7.
          05 WS-WR-7-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-7-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-7-C-8.
          05 WS-WR-7-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-7-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-7-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8.
          05 WS-WR-8-TEXT       PIC X(256) VALUE 'Wrap, Stretch, RowGap 2 / ColumnGap 18'.
          05 WS-WR-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-1.
          05 WS-WR-8-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-WR-8-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-2.
          05 WS-WR-8-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-WR-8-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-3.
          05 WS-WR-8-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-WR-8-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-4.
          05 WS-WR-8-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-WR-8-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-5.
          05 WS-WR-8-C-5-TEXT       PIC X(256) VALUE '5'.
          05 WS-WR-8-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-6.
          05 WS-WR-8-C-6-TEXT       PIC X(256) VALUE '6'.
          05 WS-WR-8-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-7.
          05 WS-WR-8-C-7-TEXT       PIC X(256) VALUE '7'.
          05 WS-WR-8-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-WR-8-C-8.
          05 WS-WR-8-C-8-TEXT       PIC X(256) VALUE '8'.
          05 WS-WR-8-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-WR-8-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-P4-HOST.
          05 WS-P4-HOST-TEXT       PIC X(256) VALUE 'P4-HOST'.
          05 WS-P4-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P4-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-1.
          05 WS-GS-1-TEXT       PIC X(256) VALUE 'FlexGrow 0 / 1 / 2 / 3 - basis 80 each, the free space shared 0:1:2:3'.
          05 WS-GS-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-1-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-1-C-1.
          05 WS-GS-1-C-1-TEXT       PIC X(256) VALUE 'grow 0'.
          05 WS-GS-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-1-C-2.
          05 WS-GS-1-C-2-TEXT       PIC X(256) VALUE 'grow 1'.
          05 WS-GS-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-1-C-3.
          05 WS-GS-1-C-3-TEXT       PIC X(256) VALUE 'grow 2'.
          05 WS-GS-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-1-C-4.
          05 WS-GS-1-C-4-TEXT       PIC X(256) VALUE 'grow 3'.
          05 WS-GS-1-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-1-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-2.
          05 WS-GS-2-TEXT       PIC X(256) VALUE 'FlexBasis 120 px / 25% / Auto (designed 200) - no growing'.
          05 WS-GS-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-2-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-2-C-1.
          05 WS-GS-2-C-1-TEXT       PIC X(256) VALUE 'basis 120'.
          05 WS-GS-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-2-C-2.
          05 WS-GS-2-C-2-TEXT       PIC X(256) VALUE 'basis 25%'.
          05 WS-GS-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-2-C-3.
          05 WS-GS-2-C-3-TEXT       PIC X(256) VALUE 'basis Auto'.
          05 WS-GS-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-3.
          05 WS-GS-3-TEXT       PIC X(256) VALUE 'FlexShrink 0 / 1 / 3 - narrow the window: the 0 keeps 300 px'.
          05 WS-GS-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-3-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-3-C-1.
          05 WS-GS-3-C-1-TEXT       PIC X(256) VALUE 'shrink 0'.
          05 WS-GS-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-3-C-2.
          05 WS-GS-3-C-2-TEXT       PIC X(256) VALUE 'shrink 1'.
          05 WS-GS-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-3-C-3.
          05 WS-GS-3-C-3-TEXT       PIC X(256) VALUE 'shrink 3'.
          05 WS-GS-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-4.
          05 WS-GS-4-TEXT       PIC X(256) VALUE 'All grow 1 - the middle has MaxWidth 160, the last MinWidth 260'.
          05 WS-GS-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-4-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-4-C-1.
          05 WS-GS-4-C-1-TEXT       PIC X(256) VALUE 'grow 1'.
          05 WS-GS-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-4-C-2.
          05 WS-GS-4-C-2-TEXT       PIC X(256) VALUE 'MaxWidth 160'.
          05 WS-GS-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-GS-4-C-3.
          05 WS-GS-4-C-3-TEXT       PIC X(256) VALUE 'MinWidth 260'.
          05 WS-GS-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-GS-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-P5-HOST.
          05 WS-P5-HOST-TEXT       PIC X(256) VALUE 'P5-HOST'.
          05 WS-P5-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-P5-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-LIVE.
          05 WS-OR-LIVE-TEXT       PIC X(256) VALUE 'Designed A B C D E - Order 3 1 0 -1 2 shows D C B E A'.
          05 WS-OR-LIVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-LIVE-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-1.
          05 WS-OR-1-TEXT       PIC X(256) VALUE 'A'.
          05 WS-OR-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-1-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-2.
          05 WS-OR-2-TEXT       PIC X(256) VALUE 'B'.
          05 WS-OR-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-2-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-3.
          05 WS-OR-3-TEXT       PIC X(256) VALUE 'C'.
          05 WS-OR-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-3-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-4.
          05 WS-OR-4-TEXT       PIC X(256) VALUE 'D'.
          05 WS-OR-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-4-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-5.
          05 WS-OR-5-TEXT       PIC X(256) VALUE 'E'.
          05 WS-OR-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-5-ENABLED    PIC 9      VALUE 1.

       01 WS-OR-CMDS.
          05 WS-OR-CMDS-TEXT       PIC X(256) VALUE 'From COBOL - each button writes one property (a 6 x 2 Grid of MinMax(96px, 1fr) columns)'.
          05 WS-OR-CMDS-VISIBLE    PIC 9      VALUE 1.
          05 WS-OR-CMDS-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-REV.
          05 WS-OC-REV-TEXT       PIC X(256) VALUE 'Order E..A'.
          05 WS-OC-REV-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-REV-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-ZERO.
          05 WS-OC-ZERO-TEXT       PIC X(256) VALUE 'Order 0'.
          05 WS-OC-ZERO-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-ZERO-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-COL.
          05 WS-OC-COL-TEXT       PIC X(256) VALUE 'Column'.
          05 WS-OC-COL-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-COL-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-ROW.
          05 WS-OC-ROW-TEXT       PIC X(256) VALUE 'Row'.
          05 WS-OC-ROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-ROW-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-RREV.
          05 WS-OC-RREV-TEXT       PIC X(256) VALUE 'RowReverse'.
          05 WS-OC-RREV-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-RREV-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-GAP.
          05 WS-OC-GAP-TEXT       PIC X(256) VALUE 'Gap 32'.
          05 WS-OC-GAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-GAP-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-JS.
          05 WS-OC-JS-TEXT       PIC X(256) VALUE 'Start'.
          05 WS-OC-JS-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-JS-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-JC.
          05 WS-OC-JC-TEXT       PIC X(256) VALUE 'Center'.
          05 WS-OC-JC-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-JC-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-JE.
          05 WS-OC-JE-TEXT       PIC X(256) VALUE 'SpaceEvenly'.
          05 WS-OC-JE-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-JE-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-GROW.
          05 WS-OC-GROW-TEXT       PIC X(256) VALUE 'C grows'.
          05 WS-OC-GROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-GROW-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-NOGROW.
          05 WS-OC-NOGROW-TEXT       PIC X(256) VALUE 'C fixed'.
          05 WS-OC-NOGROW-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-NOGROW-ENABLED    PIC 9      VALUE 1.

       01 WS-OC-GAP8.
          05 WS-OC-GAP8-TEXT       PIC X(256) VALUE 'Gap 8'.
          05 WS-OC-GAP8-VISIBLE    PIC 9      VALUE 1.
          05 WS-OC-GAP8-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-FLEX-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-FLEX-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-FLEX-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-FLEX-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-FLEX-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "OC-REV"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-REV--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-ZERO"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-ZERO--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-COL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-COL--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-ROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-ROW--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-RREV"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-RREV--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-GAP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-GAP--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-JS"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-JS--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-JC"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-JC--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-JE"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-JE--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-GROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-GROW--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-NOGROW"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-NOGROW--ONCLICK"
                       END-EVALUATE
                   WHEN "OC-GAP8"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "OC-GAP8--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-FLEX-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLX-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLEX-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLEX-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-FLEX-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLEX-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLX-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLEX-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLEX-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLX-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLEX-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-REV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 5 TO OR-1::Order
           MOVE 4 TO OR-2::Order
           MOVE 3 TO OR-3::Order
           MOVE 2 TO OR-4::Order
           MOVE 1 TO OR-5::Order

           GOBACK.

       END PROGRAM OC-REV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-ZERO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO OR-1::Order
           MOVE 0 TO OR-2::Order
           MOVE 0 TO OR-3::Order
           MOVE 0 TO OR-4::Order
           MOVE 0 TO OR-5::Order

           GOBACK.

       END PROGRAM OC-ZERO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-COL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Column" TO OR-LIVE::FlexDirection

           GOBACK.

       END PROGRAM OC-COL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-ROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Row" TO OR-LIVE::FlexDirection

           GOBACK.

       END PROGRAM OC-ROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-RREV--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "RowReverse" TO OR-LIVE::FlexDirection

           GOBACK.

       END PROGRAM OC-RREV--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-GAP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 32 TO OR-LIVE::Gap

           GOBACK.

       END PROGRAM OC-GAP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-JS--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Start" TO OR-LIVE::JustifyContent

           GOBACK.

       END PROGRAM OC-JS--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-JC--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "Center" TO OR-LIVE::JustifyContent

           GOBACK.

       END PROGRAM OC-JC--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-JE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "SpaceEvenly" TO OR-LIVE::JustifyContent

           GOBACK.

       END PROGRAM OC-JE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-GROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO OR-3::FlexGrow

           GOBACK.

       END PROGRAM OC-GROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-NOGROW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO OR-3::FlexGrow

           GOBACK.

       END PROGRAM OC-NOGROW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OC-GAP8--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 8 TO OR-LIVE::Gap

           GOBACK.

       END PROGRAM OC-GAP8--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLX-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM FLX-SHOW-INFO.

       END PROGRAM RESPONSIVE-FLEX-FORM.

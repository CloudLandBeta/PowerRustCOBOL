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
       PROGRAM-ID. RESPONSIVE-GRID-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-GRID-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Grid: rows and columns that share the space'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'Tracks in px, %, fr and Auto, MinMax and Repeat, items placed by hand or automatically. The last tab changes its columns below 1024 px.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-TAB-GRID.
          05 WS-TAB-GRID-TEXT       PIC X(256) VALUE 'TAB-GRID'.
          05 WS-TAB-GRID-VISIBLE    PIC 9      VALUE 1.
          05 WS-TAB-GRID-ENABLED    PIC 9      VALUE 1.

       01 WS-G0-HOST.
          05 WS-G0-HOST-TEXT       PIC X(256) VALUE 'G0-HOST'.
          05 WS-G0-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-G0-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1.
          05 WS-TR-1-TEXT       PIC X(256) VALUE 'GridColumns = 120px 25% 1fr 2fr Auto'.
          05 WS-TR-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1-C-1.
          05 WS-TR-1-C-1-TEXT       PIC X(256) VALUE '120px'.
          05 WS-TR-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1-C-2.
          05 WS-TR-1-C-2-TEXT       PIC X(256) VALUE '25%'.
          05 WS-TR-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1-C-3.
          05 WS-TR-1-C-3-TEXT       PIC X(256) VALUE '1fr'.
          05 WS-TR-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1-C-4.
          05 WS-TR-1-C-4-TEXT       PIC X(256) VALUE '2fr'.
          05 WS-TR-1-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-1-C-5.
          05 WS-TR-1-C-5-TEXT       PIC X(256) VALUE 'Auto: w 140'.
          05 WS-TR-1-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-1-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-2.
          05 WS-TR-2-TEXT       PIC X(256) VALUE 'GridColumns = MinMax(150px, 1fr) MinMax(100px, 300px) 1fr'.
          05 WS-TR-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-2-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-2-C-1.
          05 WS-TR-2-C-1-TEXT       PIC X(256) VALUE 'MinMax(150px, 1fr)'.
          05 WS-TR-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-2-C-2.
          05 WS-TR-2-C-2-TEXT       PIC X(256) VALUE 'MinMax(100px, 300px)'.
          05 WS-TR-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-2-C-3.
          05 WS-TR-2-C-3-TEXT       PIC X(256) VALUE '1fr'.
          05 WS-TR-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3.
          05 WS-TR-3-TEXT       PIC X(256) VALUE 'GridColumns = Repeat(3, 1fr 60px)'.
          05 WS-TR-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-1.
          05 WS-TR-3-C-1-TEXT       PIC X(256) VALUE '1fr'.
          05 WS-TR-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-2.
          05 WS-TR-3-C-2-TEXT       PIC X(256) VALUE '60'.
          05 WS-TR-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-3.
          05 WS-TR-3-C-3-TEXT       PIC X(256) VALUE '1fr'.
          05 WS-TR-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-4.
          05 WS-TR-3-C-4-TEXT       PIC X(256) VALUE '60'.
          05 WS-TR-3-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-5.
          05 WS-TR-3-C-5-TEXT       PIC X(256) VALUE '1fr'.
          05 WS-TR-3-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-3-C-6.
          05 WS-TR-3-C-6-TEXT       PIC X(256) VALUE '60'.
          05 WS-TR-3-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-3-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4.
          05 WS-TR-4-TEXT       PIC X(256) VALUE 'GridColumns = 1fr 1fr 1fr, GridRows = 30px 1fr Auto'.
          05 WS-TR-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-1.
          05 WS-TR-4-C-1-TEXT       PIC X(256) VALUE 'row 30px'.
          05 WS-TR-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-2.
          05 WS-TR-4-C-2-TEXT       PIC X(256) VALUE 'row 30px'.
          05 WS-TR-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-3.
          05 WS-TR-4-C-3-TEXT       PIC X(256) VALUE 'row 30px'.
          05 WS-TR-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-4.
          05 WS-TR-4-C-4-TEXT       PIC X(256) VALUE 'row 1fr'.
          05 WS-TR-4-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-5.
          05 WS-TR-4-C-5-TEXT       PIC X(256) VALUE 'row 1fr'.
          05 WS-TR-4-C-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-5-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-6.
          05 WS-TR-4-C-6-TEXT       PIC X(256) VALUE 'row 1fr'.
          05 WS-TR-4-C-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-6-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-7.
          05 WS-TR-4-C-7-TEXT       PIC X(256) VALUE 'row Auto'.
          05 WS-TR-4-C-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-7-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-8.
          05 WS-TR-4-C-8-TEXT       PIC X(256) VALUE 'row Auto'.
          05 WS-TR-4-C-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-8-ENABLED    PIC 9      VALUE 1.

       01 WS-TR-4-C-9.
          05 WS-TR-4-C-9-TEXT       PIC X(256) VALUE 'row Auto'.
          05 WS-TR-4-C-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-TR-4-C-9-ENABLED    PIC 9      VALUE 1.

       01 WS-G1-HOST.
          05 WS-G1-HOST-TEXT       PIC X(256) VALUE 'G1-HOST'.
          05 WS-G1-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-G1-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1.
          05 WS-CARD-1-TEXT       PIC X(256) VALUE 'CARD-1'.
          05 WS-CARD-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1-T.
          05 WS-CARD-1-T-TEXT       PIC X(256) VALUE 'Espresso'.
          05 WS-CARD-1-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-1-N.
          05 WS-CARD-1-N-TEXT       PIC X(256) VALUE 'Dark roast, 250 g'.
          05 WS-CARD-1-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-1-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2.
          05 WS-CARD-2-TEXT       PIC X(256) VALUE 'CARD-2'.
          05 WS-CARD-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2-T.
          05 WS-CARD-2-T-TEXT       PIC X(256) VALUE 'Filter'.
          05 WS-CARD-2-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-2-N.
          05 WS-CARD-2-N-TEXT       PIC X(256) VALUE 'Light roast, 500 g'.
          05 WS-CARD-2-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-2-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3.
          05 WS-CARD-3-TEXT       PIC X(256) VALUE 'CARD-3'.
          05 WS-CARD-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3-T.
          05 WS-CARD-3-T-TEXT       PIC X(256) VALUE 'Grinder'.
          05 WS-CARD-3-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-3-N.
          05 WS-CARD-3-N-TEXT       PIC X(256) VALUE 'Burr, 40 settings'.
          05 WS-CARD-3-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-3-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4.
          05 WS-CARD-4-TEXT       PIC X(256) VALUE 'CARD-4'.
          05 WS-CARD-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4-T.
          05 WS-CARD-4-T-TEXT       PIC X(256) VALUE 'Kettle'.
          05 WS-CARD-4-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-4-N.
          05 WS-CARD-4-N-TEXT       PIC X(256) VALUE 'Gooseneck, 0.9 l'.
          05 WS-CARD-4-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-4-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5.
          05 WS-CARD-5-TEXT       PIC X(256) VALUE 'CARD-5'.
          05 WS-CARD-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5-T.
          05 WS-CARD-5-T-TEXT       PIC X(256) VALUE 'Scale'.
          05 WS-CARD-5-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-5-N.
          05 WS-CARD-5-N-TEXT       PIC X(256) VALUE '0.1 g, with timer'.
          05 WS-CARD-5-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-5-N-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6.
          05 WS-CARD-6-TEXT       PIC X(256) VALUE 'CARD-6'.
          05 WS-CARD-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6-T.
          05 WS-CARD-6-T-TEXT       PIC X(256) VALUE 'Gift card'.
          05 WS-CARD-6-T-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-T-ENABLED    PIC 9      VALUE 1.

       01 WS-CARD-6-N.
          05 WS-CARD-6-N-TEXT       PIC X(256) VALUE 'Any amount'.
          05 WS-CARD-6-N-VISIBLE    PIC 9      VALUE 1.
          05 WS-CARD-6-N-ENABLED    PIC 9      VALUE 1.

       01 WS-G2-HOST.
          05 WS-G2-HOST-TEXT       PIC X(256) VALUE 'G2-HOST'.
          05 WS-G2-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-G2-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-HEAD.
          05 WS-SP-HEAD-TEXT       PIC X(256) VALUE 'GridRow 1, GridColumn 1, ColumnSpan 4'.
          05 WS-SP-HEAD-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-HEAD-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-SIDE.
          05 WS-SP-SIDE-TEXT       PIC X(256) VALUE 'Row 2, Col 1, RowSpan 2'.
          05 WS-SP-SIDE-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-SIDE-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-MAIN.
          05 WS-SP-MAIN-TEXT       PIC X(256) VALUE 'Row 2, Col 2, ColumnSpan 2, RowSpan 2'.
          05 WS-SP-MAIN-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-MAIN-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-A.
          05 WS-SP-A-TEXT       PIC X(256) VALUE 'auto-placed (row 2, col 4)'.
          05 WS-SP-A-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-A-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-B.
          05 WS-SP-B-TEXT       PIC X(256) VALUE 'auto-placed (row 3, col 4)'.
          05 WS-SP-B-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-B-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-EXTRA.
          05 WS-SP-EXTRA-TEXT       PIC X(256) VALUE 'GridRow 4 - past GridRows: an implicit Auto row'.
          05 WS-SP-EXTRA-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-EXTRA-ENABLED    PIC 9      VALUE 1.

       01 WS-SP-C.
          05 WS-SP-C-TEXT       PIC X(256) VALUE 'auto-placed, ColumnSpan 2 - row 5'.
          05 WS-SP-C-VISIBLE    PIC 9      VALUE 1.
          05 WS-SP-C-ENABLED    PIC 9      VALUE 1.

       01 WS-G3-HOST.
          05 WS-G3-HOST-TEXT       PIC X(256) VALUE 'G3-HOST'.
          05 WS-G3-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-G3-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-1.
          05 WS-AL-1-TEXT       PIC X(256) VALUE 'Stretch / Stretch (default)'.
          05 WS-AL-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-1-C-1.
          05 WS-AL-1-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-AL-1-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-1-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-1-C-2.
          05 WS-AL-1-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-AL-1-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-1-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-1-C-3.
          05 WS-AL-1-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-AL-1-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-1-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-1-C-4.
          05 WS-AL-1-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-AL-1-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-1-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-2.
          05 WS-AL-2-TEXT       PIC X(256) VALUE 'JustifyItems Start, AlignItems Start'.
          05 WS-AL-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-2-C-1.
          05 WS-AL-2-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-AL-2-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-2-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-2-C-2.
          05 WS-AL-2-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-AL-2-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-2-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-2-C-3.
          05 WS-AL-2-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-AL-2-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-2-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-2-C-4.
          05 WS-AL-2-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-AL-2-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-2-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-3.
          05 WS-AL-3-TEXT       PIC X(256) VALUE 'JustifyItems Center, AlignItems Center'.
          05 WS-AL-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-3-C-1.
          05 WS-AL-3-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-AL-3-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-3-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-3-C-2.
          05 WS-AL-3-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-AL-3-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-3-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-3-C-3.
          05 WS-AL-3-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-AL-3-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-3-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-3-C-4.
          05 WS-AL-3-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-AL-3-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-3-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-4.
          05 WS-AL-4-TEXT       PIC X(256) VALUE 'JustifyItems End, AlignItems End'.
          05 WS-AL-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-4-C-1.
          05 WS-AL-4-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-AL-4-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-4-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-4-C-2.
          05 WS-AL-4-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-AL-4-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-4-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-4-C-3.
          05 WS-AL-4-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-AL-4-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-4-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-4-C-4.
          05 WS-AL-4-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-AL-4-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-4-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-5.
          05 WS-AL-5-TEXT       PIC X(256) VALUE 'Container Center; each item''s own JustifySelf + AlignSelf'.
          05 WS-AL-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-5-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-5-C-1.
          05 WS-AL-5-C-1-TEXT       PIC X(256) VALUE 'Start'.
          05 WS-AL-5-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-5-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-5-C-2.
          05 WS-AL-5-C-2-TEXT       PIC X(256) VALUE 'End'.
          05 WS-AL-5-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-5-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-5-C-3.
          05 WS-AL-5-C-3-TEXT       PIC X(256) VALUE 'Stretch'.
          05 WS-AL-5-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-5-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-5-C-4.
          05 WS-AL-5-C-4-TEXT       PIC X(256) VALUE 'Auto'.
          05 WS-AL-5-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-5-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-6.
          05 WS-AL-6-TEXT       PIC X(256) VALUE 'JustifyItems End, AlignItems Start'.
          05 WS-AL-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-6-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-6-C-1.
          05 WS-AL-6-C-1-TEXT       PIC X(256) VALUE '1'.
          05 WS-AL-6-C-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-6-C-1-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-6-C-2.
          05 WS-AL-6-C-2-TEXT       PIC X(256) VALUE '2'.
          05 WS-AL-6-C-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-6-C-2-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-6-C-3.
          05 WS-AL-6-C-3-TEXT       PIC X(256) VALUE '3'.
          05 WS-AL-6-C-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-6-C-3-ENABLED    PIC 9      VALUE 1.

       01 WS-AL-6-C-4.
          05 WS-AL-6-C-4-TEXT       PIC X(256) VALUE '4'.
          05 WS-AL-6-C-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-AL-6-C-4-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-HOST.
          05 WS-DE-HOST-TEXT       PIC X(256) VALUE 'DE-HOST'.
          05 WS-DE-HOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-HOST-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-L-CODE.
          05 WS-DE-L-CODE-TEXT       PIC X(256) VALUE 'Customer code'.
          05 WS-DE-L-CODE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-CODE-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-CODE.
          05 WS-DE-CODE-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DE-CODE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-CODE-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-CODE-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DE-L-NAME.
          05 WS-DE-L-NAME-TEXT       PIC X(256) VALUE 'Name'.
          05 WS-DE-L-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-NAME-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-NAME.
          05 WS-DE-NAME-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DE-NAME-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-NAME-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-NAME-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DE-L-EMAIL.
          05 WS-DE-L-EMAIL-TEXT       PIC X(256) VALUE 'E-mail'.
          05 WS-DE-L-EMAIL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-EMAIL-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-EMAIL.
          05 WS-DE-EMAIL-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DE-EMAIL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-EMAIL-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-EMAIL-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DE-L-PHONE.
          05 WS-DE-L-PHONE-TEXT       PIC X(256) VALUE 'Phone'.
          05 WS-DE-L-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-PHONE-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-PHONE.
          05 WS-DE-PHONE-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DE-PHONE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-PHONE-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-PHONE-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DE-L-CITY.
          05 WS-DE-L-CITY-TEXT       PIC X(256) VALUE 'City'.
          05 WS-DE-L-CITY-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-CITY-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-CITY.
          05 WS-DE-CITY-TEXT       PIC X(256) VALUE SPACES.
          05 WS-DE-CITY-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-CITY-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-CITY-VALUE      PIC X(256) VALUE SPACES.

       01 WS-DE-L-COUNTRY.
          05 WS-DE-L-COUNTRY-TEXT       PIC X(256) VALUE 'Country'.
          05 WS-DE-L-COUNTRY-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-COUNTRY-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-COUNTRY.
          05 WS-DE-COUNTRY-TEXT       PIC X(256) VALUE 'DE-COUNTRY'.
          05 WS-DE-COUNTRY-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-COUNTRY-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-COUNTRY-VALUE      PIC X(512) VALUE SPACES.

       01 WS-DE-L-NOTES.
          05 WS-DE-L-NOTES-TEXT       PIC X(256) VALUE 'Notes'.
          05 WS-DE-L-NOTES-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-L-NOTES-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-NOTES.
          05 WS-DE-NOTES-TEXT       PIC X(2048) VALUE SPACES.
          05 WS-DE-NOTES-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-NOTES-ENABLED    PIC 9      VALUE 1.
          05 WS-DE-NOTES-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-DE-BUTTONS.
          05 WS-DE-BUTTONS-TEXT       PIC X(256) VALUE 'DE-BUTTONS'.
          05 WS-DE-BUTTONS-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-BUTTONS-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-CANCEL.
          05 WS-DE-CANCEL-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-DE-CANCEL-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-CANCEL-ENABLED    PIC 9      VALUE 1.

       01 WS-DE-SAVE.
          05 WS-DE-SAVE-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-DE-SAVE-VISIBLE    PIC 9      VALUE 1.
          05 WS-DE-SAVE-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-GRID-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-GRID-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-GRID-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-GRID-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-GRID-FORM--ONBREAKPOINTCHANGED"
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
       PROGRAM-ID. RESPONSIVE-GRID-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "GRD-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-GRID-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-GRID-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-GRID-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-GRID-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "GRD-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-GRID-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-GRID-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "GRD-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-GRID-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. GRD-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM GRD-SHOW-INFO.

       END PROGRAM RESPONSIVE-GRID-FORM.

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
       PROGRAM-ID. RESPONSIVE-FLOW-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESPONSIVE-FLOW-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-NUM GLOBAL PIC 9(5).
       01 WS-WIN-W GLOBAL PIC Z(4)9.
       01 WS-WIN-H GLOBAL PIC Z(4)9.
       01 WS-BP GLOBAL PIC X(20).
       01 WS-FSC GLOBAL PIC X(12).
       01 WS-LINE GLOBAL PIC X(200).

      *>── Form controls ───────────────────────────────────────────────
       01 WS-LBL-TITLE.
          05 WS-LBL-TITLE-TEXT       PIC X(256) VALUE 'Flow: items keep their size and wrap like words'.
          05 WS-LBL-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-LBL-INTRO.
          05 WS-LBL-INTRO-TEXT       PIC X(256) VALUE 'This form''s own LayoutMode is Flex (a column), so the title, this text, the bar and the grid below are its flex items.'.
          05 WS-LBL-INTRO-VISIBLE    PIC 9      VALUE 1.
          05 WS-LBL-INTRO-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-BAR.
          05 WS-PNL-BAR-TEXT       PIC X(256) VALUE 'PNL-BAR'.
          05 WS-PNL-BAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-BAR-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-LTR.
          05 WS-FB-LTR-TEXT       PIC X(256) VALUE 'LeftToRight'.
          05 WS-FB-LTR-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-LTR-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-RTL.
          05 WS-FB-RTL-TEXT       PIC X(256) VALUE 'RightToLeft'.
          05 WS-FB-RTL-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-RTL-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-TD.
          05 WS-FB-TD-TEXT       PIC X(256) VALUE 'TopDown'.
          05 WS-FB-TD-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-TD-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-BU.
          05 WS-FB-BU-TEXT       PIC X(256) VALUE 'BottomUp'.
          05 WS-FB-BU-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-BU-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-WON.
          05 WS-FB-WON-TEXT       PIC X(256) VALUE 'Wrap on'.
          05 WS-FB-WON-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-WON-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-WOFF.
          05 WS-FB-WOFF-TEXT       PIC X(256) VALUE 'Wrap off'.
          05 WS-FB-WOFF-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-WOFF-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-G2.
          05 WS-FB-G2-TEXT       PIC X(256) VALUE 'Gap 2'.
          05 WS-FB-G2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-G2-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-G16.
          05 WS-FB-G16-TEXT       PIC X(256) VALUE 'Gap 16'.
          05 WS-FB-G16-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-G16-ENABLED    PIC 9      VALUE 1.

       01 WS-FB-BRK.
          05 WS-FB-BRK-TEXT       PIC X(256) VALUE 'Break after 3'.
          05 WS-FB-BRK-VISIBLE    PIC 9      VALUE 1.
          05 WS-FB-BRK-ENABLED    PIC 9      VALUE 1.

       01 WS-PNL-GRID.
          05 WS-PNL-GRID-TEXT       PIC X(256) VALUE 'PNL-GRID'.
          05 WS-PNL-GRID-VISIBLE    PIC 9      VALUE 1.
          05 WS-PNL-GRID-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1.
          05 WS-FL-1-TEXT       PIC X(256) VALUE 'Live - the bar above changes this one'.
          05 WS-FL-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-1.
          05 WS-FL-1-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-1-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-2.
          05 WS-FL-1-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-1-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-3.
          05 WS-FL-1-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-1-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-4.
          05 WS-FL-1-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-1-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-5.
          05 WS-FL-1-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-1-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-6.
          05 WS-FL-1-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-1-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-7.
          05 WS-FL-1-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-1-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-8.
          05 WS-FL-1-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-1-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-9.
          05 WS-FL-1-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-1-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-10.
          05 WS-FL-1-10-TEXT       PIC X(256) VALUE 'break'.
          05 WS-FL-1-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-10-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-11.
          05 WS-FL-1-11-TEXT       PIC X(256) VALUE 'order'.
          05 WS-FL-1-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-11-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-1-12.
          05 WS-FL-1-12-TEXT       PIC X(256) VALUE 'fonts'.
          05 WS-FL-1-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-1-12-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2.
          05 WS-FL-2-TEXT       PIC X(256) VALUE 'FlowDirection = RightToLeft'.
          05 WS-FL-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-1.
          05 WS-FL-2-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-2-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-2.
          05 WS-FL-2-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-2-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-3.
          05 WS-FL-2-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-2-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-4.
          05 WS-FL-2-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-2-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-5.
          05 WS-FL-2-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-2-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-6.
          05 WS-FL-2-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-2-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-7.
          05 WS-FL-2-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-2-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-8.
          05 WS-FL-2-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-2-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-9.
          05 WS-FL-2-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-2-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-10.
          05 WS-FL-2-10-TEXT       PIC X(256) VALUE 'break'.
          05 WS-FL-2-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-10-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-11.
          05 WS-FL-2-11-TEXT       PIC X(256) VALUE 'order'.
          05 WS-FL-2-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-11-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-2-12.
          05 WS-FL-2-12-TEXT       PIC X(256) VALUE 'fonts'.
          05 WS-FL-2-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-2-12-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3.
          05 WS-FL-3-TEXT       PIC X(256) VALUE 'FlowDirection = TopDown'.
          05 WS-FL-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-1.
          05 WS-FL-3-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-3-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-2.
          05 WS-FL-3-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-3-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-3.
          05 WS-FL-3-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-3-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-4.
          05 WS-FL-3-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-3-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-5.
          05 WS-FL-3-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-3-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-6.
          05 WS-FL-3-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-3-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-7.
          05 WS-FL-3-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-3-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-8.
          05 WS-FL-3-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-3-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-3-9.
          05 WS-FL-3-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-3-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-3-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4.
          05 WS-FL-4-TEXT       PIC X(256) VALUE 'FlowDirection = BottomUp'.
          05 WS-FL-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-1.
          05 WS-FL-4-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-4-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-2.
          05 WS-FL-4-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-4-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-3.
          05 WS-FL-4-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-4-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-4.
          05 WS-FL-4-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-4-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-5.
          05 WS-FL-4-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-4-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-6.
          05 WS-FL-4-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-4-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-7.
          05 WS-FL-4-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-4-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-8.
          05 WS-FL-4-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-4-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-4-9.
          05 WS-FL-4-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-4-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-4-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5.
          05 WS-FL-5-TEXT       PIC X(256) VALUE 'WrapContents = false - one line, clipped'.
          05 WS-FL-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-1.
          05 WS-FL-5-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-5-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-2.
          05 WS-FL-5-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-5-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-3.
          05 WS-FL-5-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-5-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-4.
          05 WS-FL-5-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-5-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-5.
          05 WS-FL-5-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-5-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-6.
          05 WS-FL-5-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-5-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-7.
          05 WS-FL-5-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-5-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-8.
          05 WS-FL-5-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-5-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-9.
          05 WS-FL-5-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-5-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-10.
          05 WS-FL-5-10-TEXT       PIC X(256) VALUE 'break'.
          05 WS-FL-5-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-10-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-11.
          05 WS-FL-5-11-TEXT       PIC X(256) VALUE 'order'.
          05 WS-FL-5-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-11-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-5-12.
          05 WS-FL-5-12-TEXT       PIC X(256) VALUE 'fonts'.
          05 WS-FL-5-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-5-12-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6.
          05 WS-FL-6-TEXT       PIC X(256) VALUE 'FlowBreak after ''forms'', Gap 12'.
          05 WS-FL-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-1.
          05 WS-FL-6-1-TEXT       PIC X(256) VALUE 'COBOL'.
          05 WS-FL-6-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-1-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-2.
          05 WS-FL-6-2-TEXT       PIC X(256) VALUE 'Rust'.
          05 WS-FL-6-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-2-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-3.
          05 WS-FL-6-3-TEXT       PIC X(256) VALUE 'forms'.
          05 WS-FL-6-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-3-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-4.
          05 WS-FL-6-4-TEXT       PIC X(256) VALUE 'anchors'.
          05 WS-FL-6-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-4-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-5.
          05 WS-FL-6-5-TEXT       PIC X(256) VALUE 'grid'.
          05 WS-FL-6-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-5-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-6.
          05 WS-FL-6-6-TEXT       PIC X(256) VALUE 'flex'.
          05 WS-FL-6-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-6-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-7.
          05 WS-FL-6-7-TEXT       PIC X(256) VALUE 'flow'.
          05 WS-FL-6-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-7-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-8.
          05 WS-FL-6-8-TEXT       PIC X(256) VALUE 'wrap'.
          05 WS-FL-6-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-8-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-9.
          05 WS-FL-6-9-TEXT       PIC X(256) VALUE 'gap'.
          05 WS-FL-6-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-9-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-10.
          05 WS-FL-6-10-TEXT       PIC X(256) VALUE 'break'.
          05 WS-FL-6-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-10-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-11.
          05 WS-FL-6-11-TEXT       PIC X(256) VALUE 'order'.
          05 WS-FL-6-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-11-ENABLED    PIC 9      VALUE 1.

       01 WS-FL-6-12.
          05 WS-FL-6-12-TEXT       PIC X(256) VALUE 'fonts'.
          05 WS-FL-6-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-FL-6-12-ENABLED    PIC 9      VALUE 1.

       01 WS-SB-INFO.
          05 WS-SB-INFO-TEXT       PIC X(256) VALUE 'SB-INFO'.
          05 WS-SB-INFO-VISIBLE    PIC 9      VALUE 1.
          05 WS-SB-INFO-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESPONSIVE-FLOW-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESPONSIVE-FLOW-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "RESPONSIVE-FLOW-FORM"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResize"
                               CALL "RESPONSIVE-FLOW-FORM--ONRESIZE"
                           WHEN "onBreakpointChanged"
                               CALL "RESPONSIVE-FLOW-FORM--ONBREAKPOINTCHANGED"
                       END-EVALUATE
                   WHEN "FB-LTR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-LTR--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-RTL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-RTL--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-TD"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-TD--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-BU"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-BU--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-WON"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-WON--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-WOFF"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-WOFF--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-G2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-G2--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-G16"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-G16--ONCLICK"
                       END-EVALUATE
                   WHEN "FB-BRK"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "FB-BRK--ONCLICK"
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
       PROGRAM-ID. RESPONSIVE-FLOW-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLW-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLOW-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLOW-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESPONSIVE-FLOW-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLOW-FORM--ONRESIZE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLW-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLOW-FORM--ONRESIZE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESPONSIVE-FLOW-FORM--ONBREAKPOINTCHANGED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE me::Width TO WS-NUM
           MOVE WS-NUM TO WS-WIN-W
           MOVE me::Height TO WS-NUM
           MOVE WS-NUM TO WS-WIN-H
           MOVE me::Breakpoint TO WS-BP
           MOVE me::FontScale TO WS-FSC
           CALL "FLW-SHOW-INFO"

           GOBACK.

       END PROGRAM RESPONSIVE-FLOW-FORM--ONBREAKPOINTCHANGED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-LTR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "LeftToRight" TO FL-1::FlowDirection

           GOBACK.

       END PROGRAM FB-LTR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-RTL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "RightToLeft" TO FL-1::FlowDirection

           GOBACK.

       END PROGRAM FB-RTL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-TD--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "TopDown" TO FL-1::FlowDirection

           GOBACK.

       END PROGRAM FB-TD--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-BU--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE "BottomUp" TO FL-1::FlowDirection

           GOBACK.

       END PROGRAM FB-BU--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-WON--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO FL-1::WrapContents

           GOBACK.

       END PROGRAM FB-WON--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-WOFF--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 0 TO FL-1::WrapContents

           GOBACK.

       END PROGRAM FB-WOFF--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-G2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 2 TO FL-1::Gap

           GOBACK.

       END PROGRAM FB-G2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-G16--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 16 TO FL-1::Gap

           GOBACK.

       END PROGRAM FB-G16--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FB-BRK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           MOVE 1 TO FL-1-3::FlowBreak

           GOBACK.

       END PROGRAM FB-BRK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLW-SHOW-INFO IS COMMON PROGRAM.

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

       END PROGRAM FLW-SHOW-INFO.

       END PROGRAM RESPONSIVE-FLOW-FORM.

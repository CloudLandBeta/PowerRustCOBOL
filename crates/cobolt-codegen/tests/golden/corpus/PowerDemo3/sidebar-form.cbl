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
       PROGRAM-ID. SIDEBAR-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SIDEBAR-FORM'.

      *>── Animation runtime fields ──────────────────────────────────
      *>   INVOKE ctrl-id 'PlayAnimation' USING BY VALUE WS-ANIM-NAME
       01 WS-ANIM-NAME          PIC X(128)  VALUE SPACES.
       01 WS-ANIM-ELAPSED-MS    PIC 9(8)    VALUE 0.

      *>── Timer: Timer-1 ──────────────────────────────────────────
       01 WS-Timer-1-INTERVAL   PIC 9(8) VALUE 100.
       01 WS-Timer-1-ENABLED    PIC 9    VALUE 1.
       01 WS-Timer-1-ELAPSED-MS PIC 9(8) VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-SideMenu-1.
          05 WS-SideMenu-1-TEXT       PIC X(256) VALUE 'SideMenu-1'.
          05 WS-SideMenu-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-SideMenu-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-1.
          05 WS-Panel-1-TEXT       PIC X(256) VALUE 'Panel-1'.
          05 WS-Panel-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-1-ENABLED    PIC 9      VALUE 1.

       01 WS-SideMenu-1-Footer.
          05 WS-SideMenu-1-Footer-TEXT       PIC X(256) VALUE 'SideMenu-1-Footer'.
          05 WS-SideMenu-1-Footer-VISIBLE    PIC 9      VALUE 1.
          05 WS-SideMenu-1-Footer-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-1.
          05 WS-TextBox-1-TEXT       PIC X(256) VALUE 'TextBox-1'.
          05 WS-TextBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-1-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Panel-2.
          05 WS-Panel-2-TEXT       PIC X(256) VALUE 'Panel-2'.
          05 WS-Panel-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-3.
          05 WS-Panel-3-TEXT       PIC X(256) VALUE 'Panel-3'.
          05 WS-Panel-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-4.
          05 WS-Panel-4-TEXT       PIC X(256) VALUE 'Panel-4'.
          05 WS-Panel-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-4-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-5.
          05 WS-Panel-5-TEXT       PIC X(256) VALUE 'Panel-5'.
          05 WS-Panel-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-5-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-7.
          05 WS-Panel-7-TEXT       PIC X(256) VALUE 'Panel-7'.
          05 WS-Panel-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-7-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-8.
          05 WS-Panel-8-TEXT       PIC X(256) VALUE 'Panel-8'.
          05 WS-Panel-8-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-8-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Treeview'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-2.
          05 WS-Label-2-TEXT       PIC X(256) VALUE 'PowerDemo 3'.
          05 WS-Label-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-3.
          05 WS-Label-3-TEXT       PIC X(256) VALUE 'Progress bar'.
          05 WS-Label-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-3-ENABLED    PIC 9      VALUE 1.

       01 WS-ProgressBar-1.
          05 WS-ProgressBar-1-TEXT       PIC X(256) VALUE 'ProgressBar-1'.
          05 WS-ProgressBar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ProgressBar-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-4.
          05 WS-Label-4-TEXT       PIC X(256) VALUE 'Checkbox'.
          05 WS-Label-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-4-ENABLED    PIC 9      VALUE 1.

       01 WS-Line-1.
          05 WS-Line-1-TEXT       PIC X(256) VALUE 'Line-1'.
          05 WS-Line-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Line-1-ENABLED    PIC 9      VALUE 1.

       01 WS-lblSolicitacoes.
          05 WS-lblSolicitacoes-TEXT       PIC X(256) VALUE 'Solicitações'.
          05 WS-lblSolicitacoes-VISIBLE    PIC 9      VALUE 1.
          05 WS-lblSolicitacoes-ENABLED    PIC 9      VALUE 1.

       01 WS-lblSolicitacoesNew.
          05 WS-lblSolicitacoesNew-TEXT       PIC X(256) VALUE 'Buttons + Icons'.
          05 WS-lblSolicitacoesNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-lblSolicitacoesNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnAbrirNew.
          05 WS-btnAbrirNew-TEXT       PIC X(256) VALUE 'Open'.
          05 WS-btnAbrirNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnAbrirNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnAlterarNew.
          05 WS-btnAlterarNew-TEXT       PIC X(256) VALUE 'Edit'.
          05 WS-btnAlterarNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnAlterarNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnExcluirNew.
          05 WS-btnExcluirNew-TEXT       PIC X(256) VALUE 'Delete'.
          05 WS-btnExcluirNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnExcluirNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnAprovarNew.
          05 WS-btnAprovarNew-TEXT       PIC X(256) VALUE 'Approve'.
          05 WS-btnAprovarNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnAprovarNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnRejeitarNew.
          05 WS-btnRejeitarNew-TEXT       PIC X(256) VALUE 'Reject'.
          05 WS-btnRejeitarNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnRejeitarNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnEncaminharNew.
          05 WS-btnEncaminharNew-TEXT       PIC X(256) VALUE 'Forward'.
          05 WS-btnEncaminharNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnEncaminharNew-ENABLED    PIC 9      VALUE 1.

       01 WS-btnArquivarNew.
          05 WS-btnArquivarNew-TEXT       PIC X(256) VALUE 'Archive'.
          05 WS-btnArquivarNew-VISIBLE    PIC 9      VALUE 1.
          05 WS-btnArquivarNew-ENABLED    PIC 9      VALUE 1.

       01 WS-Switch-1.
          05 WS-Switch-1-TEXT       PIC X(256) VALUE 'Switch-1'.
          05 WS-Switch-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Switch-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Switch-1-CHECKED    PIC 9      VALUE 0.

       01 WS-Label-7.
          05 WS-Label-7-TEXT       PIC X(256) VALUE 'On'.
          05 WS-Label-7-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-7-ENABLED    PIC 9      VALUE 1.

       01 WS-CheckBox-1.
          05 WS-CheckBox-1-TEXT       PIC X(256) VALUE 'CheckBox-1'.
          05 WS-CheckBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-CheckBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-CheckBox-1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-CheckBox-2.
          05 WS-CheckBox-2-TEXT       PIC X(256) VALUE 'CheckBox-2'.
          05 WS-CheckBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-CheckBox-2-ENABLED    PIC 9      VALUE 1.
          05 WS-CheckBox-2-VALUE      PIC X(512) VALUE SPACES.

       01 WS-ToolBar-1.
          05 WS-ToolBar-1-TEXT       PIC X(256) VALUE 'ToolBar-1'.
          05 WS-ToolBar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ToolBar-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-10.
          05 WS-Label-10-TEXT       PIC X(256) VALUE 'Radio'.
          05 WS-Label-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-10-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-9.
          05 WS-Panel-9-TEXT       PIC X(256) VALUE 'Panel-9'.
          05 WS-Panel-9-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-9-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-11.
          05 WS-Label-11-TEXT       PIC X(256) VALUE 'Knob'.
          05 WS-Label-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-11-ENABLED    PIC 9      VALUE 1.

       01 WS-Knob-2.
          05 WS-Knob-2-TEXT       PIC X(256) VALUE 'Knob-2'.
          05 WS-Knob-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Knob-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Knob-2-VALUE      PIC S9(9) VALUE 42.
          05 WS-Knob-2-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Knob-2-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Knob-2-STEP       PIC S9(9) VALUE 1.
          05 WS-Knob-2-DEFAULT    PIC S9(9) VALUE 0.

       01 WS-RadioButton-1.
          05 WS-RadioButton-1-TEXT       PIC X(256) VALUE 'RadioButton-1'.
          05 WS-RadioButton-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-12.
          05 WS-Label-12-TEXT       PIC X(256) VALUE 'Switch'.
          05 WS-Label-12-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-12-ENABLED    PIC 9      VALUE 1.

       01 WS-Line-2.
          05 WS-Line-2-TEXT       PIC X(256) VALUE 'Line-2'.
          05 WS-Line-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Line-2-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-2.
          05 WS-RadioButton-2-TEXT       PIC X(256) VALUE 'RadioButton-2'.
          05 WS-RadioButton-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-2-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-3.
          05 WS-RadioButton-3-TEXT       PIC X(256) VALUE 'RadioButton-3'.
          05 WS-RadioButton-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-3-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-4.
          05 WS-RadioButton-4-TEXT       PIC X(256) VALUE 'RadioButton-4'.
          05 WS-RadioButton-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-4-ENABLED    PIC 9      VALUE 1.

       01 WS-Panel-10.
          05 WS-Panel-10-TEXT       PIC X(256) VALUE 'Panel-10'.
          05 WS-Panel-10-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-10-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-13.
          05 WS-Label-13-TEXT       PIC X(256) VALUE 'Gauge'.
          05 WS-Label-13-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-13-ENABLED    PIC 9      VALUE 1.

       01 WS-Gauge-1.
          05 WS-Gauge-1-TEXT       PIC X(256) VALUE 'Gauge-1'.
          05 WS-Gauge-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-1-VALUE      PIC S9(9) VALUE 45.
          05 WS-Gauge-1-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-1-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-1-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-Gauge-2.
          05 WS-Gauge-2-TEXT       PIC X(256) VALUE 'Gauge-2'.
          05 WS-Gauge-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-2-VALUE      PIC S9(9) VALUE 63.
          05 WS-Gauge-2-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-2-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-2-STYLE      PIC X(10)  VALUE 'Donut'.

       01 WS-TreeView-1.
          05 WS-TreeView-1-TEXT       PIC X(256) VALUE 'TreeView-1'.
          05 WS-TreeView-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TreeView-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-1.
          05 WS-Splitter-1-TEXT       PIC X(256) VALUE 'Splitter-1'.
          05 WS-Splitter-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-1-Pane1.
          05 WS-Splitter-1-Pane1-TEXT       PIC X(256) VALUE 'Splitter-1-Pane1'.
          05 WS-Splitter-1-Pane1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-1-Pane1-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-1-Pane2.
          05 WS-Splitter-1-Pane2-TEXT       PIC X(256) VALUE 'Splitter-1-Pane2'.
          05 WS-Splitter-1-Pane2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-1-Pane2-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-14.
          05 WS-Label-14-TEXT       PIC X(256) VALUE 'Long text'.
          05 WS-Label-14-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-14-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-2.
          05 WS-TextBox-2-TEXT       PIC X(2048) VALUE 'Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.'.
          05 WS-TextBox-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-2-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-2-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Panel-11.
          05 WS-Panel-11-TEXT       PIC X(256) VALUE 'Panel-11'.
          05 WS-Panel-11-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-11-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-2.
          05 WS-Splitter-2-TEXT       PIC X(256) VALUE 'Splitter-2'.
          05 WS-Splitter-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-2-Pane1.
          05 WS-Splitter-2-Pane1-TEXT       PIC X(256) VALUE 'Splitter-2-Pane1'.
          05 WS-Splitter-2-Pane1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-2-Pane1-ENABLED    PIC 9      VALUE 1.

       01 WS-Splitter-2-Pane2.
          05 WS-Splitter-2-Pane2-TEXT       PIC X(256) VALUE 'Splitter-2-Pane2'.
          05 WS-Splitter-2-Pane2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Splitter-2-Pane2-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-3.
          05 WS-TextBox-3-TEXT       PIC X(256) VALUE 'TextBox-3'.
          05 WS-TextBox-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-3-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-3-VALUE      PIC X(256) VALUE SPACES.

       01 WS-RadioButton-5.
          05 WS-RadioButton-5-TEXT       PIC X(256) VALUE 'RadioButton'.
          05 WS-RadioButton-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-5-ENABLED    PIC 9      VALUE 1.

       01 WS-Gauge-3.
          05 WS-Gauge-3-TEXT       PIC X(256) VALUE 'Gauge-3'.
          05 WS-Gauge-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Gauge-3-ENABLED    PIC 9      VALUE 1.
          05 WS-Gauge-3-VALUE      PIC S9(9) VALUE 0.
          05 WS-Gauge-3-MINIMUM    PIC S9(9) VALUE 0.
          05 WS-Gauge-3-MAXIMUM    PIC S9(9) VALUE 100.
          05 WS-Gauge-3-STYLE      PIC X(10)  VALUE 'Radial'.

       01 WS-PictureBox-1.
          05 WS-PictureBox-1-TEXT       PIC X(256) VALUE 'PictureBox-1'.
          05 WS-PictureBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-PictureBox-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Timer-1.
          05 WS-Timer-1-TEXT       PIC X(256) VALUE 'Timer-1'.
          05 WS-Timer-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Timer-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           PERFORM COBOL-START-TIMERS
           CALL "SIDEBAR-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SIDEBAR-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "btnAbrirNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNABRIRNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "btnAlterarNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNALTERARNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "btnExcluirNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNEXCLUIRNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "btnAprovarNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNAPROVARNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "btnRejeitarNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNREJEITARNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "btnEncaminharNew"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTNENCAMINHARNEW--ONCLICK"
                       END-EVALUATE
                   WHEN "Switch-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "SWITCH-1--ONCLICK"
                       END-EVALUATE
                   WHEN "ToolBar-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "TOOLBAR-1--ONCLICK"
                       END-EVALUATE
                   WHEN "Knob-2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onChange"
                               CALL "KNOB-2--ONCHANGE"
                       END-EVALUATE
                   WHEN "RadioButton-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-1--ONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-2"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-2--ONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-3"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-3--ONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-4"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-4--ONCLICK"
                       END-EVALUATE
                   WHEN "Timer-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onTick"
                               CALL "TIMER-1--ONTICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
       COBOL-START-TIMERS.
      *>    Called once from COBOL-MAIN to register timer intervals.
           INVOKE Timer-1 'SetInterval' USING BY VALUE 100
           CONTINUE.

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
               WHEN "appear"
                   INVOKE Panel-8 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnAbrirNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnAlterarNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnExcluirNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnAprovarNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnRejeitarNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnEncaminharNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE btnArquivarNew 'PlayAnimation' USING BY VALUE "appear"
                   INVOKE ToolBar-1 'PlayAnimation' USING BY VALUE "appear"
               WHEN "anim1"
                   INVOKE TreeView-1 'PlayAnimation' USING BY VALUE "anim1"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       COBOL-STOP-ANIMATION.
      *> Set WS-ANIM-NAME before calling this paragraph.
           EVALUATE WS-ANIM-NAME
               WHEN "appear"
                   INVOKE Panel-8 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnAbrirNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnAlterarNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnExcluirNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnAprovarNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnRejeitarNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnEncaminharNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE btnArquivarNew 'StopAnimation' USING BY VALUE "appear"
                   INVOKE ToolBar-1 'StopAnimation' USING BY VALUE "appear"
               WHEN "anim1"
                   INVOKE TreeView-1 'StopAnimation' USING BY VALUE "anim1"
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       Panel-8-PLAY-APPEAR.
           INVOKE Panel-8 'PlayAnimation' USING BY VALUE "appear".

       btnAbrirNew-PLAY-APPEAR.
           INVOKE btnAbrirNew 'PlayAnimation' USING BY VALUE "appear".

       btnAlterarNew-PLAY-APPEAR.
           INVOKE btnAlterarNew 'PlayAnimation' USING BY VALUE "appear".

       btnExcluirNew-PLAY-APPEAR.
           INVOKE btnExcluirNew 'PlayAnimation' USING BY VALUE "appear".

       btnAprovarNew-PLAY-APPEAR.
           INVOKE btnAprovarNew 'PlayAnimation' USING BY VALUE "appear".

       btnRejeitarNew-PLAY-APPEAR.
           INVOKE btnRejeitarNew 'PlayAnimation' USING BY VALUE "appear".

       btnEncaminharNew-PLAY-APPEAR.
           INVOKE btnEncaminharNew 'PlayAnimation' USING BY VALUE "appear".

       btnArquivarNew-PLAY-APPEAR.
           INVOKE btnArquivarNew 'PlayAnimation' USING BY VALUE "appear".

       ToolBar-1-PLAY-APPEAR.
           INVOKE ToolBar-1 'PlayAnimation' USING BY VALUE "appear".

       TreeView-1-PLAY-ANIM1.
           INVOKE TreeView-1 'PlayAnimation' USING BY VALUE "anim1".


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SIDEBAR-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SIDEBAR-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SIDEBAR-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SIDEBAR-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNABRIRNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.

           IF CheckBox-1::Checked IS true
              SET CheckBox-1::Checked TO false
           ELSE
              SET CheckBox-1::Checked TO true
           END-IF


           CONTINUE.

           GOBACK.

       END PROGRAM BTNABRIRNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNALTERARNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET panel-7::BackgroundColor to "0000FFFF"
           CONTINUE.

           GOBACK.

       END PROGRAM BTNALTERARNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNEXCLUIRNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET  CheckBox-2::Visible TO  false
           CONTINUE.

           GOBACK.

       END PROGRAM BTNEXCLUIRNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNAPROVARNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET  CheckBox-2::Visible  TO  true
           CONTINUE.

           GOBACK.

       END PROGRAM BTNAPROVARNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNREJEITARNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET Knob-2::Value  TO  false
           SET Timer-1::Enabled TO false
           CONTINUE.

           GOBACK.

       END PROGRAM BTNREJEITARNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTNENCAMINHARNEW--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET Timer-1::Enabled  TO  true
           CONTINUE.

           GOBACK.

       END PROGRAM BTNENCAMINHARNEW--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SWITCH-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           DISPLAY Switch-1::Checked " - " Label-7::Visible

           if Switch-1::Checked is false
              SET Label-7::Visible TO  false
           ELSE
              SET Label-7::Visible TO  true
           end-if

           CONTINUE.

           GOBACK.

       END PROGRAM SWITCH-1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TOOLBAR-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.

       PROCEDURE DIVISION.
      *>   in the TOOLBAR-1 onClick handler:
       DISPLAY  "TOOLBAR-1::LastButton = " TOOLBAR-1::LastButton

       GOBACK.

           GOBACK.

       END PROGRAM TOOLBAR-1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. KNOB-2--ONCHANGE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET ProgressBar-1::VALUE
               Gauge-1::VALUE
               Gauge-2::VALUE         TO Knob-2::VALUE

           CONTINUE.

           GOBACK.

       END PROGRAM KNOB-2--ONCHANGE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET TextBox-2::FontSize TO 12
           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-2--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET TextBox-2::FontSize TO 14
           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-2--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-3--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET TextBox-2::FontSize TO 16
           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-3--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-4--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET TextBox-2::FontSize TO 18

           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-4--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TIMER-1--ONTICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           if Knob-2::Value < 100
              COMPUTE  Knob-2::Value = Knob-2::Value + 1
              COMPUTE  ProgressBar-1::Value = Knob-2::Value
              COMPUTE  Gauge-1::Value = Knob-2::Value
              COMPUTE  Gauge-2::Value = Knob-2::Value
              COMPUTE  Gauge-3::Value = Knob-2::Value

              IF Knob-2::Value = 100
                 SET Knob-2::Value to 0
              END-iF

           CONTINUE.

           GOBACK.

       END PROGRAM TIMER-1--ONTICK.

       END PROGRAM SIDEBAR-FORM.

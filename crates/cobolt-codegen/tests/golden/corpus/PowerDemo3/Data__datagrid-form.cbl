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
       PROGRAM-ID. DATAGRID-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'DATAGRID-FORM'.

      *>── DataGrid DataGrid-1 CSV export ──────────────────────────
       01 WS-DataGrid-1-CSV-PATH    PIC X(512)  VALUE SPACES.
       01 WS-DataGrid-1-CSV-STATUS  PIC 9       VALUE 0.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Panel-1.
          05 WS-Panel-1-TEXT       PIC X(256) VALUE 'Panel-1'.
          05 WS-Panel-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Panel-1-ENABLED    PIC 9      VALUE 1.

       01 WS-DataGrid-1.
          05 WS-DataGrid-1-TEXT       PIC X(256) VALUE 'DataGrid-1'.
          05 WS-DataGrid-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-DataGrid-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Datagrid Demo'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-2.
          05 WS-Label-2-TEXT       PIC X(333) VALUE 'PowerRustCOBOL DataGrids are powerful tools capable of loading virtually any number of rows.  They offer numerous configuration options, optional per-column filters, CSV file import and export, data binding to indexed files, COBOL data items, and SQL tables, as well as a programmatic interface for creating content directly in code.'.
          05 WS-Label-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-1.
          05 WS-Button-1-TEXT       PIC X(256) VALUE 'Load data'.
          05 WS-Button-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Snackbar-1.
          05 WS-Snackbar-1-TEXT       PIC X(256) VALUE 'Snackbar-1'.
          05 WS-Snackbar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Snackbar-1-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-4.
          05 WS-RadioButton-4-TEXT       PIC X(256) VALUE 'Português'.
          05 WS-RadioButton-4-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-4-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-5.
          05 WS-RadioButton-5-TEXT       PIC X(256) VALUE 'Español'.
          05 WS-RadioButton-5-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-5-ENABLED    PIC 9      VALUE 1.

       01 WS-RadioButton-6.
          05 WS-RadioButton-6-TEXT       PIC X(256) VALUE 'English'.
          05 WS-RadioButton-6-VISIBLE    PIC 9      VALUE 1.
          05 WS-RadioButton-6-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "DATAGRID-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "DATAGRID-FORM--ONCLOSE"
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
                   WHEN "Snackbar-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onShown"
                               CALL "SNACKBAR-1--ONSHOWN"
                           WHEN "onClosed"
                               CALL "SNACKBAR-1--ONCLOSED"
                           WHEN "onButtonClick"
                               CALL "SNACKBAR-1--ONBUTTONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-4"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-4--ONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-5"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-5--ONCLICK"
                       END-EVALUATE
                   WHEN "RadioButton-6"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "RADIOBUTTON-6--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
       DataGrid-1-EXPORT-CSV.
      *>    Export DataGrid-1 data to CSV file.  Delimiter: ",". Mode: Filtered.
      *>    Column order and filtered/all rows follow the DataGrid settings.
      *>    Set WS-DataGrid-1-CSV-PATH to the desired output file path before calling.
           INVOKE DataGrid-1 'ExportCSV'
               USING BY REFERENCE WS-DataGrid-1-CSV-PATH
               RETURNING WS-DataGrid-1-CSV-STATUS
           IF WS-DataGrid-1-CSV-STATUS NOT = 0
               DISPLAY "CSV export error: " WS-DataGrid-1-CSV-STATUS
           END-IF.

      *> </CSV-EXPORT>
      *> <REST-CLIENT>
      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DATAGRID-FORM--ONLOAD IS COMMON PROGRAM.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM DATAGRID-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DATAGRID-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM DATAGRID-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-1--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ROW-COUNT      PIC 9(9).
       01 WS-ROW-COUNT-TEXT PIC ZZZ,ZZZ,ZZ9.
       01 WS-MESSAGE        PIC X(80).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> The grid is bound to ACTORS-FILE (indexed/actors.cidx), so the rows
      *> already arrived when the form loaded — an indexed source has no fill
      *> step to wait for. RefreshBinding re-reads the file and returns how
      *> many records it put in the grid, which is what this button is for:
      *> picking up records another program has written since.
           MOVE DataGrid-1::RefreshBinding() TO WS-ROW-COUNT
           MOVE WS-ROW-COUNT TO WS-ROW-COUNT-TEXT

           STRING "Loaded " DELIMITED BY SIZE
                  FUNCTION TRIM(WS-ROW-COUNT-TEXT) DELIMITED BY SIZE
                  " actors from ACTORS-FILE" DELIMITED BY SIZE
                  INTO WS-MESSAGE
           END-STRING

           SET Snackbar-1::Category TO "Info"
           SET Snackbar-1::Text TO FUNCTION TRIM(WS-MESSAGE)
           INVOKE Snackbar-1::Show()
           DISPLAY FUNCTION TRIM(WS-MESSAGE).

           GOBACK.

       END PROGRAM BUTTON-1--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONSHOWN IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           DISPLAY "onShown".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONSHOWN.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONCLOSED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> The reason arrives as the event value: Timeout / User / Action /
      *> Programmatic / Overflow.
           DISPLAY "onClosed".

           GOBACK.

       END PROGRAM SNACKBAR-1--ONCLOSED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SNACKBAR-1--ONBUTTONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           EVALUATE SNACKBAR-1::LastButtonId
               WHEN "retry"
                   DISPLAY "button: retry (this one dismisses)"
               WHEN "later"
                   DISPLAY "button: later (this one stays up)"
               WHEN OTHER
                   DISPLAY "button: " SNACKBAR-1::LastButtonId
           END-EVALUATE.

           GOBACK.

       END PROGRAM SNACKBAR-1--ONBUTTONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-4--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET label-2::Caption TO
```
Os DataGrids do PowerRustCOBOL são ferramentas poderosas, capazes de carregar praticamente
qualquer quantidade de linhas.

Oferecem inúmeras opções de configuração, filtros opcionais por coluna, importação e exportação
de arquivos CSV, vinculação de dados (data binding) com arquivos indexados, itens de dados COBOL
e tabelas SQL, além de uma interface programática para a criação de conteúdo diretamente por
código.
```
           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-4--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-5--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET label-2::Caption TO
```
Los DataGrids de PowerRustCOBOL son herramientas potentes capaces de cargar prácticamente
cualquier cantidad de filas.

Ofrecen numerosas opciones de configuración, filtros opcionales por columna, importación y exportación
de archivos CSV, enlace de datos con archivos indexados, elementos de datos COBOL y tablas SQL, además de una
interfaz programática para crear contenido directamente mediante código.
```
           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-5--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RADIOBUTTON-6--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           SET label-2::Caption TO
```
PowerRustCOBOL DataGrids are powerful tools capable of loading virtually any number of rows.

They offer numerous configuration options, optional per-column filters, CSV file import and
export, data binding to indexed files, COBOL data items, and SQL tables, as well as a
programmatic interface for creating content directly in code.
```

           CONTINUE.

           GOBACK.

       END PROGRAM RADIOBUTTON-6--ONCLICK.

       END PROGRAM DATAGRID-FORM.

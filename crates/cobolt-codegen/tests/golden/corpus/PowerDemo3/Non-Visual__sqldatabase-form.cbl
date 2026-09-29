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
       PROGRAM-ID. SQLDATABASE-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'SQLDATABASE-FORM'.

      *>── SQL Database runtime variables ──────────────────────────────
      *>   Usage:
      *>     MOVE 'SELECT * FROM t' TO WS-SQL-QUERY
      *>     PERFORM DB1-CONNECT
      *>     PERFORM DB1-EXEC
      *>     PERFORM UNTIL WS-SQL-MORE = 'N'
      *>         MOVE 1 TO WS-SQL-COL-INDEX
      *>         COBOL::"FETCH-ROW" ( WS-DB1-HANDLE
      *>                                  WS-SQL-COL-INDEX
      *>                                  WS-SQL-CURRENT-VALUE
      *>                                  WS-SQL-ERROR )
      *>         COBOL::"NEXT-ROW" ( WS-DB1-HANDLE WS-SQL-MORE )
      *>     END-PERFORM
       01 WS-SQL-QUERY           PIC X(4096)  VALUE SPACES.
       01 WS-SQL-ERROR            PIC X(512)   VALUE SPACES.
       01 WS-SQL-ROW-COUNT        PIC 9(9)     VALUE 0.
       01 WS-SQL-COL-INDEX        PIC 9(4)     VALUE 1.
       01 WS-SQL-CURRENT-VALUE    PIC X(512)   VALUE SPACES.
       01 WS-SQL-MORE             PIC X(1)     VALUE 'N'.

      *>── SQL instance: Db-Local (sqlite) ─────────────────────────────────
       01 WS-Db-Local-CONN-STRING   PIC X(512)  VALUE 'sqlite::memory:'.
       01 WS-Db-Local-HANDLE        PIC 9(9)    VALUE 0.
       01 WS-Db-Local-STATUS        PIC X(512)  VALUE SPACES.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'SqlDatabase'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(293) VALUE 'One control, three back ends: the ConnectionString''s scheme picks SQLite, PostgreSQL or MySQL. Open, Execute, Query, Fetch and Close are its methods. NOTE that COBOL''s own COMMIT / ROLLBACK verbs are INDEXED-file transactions - SQL transactions are executed as statements through this control.'.
          05 WS-Lbl-Sub-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sub-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Log-Cap.
          05 WS-Lbl-Log-Cap-TEXT       PIC X(256) VALUE 'Event log'.
          05 WS-Lbl-Log-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Log-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Log.
          05 WS-Txt-Log-TEXT       PIC X(2048) VALUE 'Txt-Log'.
          05 WS-Txt-Log-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Log-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Log-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Db-Local.
          05 WS-Db-Local-TEXT       PIC X(256) VALUE 'Db-Local'.
          05 WS-Db-Local-VISIBLE    PIC 9      VALUE 1.
          05 WS-Db-Local-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Tray.
          05 WS-Lbl-Tray-TEXT       PIC X(256) VALUE 'Db-Local  sqlite'.
          05 WS-Lbl-Tray-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Tray-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sql-Cap.
          05 WS-Lbl-Sql-Cap-TEXT       PIC X(256) VALUE 'Statement'.
          05 WS-Lbl-Sql-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Sql-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Sql.
          05 WS-Txt-Sql-TEXT       PIC X(2048) VALUE 'SELECT id, name FROM city ORDER BY name'.
          05 WS-Txt-Sql-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Sql-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Sql-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Lbl-Rows-Cap.
          05 WS-Lbl-Rows-Cap-TEXT       PIC X(256) VALUE 'Rows'.
          05 WS-Lbl-Rows-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Rows-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Lst-Rows.
          05 WS-Lst-Rows-TEXT       PIC X(256) VALUE 'Lst-Rows'.
          05 WS-Lst-Rows-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Rows-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Rows-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Count.
          05 WS-Lbl-Count-TEXT       PIC X(256) VALUE '0 rows'.
          05 WS-Lbl-Count-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Count-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Open.
          05 WS-Btn-Open-TEXT       PIC X(256) VALUE 'Open + create'.
          05 WS-Btn-Open-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Open-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Query.
          05 WS-Btn-Query-TEXT       PIC X(256) VALUE 'Run the query'.
          05 WS-Btn-Query-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Query-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Tx.
          05 WS-Btn-Tx-TEXT       PIC X(256) VALUE 'A transaction'.
          05 WS-Btn-Tx-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Tx-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Close.
          05 WS-Btn-Close-TEXT       PIC X(256) VALUE 'Close'.
          05 WS-Btn-Close-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Close-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "SQLDATABASE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "SQLDATABASE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Db-Local"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onConnectOk"
                               CALL "DB-LOCAL--ONCONNECTOK"
                           WHEN "onConnectError"
                               CALL "DB-LOCAL--ONCONNECTERROR"
                           WHEN "onQueryError"
                               CALL "DB-LOCAL--ONQUERYERROR"
                       END-EVALUATE
                   WHEN "Btn-Open"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-OPEN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Query"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-QUERY--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Tx"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-TX--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Close"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLOSE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Clear"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLEAR--ONCLICK"
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
       Db-Local-CONNECT.
      *>  Open a SQLite connection for Db-Local.
      *>  Connection string is in WS-Db-Local-CONN-STRING.
      *>  On success: WS-Db-Local-HANDLE holds the connection handle.
      *>  On error:   WS-SQL-ERROR contains the message.
           MOVE SPACES TO WS-SQL-ERROR
           COBOL::"OPEN-DB" ( WS-Db-Local-CONN-STRING
                 WS-Db-Local-HANDLE
                 WS-SQL-ERROR )
           IF WS-SQL-ERROR NOT = SPACES
               PERFORM Db-Local-ON-ERROR
           ELSE
               PERFORM Db-Local-ON-CONNECT
           END-IF.

       Db-Local-EXEC.
      *>  Execute WS-SQL-QUERY via Db-Local.
      *>  Stores row count in WS-SQL-ROW-COUNT.
      *>  Resets WS-SQL-MORE to 'Y' if rows are present.
           MOVE SPACES TO WS-SQL-ERROR
           COBOL::"EXEC-SQL" ( WS-Db-Local-HANDLE
                 WS-SQL-QUERY
                 WS-SQL-ROW-COUNT
                 WS-SQL-ERROR )
           IF WS-SQL-ERROR NOT = SPACES
               PERFORM Db-Local-ON-ERROR
           ELSE
               IF WS-SQL-ROW-COUNT > 0
                   MOVE 'Y' TO WS-SQL-MORE
               ELSE
                   MOVE 'N' TO WS-SQL-MORE
               END-IF
               PERFORM Db-Local-ON-QUERY-DONE
           END-IF.

       Db-Local-FETCH-ALL.
      *>  Iterate over all rows returned by Db-Local-EXEC.
      *>  Copy this paragraph and add column reads inside the loop.
      *>  Example:
      *>    MOVE 1 TO WS-SQL-COL-INDEX
      *>    COBOL::"FETCH-ROW" ( WS-Db-Local-HANDLE
      *>                             WS-SQL-COL-INDEX
      *>                             WS-SQL-CURRENT-VALUE
      *>                             WS-SQL-ERROR )
      *>    MOVE WS-SQL-CURRENT-VALUE TO WS-MY-NAME-FIELD
           PERFORM UNTIL WS-SQL-MORE = 'N'
               MOVE 1 TO WS-SQL-COL-INDEX
               COBOL::"FETCH-ROW" ( WS-Db-Local-HANDLE
                     WS-SQL-COL-INDEX
                     WS-SQL-CURRENT-VALUE
                     WS-SQL-ERROR )
      *>          MOVE WS-SQL-CURRENT-VALUE TO your-field-here
               CONTINUE
               COBOL::"NEXT-ROW" ( WS-Db-Local-HANDLE WS-SQL-MORE )
           END-PERFORM.

       Db-Local-CLOSE.
      *>  Close the SQLite connection for Db-Local.
           COBOL::"CLOSE-DB" ( WS-Db-Local-HANDLE ).

       Db-Local-ON-CONNECT.
      *>  TODO: add your Db-Local-ON-CONNECT logic here.
           CONTINUE.

       Db-Local-ON-QUERY-DONE.
      *>  TODO: add your Db-Local-ON-QUERY-DONE logic here.
           CONTINUE.

       Db-Local-ON-ERROR.
      *>  TODO: add your Db-Local-ON-ERROR logic here.
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQLDATABASE-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "ready - driver " Db-Local::Driver ", " Db-Local::Mode " mode" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM SQLDATABASE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SQLDATABASE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM SQLDATABASE-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DB-LOCAL--ONCONNECTOK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "connected: " Db-Local::ConnectionString INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM DB-LOCAL--ONCONNECTOK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DB-LOCAL--ONCONNECTERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "connect failed: " Db-Local::LastError INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM DB-LOCAL--ONCONNECTERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DB-LOCAL--ONQUERYERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "query failed: " Db-Local::LastError INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM DB-LOCAL--ONQUERYERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-OPEN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> An in-memory SQLite database needs no server and no file.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Db-Local::Open(Db-Local::ConnectionString).
      *> EN: EXTENSION - fenced block literal: multi-line text taken verbatim, quotes never doubled.
      *> PT: EXTENSAO - literal em bloco cercado: texto multilinha literal, aspas sem duplicar.
      *> ES: EXTENSION - literal de bloque cercado: texto multilinea literal, comillas sin duplicar.
      *> FR: EXTENSION - litteral de bloc cloture: texte multiligne verbatim, guillemets non doubles.
      *> JP: EXTENSION - フェンス付きブロック リテラル。複数行をそのまま取り、引用符の二重化は不要。
      *> CN: EXTENSION - 围栏块字面量：逐字保留多行文本，引号无需重复书写。
           Db-Local::Execute(
```sql
CREATE TABLE IF NOT EXISTS city (id INTEGER PRIMARY KEY, name TEXT NOT NULL)
```
           ).
      *> EN: EXTENSION - fenced block literal: multi-line text taken verbatim, quotes never doubled.
      *> PT: EXTENSAO - literal em bloco cercado: texto multilinha literal, aspas sem duplicar.
      *> ES: EXTENSION - literal de bloque cercado: texto multilinea literal, comillas sin duplicar.
      *> FR: EXTENSION - litteral de bloc cloture: texte multiligne verbatim, guillemets non doubles.
      *> JP: EXTENSION - フェンス付きブロック リテラル。複数行をそのまま取り、引用符の二重化は不要。
      *> CN: EXTENSION - 围栏块字面量：逐字保留多行文本，引号无需重复书写。
           Db-Local::Execute(
```sql
INSERT INTO city (name) VALUES ('Rio de Janeiro'), ('Sao Paulo'), ('Recife'), ('Porto Alegre')
```
           ).
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "table created and seeded" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-OPEN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-QUERY--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-ROW  PIC X(200).
       01 WS-N    PIC S9(9) COMP-5.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Lst-Rows::Clear().
      *> Query returns the row count; Fetch walks the result set one row at a time.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           MOVE Db-Local::Query(Txt-Sql::Text) TO WS-N.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING WS-N " rows" INTO WS-LINE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-LINE TO Lbl-Count::Caption.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           PERFORM UNTIL 1 = 2
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
               MOVE Db-Local::Fetch() TO WS-ROW
               IF WS-ROW = SPACES
                   EXIT PERFORM
               END-IF
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
               Lst-Rows::AddItem(FUNCTION TRIM(WS-ROW))
           END-PERFORM.

           GOBACK.

       END PROGRAM BTN-QUERY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-TX--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> SQL transactions are STATEMENTS here - COBOL's COMMIT verb is for INDEXED files.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Db-Local::Execute("BEGIN").
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Db-Local::Execute("INSERT INTO city (name) VALUES ('Salvador')").
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Db-Local::Execute("COMMIT").
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "one row inserted inside a transaction" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-TX--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Db-Local::Close().

           GOBACK.

       END PROGRAM BTN-CLOSE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLEAR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::Clear().

           GOBACK.

       END PROGRAM BTN-CLEAR--ONCLICK.

       END PROGRAM SQLDATABASE-FORM.

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
       PROGRAM-ID. WEBSEARCH-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'WEBSEARCH-FORM'.

      *>── REST / HTTP runtime variables ──────────────────────────────
      *>   Usage:
      *>     MOVE 'https://api.example.com/resource' TO WS-REQUEST-URL
      *>     PERFORM RST1-GET
      *>     IF WS-HTTP-STATUS = 200
      *>         DISPLAY WS-HTTP-RESPONSE
      *>     END-IF
       01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.
       01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.
       01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.
       01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.
       01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.
       01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.
       01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.
       01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.

      *>── Web Search: Web-Find ──────────────────────────────────
       01 WS-Web-Find-SEARCH-ENGINE-ID PIC X(64)  VALUE ''.
       01 WS-Web-Find-QUERY            PIC X(512) VALUE SPACES.
       01 WS-Web-Find-NUM-RESULTS      PIC 9(2)   VALUE 5.
       01 WS-Web-Find-SAFE-SEARCH      PIC X(6)   VALUE 'Off'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'WebSearch'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(257) VALUE 'WebSearch runs a Google Custom Search query. It is Async by default: the call returns at once and the answers arrive as onResultsReceived, where GetResult(n) reads the n-th hit as title, snippet and link separated by tabs. It needs a SearchEngineId to work.'.
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

       01 WS-Web-Find.
          05 WS-Web-Find-TEXT       PIC X(256) VALUE 'Web-Find'.
          05 WS-Web-Find-VISIBLE    PIC 9      VALUE 1.
          05 WS-Web-Find-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Q-Cap.
          05 WS-Lbl-Q-Cap-TEXT       PIC X(256) VALUE 'Query'.
          05 WS-Lbl-Q-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Q-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Query.
          05 WS-Txt-Query-TEXT       PIC X(256) VALUE 'PowerRustCOBOL indexed files'.
          05 WS-Txt-Query-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Query-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Query-VALUE      PIC X(256) VALUE SPACES.

       01 WS-Lbl-Hits-Cap.
          05 WS-Lbl-Hits-Cap-TEXT       PIC X(256) VALUE 'Hits'.
          05 WS-Lbl-Hits-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Hits-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Lst-Hits.
          05 WS-Lst-Hits-TEXT       PIC X(256) VALUE 'Lst-Hits'.
          05 WS-Lst-Hits-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Hits-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Hits-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Note.
          05 WS-Lbl-Note-TEXT       PIC X(256) VALUE 'Set SearchEngineId in the properties pane before running this. Without one the control answers through onError rather than pretending to have searched.'.
          05 WS-Lbl-Note-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Note-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Search.
          05 WS-Btn-Search-TEXT       PIC X(256) VALUE 'Search'.
          05 WS-Btn-Search-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Search-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-More.
          05 WS-Btn-More-TEXT       PIC X(256) VALUE 'Ask for ten'.
          05 WS-Btn-More-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-More-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Safe.
          05 WS-Btn-Safe-TEXT       PIC X(256) VALUE 'Safe search on'.
          05 WS-Btn-Safe-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Safe-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       01 WS-Snackbar-1.
          05 WS-Snackbar-1-TEXT       PIC X(256) VALUE 'Snackbar-1'.
          05 WS-Snackbar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Snackbar-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "WEBSEARCH-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "WEBSEARCH-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Web-Find"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResultsReceived"
                               CALL "WEB-FIND--ONRESULTSRECEIVED"
                           WHEN "onError"
                               CALL "WEB-FIND--ONERROR"
                           WHEN "onTimeout"
                               CALL "WEB-FIND--ONTIMEOUT"
                       END-EVALUATE
                   WHEN "Btn-Search"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SEARCH--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-More"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-MORE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Safe"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAFE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
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
       Web-Find-SEARCH.
      *>    Google Custom Search via Web-Find — MOVE your query text to WS-Web-Find-QUERY
      *>    before calling. This paragraph does a plain, unencoded STRING
      *>    concatenation (no key, no percent-encoding — a multi-word query
      *>    truncates at its first space), and it is GOOGLE ONLY: it does
      *>    NOT follow the control's Provider property, because two of the
      *>    providers need a POST with an authentication header, which a
      *>    COBOL-HTTP-GET cannot send. For a correct, credential-aware
      *>    search on ANY provider use INVOKE Web-Find 'SEARCH' instead.
           MOVE SPACES TO WS-REQUEST-URL
           STRING 'https://www.googleapis.com/customsearch/v1?cx='
                  WS-Web-Find-SEARCH-ENGINE-ID DELIMITED BY SPACE
                  '&q=' DELIMITED BY SIZE
                  WS-Web-Find-QUERY DELIMITED BY SPACE
                  '&num=' DELIMITED BY SIZE
                  WS-Web-Find-NUM-RESULTS DELIMITED BY SIZE
                  '&safe=' DELIMITED BY SIZE
                  WS-Web-Find-SAFE-SEARCH DELIMITED BY SPACE
               INTO WS-REQUEST-URL
           END-STRING
           COBOL::"HTTP-GET" ( WS-REQUEST-URL
                 WS-HTTP-RESPONSE
                 WS-HTTP-STATUS )
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM Web-Find-ON-RESULTS
               WHEN OTHER
                   PERFORM Web-Find-ON-ERROR
           END-EVALUATE.

       Web-Find-ON-RESULTS.
      *>    TODO: Web-Find results handler — WS-HTTP-RESPONSE contains the raw JSON body
           CONTINUE.

       Web-Find-ON-ERROR.
      *>    TODO: Web-Find error handler — WS-HTTP-STATUS contains the error code (0 = network failure)
           CONTINUE.

      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEBSEARCH-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           IF Web-Find::SearchEngineId = SPACES
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
               STRING "no SearchEngineId set - searches will report an error" INTO WS-LINE
           ELSE
               STRING "ready - " Web-Find::NumResults " results per search" INTO WS-LINE
           END-IF.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM WEBSEARCH-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEBSEARCH-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM WEBSEARCH-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEB-FIND--ONRESULTSRECEIVED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-I    PIC S9(4) COMP-5.
       01 WS-HIT  PIC X(400).
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
           Lst-Hits::Clear().
      *> GetResult(n) is 1-based and returns title, snippet and link, tab separated.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > Web-Find::NumResults
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
               MOVE Web-Find::GetResult(WS-I) TO WS-HIT
               IF WS-HIT = SPACES
                   EXIT PERFORM
               END-IF
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
               Lst-Hits::AddItem(FUNCTION TRIM(WS-HIT))
           END-PERFORM.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
      *> No hits: the list was already emptied by Clear() above, so the only
      *> thing left to do is say so. Info, not Error - an empty result set is a
      *> normal answer, not a failure.
           IF Web-Find::ResultCount() = 0
               MOVE Web-Find::Query TO WS-HIT
               MOVE SPACES TO WS-LINE
               STRING "No results for " DELIMITED BY SIZE
                      FUNCTION TRIM(WS-HIT) DELIMITED BY SIZE
                   INTO WS-LINE
               END-STRING
               SET Snackbar-1::Text TO FUNCTION TRIM(WS-LINE)
               SET Snackbar-1::Category TO "Info"
               INVOKE SNACKBAR-1::Show()
           END-IF.
           STRING "results in" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM WEB-FIND--ONRESULTSRECEIVED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEB-FIND--ONERROR IS COMMON PROGRAM.

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
           STRING "search failed - is SearchEngineId set? " Web-Find::LastError INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM WEB-FIND--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. WEB-FIND--ONTIMEOUT IS COMMON PROGRAM.

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
           STRING "timed out after " Web-Find::TimeoutMs " ms" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM WEB-FIND--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SEARCH--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE Txt-Query::Text TO Web-Find::Query.
      *> Search is reached through the INVOKE form: see the note in the demo report.
      *> EN: EXTENSION - the INVOKE form of the same call; required where the inline form is not parsed.
      *> PT: EXTENSAO - a forma INVOKE da mesma chamada; exigida onde a forma inline nao e reconhecida.
      *> ES: EXTENSION - la forma INVOKE de la misma llamada; exigida donde la forma en linea no se analiza.
      *> FR: EXTENSION - la forme INVOKE du meme appel; requise la ou la forme en ligne n'est pas analysee.
      *> JP: EXTENSION - 同じ呼び出しの INVOKE 形式。インライン形式が解析されない場合に必要。
      *> CN: EXTENSION - 同一调用的 INVOKE 写法；内联写法尚未被解析时必须使用。
           INVOKE Web-Find "Search" USING Web-Find::Query.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "searching for " Web-Find::Query INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-SEARCH--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-MORE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE 10 TO Web-Find::NumResults

           INVOKE Web-Find "Search" USING Web-Find::Query.

           GOBACK.

       END PROGRAM BTN-MORE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAFE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "High" TO Web-Find::SafeSearch.

           GOBACK.

       END PROGRAM BTN-SAFE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> Cancel also needs the INVOKE form - it is not an inline-callable name yet.
      *> EN: EXTENSION - the INVOKE form of the same call; required where the inline form is not parsed.
      *> PT: EXTENSAO - a forma INVOKE da mesma chamada; exigida onde a forma inline nao e reconhecida.
      *> ES: EXTENSION - la forma INVOKE de la misma llamada; exigida donde la forma en linea no se analiza.
      *> FR: EXTENSION - la forme INVOKE du meme appel; requise la ou la forme en ligne n'est pas analysee.
      *> JP: EXTENSION - 同じ呼び出しの INVOKE 形式。インライン形式が解析されない場合に必要。
      *> CN: EXTENSION - 同一调用的 INVOKE 写法；内联写法尚未被解析时必须使用。
           INVOKE Web-Find "Cancel"
           INVOKE Lst-Hits::Clear().

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

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

       END PROGRAM WEBSEARCH-FORM.

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
       PROGRAM-ID. INDEXEDFILE-FORM.

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
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT MENU-FILE ASSIGN TO "BurguerTime/data/menu.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS NUMERO
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.

       DATA DIVISION.
       FILE SECTION.
       FD  MENU-FILE IS GLOBAL
           RECORD CONTAINS 424 CHARACTERS.
           01 MENU-RECORD.
               05 NUMERO PIC 99.
               05 TITULO PIC X(50).
               05 DESCRICAO PIC X(100).
               05 CAMINHO-FOTO PIC X(255).
               05 PRECO PIC 9(5)V99.
               05 UNIDADE PIC X(10).
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'INDEXEDFILE-FORM'.

      *>── IndexedFile control: Idx-Menu ─────────────────────────────
      *>   Project indexed file: indexed/BurguerTime/menu.cidx
       01 WS-Idx-Menu-OPEN-MODE      PIC X(8)    VALUE 'I-O'.
       01 WS-Idx-Menu-LOAD-STRATEGY  PIC X(8)    VALUE 'Disk'.
       01 WS-Idx-Menu-IS-OPEN        PIC 9       VALUE 0.
       01 WS-Idx-Menu-AT-END         PIC 9       VALUE 0.
       01 WS-Idx-Menu-HAS-RECORD     PIC 9       VALUE 0.
       01 WS-Idx-Menu-CURRENT-OP     PIC X(16)   VALUE SPACES.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS GLOBAL PIC XX VALUE "00".

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'IndexedFile'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(311) VALUE 'The IndexedFile control is the designer-side FACE of a keyed file. The record and its keys are described once in a .cidx definition, which the SELECT and FD are generated from; the file itself is then driven with ordinary COBOL verbs - OPEN, READ, START, WRITE, REWRITE, DELETE, COMMIT, CLOSE - against that FD.'.
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

       01 WS-Idx-Menu.
          05 WS-Idx-Menu-TEXT       PIC X(256) VALUE 'Idx-Menu'.
          05 WS-Idx-Menu-VISIBLE    PIC 9      VALUE 1.
          05 WS-Idx-Menu-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Tray.
          05 WS-Lbl-Tray-TEXT       PIC X(256) VALUE 'Idx-Menu  MENU-FILE'.
          05 WS-Lbl-Tray-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Tray-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Key-Cap.
          05 WS-Lbl-Key-Cap-TEXT       PIC X(256) VALUE 'Key (NUMERO)'.
          05 WS-Lbl-Key-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Key-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Num-Key.
          05 WS-Num-Key-TEXT       PIC X(256) VALUE 'Num-Key'.
          05 WS-Num-Key-VISIBLE    PIC 9      VALUE 1.
          05 WS-Num-Key-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Title-Cap.
          05 WS-Lbl-Title-Cap-TEXT       PIC X(256) VALUE 'TITULO'.
          05 WS-Lbl-Title-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Title.
          05 WS-Txt-Title-TEXT       PIC X(50) VALUE 'Txt-Title'.
          05 WS-Txt-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Title-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Title-VALUE      PIC X(50) VALUE SPACES.

       01 WS-Lbl-Price-Cap.
          05 WS-Lbl-Price-Cap-TEXT       PIC X(256) VALUE 'PRECO'.
          05 WS-Lbl-Price-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Price-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Price.
          05 WS-Txt-Price-TEXT       PIC 9(5)V99 VALUE 0.
          05 WS-Txt-Price-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Price-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Price-VALUE      PIC 9(5)V99 VALUE 0.

       01 WS-Lbl-Fs-Cap.
          05 WS-Lbl-Fs-Cap-TEXT       PIC X(256) VALUE 'Last FILE STATUS'.
          05 WS-Lbl-Fs-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Fs-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Fs.
          05 WS-Lbl-Fs-TEXT       PIC X(256) VALUE '--'.
          05 WS-Lbl-Fs-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Fs-ENABLED    PIC 9      VALUE 1.

       01 WS-Lst-Scan.
          05 WS-Lst-Scan-TEXT       PIC X(256) VALUE 'Lst-Scan'.
          05 WS-Lst-Scan-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lst-Scan-ENABLED    PIC 9      VALUE 1.
          05 WS-Lst-Scan-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Btn-Open.
          05 WS-Btn-Open-TEXT       PIC X(256) VALUE 'OPEN I-O'.
          05 WS-Btn-Open-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Open-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Read.
          05 WS-Btn-Read-TEXT       PIC X(256) VALUE 'READ by key'.
          05 WS-Btn-Read-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Read-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Write.
          05 WS-Btn-Write-TEXT       PIC X(256) VALUE 'WRITE / REWRITE'.
          05 WS-Btn-Write-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Write-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Scan.
          05 WS-Btn-Scan-TEXT       PIC X(256) VALUE 'START + scan'.
          05 WS-Btn-Scan-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Scan-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Close.
          05 WS-Btn-Close-TEXT       PIC X(256) VALUE 'COMMIT + CLOSE'.
          05 WS-Btn-Close-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Close-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "INDEXEDFILE-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "INDEXEDFILE-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Open"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-OPEN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Read"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-READ--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Write"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-WRITE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Scan"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SCAN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Close"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLOSE--ONCLICK"
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
       Idx-Menu-OPEN.
      *>  Opens indexed file MENU for I-O.
           IF WS-Idx-Menu-IS-OPEN = 0
               OPEN I-O MENU REGISTERED USER 'operator'
               COBOL::"FILE-STATUS" ( "MENU" WS-FS )
               IF WS-FS(1:1) = '0'
                   MOVE 1 TO WS-Idx-Menu-IS-OPEN
               END-IF
               MOVE 0 TO WS-Idx-Menu-AT-END
               MOVE 0 TO WS-Idx-Menu-HAS-RECORD
           END-IF.

       Idx-Menu-START.
      *>  Set NUMERO, then PERFORM Idx-Menu-START to position the current pointer.
           START MENU KEY IS GREATER THAN OR EQUAL TO NUMERO
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-AT-END
           END-START.

       Idx-Menu-READ-NEXT.
           READ MENU NEXT
               AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 1 TO WS-Idx-Menu-AT-END
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-AT-END
                   MOVE 1 TO WS-Idx-Menu-HAS-RECORD
           END-READ.

       Idx-Menu-READ-PREVIOUS.
           READ MENU PREVIOUS
               AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 1 TO WS-Idx-Menu-AT-END
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-AT-END
                   MOVE 1 TO WS-Idx-Menu-HAS-RECORD
           END-READ.

       Idx-Menu-READ-FIRST.
      *>  Set NUMERO to the lowest desired value, position, then read NEXT.
           START MENU KEY IS GREATER THAN OR EQUAL TO NUMERO
               INVALID KEY CONTINUE
           END-START
           READ MENU NEXT
               AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 1 TO WS-Idx-Menu-AT-END
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-AT-END
                   MOVE 1 TO WS-Idx-Menu-HAS-RECORD
           END-READ.

       Idx-Menu-READ-LAST.
      *>  Set NUMERO to the highest desired value, position, then read PREVIOUS.
           START MENU KEY IS LESS THAN OR EQUAL TO NUMERO
               INVALID KEY CONTINUE
           END-START
           READ MENU PREVIOUS
               AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 1 TO WS-Idx-Menu-AT-END
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT AT END
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-AT-END
                   MOVE 1 TO WS-Idx-Menu-HAS-RECORD
           END-READ.

       Idx-Menu-READ-INVALID.
      *>  Direct keyed read. Set NUMERO before calling this paragraph.
           READ MENU
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 0 TO WS-Idx-Menu-HAS-RECORD
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
                   MOVE 1 TO WS-Idx-Menu-HAS-RECORD
           END-READ.

       Idx-Menu-WRITE.
      *>  Requires MENU opened I-O. Data comes from bound/set record fields.
           WRITE MENU-RECORD
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
           END-WRITE.

       Idx-Menu-REWRITE.
      *>  Requires MENU opened I-O. Data comes from bound/set record fields.
           REWRITE MENU-RECORD
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
           END-REWRITE.

       Idx-Menu-DELETE.
      *>  Requires MENU opened I-O. Data comes from bound/set record fields.
           DELETE MENU
               INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
               NOT INVALID KEY
                   COBOL::"FILE-STATUS" ( "MENU" WS-FS )
           END-DELETE.

       Idx-Menu-COMMIT.
      *>  Flushes pending indexed-file changes for MENU.
           CLOSE MENU
           OPEN I-O MENU
           COBOL::"FILE-STATUS" ( "MENU" WS-FS ).

       Idx-Menu-ROLLBACK.
      *>  Transaction rollback is storage-engine dependent; reopen to discard pending cursor state.
           CLOSE MENU
           OPEN I-O MENU
           COBOL::"FILE-STATUS" ( "MENU" WS-FS ).

       Idx-Menu-CLOSE.
      *>  No-op when already closed. I-O close commits automatically.
           IF WS-Idx-Menu-IS-OPEN = 1
               PERFORM Idx-Menu-COMMIT
               CLOSE MENU
               MOVE 0 TO WS-Idx-Menu-IS-OPEN
               COBOL::"FILE-STATUS" ( "MENU" WS-FS )
           END-IF.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INDEXEDFILE-FORM--ONLOAD IS COMMON PROGRAM.

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
           STRING "ready - " Idx-Menu::IndexedFile ", key " Idx-Menu::KeyName
                  ", open " Idx-Menu::OpenMode INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM INDEXEDFILE-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INDEXEDFILE-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM INDEXEDFILE-FORM--ONCLOSE.

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
      *> The OPEN is standard COBOL; WITH REGISTERED names the operator in the log.
      *> EN: EXTENSION - the INVOKE form of the same call; required where the inline form is not parsed.
      *> PT: EXTENSAO - a forma INVOKE da mesma chamada; exigida onde a forma inline nao e reconhecida.
      *> ES: EXTENSION - la forma INVOKE de la misma llamada; exigida donde la forma en linea no se analiza.
      *> FR: EXTENSION - la forme INVOKE du meme appel; requise la ou la forme en ligne n'est pas analysee.
      *> JP: EXTENSION - 同じ呼び出しの INVOKE 形式。インライン形式が解析されない場合に必要。
      *> CN: EXTENSION - 同一调用的 INVOKE 写法；内联写法尚未被解析时必须使用。
           OPEN I-O MENU-FILE WITH REGISTERED USER "operator".
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-FS TO Lbl-Fs::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "OPEN I-O -> status " WS-FS INTO WS-LINE.
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
       PROGRAM-ID. BTN-READ--ONCLICK IS COMMON PROGRAM.

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
           MOVE Num-Key::Value TO NUMERO.
           READ MENU-FILE
               INVALID KEY
                   MOVE SPACES TO TITULO
           END-READ.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-FS TO Lbl-Fs::Caption.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE TITULO TO Txt-Title::Text.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE PRECO  TO Txt-Price::Text.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "READ " NUMERO " -> " WS-FS "  " TITULO INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-READ--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-WRITE--ONCLICK IS COMMON PROGRAM.

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
           MOVE Num-Key::Value  TO NUMERO.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           MOVE Txt-Title::Text TO TITULO.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           MOVE Txt-Price::Text TO PRECO.
      *> Write it if it is new; rewrite it if the key is already there.
           WRITE MENU-RECORD
               INVALID KEY
                   REWRITE MENU-RECORD
           END-WRITE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-FS TO Lbl-Fs::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "WRITE " NUMERO " -> status " WS-FS INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-WRITE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SCAN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
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
           Lst-Scan::Clear().
      *> START positions the cursor; READ NEXT walks forward from there.
           MOVE 0 TO NUMERO.
           START MENU-FILE KEY IS NOT LESS THAN NUMERO
               INVALID KEY
                   MOVE "23" TO WS-FS
           END-START.
           PERFORM UNTIL WS-FS NOT = "00"
               READ MENU-FILE NEXT
                   AT END
                       EXIT PERFORM
               END-READ
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
               STRING NUMERO "  " TITULO "  " PRECO INTO WS-LINE
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
               Lst-Scan::AddItem(FUNCTION TRIM(WS-LINE))
           END-PERFORM.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-FS TO Lbl-Fs::Caption.

           GOBACK.

       END PROGRAM BTN-SCAN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CLOSE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> COMMIT here is the INDEXED-file transaction verb, not SQL.
           COMMIT.
           CLOSE MENU-FILE.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-FS TO Lbl-Fs::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "COMMIT + CLOSE -> status " WS-FS INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-CLOSE--ONCLICK.

       END PROGRAM INDEXEDFILE-FORM.

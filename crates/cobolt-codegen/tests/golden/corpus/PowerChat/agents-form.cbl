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
       PROGRAM-ID. AGENTS-FORM.

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
           SELECT SETTINGS-FILE ASSIGN TO WS-SETTINGS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS SET-NAME
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.
           SELECT MODELS-FILE ASSIGN TO WS-MODELS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS MDL-NAME
               FILE STATUS IS WS-FS
               STORAGE MODE IS DISK.

       DATA DIVISION.
       FILE SECTION.
       FD  SETTINGS-FILE IS GLOBAL.
       01  SETTINGS-REC.
           05 SET-NAME          PIC X(20).
           05 SET-VALUE         PIC X(200).
       FD  MODELS-FILE IS GLOBAL.
       01  MODEL-REC.
           05 MDL-NAME          PIC X(30).
           05 MDL-API           PIC X(12).
           05 MDL-URL           PIC X(200).
           05 MDL-MODEL         PIC X(80).
      *>   The capability table (spec 071 R31, 063 R72): whether the model
      *>   can call tools, and how well it orchestrates, 1 (poorly) to 9.
           05 MDL-TOOLS         PIC X.
           05 MDL-RANK          PIC 9.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'AGENTS-FORM'.

      *>── User Working Storage ────────────────────────────────────────
       01 WS-FS              GLOBAL PIC XX VALUE "00".
       01 WS-DATA-DIR        GLOBAL PIC X(200).
       01 WS-SET-NAME        GLOBAL PIC X(20).
       01 WS-SET-VALUE       GLOBAL PIC X(200).
       01 WS-EOF             GLOBAL PIC X.
       01 WS-OK              GLOBAL PIC X(4).
       01 WS-SETTINGS-PATH   GLOBAL PIC X(240).
       01 WS-MODELS-PATH     GLOBAL PIC X(240).
      *>   The interface texts (spec 071 R44): one row per text, one column
      *>   per language - en, pt, es, fr, jp, cn. Identifiers stay English;
      *>   only the values are translated.
       01 WS-LANG            GLOBAL PIC XX VALUE "en".
       01 WS-LANG-IX         GLOBAL PIC 9 VALUE 1.
       01 WS-TX-I            GLOBAL PIC 9(4).
       01 PC-TEXT-DATA       GLOBAL.
      *>   AGENTS-TITLE
          05 FILLER PIC X(120) VALUE "Agents".
          05 FILLER PIC X(120) VALUE "Agentes".
          05 FILLER PIC X(120) VALUE "Agentes".
          05 FILLER PIC X(120) VALUE "Agents".
          05 FILLER PIC X(120) VALUE "エージェント".
          05 FILLER PIC X(120) VALUE "代理".
      *>   AGENT-N
          05 FILLER PIC X(120) VALUE "Agent &1".
          05 FILLER PIC X(120) VALUE "Agente &1".
          05 FILLER PIC X(120) VALUE "Agente &1".
          05 FILLER PIC X(120) VALUE "Agent &1".
          05 FILLER PIC X(120) VALUE "エージェント &1".
          05 FILLER PIC X(120) VALUE "代理 &1".
      *>   AGENT-NONE
          05 FILLER PIC X(120) VALUE "(off)".
          05 FILLER PIC X(120) VALUE "(desligado)".
          05 FILLER PIC X(120) VALUE "(apagado)".
          05 FILLER PIC X(120) VALUE "(désactivé)".
          05 FILLER PIC X(120) VALUE "（停止）".
          05 FILLER PIC X(120) VALUE "（停用）".
      *>   AGENTS-SAVED
          05 FILLER PIC X(120) VALUE "Agents saved.".
          05 FILLER PIC X(120) VALUE "Agentes salvos.".
          05 FILLER PIC X(120) VALUE "Agentes guardados.".
          05 FILLER PIC X(120) VALUE "Agents enregistrés.".
          05 FILLER PIC X(120) VALUE "エージェントを保存しました。".
          05 FILLER PIC X(120) VALUE "代理已保存。".
      *>   SAVE
          05 FILLER PIC X(120) VALUE "Save".
          05 FILLER PIC X(120) VALUE "Salvar".
          05 FILLER PIC X(120) VALUE "Guardar".
          05 FILLER PIC X(120) VALUE "Enregistrer".
          05 FILLER PIC X(120) VALUE "保存".
          05 FILLER PIC X(120) VALUE "保存".
      *>   CANCEL
          05 FILLER PIC X(120) VALUE "Cancel".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Cancelar".
          05 FILLER PIC X(120) VALUE "Annuler".
          05 FILLER PIC X(120) VALUE "キャンセル".
          05 FILLER PIC X(120) VALUE "取消".
      *>   STATUS
          05 FILLER PIC X(120) VALUE "Status".
          05 FILLER PIC X(120) VALUE "Status".
          05 FILLER PIC X(120) VALUE "Estado".
          05 FILLER PIC X(120) VALUE "État".
          05 FILLER PIC X(120) VALUE "ステータス".
          05 FILLER PIC X(120) VALUE "状态".
      *>   TIP-AGENT-CONN
          05 FILLER PIC X(120) VALUE "The connection whose model this agent uses, or (off).".
          05 FILLER PIC X(120) VALUE "A conexão cujo modelo este agente usa, ou (desligado).".
          05 FILLER PIC X(120) VALUE "La conexión cuyo modelo usa este agente, o (apagado).".
          05 FILLER PIC X(120) VALUE "La connexion dont cet agent utilise le modèle, ou (désactivé).".
          05 FILLER PIC X(120) VALUE "このエージェントが使う接続、または（停止）。".
          05 FILLER PIC X(120) VALUE "此代理使用其模型的连接，或（停用）。".
       01 PC-TEXT-TABLE REDEFINES PC-TEXT-DATA GLOBAL.
          05 PC-TEXT-ROW     OCCURS 8.
             10 PC-TEXT      PIC X(120) OCCURS 6.
      *>   The texts in the current language, by name.
       01 PC-TEXTS-NOW       GLOBAL.
          05 T-AGENTS-TITLE PIC X(120).
          05 T-AGENT-N PIC X(120).
          05 T-AGENT-NONE PIC X(120).
          05 T-AGENTS-SAVED PIC X(120).
          05 T-SAVE PIC X(120).
          05 T-CANCEL PIC X(120).
          05 T-STATUS PIC X(120).
          05 T-TIP-AGENT-CONN PIC X(120).
       01 PC-TEXTS-NOW-R REDEFINES PC-TEXTS-NOW GLOBAL.
          05 PC-TEXT-NOW     PIC X(120) OCCURS 8.
      *>   PC-FMT: WS-FMT with &1..&4 replaced by WS-ARG1..4, into WS-FMT-OUT.
       01 WS-FMT             GLOBAL PIC X(120).
       01 WS-ARG1            GLOBAL PIC X(300).
       01 WS-ARG2            GLOBAL PIC X(300).
       01 WS-ARG3            GLOBAL PIC X(300).
       01 WS-ARG4            GLOBAL PIC X(300).
       01 WS-FMT-OUT         GLOBAL PIC X(1200).
      *>   The saved connections, in name order.
       01 WS-CONN-COUNT      GLOBAL PIC 9(4) VALUE 0.
       01 WS-CONN-NAMES      GLOBAL.
          05 WS-CONN-NAME    PIC X(30) OCCURS 200.
       01 WS-INDEX           GLOBAL PIC S9(4).
       01 WS-P               GLOBAL PIC 9(3).
       01 WS-A               GLOBAL PIC 9.
       01 WS-AGENT-ENTRIES   GLOBAL.
          05 WS-AGENT-ENTRY  PIC X(30) OCCURS 3.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'Agents'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Agent-1.
          05 WS-Lbl-Agent-1-TEXT       PIC X(256) VALUE 'Lbl-Agent-1'.
          05 WS-Lbl-Agent-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Agent-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Agent-1.
          05 WS-Cmb-Agent-1-TEXT       PIC X(256) VALUE 'Cmb-Agent-1'.
          05 WS-Cmb-Agent-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-1-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Agent-2.
          05 WS-Lbl-Agent-2-TEXT       PIC X(256) VALUE 'Lbl-Agent-2'.
          05 WS-Lbl-Agent-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Agent-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Agent-2.
          05 WS-Cmb-Agent-2-TEXT       PIC X(256) VALUE 'Cmb-Agent-2'.
          05 WS-Cmb-Agent-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-2-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-2-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Agent-3.
          05 WS-Lbl-Agent-3-TEXT       PIC X(256) VALUE 'Lbl-Agent-3'.
          05 WS-Lbl-Agent-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Agent-3-ENABLED    PIC 9      VALUE 1.

       01 WS-Cmb-Agent-3.
          05 WS-Cmb-Agent-3-TEXT       PIC X(256) VALUE 'Cmb-Agent-3'.
          05 WS-Cmb-Agent-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-3-ENABLED    PIC 9      VALUE 1.
          05 WS-Cmb-Agent-3-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Lbl-Status.
          05 WS-Lbl-Status-TEXT       PIC X(256) VALUE 'Status'.
          05 WS-Lbl-Status-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Status-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cancel.
          05 WS-Btn-Cancel-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-Cancel-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cancel-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Save.
          05 WS-Btn-Save-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-Save-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Save-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-CancelTop.
          05 WS-Btn-CancelTop-TEXT       PIC X(256) VALUE 'Cancel'.
          05 WS-Btn-CancelTop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-CancelTop-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-SaveTop.
          05 WS-Btn-SaveTop-TEXT       PIC X(256) VALUE 'Save'.
          05 WS-Btn-SaveTop-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-SaveTop-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "AGENTS-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "AGENTS-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Btn-Cancel"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Save"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVE--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-CancelTop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CANCELTOP--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-SaveTop"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-SAVETOP--ONCLICK"
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
       PROGRAM-ID. AGENTS-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-PATHS"
           CALL "PC-TEXTS"
           CALL "PC-LOAD-CONNS"
           CALL "PC-LOAD-AGENTS"
      *>   Each agent's picker: "(off)" first, then every saved connection;
      *>   the one the agent uses now is selected.
           MOVE Cmb-Agent-1::Clear() TO WS-OK
           MOVE Cmb-Agent-1::AddItem(FUNCTION TRIM(T-AGENT-NONE)) TO WS-OK
           MOVE 0 TO WS-INDEX
           PERFORM VARYING WS-P FROM 1 BY 1 UNTIL WS-P > WS-CONN-COUNT
               MOVE Cmb-Agent-1::AddItem(FUNCTION TRIM(WS-CONN-NAME(WS-P))) TO WS-OK
               IF WS-CONN-NAME(WS-P) = WS-AGENT-ENTRY(1)
                   MOVE WS-P TO WS-INDEX
               END-IF
           END-PERFORM
           MOVE WS-INDEX TO WS-P
           MOVE Cmb-Agent-1::SetSelectedIndex(WS-P) TO WS-OK
           MOVE Cmb-Agent-2::Clear() TO WS-OK
           MOVE Cmb-Agent-2::AddItem(FUNCTION TRIM(T-AGENT-NONE)) TO WS-OK
           MOVE 0 TO WS-INDEX
           PERFORM VARYING WS-P FROM 1 BY 1 UNTIL WS-P > WS-CONN-COUNT
               MOVE Cmb-Agent-2::AddItem(FUNCTION TRIM(WS-CONN-NAME(WS-P))) TO WS-OK
               IF WS-CONN-NAME(WS-P) = WS-AGENT-ENTRY(2)
                   MOVE WS-P TO WS-INDEX
               END-IF
           END-PERFORM
           MOVE WS-INDEX TO WS-P
           MOVE Cmb-Agent-2::SetSelectedIndex(WS-P) TO WS-OK
           MOVE Cmb-Agent-3::Clear() TO WS-OK
           MOVE Cmb-Agent-3::AddItem(FUNCTION TRIM(T-AGENT-NONE)) TO WS-OK
           MOVE 0 TO WS-INDEX
           PERFORM VARYING WS-P FROM 1 BY 1 UNTIL WS-P > WS-CONN-COUNT
               MOVE Cmb-Agent-3::AddItem(FUNCTION TRIM(WS-CONN-NAME(WS-P))) TO WS-OK
               IF WS-CONN-NAME(WS-P) = WS-AGENT-ENTRY(3)
                   MOVE WS-P TO WS-INDEX
               END-IF
           END-PERFORM
           MOVE WS-INDEX TO WS-P
           MOVE Cmb-Agent-3::SetSelectedIndex(WS-P) TO WS-OK

           GOBACK.

       END PROGRAM AGENTS-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENTS-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM AGENTS-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The same code as the Cancel button at the top.
           CALL "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVE--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The same code as the Save button at the top.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVE--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CANCELTOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-CANCEL"

           GOBACK.

       END PROGRAM BTN-CANCELTOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-SAVETOP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           CALL "PC-SAVE"

           GOBACK.

       END PROGRAM BTN-SAVETOP--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-PATHS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Where PowerChat keeps its own files: POWERCHAT_DATA, else "data".
           DISPLAY "POWERCHAT_DATA" UPON ENVIRONMENT-NAME
           ACCEPT WS-DATA-DIR FROM ENVIRONMENT-VALUE
           IF WS-DATA-DIR = SPACES
               MOVE "data" TO WS-DATA-DIR
           END-IF
           MOVE SPACES TO WS-SETTINGS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/settings.idx"
               DELIMITED BY SIZE INTO WS-SETTINGS-PATH
           MOVE SPACES TO WS-MODELS-PATH
           STRING FUNCTION TRIM(WS-DATA-DIR) "/models.idx"
               DELIMITED BY SIZE INTO WS-MODELS-PATH

           GOBACK.

       END PROGRAM PC-PATHS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SYNC-MODELS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SY-N          PIC 9(3).
       01 SY-I          PIC 9(3).
       01 SY-J          PIC 9(3).
       01 SY-NAME       PIC X(30).
       01 SY-API        PIC X(12).
       01 SY-URL        PIC X(200).
       01 SY-MODEL      PIC X(80).
       01 SY-FOUND      PIC X.
       01 SY-EOF        PIC X.
       01 SY-GONE-N     PIC 9(3).
       01 SY-GONE-TABLE.
          05 SY-GONE    PIC X(30) OCCURS 200 TIMES.
       PROCEDURE DIVISION.
      *>   The models are the APPLICATION's (spec 085): the runtime keeps the
      *>   list for every form of it, the application's own agents included.
      *>   MODELS mirrors that list and adds what only PowerChat knows of each
      *>   model - whether it calls tools, how well it orchestrates. Once, the
      *>   models PowerChat kept before are handed to the application's list.
           OPEN I-O MODELS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT MODELS-FILE
               CLOSE MODELS-FILE
               OPEN I-O MODELS-FILE
           END-IF
           MOVE "MODELS-ADOPTED" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           IF WS-SET-VALUE NOT = "Y"
               MOVE LOW-VALUES TO MDL-NAME
               MOVE "N" TO SY-EOF
               START MODELS-FILE KEY IS >= MDL-NAME
                   INVALID KEY MOVE "Y" TO SY-EOF
               END-START
               PERFORM UNTIL SY-EOF = "Y"
                   READ MODELS-FILE NEXT RECORD
                       AT END MOVE "Y" TO SY-EOF
                       NOT AT END
                           COBOL::"MODEL-SET" ( MDL-NAME MDL-API MDL-URL MDL-MODEL )
                   END-READ
               END-PERFORM
               MOVE "MODELS-ADOPTED" TO WS-SET-NAME
               MOVE "Y" TO WS-SET-VALUE
               CALL "PC-SETTING-PUT"
           END-IF
      *>   Every model of the application's is here, with its own metadata.
           COBOL::"MODEL-COUNT" ( SY-N )
           PERFORM VARYING SY-I FROM 1 BY 1 UNTIL SY-I > SY-N
               MOVE SPACES TO SY-NAME SY-API SY-URL SY-MODEL
               COBOL::"MODEL-GET" ( SY-I SY-NAME SY-API SY-URL SY-MODEL )
               MOVE SY-NAME TO MDL-NAME
               READ MODELS-FILE
                   INVALID KEY
                       MOVE SY-API TO MDL-API
                       MOVE SY-URL TO MDL-URL
                       MOVE SY-MODEL TO MDL-MODEL
                       MOVE "N" TO MDL-TOOLS
                       MOVE 5 TO MDL-RANK
                       WRITE MODEL-REC
                           INVALID KEY CONTINUE
                       END-WRITE
                   NOT INVALID KEY
                       IF MDL-API NOT = SY-API OR MDL-URL NOT = SY-URL
                          OR MDL-MODEL NOT = SY-MODEL
                           MOVE SY-API TO MDL-API
                           MOVE SY-URL TO MDL-URL
                           MOVE SY-MODEL TO MDL-MODEL
                           REWRITE MODEL-REC
                               INVALID KEY CONTINUE
                           END-REWRITE
                       END-IF
               END-READ
           END-PERFORM
      *>   ...and none the application has withdrawn.
           MOVE 0 TO SY-GONE-N
           MOVE LOW-VALUES TO MDL-NAME
           MOVE "N" TO SY-EOF
           START MODELS-FILE KEY IS >= MDL-NAME
               INVALID KEY MOVE "Y" TO SY-EOF
           END-START
           PERFORM UNTIL SY-EOF = "Y"
               READ MODELS-FILE NEXT RECORD
                   AT END MOVE "Y" TO SY-EOF
                   NOT AT END
                       MOVE "N" TO SY-FOUND
                       PERFORM VARYING SY-J FROM 1 BY 1
                               UNTIL SY-J > SY-N OR SY-FOUND = "Y"
                           MOVE SPACES TO SY-NAME
                           COBOL::"MODEL-GET" ( SY-J SY-NAME )
                           IF FUNCTION UPPER-CASE(SY-NAME)
                              = FUNCTION UPPER-CASE(MDL-NAME)
                               MOVE "Y" TO SY-FOUND
                           END-IF
                       END-PERFORM
                       IF SY-FOUND = "N" AND SY-GONE-N < 200
                           ADD 1 TO SY-GONE-N
                           MOVE MDL-NAME TO SY-GONE(SY-GONE-N)
                       END-IF
               END-READ
           END-PERFORM
           PERFORM VARYING SY-J FROM 1 BY 1 UNTIL SY-J > SY-GONE-N
               MOVE SY-GONE(SY-J) TO MDL-NAME
               DELETE MODELS-FILE
                   INVALID KEY CONTINUE
               END-DELETE
           END-PERFORM
           COMMIT
           CLOSE MODELS-FILE

           GOBACK.

       END PROGRAM PC-SYNC-MODELS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-GET IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-SET-NAME in, WS-SET-VALUE out (spaces when the setting is unset).
           MOVE SPACES TO WS-SET-VALUE
           OPEN I-O SETTINGS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT SETTINGS-FILE
               CLOSE SETTINGS-FILE
               OPEN I-O SETTINGS-FILE
           END-IF
           MOVE WS-SET-NAME TO SET-NAME
           READ SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE SET-VALUE TO WS-SET-VALUE
           END-READ
           CLOSE SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-GET.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SETTING-PUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   WS-SET-NAME, WS-SET-VALUE in. Written and committed at once (R10f).
           OPEN I-O SETTINGS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT SETTINGS-FILE
               CLOSE SETTINGS-FILE
               OPEN I-O SETTINGS-FILE
           END-IF
           MOVE WS-SET-NAME TO SET-NAME
           MOVE WS-SET-VALUE TO SET-VALUE
           WRITE SETTINGS-REC
               INVALID KEY REWRITE SETTINGS-REC
           END-WRITE
           COMMIT
           CLOSE SETTINGS-FILE

           GOBACK.

       END PROGRAM PC-SETTING-PUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LOAD-CONNS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Every saved connection's name, in key order, into WS-CONN-NAME -
      *>   the application's models, mirrored first (spec 085).
           CALL "PC-SYNC-MODELS"
           MOVE 0 TO WS-CONN-COUNT
           OPEN I-O MODELS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT MODELS-FILE
               CLOSE MODELS-FILE
               OPEN I-O MODELS-FILE
           END-IF
           MOVE LOW-VALUES TO MDL-NAME
           MOVE "N" TO WS-EOF
           START MODELS-FILE KEY IS >= MDL-NAME
               INVALID KEY MOVE "Y" TO WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ MODELS-FILE NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       IF WS-CONN-COUNT < 200
                           ADD 1 TO WS-CONN-COUNT
                           MOVE MDL-NAME TO WS-CONN-NAME(WS-CONN-COUNT)
                       END-IF
               END-READ
           END-PERFORM
           CLOSE MODELS-FILE

           GOBACK.

       END PROGRAM PC-LOAD-CONNS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-LOAD-AGENTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Which model each of the chat's three agents uses. Agent 1 falls
      *>   back to MODEL-ENTRY, the single setting PowerChat 1 kept.
           PERFORM VARYING WS-A FROM 1 BY 1 UNTIL WS-A > 3
               MOVE SPACES TO WS-SET-NAME
               STRING "AGENT-" WS-A "-ENTRY" DELIMITED BY SIZE INTO WS-SET-NAME
               CALL "PC-SETTING-GET"
               MOVE WS-SET-VALUE TO WS-AGENT-ENTRY(WS-A)
           END-PERFORM
           IF WS-AGENT-ENTRY(1) = SPACES
               MOVE "MODEL-ENTRY" TO WS-SET-NAME
               CALL "PC-SETTING-GET"
               MOVE WS-SET-VALUE TO WS-AGENT-ENTRY(1)
           END-IF

           GOBACK.

       END PROGRAM PC-LOAD-AGENTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-TEXTS IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   The language the user picked (LANG), the texts in it, and every
      *>   designed caption and hint that shows one (R46).
           MOVE "LANG" TO WS-SET-NAME
           CALL "PC-SETTING-GET"
           MOVE WS-SET-VALUE(1:2) TO WS-LANG
           EVALUATE WS-LANG
               WHEN "en" MOVE 1 TO WS-LANG-IX
               WHEN "pt" MOVE 2 TO WS-LANG-IX
               WHEN "es" MOVE 3 TO WS-LANG-IX
               WHEN "fr" MOVE 4 TO WS-LANG-IX
               WHEN "jp" MOVE 5 TO WS-LANG-IX
               WHEN "cn" MOVE 6 TO WS-LANG-IX
               WHEN OTHER MOVE "en" TO WS-LANG
                          MOVE 1 TO WS-LANG-IX
           END-EVALUATE
           PERFORM VARYING WS-TX-I FROM 1 BY 1 UNTIL WS-TX-I > 8
               MOVE PC-TEXT(WS-TX-I, WS-LANG-IX) TO PC-TEXT-NOW(WS-TX-I)
           END-PERFORM
           MOVE FUNCTION TRIM(T-AGENTS-TITLE) TO Lbl-Title::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-Save::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-Cancel::Caption
           MOVE FUNCTION TRIM(T-SAVE) TO Btn-SaveTop::Caption
           MOVE FUNCTION TRIM(T-CANCEL) TO Btn-CancelTop::Caption
           MOVE FUNCTION TRIM(T-STATUS) TO Lbl-Status::Caption
           MOVE T-AGENT-N TO WS-FMT
           MOVE 1 TO WS-A
           MOVE WS-A TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Agent-1::Caption
           MOVE FUNCTION TRIM(T-TIP-AGENT-CONN) TO Cmb-Agent-1::Tooltip
           MOVE T-AGENT-N TO WS-FMT
           MOVE 2 TO WS-A
           MOVE WS-A TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Agent-2::Caption
           MOVE FUNCTION TRIM(T-TIP-AGENT-CONN) TO Cmb-Agent-2::Tooltip
           MOVE T-AGENT-N TO WS-FMT
           MOVE 3 TO WS-A
           MOVE WS-A TO WS-ARG1
           CALL "PC-FMT"
           MOVE FUNCTION TRIM(WS-FMT-OUT) TO Lbl-Agent-3::Caption
           MOVE FUNCTION TRIM(T-TIP-AGENT-CONN) TO Cmb-Agent-3::Tooltip

           GOBACK.

       END PROGRAM PC-TEXTS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-FMT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PT              PIC 9(4).
       01 WS-FROM            PIC 9(4).
       01 WS-CNT             PIC 9(4).
       01 WS-PART            PIC X(120).
       01 WS-DELIM           PIC XX.
       PROCEDURE DIVISION.
      *>   A message whose words go round numbers and names differently in each
      *>   language: WS-FMT holds "&1" .. "&4" where WS-ARG1..4 go.
           MOVE SPACES TO WS-FMT-OUT
           MOVE 1 TO WS-PT
           MOVE 1 TO WS-FROM
           PERFORM UNTIL WS-FROM > FUNCTION LENGTH(WS-FMT)
               MOVE SPACES TO WS-PART WS-DELIM
               MOVE 0 TO WS-CNT
               UNSTRING WS-FMT DELIMITED BY "&1" OR "&2" OR "&3" OR "&4"
                   INTO WS-PART DELIMITER IN WS-DELIM COUNT IN WS-CNT
                   WITH POINTER WS-FROM
               END-UNSTRING
               IF WS-CNT > 0
                   STRING WS-PART(1:WS-CNT) DELIMITED BY SIZE
                       INTO WS-FMT-OUT WITH POINTER WS-PT
               END-IF
               EVALUATE WS-DELIM
                   WHEN "&1" STRING FUNCTION TRIM(WS-ARG1) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&2" STRING FUNCTION TRIM(WS-ARG2) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&3" STRING FUNCTION TRIM(WS-ARG3) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN "&4" STRING FUNCTION TRIM(WS-ARG4) DELIMITED BY SIZE
                                 INTO WS-FMT-OUT WITH POINTER WS-PT
                   WHEN OTHER COMPUTE WS-FROM = FUNCTION LENGTH(WS-FMT) + 1
               END-EVALUATE
           END-PERFORM

           GOBACK.

       END PROGRAM PC-FMT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-SAVE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Save - shared by the Save buttons at the top and at the bottom.
      *>   Each agent's connection, or none. Agent 1's is also PowerChat 1's
      *>   single MODEL-ENTRY, so "(off)" does not fall back to it.
           MOVE Cmb-Agent-1::GetSelectedIndex() TO WS-INDEX
           MOVE SPACES TO WS-SET-VALUE
           IF WS-INDEX > 0 AND WS-INDEX <= WS-CONN-COUNT
               MOVE WS-CONN-NAME(WS-INDEX) TO WS-SET-VALUE
           END-IF
           MOVE "AGENT-1-ENTRY" TO WS-SET-NAME
           CALL "PC-SETTING-PUT"
           MOVE "MODEL-ENTRY" TO WS-SET-NAME
           CALL "PC-SETTING-PUT"
           MOVE Cmb-Agent-2::GetSelectedIndex() TO WS-INDEX
           MOVE SPACES TO WS-SET-VALUE
           IF WS-INDEX > 0 AND WS-INDEX <= WS-CONN-COUNT
               MOVE WS-CONN-NAME(WS-INDEX) TO WS-SET-VALUE
           END-IF
           MOVE "AGENT-2-ENTRY" TO WS-SET-NAME
           CALL "PC-SETTING-PUT"
           MOVE Cmb-Agent-3::GetSelectedIndex() TO WS-INDEX
           MOVE SPACES TO WS-SET-VALUE
           IF WS-INDEX > 0 AND WS-INDEX <= WS-CONN-COUNT
               MOVE WS-CONN-NAME(WS-INDEX) TO WS-SET-VALUE
           END-IF
           MOVE "AGENT-3-ENTRY" TO WS-SET-NAME
           CALL "PC-SETTING-PUT"
           MOVE FUNCTION TRIM(T-AGENTS-SAVED) TO Lbl-Status::Caption
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM PC-SAVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PC-CANCEL IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
      *>   Cancel - shared by the Cancel buttons at the top and at the bottom.
      *>   Nothing is written: the dialog closes as it was opened.
           INVOKE ME::Close()

           GOBACK.

       END PROGRAM PC-CANCEL.

       END PROGRAM AGENTS-FORM.

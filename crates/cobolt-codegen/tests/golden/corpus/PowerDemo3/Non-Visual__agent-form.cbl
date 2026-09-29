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
       PROGRAM-ID. AGENT-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'AGENT-FORM'.

      *>── AI Agent infrastructure ────────────────────────────────────
      *>   INVOKE agent-id 'Ask' USING BY VALUE WS-AGENT-PROMPT
      *>   returns at once; the reply arrives as onResponse (LastReply)
      *>   or onError (LastError).
       01 WS-AGENT-PROMPT        PIC X(4096)  VALUE SPACES.
       01 WS-AGENT-RESPONSE      PIC X(32767) VALUE SPACES.
       01 WS-AGENT-ERROR         PIC X(512)   VALUE SPACES.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Lbl-Title.
          05 WS-Lbl-Title-TEXT       PIC X(256) VALUE 'AgentObject'.
          05 WS-Lbl-Title-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Title-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Sub.
          05 WS-Lbl-Sub-TEXT       PIC X(441) VALUE 'An AgentObject is a configured model endpoint. SetPrompt and SetModel change it from COBOL, Ask sends the question, and the reply arrives as onResponse - which reads it back from LastReply. Nothing here needs a network unless you press Ask.  In order to run this example you need a valid API Key to access Ollama (a model provider). Create one at www.ollama.com  After create your own API Key use it to set the AgentControl API KEY property.'.
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

       01 WS-Agent-Helper.
          05 WS-Agent-Helper-TEXT       PIC X(256) VALUE 'Agent-Helper'.
          05 WS-Agent-Helper-VISIBLE    PIC 9      VALUE 1.
          05 WS-Agent-Helper-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-Q-Cap.
          05 WS-Lbl-Q-Cap-TEXT       PIC X(256) VALUE 'Question'.
          05 WS-Lbl-Q-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-Q-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Question.
          05 WS-Txt-Question-TEXT       PIC X(2048) VALUE 'Tell me about your plans coverage'.
          05 WS-Txt-Question-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Question-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Question-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Lbl-A-Cap.
          05 WS-Lbl-A-Cap-TEXT       PIC X(256) VALUE 'Reply'.
          05 WS-Lbl-A-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-A-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Txt-Reply.
          05 WS-Txt-Reply-TEXT       PIC X(16384) VALUE 'Txt-Reply'.
          05 WS-Txt-Reply-VISIBLE    PIC 9      VALUE 1.
          05 WS-Txt-Reply-ENABLED    PIC 9      VALUE 1.
          05 WS-Txt-Reply-VALUE      PIC X(16384) VALUE SPACES.

       01 WS-Lbl-State-Cap.
          05 WS-Lbl-State-Cap-TEXT       PIC X(256) VALUE 'State'.
          05 WS-Lbl-State-Cap-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-State-Cap-ENABLED    PIC 9      VALUE 1.

       01 WS-Lbl-State.
          05 WS-Lbl-State-TEXT       PIC X(256) VALUE 'idle'.
          05 WS-Lbl-State-VISIBLE    PIC 9      VALUE 1.
          05 WS-Lbl-State-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Ask.
          05 WS-Btn-Ask-TEXT       PIC X(256) VALUE 'Ask'.
          05 WS-Btn-Ask-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Ask-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Persona.
          05 WS-Btn-Persona-TEXT       PIC X(256) VALUE 'Change persona'.
          05 WS-Btn-Persona-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Persona-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Cfg.
          05 WS-Btn-Cfg-TEXT       PIC X(256) VALUE 'Show config'.
          05 WS-Btn-Cfg-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Cfg-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Clear.
          05 WS-Btn-Clear-TEXT       PIC X(256) VALUE 'Clear log'.
          05 WS-Btn-Clear-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Clear-ENABLED    PIC 9      VALUE 1.

       01 WS-ComboBox-1.
          05 WS-ComboBox-1-TEXT       PIC X(256) VALUE 'ComboBox-1'.
          05 WS-ComboBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-ComboBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-ComboBox-1-VALUE      PIC X(512) VALUE SPACES.

       01 WS-Label-1.
          05 WS-Label-1-TEXT       PIC X(256) VALUE 'Model'.
          05 WS-Label-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-1-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-2.
          05 WS-Label-2-TEXT       PIC X(256) VALUE 'Change the model to see how it would change the answer'.
          05 WS-Label-2-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-2-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-3.
          05 WS-Label-3-TEXT       PIC X(256) VALUE 'System prompt'.
          05 WS-Label-3-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-3-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-1.
          05 WS-TextBox-1-TEXT       PIC X(2048) VALUE 'Your are an Insurance Agent, selling plans for companies interested in secure their facilities'.
          05 WS-TextBox-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-1-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-1-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-Btn-Lang-EN.
          05 WS-Btn-Lang-EN-TEXT       PIC X(256) VALUE 'EN'.
          05 WS-Btn-Lang-EN-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-EN-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lang-ES.
          05 WS-Btn-Lang-ES-TEXT       PIC X(256) VALUE 'ES'.
          05 WS-Btn-Lang-ES-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-ES-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lang-FR.
          05 WS-Btn-Lang-FR-TEXT       PIC X(256) VALUE 'FR'.
          05 WS-Btn-Lang-FR-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-FR-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lang-PT.
          05 WS-Btn-Lang-PT-TEXT       PIC X(256) VALUE 'PT'.
          05 WS-Btn-Lang-PT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-PT-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lang-CN.
          05 WS-Btn-Lang-CN-TEXT       PIC X(256) VALUE 'CN'.
          05 WS-Btn-Lang-CN-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-CN-ENABLED    PIC 9      VALUE 1.

       01 WS-Btn-Lang-JP.
          05 WS-Btn-Lang-JP-TEXT       PIC X(256) VALUE 'JP'.
          05 WS-Btn-Lang-JP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Btn-Lang-JP-ENABLED    PIC 9      VALUE 1.

       01 WS-Snackbar-1.
          05 WS-Snackbar-1-TEXT       PIC X(256) VALUE 'Snackbar-1'.
          05 WS-Snackbar-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-Snackbar-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "AGENT-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "AGENT-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Agent-Helper"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResponse"
                               CALL "AGENT-HELPER--ONRESPONSE"
                           WHEN "onError"
                               CALL "AGENT-HELPER--ONERROR"
                       END-EVALUATE
                   WHEN "Btn-Ask"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-ASK--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Persona"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-PERSONA--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Cfg"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CFG--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Clear"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-CLEAR--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-EN"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-EN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-ES"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-ES--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-FR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-FR--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-PT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-PT--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-CN"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-CN--ONCLICK"
                       END-EVALUATE
                   WHEN "Btn-Lang-JP"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-LANG-JP--ONCLICK"
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
       Agent-Helper-ASK.
      *>    Ask the AI agent Agent-Helper (model: gemma4:31b, endpoint: https://ollama.com/api/chat)
      *>    Set WS-AGENT-PROMPT before calling.
      *>    Returns at once. The reply arrives as AGENT-HELPER--ONRESPONSE
      *>    (read LastReply) or --ONERROR (read LastError).
           INVOKE Agent-Helper 'Ask'
               USING BY VALUE WS-AGENT-PROMPT.

       Agent-Helper-ON-RESPONSE.
      *>    TODO: Agent-Helper — not called by the runtime; bind onResponse and read LastReply
           CONTINUE.

       Agent-Helper-ON-ERROR.
      *>    TODO: Agent-Helper — not called by the runtime; bind onError and read LastError
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           SET TextBox-1::text TO Agent-Helper::SystemPrompt

           IF Agent-Helper::AgentAPIKey = ""
               SET Btn-Ask::Enabled TO false
               SET Snackbar-1::Text TO "Get a valid API Key at ollama.com"
               SET Snackbar-1::Category TO "critical"
               SET Snackbar-1::Visible TO true

               INVOKE SNACKBAR-1::Show()
           ELSE
               SET Btn-Ask::Enabled TO true
           END-IF

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los items PIC X SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE, PIC X 数据项按 SPACES。
           STRING "ready - " Agent-Helper::AgentAPI " / " Agent-Helper::AgentModel INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM AGENT-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM AGENT-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-HELPER--ONRESPONSE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

           MOVE FUNCTION CHAR(11) TO WS-NL.
      *> The reply is already on the object by the time this handler runs.
      *> EN: EXTENSION - a property reads as a value anywhere an operand is allowed.
      *> PT: EXTENSAO - a propriedade e lida como valor onde um operando for permitido.
      *> ES: EXTENSION - la propiedad se lee como valor donde se permita un operando.
      *> FR: EXTENSION - la propriete se lit comme valeur partout ou un operande est admis.
      *> JP: EXTENSION - プロパティは、オペランドを書ける場所ならどこでも値として読める。
      *> CN: EXTENSION - 凡是允许操作数的位置，属性都可当作值读取。
           MOVE Agent-Helper::LastReply TO Txt-Reply::Text.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE "answered" TO Lbl-State::Caption.
      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "reply received" INTO WS-LINE.
      *> EN: EXTENSION - inline method call on a control; no INVOKE ... USING needed.
      *> PT: EXTENSAO - chamada de metodo inline no controle; sem INVOKE ... USING.
      *> ES: EXTENSION - llamada a metodo en linea sobre el control; sin INVOKE ... USING.
      *> FR: EXTENSION - appel de methode en ligne sur le controle; sans INVOKE ... USING.
      *> JP: EXTENSION - コントロールのインライン メソッド呼び出し。INVOKE ... USING は不要。
      *> CN: EXTENSION - 控件的内联方法调用；无需 INVOKE ... USING。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM AGENT-HELPER--ONRESPONSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. AGENT-HELPER--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-LINE PIC X(160).
       01 WS-NL   PIC X.
       LINKAGE SECTION.
       01 ERROR-DESC PIC X(160).

       PROCEDURE DIVISION USING ERROR-DESC.

           MOVE FUNCTION CHAR(11) TO WS-NL.

           MOVE "error" TO Lbl-State::Caption.

           MOVE ERROR-DESC TO WS-LINE.

           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM AGENT-HELPER--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-ASK--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE  PIC X(160).
       01 WS-NL    PIC X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> EN: Setting WS-NL to newline character (CHAR(11)).
           *> PT: Definindo WS-NL como caractere de nova linha (CHAR(11)).
           *> ES: Estableciendo WS-NL como carácter de salto de línea (CHAR(11)).
           *> FR: Définir WS-NL comme caractère de nouvelle ligne (CHAR(11)).
           *> JP: WS-NL を改行文字 (CHAR(11)) に設定しています。
           *> CN: 将 WS-NL 设置为换行字符 (CHAR(11)).
           MOVE FUNCTION CHAR(11) TO WS-NL.

           MOVE "asking..." TO Lbl-State::Caption.

           *> Log the raw question text before calling the agent
           Txt-Log::AppendText(Txt-Question::Text).
           Txt-Log::AppendText(WS-NL).

           *> EN: Inline method call to Agent-Helper::Ask with question text.
           *> PT: Chamada inline ao método Agent-Helper::Ask com texto da pergunta.
           *> ES: Llamada inline al método Agent-Helper::Ask con el texto de la pregunta.
           *> FR: Appel inline à Agent-Helper::Ask avec le texte de la question.
           *> JP: Agent-Helper::Ask メソッドへのインラインコールに質問テキストを使用。
           *> CN: 使用问题文本进行的Agent-Helper::Ask内联方法调用。
           Agent-Helper::Ask(Txt-Question::Text).

           *> EN: Building line text by concatenating 'asked ' with AgentModel.
           *> PT: Construindo texto da linha concatenando 'asked ' com AgentModel.
           *> ES: Construyendo el texto de la línea concatenando 'asked ' con AgentModel.
           *> FR: Construction de la chaîne de ligne en concaténant 'asked ' avec AgentModel.
           *> JP: 'asked ' と AgentModel を連結して行テキストを作成。
           *> CN: 通过连接 "asked " 和 AgentModel 来构建行文本。
           STRING "asked " Agent-Helper::AgentModel INTO WS-LINE.

           *> EN: Appending trimmed WS-LINE to log.
           *> PT: Acrescentando WS-LINE ao log.
           *> ES: Añadiendo WS-LINE al registro.
           *> FR: Ajout de WS-LINE trimée au journal.
           *> JP: トリムされたWS-LINEをログに追加。
           *> CN: 将修剪后的WS-LINE追加到日志。
           Txt-Log::AppendText(FUNCTION TRIM(WS-LINE)).
           Txt-Log::AppendText(WS-NL).

           GOBACK.

       END PROGRAM BTN-ASK--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-PERSONA--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - fenced block literal: multi-line text taken verbatim, quotes never doubled.
      *> PT: EXTENSAO - literal em bloco cercado: texto multilinha literal, aspas sem duplicar.
      *> ES: EXTENSION - literal de bloque cercado: texto multilinea literal, comillas sin duplicar.
      *> FR: EXTENSION - litteral de bloc cloture: texte multiligne verbatim, guillemets non doubles.
      *> JP: EXTENSION - フェンス付きブロック リテラル。複数行をそのまま取り、引用符の二重化は不要。
      *> CN: EXTENSION - 围栏块字面量：逐字保留多行文本，引号无需重复书写。
           Agent-Helper::SetPrompt( TextBox-1::Text ).

           GOBACK.

       END PROGRAM BTN-PERSONA--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-CFG--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-TEXT PIC X(200).
       LINKAGE SECTION.

       PROCEDURE DIVISION.

      *> EN: EXTENSION - STRING with no DELIMITED BY: literals take SIZE, PIC X items take SPACES.
      *> PT: EXTENSAO - STRING sem DELIMITED BY: literais usam SIZE, itens PIC X usam SPACES.
      *> ES: EXTENSION - STRING sin DELIMITED BY: los literales usan SIZE, los PIC X usan SPACES.
      *> FR: EXTENSION - STRING sans DELIMITED BY: les litteraux prennent SIZE, les PIC X SPACES.
      *> JP: EXTENSION - DELIMITED BY を書かない STRING。定数は SIZE、PIC X 項目は SPACES が既定。
      *> CN: EXTENSION - STRING 省略 DELIMITED BY：字面量按 SIZE，PIC X 数据项按 SPACES。
           STRING "api=" Agent-Helper::AgentAPI
                  "  model=" Agent-Helper::AgentModel
                  "  url=" Agent-Helper::AgentURL
                  "  timeout=" Agent-Helper::TimeoutSeconds "s" INTO WS-TEXT.
      *> EN: EXTENSION - a property IS a receiving field, so MOVE/SET writes it directly.
      *> PT: EXTENSAO - a propriedade E um campo receptor, entao MOVE/SET escreve nela direto.
      *> ES: EXTENSION - la propiedad ES un campo receptor, asi que MOVE/SET escribe en ella.
      *> FR: EXTENSION - la propriete EST un champ recepteur, MOVE/SET y ecrit directement.
      *> JP: EXTENSION - プロパティは受取項目そのもの。MOVE/SET で直接書き込める。
      *> CN: EXTENSION - 属性本身就是接收项，MOVE/SET 可直接写入。
           MOVE WS-TEXT TO Lbl-Cfg::Caption.

           GOBACK.

       END PROGRAM BTN-CFG--ONCLICK.

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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-EN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
An AgentObject is a configured model endpoint. SetPrompt and SetModel change it from COBOL, Ask sends the question, and the reply arrives as onResponse - which reads it back from LastReply. Nothing here needs a network unless you press Ask.

In order to run this example you need a valid API Key to access Ollama (a model provider). Create one at www.ollama.com

After create your own API Key use it to set the AgentControl API KEY property.
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-EN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-ES--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
Un AgentObject es un punto final de modelo configurado. SetPrompt y SetModel lo cambian desde COBOL, Ask envía la pregunta, y la respuesta llega como onResponse - que la lee de LastReply. Nada aquí necesita una red a menos que presione Ask.

Para ejecutar este ejemplo, necesita una clave API válida para acceder a Ollama (un proveedor de modelos). Cree una en www.ollama.com

Después de crear su propia clave API, úsela para establecer la propiedad API KEY del AgentControl.
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-ES--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-FR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
Un AgentObject est un point de terminaison de modèle configuré. SetPrompt et SetModel le modifient depuis COBOL, Ask envoie la question, et la réponse arrive via onResponse - qui la lit depuis LastReply. Rien ici ne nécessite de réseau à moins que vous n'appuyiez sur Ask.

Pour exécuter cet exemple, vous avez besoin d'une clé API valide pour accéder à Ollama (un fournisseur de modèles). Créez-en une sur www.ollama.com

Après avoir créé votre propre clé API, utilisez-la pour définir la propriété API KEY de l'AgentControl.
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-FR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-PT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
Um AgentObject é um endpoint de modelo configurado. SetPrompt e SetModel o alteram a partir do COBOL, Ask envia a pergunta, e a resposta chega em onResponse - que a lê de LastReply. Nada aqui requer rede, a menos que você pressione Ask.

Para executar este exemplo, você precisa de uma Chave de API válida para acessar o Ollama (um provedor de modelos). Crie uma em www.ollama.com

Após criar sua própria Chave de API, use-a para definir a propriedade API KEY do AgentControl.
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-PT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-CN--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
AgentObject 是一个配置的模型端点。SetPrompt 和 SetModel 从 COBOL 更改它，Ask 发送问题，回复通过 onResponse 到达 - 它从 LastReply 中读取。除非您按下 Ask，否则这里不需要网络。

为了运行此示例，您需要一个有效的 API 密钥来访问 Ollama (一个模型提供商)。请在 www.ollama.com 创建一个。

创建自己的 API 密钥后，请使用它来设置 AgentControl 的 API KEY 属性。
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-CN--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-LANG-JP--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           MOVE 
```
AgentObjectは、設定済みのモデルエンドポイントです。SetPromptとSetModelでCOBOLから変更でき、Askで質問を送信すると、onResponseで応答が届きます。onResponseはLastReplyから内容を読み取ります。Askを押さない限り、ネットワーク通信は発生しません。

この例を実行するには、Ollama（モデルプロバイダー）にアクセスするための有効なAPIキーが必要です。www.ollama.com で作成してください。

APIキーを作成後、AgentControlのAPI KEYプロパティに設定して使用してください。
```
           TO Lbl-Sub::Caption.

           GOBACK.

       END PROGRAM BTN-LANG-JP--ONCLICK.

       END PROGRAM AGENT-FORM.

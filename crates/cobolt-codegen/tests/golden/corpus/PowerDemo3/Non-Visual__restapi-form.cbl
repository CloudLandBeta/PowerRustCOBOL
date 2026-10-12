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
       PROGRAM-ID. RESTAPI-FORM.

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
       01 FORM-NAME               PIC X(64)   VALUE 'RESTAPI-FORM'.

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

      *>── REST client: RestClient-1 ──────────────────────────────────
       01 WS-RestClient-1-BASE-URL      PIC X(2048) VALUE 'https://reqres.in/api/users?page=2'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-Label-TITLE.
          05 WS-Label-TITLE-TEXT       PIC X(256) VALUE 'REST Client - a call, narrated'.
          05 WS-Label-TITLE-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-TITLE-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-HINT.
          05 WS-Label-HINT-TEXT       PIC X(256) VALUE 'Every step is logged: what is sent, where, with what payload, and which event answered. The API key is never printed - only whether one is configured.'.
          05 WS-Label-HINT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-HINT-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-GET.
          05 WS-Button-REST-GET-TEXT       PIC X(256) VALUE '1  GET using the control''s BaseURL'.
          05 WS-Button-REST-GET-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-GET-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-GETURL.
          05 WS-Button-REST-GETURL-TEXT       PIC X(256) VALUE '2  GET an explicit URL'.
          05 WS-Button-REST-GETURL-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-GETURL-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-POST.
          05 WS-Button-REST-POST-TEXT       PIC X(256) VALUE '3  POST a payload'.
          05 WS-Button-REST-POST-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-POST-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-404.
          05 WS-Button-REST-404-TEXT       PIC X(256) VALUE '4  Force a 404'.
          05 WS-Button-REST-404-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-404-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-BADHOST.
          05 WS-Button-REST-BADHOST-TEXT       PIC X(256) VALUE '5  Force a transport error'.
          05 WS-Button-REST-BADHOST-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-BADHOST-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-TIMEOUT.
          05 WS-Button-REST-TIMEOUT-TEXT       PIC X(256) VALUE '6  Force a timeout'.
          05 WS-Button-REST-TIMEOUT-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-TIMEOUT-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-CANCEL.
          05 WS-Button-REST-CANCEL-TEXT       PIC X(256) VALUE '7  Cancel the call in flight'.
          05 WS-Button-REST-CANCEL-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-CANCEL-ENABLED    PIC 9      VALUE 1.

       01 WS-Button-REST-CLEAR.
          05 WS-Button-REST-CLEAR-TEXT       PIC X(256) VALUE '8  Clear the log'.
          05 WS-Button-REST-CLEAR-VISIBLE    PIC 9      VALUE 1.
          05 WS-Button-REST-CLEAR-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-STATUSCAP.
          05 WS-Label-STATUSCAP-TEXT       PIC X(256) VALUE 'Status:'.
          05 WS-Label-STATUSCAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-STATUSCAP-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-STATUS.
          05 WS-Label-STATUS-TEXT       PIC X(256) VALUE 'ready'.
          05 WS-Label-STATUS-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-STATUS-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-LOGCAP.
          05 WS-Label-LOGCAP-TEXT       PIC X(256) VALUE 'Call log'.
          05 WS-Label-LOGCAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-LOGCAP-ENABLED    PIC 9      VALUE 1.

       01 WS-Label-BODYCAP.
          05 WS-Label-BODYCAP-TEXT       PIC X(256) VALUE 'Response body'.
          05 WS-Label-BODYCAP-VISIBLE    PIC 9      VALUE 1.
          05 WS-Label-BODYCAP-ENABLED    PIC 9      VALUE 1.

       01 WS-TextBox-LOG.
          05 WS-TextBox-LOG-TEXT       PIC X(2048) VALUE 'TextBox-LOG'.
          05 WS-TextBox-LOG-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-LOG-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-LOG-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-TextBox-BODY.
          05 WS-TextBox-BODY-TEXT       PIC X(2048) VALUE 'TextBox-BODY'.
          05 WS-TextBox-BODY-VISIBLE    PIC 9      VALUE 1.
          05 WS-TextBox-BODY-ENABLED    PIC 9      VALUE 1.
          05 WS-TextBox-BODY-VALUE      PIC X(2048) VALUE SPACES.

       01 WS-RestClient-1.
          05 WS-RestClient-1-TEXT       PIC X(256) VALUE 'RestClient-1'.
          05 WS-RestClient-1-VISIBLE    PIC 9      VALUE 1.
          05 WS-RestClient-1-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           COBOL::"INIT-FORM" ( FORM-NAME )
           CALL "RESTAPI-FORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "RESTAPI-FORM--ONCLOSE"
           STOP RUN.

      *> <EVENT-LOOP>
       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               COBOL::"WAIT-EVENT" ( COBOL-EVENT-ID COBOL-CONTROL-ID )
               EVALUATE COBOL-CONTROL-ID
                   WHEN "Button-REST-GET"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-GET--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-GETURL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-GETURL--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-POST"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-POST--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-404"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-404--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-BADHOST"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-BADHOST--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-TIMEOUT"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-TIMEOUT--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-CANCEL"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-CANCEL--ONCLICK"
                       END-EVALUATE
                   WHEN "Button-REST-CLEAR"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BUTTON-REST-CLEAR--ONCLICK"
                       END-EVALUATE
                   WHEN "RestClient-1"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onComplete"
                               CALL "RESTCLIENT-1--ONCOMPLETE"
                           WHEN "onError"
                               CALL "RESTCLIENT-1--ONERROR"
                           WHEN "onTimeout"
                               CALL "RESTCLIENT-1--ONTIMEOUT"
                           WHEN "onCancelled"
                               CALL "RESTCLIENT-1--ONCANCELLED"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

      *> </EVENT-LOOP>
      *> <TIMER-STUBS>
      *> </TIMER-STUBS>
      *> <CSV-EXPORT>
      *> </CSV-EXPORT>
      *> <REST-CLIENT>
       RestClient-1-GET.
      *>    HTTP GET via RestClient-1 — set WS-REQUEST-URL before calling.
           COBOL::"HTTP-GET" ( WS-REQUEST-URL
                 WS-HTTP-RESPONSE
                 WS-HTTP-STATUS )
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM RestClient-1-ON-RESPONSE
               WHEN OTHER
                   PERFORM RestClient-1-ON-ERROR
           END-EVALUATE.

       RestClient-1-POST.
      *>    HTTP POST via RestClient-1 — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.
           COBOL::"HTTP-POST" ( WS-REQUEST-URL
                 WS-REQUEST-BODY
                 WS-HTTP-RESPONSE
                 WS-HTTP-STATUS )
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM RestClient-1-ON-RESPONSE
               WHEN OTHER
                   PERFORM RestClient-1-ON-ERROR
           END-EVALUATE.

       RestClient-1-PUT.
      *>    HTTP PUT via RestClient-1 — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.
           COBOL::"HTTP-PUT" ( WS-REQUEST-URL
                 WS-REQUEST-BODY
                 WS-HTTP-RESPONSE
                 WS-HTTP-STATUS )
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM RestClient-1-ON-RESPONSE
               WHEN OTHER
                   PERFORM RestClient-1-ON-ERROR
           END-EVALUATE.

       RestClient-1-ON-RESPONSE.
      *>    TODO: RestClient-1 response handler — WS-HTTP-RESPONSE contains the body, WS-HTTP-STATUS the code
           CONTINUE.

       RestClient-1-ON-ERROR.
      *>    TODO: RestClient-1 error handler — WS-HTTP-STATUS contains the error code (0 = network failure)
           CONTINUE.

      *> </REST-CLIENT>
      *> <WEB-SEARCH>
      *> </WEB-SEARCH>

      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTAPI-FORM--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Everything the control will use, read back from the control itself.
           *> The token is NEVER printed - only whether one is configured.

           TextBox-LOG::AppendText("RestClient-1 configuration")
           SET     WS-NL                         TO    FUNCTION CHAR(11)
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::BaseURL

           TextBox-LOG::AppendText("   BaseURL:   ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::DefaultMethod

           TextBox-LOG::AppendText("   Method:    ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::AuthType

           TextBox-LOG::AppendText("   AuthType:  ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::AuthToken
           IF    FUNCTION TRIM(WS-LINE) = SPACES
               TextBox-LOG::AppendText("   AuthToken: (none configured)")
           ELSE
               TextBox-LOG::AppendText("   AuthToken: (configured, sent as X-API-Key)")
           END-IF
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::Mode

           TextBox-LOG::AppendText("   Mode:      ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(" (Async returns at once; the answer arrives as an event)")
           TextBox-LOG::AppendText(WS-NL)
           TextBox-LOG::AppendText(WS-NL)

           SET     Label-STATUS::Caption         TO    "ready"
           DISPLAY    "restapi-form ready".

           GOBACK.

       END PROGRAM RESTAPI-FORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTAPI-FORM--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM RESTAPI-FORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-GET--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The idiomatic call: the address is configured in the designer and the
           *> handler asks for it with NO argument. Before 1.62.144 this requested
           *> the empty string and could only fail.
           *> Mode is Async, so Get() RETURNS IMMEDIATELY and the answer arrives at
           *> onComplete / onError / onTimeout. Nothing below this line has the
           *> response yet - that is what the log is for.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- GET (BaseURL, no argument)")
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::BaseURL
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   url:     ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::AuthType

           TextBox-LOG::AppendText("   auth:    ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::TimeoutMs

           TextBox-LOG::AppendText("   timeout: ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(" ms")
           TextBox-LOG::AppendText(WS-NL)

           RestClient-1::Get()

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent - waiting for onComplete / onError / onTimeout")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: GET issued".

           GOBACK.

       END PROGRAM BUTTON-REST-GET--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-GETURL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> An argument that carries its own scheme WINS over BaseURL, so a
           *> handler can go somewhere else without disturbing the designed address.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- GET (explicit url)")
           TextBox-LOG::AppendText(WS-NL)
           TextBox-LOG::AppendText("   url:     https://reqres.in/api/users?page=2")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::Get("https://reqres.in/api/users?page=2")

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: GET explicit issued".

           GOBACK.

       END PROGRAM BUTTON-REST-GETURL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-POST--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.
       01 WS-BODY                                PIC    X(200).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> POST carries a body; GET and DELETE do not. Build the payload in
           *> COBOL first - it is data, not a format string.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- POST")
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-BODY                       TO    '{"name":"morpheus","job":"leader"}'

           TextBox-LOG::AppendText("   url:     https://reqres.in/users")
           TextBox-LOG::AppendText(WS-NL)
           TextBox-LOG::AppendText("   payload: ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-BODY))
           TextBox-LOG::AppendText(WS-NL)

           RestClient-1::Post("https://reqres.in/users", FUNCTION TRIM(WS-BODY))
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: POST issued".

           GOBACK.

       END PROGRAM BUTTON-REST-POST--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-404--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> A 4xx is a REAL RESPONSE, not a transport failure: it arrives at
           *> onComplete with its own StatusCode and body. onError is for the call
           *> never completing at all - DNS, TLS, a refused connection.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- GET a path that does not exist")
           TextBox-LOG::AppendText(WS-NL)
           TextBox-LOG::AppendText("   url:     https://reqres.in/unknown/99999")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::Get("https://reqres.in/unknown/99999")

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent - expect onComplete with status 404")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: 404 probe issued".

           GOBACK.

       END PROGRAM BUTTON-REST-404--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-BADHOST--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> A host that cannot be resolved never produces an HTTP status at all,
           *> so this is the onError path. LastError carries the reason.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- GET a host that does not resolve")
           TextBox-LOG::AppendText(WS-NL)
           TextBox-LOG::AppendText("   url:     https://no-such-host.invalid/x")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::Get("https://no-such-host.invalid/x")

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent - expect onError")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: bad host issued".

           GOBACK.

       END PROGRAM BUTTON-REST-BADHOST--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-TIMEOUT--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> SetTimeout is in SECONDS. One second against a deliberately slow
           *> endpoint gives onTimeout rather than onComplete. The timeout is put
           *> back afterwards so the other buttons still work.

           SET     Label-STATUS::Caption         TO    "calling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- GET with a 1 second timeout")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::SetTimeout(1)
           TextBox-LOG::AppendText("   url:     https://reqres.in/users?delay=6")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::Get("https://reqres.in/users?delay=6")

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   sent - expect onTimeout")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: timeout probe issued".

           GOBACK.

       END PROGRAM BUTTON-REST-TIMEOUT--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-CANCEL--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Cancel() abandons whatever is running on THIS control. It fires
           *> onCancelled; the response, if it ever arrives, is discarded.

           SET     Label-STATUS::Caption         TO    "cancelling..."
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- Cancel()")
           TextBox-LOG::AppendText(WS-NL)
           RestClient-1::Cancel()

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   cancel requested")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST: cancel issued".

           GOBACK.

       END PROGRAM BUTTON-REST-CANCEL--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BUTTON-REST-CLEAR--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Clear() on a TextBox empties its Text - unlike Clear() on a Snackbar,
           *> which empties its button row and leaves the message alone.

           TextBox-LOG::Clear()
           TextBox-BODY::Clear()

           SET     Label-STATUS::Caption         TO    "ready"
           DISPLAY    "REST: log cleared".

           GOBACK.

       END PROGRAM BUTTON-REST-CLEAR--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTCLIENT-1--ONCOMPLETE IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.
       01 WS-N                                   PIC    X(12).
       01 WS-BODY                                PIC    X(4000).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The call finished and produced an HTTP STATUS - including 4xx and 5xx.
           *> StatusCode and ResponseBody are written BEFORE this handler runs.

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("++ onComplete")
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-N                          TO    RestClient-1::StatusCode

           TextBox-LOG::AppendText("   status:  ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-N))
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-BODY                       TO    RestClient-1::ResponseBody

           TextBox-BODY::Clear()
           TextBox-BODY::AppendText(FUNCTION TRIM(WS-BODY))

           *> Only 200 and 201 count as success. Any other status, even another
           *> 2xx such as 204, takes the ELSE branch.

           IF    FUNCTION TRIM(WS-N) = "200" OR FUNCTION TRIM(WS-N) = "201"
               SET     Label-STATUS::Caption     TO    "success"
           ELSE
               SET     Label-STATUS::Caption     TO    "the server answered, but not with 2xx"
           END-IF
           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("   body shown in the panel on the right")
           TextBox-LOG::AppendText(WS-NL)

           DISPLAY    "REST onComplete".

           GOBACK.

       END PROGRAM RESTCLIENT-1--ONCOMPLETE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTCLIENT-1--ONERROR IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.
       01 WS-N                                   PIC    X(12).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The call never produced a status: DNS, TLS, a refused connection, a
           *> malformed URL. LastError carries the reason.

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("!! onError")
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-LINE                       TO    RestClient-1::LastError

           TextBox-LOG::AppendText("   reason:  ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-LINE))
           TextBox-LOG::AppendText(WS-NL)

           SET     Label-STATUS::Caption         TO    "FAILED - see the log"
           DISPLAY    "REST onError".

           GOBACK.

       END PROGRAM RESTCLIENT-1--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTCLIENT-1--ONTIMEOUT IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.
       01 WS-N                                   PIC    X(12).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> The call outlived its timeout. Nothing is retried automatically.

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("!! onTimeout")
           TextBox-LOG::AppendText(WS-NL)

           SET     WS-N                          TO    RestClient-1::TimeoutMs

           TextBox-LOG::AppendText("   after:   ")
           TextBox-LOG::AppendText(FUNCTION TRIM(WS-N))
           TextBox-LOG::AppendText(" ms")
           TextBox-LOG::AppendText(WS-NL)

           SET     Label-STATUS::Caption         TO    "TIMED OUT"
           DISPLAY    "REST onTimeout".

           GOBACK.

       END PROGRAM RESTCLIENT-1--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RESTCLIENT-1--ONCANCELLED IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-NL                                  PIC    X.
       01 WS-N                                   PIC    X(12).

       LINKAGE SECTION.

       PROCEDURE DIVISION.

           *> Cancel() was called while this control had something in flight.

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           TextBox-LOG::AppendText("-- onCancelled")
           TextBox-LOG::AppendText(WS-NL)

           SET     Label-STATUS::Caption         TO    "cancelled"
           DISPLAY    "REST onCancelled".

           GOBACK.

       END PROGRAM RESTCLIENT-1--ONCANCELLED.

       END PROGRAM RESTAPI-FORM.

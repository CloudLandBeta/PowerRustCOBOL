       IDENTIFICATION DIVISION.
       PROGRAM-ID. FLOW-TEST.
      *> Test of the sending and approval of quotes: QUOTE-SEND, QUOTE-VIEW, QUOTE-APPROVE and QUOTE-EXPIRE, with
      *> QUOTE-SAVE and QUOTE-GET (CUSTOMER-SAVE and VEHICLE-SAVE give them customers and vehicles).
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding copies of the project's catalog.idx and sequences.idx.
      *> Cases: sending by both channels, with one channel down (3 tries, the other still delivered), with both down, to a
      *> customer with one consent and with none; sending again replaces the old link; the customer's page by token and
      *> code, by quote number and code, with a wrong code and with an unknown token; approving by the link once only;
      *> refusing with and without a reason; the consultant's manual answers on every channel; a refused quote revised,
      *> an approved quote that cannot be changed; quotes past their validity marked and shown as expired.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT QUOTE-FILE ASSIGN TO "data/quotes.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QUO-KEY
               ALTERNATE RECORD KEY IS QUO-CUS-ID WITH DUPLICATES
               ALTERNATE RECORD KEY IS QUO-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-QUO.

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-quote".
           COPY "backend/COPYBOOKS/res-quote".
           COPY "backend/COPYBOOKS/req-quote-get".
           COPY "backend/COPYBOOKS/req-quote-send".
           COPY "backend/COPYBOOKS/res-quote-send".
           COPY "backend/COPYBOOKS/req-quote-view".
           COPY "backend/COPYBOOKS/req-quote-approve".
           COPY "backend/COPYBOOKS/res-quote-approve".
           COPY "backend/COPYBOOKS/req-quote-expire".
           COPY "backend/COPYBOOKS/res-quote-expire".
       01 WS-FS-QUO                              PIC    XX.
       01 WS-PLATE-ARG                           PIC    X(8).
       01 WS-CASE-NAME                           PIC    X(64).
       01 WS-OK                                  PIC    9.
       01 WS-CUS-BOTH                            PIC    9(8).
       01 WS-CUS-MAIL                            PIC    9(8).
       01 WS-CUS-NONE                            PIC    9(8).
       01 WS-Q1                                  PIC    9(8).
       01 WS-Q2                                  PIC    9(8).
       01 WS-Q3                                  PIC    9(8).
       01 WS-Q4                                  PIC    9(8).
       01 WS-Q5                                  PIC    9(8).
       01 WS-Q6                                  PIC    9(8).
       01 WS-TOKEN-1                             PIC    X(32).
       01 WS-CODE-1                              PIC    X(6).
       01 WS-TOKEN-OLD                           PIC    X(32).
       01 WS-CODE-OLD                            PIC    X(6).
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME

           PERFORM MAKE-BASE-DATA
           PERFORM TEST-SEND
           PERFORM TEST-VIEW
           PERFORM TEST-APPROVE
           PERFORM TEST-MANUAL
           PERFORM TEST-EXPIRE

           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0

           DISPLAY "Quote sending and approval test (QUOTE-SEND, QUOTE-VIEW, QUOTE-APPROVE, QUOTE-EXPIRE)"
           DISPLAY "  cases run      " WS-CASES
           DISPLAY "  passed         " WS-PASS
           DISPLAY "  failed         " WS-FAIL
           DISPLAY "  elapsed        " WS-ELAPSED " hundredths of a second"
           IF    WS-FAIL = 0
               DISPLAY "  result         PASS"
           ELSE
               DISPLAY "  result         FAIL"
           END-IF
           STOP RUN.

       TALLY.
           ADD     1                             TO    WS-CASES
           IF    WS-OK = 1
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
           END-IF.

      *> MAKE-BASE-DATA: three customers (both channels, email only, no consent), each with a vehicle, and six quotes.
       MAKE-BASE-DATA.
           INITIALIZE CUS-REQ
           MOVE    "Cliente Dois Canais"         TO    CRQ-NAME
           MOVE    "52998224725"                 TO    CRQ-DOC
           MOVE    "(11) 99999-0001"             TO    CRQ-WHATSAPP
           MOVE    "dois@exemplo.com.br"         TO    CRQ-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-CUS-BOTH

           INITIALIZE CUS-REQ
           MOVE    "Cliente So Email"            TO    CRQ-NAME
           MOVE    "11144477735"                 TO    CRQ-DOC
           MOVE    "so-email@exemplo.com.br"     TO    CRQ-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-CUS-MAIL

           INITIALIZE CUS-REQ
           MOVE    "Cliente Sem Consentimento"   TO    CRQ-NAME
           MOVE    "39053344705"                 TO    CRQ-DOC
           MOVE    "N"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-CUS-NONE

           MOVE    "AAA1A11"                     TO    VRQ-PLATE
           MOVE    WS-CUS-BOTH                   TO    VRQ-CUS-ID
           PERFORM MAKE-VEHICLE
           MOVE    "BBB2B22"                     TO    VRQ-PLATE
           MOVE    WS-CUS-MAIL                   TO    VRQ-CUS-ID
           PERFORM MAKE-VEHICLE
           MOVE    "CCC3C33"                     TO    VRQ-PLATE
           MOVE    WS-CUS-NONE                   TO    VRQ-CUS-ID
           PERFORM MAKE-VEHICLE

           MOVE    "AAA1A11"                     TO    WS-PLATE-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q1
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q2
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q3
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q4
           MOVE    "BBB2B22"                     TO    WS-PLATE-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q5
           MOVE    "CCC3C33"                     TO    WS-PLATE-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q6.

       MAKE-VEHICLE.
           MOVE    "Fiat"                        TO    VRQ-BRAND
           MOVE    "Palio 1.0"                   TO    VRQ-MODEL
           MOVE    2014                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES.

       MAKE-QUOTE.
           INITIALIZE QUO-REQ
           MOVE    WS-PLATE-ARG                  TO    QRQ-PLATE
           MOVE    "BRL"                         TO    QRQ-CURRENCY
           MOVE    "Maria Atendente"             TO    QRQ-OPERATOR
           MOVE    1                             TO    QRQ-COUNT
           MOVE    "P"                           TO    QRL-KIND (1)
           MOVE    "VELA"                        TO    QRL-CODE (1)
           MOVE    1                             TO    QRL-QTY (1)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES.

       GET-QUOTE.
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES.

       SEND-IT.
           MOVE    "Maria Atendente"             TO    QSN-OPERATOR
           CALL    "QUOTE-SEND" USING QSN-REQ RETURNING QSN-RES.

       TEST-SEND.
           MOVE    "send: both channels delivered on the first try"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q1                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    QNS-TOKEN                     TO    WS-TOKEN-1
           MOVE    QNS-CODE                      TO    WS-CODE-1
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "OK" AND QNS-EMAIL = "SENT" AND QNS-WA = "SENT"
                 AND QNS-EMAIL-TRIES = 1 AND QNS-WA-TRIES = 1
                 AND QNS-TOKEN NOT = SPACES AND QNS-CODE IS NUMERIC AND QNS-TEXT NOT = SPACES
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: the quote is now ENVIADO"  TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    QGQ-ID
           PERFORM GET-QUOTE
           MOVE    0                             TO    WS-OK
           IF    QTS-QSTATUS = "ENVIADO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: email down, 3 tries then FAILED, WhatsApp still delivered"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q2                         TO    QSN-ID
           MOVE    "S"                           TO    QSN-DOWN-EMAIL
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "OK" AND QNS-EMAIL = "FAILED" AND QNS-EMAIL-TRIES = 3
                 AND QNS-WA = "SENT" AND QNS-MESSAGE NOT = SPACES
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: WhatsApp down, email still delivered"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q3                         TO    QSN-ID
           MOVE    "S"                           TO    QSN-DOWN-WA
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "OK" AND QNS-EMAIL = "SENT" AND QNS-WA = "FAILED" AND QNS-WA-TRIES = 3
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: both channels down, nothing delivered, quote stays open"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q4                         TO    QSN-ID
           MOVE    "S"                           TO    QSN-DOWN-EMAIL
           MOVE    "S"                           TO    QSN-DOWN-WA
           PERFORM SEND-IT
           MOVE    WS-Q4                         TO    QGQ-ID
           PERFORM GET-QUOTE
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "NOCHANNEL" AND QNS-EMAIL = "FAILED" AND QNS-WA = "FAILED"
                 AND QTS-QSTATUS = "ABERTO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: customer with email consent only, WhatsApp skipped"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q5                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "OK" AND QNS-EMAIL = "SENT" AND QNS-WA = "SKIPPED" AND QNS-WA-TRIES = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: customer without consent, both skipped"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q6                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "NOCHANNEL" AND QNS-EMAIL = "SKIPPED" AND QNS-WA = "SKIPPED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: quote that does not exist"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    99999                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: operator missing"      TO    WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q1                         TO    QSN-ID
           MOVE    SPACES                        TO    QSN-OPERATOR
           CALL    "QUOTE-SEND" USING QSN-REQ RETURNING QSN-RES
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send again: a new token, the old one is replaced"  TO WS-CASE-NAME
           MOVE    WS-TOKEN-1                    TO    WS-TOKEN-OLD
           MOVE    WS-CODE-1                     TO    WS-CODE-OLD
           INITIALIZE QSN-REQ
           MOVE    WS-Q1                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    QNS-TOKEN                     TO    WS-TOKEN-1
           MOVE    QNS-CODE                      TO    WS-CODE-1
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "OK" AND WS-TOKEN-1 NOT = WS-TOKEN-OLD
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       VIEW-IT.
           CALL    "QUOTE-VIEW" USING QVW-REQ RETURNING QUO-RES.

       TEST-VIEW.
           MOVE    "view: token and code open the quote"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-TOKEN-1                    TO    QVW-TOKEN
           MOVE    WS-CODE-1                     TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ID = WS-Q1 AND QTS-COUNT = 1 AND QTS-TOTAL > 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: quote number and code open it when the page has no token"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-Q1                         TO    QVW-QUO-ID
           MOVE    WS-CODE-1                     TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ID = WS-Q1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: wrong code shows no quote"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-TOKEN-1                    TO    QVW-TOKEN
           MOVE    "000000"                      TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "BADCODE" AND QTS-COUNT = 0 AND QTS-TOTAL = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: the replaced token no longer works"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-TOKEN-OLD                  TO    QVW-TOKEN
           MOVE    WS-CODE-OLD                   TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "REPLACED" AND QTS-COUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: unknown token"         TO    WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"  TO QVW-TOKEN
           MOVE    "123456"                      TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: a quote whose message reached nobody shows nothing (code is checked first)"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-Q6                         TO    QVW-QUO-ID
           MOVE    "123456"                      TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "BADCODE" AND QTS-COUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       APPROVE-IT.
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES.

       TEST-APPROVE.
           MOVE    "approve by link: wrong code is refused, token stays valid"  TO WS-CASE-NAME
           INITIALIZE QAP-REQ
           MOVE    "L"                           TO    QAP-MODE
           MOVE    WS-TOKEN-1                    TO    QAP-TOKEN
           MOVE    "000000"                      TO    QAP-CODE
           MOVE    "A"                           TO    QAP-DECISION
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "BADCODE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "approve by link: refusing needs a reason"  TO WS-CASE-NAME
           MOVE    WS-CODE-1                     TO    QAP-CODE
           MOVE    "R"                           TO    QAP-DECISION
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "INVALID" AND QAS-FIELD = "REASON"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "approve by link: customer approves, quote is APROVADO"  TO WS-CASE-NAME
           MOVE    "A"                           TO    QAP-DECISION
           PERFORM APPROVE-IT
           MOVE    WS-Q1                         TO    QGQ-ID
           PERFORM GET-QUOTE
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "OK" AND QAS-QSTATUS = "APROVADO" AND QAS-CHANNEL = "LINK"
                 AND QAS-AT > 0 AND QTS-QSTATUS = "APROVADO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "approve by link: the token is used once"  TO WS-CASE-NAME
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "USED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "view: a used token shows nothing"  TO WS-CASE-NAME
           INITIALIZE QVW-REQ
           MOVE    WS-TOKEN-1                    TO    QVW-TOKEN
           MOVE    WS-CODE-1                     TO    QVW-CODE
           PERFORM VIEW-IT
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "USED" AND QTS-COUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "an approved quote cannot be changed"  TO WS-CASE-NAME
           INITIALIZE QUO-REQ
           MOVE    WS-Q1                         TO    QRQ-ID
           MOVE    "AAA1A11"                     TO    QRQ-PLATE
           MOVE    "BRL"                         TO    QRQ-CURRENCY
           MOVE    "Maria Atendente"             TO    QRQ-OPERATOR
           MOVE    1                             TO    QRQ-COUNT
           MOVE    "P"                           TO    QRL-KIND (1)
           MOVE    "VELA"                        TO    QRL-CODE (1)
           MOVE    3                             TO    QRL-QTY (1)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "STATUS"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "approve by link: customer refuses with a reason"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q5                         TO    QSN-ID
           PERFORM SEND-IT
           INITIALIZE QAP-REQ
           MOVE    "L"                           TO    QAP-MODE
           MOVE    WS-Q5                         TO    QAP-QUO-ID
           MOVE    QNS-CODE                      TO    QAP-CODE
           MOVE    "R"                           TO    QAP-DECISION
           MOVE    "Achei caro, peço desconto."  TO    QAP-REASON
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "OK" AND QAS-QSTATUS = "RECUSADO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "a refused quote goes back to the consultant: revision 2"  TO WS-CASE-NAME
           INITIALIZE QUO-REQ
           MOVE    WS-Q5                         TO    QRQ-ID
           MOVE    "BBB2B22"                     TO    QRQ-PLATE
           MOVE    "BRL"                         TO    QRQ-CURRENCY
           MOVE    "Maria Atendente"             TO    QRQ-OPERATOR
           MOVE    1                             TO    QRQ-COUNT
           MOVE    "P"                           TO    QRL-KIND (1)
           MOVE    "VELA"                        TO    QRL-CODE (1)
           MOVE    1                             TO    QRL-QTY (1)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ACTION = "REVISED" AND QTS-REV = 2 AND QTS-QSTATUS = "ABERTO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-MANUAL.
           MOVE    "manual: consultant registers approval from the counter"  TO WS-CASE-NAME
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    WS-Q2                         TO    QAP-QUO-ID
           MOVE    "COUNTER"                     TO    QAP-CHANNEL
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "OK" AND QAS-QSTATUS = "APROVADO" AND QAS-CHANNEL = "COUNTER"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: an answer cannot be registered twice"  TO WS-CASE-NAME
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "NOTOPEN"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: phone refusal of a quote never sent, with reason"  TO WS-CASE-NAME
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    WS-Q6                         TO    QAP-QUO-ID
           MOVE    "PHONE"                       TO    QAP-CHANNEL
           MOVE    "R"                           TO    QAP-DECISION
           MOVE    "Vai fazer em outra oficina." TO    QAP-REASON
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "OK" AND QAS-QSTATUS = "RECUSADO" AND QAS-CHANNEL = "PHONE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: WhatsApp channel is accepted"  TO WS-CASE-NAME
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    WS-Q3                         TO    QAP-QUO-ID
           MOVE    "WHATSAPP"                    TO    QAP-CHANNEL
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "OK" AND QAS-QSTATUS = "APROVADO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: channel LINK is not allowed by hand"  TO WS-CASE-NAME
           MOVE    "LINK"                        TO    QAP-CHANNEL
           MOVE    WS-Q4                         TO    QAP-QUO-ID
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "INVALID" AND QAS-FIELD = "CHANNEL"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: operator missing"    TO    WS-CASE-NAME
           MOVE    "PHONE"                       TO    QAP-CHANNEL
           MOVE    SPACES                        TO    QAP-OPERATOR
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "INVALID" AND QAS-FIELD = "OPERATOR"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: decision that is neither A nor R"  TO WS-CASE-NAME
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           MOVE    "X"                           TO    QAP-DECISION
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "INVALID" AND QAS-FIELD = "DECISION"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: quote that does not exist"  TO WS-CASE-NAME
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    99999                         TO    QAP-QUO-ID
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

      *> BACKDATE: puts the validity of quote QGQ-ID, revision 1, in the past, as if time had passed.
       BACKDATE.
           OPEN    I-O QUOTE-FILE
           MOVE    QGQ-ID                        TO    QUO-ID
           MOVE    1                             TO    QUO-REV
           READ    QUOTE-FILE
               INVALID KEY
                   DISPLAY "  backdate: quote not found"
               NOT INVALID KEY
                   MOVE    20200101              TO    QUO-VALID-UNTIL
                   REWRITE QUOTE-RECORD
                   END-REWRITE
           END-READ
           CLOSE   QUOTE-FILE.

       TEST-EXPIRE.
           MOVE    WS-Q4                         TO    QGQ-ID
           PERFORM BACKDATE
           MOVE    "expire: an open quote past its validity is shown as expired"  TO WS-CASE-NAME
           PERFORM GET-QUOTE
           MOVE    0                             TO    WS-OK
           IF    QTS-QSTATUS = "EXPIRADO"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "send: an expired quote cannot be sent"  TO WS-CASE-NAME
           INITIALIZE QSN-REQ
           MOVE    WS-Q4                         TO    QSN-ID
           PERFORM SEND-IT
           MOVE    0                             TO    WS-OK
           IF    QNS-STATUS = "NOTOPEN"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "manual: an expired quote cannot be approved"  TO WS-CASE-NAME
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    WS-Q4                         TO    QAP-QUO-ID
           MOVE    "COUNTER"                     TO    QAP-CHANNEL
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           PERFORM APPROVE-IT
           MOVE    0                             TO    WS-OK
           IF    QAS-STATUS = "NOTOPEN" OR QAS-STATUS = "EXPIRED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

      *> Q4 is already marked by the sending; back-date the open revision 2 of Q5 and let the sweep find it.
           MOVE    WS-Q5                         TO    QGQ-ID
           MOVE    WS-Q5                         TO    QUO-ID
           OPEN    I-O QUOTE-FILE
           MOVE    WS-Q5                         TO    QUO-ID
           MOVE    2                             TO    QUO-REV
           READ    QUOTE-FILE
               INVALID KEY
                   DISPLAY "  backdate: revision 2 not found"
               NOT INVALID KEY
                   MOVE    20200101              TO    QUO-VALID-UNTIL
                   REWRITE QUOTE-RECORD
                   END-REWRITE
           END-READ
           CLOSE   QUOTE-FILE

           MOVE    "expire: the sweep marks the quote and counts it"  TO WS-CASE-NAME
           INITIALIZE QEX-REQ
           MOVE    "Maria Atendente"             TO    QEX-OPERATOR
           CALL    "QUOTE-EXPIRE" USING QEX-REQ RETURNING QEX-RES
           MOVE    0                             TO    WS-OK
           IF    QEY-STATUS = "OK" AND QEY-COUNT = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "expire: a second sweep finds nothing more"  TO WS-CASE-NAME
           CALL    "QUOTE-EXPIRE" USING QEX-REQ RETURNING QEX-RES
           MOVE    0                             TO    WS-OK
           IF    QEY-STATUS = "OK" AND QEY-COUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

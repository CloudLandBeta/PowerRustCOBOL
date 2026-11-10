       IDENTIFICATION DIVISION.
       PROGRAM-ID. DASH-TEST.
      *> Test of the Dashboard numbers: DASH-SUMMARY, with the programs that make the data it reads (CUSTOMER-SAVE,
      *> VEHICLE-SAVE, QUOTE-SAVE, QUOTE-SEND, QUOTE-APPROVE, PAYMENT-CONFIRM, INSTALLMENT-RUN, REFUND-REQUEST, QUEUE-UPDATE).
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding copies of the project's catalog.idx, faultrules.idx and
      *> sequences.idx.
      *> Cases: an empty system shows zeros and no lists (no figure without data); the cards of one currency (vehicles in
      *> service, quotes waiting, services running, vehicles ready, approval rate, total approved, average ticket); the same
      *> data in the other currency answers its own numbers and never mixes the two; the months; the most sold items; the
      *> payments (cash, installments, refunded); the approval channels; the failed installments; the dates to come, in
      *> order; a wrong currency refused.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-pay".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/payments.FD".
           COPY "COPYBOOKS/installments.FD".
           COPY "COPYBOOKS/refunds.FD".
           COPY "COPYBOOKS/queue.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/customers.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-pay".
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-quote".
           COPY "backend/COPYBOOKS/res-quote".
           COPY "backend/COPYBOOKS/req-quote-send".
           COPY "backend/COPYBOOKS/res-quote-send".
           COPY "backend/COPYBOOKS/req-quote-approve".
           COPY "backend/COPYBOOKS/res-quote-approve".
           COPY "backend/COPYBOOKS/req-payment".
           COPY "backend/COPYBOOKS/res-payment".
           COPY "backend/COPYBOOKS/req-inst-run".
           COPY "backend/COPYBOOKS/res-inst-run".
           COPY "backend/COPYBOOKS/req-refund".
           COPY "backend/COPYBOOKS/res-refund".
           COPY "backend/COPYBOOKS/req-queue-update".
           COPY "backend/COPYBOOKS/res-queue-update".
           COPY "backend/COPYBOOKS/req-dash".
           COPY "backend/COPYBOOKS/res-dash".
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).
       01 WS-PLATE-ARG                           PIC    X(8).
       01 WS-CUR-ARG                             PIC    X(3).
       01 WS-CASE-NAME                           PIC    X(64).
       01 WS-OK                                  PIC    9.
       01 WS-CUS                                 PIC    9(8).
       01 WS-QA                                  PIC    9(8).
       01 WS-QB                                  PIC    9(8).
       01 WS-QC                                  PIC    9(8).
       01 WS-QD                                  PIC    9(8).
       01 WS-QE                                  PIC    9(8).
       01 WS-QF                                  PIC    9(8).
       01 WS-QG                                  PIC    9(8).
       01 WS-TODAY                               PIC    9(8).
       01 WS-D30                                 PIC    9(8).
       01 WS-YM                                  PIC    9(6).
       01 WS-I                                   PIC    999.
       01 WS-Q4                                  PIC    9(8).
       01 WS-QUE-ID                              PIC    9(8).
       01 WS-EOF                                 PIC    X.
       01 WS-CODE                                PIC    X(6).
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-TICK                                PIC    9(8).
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-PH-A                                PIC    9(8).
       01 WS-PH-B                                PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).
       01 WS-RATE                                PIC    9(6).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-TODAY
           COMPUTE WS-D30 = FUNCTION DATE-OF-INTEGER ( FUNCTION INTEGER-OF-DATE ( WS-TODAY ) + 30 )
           COMPUTE WS-YM = FUNCTION INTEGER ( WS-TODAY / 100 )

           PERFORM TEST-EMPTY
           PERFORM MAKE-BASE-DATA
           ACCEPT  WS-TICK                       FROM  TIME
           COMPUTE WS-PH-A = WS-TICK - WS-T0

           PERFORM TEST-DASH
           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-PH-B = WS-T1 - WS-TICK
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0
           IF    WS-ELAPSED > 0
               COMPUTE WS-RATE = WS-CASES * 100 / WS-ELAPSED
           ELSE
               MOVE    WS-CASES                  TO    WS-RATE
           END-IF

           DISPLAY "Dashboard test (DASH-SUMMARY)"
           DISPLAY "  forms exercised  empty system; BRL cards, months, top items, payments, channels, failures, dates;"
           DISPLAY "                   EUR separately; wrong currency"
           DISPLAY "  setup            " WS-PH-A " hundredths of a second (7 quotes in two currencies, payments, queue)"
           DISPLAY "  dashboard cases  " WS-PH-B " hundredths of a second"
           DISPLAY "  cases run        " WS-CASES
           DISPLAY "  passed           " WS-PASS
           DISPLAY "  failed           " WS-FAIL
           DISPLAY "  throughput       " WS-RATE " cases per second"
           IF    WS-FAIL = 0
               DISPLAY "  result           PASS"
           ELSE
               DISPLAY "  result           FAIL"
           END-IF
           STOP RUN.

       TALLY.
           ADD     1                             TO    WS-CASES
           IF    WS-OK = 1
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
               DISPLAY "     " DSS-STATUS " " DSS-CURRENCY " vehicles " DSS-VEHICLES " awaiting " DSS-AWAITING
                       " running " DSS-RUNNING " ready " DSS-READY " sent " DSS-SENT " approved " DSS-APPROVED
                       " rate " DSS-RATE " total " DSS-APPROVED-TOTAL " ticket " DSS-TICKET
               DISPLAY "     pay " DSS-PAY-CASH "/" DSS-PAY-INST "/" DSS-PAY-REFUNDED " channels " DSS-APR-LINK "/"
                       DSS-APR-MANUAL " failed " DSS-FAILED "/" DSS-GAVE-UP " top " DSS-TOP-COUNT " dates " DSS-DL-COUNT
           END-IF.

       ASK-DASH.
           CALL    "DASH-SUMMARY" USING DSH-REQ RETURNING DSH-RES.

       TEST-EMPTY.
           MOVE    "empty: nothing to show, so no figures and no lists" TO WS-CASE-NAME
           INITIALIZE DSH-REQ
           MOVE    "BRL"                         TO    DSQ-CURRENCY
           PERFORM ASK-DASH
           MOVE    0                             TO    WS-OK
           IF    DSS-STATUS = "OK" AND DSS-VEHICLES = 0 AND DSS-AWAITING = 0 AND DSS-RUNNING = 0 AND DSS-READY = 0
                 AND DSS-SENT = 0 AND DSS-APPROVED = 0 AND DSS-RATE = 0 AND DSS-APPROVED-TOTAL = 0
                 AND DSS-TICKET = 0 AND DSS-TOP-COUNT = 0 AND DSS-DL-COUNT = 0 AND DSS-PAY-CASH = 0
                 AND DSS-FAILED = 0 AND DSM-YM (6) = WS-YM AND DSM-SENT (6) = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

      *> MAKE-BASE-DATA: BRL quotes A (paid in cash, service running), B (approved by the link, paid in 3, service done,
      *> an installment failed), C (sent, waiting), D (refused), E (open), F (approved and refunded); EUR quote G (paid in 2).
       MAKE-BASE-DATA.
           INITIALIZE CUS-REQ
           MOVE    "Cliente Painel"              TO    CRQ-NAME
           MOVE    "52998224725"                 TO    CRQ-DOC
           MOVE    "S"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-WA
           MOVE    "dois@exemplo.com.br"         TO    CRQ-EMAIL
           MOVE    "(11) 99999-0001"             TO    CRQ-WHATSAPP
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-CUS

           MOVE    "AAA1A11"                     TO    VRQ-PLATE
           MOVE    WS-CUS                        TO    VRQ-CUS-ID
           MOVE    "Fiat"                        TO    VRQ-BRAND
           MOVE    "Palio 1.0"                   TO    VRQ-MODEL
           MOVE    2014                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES

           MOVE    "AAA1A11"                     TO    WS-PLATE-ARG
           MOVE    "BRL"                         TO    WS-CUR-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QA
           PERFORM APPROVE-MANUAL
           MOVE    1                             TO    PCQ-INSTALLMENTS
           PERFORM PAY-IT

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QB
           PERFORM SEND-IT
           PERFORM APPROVE-LINK
           MOVE    3                             TO    PCQ-INSTALLMENTS
           PERFORM PAY-IT

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QC
           PERFORM SEND-IT

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QD
           PERFORM REFUSE-MANUAL

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QE

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QF
           PERFORM APPROVE-MANUAL
           MOVE    1                             TO    PCQ-INSTALLMENTS
           PERFORM PAY-IT
           MOVE    WS-QF                         TO    WS-Q4
           PERFORM GET-QUEUE
           INITIALIZE RFN-REQ
           MOVE    WS-QUE-ID                     TO    RFQ-QUE-ID
           MOVE    "Cliente Painel"              TO    RFQ-BY
           MOVE    "Desistiu"                    TO    RFQ-REASON
           CALL    "REFUND-REQUEST" USING RFN-REQ RETURNING RFN-RES

           MOVE    "EUR"                         TO    WS-CUR-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QG
           PERFORM APPROVE-MANUAL
           MOVE    2                             TO    PCQ-INSTALLMENTS
           PERFORM PAY-IT

           MOVE    WS-QA                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    WS-QUE-ID                     TO    QUQ-QUE-ID
           MOVE    "S"                           TO    QUQ-ACTION
           PERFORM UPDATE-IT

           MOVE    WS-QB                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    WS-QUE-ID                     TO    QUQ-QUE-ID
           MOVE    "S"                           TO    QUQ-ACTION
           PERFORM UPDATE-IT
           MOVE    "F"                           TO    QUQ-ACTION
           PERFORM UPDATE-IT

           INITIALIZE IRN-REQ
           MOVE    WS-D30                        TO    IRQ-ASOF
           MOVE    "D"                           TO    IRQ-GATEWAY
           CALL    "INSTALLMENT-RUN" USING IRN-REQ RETURNING IRN-RES.

       MAKE-QUOTE.
           INITIALIZE QUO-REQ
           MOVE    WS-PLATE-ARG                  TO    QRQ-PLATE
           MOVE    WS-CUR-ARG                    TO    QRQ-CURRENCY
           MOVE    "Maria Atendente"             TO    QRQ-OPERATOR
           MOVE    3                             TO    QRQ-COUNT
           MOVE    "P"                           TO    QRL-KIND (1)
           MOVE    "VELA"                        TO    QRL-CODE (1)
           MOVE    2                             TO    QRL-QTY (1)
           MOVE    "S"                           TO    QRL-KIND (2)
           MOVE    "SVC-IGNICAO"                 TO    QRL-CODE (2)
           MOVE    "A"                           TO    QRL-KIND (3)
           MOVE    "ADD-LAVAGEM"                 TO    QRL-CODE (3)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES.

       SEND-IT.
           INITIALIZE QSN-REQ
           MOVE    QTS-ID                        TO    QSN-ID
           MOVE    "Maria Atendente"             TO    QSN-OPERATOR
           CALL    "QUOTE-SEND" USING QSN-REQ RETURNING QSN-RES
           MOVE    QNS-CODE                      TO    WS-CODE.

       APPROVE-MANUAL.
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    QTS-ID                        TO    QAP-QUO-ID
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    "COUNTER"                     TO    QAP-CHANNEL
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES.

       REFUSE-MANUAL.
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    QTS-ID                        TO    QAP-QUO-ID
           MOVE    "R"                           TO    QAP-DECISION
           MOVE    "Preco alto"                  TO    QAP-REASON
           MOVE    "PHONE"                       TO    QAP-CHANNEL
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES.

       APPROVE-LINK.
           INITIALIZE QAP-REQ
           MOVE    "L"                           TO    QAP-MODE
           MOVE    QSN-ID                        TO    QAP-QUO-ID
           MOVE    WS-CODE                       TO    QAP-CODE
           MOVE    "A"                           TO    QAP-DECISION
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES.

      *> PAY-IT: charges the quote just made or approved (QAP-QUO-ID or QTS-ID) in PCQ-INSTALLMENTS installments.
       PAY-IT.
           MOVE    PCQ-INSTALLMENTS              TO    WS-I
           INITIALIZE PCF-REQ
           MOVE    WS-I                          TO    PCQ-INSTALLMENTS
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    QTS-ID                        TO    PCQ-QUO-ID
           MOVE    "S"                           TO    PCQ-CONSENT
           MOVE    "VISA"                        TO    PCQ-BRAND
           MOVE    "4242"                        TO    PCQ-LAST4
           MOVE    "Maria Atendente"             TO    PCQ-OPERATOR
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES.

       UPDATE-IT.
           MOVE    "Joao Mecanico"               TO    QUQ-OPERATOR
           MOVE    0                             TO    QUQ-QUO-ID
           MOVE    0                             TO    QUQ-DEADLINE
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES.

       GET-QUEUE.
           MOVE    0                             TO    WS-QUE-ID
           OPEN    INPUT QUEUE-FILE
           MOVE    WS-Q4                         TO    QUE-QUO-ID
           MOVE    "N"                           TO    WS-EOF
           START   QUEUE-FILE KEY = QUE-QUO-ID
               INVALID KEY
                   MOVE    "Y"                   TO    WS-EOF
           END-START
           IF    WS-EOF = "N"
               READ    QUEUE-FILE NEXT
                   AT END
                       CONTINUE
                   NOT AT END
                       MOVE    QUE-ID            TO    WS-QUE-ID
               END-READ
           END-IF
           CLOSE   QUEUE-FILE.

       TEST-DASH.
           MOVE    "BRL cards: vehicles, waiting, running, ready" TO WS-CASE-NAME
           INITIALIZE DSH-REQ
           MOVE    "BRL"                         TO    DSQ-CURRENCY
           PERFORM ASK-DASH
           MOVE    0                             TO    WS-OK
           IF    DSS-STATUS = "OK" AND DSS-VEHICLES = 1 AND DSS-AWAITING = 1 AND DSS-RUNNING = 1 AND DSS-READY = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL cards: sent, approved, rate, total and ticket" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-SENT = 5 AND DSS-APPROVED = 3 AND DSS-RATE = 60.0 AND DSS-APPROVED-TOTAL = 1857.30
                 AND DSS-TICKET = 619.10
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL months: this month has the 5 sent and 3 approved, the others none" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSM-YM (6) = WS-YM AND DSM-SENT (6) = 5 AND DSM-APPROVED (6) = 3 AND DSM-SENT (5) = 0
                 AND DSM-SENT (1) = 0 AND DSM-YM (5) < DSM-YM (6) AND DSM-YM (1) < DSM-YM (2)
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL most sold: the spark plugs, then the ignition service" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-TOP-COUNT = 2 AND DST-CODE (1) = "VELA" AND DST-QTY (1) = 6.00
                 AND DST-CODE (2) = "SVC-IGNICAO" AND DST-QTY (2) = 3.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL payments: one in cash, one in installments, one refunded" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-PAY-CASH = 1 AND DSS-PAY-INST = 1 AND DSS-PAY-REFUNDED = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL approval channels: one by the link, two by hand" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-APR-LINK = 1 AND DSS-APR-MANUAL = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL failed installments for the consultant" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-FAILED = 1 AND DSS-GAVE-UP = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL dates: a delivery and a pickup, in date order" TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    DSS-DL-COUNT = 2 AND DDL-DATE (1) NOT > DDL-DATE (2)
                 AND ( ( DDL-KIND (1) = "E" AND DDL-KIND (2) = "R" ) OR ( DDL-KIND (1) = "R" AND DDL-KIND (2) = "E" ) )
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "EUR: its own numbers, never added to BRL" TO WS-CASE-NAME
           MOVE    "EUR"                         TO    DSQ-CURRENCY
           PERFORM ASK-DASH
           MOVE    0                             TO    WS-OK
           IF    DSS-STATUS = "OK" AND DSS-CURRENCY = "EUR" AND DSS-SENT = 1 AND DSS-APPROVED = 1 AND DSS-RATE = 100.0
                 AND DSS-APPROVED-TOTAL = DSS-TICKET AND DSS-APPROVED-TOTAL NOT = 619.10
                 AND DSS-VEHICLES = 1 AND DSS-PAY-INST = 1 AND DSS-PAY-CASH = 0 AND DSS-FAILED = 1
                 AND DSS-APR-MANUAL = 1 AND DSS-APR-LINK = 0 AND DSS-AWAITING = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "wrong currency is refused" TO WS-CASE-NAME
           MOVE    "USD"                         TO    DSQ-CURRENCY
           PERFORM ASK-DASH
           MOVE    0                             TO    WS-OK
           IF    DSS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

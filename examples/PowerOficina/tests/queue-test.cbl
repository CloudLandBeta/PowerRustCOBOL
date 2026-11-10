       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUEUE-TEST.
      *> Test of the execution queue: QUEUE-LIST and QUEUE-UPDATE, with PAYMENT-CONFIRM and REFUND-REQUEST to fill it.
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding copies of the project's catalog.idx, faultrules.idx and
      *> sequences.idx.
      *> Cases: the queue lists what is waiting and running, ordered by priority and then by approval; a service starts
      *> and finishes in order and never skips or goes back; time and operator are written on the row; the order is ready
      *> for pickup only when its last service is done, and the customer is told by two channels; a refunded service
      *> leaves the list and does not hold the order back; the delivery date changes for what is still open, cannot be in
      *> the past, and the customer is told; the filters; invalid requests.

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
           COPY "backend/COPYBOOKS/req-quote-get".
           COPY "backend/COPYBOOKS/req-quote-approve".
           COPY "backend/COPYBOOKS/res-quote-approve".
           COPY "backend/COPYBOOKS/req-payment".
           COPY "backend/COPYBOOKS/res-payment".
           COPY "backend/COPYBOOKS/req-refund".
           COPY "backend/COPYBOOKS/res-refund".
           COPY "backend/COPYBOOKS/req-queue-list".
           COPY "backend/COPYBOOKS/res-queue-list".
           COPY "backend/COPYBOOKS/req-queue-update".
           COPY "backend/COPYBOOKS/res-queue-update".
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
       01 WS-A1                                  PIC    9(8).
       01 WS-A2                                  PIC    9(8).
       01 WS-B1                                  PIC    9(8).
       01 WS-C1                                  PIC    9(8).
       01 WS-TODAY                               PIC    9(8).
       01 WS-D10                                 PIC    9(8).
       01 WS-I                                   PIC    999.
       01 WS-QUE-ID                              PIC    9(8).
       01 WS-NEW-STATE                           PIC    X(10).
       01 WS-NTF-COUNT                           PIC    999.
       01 WS-WANT-KIND                           PIC    X(12).
       01 WS-WANT-STATUS                         PIC    X(8)   VALUE SPACES.
       01 WS-EOF                                 PIC    X.
       01 WS-QSTATE                              PIC    X(10).
       01 WS-Q4                                  PIC    9(8).
       01 WS-ROWQ                                PIC    9(8).
       01 WS-ROW-DEADLINE                        PIC    9(8).
       01 WS-STARTED                             PIC    9(14).
       01 WS-BY                                  PIC    X(30).
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
           COMPUTE WS-D10 = FUNCTION DATE-OF-INTEGER ( FUNCTION INTEGER-OF-DATE ( WS-TODAY ) + 10 )

           PERFORM MAKE-BASE-DATA
           ACCEPT  WS-TICK                       FROM  TIME
           COMPUTE WS-PH-A = WS-TICK - WS-T0

           PERFORM TEST-QUEUE
           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-PH-B = WS-T1 - WS-TICK
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0
           IF    WS-ELAPSED > 0
               COMPUTE WS-RATE = WS-CASES * 100 / WS-ELAPSED
           ELSE
               MOVE    WS-CASES                  TO    WS-RATE
           END-IF

           DISPLAY "Queue test (QUEUE-LIST, QUEUE-UPDATE)"
           DISPLAY "  forms exercised  order by priority and approval; start; finish; skip refused; ready for pickup;"
           DISPLAY "                   refunded service; delivery date; filters; invalid requests"
           DISPLAY "  setup            " WS-PH-A " hundredths of a second (customer, 3 paid orders, 4 services)"
           DISPLAY "  queue cases      " WS-PH-B " hundredths of a second"
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
               DISPLAY "     update " QUS-STATUS " " QUS-STATE " " QUS-READY " list " QLS-COUNT "/" QLS-QUEUED "/"
                       QLS-RUNNING "/" QLS-READY " notices " WS-NTF-COUNT
           END-IF.

      *> MAKE-BASE-DATA: order A has two services, B and C one each; all three are paid in one installment.
       MAKE-BASE-DATA.
           INITIALIZE CUS-REQ
           MOVE    "Cliente Fila"                TO    CRQ-NAME
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
           MOVE    4                             TO    QRQ-COUNT
           MOVE    "S"                           TO    QRL-KIND (4)
           MOVE    "SVC-FREIO"                   TO    QRL-CODE (4)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    QTS-ID                        TO    WS-QA
           PERFORM APPROVE-IT
           PERFORM PAY-ONE
           MOVE    WS-QA                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    WS-QUE-ID                     TO    WS-A1
           MOVE    WS-ROWQ                       TO    WS-A2

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QB
           PERFORM APPROVE-IT
           PERFORM PAY-ONE
           MOVE    WS-QB                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    WS-QUE-ID                     TO    WS-B1

           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QC
           PERFORM APPROVE-IT
           PERFORM PAY-ONE
           MOVE    WS-QC                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    WS-QUE-ID                     TO    WS-C1.

       PAY-ONE.
           INITIALIZE PCF-REQ
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    QTS-ID                        TO    PCQ-QUO-ID
           MOVE    1                             TO    PCQ-INSTALLMENTS
           MOVE    "VISA"                        TO    PCQ-BRAND
           MOVE    "4242"                        TO    PCQ-LAST4
           MOVE    "Maria Atendente"             TO    PCQ-OPERATOR
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES.

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

       APPROVE-IT.
           INITIALIZE QAP-REQ
           MOVE    "M"                           TO    QAP-MODE
           MOVE    QTS-ID                        TO    QAP-QUO-ID
           MOVE    "A"                           TO    QAP-DECISION
           MOVE    "COUNTER"                     TO    QAP-CHANNEL
           MOVE    "Maria Atendente"             TO    QAP-OPERATOR
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES.

       LIST-IT.
           MOVE    SPACES                        TO    QLQ-FILTER
           CALL    "QUEUE-LIST" USING QLN-REQ RETURNING QLN-RES.

       UPDATE-IT.
           MOVE    "Joao Mecanico"               TO    QUQ-OPERATOR
           MOVE    0                             TO    QUQ-QUO-ID
           MOVE    0                             TO    QUQ-DEADLINE
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES.

       DO-START.
           MOVE    "S"                           TO    QUQ-ACTION
           PERFORM UPDATE-IT.

       DO-FINISH.
           MOVE    "F"                           TO    QUQ-ACTION
           PERFORM UPDATE-IT.

       TEST-QUEUE.
           MOVE    "list: waiting and running services, in order of approval" TO WS-CASE-NAME
           PERFORM LIST-IT
           MOVE    0                             TO    WS-OK
           IF    QLS-STATUS = "OK" AND QLS-COUNT = 4 AND QLS-QUEUED = 4 AND QLS-RUNNING = 0 AND QLS-READY = 0
                 AND QLR-ID (1) = WS-A1 AND QLR-STATE (1) = "QUEUED" AND QLR-READY (1) = "N"
                 AND QLR-PLATE (1) = "AAA1A11"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "list: a lower priority number goes first" TO WS-CASE-NAME
           MOVE    WS-C1                         TO    WS-QUE-ID
           PERFORM SET-PRIORITY
           PERFORM LIST-IT
           MOVE    0                             TO    WS-OK
           IF    QLR-ID (1) = WS-C1 AND QLR-PRIORITY (1) = 10 AND QLR-ID (2) = WS-A1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "start: a queued service becomes running with time and operator" TO WS-CASE-NAME
           MOVE    WS-A1                         TO    QUQ-QUE-ID
           PERFORM DO-START
           MOVE    WS-A1                         TO    WS-QUE-ID
           PERFORM GET-ROW
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "OK" AND QUS-STATE = "RUNNING" AND WS-QSTATE = "RUNNING" AND WS-STARTED > 0
                 AND WS-BY = "Joao Mecanico"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "start: a running service cannot be started again" TO WS-CASE-NAME
           MOVE    WS-A1                         TO    QUQ-QUE-ID
           PERFORM DO-START
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "INVALID" AND QUS-STATE = "RUNNING"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "finish: a queued service cannot skip to done" TO WS-CASE-NAME
           MOVE    WS-A2                         TO    QUQ-QUE-ID
           PERFORM DO-FINISH
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "list: one running, the rest waiting" TO WS-CASE-NAME
           PERFORM LIST-IT
           MOVE    0                             TO    WS-OK
           IF    QLS-RUNNING = 1 AND QLS-QUEUED = 3
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "finish: the first service is done, the order is not ready yet" TO WS-CASE-NAME
           MOVE    WS-A1                         TO    QUQ-QUE-ID
           PERFORM DO-FINISH
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "OK" AND QUS-STATE = "DONE" AND QUS-READY = "N"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "ready: the last service done readies the vehicle and tells the customer" TO WS-CASE-NAME
           MOVE    "PRONTO"                      TO    WS-WANT-KIND
           PERFORM COUNT-NTF
           MOVE    WS-NTF-COUNT                  TO    WS-I
           MOVE    WS-A2                         TO    QUQ-QUE-ID
           PERFORM DO-START
           PERFORM DO-FINISH
           PERFORM COUNT-NTF
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "OK" AND QUS-READY = "Y" AND WS-I = 0 AND WS-NTF-COUNT = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "list: the ready vehicle is counted once, with its two services marked ready" TO WS-CASE-NAME
           MOVE    "ALL"                         TO    QLQ-FILTER
           CALL    "QUEUE-LIST" USING QLN-REQ RETURNING QLN-RES
           MOVE    0                             TO    WS-OK
           IF    QLS-COUNT = 4 AND QLS-READY = 1 AND QLS-QUEUED = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "a done service cannot be started or finished again" TO WS-CASE-NAME
           MOVE    WS-A1                         TO    QUQ-QUE-ID
           PERFORM DO-START
           MOVE    QUS-STATUS                    TO    WS-NEW-STATE
           PERFORM DO-FINISH
           MOVE    0                             TO    WS-OK
           IF    WS-NEW-STATE = "INVALID" AND QUS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refunded: the service leaves the waiting list and the filter finds it" TO WS-CASE-NAME
           INITIALIZE RFN-REQ
           MOVE    WS-B1                         TO    RFQ-QUE-ID
           MOVE    "Cliente Fila"                TO    RFQ-BY
           MOVE    "Cliente desistiu"            TO    RFQ-REASON
           CALL    "REFUND-REQUEST" USING RFN-REQ RETURNING RFN-RES
           PERFORM LIST-IT
           MOVE    "REFUNDED"                    TO    QLQ-FILTER
           MOVE    QLS-COUNT                     TO    WS-I
           CALL    "QUEUE-LIST" USING QLN-REQ RETURNING QLN-RES
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "OK" AND WS-I = 1 AND QLS-COUNT = 1 AND QLR-ID (1) = WS-B1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refunded: a refunded service cannot be started" TO WS-CASE-NAME
           MOVE    WS-B1                         TO    QUQ-QUE-ID
           PERFORM DO-START
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "deadline: a date in the past is refused" TO WS-CASE-NAME
           MOVE    "D"                           TO    QUQ-ACTION
           PERFORM DEADLINE-CALL
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "deadline: an open order gets the new date and the customer is told" TO WS-CASE-NAME
           MOVE    "PRAZO"                       TO    WS-WANT-KIND
           PERFORM COUNT-NTF
           MOVE    WS-NTF-COUNT                  TO    WS-I
           MOVE    WS-D10                        TO    QUQ-DEADLINE
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           PERFORM COUNT-NTF
           MOVE    WS-C1                         TO    WS-QUE-ID
           PERFORM GET-ROW
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "OK" AND QUS-CHANGED = 1 AND WS-ROW-DEADLINE = WS-D10 AND WS-NTF-COUNT = WS-I + 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "deadline: an order with nothing open has nothing to change" TO WS-CASE-NAME
           MOVE    WS-QA                         TO    QUQ-QUO-ID
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           MOVE    0                             TO    WS-OK
           IF    QUS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "invalid: no operator, unknown action, unknown service" TO WS-CASE-NAME
           MOVE    "S"                           TO    QUQ-ACTION
           MOVE    SPACES                        TO    QUQ-OPERATOR
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           MOVE    QUS-STATUS                    TO    WS-NEW-STATE
           MOVE    "X"                           TO    QUQ-ACTION
           MOVE    "Joao"                        TO    QUQ-OPERATOR
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           MOVE    QUS-STATUS                    TO    WS-QSTATE
           MOVE    "S"                           TO    QUQ-ACTION
           MOVE    99999999                      TO    QUQ-QUE-ID
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           MOVE    0                             TO    WS-OK
           IF    WS-NEW-STATE = "INVALID" AND WS-QSTATE = "INVALID" AND QUS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       DEADLINE-CALL.
           MOVE    "Joao Mecanico"               TO    QUQ-OPERATOR
           MOVE    WS-QC                         TO    QUQ-QUO-ID
           COMPUTE QUQ-DEADLINE = WS-TODAY - 1
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES.

      *> GET-QUEUE: the first queue row of quote WS-Q4 in WS-QUE-ID and the second in WS-ROWQ.
       GET-QUEUE.
           MOVE    0                             TO    WS-QUE-ID
           MOVE    0                             TO    WS-ROWQ
           OPEN    INPUT QUEUE-FILE
           MOVE    WS-Q4                         TO    QUE-QUO-ID
           MOVE    "N"                           TO    WS-EOF
           START   QUEUE-FILE KEY = QUE-QUO-ID
               INVALID KEY
                   MOVE    "Y"                   TO    WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ    QUEUE-FILE NEXT
                   AT END
                       MOVE    "Y"               TO    WS-EOF
                   NOT AT END
                       IF    QUE-QUO-ID NOT = WS-Q4
                           MOVE    "Y"           TO    WS-EOF
                       ELSE
                           IF    WS-QUE-ID = 0
                               MOVE    QUE-ID    TO    WS-QUE-ID
                           ELSE
                               MOVE    QUE-ID    TO    WS-ROWQ
                           END-IF
                       END-IF
               END-READ
           END-PERFORM
           CLOSE   QUEUE-FILE.

       GET-ROW.
           MOVE    SPACES                        TO    WS-QSTATE
           MOVE    0                             TO    WS-STARTED
           MOVE    SPACES                        TO    WS-BY
           MOVE    0                             TO    WS-ROW-DEADLINE
           OPEN    INPUT QUEUE-FILE
           MOVE    WS-QUE-ID                     TO    QUE-ID
           READ    QUEUE-FILE
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   MOVE    QUE-STATE             TO    WS-QSTATE
                   MOVE    QUE-STARTED           TO    WS-STARTED
                   MOVE    QUE-BY                TO    WS-BY
                   MOVE    QUE-DEADLINE          TO    WS-ROW-DEADLINE
           END-READ
           CLOSE   QUEUE-FILE.

       SET-PRIORITY.
           OPEN    I-O QUEUE-FILE
           MOVE    WS-QUE-ID                     TO    QUE-ID
           READ    QUEUE-FILE
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   MOVE    10                    TO    QUE-PRIORITY
                   REWRITE QUEUE-RECORD
                   END-REWRITE
           END-READ
           CLOSE   QUEUE-FILE.

       COUNT-NTF.
           MOVE    0                             TO    WS-NTF-COUNT
           OPEN    INPUT NOTIFY-FILE
           MOVE    0                             TO    NTF-ID
           MOVE    "N"                           TO    WS-EOF
           START   NOTIFY-FILE KEY NOT < NTF-ID
               INVALID KEY
                   MOVE    "Y"                   TO    WS-EOF
           END-START
           PERFORM UNTIL WS-EOF = "Y"
               READ    NOTIFY-FILE NEXT
                   AT END
                       MOVE    "Y"               TO    WS-EOF
                   NOT AT END
                       IF    NTF-KIND = WS-WANT-KIND AND (WS-WANT-STATUS = SPACES OR NTF-STATUS = WS-WANT-STATUS)
                           ADD     1             TO    WS-NTF-COUNT
                       END-IF
               END-READ
           END-PERFORM
           CLOSE   NOTIFY-FILE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PAYMENT-TEST.
      *> Test of the payment phase: PAYMENT-CONFIRM, INSTALLMENT-RUN and REFUND-REQUEST, with CUSTOMER-SAVE, VEHICLE-SAVE,
      *> QUOTE-SAVE, QUOTE-GET and QUOTE-APPROVE to make approved quotes.
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding copies of the project's catalog.idx, faultrules.idx and
      *> sequences.idx.
      *> Cases: the schedule of 3 and 12 installments (equal, the leftover cent in the first, dates 30 days apart, total
      *> equal to the quote); 0 and 13 installments refused; several installments without consent refused; an open or
      *> unknown quote refused; a declined charge and a silent gateway leave no payment, the retry pays; the same quote is
      *> never charged twice; the queue gets one row per service; a payment in EUR keeps EUR; recurring charges: nothing
      *> before the due date, a declined charge is told to the customer by two channels, retried at most 3 times, the
      *> retry that succeeds pays, a paid installment is never charged again, the payment is closed with its last one;
      *> refunds: a queued service is refunded and leaves the queue, running and done services are refused with the
      *> reason, a refund of future installments needs no return, a refund of what was paid is returned through the
      *> gateway, a declined return changes nothing, and the payment closes when every service is refunded.

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
           COPY "backend/COPYBOOKS/req-inst-run".
           COPY "backend/COPYBOOKS/res-inst-run".
           COPY "backend/COPYBOOKS/req-refund".
           COPY "backend/COPYBOOKS/res-refund".
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).
       01 WS-PLATE-ARG                           PIC    X(8).
       01 WS-CUR-ARG                             PIC    X(3).
       01 WS-CASE-NAME                           PIC    X(64).
       01 WS-OK                                  PIC    9.
       01 WS-CUS                                 PIC    9(8).
       01 WS-Q1                                  PIC    9(8).
       01 WS-Q2                                  PIC    9(8).
       01 WS-Q3                                  PIC    9(8).
       01 WS-Q4                                  PIC    9(8).
       01 WS-Q5                                  PIC    9(8).
       01 WS-Q6                                  PIC    9(8).
       01 WS-Q7                                  PIC    9(8).
       01 WS-Q8                                  PIC    9(8).
       01 WS-PAY8                                PIC    9(8).
       01 WS-QE                                  PIC    9(8).
       01 WS-PAY1                                PIC    9(8).
       01 WS-PAY3                                PIC    9(8).
       01 WS-PAY2                                PIC    9(8).
       01 WS-PAY4                                PIC    9(8).
       01 WS-TODAY                               PIC    9(8).
       01 WS-D30                                 PIC    9(8).
       01 WS-D60                                 PIC    9(8).
       01 WS-I                                   PIC    999.
       01 WS-SUM                                 PIC    9(9)V99.
       01 WS-QUE-ID                              PIC    9(8).
       01 WS-NEW-STATE                           PIC    X(10).
       01 WS-NTF-COUNT                           PIC    999.
       01 WS-WANT-KIND                           PIC    X(12).
       01 WS-WANT-STATUS                         PIC    X(8)   VALUE SPACES.
       01 WS-CUS2                                PIC    9(8).
       01 WS-EOF                                 PIC    X.
       01 WS-PSTAT                               PIC    X(10).
       01 WS-ISTAT                               PIC    X(10).
       01 WS-IAMT                                PIC    9(9)V99.
       01 WS-WANT-PAY                            PIC    9(8).
       01 WS-WANT-NO                             PIC    99.
       01 WS-QSTATE                              PIC    X(10).
       01 WS-SVC-VALUE                           PIC    9(9)V99.
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-TICK                                PIC    9(8).
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-PH-A                                PIC    9(8).
       01 WS-PH-B                                PIC    9(8).
       01 WS-PH-C                                PIC    9(8).
       01 WS-PH-D                                PIC    9(8).
       01 WS-PH-E                                PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).
       01 WS-RATE                                PIC    9(6).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-TODAY
           COMPUTE WS-D30 = FUNCTION DATE-OF-INTEGER ( FUNCTION INTEGER-OF-DATE ( WS-TODAY ) + 30 )
           COMPUTE WS-D60 = FUNCTION DATE-OF-INTEGER ( FUNCTION INTEGER-OF-DATE ( WS-TODAY ) + 60 )

           PERFORM MAKE-BASE-DATA
           ACCEPT  WS-TICK                       FROM  TIME
           COMPUTE WS-PH-A = WS-TICK - WS-T0

           PERFORM TEST-SCHEDULE
           PERFORM TEST-REFUSALS
           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-PH-B = WS-T1 - WS-TICK

           PERFORM TEST-PAYING
           ACCEPT  WS-TICK                       FROM  TIME
           COMPUTE WS-PH-C = WS-TICK - WS-T1

           PERFORM TEST-RECURRING
           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-PH-D = WS-T1 - WS-TICK

           PERFORM TEST-REFUNDS
           ACCEPT  WS-TICK                       FROM  TIME
           COMPUTE WS-PH-E = WS-TICK - WS-T1
           COMPUTE WS-ELAPSED = WS-TICK - WS-T0
           IF    WS-ELAPSED > 0
               COMPUTE WS-RATE = WS-CASES * 100 / WS-ELAPSED
           ELSE
               MOVE    WS-CASES                  TO    WS-RATE
           END-IF

           DISPLAY "Payment test (PAYMENT-CONFIRM, INSTALLMENT-RUN, REFUND-REQUEST)"
           DISPLAY "  forms exercised  schedule of 3 and 12; 0 and 13 refused; no consent; open and unknown quote; decline;"
           DISPLAY "                   silence; retry; duplicate; EUR; recurring charges; 3 tries; refunds in every state"
           DISPLAY "  setup            " WS-PH-A " hundredths of a second (customer, vehicle, 8 approved quotes)"
           DISPLAY "  schedule/refused " WS-PH-B " hundredths of a second"
           DISPLAY "  paying           " WS-PH-C " hundredths of a second"
           DISPLAY "  recurring        " WS-PH-D " hundredths of a second"
           DISPLAY "  refunds          " WS-PH-E " hundredths of a second"
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
               DISPLAY "     pay " PCS-STATUS " " PCS-CURRENCY " " PCS-TOTAL " run " IRS-CHARGED "/" IRS-FAILED
                       " refund " RFS-STATUS " notices " WS-NTF-COUNT
           END-IF.

      *> MAKE-BASE-DATA: one customer with one vehicle, and approved quotes (Q1 to Q4, Q6, QE) and one that is not (Q5).
       MAKE-BASE-DATA.
           INITIALIZE CUS-REQ
           MOVE    "Cliente Pagador"             TO    CRQ-NAME
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
           MOVE    QTS-ID                        TO    WS-Q1
           PERFORM APPROVE-IT
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q2
           PERFORM APPROVE-IT
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q3
           PERFORM APPROVE-IT
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q7
           PERFORM APPROVE-IT
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q5
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q6
           PERFORM APPROVE-IT
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-Q8
           PERFORM APPROVE-IT
           MOVE    "EUR"                         TO    WS-CUR-ARG
           PERFORM MAKE-QUOTE
           MOVE    QTS-ID                        TO    WS-QE
           PERFORM APPROVE-IT.

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

       PREVIEW.
           INITIALIZE PCF-REQ
           MOVE    "P"                           TO    PCQ-MODE
           MOVE    WS-QE                         TO    PCQ-QUO-ID
           MOVE    3                             TO    PCQ-INSTALLMENTS
           MOVE    "S"                           TO    PCQ-CONSENT
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES.

      *> PAY-IT: fills a charge request (mode C, 3 installments, consent, VISA 4242); the case sets the rest and calls.
       PAY-IT.
           INITIALIZE PCF-REQ
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    WS-QE                         TO    PCQ-QUO-ID
           MOVE    3                             TO    PCQ-INSTALLMENTS
           MOVE    "S"                           TO    PCQ-CONSENT
           MOVE    "VISA"                        TO    PCQ-BRAND
           MOVE    "4242"                        TO    PCQ-LAST4
           MOVE    "Maria Atendente"             TO    PCQ-OPERATOR.

       TEST-SCHEDULE.
           MOVE    "schedule: 3 installments, leftover cent first, 30 days apart" TO WS-CASE-NAME
           PERFORM PREVIEW
           MOVE    WS-Q1                         TO    PCQ-QUO-ID
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-N = 3 AND PCS-TOTAL = 619.10
                 AND PCI-AMOUNT (1) = 206.38 AND PCI-AMOUNT (2) = 206.36 AND PCI-AMOUNT (3) = 206.36
                 AND PCI-DUE (1) = WS-TODAY AND PCI-DUE (2) = WS-D30 AND PCI-DUE (3) = WS-D60
                 AND PCS-PAY-ID = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "schedule: 12 installments add up to the quote" TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    PCQ-QUO-ID
           MOVE    12                            TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 12
               ADD     PCI-AMOUNT (WS-I)         TO    WS-SUM
           END-PERFORM
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-N = 12 AND WS-SUM = PCS-TOTAL AND PCI-AMOUNT (1) = 51.61
                 AND PCI-AMOUNT (12) = 51.59
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "schedule: the preview writes nothing, a second one answers the same" TO WS-CASE-NAME
           MOVE    3                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-PAY-ID = 0 AND PCI-AMOUNT (1) = 206.38
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-REFUSALS.
           MOVE    "refused: 13 installments" TO WS-CASE-NAME
           PERFORM PREVIEW
           MOVE    WS-Q1                         TO    PCQ-QUO-ID
           MOVE    13                            TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "INVALID" AND PCS-FIELD = "INSTALL"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refused: 0 installments" TO WS-CASE-NAME
           MOVE    0                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "INVALID" AND PCS-FIELD = "INSTALL"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refused: several installments without the consent" TO WS-CASE-NAME
           MOVE    3                             TO    PCQ-INSTALLMENTS
           MOVE    "N"                           TO    PCQ-CONSENT
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "INVALID" AND PCS-FIELD = "CONSENT"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "accepted: 1 installment without the consent" TO WS-CASE-NAME
           MOVE    1                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCI-AMOUNT (1) = 619.10
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refused: a quote that was not approved" TO WS-CASE-NAME
           MOVE    WS-Q5                         TO    PCQ-QUO-ID
           MOVE    3                             TO    PCQ-INSTALLMENTS
           MOVE    "S"                           TO    PCQ-CONSENT
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "UNAPPROVED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refused: an unknown quote" TO WS-CASE-NAME
           MOVE    99999999                      TO    PCQ-QUO-ID
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refused: no operator and no last four digits on a charge" TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    PCQ-QUO-ID
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    "VISA"                        TO    PCQ-BRAND
           MOVE    "ab12"                        TO    PCQ-LAST4
           MOVE    "Maria"                       TO    PCQ-OPERATOR
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "INVALID" AND PCS-FIELD = "LAST4"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-PAYING.
           MOVE    "gateway declines: no payment, the quote can be paid again" TO WS-CASE-NAME
           PERFORM PAY-IT
           MOVE    WS-Q1                         TO    PCQ-QUO-ID
           MOVE    "D"                           TO    PCQ-GATEWAY
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "DECLINED" AND PCS-PAY-ID = 0 AND PCS-QUEUED = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "gateway silent: no payment" TO WS-CASE-NAME
           MOVE    "T"                           TO    PCQ-GATEWAY
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "TIMEOUT" AND PCS-PAY-ID = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "retry pays: payment active, first installment paid, service queued" TO WS-CASE-NAME
           MOVE    SPACES                        TO    PCQ-GATEWAY
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCS-PAY-ID                    TO    WS-PAY1
           MOVE    PCS-PAY-ID                    TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-PAY-ID > 0 AND PCS-QUEUED = 1 AND PCI-STATUS (1) = "PAID"
                 AND PCI-STATUS (2) = "DUE" AND WS-PSTAT = "ACTIVE" AND PCS-GW-REF NOT = SPACES
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "never twice: the same quote answers the payment that exists" TO WS-CASE-NAME
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "ALREADY" AND PCS-PAY-ID = WS-PAY1 AND PCI-STATUS (1) = "PAID"
                 AND PCI-STATUS (2) = "DUE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "the queue has the service of the quote, queued" TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    0                             TO    WS-OK
           IF    WS-QUE-ID > 0 AND WS-QSTATE = "QUEUED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "one installment, no consent: payment paid at once" TO WS-CASE-NAME
           MOVE    WS-Q2                         TO    PCQ-QUO-ID
           MOVE    1                             TO    PCQ-INSTALLMENTS
           MOVE    "N"                           TO    PCQ-CONSENT
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCS-PAY-ID                    TO    WS-PAY2
           MOVE    PCS-PAY-ID                    TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-N = 1 AND WS-PSTAT = "PAID" AND PCI-AMOUNT (1) = 619.10
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "payment in EUR keeps EUR and the total of the quote" TO WS-CASE-NAME
           MOVE    WS-QE                         TO    PCQ-QUO-ID
           MOVE    2                             TO    PCQ-INSTALLMENTS
           MOVE    "S"                           TO    PCQ-CONSENT
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCI-AMOUNT (1)                TO    WS-SUM
           ADD     PCI-AMOUNT (2)                TO    WS-SUM
           MOVE    WS-QE                         TO    QGQ-ID
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND PCS-CURRENCY = "EUR" AND WS-SUM = QTS-TOTAL
                 AND PCS-TOTAL = QTS-TOTAL
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "installments for the recurring cases (Q3 in 3, Q6 in 2)" TO WS-CASE-NAME
           MOVE    WS-Q3                         TO    PCQ-QUO-ID
           MOVE    3                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCS-PAY-ID                    TO    WS-PAY3
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK"
               MOVE    1                         TO    WS-OK
           END-IF
           MOVE    WS-Q6                         TO    PCQ-QUO-ID
           MOVE    2                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCS-PAY-ID                    TO    WS-PAY4
           IF    PCS-STATUS NOT = "OK"
               MOVE    0                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       RUN-IT.
           CALL    "INSTALLMENT-RUN" USING IRN-REQ RETURNING IRN-RES.

       TEST-RECURRING.
           MOVE    "recurring: nothing is charged before the due date" TO WS-CASE-NAME
           INITIALIZE IRN-REQ
           MOVE    WS-TODAY                      TO    IRQ-ASOF
           PERFORM RUN-IT
           MOVE    0                             TO    WS-OK
           IF    IRS-STATUS = "OK" AND IRS-CHARGED = 0 AND IRS-FAILED = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "recurring: declined charges fail, are counted, the customer is told" TO WS-CASE-NAME
           MOVE    "PARCELA"                     TO    WS-WANT-KIND
           PERFORM COUNT-NTF
           MOVE    WS-NTF-COUNT                  TO    WS-I
           MOVE    WS-D30                        TO    IRQ-ASOF
           MOVE    "D"                           TO    IRQ-GATEWAY
           PERFORM RUN-IT
           PERFORM COUNT-NTF
           MOVE    0                             TO    WS-OK
           IF    IRS-CHARGED = 0 AND IRS-FAILED = 4 AND WS-NTF-COUNT = WS-I + 8
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "recurring: the retry that succeeds pays, with no new card" TO WS-CASE-NAME
           MOVE    SPACES                        TO    IRQ-GATEWAY
           PERFORM RUN-IT
           MOVE    WS-PAY3                       TO    WS-WANT-PAY
           MOVE    2                             TO    WS-WANT-NO
           PERFORM GET-INS
           MOVE    0                             TO    WS-OK
           IF    IRS-CHARGED = 4 AND IRS-FAILED = 0 AND WS-ISTAT = "PAID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "recurring: a paid installment is never charged again" TO WS-CASE-NAME
           PERFORM RUN-IT
           MOVE    0                             TO    WS-OK
           IF    IRS-CHARGED = 0 AND IRS-FAILED = 0 AND IRS-GAVE-UP = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "recurring: the last installment closes the payment" TO WS-CASE-NAME
           MOVE    WS-D60                        TO    IRQ-ASOF
           PERFORM RUN-IT
           MOVE    WS-PAY3                       TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    IRS-CHARGED = 2 AND WS-PSTAT = "PAID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "recurring: 3 failed tries, then it is given up and never charged" TO WS-CASE-NAME
           MOVE    WS-Q7                         TO    PCQ-QUO-ID
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    2                             TO    PCQ-INSTALLMENTS
           MOVE    SPACES                        TO    PCQ-GATEWAY
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    WS-D30                        TO    IRQ-ASOF
           MOVE    "D"                           TO    IRQ-GATEWAY
           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               PERFORM RUN-IT
               ADD     IRS-FAILED                TO    WS-SUM
           END-PERFORM
           PERFORM RUN-IT
           MOVE    IRS-GAVE-UP                   TO    WS-I
           MOVE    SPACES                        TO    IRQ-GATEWAY
           PERFORM RUN-IT
           MOVE    0                             TO    WS-OK
           IF    WS-SUM = 3 AND WS-I = 1 AND IRS-CHARGED = 0 AND IRS-GAVE-UP = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-RECEIPTS.
           MOVE    "receipt: sent by both channels with the payment (2 per payment)" TO WS-CASE-NAME
           MOVE    "RECIBO"                      TO    WS-WANT-KIND
           MOVE    "SENT"                        TO    WS-WANT-STATUS
           PERFORM COUNT-NTF
           MOVE    0                             TO    WS-OK
           IF    WS-NTF-COUNT = 14
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "receipt: a customer without consent gets no message (skipped)" TO WS-CASE-NAME
           INITIALIZE CUS-REQ
           MOVE    "Cliente Sem Consentimento"   TO    CRQ-NAME
           MOVE    "11144477735"                 TO    CRQ-DOC
           MOVE    "N"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-CUS2
           MOVE    "BBB2B22"                     TO    VRQ-PLATE
           MOVE    WS-CUS2                       TO    VRQ-CUS-ID
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES
           MOVE    "BBB2B22"                     TO    WS-PLATE-ARG
           MOVE    "BRL"                         TO    WS-CUR-ARG
           PERFORM MAKE-QUOTE
           PERFORM APPROVE-IT
           PERFORM PAY-IT
           MOVE    QTS-ID                        TO    PCQ-QUO-ID
           MOVE    1                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    "SKIPPED"                     TO    WS-WANT-STATUS
           PERFORM COUNT-NTF
           MOVE    0                             TO    WS-OK
           IF    PCS-STATUS = "OK" AND WS-NTF-COUNT = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY
           MOVE    SPACES                        TO    WS-WANT-STATUS.

       TEST-REFUNDS.
           MOVE    WS-Q8                         TO    PCQ-QUO-ID
           MOVE    3                             TO    PCQ-INSTALLMENTS
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES
           MOVE    PCS-PAY-ID                    TO    WS-PAY8
           MOVE    WS-Q8                         TO    WS-Q4
           PERFORM GET-QUEUE

           MOVE    "refund refused: service running" TO WS-CASE-NAME
           MOVE    "RUNNING"                     TO    WS-NEW-STATE
           PERFORM SET-STATE
           PERFORM ASK-REFUND
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "REFUSED" AND RFS-QSTATE = "RUNNING" AND RFS-MESSAGE NOT = SPACES
                 AND RFS-AMOUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund refused: service done" TO WS-CASE-NAME
           MOVE    "DONE"                        TO    WS-NEW-STATE
           PERFORM SET-STATE
           PERFORM ASK-REFUND
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "REFUSED" AND RFS-QSTATE = "DONE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: the payment is kept after a refusal" TO WS-CASE-NAME
           MOVE    WS-PAY8                       TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    WS-PSTAT = "ACTIVE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: a queued service cancels the future installments, nothing to return" TO WS-CASE-NAME
           MOVE    "QUEUED"                      TO    WS-NEW-STATE
           PERFORM SET-STATE
           MOVE    WS-Q8                         TO    QGQ-ID
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           COMPUTE WS-SVC-VALUE = QTL-AMOUNT (2) + QTL-TAX (2)
           PERFORM ASK-REFUND
           MOVE    WS-PAY8                       TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "OK" AND RFS-AMOUNT = WS-SVC-VALUE AND RFS-CANCELLED = WS-SVC-VALUE
                 AND RFS-RETURNED = 0 AND RFS-QSTATE = "REFUNDED" AND WS-PSTAT = "REFUNDED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: the first installment stays paid, the last ones shrink" TO WS-CASE-NAME
           MOVE    WS-PAY8                       TO    WS-WANT-PAY
           MOVE    1                             TO    WS-WANT-NO
           PERFORM GET-INS
           MOVE    WS-IAMT                       TO    WS-SUM
           MOVE    2                             TO    WS-WANT-NO
           PERFORM GET-INS
           ADD     WS-IAMT                       TO    WS-SUM
           MOVE    3                             TO    WS-WANT-NO
           PERFORM GET-INS
           ADD     WS-IAMT                       TO    WS-SUM
           MOVE    0                             TO    WS-OK
           IF    WS-SUM = 619.10 - WS-SVC-VALUE
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: the same service cannot be refunded twice" TO WS-CASE-NAME
           PERFORM ASK-REFUND
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "REFUSED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: a declined return changes nothing" TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    WS-Q4
           PERFORM GET-QUEUE
           MOVE    "D"                           TO    RFQ-GATEWAY
           PERFORM ASK-REFUND
           MOVE    WS-QUE-ID                     TO    WS-I
           PERFORM GET-QUEUE
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "DECLINED" AND WS-QSTATE = "QUEUED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: what was already paid is returned by the gateway" TO WS-CASE-NAME
           MOVE    WS-Q1                         TO    QGQ-ID
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           COMPUTE WS-SVC-VALUE = QTL-AMOUNT (2) + QTL-TAX (2)
           MOVE    SPACES                        TO    RFQ-GATEWAY
           PERFORM ASK-REFUND
           MOVE    WS-PAY1                       TO    WS-WANT-PAY
           PERFORM GET-PAY-STATUS
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "OK" AND RFS-CANCELLED = 0 AND RFS-RETURNED = WS-SVC-VALUE
                 AND RFS-CURRENCY = "BRL" AND WS-PSTAT = "REFUNDED"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "refund: the customer is told by email and WhatsApp" TO WS-CASE-NAME
           MOVE    "ESTORNO"                     TO    WS-WANT-KIND
           PERFORM COUNT-NTF
           MOVE    0                             TO    WS-OK
           IF    WS-NTF-COUNT = 4
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           PERFORM TEST-RECEIPTS

           MOVE    "refund: the request needs who asked and why" TO WS-CASE-NAME
           MOVE    SPACES                        TO    RFQ-REASON
           PERFORM ASK-REFUND-RAW
           MOVE    0                             TO    WS-OK
           IF    RFS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       ASK-REFUND.
           MOVE    "Cliente Pagador"             TO    RFQ-BY
           MOVE    "Cliente desistiu do serviço" TO    RFQ-REASON
           PERFORM ASK-REFUND-RAW.

       ASK-REFUND-RAW.
           MOVE    WS-QUE-ID                     TO    RFQ-QUE-ID
           CALL    "REFUND-REQUEST" USING RFN-REQ RETURNING RFN-RES.

      *> GET-QUEUE: the queue row of quote WS-Q4: its number in WS-QUE-ID and its state in WS-QSTATE.
       GET-QUEUE.
           MOVE    0                             TO    WS-QUE-ID
           MOVE    SPACES                        TO    WS-QSTATE
           OPEN    INPUT QUEUE-FILE
           MOVE    WS-Q4                         TO    QUE-QUO-ID
           START   QUEUE-FILE KEY = QUE-QUO-ID
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   READ    QUEUE-FILE NEXT
                       AT END
                           CONTINUE
                       NOT AT END
                           MOVE    QUE-ID        TO    WS-QUE-ID
                           MOVE    QUE-STATE     TO    WS-QSTATE
                   END-READ
           END-START
           CLOSE   QUEUE-FILE.

       SET-STATE.
           OPEN    I-O QUEUE-FILE
           MOVE    WS-QUE-ID                     TO    QUE-ID
           READ    QUEUE-FILE
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   MOVE    WS-NEW-STATE          TO    QUE-STATE
                   REWRITE QUEUE-RECORD
                   END-REWRITE
           END-READ
           CLOSE   QUEUE-FILE.

       GET-PAY-STATUS.
           MOVE    SPACES                        TO    WS-PSTAT
           OPEN    INPUT PAYMENT-FILE
           MOVE    WS-WANT-PAY                   TO    PAY-ID
           READ    PAYMENT-FILE
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   MOVE    PAY-STATUS            TO    WS-PSTAT
           END-READ
           CLOSE   PAYMENT-FILE.

       GET-INS.
           MOVE    SPACES                        TO    WS-ISTAT
           MOVE    0                             TO    WS-IAMT
           OPEN    INPUT INSTALLMENT-FILE
           MOVE    WS-WANT-PAY                   TO    INS-PAY-ID
           MOVE    WS-WANT-NO                    TO    INS-NO
           READ    INSTALLMENT-FILE
               INVALID KEY
                   CONTINUE
               NOT INVALID KEY
                   MOVE    INS-STATUS            TO    WS-ISTAT
                   MOVE    INS-AMOUNT            TO    WS-IAMT
           END-READ
           CLOSE   INSTALLMENT-FILE.

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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. DASH-SUMMARY.
      *> The numbers of the Dashboard, for one currency (R52 to R55).
      *> - Cards: vehicles in service, quotes waiting for the customer, services running, vehicles ready for pickup, the
      *>   approval rate (approved over quotes sent and answered or expired), the total approved and the average ticket.
      *> - Blocks: quotes sent and approved by month (the last 6), the services and parts most sold, the payments (cash,
      *>   installments, refunded), the channels the approvals came by (link, manual), the next delivery and pickup dates, and
      *>   the installments that failed, for the consultant to see (R44c).
      *> - Only real data: a card with nothing behind it answers 0 and the screen shows no figure for it (R54).
      *> - Every value is of the one currency asked for; a quote, a payment or a total in the other currency is left out, and
      *>   nothing is ever converted or added across currencies (R55). The count of vehicles and services also follows the
      *>   currency of the quote they belong to, so BRL and EUR can be looked at one at a time.
      *> - The latest revision of each quote is the one that counts. A quote still ENVIADO past its validity counts as expired.
      *> The request and the answer are in backend/COPYBOOKS/req-dash.cpy and res-dash.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-pay".
           SELECT APPROVAL-FILE ASSIGN TO "data/approvals.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS APR-ID
               ALTERNATE RECORD KEY IS APR-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-APR.

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/payments.FD".
           COPY "COPYBOOKS/installments.FD".
           COPY "COPYBOOKS/refunds.FD".
           COPY "COPYBOOKS/queue.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/customers.FD".
           COPY "COPYBOOKS/approvals.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-pay".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-APR                              PIC    XX.
       01 WS-OPENED.
          05 WS-O-QUO                            PIC    9      VALUE 0.
          05 WS-O-QIT                            PIC    9      VALUE 0.
          05 WS-O-PAY                            PIC    9      VALUE 0.
          05 WS-O-INS                            PIC    9      VALUE 0.
          05 WS-O-QUE                            PIC    9      VALUE 0.
          05 WS-O-APR                            PIC    9      VALUE 0.
       01 WS-EOF                                 PIC    X.
       01 WS-I                                   PIC    999.
       01 WS-J                                   PIC    999.
       01 WS-K                                   PIC    999.
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).
       01 WS-TODAY                               PIC    9(8).
       01 WS-CUR                                 PIC    X(3).
       01 WS-NQ                                  PIC    999    VALUE 0.
       01 WS-QT.
          05 WS-Q                                OCCURS 300.
             10 WS-Q-ID                          PIC    9(8).
             10 WS-Q-REV                         PIC    99.
             10 WS-Q-PLATE                       PIC    X(8).
             10 WS-Q-CUR                         PIC    X(3).
             10 WS-Q-STATUS                      PIC    X(10).
             10 WS-Q-CREATED                     PIC    9(14).
             10 WS-Q-VALID                       PIC    9(8).
             10 WS-Q-TOTAL                       PIC    9(9)V99.
             10 WS-Q-OPEN                        PIC    999.
             10 WS-Q-DONE                        PIC    999.
             10 WS-Q-DEAD                        PIC    9(8).
       01 WS-FIND-ID                             PIC    9(8).
       01 WS-FOUND                               PIC    999.
       01 WS-YM                                  PIC    9(6).
       01 WS-MONTHS                              PIC    9(6).
       01 WS-NC                                  PIC    999    VALUE 0.
       01 WS-TC.
          05 WS-C                                OCCURS 80.
             10 WS-C-CODE                        PIC    X(12).
             10 WS-C-NAME                        PIC    X(40).
             10 WS-C-QTY                         PIC    9(6)V99.
             10 WS-C-USED                        PIC    9.
       01 WS-BEST                                PIC    999.
       01 WS-ND                                  PIC    999    VALUE 0.
       01 WS-DT.
          05 WS-D                                OCCURS 300.
             10 WS-D-DATE                        PIC    9(8).
             10 WS-D-QUO                         PIC    9(8).
             10 WS-D-PLATE                       PIC    X(8).
             10 WS-D-KIND                        PIC    X(1).
       01 WS-D-TMP.
          05 WS-DT-DATE                          PIC    9(8).
          05 WS-DT-QUO                           PIC    9(8).
          05 WS-DT-PLATE                         PIC    X(8).
          05 WS-DT-KIND                          PIC    X(1).
       01 WS-EFFECTIVE                           PIC    X(10).
       01 WS-SUMS                                PIC    9(11)V99.
       01 WS-STOP                                PIC    9.
       01 WS-MB                                  PIC    9(6).

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-dash".
           COPY "backend/COPYBOOKS/res-dash".

       PROCEDURE DIVISION USING DSH-REQ RETURNING DSH-RES.
       MAIN-PARA.
           INITIALIZE DSH-RES
           MOVE    "OK"                          TO    DSS-STATUS
           MOVE    DSQ-CURRENCY                  TO    WS-CUR
           MOVE    DSQ-CURRENCY                  TO    DSS-CURRENCY
           MOVE    0                             TO    WS-NQ
           MOVE    0                             TO    WS-NC
           MOVE    0                             TO    WS-ND
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-TODAY
           IF    DSQ-ASOF > 0
               MOVE    DSQ-ASOF                  TO    WS-TODAY
           END-IF

           IF    WS-CUR NOT = "BRL" AND WS-CUR NOT = "EUR"
               MOVE    "INVALID"                 TO    DSS-STATUS
               MOVE    "A moeda deve ser BRL ou EUR."
                                                 TO    DSS-MESSAGE
           ELSE
               PERFORM OPEN-ALL
               PERFORM LOAD-QUOTES
               PERFORM QUOTE-CARDS
               PERFORM MONTH-SERIES
               PERFORM READ-QUEUE
               PERFORM QUEUE-CARDS
               PERFORM TOP-ITEMS
               PERFORM PAYMENT-MIX
               PERFORM APPROVAL-CHANNELS
               PERFORM FAILED-INSTALLMENTS
               PERFORM CLOSE-ALL
           END-IF

           GOBACK.

       OPEN-ALL.
           MOVE    0                             TO    WS-O-QUO
           MOVE    0                             TO    WS-O-QIT
           MOVE    0                             TO    WS-O-PAY
           MOVE    0                             TO    WS-O-INS
           MOVE    0                             TO    WS-O-QUE
           MOVE    0                             TO    WS-O-APR
           OPEN    INPUT QUOTE-FILE
           IF    WS-FS-QUO = "00"
               MOVE    1                         TO    WS-O-QUO
           END-IF
           OPEN    INPUT QUOTE-ITEM-FILE
           IF    WS-FS-QIT = "00"
               MOVE    1                         TO    WS-O-QIT
           END-IF
           OPEN    INPUT PAYMENT-FILE
           IF    WS-FS-PAY = "00"
               MOVE    1                         TO    WS-O-PAY
           END-IF
           OPEN    INPUT INSTALLMENT-FILE
           IF    WS-FS-INS = "00"
               MOVE    1                         TO    WS-O-INS
           END-IF
           OPEN    INPUT QUEUE-FILE
           IF    WS-FS-QUE = "00"
               MOVE    1                         TO    WS-O-QUE
           END-IF
           OPEN    INPUT APPROVAL-FILE
           IF    WS-FS-APR = "00"
               MOVE    1                         TO    WS-O-APR
           END-IF.

       CLOSE-ALL.
           IF    WS-O-QUO = 1
               CLOSE   QUOTE-FILE
           END-IF
           IF    WS-O-QIT = 1
               CLOSE   QUOTE-ITEM-FILE
           END-IF
           IF    WS-O-PAY = 1
               CLOSE   PAYMENT-FILE
           END-IF
           IF    WS-O-INS = 1
               CLOSE   INSTALLMENT-FILE
           END-IF
           IF    WS-O-QUE = 1
               CLOSE   QUEUE-FILE
           END-IF
           IF    WS-O-APR = 1
               CLOSE   APPROVAL-FILE
           END-IF.

      *> LOAD-QUOTES: the latest revision of each quote (the file is in order of number and revision).
       LOAD-QUOTES.
           IF    WS-O-QUO = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-QUOTES
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-QUOTE-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       PERFORM STORE-QUOTE
                   END-IF
               END-PERFORM
           END-IF.

       STORE-QUOTE.
           IF    WS-NQ > 0 AND WS-Q-ID (WS-NQ) = QUO-ID
               MOVE    WS-NQ                     TO    WS-I
           ELSE
               IF    WS-NQ < 300
                   ADD     1                     TO    WS-NQ
                   MOVE    WS-NQ                 TO    WS-I
               ELSE
                   MOVE    0                     TO    WS-I
               END-IF
           END-IF

           IF    WS-I > 0
               MOVE    QUO-ID                    TO    WS-Q-ID (WS-I)
               MOVE    QUO-REV                   TO    WS-Q-REV (WS-I)
               MOVE    QUO-PLATE                 TO    WS-Q-PLATE (WS-I)
               MOVE    QUO-CURRENCY              TO    WS-Q-CUR (WS-I)
               MOVE    QUO-STATUS                TO    WS-Q-STATUS (WS-I)
               MOVE    QUO-CREATED               TO    WS-Q-CREATED (WS-I)
               MOVE    QUO-VALID-UNTIL           TO    WS-Q-VALID (WS-I)
               MOVE    QUO-TOTAL                 TO    WS-Q-TOTAL (WS-I)
               MOVE    QUO-DEADLINE              TO    WS-Q-DEAD (WS-I)
               MOVE    0                         TO    WS-Q-OPEN (WS-I)
               MOVE    0                         TO    WS-Q-DONE (WS-I)
           END-IF.

      *> EFFECTIVE-STATUS: the status of table row WS-I, with a sent quote past its validity counted as expired.
       EFFECTIVE-STATUS.
           MOVE    WS-Q-STATUS (WS-I)            TO    WS-EFFECTIVE
           IF    (WS-EFFECTIVE = "ENVIADO" OR WS-EFFECTIVE = "ABERTO") AND WS-Q-VALID (WS-I) < WS-TODAY
               MOVE    "EXPIRADO"                TO    WS-EFFECTIVE
           END-IF.

       QUOTE-CARDS.
           MOVE    0                             TO    WS-SUMS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-NQ
               IF    WS-Q-CUR (WS-I) = WS-CUR
                   PERFORM EFFECTIVE-STATUS
                   IF    WS-EFFECTIVE = "ENVIADO"
                       ADD     1                 TO    DSS-AWAITING
                   END-IF
                   IF    WS-EFFECTIVE NOT = "ABERTO"
                       ADD     1                 TO    DSS-SENT
                   END-IF
                   IF    WS-EFFECTIVE = "APROVADO"
                       ADD     1                 TO    DSS-APPROVED
                       ADD     WS-Q-TOTAL (WS-I) TO    WS-SUMS
                   END-IF
               END-IF
           END-PERFORM
           MOVE    WS-SUMS                       TO    DSS-APPROVED-TOTAL
           IF    DSS-SENT > 0
               COMPUTE DSS-RATE ROUNDED = DSS-APPROVED * 100 / DSS-SENT
           END-IF
           IF    DSS-APPROVED > 0
               COMPUTE DSS-TICKET ROUNDED = WS-SUMS / DSS-APPROVED
           END-IF.

      *> MONTH-SERIES: the last 6 months ending in the month of today, oldest first.
       MONTH-SERIES.
           COMPUTE WS-YM = FUNCTION INTEGER ( WS-TODAY / 100 )
           COMPUTE WS-MONTHS = FUNCTION INTEGER ( WS-YM / 100 ) * 12 + FUNCTION MOD ( WS-YM, 100 ) - 1
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 6
               COMPUTE WS-MB = WS-MONTHS - 6 + WS-K
               COMPUTE DSM-YM (WS-K) = FUNCTION INTEGER ( WS-MB / 12 ) * 100 + FUNCTION MOD ( WS-MB, 12 ) + 1
           END-PERFORM

           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-NQ
               IF    WS-Q-CUR (WS-I) = WS-CUR
                   PERFORM EFFECTIVE-STATUS
                   COMPUTE WS-YM = FUNCTION INTEGER ( WS-Q-CREATED (WS-I) / 100000000 )
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 6
                       IF    DSM-YM (WS-K) = WS-YM AND WS-EFFECTIVE NOT = "ABERTO"
                           ADD     1             TO    DSM-SENT (WS-K)
                           IF    WS-EFFECTIVE = "APROVADO"
                               ADD     1         TO    DSM-APPROVED (WS-K)
                           END-IF
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM.

      *> FIND-QUOTE: the table row of quote WS-FIND-ID in WS-FOUND (0 = not there).
       FIND-QUOTE.
           MOVE    0                             TO    WS-FOUND
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NQ OR WS-FOUND > 0
               IF    WS-Q-ID (WS-J) = WS-FIND-ID
                   MOVE    WS-J                  TO    WS-FOUND
               END-IF
           END-PERFORM.

      *> READ-QUEUE: counts the open and the done services of each quote.
       READ-QUEUE.
           IF    WS-O-QUE = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-QUEUE
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-QUEUE-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       MOVE    QUE-QUO-ID        TO    WS-FIND-ID
                       PERFORM FIND-QUOTE
                       IF    WS-FOUND > 0
                           IF    QUE-STATE = "QUEUED" OR QUE-STATE = "RUNNING"
                               ADD     1         TO    WS-Q-OPEN (WS-FOUND)
                               MOVE    QUE-DEADLINE
                                                 TO    WS-Q-DEAD (WS-FOUND)
                           END-IF
                           IF    QUE-STATE = "DONE"
                               ADD     1         TO    WS-Q-DONE (WS-FOUND)
                           END-IF
                           IF    QUE-STATE = "RUNNING" AND WS-Q-CUR (WS-FOUND) = WS-CUR
                               ADD     1         TO    DSS-RUNNING
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
           END-IF.

      *> QUEUE-CARDS: vehicles in service (an order with an open service), vehicles ready, and the dates to come.
       QUEUE-CARDS.
           MOVE    0                             TO    WS-ND
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-NQ
               IF    WS-Q-CUR (WS-I) = WS-CUR
                   IF    WS-Q-OPEN (WS-I) > 0
                       ADD     1                 TO    DSS-VEHICLES
                       ADD     1                 TO    WS-ND
                       MOVE    WS-Q-DEAD (WS-I)  TO    WS-D-DATE (WS-ND)
                       MOVE    WS-Q-ID (WS-I)    TO    WS-D-QUO (WS-ND)
                       MOVE    WS-Q-PLATE (WS-I) TO    WS-D-PLATE (WS-ND)
                       MOVE    "E"               TO    WS-D-KIND (WS-ND)
                   ELSE
                       IF    WS-Q-DONE (WS-I) > 0
                           ADD     1             TO    DSS-READY
                           ADD     1             TO    WS-ND
                           MOVE    WS-Q-DEAD (WS-I) TO WS-D-DATE (WS-ND)
                           MOVE    WS-Q-ID (WS-I) TO   WS-D-QUO (WS-ND)
                           MOVE    WS-Q-PLATE (WS-I) TO WS-D-PLATE (WS-ND)
                           MOVE    "R"           TO    WS-D-KIND (WS-ND)
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           PERFORM VARYING WS-I FROM 2 BY 1 UNTIL WS-I > WS-ND
               MOVE    WS-D (WS-I)               TO    WS-D-TMP
               MOVE    WS-I                      TO    WS-J
               MOVE    0                         TO    WS-STOP
               PERFORM UNTIL WS-J < 2 OR WS-STOP = 1
                   IF    WS-DT-DATE < WS-D-DATE (WS-J - 1)
                       MOVE    WS-D (WS-J - 1)   TO    WS-D (WS-J)
                       SUBTRACT 1                FROM  WS-J
                   ELSE
                       MOVE    1                 TO    WS-STOP
                   END-IF
               END-PERFORM
               MOVE    WS-D-TMP                  TO    WS-D (WS-J)
           END-PERFORM

           MOVE    0                             TO    DSS-DL-COUNT
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-ND OR WS-I > 8
               ADD     1                         TO    DSS-DL-COUNT
               MOVE    WS-D-DATE (WS-I)          TO    DDL-DATE (WS-I)
               MOVE    WS-D-QUO (WS-I)           TO    DDL-QUO (WS-I)
               MOVE    WS-D-PLATE (WS-I)         TO    DDL-PLATE (WS-I)
               MOVE    WS-D-KIND (WS-I)          TO    DDL-KIND (WS-I)
           END-PERFORM.

      *> TOP-ITEMS: the parts and services of approved quotes of this currency, by quantity sold.
       TOP-ITEMS.
           IF    WS-O-QIT = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-QUOTE-ITEMS
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-QUOTE-ITEM-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       PERFORM COUNT-ITEM
                   END-IF
               END-PERFORM
           END-IF
           PERFORM PICK-TOP.

       COUNT-ITEM.
           MOVE    QIT-QUO-ID                    TO    WS-FIND-ID
           PERFORM FIND-QUOTE
           IF    WS-FOUND > 0
               IF    WS-Q-REV (WS-FOUND) = QIT-REV AND WS-Q-CUR (WS-FOUND) = WS-CUR
                     AND WS-Q-STATUS (WS-FOUND) = "APROVADO" AND (QIT-KIND = "P" OR QIT-KIND = "S")
                   MOVE    0                     TO    WS-K
                   PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NC OR WS-K > 0
                       IF    WS-C-CODE (WS-J) = QIT-CODE
                           MOVE    WS-J          TO    WS-K
                       END-IF
                   END-PERFORM
                   IF    WS-K = 0 AND WS-NC < 80
                       ADD     1                 TO    WS-NC
                       MOVE    WS-NC             TO    WS-K
                       MOVE    QIT-CODE          TO    WS-C-CODE (WS-K)
                       MOVE    QIT-NAME          TO    WS-C-NAME (WS-K)
                       MOVE    0                 TO    WS-C-QTY (WS-K)
                       MOVE    0                 TO    WS-C-USED (WS-K)
                   END-IF
                   IF    WS-K > 0
                       IF    QIT-KIND = "S"
                           ADD     1             TO    WS-C-QTY (WS-K)
                       ELSE
                           ADD     QIT-QTY       TO    WS-C-QTY (WS-K)
                       END-IF
                   END-IF
               END-IF
           END-IF.

       PICK-TOP.
           MOVE    0                             TO    DSS-TOP-COUNT
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
               MOVE    0                         TO    WS-BEST
               PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-NC
                   IF    WS-C-USED (WS-J) = 0
                       IF    WS-BEST = 0
                           MOVE    WS-J          TO    WS-BEST
                       ELSE
                           IF    WS-C-QTY (WS-J) > WS-C-QTY (WS-BEST)
                               MOVE    WS-J      TO    WS-BEST
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
               IF    WS-BEST > 0
                   ADD     1                     TO    DSS-TOP-COUNT
                   MOVE    1                     TO    WS-C-USED (WS-BEST)
                   MOVE    WS-C-CODE (WS-BEST)   TO    DST-CODE (DSS-TOP-COUNT)
                   MOVE    WS-C-NAME (WS-BEST)   TO    DST-NAME (DSS-TOP-COUNT)
                   MOVE    WS-C-QTY (WS-BEST)    TO    DST-QTY (DSS-TOP-COUNT)
               END-IF
           END-PERFORM.

      *> PAYMENT-MIX: payments of this currency: paid at once, in installments, refunded.
       PAYMENT-MIX.
           IF    WS-O-PAY = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-PAYMENTS
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-PAYMENT-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    PAY-CURRENCY = WS-CUR
                           EVALUATE TRUE
                               WHEN PAY-STATUS = "REFUNDED"
                                   ADD     1     TO    DSS-PAY-REFUNDED
                               WHEN PAY-INSTALLMENTS > 1
                                   ADD     1     TO    DSS-PAY-INST
                               WHEN OTHER
                                   ADD     1     TO    DSS-PAY-CASH
                           END-EVALUATE
                       END-IF
                   END-IF
               END-PERFORM
           END-IF.

      *> APPROVAL-CHANNELS: approvals (not refusals) of quotes of this currency, by the link or registered by hand.
       APPROVAL-CHANNELS.
           IF    WS-O-APR = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-APPROVALS
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-APPROVAL-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    APR-DECISION = "A"
                           MOVE    APR-QUO-ID    TO    WS-FIND-ID
                           PERFORM FIND-QUOTE
                           IF    WS-FOUND > 0 AND WS-Q-CUR (WS-FOUND) = WS-CUR
                               IF    APR-CHANNEL = "LINK"
                                   ADD     1     TO    DSS-APR-LINK
                               ELSE
                                   ADD     1     TO    DSS-APR-MANUAL
                               END-IF
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
           END-IF.

      *> FAILED-INSTALLMENTS: installments the gateway declined, of payments in this currency; the ones with 3 tries are
      *> given up and need the consultant.
       FAILED-INSTALLMENTS.
           IF    WS-O-INS = 1 AND WS-O-PAY = 1
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-INSTALLMENTS
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-INSTALLMENT-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    INS-STATUS = "FAILED"
                           PERFORM COUNT-FAILED
                       END-IF
                   END-IF
               END-PERFORM
           END-IF.

       COUNT-FAILED.
           MOVE    INS-PAY-ID                    TO    PAY-ID
           PERFORM GET-PAYMENT-ROW
           IF    WS-PQ-STATUS = "00"
               IF    PAY-CURRENCY = WS-CUR
                   ADD     1                     TO    DSS-FAILED
                   IF    INS-TRIES >= 3
                       ADD     1                 TO    DSS-GAVE-UP
                   END-IF
               END-IF
           END-IF.

           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-payments".
           COPY "backend/COPYBOOKS/pq-installments".
           COPY "backend/COPYBOOKS/pq-queue".
           COPY "backend/COPYBOOKS/pq-approvals".

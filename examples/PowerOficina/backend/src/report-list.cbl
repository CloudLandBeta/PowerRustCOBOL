       IDENTIFICATION DIVISION.
       PROGRAM-ID. REPORT-LIST.
      *> The reports of the Reports screen (R50, R55).
      *> - Q: the quotes (latest revision of each) of one currency, with a status filter; a sent quote past its validity is
      *>   listed as EXPIRADO. P: the payments of one currency, with a status filter. F: the installments the gateway declined,
      *>   of one currency. N: the notices that were not delivered (FAILED) or not sent for lack of consent (SKIPPED).
      *> - A report of money is of one currency only: the total adds the rows of that currency and nothing else, and nothing
      *>   is converted (R41, R55). The first 40 rows are returned; RPS-TOTAL-ROWS says how many there are.
      *> The request and the answer are in backend/COPYBOOKS/req-report.cpy and res-report.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-pay".

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

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-pay".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-EOF                                 PIC    X.
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).
       01 WS-TODAY                               PIC    9(8).
       01 WS-EFFECTIVE                           PIC    X(10).
       01 WS-LAST.
          05 WS-L-ID                             PIC    9(8)   VALUE 0.
          05 WS-L-PLATE                          PIC    X(8).
          05 WS-L-CUR                            PIC    X(3).
          05 WS-L-STATUS                         PIC    X(10).
          05 WS-L-CREATED                        PIC    9(14).
          05 WS-L-VALID                          PIC    9(8).
          05 WS-L-TOTAL                          PIC    9(9)V99.
          05 WS-L-DEAD                           PIC    9(8).
       01 WS-HAVE                                PIC    9      VALUE 0.
       01 WS-ED-A                                PIC    ZZZ9.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-report".
           COPY "backend/COPYBOOKS/res-report".

       PROCEDURE DIVISION USING RPT-REQ RETURNING RPT-RES.
       MAIN-PARA.
           INITIALIZE RPT-RES
           MOVE    "OK"                          TO    RPS-STATUS
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-TODAY

           EVALUATE TRUE
               WHEN RPQ-MODE NOT = "Q" AND RPQ-MODE NOT = "P" AND RPQ-MODE NOT = "F" AND RPQ-MODE NOT = "N"
                   MOVE    "INVALID"             TO    RPS-STATUS
                   MOVE    "O relatório deve ser Q, P, F ou N."
                                                 TO    RPS-MESSAGE
               WHEN RPQ-MODE NOT = "N" AND RPQ-CURRENCY NOT = "BRL" AND RPQ-CURRENCY NOT = "EUR"
                   MOVE    "INVALID"             TO    RPS-STATUS
                   MOVE    "A moeda deve ser BRL ou EUR."
                                                 TO    RPS-MESSAGE
               WHEN OTHER
                   EVALUATE RPQ-MODE
                       WHEN "Q"
                           PERFORM REPORT-QUOTES
                       WHEN "P"
                           PERFORM REPORT-PAYMENTS
                       WHEN "F"
                           PERFORM REPORT-FAILED
                       WHEN OTHER
                           PERFORM REPORT-NOTICES
                   END-EVALUATE
           END-EVALUATE

           GOBACK.

      *> ADD-ROW: counts a row and keeps it while there is room; WS-ROW-N is the row just counted.
       ADD-ROW.
           ADD     1                             TO    RPS-TOTAL-ROWS
           IF    RPS-COUNT < 40
               ADD     1                         TO    RPS-COUNT
           END-IF.

       REPORT-QUOTES.
           OPEN    INPUT QUOTE-FILE
           IF    WS-FS-QUO = "00"
               MOVE    0                         TO    WS-HAVE
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
                       IF    WS-HAVE = 1 AND WS-L-ID NOT = QUO-ID
                           PERFORM EMIT-QUOTE
                       END-IF
                       MOVE    QUO-ID            TO    WS-L-ID
                       MOVE    QUO-PLATE         TO    WS-L-PLATE
                       MOVE    QUO-CURRENCY      TO    WS-L-CUR
                       MOVE    QUO-STATUS        TO    WS-L-STATUS
                       MOVE    QUO-CREATED       TO    WS-L-CREATED
                       MOVE    QUO-VALID-UNTIL   TO    WS-L-VALID
                       MOVE    QUO-TOTAL         TO    WS-L-TOTAL
                       MOVE    QUO-DEADLINE      TO    WS-L-DEAD
                       MOVE    1                 TO    WS-HAVE
                   END-IF
               END-PERFORM
               IF    WS-HAVE = 1
                   PERFORM EMIT-QUOTE
               END-IF
               CLOSE   QUOTE-FILE
           END-IF.

       EMIT-QUOTE.
           MOVE    WS-L-STATUS                   TO    WS-EFFECTIVE
           IF    (WS-EFFECTIVE = "ENVIADO" OR WS-EFFECTIVE = "ABERTO") AND WS-L-VALID < WS-TODAY
               MOVE    "EXPIRADO"                TO    WS-EFFECTIVE
           END-IF
           IF    WS-L-CUR = RPQ-CURRENCY AND (RPQ-FILTER = SPACES OR RPQ-FILTER = WS-EFFECTIVE)
               PERFORM ADD-ROW
               ADD     WS-L-TOTAL                TO    RPS-TOTAL
               IF    RPS-COUNT <= 40 AND RPS-TOTAL-ROWS = RPS-COUNT
                   MOVE    WS-L-ID               TO    RPL-ID (RPS-COUNT)
                   MOVE    WS-L-PLATE            TO    RPL-REF (RPS-COUNT)
                   MOVE    WS-EFFECTIVE          TO    RPL-STATUS (RPS-COUNT)
                   MOVE    WS-L-TOTAL            TO    RPL-AMOUNT (RPS-COUNT)
                   COMPUTE RPL-DATE (RPS-COUNT) = FUNCTION INTEGER ( WS-L-CREATED / 1000000 )
                   MOVE    "Orçamento"           TO    RPL-TEXT (RPS-COUNT)
               END-IF
           END-IF.

       REPORT-PAYMENTS.
           OPEN    INPUT PAYMENT-FILE
           IF    WS-FS-PAY = "00"
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
                       IF    PAY-CURRENCY = RPQ-CURRENCY AND (RPQ-FILTER = SPACES OR RPQ-FILTER = PAY-STATUS)
                           PERFORM ADD-ROW
                           ADD     PAY-TOTAL     TO    RPS-TOTAL
                           IF    RPS-TOTAL-ROWS = RPS-COUNT
                               MOVE    PAY-ID    TO    RPL-ID (RPS-COUNT)
                               MOVE    PAY-QUO-ID
                                                 TO    RPL-REF (RPS-COUNT)
                               MOVE    PAY-STATUS
                                                 TO    RPL-STATUS (RPS-COUNT)
                               MOVE    PAY-TOTAL TO    RPL-AMOUNT (RPS-COUNT)
                               COMPUTE RPL-DATE (RPS-COUNT) = FUNCTION INTEGER ( PAY-AT / 1000000 )
                               MOVE    PAY-INSTALLMENTS
                                                 TO    WS-ED-A
                               STRING    WS-ED-A "x " PAY-CARD-BRAND " final " PAY-CARD-LAST4
                                   INTO  RPL-TEXT (RPS-COUNT)
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
               CLOSE   PAYMENT-FILE
           END-IF.

       REPORT-FAILED.
           OPEN    INPUT PAYMENT-FILE
           OPEN    INPUT INSTALLMENT-FILE
           IF    WS-FS-PAY = "00" AND WS-FS-INS = "00"
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
                           PERFORM EMIT-FAILED
                       END-IF
                   END-IF
               END-PERFORM
           END-IF
      *> The PartiQL statements do not set FILE STATUS: WS-FS-PAY and WS-FS-INS hold what the OPENs answered.
           IF    WS-FS-PAY = "00" OR WS-FS-PAY = "10" OR WS-FS-PAY = "23"
               CLOSE   PAYMENT-FILE
           END-IF
           IF    WS-FS-INS = "00" OR WS-FS-INS = "10"
               CLOSE   INSTALLMENT-FILE
           END-IF.

       EMIT-FAILED.
           MOVE    INS-PAY-ID                    TO    PAY-ID
           PERFORM GET-PAYMENT-ROW
           IF    WS-PQ-STATUS = "00"
               IF    PAY-CURRENCY = RPQ-CURRENCY
                   PERFORM ADD-ROW
                   ADD     INS-AMOUNT            TO    RPS-TOTAL
                   IF    RPS-TOTAL-ROWS = RPS-COUNT
                       MOVE    INS-PAY-ID        TO    RPL-ID (RPS-COUNT)
                       MOVE    PAY-QUO-ID        TO    RPL-REF (RPS-COUNT)
                       MOVE    INS-STATUS        TO    RPL-STATUS (RPS-COUNT)
                       MOVE    INS-AMOUNT        TO    RPL-AMOUNT (RPS-COUNT)
                       MOVE    INS-DUE           TO    RPL-DATE (RPS-COUNT)
                       MOVE    INS-NO            TO    WS-ED-A
                       STRING    "Parcela " WS-ED-A " tentativas " INS-TRIES  INTO  RPL-TEXT (RPS-COUNT)
                   END-IF
               END-IF
           END-IF.

       REPORT-NOTICES.
           OPEN    INPUT NOTIFY-FILE
           IF    WS-FS-NTF = "00"
               MOVE    "N"                       TO    WS-EOF
               PERFORM SELECT-ALL-NOTICES
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM FETCH-NOTIFY-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    NTF-STATUS = "FAILED" OR NTF-STATUS = "SKIPPED"
                           PERFORM ADD-ROW
                           IF    RPS-TOTAL-ROWS = RPS-COUNT
                               MOVE    NTF-ID    TO    RPL-ID (RPS-COUNT)
                               MOVE    NTF-QUO-ID
                                                 TO    RPL-REF (RPS-COUNT)
                               MOVE    NTF-STATUS
                                                 TO    RPL-STATUS (RPS-COUNT)
                               MOVE    0         TO    RPL-AMOUNT (RPS-COUNT)
                               COMPUTE RPL-DATE (RPS-COUNT) = FUNCTION INTEGER ( NTF-AT / 1000000 )
                               STRING    NTF-CHANNEL " " NTF-KIND  INTO  RPL-TEXT (RPS-COUNT)
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
               CLOSE   NOTIFY-FILE
           END-IF.

           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-payments".
           COPY "backend/COPYBOOKS/pq-installments".
           COPY "backend/COPYBOOKS/pq-notifications".

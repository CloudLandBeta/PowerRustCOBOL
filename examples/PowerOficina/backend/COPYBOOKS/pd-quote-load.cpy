      *> A program that COPYs pd-quote-load also COPYs ws-partiql, pq-quotes and pq-quoteitems.
      *> FIND-LATEST-REV: the highest revision that exists of quote WS-LQ-ID, in WS-LQ-REV (0 = the quote does not exist).
       FIND-LATEST-REV.
           MOVE    0                             TO    WS-LQ-REV
           PERFORM VARYING WS-LQ-TRY FROM 1 BY 1 UNTIL WS-LQ-TRY > 99
               MOVE    WS-LQ-ID                  TO    QUO-ID
               MOVE    WS-LQ-TRY                 TO    QUO-REV
               PERFORM GET-QUOTE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    WS-LQ-TRY             TO    WS-LQ-REV
               ELSE
                   MOVE    100                   TO    WS-LQ-TRY
               END-IF
           END-PERFORM.

      *> LOAD-QUOTE: puts quote WS-LQ-ID, revision WS-LQ-REV (0 = the latest) into the QTS- items, with its lines.
      *> WS-LQ-OK = 1 when it was found. The quote files must be open.
       LOAD-QUOTE.
           MOVE    0                             TO    WS-LQ-OK
           IF    WS-LQ-REV = 0
               PERFORM FIND-LATEST-REV
           END-IF

           IF    WS-LQ-REV > 0
               MOVE    WS-LQ-ID                  TO    QUO-ID
               MOVE    WS-LQ-REV                 TO    QUO-REV
               PERFORM GET-QUOTE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    1                     TO    WS-LQ-OK
                   MOVE    QUO-ID                TO    QTS-ID
                   MOVE    QUO-REV               TO    QTS-REV
                   MOVE    QUO-CUS-ID            TO    QTS-CUS-ID
                   MOVE    QUO-PLATE             TO    QTS-PLATE
                   MOVE    QUO-DGN-ID            TO    QTS-DGN-ID
                   MOVE    QUO-CURRENCY          TO    QTS-CURRENCY
                   MOVE    QUO-STATUS            TO    QTS-QSTATUS
                   MOVE    QUO-CREATED           TO    QTS-CREATED
                   MOVE    QUO-VALID-UNTIL       TO    QTS-VALID-UNTIL
                   MOVE    QUO-DEADLINE          TO    QTS-DEADLINE
                   MOVE    QUO-HOURS             TO    QTS-HOURS
                   MOVE    QUO-SUBTOTAL          TO    QTS-SUBTOTAL
                   MOVE    QUO-TAX               TO    QTS-TAX
                   MOVE    QUO-TOTAL             TO    QTS-TOTAL
                   MOVE    QUO-OPERATOR          TO    QTS-OPERATOR
               END-IF
           END-IF

           IF    WS-LQ-OK = 1
               PERFORM LOAD-QUOTE-LINES
           END-IF.

       LOAD-QUOTE-LINES.
           MOVE    0                             TO    QTS-COUNT
           MOVE    WS-LQ-ID                      TO    QIT-QUO-ID
           MOVE    WS-LQ-REV                     TO    QIT-REV
           MOVE    "N"                           TO    WS-LQ-EOF
           PERFORM SELECT-LINES-OF-REVISION
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-LQ-EOF
           END-IF

           PERFORM UNTIL WS-LQ-EOF = "Y" OR QTS-COUNT = 20
               PERFORM FETCH-QUOTE-ITEM-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-LQ-EOF
               ELSE
                   IF    QIT-QUO-ID NOT = WS-LQ-ID OR QIT-REV NOT = WS-LQ-REV
                       MOVE    "Y"               TO    WS-LQ-EOF
                   ELSE
                       ADD     1                 TO    QTS-COUNT
                       MOVE    QIT-KIND          TO    QTL-KIND (QTS-COUNT)
                       MOVE    QIT-CODE          TO    QTL-CODE (QTS-COUNT)
                       MOVE    QIT-NAME          TO    QTL-NAME (QTS-COUNT)
                       MOVE    QIT-QTY           TO    QTL-QTY (QTS-COUNT)
                       MOVE    QIT-UNIT          TO    QTL-UNIT (QTS-COUNT)
                       MOVE    QIT-TAX-RATE      TO    QTL-TAX-RATE (QTS-COUNT)
                       MOVE    QIT-AMOUNT        TO    QTL-AMOUNT (QTS-COUNT)
                       MOVE    QIT-MANUAL        TO    QTL-MANUAL (QTS-COUNT)
                       MOVE    QIT-NOTE          TO    QTL-NOTE (QTS-COUNT)
                       COMPUTE QTL-TAX (QTS-COUNT) ROUNDED = QIT-AMOUNT * QIT-TAX-RATE / 100
                   END-IF
               END-IF
           END-PERFORM.

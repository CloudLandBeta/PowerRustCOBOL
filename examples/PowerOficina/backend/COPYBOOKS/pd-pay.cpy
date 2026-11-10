      *> A program that COPYs pd-pay also COPYs pd-common, ws-partiql, pq-quotes, pq-customers and pq-notifications.
      *> OPEN-PAY-FILES: opens the payment-phase files for update, creating an empty one the first time.
       OPEN-PAY-FILES.
           OPEN    I-O PAYMENT-FILE
           IF    WS-FS-PAY = "35"
               OPEN    OUTPUT PAYMENT-FILE
               CLOSE   PAYMENT-FILE
               OPEN    I-O PAYMENT-FILE
           END-IF

           OPEN    I-O INSTALLMENT-FILE
           IF    WS-FS-INS = "35"
               OPEN    OUTPUT INSTALLMENT-FILE
               CLOSE   INSTALLMENT-FILE
               OPEN    I-O INSTALLMENT-FILE
           END-IF

           OPEN    I-O REFUND-FILE
           IF    WS-FS-RFD = "35"
               OPEN    OUTPUT REFUND-FILE
               CLOSE   REFUND-FILE
               OPEN    I-O REFUND-FILE
           END-IF

           OPEN    I-O QUEUE-FILE
           IF    WS-FS-QUE = "35"
               OPEN    OUTPUT QUEUE-FILE
               CLOSE   QUEUE-FILE
               OPEN    I-O QUEUE-FILE
           END-IF

           OPEN    I-O NOTIFY-FILE
           IF    WS-FS-NTF = "35"
               OPEN    OUTPUT NOTIFY-FILE
               CLOSE   NOTIFY-FILE
               OPEN    I-O NOTIFY-FILE
           END-IF

           OPEN    INPUT CUSTOMER-FILE.

       CLOSE-PAY-FILES.
           CLOSE   PAYMENT-FILE
           CLOSE   INSTALLMENT-FILE
           CLOSE   REFUND-FILE
           CLOSE   QUEUE-FILE
           CLOSE   NOTIFY-FILE
           CLOSE   CUSTOMER-FILE.

      *> NOTIFY-BOTH: the customer of quote WS-NT-QUO is told by email and by WhatsApp (simulated, like the quote), but
      *> only by a channel the customer consented to and has the contact for (R34); the other is recorded as SKIPPED.
      *> In:  WS-NT-QUO = the quote, WS-NT-KIND = the kind of notice, WS-NT-TEXT = the message.
      *>      The quote file and the customer file must be open.
      *> Out: two NOTIFY-FILE records, one per channel, marked SENT or SKIPPED.
       NOTIFY-BOTH.
           MOVE    0                             TO    WS-NT-CUS
           MOVE    WS-NT-QUO                     TO    QUO-ID
           MOVE    1                             TO    QUO-REV
           PERFORM GET-QUOTE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    QUO-CUS-ID                TO    WS-NT-CUS
           END-IF

           MOVE    "N"                           TO    WS-NT-HAS
           MOVE    WS-NT-CUS                     TO    CUS-ID
           PERFORM GET-CUSTOMER-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    "Y"                       TO    WS-NT-HAS
           END-IF

           PERFORM VARYING WS-NT-CHAN FROM 1 BY 1 UNTIL WS-NT-CHAN > 2
               MOVE    "SKIPPED"                 TO    WS-NT-RESULT
               MOVE    "-"                       TO    WS-NT-ADDR
               IF    WS-NT-HAS = "Y"
                   IF    WS-NT-CHAN = 1 AND CUS-CONSENT-EMAIL = "S" AND CUS-EMAIL NOT = SPACES
                       MOVE    "SENT"            TO    WS-NT-RESULT
                       MOVE    CUS-EMAIL         TO    WS-NT-ADDR
                   END-IF
                   IF    WS-NT-CHAN = 2 AND CUS-CONSENT-WA = "S" AND CUS-WHATSAPP NOT = SPACES
                       MOVE    "SENT"            TO    WS-NT-RESULT
                       MOVE    CUS-WHATSAPP      TO    WS-NT-ADDR
                   END-IF
               END-IF

               MOVE    "NOTIFY"                  TO    WS-SEQ-WANT
               PERFORM NEXT-NUMBER
               INITIALIZE NOTIFY-RECORD
               MOVE    WS-SEQ-GOT                TO    NTF-ID
               MOVE    WS-NT-QUO                 TO    NTF-QUO-ID
               IF    WS-NT-CHAN = 1
                   MOVE    "EMAIL"               TO    NTF-CHANNEL
               ELSE
                   MOVE    "WHATSAPP"            TO    NTF-CHANNEL
               END-IF
               MOVE    WS-NT-KIND                TO    NTF-KIND
               MOVE    WS-NT-ADDR                TO    NTF-TO
               MOVE    WS-NOW                    TO    NTF-AT
               MOVE    WS-NT-RESULT              TO    NTF-STATUS
               MOVE    WS-NT-TEXT                TO    NTF-TEXT
               PERFORM INSERT-NOTIFY-ROW
           END-PERFORM.

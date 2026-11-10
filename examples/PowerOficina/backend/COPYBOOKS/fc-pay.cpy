      *> The files of the payment phase: payments, installments, refunds, the execution queue and the notices sent.
           SELECT PAYMENT-FILE ASSIGN TO "data/payments.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PAY-ID
               ALTERNATE RECORD KEY IS PAY-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-PAY.
           SELECT INSTALLMENT-FILE ASSIGN TO "data/installments.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS INS-KEY
               ALTERNATE RECORD KEY IS INS-DUE WITH DUPLICATES
               FILE STATUS IS WS-FS-INS.
           SELECT REFUND-FILE ASSIGN TO "data/refunds.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS RFD-ID
               ALTERNATE RECORD KEY IS RFD-PAY-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-RFD.
           SELECT QUEUE-FILE ASSIGN TO "data/queue.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QUE-ID
               ALTERNATE RECORD KEY IS QUE-QUO-ID WITH DUPLICATES
               ALTERNATE RECORD KEY IS QUE-STATE WITH DUPLICATES
               FILE STATUS IS WS-FS-QUE.
           SELECT NOTIFY-FILE ASSIGN TO "data/notifications.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS NTF-ID
               ALTERNATE RECORD KEY IS NTF-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-NTF.
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUS-ID
               ALTERNATE RECORD KEY IS CUS-DOC WITH DUPLICATES
               FILE STATUS IS WS-FS-CUS.

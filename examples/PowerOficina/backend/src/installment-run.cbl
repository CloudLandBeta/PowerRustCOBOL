       IDENTIFICATION DIVISION.
       PROGRAM-ID. INSTALLMENT-RUN.
      *> Collects the installments that have fallen due (R44b, R44c).
      *> - The gateway's own recurring billing charges each future installment on the authorization the customer gave at
      *>   the contract, without asking for the card again; this program records what the gateway answered for every
      *>   installment due up to the date asked (IRQ-ASOF, 0 = today). It is called by the screens and by whatever the
      *>   host uses to schedule it: the application has no scheduler of its own.
      *> - A charge that is confirmed marks the installment PAID. One that is declined marks it FAILED, counts the try,
      *>   tells the customer by email and WhatsApp, and is tried again on the next run, at most 3 tries in all; then it
      *>   stays FAILED and is counted as given up, for the consultant to see on the Dashboard.
      *> - An installment already PAID or CANCELLED is never charged again. The first installment is charged at the
      *>   contract and is not touched here.
      *> SIMULATED-GATEWAY is the only paragraph a real gateway replaces (IRQ-GATEWAY D declines every charge of the run).
      *> The request and the answer are in backend/COPYBOOKS/req-inst-run.cpy and res-inst-run.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-pay".
           COPY "backend/COPYBOOKS/fc-seq-audit".

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
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-pay".
       01 WS-EOF                                 PIC    X.
       01 WS-ASOF                                PIC    9(8).
       01 WS-GW-OK                               PIC    9.
       01 WS-ED-AMT                              PIC    Z(8)9.99.
       01 WS-LAST-NO                             PIC    99.
       01 WS-OPEN-LEFT                           PIC    99.
       01 WS-N-PAID                              PIC    99     VALUE 0.
       01 WS-J                                   PIC    99.
       01 WS-K                                   PIC    99.
       01 WS-PAID-ID                             PIC    9(8)   OCCURS 50.
       01 WS-HELD-COUNT                          PIC    999.
       01 WS-HANDLED-COUNT                       PIC    9(6).
       01 WS-HELD-INDEX                          PIC    999.
       01 WS-HELD.
          05 WS-HELD-INSTALLMENT                 OCCURS 100.
             10 WS-HELD-PAYMENT-ID               PIC    9(8).
             10 WS-HELD-NUMBER                   PIC    99.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-inst-run".
           COPY "backend/COPYBOOKS/res-inst-run".

       PROCEDURE DIVISION USING IRN-REQ RETURNING IRN-RES.
       MAIN-PARA.
           INITIALIZE IRN-RES
           MOVE    "OK"                          TO    IRS-STATUS
           PERFORM NOW-STAMP
           MOVE    IRQ-ASOF                      TO    WS-ASOF
           IF    WS-ASOF = 0
               MOVE    WS-NOW-DATE               TO    WS-ASOF
           END-IF

           OPEN    INPUT QUOTE-FILE
           PERFORM OPEN-PAY-FILES
           IF    WS-FS-PAY NOT = "00" OR WS-FS-INS NOT = "00" OR WS-FS-QUO NOT = "00"
               MOVE    "ERROR"                   TO    IRS-STATUS
               MOVE    "Arquivos das parcelas indisponíveis."
                                                 TO    IRS-MESSAGE
           ELSE
               MOVE    0                     TO    WS-N-PAID
               PERFORM COLLECT-DUE
               PERFORM CLOSE-PAYMENTS
               IF    IRS-STATUS = "OK"
                   MOVE    "Parcelas vencidas processadas."
                                                 TO    IRS-MESSAGE
               ELSE
                   MOVE    "Falha ao gravar uma parcela ou pagamento; confira antes de repetir."
                                                 TO    IRS-MESSAGE
               END-IF
           END-IF
           PERFORM CLOSE-PAY-FILES
           CLOSE   QUOTE-FILE

           IF    IRS-STATUS = "OK"
               MOVE    IRQ-OPERATOR              TO    WS-AU-OPERATOR
               IF    WS-AU-OPERATOR = SPACES
                   MOVE    "sistema"             TO    WS-AU-OPERATOR
               END-IF
               MOVE    "INSTALLMENT"             TO    WS-AU-ENTITY
               MOVE    WS-ASOF                   TO    WS-AU-REF
               MOVE    "RUN"                     TO    WS-AU-ACTION
               STRING    "cobradas " IRS-CHARGED " falhas " IRS-FAILED " desistidas " IRS-GAVE-UP  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

      *> COLLECT-DUE: reads the installments by due date, from the oldest, up to the date asked.
      *>      A statement on INSTALLMENT-FILE ends the rows a SELECT answered, so HOLD-DUE-INSTALLMENTS takes the keys
      *>      100 at a time and each installment is then read again, charged and rewritten.
       COLLECT-DUE.
           MOVE    "N"                           TO    WS-EOF
           MOVE    0                             TO    WS-HANDLED-COUNT

           PERFORM UNTIL WS-EOF = "Y"
               PERFORM HOLD-DUE-INSTALLMENTS
               PERFORM VARYING WS-HELD-INDEX FROM 1 BY 1 UNTIL WS-HELD-INDEX > WS-HELD-COUNT
                   MOVE    WS-HELD-PAYMENT-ID (WS-HELD-INDEX)
                                                 TO    INS-PAY-ID
                   MOVE    WS-HELD-NUMBER (WS-HELD-INDEX)
                                                 TO    INS-NO
                   PERFORM GET-INSTALLMENT-ROW
                   IF    WS-PQ-STATUS = "00"
                       IF    INS-DUE > WS-ASOF
                           MOVE    "Y"           TO    WS-EOF
                       ELSE
                           IF    INS-NO > 1 AND (INS-STATUS = "DUE" OR INS-STATUS = "FAILED")
                               PERFORM CHARGE-ONE
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
           END-PERFORM.

      *> HOLD-DUE-INSTALLMENTS: the keys of the next installments due up to WS-ASOF, by due date from the oldest, at
      *> most 100, after those already handled.
      *> In:  WS-ASOF; WS-HANDLED-COUNT = the installments already handled.
      *> Out: WS-HELD-INSTALLMENT (1 to WS-HELD-COUNT); WS-HANDLED-COUNT counts them too; WS-EOF = Y when none is left.
       HOLD-DUE-INSTALLMENTS.
           MOVE    0                             TO    WS-HELD-COUNT
           MOVE    WS-ASOF                       TO    INS-DUE
           PERFORM SELECT-INSTALLMENTS-DUE
           PERFORM FETCH-INSTALLMENT-ROW WS-HANDLED-COUNT TIMES
           PERFORM FETCH-INSTALLMENT-ROW
           PERFORM UNTIL WS-PQ-STATUS NOT = "00" OR WS-HELD-COUNT = 100
               ADD     1                         TO    WS-HELD-COUNT
               MOVE    INS-PAY-ID                TO    WS-HELD-PAYMENT-ID (WS-HELD-COUNT)
               MOVE    INS-NO                    TO    WS-HELD-NUMBER (WS-HELD-COUNT)
               IF    WS-HELD-COUNT < 100
                   PERFORM FETCH-INSTALLMENT-ROW
               END-IF
           END-PERFORM
           ADD     WS-HELD-COUNT                 TO    WS-HANDLED-COUNT
           IF    WS-HELD-COUNT < 100 OR WS-HANDLED-COUNT = WS-PQ-COUNT
               MOVE    "Y"                       TO    WS-EOF
           END-IF.

       CHARGE-ONE.
           IF    INS-TRIES >= 3
               ADD     1                         TO    IRS-GAVE-UP
           ELSE
               ADD     1                         TO    INS-TRIES
               PERFORM SIMULATED-GATEWAY
               IF    WS-GW-OK = 1
                   MOVE    "PAID"                TO    INS-STATUS
                   MOVE    WS-NOW                TO    INS-PAID-AT
                   STRING    "TEST-REC-" INS-PAY-ID "-" INS-NO  INTO  INS-GW-REF
                   ADD     1                     TO    IRS-CHARGED
               ELSE
                   MOVE    "FAILED"              TO    INS-STATUS
                   ADD     1                     TO    IRS-FAILED
               END-IF
               PERFORM UPDATE-INSTALLMENT-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "ERROR"               TO    IRS-STATUS
               END-IF
               IF    WS-GW-OK = 1
                   PERFORM CLOSE-PAYMENT-IF-LAST
               ELSE
                   PERFORM TELL-CUSTOMER
               END-IF
           END-IF.

      *> SIMULATED-GATEWAY: the only paragraph a real gateway replaces; the recurring charge is confirmed unless the
      *> run asks for a decline (D).
       SIMULATED-GATEWAY.
           IF    IRQ-GATEWAY = "D"
               MOVE    0                         TO    WS-GW-OK
           ELSE
               MOVE    1                         TO    WS-GW-OK
           END-IF.

      *> CLOSE-PAYMENT-IF-LAST: remembers the payment of an installment just paid; the payments are closed after the run,
      *> when the sequential read of the installments is over (CLOSE-PAYMENTS).
       CLOSE-PAYMENT-IF-LAST.
           IF    WS-N-PAID < 50
               ADD     1                         TO    WS-N-PAID
               MOVE    INS-PAY-ID                TO    WS-PAID-ID (WS-N-PAID)
           END-IF.

      *> CLOSE-PAYMENTS: a payment is PAID only when none of its installments is still DUE or FAILED.
       CLOSE-PAYMENTS.
           PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-N-PAID
               MOVE    WS-PAID-ID (WS-J)         TO    PAY-ID
               PERFORM GET-PAYMENT-ROW
               IF    WS-PQ-STATUS = "00"
                   PERFORM COUNT-OPEN-INSTALLMENTS
                   IF    WS-OPEN-LEFT = 0 AND PAY-STATUS NOT = "PAID" AND PAY-STATUS NOT = "REFUNDED"
                       MOVE    "PAID"            TO    PAY-STATUS
                       PERFORM UPDATE-PAYMENT-ROW
                       IF    WS-PQ-STATUS NOT = "00"
                           MOVE    "ERROR"       TO    IRS-STATUS
                       END-IF
                   END-IF
               END-IF
           END-PERFORM.

       COUNT-OPEN-INSTALLMENTS.
           MOVE    0                             TO    WS-OPEN-LEFT
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > PAY-INSTALLMENTS
               MOVE    PAY-ID                    TO    INS-PAY-ID
               MOVE    WS-K                      TO    INS-NO
               PERFORM GET-INSTALLMENT-ROW
               IF    WS-PQ-STATUS = "00"
                   IF    INS-STATUS = "DUE" OR INS-STATUS = "FAILED"
                       ADD     1                 TO    WS-OPEN-LEFT
                   END-IF
               END-IF
           END-PERFORM.

       TELL-CUSTOMER.
           MOVE    INS-PAY-ID                    TO    PAY-ID
           PERFORM GET-PAYMENT-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    PAY-QUO-ID                TO    WS-NT-QUO
               MOVE    "PARCELA"                 TO    WS-NT-KIND
               MOVE    INS-AMOUNT                TO    WS-ED-AMT
               MOVE    SPACES                    TO    WS-NT-TEXT
               STRING    "Não conseguimos cobrar a parcela " INS-NO " de " PAY-CURRENCY " " WS-ED-AMT
                         " do orçamento " PAY-QUO-ID ". Falaremos com você para regularizar."
                   INTO  WS-NT-TEXT
               PERFORM NOTIFY-BOTH
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-pay".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-payments".
           COPY "backend/COPYBOOKS/pq-installments".
           COPY "backend/COPYBOOKS/pq-notifications".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

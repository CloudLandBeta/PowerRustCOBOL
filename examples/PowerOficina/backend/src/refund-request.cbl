       IDENTIFICATION DIVISION.
       PROGRAM-ID. REFUND-REQUEST.
      *> Refunds one service of a paid order (R46, R47, R48).
      *> - The customer or the consultant (RFQ-BY says who) asks for the refund of one row of the queue, with a reason.
      *> - Only a service still QUEUED can be refunded. One that is RUNNING or DONE is refused with the reason, the
      *>   payment is kept and the refusal is recorded. Other services of the same order are not touched (R47).
      *> - The value is that of the service on the quote, with its tax. On an order paid in installments the future
      *>   installments not yet collected are cancelled first, starting from the last, and the gateway is asked to return
      *>   only what was already paid of that value (R46). The refund is made in the payment's own currency.
      *> - When the gateway confirms, the service leaves the queue (REFUNDED), the refund is recorded with amount,
      *>   currency, time and reason, and the customer is told by email and WhatsApp (R48). If the gateway declines the
      *>   return, nothing changes. When every service of the order is refunded the payment becomes REFUNDED.
      *> SIMULATED-GATEWAY is the only paragraph a real gateway replaces (RFQ-GATEWAY D declines the return).
      *> The request and the answer are in backend/COPYBOOKS/req-refund.cpy and res-refund.cpy.

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
           COPY "backend/COPYBOOKS/res-quote".
       01 WS-OPEN                                PIC    9      VALUE 0.
       01 WS-EOF                                 PIC    X.
       01 WS-I                                   PIC    999.
       01 WS-N                                   PIC    999.
       01 WS-FOUND-PAY                           PIC    9.
       01 WS-REMAIN                              PIC    9(9)V99.
       01 WS-LEFT                                PIC    999.
       01 WS-SVC                                 PIC    X(40).
       01 WS-ED-AMT                              PIC    Z(8)9.99.
       01 WS-TAB.
          05 WS-T-ROW                            OCCURS 12.
             10 WS-T-NO                          PIC    99.
             10 WS-T-AMOUNT                      PIC    9(9)V99.
             10 WS-T-STATUS                      PIC    X(10).
             10 WS-T-CHANGED                     PIC    9.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-refund".
           COPY "backend/COPYBOOKS/res-refund".

       PROCEDURE DIVISION USING RFN-REQ RETURNING RFN-RES.
       MAIN-PARA.
           INITIALIZE RFN-RES
           INITIALIZE QUO-RES
           MOVE    "OK"                          TO    RFS-STATUS
           MOVE    0                             TO    WS-OPEN
           PERFORM NOW-STAMP

           IF    RFQ-BY = SPACES
               MOVE    "INVALID"                 TO    RFS-STATUS
               MOVE    "Informe quem está pedindo o estorno."
                                                 TO    RFS-MESSAGE
           ELSE
               IF    RFQ-REASON = SPACES
                   MOVE    "INVALID"             TO    RFS-STATUS
                   MOVE    "Informe o motivo do estorno."
                                                 TO    RFS-MESSAGE
               END-IF
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM OPEN-ALL
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM FIND-SERVICE
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM FIND-PAYMENT
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM PRICE-SERVICE
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM PLAN-REFUND
               PERFORM SIMULATED-GATEWAY
           END-IF

           IF    RFS-STATUS = "OK"
               PERFORM APPLY-REFUND
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
               PERFORM CLOSE-PAY-FILES
           END-IF

           IF    RFS-STATUS = "OK" OR RFS-STATUS = "REFUSED" OR RFS-STATUS = "DECLINED"
               MOVE    RFQ-BY                    TO    WS-AU-OPERATOR
               MOVE    "REFUND"                  TO    WS-AU-ENTITY
               MOVE    RFQ-QUE-ID                TO    WS-AU-REF
               MOVE    RFS-STATUS                TO    WS-AU-ACTION
               MOVE    RFQ-REASON                TO    WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       OPEN-ALL.
           OPEN    INPUT QUOTE-FILE
           OPEN    INPUT QUOTE-ITEM-FILE
           PERFORM OPEN-PAY-FILES
           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00" AND WS-FS-PAY = "00" AND WS-FS-INS = "00"
                 AND WS-FS-QUE = "00" AND WS-FS-RFD = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    RFS-STATUS
               MOVE    "Arquivos do estorno indisponíveis."
                                                 TO    RFS-MESSAGE
           END-IF.

      *> FIND-SERVICE: the queue row asked for; only a QUEUED one goes on, the others are refused with the reason.
       FIND-SERVICE.
           MOVE    RFQ-QUE-ID                    TO    QUE-ID
           PERFORM GET-QUEUE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    QUE-STATE                 TO    RFS-QSTATE
               EVALUATE QUE-STATE
                   WHEN "QUEUED"
                       CONTINUE
                   WHEN "RUNNING"
                       MOVE    "Serviço em execução não pode ser estornado."
                                                 TO    RFS-MESSAGE
                       PERFORM REFUSE
                   WHEN "DONE"
                       MOVE    "Serviço concluído não pode ser estornado."
                                                 TO    RFS-MESSAGE
                       PERFORM REFUSE
                   WHEN OTHER
                       MOVE    "Este serviço já foi estornado."
                                                 TO    RFS-MESSAGE
                       PERFORM REFUSE
               END-EVALUATE
           ELSE
               MOVE    "NOTFOUND"                TO    RFS-STATUS
               MOVE    "Serviço da fila não encontrado."
                                                 TO    RFS-MESSAGE
           END-IF.

      *> REFUSE: the payment is kept; the refusal is written so it can be seen who asked and why it was not accepted.
       REFUSE.
           MOVE    "REFUSED"                     TO    RFS-STATUS
           PERFORM FIND-PAYMENT-QUIET
           MOVE    "REFUND"                      TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           INITIALIZE REFUND-RECORD
           MOVE    WS-SEQ-GOT                    TO    RFD-ID
           MOVE    PAY-ID                        TO    RFD-PAY-ID
           MOVE    QUE-ID                        TO    RFD-QUE-ID
           MOVE    PAY-CURRENCY                  TO    RFD-CURRENCY
           MOVE    0                             TO    RFD-AMOUNT
           MOVE    RFQ-BY                        TO    RFD-BY
           MOVE    WS-NOW                        TO    RFD-AT
           MOVE    "REFUSED"                     TO    RFD-STATUS
           MOVE    RFS-MESSAGE                   TO    RFD-REASON
           PERFORM INSERT-REFUND-ROW
           MOVE    RFD-ID                        TO    RFS-RFD-ID
           MOVE    PAY-ID                        TO    RFS-PAY-ID.

       FIND-PAYMENT-QUIET.
           MOVE    QUE-QUO-ID                    TO    PAY-QUO-ID
           MOVE    0                             TO    PAY-ID
           MOVE    SPACES                        TO    PAY-CURRENCY
           PERFORM SELECT-PAYMENTS-OF-QUOTE
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-PAYMENT-ROW
           END-IF.

      *> FIND-PAYMENT: the payment of the quote of this service, the one in force (ACTIVE or PAID).
       FIND-PAYMENT.
           MOVE    0                             TO    WS-FOUND-PAY
           MOVE    QUE-QUO-ID                    TO    PAY-QUO-ID
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-PAYMENTS-OF-QUOTE
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y"
               PERFORM FETCH-PAYMENT-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    PAY-QUO-ID NOT = QUE-QUO-ID
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    PAY-STATUS = "ACTIVE" OR PAY-STATUS = "PAID"
                           MOVE    1             TO    WS-FOUND-PAY
                           MOVE    "Y"           TO    WS-EOF
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           IF    WS-FOUND-PAY = 0
               MOVE    "REFUSED"                 TO    RFS-STATUS
               MOVE    "Não há pagamento em vigor para este serviço."
                                                 TO    RFS-MESSAGE
           ELSE
               MOVE    PAY-ID                    TO    RFS-PAY-ID
               MOVE    PAY-CURRENCY              TO    RFS-CURRENCY
           END-IF.

      *> PRICE-SERVICE: the value of the service on the approved quote, with its tax.
       PRICE-SERVICE.
           MOVE    QUE-QUO-ID                    TO    WS-LQ-ID
           MOVE    0                             TO    WS-LQ-REV
           PERFORM LOAD-QUOTE
           IF    WS-LQ-OK = 0 OR QUE-LINE < 1 OR QUE-LINE > QTS-COUNT
               MOVE    "ERROR"                   TO    RFS-STATUS
               MOVE    "Não foi possível achar o valor do serviço no orçamento."
                                                 TO    RFS-MESSAGE
           ELSE
               COMPUTE RFS-AMOUNT = QTL-AMOUNT (QUE-LINE) + QTL-TAX (QUE-LINE)
           END-IF.

      *> PLAN-REFUND: cancels future installments from the last one and keeps what is left to return through the gateway.
      *> Nothing is written yet.
       PLAN-REFUND.
           MOVE    RFS-AMOUNT                    TO    WS-REMAIN
           MOVE    0                             TO    WS-N
           MOVE    PAY-ID                        TO    INS-PAY-ID
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-INSTALLMENTS-OF-PAYMENT
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y" OR WS-N = 12
               PERFORM FETCH-INSTALLMENT-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    INS-PAY-ID NOT = PAY-ID
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       ADD     1                 TO    WS-N
                       MOVE    INS-NO            TO    WS-T-NO (WS-N)
                       MOVE    INS-AMOUNT        TO    WS-T-AMOUNT (WS-N)
                       MOVE    INS-STATUS        TO    WS-T-STATUS (WS-N)
                       MOVE    0                 TO    WS-T-CHANGED (WS-N)
                   END-IF
               END-IF
           END-PERFORM

           MOVE    0                             TO    RFS-CANCELLED
           PERFORM VARYING WS-I FROM WS-N BY -1 UNTIL WS-I < 1
               IF    WS-REMAIN > 0 AND (WS-T-STATUS (WS-I) = "DUE" OR WS-T-STATUS (WS-I) = "FAILED")
                   IF    WS-REMAIN >= WS-T-AMOUNT (WS-I)
                       SUBTRACT WS-T-AMOUNT (WS-I) FROM WS-REMAIN
                       ADD     WS-T-AMOUNT (WS-I) TO   RFS-CANCELLED
                       MOVE    0                 TO    WS-T-AMOUNT (WS-I)
                       MOVE    "CANCELLED"       TO    WS-T-STATUS (WS-I)
                   ELSE
                       SUBTRACT WS-REMAIN        FROM  WS-T-AMOUNT (WS-I)
                       ADD     WS-REMAIN         TO    RFS-CANCELLED
                       MOVE    0                 TO    WS-REMAIN
                   END-IF
                   MOVE    1                     TO    WS-T-CHANGED (WS-I)
               END-IF
           END-PERFORM
           MOVE    WS-REMAIN                     TO    RFS-RETURNED.

      *> SIMULATED-GATEWAY: the only paragraph a real gateway replaces. What was already paid is returned unless the
      *> request asks for a decline (D); a refund of future installments only needs no return at the gateway.
       SIMULATED-GATEWAY.
           IF    RFS-RETURNED > 0 AND RFQ-GATEWAY = "D"
               MOVE    "DECLINED"                TO    RFS-STATUS
               MOVE    "O gateway não devolveu o valor. Nada foi alterado; tente de novo."
                                                 TO    RFS-MESSAGE
           END-IF.

       APPLY-REFUND.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               IF    WS-T-CHANGED (WS-I) = 1
                   MOVE    PAY-ID                TO    INS-PAY-ID
                   MOVE    WS-T-NO (WS-I)        TO    INS-NO
                   PERFORM GET-INSTALLMENT-ROW
                   IF    WS-PQ-STATUS = "00"
                       MOVE    WS-T-AMOUNT (WS-I)
                                                 TO    INS-AMOUNT
                       MOVE    WS-T-STATUS (WS-I)
                                                 TO    INS-STATUS
                       PERFORM UPDATE-INSTALLMENT-ROW
                   END-IF
               END-IF
           END-PERFORM

           MOVE    RFQ-QUE-ID                    TO    QUE-ID
           PERFORM GET-QUEUE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    "REFUNDED"                TO    QUE-STATE
               MOVE    RFQ-BY                    TO    QUE-BY
               PERFORM UPDATE-QUEUE-ROW
           END-IF
           MOVE    "REFUNDED"                    TO    RFS-QSTATE

           MOVE    "REFUND"                      TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           INITIALIZE REFUND-RECORD
           MOVE    WS-SEQ-GOT                    TO    RFD-ID
           MOVE    PAY-ID                        TO    RFD-PAY-ID
           MOVE    RFQ-QUE-ID                    TO    RFD-QUE-ID
           MOVE    PAY-CURRENCY                  TO    RFD-CURRENCY
           MOVE    RFS-AMOUNT                    TO    RFD-AMOUNT
           MOVE    RFQ-BY                        TO    RFD-BY
           MOVE    WS-NOW                        TO    RFD-AT
           MOVE    "DONE"                        TO    RFD-STATUS
           MOVE    RFQ-REASON                    TO    RFD-REASON
           PERFORM INSERT-REFUND-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    RFS-STATUS
           END-IF
           MOVE    RFD-ID                        TO    RFS-RFD-ID

           MOVE    QUE-QUO-ID                    TO    WS-NT-QUO
           MOVE    QUE-SERVICE                   TO    WS-SVC
           PERFORM CLOSE-PAYMENT-IF-ALL-REFUNDED

           MOVE    "ESTORNO"                     TO    WS-NT-KIND
           MOVE    RFS-AMOUNT                    TO    WS-ED-AMT
           MOVE    SPACES                        TO    WS-NT-TEXT
           STRING    "O serviço " WS-SVC " foi estornado: " PAY-CURRENCY " " WS-ED-AMT
                     ". As parcelas futuras desse valor foram canceladas."
               INTO  WS-NT-TEXT
           PERFORM NOTIFY-BOTH
           MOVE    "Estorno registrado. O serviço saiu da fila."
                                                 TO    RFS-MESSAGE.

      *> CLOSE-PAYMENT-IF-ALL-REFUNDED: when no service of the order is left outside REFUNDED, the payment is REFUNDED.
       CLOSE-PAYMENT-IF-ALL-REFUNDED.
           MOVE    0                             TO    WS-LEFT
           MOVE    WS-NT-QUO                     TO    QUE-QUO-ID
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-QUEUE-OF-QUOTE
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y"
               PERFORM FETCH-QUEUE-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    QUE-QUO-ID NOT = WS-NT-QUO
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    QUE-STATE NOT = "REFUNDED"
                           ADD     1             TO    WS-LEFT
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           IF    WS-LEFT = 0
               MOVE    RFS-PAY-ID                TO    PAY-ID
               PERFORM GET-PAYMENT-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "REFUNDED"            TO    PAY-STATUS
                   PERFORM UPDATE-PAYMENT-ROW
               END-IF
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pd-pay".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-payments".
           COPY "backend/COPYBOOKS/pq-installments".
           COPY "backend/COPYBOOKS/pq-refunds".
           COPY "backend/COPYBOOKS/pq-queue".
           COPY "backend/COPYBOOKS/pq-notifications".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

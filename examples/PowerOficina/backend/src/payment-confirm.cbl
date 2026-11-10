       IDENTIFICATION DIVISION.
       PROGRAM-ID. PAYMENT-CONFIRM.
      *> Pays an approved quote by credit card, in 1 to 12 installments without interest (R24 to R27, R44 to R45).
      *> - Mode P only shows the schedule: the number of installments, the value of each, the due dates (the first today,
      *>   the others every 30 days) and the total, which is always the value of the quote. The cent left over by the
      *>   division goes to the first installment.
      *> - Mode C charges the first installment at the gateway (simulated, in test mode: PCQ-GATEWAY D declines and T does
      *>   not answer). When it is confirmed the payment, its installments and the queue of the services are written, and
      *>   the order is under way even if installments are still to come. If the gateway declines or does not answer, the
      *>   quote stays approved without a payment, the answer says why and the customer may try again.
      *> - Paying in more than one installment needs the customer's express consent to the future charges (R44d); without
      *>   it only 1 installment is accepted. The future charges are made by the gateway's own recurring billing: the
      *>   authorization reference is kept, never the card (R25: only brand, last four digits and the gateway reference).
      *> - The same quote is never charged twice: asking again answers ALREADY with the payment that exists (R27).
      *> SIMULATED-GATEWAY is the only paragraph a real gateway replaces.
      *> The request and the answer are in backend/COPYBOOKS/req-payment.cpy and res-payment.cpy.

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
       01 WS-GW-OK                               PIC    9.
       01 WS-PER                                 PIC    9(9)V99.
       01 WS-REST                                PIC    9(9)V99.
       01 WS-BASE                                PIC    9(8).
       01 WS-NEW-PAY                             PIC    9(8).
       01 WS-ED-AMT                              PIC    Z(8)9.99.
       01 WS-ED-TOT                              PIC    Z(8)9.99.
       01 WS-DT                                  PIC    9(8).
       01 WS-DTX REDEFINES WS-DT.
          05 WS-DT-Y                             PIC    9(4).
          05 WS-DT-M                             PIC    99.
          05 WS-DT-D                             PIC    99.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-payment".
           COPY "backend/COPYBOOKS/res-payment".

       PROCEDURE DIVISION USING PCF-REQ RETURNING PCF-RES.
       MAIN-PARA.
           INITIALIZE PCF-RES
           INITIALIZE QUO-RES
           MOVE    "OK"                          TO    PCS-STATUS
           MOVE    0                             TO    WS-OPEN
           MOVE    0                             TO    WS-GW-OK
           PERFORM NOW-STAMP

           PERFORM VALIDATE-REQUEST

           IF    PCS-STATUS = "OK"
               PERFORM OPEN-ALL
           END-IF

           IF    PCS-STATUS = "OK"
               PERFORM LOAD-APPROVED
           END-IF

           IF    PCS-STATUS = "OK"
               PERFORM FIND-EXISTING
           END-IF

           IF    PCS-STATUS = "OK"
               PERFORM BUILD-SCHEDULE
           END-IF

           IF    PCS-STATUS = "OK" AND PCQ-MODE = "C"
               PERFORM SIMULATED-GATEWAY
           END-IF

           IF    PCS-STATUS = "OK" AND PCQ-MODE = "C" AND WS-GW-OK = 1
               PERFORM SAVE-PAYMENT
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
               PERFORM CLOSE-PAY-FILES
           END-IF

           IF    PCQ-MODE = "C" AND (PCS-STATUS = "OK" OR PCS-STATUS = "DECLINED" OR PCS-STATUS = "TIMEOUT" OR PCS-STATUS = "ERROR")
               MOVE    PCQ-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "PAYMENT"                 TO    WS-AU-ENTITY
               MOVE    PCQ-QUO-ID                TO    WS-AU-REF
               MOVE    PCS-STATUS                TO    WS-AU-ACTION
               STRING    PCQ-INSTALLMENTS "x " PCQ-BRAND " final " PCQ-LAST4  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       VALIDATE-REQUEST.
           EVALUATE TRUE
               WHEN PCQ-MODE NOT = "P" AND PCQ-MODE NOT = "C"
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "MODE"                TO    PCS-FIELD
                   MOVE    "O modo deve ser P (conferir) ou C (confirmar)."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-INSTALLMENTS < 1 OR PCQ-INSTALLMENTS > 12
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "INSTALL"             TO    PCS-FIELD
                   MOVE    "O número de parcelas deve ser de 1 a 12."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-INSTALLMENTS > 1 AND PCQ-CONSENT NOT = "S"
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "CONSENT"             TO    PCS-FIELD
                   MOVE    "Sem o consentimento às cobranças futuras só é possível pagar à vista."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-MODE = "C" AND PCQ-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "OPERATOR"            TO    PCS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-MODE = "C" AND PCQ-BRAND = SPACES
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "BRAND"               TO    PCS-FIELD
                   MOVE    "Informe a bandeira do cartão."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-MODE = "C" AND PCQ-BRAND NOT ALPHABETIC
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "BRAND"               TO    PCS-FIELD
                   MOVE    "A bandeira só leva letras; nunca digite o número do cartão."
                                                 TO    PCS-MESSAGE
               WHEN PCQ-MODE = "C" AND PCQ-LAST4 NOT NUMERIC
                   MOVE    "INVALID"             TO    PCS-STATUS
                   MOVE    "LAST4"               TO    PCS-FIELD
                   MOVE    "Informe os quatro últimos dígitos do cartão."
                                                 TO    PCS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       OPEN-ALL.
           OPEN    I-O QUOTE-FILE
           OPEN    INPUT QUOTE-ITEM-FILE
           PERFORM OPEN-PAY-FILES
           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00" AND WS-FS-PAY = "00" AND WS-FS-INS = "00"
                 AND WS-FS-QUE = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    PCS-STATUS
               MOVE    "Arquivos do pagamento indisponíveis."
                                                 TO    PCS-MESSAGE
           END-IF.

      *> LOAD-APPROVED: the latest revision of the quote must exist and be approved.
       LOAD-APPROVED.
           MOVE    PCQ-QUO-ID                    TO    WS-LQ-ID
           MOVE    0                             TO    WS-LQ-REV
           PERFORM LOAD-QUOTE
           IF    WS-LQ-OK = 0
               MOVE    "NOTFOUND"                TO    PCS-STATUS
               MOVE    "Orçamento não encontrado."
                                                 TO    PCS-MESSAGE
           ELSE
               MOVE    QTS-ID                    TO    PCS-QUO-ID
               MOVE    QTS-CURRENCY              TO    PCS-CURRENCY
               MOVE    QTS-TOTAL                 TO    PCS-TOTAL
               IF    QTS-QSTATUS NOT = "APROVADO"
                   MOVE    "UNAPPROVED"         TO    PCS-STATUS
                   MOVE    "Só um orçamento aprovado pode ser pago."
                                                 TO    PCS-MESSAGE
               END-IF
           END-IF.

      *> FIND-EXISTING: a payment already made or under way for this quote is answered as it is, never charged again.
       FIND-EXISTING.
           MOVE    PCQ-QUO-ID                    TO    PAY-QUO-ID
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
                   IF    PAY-QUO-ID NOT = PCQ-QUO-ID
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    PAY-STATUS = "ACTIVE" OR PAY-STATUS = "PAID"
                           PERFORM SHOW-EXISTING
                           MOVE    "Y"           TO    WS-EOF
                       END-IF
                   END-IF
               END-IF
           END-PERFORM.

       SHOW-EXISTING.
           MOVE    "ALREADY"                     TO    PCS-STATUS
           MOVE    "Este orçamento já foi pago; nada foi cobrado de novo."
                                                 TO    PCS-MESSAGE
           MOVE    PAY-ID                        TO    PCS-PAY-ID
           MOVE    PAY-INSTALLMENTS              TO    PCS-N
           MOVE    PAY-GW-REF                    TO    PCS-GW-REF
           MOVE    PAY-ID                        TO    INS-PAY-ID
           MOVE    0                             TO    WS-I
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-INSTALLMENTS-OF-PAYMENT
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y" OR WS-I = 12
               PERFORM FETCH-INSTALLMENT-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    INS-PAY-ID NOT = PCS-PAY-ID
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       ADD     1                 TO    WS-I
                       MOVE    INS-NO            TO    PCI-NO (WS-I)
                       MOVE    INS-DUE           TO    PCI-DUE (WS-I)
                       MOVE    INS-AMOUNT        TO    PCI-AMOUNT (WS-I)
                       MOVE    INS-STATUS        TO    PCI-STATUS (WS-I)
                   END-IF
               END-IF
           END-PERFORM.

      *> BUILD-SCHEDULE: equal installments without interest; the leftover cent goes to the first; the first is due
      *> today and each next one 30 days after the previous.
       BUILD-SCHEDULE.
           COMPUTE WS-PER = FUNCTION INTEGER ( QTS-TOTAL * 100 / PCQ-INSTALLMENTS ) / 100
           COMPUTE WS-REST = QTS-TOTAL - WS-PER * PCQ-INSTALLMENTS
           COMPUTE WS-BASE = FUNCTION INTEGER-OF-DATE ( WS-NOW-DATE )
           MOVE    PCQ-INSTALLMENTS              TO    PCS-N
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > PCQ-INSTALLMENTS
               MOVE    WS-I                      TO    PCI-NO (WS-I)
               COMPUTE PCI-DUE (WS-I) = FUNCTION DATE-OF-INTEGER ( WS-BASE + 30 * ( WS-I - 1 ) )
               MOVE    WS-PER                    TO    PCI-AMOUNT (WS-I)
               MOVE    "DUE"                     TO    PCI-STATUS (WS-I)
           END-PERFORM
           ADD     WS-REST                       TO    PCI-AMOUNT (1)
           IF    PCQ-MODE = "P"
               MOVE    "Confira as parcelas antes de confirmar."
                                                 TO    PCS-MESSAGE
           END-IF.

      *> SIMULATED-GATEWAY: the only paragraph a real card gateway replaces. In test mode the charge is confirmed unless
      *> the request asks for a decline (D) or for silence (T).
       SIMULATED-GATEWAY.
           EVALUATE PCQ-GATEWAY
               WHEN "D"
                   MOVE    "DECLINED"            TO    PCS-STATUS
                   MOVE    "O gateway recusou a cobrança. O orçamento segue aprovado, sem pagamento; tente de novo."
                                                 TO    PCS-MESSAGE
               WHEN "T"
                   MOVE    "TIMEOUT"             TO    PCS-STATUS
                   MOVE    "O gateway não respondeu. Nada foi cobrado; tente de novo."
                                                 TO    PCS-MESSAGE
               WHEN OTHER
                   MOVE    1                     TO    WS-GW-OK
           END-EVALUATE.

      *> SAVE-PAYMENT: the first installment is confirmed, so the payment, its installments and the queue are written.
       SAVE-PAYMENT.
           MOVE    "PAYMENT"                     TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           IF    WS-SEQ-GOT = 0
               MOVE    "ERROR"                   TO    PCS-STATUS
               MOVE    "Não foi possível numerar o pagamento."
                                                 TO    PCS-MESSAGE
           ELSE
               MOVE    WS-SEQ-GOT                TO    WS-NEW-PAY
               PERFORM WRITE-PAYMENT
               IF    PCS-STATUS NOT = "ERROR"
                   PERFORM WRITE-INSTALLMENTS
                   PERFORM WRITE-QUEUE
                   PERFORM SEND-RECEIPT
                   MOVE    "Pagamento confirmado. Os serviços entraram na fila de execução."
                                                 TO    PCS-MESSAGE
               ELSE
                   MOVE    "Não foi possível gravar o pagamento; nada foi registrado. Confira antes de repetir."
                                                 TO    PCS-MESSAGE
               END-IF
           END-IF.

       WRITE-PAYMENT.
           INITIALIZE PAYMENT-RECORD
           MOVE    WS-NEW-PAY                    TO    PAY-ID
           MOVE    PCQ-QUO-ID                    TO    PAY-QUO-ID
           MOVE    QTS-CURRENCY                  TO    PAY-CURRENCY
           MOVE    QTS-TOTAL                     TO    PAY-TOTAL
           MOVE    PCQ-INSTALLMENTS              TO    PAY-INSTALLMENTS
           MOVE    PCQ-BRAND                     TO    PAY-CARD-BRAND
           MOVE    PCQ-LAST4                     TO    PAY-CARD-LAST4
           IF    PCQ-INSTALLMENTS > 1
               STRING    "TEST-AUTH-" WS-NEW-PAY  INTO  PAY-GW-REF
               MOVE    "ACTIVE"                  TO    PAY-STATUS
               MOVE    WS-NOW                    TO    PAY-CONSENT-AT
           ELSE
               STRING    "TEST-CHG-" WS-NEW-PAY "-1"  INTO  PAY-GW-REF
               MOVE    "PAID"                    TO    PAY-STATUS
               MOVE    0                         TO    PAY-CONSENT-AT
           END-IF
           MOVE    WS-NOW                        TO    PAY-AT
           PERFORM INSERT-PAYMENT-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    PCS-STATUS
           END-IF
           MOVE    WS-NEW-PAY                    TO    PCS-PAY-ID
           MOVE    PAY-GW-REF                    TO    PCS-GW-REF.

       WRITE-INSTALLMENTS.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > PCQ-INSTALLMENTS
               INITIALIZE INSTALLMENT-RECORD
               MOVE    WS-NEW-PAY                TO    INS-PAY-ID
               MOVE    WS-I                      TO    INS-NO
               MOVE    PCI-DUE (WS-I)            TO    INS-DUE
               MOVE    PCI-AMOUNT (WS-I)         TO    INS-AMOUNT
               IF    WS-I = 1
                   MOVE    "PAID"                TO    INS-STATUS
                   MOVE    PAY-GW-REF            TO    INS-GW-REF
                   MOVE    WS-NOW                TO    INS-PAID-AT
                   MOVE    1                     TO    INS-TRIES
                   MOVE    "PAID"                TO    PCI-STATUS (WS-I)
               ELSE
                   MOVE    "DUE"                 TO    INS-STATUS
                   MOVE    0                     TO    INS-TRIES
               END-IF
               PERFORM INSERT-INSTALLMENT-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "ERROR"               TO    PCS-STATUS
               END-IF
           END-PERFORM.

      *> WRITE-QUEUE: one row per service of the quote, waiting to be done (QUEUED) with the deadline the quote agreed.
       WRITE-QUEUE.
           MOVE    0                             TO    PCS-QUEUED
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > QTS-COUNT
               IF    QTL-KIND (WS-I) = "S"
                   MOVE    "QUEUE"               TO    WS-SEQ-WANT
                   PERFORM NEXT-NUMBER
                   INITIALIZE QUEUE-RECORD
                   MOVE    WS-SEQ-GOT            TO    QUE-ID
                   MOVE    PCQ-QUO-ID            TO    QUE-QUO-ID
                   MOVE    WS-I                  TO    QUE-LINE
                   MOVE    QTS-PLATE             TO    QUE-PLATE
                   MOVE    QTL-NAME (WS-I)       TO    QUE-SERVICE
                   MOVE    "QUEUED"              TO    QUE-STATE
                   MOVE    50                    TO    QUE-PRIORITY
                   MOVE    WS-NOW                TO    QUE-APPROVED-AT
                   MOVE    QTS-DEADLINE          TO    QUE-DEADLINE
                   MOVE    PCQ-OPERATOR          TO    QUE-BY
                   PERFORM INSERT-QUEUE-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "ERROR"           TO    PCS-STATUS
                   END-IF
                   ADD     1                     TO    PCS-QUEUED
               END-IF
           END-PERFORM.

      *> SEND-RECEIPT: the receipt of the contract (number, items, value, form of payment, date) and the date to come
      *> back for the vehicle, by email and WhatsApp to the channels the customer consented to (R31, R32, R34).
       SEND-RECEIPT.
           MOVE    PCQ-QUO-ID                    TO    WS-NT-QUO
           MOVE    "RECIBO"                      TO    WS-NT-KIND
           MOVE    PCI-AMOUNT (1)                TO    WS-ED-AMT
           MOVE    QTS-TOTAL                     TO    WS-ED-TOT
           MOVE    WS-NOW-DATE                   TO    WS-DT
           MOVE    SPACES                        TO    WS-NT-TEXT
           STRING    "Recibo " WS-NEW-PAY ": orçamento " PCQ-QUO-ID ", " QTS-COUNT " itens, total " QTS-CURRENCY " "
                     WS-ED-TOT ", " PCQ-INSTALLMENTS "x no cartão " PCQ-BRAND " final " PCQ-LAST4
                     " (1ª parcela " WS-ED-AMT "), em " WS-DT-D "/" WS-DT-M "/" WS-DT-Y "."
               INTO  WS-NT-TEXT
           PERFORM NOTIFY-BOTH

           IF    QTS-DEADLINE > 0
               MOVE    "PRAZO"                   TO    WS-NT-KIND
               MOVE    QTS-DEADLINE              TO    WS-DT
               MOVE    SPACES                    TO    WS-NT-TEXT
               STRING    "Seu veículo " QTS-PLATE " ficará pronto até " WS-DT-D "/" WS-DT-M "/" WS-DT-Y
                         ". Volte nessa data para retirá-lo."
                   INTO  WS-NT-TEXT
               PERFORM NOTIFY-BOTH
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pd-pay".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-payments".
           COPY "backend/COPYBOOKS/pq-installments".
           COPY "backend/COPYBOOKS/pq-queue".
           COPY "backend/COPYBOOKS/pq-notifications".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

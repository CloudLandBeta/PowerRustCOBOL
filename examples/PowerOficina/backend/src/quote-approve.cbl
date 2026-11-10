       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-APPROVE.
      *> Records the answer to a quote: approved or refused (R21, R22, R37).
      *> - Mode L: the customer, through the link: the token (or the quote number) and the short code must be valid; the
      *>   token is used once. Mode M: the consultant registers an answer that came by phone, WhatsApp or at the counter,
      *>   with the channel and the operator. Both leave the same state.
      *> - Approving takes the quote to APROVADO (approved, waiting for the payment) and agrees the execution date it
      *>   shows. Refusing or asking for a change needs a reason and takes it to RECUSADO, which returns it to the
      *>   consultant to revise (a new revision) or to close.
      *> - Only a sent quote can be answered by the link; the consultant may also register an answer for an open one. A
      *>   quote past its validity is marked EXPIRADO and cannot be answered (R20). An answer is never changed.
      *> Every answer writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-quote-approve.cpy and res-quote-approve.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-send".
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/tokens.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/approvals.FD".
           COPY "COPYBOOKS/customers.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-token".
           COPY "backend/COPYBOOKS/res-quote".
       01 WS-OPEN                                PIC    9      VALUE 0.
       01 WS-TARGET-ID                           PIC    9(8).
       01 WS-TARGET-REV                          PIC    99.
       01 WS-VIA-TOKEN                           PIC    9.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-approve".
           COPY "backend/COPYBOOKS/res-quote-approve".

       PROCEDURE DIVISION USING QAP-REQ RETURNING QAP-RES.
       MAIN-PARA.
           INITIALIZE QAP-RES
           INITIALIZE QUO-RES
           MOVE    "OK"                          TO    QAS-STATUS
           MOVE    0                             TO    WS-OPEN
           MOVE    0                             TO    WS-VIA-TOKEN
           PERFORM NOW-STAMP

           PERFORM VALIDATE-REQUEST

           IF    QAS-STATUS = "OK"
               PERFORM OPEN-ALL
           END-IF

           IF    QAS-STATUS = "OK"
               PERFORM FIND-TARGET
           END-IF

           IF    QAS-STATUS = "OK"
               PERFORM LOAD-TARGET
           END-IF

           IF    QAS-STATUS = "OK"
               PERFORM CHECK-OPEN
           END-IF

           IF    QAS-STATUS = "OK"
               PERFORM RECORD-ANSWER
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
               CLOSE   CATALOG-FILE
               CLOSE   TOKEN-FILE
               CLOSE   APPROVAL-FILE
           END-IF

           IF    QAS-STATUS = "OK"
               IF    QAP-MODE = "L"
                   MOVE    "CLIENTE"             TO    WS-AU-OPERATOR
               ELSE
                   MOVE    QAP-OPERATOR          TO    WS-AU-OPERATOR
               END-IF
               MOVE    "QUOTE"                   TO    WS-AU-ENTITY
               MOVE    QAS-QUO-ID                TO    WS-AU-REF
               MOVE    QAS-QSTATUS               TO    WS-AU-ACTION
               STRING    "via " QAS-CHANNEL  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       VALIDATE-REQUEST.
           EVALUATE TRUE
               WHEN QAP-MODE NOT = "L" AND QAP-MODE NOT = "M"
                   MOVE    "INVALID"             TO    QAS-STATUS
                   MOVE    "MODE"                TO    QAS-FIELD
                   MOVE    "O modo deve ser L (link) ou M (manual)."
                                                 TO    QAS-MESSAGE
               WHEN QAP-DECISION NOT = "A" AND QAP-DECISION NOT = "R"
                   MOVE    "INVALID"             TO    QAS-STATUS
                   MOVE    "DECISION"            TO    QAS-FIELD
                   MOVE    "Escolha aprovar (A) ou recusar (R)."
                                                 TO    QAS-MESSAGE
               WHEN QAP-DECISION = "R" AND QAP-REASON = SPACES
                   MOVE    "INVALID"             TO    QAS-STATUS
                   MOVE    "REASON"              TO    QAS-FIELD
                   MOVE    "Informe o motivo da recusa ou da alteração."
                                                 TO    QAS-MESSAGE
               WHEN QAP-MODE = "M" AND QAP-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    QAS-STATUS
                   MOVE    "OPERATOR"            TO    QAS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    QAS-MESSAGE
               WHEN QAP-MODE = "M" AND QAP-CHANNEL NOT = "PHONE"
                     AND QAP-CHANNEL NOT = "WHATSAPP" AND QAP-CHANNEL NOT = "COUNTER"
                   MOVE    "INVALID"             TO    QAS-STATUS
                   MOVE    "CHANNEL"             TO    QAS-FIELD
                   MOVE    "O canal deve ser PHONE, WHATSAPP ou COUNTER."
                                                 TO    QAS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       OPEN-ALL.
           OPEN    I-O QUOTE-FILE
           OPEN    INPUT QUOTE-ITEM-FILE
           OPEN    INPUT CATALOG-FILE
           OPEN    I-O TOKEN-FILE
           IF    WS-FS-TOK = "35"
               OPEN    OUTPUT TOKEN-FILE
               CLOSE   TOKEN-FILE
               OPEN    I-O TOKEN-FILE
           END-IF

           OPEN    I-O APPROVAL-FILE
           IF    WS-FS-APR = "35"
               OPEN    OUTPUT APPROVAL-FILE
               CLOSE   APPROVAL-FILE
               OPEN    I-O APPROVAL-FILE
           END-IF

           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00" AND WS-FS-TOK = "00" AND WS-FS-APR = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    QAS-STATUS
               MOVE    "Arquivos da aprovação indisponíveis."
                                                 TO    QAS-MESSAGE
           END-IF.

      *> FIND-TARGET: the quote and revision being answered. By the link it is the one the token was made for; by hand
      *> it is the latest revision of the number typed.
       FIND-TARGET.
           IF    QAP-MODE = "L"
               MOVE    QAP-TOKEN                 TO    WS-REQ-TOKEN
               MOVE    QAP-QUO-ID                TO    WS-REQ-ID
               MOVE    QAP-CODE                  TO    WS-REQ-CODE
               PERFORM RESOLVE-TOKEN
               MOVE    WS-TK-STATUS              TO    QAS-STATUS
               IF    WS-TK-STATUS = "OK"
                   MOVE    1                     TO    WS-VIA-TOKEN
                   MOVE    TOK-QUO-ID            TO    WS-TARGET-ID
                   MOVE    TOK-REV               TO    WS-TARGET-REV
                   MOVE    "LINK"                TO    QAS-CHANNEL
               ELSE
                   MOVE    "Link ou código não aceito." TO QAS-MESSAGE
               END-IF
           ELSE
               MOVE    QAP-QUO-ID                TO    WS-TARGET-ID
               MOVE    0                         TO    WS-TARGET-REV
               MOVE    QAP-CHANNEL               TO    QAS-CHANNEL
           END-IF.

       LOAD-TARGET.
           MOVE    WS-TARGET-ID                  TO    WS-LQ-ID
           MOVE    WS-TARGET-REV                 TO    WS-LQ-REV
           PERFORM LOAD-QUOTE
           IF    WS-LQ-OK = 0
               MOVE    "NOTFOUND"                TO    QAS-STATUS
               MOVE    "Orçamento não encontrado."
                                                 TO    QAS-MESSAGE
           ELSE
               MOVE    QTS-ID                    TO    QAS-QUO-ID
               MOVE    QTS-REV                   TO    QAS-REV
               MOVE    QTS-QSTATUS               TO    QAS-QSTATUS
           END-IF.

      *> CHECK-OPEN: the quote must wait for an answer, still be valid and still be the latest revision.
       CHECK-OPEN.
           MOVE    WS-TARGET-ID                  TO    WS-LQ-ID
           PERFORM FIND-LATEST-REV
           EVALUATE TRUE
               WHEN QTS-REV NOT = WS-LQ-REV
                   MOVE    "NOTOPEN"             TO    QAS-STATUS
                   MOVE    "O orçamento foi revisado; peça o link da nova revisão."
                                                 TO    QAS-MESSAGE
               WHEN QTS-QSTATUS NOT = "ENVIADO"
                     AND (QAP-MODE = "L" OR QTS-QSTATUS NOT = "ABERTO")
                   MOVE    "NOTOPEN"             TO    QAS-STATUS
                   MOVE    "Esse orçamento não está esperando resposta."
                                                 TO    QAS-MESSAGE
               WHEN QTS-VALID-UNTIL < WS-NOW-DATE
                   MOVE    WS-TARGET-ID          TO    QUO-ID
                   MOVE    QTS-REV               TO    QUO-REV
                   PERFORM GET-QUOTE-ROW
                   IF    WS-PQ-STATUS = "00"
                       MOVE    "EXPIRADO"        TO    QUO-STATUS
                       PERFORM UPDATE-QUOTE-ROW
                   END-IF
                   MOVE    "EXPIRED"             TO    QAS-STATUS
                   MOVE    "EXPIRADO"            TO    QAS-QSTATUS
                   MOVE    "O orçamento venceu e foi marcado como expirado."
                                                 TO    QAS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

      *> RECORD-ANSWER: writes the answer, moves the quote to its new state and uses the token.
       RECORD-ANSWER.
           MOVE    "APPROVAL"                    TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           INITIALIZE APPROVAL-RECORD
           MOVE    WS-SEQ-GOT                    TO    APR-ID
           MOVE    QAS-QUO-ID                    TO    APR-QUO-ID
           MOVE    QAS-REV                       TO    APR-REV
           MOVE    QAP-DECISION                  TO    APR-DECISION
           MOVE    QAS-CHANNEL                   TO    APR-CHANNEL
           MOVE    WS-NOW                        TO    APR-AT
           MOVE    QAP-REASON                    TO    APR-REASON
           IF    QAP-MODE = "L"
               MOVE    "CLIENTE"                 TO    APR-OPERATOR
           ELSE
               MOVE    QAP-OPERATOR              TO    APR-OPERATOR
           END-IF
           PERFORM INSERT-APPROVAL-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QAS-STATUS
               MOVE    "Não foi possível gravar a resposta."
                                                 TO    QAS-MESSAGE
           END-IF

           IF    QAS-STATUS = "OK"
               MOVE    QAS-QUO-ID                TO    QUO-ID
               MOVE    QAS-REV                   TO    QUO-REV
               PERFORM GET-QUOTE-ROW
               IF    WS-PQ-STATUS = "00"
                   IF    QAP-DECISION = "A"
                       MOVE    "APROVADO"        TO    QUO-STATUS
                   ELSE
                       MOVE    "RECUSADO"        TO    QUO-STATUS
                   END-IF
                   PERFORM UPDATE-QUOTE-ROW
                   MOVE    QUO-STATUS            TO    QAS-QSTATUS
               END-IF
               MOVE    WS-NOW                    TO    QAS-AT
               IF    QAP-DECISION = "A"
                   MOVE    "Orçamento aprovado. Aguardando o pagamento."
                                                 TO    QAS-MESSAGE
               ELSE
                   MOVE    "Resposta registrada. O orçamento voltou ao consultor."
                                                 TO    QAS-MESSAGE
               END-IF
           END-IF

           IF    QAS-STATUS = "OK" AND WS-VIA-TOKEN = 1
               MOVE    "S"                       TO    TOK-USED
               PERFORM UPDATE-TOKEN-ROW
           END-IF.

           COPY "backend/COPYBOOKS/pd-token".
           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-tokens".
           COPY "backend/COPYBOOKS/pq-approvals".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

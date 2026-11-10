       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-SEND.
      *> Sends the latest revision of a quote to the customer by email and by WhatsApp and records the result of each
      *> channel (R18). The delivery is SIMULATED behind this one paragraph (SIMULATED-DELIVERY): the message is
      *> recorded as sent and nothing leaves the application; a real email or WhatsApp provider replaces that paragraph.
      *> - A channel is used only if the customer consented to it and has the contact for it; otherwise it is SKIPPED.
      *> - A channel that fails is tried 3 times; the result is FAILED and the other channel is still delivered (R19).
      *> - The message carries a link token and a short code. The token is kept in the token file with an expiry (the end
      *>   of the quote's validity) and is used once; sending again replaces the earlier tokens (R37).
      *> - The quote becomes ENVIADO when at least one channel delivered. Only an open or already sent quote can be sent.
      *> Every sending writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-quote-send.cpy and res-quote-send.cpy.

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
       01 WS-EOF                                 PIC    X.
       01 WS-ALPHABET                            PIC    X(32)
                                                 VALUE "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".
       01 WS-R                                   PIC    9V9(8).
       01 WS-SEED                                PIC    9(9).
       01 WS-POS                                 PIC    99.
       01 WS-PICK                                PIC    99.
       01 WS-CODE-N                              PIC    9(6).
       01 WS-TRIES                               PIC    9.
       01 WS-DOWN                                PIC    X.
       01 WS-CHANNEL                             PIC    X(8).
       01 WS-TO                                  PIC    X(60).
       01 WS-RESULT                              PIC    X(8).
       01 WS-SENT                                PIC    9      VALUE 0.
       01 WS-TOTAL-ED                            PIC    Z(8)9.99.
       01 WS-LATEST                              PIC    99.
       01 WS-HELD-COUNT                          PIC    999.
       01 WS-HANDLED-COUNT                       PIC    9(6).
       01 WS-HELD-INDEX                          PIC    999.
       01 WS-HELD.
          05 WS-HELD-TOKEN                       PIC    X(32)  OCCURS 100.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-send".
           COPY "backend/COPYBOOKS/res-quote-send".

       PROCEDURE DIVISION USING QSN-REQ RETURNING QSN-RES.
       MAIN-PARA.
           INITIALIZE QSN-RES
           INITIALIZE QUO-RES
           MOVE    "OK"                          TO    QNS-STATUS
           MOVE    0                             TO    WS-OPEN
           MOVE    0                             TO    WS-SENT
           PERFORM NOW-STAMP

           IF    QSN-OPERATOR = SPACES
               MOVE    "INVALID"                 TO    QNS-STATUS
               MOVE    "Informe o nome do operador."
                                                 TO    QNS-MESSAGE
           END-IF

           IF    QNS-STATUS = "OK"
               PERFORM OPEN-ALL
           END-IF

           IF    QNS-STATUS = "OK"
               MOVE    QSN-ID                    TO    WS-LQ-ID
               MOVE    0                         TO    WS-LQ-REV
               PERFORM LOAD-QUOTE
               IF    WS-LQ-OK = 0
                   MOVE    "NOTFOUND"            TO    QNS-STATUS
                   MOVE    "Orçamento não encontrado."
                                                 TO    QNS-MESSAGE
               END-IF
           END-IF

           IF    QNS-STATUS = "OK"
               PERFORM CHECK-SENDABLE
           END-IF

           IF    QNS-STATUS = "OK"
               PERFORM READ-CUSTOMER
           END-IF

           IF    QNS-STATUS = "OK"
               PERFORM MAKE-TOKEN
           END-IF

           IF    QNS-STATUS = "OK"
               PERFORM MAKE-TEXT
               MOVE    "EMAIL"                   TO    WS-CHANNEL
               MOVE    QSN-DOWN-EMAIL            TO    WS-DOWN
               IF    CUS-CONSENT-EMAIL = "S" AND CUS-EMAIL NOT = SPACES
                   MOVE    CUS-EMAIL             TO    WS-TO
                   PERFORM DELIVER
               ELSE
                   MOVE    "SKIPPED"             TO    WS-RESULT
                   MOVE    0                     TO    WS-TRIES
               END-IF
               MOVE    WS-RESULT                 TO    QNS-EMAIL
               MOVE    WS-TRIES                  TO    QNS-EMAIL-TRIES

               MOVE    "WHATSAPP"                TO    WS-CHANNEL
               MOVE    QSN-DOWN-WA               TO    WS-DOWN
               IF    CUS-CONSENT-WA = "S" AND CUS-WHATSAPP NOT = SPACES
                   MOVE    CUS-WHATSAPP          TO    WS-TO
                   PERFORM DELIVER
               ELSE
                   MOVE    "SKIPPED"             TO    WS-RESULT
                   MOVE    0                     TO    WS-TRIES
               END-IF
               MOVE    WS-RESULT                 TO    QNS-WA
               MOVE    WS-TRIES                  TO    QNS-WA-TRIES

               PERFORM SET-OUTCOME
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
               CLOSE   CATALOG-FILE
               CLOSE   TOKEN-FILE
               CLOSE   NOTIFY-FILE
               CLOSE   CUSTOMER-FILE
           END-IF

           IF    QNS-STATUS = "OK" OR QNS-STATUS = "NOCHANNEL"
               MOVE    QSN-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "QUOTE"                   TO    WS-AU-ENTITY
               MOVE    QNS-ID                    TO    WS-AU-REF
               MOVE    "SEND"                    TO    WS-AU-ACTION
               STRING    "email " QNS-EMAIL " whatsapp " QNS-WA  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

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

           OPEN    I-O NOTIFY-FILE
           IF    WS-FS-NTF = "35"
               OPEN    OUTPUT NOTIFY-FILE
               CLOSE   NOTIFY-FILE
               OPEN    I-O NOTIFY-FILE
           END-IF

           OPEN    INPUT CUSTOMER-FILE
           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00" AND WS-FS-TOK = "00"
                 AND WS-FS-NTF = "00" AND WS-FS-CUS = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    QNS-STATUS
               MOVE    "Arquivos do envio indisponíveis."
                                                 TO    QNS-MESSAGE
           END-IF.

      *> CHECK-SENDABLE: only an open or already sent quote goes out; one past its validity becomes EXPIRADO instead.
       CHECK-SENDABLE.
           MOVE    QTS-ID                        TO    QNS-ID
           MOVE    QTS-REV                       TO    QNS-REV
           IF    QTS-QSTATUS NOT = "ABERTO" AND QTS-QSTATUS NOT = "ENVIADO"
               MOVE    "NOTOPEN"                 TO    QNS-STATUS
               MOVE    "Esse orçamento não pode mais ser enviado."
                                                 TO    QNS-MESSAGE
           ELSE
               IF    QTS-VALID-UNTIL < WS-NOW-DATE
                   MOVE    "EXPIRADO"            TO    QUO-STATUS
                   PERFORM UPDATE-QUOTE-ROW
                   MOVE    "NOTOPEN"             TO    QNS-STATUS
                   MOVE    "O orçamento venceu e foi marcado como expirado."
                                                 TO    QNS-MESSAGE
               END-IF
           END-IF.

       READ-CUSTOMER.
           MOVE    QTS-CUS-ID                    TO    CUS-ID
           PERFORM GET-CUSTOMER-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QNS-STATUS
               MOVE    "Cliente do orçamento não encontrado."
                                                 TO    QNS-MESSAGE
           END-IF.

      *> MAKE-TOKEN: the older tokens of the quote stop working, then a new token and a 6-digit code are made and kept.
      *> The simulated version draws them with the intrinsic RANDOM, seeded from the clock; a production Backend must
      *> draw them from a secure source (see the notes of the project).
       MAKE-TOKEN.
           MOVE    QSN-ID                        TO    WS-REQ-ID
           PERFORM INVALIDATE-OLD-TOKENS

           MOVE    "TOKEN"                       TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           COMPUTE WS-SEED = WS-SEQ-GOT * 7919 + WS-NOW-TIME * 31 + QSN-ID
           COMPUTE WS-R = FUNCTION RANDOM (WS-SEED)

           MOVE    SPACES                        TO    TOK-TOKEN
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 32
               COMPUTE WS-R = FUNCTION RANDOM
               COMPUTE WS-PICK = WS-R * 32 + 1
               MOVE    WS-ALPHABET (WS-PICK:1)   TO    TOK-TOKEN (WS-POS:1)
           END-PERFORM

           COMPUTE WS-R = FUNCTION RANDOM
           COMPUTE WS-CODE-N = WS-R * 900000 + 100000
           MOVE    WS-CODE-N                     TO    TOK-CODE
           MOVE    QSN-ID                        TO    TOK-QUO-ID
           MOVE    QTS-REV                       TO    TOK-REV
           COMPUTE TOK-EXPIRES = QTS-VALID-UNTIL * 1000000 + 235959
           MOVE    SPACE                         TO    TOK-USED
           PERFORM INSERT-TOKEN-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QNS-STATUS
               MOVE    "Não foi possível gravar o link de aprovação."
                                                 TO    QNS-MESSAGE
           END-IF

           MOVE    TOK-TOKEN                     TO    QNS-TOKEN
           MOVE    TOK-CODE                      TO    QNS-CODE
           MOVE    TOK-EXPIRES                   TO    QNS-EXPIRES.

      *> INVALIDATE-OLD-TOKENS: every unused token of quote WS-REQ-ID is marked I (replaced). A statement on
      *> TOKEN-FILE ends the rows a SELECT answered, so the tokens are taken 100 at a time into WS-HELD-TOKEN by
      *> HOLD-OLD-TOKENS, and each one is then read again and rewritten.
       INVALIDATE-OLD-TOKENS.
           MOVE    "N"                           TO    WS-EOF
           MOVE    0                             TO    WS-HANDLED-COUNT
           PERFORM UNTIL WS-EOF = "Y"
               PERFORM HOLD-OLD-TOKENS
               PERFORM VARYING WS-HELD-INDEX FROM 1 BY 1 UNTIL WS-HELD-INDEX > WS-HELD-COUNT
                   MOVE    WS-HELD-TOKEN (WS-HELD-INDEX)
                                                 TO    TOK-TOKEN
                   PERFORM GET-TOKEN-ROW
                   IF    WS-PQ-STATUS = "00"
                       IF    TOK-QUO-ID NOT = WS-REQ-ID
                           MOVE    "Y"           TO    WS-EOF
                       ELSE
                           IF    TOK-USED = SPACE
                               MOVE    "I"       TO    TOK-USED
                               PERFORM UPDATE-TOKEN-ROW
                           END-IF
                       END-IF
                   END-IF
               END-PERFORM
           END-PERFORM.

      *> HOLD-OLD-TOKENS: the keys of the next tokens of quote WS-REQ-ID, at most 100, after those already handled.
      *> In:  WS-REQ-ID; WS-HANDLED-COUNT = the tokens already handled.
      *> Out: WS-HELD-TOKEN (1 to WS-HELD-COUNT); WS-HANDLED-COUNT counts them too; WS-EOF = Y when none is left.
       HOLD-OLD-TOKENS.
           MOVE    0                             TO    WS-HELD-COUNT
           MOVE    WS-REQ-ID                     TO    TOK-QUO-ID
           PERFORM SELECT-TOKENS-OF-QUOTE
           PERFORM FETCH-TOKEN-ROW WS-HANDLED-COUNT TIMES
           PERFORM FETCH-TOKEN-ROW
           PERFORM UNTIL WS-PQ-STATUS NOT = "00" OR WS-HELD-COUNT = 100
               ADD     1                         TO    WS-HELD-COUNT
               MOVE    TOK-TOKEN                 TO    WS-HELD-TOKEN (WS-HELD-COUNT)
               IF    WS-HELD-COUNT < 100
                   PERFORM FETCH-TOKEN-ROW
               END-IF
           END-PERFORM
           ADD     WS-HELD-COUNT                 TO    WS-HANDLED-COUNT
           IF    WS-HELD-COUNT < 100 OR WS-HANDLED-COUNT = WS-PQ-COUNT
               MOVE    "Y"                       TO    WS-EOF
           END-IF.

       MAKE-TEXT.
           MOVE    QTS-TOTAL                     TO    WS-TOTAL-ED
           MOVE    SPACES                        TO    QNS-TEXT
           STRING    "Orçamento " QTS-ID " da oficina: " QTS-CURRENCY " " WS-TOTAL-ED
                     ". Aprove em https://oficina.exemplo/aprovar/" QNS-TOKEN " com o código " QNS-CODE
               INTO  QNS-TEXT.

      *> DELIVER: sends the message through one channel, up to 3 tries, and records the result (R18, R19).
       DELIVER.
           MOVE    0                             TO    WS-TRIES
           MOVE    "FAILED"                      TO    WS-RESULT
           PERFORM UNTIL WS-RESULT = "SENT" OR WS-TRIES = 3
               ADD     1                         TO    WS-TRIES
               PERFORM SIMULATED-DELIVERY
           END-PERFORM
           PERFORM RECORD-NOTIFICATION.

      *> SIMULATED-DELIVERY: the only paragraph a real email or WhatsApp provider replaces. Here the delivery
      *> succeeds unless the channel was switched off for the test (WS-DOWN = S).
       SIMULATED-DELIVERY.
           IF    WS-DOWN = "S"
               MOVE    "FAILED"                  TO    WS-RESULT
           ELSE
               MOVE    "SENT"                    TO    WS-RESULT
           END-IF.

       RECORD-NOTIFICATION.
           MOVE    "NOTIFY"                      TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           INITIALIZE NOTIFY-RECORD
           MOVE    WS-SEQ-GOT                    TO    NTF-ID
           MOVE    QSN-ID                        TO    NTF-QUO-ID
           MOVE    WS-CHANNEL                    TO    NTF-CHANNEL
           MOVE    "QUOTE"                       TO    NTF-KIND
           MOVE    WS-TO                         TO    NTF-TO
           MOVE    WS-NOW                        TO    NTF-AT
           MOVE    WS-RESULT                     TO    NTF-STATUS
           MOVE    QNS-TEXT                      TO    NTF-TEXT
           PERFORM INSERT-NOTIFY-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QNS-STATUS
               MOVE    "Não foi possível registrar o envio."
                                                 TO    QNS-MESSAGE
           END-IF.

      *> SET-OUTCOME: the quote is ENVIADO when a channel delivered; the answer says what happened to each channel.
       SET-OUTCOME.
           IF    QNS-EMAIL = "SENT" OR QNS-WA = "SENT"
               MOVE    "ENVIADO"                 TO    QUO-STATUS
               MOVE    QSN-ID                    TO    QUO-ID
               MOVE    QNS-REV                   TO    QUO-REV
               PERFORM GET-QUOTE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "ENVIADO"             TO    QUO-STATUS
                   PERFORM UPDATE-QUOTE-ROW
               END-IF

               IF    QNS-EMAIL = "FAILED" OR QNS-WA = "FAILED"
                   MOVE    "Enviado por um canal; o outro falhou. Tente de novo depois."
                                                 TO    QNS-MESSAGE
               ELSE
                   MOVE    "Orçamento enviado."  TO    QNS-MESSAGE
               END-IF
           ELSE
               IF    QNS-EMAIL = "FAILED" OR QNS-WA = "FAILED"
                   MOVE    "NOCHANNEL"           TO    QNS-STATUS
                   MOVE    "Nenhum canal entregou o orçamento."
                                                 TO    QNS-MESSAGE
               ELSE
                   MOVE    "NOCHANNEL"           TO    QNS-STATUS
                   MOVE    "O cliente não autorizou nenhum canal. Registre a aprovação à mão."
                                                 TO    QNS-MESSAGE
               END-IF
           END-IF

      *> A token nobody received is not left alive.
           IF    QNS-STATUS = "NOCHANNEL"
               MOVE    QNS-TOKEN                 TO    TOK-TOKEN
               PERFORM GET-TOKEN-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "I"                   TO    TOK-USED
                   PERFORM UPDATE-TOKEN-ROW
               END-IF
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-tokens".
           COPY "backend/COPYBOOKS/pq-notifications".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

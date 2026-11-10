       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-VIEW.
      *> The customer's approval page asks for the quote it was sent (R37). The link token (or the quote number) and the
      *> short code from the message must match a token that is still valid: not used, not replaced by a newer sending
      *> and not past its expiry. A wrong code, a used, replaced or expired token answers only a status, never the quote.
      *> Looking at the quote does not use the token; only answering it does (QUOTE-APPROVE).
      *> The request is backend/COPYBOOKS/req-quote-view.cpy and the answer res-quote.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-send".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/tokens.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/approvals.FD".
           COPY "COPYBOOKS/customers.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-token".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-TIME                       PIC    9(6).
          05 WS-CLOCK-REST                       PIC    X(7).
       01 WS-NOW-G.
          05 WS-NOW-DATE                         PIC    9(8).
          05 WS-NOW-TIME                         PIC    9(6).
       01 WS-NOW REDEFINES WS-NOW-G              PIC    9(14).

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-view".
           COPY "backend/COPYBOOKS/res-quote".

       PROCEDURE DIVISION USING QVW-REQ RETURNING QUO-RES.
       MAIN-PARA.
           INITIALIZE QUO-RES
           MOVE    "NOTFOUND"                    TO    QTS-STATUS
           MOVE    "Link ou código não confere."  TO   QTS-MESSAGE
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-NOW-DATE
           MOVE    WS-CLOCK-TIME                 TO    WS-NOW-TIME

           OPEN    INPUT TOKEN-FILE
           OPEN    INPUT QUOTE-FILE
           OPEN    INPUT QUOTE-ITEM-FILE
           IF    WS-FS-TOK = "00" AND WS-FS-QUO = "00" AND WS-FS-QIT = "00"
               MOVE    QVW-TOKEN                 TO    WS-REQ-TOKEN
               MOVE    QVW-QUO-ID                TO    WS-REQ-ID
               MOVE    QVW-CODE                  TO    WS-REQ-CODE
               PERFORM RESOLVE-TOKEN

               IF    WS-TK-STATUS = "OK"
                   MOVE    TOK-QUO-ID            TO    WS-LQ-ID
                   MOVE    TOK-REV               TO    WS-LQ-REV
                   PERFORM LOAD-QUOTE
                   IF    WS-LQ-OK = 1
                       MOVE    "OK"              TO    QTS-STATUS
                       MOVE    SPACES            TO    QTS-MESSAGE
                   END-IF
               ELSE
                   MOVE    WS-TK-STATUS          TO    QTS-STATUS
                   EVALUATE WS-TK-STATUS
                       WHEN "BADCODE"
                           MOVE    "O código não confere."  TO QTS-MESSAGE
                       WHEN "USED"
                           MOVE    "Esse link já foi usado."  TO QTS-MESSAGE
                       WHEN "REPLACED"
                           MOVE    "Existe um link mais novo para esse orçamento."
                                                 TO    QTS-MESSAGE
                       WHEN "EXPIRED"
                           MOVE    "O link venceu. Peça um novo à oficina."
                                                 TO    QTS-MESSAGE
                       WHEN OTHER
                           CONTINUE
                   END-EVALUATE
               END-IF
               CLOSE   TOKEN-FILE
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
           END-IF

           GOBACK.

           COPY "backend/COPYBOOKS/pd-token".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-tokens".

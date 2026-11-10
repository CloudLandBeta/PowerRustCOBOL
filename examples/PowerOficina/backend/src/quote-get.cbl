       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-GET.
      *> Answers one quote with its lines: the header, the taxes of every line, the total without taxes (subtotal) and
      *> the total with taxes (R16, R38). Revision 0 means the latest revision; an earlier revision is read by its
      *> number, so the history stays visible (R17). Values are shown in the quote's own currency and never converted.
      *> The request and the answer are in backend/COPYBOOKS/req-quote-get.cpy and res-quote.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-get".
           COPY "backend/COPYBOOKS/res-quote".

       PROCEDURE DIVISION USING QGT-REQ RETURNING QUO-RES.
       MAIN-PARA.
           INITIALIZE QUO-RES
           MOVE    "NOTFOUND"                    TO    QTS-STATUS
           MOVE    "Orçamento não encontrado."   TO    QTS-MESSAGE

           OPEN    INPUT QUOTE-FILE
           OPEN    INPUT QUOTE-ITEM-FILE
           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00"
               MOVE    QGQ-ID                    TO    WS-LQ-ID
               MOVE    QGQ-REV                   TO    WS-LQ-REV
               PERFORM LOAD-QUOTE
               IF    WS-LQ-OK = 1
                   MOVE    "OK"                  TO    QTS-STATUS
                   MOVE    SPACES                TO    QTS-MESSAGE
      *> An open or sent quote past its validity is shown as expired even before QUOTE-EXPIRE marks it (R20).
                   MOVE    FUNCTION CURRENT-DATE TO    WS-CLOCK
                   IF    (QTS-QSTATUS = "ABERTO" OR QTS-QSTATUS = "ENVIADO")
                         AND QTS-VALID-UNTIL < WS-CLOCK-DATE
                       MOVE    "EXPIRADO"        TO    QTS-QSTATUS
                   END-IF
               END-IF
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
           END-IF

           GOBACK.

           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".

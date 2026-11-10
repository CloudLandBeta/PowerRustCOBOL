       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-EXPIRE.
      *> Marks as EXPIRADO every quote that is still open or sent and whose validity has ended without an answer (R20).
      *> The platform has no scheduler yet, so the screens call this program when they open; running it again does
      *> nothing to quotes already marked. The count of quotes marked goes in the answer and on the audit trail.
      *> The request and the answer are in backend/COPYBOOKS/req-quote-expire.cpy and res-quote-expire.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT QUOTE-FILE ASSIGN TO "data/quotes.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QUO-KEY
               ALTERNATE RECORD KEY IS QUO-CUS-ID WITH DUPLICATES
               ALTERNATE RECORD KEY IS QUO-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-QUO.
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-QUO                              PIC    XX.
       01 WS-EOF                                 PIC    X.
       01 WS-HELD-COUNT                          PIC    999.
       01 WS-HANDLED-COUNT                       PIC    9(6).
       01 WS-HELD-INDEX                          PIC    999.
       01 WS-HELD.
          05 WS-HELD-QUOTE                       OCCURS 100.
             10 WS-HELD-QUOTE-ID                 PIC    9(8).
             10 WS-HELD-QUOTE-REV                PIC    99.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-expire".
           COPY "backend/COPYBOOKS/res-quote-expire".

       PROCEDURE DIVISION USING QEX-REQ RETURNING QEX-RES.
       MAIN-PARA.
           INITIALIZE QEX-RES
           MOVE    "OK"                          TO    QEY-STATUS
           PERFORM NOW-STAMP

           OPEN    I-O QUOTE-FILE
           IF    WS-FS-QUO = "35"
               CONTINUE
           ELSE
               IF    WS-FS-QUO NOT = "00"
                   MOVE    "ERROR"               TO    QEY-STATUS
               ELSE
                   MOVE    "N"                   TO    WS-EOF
                   MOVE    0                     TO    WS-HANDLED-COUNT

                   PERFORM UNTIL WS-EOF = "Y"
                       PERFORM HOLD-QUOTES
                       PERFORM VARYING WS-HELD-INDEX FROM 1 BY 1 UNTIL WS-HELD-INDEX > WS-HELD-COUNT
                           MOVE    WS-HELD-QUOTE-ID (WS-HELD-INDEX)
                                                 TO    QUO-ID
                           MOVE    WS-HELD-QUOTE-REV (WS-HELD-INDEX)
                                                 TO    QUO-REV
                           PERFORM GET-QUOTE-ROW
                           IF    WS-PQ-STATUS = "00"
                               IF    (QUO-STATUS = "ABERTO" OR QUO-STATUS = "ENVIADO")
                                     AND QUO-VALID-UNTIL < WS-NOW-DATE
                                   MOVE    "EXPIRADO"
                                                 TO    QUO-STATUS
                                   PERFORM UPDATE-QUOTE-ROW
                                   ADD     1     TO    QEY-COUNT
                               END-IF
                           END-IF
                       END-PERFORM
                   END-PERFORM
                   CLOSE   QUOTE-FILE
               END-IF
           END-IF

           IF    QEY-COUNT > 0
               IF    QEX-OPERATOR = SPACES
                   MOVE    "SISTEMA"             TO    WS-AU-OPERATOR
               ELSE
                   MOVE    QEX-OPERATOR          TO    WS-AU-OPERATOR
               END-IF
               MOVE    "QUOTE"                   TO    WS-AU-ENTITY
               MOVE    SPACES                    TO    WS-AU-REF
               MOVE    "EXPIRE"                  TO    WS-AU-ACTION
               STRING    QEY-COUNT " expired"  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

      *> HOLD-QUOTES: the keys of the next quote revisions, at most 100, after those already handled. A statement on
      *> QUOTE-FILE ends the rows a SELECT answered, so MAIN-PARA takes the keys first and then reads each revision
      *> again and rewrites it.
      *> In:  WS-HANDLED-COUNT = the revisions already handled.
      *> Out: WS-HELD-QUOTE (1 to WS-HELD-COUNT); WS-HANDLED-COUNT counts them too; WS-EOF = Y when none is left.
       HOLD-QUOTES.
           MOVE    0                             TO    WS-HELD-COUNT
           PERFORM SELECT-ALL-QUOTES
           PERFORM FETCH-QUOTE-ROW WS-HANDLED-COUNT TIMES
           PERFORM FETCH-QUOTE-ROW
           PERFORM UNTIL WS-PQ-STATUS NOT = "00" OR WS-HELD-COUNT = 100
               ADD     1                         TO    WS-HELD-COUNT
               MOVE    QUO-ID                    TO    WS-HELD-QUOTE-ID (WS-HELD-COUNT)
               MOVE    QUO-REV                   TO    WS-HELD-QUOTE-REV (WS-HELD-COUNT)
               IF    WS-HELD-COUNT < 100
                   PERFORM FETCH-QUOTE-ROW
               END-IF
           END-PERFORM
           ADD     WS-HELD-COUNT                 TO    WS-HANDLED-COUNT
           IF    WS-HELD-COUNT < 100 OR WS-HANDLED-COUNT = WS-PQ-COUNT
               MOVE    "Y"                       TO    WS-EOF
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

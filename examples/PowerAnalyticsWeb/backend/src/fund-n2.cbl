       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-N2.
      *> Q-N2 of the desktop: the funds per level-2 category under the filter, the nine largest in their own slot
      *> and the rest summed into slot 10, "Outros". The statement is the desktop's text ("FROM fundos " + OPN-WHERE,
      *> values bound, ORDER BY 2 DESC and nothing more), so the categories that tie on their count come back in
      *> the order the desktop's SQLite gives them for the same filter, not in an order of this program's choosing.
      *> Exported, idempotent: it only reads.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-script".

       DATA DIVISION.
       FILE SECTION.
           COPY "backend/COPYBOOKS/fd-script".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-open".
       01 WS-C1                                  PIC    X(40).
       01 WS-C2                                  PIC    X(40).
       01 WS-V                                   PIC    9(9).
       01 WS-I                                   PIC    99.
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-n2".

       PROCEDURE DIVISION USING FLT-REQ RETURNING N2R-RES.
       MAIN-PARA.
           INITIALIZE N2R-RES
           MOVE    "OK"                          TO    N2R-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    N2R-STATUS
               MOVE    OPN-MESSAGE               TO    N2R-MESSAGE
               GOBACK
           END-IF
           MOVE    0                             TO    WS-I

           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT nivel2, COUNT(*) FROM fundos " OPN-WHERE::Trim()
                   " AND nivel2 IS NOT NULL GROUP BY nivel2 ORDER BY 2 DESC"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-N2 FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-N2 CURSOR FOR S-N2 END-EXEC
           EXEC SQL OPEN C-N2 USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM N2R-FAIL
               GOBACK
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-N2 INTO :WS-C1, :WS-C2 END-EXEC
               IF SQLSTATE = "00000"
                   COMPUTE WS-V = FUNCTION NUMVAL(WS-C2)
                   ADD     WS-V                  TO    N2R-TOTAL

      *> Largest first: the 9 biggest keep their own slot, the rest are summed into slot 10, "Outros".
      *> N2R-TOTAL adds every row, so the donut shares add up to the whole.
                   IF WS-I < 9
                       ADD     1                 TO    WS-I
                       MOVE    WS-C1             TO    N2R-NAME (WS-I)
                       MOVE    WS-V              TO    N2R-CNT (WS-I)
                   ELSE
                       MOVE    "Outros"          TO    N2R-NAME (10)
                       ADD     WS-V              TO    N2R-CNT (10)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-N2 END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM N2R-FAIL
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       N2R-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    N2R-STATUS
           MOVE    OPN-MESSAGE                   TO    N2R-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-BINS.
      *> Q-BINS of the desktop: the funds that report a PL, in five bands each ten times the last. The
      *> statement is the desktop's, with "FROM fv". Exported, idempotent: it only reads.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-script".

       DATA DIVISION.
       FILE SECTION.
           COPY "backend/COPYBOOKS/fd-script".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-open".
       01 WS-BAND                                PIC    99.
       01 WS-V                                   PIC    9(9).
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-bins".

       PROCEDURE DIVISION USING FLT-REQ RETURNING BIN-RES.
       MAIN-PARA.
           INITIALIZE BIN-RES
           MOVE    "OK"                          TO    BIN-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    BIN-STATUS
               MOVE    OPN-MESSAGE               TO    BIN-MESSAGE
               GOBACK
           END-IF

      *> Five PL bands, each ten times the last, as the captions of LBL-BIN1-N to LBL-BIN5-N say (up to R$ 1 mi
      *> ... above R$ 1 bi). The CASE number indexes BIN-CNT.
           EXEC SQL
               DECLARE C-BINS CURSOR FOR
                   SELECT CASE WHEN pl < 1000000 THEN 1 WHEN pl < 10000000 THEN 2
                               WHEN pl < 100000000 THEN 3 WHEN pl < 1000000000 THEN 4 ELSE 5 END,
                          COUNT(*)
                     FROM fv WHERE pl IS NOT NULL GROUP BY 1
           END-EXEC
           EXEC SQL OPEN C-BINS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-BINS INTO :WS-BAND, :WS-V END-EXEC
               IF SQLSTATE = "00000"
                   IF WS-BAND >= 1 AND WS-BAND <= 5
                       MOVE    WS-V              TO    BIN-CNT (WS-BAND)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-BINS END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM BIN-FAIL
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       BIN-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    BIN-STATUS
           MOVE    OPN-MESSAGE                   TO    BIN-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

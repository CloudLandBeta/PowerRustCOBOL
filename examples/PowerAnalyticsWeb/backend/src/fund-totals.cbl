       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-TOTALS.
      *> Q-TOTALS of the desktop: the totals of the funds that pass the filter. The two statements are the
      *> desktop's, with "FROM fv" where it writes "FROM fundos <WS-WHERE>".
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
       01 WS-ESTR                                PIC    X(40).
       01 WS-SUMPL                               PIC    9(15)V99.
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-totals".

       PROCEDURE DIVISION USING FLT-REQ RETURNING TOT-RES.
       MAIN-PARA.
           INITIALIZE TOT-RES
           MOVE    "OK"                          TO    TOT-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    TOT-STATUS
               MOVE    OPN-MESSAGE               TO    TOT-MESSAGE
               GOBACK
           END-IF

      *> One row of totals. COUNT(pl) counts only the funds that report a PL and is the divisor for the average
      *> PL; MAX(ano) anchors the 12-year window of FUND-YEARS.
           EXEC SQL
               SELECT COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0), COUNT(pl),
                      COALESCE(MAX(pl),0), COALESCE(MAX(ano),0)
                 INTO :TOT-CNT, :TOT-PL, :TOT-COT, :TOT-PLN, :TOT-MAX, :TOT-YMAX
                 FROM fv
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOT-FAIL
               GOBACK
           END-IF

      *> PL of the "Classe" funds and of the others, one row per estrutura. The desktop assigns, not adds:
      *> if several estruturas other than "Classe" came back, the last one would stay.
           EXEC SQL
               DECLARE C-ESTR CURSOR FOR
                   SELECT COALESCE(estrutura,''), COALESCE(SUM(pl),0) FROM fv GROUP BY estrutura
           END-EXEC
           EXEC SQL OPEN C-ESTR END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-ESTR INTO :WS-ESTR, :WS-SUMPL END-EXEC
               IF SQLSTATE = "00000"
                   IF WS-ESTR = "Classe"
                       MOVE    WS-SUMPL          TO    TOT-CLS
                   ELSE
                       MOVE    WS-SUMPL          TO    TOT-SUB
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-ESTR END-EXEC
           PERFORM OPN-CLOSE
           GOBACK.

       TOT-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    TOT-STATUS
           MOVE    OPN-MESSAGE                   TO    TOT-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

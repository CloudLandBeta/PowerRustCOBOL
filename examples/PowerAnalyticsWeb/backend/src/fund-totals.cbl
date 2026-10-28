       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-TOTALS.
      *> Q-TOTALS of the desktop: the totals of the funds that pass the filter. The two statements are the
      *> desktop's text, "FROM fundos " + the WHERE of BUILD-WHERE (OPN-WHERE, values bound). Each cell is read as
      *> TEXT, as the desktop's Fetch() gives it (a REAL in its shortest decimal form: 441.78999999999996), and
      *> stored with COMPUTE ... = FUNCTION NUMVAL, as Q-TOTALS stores it: NUMVAL reads the text as a binary
      *> double, so 441.78999999999996 is stored as 441.79 on both sides; an exact decimal read would keep 441.78.
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
      *> WS-C1 .. WS-C6: the cells of a row (UNSTRING of the desktop's row into X(40) items). WS-IND-1: NULL estrutura.
       01 WS-C1                                  PIC    X(40).
       01 WS-C2                                  PIC    X(40).
       01 WS-C3                                  PIC    X(40).
       01 WS-C4                                  PIC    X(40).
       01 WS-C5                                  PIC    X(40).
       01 WS-C6                                  PIC    X(40).
       01 WS-IND-1                               PIC    S9(4)    COMP-5.
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
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0), COUNT(pl), "
                   "COALESCE(MAX(pl),0), COALESCE(MAX(ano),0) FROM fundos " OPN-WHERE::Trim()
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-TOT FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-TOT CURSOR FOR S-TOT END-EXEC
           EXEC SQL OPEN C-TOT USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOT-FAIL
               GOBACK
           END-IF
           MOVE    SPACES                        TO    WS-C1 WS-C2 WS-C3 WS-C4 WS-C5 WS-C6
           EXEC SQL FETCH C-TOT INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4, :WS-C5, :WS-C6 END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOT-FAIL
               GOBACK
           END-IF
           EXEC SQL CLOSE C-TOT END-EXEC
           COMPUTE TOT-CNT = FUNCTION NUMVAL(WS-C1)
           COMPUTE TOT-PL = FUNCTION NUMVAL(WS-C2)
           COMPUTE TOT-COT = FUNCTION NUMVAL(WS-C3)
           COMPUTE TOT-PLN = FUNCTION NUMVAL(WS-C4)
           COMPUTE TOT-MAX = FUNCTION NUMVAL(WS-C5)
           COMPUTE TOT-YMAX = FUNCTION NUMVAL(WS-C6)

      *> PL of the "Classe" funds and of the others, one row per estrutura (a NULL estrutura reads as blank, as
      *> Fetch() gives it). The desktop assigns, not adds: of several estruturas other than "Classe" the last stays.
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT estrutura, COALESCE(SUM(pl),0) FROM fundos " OPN-WHERE::Trim() " GROUP BY estrutura"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-ESTR FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-ESTR CURSOR FOR S-ESTR END-EXEC
           EXEC SQL OPEN C-ESTR USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOT-FAIL
               GOBACK
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               MOVE    SPACES                    TO    WS-C1 WS-C2
               EXEC SQL FETCH C-ESTR INTO :WS-C1:WS-IND-1, :WS-C2 END-EXEC
               IF SQLSTATE = "00000"
                   IF WS-C1 = "Classe"
                       COMPUTE TOT-CLS = FUNCTION NUMVAL(WS-C2)
                   ELSE
                       COMPUTE TOT-SUB = FUNCTION NUMVAL(WS-C2)
                   END-IF
               END-IF
           END-PERFORM
           IF SQLSTATE(1:2) > "02"
               PERFORM TOT-FAIL
               GOBACK
           END-IF
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

       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-YEARS.
      *> Q-YEARS of the desktop: the 12-year table behind the yearly charts, for all categories and for the three
      *> with most funds under the filter. The desktop reads two inputs from earlier queries (WS-T-YMAX of Q-TOTALS,
      *> WS-CAT-CNT of Q-CATS); a stateless Backend asks for them itself, with the statements of those queries.
      *> Every statement is the desktop's text, "FROM fundos " + OPN-WHERE (values bound), with a ? for the first year
      *> of the window and one ? per name of the desktop's IN (...) list of leading categories (one blank name when
      *> there is none, the desktop's ''). Each cell is read as text and stored with NUMVAL, as Q-YEARS stores it.
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
       01 WS-CATCNT                              PIC    99.
       01 WS-CAT-TAB.
          05 WS-CAT-ROW                          OCCURS 10.
             10 WS-CAT-NAME                      PIC    X(24).
             10 WS-CAT-CNT                       PIC    9(9).
             10 WS-CAT-USED                      PIC    9.
      *> WS-C1 .. WS-C5: the cells of a row, as text (the desktop's UNSTRING into X(40) items).
       01 WS-C1                                  PIC    X(40).
       01 WS-C2                                  PIC    X(40).
       01 WS-C3                                  PIC    X(40).
       01 WS-C4                                  PIC    X(40).
       01 WS-C5                                  PIC    X(40).
       01 WS-YMAX                                PIC    9(4).
       01 WS-I                                   PIC    99.
       01 WS-K                                   PIC    99.
       01 WS-BEST                                PIC    9(9).
       01 WS-BI                                  PIC    99.
       01 WS-Y                                   PIC    9(4).
      *> The tail " AND ano >= ? AND nivel1 IN (?,?,?)" of the leaders' statements: as many ? as leading categories.
       01 WS-LIST                                PIC    X(20).
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-years".

       PROCEDURE DIVISION USING FLT-REQ RETURNING YRS-RES.
       MAIN-PARA.
           INITIALIZE YRS-RES
           MOVE    "OK"                          TO    YRS-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    YRS-STATUS
               MOVE    OPN-MESSAGE               TO    YRS-MESSAGE
               GOBACK
           END-IF
           PERFORM YRS-READ-INPUTS
           IF YRS-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM YRS-PICK-TOP3
           PERFORM YRS-OPEN-WINDOW
           PERFORM YRS-READ-ALL
           IF YRS-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM YRS-READ-LEADERS
           IF YRS-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

      *> The two inputs of Q-YEARS: the category slots with their funds under the filter (Q-CATS' statement) and the
      *> newest start year (the last column of Q-TOTALS' first statement).
       YRS-READ-INPUTS.
           EXEC SQL DECLARE C-SLOTS CURSOR FOR SELECT name FROM slot ORDER BY n END-EXEC
           EXEC SQL OPEN C-SLOTS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-SLOTS INTO :WS-C1 END-EXEC
               IF SQLSTATE = "00000"
                   ADD     1                     TO    WS-CATCNT
                   MOVE    WS-C1                 TO    WS-CAT-NAME (WS-CATCNT)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-SLOTS END-EXEC
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND nivel1 IS NOT NULL GROUP BY nivel1"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-CATS FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-CATS CURSOR FOR S-CATS END-EXEC
           EXEC SQL OPEN C-CATS USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-CATS INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-CATCNT
                       IF WS-CAT-NAME (WS-I) = WS-C1
                           COMPUTE WS-CAT-CNT (WS-I) = FUNCTION NUMVAL(WS-C2)
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-CATS END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0), COUNT(pl), "
                   "COALESCE(MAX(pl),0), COALESCE(MAX(ano),0) FROM fundos " OPN-WHERE::Trim()
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-TOT FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-TOT CURSOR FOR S-TOT END-EXEC
           EXEC SQL OPEN C-TOT USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           MOVE    SPACES                        TO    WS-C1 WS-C2 WS-C3 WS-C4 WS-C5 OPN-PV
           EXEC SQL FETCH C-TOT INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4, :WS-C5, :OPN-PV END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           EXEC SQL CLOSE C-TOT END-EXEC
           COMPUTE WS-YMAX = FUNCTION NUMVAL(OPN-PV).

      *> Top 3 categories by fund count: three passes, each takes the largest WS-CAT-CNT whose WS-CAT-USED mark is
      *> still 0 (a category with no fund is never taken; ties go to the lower slot). The marks are cleared after.
       YRS-PICK-TOP3.
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
               MOVE    0                         TO    WS-BI
               MOVE    0                         TO    WS-BEST
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-CATCNT
                   IF WS-CAT-CNT (WS-I) > WS-BEST AND WS-CAT-USED (WS-I) = 0
                       MOVE    WS-CAT-CNT (WS-I) TO    WS-BEST
                       MOVE    WS-I              TO    WS-BI
                   END-IF
               END-PERFORM
               IF WS-BI > 0
                   MOVE    WS-BI                 TO    YRS-TOP3 (WS-K)
                   MOVE    1                     TO    WS-CAT-USED (WS-BI)
               END-IF
           END-PERFORM
      *> The IN (...) list of the leading categories: one ? per category found, bound to its name trimmed; with
      *> none, one ? bound to a blank name, which matches nothing, as the desktop's ''.
           MOVE    SPACES                        TO    WS-LIST
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
               IF YRS-TOP3 (WS-K) > 0
                   IF WS-LIST = SPACES
                       MOVE    "?"               TO    WS-LIST
                   ELSE
                       STRING  WS-LIST::Trim() ",?" DELIMITED BY SIZE INTO WS-LIST
                   END-IF
               END-IF
           END-PERFORM
           IF WS-LIST = SPACES
               MOVE    "?"                       TO    WS-LIST
           END-IF.

      *> The values of the leaders' statements after those of OPN-WHERE: the first year, then the names.
       YRS-LEADER-PARAMS.
           PERFORM OPN-WHERE-PARAMS
           MOVE    WS-Y                          TO    OPN-PV
           PERFORM OPN-ADD-PARAM
           MOVE    0                             TO    WS-BI
           PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
               IF YRS-TOP3 (WS-K) > 0
                   MOVE    WS-CAT-NAME (YRS-TOP3 (WS-K))::Trim() TO OPN-PV
                   PERFORM OPN-ADD-PARAM
                   ADD     1                     TO    WS-BI
               END-IF
           END-PERFORM
           IF WS-BI = 0
               MOVE    SPACES                    TO    OPN-PV
               PERFORM OPN-ADD-PARAM
           END-IF.

      *> The window is the 12 years ending at the newest start year in the filter; with no year at all it
      *> starts at 2015. The tables start at 0 (INITIALIZE did it).
       YRS-OPEN-WINDOW.
           IF WS-YMAX > 0
               COMPUTE YRS-MIN = WS-YMAX - 11
           ELSE
               MOVE    2015                      TO    YRS-MIN
           END-IF
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 12
               COMPUTE YRS-NUM (WS-I) = YRS-MIN + WS-I - 1
           END-PERFORM
           MOVE    YRS-MIN                       TO    WS-Y.

      *> All categories: per year in the window, then before the window.
       YRS-READ-ALL.
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT ano, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND ano >= ? GROUP BY ano"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           MOVE    WS-Y                          TO    OPN-PV
           PERFORM OPN-ADD-PARAM
           EXEC SQL PREPARE S-YA FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-YA CURSOR FOR S-YA END-EXEC
           EXEC SQL OPEN C-YA USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YA INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   COMPUTE WS-Y = FUNCTION NUMVAL(WS-C1)
                   COMPUTE WS-I = WS-Y - YRS-MIN + 1
                   IF WS-I >= 1 AND WS-I <= 12
                       COMPUTE YRS-A-CNT (WS-I) = FUNCTION NUMVAL(WS-C2)
                       COMPUTE YRS-A-PL (WS-I) = FUNCTION NUMVAL(WS-C3)
                       COMPUTE YRS-A-COT (WS-I) = FUNCTION NUMVAL(WS-C4)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-YA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           MOVE    YRS-MIN                       TO    WS-Y
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND ano < ?"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           MOVE    WS-Y                          TO    OPN-PV
           PERFORM OPN-ADD-PARAM
           EXEC SQL PREPARE S-YB FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-YB CURSOR FOR S-YB END-EXEC
           EXEC SQL OPEN C-YB USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           MOVE    SPACES                        TO    WS-C1 WS-C2 WS-C3
           EXEC SQL FETCH C-YB INTO :WS-C1, :WS-C2, :WS-C3 END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           EXEC SQL CLOSE C-YB END-EXEC
           COMPUTE YRS-OA-CNT = FUNCTION NUMVAL(WS-C1)
           COMPUTE YRS-OA-PL = FUNCTION NUMVAL(WS-C2)
           COMPUTE YRS-OA-COT = FUNCTION NUMVAL(WS-C3).

      *> The three leading categories: per year in the window, then before the window.
       YRS-READ-LEADERS.
           MOVE    YRS-MIN                       TO    WS-Y
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT ano, nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND ano >= ? AND nivel1 IN (" WS-LIST::Trim() ") GROUP BY ano, nivel1"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM YRS-LEADER-PARAMS
           EXEC SQL PREPARE S-YS FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-YS CURSOR FOR S-YS END-EXEC
           EXEC SQL OPEN C-YS USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YS INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4, :WS-C5 END-EXEC
               IF SQLSTATE = "00000"
                   COMPUTE WS-Y = FUNCTION NUMVAL(WS-C1)
                   COMPUTE WS-I = WS-Y - YRS-MIN + 1
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
                       IF YRS-TOP3 (WS-K) > 0
                           IF WS-CAT-NAME (YRS-TOP3 (WS-K)) = WS-C2 AND WS-I >= 1 AND WS-I <= 12
                               COMPUTE YRS-S-CNT (WS-I, WS-K) = FUNCTION NUMVAL(WS-C3)
                               COMPUTE YRS-S-PL (WS-I, WS-K) = FUNCTION NUMVAL(WS-C4)
                               COMPUTE YRS-S-COT (WS-I, WS-K) = FUNCTION NUMVAL(WS-C5)
                           END-IF
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-YS END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           MOVE    YRS-MIN                       TO    WS-Y
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND ano < ? AND nivel1 IN (" WS-LIST::Trim() ") GROUP BY nivel1"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM YRS-LEADER-PARAMS
           EXEC SQL PREPARE S-YO FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-YO CURSOR FOR S-YO END-EXEC
           EXEC SQL OPEN C-YO USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
               EXIT PARAGRAPH
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YO INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
                       IF YRS-TOP3 (WS-K) > 0
                           IF WS-CAT-NAME (YRS-TOP3 (WS-K)) = WS-C1
                               COMPUTE YRS-OS-CNT (WS-K) = FUNCTION NUMVAL(WS-C2)
                               COMPUTE YRS-OS-PL (WS-K) = FUNCTION NUMVAL(WS-C3)
                               COMPUTE YRS-OS-COT (WS-K) = FUNCTION NUMVAL(WS-C4)
                           END-IF
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-YO END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
           END-IF.

       YRS-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    YRS-STATUS
           MOVE    OPN-MESSAGE                   TO    YRS-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

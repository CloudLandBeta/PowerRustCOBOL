       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-YEARS.
      *> Q-YEARS of the desktop: the 12-year table behind the yearly charts, for all categories and for the three
      *> with most funds under the filter. The desktop reads two inputs from earlier queries (WS-T-YMAX of Q-TOTALS,
      *> WS-CAT-CNT of Q-CATS); a stateless Backend asks for them itself, with the statements of those queries.
      *> The four year statements are the desktop's, with "FROM fv" and the three category names bound (:WS-L1 to
      *> :WS-L3) where the desktop writes them into an IN (...) list; a name left blank matches nothing, as ''.
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
       01 WS-C1                                  PIC    X(40).
       01 WS-C2                                  PIC    9(9).
       01 WS-C3                                  PIC    9(15)V99.
       01 WS-C4                                  PIC    9(12).
       01 WS-NAME                                PIC    X(40).
       01 WS-YMAX                                PIC    9(4).
       01 WS-I                                   PIC    99.
       01 WS-K                                   PIC    99.
       01 WS-BEST                                PIC    9(9).
       01 WS-BI                                  PIC    99.
       01 WS-Y                                   PIC    9(4).
       01 WS-ROWYEAR                             PIC    9(4).
       01 WS-L1                                  PIC    X(24).
       01 WS-L2                                  PIC    X(24).
       01 WS-L3                                  PIC    X(24).
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

      *> The two inputs of Q-YEARS: the category slots with their funds under the filter (Q-CATS) and the newest
      *> start year (the last column of Q-TOTALS' first statement).
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
           EXEC SQL
               DECLARE C-CATS CURSOR FOR
                   SELECT nivel1, COUNT(*) FROM fv WHERE nivel1 IS NOT NULL GROUP BY nivel1
           END-EXEC
           EXEC SQL OPEN C-CATS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-CATS INTO :WS-C1, :WS-C2 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-CATCNT
                       IF WS-CAT-NAME (WS-I) = WS-C1
                           MOVE    WS-C2         TO    WS-CAT-CNT (WS-I)
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-CATS END-EXEC
           EXEC SQL SELECT COALESCE(MAX(ano),0) INTO :WS-YMAX FROM fv END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
           END-IF.

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
           MOVE    SPACES                        TO    WS-L1
           MOVE    SPACES                        TO    WS-L2
           MOVE    SPACES                        TO    WS-L3
           IF YRS-TOP3 (1) > 0
               MOVE    WS-CAT-NAME (YRS-TOP3 (1)) TO   WS-L1
           END-IF
           IF YRS-TOP3 (2) > 0
               MOVE    WS-CAT-NAME (YRS-TOP3 (2)) TO   WS-L2
           END-IF
           IF YRS-TOP3 (3) > 0
               MOVE    WS-CAT-NAME (YRS-TOP3 (3)) TO   WS-L3
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
           EXEC SQL
               DECLARE C-YA CURSOR FOR
                   SELECT ano, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fv
                    WHERE ano >= :WS-Y GROUP BY ano
           END-EXEC
           EXEC SQL OPEN C-YA END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YA INTO :WS-ROWYEAR, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   COMPUTE WS-I = WS-ROWYEAR - YRS-MIN + 1
                   IF WS-I >= 1 AND WS-I <= 12
                       MOVE    WS-C2             TO    YRS-A-CNT (WS-I)
                       MOVE    WS-C3             TO    YRS-A-PL (WS-I)
                       MOVE    WS-C4             TO    YRS-A-COT (WS-I)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-YA END-EXEC
           EXEC SQL
               SELECT COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0)
                 INTO :YRS-OA-CNT, :YRS-OA-PL, :YRS-OA-COT
                 FROM fv WHERE ano < :WS-Y
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM YRS-FAIL
           END-IF.

      *> The three leading categories: per year in the window, then before the window.
       YRS-READ-LEADERS.
           EXEC SQL
               DECLARE C-YS CURSOR FOR
                   SELECT ano, nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fv
                    WHERE ano >= :WS-Y AND nivel1 IN (:WS-L1, :WS-L2, :WS-L3) GROUP BY ano, nivel1
           END-EXEC
           EXEC SQL OPEN C-YS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YS INTO :WS-ROWYEAR, :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   COMPUTE WS-I = WS-ROWYEAR - YRS-MIN + 1
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
                       IF YRS-TOP3 (WS-K) > 0
                           IF WS-CAT-NAME (YRS-TOP3 (WS-K)) = WS-C1 AND WS-I >= 1 AND WS-I <= 12
                               MOVE    WS-C2     TO    YRS-S-CNT (WS-I, WS-K)
                               MOVE    WS-C3     TO    YRS-S-PL (WS-I, WS-K)
                               MOVE    WS-C4     TO    YRS-S-COT (WS-I, WS-K)
                           END-IF
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-YS END-EXEC
           EXEC SQL
               DECLARE C-YO CURSOR FOR
                   SELECT nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fv
                    WHERE ano < :WS-Y AND nivel1 IN (:WS-L1, :WS-L2, :WS-L3) GROUP BY nivel1
           END-EXEC
           EXEC SQL OPEN C-YO END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-YO INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > 3
                       IF YRS-TOP3 (WS-K) > 0
                           IF WS-CAT-NAME (YRS-TOP3 (WS-K)) = WS-C1
                               MOVE    WS-C2     TO    YRS-OS-CNT (WS-K)
                               MOVE    WS-C3     TO    YRS-OS-PL (WS-K)
                               MOVE    WS-C4     TO    YRS-OS-COT (WS-K)
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

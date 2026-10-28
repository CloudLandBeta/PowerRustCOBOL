       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-CATS.
      *> Q-CATS of the desktop: the funds, PL and cotistas of each of the 10 category slots under the filter. The
      *> statement is the desktop's text, "FROM fundos " + the WHERE of BUILD-WHERE (OPN-WHERE, values bound).
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
       01 WS-C3                                  PIC    X(40).
       01 WS-C4                                  PIC    X(40).
       01 WS-I                                   PIC    99.
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-cats".

       PROCEDURE DIVISION USING FLT-REQ RETURNING CAR-RES.
       MAIN-PARA.
           INITIALIZE CAR-RES
           MOVE    "OK"                          TO    CAR-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    CAR-STATUS
               MOVE    OPN-MESSAGE               TO    CAR-MESSAGE
               GOBACK
           END-IF

      *> The slot names, most funds first (LOAD-DIMS): WS-CAT-NAME(1..WS-CATCNT).
           EXEC SQL DECLARE C-SLOTS CURSOR FOR SELECT name FROM slot ORDER BY n END-EXEC
           EXEC SQL OPEN C-SLOTS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-SLOTS INTO :WS-C1 END-EXEC
               IF SQLSTATE = "00000"
                   ADD     1                     TO    CAR-N
                   MOVE    WS-C1                 TO    CAR-NAME (CAR-N)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-SLOTS END-EXEC

      *> A category with no fund in the filter never comes back from the query and keeps the 0 it started with.
      *> The desktop's statement ("FROM fundos " + OPN-WHERE, values bound); each cell read as text and stored with
      *> NUMVAL as Q-CATS stores it (the PL sum is a double added in the order the plan reads the rows).
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT nivel1, COUNT(*), COALESCE(SUM(pl),0), COALESCE(SUM(cotistas),0) FROM fundos "
                   OPN-WHERE::Trim() " AND nivel1 IS NOT NULL GROUP BY nivel1"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-CATS FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-CATS CURSOR FOR S-CATS END-EXEC
           EXEC SQL OPEN C-CATS USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM CAR-FAIL
               GOBACK
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-CATS INTO :WS-C1, :WS-C2, :WS-C3, :WS-C4 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > CAR-N
                       IF CAR-NAME (WS-I) = WS-C1
                           COMPUTE CAR-CNT (WS-I) = FUNCTION NUMVAL(WS-C2)
                           COMPUTE CAR-PL (WS-I) = FUNCTION NUMVAL(WS-C3)
                           COMPUTE CAR-COT (WS-I) = FUNCTION NUMVAL(WS-C4)
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-CATS END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM CAR-FAIL
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       CAR-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    CAR-STATUS
           MOVE    OPN-MESSAGE                   TO    CAR-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

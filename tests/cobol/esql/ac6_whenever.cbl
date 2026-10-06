       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC6.
      *> Spec 087 AC6 - WHENEVER applies in SOURCE order, and an SQL error
      *> with no WHENEVER SQLERROR in force continues.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-N                PIC 9(5).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-STEP             PIC X(20) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC6 END-EXEC
           EXEC SQL CREATE TABLE E (ID INTEGER) END-EXEC
      *> The WHENEVER in LATER-PARA runs first, but this SELECT comes
      *> before it in the source, so it is not affected.
           PERFORM LATER-PARA
           EXEC SQL SELECT ID INTO :WS-N FROM E END-EXEC
           IF SQLSTATE = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL early select " SQLSTATE
           END-IF
      *> No WHENEVER SQLERROR is in force here: the error continues.
           EXEC SQL SELEC 1 END-EXEC
           MOVE "after-error" TO WS-STEP
           IF SQLSTATE = "42601" AND WS-STEP = "after-error"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL error did not continue " SQLSTATE
           END-IF
           PERFORM JUMP-PARA THRU JUMP-EXIT
           DISPLAY "=== ESQL AC6 RESULT ==="
           DISPLAY "forms: WHENEVER NOT FOUND GO TO (source order), "
                   "SELECT INTO with no row before and after it, "
                   "a syntax error with no WHENEVER SQLERROR"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       LATER-PARA.
           EXEC SQL WHENEVER NOT FOUND GO TO NOT-FOUND-PARA END-EXEC.
       JUMP-PARA.
           EXEC SQL SELECT ID INTO :WS-N FROM E END-EXEC
           ADD 1 TO WS-FAIL
           DISPLAY "FAIL no jump after WHENEVER"
           GO TO JUMP-EXIT.
       NOT-FOUND-PARA.
           ADD 1 TO WS-PASS.
       JUMP-EXIT.
           EXIT.

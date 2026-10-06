       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC9.
      *> Spec 087 AC9 / AC14 - CONNECT TO a project SQL connection by its
      *> name. The program never says where SALES is: the project file, the
      *> deployment file beside a built binary, or the IDE decides. Run
      *> under every host (rcrun run, Run Form, an embedded child form, the
      *> compiled binary), it must give the same result.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-ID               PIC 9(5).
       01  WS-NAME             PIC X(20).
       01  WS-COUNT            PIC 9(5).
       01  WS-TOTAL            PIC 9(7).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-T0               PIC 9(9).
       01  WS-T1               PIC 9(9).
       01  WS-MS               PIC 9(9).
       01  WS-NOW              PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           EXEC SQL CONNECT TO 'SALES' END-EXEC
           IF SQLSTATE = "00000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL CONNECT TO 'SALES': " SQLSTATE " " SQLMSG
               PERFORM REPORT-RESULT
               STOP RUN
           END-IF
      *>   The table is the program's own: whatever an earlier run left is
      *>   replaced, so every host starts from the same three rows.
           EXEC SQL DROP TABLE IF EXISTS AC9_PARITY END-EXEC
           EXEC SQL CREATE TABLE AC9_PARITY (ID INTEGER, NAME VARCHAR(20))
           END-EXEC
           EXEC SQL INSERT INTO AC9_PARITY VALUES (1, 'ANA'), (2, 'BRUNO'),
                    (3, 'CARLOS')
           END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL SELECT COUNT(*), SUM(ID) INTO :WS-COUNT, :WS-TOTAL
                      FROM AC9_PARITY
           END-EXEC
           IF SQLSTATE = "00000" AND WS-COUNT = 3 AND WS-TOTAL = 6
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL count " WS-COUNT " sum " WS-TOTAL " " SQLSTATE
           END-IF
           MOVE 2 TO WS-ID
           EXEC SQL SELECT NAME INTO :WS-NAME FROM AC9_PARITY
                     WHERE ID = :WS-ID
           END-EXEC
           IF SQLSTATE = "00000" AND WS-NAME = "BRUNO"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL row 2 read " WS-NAME " " SQLSTATE
           END-IF
           EXEC SQL DISCONNECT ALL END-EXEC
           PERFORM REPORT-RESULT
           STOP RUN.
       REPORT-RESULT.
           PERFORM NOW-MS
           COMPUTE WS-T1 = WS-MS - WS-T0
           DISPLAY "=== ESQL AC9 RESULT ==="
           DISPLAY "forms: CONNECT TO 'SALES' (a project SQL connection, "
                   "by name); DROP/CREATE TABLE; INSERT x3; COMMIT; "
                   "SELECT COUNT(*), SUM(ID) INTO; SELECT ... WHERE ID = "
                   ":WS-ID; DISCONNECT ALL"
           DISPLAY "rows: count " WS-COUNT " sum of ids " WS-TOTAL
           DISPLAY "elapsed ms: " WS-T1
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

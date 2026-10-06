       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC3.
      *> Spec 087 AC3 - NULL with and without an indicator, NULL written
      *> through a negative indicator, and a truncated value.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PHONE            PIC X(10).
       01  WS-PHONE-IND        PIC S9(4) COMP-5.
       01  WS-NOTE             PIC X(10).
       01  WS-LONG             PIC X(10).
       01  WS-LONG-IND         PIC S9(4) COMP-5.
       01  WS-IN               PIC X(10) VALUE "anything".
       01  WS-IN-IND           PIC S9(4) COMP-5 VALUE -1.
       01  WS-N                PIC 9(5).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC3 END-EXEC
           EXEC SQL CREATE TABLE P (ID INTEGER, PHONE TEXT, NOTE TEXT)
           END-EXEC
           EXEC SQL INSERT INTO P VALUES (1, NULL, NULL) END-EXEC
           EXEC SQL INSERT INTO P
               VALUES (2, '123456789012345678901234567890', 'x')
           END-EXEC
      *> 1. NULL into an item with an indicator: -1, item unchanged.
           MOVE "keep" TO WS-PHONE
           EXEC SQL SELECT PHONE INTO :WS-PHONE:WS-PHONE-IND
                      FROM P WHERE ID = 1 END-EXEC
           IF SQLSTATE = "00000" AND WS-PHONE-IND = -1
              AND WS-PHONE = "keep"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL null+indicator " SQLSTATE " " WS-PHONE-IND
                   " " WS-PHONE
           END-IF
      *> 2. NULL into an item without one: 22002.
           EXEC SQL SELECT NOTE INTO :WS-NOTE FROM P WHERE ID = 1
           END-EXEC
           IF SQLSTATE = "22002" AND SQLCODE = -22002
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL null without indicator " SQLSTATE
           END-IF
      *> 3. A negative indicator writes NULL.
           EXEC SQL INSERT INTO P VALUES (3, :WS-IN:WS-IN-IND, 'y')
           END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM P
                     WHERE ID = 3 AND PHONE IS NULL END-EXEC
           IF WS-N = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL indicator -1 did not store NULL"
           END-IF
      *> 4. Thirty characters into PIC X(10): indicator 30, 01004.
           EXEC SQL SELECT PHONE INTO :WS-LONG INDICATOR :WS-LONG-IND
                      FROM P WHERE ID = 2 END-EXEC
           IF SQLSTATE = "01004" AND SQLCODE = 1004
              AND WS-LONG-IND = 30 AND WS-LONG = "1234567890"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL truncation " SQLSTATE " " WS-LONG-IND " "
                   WS-LONG
           END-IF
           DISPLAY "=== ESQL AC3 RESULT ==="
           DISPLAY "forms: SELECT INTO :H:IND (NULL), SELECT INTO :H "
                   "(NULL, no indicator), INSERT VALUES (:H:IND = -1), "
                   "SELECT INTO :H INDICATOR :IND (30 chars into X(10))"
           DISPLAY "rows: 3 inserted; NULL written through indicator "
                   WS-N
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.

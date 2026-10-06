       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC7.
      *> Spec 087 AC7 - a cursor over 10,000 rows fetched to the end, a
      *> positioned UPDATE of every third row, and a closed cursor's 24000.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-ID               PIC 9(6).
       01  WS-AMOUNT           PIC S9(7)V99 COMP-3.
       01  WS-I                PIC 9(6).
       01  WS-FETCHED          PIC 9(6) VALUE 0.
       01  WS-UPDATED          PIC 9(6) VALUE 0.
       01  WS-CHANGED          PIC 9(6).
       01  WS-END-STATE        PIC X(5).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-T0               PIC 9(9).
       01  WS-MS               PIC 9(9).
       01  WS-MS-INSERT        PIC 9(9).
       01  WS-MS-FETCH         PIC 9(9).
       01  WS-MS-UPDATE        PIC 9(9).
       01  WS-RATE             PIC 9(9).
       01  WS-NOW              PIC X(21).
      *> Declared in WORKING-STORAGE: the positioned cursor.
           EXEC SQL
               DECLARE C-UPD CURSOR FOR
                   SELECT ID, AMOUNT FROM ITEMS ORDER BY ID
                   FOR UPDATE OF AMOUNT
           END-EXEC.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC7 END-EXEC
           EXEC SQL CREATE TABLE ITEMS (ID INTEGER PRIMARY KEY,
                    AMOUNT NUMERIC) END-EXEC
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 10000
               EXEC SQL INSERT INTO ITEMS VALUES (:WS-I, 1.50)
               END-EXEC
           END-PERFORM
           EXEC SQL COMMIT END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-INSERT = WS-MS - WS-T0
      *> Declared in the PROCEDURE DIVISION: a read-only cursor.
           EXEC SQL DECLARE C-ALL CURSOR FOR
                    SELECT ID, AMOUNT FROM ITEMS ORDER BY ID END-EXEC
           MOVE WS-MS TO WS-T0
           EXEC SQL OPEN C-ALL END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-ALL INTO :WS-ID, :WS-AMOUNT END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-FETCHED
               END-IF
           END-PERFORM
           MOVE SQLSTATE TO WS-END-STATE
           EXEC SQL CLOSE C-ALL END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-FETCH = WS-MS - WS-T0
           IF WS-FETCHED = 10000 AND WS-END-STATE = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL fetched " WS-FETCHED " ended " WS-END-STATE
           END-IF
      *> Every third row, updated where the cursor stands.
           MOVE WS-MS TO WS-T0
           MOVE 0 TO WS-I
           EXEC SQL OPEN C-UPD END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-UPD INTO :WS-ID, :WS-AMOUNT END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-I
                   IF FUNCTION MOD(WS-I, 3) = 0
                       EXEC SQL UPDATE ITEMS SET AMOUNT = AMOUNT + 1
                                 WHERE CURRENT OF C-UPD END-EXEC
                       ADD 1 TO WS-UPDATED
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-UPD END-EXEC
           EXEC SQL COMMIT END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-UPDATE = WS-MS - WS-T0
           EXEC SQL SELECT COUNT(*) INTO :WS-CHANGED FROM ITEMS
                     WHERE AMOUNT = 2.50 AND ID % 3 = 0 END-EXEC
           IF WS-UPDATED = 3333 AND WS-CHANGED = 3333
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL positioned update " WS-UPDATED " "
                   WS-CHANGED
           END-IF
           EXEC SQL SELECT COUNT(*) INTO :WS-CHANGED FROM ITEMS
                     WHERE AMOUNT = 2.50 END-EXEC
           IF WS-CHANGED = 3333
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL other rows changed too: " WS-CHANGED
           END-IF
      *> A closed cursor: 24000.
           EXEC SQL FETCH C-ALL INTO :WS-ID, :WS-AMOUNT END-EXEC
           IF SQLSTATE = "24000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL fetch on a closed cursor gave " SQLSTATE
           END-IF
           IF WS-MS-FETCH > 0
               COMPUTE WS-RATE = WS-FETCHED * 1000 / WS-MS-FETCH
           ELSE
               MOVE WS-FETCHED TO WS-RATE
           END-IF
           DISPLAY "=== ESQL AC7 RESULT ==="
           DISPLAY "forms: DECLARE CURSOR (WORKING-STORAGE, FOR UPDATE "
                   "OF) and (PROCEDURE DIVISION); OPEN; FETCH INTO to "
                   "02000; CLOSE; UPDATE ... WHERE CURRENT OF; FETCH on "
                   "a closed cursor"
           DISPLAY "rows: inserted 10000 in " WS-MS-INSERT " ms; fetched "
                   WS-FETCHED " in " WS-MS-FETCH " ms (" WS-RATE
                   " rows/s); updated in place " WS-UPDATED " in "
                   WS-MS-UPDATE " ms"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC7B.
      *> Spec 087 AC7 (a second look) - what a cursor does across the
      *> boundaries of the batches a database that keeps cursors itself hands
      *> over (PostgreSQL asks for 1000 rows at a time): a result of 2,500
      *> rows read through, two cursors open at once and read alternately, a
      *> host variable in the cursor's query, a WITH HOLD cursor that goes on
      *> across a COMMIT, a CLOSE and an OPEN again with another value, and a
      *> positioned UPDATE in the middle of a long result. The same program
      *> holds on SQLite and MySQL, which read the rows at OPEN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-ID               PIC 9(6).
       01  WS-PREV             PIC 9(6).
       01  WS-I                PIC 9(6).
       01  WS-N                PIC 9(6).
       01  WS-SUM              PIC 9(12).
       01  WS-G                PIC 9(2).
       01  WS-UP               PIC 9(6).
       01  WS-DOWN             PIC 9(6).
       01  WS-ORDER-OK         PIC 9 VALUE 1.
       01  WS-END-STATE        PIC X(5).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-T0               PIC 9(9).
       01  WS-MS               PIC 9(9).
       01  WS-MS-FILL          PIC 9(9).
       01  WS-MS-READ          PIC 9(9).
       01  WS-RATE             PIC 9(9).
       01  WS-NOW              PIC X(21).
           EXEC SQL DECLARE C-ALL CURSOR FOR
                    SELECT ID FROM BATCHES ORDER BY ID END-EXEC.
           EXEC SQL DECLARE C-DESC CURSOR FOR
                    SELECT ID FROM BATCHES ORDER BY ID DESC END-EXEC.
           EXEC SQL DECLARE C-GRP CURSOR FOR
                    SELECT ID FROM BATCHES WHERE GRP = :WS-G
                    ORDER BY ID END-EXEC.
           EXEC SQL DECLARE C-HOLD CURSOR WITH HOLD FOR
                    SELECT ID FROM BATCHES ORDER BY ID END-EXEC.
           EXEC SQL DECLARE C-POS CURSOR FOR
                    SELECT ID, GRP FROM BATCHES ORDER BY ID
                    FOR UPDATE OF GRP END-EXEC.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC7B END-EXEC
           EXEC SQL CREATE TABLE BATCHES (ID INTEGER PRIMARY KEY,
                    GRP INTEGER) END-EXEC
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 2500
               COMPUTE WS-G = FUNCTION MOD(WS-I, 5)
               EXEC SQL INSERT INTO BATCHES VALUES (:WS-I, :WS-G)
               END-EXEC
           END-PERFORM
           EXEC SQL COMMIT END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-FILL = WS-MS - WS-T0
      *> 1. All 2,500 rows, in order, to NO DATA: three batches of a
      *> database that asks for 1000, and one that ends the cursor.
           MOVE WS-MS TO WS-T0
           MOVE 0 TO WS-N, WS-SUM, WS-PREV
           MOVE 1 TO WS-ORDER-OK
           EXEC SQL OPEN C-ALL END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-ALL INTO :WS-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-N
                   ADD WS-ID TO WS-SUM
                   IF WS-ID NOT = WS-PREV + 1
                       MOVE 0 TO WS-ORDER-OK
                   END-IF
                   MOVE WS-ID TO WS-PREV
               END-IF
           END-PERFORM
           MOVE SQLSTATE TO WS-END-STATE
           EXEC SQL CLOSE C-ALL END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-READ = WS-MS - WS-T0
           IF WS-N = 2500 AND WS-SUM = 3126250 AND WS-ORDER-OK = 1
              AND WS-END-STATE = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL read through: " WS-N " " WS-SUM " "
                   WS-ORDER-OK " " WS-END-STATE
           END-IF
      *> 2. Two cursors open together, read alternately, one up and one
      *> down: neither disturbs the other, across their batch boundaries.
           MOVE 0 TO WS-UP, WS-DOWN, WS-N
           MOVE 1 TO WS-ORDER-OK
           EXEC SQL OPEN C-ALL END-EXEC
           EXEC SQL OPEN C-DESC END-EXEC
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 2500
               EXEC SQL FETCH C-ALL INTO :WS-ID END-EXEC
               IF WS-ID NOT = WS-I
                   MOVE 0 TO WS-ORDER-OK
               END-IF
               EXEC SQL FETCH C-DESC INTO :WS-ID END-EXEC
               IF WS-ID NOT = 2501 - WS-I
                   MOVE 0 TO WS-ORDER-OK
               END-IF
               ADD 1 TO WS-N
           END-PERFORM
           EXEC SQL FETCH C-ALL INTO :WS-ID END-EXEC
           MOVE SQLSTATE TO WS-END-STATE
           EXEC SQL CLOSE C-ALL END-EXEC
           EXEC SQL CLOSE C-DESC END-EXEC
           IF WS-N = 2500 AND WS-ORDER-OK = 1 AND WS-END-STATE = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL two cursors: " WS-N " " WS-ORDER-OK " "
                   WS-END-STATE
           END-IF
      *> 3. A host variable in the query is read at OPEN; the cursor is
      *> closed and opened again with another value.
           MOVE 3 TO WS-G
           MOVE 0 TO WS-N, WS-SUM
           EXEC SQL OPEN C-GRP END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-GRP INTO :WS-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-N
                   ADD WS-ID TO WS-SUM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-GRP END-EXEC
           IF WS-N = 500 AND WS-SUM = 625250
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL host variable in the cursor: " WS-N " "
                   WS-SUM
           END-IF
           MOVE 4 TO WS-G
           MOVE 0 TO WS-N, WS-SUM
           EXEC SQL OPEN C-GRP END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-GRP INTO :WS-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-N
                   ADD WS-ID TO WS-SUM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-GRP END-EXEC
           IF WS-N = 500 AND WS-SUM = 625750
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL reopened with another value: " WS-N " "
                   WS-SUM
           END-IF
      *> 4. WITH HOLD: 1200 rows, a COMMIT, and the cursor goes on from
      *> row 1201 - past the first batch boundary - to the end.
           MOVE 0 TO WS-N, WS-PREV
           MOVE 1 TO WS-ORDER-OK
           EXEC SQL OPEN C-HOLD END-EXEC
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 1200
               EXEC SQL FETCH C-HOLD INTO :WS-ID END-EXEC
               ADD 1 TO WS-N
           END-PERFORM
           EXEC SQL COMMIT END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-HOLD INTO :WS-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-N
                   IF WS-ID NOT = WS-N
                       MOVE 0 TO WS-ORDER-OK
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-HOLD END-EXEC
           IF WS-N = 2500 AND WS-ORDER-OK = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL WITH HOLD across a batch: " WS-N " "
                   WS-ORDER-OK
           END-IF
      *> 5. A positioned UPDATE in the middle of a long result: rows 1,
      *> 1001 and 2001 - each at the start of a batch of a database that
      *> asks for 1000 - get GRP 9, and no other row does.
           MOVE 0 TO WS-I
           EXEC SQL OPEN C-POS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-POS INTO :WS-ID, :WS-G END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-I
                   IF FUNCTION MOD(WS-ID, 1000) = 1
                       EXEC SQL UPDATE BATCHES SET GRP = 9
                                 WHERE CURRENT OF C-POS END-EXEC
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-POS END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM BATCHES
                     WHERE GRP = 9 END-EXEC
           EXEC SQL SELECT SUM(ID) INTO :WS-SUM FROM BATCHES
                     WHERE GRP = 9 END-EXEC
           IF WS-I = 2500 AND WS-N = 3 AND WS-SUM = 3003
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL positioned in a long result: " WS-I " "
                   WS-N " " WS-SUM
           END-IF
           IF WS-MS-READ > 0
               COMPUTE WS-RATE = 2500 * 1000 / WS-MS-READ
           ELSE
               MOVE 2500 TO WS-RATE
           END-IF
           DISPLAY "=== ESQL AC7B RESULT ==="
           DISPLAY "forms: DECLARE CURSOR (plain, host variable, WITH "
                   "HOLD, FOR UPDATE OF); OPEN; FETCH INTO across 2,500 "
                   "rows; two cursors read alternately; CLOSE and OPEN "
                   "again; COMMIT with a cursor held; UPDATE ... WHERE "
                   "CURRENT OF"
           DISPLAY "rows: inserted 2500 in " WS-MS-FILL " ms; read "
                   "through in " WS-MS-READ " ms (" WS-RATE " rows/s)"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

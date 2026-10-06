       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC11.
      *> Spec 087 AC11 - dynamic SQL and the SQL descriptor area: PREPARE
      *> a query whose columns the program does not know, DESCRIBE it into
      *> a descriptor too small and then into one large enough, read every
      *> row through the descriptor - once through the entries' pointers
      *> into the program's items, once as text inside the entries - and
      *> EXECUTE IMMEDIATE / EXECUTE USING.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           EXEC SQL INCLUDE SQLDA END-EXEC.
       01  WS-STMT             PIC X(200).
       01  WS-MIN-QTY          PIC 9(4) VALUE 0.
       01  WS-CODE             PIC X(10).
       01  WS-PRICE            PIC 9(7)V99.
       01  WS-QTY              PIC 9(5).
       01  WS-PTR              USAGE POINTER.
       01  WS-SEEN             PIC X(200) VALUE SPACES.
       01  WS-SEEN-PTR         PIC 9(3) VALUE 1.
       01  WS-ROWS             PIC 9(3) VALUE 0.
       01  WS-MSG              PIC X(20) VALUE "logged".
       01  WS-N                PIC 9(5).
       01  WS-I                PIC 9(3).
       01  SQLSTATE            PIC X(5).
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
      *> Smoke test first: a pointer inside an OCCURS entry.
           SET SQLDA-DATA (1) TO ADDRESS OF WS-CODE
           SET WS-PTR TO ADDRESS OF WS-CODE
           IF SQLDA-DATA (1) = WS-PTR
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL a pointer in an OCCURS entry"
           END-IF
           EXEC SQL CONNECT TO ':memory:' AS AC11 END-EXEC
           EXEC SQL CREATE TABLE PRODUCTS (CODE VARCHAR(10),
                    PRICE NUMERIC(9,2), QTY INTEGER) END-EXEC
           EXEC SQL INSERT INTO PRODUCTS VALUES ('BOLT', 0.25, 900)
           END-EXEC
           EXEC SQL INSERT INTO PRODUCTS VALUES ('NUT', 0.10, 1500)
           END-EXEC
           EXEC SQL INSERT INTO PRODUCTS VALUES ('WASHER', 0.05, 0)
           END-EXEC
           MOVE "SELECT CODE, PRICE, QTY FROM PRODUCTS WHERE QTY > ? "
             & "ORDER BY CODE" TO WS-STMT
           EXEC SQL PREPARE S1 FROM :WS-STMT END-EXEC
      *> Too small: NEEDED says 3, nothing filled, 01005.
           MOVE 2 TO SQLDA-CAPACITY
           MOVE 0 TO SQLDA-COUNT
           EXEC SQL DESCRIBE S1 INTO SQLDA END-EXEC
           IF SQLSTATE = "01005" AND SQLDA-NEEDED = 3
              AND SQLDA-COUNT = 0
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL too small: " SQLSTATE " " SQLDA-NEEDED
           END-IF
           MOVE 100 TO SQLDA-CAPACITY
           EXEC SQL DESCRIBE S1 INTO SQLDA END-EXEC
           IF SQLSTATE = "00000" AND SQLDA-COUNT = 3
              AND SQLDA-NAME (1) = "CODE" AND SQLDA-NAME (2) = "PRICE"
              AND SQLDA-NAME (3) = "QTY"
              AND SQLDA-TYPE (1) = 4 AND SQLDA-LENGTH (1) = 10
              AND SQLDA-TYPE (2) = 2 AND SQLDA-PRECISION (2) = 9
              AND SQLDA-SCALE (2) = 2 AND SQLDA-TYPE (3) = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL describe: " SQLSTATE " " SQLDA-COUNT " "
                   SQLDA-NAME (1) " " SQLDA-TYPE (2)
           END-IF
           EXEC SQL DECLARE C1 CURSOR FOR S1 END-EXEC
      *> Mode 1: through the entries' pointers.
           SET SQLDA-DATA (2) TO ADDRESS OF WS-PRICE
           SET SQLDA-DATA (3) TO ADDRESS OF WS-QTY
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               SET SQLDA-IND-PTR (WS-I) TO NULL
           END-PERFORM
           EXEC SQL OPEN C1 USING :WS-MIN-QTY END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C1 USING DESCRIPTOR SQLDA END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-ROWS
                   STRING FUNCTION TRIM(WS-CODE) ":" WS-PRICE ":" WS-QTY
                          " " DELIMITED BY SIZE INTO WS-SEEN
                          WITH POINTER WS-SEEN-PTR
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C1 END-EXEC
           IF WS-ROWS = 2 AND WS-SEEN =
              "BOLT:000000025:00900 NUT:000000010:01500"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL through pointers: " WS-ROWS " [" WS-SEEN "]"
           END-IF
      *> Mode 2: NULL pointers, the values arrive inside the entries.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               SET SQLDA-DATA (WS-I) TO NULL
           END-PERFORM
           MOVE 1000 TO WS-MIN-QTY
           EXEC SQL OPEN C1 USING :WS-MIN-QTY END-EXEC
           EXEC SQL FETCH C1 USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE = "00000" AND SQLDA-VALUE (1) = "NUT"
              AND SQLDA-VALUE (2) = "0.1" AND SQLDA-VALUE (3) = "1500"
              AND SQLDA-IND (1) = 0
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL inline: " SQLSTATE " [" SQLDA-VALUE (1)
                   "] [" SQLDA-VALUE (2) "] [" SQLDA-VALUE (3) "]"
           END-IF
           EXEC SQL CLOSE C1 END-EXEC
      *> EXECUTE IMMEDIATE and EXECUTE … USING.
           EXEC SQL EXECUTE IMMEDIATE 'CREATE TABLE LOG (MSG TEXT)'
           END-EXEC
           MOVE "INSERT INTO LOG VALUES (?)" TO WS-STMT
           EXEC SQL PREPARE S2 FROM :WS-STMT END-EXEC
           EXEC SQL EXECUTE S2 USING :WS-MSG END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM LOG
                     WHERE MSG = 'logged' END-EXEC
           IF WS-N = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL execute using: " WS-N
           END-IF
      *> EXECUTE of a statement never prepared: 07003.
           EXEC SQL EXECUTE S9 END-EXEC
           IF SQLSTATE = "07003"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL unprepared: " SQLSTATE
           END-IF
           DISPLAY "=== ESQL AC11 RESULT ==="
           DISPLAY "forms: SET SQLDA-DATA(i) TO ADDRESS OF; PREPARE FROM "
                   ":host; DESCRIBE INTO SQLDA (capacity 2 then 100); "
                   "DECLARE CURSOR FOR S1; OPEN USING :host; FETCH USING "
                   "DESCRIPTOR (pointers, then inline text); EXECUTE "
                   "IMMEDIATE; EXECUTE USING; EXECUTE unprepared"
           DISPLAY "rows: 3 inserted; " WS-ROWS " fetched through "
                   "pointers; 1 inline; 1 logged"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.

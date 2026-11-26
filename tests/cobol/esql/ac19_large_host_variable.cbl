       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC19.
      *> A host variable as large as a data item can be. A PICTURE holds up to
      *> 65,535 bytes, and an EXEC SQL statement must carry such an item whole,
      *> in either direction and as a value the statement only reads, as the
      *> interpreter does: a 50,000-byte item (the Backend of PowerAnalyticsWeb
      *> reads one as a BLOB in a WHERE clause) and a 65,535-byte one.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-BIG              PIC X(50000).
       01  WS-BACK             PIC X(50000).
       01  WS-FLAGS            PIC X(50000).
       01  WS-HUGE             PIC X(65535).
       01  WS-HUGE-BACK        PIC X(65535).
       01  WS-N                PIC 9(9).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC19 END-EXEC
           EXEC SQL CREATE TABLE B (ID INTEGER, V TEXT) END-EXEC
           EXEC SQL CREATE TABLE F (ID INTEGER) END-EXEC
      *> 1. 50,000 bytes out and back, whole.
           MOVE ALL "0123456789" TO WS-BIG
           MOVE "START"  TO WS-BIG (1:5)
           MOVE "MIDDLE" TO WS-BIG (25000:6)
           MOVE "END"    TO WS-BIG (49998:3)
           EXEC SQL INSERT INTO B VALUES (1, :WS-BIG) END-EXEC
           IF SQLSTATE NOT = "00000"
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL insert 50000 " SQLSTATE " " SQLMSG
           END-IF
           MOVE SPACES TO WS-BACK
           EXEC SQL SELECT V INTO :WS-BACK FROM B WHERE ID = 1 END-EXEC
           IF SQLSTATE = "00000" AND WS-BACK = WS-BIG
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL round trip 50000 " SQLSTATE " "
                   WS-BACK (1:5) " " WS-BACK (25000:6) " " WS-BACK (49998:3)
           END-IF
      *> 2. The same item only read, as a BLOB inside a WHERE clause.
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM B
                     WHERE ID = 1
                       AND substr(CAST(:WS-BIG AS BLOB), 25000, 6)
                           = CAST('MIDDLE' AS BLOB) END-EXEC
           IF SQLSTATE = "00000" AND WS-N = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL blob compare " SQLSTATE " " WS-N
           END-IF
      *> 3. One byte per id, the way PowerAnalyticsWeb ticks names.
           MOVE ALL "0" TO WS-FLAGS
           MOVE "1" TO WS-FLAGS (7:1)
           MOVE "1" TO WS-FLAGS (49999:1)
           EXEC SQL INSERT INTO F VALUES (7) END-EXEC
           EXEC SQL INSERT INTO F VALUES (8) END-EXEC
           EXEC SQL INSERT INTO F VALUES (49999) END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM F
                     WHERE substr(CAST(:WS-FLAGS AS BLOB), ID, 1) = x'31'
           END-EXEC
           IF SQLSTATE = "00000" AND WS-N = 2
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL flags " SQLSTATE " " WS-N
           END-IF
      *> 4. The largest item a PICTURE can make, out and back.
           MOVE ALL "abcdefghij" TO WS-HUGE
           MOVE "FIRST" TO WS-HUGE (1:5)
           MOVE "LAST!" TO WS-HUGE (65531:5)
           EXEC SQL INSERT INTO B VALUES (2, :WS-HUGE) END-EXEC
           IF SQLSTATE NOT = "00000"
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL insert 65535 " SQLSTATE " " SQLMSG
           END-IF
           MOVE SPACES TO WS-HUGE-BACK
           EXEC SQL SELECT V INTO :WS-HUGE-BACK FROM B WHERE ID = 2
           END-EXEC
           IF SQLSTATE = "00000" AND WS-HUGE-BACK = WS-HUGE
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL round trip 65535 " SQLSTATE " "
                   WS-HUGE-BACK (1:5) " " WS-HUGE-BACK (65531:5)
           END-IF
           DISPLAY "=== ESQL AC19 RESULT ==="
           DISPLAY "forms: INSERT VALUES (:H) and SELECT INTO :H of 50000 "
                   "and 65535 bytes, a 50000-byte item read as a BLOB in "
                   "WHERE (substr of the item, one byte per id)"
           DISPLAY "rows: 2 texts of 50000 and 65535 bytes stored; "
                   WS-N " ids matched the flags"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.

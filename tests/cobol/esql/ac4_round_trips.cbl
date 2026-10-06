       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC4.
      *> Spec 087 AC4 - values round-trip unchanged through packed,
      *> binary, display, alphanumeric and date items, negatives and
      *> maxima included; one larger fails with 22003, item unchanged.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A-PACKED            PIC S9(7)V99 COMP-3.
       01  A-BINARY            PIC S9(9) COMP-5.
       01  A-DISPLAY           PIC 9(5).
       01  A-TEXT              PIC X(40).
       01  A-DATE              PIC X(10).
       01  B-PACKED            PIC S9(7)V99 COMP-3.
       01  B-BINARY            PIC S9(9) COMP-5.
       01  B-DISPLAY           PIC 9(5).
       01  B-TEXT              PIC X(40).
       01  B-DATE              PIC X(10).
       01  WS-CASE             PIC 9(2) VALUE 0.
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC4 END-EXEC
           EXEC SQL CREATE TABLE RT (K INTEGER, P NUMERIC, B INTEGER,
                    D INTEGER, T TEXT, DT DATE) END-EXEC
           MOVE -1234567.89 TO A-PACKED
           MOVE -999999999 TO A-BINARY
           MOVE 0 TO A-DISPLAY
           MOVE "Forty characters of text, exactly 40 ok." TO A-TEXT
           MOVE "2026-10-06" TO A-DATE
           PERFORM ROUND-TRIP
           MOVE 9999999.99 TO A-PACKED
           MOVE 999999999 TO A-BINARY
           MOVE 99999 TO A-DISPLAY
           MOVE "Ünïcødé and plain" TO A-TEXT
           MOVE "1999-12-31" TO A-DATE
           PERFORM ROUND-TRIP
           MOVE -9999999.99 TO A-PACKED
           MOVE 1 TO A-BINARY
           MOVE 1 TO A-DISPLAY
           MOVE SPACES TO A-TEXT
           MOVE "0001-01-01" TO A-DATE
           PERFORM ROUND-TRIP
      *> One larger than each item holds: 22003, item unchanged.
           MOVE 1.25 TO B-PACKED
           EXEC SQL SELECT 10000000.00 INTO :B-PACKED END-EXEC
           PERFORM CHECK-RANGE
           IF B-PACKED NOT = 1.25
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL B-PACKED changed: " B-PACKED
           END-IF
           MOVE 7 TO B-DISPLAY
           EXEC SQL SELECT 100000 INTO :B-DISPLAY END-EXEC
           PERFORM CHECK-RANGE
           EXEC SQL SELECT -1 INTO :B-DISPLAY END-EXEC
           PERFORM CHECK-RANGE
           IF B-DISPLAY NOT = 7
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL B-DISPLAY changed: " B-DISPLAY
           END-IF
           EXEC SQL SELECT 1000000000 INTO :B-BINARY END-EXEC
           PERFORM CHECK-RANGE
           DISPLAY "=== ESQL AC4 RESULT ==="
           DISPLAY "forms: INSERT VALUES (:packed, :binary, :display, "
                   ":text, :date); SELECT ... INTO the same five; "
                   "SELECT n INTO :item for one past each maximum"
           DISPLAY "items: S9(7)V99 COMP-3, S9(9) COMP-5, 9(5), X(40), "
                   "X(10) date; round trips " WS-CASE
                   "; out-of-range checks 4"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       ROUND-TRIP.
           ADD 1 TO WS-CASE
           EXEC SQL INSERT INTO RT VALUES (:WS-CASE, :A-PACKED,
                    :A-BINARY, :A-DISPLAY, :A-TEXT, :A-DATE) END-EXEC
           EXEC SQL SELECT P, B, D, T, DT
                      INTO :B-PACKED, :B-BINARY, :B-DISPLAY, :B-TEXT,
                           :B-DATE
                      FROM RT WHERE K = :WS-CASE END-EXEC
           IF SQLSTATE = "00000" AND B-PACKED = A-PACKED
              AND B-BINARY = A-BINARY AND B-DISPLAY = A-DISPLAY
              AND B-TEXT = A-TEXT AND B-DATE = A-DATE
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL round trip " WS-CASE " " SQLSTATE " "
                   B-PACKED " " B-BINARY " " B-DISPLAY " [" B-TEXT "] "
                   B-DATE
           END-IF.
       CHECK-RANGE.
           IF SQLSTATE = "22003" AND SQLCODE = -22003
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL out of range gave " SQLSTATE
           END-IF.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC12.
      *> National and UTF-8 host variables round-trip through the
      *> database as their text: INSERT :nat, :utf8 and SELECT back
      *> INTO items of the same and of another size, NULL through an
      *> indicator, a longer text cut to a shorter item (01004).
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  A-NAT               PIC N(24).
       01  A-UTF               PIC U(24).
       01  B-NAT               PIC N(24).
       01  B-UTF               PIC U(24).
       01  S-NAT               PIC N(4).
       01  S-UTF               PIC U(4).
       01  S-ALNUM             PIC X(30).
       01  IND-1               PIC S9(4) COMP-5.
       01  N-NUM               PIC 9(5).
       01  WS-CASE             PIC 9(2) VALUE 0.
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC12 END-EXEC
           EXEC SQL CREATE TABLE NT (K INTEGER, N TEXT, U TEXT)
                END-EXEC
           MOVE N"Ünïcødé 日本語" TO A-NAT
           MOVE U"Ünïcødé 日本語" TO A-UTF
           PERFORM ROUND-TRIP
           MOVE N"plain" TO A-NAT
           MOVE U"plain" TO A-UTF
           PERFORM ROUND-TRIP
           MOVE SPACES TO A-NAT
           MOVE SPACES TO A-UTF
           PERFORM ROUND-TRIP
           MOVE N"😀 emoji" TO A-NAT
           MOVE U"😀 emoji" TO A-UTF
           PERFORM ROUND-TRIP
      *> The stored text is what the item's characters spell.
           EXEC SQL SELECT N INTO :S-ALNUM FROM NT WHERE K = 1
           END-EXEC
           DISPLAY "alnum view [" S-ALNUM "] " SQLSTATE
           EXEC SQL SELECT LENGTH(N) INTO :N-NUM FROM NT WHERE K = 2
           END-EXEC
           DISPLAY "length " N-NUM " " SQLSTATE
      *> A longer text into a shorter item.
           EXEC SQL SELECT N, U INTO :S-NAT, :S-UTF FROM NT
                WHERE K = 2 END-EXEC
           DISPLAY "short [" S-NAT "] [" S-UTF "] " SQLSTATE
           IF SQLSTATE = "01004"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL truncation gave " SQLSTATE
           END-IF
      *> NULL, through an indicator.
           MOVE N"keep" TO B-NAT
           EXEC SQL SELECT NULL INTO :B-NAT:IND-1 FROM NT WHERE K = 1
           END-EXEC
           DISPLAY "null [" B-NAT "] " IND-1 " " SQLSTATE
      *> A text from a literal in the statement into both.
           EXEC SQL SELECT 'from the statement', 'ünï' INTO :B-NAT,
                :B-UTF FROM NT WHERE K = 1 END-EXEC
           DISPLAY "literal [" B-NAT "] [" B-UTF "] " SQLSTATE
           EXEC SQL SELECT 42, 7 INTO :B-NAT, :B-UTF FROM NT
                WHERE K = 1 END-EXEC
           DISPLAY "numbers [" B-NAT "] [" B-UTF "] " SQLSTATE
           DISPLAY "=== ESQL AC12 RESULT ==="
           DISPLAY "forms: INSERT VALUES (:national, :utf8); SELECT ... "
                   "INTO the same and a shorter pair; an alphanumeric "
                   "item reading the stored text; NULL through an "
                   "indicator; a statement's literal and number INTO "
                   "both kinds"
           DISPLAY "items: N(24), U(24), N(4), U(4); round trips "
                   WS-CASE "; text of one, two and four-byte characters"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       ROUND-TRIP.
           ADD 1 TO WS-CASE
           EXEC SQL INSERT INTO NT VALUES (:WS-CASE, :A-NAT, :A-UTF)
           END-EXEC
           MOVE SPACES TO B-NAT
           MOVE SPACES TO B-UTF
           EXEC SQL SELECT N, U INTO :B-NAT, :B-UTF FROM NT
                WHERE K = :WS-CASE END-EXEC
           IF SQLSTATE = "00000" AND B-NAT = A-NAT AND B-UTF = A-UTF
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL round trip " WS-CASE " " SQLSTATE " ["
                   B-NAT "] [" B-UTF "]"
           END-IF.

       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-SEARCH
      *> Table search: serial SEARCH (WHEN / AT END, success & fail)
      *> and SEARCH ALL binary search over an ASCENDING KEY table,
      *> probing first / middle / last / not-found.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-SEARCH.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  CITY-TABLE.
           05 CITY-ENTRY OCCURS 6 TIMES
              ASCENDING KEY IS CITY-CODE
              INDEXED BY CITY-IX.
              10 CITY-CODE         PIC 9(2).
              10 CITY-NAME         PIC X(10).
       01  WS-WANTED-CODE          PIC 9(2)  VALUE 0.
       01  WS-WANTED-NAME          PIC X(10) VALUE SPACES.
       01  WS-FOUND                PIC X     VALUE "N".
       01  WS-FOUND-NAME           PIC X(10) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM LOAD-TABLE.
           DISPLAY "============================================".
           DISPLAY "TEST-SEARCH".
           DISPLAY "Constructs: serial SEARCH (WHEN / AT END),".
           DISPLAY "  SEARCH ALL (binary) first/middle/last/none".
           DISPLAY "Table: 6 cities, codes 10,20,30,40,50,60".
           DISPLAY "--------------------------------------------".
           PERFORM S001-SERIAL-FIRST.
           PERFORM S002-SERIAL-MIDDLE.
           PERFORM S003-SERIAL-LAST.
           PERFORM S004-SERIAL-ATEND.
           PERFORM S005-ALL-FIRST.
           PERFORM S006-ALL-MIDDLE.
           PERFORM S007-ALL-LAST.
           PERFORM S008-ALL-ATEND.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 8".
           DISPLAY "TESTS RUN       : " TESTS-RUN.
           DISPLAY "TESTS PASSED    : " TESTS-PASSED.
           DISPLAY "TESTS FAILED    : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".
           STOP RUN.
       LOAD-TABLE.
           MOVE 10 TO CITY-CODE (1). MOVE "LONDON"  TO CITY-NAME (1).
           MOVE 20 TO CITY-CODE (2). MOVE "PARIS"   TO CITY-NAME (2).
           MOVE 30 TO CITY-CODE (3). MOVE "BERLIN"  TO CITY-NAME (3).
           MOVE 40 TO CITY-CODE (4). MOVE "MADRID"  TO CITY-NAME (4).
           MOVE 50 TO CITY-CODE (5). MOVE "ROME"    TO CITY-NAME (5).
           MOVE 60 TO CITY-CODE (6). MOVE "LISBON"  TO CITY-NAME (6).
      *> ---- serial SEARCH (scans by index) ----
       S001-SERIAL-FIRST.
           ADD 1 TO TESTS-RUN.
           MOVE "LONDON" TO WS-WANTED-NAME.
           PERFORM DO-SERIAL-SEARCH.
           IF WS-FOUND = "Y" AND CITY-CODE (CITY-IX) = 10
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S001 serial SEARCH finds first (LONDON)".
       S002-SERIAL-MIDDLE.
           ADD 1 TO TESTS-RUN.
           MOVE "MADRID" TO WS-WANTED-NAME.
           PERFORM DO-SERIAL-SEARCH.
           IF WS-FOUND = "Y" AND CITY-CODE (CITY-IX) = 40
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S002 serial SEARCH finds middle (MADRID)".
       S003-SERIAL-LAST.
           ADD 1 TO TESTS-RUN.
           MOVE "LISBON" TO WS-WANTED-NAME.
           PERFORM DO-SERIAL-SEARCH.
           IF WS-FOUND = "Y" AND CITY-CODE (CITY-IX) = 60
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S003 serial SEARCH finds last (LISBON)".
       S004-SERIAL-ATEND.
           ADD 1 TO TESTS-RUN.
           MOVE "TOKYO" TO WS-WANTED-NAME.
           PERFORM DO-SERIAL-SEARCH.
           IF WS-FOUND = "N"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S004 serial SEARCH AT END (TOKYO not found)".
       DO-SERIAL-SEARCH.
           MOVE "N" TO WS-FOUND.
           SET CITY-IX TO 1.
           SEARCH CITY-ENTRY
               AT END
                   MOVE "N" TO WS-FOUND
               WHEN CITY-NAME (CITY-IX) = WS-WANTED-NAME
                   MOVE "Y" TO WS-FOUND
           END-SEARCH.
      *> ---- SEARCH ALL (binary search on ASCENDING KEY CITY-CODE) ----
       S005-ALL-FIRST.
           ADD 1 TO TESTS-RUN.
           MOVE 10 TO WS-WANTED-CODE.
           PERFORM DO-BINARY-SEARCH.
           IF WS-FOUND = "Y" AND WS-FOUND-NAME = "LONDON"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S005 SEARCH ALL finds first (code 10)".
       S006-ALL-MIDDLE.
           ADD 1 TO TESTS-RUN.
           MOVE 30 TO WS-WANTED-CODE.
           PERFORM DO-BINARY-SEARCH.
           IF WS-FOUND = "Y" AND WS-FOUND-NAME = "BERLIN"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S006 SEARCH ALL finds middle (code 30)".
       S007-ALL-LAST.
           ADD 1 TO TESTS-RUN.
           MOVE 60 TO WS-WANTED-CODE.
           PERFORM DO-BINARY-SEARCH.
           IF WS-FOUND = "Y" AND WS-FOUND-NAME = "LISBON"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S007 SEARCH ALL finds last (code 60)".
       S008-ALL-ATEND.
           ADD 1 TO TESTS-RUN.
           MOVE 99 TO WS-WANTED-CODE.
           PERFORM DO-BINARY-SEARCH.
           IF WS-FOUND = "N"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  S008 SEARCH ALL AT END (code 99 not found)".
       DO-BINARY-SEARCH.
           MOVE "N" TO WS-FOUND.
           MOVE SPACES TO WS-FOUND-NAME.
           SET CITY-IX TO 1.
           SEARCH ALL CITY-ENTRY
               AT END
                   MOVE "N" TO WS-FOUND
               WHEN CITY-CODE (CITY-IX) = WS-WANTED-CODE
                   MOVE "Y" TO WS-FOUND
                   MOVE CITY-NAME (CITY-IX) TO WS-FOUND-NAME
           END-SEARCH.
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

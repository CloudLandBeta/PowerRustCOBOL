       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-STRING-OVERFLOW
      *> String conditional clauses: STRING ... ON OVERFLOW /
      *> NOT ON OVERFLOW and UNSTRING ... ON OVERFLOW / NOT ON
      *> OVERFLOW; destination too small / large enough; delimiter
      *> found / not found; multiple vs too few receiving fields.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-STRING-OVERFLOW.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-DEST4                PIC X(4)  VALUE SPACES.
       01  WS-DEST20               PIC X(20) VALUE SPACES.
       01  WS-P1                   PIC X(5)  VALUE "HELLO".
       01  WS-P2                   PIC X(5)  VALUE "WORLD".
       01  WS-SRC3                 PIC X(5)  VALUE "A,B,C".
       01  WS-SRC4                 PIC X(7)  VALUE "A,B,C,D".
       01  WS-NODELIM              PIC X(3)  VALUE "ABC".
       01  R1                      PIC X(3)  VALUE SPACES.
       01  R2                      PIC X(3)  VALUE SPACES.
       01  R3                      PIC X(3)  VALUE SPACES.
       01  WS-FLAG                 PIC XX    VALUE SPACES.
       01  WS-PTR                  PIC 9(2)  VALUE 1.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-STRING-OVERFLOW".
           DISPLAY "Constructs: STRING/UNSTRING ON OVERFLOW &".
           DISPLAY "  NOT ON OVERFLOW; small/large destination;".
           DISPLAY "  delimiter found/not; multi/too-few receivers".
           DISPLAY "--------------------------------------------".
           PERFORM ST001-STRING-OVERFLOW.
           PERFORM ST002-STRING-OK.
           PERFORM ST003-STRING-POINTER.
           PERFORM ST004-UNSTRING-OK.
           PERFORM ST005-UNSTRING-OVERFLOW.
           PERFORM ST006-UNSTRING-NODELIM.
           PERFORM ST007-STRING-CONCAT.
           PERFORM ST008-UNSTRING-CONTENT.
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
       ST001-STRING-OVERFLOW.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE SPACES TO WS-DEST4.
           STRING WS-P1 DELIMITED BY SIZE
                  WS-P2 DELIMITED BY SIZE
               INTO WS-DEST4
               ON OVERFLOW     MOVE "OV" TO WS-FLAG
               NOT ON OVERFLOW MOVE "OK" TO WS-FLAG
           END-STRING.
           IF WS-FLAG = "OV"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST001 STRING into PIC X(4) -> ON OVERFLOW".
       ST002-STRING-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE SPACES TO WS-DEST20.
           STRING WS-P1 DELIMITED BY SIZE
                  WS-P2 DELIMITED BY SIZE
               INTO WS-DEST20
               ON OVERFLOW     MOVE "OV" TO WS-FLAG
               NOT ON OVERFLOW MOVE "OK" TO WS-FLAG
           END-STRING.
           IF WS-FLAG = "OK" AND WS-DEST20 = "HELLOWORLD"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST002 STRING into PIC X(20) -> NOT ON OVERFLOW".
       ST003-STRING-POINTER.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-DEST20.
           MOVE 1 TO WS-PTR.
           STRING WS-P1 DELIMITED BY SIZE
               INTO WS-DEST20
               WITH POINTER WS-PTR
           END-STRING.
           IF WS-PTR = 6 AND WS-DEST20 = "HELLO"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST003 STRING WITH POINTER advances 1 -> 6".
       ST004-UNSTRING-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE SPACES TO R1. MOVE SPACES TO R2. MOVE SPACES TO R3.
           UNSTRING WS-SRC3 DELIMITED BY ","
               INTO R1 R2 R3
               ON OVERFLOW     MOVE "OV" TO WS-FLAG
               NOT ON OVERFLOW MOVE "OK" TO WS-FLAG
           END-UNSTRING.
           IF WS-FLAG = "OK" AND R1 = "A" AND R2 = "B" AND R3 = "C"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST004 UNSTRING ""A,B,C"" 3 receivers -> NOT OV".
       ST005-UNSTRING-OVERFLOW.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE SPACES TO R1. MOVE SPACES TO R2.
           UNSTRING WS-SRC4 DELIMITED BY ","
               INTO R1 R2
               ON OVERFLOW     MOVE "OV" TO WS-FLAG
               NOT ON OVERFLOW MOVE "OK" TO WS-FLAG
           END-UNSTRING.
           IF WS-FLAG = "OV"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST005 UNSTRING ""A,B,C,D"" 2 recv -> ON OVERFLOW".
       ST006-UNSTRING-NODELIM.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE SPACES TO R1.
           UNSTRING WS-NODELIM DELIMITED BY ","
               INTO R1
               ON OVERFLOW     MOVE "OV" TO WS-FLAG
               NOT ON OVERFLOW MOVE "OK" TO WS-FLAG
           END-UNSTRING.
           IF WS-FLAG = "OK" AND R1 = "ABC"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST006 UNSTRING no delimiter -> whole field".
       ST007-STRING-CONCAT.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-DEST20.
           STRING "[" DELIMITED BY SIZE
                  WS-P1 DELIMITED BY SPACES
                  "]" DELIMITED BY SIZE
               INTO WS-DEST20
           END-STRING.
           IF WS-DEST20 = "[HELLO]"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST007 STRING concat literals + item -> [HELLO]".
       ST008-UNSTRING-CONTENT.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO R1. MOVE SPACES TO R2. MOVE SPACES TO R3.
           UNSTRING WS-SRC3 DELIMITED BY ","
               INTO R1 R2 R3
           END-UNSTRING.
           IF R1 = "A" AND R3 = "C"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  ST008 UNSTRING splits in order (A / B / C)".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

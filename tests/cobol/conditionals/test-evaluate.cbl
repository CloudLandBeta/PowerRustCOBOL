       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-EVALUATE
      *> EVALUATE constructs: single subject, multiple WHEN, WHEN
      *> OTHER, EVALUATE TRUE, EVALUATE ... ALSO ..., THRU ranges,
      *> WHEN NOT, and a WHEN listing several values.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-EVALUATE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-CODE                 PIC 9(2) VALUE 0.
       01  WS-CUST-TYPE            PIC X    VALUE SPACE.
       01  WS-SCORE                PIC 9(3) VALUE 0.
       01  WS-RESULT               PIC X(8) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-EVALUATE".
           DISPLAY "Constructs: single subject, multiple WHEN,".
           DISPLAY "  WHEN OTHER, EVALUATE TRUE, ALSO, THRU range,".
           DISPLAY "  WHEN NOT, WHEN listing several values".
           DISPLAY "--------------------------------------------".
           PERFORM E001-FIRST-WHEN.
           PERFORM E002-LATER-WHEN.
           PERFORM E003-WHEN-OTHER.
           PERFORM E004-EVALUATE-TRUE.
           PERFORM E005-ALSO.
           PERFORM E006-MULTI-VALUE-WHEN.
           PERFORM E007-THRU-RANGE.
           PERFORM E008-WHEN-NOT.
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
       E001-FIRST-WHEN.
           ADD 1 TO TESTS-RUN.
           MOVE 1 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN 1     MOVE "ONE"   TO WS-RESULT
               WHEN 2     MOVE "TWO"   TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "ONE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E001 single subject, first WHEN matches (=1)".
       E002-LATER-WHEN.
           ADD 1 TO TESTS-RUN.
           MOVE 2 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN 1     MOVE "ONE"   TO WS-RESULT
               WHEN 2     MOVE "TWO"   TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "TWO"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E002 single subject, later WHEN matches (=2)".
       E003-WHEN-OTHER.
           ADD 1 TO TESTS-RUN.
           MOVE 9 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN 1     MOVE "ONE"   TO WS-RESULT
               WHEN 2     MOVE "TWO"   TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "OTHER"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E003 WHEN OTHER matches (=9)".
       E004-EVALUATE-TRUE.
           ADD 1 TO TESTS-RUN.
           MOVE 75 TO WS-SCORE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE TRUE
               WHEN WS-SCORE >= 90 MOVE "A" TO WS-RESULT
               WHEN WS-SCORE >= 70 MOVE "B" TO WS-RESULT
               WHEN OTHER          MOVE "C" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "B"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E004 EVALUATE TRUE (score 75 -> grade B)".
       E005-ALSO.
           ADD 1 TO TESTS-RUN.
           MOVE 5 TO WS-CODE.
           MOVE "G" TO WS-CUST-TYPE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE ALSO WS-CUST-TYPE
               WHEN 5 ALSO "G" MOVE "VIP"  TO WS-RESULT
               WHEN 5 ALSO "S" MOVE "STD"  TO WS-RESULT
               WHEN OTHER      MOVE "NONE" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "VIP"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E005 EVALUATE x ALSO y (5 ALSO ""G"" -> VIP)".
       E006-MULTI-VALUE-WHEN.
           ADD 1 TO TESTS-RUN.
           MOVE 3 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN 1 WHEN 3 WHEN 5 MOVE "ODD"  TO WS-RESULT
               WHEN 2 WHEN 4 WHEN 6 MOVE "EVEN" TO WS-RESULT
               WHEN OTHER           MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "ODD"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E006 several WHENs share a branch (3 -> ODD)".
       E007-THRU-RANGE.
           ADD 1 TO TESTS-RUN.
           MOVE 15 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN 1  THRU 9  MOVE "LOW"  TO WS-RESULT
               WHEN 10 THRU 20 MOVE "MID"  TO WS-RESULT
               WHEN OTHER      MOVE "HIGH" TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "MID"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E007 THRU range (15 in 10 THRU 20 -> MID)".
       E008-WHEN-NOT.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-CODE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-CODE
               WHEN NOT 0 MOVE "NONZERO" TO WS-RESULT
               WHEN OTHER MOVE "ZERO"    TO WS-RESULT
           END-EVALUATE.
           IF WS-RESULT = "NONZERO"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  E008 WHEN NOT 0 (7 is non-zero -> NONZERO)".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

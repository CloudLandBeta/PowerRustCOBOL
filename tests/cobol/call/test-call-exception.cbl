       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-CALL-EXCEPTION
      *> CALL conditional clauses: CALL ... ON EXCEPTION /
      *> NOT ON EXCEPTION. A successful CALL to an existing sibling
      *> subprogram (ADDER, via USING parameters) and a failed CALL
      *> to a missing program exercise both branches.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-CALL-EXCEPTION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-A                    PIC 9(4) VALUE 0.
       01  WS-B                    PIC 9(4) VALUE 0.
       01  WS-R                    PIC 9(4) VALUE 0.
       01  WS-FLAG                 PIC XX   VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-CALL-EXCEPTION".
           DISPLAY "Constructs: CALL ... USING, ON EXCEPTION /".
           DISPLAY "  NOT ON EXCEPTION; existing vs missing program".
           DISPLAY "--------------------------------------------".
           PERFORM C001-CALL-OK.
           PERFORM C002-CALL-MISSING.
           PERFORM C003-CALL-RESULT.
           PERFORM C004-MISSING-NOT-RUN.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 4".
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
       C001-CALL-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 3 TO WS-A.
           MOVE 4 TO WS-B.
           MOVE 0 TO WS-R.
           CALL "ADDER" USING WS-A WS-B WS-R
               ON EXCEPTION     MOVE "EX" TO WS-FLAG
               NOT ON EXCEPTION MOVE "OK" TO WS-FLAG
           END-CALL.
           IF WS-FLAG = "OK"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  C001 CALL existing ADDER -> NOT ON EXCEPTION".
       C002-CALL-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           CALL "NOSUCHPROG" USING WS-A
               ON EXCEPTION     MOVE "EX" TO WS-FLAG
               NOT ON EXCEPTION MOVE "OK" TO WS-FLAG
           END-CALL.
           IF WS-FLAG = "EX"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  C002 CALL missing program -> ON EXCEPTION".
       C003-CALL-RESULT.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           MOVE 8 TO WS-B.
           MOVE 0 TO WS-R.
           CALL "ADDER" USING WS-A WS-B WS-R
           END-CALL.
           IF WS-R = 15
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  C003 CALL ADDER by reference -> 7+8 = 15".
       C004-MISSING-NOT-RUN.
           ADD 1 TO TESTS-RUN.
           MOVE "ZZ" TO WS-FLAG.
           CALL "NOSUCHPROG"
               ON EXCEPTION     CONTINUE
               NOT ON EXCEPTION MOVE "OK" TO WS-FLAG
           END-CALL.
           IF WS-FLAG = "ZZ"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  C004 missing program -> NOT ON EXCEPTION skipped".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".
       END PROGRAM TEST-CALL-EXCEPTION.
      *> ============================================================
      *> Called subprogram: ADDER (L-R := L-A + L-B)
      *> ============================================================
       IDENTIFICATION DIVISION.
       PROGRAM-ID. ADDER.
       DATA DIVISION.
       LINKAGE SECTION.
       01  L-A                     PIC 9(4).
       01  L-B                     PIC 9(4).
       01  L-R                     PIC 9(4).
       PROCEDURE DIVISION USING L-A L-B L-R.
       ADDER-PARA.
           COMPUTE L-R = L-A + L-B.
           GOBACK.
       END PROGRAM ADDER.

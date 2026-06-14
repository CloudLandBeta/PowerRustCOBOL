       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-ARITH-SIZE-ERROR
      *> Arithmetic conditional clauses: ON SIZE ERROR / NOT ON SIZE
      *> ERROR for ADD, SUBTRACT, MULTIPLY, DIVIDE and COMPUTE,
      *> including overflow and division by zero. Small PIC 9(2)
      *> receivers force the overflow predictably.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-ARITH-SIZE-ERROR.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-R2                   PIC 9(2) VALUE 0.
       01  WS-FLAG                 PIC XX   VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-ARITH-SIZE-ERROR".
           DISPLAY "Constructs: ON SIZE ERROR / NOT ON SIZE ERROR".
           DISPLAY "  for ADD/SUBTRACT/MULTIPLY/DIVIDE/COMPUTE,".
           DISPLAY "  overflow & divide-by-zero (PIC 9(2) receiver)".
           DISPLAY "--------------------------------------------".
           PERFORM A001-ADD-SE.
           PERFORM A002-ADD-OK.
           PERFORM A003-SUB-SE.
           PERFORM A004-SUB-OK.
           PERFORM A005-MUL-SE.
           PERFORM A006-MUL-OK.
           PERFORM A007-DIV-ZERO-SE.
           PERFORM A008-DIV-OK.
           PERFORM A009-COMPUTE-SE.
           PERFORM A010-COMPUTE-OK.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 10".
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
       A001-ADD-SE.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 0 TO WS-R2.
           ADD 99 1 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-ADD.
           IF WS-FLAG = "SE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A001 ADD 99+1 -> 100 overflows -> SIZE ERROR".
       A002-ADD-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           ADD 40 9 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-ADD.
           IF WS-FLAG = "OK" AND WS-R2 = 49
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A002 ADD 40+9 -> 49 -> NOT ON SIZE ERROR".
       A003-SUB-SE.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           SUBTRACT 5 FROM 200 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-SUBTRACT.
           IF WS-FLAG = "SE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A003 SUBTRACT 5 FROM 200 -> 195 -> SIZE ERROR".
       A004-SUB-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           SUBTRACT 5 FROM 50 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-SUBTRACT.
           IF WS-FLAG = "OK" AND WS-R2 = 45
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A004 SUBTRACT 5 FROM 50 -> 45 -> NOT SIZE ERR".
       A005-MUL-SE.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MULTIPLY 50 BY 3 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-MULTIPLY.
           IF WS-FLAG = "SE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A005 MULTIPLY 50 BY 3 -> 150 -> SIZE ERROR".
       A006-MUL-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MULTIPLY 4 BY 5 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-MULTIPLY.
           IF WS-FLAG = "OK" AND WS-R2 = 20
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A006 MULTIPLY 4 BY 5 -> 20 -> NOT SIZE ERROR".
       A007-DIV-ZERO-SE.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           DIVIDE 10 BY 0 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-DIVIDE.
           IF WS-FLAG = "SE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A007 DIVIDE 10 BY 0 -> divide by zero -> SE".
       A008-DIV-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           DIVIDE 10 BY 2 GIVING WS-R2
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-DIVIDE.
           IF WS-FLAG = "OK" AND WS-R2 = 5
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A008 DIVIDE 10 BY 2 -> 5 -> NOT ON SIZE ERROR".
       A009-COMPUTE-SE.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           COMPUTE WS-R2 = 99 + 5
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-COMPUTE.
           IF WS-FLAG = "SE"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A009 COMPUTE 99+5 -> 104 -> SIZE ERROR".
       A010-COMPUTE-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           COMPUTE WS-R2 = 10 + 5
               ON SIZE ERROR     MOVE "SE" TO WS-FLAG
               NOT ON SIZE ERROR MOVE "OK" TO WS-FLAG
           END-COMPUTE.
           IF WS-FLAG = "OK" AND WS-R2 = 15
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  A010 COMPUTE 10+5 -> 15 -> NOT ON SIZE ERROR".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

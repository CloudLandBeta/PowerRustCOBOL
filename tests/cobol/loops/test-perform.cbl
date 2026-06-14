       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-PERFORM
      *> Loop constructs with explicit iteration counts:
      *> PERFORM UNTIL, PERFORM VARYING ... UNTIL, WITH TEST BEFORE,
      *> WITH TEST AFTER, PERFORM n TIMES; loops that run zero / one /
      *> many times, flag-controlled and counter-controlled.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-PERFORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-COUNT                PIC 9(4) VALUE 0.
       01  WS-SUM                  PIC 9(4) VALUE 0.
       01  WS-I                    PIC 9(4) VALUE 0.
       01  WS-FLAG                 PIC X    VALUE "N".
           88 DONE-FLAG            VALUE "Y".
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-PERFORM".
           DISPLAY "Constructs: PERFORM UNTIL, VARYING UNTIL,".
           DISPLAY "  WITH TEST BEFORE / AFTER, PERFORM n TIMES;".
           DISPLAY "  zero / one / many iterations, flag & counter".
           DISPLAY "--------------------------------------------".
           PERFORM P001-UNTIL-MANY.
           PERFORM P002-VARYING.
           PERFORM P003-TEST-BEFORE-ZERO.
           PERFORM P004-TEST-AFTER-ONE.
           PERFORM P005-ZERO-TIMES.
           PERFORM P006-EXACTLY-ONCE.
           PERFORM P007-MANY-TIMES.
           PERFORM P008-FLAG-CONTROLLED.
           PERFORM P009-COUNTER-CONTROLLED.
           PERFORM P010-PERFORM-TIMES.
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
       P001-UNTIL-MANY.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           PERFORM UNTIL WS-COUNT >= 5
               ADD 1 TO WS-COUNT
           END-PERFORM.
           IF WS-COUNT = 5
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P001 PERFORM UNTIL >= 5 -> 5 iterations".
       P002-VARYING.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-SUM.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
               ADD WS-I TO WS-SUM
           END-PERFORM.
           IF WS-SUM = 15
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P002 PERFORM VARYING 1..5 -> sum 15".
       P003-TEST-BEFORE-ZERO.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           PERFORM WITH TEST BEFORE UNTIL 1 = 1
               ADD 1 TO WS-COUNT
           END-PERFORM.
           IF WS-COUNT = 0
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P003 TEST BEFORE, cond already true -> 0".
       P004-TEST-AFTER-ONE.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           PERFORM WITH TEST AFTER UNTIL 1 = 1
               ADD 1 TO WS-COUNT
           END-PERFORM.
           IF WS-COUNT = 1
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P004 TEST AFTER, cond true -> 1 guaranteed".
       P005-ZERO-TIMES.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           MOVE 10 TO WS-I.
           PERFORM UNTIL WS-I > 5
               ADD 1 TO WS-COUNT
               ADD 1 TO WS-I
           END-PERFORM.
           IF WS-COUNT = 0
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P005 loop body skipped (I=10, until I>5)".
       P006-EXACTLY-ONCE.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           MOVE 5 TO WS-I.
           PERFORM UNTIL WS-I > 5
               ADD 1 TO WS-COUNT
               ADD 1 TO WS-I
           END-PERFORM.
           IF WS-COUNT = 1
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P006 loop runs exactly once (I=5..6)".
       P007-MANY-TIMES.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           MOVE 1 TO WS-I.
           PERFORM UNTIL WS-I > 8
               ADD 1 TO WS-COUNT
               ADD 1 TO WS-I
           END-PERFORM.
           IF WS-COUNT = 8
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P007 loop runs many times (8 iterations)".
       P008-FLAG-CONTROLLED.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           MOVE "N" TO WS-FLAG.
           PERFORM UNTIL DONE-FLAG
               ADD 1 TO WS-COUNT
               IF WS-COUNT = 3
                   SET DONE-FLAG TO TRUE
               END-IF
           END-PERFORM.
           IF WS-COUNT = 3
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P008 flag-controlled (88 SET TRUE at 3)".
       P009-COUNTER-CONTROLLED.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           PERFORM VARYING WS-I FROM 2 BY 2 UNTIL WS-I > 10
               ADD 1 TO WS-COUNT
           END-PERFORM.
           IF WS-COUNT = 5
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P009 counter-controlled VARYING BY 2 (5 steps)".
       P010-PERFORM-TIMES.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-COUNT.
           PERFORM 4 TIMES
               ADD 1 TO WS-COUNT
           END-PERFORM.
           IF WS-COUNT = 4
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  P010 PERFORM 4 TIMES -> 4 iterations".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

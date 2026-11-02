       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-FUNCTIONS  (spec 109 S1)
      *> FUNCTION-ID units called inside expressions: by-value
      *> arguments, nesting, recursion, a literal argument.
      *> The program unit comes first; the function units follow it.
      *> Self-checking: a case prints only if it FAILS; one result
      *> block at the end gives the cases, the timing and the tally.
      *> ============================================================
       PROGRAM-ID. TEST-FUNCTIONS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-PRICE                PIC S9(5)V99 VALUE 21.50.
       01  WS-OUT                  PIC S9(9)V99 VALUE 0.
       01  WS-EXP                  PIC S9(9)V99 VALUE 0.
       01  WS-N                    PIC 9(4) VALUE 5.
       01  WS-LABEL                PIC X(40) VALUE SPACES.
       01  WS-I                    PIC 9(6)  VALUE 0.
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0.
       01  WS-T0                   PIC 9(8)  VALUE 0.
       01  WS-T1                   PIC 9(8)  VALUE 0.
       01  WS-TMS                  PIC 9(9)  VALUE 0.
       01  WS-LOOPS                PIC 9(6)  VALUE 5000.
       01  WS-RATE                 PIC 9(9)  VALUE 0.
       01  WS-HH                   PIC 9(2)  VALUE 0.
       01  WS-MM                   PIC 9(2)  VALUE 0.
       01  WS-SS                   PIC 9(2)  VALUE 0.
       01  WS-CC                   PIC 9(2)  VALUE 0.
       01  WS-MS0                  PIC 9(9)  VALUE 0.
       01  WS-MS1                  PIC 9(9)  VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM CASES-FUNCTIONS.
           PERFORM TIMING-LOOP.
           PERFORM SHOW-RESULTS.
           STOP RUN.

       CASES-FUNCTIONS.
           MOVE "one call on an item" TO WS-LABEL.
           MOVE 43 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION DOUBLE-AMOUNT(WS-PRICE).
           PERFORM CHECK-NUM.
           MOVE "two calls in one expression" TO WS-LABEL.
           MOVE 53 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION DOUBLE-AMOUNT(WS-PRICE)
               + FUNCTION DOUBLE-AMOUNT(5).
           PERFORM CHECK-NUM.
           MOVE "nested calls" TO WS-LABEL.
           MOVE 86 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION DOUBLE-AMOUNT(
               FUNCTION DOUBLE-AMOUNT(WS-PRICE)).
           PERFORM CHECK-NUM.
           MOVE "argument is a copy: caller item unchanged" TO WS-LABEL.
           MOVE 21.50 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION DOUBLE-AMOUNT(WS-PRICE).
           MOVE WS-PRICE TO WS-OUT.
           PERFORM CHECK-NUM.
           MOVE "recursion FACT(5)" TO WS-LABEL.
           MOVE 120 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION FACT(WS-N).
           PERFORM CHECK-NUM.
           MOVE "recursion FACT(1)" TO WS-LABEL.
           MOVE 1 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION FACT(1).
           PERFORM CHECK-NUM.
           MOVE "recursion FACT(9)" TO WS-LABEL.
           MOVE 362880 TO WS-EXP.
           COMPUTE WS-OUT = FUNCTION FACT(9).
           PERFORM CHECK-NUM.
           MOVE "call inside a condition" TO WS-LABEL.
           MOVE 1 TO WS-EXP.
           MOVE 0 TO WS-OUT.
           IF FUNCTION DOUBLE-AMOUNT(WS-PRICE) > 40
               MOVE 1 TO WS-OUT
           END-IF.
           PERFORM CHECK-NUM.

       TIMING-LOOP.
           ACCEPT WS-T0 FROM TIME.
           MOVE WS-T0(1:2) TO WS-HH.
           MOVE WS-T0(3:2) TO WS-MM.
           MOVE WS-T0(5:2) TO WS-SS.
           MOVE WS-T0(7:2) TO WS-CC.
           COMPUTE WS-MS0 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LOOPS
               COMPUTE WS-OUT = FUNCTION FACT(6)
           END-PERFORM.
           ACCEPT WS-T1 FROM TIME.
           MOVE WS-T1(1:2) TO WS-HH.
           MOVE WS-T1(3:2) TO WS-MM.
           MOVE WS-T1(5:2) TO WS-SS.
           MOVE WS-T1(7:2) TO WS-CC.
           COMPUTE WS-MS1 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.
           IF WS-MS1 >= WS-MS0
               COMPUTE WS-TMS = WS-MS1 - WS-MS0
           ELSE
               MOVE 0 TO WS-TMS
           END-IF.
           IF WS-TMS > 0
               COMPUTE WS-RATE = WS-LOOPS * 1000 / WS-TMS
           ELSE
               MOVE WS-LOOPS TO WS-RATE
           END-IF.

       CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           IF WS-OUT = WS-EXP
               ADD 1 TO WS-TOT-OK
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      " WS-OUT
               DISPLAY "  expected " WS-EXP
           END-IF.

       SHOW-RESULTS.
           DISPLAY "============================================".
           DISPLAY "TEST-FUNCTIONS (spec 109 S1)".
           DISPLAY "Forms: FUNCTION-ID by value, nested calls,".
           DISPLAY "       recursion (LOCAL-STORAGE), in IF".
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES RUN       : " WS-TOT-RUN.
           DISPLAY "CASES PASSED    : " WS-TOT-OK.
           DISPLAY "CASES FAILED    : " WS-TOT-BAD.
           DISPLAY "PHASE TIMING    : " WS-LOOPS
               " recursive FACT(6) calls in " WS-TMS " ms".
           DISPLAY "THROUGHPUT      : " WS-RATE " calls/sec".
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".
       IDENTIFICATION DIVISION.
       FUNCTION-ID. DOUBLE-AMOUNT.
       DATA DIVISION.
       LINKAGE SECTION.
       01 ARG-AMOUNT PIC S9(5)V99.
       01 RESULT-AMOUNT PIC S9(5)V99.
       PROCEDURE DIVISION USING BY VALUE ARG-AMOUNT RETURNING RESULT-AMOUNT.
           COMPUTE RESULT-AMOUNT = ARG-AMOUNT * 2
           GOBACK.
       END FUNCTION DOUBLE-AMOUNT.

       IDENTIFICATION DIVISION.
       FUNCTION-ID. FACT.
       DATA DIVISION.
       LINKAGE SECTION.
       01 ARG-N PIC 9(4).
       01 RES-N PIC 9(9).
       LOCAL-STORAGE SECTION.
       01 LS-N PIC 9(4).
       PROCEDURE DIVISION USING BY VALUE ARG-N RETURNING RES-N.
           IF ARG-N <= 1
               MOVE 1 TO RES-N
           ELSE
               COMPUTE LS-N = ARG-N - 1
               COMPUTE RES-N = ARG-N * FUNCTION FACT(LS-N)
           END-IF
           GOBACK.
       END FUNCTION FACT.

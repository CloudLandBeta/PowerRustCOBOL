       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-ARITH-RELATION-AFTER-AND
      *> After AND / OR, a relation whose SUBJECT is an arithmetic
      *> expression is a complete relation (COBOL-85: either side of a
      *> relation condition may be an arithmetic expression). It used
      *> to be read as the OBJECT of an abbreviated relation, so
      *>     IF A > 0 AND A + 0.3 > B
      *> failed to compile: "unexpected token in statement: Gt".
      *> Covers: identifier, literal and signed heads; + - * / and
      *> mixed precedence; NOT =, IS GREATER THAN, a sign condition;
      *> three terms; PERFORM UNTIL and EVALUATE TRUE; and the two
      *> abbreviations that must NOT change (A = B OR C - 1,
      *> A > B AND 2 * C).
      *> Self-checking, one result block: cases, timing, tally.
      *> ============================================================
       PROGRAM-ID. TEST-ARITH-RELATION-AFTER-AND.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-FAILED-CASES         PIC X(60) VALUE SPACES.
       01  WS-FAILED-PTR           PIC 99 VALUE 1.
       01  WS-CASE                 PIC XX VALUE SPACES.
       01  WS-GOT                  PIC X VALUE SPACE.
       01  WS-WANT                 PIC X VALUE SPACE.
       01  A                       PIC S9(4)V9(4) VALUE 0.
       01  B                       PIC S9(4)V9(4) VALUE 0.
       01  C                       PIC S9(4)V9(4) VALUE 0.
       01  I                       PIC 9(4) VALUE 0.
       01  N                       PIC 9(7) VALUE 0.
       01  HITS                    PIC 9(7) VALUE 0.
       01  LOOPS                   PIC 9(7) VALUE 50000.
       01  T-START                 PIC X(21).
       01  T-END                   PIC X(21).
       01  T-MS                    PIC 9(9) VALUE 0.
       01  T-RATE                  PIC 9(11) VALUE 0.
       01  T-CHECK-MS              PIC 9(9) VALUE 0.
       01  WS-EDIT-MS              PIC ZZZZZZZZ9.
       01  WS-EDIT-RATE            PIC ZZZ,ZZZ,ZZZ,ZZ9.
       01  WS-EDIT-N               PIC ZZZ,ZZ9.
       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE FUNCTION CURRENT-DATE TO T-START
           PERFORM C01-IDENT-HEAD-TRUE
           PERFORM C02-IDENT-HEAD-FALSE
           PERFORM C03-LITERAL-HEAD
           PERFORM C04-SIGNED-HEAD
           PERFORM C05-OR-PRODUCTS
           PERFORM C06-NOT-EQUAL
           PERFORM C07-IS-GREATER-THAN
           PERFORM C08-SIGN-CONDITION
           PERFORM C09-THREE-TERMS
           PERFORM C10-PERFORM-UNTIL
           PERFORM C11-EVALUATE-TRUE
           PERFORM C12-ABBREV-OR-ARITH
           PERFORM C13-ABBREV-AND-ARITH
           MOVE FUNCTION CURRENT-DATE TO T-END
           PERFORM ELAPSED-MS
           MOVE T-MS TO T-CHECK-MS
           PERFORM TIMED-LOOP
           DISPLAY "============================================"
           DISPLAY "TEST-ARITH-RELATION-AFTER-AND"
           DISPLAY "A relation with an arithmetic subject after AND/OR"
           DISPLAY "--------------------------------------------"
           DISPLAY "CASES EXERCISED : 14"
           DISPLAY "  identifier, literal and signed heads; + - * /;"
           DISPLAY "  NOT =, IS GREATER THAN, NEGATIVE; three terms;"
           DISPLAY "  PERFORM UNTIL, EVALUATE TRUE; two abbreviations"
           DISPLAY "  kept: A = B OR C - 1, A > B AND 2 * C;"
           DISPLAY "  and the hit count of the timed loop"
           MOVE T-CHECK-MS TO WS-EDIT-MS
           DISPLAY "CHECK PHASE     : " WS-EDIT-MS " ms"
           MOVE LOOPS TO WS-EDIT-N
           MOVE T-MS TO WS-EDIT-MS
           DISPLAY "TIMED PHASE     : " WS-EDIT-N " evaluations in "
               WS-EDIT-MS " ms"
           IF T-MS > 0
               COMPUTE T-RATE = LOOPS * 1000 / T-MS
               MOVE T-RATE TO WS-EDIT-RATE
               DISPLAY "THROUGHPUT      : " WS-EDIT-RATE
                   " evaluations/sec"
           ELSE
               DISPLAY "THROUGHPUT      : under the clock's 10 ms"
           END-IF
           DISPLAY "TIMED HITS      : " HITS
           DISPLAY "TESTS RUN       : " TESTS-RUN
           DISPLAY "TESTS PASSED    : " TESTS-PASSED
           DISPLAY "TESTS FAILED    : " TESTS-FAILED
           IF TESTS-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "FAILED CASES    : " WS-FAILED-CASES
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF
           DISPLAY "============================================"
           STOP RUN.

       C01-IDENT-HEAD-TRUE.
           MOVE "01" TO WS-CASE MOVE 1 TO A MOVE 1 TO B MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND A + 0.3 > B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C02-IDENT-HEAD-FALSE.
           MOVE "02" TO WS-CASE MOVE 1 TO A MOVE 2 TO B MOVE "N" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND A + 0.3 > B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C03-LITERAL-HEAD.
           MOVE "03" TO WS-CASE MOVE 1 TO A MOVE 1.5 TO B MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND 1 + A > B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C04-SIGNED-HEAD.
           MOVE "04" TO WS-CASE MOVE 1 TO A MOVE 1 TO B MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND -1 + A < B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C05-OR-PRODUCTS.
           MOVE "05" TO WS-CASE MOVE 1 TO A MOVE 1 TO C MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A < 0 OR A * A + C * C < 3
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C06-NOT-EQUAL.
           MOVE "06" TO WS-CASE MOVE 1 TO A MOVE 1 TO B MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND A - 1 NOT = B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C07-IS-GREATER-THAN.
           MOVE "07" TO WS-CASE MOVE 1 TO A MOVE 1 TO B MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND A + 1 IS GREATER THAN B
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C08-SIGN-CONDITION.
           MOVE "08" TO WS-CASE MOVE 1 TO A MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           IF A > 0 AND A - 2 NEGATIVE
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C09-THREE-TERMS.
           MOVE "09" TO WS-CASE MOVE 1 TO A MOVE 1 TO B MOVE 2 TO C
           MOVE "Y" TO WS-WANT MOVE "N" TO WS-GOT
           IF A > 0 AND A + 1 > B AND C * 2 = 4
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C10-PERFORM-UNTIL.
           MOVE "10" TO WS-CASE MOVE "Y" TO WS-WANT MOVE "N" TO WS-GOT
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 9 OR I * I > 20
               CONTINUE
           END-PERFORM
           IF I = 5
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C11-EVALUATE-TRUE.
           MOVE "11" TO WS-CASE MOVE 1 TO A MOVE "Y" TO WS-WANT
           MOVE "N" TO WS-GOT
           EVALUATE TRUE
               WHEN A > 0 AND A + 1 = 2
                   MOVE "Y" TO WS-GOT
               WHEN OTHER
                   MOVE "N" TO WS-GOT
           END-EVALUATE
           PERFORM CHECK-IT.
       C12-ABBREV-OR-ARITH.
      *>   A = B OR C - 1 is A = B OR A = C - 1.
           MOVE "12" TO WS-CASE MOVE 1 TO A MOVE 5 TO B MOVE 2 TO C
           MOVE "Y" TO WS-WANT MOVE "N" TO WS-GOT
           IF A = B OR C - 1
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.
       C13-ABBREV-AND-ARITH.
      *>   A > B AND 2 * C is A > B AND A > 2 * C: 5 > 1 and 5 > 6 fails.
           MOVE "13" TO WS-CASE MOVE 5 TO A MOVE 1 TO B MOVE 3 TO C
           MOVE "N" TO WS-WANT MOVE "N" TO WS-GOT
           IF A > B AND 2 * C
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.

       TIMED-LOOP.
      *>   The reported condition, evaluated LOOPS times over a moving A.
           MOVE 0 TO HITS
           MOVE 1 TO B
           MOVE FUNCTION CURRENT-DATE TO T-START
           PERFORM VARYING N FROM 1 BY 1 UNTIL N > LOOPS
               COMPUTE A = FUNCTION MOD( N 4 ) * 0.5
               IF A > 0 AND A + 0.3 > B
                   ADD 1 TO HITS
               END-IF
           END-PERFORM
           MOVE FUNCTION CURRENT-DATE TO T-END
           PERFORM ELAPSED-MS
      *>   A cycles 0.5 1.0 1.5 0.0: A + 0.3 > 1 for 1.0 and 1.5.
           MOVE "TL" TO WS-CASE MOVE "Y" TO WS-WANT MOVE "N" TO WS-GOT
           IF HITS = LOOPS / 2
               MOVE "Y" TO WS-GOT
           END-IF
           PERFORM CHECK-IT.

       ELAPSED-MS.
           COMPUTE T-MS =
               ( FUNCTION NUMVAL( T-END(9:2) ) * 3600000
               + FUNCTION NUMVAL( T-END(11:2) ) * 60000
               + FUNCTION NUMVAL( T-END(13:2) ) * 1000
               + FUNCTION NUMVAL( T-END(15:2) ) * 10 )
             - ( FUNCTION NUMVAL( T-START(9:2) ) * 3600000
               + FUNCTION NUMVAL( T-START(11:2) ) * 60000
               + FUNCTION NUMVAL( T-START(13:2) ) * 1000
               + FUNCTION NUMVAL( T-START(15:2) ) * 10 ).

       CHECK-IT.
           ADD 1 TO TESTS-RUN
           IF WS-GOT = WS-WANT
               ADD 1 TO TESTS-PASSED
           ELSE
               ADD 1 TO TESTS-FAILED
               STRING WS-CASE " " DELIMITED BY SIZE
                   INTO WS-FAILED-CASES WITH POINTER WS-FAILED-PTR
           END-IF.

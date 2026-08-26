       IDENTIFICATION DIVISION.
       PROGRAM-ID. LEADDOT.
      *>****************************************************************
      *>    NUMERIC LITERALS THAT BEGIN WITH A DECIMAL POINT           *
      *>                                                               *
      *>    COBOL-85 ALLOWS A NUMERIC LITERAL TO START WITH THE        *
      *>    DECIMAL POINT; IT MUST ONLY NOT END WITH ONE. THE NIST     *
      *>    CCVS85 VALIDATION SUITE RELIES ON THIS IN 48 OF ITS 459    *
      *>    PROGRAMS, MOST HEAVILY IN THE INTRINSIC-FUNCTION MODULE:   *
      *>        COMPUTE WS-NUM = FUNCTION ACOS(.999).                  *
      *>                                                               *
      *>    THE HARD PART IS THE LEADING ZEROS. .00001 AND .1 CARRY    *
      *>    THE SAME DIGIT VALUE ONCE PARSED; ONLY THE NUMBER OF       *
      *>    DIGITS WRITTEN TELLS THEM APART, SO EVERY CASE BELOW       *
      *>    CHECKS THE SCALE AND NOT MERELY THAT IT COMPILED.          *
      *>                                                               *
      *>    EACH CASE REPORTS ITS FORM, THE EXPECTED VALUE AND THE     *
      *>    ACTUAL VALUE; A SINGLE SUMMARY BLOCK IS PRINTED AT THE END.*
      *>****************************************************************
       DATA DIVISION.
       WORKING-STORAGE SECTION.

      *>    Literals written with a leading decimal point.
       77  A-TENTH            PICTURE S9V9        VALUE .1.
       77  A-NINE-HUNDREDTHS  PICTURE S9V99       VALUE .09.
       77  A-NINE-NINE-NINE   PICTURE S9V999      VALUE .999.
       77  A-FIVE-ONES        PICTURE SV9(5)      VALUE .11111.
       77  A-ONE-BILLIONTH    PICTURE SV9(9)      VALUE .000000001.
       77  A-NEGATIVE-HALF    PICTURE S9V9        VALUE -.5.
       77  A-POSITIVE-HALF    PICTURE S9V9        VALUE +.5.

      *>    The same values built by arithmetic, to prove the scale
      *>    survives a computation and is not merely stored.
       77  WRK-A              PICTURE S9(9)V9(9).
       77  WRK-B              PICTURE S9(9)V9(9).

      *>    Expected values, written the ordinary way, so a mistake in
      *>    the leading-point form shows up as a mismatch.
       77  E-TENTH            PICTURE S9V9        VALUE 0.1.
       77  E-NINE-HUNDREDTHS  PICTURE S9V99       VALUE 0.09.
       77  E-NINE-NINE-NINE   PICTURE S9V999      VALUE 0.999.
       77  E-FIVE-ONES        PICTURE SV9(5)      VALUE 0.11111.
       77  E-ONE-BILLIONTH    PICTURE SV9(9)      VALUE 0.000000001.
       77  E-NEGATIVE-HALF    PICTURE S9V9        VALUE -0.5.
       77  E-SUM              PICTURE S9(9)V9(9)  VALUE 0.111111111.

       77  TESTS-RUN          PICTURE 9(3)        VALUE ZERO.
       77  TESTS-PASSED       PICTURE 9(3)        VALUE ZERO.
       77  TESTS-FAILED       PICTURE 9(3)        VALUE ZERO.

       PROCEDURE DIVISION.
       MAIN-CONTROL.
           DISPLAY "=================================================".
           DISPLAY "LEADDOT - NUMERIC LITERAL WITH A LEADING '.'".
           DISPLAY "=================================================".
           PERFORM TEST-STORED-VALUES.
           PERFORM TEST-COMPUTED-VALUES.
           PERFORM TEST-COMPARISON.
           PERFORM REPORT-SUMMARY.
           STOP RUN.

      *>    Each stored VALUE must equal the same number written with an
      *>    explicit leading zero.
       TEST-STORED-VALUES.
           ADD 1 TO TESTS-RUN.
           IF A-TENTH = E-TENTH
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T001 VALUE .1          = " A-TENTH
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T001 VALUE .1"
               DISPLAY "     ACTUAL   = " A-TENTH
               DISPLAY "     EXPECTED = " E-TENTH
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF A-NINE-HUNDREDTHS = E-NINE-HUNDREDTHS
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T002 VALUE .09         = " A-NINE-HUNDREDTHS
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T002 VALUE .09"
               DISPLAY "     ACTUAL   = " A-NINE-HUNDREDTHS
               DISPLAY "     EXPECTED = " E-NINE-HUNDREDTHS
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF A-NINE-NINE-NINE = E-NINE-NINE-NINE
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T003 VALUE .999        = " A-NINE-NINE-NINE
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T003 VALUE .999"
               DISPLAY "     ACTUAL   = " A-NINE-NINE-NINE
               DISPLAY "     EXPECTED = " E-NINE-NINE-NINE
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF A-FIVE-ONES = E-FIVE-ONES
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T004 VALUE .11111      = " A-FIVE-ONES
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T004 VALUE .11111"
               DISPLAY "     ACTUAL   = " A-FIVE-ONES
               DISPLAY "     EXPECTED = " E-FIVE-ONES
           END-IF.

      *>    The leading-zero case: .000000001 has eight zeros before its
      *>    only significant digit. Losing them yields .1 instead.
           ADD 1 TO TESTS-RUN.
           IF A-ONE-BILLIONTH = E-ONE-BILLIONTH
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T005 VALUE .000000001  = " A-ONE-BILLIONTH
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T005 VALUE .000000001"
               DISPLAY "     ACTUAL   = " A-ONE-BILLIONTH
               DISPLAY "     EXPECTED = " E-ONE-BILLIONTH
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF A-NEGATIVE-HALF = E-NEGATIVE-HALF
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T006 VALUE -.5         = " A-NEGATIVE-HALF
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T006 VALUE -.5"
               DISPLAY "     ACTUAL   = " A-NEGATIVE-HALF
               DISPLAY "     EXPECTED = " E-NEGATIVE-HALF
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF A-POSITIVE-HALF = .5
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T007 VALUE +.5         = " A-POSITIVE-HALF
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T007 VALUE +.5"
               DISPLAY "     ACTUAL   = " A-POSITIVE-HALF
           END-IF.

      *>    A leading-point literal used directly in arithmetic.
       TEST-COMPUTED-VALUES.
           ADD 1 TO TESTS-RUN.
           COMPUTE WRK-A = .000000001 * 1000000000.
           IF WRK-A = 1
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T008 .000000001 * 1E9  = " WRK-A
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T008 .000000001 * 1E9"
               DISPLAY "     ACTUAL   = " WRK-A
               DISPLAY "     EXPECTED = 1"
           END-IF.

           ADD 1 TO TESTS-RUN.
           COMPUTE WRK-B = .1 + .01 + .001 + .0001 + .00001
                         + .000001 + .0000001 + .00000001
                         + .000000001.
           IF WRK-B = E-SUM
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T009 SUM OF 9 SCALES   = " WRK-B
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T009 SUM OF 9 SCALES"
               DISPLAY "     ACTUAL   = " WRK-B
               DISPLAY "     EXPECTED = " E-SUM
           END-IF.

      *>    A leading-point literal as the object of a comparison.
       TEST-COMPARISON.
           ADD 1 TO TESTS-RUN.
           IF A-TENTH = .1
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T010 IF X = .1"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T010 IF X = .1"
               DISPLAY "     ACTUAL   = " A-TENTH
           END-IF.

           ADD 1 TO TESTS-RUN.
           IF .09 < .1
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T011 IF .09 < .1"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T011 IF .09 < .1"
           END-IF.

       REPORT-SUMMARY.
           DISPLAY " ".
           DISPLAY "=================================================".
           DISPLAY "LEADDOT - SUMMARY".
           DISPLAY "=================================================".
           DISPLAY "FORMS EXERCISED :".
           DISPLAY "  VALUE .1 / .09 / .999 / .11111 / .000000001".
           DISPLAY "  VALUE -.5 and +.5        (signed leading point)".
           DISPLAY "  COMPUTE with .000000001  (scale kept in arith.)".
           DISPLAY "  COMPUTE summing 9 scales (.1 through .000000001)".
           DISPLAY "  IF X = .1                (literal as comparand)".
           DISPLAY "  IF .09 < .1              (literal on both sides)".
           DISPLAY " ".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF.
           DISPLAY "=================================================".

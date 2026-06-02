       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMPREC.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN             PIC 9(4) VALUE 0.
           05 TESTS-PASSED          PIC 9(4) VALUE 0.
           05 TESTS-FAILED          PIC 9(4) VALUE 0.
       01  TEST-FLAGS.
           05 SIZE-ERROR-FLAG       PIC X VALUE "N".
       01  FIXED-18.
           05 N18-A                 PIC S9(9)V9(9) VALUE ZERO.
           05 N18-B                 PIC S9(9)V9(9) VALUE ZERO.
           05 N18-C                 PIC S9(9)V9(9) VALUE ZERO.
           05 N18-EXPECTED          PIC S9(9)V9(9) VALUE ZERO.
       01  FIXED-18-MULT.
           05 M18-A                 PIC S9(12)V9(6) VALUE ZERO.
           05 M18-B                 PIC S9(12)V9(6) VALUE ZERO.
           05 M18-C                 PIC S9(12)V9(6) VALUE ZERO.
           05 M18-EXPECTED          PIC S9(12)V9(6) VALUE ZERO.
       01  FIXED-18-ROUND.
           05 R18-A                 PIC S9(7)V9(6) VALUE ZERO.
           05 R18-B                 PIC S9(7)V9(6) VALUE ZERO.
           05 R18-C                 PIC S9(7)V9(6) VALUE ZERO.
           05 R18-EXPECTED          PIC S9(7)V9(6) VALUE ZERO.
       01  FIXED-31.
           05 N31-A                 PIC S9(18)V9(13) VALUE ZERO.
           05 N31-B                 PIC S9(18)V9(13) VALUE ZERO.
           05 N31-C                 PIC S9(18)V9(13) VALUE ZERO.
           05 N31-EXPECTED          PIC S9(18)V9(13) VALUE ZERO.
       01  FIXED-31-MULT.
           05 M31-A                 PIC S9(18)V9(13) VALUE ZERO.
           05 M31-B                 PIC S9(18)V9(13) VALUE ZERO.
           05 M31-C                 PIC S9(18)V9(13) VALUE ZERO.
           05 M31-EXPECTED          PIC S9(18)V9(13) VALUE ZERO.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "NUMERIC PRECISION TEST SUITE".
           DISPLAY "----------------------------".
           PERFORM TEST-DECIMAL-ADDITION.
           PERFORM TEST-DECIMAL-SUBTRACTION.
           PERFORM TEST-FRACTIONAL-CARRY-18.
           PERFORM TEST-MULTIPLICATION-18.
           PERFORM TEST-ROUNDED-DIVISION-18.
           PERFORM TEST-31-LOWEST-FRACTION.
           PERFORM TEST-31-LARGE-ADDITION.
           PERFORM TEST-31-FRACTIONAL-CARRY.
           PERFORM TEST-31-MULTIPLICATION.
           PERFORM TEST-SIZE-ERROR-31.
           DISPLAY "----------------------------".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF.
           STOP RUN.
       TEST-DECIMAL-ADDITION.
           ADD 1 TO TESTS-RUN.
           MOVE 0.100000000 TO N18-A.
           MOVE 0.200000000 TO N18-B.
           MOVE 0.300000000 TO N18-EXPECTED.
           COMPUTE N18-C = N18-A + N18-B.
           IF N18-C = N18-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T001 DECIMAL ADDITION"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T001 DECIMAL ADDITION"
               DISPLAY "     ACTUAL   = " N18-C
               DISPLAY "     EXPECTED = " N18-EXPECTED
           END-IF.
       TEST-DECIMAL-SUBTRACTION.
           ADD 1 TO TESTS-RUN.
           MOVE 1.000000000 TO N18-A.
           MOVE 0.010000000 TO N18-B.
           MOVE 0.990000000 TO N18-EXPECTED.
           COMPUTE N18-C = N18-A - N18-B.
           IF N18-C = N18-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T002 DECIMAL SUBTRACTION"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T002 DECIMAL SUBTRACTION"
               DISPLAY "     ACTUAL   = " N18-C
               DISPLAY "     EXPECTED = " N18-EXPECTED
           END-IF.
       TEST-FRACTIONAL-CARRY-18.
           ADD 1 TO TESTS-RUN.
           MOVE 999999998.999999999 TO N18-A.
           MOVE 0.000000001         TO N18-B.
           MOVE 999999999.000000000 TO N18-EXPECTED.
           COMPUTE N18-C = N18-A + N18-B.
           IF N18-C = N18-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T003 FRACTIONAL CARRY 18"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T003 FRACTIONAL CARRY 18"
               DISPLAY "     ACTUAL   = " N18-C
               DISPLAY "     EXPECTED = " N18-EXPECTED
           END-IF.
       TEST-MULTIPLICATION-18.
           ADD 1 TO TESTS-RUN.
           MOVE 12345.678901    TO M18-A.
           MOVE 1000.000000     TO M18-B.
           MOVE 12345678.901000 TO M18-EXPECTED.
           COMPUTE M18-C = M18-A * M18-B.
           IF M18-C = M18-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T004 MULTIPLICATION 18"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T004 MULTIPLICATION 18"
               DISPLAY "     ACTUAL   = " M18-C
               DISPLAY "     EXPECTED = " M18-EXPECTED
           END-IF.
       TEST-ROUNDED-DIVISION-18.
           ADD 1 TO TESTS-RUN.
           MOVE 1.000000 TO R18-A.
           MOVE 3.000000 TO R18-B.
           MOVE 0.333333 TO R18-EXPECTED.
           COMPUTE R18-C ROUNDED = R18-A / R18-B.
           IF R18-C = R18-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T005 ROUNDED DIVISION 18"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T005 ROUNDED DIVISION 18"
               DISPLAY "     ACTUAL   = " R18-C
               DISPLAY "     EXPECTED = " R18-EXPECTED
           END-IF.
       TEST-31-LOWEST-FRACTION.
           ADD 1 TO TESTS-RUN.
           MOVE 0.0000000000001 TO N31-A.
           MOVE 0.0000000000001 TO N31-B.
           MOVE 0.0000000000002 TO N31-EXPECTED.
           COMPUTE N31-C = N31-A + N31-B.
           IF N31-C = N31-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T006 LOWEST FRACTION 31"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T006 LOWEST FRACTION 31"
               DISPLAY "     ACTUAL   = " N31-C
               DISPLAY "     EXPECTED = " N31-EXPECTED
           END-IF.
       TEST-31-LARGE-ADDITION.
           ADD 1 TO TESTS-RUN.
           MOVE 123456789012345678.1234567890123 TO N31-A.
           MOVE 1.0000000000001                  TO N31-B.
           MOVE 123456789012345679.1234567890124 TO N31-EXPECTED.
           COMPUTE N31-C = N31-A + N31-B.
           IF N31-C = N31-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T007 LARGE ADDITION 31"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T007 LARGE ADDITION 31"
               DISPLAY "     ACTUAL   = " N31-C
               DISPLAY "     EXPECTED = " N31-EXPECTED
           END-IF.
       TEST-31-FRACTIONAL-CARRY.
           ADD 1 TO TESTS-RUN.
           MOVE 999999999999999997.9999999999999 TO N31-A.
           MOVE 1.0000000000001                  TO N31-B.
           MOVE 999999999999999999.0000000000000 TO N31-EXPECTED.
           COMPUTE N31-C = N31-A + N31-B.
           IF N31-C = N31-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T008 FRACTIONAL CARRY 31"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T008 FRACTIONAL CARRY 31"
               DISPLAY "     ACTUAL   = " N31-C
               DISPLAY "     EXPECTED = " N31-EXPECTED
           END-IF.
       TEST-31-MULTIPLICATION.
           ADD 1 TO TESTS-RUN.
           MOVE 1000000000000.0000000000001 TO M31-A.
           MOVE 2.0000000000000             TO M31-B.
           MOVE 2000000000000.0000000000002 TO M31-EXPECTED.
           COMPUTE M31-C = M31-A * M31-B.
           IF M31-C = M31-EXPECTED
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T009 MULTIPLICATION 31"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T009 MULTIPLICATION 31"
               DISPLAY "     ACTUAL   = " M31-C
               DISPLAY "     EXPECTED = " M31-EXPECTED
           END-IF.
       TEST-SIZE-ERROR-31.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO SIZE-ERROR-FLAG.
           MOVE 999999999999999999.9999999999999 TO N31-A.
           MOVE 1.0000000000000                  TO N31-B.
           MOVE ZERO                             TO N31-C.
           ADD N31-A TO N31-B
               GIVING N31-C
               ON SIZE ERROR
                   MOVE "Y" TO SIZE-ERROR-FLAG
           END-ADD.
           IF SIZE-ERROR-FLAG = "Y"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T010 SIZE ERROR 31"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T010 SIZE ERROR 31"
               DISPLAY "     SIZE ERROR WAS NOT RAISED"
               DISPLAY "     RESULT = " N31-C
           END-IF.

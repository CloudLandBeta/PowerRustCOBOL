       IDENTIFICATION DIVISION.
       PROGRAM-ID. COPYTEST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 TEST-COUNTERS.
          05 TESTS-RUN    PIC 9(4) VALUE 0.
          05 TESTS-PASSED PIC 9(4) VALUE 0.
          05 TESTS-FAILED PIC 9(4) VALUE 0.
       01 WS-CUST.
       COPY CUSTREC REPLACING ==:PFX:== BY ==WS==.
       COPY OUTER.
       PROCEDURE DIVISION.
       MAIN.
           DISPLAY "COPYBOOK TEST SUITE".
           DISPLAY "-------------------".
           PERFORM T001-COPY-ALPHA.
           PERFORM T002-COPY-NUMERIC.
           PERFORM T003-NESTED-COPY.
           DISPLAY "-------------------".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
              DISPLAY "RESULT       : PASS"
           ELSE
              DISPLAY "RESULT       : FAIL"
           END-IF.
           STOP RUN.
       T001-COPY-ALPHA.
           ADD 1 TO TESTS-RUN.
           MOVE "ACME" TO WS-NAME.
           IF WS-NAME = "ACME"
              ADD 1 TO TESTS-PASSED
              DISPLAY "PASS T001 COPY ALPHA FIELD (REPLACING :PFX: -> WS)"
           ELSE
              ADD 1 TO TESTS-FAILED
              DISPLAY "FAIL T001 COPY ALPHA [" WS-NAME "]"
           END-IF.
       T002-COPY-NUMERIC.
           ADD 1 TO TESTS-RUN.
           MOVE 100.50 TO WS-BALANCE.
           IF WS-BALANCE = 100.50
              ADD 1 TO TESTS-PASSED
              DISPLAY "PASS T002 COPY NUMERIC FIELD"
           ELSE
              ADD 1 TO TESTS-FAILED
              DISPLAY "FAIL T002 COPY NUMERIC"
           END-IF.
       T003-NESTED-COPY.
           ADD 1 TO TESTS-RUN.
           MOVE 123 TO INNER-CODE.
           IF INNER-CODE = 123
              ADD 1 TO TESTS-PASSED
              DISPLAY "PASS T003 NESTED COPY (OUTER -> INNER)"
           ELSE
              ADD 1 TO TESTS-FAILED
              DISPLAY "FAIL T003 NESTED COPY"
           END-IF.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TCPYREP.

      *---------------------------------------------------------------*
      * TCPYREP                                                       *
      * Complete COBOL-85 style unit test for COPY and COPY REPLACING.*
      *---------------------------------------------------------------*

       ENVIRONMENT DIVISION.

       DATA DIVISION.
       WORKING-STORAGE SECTION.

       01  TEST-COUNTERS.
           05 TESTS-RUN                 PIC 9(4) VALUE 0.
           05 TESTS-PASSED              PIC 9(4) VALUE 0.
           05 TESTS-FAILED              PIC 9(4) VALUE 0.

       01  TEST-CONTEXT.
           05 TEST-ID                   PIC X(8)  VALUE SPACES.
           05 TEST-NAME                 PIC X(60) VALUE SPACES.
           05 ACTUAL-RESULT             PIC X(80) VALUE SPACES.
           05 EXPECTED-RESULT           PIC X(80) VALUE SPACES.

           COPY CRBASE.

           COPY CRREC
               REPLACING
                   ==REC-AREA==         BY ==CUSTOMER-AREA==
                   ==REC-ID==           BY ==CUSTOMER-ID==
                   ==REC-NAME==         BY ==CUSTOMER-NAME==
                   ==REC-AMOUNT==       BY ==CUSTOMER-AMOUNT==
                   =="DEFAULT-NAME"==   BY =="ALICE"==.

           COPY CRREC
               REPLACING
                   ==REC-AREA==         BY ==VENDOR-AREA==
                   ==REC-ID==           BY ==VENDOR-ID==
                   ==REC-NAME==         BY ==VENDOR-NAME==
                   ==REC-AMOUNT==       BY ==VENDOR-AMOUNT==
                   =="DEFAULT-NAME"==   BY =="BOB"==.

           COPY CRPIC
               REPLACING
                   ==GEN-FIELD==        BY ==SHORT-FIELD==
                   ==PIC X(10)==        BY ==PIC X(05)==
                   ==VALUE "ABCDEFGHIJ"==
                                        BY ==VALUE "ABCDE"==.

           COPY CRPIC
               REPLACING
                   ==GEN-FIELD==        BY ==LONG-FIELD==
                   ==PIC X(10)==        BY ==PIC X(12)==
                   ==VALUE "ABCDEFGHIJ"==
                                        BY ==VALUE "ABCDEFGHIJKL"==.

           COPY CRPART
               REPLACING
                   ==BASE==             BY ==CLIENT==.

       01  COMPUTE-SOURCES.
           05 ADD-A                     PIC 9(4) VALUE 0.
           05 ADD-B                     PIC 9(4) VALUE 0.
           05 ADD-C                     PIC 9(5) VALUE 0.

       PROCEDURE DIVISION.

       MAIN-PARA.

           DISPLAY "COPY / REPLACE TEST SUITE".
           DISPLAY "-------------------------".

           PERFORM TEST-001-COPY-DATA-VALUES.
           PERFORM TEST-002-COPY-PROCEDURE-NO-REPLACE.
           PERFORM TEST-003-REPLACE-FIRST-RECORD.
           PERFORM TEST-004-REPLACE-SECOND-RECORD.
           PERFORM TEST-005-REPLACE-PIC-SHORT.
           PERFORM TEST-006-REPLACE-PIC-LONG.
           PERFORM TEST-007-PROCEDURE-REPLACE-CUSTOMER.
           PERFORM TEST-008-PROCEDURE-REPLACE-VENDOR.
           PERFORM TEST-009-PARTIAL-WORD-NOT-REPLACED.
           PERFORM TEST-010-COPY-REPLACING-ARITHMETIC.

           DISPLAY "-------------------------".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.

           IF TESTS-FAILED = ZERO
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF.

           STOP RUN.

       ASSERT-RESULT.

           ADD 1 TO TESTS-RUN.

           IF ACTUAL-RESULT = EXPECTED-RESULT
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS " TEST-ID " " TEST-NAME
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL " TEST-ID " " TEST-NAME
               DISPLAY "     ACTUAL   = [" ACTUAL-RESULT "]"
               DISPLAY "     EXPECTED = [" EXPECTED-RESULT "]"
           END-IF.

       TEST-001-COPY-DATA-VALUES.

           MOVE "T001" TO TEST-ID.
           MOVE "COPY DATA WITHOUT REPLACING" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE BASE-CODE TO ACTUAL-RESULT.
           MOVE "BASE" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-002-COPY-PROCEDURE-NO-REPLACE.

           MOVE "T002" TO TEST-ID.
           MOVE "COPY PROCEDURE WITHOUT REPLACING" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE SPACES TO PROC-RESULT.
           COPY CRPROC.

           MOVE PROC-RESULT TO ACTUAL-RESULT.
           MOVE "PROC-COPIED" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-003-REPLACE-FIRST-RECORD.

           MOVE "T003" TO TEST-ID.
           MOVE "COPY REPLACING FIRST RECORD TEMPLATE" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE 1001 TO CUSTOMER-ID.
           MOVE 12345 TO CUSTOMER-AMOUNT.

           MOVE CUSTOMER-NAME TO ACTUAL-RESULT.
           MOVE "ALICE" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-004-REPLACE-SECOND-RECORD.

           MOVE "T004" TO TEST-ID.
           MOVE "COPY REPLACING SECOND RECORD TEMPLATE" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE 2002 TO VENDOR-ID.
           MOVE 54321 TO VENDOR-AMOUNT.

           MOVE VENDOR-NAME TO ACTUAL-RESULT.
           MOVE "BOB" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-005-REPLACE-PIC-SHORT.

           MOVE "T005" TO TEST-ID.
           MOVE "COPY REPLACING SHORT PIC CLAUSE" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE SHORT-FIELD TO ACTUAL-RESULT.
           MOVE "ABCDE" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-006-REPLACE-PIC-LONG.

           MOVE "T006" TO TEST-ID.
           MOVE "COPY REPLACING LONG PIC CLAUSE" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE LONG-FIELD TO ACTUAL-RESULT.
           MOVE "ABCDEFGHIJKL" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-007-PROCEDURE-REPLACE-CUSTOMER.

           MOVE "T007" TO TEST-ID.
           MOVE "PROCEDURE COPY REPLACING CUSTOMER" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           COPY CRADD
               REPLACING
                   ==SOURCE-ID==        BY ==CUSTOMER-ID==
                   ==SOURCE-NAME==      BY ==CUSTOMER-NAME==
                   ==TARGET-TEXT==      BY ==ACTUAL-RESULT==
                   =="TAG"==            BY =="CUSTOMER"==.

           MOVE "CUSTOMER:ALICE" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-008-PROCEDURE-REPLACE-VENDOR.

           MOVE "T008" TO TEST-ID.
           MOVE "PROCEDURE COPY REPLACING VENDOR" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           COPY CRADD
               REPLACING
                   ==SOURCE-ID==        BY ==VENDOR-ID==
                   ==SOURCE-NAME==      BY ==VENDOR-NAME==
                   ==TARGET-TEXT==      BY ==ACTUAL-RESULT==
                   =="TAG"==            BY =="VENDOR"==.

           MOVE "VENDOR:BOB" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

       TEST-009-PARTIAL-WORD-NOT-REPLACED.

           MOVE "T009" TO TEST-ID.
           MOVE "COPY REPLACING DOES NOT ALTER PARTIAL WORDS"
                TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE CLIENT TO ACTUAL-RESULT.
           MOVE "CLIENT" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "T009B" TO TEST-ID.
           MOVE "COPY REPLACING PRESERVES BASEMENT IDENTIFIER"
                TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE BASEMENT TO ACTUAL-RESULT.
           MOVE "BASEMENT" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

       TEST-010-COPY-REPLACING-ARITHMETIC.

           MOVE "T010" TO TEST-ID.
           MOVE "COPY REPLACING PROCEDURAL ARITHMETIC" TO TEST-NAME.
           MOVE SPACES TO ACTUAL-RESULT EXPECTED-RESULT.

           MOVE 1000 TO ADD-A.
           MOVE 234 TO ADD-B.
           MOVE ZERO TO ADD-C.

           COPY CRSUM
               REPLACING
                   ==LEFT-OPERAND==     BY ==ADD-A==
                   ==RIGHT-OPERAND==    BY ==ADD-B==
                   ==SUM-RESULT==       BY ==ADD-C==.

           MOVE ADD-C TO ACTUAL-RESULT.
           MOVE "01234" TO EXPECTED-RESULT.

           PERFORM ASSERT-RESULT.

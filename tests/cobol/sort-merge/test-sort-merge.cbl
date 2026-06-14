       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-SORT-MERGE
      *> SORT / MERGE and their conditional clauses: SORT with INPUT
      *> PROCEDURE / OUTPUT PROCEDURE, RELEASE, RETURN ... AT END /
      *> NOT AT END, ascending & descending keys, and MERGE of two
      *> ordered files via an OUTPUT PROCEDURE.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-SORT-MERGE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SF ASSIGN TO "/tmp/test-sm-sf.tmp".
           SELECT MF ASSIGN TO "/tmp/test-sm-mf.tmp".
           SELECT F1 ASSIGN TO "/tmp/test-sm-f1.tmp"
               ORGANIZATION IS LINE SEQUENTIAL.
           SELECT F2 ASSIGN TO "/tmp/test-sm-f2.tmp"
               ORGANIZATION IS LINE SEQUENTIAL.
       DATA DIVISION.
       FILE SECTION.
       SD  SF.
       01  SR.
           05 SR-K                 PIC 9(2).
       SD  MF.
       01  MR.
           05 MR-K                 PIC 9(2).
       FD  F1.
       01  F1-REC.
           05 F1-K                 PIC 9(2).
       FD  F2.
       01  F2-REC.
           05 F2-K                 PIC 9(2).
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-ORDER                PIC X(12) VALUE SPACES.
       01  WS-PTR                  PIC 9(2)  VALUE 1.
       01  WS-COUNT                PIC 9(2)  VALUE 0.
       01  EOFSW                   PIC X     VALUE "N".
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-SORT-MERGE".
           DISPLAY "Constructs: SORT INPUT/OUTPUT PROCEDURE,".
           DISPLAY "  RELEASE, RETURN AT END / NOT AT END,".
           DISPLAY "  ascending & descending keys, MERGE OUTPUT PROC".
           DISPLAY "--------------------------------------------".
           PERFORM SM001-SORT-ASCENDING.
           PERFORM SM002-SORT-DESCENDING.
           PERFORM SM003-RETURN-ATEND.
           PERFORM SM004-RETURN-COUNT.
           PERFORM SM005-MERGE.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 5".
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
       SM001-SORT-ASCENDING.
           ADD 1 TO TESTS-RUN.
           SORT SF ON ASCENDING KEY SR-K
               INPUT PROCEDURE IS FEED-RECORDS
               OUTPUT PROCEDURE IS DRAIN-SORT.
           IF WS-ORDER = "102030"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  SM001 SORT ASCENDING (30,10,20 -> 102030)".
       SM002-SORT-DESCENDING.
           ADD 1 TO TESTS-RUN.
           SORT SF ON DESCENDING KEY SR-K
               INPUT PROCEDURE IS FEED-RECORDS
               OUTPUT PROCEDURE IS DRAIN-SORT.
           IF WS-ORDER = "302010"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  SM002 SORT DESCENDING (-> 302010)".
       SM003-RETURN-ATEND.
           ADD 1 TO TESTS-RUN.
           SORT SF ON ASCENDING KEY SR-K
               INPUT PROCEDURE IS FEED-RECORDS
               OUTPUT PROCEDURE IS DRAIN-SORT.
           IF EOFSW = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  SM003 RETURN ... AT END reached (EOFSW=Y)".
       SM004-RETURN-COUNT.
           ADD 1 TO TESTS-RUN.
           SORT SF ON ASCENDING KEY SR-K
               INPUT PROCEDURE IS FEED-RECORDS
               OUTPUT PROCEDURE IS DRAIN-SORT.
           IF WS-COUNT = 3
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  SM004 RETURN NOT AT END ran 3 times".
       SM005-MERGE.
           ADD 1 TO TESTS-RUN.
           PERFORM BUILD-MERGE-FILES.
           MOVE SPACES TO WS-ORDER.
           MOVE 1 TO WS-PTR.
           MOVE 0 TO WS-COUNT.
           MOVE "N" TO EOFSW.
           MERGE MF ON ASCENDING KEY MR-K
               USING F1 F2
               OUTPUT PROCEDURE IS DRAIN-MERGE.
           IF WS-ORDER = "1020304050"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  SM005 MERGE two ordered files -> 1020304050".
      *> ---- shared SORT input / output procedures ----
       FEED-RECORDS.
           MOVE 30 TO SR-K. RELEASE SR.
           MOVE 10 TO SR-K. RELEASE SR.
           MOVE 20 TO SR-K. RELEASE SR.
       DRAIN-SORT.
           MOVE SPACES TO WS-ORDER.
           MOVE 1 TO WS-PTR.
           MOVE 0 TO WS-COUNT.
           MOVE "N" TO EOFSW.
           PERFORM UNTIL EOFSW = "Y"
               RETURN SF
                   AT END
                       MOVE "Y" TO EOFSW
                   NOT AT END
                       ADD 1 TO WS-COUNT
                       STRING SR-K DELIMITED BY SIZE
                           INTO WS-ORDER
                           WITH POINTER WS-PTR
                       END-STRING
               END-RETURN
           END-PERFORM.
      *> ---- MERGE input files + output procedure ----
       BUILD-MERGE-FILES.
           OPEN OUTPUT F1.
           MOVE 10 TO F1-K. WRITE F1-REC.
           MOVE 30 TO F1-K. WRITE F1-REC.
           MOVE 50 TO F1-K. WRITE F1-REC.
           CLOSE F1.
           OPEN OUTPUT F2.
           MOVE 20 TO F2-K. WRITE F2-REC.
           MOVE 40 TO F2-K. WRITE F2-REC.
           CLOSE F2.
       DRAIN-MERGE.
           PERFORM UNTIL EOFSW = "Y"
               RETURN MF
                   AT END
                       MOVE "Y" TO EOFSW
                   NOT AT END
                       ADD 1 TO WS-COUNT
                       STRING MR-K DELIMITED BY SIZE
                           INTO WS-ORDER
                           WITH POINTER WS-PTR
                       END-STRING
               END-RETURN
           END-PERFORM.
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

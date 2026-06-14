       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-DECLARATIVES
      *> DECLARATIVES with USE AFTER STANDARD ERROR PROCEDURE on an
      *> INDEXED file. Verifies the handler fires on an unhandled file
      *> error, does NOT fire on a successful operation, does NOT fire
      *> when the statement supplies its own INVALID KEY phrase, and
      *> does NOT run during normal (error-free) flow.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-DECLARATIVES.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUST-FILE ASSIGN TO "/tmp/test-decl-cust"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               FILE STATUS IS CUST-ST.
           SELECT MISS-FILE ASSIGN TO "/tmp/test-decl-missing"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS MISS-ID
               FILE STATUS IS MISS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD  CUST-FILE.
       01  CUST-REC.
           05 CUST-ID              PIC 9(5).
           05 CUST-NAME            PIC X(20).
       FD  MISS-FILE.
       01  MISS-REC.
           05 MISS-ID              PIC 9(5).
           05 MISS-NAME            PIC X(20).
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  CUST-ST                 PIC XX   VALUE "  ".
       01  MISS-ST                 PIC XX   VALUE "  ".
       01  WS-FIRED                PIC X    VALUE "N".
       01  WS-LAST-ST              PIC XX   VALUE "  ".
       01  WS-FLAG                 PIC XX   VALUE SPACES.
       PROCEDURE DIVISION.
       DECLARATIVES.
       CUST-ERROR-SECTION SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON CUST-FILE.
       CUST-ERROR-HANDLER.
           MOVE "Y" TO WS-FIRED.
           MOVE CUST-ST TO WS-LAST-ST.
       MISS-ERROR-SECTION SECTION.
           USE AFTER STANDARD ERROR PROCEDURE ON MISS-FILE.
       MISS-ERROR-HANDLER.
           MOVE "Y" TO WS-FIRED.
           MOVE MISS-ST TO WS-LAST-ST.
       END DECLARATIVES.
       MAIN SECTION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-DECLARATIVES".
           DISPLAY "Constructs: DECLARATIVES + USE AFTER STANDARD".
           DISPLAY "  ERROR PROCEDURE; handler fires on unhandled".
           DISPLAY "  file error, silent on success / phrase / flow".
           DISPLAY "--------------------------------------------".
           PERFORM DC001-OPEN-MISSING.
           PERFORM DC002-SUCCESS-SILENT.
           PERFORM DC003-PHRASE-WINS.
           PERFORM DC004-UNHANDLED-FIRES.
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
      *> ---- DC001: OPEN INPUT on a non-existent file fires the USE ----
       DC001-OPEN-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-FIRED.
           MOVE SPACES TO WS-LAST-ST.
           OPEN INPUT MISS-FILE.
           IF WS-FIRED = "Y" AND WS-LAST-ST = "35"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  DC001 OPEN INPUT missing -> USE fires (st 35)".
      *> ---- DC002: a successful operation does NOT fire the USE ----
       DC002-SUCCESS-SILENT.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-FIRED.
           OPEN OUTPUT CUST-FILE.
           MOVE 1 TO CUST-ID.
           MOVE "ALICE" TO CUST-NAME.
           WRITE CUST-REC.
           CLOSE CUST-FILE.
           IF WS-FIRED = "N"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  DC002 OPEN/WRITE/CLOSE ok -> USE silent".
      *> ---- DC003: an INVALID KEY phrase takes precedence over USE ----
       DC003-PHRASE-WINS.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-FIRED.
           MOVE SPACES TO WS-FLAG.
           OPEN I-O CUST-FILE.
           MOVE 999 TO CUST-ID.
           READ CUST-FILE
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-READ.
           IF WS-FIRED = "N" AND WS-FLAG = "IK"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  DC003 READ missing WITH phrase -> USE silent".
      *> ---- DC004: an unhandled error (no phrase) fires the USE ----
       DC004-UNHANDLED-FIRES.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-FIRED.
           MOVE SPACES TO WS-LAST-ST.
           MOVE 888 TO CUST-ID.
           DELETE CUST-FILE.
           IF WS-FIRED = "Y" AND WS-LAST-ST = "23"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           CLOSE CUST-FILE.
           DISPLAY "  DC004 DELETE missing, no phrase -> USE fires".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

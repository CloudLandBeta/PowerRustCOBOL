       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-FILE-CONDITIONS
      *> File conditional clauses on an INDEXED file (ACCESS DYNAMIC,
      *> RECORD KEY, FILE STATUS): WRITE / READ / REWRITE / DELETE /
      *> START with INVALID KEY and NOT INVALID KEY branches, and
      *> sequential READ ... AT END / NOT AT END. Expected FILE STATUS
      *> values are asserted throughout.
      *>
      *> Per the project rule, the bulk phases are timed: the summary
      *> reports how long the load and the sequential scan took and the
      *> throughput, in addition to the pass/fail tally.
      *> ============================================================
       PROGRAM-ID. TEST-FILE-CONDITIONS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUST-FILE ASSIGN TO "/tmp/test-fc-cust"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               FILE STATUS IS CUST-ST.
       DATA DIVISION.
       FILE SECTION.
       FD  CUST-FILE.
       01  CUST-REC.
           05 CUST-ID              PIC 9(5).
           05 CUST-NAME            PIC X(20).
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  CUST-ST                 PIC XX    VALUE "  ".
       01  WS-I                    PIC 9(5)  VALUE 0.
       01  WS-EOF                  PIC X     VALUE "N".
       01  WS-WRITE-FAILS          PIC 9(5)  VALUE 0.
       01  WS-READ-COUNT           PIC 9(5)  VALUE 0.
       01  WS-FLAG                 PIC XX    VALUE SPACES.
       01  REC-TOTAL               PIC 9(5)  VALUE 1000.
      *> ---- timing (HHMMSScc from ACCEPT FROM TIME) ----
       01  WS-TIME.
           05 WS-T-HH              PIC 9(2).
           05 WS-T-MM              PIC 9(2).
           05 WS-T-SS              PIC 9(2).
           05 WS-T-CC              PIC 9(2).
       01  WS-T-START              PIC 9(8) VALUE 0.
       01  WS-T-NOW                PIC 9(8) VALUE 0.
       01  WS-EL-LOAD              PIC 9(8) VALUE 0.
       01  WS-EL-SCAN              PIC 9(8) VALUE 0.
       01  WS-RATE                 PIC 9(9) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-FILE-CONDITIONS  (INDEXED)".
           DISPLAY "Constructs: WRITE/READ/REWRITE/DELETE/START".
           DISPLAY "  INVALID KEY & NOT INVALID KEY; READ NEXT".
           DISPLAY "  AT END / NOT AT END; FILE STATUS checks".
           DISPLAY "--------------------------------------------".
           PERFORM FC001-LOAD-BULK.
           PERFORM FC002-WRITE-DUP.
           PERFORM FC003-READ-OK.
           PERFORM FC004-READ-MISSING.
           PERFORM FC005-REWRITE-OK.
           PERFORM FC006-REWRITE-MISSING.
           PERFORM FC007-START-OK.
           PERFORM FC008-START-MISSING.
           PERFORM FC009-SEQ-ATEND.
           PERFORM FC010-DELETE-OK.
           PERFORM FC011-DELETE-MISSING.
           DISPLAY "--------------------------------------------".
           DISPLAY "PERFORMANCE (reference)".
           DISPLAY "  records loaded     : " REC-TOTAL.
           DISPLAY "  load elapsed (cs)  : " WS-EL-LOAD.
           PERFORM SHOW-LOAD-RATE.
           DISPLAY "  scan elapsed (cs)  : " WS-EL-SCAN.
           DISPLAY "  records scanned    : " WS-READ-COUNT.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 11".
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
      *> ---- FC001: bulk WRITE (timed) + NOT INVALID KEY ----
       FC001-LOAD-BULK.
           ADD 1 TO TESTS-RUN.
           OPEN OUTPUT CUST-FILE.
           PERFORM CAPTURE-START.
           MOVE 0 TO WS-WRITE-FAILS.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > REC-TOTAL
               MOVE WS-I TO CUST-ID
               MOVE "CUSTOMER"  TO CUST-NAME
               WRITE CUST-REC
                   INVALID KEY     ADD 1 TO WS-WRITE-FAILS
                   NOT INVALID KEY CONTINUE
               END-WRITE
           END-PERFORM.
           PERFORM CAPTURE-ELAPSED.
           MOVE WS-T-NOW TO WS-EL-LOAD.
           CLOSE CUST-FILE.
           IF WS-WRITE-FAILS = 0
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC001 WRITE 1000 keys -> all NOT INVALID KEY".
      *> ---- FC002: duplicate WRITE -> INVALID KEY (status 22) ----
       FC002-WRITE-DUP.
           ADD 1 TO TESTS-RUN.
           OPEN I-O CUST-FILE.
           MOVE SPACES TO WS-FLAG.
           MOVE 1 TO CUST-ID.
           MOVE "DUPLICATE" TO CUST-NAME.
           WRITE CUST-REC
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-WRITE.
           IF WS-FLAG = "IK" AND CUST-ST = "22"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC002 WRITE duplicate key 1 -> INVALID KEY (22)".
      *> ---- FC003: random READ existing -> NOT INVALID KEY (00) ----
       FC003-READ-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 500 TO CUST-ID.
           READ CUST-FILE
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-READ.
           IF WS-FLAG = "OK" AND CUST-ST = "00"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC003 READ key 500 -> NOT INVALID KEY (00)".
      *> ---- FC004: random READ missing -> INVALID KEY (23) ----
       FC004-READ-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 99999 TO CUST-ID.
           READ CUST-FILE
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-READ.
           IF WS-FLAG = "IK" AND CUST-ST = "23"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC004 READ key 99999 -> INVALID KEY (23)".
      *> ---- FC005: REWRITE existing -> NOT INVALID KEY (00) ----
       FC005-REWRITE-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 500 TO CUST-ID.
           MOVE "UPDATED-NAME" TO CUST-NAME.
           REWRITE CUST-REC
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-REWRITE.
           IF WS-FLAG = "OK" AND CUST-ST = "00"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC005 REWRITE key 500 -> NOT INVALID KEY (00)".
      *> ---- FC006: REWRITE missing -> INVALID KEY (23) ----
       FC006-REWRITE-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 99999 TO CUST-ID.
           MOVE "GHOST" TO CUST-NAME.
           REWRITE CUST-REC
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-REWRITE.
           IF WS-FLAG = "IK" AND CUST-ST = "23"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC006 REWRITE key 99999 -> INVALID KEY (23)".
      *> ---- FC007: START >= existing -> NOT INVALID KEY (00) ----
       FC007-START-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 500 TO CUST-ID.
           START CUST-FILE KEY >= CUST-ID
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-START.
           IF WS-FLAG = "OK" AND CUST-ST = "00"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC007 START KEY >= 500 -> NOT INVALID KEY (00)".
      *> ---- FC008: START past end -> INVALID KEY (23) ----
       FC008-START-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 99999 TO CUST-ID.
           START CUST-FILE KEY > CUST-ID
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-START.
           IF WS-FLAG = "IK" AND CUST-ST = "23"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC008 START KEY > 99999 -> INVALID KEY (23)".
      *> ---- FC009: sequential READ NEXT to AT END (timed scan) ----
       FC009-SEQ-ATEND.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-EOF.
           MOVE 0 TO WS-READ-COUNT.
           MOVE 1 TO CUST-ID.
           START CUST-FILE KEY >= CUST-ID
               INVALID KEY CONTINUE
               NOT INVALID KEY CONTINUE
           END-START.
           PERFORM CAPTURE-START.
           PERFORM UNTIL WS-EOF = "Y"
               READ CUST-FILE NEXT
                   AT END     MOVE "Y" TO WS-EOF
                   NOT AT END ADD 1 TO WS-READ-COUNT
               END-READ
           END-PERFORM.
           PERFORM CAPTURE-ELAPSED.
           MOVE WS-T-NOW TO WS-EL-SCAN.
           IF WS-EOF = "Y" AND WS-READ-COUNT = REC-TOTAL
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC009 READ NEXT to AT END -> 1000 then AT END".
      *> ---- FC010: DELETE existing -> NOT INVALID KEY (00) ----
       FC010-DELETE-OK.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 500 TO CUST-ID.
           DELETE CUST-FILE
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-DELETE.
           IF WS-FLAG = "OK" AND CUST-ST = "00"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  FC010 DELETE key 500 -> NOT INVALID KEY (00)".
      *> ---- FC011: DELETE missing -> INVALID KEY (23) ----
       FC011-DELETE-MISSING.
           ADD 1 TO TESTS-RUN.
           MOVE SPACES TO WS-FLAG.
           MOVE 99999 TO CUST-ID.
           DELETE CUST-FILE
               INVALID KEY     MOVE "IK" TO WS-FLAG
               NOT INVALID KEY MOVE "OK" TO WS-FLAG
           END-DELETE.
           IF WS-FLAG = "IK" AND CUST-ST = "23"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           CLOSE CUST-FILE.
           DISPLAY "  FC011 DELETE key 99999 -> INVALID KEY (23)".
      *> ---- timing helpers (centiseconds within the day) ----
       CAPTURE-START.
           ACCEPT WS-TIME FROM TIME.
           COMPUTE WS-T-START =
               (WS-T-HH * 360000) + (WS-T-MM * 6000)
               + (WS-T-SS * 100) + WS-T-CC.
       CAPTURE-ELAPSED.
           ACCEPT WS-TIME FROM TIME.
           COMPUTE WS-T-NOW =
               (WS-T-HH * 360000) + (WS-T-MM * 6000)
               + (WS-T-SS * 100) + WS-T-CC - WS-T-START.
       SHOW-LOAD-RATE.
           IF WS-EL-LOAD > 0
               COMPUTE WS-RATE = (REC-TOTAL * 100) / WS-EL-LOAD
               DISPLAY "  load rate (rec/s) : " WS-RATE
           ELSE
               DISPLAY "  load rate (rec/s) : (under 1 cs; very fast)"
           END-IF.
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

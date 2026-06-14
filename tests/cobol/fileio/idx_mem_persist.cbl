       IDENTIFICATION DIVISION.
      *> ============================================================
      *> IDX-MEM-PERSIST
      *> STORAGE IS MEMORY persistence policy (PowerRustCOBOL ext.):
      *>   - default MEMORY is EPHEMERAL: COMMIT/CLOSE do NOT write to
      *>     disk, so data is gone after CLOSE/reopen.
      *>   - MEMORY WITH PERSISTENCE writes to disk on CLOSE (only),
      *>     so data survives CLOSE/reopen.
      *>   - OPEN OUTPUT always (re)creates the disk file in both modes.
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. IDX-MEM-PERSIST.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT PERS-FILE ASSIGN TO "/tmp/idx-mem-pers.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY WITH PERSISTENCE
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS P-ID
               FILE STATUS IS P-ST.
           SELECT EPH-FILE ASSIGN TO "/tmp/idx-mem-eph.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS E-ID
               FILE STATUS IS E-ST.
       DATA DIVISION.
       FILE SECTION.
       FD  PERS-FILE.
       01  PERS-REC.
           05 P-ID                 PIC 9(3).
           05 P-NAME               PIC X(8).
       FD  EPH-FILE.
       01  EPH-REC.
           05 E-ID                 PIC 9(3).
           05 E-NAME               PIC X(8).
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  P-ST                    PIC XX   VALUE "  ".
       01  E-ST                    PIC XX   VALUE "  ".
       01  WS-PCOUNT               PIC 9(3) VALUE 0.
       01  WS-ECOUNT               PIC 9(3) VALUE 0.
       01  WS-EOPEN-ST             PIC XX   VALUE "  ".
       01  WS-FOUND-NAME           PIC X(8) VALUE SPACES.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "IDX-MEM-PERSIST".
           DISPLAY "STORAGE IS MEMORY: ephemeral (default) vs".
           DISPLAY "  WITH PERSISTENCE (save on CLOSE only)".
           DISPLAY "--------------------------------------------".
           PERFORM BUILD-PERSISTENT.
           PERFORM VERIFY-PERSISTENT.
           PERFORM BUILD-EPHEMERAL.
           PERFORM VERIFY-EPHEMERAL.
           PERFORM MP001-PERS-COUNT.
           PERFORM MP002-PERS-VALUE.
           PERFORM MP003-EPH-FILE-CREATED.
           PERFORM MP004-EPH-DISCARDED.
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
      *> ---- write 3 records to the PERSISTENT memory file, COMMIT, CLOSE ----
       BUILD-PERSISTENT.
           OPEN OUTPUT PERS-FILE.
           MOVE 1 TO P-ID. MOVE "ALPHA" TO P-NAME. WRITE PERS-REC.
           MOVE 2 TO P-ID. MOVE "BETA"  TO P-NAME. WRITE PERS-REC.
           MOVE 3 TO P-ID. MOVE "GAMMA" TO P-NAME. WRITE PERS-REC.
           COMMIT.
           CLOSE PERS-FILE.
      *> ---- reopen the persistent file and count / fetch ----
       VERIFY-PERSISTENT.
           MOVE 0 TO WS-PCOUNT.
           MOVE SPACES TO WS-FOUND-NAME.
           OPEN INPUT PERS-FILE.
      *>   Sequential count first (cursor at start), then a random read.
           PERFORM UNTIL P-ST NOT = "00"
               READ PERS-FILE NEXT
                   AT END MOVE "10" TO P-ST
                   NOT AT END ADD 1 TO WS-PCOUNT
               END-READ
           END-PERFORM.
           MOVE 2 TO P-ID.
           READ PERS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE P-NAME TO WS-FOUND-NAME
           END-READ.
           CLOSE PERS-FILE.
      *> ---- write 3 records to the EPHEMERAL memory file, COMMIT, CLOSE ----
       BUILD-EPHEMERAL.
           OPEN OUTPUT EPH-FILE.
           MOVE 1 TO E-ID. MOVE "ONE"   TO E-NAME. WRITE EPH-REC.
           MOVE 2 TO E-ID. MOVE "TWO"   TO E-NAME. WRITE EPH-REC.
           MOVE 3 TO E-ID. MOVE "THREE" TO E-NAME. WRITE EPH-REC.
           COMMIT.
           CLOSE EPH-FILE.
      *> ---- reopen the ephemeral file: it exists (OPEN OUTPUT made it),
      *>      but COMMIT/CLOSE persisted nothing, so it is empty ----
       VERIFY-EPHEMERAL.
           MOVE 0 TO WS-ECOUNT.
           OPEN INPUT EPH-FILE.
           MOVE E-ST TO WS-EOPEN-ST.
           PERFORM UNTIL E-ST NOT = "00"
               READ EPH-FILE NEXT
                   AT END MOVE "10" TO E-ST
                   NOT AT END ADD 1 TO WS-ECOUNT
               END-READ
           END-PERFORM.
           CLOSE EPH-FILE.
      *> ---- assertions ----
       MP001-PERS-COUNT.
           ADD 1 TO TESTS-RUN.
           IF WS-PCOUNT = 3
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP001 WITH PERSISTENCE survives CLOSE/reopen (3)".
       MP002-PERS-VALUE.
           ADD 1 TO TESTS-RUN.
           IF WS-FOUND-NAME = "BETA"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP002 persisted record value intact (key 2=BETA)".
       MP003-EPH-FILE-CREATED.
           ADD 1 TO TESTS-RUN.
           IF WS-EOPEN-ST = "00"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP003 OPEN OUTPUT created the ephemeral file (st 00)".
       MP004-EPH-DISCARDED.
           ADD 1 TO TESTS-RUN.
           IF WS-ECOUNT = 0
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP004 ephemeral: COMMIT/CLOSE persisted nothing (0)".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

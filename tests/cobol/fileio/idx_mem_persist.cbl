       IDENTIFICATION DIVISION.
      *> ============================================================
      *> IDX-MEM-PERSIST
      *> STORAGE IS MEMORY policy (PowerRustCOBOL ext., 1.80.37):
      *>   - MEMORY WITH PERSISTENCE writes to disk on CLOSE (only),
      *>     so data survives CLOSE/reopen, and it is written in the
      *>     one at-rest format: a STORAGE IS DISK program reads the
      *>     same file in place.
      *>   - MEMORY without PERSISTENCE is a READ-ONLY copy held for
      *>     fast queries: OPEN INPUT reads the file; OPEN OUTPUT,
      *>     I-O and EXTEND are refused with FILE STATUS 37, and the
      *>     file on disk is left exactly as it was.
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
           SELECT DISK-FILE ASSIGN TO "/tmp/idx-mem-pers.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS DISK
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS D-ID
               FILE STATUS IS D-ST.
           SELECT QUERY-FILE ASSIGN TO "/tmp/idx-mem-pers.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS Q-ID
               FILE STATUS IS Q-ST.
       DATA DIVISION.
       FILE SECTION.
       FD  PERS-FILE.
       01  PERS-REC.
           05 P-ID                 PIC 9(3).
           05 P-NAME               PIC X(8).
       FD  DISK-FILE.
       01  DISK-REC.
           05 D-ID                 PIC 9(3).
           05 D-NAME               PIC X(8).
       FD  QUERY-FILE.
       01  QUERY-REC.
           05 Q-ID                 PIC 9(3).
           05 Q-NAME               PIC X(8).
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  P-ST                    PIC XX   VALUE "  ".
       01  D-ST                    PIC XX   VALUE "  ".
       01  Q-ST                    PIC XX   VALUE "  ".
       01  WS-PCOUNT               PIC 9(3) VALUE 0.
       01  WS-DCOUNT               PIC 9(3) VALUE 0.
       01  WS-QCOUNT               PIC 9(3) VALUE 0.
       01  WS-AFTER-COUNT          PIC 9(3) VALUE 0.
       01  WS-FOUND-NAME           PIC X(8) VALUE SPACES.
       01  WS-QUERY-NAME           PIC X(8) VALUE SPACES.
       01  WS-OUTPUT-ST            PIC XX   VALUE "  ".
       01  WS-IO-ST                PIC XX   VALUE "  ".
       01  WS-EXTEND-ST            PIC XX   VALUE "  ".
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "IDX-MEM-PERSIST".
           DISPLAY "STORAGE IS MEMORY: WITH PERSISTENCE saves on".
           DISPLAY "  CLOSE; without it the file is read-only".
           DISPLAY "--------------------------------------------".
           PERFORM BUILD-PERSISTENT.
           PERFORM VERIFY-PERSISTENT.
           PERFORM VERIFY-AS-DISK.
           PERFORM QUERY-READ-ONLY.
           PERFORM QUERY-WRITES-REFUSED.
           PERFORM COUNT-AFTER-REFUSALS.
           PERFORM MP001-PERS-COUNT.
           PERFORM MP002-PERS-VALUE.
           PERFORM MP003-DISK-READS-IT.
           PERFORM MP004-QUERY-READS.
           PERFORM MP005-OUTPUT-REFUSED.
           PERFORM MP006-IO-REFUSED.
           PERFORM MP007-EXTEND-REFUSED.
           PERFORM MP008-FILE-UNTOUCHED.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 8".
           DISPLAY "  MEMORY WITH PERSISTENCE: OPEN OUTPUT, WRITE x3,".
           DISPLAY "    COMMIT, CLOSE; OPEN INPUT, READ NEXT, READ key".
           DISPLAY "  STORAGE IS DISK on the same file: READ NEXT".
           DISPLAY "  MEMORY (read-only): OPEN INPUT, READ NEXT,".
           DISPLAY "    READ key; OPEN OUTPUT / I-O / EXTEND -> 37".
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
      *> ---- the same file through a STORAGE IS DISK program ----
       VERIFY-AS-DISK.
           MOVE 0 TO WS-DCOUNT.
           OPEN INPUT DISK-FILE.
           PERFORM UNTIL D-ST NOT = "00"
               READ DISK-FILE NEXT
                   AT END MOVE "10" TO D-ST
                   NOT AT END ADD 1 TO WS-DCOUNT
               END-READ
           END-PERFORM.
           CLOSE DISK-FILE.
      *> ---- the read-only copy: it reads ----
       QUERY-READ-ONLY.
           MOVE 0 TO WS-QCOUNT.
           MOVE SPACES TO WS-QUERY-NAME.
           OPEN INPUT QUERY-FILE.
           PERFORM UNTIL Q-ST NOT = "00"
               READ QUERY-FILE NEXT
                   AT END MOVE "10" TO Q-ST
                   NOT AT END ADD 1 TO WS-QCOUNT
               END-READ
           END-PERFORM.
           MOVE 3 TO Q-ID.
           READ QUERY-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE Q-NAME TO WS-QUERY-NAME
           END-READ.
           CLOSE QUERY-FILE.
      *> ---- …and refuses every mode that could change the file ----
       QUERY-WRITES-REFUSED.
           OPEN OUTPUT QUERY-FILE.
           MOVE Q-ST TO WS-OUTPUT-ST.
           OPEN I-O QUERY-FILE.
           MOVE Q-ST TO WS-IO-ST.
           OPEN EXTEND QUERY-FILE.
           MOVE Q-ST TO WS-EXTEND-ST.
      *> ---- after the refusals, the file still holds its 3 records ----
       COUNT-AFTER-REFUSALS.
           MOVE 0 TO WS-AFTER-COUNT.
           OPEN INPUT DISK-FILE.
           PERFORM UNTIL D-ST NOT = "00"
               READ DISK-FILE NEXT
                   AT END MOVE "10" TO D-ST
                   NOT AT END ADD 1 TO WS-AFTER-COUNT
               END-READ
           END-PERFORM.
           CLOSE DISK-FILE.
      *> ---- assertions ----
       MP001-PERS-COUNT.
           ADD 1 TO TESTS-RUN.
           IF WS-PCOUNT = 3 PERFORM PASS-IT ELSE PERFORM FAIL-IT END-IF.
           DISPLAY "  MP001 WITH PERSISTENCE survives CLOSE/reopen (3)".
       MP002-PERS-VALUE.
           ADD 1 TO TESTS-RUN.
           IF WS-FOUND-NAME = "BETA"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP002 persisted record value intact (key 2=BETA)".
       MP003-DISK-READS-IT.
           ADD 1 TO TESTS-RUN.
           IF WS-DCOUNT = 3 PERFORM PASS-IT ELSE PERFORM FAIL-IT END-IF.
           DISPLAY "  MP003 a STORAGE IS DISK program reads it (3)".
       MP004-QUERY-READS.
           ADD 1 TO TESTS-RUN.
           IF WS-QCOUNT = 3 AND WS-QUERY-NAME = "GAMMA"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP004 read-only MEMORY reads it (3, key 3=GAMMA)".
       MP005-OUTPUT-REFUSED.
           ADD 1 TO TESTS-RUN.
           IF WS-OUTPUT-ST = "37"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP005 read-only MEMORY: OPEN OUTPUT -> 37".
       MP006-IO-REFUSED.
           ADD 1 TO TESTS-RUN.
           IF WS-IO-ST = "37" PERFORM PASS-IT ELSE PERFORM FAIL-IT END-IF.
           DISPLAY "  MP006 read-only MEMORY: OPEN I-O -> 37".
       MP007-EXTEND-REFUSED.
           ADD 1 TO TESTS-RUN.
           IF WS-EXTEND-ST = "37"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP007 read-only MEMORY: OPEN EXTEND -> 37".
       MP008-FILE-UNTOUCHED.
           ADD 1 TO TESTS-RUN.
           IF WS-AFTER-COUNT = 3
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  MP008 the refusals left the file as it was (3)".
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

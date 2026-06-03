      *>*****************************************************************
      *> IDXBASIC — indexed (ISAM) file regression suite.
      *>
      *> Exercises the full indexed verb set dispatched by ORGANIZATION
      *> INDEXED in the SELECT: OPEN OUTPUT/I-O/INPUT, WRITE, READ
      *> (random by RECORD KEY and sequential NEXT), REWRITE, DELETE and
      *> START (=, >, >=). Records are written out of key order to prove
      *> the engine keeps them in ascending primary-key order on disk.
      *>
      *> Self-checking: every assertion prints PASS Tnnn / FAIL Tnnn and a
      *> final RESULT line. All COBOL identifiers stay in English.
      *>*****************************************************************
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDXBASIC.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUST-FILE ASSIGN TO "idxbasic.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD CUST-FILE.
       01 CUST-REC.
          05 CUST-ID    PIC 9(4).
          05 CUST-NAME  PIC X(10).
       WORKING-STORAGE SECTION.
       01 FS        PIC XX.
       01 TCNO      PIC 9(3).
       01 WS-FAILS  PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
      *>--- load (out of key order: 1003, 1001, 1002) ---------------
           OPEN OUTPUT CUST-FILE
           MOVE 1003 TO CUST-ID  MOVE "CAROL" TO CUST-NAME
           WRITE CUST-REC
           MOVE 101 TO TCNO PERFORM REP
           MOVE 1001 TO CUST-ID  MOVE "ALICE" TO CUST-NAME
           WRITE CUST-REC
           MOVE 1002 TO CUST-ID  MOVE "BOB"   TO CUST-NAME
           WRITE CUST-REC
           CLOSE CUST-FILE

      *>--- random read by key + INVALID KEY phrase ----------------
           OPEN INPUT CUST-FILE
           MOVE 1002 TO CUST-ID
           READ CUST-FILE
               INVALID KEY MOVE 102 TO TCNO PERFORM FAIL
               NOT INVALID KEY MOVE 102 TO TCNO PERFORM PASS
           END-READ
           IF CUST-NAME = "BOB       "
               MOVE 103 TO TCNO PERFORM PASS
           ELSE
               MOVE 103 TO TCNO PERFORM FAIL
           END-IF
      *>--- missing key -> INVALID KEY fires, status 23 ------------
           MOVE 7777 TO CUST-ID
           READ CUST-FILE
               INVALID KEY MOVE 104 TO TCNO PERFORM PASS
               NOT INVALID KEY MOVE 104 TO TCNO PERFORM FAIL
           END-READ
      *>--- sequential ascending order -----------------------------
           MOVE 0 TO CUST-ID
           START CUST-FILE KEY IS GREATER THAN CUST-ID
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 1001 MOVE 105 TO TCNO PERFORM PASS
                             ELSE MOVE 105 TO TCNO PERFORM FAIL END-IF
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 1002 MOVE 106 TO TCNO PERFORM PASS
                             ELSE MOVE 106 TO TCNO PERFORM FAIL END-IF
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 1003 MOVE 107 TO TCNO PERFORM PASS
                             ELSE MOVE 107 TO TCNO PERFORM FAIL END-IF
           READ CUST-FILE NEXT AT END MOVE 108 TO TCNO PERFORM PASS
               NOT AT END MOVE 108 TO TCNO PERFORM FAIL END-READ
           CLOSE CUST-FILE

      *>--- update (REWRITE) + DELETE in I-O -----------------------
           OPEN I-O CUST-FILE
           MOVE 1002 TO CUST-ID
           READ CUST-FILE
           MOVE "ROBERT" TO CUST-NAME
           REWRITE CUST-REC
           MOVE 109 TO TCNO PERFORM REP
           MOVE 1003 TO CUST-ID
           DELETE CUST-FILE
           MOVE 110 TO TCNO PERFORM REP
      *>--- START >= 1002 lands on the rewritten 1002 --------------
           MOVE 1002 TO CUST-ID
           START CUST-FILE KEY IS NOT LESS THAN CUST-ID
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-NAME = "ROBERT    " MOVE 111 TO TCNO PERFORM PASS
               ELSE MOVE 111 TO TCNO PERFORM FAIL END-IF
      *>--- 1003 is gone -> EOF on next ----------------------------
           READ CUST-FILE NEXT AT END MOVE 112 TO TCNO PERFORM PASS
               NOT AT END MOVE 112 TO TCNO PERFORM FAIL END-READ
      *>--- duplicate primary key -> WRITE INVALID KEY -------------
           MOVE 1001 TO CUST-ID  MOVE "DUPE" TO CUST-NAME
           WRITE CUST-REC
               INVALID KEY MOVE 113 TO TCNO PERFORM PASS
               NOT INVALID KEY MOVE 113 TO TCNO PERFORM FAIL
           END-WRITE
           CLOSE CUST-FILE

           IF WS-FAILS = 0
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF
           STOP RUN.

      *>--- helpers ------------------------------------------------
       REP.
           IF FS = "00"
               PERFORM PASS
           ELSE
               PERFORM FAIL
           END-IF.
       PASS.
           DISPLAY "PASS T" TCNO.
       FAIL.
           ADD 1 TO WS-FAILS
           DISPLAY "FAIL T" TCNO " FS=" FS " ID=" CUST-ID.

      *>****************************************************************
      *> IDXSTORAGE — STORAGE IS DISK WITH COMPRESSION.
      *>
      *> Exercises the persistent on-disk B+tree backend through the full
      *> COBOL pipeline: an INDEXED file with a primary RECORD KEY and an
      *> ALTERNATE RECORD KEY WITH DUPLICATES, written out of key order,
      *> persisted to disk, reopened, and read back random + sequentially,
      *> plus REWRITE and DELETE. Records carry a roomy padded field so
      *> WITH COMPRESSION has something to crush.
      *>
      *> Self-checking: PASS Tnnn / FAIL Tnnn + a final RESULT line.
      *> All COBOL identifiers stay in English.
      *>****************************************************************
       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDXSTORAGE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUST-FILE
               STORAGE IS DISK WITH COMPRESSION
               ASSIGN TO "idxstorage.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               ALTERNATE RECORD KEY IS CUST-CITY WITH DUPLICATES
               FILE STATUS IS FS.
       DATA DIVISION.
       FILE SECTION.
       FD CUST-FILE.
       01 CUST-REC.
          05 CUST-ID    PIC 9(5).
          05 CUST-NAME  PIC X(20).
          05 CUST-CITY  PIC X(15).
       WORKING-STORAGE SECTION.
       01 FS        PIC XX.
       01 TCNO      PIC 9(3).
       01 WS-FAILS  PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
      *>--- load out of key order (300, 100, 200) ------------------
           OPEN OUTPUT CUST-FILE
           MOVE 300 TO CUST-ID MOVE "CAROL" TO CUST-NAME
           MOVE "LONDON" TO CUST-CITY  WRITE CUST-REC
           MOVE 101 TO TCNO PERFORM REP
           MOVE 100 TO CUST-ID MOVE "ALICE" TO CUST-NAME
           MOVE "PARIS" TO CUST-CITY   WRITE CUST-REC
           MOVE 200 TO CUST-ID MOVE "BOB" TO CUST-NAME
           MOVE "PARIS" TO CUST-CITY   WRITE CUST-REC
           CLOSE CUST-FILE

      *>--- reopen INPUT: random read + ascending scan ------------
           OPEN INPUT CUST-FILE
           MOVE 200 TO CUST-ID
           READ CUST-FILE
               INVALID KEY MOVE 102 TO TCNO PERFORM FAIL
               NOT INVALID KEY MOVE 102 TO TCNO PERFORM PASS
           END-READ
           IF CUST-NAME = "BOB                 "
               MOVE 103 TO TCNO PERFORM PASS
           ELSE
               MOVE 103 TO TCNO PERFORM FAIL
           END-IF
           MOVE 0 TO CUST-ID
           START CUST-FILE KEY IS GREATER THAN CUST-ID
               INVALID KEY MOVE 104 TO TCNO PERFORM FAIL
               NOT INVALID KEY MOVE 104 TO TCNO PERFORM PASS
           END-START
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 100 MOVE 105 TO TCNO PERFORM PASS
                            ELSE MOVE 105 TO TCNO PERFORM FAIL END-IF
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 200 MOVE 106 TO TCNO PERFORM PASS
                            ELSE MOVE 106 TO TCNO PERFORM FAIL END-IF
           READ CUST-FILE NEXT AT END CONTINUE END-READ
           IF CUST-ID = 300 MOVE 107 TO TCNO PERFORM PASS
                            ELSE MOVE 107 TO TCNO PERFORM FAIL END-IF
           CLOSE CUST-FILE

      *>--- I-O: REWRITE 200, DELETE 300 --------------------------
           OPEN I-O CUST-FILE
           MOVE 200 TO CUST-ID
           READ CUST-FILE
           MOVE "ROBERT" TO CUST-NAME
           REWRITE CUST-REC
           MOVE 108 TO TCNO PERFORM REP
           MOVE 300 TO CUST-ID
           DELETE CUST-FILE
           MOVE 109 TO TCNO PERFORM REP
           CLOSE CUST-FILE

      *>--- reopen: 200 updated, 300 gone -------------------------
           OPEN INPUT CUST-FILE
           MOVE 200 TO CUST-ID
           READ CUST-FILE
           IF CUST-NAME = "ROBERT              "
               MOVE 110 TO TCNO PERFORM PASS
           ELSE
               MOVE 110 TO TCNO PERFORM FAIL
           END-IF
           MOVE 300 TO CUST-ID
           READ CUST-FILE
               INVALID KEY MOVE 111 TO TCNO PERFORM PASS
               NOT INVALID KEY MOVE 111 TO TCNO PERFORM FAIL
           END-READ
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

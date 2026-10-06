       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC8.
      *> Spec 087 AC8 - units of work: EXEC SQL COMMIT/ROLLBACK against the
      *> COBOL verbs that govern INDEXED files, and WITH HOLD cursors.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IDX-FILE ASSIGN TO "@DIR@/esql-ac8.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS IDX-KEY
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  IDX-FILE.
       01  IDX-REC.
           05  IDX-KEY         PIC 9(4).
           05  IDX-DATA        PIC X(10).
       WORKING-STORAGE SECTION.
       01  WS-FS               PIC XX.
       01  WS-N                PIC 9(5).
       01  WS-ID               PIC 9(5).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
           EXEC SQL DECLARE C-HOLD CURSOR WITH HOLD FOR
                    SELECT ID FROM T ORDER BY ID END-EXEC.
           EXEC SQL DECLARE C-PLAIN CURSOR FOR
                    SELECT ID FROM T ORDER BY ID END-EXEC.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC8 END-EXEC
           EXEC SQL CREATE TABLE T (ID INTEGER) END-EXEC
           EXEC SQL COMMIT END-EXEC
      *> INSERT + ROLLBACK leaves no row; INSERT + COMMIT leaves it.
           EXEC SQL INSERT INTO T VALUES (1) END-EXEC
           EXEC SQL ROLLBACK END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM T END-EXEC
           PERFORM EXPECT-ZERO
           EXEC SQL INSERT INTO T VALUES (2) END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM T END-EXEC
           PERFORM EXPECT-ONE
      *> The COBOL ROLLBACK verb (INDEXED files) does not undo it.
           ROLLBACK
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM T END-EXEC
           PERFORM EXPECT-ONE
      *> Nor does the COBOL COMMIT verb keep uncommitted SQL work.
           EXEC SQL INSERT INTO T VALUES (3) END-EXEC
           COMMIT
           EXEC SQL ROLLBACK END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM T END-EXEC
           PERFORM EXPECT-ONE
      *> EXEC SQL ROLLBACK does not undo an INDEXED-file write.
           OPEN OUTPUT IDX-FILE
           MOVE 7 TO IDX-KEY
           MOVE "kept" TO IDX-DATA
           WRITE IDX-REC
           EXEC SQL ROLLBACK END-EXEC
           COMMIT
           CLOSE IDX-FILE
           OPEN INPUT IDX-FILE
           MOVE 7 TO IDX-KEY
           READ IDX-FILE KEY IS IDX-KEY
           IF WS-FS = "00" AND IDX-DATA = "kept"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL indexed write lost: FS " WS-FS
           END-IF
           CLOSE IDX-FILE
      *> WITH HOLD survives COMMIT; the other does not; ROLLBACK closes
      *> both.
           EXEC SQL OPEN C-HOLD END-EXEC
           EXEC SQL OPEN C-PLAIN END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL FETCH C-HOLD INTO :WS-ID END-EXEC
           IF SQLSTATE = "00000" AND WS-ID = 2
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL WITH HOLD cursor after COMMIT: " SQLSTATE
           END-IF
           EXEC SQL FETCH C-PLAIN INTO :WS-ID END-EXEC
           PERFORM EXPECT-CLOSED
           EXEC SQL ROLLBACK END-EXEC
           EXEC SQL FETCH C-HOLD INTO :WS-ID END-EXEC
           PERFORM EXPECT-CLOSED
           DISPLAY "=== ESQL AC8 RESULT ==="
           DISPLAY "forms: INSERT + EXEC SQL ROLLBACK; INSERT + EXEC "
                   "SQL COMMIT; COBOL ROLLBACK / COMMIT verbs beside SQL "
                   "work; INDEXED WRITE + EXEC SQL ROLLBACK; DECLARE "
                   "CURSOR WITH HOLD across COMMIT and ROLLBACK"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       EXPECT-ZERO.
           IF WS-N = 0
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL expected no row, found " WS-N
           END-IF.
       EXPECT-ONE.
           IF WS-N = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL expected one row, found " WS-N
           END-IF.
       EXPECT-CLOSED.
           IF SQLSTATE = "24000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL expected a closed cursor, got " SQLSTATE
           END-IF.

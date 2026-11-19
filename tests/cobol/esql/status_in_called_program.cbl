       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-CALLED-STATUS.
      *> ============================================================
      *> ESQL-CALLED-STATUS
      *> A CALLed program that declares its own SQLSTATE and SQLCODE
      *> sees the status of its own EXEC SQL statements in them. The
      *> status used to go to the items of the first program that ran
      *> a statement, so a FETCH loop in a called program never saw
      *> 02000: it ran on for ever, the last row left in its host
      *> variables. A Backend nests every program it CALLs in the
      *> exported one, so every Backend routine with its own SQLSTATE
      *> was affected.
      *> Forms exercised:
      *>   S01  SELECT INTO one row: the called program's SQLSTATE
      *>   S02  SELECT INTO no row: 02000 and SQLCODE +100 there
      *>   S03  the reported sequence (SELECT INTO, UPDATE, COMMIT,
      *>        OPEN, FETCH until SQLSTATE NOT = "00000") ends after
      *>        the 200th row with 02000
      *>   S04  the caller's SQLSTATE is left as the caller had it
      *>   S05  the same program CALLed again ends the same way
      *>   S06  a program CALLed by a called program, the same
      *>   S07  the outermost program's own FETCH loop (the control)
      *> Then a bulk phase: a called program reads 10,000 rows to
      *> the end, timed.
      *> Self-checking. One result block at the end.
      *> ============================================================
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  WS-I                PIC 9(6).
       01  WS-ID               PIC 9(6).
       01  WS-N                PIC 9(6) VALUE 0.
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-T0               PIC 9(9).
       01  WS-MS               PIC 9(9).
       01  WS-MS-LOAD          PIC 9(9).
       01  WS-MS-BULK          PIC 9(9).
       01  WS-RATE             PIC 9(9).
       01  WS-NOW              PIC X(21).
       01  G-ROWS              PIC 9(6) GLOBAL.
       01  G-LOOPS             PIC 9(6) GLOBAL.
       01  G-END               PIC X(5) GLOBAL.
       01  G-ONE               PIC X(5) GLOBAL.
       01  G-NONE              PIC X(5) GLOBAL.
       01  G-NONE-CODE         PIC S9(9) GLOBAL.
       01  G-INNER-ROWS        PIC 9(6) GLOBAL.
       01  G-INNER-END         PIC X(5) GLOBAL.
       01  G-AFTER-INNER       PIC X(5) GLOBAL.
       01  G-TABLE             PIC X(20) GLOBAL.
           EXEC SQL DECLARE C-TOP CURSOR FOR
                SELECT modulo FROM prc_perm ORDER BY modulo
           END-EXEC.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS CS END-EXEC
           EXEC SQL DROP TABLE IF EXISTS prc_sessao END-EXEC
           EXEC SQL DROP TABLE IF EXISTS prc_perm END-EXEC
           EXEC SQL DROP TABLE IF EXISTS prc_bulk END-EXEC
           EXEC SQL CREATE TABLE prc_sessao (chave VARCHAR(10),
                usuario VARCHAR(20), expira_em INTEGER) END-EXEC
           EXEC SQL CREATE TABLE prc_perm (modulo INTEGER,
                opcao VARCHAR(20)) END-EXEC
           EXEC SQL CREATE TABLE prc_bulk (id INTEGER) END-EXEC
           EXEC SQL INSERT INTO prc_sessao VALUES ('K1', 'ana', 0)
           END-EXEC
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 200
               EXEC SQL INSERT INTO prc_perm VALUES (:WS-I, 'x')
               END-EXEC
           END-PERFORM
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 10000
               EXEC SQL INSERT INTO prc_bulk VALUES (:WS-I) END-EXEC
           END-PERFORM
           EXEC SQL COMMIT END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-MS-LOAD = WS-MS - WS-T0

           MOVE "ABCDE" TO SQLSTATE

      *> PERMS: the reported sequence, in a called program.
      *> Uses:   prc_sessao holds K1; prc_perm holds 200 rows.
      *> Result: G-ONE, G-NONE, G-NONE-CODE, G-ROWS, G-LOOPS and G-END
      *>         as PERMS's own SQLSTATE and SQLCODE showed them.

           CALL "PERMS"

           IF G-ONE = "00000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S01 SELECT INTO one row: [" G-ONE "]"
           END-IF
           IF G-NONE = "02000" AND G-NONE-CODE = 100
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S02 SELECT INTO no row: [" G-NONE "] "
                       G-NONE-CODE
           END-IF
           IF G-ROWS = 200 AND G-LOOPS = 201 AND G-END = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S03 cursor loop: rows " G-ROWS " loops "
                       G-LOOPS " end [" G-END "]"
           END-IF
           IF SQLSTATE = "ABCDE"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S04 the caller's SQLSTATE became ["
                       SQLSTATE "]"
           END-IF

           MOVE 0 TO G-ROWS
           MOVE 0 TO G-LOOPS
           MOVE SPACES TO G-END

      *> PERMS: called again, a second activation.
      *> Uses:   the same tables.
      *> Result: G-ROWS = 200, G-LOOPS = 201, G-END = "02000".

           CALL "PERMS"

           IF G-ROWS = 200 AND G-LOOPS = 201 AND G-END = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S05 second CALL: rows " G-ROWS " loops "
                       G-LOOPS " end [" G-END "]"
           END-IF

      *> OUTER: CALLs INNER, which reads the cursor itself.
      *> Uses:   prc_perm holds 200 rows.
      *> Result: G-INNER-ROWS, G-INNER-END as INNER saw them;
      *>         G-AFTER-INNER = OUTER's SQLSTATE after the CALL.

           CALL "OUTER"

           IF G-INNER-ROWS = 200 AND G-INNER-END = "02000"
                   AND G-AFTER-INNER = "00000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S06 two levels: rows " G-INNER-ROWS
                       " end [" G-INNER-END "] outer ["
                       G-AFTER-INNER "]"
           END-IF

           MOVE 0 TO WS-N
           EXEC SQL OPEN C-TOP END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000" OR WS-N > 300
               EXEC SQL FETCH C-TOP INTO :WS-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO WS-N
               END-IF
           END-PERFORM
           IF WS-N = 200 AND SQLSTATE = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL S07 outermost loop: rows " WS-N " end ["
                       SQLSTATE "]"
           END-IF
           EXEC SQL CLOSE C-TOP END-EXEC

           MOVE "prc_bulk" TO G-TABLE
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0

      *> BULK: reads every row of prc_bulk through its own cursor.
      *> Uses:   prc_bulk holds 10,000 rows.
      *> Result: G-ROWS = 10000, G-END = "02000".

           CALL "BULK"

           PERFORM NOW-MS
           COMPUTE WS-MS-BULK = WS-MS - WS-T0
           IF G-ROWS = 10000 AND G-END = "02000"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL B01 bulk: rows " G-ROWS " end [" G-END "]"
           END-IF
           IF WS-MS-BULK > 0
               COMPUTE WS-RATE = G-ROWS * 1000 / WS-MS-BULK
           ELSE
               MOVE G-ROWS TO WS-RATE
           END-IF

           EXEC SQL DISCONNECT ALL END-EXEC
           DISPLAY "=== ESQL CALLED-PROGRAM STATUS RESULT ==="
           DISPLAY "forms: SQLSTATE/SQLCODE declared in a called "
                   "program; SELECT INTO (row, no row); UPDATE; "
                   "COMMIT; OPEN; FETCH to 02000; second CALL; two "
                   "levels deep; caller's item untouched"
           DISPLAY "rows: loaded 10200 in " WS-MS-LOAD " ms; bulk "
                   "fetched " G-ROWS " in a called program in "
                   WS-MS-BULK " ms (" WS-RATE " rows/s)"
           DISPLAY "cases: 8 (S01-S07, B01)"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

      *> ----- PERMS: the reported sequence --------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PERMS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  H-USUARIO           PIC X(20).
       01  H-CHAVE             PIC X(10).
       01  H-MODULO            PIC 9(3).
       01  H-OPCAO             PIC X(20).
           EXEC SQL DECLARE C-PERM CURSOR FOR
                SELECT modulo, opcao FROM prc_perm ORDER BY modulo
           END-EXEC.
       PROCEDURE DIVISION.
           MOVE "K1" TO H-CHAVE
           EXEC SQL SELECT usuario INTO :H-USUARIO FROM prc_sessao
                WHERE chave = :H-CHAVE END-EXEC
           MOVE SQLSTATE TO G-ONE
           MOVE "NONE" TO H-CHAVE
           EXEC SQL SELECT usuario INTO :H-USUARIO FROM prc_sessao
                WHERE chave = :H-CHAVE END-EXEC
           MOVE SQLSTATE TO G-NONE
           MOVE SQLCODE TO G-NONE-CODE
           MOVE "K1" TO H-CHAVE
           EXEC SQL UPDATE prc_sessao SET expira_em = 5
                WHERE chave = :H-CHAVE END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL OPEN C-PERM END-EXEC
           MOVE 0 TO G-ROWS
           MOVE 0 TO G-LOOPS
           PERFORM UNTIL SQLSTATE NOT = "00000" OR G-LOOPS > 300
               ADD 1 TO G-LOOPS
               EXEC SQL FETCH C-PERM INTO :H-MODULO, :H-OPCAO
               END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO G-ROWS
               END-IF
           END-PERFORM
           MOVE SQLSTATE TO G-END
           EXEC SQL CLOSE C-PERM END-EXEC
           GOBACK.
       END PROGRAM PERMS.

      *> ----- OUTER and INNER: two levels deep ----------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  H-N                 PIC 9(6).
       PROCEDURE DIVISION.
           EXEC SQL SELECT COUNT(*) INTO :H-N FROM prc_perm END-EXEC

      *> INNER: reads prc_perm to the end through its own cursor.
      *> Uses:   prc_perm holds 200 rows.
      *> Result: G-INNER-ROWS = 200, G-INNER-END = "02000"; OUTER's
      *>         own SQLSTATE keeps the 00000 of its SELECT.

           CALL "INNER"

           MOVE SQLSTATE TO G-AFTER-INNER
           GOBACK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. INNER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  H-MODULO            PIC 9(3).
           EXEC SQL DECLARE C-INNER CURSOR FOR
                SELECT modulo FROM prc_perm ORDER BY modulo
           END-EXEC.
       PROCEDURE DIVISION.
           MOVE 0 TO G-INNER-ROWS
           EXEC SQL OPEN C-INNER END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000" OR G-INNER-ROWS > 300
               EXEC SQL FETCH C-INNER INTO :H-MODULO END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO G-INNER-ROWS
               END-IF
           END-PERFORM
           MOVE SQLSTATE TO G-INNER-END
           EXEC SQL CLOSE C-INNER END-EXEC
           GOBACK.
       END PROGRAM INNER.
       END PROGRAM OUTER.

      *> ----- BULK: 10,000 rows in a called program ----------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BULK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  SQLSTATE            PIC X(5).
       01  H-ID                PIC 9(6).
           EXEC SQL DECLARE C-BULK CURSOR FOR
                SELECT id FROM prc_bulk ORDER BY id
           END-EXEC.
       PROCEDURE DIVISION.
           MOVE 0 TO G-ROWS
           EXEC SQL OPEN C-BULK END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000" OR G-ROWS > 10100
               EXEC SQL FETCH C-BULK INTO :H-ID END-EXEC
               IF SQLSTATE = "00000"
                   ADD 1 TO G-ROWS
               END-IF
           END-PERFORM
           MOVE SQLSTATE TO G-END
           EXEC SQL CLOSE C-BULK END-EXEC
           GOBACK.
       END PROGRAM BULK.

       END PROGRAM ESQL-CALLED-STATUS.

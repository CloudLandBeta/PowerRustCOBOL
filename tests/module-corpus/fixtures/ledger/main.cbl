      *> DESKTOP-WASM-001 T21 corpus fixture "ledger": arithmetic and
      *> rounding, edited pictures, strings, a table with SEARCH, a CALL
      *> into Common Code, an in-memory INDEXED file (write, read by key,
      *> START and READ NEXT, REWRITE, DELETE) and SORT with INPUT and
      *> OUTPUT PROCEDUREs: the statements a console module computes in
      *> itself at M1.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORPUSLEDGER.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT ACCOUNTS ASSIGN TO "accounts"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS ACC-ID
               STORAGE IS MEMORY
               FILE STATUS IS WS-FS.
           SELECT WORK-FILE ASSIGN TO "work".
       DATA DIVISION.
       FILE SECTION.
       FD ACCOUNTS.
       01 ACC-REC.
          05 ACC-ID      PIC X(4).
          05 ACC-NAME    PIC X(12).
          05 ACC-BAL     PIC S9(7)V99.
       SD WORK-FILE.
       01 W-REC.
          05 W-BAL       PIC S9(7)V99.
          05 W-ID        PIC X(4).
       WORKING-STORAGE SECTION.
       01 WS-FS          PIC XX.
       01 WS-SEED.
          05 FILLER      PIC X(25) VALUE "A001ADA LOVELACE00012550".
          05 FILLER      PIC X(25) VALUE "A002ALAN TURING 00009875".
          05 FILLER      PIC X(25) VALUE "A003GRACE HOPPER00031000".
          05 FILLER      PIC X(25) VALUE "A004JOHN BACKUS 00000425".
       01 WS-TABLE REDEFINES WS-SEED.
          05 WS-ROW OCCURS 4 TIMES INDEXED BY WS-IX.
             10 WS-ID    PIC X(4).
             10 WS-NAME  PIC X(12).
             10 WS-CENTS PIC 9(8).
             10 FILLER   PIC X.
       01 WS-I           PIC 9.
       01 WS-TOTAL       PIC S9(9)V99 VALUE 0.
       01 WS-AVG         PIC S9(7)V99.
       01 WS-REM         PIC S9(7)V99.
       01 WS-Q           PIC S9(7).
       01 WS-SHOW        PIC -(7)9.99.
       01 WS-RATE        PIC 9V9999 VALUE 0.0375.
       01 WS-INTEREST    PIC S9(7)V99.
       01 WS-EOF         PIC X.
       01 WS-TEXT        PIC X(40).
       01 WS-FIRST       PIC X(10).
       01 WS-LAST        PIC X(10).
       01 WS-VOWELS      PIC 99.
       01 WS-UP          PIC X(12).
       01 WS-SEEN        PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "corpus ledger fixture"
           OPEN OUTPUT ACCOUNTS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               MOVE WS-ID(WS-I) TO ACC-ID
               MOVE WS-NAME(WS-I) TO ACC-NAME
               COMPUTE ACC-BAL = WS-CENTS(WS-I) / 100
               WRITE ACC-REC
               ADD ACC-BAL TO WS-TOTAL
           END-PERFORM
           CLOSE ACCOUNTS
           MOVE WS-TOTAL TO WS-SHOW
           DISPLAY "total " WS-SHOW " status " WS-FS
           DIVIDE WS-TOTAL BY 3 GIVING WS-Q REMAINDER WS-REM
           MOVE WS-REM TO WS-SHOW
           DISPLAY "thirds " WS-Q " remainder " WS-SHOW
           COMPUTE WS-AVG ROUNDED = WS-TOTAL / 4
           MOVE WS-AVG TO WS-SHOW
           DISPLAY "average " WS-SHOW

           OPEN I-O ACCOUNTS
           MOVE "A003" TO ACC-ID
           READ ACCOUNTS KEY IS ACC-ID
               INVALID KEY DISPLAY "A003 missing"
           END-READ
           CALL "CORPUSINTEREST" USING ACC-BAL WS-RATE WS-INTEREST
           ADD WS-INTEREST TO ACC-BAL
           REWRITE ACC-REC
           MOVE WS-INTEREST TO WS-SHOW
           DISPLAY "interest " WS-SHOW " status " WS-FS
           MOVE "A002" TO ACC-ID
           DELETE ACCOUNTS RECORD
           DISPLAY "delete A002 status " WS-FS
           MOVE "A009" TO ACC-ID
           READ ACCOUNTS KEY IS ACC-ID
               INVALID KEY DISPLAY "A009 missing status " WS-FS
           END-READ
           MOVE "A002" TO ACC-ID
           START ACCOUNTS KEY IS NOT LESS THAN ACC-ID
           MOVE "N" TO WS-EOF
           PERFORM UNTIL WS-EOF = "Y"
               READ ACCOUNTS NEXT RECORD
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       MOVE ACC-BAL TO WS-SHOW
                       DISPLAY "  " ACC-ID " " ACC-NAME " " WS-SHOW
               END-READ
           END-PERFORM
           CLOSE ACCOUNTS

           SET WS-IX TO 1
           SEARCH WS-ROW
               AT END DISPLAY "no Hopper"
               WHEN WS-NAME(WS-IX)(1:5) = "GRACE"
                   SET WS-I TO WS-IX
                   DISPLAY "found " WS-ID(WS-IX) " at row " WS-I
           END-SEARCH

           MOVE "  grace brewster hopper  " TO WS-TEXT
           MOVE FUNCTION UPPER-CASE(FUNCTION TRIM(WS-TEXT)) TO WS-TEXT
           UNSTRING WS-TEXT DELIMITED BY ALL SPACE
               INTO WS-FIRST WS-LAST WS-LAST
           END-UNSTRING
           MOVE 0 TO WS-VOWELS
           INSPECT WS-TEXT TALLYING WS-VOWELS FOR ALL "A" ALL "E"
               ALL "I" ALL "O" ALL "U"
           INSPECT WS-TEXT REPLACING ALL " " BY "-"
           DISPLAY "words " WS-FIRST "/" WS-LAST " vowels " WS-VOWELS
           DISPLAY "dashed " WS-TEXT(1:22)
           MOVE SPACES TO WS-TEXT
           STRING WS-LAST DELIMITED BY SPACE ", " DELIMITED BY SIZE
               WS-FIRST DELIMITED BY SPACE INTO WS-TEXT
           END-STRING
           DISPLAY "filed as [" FUNCTION TRIM(WS-TEXT) "]"

           SORT WORK-FILE ON DESCENDING KEY W-BAL
               INPUT PROCEDURE IS GIVE-ROWS
               OUTPUT PROCEDURE IS TAKE-ROWS
           EVALUATE TRUE
               WHEN WS-TOTAL > 500
                   DISPLAY "band high"
               WHEN WS-TOTAL > 100
                   DISPLAY "band middle"
               WHEN OTHER
                   DISPLAY "band low"
           END-EVALUATE
           MOVE 3 TO RETURN-CODE
           STOP RUN.

       GIVE-ROWS.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               COMPUTE W-BAL = WS-CENTS(WS-I) / 100
               MOVE WS-ID(WS-I) TO W-ID
               RELEASE W-REC
           END-PERFORM.

       TAKE-ROWS.
           MOVE "N" TO WS-EOF
           PERFORM UNTIL WS-EOF = "Y"
               RETURN WORK-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END
                       MOVE W-BAL TO WS-SHOW
                       DISPLAY "sorted " W-ID " " WS-SHOW
               END-RETURN
           END-PERFORM.

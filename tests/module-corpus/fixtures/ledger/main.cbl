      *> DESKTOP-WASM-001 T21 corpus fixture "ledger": arithmetic and
      *> rounding, edited pictures, strings, a table with SEARCH, a CALL
      *> into Common Code and SORT with INPUT and OUTPUT PROCEDUREs: the
      *> statements a console module computes in itself at M1. Until
      *> 2.0.521 it also wrote an INDEXED file declared STORAGE IS MEMORY;
      *> a desktop module refuses one from 2.0.522 (T24), as on the
      *> desktop that file is read from disk, which comes at M3.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORPUSLEDGER.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT WORK-FILE ASSIGN TO "work".
       DATA DIVISION.
       FILE SECTION.
       SD WORK-FILE.
       01 W-REC.
          05 W-BAL       PIC S9(7)V99.
          05 W-ID        PIC X(4).
       WORKING-STORAGE SECTION.
       01 WS-BALANCES.
          05 WS-BAL      PIC S9(7)V99 OCCURS 4 TIMES.
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
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               COMPUTE WS-BAL(WS-I) = WS-CENTS(WS-I) / 100
               ADD WS-BAL(WS-I) TO WS-TOTAL
           END-PERFORM
           MOVE WS-TOTAL TO WS-SHOW
           DISPLAY "total " WS-SHOW
           DIVIDE WS-TOTAL BY 3 GIVING WS-Q REMAINDER WS-REM
           MOVE WS-REM TO WS-SHOW
           DISPLAY "thirds " WS-Q " remainder " WS-SHOW
           COMPUTE WS-AVG ROUNDED = WS-TOTAL / 4
           MOVE WS-AVG TO WS-SHOW
           DISPLAY "average " WS-SHOW

           *> CORPUSINTEREST: works out the interest on a balance at a rate,
           *> rounded to the cent.
           *> Uses:   WS-BAL(3) (310.00, account A003), WS-RATE (0.0375).
           *> Result: WS-INTEREST becomes 11.63; WS-BAL(3) and WS-RATE are unchanged.
           CALL "CORPUSINTEREST" USING WS-BAL(3) WS-RATE WS-INTEREST
           ADD WS-INTEREST TO WS-BAL(3)
           MOVE WS-INTEREST TO WS-SHOW
           DISPLAY "interest " WS-SHOW
           PERFORM VARYING WS-I FROM 3 BY 1 UNTIL WS-I > 4
               MOVE WS-BAL(WS-I) TO WS-SHOW
               DISPLAY "  " WS-ID(WS-I) " " WS-NAME(WS-I) " " WS-SHOW
           END-PERFORM

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

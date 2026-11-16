      *> DESKTOP-WASM-001 T24 console-parity fixture "statements": the
      *> statements a desktop module computes in itself at M1, each with
      *> the phrases that change its result, run on the Cargo path and on
      *> the WebAssembly path; the two outputs must be the same bytes.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYSTATEMENTS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-A           PIC S9(5)V99 VALUE 1234.56.
       01 WS-B           PIC S9(5)V99 VALUE -78.9.
       01 WS-C           PIC S9(7)V99.
       01 WS-Q           PIC S9(5).
       01 WS-R           PIC S9(5)V99.
       01 WS-SMALL       PIC 99.
       01 WS-SHOW        PIC -(6)9.99.
       01 WS-MONEY       PIC $$$,$$9.99-.
       01 WS-STARS       PIC ***,**9.99.
       01 WS-ZEROS       PIC ZZZ9.
       01 WS-BWZ         PIC ZZZ9 BLANK WHEN ZERO.
       01 WS-SIGNED      PIC +9(4).
       01 WS-RIGHT       PIC X(10) JUSTIFIED RIGHT.
       01 WS-TEXT        PIC X(40).
       01 WS-PART        PIC X(10).
       01 WS-PART2       PIC X(10).
       01 WS-PART3       PIC X(10).
       01 WS-COUNT1      PIC 99.
       01 WS-COUNT2      PIC 99.
       01 WS-PTR         PIC 99.
       01 WS-FIELDS      PIC 99.
       01 WS-N           PIC 9(4).
       01 WS-I           PIC 99.
       01 WS-J           PIC 99.
       01 WS-K           PIC 9.
       01 WS-LOOPS       PIC 999 VALUE 0.
       01 WS-FLAG        PIC X VALUE "N".
          88 WS-DONE     VALUE "Y".
          88 WS-NOT-DONE VALUE "N".
       01 WS-GRADE       PIC 999.
          88 WS-PASS     VALUE 50 THRU 100.
       01 WS-SRC.
          05 AMOUNT      PIC 9(4) VALUE 120.
          05 TAX         PIC 9(4) VALUE 30.
          05 NAME        PIC X(8) VALUE "SOURCE".
       01 WS-DST.
          05 NAME        PIC X(8) VALUE "TARGET".
          05 AMOUNT      PIC 9(4) VALUE 1000.
          05 TAX         PIC 9(4) VALUE 5.
       01 WS-REC.
          05 WS-REC-ID   PIC 9(3) VALUE 7.
          05 WS-REC-NM   PIC X(6) VALUE "ABCDEF".
          05 WS-REC-AM   PIC S9(3)V9 VALUE -1.5.
       01 WS-TABLE.
          05 WS-ENTRY OCCURS 6 TIMES
                ASCENDING KEY IS WS-KEY INDEXED BY WS-X.
             10 WS-KEY   PIC 99.
             10 WS-VAL.
                15 WS-VAL-P PIC X.
                15 WS-VAL-N PIC 99.
                15 FILLER   PIC XX.
       01 WS-DIGITS      PIC X(6) VALUE "123ABC".
       01 WS-ROUTE       PIC 9 VALUE 2.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "parity statements fixture"
      *> ADD, SUBTRACT, MULTIPLY, DIVIDE, with GIVING, ROUNDED,
      *> REMAINDER and ON SIZE ERROR.
           ADD WS-A WS-B GIVING WS-C
           MOVE WS-C TO WS-SHOW
           DISPLAY "add " WS-SHOW
           SUBTRACT WS-B FROM WS-A GIVING WS-C ROUNDED
           MOVE WS-C TO WS-SHOW
           DISPLAY "subtract " WS-SHOW
           MULTIPLY WS-A BY WS-B GIVING WS-C ROUNDED
           MOVE WS-C TO WS-SHOW
           DISPLAY "multiply " WS-SHOW
           DIVIDE WS-A BY 7 GIVING WS-Q REMAINDER WS-R
           MOVE WS-R TO WS-SHOW
           DISPLAY "divide " WS-Q " remainder " WS-SHOW
           MOVE 95 TO WS-SMALL
           ADD 10 TO WS-SMALL
               ON SIZE ERROR DISPLAY "size error, kept " WS-SMALL
               NOT ON SIZE ERROR DISPLAY "no size error"
           END-ADD
           DIVIDE 0 INTO WS-A
               ON SIZE ERROR DISPLAY "divide by zero caught"
           END-DIVIDE
           COMPUTE WS-C ROUNDED = (WS-A + 2 ** 3) / 3 - WS-B * 2
           MOVE WS-C TO WS-SHOW
           DISPLAY "compute " WS-SHOW
           COMPUTE WS-C = -WS-A
           MOVE WS-C TO WS-MONEY
           DISPLAY "money [" WS-MONEY "]"
           MOVE 1234.5 TO WS-STARS
           DISPLAY "stars [" WS-STARS "]"
           MOVE 7 TO WS-ZEROS
           MOVE 0 TO WS-BWZ
           MOVE -42 TO WS-SIGNED
           DISPLAY "zeros [" WS-ZEROS "] bwz [" WS-BWZ "] signed ["
               WS-SIGNED "]"
           MOVE "RIGHT" TO WS-RIGHT
           DISPLAY "justified [" WS-RIGHT "]"
      *> The CORRESPONDING forms.
           ADD CORRESPONDING WS-SRC TO WS-DST
           DISPLAY "add corr " AMOUNT OF WS-DST " " TAX OF WS-DST " "
               NAME OF WS-DST
           SUBTRACT CORRESPONDING WS-SRC FROM WS-DST
           DISPLAY "subtract corr " AMOUNT OF WS-DST " " TAX OF WS-DST
           MOVE CORRESPONDING WS-SRC TO WS-DST
           DISPLAY "move corr " AMOUNT OF WS-DST " " NAME OF WS-DST
      *> INITIALIZE, plain and with REPLACING.
           INITIALIZE WS-REC
           DISPLAY "initialize [" WS-REC-ID "][" WS-REC-NM "]["
               WS-REC-AM "]"
           INITIALIZE WS-REC REPLACING NUMERIC BY 9
               ALPHANUMERIC BY "Z"
           DISPLAY "replacing [" WS-REC-ID "][" WS-REC-NM "]"
      *> STRING, UNSTRING and INSPECT.
           MOVE SPACES TO WS-TEXT
           MOVE 3 TO WS-PTR
           STRING "ab" DELIMITED BY SIZE
                  "cd,ef" DELIMITED BY ","
                  "-gh" DELIMITED BY SIZE
               INTO WS-TEXT WITH POINTER WS-PTR
           END-STRING
           DISPLAY "string [" WS-TEXT(1:12) "] pointer " WS-PTR
           MOVE SPACES TO WS-PART
           STRING "0123456789" "ABC" DELIMITED BY SIZE INTO WS-PART
               ON OVERFLOW DISPLAY "string overflow [" WS-PART "]"
           END-STRING
           MOVE "alpha;beta,,gamma" TO WS-TEXT
           MOVE 0 TO WS-FIELDS
           UNSTRING WS-TEXT DELIMITED BY ";" OR ","
               INTO WS-PART COUNT IN WS-COUNT1
                    WS-PART2 COUNT IN WS-COUNT2
                    WS-PART3
               TALLYING IN WS-FIELDS
           END-UNSTRING
           DISPLAY "unstring [" WS-PART "][" WS-PART2 "][" WS-PART3
               "] counts " WS-COUNT1 " " WS-COUNT2 " fields " WS-FIELDS
           MOVE "MISSISSIPPI RIVER" TO WS-TEXT
           MOVE 0 TO WS-COUNT1 WS-COUNT2
           INSPECT WS-TEXT TALLYING WS-COUNT1 FOR ALL "SS"
                   WS-COUNT2 FOR LEADING "M"
           DISPLAY "tallying " WS-COUNT1 " " WS-COUNT2
           INSPECT WS-TEXT REPLACING FIRST "I" BY "!"
                   ALL "S" BY "s" AFTER INITIAL "P"
           DISPLAY "replacing [" WS-TEXT(1:17) "]"
           INSPECT WS-TEXT CONVERTING "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                   TO "abcdefghijklmnopqrstuvwxyz" BEFORE INITIAL " "
           DISPLAY "converting [" WS-TEXT(1:17) "]"
      *> IF, class and sign conditions, condition names, NEXT SENTENCE.
           IF WS-DIGITS(1:3) IS NUMERIC AND WS-DIGITS(4:3) IS ALPHABETIC
               DISPLAY "class conditions hold"
           END-IF
           IF WS-B IS NEGATIVE AND NOT WS-A IS ZERO
               DISPLAY "sign conditions hold"
           END-IF
           MOVE 77 TO WS-GRADE
           IF WS-PASS DISPLAY "grade passes" ELSE DISPLAY "grade fails".
           IF WS-NOT-DONE
               NEXT SENTENCE
           ELSE
               DISPLAY "not reached"
           END-IF.
           SET WS-DONE TO TRUE
           DISPLAY "flag " WS-FLAG
      *> EVALUATE with ALSO, THRU, TRUE and OTHER.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               EVALUATE WS-I ALSO TRUE
                   WHEN 1 ALSO WS-DONE
                       DISPLAY "evaluate " WS-I " first"
                   WHEN 2 THRU 3 ALSO ANY
                       DISPLAY "evaluate " WS-I " middle"
                   WHEN OTHER
                       DISPLAY "evaluate " WS-I " other"
               END-EVALUATE
           END-PERFORM
      *> PERFORM: VARYING with AFTER, TIMES, TEST AFTER, THRU.
           MOVE 0 TO WS-LOOPS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
                   AFTER WS-J FROM 1 BY 2 UNTIL WS-J > 5
               ADD 1 TO WS-LOOPS
           END-PERFORM
           DISPLAY "nested loops " WS-LOOPS
           PERFORM 3 TIMES
               ADD 10 TO WS-LOOPS
           END-PERFORM
           MOVE 9 TO WS-K
           PERFORM WITH TEST AFTER UNTIL WS-K > 5
               ADD 1 TO WS-LOOPS
           END-PERFORM
           DISPLAY "times and test after " WS-LOOPS
           PERFORM STEP-ONE THRU STEP-TWO
      *> Tables: SET, SEARCH and SEARCH ALL.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 6
               COMPUTE WS-KEY(WS-I) = WS-I * 5
               MOVE SPACES TO WS-VAL(WS-I)
               MOVE "V" TO WS-VAL-P(WS-I)
               MOVE WS-I TO WS-VAL-N(WS-I)
           END-PERFORM
           SET WS-X TO 2
           SET WS-X UP BY 2
           DISPLAY "index at " WS-KEY(WS-X)
           SET WS-X TO 1
           SEARCH WS-ENTRY VARYING WS-X
               AT END DISPLAY "search: none"
               WHEN WS-KEY(WS-X) > 12
                   DISPLAY "search: " WS-VAL(WS-X)
           END-SEARCH
           SEARCH ALL WS-ENTRY
               AT END DISPLAY "search all: none"
               WHEN WS-KEY(WS-X) = 25
                   DISPLAY "search all: " WS-VAL(WS-X)
           END-SEARCH
      *> Intrinsic functions the module computes.
           DISPLAY "mod " FUNCTION MOD(-11 4) " rem " FUNCTION REM(-11 4)
           DISPLAY "integer " FUNCTION INTEGER(-3.5)
               " integer-part " FUNCTION INTEGER-PART(-3.5)
           DISPLAY "max " FUNCTION MAX(3 9 2) " min " FUNCTION MIN(3 9 2)
           MOVE FUNCTION NUMVAL(" -12.50 ") TO WS-C
           MOVE WS-C TO WS-SHOW
           DISPLAY "numval " WS-SHOW
           DISPLAY "reverse [" FUNCTION REVERSE("abc  ") "] lower ["
               FUNCTION LOWER-CASE("MiXeD") "]"
           DISPLAY "length " FUNCTION LENGTH(WS-TEXT)
               " ord " FUNCTION ORD("A") " char [" FUNCTION CHAR(66) "]"
           DISPLAY "dates " FUNCTION INTEGER-OF-DATE(20260102) " "
               FUNCTION DATE-OF-INTEGER(150000)
      *> GO TO DEPENDING ON and ALTER.
           GO TO ROUTE-ONE ROUTE-TWO DEPENDING ON WS-ROUTE
           DISPLAY "route: fell through".
       ROUTE-ONE.
           DISPLAY "route one"
           GO TO ROUTE-END.
       ROUTE-TWO.
           DISPLAY "route two".
           ALTER SWITCH-PARA TO PROCEED TO ALTERED-PARA.
       ROUTE-END.
           PERFORM SWITCH-PARA
           CONTINUE
           DISPLAY "done"
           STOP RUN.

       STEP-ONE.
           DISPLAY "step one".
       STEP-TWO.
           DISPLAY "step two"
           EXIT.

       SWITCH-PARA.
           GO TO PLAIN-PARA.
       PLAIN-PARA.
           DISPLAY "alter: not taken".
       ALTERED-PARA.
           DISPLAY "alter: taken".

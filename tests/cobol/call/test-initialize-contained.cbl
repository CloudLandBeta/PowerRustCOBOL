       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-INITIALIZE-CONTAINED
      *> INITIALIZE on items a CONTAINED program declares in its own
      *> WORKING-STORAGE. Every form event handler is such a program,
      *> and its items live under an activation key, not under the
      *> bare name. INITIALIZE used to treat them as control objects
      *> and left the item as it was.
      *> Each case is a contained program that INITIALIZEs, then
      *> reports what the item holds into the GLOBAL item G-GOT; the
      *> outermost program compares it with the expected image.
      *> Self-checking: a case table and a throughput line close it.
      *> ============================================================
       PROGRAM-ID. TEST-INITIALIZE-CONTAINED.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  G-GOT                   PIC X(60) GLOBAL.
       01  G-GOT2                  PIC X(60) GLOBAL.
       01  G-WANT                  PIC X(60).
       01  G-NAME                  PIC X(40).
      *> The outermost program declares SHARED too: a contained
      *> program's INITIALIZE SHARED must never reach this one.
       01  SHARED                  PIC X(7) VALUE "outer-1".
       01  OUT-X                   PIC X(5) VALUE "hello".
       01  OUT-GRP.
           05 OUT-GA               PIC X(3) VALUE "abc".
           05 OUT-GN               PIC 99   VALUE 42.
       01  PERF-START              PIC 9(8) VALUE 0.
       01  PERF-START-PARTS REDEFINES PERF-START.
           05 PS-HH                PIC 99.
           05 PS-MM                PIC 99.
           05 PS-SS                PIC 99.
           05 PS-HS                PIC 99.
       01  PERF-END                PIC 9(8) VALUE 0.
       01  PERF-END-PARTS REDEFINES PERF-END.
           05 PE-HH                PIC 99.
           05 PE-MM                PIC 99.
           05 PE-SS                PIC 99.
           05 PE-HS                PIC 99.
       01  PERF-START-CS           PIC 9(9) VALUE 0.
       01  PERF-END-CS             PIC 9(9) VALUE 0.
       01  PERF-ELAPSED-CS         PIC 9(9) VALUE 0.
       01  PERF-LEAVES             PIC 9(9) VALUE 0.
       01  PERF-LEAVES-EDIT        PIC ZZZ,ZZZ,ZZ9.
       01  PERF-PER-SEC            PIC 9(9) VALUE 0.
       01  PERF-PER-SEC-EDIT       PIC ZZZ,ZZZ,ZZ9.
       01  PERF-ELAPSED-EDIT       PIC ZZZ,ZZ9.99.
       01  PERF-ELAPSED-SECS       PIC 9(7)V99 VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-INITIALIZE-CONTAINED".
           DISPLAY "Construct: INITIALIZE of items declared in a".
           DISPLAY "  contained program's own WORKING-STORAGE".
           DISPLAY "--------------------------------------------".
           PERFORM C01-ALPHANUMERIC.
           PERFORM C02-NUMERIC.
           PERFORM C03-NUMERIC-DECIMALS.
           PERFORM C04-TWO-ITEMS.
           PERFORM C05-GROUP-WITH-FILLER.
           PERFORM C06-TABLE-ALL-OCCURRENCES.
           PERFORM C07-TABLE-OF-GROUPS.
           PERFORM C08-ONE-OCCURRENCE.
           PERFORM C09-SAME-NAME-IN-THREE-PROGRAMS.
           PERFORM C10-REPLACING.
           PERFORM C11-PARAMETER-OF-OUTERMOST-ITEM.
           PERFORM C12-PARAMETER-OF-CONTAINED-ITEM.
           PERFORM C13-EDITED-ITEMS.
           PERFORM C14-BULK.
           PERFORM C15-BULK-UNTOUCHED.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 15".
           DISPLAY "CHECKS RUN      : " TESTS-RUN.
           DISPLAY "CHECKS PASSED   : " TESTS-PASSED.
           DISPLAY "CHECKS FAILED   : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".
           STOP RUN.

       C01-ALPHANUMERIC.
           MOVE SPACES TO G-GOT.
           CALL "C01".
           MOVE "[          ]" TO G-WANT.
           MOVE "C01 alphanumeric PIC X(10)" TO G-NAME.
           PERFORM CHECK-IT.
       C02-NUMERIC.
           MOVE SPACES TO G-GOT.
           CALL "C02".
           MOVE "[000]" TO G-WANT.
           MOVE "C02 numeric PIC 9(3)" TO G-NAME.
           PERFORM CHECK-IT.
       C03-NUMERIC-DECIMALS.
           MOVE SPACES TO G-GOT.
           CALL "C03".
           MOVE "[00000][Z]" TO G-WANT.
           MOVE "C03 9(3)V99 and S9(3)V99" TO G-NAME.
           PERFORM CHECK-IT.
       C04-TWO-ITEMS.
           MOVE SPACES TO G-GOT.
           CALL "C04".
           MOVE "[          ][000][kep]" TO G-WANT.
           MOVE "C04 two operands, third untouched" TO G-NAME.
           PERFORM CHECK-IT.
       C05-GROUP-WITH-FILLER.
           MOVE SPACES TO G-GOT.
           CALL "C05".
           MOVE "[   00-00]" TO G-WANT.
           MOVE "C05 group, its FILLER kept" TO G-NAME.
           PERFORM CHECK-IT.
       C06-TABLE-ALL-OCCURRENCES.
           MOVE SPACES TO G-GOT.
           CALL "C06".
           MOVE "[      ][000000]" TO G-WANT.
           MOVE "C06 tables, every occurrence" TO G-NAME.
           PERFORM CHECK-IT.
       C07-TABLE-OF-GROUPS.
           MOVE SPACES TO G-GOT.
           CALL "C07".
           MOVE "[0 0 ]" TO G-WANT.
           MOVE "C07 table of groups" TO G-NAME.
           PERFORM CHECK-IT.
       C08-ONE-OCCURRENCE.
           MOVE SPACES TO G-GOT.
           CALL "C08".
           MOVE "[XX  ZZ]" TO G-WANT.
           MOVE "C08 T(2) only, 1 and 3 untouched" TO G-NAME.
           PERFORM CHECK-IT.
       C09-SAME-NAME-IN-THREE-PROGRAMS.
           MOVE SPACES TO G-GOT.
           MOVE SPACES TO G-GOT2.
           CALL "C09A".
           MOVE "[inner-a][twin-a][inner-a][twin-a][       ][      ]"
               TO G-WANT.
           MOVE "C09 A's items: B's reset leaves them" TO G-NAME.
           PERFORM CHECK-IT.
           MOVE G-GOT2 TO G-GOT.
           MOVE "[inner-b][twin-b][       ][      ]" TO G-WANT.
           MOVE "C09 B's items: reset in B" TO G-NAME.
           PERFORM CHECK-IT.
           MOVE SPACES TO G-GOT.
           STRING "[" SHARED "]" DELIMITED BY SIZE INTO G-GOT.
           MOVE "[outer-1]" TO G-WANT.
           MOVE "C09 SHARED in the outermost untouched" TO G-NAME.
           PERFORM CHECK-IT.
       C10-REPLACING.
           MOVE SPACES TO G-GOT.
           CALL "C10".
           MOVE "[007ABC][000ZZ ]" TO G-WANT.
           MOVE "C10 REPLACING NUMERIC / ALPHANUMERIC" TO G-NAME.
           PERFORM CHECK-IT.
       C11-PARAMETER-OF-OUTERMOST-ITEM.
           CALL "C11" USING OUT-X OUT-GRP.
           MOVE SPACES TO G-GOT.
           STRING "[" OUT-X "][" OUT-GRP "]" DELIMITED BY SIZE
               INTO G-GOT.
           MOVE "[     ][   00]" TO G-WANT.
           MOVE "C11 LINKAGE bound to outermost item" TO G-NAME.
           PERFORM CHECK-IT.
       C12-PARAMETER-OF-CONTAINED-ITEM.
           MOVE SPACES TO G-GOT.
           CALL "C12A".
           MOVE "[     ][   00]" TO G-WANT.
           MOVE "C12 LINKAGE bound to contained item"
               TO G-NAME.
           PERFORM CHECK-IT.
       C13-EDITED-ITEMS.
           MOVE SPACES TO G-GOT.
           CALL "C13".
           MOVE "[   0.00][     ]" TO G-WANT.
           MOVE "C13 numeric- and alphanumeric-edited" TO G-NAME.
           PERFORM CHECK-IT.
       C14-BULK.
           ACCEPT PERF-START FROM TIME.
           MOVE SPACES TO G-GOT.
           CALL "C14".
           ACCEPT PERF-END FROM TIME.
           MOVE "[0000][      ][0000][      ][done]" TO G-WANT.
           MOVE "C14 400 groups, written then reset x50" TO G-NAME.
           PERFORM CHECK-IT.
      *> 50 passes x 400 occurrences x 2 leaves, each written first.
           COMPUTE PERF-LEAVES = 50 * 400 * 2.
           PERFORM TIME-IT.
           DISPLAY "  bulk C14: leaves written and reset: "
               PERF-LEAVES-EDIT.
           DISPLAY "  bulk C14: elapsed seconds         : "
               PERF-ELAPSED-EDIT.
           DISPLAY "  bulk C14: leaves/second           : "
               PERF-PER-SEC-EDIT.
       C15-BULK-UNTOUCHED.
           ACCEPT PERF-START FROM TIME.
           MOVE SPACES TO G-GOT.
           CALL "C15".
           ACCEPT PERF-END FROM TIME.
           MOVE "[0000][0000][0000][0000]" TO G-WANT.
           MOVE "C15 100,000 occurrences, untouched x50" TO G-NAME.
           PERFORM CHECK-IT.
      *> 50 passes over a table of 100,000 occurrences, one of them
      *> written: the others follow the table's template, so the
      *> work does not grow with the table.
           COMPUTE PERF-LEAVES = 50 * 100000.
           PERFORM TIME-IT.
           DISPLAY "  bulk C15: occurrences covered     : "
               PERF-LEAVES-EDIT.
           DISPLAY "  bulk C15: elapsed seconds         : "
               PERF-ELAPSED-EDIT " (clock resolution 0.01 s)".
       TIME-IT.
           COMPUTE PERF-START-CS = PS-HH * 360000 + PS-MM * 6000
               + PS-SS * 100 + PS-HS.
           COMPUTE PERF-END-CS = PE-HH * 360000 + PE-MM * 6000
               + PE-SS * 100 + PE-HS.
           IF PERF-END-CS < PERF-START-CS
               ADD 8640000 TO PERF-END-CS
           END-IF.
           COMPUTE PERF-ELAPSED-CS = PERF-END-CS - PERF-START-CS.
           IF PERF-ELAPSED-CS = ZERO
               MOVE 1 TO PERF-ELAPSED-CS
           END-IF.
           COMPUTE PERF-PER-SEC = PERF-LEAVES * 100 / PERF-ELAPSED-CS.
           COMPUTE PERF-ELAPSED-SECS = PERF-ELAPSED-CS / 100.
           MOVE PERF-LEAVES TO PERF-LEAVES-EDIT.
           MOVE PERF-PER-SEC TO PERF-PER-SEC-EDIT.
           MOVE PERF-ELAPSED-SECS TO PERF-ELAPSED-EDIT.

       CHECK-IT.
           ADD 1 TO TESTS-RUN.
           IF G-GOT = G-WANT
               ADD 1 TO TESTS-PASSED
               DISPLAY "  PASS " G-NAME
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "  FAIL " G-NAME
               DISPLAY "       got  [" G-GOT "]"
               DISPLAY "       want [" G-WANT "]"
           END-IF.

      *> ----- C01: alphanumeric ------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C01.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-X PIC X(10) VALUE "abc".
       PROCEDURE DIVISION.
           INITIALIZE L-X
           STRING "[" L-X "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C01.

      *> ----- C02: numeric -----------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C02.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-N PIC 9(3) VALUE 7.
       PROCEDURE DIVISION.
           INITIALIZE L-N
           STRING "[" L-N "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C02.

      *> ----- C03: numeric with decimals, and a signed one ---------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C03.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-D PIC 9(3)V99 VALUE 12.34.
       01 L-S PIC S9(3)V99 VALUE -56.78.
       01 L-FLAG PIC X VALUE "?".
       PROCEDURE DIVISION.
           INITIALIZE L-D L-S
           IF L-S = ZERO
               MOVE "Z" TO L-FLAG
           END-IF
           STRING "[" L-D "][" L-FLAG "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C03.

      *> ----- C04: two operands in one statement; a third is left --
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C04.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 LK-FILE PIC X(10) VALUE "abc".
       01 LK-N PIC 9(3) VALUE 7.
       01 LK-KEEP PIC X(3) VALUE "kep".
       PROCEDURE DIVISION.
           INITIALIZE LK-FILE LK-N
           STRING "[" LK-FILE "][" LK-N "][" LK-KEEP "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C04.

      *> ----- C05: a group; its FILLER keeps its value -------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C05.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-G.
          05 L-GA PIC X(3) VALUE "xyz".
          05 L-GN PIC 99 VALUE 55.
          05 FILLER PIC X VALUE "-".
          05 L-GD PIC 9V9 VALUE 5.5.
       PROCEDURE DIVISION.
           INITIALIZE L-G
           STRING "[" L-G "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C05.

      *> ----- C06: tables, occurrences written before the verb -----
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C06.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-T.
          05 L-TE PIC X(2) OCCURS 3 VALUE "ab".
       01 L-M.
          05 L-MN PIC 99 OCCURS 3 VALUE 11.
       PROCEDURE DIVISION.
           MOVE "ZZ" TO L-TE (2)
           MOVE 99 TO L-MN (3)
           INITIALIZE L-T L-M
           STRING "[" L-TE (1) L-TE (2) L-TE (3) "]["
               L-MN (1) L-MN (2) L-MN (3) "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C06.

      *> ----- C07: a table whose occurrences are groups ------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C07.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-U.
          05 L-UE OCCURS 2.
             10 L-UC PIC 9 VALUE 4.
             10 L-US PIC X VALUE "q".
       PROCEDURE DIVISION.
           MOVE 9 TO L-UC (2)
           MOVE "w" TO L-US (1)
           INITIALIZE L-U
           STRING "[" L-U "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C07.

      *> ----- C08: one occurrence only -----------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C08.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-T.
          05 L-TE PIC X(2) OCCURS 3 VALUE "ab".
       PROCEDURE DIVISION.
           MOVE "XX" TO L-TE (1)
           MOVE "YY" TO L-TE (2)
           MOVE "ZZ" TO L-TE (3)
           INITIALIZE L-TE (2)
           STRING "[" L-TE (1) L-TE (2) L-TE (3) "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C08.

      *> ----- C09: SHARED is declared here, in C09B and outside; ---
      *> TWIN only here and in C09B (the flat lookup missed it) ---
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C09A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHARED PIC X(7) VALUE "inner-a".
       01 TWIN PIC X(6) VALUE "twin-a".
       01 L-PTR PIC 99 VALUE 1.
       PROCEDURE DIVISION.
           STRING "[" SHARED "][" TWIN "]" DELIMITED BY SIZE
               INTO G-GOT WITH POINTER L-PTR
           CALL "C09B"
           STRING "[" SHARED "][" TWIN "]" DELIMITED BY SIZE
               INTO G-GOT WITH POINTER L-PTR
           INITIALIZE SHARED TWIN
           STRING "[" SHARED "][" TWIN "]" DELIMITED BY SIZE
               INTO G-GOT WITH POINTER L-PTR
           GOBACK.
       END PROGRAM C09A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. C09B.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SHARED PIC X(7) VALUE "inner-b".
       01 TWIN PIC X(6) VALUE "twin-b".
       01 L-PTR PIC 99 VALUE 1.
       PROCEDURE DIVISION.
           STRING "[" SHARED "][" TWIN "]" DELIMITED BY SIZE
               INTO G-GOT2 WITH POINTER L-PTR
           INITIALIZE SHARED TWIN
           STRING "[" SHARED "][" TWIN "]" DELIMITED BY SIZE
               INTO G-GOT2 WITH POINTER L-PTR
           GOBACK.
       END PROGRAM C09B.

      *> ----- C10: REPLACING touches only the named categories -----
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C10.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-R.
          05 L-RN PIC 9(3) VALUE 123.
          05 L-RX PIC X(3) VALUE "ABC".
       01 L-Q.
          05 L-QN PIC 9(3) VALUE 123.
          05 L-QX PIC X(3) VALUE "ABC".
       PROCEDURE DIVISION.
           INITIALIZE L-R REPLACING NUMERIC DATA BY 7
           INITIALIZE L-Q REPLACING NUMERIC DATA BY 0
                                    ALPHANUMERIC DATA BY "ZZ"
           STRING "[" L-R "][" L-Q "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C10.

      *> ----- C11: a LINKAGE item bound to the outermost's storage -
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C11.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-X PIC X(5).
       01 LK-GRP.
          05 LK-GA PIC X(3).
          05 LK-GN PIC 99.
       PROCEDURE DIVISION USING LK-X LK-GRP.
           INITIALIZE LK-X LK-GRP
           GOBACK.
       END PROGRAM C11.

      *> ----- C12: a LINKAGE item bound to a CONTAINED program's ---
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C12A.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-X PIC X(5) VALUE "hello".
       01 L-GRP.
          05 L-GA PIC X(3) VALUE "abc".
          05 L-GN PIC 99 VALUE 42.
       PROCEDURE DIVISION.
           CALL "C12B" USING L-X L-GRP
           STRING "[" L-X "][" L-GRP "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C12A.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. C12B.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-X PIC X(5).
       01 LK-GRP.
          05 LK-GA PIC X(3).
          05 LK-GN PIC 99.
       PROCEDURE DIVISION USING LK-X LK-GRP.
           INITIALIZE LK-X
           INITIALIZE LK-GRP
           GOBACK.
       END PROGRAM C12B.

      *> ----- C13: edited items ------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C13.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-NE PIC Z(3)9.99.
       01 L-AE PIC XXBXX.
       PROCEDURE DIVISION.
           MOVE 12.5 TO L-NE
           MOVE "ab cd" TO L-AE
           INITIALIZE L-NE L-AE
           STRING "[" L-NE "][" L-AE "]" DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C13.

      *> ----- C14: 400 groups, every occurrence written, reset x50 -
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C14.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-B.
          05 L-BE OCCURS 400.
             10 L-BN PIC 9(4) VALUE 1234.
             10 L-BX PIC X(6) VALUE "abcdef".
             10 FILLER PIC X VALUE "|".
       01 L-I PIC 999.
       PROCEDURE DIVISION.
           PERFORM 50 TIMES
               PERFORM VARYING L-I FROM 1 BY 1 UNTIL L-I > 400
                   MOVE 7 TO L-BN (L-I)
                   MOVE "zz" TO L-BX (L-I)
               END-PERFORM
               INITIALIZE L-B
           END-PERFORM
           STRING "[" L-BN (1) "][" L-BX (1) "][" L-BN (400) "]["
               L-BX (400) "][done]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C14.

      *> ----- C15: a huge table, one occurrence written, reset x50 -
       IDENTIFICATION DIVISION.
       PROGRAM-ID. C15.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 L-HUGE.
          05 L-HE PIC 9(4) OCCURS 100000 VALUE 55.
       PROCEDURE DIVISION.
           MOVE 9 TO L-HE (100000)
           PERFORM 50 TIMES
               INITIALIZE L-HUGE
           END-PERFORM
           STRING "[" L-HE (1) "][" L-HE (50000) "][" L-HE (99999) "]["
               L-HE (100000) "]"
               DELIMITED BY SIZE INTO G-GOT
           GOBACK.
       END PROGRAM C15.

       END PROGRAM TEST-INITIALIZE-CONTAINED.

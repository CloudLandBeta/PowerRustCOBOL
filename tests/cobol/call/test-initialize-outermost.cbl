       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-INITIALIZE-OUTERMOST
      *> INITIALIZE on items the OUTERMOST program declares. The
      *> companion of TEST-INITIALIZE-CONTAINED: both programs now
      *> take the same walk, read off the environment's symbol table.
      *> It used to walk the DATA DIVISION by bare name, so
      *>  - a table kept every occurrence written before the verb,
      *>  - a name declared in two groups was reset under a key that
      *>    holds nothing, and
      *>  - a REDEFINES item reset after its target overwrote it.
      *> Self-checking: a case table and a throughput line close it.
      *> ============================================================
       PROGRAM-ID. TEST-INITIALIZE-OUTERMOST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-GOT                  PIC X(60).
       01  WS-WANT                 PIC X(60).
       01  WS-NAME                 PIC X(40).
       01  WS-I                    PIC 9(3).
       01  WS-PASS                 PIC 99.

       01  E-X                     PIC X(10) VALUE "abc".
       01  E-N                     PIC 9(3) VALUE 7.
       01  E-D                     PIC 9(3)V99 VALUE 12.34.
       01  E-S                     PIC S9(3)V99 VALUE -56.78.
       01  E-FLAG                  PIC X VALUE "?".
       01  E-KEEP                  PIC X(3) VALUE "kep".

       01  G-GRP.
           05 G-A                  PIC X(3) VALUE "xyz".
           05 G-N                  PIC 99 VALUE 55.
           05 FILLER               PIC X VALUE "-".
           05 G-D                  PIC 9V9 VALUE 5.5.

       01  T-TEXT.
           05 T-TE                 PIC X(2) OCCURS 3 VALUE "ab".
       01  T-NUM.
           05 T-TN                 PIC 99 OCCURS 3 VALUE 11.

       01  U-TAB.
           05 U-E OCCURS 2.
              10 U-C               PIC 9 VALUE 4.
              10 U-S               PIC X VALUE "q".

       01  P-ONE.
           05 P-E                  PIC X(2) OCCURS 3 VALUE "ab".

       01  D-G1.
           05 D-F                  PIC X(3) VALUE "one".
       01  D-G2.
           05 D-F                  PIC X(3) VALUE "two".
       01  D-G3.
           05 D-H                  PIC X(3) VALUE "aaa".
       01  D-G4.
           05 D-H                  PIC X(3) VALUE "bbb".

       01  R-GRP.
           05 R-N                  PIC 99 VALUE 42.
           05 R-R REDEFINES R-N    PIC XX.

       01  L-GRP.
           05 L-FLAG               PIC X VALUE "Y".
              88 L-IS-Y            VALUE "Y".
           05 L-AFTER              PIC X VALUE "z".
       01  L-SAW                   PIC X VALUE "?".

       01  X-GRP.
           05 X-E OCCURS 2.
              10 X-N               PIC 9(3) VALUE 123.
              10 X-T               PIC X(3) VALUE "ABC".

       01  M-TAB.
           05 M-ROW OCCURS 2.
              10 M-CELL            PIC 9 OCCURS 3 VALUE 5.

       01  B-TAB.
           05 B-E OCCURS 400.
              10 B-N               PIC 9(4) VALUE 1234.
              10 B-X               PIC X(6) VALUE "abcdef".
              10 FILLER            PIC X VALUE "|".
       01  H-TAB.
           05 H-E                  PIC 9(4) OCCURS 100000 VALUE 55.

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
           DISPLAY "TEST-INITIALIZE-OUTERMOST".
           DISPLAY "Construct: INITIALIZE of items the outermost".
           DISPLAY "  program declares: groups, tables, REDEFINES".
           DISPLAY "--------------------------------------------".
           PERFORM O01-ELEMENTARY.
           PERFORM O02-GROUP-WITH-FILLER.
           PERFORM O03-TABLES-WRITTEN-BEFORE.
           PERFORM O04-TABLE-OF-GROUPS.
           PERFORM O05-ONE-OCCURRENCE.
           PERFORM O06-SAME-CHILD-NAME-IN-TWO-GROUPS.
           PERFORM O07-QUALIFIED-CHILD.
           PERFORM O08-REDEFINES.
           PERFORM O09-CONDITION-NAME.
           PERFORM O10-REPLACING-OVER-A-TABLE.
           PERFORM O11-SLICE-OF-A-2-D-TABLE.
           PERFORM O12-BULK-WRITTEN.
           PERFORM O13-BULK-UNTOUCHED.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 13".
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

       O01-ELEMENTARY.
           INITIALIZE E-X E-N E-D E-S.
           IF E-S = ZERO
               MOVE "Z" TO E-FLAG
           END-IF.
           MOVE SPACES TO WS-GOT.
           STRING "[" E-X "][" E-N "][" E-D "][" E-FLAG "][" E-KEEP "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[          ][000][00000][Z][kep]" TO WS-WANT.
           MOVE "O01 X(10) 9(3) 9(3)V99 S9(3)V99" TO WS-NAME.
           PERFORM CHECK-IT.
       O02-GROUP-WITH-FILLER.
           INITIALIZE G-GRP.
           MOVE SPACES TO WS-GOT.
           STRING "[" G-GRP "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[   00-00]" TO WS-WANT.
           MOVE "O02 group, its FILLER kept" TO WS-NAME.
           PERFORM CHECK-IT.
       O03-TABLES-WRITTEN-BEFORE.
           MOVE "ZZ" TO T-TE (2).
           MOVE 99 TO T-TN (3).
           INITIALIZE T-TEXT T-NUM.
           MOVE SPACES TO WS-GOT.
           STRING "[" T-TE (1) T-TE (2) T-TE (3) "]["
               T-TN (1) T-TN (2) T-TN (3) "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[      ][000000]" TO WS-WANT.
           MOVE "O03 tables, occurrences written before" TO WS-NAME.
           PERFORM CHECK-IT.
       O04-TABLE-OF-GROUPS.
           MOVE 9 TO U-C (2).
           MOVE "w" TO U-S (1).
           INITIALIZE U-TAB.
           MOVE SPACES TO WS-GOT.
           STRING "[" U-TAB "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[0 0 ]" TO WS-WANT.
           MOVE "O04 table of groups" TO WS-NAME.
           PERFORM CHECK-IT.
       O05-ONE-OCCURRENCE.
           MOVE "XX" TO P-E (1).
           MOVE "YY" TO P-E (2).
           MOVE "ZZ" TO P-E (3).
           INITIALIZE P-E (2).
           MOVE SPACES TO WS-GOT.
           STRING "[" P-E (1) P-E (2) P-E (3) "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[XX  ZZ]" TO WS-WANT.
           MOVE "O05 P(2) only, 1 and 3 untouched" TO WS-NAME.
           PERFORM CHECK-IT.
       O06-SAME-CHILD-NAME-IN-TWO-GROUPS.
           INITIALIZE D-G1 D-G2.
           MOVE SPACES TO WS-GOT.
           STRING "[" D-F OF D-G1 "][" D-F OF D-G2 "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[   ][   ]" TO WS-WANT.
           MOVE "O06 child F in two groups, both reset" TO WS-NAME.
           PERFORM CHECK-IT.
       O07-QUALIFIED-CHILD.
           INITIALIZE D-H OF D-G4.
           MOVE SPACES TO WS-GOT.
           STRING "[" D-H OF D-G3 "][" D-H OF D-G4 "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[aaa][   ]" TO WS-WANT.
           MOVE "O07 H OF G4 only, G3's H kept" TO WS-NAME.
           PERFORM CHECK-IT.
       O08-REDEFINES.
           INITIALIZE R-GRP.
           MOVE SPACES TO WS-GOT.
           STRING "[" R-N "][" R-R "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[00][00]" TO WS-WANT.
           MOVE "O08 REDEFINES follows its target" TO WS-NAME.
           PERFORM CHECK-IT.
       O09-CONDITION-NAME.
           INITIALIZE L-GRP.
           IF L-IS-Y
               MOVE "Y" TO L-SAW
           ELSE
               MOVE "n" TO L-SAW
           END-IF.
           MOVE SPACES TO WS-GOT.
           STRING "[" L-FLAG "][" L-AFTER "][" L-SAW "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[ ][ ][n]" TO WS-WANT.
           MOVE "O09 88-level beside data items" TO WS-NAME.
           PERFORM CHECK-IT.
       O10-REPLACING-OVER-A-TABLE.
           INITIALIZE X-GRP REPLACING NUMERIC DATA BY 5.
           MOVE SPACES TO WS-GOT.
           STRING "[" X-GRP "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[005ABC005ABC]" TO WS-WANT.
           MOVE "O10 REPLACING NUMERIC over a table" TO WS-NAME.
           PERFORM CHECK-IT.
       O11-SLICE-OF-A-2-D-TABLE.
           MOVE 8 TO M-CELL (1, 1).
           MOVE 8 TO M-CELL (2, 2).
           INITIALIZE M-ROW (2).
           MOVE SPACES TO WS-GOT.
           STRING "[" M-CELL (1, 1) M-CELL (1, 2) M-CELL (1, 3) "]["
               M-CELL (2, 1) M-CELL (2, 2) M-CELL (2, 3) "]"
               DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[855][000]" TO WS-WANT.
           MOVE "O11 ROW (2) of a 2 x 3 table" TO WS-NAME.
           PERFORM CHECK-IT.
       O12-BULK-WRITTEN.
           ACCEPT PERF-START FROM TIME.
           PERFORM VARYING WS-PASS FROM 1 BY 1 UNTIL WS-PASS > 50
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 400
                   MOVE 7 TO B-N (WS-I)
                   MOVE "zz" TO B-X (WS-I)
               END-PERFORM
               INITIALIZE B-TAB
           END-PERFORM.
           ACCEPT PERF-END FROM TIME.
           MOVE SPACES TO WS-GOT.
           STRING "[" B-N (1) "][" B-X (1) "][" B-N (400) "]["
               B-X (400) "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[0000][      ][0000][      ]" TO WS-WANT.
           MOVE "O12 400 groups, written then reset x50" TO WS-NAME.
           PERFORM CHECK-IT.
           COMPUTE PERF-LEAVES = 50 * 400 * 2.
           PERFORM TIME-IT.
           DISPLAY "  bulk O12: leaves written and reset: "
               PERF-LEAVES-EDIT.
           DISPLAY "  bulk O12: elapsed seconds         : "
               PERF-ELAPSED-EDIT.
           DISPLAY "  bulk O12: leaves/second           : "
               PERF-PER-SEC-EDIT.
       O13-BULK-UNTOUCHED.
           MOVE 9 TO H-E (100000).
           ACCEPT PERF-START FROM TIME.
           PERFORM VARYING WS-PASS FROM 1 BY 1 UNTIL WS-PASS > 50
               INITIALIZE H-TAB
           END-PERFORM.
           ACCEPT PERF-END FROM TIME.
           MOVE SPACES TO WS-GOT.
           STRING "[" H-E (1) "][" H-E (50000) "][" H-E (99999) "]["
               H-E (100000) "]" DELIMITED BY SIZE INTO WS-GOT.
           MOVE "[0000][0000][0000][0000]" TO WS-WANT.
           MOVE "O13 100,000 occurrences, untouched x50" TO WS-NAME.
           PERFORM CHECK-IT.
           COMPUTE PERF-LEAVES = 50 * 100000.
           PERFORM TIME-IT.
           DISPLAY "  bulk O13: occurrences covered     : "
               PERF-LEAVES-EDIT.
           DISPLAY "  bulk O13: elapsed seconds         : "
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
           IF WS-GOT = WS-WANT
               ADD 1 TO TESTS-PASSED
               DISPLAY "  PASS " WS-NAME
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "  FAIL " WS-NAME
               DISPLAY "       got  [" WS-GOT "]"
               DISPLAY "       want [" WS-WANT "]"
           END-IF.
       END PROGRAM TEST-INITIALIZE-OUTERMOST.

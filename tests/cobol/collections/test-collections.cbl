       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-COLLECTIONS  (spec 109 S3)
      *> Collection methods over a Split list:
      *>   Map  Filter  SortByDescending  Take   (answer a list)
      *>   Join Sum     Max                       (answer one value)
      *> and PERFORM VARYING element THROUGH collection.
      *> Self-checking: a case prints only if it FAILS; one result
      *> block at the end gives the cases, the timing and the tally.
      *> ============================================================
       PROGRAM-ID. TEST-COLLECTIONS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-NAMES                PIC X(40) VALUE " b , a ,, c ".
       01  WS-NUMS                 PIC X(40) VALUE "10,4,7,25,3".
       01  WS-EMPTY                PIC X(10) VALUE SPACES.
       01  WS-RES                  PIC X(60) VALUE SPACES.
       01  WS-EXP                  PIC X(60) VALUE SPACES.
       01  WS-N                    PIC S9(9) VALUE 0.
       01  WS-EXPN                 PIC S9(9) VALUE 0.
       01  WS-LABEL                PIC X(40) VALUE SPACES.
       01  WS-ELEM                 PIC X(10) VALUE SPACES.
       01  WS-COUNT                PIC 9(4)  VALUE 0.
       01  WS-I                    PIC 9(6)  VALUE 0.
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0.
       01  WS-T0                   PIC 9(8)  VALUE 0.
       01  WS-T1                   PIC 9(8)  VALUE 0.
       01  WS-TMS                  PIC 9(9)  VALUE 0.
       01  WS-LOOPS                PIC 9(6)  VALUE 2000.
       01  WS-RATE                 PIC 9(9)  VALUE 0.
       01  WS-HH                   PIC 9(2)  VALUE 0.
       01  WS-MM                   PIC 9(2)  VALUE 0.
       01  WS-SS                   PIC 9(2)  VALUE 0.
       01  WS-CC                   PIC 9(2)  VALUE 0.
       01  WS-MS0                  PIC 9(9)  VALUE 0.
       01  WS-MS1                  PIC 9(9)  VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT WS-T0 FROM TIME.
           PERFORM CASES-LIST.
           PERFORM CASES-VALUE.
           PERFORM CASES-THROUGH.
           ACCEPT WS-T1 FROM TIME.
           PERFORM TIMING-LOOP.
           PERFORM SHOW-RESULTS.
           STOP RUN.

       CASES-LIST.
           MOVE "Map Trim, Filter blanks, Join" TO WS-LABEL.
           MOVE "b/a/c" TO WS-EXP.
           MOVE WS-NAMES::Split(",")::Map(ITEM => ITEM::Trim())
               ::Filter(ITEM => ITEM <> " ")::Join("/") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "SortByDescending then Join" TO WS-LABEL.
           MOVE "c,b,a" TO WS-EXP.
           MOVE WS-NAMES::Split(",")::Map(ITEM => ITEM::Trim())
               ::Filter(ITEM => ITEM <> " ")::SortByDescending(ITEM => ITEM)
               ::Join(",") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "Take 2 of 3" TO WS-LABEL.
           MOVE "b/a" TO WS-EXP.
           MOVE WS-NAMES::Split(",")::Map(ITEM => ITEM::Trim())
               ::Filter(ITEM => ITEM <> " ")::Take(2)::Join("/") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "Take 0 is empty" TO WS-LABEL.
           MOVE SPACES TO WS-EXP.
           MOVE WS-NAMES::Split(",")::Take(0)::Join("/") TO WS-RES.
           PERFORM CHECK-TEXT.

       CASES-VALUE.
           MOVE "Sum of a numeric list" TO WS-LABEL.
           MOVE 49 TO WS-EXPN.
           MOVE WS-NUMS::Split(",")::Sum() TO WS-N.
           PERFORM CHECK-NUM.
           MOVE "Max of a numeric list" TO WS-LABEL.
           MOVE 25 TO WS-EXPN.
           MOVE WS-NUMS::Split(",")::Max() TO WS-N.
           PERFORM CHECK-NUM.
           MOVE "Sum after Filter on Len" TO WS-LABEL.
           MOVE 35 TO WS-EXPN.
           MOVE WS-NUMS::Split(",")::Map(ITEM => ITEM::Trim())::Filter(ITEM => ITEM::Len > 1)::Sum() TO WS-N.
           PERFORM CHECK-NUM.
           MOVE "Sum of an empty list is zero" TO WS-LABEL.
           MOVE 0 TO WS-EXPN.
           MOVE WS-NUMS::Split(",")::Map(ITEM => ITEM::Trim())::Filter(ITEM => ITEM::Len > 99)::Sum() TO WS-N.
           PERFORM CHECK-NUM.

       CASES-THROUGH.
           MOVE "THROUGH visits every element" TO WS-LABEL.
           MOVE 0 TO WS-COUNT.
           MOVE SPACES TO WS-RES.
           PERFORM VARYING WS-ELEM THROUGH
                   WS-NUMS::Split(",")::Map(ITEM => ITEM::Trim())::Filter(ITEM => ITEM::Len > 1)
               ADD 1 TO WS-COUNT
           END-PERFORM.
           MOVE WS-COUNT TO WS-N.
           MOVE 2 TO WS-EXPN.
           PERFORM CHECK-NUM.
           MOVE "THROUGH early exit" TO WS-LABEL.
           MOVE 0 TO WS-COUNT.
           PERFORM VARYING WS-ELEM THROUGH WS-NUMS::Split(",")
               ADD 1 TO WS-COUNT
               IF WS-COUNT = 2
                   EXIT PERFORM
               END-IF
           END-PERFORM.
           MOVE WS-COUNT TO WS-N.
           MOVE 2 TO WS-EXPN.
           PERFORM CHECK-NUM.

       TIMING-LOOP.
           PERFORM TIME-TO-MS-1.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LOOPS
               MOVE WS-NAMES::Split(",")::Map(ITEM => ITEM::Trim())
                   ::Filter(ITEM => ITEM <> " ")::Join("/") TO WS-RES
           END-PERFORM.
           ACCEPT WS-T1 FROM TIME.
           PERFORM TIME-TO-MS-2.
           IF WS-MS1 >= WS-MS0
               COMPUTE WS-TMS = WS-MS1 - WS-MS0
           ELSE
               MOVE 0 TO WS-TMS
           END-IF.
           IF WS-TMS > 0
               COMPUTE WS-RATE = WS-LOOPS * 1000 / WS-TMS
           ELSE
               MOVE WS-LOOPS TO WS-RATE
           END-IF.

       TIME-TO-MS-1.
           ACCEPT WS-T0 FROM TIME.
           MOVE WS-T0(1:2) TO WS-HH.
           MOVE WS-T0(3:2) TO WS-MM.
           MOVE WS-T0(5:2) TO WS-SS.
           MOVE WS-T0(7:2) TO WS-CC.
           COMPUTE WS-MS0 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.

       TIME-TO-MS-2.
           MOVE WS-T1(1:2) TO WS-HH.
           MOVE WS-T1(3:2) TO WS-MM.
           MOVE WS-T1(5:2) TO WS-SS.
           MOVE WS-T1(7:2) TO WS-CC.
           COMPUTE WS-MS1 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.

       CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           IF WS-RES = WS-EXP
               ADD 1 TO WS-TOT-OK
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      [" WS-RES "]"
               DISPLAY "  expected [" WS-EXP "]"
           END-IF.

       CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           IF WS-N = WS-EXPN
               ADD 1 TO WS-TOT-OK
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      " WS-N
               DISPLAY "  expected " WS-EXPN
           END-IF.

       SHOW-RESULTS.
           DISPLAY "============================================".
           DISPLAY "TEST-COLLECTIONS (spec 109 S3)".
           DISPLAY "Forms: Map Filter SortByDescending Take Join".
           DISPLAY "       Sum Max, PERFORM VARYING THROUGH".
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES RUN       : " WS-TOT-RUN.
           DISPLAY "CASES PASSED    : " WS-TOT-OK.
           DISPLAY "CASES FAILED    : " WS-TOT-BAD.
           DISPLAY "PHASE TIMING    : " WS-LOOPS
               " chains Map/Filter/Join in " WS-TMS " ms".
           DISPLAY "THROUGHPUT      : " WS-RATE " chains/sec".
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".

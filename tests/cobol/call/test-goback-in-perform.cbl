       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-GOBACK-IN-PERFORM
      *> GOBACK and EXIT PROGRAM return control to the caller from
      *> wherever they are executed, including a paragraph or
      *> section reached by PERFORM. They used to end only the
      *> PERFORM, so the called program carried on with the
      *> statement after it.
      *> Each case CALLs a contained program that leaves a marker in
      *> its LINKAGE item before and after the PERFORM; the marker
      *> the caller gets back shows where the program stopped.
      *> Forms exercised:
      *>   G01  GOBACK in a paragraph PERFORMed once
      *>   G02  GOBACK two PERFORM levels deep
      *>   G03  GOBACK inside an inline PERFORM UNTIL loop
      *>   G04  GOBACK in a paragraph PERFORMed VARYING
      *>   G05  GOBACK in the middle of a PERFORM ... THRU range
      *>   G06  GOBACK in a PERFORMed SECTION
      *>   G07  EXIT PROGRAM in a PERFORMed paragraph
      *>   G08  GO TO the exit paragraph (unchanged, the control)
      *>   G09  the caller's own PERFORM goes on after the CALL
      *>   G10  the same program called again runs from the top
      *> Then a bulk phase: 20,000 CALLs that each leave through
      *> two PERFORM levels, timed (a level left open would hit the
      *> PERFORM depth limit long before the end).
      *> Self-checking. One result block at the end.
      *> ============================================================
       PROGRAM-ID. TEST-GOBACK-IN-PERFORM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-R                    PIC X(20) VALUE SPACES.
       01  WS-AFTER                PIC X(20) VALUE SPACES.
       01  G-GOT                   PIC X(60) VALUE SPACES.
       01  G-WANT                  PIC X(60) VALUE SPACES.
       01  G-NAME                  PIC X(44) VALUE SPACES.
       01  BULK-CALLS              PIC 9(6)  VALUE 0.
       01  BULK-OK                 PIC 9(6)  VALUE 0.
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
       01  PERF-ELAPSED-MS         PIC 9(9) VALUE 0.
       01  PERF-ELAPSED-MS-EDIT    PIC ZZZ,ZZZ,ZZ9.
       01  PERF-PER-SEC            PIC 9(9) VALUE 0.
       01  PERF-PER-SEC-EDIT       PIC ZZZ,ZZZ,ZZ9.
       01  BULK-CALLS-EDIT         PIC ZZZ,ZZ9.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-GOBACK-IN-PERFORM".
           DISPLAY "Construct: GOBACK / EXIT PROGRAM executed in a".
           DISPLAY "  paragraph or section reached by PERFORM".
           DISPLAY "--------------------------------------------".
           PERFORM G01-ONE-LEVEL.
           PERFORM G02-TWO-LEVELS.
           PERFORM G03-INLINE-LOOP.
           PERFORM G04-VARYING.
           PERFORM G05-THRU-RANGE.
           PERFORM G06-SECTION.
           PERFORM G07-EXIT-PROGRAM.
           PERFORM G08-GO-TO.
           PERFORM G09-CALLER-PERFORM-GOES-ON.
           PERFORM G10-CALLED-AGAIN.
           PERFORM B01-BULK.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 11 (10 forms + the bulk count)".
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

       G01-ONE-LEVEL.
           MOVE SPACES TO WS-R.

      *> G01: PERFORMs its exit paragraph, which holds the GOBACK.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "BEFORE-END" (it is "CONTINUED" if the
      *>         program ran on after the PERFORM).

           CALL "G01" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "BEFORE-END" TO G-WANT.
           MOVE "G01 GOBACK in a PERFORMed paragraph" TO G-NAME.
           PERFORM CHECK-IT.
       G02-TWO-LEVELS.
           MOVE SPACES TO WS-R.

      *> G02: PERFORMs a paragraph that PERFORMs the one holding
      *>      the GOBACK.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "LEVEL-2".

           CALL "G02" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "LEVEL-2" TO G-WANT.
           MOVE "G02 GOBACK two PERFORM levels deep" TO G-NAME.
           PERFORM CHECK-IT.
       G03-INLINE-LOOP.
           MOVE SPACES TO WS-R.

      *> G03: an inline PERFORM UNTIL whose third pass GOBACKs.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "STOPPED-AT-3".

           CALL "G03" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "STOPPED-AT-3" TO G-WANT.
           MOVE "G03 GOBACK inside PERFORM UNTIL" TO G-NAME.
           PERFORM CHECK-IT.
       G04-VARYING.
           MOVE SPACES TO WS-R.

      *> G04: PERFORM ... VARYING 1 BY 1 UNTIL > 5 of a paragraph
      *>      that GOBACKs on its second pass.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "STOPPED-AT-2".

           CALL "G04" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "STOPPED-AT-2" TO G-WANT.
           MOVE "G04 GOBACK in a paragraph PERFORMed VARYING"
               TO G-NAME.
           PERFORM CHECK-IT.
       G05-THRU-RANGE.
           MOVE SPACES TO WS-R.

      *> G05: PERFORM G05-A THRU G05-C, where G05-B GOBACKs.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "B" (G05-C never runs).

           CALL "G05" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "B" TO G-WANT.
           MOVE "G05 GOBACK inside a PERFORM THRU range" TO G-NAME.
           PERFORM CHECK-IT.
       G06-SECTION.
           MOVE SPACES TO WS-R.

      *> G06: PERFORMs a SECTION whose second paragraph GOBACKs.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "SECTION-L1" (the third paragraph never
      *>         runs).

           CALL "G06" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "SECTION-L1" TO G-WANT.
           MOVE "G06 GOBACK in a PERFORMed SECTION" TO G-NAME.
           PERFORM CHECK-IT.
       G07-EXIT-PROGRAM.
           MOVE SPACES TO WS-R.

      *> G07: PERFORMs a paragraph holding EXIT PROGRAM.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "BEFORE-EXIT".

           CALL "G07" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "BEFORE-EXIT" TO G-WANT.
           MOVE "G07 EXIT PROGRAM in a PERFORMed paragraph"
               TO G-NAME.
           PERFORM CHECK-IT.
       G08-GO-TO.
           MOVE SPACES TO WS-R.

      *> G08: GO TO its exit paragraph, which holds the GOBACK.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "BEFORE-END".

           CALL "G08" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "BEFORE-END" TO G-WANT.
           MOVE "G08 GO TO the exit paragraph (control)" TO G-NAME.
           PERFORM CHECK-IT.
       G09-CALLER-PERFORM-GOES-ON.
           MOVE SPACES TO WS-R.
           MOVE SPACES TO WS-AFTER.
           PERFORM G09-CALL-FROM-A-PERFORM.
           MOVE SPACES TO G-GOT.
           STRING "[" WS-R "][" WS-AFTER "]" DELIMITED BY SIZE
               INTO G-GOT.
           MOVE "[BEFORE-END          ][CALLER-WENT-ON      ]"
               TO G-WANT.
           MOVE "G09 the caller's PERFORM goes on" TO G-NAME.
           PERFORM CHECK-IT.
       G09-CALL-FROM-A-PERFORM.

      *> G01: PERFORMs its exit paragraph, which holds the GOBACK.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "BEFORE-END"; the GOBACK ends G01 only, so
      *>         this paragraph goes on to its next statement.

           CALL "G01" USING WS-R.

           MOVE "CALLER-WENT-ON" TO WS-AFTER.
       G10-CALLED-AGAIN.
           MOVE SPACES TO WS-R.

      *> G02: same as in G02-TWO-LEVELS; its second CALL here.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "LEVEL-2".

           CALL "G02" USING WS-R.

           MOVE SPACES TO WS-R.

      *> G02: the third CALL, to show nothing was left open.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "LEVEL-2".

           CALL "G02" USING WS-R.

           MOVE WS-R TO G-GOT.
           MOVE "LEVEL-2" TO G-WANT.
           MOVE "G10 the program called again runs again" TO G-NAME.
           PERFORM CHECK-IT.
       B01-BULK.
           MOVE 0 TO BULK-CALLS.
           MOVE 0 TO BULK-OK.
           ACCEPT PERF-START FROM TIME.
           PERFORM B01-ONE-CALL 20000 TIMES.
           ACCEPT PERF-END FROM TIME.
           MOVE SPACES TO G-GOT.
           STRING "[" BULK-CALLS "][" BULK-OK "]" DELIMITED BY SIZE
               INTO G-GOT.
           MOVE "[020000][020000]" TO G-WANT.
           MOVE "B01 20,000 CALLs leave two PERFORM levels"
               TO G-NAME.
           PERFORM CHECK-IT.
           PERFORM TIME-IT.
           MOVE BULK-CALLS TO BULK-CALLS-EDIT.
           DISPLAY "  bulk B01: CALLs made              : "
               BULK-CALLS-EDIT.
           DISPLAY "  bulk B01: elapsed ms              : "
               PERF-ELAPSED-MS-EDIT " (clock resolution 10 ms)".
           DISPLAY "  bulk B01: CALLs/second            : "
               PERF-PER-SEC-EDIT.
       B01-ONE-CALL.
           MOVE SPACES TO WS-R.

      *> G02: leaves through two PERFORM levels.
      *> Uses:   WS-R = spaces.
      *> Result: WS-R = "LEVEL-2".

           CALL "G02" USING WS-R.

           ADD 1 TO BULK-CALLS.
           IF WS-R = "LEVEL-2"
               ADD 1 TO BULK-OK
           END-IF.
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
           COMPUTE PERF-ELAPSED-MS = PERF-ELAPSED-CS * 10.
           COMPUTE PERF-PER-SEC = BULK-CALLS * 100 / PERF-ELAPSED-CS.
           MOVE PERF-ELAPSED-MS TO PERF-ELAPSED-MS-EDIT.
           MOVE PERF-PER-SEC TO PERF-PER-SEC-EDIT.
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

      *> ----- G01: GOBACK in a PERFORMed paragraph -----------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G01.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G01-START.
           MOVE "BEFORE-END" TO LK-R
           PERFORM G01-END
           MOVE "CONTINUED" TO LK-R
           GOBACK.
       G01-END.
           GOBACK.
       END PROGRAM G01.

      *> ----- G02: GOBACK two PERFORM levels deep ------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G02.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G02-START.
           MOVE "LEVEL-0" TO LK-R
           PERFORM G02-LEVEL-1
           MOVE "CONTINUED-0" TO LK-R
           GOBACK.
       G02-LEVEL-1.
           MOVE "LEVEL-1" TO LK-R
           PERFORM G02-LEVEL-2
           MOVE "CONTINUED-1" TO LK-R.
       G02-LEVEL-2.
           MOVE "LEVEL-2" TO LK-R
           GOBACK.
       END PROGRAM G02.

      *> ----- G03: GOBACK inside an inline PERFORM UNTIL -----------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G03.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-I                    PIC 99 VALUE 0.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G03-START.
           MOVE 0 TO WS-I
           PERFORM UNTIL WS-I > 10
               ADD 1 TO WS-I
               IF WS-I = 3
                   MOVE "STOPPED-AT-3" TO LK-R
                   GOBACK
               END-IF
           END-PERFORM
           MOVE "RAN-TO-END" TO LK-R
           GOBACK.
       END PROGRAM G03.

      *> ----- G04: GOBACK in a paragraph PERFORMed VARYING ---------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G04.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-I                    PIC 99 VALUE 0.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G04-START.
           PERFORM G04-STEP VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
           MOVE "RAN-TO-END" TO LK-R
           GOBACK.
       G04-STEP.
           IF WS-I = 2
               MOVE "STOPPED-AT-2" TO LK-R
               GOBACK
           END-IF.
       END PROGRAM G04.

      *> ----- G05: GOBACK in the middle of a PERFORM THRU range ----
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G05.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G05-START.
           PERFORM G05-A THRU G05-C
           MOVE "CONTINUED" TO LK-R
           GOBACK.
       G05-A.
           MOVE "A" TO LK-R.
       G05-B.
           MOVE "B" TO LK-R
           GOBACK.
       G05-C.
           MOVE "C" TO LK-R.
       END PROGRAM G05.

      *> ----- G06: GOBACK in a PERFORMed SECTION -------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G06.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G06-MAIN SECTION.
       G06-START.
           MOVE "BEFORE-SECTION" TO LK-R
           PERFORM G06-LEAVE-SECTION
           MOVE "CONTINUED" TO LK-R
           GOBACK.
       G06-LEAVE-SECTION SECTION.
       G06-L1.
           MOVE "SECTION-L1" TO LK-R.
       G06-L2.
           GOBACK.
       G06-L3.
           MOVE "SECTION-L3" TO LK-R.
       END PROGRAM G06.

      *> ----- G07: EXIT PROGRAM in a PERFORMed paragraph -----------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G07.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G07-START.
           MOVE "BEFORE-EXIT" TO LK-R
           PERFORM G07-LEAVE
           MOVE "CONTINUED" TO LK-R
           GOBACK.
       G07-LEAVE.
           EXIT PROGRAM.
       END PROGRAM G07.

      *> ----- G08: GO TO the exit paragraph (the control) ----------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. G08.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-R                    PIC X(20).
       PROCEDURE DIVISION USING LK-R.
       G08-START.
           MOVE "BEFORE-END" TO LK-R
           GO TO G08-END.
       G08-MIDDLE.
           MOVE "CONTINUED" TO LK-R.
       G08-END.
           GOBACK.
       END PROGRAM G08.

       END PROGRAM TEST-GOBACK-IN-PERFORM.

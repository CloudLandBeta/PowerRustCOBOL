       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-IF-CONDITIONS
      *> Direct conditional constructs: IF / ELSE / nested / END-IF,
      *> combined (full + abbreviated) conditions, AND / OR / NOT,
      *> parenthesised conditions, relational operators, class tests,
      *> sign tests and 88-level condition-names (incl. SET TO TRUE).
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-IF-CONDITIONS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-A                    PIC S9(4) VALUE 0.
       01  WS-B                    PIC S9(4) VALUE 0.
       01  WS-HIT                  PIC X     VALUE "N".
       01  WS-NUM-TEXT             PIC X(5)  VALUE SPACES.
       01  WS-ALPHA-TEXT           PIC X(5)  VALUE SPACES.
       01  WS-STATUS               PIC X     VALUE "Y".
           88 STATUS-OK            VALUE "Y".
           88 STATUS-ERROR         VALUE "N".
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-IF-CONDITIONS".
           DISPLAY "Constructs: IF/ELSE/nested/END-IF, combined &".
           DISPLAY "  abbreviated conditions, AND/OR/NOT, parens,".
           DISPLAY "  =, NOT =, >, <, >=, <=, IS NUMERIC/ALPHABETIC,".
           DISPLAY "  IS POSITIVE/NEGATIVE/ZERO, 88-level + SET TRUE".
           DISPLAY "--------------------------------------------".
           PERFORM T001-IF-TRUE.
           PERFORM T002-IF-ELSE.
           PERFORM T003-NESTED-IF.
           PERFORM T004-END-IF-SCOPE.
           PERFORM T005-ABBREV-OR.
           PERFORM T006-ABBREV-OR-FALSE.
           PERFORM T007-FULL-COMBINED-OR.
           PERFORM T008-AND.
           PERFORM T009-OR.
           PERFORM T010-NOT.
           PERFORM T011-PARENS.
           PERFORM T012-REL-EQ.
           PERFORM T013-REL-NE.
           PERFORM T014-REL-GT.
           PERFORM T015-REL-LT.
           PERFORM T016-REL-GE.
           PERFORM T017-REL-LE.
           PERFORM T018-CLASS-NUMERIC-T.
           PERFORM T019-CLASS-NUMERIC-F.
           PERFORM T020-CLASS-ALPHA-T.
           PERFORM T021-CLASS-ALPHA-F.
           PERFORM T022-SIGN-POSITIVE.
           PERFORM T023-SIGN-NEGATIVE.
           PERFORM T024-SIGN-ZERO.
           PERFORM T025-88-TRUE.
           PERFORM T026-88-FALSE.
           PERFORM T027-88-SET-TRUE.
           PERFORM T028-CLASS-NOT-NUMERIC.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 28".
           DISPLAY "TESTS RUN       : " TESTS-RUN.
           DISPLAY "TESTS PASSED    : " TESTS-PASSED.
           DISPLAY "TESTS FAILED    : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".
           STOP RUN.
      *> ---- individual cases ----
       T001-IF-TRUE.
           ADD 1 TO TESTS-RUN.
           MOVE 5 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A = 5
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T001 IF (then-branch taken when true)".
       T002-IF-ELSE.
           ADD 1 TO TESTS-RUN.
           MOVE 9 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A = 5
               MOVE "X" TO WS-HIT
           ELSE
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T002 IF/ELSE (else-branch taken when false)".
       T003-NESTED-IF.
           ADD 1 TO TESTS-RUN.
           MOVE 5 TO WS-A.
           MOVE 7 TO WS-B.
           MOVE "N" TO WS-HIT.
           IF WS-A = 5
               IF WS-B = 7
                   MOVE "Y" TO WS-HIT
               END-IF
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T003 nested IF (inner reached only when outer".
           DISPLAY "       true)".
       T004-END-IF-SCOPE.
           ADD 1 TO TESTS-RUN.
           MOVE 1 TO WS-A.
           MOVE 0 TO WS-B.
           IF WS-A = 1
               ADD 10 TO WS-B
               ADD 5  TO WS-B
           END-IF.
           IF WS-B = 15
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T004 END-IF groups two statements in THEN".
       T005-ABBREV-OR.
           ADD 1 TO TESTS-RUN.
           MOVE 2 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A = 1 OR 2 OR 3
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T005 abbreviated: IF WS-A = 1 OR 2 OR 3 (A=2)".
       T006-ABBREV-OR-FALSE.
           ADD 1 TO TESTS-RUN.
           MOVE 9 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A = 1 OR 2 OR 3
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "N"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T006 abbreviated OR false when A=9".
       T007-FULL-COMBINED-OR.
           ADD 1 TO TESTS-RUN.
           MOVE 3 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A = 1 OR WS-A = 2 OR WS-A = 3
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T007 full: WS-A=1 OR WS-A=2 OR WS-A=3 (A=3)".
       T008-AND.
           ADD 1 TO TESTS-RUN.
           MOVE 5 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A > 0 AND WS-A < 10
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T008 AND: WS-A > 0 AND WS-A < 10".
       T009-OR.
           ADD 1 TO TESTS-RUN.
           MOVE 50 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF WS-A < 0 OR WS-A > 10
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T009 OR: WS-A < 0 OR WS-A > 10".
       T010-NOT.
           ADD 1 TO TESTS-RUN.
           MOVE 5 TO WS-A.
           MOVE "N" TO WS-HIT.
           IF NOT WS-A = 0
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T010 NOT: IF NOT WS-A = 0".
       T011-PARENS.
           ADD 1 TO TESTS-RUN.
           MOVE 2 TO WS-A.
           MOVE 9 TO WS-B.
           MOVE "N" TO WS-HIT.
           IF (WS-A = 1 OR WS-A = 2) AND WS-B = 9
               MOVE "Y" TO WS-HIT
           END-IF.
           IF WS-HIT = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T011 parentheses: (A=1 OR A=2) AND B=9".
       T012-REL-EQ.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A = 7
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T012 relational  =".
       T013-REL-NE.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A NOT = 8
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T013 relational  NOT =".
       T014-REL-GT.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A > 3
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T014 relational  >".
       T015-REL-LT.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A < 10
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T015 relational  <".
       T016-REL-GE.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A >= 7
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T016 relational  >=".
       T017-REL-LE.
           ADD 1 TO TESTS-RUN.
           MOVE 7 TO WS-A.
           IF WS-A <= 7
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T017 relational  <=".
       T018-CLASS-NUMERIC-T.
           ADD 1 TO TESTS-RUN.
           MOVE "12345" TO WS-NUM-TEXT.
           IF WS-NUM-TEXT IS NUMERIC
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T018 class IS NUMERIC true  (""12345"")".
       T019-CLASS-NUMERIC-F.
           ADD 1 TO TESTS-RUN.
           MOVE "12A45" TO WS-NUM-TEXT.
           IF WS-NUM-TEXT IS NUMERIC
               PERFORM FAIL-IT
           ELSE
               PERFORM PASS-IT
           END-IF.
           DISPLAY "  T019 class IS NUMERIC false (""12A45"")".
       T020-CLASS-ALPHA-T.
           ADD 1 TO TESTS-RUN.
           MOVE "ABCDE" TO WS-ALPHA-TEXT.
           IF WS-ALPHA-TEXT IS ALPHABETIC
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T020 class IS ALPHABETIC true  (""ABCDE"")".
       T021-CLASS-ALPHA-F.
           ADD 1 TO TESTS-RUN.
           MOVE "AB123" TO WS-ALPHA-TEXT.
           IF WS-ALPHA-TEXT IS ALPHABETIC
               PERFORM FAIL-IT
           ELSE
               PERFORM PASS-IT
           END-IF.
           DISPLAY "  T021 class IS ALPHABETIC false (""AB123"")".
       T022-SIGN-POSITIVE.
           ADD 1 TO TESTS-RUN.
           MOVE 42 TO WS-A.
           IF WS-A IS POSITIVE
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T022 sign IS POSITIVE  (+42)".
       T023-SIGN-NEGATIVE.
           ADD 1 TO TESTS-RUN.
           MOVE -7 TO WS-A.
           IF WS-A IS NEGATIVE
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T023 sign IS NEGATIVE  (-7)".
       T024-SIGN-ZERO.
           ADD 1 TO TESTS-RUN.
           MOVE 0 TO WS-A.
           IF WS-A IS ZERO
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T024 sign IS ZERO  (0)".
       T025-88-TRUE.
           ADD 1 TO TESTS-RUN.
           MOVE "Y" TO WS-STATUS.
           IF STATUS-OK
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T025 88-level true  (STATUS-OK when ""Y"")".
       T026-88-FALSE.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-STATUS.
           IF STATUS-OK
               PERFORM FAIL-IT
           ELSE
               PERFORM PASS-IT
           END-IF.
           DISPLAY "  T026 88-level false (STATUS-OK when ""N"")".
       T027-88-SET-TRUE.
           ADD 1 TO TESTS-RUN.
           MOVE "N" TO WS-STATUS.
           SET STATUS-OK TO TRUE.
           IF WS-STATUS = "Y"
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T027 SET STATUS-OK TO TRUE  (stores ""Y"")".
       T028-CLASS-NOT-NUMERIC.
           ADD 1 TO TESTS-RUN.
           MOVE "AB123" TO WS-NUM-TEXT.
           IF WS-NUM-TEXT IS NOT NUMERIC
               PERFORM PASS-IT
           ELSE
               PERFORM FAIL-IT
           END-IF.
           DISPLAY "  T028 negated class IS NOT NUMERIC (""AB123"")".
      *> ---- shared pass/fail recorders ----
       PASS-IT.
           ADD 1 TO TESTS-PASSED.
           DISPLAY "  PASS".
       FAIL-IT.
           ADD 1 TO TESTS-FAILED.
           DISPLAY "  FAIL".

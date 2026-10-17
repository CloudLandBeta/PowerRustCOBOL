       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-EVALUATE-SUBSCRIPT
      *> A SUBSCRIPTED data item (or a qualified one) is a valid
      *> EVALUATE subject, exactly like a plain item. It used to be
      *> read as a CONDITION, so every literal WHEN missed and the
      *> branch taken was always WHEN OTHER.
      *> Covers: 1- and 2-level tables, literal and variable subscript,
      *> numeric and alphanumeric subjects, THRU ranges, WHEN NOT,
      *> stacked WHEN, ALSO with two subjects, a WHEN object that is
      *> itself subscripted, and the two readings that must NOT change
      *> (a subscripted 88-level subject, a reference-modified subject).
      *> Self-checking: each case prints PASS/FAIL; a summary closes.
      *> ============================================================
       PROGRAM-ID. TEST-EVALUATE-SUBSCRIPT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN            PIC 9(4) VALUE 0.
           05 TESTS-PASSED         PIC 9(4) VALUE 0.
           05 TESTS-FAILED         PIC 9(4) VALUE 0.
       01  WS-RESULT               PIC X(12) VALUE SPACES.
       01  WS-EXPECT               PIC X(12) VALUE SPACES.
       01  WS-CASE                 PIC X(44) VALUE SPACES.
       01  N                       PIC 99 VALUE 2.
       01  ONE                     PIC X VALUE "D".
       01  WS-STR                  PIC X(4) VALUE "ABCD".
       01  SIMPLE-TBL.
           05 CODE-X               PIC X OCCURS 3.
              88 CODE-IS-D         VALUE "D".
       01  NUM-TBL.
           05 NUM-X                PIC 99 OCCURS 3.
       01  TBL.
           05 ROWX OCCURS 3.
              10 ST                PIC X.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "============================================".
           DISPLAY "TEST-EVALUATE-SUBSCRIPT".
           DISPLAY "A subscripted item as the EVALUATE subject".
           DISPLAY "--------------------------------------------".
           PERFORM SETUP-TABLES.
           PERFORM C01-ONE-LEVEL-LITERAL.
           PERFORM C02-ONE-LEVEL-VARIABLE.
           PERFORM C03-TWO-LEVEL-LITERAL.
           PERFORM C04-TWO-LEVEL-VARIABLE.
           PERFORM C05-PLAIN-ITEM.
           PERFORM C06-EVALUATE-TRUE.
           PERFORM C07-NO-MATCH-TAKES-OTHER.
           PERFORM C08-SUBSCRIPT-FIRST-ROW.
           PERFORM C09-SUBSCRIPT-LAST-ROW.
           PERFORM C10-NUMERIC-SUBJECT.
           PERFORM C11-THRU-NUMERIC-INSIDE.
           PERFORM C12-THRU-NUMERIC-OUTSIDE.
           PERFORM C13-THRU-ALPHA-ONE-LEVEL.
           PERFORM C14-THRU-ALPHA-TWO-LEVEL.
           PERFORM C15-WHEN-NOT.
           PERFORM C16-STACKED-WHEN.
           PERFORM C17-ALSO-BOTH-MATCH.
           PERFORM C18-ALSO-SECOND-MISSES.
           PERFORM C19-ALSO-ANY.
           PERFORM C20-ALSO-SUBSCRIPT-AND-PLAIN.
           PERFORM C21-ALSO-THRU-ON-SECOND.
           PERFORM C22-ALSO-THRU-ON-FIRST.
           PERFORM C23-ALSO-TWO-LEVEL-AND-NUMERIC.
           PERFORM C24-ALSO-THRU-ON-BOTH.
           PERFORM C25-WHEN-OBJECT-SUBSCRIPTED.
           PERFORM C26-QUALIFIED-SUBJECT.
           PERFORM C27-88-LEVEL-TRUE.
           PERFORM C28-88-LEVEL-FALSE.
           PERFORM C29-REF-MOD-SUBJECT.
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES EXERCISED : 29".
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
       SETUP-TABLES.
           MOVE "A" TO CODE-X(1).
           MOVE "D" TO CODE-X(2).
           MOVE "Z" TO CODE-X(3).
           MOVE 3  TO NUM-X(1).
           MOVE 7  TO NUM-X(2).
           MOVE 12 TO NUM-X(3).
           MOVE "A" TO ST(1).
           MOVE "D" TO ST(2).
           MOVE "Z" TO ST(3).
       C01-ONE-LEVEL-LITERAL.
           MOVE "1-level table, literal subscript" TO WS-CASE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(2)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C02-ONE-LEVEL-VARIABLE.
           MOVE "1-level table, variable subscript" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C03-TWO-LEVEL-LITERAL.
           MOVE "2-level table, literal subscript" TO WS-CASE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(2)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C04-TWO-LEVEL-VARIABLE.
           MOVE "2-level table, variable subscript" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C05-PLAIN-ITEM.
           MOVE "control: plain item" TO WS-CASE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ONE
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C06-EVALUATE-TRUE.
           MOVE "control: EVALUATE TRUE WHEN ST(N) = D" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE TRUE
               WHEN ST(N) = "A" MOVE "A"     TO WS-RESULT
               WHEN ST(N) = "D" MOVE "D"     TO WS-RESULT
               WHEN OTHER       MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C07-NO-MATCH-TAKES-OTHER.
           MOVE "no WHEN matches: WHEN OTHER is taken" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "B"   MOVE "B"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "OTHER" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C08-SUBSCRIPT-FIRST-ROW.
           MOVE "subscript 1 selects the first occurrence" TO WS-CASE.
           MOVE 1 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "A" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C09-SUBSCRIPT-LAST-ROW.
           MOVE "subscript 3 selects the last occurrence" TO WS-CASE.
           MOVE 3 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "Z"   MOVE "Z"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "Z" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C10-NUMERIC-SUBJECT.
           MOVE "numeric subscripted subject, literal WHEN" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE NUM-X(N)
               WHEN 3     MOVE "THREE" TO WS-RESULT
               WHEN 7     MOVE "SEVEN" TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "SEVEN" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C11-THRU-NUMERIC-INSIDE.
           MOVE "THRU range, subject inside" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE NUM-X(N)
               WHEN 1 THRU 5 MOVE "LOW"   TO WS-RESULT
               WHEN 6 THRU 9 MOVE "MID"   TO WS-RESULT
               WHEN OTHER    MOVE "HIGH"  TO WS-RESULT
           END-EVALUATE.
           MOVE "MID" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C12-THRU-NUMERIC-OUTSIDE.
           MOVE "THRU range, subject outside every range" TO WS-CASE.
           MOVE 3 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE NUM-X(N)
               WHEN 1 THRU 5 MOVE "LOW"   TO WS-RESULT
               WHEN 6 THRU 9 MOVE "MID"   TO WS-RESULT
               WHEN OTHER    MOVE "HIGH"  TO WS-RESULT
           END-EVALUATE.
           MOVE "HIGH" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C13-THRU-ALPHA-ONE-LEVEL.
           MOVE "THRU range, alphanumeric, 1-level" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN "A" THRU "C" MOVE "ABC"   TO WS-RESULT
               WHEN "D" THRU "F" MOVE "DEF"   TO WS-RESULT
               WHEN OTHER        MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "DEF" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C14-THRU-ALPHA-TWO-LEVEL.
           MOVE "THRU range, alphanumeric, 2-level" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N)
               WHEN "A" THRU "C" MOVE "ABC"   TO WS-RESULT
               WHEN "D" THRU "F" MOVE "DEF"   TO WS-RESULT
               WHEN OTHER        MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "DEF" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C15-WHEN-NOT.
           MOVE "WHEN NOT literal" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN NOT "D" MOVE "NOT-D" TO WS-RESULT
               WHEN OTHER   MOVE "IS-D"  TO WS-RESULT
           END-EVALUATE.
           MOVE "IS-D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C16-STACKED-WHEN.
           MOVE "stacked WHEN sharing one imperative" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N)
               WHEN "C"
               WHEN "D"   MOVE "C-OR-D" TO WS-RESULT
               WHEN OTHER MOVE "OTHER"  TO WS-RESULT
           END-EVALUATE.
           MOVE "C-OR-D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C17-ALSO-BOTH-MATCH.
           MOVE "two subjects, both subscripted, both match" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N) ALSO NUM-X(N)
               WHEN "D" ALSO 7 MOVE "BOTH"  TO WS-RESULT
               WHEN OTHER      MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "BOTH" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C18-ALSO-SECOND-MISSES.
           MOVE "two subjects, the second one misses" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N) ALSO NUM-X(N)
               WHEN "D" ALSO 8 MOVE "WRONG" TO WS-RESULT
               WHEN OTHER      MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "OTHER" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C19-ALSO-ANY.
           MOVE "two subjects, ANY on the second" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N) ALSO NUM-X(N)
               WHEN "D" ALSO ANY MOVE "D-ANY" TO WS-RESULT
               WHEN OTHER        MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D-ANY" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C20-ALSO-SUBSCRIPT-AND-PLAIN.
           MOVE "two subjects, subscripted ALSO plain item" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N) ALSO ONE
               WHEN "D" ALSO "D" MOVE "BOTH"  TO WS-RESULT
               WHEN OTHER        MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "BOTH" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C21-ALSO-THRU-ON-SECOND.
           MOVE "two subjects, THRU range on the second" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N) ALSO NUM-X(N)
               WHEN "D" ALSO 5 THRU 9 MOVE "IN"    TO WS-RESULT
               WHEN OTHER             MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "IN" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C22-ALSO-THRU-ON-FIRST.
           MOVE "two subjects, THRU range on the first" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-X(N) ALSO NUM-X(N)
               WHEN "A" THRU "F" ALSO 7 MOVE "IN"    TO WS-RESULT
               WHEN OTHER               MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "IN" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C23-ALSO-TWO-LEVEL-AND-NUMERIC.
           MOVE "two subjects, 2-level ALSO numeric" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N) ALSO NUM-X(N)
               WHEN "A" ALSO 7 MOVE "WRONG" TO WS-RESULT
               WHEN "D" ALSO 7 MOVE "D-7"   TO WS-RESULT
               WHEN OTHER      MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D-7" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C24-ALSO-THRU-ON-BOTH.
           MOVE "two subjects, THRU range on both" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST(N) ALSO NUM-X(N)
               WHEN "D" THRU "F" ALSO 1 THRU 5 MOVE "WRONG" TO WS-RESULT
               WHEN "D" THRU "F" ALSO 6 THRU 9 MOVE "IN-IN" TO WS-RESULT
               WHEN OTHER                      MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "IN-IN" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C25-WHEN-OBJECT-SUBSCRIPTED.
           MOVE "WHEN object is itself a subscripted item" TO WS-CASE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ONE
               WHEN CODE-X(1) MOVE "OBJ-1"  TO WS-RESULT
               WHEN CODE-X(2) MOVE "OBJ-2"  TO WS-RESULT
               WHEN OTHER     MOVE "OTHER"  TO WS-RESULT
           END-EVALUATE.
           MOVE "OBJ-2" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C26-QUALIFIED-SUBJECT.
           MOVE "qualified and subscripted: ST OF ROWX (N)" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE ST OF ROWX (N)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "D"   MOVE "D"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "D" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C27-88-LEVEL-TRUE.
           MOVE "subscripted 88-level subject, WHEN TRUE" TO WS-CASE.
           MOVE 2 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-IS-D(N)
               WHEN TRUE  MOVE "TRUE"  TO WS-RESULT
               WHEN FALSE MOVE "FALSE" TO WS-RESULT
           END-EVALUATE.
           MOVE "TRUE" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C28-88-LEVEL-FALSE.
           MOVE "subscripted 88-level subject, WHEN FALSE" TO WS-CASE.
           MOVE 1 TO N.
           MOVE SPACES TO WS-RESULT.
           EVALUATE CODE-IS-D(N)
               WHEN TRUE  MOVE "TRUE"  TO WS-RESULT
               WHEN FALSE MOVE "FALSE" TO WS-RESULT
           END-EVALUATE.
           MOVE "FALSE" TO WS-EXPECT.
           PERFORM CHECK-IT.
       C29-REF-MOD-SUBJECT.
           MOVE "control: reference-modified subject" TO WS-CASE.
           MOVE SPACES TO WS-RESULT.
           EVALUATE WS-STR(2:1)
               WHEN "A"   MOVE "A"     TO WS-RESULT
               WHEN "B"   MOVE "B"     TO WS-RESULT
               WHEN OTHER MOVE "OTHER" TO WS-RESULT
           END-EVALUATE.
           MOVE "B" TO WS-EXPECT.
           PERFORM CHECK-IT.
       CHECK-IT.
           ADD 1 TO TESTS-RUN.
           IF WS-RESULT = WS-EXPECT
               ADD 1 TO TESTS-PASSED
               DISPLAY "  PASS  " WS-CASE
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "  FAIL  " WS-CASE
               DISPLAY "        expected [" WS-EXPECT
                   "] got [" WS-RESULT "]"
           END-IF.

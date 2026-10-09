       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-DATA-ITEM-METHODS
      *> The value methods of an ordinary data item (a PIC X field,
      *> a group, a table occurrence, a reference-modified slice):
      *>   Trim · UpperCase / ToUpperCase / Upper
      *>   LowerCase / ToLowerCase / Lower · Replace
      *>   Len / Length (method and bare property)
      *>   Split(sep) · Split(sep)(n) · chains of them.
      *> Two worked examples of each, in every spelling the language
      *> accepts. Self-checking: a case prints only if it FAILS; one
      *> result block at the end lists every form and its tally.
      *> ============================================================
       PROGRAM-ID. TEST-DATA-ITEM-METHODS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-A                    PIC X(20) VALUE SPACES.
       01  WS-S5                   PIC X(5)  VALUE "ab".
       01  WS-S8                   PIC X(8)  VALUE "abc".
       01  WS-CSV                  PIC X(30) VALUE SPACES.
       01  WS-RES                  PIC X(40) VALUE SPACES.
       01  WS-EXP                  PIC X(40) VALUE SPACES.
       01  WS-N                    PIC 9(4)  VALUE 0.
       01  WS-EXPN                 PIC 9(4)  VALUE 0.
       01  WS-LABEL                PIC X(40) VALUE SPACES.
       01  WS-GI                   PIC 99    VALUE 1.
       01  WS-I                    PIC 99    VALUE 1.
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0.
       01  WS-GROUPS.
           05 WS-G OCCURS 16 TIMES.
              10 WS-G-NAME         PIC X(26).
              10 WS-G-RUN          PIC 9(3) VALUE 0.
              10 WS-G-OK           PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM INIT-NAMES.
           MOVE 1 TO WS-GI.
           MOVE "   Hello   " TO WS-A.
           MOVE "Trim #1" TO WS-LABEL.
           MOVE "Hello" TO WS-EXP.
           MOVE WS-A::Trim() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "  a b  " TO WS-A.
           MOVE "Trim #2 (inner blank kept)" TO WS-LABEL.
           MOVE "a b" TO WS-EXP.
           MOVE WS-A::Trim() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 2 TO WS-GI.
           MOVE "abc def" TO WS-A.
           MOVE "UpperCase #1" TO WS-LABEL.
           MOVE "ABC DEF" TO WS-EXP.
           MOVE WS-A::UpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "MiXeD 123" TO WS-A.
           MOVE "UpperCase #2" TO WS-LABEL.
           MOVE "MIXED 123" TO WS-EXP.
           MOVE WS-A::UpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 3 TO WS-GI.
           MOVE "rust" TO WS-A.
           MOVE "ToUpperCase #1" TO WS-LABEL.
           MOVE "RUST" TO WS-EXP.
           MOVE WS-A::ToUpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "cobol-85" TO WS-A.
           MOVE "ToUpperCase #2" TO WS-LABEL.
           MOVE "COBOL-85" TO WS-EXP.
           MOVE WS-A::ToUpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 4 TO WS-GI.
           MOVE "hello" TO WS-A.
           MOVE "Upper #1" TO WS-LABEL.
           MOVE "HELLO" TO WS-EXP.
           MOVE WS-A::Upper() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "a1b2" TO WS-A.
           MOVE "Upper #2" TO WS-LABEL.
           MOVE "A1B2" TO WS-EXP.
           MOVE WS-A::Upper() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 5 TO WS-GI.
           MOVE "ABC DEF" TO WS-A.
           MOVE "LowerCase #1" TO WS-LABEL.
           MOVE "abc def" TO WS-EXP.
           MOVE WS-A::LowerCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "MiXeD 123" TO WS-A.
           MOVE "LowerCase #2" TO WS-LABEL.
           MOVE "mixed 123" TO WS-EXP.
           MOVE WS-A::LowerCase() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 6 TO WS-GI.
           MOVE "RUST" TO WS-A.
           MOVE "ToLowerCase #1" TO WS-LABEL.
           MOVE "rust" TO WS-EXP.
           MOVE WS-A::ToLowerCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "COBOL-85" TO WS-A.
           MOVE "ToLowerCase #2" TO WS-LABEL.
           MOVE "cobol-85" TO WS-EXP.
           MOVE WS-A::ToLowerCase() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 7 TO WS-GI.
           MOVE "HELLO" TO WS-A.
           MOVE "Lower #1" TO WS-LABEL.
           MOVE "hello" TO WS-EXP.
           MOVE WS-A::Lower() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "A1B2" TO WS-A.
           MOVE "Lower #2" TO WS-LABEL.
           MOVE "a1b2" TO WS-EXP.
           MOVE WS-A::Lower() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 8 TO WS-GI.
           MOVE "Hello World" TO WS-A.
           MOVE "Replace #1 (word)" TO WS-LABEL.
           MOVE "Hello COBOL" TO WS-EXP.
           MOVE WS-A::Replace("World" "COBOL") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "a-b-c" TO WS-A.
           MOVE "Replace #2 (every hyphen)" TO WS-LABEL.
           MOVE "a b c" TO WS-EXP.
           MOVE WS-A::Replace("-" " ") TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 9 TO WS-GI.
           MOVE "hello" TO WS-A.
           MOVE "Len #1 (PIC X(20))" TO WS-LABEL.
           MOVE 20 TO WS-EXPN.
           COMPUTE WS-N = WS-A::Len().
           PERFORM CHECK-NUM.
           MOVE "x" TO WS-CSV.
           MOVE "Len #2 (PIC X(30))" TO WS-LABEL.
           MOVE 30 TO WS-EXPN.
           COMPUTE WS-N = WS-CSV::Len().
           PERFORM CHECK-NUM.

           MOVE 10 TO WS-GI.
           MOVE "Length #1 (PIC X(5))" TO WS-LABEL.
           MOVE 5 TO WS-EXPN.
           COMPUTE WS-N = WS-S5::Length().
           PERFORM CHECK-NUM.
           MOVE "Length #2 (PIC X(8))" TO WS-LABEL.
           MOVE 8 TO WS-EXPN.
           COMPUTE WS-N = WS-S8::Length().
           PERFORM CHECK-NUM.

           MOVE 11 TO WS-GI.
           MOVE "x" TO WS-A.
           MOVE "Length property #1 (X(20))" TO WS-LABEL.
           MOVE 20 TO WS-EXPN.
           COMPUTE WS-N = WS-A::Length.
           PERFORM CHECK-NUM.
           MOVE "Length property #2 (X(5))" TO WS-LABEL.
           MOVE 5 TO WS-EXPN.
           COMPUTE WS-N = WS-S5::Length.
           PERFORM CHECK-NUM.

           MOVE 12 TO WS-GI.
           MOVE "a-b-c" TO WS-CSV.
           MOVE "Split #1 (first of a-b-c)" TO WS-LABEL.
           MOVE "a" TO WS-EXP.
           MOVE WS-CSV::Split("-") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "2024-10-09" TO WS-CSV.
           MOVE "Split #2 (year of a date)" TO WS-LABEL.
           MOVE "2024" TO WS-EXP.
           MOVE WS-CSV::Split("-") TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 13 TO WS-GI.
           MOVE "a-b-c" TO WS-CSV.
           MOVE "Split(n) #1 (2nd of a-b-c)" TO WS-LABEL.
           MOVE "b" TO WS-EXP.
           MOVE WS-CSV::Split("-")(2) TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "2024-10-09" TO WS-CSV.
           MOVE "Split(n) #2 (day of a date)" TO WS-LABEL.
           MOVE "09" TO WS-EXP.
           MOVE WS-CSV::Split("-")(3) TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 14 TO WS-GI.
           MOVE "  hi there  " TO WS-A.
           MOVE "Chain #1 Trim()::UpperCase()" TO WS-LABEL.
           MOVE "HI THERE" TO WS-EXP.
           MOVE WS-A::Trim()::UpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "  hi  " TO WS-A.
           MOVE "Chain #2 Trim()::Len()" TO WS-LABEL.
           MOVE 2 TO WS-EXPN.
           COMPUTE WS-N = WS-A::Trim()::Len().
           PERFORM CHECK-NUM.

           MOVE 15 TO WS-GI.
           MOVE "abc-def" TO WS-A.
           MOVE "Slice #1 (5:3)::UpperCase()" TO WS-LABEL.
           MOVE "DEF" TO WS-EXP.
           MOVE WS-A(5:3)::UpperCase() TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "  pad  " TO WS-A.
           MOVE "Slice #2 (1:7)::Trim()" TO WS-LABEL.
           MOVE "pad" TO WS-EXP.
           MOVE WS-A(1:7)::Trim() TO WS-RES.
           PERFORM CHECK-TEXT.

           MOVE 16 TO WS-GI.
           MOVE "  yes " TO WS-A.
           MOVE "Condition #1 IF Trim() = ..." TO WS-LABEL.
           MOVE "ok" TO WS-EXP.
           MOVE "no" TO WS-RES.
           IF WS-A::Trim() = "yes"
               MOVE "ok" TO WS-RES
           END-IF.
           PERFORM CHECK-TEXT.
           MOVE "YES" TO WS-A.
           MOVE "Condition #2 IF Lower() = ..." TO WS-LABEL.
           MOVE "ok" TO WS-EXP.
           MOVE "no" TO WS-RES.
           IF WS-A::Lower() = "yes"
               MOVE "ok" TO WS-RES
           END-IF.
           PERFORM CHECK-TEXT.

           PERFORM SHOW-RESULTS.
           STOP RUN.

       INIT-NAMES.
           MOVE "Trim()"                  TO WS-G-NAME (1).
           MOVE "UpperCase()"             TO WS-G-NAME (2).
           MOVE "ToUpperCase()"           TO WS-G-NAME (3).
           MOVE "Upper()"                 TO WS-G-NAME (4).
           MOVE "LowerCase()"             TO WS-G-NAME (5).
           MOVE "ToLowerCase()"           TO WS-G-NAME (6).
           MOVE "Lower()"                 TO WS-G-NAME (7).
           MOVE "Replace(from, to)"       TO WS-G-NAME (8).
           MOVE "Len()"                   TO WS-G-NAME (9).
           MOVE "Length()"                TO WS-G-NAME (10).
           MOVE "::Length (property)"     TO WS-G-NAME (11).
           MOVE "Split(sep)"              TO WS-G-NAME (12).
           MOVE "Split(sep)(n)"           TO WS-G-NAME (13).
           MOVE "chains of methods"       TO WS-G-NAME (14).
           MOVE "reference-modified slice" TO WS-G-NAME (15).
           MOVE "inside a condition"      TO WS-G-NAME (16).

       CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (WS-GI).
           IF WS-RES = WS-EXP
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (WS-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      [" WS-RES "]"
               DISPLAY "  expected [" WS-EXP "]"
           END-IF.

       CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (WS-GI).
           IF WS-N = WS-EXPN
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (WS-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      " WS-N
               DISPLAY "  expected " WS-EXPN
           END-IF.

       SHOW-RESULTS.
           DISPLAY "============================================".
           DISPLAY "TEST-DATA-ITEM-METHODS".
           DISPLAY "Value methods of a data item, two examples of".
           DISPLAY "each form (passed / run):".
           DISPLAY "--------------------------------------------".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 16
               DISPLAY "  " WS-G-NAME (WS-I) " "
                   WS-G-OK (WS-I) " / " WS-G-RUN (WS-I)
           END-PERFORM.
           DISPLAY "--------------------------------------------".
           DISPLAY "FORMS EXERCISED : 16".
           DISPLAY "TESTS RUN       : " WS-TOT-RUN.
           DISPLAY "TESTS PASSED    : " WS-TOT-OK.
           DISPLAY "TESTS FAILED    : " WS-TOT-BAD.
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".

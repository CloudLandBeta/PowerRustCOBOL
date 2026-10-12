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
      *>
      *> The same eleven names (thirteen forms) are then run from
      *> CONTAINED programs - a form's event handlers are such
      *> programs, each with its own WORKING-STORAGE - in the three
      *> situations that decide where the item is stored:
      *>   17  an item only that contained program declares
      *>   18  one name declared by TWO contained programs (SUB-DUP-1
      *>       and SUB-DUP-2, with different values and widths)
      *>   19  an item the outermost program declares GLOBAL
      *> They report into the outermost program's GLOBAL tally.
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
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0 GLOBAL.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0 GLOBAL.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0 GLOBAL.
       01  WS-GLB-A                PIC X(20) VALUE SPACES GLOBAL.
       01  WS-GLB-CSV              PIC X(30) VALUE SPACES GLOBAL.
       01  WS-GLB-S5               PIC X(5)  VALUE "ab" GLOBAL.
       01  WS-GLB-S8               PIC X(8)  VALUE "abc" GLOBAL.
       01  WS-GROUPS GLOBAL.
           05 WS-G OCCURS 19 TIMES.
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

           CALL "SUB-UNIQUE".
           CALL "SUB-DUP-1".
           CALL "SUB-DUP-2".
           CALL "SUB-GLOBAL".

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
           MOVE "contained: unique name"  TO WS-G-NAME (17).
           MOVE "contained: same name x2" TO WS-G-NAME (18).
           MOVE "contained: outer GLOBAL" TO WS-G-NAME (19).

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
           DISPLAY "each form (passed / run); rows 17-19 run all".
           DISPLAY "13 forms from contained programs:".
           DISPLAY "--------------------------------------------".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 19
               DISPLAY "  " WS-G-NAME (WS-I) " "
                   WS-G-OK (WS-I) " / " WS-G-RUN (WS-I)
           END-PERFORM.
           DISPLAY "--------------------------------------------".
           DISPLAY "FORMS EXERCISED : 19".
           DISPLAY "TESTS RUN       : " WS-TOT-RUN.
           DISPLAY "TESTS PASSED    : " WS-TOT-OK.
           DISPLAY "TESTS FAILED    : " WS-TOT-BAD.
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".

      *> ------------------------------------------------------------
      *> Situation 17 - an item ONLY this contained program declares.
      *> It is stored under this program's own key, not under its
      *> bare name, so a lookup of the bare name finds nothing.
      *> ------------------------------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-UNIQUE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  UN-A                    PIC X(20) VALUE SPACES.
       01  UN-CSV                  PIC X(30) VALUE SPACES.
       01  UN-S5                   PIC X(5)  VALUE "ab".
       01  UN-S8                   PIC X(8)  VALUE "abc".
       01  UN-RES                  PIC X(40) VALUE SPACES.
       01  UN-EXP                  PIC X(40) VALUE SPACES.
       01  UN-N                    PIC 9(4)  VALUE 0.
       01  UN-EXPN                 PIC 9(4)  VALUE 0.
       01  UN-LABEL                PIC X(40) VALUE SPACES.
       01  UN-GI                   PIC 99    VALUE 17.
       PROCEDURE DIVISION.
       UN-RUN.
           MOVE "Trim()" TO UN-LABEL.
           MOVE "   Hello   " TO UN-A.
           MOVE "Hello" TO UN-EXP.
           MOVE UN-A::Trim() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "UpperCase()" TO UN-LABEL.
           MOVE "abc def" TO UN-A.
           MOVE "ABC DEF" TO UN-EXP.
           MOVE UN-A::UpperCase() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "ToUpperCase()" TO UN-LABEL.
           MOVE "rust" TO UN-A.
           MOVE "RUST" TO UN-EXP.
           MOVE UN-A::ToUpperCase() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "Upper()" TO UN-LABEL.
           MOVE "hello" TO UN-A.
           MOVE "HELLO" TO UN-EXP.
           MOVE UN-A::Upper() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "LowerCase()" TO UN-LABEL.
           MOVE "ABC DEF" TO UN-A.
           MOVE "abc def" TO UN-EXP.
           MOVE UN-A::LowerCase() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "ToLowerCase()" TO UN-LABEL.
           MOVE "RUST" TO UN-A.
           MOVE "rust" TO UN-EXP.
           MOVE UN-A::ToLowerCase() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "Lower()" TO UN-LABEL.
           MOVE "HELLO" TO UN-A.
           MOVE "hello" TO UN-EXP.
           MOVE UN-A::Lower() TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "Replace(from, to)" TO UN-LABEL.
           MOVE "Hello World" TO UN-A.
           MOVE "Hello COBOL" TO UN-EXP.
           MOVE UN-A::Replace("World" "COBOL") TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "Len()" TO UN-LABEL.
           MOVE 20 TO UN-EXPN.
           COMPUTE UN-N = UN-A::Len().
           PERFORM UN-CHECK-NUM.
           MOVE "Length()" TO UN-LABEL.
           MOVE 5 TO UN-EXPN.
           COMPUTE UN-N = UN-S5::Length().
           PERFORM UN-CHECK-NUM.
           MOVE "::Length (property)" TO UN-LABEL.
           MOVE 8 TO UN-EXPN.
           COMPUTE UN-N = UN-S8::Length.
           PERFORM UN-CHECK-NUM.
           MOVE "Split(sep)" TO UN-LABEL.
           MOVE "a-b-c" TO UN-CSV.
           MOVE "a" TO UN-EXP.
           MOVE UN-CSV::Split("-") TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           MOVE "Split(sep)(n)" TO UN-LABEL.
           MOVE "2024-10-09" TO UN-CSV.
           MOVE "09" TO UN-EXP.
           MOVE UN-CSV::Split("-")(3) TO UN-RES.
           PERFORM UN-CHECK-TEXT.
           GOBACK.

       UN-CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (UN-GI).
           IF UN-RES = UN-EXP
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (UN-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-UNIQUE " UN-LABEL
               DISPLAY "  got      [" UN-RES "]"
               DISPLAY "  expected [" UN-EXP "]"
           END-IF.

       UN-CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (UN-GI).
           IF UN-N = UN-EXPN
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (UN-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-UNIQUE " UN-LABEL
               DISPLAY "  got      " UN-N
               DISPLAY "  expected " UN-EXPN
           END-IF.
       END PROGRAM SUB-UNIQUE.

      *> ------------------------------------------------------------
      *> Situation 18 - ONE NAME DECLARED BY TWO CONTAINED PROGRAMS.
      *> SUB-DUP-1 and SUB-DUP-2 declare DP-A ... DP-GI alike, with
      *> different values here and DIFFERENT WIDTHS in SUB-DUP-2, so
      *> an item read from the wrong program's storage gives itself
      *> away (Len / Length answer the width).
      *> ------------------------------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-DUP-1.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  DP-A                    PIC X(20) VALUE SPACES.
       01  DP-CSV                  PIC X(30) VALUE SPACES.
       01  DP-S5                   PIC X(5)  VALUE "ab".
       01  DP-S8                   PIC X(8)  VALUE "abc".
       01  DP-RES                  PIC X(40) VALUE SPACES.
       01  DP-EXP                  PIC X(40) VALUE SPACES.
       01  DP-N                    PIC 9(4)  VALUE 0.
       01  DP-EXPN                 PIC 9(4)  VALUE 0.
       01  DP-LABEL                PIC X(40) VALUE SPACES.
       01  DP-GI                   PIC 99    VALUE 18.
       PROCEDURE DIVISION.
       D1-RUN.
           MOVE "Trim()" TO DP-LABEL.
           MOVE "   Hello   " TO DP-A.
           MOVE "Hello" TO DP-EXP.
           MOVE DP-A::Trim() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "UpperCase()" TO DP-LABEL.
           MOVE "abc def" TO DP-A.
           MOVE "ABC DEF" TO DP-EXP.
           MOVE DP-A::UpperCase() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "ToUpperCase()" TO DP-LABEL.
           MOVE "rust" TO DP-A.
           MOVE "RUST" TO DP-EXP.
           MOVE DP-A::ToUpperCase() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "Upper()" TO DP-LABEL.
           MOVE "hello" TO DP-A.
           MOVE "HELLO" TO DP-EXP.
           MOVE DP-A::Upper() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "LowerCase()" TO DP-LABEL.
           MOVE "ABC DEF" TO DP-A.
           MOVE "abc def" TO DP-EXP.
           MOVE DP-A::LowerCase() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "ToLowerCase()" TO DP-LABEL.
           MOVE "RUST" TO DP-A.
           MOVE "rust" TO DP-EXP.
           MOVE DP-A::ToLowerCase() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "Lower()" TO DP-LABEL.
           MOVE "HELLO" TO DP-A.
           MOVE "hello" TO DP-EXP.
           MOVE DP-A::Lower() TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "Replace(from, to)" TO DP-LABEL.
           MOVE "Hello World" TO DP-A.
           MOVE "Hello COBOL" TO DP-EXP.
           MOVE DP-A::Replace("World" "COBOL") TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "Len()" TO DP-LABEL.
           MOVE 20 TO DP-EXPN.
           COMPUTE DP-N = DP-A::Len().
           PERFORM D1-CHECK-NUM.
           MOVE "Length()" TO DP-LABEL.
           MOVE 5 TO DP-EXPN.
           COMPUTE DP-N = DP-S5::Length().
           PERFORM D1-CHECK-NUM.
           MOVE "::Length (property)" TO DP-LABEL.
           MOVE 8 TO DP-EXPN.
           COMPUTE DP-N = DP-S8::Length.
           PERFORM D1-CHECK-NUM.
           MOVE "Split(sep)" TO DP-LABEL.
           MOVE "a-b-c" TO DP-CSV.
           MOVE "a" TO DP-EXP.
           MOVE DP-CSV::Split("-") TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           MOVE "Split(sep)(n)" TO DP-LABEL.
           MOVE "2024-10-09" TO DP-CSV.
           MOVE "09" TO DP-EXP.
           MOVE DP-CSV::Split("-")(3) TO DP-RES.
           PERFORM D1-CHECK-TEXT.
           GOBACK.

       D1-CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (DP-GI).
           IF DP-RES = DP-EXP
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (DP-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-DUP-1 " DP-LABEL
               DISPLAY "  got      [" DP-RES "]"
               DISPLAY "  expected [" DP-EXP "]"
           END-IF.

       D1-CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (DP-GI).
           IF DP-N = DP-EXPN
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (DP-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-DUP-1 " DP-LABEL
               DISPLAY "  got      " DP-N
               DISPLAY "  expected " DP-EXPN
           END-IF.
       END PROGRAM SUB-DUP-1.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-DUP-2.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  DP-A                    PIC X(12) VALUE SPACES.
       01  DP-CSV                  PIC X(14) VALUE SPACES.
       01  DP-S5                   PIC X(6)  VALUE "abcd".
       01  DP-S8                   PIC X(9)  VALUE "abcde".
       01  DP-RES                  PIC X(40) VALUE SPACES.
       01  DP-EXP                  PIC X(40) VALUE SPACES.
       01  DP-N                    PIC 9(4)  VALUE 0.
       01  DP-EXPN                 PIC 9(4)  VALUE 0.
       01  DP-LABEL                PIC X(40) VALUE SPACES.
       01  DP-GI                   PIC 99    VALUE 18.
       PROCEDURE DIVISION.
       D2-RUN.
           MOVE "Trim()" TO DP-LABEL.
           MOVE "  World  " TO DP-A.
           MOVE "World" TO DP-EXP.
           MOVE DP-A::Trim() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "UpperCase()" TO DP-LABEL.
           MOVE "xyz uvw" TO DP-A.
           MOVE "XYZ UVW" TO DP-EXP.
           MOVE DP-A::UpperCase() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "ToUpperCase()" TO DP-LABEL.
           MOVE "go" TO DP-A.
           MOVE "GO" TO DP-EXP.
           MOVE DP-A::ToUpperCase() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "Upper()" TO DP-LABEL.
           MOVE "world" TO DP-A.
           MOVE "WORLD" TO DP-EXP.
           MOVE DP-A::Upper() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "LowerCase()" TO DP-LABEL.
           MOVE "XYZ UVW" TO DP-A.
           MOVE "xyz uvw" TO DP-EXP.
           MOVE DP-A::LowerCase() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "ToLowerCase()" TO DP-LABEL.
           MOVE "GO" TO DP-A.
           MOVE "go" TO DP-EXP.
           MOVE DP-A::ToLowerCase() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "Lower()" TO DP-LABEL.
           MOVE "WORLD" TO DP-A.
           MOVE "world" TO DP-EXP.
           MOVE DP-A::Lower() TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "Replace(from, to)" TO DP-LABEL.
           MOVE "Hello World" TO DP-A.
           MOVE "Bye World" TO DP-EXP.
           MOVE DP-A::Replace("Hello" "Bye") TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "Len()" TO DP-LABEL.
           MOVE 12 TO DP-EXPN.
           COMPUTE DP-N = DP-A::Len().
           PERFORM D2-CHECK-NUM.
           MOVE "Length()" TO DP-LABEL.
           MOVE 6 TO DP-EXPN.
           COMPUTE DP-N = DP-S5::Length().
           PERFORM D2-CHECK-NUM.
           MOVE "::Length (property)" TO DP-LABEL.
           MOVE 9 TO DP-EXPN.
           COMPUTE DP-N = DP-S8::Length.
           PERFORM D2-CHECK-NUM.
           MOVE "Split(sep)" TO DP-LABEL.
           MOVE "p-q-r" TO DP-CSV.
           MOVE "p" TO DP-EXP.
           MOVE DP-CSV::Split("-") TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           MOVE "Split(sep)(n)" TO DP-LABEL.
           MOVE "1999-12-31" TO DP-CSV.
           MOVE "31" TO DP-EXP.
           MOVE DP-CSV::Split("-")(3) TO DP-RES.
           PERFORM D2-CHECK-TEXT.
           GOBACK.

       D2-CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (DP-GI).
           IF DP-RES = DP-EXP
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (DP-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-DUP-2 " DP-LABEL
               DISPLAY "  got      [" DP-RES "]"
               DISPLAY "  expected [" DP-EXP "]"
           END-IF.

       D2-CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (DP-GI).
           IF DP-N = DP-EXPN
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (DP-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-DUP-2 " DP-LABEL
               DISPLAY "  got      " DP-N
               DISPLAY "  expected " DP-EXPN
           END-IF.
       END PROGRAM SUB-DUP-2.

      *> ------------------------------------------------------------
      *> Situation 19 - an item the OUTERMOST program declares GLOBAL
      *> (WS-GLB-*), read from a contained program. This already
      *> worked; it is here so it keeps working.
      *> ------------------------------------------------------------
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUB-GLOBAL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  GL-RES                  PIC X(40) VALUE SPACES.
       01  GL-EXP                  PIC X(40) VALUE SPACES.
       01  GL-N                    PIC 9(4)  VALUE 0.
       01  GL-EXPN                 PIC 9(4)  VALUE 0.
       01  GL-LABEL                PIC X(40) VALUE SPACES.
       01  GL-GI                   PIC 99    VALUE 19.
       PROCEDURE DIVISION.
       GL-RUN.
           MOVE "Trim()" TO GL-LABEL.
           MOVE "   Hello   " TO WS-GLB-A.
           MOVE "Hello" TO GL-EXP.
           MOVE WS-GLB-A::Trim() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "UpperCase()" TO GL-LABEL.
           MOVE "abc def" TO WS-GLB-A.
           MOVE "ABC DEF" TO GL-EXP.
           MOVE WS-GLB-A::UpperCase() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "ToUpperCase()" TO GL-LABEL.
           MOVE "rust" TO WS-GLB-A.
           MOVE "RUST" TO GL-EXP.
           MOVE WS-GLB-A::ToUpperCase() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "Upper()" TO GL-LABEL.
           MOVE "hello" TO WS-GLB-A.
           MOVE "HELLO" TO GL-EXP.
           MOVE WS-GLB-A::Upper() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "LowerCase()" TO GL-LABEL.
           MOVE "ABC DEF" TO WS-GLB-A.
           MOVE "abc def" TO GL-EXP.
           MOVE WS-GLB-A::LowerCase() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "ToLowerCase()" TO GL-LABEL.
           MOVE "RUST" TO WS-GLB-A.
           MOVE "rust" TO GL-EXP.
           MOVE WS-GLB-A::ToLowerCase() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "Lower()" TO GL-LABEL.
           MOVE "HELLO" TO WS-GLB-A.
           MOVE "hello" TO GL-EXP.
           MOVE WS-GLB-A::Lower() TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "Replace(from, to)" TO GL-LABEL.
           MOVE "Hello World" TO WS-GLB-A.
           MOVE "Hello COBOL" TO GL-EXP.
           MOVE WS-GLB-A::Replace("World" "COBOL") TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "Len()" TO GL-LABEL.
           MOVE 20 TO GL-EXPN.
           COMPUTE GL-N = WS-GLB-A::Len().
           PERFORM GL-CHECK-NUM.
           MOVE "Length()" TO GL-LABEL.
           MOVE 5 TO GL-EXPN.
           COMPUTE GL-N = WS-GLB-S5::Length().
           PERFORM GL-CHECK-NUM.
           MOVE "::Length (property)" TO GL-LABEL.
           MOVE 8 TO GL-EXPN.
           COMPUTE GL-N = WS-GLB-S8::Length.
           PERFORM GL-CHECK-NUM.
           MOVE "Split(sep)" TO GL-LABEL.
           MOVE "a-b-c" TO WS-GLB-CSV.
           MOVE "a" TO GL-EXP.
           MOVE WS-GLB-CSV::Split("-") TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           MOVE "Split(sep)(n)" TO GL-LABEL.
           MOVE "2024-10-09" TO WS-GLB-CSV.
           MOVE "09" TO GL-EXP.
           MOVE WS-GLB-CSV::Split("-")(3) TO GL-RES.
           PERFORM GL-CHECK-TEXT.
           GOBACK.

       GL-CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (GL-GI).
           IF GL-RES = GL-EXP
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (GL-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-GLOBAL " GL-LABEL
               DISPLAY "  got      [" GL-RES "]"
               DISPLAY "  expected [" GL-EXP "]"
           END-IF.

       GL-CHECK-NUM.
           ADD 1 TO WS-TOT-RUN.
           ADD 1 TO WS-G-RUN (GL-GI).
           IF GL-N = GL-EXPN
               ADD 1 TO WS-TOT-OK
               ADD 1 TO WS-G-OK (GL-GI)
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL SUB-GLOBAL " GL-LABEL
               DISPLAY "  got      " GL-N
               DISPLAY "  expected " GL-EXPN
           END-IF.
       END PROGRAM SUB-GLOBAL.
       END PROGRAM TEST-DATA-ITEM-METHODS.

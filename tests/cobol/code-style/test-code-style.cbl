       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-CODE-STYLE
      *> The examples of the Developer's Guide section "Code style":
      *> the RustCOBOL extensions that remove the scratch fields of
      *> plain COBOL-85.
      *>   A  a control's property is a data item (type inference)
      *>   B  a method that returns a value is an expression
      *>   C  methods of an ordinary data item (value methods)
      *>   D  an expression where COBOL-85 wants an identifier
      *>   E  the built-ins, written inline: COBOL::"NAME" ( ... )
      *> Self-checking: a case prints only if it FAILS; one result
      *> block at the end lists every form and its tally.
      *> ============================================================
       PROGRAM-ID. TEST-CODE-STYLE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-N                    PIC 9(4)  VALUE 0.
       01  WS-RES                  PIC X(60) VALUE SPACES.
       01  WS-EXP                  PIC X(60) VALUE SPACES.
       01  WS-EXPN                 PIC 9(4)  VALUE 0.
       01  WS-LABEL                PIC X(40) VALUE SPACES.
       01  WS-GI                   PIC 99    VALUE 1.
       01  WS-I                    PIC 99    VALUE 1.
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0.
       01  WS-PERSON-NAME          PIC X(40) VALUE "Dr. House, MD".
       01  WS-TEXT                 PIC X(30) VALUE SPACES.
       01  WS-CNT                  PIC 9(4)  VALUE 0.
       01  WS-RECORD.
           05 WS-REC-ID            PIC 9(4).
           05 WS-REC-NAME          PIC X(20).
       01  WS-DB                   PIC S9(9) COMP-5.
       01  WS-ST                   PIC XX.
       01  WS-ROWS                 PIC 9(4).
       01  WS-VAL                  PIC X(20).
       01  WS-GROUPS.
           05 WS-G OCCURS 12 TIMES.
              10 WS-G-NAME         PIC X(40).
              10 WS-G-RUN          PIC 9(3) VALUE 0.
              10 WS-G-OK           PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM INIT-NAMES.

      *> ---- A. a property is a data item ------------------------
           MOVE 1 TO WS-GI.
           MOVE 17 TO PERSON::Age.
           MOVE "minor" TO WS-RES.
           IF PERSON::Age > 18
               MOVE "adult" TO WS-RES
           END-IF.
           MOVE "A1 IF on a property (17)" TO WS-LABEL.
           MOVE "minor" TO WS-EXP.
           PERFORM CHECK-TEXT.
           COMPUTE PERSON::Age = PERSON::Age + 2.
           MOVE "minor" TO WS-RES.
           IF PERSON::Age > 18
               MOVE "adult" TO WS-RES
           END-IF.
           MOVE "A2 COMPUTE a property, IF (19)" TO WS-LABEL.
           MOVE "adult" TO WS-EXP.
           PERFORM CHECK-TEXT.

           MOVE 2 TO WS-GI.
           ADD 1 TO PERSON::Age.
           MOVE "A3 ADD 1 TO a property" TO WS-LABEL.
           MOVE 20 TO WS-EXPN.
           MOVE PERSON::Age TO WS-N.
           PERFORM CHECK-NUM.
           PERFORM UNTIL PERSON::Age >= 25
               ADD 1 TO PERSON::Age
           END-PERFORM.
           MOVE "A4 PERFORM UNTIL a property" TO WS-LABEL.
           MOVE 25 TO WS-EXPN.
           MOVE PERSON::Age TO WS-N.
           PERFORM CHECK-NUM.

           MOVE 3 TO WS-GI.
           MOVE "Gregory House" TO PERSON::Name.
           MOVE PERSON::Name TO LBL-B::Caption.
           MOVE LBL-B::Caption TO WS-RES.
           MOVE "A5 property to property" TO WS-LABEL.
           MOVE "Gregory House" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE SPACES TO WS-RES.
           STRING "Name: " DELIMITED BY SIZE
                  PERSON::Name DELIMITED BY SIZE
               INTO WS-RES.
           MOVE "A6 STRING a property" TO WS-LABEL.
           MOVE "Name: Gregory House" TO WS-EXP.
           PERFORM CHECK-TEXT.

           MOVE 4 TO WS-GI.
           MOVE 7 TO PERSON::Age.
           MOVE "other" TO WS-RES.
           EVALUATE PERSON::Age
               WHEN 7 MOVE "seven" TO WS-RES
           END-EVALUATE.
           MOVE "A7 EVALUATE a property" TO WS-LABEL.
           MOVE "seven" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE TRUE TO CHK-1::Checked.
           MOVE "no" TO WS-RES.
           IF CHK-1::Checked = TRUE
               MOVE "yes" TO WS-RES
           END-IF.
           MOVE "A8 a Boolean property = TRUE" TO WS-LABEL.
           MOVE "yes" TO WS-EXP.
           PERFORM CHECK-TEXT.

      *> ---- B. a method that returns a value is an expression ---
           MOVE 5 TO WS-GI.
           INVOKE LST-1::AddItem("alpha").
           INVOKE LST-1::AddItem("beta").
           MOVE "no" TO WS-RES.
           IF LST-1::GetCount() > 1
               MOVE "yes" TO WS-RES
           END-IF.
           MOVE "B1 IF on GetCount()" TO WS-LABEL.
           MOVE "yes" TO WS-EXP.
           PERFORM CHECK-TEXT.
           COMPUTE WS-N = LST-1::GetCount() * 10.
           MOVE "B2 COMPUTE with GetCount()" TO WS-LABEL.
           MOVE 20 TO WS-EXPN.
           PERFORM CHECK-NUM.

           MOVE 6 TO WS-GI.
           MOVE "0042Gregory House" TO TXT-REC::Text.
           MOVE TXT-REC::GetText() TO WS-RECORD.
           MOVE WS-REC-NAME TO WS-RES.
           MOVE "B3 a record lands in a group (name)" TO WS-LABEL.
           MOVE "Gregory House" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE WS-REC-ID TO WS-N.
           MOVE "B4 a record lands in a group (id)" TO WS-LABEL.
           MOVE 42 TO WS-EXPN.
           PERFORM CHECK-NUM.

           MOVE 7 TO WS-GI.
           MOVE 40 TO NUD-AGE::Value.
           INVOKE NUD-AGE::Increment().
           MOVE "B5 INVOKE a method, read the property" TO WS-LABEL.
           MOVE NUD-AGE::Value TO WS-N.
           MOVE 41 TO WS-EXPN.
           PERFORM CHECK-NUM.
           COMPUTE WS-N = NUD-AGE::Value * 2.
           MOVE "B6 a property inside arithmetic" TO WS-LABEL.
           MOVE 82 TO WS-EXPN.
           PERFORM CHECK-NUM.

      *> ---- C. methods of an ordinary data item -----------------
           MOVE 8 TO WS-GI.
           MOVE WS-PERSON-NAME::Replace(", MD", ", M.D.")
               TO WS-PERSON-NAME.
           MOVE WS-PERSON-NAME TO WS-RES.
           MOVE "C1 Replace on a PIC X item" TO WS-LABEL.
           MOVE "Dr. House, M.D." TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE WS-PERSON-NAME::Trim()::UpperCase() TO WS-RES.
           MOVE "C2 a chain of methods" TO WS-LABEL.
           MOVE "DR. HOUSE, M.D." TO WS-EXP.
           PERFORM CHECK-TEXT.

           MOVE 9 TO WS-GI.
           MOVE PERSON::Name::UpperCase() TO WS-RES.
           MOVE "C3 a value method on a property" TO WS-LABEL.
           MOVE "GREGORY HOUSE" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE "no" TO WS-RES.
           IF PERSON::Name::Len() > 5
               MOVE "yes" TO WS-RES
           END-IF.
           MOVE "C4 Len() of a property in an IF" TO WS-LABEL.
           MOVE "yes" TO WS-EXP.
           PERFORM CHECK-TEXT.

      *> ---- D. an expression where COBOL-85 wants an item -------
           MOVE 10 TO WS-GI.
           MOVE 5 TO WS-N.
           MOVE WS-N * 2 TO WS-CNT.
           MOVE "D1 MOVE an expression" TO WS-LABEL.
           MOVE 10 TO WS-EXPN.
           MOVE WS-CNT TO WS-N.
           PERFORM CHECK-NUM.
           SET WS-CNT TO WS-CNT + 1.
           MOVE "D2 SET an item TO an expression" TO WS-LABEL.
           MOVE 11 TO WS-EXPN.
           MOVE WS-CNT TO WS-N.
           PERFORM CHECK-NUM.
           MOVE "  padded  " TO WS-TEXT.
           SET LBL-A::Caption TO WS-TEXT::Trim().
           MOVE LBL-A::Caption TO WS-RES.
           MOVE "D3 SET a property TO a method result" TO WS-LABEL.
           MOVE "padded" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE 5 TO WS-N.
           MOVE SPACES TO WS-RES.
           STRING WS-N * 3 DELIMITED BY SIZE
                  " items" DELIMITED BY SIZE
               INTO WS-RES.
           MOVE "D4 STRING an expression" TO WS-LABEL.
           MOVE "15 items" TO WS-EXP.
           PERFORM CHECK-TEXT.

      *> ---- E. the built-ins, written inline --------------------
           MOVE 11 TO WS-GI.
           COBOL::"OPEN-DB" ( ":memory:" WS-DB WS-ST ).
           COBOL::"EXEC-SQL" ( WS-DB "CREATE TABLE T(A TEXT)"
               WS-ROWS WS-ST ).
           COBOL::"EXEC-SQL" ( WS-DB "INSERT INTO T VALUES ('x')"
               WS-ROWS WS-ST ).
           COBOL::"EXEC-SQL" ( WS-DB "SELECT A FROM T" WS-ROWS WS-ST ).
           MOVE WS-ROWS TO WS-N.
           MOVE "E1 COBOL::OPEN-DB / EXEC-SQL rows" TO WS-LABEL.
           MOVE 1 TO WS-EXPN.
           PERFORM CHECK-NUM.
           COBOL::"FETCH-ROW" ( WS-DB 1 WS-VAL WS-ST ).
           MOVE WS-VAL TO WS-RES.
           MOVE "E2 COBOL::FETCH-ROW" TO WS-LABEL.
           MOVE "x" TO WS-EXP.
           PERFORM CHECK-TEXT.
           COBOL::"CLOSE-DB" ( WS-DB ).

           MOVE 12 TO WS-GI.
           MOVE "a,b" TO WS-TEXT.
           MOVE WS-TEXT::Replace(",", ";") TO WS-RES.
           MOVE "F1 commas between arguments" TO WS-LABEL.
           MOVE "a;b" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE WS-TEXT::Replace("," ";") TO WS-RES.
           MOVE "F2 no commas between arguments" TO WS-LABEL.
           MOVE "a;b" TO WS-EXP.
           PERFORM CHECK-TEXT.

           PERFORM SHOW-RESULTS.
           STOP RUN.

       INIT-NAMES.
           MOVE "A  property in IF / COMPUTE" TO WS-G-NAME (1).
           MOVE "A  property in ADD / PERFORM UNTIL" TO WS-G-NAME (2).
           MOVE "A  property to property, STRING" TO WS-G-NAME (3).
           MOVE "A  property in EVALUATE, TRUE/FALSE" TO WS-G-NAME (4).
           MOVE "B  GetCount() in IF / COMPUTE" TO WS-G-NAME (5).
           MOVE "B  a returned record into a group" TO WS-G-NAME (6).
           MOVE "B  INVOKE, property in arithmetic" TO WS-G-NAME (7).
           MOVE "C  value methods on a data item" TO WS-G-NAME (8).
           MOVE "C  value methods on a property" TO WS-G-NAME (9).
           MOVE "D  expressions in MOVE/SET/STRING" TO WS-G-NAME (10).
           MOVE "E  built-ins written inline" TO WS-G-NAME (11).
           MOVE "F  commas between arguments" TO WS-G-NAME (12).

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
           DISPLAY "TEST-CODE-STYLE".
           DISPLAY "The RustCOBOL extensions of the Code style".
           DISPLAY "section, by form (passed / run):".
           DISPLAY "--------------------------------------------".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 12
               DISPLAY "  " WS-G-NAME (WS-I) " "
                   WS-G-OK (WS-I) " / " WS-G-RUN (WS-I)
           END-PERFORM.
           DISPLAY "--------------------------------------------".
           DISPLAY "FORMS EXERCISED : 12".
           DISPLAY "TESTS RUN       : " WS-TOT-RUN.
           DISPLAY "TESTS PASSED    : " WS-TOT-OK.
           DISPLAY "TESTS FAILED    : " WS-TOT-BAD.
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".

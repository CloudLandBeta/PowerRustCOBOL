       IDENTIFICATION DIVISION.
       PROGRAM-ID. TEST-LAYOUT-STYLE.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-NL PIC X.
       01 WS-RES PIC X(40) VALUE SPACES.
       01 WS-NAME PIC X(40) VALUE "Dr. House, MD".
       01 WS-N PIC 9(4) VALUE 0.
       01 WS-CNT PIC 9(4) VALUE 0.
       01 WS-RUN PIC 9(4) VALUE 0.
       01 WS-FAILS PIC 9(4) VALUE 0.

       PROCEDURE DIVISION.
           *> The "layout" rule of the plugin's golden rules, run. Case 1: a
           *> property is set from a literal and read back into a plain item
           *> and into another property, all with SET and no scratch field.

           SET     PERSON::Name                  TO    "Gregory House"
           SET     PERSON::Age                   TO    17
           SET     LBL-B::Caption                TO    PERSON::Name
           SET     WS-RES                        TO    LBL-B::Caption

           ADD 1 TO WS-RUN

           IF WS-RES NOT = "Gregory House"
               ADD 1 TO WS-FAILS
           END-IF

           *> Case 2: SET takes an intrinsic function, so a newline needs no
           *> MOVE.

           SET     WS-NL                         TO    FUNCTION CHAR(11)

           ADD 1 TO WS-RUN

           IF WS-NL = SPACE
               ADD 1 TO WS-FAILS
           END-IF

           *> Case 3: SET takes an expression, so counting needs no
           *> temporary item. Case 4: SET takes a value method.

           SET     WS-CNT                        TO    PERSON::Age * 2
           SET     WS-NAME                       TO    WS-NAME::Replace(", MD", ", M.D.")

           ADD 2 TO WS-RUN

           IF WS-CNT NOT = 34
               ADD 1 TO WS-FAILS
           END-IF

           IF WS-NAME NOT = "Dr. House, M.D."
               ADD 1 TO WS-FAILS
           END-IF

           *> Case 5: a method is a statement of its own, with no INVOKE. The
           *> answer of GetCount() is used where the test needs it.

           LST-1::AddItem("alpha")
           LST-1::AddItem("beta")

           ADD 1 TO WS-RUN

           IF LST-1::GetCount() NOT = 2
               ADD 1 TO WS-FAILS
           END-IF

           *> Case 6: the operand before TO is too long to reach column 50, so
           *> TO goes on the next line and a blank line follows the statement.

           SET     LBL-CUSTOMER-STATUS-LINE-MAIN::Caption
                                                 TO    "Ready"

           ADD 1 TO WS-RUN

           IF LBL-CUSTOMER-STATUS-LINE-MAIN::Caption NOT = "Ready"
               ADD 1 TO WS-FAILS
           END-IF

           *> Case 7: count up to the limit; the loop ends when the counter
           *> reaches it, and its scope is closed by END-PERFORM, not a period.

           PERFORM UNTIL WS-N >= 3
               ADD 1 TO WS-N
           END-PERFORM

           ADD 1 TO WS-RUN

           IF WS-N NOT = 3
               ADD 1 TO WS-FAILS
           END-IF

           PERFORM SHOW-RESULT
           STOP RUN.

       SHOW-RESULT.
           DISPLAY "============================================================"
           DISPLAY "TEST-LAYOUT-STYLE"
           DISPLAY "  1  SET a property from a literal, and an item from a property"
           DISPLAY "  2  SET an item from an intrinsic function"
           DISPLAY "  3  SET an item from an expression"
           DISPLAY "  4  SET an item from a value method (Replace)"
           DISPLAY "  5  a method called as a statement, no INVOKE"
           DISPLAY "  6  TO in column 50 of the next line, operand too long"
           DISPLAY "  7  PERFORM UNTIL closed by END-PERFORM"
           DISPLAY "TESTS RUN       : " WS-RUN
           DISPLAY "TESTS FAILED    : " WS-FAILS

           IF WS-FAILS = 0
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF

           DISPLAY "============================================================".

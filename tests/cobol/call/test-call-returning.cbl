       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-CALL-RETURNING
      *> CALL ... RETURNING identifier. The value a called program
      *> leaves in its PROCEDURE DIVISION ... RETURNING item is MOVEd
      *> into the caller's identifier: the receiver's PICTURE
      *> converts, edits and truncates. A call that returns no value
      *> (no RETURNING item, a paragraph, an unresolved program)
      *> leaves the identifier as it was.
      *> Forms exercised:
      *>   R01-R08  the receiver's PICTURE (9, 9V9, 99, edited, X, S9)
      *>   R09-R10  the callee's item in WORKING-STORAGE; a group item
      *>   R11-R13  receiver as table element, qualified, ref-modified
      *>   R14-R17  no RETURNING item; paragraph; unresolved program;
      *>            NOT ON EXCEPTION sees the value
      *>   R18-R20  static callee called 3 times; BY CONTENT +
      *>            BY REFERENCE + RETURNING; a chain of two calls
      *> Then a bulk phase: 20,000 calls with RETURNING, timed.
      *> Self-checking. One result block at the end, no per-call lines.
      *> ============================================================
       PROGRAM-ID. TEST-CALL-RETURNING.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  CASE-TABLE.
           05  CASE-ROW OCCURS 20 TIMES.
               10  CASE-ID         PIC X(4).
               10  CASE-FORM       PIC X(46).
               10  CASE-WANT       PIC X(12).
               10  CASE-GOT        PIC X(12).
               10  CASE-OK         PIC X(4).
       01  CASES-RUN               PIC 9(3)  VALUE 0.
       01  CASES-PASSED            PIC 9(3)  VALUE 0.
       01  CASES-FAILED            PIC 9(3)  VALUE 0.
       01  WS-ID                   PIC X(4)  VALUE SPACES.
       01  WS-FORM                 PIC X(46) VALUE SPACES.
       01  WS-WANT                 PIC X(12) VALUE SPACES.
       01  WS-GOT                  PIC X(12) VALUE SPACES.
       01  WS-I                    PIC 99    VALUE 0.
       01  WS-N                    PIC 9(4)  VALUE 21.
       01  R-WIDE                  PIC 9(6)V99 VALUE 0.
       01  R-FRAC                  PIC 9(3)V9  VALUE 0.
       01  R-INT                   PIC 99      VALUE 0.
       01  R-EDIT                  PIC ZZ9.99.
       01  R-COMMA                 PIC ZZ,ZZ9.99.
       01  R-TEXT                  PIC X(8)    VALUE SPACES.
       01  R-SIGNED                PIC S9(4)   VALUE 0.
       01  R-SIGN-EDIT             PIC -ZZZ9.
       01  R-FIVE                  PIC 9(5)    VALUE 0.
       01  R-TAG                   PIC X(6)    VALUE SPACES.
       01  R-REC.
           05  R-NAME              PIC X(5)    VALUE SPACES.
           05  R-QTY               PIC 9(3)    VALUE 0.
       01  R-TABLE.
           05  R-ELEM              PIC 9(5)    OCCURS 3 TIMES VALUE 0.
       01  R-BOX.
           05  R-FIELD             PIC 9(5)    VALUE 0.
       01  R-BUF                   PIC X(10)   VALUE "0123456789".
       01  R-KEEP                  PIC 9(3)    VALUE 77.
       01  R-FLAG                  PIC XX      VALUE SPACES.
       01  R-SEEN                  PIC 9(5)    VALUE 0.
       01  R-TICKET-1              PIC 9(3)    VALUE 0.
       01  R-TICKET-2              PIC 9(3)    VALUE 0.
       01  R-TICKET-3              PIC 9(3)    VALUE 0.
       01  R-IN                    PIC 9(3)    VALUE 5.
       01  R-OUT                   PIC 9(3)    VALUE 0.
       01  R-SUM                   PIC 9(4)    VALUE 0.
       01  R-CHAIN                 PIC 9(5)    VALUE 0.
       01  R-PARA                  PIC 9(3)    VALUE 31.
       01  BULK-COUNT              PIC 9(6)    VALUE 20000.
       01  BULK-I                  PIC 9(6)    VALUE 0.
       01  BULK-R                  PIC 9(9)    VALUE 0.
       01  BULK-SUM                PIC 9(12)   VALUE 0.
       01  BULK-WANT               PIC 9(12)   VALUE 0.
       01  BULK-N                  PIC 9(4)    VALUE 0.
       01  T-START                 PIC 9(8)    VALUE 0.
       01  T-END                   PIC 9(8)    VALUE 0.
       01  T-HH                    PIC 99      VALUE 0.
       01  T-MM                    PIC 99      VALUE 0.
       01  T-SS                    PIC 99      VALUE 0.
       01  T-HS                    PIC 99      VALUE 0.
       01  T-WORK                  PIC 9(8)    VALUE 0.
       01  T-REST                  PIC 9(8)    VALUE 0.
       01  T-START-CS              PIC 9(9)    VALUE 0.
       01  T-END-CS                PIC 9(9)    VALUE 0.
       01  T-ELAPSED-CS            PIC 9(9)    VALUE 0.
       01  T-ELAPSED-SEC           PIC 9(7)V99 VALUE 0.
       01  T-RATE                  PIC 9(9)    VALUE 0.
       01  T-ELAPSED-EDIT          PIC ZZZ,ZZ9.99.
       01  T-RATE-EDIT             PIC ZZZ,ZZZ,ZZ9.
       01  BULK-COUNT-EDIT         PIC ZZZ,ZZ9.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM PICTURE-CASES.
           PERFORM CALLEE-STORAGE-CASES.
           PERFORM RECEIVER-CASES.
           PERFORM NO-VALUE-CASES.
           PERFORM REPEAT-AND-COMBINE-CASES.
           PERFORM BULK-PHASE.
           PERFORM PRINT-RESULTS.
           STOP RUN.

      *> ---- R01-R08: the receiver's PICTURE decides ---------------
       PICTURE-CASES.
           MOVE 21 TO WS-N.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   WS-N = 21 (set just above; BY REFERENCE).
      *> Result: R-FIVE = 00042. WS-N is unchanged.

           CALL "DOUBLE" USING WS-N RETURNING R-FIVE.

           MOVE "R01" TO WS-ID.
           MOVE "RETURNING PIC 9(5) <- 21 * 2" TO WS-FORM.
           MOVE "00042" TO WS-WANT.
           MOVE R-FIVE TO WS-GOT.
           PERFORM RECORD-CASE.

      *> PRICE: returns 12.34 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-WIDE = 00001234 (PIC 9(6)V99).

           CALL "PRICE" RETURNING R-WIDE.

           MOVE "R02" TO WS-ID.
           MOVE "RETURNING PIC 9(6)V99 <- 12.34" TO WS-FORM.
           MOVE "00001234" TO WS-WANT.
           MOVE R-WIDE TO WS-GOT.
           PERFORM RECORD-CASE.

      *> PRICE: returns 12.34 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-FRAC = 0123 (PIC 9(3)V9: 12.3, fraction cut).

           CALL "PRICE" RETURNING R-FRAC.

           MOVE "R03" TO WS-ID.
           MOVE "RETURNING PIC 9(3)V9 <- 12.34" TO WS-FORM.
           MOVE "0123" TO WS-WANT.
           MOVE R-FRAC TO WS-GOT.
           PERFORM RECORD-CASE.

      *> PRICE: returns 12.34 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-INT = 12 (PIC 99: the fraction is dropped).

           CALL "PRICE" RETURNING R-INT.

           MOVE "R04" TO WS-ID.
           MOVE "RETURNING PIC 99 <- 12.34" TO WS-FORM.
           MOVE "12" TO WS-WANT.
           MOVE R-INT TO WS-GOT.
           PERFORM RECORD-CASE.

      *> PRICE: returns 12.34 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-EDIT = " 12.34" (PIC ZZ9.99: edited on arrival).

           CALL "PRICE" RETURNING R-EDIT.

           MOVE "R05" TO WS-ID.
           MOVE "RETURNING PIC ZZ9.99 <- 12.34" TO WS-FORM.
           MOVE " 12.34" TO WS-WANT.
           MOVE R-EDIT TO WS-GOT.
           PERFORM RECORD-CASE.

      *> BIG-PRICE: returns 1234.56 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-COMMA = " 1,234.56" (PIC ZZ,ZZ9.99).

           CALL "BIG-PRICE" RETURNING R-COMMA.

           MOVE "R06" TO WS-ID.
           MOVE "RETURNING PIC ZZ,ZZ9.99 <- 1234.56" TO WS-FORM.
           MOVE " 1,234.56" TO WS-WANT.
           MOVE R-COMMA TO WS-GOT.
           PERFORM RECORD-CASE.

      *> PRICE: returns 12.34 in a PIC 9(4)V99 item.
      *> Uses:   No data item.
      *> Result: R-TEXT = "001234  " (PIC X(8): the digits, left).

           CALL "PRICE" RETURNING R-TEXT.

           MOVE "R07" TO WS-ID.
           MOVE "RETURNING PIC X(8) <- 12.34" TO WS-FORM.
           MOVE "001234" TO WS-WANT.
           MOVE R-TEXT TO WS-GOT.
           PERFORM RECORD-CASE.

      *> NEGATIVE: returns -42 in a PIC S9(4) item.
      *> Uses:   No data item.
      *> Result: R-SIGNED = -42 (PIC S9(4)); R-SIGN-EDIT = "-  42"
      *>         (PIC -ZZZ9) when the receiver is the edited item.

           CALL "NEGATIVE" RETURNING R-SIGNED.
           CALL "NEGATIVE" RETURNING R-SIGN-EDIT.

           MOVE "R08" TO WS-ID.
           MOVE "RETURNING PIC S9(4) / -ZZZ9 <- -42" TO WS-FORM.
           MOVE "-42 / -  42" TO WS-WANT.
           IF R-SIGNED = -42
               MOVE "-42 /" TO WS-GOT
           ELSE
               MOVE "WRONG /" TO WS-GOT
           END-IF.
           MOVE R-SIGN-EDIT TO WS-GOT (7:5).
           PERFORM RECORD-CASE.

      *> ---- R09-R10: where the callee keeps its item -------------
       CALLEE-STORAGE-CASES.

      *> TAG: returns "ABC" in an item of its own WORKING-STORAGE.
      *> Uses:   No data item.
      *> Result: R-TAG = "ABC   ".

           CALL "TAG" RETURNING R-TAG.

           MOVE "R09" TO WS-ID.
           MOVE "callee item in WORKING-STORAGE" TO WS-FORM.
           MOVE "ABC" TO WS-WANT.
           MOVE R-TAG TO WS-GOT.
           PERFORM RECORD-CASE.

      *> MAKE-REC: returns a group (PIC X(5) + PIC 9(3)).
      *> Uses:   No data item.
      *> Result: R-NAME = "PEARS", R-QTY = 012.

           CALL "MAKE-REC" RETURNING R-REC.

           MOVE "R10" TO WS-ID.
           MOVE "RETURNING a group item" TO WS-FORM.
           MOVE "PEARS012" TO WS-WANT.
           MOVE R-REC TO WS-GOT.
           PERFORM RECORD-CASE.

      *> ---- R11-R13: the receiver can be any data reference ------
       RECEIVER-CASES.
           MOVE 4 TO WS-N.
           MOVE 2 TO WS-I.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   WS-N = 4 (set just above); WS-I = 2 (the subscript).
      *> Result: R-ELEM (2) = 00008; R-ELEM (1) and (3) unchanged.

           CALL "DOUBLE" USING WS-N RETURNING R-ELEM (WS-I).

           MOVE "R11" TO WS-ID.
           MOVE "RETURNING table element R-ELEM (2)" TO WS-FORM.
           MOVE "0/8/0" TO WS-WANT.
           IF R-ELEM (1) = 0 AND R-ELEM (2) = 8 AND R-ELEM (3) = 0
               MOVE "0/8/0" TO WS-GOT
           ELSE
               MOVE "WRONG" TO WS-GOT
           END-IF.
           PERFORM RECORD-CASE.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   WS-N = 4 (still).
      *> Result: R-FIELD OF R-BOX = 00008.

           CALL "DOUBLE" USING WS-N RETURNING R-FIELD OF R-BOX.

           MOVE "R12" TO WS-ID.
           MOVE "RETURNING qualified R-FIELD OF R-BOX" TO WS-FORM.
           MOVE "00008" TO WS-WANT.
           MOVE R-FIELD OF R-BOX TO WS-GOT.
           PERFORM RECORD-CASE.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   WS-N = 4 (still); R-BUF = "0123456789".
      *> Result: R-BUF (4:3) holds what MOVE of 00008 would put
      *>         there; the other seven bytes are untouched.

           CALL "DOUBLE" USING WS-N RETURNING R-BUF (4:3).

           MOVE "R13" TO WS-ID.
           MOVE "RETURNING ref-modified R-BUF (4:3)" TO WS-FORM.
           MOVE "012/789" TO WS-WANT.
           STRING R-BUF (1:3) "/" R-BUF (8:3) DELIMITED BY SIZE
               INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> ---- R14-R17: calls that return no value ------------------
       NO-VALUE-CASES.

      *> QUIET: has no RETURNING item; it adds 1 to its argument.
      *> Uses:   R-KEEP = 77 (the receiver; also the argument).
      *> Result: R-KEEP = 078 from the ADD through USING; the
      *>         RETURNING phrase moves nothing over it.

           CALL "QUIET" USING R-KEEP RETURNING R-FIVE.

           MOVE "R14" TO WS-ID.
           MOVE "callee with no RETURNING item" TO WS-FORM.
           MOVE "078/00042" TO WS-WANT.
           STRING R-KEEP "/" R-FIVE DELIMITED BY SIZE INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> HELPER: a paragraph of this program, not a program.
      *> Uses:   R-PARA = 031 (the receiver).
      *> Result: R-PARA = 031; a paragraph returns no value.

           CALL "HELPER" RETURNING R-PARA.

           MOVE "R15" TO WS-ID.
           MOVE "CALL of a paragraph" TO WS-FORM.
           MOVE "031" TO WS-WANT.
           MOVE R-PARA TO WS-GOT.
           PERFORM RECORD-CASE.

      *> NOSUCH: no program, paragraph or built-in has this name.
      *> Uses:   R-KEEP = 078 (the receiver); R-FLAG = SPACES.
      *> Result: R-FLAG = "EX" (ON EXCEPTION ran); R-KEEP = 078.

           CALL "NOSUCH" RETURNING R-KEEP
               ON EXCEPTION     MOVE "EX" TO R-FLAG
               NOT ON EXCEPTION MOVE "OK" TO R-FLAG
           END-CALL.

           MOVE "R16" TO WS-ID.
           MOVE "unresolved program: receiver untouched" TO WS-FORM.
           MOVE "EX/078" TO WS-WANT.
           STRING R-FLAG "/" R-KEEP DELIMITED BY SIZE INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   WS-N = 4 (still).
      *> Result: R-FIVE = 00008, and the NOT ON EXCEPTION branch has
      *>         already seen it: R-SEEN = 00008.

           CALL "DOUBLE" USING WS-N RETURNING R-FIVE
               NOT ON EXCEPTION MOVE R-FIVE TO R-SEEN
           END-CALL.

           MOVE "R17" TO WS-ID.
           MOVE "NOT ON EXCEPTION sees the value" TO WS-FORM.
           MOVE "00008" TO WS-WANT.
           MOVE R-SEEN TO WS-GOT.
           PERFORM RECORD-CASE.

      *> ---- R18-R20: repeated and combined calls ------------------
       REPEAT-AND-COMBINE-CASES.

      *> TICKET: a static program; each call returns the next number.
      *> Uses:   TICKET's own counter (starts at 0, kept between calls).
      *> Result: R-TICKET-1 = 001, R-TICKET-2 = 002, R-TICKET-3 = 003.

           CALL "TICKET" RETURNING R-TICKET-1.
           CALL "TICKET" RETURNING R-TICKET-2.
           CALL "TICKET" RETURNING R-TICKET-3.

           MOVE "R18" TO WS-ID.
           MOVE "static callee called three times" TO WS-FORM.
           MOVE "001/002/003" TO WS-WANT.
           STRING R-TICKET-1 "/" R-TICKET-2 "/" R-TICKET-3
               DELIMITED BY SIZE INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> MIXED: adds 100 to its first argument, copies it to its
      *>        second, and returns the sum of the two.
      *> Uses:   R-IN = 005 (BY CONTENT); R-OUT = 000 (BY REFERENCE).
      *> Result: R-IN = 005 (put back); R-OUT = 105 (written
      *>         through); R-SUM = 0210.

           CALL "MIXED" USING BY CONTENT R-IN BY REFERENCE R-OUT
               RETURNING R-SUM.

           MOVE "R19" TO WS-ID.
           MOVE "BY CONTENT + BY REFERENCE + RETURNING" TO WS-FORM.
           MOVE "005/105/0210" TO WS-WANT.
           STRING R-IN "/" R-OUT "/" R-SUM DELIMITED BY SIZE
               INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> OUTER-ONE: calls DOUBLE with RETURNING, adds 1, returns it.
      *> Uses:   WS-N = 20 (set just below).
      *> Result: R-CHAIN = 00041 (20 * 2 + 1); WS-N is unchanged.

           MOVE 20 TO WS-N.
           CALL "OUTER-ONE" USING WS-N RETURNING R-CHAIN.

           MOVE "R20" TO WS-ID.
           MOVE "RETURNING through a chain of two calls" TO WS-FORM.
           MOVE "00041/0020" TO WS-WANT.
           STRING R-CHAIN "/" WS-N DELIMITED BY SIZE INTO WS-GOT.
           PERFORM RECORD-CASE.

      *> ---- bulk: 20,000 calls, timed -----------------------------
       BULK-PHASE.
           MOVE 0 TO BULK-SUM.
           MOVE 0 TO BULK-WANT.
           ACCEPT T-START FROM TIME.
           PERFORM VARYING BULK-I FROM 1 BY 1
                   UNTIL BULK-I > BULK-COUNT
               MOVE BULK-I TO BULK-N

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   BULK-N = the loop number, 1 to 9999 and then wraps
      *>         (PIC 9(4)), the same one added to BULK-WANT below.
      *> Result: BULK-R = 2 * BULK-N.

               CALL "DOUBLE" USING BULK-N RETURNING BULK-R

               ADD BULK-R TO BULK-SUM
               COMPUTE BULK-WANT = BULK-WANT + (BULK-N * 2)
           END-PERFORM.
           ACCEPT T-END FROM TIME.
           PERFORM COMPUTE-ELAPSED.

      *> ---- helpers -----------------------------------------------
       RECORD-CASE.
           ADD 1 TO CASES-RUN.
           MOVE WS-ID   TO CASE-ID   (CASES-RUN).
           MOVE WS-FORM TO CASE-FORM (CASES-RUN).
           MOVE WS-WANT TO CASE-WANT (CASES-RUN).
           MOVE WS-GOT  TO CASE-GOT  (CASES-RUN).
           IF WS-GOT = WS-WANT
               MOVE "PASS" TO CASE-OK (CASES-RUN)
               ADD 1 TO CASES-PASSED
           ELSE
               MOVE "FAIL" TO CASE-OK (CASES-RUN)
               ADD 1 TO CASES-FAILED
           END-IF.
           MOVE SPACES TO WS-GOT.

       COMPUTE-ELAPSED.
           MOVE T-START TO T-WORK.
           DIVIDE T-WORK BY 1000000 GIVING T-HH REMAINDER T-REST.
           MOVE T-REST TO T-WORK.
           DIVIDE T-WORK BY 10000 GIVING T-MM REMAINDER T-REST.
           MOVE T-REST TO T-WORK.
           DIVIDE T-WORK BY 100 GIVING T-SS REMAINDER T-HS.
           COMPUTE T-START-CS =
               (((T-HH * 60 + T-MM) * 60 + T-SS) * 100) + T-HS.
           MOVE T-END TO T-WORK.
           DIVIDE T-WORK BY 1000000 GIVING T-HH REMAINDER T-REST.
           MOVE T-REST TO T-WORK.
           DIVIDE T-WORK BY 10000 GIVING T-MM REMAINDER T-REST.
           MOVE T-REST TO T-WORK.
           DIVIDE T-WORK BY 100 GIVING T-SS REMAINDER T-HS.
           COMPUTE T-END-CS =
               (((T-HH * 60 + T-MM) * 60 + T-SS) * 100) + T-HS.
           IF T-END-CS < T-START-CS
               ADD 8640000 TO T-END-CS
           END-IF.
           COMPUTE T-ELAPSED-CS = T-END-CS - T-START-CS.
           IF T-ELAPSED-CS = ZERO
               MOVE 1 TO T-ELAPSED-CS
           END-IF.
           COMPUTE T-ELAPSED-SEC = T-ELAPSED-CS / 100.
           COMPUTE T-RATE = (BULK-COUNT * 100) / T-ELAPSED-CS.

       PRINT-RESULTS.
           DISPLAY "============================================".
           DISPLAY "TEST-CALL-RETURNING".
           DISPLAY "CALL ... RETURNING identifier (nested programs)".
           DISPLAY "--------------------------------------------".
           DISPLAY "Case  Form exercised / wanted / got".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > CASES-RUN
               DISPLAY CASE-OK (WS-I) " " CASE-ID (WS-I) " "
                       CASE-FORM (WS-I)
               DISPLAY "           wanted [" CASE-WANT (WS-I)
                       "] got [" CASE-GOT (WS-I) "]"
           END-PERFORM.
           DISPLAY "--------------------------------------------".
           MOVE BULK-COUNT TO BULK-COUNT-EDIT.
           MOVE T-ELAPSED-SEC TO T-ELAPSED-EDIT.
           MOVE T-RATE TO T-RATE-EDIT.
           DISPLAY "Bulk phase      : CALL DOUBLE USING n RETURNING r".
           DISPLAY "Calls           : " BULK-COUNT-EDIT.
           DISPLAY "Elapsed seconds : " T-ELAPSED-EDIT.
           DISPLAY "Calls per second: " T-RATE-EDIT.
           IF BULK-SUM = BULK-WANT
               DISPLAY "Sum of results  : " BULK-SUM " (as expected)"
               ADD 1 TO CASES-PASSED
           ELSE
               DISPLAY "Sum of results  : " BULK-SUM " WANTED "
                       BULK-WANT
               ADD 1 TO CASES-FAILED
           END-IF.
           ADD 1 TO CASES-RUN.
           DISPLAY "--------------------------------------------".
           DISPLAY "Cases exercised : " CASES-RUN
                   " (20 forms + the bulk checksum)".
           DISPLAY "Cases passed    : " CASES-PASSED.
           DISPLAY "Cases failed    : " CASES-FAILED.
           IF CASES-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".

       HELPER.
           CONTINUE.
       END PROGRAM TEST-CALL-RETURNING.

      *> ============================================================
      *> Called programs
      *> ============================================================
       IDENTIFICATION DIVISION.
       PROGRAM-ID. DOUBLE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-N                    PIC 9(4).
       01  LK-R                    PIC 9(9).
       PROCEDURE DIVISION USING LK-N RETURNING LK-R.
       DOUBLE-PARA.
           COMPUTE LK-R = LK-N * 2.
           GOBACK.
       END PROGRAM DOUBLE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PRICE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-PRICE                PIC 9(4)V99.
       PROCEDURE DIVISION RETURNING LK-PRICE.
       PRICE-PARA.
           MOVE 12.34 TO LK-PRICE.
           GOBACK.
       END PROGRAM PRICE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BIG-PRICE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-PRICE                PIC 9(4)V99.
       PROCEDURE DIVISION RETURNING LK-PRICE.
       BIG-PRICE-PARA.
           MOVE 1234.56 TO LK-PRICE.
           GOBACK.
       END PROGRAM BIG-PRICE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. NEGATIVE.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-VALUE                PIC S9(4).
       PROCEDURE DIVISION RETURNING LK-VALUE.
       NEGATIVE-PARA.
           MOVE -42 TO LK-VALUE.
           GOBACK.
       END PROGRAM NEGATIVE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TAG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-OUT                  PIC X(6) VALUE SPACES.
       PROCEDURE DIVISION RETURNING WS-OUT.
       TAG-PARA.
           MOVE "ABC" TO WS-OUT.
           GOBACK.
       END PROGRAM TAG.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAKE-REC.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-REC.
           05  LK-NAME             PIC X(5).
           05  LK-QTY              PIC 9(3).
       PROCEDURE DIVISION RETURNING LK-REC.
       MAKE-REC-PARA.
           MOVE "PEARS" TO LK-NAME.
           MOVE 12 TO LK-QTY.
           GOBACK.
       END PROGRAM MAKE-REC.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUIET.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-X                    PIC 9(3).
       PROCEDURE DIVISION USING LK-X.
       QUIET-PARA.
           ADD 1 TO LK-X.
           GOBACK.
       END PROGRAM QUIET.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TICKET.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-LAST                 PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01  LK-NEXT                 PIC 9(3).
       PROCEDURE DIVISION RETURNING LK-NEXT.
       TICKET-PARA.
           ADD 1 TO WS-LAST.
           MOVE WS-LAST TO LK-NEXT.
           GOBACK.
       END PROGRAM TICKET.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. MIXED.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-IN                   PIC 9(3).
       01  LK-OUT                  PIC 9(3).
       01  LK-SUM                  PIC 9(4).
       PROCEDURE DIVISION USING LK-IN LK-OUT RETURNING LK-SUM.
       MIXED-PARA.
           ADD 100 TO LK-IN.
           MOVE LK-IN TO LK-OUT.
           COMPUTE LK-SUM = LK-IN + LK-OUT.
           GOBACK.
       END PROGRAM MIXED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. OUTER-ONE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-MID                  PIC 9(5) VALUE 0.
       LINKAGE SECTION.
       01  LK-N                    PIC 9(4).
       01  LK-R                    PIC 9(5).
       PROCEDURE DIVISION USING LK-N RETURNING LK-R.
       OUTER-PARA.

      *> DOUBLE: returns twice its argument in its RETURNING item.
      *> Uses:   LK-N = the caller's number (20 in this test).
      *> Result: WS-MID = 2 * LK-N.

           CALL "DOUBLE" USING LK-N RETURNING WS-MID.

           ADD 1 TO WS-MID.
           MOVE WS-MID TO LK-R.
           GOBACK.
       END PROGRAM OUTER-ONE.

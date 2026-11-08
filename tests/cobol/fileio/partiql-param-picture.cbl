       IDENTIFICATION DIVISION.
      *> ============================================================
      *> PARTIQL-PARAM-PICTURE
      *> Spec 108 R3: COBOL::"PARTIQL" types each ? parameter by the
      *> PICTURE of the data item, never by the kind of value its
      *> storage happens to hold. A group MOVE is alphanumeric, so it
      *> leaves characters in a PIC 9 child; a key filled that way
      *> used to reach the statement as a String and fail with
      *> 94 ValidationException "argument n is a String".
      *> The two files are QUOTE-FILE and SEQUENCE-FILE of the
      *> PowerOficina Backend, cut to the fields the cases use.
      *> Forms exercised:
      *>   Q01  UPDATE ... WHERE key = ?, the key from a group MOVE
      *>   Q02  SELECT on an alternate key filled by a group MOVE
      *>   Q03  PIC 9(9)V99 through a group MOVE keeps its scale
      *>   Q04  PIC S9(7)V99 through a group MOVE keeps its sign
      *>   Q05  a key returned by CALL ... RETURNING into a group
      *>   Q06  INSERT of PIC X(12) + PIC 9(10) from a group image
      *>   Q07  UPDATE ... RETURNING ALL NEW * on that key, value + 1
      *>   Q08  a numeric literal is still a Number
      *>   Q09  a PIC 9 key holding spaces is still refused with 94
      *> Then two timed phases on BULK-COUNT quotes: the load (one
      *> INSERT each) and the bulk UPDATE, every key filled by a
      *> group MOVE first, and a SELECT that counts what it changed.
      *> Self-checking. One result block at the end, no per-row lines.
      *> ============================================================
       PROGRAM-ID. PARTIQL-PARAM-PICTURE.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT QUOTE-FILE ASSIGN TO "partiql-param-quotes.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QUO-KEY
               ALTERNATE RECORD KEY IS QUO-CUS-ID WITH DUPLICATES.
           SELECT SEQUENCE-FILE ASSIGN TO "partiql-param-seqs.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS SEQ-NAME.
       DATA DIVISION.
       FILE SECTION.
       FD  QUOTE-FILE.
       01  QUOTE-RECORD.
           05  QUO-KEY.
               10  QUO-ID          PIC 9(8).
               10  QUO-REV         PIC 9(2).
           05  QUO-CUS-ID          PIC 9(8).
           05  QUO-STATUS          PIC X(10).
           05  QUO-TOTAL           PIC 9(9)V99.
           05  QUO-BALANCE         PIC S9(7)V99.
       FD  SEQUENCE-FILE.
       01  SEQUENCE-RECORD.
           05  SEQ-NAME            PIC X(12).
           05  SEQ-VALUE           PIC 9(10).
       WORKING-STORAGE SECTION.
       01  CASE-TABLE.
           05  CASE-ROW OCCURS 10 TIMES.
               10  CASE-ID         PIC X(4).
               10  CASE-FORM       PIC X(60).
               10  CASE-WANT       PIC X(24).
               10  CASE-GOT        PIC X(24).
               10  CASE-OK         PIC X(4).
       01  CASES-RUN               PIC 9(3)  VALUE 0.
       01  CASES-PASSED            PIC 9(3)  VALUE 0.
       01  CASES-FAILED            PIC 9(3)  VALUE 0.
       01  WS-ID                   PIC X(4)  VALUE SPACES.
       01  WS-FORM                 PIC X(60) VALUE SPACES.
       01  WS-WANT                 PIC X(24) VALUE SPACES.
       01  WS-GOT                  PIC X(24) VALUE SPACES.
       01  WS-COUNT                PIC 9(6)  VALUE 0.
       01  WS-ST                   PIC XX    VALUE SPACES.
       01  WS-ERR                  PIC X(200) VALUE SPACES.
       01  WS-ROW                  PIC X(200) VALUE SPACES.
       01  WS-IMAGE                PIC X(48) VALUE SPACES.
       01  WS-SEQ-IMAGE            PIC X(22) VALUE SPACES.
       01  WS-STRING-ARGS          PIC 9(3)  VALUE 0.
       01  WS-I                    PIC 9(4)  VALUE 0.
       01  WS-WANT-ID              PIC 9(8)  VALUE 0.
       01  WS-RES.
           05  RES-ID              PIC 9(8).
           05  RES-REV             PIC 9(2).
       01  BULK-COUNT              PIC 9(4)  VALUE 200.
       01  BULK-WANT-CUSTOMER      PIC 9(4)  VALUE 0.
       01  LOAD-OK                 PIC 9(4)  VALUE 0.
       01  BULK-OK                 PIC 9(4)  VALUE 0.
       01  BULK-CHECKED            PIC 9(6)  VALUE 0.
       01  T-START                 PIC 9(8)  VALUE 0.
       01  T-END                   PIC 9(8)  VALUE 0.
       01  T-HH                    PIC 99    VALUE 0.
       01  T-MM                    PIC 99    VALUE 0.
       01  T-SS                    PIC 99    VALUE 0.
       01  T-HS                    PIC 99    VALUE 0.
       01  T-WORK                  PIC 9(8)  VALUE 0.
       01  T-REST                  PIC 9(8)  VALUE 0.
       01  T-START-CS              PIC 9(9)  VALUE 0.
       01  T-END-CS                PIC 9(9)  VALUE 0.
       01  T-ELAPSED-CS            PIC 9(9)  VALUE 0.
       01  T-ELAPSED-SEC           PIC 9(7)V99 VALUE 0.
       01  T-RATE                  PIC 9(9)  VALUE 0.
       01  LOAD-SEC-EDIT           PIC ZZZ,ZZ9.99.
       01  LOAD-RATE-EDIT          PIC ZZZ,ZZZ,ZZ9.
       01  BULK-SEC-EDIT           PIC ZZZ,ZZ9.99.
       01  BULK-RATE-EDIT          PIC ZZZ,ZZZ,ZZ9.
       PROCEDURE DIVISION.
       MAIN-PARA.
           OPEN OUTPUT QUOTE-FILE
           CLOSE QUOTE-FILE
           OPEN I-O QUOTE-FILE
           OPEN OUTPUT SEQUENCE-FILE
           CLOSE SEQUENCE-FILE
           OPEN I-O SEQUENCE-FILE
           PERFORM LOAD-PHASE
           PERFORM KEY-CASES
           PERFORM SCALE-CASES
           PERFORM RETURNING-CASE
           PERFORM SEQUENCE-CASES
           PERFORM LITERAL-AND-REFUSAL-CASES
           PERFORM BULK-PHASE
           CLOSE QUOTE-FILE SEQUENCE-FILE
           PERFORM PRINT-RESULTS
           STOP RUN.

      *> ---- Load: BULK-COUNT quotes, each by one INSERT --------------
      *> Every number here was MOVEd as a number, so this phase passed
      *> before the fix too. Customer = quote number mod 10, plus 1.
       LOAD-PHASE.
           ACCEPT T-START FROM TIME
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > BULK-COUNT
               INITIALIZE QUOTE-RECORD
               MOVE WS-I TO QUO-ID
               MOVE 1 TO QUO-REV
               COMPUTE QUO-CUS-ID = FUNCTION MOD(WS-I, 10) + 1
               MOVE "DRAFT" TO QUO-STATUS
               MOVE 10.50 TO QUO-TOTAL
               COBOL::"PARTIQL" ( "QUOTE-FILE"
                   "INSERT INTO QUOTE-FILE VALUE {'QUO-ID': ?, 'QUO-REV': ?, 'QUO-CUS-ID': ?, 'QUO-STATUS': ?, 'QUO-TOTAL': ?, 'QUO-BALANCE': ?}"
                   WS-COUNT WS-ST QUO-ID QUO-REV QUO-CUS-ID QUO-STATUS
                   QUO-TOTAL QUO-BALANCE )
               IF WS-ST = "00" AND WS-COUNT = 1
                   ADD 1 TO LOAD-OK
               END-IF
           END-PERFORM
           ACCEPT T-END FROM TIME
           PERFORM COMPUTE-ELAPSED
           MOVE T-ELAPSED-SEC TO LOAD-SEC-EDIT
           MOVE T-RATE TO LOAD-RATE-EDIT.

      *> ---- Q01-Q02: a key filled by a group MOVE ---------------------
       KEY-CASES.
      *> Q01: quote 7 revision 1. The record goes out to an image and
      *> comes back by a group MOVE, which leaves the characters
      *> "00000007" in QUO-ID: a Number by its PICTURE all the same.
           INITIALIZE QUOTE-RECORD
           MOVE 7 TO QUO-ID
           MOVE 1 TO QUO-REV
           PERFORM ROUND-TRIP-QUOTE
           MOVE "SENT" TO QUO-STATUS
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "UPDATE QUOTE-FILE SET QUO-STATUS = ? WHERE QUO-ID = ? AND QUO-REV = ?"
               WS-COUNT WS-ST QUO-STATUS QUO-ID QUO-REV )
           MOVE "Q01" TO WS-ID
           MOVE "UPDATE WHERE QUO-ID = ? AND QUO-REV = ?, group-filled"
               TO WS-FORM
           MOVE "00 000001" TO WS-WANT
           STRING WS-ST " " WS-COUNT DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE

      *> Q02: the alternate key QUO-CUS-ID = 3, through a group MOVE.
      *> Customer 3 holds every quote whose number mod 10 is 2.
           INITIALIZE QUOTE-RECORD
           MOVE 3 TO QUO-CUS-ID
           PERFORM ROUND-TRIP-QUOTE
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-ID FROM QUOTE-FILE WHERE QUO-CUS-ID = ?"
               WS-COUNT WS-ST QUO-CUS-ID )
           COMPUTE BULK-WANT-CUSTOMER = BULK-COUNT / 10
           MOVE "Q02" TO WS-ID
           MOVE "SELECT WHERE QUO-CUS-ID = ? (alternate key), group-filled"
               TO WS-FORM
           STRING "00 " BULK-WANT-CUSTOMER DELIMITED BY SIZE
               INTO WS-WANT
           STRING WS-ST " " WS-COUNT(3:4) DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE.

      *> ---- Q03-Q04: decimals and sign survive the group MOVE ---------
       SCALE-CASES.
      *> Q03: quote 9 gets the total 1234.56 from a PIC 9(9)V99 that
      *> came back through a group MOVE; read back with a literal key.
           INITIALIZE QUOTE-RECORD
           MOVE 9 TO QUO-ID
           MOVE 1 TO QUO-REV
           MOVE 1234.56 TO QUO-TOTAL
           PERFORM ROUND-TRIP-QUOTE
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "UPDATE QUOTE-FILE SET QUO-TOTAL = ? WHERE QUO-ID = ? AND QUO-REV = ?"
               WS-COUNT WS-ST QUO-TOTAL QUO-ID QUO-REV )
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-TOTAL FROM QUOTE-FILE WHERE QUO-ID = 9 AND QUO-REV = 1"
               WS-COUNT WS-ST )
           MOVE SPACES TO WS-ROW
           COBOL::"PARTIQL-FETCH" ( "QUOTE-FILE" WS-ROW WS-ST )
           MOVE "Q03" TO WS-ID
           MOVE "SET QUO-TOTAL = ? from PIC 9(9)V99, group-filled"
               TO WS-FORM
           MOVE "00 1234.56" TO WS-WANT
           STRING WS-ST " " WS-ROW DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE

      *> Q04: quote 10 gets the balance -98.76 from a PIC S9(7)V99.
           INITIALIZE QUOTE-RECORD
           MOVE 10 TO QUO-ID
           MOVE 1 TO QUO-REV
           MOVE -98.76 TO QUO-BALANCE
           PERFORM ROUND-TRIP-QUOTE
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "UPDATE QUOTE-FILE SET QUO-BALANCE = ? WHERE QUO-ID = ? AND QUO-REV = ?"
               WS-COUNT WS-ST QUO-BALANCE QUO-ID QUO-REV )
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-BALANCE FROM QUOTE-FILE WHERE QUO-ID = 10 AND QUO-REV = 1"
               WS-COUNT WS-ST )
           MOVE SPACES TO WS-ROW
           COBOL::"PARTIQL-FETCH" ( "QUOTE-FILE" WS-ROW WS-ST )
           MOVE "Q04" TO WS-ID
           MOVE "SET QUO-BALANCE = ? from PIC S9(7)V99, group-filled"
               TO WS-FORM
           MOVE "00 -98.76" TO WS-WANT
           STRING WS-ST " " WS-ROW DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE.

      *> ---- Q05: the PowerOficina path, CALL ... RETURNING a group ----
       RETURNING-CASE.
      *> LOAD-QUOTE-KEY answers the key of quote 11 in a group; the
      *> RETURNING item lands in WS-RES as a group, so RES-ID holds
      *> characters. Quote 11 is still a DRAFT.
           MOVE 11 TO WS-WANT-ID
           CALL "LOAD-QUOTE-KEY" USING WS-WANT-ID RETURNING WS-RES
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-STATUS FROM QUOTE-FILE WHERE QUO-ID = ? AND QUO-REV = ?"
               WS-COUNT WS-ST RES-ID RES-REV )
           MOVE SPACES TO WS-ROW
           COBOL::"PARTIQL-FETCH" ( "QUOTE-FILE" WS-ROW WS-ST )
           MOVE "Q05" TO WS-ID
           MOVE "WHERE QUO-ID = ? with a key from CALL ... RETURNING"
               TO WS-FORM
           MOVE "00 DRAFT" TO WS-WANT
           STRING WS-ST " " WS-ROW DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE.

      *> ---- Q06-Q07: SEQUENCE-FILE from a record image ----------------
       SEQUENCE-CASES.
      *> Q06: the 22-byte image "AUDIT" + 0000000041, moved over the
      *> record as a group: SEQ-NAME a String, SEQ-VALUE a Number.
           MOVE "AUDIT       0000000041" TO WS-SEQ-IMAGE
           MOVE WS-SEQ-IMAGE TO SEQUENCE-RECORD
           COBOL::"PARTIQL" ( "SEQUENCE-FILE"
               "INSERT INTO SEQUENCE-FILE VALUE {'SEQ-NAME': ?, 'SEQ-VALUE': ?}"
               WS-COUNT WS-ST SEQ-NAME SEQ-VALUE )
           MOVE "Q06" TO WS-ID
           MOVE "INSERT PIC X(12) + PIC 9(10) from a group image"
               TO WS-FORM
           MOVE "00 000001" TO WS-WANT
           STRING WS-ST " " WS-COUNT DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE

      *> Q07: the next number of AUDIT, keyed by the same SEQ-NAME. A
      *> row shows an integer field with all its digits: 0000000042.
           COBOL::"PARTIQL" ( "SEQUENCE-FILE"
               "UPDATE SEQUENCE-FILE SET SEQ-VALUE = SEQ-VALUE + 1 WHERE SEQ-NAME = ? RETURNING ALL NEW *"
               WS-COUNT WS-ST SEQ-NAME )
           MOVE SPACES TO WS-ROW
           COBOL::"PARTIQL-FETCH" ( "SEQUENCE-FILE" WS-ROW WS-ST )
           INSPECT WS-ROW REPLACING ALL X"09" BY "/"
           MOVE "Q07" TO WS-ID
           MOVE "UPDATE SEQ-VALUE + 1 RETURNING ALL NEW *, group key"
               TO WS-FORM
           MOVE "00 AUDIT/0000000042" TO WS-WANT
           STRING WS-ST " " WS-ROW DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE.

      *> ---- Q08-Q09: what the fix must not change ---------------------
       LITERAL-AND-REFUSAL-CASES.
      *> Q08: a literal is typed by its form: 12 is a Number.
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-ID FROM QUOTE-FILE WHERE QUO-ID = ? AND QUO-REV = 1"
               WS-COUNT WS-ST 12 )
           MOVE "Q08" TO WS-ID
           MOVE "WHERE QUO-ID = ? with the numeric literal 12"
               TO WS-FORM
           MOVE "00 000001" TO WS-WANT
           STRING WS-ST " " WS-COUNT DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE

      *> Q09: MOVE SPACES over the record leaves spaces in QUO-ID.
      *> Spaces spell no number, so no number is invented: the
      *> statement is refused, names argument 2 and changes nothing.
           MOVE SPACES TO QUOTE-RECORD
           MOVE "LOST" TO QUO-STATUS
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "UPDATE QUOTE-FILE SET QUO-STATUS = ? WHERE QUO-ID = ? AND QUO-REV = ?"
               WS-COUNT WS-ST QUO-STATUS QUO-ID QUO-REV )
           MOVE SPACES TO WS-ERR
           COBOL::"PARTIQL-ERROR" ( "QUOTE-FILE" WS-ERR )
           MOVE 0 TO WS-STRING-ARGS
           INSPECT WS-ERR TALLYING WS-STRING-ARGS
               FOR ALL "argument 2 is a String"
           MOVE "Q09" TO WS-ID
           MOVE "a PIC 9 key holding spaces is refused, argument named"
               TO WS-FORM
           MOVE "94 000000 001" TO WS-WANT
           STRING WS-ST " " WS-COUNT " " WS-STRING-ARGS
               DELIMITED BY SIZE INTO WS-GOT
           PERFORM RECORD-CASE.

      *> ---- Bulk: every quote, its key filled by a group MOVE ---------
       BULK-PHASE.
           ACCEPT T-START FROM TIME
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > BULK-COUNT
               INITIALIZE QUOTE-RECORD
               MOVE WS-I TO QUO-ID
               MOVE 1 TO QUO-REV
               PERFORM ROUND-TRIP-QUOTE
               MOVE "CHECKED" TO QUO-STATUS
               COBOL::"PARTIQL" ( "QUOTE-FILE"
                   "UPDATE QUOTE-FILE SET QUO-STATUS = ? WHERE QUO-ID = ? AND QUO-REV = ?"
                   WS-COUNT WS-ST QUO-STATUS QUO-ID QUO-REV )
               IF WS-ST = "00" AND WS-COUNT = 1
                   ADD 1 TO BULK-OK
               END-IF
           END-PERFORM
           ACCEPT T-END FROM TIME
           PERFORM COMPUTE-ELAPSED
           MOVE T-ELAPSED-SEC TO BULK-SEC-EDIT
           MOVE T-RATE TO BULK-RATE-EDIT
           COBOL::"PARTIQL" ( "QUOTE-FILE"
               "SELECT QUO-ID FROM QUOTE-FILE WHERE QUO-STATUS = 'CHECKED'"
               WS-COUNT WS-ST )
           MOVE WS-COUNT TO BULK-CHECKED.

      *> The record goes out to WS-IMAGE and comes back by a group MOVE.
       ROUND-TRIP-QUOTE.
           MOVE QUOTE-RECORD TO WS-IMAGE
           INITIALIZE QUOTE-RECORD
           MOVE WS-IMAGE TO QUOTE-RECORD.

       RECORD-CASE.
           ADD 1 TO CASES-RUN
           MOVE WS-ID TO CASE-ID (CASES-RUN)
           MOVE WS-FORM TO CASE-FORM (CASES-RUN)
           MOVE WS-WANT TO CASE-WANT (CASES-RUN)
           MOVE WS-GOT TO CASE-GOT (CASES-RUN)
           IF WS-GOT = WS-WANT
               MOVE "PASS" TO CASE-OK (CASES-RUN)
               ADD 1 TO CASES-PASSED
           ELSE
               MOVE "FAIL" TO CASE-OK (CASES-RUN)
               ADD 1 TO CASES-FAILED
           END-IF
           MOVE SPACES TO WS-GOT WS-WANT.

       COMPUTE-ELAPSED.
           MOVE T-START TO T-WORK
           DIVIDE T-WORK BY 1000000 GIVING T-HH REMAINDER T-REST
           MOVE T-REST TO T-WORK
           DIVIDE T-WORK BY 10000 GIVING T-MM REMAINDER T-REST
           MOVE T-REST TO T-WORK
           DIVIDE T-WORK BY 100 GIVING T-SS REMAINDER T-HS
           COMPUTE T-START-CS =
               (((T-HH * 60 + T-MM) * 60 + T-SS) * 100) + T-HS
           MOVE T-END TO T-WORK
           DIVIDE T-WORK BY 1000000 GIVING T-HH REMAINDER T-REST
           MOVE T-REST TO T-WORK
           DIVIDE T-WORK BY 10000 GIVING T-MM REMAINDER T-REST
           MOVE T-REST TO T-WORK
           DIVIDE T-WORK BY 100 GIVING T-SS REMAINDER T-HS
           COMPUTE T-END-CS =
               (((T-HH * 60 + T-MM) * 60 + T-SS) * 100) + T-HS
           IF T-END-CS < T-START-CS
               ADD 8640000 TO T-END-CS
           END-IF
           COMPUTE T-ELAPSED-CS = T-END-CS - T-START-CS
           IF T-ELAPSED-CS = ZERO
               MOVE 1 TO T-ELAPSED-CS
           END-IF
           COMPUTE T-ELAPSED-SEC = T-ELAPSED-CS / 100
           COMPUTE T-RATE = (BULK-COUNT * 100) / T-ELAPSED-CS.

       PRINT-RESULTS.
           DISPLAY "============================================"
           DISPLAY "PARTIQL-PARAM-PICTURE"
           DISPLAY "COBOL::""PARTIQL"" ? typed by PICTURE (108 R3)"
           DISPLAY "--------------------------------------------"
           DISPLAY "Case  Form exercised / wanted / got"
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > CASES-RUN
               DISPLAY CASE-OK (WS-I) " " CASE-ID (WS-I) " "
                       CASE-FORM (WS-I)
               DISPLAY "           wanted [" CASE-WANT (WS-I)
                       "] got [" CASE-GOT (WS-I) "]"
           END-PERFORM
           DISPLAY "--------------------------------------------"
           DISPLAY "Load phase      : INSERT, numbers MOVEd as numbers"
           DISPLAY "Statements      : " BULK-COUNT " (" LOAD-OK " ok)"
           DISPLAY "Elapsed seconds : " LOAD-SEC-EDIT
           DISPLAY "Per second      : " LOAD-RATE-EDIT
           IF LOAD-OK = BULK-COUNT
               ADD 1 TO CASES-PASSED
           ELSE
               ADD 1 TO CASES-FAILED
           END-IF
           ADD 1 TO CASES-RUN
           DISPLAY "Bulk phase      : UPDATE, key from a group MOVE"
           DISPLAY "Statements      : " BULK-COUNT " (" BULK-OK " ok)"
           DISPLAY "Elapsed seconds : " BULK-SEC-EDIT
           DISPLAY "Per second      : " BULK-RATE-EDIT
           DISPLAY "Rows CHECKED    : " BULK-CHECKED
           IF BULK-OK = BULK-COUNT AND BULK-CHECKED = BULK-COUNT
               ADD 1 TO CASES-PASSED
           ELSE
               ADD 1 TO CASES-FAILED
           END-IF
           ADD 1 TO CASES-RUN
           DISPLAY "--------------------------------------------"
           DISPLAY "Cases exercised : " CASES-RUN
                   " (9 forms + the load + the bulk phase)"
           DISPLAY "Cases passed    : " CASES-PASSED
           DISPLAY "Cases failed    : " CASES-FAILED
           IF CASES-FAILED = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF
           DISPLAY "============================================".
       END PROGRAM PARTIQL-PARAM-PICTURE.

      *> ============================================================
      *> Called program: the key of one quote, answered in a group.
      *> ============================================================
       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOAD-QUOTE-KEY.
       DATA DIVISION.
       LINKAGE SECTION.
       01  LK-ID                   PIC 9(8).
       01  LK-RES.
           05  LK-RES-ID           PIC 9(8).
           05  LK-RES-REV          PIC 9(2).
       PROCEDURE DIVISION USING LK-ID RETURNING LK-RES.
       LOAD-QUOTE-KEY-PARA.
           MOVE LK-ID TO LK-RES-ID
           MOVE 1 TO LK-RES-REV
           GOBACK.
       END PROGRAM LOAD-QUOTE-KEY.

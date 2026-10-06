       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC2.
      *> Spec 087 AC2 - a host structure fills its named elementary items
      *> in order (FILLER and REDEFINES skipped), and a host variable is
      *> bound as a parameter, never spliced into the SQL text.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  CUST-ROW.
           05  CR-ID           PIC 9(5).
           05  FILLER          PIC X(3).
           05  CR-NAME         PIC X(20).
           05  CR-CITY         PIC X(15).
           05  CR-CITY-ALT     REDEFINES CR-CITY PIC X(15).
           05  CR-BALANCE      PIC S9(7)V99 COMP-3.
       01  WS-HOSTILE          PIC X(20) VALUE "x' OR '1'='1".
       01  WS-COUNT            PIC 9(5).
       01  SQLSTATE            PIC X(5).
       01  SQLCODE             PIC S9(9) COMP-5.
       01  SQLMSG              PIC X(200).
       01  WS-PASS             PIC 9(3) VALUE 0.
       01  WS-FAIL             PIC 9(3) VALUE 0.
       01  WS-T0               PIC 9(9).
       01  WS-T1               PIC 9(9).
       01  WS-MS               PIC 9(9).
       01  WS-NOW              PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           EXEC SQL CONNECT TO ':memory:' AS AC2 END-EXEC
           EXEC SQL
               CREATE TABLE CUSTOMER
                 (ID INTEGER, NAME TEXT, CITY TEXT, BALANCE NUMERIC)
           END-EXEC
           EXEC SQL INSERT INTO CUSTOMER
               VALUES (7, 'Ana Lima', 'Recife', -1234.56) END-EXEC
           EXEC SQL INSERT INTO CUSTOMER
               VALUES (8, 'x'' OR ''1''=''1', 'Natal', 0) END-EXEC
           EXEC SQL INSERT INTO CUSTOMER
               VALUES (9, 'Rui', 'Natal', 1) END-EXEC
           MOVE "ZZZ" TO CR-CITY-ALT
           EXEC SQL
               SELECT ID, NAME, CITY, BALANCE
                 INTO :CUST-ROW
                 FROM CUSTOMER
                WHERE ID = 7
           END-EXEC
           IF SQLSTATE = "00000" AND CR-ID = 7
              AND CR-NAME = "Ana Lima" AND CR-CITY = "Recife"
              AND CR-BALANCE = -1234.56
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL host structure: " SQLSTATE " " CR-ID " "
                   CR-NAME " " CR-CITY " " CR-BALANCE
           END-IF
           EXEC SQL
               SELECT COUNT(*) INTO :WS-COUNT
                 FROM CUSTOMER WHERE NAME = :WS-HOSTILE
           END-EXEC
           IF WS-COUNT = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL bound value matched " WS-COUNT " rows"
           END-IF
           EXEC SQL DISCONNECT ALL END-EXEC
           PERFORM NOW-MS
           COMPUTE WS-T1 = WS-MS - WS-T0
           DISPLAY "=== ESQL AC2 RESULT ==="
           DISPLAY "forms: CONNECT TO ':memory:' AS AC2; CREATE TABLE; "
                   "INSERT x3; SELECT ... INTO :CUST-ROW (5 items, "
                   "FILLER + REDEFINES skipped, 4 filled); "
                   "SELECT COUNT(*) INTO :WS-COUNT WHERE NAME = :WS-HOSTILE"
           DISPLAY "rows: inserted 3; hostile text matched " WS-COUNT
           DISPLAY "elapsed ms: " WS-T1
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

       *> Split keys: a key made of several fields, joined in order, that
       *> need not be next to each other in the record. Both spellings:
       *>   Micro Focus  RECORD KEY IS ORD-KEY = ORD-REGION ORD-NUMBER
       *>   Fujitsu      ALTERNATE RECORD KEY IS ORD-CUSTOMER, ORD-DATE
       *> The STORAGE line below is replaced by the harness to run the same
       *> program on each engine.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. SPLITKEYS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT ORDERS ASSIGN TO "/tmp/split-orders.dat"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               *>STORAGE
               RECORD KEY IS ORD-KEY = ORD-REGION ORD-NUMBER
               ALTERNATE RECORD KEY IS ORD-CUSTOMER, ORD-DATE
                   WITH DUPLICATES
               ALTERNATE RECORD KEY IS ORD-REF = ORD-DATE ORD-NUMBER
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD ORDERS.
       01 ORD-REC.
          05 ORD-REGION    PIC X(2).
          05 ORD-CUSTOMER  PIC X(6).
          05 ORD-AMOUNT    PIC 9(5).
          05 ORD-DATE      PIC 9(8).
          05 ORD-NUMBER    PIC 9(4).
       WORKING-STORAGE SECTION.
       01 WS-FS            PIC XX.
       01 WS-PASS          PIC 999 VALUE 0.
       01 WS-FAIL          PIC 999 VALUE 0.
       01 WS-N             PIC 99 VALUE 0.
       01 WS-SUM           PIC 9(6) VALUE 0.
       01 WS-SEEN          PIC X(30) VALUE SPACES.
       01 WS-PTR           PIC 99 VALUE 1.
       01 WS-AMT           PIC Z(4)9.
       01 WS-EOF           PIC 9 VALUE 0.
       01 WS-T0            PIC X(21).
       01 WS-T1            PIC X(21).
       01 WS-MS            PIC 9(9).
       01 WS-ROW.
          05 R-REGION      PIC X(2).
          05 R-CUSTOMER    PIC X(6).
          05 R-AMOUNT      PIC 9(5).
          05 R-DATE        PIC 9(8).
          05 R-NUMBER      PIC 9(4).
       PROCEDURE DIVISION.
       MAIN.
           MOVE FUNCTION CURRENT-DATE TO WS-T0
           OPEN OUTPUT ORDERS
           CLOSE ORDERS
           OPEN I-O ORDERS
      *>   1. Five orders: the Fujitsu key repeats (ANA, 2026-10-01) three
      *>      times, which WITH DUPLICATES allows.
           MOVE "EUANA   0010020261001" TO WS-ROW MOVE 1 TO R-NUMBER
           PERFORM WRITE-ROW
           MOVE "EUBOB   0020020261002" TO WS-ROW MOVE 2 TO R-NUMBER
           PERFORM WRITE-ROW
           MOVE "USANA   0030020261001" TO WS-ROW MOVE 11 TO R-NUMBER
           PERFORM WRITE-ROW
           MOVE "USANA   0040020261003" TO WS-ROW MOVE 12 TO R-NUMBER
           PERFORM WRITE-ROW
           MOVE "EUANA   0015020261001" TO WS-ROW MOVE 3 TO R-NUMBER
           PERFORM WRITE-ROW
           IF WS-N = 5
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL five writes, " WS-N " succeeded"
           END-IF
      *>   2. The primary split key (EU, 0001) again: 22.
           MOVE "EUZED   0000120261009" TO WS-ROW MOVE 1 TO R-NUMBER
           MOVE WS-ROW TO ORD-REC
           WRITE ORD-REC
           PERFORM EXPECT-22
      *>   3. The Micro Focus alternate (2026-10-02, 0002) again, under a new
      *>      primary key: 22, since it has no WITH DUPLICATES.
           MOVE "ASCAT   0000120261002" TO WS-ROW MOVE 2 TO R-NUMBER
           MOVE WS-ROW TO ORD-REC
           WRITE ORD-REC
           PERFORM EXPECT-22
      *>   4. A random READ by the primary split key.
           MOVE "US" TO ORD-REGION
           MOVE 12 TO ORD-NUMBER
           READ ORDERS
           IF WS-FS = "00" AND ORD-AMOUNT = 400
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL READ by ORD-KEY: " WS-FS " " ORD-AMOUNT
           END-IF
      *>   5. START on the Fujitsu key, named by its first field, then every
      *>      duplicate. Which three, not their order: the engines differ on
      *>      the order of duplicates (written order on disk and redb, primary
      *>      key order in memory), and that is not what this test is about.
           MOVE "ANA" TO ORD-CUSTOMER
           MOVE 20261001 TO ORD-DATE
           START ORDERS KEY IS = ORD-CUSTOMER
           MOVE 0 TO WS-N
           MOVE 0 TO WS-SUM
           MOVE 0 TO WS-EOF
           PERFORM UNTIL WS-EOF = 1
               READ ORDERS NEXT
                   AT END MOVE 1 TO WS-EOF
               END-READ
               IF WS-EOF = 0
                   IF ORD-CUSTOMER = "ANA" AND ORD-DATE = 20261001
                       ADD 1 TO WS-N
                       ADD ORD-AMOUNT TO WS-SUM
                   ELSE
                       MOVE 1 TO WS-EOF
                   END-IF
               END-IF
           END-PERFORM
           IF WS-N = 3 AND WS-SUM = 550
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL START on ORD-CUSTOMER: " WS-N " records, " WS-SUM
           END-IF
      *>   6. START on the Micro Focus alternate, by its key name.
           MOVE 20261002 TO ORD-DATE
           MOVE 0 TO ORD-NUMBER
           START ORDERS KEY IS >= ORD-REF
           READ ORDERS NEXT
           IF WS-FS = "00" AND ORD-CUSTOMER = "BOB"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL START on ORD-REF: " WS-FS " " ORD-CUSTOMER
           END-IF
      *>   7. A random READ by the Micro Focus alternate.
           MOVE 20261003 TO ORD-DATE
           MOVE 12 TO ORD-NUMBER
           READ ORDERS KEY IS ORD-REF
           IF WS-FS = "00" AND ORD-AMOUNT = 400
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL READ KEY IS ORD-REF: " WS-FS " " ORD-AMOUNT
           END-IF
           CLOSE ORDERS
      *>   8. Reopened, the stored key layout matches the declared one, and
      *>      the records come back in primary split-key order.
           OPEN INPUT ORDERS
           IF WS-FS = "00"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL reopen: " WS-FS
           END-IF
           MOVE SPACES TO WS-SEEN
           MOVE 1 TO WS-PTR
           MOVE 0 TO WS-EOF
           PERFORM UNTIL WS-EOF = 1
               READ ORDERS NEXT
                   AT END MOVE 1 TO WS-EOF
               END-READ
               IF WS-EOF = 0
                   MOVE ORD-AMOUNT TO WS-AMT
                   STRING FUNCTION TRIM(WS-AMT) " " DELIMITED BY SIZE
                       INTO WS-SEEN WITH POINTER WS-PTR
               END-IF
           END-PERFORM
           IF WS-SEEN = "100 200 150 300 400"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL primary order: [" WS-SEEN "]"
           END-IF
           CLOSE ORDERS
           MOVE FUNCTION CURRENT-DATE TO WS-T1
           COMPUTE WS-MS =
               ((FUNCTION NUMVAL(WS-T1(9:2)) * 3600
                 + FUNCTION NUMVAL(WS-T1(11:2)) * 60
                 + FUNCTION NUMVAL(WS-T1(13:2))) * 100
                 + FUNCTION NUMVAL(WS-T1(15:2))) * 10
             - ((FUNCTION NUMVAL(WS-T0(9:2)) * 3600
                 + FUNCTION NUMVAL(WS-T0(11:2)) * 60
                 + FUNCTION NUMVAL(WS-T0(13:2))) * 100
                 + FUNCTION NUMVAL(WS-T0(15:2))) * 10
           DISPLAY "==== Split keys ===="
           DISPLAY "Keys:     ORD-KEY = ORD-REGION ORD-NUMBER (Micro Focus,"
                   " primary)"
           DISPLAY "          ORD-CUSTOMER, ORD-DATE WITH DUPLICATES"
                   " (Fujitsu, alternate)"
           DISPLAY "          ORD-REF = ORD-DATE ORD-NUMBER (Micro Focus,"
                   " alternate)"
           DISPLAY "Checked:  5 writes; duplicate primary 22; duplicate"
                   " alternate 22; READ by primary; START + READ NEXT"
                   " through 3 duplicates; START by key name; READ KEY IS;"
                   " reopen schema; primary order"
           DISPLAY "Elapsed:  " WS-MS " ms"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.

       WRITE-ROW.
           MOVE WS-ROW TO ORD-REC
           WRITE ORD-REC
           IF WS-FS = "00"
               ADD 1 TO WS-N
           ELSE
               DISPLAY "FAIL WRITE " WS-ROW ": " WS-FS
           END-IF.

       EXPECT-22.
           IF WS-FS = "22"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL expected 22, got " WS-FS " for " WS-ROW
           END-IF.

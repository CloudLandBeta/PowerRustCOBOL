       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAT-INDEXED.
      *> Spec 077 AC9 - an INDEXED file keyed on a national item. Writes
      *> WS-RECS records whose PIC N(20) key and PIC N(40) field hold
      *> accented text, reads each one back by key, rewrites them all,
      *> scans the file in key order with START / READ NEXT, and checks
      *> every character. Each phase is timed; one result block at the end.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUSTOMERS ASSIGN TO "nat_indexed.dat"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-NAME
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD  CUSTOMERS.
       01  CUST-REC.
           05  CUST-NAME  PIC N(20).
           05  CUST-CITY  PIC N(40).
       WORKING-STORAGE SECTION.
       01  WS-FS          PIC XX.
       01  WS-RECS        PIC 9(5) VALUE 2000.
       01  WS-I           PIC 9(5).
       01  WS-SEQ         PIC 9(5).
       01  WS-NAME        PIC N(20).
       01  WS-CITY        PIC N(40).
       01  WS-PREV        PIC N(20).
       01  WS-READ        PIC 9(5) VALUE 0.
       01  WS-REWRITTEN   PIC 9(5) VALUE 0.
       01  WS-SCANNED     PIC 9(5) VALUE 0.
       01  WS-BAD         PIC 9(5) VALUE 0.
       01  WS-L           PIC 9(4).
       01  WS-PASS        PIC 9(3) VALUE 0.
       01  WS-FAIL        PIC 9(3) VALUE 0.
       01  WS-T0          PIC 9(9).
       01  WS-MS          PIC 9(9).
       01  WS-WRITE-MS    PIC 9(9).
       01  WS-READ-MS     PIC 9(9).
       01  WS-REWRITE-MS  PIC 9(9).
       01  WS-SCAN-MS     PIC 9(9).
       01  WS-RATE        PIC 9(9).
       01  WS-NOW         PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
      *>   K01 - the record is 120 bytes: 2 x 20 + 2 x 40.
           MOVE FUNCTION BYTE-LENGTH(CUST-REC) TO WS-L
           IF WS-L = 120 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL K01 record " WS-L
           END-IF
      *>   Write.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           OPEN OUTPUT CUSTOMERS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-RECS
               PERFORM MAKE-NAME
               MOVE WS-NAME TO CUST-NAME
               MOVE N"São João – Açores" TO CUST-CITY
               WRITE CUST-REC
           END-PERFORM
           CLOSE CUSTOMERS
           PERFORM NOW-MS
           COMPUTE WS-WRITE-MS = WS-MS - WS-T0
      *>   K02 - read each record back by its key; K03 - rewrite it.
           OPEN I-O CUSTOMERS
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-RECS
               PERFORM MAKE-NAME
               MOVE WS-NAME TO CUST-NAME
               READ CUSTOMERS
               IF WS-FS = "00" AND CUST-CITY = N"São João – Açores"
                   ADD 1 TO WS-READ
               END-IF
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-READ-MS = WS-MS - WS-T0
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-RECS
               PERFORM MAKE-NAME
               MOVE WS-NAME TO CUST-NAME
               READ CUSTOMERS
               MOVE N"Ilhéus – Bahia, 日本" TO CUST-CITY
               REWRITE CUST-REC
               IF WS-FS = "00" ADD 1 TO WS-REWRITTEN END-IF
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-REWRITE-MS = WS-MS - WS-T0
           CLOSE CUSTOMERS
           IF WS-READ = WS-RECS ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL K02 read " WS-READ
           END-IF
           IF WS-REWRITTEN = WS-RECS ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL K03 rewrite " WS-REWRITTEN
           END-IF
      *>   K04 - reopened, scanned in key order: every key ascending, every
      *>   field the rewritten text.
           OPEN INPUT CUSTOMERS
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           MOVE LOW-VALUES TO CUST-NAME
           MOVE LOW-VALUES TO WS-PREV
           START CUSTOMERS KEY IS NOT LESS THAN CUST-NAME
           PERFORM UNTIL WS-FS NOT = "00"
               READ CUSTOMERS NEXT
               IF WS-FS = "00"
                   ADD 1 TO WS-SCANNED
                   IF CUST-CITY NOT = N"Ilhéus – Bahia, 日本"
                      OR CUST-NAME NOT > WS-PREV
                       ADD 1 TO WS-BAD
                   END-IF
                   MOVE CUST-NAME TO WS-PREV
               END-IF
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-SCAN-MS = WS-MS - WS-T0
           CLOSE CUSTOMERS
           IF WS-SCANNED = WS-RECS AND WS-BAD = 0 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL
                DISPLAY "FAIL K04 scanned " WS-SCANNED " bad " WS-BAD
           END-IF
           DISPLAY "=== NATIONAL INDEXED RESULT ==="
           DISPLAY "cases: K01 record PIC N(20) + PIC N(40) = 120 bytes;"
           DISPLAY "  K02 READ by a national key; K03 REWRITE;"
           DISPLAY "  K04 reopen, START + READ NEXT in key order, every"
           DISPLAY "      character checked"
           DISPLAY "rows: written " WS-RECS " read " WS-READ
                   " rewritten " WS-REWRITTEN " scanned " WS-SCANNED
           IF WS-WRITE-MS = 0 MOVE 1 TO WS-WRITE-MS END-IF
           COMPUTE WS-RATE = WS-RECS * 1000 / WS-WRITE-MS
           DISPLAY "write:   " WS-WRITE-MS " ms, " WS-RATE " records/s"
           IF WS-READ-MS = 0 MOVE 1 TO WS-READ-MS END-IF
           COMPUTE WS-RATE = WS-RECS * 1000 / WS-READ-MS
           DISPLAY "read:    " WS-READ-MS " ms, " WS-RATE " records/s"
           IF WS-REWRITE-MS = 0 MOVE 1 TO WS-REWRITE-MS END-IF
           COMPUTE WS-RATE = WS-RECS * 1000 / WS-REWRITE-MS
           DISPLAY "rewrite: " WS-REWRITE-MS " ms, " WS-RATE
                   " records/s"
           IF WS-SCAN-MS = 0 MOVE 1 TO WS-SCAN-MS END-IF
           COMPUTE WS-RATE = WS-RECS * 1000 / WS-SCAN-MS
           DISPLAY "scan:    " WS-SCAN-MS " ms, " WS-RATE " records/s"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
      *>   WS-NAME = "Cliente-nnnnn-ação", nnnnn the record number.
       MAKE-NAME.
           MOVE WS-I TO WS-SEQ
           MOVE SPACES TO WS-NAME
           STRING N"Cliente-" DELIMITED BY SIZE
                  WS-SEQ DELIMITED BY SIZE
                  N"-ação" DELIMITED BY SIZE
                  INTO WS-NAME.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.

      *> ───────────────────────────────────────────────────────────
      *>  This code was generated automatically by PowerRustCOBOL RAD.
      *>
      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated on every Build of
      *>  the project, so manual edits are lost. Edit the form and its
      *>  data binding in the Form Designer instead.
      *>
      *>  PowerRustCOBOL and its components are distributed under the
      *>  Apache 2.0 License.
      *> ───────────────────────────────────────────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. RC-6-ORDERS-9-GRID-CUST-SAVE.
      *> SAVE of DataGrid GRID-CUST on form ORDERS (binding customers),
      *> run on the backend over the indexed file CUSTOMER-FILE (indexed/customers.cidx).

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUST-ID
               STORAGE MODE IS DISK.

       DATA DIVISION.
       FILE SECTION.
       FD  CUSTOMER-FILE
           RECORD CONTAINS 30 CHARACTERS.
           01 CUSTOMER-REC.
               05 CUST-ID PIC 9(5).
               05 CUST-NAME PIC X(20).
               05 CUST-BALANCE PIC S9(5)V99 USAGE IS COMP-3.
               05 CUST-ACTIVE PIC 9.
       WORKING-STORAGE SECTION.
       01 RC-FILE-STATUS PIC XX.
       01 RC-CHANGE PIC 9(4) VALUE 0.
       01 RC-WHY PIC X(256) VALUE SPACES.
       01 RC-FOUND PIC 9 VALUE 0.
       LINKAGE SECTION.
       01 RC-PARAMS.
          05 CHANGE-COUNT PIC 9(4).
          05 CHANGES OCCURS 0 TO 100 TIMES
                DEPENDING ON CHANGE-COUNT.
             10 CHANGE-ACTION PIC X(6).
             10 KEY-COUNT PIC 9(3).
             10 KEYS OCCURS 0 TO 32 TIMES
                   DEPENDING ON KEY-COUNT.
                15 KEY-TEXT PIC X(128).
             10 CELL-COUNT PIC 9(3).
             10 CELLS OCCURS 0 TO 32 TIMES
                   DEPENDING ON CELL-COUNT.
                15 CELL-TEXT PIC X(128).
       01 RC-RESULT.
          05 COMMIT-STATUS PIC X(2).
          05 COMMIT-MESSAGE PIC X(256).
          05 FAILED-CHANGE PIC 9(4).

       PROCEDURE DIVISION USING RC-PARAMS RETURNING RC-RESULT.
       RC-MAIN.
           MOVE "00" TO COMMIT-STATUS
           MOVE SPACES TO COMMIT-MESSAGE
           MOVE 0 TO FAILED-CHANGE
      *>   A save never makes the file (OPEN I-O would): no file is 35,
      *>   as on the desktop.
           OPEN INPUT CUSTOMER-FILE
           COBOL::"FILE-STATUS" ( "CUSTOMER-FILE" RC-FILE-STATUS )
           IF RC-FILE-STATUS = "35"
               MOVE "35" TO COMMIT-STATUS
               MOVE "the file does not exist" TO COMMIT-MESSAGE
               GOBACK
           END-IF
           IF RC-FILE-STATUS(1:1) = "0"
               CLOSE CUSTOMER-FILE
           END-IF
           OPEN I-O CUSTOMER-FILE
           COBOL::"FILE-STATUS" ( "CUSTOMER-FILE" RC-FILE-STATUS )
           IF RC-FILE-STATUS(1:1) NOT = "0"
               MOVE RC-FILE-STATUS TO COMMIT-STATUS
               MOVE "the file cannot be opened for writing" TO COMMIT-MESSAGE
               GOBACK
           END-IF
      *>   Every change is checked before the first one is written.
           PERFORM RC-CHECK-CHANGE VARYING RC-CHANGE FROM 1 BY 1
               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = "00"
           IF COMMIT-STATUS = "00"
               PERFORM RC-APPLY-CHANGE VARYING RC-CHANGE FROM 1 BY 1
                   UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = "00"
           END-IF
      *>   All or nothing: a write that failed undoes the ones before it.
           IF COMMIT-STATUS = "00"
               COMMIT
           ELSE
               ROLLBACK
           END-IF
           CLOSE CUSTOMER-FILE
           GOBACK.

       RC-REFUSE.
           MOVE RC-WHY TO COMMIT-MESSAGE
           MOVE RC-CHANGE TO FAILED-CHANGE.

       RC-CHECK-CHANGE.
           MOVE SPACES TO RC-WHY
           EVALUATE CHANGE-ACTION (RC-CHANGE)
               WHEN "UPDATE"
                   IF RC-WHY = SPACES AND KEY-COUNT (RC-CHANGE) NOT = 1
                       MOVE "the change does not carry 1 key cells" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES AND CELL-COUNT (RC-CHANGE) NOT = 4
                       MOVE "the change does not carry 4 cells" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-CHECK-KEYS
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-CHECK-CELLS
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-KEY-UNCHANGED
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-KEY-FROM-KEYS
                       PERFORM RC-FIND
                       IF RC-FOUND = 0
                           MOVE "23" TO COMMIT-STATUS
                           MOVE "the record is no longer in the file" TO RC-WHY
                       END-IF
                   END-IF
               WHEN "DELETE"
                   IF RC-WHY = SPACES AND KEY-COUNT (RC-CHANGE) NOT = 1
                       MOVE "the change does not carry 1 key cells" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-CHECK-KEYS
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-KEY-FROM-KEYS
                       PERFORM RC-FIND
                       IF RC-FOUND = 0
                           MOVE "23" TO COMMIT-STATUS
                           MOVE "a deleted row is no longer in the file" TO RC-WHY
                       END-IF
                   END-IF
               WHEN "INSERT"
                   IF RC-WHY = SPACES AND CELL-COUNT (RC-CHANGE) NOT = 4
                       MOVE "the change does not carry 4 cells" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-CHECK-CELLS
                   END-IF
                   IF RC-WHY = SPACES AND CELL-TEXT (RC-CHANGE 1) = SPACES
                       MOVE "'CUST-ID' is part of the key; a new row needs it" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-KEY-FROM-CELLS
                       PERFORM RC-FIND
                       IF RC-FOUND = 1
                           MOVE "22" TO COMMIT-STATUS
                           MOVE "a record with this key is already in the file" TO RC-WHY
                       END-IF
                   END-IF
               WHEN OTHER
                   MOVE "the change is not UPDATE, INSERT or DELETE" TO RC-WHY
           END-EVALUATE
           IF RC-WHY NOT = SPACES
               IF COMMIT-STATUS = "00"
                   MOVE "92" TO COMMIT-STATUS
               END-IF
               PERFORM RC-REFUSE
           END-IF.

       RC-CHECK-KEYS.
           IF RC-WHY = SPACES
               EVALUATE TRUE
                   WHEN KEY-TEXT (RC-CHANGE 1) = SPACES
                       MOVE "CUST-ID: value required" TO RC-WHY
                   WHEN FUNCTION TEST-NUMVAL(KEY-TEXT (RC-CHANGE 1)) NOT = 0
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION NUMVAL(KEY-TEXT (RC-CHANGE 1)) NOT = FUNCTION INTEGER-PART(FUNCTION NUMVAL(KEY-TEXT (RC-CHANGE 1)))
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION NUMVAL(KEY-TEXT (RC-CHANGE 1)) < 0
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION ABS(FUNCTION NUMVAL(KEY-TEXT (RC-CHANGE 1))) >= 100000
                       MOVE "CUST-ID: value too large for PIC" TO RC-WHY
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-IF
           CONTINUE.

       RC-CHECK-CELLS.
           IF RC-WHY = SPACES
               EVALUATE TRUE
                   WHEN CELL-TEXT (RC-CHANGE 1) = SPACES
                       MOVE "CUST-ID: value required" TO RC-WHY
                   WHEN FUNCTION TEST-NUMVAL(CELL-TEXT (RC-CHANGE 1)) NOT = 0
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1)) NOT = FUNCTION INTEGER-PART(FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1)))
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1)) < 0
                       MOVE "CUST-ID: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION ABS(FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1))) >= 100000
                       MOVE "CUST-ID: value too large for PIC" TO RC-WHY
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-IF
           IF RC-WHY = SPACES
               IF CELL-TEXT (RC-CHANGE 2)(21:) NOT = SPACES
                   MOVE "CUST-NAME: value length exceeds PIC" TO RC-WHY
               END-IF
           END-IF
           IF RC-WHY = SPACES
               EVALUATE TRUE
                   WHEN CELL-TEXT (RC-CHANGE 3) = SPACES
                       MOVE "CUST-BALANCE: value required" TO RC-WHY
                   WHEN FUNCTION TEST-NUMVAL(CELL-TEXT (RC-CHANGE 3)) NOT = 0
                       MOVE "CUST-BALANCE: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 3)) NOT = FUNCTION INTEGER-PART(FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 3)))
                       MOVE "CUST-BALANCE: invalid characters for PIC" TO RC-WHY
                   WHEN FUNCTION ABS(FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 3))) >= 10000000
                       MOVE "CUST-BALANCE: value too large for PIC" TO RC-WHY
                   WHEN OTHER
                       CONTINUE
               END-EVALUATE
           END-IF
           IF RC-WHY = SPACES
               EVALUATE CELL-TEXT (RC-CHANGE 4)
                   WHEN "1"
                   WHEN "Y"
                   WHEN "y"
                   WHEN "YES"
                   WHEN "yes"
                   WHEN "TRUE"
                   WHEN "true"
                   WHEN "0"
                   WHEN "N"
                   WHEN "n"
                   WHEN "NO"
                   WHEN "no"
                   WHEN "FALSE"
                   WHEN "false"
                   WHEN SPACES
                       CONTINUE
                   WHEN OTHER
                       MOVE "CUST-ACTIVE: indicator must be 0 or 1" TO RC-WHY
               END-EVALUATE
           END-IF
           CONTINUE.

       RC-KEY-UNCHANGED.
           IF FUNCTION TRIM(CELL-TEXT (RC-CHANGE 1)) NOT = FUNCTION TRIM(KEY-TEXT (RC-CHANGE 1))
               MOVE "21" TO COMMIT-STATUS
               MOVE "the key of the row was changed; a row is found by its key" TO RC-WHY
           END-IF
           CONTINUE.

       RC-KEY-FROM-KEYS.
           COMPUTE CUST-ID = FUNCTION NUMVAL(KEY-TEXT (RC-CHANGE 1))
           CONTINUE.

       RC-KEY-FROM-CELLS.
           COMPUTE CUST-ID = FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1))
           CONTINUE.

       RC-FIND.
           READ CUSTOMER-FILE
               INVALID KEY
                   MOVE 0 TO RC-FOUND
               NOT INVALID KEY
                   MOVE 1 TO RC-FOUND
           END-READ.

       RC-APPLY-CHANGE.
           MOVE SPACES TO RC-WHY
           EVALUATE CHANGE-ACTION (RC-CHANGE)
               WHEN "UPDATE"
                   PERFORM RC-KEY-FROM-KEYS
                   PERFORM RC-FIND
                   IF RC-FOUND = 1
                       MOVE CELL-TEXT (RC-CHANGE 2) TO CUST-NAME
                       COMPUTE CUST-BALANCE = FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 3)) / 100
                       EVALUATE CELL-TEXT (RC-CHANGE 4)
                           WHEN "1"
                           WHEN "Y"
                           WHEN "y"
                           WHEN "YES"
                           WHEN "yes"
                           WHEN "TRUE"
                           WHEN "true"
                               MOVE 1 TO CUST-ACTIVE
                           WHEN OTHER
                               MOVE 0 TO CUST-ACTIVE
                       END-EVALUATE
                       REWRITE CUSTOMER-REC
                           INVALID KEY CONTINUE
                       END-REWRITE
                   END-IF
               WHEN "DELETE"
                   PERFORM RC-KEY-FROM-KEYS
                   DELETE CUSTOMER-FILE
                       INVALID KEY CONTINUE
                   END-DELETE
               WHEN "INSERT"
                   INITIALIZE CUSTOMER-REC
                   COMPUTE CUST-ID = FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 1))
                   MOVE CELL-TEXT (RC-CHANGE 2) TO CUST-NAME
                   COMPUTE CUST-BALANCE = FUNCTION NUMVAL(CELL-TEXT (RC-CHANGE 3)) / 100
                   EVALUATE CELL-TEXT (RC-CHANGE 4)
                       WHEN "1"
                       WHEN "Y"
                       WHEN "y"
                       WHEN "YES"
                       WHEN "yes"
                       WHEN "TRUE"
                       WHEN "true"
                           MOVE 1 TO CUST-ACTIVE
                       WHEN OTHER
                           MOVE 0 TO CUST-ACTIVE
                   END-EVALUATE
                   WRITE CUSTOMER-REC
                       INVALID KEY CONTINUE
                   END-WRITE
           END-EVALUATE
           COBOL::"FILE-STATUS" ( "CUSTOMER-FILE" RC-FILE-STATUS )
           IF RC-FILE-STATUS(1:1) NOT = "0"
               MOVE RC-FILE-STATUS TO COMMIT-STATUS
               MOVE "the file refused the change" TO RC-WHY
               PERFORM RC-REFUSE
           END-IF.

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
       PROGRAM-ID. RC-6-ORDERS-10-GRID-ITEMS-SAVE.
      *> SAVE of DataGrid GRID-ITEMS on form ORDERS (binding items),
      *> run on the backend over the SQL query of DB-SALES on the connection 'SALES'.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SQLSTATE PIC X(5).
       01 SQLCODE PIC S9(9) COMP-5.
       01 SQLMSG PIC X(200).
       01 RC-CHANGE PIC 9(4) VALUE 0.
       01 RC-WHY PIC X(256) VALUE SPACES.
       01 RC-V1 PIC X(128).
       01 RC-I1 PIC S9(4) COMP-5.
       01 RC-V2 PIC X(128).
       01 RC-I2 PIC S9(4) COMP-5.
       01 RC-V3 PIC X(128).
       01 RC-I3 PIC S9(4) COMP-5.
       01 RC-V4 PIC X(128).
       01 RC-I4 PIC S9(4) COMP-5.
       01 RC-K1 PIC X(128).
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
      *>   Every change is checked before the database is reached.
           PERFORM RC-CHECK-CHANGE VARYING RC-CHANGE FROM 1 BY 1
               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = "00"
           IF COMMIT-STATUS NOT = "00"
               GOBACK
           END-IF
           EXEC SQL CONNECT TO 'SALES' AS RC-CONN END-EXEC
           IF SQLSTATE NOT = "00000"
               MOVE "90" TO COMMIT-STATUS
               MOVE SQLMSG TO COMMIT-MESSAGE
               GOBACK
           END-IF
           PERFORM RC-APPLY-CHANGE VARYING RC-CHANGE FROM 1 BY 1
               UNTIL RC-CHANGE > CHANGE-COUNT OR COMMIT-STATUS NOT = "00"
      *>   One unit: all of it, or none of it.
           IF COMMIT-STATUS = "00"
               EXEC SQL COMMIT END-EXEC
               IF SQLSTATE NOT = "00000"
                   MOVE "90" TO COMMIT-STATUS
                   MOVE SQLMSG TO COMMIT-MESSAGE
                   EXEC SQL ROLLBACK END-EXEC
               END-IF
           ELSE
               EXEC SQL ROLLBACK END-EXEC
           END-IF
           EXEC SQL DISCONNECT RC-CONN END-EXEC
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
                       PERFORM RC-CHECK-CELLS
                   END-IF
                   IF RC-WHY = SPACES AND FUNCTION TRIM(CELL-TEXT (RC-CHANGE 1)) NOT = FUNCTION TRIM(KEY-TEXT (RC-CHANGE 1))
                       MOVE "21" TO COMMIT-STATUS
                       MOVE "the key of the row was changed; a row is found by its key" TO RC-WHY
                   END-IF
               WHEN "DELETE"
                   IF RC-WHY = SPACES AND KEY-COUNT (RC-CHANGE) NOT = 1
                       MOVE "the change does not carry 1 key cells" TO RC-WHY
                   END-IF
               WHEN "INSERT"
                   IF RC-WHY = SPACES AND CELL-COUNT (RC-CHANGE) NOT = 4
                       MOVE "the change does not carry 4 cells" TO RC-WHY
                   END-IF
                   IF RC-WHY = SPACES
                       PERFORM RC-CHECK-CELLS
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

       RC-CHECK-CELLS.
           IF RC-WHY = SPACES AND CELL-TEXT (RC-CHANGE 1) NOT = SPACES
               IF FUNCTION TEST-NUMVAL(CELL-TEXT (RC-CHANGE 1)) NOT = 0
                   MOVE "ID: the value is not a number" TO RC-WHY
               END-IF
           END-IF
           IF RC-WHY = SPACES AND CELL-TEXT (RC-CHANGE 3) NOT = SPACES
               IF FUNCTION TEST-NUMVAL(CELL-TEXT (RC-CHANGE 3)) NOT = 0
                   MOVE "PRICE: the value is not a number" TO RC-WHY
               END-IF
           END-IF
           IF RC-WHY = SPACES
               EVALUATE FUNCTION LOWER-CASE(FUNCTION TRIM(CELL-TEXT (RC-CHANGE 4)))
                   WHEN "1"
                   WHEN "true"
                   WHEN "yes"
                   WHEN "y"
                   WHEN "0"
                   WHEN "false"
                   WHEN "no"
                   WHEN "n"
                       CONTINUE
                   WHEN OTHER
                       MOVE "IN_STOCK: the value is not a yes/no value" TO RC-WHY
               END-EVALUATE
           END-IF
           CONTINUE.

       RC-TAKE-VALUES.
           MOVE CELL-TEXT (RC-CHANGE 1) TO RC-V1
           MOVE 0 TO RC-I1
           IF RC-V1 = SPACES
               MOVE -1 TO RC-I1
           END-IF
           MOVE CELL-TEXT (RC-CHANGE 2) TO RC-V2
           MOVE 0 TO RC-I2
           MOVE CELL-TEXT (RC-CHANGE 3) TO RC-V3
           MOVE 0 TO RC-I3
           IF RC-V3 = SPACES
               MOVE -1 TO RC-I3
           END-IF
           MOVE CELL-TEXT (RC-CHANGE 4) TO RC-V4
           MOVE 0 TO RC-I4
           EVALUATE FUNCTION LOWER-CASE(FUNCTION TRIM(RC-V4))
               WHEN "1"
               WHEN "true"
               WHEN "yes"
               WHEN "y"
                   MOVE "1" TO RC-V4
               WHEN OTHER
                   MOVE "0" TO RC-V4
           END-EVALUATE
           MOVE KEY-TEXT (RC-CHANGE 1) TO RC-K1
           CONTINUE.

       RC-APPLY-CHANGE.
           PERFORM RC-TAKE-VALUES
           EVALUATE CHANGE-ACTION (RC-CHANGE)
               WHEN "UPDATE"
                   EXEC SQL UPDATE ITEMS
                       SET NAME = :RC-V2:RC-I2,
                           PRICE = :RC-V3:RC-I3,
                           IN_STOCK = :RC-V4:RC-I4
                       WHERE ID = :RC-K1
                   END-EXEC
               WHEN "INSERT"
                   EXEC SQL INSERT INTO ITEMS (ID, NAME, PRICE, IN_STOCK)
                       VALUES (:RC-V1:RC-I1, :RC-V2:RC-I2, :RC-V3:RC-I3, :RC-V4:RC-I4)
                   END-EXEC
               WHEN "DELETE"
                   EXEC SQL DELETE FROM ITEMS
                       WHERE ID = :RC-K1
                   END-EXEC
           END-EVALUATE
           EVALUATE TRUE
               WHEN SQLSTATE = "02000"
                   MOVE "23" TO COMMIT-STATUS
                   MOVE "no row has this key" TO RC-WHY
                   PERFORM RC-REFUSE
               WHEN SQLSTATE(1:2) = "00" OR SQLSTATE(1:2) = "01"
                   CONTINUE
               WHEN OTHER
                   MOVE "90" TO COMMIT-STATUS
                   MOVE SQLMSG TO RC-WHY
                   PERFORM RC-REFUSE
           END-EVALUATE.

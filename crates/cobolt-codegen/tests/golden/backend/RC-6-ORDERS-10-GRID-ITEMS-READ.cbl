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
       PROGRAM-ID. RC-6-ORDERS-10-GRID-ITEMS-READ.
      *> READ of DataGrid GRID-ITEMS on form ORDERS (binding items),
      *> run on the backend over the SQL query of DB-SALES on the connection 'SALES'.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SQLSTATE PIC X(5).
       01 SQLCODE PIC S9(9) COMP-5.
       01 SQLMSG PIC X(200).
       01 RC-TOTAL PIC 9(9) VALUE 0.
       01 RC-SEEN PIC 9(9) VALUE 0.
       01 RC-ROW PIC 9(4) VALUE 0.
       01 RC-WANT PIC 9(4) VALUE 0.
       01 RC-DONE PIC 9 VALUE 0.
       01 RC-V1 PIC X(128).
       01 RC-I1 PIC S9(4) COMP-5.
       01 RC-V2 PIC X(128).
       01 RC-I2 PIC S9(4) COMP-5.
       01 RC-V3 PIC X(128).
       01 RC-I3 PIC S9(4) COMP-5.
       01 RC-V4 PIC X(128).
       01 RC-I4 PIC S9(4) COMP-5.
       LINKAGE SECTION.
       01 RC-PARAMS.
          05 ROW-OFFSET PIC 9(9).
          05 ROW-COUNT PIC 9(4).
          05 PARAM-COUNT PIC 9(2).
          05 PARAMS OCCURS 0 TO 16 TIMES
                DEPENDING ON PARAM-COUNT.
             10 PARAM-VALUE PIC X(256).
       01 RC-RESULT.
          05 READ-STATUS PIC X(2).
          05 READ-MESSAGE PIC X(256).
          05 ROW-TOTAL PIC 9(9).
          05 ROWS-RETURNED PIC 9(4).
          05 ROWS OCCURS 0 TO 100 TIMES
                DEPENDING ON ROWS-RETURNED.
             10 CELL-COUNT PIC 9(3).
             10 CELLS OCCURS 0 TO 32 TIMES
                   DEPENDING ON CELL-COUNT.
                15 CELL-TEXT PIC X(128).

       PROCEDURE DIVISION USING RC-PARAMS RETURNING RC-RESULT.
       RC-MAIN.
           MOVE "00" TO READ-STATUS
           MOVE SPACES TO READ-MESSAGE
           MOVE 0 TO ROW-TOTAL ROWS-RETURNED
           MOVE FUNCTION MIN(ROW-COUNT 100) TO RC-WANT
           IF PARAM-COUNT NOT = 0
               MOVE "92" TO READ-STATUS
               MOVE "this source takes no parameters" TO READ-MESSAGE
               GOBACK
           END-IF
           EXEC SQL CONNECT TO 'SALES' AS RC-CONN END-EXEC
           IF SQLSTATE NOT = "00000"
               MOVE "90" TO READ-STATUS
               MOVE SQLMSG TO READ-MESSAGE
               GOBACK
           END-IF
           EXEC SQL SELECT COUNT(*) INTO :RC-TOTAL
               FROM (
                   SELECT ID, NAME, PRICE, IN_STOCK
                   FROM ITEMS
                   ORDER BY ID
               ) AS RC_Q
           END-EXEC
           IF SQLSTATE NOT = "00000"
               PERFORM RC-FAILED
               GOBACK
           END-IF
           MOVE RC-TOTAL TO ROW-TOTAL
      *>   The bound fields, found in the query's result by name.
           EXEC SQL DECLARE RC-ROWS CURSOR FOR
               SELECT ID, NAME, PRICE, IN_STOCK
               FROM (
                   SELECT ID, NAME, PRICE, IN_STOCK
                   FROM ITEMS
                   ORDER BY ID
               ) AS RC_Q
           END-EXEC
           EXEC SQL OPEN RC-ROWS END-EXEC
           IF SQLSTATE NOT = "00000"
               PERFORM RC-FAILED
               GOBACK
           END-IF
           IF RC-WANT = 0
               MOVE 1 TO RC-DONE
           END-IF
           PERFORM UNTIL RC-DONE = 1
               MOVE SPACES TO RC-V1 RC-V2 RC-V3 RC-V4
               EXEC SQL FETCH RC-ROWS INTO :RC-V1:RC-I1, :RC-V2:RC-I2, :RC-V3:RC-I3, :RC-V4:RC-I4
               END-EXEC
               EVALUATE TRUE
                   WHEN SQLSTATE = "02000"
                       MOVE 1 TO RC-DONE
                   WHEN SQLSTATE(1:2) = "00" OR SQLSTATE(1:2) = "01"
                       PERFORM RC-TAKE-ROW
                   WHEN OTHER
                       PERFORM RC-FAILED
                       MOVE 1 TO RC-DONE
               END-EVALUATE
           END-PERFORM
           EXEC SQL CLOSE RC-ROWS END-EXEC
           EXEC SQL DISCONNECT RC-CONN END-EXEC
           GOBACK.

       RC-TAKE-ROW.
           ADD 1 TO RC-SEEN
           IF RC-SEEN > ROW-OFFSET
               ADD 1 TO ROWS-RETURNED
               MOVE ROWS-RETURNED TO RC-ROW
               MOVE 4 TO CELL-COUNT (RC-ROW)
               MOVE RC-V1 TO CELL-TEXT (RC-ROW 1)
               MOVE RC-V2 TO CELL-TEXT (RC-ROW 2)
               MOVE RC-V3 TO CELL-TEXT (RC-ROW 3)
               MOVE RC-V4 TO CELL-TEXT (RC-ROW 4)
               IF ROWS-RETURNED >= RC-WANT
                   MOVE 1 TO RC-DONE
               END-IF
           END-IF.

       RC-FAILED.
           MOVE "90" TO READ-STATUS
           MOVE SQLMSG TO READ-MESSAGE
           MOVE 0 TO ROWS-RETURNED
           EXEC SQL DISCONNECT RC-CONN END-EXEC.

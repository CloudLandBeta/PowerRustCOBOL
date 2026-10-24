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
       PROGRAM-ID. RC-6-ORDERS-9-GRID-CUST-READ.
      *> READ of DataGrid GRID-CUST on form ORDERS (binding customers),
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
       01 RC-SEEN PIC 9(9) VALUE 0.
       01 RC-ROW PIC 9(4) VALUE 0.
       01 RC-WANT PIC 9(4) VALUE 0.
       01 RC-AT-END PIC 9 VALUE 0.
       01 RC-NUMBER PIC -(37)9.
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
           OPEN INPUT CUSTOMER-FILE
           COBOL::"FILE-STATUS" ( "CUSTOMER-FILE" RC-FILE-STATUS )
           IF RC-FILE-STATUS = "35"
      *>       No file yet is an empty grid, as on the desktop.
               MOVE "35" TO READ-STATUS
               MOVE "the file does not exist yet" TO READ-MESSAGE
               GOBACK
           END-IF
           IF RC-FILE-STATUS(1:1) NOT = "0"
               MOVE RC-FILE-STATUS TO READ-STATUS
               MOVE "the file cannot be opened" TO READ-MESSAGE
               GOBACK
           END-IF
           PERFORM UNTIL RC-AT-END = 1
               READ CUSTOMER-FILE NEXT
                   AT END
                       MOVE 1 TO RC-AT-END
                   NOT AT END
                       PERFORM RC-TAKE-RECORD
               END-READ
               COBOL::"FILE-STATUS" ( "CUSTOMER-FILE" RC-FILE-STATUS )
               IF RC-FILE-STATUS(1:1) NOT = "0"
                   MOVE 1 TO RC-AT-END
               END-IF
           END-PERFORM
           IF RC-FILE-STATUS NOT = "10"
               MOVE RC-FILE-STATUS TO READ-STATUS
               MOVE "the file cannot be read" TO READ-MESSAGE
           END-IF
           MOVE RC-SEEN TO ROW-TOTAL
           CLOSE CUSTOMER-FILE
           GOBACK.

       RC-TAKE-RECORD.
           ADD 1 TO RC-SEEN
           IF RC-SEEN > ROW-OFFSET AND ROWS-RETURNED < RC-WANT
               ADD 1 TO ROWS-RETURNED
               MOVE ROWS-RETURNED TO RC-ROW
               MOVE 4 TO CELL-COUNT (RC-ROW)
               MOVE CUST-ID TO RC-NUMBER
               MOVE FUNCTION TRIM(RC-NUMBER) TO CELL-TEXT (RC-ROW 1)
               MOVE CUST-NAME TO CELL-TEXT (RC-ROW 2)
               COMPUTE RC-NUMBER = CUST-BALANCE * 100
               MOVE FUNCTION TRIM(RC-NUMBER) TO CELL-TEXT (RC-ROW 3)
               IF CUST-ACTIVE = 1
                   MOVE "1" TO CELL-TEXT (RC-ROW 4)
               ELSE
                   MOVE "0" TO CELL-TEXT (RC-ROW 4)
               END-IF
           END-IF.

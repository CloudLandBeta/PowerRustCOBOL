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
       PROGRAM-ID. RC-4-SHOP-4-DB-1-QUERY-1.
      *> Query of SqlDatabase DB-1 on form SHOP (call site 1), run on the backend
      *> over the connection 'SALES'.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 RC-EVENT PIC X(64).
       01 RC-SOURCE PIC X(64).
       01 RC-BUSY PIC X(8).
       01 RC-WAITING PIC 9 VALUE 0.
       01 RC-KEEP PIC 9 VALUE 1.
       01 RC-OPENED PIC 9 VALUE 0.
       01 RC-LINE PIC X(8192).
       01 RC-COLS PIC 9(3) VALUE 0.
       01 RC-C PIC 9(3) VALUE 0.
       01 RC-PTR PIC 9(5) VALUE 0.
       01 RC-ROW PIC 9(4) VALUE 0.
       01 RC-DONE PIC 9 VALUE 0.
       LINKAGE SECTION.
       01 RC-PARAMS.
          05 ARG-COUNT PIC 9(2).
          05 ARGS OCCURS 0 TO 8 TIMES
                DEPENDING ON ARG-COUNT.
             10 ARG-VALUE PIC X(2048).
       01 RC-RESULT.
          05 RESULT-VALUE PIC X(2048).
          05 LAST-ERROR PIC X(512).
          05 PROP-COUNT PIC 9(2).
          05 PROPS OCCURS 0 TO 32 TIMES
                DEPENDING ON PROP-COUNT.
             10 PROP-NAME PIC X(64).
             10 PROP-VALUE PIC X(2048).
          05 EVENT-COUNT PIC 9(2).
          05 EVENTS OCCURS 0 TO 16 TIMES
                DEPENDING ON EVENT-COUNT.
             10 EVENT-NAME PIC X(64).
          05 ROWS-RETURNED PIC 9(4).
          05 ROWS OCCURS 0 TO 100 TIMES
                DEPENDING ON ROWS-RETURNED.
             10 CELL-COUNT PIC 9(3).
             10 CELLS OCCURS 0 TO 32 TIMES
                   DEPENDING ON CELL-COUNT.
                15 CELL-TEXT PIC X(128).

       PROCEDURE DIVISION USING RC-PARAMS RETURNING RC-RESULT.
       RC-MAIN.
           MOVE SPACES TO RESULT-VALUE LAST-ERROR
           MOVE 0 TO PROP-COUNT EVENT-COUNT ROWS-RETURNED
           IF ARG-COUNT NOT = 0
               COBOL::"WS-FAIL" ( 1 "RC-4-SHOP-4-DB-1-QUERY-1 takes 0 argument(s)" )
               GOBACK
           END-IF
      *>   No connection outlives a call: open it on the first one.
           IF RC-OPENED = 0
               MOVE 1 TO RC-OPENED
               DB-1::Open()
               MOVE 0 TO RC-KEEP
               PERFORM RC-WAIT
               MOVE 1 TO RC-KEEP
               IF DB-1::StatusCode NOT = 0
                   MOVE DB-1::LastError TO LAST-ERROR
                   MOVE 0 TO RC-OPENED
                   GOBACK
               END-IF
           END-IF
           MOVE DB-1::Query( "SELECT NAME, PRICE FROM ITEMS ORDER BY NAME" ) TO RESULT-VALUE
           PERFORM RC-WAIT
           PERFORM RC-TAKE-ROWS
           MOVE DB-1::LastError TO LAST-ERROR
           GOBACK.

      *>   Wait until the control has finished, as the desktop's event
      *>   loop would, keeping the events it raised (RC-KEEP = 1). The
      *>   backend's RC-TICK reaches this program only when no event
      *>   is queued, so a tick while the control is not busy ends it.
       RC-WAIT.
           MOVE 1 TO RC-WAITING
           PERFORM UNTIL RC-WAITING = 0
               COBOL::"WAIT-EVENT" ( RC-EVENT RC-SOURCE )
               EVALUATE TRUE
                   WHEN RC-EVENT = "RC-TICK"
                       MOVE DB-1::Busy TO RC-BUSY
                       IF RC-BUSY NOT = "1" AND RC-BUSY NOT = "true"
                           MOVE 0 TO RC-WAITING
                       END-IF
                   WHEN RC-KEEP = 1 AND EVENT-COUNT < 16
                       ADD 1 TO EVENT-COUNT
                       MOVE RC-EVENT TO EVENT-NAME (EVENT-COUNT)
               END-EVALUATE
           END-PERFORM.

      *>   No result set outlives a call: its rows travel with the answer,
      *>   fetched here; the fetches' own events are not the call's.
       RC-TAKE-ROWS.
           MOVE DB-1::ColumnCount() TO RC-COLS
           IF RC-COLS > 32
               MOVE 32 TO RC-COLS
           END-IF
           MOVE 0 TO RC-DONE
           PERFORM UNTIL RC-DONE = 1 OR ROWS-RETURNED >= 100
               MOVE DB-1::Fetch() TO RC-LINE
               IF RC-LINE = SPACES
                   MOVE 1 TO RC-DONE
               ELSE
                   ADD 1 TO ROWS-RETURNED
                   MOVE ROWS-RETURNED TO RC-ROW
                   MOVE RC-COLS TO CELL-COUNT (RC-ROW)
                   MOVE 1 TO RC-PTR
                   PERFORM VARYING RC-C FROM 1 BY 1 UNTIL RC-C > RC-COLS
                       UNSTRING RC-LINE DELIMITED BY X"09"
                           INTO CELL-TEXT (RC-ROW RC-C) WITH POINTER RC-PTR
                       END-UNSTRING
                   END-PERFORM
               END-IF
           END-PERFORM
           MOVE 0 TO RC-KEEP
           PERFORM RC-WAIT
           MOVE 1 TO RC-KEEP.

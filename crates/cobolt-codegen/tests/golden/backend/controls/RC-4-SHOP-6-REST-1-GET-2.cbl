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
       PROGRAM-ID. RC-4-SHOP-6-REST-1-GET-2.
      *> Get of RestClient REST-1 on form SHOP (call site 2), run on the backend
      *> with the configuration 'CRM' of backend.toml.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 RC-EVENT PIC X(64).
       01 RC-SOURCE PIC X(64).
       01 RC-BUSY PIC X(8).
       01 RC-WAITING PIC 9 VALUE 0.
       01 RC-KEEP PIC 9 VALUE 1.
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
               COBOL::"WS-FAIL" ( 1 "RC-4-SHOP-6-REST-1-GET-2 takes 0 argument(s)" )
               GOBACK
           END-IF
           MOVE REST-1::Get( "customers/7" ) TO RESULT-VALUE
           PERFORM RC-WAIT
           MOVE REST-1::LastError TO LAST-ERROR
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
                       MOVE REST-1::Busy TO RC-BUSY
                       IF RC-BUSY NOT = "1" AND RC-BUSY NOT = "true"
                           MOVE 0 TO RC-WAITING
                       END-IF
                   WHEN RC-KEEP = 1 AND EVENT-COUNT < 16
                       ADD 1 TO EVENT-COUNT
                       MOVE RC-EVENT TO EVENT-NAME (EVENT-COUNT)
               END-EVALUATE
           END-PERFORM.

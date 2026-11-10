      *> pq-queue: the PartiQL statements on QUEUE-FILE (queue.FD) and the reading of its rows into QUEUE-RECORD. The
      *> program declares the file, COPYs ws-partiql, and keeps the file open while it uses these paragraphs. A
      *> statement on QUEUE-FILE replaces the rows the one before it answered.

      *> GET-QUEUE-ROW: the queue row whose key is in QUE-ID, read into QUEUE-RECORD.
      *> In:  QUE-ID.
      *> Out: WS-PQ-STATUS = 00 and QUEUE-RECORD filled, or 23 when there is no such queue row (the record is left as
      *>      it was).
       GET-QUEUE-ROW.
           COBOL::"PARTIQL" ( "QUEUE-FILE" "SELECT * FROM QUEUE-FILE WHERE QUE-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QUE-ID )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-QUEUE-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-QUEUE-OF-QUOTE: the services of quote QUE-QUO-ID, in the order they were queued;
      *> FETCH-QUEUE-ROW reads them.
       SELECT-QUEUE-OF-QUOTE.
           COBOL::"PARTIQL" ( "QUEUE-FILE" "SELECT * FROM QUEUE-FILE.QUE-QUO-ID WHERE QUE-QUO-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QUE-QUO-ID ).

      *> SELECT-ALL-QUEUE: every row of the queue, in the order of QUE-ID; FETCH-QUEUE-ROW reads them.
       SELECT-ALL-QUEUE.
           COBOL::"PARTIQL" ( "QUEUE-FILE" "SELECT * FROM QUEUE-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-QUEUE-ROW: the next row the last statement on QUEUE-FILE answered, read into QUEUE-RECORD.
      *> Out: WS-PQ-STATUS = 00 and QUEUE-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-QUEUE-ROW.
           COBOL::"PARTIQL-FETCH" ( "QUEUE-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO QUE-ID QUE-QUO-ID QUE-LINE QUE-PLATE QUE-SERVICE QUE-STATE QUE-PRIORITY QUE-APPROVED-AT
                        QUE-DEADLINE QUE-STARTED QUE-DONE QUE-BY
               END-UNSTRING
           END-IF.

      *> INSERT-QUEUE-ROW: writes QUEUE-RECORD as a new queue row, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a queue row with that key exists.
       INSERT-QUEUE-ROW.
           COBOL::"PARTIQL" ( "QUEUE-FILE"
                   "INSERT INTO QUEUE-FILE VALUE {'QUE-ID': ?, 'QUE-QUO-ID': ?, 'QUE-LINE': ?, 'QUE-PLATE': ?, "
                 & "'QUE-SERVICE': ?, 'QUE-STATE': ?, 'QUE-PRIORITY': ?, 'QUE-APPROVED-AT': ?, 'QUE-DEADLINE': ?, "
                 & "'QUE-STARTED': ?, 'QUE-DONE': ?, 'QUE-BY': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS QUE-ID QUE-QUO-ID QUE-LINE QUE-PLATE QUE-SERVICE QUE-STATE QUE-PRIORITY
                   QUE-APPROVED-AT QUE-DEADLINE QUE-STARTED QUE-DONE QUE-BY ).

      *> UPDATE-QUEUE-ROW: rewrites the queue row whose key is in QUE-ID with every other field of QUEUE-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such queue row.
       UPDATE-QUEUE-ROW.
           COBOL::"PARTIQL" ( "QUEUE-FILE"
                   "UPDATE QUEUE-FILE SET QUE-QUO-ID = ? SET QUE-LINE = ? SET QUE-PLATE = ? SET QUE-SERVICE = ? "
                 & "SET QUE-STATE = ? SET QUE-PRIORITY = ? SET QUE-APPROVED-AT = ? SET QUE-DEADLINE = ? "
                 & "SET QUE-STARTED = ? SET QUE-DONE = ? SET QUE-BY = ? WHERE QUE-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QUE-QUO-ID QUE-LINE QUE-PLATE QUE-SERVICE QUE-STATE QUE-PRIORITY
                   QUE-APPROVED-AT QUE-DEADLINE QUE-STARTED QUE-DONE QUE-BY QUE-ID ).

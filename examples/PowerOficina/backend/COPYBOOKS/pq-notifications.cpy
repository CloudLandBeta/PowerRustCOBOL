      *> pq-notifications: the PartiQL statements on NOTIFY-FILE (notifications.FD) and the reading of its rows into
      *> NOTIFY-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on NOTIFY-FILE replaces the rows the one before it answered.

      *> SELECT-ALL-NOTICES: every notice, in the order of NTF-ID; FETCH-NOTIFY-ROW reads them.
       SELECT-ALL-NOTICES.
           COBOL::"PARTIQL" ( "NOTIFY-FILE" "SELECT * FROM NOTIFY-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-NOTIFY-ROW: the next row the last statement on NOTIFY-FILE answered, read into NOTIFY-RECORD.
      *> Out: WS-PQ-STATUS = 00 and NOTIFY-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-NOTIFY-ROW.
           COBOL::"PARTIQL-FETCH" ( "NOTIFY-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO NTF-ID NTF-QUO-ID NTF-CHANNEL NTF-KIND NTF-TO NTF-AT NTF-STATUS NTF-TEXT
               END-UNSTRING
           END-IF.

      *> INSERT-NOTIFY-ROW: writes NOTIFY-RECORD as a new notice, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a notice with that key exists.
       INSERT-NOTIFY-ROW.
           COBOL::"PARTIQL" ( "NOTIFY-FILE"
                   "INSERT INTO NOTIFY-FILE VALUE {'NTF-ID': ?, 'NTF-QUO-ID': ?, 'NTF-CHANNEL': ?, 'NTF-KIND': ?, "
                 & "'NTF-TO': ?, 'NTF-AT': ?, 'NTF-STATUS': ?, 'NTF-TEXT': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS NTF-ID NTF-QUO-ID NTF-CHANNEL NTF-KIND NTF-TO NTF-AT NTF-STATUS NTF-TEXT ).

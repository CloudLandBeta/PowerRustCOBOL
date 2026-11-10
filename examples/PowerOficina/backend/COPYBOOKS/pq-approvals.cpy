      *> pq-approvals: the PartiQL statements on APPROVAL-FILE (approvals.FD) and the reading of its rows into
      *> APPROVAL-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on APPROVAL-FILE replaces the rows the one before it answered.

      *> SELECT-ALL-APPROVALS: every answer, in the order of APR-ID; FETCH-APPROVAL-ROW reads them.
       SELECT-ALL-APPROVALS.
           COBOL::"PARTIQL" ( "APPROVAL-FILE" "SELECT * FROM APPROVAL-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-APPROVAL-ROW: the next row the last statement on APPROVAL-FILE answered, read into APPROVAL-RECORD.
      *> Out: WS-PQ-STATUS = 00 and APPROVAL-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-APPROVAL-ROW.
           COBOL::"PARTIQL-FETCH" ( "APPROVAL-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO APR-ID APR-QUO-ID APR-REV APR-DECISION APR-CHANNEL APR-AT APR-REASON APR-OPERATOR
               END-UNSTRING
           END-IF.

      *> INSERT-APPROVAL-ROW: writes APPROVAL-RECORD as a new answer, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when an answer with that key exists.
       INSERT-APPROVAL-ROW.
           COBOL::"PARTIQL" ( "APPROVAL-FILE"
                   "INSERT INTO APPROVAL-FILE VALUE {'APR-ID': ?, 'APR-QUO-ID': ?, 'APR-REV': ?, "
                 & "'APR-DECISION': ?, 'APR-CHANNEL': ?, 'APR-AT': ?, 'APR-REASON': ?, 'APR-OPERATOR': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS APR-ID APR-QUO-ID APR-REV APR-DECISION APR-CHANNEL APR-AT APR-REASON
                   APR-OPERATOR ).

      *> pq-audit: the PartiQL statements on AUDIT-FILE (audit.FD) and the reading of its rows into AUDIT-RECORD. The
      *> program declares the file, COPYs ws-partiql, and keeps the file open while it uses these paragraphs. A
      *> statement on AUDIT-FILE replaces the rows the one before it answered.

      *> INSERT-AUDIT-ROW: writes AUDIT-RECORD as a new audit line, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when an audit line with that key exists.
       INSERT-AUDIT-ROW.
           COBOL::"PARTIQL" ( "AUDIT-FILE"
                   "INSERT INTO AUDIT-FILE VALUE {'AUD-ID': ?, 'AUD-AT': ?, 'AUD-OPERATOR': ?, 'AUD-ENTITY': ?, "
                 & "'AUD-REF': ?, 'AUD-ACTION': ?, 'AUD-DETAIL': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS AUD-ID AUD-AT AUD-OPERATOR AUD-ENTITY AUD-REF AUD-ACTION AUD-DETAIL ).

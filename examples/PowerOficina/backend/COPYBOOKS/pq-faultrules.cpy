      *> pq-faultrules: the PartiQL statements on FAULT-RULE-FILE (faultrules.FD) and the reading of its rows into
      *> FAULT-RULE-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses
      *> these paragraphs. A statement on FAULT-RULE-FILE replaces the rows the one before it answered.

      *> GET-FAULT-RULE-ROW: the fault rule whose key is in FLT-CODE, read into FAULT-RULE-RECORD.
      *> In:  FLT-CODE.
      *> Out: WS-PQ-STATUS = 00 and FAULT-RULE-RECORD filled, or 23 when there is no such fault rule (the record is
      *>      left as it was).
       GET-FAULT-RULE-ROW.
           COBOL::"PARTIQL" ( "FAULT-RULE-FILE" "SELECT * FROM FAULT-RULE-FILE WHERE FLT-CODE = ?"
                   WS-PQ-COUNT WS-PQ-STATUS FLT-CODE )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-FAULT-RULE-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-ALL-FAULT-RULES: every fault rule, in code order; FETCH-FAULT-RULE-ROW reads them.
       SELECT-ALL-FAULT-RULES.
           COBOL::"PARTIQL" ( "FAULT-RULE-FILE" "SELECT * FROM FAULT-RULE-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-FAULT-RULE-ROW: the next row the last statement on FAULT-RULE-FILE answered, read into
      *> FAULT-RULE-RECORD.
      *> Out: WS-PQ-STATUS = 00 and FAULT-RULE-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-FAULT-RULE-ROW.
           COBOL::"PARTIQL-FETCH" ( "FAULT-RULE-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO FLT-CODE FLT-DESC FLT-PART-1 FLT-PART-2 FLT-PART-3 FLT-SERVICE
               END-UNSTRING
           END-IF.

      *> INSERT-FAULT-RULE-ROW: writes FAULT-RULE-RECORD as a new fault rule, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a fault rule with that key exists.
       INSERT-FAULT-RULE-ROW.
           COBOL::"PARTIQL" ( "FAULT-RULE-FILE"
                   "INSERT INTO FAULT-RULE-FILE VALUE {'FLT-CODE': ?, 'FLT-DESC': ?, 'FLT-PART-1': ?, "
                 & "'FLT-PART-2': ?, 'FLT-PART-3': ?, 'FLT-SERVICE': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS FLT-CODE FLT-DESC FLT-PART-1 FLT-PART-2 FLT-PART-3 FLT-SERVICE ).

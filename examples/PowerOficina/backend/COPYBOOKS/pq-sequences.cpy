      *> pq-sequences: the PartiQL statements on SEQUENCE-FILE (sequences.FD) and the reading of its rows into
      *> SEQUENCE-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on SEQUENCE-FILE replaces the rows the one before it answered.

      *> GET-SEQUENCE-ROW: the sequence whose key is in SEQ-NAME, read into SEQUENCE-RECORD.
      *> In:  SEQ-NAME.
      *> Out: WS-PQ-STATUS = 00 and SEQUENCE-RECORD filled, or 23 when there is no such sequence (the record is left
      *>      as it was).
       GET-SEQUENCE-ROW.
           COBOL::"PARTIQL" ( "SEQUENCE-FILE" "SELECT * FROM SEQUENCE-FILE WHERE SEQ-NAME = ?"
                   WS-PQ-COUNT WS-PQ-STATUS SEQ-NAME )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-SEQUENCE-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> FETCH-SEQUENCE-ROW: the next row the last statement on SEQUENCE-FILE answered, read into SEQUENCE-RECORD.
      *> Out: WS-PQ-STATUS = 00 and SEQUENCE-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". The last value runs to the end of
      *> WS-PQ-ROW, so it goes through WS-PQ-NUMBER-1 and NUMVAL.
       FETCH-SEQUENCE-ROW.
           COBOL::"PARTIQL-FETCH" ( "SEQUENCE-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO SEQ-NAME WS-PQ-NUMBER-1
               END-UNSTRING
               COMPUTE SEQ-VALUE = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
           END-IF.

      *> INSERT-SEQUENCE-ROW: writes SEQUENCE-RECORD as a new sequence, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a sequence with that key exists.
       INSERT-SEQUENCE-ROW.
           COBOL::"PARTIQL" ( "SEQUENCE-FILE" "INSERT INTO SEQUENCE-FILE VALUE {'SEQ-NAME': ?, 'SEQ-VALUE': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS SEQ-NAME SEQ-VALUE ).

      *> UPDATE-SEQUENCE-ROW: rewrites the sequence whose key is in SEQ-NAME with every other field of
      *> SEQUENCE-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such sequence.
       UPDATE-SEQUENCE-ROW.
           COBOL::"PARTIQL" ( "SEQUENCE-FILE" "UPDATE SEQUENCE-FILE SET SEQ-VALUE = ? WHERE SEQ-NAME = ?"
                   WS-PQ-COUNT WS-PQ-STATUS SEQ-VALUE SEQ-NAME ).

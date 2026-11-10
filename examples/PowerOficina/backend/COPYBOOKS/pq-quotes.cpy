      *> pq-quotes: the PartiQL statements on QUOTE-FILE (quotes.FD) and the reading of its rows into QUOTE-RECORD.
      *> The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these paragraphs. A
      *> statement on QUOTE-FILE replaces the rows the one before it answered.

      *> GET-QUOTE-ROW: the quote revision whose key is in QUO-ID and QUO-REV, read into QUOTE-RECORD.
      *> In:  QUO-ID and QUO-REV.
      *> Out: WS-PQ-STATUS = 00 and QUOTE-RECORD filled, or 23 when there is no such quote revision (the record is
      *>      left as it was).
       GET-QUOTE-ROW.
           COBOL::"PARTIQL" ( "QUOTE-FILE" "SELECT * FROM QUOTE-FILE WHERE QUO-ID = ? AND QUO-REV = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QUO-ID QUO-REV )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-QUOTE-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-ALL-QUOTES: every revision of every quote, by number and revision; FETCH-QUOTE-ROW reads them.
       SELECT-ALL-QUOTES.
           COBOL::"PARTIQL" ( "QUOTE-FILE" "SELECT * FROM QUOTE-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-QUOTE-ROW: the next row the last statement on QUOTE-FILE answered, read into QUOTE-RECORD.
      *> Out: WS-PQ-STATUS = 00 and QUOTE-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point, so it
      *> goes through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-QUOTE-ROW.
           COBOL::"PARTIQL-FETCH" ( "QUOTE-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO QUO-ID QUO-REV QUO-CUS-ID QUO-PLATE QUO-DGN-ID QUO-CURRENCY QUO-STATUS QUO-CREATED
                        QUO-VALID-UNTIL QUO-DEADLINE WS-PQ-NUMBER-1 WS-PQ-NUMBER-2 WS-PQ-NUMBER-3 WS-PQ-NUMBER-4
                        QUO-OPERATOR
               END-UNSTRING
               COMPUTE QUO-HOURS = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE QUO-SUBTOTAL = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
               COMPUTE QUO-TAX = FUNCTION NUMVAL ( WS-PQ-NUMBER-3 )
               COMPUTE QUO-TOTAL = FUNCTION NUMVAL ( WS-PQ-NUMBER-4 )
           END-IF.

      *> INSERT-QUOTE-ROW: writes QUOTE-RECORD as a new quote revision, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a quote revision with that key exists.
       INSERT-QUOTE-ROW.
           COBOL::"PARTIQL" ( "QUOTE-FILE"
                   "INSERT INTO QUOTE-FILE VALUE {'QUO-ID': ?, 'QUO-REV': ?, 'QUO-CUS-ID': ?, 'QUO-PLATE': ?, "
                 & "'QUO-DGN-ID': ?, 'QUO-CURRENCY': ?, 'QUO-STATUS': ?, 'QUO-CREATED': ?, 'QUO-VALID-UNTIL': ?, "
                 & "'QUO-DEADLINE': ?, 'QUO-HOURS': ?, 'QUO-SUBTOTAL': ?, 'QUO-TAX': ?, 'QUO-TOTAL': ?, "
                 & "'QUO-OPERATOR': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS QUO-ID QUO-REV QUO-CUS-ID QUO-PLATE QUO-DGN-ID QUO-CURRENCY QUO-STATUS
                   QUO-CREATED QUO-VALID-UNTIL QUO-DEADLINE QUO-HOURS QUO-SUBTOTAL QUO-TAX QUO-TOTAL QUO-OPERATOR ).

      *> UPDATE-QUOTE-ROW: rewrites the quote revision whose key is in QUO-ID and QUO-REV with every other field of
      *> QUOTE-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such quote revision.
       UPDATE-QUOTE-ROW.
           COBOL::"PARTIQL" ( "QUOTE-FILE"
                   "UPDATE QUOTE-FILE SET QUO-CUS-ID = ? SET QUO-PLATE = ? SET QUO-DGN-ID = ? SET QUO-CURRENCY = ? "
                 & "SET QUO-STATUS = ? SET QUO-CREATED = ? SET QUO-VALID-UNTIL = ? SET QUO-DEADLINE = ? "
                 & "SET QUO-HOURS = ? SET QUO-SUBTOTAL = ? SET QUO-TAX = ? SET QUO-TOTAL = ? SET QUO-OPERATOR = ? "
                 & "WHERE QUO-ID = ? AND QUO-REV = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QUO-CUS-ID QUO-PLATE QUO-DGN-ID QUO-CURRENCY QUO-STATUS QUO-CREATED
                   QUO-VALID-UNTIL QUO-DEADLINE QUO-HOURS QUO-SUBTOTAL QUO-TAX QUO-TOTAL QUO-OPERATOR QUO-ID QUO-REV ).

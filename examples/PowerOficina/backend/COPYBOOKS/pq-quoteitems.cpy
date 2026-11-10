      *> pq-quoteitems: the PartiQL statements on QUOTE-ITEM-FILE (quoteitems.FD) and the reading of its rows into
      *> QUOTE-ITEM-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses
      *> these paragraphs. A statement on QUOTE-ITEM-FILE replaces the rows the one before it answered.

      *> SELECT-LINES-OF-REVISION: the lines of quote QIT-QUO-ID, revision QIT-REV, in line order;
      *> FETCH-QUOTE-ITEM-ROW reads them.
       SELECT-LINES-OF-REVISION.
           COBOL::"PARTIQL" ( "QUOTE-ITEM-FILE" "SELECT * FROM QUOTE-ITEM-FILE WHERE QIT-QUO-ID = ? AND QIT-REV = ?"
                   WS-PQ-COUNT WS-PQ-STATUS QIT-QUO-ID QIT-REV ).

      *> SELECT-ALL-QUOTE-ITEMS: every line of every quote, by quote, revision and line;
      *> FETCH-QUOTE-ITEM-ROW reads them.
       SELECT-ALL-QUOTE-ITEMS.
           COBOL::"PARTIQL" ( "QUOTE-ITEM-FILE" "SELECT * FROM QUOTE-ITEM-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-QUOTE-ITEM-ROW: the next row the last statement on QUOTE-ITEM-FILE answered, read into
      *> QUOTE-ITEM-RECORD.
      *> Out: WS-PQ-STATUS = 00 and QUOTE-ITEM-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point, so it
      *> goes through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-QUOTE-ITEM-ROW.
           COBOL::"PARTIQL-FETCH" ( "QUOTE-ITEM-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO QIT-QUO-ID QIT-REV QIT-LINE QIT-KIND QIT-CODE QIT-NAME WS-PQ-NUMBER-1 WS-PQ-NUMBER-2
                        WS-PQ-NUMBER-3 WS-PQ-NUMBER-4 QIT-MANUAL QIT-NOTE
               END-UNSTRING
               COMPUTE QIT-QTY = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE QIT-UNIT = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
               COMPUTE QIT-TAX-RATE = FUNCTION NUMVAL ( WS-PQ-NUMBER-3 )
               COMPUTE QIT-AMOUNT = FUNCTION NUMVAL ( WS-PQ-NUMBER-4 )
           END-IF.

      *> INSERT-QUOTE-ITEM-ROW: writes QUOTE-ITEM-RECORD as a new quote line, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a quote line with that key exists.
       INSERT-QUOTE-ITEM-ROW.
           COBOL::"PARTIQL" ( "QUOTE-ITEM-FILE"
                   "INSERT INTO QUOTE-ITEM-FILE VALUE {'QIT-QUO-ID': ?, 'QIT-REV': ?, 'QIT-LINE': ?, "
                 & "'QIT-KIND': ?, 'QIT-CODE': ?, 'QIT-NAME': ?, 'QIT-QTY': ?, 'QIT-UNIT': ?, 'QIT-TAX-RATE': ?, "
                 & "'QIT-AMOUNT': ?, 'QIT-MANUAL': ?, 'QIT-NOTE': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS QIT-QUO-ID QIT-REV QIT-LINE QIT-KIND QIT-CODE QIT-NAME QIT-QTY QIT-UNIT
                   QIT-TAX-RATE QIT-AMOUNT QIT-MANUAL QIT-NOTE ).

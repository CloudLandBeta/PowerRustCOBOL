      *> pq-catalog: the PartiQL statements on CATALOG-FILE (catalog.FD) and the reading of its rows into
      *> CATALOG-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on CATALOG-FILE replaces the rows the one before it answered.

      *> GET-CATALOG-ROW: the catalog row whose key is in CAT-CURRENCY and CAT-CODE, read into CATALOG-RECORD.
      *> In:  CAT-CURRENCY and CAT-CODE.
      *> Out: WS-PQ-STATUS = 00 and CATALOG-RECORD filled, or 23 when there is no such catalog row (the record is left
      *>      as it was).
       GET-CATALOG-ROW.
           COBOL::"PARTIQL" ( "CATALOG-FILE" "SELECT * FROM CATALOG-FILE WHERE CAT-CURRENCY = ? AND CAT-CODE = ?"
                   WS-PQ-COUNT WS-PQ-STATUS CAT-CURRENCY CAT-CODE )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-CATALOG-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-CATALOG-OF-CURRENCY: the catalog rows of currency CAT-CURRENCY, in code order;
      *> FETCH-CATALOG-ROW reads them.
       SELECT-CATALOG-OF-CURRENCY.
           COBOL::"PARTIQL" ( "CATALOG-FILE" "SELECT * FROM CATALOG-FILE WHERE CAT-CURRENCY = ?"
                   WS-PQ-COUNT WS-PQ-STATUS CAT-CURRENCY ).

      *> FETCH-CATALOG-ROW: the next row the last statement on CATALOG-FILE answered, read into CATALOG-RECORD.
      *> Out: WS-PQ-STATUS = 00 and CATALOG-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point and
      *> the last value runs to the end of WS-PQ-ROW, so those numeric fields go through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-CATALOG-ROW.
           COBOL::"PARTIQL-FETCH" ( "CATALOG-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO CAT-CURRENCY CAT-CODE CAT-KIND CAT-NAME WS-PQ-NUMBER-1 WS-PQ-NUMBER-2 WS-PQ-NUMBER-3
               END-UNSTRING
               COMPUTE CAT-PRICE = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE CAT-HOURS = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
               COMPUTE CAT-TAX-RATE = FUNCTION NUMVAL ( WS-PQ-NUMBER-3 )
           END-IF.

      *> INSERT-CATALOG-ROW: writes CATALOG-RECORD as a new catalog row, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a catalog row with that key exists.
       INSERT-CATALOG-ROW.
           COBOL::"PARTIQL" ( "CATALOG-FILE"
                   "INSERT INTO CATALOG-FILE VALUE {'CAT-CURRENCY': ?, 'CAT-CODE': ?, 'CAT-KIND': ?, "
                 & "'CAT-NAME': ?, 'CAT-PRICE': ?, 'CAT-HOURS': ?, 'CAT-TAX-RATE': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS CAT-CURRENCY CAT-CODE CAT-KIND CAT-NAME CAT-PRICE CAT-HOURS CAT-TAX-RATE ).

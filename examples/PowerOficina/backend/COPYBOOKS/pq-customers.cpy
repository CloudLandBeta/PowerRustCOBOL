      *> pq-customers: the PartiQL statements on CUSTOMER-FILE (customers.FD) and the reading of its rows into
      *> CUSTOMER-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on CUSTOMER-FILE replaces the rows the one before it answered.

      *> GET-CUSTOMER-ROW: the customer whose key is in CUS-ID, read into CUSTOMER-RECORD.
      *> In:  CUS-ID.
      *> Out: WS-PQ-STATUS = 00 and CUSTOMER-RECORD filled, or 23 when there is no such customer (the record is left
      *>      as it was).
       GET-CUSTOMER-ROW.
           COBOL::"PARTIQL" ( "CUSTOMER-FILE" "SELECT * FROM CUSTOMER-FILE WHERE CUS-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS CUS-ID )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-CUSTOMER-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-CUSTOMERS-BY-DOCUMENT: the customers whose CPF or CNPJ is CUS-DOC, in the order they were
      *> registered; FETCH-CUSTOMER-ROW reads them.
       SELECT-CUSTOMERS-BY-DOCUMENT.
           COBOL::"PARTIQL" ( "CUSTOMER-FILE" "SELECT * FROM CUSTOMER-FILE.CUS-DOC WHERE CUS-DOC = ?"
                   WS-PQ-COUNT WS-PQ-STATUS CUS-DOC ).

      *> FETCH-CUSTOMER-ROW: the next row the last statement on CUSTOMER-FILE answered, read into CUSTOMER-RECORD.
      *> Out: WS-PQ-STATUS = 00 and CUSTOMER-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-CUSTOMER-ROW.
           COBOL::"PARTIQL-FETCH" ( "CUSTOMER-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO CUS-ID CUS-NAME CUS-DOC CUS-ADDRESS CUS-PHONE CUS-WHATSAPP CUS-EMAIL CUS-CONSENT-EMAIL
                        CUS-CONSENT-WA
               END-UNSTRING
           END-IF.

      *> INSERT-CUSTOMER-ROW: writes CUSTOMER-RECORD as a new customer, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a customer with that key exists.
       INSERT-CUSTOMER-ROW.
           COBOL::"PARTIQL" ( "CUSTOMER-FILE"
                   "INSERT INTO CUSTOMER-FILE VALUE {'CUS-ID': ?, 'CUS-NAME': ?, 'CUS-DOC': ?, 'CUS-ADDRESS': ?, "
                 & "'CUS-PHONE': ?, 'CUS-WHATSAPP': ?, 'CUS-EMAIL': ?, 'CUS-CONSENT-EMAIL': ?, 'CUS-CONSENT-WA': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS CUS-ID CUS-NAME CUS-DOC CUS-ADDRESS CUS-PHONE CUS-WHATSAPP CUS-EMAIL
                   CUS-CONSENT-EMAIL CUS-CONSENT-WA ).

      *> UPDATE-CUSTOMER-ROW: rewrites the customer whose key is in CUS-ID with every other field of CUSTOMER-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such customer.
       UPDATE-CUSTOMER-ROW.
           COBOL::"PARTIQL" ( "CUSTOMER-FILE"
                   "UPDATE CUSTOMER-FILE SET CUS-NAME = ? SET CUS-DOC = ? SET CUS-ADDRESS = ? SET CUS-PHONE = ? "
                 & "SET CUS-WHATSAPP = ? SET CUS-EMAIL = ? SET CUS-CONSENT-EMAIL = ? SET CUS-CONSENT-WA = ? "
                 & "WHERE CUS-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS CUS-NAME CUS-DOC CUS-ADDRESS CUS-PHONE CUS-WHATSAPP CUS-EMAIL
                   CUS-CONSENT-EMAIL CUS-CONSENT-WA CUS-ID ).

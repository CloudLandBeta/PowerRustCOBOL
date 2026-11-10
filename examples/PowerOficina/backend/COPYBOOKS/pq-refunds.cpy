      *> pq-refunds: the PartiQL statements on REFUND-FILE (refunds.FD) and the reading of its rows into
      *> REFUND-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on REFUND-FILE replaces the rows the one before it answered.

      *> INSERT-REFUND-ROW: writes REFUND-RECORD as a new refund, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a refund with that key exists.
       INSERT-REFUND-ROW.
           COBOL::"PARTIQL" ( "REFUND-FILE"
                   "INSERT INTO REFUND-FILE VALUE {'RFD-ID': ?, 'RFD-PAY-ID': ?, 'RFD-QUE-ID': ?, "
                 & "'RFD-CURRENCY': ?, 'RFD-AMOUNT': ?, 'RFD-BY': ?, 'RFD-AT': ?, 'RFD-STATUS': ?, 'RFD-REASON': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS RFD-ID RFD-PAY-ID RFD-QUE-ID RFD-CURRENCY RFD-AMOUNT RFD-BY RFD-AT
                   RFD-STATUS RFD-REASON ).

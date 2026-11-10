      *> pq-installments: the PartiQL statements on INSTALLMENT-FILE (installments.FD) and the reading of its rows
      *> into INSTALLMENT-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it
      *> uses these paragraphs. A statement on INSTALLMENT-FILE replaces the rows the one before it answered.

      *> GET-INSTALLMENT-ROW: the installment whose key is in INS-PAY-ID and INS-NO, read into INSTALLMENT-RECORD.
      *> In:  INS-PAY-ID and INS-NO.
      *> Out: WS-PQ-STATUS = 00 and INSTALLMENT-RECORD filled, or 23 when there is no such installment (the record is
      *>      left as it was).
       GET-INSTALLMENT-ROW.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE" "SELECT * FROM INSTALLMENT-FILE WHERE INS-PAY-ID = ? AND INS-NO = ?"
                   WS-PQ-COUNT WS-PQ-STATUS INS-PAY-ID INS-NO )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-INSTALLMENT-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-INSTALLMENTS-OF-PAYMENT: the installments of payment INS-PAY-ID, in number order;
      *> FETCH-INSTALLMENT-ROW reads them.
       SELECT-INSTALLMENTS-OF-PAYMENT.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE" "SELECT * FROM INSTALLMENT-FILE WHERE INS-PAY-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS INS-PAY-ID ).

      *> SELECT-INSTALLMENTS-DUE: the installments due on or before INS-DUE, by due date from the oldest (those
      *> of one date in the order they were written); FETCH-INSTALLMENT-ROW reads them.
       SELECT-INSTALLMENTS-DUE.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE" "SELECT * FROM INSTALLMENT-FILE.INS-DUE WHERE INS-DUE <= ?"
                   WS-PQ-COUNT WS-PQ-STATUS INS-DUE ).

      *> SELECT-ALL-INSTALLMENTS: every installment, by payment and number; FETCH-INSTALLMENT-ROW reads them.
       SELECT-ALL-INSTALLMENTS.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE" "SELECT * FROM INSTALLMENT-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-INSTALLMENT-ROW: the next row the last statement on INSTALLMENT-FILE answered, read into
      *> INSTALLMENT-RECORD.
      *> Out: WS-PQ-STATUS = 00 and INSTALLMENT-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point and
      *> the last value runs to the end of WS-PQ-ROW, so those numeric fields go through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-INSTALLMENT-ROW.
           COBOL::"PARTIQL-FETCH" ( "INSTALLMENT-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO INS-PAY-ID INS-NO INS-DUE WS-PQ-NUMBER-1 INS-STATUS INS-GW-REF INS-PAID-AT WS-PQ-NUMBER-2
               END-UNSTRING
               COMPUTE INS-AMOUNT = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE INS-TRIES = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
           END-IF.

      *> INSERT-INSTALLMENT-ROW: writes INSTALLMENT-RECORD as a new installment, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when an installment with that key exists.
       INSERT-INSTALLMENT-ROW.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE"
                   "INSERT INTO INSTALLMENT-FILE VALUE {'INS-PAY-ID': ?, 'INS-NO': ?, 'INS-DUE': ?, "
                 & "'INS-AMOUNT': ?, 'INS-STATUS': ?, 'INS-GW-REF': ?, 'INS-PAID-AT': ?, 'INS-TRIES': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS INS-PAY-ID INS-NO INS-DUE INS-AMOUNT INS-STATUS INS-GW-REF INS-PAID-AT
                   INS-TRIES ).

      *> UPDATE-INSTALLMENT-ROW: rewrites the installment whose key is in INS-PAY-ID and INS-NO with every other field
      *> of INSTALLMENT-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such installment.
       UPDATE-INSTALLMENT-ROW.
           COBOL::"PARTIQL" ( "INSTALLMENT-FILE"
                   "UPDATE INSTALLMENT-FILE SET INS-DUE = ? SET INS-AMOUNT = ? SET INS-STATUS = ? "
                 & "SET INS-GW-REF = ? SET INS-PAID-AT = ? SET INS-TRIES = ? WHERE INS-PAY-ID = ? AND INS-NO = ?"
                   WS-PQ-COUNT WS-PQ-STATUS INS-DUE INS-AMOUNT INS-STATUS INS-GW-REF INS-PAID-AT INS-TRIES INS-PAY-ID
                   INS-NO ).

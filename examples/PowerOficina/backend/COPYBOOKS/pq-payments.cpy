      *> pq-payments: the PartiQL statements on PAYMENT-FILE (payments.FD) and the reading of its rows into
      *> PAYMENT-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on PAYMENT-FILE replaces the rows the one before it answered.

      *> GET-PAYMENT-ROW: the payment whose key is in PAY-ID, read into PAYMENT-RECORD.
      *> In:  PAY-ID.
      *> Out: WS-PQ-STATUS = 00 and PAYMENT-RECORD filled, or 23 when there is no such payment (the record is left as
      *>      it was).
       GET-PAYMENT-ROW.
           COBOL::"PARTIQL" ( "PAYMENT-FILE" "SELECT * FROM PAYMENT-FILE WHERE PAY-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS PAY-ID )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-PAYMENT-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-PAYMENTS-OF-QUOTE: the payments of quote PAY-QUO-ID, in the order they were made;
      *> FETCH-PAYMENT-ROW reads them.
       SELECT-PAYMENTS-OF-QUOTE.
           COBOL::"PARTIQL" ( "PAYMENT-FILE" "SELECT * FROM PAYMENT-FILE.PAY-QUO-ID WHERE PAY-QUO-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS PAY-QUO-ID ).

      *> SELECT-ALL-PAYMENTS: every payment, in the order of PAY-ID; FETCH-PAYMENT-ROW reads them.
       SELECT-ALL-PAYMENTS.
           COBOL::"PARTIQL" ( "PAYMENT-FILE" "SELECT * FROM PAYMENT-FILE"
                   WS-PQ-COUNT WS-PQ-STATUS ).

      *> FETCH-PAYMENT-ROW: the next row the last statement on PAYMENT-FILE answered, read into PAYMENT-RECORD.
      *> Out: WS-PQ-STATUS = 00 and PAYMENT-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point and
      *> the last value runs to the end of WS-PQ-ROW, so those numeric fields go through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-PAYMENT-ROW.
           COBOL::"PARTIQL-FETCH" ( "PAYMENT-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO PAY-ID PAY-QUO-ID PAY-CURRENCY WS-PQ-NUMBER-1 PAY-INSTALLMENTS PAY-CARD-BRAND PAY-CARD-LAST4
                        PAY-GW-REF PAY-STATUS PAY-CONSENT-AT WS-PQ-NUMBER-2
               END-UNSTRING
               COMPUTE PAY-TOTAL = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE PAY-AT = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
           END-IF.

      *> INSERT-PAYMENT-ROW: writes PAYMENT-RECORD as a new payment, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a payment with that key exists.
       INSERT-PAYMENT-ROW.
           COBOL::"PARTIQL" ( "PAYMENT-FILE"
                   "INSERT INTO PAYMENT-FILE VALUE {'PAY-ID': ?, 'PAY-QUO-ID': ?, 'PAY-CURRENCY': ?, "
                 & "'PAY-TOTAL': ?, 'PAY-INSTALLMENTS': ?, 'PAY-CARD-BRAND': ?, 'PAY-CARD-LAST4': ?, "
                 & "'PAY-GW-REF': ?, 'PAY-STATUS': ?, 'PAY-CONSENT-AT': ?, 'PAY-AT': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS PAY-ID PAY-QUO-ID PAY-CURRENCY PAY-TOTAL PAY-INSTALLMENTS PAY-CARD-BRAND
                   PAY-CARD-LAST4 PAY-GW-REF PAY-STATUS PAY-CONSENT-AT PAY-AT ).

      *> UPDATE-PAYMENT-ROW: rewrites the payment whose key is in PAY-ID with every other field of PAYMENT-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such payment.
       UPDATE-PAYMENT-ROW.
           COBOL::"PARTIQL" ( "PAYMENT-FILE"
                   "UPDATE PAYMENT-FILE SET PAY-QUO-ID = ? SET PAY-CURRENCY = ? SET PAY-TOTAL = ? "
                 & "SET PAY-INSTALLMENTS = ? SET PAY-CARD-BRAND = ? SET PAY-CARD-LAST4 = ? SET PAY-GW-REF = ? "
                 & "SET PAY-STATUS = ? SET PAY-CONSENT-AT = ? SET PAY-AT = ? WHERE PAY-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS PAY-QUO-ID PAY-CURRENCY PAY-TOTAL PAY-INSTALLMENTS PAY-CARD-BRAND
                   PAY-CARD-LAST4 PAY-GW-REF PAY-STATUS PAY-CONSENT-AT PAY-AT PAY-ID ).

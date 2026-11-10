      *> A program that COPYs pd-common also COPYs ws-partiql, pq-sequences and pq-audit.
      *> NEXT-NUMBER: the next number of one sequence.
      *> In:  WS-SEQ-WANT = the sequence name (SEQ-NAME of sequences.cidx).
      *> Out: WS-SEQ-GOT = the number issued, or 0 when the sequence file could not be used.
       NEXT-NUMBER.
           MOVE    0                             TO    WS-SEQ-GOT
           OPEN    I-O SEQUENCE-FILE
           IF    WS-FS-SEQ = "35"
               OPEN    OUTPUT SEQUENCE-FILE
               CLOSE   SEQUENCE-FILE
               OPEN    I-O SEQUENCE-FILE
           END-IF
           IF    WS-FS-SEQ = "00"
               MOVE    WS-SEQ-WANT               TO    SEQ-NAME
               PERFORM GET-SEQUENCE-ROW
               IF    WS-PQ-STATUS = "00"
                   ADD     1                     TO    SEQ-VALUE
                   PERFORM UPDATE-SEQUENCE-ROW
               ELSE
                   INITIALIZE    SEQUENCE-RECORD
                   MOVE    WS-SEQ-WANT           TO    SEQ-NAME
                   MOVE    1                     TO    SEQ-VALUE
                   PERFORM INSERT-SEQUENCE-ROW
               END-IF
               MOVE    SEQ-VALUE                 TO    WS-SEQ-GOT
               CLOSE   SEQUENCE-FILE
           END-IF.

      *> NOW-STAMP: the date and time as YYYYMMDDHHMMSS in WS-NOW.
       NOW-STAMP.
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-NOW-DATE
           MOVE    WS-CLOCK-TIME                 TO    WS-NOW-TIME.

      *> WRITE-AUDIT: one line of the audit trail (R36).
      *> In:  WS-AU-OPERATOR WS-AU-ENTITY WS-AU-REF WS-AU-ACTION WS-AU-DETAIL.
      *> Out: one AUDIT-FILE record, numbered by the AUDIT sequence and stamped with the time.
       WRITE-AUDIT.
           PERFORM NOW-STAMP
           MOVE    "AUDIT"                       TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           OPEN    I-O AUDIT-FILE
           IF    WS-FS-AUD = "35"
               OPEN    OUTPUT AUDIT-FILE
               CLOSE   AUDIT-FILE
               OPEN    I-O AUDIT-FILE
           END-IF
           IF    WS-FS-AUD = "00" AND WS-SEQ-GOT > 0
               INITIALIZE    AUDIT-RECORD
               MOVE    WS-SEQ-GOT                TO    AUD-ID
               MOVE    WS-NOW                    TO    AUD-AT
               MOVE    WS-AU-OPERATOR            TO    AUD-OPERATOR
               MOVE    WS-AU-ENTITY              TO    AUD-ENTITY
               MOVE    WS-AU-REF                 TO    AUD-REF
               MOVE    WS-AU-ACTION              TO    AUD-ACTION
               MOVE    WS-AU-DETAIL              TO    AUD-DETAIL
               PERFORM INSERT-AUDIT-ROW
               CLOSE   AUDIT-FILE
           END-IF.

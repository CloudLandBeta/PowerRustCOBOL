      *> pq-diagnoses: the PartiQL statements on DIAGNOSIS-FILE (diagnoses.FD) and the reading of its rows into
      *> DIAGNOSIS-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses
      *> these paragraphs. A statement on DIAGNOSIS-FILE replaces the rows the one before it answered.

      *> GET-DIAGNOSIS-ROW: the diagnosis whose key is in DGN-ID, read into DIAGNOSIS-RECORD.
      *> In:  DGN-ID.
      *> Out: WS-PQ-STATUS = 00 and DIAGNOSIS-RECORD filled, or 23 when there is no such diagnosis (the record is left
      *>      as it was).
       GET-DIAGNOSIS-ROW.
           COBOL::"PARTIQL" ( "DIAGNOSIS-FILE" "SELECT * FROM DIAGNOSIS-FILE WHERE DGN-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS DGN-ID )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-DIAGNOSIS-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> FETCH-DIAGNOSIS-ROW: the next row the last statement on DIAGNOSIS-FILE answered, read into DIAGNOSIS-RECORD.
      *> Out: WS-PQ-STATUS = 00 and DIAGNOSIS-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". A decimal comes back with a point, so it
      *> goes through WS-PQ-NUMBER-n and NUMVAL.
       FETCH-DIAGNOSIS-ROW.
           COBOL::"PARTIQL-FETCH" ( "DIAGNOSIS-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO DGN-ID DGN-PLATE DGN-AT DGN-SOURCE DGN-MODEL DGN-ENGINE-TEMP DGN-RPM WS-PQ-NUMBER-1
                        WS-PQ-NUMBER-2 DGN-FAULT-1 DGN-FAULT-2 DGN-FAULT-3 DGN-FAULT-4 DGN-FAULT-5
               END-UNSTRING
               COMPUTE DGN-BATTERY = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
               COMPUTE DGN-OIL-PRESSURE = FUNCTION NUMVAL ( WS-PQ-NUMBER-2 )
           END-IF.

      *> INSERT-DIAGNOSIS-ROW: writes DIAGNOSIS-RECORD as a new diagnosis, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a diagnosis with that key exists.
       INSERT-DIAGNOSIS-ROW.
           COBOL::"PARTIQL" ( "DIAGNOSIS-FILE"
                   "INSERT INTO DIAGNOSIS-FILE VALUE {'DGN-ID': ?, 'DGN-PLATE': ?, 'DGN-AT': ?, 'DGN-SOURCE': ?, "
                 & "'DGN-MODEL': ?, 'DGN-ENGINE-TEMP': ?, 'DGN-RPM': ?, 'DGN-BATTERY': ?, 'DGN-OIL-PRESSURE': ?, "
                 & "'DGN-FAULT-1': ?, 'DGN-FAULT-2': ?, 'DGN-FAULT-3': ?, 'DGN-FAULT-4': ?, 'DGN-FAULT-5': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS DGN-ID DGN-PLATE DGN-AT DGN-SOURCE DGN-MODEL DGN-ENGINE-TEMP DGN-RPM
                   DGN-BATTERY DGN-OIL-PRESSURE DGN-FAULT-1 DGN-FAULT-2 DGN-FAULT-3 DGN-FAULT-4 DGN-FAULT-5 ).

      *> UPDATE-DIAGNOSIS-ROW: rewrites the diagnosis whose key is in DGN-ID with every other field of
      *> DIAGNOSIS-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such diagnosis.
       UPDATE-DIAGNOSIS-ROW.
           COBOL::"PARTIQL" ( "DIAGNOSIS-FILE"
                   "UPDATE DIAGNOSIS-FILE SET DGN-PLATE = ? SET DGN-AT = ? SET DGN-SOURCE = ? SET DGN-MODEL = ? "
                 & "SET DGN-ENGINE-TEMP = ? SET DGN-RPM = ? SET DGN-BATTERY = ? SET DGN-OIL-PRESSURE = ? "
                 & "SET DGN-FAULT-1 = ? SET DGN-FAULT-2 = ? SET DGN-FAULT-3 = ? SET DGN-FAULT-4 = ? "
                 & "SET DGN-FAULT-5 = ? WHERE DGN-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS DGN-PLATE DGN-AT DGN-SOURCE DGN-MODEL DGN-ENGINE-TEMP DGN-RPM DGN-BATTERY
                   DGN-OIL-PRESSURE DGN-FAULT-1 DGN-FAULT-2 DGN-FAULT-3 DGN-FAULT-4 DGN-FAULT-5 DGN-ID ).

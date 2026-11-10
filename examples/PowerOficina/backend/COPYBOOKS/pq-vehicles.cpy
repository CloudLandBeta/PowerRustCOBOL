      *> pq-vehicles: the PartiQL statements on VEHICLE-FILE (vehicles.FD) and the reading of its rows into
      *> VEHICLE-RECORD. The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these
      *> paragraphs. A statement on VEHICLE-FILE replaces the rows the one before it answered.

      *> GET-VEHICLE-ROW: the vehicle whose key is in VEH-PLATE, read into VEHICLE-RECORD.
      *> In:  VEH-PLATE.
      *> Out: WS-PQ-STATUS = 00 and VEHICLE-RECORD filled, or 23 when there is no such vehicle (the record is left as
      *>      it was).
       GET-VEHICLE-ROW.
           COBOL::"PARTIQL" ( "VEHICLE-FILE" "SELECT * FROM VEHICLE-FILE WHERE VEH-PLATE = ?"
                   WS-PQ-COUNT WS-PQ-STATUS VEH-PLATE )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-VEHICLE-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> FETCH-VEHICLE-ROW: the next row the last statement on VEHICLE-FILE answered, read into VEHICLE-RECORD.
      *> Out: WS-PQ-STATUS = 00 and VEHICLE-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09". The last value runs to the end of
      *> WS-PQ-ROW, so it goes through WS-PQ-NUMBER-1 and NUMVAL.
       FETCH-VEHICLE-ROW.
           COBOL::"PARTIQL-FETCH" ( "VEHICLE-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO VEH-PLATE VEH-CUS-ID VEH-CHASSIS VEH-BRAND VEH-MODEL VEH-YEAR VEH-COLOR WS-PQ-NUMBER-1
               END-UNSTRING
               COMPUTE VEH-MILEAGE = FUNCTION NUMVAL ( WS-PQ-NUMBER-1 )
           END-IF.

      *> INSERT-VEHICLE-ROW: writes VEHICLE-RECORD as a new vehicle, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a vehicle with that key exists.
       INSERT-VEHICLE-ROW.
           COBOL::"PARTIQL" ( "VEHICLE-FILE"
                   "INSERT INTO VEHICLE-FILE VALUE {'VEH-PLATE': ?, 'VEH-CUS-ID': ?, 'VEH-CHASSIS': ?, "
                 & "'VEH-BRAND': ?, 'VEH-MODEL': ?, 'VEH-YEAR': ?, 'VEH-COLOR': ?, 'VEH-MILEAGE': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS VEH-PLATE VEH-CUS-ID VEH-CHASSIS VEH-BRAND VEH-MODEL VEH-YEAR VEH-COLOR
                   VEH-MILEAGE ).

      *> UPDATE-VEHICLE-ROW: rewrites the vehicle whose key is in VEH-PLATE with every other field of VEHICLE-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such vehicle.
       UPDATE-VEHICLE-ROW.
           COBOL::"PARTIQL" ( "VEHICLE-FILE"
                   "UPDATE VEHICLE-FILE SET VEH-CUS-ID = ? SET VEH-CHASSIS = ? SET VEH-BRAND = ? SET VEH-MODEL = ? "
                 & "SET VEH-YEAR = ? SET VEH-COLOR = ? SET VEH-MILEAGE = ? WHERE VEH-PLATE = ?"
                   WS-PQ-COUNT WS-PQ-STATUS VEH-CUS-ID VEH-CHASSIS VEH-BRAND VEH-MODEL VEH-YEAR VEH-COLOR VEH-MILEAGE
                   VEH-PLATE ).

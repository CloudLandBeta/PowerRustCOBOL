       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIAG-TEST.
      *> Test of the Backend program DIAG-COLLECT (CUSTOMER-SAVE and VEHICLE-SAVE give it a vehicle).
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding a copy of the project's faultrules.idx.
      *> Cases: unknown vehicle, bad operator, bad plate, bad mode, a complete collection (same plate reads the same
      *> twice, a different plate reads differently), a collection interrupted after 1, 2 and 3 sensors (partial, what
      *> did not arrive stays empty), the partial completed by hand (becomes manual, same id), a hand-typed entry from
      *> scratch, readings out of range, a partial of another vehicle, a partial that does not exist.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-diag".
           COPY "backend/COPYBOOKS/res-diag".
       01 WS-CASE-NAME                           PIC    X(52).
       01 WS-WANT-STATUS                         PIC    X(10).
       01 WS-WANT-FIELD                          PIC    X(10).
       01 WS-OWNER-ID                            PIC    9(8).
       01 WS-FIRST-ID                            PIC    9(8).
       01 WS-PARTIAL-ID                          PIC    9(8).
       01 WS-SAVE-TEMP                           PIC    9(3).
       01 WS-SAVE-RPM                            PIC    9(5).
       01 WS-SAVE-FAULT                          PIC    X(8).
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME

           INITIALIZE CUS-REQ
           MOVE    "João da Silva"               TO    CRQ-NAME
           MOVE    "52998224725"                 TO    CRQ-DOC
           MOVE    "N"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-OWNER-ID

           PERFORM MAKE-VEHICLE-ABC
           PERFORM MAKE-VEHICLE-XYZ

           MOVE    "vehicle not on file"         TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "QQQ9Q99"                     TO    DCQ-PLATE
           MOVE    "NOVEHICLE"                   TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT

           MOVE    "operator missing"            TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    SPACES                        TO    DCQ-OPERATOR
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "OPERATOR"                    TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "plate too short"             TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "AB1"                         TO    DCQ-PLATE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PLATE"                       TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "mode that is neither S nor M"  TO  WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "X"                           TO    DCQ-MODE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "MODE"                        TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "complete collection, plate typed with hyphen"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "abc-1d23"                    TO    DCQ-PLATE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           MOVE    DCS-ID                        TO    WS-FIRST-ID
           MOVE    DCS-ENGINE-TEMP               TO    WS-SAVE-TEMP
           MOVE    DCS-RPM                       TO    WS-SAVE-RPM
           MOVE    DCS-FAULT-1                   TO    WS-SAVE-FAULT
           PERFORM CHECK-COMPLETE

           MOVE    "same plate reads the same, with a new id"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "OK"                          TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           PERFORM CHECK-COMPLETE
           ADD     1                             TO    WS-CASES
           IF    DCS-ID NOT = WS-FIRST-ID AND DCS-ENGINE-TEMP = WS-SAVE-TEMP
                 AND DCS-RPM = WS-SAVE-RPM AND DCS-FAULT-1 = WS-SAVE-FAULT
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " got " DCS-ID " " DCS-ENGINE-TEMP " " DCS-RPM " " DCS-FAULT-1
           END-IF

           MOVE    "another plate, another reading"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "XYZ4E56"                     TO    DCQ-PLATE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           PERFORM CHECK-COMPLETE
           ADD     1                             TO    WS-CASES
           IF    DCS-ENGINE-TEMP NOT = WS-SAVE-TEMP OR DCS-RPM NOT = WS-SAVE-RPM
                 OR DCS-FAULT-1 NOT = WS-SAVE-FAULT
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " both read the same"
           END-IF

           MOVE    "interrupted after sensor 1"  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    1                             TO    DCQ-INTERRUPT-AFTER
           MOVE    "PARTIAL"                     TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           ADD     1                             TO    WS-CASES
           IF    DCS-COLLECTED = 1 AND DCS-RPM = 0 AND DCS-BATTERY = 0 AND DCS-OIL-PRESSURE = 0
                 AND DCS-FAULT-1 = SPACES AND DCS-ENGINE-TEMP > 0
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " collected " DCS-COLLECTED " rpm " DCS-RPM
           END-IF
           MOVE    DCS-ID                        TO    WS-PARTIAL-ID

           MOVE    "interrupted after sensor 2"  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    2                             TO    DCQ-INTERRUPT-AFTER
           MOVE    "PARTIAL"                     TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           ADD     1                             TO    WS-CASES
           IF    DCS-COLLECTED = 2 AND DCS-RPM > 0 AND DCS-BATTERY = 0 AND DCS-OIL-PRESSURE = 0
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " collected " DCS-COLLECTED
           END-IF

           MOVE    "interrupted after sensor 3"  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    3                             TO    DCQ-INTERRUPT-AFTER
           MOVE    "PARTIAL"                     TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           ADD     1                             TO    WS-CASES
           IF    DCS-COLLECTED = 3 AND DCS-BATTERY > 0 AND DCS-OIL-PRESSURE = 0
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " collected " DCS-COLLECTED
           END-IF

           MOVE    "partial completed by hand keeps its id, goes manual"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    WS-PARTIAL-ID                 TO    DCQ-ID
           MOVE    92                            TO    DCQ-ENGINE-TEMP
           MOVE    820                           TO    DCQ-RPM
           MOVE    13.1                          TO    DCQ-BATTERY
           MOVE    290.5                         TO    DCQ-OIL-PRESSURE
           MOVE    "P0300"                       TO    DCQ-FAULT-1
           MOVE    "OK"                          TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           ADD     1                             TO    WS-CASES
           IF    DCS-ID = WS-PARTIAL-ID AND DCS-SOURCE = "M" AND DCS-COLLECTED = 4
                 AND DCS-RPM = 820 AND DCS-FAULT-1 = "P0300"
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " got " DCS-ID " " DCS-SOURCE " " DCS-RPM
           END-IF

           MOVE    "hand-typed entry from scratch"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    88                            TO    DCQ-ENGINE-TEMP
           MOVE    900                           TO    DCQ-RPM
           MOVE    12.6                          TO    DCQ-BATTERY
           MOVE    300.0                         TO    DCQ-OIL-PRESSURE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT
           ADD     1                             TO    WS-CASES
           IF    DCS-SOURCE = "M" AND DCS-ID > 0 AND DCS-BATTERY = 12.6
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " got " DCS-SOURCE " " DCS-BATTERY
           END-IF

           MOVE    "engine temperature above 200"  TO  WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    250                           TO    DCQ-ENGINE-TEMP
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "TEMP"                        TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "rpm above 12000"             TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    13000                         TO    DCQ-RPM
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "RPM"                         TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "battery above 30 volts"      TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    31.5                          TO    DCQ-BATTERY
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "BATTERY"                     TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "partial of another vehicle"  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "XYZ4E56"                     TO    DCQ-PLATE
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    WS-PARTIAL-ID                 TO    DCQ-ID
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PLATE"                       TO    WS-WANT-FIELD
           PERFORM RUN-COLLECT

           MOVE    "partial that does not exist"  TO   WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    99999                         TO    DCQ-ID
           MOVE    "NOTFOUND"                    TO    WS-WANT-STATUS
           PERFORM RUN-COLLECT

           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0

           DISPLAY "DIAG-COLLECT test"
           DISPLAY "  cases run      " WS-CASES
           DISPLAY "  passed         " WS-PASS
           DISPLAY "  failed         " WS-FAIL
           DISPLAY "  elapsed        " WS-ELAPSED " hundredths of a second"
           IF    WS-FAIL = 0
               DISPLAY "  result         PASS"
           ELSE
               DISPLAY "  result         FAIL"
           END-IF
           STOP RUN.

       MAKE-VEHICLE-ABC.
           INITIALIZE VEH-REQ
           MOVE    "ABC1D23"                     TO    VRQ-PLATE
           MOVE    WS-OWNER-ID                   TO    VRQ-CUS-ID
           MOVE    "Fiat"                        TO    VRQ-BRAND
           MOVE    "Palio 1.0"                   TO    VRQ-MODEL
           MOVE    2014                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES.

       MAKE-VEHICLE-XYZ.
           INITIALIZE VEH-REQ
           MOVE    "XYZ4E56"                     TO    VRQ-PLATE
           MOVE    WS-OWNER-ID                   TO    VRQ-CUS-ID
           MOVE    "VW"                          TO    VRQ-BRAND
           MOVE    "Gol 1.6"                     TO    VRQ-MODEL
           MOVE    2018                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES.

       BASE-REQUEST.
           INITIALIZE DGC-REQ
           MOVE    "ABC1D23"                     TO    DCQ-PLATE
           MOVE    "Maria Atendente"             TO    DCQ-OPERATOR
           MOVE    "S"                           TO    DCQ-MODE
           MOVE    SPACES                        TO    WS-WANT-FIELD.

       RUN-COLLECT.
           ADD     1                             TO    WS-CASES
           CALL    "DIAG-COLLECT" USING DGC-REQ RETURNING DGC-RES
           IF    DCS-STATUS = WS-WANT-STATUS AND DCS-FIELD = WS-WANT-FIELD
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
               DISPLAY "    wanted " WS-WANT-STATUS " " WS-WANT-FIELD
               DISPLAY "    got    " DCS-STATUS " " DCS-FIELD " " DCS-MESSAGE
           END-IF.

      *> CHECK-COMPLETE: a complete collection has the model, four sensors, in range, and is marked collected.
       CHECK-COMPLETE.
           ADD     1                             TO    WS-CASES
           IF    DCS-SOURCE = "S" AND DCS-COLLECTED = 4 AND DCS-ID > 0 AND DCS-MODEL NOT = SPACES
                 AND DCS-ENGINE-TEMP > 0 AND DCS-RPM > 0 AND DCS-BATTERY > 0 AND DCS-OIL-PRESSURE > 0
                 AND DCS-AT > 0
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " (complete) src " DCS-SOURCE " n " DCS-COLLECTED
           END-IF.

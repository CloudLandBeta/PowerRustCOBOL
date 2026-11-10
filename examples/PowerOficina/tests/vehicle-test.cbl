       IDENTIFICATION DIVISION.
       PROGRAM-ID. VEHICLE-TEST.
      *> Test of the Backend programs VEHICLE-SAVE and VEHICLE-FIND (CUSTOMER-SAVE gives them an owner).
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder, so the project's own data is never touched.
      *> Cases: plate normalisation (case, hyphen, spaces), a new plate and a repeated one (change, no duplicate),
      *> a plate that is too short, too long or has a symbol, an owner that does not exist, a short chassis, a year out
      *> of range, a missing model or operator, and the lookup by plate (found with owner, not found, invalid).

       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-vehicle-find".
           COPY "backend/COPYBOOKS/res-vehicle-find".
       01 WS-CASE-NAME                           PIC    X(44).
       01 WS-WANT-STATUS                         PIC    X(10).
       01 WS-WANT-FIELD                          PIC    X(10).
       01 WS-WANT-ACTION                         PIC    X(8).
       01 WS-WANT-PLATE                          PIC    X(8).
       01 WS-OWNER-ID                            PIC    9(8).
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
           MOVE    "(11) 3000-1234"              TO    CRQ-PHONE
           MOVE    "joao@exemplo.com.br"         TO    CRQ-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-OWNER-ID

           MOVE    "new plate typed in lower case with hyphen"  TO WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "abc-1d23"                    TO    VRQ-PLATE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           MOVE    "CREATED"                     TO    WS-WANT-ACTION
           MOVE    "ABC1D23"                     TO    WS-WANT-PLATE
           PERFORM RUN-SAVE

           MOVE    "same plate again is a change, no duplicate"  TO WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "ABC1D23"                     TO    VRQ-PLATE
           MOVE    55000                         TO    VRQ-MILEAGE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           MOVE    "UPDATED"                     TO    WS-WANT-ACTION
           MOVE    "ABC1D23"                     TO    WS-WANT-PLATE
           PERFORM RUN-SAVE

           MOVE    "plate with 6 characters"     TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "ABC1D2"                      TO    VRQ-PLATE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PLATE"                       TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "plate with 8 characters"     TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "ABC1D234"                    TO    VRQ-PLATE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PLATE"                       TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "plate with a symbol"         TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "AB*1234"                     TO    VRQ-PLATE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PLATE"                       TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "owner that does not exist"   TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    999                           TO    VRQ-CUS-ID
           MOVE    "NOCUSTOMER"                  TO    WS-WANT-STATUS
           PERFORM RUN-SAVE

           MOVE    "chassis with 16 characters"  TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    "9BWZZZ377VT00445"            TO    VRQ-CHASSIS
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "CHASSIS"                     TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "year 1850"                   TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    1850                          TO    VRQ-YEAR
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "YEAR"                        TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "empty model"                 TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    SPACES                        TO    VRQ-MODEL
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "MODEL"                       TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "empty operator"              TO    WS-CASE-NAME
           PERFORM BASE-VEHICLE
           MOVE    SPACES                        TO    VRQ-OPERATOR
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "OPERATOR"                    TO    WS-WANT-FIELD
           PERFORM RUN-SAVE

           MOVE    "find: plate on file, with its owner"  TO WS-CASE-NAME
           MOVE    "abc 1d23"                    TO    VFQ-PLATE
           PERFORM RUN-FIND
           IF    VFS-STATUS = "FOUND" AND VFS-PLATE = "ABC1D23"
                 AND VFS-MODEL (1:6) = "Palio " AND VFS-MILEAGE = 55000
                 AND VFS-CUS-ID = WS-OWNER-ID AND VFS-CUS-DOC = "52998224725"
                 AND VFS-CUS-NAME (1:5) = "João"
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
               DISPLAY "    got " VFS-STATUS " " VFS-PLATE " " VFS-MODEL " " VFS-MILEAGE " " VFS-CUS-ID " " VFS-CUS-NAME
           END-IF

           MOVE    "find: plate not on file"     TO    WS-CASE-NAME
           MOVE    "ZZZ9Z99"                     TO    VFQ-PLATE
           PERFORM RUN-FIND
           IF    VFS-STATUS = "NOTFOUND" AND VFS-PLATE = "ZZZ9Z99"
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " got " VFS-STATUS
           END-IF

           MOVE    "find: plate that is not 7 characters"  TO WS-CASE-NAME
           MOVE    "AB1"                         TO    VFQ-PLATE
           PERFORM RUN-FIND
           IF    VFS-STATUS = "INVALID"
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME " got " VFS-STATUS
           END-IF

           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0

           DISPLAY "VEHICLE-SAVE and VEHICLE-FIND test"
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

       BASE-VEHICLE.
           INITIALIZE VEH-REQ
           MOVE    WS-OWNER-ID                   TO    VRQ-CUS-ID
           MOVE    "9BWZZZ377VT004251"           TO    VRQ-CHASSIS
           MOVE    "Fiat"                        TO    VRQ-BRAND
           MOVE    "Palio 1.0"                   TO    VRQ-MODEL
           MOVE    2014                          TO    VRQ-YEAR
           MOVE    "Prata"                       TO    VRQ-COLOR
           MOVE    48000                         TO    VRQ-MILEAGE
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           MOVE    SPACES                        TO    WS-WANT-FIELD
           MOVE    SPACES                        TO    WS-WANT-ACTION
           MOVE    SPACES                        TO    WS-WANT-PLATE.

       RUN-SAVE.
           ADD     1                             TO    WS-CASES
           IF    VRQ-PLATE = SPACES
               MOVE    "KJH2G34"                 TO    VRQ-PLATE
           END-IF
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES
           IF    VRS-STATUS = WS-WANT-STATUS
                 AND VRS-FIELD = WS-WANT-FIELD
                 AND VRS-ACTION = WS-WANT-ACTION
                 AND VRS-PLATE = WS-WANT-PLATE
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
               DISPLAY "    wanted " WS-WANT-STATUS " " WS-WANT-FIELD " " WS-WANT-ACTION " " WS-WANT-PLATE
               DISPLAY "    got    " VRS-STATUS " " VRS-FIELD " " VRS-ACTION " " VRS-PLATE " " VRS-MESSAGE
           END-IF.

       RUN-FIND.
           ADD     1                             TO    WS-CASES
           CALL    "VEHICLE-FIND" USING VFD-REQ RETURNING VFD-RES.

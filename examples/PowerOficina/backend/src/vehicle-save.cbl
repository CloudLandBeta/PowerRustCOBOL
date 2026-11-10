       IDENTIFICATION DIVISION.
       PROGRAM-ID. VEHICLE-SAVE.
      *> Saves a vehicle, linked to its owner: a new plate is created, a plate already on file is changed (R2, R3).
      *> The plate is stored in capitals without hyphen or space; it must have 7 letters or digits. The owner must
      *> exist. The chassis, when given, has 17 characters; the year is between 1900 and 2100.
      *> Every save writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-vehicle.cpy and res-vehicle.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT VEHICLE-FILE ASSIGN TO "data/vehicles.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS VEH-PLATE
               ALTERNATE RECORD KEY IS VEH-CUS-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-VEH.
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUS-ID
               ALTERNATE RECORD KEY IS CUS-DOC WITH DUPLICATES
               FILE STATUS IS WS-FS-CUS.
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/vehicles.FD".
           COPY "COPYBOOKS/customers.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-plate".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-VEH                              PIC    XX.
       01 WS-FS-CUS                              PIC    XX.
       01 WS-CHASSIS-LEN                         PIC    99.
       01 WS-POS                                 PIC    99.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".

       PROCEDURE DIVISION USING VEH-REQ RETURNING VEH-RES.
       MAIN-PARA.
           INITIALIZE VEH-RES
           MOVE    "OK"                          TO    VRS-STATUS

           MOVE    VRQ-PLATE                     TO    WS-PLATE-IN
           PERFORM NORMALIZE-PLATE
           PERFORM VALIDATE-REQUEST

           IF    VRS-STATUS = "OK"
               PERFORM STORE-VEHICLE
           END-IF

           GOBACK.

       VALIDATE-REQUEST.
           MOVE    0                             TO    WS-CHASSIS-LEN
           PERFORM VARYING WS-POS FROM 17 BY -1 UNTIL WS-POS < 1 OR WS-CHASSIS-LEN > 0
               IF    VRQ-CHASSIS (WS-POS:1) NOT = SPACE
                   MOVE    WS-POS                TO    WS-CHASSIS-LEN
               END-IF
           END-PERFORM

           EVALUATE TRUE
               WHEN VRQ-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    VRS-STATUS
                   MOVE    "OPERATOR"            TO    VRS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    VRS-MESSAGE
               WHEN WS-PLATE-OK = 0
                   MOVE    "INVALID"             TO    VRS-STATUS
                   MOVE    "PLATE"               TO    VRS-FIELD
                   MOVE    "A placa deve ter 7 letras ou números."
                                                 TO    VRS-MESSAGE
               WHEN VRQ-MODEL = SPACES
                   MOVE    "INVALID"             TO    VRS-STATUS
                   MOVE    "MODEL"               TO    VRS-FIELD
                   MOVE    "Informe o modelo do veículo."
                                                 TO    VRS-MESSAGE
               WHEN WS-CHASSIS-LEN > 0 AND WS-CHASSIS-LEN NOT = 17
                   MOVE    "INVALID"             TO    VRS-STATUS
                   MOVE    "CHASSIS"             TO    VRS-FIELD
                   MOVE    "O chassi deve ter 17 caracteres."
                                                 TO    VRS-MESSAGE
               WHEN VRQ-YEAR < 1900 OR VRQ-YEAR > 2100
                   MOVE    "INVALID"             TO    VRS-STATUS
                   MOVE    "YEAR"                TO    VRS-FIELD
                   MOVE    "O ano deve estar entre 1900 e 2100."
                                                 TO    VRS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

       STORE-VEHICLE.
           OPEN    INPUT CUSTOMER-FILE
           IF    WS-FS-CUS = "00"
               MOVE    VRQ-CUS-ID                TO    CUS-ID
               PERFORM GET-CUSTOMER-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "NOCUSTOMER"          TO    VRS-STATUS
                   MOVE    "O cliente informado não está cadastrado."
                                                 TO    VRS-MESSAGE
               END-IF
               CLOSE   CUSTOMER-FILE
           ELSE
               MOVE    "NOCUSTOMER"              TO    VRS-STATUS
               MOVE    "O cliente informado não está cadastrado."
                                                 TO    VRS-MESSAGE
           END-IF

           IF    VRS-STATUS = "OK"
               OPEN    I-O VEHICLE-FILE
               IF    WS-FS-VEH = "35"
                   OPEN    OUTPUT VEHICLE-FILE
                   CLOSE   VEHICLE-FILE
                   OPEN    I-O VEHICLE-FILE
               END-IF

               IF    WS-FS-VEH NOT = "00"
                   MOVE    "ERROR"               TO    VRS-STATUS
                   MOVE    "Arquivo de veículos indisponível."
                                                 TO    VRS-MESSAGE
               ELSE
                   MOVE    WS-PLATE-OUT          TO    VEH-PLATE
                   PERFORM GET-VEHICLE-ROW
                   IF    WS-PQ-STATUS = "00"
                       PERFORM UPDATE-VEHICLE
                   ELSE
                       PERFORM INSERT-VEHICLE
                   END-IF
                   CLOSE   VEHICLE-FILE
               END-IF
           END-IF

           IF    VRS-STATUS = "OK"
               MOVE    VRQ-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "VEHICLE"                 TO    WS-AU-ENTITY
               MOVE    VRS-PLATE                 TO    WS-AU-REF
               MOVE    VRS-ACTION                TO    WS-AU-ACTION
               MOVE    VRQ-MODEL                 TO    WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF.

       INSERT-VEHICLE.
           INITIALIZE VEHICLE-RECORD
           MOVE    WS-PLATE-OUT                  TO    VEH-PLATE
           PERFORM MOVE-REQUEST-TO-RECORD
           PERFORM INSERT-VEHICLE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    "CREATED"                 TO    VRS-ACTION
               MOVE    VEH-PLATE                 TO    VRS-PLATE
           ELSE
               MOVE    "ERROR"                   TO    VRS-STATUS
               MOVE    "Não foi possível gravar o veículo."
                                                 TO    VRS-MESSAGE
           END-IF.

       UPDATE-VEHICLE.
           PERFORM MOVE-REQUEST-TO-RECORD
           PERFORM UPDATE-VEHICLE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    "UPDATED"                 TO    VRS-ACTION
               MOVE    VEH-PLATE                 TO    VRS-PLATE
           ELSE
               MOVE    "ERROR"                   TO    VRS-STATUS
               MOVE    "Não foi possível alterar o veículo."
                                                 TO    VRS-MESSAGE
           END-IF.

       MOVE-REQUEST-TO-RECORD.
           MOVE    VRQ-CUS-ID                    TO    VEH-CUS-ID
           MOVE    VRQ-CHASSIS                   TO    VEH-CHASSIS
           MOVE    VRQ-BRAND                     TO    VEH-BRAND
           MOVE    VRQ-MODEL                     TO    VEH-MODEL
           MOVE    VRQ-YEAR                      TO    VEH-YEAR
           MOVE    VRQ-COLOR                     TO    VEH-COLOR
           MOVE    VRQ-MILEAGE                   TO    VEH-MILEAGE.

           COPY "backend/COPYBOOKS/pd-plate".
           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pq-vehicles".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

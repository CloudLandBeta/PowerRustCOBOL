       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIAG-COLLECT.
      *> Collects the data of a vehicle that has been connected to the computer and saves it as a diagnosis: the date of
      *> entry, the vehicle model and the sensor readings (engine temperature, rpm, battery, oil pressure) and the fault
      *> codes (R5, R8). The collector is simulated in this version (R5a): the readings are made from the plate, so the
      *> same vehicle always reads the same, and the fault codes come from the fault rules (faultrules.cidx). A real
      *> reader replaces PARA "SIMULATE-COLLECTION" and nothing else.
      *> If the connection drops (DCQ-INTERRUPT-AFTER 1 to 3) what arrived is saved, the answer is PARTIAL, and the
      *> consultant completes it by hand with mode M and the same id; hand-typed values are marked as manual (R7).
      *> Every save writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-diag.cpy and res-diag.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT DIAGNOSIS-FILE ASSIGN TO "data/diagnoses.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS DGN-ID
               ALTERNATE RECORD KEY IS DGN-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-DGN.
           SELECT VEHICLE-FILE ASSIGN TO "data/vehicles.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS VEH-PLATE
               ALTERNATE RECORD KEY IS VEH-CUS-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-VEH.
           SELECT FAULT-RULE-FILE ASSIGN TO "data/faultrules.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS FLT-CODE
               FILE STATUS IS WS-FS-FLT.
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/diagnoses.FD".
           COPY "COPYBOOKS/vehicles.FD".
           COPY "COPYBOOKS/faultrules.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
           COPY "backend/COPYBOOKS/ws-plate".
       01 WS-FS-DGN                              PIC    XX.
       01 WS-FS-VEH                              PIC    XX.
       01 WS-FS-FLT                              PIC    XX.
       01 WS-SEED                                PIC    9(9).
       01 WS-QUOTIENT                            PIC    9(9).
       01 WS-REMAINDER                           PIC    9(9).
       01 WS-RULE-COUNT                          PIC    99.
       01 WS-RULE-CODE                           PIC    X(8)    OCCURS 10.
       01 WS-PICK-1                              PIC    99.
       01 WS-PICK-2                              PIC    99.
       01 WS-EOF                                 PIC    X.
       01 WS-ACTION                              PIC    X(12).

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-diag".
           COPY "backend/COPYBOOKS/res-diag".

       PROCEDURE DIVISION USING DGC-REQ RETURNING DGC-RES.
       MAIN-PARA.
           INITIALIZE DGC-RES
           MOVE    "OK"                          TO    DCS-STATUS

           MOVE    DCQ-PLATE                     TO    WS-PLATE-IN
           PERFORM NORMALIZE-PLATE
           PERFORM VALIDATE-REQUEST

           IF    DCS-STATUS = "OK"
               PERFORM READ-VEHICLE
           END-IF

           IF    DCS-STATUS = "OK"
               OPEN    I-O DIAGNOSIS-FILE
               IF    WS-FS-DGN = "35"
                   OPEN    OUTPUT DIAGNOSIS-FILE
                   CLOSE   DIAGNOSIS-FILE
                   OPEN    I-O DIAGNOSIS-FILE
               END-IF

               IF    WS-FS-DGN NOT = "00"
                   MOVE    "ERROR"               TO    DCS-STATUS
                   MOVE    "Arquivo de diagnósticos indisponível."
                                                 TO    DCS-MESSAGE
               ELSE
                   IF    DCQ-MODE = "S"
                       PERFORM SIMULATE-COLLECTION
                       PERFORM INSERT-DIAGNOSIS
                   ELSE
                       IF    DCQ-ID = 0
                           PERFORM INSERT-MANUAL
                       ELSE
                           PERFORM COMPLETE-BY-HAND
                       END-IF
                   END-IF
                   CLOSE   DIAGNOSIS-FILE
               END-IF
           END-IF

           IF    (DCS-STATUS = "OK" OR DCS-STATUS = "PARTIAL") AND DCS-ID > 0
               MOVE    DCQ-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "DIAGNOSIS"               TO    WS-AU-ENTITY
               MOVE    DCS-ID                    TO    WS-AU-REF
               MOVE    WS-ACTION                 TO    WS-AU-ACTION
               MOVE    DCQ-PLATE                 TO    WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       VALIDATE-REQUEST.
           EVALUATE TRUE
               WHEN DCQ-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "OPERATOR"            TO    DCS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    DCS-MESSAGE
               WHEN WS-PLATE-OK = 0
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "PLATE"               TO    DCS-FIELD
                   MOVE    "A placa deve ter 7 letras ou números."
                                                 TO    DCS-MESSAGE
               WHEN DCQ-MODE NOT = "S" AND DCQ-MODE NOT = "M"
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "MODE"                TO    DCS-FIELD
                   MOVE    "O modo deve ser S (coleta) ou M (manual)."
                                                 TO    DCS-MESSAGE
               WHEN DCQ-MODE = "M" AND DCQ-ENGINE-TEMP > 200
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "TEMP"                TO    DCS-FIELD
                   MOVE    "Temperatura do motor acima de 200 graus."
                                                 TO    DCS-MESSAGE
               WHEN DCQ-MODE = "M" AND DCQ-RPM > 12000
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "RPM"                 TO    DCS-FIELD
                   MOVE    "Rotação acima de 12000 rpm."
                                                 TO    DCS-MESSAGE
               WHEN DCQ-MODE = "M" AND DCQ-BATTERY > 30.0
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "BATTERY"             TO    DCS-FIELD
                   MOVE    "Tensão da bateria acima de 30 volts."
                                                 TO    DCS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

      *> READ-VEHICLE: the vehicle must be on file; its model goes on the diagnosis.
       READ-VEHICLE.
           MOVE    "NOVEHICLE"                   TO    DCS-STATUS
           MOVE    "Veículo não cadastrado. Faça o atendimento primeiro."
                                                 TO    DCS-MESSAGE
           OPEN    INPUT VEHICLE-FILE
           IF    WS-FS-VEH = "00"
               MOVE    WS-PLATE-OUT              TO    VEH-PLATE
               PERFORM GET-VEHICLE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "OK"                  TO    DCS-STATUS
                   MOVE    SPACES                TO    DCS-MESSAGE
                   MOVE    VEH-MODEL             TO    DCS-MODEL
               END-IF
               CLOSE   VEHICLE-FILE
           END-IF.

      *> SIMULATE-COLLECTION: the collector. Readings are made from the plate (the same plate reads the same) and the
      *> fault codes are taken from the fault rules. A real reader replaces this paragraph and fills the same DCS-
      *> items: DCS-COLLECTED, the four readings and the fault codes.
       SIMULATE-COLLECTION.
           MOVE    "S"                           TO    DCS-SOURCE
           MOVE    "COLLECT-S"                   TO    WS-ACTION
           PERFORM MAKE-SEED
           PERFORM LOAD-FAULT-RULES

           DIVIDE  WS-SEED BY 12 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           COMPUTE DCS-ENGINE-TEMP = 85 + WS-REMAINDER
           DIVIDE  WS-SEED BY 90 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           COMPUTE DCS-RPM = 750 + WS-REMAINDER
           DIVIDE  WS-SEED BY 7 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           COMPUTE DCS-BATTERY = 12.2 + WS-REMAINDER / 10
           DIVIDE  WS-SEED BY 50 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           COMPUTE DCS-OIL-PRESSURE = 280.0 + WS-REMAINDER

           IF    WS-RULE-COUNT > 0
               DIVIDE  WS-SEED BY WS-RULE-COUNT GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
               COMPUTE WS-PICK-1 = WS-REMAINDER + 1
               MOVE    WS-RULE-CODE (WS-PICK-1)  TO    DCS-FAULT-1
               DIVIDE  WS-SEED BY 3 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
               IF    WS-REMAINDER = 0 AND WS-RULE-COUNT > 1
                   COMPUTE WS-PICK-2 = WS-PICK-1 + 1
                   IF    WS-PICK-2 > WS-RULE-COUNT
                       MOVE    1                 TO    WS-PICK-2
                   END-IF
                   MOVE    WS-RULE-CODE (WS-PICK-2)  TO DCS-FAULT-2
               END-IF
           END-IF

      *> A fault shows in the readings it would really change.
           EVALUATE DCS-FAULT-1
               WHEN "P0217"
                   MOVE    118                   TO    DCS-ENGINE-TEMP
               WHEN "P0562"
                   MOVE    11.2                  TO    DCS-BATTERY
               WHEN "P0300"
                   MOVE    640                   TO    DCS-RPM
               WHEN "P0301"
                   MOVE    640                   TO    DCS-RPM
               WHEN OTHER
                   CONTINUE
           END-EVALUATE

           MOVE    4                             TO    DCS-COLLECTED

           IF    DCQ-INTERRUPT-AFTER > 0 AND DCQ-INTERRUPT-AFTER < 4
               MOVE    DCQ-INTERRUPT-AFTER       TO    DCS-COLLECTED
               MOVE    "PARTIAL"                 TO    DCS-STATUS
               MOVE    "A conexão com o veículo foi interrompida. Complete os valores à mão."
                                                 TO    DCS-MESSAGE
               IF    DCS-COLLECTED < 4
                   MOVE    0                     TO    DCS-OIL-PRESSURE
               END-IF
               IF    DCS-COLLECTED < 3
                   MOVE    0                     TO    DCS-BATTERY
               END-IF
               IF    DCS-COLLECTED < 2
                   MOVE    0                     TO    DCS-RPM
               END-IF
               MOVE    SPACES                    TO    DCS-FAULT-1
               MOVE    SPACES                    TO    DCS-FAULT-2
           END-IF.

      *> MAKE-SEED: a number made from the seven characters of the plate: the same plate gives the same number.
       MAKE-SEED.
           MOVE    0                             TO    WS-SEED
           PERFORM VARYING WS-PLATE-POS FROM 1 BY 1 UNTIL WS-PLATE-POS > 7
               COMPUTE WS-SEED = WS-SEED * 31
                   + FUNCTION ORD (WS-PLATE-OUT (WS-PLATE-POS:1))
               DIVIDE  WS-SEED BY 1000003 GIVING WS-QUOTIENT REMAINDER WS-SEED
           END-PERFORM.

      *> LOAD-FAULT-RULES: the fault codes of the rules file, in key order, at most 10.
       LOAD-FAULT-RULES.
           MOVE    0                             TO    WS-RULE-COUNT
           MOVE    "N"                           TO    WS-EOF
           OPEN    INPUT FAULT-RULE-FILE
           IF    WS-FS-FLT = "00"
               PERFORM SELECT-ALL-FAULT-RULES
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               END-IF
               PERFORM UNTIL WS-EOF = "Y" OR WS-RULE-COUNT = 10
                   PERFORM FETCH-FAULT-RULE-ROW
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       ADD     1                 TO    WS-RULE-COUNT
                       MOVE    FLT-CODE          TO    WS-RULE-CODE (WS-RULE-COUNT)
                   END-IF
               END-PERFORM
               CLOSE   FAULT-RULE-FILE
           END-IF.

       INSERT-MANUAL.
           MOVE    "M"                           TO    DCS-SOURCE
           MOVE    "COLLECT-M"                   TO    WS-ACTION
           MOVE    4                             TO    DCS-COLLECTED
           MOVE    DCQ-ENGINE-TEMP               TO    DCS-ENGINE-TEMP
           MOVE    DCQ-RPM                       TO    DCS-RPM
           MOVE    DCQ-BATTERY                   TO    DCS-BATTERY
           MOVE    DCQ-OIL-PRESSURE              TO    DCS-OIL-PRESSURE
           MOVE    DCQ-FAULT-1                   TO    DCS-FAULT-1
           MOVE    DCQ-FAULT-2                   TO    DCS-FAULT-2
           MOVE    DCQ-FAULT-3                   TO    DCS-FAULT-3
           MOVE    DCQ-FAULT-4                   TO    DCS-FAULT-4
           MOVE    DCQ-FAULT-5                   TO    DCS-FAULT-5
           PERFORM INSERT-DIAGNOSIS.

      *> INSERT-DIAGNOSIS: numbers and writes a new diagnosis from the DCS- items.
       INSERT-DIAGNOSIS.
           MOVE    "DIAGNOSIS"                   TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER
           PERFORM NOW-STAMP

           IF    WS-SEQ-GOT = 0
               MOVE    "ERROR"                   TO    DCS-STATUS
               MOVE    "Não foi possível numerar o diagnóstico."
                                                 TO    DCS-MESSAGE
           ELSE
               INITIALIZE DIAGNOSIS-RECORD
               MOVE    WS-SEQ-GOT                TO    DGN-ID
               MOVE    WS-PLATE-OUT              TO    DGN-PLATE
               MOVE    WS-NOW                    TO    DGN-AT
               PERFORM DCS-TO-RECORD
               PERFORM INSERT-DIAGNOSIS-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    DGN-ID                TO    DCS-ID
                   MOVE    DGN-AT                TO    DCS-AT
               ELSE
                   MOVE    "ERROR"               TO    DCS-STATUS
                   MOVE    "Não foi possível gravar o diagnóstico."
                                                 TO    DCS-MESSAGE
               END-IF
           END-IF.

      *> COMPLETE-BY-HAND: the typed values replace those of a partial collection; the diagnosis becomes manual.
       COMPLETE-BY-HAND.
           MOVE    DCQ-ID                        TO    DGN-ID
           PERFORM GET-DIAGNOSIS-ROW
           IF    WS-PQ-STATUS = "00"
               IF    DGN-PLATE NOT = WS-PLATE-OUT
                   MOVE    "INVALID"             TO    DCS-STATUS
                   MOVE    "PLATE"               TO    DCS-FIELD
                   MOVE    "Esse diagnóstico é de outro veículo."
                                                 TO    DCS-MESSAGE
               ELSE
                   MOVE    "M"                   TO    DCS-SOURCE
                   MOVE    "COMPLETE"            TO    WS-ACTION
                   MOVE    4                     TO    DCS-COLLECTED
                   MOVE    DCQ-ENGINE-TEMP       TO    DCS-ENGINE-TEMP
                   MOVE    DCQ-RPM               TO    DCS-RPM
                   MOVE    DCQ-BATTERY           TO    DCS-BATTERY
                   MOVE    DCQ-OIL-PRESSURE      TO    DCS-OIL-PRESSURE
                   MOVE    DCQ-FAULT-1           TO    DCS-FAULT-1
                   MOVE    DCQ-FAULT-2           TO    DCS-FAULT-2
                   MOVE    DCQ-FAULT-3           TO    DCS-FAULT-3
                   MOVE    DCQ-FAULT-4           TO    DCS-FAULT-4
                   MOVE    DCQ-FAULT-5           TO    DCS-FAULT-5
                   PERFORM DCS-TO-RECORD
                   PERFORM UPDATE-DIAGNOSIS-ROW
                   IF    WS-PQ-STATUS = "00"
                       MOVE    DGN-ID            TO    DCS-ID
                       MOVE    DGN-AT            TO    DCS-AT
                   ELSE
                       MOVE    "ERROR"           TO    DCS-STATUS
                       MOVE    "Não foi possível alterar o diagnóstico."
                                                 TO    DCS-MESSAGE
                   END-IF
               END-IF
           ELSE
               MOVE    "NOTFOUND"                TO    DCS-STATUS
               MOVE    "Diagnóstico não encontrado."
                                                 TO    DCS-MESSAGE
           END-IF.

       DCS-TO-RECORD.
           MOVE    DCS-SOURCE                    TO    DGN-SOURCE
           MOVE    DCS-MODEL                     TO    DGN-MODEL
           MOVE    DCS-ENGINE-TEMP               TO    DGN-ENGINE-TEMP
           MOVE    DCS-RPM                       TO    DGN-RPM
           MOVE    DCS-BATTERY                   TO    DGN-BATTERY
           MOVE    DCS-OIL-PRESSURE              TO    DGN-OIL-PRESSURE
           MOVE    DCS-FAULT-1                   TO    DGN-FAULT-1
           MOVE    DCS-FAULT-2                   TO    DGN-FAULT-2
           MOVE    DCS-FAULT-3                   TO    DGN-FAULT-3
           MOVE    DCS-FAULT-4                   TO    DGN-FAULT-4
           MOVE    DCS-FAULT-5                   TO    DGN-FAULT-5.

           COPY "backend/COPYBOOKS/pd-plate".
           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pq-vehicles".
           COPY "backend/COPYBOOKS/pq-faultrules".
           COPY "backend/COPYBOOKS/pq-diagnoses".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VEHICLE-FIND.
      *> Looks a vehicle up by licence plate and answers it together with its owner, so the Intake screen can fill
      *> the vehicle and the customer in one step and never create a duplicate (R3). It only reads: nothing is written.
      *> The request and the answer are in backend/COPYBOOKS/req-vehicle-find.cpy and res-vehicle-find.cpy.

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

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/vehicles.FD".
           COPY "COPYBOOKS/customers.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-plate".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-VEH                              PIC    XX.
       01 WS-FS-CUS                              PIC    XX.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-vehicle-find".
           COPY "backend/COPYBOOKS/res-vehicle-find".

       PROCEDURE DIVISION USING VFD-REQ RETURNING VFD-RES.
       MAIN-PARA.
           INITIALIZE VFD-RES
           MOVE    VFQ-PLATE                     TO    WS-PLATE-IN
           PERFORM NORMALIZE-PLATE

           IF    WS-PLATE-OK = 0
               MOVE    "INVALID"                 TO    VFS-STATUS
           ELSE
               MOVE    WS-PLATE-OUT              TO    VFS-PLATE
               MOVE    "NOTFOUND"                TO    VFS-STATUS
               PERFORM READ-VEHICLE
           END-IF

           GOBACK.

       READ-VEHICLE.
           OPEN    INPUT VEHICLE-FILE
           IF    WS-FS-VEH = "00"
               MOVE    WS-PLATE-OUT              TO    VEH-PLATE
               PERFORM GET-VEHICLE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "FOUND"               TO    VFS-STATUS
                   MOVE    VEH-CHASSIS           TO    VFS-CHASSIS
                   MOVE    VEH-BRAND             TO    VFS-BRAND
                   MOVE    VEH-MODEL             TO    VFS-MODEL
                   MOVE    VEH-YEAR              TO    VFS-YEAR
                   MOVE    VEH-COLOR             TO    VFS-COLOR
                   MOVE    VEH-MILEAGE           TO    VFS-MILEAGE
                   MOVE    VEH-CUS-ID            TO    VFS-CUS-ID
               END-IF
               CLOSE   VEHICLE-FILE
           END-IF

           IF    VFS-STATUS = "FOUND"
               PERFORM READ-OWNER
           END-IF.

       READ-OWNER.
           OPEN    INPUT CUSTOMER-FILE
           IF    WS-FS-CUS = "00"
               MOVE    VFS-CUS-ID                TO    CUS-ID
               PERFORM GET-CUSTOMER-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    CUS-NAME              TO    VFS-CUS-NAME
                   MOVE    CUS-DOC               TO    VFS-CUS-DOC
                   MOVE    CUS-ADDRESS           TO    VFS-CUS-ADDRESS
                   MOVE    CUS-PHONE             TO    VFS-CUS-PHONE
                   MOVE    CUS-WHATSAPP          TO    VFS-CUS-WHATSAPP
                   MOVE    CUS-EMAIL             TO    VFS-CUS-EMAIL
                   MOVE    CUS-CONSENT-EMAIL     TO    VFS-CUS-CONSENT-EMAIL
                   MOVE    CUS-CONSENT-WA        TO    VFS-CUS-CONSENT-WA
               END-IF
               CLOSE   CUSTOMER-FILE
           END-IF.

           COPY "backend/COPYBOOKS/pd-plate".
           COPY "backend/COPYBOOKS/pq-vehicles".
           COPY "backend/COPYBOOKS/pq-customers".

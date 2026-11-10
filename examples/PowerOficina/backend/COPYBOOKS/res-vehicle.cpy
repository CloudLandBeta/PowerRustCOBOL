      *> VEH-RES: the answer of VEHICLE-SAVE.
      *> VRS-STATUS  OK | INVALID | NOCUSTOMER | ERROR
      *> VRS-FIELD   with INVALID, the field that failed: PLATE CHASSIS YEAR MODEL OPERATOR.
      *> VRS-ACTION  CREATED | UPDATED
      *> VRS-PLATE   the plate as stored.
       01 VEH-RES.
          05 VRS-STATUS                          PIC    X(10).
          05 VRS-MESSAGE                         PIC    X(80).
          05 VRS-FIELD                           PIC    X(10).
          05 VRS-ACTION                          PIC    X(8).
          05 VRS-PLATE                           PIC    X(8).

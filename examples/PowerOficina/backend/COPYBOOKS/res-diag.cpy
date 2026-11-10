      *> DGC-RES: the answer of DIAG-COLLECT: the collection as saved.
      *> DCS-STATUS     OK | PARTIAL (the connection dropped; what arrived is saved, complete it by hand with mode M and
      *>                DCS-ID) | NOVEHICLE | INVALID | NOTFOUND | ERROR
      *> DCS-FIELD      with INVALID, the field that failed: PLATE OPERATOR MODE TEMP RPM BATTERY OIL.
      *> DCS-SOURCE     S collected, M typed by hand.
      *> DCS-COLLECTED  how many of the 4 sensors have a value.
      *> DCS-AT         date and time of entry, YYYYMMDDHHMMSS.
       01 DGC-RES.
          05 DCS-STATUS                          PIC    X(10).
          05 DCS-MESSAGE                         PIC    X(80).
          05 DCS-FIELD                           PIC    X(10).
          05 DCS-ID                              PIC    9(8).
          05 DCS-AT                              PIC    9(14).
          05 DCS-SOURCE                          PIC    X(1).
          05 DCS-MODEL                           PIC    X(30).
          05 DCS-COLLECTED                       PIC    9(1).
          05 DCS-ENGINE-TEMP                     PIC    9(3).
          05 DCS-RPM                             PIC    9(5).
          05 DCS-BATTERY                         PIC    9(2)V9.
          05 DCS-OIL-PRESSURE                    PIC    9(3)V9.
          05 DCS-FAULT-1                         PIC    X(8).
          05 DCS-FAULT-2                         PIC    X(8).
          05 DCS-FAULT-3                         PIC    X(8).
          05 DCS-FAULT-4                         PIC    X(8).
          05 DCS-FAULT-5                         PIC    X(8).

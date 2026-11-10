      *> DGC-REQ: what the Diagnosis screen sends to DIAG-COLLECT.
      *> DCQ-MODE            S = collect from the vehicle connection (simulated in this version, behind one
      *>                     collector interface, R5a); M = values typed by hand, marked as manual (R7).
      *> DCQ-ID              0 = a new collection; otherwise the partial collection to complete by hand (mode M).
      *> DCQ-INTERRUPT-AFTER mode S only: 1 to 3 simulates the connection dropping after that many sensors
      *>                     (engine temperature, rpm, battery, oil pressure, in that order); 0 = no interruption.
      *> DCQ-ENGINE-TEMP .. DCQ-FAULT-5   mode M only: the values typed. Temperature in degrees Celsius, battery in
      *>                     volts, oil pressure in kPa. A fault code is the vehicle's diagnostic code, such as P0300.
       01 DGC-REQ.
          05 DCQ-PLATE                           PIC    X(8).
          05 DCQ-OPERATOR                        PIC    X(30).
          05 DCQ-MODE                            PIC    X(1).
          05 DCQ-ID                              PIC    9(8).
          05 DCQ-INTERRUPT-AFTER                 PIC    9(1).
          05 DCQ-ENGINE-TEMP                     PIC    9(3).
          05 DCQ-RPM                             PIC    9(5).
          05 DCQ-BATTERY                         PIC    9(2)V9.
          05 DCQ-OIL-PRESSURE                    PIC    9(3)V9.
          05 DCQ-FAULT-1                         PIC    X(8).
          05 DCQ-FAULT-2                         PIC    X(8).
          05 DCQ-FAULT-3                         PIC    X(8).
          05 DCQ-FAULT-4                         PIC    X(8).
          05 DCQ-FAULT-5                         PIC    X(8).

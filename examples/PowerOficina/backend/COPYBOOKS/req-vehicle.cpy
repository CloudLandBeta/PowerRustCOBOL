      *> VEH-REQ: what the Intake screen sends to VEHICLE-SAVE.
      *> VRQ-PLATE   as typed (the program stores it in capitals, without hyphen or space).
      *> VRQ-CUS-ID  the owner, a customer already saved by CUSTOMER-SAVE.
      *> A plate that is already registered is changed, never duplicated (R3).
       01 VEH-REQ.
          05 VRQ-PLATE                           PIC    X(8).
          05 VRQ-CUS-ID                          PIC    9(8).
          05 VRQ-CHASSIS                         PIC    X(17).
          05 VRQ-BRAND                           PIC    X(20).
          05 VRQ-MODEL                           PIC    X(30).
          05 VRQ-YEAR                            PIC    9(4).
          05 VRQ-COLOR                           PIC    X(15).
          05 VRQ-MILEAGE                         PIC    9(7).
          05 VRQ-OPERATOR                        PIC    X(30).

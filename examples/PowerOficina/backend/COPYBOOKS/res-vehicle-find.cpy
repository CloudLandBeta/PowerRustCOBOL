      *> VFD-RES: the answer of VEHICLE-FIND: the vehicle and its owner, so the screen fills both at once (R3).
      *> VFS-STATUS  FOUND | NOTFOUND | INVALID (the plate is not 7 letters or digits)
      *> VFS-PLATE   the plate as stored, whether or not it was found.
      *> Owner fields are blank when the vehicle has no customer on file.
       01 VFD-RES.
          05 VFS-STATUS                          PIC    X(10).
          05 VFS-PLATE                           PIC    X(8).
          05 VFS-CHASSIS                         PIC    X(17).
          05 VFS-BRAND                           PIC    X(20).
          05 VFS-MODEL                           PIC    X(30).
          05 VFS-YEAR                            PIC    9(4).
          05 VFS-COLOR                           PIC    X(15).
          05 VFS-MILEAGE                         PIC    9(7).
          05 VFS-CUS-ID                          PIC    9(8).
          05 VFS-CUS-NAME                        PIC    X(60).
          05 VFS-CUS-DOC                         PIC    X(14).
          05 VFS-CUS-ADDRESS                     PIC    X(80).
          05 VFS-CUS-PHONE                       PIC    X(20).
          05 VFS-CUS-WHATSAPP                    PIC    X(20).
          05 VFS-CUS-EMAIL                       PIC    X(60).
          05 VFS-CUS-CONSENT-EMAIL               PIC    X(1).
          05 VFS-CUS-CONSENT-WA                  PIC    X(1).

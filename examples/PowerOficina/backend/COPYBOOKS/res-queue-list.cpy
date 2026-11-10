       01 QLN-RES.
          05 QLS-STATUS                          PIC    X(10).
          05 QLS-MESSAGE                         PIC    X(80).
          05 QLS-COUNT                           PIC    9(3).
          05 QLS-READY                           PIC    9(3).
          05 QLS-RUNNING                         PIC    9(3).
          05 QLS-QUEUED                          PIC    9(3).
          05 QLS-ROW                             OCCURS 40.
             10 QLR-ID                           PIC    9(8).
             10 QLR-QUO-ID                       PIC    9(8).
             10 QLR-PLATE                        PIC    X(8).
             10 QLR-SERVICE                      PIC    X(40).
             10 QLR-STATE                        PIC    X(10).
             10 QLR-PRIORITY                     PIC    9(2).
             10 QLR-APPROVED-AT                  PIC    9(14).
             10 QLR-DEADLINE                     PIC    9(8).
             10 QLR-READY                        PIC    X(1).

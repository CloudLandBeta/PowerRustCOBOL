      *> PAYMENT-CONFIRM answer.
       01 PCF-RES.
          05 PCS-STATUS                          PIC    X(10).
          05 PCS-MESSAGE                         PIC    X(80).
          05 PCS-FIELD                           PIC    X(10).
          05 PCS-PAY-ID                          PIC    9(8).
          05 PCS-QUO-ID                          PIC    9(8).
          05 PCS-CURRENCY                        PIC    X(3).
          05 PCS-TOTAL                           PIC    9(9)V99.
          05 PCS-N                               PIC    9(2).
          05 PCS-GW-REF                          PIC    X(40).
          05 PCS-QUEUED                          PIC    9(2).
          05 PCS-INS                             OCCURS 12.
             10 PCI-NO                           PIC    9(2).
             10 PCI-DUE                          PIC    9(8).
             10 PCI-AMOUNT                       PIC    9(9)V99.
             10 PCI-STATUS                       PIC    X(10).

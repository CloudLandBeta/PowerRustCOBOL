      *> PAYMENT-CONFIRM request: mode P shows the schedule, mode C charges the first installment.
       01 PCF-REQ.
          05 PCQ-MODE                            PIC    X(1).
          05 PCQ-QUO-ID                          PIC    9(8).
          05 PCQ-INSTALLMENTS                    PIC    9(2).
          05 PCQ-CONSENT                         PIC    X(1).
          05 PCQ-BRAND                           PIC    X(12).
          05 PCQ-LAST4                           PIC    X(4).
          05 PCQ-GATEWAY                         PIC    X(1).
          05 PCQ-OPERATOR                        PIC    X(30).

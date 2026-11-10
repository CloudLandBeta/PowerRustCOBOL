      *> INSTALLMENT-RUN request: charges the installments due up to the date (0 = today); gateway D declines them.
       01 IRN-REQ.
          05 IRQ-ASOF                            PIC    9(8).
          05 IRQ-GATEWAY                         PIC    X(1).
          05 IRQ-OPERATOR                        PIC    X(30).

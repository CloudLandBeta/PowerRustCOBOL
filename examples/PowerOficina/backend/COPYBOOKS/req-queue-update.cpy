      *> QUEUE-UPDATE request: S starts a queued service, F finishes a running one, D changes the delivery date of an order.
       01 QUN-REQ.
          05 QUQ-ACTION                          PIC    X(1).
          05 QUQ-QUE-ID                          PIC    9(8).
          05 QUQ-QUO-ID                          PIC    9(8).
          05 QUQ-DEADLINE                        PIC    9(8).
          05 QUQ-OPERATOR                        PIC    X(30).

      *> QSN-REQ: asks QUOTE-SEND to send the latest revision of a quote to the customer (R18).
      *> QSN-DOWN-EMAIL / QSN-DOWN-WA: S simulates that channel being out of service, to prove that the other one is still
      *> delivered (R19). They are test switches of the simulated notifier; a real notifier ignores them.
       01 QSN-REQ.
          05 QSN-ID                              PIC    9(8).
          05 QSN-OPERATOR                        PIC    X(30).
          05 QSN-DOWN-EMAIL                      PIC    X(1).
          05 QSN-DOWN-WA                         PIC    X(1).

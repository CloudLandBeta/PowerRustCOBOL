      *> QSN-RES: the result of the sending, channel by channel (R18, R19).
      *> QNS-STATUS   OK (at least one channel delivered) | NOCHANNEL (none delivered) | NOTFOUND | NOTOPEN | INVALID | ERROR
      *> QNS-EMAIL / QNS-WA   SENT, FAILED (after QNS-...-TRIES tries) or SKIPPED (no consent or no contact for that channel)
      *> QNS-TOKEN, QNS-CODE   the link token and the short code of the message. In this version the messages go nowhere, so
      *>                       the screen shows them; a real notifier sends them to the customer only.
      *> QNS-TEXT     the text of the message that was recorded.
       01 QSN-RES.
          05 QNS-STATUS                          PIC    X(10).
          05 QNS-MESSAGE                         PIC    X(80).
          05 QNS-ID                              PIC    9(8).
          05 QNS-REV                             PIC    9(2).
          05 QNS-EMAIL                           PIC    X(8).
          05 QNS-EMAIL-TRIES                     PIC    9.
          05 QNS-WA                              PIC    X(8).
          05 QNS-WA-TRIES                        PIC    9.
          05 QNS-TOKEN                           PIC    X(32).
          05 QNS-CODE                            PIC    X(6).
          05 QNS-EXPIRES                         PIC    9(14).
          05 QNS-TEXT                            PIC    X(200).

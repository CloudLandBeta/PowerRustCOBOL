      *> QUO-RES: a quote as saved (the answer of QUOTE-SAVE and QUOTE-GET).
      *> QTS-STATUS  OK | INVALID | NOVEHICLE | NODIAG | NOTFOUND | ERROR
      *> QTS-FIELD   with INVALID: OPERATOR PLATE CURRENCY DIAG LINE KIND HOURS NOTE DEADLINE COUNT;
      *>             QTS-LINE-NO says which line when the field is a line field.
      *> QTS-ACTION  CREATED | REVISED | NEWCURRENCY (a new quote made from another one, R43)
      *> Values are in the quote's own currency. QTS-SUBTOTAL is the total WITHOUT taxes, QTS-TAX the taxes of every
      *> line added up and QTS-TOTAL the two together (R38). QTS-SUGGESTED is the date the queue suggested (R16).
       01 QUO-RES.
          05 QTS-STATUS                          PIC    X(10).
          05 QTS-MESSAGE                         PIC    X(80).
          05 QTS-FIELD                           PIC    X(10).
          05 QTS-LINE-NO                         PIC    9(3).
          05 QTS-ACTION                          PIC    X(12).
          05 QTS-ID                              PIC    9(8).
          05 QTS-REV                             PIC    9(2).
          05 QTS-CUS-ID                          PIC    9(8).
          05 QTS-PLATE                           PIC    X(8).
          05 QTS-DGN-ID                          PIC    9(8).
          05 QTS-CURRENCY                        PIC    X(3).
          05 QTS-QSTATUS                         PIC    X(10).
          05 QTS-CREATED                         PIC    9(14).
          05 QTS-VALID-UNTIL                     PIC    9(8).
          05 QTS-DEADLINE                        PIC    9(8).
          05 QTS-SUGGESTED                       PIC    9(8).
          05 QTS-HOURS                           PIC    9(4)V99.
          05 QTS-SUBTOTAL                        PIC    9(9)V99.
          05 QTS-TAX                             PIC    9(9)V99.
          05 QTS-TOTAL                           PIC    9(9)V99.
          05 QTS-OPERATOR                        PIC    X(30).
          05 QTS-COUNT                           PIC    9(3).
          05 QTS-LINE                            OCCURS 20.
             10 QTL-KIND                         PIC    X(1).
             10 QTL-CODE                         PIC    X(12).
             10 QTL-NAME                         PIC    X(40).
             10 QTL-QTY                          PIC    9(4)V99.
             10 QTL-UNIT                         PIC    9(7)V99.
             10 QTL-TAX-RATE                     PIC    9(2)V99.
             10 QTL-AMOUNT                       PIC    9(9)V99.
             10 QTL-TAX                          PIC    9(9)V99.
             10 QTL-MANUAL                       PIC    X(1).
             10 QTL-NOTE                         PIC    X(60).

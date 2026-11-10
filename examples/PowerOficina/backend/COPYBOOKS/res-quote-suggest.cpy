      *> QSG-RES: the parts and services the fault codes of the diagnosis call for, from the fault rules, priced from
      *> the catalog of the currency (display only: the amounts are made by QUOTE-SAVE). A service line carries its
      *> standard hours in QSL-HOURS (R11).
      *> QSS-STATUS  OK | NODIAG | INVALID.   QSS-COUNT = 0 with OK: the diagnosis has no fault code.
       01 QSG-RES.
          05 QSS-STATUS                          PIC    X(10).
          05 QSS-MESSAGE                         PIC    X(80).
          05 QSS-COUNT                           PIC    9(3).
          05 QSS-LINE                            OCCURS 20.
             10 QSL-KIND                         PIC    X(1).
             10 QSL-CODE                         PIC    X(12).
             10 QSL-NAME                         PIC    X(40).
             10 QSL-QTY                          PIC    9(4)V99.
             10 QSL-UNIT                         PIC    9(7)V99.
             10 QSL-HOURS                        PIC    9(3)V99.
             10 QSL-FAULT                        PIC    X(8).

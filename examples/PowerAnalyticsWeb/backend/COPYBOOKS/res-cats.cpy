      *> CAR-RES: what Q-CATS leaves in WS-CAT-TAB. The 10 slots are the categories of INI-CAT-NAME, most funds
      *> first, whatever the filter; a category with no fund under the filter shows 0.
      *> CAR-N = WS-CATCNT; CAR-NAME(k) = WS-CAT-NAME(k); CAR-CNT / CAR-PL / CAR-COT(k) = WS-CAT-CNT / -PL / -COT(k)
       01 CAR-RES.
          05 CAR-STATUS                          PIC    X(8).
          05 CAR-MESSAGE                         PIC    X(160).
          05 CAR-N                               PIC    99.
          05 CAR-NAME                            PIC    X(24)    OCCURS 10.
          05 CAR-CNT                             PIC    9(9)     OCCURS 10.
          05 CAR-PL                              PIC    9(15)V99 OCCURS 10.
          05 CAR-COT                             PIC    9(12)    OCCURS 10.

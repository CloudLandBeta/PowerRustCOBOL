      *> BIN-RES: what Q-BINS leaves in WS-BIN-TAB: the funds that report a PL, by band - under R$ 1 mi, under
      *> 10 mi, under 100 mi, under 1 bi, 1 bi or more. BIN-CNT(k) = WS-BIN-CNT(k).
       01 BIN-RES.
          05 BIN-STATUS                          PIC    X(8).
          05 BIN-MESSAGE                         PIC    X(160).
          05 BIN-CNT                             PIC    9(9)     OCCURS 5.

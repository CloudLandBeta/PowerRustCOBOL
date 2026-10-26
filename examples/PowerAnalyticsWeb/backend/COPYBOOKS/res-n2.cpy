      *> N2R-RES: what Q-N2 leaves in WS-N2-TAB and WS-N2-TOTAL. Slots 1..9 hold the nine level-2 categories
      *> with most funds, largest first; slot 10 is "Outros" with all the rest, present only when there are more
      *> than nine. N2R-NAME(k) = WS-N2-NAME(k) (24 bytes), N2R-CNT(k) = WS-N2-CNT(k), N2R-TOTAL = WS-N2-TOTAL
      *> (the funds counted in every level-2 category: those with no level 2 are not).
       01 N2R-RES.
          05 N2R-STATUS                          PIC    X(8).
          05 N2R-MESSAGE                         PIC    X(160).
          05 N2R-NAME                            PIC    X(24)    OCCURS 10.
          05 N2R-CNT                             PIC    9(9)     OCCURS 10.
          05 N2R-TOTAL                           PIC    9(9).

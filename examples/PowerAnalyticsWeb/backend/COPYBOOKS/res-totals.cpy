      *> TOT-RES: what Q-TOTALS leaves in WS-TOT, field for field.
      *> TOT-CNT funds (WS-T-CNT)   TOT-PL summed PL (WS-T-PL)   TOT-COT summed cotistas (WS-T-COT)
      *> TOT-PLN funds that report a PL (WS-T-PLN)   TOT-MAX largest PL (WS-T-MAX)
      *> TOT-YMAX newest start year in the filter (WS-T-YMAX)
      *> TOT-CLS / TOT-SUB summed PL of the "Classe" funds / of every other estrutura (WS-T-CLS / WS-T-SUB)
       01 TOT-RES.
          05 TOT-STATUS                          PIC    X(8).
          05 TOT-MESSAGE                         PIC    X(160).
          05 TOT-CNT                             PIC    9(9).
          05 TOT-PL                              PIC    9(15)V99.
          05 TOT-COT                             PIC    9(12).
          05 TOT-PLN                             PIC    9(9).
          05 TOT-MAX                             PIC    9(15)V99.
          05 TOT-YMAX                            PIC    9(4).
          05 TOT-CLS                             PIC    9(15)V99.
          05 TOT-SUB                             PIC    9(15)V99.

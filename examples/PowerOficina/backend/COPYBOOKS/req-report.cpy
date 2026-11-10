      *> REPORT-LIST request: Q quotes, P payments, F failed installments, N notices not delivered; one currency (BRL or
      *> EUR) for the first three; the filter is a status (blank = all) for Q and P.
       01 RPT-REQ.
          05 RPQ-MODE                            PIC    X(1).
          05 RPQ-CURRENCY                        PIC    X(3).
          05 RPQ-FILTER                          PIC    X(10).

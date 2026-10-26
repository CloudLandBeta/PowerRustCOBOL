      *> GRD-REQ: which rows of the grid. GRD-ORDER is CMB-ORDER::Value, read the way Q-GRID reads it: "Maior ..."
      *> by PL, "Mais coti..." by cotistas, "Mais rece..." newest start first, any other "Mais ..." oldest start
      *> first, anything else by name. GRD-OFFSET is the number of rows to skip ((WS-PAGE - 1) * WS-PSIZE, plus
      *> what was already read of this page); GRD-LIMIT how many rows are wanted, 1 to 150.
       01 GRD-REQ.
          05 GRD-ORDER                           PIC    X(40).
          05 GRD-OFFSET                          PIC    9(9).
          05 GRD-LIMIT                           PIC    9(4).

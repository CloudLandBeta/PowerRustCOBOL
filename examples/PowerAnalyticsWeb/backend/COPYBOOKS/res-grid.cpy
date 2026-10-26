      *> GRD-RES: the rows Q-GRID adds to GRD-FUNDOS, in order. Each GRD-ROW(k) is the row text the desktop's
      *> Fetch() answers (WS-ROW): the 16 cells joined by TAB, before the ::Trim() that AddRow gets. A row whose
      *> first cell is empty starts with a TAB, which Trim() removes, exactly as on the desktop.
      *> GRD-N rows are answered. GRD-MORE = Y when fewer than GRD-LIMIT rows came back although the table has
      *> more in the window: the answer reached its byte budget (the module takes 64 KiB per call). Ask again
      *> from GRD-OFFSET + GRD-N. The shipped data never reaches it with 150 rows (the longest 150 rows make
      *> 55,257 bytes); another data file might.
       01 GRD-RES.
          05 GRD-STATUS                          PIC    X(8).
          05 GRD-MESSAGE                         PIC    X(160).
          05 GRD-N                               PIC    9(4).
          05 GRD-MORE                            PIC    X.
          05 GRD-ROW                             PIC    X(400)
                                                 OCCURS 0 TO 150 DEPENDING ON GRD-N.

      *> NAM-REQ: which search list. NAM-BOX = N for Nome comercial (WS-SM = "N"), C for Categoria. The text typed in
      *> that box and the names already ticked in it are read from FLT-REQ (FLT-NAME-TERM / FLT-NAME-FLAGS for N,
      *> FLT-CAT-TERM / FLT-CAT-PICK for C); the other conditions of FLT-REQ play no part, as in SM-REFRESH.
       01 NAM-REQ.
          05 NAM-BOX                             PIC    X.

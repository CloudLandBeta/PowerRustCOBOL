      *> FLT-REQ: the filter of PowerAnalytics, as the form keeps it. Every query program of the Backend
      *> takes it as its first argument. The browser sends VALUES only; the SQL is fixed in the programs.
      *>
      *> FLT-NAME-TERM   TXT-NOME::Text as typed (the first 80 bytes count, as WS-TERM of the desktop).
      *> FLT-NAME-N      how many company names are ticked (WS-ALL-N = 1, "Todos": send 0 and the text rules).
      *> FLT-NAME-PICK   the ticked names, exactly as listed (at most 150).
      *> FLT-CAT-TERM    TXT-CAT::Text as typed.      FLT-CAT-N / FLT-CAT-PICK  the ticked categories (WS-ALL-C = 1: 0).
      *> FLT-DATE-FROM   DTP-INI::Value (yyyy-mm-dd).  FLT-DATE-TO  DTP-FIM::Value.
      *> FLT-PL-FROM     NUD-PL1::Value (R$ million).   FLT-PL-TO    NUD-PL2::Value.
      *> FLT-N2-N / FLT-N2-PICK   the names ticked in LST-N2 (one per line of CheckedItems).
      *> FLT-N3-N / FLT-N3-PICK   the names ticked in LST-N3.
       01 FLT-REQ.
          05 FLT-NAME-TERM                       PIC    X(320).
          05 FLT-NAME-N                          PIC    9(3).
          05 FLT-NAME-PICK                       PIC    X(210)
                                                 OCCURS 0 TO 150 DEPENDING ON FLT-NAME-N.
          05 FLT-CAT-TERM                        PIC    X(320).
          05 FLT-CAT-N                           PIC    99.
          05 FLT-CAT-PICK                        PIC    X(60)
                                                 OCCURS 0 TO 10 DEPENDING ON FLT-CAT-N.
          05 FLT-DATE-FROM                       PIC    X(10).
          05 FLT-DATE-TO                         PIC    X(10).
          05 FLT-PL-FROM                         PIC    9(9).
          05 FLT-PL-TO                           PIC    9(9).
          05 FLT-N2-N                            PIC    99.
          05 FLT-N2-PICK                         PIC    X(60)
                                                 OCCURS 0 TO 32 DEPENDING ON FLT-N2-N.
          05 FLT-N3-N                            PIC    99.
          05 FLT-N3-PICK                         PIC    X(60)
                                                 OCCURS 0 TO 64 DEPENDING ON FLT-N3-N.

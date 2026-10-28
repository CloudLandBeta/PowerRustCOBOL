      *> FLT-REQ: the filter of PowerAnalytics, as the form keeps it. Every query program of the Backend
      *> takes it as its first argument. The browser sends VALUES only; the SQL is fixed in the programs.
      *>
      *> FLT-NAME-TERM   TXT-NOME::Text, its first 80 bytes as the desktop keeps them (WS-TERM, PIC X(80)), plus
      *>                 at most 3 more bytes: when the 80th byte falls inside a letter, the rest of that letter, so
      *>                 the text can travel (a request must be UTF-8). The programs cut it to 80 bytes again
      *>                 (SH-TERM-N), the same cut the desktop makes, letter or not.
      *> FLT-NAME-N      how many company names are ticked (WS-ALL-N = 1, "Todos": send 0 and the text rules).
      *> FLT-NAME-FLAGS  the ticked names, one byte per fund id: byte k is "1" when the name of the fund with id k
      *>                 is ticked (the form ticks a name by the id FUND-NAMES answered for it, NAM-ID: the first
      *>                 fund carrying that name). The desktop keeps its picks in a table with no limit; one byte
      *>                 per fund lets every name of the data be ticked within the 64 KiB a call may carry
      *>                 (trailing spaces do not travel: only up to the highest id ticked). Ids up to 50,000.
      *> FLT-CAT-TERM    TXT-CAT::Text, cut like FLT-NAME-TERM.
      *> FLT-CAT-N / FLT-CAT-PICK  the ticked categories (WS-ALL-C = 1: 0).
      *> FLT-DATE-FROM   DTP-INI::Value (yyyy-mm-dd).  FLT-DATE-TO  DTP-FIM::Value.
      *> FLT-PL-FROM     NUD-PL1::Value (R$ million).   FLT-PL-TO    NUD-PL2::Value.
      *> FLT-N2-N / FLT-N2-PICK   the names ticked in LST-N2 (one per line of CheckedItems).
      *> FLT-N3-N / FLT-N3-PICK   the names ticked in LST-N3.
       01 FLT-REQ.
          05 FLT-NAME-TERM                       PIC    X(83).
          05 FLT-NAME-N                          PIC    9(5).
          05 FLT-NAME-FLAGS                      PIC    X(50000).
          05 FLT-CAT-TERM                        PIC    X(83).
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

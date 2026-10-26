      *> TOP-RES: what Q-TOP leaves in WS-TOP-TAB and WS-TOP-N: the four funds with the largest PL, largest first.
      *> TOP-N = WS-TOP-N (0..4); TOP-NAME(k) = WS-TOP-NAME(k) (the name, or the code of a fund with no name; the first
      *> 40 BYTES: a cut inside a letter is the replacement character U+FFFD, as the desktop's label shows it, so the
      *> item holds up to 42 bytes); TOP-PL(k) = WS-TOP-PL(k).
       01 TOP-RES.
          05 TOP-STATUS                          PIC    X(8).
          05 TOP-MESSAGE                         PIC    X(160).
          05 TOP-N                               PIC    9.
          05 TOP-NAME                            PIC    X(42)    OCCURS 4.
          05 TOP-PL                              PIC    9(15)V99 OCCURS 4.

      *> RAD-RES: the data PAINT-RADAR asks the database for when two or three company names are ticked.
      *> RAD-NM = WS-NM: 2 or 3 when the request holds that many ticked names, else 0 and nothing else is filled
      *> (the radar then draws from FUND-CATS alone). RAD-NAME(k) = WS-RN(k): the ticked names in name order, the first 160 BYTES (a cut inside a letter is
      *> the replacement character U+FFFD, so the item holds up to 162 bytes).
      *> RAD-CNT(k,j) = WS-RV(k,j): the funds of company k in category slot j, under the whole filter.
       01 RAD-RES.
          05 RAD-STATUS                          PIC    X(8).
          05 RAD-MESSAGE                         PIC    X(160).
          05 RAD-NM                              PIC    9.
          05 RAD-NAME                            PIC    X(162)   OCCURS 3.
          05 RAD-RV                              OCCURS 3.
             10 RAD-CNT                          PIC    9(9)     OCCURS 10.

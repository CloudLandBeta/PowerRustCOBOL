      *> YRS-RES: what Q-YEARS leaves behind - the 12-year table behind the yearly charts, for all categories and
      *> for the three with most funds under the filter.
      *> YRS-TOP3(k)    WS-TOP3-IDX(k): the slot (1..10) of the k-th category by fund count, 0 when fewer than three
      *> YRS-MIN        WS-YR-MIN: the window starts here; it ends at the newest start year of the filter (2015 when none)
      *> YRS-NUM(i)     WS-YR-NUM(i): the year of row i (i = 1..12)
      *> YRS-A-*(i)     WS-YA-CNT / -PL / -COT(i): all categories, that year
      *> YRS-S-*(i,k)   WS-YS-CNT / -PL / -COT(i,k): the k-th leading category, that year
      *> YRS-OA-*       WS-OA-CNT / -PL / -COT: all categories, the funds that began before the window
      *> YRS-OS-*(k)    WS-OS-CNT / -PL / -COT(k): the k-th leading category, before the window
       01 YRS-RES.
          05 YRS-STATUS                          PIC    X(8).
          05 YRS-MESSAGE                         PIC    X(160).
          05 YRS-TOP3                            PIC    99       OCCURS 3.
          05 YRS-MIN                             PIC    9(4).
          05 YRS-YEAR                            OCCURS 12.
             10 YRS-NUM                          PIC    9(4).
             10 YRS-A-CNT                        PIC    9(9).
             10 YRS-A-PL                         PIC    9(15)V99.
             10 YRS-A-COT                        PIC    9(12).
             10 YRS-S                            OCCURS 3.
                15 YRS-S-CNT                     PIC    9(9).
                15 YRS-S-PL                      PIC    9(15)V99.
                15 YRS-S-COT                     PIC    9(12).
          05 YRS-OA-CNT                          PIC    9(9).
          05 YRS-OA-PL                           PIC    9(15)V99.
          05 YRS-OA-COT                          PIC    9(12).
          05 YRS-OS                              OCCURS 3.
             10 YRS-OS-CNT                       PIC    9(9).
             10 YRS-OS-PL                        PIC    9(15)V99.
             10 YRS-OS-COT                       PIC    9(12).

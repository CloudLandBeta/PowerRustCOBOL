      *> NAM-RES: what SM-REFRESH (with SM-TICKS) leaves for the list LST-SM.
      *> NAM-KEY      the typed text folded like the nome_key column (WS-TERM after NORM-TERM, trimmed); WS-PAT = % NAM-KEY %
      *> NAM-MATCHES  WS-SMN: for N the DISTINCT names that match (the whole count, listed or not); for C the categories kept
      *> NAM-LISTED   WS-SMK: the names listed (N: the first 150 by name; C: every category kept, in popularity order)
      *> NAM-NAME(k)  the k-th item of the list after "Todos" (the original name, not the folded key)
      *> NAM-TICK(k)  1 when that name is among the ticked ones of the request (the rows SM-TICKS ticks outside "Todos" mode)
       01 NAM-RES.
          05 NAM-STATUS                          PIC    X(8).
          05 NAM-MESSAGE                         PIC    X(160).
          05 NAM-KEY                             PIC    X(240).
          05 NAM-MATCHES                         PIC    9(9).
          05 NAM-LISTED                          PIC    9(4).
          05 NAM-ROW                             OCCURS 0 TO 150 DEPENDING ON NAM-LISTED.
             10 NAM-NAME                         PIC    X(210).
             10 NAM-TICK                         PIC    9.

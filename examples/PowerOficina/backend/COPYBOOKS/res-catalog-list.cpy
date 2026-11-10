      *> CLS-RES: the catalog rows asked for, at most 30, in code order. CLR-STATUS OK | INVALID.
       01 CLS-RES.
          05 CLR-STATUS                          PIC    X(10).
          05 CLR-MESSAGE                         PIC    X(80).
          05 CLR-COUNT                           PIC    9(3).
          05 CLR-ROW                             OCCURS 30.
             10 CLR-CODE                         PIC    X(12).
             10 CLR-NAME                         PIC    X(40).
             10 CLR-UNIT                         PIC    9(7)V99.
             10 CLR-HOURS                        PIC    9(3)V99.
             10 CLR-TAX-RATE                     PIC    9(2)V99.

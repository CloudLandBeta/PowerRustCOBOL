       01 RPT-RES.
          05 RPS-STATUS                          PIC    X(10).
          05 RPS-MESSAGE                         PIC    X(80).
          05 RPS-COUNT                           PIC    9(3).
          05 RPS-TOTAL-ROWS                      PIC    9(5).
          05 RPS-TOTAL                           PIC    9(11)V99.
          05 RPS-ROW                             OCCURS 40.
             10 RPL-ID                           PIC    9(8).
             10 RPL-REF                          PIC    X(8).
             10 RPL-STATUS                       PIC    X(10).
             10 RPL-AMOUNT                       PIC    9(9)V99.
             10 RPL-DATE                         PIC    9(8).
             10 RPL-TEXT                         PIC    X(60).

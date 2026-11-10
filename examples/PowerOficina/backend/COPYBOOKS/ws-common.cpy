      *> Working items shared by the Backend programs that number records, stamp the time and write the audit trail.
       01 WS-FS-SEQ                              PIC    XX.
       01 WS-FS-AUD                              PIC    XX.
       01 WS-SEQ-WANT                            PIC    X(12).
       01 WS-SEQ-GOT                             PIC    9(10).
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-TIME                       PIC    9(6).
          05 WS-CLOCK-REST                       PIC    X(7).
       01 WS-NOW-G.
          05 WS-NOW-DATE                         PIC    9(8).
          05 WS-NOW-TIME                         PIC    9(6).
       01 WS-NOW REDEFINES WS-NOW-G              PIC    9(14).
       01 WS-AU-OPERATOR                         PIC    X(30).
       01 WS-AU-ENTITY                           PIC    X(12).
       01 WS-AU-REF                              PIC    X(20).
       01 WS-AU-ACTION                           PIC    X(12).
       01 WS-AU-DETAIL                           PIC    X(80).

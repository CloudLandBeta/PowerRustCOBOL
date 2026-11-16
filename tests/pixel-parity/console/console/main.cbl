      *> DESKTOP-WASM-001 T24 console-parity fixture "console": what a
      *> desktop module asks the runner for at M1 (the console, the
      *> environment, the command line, the clock and its zone, the
      *> RANDOM seed and the exit code), on both paths under the pins.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYCONSOLE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NAME     PIC X(16).
       01 WS-AMOUNT   PIC 9(5)V99.
       01 WS-SHOW     PIC ZZ,ZZ9.99.
       01 WS-EMPTY    PIC X(8) VALUE "KEEP".
       01 WS-HOME     PIC X(24).
       01 WS-NONE     PIC X(6) VALUE "UNSET".
       01 WS-COUNT    PIC 99.
       01 WS-LINE     PIC X(40).
       01 WS-ARG      PIC X(12).
       01 WS-DATE6    PIC 9(6).
       01 WS-DAY5     PIC 9(5).
       01 WS-TIME     PIC 9(8).
       01 WS-DOW      PIC 9.
       01 WS-STAMP    PIC X(21).
       01 WS-R        PIC 9V9(6).
       01 WS-PICK     PIC 99.
       01 WS-I        PIC 9.
       PROCEDURE DIVISION.
           DISPLAY "parity console fixture"
           ACCEPT WS-NAME
           ACCEPT WS-AMOUNT
           MOVE WS-AMOUNT TO WS-SHOW
           DISPLAY "name [" WS-NAME "] amount [" WS-SHOW "]"
           ACCEPT WS-EMPTY
           DISPLAY "past the end [" WS-EMPTY "]"
           ACCEPT WS-HOME FROM ENVIRONMENT "PRC_PARITY_HOME"
           ACCEPT WS-NONE FROM ENVIRONMENT "PRC_PARITY_NOT_SET"
           DISPLAY "home [" WS-HOME "] none [" WS-NONE "]"
           ACCEPT WS-COUNT FROM ARGUMENT-NUMBER
           ACCEPT WS-LINE FROM COMMAND-LINE
           DISPLAY "args " WS-COUNT " [" WS-LINE "]"
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-COUNT
               DISPLAY WS-I UPON ARGUMENT-NUMBER
               ACCEPT WS-ARG FROM ARGUMENT-VALUE
               DISPLAY "  argument " WS-I " [" WS-ARG "]"
           END-PERFORM
           ACCEPT WS-DATE6 FROM DATE
           ACCEPT WS-DAY5 FROM DAY
           ACCEPT WS-TIME FROM TIME
           ACCEPT WS-DOW FROM DAY-OF-WEEK
           DISPLAY "date " WS-DATE6 " day " WS-DAY5 " time " WS-TIME
               " dow " WS-DOW
           MOVE FUNCTION CURRENT-DATE TO WS-STAMP
           DISPLAY "current-date " WS-STAMP
           DISPLAY "inline " FUNCTION CURRENT-DATE
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               COMPUTE WS-R = FUNCTION RANDOM
               COMPUTE WS-PICK = FUNCTION RANDOM * 100
               DISPLAY "random " WS-I " " WS-R " pick " WS-PICK
           END-PERFORM
           COMPUTE WS-R = FUNCTION RANDOM(42)
           DISPLAY "random(42) " WS-R
           MOVE 12 TO RETURN-CODE
           SUBTRACT 3 FROM RETURN-CODE
           DISPLAY "return-code " RETURN-CODE
           GOBACK.

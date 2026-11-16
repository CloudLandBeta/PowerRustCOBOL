      *> DESKTOP-WASM-001 T21 corpus fixture "console": the console, the
      *> environment, the arguments, the pinned clock, RANDOM and
      *> RETURN-CODE, which are what a desktop module's console imports
      *> and the test pins carry.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORPUSCONSOLE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-NAME     PIC X(20).
       01 WS-QTY      PIC 9(4).
       01 WS-HOME     PIC X(30).
       01 WS-NONE     PIC X(6) VALUE "KEEP".
       01 WS-COUNT    PIC 99.
       01 WS-LINE     PIC X(40).
       01 WS-ARG      PIC X(12).
       01 WS-DATE     PIC 9(6).
       01 WS-TIME     PIC 9(8).
       01 WS-DAY      PIC 9(5).
       01 WS-DOW      PIC 9.
       01 WS-STAMP    PIC X(21).
       01 WS-R        PIC 9(6).
       01 WS-I        PIC 9.
       PROCEDURE DIVISION.
           DISPLAY "corpus console fixture"
           ACCEPT WS-NAME
           ACCEPT WS-QTY
           DISPLAY "name [" WS-NAME "] qty " WS-QTY
           ACCEPT WS-HOME FROM ENVIRONMENT "PRC_CORPUS_HOME"
           DISPLAY "home [" WS-HOME "]"
           ACCEPT WS-NONE FROM ENVIRONMENT "PRC_CORPUS_NOT_SET"
           DISPLAY "none [" WS-NONE "]"
           ACCEPT WS-COUNT FROM ARGUMENT-NUMBER
           ACCEPT WS-LINE FROM COMMAND-LINE
           ACCEPT WS-ARG FROM ARGUMENT-VALUE
           DISPLAY "args " WS-COUNT " [" WS-LINE "] first [" WS-ARG "]"
           ACCEPT WS-DATE FROM DATE
           ACCEPT WS-TIME FROM TIME
           ACCEPT WS-DAY FROM DAY
           ACCEPT WS-DOW FROM DAY-OF-WEEK
           DISPLAY "date " WS-DATE " time " WS-TIME " day " WS-DAY
               " dow " WS-DOW
           MOVE FUNCTION CURRENT-DATE TO WS-STAMP
           DISPLAY "stamp " WS-STAMP(1:16)
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 3
               COMPUTE WS-R = FUNCTION RANDOM * 1000000
               DISPLAY "random " WS-I " " WS-R
           END-PERFORM
           MOVE 7 TO RETURN-CODE
           STOP RUN.

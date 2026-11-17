      *> Console-parity fixture "switches" (WASM-PARITY #55): external
      *> switches set by COBOL_SWITCHES before the first statement, read
      *> by their ON and OFF conditions, on both paths.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYSWITCHES.
       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           SWITCH-1 IS SW-ONE
               ON STATUS IS ONE-ON
               OFF STATUS IS ONE-OFF
           SWITCH-2 IS SW-TWO
               ON STATUS IS TWO-ON
           SWITCH-3 IS SW-THREE
               OFF STATUS IS THREE-OFF
           SWITCH-4 IS SW-FOUR
               ON STATUS IS FOUR-ON.
       PROCEDURE DIVISION.
           IF ONE-ON
               DISPLAY "SWITCH-1 ON"
           END-IF
           IF ONE-OFF
               DISPLAY "SWITCH-1 OFF"
           END-IF
           IF TWO-ON
               DISPLAY "SWITCH-2 ON"
           ELSE
               DISPLAY "SWITCH-2 OFF"
           END-IF
           IF THREE-OFF
               DISPLAY "SWITCH-3 OFF"
           ELSE
               DISPLAY "SWITCH-3 ON"
           END-IF
           IF FOUR-ON
               DISPLAY "SWITCH-4 ON"
           ELSE
               DISPLAY "SWITCH-4 OFF"
           END-IF
           STOP RUN.

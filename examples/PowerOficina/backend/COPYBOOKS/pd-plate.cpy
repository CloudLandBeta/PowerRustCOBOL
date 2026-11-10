      *> NORMALIZE-PLATE: the licence plate as it is stored: capital letters, no hyphen and no space.
      *> In:  WS-PLATE-IN, as typed, for example "abc-1d23".
      *> Out: WS-PLATE-OUT, left-justified, and WS-PLATE-OK = 1 when it has exactly 7 letters or digits.
       NORMALIZE-PLATE.
           MOVE    SPACES                        TO    WS-PLATE-OUT
           MOVE    0                             TO    WS-PLATE-LEN
           MOVE    1                             TO    WS-PLATE-OK
           INSPECT WS-PLATE-IN CONVERTING "abcdefghijklmnopqrstuvwxyz"
                                       TO "ABCDEFGHIJKLMNOPQRSTUVWXYZ"

           PERFORM VARYING WS-PLATE-POS FROM 1 BY 1 UNTIL WS-PLATE-POS > 8
               MOVE    WS-PLATE-IN (WS-PLATE-POS:1)  TO    WS-PLATE-CHAR
               IF    WS-PLATE-CHAR NOT = SPACE AND WS-PLATE-CHAR NOT = "-"
                   IF    WS-PLATE-CHAR IS NUMERIC OR WS-PLATE-CHAR IS ALPHABETIC
                       IF    WS-PLATE-LEN < 7
                           ADD     1             TO    WS-PLATE-LEN
                           MOVE    WS-PLATE-CHAR TO    WS-PLATE-OUT (WS-PLATE-LEN:1)
                       ELSE
                           MOVE    0             TO    WS-PLATE-OK
                       END-IF
                   ELSE
                       MOVE    0                 TO    WS-PLATE-OK
                   END-IF
               END-IF
           END-PERFORM

           IF    WS-PLATE-LEN NOT = 7
               MOVE    0                         TO    WS-PLATE-OK
           END-IF.

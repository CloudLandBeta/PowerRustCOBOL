      *> A program that COPYs pd-token also COPYs ws-partiql and pq-tokens.
      *> RESOLVE-TOKEN: finds the token a customer holds and says whether it can be used (R37).
      *> In:  WS-REQ-TOKEN (the token of the link, or blank), WS-REQ-ID (the quote number, used when the token is blank),
      *>      WS-REQ-CODE (the short code typed). WS-NOW must be current (PERFORM NOW-STAMP first). TOKEN-FILE is open.
      *> Out: WS-TK-STATUS = OK | NOTFOUND | BADCODE | USED | REPLACED | EXPIRED; with a found token the TOKEN-RECORD
      *>      is the current record. A token is used once (USED) and a newer sending replaces the older tokens (REPLACED).
      *>      TOKEN-RECORD then holds that token, and UPDATE-TOKEN-ROW writes it back by its key, TOK-TOKEN.
       RESOLVE-TOKEN.
           MOVE    "NOTFOUND"                    TO    WS-TK-STATUS
           IF    WS-REQ-TOKEN NOT = SPACES
               MOVE    WS-REQ-TOKEN              TO    TOK-TOKEN
               PERFORM GET-TOKEN-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "FOUND"               TO    WS-TK-STATUS
               END-IF
           ELSE
               PERFORM FIND-TOKEN-BY-QUOTE
           END-IF

           IF    WS-TK-STATUS = "FOUND"
               EVALUATE TRUE
                   WHEN TOK-CODE NOT = WS-REQ-CODE
                       MOVE    "BADCODE"         TO    WS-TK-STATUS
                   WHEN TOK-USED = "S"
                       MOVE    "USED"            TO    WS-TK-STATUS
                   WHEN TOK-USED = "I"
                       MOVE    "REPLACED"        TO    WS-TK-STATUS
                   WHEN TOK-EXPIRES < WS-NOW
                       MOVE    "EXPIRED"         TO    WS-TK-STATUS
                   WHEN OTHER
                       MOVE    "OK"              TO    WS-TK-STATUS
               END-EVALUATE
           END-IF.

      *> FIND-TOKEN-BY-QUOTE: the unused token of quote WS-REQ-ID; if none is unused, the last one it had.
       FIND-TOKEN-BY-QUOTE.
           MOVE    SPACES                        TO    WS-TK-LAST-KEY
           MOVE    "N"                           TO    WS-TK-EOF
           MOVE    WS-REQ-ID                     TO    TOK-QUO-ID
           PERFORM SELECT-TOKENS-OF-QUOTE
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-TK-EOF
           END-IF

           PERFORM UNTIL WS-TK-EOF = "Y"
               PERFORM FETCH-TOKEN-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-TK-EOF
               ELSE
                   IF    TOK-QUO-ID NOT = WS-REQ-ID
                       MOVE    "Y"               TO    WS-TK-EOF
                   ELSE
                       MOVE    TOK-TOKEN         TO    WS-TK-LAST-KEY
                       IF    TOK-USED = SPACE
                           MOVE    "Y"           TO    WS-TK-EOF
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           IF    WS-TK-LAST-KEY NOT = SPACES
               MOVE    WS-TK-LAST-KEY            TO    TOK-TOKEN
               PERFORM GET-TOKEN-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "FOUND"               TO    WS-TK-STATUS
               END-IF
           END-IF.

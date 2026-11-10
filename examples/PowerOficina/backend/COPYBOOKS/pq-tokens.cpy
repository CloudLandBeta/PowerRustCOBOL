      *> pq-tokens: the PartiQL statements on TOKEN-FILE (tokens.FD) and the reading of its rows into TOKEN-RECORD.
      *> The program declares the file, COPYs ws-partiql, and keeps the file open while it uses these paragraphs. A
      *> statement on TOKEN-FILE replaces the rows the one before it answered.

      *> GET-TOKEN-ROW: the token whose key is in TOK-TOKEN, read into TOKEN-RECORD.
      *> In:  TOK-TOKEN.
      *> Out: WS-PQ-STATUS = 00 and TOKEN-RECORD filled, or 23 when there is no such token (the record is left as it
      *>      was).
       GET-TOKEN-ROW.
           COBOL::"PARTIQL" ( "TOKEN-FILE" "SELECT * FROM TOKEN-FILE WHERE TOK-TOKEN = ?"
                   WS-PQ-COUNT WS-PQ-STATUS TOK-TOKEN )
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-TOKEN-ROW
               IF    WS-PQ-STATUS = "10"
                   MOVE    "23"                  TO    WS-PQ-STATUS
               END-IF
           END-IF.

      *> SELECT-TOKENS-OF-QUOTE: the tokens of quote TOK-QUO-ID, in the order they were made;
      *> FETCH-TOKEN-ROW reads them.
       SELECT-TOKENS-OF-QUOTE.
           COBOL::"PARTIQL" ( "TOKEN-FILE" "SELECT * FROM TOKEN-FILE.TOK-QUO-ID WHERE TOK-QUO-ID = ?"
                   WS-PQ-COUNT WS-PQ-STATUS TOK-QUO-ID ).

      *> FETCH-TOKEN-ROW: the next row the last statement on TOKEN-FILE answered, read into TOKEN-RECORD.
      *> Out: WS-PQ-STATUS = 00 and TOKEN-RECORD filled, or 10 past the last row (the record is left as it was).
      *> The values come back in the order of the record, separated by X"09".
       FETCH-TOKEN-ROW.
           COBOL::"PARTIQL-FETCH" ( "TOKEN-FILE" WS-PQ-ROW WS-PQ-STATUS )
           IF    WS-PQ-STATUS = "00"
               UNSTRING WS-PQ-ROW DELIMITED BY X"09"
                   INTO TOK-TOKEN TOK-QUO-ID TOK-REV TOK-CODE TOK-EXPIRES TOK-USED
               END-UNSTRING
           END-IF.

      *> INSERT-TOKEN-ROW: writes TOKEN-RECORD as a new token, every field as it stands.
      *> Out: WS-PQ-STATUS = 00 when it was written, 22 when a token with that key exists.
       INSERT-TOKEN-ROW.
           COBOL::"PARTIQL" ( "TOKEN-FILE"
                   "INSERT INTO TOKEN-FILE VALUE {'TOK-TOKEN': ?, 'TOK-QUO-ID': ?, 'TOK-REV': ?, 'TOK-CODE': ?, "
                 & "'TOK-EXPIRES': ?, 'TOK-USED': ?}"
                   WS-PQ-COUNT WS-PQ-STATUS TOK-TOKEN TOK-QUO-ID TOK-REV TOK-CODE TOK-EXPIRES TOK-USED ).

      *> UPDATE-TOKEN-ROW: rewrites the token whose key is in TOK-TOKEN with every other field of TOKEN-RECORD.
      *> Out: WS-PQ-STATUS = 00 when it was rewritten, 23 when there is no such token.
       UPDATE-TOKEN-ROW.
           COBOL::"PARTIQL" ( "TOKEN-FILE"
                   "UPDATE TOKEN-FILE SET TOK-QUO-ID = ? SET TOK-REV = ? SET TOK-CODE = ? SET TOK-EXPIRES = ? "
                 & "SET TOK-USED = ? WHERE TOK-TOKEN = ?"
                   WS-PQ-COUNT WS-PQ-STATUS TOK-QUO-ID TOK-REV TOK-CODE TOK-EXPIRES TOK-USED TOK-TOKEN ).

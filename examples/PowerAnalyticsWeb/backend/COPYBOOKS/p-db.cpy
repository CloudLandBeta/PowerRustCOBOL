      *> Opens the database of PowerAnalytics: connects to the project's SQL connection FUNDOS (data/fundos.db,
      *> created on first use) and, when it holds no table fundos yet, builds it from data/fundos.sql, one
      *> statement per line, inside ONE unit of work (the file is the desktop's, byte for byte). The desktop
      *> does the same in INIT-DB, into an in-memory database that every window start rebuilds; here the
      *> database is a file on the Backend, built once and read by every call afterwards.
      *> Two calls can be the first at the same time: the one that loses finds the table there when its own
      *> load fails ("already exists" / "locked"), rolls back and reports RACED, which is success.
      *> Uses SH-* and OPN-OUT of ws-open, the file of fc-script / fd-script and SQLCA; sets OPN-OUT.
       OPN-DB.
           MOVE    "OK"                          TO    OPN-STATUS
           MOVE    SPACES                        TO    OPN-MESSAGE
           MOVE    "READY"                       TO    OPN-LOAD
           MOVE    0                             TO    OPN-LINES
           MOVE    SPACES                        TO    OPN-NAME-KEY
           MOVE    SPACES                        TO    OPN-CAT-KEY
           EXEC SQL CONNECT TO 'FUNDOS' END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
               EXIT PARAGRAPH
           END-IF
           PERFORM OPN-COUNT-TABLE
           IF OPN-STATUS = "OK" AND SH-HAS = 0
               PERFORM OPN-LOAD-SCRIPT
           END-IF.

      *> Whether the table fundos is there: SH-HAS = 1 or 0.
       OPN-COUNT-TABLE.
           MOVE    0                             TO    SH-HAS
           EXEC SQL
               SELECT COUNT(*) INTO :SH-HAS FROM sqlite_master
                WHERE type = 'table' AND name = 'fundos'
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.

      *> Runs data/fundos.sql, a statement a line, and commits it as one unit; nothing is left half built.
       OPN-LOAD-SCRIPT.
           OPEN INPUT SQL-FILE
           IF SH-FS NOT = "00"
               MOVE    "NOFILE"                  TO    OPN-STATUS
               MOVE    "no database yet, and data/fundos.sql cannot be opened (start the Backend in the project folder)"
                                                 TO    OPN-MESSAGE
               EXIT PARAGRAPH
           END-IF
           MOVE    0                             TO    SH-FAILED
           PERFORM UNTIL 1 = 2
               READ SQL-FILE
                   AT END EXIT PERFORM
               END-READ
               ADD     1                         TO    OPN-LINES
               EXEC SQL EXECUTE IMMEDIATE :SQL-REC END-EXEC
               IF SQLSTATE(1:2) > "02"
                   MOVE    1                     TO    SH-FAILED
                   MOVE    SQLSTATE              TO    SH-STATE
                   MOVE    SQLCA-MESSAGE         TO    SH-WHY
                   EXIT PERFORM
               END-IF
           END-PERFORM
           CLOSE SQL-FILE
           IF SH-FAILED = 0
               EXEC SQL COMMIT END-EXEC
               MOVE    "LOADED"                  TO    OPN-LOAD
               EXIT PARAGRAPH
           END-IF

      *> The load failed. If another call built the table meanwhile, that is the answer; else it is an error.
           EXEC SQL ROLLBACK END-EXEC
           MOVE    0                             TO    OPN-LINES
           PERFORM OPN-COUNT-TABLE
           IF OPN-STATUS = "OK"
               IF SH-HAS > 0
                   MOVE    "RACED"               TO    OPN-LOAD
               ELSE
                   MOVE    "SQLERR"              TO    OPN-STATUS
                   MOVE    SPACES                TO    OPN-MESSAGE
                   STRING  SH-STATE " " SH-WHY::Trim() DELIMITED BY SIZE INTO OPN-MESSAGE
               END-IF
           END-IF.

      *> The last statement failed: OPN-STATUS = SQLERR and OPN-MESSAGE = its SQLSTATE and the database's words.
       OPN-SQL-FAILED.
           MOVE    "SQLERR"                      TO    OPN-STATUS
           MOVE    SPACES                        TO    OPN-MESSAGE
           STRING  SQLSTATE " " SQLCA-MESSAGE::Trim() DELIMITED BY SIZE INTO OPN-MESSAGE.

      *> End of the call: the temporary tables go with the connection, and nothing was written to fundos.db.
       OPN-CLOSE.
           EXEC SQL DISCONNECT CURRENT END-EXEC.

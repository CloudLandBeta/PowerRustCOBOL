       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-INIT.
      *> INIT-DB and LOAD-DIMS of the desktop, as one call made once, when the page starts:
      *>   INIT-DB    the database is built from data/fundos.sql if this is the first call ever (the desktop builds
      *>              an in-memory copy at every window start; here it is a file, built once, read by every call).
      *>   LOAD-DIMS  the universe, the reference date, the 10 category slots and the level-2 / level-3 lists. The
      *>              statements are the desktop's, word for word.
      *> The helper tables pick / vis / chk of INIT-DB are not created: the picks live in the form (FLT-REQ).
      *> Exported, idempotent: after the first call it only reads.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-script".

       DATA DIVISION.
       FILE SECTION.
           COPY "backend/COPYBOOKS/fd-script".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-open".
       01 WS-NAME                                PIC    X(60).
       01 WS-K                                   PIC    99.
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/res-init".

       PROCEDURE DIVISION RETURNING INI-RES.
       MAIN-PARA.
           INITIALIZE INI-RES
           MOVE    "OK"                          TO    INI-STATUS
           PERFORM OPN-DB
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    INI-STATUS
               MOVE    OPN-MESSAGE               TO    INI-MESSAGE
               GOBACK
           END-IF
           MOVE    OPN-LOAD                      TO    INI-LOAD
           MOVE    OPN-LINES                     TO    INI-LINES

      *> The universe, and the date of the newest data: the header caption is built from them.
           EXEC SQL SELECT COUNT(*) INTO :INI-UNIV FROM fundos END-EXEC
           EXEC SQL
               SELECT COALESCE(strftime('%d/%m/%Y', MAX(data_ref)),'') INTO :INI-REFDATE FROM fundos
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM INI-FAIL
               GOBACK
           END-IF

      *> The 10 most populated level-1 categories, most funds first, fill the fixed slot table. Every later
      *> query matches its counts to these slots by name, so a category keeps its slot under any filter.
           EXEC SQL
               DECLARE C-CAT CURSOR FOR
                   SELECT nivel1 FROM fundos WHERE nivel1 IS NOT NULL GROUP BY nivel1 ORDER BY COUNT(*) DESC
           END-EXEC
           EXEC SQL OPEN C-CAT END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-CAT INTO :WS-NAME END-EXEC
               IF SQLSTATE = "00000" AND INI-CAT-N < 10
                   ADD     1                     TO    INI-CAT-N
                   MOVE    WS-NAME               TO    INI-CAT-NAME (INI-CAT-N)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-CAT END-EXEC

      *> Every distinct level-2 name, most funds first (the items of LST-N2). INI-N2-TOTAL counts them all; the
      *> table keeps 64, and TRUNC says so when there are more.
           EXEC SQL
               DECLARE C-N2 CURSOR FOR
                   SELECT nivel2 FROM fundos WHERE nivel2 IS NOT NULL GROUP BY nivel2 ORDER BY COUNT(*) DESC
           END-EXEC
           EXEC SQL OPEN C-N2 END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-N2 INTO :WS-NAME END-EXEC
               IF SQLSTATE = "00000"
                   ADD     1                     TO    INI-N2-TOTAL
                   IF INI-N2-N < 64
                       ADD     1                 TO    INI-N2-N
                       MOVE    WS-NAME           TO    INI-N2-NAME (INI-N2-N)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-N2 END-EXEC

           EXEC SQL
               DECLARE C-N3 CURSOR FOR
                   SELECT nivel3 FROM fundos WHERE nivel3 IS NOT NULL GROUP BY nivel3 ORDER BY COUNT(*) DESC
           END-EXEC
           EXEC SQL OPEN C-N3 END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-N3 INTO :WS-NAME END-EXEC
               IF SQLSTATE = "00000"
                   ADD     1                     TO    INI-N3-TOTAL
                   IF INI-N3-N < 64
                       ADD     1                 TO    INI-N3-N
                       MOVE    WS-NAME           TO    INI-N3-NAME (INI-N3-N)
                   END-IF
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-N3 END-EXEC

           IF INI-N2-TOTAL > INI-N2-N OR INI-N3-TOTAL > INI-N3-N
               MOVE    "TRUNC"                   TO    INI-STATUS
               MOVE    "a level list is longer than the 64 names the answer holds (see INI-N2-TOTAL / INI-N3-TOTAL)"
                                                 TO    INI-MESSAGE
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       INI-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    INI-STATUS
           MOVE    OPN-MESSAGE                   TO    INI-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".

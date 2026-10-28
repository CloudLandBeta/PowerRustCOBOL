       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-NAMES.
      *> SM-REFRESH of the desktop (with what SM-TICKS reads): the list of the search box that is open.
      *>   N  Nome comercial: the first 150 distinct fund names, in name order, whose folded key contains the
      *>      folded text, and the count of ALL the names that match.
      *>   C  Categoria: the categories of the 10 slots whose folded name contains the folded text, in
      *>      popularity order (an empty text keeps all 10).
      *> The statements are the desktop's: it fills the table vis with them and reads it back; here the answer
      *> carries the names, and the tick of each one (the row is among the names the request says are ticked).
      *> Exported, idempotent: it only reads.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-script".

       DATA DIVISION.
       FILE SECTION.
           COPY "backend/COPYBOOKS/fd-script".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-open".
       01 WS-NAME                                PIC    X(210).
       01 WS-TICK                                PIC    9.
       01 WS-BYTES                               PIC    9(9).
       01 WS-STOP                                PIC    9.
      *> WS-PAT of SM-REFRESH: "%" + the folded text + "%" (just "%%" for a blank text, which matches as "%" does).
       01 WS-PAT                                 PIC    X(250).
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/req-names".
           COPY "backend/COPYBOOKS/res-names".

       PROCEDURE DIVISION USING FLT-REQ NAM-REQ RETURNING NAM-RES.
       MAIN-PARA.
           INITIALIZE NAM-RES
           MOVE    "OK"                          TO    NAM-STATUS
           IF NAM-BOX NOT = "N" AND NAM-BOX NOT = "C"
               MOVE    "BADREQ"                  TO    NAM-STATUS
               MOVE    "NAM-BOX must be N (Nome comercial) or C (Categoria)"
                                                 TO    NAM-MESSAGE
               GOBACK
           END-IF
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    NAM-STATUS
               MOVE    OPN-MESSAGE               TO    NAM-MESSAGE
               GOBACK
           END-IF
           IF NAM-BOX = "N"
               MOVE    OPN-NAME-KEY              TO    NAM-KEY
               PERFORM NAM-READ-NAMES
           ELSE
               MOVE    OPN-CAT-KEY               TO    NAM-KEY
               PERFORM NAM-READ-CATS
           END-IF
           IF NAM-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

      *> The list shows at most 150 unique names (LIMIT), while NAM-MATCHES counts every match for the footer,
      *> so the total can exceed what is listed. A fund with no name is never listed. LIKE has no ESCAPE: a
      *> typed % or _ is a wildcard, as on the desktop.
       NAM-READ-NAMES.
           MOVE    SPACES                        TO    WS-PAT
           STRING  "%" OPN-NAME-KEY::Trim() "%" DELIMITED BY SIZE INTO WS-PAT
           EXEC SQL
               SELECT COUNT(DISTINCT nome) INTO :NAM-MATCHES FROM fundos
                WHERE nome IS NOT NULL AND nome <> '' AND nome_key LIKE :WS-PAT
           END-EXEC
           EXEC SQL
               DECLARE C-NAMES CURSOR FOR
                   SELECT nome, EXISTS (SELECT 1 FROM pick WHERE fld = 'N' AND name = fundos.nome)
                     FROM fundos
                    WHERE nome IS NOT NULL AND nome <> '' AND nome_key LIKE :WS-PAT
                    GROUP BY nome ORDER BY nome LIMIT 150
           END-EXEC
           EXEC SQL OPEN C-NAMES END-EXEC
           MOVE    0                             TO    WS-STOP
           MOVE    0                             TO    WS-BYTES
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-NAMES INTO :WS-NAME, :WS-TICK END-EXEC
               PERFORM NAM-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-NAMES END-EXEC
           PERFORM NAM-DONE.

      *> Only 10 categories, so the slots decide: OPN-FILTER kept in vis 'C' the ones whose folded name contains
      *> the folded text (instr), in slot order; the list keeps that popularity order.
       NAM-READ-CATS.
           EXEC SQL
               DECLARE C-CATS CURSOR FOR
                   SELECT v.name, EXISTS (SELECT 1 FROM pick x WHERE x.fld = 'C' AND x.name = v.name)
                     FROM vis v WHERE v.fld = 'C' ORDER BY v.rowid
           END-EXEC
           EXEC SQL OPEN C-CATS END-EXEC
           MOVE    0                             TO    WS-STOP
           MOVE    0                             TO    WS-BYTES
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-CATS INTO :WS-NAME, :WS-TICK END-EXEC
               PERFORM NAM-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-CATS END-EXEC
           MOVE    NAM-LISTED                    TO    NAM-MATCHES
           PERFORM NAM-DONE.

      *> A fetched name joins the answer. The answer stays under 64 KiB for 150 names of the longest the data holds
      *> (205 bytes: about 35,000 bytes); if a data file ever made it pass 60,000, the list stops and says so.
       NAM-TAKE.
           IF SQLSTATE(1:2) < "02"
               IF SQLSTATE = "01004"
                   MOVE    "TRUNC"               TO    NAM-STATUS
                   MOVE    "a name is longer than the 210 bytes NAM-NAME holds"
                                                 TO    NAM-MESSAGE
               END-IF
               ADD     FUNCTION LENGTH(WS-NAME::Trim()) TO WS-BYTES
               ADD     40                        TO    WS-BYTES
               IF WS-BYTES > 60000
                   MOVE    "TRUNC"               TO    NAM-STATUS
                   MOVE    "the list reached the 64 KiB a call may answer; type more of the name"
                                                 TO    NAM-MESSAGE
                   MOVE    1                     TO    WS-STOP
               ELSE
                   ADD     1                     TO    NAM-LISTED
                   MOVE    WS-NAME               TO    NAM-NAME (NAM-LISTED)
                   MOVE    WS-TICK               TO    NAM-TICK (NAM-LISTED)
               END-IF
           END-IF.

       NAM-DONE.
           IF SQLSTATE(1:2) > "02"
               PERFORM NAM-FAIL
           END-IF.

       NAM-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    NAM-STATUS
           MOVE    OPN-MESSAGE                   TO    NAM-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

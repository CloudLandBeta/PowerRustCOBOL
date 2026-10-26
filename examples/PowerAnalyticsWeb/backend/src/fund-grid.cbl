       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-GRID.
      *> Q-GRID of the desktop: rows of the grid GRD-FUNDOS - the funds that pass the filter, in the order of
      *> CMB-ORDER, from GRD-OFFSET on. The 16 columns are the desktop's, one TAB-joined text per fund. The module
      *> takes 64 KiB per call, so a page of 300 rows is read as two windows of 150: the page, its count and
      *> the clamping of the page number stay in the form (Q-GRID and PAINT-GRID-INFO), with WS-T-CNT of FUND-TOTALS.
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
       01 WS-SEL                                 PIC    X(40).
       01 WS-KEY                                 PIC    9.
       01 WS-ROW                                 PIC    X(400).
       01 WS-RLEN                                PIC    9(9).
       01 WS-BYTES                               PIC    9(9).
       01 WS-STOP                                PIC    9.
       01 WS-LIM                                 PIC    9(4).
       01 WS-OFFS                                PIC    9(9).
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/req-grid".
           COPY "backend/COPYBOOKS/res-grid".

       PROCEDURE DIVISION USING FLT-REQ GRD-REQ RETURNING GRD-RES.
       MAIN-PARA.
           INITIALIZE GRD-RES
           MOVE    "OK"                          TO    GRD-STATUS
           MOVE    "N"                           TO    GRD-MORE
           IF GRD-LIMIT < 1 OR GRD-LIMIT > 150
               MOVE    "BADREQ"                  TO    GRD-STATUS
               MOVE    "GRD-LIMIT must be 1 to 150 rows per call; read a page of 300 rows as two calls"
                                                 TO    GRD-MESSAGE
               GOBACK
           END-IF
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    GRD-STATUS
               MOVE    OPN-MESSAGE               TO    GRD-MESSAGE
               GOBACK
           END-IF

      *> Each CMB-ORDER item is recognised by its first words. Every key but the name sorts IS NULL first, which
      *> sends funds with no value to the end: SQLite puts NULLs first when ascending (as in oldest first).
           MOVE    GRD-ORDER                     TO    WS-SEL
           EVALUATE TRUE
               WHEN WS-SEL(1:6) = "Maior "
                   MOVE    1                     TO    WS-KEY
               WHEN WS-SEL(1:5) = "Mais "
                   IF WS-SEL(1:9) = "Mais coti"
                       MOVE    2                 TO    WS-KEY
                   ELSE
                       IF WS-SEL(1:9) = "Mais rece"
                           MOVE    3             TO    WS-KEY
                       ELSE
                           MOVE    4             TO    WS-KEY
                       END-IF
                   END-IF
               WHEN OTHER
                   MOVE    5                     TO    WS-KEY
           END-EVALUATE
           MOVE    GRD-LIMIT                     TO    WS-LIM
           MOVE    0                             TO    WS-STOP
           MOVE    0                             TO    WS-BYTES
           MOVE    GRD-OFFSET                    TO    WS-OFFS

      *> The 16 columns follow the column order of GRD-FUNDOS, PL scaled to R$ million as its header says. Each
      *> fund is one text, cells joined by TAB, as the desktop's Fetch() joins them; NULL is empty. rowlen is
      *> what the row costs in the JSON answer (its bytes, quotes, one byte more for each TAB, quote and backslash).
           EXEC SQL
               CREATE TEMP VIEW fg AS
                   SELECT id, pl, cotistas, inicio, nome_key,
                          COALESCE(nome,'') || char(9) || codigo || char(9) || COALESCE(estrutura,'') || char(9)
                          || COALESCE(nivel1,'') || char(9) || COALESCE(nivel2,'') || char(9)
                          || COALESCE(nivel3,'') || char(9) || COALESCE(strftime('%d/%m/%Y', inicio),'') || char(9)
                          || COALESCE(printf('%.2f', pl/1000000.0),'') || char(9) || COALESCE(cotistas,'') || char(9)
                          || COALESCE(printf('%.4f', cota),'') || char(9) || COALESCE(foco,'') || char(9)
                          || COALESCE(cnpj_classe,'') || char(9) || COALESCE(cnpj_fundo,'') || char(9)
                          || COALESCE(status,'') || char(9) || COALESCE(subclasses,'') || char(9)
                          || COALESCE(strftime('%d/%m/%Y', data_ref),'') AS rowtext
                     FROM fv
           END-EXEC
           EXEC SQL
               CREATE TEMP VIEW fr AS
                   SELECT id, pl, cotistas, inicio, nome_key, rowtext,
                          length(CAST(rowtext AS BLOB)) + 18
                          + length(rowtext) - length(replace(rowtext, '"', ''))
                          + length(rowtext) - length(replace(rowtext, char(92), '')) AS rowlen
                     FROM fg
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM GRD-FAIL
               GOBACK
           END-IF

           EVALUATE WS-KEY
               WHEN 1
                   PERFORM GRD-READ-1
               WHEN 2
                   PERFORM GRD-READ-2
               WHEN 3
                   PERFORM GRD-READ-3
               WHEN 4
                   PERFORM GRD-READ-4
               WHEN OTHER
                   PERFORM GRD-READ-5
           END-EVALUATE
           IF GRD-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

      *> "Maior PL": pl IS NULL, pl DESC.
       GRD-READ-1.
           EXEC SQL
               DECLARE C-G1 CURSOR FOR
                   SELECT rowtext, rowlen FROM fr ORDER BY pl IS NULL, pl DESC, id LIMIT :WS-LIM OFFSET :WS-OFFS
           END-EXEC
           EXEC SQL OPEN C-G1 END-EXEC
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-G1 INTO :WS-ROW, :WS-RLEN END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-G1 END-EXEC
           PERFORM GRD-DONE.

      *> "Mais cotistas": cotistas IS NULL, cotistas DESC.
       GRD-READ-2.
           EXEC SQL
               DECLARE C-G2 CURSOR FOR
                   SELECT rowtext, rowlen FROM fr ORDER BY cotistas IS NULL, cotistas DESC, id LIMIT :WS-LIM OFFSET :WS-OFFS
           END-EXEC
           EXEC SQL OPEN C-G2 END-EXEC
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-G2 INTO :WS-ROW, :WS-RLEN END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-G2 END-EXEC
           PERFORM GRD-DONE.

      *> "Mais recentes": inicio IS NULL, inicio DESC.
       GRD-READ-3.
           EXEC SQL
               DECLARE C-G3 CURSOR FOR
                   SELECT rowtext, rowlen FROM fr ORDER BY inicio IS NULL, inicio DESC, id LIMIT :WS-LIM OFFSET :WS-OFFS
           END-EXEC
           EXEC SQL OPEN C-G3 END-EXEC
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-G3 INTO :WS-ROW, :WS-RLEN END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-G3 END-EXEC
           PERFORM GRD-DONE.

      *> "Mais antigos": inicio IS NULL, inicio ASC.
       GRD-READ-4.
           EXEC SQL
               DECLARE C-G4 CURSOR FOR
                   SELECT rowtext, rowlen FROM fr ORDER BY inicio IS NULL, inicio ASC, id LIMIT :WS-LIM OFFSET :WS-OFFS
           END-EXEC
           EXEC SQL OPEN C-G4 END-EXEC
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-G4 INTO :WS-ROW, :WS-RLEN END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-G4 END-EXEC
           PERFORM GRD-DONE.

      *> "Nome (A-Z)" and anything else: nome_key ASC (funds with no name come first).
       GRD-READ-5.
           EXEC SQL
               DECLARE C-G5 CURSOR FOR
                   SELECT rowtext, rowlen FROM fr ORDER BY nome_key ASC, id LIMIT :WS-LIM OFFSET :WS-OFFS
           END-EXEC
           EXEC SQL OPEN C-G5 END-EXEC
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               EXEC SQL FETCH C-G5 INTO :WS-ROW, :WS-RLEN END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-G5 END-EXEC
           PERFORM GRD-DONE.

      *> A fetched row joins the answer unless the answer would pass its byte budget (60,000 of the 64 KiB), in
      *> which case GRD-MORE = Y and the reading stops. SQLSTATE 01004 is a row cut to the 400 bytes of GRD-ROW.
       GRD-TAKE.
           IF SQLSTATE(1:2) < "02"
               IF SQLSTATE = "01004"
                   MOVE    "TRUNC"               TO    GRD-STATUS
                   MOVE    "a row is longer than the 400 bytes GRD-ROW holds"
                                                 TO    GRD-MESSAGE
               END-IF
               ADD     WS-RLEN                   TO    WS-BYTES
               IF WS-BYTES > 60000
                   MOVE    "Y"                   TO    GRD-MORE
                   MOVE    1                     TO    WS-STOP
               ELSE
                   ADD     1                     TO    GRD-N
                   MOVE    WS-ROW                TO    GRD-ROW (GRD-N)
               END-IF
           END-IF.

       GRD-DONE.
           IF SQLSTATE(1:2) > "02"
               PERFORM GRD-FAIL
           END-IF.

       GRD-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    GRD-STATUS
           MOVE    OPN-MESSAGE                   TO    GRD-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

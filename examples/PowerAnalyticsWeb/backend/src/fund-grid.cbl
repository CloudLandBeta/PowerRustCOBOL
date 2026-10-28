       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-GRID.
      *> Q-GRID of the desktop: rows of the grid GRD-FUNDOS - the funds that pass the filter, in the order of
      *> CMB-ORDER, from GRD-OFFSET on. The 16 columns are the desktop's, one TAB-joined text per fund. The module
      *> takes 64 KiB per call, so a page of 300 rows is read as two windows of 150: the page, its count and
      *> the clamping of the page number stay in the form (Q-GRID and PAINT-GRID-INFO), with WS-T-CNT of FUND-TOTALS.
      *> The statement is the desktop's text: "FROM fundos " + OPN-WHERE (values bound), " ORDER BY " + the order of
      *> Q-GRID and nothing more, LIMIT and OFFSET bound; so funds that tie on the order come back in the order the
      *> desktop's SQLite gives them for the same filter. (The 16 columns are joined in the statement; what a column
      *> computes does not change the plan.)
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
       01 WS-ORD                                 PIC    X(60).
       01 WS-QUOTES                              PIC    9(4).
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
                   MOVE    "pl IS NULL, pl DESC" TO    WS-ORD
               WHEN WS-SEL(1:5) = "Mais "
                   IF WS-SEL(1:9) = "Mais coti"
                       MOVE    "cotistas IS NULL, cotistas DESC"
                                                 TO    WS-ORD
                   ELSE
                       IF WS-SEL(1:9) = "Mais rece"
                           MOVE    "inicio IS NULL, inicio DESC"
                                                 TO    WS-ORD
                       ELSE
                           MOVE    "inicio IS NULL, inicio ASC"
                                                 TO    WS-ORD
                       END-IF
                   END-IF
               WHEN OTHER
                   MOVE    "nome_key ASC"        TO    WS-ORD
           END-EVALUATE
           MOVE    GRD-LIMIT                     TO    WS-LIM
           MOVE    0                             TO    WS-STOP
           MOVE    0                             TO    WS-BYTES
           MOVE    GRD-OFFSET                    TO    WS-OFFS

      *> The 16 columns follow the column order of GRD-FUNDOS, PL scaled to R$ million as its header says. Each
      *> fund is one text, cells joined by TAB, as the desktop's Fetch() joins them; NULL is empty.
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT COALESCE(nome,'') || char(9) || codigo || char(9) || COALESCE(estrutura,'') || char(9) || "
                   "COALESCE(nivel1,'') || char(9) || COALESCE(nivel2,'') || char(9) || "
                   "COALESCE(nivel3,'') || char(9) || COALESCE(strftime('%d/%m/%Y', inicio),'') || char(9) || "
                   "COALESCE(printf('%.2f', pl/1000000.0),'') || char(9) || COALESCE(cotistas,'') || char(9) || "
                   "COALESCE(printf('%.4f', cota),'') || char(9) || COALESCE(foco,'') || char(9) || "
                   "COALESCE(cnpj_classe,'') || char(9) || COALESCE(cnpj_fundo,'') || char(9) || "
                   "COALESCE(status,'') || char(9) || COALESCE(subclasses,'') || char(9) || "
                   "COALESCE(strftime('%d/%m/%Y', data_ref),'') FROM fundos "
                   OPN-WHERE::Trim() " ORDER BY " WS-ORD::Trim() " LIMIT ? OFFSET ?"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           MOVE    WS-LIM                        TO    OPN-PV
           PERFORM OPN-ADD-PARAM
           MOVE    WS-OFFS                       TO    OPN-PV
           PERFORM OPN-ADD-PARAM
           EXEC SQL PREPARE S-GRID FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-GRID CURSOR FOR S-GRID END-EXEC
           EXEC SQL OPEN C-GRID USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM GRD-FAIL
               GOBACK
           END-IF
           PERFORM UNTIL SQLSTATE(1:2) > "01" OR WS-STOP = 1
               MOVE    SPACES                    TO    WS-ROW
               EXEC SQL FETCH C-GRID INTO :WS-ROW END-EXEC
               PERFORM GRD-TAKE
           END-PERFORM
           EXEC SQL CLOSE C-GRID END-EXEC
           PERFORM GRD-DONE
           IF GRD-STATUS NOT = "OK" AND GRD-STATUS NOT = "TRUNC"
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

      *> A fetched row joins the answer unless the answer would pass its byte budget (60,000 of the 64 KiB), in
      *> which case GRD-MORE = Y and the reading stops. SQLSTATE 01004 is a row cut to the 400 bytes of GRD-ROW.
       GRD-TAKE.
           IF SQLSTATE(1:2) < "02"
               IF SQLSTATE = "01004"
                   MOVE    "TRUNC"               TO    GRD-STATUS
                   MOVE    "a row is longer than the 400 bytes GRD-ROW holds"
                                                 TO    GRD-MESSAGE
               END-IF
      *>       What the row costs in the JSON answer: its bytes, 18 for the quotes, the separator and the escapes of
      *>       its 15 TABs, and one more for each quote and backslash.
               MOVE    0                         TO    WS-QUOTES
               INSPECT WS-ROW TALLYING WS-QUOTES FOR ALL '"' WS-QUOTES FOR ALL "\"
               PERFORM VARYING WS-RLEN FROM 400 BY -1
                       UNTIL WS-RLEN = 0 OR WS-ROW (WS-RLEN:1) NOT = SPACE
               END-PERFORM
               ADD     18 WS-QUOTES              TO    WS-RLEN
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

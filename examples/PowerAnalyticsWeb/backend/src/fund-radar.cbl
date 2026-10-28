       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-RADAR.
      *> The two queries of PAINT-RADAR: when two or three company names are ticked, the radar draws one polygon per
      *> company, from the funds each has in each category slot under the filter. Everything else PAINT-RADAR needs is
      *> already in the form (the slots and their counts: FUND-INIT, FUND-CATS). The statements are the desktop's, with
      *> "FROM fundos " + OPN-WHERE (values bound) and "FROM pick" (the ticked names of the request).
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
       01 WS-NSEL                                PIC    9(9).
       01 WS-CATCNT                              PIC    99.
       01 WS-CAT-TAB.
          05 WS-CAT-NAME                         PIC    X(24)    OCCURS 10.
      *> WS-RN and the name of a row are X(160) on the desktop: both are cut to their first 160 BYTES before they are
      *> compared. Here the cut is made in the statements (CAST to BLOB, SUBSTR, back to text) on both sides; a cut inside
      *> a letter reads back as the replacement character U+FFFD (3 bytes), so the text is at most 162 bytes.
       01 WS-C1                                  PIC    X(162).
       01 WS-C2                                  PIC    X(60).
       01 WS-C3                                  PIC    9(9).
       01 WS-I                                   PIC    99.
       01 WS-K                                   PIC    9.
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-radar".

       PROCEDURE DIVISION USING FLT-REQ RETURNING RAD-RES.
       MAIN-PARA.
           INITIALIZE RAD-RES
           MOVE    "OK"                          TO    RAD-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    RAD-STATUS
               MOVE    OPN-MESSAGE               TO    RAD-MESSAGE
               GOBACK
           END-IF

      *> The radar draws one polygon per company only for two or three picked names.
           EXEC SQL SELECT COUNT(*) INTO :WS-NSEL FROM pick WHERE fld = 'N' END-EXEC
           IF WS-NSEL = 2 OR WS-NSEL = 3
               PERFORM RAD-READ
           END-IF
           IF RAD-STATUS NOT = "OK"
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       RAD-READ.
           EXEC SQL DECLARE C-SLOTS CURSOR FOR SELECT name FROM slot ORDER BY n END-EXEC
           EXEC SQL OPEN C-SLOTS END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-SLOTS INTO :WS-C2 END-EXEC
               IF SQLSTATE = "00000"
                   ADD     1                     TO    WS-CATCNT
                   MOVE    WS-C2                 TO    WS-CAT-NAME (WS-CATCNT)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-SLOTS END-EXEC

      *> One polygon per company: the ticked names, in name order.
           EXEC SQL
               DECLARE C-RN CURSOR FOR
                   SELECT CAST(SUBSTR(CAST(name AS BLOB), 1, 160) AS TEXT) FROM pick WHERE fld = 'N' ORDER BY name
           END-EXEC
           EXEC SQL OPEN C-RN END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-RN INTO :WS-C1 END-EXEC
               IF SQLSTATE = "00000" AND RAD-NM < 3
                   ADD     1                     TO    RAD-NM
                   MOVE    WS-C1                 TO    RAD-NAME (RAD-NM)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-RN END-EXEC

      *> The funds of each company in each category; slots are matched by name.
      *> The desktop's statement ("FROM fundos " + OPN-WHERE, values bound; the name cut as above).
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT CAST(SUBSTR(CAST(nome AS BLOB), 1, 160) AS TEXT), nivel1, COUNT(*) FROM fundos "
                   OPN-WHERE::Trim() " AND nivel1 IS NOT NULL GROUP BY nome, nivel1"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-RV FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-RV CURSOR FOR S-RV END-EXEC
           EXEC SQL OPEN C-RV USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM RAD-FAIL
               EXIT PARAGRAPH
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-RV INTO :WS-C1, :WS-C2, :WS-C3 END-EXEC
               IF SQLSTATE = "00000"
                   PERFORM VARYING WS-K FROM 1 BY 1 UNTIL WS-K > RAD-NM
                       IF RAD-NAME (WS-K) = WS-C1
                           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-CATCNT
                               IF WS-CAT-NAME (WS-I) = WS-C2
                                   MOVE    WS-C3 TO    RAD-CNT (WS-K, WS-I)
                               END-IF
                           END-PERFORM
                       END-IF
                   END-PERFORM
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-RV END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM RAD-FAIL
           END-IF.

       RAD-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    RAD-STATUS
           MOVE    OPN-MESSAGE                   TO    RAD-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

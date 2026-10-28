       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-TOP.
      *> Q-TOP of the desktop: the four funds with the largest PL under the filter. The statement is the
      *> desktop's text ("FROM fundos " + OPN-WHERE, values bound, ORDER BY pl DESC LIMIT 4 and nothing more), so
      *> funds that tie on their PL come back in the order the desktop's SQLite gives them for the same filter.
      *> The PL is read as text and stored with NUMVAL, as Q-TOP does. Exported, idempotent: it only reads.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-script".

       DATA DIVISION.
       FILE SECTION.
           COPY "backend/COPYBOOKS/fd-script".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-open".
      *> The name is cut to its first 40 BYTES in the statement (CAST to BLOB, SUBSTR, back to text), as WS-TOP-NAME,
      *> PIC X(40), cuts it on the desktop. A cut inside a letter leaves an invalid byte there; the database layer reads
      *> it as the replacement character U+FFFD (3 bytes), which is what the desktop's label shows. So the text is at
      *> most 42 bytes: 39 whole bytes and the replacement character.
       01 WS-C1                                  PIC    X(42).
       01 WS-C2                                  PIC    X(40).
           EXEC SQL INCLUDE SQLCA END-EXEC.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-filter".
           COPY "backend/COPYBOOKS/res-top".

       PROCEDURE DIVISION USING FLT-REQ RETURNING TOP-RES.
       MAIN-PARA.
           INITIALIZE TOP-RES
           MOVE    "OK"                          TO    TOP-STATUS
           PERFORM OPN-OPEN
           IF OPN-STATUS NOT = "OK"
               MOVE    OPN-STATUS                TO    TOP-STATUS
               MOVE    OPN-MESSAGE               TO    TOP-MESSAGE
               GOBACK
           END-IF

      *> A fund with no name falls back to its code.
      *> (The first column cuts the name in the statement, see WS-C1; what a column computes does not change the
      *> plan, the WHERE and the ORDER BY do.)
           MOVE    SPACES                        TO    OPN-SQL
           STRING  "SELECT CAST(SUBSTR(CAST(COALESCE(nome,codigo) AS BLOB), 1, 40) AS TEXT), pl FROM fundos "
                   OPN-WHERE::Trim() " AND pl IS NOT NULL ORDER BY pl DESC LIMIT 4"
                   DELIMITED BY SIZE INTO OPN-SQL
           PERFORM OPN-WHERE-PARAMS
           EXEC SQL PREPARE S-TOP FROM :OPN-SQL END-EXEC
           EXEC SQL DECLARE C-TOP CURSOR FOR S-TOP END-EXEC
           EXEC SQL OPEN C-TOP USING DESCRIPTOR SQLDA END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOP-FAIL
               GOBACK
           END-IF
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-TOP INTO :WS-C1, :WS-C2 END-EXEC
               IF SQLSTATE = "00000" AND TOP-N < 4
                   ADD     1                     TO    TOP-N
                   MOVE    WS-C1                 TO    TOP-NAME (TOP-N)
                   COMPUTE TOP-PL (TOP-N) = FUNCTION NUMVAL(WS-C2)
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-TOP END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM TOP-FAIL
               GOBACK
           END-IF
           PERFORM OPN-CLOSE
           GOBACK.

       TOP-FAIL.
           PERFORM OPN-SQL-FAILED
           MOVE    OPN-STATUS                    TO    TOP-STATUS
           MOVE    OPN-MESSAGE                   TO    TOP-MESSAGE
           PERFORM OPN-CLOSE.

           COPY "backend/COPYBOOKS/p-db".
           COPY "backend/COPYBOOKS/p-filter".

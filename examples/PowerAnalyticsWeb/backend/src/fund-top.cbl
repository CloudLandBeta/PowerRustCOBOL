       IDENTIFICATION DIVISION.
       PROGRAM-ID. FUND-TOP.
      *> Q-TOP of the desktop: the four funds with the largest PL under the filter. The statement is the
      *> desktop's, with "FROM fv". Exported, idempotent: it only reads.

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
       01 WS-C2                                  PIC    9(15)V99.
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
           EXEC SQL
               DECLARE C-TOP CURSOR FOR
                   SELECT CAST(SUBSTR(CAST(COALESCE(nome,codigo) AS BLOB), 1, 40) AS TEXT), pl
                     FROM fv WHERE pl IS NOT NULL ORDER BY pl DESC, id DESC LIMIT 4
           END-EXEC
           EXEC SQL OPEN C-TOP END-EXEC
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-TOP INTO :WS-C1, :WS-C2 END-EXEC
               IF SQLSTATE = "00000" AND TOP-N < 4
                   ADD     1                     TO    TOP-N
                   MOVE    WS-C1                 TO    TOP-NAME (TOP-N)
                   MOVE    WS-C2                 TO    TOP-PL (TOP-N)
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

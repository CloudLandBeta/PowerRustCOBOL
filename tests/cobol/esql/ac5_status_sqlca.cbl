       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC5C.
      *> Spec 087 AC5 - the SQLCA style, same five outcomes.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           EXEC SQL INCLUDE SQLCA END-EXEC.
       01  WS-N                PIC 9(5).
       01  WS-SHORT            PIC X(3).
       01  WS-CODE             PIC -(8)9.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO ':memory:' AS AC5 END-EXEC
           EXEC SQL CREATE TABLE U (ID INTEGER PRIMARY KEY) END-EXEC
           EXEC SQL INSERT INTO U VALUES (1) END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM U END-EXEC
           PERFORM SHOW
           EXEC SQL SELECT ID INTO :WS-N FROM U WHERE ID = 2 END-EXEC
           PERFORM SHOW
           EXEC SQL SELECT 'truncated' INTO :WS-SHORT END-EXEC
           PERFORM SHOW
           EXEC SQL INSERT INTO U VALUES (1) END-EXEC
           PERFORM SHOW
           EXEC SQL SELEC 1 END-EXEC
           PERFORM SHOW
           DISPLAY "=== ESQL AC5 SQLCA RESULT ==="
           DISPLAY "forms: success, no data, truncation, duplicate "
                   "key, syntax error; SQLCA-ROWS " SQLCA-ROWS
                   " SQLCA-CONNECTION " FUNCTION TRIM(SQLCA-CONNECTION)
           STOP RUN.
       SHOW.
           MOVE SQLCODE TO WS-CODE
           DISPLAY "STATUS " SQLSTATE " " WS-CODE " "
               FUNCTION TRIM(SQLCA-MESSAGE).

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ESQL-AC18.
      *> Spec 087 AC18 - qualification, hyphens, INCLUDE, DECLARE TABLE,
      *> two connections, a password that never shows, DESCRIBE INPUT, and
      *> a prepared statement that ignores later changes to its source.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
           EXEC SQL INCLUDE AC18REC END-EXEC.
           EXEC SQL DECLARE ITEMS TABLE (ID INTEGER, QTY INTEGER)
           END-EXEC.
           EXEC SQL INCLUDE SQLDA END-EXEC.
       01  CUSTOMER.
           05  CITY                PIC X(12).
       01  SUPPLIER.
           05  CITY                PIC X(12).
       01  WS-QTY-LESS-ONE         PIC S9(5).
       01  WS-USER                 PIC X(10) VALUE "ana".
       01  WS-PASSWORD             PIC X(20) VALUE "@SECRET@".
       01  WS-STMT                 PIC X(100).
       01  WS-N                    PIC 9(5).
       01  WS-FROM                 PIC X(10).
       01  SQLSTATE                PIC X(5).
       01  SQLMSG                  PIC X(200).
       01  WS-PASS                 PIC 9(3) VALUE 0.
       01  WS-FAIL                 PIC 9(3) VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           EXEC SQL CONNECT TO 'sqlite:@DIR@/first.db' AS FIRST
                    USER :WS-USER USING :WS-PASSWORD END-EXEC
           EXEC SQL CREATE TABLE ITEMS (ID INTEGER, QTY INTEGER)
           END-EXEC
           EXEC SQL CREATE TABLE PLACES (KIND TEXT, CITY TEXT) END-EXEC
           EXEC SQL INSERT INTO ITEMS VALUES (1, 10) END-EXEC
           EXEC SQL INSERT INTO PLACES VALUES ('C', 'Recife') END-EXEC
           EXEC SQL INSERT INTO PLACES VALUES ('S', 'Natal') END-EXEC
           EXEC SQL COMMIT END-EXEC
      *> Two items share a name: OF, and the period form.
           EXEC SQL SELECT CITY INTO :CITY OF CUSTOMER FROM PLACES
                     WHERE KIND = 'C' END-EXEC
           EXEC SQL SELECT CITY INTO :CITY.SUPPLIER FROM PLACES
                     WHERE KIND = 'S' END-EXEC
           IF CITY OF CUSTOMER = "Recife" AND CITY OF SUPPLIER = "Natal"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL qualification: " CITY OF CUSTOMER " "
                   CITY OF SUPPLIER
           END-IF
      *> The hyphen in QTY-1 is SQL; the one in the host name is COBOL.
           EXEC SQL SELECT QTY-1 INTO :WS-QTY-LESS-ONE FROM ITEMS
                     WHERE ID = 1 END-EXEC
           IF WS-QTY-LESS-ONE = 9
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL QTY-1: " WS-QTY-LESS-ONE
           END-IF
      *> A copybook brought in by INCLUDE declares the host variables.
           EXEC SQL SELECT 7, 'included' INTO :REC-ID, :REC-NAME
           END-EXEC
           IF REC-ID = 7 AND REC-NAME = "included"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL include: " REC-ID " " REC-NAME
           END-IF
      *> A second connection; each reads its own file.
           EXEC SQL CONNECT TO 'sqlite:@DIR@/second.db' AS SECOND
           END-EXEC
           EXEC SQL CREATE TABLE ITEMS (ID INTEGER, QTY INTEGER)
           END-EXEC
           EXEC SQL INSERT INTO ITEMS VALUES (1, 500) END-EXEC
           EXEC SQL INSERT INTO ITEMS VALUES (2, 600) END-EXEC
           EXEC SQL COMMIT END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ITEMS END-EXEC
           IF WS-N = 2
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL second connection: " WS-N
           END-IF
           EXEC SQL SET CONNECTION FIRST END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ITEMS END-EXEC
           IF WS-N = 1
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL SET CONNECTION: " WS-N
           END-IF
      *> DESCRIBE INPUT counts the parameters.
           MOVE "SELECT QTY FROM ITEMS WHERE ID = ? OR QTY > ?"
             TO WS-STMT
           EXEC SQL PREPARE P1 FROM :WS-STMT END-EXEC
           EXEC SQL DESCRIBE INPUT P1 INTO SQLDA END-EXEC
           IF SQLSTATE = "00000" AND SQLDA-COUNT = 2
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL describe input: " SQLSTATE " " SQLDA-COUNT
           END-IF
      *> A prepared statement runs as prepared, whatever its source holds
      *> by now.
           MOVE "INSERT INTO ITEMS VALUES (5, 50)" TO WS-STMT
           EXEC SQL PREPARE P2 FROM :WS-STMT END-EXEC
           MOVE "DELETE FROM ITEMS" TO WS-STMT
           EXEC SQL EXECUTE P2 END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ITEMS END-EXEC
           IF WS-N = 2
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL run as prepared: " WS-N
           END-IF
           EXEC SQL DISCONNECT ALL END-EXEC
           EXEC SQL SELECT COUNT(*) INTO :WS-N FROM ITEMS END-EXEC
           IF SQLSTATE = "08003"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL after DISCONNECT ALL: " SQLSTATE
           END-IF
           DISPLAY "=== ESQL AC18 RESULT ==="
           DISPLAY "forms: :CITY OF CUSTOMER; :CITY.SUPPLIER; SELECT "
                   "QTY-1 INTO :WS-QTY-LESS-ONE; INCLUDE AC18REC; DECLARE "
                   "TABLE; CONNECT ... AS FIRST USER :u USING :p; CONNECT "
                   "... AS SECOND; SET CONNECTION; DESCRIBE INPUT; EXECUTE "
                   "after the source changed; DISCONNECT ALL"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.

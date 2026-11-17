      *> DESKTOP-WASM-001 T41 corpus fixture "sql": EXEC SQL on the
      *> WebAssembly path (ABI A41). Host variables of every kind cross
      *> the import, a host structure, an indicator and NULL, a cursor
      *> read to its end under WHENEVER NOT FOUND, the status items, an
      *> error that continues, and COMMIT; the module's prsql section
      *> carries the program's declarations and its statements.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CORPUSSQL.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SQLSTATE    PIC X(5).
       01 SQLCODE     PIC S9(9) COMP-5.
       01 WS-ITEM.
          05 WS-ID    PIC 9(4).
          05 WS-NAME  PIC X(12).
          05 WS-PRICE PIC S9(5)V99 COMP-3.
       01 WS-NOTE     PIC X(10).
       01 WS-NOTE-IND PIC S9(4) COMP.
       01 WS-RATE     COMP-2.
       01 WS-COUNT    PIC 9(3).
       01 WS-TOTAL    PIC S9(7)V99.
       01 WS-SHOW     PIC -(6)9.99.
       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "corpus sql fixture"
           EXEC SQL CONNECT TO ':memory:' AS CORPUS END-EXEC
           EXEC SQL
               CREATE TABLE ITEM (ID INTEGER, NAME VARCHAR(12),
                                  PRICE DECIMAL(7,2), NOTE VARCHAR(10),
                                  RATE DOUBLE)
           END-EXEC
           MOVE 1 TO WS-ID
           MOVE "LAMP" TO WS-NAME
           MOVE 19.90 TO WS-PRICE
           MOVE "BRASS" TO WS-NOTE
           MOVE 0 TO WS-NOTE-IND
           MOVE 0.5 TO WS-RATE
           PERFORM INSERT-ITEM
           MOVE 2 TO WS-ID
           MOVE "CHAIR" TO WS-NAME
           MOVE 120.05 TO WS-PRICE
           MOVE -1 TO WS-NOTE-IND
           MOVE 0.25 TO WS-RATE
           PERFORM INSERT-ITEM
           MOVE 3 TO WS-ID
           MOVE "TABLE" TO WS-NAME
           MOVE 349.00 TO WS-PRICE
           MOVE "OAK" TO WS-NOTE
           MOVE 0 TO WS-NOTE-IND
           PERFORM INSERT-ITEM
           EXEC SQL SELECT COUNT(*) INTO :WS-COUNT FROM ITEM END-EXEC
           DISPLAY "rows " WS-COUNT " sqlcode " SQLCODE
           EXEC SQL
               SELECT NAME, PRICE, NOTE INTO :WS-NAME, :WS-PRICE,
                      :WS-NOTE:WS-NOTE-IND
               FROM ITEM WHERE ID = 2
           END-EXEC
           MOVE WS-PRICE TO WS-SHOW
           DISPLAY "item 2 [" WS-NAME "] " WS-SHOW " ind " WS-NOTE-IND
           EXEC SQL DECLARE C1 CURSOR FOR
               SELECT ID, NAME, PRICE FROM ITEM ORDER BY ID
           END-EXEC
           EXEC SQL OPEN C1 END-EXEC
           MOVE 0 TO WS-TOTAL
           EXEC SQL WHENEVER NOT FOUND GO TO READ-DONE END-EXEC
           PERFORM UNTIL 0 = 1
               EXEC SQL FETCH C1 INTO :WS-ITEM END-EXEC
               ADD WS-PRICE TO WS-TOTAL
               DISPLAY "fetch " WS-ID " " WS-NAME
           END-PERFORM.
       READ-DONE.
           EXEC SQL WHENEVER NOT FOUND CONTINUE END-EXEC
           EXEC SQL CLOSE C1 END-EXEC
           MOVE WS-TOTAL TO WS-SHOW
           DISPLAY "total " WS-SHOW " sqlstate " SQLSTATE
           EXEC SQL SELECT RATE INTO :WS-RATE FROM ITEM WHERE ID = 3
           END-EXEC
           IF WS-RATE = 0.25
               DISPLAY "rate 0.25"
           ELSE
               DISPLAY "rate wrong"
           END-IF
           EXEC SQL SELEC 1 END-EXEC
           DISPLAY "error " SQLSTATE " continues"
           EXEC SQL COMMIT END-EXEC
           EXEC SQL DISCONNECT ALL END-EXEC
           DISPLAY "sqlcode " SQLCODE
           MOVE 5 TO RETURN-CODE
           STOP RUN.
       INSERT-ITEM.
           EXEC SQL
               INSERT INTO ITEM VALUES (:WS-ID, :WS-NAME, :WS-PRICE,
                                        :WS-NOTE:WS-NOTE-IND, :WS-RATE)
           END-EXEC.

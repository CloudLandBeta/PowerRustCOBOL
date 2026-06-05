       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDX-TX.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "/tmp/idx-tx.dat"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS R-COD
               STORAGE IS DISK
               FILE STATUS IS WS-ST.
       DATA DIVISION.
       FILE SECTION.
       FD F.
       01 R-REC.
          05 R-COD  PIC 9(4).
          05 R-NOME PIC X(8).
       WORKING-STORAGE SECTION.
       01 WS-ST  PIC XX.
       01 WS-EOF PIC 9 VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           MOVE 0001 TO R-COD MOVE "ALPHA" TO R-NOME WRITE R-REC
           MOVE 0002 TO R-COD MOVE "BETA" TO R-NOME WRITE R-REC
           CLOSE F
           OPEN I-O F
           MOVE 0003 TO R-COD MOVE "GAMMA" TO R-NOME WRITE R-REC
           COMMIT
           MOVE 0004 TO R-COD MOVE "DELTA" TO R-NOME WRITE R-REC
           MOVE 0001 TO R-COD READ F END-READ
           MOVE "ALPHAX" TO R-NOME REWRITE R-REC
           MOVE 0002 TO R-COD DELETE F
           ROLLBACK
           MOVE 0000 TO R-COD
           START F KEY IS GREATER THAN R-COD END-START
           PERFORM UNTIL WS-EOF = 1
               READ F NEXT
                   AT END MOVE 1 TO WS-EOF
                   NOT AT END DISPLAY "TX " R-COD " " R-NOME
               END-READ
           END-PERFORM
           CLOSE F
           STOP RUN.

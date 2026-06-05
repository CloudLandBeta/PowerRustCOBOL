       IDENTIFICATION DIVISION.
       PROGRAM-ID. IDX-PERSIST.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT F ASSIGN TO "/tmp/idx-persist.dat"
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
       01 WS-CNT PIC 9(2) VALUE 0.
       PROCEDURE DIVISION.
       MAIN.
           OPEN OUTPUT F
           MOVE 0010 TO R-COD MOVE "DEZ" TO R-NOME WRITE R-REC
           MOVE 0020 TO R-COD MOVE "VINTE" TO R-NOME WRITE R-REC
           MOVE 0030 TO R-COD MOVE "TRINTA" TO R-NOME WRITE R-REC
           CLOSE F
           OPEN INPUT F
           PERFORM UNTIL WS-EOF = 1
               READ F NEXT
                   AT END MOVE 1 TO WS-EOF
                   NOT AT END
                       ADD 1 TO WS-CNT
                       DISPLAY "REC " R-COD " " R-NOME
               END-READ
           END-PERFORM
           DISPLAY "TOTAL " WS-CNT
           CLOSE F
           STOP RUN.

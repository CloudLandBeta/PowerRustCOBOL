       IDENTIFICATION DIVISION.
       PROGRAM-ID. PS-SETTING-PUT.
      *>   Keeps one of the application's settings: written when new,
      *>   rewritten when it was already there. The file is made the
      *>   first time a setting is kept.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SETTINGS-FILE ASSIGN TO WS-PATH
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS SET-NAME
               FILE STATUS IS WS-FS.
       DATA DIVISION.
       FILE SECTION.
       FD SETTINGS-FILE.
       01 SET-RECORD.
          05 SET-NAME  PIC X(20).
          05 SET-VALUE PIC X(200).
       WORKING-STORAGE SECTION.
       01 WS-PATH PIC X(400).
       01 WS-DIR  PIC X(380).
       01 WS-FS   PIC XX.
       LINKAGE SECTION.
       01 LK-NAME  PIC X(20).
       01 LK-VALUE PIC X(200).
       PROCEDURE DIVISION USING LK-NAME LK-VALUE.
           MOVE SPACES TO WS-DIR
           ACCEPT WS-DIR FROM ENVIRONMENT "POWERSPATIAL_DATA"
           IF WS-DIR = SPACES
               MOVE "data/settings.idx" TO WS-PATH
           ELSE
               MOVE SPACES TO WS-PATH
               STRING FUNCTION TRIM(WS-DIR) "/settings.idx"
                   DELIMITED BY SIZE INTO WS-PATH
           END-IF
           OPEN I-O SETTINGS-FILE
           IF WS-FS = "35"
               OPEN OUTPUT SETTINGS-FILE
               CLOSE SETTINGS-FILE
               OPEN I-O SETTINGS-FILE
           END-IF
           IF WS-FS NOT = "00"
               GOBACK
           END-IF
           MOVE LK-NAME TO SET-NAME
           MOVE LK-VALUE TO SET-VALUE
           WRITE SET-RECORD
               INVALID KEY REWRITE SET-RECORD
           END-WRITE
           CLOSE SETTINGS-FILE
           GOBACK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PS-SETTING-GET.
      *>   Reads one of the application's settings by name. A setting
      *>   never written reads as spaces. The settings live in
      *>   data/settings.idx, or under POWERSPATIAL_DATA when it is set.
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
           MOVE SPACES TO LK-VALUE
           MOVE SPACES TO WS-DIR
           ACCEPT WS-DIR FROM ENVIRONMENT "POWERSPATIAL_DATA"
           IF WS-DIR = SPACES
               MOVE "data/settings.idx" TO WS-PATH
           ELSE
               MOVE SPACES TO WS-PATH
               STRING FUNCTION TRIM(WS-DIR) "/settings.idx"
                   DELIMITED BY SIZE INTO WS-PATH
           END-IF
           OPEN INPUT SETTINGS-FILE
           IF WS-FS NOT = "00"
               GOBACK
           END-IF
           MOVE LK-NAME TO SET-NAME
           READ SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE SET-VALUE TO LK-VALUE
           END-READ
           CLOSE SETTINGS-FILE
           GOBACK.

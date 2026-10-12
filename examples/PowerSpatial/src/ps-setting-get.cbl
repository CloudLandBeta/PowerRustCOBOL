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
           05 SET-NAME                           PIC    X(20).
           05 SET-VALUE                          PIC    X(200).

       WORKING-STORAGE SECTION.
       01 WS-PATH                                PIC    X(400).
       01 WS-DIR                                 PIC    X(380).
       01 WS-FS                                  PIC    XX.

       LINKAGE SECTION.
       01 LK-NAME                                PIC    X(20).
       01 LK-VALUE                               PIC    X(200).

       PROCEDURE DIVISION USING LK-NAME LK-VALUE.
           SET     LK-VALUE                      TO    SPACES
           SET     WS-DIR                        TO    SPACES

           ACCEPT    WS-DIR FROM ENVIRONMENT "POWERSPATIAL_DATA"
           IF    WS-DIR = SPACES
               SET     WS-PATH                   TO    "data/settings.idx"
           ELSE
               SET     WS-PATH                   TO    SPACES
               STRING    WS-DIR::Trim() "/settings.idx" INTO WS-PATH
           END-IF
           OPEN    INPUT SETTINGS-FILE

           *> No file yet means nothing was ever kept: return the spaces set above,
           *> as for any other open failure.

           IF    WS-FS NOT = "00"
               GOBACK
           END-IF
           SET     SET-NAME                      TO    LK-NAME
           READ    SETTINGS-FILE
               INVALID KEY CONTINUE
               NOT INVALID KEY MOVE SET-VALUE TO LK-VALUE
           END-READ
           CLOSE    SETTINGS-FILE
           GOBACK.

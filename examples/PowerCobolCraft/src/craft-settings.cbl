       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-SETTINGS.

      *> Keeps the player's choices between runs: the player's name, the game mode and the
      *> language of the screens. One record per setting, by name, in the indexed file
      *> data/settings.idx. LK-ACTION "GET" reads a setting into LK-VALUE (spaces when it was
      *> never kept, or when the file cannot be opened); "PUT" writes it, or rewrites it when
      *> it is already there. The file is made the first time a setting is kept.

       ENVIRONMENT DIVISION.

       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT SETTINGS-FILE ASSIGN TO "data/settings.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS SET-NAME
               FILE STATUS IS WS-FILE-STATUS.

       DATA DIVISION.

       FILE SECTION.
       FD SETTINGS-FILE.
       01 SET-RECORD.
           05 SET-NAME                           PIC    X(20).
           05 SET-VALUE                          PIC    X(80).

       WORKING-STORAGE SECTION.
       01 WS-FILE-STATUS                         PIC    XX.

       LINKAGE SECTION.
       01 LK-ACTION                              PIC    X(3).
       01 LK-NAME                                PIC    X(20).
       01 LK-VALUE                               PIC    X(80).

       PROCEDURE DIVISION USING LK-ACTION LK-NAME LK-VALUE.
           IF    LK-ACTION = "GET"
               SET     LK-VALUE                      TO    SPACES
               OPEN    INPUT SETTINGS-FILE

               *> No file yet means nothing was ever kept: the spaces set above are the answer.

               IF    WS-FILE-STATUS NOT = "00"
                   GOBACK
               END-IF
               SET     SET-NAME                      TO    LK-NAME
               READ    SETTINGS-FILE
                   INVALID KEY CONTINUE
                   NOT INVALID KEY MOVE SET-VALUE TO LK-VALUE
               END-READ
               CLOSE    SETTINGS-FILE
               GOBACK
           END-IF

           *> PUT: open for update, or make the file when it is not there yet ("35").

           OPEN    I-O SETTINGS-FILE
           IF    WS-FILE-STATUS = "35"
               OPEN    OUTPUT SETTINGS-FILE
               CLOSE    SETTINGS-FILE
               OPEN    I-O SETTINGS-FILE
           END-IF
           IF    WS-FILE-STATUS NOT = "00"
               GOBACK
           END-IF
           SET     SET-NAME                      TO    LK-NAME
           SET     SET-VALUE                     TO    LK-VALUE
           WRITE    SET-RECORD
               INVALID KEY REWRITE SET-RECORD
           END-WRITE
           CLOSE    SETTINGS-FILE
           GOBACK.

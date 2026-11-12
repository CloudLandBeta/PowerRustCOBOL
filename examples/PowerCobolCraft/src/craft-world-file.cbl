       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-WORLD-FILE.

      *> Saves the world and reads it back. The file data/world.dat holds one line for the player
      *> and the world's state (LK-HEADER) and then one line for every row of the 48 by 48 columns
      *> of blocks (LK-WORLD): 48 columns of 24 blocks each, one character a block, bottom first.
      *>
      *> LK-ACTION "SAVE" writes the file, "LOAD" reads all of it, "INFO" reads only the first line
      *> (the title screen uses it to offer "Continue"). LK-STATUS answers 0 when it worked and 1
      *> when there is no saved world, or the file is not one this program wrote.
      *> The caller declares LK-HEADER and LK-WORLD with exactly these fields.

       ENVIRONMENT DIVISION.

       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT WORLD-FILE ASSIGN TO "data/world.dat"
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS WS-FILE-STATUS.

       DATA DIVISION.

       FILE SECTION.
       FD WORLD-FILE.
       01 WORLD-LINE                             PIC    X(1152).

       WORKING-STORAGE SECTION.
       01 WS-FILE-STATUS                         PIC    XX.
       01 WS-ROW                                 PIC    99.
       01 WS-NOW                                 PIC    X(21).

       LINKAGE SECTION.
       01 LK-ACTION                              PIC    X(4).
       01 LK-HEADER.
           05 LK-MAGIC                           PIC    X(12).
           05 LK-PLAYER-NAME                     PIC    X(16).
           05 LK-WORLD-TIME                      PIC    9(5).
           05 LK-DAY                             PIC    9(5).
           05 LK-GAME-MODE                       PIC    9.
           05 LK-POS-X                           PIC    S9(4)V9(4).
           05 LK-POS-Y                           PIC    S9(4)V9(4).
           05 LK-POS-Z                           PIC    S9(4)V9(4).
           05 LK-YAW                             PIC    S9(3)V9(4).
           05 LK-PITCH                           PIC    S9(3)V9(4).
           05 LK-HEALTH                          PIC    99.
           05 LK-SLOT                            PIC    9.
           05 LK-COUNT                           PIC    999     OCCURS 9.
           05 LK-WHITELIST-ON                    PIC    9.
           05 LK-WHITELIST-COUNT                 PIC    99.
           05 LK-WHITELIST-NAME                  PIC    X(16)   OCCURS 20.
           05 LK-SAVED-AT                        PIC    X(16).
       01 LK-WORLD.
           05 LK-WORLD-ROW                               OCCURS 48.
               10 LK-WORLD-COLUMN                PIC    X(24)   OCCURS 48.
       01 LK-STATUS                              PIC    9.

       PROCEDURE DIVISION USING LK-ACTION LK-HEADER LK-WORLD LK-STATUS.
           SET     LK-STATUS                     TO    1
           IF    LK-ACTION = "SAVE"
               PERFORM SAVE-WORLD
           ELSE
               PERFORM READ-WORLD
           END-IF
           GOBACK.

       SAVE-WORLD.

           *> The header carries the program's mark and when it was saved, as "YYYY-MM-DD HH:MM".

           SET     WS-NOW                        TO    FUNCTION CURRENT-DATE
           SET     LK-MAGIC                      TO    "COBOLCRAFT01"
           STRING    WS-NOW(1:4) "-" WS-NOW(5:2) "-" WS-NOW(7:2) " " WS-NOW(9:2) ":" WS-NOW(11:2)
               DELIMITED BY SIZE INTO LK-SAVED-AT
           OPEN    OUTPUT WORLD-FILE
           IF    WS-FILE-STATUS NOT = "00"
               EXIT PARAGRAPH
           END-IF
           SET     WORLD-LINE                    TO    LK-HEADER
           WRITE    WORLD-LINE
           PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > 48
               SET     WORLD-LINE                    TO    LK-WORLD-ROW(WS-ROW)
               WRITE    WORLD-LINE
           END-PERFORM
           CLOSE    WORLD-FILE
           SET     LK-STATUS                     TO    0.

       READ-WORLD.
           OPEN    INPUT WORLD-FILE
           IF    WS-FILE-STATUS NOT = "00"
               EXIT PARAGRAPH
           END-IF
           READ    WORLD-FILE
               AT END
                   CLOSE    WORLD-FILE
                   EXIT PARAGRAPH
           END-READ
           IF    WORLD-LINE(1:12) NOT = "COBOLCRAFT01"
               CLOSE    WORLD-FILE
               EXIT PARAGRAPH
           END-IF
           SET     LK-HEADER                     TO    WORLD-LINE
           IF    LK-ACTION = "LOAD"

               *> A row missing at the end of a damaged file is left as it was.

               PERFORM VARYING WS-ROW FROM 1 BY 1 UNTIL WS-ROW > 48
                   READ    WORLD-FILE
                       AT END
                           SET     WS-ROW                        TO    48
                       NOT AT END
                           SET     LK-WORLD-ROW(WS-ROW)          TO    WORLD-LINE
                   END-READ
               END-PERFORM
           END-IF
           CLOSE    WORLD-FILE
           SET     LK-STATUS                     TO    0.

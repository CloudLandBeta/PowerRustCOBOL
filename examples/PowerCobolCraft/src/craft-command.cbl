       IDENTIFICATION DIVISION.
       PROGRAM-ID. CRAFT-COMMAND.

      *> One line typed into the console, run as CobolCraft runs it: a leading "/" is dropped
      *> (CobolCraft's own console takes "help" as well as "/help"), the line is split on spaces,
      *> the first word names the command (case matters), and the answers are CobolCraft's own
      *> words - "Unknown command: ...", "Usage: " followed by the command's help, "Set the time
      *> to ...". A line without the slash whose first word is no command is a chat message
      *> from the player, "<name> text".
      *>
      *> The commands, in the order /help lists them, change LK-STATE themselves (the time, the
      *> game mode, the whitelist); what needs the world - killing the player, saving, stopping -
      *> comes back in LK-ACTION for the form to do: CHAT, KILL, SAVE, STOP, TIME, MODE or NONE.

       DATA DIVISION.

       WORKING-STORAGE SECTION.
       01 WS-LINE                                PIC    X(200).
       01 WS-PARTS.
           05 WS-PART                            PIC    X(64)   OCCURS 32.
       01 WS-PART-COUNT                          PIC    99.
       01 WS-INDEX                               PIC    99.
       01 WS-FOUND                               PIC    99.
       01 WS-POINTER                             PIC    9(3).
       01 WS-NUMBER                              PIC    S9(9).
       01 WS-SHOW-NUMBER                         PIC    -(9)9.
       01 WS-SHOW-COUNT                          PIC    Z9.
       01 WS-MODE                                PIC    9.
       01 WS-MODE-NAME                           PIC    X(16).

      *> The help line of every command, as CobolCraft registers it.
       01 WS-HELP-DATA.
           05 FILLER                             PIC    X(80)   VALUE
               "/help - show this help".
           05 FILLER                             PIC    X(80)   VALUE
               "/gamemode <gamemode> [player] - change your game mode".
           05 FILLER                             PIC    X(80)   VALUE
               "/kill [player] - kill a player".
           05 FILLER                             PIC    X(80)   VALUE
               "/save - save the world".
           05 FILLER                             PIC    X(80)   VALUE
               "/say <message> - broadcast a message".
           05 FILLER                             PIC    X(80)   VALUE
               "/stop - stop the server".
           05 FILLER                             PIC    X(80)   VALUE
               "/time set (day|noon|night|midnight|<time>) - change the time".
           05 FILLER                             PIC    X(80)   VALUE
               "/whitelist (reload|on|off|list|add|remove) [player] - manage the whitelist".
       01 WS-HELP REDEFINES WS-HELP-DATA.
           05 WS-HELP-LINE                       PIC    X(80)   OCCURS 8.
       01 WS-COMMAND-NAMES                       PIC    X(80)   VALUE
               "help    gamemodekill    save    say     stop    time    whitelist".
       01 WS-COMMAND-INDEX                       PIC    9.
       01 WS-SLASH                               PIC    9.

       LINKAGE SECTION.
       01 LK-INPUT                               PIC    X(200).
       01 LK-STATE.
           05 LK-PLAYER-NAME                     PIC    X(16).
           05 LK-WORLD-TIME                      PIC    9(5).
           05 LK-GAME-MODE                       PIC    9.
           05 LK-WHITELIST-ON                    PIC    9.
           05 LK-WHITELIST-COUNT                 PIC    99.
           05 LK-WHITELIST-NAME                  PIC    X(16)   OCCURS 20.
       01 LK-REPLY.
           05 LK-ACTION                          PIC    X(8).
           05 LK-LINE-COUNT                      PIC    99.
           05 LK-LINE                            PIC    X(160)  OCCURS 12.

       PROCEDURE DIVISION USING LK-INPUT LK-STATE LK-REPLY.
           SET     LK-ACTION                     TO    "NONE"
           SET     LK-LINE-COUNT                 TO    0
           SET     WS-LINE                       TO    LK-INPUT::Trim()
           IF    WS-LINE = SPACES
               GOBACK
           END-IF

           *> Drop the slash and split the rest on spaces (runs of spaces make no empty parts).

           SET     WS-SLASH                      TO    0
           IF    WS-LINE(1:1) = "/"
               SET     WS-SLASH                      TO    1
               SET     WS-LINE                       TO    WS-LINE(2:)
           END-IF
           SET     WS-PARTS                      TO    SPACES
           SET     WS-PART-COUNT                 TO    0
           UNSTRING WS-LINE DELIMITED BY ALL SPACE
               INTO WS-PART(1) WS-PART(2) WS-PART(3) WS-PART(4) WS-PART(5) WS-PART(6) WS-PART(7) WS-PART(8)
                    WS-PART(9) WS-PART(10) WS-PART(11) WS-PART(12) WS-PART(13) WS-PART(14) WS-PART(15) WS-PART(16)
                    WS-PART(17) WS-PART(18) WS-PART(19) WS-PART(20) WS-PART(21) WS-PART(22) WS-PART(23) WS-PART(24)
                    WS-PART(25) WS-PART(26) WS-PART(27) WS-PART(28) WS-PART(29) WS-PART(30) WS-PART(31) WS-PART(32)
               TALLYING IN WS-PART-COUNT
           END-UNSTRING

           *> Which command: the names are kept eight characters apart in WS-COMMAND-NAMES.

           SET     WS-COMMAND-INDEX              TO    0
           PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > 8
               IF    WS-COMMAND-NAMES((WS-INDEX - 1) * 8 + 1:8) = WS-PART(1)
                   AND WS-PART(1)(9:) = SPACES
                   SET     WS-COMMAND-INDEX              TO    WS-INDEX
               END-IF
           END-PERFORM
           IF    WS-PART(1) = "whitelist"
               SET     WS-COMMAND-INDEX              TO    8
           END-IF

           *> No command and no slash: a chat message, which everyone sees as "<name> text".

           IF    WS-COMMAND-INDEX = 0 AND WS-SLASH = 0
               SET     LK-ACTION                     TO    "CHAT"
               ADD     1                             TO    LK-LINE-COUNT
               STRING    "<" LK-PLAYER-NAME::Trim() "> " LK-INPUT::Trim()
                   INTO LK-LINE(LK-LINE-COUNT)
               GOBACK
           END-IF

           EVALUATE WS-COMMAND-INDEX
               WHEN 1
                   PERFORM COMMAND-HELP
               WHEN 2
                   PERFORM COMMAND-GAMEMODE
               WHEN 3
                   PERFORM COMMAND-KILL
               WHEN 4
                   PERFORM COMMAND-SAVE
               WHEN 5
                   PERFORM COMMAND-SAY
               WHEN 6
                   PERFORM COMMAND-STOP
               WHEN 7
                   PERFORM COMMAND-TIME
               WHEN 8
                   PERFORM COMMAND-WHITELIST
               WHEN OTHER
                   ADD     1                             TO    LK-LINE-COUNT
                   STRING    "Unknown command: " WS-PART(1)::Trim() INTO LK-LINE(LK-LINE-COUNT)
           END-EVALUATE
           GOBACK.

       COMMAND-HELP.
           IF    WS-PART-COUNT NOT = 1
               PERFORM PRINT-USAGE
           ELSE
               ADD     1                             TO    LK-LINE-COUNT
               SET     LK-LINE(LK-LINE-COUNT)        TO    "Available commands:"
               PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > 8
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    WS-HELP-LINE(WS-INDEX)
               END-PERFORM
           END-IF.

       COMMAND-GAMEMODE.

           *> The mode is written in lower case, as CobolCraft expects; a player named must be the
           *> one in this world.

           SET     WS-MODE                       TO    0
           EVALUATE WS-PART(2)
               WHEN "survival"
                   SET     WS-MODE                       TO    1
                   SET     WS-MODE-NAME                  TO    "Survival Mode"
               WHEN "creative"
                   SET     WS-MODE                       TO    2
                   SET     WS-MODE-NAME                  TO    "Creative Mode"
               WHEN "adventure"
                   SET     WS-MODE                       TO    3
                   SET     WS-MODE-NAME                  TO    "Adventure Mode"
               WHEN "spectator"
                   SET     WS-MODE                       TO    4
                   SET     WS-MODE-NAME                  TO    "Spectator Mode"
           END-EVALUATE
           EVALUATE TRUE
               WHEN WS-MODE = 0 OR WS-PART-COUNT < 2 OR WS-PART-COUNT > 3
                   PERFORM PRINT-USAGE
               WHEN WS-PART-COUNT = 3 AND WS-PART(3) NOT = LK-PLAYER-NAME
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    "Player not found"
               WHEN WS-PART-COUNT = 3
                   SET     LK-GAME-MODE                  TO    WS-MODE
                   SET     LK-ACTION                     TO    "MODE"
                   ADD     1                             TO    LK-LINE-COUNT
                   STRING    "Set " LK-PLAYER-NAME::Trim() "'s game mode to " WS-MODE-NAME::Trim()
                       INTO LK-LINE(LK-LINE-COUNT)
               WHEN OTHER
                   SET     LK-GAME-MODE                  TO    WS-MODE
                   SET     LK-ACTION                     TO    "MODE"
                   ADD     1                             TO    LK-LINE-COUNT
                   STRING    "Set own game mode to " WS-MODE-NAME::Trim() INTO LK-LINE(LK-LINE-COUNT)
           END-EVALUATE.

       COMMAND-KILL.
           EVALUATE TRUE
               WHEN WS-PART-COUNT > 2
                   PERFORM PRINT-USAGE
               WHEN WS-PART-COUNT = 2 AND WS-PART(2) NOT = LK-PLAYER-NAME
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    "Player not found"
               WHEN OTHER
                   SET     LK-ACTION                     TO    "KILL"
                   ADD     1                             TO    LK-LINE-COUNT
                   STRING    "Killed " LK-PLAYER-NAME::Trim() INTO LK-LINE(LK-LINE-COUNT)
           END-EVALUATE.

       COMMAND-SAVE.

           *> CobolCraft answers "Saving world" before it saves and "World saved" after: the form
           *> saves on SAVE and then writes the second line.

           IF    WS-PART-COUNT NOT = 1
               PERFORM PRINT-USAGE
           ELSE
               SET     LK-ACTION                     TO    "SAVE"
               ADD     1                             TO    LK-LINE-COUNT
               SET     LK-LINE(LK-LINE-COUNT)        TO    "Saving world"
           END-IF.

       COMMAND-SAY.

           *> "[name] word word ...": the words are joined again by single spaces.

           IF    WS-PART-COUNT < 2
               PERFORM PRINT-USAGE
           ELSE
               SET     LK-ACTION                     TO    "CHAT"
               ADD     1                             TO    LK-LINE-COUNT
               SET     WS-POINTER                    TO    1
               STRING    "[" LK-PLAYER-NAME::Trim() "]" INTO LK-LINE(LK-LINE-COUNT) WITH POINTER WS-POINTER
               PERFORM VARYING WS-INDEX FROM 2 BY 1 UNTIL WS-INDEX > WS-PART-COUNT
                   STRING    " " WS-PART(WS-INDEX)::Trim() INTO LK-LINE(LK-LINE-COUNT) WITH POINTER WS-POINTER
               END-PERFORM
           END-IF.

       COMMAND-STOP.
           IF    WS-PART-COUNT NOT = 1
               PERFORM PRINT-USAGE
           ELSE
               SET     LK-ACTION                     TO    "STOP"
           END-IF.

       COMMAND-TIME.
           IF    WS-PART-COUNT NOT = 3 OR WS-PART(2) NOT = "set"
               PERFORM PRINT-USAGE
           ELSE
               EVALUATE WS-PART(3)
                   WHEN "day"
                       SET     WS-NUMBER                     TO    1000
                   WHEN "noon"
                       SET     WS-NUMBER                     TO    6000
                   WHEN "night"
                       SET     WS-NUMBER                     TO    13000
                   WHEN "midnight"
                       SET     WS-NUMBER                     TO    18000
                   WHEN OTHER
                       COMPUTE WS-NUMBER = FUNCTION NUMVAL( WS-PART(3) )
               END-EVALUATE

               *> The world's clock keeps the time within the day; the answer repeats the time given.

               COMPUTE LK-WORLD-TIME = FUNCTION MOD( WS-NUMBER, 24000 )
               SET     LK-ACTION                     TO    "TIME"
               SET     WS-SHOW-NUMBER                TO    WS-NUMBER
               ADD     1                             TO    LK-LINE-COUNT
               STRING    "Set the time to " WS-SHOW-NUMBER::Trim() INTO LK-LINE(LK-LINE-COUNT)
           END-IF.

       COMMAND-WHITELIST.
           EVALUATE TRUE
               WHEN WS-PART-COUNT = 2 AND WS-PART(2) = "reload"
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    "Reloaded the whitelist"
               WHEN WS-PART-COUNT = 2 AND WS-PART(2) = "on"
                   SET     LK-WHITELIST-ON               TO    1
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    "Whitelist enabled"
               WHEN WS-PART-COUNT = 2 AND WS-PART(2) = "off"
                   SET     LK-WHITELIST-ON               TO    0
                   ADD     1                             TO    LK-LINE-COUNT
                   SET     LK-LINE(LK-LINE-COUNT)        TO    "Whitelist disabled"
               WHEN WS-PART-COUNT = 2 AND WS-PART(2) = "list"
                   PERFORM WHITELIST-LIST
               WHEN WS-PART-COUNT = 3 AND WS-PART(2) = "add"
                   PERFORM WHITELIST-FIND
                   ADD     1                             TO    LK-LINE-COUNT
                   EVALUATE TRUE
                       WHEN WS-FOUND > 0
                           SET     LK-LINE(LK-LINE-COUNT)        TO    "Player is already whitelisted"
                       WHEN LK-WHITELIST-COUNT >= 20
                           SET     LK-LINE(LK-LINE-COUNT)        TO    "Input/output error"
                       WHEN OTHER
                           ADD     1                             TO    LK-WHITELIST-COUNT
                           SET     LK-WHITELIST-NAME(LK-WHITELIST-COUNT)    TO    WS-PART(3)
                           STRING    "Added " WS-PART(3)::Trim() " to the whitelist" INTO LK-LINE(LK-LINE-COUNT)
                   END-EVALUATE
               WHEN WS-PART-COUNT = 3 AND WS-PART(2) = "remove"
                   PERFORM WHITELIST-FIND
                   ADD     1                             TO    LK-LINE-COUNT
                   IF    WS-FOUND = 0
                       SET     LK-LINE(LK-LINE-COUNT)        TO    "Player is not whitelisted"
                   ELSE

                       *> The last name takes the place of the one removed.

                       SET     LK-WHITELIST-NAME(WS-FOUND)   TO    LK-WHITELIST-NAME(LK-WHITELIST-COUNT)
                       SET     LK-WHITELIST-NAME(LK-WHITELIST-COUNT)    TO    SPACES
                       SUBTRACT 1                            FROM  LK-WHITELIST-COUNT
                       STRING    "Removed " WS-PART(3)::Trim() " from the whitelist" INTO LK-LINE(LK-LINE-COUNT)
                   END-IF
               WHEN OTHER
                   PERFORM PRINT-USAGE
           END-EVALUATE.

       WHITELIST-LIST.
           ADD     1                             TO    LK-LINE-COUNT
           IF    LK-WHITELIST-COUNT = 0
               SET     LK-LINE(LK-LINE-COUNT)        TO    "There are no whitelisted players"
           ELSE
               SET     WS-SHOW-COUNT                 TO    LK-WHITELIST-COUNT
               SET     WS-POINTER                    TO    1
               STRING    "There are " WS-SHOW-COUNT::Trim() " whitelisted player(s): "
                   INTO LK-LINE(LK-LINE-COUNT) WITH POINTER WS-POINTER
               PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > LK-WHITELIST-COUNT
                   IF    WS-INDEX > 1
                       STRING    ", " INTO LK-LINE(LK-LINE-COUNT) WITH POINTER WS-POINTER
                   END-IF
                   STRING    LK-WHITELIST-NAME(WS-INDEX)::Trim() INTO LK-LINE(LK-LINE-COUNT) WITH POINTER WS-POINTER
               END-PERFORM
           END-IF.

       WHITELIST-FIND.
           SET     WS-FOUND                      TO    0
           PERFORM VARYING WS-INDEX FROM 1 BY 1 UNTIL WS-INDEX > LK-WHITELIST-COUNT
               IF    LK-WHITELIST-NAME(WS-INDEX) = WS-PART(3)
                   SET     WS-FOUND                      TO    WS-INDEX
               END-IF
           END-PERFORM.

       PRINT-USAGE.
           ADD     1                             TO    LK-LINE-COUNT
           STRING    "Usage: " WS-HELP-LINE(WS-COMMAND-INDEX)::Trim() INTO LK-LINE(LK-LINE-COUNT).

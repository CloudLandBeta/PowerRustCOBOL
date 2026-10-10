       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-AUDIO
      *> The sound built-ins (spec 092 R15-R18), written inline:
      *>   COBOL::"AUDIO-PLAY"   ( file status )
      *>   COBOL::"AUDIO-PAUSE"  ( status )
      *>   COBOL::"AUDIO-RESUME" ( status )
      *>   COBOL::"AUDIO-STOP"   ( status )
      *>   COBOL::"AUDIO-STATE"  ( state position-ms duration-ms )
      *> Run it on the null output, which decodes the file exactly
      *> as for a sound card and lets a clock "play" it - no device
      *> is needed and nothing is heard:
      *>   COBOLT_AUDIO=null rcrun run tests/cobol/audio/test-audio.cbl
      *> The sound is tone-2s.wav: two seconds of silence, 8 kHz
      *> 8-bit mono PCM. AUDIO_TEST_DIR names the folder holding it
      *> when the program is not run from the repository root.
      *> A case prints only if it FAILS; one result block at the end
      *> lists every case, the timings and the tally.
      *> ============================================================
       PROGRAM-ID. TEST-AUDIO.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-DIR              PIC X(200) VALUE SPACES.
       01  WS-TONE             PIC X(240) VALUE SPACES.
       01  WS-NOT-SOUND        PIC X(240) VALUE SPACES.
       01  WS-MISSING          PIC X(240) VALUE SPACES.
       01  WS-STATUS           PIC X(10)  VALUE SPACES.
       01  WS-STATE            PIC X(10)  VALUE SPACES.
           88 IS-IDLE                     VALUE "IDLE".
           88 IS-PLAYING                  VALUE "PLAYING".
           88 IS-PAUSED                   VALUE "PAUSED".
           88 IS-ENDED                    VALUE "ENDED".
           88 IS-ERROR                    VALUE "ERROR".
       01  WS-POS              PIC 9(7)   VALUE 0.
       01  WS-LEN              PIC 9(7)   VALUE 0.
       01  WS-HELD             PIC 9(7)   VALUE 0.
       01  WS-TARGET           PIC 9(7)   VALUE 0.
       01  WS-HOLDS            PIC X      VALUE "Y".
           88 POSITION-HELD               VALUE "Y".
       01  WS-NOW              PIC X(21).
       01  WS-NOW-MS           PIC 9(9)   VALUE 0.
       01  WS-MARK             PIC 9(9)   VALUE 0.
       01  WS-ELAPSED          PIC S9(9)  VALUE 0.
       01  WS-PLAY-MARK        PIC 9(9)   VALUE 0.
       01  WS-ANSWER-MS        PIC 9(6)   VALUE 0.
       01  WS-REACH-MS         PIC 9(6)   VALUE 0.
       01  WS-WINDOW-MS        PIC 9(6)   VALUE 0.
       01  WS-WALL-MS          PIC 9(6)   VALUE 0.
       01  WS-POLLS            PIC 9(9)   VALUE 0.
       01  WS-LABEL            PIC X(48)  VALUE SPACES.
       01  WS-I                PIC 99     VALUE 0.
       01  WS-N                PIC 99     VALUE 0.
       01  WS-TOT-OK           PIC 99     VALUE 0.
       01  WS-TOT-BAD          PIC 99     VALUE 0.
       01  WS-CASES.
           05 WS-CASE OCCURS 16 TIMES.
              10 WS-CASE-MARK  PIC X(4).
              10 WS-CASE-NAME  PIC X(48).
       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT WS-DIR FROM ENVIRONMENT "AUDIO_TEST_DIR".
           IF WS-DIR = SPACES
               MOVE "tests/cobol/audio" TO WS-DIR
           END-IF.
           STRING WS-DIR::Trim() DELIMITED BY SIZE
                  "/tone-2s.wav" DELIMITED BY SIZE
               INTO WS-TONE.
           STRING WS-DIR::Trim() DELIMITED BY SIZE
                  "/test-audio.cbl" DELIMITED BY SIZE
               INTO WS-NOT-SOUND.
           STRING WS-DIR::Trim() DELIMITED BY SIZE
                  "/no-such-sound.mp3" DELIMITED BY SIZE
               INTO WS-MISSING.

      *> ---- what a file can be told --------------------------------
           MOVE "PLAY a missing file -> NOFILE" TO WS-LABEL.
           COBOL::"AUDIO-PLAY" ( WS-MISSING WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "NOFILE"
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "  ...and AUDIO-STATE reads ERROR" TO WS-LABEL.
           IF IS-ERROR
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "PLAY a file that is not sound -> BADFORMAT"
               TO WS-LABEL.
           COBOL::"AUDIO-PLAY" ( WS-NOT-SOUND WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "BADFORMAT" AND IS-ERROR
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

      *> ---- commands with nothing playing --------------------------
           MOVE "PAUSE with nothing playing -> ERROR" TO WS-LABEL.
           COBOL::"AUDIO-PAUSE" ( WS-STATUS ).
           IF WS-STATUS = "ERROR"
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "RESUME with nothing playing -> ERROR" TO WS-LABEL.
           COBOL::"AUDIO-RESUME" ( WS-STATUS ).
           IF WS-STATUS = "ERROR"
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "STOP -> OK, IDLE at 0 of 0 ms" TO WS-LABEL.
           COBOL::"AUDIO-STOP" ( WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "OK" AND IS-IDLE AND WS-POS = 0
                   AND WS-LEN = 0
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

      *> ---- a two-second sound, paused and resumed -----------------
           MOVE "PLAY tone-2s.wav -> OK, PLAYING" TO WS-LABEL.
           PERFORM NOW.
           MOVE WS-NOW-MS TO WS-MARK.
           COBOL::"AUDIO-PLAY" ( WS-TONE WS-STATUS ).
           PERFORM SINCE-MARK.
           MOVE WS-ELAPSED TO WS-ANSWER-MS.
      *>   The sound starts when PLAY answers: on a real device, opening
      *>   it is part of the answer, not of the playing.
           MOVE WS-NOW-MS TO WS-PLAY-MARK.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "OK" AND IS-PLAYING
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "the length is known at once: 2000 ms" TO WS-LABEL.
           IF WS-LEN = 2000
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

           MOVE "the position advances to 500 ms" TO WS-LABEL.
           MOVE 500 TO WS-TARGET.
           PERFORM WAIT-FOR-POSITION.
           MOVE WS-ELAPSED TO WS-REACH-MS.
           IF WS-POS >= 500 AND IS-PLAYING
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

           MOVE "PAUSE -> OK, PAUSED" TO WS-LABEL.
           COBOL::"AUDIO-PAUSE" ( WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-HELD WS-LEN ).
           IF WS-STATUS = "OK" AND IS-PAUSED
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "the position holds for 300 ms while paused"
               TO WS-LABEL.
           PERFORM WATCH-THE-PAUSE.
           MOVE WS-ELAPSED TO WS-WINDOW-MS.
           IF POSITION-HELD
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

           MOVE "RESUME -> OK, the position moves on" TO WS-LABEL.
           COBOL::"AUDIO-RESUME" ( WS-STATUS ).
           COMPUTE WS-TARGET = WS-HELD + 100.
           PERFORM WAIT-FOR-POSITION.
           IF WS-STATUS = "OK" AND IS-PLAYING AND WS-POS > WS-HELD
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

           MOVE "ENDED at the length: 2000 of 2000 ms" TO WS-LABEL.
           PERFORM WAIT-FOR-END.
           PERFORM NOW.
           COMPUTE WS-ELAPSED = WS-NOW-MS - WS-PLAY-MARK.
           IF WS-ELAPSED < 0
               ADD 86400000 TO WS-ELAPSED
           END-IF.
           MOVE WS-ELAPSED TO WS-WALL-MS.
           IF IS-ENDED AND WS-POS = 2000 AND WS-LEN = 2000
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "wall time = 2000 ms of sound + the pause"
               TO WS-LABEL.
           IF WS-WALL-MS >= 2000 + WS-WINDOW-MS - 60
                   AND WS-WALL-MS <= 2000 + WS-WINDOW-MS + 1500
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

      *> ---- replacing a sound, and stopping it ---------------------
           MOVE "PLAY over a playing sound replaces it" TO WS-LABEL.
           COBOL::"AUDIO-PLAY" ( WS-TONE WS-STATUS ).
           MOVE 300 TO WS-TARGET.
           PERFORM WAIT-FOR-POSITION.
           COBOL::"AUDIO-PLAY" ( WS-TONE WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "OK" AND IS-PLAYING AND WS-POS < 300
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.
           MOVE "STOP a playing sound -> OK, IDLE at 0 of 0 ms"
               TO WS-LABEL.
           COBOL::"AUDIO-STOP" ( WS-STATUS ).
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           IF WS-STATUS = "OK" AND IS-IDLE AND WS-POS = 0
                   AND WS-LEN = 0
               PERFORM PASSED
           ELSE
               PERFORM FAILED
           END-IF.

           PERFORM SHOW-RESULTS.
           STOP RUN.

      *> Poll until the sound reaches WS-TARGET ms, stops playing, or
      *> five seconds pass. WS-ELAPSED is how long it took.
       WAIT-FOR-POSITION.
           PERFORM NOW.
           MOVE WS-NOW-MS TO WS-MARK.
           MOVE 0 TO WS-ELAPSED.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           PERFORM UNTIL WS-POS >= WS-TARGET OR NOT IS-PLAYING
                   OR WS-ELAPSED > 5000
               COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN )
               ADD 1 TO WS-POLLS
               PERFORM SINCE-MARK
           END-PERFORM.

      *> Poll until the sound is no longer playing, or ten seconds pass.
       WAIT-FOR-END.
           PERFORM NOW.
           MOVE WS-NOW-MS TO WS-MARK.
           MOVE 0 TO WS-ELAPSED.
           COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN ).
           PERFORM UNTIL NOT IS-PLAYING OR WS-ELAPSED > 10000
               COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN )
               ADD 1 TO WS-POLLS
               PERFORM SINCE-MARK
           END-PERFORM.

      *> Watch a paused sound for 300 ms: it must stay PAUSED at
      *> WS-HELD the whole time.
       WATCH-THE-PAUSE.
           PERFORM NOW.
           MOVE WS-NOW-MS TO WS-MARK.
           MOVE 0 TO WS-ELAPSED.
           MOVE "Y" TO WS-HOLDS.
           PERFORM UNTIL WS-ELAPSED >= 300
               COBOL::"AUDIO-STATE" ( WS-STATE WS-POS WS-LEN )
               ADD 1 TO WS-POLLS
               IF WS-POS NOT = WS-HELD OR NOT IS-PAUSED
                   MOVE "N" TO WS-HOLDS
               END-IF
               PERFORM SINCE-MARK
           END-PERFORM.

      *> The time of day in milliseconds (CURRENT-DATE counts in
      *> hundredths, so every timing here is good to 10 ms).
       NOW.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW.
           COMPUTE WS-NOW-MS =
               ((FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                 + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                 + FUNCTION NUMVAL(WS-NOW(13:2))) * 100
                 + FUNCTION NUMVAL(WS-NOW(15:2))) * 10.

      *> Milliseconds since WS-MARK, across midnight too.
       SINCE-MARK.
           PERFORM NOW.
           COMPUTE WS-ELAPSED = WS-NOW-MS - WS-MARK.
           IF WS-ELAPSED < 0
               ADD 86400000 TO WS-ELAPSED
           END-IF.

       PASSED.
           ADD 1 TO WS-N.
           ADD 1 TO WS-TOT-OK.
           MOVE "PASS" TO WS-CASE-MARK (WS-N).
           MOVE WS-LABEL TO WS-CASE-NAME (WS-N).

       FAILED.
           ADD 1 TO WS-N.
           ADD 1 TO WS-TOT-BAD.
           MOVE "FAIL" TO WS-CASE-MARK (WS-N).
           MOVE WS-LABEL TO WS-CASE-NAME (WS-N).
           DISPLAY "FAIL " WS-LABEL.
           DISPLAY "  status [" WS-STATUS "] state [" WS-STATE
               "] position " WS-POS " of " WS-LEN " ms".
           IF WS-STATUS = "NODEVICE"
               DISPLAY "  no output device: run with COBOLT_AUDIO=null"
           END-IF.

       SHOW-RESULTS.
           DISPLAY "==================================================".
           DISPLAY "TEST-AUDIO - the sound built-ins, null output".
           DISPLAY "Forms: COBOL::AUDIO-PLAY ( file status )".
           DISPLAY "       COBOL::AUDIO-PAUSE ( status )".
           DISPLAY "       COBOL::AUDIO-RESUME ( status )".
           DISPLAY "       COBOL::AUDIO-STOP ( status )".
           DISPLAY "       COBOL::AUDIO-STATE ( state pos-ms len-ms )".
           DISPLAY "--------------------------------------------------".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               DISPLAY "  " WS-CASE-MARK (WS-I) "  " WS-CASE-NAME (WS-I)
           END-PERFORM.
           DISPLAY "--------------------------------------------------".
           DISPLAY "Sound played        : tone-2s.wav, 2000 ms".
           DISPLAY "PLAY answered in    : " WS-ANSWER-MS " ms".
           DISPLAY "Reached 500 ms after: " WS-REACH-MS " ms".
           DISPLAY "Paused and held for : " WS-WINDOW-MS " ms at "
               WS-HELD " ms".
           DISPLAY "Playing to ENDED    : " WS-WALL-MS " ms (wall)".
           DISPLAY "AUDIO-STATE polls   : " WS-POLLS.
           DISPLAY "--------------------------------------------------".
           DISPLAY "CASES RUN           : " WS-N.
           DISPLAY "CASES PASSED        : " WS-TOT-OK.
           DISPLAY "CASES FAILED        : " WS-TOT-BAD.
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT      : PASS"
           ELSE
               DISPLAY "OVERALL RESULT      : FAIL"
           END-IF.
           DISPLAY "==================================================".

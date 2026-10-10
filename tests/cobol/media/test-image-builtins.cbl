       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-IMAGE-BUILTINS
      *> The picture and download built-ins of spec 092:
      *>   COBOL::"IMAGE-INFO"    ( path width height format status )
      *>   COBOL::"IMAGE-FIT"     ( source target width height status )
      *>   COBOL::"HTTP-DOWNLOAD" ( url file http-status status )
      *> The sources are two pictures the repository already ships:
      *>   assets/images/bg1.jpg            1360 x 752 JPEG
      *>   assets/images/chibi-head-256.png  256 x 214 PNG, transparent
      *> Every fitted copy is measured back with IMAGE-INFO. Outputs
      *> go to /tmp/prc-media/. Run it from the repository root:
      *>   rcrun run tests/cobol/media/test-image-builtins.cbl
      *> HTTP-DOWNLOAD is exercised here only on the answers that need
      *> no server (a bad address, a refused connection); the exact
      *> bytes, redirects, 404, size cap and timeout run against a local
      *> test server in crates/cobolt-runtime/tests/test_http_download.rs
      *> Self-checking: one result block at the end, nothing before it.
      *> ============================================================
       PROGRAM-ID. TEST-IMAGE-BUILTINS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-JPEG          PIC X(200) VALUE "assets/images/bg1.jpg".
       01  WS-PNG           PIC X(200)
                            VALUE "assets/images/chibi-head-256.png".
       01  WS-WIDTH         PIC 9(6)   VALUE 0.
       01  WS-HEIGHT        PIC 9(6)   VALUE 0.
       01  WS-FORMAT        PIC X(8)   VALUE SPACES.
       01  WS-STATUS        PIC X(12)  VALUE SPACES.
       01  WS-MEASURED      PIC X(12)  VALUE SPACES.
       01  WS-HTTP-CODE     PIC 9(3)   VALUE 0.
      *> What the case in hand expects.
       01  WS-LABEL         PIC X(44)  VALUE SPACES.
       01  WS-EXP-STATUS    PIC X(12)  VALUE SPACES.
       01  WS-EXP-W         PIC 9(6)   VALUE 0.
       01  WS-EXP-H         PIC 9(6)   VALUE 0.
       01  WS-EXP-FORMAT    PIC X(8)   VALUE SPACES.
      *> The clock: FUNCTION CURRENT-DATE, hundredths of a second.
       01  WS-NOW.
           05 FILLER        PIC X(8).
           05 WS-NOW-HH     PIC 99.
           05 WS-NOW-MM     PIC 99.
           05 WS-NOW-SS     PIC 99.
           05 WS-NOW-CC     PIC 99.
           05 FILLER        PIC X(5).
       01  WS-T0            PIC 9(9)   VALUE 0.
       01  WS-T1            PIC 9(9)   VALUE 0.
       01  WS-MS            PIC 9(7)   VALUE 0.
       01  WS-TOTAL-MS      PIC 9(7)   VALUE 0.
      *> One row per case, printed at the end.
       01  WS-N             PIC 99     VALUE 0.
       01  WS-I             PIC 99     VALUE 0.
       01  WS-PASS          PIC 99     VALUE 0.
       01  WS-FAIL          PIC 99     VALUE 0.
       01  WS-CASES.
           05 WS-CASE OCCURS 20 TIMES.
              10 WS-C-NAME   PIC X(44).
              10 WS-C-STATUS PIC X(10).
              10 WS-C-W      PIC Z(5)9.
              10 WS-C-H      PIC Z(5)9.
              10 WS-C-FORMAT PIC X(5).
              10 WS-C-MS     PIC Z(5)9.
              10 WS-C-OK     PIC X(4).
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM INFO-CASES.
           PERFORM FIT-CASES.
           PERFORM REFUSAL-CASES.
           PERFORM DOWNLOAD-CASES.
           PERFORM SHOW-RESULTS.
           STOP RUN.

      *> ── IMAGE-INFO: the header alone ─────────────────────────────
       INFO-CASES.
           MOVE "IMAGE-INFO bg1.jpg" TO WS-LABEL.
           MOVE "OK" TO WS-EXP-STATUS.
           MOVE 1360 TO WS-EXP-W.
           MOVE 752 TO WS-EXP-H.
           MOVE "JPEG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-INFO" ( WS-JPEG WS-WIDTH WS-HEIGHT WS-FORMAT
                                 WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-INFO chibi-head-256.png" TO WS-LABEL.
           MOVE 256 TO WS-EXP-W.
           MOVE 214 TO WS-EXP-H.
           MOVE "PNG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-INFO" ( WS-PNG WS-WIDTH WS-HEIGHT WS-FORMAT
                                 WS-STATUS ).
           PERFORM RECORD-CASE.

      *> ── IMAGE-FIT: crop to fill, exact size, PNG or JPEG ─────────
       FIT-CASES.
           MOVE "IMAGE-FIT bg1.jpg -> 400x400 .png" TO WS-LABEL.
           MOVE "OK" TO WS-EXP-STATUS.
           MOVE 400 TO WS-EXP-W.
           MOVE 400 TO WS-EXP-H.
           MOVE "PNG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-JPEG "/tmp/prc-media/bg1-400.png"
                                400 400 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/bg1-400.png"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-MEASURED ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT bg1.jpg -> 40x40 .jpg" TO WS-LABEL.
           MOVE 40 TO WS-EXP-W.
           MOVE 40 TO WS-EXP-H.
           MOVE "JPEG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-JPEG "/tmp/prc-media/bg1-40.jpg"
                                40 40 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/bg1-40.jpg"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-MEASURED ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT bg1.jpg -> 1360x752 (no crop)" TO WS-LABEL.
           MOVE 1360 TO WS-EXP-W.
           MOVE 752 TO WS-EXP-H.
           MOVE "JPEG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-JPEG "/tmp/prc-media/bg1-same.jpeg"
                                1360 752 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/bg1-same.jpeg"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-MEASURED ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT chibi.png -> 120x80 .jpg" TO WS-LABEL.
           MOVE 120 TO WS-EXP-W.
           MOVE 80 TO WS-EXP-H.
           MOVE "JPEG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-PNG "/tmp/prc-media/chibi-120x80.jpg"
                                120 80 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/chibi-120x80.jpg"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-MEASURED ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT chibi.png -> 64x64 .PNG" TO WS-LABEL.
           MOVE 64 TO WS-EXP-W.
           MOVE 64 TO WS-EXP-H.
           MOVE "PNG" TO WS-EXP-FORMAT.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-PNG "/tmp/prc-media/chibi-64.PNG"
                                64 64 WS-STATUS ).
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/chibi-64.PNG"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-MEASURED ).
           PERFORM RECORD-CASE.

      *> ── The refusals: each answers its word and writes nothing ───
       REFUSAL-CASES.
           MOVE 0 TO WS-EXP-W WS-EXP-H.
           MOVE SPACES TO WS-EXP-FORMAT.
           COBOL::"WRITE-FILE" ( "/tmp/prc-media/not-a-picture.png"
                                 "plain text, not a picture" ).

           MOVE "IMAGE-FIT missing source" TO WS-LABEL.
           MOVE "NOFILE" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( "/tmp/prc-media/no-such.jpg"
                "/tmp/prc-media/r1.png" 40 40 WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT text named .png" TO WS-LABEL.
           MOVE "BADIMAGE" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( "/tmp/prc-media/not-a-picture.png"
                "/tmp/prc-media/r2.png" 40 40 WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT target .gif" TO WS-LABEL.
           MOVE "BADEXT" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-JPEG "/tmp/prc-media/r3.gif"
                                40 40 WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-FIT width 0" TO WS-LABEL.
           MOVE "BADSIZE" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-FIT" ( WS-JPEG "/tmp/prc-media/r4.png"
                                0 40 WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-INFO missing file" TO WS-LABEL.
           MOVE "NOFILE" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/no-such.jpg"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "IMAGE-INFO text named .png" TO WS-LABEL.
           MOVE "BADIMAGE" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"IMAGE-INFO" ( "/tmp/prc-media/not-a-picture.png"
                WS-WIDTH WS-HEIGHT WS-FORMAT WS-STATUS ).
           PERFORM RECORD-CASE.

      *> ── HTTP-DOWNLOAD, the answers that need no server ───────────
       DOWNLOAD-CASES.
           MOVE "HTTP-DOWNLOAD ftp:// address" TO WS-LABEL.
           MOVE "BADURL" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"HTTP-DOWNLOAD" ( "ftp://example.com/a.bin"
                "/tmp/prc-media/d1.bin" WS-HTTP-CODE WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "HTTP-DOWNLOAD not an address" TO WS-LABEL.
           PERFORM START-CLOCK.
           COBOL::"HTTP-DOWNLOAD" ( "not an address"
                "/tmp/prc-media/d2.bin" WS-HTTP-CODE WS-STATUS ).
           PERFORM RECORD-CASE.

           MOVE "HTTP-DOWNLOAD refused connection" TO WS-LABEL.
           MOVE "NET" TO WS-EXP-STATUS.
           PERFORM START-CLOCK.
           COBOL::"HTTP-DOWNLOAD" ( "http://127.0.0.1:1/a.bin"
                "/tmp/prc-media/d3.bin" WS-HTTP-CODE WS-STATUS ).
           PERFORM RECORD-CASE.

      *> ── The harness ─────────────────────────────────────────────
       START-CLOCK.
           MOVE 0 TO WS-WIDTH WS-HEIGHT.
           MOVE SPACES TO WS-FORMAT WS-STATUS WS-MEASURED.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW.
           COMPUTE WS-T0 = ((WS-NOW-HH * 60 + WS-NOW-MM) * 60
                           + WS-NOW-SS) * 100 + WS-NOW-CC.

      *> A fit is judged by its own status AND by what IMAGE-INFO
      *> measures in the file it wrote; a refusal by its word alone.
       RECORD-CASE.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW.
           COMPUTE WS-T1 = ((WS-NOW-HH * 60 + WS-NOW-MM) * 60
                           + WS-NOW-SS) * 100 + WS-NOW-CC.
           COMPUTE WS-MS = (WS-T1 - WS-T0) * 10.
           ADD WS-MS TO WS-TOTAL-MS.
           ADD 1 TO WS-N.
           MOVE WS-LABEL TO WS-C-NAME (WS-N).
           MOVE WS-STATUS TO WS-C-STATUS (WS-N).
           MOVE WS-WIDTH TO WS-C-W (WS-N).
           MOVE WS-HEIGHT TO WS-C-H (WS-N).
           MOVE WS-FORMAT TO WS-C-FORMAT (WS-N).
           MOVE WS-MS TO WS-C-MS (WS-N).
           MOVE "FAIL" TO WS-C-OK (WS-N).
           IF WS-STATUS = WS-EXP-STATUS
               IF WS-EXP-STATUS NOT = "OK"
                   MOVE "PASS" TO WS-C-OK (WS-N)
               ELSE
                   IF WS-WIDTH = WS-EXP-W
                       IF WS-HEIGHT = WS-EXP-H
                           IF WS-FORMAT = WS-EXP-FORMAT
                               MOVE "PASS" TO WS-C-OK (WS-N)
                           END-IF
                       END-IF
                   END-IF
               END-IF
           END-IF.
           IF WS-C-OK (WS-N) = "PASS"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
           END-IF.

       SHOW-RESULTS.
           DISPLAY "==================================================="
                   "=====================".
           DISPLAY "TEST-IMAGE-BUILTINS".
           DISPLAY "IMAGE-INFO, IMAGE-FIT (measured back with "
                   "IMAGE-INFO) and HTTP-DOWNLOAD".
           DISPLAY "---------------------------------------------------"
                   "---------------------".
           DISPLAY "CASE                                         "
                   "STATUS     WIDTH HEIGHT FMT     MS RESULT".
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               DISPLAY WS-C-NAME (WS-I) " " WS-C-STATUS (WS-I)
                       WS-C-W (WS-I) " " WS-C-H (WS-I) " "
                       WS-C-FORMAT (WS-I) WS-C-MS (WS-I) " "
                       WS-C-OK (WS-I)
           END-PERFORM.
           DISPLAY "---------------------------------------------------"
                   "---------------------".
           DISPLAY "CASES RUN       : " WS-N.
           DISPLAY "CASES PASSED    : " WS-PASS.
           DISPLAY "CASES FAILED    : " WS-FAIL.
           DISPLAY "TOTAL TIME (ms) : " WS-TOTAL-MS
                   " (clock resolution 10 ms)".
           IF WS-FAIL = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "==================================================="
                   "=====================".

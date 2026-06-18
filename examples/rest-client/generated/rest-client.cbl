      *> ───────────────────────────────────────────────────────────
      *>  This code was generated automatically by PowerRustCOBOL RAD.
      *>
      *>  DO NOT MODIFY IT DIRECTLY: it is regenerated the next time
      *>  you interact with the Form Designer, so manual edits are lost.
      *>  Edit the form and its event handlers in the Form Designer
      *>  instead.
      *>
      *>  PowerRustCOBOL may change the structure of this generated code
      *>  at any time — without breaking your code's functionality — for
      *>  reasons such as performance improvements, new observability
      *>  features, and bug fixes.
      *>
      *>  PowerRustCOBOL and its components are distributed under the
      *>  Apache 2.0 License.
      *> ───────────────────────────────────────────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTFORM.

       ENVIRONMENT DIVISION.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'TESTFORM'.

      *>── REST / HTTP runtime variables ──────────────────────────────
      *>   Usage:
      *>     MOVE 'https://api.example.com/resource' TO WS-REQUEST-URL
      *>     PERFORM RST1-GET
      *>     IF WS-HTTP-STATUS = 200
      *>         DISPLAY WS-HTTP-RESPONSE
      *>     END-IF
       01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.
       01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.
       01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.
       01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.
       01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.
       01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.
       01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.
       01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.

      *>── REST client: SUBJ ──────────────────────────────────
       01 WS-SUBJ-BASE-URL      PIC X(2048) VALUE 'x'.

      *>── Form controls ───────────────────────────────────────────────
       01 WS-SUBJ.
          05 WS-SUBJ-TEXT       PIC X(256) VALUE 'SUBJ'.
          05 WS-SUBJ-VISIBLE    PIC 9      VALUE 1.
          05 WS-SUBJ-ENABLED    PIC 9      VALUE 1.

       01 WS-B00.
          05 WS-B00-TEXT       PIC X(256) VALUE 'BackgroundColor'.
          05 WS-B00-VISIBLE    PIC 9      VALUE 1.
          05 WS-B00-ENABLED    PIC 9      VALUE 1.

       01 WS-B01.
          05 WS-B01-TEXT       PIC X(256) VALUE 'ForegroundColor'.
          05 WS-B01-VISIBLE    PIC 9      VALUE 1.
          05 WS-B01-ENABLED    PIC 9      VALUE 1.

       01 WS-B02.
          05 WS-B02-TEXT       PIC X(256) VALUE 'FontName'.
          05 WS-B02-VISIBLE    PIC 9      VALUE 1.
          05 WS-B02-ENABLED    PIC 9      VALUE 1.

       01 WS-B03.
          05 WS-B03-TEXT       PIC X(256) VALUE 'FontSize'.
          05 WS-B03-VISIBLE    PIC 9      VALUE 1.
          05 WS-B03-ENABLED    PIC 9      VALUE 1.

       01 WS-B04.
          05 WS-B04-TEXT       PIC X(256) VALUE 'Bold'.
          05 WS-B04-VISIBLE    PIC 9      VALUE 1.
          05 WS-B04-ENABLED    PIC 9      VALUE 1.

       01 WS-B05.
          05 WS-B05-TEXT       PIC X(256) VALUE 'Italic'.
          05 WS-B05-VISIBLE    PIC 9      VALUE 1.
          05 WS-B05-ENABLED    PIC 9      VALUE 1.

       01 WS-B06.
          05 WS-B06-TEXT       PIC X(256) VALUE 'Underline'.
          05 WS-B06-VISIBLE    PIC 9      VALUE 1.
          05 WS-B06-ENABLED    PIC 9      VALUE 1.

       01 WS-B07.
          05 WS-B07-TEXT       PIC X(256) VALUE 'Strikethrough'.
          05 WS-B07-VISIBLE    PIC 9      VALUE 1.
          05 WS-B07-ENABLED    PIC 9      VALUE 1.

       01 WS-B08.
          05 WS-B08-TEXT       PIC X(256) VALUE 'Tooltip'.
          05 WS-B08-VISIBLE    PIC 9      VALUE 1.
          05 WS-B08-ENABLED    PIC 9      VALUE 1.

       01 WS-B09.
          05 WS-B09-TEXT       PIC X(256) VALUE 'Cursor'.
          05 WS-B09-VISIBLE    PIC 9      VALUE 1.
          05 WS-B09-ENABLED    PIC 9      VALUE 1.

       01 WS-B10.
          05 WS-B10-TEXT       PIC X(256) VALUE 'Dock'.
          05 WS-B10-VISIBLE    PIC 9      VALUE 1.
          05 WS-B10-ENABLED    PIC 9      VALUE 1.

       01 WS-B11.
          05 WS-B11-TEXT       PIC X(256) VALUE 'Anchor'.
          05 WS-B11-VISIBLE    PIC 9      VALUE 1.
          05 WS-B11-ENABLED    PIC 9      VALUE 1.

       01 WS-B12.
          05 WS-B12-TEXT       PIC X(256) VALUE 'Padding'.
          05 WS-B12-VISIBLE    PIC 9      VALUE 1.
          05 WS-B12-ENABLED    PIC 9      VALUE 1.

       01 WS-B13.
          05 WS-B13-TEXT       PIC X(256) VALUE 'Opacity'.
          05 WS-B13-VISIBLE    PIC 9      VALUE 1.
          05 WS-B13-ENABLED    PIC 9      VALUE 1.

       01 WS-B14.
          05 WS-B14-TEXT       PIC X(256) VALUE 'ShadowEnabled'.
          05 WS-B14-VISIBLE    PIC 9      VALUE 1.
          05 WS-B14-ENABLED    PIC 9      VALUE 1.

       01 WS-B15.
          05 WS-B15-TEXT       PIC X(256) VALUE 'ShadowOpacity'.
          05 WS-B15-VISIBLE    PIC 9      VALUE 1.
          05 WS-B15-ENABLED    PIC 9      VALUE 1.

       01 WS-B16.
          05 WS-B16-TEXT       PIC X(256) VALUE 'ShadowColor'.
          05 WS-B16-VISIBLE    PIC 9      VALUE 1.
          05 WS-B16-ENABLED    PIC 9      VALUE 1.

       01 WS-B17.
          05 WS-B17-TEXT       PIC X(256) VALUE 'ShadowDirection'.
          05 WS-B17-VISIBLE    PIC 9      VALUE 1.
          05 WS-B17-ENABLED    PIC 9      VALUE 1.

       01 WS-B18.
          05 WS-B18-TEXT       PIC X(256) VALUE 'ShadowDistance'.
          05 WS-B18-VISIBLE    PIC 9      VALUE 1.
          05 WS-B18-ENABLED    PIC 9      VALUE 1.

       01 WS-B19.
          05 WS-B19-TEXT       PIC X(256) VALUE 'ShadowBlur'.
          05 WS-B19-VISIBLE    PIC 9      VALUE 1.
          05 WS-B19-ENABLED    PIC 9      VALUE 1.

       01 WS-B20.
          05 WS-B20-TEXT       PIC X(256) VALUE 'ShadowBlurStrength'.
          05 WS-B20-VISIBLE    PIC 9      VALUE 1.
          05 WS-B20-ENABLED    PIC 9      VALUE 1.

       01 WS-B21.
          05 WS-B21-TEXT       PIC X(256) VALUE 'ZOrder'.
          05 WS-B21-VISIBLE    PIC 9      VALUE 1.
          05 WS-B21-ENABLED    PIC 9      VALUE 1.

       01 WS-B22.
          05 WS-B22-TEXT       PIC X(256) VALUE 'LabelFor'.
          05 WS-B22-VISIBLE    PIC 9      VALUE 1.
          05 WS-B22-ENABLED    PIC 9      VALUE 1.

       01 WS-B23.
          05 WS-B23-TEXT       PIC X(256) VALUE 'DataItem'.
          05 WS-B23-VISIBLE    PIC 9      VALUE 1.
          05 WS-B23-ENABLED    PIC 9      VALUE 1.

       01 WS-B24.
          05 WS-B24-TEXT       PIC X(256) VALUE 'DataFormat'.
          05 WS-B24-VISIBLE    PIC 9      VALUE 1.
          05 WS-B24-ENABLED    PIC 9      VALUE 1.

       01 WS-B25.
          05 WS-B25-TEXT       PIC X(256) VALUE 'BaseURL'.
          05 WS-B25-VISIBLE    PIC 9      VALUE 1.
          05 WS-B25-ENABLED    PIC 9      VALUE 1.

       01 WS-B26.
          05 WS-B26-TEXT       PIC X(256) VALUE 'DefaultMethod'.
          05 WS-B26-VISIBLE    PIC 9      VALUE 1.
          05 WS-B26-ENABLED    PIC 9      VALUE 1.

       01 WS-B27.
          05 WS-B27-TEXT       PIC X(256) VALUE 'AuthType'.
          05 WS-B27-VISIBLE    PIC 9      VALUE 1.
          05 WS-B27-ENABLED    PIC 9      VALUE 1.

       01 WS-B28.
          05 WS-B28-TEXT       PIC X(256) VALUE 'AuthToken'.
          05 WS-B28-VISIBLE    PIC 9      VALUE 1.
          05 WS-B28-ENABLED    PIC 9      VALUE 1.

       01 WS-B29.
          05 WS-B29-TEXT       PIC X(256) VALUE 'DefaultHeaders'.
          05 WS-B29-VISIBLE    PIC 9      VALUE 1.
          05 WS-B29-ENABLED    PIC 9      VALUE 1.

       01 WS-B30.
          05 WS-B30-TEXT       PIC X(256) VALUE 'TimeoutSeconds'.
          05 WS-B30-VISIBLE    PIC 9      VALUE 1.
          05 WS-B30-ENABLED    PIC 9      VALUE 1.

       01 WS-B31.
          05 WS-B31-TEXT       PIC X(256) VALUE 'FollowRedirects'.
          05 WS-B31-VISIBLE    PIC 9      VALUE 1.
          05 WS-B31-ENABLED    PIC 9      VALUE 1.

       01 WS-B32.
          05 WS-B32-TEXT       PIC X(256) VALUE 'VerifyTLS'.
          05 WS-B32-VISIBLE    PIC 9      VALUE 1.
          05 WS-B32-ENABLED    PIC 9      VALUE 1.

       01 WS-B33.
          05 WS-B33-TEXT       PIC X(256) VALUE 'RequestDataItem'.
          05 WS-B33-VISIBLE    PIC 9      VALUE 1.
          05 WS-B33-ENABLED    PIC 9      VALUE 1.

       01 WS-B34.
          05 WS-B34-TEXT       PIC X(256) VALUE 'ResponseDataItem'.
          05 WS-B34-VISIBLE    PIC 9      VALUE 1.
          05 WS-B34-ENABLED    PIC 9      VALUE 1.

       01 WS-B35.
          05 WS-B35-TEXT       PIC X(256) VALUE 'StatusDataItem'.
          05 WS-B35-VISIBLE    PIC 9      VALUE 1.
          05 WS-B35-ENABLED    PIC 9      VALUE 1.

       01 WS-B36.
          05 WS-B36-TEXT       PIC X(256) VALUE 'ResponseParagraph'.
          05 WS-B36-VISIBLE    PIC 9      VALUE 1.
          05 WS-B36-ENABLED    PIC 9      VALUE 1.

       01 WS-B37.
          05 WS-B37-TEXT       PIC X(256) VALUE 'ErrorParagraph'.
          05 WS-B37-VISIBLE    PIC 9      VALUE 1.
          05 WS-B37-ENABLED    PIC 9      VALUE 1.

       01 WS-B38.
          05 WS-B38-TEXT       PIC X(256) VALUE 'X'.
          05 WS-B38-VISIBLE    PIC 9      VALUE 1.
          05 WS-B38-ENABLED    PIC 9      VALUE 1.

       01 WS-B39.
          05 WS-B39-TEXT       PIC X(256) VALUE 'Y'.
          05 WS-B39-VISIBLE    PIC 9      VALUE 1.
          05 WS-B39-ENABLED    PIC 9      VALUE 1.

       01 WS-B40.
          05 WS-B40-TEXT       PIC X(256) VALUE 'Width'.
          05 WS-B40-VISIBLE    PIC 9      VALUE 1.
          05 WS-B40-ENABLED    PIC 9      VALUE 1.

       01 WS-B41.
          05 WS-B41-TEXT       PIC X(256) VALUE 'Height'.
          05 WS-B41-VISIBLE    PIC 9      VALUE 1.
          05 WS-B41-ENABLED    PIC 9      VALUE 1.

       01 WS-B42.
          05 WS-B42-TEXT       PIC X(256) VALUE 'PlayAnimation'.
          05 WS-B42-VISIBLE    PIC 9      VALUE 1.
          05 WS-B42-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           CALL "COBOL-INIT-FORM" USING FORM-NAME
           CALL "TESTFORM--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "TESTFORM--ONCLOSE"
           STOP RUN.

       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-CONTROL-ID
                   WHEN "SUBJ"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onResponseReceived"
                               CALL "SUBJ--ONRESPONSERECEIVED"
                           WHEN "onError"
                               CALL "SUBJ--ONERROR"
                           WHEN "onTimeout"
                               CALL "SUBJ--ONTIMEOUT"
                           WHEN "onProgress"
                               CALL "SUBJ--ONPROGRESS"
                       END-EVALUATE
                   WHEN "B00"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B00--ONCLICK"
                       END-EVALUATE
                   WHEN "B01"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B01--ONCLICK"
                       END-EVALUATE
                   WHEN "B02"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B02--ONCLICK"
                       END-EVALUATE
                   WHEN "B03"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B03--ONCLICK"
                       END-EVALUATE
                   WHEN "B04"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B04--ONCLICK"
                       END-EVALUATE
                   WHEN "B05"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B05--ONCLICK"
                       END-EVALUATE
                   WHEN "B06"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B06--ONCLICK"
                       END-EVALUATE
                   WHEN "B07"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B07--ONCLICK"
                       END-EVALUATE
                   WHEN "B08"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B08--ONCLICK"
                       END-EVALUATE
                   WHEN "B09"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B09--ONCLICK"
                       END-EVALUATE
                   WHEN "B10"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B10--ONCLICK"
                       END-EVALUATE
                   WHEN "B11"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B11--ONCLICK"
                       END-EVALUATE
                   WHEN "B12"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B12--ONCLICK"
                       END-EVALUATE
                   WHEN "B13"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B13--ONCLICK"
                       END-EVALUATE
                   WHEN "B14"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B14--ONCLICK"
                       END-EVALUATE
                   WHEN "B15"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B15--ONCLICK"
                       END-EVALUATE
                   WHEN "B16"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B16--ONCLICK"
                       END-EVALUATE
                   WHEN "B17"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B17--ONCLICK"
                       END-EVALUATE
                   WHEN "B18"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B18--ONCLICK"
                       END-EVALUATE
                   WHEN "B19"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B19--ONCLICK"
                       END-EVALUATE
                   WHEN "B20"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B20--ONCLICK"
                       END-EVALUATE
                   WHEN "B21"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B21--ONCLICK"
                       END-EVALUATE
                   WHEN "B22"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B22--ONCLICK"
                       END-EVALUATE
                   WHEN "B23"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B23--ONCLICK"
                       END-EVALUATE
                   WHEN "B24"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B24--ONCLICK"
                       END-EVALUATE
                   WHEN "B25"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B25--ONCLICK"
                       END-EVALUATE
                   WHEN "B26"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B26--ONCLICK"
                       END-EVALUATE
                   WHEN "B27"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B27--ONCLICK"
                       END-EVALUATE
                   WHEN "B28"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B28--ONCLICK"
                       END-EVALUATE
                   WHEN "B29"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B29--ONCLICK"
                       END-EVALUATE
                   WHEN "B30"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B30--ONCLICK"
                       END-EVALUATE
                   WHEN "B31"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B31--ONCLICK"
                       END-EVALUATE
                   WHEN "B32"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B32--ONCLICK"
                       END-EVALUATE
                   WHEN "B33"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B33--ONCLICK"
                       END-EVALUATE
                   WHEN "B34"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B34--ONCLICK"
                       END-EVALUATE
                   WHEN "B35"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B35--ONCLICK"
                       END-EVALUATE
                   WHEN "B36"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B36--ONCLICK"
                       END-EVALUATE
                   WHEN "B37"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B37--ONCLICK"
                       END-EVALUATE
                   WHEN "B38"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B38--ONCLICK"
                       END-EVALUATE
                   WHEN "B39"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B39--ONCLICK"
                       END-EVALUATE
                   WHEN "B40"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B40--ONCLICK"
                       END-EVALUATE
                   WHEN "B41"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B41--ONCLICK"
                       END-EVALUATE
                   WHEN "B42"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "B42--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.

       SUBJ-GET.
      *>    HTTP GET via SUBJ — set WS-REQUEST-URL before calling.
           CALL "COBOL-HTTP-GET"
               USING WS-REQUEST-URL
                     WS-HTTP-RESPONSE
                     WS-HTTP-STATUS
           END-CALL
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM 
               WHEN OTHER
                   PERFORM 
           END-EVALUATE.

       SUBJ-POST.
      *>    HTTP POST via SUBJ — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.
           CALL "COBOL-HTTP-POST"
               USING WS-REQUEST-URL
                     WS-REQUEST-BODY
                     WS-HTTP-RESPONSE
                     WS-HTTP-STATUS
           END-CALL
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM 
               WHEN OTHER
                   PERFORM 
           END-EVALUATE.

       SUBJ-PUT.
      *>    HTTP PUT via SUBJ — set WS-REQUEST-URL and WS-REQUEST-BODY before calling.
           CALL "COBOL-HTTP-PUT"
               USING WS-REQUEST-URL
                     WS-REQUEST-BODY
                     WS-HTTP-RESPONSE
                     WS-HTTP-STATUS
           END-CALL
           EVALUATE TRUE
               WHEN WS-HTTP-STATUS >= 200
                AND WS-HTTP-STATUS <= 299
                   PERFORM 
               WHEN OTHER
                   PERFORM 
           END-EVALUATE.

       .
      *>    TODO: SUBJ response handler — WS-HTTP-RESPONSE contains the body, WS-HTTP-STATUS the code
           CONTINUE.

       .
      *>    TODO: SUBJ error handler — WS-HTTP-STATUS contains the error code (0 = network failure)
           CONTINUE.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTFORM--ONLOAD.

      *>    TODO: Form onLoad handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TESTFORM--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. TESTFORM--ONCLOSE.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM TESTFORM--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBJ--ONRESPONSERECEIVED.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           DISPLAY "onResponseReceived working".

           GOBACK.

       END PROGRAM SUBJ--ONRESPONSERECEIVED.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBJ--ONERROR.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           DISPLAY "onError working".

           GOBACK.

       END PROGRAM SUBJ--ONERROR.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBJ--ONTIMEOUT.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           DISPLAY "onTimeout working".

           GOBACK.

       END PROGRAM SUBJ--ONTIMEOUT.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBJ--ONPROGRESS.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           DISPLAY "onProgress working".

           GOBACK.

       END PROGRAM SUBJ--ONPROGRESS.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B00--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "BackgroundColor" "#0066CC"
           DISPLAY SUBJ::GetProperty("BackgroundColor").

           GOBACK.

       END PROGRAM B00--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B01--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ForegroundColor" "#CC3300"
           DISPLAY SUBJ::GetProperty("ForegroundColor").

           GOBACK.

       END PROGRAM B01--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B02--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "FontName" "TEST"
           DISPLAY SUBJ::GetProperty("FontName").

           GOBACK.

       END PROGRAM B02--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B03--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "FontSize" "20"
           DISPLAY SUBJ::GetProperty("FontSize").

           GOBACK.

       END PROGRAM B03--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B04--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Bold" "1"
           DISPLAY SUBJ::GetProperty("Bold").

           GOBACK.

       END PROGRAM B04--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B05--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Italic" "1"
           DISPLAY SUBJ::GetProperty("Italic").

           GOBACK.

       END PROGRAM B05--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B06--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Underline" "1"
           DISPLAY SUBJ::GetProperty("Underline").

           GOBACK.

       END PROGRAM B06--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B07--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Strikethrough" "1"
           DISPLAY SUBJ::GetProperty("Strikethrough").

           GOBACK.

       END PROGRAM B07--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B08--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Tooltip" "TEST"
           DISPLAY SUBJ::GetProperty("Tooltip").

           GOBACK.

       END PROGRAM B08--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B09--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Cursor" "TEST"
           DISPLAY SUBJ::GetProperty("Cursor").

           GOBACK.

       END PROGRAM B09--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B10--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Dock" "TEST"
           DISPLAY SUBJ::GetProperty("Dock").

           GOBACK.

       END PROGRAM B10--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B11--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Anchor" "TEST"
           DISPLAY SUBJ::GetProperty("Anchor").

           GOBACK.

       END PROGRAM B11--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B12--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Padding" "10"
           DISPLAY SUBJ::GetProperty("Padding").

           GOBACK.

       END PROGRAM B12--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B13--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Opacity" "110"
           DISPLAY SUBJ::GetProperty("Opacity").

           GOBACK.

       END PROGRAM B13--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B14--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowEnabled" "1"
           DISPLAY SUBJ::GetProperty("ShadowEnabled").

           GOBACK.

       END PROGRAM B14--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B15--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowOpacity" "30"
           DISPLAY SUBJ::GetProperty("ShadowOpacity").

           GOBACK.

       END PROGRAM B15--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B16--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowColor" "#CC3300"
           DISPLAY SUBJ::GetProperty("ShadowColor").

           GOBACK.

       END PROGRAM B16--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B17--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowDirection" "TEST"
           DISPLAY SUBJ::GetProperty("ShadowDirection").

           GOBACK.

       END PROGRAM B17--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B18--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowDistance" "17"
           DISPLAY SUBJ::GetProperty("ShadowDistance").

           GOBACK.

       END PROGRAM B18--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B19--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowBlur" "0"
           DISPLAY SUBJ::GetProperty("ShadowBlur").

           GOBACK.

       END PROGRAM B19--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B20--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ShadowBlurStrength" "18"
           DISPLAY SUBJ::GetProperty("ShadowBlurStrength").

           GOBACK.

       END PROGRAM B20--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B21--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ZOrder" "10"
           DISPLAY SUBJ::GetProperty("ZOrder").

           GOBACK.

       END PROGRAM B21--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B22--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "LabelFor" "TEST"
           DISPLAY SUBJ::GetProperty("LabelFor").

           GOBACK.

       END PROGRAM B22--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B23--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "DataItem" "TEST"
           DISPLAY SUBJ::GetProperty("DataItem").

           GOBACK.

       END PROGRAM B23--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B24--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "DataFormat" "TEST"
           DISPLAY SUBJ::GetProperty("DataFormat").

           GOBACK.

       END PROGRAM B24--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B25--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "BaseURL" "TEST"
           DISPLAY SUBJ::GetProperty("BaseURL").

           GOBACK.

       END PROGRAM B25--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B26--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "DefaultMethod" "TEST"
           DISPLAY SUBJ::GetProperty("DefaultMethod").

           GOBACK.

       END PROGRAM B26--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B27--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "AuthType" "TEST"
           DISPLAY SUBJ::GetProperty("AuthType").

           GOBACK.

       END PROGRAM B27--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B28--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "AuthToken" "TEST"
           DISPLAY SUBJ::GetProperty("AuthToken").

           GOBACK.

       END PROGRAM B28--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B29--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "DefaultHeaders" "TEST"
           DISPLAY SUBJ::GetProperty("DefaultHeaders").

           GOBACK.

       END PROGRAM B29--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B30--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "TimeoutSeconds" "40"
           DISPLAY SUBJ::GetProperty("TimeoutSeconds").

           GOBACK.

       END PROGRAM B30--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B31--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "FollowRedirects" "0"
           DISPLAY SUBJ::GetProperty("FollowRedirects").

           GOBACK.

       END PROGRAM B31--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B32--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "VerifyTLS" "0"
           DISPLAY SUBJ::GetProperty("VerifyTLS").

           GOBACK.

       END PROGRAM B32--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B33--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "RequestDataItem" "TEST"
           DISPLAY SUBJ::GetProperty("RequestDataItem").

           GOBACK.

       END PROGRAM B33--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B34--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ResponseDataItem" "TEST"
           DISPLAY SUBJ::GetProperty("ResponseDataItem").

           GOBACK.

       END PROGRAM B34--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B35--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "StatusDataItem" "TEST"
           DISPLAY SUBJ::GetProperty("StatusDataItem").

           GOBACK.

       END PROGRAM B35--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B36--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ResponseParagraph" "TEST"
           DISPLAY SUBJ::GetProperty("ResponseParagraph").

           GOBACK.

       END PROGRAM B36--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B37--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "ErrorParagraph" "TEST"
           DISPLAY SUBJ::GetProperty("ErrorParagraph").

           GOBACK.

       END PROGRAM B37--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B38--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "X" "60"
           DISPLAY SUBJ::GetProperty("X").

           GOBACK.

       END PROGRAM B38--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B39--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Y" "60"
           DISPLAY SUBJ::GetProperty("Y").

           GOBACK.

       END PROGRAM B39--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B40--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Width" "180"
           DISPLAY SUBJ::GetProperty("Width").

           GOBACK.

       END PROGRAM B40--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B41--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "SetProperty"
               USING "Height" "180"
           DISPLAY SUBJ::GetProperty("Height").

           GOBACK.

       END PROGRAM B41--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. B42--ONCLICK.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           INVOKE SUBJ "PlayAnimation"
               USING "1"
           DISPLAY "PlayAnimation invoked".

           GOBACK.

       END PROGRAM B42--ONCLICK.

       END PROGRAM TESTFORM.

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
       PROGRAM-ID. ALL-SITES.

       ENVIRONMENT DIVISION.
       CONFIGURATION SECTION.
       SPECIAL-NAMES.
           DECIMAL-POINT IS COMMA
      *> MARK-SPECIAL-NAMES-053.
       REPOSITORY.
           CLASS MARK-REPOSITORY-053 IS "Mark.Repository".
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.

           SELECT MARK-FILE ASSIGN TO "mark053.dat"
               ORGANIZATION IS LINE SEQUENTIAL. *> MARK-FILE-CONTROL-053.

       DATA DIVISION.
       FILE SECTION.
       FD  MARK-FILE.
       01  MARK-FILE-REC PIC X(80). *> MARK-FILE-SECTION-053.
       WORKING-STORAGE SECTION.
      *>── Cobolt runtime fields ─────────────────────────────────────
       01 COBOL-QUIT             PIC 9        VALUE 0.
       01 COBOL-EVENT-ID         PIC X(64)   VALUE SPACES.
       01 COBOL-CONTROL-ID       PIC X(64)   VALUE SPACES.
       01 COBOL-LAST-STATUS       PIC X(256)  VALUE SPACES.
       01 FORM-NAME               PIC X(64)   VALUE 'ALL-SITES'.

      *>── User Working Storage ────────────────────────────────────────
       01  WS-MARK-053 PIC X(24) VALUE "MARK-WORKING-STORAGE-053".

      *>── Form controls ───────────────────────────────────────────────
       01 WS-BTN-GO.
          05 WS-BTN-GO-TEXT       PIC X(256) VALUE 'BTN-GO'.
          05 WS-BTN-GO-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-GO-ENABLED    PIC 9      VALUE 1.

       01 WS-BTN-EMPTY.
          05 WS-BTN-EMPTY-TEXT       PIC X(256) VALUE 'BTN-EMPTY'.
          05 WS-BTN-EMPTY-VISIBLE    PIC 9      VALUE 1.
          05 WS-BTN-EMPTY-ENABLED    PIC 9      VALUE 1.

       PROCEDURE DIVISION.
       COBOL-MAIN.
           CALL "COBOL-INIT-FORM" USING FORM-NAME
           CALL "ALL-SITES--ONLOAD"
           PERFORM COBOL-EVENT-LOOP
           CALL "ALL-SITES--ONCLOSE"
           STOP RUN.

       COBOL-EVENT-LOOP.
           PERFORM UNTIL COBOL-QUIT = 1
               CALL "COBOL-WAIT-EVENT"
                   USING COBOL-EVENT-ID COBOL-CONTROL-ID
               EVALUATE COBOL-CONTROL-ID
                   WHEN "BTN-GO"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-GO--ONCLICK"
                       END-EVALUATE
                   WHEN "BTN-EMPTY"
                       EVALUATE COBOL-EVENT-ID
                           WHEN "onClick"
                               CALL "BTN-EMPTY--ONCLICK"
                       END-EVALUATE
               END-EVALUATE
           END-PERFORM.


      *> ── Nested event-handler programs (COBOL-85) ─────────────────────

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALL-SITES--ONLOAD IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       DATA DIVISION.
       PROCEDURE DIVISION.
           DISPLAY "MARK-FORM-ONLOAD-053".   
           CONTINUE.

           GOBACK.

       END PROGRAM ALL-SITES--ONLOAD.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. ALL-SITES--ONCLOSE IS COMMON PROGRAM.

      *>    TODO: Form onClose handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM ALL-SITES--ONCLOSE.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-GO--ONCLICK IS COMMON PROGRAM.

       ENVIRONMENT DIVISION.
       PROCEDURE DIVISION.
           DISPLAY "MARK-BTN-GO-ONCLICK-053".
           CONTINUE.

           GOBACK.

       END PROGRAM BTN-GO--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. BTN-EMPTY--ONCLICK IS COMMON PROGRAM.

      *>    TODO: BTN-EMPTY onClick handler
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       LINKAGE SECTION.

       PROCEDURE DIVISION.
           CONTINUE.

           GOBACK.

       END PROGRAM BTN-EMPTY--ONCLICK.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. VALIDATE-CUSTOMER IS COMMON PROGRAM.


       ENVIRONMENT DIVISION.
       PROCEDURE DIVISION.
           DISPLAY "MARK-PROCEDURE-053".

           GOBACK.

       END PROGRAM VALIDATE-CUSTOMER.

       END PROGRAM ALL-SITES.

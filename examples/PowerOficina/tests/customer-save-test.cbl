       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTOMER-SAVE-TEST.
      *> Test of the Backend program CUSTOMER-SAVE. It is a driver: the program under test is added to this file,
      *> as a contained program, by the assembly step, and the COPY paths are made absolute. Run it in an empty folder
      *> that has a data/ folder, so the project's own data is never touched.
      *> Cases: CPF and CNPJ (valid, wrong check digit, repeated digits), email, phone, consent flags, operator,
      *> a duplicate document, a change, a customer that does not exist, and the audit trail.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT AUDIT-CHECK ASSIGN TO "data/audit.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS AC-ID FILE STATUS IS WS-FS-AUD.

       DATA DIVISION.
       FILE SECTION.
      *> The driver reads the audit file under its own names: a file of the program under test is a different file.
       FD  AUDIT-CHECK
           RECORD CONTAINS 178 CHARACTERS.
       01  AUDIT-CHECK-RECORD.
           05 AC-ID                              PIC 9(10).
           05 AC-REST                            PIC X(168).

       WORKING-STORAGE SECTION.
       01 WS-FS-AUD                              PIC    XX.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
       01 WS-CASE-NAME                           PIC    X(44).
       01 WS-WANT-STATUS                         PIC    X(10).
       01 WS-WANT-FIELD                          PIC    X(10).
       01 WS-WANT-ID                             PIC    9(8).
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-AUDIT-LINES                         PIC    999    VALUE 0.
       01 WS-EOF                                 PIC    X      VALUE "N".
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME

           MOVE    "new customer, CPF, email consent"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "OK"                          TO    WS-WANT-STATUS
           MOVE    SPACES                        TO    WS-WANT-FIELD
           MOVE    1                             TO    WS-WANT-ID
           PERFORM RUN-CASE

           MOVE    "same document again is a duplicate"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "DUPLICATE"                   TO    WS-WANT-STATUS
           MOVE    1                             TO    WS-WANT-ID
           PERFORM RUN-CASE

           MOVE    "change the phone of customer 1"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    1                             TO    CRQ-ID
           MOVE    "(11) 4000-1234"              TO    CRQ-PHONE
           MOVE    "OK"                          TO    WS-WANT-STATUS
           MOVE    1                             TO    WS-WANT-ID
           PERFORM RUN-CASE

           MOVE    "new customer, valid CNPJ"    TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "11222333000181"              TO    CRQ-DOC
           MOVE    "Oficina Parceira Ltda"       TO    CRQ-NAME
           MOVE    "OK"                          TO    WS-WANT-STATUS
           MOVE    2                             TO    WS-WANT-ID
           PERFORM RUN-CASE

           MOVE    "CPF with a wrong check digit"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "52998224726"                 TO    CRQ-DOC
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "DOC"                         TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "CNPJ with a wrong check digit"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "11222333000182"              TO    CRQ-DOC
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "DOC"                         TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "CPF of repeated digits"      TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "11111111111"                 TO    CRQ-DOC
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "DOC"                         TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "document with a letter"      TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "5299822472A"                 TO    CRQ-DOC
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "DOC"                         TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "document of the wrong length"  TO  WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "529982247"                   TO    CRQ-DOC
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "DOC"                         TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "email without @"             TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "joao.exemplo.com"            TO    CRQ-EMAIL
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "EMAIL"                       TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "email consent but no email"  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    SPACES                        TO    CRQ-EMAIL
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "EMAIL"                       TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "phone with letters"          TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "12ab"                        TO    CRQ-PHONE
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PHONE"                       TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "WhatsApp consent but no number"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    SPACES                        TO    CRQ-WHATSAPP
           MOVE    "S"                           TO    CRQ-CONSENT-WA
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "PHONE"                       TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "consent flag that is not S or N"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    "X"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "CONSENT"                     TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "empty name"                  TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    SPACES                        TO    CRQ-NAME
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "NAME"                        TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "empty operator"              TO    WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    SPACES                        TO    CRQ-OPERATOR
           MOVE    "INVALID"                     TO    WS-WANT-STATUS
           MOVE    "OPERATOR"                    TO    WS-WANT-FIELD
           PERFORM RUN-CASE

           MOVE    "change of a customer that does not exist"  TO WS-CASE-NAME
           PERFORM BASE-REQUEST
           MOVE    999                           TO    CRQ-ID
           MOVE    "11144477735"                 TO    CRQ-DOC
           MOVE    "NOTFOUND"                    TO    WS-WANT-STATUS
           PERFORM RUN-CASE

           PERFORM COUNT-AUDIT-LINES
           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0

           DISPLAY "CUSTOMER-SAVE test"
           DISPLAY "  cases run      " WS-CASES
           DISPLAY "  passed         " WS-PASS
           DISPLAY "  failed         " WS-FAIL
           DISPLAY "  audit lines    " WS-AUDIT-LINES " (expected 003: two creations and one change)"
           DISPLAY "  elapsed        " WS-ELAPSED " hundredths of a second"
           IF    WS-FAIL = 0 AND WS-AUDIT-LINES = 3
               DISPLAY "  result         PASS"
           ELSE
               DISPLAY "  result         FAIL"
           END-IF
           STOP RUN.

       BASE-REQUEST.
           INITIALIZE CUS-REQ
           MOVE    0                             TO    CRQ-ID
           MOVE    "João da Silva"               TO    CRQ-NAME
           MOVE    "52998224725"                 TO    CRQ-DOC
           MOVE    "Rua das Flores, 100, São Paulo"  TO CRQ-ADDRESS
           MOVE    "(11) 3000-1234"              TO    CRQ-PHONE
           MOVE    "(11) 99999-0000"             TO    CRQ-WHATSAPP
           MOVE    "joao@exemplo.com.br"         TO    CRQ-EMAIL
           MOVE    "S"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           MOVE    SPACES                        TO    WS-WANT-FIELD
           MOVE    0                             TO    WS-WANT-ID.

       RUN-CASE.
           ADD     1                             TO    WS-CASES
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           IF    CRS-STATUS = WS-WANT-STATUS
                 AND CRS-FIELD = WS-WANT-FIELD
                 AND (WS-WANT-ID = 0 OR CRS-ID = WS-WANT-ID)
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
               DISPLAY "    wanted " WS-WANT-STATUS " " WS-WANT-FIELD " " WS-WANT-ID
               DISPLAY "    got    " CRS-STATUS " " CRS-FIELD " " CRS-ID " " CRS-MESSAGE
           END-IF.

       COUNT-AUDIT-LINES.
           OPEN    INPUT AUDIT-CHECK
           IF    WS-FS-AUD = "00"
               MOVE    LOW-VALUES                TO    AC-ID
               START   AUDIT-CHECK KEY NOT < AC-ID
               PERFORM UNTIL WS-EOF = "Y"
                   READ    AUDIT-CHECK NEXT
                       AT END
                           MOVE    "Y"           TO    WS-EOF
                       NOT AT END
                           ADD     1             TO    WS-AUDIT-LINES
                   END-READ
               END-PERFORM
               CLOSE   AUDIT-CHECK
           END-IF.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. CUSTOMER-SAVE.
      *> Saves a customer of the workshop: a new one (CRQ-ID = 0) or a change to an existing one.
      *> Checks the data first (R4): the name, the operator, the CPF or CNPJ with its check digits, the email
      *> when the customer agreed to be contacted by email, the phone numbers and the two consent flags.
      *> A new customer whose document is already registered is not duplicated: the answer is DUPLICATE and
      *> carries the id of the customer that has it (R3). Every save writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-customer.cpy and res-customer.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUS-ID
               ALTERNATE RECORD KEY IS CUS-DOC WITH DUPLICATES
               FILE STATUS IS WS-FS-CUS.
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/customers.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-CUS                              PIC    XX.
       01 WS-DOC                                 PIC    X(14).
       01 WS-DOC-LEN                             PIC    99.
       01 WS-POS                                 PIC    99.
       01 WS-SUM                                 PIC    9(5).
       01 WS-WEIGHT                              PIC    99.
       01 WS-DIGIT                               PIC    9.
       01 WS-QUOTIENT                            PIC    9(5).
       01 WS-REMAINDER                           PIC    99.
       01 WS-CHECK-1                             PIC    9.
       01 WS-CHECK-2                             PIC    9.
       01 WS-DOC-OK                              PIC    9.
       01 WS-ALL-SAME                            PIC    9.
       01 WS-AT-COUNT                            PIC    99.
       01 WS-AT-POS                              PIC    99.
       01 WS-DOT-COUNT                           PIC    99.
       01 WS-CHAR                                PIC    X.
       01 WS-PHONE-TEXT                          PIC    X(20).
       01 WS-PHONE-OK                            PIC    9.
       01 WS-EXISTING-ID                         PIC    9(8).
       01 WS-ACTION                              PIC    X(12).
      *> The CNPJ weights of the first and of the second check digit.
       01 WS-CNPJ-W1                             PIC    X(12)    VALUE "543298765432".
       01 WS-CNPJ-W1-TABLE REDEFINES WS-CNPJ-W1.
          05 WS-CNPJ-WEIGHT-1                    PIC    9    OCCURS 12.
       01 WS-CNPJ-W2                             PIC    X(13)    VALUE "6543298765432".
       01 WS-CNPJ-W2-TABLE REDEFINES WS-CNPJ-W2.
          05 WS-CNPJ-WEIGHT-2                    PIC    9    OCCURS 13.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".

       PROCEDURE DIVISION USING CUS-REQ RETURNING CUS-RES.
       MAIN-PARA.
           INITIALIZE CUS-RES
           MOVE    "OK"                          TO    CRS-STATUS

           PERFORM VALIDATE-REQUEST

           IF    CRS-STATUS = "OK"
               PERFORM STORE-CUSTOMER
           END-IF

           GOBACK.

      *> VALIDATE-REQUEST: stops at the first field that fails and names it in CRS-FIELD.
       VALIDATE-REQUEST.
           EVALUATE TRUE
               WHEN CRQ-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "OPERATOR"            TO    CRS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    CRS-MESSAGE
               WHEN CRQ-NAME = SPACES
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "NAME"                TO    CRS-FIELD
                   MOVE    "Informe o nome do cliente."
                                                 TO    CRS-MESSAGE
               WHEN CRQ-CONSENT-EMAIL NOT = "S" AND CRQ-CONSENT-EMAIL NOT = "N"
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "CONSENT"             TO    CRS-FIELD
                   MOVE    "Consentimento de email deve ser S ou N."
                                                 TO    CRS-MESSAGE
               WHEN CRQ-CONSENT-WA NOT = "S" AND CRQ-CONSENT-WA NOT = "N"
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "CONSENT"             TO    CRS-FIELD
                   MOVE    "Consentimento de WhatsApp deve ser S ou N."
                                                 TO    CRS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE

           IF    CRS-STATUS = "OK"
               PERFORM CHECK-DOCUMENT
               IF    WS-DOC-OK = 0
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "DOC"                 TO    CRS-FIELD
                   MOVE    "CPF ou CNPJ inválido."
                                                 TO    CRS-MESSAGE
               END-IF
           END-IF

           IF    CRS-STATUS = "OK" AND (CRQ-CONSENT-EMAIL = "S" OR CRQ-EMAIL NOT = SPACES)
               PERFORM CHECK-EMAIL
           END-IF

           IF    CRS-STATUS = "OK"
               MOVE    CRQ-PHONE                 TO    WS-PHONE-TEXT
               PERFORM CHECK-PHONE
               IF    WS-PHONE-OK = 1
                   MOVE    CRQ-WHATSAPP          TO    WS-PHONE-TEXT
                   PERFORM CHECK-PHONE
               END-IF
               IF    WS-PHONE-OK = 0
                   MOVE    "INVALID"             TO    CRS-STATUS
                   MOVE    "PHONE"               TO    CRS-FIELD
                   MOVE    "Telefone com caracteres inválidos."
                                                 TO    CRS-MESSAGE
               END-IF
           END-IF

           IF    CRS-STATUS = "OK" AND CRQ-CONSENT-WA = "S" AND CRQ-WHATSAPP = SPACES
               MOVE    "INVALID"                 TO    CRS-STATUS
               MOVE    "PHONE"                   TO    CRS-FIELD
               MOVE    "Informe o WhatsApp para contatar por esse canal."
                                                 TO    CRS-MESSAGE
           END-IF.

      *> CHECK-DOCUMENT: WS-DOC-OK = 1 when CRQ-DOC is a CPF (11 digits) or a CNPJ (14 digits) whose check digits agree.
       CHECK-DOCUMENT.
           MOVE    0                             TO    WS-DOC-OK
           MOVE    CRQ-DOC                       TO    WS-DOC
           MOVE    0                             TO    WS-DOC-LEN

           PERFORM VARYING WS-POS FROM 14 BY -1 UNTIL WS-POS < 1 OR WS-DOC-LEN > 0
               IF    WS-DOC (WS-POS:1) NOT = SPACE
                   MOVE    WS-POS                TO    WS-DOC-LEN
               END-IF
           END-PERFORM

           IF    WS-DOC-LEN = 11 OR WS-DOC-LEN = 14
               IF    WS-DOC (1:WS-DOC-LEN) IS NUMERIC
                   MOVE    1                     TO    WS-ALL-SAME
                   PERFORM VARYING WS-POS FROM 2 BY 1 UNTIL WS-POS > WS-DOC-LEN
                       IF    WS-DOC (WS-POS:1) NOT = WS-DOC (1:1)
                           MOVE    0             TO    WS-ALL-SAME
                       END-IF
                   END-PERFORM

                   IF    WS-ALL-SAME = 0
                       IF    WS-DOC-LEN = 11
                           PERFORM CHECK-CPF
                       ELSE
                           PERFORM CHECK-CNPJ
                       END-IF
                   END-IF
               END-IF
           END-IF.

       CHECK-CPF.
           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 9
               MOVE    WS-DOC (WS-POS:1)         TO    WS-DIGIT
               COMPUTE WS-WEIGHT = 11 - WS-POS
               COMPUTE WS-SUM = WS-SUM + WS-DIGIT * WS-WEIGHT
           END-PERFORM
           DIVIDE  WS-SUM BY 11 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           IF    WS-REMAINDER < 2
               MOVE    0                         TO    WS-CHECK-1
           ELSE
               COMPUTE WS-CHECK-1 = 11 - WS-REMAINDER
           END-IF

           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 9
               MOVE    WS-DOC (WS-POS:1)         TO    WS-DIGIT
               COMPUTE WS-WEIGHT = 12 - WS-POS
               COMPUTE WS-SUM = WS-SUM + WS-DIGIT * WS-WEIGHT
           END-PERFORM
           COMPUTE WS-SUM = WS-SUM + WS-CHECK-1 * 2
           DIVIDE  WS-SUM BY 11 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           IF    WS-REMAINDER < 2
               MOVE    0                         TO    WS-CHECK-2
           ELSE
               COMPUTE WS-CHECK-2 = 11 - WS-REMAINDER
           END-IF

           IF    WS-DOC (10:1) = WS-CHECK-1 AND WS-DOC (11:1) = WS-CHECK-2
               MOVE    1                         TO    WS-DOC-OK
           END-IF.

       CHECK-CNPJ.
           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 12
               MOVE    WS-DOC (WS-POS:1)         TO    WS-DIGIT
               COMPUTE WS-SUM = WS-SUM + WS-DIGIT * WS-CNPJ-WEIGHT-1 (WS-POS)
           END-PERFORM
           DIVIDE  WS-SUM BY 11 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           IF    WS-REMAINDER < 2
               MOVE    0                         TO    WS-CHECK-1
           ELSE
               COMPUTE WS-CHECK-1 = 11 - WS-REMAINDER
           END-IF

           MOVE    0                             TO    WS-SUM
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 13
               IF    WS-POS = 13
                   MOVE    WS-CHECK-1            TO    WS-DIGIT
               ELSE
                   MOVE    WS-DOC (WS-POS:1)     TO    WS-DIGIT
               END-IF
               COMPUTE WS-SUM = WS-SUM + WS-DIGIT * WS-CNPJ-WEIGHT-2 (WS-POS)
           END-PERFORM
           DIVIDE  WS-SUM BY 11 GIVING WS-QUOTIENT REMAINDER WS-REMAINDER
           IF    WS-REMAINDER < 2
               MOVE    0                         TO    WS-CHECK-2
           ELSE
               COMPUTE WS-CHECK-2 = 11 - WS-REMAINDER
           END-IF

           IF    WS-DOC (13:1) = WS-CHECK-1 AND WS-DOC (14:1) = WS-CHECK-2
               MOVE    1                         TO    WS-DOC-OK
           END-IF.

      *> CHECK-EMAIL: one "@" with text before it, and a "." somewhere after it. An email is required when the customer
      *> agreed to be contacted by email, and checked whenever it is given.
       CHECK-EMAIL.
           MOVE    0                             TO    WS-AT-COUNT
           MOVE    0                             TO    WS-AT-POS
           MOVE    0                             TO    WS-DOT-COUNT

           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 60
               MOVE    CRQ-EMAIL (WS-POS:1)      TO    WS-CHAR
               IF    WS-CHAR = "@"
                   ADD     1                     TO    WS-AT-COUNT
                   MOVE    WS-POS                TO    WS-AT-POS
               END-IF
               IF    WS-CHAR = "." AND WS-AT-POS > 0 AND WS-POS > WS-AT-POS + 1
                   ADD     1                     TO    WS-DOT-COUNT
               END-IF
           END-PERFORM

           IF    WS-AT-COUNT NOT = 1 OR WS-AT-POS < 2 OR WS-DOT-COUNT = 0
               MOVE    "INVALID"                 TO    CRS-STATUS
               MOVE    "EMAIL"                   TO    CRS-FIELD
               MOVE    "Email inválido."         TO    CRS-MESSAGE
           END-IF.

      *> CHECK-PHONE: WS-PHONE-OK = 0 when WS-PHONE-TEXT holds anything but digits, spaces and + ( ) -.
       CHECK-PHONE.
           MOVE    1                             TO    WS-PHONE-OK
           PERFORM VARYING WS-POS FROM 1 BY 1 UNTIL WS-POS > 20
               MOVE    WS-PHONE-TEXT (WS-POS:1)  TO    WS-CHAR
               IF    NOT (WS-CHAR IS NUMERIC OR WS-CHAR = SPACE OR WS-CHAR = "+"
                     OR WS-CHAR = "(" OR WS-CHAR = ")" OR WS-CHAR = "-")
                   MOVE    0                     TO    WS-PHONE-OK
               END-IF
           END-PERFORM.

      *> STORE-CUSTOMER: inserts or rewrites the customer, then writes the audit line.
       STORE-CUSTOMER.
           OPEN    I-O CUSTOMER-FILE
           IF    WS-FS-CUS = "35"
               OPEN    OUTPUT CUSTOMER-FILE
               CLOSE   CUSTOMER-FILE
               OPEN    I-O CUSTOMER-FILE
           END-IF

           IF    WS-FS-CUS NOT = "00"
               MOVE    "ERROR"                   TO    CRS-STATUS
               MOVE    "Arquivo de clientes indisponível."
                                                 TO    CRS-MESSAGE
           ELSE
               PERFORM FIND-DOCUMENT-OWNER

               EVALUATE TRUE
                   WHEN WS-EXISTING-ID > 0 AND WS-EXISTING-ID NOT = CRQ-ID
                       MOVE    "DUPLICATE"       TO    CRS-STATUS
                       MOVE    WS-EXISTING-ID    TO    CRS-ID
                       MOVE    "Este documento já está cadastrado."
                                                 TO    CRS-MESSAGE
                   WHEN CRQ-ID = 0
                       PERFORM INSERT-CUSTOMER
                   WHEN OTHER
                       PERFORM UPDATE-CUSTOMER
               END-EVALUATE

               CLOSE   CUSTOMER-FILE

               IF    CRS-STATUS = "OK"
                   MOVE    CRQ-OPERATOR          TO    WS-AU-OPERATOR
                   MOVE    "CUSTOMER"            TO    WS-AU-ENTITY
                   MOVE    CRS-ID                TO    WS-AU-REF
                   MOVE    WS-ACTION             TO    WS-AU-ACTION
                   MOVE    CRQ-NAME              TO    WS-AU-DETAIL
                   PERFORM WRITE-AUDIT
               END-IF
           END-IF.

      *> FIND-DOCUMENT-OWNER: WS-EXISTING-ID = the customer already carrying CRQ-DOC, or 0.
       FIND-DOCUMENT-OWNER.
           MOVE    0                             TO    WS-EXISTING-ID
           MOVE    CRQ-DOC                       TO    CUS-DOC
           PERFORM SELECT-CUSTOMERS-BY-DOCUMENT
           IF    WS-PQ-STATUS = "00"
               PERFORM FETCH-CUSTOMER-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    CUS-ID                TO    WS-EXISTING-ID
               END-IF
           END-IF.

       INSERT-CUSTOMER.
           MOVE    "CUSTOMER"                    TO    WS-SEQ-WANT
           PERFORM NEXT-NUMBER

           IF    WS-SEQ-GOT = 0
               MOVE    "ERROR"                   TO    CRS-STATUS
               MOVE    "Não foi possível numerar o cliente."
                                                 TO    CRS-MESSAGE
           ELSE
               INITIALIZE CUSTOMER-RECORD
               MOVE    WS-SEQ-GOT                TO    CUS-ID
               PERFORM MOVE-REQUEST-TO-RECORD
               PERFORM INSERT-CUSTOMER-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    CUS-ID                TO    CRS-ID
                   MOVE    "CREATE"              TO    WS-ACTION
               ELSE
                   MOVE    "ERROR"               TO    CRS-STATUS
                   MOVE    "Não foi possível gravar o cliente."
                                                 TO    CRS-MESSAGE
               END-IF
           END-IF.

       UPDATE-CUSTOMER.
           MOVE    CRQ-ID                        TO    CUS-ID
           PERFORM GET-CUSTOMER-ROW
           IF    WS-PQ-STATUS = "00"
               PERFORM MOVE-REQUEST-TO-RECORD
               PERFORM UPDATE-CUSTOMER-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    CUS-ID                TO    CRS-ID
                   MOVE    "UPDATE"              TO    WS-ACTION
               ELSE
                   MOVE    "ERROR"               TO    CRS-STATUS
                   MOVE    "Não foi possível alterar o cliente."
                                                 TO    CRS-MESSAGE
               END-IF
           ELSE
               MOVE    "NOTFOUND"                TO    CRS-STATUS
               MOVE    "Cliente não encontrado."
                                                 TO    CRS-MESSAGE
           END-IF.

       MOVE-REQUEST-TO-RECORD.
           MOVE    CRQ-NAME                      TO    CUS-NAME
           MOVE    CRQ-DOC                       TO    CUS-DOC
           MOVE    CRQ-ADDRESS                   TO    CUS-ADDRESS
           MOVE    CRQ-PHONE                     TO    CUS-PHONE
           MOVE    CRQ-WHATSAPP                  TO    CUS-WHATSAPP
           MOVE    CRQ-EMAIL                     TO    CUS-EMAIL
           MOVE    CRQ-CONSENT-EMAIL             TO    CUS-CONSENT-EMAIL
           MOVE    CRQ-CONSENT-WA                TO    CUS-CONSENT-WA.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

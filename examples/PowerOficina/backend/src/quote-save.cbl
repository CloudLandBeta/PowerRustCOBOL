       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-SAVE.
      *> Makes a quote from the parts, services and add-ons the consultant chose (R10, R11, R13, R14, R16, R17, R38,
      *> R40-R43). The request carries only codes and quantities: every price, the labour rate and every tax rate are
      *> taken here from the catalog of the quote's currency, so a screen cannot dictate a price.
      *> - A service costs its hours times the labour rate (HORA-MO); the standard hours are the service's own, and
      *>   other hours need a reason (R11, R12). An add-on has its own price and hours; a part its unit price.
      *> - Every line has its own tax rate, and the quote shows the total without taxes, the taxes and the total (R38).
      *> - The execution date suggested is the first day on which the hours already approved plus this quote's fit
      *>   in the shop's capacity of 16 hours a day; the consultant may agree another date that is not in the past
      *>   (R16). The quote is valid for 7 days.
      *> - Quotes are numbered in sequence. Saving again with the number of an existing quote adds the next revision
      *>   and leaves the earlier ones as they were (R17); if the currency is another one, a NEW quote is made with the
      *>   prices of that currency and the original is untouched (R43). Nothing is ever converted (R41).
      *> Every save writes a line of the audit trail (R36).
      *> The request and the answer are in backend/COPYBOOKS/req-quote.cpy and res-quote.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           SELECT VEHICLE-FILE ASSIGN TO "data/vehicles.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS VEH-PLATE
               ALTERNATE RECORD KEY IS VEH-CUS-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-VEH.
           SELECT DIAGNOSIS-FILE ASSIGN TO "data/diagnoses.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS DGN-ID
               ALTERNATE RECORD KEY IS DGN-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-DGN.
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/vehicles.FD".
           COPY "COPYBOOKS/diagnoses.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
           COPY "backend/COPYBOOKS/ws-plate".
           COPY "backend/COPYBOOKS/ws-quote".
       01 WS-FS-VEH                              PIC    XX.
       01 WS-FS-DGN                              PIC    XX.
       01 WS-I                                   PIC    999.
       01 WS-RATE                                PIC    9(7)V99.
       01 WS-RATE-TAX                            PIC    9(2)V99.
       01 WS-LINE-HOURS                          PIC    9(4)V99.
       01 WS-QUEUE-HOURS                         PIC    9(6)V99.
       01 WS-HX                                  PIC    9(9).
       01 WS-DAYS                                PIC    9(4).
       01 WS-DAYNUM                              PIC    9(9).
       01 WS-NEW-NUMBER                          PIC    9.
       01 WS-EOF                                 PIC    X.
       01 WS-OPEN                                PIC    9      VALUE 0.
       01 WS-CAPACITY-X100                       PIC    9(4)   VALUE 1600.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote".
           COPY "backend/COPYBOOKS/res-quote".

       PROCEDURE DIVISION USING QUO-REQ RETURNING QUO-RES.
       MAIN-PARA.
           INITIALIZE QUO-RES
           MOVE    "OK"                          TO    QTS-STATUS
           MOVE    0                             TO    WS-OPEN

           MOVE    QRQ-PLATE                     TO    WS-PLATE-IN
           PERFORM NORMALIZE-PLATE
           PERFORM VALIDATE-HEADER

           IF    QTS-STATUS = "OK"
               PERFORM READ-VEHICLE
           END-IF

           IF    QTS-STATUS = "OK" AND QRQ-DGN-ID > 0
               PERFORM CHECK-DIAGNOSIS
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM OPEN-FILES
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM READ-LABOUR-RATE
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM PRICE-LINES
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM SET-DEADLINE
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM DECIDE-NUMBER
           END-IF

           IF    QTS-STATUS = "OK"
               PERFORM WRITE-QUOTE
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               CLOSE   QUOTE-ITEM-FILE
               CLOSE   CATALOG-FILE
           END-IF

           IF    QTS-STATUS = "OK"
               MOVE    QRQ-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "QUOTE"                   TO    WS-AU-ENTITY
               MOVE    QTS-ID                    TO    WS-AU-REF
               MOVE    QTS-ACTION                TO    WS-AU-ACTION
               STRING    QTS-PLATE " " QTS-CURRENCY " rev " QTS-REV  INTO  WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       VALIDATE-HEADER.
           EVALUATE TRUE
               WHEN QRQ-OPERATOR = SPACES
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "OPERATOR"            TO    QTS-FIELD
                   MOVE    "Informe o nome do operador."
                                                 TO    QTS-MESSAGE
               WHEN WS-PLATE-OK = 0
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "PLATE"               TO    QTS-FIELD
                   MOVE    "A placa deve ter 7 letras ou números."
                                                 TO    QTS-MESSAGE
               WHEN QRQ-CURRENCY NOT = "BRL" AND QRQ-CURRENCY NOT = "EUR"
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "CURRENCY"            TO    QTS-FIELD
                   MOVE    "A moeda deve ser BRL ou EUR."
                                                 TO    QTS-MESSAGE
               WHEN QRQ-COUNT = 0 OR QRQ-COUNT > 20
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "COUNT"               TO    QTS-FIELD
                   MOVE    "O orçamento precisa de 1 a 20 itens."
                                                 TO    QTS-MESSAGE
               WHEN OTHER
                   CONTINUE
           END-EVALUATE.

      *> READ-VEHICLE: the vehicle must be on file; its owner is the quote's customer.
       READ-VEHICLE.
           MOVE    "NOVEHICLE"                   TO    QTS-STATUS
           MOVE    "Veículo não cadastrado. Faça o atendimento primeiro."
                                                 TO    QTS-MESSAGE
           OPEN    INPUT VEHICLE-FILE
           IF    WS-FS-VEH = "00"
               MOVE    WS-PLATE-OUT              TO    VEH-PLATE
               PERFORM GET-VEHICLE-ROW
               IF    WS-PQ-STATUS = "00"
                   MOVE    "OK"                  TO    QTS-STATUS
                   MOVE    SPACES                TO    QTS-MESSAGE
                   MOVE    VEH-CUS-ID            TO    QTS-CUS-ID
               END-IF
               CLOSE   VEHICLE-FILE
           END-IF.

      *> CHECK-DIAGNOSIS: a diagnosis named on the quote must exist and be of the same vehicle.
       CHECK-DIAGNOSIS.
           MOVE    "NODIAG"                      TO    QTS-STATUS
           MOVE    "Diagnóstico não encontrado." TO    QTS-MESSAGE
           OPEN    INPUT DIAGNOSIS-FILE
           IF    WS-FS-DGN = "00"
               MOVE    QRQ-DGN-ID                TO    DGN-ID
               PERFORM GET-DIAGNOSIS-ROW
               IF    WS-PQ-STATUS = "00"
                   IF    DGN-PLATE = WS-PLATE-OUT
                       MOVE    "OK"              TO    QTS-STATUS
                       MOVE    SPACES            TO    QTS-MESSAGE
                   ELSE
                       MOVE    "INVALID"         TO    QTS-STATUS
                       MOVE    "DIAG"            TO    QTS-FIELD
                       MOVE    "Esse diagnóstico é de outro veículo."
                                                 TO    QTS-MESSAGE
                   END-IF
               END-IF
               CLOSE   DIAGNOSIS-FILE
           END-IF.

       OPEN-FILES.
           OPEN    I-O QUOTE-FILE
           IF    WS-FS-QUO = "35"
               OPEN    OUTPUT QUOTE-FILE
               CLOSE   QUOTE-FILE
               OPEN    I-O QUOTE-FILE
           END-IF

           OPEN    I-O QUOTE-ITEM-FILE
           IF    WS-FS-QIT = "35"
               OPEN    OUTPUT QUOTE-ITEM-FILE
               CLOSE   QUOTE-ITEM-FILE
               OPEN    I-O QUOTE-ITEM-FILE
           END-IF

           OPEN    INPUT CATALOG-FILE

           IF    WS-FS-QUO = "00" AND WS-FS-QIT = "00" AND WS-FS-CAT = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    QTS-STATUS
               MOVE    "Arquivos do orçamento indisponíveis."
                                                 TO    QTS-MESSAGE
           END-IF.

      *> READ-LABOUR-RATE: the hourly rate and its tax rate, from the catalog row HORA-MO of the quote's currency.
       READ-LABOUR-RATE.
           MOVE    QRQ-CURRENCY                  TO    CAT-CURRENCY
           MOVE    "HORA-MO"                     TO    CAT-CODE
           PERFORM GET-CATALOG-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    CAT-PRICE                 TO    WS-RATE
               MOVE    CAT-TAX-RATE              TO    WS-RATE-TAX
           ELSE
               MOVE    "ERROR"                   TO    QTS-STATUS
               MOVE    "Tarifa por hora não cadastrada nessa moeda."
                                                 TO    QTS-MESSAGE
           END-IF.

      *> PRICE-LINES: checks every line against the catalog of the currency and prices it. Stops at the first
      *> line that is wrong and says which one (QTS-LINE-NO).
       PRICE-LINES.
           MOVE    QRQ-COUNT                     TO    QTS-COUNT
           MOVE    0                             TO    QTS-HOURS
           MOVE    0                             TO    QTS-SUBTOTAL
           MOVE    0                             TO    QTS-TAX
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > QRQ-COUNT OR QTS-STATUS NOT = "OK"
               PERFORM PRICE-ONE-LINE
           END-PERFORM
           IF    QTS-STATUS = "OK"
               COMPUTE QTS-TOTAL = QTS-SUBTOTAL + QTS-TAX
           END-IF.

       PRICE-ONE-LINE.
           MOVE    QRQ-CURRENCY                  TO    CAT-CURRENCY
           MOVE    QRL-CODE (WS-I)               TO    CAT-CODE
           PERFORM GET-CATALOG-ROW
           IF    WS-PQ-STATUS = "00"
               IF    CAT-KIND NOT = QRL-KIND (WS-I)
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "KIND"                TO    QTS-FIELD
                   MOVE    WS-I                  TO    QTS-LINE-NO
                   MOVE    "O tipo do item não confere com o catálogo."
                                                 TO    QTS-MESSAGE
               ELSE
                   PERFORM PRICE-BY-KIND
               END-IF
           ELSE
               MOVE    "INVALID"                 TO    QTS-STATUS
               MOVE    "LINE"                    TO    QTS-FIELD
               MOVE    WS-I                      TO    QTS-LINE-NO
               MOVE    "Item não encontrado no catálogo dessa moeda."
                                                 TO    QTS-MESSAGE
           END-IF.

       PRICE-BY-KIND.
           MOVE    QRL-KIND (WS-I)               TO    QTL-KIND (WS-I)
           MOVE    QRL-CODE (WS-I)               TO    QTL-CODE (WS-I)
           MOVE    CAT-NAME                      TO    QTL-NAME (WS-I)
           MOVE    QRL-NOTE (WS-I)               TO    QTL-NOTE (WS-I)
           MOVE    "N"                           TO    QTL-MANUAL (WS-I)
           IF    QRL-MANUAL (WS-I) = "S"
               MOVE    "S"                       TO    QTL-MANUAL (WS-I)
           END-IF

           EVALUATE CAT-KIND
               WHEN "P"
                   IF    QRL-QTY (WS-I) = 0
                       MOVE    "INVALID"         TO    QTS-STATUS
                       MOVE    "LINE"            TO    QTS-FIELD
                       MOVE    WS-I              TO    QTS-LINE-NO
                       MOVE    "Informe a quantidade da peça."
                                                 TO    QTS-MESSAGE
                   ELSE
                       MOVE    QRL-QTY (WS-I)    TO    QTL-QTY (WS-I)
                       MOVE    CAT-PRICE         TO    QTL-UNIT (WS-I)
                       MOVE    CAT-TAX-RATE      TO    QTL-TAX-RATE (WS-I)
                       MOVE    0                 TO    WS-LINE-HOURS
                   END-IF
               WHEN "S"
                   MOVE    CAT-HOURS             TO    WS-LINE-HOURS
                   IF    QRL-HOURS (WS-I) > 0
                       IF    QRL-NOTE (WS-I) = SPACES
                           MOVE    "INVALID"     TO    QTS-STATUS
                           MOVE    "NOTE"        TO    QTS-FIELD
                           MOVE    WS-I          TO    QTS-LINE-NO
                           MOVE    "Horas diferentes do padrão pedem uma justificativa."
                                                 TO    QTS-MESSAGE
                       ELSE
                           MOVE    QRL-HOURS (WS-I)  TO WS-LINE-HOURS
                           MOVE    "S"           TO    QTL-MANUAL (WS-I)
                       END-IF
                   END-IF
                   MOVE    WS-LINE-HOURS         TO    QTL-QTY (WS-I)
                   MOVE    WS-RATE               TO    QTL-UNIT (WS-I)
                   MOVE    WS-RATE-TAX           TO    QTL-TAX-RATE (WS-I)
               WHEN "A"
                   MOVE    1                     TO    QTL-QTY (WS-I)
                   IF    QRL-QTY (WS-I) > 0
                       MOVE    QRL-QTY (WS-I)    TO    QTL-QTY (WS-I)
                   END-IF
                   MOVE    CAT-PRICE             TO    QTL-UNIT (WS-I)
                   MOVE    CAT-TAX-RATE          TO    QTL-TAX-RATE (WS-I)
                   COMPUTE WS-LINE-HOURS = CAT-HOURS * QTL-QTY (WS-I)
               WHEN OTHER
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "KIND"                TO    QTS-FIELD
                   MOVE    WS-I                  TO    QTS-LINE-NO
                   MOVE    "Use peça (P), serviço (S) ou adicional (A)."
                                                 TO    QTS-MESSAGE
           END-EVALUATE

           IF    QTS-STATUS = "OK"
               COMPUTE QTL-AMOUNT (WS-I) ROUNDED = QTL-QTY (WS-I) * QTL-UNIT (WS-I)
               COMPUTE QTL-TAX (WS-I) ROUNDED = QTL-AMOUNT (WS-I) * QTL-TAX-RATE (WS-I) / 100
               ADD     QTL-AMOUNT (WS-I)         TO    QTS-SUBTOTAL
               ADD     QTL-TAX (WS-I)            TO    QTS-TAX
               ADD     WS-LINE-HOURS             TO    QTS-HOURS
           END-IF.

      *> SET-DEADLINE: the date the queue suggests, the date agreed and how long the quote is valid.
      *> The hours already in the queue are those of the approved quotes.
       SET-DEADLINE.
           PERFORM NOW-STAMP
           MOVE    0                             TO    WS-QUEUE-HOURS
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-ALL-QUOTES
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y"
               PERFORM FETCH-QUOTE-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    QUO-STATUS = "APROVADO"
                       ADD     QUO-HOURS         TO    WS-QUEUE-HOURS
                   END-IF
               END-IF
           END-PERFORM

           COMPUTE WS-HX = (WS-QUEUE-HOURS + QTS-HOURS) * 100
           COMPUTE WS-DAYS = (WS-HX + WS-CAPACITY-X100 - 1) / WS-CAPACITY-X100
           IF    WS-DAYS < 1
               MOVE    1                         TO    WS-DAYS
           END-IF
           COMPUTE WS-DAYNUM = FUNCTION INTEGER-OF-DATE (WS-NOW-DATE) + WS-DAYS
           COMPUTE QTS-SUGGESTED = FUNCTION DATE-OF-INTEGER (WS-DAYNUM)

           IF    QRQ-DEADLINE = 0
               MOVE    QTS-SUGGESTED             TO    QTS-DEADLINE
           ELSE
               IF    QRQ-DEADLINE < WS-NOW-DATE
                   MOVE    "INVALID"             TO    QTS-STATUS
                   MOVE    "DEADLINE"            TO    QTS-FIELD
                   MOVE    "O prazo não pode estar no passado."
                                                 TO    QTS-MESSAGE
               ELSE
                   MOVE    QRQ-DEADLINE          TO    QTS-DEADLINE
               END-IF
           END-IF

           COMPUTE WS-DAYNUM = FUNCTION INTEGER-OF-DATE (WS-NOW-DATE) + 7
           COMPUTE QTS-VALID-UNTIL = FUNCTION DATE-OF-INTEGER (WS-DAYNUM).

      *> DECIDE-NUMBER: a new quote takes the next number; a change of an existing quote takes its next revision,
      *> unless the currency changed, which makes a new quote.
       DECIDE-NUMBER.
           MOVE    1                             TO    WS-NEW-NUMBER
           MOVE    "CREATED"                     TO    QTS-ACTION

           IF    QRQ-ID > 0
               MOVE    QRQ-ID                    TO    WS-LQ-ID
               PERFORM FIND-LATEST-REV
               IF    WS-LQ-REV = 0
                   MOVE    "NOTFOUND"            TO    QTS-STATUS
                   MOVE    "Orçamento não encontrado."
                                                 TO    QTS-MESSAGE
               ELSE
                   MOVE    QRQ-ID                TO    QUO-ID
                   MOVE    WS-LQ-REV             TO    QUO-REV
                   PERFORM GET-QUOTE-ROW

                   IF    QUO-CURRENCY = QRQ-CURRENCY
                         AND QUO-STATUS NOT = "ABERTO" AND QUO-STATUS NOT = "ENVIADO"
                         AND QUO-STATUS NOT = "RECUSADO" AND QUO-STATUS NOT = "EXPIRADO"
                       MOVE    "INVALID"         TO    QTS-STATUS
                       MOVE    "STATUS"          TO    QTS-FIELD
                       MOVE    "Orçamento aprovado não pode ser alterado."
                                                 TO    QTS-MESSAGE
                   ELSE
                   IF    QUO-CURRENCY = QRQ-CURRENCY
                       IF    WS-LQ-REV = 99
                           MOVE    "ERROR"       TO    QTS-STATUS
                           MOVE    "Limite de 99 revisões atingido."
                                                 TO    QTS-MESSAGE
                       ELSE
                           MOVE    0             TO    WS-NEW-NUMBER
                           MOVE    "REVISED"     TO    QTS-ACTION
                           MOVE    QRQ-ID        TO    QTS-ID
                           COMPUTE QTS-REV       =     WS-LQ-REV + 1
                       END-IF
                   ELSE
                       MOVE    "NEWCURRENCY"     TO    QTS-ACTION
                   END-IF
                   END-IF
               END-IF
           END-IF

           IF    QTS-STATUS = "OK" AND WS-NEW-NUMBER = 1
               MOVE    "QUOTE"                   TO    WS-SEQ-WANT
               PERFORM NEXT-NUMBER
               IF    WS-SEQ-GOT = 0
                   MOVE    "ERROR"               TO    QTS-STATUS
                   MOVE    "Não foi possível numerar o orçamento."
                                                 TO    QTS-MESSAGE
               ELSE
                   MOVE    WS-SEQ-GOT            TO    QTS-ID
                   MOVE    1                     TO    QTS-REV
               END-IF
           END-IF.

      *> WRITE-QUOTE: the header and then every line. A quote that is written is never changed.
       WRITE-QUOTE.
           PERFORM NOW-STAMP
           MOVE    WS-NOW                        TO    QTS-CREATED
           MOVE    "ABERTO"                      TO    QTS-QSTATUS
           MOVE    QRQ-OPERATOR                  TO    QTS-OPERATOR
           MOVE    WS-PLATE-OUT                  TO    QTS-PLATE
           MOVE    QRQ-DGN-ID                    TO    QTS-DGN-ID
           MOVE    QRQ-CURRENCY                  TO    QTS-CURRENCY

           INITIALIZE QUOTE-RECORD
           MOVE    QTS-ID                        TO    QUO-ID
           MOVE    QTS-REV                       TO    QUO-REV
           MOVE    QTS-CUS-ID                    TO    QUO-CUS-ID
           MOVE    QTS-PLATE                     TO    QUO-PLATE
           MOVE    QTS-DGN-ID                    TO    QUO-DGN-ID
           MOVE    QTS-CURRENCY                  TO    QUO-CURRENCY
           MOVE    QTS-QSTATUS                   TO    QUO-STATUS
           MOVE    QTS-CREATED                   TO    QUO-CREATED
           MOVE    QTS-VALID-UNTIL               TO    QUO-VALID-UNTIL
           MOVE    QTS-DEADLINE                  TO    QUO-DEADLINE
           MOVE    QTS-HOURS                     TO    QUO-HOURS
           MOVE    QTS-SUBTOTAL                  TO    QUO-SUBTOTAL
           MOVE    QTS-TAX                       TO    QUO-TAX
           MOVE    QTS-TOTAL                     TO    QUO-TOTAL
           MOVE    QTS-OPERATOR                  TO    QUO-OPERATOR
           PERFORM INSERT-QUOTE-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QTS-STATUS
               MOVE    "Não foi possível gravar o orçamento."
                                                 TO    QTS-MESSAGE
           END-IF

           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > QTS-COUNT OR QTS-STATUS NOT = "OK"
               INITIALIZE QUOTE-ITEM-RECORD
               MOVE    QTS-ID                    TO    QIT-QUO-ID
               MOVE    QTS-REV                   TO    QIT-REV
               MOVE    WS-I                      TO    QIT-LINE
               MOVE    QTL-KIND (WS-I)           TO    QIT-KIND
               MOVE    QTL-CODE (WS-I)           TO    QIT-CODE
               MOVE    QTL-NAME (WS-I)           TO    QIT-NAME
               MOVE    QTL-QTY (WS-I)            TO    QIT-QTY
               MOVE    QTL-UNIT (WS-I)           TO    QIT-UNIT
               MOVE    QTL-TAX-RATE (WS-I)       TO    QIT-TAX-RATE
               MOVE    QTL-AMOUNT (WS-I)         TO    QIT-AMOUNT
               MOVE    QTL-MANUAL (WS-I)         TO    QIT-MANUAL
               MOVE    QTL-NOTE (WS-I)           TO    QIT-NOTE
               PERFORM INSERT-QUOTE-ITEM-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "ERROR"               TO    QTS-STATUS
                   MOVE    "Não foi possível gravar um item do orçamento."
                                                 TO    QTS-MESSAGE
               END-IF
           END-PERFORM.

           COPY "backend/COPYBOOKS/pd-plate".
           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-quote-load".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-quoteitems".
           COPY "backend/COPYBOOKS/pq-catalog".
           COPY "backend/COPYBOOKS/pq-vehicles".
           COPY "backend/COPYBOOKS/pq-diagnoses".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

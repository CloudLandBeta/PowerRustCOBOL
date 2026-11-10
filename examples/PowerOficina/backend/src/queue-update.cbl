       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUEUE-UPDATE.
      *> Moves a service along the execution queue and keeps the customer informed (R29, R30, R32, R33).
      *> - S starts a service that is QUEUED (it becomes RUNNING); F finishes one that is RUNNING (it becomes DONE). Every
      *>   change writes the time and the operator on the row and a line on the audit trail (R29). A service cannot skip a
      *>   state, go back, or be touched once it is DONE or REFUNDED.
      *> - When the last service of an order that is not refunded is DONE, the vehicle is ready for pickup and the
      *>   customer is told by email and WhatsApp (R30, R33), only through the channels the customer consented to (R34).
      *> - D changes the delivery date of every service of the order that is still QUEUED or RUNNING (the date cannot be in
      *>   the past) and tells the customer the date to come back for the vehicle (R32).
      *> The request and the answer are in backend/COPYBOOKS/req-queue-update.cpy and res-queue-update.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-quote".
           COPY "backend/COPYBOOKS/fc-pay".
           COPY "backend/COPYBOOKS/fc-seq-audit".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/quoteitems.FD".
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/payments.FD".
           COPY "COPYBOOKS/installments.FD".
           COPY "COPYBOOKS/refunds.FD".
           COPY "COPYBOOKS/queue.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/customers.FD".
           COPY "COPYBOOKS/sequences.FD".
           COPY "COPYBOOKS/audit.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-common".
           COPY "backend/COPYBOOKS/ws-partiql".
           COPY "backend/COPYBOOKS/ws-quote".
           COPY "backend/COPYBOOKS/ws-pay".
       01 WS-OPEN                                PIC    9      VALUE 0.
       01 WS-EOF                                 PIC    X.
       01 WS-LEFT                                PIC    999.
       01 WS-DONE                                PIC    999.
       01 WS-PLATE                               PIC    X(8).
       01 WS-DT                                  PIC    9(8).
       01 WS-DTX REDEFINES WS-DT.
          05 WS-DT-Y                             PIC    9(4).
          05 WS-DT-M                             PIC    99.
          05 WS-DT-D                             PIC    99.
       01 WS-HELD-COUNT                          PIC    999.
       01 WS-HANDLED-COUNT                       PIC    9(6).
       01 WS-HELD-INDEX                          PIC    999.
       01 WS-HELD.
          05 WS-HELD-QUEUE-ID                    PIC    9(8)   OCCURS 100.

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-queue-update".
           COPY "backend/COPYBOOKS/res-queue-update".

       PROCEDURE DIVISION USING QUN-REQ RETURNING QUN-RES.
       MAIN-PARA.
           INITIALIZE QUN-RES
           MOVE    "OK"                          TO    QUS-STATUS
           MOVE    "N"                           TO    QUS-READY
           MOVE    0                             TO    WS-OPEN
           PERFORM NOW-STAMP

           IF    QUQ-OPERATOR = SPACES
               MOVE    "INVALID"                 TO    QUS-STATUS
               MOVE    "Informe o nome do operador."
                                                 TO    QUS-MESSAGE
           END-IF
           IF    QUS-STATUS = "OK" AND QUQ-ACTION NOT = "S" AND QUQ-ACTION NOT = "F" AND QUQ-ACTION NOT = "D"
               MOVE    "INVALID"                 TO    QUS-STATUS
               MOVE    "A ação deve ser S (iniciar), F (concluir) ou D (data de entrega)."
                                                 TO    QUS-MESSAGE
           END-IF

           IF    QUS-STATUS = "OK"
               PERFORM OPEN-ALL
           END-IF

           IF    QUS-STATUS = "OK"
               IF    QUQ-ACTION = "D"
                   PERFORM CHANGE-DEADLINE
               ELSE
                   PERFORM MOVE-SERVICE
               END-IF
           END-IF

           IF    WS-OPEN = 1
               CLOSE   QUOTE-FILE
               PERFORM CLOSE-PAY-FILES
           END-IF

           IF    QUS-STATUS = "OK"
               MOVE    QUQ-OPERATOR              TO    WS-AU-OPERATOR
               MOVE    "QUEUE"                   TO    WS-AU-ENTITY
               MOVE    QUS-QUO-ID                TO    WS-AU-REF
               MOVE    QUQ-ACTION                TO    WS-AU-ACTION
               MOVE    QUS-STATE                 TO    WS-AU-DETAIL
               PERFORM WRITE-AUDIT
           END-IF

           GOBACK.

       OPEN-ALL.
           OPEN    INPUT QUOTE-FILE
           PERFORM OPEN-PAY-FILES
           IF    WS-FS-QUO = "00" AND WS-FS-QUE = "00" AND WS-FS-NTF = "00"
               MOVE    1                         TO    WS-OPEN
           ELSE
               MOVE    "ERROR"                   TO    QUS-STATUS
               MOVE    "Fila indisponível."      TO    QUS-MESSAGE
           END-IF.

       MOVE-SERVICE.
           MOVE    QUQ-QUE-ID                    TO    QUE-ID
           PERFORM GET-QUEUE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    QUE-QUO-ID                TO    QUS-QUO-ID
               MOVE    QUE-STATE                 TO    QUS-STATE
               MOVE    QUE-PLATE                 TO    WS-PLATE
               EVALUATE TRUE
                   WHEN QUQ-ACTION = "S" AND QUE-STATE = "QUEUED"
                       MOVE    "RUNNING"         TO    QUE-STATE
                       MOVE    WS-NOW            TO    QUE-STARTED
                       PERFORM SAVE-ROW
                   WHEN QUQ-ACTION = "F" AND QUE-STATE = "RUNNING"
                       MOVE    "DONE"            TO    QUE-STATE
                       MOVE    WS-NOW            TO    QUE-DONE
                       PERFORM SAVE-ROW
                       PERFORM CHECK-READY
                   WHEN QUQ-ACTION = "S"
                       MOVE    "INVALID"         TO    QUS-STATUS
                       MOVE    "Só um serviço que está na fila pode ser iniciado."
                                                 TO    QUS-MESSAGE
                   WHEN OTHER
                       MOVE    "INVALID"         TO    QUS-STATUS
                       MOVE    "Só um serviço em execução pode ser concluído."
                                                 TO    QUS-MESSAGE
               END-EVALUATE
           ELSE
               MOVE    "NOTFOUND"                TO    QUS-STATUS
               MOVE    "Serviço da fila não encontrado."
                                                 TO    QUS-MESSAGE
           END-IF.

       SAVE-ROW.
           MOVE    QUQ-OPERATOR                  TO    QUE-BY
           PERFORM UPDATE-QUEUE-ROW
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "ERROR"                   TO    QUS-STATUS
           END-IF
           MOVE    QUE-STATE                     TO    QUS-STATE
           IF    QUS-STATUS = "OK"
               MOVE    "Estado do serviço atualizado."
                                                 TO    QUS-MESSAGE
           END-IF.

      *> CHECK-READY: the vehicle is ready when no service of the order is left waiting or running.
       CHECK-READY.
           MOVE    0                             TO    WS-LEFT
           MOVE    0                             TO    WS-DONE
           MOVE    QUS-QUO-ID                    TO    QUE-QUO-ID
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-QUEUE-OF-QUOTE
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y"
               PERFORM FETCH-QUEUE-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   IF    QUE-QUO-ID NOT = QUS-QUO-ID
                       MOVE    "Y"               TO    WS-EOF
                   ELSE
                       IF    QUE-STATE = "QUEUED" OR QUE-STATE = "RUNNING"
                           ADD     1             TO    WS-LEFT
                       END-IF
                       IF    QUE-STATE = "DONE"
                           ADD     1             TO    WS-DONE
                       END-IF
                   END-IF
               END-IF
           END-PERFORM

           IF    WS-LEFT = 0 AND WS-DONE > 0
               MOVE    "Y"                       TO    QUS-READY
               MOVE    QUS-QUO-ID                TO    WS-NT-QUO
               MOVE    "PRONTO"                  TO    WS-NT-KIND
               MOVE    SPACES                    TO    WS-NT-TEXT
               STRING    "Seu veículo " WS-PLATE " está pronto para retirada. Venha buscá-lo."
                   INTO  WS-NT-TEXT
               PERFORM NOTIFY-BOTH
               MOVE    "Último serviço concluído: o veículo está pronto para retirada."
                                                 TO    QUS-MESSAGE
           END-IF.

      *> CHANGE-DEADLINE: the new delivery date goes to every service of the order still waiting or running.
       CHANGE-DEADLINE.
           MOVE    QUQ-QUO-ID                    TO    QUS-QUO-ID
           IF    QUQ-DEADLINE < WS-NOW-DATE
               MOVE    "INVALID"                 TO    QUS-STATUS
               MOVE    "A data de entrega não pode estar no passado."
                                                 TO    QUS-MESSAGE
           ELSE
               MOVE    0                         TO    QUS-CHANGED
               MOVE    "N"                       TO    WS-EOF
               MOVE    0                         TO    WS-HANDLED-COUNT

               PERFORM UNTIL WS-EOF = "Y"
                   PERFORM HOLD-ORDER-SERVICES
                   PERFORM VARYING WS-HELD-INDEX FROM 1 BY 1 UNTIL WS-HELD-INDEX > WS-HELD-COUNT
                       MOVE    WS-HELD-QUEUE-ID (WS-HELD-INDEX)
                                                 TO    QUE-ID
                       PERFORM GET-QUEUE-ROW
                       IF    WS-PQ-STATUS = "00"
                           IF    QUE-QUO-ID NOT = QUQ-QUO-ID
                               MOVE    "Y"       TO    WS-EOF
                           ELSE
                               IF    QUE-STATE = "QUEUED" OR QUE-STATE = "RUNNING"
                                   MOVE    QUQ-DEADLINE
                                                 TO    QUE-DEADLINE
                                   MOVE    QUQ-OPERATOR
                                                 TO    QUE-BY
                                   MOVE    QUE-PLATE
                                                 TO    WS-PLATE
                                   PERFORM UPDATE-QUEUE-ROW
                                   ADD     1     TO    QUS-CHANGED
                               END-IF
                           END-IF
                       END-IF
                   END-PERFORM
               END-PERFORM

               IF    QUS-CHANGED = 0
                   MOVE    "NOTFOUND"            TO    QUS-STATUS
                   MOVE    "Não há serviço a entregar neste orçamento."
                                                 TO    QUS-MESSAGE
               ELSE
                   MOVE    QUQ-QUO-ID            TO    WS-NT-QUO
                   MOVE    "PRAZO"               TO    WS-NT-KIND
                   MOVE    QUQ-DEADLINE          TO    WS-DT
                   MOVE    SPACES                TO    WS-NT-TEXT
                   STRING    "Seu veículo " WS-PLATE " ficará pronto até " WS-DT-D "/" WS-DT-M "/" WS-DT-Y
                             ". Volte nessa data para retirá-lo."
                       INTO  WS-NT-TEXT
                   PERFORM NOTIFY-BOTH
                   MOVE    "Data de entrega alterada e o cliente avisado."
                                                 TO    QUS-MESSAGE
               END-IF
           END-IF.

      *> HOLD-ORDER-SERVICES: the keys of the next services of quote QUQ-QUO-ID, at most 100, after those already
      *> handled. A statement on QUEUE-FILE ends the rows a SELECT answered, so CHANGE-DEADLINE takes the keys first and
      *> then reads each service again and rewrites it.
      *> In:  QUQ-QUO-ID; WS-HANDLED-COUNT = the services already handled.
      *> Out: WS-HELD-QUEUE-ID (1 to WS-HELD-COUNT); WS-HANDLED-COUNT counts them too; WS-EOF = Y when none is left.
       HOLD-ORDER-SERVICES.
           MOVE    0                             TO    WS-HELD-COUNT
           MOVE    QUQ-QUO-ID                    TO    QUE-QUO-ID
           PERFORM SELECT-QUEUE-OF-QUOTE
           PERFORM FETCH-QUEUE-ROW WS-HANDLED-COUNT TIMES
           PERFORM FETCH-QUEUE-ROW
           PERFORM UNTIL WS-PQ-STATUS NOT = "00" OR WS-HELD-COUNT = 100
               ADD     1                         TO    WS-HELD-COUNT
               MOVE    QUE-ID                    TO    WS-HELD-QUEUE-ID (WS-HELD-COUNT)
               IF    WS-HELD-COUNT < 100
                   PERFORM FETCH-QUEUE-ROW
               END-IF
           END-PERFORM
           ADD     WS-HELD-COUNT                 TO    WS-HANDLED-COUNT
           IF    WS-HELD-COUNT < 100 OR WS-HANDLED-COUNT = WS-PQ-COUNT
               MOVE    "Y"                       TO    WS-EOF
           END-IF.

           COPY "backend/COPYBOOKS/pd-common".
           COPY "backend/COPYBOOKS/pd-pay".
           COPY "backend/COPYBOOKS/pq-quotes".
           COPY "backend/COPYBOOKS/pq-queue".
           COPY "backend/COPYBOOKS/pq-notifications".
           COPY "backend/COPYBOOKS/pq-customers".
           COPY "backend/COPYBOOKS/pq-sequences".
           COPY "backend/COPYBOOKS/pq-audit".

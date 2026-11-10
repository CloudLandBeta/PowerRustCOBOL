       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUEUE-LIST.
      *> Lists the execution queue (R28, R30, R52, R53).
      *> - The rows come ordered by priority (the lower number first) and then by the time the order was approved.
      *> - A blank filter shows what is waiting and running; ALL shows every row; a state shows only that state.
      *> - Each row says whether its whole order is ready for pickup (every service not refunded is DONE), and the
      *>   answer counts the vehicles ready, the services running and the services waiting, for the Dashboard.
      *> The request and the answer are in backend/COPYBOOKS/req-queue-list.cpy and res-queue-list.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "backend/COPYBOOKS/fc-pay".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/payments.FD".
           COPY "COPYBOOKS/installments.FD".
           COPY "COPYBOOKS/refunds.FD".
           COPY "COPYBOOKS/queue.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/customers.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-pay".
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-EOF                                 PIC    X.
       01 WS-I                                   PIC    999.
       01 WS-J                                   PIC    999.
       01 WS-N                                   PIC    999.
       01 WS-KEEP                                PIC    9.
       01 WS-FIRST                               PIC    9.
       01 WS-ROW.
          05 WS-R-ID                             PIC    9(8).
          05 WS-R-QUO                            PIC    9(8).
          05 WS-R-PLATE                          PIC    X(8).
          05 WS-R-SERVICE                        PIC    X(40).
          05 WS-R-STATE                          PIC    X(10).
          05 WS-R-PRIORITY                       PIC    9(2).
          05 WS-R-APPROVED                       PIC    9(14).
          05 WS-R-DEADLINE                       PIC    9(8).
          05 WS-R-READY                          PIC    X(1).
       01 WS-ALL.
          05 WS-A                                OCCURS 200.
             10 WS-A-ID                          PIC    9(8).
             10 WS-A-QUO                         PIC    9(8).
             10 WS-A-PLATE                       PIC    X(8).
             10 WS-A-SERVICE                     PIC    X(40).
             10 WS-A-STATE                       PIC    X(10).
             10 WS-A-PRIORITY                    PIC    9(2).
             10 WS-A-APPROVED                    PIC    9(14).
             10 WS-A-DEADLINE                    PIC    9(8).
             10 WS-A-READY                       PIC    X(1).
       01 WS-ORDER-LEFT                          PIC    9(3).
       01 WS-ORDER-DONE                          PIC    9(3).
       01 WS-ORDER-QUO                           PIC    9(8).

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-queue-list".
           COPY "backend/COPYBOOKS/res-queue-list".

       PROCEDURE DIVISION USING QLN-REQ RETURNING QLN-RES.
       MAIN-PARA.
           INITIALIZE QLN-RES
           MOVE    "OK"                          TO    QLS-STATUS
           MOVE    0                             TO    WS-N

           OPEN    INPUT QUEUE-FILE

           IF    WS-FS-QUE NOT = "00"
               MOVE    "ERROR"                   TO    QLS-STATUS
               MOVE    "Fila indisponível."      TO    QLS-MESSAGE
           ELSE
               PERFORM READ-ALL
               PERFORM MARK-READY
               PERFORM SORT-ALL
               PERFORM FILL-ANSWER
               CLOSE   QUEUE-FILE
           END-IF

           GOBACK.

       READ-ALL.
           MOVE    "N"                           TO    WS-EOF
           PERFORM SELECT-ALL-QUEUE
           IF    WS-PQ-STATUS NOT = "00"
               MOVE    "Y"                       TO    WS-EOF
           END-IF

           PERFORM UNTIL WS-EOF = "Y" OR WS-N = 200
               PERFORM FETCH-QUEUE-ROW
               IF    WS-PQ-STATUS NOT = "00"
                   MOVE    "Y"                   TO    WS-EOF
               ELSE
                   ADD     1                     TO    WS-N
                   MOVE    QUE-ID                TO    WS-A-ID (WS-N)
                   MOVE    QUE-QUO-ID            TO    WS-A-QUO (WS-N)
                   MOVE    QUE-PLATE             TO    WS-A-PLATE (WS-N)
                   MOVE    QUE-SERVICE           TO    WS-A-SERVICE (WS-N)
                   MOVE    QUE-STATE             TO    WS-A-STATE (WS-N)
                   MOVE    QUE-PRIORITY          TO    WS-A-PRIORITY (WS-N)
                   MOVE    QUE-APPROVED-AT       TO    WS-A-APPROVED (WS-N)
                   MOVE    QUE-DEADLINE          TO    WS-A-DEADLINE (WS-N)
                   MOVE    "N"                   TO    WS-A-READY (WS-N)
               END-IF
           END-PERFORM.

      *> MARK-READY: an order is ready when it has a DONE service and none left waiting or running.
       MARK-READY.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               MOVE    WS-A-QUO (WS-I)           TO    WS-ORDER-QUO
               MOVE    0                         TO    WS-ORDER-LEFT
               MOVE    0                         TO    WS-ORDER-DONE
               PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > WS-N
                   IF    WS-A-QUO (WS-J) = WS-ORDER-QUO
                       IF    WS-A-STATE (WS-J) = "DONE"
                           ADD     1             TO    WS-ORDER-DONE
                       END-IF
                       IF    WS-A-STATE (WS-J) = "QUEUED" OR WS-A-STATE (WS-J) = "RUNNING"
                           ADD     1             TO    WS-ORDER-LEFT
                       END-IF
                   END-IF
               END-PERFORM
               IF    WS-ORDER-DONE > 0 AND WS-ORDER-LEFT = 0
                   MOVE    "Y"                   TO    WS-A-READY (WS-I)
               END-IF
           END-PERFORM.

      *> SORT-ALL: priority, then approval time, then number (insertion sort; the queue is small).
       SORT-ALL.
           PERFORM VARYING WS-I FROM 2 BY 1 UNTIL WS-I > WS-N
               MOVE    WS-A (WS-I)               TO    WS-ROW
               MOVE    WS-I                      TO    WS-J
               PERFORM UNTIL WS-J < 2 OR NOT ( WS-R-PRIORITY < WS-A-PRIORITY (WS-J - 1)
                     OR ( WS-R-PRIORITY = WS-A-PRIORITY (WS-J - 1) AND WS-R-APPROVED < WS-A-APPROVED (WS-J - 1) )
                     OR ( WS-R-PRIORITY = WS-A-PRIORITY (WS-J - 1) AND WS-R-APPROVED = WS-A-APPROVED (WS-J - 1)
                          AND WS-R-ID < WS-A-ID (WS-J - 1) ) )
                   MOVE    WS-A (WS-J - 1)       TO    WS-A (WS-J)
                   SUBTRACT 1                    FROM  WS-J
               END-PERFORM
               MOVE    WS-ROW                    TO    WS-A (WS-J)
           END-PERFORM.

       FILL-ANSWER.
           MOVE    0                             TO    QLS-COUNT
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-N
               IF    WS-A-STATE (WS-I) = "RUNNING"
                   ADD     1                     TO    QLS-RUNNING
               END-IF
               IF    WS-A-STATE (WS-I) = "QUEUED"
                   ADD     1                     TO    QLS-QUEUED
               END-IF
               IF    WS-A-READY (WS-I) = "Y"
                   MOVE    1                     TO    WS-FIRST
                   PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J >= WS-I
                       IF    WS-A-QUO (WS-J) = WS-A-QUO (WS-I) AND WS-A-READY (WS-J) = "Y"
                           MOVE    0             TO    WS-FIRST
                       END-IF
                   END-PERFORM
                   IF    WS-FIRST = 1
                       ADD     1                 TO    QLS-READY
                   END-IF
               END-IF

               MOVE    0                         TO    WS-KEEP
               EVALUATE TRUE
                   WHEN QLQ-FILTER = "ALL"
                       MOVE    1                 TO    WS-KEEP
                   WHEN QLQ-FILTER = SPACES
                       IF    WS-A-STATE (WS-I) = "QUEUED" OR WS-A-STATE (WS-I) = "RUNNING"
                           MOVE    1             TO    WS-KEEP
                       END-IF
                   WHEN OTHER
                       IF    WS-A-STATE (WS-I) = QLQ-FILTER
                           MOVE    1             TO    WS-KEEP
                       END-IF
               END-EVALUATE

               IF    WS-KEEP = 1 AND QLS-COUNT < 40
                   ADD     1                     TO    QLS-COUNT
                   MOVE    WS-A-ID (WS-I)        TO    QLR-ID (QLS-COUNT)
                   MOVE    WS-A-QUO (WS-I)       TO    QLR-QUO-ID (QLS-COUNT)
                   MOVE    WS-A-PLATE (WS-I)     TO    QLR-PLATE (QLS-COUNT)
                   MOVE    WS-A-SERVICE (WS-I)   TO    QLR-SERVICE (QLS-COUNT)
                   MOVE    WS-A-STATE (WS-I)     TO    QLR-STATE (QLS-COUNT)
                   MOVE    WS-A-PRIORITY (WS-I)  TO    QLR-PRIORITY (QLS-COUNT)
                   MOVE    WS-A-APPROVED (WS-I)  TO    QLR-APPROVED-AT (QLS-COUNT)
                   MOVE    WS-A-DEADLINE (WS-I)  TO    QLR-DEADLINE (QLS-COUNT)
                   MOVE    WS-A-READY (WS-I)     TO    QLR-READY (QLS-COUNT)
               END-IF
           END-PERFORM.

           COPY "backend/COPYBOOKS/pq-queue".

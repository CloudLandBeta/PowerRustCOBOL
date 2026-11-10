       IDENTIFICATION DIVISION.
       PROGRAM-ID. DEMO-DATA.
      *> Fills the workshop's files with fictitious, consistent data, so every screen and the Dashboard have something
      *> to show: 14 customers, 16 vehicles and 39 quotes over the last six months, in BRL and EUR, in every state a
      *> quote reaches (open, sent, expired, refused, approved, paid in cash or in installments, queued, running, done,
      *> refunded), with diagnoses, approvals, payments, installments (two of them declined), the queue and the messages.
      *> Every record is made by the Backend programs themselves (CUSTOMER-SAVE, VEHICLE-SAVE, DIAG-COLLECT, QUOTE-SAVE,
      *> QUOTE-SEND, QUOTE-APPROVE, PAYMENT-CONFIRM, QUEUE-UPDATE, REFUND-REQUEST, INSTALLMENT-RUN, QUOTE-EXPIRE), so the
      *> data obeys the same rules as the screens; afterwards PartiQL statements move each job back to the day it happened.
      *> The names, documents (valid CPF check digits), plates, chassis numbers, phones and addresses are invented.
      *> A driver like the tests: the programs it calls are added to this file as contained programs by the assembly step.
      *> Run it in the project folder: it writes to data/. Run it once, on files that hold no other demo data.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           COPY "COPYBOOKS/quotes.SEL".
           COPY "COPYBOOKS/approvals.SEL".
           COPY "COPYBOOKS/payments.SEL".
           COPY "COPYBOOKS/installments.SEL".
           COPY "COPYBOOKS/queue.SEL".
           COPY "COPYBOOKS/notifications.SEL".
           COPY "COPYBOOKS/diagnoses.SEL".

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/quotes.FD".
           COPY "COPYBOOKS/approvals.FD".
           COPY "COPYBOOKS/payments.FD".
           COPY "COPYBOOKS/installments.FD".
           COPY "COPYBOOKS/queue.FD".
           COPY "COPYBOOKS/notifications.FD".
           COPY "COPYBOOKS/diagnoses.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-diag".
           COPY "backend/COPYBOOKS/res-diag".
           COPY "backend/COPYBOOKS/req-quote".
           COPY "backend/COPYBOOKS/res-quote".
           COPY "backend/COPYBOOKS/req-quote-send".
           COPY "backend/COPYBOOKS/res-quote-send".
           COPY "backend/COPYBOOKS/req-quote-approve".
           COPY "backend/COPYBOOKS/res-quote-approve".
           COPY "backend/COPYBOOKS/req-payment".
           COPY "backend/COPYBOOKS/res-payment".
           COPY "backend/COPYBOOKS/req-queue-update".
           COPY "backend/COPYBOOKS/res-queue-update".
           COPY "backend/COPYBOOKS/req-refund".
           COPY "backend/COPYBOOKS/res-refund".
           COPY "backend/COPYBOOKS/req-inst-run".
           COPY "backend/COPYBOOKS/res-inst-run".
           COPY "backend/COPYBOOKS/req-quote-expire".
           COPY "backend/COPYBOOKS/res-quote-expire".
      *> One line of the data below, its fields separated by "|".
       01 WS-LINE                                PIC    X(240).
      *> The ids CUSTOMER-SAVE gave, by the customer's number in the list, and the plates by the vehicle's number.
       01 WS-CUSTOMERS.
          05 WS-CUS-ID                           PIC    9(8)    OCCURS 20.
       01 WS-VEHICLES.
          05 WS-VEH-PLATE                        PIC    X(8)    OCCURS 20.
       01 WS-NCUS                                PIC    99      VALUE 0.
       01 WS-NVEH                                PIC    99      VALUE 0.
       01 WS-OWNER                               PIC    X(2).
       01 WS-IDX                                 PIC    99.
       01 WS-NUMBER-TEXT                         PIC    X(10).
      *> The job being made: vehicle, currency, age in days, fate, installments, approval channel, diagnosis, declined
      *> installment, and up to three lines as KIND:CODE:QUANTITY.
       01 WS-JOB.
          05 WS-J-VEH                            PIC    X(2).
          05 WS-J-CUR                            PIC    X(3).
          05 WS-J-AGE                            PIC    X(3).
          05 WS-J-FATE                           PIC    X(2).
          05 WS-J-INST                           PIC    X(2).
          05 WS-J-CHAN                           PIC    X(1).
          05 WS-J-DIAG                           PIC    X(1).
          05 WS-J-FAIL                           PIC    X(1).
          05 WS-J-ITEM                           PIC    X(30)   OCCURS 3.
       01 WS-AGE                                 PIC    999.
       01 WS-QTY-TEXT                            PIC    X(6).
       01 WS-L                                   PIC    9.
       01 WS-DGN-ID                              PIC    9(8).
       01 WS-CODE                                PIC    X(6).
      *> The day the job happened, as a date and as the YYYYMMDDHHMMSS stamps the files keep.
       01 WS-TODAY                               PIC    9(8).
       01 WS-TODAY-INT                           PIC    9(8).
       01 WS-DAY                                 PIC    9(8).
       01 WS-DAY-INT                             PIC    9(8).
       01 WS-STAMP                               PIC    9(14).
       01 WS-STAMP-NEXT                          PIC    9(14).
       01 WS-STAMP-DONE                          PIC    9(14).
       01 WS-VALID                               PIC    9(8).
       01 WS-DEADLINE                            PIC    9(8).
       01 WS-NEW-DUE                             PIC    9(8).
       01 WS-PAID-AT                             PIC    9(14).
       01 WS-INS-STATUS                          PIC    X(10).
       01 WS-LAST-PAST                           PIC    99.
       01 WS-ALL-PAID                            PIC    X.
       01 WS-PAY-STATUS                          PIC    X(10).
       01 WS-CLOCK.
          05 WS-CLOCK-DATE                       PIC    9(8).
          05 WS-CLOCK-REST                       PIC    X(13).
      *> PartiQL: the file a statement runs on, the statement, its key argument, and what it answered.
       01 WS-PQ-FILE                             PIC    X(20).
       01 WS-PQ-SQL                              PIC    X(200).
       01 WS-PQ-KEY                              PIC    9(8).
       01 WS-PQ-COUNT                            PIC    9(6).
       01 WS-PQ-STATUS                           PIC    XX.
       01 WS-PQ-ROW                              PIC    X(200).
       01 WS-PQ-ERROR                            PIC    X(200).
      *> The rows a SELECT answered, kept before the same file is updated (a statement replaces the file's result set).
       01 WS-ROWS.
          05 WS-ROW                              PIC    X(80)   OCCURS 20.
       01 WS-NROW                                PIC    99.
       01 WS-R                                   PIC    99.
       01 WS-ROW-ID                              PIC    9(8).
       01 WS-ROW-NO                              PIC    99.
       01 WS-ROW-DUE                             PIC    9(8).
       01 WS-ROW-STARTED                         PIC    9(14).
       01 WS-ROW-DONE                            PIC    9(14).
      *> Counts for the result block.
       01 WS-OK-CUS                              PIC    999     VALUE 0.
       01 WS-OK-VEH                              PIC    999     VALUE 0.
       01 WS-OK-QUO                              PIC    999     VALUE 0.
       01 WS-OK-DGN                              PIC    999     VALUE 0.
       01 WS-OK-PAY                              PIC    999     VALUE 0.
       01 WS-OK-MOVE                             PIC    999     VALUE 0.
       01 WS-OK-DATE                             PIC    9(4)    VALUE 0.
       01 WS-ERRORS                              PIC    999     VALUE 0.
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-T2                                  PIC    9(8).
       01 WS-T3                                  PIC    9(8).
       01 WS-PH-PEOPLE                           PIC    9(8).
       01 WS-PH-JOBS                             PIC    9(8).
       01 WS-PH-CLOSE                            PIC    9(8).
       01 WS-RATE                                PIC    9(6).
       01 WS-OPERATOR                            PIC    X(30)   VALUE "Maria Atendente".
       01 WS-MECHANIC                            PIC    X(30)   VALUE "João Mecânico".

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME
           MOVE    FUNCTION CURRENT-DATE         TO    WS-CLOCK
           MOVE    WS-CLOCK-DATE                 TO    WS-TODAY
           COMPUTE WS-TODAY-INT = FUNCTION INTEGER-OF-DATE ( WS-TODAY )

           PERFORM LOAD-CUSTOMERS
           PERFORM LOAD-VEHICLES
           ACCEPT  WS-T1                         FROM  TIME

           PERFORM LOAD-JOBS
           ACCEPT  WS-T2                         FROM  TIME

      *> The installments left due on the two plans marked F are charged now and the gateway declines them, so the
      *> consultant sees failed installments; then the sent quotes past their validity are marked expired.
           INITIALIZE IRN-REQ
           MOVE    WS-TODAY                      TO    IRQ-ASOF
           MOVE    "D"                           TO    IRQ-GATEWAY
           MOVE    WS-OPERATOR                   TO    IRQ-OPERATOR
           CALL    "INSTALLMENT-RUN" USING IRN-REQ RETURNING IRN-RES

           INITIALIZE QEX-REQ
           MOVE    WS-OPERATOR                   TO    QEX-OPERATOR
           CALL    "QUOTE-EXPIRE" USING QEX-REQ RETURNING QEX-RES
           ACCEPT  WS-T3                         FROM  TIME

           COMPUTE WS-PH-PEOPLE = WS-T1 - WS-T0
           COMPUTE WS-PH-JOBS = WS-T2 - WS-T1
           COMPUTE WS-PH-CLOSE = WS-T3 - WS-T2
           IF    WS-PH-JOBS > 0
               COMPUTE WS-RATE = WS-OK-QUO * 100 / WS-PH-JOBS
           ELSE
               MOVE    WS-OK-QUO                 TO    WS-RATE
           END-IF

           DISPLAY "Demo data for PowerOficina (DEMO-DATA)"
           DISPLAY "  made             customers " WS-OK-CUS " of 14, vehicles " WS-OK-VEH " of 16, quotes " WS-OK-QUO
                   " of 39, diagnoses " WS-OK-DGN ", payments " WS-OK-PAY ", queue moves " WS-OK-MOVE
           DISPLAY "  dated back       " WS-OK-DATE " records moved to the day they happened (PartiQL UPDATE)"
           DISPLAY "  installment run  charged " IRS-CHARGED ", declined " IRS-FAILED " (gateway D)"
           DISPLAY "  expired          " QEY-COUNT " sent quotes past their validity"
           DISPLAY "  customers        " WS-PH-PEOPLE " hundredths of a second (customers and vehicles)"
           DISPLAY "  jobs             " WS-PH-JOBS " hundredths of a second, " WS-RATE " quotes per second"
           DISPLAY "  closing runs     " WS-PH-CLOSE " hundredths of a second"
           DISPLAY "  errors           " WS-ERRORS
           IF    WS-ERRORS = 0
               DISPLAY "  result           PASS"
           ELSE
               DISPLAY "  result           FAIL"
           END-IF
           STOP RUN.

      *> LOAD-CUSTOMERS: name | CPF | phone | WhatsApp | email | consent to email | consent to WhatsApp | address.
       LOAD-CUSTOMERS.
           MOVE "Ana Paula Ferreira|52601815906|(11) 3456-1201|(11) 98765-1201|ana.ferreira@exemplo.com.br|S|S|Rua das Acácias, 120 - Moema, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Bruno Carvalho Lima|08301661305|(11) 3456-1202|(11) 98765-1202|bruno.lima@exemplo.com.br|S|N|Av. Ibirapuera, 2045 - Indianópolis, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Carla Mendes Rocha|18609139034|(11) 3456-1203|(11) 98765-1203|carla.rocha@exemplo.com.br|S|S|Rua Bela Cintra, 870 - Consolação, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Diego Souza Martins|99603082430|(11) 3456-1204|(11) 98765-1204|diego.martins@exemplo.com.br|N|S|Rua Vergueiro, 3300 - Vila Mariana, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Eduarda Nogueira Prado|62819482112|(11) 3456-1205|(11) 98765-1205|eduarda.prado@exemplo.com.br|S|S|Alameda Santos, 455 - Jardim Paulista, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Felipe Andrade Costa|99351819019|(11) 3456-1206|(11) 98765-1206|felipe.costa@exemplo.com.br|S|N|Rua Augusta, 1508 - Cerqueira César, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Gabriela Ribeiro Alves|93786579741|(11) 3456-1207|(11) 98765-1207|gabriela.alves@exemplo.com.br|S|S|Rua Harmonia, 233 - Vila Madalena, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Henrique Batista Moura|54323194897|(11) 3456-1208|(11) 98765-1208|henrique.moura@exemplo.com.br|N|S|Av. Rebouças, 1290 - Pinheiros, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Isabela Fontes Ramos|75749118606|(11) 3456-1209|(11) 98765-1209|isabela.ramos@exemplo.com.br|S|S|Rua Tuim, 610 - Moema, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "João Victor Teixeira|25276018987|(11) 3456-1210|(11) 98765-1210|joao.teixeira@exemplo.com.br|S|S|Rua Domingos de Morais, 1702 - Vila Mariana, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Larissa Gomes Duarte|55597971115|(11) 3456-1211|(11) 98765-1211|larissa.duarte@exemplo.com.br|S|N|Rua Oscar Freire, 980 - Jardins, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Marcelo Pires Barbosa|47104974601|(11) 3456-1212|(11) 98765-1212|marcelo.barbosa@exemplo.com.br|S|S|Av. Santo Amaro, 4100 - Brooklin, São Paulo/SP" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Tiago Moreira Santos|50752917080|+351 21 345 6713|+351 912 345 613|tiago.santos@exemplo.pt|S|S|Rua Augusta, 145 - Baixa, Lisboa, Portugal" TO WS-LINE
           PERFORM ADD-CUSTOMER
           MOVE "Inês Carvalho Silva|34236671255|+351 22 345 6714|+351 912 345 614|ines.silva@exemplo.pt|S|N|Rua de Santa Catarina, 312 - Porto, Portugal" TO WS-LINE
           PERFORM ADD-CUSTOMER.

       ADD-CUSTOMER.
           INITIALIZE CUS-REQ
           UNSTRING WS-LINE DELIMITED BY "|" INTO CRQ-NAME CRQ-DOC CRQ-PHONE CRQ-WHATSAPP CRQ-EMAIL CRQ-CONSENT-EMAIL
                    CRQ-CONSENT-WA CRQ-ADDRESS
           MOVE    WS-OPERATOR                   TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES

           ADD     1                             TO    WS-NCUS
           MOVE    CRS-ID                        TO    WS-CUS-ID (WS-NCUS)
           IF    CRS-STATUS = "OK"
               ADD     1                         TO    WS-OK-CUS
           ELSE
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  customer refused: " CRQ-NAME " " CRS-STATUS " " CRS-FIELD " " CRS-MESSAGE
           END-IF.

      *> LOAD-VEHICLES: plate | owner (customer number above) | chassis | brand | model | year | colour | kilometres.
       LOAD-VEHICLES.
           MOVE "BRA2E19|01|9BWAB45U0KT000101|Volkswagen|Gol 1.6 MSI|2019|Prata|84210" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "RIO4F32|02|9BGKS48T0MG000102|Chevrolet|Onix 1.0 Turbo|2021|Branco|41530" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "SPX7A45|03|9BD358A40NY000103|Fiat|Argo 1.3 Drive|2022|Vermelho|28770" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "QWE1B23|04|9BHBG51CAKP000104|Hyundai|HB20 1.0 Comfort|2019|Preto|96400" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "MGT3C88|05|9BRBD3HE5L0000105|Toyota|Corolla 2.0 XEi|2020|Cinza|63120" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "PRN5D61|06|93HFC2630JZ000106|Honda|Civic 2.0 EXL|2018|Azul|112300" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "SCA9E07|07|93Y4SRF84NJ000107|Renault|Kwid 1.0 Zen|2022|Laranja|21980" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "BAH2F44|08|98861115XLK000108|Jeep|Renegade 1.8 Longitude|2020|Verde|74650" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "CEA6G15|09|9BFZH55L8J8000109|Ford|Ka 1.5 SE|2018|Prata|101200" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "GOI8H72|10|94DFCAP15MB000110|Nissan|Kicks 1.6 SV|2021|Branco|38900" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "PAR1J39|11|9BD226127P7000111|Fiat|Toro 2.0 Diesel|2023|Cinza|19870" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "AMZ4K56|12|9BWBH6BF0P4000112|Volkswagen|T-Cross 1.0 TSI|2023|Azul|15420" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "TOC7L83|02|9BGEB76H0NB000113|Chevrolet|Tracker 1.2 Turbo|2022|Preto|33310" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "ESP0M21|05|8AJBA3CD5M1000114|Toyota|Hilux 2.8 SRX|2021|Branco|88760" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "LIS3N64|13|VR3UHZKXZPT000115|Peugeot|208 1.2 Active|2023|Amarelo|12340" TO WS-LINE
           PERFORM ADD-VEHICLE
           MOVE "PTG5P90|14|VF15RHA0D6Z000116|Renault|Clio 1.0 TCe|2021|Vermelho|47700" TO WS-LINE
           PERFORM ADD-VEHICLE.

       ADD-VEHICLE.
           INITIALIZE VEH-REQ
           UNSTRING WS-LINE DELIMITED BY "|" INTO VRQ-PLATE WS-OWNER VRQ-CHASSIS VRQ-BRAND VRQ-MODEL WS-NUMBER-TEXT
                    VRQ-COLOR WS-QTY-TEXT
           COMPUTE VRQ-YEAR = FUNCTION NUMVAL ( WS-NUMBER-TEXT )
           COMPUTE VRQ-MILEAGE = FUNCTION NUMVAL ( WS-QTY-TEXT )
           COMPUTE WS-IDX = FUNCTION NUMVAL ( WS-OWNER )
           MOVE    WS-CUS-ID (WS-IDX)            TO    VRQ-CUS-ID
           MOVE    WS-OPERATOR                   TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES

           ADD     1                             TO    WS-NVEH
           MOVE    VRS-PLATE                     TO    WS-VEH-PLATE (WS-NVEH)
           IF    VRS-STATUS = "OK"
               ADD     1                         TO    WS-OK-VEH
           ELSE
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  vehicle refused: " VRQ-PLATE " " VRS-STATUS " " VRS-FIELD " " VRS-MESSAGE
           END-IF.

      *> LOAD-JOBS: vehicle | currency | days ago | fate | installments | channel | diagnosis | declined | lines.
      *> Fate: OP open, SW sent and waiting, SX sent and left to expire, RF refused, AP approved and not paid, PQ paid
      *> and queued, PR paid and running, PD paid and done, RD paid and refunded. Channel: L the customer's link, C the
      *> counter, P phone, W WhatsApp. Diagnosis S: collected from the car first. Declined F: one installment that fell
      *> due is left for the gateway to decline. A line is KIND:CODE:QUANTITY (P part, S service, A add-on).
       LOAD-JOBS.
           MOVE "01|BRL|170|PD|01|C|S|N|P:PAD-FREIO:1|P:DISCO-FREIO:1|S:SVC-FREIO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "02|BRL|165|RF|01|P|N|N|P:BATERIA:1|S:SVC-BATERIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "03|BRL|160|PD|03|L|S|N|P:VELA:1|P:BOBINA:1|S:SVC-IGNICAO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "04|BRL|150|SX|01|L|N|N|P:FILTRO-OLEO:1|P:OLEO-5W30:4|A:ADD-OLEO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "05|BRL|140|PD|01|W|N|N|P:AMORTECEDOR:2|S:SVC-SUSP:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "06|BRL|132|PD|06|L|N|N|P:CORREIA-DENT:1|P:BOMBA-AGUA:1|S:SVC-CORREIA:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "07|BRL|125|RF|01|W|S|N|P:SENSOR-MAP:1|S:SVC-ADMISSAO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "08|BRL|118|PD|01|C|S|N|P:SENSOR-O2:1|P:CATALISADOR:1|S:SVC-ESCAPE:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "09|BRL|110|SX|01|L|N|N|P:FILTRO-AR:1|P:FILTRO-CAB:1|A:ADD-FILTROS:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "10|BRL|100|PD|04|L|S|F|P:TERMOSTATO:1|P:BOMBA-AGUA:1|S:SVC-ARREFEC:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "11|BRL|095|RD|01|C|N|N|P:PAD-FREIO:1|S:SVC-FREIO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "12|BRL|088|PD|02|W|N|N|P:ALTERNADOR:1|S:SVC-BATERIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "13|BRL|080|RF|01|P|S|N|P:VALVULA-EGR:1|S:SVC-EGR:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "14|BRL|075|PD|10|L|S|F|P:INJETOR:4|P:FILTRO-COMB:1|S:SVC-INJECAO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "15|EUR|120|PD|01|C|N|N|P:PAD-FREIO:1|S:SVC-FREIO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "16|EUR|070|PD|02|L|N|N|P:FILTRO-OLEO:1|P:OLEO-5W30:4|A:ADD-OLEO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "01|BRL|066|PD|01|C|N|N|P:FILTRO-OLEO:1|P:OLEO-5W30:4|A:ADD-OLEO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "02|BRL|060|SX|01|L|N|N|P:BATERIA:1|S:SVC-BATERIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "03|BRL|055|PD|02|L|N|N|P:AMORTECEDOR:2|S:SVC-SUSP:1|A:ADD-LAVAGEM:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "04|BRL|048|PD|01|W|S|N|P:VELA:1|S:SVC-IGNICAO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "05|BRL|042|RF|01|W|N|N|P:CORREIA-DENT:1|S:SVC-CORREIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "06|BRL|038|PD|03|C|N|N|P:PAD-FREIO:1|P:DISCO-FREIO:1|S:SVC-FREIO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "07|BRL|033|AP|01|C|N|N|P:FILTRO-AR:1|A:ADD-FILTROS:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "15|EUR|030|RF|01|P|N|N|P:BATERIA:1|S:SVC-BATERIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "08|BRL|028|PD|01|L|N|N|P:FILTRO-OLEO:1|P:OLEO-5W30:4|A:ADD-LAVAGEM:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "09|BRL|024|RD|02|W|S|N|P:SENSOR-ROT:1|S:SVC-IGNICAO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "10|BRL|020|PR|01|C|S|N|P:BATERIA:1|S:SVC-BATERIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "11|BRL|016|PD|05|L|N|N|P:CATALISADOR:1|S:SVC-ESCAPE:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "12|BRL|012|PR|02|W|N|N|P:AMORTECEDOR:2|S:SVC-SUSP:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "13|BRL|009|PR|01|C|S|N|P:VELA:1|P:BOBINA:1|S:SVC-IGNICAO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "14|BRL|007|PQ|03|L|N|N|P:PAD-FREIO:1|S:SVC-FREIO:1|A:ADD-CARPETE:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "06|BRL|006|AP|01|C|N|N|P:TERMOSTATO:1|S:SVC-ARREFEC:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "16|EUR|005|SW|01|L|N|N|P:VELA:1|S:SVC-IGNICAO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "01|BRL|004|SW|01|L|S|N|P:SENSOR-O2:1|S:SVC-ESCAPE:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "02|BRL|003|SW|01|W|N|N|P:FILTRO-AR:1|P:FILTRO-CAB:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "09|BRL|002|SW|01|L|N|N|P:CORREIA-DENT:1|S:SVC-CORREIA:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "03|BRL|002|PQ|01|W|N|N|P:FILTRO-OLEO:1|P:OLEO-5W30:4|A:ADD-OLEO:1" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "04|BRL|001|OP|01|C|N|N|P:PAD-FREIO:1|S:SVC-FREIO:1|" TO WS-LINE
           PERFORM RUN-JOB
           MOVE "05|BRL|000|OP|01|C|N|N|P:BATERIA:1||" TO WS-LINE
           PERFORM RUN-JOB.

      *> RUN-JOB: one quote, from the diagnosis to where its fate leaves it, then moved back to the day it happened.
       RUN-JOB.
           INITIALIZE WS-JOB
           UNSTRING WS-LINE DELIMITED BY "|" INTO WS-J-VEH WS-J-CUR WS-J-AGE WS-J-FATE WS-J-INST WS-J-CHAN WS-J-DIAG
                    WS-J-FAIL WS-J-ITEM (1) WS-J-ITEM (2) WS-J-ITEM (3)
           COMPUTE WS-IDX = FUNCTION NUMVAL ( WS-J-VEH )
           COMPUTE WS-AGE = FUNCTION NUMVAL ( WS-J-AGE )
           COMPUTE WS-DAY-INT = WS-TODAY-INT - WS-AGE
           COMPUTE WS-DAY = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT )
           COMPUTE WS-STAMP = WS-DAY * 1000000 + 93000
           COMPUTE WS-STAMP-NEXT = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT + 1 ) * 1000000 + 141500
           COMPUTE WS-STAMP-DONE = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT + 3 ) * 1000000 + 170000

           MOVE    0                             TO    WS-DGN-ID
           IF    WS-J-DIAG = "S"
               PERFORM COLLECT-DIAGNOSIS
           END-IF

           PERFORM MAKE-QUOTE
           IF    QTS-STATUS NOT = "OK"
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  quote refused: " WS-LINE (1:40) " " QTS-STATUS " " QTS-FIELD " " QTS-MESSAGE
           ELSE
               ADD     1                         TO    WS-OK-QUO
               EVALUATE WS-J-FATE
                   WHEN "SW"
                   WHEN "SX"
                       PERFORM SEND-QUOTE
                   WHEN "RF"
                       PERFORM SEND-QUOTE
                       PERFORM ANSWER-QUOTE
                   WHEN "AP"
                   WHEN "PQ"
                   WHEN "PR"
                   WHEN "PD"
                   WHEN "RD"
                       PERFORM SEND-QUOTE
                       PERFORM ANSWER-QUOTE
               END-EVALUATE

               IF    WS-J-FATE = "PQ" OR WS-J-FATE = "PR" OR WS-J-FATE = "PD" OR WS-J-FATE = "RD"
                   PERFORM PAY-QUOTE
                   PERFORM MOVE-QUEUE
               END-IF

               PERFORM DATE-QUOTE
               PERFORM DATE-APPROVALS
               PERFORM DATE-PAYMENT
               PERFORM DATE-QUEUE
               PERFORM DATE-NOTIFICATIONS
           END-IF.

      *> COLLECT-DIAGNOSIS: the simulated reading of the car's sensors and fault codes (mode S of DIAG-COLLECT).
       COLLECT-DIAGNOSIS.
           INITIALIZE DGC-REQ
           MOVE    WS-VEH-PLATE (WS-IDX)         TO    DCQ-PLATE
           MOVE    WS-OPERATOR                   TO    DCQ-OPERATOR
           MOVE    "S"                           TO    DCQ-MODE
           CALL    "DIAG-COLLECT" USING DGC-REQ RETURNING DGC-RES

           IF    DCS-STATUS = "OK"
               ADD     1                         TO    WS-OK-DGN
               MOVE    DCS-ID                    TO    WS-DGN-ID
           ELSE
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  diagnosis refused: " DCQ-PLATE " " DCS-STATUS " " DCS-FIELD " " DCS-MESSAGE
           END-IF.

       MAKE-QUOTE.
           INITIALIZE QUO-REQ
           MOVE    WS-VEH-PLATE (WS-IDX)         TO    QRQ-PLATE
           MOVE    WS-J-CUR                      TO    QRQ-CURRENCY
           MOVE    WS-DGN-ID                     TO    QRQ-DGN-ID
           MOVE    WS-OPERATOR                   TO    QRQ-OPERATOR
           PERFORM VARYING WS-L FROM 1 BY 1 UNTIL WS-L > 3
               IF    WS-J-ITEM (WS-L) NOT = SPACES
                   ADD     1                     TO    QRQ-COUNT
                   UNSTRING WS-J-ITEM (WS-L) DELIMITED BY ":" INTO QRL-KIND (QRQ-COUNT) QRL-CODE (QRQ-COUNT)
                            WS-QTY-TEXT
                   COMPUTE QRL-QTY (QRQ-COUNT) = FUNCTION NUMVAL ( WS-QTY-TEXT )
               END-IF
           END-PERFORM
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES.

       SEND-QUOTE.
           INITIALIZE QSN-REQ
           MOVE    QTS-ID                        TO    QSN-ID
           MOVE    WS-OPERATOR                   TO    QSN-OPERATOR
           CALL    "QUOTE-SEND" USING QSN-REQ RETURNING QSN-RES

           MOVE    QNS-CODE                      TO    WS-CODE
           IF    QNS-STATUS NOT = "OK"
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  sending refused: " QTS-ID " " QNS-STATUS " " QNS-MESSAGE
           END-IF.

      *> ANSWER-QUOTE: the customer's answer, through the link (with the code the message carried) or registered by the
      *> consultant; a refusal gives the reason the customer gave.
       ANSWER-QUOTE.
           INITIALIZE QAP-REQ
           MOVE    QTS-ID                        TO    QAP-QUO-ID
           IF    WS-J-CHAN = "L"
               MOVE    "L"                       TO    QAP-MODE
               MOVE    WS-CODE                   TO    QAP-CODE
           ELSE
               MOVE    "M"                       TO    QAP-MODE
               MOVE    WS-OPERATOR               TO    QAP-OPERATOR
               EVALUATE WS-J-CHAN
                   WHEN "C"
                       MOVE    "COUNTER"         TO    QAP-CHANNEL
                   WHEN "P"
                       MOVE    "PHONE"           TO    QAP-CHANNEL
                   WHEN OTHER
                       MOVE    "WHATSAPP"        TO    QAP-CHANNEL
               END-EVALUATE
           END-IF

           IF    WS-J-FATE = "RF"
               MOVE    "R"                       TO    QAP-DECISION
               MOVE    "Achou o valor alto; vai pensar e volta a ligar" TO QAP-REASON
           ELSE
               MOVE    "A"                       TO    QAP-DECISION
           END-IF
           CALL    "QUOTE-APPROVE" USING QAP-REQ RETURNING QAP-RES

           IF    QAS-STATUS NOT = "OK"
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  answer refused: " QTS-ID " " QAS-STATUS " " QAS-FIELD " " QAS-MESSAGE
           END-IF.

      *> PAY-QUOTE: the first installment charged at the gateway (simulated, approved), with the customer's consent to
      *> the charges to come.
       PAY-QUOTE.
           INITIALIZE PCF-REQ
           MOVE    "C"                           TO    PCQ-MODE
           MOVE    QTS-ID                        TO    PCQ-QUO-ID
           COMPUTE PCQ-INSTALLMENTS = FUNCTION NUMVAL ( WS-J-INST )
           MOVE    "S"                           TO    PCQ-CONSENT
           EVALUATE TRUE
               WHEN WS-AGE > 100
                   MOVE    "VISA"                TO    PCQ-BRAND
                   MOVE    "4242"                TO    PCQ-LAST4
               WHEN WS-AGE > 40
                   MOVE    "MASTERCARD"          TO    PCQ-BRAND
                   MOVE    "5454"                TO    PCQ-LAST4
               WHEN OTHER
                   MOVE    "ELO"                 TO    PCQ-BRAND
                   MOVE    "6363"                TO    PCQ-LAST4
           END-EVALUATE
           MOVE    WS-OPERATOR                   TO    PCQ-OPERATOR
           CALL    "PAYMENT-CONFIRM" USING PCF-REQ RETURNING PCF-RES

           IF    PCS-STATUS = "OK"
               ADD     1                         TO    WS-OK-PAY
           ELSE
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  payment refused: " QTS-ID " " PCS-STATUS " " PCS-FIELD " " PCS-MESSAGE
           END-IF.

      *> MOVE-QUEUE: the order's services started (PR), started and finished (PD), or the first one refunded (RD).
       MOVE-QUEUE.
           MOVE    "QUEUE-FILE"                  TO    WS-PQ-FILE
           MOVE    "SELECT QUE-ID FROM QUEUE-FILE.QUE-QUO-ID WHERE QUE-QUO-ID = ?" TO WS-PQ-SQL
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           OPEN    INPUT QUEUE-FILE
           PERFORM COLLECT-ROWS
           CLOSE   QUEUE-FILE

           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               COMPUTE WS-ROW-ID = FUNCTION NUMVAL ( WS-ROW (WS-R) )
               INITIALIZE QUN-REQ
               MOVE    WS-ROW-ID                 TO    QUQ-QUE-ID
               MOVE    WS-MECHANIC               TO    QUQ-OPERATOR
               EVALUATE WS-J-FATE
                   WHEN "PR"
                       MOVE    "S"               TO    QUQ-ACTION
                       PERFORM QUEUE-STEP
                   WHEN "PD"
                       MOVE    "S"               TO    QUQ-ACTION
                       PERFORM QUEUE-STEP
                       MOVE    "F"               TO    QUQ-ACTION
                       PERFORM QUEUE-STEP
                   WHEN "RD"
                       IF    WS-R = 1
                           PERFORM REFUND-SERVICE
                       END-IF
               END-EVALUATE
           END-PERFORM.

       QUEUE-STEP.
           CALL    "QUEUE-UPDATE" USING QUN-REQ RETURNING QUN-RES
           IF    QUS-STATUS = "OK"
               ADD     1                         TO    WS-OK-MOVE
           ELSE
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  queue move refused: " QUQ-QUE-ID " " QUQ-ACTION " " QUS-STATUS " " QUS-MESSAGE
           END-IF.

       REFUND-SERVICE.
           INITIALIZE RFN-REQ
           MOVE    WS-ROW-ID                     TO    RFQ-QUE-ID
           MOVE    "Cliente"                     TO    RFQ-BY
           MOVE    "Vendeu o carro antes do serviço" TO RFQ-REASON
           CALL    "REFUND-REQUEST" USING RFN-REQ RETURNING RFN-RES
           IF    RFS-STATUS NOT = "OK"
               ADD     1                         TO    WS-ERRORS
               DISPLAY "  refund refused: " RFQ-QUE-ID " " RFS-STATUS " " RFS-MESSAGE
           END-IF.

      *> COLLECT-ROWS: runs WS-PQ-SQL on WS-PQ-FILE (open) with WS-PQ-KEY and keeps up to 20 answered rows in WS-ROW.
       COLLECT-ROWS.
           MOVE    0                             TO    WS-NROW
           COBOL::"PARTIQL" ( WS-PQ-FILE WS-PQ-SQL WS-PQ-COUNT WS-PQ-STATUS WS-PQ-KEY )
           IF    WS-PQ-STATUS NOT = "00"
               PERFORM PARTIQL-FAILED
           END-IF

           PERFORM UNTIL WS-PQ-STATUS NOT = "00" OR WS-NROW = 20
               COBOL::"PARTIQL-FETCH" ( WS-PQ-FILE WS-PQ-ROW WS-PQ-STATUS )
               IF    WS-PQ-STATUS = "00"
                   ADD     1                     TO    WS-NROW
                   MOVE    WS-PQ-ROW             TO    WS-ROW (WS-NROW)
               END-IF
           END-PERFORM.

      *> CHECK-UPDATE: counts an UPDATE that ran, and reports one that did not.
       CHECK-UPDATE.
           IF    WS-PQ-STATUS = "00"
               ADD     WS-PQ-COUNT               TO    WS-OK-DATE
           ELSE
               PERFORM PARTIQL-FAILED
           END-IF.

       PARTIQL-FAILED.
           COBOL::"PARTIQL-ERROR" ( WS-PQ-FILE WS-PQ-ERROR )
           ADD     1                             TO    WS-ERRORS
           DISPLAY "  PartiQL " WS-PQ-STATUS " on " WS-PQ-FILE ": " WS-PQ-ERROR (1:120).

      *> DATE-QUOTE: the quote was made on the job's day and valid for 15 days, 10 for one left to expire; its diagnosis
      *> was read half an hour before. An open or waiting quote is recent, so it is still valid.
       DATE-QUOTE.
           IF    WS-J-FATE = "SX"
               COMPUTE WS-VALID = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT + 10 )
           ELSE
               COMPUTE WS-VALID = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT + 15 )
           END-IF
      *> The quote's number and revision are moved to working items first: as part of the answer record QUOTE-SAVE
      *> filled, they would reach PartiQL as text, and a text never equals the numeric key.
           OPEN    I-O QUOTE-FILE
           MOVE    "QUOTE-FILE"                  TO    WS-PQ-FILE
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           MOVE    QTS-REV                       TO    WS-ROW-NO
           COBOL::"PARTIQL" ( "QUOTE-FILE"
                   "UPDATE QUOTE-FILE SET QUO-CREATED = ? SET QUO-VALID-UNTIL = ? WHERE QUO-ID = ? AND QUO-REV = ?"
                   WS-PQ-COUNT WS-PQ-STATUS WS-STAMP WS-VALID WS-PQ-KEY WS-ROW-NO )
           PERFORM CHECK-UPDATE
           CLOSE   QUOTE-FILE

           IF    WS-DGN-ID > 0
               OPEN    I-O DIAGNOSIS-FILE
               MOVE    "DIAGNOSIS-FILE"          TO    WS-PQ-FILE
               COMPUTE WS-PAID-AT = WS-STAMP - 3000
               COBOL::"PARTIQL" ( "DIAGNOSIS-FILE" "UPDATE DIAGNOSIS-FILE SET DGN-AT = ? WHERE DGN-ID = ?"
                       WS-PQ-COUNT WS-PQ-STATUS WS-PAID-AT WS-DGN-ID )
               PERFORM CHECK-UPDATE
               CLOSE   DIAGNOSIS-FILE
           END-IF.

      *> DATE-APPROVALS: the customer answered the day after the quote was made.
       DATE-APPROVALS.
           MOVE    "APPROVAL-FILE"               TO    WS-PQ-FILE
           MOVE    "SELECT APR-ID FROM APPROVAL-FILE.APR-QUO-ID WHERE APR-QUO-ID = ?" TO WS-PQ-SQL
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           OPEN    I-O APPROVAL-FILE
           PERFORM COLLECT-ROWS
           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               COMPUTE WS-ROW-ID = FUNCTION NUMVAL ( WS-ROW (WS-R) )
               COBOL::"PARTIQL" ( "APPROVAL-FILE" "UPDATE APPROVAL-FILE SET APR-AT = ? WHERE APR-ID = ?"
                       WS-PQ-COUNT WS-PQ-STATUS WS-STAMP-NEXT WS-ROW-ID )
               PERFORM CHECK-UPDATE
           END-PERFORM
           CLOSE   APPROVAL-FILE.

      *> DATE-PAYMENT: paid the day after the quote; every installment moves back by the same days, and one that has
      *> fallen due since was paid on its day, except the last one of a plan marked F, left due for the gateway to
      *> decline. A plan with every installment paid is PAID.
       DATE-PAYMENT.
           MOVE    "PAYMENT-FILE"                TO    WS-PQ-FILE
           MOVE    "SELECT PAY-ID FROM PAYMENT-FILE.PAY-QUO-ID WHERE PAY-QUO-ID = ?" TO WS-PQ-SQL
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           OPEN    I-O PAYMENT-FILE
           PERFORM COLLECT-ROWS
           CLOSE   PAYMENT-FILE

           IF    WS-NROW > 0
               COMPUTE WS-ROW-ID = FUNCTION NUMVAL ( WS-ROW (1) )
               PERFORM DATE-INSTALLMENTS

               MOVE    "PAYMENT-FILE"            TO    WS-PQ-FILE
               OPEN    I-O PAYMENT-FILE
               IF    WS-ALL-PAID = "Y" AND WS-J-FATE NOT = "RD"
                   MOVE    "PAID"                TO    WS-PAY-STATUS
                   COBOL::"PARTIQL" ( "PAYMENT-FILE"
                           "UPDATE PAYMENT-FILE SET PAY-AT = ? SET PAY-CONSENT-AT = ? SET PAY-STATUS = ? "
                         & "WHERE PAY-ID = ?"
                           WS-PQ-COUNT WS-PQ-STATUS WS-STAMP-NEXT WS-STAMP-NEXT WS-PAY-STATUS WS-ROW-ID )
               ELSE
                   COBOL::"PARTIQL" ( "PAYMENT-FILE"
                           "UPDATE PAYMENT-FILE SET PAY-AT = ? SET PAY-CONSENT-AT = ? WHERE PAY-ID = ?"
                           WS-PQ-COUNT WS-PQ-STATUS WS-STAMP-NEXT WS-STAMP-NEXT WS-ROW-ID )
               END-IF
               PERFORM CHECK-UPDATE
               CLOSE   PAYMENT-FILE
           END-IF.

       DATE-INSTALLMENTS.
           MOVE    "INSTALLMENT-FILE"            TO    WS-PQ-FILE
           MOVE    "SELECT INS-NO, INS-DUE, INS-STATUS FROM INSTALLMENT-FILE WHERE INS-PAY-ID = ?" TO WS-PQ-SQL
           MOVE    WS-ROW-ID                     TO    WS-PQ-KEY
           OPEN    I-O INSTALLMENT-FILE
           PERFORM COLLECT-ROWS

           MOVE    0                             TO    WS-LAST-PAST
           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               UNSTRING WS-ROW (WS-R) DELIMITED BY X"09" INTO WS-ROW-NO WS-ROW-DUE
               IF    FUNCTION INTEGER-OF-DATE ( WS-ROW-DUE ) - WS-AGE + 1 < WS-TODAY-INT
                   MOVE    WS-ROW-NO             TO    WS-LAST-PAST
               END-IF
           END-PERFORM

           MOVE    "Y"                           TO    WS-ALL-PAID
           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               UNSTRING WS-ROW (WS-R) DELIMITED BY X"09" INTO WS-ROW-NO WS-ROW-DUE WS-INS-STATUS
               COMPUTE WS-NEW-DUE = FUNCTION DATE-OF-INTEGER ( FUNCTION INTEGER-OF-DATE ( WS-ROW-DUE ) - WS-AGE + 1 )
               COMPUTE WS-PAID-AT = WS-NEW-DUE * 1000000 + 101500
               IF    WS-ROW-NO = 1
                   MOVE    WS-STAMP-NEXT         TO    WS-PAID-AT
               END-IF
               IF    WS-NEW-DUE < WS-TODAY AND WS-INS-STATUS = "DUE"
                     AND NOT ( WS-J-FAIL = "F" AND WS-ROW-NO = WS-LAST-PAST )
                   MOVE    "PAID"                TO    WS-INS-STATUS
               END-IF
               IF    WS-INS-STATUS NOT = "PAID"
                   MOVE    "N"                   TO    WS-ALL-PAID
                   MOVE    0                     TO    WS-PAID-AT
               END-IF
               COBOL::"PARTIQL" ( "INSTALLMENT-FILE"
                       "UPDATE INSTALLMENT-FILE SET INS-DUE = ? SET INS-STATUS = ? SET INS-PAID-AT = ? "
                     & "WHERE INS-PAY-ID = ? AND INS-NO = ?"
                       WS-PQ-COUNT WS-PQ-STATUS WS-NEW-DUE WS-INS-STATUS WS-PAID-AT WS-ROW-ID WS-ROW-NO )
               PERFORM CHECK-UPDATE
           END-PERFORM
           CLOSE   INSTALLMENT-FILE.

      *> DATE-QUEUE: the services joined the queue when the order was paid, started that day and finished two days
      *> later; the agreed delivery date is five days after the order.
       DATE-QUEUE.
           MOVE    "QUEUE-FILE"                  TO    WS-PQ-FILE
           MOVE    "SELECT QUE-ID, QUE-STARTED, QUE-DONE FROM QUEUE-FILE.QUE-QUO-ID WHERE QUE-QUO-ID = ?" TO WS-PQ-SQL
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           COMPUTE WS-DEADLINE = FUNCTION DATE-OF-INTEGER ( WS-DAY-INT + 5 )
           OPEN    I-O QUEUE-FILE
           PERFORM COLLECT-ROWS
           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               UNSTRING WS-ROW (WS-R) DELIMITED BY X"09" INTO WS-ROW-ID WS-ROW-STARTED WS-ROW-DONE
               IF    WS-ROW-STARTED > 0
                   MOVE    WS-STAMP-NEXT         TO    WS-ROW-STARTED
               END-IF
               IF    WS-ROW-DONE > 0
                   MOVE    WS-STAMP-DONE         TO    WS-ROW-DONE
               END-IF
               COBOL::"PARTIQL" ( "QUEUE-FILE"
                       "UPDATE QUEUE-FILE SET QUE-APPROVED-AT = ? SET QUE-STARTED = ? SET QUE-DONE = ? "
                     & "SET QUE-DEADLINE = ? WHERE QUE-ID = ?"
                       WS-PQ-COUNT WS-PQ-STATUS WS-STAMP-NEXT WS-ROW-STARTED WS-ROW-DONE WS-DEADLINE WS-ROW-ID )
               PERFORM CHECK-UPDATE
           END-PERFORM
           CLOSE   QUEUE-FILE.

      *> DATE-NOTIFICATIONS: the messages to the customer went out on the job's day.
       DATE-NOTIFICATIONS.
           MOVE    "NOTIFY-FILE"                 TO    WS-PQ-FILE
           MOVE    "SELECT NTF-ID FROM NOTIFY-FILE.NTF-QUO-ID WHERE NTF-QUO-ID = ?" TO WS-PQ-SQL
           MOVE    QTS-ID                        TO    WS-PQ-KEY
           OPEN    I-O NOTIFY-FILE
           PERFORM COLLECT-ROWS
           PERFORM VARYING WS-R FROM 1 BY 1 UNTIL WS-R > WS-NROW
               COMPUTE WS-ROW-ID = FUNCTION NUMVAL ( WS-ROW (WS-R) )
               COBOL::"PARTIQL" ( "NOTIFY-FILE" "UPDATE NOTIFY-FILE SET NTF-AT = ? WHERE NTF-ID = ?"
                       WS-PQ-COUNT WS-PQ-STATUS WS-STAMP WS-ROW-ID )
               PERFORM CHECK-UPDATE
           END-PERFORM
           CLOSE   NOTIFY-FILE.

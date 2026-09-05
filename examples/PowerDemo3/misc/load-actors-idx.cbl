       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOAD-ACTORS-IDX.

      *>---------------------------------------------------------------*
      *> LOAD-ACTORS-IDX                                                *
      *>                                                                *
      *> Builds data/idxfiles/actors.idx from the 100 actor rows that   *
      *> main-form.cbl seeds into the ACTORS table.  The record is a    *
      *> plain scalar record -- no OCCURS -- keyed on ACTOR-ID.         *
      *>                                                                *
      *> Run it from the PROJECT ROOT so the relative ASSIGN path       *
      *> resolves:                                                      *
      *>     rcrun run misc/load-actors-idx.cbl                         *
      *>                                                                *
      *> OPEN OUTPUT recreates the file, so re-running is idempotent.   *
      *>---------------------------------------------------------------*

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.

           SELECT ACTORS-FILE
               ASSIGN TO "data/idxfiles/actors.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS RANDOM
               RECORD KEY IS ACTOR-ID
               STORAGE MODE IS DISK
               FILE STATUS IS WS-STATUS.

       DATA DIVISION.
       FILE SECTION.

       FD  ACTORS-FILE
           RECORD CONTAINS 111 CHARACTERS.
       01  ACTORS-RECORD.
           05 ACTOR-ID       PIC 9(9).
           05 ACTOR-THUMB    PIC X(60).
           05 ACTOR-CAPTION  PIC X(30).
           05 ACTOR-SALARY   PIC 9(9)V99.
           05 ACTOR-AWARDS   PIC X.

       WORKING-STORAGE SECTION.
       01  WS-STATUS         PIC XX.
       01  WS-WRITTEN        PIC 9(4) VALUE ZERO.
       01  WS-FAILED         PIC 9(4) VALUE ZERO.
       01  WS-ED             PIC ZZZ9.
      *>---------------------------------------------------------------*
      *> Generated cast (998 rows, ACTOR-ID 101..1098).                 *
      *>                                                                *
      *> Comic parodies of real actors from the US, Brazil, Portugal,   *
      *> Mexico, France, China and Japan -- 16 first names and 16       *
      *> surnames per country, BOTH halves already parodies, so every   *
      *> combination reads as one: "John-John Waves", "Leobardo di      *
      *> Castrol", "Keannes Rivers".                                    *
      *>                                                                *
      *> Combinatorial rather than 998 hand-written MOVEs: 16 x 16 is   *
      *> 256 pairs per country and only ~143 are needed, so the mapping  *
      *> below never repeats a pair within a country.                   *
      *>                                                                *
      *> Pure ASCII on purpose. The record is FIXED at 111 bytes and    *
      *> every field offset is a byte offset, so one accented character  *
      *> would shift the fields after it.                               *
      *>---------------------------------------------------------------*
       01  WS-COUNTRY-TAB.
           05 FILLER PIC X(9) VALUE "US".
           05 FILLER PIC X(9) VALUE "Brazil".
           05 FILLER PIC X(9) VALUE "Portugal".
           05 FILLER PIC X(9) VALUE "Mexico".
           05 FILLER PIC X(9) VALUE "France".
           05 FILLER PIC X(9) VALUE "China".
           05 FILLER PIC X(9) VALUE "Japan".
       01  WS-COUNTRY-R REDEFINES WS-COUNTRY-TAB.
           05 WS-COUNTRY PIC X(9) OCCURS 7.
       01  WS-PER-COUNTRY-TAB.
           05 WS-PER-COUNTRY PIC 9(4) OCCURS 7 VALUE ZERO.
       01  WS-FIRST-TAB.
           05 FILLER PIC X(16) VALUE "John-John".
           05 FILLER PIC X(16) VALUE "Leobardo".
           05 FILLER PIC X(16) VALUE "Keannes".
           05 FILLER PIC X(16) VALUE "Bratt".
           05 FILLER PIC X(16) VALUE "Tomm".
           05 FILLER PIC X(16) VALUE "Robbert".
           05 FILLER PIC X(16) VALUE "Denzil".
           05 FILLER PIC X(16) VALUE "Morgun".
           05 FILLER PIC X(16) VALUE "Jaqk".
           05 FILLER PIC X(16) VALUE "Harryson".
           05 FILLER PIC X(16) VALUE "Silvestro".
           05 FILLER PIC X(16) VALUE "Arnoldo".
           05 FILLER PIC X(16) VALUE "Wille".
           05 FILLER PIC X(16) VALUE "Dwaine".
           05 FILLER PIC X(16) VALUE "Scarlette".
           05 FILLER PIC X(16) VALUE "Meryll".
           05 FILLER PIC X(16) VALUE "Wagnner".
           05 FILLER PIC X(16) VALUE "Rodrigno".
           05 FILLER PIC X(16) VALUE "Fernanda".
           05 FILLER PIC X(16) VALUE "Seltton".
           05 FILLER PIC X(16) VALUE "Lazaro".
           05 FILLER PIC X(16) VALUE "Matheuz".
           05 FILLER PIC X(16) VALUE "Cauan".
           05 FILLER PIC X(16) VALUE "Bruna".
           05 FILLER PIC X(16) VALUE "Alicce".
           05 FILLER PIC X(16) VALUE "Gloria".
           05 FILLER PIC X(16) VALUE "Paolo".
           05 FILLER PIC X(16) VALUE "Tarcisio".
           05 FILLER PIC X(16) VALUE "Antonho".
           05 FILLER PIC X(16) VALUE "Regyna".
           05 FILLER PIC X(16) VALUE "Deborah".
           05 FILLER PIC X(16) VALUE "Murillo".
           05 FILLER PIC X(16) VALUE "Joaquim".
           05 FILLER PIC X(16) VALUE "Diogo".
           05 FILLER PIC X(16) VALUE "Nuno".
           05 FILLER PIC X(16) VALUE "Ricardo".
           05 FILLER PIC X(16) VALUE "Maria".
           05 FILLER PIC X(16) VALUE "Beatrize".
           05 FILLER PIC X(16) VALUE "Rui".
           05 FILLER PIC X(16) VALUE "Vasqo".
           05 FILLER PIC X(16) VALUE "Ineez".
           05 FILLER PIC X(16) VALUE "Tiaguo".
           05 FILLER PIC X(16) VALUE "Leonor".
           05 FILLER PIC X(16) VALUE "Miguelo".
           05 FILLER PIC X(16) VALUE "Catarinha".
           05 FILLER PIC X(16) VALUE "Bruno".
           05 FILLER PIC X(16) VALUE "Soffia".
           05 FILLER PIC X(16) VALUE "Filipe".
           05 FILLER PIC X(16) VALUE "Gaelo".
           05 FILLER PIC X(16) VALUE "Diegho".
           05 FILLER PIC X(16) VALUE "Salmita".
           05 FILLER PIC X(16) VALUE "Yalitzza".
           05 FILLER PIC X(16) VALUE "Demiann".
           05 FILLER PIC X(16) VALUE "Eugenio".
           05 FILLER PIC X(16) VALUE "Kate del".
           05 FILLER PIC X(16) VALUE "Adriann".
           05 FILLER PIC X(16) VALUE "Ana de".
           05 FILLER PIC X(16) VALUE "Alfonzo".
           05 FILLER PIC X(16) VALUE "Marinna".
           05 FILLER PIC X(16) VALUE "Tenoch".
           05 FILLER PIC X(16) VALUE "Karla".
           05 FILLER PIC X(16) VALUE "Emilio".
           05 FILLER PIC X(16) VALUE "Ilse".
           05 FILLER PIC X(16) VALUE "Joaquino".
           05 FILLER PIC X(16) VALUE "Jeann".
           05 FILLER PIC X(16) VALUE "Marionn".
           05 FILLER PIC X(16) VALUE "Vincennt".
           05 FILLER PIC X(16) VALUE "Audrei".
           05 FILLER PIC X(16) VALUE "Omarr".
           05 FILLER PIC X(16) VALUE "Sophi".
           05 FILLER PIC X(16) VALUE "Gerardo".
           05 FILLER PIC X(16) VALUE "Juliette".
           05 FILLER PIC X(16) VALUE "Matthiew".
           05 FILLER PIC X(16) VALUE "Isabellle".
           05 FILLER PIC X(16) VALUE "Louie".
           05 FILLER PIC X(16) VALUE "Adele".
           05 FILLER PIC X(16) VALUE "Guillaume".
           05 FILLER PIC X(16) VALUE "Leaa".
           05 FILLER PIC X(16) VALUE "Tahar".
           05 FILLER PIC X(16) VALUE "Cattherine".
           05 FILLER PIC X(16) VALUE "Jaquie".
           05 FILLER PIC X(16) VALUE "Jett".
           05 FILLER PIC X(16) VALUE "Tonny".
           05 FILLER PIC X(16) VALUE "Ziyii".
           05 FILLER PIC X(16) VALUE "Gongg".
           05 FILLER PIC X(16) VALUE "Andi".
           05 FILLER PIC X(16) VALUE "Donnie".
           05 FILLER PIC X(16) VALUE "Fann".
           05 FILLER PIC X(16) VALUE "Chowe".
           05 FILLER PIC X(16) VALUE "Zhangg".
           05 FILLER PIC X(16) VALUE "Maggy".
           05 FILLER PIC X(16) VALUE "Huang".
           05 FILLER PIC X(16) VALUE "Xun".
           05 FILLER PIC X(16) VALUE "Wenn".
           05 FILLER PIC X(16) VALUE "Takeshi".
           05 FILLER PIC X(16) VALUE "Liuu".
           05 FILLER PIC X(16) VALUE "Toshiroo".
           05 FILLER PIC X(16) VALUE "Kenn".
           05 FILLER PIC X(16) VALUE "Hirokazu".
           05 FILLER PIC X(16) VALUE "Tadanobu".
           05 FILLER PIC X(16) VALUE "Rinko".
           05 FILLER PIC X(16) VALUE "Masaharu".
           05 FILLER PIC X(16) VALUE "Koji".
           05 FILLER PIC X(16) VALUE "Yakusho".
           05 FILLER PIC X(16) VALUE "Satoshi".
           05 FILLER PIC X(16) VALUE "Aoi".
           05 FILLER PIC X(16) VALUE "Takeshi".
           05 FILLER PIC X(16) VALUE "Hiroyuki".
           05 FILLER PIC X(16) VALUE "Machiko".
           05 FILLER PIC X(16) VALUE "Issey".
           05 FILLER PIC X(16) VALUE "Sakura".
           05 FILLER PIC X(16) VALUE "Ryo".
       01  WS-FIRST-R REDEFINES WS-FIRST-TAB.
           05 WS-FIRST PIC X(16) OCCURS 112.
       01  WS-LAST-TAB.
           05 FILLER PIC X(16) VALUE "Waves".
           05 FILLER PIC X(16) VALUE "di Castrol".
           05 FILLER PIC X(16) VALUE "Rivers".
           05 FILLER PIC X(16) VALUE "Pittt".
           05 FILLER PIC X(16) VALUE "Hankz".
           05 FILLER PIC X(16) VALUE "de Nero".
           05 FILLER PIC X(16) VALUE "Washingtown".
           05 FILLER PIC X(16) VALUE "Freemann".
           05 FILLER PIC X(16) VALUE "Nickelson".
           05 FILLER PIC X(16) VALUE "Fjord".
           05 FILLER PIC X(16) VALUE "Stallonne".
           05 FILLER PIC X(16) VALUE "Schwarzenboxer".
           05 FILLER PIC X(16) VALUE "Smithe".
           05 FILLER PIC X(16) VALUE "Johnsson".
           05 FILLER PIC X(16) VALUE "Johannsen".
           05 FILLER PIC X(16) VALUE "Streepe".
           05 FILLER PIC X(16) VALUE "Mourao".
           05 FILLER PIC X(16) VALUE "Santoros".
           05 FILLER PIC X(16) VALUE "Montenegra".
           05 FILLER PIC X(16) VALUE "Melo Souza".
           05 FILLER PIC X(16) VALUE "Ramiros".
           05 FILLER PIC X(16) VALUE "Nachtergaele".
           05 FILLER PIC X(16) VALUE "Raimundos".
           05 FILLER PIC X(16) VALUE "Marquez".
           05 FILLER PIC X(16) VALUE "Bragga".
           05 FILLER PIC X(16) VALUE "Pirez".
           05 FILLER PIC X(16) VALUE "Autrann".
           05 FILLER PIC X(16) VALUE "Meirelles".
           05 FILLER PIC X(16) VALUE "Fagundez".
           05 FILLER PIC X(16) VALUE "Duartte".
           05 FILLER PIC X(16) VALUE "Falcao".
           05 FILLER PIC X(16) VALUE "Bennicio".
           05 FILLER PIC X(16) VALUE "de Almeidas".
           05 FILLER PIC X(16) VALUE "Infantado".
           05 FILLER PIC X(16) VALUE "Lopes-Lopes".
           05 FILLER PIC X(16) VALUE "Pereirinha".
           05 FILLER PIC X(16) VALUE "de Medeiros".
           05 FILLER PIC X(16) VALUE "Bragancca".
           05 FILLER PIC X(16) VALUE "Unas".
           05 FILLER PIC X(16) VALUE "Mourinhos".
           05 FILLER PIC X(16) VALUE "Salazarra".
           05 FILLER PIC X(16) VALUE "Cruzeiro".
           05 FILLER PIC X(16) VALUE "Seabrra".
           05 FILLER PIC X(16) VALUE "Guedez".
           05 FILLER PIC X(16) VALUE "Vasconcellos".
           05 FILLER PIC X(16) VALUE "Nogueiro".
           05 FILLER PIC X(16) VALUE "Mendonssa".
           05 FILLER PIC X(16) VALUE "Carvalhal".
           05 FILLER PIC X(16) VALUE "Garcia Bernardo".
           05 FILLER PIC X(16) VALUE "Lunar".
           05 FILLER PIC X(16) VALUE "Hayeck".
           05 FILLER PIC X(16) VALUE "Aparissio".
           05 FILLER PIC X(16) VALUE "Bichirr".
           05 FILLER PIC X(16) VALUE "Derbezz".
           05 FILLER PIC X(16) VALUE "Castillos".
           05 FILLER PIC X(16) VALUE "Uribbe".
           05 FILLER PIC X(16) VALUE "Armaz".
           05 FILLER PIC X(16) VALUE "Cuaronn".
           05 FILLER PIC X(16) VALUE "de Tavirra".
           05 FILLER PIC X(16) VALUE "Huertta".
           05 FILLER PIC X(16) VALUE "Souzza".
           05 FILLER PIC X(16) VALUE "Echevarry".
           05 FILLER PIC X(16) VALUE "Salass".
           05 FILLER PIC X(16) VALUE "Cosio-Cosio".
           05 FILLER PIC X(16) VALUE "Renault".
           05 FILLER PIC X(16) VALUE "Cotillardo".
           05 FILLER PIC X(16) VALUE "Casselle".
           05 FILLER PIC X(16) VALUE "Toutou".
           05 FILLER PIC X(16) VALUE "Sy-Sy".
           05 FILLER PIC X(16) VALUE "Marceaux".
           05 FILLER PIC X(16) VALUE "Depardieuxx".
           05 FILLER PIC X(16) VALUE "Binochet".
           05 FILLER PIC X(16) VALUE "Amalricco".
           05 FILLER PIC X(16) VALUE "Huppertine".
           05 FILLER PIC X(16) VALUE "Garrelo".
           05 FILLER PIC X(16) VALUE "Exarchopoulle".
           05 FILLER PIC X(16) VALUE "Canet-Canet".
           05 FILLER PIC X(16) VALUE "Seydouxx".
           05 FILLER PIC X(16) VALUE "Rahimm".
           05 FILLER PIC X(16) VALUE "Deneuvre".
           05 FILLER PIC X(16) VALUE "Chann".
           05 FILLER PIC X(16) VALUE "Leee".
           05 FILLER PIC X(16) VALUE "Leungg".
           05 FILLER PIC X(16) VALUE "Zhangg-Zhang".
           05 FILLER PIC X(16) VALUE "Lii".
           05 FILLER PIC X(16) VALUE "Lauu".
           05 FILLER PIC X(16) VALUE "Yenn".
           05 FILLER PIC X(16) VALUE "Bingbingo".
           05 FILLER PIC X(16) VALUE "Yun-Fatt".
           05 FILLER PIC X(16) VALUE "Ziyi-Yi".
           05 FILLER PIC X(16) VALUE "Cheungg".
           05 FILLER PIC X(16) VALUE "Bo-Bo".
           05 FILLER PIC X(16) VALUE "Zhouu".
           05 FILLER PIC X(16) VALUE "Jiang".
           05 FILLER PIC X(16) VALUE "Kaneshiroo".
           05 FILLER PIC X(16) VALUE "Yifeii".
           05 FILLER PIC X(16) VALUE "Mifunny".
           05 FILLER PIC X(16) VALUE "Watanabee".
           05 FILLER PIC X(16) VALUE "Kore-Edda".
           05 FILLER PIC X(16) VALUE "Asanoo".
           05 FILLER PIC X(16) VALUE "Kikuchee".
           05 FILLER PIC X(16) VALUE "Fukuyamma".
           05 FILLER PIC X(16) VALUE "Yakushoo".
           05 FILLER PIC X(16) VALUE "Sanada".
           05 FILLER PIC X(16) VALUE "Tsumabukki".
           05 FILLER PIC X(16) VALUE "Miyazakii".
           05 FILLER PIC X(16) VALUE "Kitanno".
           05 FILLER PIC X(16) VALUE "Sanadda".
           05 FILLER PIC X(16) VALUE "Onoo".
           05 FILLER PIC X(16) VALUE "Ogatta".
           05 FILLER PIC X(16) VALUE "Andoo".
           05 FILLER PIC X(16) VALUE "Kase-Kase".
       01  WS-LAST-R REDEFINES WS-LAST-TAB.
           05 WS-LAST PIC X(16) OCCURS 112.
       01  WS-N              PIC 9(5) VALUE ZERO.
       01  WS-IDX            PIC 9(5) VALUE ZERO.
       01  WS-C              PIC 9(2) VALUE ZERO.
       01  WS-FI             PIC 9(3) VALUE ZERO.
       01  WS-LI             PIC 9(3) VALUE ZERO.
       01  WS-REM            PIC 9(5) VALUE ZERO.
       01  WS-PHOTO          PIC 9(3) VALUE ZERO.
       01  WS-PHOTO-ED       PIC ZZ9.
       01  WS-NAME           PIC X(30) VALUE SPACES.
       01  WS-THUMB          PIC X(60) VALUE SPACES.
       01  WS-GEN            PIC 9(5) VALUE ZERO.
       01  WS-ED5            PIC ZZZZ9.

       PROCEDURE DIVISION.

       MAIN-PARA.
           OPEN OUTPUT ACTORS-FILE
           IF WS-STATUS NOT = "00"
               DISPLAY "OPEN OUTPUT failed, FILE STATUS = " WS-STATUS
               STOP RUN
           END-IF

           PERFORM LOAD-ACTORS
           PERFORM LOAD-GENERATED-CAST

           CLOSE ACTORS-FILE
           IF WS-STATUS NOT = "00"
               DISPLAY "CLOSE failed, FILE STATUS = " WS-STATUS
           END-IF

           PERFORM REPORT-RESULTS
           STOP RUN.

       WRITE-ACTOR.
           WRITE ACTORS-RECORD
               INVALID KEY
                   ADD 1 TO WS-FAILED
                   DISPLAY "WRITE rejected for ACTOR-ID " ACTOR-ID
                       " FILE STATUS = " WS-STATUS
               NOT INVALID KEY
                   ADD 1 TO WS-WRITTEN
           END-WRITE.

       REPORT-RESULTS.
           DISPLAY " "
           DISPLAY "==============================================="
           DISPLAY " LOAD-ACTORS-IDX -- result summary"
           DISPLAY "==============================================="
           DISPLAY " File          : data/idxfiles/actors.idx"
           DISPLAY " Organization  : INDEXED, STORAGE MODE IS DISK"
           DISPLAY " Record length : 111 characters (scalar record)"
           DISPLAY " Primary key   : ACTOR-ID  PIC 9(9), no duplicates"
           DISPLAY " Fields        : ACTOR-ID     PIC 9(9)"
           DISPLAY "                 ACTOR-THUMB  PIC X(60)"
           DISPLAY "                 ACTOR-CAPTION PIC X(30)"
           DISPLAY "                 ACTOR-SALARY PIC 9(9)V99"
           DISPLAY "                 ACTOR-AWARDS PIC X"
           DISPLAY "-----------------------------------------------"
           MOVE WS-WRITTEN TO WS-ED
           DISPLAY " Records written : " WS-ED
           MOVE WS-FAILED TO WS-ED
           DISPLAY " Records failed  : " WS-ED
           DISPLAY " Expected        : 1098  (100 seeded + 998 generated)"
           PERFORM REPORT-GENERATED
           DISPLAY "==============================================="
           DISPLAY " ".


       LOAD-GENERATED-CAST.
      *> 998 rows, ACTOR-ID 101..1098. The country cycles every row, so a
      *> grid scrolled a screen at a time shows all seven mixed together
      *> rather than seven long blocks.
           PERFORM VARYING WS-N FROM 1 BY 1 UNTIL WS-N > 998
               DIVIDE WS-N BY 7 GIVING WS-IDX REMAINDER WS-C
      *>     WS-C is 1..7 after the ADD; WS-IDX counts rows within it.
               ADD 1 TO WS-C
               COMPUTE WS-IDX = (WS-N - 1) / 7
               DIVIDE WS-IDX BY 16 GIVING WS-LI REMAINDER WS-FI
               ADD 1 TO WS-FI
               ADD 1 TO WS-LI
      *>     Into the country's own 16-name window.
               COMPUTE WS-FI = (WS-C - 1) * 16 + WS-FI
               COMPUTE WS-LI = (WS-C - 1) * 16 + WS-LI
               MOVE SPACES TO WS-NAME
               STRING FUNCTION TRIM(WS-FIRST(WS-FI))
                      " "
                      FUNCTION TRIM(WS-LAST(WS-LI))
                   DELIMITED BY SIZE INTO WS-NAME
               END-STRING
      *>     Reuse the 100 photos that exist rather than name files that
      *>     do not.
               DIVIDE WS-N BY 100 GIVING WS-REM REMAINDER WS-PHOTO
               ADD 1 TO WS-PHOTO
               MOVE WS-PHOTO TO WS-PHOTO-ED
               MOVE SPACES TO WS-THUMB
               STRING "~/PowerDemo2/assets/images/photo"
                      FUNCTION TRIM(WS-PHOTO-ED)
                      ".jpg"
                   DELIMITED BY SIZE INTO WS-THUMB
               END-STRING
               COMPUTE ACTOR-ID = 100 + WS-N
               MOVE WS-THUMB TO ACTOR-THUMB
               MOVE WS-NAME TO ACTOR-CAPTION
               COMPUTE ACTOR-SALARY = 500 + WS-IDX * 31
               DIVIDE WS-N BY 4 GIVING WS-REM REMAINDER WS-C
               MOVE WS-C TO ACTOR-AWARDS
      *>     WS-C was reused for the award digit, so recompute the
      *>     country before counting it.
               DIVIDE WS-N BY 7 GIVING WS-REM REMAINDER WS-C
               ADD 1 TO WS-C
               ADD 1 TO WS-PER-COUNTRY(WS-C)
               ADD 1 TO WS-GEN
               PERFORM WRITE-ACTOR
           END-PERFORM.

       REPORT-GENERATED.
           DISPLAY " "
           DISPLAY "-----------------------------------------------"
           DISPLAY " Generated cast (ACTOR-ID 101..1098)"
           DISPLAY "-----------------------------------------------"
           MOVE WS-GEN TO WS-ED5
           DISPLAY " Rows generated  : " WS-ED5
           PERFORM VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 7
               MOVE WS-PER-COUNTRY(WS-C) TO WS-ED
               DISPLAY "   " WS-COUNTRY(WS-C) " : " WS-ED
           END-PERFORM
           DISPLAY "-----------------------------------------------".

       LOAD-ACTORS.

      *> ---- actor   1 ----
           MOVE 000000001 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo1.jpg" TO ACTOR-THUMB.
           MOVE "Leonardo DiCaprio" TO ACTOR-CAPTION.
           MOVE 3000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   2 ----
           MOVE 000000002 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo2.jpg" TO ACTOR-THUMB.
           MOVE "Joe Pesci" TO ACTOR-CAPTION.
           MOVE 1200.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   3 ----
           MOVE 000000003 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo3.jpg" TO ACTOR-THUMB.
           MOVE "Robert De Niro" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   4 ----
           MOVE 000000004 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo4.jpg" TO ACTOR-THUMB.
           MOVE "Al Pacino" TO ACTOR-CAPTION.
           MOVE 1800.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   5 ----
           MOVE 000000005 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo5.jpg" TO ACTOR-THUMB.
           MOVE "Marlon Brando" TO ACTOR-CAPTION.
           MOVE 1500.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   6 ----
           MOVE 000000006 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo6.jpg" TO ACTOR-THUMB.
           MOVE "Jack Nicholson" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   7 ----
           MOVE 000000007 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo7.jpg" TO ACTOR-THUMB.
           MOVE "Daniel Day-Lewis" TO ACTOR-CAPTION.
           MOVE 1200.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   8 ----
           MOVE 000000008 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo8.jpg" TO ACTOR-THUMB.
           MOVE "Denzel Washington" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor   9 ----
           MOVE 000000009 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo9.jpg" TO ACTOR-THUMB.
           MOVE "Tom Hanks" TO ACTOR-CAPTION.
           MOVE 2500.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  10 ----
           MOVE 000000010 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo10.jpg" TO ACTOR-THUMB.
           MOVE "Morgan Freeman" TO ACTOR-CAPTION.
           MOVE 1200.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  11 ----
           MOVE 000000011 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo11.jpg" TO ACTOR-THUMB.
           MOVE "Anthony Hopkins" TO ACTOR-CAPTION.
           MOVE 1500.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  12 ----
           MOVE 000000012 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo12.jpg" TO ACTOR-THUMB.
           MOVE "Christian Bale" TO ACTOR-CAPTION.
           MOVE 1500.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  13 ----
           MOVE 000000013 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo13.jpg" TO ACTOR-THUMB.
           MOVE "Gary Oldman" TO ACTOR-CAPTION.
           MOVE 1000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  14 ----
           MOVE 000000014 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo14.jpg" TO ACTOR-THUMB.
           MOVE "Samuel L. Jackson" TO ACTOR-CAPTION.
           MOVE 1500.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  15 ----
           MOVE 000000015 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo15.jpg" TO ACTOR-THUMB.
           MOVE "Brad Pitt" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  16 ----
           MOVE 000000016 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo16.jpg" TO ACTOR-THUMB.
           MOVE "George Clooney" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  17 ----
           MOVE 000000017 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo17.jpg" TO ACTOR-THUMB.
           MOVE "Johnny Depp" TO ACTOR-CAPTION.
           MOVE 2000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  18 ----
           MOVE 000000018 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo18.jpg" TO ACTOR-THUMB.
           MOVE "Keanu Reeves" TO ACTOR-CAPTION.
           MOVE 1800.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  19 ----
           MOVE 000000019 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo19.jpg" TO ACTOR-THUMB.
           MOVE "Tom Cruise" TO ACTOR-CAPTION.
           MOVE 300000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  20 ----
           MOVE 000000020 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo20.jpg" TO ACTOR-THUMB.
           MOVE "Matt Damon" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  21 ----
           MOVE 000000021 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo21.jpg" TO ACTOR-THUMB.
           MOVE "Ben Affleck" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  22 ----
           MOVE 000000022 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo22.jpg" TO ACTOR-THUMB.
           MOVE "Joaquin Phoenix" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  23 ----
           MOVE 000000023 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo23.jpg" TO ACTOR-THUMB.
           MOVE "Russell Crowe" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  24 ----
           MOVE 000000024 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo24.jpg" TO ACTOR-THUMB.
           MOVE "Harrison Ford" TO ACTOR-CAPTION.
           MOVE 180000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  25 ----
           MOVE 000000025 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo25.jpg" TO ACTOR-THUMB.
           MOVE "Clint Eastwood" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  26 ----
           MOVE 000000026 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo26.jpg" TO ACTOR-THUMB.
           MOVE "Sean Connery" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  27 ----
           MOVE 000000027 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo27.jpg" TO ACTOR-THUMB.
           MOVE "Michael Caine" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  28 ----
           MOVE 000000028 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo28.jpg" TO ACTOR-THUMB.
           MOVE "Liam Neeson" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  29 ----
           MOVE 000000029 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo29.jpg" TO ACTOR-THUMB.
           MOVE "Bruce Willis" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  30 ----
           MOVE 000000030 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo30.jpg" TO ACTOR-THUMB.
           MOVE "Arnold Schwarzenegger" TO ACTOR-CAPTION.
           MOVE 200000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  31 ----
           MOVE 000000031 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo31.jpg" TO ACTOR-THUMB.
           MOVE "Sylvester Stallone" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  32 ----
           MOVE 000000032 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo32.jpg" TO ACTOR-THUMB.
           MOVE "Will Smith" TO ACTOR-CAPTION.
           MOVE 250000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  33 ----
           MOVE 000000033 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo33.jpg" TO ACTOR-THUMB.
           MOVE "Jamie Foxx" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  34 ----
           MOVE 000000034 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo34.jpg" TO ACTOR-THUMB.
           MOVE "Forest Whitaker" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  35 ----
           MOVE 000000035 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo35.jpg" TO ACTOR-THUMB.
           MOVE "Laurence Fishburne" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  36 ----
           MOVE 000000036 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo36.jpg" TO ACTOR-THUMB.
           MOVE "Idris Elba" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  37 ----
           MOVE 000000037 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo37.jpg" TO ACTOR-THUMB.
           MOVE "Mahershala Ali" TO ACTOR-CAPTION.
           MOVE 60000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  38 ----
           MOVE 000000038 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo38.jpg" TO ACTOR-THUMB.
           MOVE "Chadwick Boseman" TO ACTOR-CAPTION.
           MOVE 50000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  39 ----
           MOVE 000000039 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo39.jpg" TO ACTOR-THUMB.
           MOVE "Michael B. Jordan" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  40 ----
           MOVE 000000040 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo40.jpg" TO ACTOR-THUMB.
           MOVE "Dwayne Johnson" TO ACTOR-CAPTION.
           MOVE 250000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  41 ----
           MOVE 000000041 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo41.jpg" TO ACTOR-THUMB.
           MOVE "Ryan Reynolds" TO ACTOR-CAPTION.
           MOVE 200000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  42 ----
           MOVE 000000042 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo42.jpg" TO ACTOR-THUMB.
           MOVE "Hugh Jackman" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  43 ----
           MOVE 000000043 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo43.jpg" TO ACTOR-THUMB.
           MOVE "Patrick Stewart" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  44 ----
           MOVE 000000044 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo44.jpg" TO ACTOR-THUMB.
           MOVE "Ian McKellen" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  45 ----
           MOVE 000000045 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo45.jpg" TO ACTOR-THUMB.
           MOVE "Benedict Cumberbatch" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  46 ----
           MOVE 000000046 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo46.jpg" TO ACTOR-THUMB.
           MOVE "Tom Hardy" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  47 ----
           MOVE 000000047 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo47.jpg" TO ACTOR-THUMB.
           MOVE "Cillian Murphy" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  48 ----
           MOVE 000000048 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo48.jpg" TO ACTOR-THUMB.
           MOVE "Ralph Fiennes" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  49 ----
           MOVE 000000049 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo49.jpg" TO ACTOR-THUMB.
           MOVE "Edward Norton" TO ACTOR-CAPTION.
           MOVE 90000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  50 ----
           MOVE 000000050 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo50.jpg" TO ACTOR-THUMB.
           MOVE "Mark Ruffalo" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  51 ----
           MOVE 000000051 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo51.jpg" TO ACTOR-THUMB.
           MOVE "Robert Downey Jr." TO ACTOR-CAPTION.
           MOVE 400000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  52 ----
           MOVE 000000052 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo52.jpg" TO ACTOR-THUMB.
           MOVE "Chris Evans" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  53 ----
           MOVE 000000053 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo53.jpg" TO ACTOR-THUMB.
           MOVE "Chris Hemsworth" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  54 ----
           MOVE 000000054 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo54.jpg" TO ACTOR-THUMB.
           MOVE "Jeremy Renner" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  55 ----
           MOVE 000000055 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo55.jpg" TO ACTOR-THUMB.
           MOVE "Paul Rudd" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  56 ----
           MOVE 000000056 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo56.jpg" TO ACTOR-THUMB.
           MOVE "Don Cheadle" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  57 ----
           MOVE 000000057 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo57.jpg" TO ACTOR-THUMB.
           MOVE "Chris Pratt" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  58 ----
           MOVE 000000058 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo58.jpg" TO ACTOR-THUMB.
           MOVE "Vin Diesel" TO ACTOR-CAPTION.
           MOVE 180000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  59 ----
           MOVE 000000059 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo59.jpg" TO ACTOR-THUMB.
           MOVE "Jason Statham" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  60 ----
           MOVE 000000060 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo60.jpg" TO ACTOR-THUMB.
           MOVE "Jackie Chan" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  61 ----
           MOVE 000000061 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo61.jpg" TO ACTOR-THUMB.
           MOVE "Jet Li" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  62 ----
           MOVE 000000062 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo62.jpg" TO ACTOR-THUMB.
           MOVE "Tony Leung" TO ACTOR-CAPTION.
           MOVE 60000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  63 ----
           MOVE 000000063 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo63.jpg" TO ACTOR-THUMB.
           MOVE "Ken Watanabe" TO ACTOR-CAPTION.
           MOVE 50000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  64 ----
           MOVE 000000064 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo64.jpg" TO ACTOR-THUMB.
           MOVE "Song Kang-ho" TO ACTOR-CAPTION.
           MOVE 40000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  65 ----
           MOVE 000000065 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo65.jpg" TO ACTOR-THUMB.
           MOVE "Toshiro Mifune" TO ACTOR-CAPTION.
           MOVE 30000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  66 ----
           MOVE 000000066 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo66.jpg" TO ACTOR-THUMB.
           MOVE "Mads Mikkelsen" TO ACTOR-CAPTION.
           MOVE 60000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  67 ----
           MOVE 000000067 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo67.jpg" TO ACTOR-THUMB.
           MOVE "Christoph Waltz" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  68 ----
           MOVE 000000068 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo68.jpg" TO ACTOR-THUMB.
           MOVE "Javier Bardem" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  69 ----
           MOVE 000000069 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo69.jpg" TO ACTOR-THUMB.
           MOVE "Antonio Banderas" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  70 ----
           MOVE 000000070 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo70.jpg" TO ACTOR-THUMB.
           MOVE "Pedro Pascal" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  71 ----
           MOVE 000000071 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo71.jpg" TO ACTOR-THUMB.
           MOVE "Oscar Isaac" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  72 ----
           MOVE 000000072 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo72.jpg" TO ACTOR-THUMB.
           MOVE "Gael Garcia Bernal" TO ACTOR-CAPTION.
           MOVE 40000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  73 ----
           MOVE 000000073 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo73.jpg" TO ACTOR-THUMB.
           MOVE "Diego Luna" TO ACTOR-CAPTION.
           MOVE 40000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  74 ----
           MOVE 000000074 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo74.jpg" TO ACTOR-THUMB.
           MOVE "Wagner Moura" TO ACTOR-CAPTION.
           MOVE 30000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  75 ----
           MOVE 000000075 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo75.jpg" TO ACTOR-THUMB.
           MOVE "Rodrigo Santoro" TO ACTOR-CAPTION.
           MOVE 30000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  76 ----
           MOVE 000000076 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo76.jpg" TO ACTOR-THUMB.
           MOVE "Tobey Maguire" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  77 ----
           MOVE 000000077 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo77.jpg" TO ACTOR-THUMB.
           MOVE "Andrew Garfield" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  78 ----
           MOVE 000000078 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo78.jpg" TO ACTOR-THUMB.
           MOVE "Tom Holland" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  79 ----
           MOVE 000000079 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo79.jpg" TO ACTOR-THUMB.
           MOVE "Heath Ledger" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  80 ----
           MOVE 000000080 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo80.jpg" TO ACTOR-THUMB.
           MOVE "Jake Gyllenhaal" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  81 ----
           MOVE 000000081 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo81.jpg" TO ACTOR-THUMB.
           MOVE "Ryan Gosling" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  82 ----
           MOVE 000000082 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo82.jpg" TO ACTOR-THUMB.
           MOVE "Matthew McConaughey" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  83 ----
           MOVE 000000083 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo83.jpg" TO ACTOR-THUMB.
           MOVE "Woody Harrelson" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  84 ----
           MOVE 000000084 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo84.jpg" TO ACTOR-THUMB.
           MOVE "Kevin Costner" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  85 ----
           MOVE 000000085 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo85.jpg" TO ACTOR-THUMB.
           MOVE "John Travolta" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  86 ----
           MOVE 000000086 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo86.jpg" TO ACTOR-THUMB.
           MOVE "Nicolas Cage" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  87 ----
           MOVE 000000087 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo87.jpg" TO ACTOR-THUMB.
           MOVE "John Malkovich" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  88 ----
           MOVE 000000088 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo88.jpg" TO ACTOR-THUMB.
           MOVE "Steve Buscemi" TO ACTOR-CAPTION.
           MOVE 40000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  89 ----
           MOVE 000000089 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo89.jpg" TO ACTOR-THUMB.
           MOVE "Philip Seymour Hoffman" TO ACTOR-CAPTION.
           MOVE 60000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  90 ----
           MOVE 000000090 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo90.jpg" TO ACTOR-THUMB.
           MOVE "William H. Macy" TO ACTOR-CAPTION.
           MOVE 40000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  91 ----
           MOVE 000000091 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo91.jpg" TO ACTOR-THUMB.
           MOVE "Jeff Bridges" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  92 ----
           MOVE 000000092 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo92.jpg" TO ACTOR-THUMB.
           MOVE "Kurt Russell" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  93 ----
           MOVE 000000093 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo93.jpg" TO ACTOR-THUMB.
           MOVE "Bill Murray" TO ACTOR-CAPTION.
           MOVE 80000.00 TO ACTOR-SALARY.
           MOVE "4" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  94 ----
           MOVE 000000094 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo94.jpg" TO ACTOR-THUMB.
           MOVE "Dan Aykroyd" TO ACTOR-CAPTION.
           MOVE 50000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  95 ----
           MOVE 000000095 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo95.jpg" TO ACTOR-THUMB.
           MOVE "Eddie Murphy" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  96 ----
           MOVE 000000096 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo96.jpg" TO ACTOR-THUMB.
           MOVE "Jim Carrey" TO ACTOR-CAPTION.
           MOVE 150000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  97 ----
           MOVE 000000097 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo97.jpg" TO ACTOR-THUMB.
           MOVE "Robin Williams" TO ACTOR-CAPTION.
           MOVE 120000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  98 ----
           MOVE 000000098 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo98.jpg" TO ACTOR-THUMB.
           MOVE "Gene Hackman" TO ACTOR-CAPTION.
           MOVE 100000.00 TO ACTOR-SALARY.
           MOVE "1" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor  99 ----
           MOVE 000000099 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo99.jpg" TO ACTOR-THUMB.
           MOVE "Christopher Walken" TO ACTOR-CAPTION.
           MOVE 70000.00 TO ACTOR-SALARY.
           MOVE "2" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.

      *> ---- actor 100 ----
           MOVE 000000100 TO ACTOR-ID.
           MOVE "~/PowerDemo2/assets/images/photo100.jpg" TO ACTOR-THUMB.
           MOVE "Christopher Lee" TO ACTOR-CAPTION.
           MOVE 50000.00 TO ACTOR-SALARY.
           MOVE "3" TO ACTOR-AWARDS.
           PERFORM WRITE-ACTOR.


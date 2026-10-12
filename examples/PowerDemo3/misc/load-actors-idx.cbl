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

       *> RECORD CONTAINS must equal the field sizes: 9 + 60 + 30 + 11 + 1 = 111
       *> (the V in ACTOR-SALARY is implied and takes no byte).

       FD ACTORS-FILE
           RECORD CONTAINS 111 CHARACTERS.
       01 ACTORS-RECORD.
           05 ACTOR-ID                           PIC    9(9).
           05 ACTOR-THUMB                        PIC    X(60).
           05 ACTOR-CAPTION                      PIC    X(30).
           05 ACTOR-SALARY                       PIC    9(9)V99.
           05 ACTOR-AWARDS                       PIC    X.

       WORKING-STORAGE SECTION.
       01 WS-STATUS                              PIC    XX.
       01 WS-WRITTEN                             PIC    9(4)    VALUE ZERO.
       01 WS-FAILED                              PIC    9(4)    VALUE ZERO.
       01 WS-ED                                  PIC    ZZZ9.
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

       01 WS-COUNTRY-TAB.
           05 FILLER                             PIC    X(9)    VALUE "US".
           05 FILLER                             PIC    X(9)    VALUE "Brazil".
           05 FILLER                             PIC    X(9)    VALUE "Portugal".
           05 FILLER                             PIC    X(9)    VALUE "Mexico".
           05 FILLER                             PIC    X(9)    VALUE "France".
           05 FILLER                             PIC    X(9)    VALUE "China".
           05 FILLER                             PIC    X(9)    VALUE "Japan".
       01 WS-COUNTRY-R                           REDEFINES    WS-COUNTRY-TAB.
           05 WS-COUNTRY                         PIC    X(9)    OCCURS 7.
       01 WS-PER-COUNTRY-TAB.
           05 WS-PER-COUNTRY                     PIC    9(4)    OCCURS 7    VALUE ZERO.
       01 WS-FIRST-TAB.
           05 FILLER                             PIC    X(16)    VALUE "John-John".
           05 FILLER                             PIC    X(16)    VALUE "Leobardo".
           05 FILLER                             PIC    X(16)    VALUE "Keannes".
           05 FILLER                             PIC    X(16)    VALUE "Bratt".
           05 FILLER                             PIC    X(16)    VALUE "Tomm".
           05 FILLER                             PIC    X(16)    VALUE "Robbert".
           05 FILLER                             PIC    X(16)    VALUE "Denzil".
           05 FILLER                             PIC    X(16)    VALUE "Morgun".
           05 FILLER                             PIC    X(16)    VALUE "Jaqk".
           05 FILLER                             PIC    X(16)    VALUE "Harryson".
           05 FILLER                             PIC    X(16)    VALUE "Silvestro".
           05 FILLER                             PIC    X(16)    VALUE "Arnoldo".
           05 FILLER                             PIC    X(16)    VALUE "Wille".
           05 FILLER                             PIC    X(16)    VALUE "Dwaine".
           05 FILLER                             PIC    X(16)    VALUE "Scarlette".
           05 FILLER                             PIC    X(16)    VALUE "Meryll".
           05 FILLER                             PIC    X(16)    VALUE "Wagnner".
           05 FILLER                             PIC    X(16)    VALUE "Rodrigno".
           05 FILLER                             PIC    X(16)    VALUE "Fernanda".
           05 FILLER                             PIC    X(16)    VALUE "Seltton".
           05 FILLER                             PIC    X(16)    VALUE "Lazaro".
           05 FILLER                             PIC    X(16)    VALUE "Matheuz".
           05 FILLER                             PIC    X(16)    VALUE "Cauan".
           05 FILLER                             PIC    X(16)    VALUE "Bruna".
           05 FILLER                             PIC    X(16)    VALUE "Alicce".
           05 FILLER                             PIC    X(16)    VALUE "Gloria".
           05 FILLER                             PIC    X(16)    VALUE "Paolo".
           05 FILLER                             PIC    X(16)    VALUE "Tarcisio".
           05 FILLER                             PIC    X(16)    VALUE "Antonho".
           05 FILLER                             PIC    X(16)    VALUE "Regyna".
           05 FILLER                             PIC    X(16)    VALUE "Deborah".
           05 FILLER                             PIC    X(16)    VALUE "Murillo".
           05 FILLER                             PIC    X(16)    VALUE "Joaquim".
           05 FILLER                             PIC    X(16)    VALUE "Diogo".
           05 FILLER                             PIC    X(16)    VALUE "Nuno".
           05 FILLER                             PIC    X(16)    VALUE "Ricardo".
           05 FILLER                             PIC    X(16)    VALUE "Maria".
           05 FILLER                             PIC    X(16)    VALUE "Beatrize".
           05 FILLER                             PIC    X(16)    VALUE "Rui".
           05 FILLER                             PIC    X(16)    VALUE "Vasqo".
           05 FILLER                             PIC    X(16)    VALUE "Ineez".
           05 FILLER                             PIC    X(16)    VALUE "Tiaguo".
           05 FILLER                             PIC    X(16)    VALUE "Leonor".
           05 FILLER                             PIC    X(16)    VALUE "Miguelo".
           05 FILLER                             PIC    X(16)    VALUE "Catarinha".
           05 FILLER                             PIC    X(16)    VALUE "Bruno".
           05 FILLER                             PIC    X(16)    VALUE "Soffia".
           05 FILLER                             PIC    X(16)    VALUE "Filipe".
           05 FILLER                             PIC    X(16)    VALUE "Gaelo".
           05 FILLER                             PIC    X(16)    VALUE "Diegho".
           05 FILLER                             PIC    X(16)    VALUE "Salmita".
           05 FILLER                             PIC    X(16)    VALUE "Yalitzza".
           05 FILLER                             PIC    X(16)    VALUE "Demiann".
           05 FILLER                             PIC    X(16)    VALUE "Eugenio".
           05 FILLER                             PIC    X(16)    VALUE "Kate del".
           05 FILLER                             PIC    X(16)    VALUE "Adriann".
           05 FILLER                             PIC    X(16)    VALUE "Ana de".
           05 FILLER                             PIC    X(16)    VALUE "Alfonzo".
           05 FILLER                             PIC    X(16)    VALUE "Marinna".
           05 FILLER                             PIC    X(16)    VALUE "Tenoch".
           05 FILLER                             PIC    X(16)    VALUE "Karla".
           05 FILLER                             PIC    X(16)    VALUE "Emilio".
           05 FILLER                             PIC    X(16)    VALUE "Ilse".
           05 FILLER                             PIC    X(16)    VALUE "Joaquino".
           05 FILLER                             PIC    X(16)    VALUE "Jeann".
           05 FILLER                             PIC    X(16)    VALUE "Marionn".
           05 FILLER                             PIC    X(16)    VALUE "Vincennt".
           05 FILLER                             PIC    X(16)    VALUE "Audrei".
           05 FILLER                             PIC    X(16)    VALUE "Omarr".
           05 FILLER                             PIC    X(16)    VALUE "Sophi".
           05 FILLER                             PIC    X(16)    VALUE "Gerardo".
           05 FILLER                             PIC    X(16)    VALUE "Juliette".
           05 FILLER                             PIC    X(16)    VALUE "Matthiew".
           05 FILLER                             PIC    X(16)    VALUE "Isabellle".
           05 FILLER                             PIC    X(16)    VALUE "Louie".
           05 FILLER                             PIC    X(16)    VALUE "Adele".
           05 FILLER                             PIC    X(16)    VALUE "Guillaume".
           05 FILLER                             PIC    X(16)    VALUE "Leaa".
           05 FILLER                             PIC    X(16)    VALUE "Tahar".
           05 FILLER                             PIC    X(16)    VALUE "Cattherine".
           05 FILLER                             PIC    X(16)    VALUE "Jaquie".
           05 FILLER                             PIC    X(16)    VALUE "Jett".
           05 FILLER                             PIC    X(16)    VALUE "Tonny".
           05 FILLER                             PIC    X(16)    VALUE "Ziyii".
           05 FILLER                             PIC    X(16)    VALUE "Gongg".
           05 FILLER                             PIC    X(16)    VALUE "Andi".
           05 FILLER                             PIC    X(16)    VALUE "Donnie".
           05 FILLER                             PIC    X(16)    VALUE "Fann".
           05 FILLER                             PIC    X(16)    VALUE "Chowe".
           05 FILLER                             PIC    X(16)    VALUE "Zhangg".
           05 FILLER                             PIC    X(16)    VALUE "Maggy".
           05 FILLER                             PIC    X(16)    VALUE "Huang".
           05 FILLER                             PIC    X(16)    VALUE "Xun".
           05 FILLER                             PIC    X(16)    VALUE "Wenn".
           05 FILLER                             PIC    X(16)    VALUE "Takeshi".
           05 FILLER                             PIC    X(16)    VALUE "Liuu".
           05 FILLER                             PIC    X(16)    VALUE "Toshiroo".
           05 FILLER                             PIC    X(16)    VALUE "Kenn".
           05 FILLER                             PIC    X(16)    VALUE "Hirokazu".
           05 FILLER                             PIC    X(16)    VALUE "Tadanobu".
           05 FILLER                             PIC    X(16)    VALUE "Rinko".
           05 FILLER                             PIC    X(16)    VALUE "Masaharu".
           05 FILLER                             PIC    X(16)    VALUE "Koji".
           05 FILLER                             PIC    X(16)    VALUE "Yakusho".
           05 FILLER                             PIC    X(16)    VALUE "Satoshi".
           05 FILLER                             PIC    X(16)    VALUE "Aoi".
           05 FILLER                             PIC    X(16)    VALUE "Takeshi".
           05 FILLER                             PIC    X(16)    VALUE "Hiroyuki".
           05 FILLER                             PIC    X(16)    VALUE "Machiko".
           05 FILLER                             PIC    X(16)    VALUE "Issey".
           05 FILLER                             PIC    X(16)    VALUE "Sakura".
           05 FILLER                             PIC    X(16)    VALUE "Ryo".
       01 WS-FIRST-R                             REDEFINES    WS-FIRST-TAB.
           05 WS-FIRST                           PIC    X(16)    OCCURS 112.
       01 WS-LAST-TAB.
           05 FILLER                             PIC    X(16)    VALUE "Waves".
           05 FILLER                             PIC    X(16)    VALUE "di Castrol".
           05 FILLER                             PIC    X(16)    VALUE "Rivers".
           05 FILLER                             PIC    X(16)    VALUE "Pittt".
           05 FILLER                             PIC    X(16)    VALUE "Hankz".
           05 FILLER                             PIC    X(16)    VALUE "de Nero".
           05 FILLER                             PIC    X(16)    VALUE "Washingtown".
           05 FILLER                             PIC    X(16)    VALUE "Freemann".
           05 FILLER                             PIC    X(16)    VALUE "Nickelson".
           05 FILLER                             PIC    X(16)    VALUE "Fjord".
           05 FILLER                             PIC    X(16)    VALUE "Stallonne".
           05 FILLER                             PIC    X(16)    VALUE "Schwarzenboxer".
           05 FILLER                             PIC    X(16)    VALUE "Smithe".
           05 FILLER                             PIC    X(16)    VALUE "Johnsson".
           05 FILLER                             PIC    X(16)    VALUE "Johannsen".
           05 FILLER                             PIC    X(16)    VALUE "Streepe".
           05 FILLER                             PIC    X(16)    VALUE "Mourao".
           05 FILLER                             PIC    X(16)    VALUE "Santoros".
           05 FILLER                             PIC    X(16)    VALUE "Montenegra".
           05 FILLER                             PIC    X(16)    VALUE "Melo Souza".
           05 FILLER                             PIC    X(16)    VALUE "Ramiros".
           05 FILLER                             PIC    X(16)    VALUE "Nachtergaele".
           05 FILLER                             PIC    X(16)    VALUE "Raimundos".
           05 FILLER                             PIC    X(16)    VALUE "Marquez".
           05 FILLER                             PIC    X(16)    VALUE "Bragga".
           05 FILLER                             PIC    X(16)    VALUE "Pirez".
           05 FILLER                             PIC    X(16)    VALUE "Autrann".
           05 FILLER                             PIC    X(16)    VALUE "Meirelles".
           05 FILLER                             PIC    X(16)    VALUE "Fagundez".
           05 FILLER                             PIC    X(16)    VALUE "Duartte".
           05 FILLER                             PIC    X(16)    VALUE "Falcao".
           05 FILLER                             PIC    X(16)    VALUE "Bennicio".
           05 FILLER                             PIC    X(16)    VALUE "de Almeidas".
           05 FILLER                             PIC    X(16)    VALUE "Infantado".
           05 FILLER                             PIC    X(16)    VALUE "Lopes-Lopes".
           05 FILLER                             PIC    X(16)    VALUE "Pereirinha".
           05 FILLER                             PIC    X(16)    VALUE "de Medeiros".
           05 FILLER                             PIC    X(16)    VALUE "Bragancca".
           05 FILLER                             PIC    X(16)    VALUE "Unas".
           05 FILLER                             PIC    X(16)    VALUE "Mourinhos".
           05 FILLER                             PIC    X(16)    VALUE "Salazarra".
           05 FILLER                             PIC    X(16)    VALUE "Cruzeiro".
           05 FILLER                             PIC    X(16)    VALUE "Seabrra".
           05 FILLER                             PIC    X(16)    VALUE "Guedez".
           05 FILLER                             PIC    X(16)    VALUE "Vasconcellos".
           05 FILLER                             PIC    X(16)    VALUE "Nogueiro".
           05 FILLER                             PIC    X(16)    VALUE "Mendonssa".
           05 FILLER                             PIC    X(16)    VALUE "Carvalhal".
           05 FILLER                             PIC    X(16)    VALUE "Garcia Bernardo".
           05 FILLER                             PIC    X(16)    VALUE "Lunar".
           05 FILLER                             PIC    X(16)    VALUE "Hayeck".
           05 FILLER                             PIC    X(16)    VALUE "Aparissio".
           05 FILLER                             PIC    X(16)    VALUE "Bichirr".
           05 FILLER                             PIC    X(16)    VALUE "Derbezz".
           05 FILLER                             PIC    X(16)    VALUE "Castillos".
           05 FILLER                             PIC    X(16)    VALUE "Uribbe".
           05 FILLER                             PIC    X(16)    VALUE "Armaz".
           05 FILLER                             PIC    X(16)    VALUE "Cuaronn".
           05 FILLER                             PIC    X(16)    VALUE "de Tavirra".
           05 FILLER                             PIC    X(16)    VALUE "Huertta".
           05 FILLER                             PIC    X(16)    VALUE "Souzza".
           05 FILLER                             PIC    X(16)    VALUE "Echevarry".
           05 FILLER                             PIC    X(16)    VALUE "Salass".
           05 FILLER                             PIC    X(16)    VALUE "Cosio-Cosio".
           05 FILLER                             PIC    X(16)    VALUE "Renault".
           05 FILLER                             PIC    X(16)    VALUE "Cotillardo".
           05 FILLER                             PIC    X(16)    VALUE "Casselle".
           05 FILLER                             PIC    X(16)    VALUE "Toutou".
           05 FILLER                             PIC    X(16)    VALUE "Sy-Sy".
           05 FILLER                             PIC    X(16)    VALUE "Marceaux".
           05 FILLER                             PIC    X(16)    VALUE "Depardieuxx".
           05 FILLER                             PIC    X(16)    VALUE "Binochet".
           05 FILLER                             PIC    X(16)    VALUE "Amalricco".
           05 FILLER                             PIC    X(16)    VALUE "Huppertine".
           05 FILLER                             PIC    X(16)    VALUE "Garrelo".
           05 FILLER                             PIC    X(16)    VALUE "Exarchopoulle".
           05 FILLER                             PIC    X(16)    VALUE "Canet-Canet".
           05 FILLER                             PIC    X(16)    VALUE "Seydouxx".
           05 FILLER                             PIC    X(16)    VALUE "Rahimm".
           05 FILLER                             PIC    X(16)    VALUE "Deneuvre".
           05 FILLER                             PIC    X(16)    VALUE "Chann".
           05 FILLER                             PIC    X(16)    VALUE "Leee".
           05 FILLER                             PIC    X(16)    VALUE "Leungg".
           05 FILLER                             PIC    X(16)    VALUE "Zhangg-Zhang".
           05 FILLER                             PIC    X(16)    VALUE "Lii".
           05 FILLER                             PIC    X(16)    VALUE "Lauu".
           05 FILLER                             PIC    X(16)    VALUE "Yenn".
           05 FILLER                             PIC    X(16)    VALUE "Bingbingo".
           05 FILLER                             PIC    X(16)    VALUE "Yun-Fatt".
           05 FILLER                             PIC    X(16)    VALUE "Ziyi-Yi".
           05 FILLER                             PIC    X(16)    VALUE "Cheungg".
           05 FILLER                             PIC    X(16)    VALUE "Bo-Bo".
           05 FILLER                             PIC    X(16)    VALUE "Zhouu".
           05 FILLER                             PIC    X(16)    VALUE "Jiang".
           05 FILLER                             PIC    X(16)    VALUE "Kaneshiroo".
           05 FILLER                             PIC    X(16)    VALUE "Yifeii".
           05 FILLER                             PIC    X(16)    VALUE "Mifunny".
           05 FILLER                             PIC    X(16)    VALUE "Watanabee".
           05 FILLER                             PIC    X(16)    VALUE "Kore-Edda".
           05 FILLER                             PIC    X(16)    VALUE "Asanoo".
           05 FILLER                             PIC    X(16)    VALUE "Kikuchee".
           05 FILLER                             PIC    X(16)    VALUE "Fukuyamma".
           05 FILLER                             PIC    X(16)    VALUE "Yakushoo".
           05 FILLER                             PIC    X(16)    VALUE "Sanada".
           05 FILLER                             PIC    X(16)    VALUE "Tsumabukki".
           05 FILLER                             PIC    X(16)    VALUE "Miyazakii".
           05 FILLER                             PIC    X(16)    VALUE "Kitanno".
           05 FILLER                             PIC    X(16)    VALUE "Sanadda".
           05 FILLER                             PIC    X(16)    VALUE "Onoo".
           05 FILLER                             PIC    X(16)    VALUE "Ogatta".
           05 FILLER                             PIC    X(16)    VALUE "Andoo".
           05 FILLER                             PIC    X(16)    VALUE "Kase-Kase".
       01 WS-LAST-R                              REDEFINES    WS-LAST-TAB.
           05 WS-LAST                            PIC    X(16)    OCCURS 112.
       01 WS-N                                   PIC    9(5)    VALUE ZERO.
       01 WS-IDX                                 PIC    9(5)    VALUE ZERO.
       01 WS-C                                   PIC    9(2)    VALUE ZERO.
       01 WS-FI                                  PIC    9(3)    VALUE ZERO.
       01 WS-LI                                  PIC    9(3)    VALUE ZERO.
       01 WS-REM                                 PIC    9(5)    VALUE ZERO.
       01 WS-PHOTO                               PIC    9(3)    VALUE ZERO.
       01 WS-PHOTO-ED                            PIC    ZZ9.
       01 WS-NAME                                PIC    X(30)    VALUE SPACES.
       01 WS-THUMB                               PIC    X(60)    VALUE SPACES.
       01 WS-GEN                                 PIC    9(5)    VALUE ZERO.
       01 WS-ED5                                 PIC    ZZZZ9.

       PROCEDURE DIVISION.

       MAIN-PARA.
           OPEN    OUTPUT ACTORS-FILE
           IF    WS-STATUS NOT = "00"
               DISPLAY    "OPEN OUTPUT failed, FILE STATUS = " WS-STATUS
               STOP RUN
           END-IF

           PERFORM    LOAD-ACTORS
           PERFORM    LOAD-GENERATED-CAST

           CLOSE    ACTORS-FILE
           IF    WS-STATUS NOT = "00"
               DISPLAY    "CLOSE failed, FILE STATUS = " WS-STATUS
           END-IF

           PERFORM    REPORT-RESULTS
           STOP RUN.

       WRITE-ACTOR.
           WRITE    ACTORS-RECORD
               INVALID KEY
                   ADD    1 TO WS-FAILED
                   DISPLAY    "WRITE rejected for ACTOR-ID " ACTOR-ID
                       " FILE STATUS = " WS-STATUS
               NOT INVALID KEY
                   ADD    1 TO WS-WRITTEN
           END-WRITE.

       REPORT-RESULTS.
           DISPLAY    " "
           DISPLAY    "==============================================="
           DISPLAY    " LOAD-ACTORS-IDX -- result summary"
           DISPLAY    "==============================================="
           DISPLAY    " File          : data/idxfiles/actors.idx"
           DISPLAY    " Organization  : INDEXED, STORAGE MODE IS DISK"
           DISPLAY    " Record length : 111 characters (scalar record)"
           DISPLAY    " Primary key   : ACTOR-ID  PIC 9(9), no duplicates"
           DISPLAY    " Fields        : ACTOR-ID     PIC 9(9)"
           DISPLAY    "                 ACTOR-THUMB  PIC X(60)"
           DISPLAY    "                 ACTOR-CAPTION PIC X(30)"
           DISPLAY    "                 ACTOR-SALARY PIC 9(9)V99"
           DISPLAY    "                 ACTOR-AWARDS PIC X"
           DISPLAY    "-----------------------------------------------"

           SET     WS-ED                         TO    WS-WRITTEN
           DISPLAY    " Records written : " WS-ED
           SET     WS-ED                         TO    WS-FAILED

           DISPLAY    " Records failed  : " WS-ED
           DISPLAY    " Expected        : 1098  (100 seeded + 998 generated)"

           PERFORM    REPORT-GENERATED

           DISPLAY    "==============================================="
           DISPLAY    " ".

       LOAD-GENERATED-CAST.
           *> 998 rows, ACTOR-ID 101..1098. The country cycles every row, so a
           *> grid scrolled a screen at a time shows all seven mixed together
           *> rather than seven long blocks.

           PERFORM    VARYING WS-N FROM 1 BY 1 UNTIL WS-N > 998
               DIVIDE    WS-N BY 7 GIVING WS-IDX REMAINDER WS-C
               *>     WS-C is 1..7 after the ADD; WS-IDX counts rows within it.

               ADD    1 TO WS-C
               *> WS-IDX is the row's turn within its country (0-based): 7 consecutive
               *> rows share it, one per country. Its remainder by 16 picks the first
               *> name and its quotient the last, so no pair repeats in a country.

               COMPUTE    WS-IDX = (WS-N - 1) / 7
               DIVIDE    WS-IDX BY 16 GIVING WS-LI REMAINDER WS-FI

               ADD    1 TO WS-FI
               ADD    1 TO WS-LI

               *>     Into the country's own 16-name window.

               COMPUTE    WS-FI = (WS-C - 1) * 16 + WS-FI
               COMPUTE    WS-LI = (WS-C - 1) * 16 + WS-LI

               SET     WS-NAME                   TO    SPACES
               STRING    WS-FIRST(WS-FI)::Trim() " " WS-LAST(WS-LI)::Trim() INTO WS-NAME
               END-STRING
               *>     Reuse the 100 photos that exist rather than name files that
               *>     do not.

               DIVIDE    WS-N BY 100 GIVING WS-REM REMAINDER WS-PHOTO
               ADD    1 TO WS-PHOTO

               SET     WS-PHOTO-ED               TO    WS-PHOTO
               SET     WS-THUMB                  TO    SPACES

               STRING    "~/PowerDemo2/assets/images/photo" WS-PHOTO-ED::Trim() ".jpg" INTO WS-THUMB
               END-STRING
               COMPUTE    ACTOR-ID = 100 + WS-N

               SET     ACTOR-THUMB               TO    WS-THUMB
               SET     ACTOR-CAPTION             TO    WS-NAME

               COMPUTE    ACTOR-SALARY = 500 + WS-IDX * 31
               DIVIDE    WS-N BY 4 GIVING WS-REM REMAINDER WS-C
               SET     ACTOR-AWARDS              TO    WS-C
               *>     WS-C was reused for the award digit, so recompute the
               *>     country before counting it.

               DIVIDE    WS-N BY 7 GIVING WS-REM REMAINDER WS-C

               ADD    1 TO WS-C
               ADD    1 TO WS-PER-COUNTRY(WS-C)
               ADD    1 TO WS-GEN

               PERFORM    WRITE-ACTOR
           END-PERFORM.

       REPORT-GENERATED.
           DISPLAY    " "
           DISPLAY    "-----------------------------------------------"
           DISPLAY    " Generated cast (ACTOR-ID 101..1098)"
           DISPLAY    "-----------------------------------------------"

           SET     WS-ED5                        TO    WS-GEN
           DISPLAY    " Rows generated  : " WS-ED5
           PERFORM    VARYING WS-C FROM 1 BY 1 UNTIL WS-C > 7
               SET     WS-ED                     TO    WS-PER-COUNTRY(WS-C)
               DISPLAY    "   " WS-COUNTRY(WS-C) " : " WS-ED
           END-PERFORM
           DISPLAY    "-----------------------------------------------".

       LOAD-ACTORS.

           *> ---- actor   1 ----

           SET     ACTOR-ID                      TO    000000001
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo1.jpg"
           SET     ACTOR-CAPTION                 TO    "Leonardo DiCaprio"
           SET     ACTOR-SALARY                  TO    3000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor   2 ----

           SET     ACTOR-ID                      TO    000000002
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo2.jpg"
           SET     ACTOR-CAPTION                 TO    "Joe Pesci"
           SET     ACTOR-SALARY                  TO    1200.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor   3 ----

           SET     ACTOR-ID                      TO    000000003
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo3.jpg"
           SET     ACTOR-CAPTION                 TO    "Robert De Niro"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor   4 ----

           SET     ACTOR-ID                      TO    000000004
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo4.jpg"
           SET     ACTOR-CAPTION                 TO    "Al Pacino"
           SET     ACTOR-SALARY                  TO    1800.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor   5 ----

           SET     ACTOR-ID                      TO    000000005
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo5.jpg"
           SET     ACTOR-CAPTION                 TO    "Marlon Brando"
           SET     ACTOR-SALARY                  TO    1500.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor   6 ----

           SET     ACTOR-ID                      TO    000000006
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo6.jpg"
           SET     ACTOR-CAPTION                 TO    "Jack Nicholson"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor   7 ----

           SET     ACTOR-ID                      TO    000000007
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo7.jpg"
           SET     ACTOR-CAPTION                 TO    "Daniel Day-Lewis"
           SET     ACTOR-SALARY                  TO    1200.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor   8 ----

           SET     ACTOR-ID                      TO    000000008
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo8.jpg"
           SET     ACTOR-CAPTION                 TO    "Denzel Washington"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor   9 ----

           SET     ACTOR-ID                      TO    000000009
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo9.jpg"
           SET     ACTOR-CAPTION                 TO    "Tom Hanks"
           SET     ACTOR-SALARY                  TO    2500.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  10 ----

           SET     ACTOR-ID                      TO    000000010
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo10.jpg"
           SET     ACTOR-CAPTION                 TO    "Morgan Freeman"
           SET     ACTOR-SALARY                  TO    1200.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  11 ----

           SET     ACTOR-ID                      TO    000000011
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo11.jpg"
           SET     ACTOR-CAPTION                 TO    "Anthony Hopkins"
           SET     ACTOR-SALARY                  TO    1500.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  12 ----

           SET     ACTOR-ID                      TO    000000012
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo12.jpg"
           SET     ACTOR-CAPTION                 TO    "Christian Bale"
           SET     ACTOR-SALARY                  TO    1500.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  13 ----

           SET     ACTOR-ID                      TO    000000013
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo13.jpg"
           SET     ACTOR-CAPTION                 TO    "Gary Oldman"
           SET     ACTOR-SALARY                  TO    1000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  14 ----

           SET     ACTOR-ID                      TO    000000014
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo14.jpg"
           SET     ACTOR-CAPTION                 TO    "Samuel L. Jackson"
           SET     ACTOR-SALARY                  TO    1500.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  15 ----

           SET     ACTOR-ID                      TO    000000015
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo15.jpg"
           SET     ACTOR-CAPTION                 TO    "Brad Pitt"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  16 ----

           SET     ACTOR-ID                      TO    000000016
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo16.jpg"
           SET     ACTOR-CAPTION                 TO    "George Clooney"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  17 ----

           SET     ACTOR-ID                      TO    000000017
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo17.jpg"
           SET     ACTOR-CAPTION                 TO    "Johnny Depp"
           SET     ACTOR-SALARY                  TO    2000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  18 ----

           SET     ACTOR-ID                      TO    000000018
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo18.jpg"
           SET     ACTOR-CAPTION                 TO    "Keanu Reeves"
           SET     ACTOR-SALARY                  TO    1800.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  19 ----

           SET     ACTOR-ID                      TO    000000019
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo19.jpg"
           SET     ACTOR-CAPTION                 TO    "Tom Cruise"
           SET     ACTOR-SALARY                  TO    300000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  20 ----

           SET     ACTOR-ID                      TO    000000020
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo20.jpg"
           SET     ACTOR-CAPTION                 TO    "Matt Damon"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  21 ----

           SET     ACTOR-ID                      TO    000000021
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo21.jpg"
           SET     ACTOR-CAPTION                 TO    "Ben Affleck"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  22 ----

           SET     ACTOR-ID                      TO    000000022
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo22.jpg"
           SET     ACTOR-CAPTION                 TO    "Joaquin Phoenix"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  23 ----

           SET     ACTOR-ID                      TO    000000023
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo23.jpg"
           SET     ACTOR-CAPTION                 TO    "Russell Crowe"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  24 ----

           SET     ACTOR-ID                      TO    000000024
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo24.jpg"
           SET     ACTOR-CAPTION                 TO    "Harrison Ford"
           SET     ACTOR-SALARY                  TO    180000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  25 ----

           SET     ACTOR-ID                      TO    000000025
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo25.jpg"
           SET     ACTOR-CAPTION                 TO    "Clint Eastwood"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  26 ----

           SET     ACTOR-ID                      TO    000000026
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo26.jpg"
           SET     ACTOR-CAPTION                 TO    "Sean Connery"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  27 ----

           SET     ACTOR-ID                      TO    000000027
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo27.jpg"
           SET     ACTOR-CAPTION                 TO    "Michael Caine"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  28 ----

           SET     ACTOR-ID                      TO    000000028
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo28.jpg"
           SET     ACTOR-CAPTION                 TO    "Liam Neeson"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  29 ----

           SET     ACTOR-ID                      TO    000000029
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo29.jpg"
           SET     ACTOR-CAPTION                 TO    "Bruce Willis"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  30 ----

           SET     ACTOR-ID                      TO    000000030
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo30.jpg"
           SET     ACTOR-CAPTION                 TO    "Arnold Schwarzenegger"
           SET     ACTOR-SALARY                  TO    200000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  31 ----

           SET     ACTOR-ID                      TO    000000031
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo31.jpg"
           SET     ACTOR-CAPTION                 TO    "Sylvester Stallone"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  32 ----

           SET     ACTOR-ID                      TO    000000032
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo32.jpg"
           SET     ACTOR-CAPTION                 TO    "Will Smith"
           SET     ACTOR-SALARY                  TO    250000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  33 ----

           SET     ACTOR-ID                      TO    000000033
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo33.jpg"
           SET     ACTOR-CAPTION                 TO    "Jamie Foxx"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  34 ----

           SET     ACTOR-ID                      TO    000000034
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo34.jpg"
           SET     ACTOR-CAPTION                 TO    "Forest Whitaker"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  35 ----

           SET     ACTOR-ID                      TO    000000035
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo35.jpg"
           SET     ACTOR-CAPTION                 TO    "Laurence Fishburne"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  36 ----

           SET     ACTOR-ID                      TO    000000036
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo36.jpg"
           SET     ACTOR-CAPTION                 TO    "Idris Elba"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  37 ----

           SET     ACTOR-ID                      TO    000000037
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo37.jpg"
           SET     ACTOR-CAPTION                 TO    "Mahershala Ali"
           SET     ACTOR-SALARY                  TO    60000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  38 ----

           SET     ACTOR-ID                      TO    000000038
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo38.jpg"
           SET     ACTOR-CAPTION                 TO    "Chadwick Boseman"
           SET     ACTOR-SALARY                  TO    50000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  39 ----

           SET     ACTOR-ID                      TO    000000039
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo39.jpg"
           SET     ACTOR-CAPTION                 TO    "Michael B. Jordan"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  40 ----

           SET     ACTOR-ID                      TO    000000040
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo40.jpg"
           SET     ACTOR-CAPTION                 TO    "Dwayne Johnson"
           SET     ACTOR-SALARY                  TO    250000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  41 ----

           SET     ACTOR-ID                      TO    000000041
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo41.jpg"
           SET     ACTOR-CAPTION                 TO    "Ryan Reynolds"
           SET     ACTOR-SALARY                  TO    200000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  42 ----

           SET     ACTOR-ID                      TO    000000042
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo42.jpg"
           SET     ACTOR-CAPTION                 TO    "Hugh Jackman"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  43 ----

           SET     ACTOR-ID                      TO    000000043
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo43.jpg"
           SET     ACTOR-CAPTION                 TO    "Patrick Stewart"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  44 ----

           SET     ACTOR-ID                      TO    000000044
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo44.jpg"
           SET     ACTOR-CAPTION                 TO    "Ian McKellen"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  45 ----

           SET     ACTOR-ID                      TO    000000045
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo45.jpg"
           SET     ACTOR-CAPTION                 TO    "Benedict Cumberbatch"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  46 ----

           SET     ACTOR-ID                      TO    000000046
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo46.jpg"
           SET     ACTOR-CAPTION                 TO    "Tom Hardy"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  47 ----

           SET     ACTOR-ID                      TO    000000047
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo47.jpg"
           SET     ACTOR-CAPTION                 TO    "Cillian Murphy"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  48 ----

           SET     ACTOR-ID                      TO    000000048
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo48.jpg"
           SET     ACTOR-CAPTION                 TO    "Ralph Fiennes"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  49 ----

           SET     ACTOR-ID                      TO    000000049
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo49.jpg"
           SET     ACTOR-CAPTION                 TO    "Edward Norton"
           SET     ACTOR-SALARY                  TO    90000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  50 ----

           SET     ACTOR-ID                      TO    000000050
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo50.jpg"
           SET     ACTOR-CAPTION                 TO    "Mark Ruffalo"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  51 ----

           SET     ACTOR-ID                      TO    000000051
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo51.jpg"
           SET     ACTOR-CAPTION                 TO    "Robert Downey Jr."
           SET     ACTOR-SALARY                  TO    400000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  52 ----

           SET     ACTOR-ID                      TO    000000052
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo52.jpg"
           SET     ACTOR-CAPTION                 TO    "Chris Evans"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  53 ----

           SET     ACTOR-ID                      TO    000000053
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo53.jpg"
           SET     ACTOR-CAPTION                 TO    "Chris Hemsworth"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  54 ----

           SET     ACTOR-ID                      TO    000000054
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo54.jpg"
           SET     ACTOR-CAPTION                 TO    "Jeremy Renner"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  55 ----

           SET     ACTOR-ID                      TO    000000055
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo55.jpg"
           SET     ACTOR-CAPTION                 TO    "Paul Rudd"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  56 ----

           SET     ACTOR-ID                      TO    000000056
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo56.jpg"
           SET     ACTOR-CAPTION                 TO    "Don Cheadle"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  57 ----

           SET     ACTOR-ID                      TO    000000057
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo57.jpg"
           SET     ACTOR-CAPTION                 TO    "Chris Pratt"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  58 ----

           SET     ACTOR-ID                      TO    000000058
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo58.jpg"
           SET     ACTOR-CAPTION                 TO    "Vin Diesel"
           SET     ACTOR-SALARY                  TO    180000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  59 ----

           SET     ACTOR-ID                      TO    000000059
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo59.jpg"
           SET     ACTOR-CAPTION                 TO    "Jason Statham"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  60 ----

           SET     ACTOR-ID                      TO    000000060
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo60.jpg"
           SET     ACTOR-CAPTION                 TO    "Jackie Chan"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  61 ----

           SET     ACTOR-ID                      TO    000000061
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo61.jpg"
           SET     ACTOR-CAPTION                 TO    "Jet Li"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  62 ----

           SET     ACTOR-ID                      TO    000000062
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo62.jpg"
           SET     ACTOR-CAPTION                 TO    "Tony Leung"
           SET     ACTOR-SALARY                  TO    60000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  63 ----

           SET     ACTOR-ID                      TO    000000063
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo63.jpg"
           SET     ACTOR-CAPTION                 TO    "Ken Watanabe"
           SET     ACTOR-SALARY                  TO    50000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  64 ----

           SET     ACTOR-ID                      TO    000000064
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo64.jpg"
           SET     ACTOR-CAPTION                 TO    "Song Kang-ho"
           SET     ACTOR-SALARY                  TO    40000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  65 ----

           SET     ACTOR-ID                      TO    000000065
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo65.jpg"
           SET     ACTOR-CAPTION                 TO    "Toshiro Mifune"
           SET     ACTOR-SALARY                  TO    30000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  66 ----

           SET     ACTOR-ID                      TO    000000066
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo66.jpg"
           SET     ACTOR-CAPTION                 TO    "Mads Mikkelsen"
           SET     ACTOR-SALARY                  TO    60000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  67 ----

           SET     ACTOR-ID                      TO    000000067
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo67.jpg"
           SET     ACTOR-CAPTION                 TO    "Christoph Waltz"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  68 ----

           SET     ACTOR-ID                      TO    000000068
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo68.jpg"
           SET     ACTOR-CAPTION                 TO    "Javier Bardem"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  69 ----

           SET     ACTOR-ID                      TO    000000069
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo69.jpg"
           SET     ACTOR-CAPTION                 TO    "Antonio Banderas"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  70 ----

           SET     ACTOR-ID                      TO    000000070
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo70.jpg"
           SET     ACTOR-CAPTION                 TO    "Pedro Pascal"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  71 ----

           SET     ACTOR-ID                      TO    000000071
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo71.jpg"
           SET     ACTOR-CAPTION                 TO    "Oscar Isaac"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  72 ----

           SET     ACTOR-ID                      TO    000000072
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo72.jpg"
           SET     ACTOR-CAPTION                 TO    "Gael Garcia Bernal"
           SET     ACTOR-SALARY                  TO    40000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  73 ----

           SET     ACTOR-ID                      TO    000000073
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo73.jpg"
           SET     ACTOR-CAPTION                 TO    "Diego Luna"
           SET     ACTOR-SALARY                  TO    40000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  74 ----

           SET     ACTOR-ID                      TO    000000074
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo74.jpg"
           SET     ACTOR-CAPTION                 TO    "Wagner Moura"
           SET     ACTOR-SALARY                  TO    30000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  75 ----

           SET     ACTOR-ID                      TO    000000075
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo75.jpg"
           SET     ACTOR-CAPTION                 TO    "Rodrigo Santoro"
           SET     ACTOR-SALARY                  TO    30000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  76 ----

           SET     ACTOR-ID                      TO    000000076
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo76.jpg"
           SET     ACTOR-CAPTION                 TO    "Tobey Maguire"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  77 ----

           SET     ACTOR-ID                      TO    000000077
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo77.jpg"
           SET     ACTOR-CAPTION                 TO    "Andrew Garfield"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  78 ----

           SET     ACTOR-ID                      TO    000000078
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo78.jpg"
           SET     ACTOR-CAPTION                 TO    "Tom Holland"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  79 ----

           SET     ACTOR-ID                      TO    000000079
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo79.jpg"
           SET     ACTOR-CAPTION                 TO    "Heath Ledger"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  80 ----

           SET     ACTOR-ID                      TO    000000080
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo80.jpg"
           SET     ACTOR-CAPTION                 TO    "Jake Gyllenhaal"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  81 ----

           SET     ACTOR-ID                      TO    000000081
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo81.jpg"
           SET     ACTOR-CAPTION                 TO    "Ryan Gosling"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  82 ----

           SET     ACTOR-ID                      TO    000000082
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo82.jpg"
           SET     ACTOR-CAPTION                 TO    "Matthew McConaughey"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  83 ----

           SET     ACTOR-ID                      TO    000000083
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo83.jpg"
           SET     ACTOR-CAPTION                 TO    "Woody Harrelson"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  84 ----

           SET     ACTOR-ID                      TO    000000084
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo84.jpg"
           SET     ACTOR-CAPTION                 TO    "Kevin Costner"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  85 ----

           SET     ACTOR-ID                      TO    000000085
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo85.jpg"
           SET     ACTOR-CAPTION                 TO    "John Travolta"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  86 ----

           SET     ACTOR-ID                      TO    000000086
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo86.jpg"
           SET     ACTOR-CAPTION                 TO    "Nicolas Cage"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  87 ----

           SET     ACTOR-ID                      TO    000000087
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo87.jpg"
           SET     ACTOR-CAPTION                 TO    "John Malkovich"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  88 ----

           SET     ACTOR-ID                      TO    000000088
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo88.jpg"
           SET     ACTOR-CAPTION                 TO    "Steve Buscemi"
           SET     ACTOR-SALARY                  TO    40000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  89 ----

           SET     ACTOR-ID                      TO    000000089
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo89.jpg"
           SET     ACTOR-CAPTION                 TO    "Philip Seymour Hoffman"
           SET     ACTOR-SALARY                  TO    60000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  90 ----

           SET     ACTOR-ID                      TO    000000090
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo90.jpg"
           SET     ACTOR-CAPTION                 TO    "William H. Macy"
           SET     ACTOR-SALARY                  TO    40000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  91 ----

           SET     ACTOR-ID                      TO    000000091
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo91.jpg"
           SET     ACTOR-CAPTION                 TO    "Jeff Bridges"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  92 ----

           SET     ACTOR-ID                      TO    000000092
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo92.jpg"
           SET     ACTOR-CAPTION                 TO    "Kurt Russell"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR

           *> ---- actor  93 ----

           SET     ACTOR-ID                      TO    000000093
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo93.jpg"
           SET     ACTOR-CAPTION                 TO    "Bill Murray"
           SET     ACTOR-SALARY                  TO    80000.00
           SET     ACTOR-AWARDS                  TO    "4"

           PERFORM    WRITE-ACTOR

           *> ---- actor  94 ----

           SET     ACTOR-ID                      TO    000000094
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo94.jpg"
           SET     ACTOR-CAPTION                 TO    "Dan Aykroyd"
           SET     ACTOR-SALARY                  TO    50000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  95 ----

           SET     ACTOR-ID                      TO    000000095
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo95.jpg"
           SET     ACTOR-CAPTION                 TO    "Eddie Murphy"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  96 ----

           SET     ACTOR-ID                      TO    000000096
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo96.jpg"
           SET     ACTOR-CAPTION                 TO    "Jim Carrey"
           SET     ACTOR-SALARY                  TO    150000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor  97 ----

           SET     ACTOR-ID                      TO    000000097
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo97.jpg"
           SET     ACTOR-CAPTION                 TO    "Robin Williams"
           SET     ACTOR-SALARY                  TO    120000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  98 ----

           SET     ACTOR-ID                      TO    000000098
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo98.jpg"
           SET     ACTOR-CAPTION                 TO    "Gene Hackman"
           SET     ACTOR-SALARY                  TO    100000.00
           SET     ACTOR-AWARDS                  TO    "1"

           PERFORM    WRITE-ACTOR

           *> ---- actor  99 ----

           SET     ACTOR-ID                      TO    000000099
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo99.jpg"
           SET     ACTOR-CAPTION                 TO    "Christopher Walken"
           SET     ACTOR-SALARY                  TO    70000.00
           SET     ACTOR-AWARDS                  TO    "2"

           PERFORM    WRITE-ACTOR

           *> ---- actor 100 ----

           SET     ACTOR-ID                      TO    000000100
           SET     ACTOR-THUMB                   TO    "~/PowerDemo2/assets/images/photo100.jpg"
           SET     ACTOR-CAPTION                 TO    "Christopher Lee"
           SET     ACTOR-SALARY                  TO    50000.00
           SET     ACTOR-AWARDS                  TO    "3"

           PERFORM    WRITE-ACTOR.

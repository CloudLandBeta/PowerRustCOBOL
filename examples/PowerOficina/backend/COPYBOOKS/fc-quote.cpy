      *> The SELECTs of the quote files, with the alternate keys the generated .SEL members carry.
           SELECT QUOTE-FILE ASSIGN TO "data/quotes.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QUO-KEY
               ALTERNATE RECORD KEY IS QUO-CUS-ID WITH DUPLICATES
               ALTERNATE RECORD KEY IS QUO-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-QUO.
           SELECT QUOTE-ITEM-FILE ASSIGN TO "data/quoteitems.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS QIT-KEY
               FILE STATUS IS WS-FS-QIT.
           SELECT CATALOG-FILE ASSIGN TO "data/catalog.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CAT-KEY
               FILE STATUS IS WS-FS-CAT.

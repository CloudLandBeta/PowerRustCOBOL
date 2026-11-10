      *> QUO-REQ: what the Quote screen sends to QUOTE-SAVE.
      *> QRQ-ID        0 = a new quote; a number = a change of that quote: it is saved as the next revision and the
      *>               earlier revisions stay as they were (R17). If the currency differs from that quote's, a NEW
      *>               quote is made with the prices of the other currency; the original is never converted (R43).
      *> QRQ-CURRENCY  BRL or EUR, fixed when the quote is created (R38, R43).
      *> QRQ-DEADLINE  the agreed execution date, YYYYMMDD; 0 = take the date the queue suggests (R16).
      *> QRQ-LINE      the parts (kind P), services (S) and add-ons (A) chosen. Only the code and the quantity are sent:
      *>               the Backend takes every price, labour rate and tax rate from its own tables, in the currency.
      *>               QRL-HOURS on a service: 0 = the standard time; another value needs a reason in QRL-NOTE (R12).
      *>               QRL-MANUAL = S when the consultant added or changed the line by hand (R10).
       01 QUO-REQ.
          05 QRQ-ID                              PIC    9(8).
          05 QRQ-PLATE                           PIC    X(8).
          05 QRQ-DGN-ID                          PIC    9(8).
          05 QRQ-CURRENCY                        PIC    X(3).
          05 QRQ-OPERATOR                        PIC    X(30).
          05 QRQ-DEADLINE                        PIC    9(8).
          05 QRQ-COUNT                           PIC    9(3).
          05 QRQ-LINE                            OCCURS 20.
             10 QRL-KIND                         PIC    X(1).
             10 QRL-CODE                         PIC    X(12).
             10 QRL-QTY                          PIC    9(4)V99.
             10 QRL-HOURS                        PIC    9(3)V99.
             10 QRL-MANUAL                       PIC    X(1).
             10 QRL-NOTE                         PIC    X(60).

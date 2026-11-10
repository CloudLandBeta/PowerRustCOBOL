      *> QAP-REQ: the answer to a quote (R21, R22, R37).
      *> QAP-MODE     L = the customer, through the link (QAP-TOKEN or QAP-QUO-ID, and QAP-CODE); M = the consultant registers
      *>              an answer received by phone, WhatsApp or at the counter (QAP-QUO-ID, QAP-CHANNEL, QAP-OPERATOR).
      *> QAP-DECISION A = approves the quote and its execution date; R = refuses or asks for a change (QAP-REASON required).
       01 QAP-REQ.
          05 QAP-MODE                            PIC    X(1).
          05 QAP-TOKEN                           PIC    X(32).
          05 QAP-QUO-ID                          PIC    9(8).
          05 QAP-CODE                            PIC    X(6).
          05 QAP-CHANNEL                         PIC    X(10).
          05 QAP-DECISION                        PIC    X(1).
          05 QAP-REASON                          PIC    X(80).
          05 QAP-OPERATOR                        PIC    X(30).

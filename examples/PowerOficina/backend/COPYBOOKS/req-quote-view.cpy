      *> QVW-REQ: the customer opens the approval page. QVW-TOKEN is the token of the link; when the page cannot read it
      *> from the address, it is left blank and the quote number plus the short code identify the quote.
      *> The code is always required, so a link alone does not open a quote (R37).
       01 QVW-REQ.
          05 QVW-TOKEN                           PIC    X(32).
          05 QVW-QUO-ID                          PIC    9(8).
          05 QVW-CODE                            PIC    X(6).

      *> CUS-RES: the answer of CUSTOMER-SAVE.
      *> CRS-STATUS  OK | INVALID | DUPLICATE | NOTFOUND | ERROR
      *> CRS-FIELD   with INVALID, the field that failed: NAME DOC EMAIL PHONE CONSENT OPERATOR.
      *> CRS-ID      the customer saved; with DUPLICATE, the customer that already carries that document.
       01 CUS-RES.
          05 CRS-STATUS                          PIC    X(10).
          05 CRS-MESSAGE                         PIC    X(80).
          05 CRS-FIELD                           PIC    X(10).
          05 CRS-ID                              PIC    9(8).

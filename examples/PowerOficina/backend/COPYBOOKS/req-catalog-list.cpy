      *> CLS-REQ: asks CATALOG-LIST for the catalog rows of one currency and one kind
      *> (P part, S service, A add-on). Every list shows one currency only (R42).
       01 CLS-REQ.
          05 CLQ-CURRENCY                        PIC    X(3).
          05 CLQ-KIND                            PIC    X(1).

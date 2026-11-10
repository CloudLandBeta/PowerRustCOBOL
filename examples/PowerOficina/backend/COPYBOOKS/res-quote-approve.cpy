      *> QAP-RES: what the answer did.
      *> QAS-STATUS  OK | INVALID | NOTFOUND | BADCODE | USED | REPLACED | EXPIRED | NOTOPEN | ERROR
      *> QAS-FIELD   with INVALID: MODE DECISION CHANNEL REASON OPERATOR.
      *> QAS-QSTATUS the state the quote is in now: APROVADO (approved, waiting for the payment) or RECUSADO.
       01 QAP-RES.
          05 QAS-STATUS                          PIC    X(10).
          05 QAS-MESSAGE                         PIC    X(80).
          05 QAS-FIELD                           PIC    X(10).
          05 QAS-QUO-ID                          PIC    9(8).
          05 QAS-REV                             PIC    9(2).
          05 QAS-QSTATUS                         PIC    X(10).
          05 QAS-AT                              PIC    9(14).
          05 QAS-CHANNEL                         PIC    X(10).
